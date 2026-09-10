//! The serving node's mailbox (design §14.1.6, `infra-client-requirements.md`
//! §2): ciphertext held for a client that is not attached, with the minimum
//! metadata — recipient keyhash and arrival time — deleted on delivery,
//! capped per subordinate with the newest refused, and kept for an absence
//! of any length.  The store is the session layer's contract and the memory
//! store its default; the directory store a restarting node keeps is
//! `rhtn-node`'s (`Robot/implementation-plan.md`'s crate table).

use std::collections::{HashMap, VecDeque};
use std::sync::Mutex;

/// One waiting message: the whole of what the node holds about it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Queued {
    pub ciphertext: Vec<u8>,
    pub recipient: [u8; 32],
    pub arrival: u64,
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
    /// Remove and return everything queued for `recipient`, oldest first.
    fn take_all(&self, recipient: &[u8; 32]) -> Vec<Queued>;
    /// What is queued for `recipient`, for inspection; nothing is removed.
    fn list(&self, recipient: &[u8; 32]) -> Vec<Queued>;
    fn drop_all(&self, recipient: &[u8; 32]);
    fn count(&self, recipient: &[u8; 32]) -> usize {
        self.list(recipient).len()
    }
    fn bytes(&self, recipient: &[u8; 32]) -> usize {
        self.list(recipient).iter().map(|q| q.ciphertext.len()).sum()
    }
}

#[derive(Default)]
pub struct MemoryStore(Mutex<HashMap<[u8; 32], VecDeque<Queued>>>);

impl QueueStore for MemoryStore {
    fn push(&self, item: Queued) {
        self.0.lock().unwrap().entry(item.recipient).or_default().push_back(item);
    }
    fn take_all(&self, recipient: &[u8; 32]) -> Vec<Queued> {
        self.0.lock().unwrap().remove(recipient).map(|q| q.into_iter().collect()).unwrap_or_default()
    }
    fn list(&self, recipient: &[u8; 32]) -> Vec<Queued> {
        self.0.lock().unwrap().get(recipient).map(|q| q.iter().cloned().collect()).unwrap_or_default()
    }
    fn drop_all(&self, recipient: &[u8; 32]) {
        self.0.lock().unwrap().remove(recipient);
    }
}
