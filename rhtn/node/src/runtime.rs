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
use crate::resolution::{AnchorTable, ClientResolution, NetworkPoint, REQUEST_RESOLVE, ResolveReply, ResolveRequest, Resolution, Step};
use crate::propagation::decode_push;
use crate::store::{Decision, KIND_ENDPOINT_RECORD, KIND_TRANSACTION};
use crate::view::NodeView;
use crate::{Adjacency, Keyhash};
use rhtn_archive::record::Record;
use rhtn_archive::topology::Supersession;
use rhtn_codec::schema::Family;
use rhtn_crypto::Identity;
use rhtn_transport::session::{AttachOutcome, ClientConfig, ControlHandler, Node, NodeConfig, RequestHandler, Session, fresh_attach};
use rhtn_transport::tls;
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::sync::mpsc;

/// A handle into each session this node holds upstream, by peer.
type Upstream = Arc<Mutex<HashMap<Keyhash, mpsc::UnboundedSender<(u64, Vec<u8>)>>>>;

/// The frames a node can send: into an attached session, or up a session
/// it holds as a client.
#[derive(Clone)]
pub struct LiveAdjacency {
    node: Arc<Mutex<Option<Arc<Node>>>>,
    upstream: Upstream,
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

/// Carry a supersession the view has just stored into the serving state
/// (`infra-client-requirements.md` §2, design §12.6.5): a validated recovery
/// or reissue ends the old credential's sessions and queue at the transport
/// as it does in the table.  Anything but a verified transaction of those
/// two kinds carries nothing.
fn carry_supersession(node: &Arc<Mutex<Option<Arc<Node>>>>, ids: &[Identity], kind: u64, object: &[u8]) {
    if kind != KIND_TRANSACTION {
        return;
    }
    let Ok(rec) = Record::parse(object) else { return };
    let Ok(sup) = Supersession::from_record(&rec, ids) else { return };
    if let Some(n) = node.lock().unwrap().as_ref() {
        n.supersede(sup);
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

/// Requests per requester per window (`wire-format.md` §7.1: rate-limit
/// per requester as with any other query).  Over the limit a request
/// stream fails; the values are the operator's.
pub struct RateLimit {
    pub per_window: u32,
    pub window: Duration,
    buckets: Mutex<HashMap<Keyhash, (tokio::time::Instant, u32)>>,
}

impl RateLimit {
    pub fn new(per_window: u32, window: Duration) -> Self {
        RateLimit { per_window, window, buckets: Mutex::new(HashMap::new()) }
    }

    /// Whether one more request from `peer` is within its allowance now.
    pub fn allow(&self, peer: &Keyhash) -> bool {
        let now = tokio::time::Instant::now();
        let mut b = self.buckets.lock().unwrap();
        let e = b.entry(*peer).or_insert((now, 0));
        if now.duration_since(e.0) >= self.window {
            *e = (now, 0);
        }
        if e.1 >= self.per_window {
            return false;
        }
        e.1 += 1;
        true
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
    pub limits: Arc<RateLimit>,
}

impl LiveNode {
    /// Start a node on loopback.  The hooks are installed on `cfg` before
    /// the transport node is built, so the first session already carries.
    pub fn start(cfg: NodeConfig, view: NodeView, ids: Vec<Identity>, anchors: AnchorTable) -> Arc<LiveNode> {
        Self::start_with(cfg, view, ids, anchors, RateLimit::new(120, Duration::from_secs(60)))
    }

    /// `start`, with the request allowance the operator chose.
    pub fn start_with(mut cfg: NodeConfig, mut view: NodeView, ids: Vec<Identity>, anchors: AnchorTable, limits: RateLimit) -> Arc<LiveNode> {
        let limits = Arc::new(limits);
        // one clock for the node: the configuration's, read by every
        // decision the view takes from here on
        view.clock = cfg.clock.clone();
        let view = Arc::new(Mutex::new(view));
        let currency = Arc::new(Mutex::new(CurrencyState::default()));
        let anchors = Arc::new(Mutex::new(anchors));
        let ids = Arc::new(Mutex::new(ids));
        let slot: Arc<Mutex<Option<Arc<Node>>>> = Arc::default();
        let adjacency = LiveAdjacency { node: slot.clone(), upstream: Arc::default() };

        // stream 0: topology frames into the forwarding rule and the memo
        let (v, i, a, s) = (view.clone(), ids.clone(), adjacency.clone(), slot.clone());
        let on_control: ControlHandler = Arc::new(move |peer, ft, body| {
            let mut view = v.lock().unwrap();
            let ids = i.lock().unwrap();
            match ft {
                5 => {
                    if view.receive_push(&a, &peer, &body, &*ids) == Decision::Stored
                        && let Ok((kind, object)) = decode_push(&body) {
                            carry_supersession(&s, &ids, kind, &object);
                        }
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
        // the detector's verdicts on the sessions this node serves feed the
        // ladder, stamped with the node's own clock (design §12.6.5.1), and
        // then go wherever the operator sent them
        let (ladder, clock, prev) = (currency.clone(), view.clone(), cfg.replicate.clone());
        cfg.replicate = Some(Arc::new(move |kh, r| {
            let now = clock.lock().unwrap().now();
            let mut cur = ladder.lock().unwrap();
            match r {
                rhtn_transport::session::Reachability::Unreachable => cur.dark(kh, now),
                rhtn_transport::session::Reachability::Reachable => cur.back(&kh),
            }
            drop(cur);
            if let Some(f) = &prev {
                f(kh, r);
            }
        }));
        let client_ep = tls::client_endpoint("127.0.0.1:0".parse().unwrap()).expect("client endpoint");
        let dial_ep = client_ep.clone();
        let lim = limits.clone();
        let on_request: RequestHandler = Arc::new(move |peer, family, body| {
            let (v, c, an, s, identity, dial_ep, lim) = (v.clone(), c.clone(), an.clone(), s.clone(), identity.clone(), dial_ep.clone(), lim.clone());
            Box::pin(async move {
                // over the requester's allowance the stream fails, and nothing
                // about the request is kept
                if !lim.allow(&peer) {
                    return None;
                }
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
        // an infra node publishes its own endpoint record per relationship
        // line (`wire-format.md` §7.6, `infra-client-requirements.md` §4.4):
        // the record it holds where the address is unchanged, the next
        // counter where it moved; into its own store now, and onto the
        // flood as sessions come
        if let Some(point) = NetworkPoint::from_socket(addr) {
            let mut v = view.lock().unwrap();
            let me = v.me();
            let i = ids.lock().unwrap();
            for anchor in v.anchors() {
                if let Some(record) = v.publish_own_endpoints(&anchor, std::slice::from_ref(&point)) {
                    v.take_object(&adjacency, &me, KIND_ENDPOINT_RECORD, &record, &*i);
                }
            }
        }
        let node = Node::new(cfg);
        *slot.lock().unwrap() = Some(node.clone());
        // what the store already holds about supersession reaches the
        // serving state before the first session: a restarted node serves
        // nothing under a credential it knew superseded
        {
            let v = view.lock().unwrap();
            let i = ids.lock().unwrap();
            for (kind, object) in v.store.objects() {
                carry_supersession(&slot, &i, kind, &object);
            }
        }
        tokio::spawn(node.clone().serve(endpoint.clone()));
        Arc::new(LiveNode { node, view, currency, anchors, ids, adjacency, endpoint, addr, client_ep, dial_timeout: Duration::from_secs(3), limits })
    }

    pub fn me(&self) -> Keyhash {
        self.node.cfg.identity.public.keyhash
    }

    /// Attach upstream, to this node's patron or serving node, and carry
    /// what arrives on that session into the decision layer.  The session
    /// is returned for the caller to hold.
    pub async fn attach_upstream(&self, cfg: &ClientConfig, serving: Keyhash) -> AttachOutcome {
        // what the detector settles about the party upstream feeds the
        // ladder too: a patron gone dark is what opens the next rung
        let mut cfg = cfg.clone();
        let (c, v) = (self.currency.clone(), self.view.clone());
        cfg.on_reachability = Some(Arc::new(move |r| {
            let now = v.lock().unwrap().now();
            let mut cur = c.lock().unwrap();
            match r {
                rhtn_transport::session::Reachability::Unreachable => cur.dark(serving, now),
                rhtn_transport::session::Reachability::Reachable => cur.back(&serving),
            }
        }));
        match fresh_attach(&cfg, &self.client_ep, serving, false).await {
            AttachOutcome::Attached(mut session) => {
                self.adjacency.upstream.lock().unwrap().insert(serving, session.outbound.clone());
                let (_, dummy) = mpsc::unbounded_channel();
                let mut frames = std::mem::replace(&mut session.frames, dummy);
                let (v, i, a, s) = (self.view.clone(), self.ids.clone(), self.adjacency.clone(), self.adjacency.node.clone());
                tokio::spawn(async move {
                    while let Some((ft, body)) = frames.recv().await {
                        let mut view = v.lock().unwrap();
                        let ids = i.lock().unwrap();
                        match ft {
                            5 => {
                                if view.receive_push(&a, &serving, &body, &*ids) == Decision::Stored
                                    && let Ok((kind, object)) = decode_push(&body) {
                                        carry_supersession(&s, &ids, kind, &object);
                                    }
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

    /// Originate a topology object this node is a party to.  A recovery or
    /// reissue it stores ends the old credential's service at once.
    pub fn originate(&self, kind: u64, object: &[u8]) -> Decision {
        let ids = self.ids.lock().unwrap();
        let decision = self.view.lock().unwrap().originate_push(&self.adjacency, kind, object, &*ids);
        if decision == Decision::Stored {
            carry_supersession(&self.adjacency.node, &ids, kind, object);
        }
        decision
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
        on_reachability: None,
        log: rhtn_transport::session::Log::default(),
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
