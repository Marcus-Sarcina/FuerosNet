//! Currency attestations: issuance, the escalation ladder when a patron is
//! unreachable, and the checks a relying party runs against its own clock
//! (design §12.6.5, §12.6.5.1; `wire-format.md` §7.1).
//!
//! Two halves of one rule, in different hands.  Issuing: issue only for the
//! key you currently record, fresh and never extended.  Relying: fail open
//! for what nobody else relies upon, fail closed for anything trust-bearing,
//! and never fail open on knowledge you hold.

use crate::view::NodeView;
use crate::{Adjacency, Keyhash};
use rhtn_archive::currency::Attestation;
use rhtn_archive::topology::Supersession;
use rhtn_archive::tx::currency_attestation;
use rhtn_codec::cbor::*;
use rhtn_codec::encode::*;
use rhtn_codec::schema::{self, Family};
use rhtn_crypto::verify::Lookup;
use std::collections::BTreeMap;

/// `issuer_role` (`wire-format.md` §7.1): the rungs of §12.6.5.1's ladder.
pub const ROLE_PATRON: u64 = 0;
pub const ROLE_SIBLING: u64 = 1;
pub const ROLE_GRANDPATRON: u64 = 2;
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
/// not reached, and a caller receiving neither fails closed
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
            _ => Ok(CurrencyReply::CannotIssue { nonce }),
        }
    }
}

/// How a relying party reads a staple, against its own clock (design §12.6.5).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Staple {
    Current,
    Expired,
    Absent,
    /// The issuer is not one this party accepts for the subject.
    WrongIssuer,
}

/// Whether an operation is one others may later rely upon (design §12.6.5).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Operation {
    /// Receiving messages, routine payload: nothing relied upon by others.
    Routine,
    /// Creates, transfers or spends social standing, establishes a new trust
    /// relationship, or produces evidence others may rely on.
    TrustBearing,
}

/// What a relying party does.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Gate {
    /// Proceed.
    Proceed,
    /// Fail closed: current control of the acting credential must be
    /// established first.
    Refuse(&'static str),
}

/// The fail-open / fail-closed table, plus the rule above it: fail-open is
/// for ignorance, never for knowledge (design §12.6.5).
pub fn gate(op: Operation, staple: Staple, superseded: bool) -> Gate {
    if superseded {
        // authenticated supersession evidence: nothing is served under the
        // old credential, whatever the staple says
        return Gate::Refuse("the binding is verified superseded");
    }
    match (op, staple) {
        (Operation::Routine, _) => Gate::Proceed,
        (Operation::TrustBearing, Staple::Current) => Gate::Proceed,
        (Operation::TrustBearing, Staple::Expired) => Gate::Refuse("the staple is expired"),
        (Operation::TrustBearing, Staple::Absent) => Gate::Refuse("no staple"),
        (Operation::TrustBearing, Staple::WrongIssuer) => Gate::Refuse("the issuer is not one this party accepts"),
    }
}

/// Read a staple against `now`, the relying party's own clock.
pub fn staple_state(att: Option<&Attestation>, subject: &Keyhash, now: u64) -> Staple {
    match att {
        None => Staple::Absent,
        Some(a) if a.subject != *subject => Staple::WrongIssuer,
        Some(a) if a.expires_at <= now => Staple::Expired,
        Some(_) => Staple::Current,
    }
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

/// A node's own currency state: the lifetime it issues for, the keys it
/// records for its subordinates, and the supersessions it has verified.
pub struct CurrencyState {
    pub lifetime: u64,
    /// The key this node currently records for a subject.  A recovery
    /// adoption overwrites it; after that there is nothing to issue for the
    /// old key.
    recorded: BTreeMap<Keyhash, Keyhash>,
    superseded: BTreeMap<Keyhash, Keyhash>,
    /// Parties this node believes unreachable, which is what opens the
    /// ladder's next rung.
    pub unreachable: std::collections::BTreeSet<Keyhash>,
}

impl Default for CurrencyState {
    fn default() -> Self {
        CurrencyState { lifetime: DEFAULT_LIFETIME_SECONDS, recorded: BTreeMap::new(), superseded: BTreeMap::new(), unreachable: Default::default() }
    }
}

impl CurrencyState {
    pub fn record(&mut self, subject: Keyhash, key: Keyhash) {
        self.recorded.insert(subject, key);
    }
    pub fn recorded_key(&self, subject: &Keyhash) -> Option<Keyhash> {
        self.recorded.get(subject).copied()
    }
    /// Take supersession evidence: the record moves to the successor and the
    /// old key has nothing left to issue for.
    pub fn supersede(&mut self, s: Supersession) {
        self.superseded.insert(s.superseded, s.successor);
        if s.successor != s.superseded {
            self.recorded.remove(&s.superseded);
            self.recorded.insert(s.successor, s.successor);
        }
    }
    pub fn is_superseded(&self, k: &Keyhash) -> bool {
        self.superseded.get(k).is_some_and(|s| s != k)
    }
    pub fn successor_of(&self, k: &Keyhash) -> Option<Keyhash> {
        self.superseded.get(k).copied().filter(|s| s != k)
    }
}

impl NodeView {
    /// The rung this node stands on for `subject`, if any.
    pub fn rung_for(&self, cur: &CurrencyState, subject: &Keyhash) -> Option<Rung> {
        let me = self.me();
        let patrons = self.table.patrons(subject);
        if patrons.contains(&me) {
            return Some(Rung::Patron);
        }
        // a sibling of the patron, holding the record by replication
        let unreachable_patron = patrons.iter().find(|p| cur.unreachable.contains(*p))?;
        if self.table.siblings(unreachable_patron).contains(&me) {
            return Some(Rung::Sibling);
        }
        // the grandpatron, once the patron and every sibling is unreachable
        if self.table.patrons(unreachable_patron).contains(&me) {
            let siblings = self.table.siblings(unreachable_patron);
            if siblings.iter().all(|s| cur.unreachable.contains(s)) {
                return Some(Rung::Grandpatron);
            }
        }
        None
    }

    /// Issue a fresh attestation for `subject`, on whatever rung this node
    /// stands.  Never an extension of an earlier one: there is no extend
    /// operation and no field for one.
    pub fn issue_currency(&self, cur: &CurrencyState, subject: &Keyhash) -> Option<Vec<u8>> {
        let rung = self.rung_for(cur, subject)?;
        // issue only for the key currently recorded; a superseded key has
        // nothing to issue for
        if cur.is_superseded(subject) {
            return None;
        }
        let current = cur.recorded_key(subject)?;
        if cur.is_superseded(&current) {
            return None;
        }
        Some(currency_attestation(&self.identity, subject, &current, self.now, self.now + cur.lifetime, rung.role()))
    }

    /// Answer a currency request.  Nothing about it is retained.
    pub fn answer_currency(&self, cur: &CurrencyState, req: &CurrencyRequest) -> CurrencyReply {
        match self.issue_currency(cur, &req.subject) {
            Some(bytes) => CurrencyReply::Attestation { nonce: req.nonce, bytes },
            None => CurrencyReply::CannotIssue { nonce: req.nonce },
        }
    }

    /// Ask about `subject`.  A caller with a stale staple asks its
    /// introducer first: that party already knows the caller is talking to
    /// the subject, where the patron learns something it did not know
    /// (design §12.6.5).
    pub fn ask_currency(&self, adj: &dyn Adjacency, subject: Keyhash, introducer: Option<Keyhash>, patron: Option<Keyhash>, nonce: [u8; 16]) -> Option<Keyhash> {
        let req = CurrencyRequest { subject, nonce };
        let to = introducer.filter(|i| adj.has_session(i)).or(patron.filter(|p| adj.has_session(p)))?;
        adj.send(&to, crate::resolution::REQUEST_CURRENCY, &req.encode());
        Some(to)
    }

    /// Verify an attestation and read it against this node's own clock.
    pub fn read_staple<L: Lookup + ?Sized>(&self, ids: &L, bytes: &[u8], subject: &Keyhash) -> (Option<Attestation>, Staple) {
        match rhtn_archive::currency::parse_attestation(ids, bytes) {
            Ok(a) => {
                let s = staple_state(Some(&a), subject, self.now);
                (Some(a), s)
            }
            Err(_) => (None, Staple::Absent),
        }
    }
}
