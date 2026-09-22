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
    /// How many one-time keys a client may leave in one deposit, and how
    /// large a pool this node lends it (`wire-format.md` §7.10).  The wire
    /// bounds a deposit at 256; an operator may lend less, and says so
    /// with the reply's code rather than trimming a deposit silently.
    pub per_deposit: usize,
    pub pool: usize,
}

impl Default for PrekeyConfig {
    fn default() -> Self {
        PrekeyConfig {
            one_time_per_requester_per_subject: 4,
            window_s: 3600,
            per_deposit: 256,
            pool: 1024,
        }
    }
}

/// A subject's device: the key it presents (`wire-format.md` §7.8).  A
/// node holds one bundle and one pool per device.
pub type Slot = (Keyhash, [u8; 32]);

/// The device a pool stocked with no device named belongs to: drawn from
/// for any of the subject's devices.
pub const ANY_DEVICE: [u8; 32] = rhtn_transport::queue::ANY_DEVICE;

/// The bundles and pools this node holds, by subject and device.  The
/// rate-limit counters are the one thing here that names a requester:
/// they live in memory for one window and are never written anywhere.
#[derive(Debug, Default)]
pub struct PrekeyService {
    pub cfg: PrekeyConfig,
    bundles: BTreeMap<Slot, Vec<u8>>,
    /// Each key under the name it is kept at, so serving one can unlink it.
    pools: BTreeMap<Slot, VecDeque<(String, Vec<u8>)>>,
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
        PrekeyService {
            cfg,
            ..Default::default()
        }
    }

    /// Hold a bundle a client publishes: it must verify under the subject
    /// it names, and nothing of its blob is read.
    pub fn publish<L: Lookup + ?Sized>(
        &mut self,
        ids: &L,
        bytes: &[u8],
    ) -> Result<Keyhash, String> {
        let b = PrekeyBundle::parse(bytes)?;
        if b.verify(ids).is_err() {
            return Err("bundle signature fails".into());
        }
        // written through where the service is kept, for the same reason
        // the keys are: what is held and what is stored never differ, and
        // a pool whose bundle is missing is a subject a restart drops
        if let Some(dir) = &self.dir {
            let d = slot_dir(dir, &(b.subject, b.device));
            std::fs::create_dir_all(&d)
                .and_then(|_| std::fs::write(d.join("bundle"), bytes))
                .map_err(|e| e.to_string())?;
        }
        self.bundles.insert((b.subject, b.device), bytes.to_vec());
        Ok(b.subject)
    }

    /// Add one-time keys to a subject's pool, as uploaded.  Where the
    /// service is kept on disk each is written as it arrives, so what is
    /// held and what is stored never differ.
    ///
    /// **All of them or none of them**, and the caller is told which
    /// (`wire-format.md` §7.10).  A deposit half written would have the
    /// node answer *accepted* for a pool it did not fill, and a client
    /// that believed the answer would stop replenishing.  What was
    /// written before the failure is unlinked, so the pool and the reply
    /// agree either way.
    #[must_use]
    pub fn stock(&mut self, subject: Keyhash, keys: Vec<Vec<u8>>) -> bool {
        self.stock_for(subject, ANY_DEVICE, keys)
    }

    /// `stock` into the pool of one of the subject's devices: a one-time
    /// key is a device's, since the device holds its private half
    /// (`wire-format.md` §7.8).
    #[must_use]
    pub fn stock_for(&mut self, subject: Keyhash, device: [u8; 32], keys: Vec<Vec<u8>>) -> bool {
        let slot = (subject, device);
        let mut written: Vec<(String, Vec<u8>)> = Vec::with_capacity(keys.len());
        for k in keys {
            let name = format!("otk-{:012}", self.seq);
            self.seq += 1;
            if let Some(dir) = &self.dir {
                let d = slot_dir(dir, &slot);
                if std::fs::create_dir_all(&d)
                    .and_then(|_| std::fs::write(d.join(&name), &k))
                    .is_err()
                {
                    for (n, _) in &written {
                        self.forget(&slot, n);
                    }
                    return false;
                }
            }
            written.push((name, k));
        }
        self.pools.entry(slot).or_default().extend(written);
        true
    }

    /// One of the subject's bundles, whichever device: what a caller that
    /// does not name a device sees.
    pub fn bundle(&self, subject: &Keyhash) -> Option<&Vec<u8>> {
        self.bundles_of(subject).into_iter().next()
    }

    /// The bundle of one of the subject's devices.
    pub fn bundle_for(&self, subject: &Keyhash, device: &[u8; 32]) -> Option<&Vec<u8>> {
        self.bundles.get(&(*subject, *device))
    }

    /// Every bundle held for the subject, one per device, in device order.
    pub fn bundles_of(&self, subject: &Keyhash) -> Vec<&Vec<u8>> {
        self.bundles
            .range((*subject, [0; 32])..=(*subject, [0xff; 32]))
            .map(|(_, b)| b)
            .collect()
    }

    /// The subject's devices with a bundle held.
    pub fn devices_of(&self, subject: &Keyhash) -> Vec<[u8; 32]> {
        self.bundles
            .range((*subject, [0; 32])..=(*subject, [0xff; 32]))
            .map(|((_, d), _)| *d)
            .collect()
    }

    /// Keys held for the subject, every device's pool counted.
    pub fn pool_size(&self, subject: &Keyhash) -> usize {
        self.pools
            .range((*subject, [0; 32])..=(*subject, [0xff; 32]))
            .map(|(_, p)| p.len())
            .sum()
    }

    pub fn pool_size_for(&self, subject: &Keyhash, device: &[u8; 32]) -> usize {
        self.pools.get(&(*subject, *device)).map_or(0, |p| p.len())
    }

    /// Every subject with a bundle, once each.
    pub fn subjects(&self) -> Vec<Keyhash> {
        let mut out: Vec<Keyhash> = self.bundles.keys().map(|(s, _)| *s).collect();
        out.dedup();
        out
    }

    /// Answer a request from `requester` at `now`: the reply bytes, or
    /// nothing where the body is not a request.
    pub fn answer(&mut self, requester: &Keyhash, body: &[u8], now: u64) -> Option<Vec<u8>> {
        match PrekeyRequest::decode(body).ok()? {
            PrekeyRequest::One {
                subject,
                one_time,
                nonce,
                device,
            } => Some(
                self.answer_one(requester, &subject, device, one_time, nonce, now)
                    .encode(),
            ),
            PrekeyRequest::Batch { subjects, nonce } => {
                // a sweep: reusable material only, whatever the pools hold
                let replies: Vec<PrekeyReply> = subjects
                    .iter()
                    .map(|s| self.reusable(s, None, nonce))
                    .collect();
                Some(encode_batch_reply(&replies))
            }
        }
    }

    /// The reusable material: one bundle per device the node holds for the
    /// subject, up to the wire's ceiling, or the one device named
    /// (`wire-format.md` §7.8).
    fn reusable(
        &self,
        subject: &Keyhash,
        device: Option<[u8; 32]>,
        nonce: [u8; 16],
    ) -> PrekeyReply {
        let bundles: Vec<Vec<u8>> = match device {
            Some(d) => self.bundle_for(subject, &d).cloned().into_iter().collect(),
            None => self
                .bundles_of(subject)
                .into_iter()
                .take(rhtn_codec::bounds::PREKEY_BUNDLES_PER_REPLY)
                .cloned()
                .collect(),
        };
        if bundles.is_empty() {
            PrekeyReply {
                nonce,
                bundles,
                one_time: None,
                code: Some(FAIL_UNKNOWN_SUBJECT),
            }
        } else {
            PrekeyReply {
                nonce,
                bundles,
                one_time: None,
                code: None,
            }
        }
    }

    fn answer_one(
        &mut self,
        requester: &Keyhash,
        subject: &Keyhash,
        device: Option<[u8; 32]>,
        one_time: bool,
        nonce: [u8; 16],
        now: u64,
    ) -> PrekeyReply {
        let mut reply = self.reusable(subject, device, nonce);
        if reply.bundles.is_empty() || !one_time {
            return reply;
        }
        // the allowance: within it a key is consumed; over it the reusable
        // material is served and nothing is spent
        let e = self
            .issued
            .entry((*requester, *subject))
            .or_insert((now, 0));
        if now.saturating_sub(e.0) >= self.cfg.window_s {
            *e = (now, 0);
        }
        if e.1 >= self.cfg.one_time_per_requester_per_subject {
            return reply;
        }
        // the device's own pool first, then keys stocked for any device
        let d = device.unwrap_or(ANY_DEVICE);
        let slot = [(*subject, d), (*subject, ANY_DEVICE)]
            .into_iter()
            .find(|s| self.pools.get(s).is_some_and(|p| !p.is_empty()));
        let Some(slot) = slot else {
            return reply;
        };
        let taken = self.pools.get_mut(&slot).and_then(|p| p.pop_front());
        if let Some((name, key)) = taken {
            // spent on disk first: a key whose file cannot be removed is
            // not served at all, since serving it would leave it able to
            // come back
            if !self.forget(&slot, &name) {
                self.pools.entry(slot).or_default().push_front((name, key));
                return reply;
            }
            let e = self
                .issued
                .entry((*requester, *subject))
                .or_insert((now, 0));
            e.1 += 1;
            reply.one_time = Some(key);
            // **The counter is not written anywhere**
            // (`infra-client-requirements.md` §6): a line naming the
            // requester and the subject is a durable record of who asked
            // for whose bundle, which is the one thing §1's
            // process-and-discard obligation names about this exchange.
            //
            // A restart therefore opens a fresh window.  That costs the
            // *rate* and not the *budget*: a key spent is unlinked before
            // its reply goes out and never comes back, so a restart
            // refills nobody's pool.
            if self.pools.get(&slot).is_none_or(|p| p.is_empty()) {
                self.exhausted.push(*subject);
            }
        }
        reply
    }

    /// Unlink the key kept under `name`, where this service is kept on
    /// disk.  Whether it is gone: a service with no directory has nothing
    /// to remove and nothing can resurrect it.
    fn forget(&self, slot: &Slot, name: &str) -> bool {
        let Some(dir) = &self.dir else { return true };
        let p = slot_dir(dir, slot).join(name);
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
        self.issued
            .retain(|_, (opened, _)| now.saturating_sub(*opened) < w);
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
        for (slot, bundle) in &self.bundles {
            let d = slot_dir(dir, slot);
            std::fs::create_dir_all(&d)?;
            std::fs::write(d.join("bundle"), bundle)?;
        }
        for (slot, pool) in &self.pools {
            let d = slot_dir(dir, slot);
            std::fs::create_dir_all(&d)?;
            let held: std::collections::BTreeSet<&str> =
                pool.iter().map(|(n, _)| n.as_str()).collect();
            for (name, k) in pool {
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
        // and a device whose bundle and pool are gone is gone: a directory
        // with neither is one `load` skips anyway
        if let Ok(rd) = std::fs::read_dir(&root) {
            let kept: std::collections::BTreeSet<std::path::PathBuf> = self
                .bundles
                .keys()
                .chain(self.pools.keys())
                .map(|s| slot_dir(dir, s))
                .collect();
            for e in rd.flatten() {
                // **a subject is a directory**, and anything else under the
                // root is left alone: treating every entry as one turned a
                // stray file into `NotADirectory` out of `save`, which the
                // daemon propagates instead of writing its archive, its
                // store and its derived view
                if !e.file_type().is_ok_and(|t| t.is_dir()) {
                    continue;
                }
                let mut any = false;
                if let Ok(inner) = std::fs::read_dir(e.path()) {
                    for dd in inner.flatten() {
                        if !dd.file_type().is_ok_and(|t| t.is_dir()) {
                            continue;
                        }
                        if kept.contains(&dd.path()) {
                            any = true;
                        } else {
                            std::fs::remove_dir_all(dd.path())?;
                        }
                    }
                }
                if !any {
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
        s.dir = Some(dir.to_path_buf());
        Ok(s)
    }

    pub fn load(dir: &std::path::Path, cfg: PrekeyConfig) -> std::io::Result<PrekeyService> {
        let mut s = PrekeyService::new(cfg);
        let root = dir.join("prekeys");
        // **the rate-limit window is never written down**
        // (`infra-client-requirements.md` §6): who asked for whose bundle
        // is exactly the record the section forbids keeping, and an
        // earlier version of this service wrote one.  Issuance stopped
        // writing it; **nothing removed what was already there**, so an
        // upgraded node kept a durable requester/subject log for the life
        // of the directory.
        //
        // One named file, removed once, with the failure visible: an
        // operator's own files under this root are not this function's to
        // delete on suspicion.
        let legacy = root.join("issued");
        if legacy.is_file() {
            std::fs::remove_file(&legacy)?;
        }
        let Ok(rd) = std::fs::read_dir(&root) else {
            return Ok(s);
        };
        for e in rd.flatten() {
            let Ok(devices) = std::fs::read_dir(e.path()) else {
                continue;
            };
            for dd in devices.flatten() {
                let Some(device) = device_of_dir(&dd.file_name().to_string_lossy()) else {
                    continue;
                };
                let mut slot: Option<Slot> = None;
                if let Ok(bundle) = std::fs::read(dd.path().join("bundle"))
                    && let Ok(b) = PrekeyBundle::parse(&bundle)
                {
                    slot = Some((b.subject, b.device));
                    s.bundles.insert((b.subject, b.device), bundle);
                }
                let mut keys: Vec<(String, Vec<u8>)> = Vec::new();
                if let Ok(inner) = std::fs::read_dir(dd.path()) {
                    for f in inner.flatten() {
                        let name = f.file_name().to_string_lossy().to_string();
                        if name.starts_with("otk-")
                            && let Ok(k) = std::fs::read(f.path())
                        {
                            keys.push((name, k));
                        }
                    }
                }
                if keys.is_empty() {
                    continue;
                }
                // a pool stocked for any device sits beside the bundles
                // under the subject's own directory name
                let subject = match slot {
                    Some((sub, _)) => sub,
                    None => match unhex_keyhash(&e.file_name().to_string_lossy()) {
                        Some(sub) => sub,
                        None => continue,
                    },
                };
                keys.sort();
                // names carry the order the keys were stocked in, and the
                // counter resumes past the highest so a new key never takes a
                // name a served one had
                for (name, _) in &keys {
                    if let Some(n) = name
                        .strip_prefix("otk-")
                        .and_then(|n| n.parse::<u64>().ok())
                    {
                        s.seq = s.seq.max(n + 1);
                    }
                }
                s.pools
                    .insert((subject, device), keys.into_iter().collect());
            }
        }
        Ok(s)
    }
}

fn hex(k: &Keyhash) -> String {
    k.iter().map(|b| format!("{b:02x}")).collect()
}

/// `<dir>/prekeys/<subject hex>/<device hex or "any">`.
fn slot_dir(dir: &std::path::Path, slot: &Slot) -> std::path::PathBuf {
    let device = if slot.1 == ANY_DEVICE {
        "any".to_string()
    } else {
        hex(&slot.1)
    };
    dir.join("prekeys").join(hex(&slot.0)).join(device)
}

fn device_of_dir(name: &str) -> Option<[u8; 32]> {
    if name == "any" {
        return Some(ANY_DEVICE);
    }
    unhex_keyhash(name)
}

fn unhex_keyhash(s: &str) -> Option<[u8; 32]> {
    if s.len() != 64 {
        return None;
    }
    let mut out = [0u8; 32];
    for (i, b) in out.iter_mut().enumerate() {
        *b = u8::from_str_radix(&s[i * 2..i * 2 + 2], 16).ok()?;
    }
    Some(out)
}
