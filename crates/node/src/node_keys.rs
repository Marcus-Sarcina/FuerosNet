//! The node-key service (`wire-format.md` §7.8; `infra-client-requirements.md`
//! §6; design §14.2.4.3): a serving node keeps a rotating X-Wing key for
//! each identity it serves, signs and serves its public half to whoever
//! asks, accepts a message sealed to the newest key or, for a grace after
//! a rotation, the one before, opens its own half of each message at
//! delivery, and **holds a private half while any message sealed to it is
//! queued**, deleting it when the last one is removed.  It keeps no record
//! of who asked for whose key: a fetch arrives under a key minted for it
//! and names nobody.

use crate::Keyhash;
pub use rhtn_archive::node_key::*;
use rhtn_crypto::node_key::{self, NodeKeyFields};
use rhtn_crypto::signer::Sign1;
use rhtn_crypto::xwing;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use zeroize::Zeroizing;

/// The service's own numbers, each the node's to change (design §21.1.1).
#[derive(Debug, Clone)]
pub struct NodeKeyConfig {
    /// How often a recipient's key is replaced, in seconds: **one day** as
    /// shipped [author, 2026-11-19].
    pub rotate_secs: u64,
    /// How long after a rotation the key before is still accepted, in
    /// seconds: **thirty**, generous network latency [author, 2026-11-19].
    pub grace_secs: u64,
}

impl Default for NodeKeyConfig {
    fn default() -> Self {
        NodeKeyConfig {
            rotate_secs: 86_400,
            grace_secs: 30,
        }
    }
}

/// One key held for one recipient.
struct Held {
    fields: NodeKeyFields,
    /// The X-Wing seed: the private half.
    seed: Zeroizing<[u8; xwing::SECRET_BYTES]>,
    /// The signed record, as served.
    record: Vec<u8>,
    /// When its successor was published, which starts its grace.
    retired_at: Option<u64>,
}

/// The keys this node holds, by recipient, oldest first.
#[derive(Default)]
pub struct NodeKeyService {
    /// What this service was configured with.
    pub cfg: NodeKeyConfig,
    keys: BTreeMap<Keyhash, Vec<Held>>,
    /// Where the keys are kept, once an owner has said. **A key is written
    /// when it is made and unlinked when it goes**, so a stop between
    /// snapshots neither loses one a queued message needs nor brings back
    /// one deleted. Absent, the keys live in memory alone.
    dir: Option<PathBuf>,
}

impl std::fmt::Debug for NodeKeyService {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "NodeKeyService({} recipients)", self.keys.len())
    }
}

fn hex(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect()
}

fn unhex32(s: &str) -> Option<[u8; 32]> {
    if s.len() != 64 {
        return None;
    }
    let mut out = [0u8; 32];
    for (i, b) in out.iter_mut().enumerate() {
        *b = u8::from_str_radix(&s[i * 2..i * 2 + 2], 16).ok()?;
    }
    Some(out)
}

impl NodeKeyService {
    /// A service with `cfg` and no keys.
    pub fn new(cfg: NodeKeyConfig) -> Self {
        NodeKeyService {
            cfg,
            ..Default::default()
        }
    }

    /// The service kept at `dir`: what is there is loaded, and from here
    /// on a key is written as it is made and unlinked as it goes.
    pub fn at(dir: &Path, cfg: NodeKeyConfig) -> std::io::Result<NodeKeyService> {
        let mut s = NodeKeyService::new(cfg);
        let root = dir.join("node-keys");
        // **a service that cannot keep its keys does not start** [review,
        // 2026-11-25]: a key published and never written is one whose
        // queued messages a restart leaves unopenable
        std::fs::create_dir_all(&root)?;
        if let Ok(rd) = std::fs::read_dir(&root) {
            for e in rd.flatten() {
                let Some(recipient) = unhex32(&e.file_name().to_string_lossy()) else {
                    continue;
                };
                let Ok(files) = std::fs::read_dir(e.path()) else {
                    continue;
                };
                let mut held = Vec::new();
                for f in files.flatten() {
                    if let Some(h) = std::fs::read(f.path()).ok().and_then(|b| read_held(&b)) {
                        held.push(h);
                    }
                }
                held.sort_by_key(|h| h.fields.n);
                if !held.is_empty() {
                    s.keys.insert(recipient, held);
                }
            }
        }
        s.dir = Some(dir.to_path_buf());
        Ok(s)
    }

    fn path(&self, recipient: &Keyhash, n: u64) -> Option<PathBuf> {
        self.dir
            .as_ref()
            .map(|d| d.join("node-keys").join(hex(recipient)).join(n.to_string()))
    }

    /// Write one key where this service keeps them: done where it keeps
    /// none on disk, and an error where it does and could not.
    fn write(&self, recipient: &Keyhash, h: &Held) -> std::io::Result<()> {
        let Some(p) = self.path(recipient, h.fields.n) else {
            return Ok(());
        };
        if let Some(parent) = p.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&p, write_held(h))
    }

    fn unlink(&self, recipient: &Keyhash, n: u64) {
        if let Some(p) = self.path(recipient, n) {
            let _ = std::fs::remove_file(&p);
            if let Some(parent) = p.parent() {
                // the recipient's directory goes with its last key
                let _ = std::fs::remove_dir(parent);
            }
        }
    }

    /// **The newest key for `recipient`**, made where none is held and
    /// replaced where the newest is a rotation old; the one replaced is
    /// retired, accepted for its grace and kept while anything sealed to
    /// it is queued. Signed by `node`.
    ///
    /// For a caller that keeps its keys in memory, or holds that a write
    /// here cannot fail; one serving a request asks [`try_newest`].
    ///
    /// [`try_newest`]: NodeKeyService::try_newest
    pub fn newest(&mut self, node: &dyn Sign1, recipient: &Keyhash, now: u64) -> Vec<u8> {
        self.try_newest(node, recipient, now)
            .expect("a node key is written where it is kept")
    }

    /// **The same, or why no key is published**: a key this service could
    /// not write where it keeps them is not held and not served [review,
    /// 2026-11-25], since a sender would seal to it, the message would be
    /// queued, and a restart would leave it unopenable. The key before is
    /// as it was.
    pub fn try_newest(
        &mut self,
        node: &dyn Sign1,
        recipient: &Keyhash,
        now: u64,
    ) -> std::io::Result<Vec<u8>> {
        let due = self
            .keys
            .get(recipient)
            .and_then(|k| k.last())
            .is_none_or(|h| now.saturating_sub(h.fields.published_at) >= self.cfg.rotate_secs);
        if due {
            let n = self
                .keys
                .get(recipient)
                .and_then(|k| k.last())
                .map_or(1, |h| h.fields.n + 1);
            let seed =
                Zeroizing::new(rhtn_transport::tls::random_bytes::<{ xwing::SECRET_BYTES }>());
            let secret = xwing::Secret::from_seed(*seed);
            let fields = NodeKeyFields {
                recipient: *recipient,
                node: node.keyhash(),
                n,
                public: *secret.public(),
                published_at: now,
            };
            let record = node_key::issue(node, &fields);
            let held = Held {
                fields,
                seed,
                record,
                retired_at: None,
            };
            // written before it is held, so what cannot be kept is never
            // published
            self.write(recipient, &held)?;
            let keys = self.keys.entry(*recipient).or_default();
            if let Some(prev) = keys.last_mut() {
                prev.retired_at = Some(now);
            }
            keys.push(held);
            if let Some(prev) = self.keys[recipient].iter().rev().nth(1) {
                // the retirement is the grace's start: one not written is
                // a key accepted for longer after a restart, never one lost
                let _ = self.write(recipient, prev);
            }
        }
        Ok(self.keys[recipient]
            .last()
            .expect("one is held")
            .record
            .clone())
    }

    /// **Answer a request** (`wire-format.md` §7.8): the newest key for
    /// each recipient this node serves, a failure code for each it does
    /// not, in the request's own order. Nothing about the asker is kept.
    pub fn answer(
        &mut self,
        node: &dyn Sign1,
        serves: &dyn Fn(&Keyhash) -> bool,
        body: &[u8],
        now: u64,
    ) -> Option<Vec<u8>> {
        let req = NodeKeyRequest::decode(body).ok()?;
        let nonce = req.nonce();
        let replies: Vec<NodeKeyReply> = req
            .recipients()
            .iter()
            .map(|r| NodeKeyReply {
                nonce,
                outcome: if serves(r) {
                    self.try_newest(node, r, now).map_err(|why| {
                        tracing::warn!(
                            target: "node",
                            recipient8 = %crate::diag::id8(r),
                            %why,
                            "node.key.unwritten"
                        );
                        FAIL_REFUSED
                    })
                } else {
                    Err(FAIL_NOT_SERVED)
                },
            })
            .collect();
        Some(match req {
            NodeKeyRequest::One { .. } => replies[0].encode(),
            NodeKeyRequest::Batch { .. } => encode_batch_reply(&replies),
        })
    }

    /// **Whether a message sealed to key `n` is taken** (design
    /// §14.2.4.3): the newest within its day, or the one before within its
    /// grace.
    ///
    /// **The newest is not accepted for ever** [review, 2026-11-25]. A key
    /// is replaced when somebody asks for one after its day has run, so a
    /// recipient nobody wrote to kept one key for as long as nobody did,
    /// and a message sealed to it could be put into the queue months
    /// later — where §14.2.4.3 has a rotation ensure "a sealed message
    /// cannot be replayed into the queue past its key's day". So the
    /// newest is taken for its rotation interval and the grace after it,
    /// and a sender refused fetches the key that asking then makes.
    pub fn accepts(&self, recipient: &Keyhash, n: u64, now: u64) -> bool {
        let Some(keys) = self.keys.get(recipient) else {
            return false;
        };
        let Some(newest) = keys.last() else {
            return false;
        };
        if newest.fields.n == n {
            return now.saturating_sub(newest.fields.published_at)
                < self.cfg.rotate_secs.saturating_add(self.cfg.grace_secs);
        }
        keys.iter().rev().nth(1).is_some_and(|prev| {
            prev.fields.n == n
                && prev
                    .retired_at
                    .is_some_and(|at| now.saturating_sub(at) < self.cfg.grace_secs)
        })
    }

    /// **The node's half of one message**: the secret `ct_n` carries,
    /// decapsulated under key `n`, or nothing where that key is not held.
    pub fn open_half(
        &self,
        recipient: &Keyhash,
        n: u64,
        ct_n: &[u8; xwing::CIPHERTEXT_BYTES],
    ) -> Option<Zeroizing<[u8; 32]>> {
        let h = self.keys.get(recipient)?.iter().find(|h| h.fields.n == n)?;
        Some(xwing::Secret::from_seed(*h.seed).decapsulate(ct_n))
    }

    /// **Delete what nothing needs** (design §14.2.4.3): every key for
    /// `recipient` but the newest whose grace has run and to which no
    /// queued message is sealed, `queued` saying which numbers still are.
    /// The private half goes immediately and leaves nothing recoverable.
    pub fn prune(&mut self, recipient: &Keyhash, queued: &dyn Fn(u64) -> bool, now: u64) -> usize {
        let grace = self.cfg.grace_secs;
        let Some(keys) = self.keys.get_mut(recipient) else {
            return 0;
        };
        let newest = keys.last().map(|h| h.fields.n);
        let mut gone = Vec::new();
        keys.retain(|h| {
            let keep = Some(h.fields.n) == newest
                || queued(h.fields.n)
                || h.retired_at.is_none_or(|at| now.saturating_sub(at) < grace);
            if !keep {
                gone.push(h.fields.n);
            }
            keep
        });
        for n in &gone {
            self.unlink(recipient, *n);
        }
        gone.len()
    }

    /// Forget every key held for `recipient`: the relationship has ended.
    pub fn forget(&mut self, recipient: &Keyhash) {
        if let Some(keys) = self.keys.remove(recipient) {
            for h in keys {
                self.unlink(recipient, h.fields.n);
            }
        }
    }

    /// Every recipient a key is held for.
    pub fn recipients(&self) -> Vec<Keyhash> {
        self.keys.keys().copied().collect()
    }

    /// The numbers of the keys held for `recipient`, oldest first.
    pub fn held(&self, recipient: &Keyhash) -> Vec<u64> {
        self.keys
            .get(recipient)
            .map(|k| k.iter().map(|h| h.fields.n).collect())
            .unwrap_or_default()
    }
}

/// `seed ‖ retired_at (u64 BE, 0 for none) ‖ record`: what a key is kept as.
fn write_held(h: &Held) -> Zeroizing<Vec<u8>> {
    let mut out = Zeroizing::new(Vec::with_capacity(40 + h.record.len()));
    out.extend_from_slice(&h.seed[..]);
    out.extend_from_slice(&h.retired_at.unwrap_or(0).to_be_bytes());
    out.extend_from_slice(&h.record);
    out
}

fn read_held(b: &[u8]) -> Option<Held> {
    if b.len() < 40 {
        return None;
    }
    let seed = Zeroizing::new(<[u8; 32]>::try_from(&b[..32]).ok()?);
    let retired = u64::from_be_bytes(b[32..40].try_into().ok()?);
    let record = b[40..].to_vec();
    let fields = NodeKeyFields::read(&record)?;
    Some(Held {
        fields,
        seed,
        record,
        retired_at: (retired != 0).then_some(retired),
    })
}
