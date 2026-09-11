//! The prekey service (`wire-format.md` §7.8; `infra-client-requirements.md`
//! §6; design §14.2.2, §14.2.4.5): a serving node holds the bundle and the
//! one-time pool of each client it serves, serves the reusable material to
//! anyone any number of times, consumes a one-time key only when one is
//! requested and rate-limits that per requester per subject, tells the
//! subject when its pool runs dry, reads nothing of the bundle it holds,
//! and keeps no record of who asked for whose.

use crate::Keyhash;
pub use rhtn_archive::prekey::*;
use rhtn_crypto::verify::Lookup;
use std::collections::{BTreeMap, VecDeque};

/// The service's own numbers: one-time keys per requester per subject per
/// window (`infra-client-requirements.md` §6).
#[derive(Debug, Clone)]
pub struct PrekeyConfig {
    pub one_time_per_requester_per_subject: u32,
    pub window_s: u64,
}

impl Default for PrekeyConfig {
    fn default() -> Self {
        PrekeyConfig { one_time_per_requester_per_subject: 4, window_s: 3600 }
    }
}

/// The bundles and pools this node holds, by subject.  The rate-limit
/// counters are the one thing here that names a requester: they live in
/// memory for one window and are never written anywhere.
#[derive(Debug, Default)]
pub struct PrekeyService {
    pub cfg: PrekeyConfig,
    bundles: BTreeMap<Keyhash, Vec<u8>>,
    pools: BTreeMap<Keyhash, VecDeque<Vec<u8>>>,
    issued: BTreeMap<(Keyhash, Keyhash), (u64, u32)>,
    exhausted: Vec<Keyhash>,
}

impl PrekeyService {
    pub fn new(cfg: PrekeyConfig) -> Self {
        PrekeyService { cfg, ..Default::default() }
    }

    /// Hold a bundle a client publishes: it must verify under the subject
    /// it names, and nothing of its blob is read.
    pub fn publish<L: Lookup + ?Sized>(&mut self, ids: &L, bytes: &[u8]) -> Result<Keyhash, String> {
        let b = PrekeyBundle::parse(bytes)?;
        if !b.verify(ids)? {
            return Err("bundle signature fails".into());
        }
        self.bundles.insert(b.subject, bytes.to_vec());
        Ok(b.subject)
    }

    /// Add one-time keys to a subject's pool, as uploaded.
    pub fn stock(&mut self, subject: Keyhash, keys: Vec<Vec<u8>>) {
        self.pools.entry(subject).or_default().extend(keys);
    }

    pub fn bundle(&self, subject: &Keyhash) -> Option<&Vec<u8>> {
        self.bundles.get(subject)
    }

    pub fn pool_size(&self, subject: &Keyhash) -> usize {
        self.pools.get(subject).map(|p| p.len()).unwrap_or(0)
    }

    pub fn subjects(&self) -> Vec<Keyhash> {
        self.bundles.keys().copied().collect()
    }

    /// Answer a request from `requester` at `now`: the reply bytes, or
    /// nothing where the body is not a request.
    pub fn answer(&mut self, requester: &Keyhash, body: &[u8], now: u64) -> Option<Vec<u8>> {
        match PrekeyRequest::decode(body).ok()? {
            PrekeyRequest::One { subject, one_time, nonce } => Some(self.answer_one(requester, &subject, one_time, nonce, now).encode()),
            PrekeyRequest::Batch { subjects, nonce } => {
                // a sweep: reusable material only, whatever the pools hold
                let replies: Vec<PrekeyReply> = subjects.iter().map(|s| self.reusable(s, nonce)).collect();
                Some(encode_batch_reply(&replies))
            }
        }
    }

    fn reusable(&self, subject: &Keyhash, nonce: [u8; 16]) -> PrekeyReply {
        match self.bundles.get(subject) {
            Some(b) => PrekeyReply { nonce, bundle: Some(b.clone()), one_time: None, code: None },
            None => PrekeyReply { nonce, bundle: None, one_time: None, code: Some(FAIL_UNKNOWN_SUBJECT) },
        }
    }

    fn answer_one(&mut self, requester: &Keyhash, subject: &Keyhash, one_time: bool, nonce: [u8; 16], now: u64) -> PrekeyReply {
        let mut reply = self.reusable(subject, nonce);
        if reply.bundle.is_none() || !one_time {
            return reply;
        }
        // the allowance: within it a key is consumed; over it the reusable
        // material is served and nothing is spent
        let e = self.issued.entry((*requester, *subject)).or_insert((now, 0));
        if now.saturating_sub(e.0) >= self.cfg.window_s {
            *e = (now, 0);
        }
        if e.1 >= self.cfg.one_time_per_requester_per_subject {
            return reply;
        }
        if let Some(pool) = self.pools.get_mut(subject)
            && let Some(key) = pool.pop_front() {
                e.1 += 1;
                reply.one_time = Some(key);
                if pool.is_empty() {
                    self.exhausted.push(*subject);
                }
            }
        reply
    }

    /// The subjects whose pools were drained since last asked: what this
    /// node tells each of them.  No wire object carries it; the session
    /// does.
    pub fn take_exhausted(&mut self) -> Vec<Keyhash> {
        std::mem::take(&mut self.exhausted)
    }

    /// Drop rate-limit windows that have closed.
    pub fn expire(&mut self, now: u64) {
        let w = self.cfg.window_s;
        self.issued.retain(|_, (opened, _)| now.saturating_sub(*opened) < w);
    }

    /// Persist the bundles and pools, and nothing else: no requester and
    /// no request is written (`infra-client-requirements.md` §6).
    pub fn save(&self, dir: &std::path::Path) -> std::io::Result<()> {
        let root = dir.join("prekeys");
        if root.exists() {
            std::fs::remove_dir_all(&root)?;
        }
        for (subject, bundle) in &self.bundles {
            let d = root.join(hex(subject));
            std::fs::create_dir_all(&d)?;
            std::fs::write(d.join("bundle"), bundle)?;
            for (i, k) in self.pools.get(subject).map(|p| p.iter().collect::<Vec<_>>()).unwrap_or_default().iter().enumerate() {
                std::fs::write(d.join(format!("otk-{i:06}")), k)?;
            }
        }
        Ok(())
    }

    pub fn load(dir: &std::path::Path, cfg: PrekeyConfig) -> std::io::Result<PrekeyService> {
        let mut s = PrekeyService::new(cfg);
        let root = dir.join("prekeys");
        let Ok(rd) = std::fs::read_dir(&root) else { return Ok(s) };
        for e in rd.flatten() {
            let Ok(bundle) = std::fs::read(e.path().join("bundle")) else { continue };
            let Ok(b) = PrekeyBundle::parse(&bundle) else { continue };
            s.bundles.insert(b.subject, bundle);
            let mut keys: Vec<(String, Vec<u8>)> = Vec::new();
            if let Ok(inner) = std::fs::read_dir(e.path()) {
                for f in inner.flatten() {
                    let name = f.file_name().to_string_lossy().to_string();
                    if name.starts_with("otk-")
                        && let Ok(k) = std::fs::read(f.path()) {
                            keys.push((name, k));
                        }
                }
            }
            keys.sort();
            s.pools.insert(b.subject, keys.into_iter().map(|(_, k)| k).collect());
        }
        Ok(s)
    }
}

fn hex(k: &Keyhash) -> String {
    k.iter().map(|b| format!("{b:02x}")).collect()
}
