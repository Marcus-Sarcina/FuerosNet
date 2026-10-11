//! Verifier selection by recognition (`wire-format.md` §5; design §8.1.2):
//! the bundle a subject hands over as its own claim — introductions to the
//! prior counterparties it names (§2.3, §5.4) — its qualification (§5.3),
//! the reasonableness criterion (§5.2), and the selector's tiers over its
//! own knowledge (§5.1), computed locally and owed to nobody.

use crate::Keyhash;
use rhtn_archive::locator::Introduction;
use std::collections::{BTreeMap, BTreeSet};

/// One handed introduction that qualifies: the subject's prior
/// counterparty, verified, with the material to pin and the locator to
/// reach it by.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Introduced {
    /// The party introduced.
    pub subject: Keyhash,
    /// The introduction, as verified.
    pub introduction: Introduction,
}

/// What a bundle yields for one subject: `n`, the qualifying introductions
/// counted once by subject; the candidates, the introduced parties less
/// the current counterparty; the introductions themselves; and how many
/// entries did not qualify, which contribute nothing (§5.3, §5.4).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Pool {
    /// The qualifying introductions, counted once by subject.
    pub n: usize,
    /// The distinct prior counterparties, less the current one.
    pub candidates: BTreeSet<Keyhash>,
    /// The qualifying introductions, one per subject.
    pub introduced: Vec<Introduced>,
    /// Entries that did not qualify.
    pub refused: usize,
}

/// Read a bundle as the selector does (§5.3, §5.4): each introduction
/// verified alone — its material naming an identity, its locator's subject
/// that identity, its signature good under that material; an introduction
/// of `subject` itself not qualifying; counted once by subject, the greater
/// counter in the same series standing; the current `counterparty` never a
/// candidate for their own verification, though counted toward *n*.
/// Nothing is looked up: the introduction carries what verifies it.
pub fn pool(bundle: &[Vec<u8>], subject: &Keyhash, counterparty: &Keyhash) -> Pool {
    let mut out = Pool::default();
    let mut held: BTreeMap<Keyhash, Introduction> = BTreeMap::new();
    for bytes in bundle {
        let Ok(intro) = Introduction::parse(bytes) else {
            out.refused += 1;
            continue;
        };
        let who = intro.identity.keyhash;
        if who == *subject {
            out.refused += 1;
            continue;
        }
        match held.get(&who) {
            Some(have) if !supersedes(&intro, have) => {}
            _ => {
                held.insert(who, intro);
            }
        }
    }
    for (who, intro) in held {
        out.n += 1;
        if who != *counterparty {
            out.candidates.insert(who);
        }
        out.introduced.push(Introduced {
            subject: who,
            introduction: intro,
        });
    }
    out
}

/// Whether `new` replaces `have` as the locator held for one subject
/// (`wire-format.md` §2.3): the same series with a greater counter does;
/// series do not rank against each other, so a different series does not.
pub fn supersedes(new: &Introduction, have: &Introduction) -> bool {
    let (a, b) = (&new.locator.locator.seqno, &have.locator.locator.seqno);
    a.series == b.series && a.counter > b.counter
}

/// The reasonableness criterion (§5.2): `min(floor(n / 2), 10, |candidates|)`.
pub fn required(n: usize, candidates: usize) -> usize {
    (n / 2).min(10).min(candidates)
}

/// The selector's claim of why a verifier was picked (`wire-format.md`
/// §5.5 field 10), tier-aligned.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum SelectionBasis {
    /// Someone the selector has met.
    Met = 0,
    /// A member of one of its trust horizons.
    InHorizon = 1,
    /// One further edge over either of those.
    Reachable = 2,
    /// Picked on no stated acquaintance.
    Discretionary = 3,
}

/// What a selector knows, from its own records and tables (§5.1): the
/// people it has met, the members of any of its trust horizons, and the
/// one further edge over both.  Nobody audits it.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Acquaintance {
    /// The people this selector has met.
    pub met: BTreeSet<Keyhash>,
    /// The members of any of its trust horizons.
    pub horizon: BTreeSet<Keyhash>,
    /// One further edge over both of those.
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
