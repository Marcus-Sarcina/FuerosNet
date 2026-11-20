//! The serving node's mailbox on disk (design §14.1.6,
//! `infra-client-requirements.md` §2): a directory of files for a node that
//! restarts, one per held message, with the minimum metadata in the path
//! and the sealed message as the content.  The store contract and the
//! memory store are the transport's, since the session layer delivers from
//! them; this is the store a node runs on.

use std::path::{Path, PathBuf};
use std::sync::Mutex;

pub use rhtn_transport::queue::{MemoryStore, QueueStore, Queued, Refusal};

/// One file per message under `<dir>/<recipient hex>/<position>-<n>-<arrival>`,
/// with `-<first delivery>` appended once it has been delivered: the path
/// carries the recipient, its position, the node key it was sealed to and
/// its times, and the content is the seal.  Removal unlinks the file — no
/// journal, no tombstone. A `high` file per recipient holds the last
/// position handed out, so a position is never reused across a restart
/// or after the queue empties.
pub struct DirStore {
    dir: PathBuf,
    /// Held across a position's reading and writing.
    positions: Mutex<()>,
}

/// The parts a file's name carries.
struct Name {
    position: u64,
    n: u64,
    arrival: u64,
    delivered_at: Option<u64>,
}

impl Name {
    fn render(&self) -> String {
        match self.delivered_at {
            Some(d) => format!("{}-{}-{}-{d}", self.position, self.n, self.arrival),
            None => format!("{}-{}-{}", self.position, self.n, self.arrival),
        }
    }

    fn parse(name: &str) -> Option<Name> {
        let parts: Vec<u64> = name
            .split('-')
            .map(|p| p.parse::<u64>().ok())
            .collect::<Option<_>>()?;
        match parts.as_slice() {
            [p, n, a] => Some(Name {
                position: *p,
                n: *n,
                arrival: *a,
                delivered_at: None,
            }),
            [p, n, a, d] => Some(Name {
                position: *p,
                n: *n,
                arrival: *a,
                delivered_at: Some(*d),
            }),
            _ => None,
        }
    }
}

impl DirStore {
    /// A store writing one file per queued message under `dir`.
    pub fn new(dir: impl Into<PathBuf>) -> Self {
        let dir = dir.into();
        std::fs::create_dir_all(&dir).expect("queue directory");
        DirStore {
            dir,
            positions: Mutex::new(()),
        }
    }

    fn recipient_dir(&self, r: &[u8; 32]) -> PathBuf {
        self.dir
            .join(r.iter().map(|b| format!("{b:02x}")).collect::<String>())
    }

    fn recipient_of(name: &str) -> Option<[u8; 32]> {
        if name.len() != 64 {
            return None;
        }
        let mut out = [0u8; 32];
        for (i, b) in out.iter_mut().enumerate() {
            *b = u8::from_str_radix(&name[i * 2..i * 2 + 2], 16).ok()?;
        }
        Some(out)
    }

    /// Files for one recipient, in ascending position.
    fn files(&self, r: &[u8; 32]) -> Vec<(Name, PathBuf)> {
        let mut out = Vec::new();
        if let Ok(rd) = std::fs::read_dir(self.recipient_dir(r)) {
            for e in rd.flatten() {
                if let Some(n) = Name::parse(&e.file_name().to_string_lossy()) {
                    out.push((n, e.path()));
                }
            }
        }
        out.sort_by_key(|(n, _)| n.position);
        out
    }

    fn file_at(&self, r: &[u8; 32], position: u64) -> Option<(Name, PathBuf)> {
        self.files(r)
            .into_iter()
            .find(|(n, _)| n.position == position)
    }

    /// Every message file under the store, for a test that checks nothing
    /// outlives a removal.
    pub fn all_files(&self) -> Vec<PathBuf> {
        let mut out = Vec::new();
        if let Ok(rd) = std::fs::read_dir(&self.dir) {
            for d in rd.flatten() {
                if let Ok(inner) = std::fs::read_dir(d.path()) {
                    out.extend(
                        inner
                            .flatten()
                            .filter(|e| Name::parse(&e.file_name().to_string_lossy()).is_some())
                            .map(|e| e.path()),
                    );
                }
            }
        }
        out
    }
}

/// Flush a directory's own entry, so a file just created or renamed in it
/// survives a crash and not only the file's contents (`fsync` of a file
/// does not durably record that the file exists in its directory).
fn sync_dir(dir: &Path) -> std::io::Result<()> {
    std::fs::File::open(dir)?.sync_all()
}

/// Write `bytes` to a new file at `path`, flushed, and its directory.
fn create(path: &Path, bytes: &[u8]) -> bool {
    use std::io::Write;
    let Ok(mut f) = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
    else {
        return false;
    };
    // written AND flushed before this returns true: an item counts as
    // queued once it is on the disk, not once it is in a buffer the
    // process may not outlive
    if f.write_all(bytes).and_then(|()| f.sync_all()).is_err() {
        // a half-written file would be read back and delivered as a
        // garbage message: leave none
        let _ = std::fs::remove_file(path);
        return false;
    }
    path.parent().is_some_and(|d| sync_dir(d).is_ok())
}

impl QueueStore for DirStore {
    fn next_position(&self, recipient: &[u8; 32]) -> u64 {
        let _held = self.positions.lock().unwrap();
        let dir = self.recipient_dir(recipient);
        let _ = std::fs::create_dir_all(&dir);
        let high = dir.join("high");
        let last = std::fs::read_to_string(&high)
            .ok()
            .and_then(|s| s.trim().parse::<u64>().ok())
            .unwrap_or(0)
            // a store whose mark was lost resumes past what it holds
            .max(
                self.files(recipient)
                    .last()
                    .map(|(n, _)| n.position)
                    .unwrap_or(0),
            );
        let next = last + 1;
        let tmp = dir.join("high.tmp");
        if std::fs::write(&tmp, next.to_string()).is_ok() && std::fs::rename(&tmp, &high).is_ok() {
            let _ = sync_dir(&dir);
        }
        next
    }

    /// **A file per item, so a block is what an item costs** (design
    /// §14.1.6 as restated [author, 2026-11-06]: the cap means the disk
    /// consumed).
    ///
    /// **4 KiB, assumed rather than asked.** It is the block size of every
    /// filesystem a node is plausibly deployed on — ext4, xfs and btrfs all
    /// default to it — and asking the kernel would mean `statvfs` and a
    /// libc dependency this crate does not have.
    fn block(&self) -> usize {
        4096
    }

    fn push(&self, item: Queued) -> bool {
        let dir = self.recipient_dir(&item.recipient);
        if std::fs::create_dir_all(&dir).is_err() {
            return false;
        }
        let name = Name {
            position: item.position,
            n: item.n,
            arrival: item.arrival,
            delivered_at: item.delivered_at,
        };
        create(&dir.join(name.render()), &item.sealed)
    }

    fn list(&self, recipient: &[u8; 32]) -> Vec<Queued> {
        self.files(recipient)
            .into_iter()
            .filter_map(|(n, path)| {
                std::fs::read(&path).ok().map(|sealed| Queued {
                    sealed,
                    recipient: *recipient,
                    n: n.n,
                    position: n.position,
                    arrival: n.arrival,
                    delivered_at: n.delivered_at,
                })
            })
            .collect()
    }

    fn recipients(&self) -> Vec<[u8; 32]> {
        let Ok(rd) = std::fs::read_dir(&self.dir) else {
            return Vec::new();
        };
        rd.flatten()
            .filter_map(|d| Self::recipient_of(&d.file_name().to_string_lossy()))
            .filter(|r| !self.files(r).is_empty())
            .collect()
    }

    fn mark_delivered(&self, recipient: &[u8; 32], position: u64, at: u64) -> bool {
        let Some((mut name, path)) = self.file_at(recipient, position) else {
            return false;
        };
        if name.delivered_at.is_some() {
            return false;
        }
        name.delivered_at = Some(at);
        let dir = self.recipient_dir(recipient);
        let renamed = std::fs::rename(&path, dir.join(name.render())).is_ok();
        if renamed {
            let _ = sync_dir(&dir);
        }
        renamed
    }

    fn remove(&self, recipient: &[u8; 32], position: u64) -> bool {
        // the file alone: the recipient's directory stays, holding the
        // high mark, since positions are never reused
        match self.file_at(recipient, position) {
            Some((_, path)) => std::fs::remove_file(path).is_ok(),
            None => false,
        }
    }

    fn drop_all(&self, recipient: &[u8; 32]) {
        for (_, path) in self.files(recipient) {
            let _ = std::fs::remove_file(path);
        }
    }
}
