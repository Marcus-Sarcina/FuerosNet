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
    /// Each key under the name it is kept at, so serving one can unlink it.
    pools: BTreeMap<Keyhash, VecDeque<(String, Vec<u8>)>>,
    issued: BTreeMap<(Keyhash, Keyhash), (u64, u32)>,
    exhausted: Vec<Keyhash>,
    /// Where the pools are kept, once an owner has said.
    ///
    /// **A one-time key is spent on disk before its reply goes out.** It
    /// is served once and never again (`wire-format.md` §7.8), and a
    /// snapshot taken later cannot carry that: a stop between snapshots
    /// would bring a served key back, and a second initiator would be
    /// handed material whose private half was already consumed.  Absent,
    /// nothing is written and the pools live in memory alone, which is
    /// what a harness wants.
    dir: Option<std::path::PathBuf>,
    /// The next name to keep a key under.  Names only have to be distinct
    /// and to sort in the order the keys were stocked.
    seq: u64,
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
        // written through where the service is kept, for the same reason
        // the keys are: what is held and what is stored never differ, and
        // a pool whose bundle is missing is a subject a restart drops
        if let Some(dir) = &self.dir {
            let d = dir.join("prekeys").join(hex(&b.subject));
            std::fs::create_dir_all(&d).and_then(|_| std::fs::write(d.join("bundle"), bytes)).map_err(|e| e.to_string())?;
        }
        self.bundles.insert(b.subject, bytes.to_vec());
        Ok(b.subject)
    }

    /// Add one-time keys to a subject's pool, as uploaded.  Where the
    /// service is kept on disk each is written as it arrives, so what is
    /// held and what is stored never differ.
    pub fn stock(&mut self, subject: Keyhash, keys: Vec<Vec<u8>>) {
        for k in keys {
            let name = format!("otk-{:012}", self.seq);
            self.seq += 1;
            if let Some(dir) = &self.dir {
                let d = dir.join("prekeys").join(hex(&subject));
                if std::fs::create_dir_all(&d).and_then(|_| std::fs::write(d.join(&name), &k)).is_err() {
                    continue;
                }
            }
            self.pools.entry(subject).or_default().push_back((name, k));
        }
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
        let taken = self.pools.get_mut(subject).and_then(|p| p.pop_front());
        if let Some((name, key)) = taken {
            // spent on disk first: a key whose file cannot be removed is
            // not served at all, since serving it would leave it able to
            // come back
            if !self.forget(subject, &name) {
                self.pools.entry(*subject).or_default().push_front((name, key));
                return reply;
            }
            let e = self.issued.entry((*requester, *subject)).or_insert((now, 0));
            e.1 += 1;
            reply.one_time = Some(key);
            // the allowance is spent with the key: a counter kept only in
            // memory gives a requester a fresh window every time the
            // process stops, and a kernel that wakes and sleeps stops
            // often enough for the bound to stop binding
            self.write_counters();
            if self.pools.get(subject).is_none_or(|p| p.is_empty()) {
                self.exhausted.push(*subject);
            }
        }
        reply
    }

    /// Write the allowance counters where the service is kept.  One small
    /// file beside the pools, rewritten as a key is spent, which already
    /// costs an unlink.
    ///
    /// A failure here is not a refusal: the key has been served and the
    /// count is an upper bound the window closes anyway, so losing it
    /// grants at most one window's worth (`wire-format.md` §7.8).
    fn write_counters(&self) {
        let Some(dir) = &self.dir else { return };
        let mut out = String::new();
        for ((requester, subject), (opened, count)) in &self.issued {
            out.push_str(&format!("{} {} {opened} {count}\n", hex(requester), hex(subject)));
        }
        let root = dir.join("prekeys");
        if std::fs::create_dir_all(&root).is_ok() {
            let _ = std::fs::write(root.join("issued"), out);
        }
    }

    fn read_counters(&mut self, dir: &std::path::Path) {
        let Ok(text) = std::fs::read_to_string(dir.join("prekeys").join("issued")) else { return };
        for line in text.lines() {
            let f: Vec<&str> = line.split_whitespace().collect();
            let [r, s, opened, count] = f.as_slice() else { continue };
            if let (Some(r), Some(s), Ok(opened), Ok(count)) = (unhex(r), unhex(s), opened.parse(), count.parse()) {
                self.issued.insert((r, s), (opened, count));
            }
        }
    }

    /// Unlink the key kept under `name`, where this service is kept on
    /// disk.  Whether it is gone: a service with no directory has nothing
    /// to remove and nothing can resurrect it.
    fn forget(&self, subject: &Keyhash, name: &str) -> bool {
        let Some(dir) = &self.dir else { return true };
        let p = dir.join("prekeys").join(hex(subject)).join(name);
        match std::fs::remove_file(&p) {
            Ok(()) => true,
            Err(e) => e.kind() == std::io::ErrorKind::NotFound,
        }
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

    /// Make the directory match what is held: write what is missing and
    /// remove what is not held.  No requester and no request is written
    /// (`infra-client-requirements.md` §6).
    ///
    /// **One rule, whichever way the service is kept.** A service bound to
    /// a directory already has disk and memory in step, so this removes
    /// nothing; a detached one is snapshotted, and a key it served is gone
    /// from the directory because it is gone from the pool.  Adding
    /// without removing would leave a served key on disk for the next load
    /// to hand out again, and wiping the directory first would open a
    /// window in which a stop loses everything.
    pub fn save(&self, dir: &std::path::Path) -> std::io::Result<()> {
        let root = dir.join("prekeys");
        for (subject, bundle) in &self.bundles {
            let d = root.join(hex(subject));
            std::fs::create_dir_all(&d)?;
            std::fs::write(d.join("bundle"), bundle)?;
            let held: std::collections::BTreeSet<&str> = self.pools.get(subject).into_iter().flatten().map(|(n, _)| n.as_str()).collect();
            for (name, k) in self.pools.get(subject).into_iter().flatten() {
                if !d.join(name).exists() {
                    std::fs::write(d.join(name), k)?;
                }
            }
            // a key this service no longer holds is one it served: it does
            // not survive the snapshot that follows
            if let Ok(rd) = std::fs::read_dir(&d) {
                for f in rd.flatten() {
                    let name = f.file_name().to_string_lossy().to_string();
                    if name.starts_with("otk-") && !held.contains(name.as_str()) {
                        std::fs::remove_file(f.path())?;
                    }
                }
            }
        }
        // and a subject whose bundle is gone is gone: a pool without one is
        // a subject `load` skips anyway
        if let Ok(rd) = std::fs::read_dir(&root) {
            let kept: std::collections::BTreeSet<String> = self.bundles.keys().map(hex).collect();
            for e in rd.flatten() {
                if !kept.contains(&e.file_name().to_string_lossy().to_string()) {
                    std::fs::remove_dir_all(e.path())?;
                }
            }
        }
        Ok(())
    }

    /// The service kept at `dir`: what is there is loaded, and from here
    /// on a key is written as it is stocked and unlinked as it is served.
    /// A directory that does not exist yet is an empty service kept there.
    pub fn at(dir: &std::path::Path, cfg: PrekeyConfig) -> std::io::Result<PrekeyService> {
        let mut s = PrekeyService::load(dir, cfg)?;
        s.read_counters(dir);
        s.dir = Some(dir.to_path_buf());
        Ok(s)
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
            // names carry the order the keys were stocked in, and the
            // counter resumes past the highest so a new key never takes a
            // name a served one had
            for (name, _) in &keys {
                if let Some(n) = name.strip_prefix("otk-").and_then(|n| n.parse::<u64>().ok()) {
                    s.seq = s.seq.max(n + 1);
                }
            }
            s.pools.insert(b.subject, keys.into_iter().collect());
        }
        Ok(s)
    }
}

fn unhex(s: &str) -> Option<Keyhash> {
    if s.len() != 64 {
        return None;
    }
    let mut out = [0u8; 32];
    for (i, b) in out.iter_mut().enumerate() {
        *b = u8::from_str_radix(s.get(i * 2..i * 2 + 2)?, 16).ok()?;
    }
    Some(out)
}

fn hex(k: &Keyhash) -> String {
    k.iter().map(|b| format!("{b:02x}")).collect()
}
