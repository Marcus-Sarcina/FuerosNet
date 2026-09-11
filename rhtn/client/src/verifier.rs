//! The verifier's side of a query (`wire-format.md` §5.5, §5.6, §7.3;
//! design §7.3, §7.4, §7.5.2.4): what a client does with a type-4 request
//! and a key grant, automatically and without telling its operator.  The
//! comparison itself is behind [`Matcher`], the biometric engine being
//! undecided (design §22.2).

use crate::query::{Basis, KeyGrant, QueryRequest, Response, VerificationQuery, Verdict};
use crate::store::{ClientStore, SealParams, open};
use crate::{Keyhash, query};
use rhtn_codec::cose::sha256;
use rhtn_crypto::SigningIdentity;
use rhtn_crypto::verify::Lookup;
use std::collections::{BTreeMap, BTreeSet};

/// The comparison a verifier runs between a template it holds and the
/// profile a query carries: categorical, never a score (design §7.4.1).
pub trait Matcher {
    fn compare(&self, template: &[u8], profile: &[u8], template_version: u64) -> Verdict;
}

/// The reference client's stand-in for an engine (design §22.2): a profile
/// matches a template when the bytes are equal.
pub struct ByteEquality;

impl Matcher for ByteEquality {
    fn compare(&self, template: &[u8], profile: &[u8], _template_version: u64) -> Verdict {
        if template == profile { Verdict::Match } else { Verdict::NoMatch }
    }
}

/// A verifier's own numbers, all local (`wire-format.md` §7.3, design
/// §7.4.1): how long an unopened grant waits for its query and a query for
/// its grant, how many queries one requester may make in a window, and the
/// template versions this client can compare under.
#[derive(Debug, Clone)]
pub struct VerifierConfig {
    pub seal: SealParams,
    pub grant_buffer_ms: u64,
    pub per_requester_limit: u32,
    pub window_ms: u64,
    pub template_versions: BTreeSet<u64>,
}

impl Default for VerifierConfig {
    fn default() -> Self {
        VerifierConfig { seal: SealParams::default(), grant_buffer_ms: 60_000, per_requester_limit: 16, window_ms: 600_000, template_versions: BTreeSet::from([1]) }
    }
}

/// What a verifier acts with: its own identity, the keys it holds, its
/// store and its engine.  Borrowed for the duration of one event.
pub struct Verifying<'a, L: Lookup + ?Sized> {
    pub me: &'a SigningIdentity,
    pub ids: &'a L,
    pub store: &'a ClientStore,
    pub matcher: &'a dyn Matcher,
}

/// A signed response and where it goes: to the querier on its stream, and
/// a copy to the subject over the association the grant established
/// (`wire-format.md` §5.6).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Answer {
    pub query_id: [u8; 32],
    pub to_querier: Vec<u8>,
    pub to_subject: (Keyhash, Vec<u8>),
}

/// What a type-4 request came to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum QueryOutcome {
    /// The stream is closed with nothing sent: no error schema exists, and
    /// no signed response is fabricated for input that is not evidence.
    Closed(&'static str),
    Answered(Answer),
    /// A well-formed query whose grant has not arrived: it waits, bounded.
    AwaitingGrant,
}

/// What an arriving grant came to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GrantOutcome {
    Rejected(&'static str),
    /// Held, bounded, for a query not yet seen.
    Buffered,
    /// A duplicate, or a second grant for a query already answered.
    Ignored,
    Answered(Answer),
}

#[derive(Debug, Clone)]
struct Pending {
    query: VerificationQuery,
    consent: Vec<u8>,
    selection_basis: u64,
    received_at: u64,
}

/// A verifier's state across queries: which profile each ceremony carries,
/// the requester counters for the window, the queries waiting for grants
/// and the grants waiting for queries, and which queries were answered.
/// No key is kept: a grant's key lives until it is used or the buffer bound
/// passes, and never past an answer.
#[derive(Debug, Clone)]
pub struct VerifierState {
    pub cfg: VerifierConfig,
    ceremonies: BTreeMap<[u8; 32], [u8; 32]>,
    counters: BTreeMap<Keyhash, (u64, u32)>,
    pending_queries: BTreeMap<[u8; 32], Pending>,
    pending_grants: BTreeMap<[u8; 32], (Keyhash, KeyGrant, u64)>,
    answered: BTreeSet<[u8; 32]>,
}

impl VerifierState {
    pub fn new(cfg: VerifierConfig) -> Self {
        VerifierState { cfg, ceremonies: BTreeMap::new(), counters: BTreeMap::new(), pending_queries: BTreeMap::new(), pending_grants: BTreeMap::new(), answered: BTreeSet::new() }
    }

    /// Queries waiting for their grant, by id.
    pub fn awaiting(&self) -> Vec<[u8; 32]> {
        self.pending_queries.keys().copied().collect()
    }

    /// Grants waiting for their query, by id.
    pub fn buffered(&self) -> Vec<[u8; 32]> {
        self.pending_grants.keys().copied().collect()
    }

    /// Take a type-4 request from the authenticated `peer`.  Checked before
    /// anything else, each closing the stream: the body's shape, that field
    /// 7 names this verifier, that field 2 names the peer, that the consent
    /// is the subject's over the id, that the ceremony's one profile is
    /// this one, and the requester's allowance.  Then a claimed `met` this
    /// verifier's own records refute, or a template version it cannot
    /// compare under, is answered `unavailable` with no comparison run.
    pub fn take_query<L: Lookup + ?Sized>(&mut self, cx: &Verifying<L>, peer: Keyhash, body: &[u8], now_ms: u64) -> QueryOutcome {
        let Ok(req) = QueryRequest::decode(body) else { return QueryOutcome::Closed("malformed request") };
        let q = req.query;
        if q.verifier != cx.me.public.keyhash {
            return QueryOutcome::Closed("not addressed to this verifier");
        }
        if q.querier != peer {
            return QueryOutcome::Closed("querier is not the authenticated requester");
        }
        let Some(subject) = cx.ids.identity(&q.subject) else { return QueryOutcome::Closed("subject's key not held") };
        let qid = q.query_id();
        if !query::consent_verifies(subject, &req.consent, &qid) {
            return QueryOutcome::Closed("consent does not verify under the subject");
        }
        // one profile per ceremony (design §7.4.1, §7.4.2)
        let digest = sha256(&q.profile);
        match self.ceremonies.get(&q.ceremony_id) {
            Some(d) if *d != digest => return QueryOutcome::Closed("a second profile within one ceremony"),
            Some(_) => {}
            None => {
                self.ceremonies.insert(q.ceremony_id, digest);
            }
        }
        // the requester's allowance in this window (design §7.4.1)
        let entry = self.counters.entry(peer).or_insert((now_ms, 0));
        if now_ms.saturating_sub(entry.0) >= self.cfg.window_ms {
            *entry = (now_ms, 0);
        }
        if entry.1 >= self.cfg.per_requester_limit {
            return QueryOutcome::Closed("over the requester's allowance");
        }
        entry.1 += 1;
        if self.answered.contains(&qid) {
            return QueryOutcome::Closed("already answered");
        }
        // a claimed `met` this verifier's own records refute, or a version
        // it cannot compare under: unavailable, and nothing compared
        if (req.selection_basis == 0 && !cx.store.has_met(&peer)) || !self.cfg.template_versions.contains(&q.template_version) {
            let answer = self.answer(cx, &q, &req.consent, req.selection_basis, None);
            return QueryOutcome::Answered(answer);
        }
        if let Some((sender, grant, _)) = self.pending_grants.remove(&qid) {
            if sender != q.subject {
                // a buffered grant from anyone but the subject is fabrication
                self.pending_queries.insert(qid, Pending { query: q, consent: req.consent, selection_basis: req.selection_basis, received_at: now_ms });
                return QueryOutcome::AwaitingGrant;
            }
            let answer = self.answer(cx, &q, &req.consent, req.selection_basis, Some(grant));
            return QueryOutcome::Answered(answer);
        }
        self.pending_queries.insert(qid, Pending { query: q, consent: req.consent, selection_basis: req.selection_basis, received_at: now_ms });
        QueryOutcome::AwaitingGrant
    }

    /// Take a grant from the authenticated `sender`.  The sender must be
    /// the subject of the query it names, once that query is known; the
    /// first grant for a query stands and later ones change nothing; one
    /// arriving before its query waits, bounded.
    pub fn take_grant<L: Lookup + ?Sized>(&mut self, cx: &Verifying<L>, sender: Keyhash, bytes: &[u8], now_ms: u64) -> GrantOutcome {
        let Ok(grant) = KeyGrant::decode(bytes) else { return GrantOutcome::Rejected("malformed grant") };
        if self.answered.contains(&grant.query_id) {
            return GrantOutcome::Ignored;
        }
        if let Some(p) = self.pending_queries.get(&grant.query_id) {
            if p.query.subject != sender {
                return GrantOutcome::Rejected("the sender is not the subject");
            }
            let p = self.pending_queries.remove(&grant.query_id).expect("present");
            let answer = self.answer(cx, &p.query, &p.consent, p.selection_basis, Some(grant));
            return GrantOutcome::Answered(answer);
        }
        if self.pending_grants.contains_key(&grant.query_id) {
            return GrantOutcome::Ignored;
        }
        self.pending_grants.insert(grant.query_id, (sender, grant, now_ms));
        GrantOutcome::Buffered
    }

    /// Let the bounds pass: a grant that waited its bound without a query is
    /// discarded unopened, and a query that waited its bound without a
    /// grant is answered `unavailable`, the key never having reached the
    /// comparison.  Counters whose window closed go too.
    pub fn expire<L: Lookup + ?Sized>(&mut self, cx: &Verifying<L>, now_ms: u64) -> Vec<Answer> {
        let bound = self.cfg.grant_buffer_ms;
        self.pending_grants.retain(|_, (_, _, at)| now_ms.saturating_sub(*at) < bound);
        let window = self.cfg.window_ms;
        self.counters.retain(|_, (opened, _)| now_ms.saturating_sub(*opened) < window);
        let due: Vec<[u8; 32]> = self.pending_queries.iter().filter(|(_, p)| now_ms.saturating_sub(p.received_at) >= bound).map(|(k, _)| *k).collect();
        let mut out = Vec::new();
        for qid in due {
            let p = self.pending_queries.remove(&qid).expect("present");
            out.push(self.answer(cx, &p.query, &p.consent, p.selection_basis, None));
        }
        out
    }

    /// Answer a query, with or without a grant (`wire-format.md` §5.5, §7.3;
    /// design §7.5.2.7): no grant, or one naming a record not held, or one
    /// for another subject's capture, is `unavailable`; a capture that
    /// fails to open is `inconclusive` with basis 0 and the query's
    /// version; a capture that opens is compared.  The grant's key is
    /// consumed here and kept nowhere.
    fn answer<L: Lookup + ?Sized>(&mut self, cx: &Verifying<L>, q: &VerificationQuery, consent: &[u8], selection_basis: u64, grant: Option<KeyGrant>) -> Answer {
        let evaluated = grant.and_then(|g| {
            let sealed = cx.store.sealed.get(&g.record)?;
            if sealed.subject != q.subject {
                return None;
            }
            Some(match open(&self.cfg.seal, &g.key, sealed) {
                Err(_) => Verdict::Inconclusive,
                Ok(capture) => cx.matcher.compare(&capture.template, &q.profile, q.template_version),
            })
        });
        let (verdict, basis, template_version) = match evaluated {
            Some(v) => (v, Some(Basis::PhotoMatch), Some(q.template_version)),
            None => (Verdict::Unavailable, None, None),
        };
        let qid = q.query_id();
        let bytes = Response { verifier: cx.me.public.keyhash, subject: q.subject, query_id: qid, verdict, basis, template_version, consent: consent.to_vec(), selection_basis }.sign(cx.me);
        self.answered.insert(qid);
        Answer { query_id: qid, to_querier: bytes.clone(), to_subject: (q.subject, bytes) }
    }
}
