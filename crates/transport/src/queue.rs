//! The serving node's mailbox (design §14.1.6, `infra-client-requirements.md`
//! §2): ciphertext held for a client, with the minimum metadata — recipient
//! keyhash and arrival time — deleted on delivery and not before, capped
//! per subordinate with the newest refused, and kept for an absence of any
//! length.  Every accepted message enters the store; a live session drains
//! it from there, one message at a time, each removed once the peer has
//! taken it.  The store is the session layer's contract and the memory
//! store its default; the directory store a restarting node keeps is
//! `rhtn-node`'s (`Robot/implementation-plan.md`'s crate table).

use std::collections::{HashMap, VecDeque};
use std::sync::Mutex;

/// One waiting message: the whole of what the node holds about it
/// (design §14.1.6): ciphertext, recipient keyhash and device, arrival time.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Queued {
    pub ciphertext: Vec<u8>,
    pub recipient: [u8; 32],
    /// The recipient's device the ciphertext is readable by, named by the
    /// key it presents (`wire-format.md` §7.10); [`ANY_DEVICE`] where the
    /// submission named none, which any of the recipient's sessions drains.
    pub device: [u8; 32],
    pub arrival: u64,
}

/// The device a message for any of the recipient's devices names.
pub const ANY_DEVICE: [u8; 32] = [0; 32];

/// Whether a queued item is for the session of `device`.
pub fn for_device(item: &Queued, device: &[u8; 32]) -> bool {
    for_device_key(&item.device, device)
}

/// Whether a message named for `named` is for the session of `device`.
pub fn for_device_key(named: &[u8; 32], device: &[u8; 32]) -> bool {
    *named == ANY_DEVICE || *named == *device
}

/// Why a submission was not accepted; the sender is told.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Refusal {
    /// The recipient's queue is at its cap; what is queued is kept.
    AtCap,
    /// The node has verified this credential superseded (design §12.6.5).
    Superseded,
    /// This node holds no record of the recipient at all, which is a
    /// different answer from "offline" (design §14.1.2, §7.4.3).
    NoRecord,
}

pub trait QueueStore: Send + Sync {
    fn push(&self, item: Queued);
    /// The oldest message waiting for `recipient`, left in place: what a
    /// drain reads before it delivers.
    fn peek_oldest(&self, recipient: &[u8; 32]) -> Option<Queued>;
    /// Remove one message, the first equal to `item`, once it has been
    /// delivered.  Whether anything was removed.
    fn remove(&self, recipient: &[u8; 32], item: &Queued) -> bool;
    /// What is queued for `recipient`, for inspection; nothing is removed.
    fn list(&self, recipient: &[u8; 32]) -> Vec<Queued>;
    fn drop_all(&self, recipient: &[u8; 32]);
    fn count(&self, recipient: &[u8; 32]) -> usize {
        self.list(recipient).len()
    }
    /// The oldest message waiting for `recipient`'s `device`: what that
    /// device's session drains (design §14.1.6).
    fn peek_oldest_for(&self, recipient: &[u8; 32], device: &[u8; 32]) -> Option<Queued> {
        self.list(recipient)
            .into_iter()
            .find(|q| for_device(q, device))
    }
    fn count_for(&self, recipient: &[u8; 32], device: &[u8; 32]) -> usize {
        self.list(recipient)
            .iter()
            .filter(|q| for_device(q, device))
            .count()
    }
    fn bytes(&self, recipient: &[u8; 32]) -> usize {
        self.list(recipient)
            .iter()
            .map(|q| q.ciphertext.len())
            .sum()
    }
}

#[derive(Default)]
pub struct MemoryStore(Mutex<HashMap<[u8; 32], VecDeque<Queued>>>);

impl QueueStore for MemoryStore {
    fn push(&self, item: Queued) {
        self.0
            .lock()
            .unwrap()
            .entry(item.recipient)
            .or_default()
            .push_back(item);
    }
    fn peek_oldest(&self, recipient: &[u8; 32]) -> Option<Queued> {
        self.0
            .lock()
            .unwrap()
            .get(recipient)
            .and_then(|q| q.front().cloned())
    }
    fn remove(&self, recipient: &[u8; 32], item: &Queued) -> bool {
        let mut m = self.0.lock().unwrap();
        let Some(q) = m.get_mut(recipient) else {
            return false;
        };
        let Some(i) = q.iter().position(|x| x == item) else {
            return false;
        };
        q.remove(i);
        if q.is_empty() {
            m.remove(recipient);
        }
        true
    }
    fn list(&self, recipient: &[u8; 32]) -> Vec<Queued> {
        self.0
            .lock()
            .unwrap()
            .get(recipient)
            .map(|q| q.iter().cloned().collect())
            .unwrap_or_default()
    }
    fn drop_all(&self, recipient: &[u8; 32]) {
        self.0.lock().unwrap().remove(recipient);
    }
}
