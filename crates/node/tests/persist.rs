//! The topology store across a restart (`infra-client-requirements.md`
//! §4.3): it is the seen-set, so a node that kept it does not replay a
//! forwarding wave when the objects it held come round again.

mod common;

use common::*;
use rhtn_archive::tx::Seqno;
use rhtn_node::propagation::{FRAME_TOPOLOGY_PUSH, encode_push};
use rhtn_node::resolution::endpoint_record;
use rhtn_node::store::{Decision, KIND_ENDPOINT_RECORD, KIND_TRANSACTION, TopologyStore};

// acceptance: TOP-45
#[test]
fn a_saved_store_is_the_seen_set_after_a_restart() {
    let mut w = World::new();
    let (a_n, _) = w.adopt("bob", "alice", 1);
    let (a_s, _) = w.adopt("carol", "bob", 2);
    let (a_x, pop) = w.adopt("w1", "carol", 3);
    let table = table_with(kh("bob"), &w, &[&a_n, &a_s], &["alice", "bob", "carol"]);
    let mut n = view("bob", table, "alice", &[0]);
    n.set_now(w.clock + 1);
    let fab = Fabric::with(&[kh("alice"), kh("carol")]);
    let er = endpoint_record(
        &id("carol"),
        &[point(3, 7003)],
        Seqno {
            series: 2,
            counter: 4,
        },
    );
    assert_eq!(
        n.receive_push(
            &*fab,
            &kh("alice"),
            &encode_push(KIND_TRANSACTION, &a_x.bytes),
            &ids()
        ),
        Decision::Stored
    );
    assert_eq!(
        n.receive_push(
            &*fab,
            &kh("alice"),
            &encode_push(KIND_ENDPOINT_RECORD, &er),
            &ids()
        ),
        Decision::Stored
    );
    n.store.keep_presence(pop.txid, pop.bytes.clone());
    n.store.prove_series(kh("carol"), 2);
    let forwarded = fab.count(FRAME_TOPOLOGY_PUSH);
    assert!(forwarded > 0);

    // the node stops, and a new one starts from what the first saved
    let dir = std::env::temp_dir().join(format!("rhtn-store-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    n.store.save(&dir).unwrap();
    let loaded = TopologyStore::load(&dir).unwrap();
    assert!(loaded.holds_txid(&a_x.txid));
    assert_eq!(loaded.endpoint(&kh("carol")).unwrap().bytes, er);
    // **a record composing the horizon is kept as the record**
    // (`infra-client-requirements.md` §4.3 [author, 2026-11-25]): the
    // adoption survives the restart whole, though this node signed none
    // of it, and its identifier still means no second forwarding wave
    // leaves here.  The presence record it cited is not such a record,
    // and does not
    assert_eq!(loaded.presence(&pop.txid), None);
    assert_eq!(
        loaded.transaction(&a_x.txid).map(|r| r.bytes.clone()),
        Some(a_x.bytes.clone())
    );
    assert!(loaded.series_proved(&kh("carol"), 2));
    assert_eq!(loaded.objects().len(), n.store.objects().len());
    let mut again = view(
        "bob",
        table_with(kh("bob"), &w, &[&a_n, &a_s], &["alice", "bob", "carol"]),
        "alice",
        &[0],
    );
    again.store = loaded;
    // the objects come round again on another session: duplicates, and no
    // second forwarding wave
    let fab2 = Fabric::with(&[kh("alice"), kh("carol")]);
    assert_eq!(
        again.receive_push(
            &*fab2,
            &kh("carol"),
            &encode_push(KIND_TRANSACTION, &a_x.bytes),
            &ids()
        ),
        Decision::Duplicate
    );
    assert_eq!(
        again.receive_push(
            &*fab2,
            &kh("carol"),
            &encode_push(KIND_ENDPOINT_RECORD, &er),
            &ids()
        ),
        Decision::Duplicate
    );
    assert_eq!(
        fab2.count(FRAME_TOPOLOGY_PUSH),
        0,
        "a node that kept its store replays nothing"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

// acceptance: TOP-56
// functional: APP-019
#[test]
fn a_member_served_from_above_is_handed_their_adoption_before_and_after_a_restart() {
    // bob runs the node; carol holds a slot there and runs none; w1 is
    // carol's, so bob's node is the one w1 is served by
    let mut w = World::new();
    let (a_c, _) = w.adopt("carol", "bob", 1);
    let (a_w, _) = w.adopt("w1", "carol", 1);
    let fab = Fabric::with(&[kh("carol")]);
    let mut n = view("bob", table_with(kh("bob"), &w, &[], &["bob"]), "bob", &[]);
    for r in [&a_c, &a_w] {
        assert_eq!(
            n.take_object(&*fab, &kh("carol"), KIND_TRANSACTION, &r.bytes, &ids()),
            Decision::Stored
        );
    }
    assert_eq!(n.placement_of(&kh("carol")), Some(a_c.bytes.clone()));
    assert_eq!(
        n.placement_of(&kh("w1")),
        Some(a_w.bytes.clone()),
        "the record w1 and carol signed, from the node that serves w1"
    );
    assert_eq!(n.placement_of(&kh("alice")), None, "nobody else's");

    // **the node stops and starts**: the record is one it kept, so the
    // answer is the same, and the record coming round again is a duplicate
    let dir = std::env::temp_dir().join(format!("rhtn-store-above-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    n.store.save(&dir).unwrap();
    let mut again = view("bob", table_with(kh("bob"), &w, &[], &["bob"]), "bob", &[]);
    again.store = TopologyStore::load(&dir).unwrap();
    again.restore_materialised(None, &ids());
    let _ = std::fs::remove_dir_all(&dir);
    assert_eq!(again.placement_of(&kh("w1")), Some(a_w.bytes.clone()));
    assert_eq!(again.placement_of(&kh("carol")), Some(a_c.bytes.clone()));
    assert_eq!(
        again.take_object(&*fab, &kh("carol"), KIND_TRANSACTION, &a_w.bytes, &ids()),
        Decision::Duplicate
    );

    // a patron that runs a node of its own is the one to say it
    again.table.mark_infra(kh("carol"));
    assert_eq!(again.placement_of(&kh("w1")), None);

    // and a member that left is handed nothing
    let d = w.depart(
        "w1",
        "carol",
        Seqno {
            series: 1,
            counter: 1,
        },
    );
    assert_eq!(
        n.take_object(&*fab, &kh("carol"), KIND_TRANSACTION, &d.bytes, &ids()),
        Decision::Stored
    );
    assert_eq!(n.placement_of(&kh("w1")), None);
}

// acceptance: PRP-20
#[test]
fn a_retired_endpoint_record_does_not_return_from_disk() {
    let mut w = World::new();
    let (a_n, _) = w.adopt("bob", "alice", 1);
    let (a_s, _) = w.adopt("carol", "bob", 2);
    let table = table_with(kh("bob"), &w, &[&a_n, &a_s], &["alice", "bob", "carol"]);
    let mut n = view("bob", table, "alice", &[0]);
    let fab = Fabric::with(&[kh("alice"), kh("carol")]);
    let sq = Seqno {
        series: 2,
        counter: 4,
    };
    let first = endpoint_record(&id("carol"), &[point(3, 7003)], sq);
    let second = endpoint_record(&id("carol"), &[point(3, 7004)], sq);
    assert_eq!(
        n.receive_push(
            &*fab,
            &kh("alice"),
            &encode_push(KIND_ENDPOINT_RECORD, &first),
            &ids()
        ),
        Decision::Stored
    );
    let dir = std::env::temp_dir().join(format!("rhtn-store-conflict-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    n.store.save(&dir).unwrap();
    // the same number with different signed contents: the pair is malformed
    // and neither is current
    assert!(matches!(
        n.receive_push(
            &*fab,
            &kh("carol"),
            &encode_push(KIND_ENDPOINT_RECORD, &second),
            &ids()
        ),
        Decision::Conflict { .. }
    ));
    assert!(n.store.endpoint(&kh("carol")).is_none());
    n.store.save(&dir).unwrap();
    let loaded = TopologyStore::load(&dir).unwrap();
    assert!(loaded.conflicted(&kh("carol"), sq), "the marker persists");
    assert!(
        loaded.endpoint(&kh("carol")).is_none(),
        "and the retired record does not return"
    );
    assert!(
        loaded
            .objects()
            .iter()
            .all(|(k, b)| *k != KIND_ENDPOINT_RECORD || *b != first),
        "nor is it replayed"
    );
    let _ = std::fs::remove_dir_all(&dir);
}
