//! Verifier selection against the selection vectors
//! (`test-vectors/verifier-selection.md`): the criterion's rows, the window
//! boundaries, and the curated bundle read as the selector reads it.

mod common;

use common::*;
use rhtn_client::selection::*;
use std::collections::BTreeSet;

#[test]
fn the_criterion_and_the_window_match_the_vectors() {
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
    let started = 1_767_268_800u64;
    assert!(!in_window(1_704_196_800, started), "exactly 730 days: out");
    assert!(in_window(1_704_196_801, started));
    assert!(in_window(1_767_268_799, started));
    assert!(!in_window(started, started), "not before started_at");
    assert!(in_window(1, 1000), "the lower bound saturates at zero");
}

// acceptance: CER-27
#[test]
fn n_counts_qualifying_records_once_by_txid_and_the_pool_excludes_the_counterparty() {
    // the vectors' bar 3, restated over records signed here: alice's history
    // with carol, then a meeting with bob, handed to bob for his selection
    let mut w = World::new();
    let f = w.meet("alice", "carol");
    let ac1 = w.meet("alice", "carol");
    let ac2 = w.meet("alice", "carol");
    let ab = w.meet("alice", "bob");
    let mut forged = f.bytes.clone();
    *forged.last_mut().unwrap() ^= 1;
    let started = w.clock + 7200;
    let bundle = vec![
        f.bytes.clone(),
        ac1.bytes.clone(),
        ac2.bytes.clone(),
        ac1.bytes.clone(),
        ab.bytes.clone(),
        forged,
    ];
    let p = pool(&ids(), &bundle, &kh("alice"), &kh("bob"), started);
    assert_eq!(
        p.n, 4,
        "distinct qualifying records: the formation, ac1, ac2 and ab; ac1 once, the forgery not at all"
    );
    assert_eq!(
        p.candidates,
        BTreeSet::from([kh("carol")]),
        "bob, the current counterparty, is never a candidate"
    );
    assert_eq!(required(p.n, p.candidates.len()), 1);
    assert!(p.unverifiable.is_empty());
    // handed by the witness as subject: a witnessed ceremony's participants
    // met each other, not the witness
    let pw = pool(
        &ids(),
        std::slice::from_ref(&ac1.bytes),
        &kh("witness"),
        &kh("bob"),
        started,
    );
    assert_eq!((pw.n, pw.candidates.len()), (0, 0));
    // understatement is free: two records handed, a smaller claim
    let ps = pool(
        &ids(),
        &[ac1.bytes.clone(), ab.bytes.clone()],
        &kh("alice"),
        &kh("bob"),
        started,
    );
    assert_eq!(
        (
            ps.n,
            ps.candidates.len(),
            required(ps.n, ps.candidates.len())
        ),
        (2, 1, 1)
    );
    // a record whose witness key the selector lacks is unverifiable, apart from n
    let without: Vec<_> = ids()
        .into_iter()
        .filter(|i| i.keyhash != kh("witness"))
        .collect();
    let pu = pool(&without, &bundle, &kh("alice"), &kh("bob"), started);
    assert_eq!(
        pu.n, 1,
        "only the formation verifies without the witness's key"
    );
    assert_eq!(pu.unverifiable.len(), 3);
    assert!(
        pu.unverifiable
            .iter()
            .all(|(_, missing)| *missing == kh("witness"))
    );
    // outside the window nothing qualifies
    let late = pool(&ids(), &bundle, &kh("alice"), &kh("bob"), ab.effective);
    assert_eq!(
        late.n, 3,
        "a record finalizing at this ceremony's own start is out"
    );
    let far = pool(
        &ids(),
        &bundle,
        &kh("alice"),
        &kh("bob"),
        started + WINDOW_SECONDS + 10,
    );
    assert_eq!(far.n, 0);
}

#[test]
fn selection_runs_by_the_selectors_own_tiers_and_marks_the_fill() {
    let pool = Pool {
        n: 8,
        candidates: BTreeSet::from([kh("carol"), kh("w1"), kh("w2"), kh("w3"), kh("c1")]),
        records: vec![],
        unverifiable: vec![],
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
