//! The local topology table (design §6, §3.4, §11.2.1, §6.2.5, §14.1.2):
//! the bindings a node has verified, what it derives from them, and the
//! acknowledgements it holds or issues.  Every node's table is its own.

use crate::record::{Record, SigStatus};
use crate::tx::*;
use crate::walk::Fetch;
use crate::{Keyhash, Txid};
use rhtn_codec::cbor::{Item, as_uint, map_get, parse_all};
use rhtn_codec::encode::*;
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

impl End {
    /// Whether the patron made an adverse judgment, where the ending was a
    /// disavowal that stated a reason.
    ///
    /// **The band is the whole point of the code space**
    /// (`wire-format.md` §4.3): 64 values with bit 5 carrying the
    /// distinction, so a policy can act correctly on a code it has never
    /// seen — `code >= 32` and nothing else — without a lookup table and
    /// without a specification update. Reading the code and never reading
    /// the band would leave every future assignment a flag day, which is
    /// the thing the banding exists to prevent.
    ///
    /// Nothing here weights it: §4.3 says a trust policy *may* reasonably
    /// weight the stated reason, and design §16.4 keeps what a node stores
    /// and forwards independent of its policy.
    #[must_use]
    pub fn with_prejudice(&self) -> Option<bool> {
        match self {
            End::Disavowal(Some(code)) => Some(End::band(*code)),
            _ => None,
        }
    }

    /// `wire-format.md` §4.3's band: bit 5 separates 0-31 from 32-63, so a
    /// code nobody recognises is still readable as an allegation or not.
    ///
    /// **One definition, because two readers want it**: the fold, for the
    /// end it recorded, and a policy reading a disavowal out of the
    /// records it holds whether or not that disavowal was the object that
    /// closed the binding (design §18.5).
    #[must_use]
    pub fn band(code: u64) -> bool {
        code >= 32
    }
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
    /// Which of the patron's ten slots the adoption's locator claims: its
    /// path's final nibble (design §3.1).  Nothing for a self-anchored
    /// root, which occupies no patron's slot.
    pub slot: Option<u8>,
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
    /// The slot the locator claims is already held by an open subordinate
    /// of the same patron in the same subnet (design §3.1).
    ///
    /// **A bound on this table, not a judgement about the record.** Ten
    /// slots hold ten subordinates and there is nowhere to put an eleventh
    /// or a second occupant, so the incumbent stays and the record is not
    /// stored.  Nothing here adjudicates which of two signed adoptions was
    /// the patron's real intent — that is the patron's to get right, and
    /// §1.1 leaves a holder no way to find out.
    Slot { patron: Keyhash, slot: u8, held: Keyhash },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Applied {
    Adopted,
    /// A reissue moved a relationship into the series it entered.
    Reissued { node: Keyhash, patron: Keyhash, series: u32 },
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
    /// Reissues whose binding has not arrived: `(node, patron, left, entered)`.
    pending_reissues: Vec<(Keyhash, Keyhash, u32, u32)>,
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
            pending_reissues: self.pending_reissues.clone(),
            prefer: self.prefer.clone(),
        }
    }

    /// Mark a node as infrastructure.
    ///
    /// A node marks itself, and marks another when it holds an endpoint
    /// record it published: `wire-format.md` §7.6 has only infra nodes
    /// publish, which makes the record the evidence [author, 2026-09-14].
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

    /// How many adoption or sibling edges separate `me` from `other`,
    /// searching no further than `h`.  Nothing where `other` is not within
    /// `h` edges, which at `h = 2` is design §15.1's horizon.
    ///
    /// **The same walk that defines the region measures inside it**
    /// (design §15.1.1), so a party's distance and its membership are one
    /// question asked twice and cannot come back disagreeing.
    pub fn distance(&self, me: &Keyhash, other: &Keyhash, h: usize) -> Option<usize> {
        if me == other {
            return Some(0);
        }
        let mut seen: BTreeSet<Keyhash> = BTreeSet::from([*me]);
        let mut frontier: BTreeSet<Keyhash> = seen.clone();
        for d in 1..=h {
            let mut next = BTreeSet::new();
            for x in &frontier {
                next.extend(self.patrons(x));
                next.extend(self.subordinates(x));
                next.extend(self.siblings(x));
            }
            frontier = next.difference(&seen).copied().collect();
            if frontier.contains(other) {
                return Some(d);
            }
            seen.extend(frontier.iter().copied());
        }
        None
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
    /// Whether this table can hold what `rec` claims of a patron's slot
    /// (design §3.1), deciding nothing else and changing nothing.
    ///
    /// **Ten slots, one occupant each**: the patron's fanout and the
    /// path's nibble range are the same bound, so holding a second
    /// occupant would mean holding two subordinates at one index. A
    /// recovery's predecessor does not count as the occupant — its
    /// bindings close when the recovery applies, and a successor
    /// inheriting the slot it vacates is the whole point of that
    /// transaction. A record this table already holds is admitted whatever
    /// it claims: §10.1.3's reconciliation is a replay of the same frames,
    /// and a replay that refused itself would not converge.
    ///
    /// **Public because the storage decision asks it too.**
    /// `wire-format.md` §10.1.2 has a forwarding node vouch with its
    /// storage decision, so a node that stored and flooded a record its
    /// own fold then refused would be vouching for what it rejects. One
    /// predicate, asked twice, rather than two that can drift.
    pub fn admits_slot(&self, rec: &Record) -> Result<(), Refusal> {
        if rec.tx_type != TYPE_ADOPTION || self.bindings.iter().any(|b| b.adoption == rec.txid) {
            return Ok(());
        }
        let (Some(node), Some(patron)) = (rec.field_hash(1), rec.field_hash(2)) else { return Ok(()) };
        let Some(loc) = rec.locator() else { return Ok(()) };
        let Some(slot) = loc.slot() else { return Ok(()) };
        let prior = rec.prior_key();
        match self.bindings.iter().find(|b| {
            b.open() && b.patron == patron && b.anchor == Some(loc.anchor) && b.slot == Some(slot) && b.node != node && Some(b.node) != prior
        }) {
            Some(held) => Err(Refusal::Slot { patron, slot, held: held.node }),
            None => Ok(()),
        }
    }

    pub fn apply<L: Lookup + ?Sized>(&mut self, rec: &Record, ids: &L, presence: &dyn Fetch, issuer: Option<&AckIssuer>) -> Result<Outcome, Refusal> {
        self.apply_with(rec, ids, presence, issuer, Evaluation::Required)
    }

    /// Check the presence record an adoption names against the two parties.
    fn evidence_status<L: Lookup + ?Sized>(rec: &Record, ids: &L, node: &Keyhash, patron: &Keyhash, presence: &dyn Fetch, mode: Evaluation) -> Result<EvidenceStatus, Refusal> {
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
        // evidence counts only once its own signatures verify
        // (`wire-format.md` §3.4): a record that fails is never evidence,
        // and one this holder cannot verify is unevaluated where evaluation
        // is deferred and refused, naming the key, where it is required
        match pr.check_signatures(ids) {
            SigStatus::Verified => Ok(EvidenceStatus::Satisfied),
            SigStatus::Invalid(e) => Err(Refusal::Evidence(format!("presence record fails verification: {e}"))),
            SigStatus::Unverifiable { missing } => match mode {
                Evaluation::Required => Err(Refusal::Evidence(format!("presence record unverifiable: missing key {}", missing.iter().map(|b| format!("{b:02x}")).collect::<String>()))),
                Evaluation::Deferred => Ok(EvidenceStatus::Unevaluated),
            },
        }
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
                let evidence = Self::evidence_status(rec, ids, &node, &patron, presence, mode)?;
                let anchor = rec.locator().map(|l| l.anchor);
                let slot = rec.locator().and_then(|l| l.slot());
                // an adoption already held is not bound twice; an unevaluated
                // one whose evidence has since arrived is upgraded
                if let Some(b) = self.bindings.iter_mut().find(|b| b.adoption == rec.txid) {
                    if b.evidence == EvidenceStatus::Unevaluated && evidence == EvidenceStatus::Satisfied {
                        b.evidence = EvidenceStatus::Satisfied;
                    }
                    return Ok(Outcome { applied: Applied::Nothing, acks: Vec::new() });
                }
                self.admits_slot(rec)?;
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
                self.bindings.push(Binding { node, patron, series, adoption: rec.txid, from: rec.time, end: None, anchor, slot, evidence });
                self.settle_pending_reissues();
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
            TYPE_REISSUE => {
                let node = f(1)?;
                let patron = f(2)?;
                // A reissue leaves one series and enters another within the
                // same relationship (`wire-format.md` §4.6): the binding
                // follows it, so a departure naming the series now current
                // ends the relationship this reissue advanced rather than
                // matching nothing.
                //
                // **The series left is what identifies it**, not the larger
                // number: succession is proved by the countersigned chain
                // (§4.6.1), and a re-adoption that happens to open a higher
                // series is a different binding this must not touch.
                let left = rec.seqno_left().ok_or(Refusal::Structure("field 3".into()))?.series;
                let entered = rec.seqno().ok_or(Refusal::Structure("field 4".into()))?.series;
                let advanced = match self.bindings.iter_mut().find(|b| b.node == node && b.patron == patron && b.series == left && b.open()) {
                    Some(b) => {
                        b.series = entered;
                        true
                    }
                    // held for the adoption it advances, as a departure is:
                    // arrival order cannot lose a series change
                    None => {
                        self.pending_reissues.push((node, patron, left, entered));
                        false
                    }
                };
                if advanced {
                    self.settle_pending_departures();
                }
                self.nodes.insert(node);
                Outcome { applied: if advanced { Applied::Reissued { node, patron, series: entered } } else { Applied::Nothing }, acks: Vec::new() }
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

    /// Apply reissues held for a binding that has since arrived.  Run
    /// before the departures, since a departure may name the series a
    /// reissue is about to enter.  Repeated until nothing moves, a chain
    /// of reissues arriving in reverse settling one link per pass.
    fn settle_pending_reissues(&mut self) {
        loop {
            let mut moved = false;
            let pending = std::mem::take(&mut self.pending_reissues);
            for (node, patron, left, entered) in pending {
                match self.bindings.iter_mut().find(|b| b.node == node && b.patron == patron && b.series == left && b.open()) {
                    Some(b) => {
                        b.series = entered;
                        moved = true;
                    }
                    None => self.pending_reissues.push((node, patron, left, entered)),
                }
            }
            if !moved {
                return;
            }
        }
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
        if verify::record(ids, "SubtreeAck", bytes).map_err(|e| e.to_string()).is_err() {
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

// ----------------------------------------------- the table, materialised

/// The derived table written out, so a party that wakes need not replay
/// the transactions that produced it.
///
/// **The store stays the system of record.** This is a cache of a pure
/// function of it, carrying enough to say whether it is still that
/// function's value: the number of records folded in and the highest
/// `(effective, txid)` among them, which is the order [`Table::apply`] is
/// fed in.  A snapshot that cannot account for what the store holds is
/// discarded and the replay runs, so the worst a stale or damaged one
/// costs is the work it was meant to save.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Snapshot {
    /// A digest over the transaction identifiers folded in, in the order
    /// they were folded.  **This names the input exactly**, where a count
    /// only names how many there were.
    pub folded: [u8; 32],
    pub high: Option<(u64, Txid)>,
    pub table: Vec<u8>,
}

/// What a restore did, which a caller reports rather than assumes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Restored {
    /// The snapshot was the store's value: nothing was replayed.
    Current,
    /// The snapshot was behind and `folded` later records brought it up.
    Extended { folded: usize },
    /// No usable snapshot, or one the store had moved under: `replayed`
    /// records went through `apply` from nothing.
    Replayed { replayed: usize },
}

/// Whether `(effective, txid)` sorts after everything a snapshot folded in.
///
/// A record at or below the high-water is one the snapshot either already
/// holds or should have: either way this side cannot tell which, and the
/// answer is to replay rather than to guess.
pub fn above(high: &Option<(u64, Txid)>, at: (u64, Txid)) -> bool {
    match high {
        None => true,
        Some(h) => at > *h,
    }
}

/// The digest a snapshot carries: the transaction identifiers it folded
/// in, concatenated in the order they were folded.
pub fn fold_digest<'a>(txids: impl Iterator<Item = &'a Txid>) -> [u8; 32] {
    let mut acc = Vec::new();
    for t in txids {
        acc.extend_from_slice(t);
    }
    rhtn_codec::cose::sha256(&acc)
}

/// Which of `records` a snapshot has not folded in, in the order the
/// caller holds them, or nothing where the snapshot cannot account for the
/// set at all.
///
/// **The digest is the whole test** (design §15.1.1), and it names the
/// input rather than counting it.  A count and a high-water mark are
/// equal for two different record sets whenever one record has been
/// replaced by another below the mark, and the stale fold is then reused
/// over input it was never taken from.  Recomputing the digest over what
/// is held costs one pass and cannot be satisfied by a substitution.
///
/// Ordering is the caller's, since it holds the records in whatever form
/// it keeps them; both callers sort by `(effective, txid)`, which is the
/// order [`Table::apply`] is fed in.
pub fn unfolded<T>(snap: &Snapshot, records: &[T], at: impl Fn(&T) -> (u64, Txid)) -> Option<Vec<usize>> {
    let (mut later, mut below) = (Vec::new(), Vec::new());
    for (i, r) in records.iter().enumerate() {
        let k = at(r);
        if above(&snap.high, k) {
            later.push(i);
        } else {
            below.push(k.1);
        }
    }
    (fold_digest(below.iter()) == snap.folded).then_some(later)
}

impl Snapshot {
    /// The bytes as they go to disk.
    pub fn encode(&self) -> Vec<u8> {
        let mut out = Vec::new();
        emit_map_head(&mut out, 3);
        emit_uint(&mut out, 1);
        emit_bstr(&mut out, &self.folded);
        emit_uint(&mut out, 2);
        match &self.high {
            Some((t, id)) => {
                emit_array_head(&mut out, 2);
                emit_uint(&mut out, *t);
                emit_bstr(&mut out, id);
            }
            None => emit_array_head(&mut out, 0),
        }
        emit_uint(&mut out, 3);
        emit_bstr(&mut out, &self.table);
        out
    }

    pub fn decode(b: &[u8]) -> Option<Snapshot> {
        let item = parse_all(b).ok()?;
        let Item::Map(m) = &item else { return None };
        let Item::Bytes(f) = map_get(m, 1)? else { return None };
        let folded: [u8; 32] = b[f.clone()].try_into().ok()?;
        let high = match map_get(m, 2)? {
            Item::Array(a) if a.is_empty() => None,
            Item::Array(a) => match a.as_slice() {
                [Item::Uint(t), Item::Bytes(r)] => Some((*t, b[r.clone()].try_into().ok()?)),
                _ => return None,
            },
            _ => return None,
        };
        let Item::Bytes(r) = map_get(m, 3)? else { return None };
        Some(Snapshot { folded, high, table: b[r.clone()].to_vec() })
    }
}

fn emit_kh_set(out: &mut Vec<u8>, s: &BTreeSet<Keyhash>) {
    emit_array_head(out, s.len());
    for k in s {
        emit_bstr(out, k);
    }
}

fn emit_txid_set(out: &mut Vec<u8>, s: &BTreeSet<Txid>) {
    emit_array_head(out, s.len());
    for k in s {
        emit_bstr(out, k);
    }
}

fn bytes_of(b: &[u8], it: &Item) -> Option<Vec<u8>> {
    match it {
        Item::Bytes(r) => Some(b[r.clone()].to_vec()),
        _ => None,
    }
}

fn kh_of(b: &[u8], it: &Item) -> Option<Keyhash> {
    bytes_of(b, it)?.try_into().ok()
}

fn kh_list(b: &[u8], it: &Item) -> Option<Vec<Keyhash>> {
    let Item::Array(a) = it else { return None };
    a.iter().map(|x| kh_of(b, x)).collect()
}

fn uint_of(it: &Item) -> Option<u64> {
    as_uint(it)
}

impl Table {
    /// Write the derived state out.  **Not the records**: what is here is
    /// what replaying them produced, and the records themselves are the
    /// store's business.
    ///
    /// The acknowledgement policy and the recovery preference are not
    /// written.  Both are the operator's standing choices rather than
    /// derived state, and a party that read its own policy back out of a
    /// cache would be taking last run's decision for this run's.
    pub fn materialise(&self) -> Vec<u8> {
        let mut out = Vec::new();
        emit_map_head(&mut out, 10);

        emit_uint(&mut out, 1);
        match &self.me {
            Some(k) => emit_bstr(&mut out, k),
            None => emit_bstr(&mut out, &[]),
        }

        emit_uint(&mut out, 2);
        emit_array_head(&mut out, self.bindings.len());
        for b in &self.bindings {
            emit_array_head(&mut out, 9);
            emit_bstr(&mut out, &b.node);
            emit_bstr(&mut out, &b.patron);
            emit_uint(&mut out, b.series as u64);
            emit_bstr(&mut out, &b.adoption);
            emit_uint(&mut out, b.from);
            match &b.end {
                None => emit_array_head(&mut out, 0),
                Some((txid, at, end)) => {
                    emit_array_head(&mut out, 4);
                    emit_bstr(&mut out, txid);
                    emit_uint(&mut out, *at);
                    match end {
                        End::Departure => {
                            emit_uint(&mut out, 0);
                            emit_array_head(&mut out, 0);
                        }
                        End::Disavowal(code) => {
                            emit_uint(&mut out, 1);
                            match code {
                                Some(c) => {
                                    emit_array_head(&mut out, 1);
                                    emit_uint(&mut out, *c);
                                }
                                None => emit_array_head(&mut out, 0),
                            }
                        }
                        End::Superseded(k) => {
                            emit_uint(&mut out, 2);
                            emit_array_head(&mut out, 1);
                            emit_bstr(&mut out, k);
                        }
                    }
                }
            }
            match &b.anchor {
                Some(a) => emit_bstr(&mut out, a),
                None => emit_bstr(&mut out, &[]),
            }
            match &b.slot {
                Some(i) => {
                    emit_array_head(&mut out, 1);
                    emit_uint(&mut out, *i as u64);
                }
                None => emit_array_head(&mut out, 0),
            }
            emit_uint(&mut out, matches!(b.evidence, EvidenceStatus::Satisfied) as u64);
        }

        emit_uint(&mut out, 3);
        emit_kh_set(&mut out, &self.nodes);
        emit_uint(&mut out, 4);
        emit_txid_set(&mut out, &self.held);

        emit_uint(&mut out, 5);
        emit_array_head(&mut out, self.acks.len());
        for a in &self.acks {
            emit_array_head(&mut out, 4);
            emit_bstr(&mut out, &a.adoption);
            emit_bstr(&mut out, &a.grandpatron);
            emit_bstr(&mut out, &a.node);
            emit_bstr(&mut out, &a.bytes);
        }

        emit_uint(&mut out, 6);
        emit_kh_set(&mut out, &self.infra);

        emit_uint(&mut out, 7);
        emit_array_head(&mut out, self.attached.len());
        for (client, patrons) in &self.attached {
            emit_array_head(&mut out, 2);
            emit_bstr(&mut out, client);
            emit_array_head(&mut out, patrons.len());
            for p in patrons {
                emit_bstr(&mut out, p);
            }
        }

        emit_uint(&mut out, 8);
        emit_array_head(&mut out, self.lineage.len());
        for (prior, succs) in &self.lineage {
            emit_array_head(&mut out, 2);
            emit_bstr(&mut out, prior);
            emit_array_head(&mut out, succs.len());
            for (s, p) in succs {
                emit_array_head(&mut out, 2);
                emit_bstr(&mut out, s);
                emit_bstr(&mut out, p);
            }
        }

        // what arrived out of order and is still waiting for the record it
        // refers to: dropping these would silently un-end a relationship a
        // disavowal already closed
        emit_uint(&mut out, 9);
        emit_array_head(&mut out, self.pending_disavowals.len() + self.pending_departures.len());
        for (p, n, at, txid, code) in &self.pending_disavowals {
            emit_array_head(&mut out, 6);
            emit_uint(&mut out, 0);
            emit_bstr(&mut out, p);
            emit_bstr(&mut out, n);
            emit_uint(&mut out, *at);
            emit_bstr(&mut out, txid);
            match code {
                Some(c) => {
                    emit_array_head(&mut out, 1);
                    emit_uint(&mut out, *c);
                }
                None => emit_array_head(&mut out, 0),
            }
        }
        for (n, p, series, txid, at) in &self.pending_departures {
            emit_array_head(&mut out, 6);
            emit_uint(&mut out, 1);
            emit_bstr(&mut out, n);
            emit_bstr(&mut out, p);
            emit_uint(&mut out, *at);
            emit_bstr(&mut out, txid);
            emit_array_head(&mut out, 1);
            emit_uint(&mut out, *series as u64);
        }

        emit_uint(&mut out, 10);
        emit_array_head(&mut out, self.pending_reissues.len());
        for (n, p, left, entered) in &self.pending_reissues {
            emit_array_head(&mut out, 4);
            emit_bstr(&mut out, n);
            emit_bstr(&mut out, p);
            emit_uint(&mut out, *left as u64);
            emit_uint(&mut out, *entered as u64);
        }

        out
    }

    /// Read a materialised table back.  Nothing here is trusted for its
    /// content — the bytes are this party's own — but a shape that does
    /// not read is discarded whole rather than read part-way, since a
    /// table missing half its bindings is worse than no table at all.
    pub fn from_materialised(b: &[u8]) -> Option<Table> {
        let item = parse_all(b).ok()?;
        let Item::Map(m) = &item else { return None };
        let mut t = Table::new();

        let me = bytes_of(b, map_get(m, 1)?)?;
        t.me = (!me.is_empty()).then(|| me.try_into().ok()).flatten();

        let Item::Array(bindings) = map_get(m, 2)? else { return None };
        for row in bindings {
            let Item::Array(f) = row else { return None };
            if f.len() != 9 {
                return None;
            }
            let end = match &f[5] {
                Item::Array(e) if e.is_empty() => None,
                Item::Array(e) if e.len() == 4 => {
                    let txid: Txid = kh_of(b, &e[0])?;
                    let at = uint_of(&e[1])?;
                    let Item::Array(arg) = &e[3] else { return None };
                    let kind = match (uint_of(&e[2])?, arg.as_slice()) {
                        (0, []) => End::Departure,
                        (1, []) => End::Disavowal(None),
                        (1, [c]) => End::Disavowal(Some(uint_of(c)?)),
                        (2, [k]) => End::Superseded(kh_of(b, k)?),
                        _ => return None,
                    };
                    Some((txid, at, kind))
                }
                _ => return None,
            };
            let anchor = bytes_of(b, &f[6])?;
            let slot = match &f[7] {
                Item::Array(v) if v.is_empty() => None,
                Item::Array(v) if v.len() == 1 => Some(u8::try_from(uint_of(&v[0])?).ok()?),
                _ => return None,
            };
            t.bindings.push(Binding {
                node: kh_of(b, &f[0])?,
                patron: kh_of(b, &f[1])?,
                series: uint_of(&f[2])?.try_into().ok()?,
                adoption: kh_of(b, &f[3])?,
                from: uint_of(&f[4])?,
                end,
                anchor: (!anchor.is_empty()).then(|| anchor.try_into().ok()).flatten(),
                slot,
                evidence: if uint_of(&f[8])? == 1 { EvidenceStatus::Satisfied } else { EvidenceStatus::Unevaluated },
            });
        }

        t.nodes = kh_list(b, map_get(m, 3)?)?.into_iter().collect();
        t.held = kh_list(b, map_get(m, 4)?)?.into_iter().collect();

        let Item::Array(acks) = map_get(m, 5)? else { return None };
        for row in acks {
            let Item::Array(f) = row else { return None };
            if f.len() != 4 {
                return None;
            }
            t.acks.push(Ack { adoption: kh_of(b, &f[0])?, grandpatron: kh_of(b, &f[1])?, node: kh_of(b, &f[2])?, bytes: bytes_of(b, &f[3])? });
        }

        t.infra = kh_list(b, map_get(m, 6)?)?.into_iter().collect();

        let Item::Array(attached) = map_get(m, 7)? else { return None };
        for row in attached {
            let Item::Array(f) = row else { return None };
            if f.len() != 2 {
                return None;
            }
            t.attached.insert(kh_of(b, &f[0])?, kh_list(b, &f[1])?);
        }

        let Item::Array(lineage) = map_get(m, 8)? else { return None };
        for row in lineage {
            let Item::Array(f) = row else { return None };
            if f.len() != 2 {
                return None;
            }
            let Item::Array(succs) = &f[1] else { return None };
            let mut out = Vec::with_capacity(succs.len());
            for s in succs {
                let Item::Array(pair) = s else { return None };
                if pair.len() != 2 {
                    return None;
                }
                out.push((kh_of(b, &pair[0])?, kh_of(b, &pair[1])?));
            }
            t.lineage.insert(kh_of(b, &f[0])?, out);
        }

        let Item::Array(pending) = map_get(m, 9)? else { return None };
        for row in pending {
            let Item::Array(f) = row else { return None };
            if f.len() != 6 {
                return None;
            }
            let (a, c, at, txid) = (kh_of(b, &f[1])?, kh_of(b, &f[2])?, uint_of(&f[3])?, kh_of(b, &f[4])?);
            let Item::Array(arg) = &f[5] else { return None };
            match uint_of(&f[0])? {
                0 => {
                    let code = match arg.as_slice() {
                        [] => None,
                        [c] => Some(uint_of(c)?),
                        _ => return None,
                    };
                    t.pending_disavowals.push((a, c, at, txid, code));
                }
                1 => {
                    let [s] = arg.as_slice() else { return None };
                    t.pending_departures.push((a, c, uint_of(s)?.try_into().ok()?, txid, at));
                }
                _ => return None,
            }
        }

        let Item::Array(reissues) = map_get(m, 10)? else { return None };
        for row in reissues {
            let Item::Array(f) = row else { return None };
            if f.len() != 4 {
                return None;
            }
            t.pending_reissues.push((kh_of(b, &f[0])?, kh_of(b, &f[1])?, uint_of(&f[2])?.try_into().ok()?, uint_of(&f[3])?.try_into().ok()?));
        }

        Some(t)
    }
}
