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
    let picked = select(&pool, &me, 4, false);
    assert_eq!(picked[0], (kh("w2"), SelectionBasis::Met));
    assert_eq!(picked[1].1, SelectionBasis::InHorizon);
    assert_eq!(picked[2].1, SelectionBasis::InHorizon);
    assert_eq!(picked[3], (kh("w3"), SelectionBasis::Reachable));
    assert_eq!(
        select(&pool, &me, 5, false).len(),
        4,
        "nobody unrecognised without the fill"
    );
    let filled = select(&pool, &me, 5, true);
    assert_eq!(filled.len(), 5);
    assert_eq!(filled[4], (kh("c1"), SelectionBasis::Discretionary));
    let stranger = Acquaintance::default();
    assert!(
        !stranger.recognises_any(&pool),
        "a pool holding nobody the selector knows"
    );
    assert!(select(&pool, &stranger, 3, false).is_empty());
}
