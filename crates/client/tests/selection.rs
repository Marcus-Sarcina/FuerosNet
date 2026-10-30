//! Verifier selection against the selection vectors
//! (`test-vectors/verifier-selection.md`): the criterion's rows and the
//! curated bundle of introductions read as the selector reads it.

mod common;

use common::*;
use rhtn_archive::locator::{introduction, signed_locator};
use rhtn_archive::tx::{Locator, Seqno};
use rhtn_client::selection::*;
use std::collections::BTreeSet;

#[test]
fn the_criterion_matches_the_vectors() {
    for (n, c, want) in [
        (0, 0, 0),
        (1, 1, 0),
        (2, 1, 1),
        (3, 2, 1),
        (7, 5, 3),
        (20, 1, 1),
        (25, 12, 10),
    ] {
        assert_eq!(required(n, c), want, "n={n} candidates={c}");
    }
}

/// An introduction of `who` at `seqno`, as a holder hands it: their key
/// material with a locator they signed.
fn intro(who: &str, series: u32, counter: u32) -> Vec<u8> {
    let signer = id(who);
    let loc = Locator::root(kh(who), Seqno { series, counter });
    introduction(
        &signer.public.key_material(),
        &signed_locator(&signer, &loc),
    )
}

// acceptance: CER-27
#[test]
fn n_counts_introductions_once_by_subject_and_the_pool_excludes_the_counterparty() {
    // the vectors' bar 3, restated over introductions signed here: alice
    // hands bob the prior counterparties she names, each with the most
    // recent locator she holds for them (`wire-format.md` §2.3, §5.4)
    let c1_loc = Locator::root(
        kh("c1"),
        Seqno {
            series: 2,
            counter: 7,
        },
    );
    let mismatch = introduction(
        &id("alice").public.key_material(),
        &signed_locator(&id("c1"), &c1_loc),
    );
    let bundle = vec![
        intro("c1", 2, 7),
        intro("carol", 1, 0),
        intro("c1", 2, 9),
        intro("bob", 1, 0),
        mismatch,
        intro("alice", 1, 0),
    ];
    let p = pool(&bundle, &kh("alice"), &kh("bob"));
    assert_eq!(
        p.n, 3,
        "distinct qualifying subjects: c1, carol and bob; c1 once, the mismatch and alice herself not at all"
    );
    assert_eq!(
        p.candidates,
        BTreeSet::from([kh("c1"), kh("carol")]),
        "bob, the current counterparty, is never a candidate"
    );
    assert_eq!(required(p.n, p.candidates.len()), 1);
    assert_eq!(
        p.refused, 2,
        "the mismatch and the self-introduction contribute nothing"
    );
    let c1 = p
        .introduced
        .iter()
        .find(|i| i.subject == kh("c1"))
        .expect("c1 introduced");
    assert_eq!(
        c1.introduction.locator.locator.seqno.counter, 9,
        "the greater counter in the same series stands"
    );
    // a different series does not rank against the one held
    let p2 = pool(
        &[intro("c1", 2, 9), intro("c1", 3, 1)],
        &kh("alice"),
        &kh("bob"),
    );
    assert_eq!(
        p2.introduced[0].introduction.locator.locator.seqno.series,
        2
    );
    // understatement is free: one introduction handed, a smaller claim
    let ps = pool(&[intro("carol", 1, 0)], &kh("alice"), &kh("bob"));
    assert_eq!(
        (
            ps.n,
            ps.candidates.len(),
            required(ps.n, ps.candidates.len())
        ),
        (1, 1, 0)
    );
    // nothing is looked up: a stranger's introduction verifies by itself
    let stranger = pool(&[intro("w7", 1, 0)], &kh("alice"), &kh("bob"));
    assert_eq!((stranger.n, stranger.refused), (1, 0));
}

#[test]
fn selection_runs_by_the_selectors_own_tiers_and_marks_the_fill() {
    let pool = Pool {
        n: 8,
        candidates: BTreeSet::from([kh("carol"), kh("w1"), kh("w2"), kh("w3"), kh("c1")]),
        introduced: vec![],
        refused: 0,
    };
    let me = Acquaintance {
        met: BTreeSet::from([kh("w2")]),
        horizon: BTreeSet::from([kh("w1"), kh("carol")]),
        reachable: BTreeSet::from([kh("w3")]),
    };
    assert!(me.recognises_any(&pool));
    let picked = select(&pool, &me, 4);
    assert_eq!(picked[0], (kh("w2"), SelectionBasis::Met));
    assert_eq!(picked[1].1, SelectionBasis::InHorizon);
    assert_eq!(picked[2].1, SelectionBasis::InHorizon);
    assert_eq!(picked[3], (kh("w3"), SelectionBasis::Reachable));
    // recognition ranks the pool and never shortens it [author,
    // 2026-10-30]: the fifth is a stranger, marked as such
    let filled = select(&pool, &me, 5);
    assert_eq!(filled.len(), 5);
    assert_eq!(filled[4], (kh("c1"), SelectionBasis::Discretionary));
    let stranger = Acquaintance::default();
    assert!(
        !stranger.recognises_any(&pool),
        "a pool holding nobody the selector knows"
    );
    // and a selector who knows nobody still asks: one stranger is better
    // than no verifier
    let asked = select(&pool, &stranger, 3);
    assert_eq!(asked.len(), 3);
    assert!(
        asked
            .iter()
            .all(|(_, b)| *b == SelectionBasis::Discretionary)
    );
}

/// **How many are asked**: the criterion's ceiling, and at least one
/// wherever one is offered [author, 2026-10-30].
#[test]
fn at_least_one_is_asked_wherever_one_is_offered_and_never_more_than_the_ceiling() {
    use rhtn_client::selection::attempt;
    assert_eq!(attempt(0, 0), 0, "nobody to ask");
    assert_eq!(
        attempt(1, 1),
        1,
        "one prior counterparty is asked, required or not"
    );
    assert_eq!(required(1, 1), 0, "and nothing is required of them");
    assert_eq!(attempt(2, 1), 1);
    assert_eq!(attempt(7, 7), 3);
    assert_eq!(attempt(40, 40), 10, "ten is the ceiling");
    assert_eq!(attempt(40, 4), 4, "and the pool bounds it");
}
