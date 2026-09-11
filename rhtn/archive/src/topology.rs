//! The local topology table (design §6, §3.4, §11.2.1, §6.2.5, §14.1.2):
//! the bindings a node has verified, what it derives from them, and the
//! acknowledgements it holds or issues.  Every node's table is its own.

use crate::record::{Record, SigStatus};
use crate::tx::*;
use crate::walk::Fetch;
use crate::{Keyhash, Txid};
use rhtn_crypto::SigningIdentity;
use rhtn_crypto::verify::{self, Lookup};
use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::sync::Arc;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum End {
    Departure,
    Disavowal(Option<u64>),
    /// The key was replaced by a recovery adoption (design §9.0.2).
    Superseded(Keyhash),
}

/// Whether an adoption's evidence has been dereferenced (`wire-format.md`
/// §3.4): structural verification does not dereference it, and a holder
/// that cannot may still hold the binding as a position.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EvidenceStatus {
    /// The named record was fetched and names these two parties, or the
    /// evidence is carried in the adoption itself.
    Satisfied,
    /// The named record is not held; a position, not yet a weighed edge.
    Unevaluated,
}

/// How an adoption's evidence gate is applied.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Evaluation {
    /// Refuse the adoption when its evidence cannot be dereferenced: what a
    /// patron does before relying on it.
    Required,
    /// Bind on structural verification and record the evidence as
    /// unevaluated: what a relay's table does, so its horizon follows the
    /// flood.  A record naming other parties is still refused.
    Deferred,
}

/// One patron-subordinate relationship, from the adoption that opened it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Binding {
    pub node: Keyhash,
    pub patron: Keyhash,
    pub series: u32,
    pub adoption: Txid,
    /// The adoption's timestamp: the patron's own clock.
    pub from: u64,
    pub end: Option<(Txid, u64, End)>,
    /// The subnet the adoption's locator names.
    pub anchor: Option<Keyhash>,
    pub evidence: EvidenceStatus,
}

impl Binding {
    pub fn open(&self) -> bool {
        self.end.is_none()
    }
}

/// A held `SubtreeAck` (`wire-format.md` §7.5).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ack {
    pub adoption: Txid,
    pub grandpatron: Keyhash,
    pub node: Keyhash,
    pub bytes: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Refusal {
    Structure(String),
    Signatures(SigStatus),
    /// Structurally valid; the evidence it names is not what it claims
    /// (`wire-format.md` §3.4's evaluation step).
    Evidence(String),
    /// The proposed patron sits in this node's own down-line (design §6.2.5).
    Cycle { below: Keyhash },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Applied {
    Adopted,
    /// A recovery replaced `prior` with `successor` as the current key.
    Replaced { prior: Keyhash, successor: Keyhash },
    Ended,
    /// Verified, and there was no binding for it to change.
    Nothing,
}

#[derive(Debug, Clone)]
pub struct Outcome {
    pub applied: Applied,
    /// Acknowledgements this node issued as a consequence (design §11.2.1).
    pub acks: Vec<Vec<u8>>,
}

/// The standing acknowledgement policy of an infra node (design §11.2.1,
/// `infra-client-requirements.md` §10.1): set beforehand, applied to every
/// adoption under one of its subordinates without anyone being asked.
/// Whether to acknowledge a given adoption: `(patron, node)` to a decision.
pub type AckPolicy = Arc<dyn Fn(&Keyhash, &Keyhash) -> bool + Send + Sync>;

pub struct AckIssuer {
    pub identity: Arc<SigningIdentity>,
    pub policy: AckPolicy,
    pub now: u64,
}

/// A patron preference for competing recovery claims (design §9.0.2):
/// which of two patrons this node trusts more.
pub type Preference = Arc<dyn Fn(&Keyhash, &Keyhash) -> Ordering + Send + Sync>;

/// A disavowal that arrived before the adoption it ends: patron, node, the
/// patron's timestamp, the disavowal's txid, and its reason code.
type PendingDisavowal = (Keyhash, Keyhash, u64, Txid, Option<u64>);
/// A departure received before the adoption whose series it names: node,
/// patron, series, txid, time.
type PendingDeparture = (Keyhash, Keyhash, u32, Txid, u64);

#[derive(Default)]
pub struct Table {
    /// This node's own identity, where it has one in the table.
    pub me: Option<Keyhash>,
    bindings: Vec<Binding>,
    nodes: BTreeSet<Keyhash>,
    held: BTreeSet<Txid>,
    acks: Vec<Ack>,
    infra: BTreeSet<Keyhash>,
    attached: BTreeMap<Keyhash, Vec<Keyhash>>,
    /// prior key -> (successor, its patron), every recovery seen
    lineage: BTreeMap<Keyhash, Vec<(Keyhash, Keyhash)>>,
    pending_disavowals: Vec<PendingDisavowal>,
    pending_departures: Vec<PendingDeparture>,
    pub prefer: Option<Preference>,
}

impl Table {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_me(me: Keyhash) -> Self {
        let mut t = Table { me: Some(me), ..Default::default() };
        t.nodes.insert(me);
        t
    }

    /// The same bindings, read as another node's own table.  Every node's
    /// view is its own; this copies what one node holds into another's,
    /// which is what a test does when both learned the same transactions.
    pub fn clone_for(&self, me: Keyhash) -> Table {
        Table {
            me: Some(me),
            bindings: self.bindings.clone(),
            nodes: self.nodes.clone(),
            held: self.held.clone(),
            acks: self.acks.clone(),
            infra: self.infra.clone(),
            attached: BTreeMap::new(),
            lineage: self.lineage.clone(),
            pending_disavowals: self.pending_disavowals.clone(),
            pending_departures: self.pending_departures.clone(),
            prefer: self.prefer.clone(),
        }
    }

    /// Mark a node as infrastructure (design §12.6.3).
    pub fn mark_infra(&mut self, k: Keyhash) {
        self.infra.insert(k);
        self.nodes.insert(k);
    }

    pub fn is_infra(&self, k: &Keyhash) -> bool {
        self.infra.contains(k)
    }

    pub fn holds(&self, txid: &Txid) -> bool {
        self.held.contains(txid)
    }

    pub fn is_node(&self, k: &Keyhash) -> bool {
        self.nodes.contains(k)
    }

    pub fn bindings(&self) -> &[Binding] {
        &self.bindings
    }

    fn open_bindings(&self) -> impl Iterator<Item = &Binding> {
        self.bindings.iter().filter(|b| b.open())
    }

    pub fn patrons(&self, node: &Keyhash) -> BTreeSet<Keyhash> {
        self.open_bindings().filter(|b| b.node == *node).map(|b| b.patron).collect()
    }

    pub fn subordinates(&self, patron: &Keyhash) -> BTreeSet<Keyhash> {
        self.open_bindings().filter(|b| b.patron == *patron).map(|b| b.node).collect()
    }

    /// Siblings share a patron (design §3.4).
    pub fn siblings(&self, node: &Keyhash) -> BTreeSet<Keyhash> {
        let mut out = BTreeSet::new();
        for p in self.patrons(node) {
            out.extend(self.subordinates(&p));
        }
        out.remove(node);
        out
    }

    pub fn is_root(&self, node: &Keyhash) -> bool {
        self.nodes.contains(node) && self.patrons(node).is_empty()
    }

    /// Every node within an `h`-edge walk of `me` over adoption and sibling
    /// edges, `me` included (design §15.1).  Peering edges do not count.
    pub fn horizon(&self, me: &Keyhash, h: usize) -> BTreeSet<Keyhash> {
        let mut seen: BTreeSet<Keyhash> = BTreeSet::from([*me]);
        let mut frontier: BTreeSet<Keyhash> = seen.clone();
        for _ in 0..h {
            let mut next = BTreeSet::new();
            for x in &frontier {
                next.extend(self.patrons(x));
                next.extend(self.subordinates(x));
                next.extend(self.siblings(x));
            }
            frontier = next.difference(&seen).copied().collect();
            seen.extend(frontier.iter().copied());
        }
        seen
    }

    /// design §15.1's h = 2: the ball a node stores topology for.
    pub fn in_h_store(&self, me: &Keyhash, x: &Keyhash) -> bool {
        self.horizon(me, 2).contains(x)
    }

    /// Whether `x` lies at or below `node` over open bindings.
    pub fn downline_contains(&self, node: &Keyhash, x: &Keyhash) -> bool {
        let mut seen = BTreeSet::new();
        let mut q = VecDeque::from([*node]);
        while let Some(n) = q.pop_front() {
            if n == *x {
                return true;
            }
            if seen.insert(n) {
                q.extend(self.subordinates(&n));
            }
        }
        false
    }

    /// The cycle check a node runs on its own proposed adoption (design
    /// §6.2.5): refuse only where local topology places the proposed patron
    /// at or below this node, never for absence of knowledge.
    pub fn propose_patron(&self, me: &Keyhash, candidate: &Keyhash) -> Result<(), Refusal> {
        if self.downline_contains(me, candidate) { Err(Refusal::Cycle { below: *candidate }) } else { Ok(()) }
    }

    /// The nearest infrastructure node on `client`'s patron chain, walking
    /// up past light-client patrons (design §14.1.2).
    pub fn serving_node(&self, client: &Keyhash) -> Option<Keyhash> {
        let mut cur = *client;
        for _ in 0..64 {
            let p = *self.patrons(&cur).iter().next()?;
            if self.infra.contains(&p) {
                return Some(p);
            }
            cur = p;
        }
        None
    }

    /// Record a light client attaching to this node, by path from this node
    /// down to it (`infra-client-requirements.md` §4.1).
    pub fn attach_client(&mut self, client: &Keyhash) -> Result<Vec<Keyhash>, String> {
        let me = self.me.ok_or("no own identity")?;
        if self.serving_node(client) != Some(me) {
            return Err("not served by this node".into());
        }
        let mut path = vec![*client];
        let mut cur = *client;
        while let Some(p) = self.patrons(&cur).iter().next().copied() {
            if p == me {
                break;
            }
            path.push(p);
            cur = p;
        }
        path.reverse();
        self.attached.insert(*client, path.clone());
        Ok(path)
    }

    pub fn attached_clients(&self) -> &BTreeMap<Keyhash, Vec<Keyhash>> {
        &self.attached
    }

    pub fn acks(&self) -> &[Ack] {
        &self.acks
    }

    /// The key this table treats as current for `k`, following recoveries
    /// (design §9.0.2): one per identity per observer.
    pub fn current_key(&self, k: &Keyhash) -> Keyhash {
        let mut cur = *k;
        for _ in 0..64 {
            let Some(succs) = self.lineage.get(&cur) else { return cur };
            let Some(chosen) = self.resolve(succs) else { return cur };
            cur = chosen;
        }
        cur
    }

    fn resolve(&self, succs: &[(Keyhash, Keyhash)]) -> Option<Keyhash> {
        let mut best = succs.first()?;
        if let Some(pref) = &self.prefer {
            for s in &succs[1..] {
                if pref(&s.1, &best.1) == Ordering::Greater {
                    best = s;
                }
            }
        }
        Some(best.0)
    }

    /// The occupancy of patron `p`'s slot for `n` as the patron's own
    /// timestamps order it: (from, Some(series)) when filled, (from, None)
    /// when emptied (design §6.2.2).
    pub fn slot_history(&self, p: &Keyhash, n: &Keyhash) -> Vec<(u64, Option<u32>)> {
        let mut ev = Vec::new();
        for b in self.bindings.iter().filter(|b| b.patron == *p && b.node == *n) {
            ev.push((b.from, Some(b.series)));
            if let Some((_, t, _)) = &b.end {
                ev.push((*t, None));
            }
        }
        ev.sort();
        ev
    }

    /// The series in `p`'s slot for `n` at time `t`, by `p`'s own clock.
    pub fn status_at(&self, p: &Keyhash, n: &Keyhash, t: u64) -> Option<u32> {
        self.slot_history(p, n).into_iter().take_while(|(at, _)| *at <= t).last().and_then(|(_, s)| s)
    }

    // ------------------------------------------------------------ applying

    /// Verify a transaction and apply it.  `presence` serves presence records
    /// an adoption's field 8 may name; `issuer` is this node's standing
    /// acknowledgement policy, if it is a grandpatron.
    pub fn apply<L: Lookup + ?Sized>(&mut self, rec: &Record, ids: &L, presence: &dyn Fetch, issuer: Option<&AckIssuer>) -> Result<Outcome, Refusal> {
        self.apply_with(rec, ids, presence, issuer, Evaluation::Required)
    }

    /// Check the presence record an adoption names against the two parties.
    fn evidence_status(rec: &Record, node: &Keyhash, patron: &Keyhash, presence: &dyn Fetch, mode: Evaluation) -> Result<EvidenceStatus, Refusal> {
        let Some(pop) = rec.field_hash(8) else { return Ok(EvidenceStatus::Satisfied) };
        let Some(bytes) = presence.fetch(&pop) else {
            return match mode {
                Evaluation::Required => Err(Refusal::Evidence("presence record not held".into())),
                Evaluation::Deferred => Ok(EvidenceStatus::Unevaluated),
            };
        };
        let pr = Record::parse(&bytes).map_err(Refusal::Evidence)?;
        let parts = pr.participants();
        if pr.tx_type != TYPE_PRESENCE || pr.txid != pop || !(parts.contains(node) && parts.contains(patron)) {
            return Err(Refusal::Evidence("presence record does not name these two parties".into()));
        }
        Ok(EvidenceStatus::Satisfied)
    }

    /// `apply`, with the evidence gate applied as `mode` says.
    pub fn apply_with<L: Lookup + ?Sized>(&mut self, rec: &Record, ids: &L, presence: &dyn Fetch, issuer: Option<&AckIssuer>, mode: Evaluation) -> Result<Outcome, Refusal> {
        match rec.check_signatures(ids) {
            SigStatus::Verified => {}
            s => return Err(Refusal::Signatures(s)),
        }
        let f = |k| rec.field_hash(k).ok_or_else(|| Refusal::Structure(format!("field {k}")));
        // an ending transaction already held is not applied twice: it ended
        // what it ended, or waits where it waits
        if self.held.contains(&rec.txid) && matches!(rec.tx_type, TYPE_DEPARTURE | TYPE_DISAVOWAL) {
            return Ok(Outcome { applied: Applied::Nothing, acks: Vec::new() });
        }
        let out = match rec.tx_type {
            TYPE_ADOPTION => {
                let node = f(1)?;
                let patron = f(2)?;
                let series = rec.seqno().ok_or(Refusal::Structure("seqno".into()))?.series;
                let evidence = Self::evidence_status(rec, &node, &patron, presence, mode)?;
                let anchor = rec.locator().map(|l| l.anchor);
                // an adoption already held is not bound twice; an unevaluated
                // one whose evidence has since arrived is upgraded
                if let Some(b) = self.bindings.iter_mut().find(|b| b.adoption == rec.txid) {
                    if b.evidence == EvidenceStatus::Unevaluated && evidence == EvidenceStatus::Satisfied {
                        b.evidence = EvidenceStatus::Satisfied;
                    }
                    return Ok(Outcome { applied: Applied::Nothing, acks: Vec::new() });
                }
                let mut applied = Applied::Adopted;
                if let Some(prior) = rec.prior_key() {
                    self.lineage.entry(prior).or_default().push((node, patron));
                    let successor = self.current_key(&prior);
                    for b in self.bindings.iter_mut().filter(|b| b.node == prior) {
                        match &b.end {
                            None => b.end = Some((rec.txid, rec.time, End::Superseded(successor))),
                            Some((_, _, End::Superseded(_))) => b.end = Some((rec.txid, rec.time, End::Superseded(successor))),
                            _ => {}
                        }
                    }
                    applied = Applied::Replaced { prior, successor };
                }
                self.nodes.insert(node);
                self.nodes.insert(patron);
                self.bindings.push(Binding { node, patron, series, adoption: rec.txid, from: rec.time, end: None, anchor, evidence });
                self.settle_pending_departures();
                self.settle_pending_disavowals();
                let mut acks = Vec::new();
                if let (Some(me), Some(iss)) = (self.me, issuer)
                    && self.subordinates(&me).contains(&patron) && (iss.policy)(&patron, &node) {
                        let bytes = subtree_ack(&iss.identity, &rec.txid, &node, iss.now);
                        self.acks.push(Ack { adoption: rec.txid, grandpatron: me, node, bytes: bytes.clone() });
                        acks.push(bytes);
                    }
                Outcome { applied, acks }
            }
            TYPE_DEPARTURE => {
                let node = f(1)?;
                let patron = f(2)?;
                // a departure ends the binding whose series it names
                // (`wire-format.md` §4.2, §2.3): a later re-adoption opens a
                // new series and is untouched by it.  Received before that
                // binding's adoption, it is held and settled when the
                // adoption arrives, so arrival order cannot resurrect a
                // relationship the node ended (§10.1.3's replay)
                let series = rec.seqno().ok_or(Refusal::Structure("seqno".into()))?.series;
                let ended = self.end_binding(&node, &patron, Some(series), (rec.txid, rec.time, End::Departure), None);
                if !ended {
                    self.pending_departures.push((node, patron, series, rec.txid, rec.time));
                }
                self.nodes.insert(node);
                Outcome { applied: if ended { Applied::Ended } else { Applied::Nothing }, acks: Vec::new() }
            }
            TYPE_DISAVOWAL => {
                let patron = f(1)?;
                let node = f(2)?;
                let code = rec.field_uint(4);
                let ended = self.end_binding(&node, &patron, None, (rec.txid, rec.time, End::Disavowal(code)), Some(rec.time));
                if !ended {
                    self.pending_disavowals.push((patron, node, rec.time, rec.txid, code));
                }
                Outcome { applied: if ended { Applied::Ended } else { Applied::Nothing }, acks: Vec::new() }
            }
            _ => Outcome { applied: Applied::Nothing, acks: Vec::new() },
        };
        self.held.insert(rec.txid);
        self.lapse_acks();
        Ok(out)
    }

    /// End the binding of `node` under `patron`.  A departure names the
    /// series of the relationship it ends; a disavowal is ordered by the
    /// patron's own clock: it ends the adoption in that slot whose
    /// timestamp precedes it and which no later adoption has replaced.
    fn end_binding(&mut self, node: &Keyhash, patron: &Keyhash, series: Option<u32>, ending: (Txid, u64, End), ordered_at: Option<u64>) -> bool {
        let idx = match ordered_at {
            None => self.bindings.iter().position(|b| b.node == *node && b.patron == *patron && series.is_none_or(|s| b.series == s) && b.open()),
            Some(t) => self
                .bindings
                .iter()
                .enumerate()
                .filter(|(_, b)| b.node == *node && b.patron == *patron && b.from <= t && b.end.as_ref().is_none_or(|(_, e, _)| *e > t))
                .max_by_key(|(_, b)| b.from)
                .map(|(i, _)| i),
        };
        let Some(i) = idx else { return false };
        // a later adoption in the same slot bounds what this disavowal can end
        if let Some(t) = ordered_at {
            let later = self.bindings.iter().any(|b| b.node == *node && b.patron == *patron && b.from > self.bindings[i].from && b.from <= t);
            if later {
                return false;
            }
        }
        self.bindings[i].end = Some(ending);
        true
    }

    fn settle_pending_departures(&mut self) {
        let pending = std::mem::take(&mut self.pending_departures);
        for (node, patron, series, txid, t) in pending {
            if !self.end_binding(&node, &patron, Some(series), (txid, t, End::Departure), None) {
                self.pending_departures.push((node, patron, series, txid, t));
            }
        }
    }

    fn settle_pending_disavowals(&mut self) {
        let pending = std::mem::take(&mut self.pending_disavowals);
        for (patron, node, t, txid, code) in pending {
            if !self.end_binding(&node, &patron, None, (txid, t, End::Disavowal(code)), Some(t)) {
                self.pending_disavowals.push((patron, node, t, txid, code));
            }
        }
    }

    /// Discard acknowledgements whose relationship has ended
    /// (`wire-format.md` §7.5): the acknowledged adoption, or the patron's
    /// own binding under the grandpatron.
    fn lapse_acks(&mut self) {
        let bindings = self.bindings.clone();
        self.acks.retain(|a| {
            let Some(b) = bindings.iter().find(|b| b.adoption == a.adoption) else { return false };
            b.open() && bindings.iter().any(|pb| pb.node == b.patron && pb.patron == a.grandpatron && pb.open())
        });
    }

    /// Take a received `SubtreeAck` (`wire-format.md` §7.5): verify it
    /// under the grandpatron it names, hold it when the adoption it
    /// acknowledges is held, and never create a binding from it.
    pub fn take_ack<L: Lookup + ?Sized>(&mut self, ids: &L, bytes: &[u8]) -> Result<bool, String> {
        let item = rhtn_codec::cbor::parse_all(bytes).map_err(|e| e.0)?;
        rhtn_codec::schema::check_kind(bytes, "SubtreeAck", &item).map_err(|e| e.0)?;
        if !verify::record(ids, "SubtreeAck", bytes).map_err(|e| e.to_string())? {
            return Err("grandpatron signature fails".into());
        }
        let rhtn_codec::cbor::Item::Map(m) = &item else { return Err("map".into()) };
        let kh = |k: u64| match rhtn_codec::cbor::map_get(m, k) {
            Some(rhtn_codec::cbor::Item::Bytes(r)) if r.len() == 32 => bytes[r.clone()].try_into().ok(),
            _ => None,
        };
        let adoption: Txid = kh(1).ok_or("field 1")?;
        let grandpatron: Keyhash = kh(2).ok_or("field 2")?;
        let node: Keyhash = kh(3).ok_or("field 3")?;
        if !self.bindings.iter().any(|b| b.adoption == adoption && b.open()) {
            return Ok(false);
        }
        self.acks.push(Ack { adoption, grandpatron, node, bytes: bytes.to_vec() });
        self.lapse_acks();
        Ok(true)
    }
}

/// Authenticated supersession evidence for a credential (design §12.6.5,
/// `infra-client-requirements.md` §2): a validated recovery names the
/// successor key; a verified reissue keeps the key and moves its series.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Supersession {
    pub superseded: Keyhash,
    pub successor: Keyhash,
    pub evidence: Txid,
}

impl Supersession {
    /// From a verified recovery adoption or series reissue; anything else,
    /// or a failing signature, is no evidence.
    pub fn from_record<L: Lookup + ?Sized>(rec: &Record, ids: &L) -> Result<Self, String> {
        if rec.check_signatures(ids) != SigStatus::Verified {
            return Err("not verified".into());
        }
        match rec.tx_type {
            TYPE_ADOPTION => {
                let prior = rec.prior_key().ok_or("not a recovery adoption")?;
                let successor = rec.field_hash(1).ok_or("node")?;
                Ok(Supersession { superseded: prior, successor, evidence: rec.txid })
            }
            TYPE_REISSUE => {
                let node = rec.field_hash(1).ok_or("node")?;
                Ok(Supersession { superseded: node, successor: node, evidence: rec.txid })
            }
            _ => Err("neither a recovery nor a reissue".into()),
        }
    }
}

/// What a patron derives from a presented archive (design §16.7,
/// `infra-client-requirements.md` §5): the transactions with counterparties
/// it already knows, by counterparty.  Nothing is totalled; records naming
/// strangers contribute nothing.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct InitialTrust {
    pub by_counterparty: BTreeMap<Keyhash, Vec<Txid>>,
}

pub fn initial_trust(known: &BTreeSet<Keyhash>, subject: &Keyhash, records: &[Record]) -> InitialTrust {
    let mut out = InitialTrust::default();
    for r in records {
        for s in &r.signers {
            if s != subject && known.contains(s) {
                out.by_counterparty.entry(*s).or_default().push(r.txid);
            }
        }
    }
    out
}
