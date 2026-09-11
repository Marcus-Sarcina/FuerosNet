//! The subject's side of a query (design §7.4.1, §7.4.2, §7.5.2.4,
//! §7.5.2.8; `light-client-requirements.md` §1.3, §1.4): consent to each
//! query about oneself, a counter per requester kept only for the ceremony
//! window, the capture key released directly to the selected verifier
//! against the most recent eligible capture, and the responses verifiers
//! deliver back, held for the finalization veto.

use crate::keys::capture_key;
use crate::notice::{Notice, Notifier};
use crate::query::{KeyGrant, Response, VerificationQuery, consent};
use crate::store::ClientStore;
use crate::{Keyhash, Txid};
use rhtn_codec::cose::sha256;
use rhtn_crypto::SigningIdentity;
use rhtn_crypto::verify::{self, Lookup};
use std::collections::{BTreeMap, BTreeSet};

/// A retention year is 365 days (design §7.5.1).
pub const YEAR_SECONDS: u64 = 365 * 86_400;

/// The subject's own numbers: how many queries one requester may make in a
/// ceremony window, and the retention horizon it enforces by declining to
/// release keys past it (design §7.5.2.3), two years by default.
#[derive(Debug, Clone)]
pub struct SubjectConfig {
    pub per_requester_limit: u32,
    pub retention_seconds: u64,
}

impl Default for SubjectConfig {
    fn default() -> Self {
        SubjectConfig { per_requester_limit: 10, retention_seconds: 2 * YEAR_SECONDS }
    }
}

/// The anti-oracle aggregate for one ceremony (design §7.4.1): the one
/// profile this ceremony carries and a counter per requester, both
/// discarded when the window closes.  A lock, not a log.
#[derive(Debug, Clone)]
struct Window {
    ceremony_id: [u8; 32],
    profile_digest: Option<[u8; 32]>,
    counters: BTreeMap<Keyhash, u32>,
}

/// A subject's state: the open window, if any; the ids of the queries it
/// consented to, by ceremony, which is what a late response is checked
/// against and names no requester; and the responses delivered to it, by
/// query id, which is what the finalization veto reads.
#[derive(Debug, Clone)]
pub struct SubjectState {
    pub cfg: SubjectConfig,
    window: Option<Window>,
    consented: BTreeMap<[u8; 32], BTreeSet<[u8; 32]>>,
    pub responses: BTreeMap<[u8; 32], Vec<u8>>,
}

impl SubjectState {
    pub fn new(cfg: SubjectConfig) -> Self {
        SubjectState { cfg, window: None, consented: BTreeMap::new(), responses: BTreeMap::new() }
    }

    /// A ceremony about this subject begins: its pre-commitment fixes the
    /// window queries must bind to.
    pub fn open_window(&mut self, ceremony_id: [u8; 32]) {
        self.window = Some(Window { ceremony_id, profile_digest: None, counters: BTreeMap::new() });
    }

    /// The ceremony window closes: the counters go with it, and nothing of
    /// who probed remains.
    pub fn close_window(&mut self) {
        self.window = None;
    }

    pub fn window_open(&self) -> bool {
        self.window.is_some()
    }

    /// The query ids this subject consented to in `ceremony`.
    pub fn consented(&self, ceremony: &[u8; 32]) -> BTreeSet<[u8; 32]> {
        self.consented.get(ceremony).cloned().unwrap_or_default()
    }

    /// Every query id this subject consented to, in any ceremony.
    pub fn all_consented(&self) -> BTreeSet<[u8; 32]> {
        self.consented.values().flatten().copied().collect()
    }

    /// Consent to `q`, or not: only within the open window it binds to,
    /// only about this subject, only under the ceremony's one profile, and
    /// only within the requester's allowance.  The query is surfaced to the
    /// person as it arrives, and an excess is refused visibly.
    pub fn consent_to(&mut self, me: &SigningIdentity, q: &VerificationQuery, notifier: &dyn Notifier) -> Option<Vec<u8>> {
        if q.subject != me.public.keyhash {
            return None;
        }
        let limit = self.cfg.per_requester_limit;
        let w = self.window.as_mut()?;
        if w.ceremony_id != q.ceremony_id {
            return None;
        }
        let digest = sha256(&q.profile);
        match w.profile_digest {
            Some(d) if d != digest => return None,
            Some(_) => {}
            None => w.profile_digest = Some(digest),
        }
        let count = w.counters.entry(q.querier).or_insert(0);
        if *count >= limit {
            notifier.notify(Notice::ProbingRefused { requester: q.querier });
            return None;
        }
        *count += 1;
        let qid = q.query_id();
        self.consented.entry(q.ceremony_id).or_default().insert(qid);
        notifier.notify(Notice::QuerySurfaced { verifier: q.verifier });
        Some(consent(me, &qid))
    }

    /// The grant for a query addressed to `verifier`: the most recent
    /// eligible capture that verifier holds of this subject, eligible
    /// meaning within the retention horizon, by default (design §7.5.2.8).
    /// `None` where the verifier holds nothing eligible, which is a
    /// deliberate act and reads as unavailability.
    pub fn grant_for(&self, me: &SigningIdentity, store: &ClientStore, verifier: &Keyhash, query_id: [u8; 32], now: u64) -> Option<KeyGrant> {
        let (record, seed) = store
            .seeds
            .iter()
            .filter(|(_, s)| s.counterparty == *verifier && now.saturating_sub(s.finalized_at) < self.cfg.retention_seconds)
            .max_by_key(|(_, s)| s.finalized_at)?;
        Some(KeyGrant { record: *record, query_id, key: capture_key(&seed.seed, &me.public.keyhash, verifier, &seed.ceremony_id) })
    }

    /// Take the copy of a response a verifier delivers about this subject
    /// (`wire-format.md` §5.6): verified under the verifier, about this
    /// subject, answering a query it consented to.  Held by query id.
    pub fn take_response_copy<L: Lookup + ?Sized>(&mut self, me: &SigningIdentity, ids: &L, bytes: &[u8]) -> Result<[u8; 32], String> {
        let r = Response::read(bytes)?;
        if r.subject != me.public.keyhash {
            return Err("not about this subject".into());
        }
        if !self.consented.values().any(|s| s.contains(&r.query_id)) {
            return Err("a query this subject did not consent to".into());
        }
        verify::response(ids, bytes, false).map_err(|e| e.to_string())?;
        self.responses.insert(r.query_id, bytes.to_vec());
        Ok(r.query_id)
    }

    /// The record each held response would belong to is the caller's to
    /// say; this is every held response's bytes.
    pub fn held_responses(&self) -> Vec<Vec<u8>> {
        self.responses.values().cloned().collect()
    }

    /// Forget a record's ceremony state once the record is discarded.
    pub fn discard_ceremony(&mut self, ceremony: &[u8; 32]) {
        self.consented.remove(ceremony);
    }
}

/// The eligible captures a verifier holds of `me`, most recent first: what
/// a subject choosing to grant against an older one picks from.
pub fn eligible_captures(store: &ClientStore, verifier: &Keyhash, retention_seconds: u64, now: u64) -> Vec<Txid> {
    let mut v: Vec<(u64, Txid)> = store.seeds.iter().filter(|(_, s)| s.counterparty == *verifier && now.saturating_sub(s.finalized_at) < retention_seconds).map(|(t, s)| (s.finalized_at, *t)).collect();
    v.sort_by(|a, b| b.cmp(a));
    v.into_iter().map(|(_, t)| t).collect()
}
