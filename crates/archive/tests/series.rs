//! The chain that proves a series (`wire-format.md` §4.6.1): what a holder
//! takes into it and what it refuses.

mod common;

use common::World;
use rhtn_archive::series::*;
use rhtn_archive::tx::*;

// acceptance: REC-08
#[test]
fn a_reissue_naming_a_series_already_in_the_chain_is_rejected_and_an_unused_one_extends_it() {
    let mut w = World::new(&["alice", "bob", "carol"]);
    let f = w.meet("alice", "bob");
    let a = w.adopt("bob", "alice", f.txid, 1);
    let r1 = w.reissue(
        "bob",
        "alice",
        Seqno {
            series: 1,
            counter: 5,
        },
        2,
    );
    let ids = w.lookup();
    // H holds the chain for S: adoption into s1, reissue to s2
    let mut chain = SeriesChain::from_adoption(&a, &ids).unwrap();
    chain.take_reissue(&r1, &ids).unwrap();
    assert_eq!(
        (chain.current(), chain.depth(), chain.series()),
        (2, 2, vec![1, 2])
    );
    // a patron-countersigned reissue back into s1 is rejected, and the chain is unchanged
    let back = w.reissue(
        "bob",
        "alice",
        Seqno {
            series: 2,
            counter: 3,
        },
        1,
    );
    let e = chain.take_reissue(&back, &ids).unwrap_err();
    assert!(e.contains("already in the chain"), "{e}");
    assert_eq!((chain.current(), chain.depth()), (2, 2));
    // one into an unused s3 extends it to length three
    let s3 = w.reissue(
        "bob",
        "alice",
        Seqno {
            series: 2,
            counter: 4,
        },
        3,
    );
    chain.take_reissue(&s3, &ids).unwrap();
    assert_eq!((chain.current(), chain.depth()), (3, 3));
    assert_eq!(
        chain.abandoned().into_iter().collect::<Vec<_>>(),
        vec![1, 2]
    );
    // a reissue leaving a series other than the current one is not this chain's next link
    let stray = w.reissue(
        "bob",
        "alice",
        Seqno {
            series: 1,
            counter: 9,
        },
        4,
    );
    assert!(
        chain
            .take_reissue(&stray, &ids)
            .unwrap_err()
            .contains("other than the current")
    );
    // nor is another relationship's
    let f2 = w.meet("bob", "carol");
    let _ = w.adopt("bob", "carol", f2.txid, 7);
    let other = w.reissue(
        "bob",
        "carol",
        Seqno {
            series: 7,
            counter: 1,
        },
        8,
    );
    assert!(
        chain
            .take_reissue(&other, &ids)
            .unwrap_err()
            .contains("another relationship")
    );
    assert_eq!(chain.depth(), 3);
    // presented and read back, it is the same chain; a shorter one it extends
    let read = SeriesChain::from_records(&chain.bytes(), &ids).unwrap();
    assert_eq!(read.series(), chain.series());
    assert_eq!(read.rank(&chain), Ranking::Same);
    let shorter = SeriesChain::from_records(&chain.bytes()[..2], &ids).unwrap();
    assert_eq!(chain.rank(&shorter), Ranking::Extends);
    assert_eq!(shorter.rank(&chain), Ranking::Extended);
    // the subject's own archive yields the same chain
    let own = w.archive("bob").chain_for(&w.kh("alice")).unwrap();
    assert_eq!(own.series(), vec![1, 2, 3]);
    assert_eq!(own.rank(&chain), Ranking::Same);
    // a presentation that skips the adoption, or repeats a series, does not read
    assert!(SeriesChain::from_records(&chain.bytes()[1..], &ids).is_err());
    let mut repeated = chain.bytes();
    repeated.push(back.bytes.clone());
    assert!(SeriesChain::from_records(&repeated, &ids).is_err());
}

#[test]
fn two_chains_from_one_adoption_that_diverge_are_equivocation() {
    let mut w = World::new(&["alice", "bob"]);
    let f = w.meet("alice", "bob");
    let a = w.adopt("bob", "alice", f.txid, 1);
    let ids = w.lookup();
    let x = w.reissue(
        "bob",
        "alice",
        Seqno {
            series: 1,
            counter: 5,
        },
        2,
    );
    let y = w.reissue(
        "bob",
        "alice",
        Seqno {
            series: 1,
            counter: 6,
        },
        3,
    );
    let mut cx = SeriesChain::from_adoption(&a, &ids).unwrap();
    cx.take_reissue(&x, &ids).unwrap();
    let mut cy = SeriesChain::from_adoption(&a, &ids).unwrap();
    cy.take_reissue(&y, &ids).unwrap();
    assert_eq!(cx.rank(&cy), Ranking::Diverge);
    assert_eq!(cy.rank(&cx), Ranking::Diverge);
}

// acceptance: ARC-19
#[test]
fn a_presented_reissue_timed_before_the_record_it_follows_is_refused() {
    let mut w = World::new(&["alice", "bob"]);
    let f = w.meet("alice", "bob");
    // the adoption's own timestamp, and a reissue dated one second before it
    let a = w.adopt("bob", "alice", f.txid, 1);
    let early = w.loose_reissue_at(
        "bob",
        "alice",
        Seqno {
            series: 1,
            counter: 5,
        },
        2,
        a.time - 1,
    );
    let ids = w.lookup();
    // everything verifies and the predecessor is in the presentation, so
    // this is chronology and not unavailable history
    assert_eq!(
        early.check_signatures(&ids),
        rhtn_archive::record::SigStatus::Verified
    );
    let e = SeriesChain::from_records(&[a.bytes.clone(), early.bytes.clone()], &ids).unwrap_err();
    assert!(e.contains("before its predecessor"), "{e}");
    // and taken into a held chain it is refused the same way, leaving it
    let mut chain = SeriesChain::from_adoption(&a, &ids).unwrap();
    assert!(
        chain
            .take_reissue(&early, &ids)
            .unwrap_err()
            .contains("before its predecessor")
    );
    assert_eq!((chain.current(), chain.depth()), (1, 1));
    // the same chain dated after its predecessor is taken
    let later = w.loose_reissue_at(
        "bob",
        "alice",
        Seqno {
            series: 1,
            counter: 6,
        },
        3,
        a.time + 1,
    );
    let ok =
        SeriesChain::from_records(&[a.bytes.clone(), later.bytes.clone()], &ids).expect("in order");
    assert_eq!((ok.current(), ok.depth()), (3, 2));
    // a second reissue is held to the one before it, not only to the adoption
    let back = w.loose_reissue_at(
        "bob",
        "alice",
        Seqno {
            series: 3,
            counter: 1,
        },
        4,
        later.time - 1,
    );
    let e = SeriesChain::from_records(&[a.bytes, later.bytes, back.bytes], &ids).unwrap_err();
    assert!(e.contains("before its predecessor"), "{e}");
}
