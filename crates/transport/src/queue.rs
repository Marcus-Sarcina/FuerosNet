//! The serving node's mailbox (design §14.1.6, `infra-client-requirements.md`
//! §2): sealed messages held for a recipient, **per recipient and naming no
//! device**, with the minimum metadata — the recipient, the node key each
//! was sealed to, its position in the queue, its arrival and the time of its
//! first delivery.  An undelivered message is kept for an absence of any
//! length; a delivered one for the retention after its first delivery, so
//! every device of the identity collects it; at the per-subordinate cap
//! the oldest delivered message goes first, and only where every message
//! held is undelivered is the newest refused.  The store is the session
//! layer's contract and the memory store its default; the directory store
//! a restarting node keeps is `rhtn-node`'s.

use std::collections::{BTreeMap, HashMap};
use std::sync::Mutex;

/// One waiting message: the whole of what the node holds about it
/// (design §14.1.6).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Queued {
    /// The `Sealed` (`wire-format.md` §7.8.1), encoded: opaque to the node
    /// but for the half it opens at delivery.
    pub sealed: Vec<u8>,
    /// Who it is for.
    pub recipient: [u8; 32],
    /// The number of the node key it was sealed to.
    pub n: u64,
    /// Its position in the recipient's queue, increasing with every
    /// arrival and never reused (`wire-format.md` §7.10).
    pub position: u64,
    /// When it arrived, in seconds.
    pub arrival: u64,
    /// When it was first delivered, in seconds: what starts its retention.
    pub delivered_at: Option<u64>,
}

/// Why a submission was not accepted; the sender is told.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Refusal {
    /// The recipient's queue is at its cap with nothing delivered to make
    /// room; what is queued is kept.
    AtCap,
    /// The node has verified this credential superseded (design §12.6.5).
    Superseded,
    /// This node holds no record of the recipient at all, which is a
    /// different answer from "offline" (design §14.1.2, §7.4.3).
    NoRecord,
    /// The store would not take it -- a full or unwritable disk
    /// [2026-09-30].  **Accepted means stored** (§7.10), so a submission
    /// that cannot be stored is refused rather than acknowledged, and the
    /// sender still holds it.
    NotStored,
}

/// Where a node keeps what it holds for its clients. **Accepted means
/// stored** (`wire-format.md` §7.10), so an implementation that cannot
/// store says so rather than acknowledging.
pub trait QueueStore: Send + Sync {
    /// The position the next arrival for `recipient` takes, and the high
    /// mark advanced past it: increasing with every arrival and never
    /// reused, whatever has been removed since, so a device's cursor is
    /// never overtaken by a smaller number.
    fn next_position(&self, recipient: &[u8; 32]) -> u64;
    /// Store `item`.  **False means it is not stored**, and the caller
    /// refuses the submission rather than reporting it queued.
    fn push(&self, item: Queued) -> bool;
    /// What is held for `recipient`, in ascending position; nothing is
    /// removed.
    fn list(&self, recipient: &[u8; 32]) -> Vec<Queued>;
    /// Every recipient something is held for.
    fn recipients(&self) -> Vec<[u8; 32]>;
    /// Record the first delivery of the message at `position`; a later
    /// delivery leaves the first in place.  Whether anything changed.
    fn mark_delivered(&self, recipient: &[u8; 32], position: u64, at: u64) -> bool;
    /// Remove the message at `position`, immediately and leaving nothing
    /// recoverable (design §14.1.6).  Whether anything was removed.
    fn remove(&self, recipient: &[u8; 32], position: u64) -> bool;
    /// Drop everything queued for `recipient`.
    fn drop_all(&self, recipient: &[u8; 32]);
    /// How many messages are held for `recipient`.
    fn count(&self, recipient: &[u8; 32]) -> usize {
        self.list(recipient).len()
    }
    /// How many are newer than `cursor`: what an `AttachAck` reports to a
    /// device that holds everything up to it.
    fn count_after(&self, recipient: &[u8; 32], cursor: Option<u64>) -> usize {
        self.list(recipient)
            .iter()
            .filter(|q| cursor.is_none_or(|c| q.position > c))
            .count()
    }
    /// **The granularity this store consumes space in**, which is what
    /// turns a queue's content into the disk it occupies.
    ///
    /// One means no rounding, for a store that holds nothing on a disk.  A
    /// store keeping a file per item returns that filesystem's block size:
    /// a 200-byte message in a 4 KiB block costs 4 KiB, and a cap read
    /// against content would promise an operator a fifth of the disk it
    /// actually takes.
    fn block(&self) -> usize {
        1
    }
    /// What `bytes` of content cost in this store: rounded up to
    /// [`QueueStore::block`].
    fn cost(&self, bytes: usize) -> usize {
        let block = self.block().max(1);
        bytes.div_ceil(block) * block
    }
    /// **The disk `recipient`'s queue occupies, which is what the cap
    /// means** [author, 2026-11-06]: each item at its
    /// [`QueueStore::cost`].  **Data blocks only**; a file's own metadata
    /// is the filesystem's business.
    fn occupied(&self, recipient: &[u8; 32]) -> usize {
        self.list(recipient)
            .iter()
            .map(|q| self.cost(q.sealed.len()))
            .sum()
    }
}

/// An in-memory store, for a node that keeps nothing across a restart and
/// for tests.
#[derive(Default)]
pub struct MemoryStore {
    held: Mutex<HashMap<[u8; 32], BTreeMap<u64, Queued>>>,
    high: Mutex<HashMap<[u8; 32], u64>>,
}

impl QueueStore for MemoryStore {
    fn next_position(&self, recipient: &[u8; 32]) -> u64 {
        let mut high = self.high.lock().unwrap();
        let h = high.entry(*recipient).or_insert(0);
        *h += 1;
        *h
    }
    fn push(&self, item: Queued) -> bool {
        self.held
            .lock()
            .unwrap()
            .entry(item.recipient)
            .or_default()
            .insert(item.position, item);
        true
    }
    fn list(&self, recipient: &[u8; 32]) -> Vec<Queued> {
        self.held
            .lock()
            .unwrap()
            .get(recipient)
            .map(|q| q.values().cloned().collect())
            .unwrap_or_default()
    }
    fn recipients(&self) -> Vec<[u8; 32]> {
        self.held.lock().unwrap().keys().copied().collect()
    }
    fn mark_delivered(&self, recipient: &[u8; 32], position: u64, at: u64) -> bool {
        let mut m = self.held.lock().unwrap();
        match m.get_mut(recipient).and_then(|q| q.get_mut(&position)) {
            Some(q) if q.delivered_at.is_none() => {
                q.delivered_at = Some(at);
                true
            }
            _ => false,
        }
    }
    fn remove(&self, recipient: &[u8; 32], position: u64) -> bool {
        let mut m = self.held.lock().unwrap();
        let Some(q) = m.get_mut(recipient) else {
            return false;
        };
        let gone = q.remove(&position).is_some();
        if q.is_empty() {
            m.remove(recipient);
        }
        gone
    }
    fn drop_all(&self, recipient: &[u8; 32]) {
        self.held.lock().unwrap().remove(recipient);
    }
}
