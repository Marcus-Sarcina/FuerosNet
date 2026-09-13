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

/// A session this node holds upstream: where its control frames go, and
/// the connection its request streams open on.
struct UpstreamSession {
    outbound: mpsc::UnboundedSender<(u64, Vec<u8>)>,
    conn: quinn::Connection,
}

/// A handle into each session this node holds upstream, by peer.
type Upstream = Arc<Mutex<HashMap<Keyhash, UpstreamSession>>>;

/// Where the reply to a request this node sent goes: the peer that
/// answered, the request type, and the reply body.
pub type ReplyHandler = Arc<dyn Fn(Keyhash, u64, Vec<u8>) + Send + Sync>;

/// Send one request on `conn` and hand the reply to `hook`, off the caller's
/// thread.  No reply, or no hook, and the request concludes nothing.
fn request_and_reply(conn: quinn::Connection, peer: Keyhash, request_type: u64, body: &[u8], hook: Arc<Mutex<Option<ReplyHandler>>>) {
    let body = body.to_vec();
    tokio::spawn(async move {
        if let Ok(reply) = rhtn_transport::session::request_on(&conn, request_type, &body).await {
            let hook = hook.lock().unwrap().clone();
            if let Some(h) = hook {
                h(peer, request_type, reply);
            }
        }
    });
}

/// The frames a node can send: into an attached session, or up a session
/// it holds as a client; and the requests it can make, on the sessions it
/// holds as a client alone.
#[derive(Clone)]
pub struct LiveAdjacency {
    node: Arc<Mutex<Option<Arc<Node>>>>,
    upstream: Upstream,
    on_reply: Arc<Mutex<Option<ReplyHandler>>>,
}

impl Adjacency for LiveAdjacency {
    fn peers(&self) -> Vec<Keyhash> {
        let mut out: Vec<Keyhash> = self.node.lock().unwrap().as_ref().map(|n| n.sessions()).unwrap_or_default();
        out.extend(self.upstream.lock().unwrap().keys().copied());
        out
    }
    fn send(&self, peer: &Keyhash, frame_type: u64, body: &[u8]) {
        if let Some(u) = self.upstream.lock().unwrap().get(peer) {
            let _ = u.outbound.send((frame_type, body.to_vec()));
            return;
        }
        if let Some(n) = self.node.lock().unwrap().as_ref() {
            n.send_control(peer, frame_type, body);
        }
    }
    /// A request goes up a session this node holds as the client.  A
    /// session it serves carries none: a light client answers no request
    /// stream, so a party attached below cannot be asked [author,
    /// 2026-09-11].
    fn request(&self, peer: &Keyhash, request_type: u64, body: &[u8]) -> bool {
        let Some(conn) = self.upstream.lock().unwrap().get(peer).map(|u| u.conn.clone()) else { return false };
        request_and_reply(conn, *peer, request_type, body, self.on_reply.clone());
        true
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

/// A light client's one session, as an adjacency: control frames on stream
/// 0, requests on fresh bidirectional streams of the same connection, and
/// the replies back through `on_reply`.
pub struct SessionAdjacency {
    pub peer: Keyhash,
    pub outbound: mpsc::UnboundedSender<(u64, Vec<u8>)>,
    pub conn: Option<quinn::Connection>,
    pub on_reply: Arc<Mutex<Option<ReplyHandler>>>,
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
    fn request(&self, peer: &Keyhash, request_type: u64, body: &[u8]) -> bool {
        let Some(conn) = self.conn.clone().filter(|_| *peer == self.peer) else { return false };
        request_and_reply(conn, *peer, request_type, body, self.on_reply.clone());
        true
    }
}

/// The reply path into a view: a currency reply settles the ask it answers,
/// a resolve reply steps the repair it answers.
fn reply_handler(view: Arc<Mutex<NodeView>>, ids: Arc<Mutex<Vec<Identity>>>, adj: Arc<dyn Adjacency + Send + Sync>) -> ReplyHandler {
    Arc::new(move |_peer, request_type, bytes| {
        let mut v = view.lock().unwrap();
        let i = ids.lock().unwrap();
        match request_type {
            crate::resolution::REQUEST_CURRENCY => {
                v.take_currency_reply(&*adj, &*i, &bytes);
            }
            REQUEST_RESOLVE => {
                v.take_resolve_reply(&*adj, &bytes);
            }
            _ => {}
        }
    })
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
/// Payload that arrived on the direct path, with the peer it came from.
pub type DirectInbox = mpsc::UnboundedReceiver<(Keyhash, Vec<u8>)>;

/// A verifier this process hosts: a node that is a participant answering
/// for its own key, or a light client beside its serving node.  Given the
/// authenticated requester and a type-4 body, the signed response for the
/// stream, or nothing, which fails the stream (`wire-format.md` §9.2).
pub type LocalVerifier = Arc<dyn Fn(Keyhash, Vec<u8>) -> std::pin::Pin<Box<dyn std::future::Future<Output = Option<Vec<u8>>> + Send>> + Send + Sync>;

/// The state of the direct path to one peer (design §14.1.1): held, or
/// failed and not retried until the next path opening.
pub enum DirectState {
    Connected(quinn::Connection),
    Failed(std::time::Instant),
}

/// Where live payload went.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LiveDelivery {
    Direct,
    Relayed,
}

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
    /// The socket this node serves and dials on: QUIC, STUN answered for
    /// the clients it serves, and STUN asked of its own serving node.
    pub traversal: Arc<rhtn_transport::traversal::TraversalSocket>,
    /// Where this node's serving node is, once attached: the STUN server
    /// its candidates are gathered against.
    pub upstream_addr: Mutex<Option<SocketAddr>>,
    /// The direct path per peer.
    direct: Mutex<HashMap<Keyhash, DirectState>>,
    /// Payload that arrived on the direct path, for the caller to take.
    direct_deliveries: Mutex<Option<DirectInbox>>,
    /// The verifiers this process hosts, answering request type 4 for the
    /// key a query names; none installed, and such a request fails the
    /// stream.
    pub verifier: Arc<Mutex<Option<LocalVerifier>>>,
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
        let adjacency = LiveAdjacency { node: slot.clone(), upstream: Arc::default(), on_reply: Arc::default() };
        // replies to this node's own requests come back into the view
        *adjacency.on_reply.lock().unwrap() = Some(reply_handler(view.clone(), ids.clone(), Arc::new(adjacency.clone())));

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
        // the outward dials sit behind the same NAT as the served socket on
        // a harness; control traffic needs no traversal (`wire-format.md`
        // §9.2), so this socket never asks STUN of anyone.  It takes an
        // ephemeral port on the same interface the node serves, so a host
        // with several does not dial out of one it was not given.
        let outward = cfg.listen.map(|a| std::net::SocketAddr::new(a.ip(), 0)).unwrap_or_else(|| "127.0.0.1:0".parse().unwrap());
        let client_socket = rhtn_transport::traversal::TraversalSocket::bind(outward, cfg.nat).expect("client socket");
        let client_ep = rhtn_transport::traversal::endpoint(client_socket, None).expect("client endpoint");
        let dial_ep = client_ep.clone();
        let lim = limits.clone();
        let ids_for_requests = ids.clone();
        let verifier: Arc<Mutex<Option<LocalVerifier>>> = Arc::default();
        let hosted_verifier = verifier.clone();
        let on_request: RequestHandler = Arc::new(move |peer, family, body| {
            let (v, c, an, s, identity, dial_ep, lim, ids_for_requests, hosted_verifier) =
                (v.clone(), c.clone(), an.clone(), s.clone(), identity.clone(), dial_ep.clone(), lim.clone(), ids_for_requests.clone(), hosted_verifier.clone());
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
                    Family::PrekeyRequestOrBatch => {
                        let mut view = v.lock().unwrap();
                        let now = view.now();
                        view.prekeys.answer(&peer, &body, now)
                    }
                    Family::ArchiveRequest => {
                        // this node serves its own archive; a subject it is
                        // not gets an empty batch, which says nothing about
                        // that archive (`wire-format.md` §7.9)
                        let req = rhtn_archive::chain::ArchiveRequest::decode(&body).ok()?;
                        let view = v.lock().unwrap();
                        Some(view.archive.serve(&req).encode())
                    }
                    Family::VerifierQuery => {
                        // the query names its verifier (field 7): one hosted
                        // in this process answers.  The leg from a serving
                        // node to a client attached over the wire is not
                        // written (`wire-format.md` §7.7.2), so with no
                        // hosted verifier the stream fails
                        let hosted = hosted_verifier.lock().unwrap().clone();
                        match hosted {
                            Some(answer) => answer(peer, body).await,
                            None => None,
                        }
                    }
                    Family::CatalogQuery => {
                        let view = v.lock().unwrap();
                        let me = view.me();
                        let scopes = crate::catalog::TableScopes { table: &view.table, me };
                        view.catalog.answer(&peer, &body, &scopes)
                    }
                    Family::ResourceRegistration => {
                        let mut view = v.lock().unwrap();
                        let i = ids_for_requests.lock().unwrap();
                        view.catalog.register(&*i, &peer, &body)
                    }
                    Family::ResourceRequest => {
                        // never in early data: the transport defers every
                        // request stream past the handshake; one request
                        // per stream: the transport reads one and answers
                        let mut view = v.lock().unwrap();
                        let me = view.me();
                        let table = view.table.clone_for(me);
                        Some(view.resources.serve(&me, &table, &peer, &body).encode())
                    }
                    // the four things a client hands the node serving it
                    // (`wire-format.md` §7.10).  Each is about the peer that
                    // sent it, so the authenticated requester is the subject,
                    // the pool's owner, the submitter and the endpoint's
                    // holder, and none of the four can name another party
                    Family::PrekeyPublication => {
                        let mut view = v.lock().unwrap();
                        let i = ids_for_requests.lock().unwrap();
                        crate::submissions::publication(&mut view, &i, &peer, &body)
                    }
                    Family::OneTimeDeposit => {
                        let mut view = v.lock().unwrap();
                        crate::submissions::deposit(&mut view, &peer, &body)
                    }
                    Family::RelaySubmission => {
                        let node = s.lock().unwrap().clone();
                        crate::submissions::relay(node.as_ref(), &peer, &body)
                    }
                    Family::WakeRegistration => {
                        let mut view = v.lock().unwrap();
                        crate::submissions::wake(&mut view, &peer, &body)
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

        // payload delivered on the direct path reaches the caller through
        // a channel; the peer is the connection's authenticated identity
        let (dtx, drx) = mpsc::unbounded_channel();
        cfg.on_direct = Some(Arc::new(move |peer, bytes| {
            let _ = dtx.send((peer, bytes));
        }));
        // and the same decision on the way in: a connection this node
        // would not have dialled is refused before it is read, which is
        // where the address would otherwise be disclosed (design §12.6.3)
        let av = view.clone();
        cfg.accepts_direct = Arc::new(move |peer| permits_direct(&av, peer));
        // one socket for QUIC and STUN (design §14.1.1): the node answers
        // Binding Requests at the address it serves on, which the operator
        // chooses (`infra-client-requirements.md` §7)
        let served = cfg.listen.unwrap_or_else(|| "127.0.0.1:0".parse().unwrap());
        let traversal = rhtn_transport::traversal::TraversalSocket::bind(served, cfg.nat).expect("traversal socket");
        let endpoint = {
            let crypto = quinn::crypto::rustls::QuicServerConfig::try_from(tls::server_config(&cfg.identity)).expect("quinn accepts the profile");
            let mut qcfg = quinn::ServerConfig::with_crypto(Arc::new(crypto));
            qcfg.transport_config(Arc::new(tls::transport_config()));
            rhtn_transport::traversal::endpoint(traversal.clone(), Some(qcfg)).expect("server endpoint")
        };
        let addr = traversal.addr().expect("bound");
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
        Arc::new(LiveNode { node, view, currency, anchors, ids, adjacency, endpoint, addr, client_ep, dial_timeout: Duration::from_secs(3), limits, traversal, upstream_addr: Mutex::new(None), direct: Mutex::new(HashMap::new()), direct_deliveries: Mutex::new(Some(drx)), verifier })
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
                *self.upstream_addr.lock().unwrap() = Some(session.conn.remote_address());
                self.adjacency.upstream.lock().unwrap().insert(serving, UpstreamSession { outbound: session.outbound.clone(), conn: session.conn.clone() });
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

    /// Gather this node's candidates for the direct path (design §14.1.1):
    /// its host address, and the reflexive address its serving node
    /// reports, asked of the STUN server at the address it attached to.
    pub async fn gather(&self) -> Vec<rhtn_transport::traversal::Candidate> {
        let stun = *self.upstream_addr.lock().unwrap();
        self.traversal.gather(stun, Duration::from_secs(2)).await
    }

    /// Prepare the direct path to `peer`: candidates gathered only where
    /// design §12.6.3's decision says the path may be direct, which is
    /// inside the horizon absent an override; outside it nothing is
    /// gathered, nothing exchanged and nothing dialled.
    pub async fn prepare_direct(&self, peer: &Keyhash) -> Option<Vec<rhtn_transport::traversal::Candidate>> {
        let path = self.view.lock().unwrap().payload_path(peer, crate::peering::PathOverride::None);
        match path {
            crate::peering::PayloadPath::Direct => Some(self.gather().await),
            crate::peering::PayloadPath::Relayed => None,
        }
    }

    /// Open the direct path to `peer` on its candidates: every candidate
    /// dialled at once, the first handshake under the pinned key kept.
    /// Whether it opened; a failure is remembered and not retried until
    /// the next opening.
    pub async fn open_direct(&self, peer: Keyhash, pins: &rhtn_transport::tls::Pins, candidates: &[rhtn_transport::traversal::Candidate]) -> bool {
        // The decision is this node's own (design §12.6.3).  Candidates
        // arrive from the peer, and a peer willing to connect is not
        // permission to connect to it, so the same question `prepare_direct`
        // asks before gathering is asked again here, which is the entry
        // every caller reaches.
        if !permits_direct(&self.view, &peer) {
            return false;
        }
        let me = self.node.cfg.identity.clone();
        let conn = rhtn_transport::traversal::connect_direct(&self.endpoint, &me, pins, &peer, candidates, self.dial_timeout).await;
        let mut d = self.direct.lock().unwrap();
        match conn {
            Some(c) => {
                d.insert(peer, DirectState::Connected(c));
                true
            }
            None => {
                d.insert(peer, DirectState::Failed(std::time::Instant::now()));
                false
            }
        }
    }

    /// Whether a direct path with `peer` is one this node permits: inside
    /// the horizon absent an override (design §12.6.3).
    pub fn permits_direct(&self, peer: &Keyhash) -> bool {
        permits_direct(&self.view, peer)
    }

    /// The direct path's state toward `peer`: `Some(true)` held,
    /// `Some(false)` failed, `None` never opened.
    pub fn direct_state(&self, peer: &Keyhash) -> Option<bool> {
        match self.direct.lock().unwrap().get(peer) {
            Some(DirectState::Connected(_)) => Some(true),
            Some(DirectState::Failed(_)) => Some(false),
            None => None,
        }
    }

    /// The connection the direct path to `peer` holds, where one is held.
    pub fn direct_connection(&self, peer: &Keyhash) -> Option<quinn::Connection> {
        match self.direct.lock().unwrap().get(peer) {
            Some(DirectState::Connected(c)) => Some(c.clone()),
            _ => None,
        }
    }

    /// Payload arriving on the direct path, once: the receiver.
    pub fn take_direct_deliveries(&self) -> DirectInbox {
        self.direct_deliveries.lock().unwrap().take().expect("taken once")
    }

    /// Send live payload to `peer`: on the direct path where it is held,
    /// and otherwise through `relay`, which hands the bytes to the serving
    /// node.  A direct path that failed is not retried here; a send never
    /// waits on one.
    pub async fn send_payload_live(&self, peer: Keyhash, bytes: Vec<u8>, relay: &dyn Fn(Vec<u8>)) -> LiveDelivery {
        let conn = match self.direct.lock().unwrap().get(&peer) {
            Some(DirectState::Connected(c)) => Some(c.clone()),
            _ => None,
        };
        if let Some(c) = conn
            && rhtn_transport::session::deliver(&c, bytes.clone()).await {
                return LiveDelivery::Direct;
            }
        relay(bytes);
        LiveDelivery::Relayed
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
    let on_reply: Arc<Mutex<Option<ReplyHandler>>> = Arc::default();
    let adj = SessionAdjacency { peer: serving, outbound: session.outbound.clone(), conn: Some(session.conn.clone()), on_reply: on_reply.clone() };
    let pump_adj = SessionAdjacency { peer: serving, outbound: session.outbound.clone(), conn: Some(session.conn.clone()), on_reply: on_reply.clone() };
    let reply_adj = SessionAdjacency { peer: serving, outbound: session.outbound.clone(), conn: Some(session.conn.clone()), on_reply: on_reply.clone() };
    *on_reply.lock().unwrap() = Some(reply_handler(view.clone(), ids.clone(), Arc::new(reply_adj)));
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

/// Whether a direct payload path with `peer` is one this node permits:
/// inside the horizon absent an override (design §12.6.3).
///
/// **Asked at the gather, at the dial and at the accept.** The decision is
/// this node's own and the same in every direction: candidates arrive from
/// the peer, and a peer willing to connect is not permission to connect to
/// it, nor to accept what it opens.
fn permits_direct(view: &Arc<Mutex<NodeView>>, peer: &Keyhash) -> bool {
    let view = view.lock().unwrap();
    view.payload_path(peer, crate::peering::PathOverride::None) == crate::peering::PayloadPath::Direct
}
