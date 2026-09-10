//! Currency attestations: issuance, the escalation ladder when a patron is
//! unreachable, and the checks a relying party runs against its own clock
//! and its own topology (design §12.6.5, §12.6.5.1; `wire-format.md` §7.1).
//!
//! Two halves of one rule, in different hands.  Issuing: issue only for the
//! key you currently record, fresh and never extended.  Relying: nothing
//! waits on a staple, which says only which key to address, and never fail
//! open on knowledge you hold.

use crate::view::NodeView;
use crate::{Adjacency, Keyhash};
use rhtn_archive::currency::Attestation;
use rhtn_archive::tx::currency_attestation;
use rhtn_codec::cbor::*;
use rhtn_codec::encode::*;
use rhtn_codec::schema::{self, Family};
use rhtn_crypto::verify::Lookup;
use std::collections::{BTreeMap, BTreeSet};

/// `issuer_role` (`wire-format.md` §7.1): the rungs of §12.6.5.1's ladder.
pub const ROLE_PATRON: u64 = 0;
pub const ROLE_SIBLING: u64 = 1;
pub const ROLE_GRANDPATRON: u64 = 2;
/// A down-line threshold attesting a root's current key (design §12.7.2):
/// an optional input to anchor caching, not a rung of the ladder.  Declared
/// because the wire assigns it; nothing here issues or consults it.
pub const ROLE_DOWNLINE: u64 = 3;

/// `CurrencyReply` field 2.
pub const REPLY_ATTESTATION: u64 = 0;
pub const REPLY_CANNOT_ISSUE: u64 = 1;

/// design §21's default: hours, not days.
pub const DEFAULT_LIFETIME_SECONDS: u64 = 10 * 3600;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CurrencyRequest {
    /// The subject asked about.  The querier is not named, so an answer
    /// forwarded onward attributes the question to nobody.
    pub subject: Keyhash,
    pub nonce: [u8; 16],
}

impl CurrencyRequest {
    pub fn encode(&self) -> Vec<u8> {
        let mut out = Vec::new();
        emit_map_head(&mut out, 2);
        emit_uint(&mut out, 1);
        emit_bstr(&mut out, &self.subject);
        emit_uint(&mut out, 2);
        emit_bstr(&mut out, &self.nonce);
        out
    }
    pub fn decode(b: &[u8]) -> Result<Self, String> {
        parse_all(b).map_err(|e| e.0)?;
        schema::check_unsigned(Family::CurrencyRequest, b, 0).map_err(|e| e.0)?;
        let item = parse_all(b).map_err(|e| e.0)?;
        let Item::Map(m) = &item else { return Err("not a map".into()) };
        let subject = match map_get(m, 1) {
            Some(Item::Bytes(r)) if r.len() == 32 => <[u8; 32]>::try_from(&b[r.clone()]).map_err(|_| "subject")?,
            _ => return Err("field 1".into()),
        };
        let nonce = match map_get(m, 2) {
            Some(Item::Bytes(r)) if r.len() == 16 => <[u8; 16]>::try_from(&b[r.clone()]).map_err(|_| "nonce")?,
            _ => return Err("field 2".into()),
        };
        Ok(CurrencyRequest { subject, nonce })
    }
}

/// The answers are an attestation, or nothing.  Code 1 says the responder
/// cannot issue rather than inventing one; silence says the responder was
/// not reached, and a caller receiving neither concludes nothing
/// (`wire-format.md` §7.1).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CurrencyReply {
    Attestation { nonce: [u8; 16], bytes: Vec<u8> },
    CannotIssue { nonce: [u8; 16] },
}

impl CurrencyReply {
    pub fn nonce(&self) -> [u8; 16] {
        match self {
            CurrencyReply::Attestation { nonce, .. } | CurrencyReply::CannotIssue { nonce } => *nonce,
        }
    }
    pub fn encode(&self) -> Vec<u8> {
        let mut out = Vec::new();
        match self {
            CurrencyReply::Attestation { nonce, bytes } => {
                emit_map_head(&mut out, 3);
                emit_uint(&mut out, 1);
                emit_bstr(&mut out, nonce);
                emit_uint(&mut out, 2);
                emit_uint(&mut out, REPLY_ATTESTATION);
                emit_uint(&mut out, 3);
                out.extend_from_slice(bytes);
            }
            CurrencyReply::CannotIssue { nonce } => {
                emit_map_head(&mut out, 2);
                emit_uint(&mut out, 1);
                emit_bstr(&mut out, nonce);
                emit_uint(&mut out, 2);
                emit_uint(&mut out, REPLY_CANNOT_ISSUE);
            }
        }
        out
    }
    pub fn decode(b: &[u8]) -> Result<Self, String> {
        parse_all(b).map_err(|e| e.0)?;
        schema::check_unsigned(Family::CurrencyReply, b, 0).map_err(|e| e.0)?;
        let item = parse_all(b).map_err(|e| e.0)?;
        let Item::Map(m) = &item else { return Err("not a map".into()) };
        let nonce = match map_get(m, 1) {
            Some(Item::Bytes(r)) if r.len() == 16 => <[u8; 16]>::try_from(&b[r.clone()]).map_err(|_| "nonce")?,
            _ => return Err("field 1".into()),
        };
        match map_get(m, 2).and_then(as_uint).ok_or("field 2")? {
            REPLY_ATTESTATION => {
                let r3 = value_slice(b, 3).ok_or("field 3")?;
                Ok(CurrencyReply::Attestation { nonce, bytes: b[r3].to_vec() })
            }
            REPLY_CANNOT_ISSUE => Ok(CurrencyReply::CannotIssue { nonce }),
            _ => Err("unknown reply code".into()),
        }
    }
}

/// How a relying party reads a staple, against its own clock and its own
/// topology (design §12.6.5).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Staple {
    Current,
    Expired,
    Absent,
    /// The attestation names a different subject.
    WrongSubject,
    /// The issuer is a party this node can place, and it does not stand on
    /// the rung the attestation claims for this subject.
    WrongIssuer,
    /// This node cannot place the issuer at all: it holds no patron for the
    /// subject and was handed none.  A staple it cannot place establishes
    /// nothing.
    UnknownIssuer,
}

/// What a relying party does about a binding.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Gate {
    /// Proceed: nothing waits on a staple (design §12.6.5).
    Proceed,
    /// Serve nothing under the binding: this party holds authenticated
    /// supersession evidence for it.
    Refuse(&'static str),
}

/// Nothing waits on a staple (design §12.6.5): stapling supports routing —
/// which key to address — and no trust transaction or user operation is
/// gated on it, current, expired or absent.  The one refusal is knowledge:
/// a party holding authenticated supersession evidence serves nothing under
/// the superseded key.  Fail-open is for ignorance, never for knowledge.
pub fn gate(superseded: bool) -> Gate {
    if superseded { Gate::Refuse("the binding is verified superseded") } else { Gate::Proceed }
}

/// Read a staple against `now`, the relying party's own clock, and against
/// the issuers the relying party can place for the subject: `patrons` are
/// the subject's patrons as this party knows them, from its table or from
/// the introduction's locator (design §12.6.5, §9.0.2), and `placed` says
/// whether an issuer stands on the rung the attestation claims.
pub fn staple_state(att: Option<&Attestation>, subject: &Keyhash, now: u64, patrons: &BTreeSet<Keyhash>, placed: &dyn Fn(&Keyhash, u64) -> Option<bool>) -> Staple {
    let Some(a) = att else { return Staple::Absent };
    if a.subject != *subject {
        return Staple::WrongSubject;
    }
    match placed(&a.issuer, a.role) {
        Some(true) => {}
        Some(false) => return Staple::WrongIssuer,
        None if patrons.is_empty() => return Staple::UnknownIssuer,
        None => return Staple::WrongIssuer,
    }
    if a.expires_at <= now { Staple::Expired } else { Staple::Current }
}

/// Whom this node will issue for, and on what rung.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Rung {
    Patron,
    /// The patron is unreachable and this node is its sibling, holding the
    /// adoption record by replication (design §3.4).
    Sibling,
    /// The patron and its siblings are unreachable and this node is the
    /// grandpatron, which holds the adoption record anyway.
    Grandpatron,
}

impl Rung {
    pub fn role(&self) -> u64 {
        match self {
            Rung::Patron => ROLE_PATRON,
            Rung::Sibling => ROLE_SIBLING,
            Rung::Grandpatron => ROLE_GRANDPATRON,
        }
    }
}

/// A node's own currency policy: the lifetime it issues for, the parties it
/// holds unreachable and since when, and how long an outage must have
/// lasted before each rung of the ladder opens (design §12.6.5.1's table is
/// keyed on outage duration).  Which key a subject currently holds is the
/// table's business, not this.
pub struct CurrencyState {
    pub lifetime: u64,
    /// Fed by the transport's reachability detector on the sessions this
    /// node holds, or by an operator: who is dark, and since when on this
    /// node's own clock.
    pub unreachable: BTreeMap<Keyhash, u64>,
    /// How long a patron must have been dark before its sibling issues:
    /// the table's "hours to days".  The reference node's number is the
    /// attestation lifetime, the point at which staples start expiring.
    pub sibling_after: u64,
    /// How long the patron and every sibling must have been dark before the
    /// grandpatron issues: the table's "days".  The reference node's number
    /// is two days.  Both are the operator's, not the design's.
    pub grandpatron_after: u64,
}

impl CurrencyState {
    /// The detector settled `who` unreachable at `since`; an earlier mark
    /// stands, since the outage began when it began.
    pub fn dark(&mut self, who: Keyhash, since: u64) {
        self.unreachable.entry(who).or_insert(since);
    }

    /// The detector settled `who` reachable again.
    pub fn back(&mut self, who: &Keyhash) {
        self.unreachable.remove(who);
    }

    /// How long `who` has been dark at `now`, if it is.
    pub fn dark_for(&self, who: &Keyhash, now: u64) -> Option<u64> {
        self.unreachable.get(who).map(|s| now.saturating_sub(*s))
    }
}

impl Default for CurrencyState {
    fn default() -> Self {
        CurrencyState { lifetime: DEFAULT_LIFETIME_SECONDS, unreachable: BTreeMap::new(), sibling_after: DEFAULT_LIFETIME_SECONDS, grandpatron_after: 2 * 86_400 }
    }
}

/// One outstanding fallback query (design §12.6.5): the introducer first,
/// then the patron only if the introducer answers code 1 or not at all.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CurrencyAsk {
    pub subject: Keyhash,
    pub nonce: [u8; 16],
    pub introducer: Option<Keyhash>,
    pub patron: Option<Keyhash>,
    /// Whom the request went to, in order.
    pub asked: Vec<Keyhash>,
}

/// What learning the current key came to: settled from what the node
/// holds, or a question now outstanding.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Requirement {
    Settled(Gate),
    Asked(CurrencyAsk),
}

/// What a reply to an outstanding ask did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AskStep {
    /// A verified, current staple, now held for the subject.
    Current,
    /// The reply carried a staple this node does not accept as current.
    NotCurrent(Staple),
    /// The responder could not issue and the next party was asked.
    AskedNext(Keyhash),
    /// Nobody left to ask: the caller concludes nothing and addresses what
    /// it holds.
    Exhausted,
    WrongNonce,
}

impl NodeView {
    /// Whether `k` has been superseded in this node's own view: a recovery
    /// it has applied names a successor (design §9.0.2).
    pub fn is_superseded(&self, k: &Keyhash) -> bool {
        self.table.current_key(k) != *k
    }

    /// Whether `issuer` stands on the rung `role` claims for `subject`, as
    /// this node's table has it; `None` when the table cannot say.
    pub fn places_issuer(&self, subject: &Keyhash, issuer: &Keyhash, role: u64) -> Option<bool> {
        // an identity a querier still names by its old key is placed by the
        // bindings its current key holds
        let subject = self.table.current_key(subject);
        let patrons = self.table.patrons(&subject);
        if patrons.is_empty() {
            return None;
        }
        Some(match role {
            ROLE_PATRON => patrons.contains(issuer),
            ROLE_SIBLING => patrons.iter().any(|p| self.table.siblings(p).contains(issuer)),
            ROLE_GRANDPATRON => patrons.iter().any(|p| self.table.patrons(p).contains(issuer)),
            _ => false,
        })
    }

    /// The rung this node stands on for `subject`, if any, at this node's
    /// own clock: the patron always; a sibling once the patron has been
    /// dark for `sibling_after`; the grandpatron once the patron and every
    /// sibling have been dark for `grandpatron_after` (design §12.6.5.1).
    pub fn rung_for(&self, cur: &CurrencyState, subject: &Keyhash) -> Option<Rung> {
        let me = self.me();
        let subject = self.table.current_key(subject);
        let patrons = self.table.patrons(&subject);
        if patrons.contains(&me) {
            return Some(Rung::Patron);
        }
        // a sibling of the patron, holding the record by replication
        let unreachable_patron = patrons.iter().find(|p| cur.unreachable.contains_key(*p))?;
        let outage = cur.dark_for(unreachable_patron, self.now).unwrap_or(0);
        if self.table.siblings(unreachable_patron).contains(&me) {
            return if outage >= cur.sibling_after { Some(Rung::Sibling) } else { None };
        }
        // the grandpatron, once the patron and every sibling have been dark
        // for days
        if self.table.patrons(unreachable_patron).contains(&me) && outage >= cur.grandpatron_after {
            let siblings = self.table.siblings(unreachable_patron);
            if siblings.iter().all(|s| cur.dark_for(s, self.now).is_some_and(|d| d >= cur.grandpatron_after)) {
                return Some(Rung::Grandpatron);
            }
        }
        None
    }

    /// Issue a fresh attestation for `subject`, on whatever rung this node
    /// stands, naming the key its table currently records for that
    /// identity.  After a recovery that is the successor and never the old
    /// key (`infra-client-requirements.md` §3).  Never an extension of an
    /// earlier one: there is no extend operation and no field for one.
    pub fn issue_currency(&self, cur: &CurrencyState, subject: &Keyhash) -> Option<Vec<u8>> {
        let rung = self.rung_for(cur, subject)?;
        let current = self.table.current_key(subject);
        Some(currency_attestation(&self.identity, subject, &current, self.now, self.now + cur.lifetime, rung.role()))
    }

    /// Answer a currency request.  Nothing about it is retained.
    pub fn answer_currency(&self, cur: &CurrencyState, req: &CurrencyRequest) -> CurrencyReply {
        match self.issue_currency(cur, &req.subject) {
            Some(bytes) => CurrencyReply::Attestation { nonce: req.nonce, bytes },
            None => CurrencyReply::CannotIssue { nonce: req.nonce },
        }
    }

    /// Verify an attestation and read it against this node's own clock and
    /// topology.  `patrons` may add what an introduction's locator says the
    /// subject's patron is, where the table holds nothing.
    pub fn read_staple<L: Lookup + ?Sized>(&self, ids: &L, bytes: &[u8], subject: &Keyhash, patrons: &[Keyhash]) -> (Option<Attestation>, Staple) {
        let Ok(a) = rhtn_archive::currency::parse_attestation(ids, bytes) else { return (None, Staple::Absent) };
        let mut known = self.table.patrons(&self.table.current_key(subject));
        known.extend(patrons.iter().copied());
        let placed = |issuer: &Keyhash, role: u64| -> Option<bool> {
            match self.places_issuer(subject, issuer, role) {
                Some(v) => Some(v),
                None if patrons.contains(issuer) && role == ROLE_PATRON => Some(true),
                None => None,
            }
        };
        let s = staple_state(Some(&a), subject, self.now, &known, &placed);
        (Some(a), s)
    }

    /// The staple this node holds for `subject`, read now.
    pub fn staple_for<L: Lookup + ?Sized>(&self, ids: &L, subject: &Keyhash, patrons: &[Keyhash]) -> Staple {
        match self.staples.get(subject) {
            Some(bytes) => self.read_staple(ids, bytes, subject, patrons).1,
            None => Staple::Absent,
        }
    }

    /// Take a staple handed over with an introduction, keeping it only
    /// where it verifies and names the subject.
    pub fn take_staple<L: Lookup + ?Sized>(&mut self, ids: &L, subject: &Keyhash, bytes: &[u8], patrons: &[Keyhash]) -> Staple {
        let (att, state) = self.read_staple(ids, bytes, subject, patrons);
        if att.is_some() && state != Staple::WrongSubject {
            self.staples.insert(*subject, bytes.to_vec());
        }
        state
    }

    /// Learn which key is current for `subject` before addressing it
    /// (design §12.6.5): the held staple where it is current; otherwise the
    /// fallback query, sent to the introducer first where one is known, so
    /// the patron learns nothing it did not know.  Nothing waits on the
    /// answer; a party holding supersession evidence refuses instead.
    pub fn require_currency<L: Lookup + ?Sized>(&self, adj: &dyn Adjacency, ids: &L, subject: &Keyhash, introducer: Option<Keyhash>, patron: Option<Keyhash>) -> Requirement {
        if self.is_superseded(subject) {
            return Requirement::Settled(gate(true));
        }
        let hint: Vec<Keyhash> = patron.into_iter().collect();
        let state = self.staple_for(ids, subject, &hint);
        if state == Staple::Current {
            return Requirement::Settled(Gate::Proceed);
        }
        let nonce = rhtn_transport::tls::random_bytes();
        match self.ask_currency(adj, *subject, introducer, patron, nonce) {
            Some(ask) => Requirement::Asked(ask),
            // nobody to ask: address what is held, knowing it unattested
            None => Requirement::Settled(Gate::Proceed),
        }
    }

    /// Ask about `subject`: the introducer first, then the patron.
    pub fn ask_currency(&self, adj: &dyn Adjacency, subject: Keyhash, introducer: Option<Keyhash>, patron: Option<Keyhash>, nonce: [u8; 16]) -> Option<CurrencyAsk> {
        let mut ask = CurrencyAsk { subject, nonce, introducer, patron, asked: Vec::new() };
        let to = introducer.filter(|i| adj.has_session(i)).or(patron.filter(|p| adj.has_session(p)))?;
        adj.send(&to, crate::resolution::REQUEST_CURRENCY, &CurrencyRequest { subject, nonce }.encode());
        ask.asked.push(to);
        Some(ask)
    }

    /// Take the reply to an outstanding ask.  Code 1 from the introducer
    /// moves the question to the patron; code 1 from the patron exhausts
    /// it, and the caller concludes nothing.
    pub fn on_currency_reply<L: Lookup + ?Sized>(&mut self, adj: &dyn Adjacency, ids: &L, ask: &mut CurrencyAsk, reply: &CurrencyReply) -> AskStep {
        if reply.nonce() != ask.nonce {
            return AskStep::WrongNonce;
        }
        match reply {
            CurrencyReply::Attestation { bytes, .. } => {
                let hint: Vec<Keyhash> = ask.patron.into_iter().collect();
                match self.take_staple(ids, &ask.subject, bytes, &hint) {
                    Staple::Current => AskStep::Current,
                    other => AskStep::NotCurrent(other),
                }
            }
            CurrencyReply::CannotIssue { .. } => {
                let next = ask.patron.filter(|p| !ask.asked.contains(p) && adj.has_session(p));
                match next {
                    Some(p) => {
                        adj.send(&p, crate::resolution::REQUEST_CURRENCY, &CurrencyRequest { subject: ask.subject, nonce: ask.nonce }.encode());
                        ask.asked.push(p);
                        AskStep::AskedNext(p)
                    }
                    None => AskStep::Exhausted,
                }
            }
        }
    }
}

impl NodeView {
    /// The body of an adoption this node would countersign for `subject`.
    /// No staple gates it (design §12.6.5); a subject whose key this node
    /// knows superseded is refused, which is knowledge.  The body is fixed
    /// before either party signs, so the subject's own back-pointers are
    /// the subject's to supply (`wire-format.md` §3.1, §3.4); the child
    /// index is the lowest free slot under this node's position, and the
    /// slot is written when the signed adoption is applied.
    pub fn propose_adoption(&self, subject: &Keyhash, evidence: rhtn_archive::tx::Evidence, series: u32, subject_back: &[rhtn_archive::Txid]) -> Result<Vec<u8>, Gate> {
        match gate(self.is_superseded(subject)) {
            Gate::Proceed => {}
            refusal => return Err(refusal),
        }
        let slot = (0..10u8).find(|i| self.slots.get(&(*i as u64)).is_none_or(|s| s.occupant.is_none())).ok_or(Gate::Refuse("no free slot"))?;
        let mut indices = crate::resolution::Path { bytes: self.position.path.clone(), nibbles: self.position.nibbles }.indices();
        indices.push(slot);
        let path = crate::resolution::Path::from_indices(&indices);
        let back_self = self.archive.next_back_pointers();
        let a = rhtn_archive::tx::Adoption {
            node: *subject,
            patron: self.me(),
            locator: rhtn_archive::tx::Locator { anchor: self.anchor(), path: path.bytes, nibbles: path.nibbles, seqno: rhtn_archive::tx::Seqno { series, counter: 0 } },
            timestamp: self.now,
            key_material: None,
            evidence,
            presented_head: None,
            back: [subject_back, &back_self],
        };
        Ok(rhtn_archive::tx::adoption_body(&a))
    }

    /// Countersign a proposed adoption body with this node's key, given the
    /// subject's signature entries were gathered separately; the result is
    /// this node's own transaction and advances its chain.
    pub fn countersign_adoption(&mut self, body: &[u8], subject_signer: &rhtn_crypto::SigningIdentity) -> Option<rhtn_archive::record::Record> {
        let env = rhtn_archive::tx::envelope(rhtn_archive::tx::TYPE_ADOPTION, body, &[subject_signer, &self.identity]);
        let rec = rhtn_archive::record::Record::parse(&env).ok()?;
        self.archive.append(rec.clone()).ok()?;
        Some(rec)
    }
}
