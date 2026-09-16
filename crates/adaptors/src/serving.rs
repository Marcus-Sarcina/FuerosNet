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
pub type Inbound = Arc<dyn Fn(Keyhash, Vec<u8>) + Send + Sync>;

/// The clients hosted in this process, by keyhash, each with where its
/// payload goes: what a node holds for a client beside it in place of a
/// session.
#[derive(Clone, Default)]
pub struct Inboxes(Arc<Mutex<HashMap<Keyhash, Inbound>>>);

impl Inboxes {
    pub fn host(&self, client: Keyhash, inbound: Inbound) {
        self.0.lock().unwrap().insert(client, inbound);
    }

    pub fn hosts(&self, client: &Keyhash) -> bool {
        self.0.lock().unwrap().contains_key(client)
    }

    /// Deliver to a client hosted here; false where none is.
    pub fn deliver(&self, to: &Keyhash, from: Keyhash, bytes: Vec<u8>) -> bool {
        let inbound = self.0.lock().unwrap().get(to).cloned();
        match inbound {
            Some(f) => {
                f(from, bytes);
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
    fn me(&self) -> Keyhash;
    /// Whether this node is known here to hold `subject`'s prekey
    /// material.
    fn holds(&self, subject: &Keyhash) -> bool;
    /// Whether this node is known here to host or serve `client`.
    fn serves(&self, client: &Keyhash) -> bool;
    /// Publish the caller's own bundle; whether the node took it.
    fn publish<'a>(&'a self, bytes: &'a [u8]) -> Answer<'a, bool>;
    /// Stock `subject`'s one-time pool; whether the node took the
    /// deposit, which a bound may refuse whole.
    fn stock<'a>(&'a self, subject: Keyhash, keys: Vec<Vec<u8>>) -> Answer<'a, bool>;
    fn prekey<'a>(&'a self, from: Keyhash, body: &'a [u8]) -> Answer<'a, Option<Vec<u8>>>;
    /// Carry `bytes` from `from` to `to`; whether anything took them.
    fn relay<'a>(&'a self, from: Keyhash, to: Keyhash, bytes: Vec<u8>) -> Answer<'a, bool>;
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

/// The node this client lives beside, as its serving node.
pub struct LocalNode {
    pub node: Arc<LiveNode>,
    pub inboxes: Inboxes,
    /// Where this node cannot answer, a subject whose material it does not
    /// hold or a recipient it neither hosts nor serves: on the wire the
    /// subject's own serving node, reached by resolution; here whatever
    /// the process holds, or nothing.
    beyond: Mutex<Option<Arc<dyn Serving>>>,
}

impl LocalNode {
    pub fn new(node: Arc<LiveNode>, inboxes: Inboxes) -> Arc<LocalNode> {
        Arc::new(LocalNode { node, inboxes, beyond: Mutex::new(None) })
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

    fn holds(&self, subject: &Keyhash) -> bool {
        self.node.view.lock().unwrap().prekeys.bundle(subject).is_some()
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

    /// The node beside this client takes it the way it takes one off the
    /// wire, arriving from the client: stored where the client is in its
    /// store reach, and forwarded to every adjacency but this one.
    fn propagate<'a>(&'a self, bytes: Vec<u8>) -> Answer<'a, bool> {
        Box::pin(async move {
            let from = self.node.me();
            let mut view = self.node.view.lock().unwrap();
            let ids = self.node.ids.lock().unwrap();
            matches!(
                view.take_object(&self.node.adjacency, &from, rhtn_node::store::KIND_TRANSACTION, &bytes, &*ids),
                rhtn_node::store::Decision::Stored | rhtn_node::store::Decision::Duplicate
            )
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

    fn relay<'a>(&'a self, from: Keyhash, to: Keyhash, bytes: Vec<u8>) -> Answer<'a, bool> {
        Box::pin(async move {
            // hosted here: handed over, no queue between
            if self.inboxes.deliver(&to, from, bytes.clone()) {
                return true;
            }
            // served here: delivered on the session or queued for the next
            // (design §14.1.6), the sender named in front for the
            // recipient, as `wire-format.md` §7.10 composes it
            if self.node.node.has_session(&to) {
                return self.node.node.enqueue(to, framed(from, &bytes)).is_ok();
            }
            // a recipient the node beyond serves goes there; anyone else waits
            // here for a session (design §14.1.6)
            match self.beyond() {
                Some(b) if b.serves(&to) => b.relay(from, to, bytes).await,
                _ => self.node.node.enqueue(to, framed(from, &bytes)).is_ok(),
            }
        })
    }

    fn wake<'a>(&'a self, client: Keyhash, endpoint: Option<WakeEndpoint>) -> Answer<'a, bool> {
        Box::pin(async move {
            let mut view = self.node.view.lock().unwrap();
            match endpoint {
                Some(e) => view.wake.register(client, Some(e.url), Some(e.key), e.lapses_at) != rhtn_node::wake::Registered::Refused,
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
