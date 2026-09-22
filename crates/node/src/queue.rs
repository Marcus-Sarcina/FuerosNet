//! The serving node's mailbox on disk (design §14.1.6,
//! `infra-client-requirements.md` §2): a directory of files for a node that
//! restarts, one per waiting message, with the minimum metadata in the path
//! and the ciphertext as the content.  The store contract and the memory
//! store are the transport's, since the session layer delivers from them;
//! this is the store a node runs on.

use std::path::PathBuf;
use std::sync::Mutex;

pub use rhtn_transport::queue::{ANY_DEVICE, MemoryStore, QueueStore, Queued, Refusal, for_device};

/// One file per message under `<dir>/<recipient hex>/<arrival>-<seq>-<device
/// hex>`: the path carries the recipient, the arrival time and the device,
/// the content is the ciphertext, and delivery unlinks the file.  No
/// journal, no tombstone.
/// The sequence resumes past every file the directory already holds and a
/// file is only ever created, never overwritten, so a restart in the same
/// second as an earlier arrival cannot take an accepted message's name.
pub struct DirStore {
    dir: PathBuf,
    seq: Mutex<u64>,
}

impl DirStore {
    pub fn new(dir: impl Into<PathBuf>) -> Self {
        let dir = dir.into();
        std::fs::create_dir_all(&dir).expect("queue directory");
        let mut seq = 0u64;
        if let Ok(rd) = std::fs::read_dir(&dir) {
            for d in rd.flatten() {
                if let Ok(inner) = std::fs::read_dir(d.path()) {
                    for e in inner.flatten() {
                        if let Some((_, s, _)) = split_name(&e.file_name().to_string_lossy()) {
                            seq = seq.max(s);
                        }
                    }
                }
            }
        }
        DirStore {
            dir,
            seq: Mutex::new(seq),
        }
    }

    fn recipient_dir(&self, r: &[u8; 32]) -> PathBuf {
        self.dir
            .join(r.iter().map(|b| format!("{b:02x}")).collect::<String>())
    }

    /// Files for one recipient, oldest first: (arrival, seq, device, path).
    fn files(&self, r: &[u8; 32]) -> Vec<(u64, u64, [u8; 32], PathBuf)> {
        let mut out = Vec::new();
        if let Ok(rd) = std::fs::read_dir(self.recipient_dir(r)) {
            for e in rd.flatten() {
                let name = e.file_name().to_string_lossy().to_string();
                if let Some((a, s, d)) = split_name(&name) {
                    out.push((a, s, d, e.path()));
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

/// `<arrival>-<seq>-<device hex>` to its parts.
fn split_name(name: &str) -> Option<(u64, u64, [u8; 32])> {
    let mut it = name.splitn(3, '-');
    let a = it.next()?.parse::<u64>().ok()?;
    let s = it.next()?.parse::<u64>().ok()?;
    let d = it.next()?;
    if d.len() != 64 {
        return None;
    }
    let mut device = [0u8; 32];
    for (i, b) in device.iter_mut().enumerate() {
        *b = u8::from_str_radix(&d[i * 2..i * 2 + 2], 16).ok()?;
    }
    Some((a, s, device))
}

impl QueueStore for DirStore {
    fn push(&self, item: Queued) {
        use std::io::Write;
        let dir = self.recipient_dir(&item.recipient);
        std::fs::create_dir_all(&dir).expect("recipient directory");
        let mut seq = self.seq.lock().unwrap();
        let device: String = item.device.iter().map(|b| format!("{b:02x}")).collect();
        loop {
            *seq += 1;
            let path = dir.join(format!("{}-{}-{device}", item.arrival, *seq));
            // exclusive creation: a name already taken is passed over, and
            // nothing accepted is ever written over
            match std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&path)
            {
                Ok(mut f) => {
                    f.write_all(&item.ciphertext).expect("queue write");
                    return;
                }
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(e) => panic!("queue write: {e}"),
            }
        }
    }
    fn peek_oldest(&self, recipient: &[u8; 32]) -> Option<Queued> {
        self.files(recipient)
            .into_iter()
            .find_map(|(arrival, _, device, path)| {
                std::fs::read(&path).ok().map(|ciphertext| Queued {
                    ciphertext,
                    recipient: *recipient,
                    device,
                    arrival,
                })
            })
    }
    fn remove(&self, recipient: &[u8; 32], item: &Queued) -> bool {
        for (arrival, _, device, path) in self.files(recipient) {
            if arrival == item.arrival
                && device == item.device
                && std::fs::read(&path).is_ok_and(|c| c == item.ciphertext)
            {
                let _ = std::fs::remove_file(&path);
                let _ = std::fs::remove_dir(self.recipient_dir(recipient));
                return true;
            }
        }
        false
    }
    fn list(&self, recipient: &[u8; 32]) -> Vec<Queued> {
        self.files(recipient)
            .into_iter()
            .filter_map(|(arrival, _, device, path)| {
                std::fs::read(&path).ok().map(|ciphertext| Queued {
                    ciphertext,
                    recipient: *recipient,
                    device,
                    arrival,
                })
            })
            .collect()
    }
    fn drop_all(&self, recipient: &[u8; 32]) {
        let _ = std::fs::remove_dir_all(self.recipient_dir(recipient));
    }
}
