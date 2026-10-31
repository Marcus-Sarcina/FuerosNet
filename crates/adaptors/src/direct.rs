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
    /// Record whether a direct path to `peer` is held.
    pub fn set(&self, peer: Keyhash, held: bool) {
        let mut s = self.0.lock().unwrap();
        if held {
            s.insert(peer);
        } else {
            s.remove(&peer);
        }
    }

    /// Forget every path: what a socket that is gone held.
    pub fn clear(&self) {
        self.0.lock().unwrap().clear();
    }

    /// Whether a direct path to `peer` is held.
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

/// **What the direct path asks of the client for its control streams**
/// (`wire-format.md` §9.1): this side's own introduction to present on
/// a dial, the taking of one a dialler presented — a cold call, admitted
/// by the client's policy — and the answer to a peer's prekey request
/// over a held path, so a session opens with no node asked.
#[derive(Clone)]
pub struct Control {
    /// This side's introduction (`wire-format.md` §2.3), or none where it
    /// has no locator to introduce itself by yet.
    pub introduce: Introduce,
    /// An introduction a dialler presented: whether the dialler is a
    /// party this client now holds.
    pub introduced: Introduced,
    /// A prekey request (`wire-format.md` §7.8) from a peer on the path:
    /// the reply, or none where this side will not answer it.
    pub answer: Answer,
}

/// See [`Control::introduce`].
pub type Introduce = Arc<dyn Fn() -> Fut<'static, Option<Vec<u8>>> + Send + Sync>;
/// See [`Control::introduced`].
pub type Introduced = Arc<dyn Fn(Keyhash, Vec<u8>) -> Fut<'static, bool> + Send + Sync>;
/// See [`Control::answer`].
pub type Answer = Arc<dyn Fn(Keyhash, Vec<u8>) -> Fut<'static, Option<Vec<u8>>> + Send + Sync>;

impl Control {
    /// Introduces nobody, takes no introduction, answers nothing: the
    /// direct path as it was before control streams.
    pub fn none() -> Control {
        Control {
            introduce: Arc::new(|| Box::pin(async { None })),
            introduced: Arc::new(|_, _| Box::pin(async { false })),
            answer: Arc::new(|_, _| Box::pin(async { None })),
        }
    }
}

/// The first byte of a control stream: the dialler's introduction, or a
/// prekey request answered on the same stream (`wire-format.md` §9.1).
pub const CONTROL_INTRODUCTION: u8 = 2;
/// See [`CONTROL_INTRODUCTION`].
pub const CONTROL_PREKEY: u8 = 3;
/// How long an acceptor waits for the introduction of a dialler it
/// cannot bind otherwise, and a requester for a reply.
const CONTROL_WAIT: Duration = Duration::from_secs(3);
/// The most a control stream carries either way.
const CONTROL_BYTES: usize = 1 << 20;

/// The transport side of the direct path.
pub trait Direct: Send + Sync {
    /// Where this side can be reached, as the transport knows it.
    fn reachable(&self) -> Reachable;
    /// This side's candidates for `peer`, or none where the path may not
    /// be direct: then nothing is gathered, exchanged or dialled.
    fn gather(&self, peer: Keyhash) -> Fut<'_, Option<Vec<Candidate>>>;
    /// Dial the peer's candidates and hold what opens.
    fn open(&self, peer: Keyhash, candidates: Vec<Candidate>) -> Fut<'_, bool>;
    /// Deliver on the path held; false leaves the message with the caller,
    /// and the relay is next.
    fn deliver(&self, peer: Keyhash, bytes: Vec<u8>) -> Fut<'_, bool>;
    /// Ask `peer`, on the path held, for its prekey material
    /// (`wire-format.md` §7.8) and hand the reply back; none where no
    /// path is held or the peer did not answer.
    fn request(&self, _peer: Keyhash, _bytes: Vec<u8>) -> Fut<'_, Option<Vec<u8>>> {
        Box::pin(async { None })
    }
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
    /// Where a reflexive candidate is asked for: the serving node's
    /// socket once one is attached, nothing before.  Set late, because the
    /// socket outlives any one attach.
    stun: Mutex<Option<SocketAddr>>,
    gate: Gate,
    inbound: Inbound,
    conns: Mutex<HashMap<Keyhash, quinn::Connection>>,
    reachable: Reachable,
    dial_timeout: Duration,
    /// Where the checks' events go: nowhere by default.
    log: rhtn_transport::session::Log,
    /// What the client does for the control streams.
    control: Control,
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
        control: Control,
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
            stun: Mutex::new(stun),
            gate,
            inbound,
            conns: Mutex::new(HashMap::new()),
            reachable,
            dial_timeout: Duration::from_secs(3),
            log: rhtn_transport::session::Log::default(),
            control,
        }));
        // what a peer opens toward this socket
        let accepting = light.clone();
        tokio::spawn(async move {
            while let Some(incoming) = endpoint.accept().await {
                let l = accepting.clone();
                tokio::spawn(async move {
                    if let Ok(conn) = incoming.await {
                        l.accept(conn).await;
                    }
                });
            }
        });
        Ok(light)
    }

    /// The address this socket is bound to.
    pub fn addr(&self) -> io::Result<SocketAddr> {
        self.0.socket.addr()
    }

    /// Where to ask for a reflexive candidate from now on: the serving
    /// node's socket on attach (design §14.1.1), nothing on detach.
    pub fn set_stun(&self, stun: Option<SocketAddr>) {
        *self.0.stun.lock().unwrap() = stun;
    }

    /// Close the socket: every connection it holds ends and the accept
    /// task behind it returns.  For a device that moved to another
    /// interface, whose socket is bound where nothing reaches any more.
    pub fn close(&self) {
        self.0.conns.lock().unwrap().clear();
        self.0.endpoint.close(quinn::VarInt::from_u32(0), b"moved");
    }

    /// Whom the key `conn` presented speaks as, by what this side holds
    /// with no frame (`wire-format.md` §9.1): the pin, or a delegation
    /// held in the horizon view.
    fn bound_key(&self, conn: &quinn::Connection) -> Option<Keyhash> {
        tls::peer_key(conn).and_then(|k| {
            self.0.pins.keyhash_for_key(&k).or_else(|| {
                self.0
                    .bind
                    .held
                    .by_key(&k)
                    .filter(|d| self.0.bind.in_window(d))
                    .map(|d| d.keyhash)
            })
        })
    }

    /// **A connection a peer opened toward this socket.** Bound by what
    /// this side holds, it is held at once; bound by nothing, this side
    /// waits for the dialler's introduction on its first control stream
    /// (`wire-format.md` §9.1, §2.3) and takes it as a cold call where
    /// the client's policy admits one — which is how a peer reaches a
    /// client restored from its phrase, that knows nobody, with no node
    /// between them [author, 2026-10-30].  Refused where nothing came,
    /// or what came did not bind the key presented.
    async fn accept(&self, conn: quinn::Connection) -> bool {
        if let Some(peer) = self.bound_key(&conn) {
            return self.hold_as(peer, conn);
        }
        let Some(presented) = tls::peer_key(&conn) else {
            conn.close(quinn::VarInt::from_u32(CLOSE_REFUSED), b"unbound");
            return false;
        };
        let intro = tokio::time::timeout(CONTROL_WAIT, async {
            let (send, mut recv) = conn.accept_bi().await.ok()?;
            let bytes = recv.read_to_end(CONTROL_BYTES).await.ok()?;
            Some((send, bytes))
        })
        .await
        .ok()
        .flatten();
        let Some((mut send, bytes)) = intro else {
            conn.close(quinn::VarInt::from_u32(CLOSE_REFUSED), b"unbound");
            return false;
        };
        let taken = match bytes.split_first() {
            Some((&CONTROL_INTRODUCTION, body)) => self.take_introduction(presented, body).await,
            _ => None,
        };
        let Some(peer) = taken else {
            let _ = send.write_all(&[0]).await;
            let _ = send.finish();
            conn.close(quinn::VarInt::from_u32(CLOSE_REFUSED), b"unbound");
            return false;
        };
        let _ = send.write_all(&[1]).await;
        let _ = send.finish();
        self.hold_as(peer, conn)
    }

    /// An introduction presented first on a connection: the party it
    /// names, where the material's classical member is the key presented
    /// and the client took the party as a cold call; pinned then, so the
    /// key binds from here on.
    async fn take_introduction(&self, presented: [u8; 32], body: &[u8]) -> Option<Keyhash> {
        let intro = rhtn_archive::locator::Introduction::parse(body).ok()?;
        if intro.identity.ed.as_bytes() != &presented {
            tracing::info!(target: "pay", "pay.direct.introduction.not_theirs");
            return None;
        }
        let who = intro.identity.keyhash;
        if !(self.0.control.introduced)(who, body.to_vec()).await {
            tracing::info!(target: "pay", peer = %hex4(&who), "pay.direct.introduction.refused");
            return None;
        }
        self.0.pins.pin_identity(&intro.identity);
        tracing::info!(target: "pay", peer = %hex4(&who), "pay.direct.cold_call");
        Some(who)
    }

    /// **This side's introduction, presented first on a dial** where it
    /// has one: the peer may hold nothing that binds this key.  Whether
    /// the peer took it — or needed nothing, which it says the same way.
    async fn present(&self, conn: &quinn::Connection) -> bool {
        let Some(intro) = (self.0.control.introduce)().await else {
            return true;
        };
        let mut frame = Vec::with_capacity(intro.len() + 1);
        frame.push(CONTROL_INTRODUCTION);
        frame.extend_from_slice(&intro);
        tokio::time::timeout(CONTROL_WAIT, async {
            let (mut send, mut recv) = conn.open_bi().await.ok()?;
            send.write_all(&frame).await.ok()?;
            send.finish().ok()?;
            let ack = recv.read_to_end(8).await.ok()?;
            Some(ack.first() == Some(&1))
        })
        .await
        .ok()
        .flatten()
        .unwrap_or(false)
    }

    /// Hold a connection this side dialled: bound by what it holds, since
    /// it dialled a party it knew.
    fn hold(&self, conn: quinn::Connection) -> bool {
        match self.bound_key(&conn) {
            Some(peer) => self.hold_as(peer, conn),
            None => {
                conn.close(quinn::VarInt::from_u32(CLOSE_REFUSED), b"unbound");
                false
            }
        }
    }

    /// Hold `conn` as `peer`'s and read what it delivers until it closes;
    /// answer its control streams meanwhile.
    fn hold_as(&self, peer: Keyhash, conn: quinn::Connection) -> bool {
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
        // the control streams: an introduction (taken, or needed by
        // nothing), and a prekey request answered on the same stream
        let ctl = self.clone();
        let serving = conn.clone();
        tokio::spawn(async move {
            while let Ok((mut send, mut recv)) = serving.accept_bi().await {
                let Ok(bytes) = recv.read_to_end(CONTROL_BYTES).await else {
                    continue;
                };
                match bytes.split_first() {
                    Some((&CONTROL_INTRODUCTION, body)) => {
                        let _ = (ctl.0.control.introduced)(peer, body.to_vec()).await;
                        let _ = send.write_all(&[1]).await;
                    }
                    Some((&CONTROL_PREKEY, body)) => {
                        if let Some(reply) = (ctl.0.control.answer)(peer, body.to_vec()).await {
                            tracing::info!(target: "pay", peer = %hex4(&peer), bytes = reply.len(), "pay.direct.prekey.answered");
                            let _ = send.write_all(&reply).await;
                        }
                    }
                    _ => {}
                }
                let _ = send.finish();
            }
        });
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
            let stun = *self.0.stun.lock().unwrap();
            let cands = self.0.socket.gather(stun, Duration::from_secs(2)).await;
            // a candidate exchange carries one to eight candidates
            // (`wire-format.md` §14.3.2): an empty gather is no offer,
            // not an offer of nothing
            if cands.is_empty() { None } else { Some(cands) }
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
                Some(conn) => {
                    // the peer may hold nothing that binds this key: this
                    // side's introduction goes first, and the peer says
                    // whether it took it or needed nothing
                    if !self.present(&conn).await {
                        conn.close(quinn::VarInt::from_u32(CLOSE_REFUSED), b"not introduced");
                        return false;
                    }
                    self.hold(conn)
                }
                None => false,
            }
        })
    }

    fn request(&self, peer: Keyhash, bytes: Vec<u8>) -> Fut<'_, Option<Vec<u8>>> {
        Box::pin(async move {
            let conn = self.0.conns.lock().unwrap().get(&peer).cloned()?;
            let mut frame = Vec::with_capacity(bytes.len() + 1);
            frame.push(CONTROL_PREKEY);
            frame.extend_from_slice(&bytes);
            let reply = tokio::time::timeout(CONTROL_WAIT, async {
                let (mut send, mut recv) = conn.open_bi().await.ok()?;
                send.write_all(&frame).await.ok()?;
                send.finish().ok()?;
                recv.read_to_end(CONTROL_BYTES).await.ok()
            })
            .await
            .ok()
            .flatten()
            .filter(|r| !r.is_empty());
            tracing::info!(target: "pay", peer = %hex4(&peer), answered = reply.is_some(), "pay.direct.prekey.asked");
            reply
        })
    }

    fn deliver(&self, peer: Keyhash, bytes: Vec<u8>) -> Fut<'_, bool> {
        Box::pin(async move {
            let conn = self.0.conns.lock().unwrap().get(&peer).cloned();
            let Some(c) = conn else {
                return false;
            };
            // **a delivery is bounded by the dial's own timeout, plus a
            // second for every 32 KiB.** The direct path has no heartbeat:
            // a peer that went away leaves a connection nothing closes
            // until QUIC's idle timer, and a stream it will never
            // acknowledge.  A delivery that does not complete in that time
            // is a path no longer held, and the message goes to the relay
            // (design §14.1.1).  The size term is the bench's lesson [run
            // 0c8c8f8-a3-node-1, 2026-10-24]: a 424 KiB backup blob met
            // packet loss in its first hundred milliseconds, QUIC's
            // recovery ran past the dial's three seconds, and the path was
            // judged lost with the blob a third of the way across — while
            // every message under a kilobyte went through.  A client's own
            // number, no part of the wire
            let n = bytes.len();
            let bound = self.0.dial_timeout + Duration::from_secs((n / (32 * 1024)) as u64);
            let started = std::time::Instant::now();
            let ok = tokio::time::timeout(bound, deliver(&c, bytes))
                .await
                .unwrap_or(false);
            tracing::info!(
                target: "pay",
                peer = %hex4(&peer),
                bytes = n,
                ok,
                took_ms = started.elapsed().as_millis() as u64,
                bound_ms = bound.as_millis() as u64,
                "pay.direct.deliver"
            );
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

fn hex4(k: &Keyhash) -> String {
    k[..4].iter().map(|b| format!("{b:02x}")).collect()
}
