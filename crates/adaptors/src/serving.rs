//! What a client asks of its serving node, answered by the node beside it.

use rhtn_archive::Keyhash;
use rhtn_archive::prekey::PrekeyRequest;
use rhtn_archive::submission::WakeEndpoint;
use rhtn_node::runtime::LiveNode;
use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, Mutex};

/// Where payload for a client goes, with the peer it came from.
/// What a delivery hands the client: the submitter the node named, the
/// ciphertext, and the submitter's binding where the node carried one
/// (`wire-format.md` §7.10).
pub type Inbound = Arc<dyn Fn(Keyhash, Vec<u8>, Option<Vec<u8>>) + Send + Sync>;

/// The clients hosted in this process, by keyhash, each with where its
/// payload goes: what a node holds for a client beside it in place of a
/// session.
#[derive(Clone, Default)]
pub struct Inboxes(Arc<Mutex<HashMap<Keyhash, Inbound>>>);

impl Inboxes {
    /// Deliver `client`'s payload to `inbound` from now on.
    pub fn host(&self, client: Keyhash, inbound: Inbound) {
        self.0.lock().unwrap().insert(client, inbound);
    }

    /// Whether this process hosts `client`.
    pub fn hosts(&self, client: &Keyhash) -> bool {
        self.0.lock().unwrap().contains_key(client)
    }

    /// Deliver to a client hosted here; false where none is.
    pub fn deliver(&self, to: &Keyhash, from: Keyhash, bytes: Vec<u8>) -> bool {
        let inbound = self.0.lock().unwrap().get(to).cloned();
        match inbound {
            Some(f) => {
                f(from, bytes, None);
                true
            }
            None => false,
        }
    }
}

/// An answer that may have to cross the wire before it exists.
pub type Answer<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

/// What a client asks of its serving node (design §14.1.2, §14.2.4.5):
/// its prekey material published and stocked, a peer's fetched, payload
/// carried to a peer it holds no direct path to, and where to be rung.
/// `wire-format.md` §7.10 carries four of these as request types and §7.8
/// the fetch, so a node beside the client and a node across a session
/// answer the same questions.
///
/// The four that change something are asynchronous because one of the two
/// implementations has to wait for an answer.  The two predicates are not:
/// they say what this side knows without asking, and a node across a
/// session knows neither.
pub trait Serving: Send + Sync {
    /// The serving node's own identity.
    fn me(&self) -> Keyhash;
    /// The serving node's own device: the key it presents in a handshake
    /// (`wire-format.md` §8.2), which is where material for the node
    /// itself is addressed.
    fn node_device(&self) -> [u8; 32];
    /// Whether this node is known here to hold `subject`'s prekey
    /// material.
    fn holds(&self, subject: &Keyhash) -> bool;
    /// Whether this node is known here to host or serve `client`.
    fn serves(&self, client: &Keyhash) -> bool;
    /// Publish the caller's own bundle; whether the node took it.
    fn publish<'a>(&'a self, bytes: &'a [u8]) -> Answer<'a, bool>;
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
    /// Stock `subject`'s one-time pool; whether the node took the
    /// deposit, which a bound may refuse whole.
    fn stock<'a>(&'a self, subject: Keyhash, keys: Vec<Vec<u8>>) -> Answer<'a, bool>;
    /// A `PrekeyRequest` from `from` (`wire-format.md` §7.8): the node's
    /// reply, or nothing where it answers none.
    fn prekey<'a>(&'a self, from: Keyhash, body: &'a [u8]) -> Answer<'a, Option<Vec<u8>>>;
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
    /// A `ResourceRequest` from `from` (`wire-format.md` §11.1): the
    /// node's `ResourceResponse`, or nothing where it answers none.
    fn resource<'a>(&'a self, from: Keyhash, body: &'a [u8]) -> Answer<'a, Option<Vec<u8>>>;
    /// Carry `bytes` from `from` to `to`; whether anything took them.
    /// `device`: the recipient's device the ciphertext is for
    /// (`wire-format.md` §7.10 field 4).
    fn relay<'a>(
        &'a self,
        from: Keyhash,
        to: Keyhash,
        bytes: Vec<u8>,
        device: [u8; 32],
    ) -> Answer<'a, bool>;
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
    fn node_device(&self) -> [u8; 32] {
        [0; 32]
    }
    fn holds(&self, _: &Keyhash) -> bool {
        false
    }
    fn serves(&self, _: &Keyhash) -> bool {
        false
    }
    fn publish<'a>(&'a self, _: &'a [u8]) -> Answer<'a, bool> {
        Box::pin(async { false })
    }
    fn teach<'a>(&'a self, _: Vec<Vec<u8>>) -> Answer<'a, usize> {
        Box::pin(async { 0 })
    }
    fn stock<'a>(&'a self, _: Keyhash, _: Vec<Vec<u8>>) -> Answer<'a, bool> {
        Box::pin(async { false })
    }
    fn prekey<'a>(&'a self, _: Keyhash, _: &'a [u8]) -> Answer<'a, Option<Vec<u8>>> {
        Box::pin(async { None })
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
    fn relay<'a>(&'a self, _: Keyhash, _: Keyhash, _: Vec<u8>, _: [u8; 32]) -> Answer<'a, bool> {
        Box::pin(async { false })
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

    fn node_device(&self) -> [u8; 32] {
        self.node.node.cfg.me.presenter.presented_key()
    }

    fn holds(&self, subject: &Keyhash) -> bool {
        self.node
            .view
            .lock()
            .unwrap()
            .prekeys
            .bundle(subject)
            .is_some()
    }

    fn serves(&self, client: &Keyhash) -> bool {
        self.inboxes.hosts(client) || self.node.node.has_session(client)
    }

    fn publish<'a>(&'a self, bytes: &'a [u8]) -> Answer<'a, bool> {
        Box::pin(async move {
            let mut view = self.node.view.lock().unwrap();
            let ids = self.node.ids.lock().unwrap();
            view.prekeys.publish(&*ids, bytes).is_ok()
        })
    }

    fn stock<'a>(&'a self, subject: Keyhash, keys: Vec<Vec<u8>>) -> Answer<'a, bool> {
        Box::pin(async move { self.node.view.lock().unwrap().prekeys.stock(subject, keys) })
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

    fn resource<'a>(&'a self, from: Keyhash, body: &'a [u8]) -> Answer<'a, Option<Vec<u8>>> {
        Box::pin(async move {
            let mut view = self.node.view.lock().unwrap();
            let me = view.me();
            let table = view.table.clone_for(me);
            Some(view.resources.serve(&me, &table, &from, body).encode())
        })
    }

    fn prekey<'a>(&'a self, from: Keyhash, body: &'a [u8]) -> Answer<'a, Option<Vec<u8>>> {
        Box::pin(async move {
            // a subject this node holds no material for is another node's:
            // asked of the node beyond only where that node holds it, so two
            // nodes each beyond the other never bounce a fetch between them
            if let Ok(PrekeyRequest::One { subject, .. }) = PrekeyRequest::decode(body)
                && !self.holds(&subject)
                && let Some(b) = self.beyond()
                && b.holds(&subject)
            {
                return b.prekey(from, body).await;
            }
            let mut view = self.node.view.lock().unwrap();
            let now = view.now();
            view.prekeys.answer(&from, body, now)
        })
    }

    fn relay<'a>(
        &'a self,
        from: Keyhash,
        to: Keyhash,
        bytes: Vec<u8>,
        device: [u8; 32],
    ) -> Answer<'a, bool> {
        Box::pin(async move {
            // hosted here: handed over, no queue between
            if self.inboxes.deliver(&to, from, bytes.clone()) {
                return true;
            }
            // served here: delivered on the session or queued for the next
            // (design §14.1.6), the sender named in front for the
            // recipient, as `wire-format.md` §7.10 composes it.
            // **No binding goes with it here**: this path is handed the
            // recipient's device and never the submitter's, so it cannot
            // name the bundle §7.10's third element wants.  The session
            // path that authenticated a submitting device carries one
            // (`node/src/submissions.rs`)
            if self.node.node.has_session(&to) {
                return self
                    .node
                    .node
                    .enqueue_for(to, device, framed(from, &bytes, None))
                    .is_ok();
            }
            // a recipient the node beyond serves goes there; anyone else waits
            // here for a session (design §14.1.6)
            match self.beyond() {
                Some(b) if b.serves(&to) => b.relay(from, to, bytes, device).await,
                _ => self
                    .node
                    .node
                    .enqueue_for(to, device, framed(from, &bytes, None))
                    .is_ok(),
            }
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
                        rhtn_node::prekeys::ANY_DEVICE,
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

/// Relayed payload as the wire composes it (`wire-format.md` §7.10): the
/// submitter in front of the ciphertext.  A node beside its client and a
/// node reached over a session put the same bytes in the mailbox.
pub use rhtn_archive::submission::{relayed as framed, unrelayed as unframed};
