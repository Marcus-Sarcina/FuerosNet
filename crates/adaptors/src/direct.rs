//! The direct payload path as the client's interface sees it, over the
//! transport's socket (design §14.1.1, §12.6.3): the client's own for a
//! light client, the node's for a node that is a participant.

use crate::serving::Inbound;
use rhtn_archive::Keyhash;
use rhtn_client::device::DirectPath;
use rhtn_node::runtime::LiveNode;
use rhtn_transport::bind::Binding;
use rhtn_transport::session::{CLOSE_REFUSED, deliver};
use rhtn_transport::tls::{self, Pins};
use rhtn_transport::traversal::{Candidate, TraversalSocket, connect_direct, endpoint};
use std::collections::{HashMap, HashSet};
use std::future::Future;
use std::io;
use std::net::SocketAddr;
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// Which peers the direct path reaches now: the transport side sets it,
/// the client reads it to choose a route, and nothing else crosses.
#[derive(Clone, Default)]
pub struct Reachable(Arc<Mutex<HashSet<Keyhash>>>);

impl Reachable {
    pub fn set(&self, peer: Keyhash, held: bool) {
        let mut s = self.0.lock().unwrap();
        if held {
            s.insert(peer);
        } else {
            s.remove(&peer);
        }
    }

    pub fn holds(&self, peer: &Keyhash) -> bool {
        self.0.lock().unwrap().contains(peer)
    }
}

impl DirectPath for Reachable {
    fn reachable(&self, peer: &Keyhash) -> bool {
        self.holds(peer)
    }
}

/// Whether the path to a peer may be direct at all: inside the horizon
/// absent an override (design §12.6.3).  A node reads its own table; a
/// light client keeps none and asks the node beside it.
pub type Gate = Arc<dyn Fn(&Keyhash) -> bool + Send + Sync>;

type Fut<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

/// The transport side of the direct path.
pub trait Direct: Send + Sync {
    fn reachable(&self) -> Reachable;
    /// This side's candidates for `peer`, or none where the path may not
    /// be direct: then nothing is gathered, exchanged or dialled.
    fn gather(&self, peer: Keyhash) -> Fut<'_, Option<Vec<Candidate>>>;
    /// Dial the peer's candidates and hold what opens.
    fn open(&self, peer: Keyhash, candidates: Vec<Candidate>) -> Fut<'_, bool>;
    /// Deliver on the path held; false leaves the message with the caller,
    /// and the relay is next.
    fn deliver(&self, peer: Keyhash, bytes: Vec<u8>) -> Fut<'_, bool>;
}

/// No direct path to anyone.
pub struct NoDirect(pub Reachable);

impl Direct for NoDirect {
    fn reachable(&self) -> Reachable {
        self.0.clone()
    }
    fn gather(&self, _: Keyhash) -> Fut<'_, Option<Vec<Candidate>>> {
        Box::pin(async { None })
    }
    fn open(&self, _: Keyhash, _: Vec<Candidate>) -> Fut<'_, bool> {
        Box::pin(async { false })
    }
    fn deliver(&self, _: Keyhash, _: Vec<u8>) -> Fut<'_, bool> {
        Box::pin(async { false })
    }
}

struct Light {
    /// What this side presents when it dials: its own classical member, or
    /// the delegated key of a device that holds no seed (design §23.3).
    presenter: tls::Presenter,
    pins: Pins,
    /// §9.1's bind for a peer known by the key it presented: the pin, or
    /// a delegation held in the client's horizon view (`light-client-
    /// requirements.md` §4.2).
    bind: Binding,
    socket: Arc<TraversalSocket>,
    endpoint: quinn::Endpoint,
    stun: Option<SocketAddr>,
    gate: Gate,
    inbound: Inbound,
    conns: Mutex<HashMap<Keyhash, quinn::Connection>>,
    reachable: Reachable,
    dial_timeout: Duration,
    /// Where the checks' events go: nowhere by default.
    log: rhtn_transport::session::Log,
}

/// A light client's own socket: candidates gathered on it, the peer's
/// dialled from it, and what the peer opens toward it accepted under the
/// key it authenticates, held and read.
#[derive(Clone)]
pub struct LightDirect(Arc<Light>);

impl LightDirect {
    /// Bind at `addr`, behind `nat` on a harness, asking `stun` for the
    /// reflexive address where one is given.  What arrives from a peer
    /// goes to `inbound`.
    #[allow(clippy::too_many_arguments)]
    pub fn bind(
        me: tls::Party,
        pins: Pins,
        bind: Binding,
        reachable: Reachable,
        addr: SocketAddr,
        nat: Option<SocketAddr>,
        stun: Option<SocketAddr>,
        gate: Gate,
        inbound: Inbound,
    ) -> io::Result<LightDirect> {
        let socket = TraversalSocket::bind(addr, nat)?;
        let presenter = match &bind.credential {
            Some(c) => tls::Presenter::Delegated(c.clone()),
            None => me.presenter.clone(),
        };
        let crypto =
            quinn::crypto::rustls::QuicServerConfig::try_from(tls::server_config(&presenter))
                .map_err(|e| io::Error::other(e.to_string()))?;
        let mut qcfg = quinn::ServerConfig::with_crypto(Arc::new(crypto));
        qcfg.transport_config(Arc::new(tls::transport_config()));
        let endpoint = endpoint(socket.clone(), Some(qcfg))?;
        let light = LightDirect(Arc::new(Light {
            presenter: presenter.clone(),
            pins,
            bind,
            socket,
            endpoint: endpoint.clone(),
            stun,
            gate,
            inbound,
            conns: Mutex::new(HashMap::new()),
            reachable,
            dial_timeout: Duration::from_secs(3),
            log: rhtn_transport::session::Log::default(),
        }));
        // what a peer opens toward this socket
        let accepting = light.clone();
        tokio::spawn(async move {
            while let Some(incoming) = endpoint.accept().await {
                let l = accepting.clone();
                tokio::spawn(async move {
                    if let Ok(conn) = incoming.await {
                        l.hold(conn);
                    }
                });
            }
        });
        Ok(light)
    }

    pub fn addr(&self) -> io::Result<SocketAddr> {
        self.0.socket.addr()
    }

    /// Hold `conn` under the peer it authenticates and read what it
    /// delivers until it closes.  The presented key is bound by the pin
    /// or by a delegation held in the horizon view, with no frame
    /// (`wire-format.md` §9.1); a peer bound by neither is closed unheld.
    fn hold(&self, conn: quinn::Connection) -> bool {
        let Some(peer) = tls::peer_key(&conn).and_then(|k| {
            self.0.pins.keyhash_for_key(&k).or_else(|| {
                self.0
                    .bind
                    .held
                    .by_key(&k)
                    .filter(|d| self.0.bind.in_window(d))
                    .map(|d| d.keyhash)
            })
        }) else {
            conn.close(quinn::VarInt::from_u32(CLOSE_REFUSED), b"unbound");
            return false;
        };
        // A connection this side would not have dialled is not one it
        // accepts: the direct path is refused in both directions by the
        // same local decision, or a peer outside the horizon opens it by
        // dialling first.
        if !(self.0.gate)(&peer) {
            conn.close(quinn::VarInt::from_u32(CLOSE_REFUSED), b"no direct path");
            return false;
        }
        self.0.conns.lock().unwrap().insert(peer, conn.clone());
        self.0.reachable.set(peer, true);
        let l = self.clone();
        tokio::spawn(async move {
            while let Ok(mut s) = conn.accept_uni().await {
                if let Ok(bytes) = s.read_to_end(1 << 20).await {
                    // no binding: nothing relayed this, so there was no
                    // node in the path to carry one (`wire-format.md`
                    // §7.10)
                    (l.0.inbound)(peer, bytes, None);
                }
            }
            // gone: the path is down unless a newer connection replaced this one
            let mut c = l.0.conns.lock().unwrap();
            if c.get(&peer)
                .is_some_and(|h| h.stable_id() == conn.stable_id())
            {
                c.remove(&peer);
                l.0.reachable.set(peer, false);
            }
        });
        true
    }
}

impl Direct for LightDirect {
    fn reachable(&self) -> Reachable {
        self.0.reachable.clone()
    }

    fn gather(&self, peer: Keyhash) -> Fut<'_, Option<Vec<Candidate>>> {
        Box::pin(async move {
            if !(self.0.gate)(&peer) {
                return None;
            }
            Some(
                self.0
                    .socket
                    .gather(self.0.stun, Duration::from_secs(2))
                    .await,
            )
        })
    }

    fn open(&self, peer: Keyhash, candidates: Vec<Candidate>) -> Fut<'_, bool> {
        Box::pin(async move {
            // The decision is this side's (design §12.6.3): a peer willing
            // to connect is not permission to connect to it.  Asked again
            // here rather than trusted from the gather, because candidates
            // arrive from the peer and nothing else on this path consulted
            // policy.
            if !(self.0.gate)(&peer) {
                return false;
            }
            let me = self.0.presenter.clone();
            match connect_direct(
                &self.0.endpoint,
                &me,
                &self.0.pins,
                &self.0.bind,
                &peer,
                &candidates,
                self.0.dial_timeout,
                &self.0.log,
            )
            .await
            {
                Some(conn) => self.hold(conn),
                None => false,
            }
        })
    }

    fn deliver(&self, peer: Keyhash, bytes: Vec<u8>) -> Fut<'_, bool> {
        Box::pin(async move {
            let conn = self.0.conns.lock().unwrap().get(&peer).cloned();
            let Some(c) = conn else {
                return false;
            };
            // **a delivery is bounded by the dial's own timeout.** The
            // direct path has no heartbeat: a peer that went away leaves a
            // connection nothing closes until QUIC's idle timer, and a
            // stream it will never acknowledge.  A delivery that does not
            // complete in the time a dial gets is a path no longer held,
            // and the message goes to the relay (design §14.1.1)
            let ok = tokio::time::timeout(self.0.dial_timeout, deliver(&c, bytes))
                .await
                .unwrap_or(false);
            if !ok {
                self.0.conns.lock().unwrap().remove(&peer);
                self.0.reachable.set(peer, false);
                c.close(quinn::VarInt::from_u32(0), b"path lost");
            }
            ok
        })
    }
}

/// The node's own socket, for a node that is a participant: the node
/// gathers inside its horizon, dials and holds the path per peer already
/// (`rhtn-node`'s `LiveNode`); this reads and writes it as the client's
/// interface.
pub struct NodeDirect {
    node: Arc<LiveNode>,
    pins: Pins,
    reachable: Reachable,
}

impl NodeDirect {
    /// Payload arriving on the node's direct path goes to `inbound`.  The
    /// node's direct inbox is taken here, once.
    pub fn new(node: Arc<LiveNode>, pins: Pins, inbound: Inbound) -> Arc<NodeDirect> {
        let mut inbox = node.take_direct_deliveries();
        tokio::spawn(async move {
            while let Some((peer, bytes)) = inbox.recv().await {
                // direct again: no node carried it and none bound it
                inbound(peer, bytes, None);
            }
        });
        Arc::new(NodeDirect {
            node,
            pins,
            reachable: Reachable::default(),
        })
    }
}

impl Direct for NodeDirect {
    fn reachable(&self) -> Reachable {
        self.reachable.clone()
    }

    fn gather(&self, peer: Keyhash) -> Fut<'_, Option<Vec<Candidate>>> {
        Box::pin(async move { self.node.prepare_direct(&peer).await })
    }

    fn open(&self, peer: Keyhash, candidates: Vec<Candidate>) -> Fut<'_, bool> {
        Box::pin(async move {
            let held = self.node.open_direct(peer, &self.pins, &candidates).await;
            self.reachable.set(peer, held);
            held
        })
    }

    fn deliver(&self, peer: Keyhash, bytes: Vec<u8>) -> Fut<'_, bool> {
        Box::pin(async move {
            match self.node.direct_connection(&peer) {
                Some(c) => deliver(&c, bytes).await,
                None => false,
            }
        })
    }
}
