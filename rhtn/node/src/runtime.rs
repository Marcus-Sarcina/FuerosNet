//! The node's decisions bound to live sessions.  A [`LiveNode`] owns a
//! transport node, a [`NodeView`], and the currency and anchor state, and
//! installs the hooks that carry stream-0 topology frames and request
//! streams into the decision layer and the decision layer's frames back
//! out on the sessions the node holds.
//!
//! Adjacency here is what `wire-format.md` §10.1.1 says it is: the sessions
//! the node has anyway — the ones attached to it, and the ones it holds
//! upstream to its own patron or serving node.

use crate::currency::{CurrencyRequest, CurrencyState};
use crate::resolution::{AnchorTable, ClientResolution, NetworkPoint, REQUEST_RESOLVE, ResolveReply, ResolveRequest, Resolution, Step, endpoint_record};
use crate::store::{KIND_ENDPOINT_RECORD, KIND_TRANSACTION};
use crate::view::NodeView;
use crate::{Adjacency, Keyhash};
use rhtn_codec::schema::Family;
use rhtn_crypto::Identity;
use rhtn_transport::session::{AttachOutcome, ClientConfig, ControlHandler, Node, NodeConfig, RequestHandler, Session, fresh_attach};
use rhtn_transport::tls;
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::sync::mpsc;

/// The frames a node can send: into an attached session, or up a session
/// it holds as a client.
#[derive(Clone)]
pub struct LiveAdjacency {
    node: Arc<Mutex<Option<Arc<Node>>>>,
    upstream: Arc<Mutex<HashMap<Keyhash, mpsc::UnboundedSender<(u64, Vec<u8>)>>>>,
}

impl Adjacency for LiveAdjacency {
    fn peers(&self) -> Vec<Keyhash> {
        let mut out: Vec<Keyhash> = self.node.lock().unwrap().as_ref().map(|n| n.sessions()).unwrap_or_default();
        out.extend(self.upstream.lock().unwrap().keys().copied());
        out
    }
    fn send(&self, peer: &Keyhash, frame_type: u64, body: &[u8]) {
        if let Some(tx) = self.upstream.lock().unwrap().get(peer) {
            let _ = tx.send((frame_type, body.to_vec()));
            return;
        }
        if let Some(n) = self.node.lock().unwrap().as_ref() {
            n.send_control(peer, frame_type, body);
        }
    }
}

/// A light client's one session, as an adjacency.
pub struct SessionAdjacency {
    pub peer: Keyhash,
    pub outbound: mpsc::UnboundedSender<(u64, Vec<u8>)>,
}

impl Adjacency for SessionAdjacency {
    fn peers(&self) -> Vec<Keyhash> {
        vec![self.peer]
    }
    fn send(&self, peer: &Keyhash, frame_type: u64, body: &[u8]) {
        if *peer == self.peer {
            let _ = self.outbound.send((frame_type, body.to_vec()));
        }
    }
}

/// A running node: transport, view, and the state the request handlers
/// read.
pub struct LiveNode {
    pub node: Arc<Node>,
    pub view: Arc<Mutex<NodeView>>,
    pub currency: Arc<Mutex<CurrencyState>>,
    pub anchors: Arc<Mutex<AnchorTable>>,
    pub ids: Arc<Mutex<Vec<Identity>>>,
    pub adjacency: LiveAdjacency,
    pub endpoint: quinn::Endpoint,
    pub addr: SocketAddr,
    /// The endpoint this node dials out on: upstream attaches, and the hops
    /// of a resolution it runs for a client.
    pub client_ep: quinn::Endpoint,
    /// How long one dial may take on a proxied resolution.
    pub dial_timeout: Duration,
}

impl LiveNode {
    /// Start a node on loopback.  The hooks are installed on `cfg` before
    /// the transport node is built, so the first session already carries.
    pub fn start(mut cfg: NodeConfig, view: NodeView, ids: Vec<Identity>, anchors: AnchorTable) -> Arc<LiveNode> {
        let view = Arc::new(Mutex::new(view));
        let currency = Arc::new(Mutex::new(CurrencyState::default()));
        let anchors = Arc::new(Mutex::new(anchors));
        let ids = Arc::new(Mutex::new(ids));
        let slot: Arc<Mutex<Option<Arc<Node>>>> = Arc::default();
        let adjacency = LiveAdjacency { node: slot.clone(), upstream: Arc::default() };

        // stream 0: topology frames into the forwarding rule and the memo
        let (v, i, a) = (view.clone(), ids.clone(), adjacency.clone());
        let on_control: ControlHandler = Arc::new(move |peer, ft, body| {
            let mut view = v.lock().unwrap();
            let ids = i.lock().unwrap();
            match ft {
                5 => {
                    view.receive_push(&a, &peer, &body, &*ids);
                }
                6 => {
                    view.receive_memo(&a, &peer, &body);
                }
                _ => {}
            }
        });
        cfg.on_control = Some(on_control);

        // request streams: resolution and currency, answered from the view
        let (v, c, an, s) = (view.clone(), currency.clone(), anchors.clone(), slot.clone());
        let identity = cfg.identity.clone();
        let client_ep = tls::client_endpoint("127.0.0.1:0".parse().unwrap()).expect("client endpoint");
        let dial_ep = client_ep.clone();
        let on_request: RequestHandler = Arc::new(move |_peer, family, body| {
            let (v, c, an, s, identity, dial_ep) = (v.clone(), c.clone(), an.clone(), s.clone(), identity.clone(), dial_ep.clone());
            Box::pin(async move {
                match family {
                    Family::CurrencyRequest => {
                        let req = CurrencyRequest::decode(&body).ok()?;
                        let reply = v.lock().unwrap().answer_currency(&c.lock().unwrap(), &req);
                        Some(reply.encode())
                    }
                    Family::ResolveRequest => {
                        let req = ResolveRequest::decode(&body).ok()?;
                        let started = v.lock().unwrap().resolve_for_client(&an.lock().unwrap(), &req);
                        match started {
                            Ok(ClientResolution::Answered(reply)) => Some(reply.encode()),
                            Ok(ClientResolution::Proxied(r)) => {
                                let node = s.lock().unwrap().clone()?;
                                let (r, last) = drive(r, &dial_ep, &identity, &node, Duration::from_secs(3)).await;
                                let reply = v.lock().unwrap().reply_for_client(&r, last.as_ref());
                                Some(reply.encode())
                            }
                            // no anchor to start from: the client is told this
                            // node is not authoritative and cannot refer
                            Err(_) => Some(ResolveReply::Failure { nonce: req.nonce, code: crate::resolution::FAIL_NOT_AUTHORITATIVE }.encode()),
                        }
                    }
                    _ => None,
                }
            })
        });
        cfg.on_request = Some(on_request);

        let endpoint = tls::server_endpoint(&cfg.identity, "127.0.0.1:0".parse().unwrap()).expect("server endpoint");
        let addr = endpoint.local_addr().expect("bound");
        // an infra node publishes its own endpoint record (`wire-format.md`
        // §7.6): into its own store now, and onto the flood as sessions come
        if let Some(point) = NetworkPoint::from_socket(addr) {
            let mut v = view.lock().unwrap();
            let series = v.position.seqno.series;
            let record = endpoint_record(&cfg.identity, &[point], rhtn_archive::tx::Seqno { series, counter: 1 });
            let me = v.me();
            let i = ids.lock().unwrap();
            v.take_object(&adjacency, &me, KIND_ENDPOINT_RECORD, &record, &*i);
        }
        let node = Node::new(cfg);
        *slot.lock().unwrap() = Some(node.clone());
        tokio::spawn(node.clone().serve(endpoint.clone()));
        Arc::new(LiveNode { node, view, currency, anchors, ids, adjacency, endpoint, addr, client_ep, dial_timeout: Duration::from_secs(3) })
    }

    pub fn me(&self) -> Keyhash {
        self.node.cfg.identity.public.keyhash
    }

    /// Attach upstream, to this node's patron or serving node, and carry
    /// what arrives on that session into the decision layer.  The session
    /// is returned for the caller to hold.
    pub async fn attach_upstream(&self, cfg: &ClientConfig, serving: Keyhash) -> AttachOutcome {
        match fresh_attach(cfg, &self.client_ep, serving, false).await {
            AttachOutcome::Attached(mut session) => {
                self.adjacency.upstream.lock().unwrap().insert(serving, session.outbound.clone());
                let (_, dummy) = mpsc::unbounded_channel();
                let mut frames = std::mem::replace(&mut session.frames, dummy);
                let (v, i, a) = (self.view.clone(), self.ids.clone(), self.adjacency.clone());
                tokio::spawn(async move {
                    while let Some((ft, body)) = frames.recv().await {
                        let mut view = v.lock().unwrap();
                        let ids = i.lock().unwrap();
                        match ft {
                            5 => {
                                view.receive_push(&a, &serving, &body, &*ids);
                            }
                            6 => {
                                view.receive_memo(&a, &serving, &body);
                            }
                            _ => {}
                        }
                    }
                });
                AttachOutcome::Attached(session)
            }
            other => other,
        }
    }

    /// Originate a topology object this node is a party to.
    pub fn originate(&self, kind: u64, object: &[u8]) -> crate::store::Decision {
        let ids = self.ids.lock().unwrap();
        self.view.lock().unwrap().originate_push(&self.adjacency, kind, object, &*ids)
    }

    /// Originate a transaction.
    pub fn originate_transaction(&self, object: &[u8]) -> crate::store::Decision {
        self.originate(KIND_TRANSACTION, object)
    }

    /// What this node replicates to a sibling: its topology store and the
    /// trust-bearing history in it.  The transport's mailbox is not read.
    pub fn replication_payload(&self) -> Vec<crate::peering::Replicated> {
        self.view.lock().unwrap().replication_payload()
    }
}

/// Run a proxied resolution to its end: open a session with each hop at an
/// endpoint from its list, ask on a request stream, and follow referrals
/// until a serving answer or a failure.  A session is what a party opens
/// with any node it needs to request from (design §14.1.2); the hop's
/// answer says where this node stands, and nothing is served on it.
async fn drive(mut r: Resolution, ep: &quinn::Endpoint, me: &Arc<rhtn_crypto::SigningIdentity>, node: &Node, per_endpoint: Duration) -> (Resolution, Option<ResolveReply>) {
    let cfg = ClientConfig {
        identity: me.clone(),
        pins: node.cfg.pins.clone(),
        capabilities: Default::default(),
        attestation: None,
        filter: None,
        sibling_cache: Arc::new(Mutex::new(Vec::new())),
        addresses: Arc::new(Mutex::new(HashMap::new())),
        tls: Arc::new(Mutex::new(HashMap::new())),
        connect_timeout: per_endpoint,
    };
    let mut last: Option<ResolveReply> = None;
    for _ in 0..16 {
        let (hop, endpoints) = r.next_hop();
        let addrs: Vec<SocketAddr> = endpoints.iter().map(|e| e.socket()).collect();
        let AttachOutcome::Attached(session) = rhtn_transport::session::attach_any(&cfg, ep, hop, &addrs, false).await else { break };
        let Ok(bytes) = session.request(REQUEST_RESOLVE, &r.request.encode()).await else { break };
        session.conn.close(0u32.into(), b"resolved");
        let Ok(reply) = ResolveReply::decode(&bytes) else { break };
        let step = r.take(&reply);
        last = Some(reply);
        match step {
            Step::Continue(_) => continue,
            _ => break,
        }
    }
    (r, last)
}

/// Drain the frames a client session delivers into a light client's own
/// view, so the client keeps its horizon (`wire-format.md` §10.1.1: a
/// client's floods enter and leave through the node that serves it).
pub fn pump_client(session: &mut Session, serving: Keyhash, view: Arc<Mutex<NodeView>>, ids: Arc<Mutex<Vec<Identity>>>) -> SessionAdjacency {
    let (_, dummy) = mpsc::unbounded_channel();
    let mut frames = std::mem::replace(&mut session.frames, dummy);
    let adj = SessionAdjacency { peer: serving, outbound: session.outbound.clone() };
    let pump_adj = SessionAdjacency { peer: serving, outbound: session.outbound.clone() };
    tokio::spawn(async move {
        while let Some((ft, body)) = frames.recv().await {
            let mut v = view.lock().unwrap();
            let i = ids.lock().unwrap();
            match ft {
                5 => {
                    v.receive_push(&pump_adj, &serving, &body, &*i);
                }
                6 => {
                    v.receive_memo(&pump_adj, &serving, &body);
                }
                _ => {}
            }
        }
    });
    adj
}
