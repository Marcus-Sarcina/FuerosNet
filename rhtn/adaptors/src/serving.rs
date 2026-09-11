//! What a client asks of its serving node, answered by the node beside it.

use rhtn_archive::Keyhash;
use rhtn_archive::prekey::PrekeyRequest;
use rhtn_codec::cbor::{Item, parse_all};
use rhtn_codec::encode::{emit_array_head, emit_bstr};
use rhtn_node::runtime::LiveNode;
use std::collections::HashMap;
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

/// What a client asks of its serving node (design §14.1.2, §14.2.4.5):
/// its prekey material published and stocked, a peer's fetched, and
/// payload carried to a peer it holds no direct path to.  The wire
/// carries the fetch (request type 3) and the node's delivery to a client
/// it serves; how a client hands its serving node material to relay is not
/// written, so the implementation that exists is the node beside the
/// client.
pub trait Serving: Send + Sync {
    fn me(&self) -> Keyhash;
    /// Whether this node holds `subject`'s prekey material itself.
    fn holds(&self, subject: &Keyhash) -> bool;
    /// Whether this node hosts or serves `client` itself.
    fn serves(&self, client: &Keyhash) -> bool;
    fn publish(&self, bytes: &[u8]) -> bool;
    fn stock(&self, subject: Keyhash, keys: Vec<Vec<u8>>);
    fn prekey(&self, from: Keyhash, body: &[u8]) -> Option<Vec<u8>>;
    /// Carry `bytes` from `from` to `to`; whether anything took them.
    fn relay(&self, from: Keyhash, to: Keyhash, bytes: Vec<u8>) -> bool;
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

    fn publish(&self, bytes: &[u8]) -> bool {
        let mut view = self.node.view.lock().unwrap();
        let ids = self.node.ids.lock().unwrap();
        view.prekeys.publish(&*ids, bytes).is_ok()
    }

    fn stock(&self, subject: Keyhash, keys: Vec<Vec<u8>>) {
        self.node.view.lock().unwrap().prekeys.stock(subject, keys);
    }

    fn prekey(&self, from: Keyhash, body: &[u8]) -> Option<Vec<u8>> {
        // a subject this node holds no material for is another node's:
        // asked of the node beyond only where that node holds it, so two
        // nodes each beyond the other never bounce a fetch between them
        if let Ok(PrekeyRequest::One { subject, .. }) = PrekeyRequest::decode(body)
            && !self.holds(&subject)
            && let Some(b) = self.beyond()
            && b.holds(&subject)
        {
            return b.prekey(from, body);
        }
        let mut view = self.node.view.lock().unwrap();
        let now = view.now();
        view.prekeys.answer(&from, body, now)
    }

    fn relay(&self, from: Keyhash, to: Keyhash, bytes: Vec<u8>) -> bool {
        // hosted here: handed over, no queue between
        if self.inboxes.deliver(&to, from, bytes.clone()) {
            return true;
        }
        // served here: delivered on the session or queued for the next
        // (design §14.1.6), the sender named in front for the recipient's
        // adaptor
        if self.node.node.has_session(&to) {
            return self.node.node.enqueue(to, framed(from, &bytes)).is_ok();
        }
        // a recipient the node beyond serves goes there; anyone else waits
        // here for a session (design §14.1.6)
        match self.beyond() {
            Some(b) if b.serves(&to) => b.relay(from, to, bytes),
            _ => self.node.node.enqueue(to, framed(from, &bytes)).is_ok(),
        }
    }
}

/// Relayed payload as the adaptors carry it: the sender in front of the
/// bytes, since the queue and the delivery stream carry bytes alone and
/// the recipient decrypts under the sender.  An adaptor convention; the
/// wire names no envelope for relayed payload.
pub fn framed(from: Keyhash, bytes: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    emit_array_head(&mut out, 2);
    emit_bstr(&mut out, &from);
    emit_bstr(&mut out, bytes);
    out
}

pub fn unframed(b: &[u8]) -> Option<(Keyhash, Vec<u8>)> {
    let item = parse_all(b).ok()?;
    let Item::Array(parts) = &item else { return None };
    let [Item::Bytes(f), Item::Bytes(p)] = parts.as_slice() else { return None };
    let from: Keyhash = b[f.clone()].try_into().ok()?;
    Some((from, b[p.clone()].to_vec()))
}
