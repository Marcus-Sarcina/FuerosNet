//! What a client asks of its serving node, answered by the node beside it.

use rhtn_archive::Keyhash;
use rhtn_archive::submission::WakeEndpoint;
use rhtn_node::runtime::LiveNode;
use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, Mutex};

/// Where payload for a client goes: who it came from and the bytes. On
/// the direct path that is the peer and `[kind, bytes]`; from a node it
/// is the node and a `RelayedPayload` (`wire-format.md` §7.10), whose
/// sender is inside the seal.
pub type Inbound = Arc<dyn Fn(Keyhash, Vec<u8>) + Send + Sync>;

/// The clients hosted in this process, by keyhash, each with where its
/// payload goes and how far it has collected: what a node holds for a
/// client beside it in place of a session.
#[derive(Clone, Default)]
pub struct Inboxes(Arc<Mutex<HashMap<Keyhash, (Inbound, u64)>>>);

impl Inboxes {
    /// Deliver `client`'s payload to `inbound` from now on.
    pub fn host(&self, client: Keyhash, inbound: Inbound) {
        self.0.lock().unwrap().insert(client, (inbound, 0));
    }

    /// Whether this process hosts `client`.
    pub fn hosts(&self, client: &Keyhash) -> bool {
        self.0.lock().unwrap().contains_key(client)
    }

    /// **Hand a hosted client what `node` holds for it** past what it has
    /// collected, as a session's drain does (`wire-format.md` §7.10): each
    /// a `RelayedPayload` with the node's half, its first delivery marked.
    /// How many went; none where `to` is not hosted here.
    pub fn deliver_queued(&self, node: &LiveNode, to: &Keyhash) -> usize {
        let Some((inbound, cursor)) = self.0.lock().unwrap().get(to).cloned() else {
            return 0;
        };
        let n = &node.node;
        let mut last = cursor;
        let mut went = 0;
        for q in n
            .queue_records(to)
            .into_iter()
            .filter(|q| q.position > cursor)
        {
            let half = match &n.cfg.open_half {
                Some(open) => open(to, &q),
                None => Some(zeroize::Zeroizing::new([0u8; 32])),
            };
            last = q.position;
            let Some(half) = half else { continue };
            inbound(
                node.me(),
                rhtn_archive::submission::relayed(q.position, &q.sealed, &half),
            );
            n.cfg.queue.mark_delivered(to, q.position, (n.cfg.clock)());
            went += 1;
        }
        if let Some(h) = self.0.lock().unwrap().get_mut(to) {
            h.1 = h.1.max(last);
        }
        went
    }
}

/// An answer that may have to cross the wire before it exists.
pub type Answer<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

/// What a client asks of a node (design §14.1.2, §14.2.4): its profile
/// published, its horizon asserted, a resource or the catalog asked, where
/// to be rung — and, **as nobody** (`wire-format.md` §9.1), a recipient's
/// node key fetched and a sealed message handed over. A node beside the
/// client and a node across a session answer the same questions.
///
/// The four that change something are asynchronous because one of the two
/// implementations has to wait for an answer.  The two predicates are not:
/// they say what this side knows without asking, and a node across a
/// session knows neither.
pub trait Serving: Send + Sync {
    /// The serving node's own identity.
    fn me(&self) -> Keyhash;
    /// Whether this node is known here to host or serve `client`.
    fn serves(&self, client: &Keyhash) -> bool;
    /// **A request put to this node as nobody** (`wire-format.md` §9.1): a
    /// node-key fetch or a sealed message, on a connection under a key
    /// minted for it and never on the session held, which names the
    /// client. The node's reply; none where this side cannot put it — a
    /// node across a session is reached through the dialler instead.
    fn minted<'a>(&'a self, _request: u64, _body: Vec<u8>) -> Answer<'a, Option<Vec<u8>>> {
        Box::pin(async { None })
    }
    /// **Teach this node what the client holds** (`wire-format.md`
    /// §10.1.1): the push frames of [`rhtn_client::ceremony::Client::replay_frames`],
    /// sent on the session's stream 0 as the node sends its own the other
    /// way. How many went; none where there is no node.
    ///
    /// **It is an attach's second half** [author, 2026-11-01]: a node
    /// enrolled into a subnet that already exists holds none of it until
    /// its members teach it, and a role row cannot be materialised for a
    /// member the node has never heard of — so the topology would not
    /// survive a change of hosting, which it must.
    fn teach<'a>(&'a self, frames: Vec<Vec<u8>>) -> Answer<'a, usize>;
    /// Publish the caller's own profile (`wire-format.md` §7.11); whether
    /// the node took it.
    fn publish_profile<'a>(&'a self, bytes: &'a [u8]) -> Answer<'a, bool>;
    /// **One row of this client's horizon** (`wire-format.md` §7.12): the
    /// bytes already built, with the nonce the reply must echo, because
    /// the client composed the row and the node bounds it by where this
    /// client sits. Whether the node took it.
    fn assert_horizon<'a>(&'a self, bytes: &'a [u8], nonce: [u8; 16]) -> Answer<'a, bool>;
    /// A `ProfileRequest` (`wire-format.md` §7.11): the node's reply, or
    /// nothing where it answers none.
    fn profile<'a>(&'a self, body: &'a [u8]) -> Answer<'a, Option<Vec<u8>>>;
    /// A `CatalogQuery` from `from` (`wire-format.md` §6.4): the node's
    /// reply, or nothing where it answers none.
    fn catalog<'a>(&'a self, from: Keyhash, body: &'a [u8]) -> Answer<'a, Option<Vec<u8>>>;
    /// A `CatalogPageRequest` from `from` (`wire-format.md` §6.9): the
    /// node's page for it, or nothing where it answers none, which is what
    /// a node that serves no page answers.
    fn page<'a>(&'a self, _from: Keyhash, _body: &'a [u8]) -> Answer<'a, Option<Vec<u8>>> {
        Box::pin(async { None })
    }
    /// A `ResourceRequest` from `from` (`wire-format.md` §11.1): the
    /// node's `ResourceResponse`, or nothing where it answers none.
    fn resource<'a>(&'a self, from: Keyhash, body: &'a [u8]) -> Answer<'a, Option<Vec<u8>>>;
    /// Register, refresh or withdraw where this node rings the caller
    /// (design §14.1.5).  No endpoint withdraws.
    fn wake<'a>(&'a self, client: Keyhash, endpoint: Option<WakeEndpoint>) -> Answer<'a, bool>;
    /// Hand the node a transaction this client made, for it to store and
    /// flood (`wire-format.md` §10.1).
    ///
    /// **A client originates and does not forward.** It is a leaf: the
    /// records it makes are its own, and the one party that can put them
    /// into the flood is the node serving it, which §10.1.1 already counts
    /// it an adjacency of. Nothing comes back — the push is not a request
    /// and §10.1.3 defines no acknowledgement, deliberately.
    fn propagate<'a>(&'a self, bytes: Vec<u8>) -> Answer<'a, bool>;
}

/// **No serving node at all** (design §6.4): what a detached client's
/// courier is built over, so that the direct path exists before any
/// attach and after a detach.  Every question a node would answer is
/// answered as a node that is not there answers it — nothing published,
/// nothing relayed, nothing held — and the client's payload goes direct
/// where a path is held and nowhere otherwise.
pub struct Nobody;

impl Serving for Nobody {
    fn me(&self) -> Keyhash {
        [0; 32]
    }
    fn serves(&self, _: &Keyhash) -> bool {
        false
    }
    fn teach<'a>(&'a self, _: Vec<Vec<u8>>) -> Answer<'a, usize> {
        Box::pin(async { 0 })
    }
    fn publish_profile<'a>(&'a self, _: &'a [u8]) -> Answer<'a, bool> {
        Box::pin(async { false })
    }
    fn assert_horizon<'a>(&'a self, _: &'a [u8], _: [u8; 16]) -> Answer<'a, bool> {
        Box::pin(async { false })
    }
    fn profile<'a>(&'a self, _: &'a [u8]) -> Answer<'a, Option<Vec<u8>>> {
        Box::pin(async { None })
    }
    fn catalog<'a>(&'a self, _: Keyhash, _: &'a [u8]) -> Answer<'a, Option<Vec<u8>>> {
        Box::pin(async { None })
    }
    fn resource<'a>(&'a self, _: Keyhash, _: &'a [u8]) -> Answer<'a, Option<Vec<u8>>> {
        Box::pin(async { None })
    }
    fn wake<'a>(&'a self, _: Keyhash, _: Option<WakeEndpoint>) -> Answer<'a, bool> {
        Box::pin(async { false })
    }
    fn propagate<'a>(&'a self, _: Vec<u8>) -> Answer<'a, bool> {
        Box::pin(async { false })
    }
}

/// The node this client lives beside, as its serving node.
pub struct LocalNode {
    /// The node itself.
    pub node: Arc<LiveNode>,
    /// Where payload for each hosted client is delivered.
    pub inboxes: Inboxes,
    /// Where this node cannot answer, a subject whose material it does not
    /// hold or a recipient it neither hosts nor serves: on the wire the
    /// subject's own serving node, reached by resolution; here whatever
    /// the process holds, or nothing.
    beyond: Mutex<Option<Arc<dyn Serving>>>,
}

impl LocalNode {
    /// A local node wrapping `node`, delivering to `inboxes`.
    pub fn new(node: Arc<LiveNode>, inboxes: Inboxes) -> Arc<LocalNode> {
        Arc::new(LocalNode {
            node,
            inboxes,
            beyond: Mutex::new(None),
        })
    }

    /// Ask `other` for what this node cannot answer.
    pub fn reach_beyond(&self, other: Arc<dyn Serving>) {
        *self.beyond.lock().unwrap() = Some(other);
    }

    fn beyond(&self) -> Option<Arc<dyn Serving>> {
        self.beyond.lock().unwrap().clone()
    }
}

impl Serving for LocalNode {
    fn me(&self) -> Keyhash {
        self.node.me()
    }

    fn serves(&self, client: &Keyhash) -> bool {
        self.inboxes.hosts(client) || self.node.node.has_session(client)
    }

    /// **As nobody, in process** (`wire-format.md` §9.1): the node's own
    /// request handler answers as it answers a minted connection, a
    /// recipient this node neither hosts nor serves is put to the node
    /// beyond where that one does, and a sealed message for a client hosted
    /// here is handed to it from the queue, as a session's drain would.
    fn minted<'a>(&'a self, request: u64, body: Vec<u8>) -> Answer<'a, Option<Vec<u8>>> {
        Box::pin(async move {
            use rhtn_archive::node_key::{NodeKeyRequest, REQUEST_NODE_KEY};
            use rhtn_archive::submission::{REQUEST_RELAY, RelaySubmission};
            let (family, recipient) = match request {
                REQUEST_NODE_KEY => (
                    rhtn_codec::schema::Family::NodeKeyRequestOrBatch,
                    NodeKeyRequest::decode(&body)
                        .ok()?
                        .recipients()
                        .first()
                        .copied()?,
                ),
                REQUEST_RELAY => (
                    rhtn_codec::schema::Family::RelaySubmission,
                    RelaySubmission::decode(&body).ok()?.recipient,
                ),
                _ => return None,
            };
            if !self.serves(&recipient)
                && let Some(b) = self.beyond()
                && b.serves(&recipient)
            {
                return b.minted(request, body).await;
            }
            let handler = self.node.node.cfg.on_request.clone()?;
            let reply = handler(rhtn_transport::session::MINTED, [0; 32], family, body).await;
            if request == REQUEST_RELAY {
                self.inboxes.deliver_queued(&self.node, &recipient);
            }
            reply
        })
    }

    /// **The in-process node takes the client's acts as a remote one
    /// does** (`wire-format.md` §10.1.1): through its own receive, so the
    /// store reach, the verification and the forwarding are the node's
    /// and not a harness's shortcut.
    fn teach<'a>(&'a self, frames: Vec<Vec<u8>>) -> Answer<'a, usize> {
        Box::pin(async move {
            let me = self.node.me();
            let mut took = 0usize;
            for frame in frames {
                let decided = {
                    let mut view = self.node.view.lock().unwrap();
                    let ids = self.node.ids.lock().unwrap();
                    view.receive_push(&self.node.adjacency, &me, &frame, &*ids)
                };
                if decided == rhtn_node::store::Decision::Stored {
                    took += 1;
                }
            }
            took
        })
    }

    fn publish_profile<'a>(&'a self, bytes: &'a [u8]) -> Answer<'a, bool> {
        Box::pin(async move {
            let mut view = self.node.view.lock().unwrap();
            let ids = self.node.ids.lock().unwrap();
            view.profiles.publish(&*ids, bytes).is_ok()
        })
    }

    fn assert_horizon<'a>(&'a self, bytes: &'a [u8], _: [u8; 16]) -> Answer<'a, bool> {
        Box::pin(async move {
            // **the sender is this client, as the connection would have
            // bound it** (`wire-format.md` §7.12): in process there is no
            // handshake, so the identity is supplied rather than read, and
            // it is the same keyhash a real one would have presented
            let me = self.me();
            let mut view = self.node.view.lock().unwrap();
            let pins = &self.node.node.cfg.pins;
            // the identities this node can verify a signature from, which a
            // taken row adds to: in process as on the wire
            let mut ids = self.node.ids.lock().unwrap();
            rhtn_node::submissions::horizon(&mut view, Some(pins), &mut ids, &me, bytes)
                .is_some_and(|r| {
                    rhtn_archive::submission::SubmissionReply::decode(&r)
                        .is_ok_and(|r| r.code == rhtn_archive::submission::SUBMISSION_ACCEPTED)
                })
        })
    }

    fn profile<'a>(&'a self, body: &'a [u8]) -> Answer<'a, Option<Vec<u8>>> {
        Box::pin(async move {
            // held here, or asked of the node beyond where a subject's
            // locator leads there and this node holds nothing
            let held = self.node.view.lock().unwrap().profiles.answer(body);
            match held {
                Some(reply)
                    if rhtn_archive::profile::ProfileReply::decode(&reply)
                        .is_ok_and(|r| r.profile.is_some()) =>
                {
                    Some(reply)
                }
                other => match self.beyond() {
                    Some(b) => b.profile(body).await.or(other),
                    None => other,
                },
            }
        })
    }

    /// The node beside this client takes it the way it takes one off the
    /// wire, arriving from the client: stored where the client is in its
    /// store reach, and forwarded to every adjacency but this one.
    fn propagate<'a>(&'a self, bytes: Vec<u8>) -> Answer<'a, bool> {
        Box::pin(async move {
            let from = self.node.me();
            let mut view = self.node.view.lock().unwrap();
            let ids = self.node.ids.lock().unwrap();
            let decision = view.take_object(
                &self.node.adjacency,
                &from,
                rhtn_node::store::KIND_TRANSACTION,
                &bytes,
                &*ids,
            );
            let taken = matches!(
                decision,
                rhtn_node::store::Decision::Stored | rhtn_node::store::Decision::Duplicate
            );
            // the decision, because a refusal reaches the client as one
            // bit and a test that sees that bit intermittently needs to
            // know which refusal it was
            tracing::debug!(target: "node", taken, decision = ?decision, "node.propagate");
            taken
        })
    }

    fn catalog<'a>(&'a self, from: Keyhash, body: &'a [u8]) -> Answer<'a, Option<Vec<u8>>> {
        Box::pin(async move {
            let view = self.node.view.lock().unwrap();
            let me = view.me();
            let scopes = rhtn_node::catalog::TableScopes {
                table: &view.table,
                me,
            };
            view.catalog.answer(&from, body, &scopes)
        })
    }

    fn page<'a>(&'a self, from: Keyhash, body: &'a [u8]) -> Answer<'a, Option<Vec<u8>>> {
        Box::pin(async move {
            let req = rhtn_archive::catalog::CatalogPageRequest::decode(body).ok()?;
            let view = self.node.view.lock().unwrap();
            let me = view.me();
            if !view.table.horizon(&me, 2).contains(&from) {
                return None;
            }
            Some(
                rhtn_archive::catalog::CatalogPage {
                    nonce: req.nonce,
                    items: view.resources.page(&from, &view.catalog),
                }
                .encode(),
            )
        })
    }

    fn resource<'a>(&'a self, from: Keyhash, body: &'a [u8]) -> Answer<'a, Option<Vec<u8>>> {
        Box::pin(async move {
            let mut view = self.node.view.lock().unwrap();
            let me = view.me();
            let table = view.table.clone_for(me);
            let now = view.now();
            Some(view.resources.serve(&me, &table, &from, body, now).encode())
        })
    }

    fn wake<'a>(&'a self, client: Keyhash, endpoint: Option<WakeEndpoint>) -> Answer<'a, bool> {
        Box::pin(async move {
            let mut view = self.node.view.lock().unwrap();
            match endpoint {
                // a client beside its node speaks from no handshake: its
                // endpoint is held for any of its devices
                Some(e) => {
                    view.wake.register(
                        client,
                        [0; 32],
                        Some(e.url),
                        Some(e.key),
                        e.lapses_at,
                        Some(e.signing_key),
                    ) != rhtn_node::wake::Registered::Refused
                }
                None => {
                    view.wake.forget(&client);
                    true
                }
            }
        })
    }
}
