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
    // and what a copy taken to save from writes is the same, file for file
    // (the service writes from one once the view's lock is released)
    let from_copy = std::env::temp_dir().join(format!("rhtn-store-copy-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&from_copy);
    n.store.persisted().save(&from_copy).unwrap();
    assert_eq!(contents_under(&dir), contents_under(&from_copy));
    let _ = std::fs::remove_dir_all(&from_copy);
    let loaded = TopologyStore::load(&dir).unwrap();
    assert!(loaded.holds_txid(&a_x.txid));
    assert_eq!(loaded.endpoint(&kh("carol")).unwrap().bytes, er);
    // **the record in force is kept, as proof and apart from what the
    // table is made of** (`infra-client-requirements.md` §4.3 [author,
    // 2026-11-25]): the adoption that places w1 now survives the restart
    // whole, though this node signed none of it, and its identifier still
    // means no second forwarding wave leaves here.  The bodies this run
    // folded are not read back, and the presence record the adoption
    // cited is no such record
    assert_eq!(loaded.presence(&pop.txid), None);
    assert!(loaded.transaction(&a_x.txid).is_none());
    assert_eq!(
        loaded.kept().map(|r| r.bytes.clone()).collect::<Vec<_>>(),
        vec![a_x.bytes.clone()]
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

    // **the node stops and starts**: its table comes back from the
    // derived view and the record is one it kept, so the answer is the
    // same, and the record coming round again is a duplicate
    let dir = std::env::temp_dir().join(format!("rhtn-store-above-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    n.store.save(&dir).unwrap();
    let derived = n.materialise();
    let mut again = view("bob", table_with(kh("bob"), &w, &[], &["bob"]), "bob", &[]);
    again.store = TopologyStore::load(&dir).unwrap();
    assert_eq!(
        again.restore_materialised(Some(&derived), &ids()),
        rhtn_archive::topology::Restored::Current
    );
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

/// What a saved store holds of other parties' records, as bodies.
fn kept_on_disk(n: &rhtn_node::view::NodeView, tag: &str) -> Vec<Vec<u8>> {
    let dir = std::env::temp_dir().join(format!("rhtn-store-{tag}-{}", std::process::id()));
    n.store.save(&dir).unwrap();
    let mut out: Vec<Vec<u8>> = files_under(&dir.join("tx"))
        .iter()
        .map(|f| std::fs::read(f).unwrap())
        .collect();
    out.sort();
    out
}

// acceptance: TOP-45
// functional: TOP-035
#[test]
fn what_is_kept_of_a_relationship_is_the_record_in_force_and_nothing_behind_it() {
    // bob runs the node; carol is his, and w1 is carol's, so every record
    // of w1 under carol is one bob's node signed none of
    let mut w = World::new();
    let (a_c, _) = w.adopt("carol", "bob", 1);
    let (a_w, _) = w.adopt("w1", "carol", 1);
    let fab = Fabric::with(&[kh("carol")]);
    let mut n = view("bob", table_with(kh("bob"), &w, &[], &["bob"]), "bob", &[]);
    let take = |n: &mut rhtn_node::view::NodeView, r: &rhtn_archive::record::Record| {
        assert_eq!(
            n.take_object(&*fab, &kh("carol"), KIND_TRANSACTION, &r.bytes, &ids()),
            Decision::Stored
        );
    };
    take(&mut n, &a_c);
    take(&mut n, &a_w);
    // **the adoption in force**; bob's own adoption of carol is among his
    // own acts and not repeated among these
    assert_eq!(kept_on_disk(&n, "force"), vec![a_w.bytes.clone()]);

    // **a series reissue is not kept**: the series a relationship stands
    // in is the table's to say, and the adoption is still what is in force
    let r1 = w.reissue(
        "w1",
        "carol",
        Seqno {
            series: 1,
            counter: 1,
        },
        5,
    );
    take(&mut n, &r1);
    assert_eq!(kept_on_disk(&n, "force"), vec![a_w.bytes.clone()]);
    let r2 = w.reissue(
        "w1",
        "carol",
        Seqno {
            series: 5,
            counter: 1,
        },
        6,
    );
    take(&mut n, &r2);
    assert_eq!(kept_on_disk(&n, "force"), vec![a_w.bytes.clone()]);
    assert_eq!(
        n.table
            .bindings()
            .iter()
            .find(|b| b.node == kh("w1") && b.open())
            .map(|b| b.series),
        Some(6),
        "and the table says where the series stands"
    );

    // **a restart holds the same**, and the table is not made from them:
    // with its derived view the node holds w1, and without it holds
    // nothing of w1 and still keeps the proof
    let dir = std::env::temp_dir().join(format!("rhtn-store-force-{}", std::process::id()));
    let derived = n.materialise();
    let mut again = view("bob", table_with(kh("bob"), &w, &[], &["bob"]), "bob", &[]);
    again.store = TopologyStore::load(&dir).unwrap();
    again.restore_materialised(Some(&derived), &ids());
    assert!(again.table.patrons(&kh("w1")).contains(&kh("carol")));
    assert_eq!(kept_on_disk(&again, "force-again"), vec![a_w.bytes.clone()]);
    let mut bare = view("bob", table_with(kh("bob"), &w, &[], &["bob"]), "bob", &[]);
    bare.store = TopologyStore::load(&dir).unwrap();
    assert_eq!(
        bare.restore_materialised(None, &ids()),
        rhtn_archive::topology::Restored::Replayed { replayed: 0 }
    );
    assert!(
        bare.table.patrons(&kh("w1")).is_empty(),
        "a kept record is proof and never the table's input"
    );
    assert_eq!(
        kept_on_disk(&bare, "force-bare"),
        vec![a_w.bytes.clone()],
        "and a table that says nothing of a relationship drops nothing of it"
    );

    // **a departure takes the adoption's place**
    let d = w.depart(
        "w1",
        "carol",
        Seqno {
            series: 6,
            counter: 1,
        },
    );
    take(&mut n, &d);
    assert_eq!(kept_on_disk(&n, "force"), vec![d.bytes.clone()]);

    // **a later adoption takes the departure's**, and a disavowal that
    // adoption's
    let (a_w2, _) = w.adopt("w1", "carol", 7);
    take(&mut n, &a_w2);
    assert_eq!(kept_on_disk(&n, "force"), vec![a_w2.bytes.clone()]);
    let x = w.disavow("carol", "w1", None);
    take(&mut n, &x);
    assert_eq!(kept_on_disk(&n, "force"), vec![x.bytes.clone()]);

    for tag in ["force", "force-again", "force-bare"] {
        let _ = std::fs::remove_dir_all(
            std::env::temp_dir().join(format!("rhtn-store-{tag}-{}", std::process::id())),
        );
    }
}

// acceptance: TOP-45
// functional: TOP-035
#[test]
fn what_is_kept_goes_when_neither_party_to_it_is_in_the_horizon_any_longer() {
    // bob runs the node; carol is his and w1 is carol's, two hops out
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
    n.settle_kept();
    assert_eq!(kept_on_disk(&n, "gone"), vec![a_w.bytes.clone()]);

    // **w1 leaves carol, and the ending is kept while carol is in view**:
    // one party to it remains
    let d_w = w.depart(
        "w1",
        "carol",
        Seqno {
            series: 1,
            counter: 1,
        },
    );
    assert_eq!(
        n.take_object(&*fab, &kh("carol"), KIND_TRANSACTION, &d_w.bytes, &ids()),
        Decision::Stored
    );
    n.settle_kept();
    assert_eq!(kept_on_disk(&n, "gone"), vec![d_w.bytes.clone()]);

    // **carol leaves bob, and neither party to that ending is in bob's
    // horizon any longer**: it goes, and what is kept is carol's own
    // ending under bob, a party to which is bob
    let d_c = w.depart(
        "carol",
        "bob",
        Seqno {
            series: 1,
            counter: 1,
        },
    );
    assert_eq!(
        n.take_object(&*fab, &kh("carol"), KIND_TRANSACTION, &d_c.bytes, &ids()),
        Decision::Stored
    );
    assert!(
        kept_on_disk(&n, "gone").contains(&d_w.bytes),
        "nothing goes until the node settles what it keeps"
    );
    n.settle_kept();
    assert_eq!(kept_on_disk(&n, "gone"), vec![d_c.bytes.clone()]);

    // **and a record whose parties come into view after a write-back
    // dropped it is taken again**, where this run still holds its body: a
    // later adoption of w1 by carol is two hops from nobody until carol is
    // bob's again
    let (a_w2, _) = w.adopt("w1", "carol", 2);
    assert_eq!(
        n.take_object(&*fab, &kh("bob"), KIND_TRANSACTION, &a_w2.bytes, &ids()),
        Decision::Stored,
        "bob's own identity pushes what his horizon does not yet reach"
    );
    n.settle_kept();
    assert!(!kept_on_disk(&n, "gone").contains(&a_w2.bytes));
    let (a_c2, _) = w.adopt("carol", "bob", 2);
    assert_eq!(
        n.take_object(&*fab, &kh("bob"), KIND_TRANSACTION, &a_c2.bytes, &ids()),
        Decision::Stored
    );
    n.settle_kept();
    assert!(kept_on_disk(&n, "gone").contains(&a_w2.bytes));

    let _ = std::fs::remove_dir_all(
        std::env::temp_dir().join(format!("rhtn-store-gone-{}", std::process::id())),
    );
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

/// Every file beneath `dir`, by its path from there, with its bytes.
fn contents_under(dir: &std::path::Path) -> Vec<(std::path::PathBuf, Vec<u8>)> {
    let mut out = Vec::new();
    let mut todo = vec![dir.to_path_buf()];
    while let Some(d) = todo.pop() {
        for e in std::fs::read_dir(&d).unwrap().flatten() {
            let p = e.path();
            if p.is_dir() {
                todo.push(p);
            } else {
                out.push((
                    p.strip_prefix(dir).unwrap().to_path_buf(),
                    std::fs::read(&p).unwrap(),
                ));
            }
        }
    }
    out.sort();
    out
}
