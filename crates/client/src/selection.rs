//! Verifier selection by recognition (`wire-format.md` §5; design §8.1.2):
//! the bundle a subject hands over as its own claim, qualification and the
//! window (§5.3), the reasonableness criterion (§5.2), and the selector's
//! tiers over its own knowledge (§5.1), computed locally and owed to
//! nobody.

use crate::{Keyhash, Txid};
use rhtn_archive::record::{Record, SigStatus};
use rhtn_archive::tx::TYPE_PRESENCE;
use rhtn_crypto::verify::Lookup;
use std::collections::{BTreeMap, BTreeSet};

/// 730 days of 86,400 seconds (`wire-format.md` §5.3).
pub const WINDOW_SECONDS: u64 = 730 * 86_400;

/// One handed record that qualifies: its txid, the subject's counterparty
/// in it, and when it finalized.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Qualified {
    pub txid: Txid,
    pub counterparty: Keyhash,
    pub finalized_at: u64,
}

/// What a bundle yields for one subject: `n`, the distinct qualifying
/// records counted once by txid; the candidates, distinct prior
/// counterparties less the current one; and the records themselves.
/// Records the selector could not verify for want of a key are listed
/// apart, unverifiable rather than invalid (`light-client-requirements.md`
/// §1.4).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Pool {
    pub n: usize,
    pub candidates: BTreeSet<Keyhash>,
    pub records: Vec<Qualified>,
    pub unverifiable: Vec<(Txid, Keyhash)>,
}

/// Whether `finalized_at` lies inside the window a ceremony starting at
/// `started_at` counts: exclusive at both ends, the lower bound saturating
/// at zero (§5.3).
pub fn in_window(finalized_at: u64, started_at: u64) -> bool {
    let floor = started_at.saturating_sub(WINDOW_SECONDS);
    floor < finalized_at && finalized_at < started_at
}

/// Read a bundle as the selector `me` does with the keys it holds
/// (§5.3, §5.4): each record verified alone, a presence record naming
/// `subject` as a participant, finalized inside the window; counted once
/// by txid; the current `counterparty` never a candidate for their own
/// verification.  A record that fails contributes nothing.
pub fn pool<L: Lookup + ?Sized>(
    ids: &L,
    bundle: &[Vec<u8>],
    subject: &Keyhash,
    counterparty: &Keyhash,
    started_at: u64,
) -> Pool {
    let mut out = Pool::default();
    let mut seen: BTreeMap<Txid, ()> = BTreeMap::new();
    for bytes in bundle {
        let Ok(rec) = Record::parse(bytes) else {
            continue;
        };
        if rec.tx_type != TYPE_PRESENCE || seen.contains_key(&rec.txid) {
            continue;
        }
        let parts = rec.participants();
        if !parts.contains(subject) || parts.len() != 2 {
            continue;
        }
        let other = if parts[0] == *subject {
            parts[1]
        } else {
            parts[0]
        };
        if !in_window(rec.effective, started_at) {
            continue;
        }
        match rec.check_signatures(ids) {
            SigStatus::Verified => {}
            SigStatus::Unverifiable { missing } => {
                if !out.unverifiable.iter().any(|(t, _)| *t == rec.txid) {
                    out.unverifiable.push((rec.txid, missing));
                }
                continue;
            }
            SigStatus::Invalid(_) => continue,
        }
        seen.insert(rec.txid, ());
        out.n += 1;
        if other != *counterparty {
            out.candidates.insert(other);
        }
        out.records.push(Qualified {
            txid: rec.txid,
            counterparty: other,
            finalized_at: rec.effective,
        });
    }
    out
}

/// The reasonableness criterion (§5.2): `min(floor(n / 2), 10, |candidates|)`.
pub fn required(n: usize, candidates: usize) -> usize {
    (n / 2).min(10).min(candidates)
}

/// The selector's claim of why a verifier was picked (`wire-format.md`
/// §5.5 field 10), tier-aligned.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum SelectionBasis {
    Met = 0,
    InHorizon = 1,
    Reachable = 2,
    Discretionary = 3,
}

/// What a selector knows, from its own records and tables (§5.1): the
/// people it has met, the members of any of its trust horizons, and the
/// one further edge over both.  Nobody audits it.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Acquaintance {
    pub met: BTreeSet<Keyhash>,
    pub horizon: BTreeSet<Keyhash>,
    pub reachable: BTreeSet<Keyhash>,
}

impl Acquaintance {
    /// The highest tier `k` falls in, or none.
    pub fn basis_of(&self, k: &Keyhash) -> Option<SelectionBasis> {
        if self.met.contains(k) {
            Some(SelectionBasis::Met)
        } else if self.horizon.contains(k) {
            Some(SelectionBasis::InHorizon)
        } else if self.reachable.contains(k) {
            Some(SelectionBasis::Reachable)
        } else {
            None
        }
    }

    /// Whether the selector recognises anybody in the pool: a candidate set
    /// holding nobody it knows defeats the check outright
    /// (`light-client-requirements.md` §1.4).
    pub fn recognises_any(&self, pool: &Pool) -> bool {
        pool.candidates.iter().any(|c| self.basis_of(c).is_some())
    }
}

/// Pick up to `required` verifiers from the pool in descending tier, each
/// with the basis claimed for it; `fill` says whether the remainder is
/// filled at the selector's discretion from strangers, marked as such.
pub fn select(
    pool: &Pool,
    me: &Acquaintance,
    required: usize,
    fill: bool,
) -> Vec<(Keyhash, SelectionBasis)> {
    let mut ranked: Vec<(SelectionBasis, Keyhash)> = pool
        .candidates
        .iter()
        .filter_map(|c| me.basis_of(c).map(|b| (b, *c)))
        .collect();
    ranked.sort();
    let mut out: Vec<(Keyhash, SelectionBasis)> = ranked
        .into_iter()
        .take(required)
        .map(|(b, k)| (k, b))
        .collect();
    if fill {
        for c in &pool.candidates {
            if out.len() >= required {
                break;
            }
            if !out.iter().any(|(k, _)| k == c) {
                out.push((*c, SelectionBasis::Discretionary));
            }
        }
    }
    out
}
