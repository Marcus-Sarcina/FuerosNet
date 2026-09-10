//! The topology store across a restart (`infra-client-requirements.md`
//! §4.3): it is the seen-set, so a node that kept it does not replay a
//! forwarding wave when the objects it held come round again.

mod common;

use common::*;
use rhtn_archive::tx::Seqno;
use rhtn_node::propagation::{FRAME_TOPOLOGY_PUSH, encode_push};
use rhtn_node::resolution::endpoint_record;
use rhtn_node::store::{Decision, KIND_ENDPOINT_RECORD, KIND_TRANSACTION, TopologyStore};

#[test]
fn a_saved_store_is_the_seen_set_after_a_restart() {
    let mut w = World::new();
    let (a_n, _) = w.adopt("bob", "alice", 1);
    let (a_s, _) = w.adopt("carol", "bob", 2);
    let (a_x, pop) = w.adopt("w1", "carol", 3);
    let table = table_with(kh("bob"), &w, &[&a_n, &a_s], &["alice", "bob", "carol"]);
    let mut n = view("bob", table, "alice", &[0]);
    n.now = w.clock + 1;
    let fab = Fabric::with(&[kh("alice"), kh("carol")]);
    let er = endpoint_record(&id("carol"), &[point(3, 7003)], Seqno { series: 2, counter: 4 });
    assert_eq!(n.receive_push(&*fab, &kh("alice"), &encode_push(KIND_TRANSACTION, &a_x.bytes), &ids()), Decision::Stored);
    assert_eq!(n.receive_push(&*fab, &kh("alice"), &encode_push(KIND_ENDPOINT_RECORD, &er), &ids()), Decision::Stored);
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
    assert_eq!(loaded.presence(&pop.txid), Some(&pop.bytes));
    assert!(loaded.series_proved(&kh("carol"), 2));
    assert_eq!(loaded.objects().len(), n.store.objects().len());
    let mut again = view("bob", table_with(kh("bob"), &w, &[&a_n, &a_s], &["alice", "bob", "carol"]), "alice", &[0]);
    again.store = loaded;
    // the objects come round again on another session: duplicates, and no
    // second forwarding wave
    let fab2 = Fabric::with(&[kh("alice"), kh("carol")]);
    assert_eq!(again.receive_push(&*fab2, &kh("carol"), &encode_push(KIND_TRANSACTION, &a_x.bytes), &ids()), Decision::Duplicate);
    assert_eq!(again.receive_push(&*fab2, &kh("carol"), &encode_push(KIND_ENDPOINT_RECORD, &er), &ids()), Decision::Duplicate);
    assert_eq!(fab2.count(FRAME_TOPOLOGY_PUSH), 0, "a node that kept its store replays nothing");
    let _ = std::fs::remove_dir_all(&dir);
}
