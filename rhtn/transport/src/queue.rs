//! The serving node's mailbox (design §14.1.6, `infra-client-requirements.md`
//! §2): ciphertext held for a client that is not attached, with the minimum
//! metadata — recipient keyhash and arrival time — deleted on delivery,
//! capped per subordinate with the newest refused, and kept for an absence
//! of any length.  Two stores: in memory, and a directory of files for a
//! node that restarts.

use std::collections::{HashMap, VecDeque};
use std::path::PathBuf;
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

/// One file per message under `<dir>/<recipient hex>/<arrival>-<seq>`: the
/// path carries the recipient and arrival time, the content is the
/// ciphertext, and delivery unlinks the file.  No journal, no tombstone.
pub struct DirStore {
    dir: PathBuf,
    seq: Mutex<u64>,
}

impl DirStore {
    pub fn new(dir: impl Into<PathBuf>) -> Self {
        let dir = dir.into();
        std::fs::create_dir_all(&dir).expect("queue directory");
        DirStore { dir, seq: Mutex::new(0) }
    }

    fn recipient_dir(&self, r: &[u8; 32]) -> PathBuf {
        self.dir.join(r.iter().map(|b| format!("{b:02x}")).collect::<String>())
    }

    /// Files for one recipient, oldest first: (arrival, path).
    fn files(&self, r: &[u8; 32]) -> Vec<(u64, u64, PathBuf)> {
        let mut out = Vec::new();
        if let Ok(rd) = std::fs::read_dir(self.recipient_dir(r)) {
            for e in rd.flatten() {
                let name = e.file_name().to_string_lossy().to_string();
                if let Some((a, s)) = name.split_once('-')
                    && let (Ok(a), Ok(s)) = (a.parse::<u64>(), s.parse::<u64>()) {
                        out.push((a, s, e.path()));
                    }
            }
        }
        out.sort();
        out
    }

    /// Every file under the store, for a test that checks nothing outlives a
    /// delivery.
    pub fn all_files(&self) -> Vec<PathBuf> {
        let mut out = Vec::new();
        if let Ok(rd) = std::fs::read_dir(&self.dir) {
            for d in rd.flatten() {
                if let Ok(inner) = std::fs::read_dir(d.path()) {
                    out.extend(inner.flatten().map(|e| e.path()));
                }
            }
        }
        out
    }
}

impl QueueStore for DirStore {
    fn push(&self, item: Queued) {
        let dir = self.recipient_dir(&item.recipient);
        std::fs::create_dir_all(&dir).expect("recipient directory");
        let mut seq = self.seq.lock().unwrap();
        *seq += 1;
        let path = dir.join(format!("{}-{}", item.arrival, *seq));
        std::fs::write(path, &item.ciphertext).expect("queue write");
    }
    fn take_all(&self, recipient: &[u8; 32]) -> Vec<Queued> {
        let mut out = Vec::new();
        for (arrival, _, path) in self.files(recipient) {
            if let Ok(ciphertext) = std::fs::read(&path) {
                let _ = std::fs::remove_file(&path);
                out.push(Queued { ciphertext, recipient: *recipient, arrival });
            }
        }
        let _ = std::fs::remove_dir(self.recipient_dir(recipient));
        out
    }
    fn list(&self, recipient: &[u8; 32]) -> Vec<Queued> {
        self.files(recipient).into_iter().filter_map(|(arrival, _, path)| std::fs::read(&path).ok().map(|ciphertext| Queued { ciphertext, recipient: *recipient, arrival })).collect()
    }
    fn drop_all(&self, recipient: &[u8; 32]) {
        let _ = std::fs::remove_dir_all(self.recipient_dir(recipient));
    }
}
