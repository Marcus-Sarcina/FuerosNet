//! What a node holds for a prerequisite that has not arrived, and the
//! order at the ceiling (`wire-format.md` §10): the reconstructible goes
//! first, and what ends a relationship outlasts it.
//!
//! Each property is run twice: by the gate at a reduced ceiling, since the
//! order at the ceiling does not depend on where the ceiling stands, and
//! as an ignored load test at the production ceiling, which takes
//! hundreds of signed and verified records to reach.  The ceiling is read
//! from the store, never assumed.

mod common;

use common::*;
use rhtn_archive::tx::Seqno;
use rhtn_crypto::Identity;
use rhtn_node::Keyhash;
use rhtn_node::resolution::{self, NetworkPoint};
use rhtn_node::store::{
    Decision, Horizon, KIND_ENDPOINT_RECORD, KIND_TRANSACTION, PENDING_HELD, TopologyStore,
};

/// The gate's ceiling: enough to show the order at it, in seconds.
const GATE_CEILING: usize = 8;

/// Everyone is within the store horizon: what a test that is not about
/// the horizon wants.
struct Everywhere;
impl Horizon for Everywhere {
    fn within(&self, _: &Keyhash, _: usize) -> bool {
        true
    }
}

fn er(name: &str, points: &[NetworkPoint], seqno: Seqno) -> Vec<u8> {
    resolution::endpoint_record(&id(name), points, seqno)
}

/// Every identity but those named: a holder that has yet to learn their
/// keys, and so holds what they signed.
fn without(names: &[&str]) -> Vec<Identity> {
    ids()
        .into_iter()
        .filter(|i| !names.iter().any(|n| i.keyhash == kh(n)))
        .collect()
}

fn holds(st: &TopologyStore, bytes: &[u8]) -> bool {
    st.pending().iter().any(|p| p.bytes == bytes)
}

fn gate_store() -> TopologyStore {
    TopologyStore::with_pending_ceiling_for_tests(GATE_CEILING)
}

fn a_flood_of_the_reconstructible_never_displaces_a_held_disavowal(new: fn() -> TopologyStore) {
    // alice's disavowal of carol arrives from a peer before alice's key is
    // known, and is held for it.  Then w1 publishes an endpoint record in
    // one series, and a flood of records in series no chain has proved,
    // each held for one, past the ceiling and on.  The disavowal is the
    // oldest held item and still outlasts them all: what goes is the
    // oldest endpoint record, which a later locator distribution rebuilds
    let mut w = World::new();
    let mut st = new();
    let n = st.pending_ceiling();
    let all = ids();
    let dis = w.disavow("alice", "carol", Some(0));
    let dis_bytes = w.bytes(&dis.txid);
    match st.accept(
        KIND_TRANSACTION,
        &dis_bytes,
        &kh("bob"),
        &without(&["alice"]),
        &Everywhere,
    ) {
        Decision::Held(p) => {
            assert_eq!(p.missing_key, Some(kh("alice")));
            assert!(p.trust_reducing, "a disavowal reduces a relationship");
        }
        other => panic!("{other:?}"),
    }
    let first = er(
        "w1",
        &[point(1, 5000)],
        Seqno {
            series: 1,
            counter: 1,
        },
    );
    assert_eq!(
        st.accept(KIND_ENDPOINT_RECORD, &first, &kh("bob"), &all, &Everywhere),
        Decision::Stored,
        "a subject's first line is taken as gossip"
    );
    let mut flood = Vec::new();
    for i in 0..n + 16 {
        let series = u32::try_from(i).unwrap() + 2;
        let rec = er("w1", &[point(1, 5000)], Seqno { series, counter: 1 });
        match st.accept(KIND_ENDPOINT_RECORD, &rec, &kh("bob"), &all, &Everywhere) {
            Decision::Held(p) => {
                assert_eq!(p.unproved_series, Some((kh("w1"), series)));
                assert!(!p.trust_reducing, "an endpoint record is reconstructible");
            }
            other => panic!("{other:?}"),
        }
        flood.push(rec);
        assert!(st.pending().len() <= n, "never past the ceiling");
        assert!(
            holds(&st, &dis_bytes),
            "the disavowal, the oldest held item, outlasts record {i}"
        );
    }
    assert_eq!(st.pending().len(), n, "the ceiling holds");
    // what went was the flood's own oldest, in arrival order
    let gone = flood.len() + 1 - n;
    for (i, rec) in flood.iter().enumerate() {
        assert_eq!(holds(&st, rec), i >= gone, "flood record {i}");
    }
    // alice's key arrives: the disavowal is what is ready, and it enters
    let ready = st.release_pending(&all);
    assert_eq!(ready.len(), 1);
    assert_eq!(ready[0].bytes, dis_bytes);
    assert_eq!(
        st.accept(KIND_TRANSACTION, &dis_bytes, &kh("bob"), &all, &Everywhere),
        Decision::Stored
    );
    assert_eq!(
        st.pending().len(),
        n - 1,
        "the endpoint records still wait on their chains"
    );
}

fn with_nothing_reconstructible_left_the_oldest_trust_reducing_item_goes(
    new: fn() -> TopologyStore,
) {
    // alice's disavowal of carol is held for alice's key; then bob, whose
    // key is also unknown, disavows w2 over and over, each a distinct act
    // held for his key.  One short of the ceiling alice's is still held;
    // at the ceiling, with nothing reconstructible to let go, it goes as
    // the oldest of its kind and nothing newer does
    let mut w = World::new();
    let mut st = new();
    let n = st.pending_ceiling();
    let short = without(&["alice", "bob"]);
    let first = w.disavow("alice", "carol", Some(0));
    let first_bytes = w.bytes(&first.txid);
    assert!(matches!(
        st.accept(
            KIND_TRANSACTION,
            &first_bytes,
            &kh("w1"),
            &short,
            &Everywhere
        ),
        Decision::Held(_)
    ));
    let mut flood = Vec::new();
    let mut flood_one = |st: &mut TopologyStore, w: &mut World| {
        let d = w.disavow("bob", "w2", Some(0));
        let bytes = w.bytes(&d.txid);
        match st.accept(KIND_TRANSACTION, &bytes, &kh("w1"), &short, &Everywhere) {
            Decision::Held(p) => assert!(p.trust_reducing),
            other => panic!("{other:?}"),
        }
        flood.push(bytes);
    };
    for _ in 0..n - 1 {
        flood_one(&mut st, &mut w);
    }
    assert_eq!(st.pending().len(), n);
    assert!(
        holds(&st, &first_bytes),
        "one short of the ceiling, the first disavowal is still held"
    );
    flood_one(&mut st, &mut w);
    assert_eq!(st.pending().len(), n, "the ceiling holds");
    assert!(
        !holds(&st, &first_bytes),
        "the oldest of its kind went to make room"
    );
    assert!(
        flood.iter().all(|b| holds(&st, b)),
        "nothing newer went in its place"
    );
    // alice's key arrives and nothing is ready for it: what was held for
    // it is gone, not waiting
    assert!(st.release_pending(&without(&["bob"])).is_empty());
}

// acceptance: TOP-46
#[test]
fn a_flood_of_the_reconstructible_never_displaces_a_held_disavowal_at_the_gate_ceiling() {
    // the default is checked here and exercised by the load test below
    assert_eq!(TopologyStore::new().pending_ceiling(), PENDING_HELD);
    a_flood_of_the_reconstructible_never_displaces_a_held_disavowal(gate_store);
}

// acceptance: TOP-46
#[test]
fn with_nothing_reconstructible_left_the_oldest_trust_reducing_item_goes_at_the_gate_ceiling() {
    with_nothing_reconstructible_left_the_oldest_trust_reducing_item_goes(gate_store);
}

// acceptance: TOP-46
#[test]
#[ignore = "load: production-size ceiling, about two minutes in debug; run with --ignored when topology.rs or store.rs change and before any release"]
fn a_flood_of_the_reconstructible_never_displaces_a_held_disavowal_at_the_production_ceiling() {
    a_flood_of_the_reconstructible_never_displaces_a_held_disavowal(TopologyStore::new);
}

// acceptance: TOP-46
#[test]
#[ignore = "load: production-size ceiling, about two minutes in debug; run with --ignored when topology.rs or store.rs change and before any release"]
fn with_nothing_reconstructible_left_the_oldest_trust_reducing_item_goes_at_the_production_ceiling()
{
    with_nothing_reconstructible_left_the_oldest_trust_reducing_item_goes(TopologyStore::new);
}
