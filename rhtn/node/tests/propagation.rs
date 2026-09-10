//! Propagation entries (PRP): the forwarding rule, duplicate and conflict
//! suppression, and the rootward memo.

mod common;

use common::*;
use rhtn_archive::topology::Table;
use rhtn_archive::tx::Seqno;
use rhtn_node::propagation::*;
use rhtn_node::resolution::{self, NetworkPoint, REQUEST_RESOLVE, ResolveRequest};
use rhtn_node::store::{Decision, KIND_ENDPOINT_RECORD, KIND_TRANSACTION};
use rhtn_node::view::NodeView;
use rhtn_node::Keyhash;
use std::collections::BTreeSet;
use std::sync::Arc;

/// P (alice) over N (bob) over S1 (carol) and S2 (w1); S3 (w2) under S1 and
/// FAR (w4) under S3, three adoption edges from N.  Q (w3) is N's peer and
/// C (c1) its attached client.
struct Scene {
    w: World,
    n: NodeView,
    fab: Arc<Fabric>,
}

fn scene() -> Scene {
    let mut w = World::new();
    let (a_n, _) = w.adopt("bob", "alice", 1);
    let (a_s1, _) = w.adopt("carol", "bob", 2);
    let (a_s2, _) = w.adopt("w1", "bob", 3);
    let (a_s3, _) = w.adopt("w2", "carol", 4);
    let table = table_with(kh("bob"), &w, &[&a_n, &a_s1, &a_s2, &a_s3], &["alice", "bob", "carol", "w1", "w3"]);
    let mut n = view("bob", table, "alice", &[0]);
    n.peers.insert(kh("w3"));
    n.attached.insert(kh("c1"));
    n.set_slot(0, Some(kh("carol")), w.clock);
    n.set_slot(1, Some(kh("w1")), w.clock);
    let fab = Fabric::with(&[kh("alice"), kh("carol"), kh("w1"), kh("w3"), kh("c1")]);
    Scene { w, n, fab }
}

fn others(_s: &Scene) -> BTreeSet<Keyhash> {
    [kh("carol"), kh("w1"), kh("w3"), kh("c1")].into_iter().collect()
}

// acceptance: PRP-01
#[test]
fn a_stored_transaction_goes_to_every_adjacency_but_the_arrival_one() {
    let mut s = scene();
    // an adoption whose subject lies within N's own h_store: w5 under w1,
    // two edges from N
    let (obj, _) = s.w.adopt("w5", "w1", 7);
    s.fab.clear();
    let d = s.n.receive_push(&*s.fab, &kh("alice"), &encode_push(KIND_TRANSACTION, &obj.bytes), &ids());
    assert_eq!(d, Decision::Stored);
    assert!(s.n.store.holds_txid(&obj.txid));
    assert_eq!(s.fab.recipients(FRAME_TOPOLOGY_PUSH), others(&s), "S1, S2, Q and C");
    assert!(s.fab.to(&kh("alice"), FRAME_TOPOLOGY_PUSH).is_empty(), "none back to P");
    for r in others(&s) {
        assert_eq!(s.fab.to(&r, FRAME_TOPOLOGY_PUSH).len(), 1, "one frame each");
    }
}

// acceptance: PRP-02
#[test]
fn nothing_outside_h_store_is_stored_or_forwarded() {
    let mut s = scene();
    // w4 under w2: w2 is two edges from N, so its subordinate is three
    let (obj, _) = s.w.adopt("w4", "w2", 8);
    s.fab.clear();
    let before = s.n.store.len();
    let d = s.n.receive_push(&*s.fab, &kh("alice"), &encode_push(KIND_TRANSACTION, &obj.bytes), &ids());
    assert_eq!(d, Decision::OutOfStore);
    assert_eq!(s.n.store.len(), before);
    assert_eq!(s.fab.count(FRAME_TOPOLOGY_PUSH), 0, "nothing in the frame told N how far to forward");
}

// acceptance: PRP-03
#[test]
fn an_object_already_held_is_neither_stored_again_nor_forwarded() {
    let mut s = scene();
    let (obj, _) = s.w.adopt("w5", "w1", 7);
    let push = encode_push(KIND_TRANSACTION, &obj.bytes);
    assert_eq!(s.n.receive_push(&*s.fab, &kh("alice"), &push, &ids()), Decision::Stored);
    let held = s.n.store.len();
    s.fab.clear();
    let d = s.n.receive_push(&*s.fab, &kh("w3"), &push, &ids());
    assert_eq!(d, Decision::Duplicate);
    assert_eq!(s.n.store.len(), held, "the store is unchanged");
    assert_eq!(s.fab.count(FRAME_TOPOLOGY_PUSH), 0);
}

// acceptance: PRP-04
#[test]
fn an_object_whose_signer_key_is_missing_waits_and_then_flows() {
    let mut s = scene();
    let (obj, _) = s.w.adopt("w5", "w1", 7);
    // N holds every key but w5's
    let partial: Vec<_> = ids().into_iter().filter(|i| i.keyhash != kh("w5")).collect();
    s.fab.clear();
    let d = s.n.receive_push(&*s.fab, &kh("alice"), &encode_push(KIND_TRANSACTION, &obj.bytes), &partial);
    match &d {
        Decision::Held(p) => assert_eq!(p.missing_key, Some(kh("w5"))),
        other => panic!("{other:?}"),
    }
    assert!(!s.n.store.holds_txid(&obj.txid));
    assert_eq!(s.fab.count(FRAME_TOPOLOGY_PUSH), 0, "nothing before the key arrives");
    // the key arrives through the path N uses to fetch it
    let out = s.n.release_pending(&*s.fab, &ids());
    assert_eq!(out, vec![Decision::Stored]);
    assert!(s.n.store.holds_txid(&obj.txid));
    assert_eq!(s.fab.recipients(FRAME_TOPOLOGY_PUSH), others(&s));
}

// acceptance: PRP-05
#[test]
fn a_relay_forwards_an_object_it_is_no_party_to() {
    let mut s = scene();
    let (obj, _) = s.w.adopt("w5", "w1", 7);
    let me = kh("bob");
    assert!(!obj.signers.contains(&me), "N is neither patron nor subject");
    assert_ne!(obj.field_hash(1), Some(me));
    assert_ne!(obj.field_hash(2), Some(me));
    s.fab.clear();
    assert_eq!(s.n.receive_push(&*s.fab, &kh("alice"), &encode_push(KIND_TRANSACTION, &obj.bytes), &ids()), Decision::Stored);
    assert_eq!(s.fab.recipients(FRAME_TOPOLOGY_PUSH), others(&s), "the flood does not stop at its first relay");
}

// acceptance: PRP-06
#[test]
fn a_push_carries_the_object_byte_for_byte_and_one_wrapper_field() {
    let mut s = scene();
    let (obj, _) = s.w.adopt("w5", "w1", 7);
    let recorded = obj.bytes.clone();
    s.fab.clear();
    s.n.receive_push(&*s.fab, &kh("alice"), &encode_push(KIND_TRANSACTION, &recorded), &ids());
    let out = s.fab.to(&kh("carol"), FRAME_TOPOLOGY_PUSH);
    assert_eq!(out.len(), 1);
    let (kind, bytes) = decode_push(&out[0]).unwrap();
    assert_eq!(bytes, recorded, "byte-for-byte");
    assert_eq!(kind, KIND_TRANSACTION);
    // the wrapper carries the tag and no other field
    let item = rhtn_codec::cbor::parse_all(&out[0]).unwrap();
    let rhtn_codec::cbor::Item::Map(m) = &item else { panic!() };
    assert_eq!(m.len(), 2, "body-kind tag and the object, and nothing else");
    let mut extended = out[0].clone();
    extended[0] = 0xa3; // a third entry
    extended.extend_from_slice(&[0x03, 0x01]);
    assert!(decode_push(&extended).is_err(), "a further wrapper field is not accepted");
}

fn er(name: &str, points: &[NetworkPoint], seqno: Seqno) -> Vec<u8> {
    resolution::endpoint_record(&id(name), points, seqno)
}

// acceptance: PRP-07
#[test]
fn an_endpoint_record_supersedes_on_a_greater_counter() {
    let mut s = scene();
    let x = "carol";
    let five = er(x, &[point(1, 5000)], Seqno { series: 2, counter: 5 });
    assert_eq!(s.n.receive_push(&*s.fab, &kh("alice"), &encode_push(KIND_ENDPOINT_RECORD, &five), &ids()), Decision::Stored);
    s.fab.clear();
    // seqno 4, then the identical seqno 5 record
    let four = er(x, &[point(1, 4000)], Seqno { series: 2, counter: 4 });
    assert_eq!(s.n.receive_push(&*s.fab, &kh("alice"), &encode_push(KIND_ENDPOINT_RECORD, &four), &ids()), Decision::Duplicate);
    assert_eq!(s.n.receive_push(&*s.fab, &kh("alice"), &encode_push(KIND_ENDPOINT_RECORD, &five), &ids()), Decision::Duplicate);
    assert_eq!(s.n.store.endpoint(&kh(x)).unwrap().bytes, five, "still the seqno 5 record");
    assert_eq!(s.fab.count(FRAME_TOPOLOGY_PUSH), 0);
    // seqno 6 in the same series
    let six = er(x, &[point(1, 6000)], Seqno { series: 2, counter: 6 });
    assert_eq!(s.n.receive_push(&*s.fab, &kh("alice"), &encode_push(KIND_ENDPOINT_RECORD, &six), &ids()), Decision::Stored);
    assert_eq!(s.n.store.endpoint(&kh(x)).unwrap().bytes, six);
    assert_eq!(s.fab.recipients(FRAME_TOPOLOGY_PUSH), others(&s));
    assert!(s.fab.to(&kh("alice"), FRAME_TOPOLOGY_PUSH).is_empty());
}

// acceptance: PRP-08
#[test]
fn equal_seqnos_with_different_contents_retire_the_pair() {
    let mut s = scene();
    let x = "carol";
    let a = er(x, &[point(1, 5000)], Seqno { series: 2, counter: 5 });
    let b = er(x, &[point(2, 5001)], Seqno { series: 2, counter: 5 });
    assert_ne!(a, b);
    assert_eq!(s.n.receive_push(&*s.fab, &kh("alice"), &encode_push(KIND_ENDPOINT_RECORD, &a), &ids()), Decision::Stored);
    s.fab.clear();
    let d = s.n.receive_push(&*s.fab, &kh("w1"), &encode_push(KIND_ENDPOINT_RECORD, &b), &ids());
    assert_eq!(d, Decision::Conflict { subject: kh(x), seqno: Seqno { series: 2, counter: 5 } });
    assert!(s.n.store.endpoint(&kh(x)).is_none(), "neither is current");
    assert_eq!(s.fab.count(FRAME_TOPOLOGY_PUSH), 0, "nothing further for that pair");
    // and it repairs by re-resolving
    let resolutions: Vec<Vec<u8>> = s.fab.frames().into_iter().filter(|f| f.frame_type == REQUEST_RESOLVE).map(|f| f.body).collect();
    assert_eq!(resolutions.len(), 1, "a resolution for X leaves N");
    assert_eq!(ResolveRequest::decode(&resolutions[0]).unwrap().subject, kh(x));
    // a later arrival of either content is still not taken
    assert_eq!(s.n.receive_push(&*s.fab, &kh("w1"), &encode_push(KIND_ENDPOINT_RECORD, &a), &ids()), Decision::Duplicate);
    assert!(s.n.store.endpoint(&kh(x)).is_none());
}

fn memo(patron: &str, anchor: &str, slot: u64, timestamp: u64, occupant: Option<&str>) -> Memo {
    Memo {
        patron: kh(patron),
        position: rhtn_archive::tx::Locator { anchor: kh(anchor), path: Path::pack(&[0]), nibbles: 1, seqno: Seqno { series: 1, counter: 0 } },
        slot,
        timestamp,
        occupant: occupant.map(kh),
    }
}

// acceptance: PRP-09
#[test]
fn a_memo_from_another_subnet_is_dropped_not_forwarded() {
    let mut s = scene();
    let before = s.n.memo_table.clone();
    s.fab.clear();
    let m = memo("carol", "w6", 3, s.w.clock, Some("w2")); // anchored at B
    let out = s.n.receive_memo(&*s.fab, &kh("carol"), &m.encode());
    assert_eq!(out, MemoOutcome::ForeignSubnet);
    assert_eq!(s.fab.to(&kh("alice"), FRAME_TOPOLOGY_MEMO).len(), 0);
    assert_eq!(s.n.memo_table, before, "neither applied to a table nor passed on");
}

// acceptance: PRP-10
#[test]
fn a_memo_is_forwarded_unchanged_to_the_patron_and_stops_at_the_root() {
    let mut s = scene();
    let m = memo("carol", "alice", 3, s.w.clock, Some("w2"));
    let bytes = m.encode();
    s.fab.clear();
    let out = s.n.receive_memo(&*s.fab, &kh("carol"), &bytes);
    assert_eq!(out, MemoOutcome::Forwarded { to: kh("alice") });
    assert_eq!(s.fab.to(&kh("alice"), FRAME_TOPOLOGY_MEMO), vec![bytes.clone()], "unchanged");
    // the root has no patron and forwarding stops there
    let mut root = view("alice", Table::with_me(kh("alice")), "alice", &[]);
    root.set_slot(0, Some(kh("bob")), s.w.clock);
    let rfab = Fabric::with(&[kh("bob")]);
    assert!(root.is_root());
    let out = root.receive_memo(&*rfab, &kh("bob"), &bytes);
    assert_eq!(out, MemoOutcome::StoppedAtRoot);
    assert_eq!(rfab.count(FRAME_TOPOLOGY_MEMO), 0, "R sends no memo on any session");
}

// acceptance: PRP-11
#[test]
fn a_slot_held_at_or_after_the_memos_timestamp_is_not_forwarded() {
    let mut s = scene();
    let t = s.w.clock;
    s.n.set_slot(3, Some(kh("w2")), t);
    s.fab.clear();
    // field 1 names N and slot 3, timestamp at or before t
    let m = memo("bob", "alice", 3, t - 60, Some("w2"));
    let out = s.n.receive_memo(&*s.fab, &kh("carol"), &m.encode());
    assert!(matches!(out, MemoOutcome::CycleConfirmed { .. } | MemoOutcome::AlreadyPassed | MemoOutcome::Unconfirmed), "{out:?}");
    assert_eq!(s.fab.to(&kh("alice"), FRAME_TOPOLOGY_MEMO).len(), 0, "nothing to P");
    // the same rule against the memo table, for a slot this node relays
    let mut s2 = scene();
    let held = memo("carol", "alice", 5, t, Some("w5"));
    assert_eq!(s2.n.receive_memo(&*s2.fab, &kh("carol"), &held.encode()), MemoOutcome::Forwarded { to: kh("alice") });
    s2.fab.clear();
    let earlier = memo("carol", "alice", 5, t - 3600, Some("w6"));
    assert_eq!(s2.n.receive_memo(&*s2.fab, &kh("carol"), &earlier.encode()), MemoOutcome::AlreadyPassed);
    assert_eq!(s2.fab.to(&kh("alice"), FRAME_TOPOLOGY_MEMO).len(), 0);
    assert_eq!(s2.n.memo_table[&(kh("carol"), 5)].occupant, Some(kh("w5")), "the later row stands");
}

// acceptance: PRP-12
#[test]
fn a_memo_naming_this_node_and_matching_its_row_is_a_confirmed_cycle() {
    let mut s = scene();
    let t = s.w.clock;
    s.n.set_slot(0, Some(kh("carol")), t);
    s.fab.clear();
    // a memo N originated for slot 0 returns from S
    let m = memo("bob", "alice", 0, t, Some("carol"));
    let out = s.n.receive_memo(&*s.fab, &kh("carol"), &m.encode());
    assert_eq!(out, MemoOutcome::CycleConfirmed { disavowed: kh("carol") });
    assert_eq!(s.fab.count(FRAME_TOPOLOGY_MEMO), 0, "the memo is not forwarded further");
    // the disavowal is observed, naming S with reason code 5
    let pushed: Vec<Vec<u8>> = s.fab.frames().into_iter().filter(|f| f.frame_type == FRAME_TOPOLOGY_PUSH).map(|f| f.body).collect();
    assert!(!pushed.is_empty());
    let (kind, obj) = decode_push(&pushed[0]).unwrap();
    assert_eq!(kind, KIND_TRANSACTION);
    let rec = rhtn_archive::record::Record::parse(&obj).unwrap();
    assert_eq!(rec.tx_type, rhtn_archive::tx::TYPE_DISAVOWAL);
    assert_eq!(rec.field_hash(1), Some(kh("bob")));
    assert_eq!(rec.field_hash(2), Some(kh("carol")));
    assert_eq!(rec.field_uint(4), Some(5), "reason code 5, without prejudice");
    assert_eq!(s.n.slots[&0].occupant, None, "N's slot 0 is empty afterwards");
}

// acceptance: PRP-13
#[test]
fn a_memo_the_nodes_own_records_do_not_confirm_is_not_acted_on() {
    let mut s = scene();
    let t = s.w.clock;
    s.n.set_slot(0, Some(kh("carol")), t);
    let rows = s.n.slots.clone();
    s.fab.clear();
    // an occupant N's row does not hold
    let m = memo("bob", "alice", 0, t, Some("w9"));
    let out = s.n.receive_memo(&*s.fab, &kh("carol"), &m.encode());
    assert_eq!(out, MemoOutcome::Unconfirmed);
    assert_eq!(s.fab.frames().len(), 0, "no disavowal, no fetch, no forward");
    assert_eq!(s.n.slots, rows, "N's rows are unchanged");
}

// acceptance: PRP-14
#[test]
fn a_memo_goes_to_the_nearest_infra_node_when_the_patron_holds_no_session() {
    let mut w = World::new();
    let (a_l, _) = w.adopt("carol", "alice", 1); // L (carol) under I's subtree
    let (a_n, _) = w.adopt("bob", "carol", 2); // N (bob) under L
    let (a_sub, _) = w.adopt("w5", "bob", 3);
    // alice is infrastructure; carol, a light client, holds no sessions
    let table = table_with(kh("bob"), &w, &[&a_l, &a_n, &a_sub], &["alice"]);
    let mut n = view("bob", table, "alice", &[0, 1]);
    n.serving_node = Some(kh("alice"));
    n.set_slot(2, Some(kh("w5")), w.clock);
    assert_eq!(n.patron(), Some(kh("carol")));
    assert_eq!(n.table.serving_node(&kh("bob")), Some(kh("alice")));
    // the session to L does not exist; the one to I does
    let fab = Fabric::with(&[kh("alice")]);
    let to = n.originate_memo(&*fab, 2);
    assert_eq!(to, Some(kh("alice")));
    assert_eq!(fab.to(&kh("alice"), FRAME_TOPOLOGY_MEMO).len(), 1);
    assert_eq!(fab.to(&kh("carol"), FRAME_TOPOLOGY_MEMO).len(), 0, "nothing is attempted toward L");
}

// acceptance: PRP-15
#[test]
fn reconciliation_replays_the_same_frame() {
    let mut s = scene();
    // a departure of S1 from N: its subject is one edge from N and two from
    // P, so it falls in both stores
    let lost = s.w.depart("carol", "bob", rhtn_archive::tx::Seqno { series: 2, counter: 1 });
    let mut p = view("alice", s.n.table.clone_for(kh("alice")), "alice", &[]);
    let pfab = Fabric::with(&[kh("bob")]);
    p.take_object(&*pfab, &kh("w1"), KIND_TRANSACTION, &lost.bytes, &ids());
    assert!(p.store.holds_txid(&lost.txid));
    assert!(!s.n.store.holds_txid(&lost.txid));
    // N reconciles with its patron: a replay of the same frames
    pfab.clear();
    p.replay_to(&*pfab, &kh("bob"));
    let replayed = pfab.to(&kh("bob"), FRAME_TOPOLOGY_PUSH);
    assert_eq!(replayed.len(), 1);
    assert_eq!(replayed[0], encode_push(KIND_TRANSACTION, &lost.bytes), "the same shape as the original");
    s.fab.clear();
    let d = s.n.receive_push(&*s.fab, &kh("alice"), &replayed[0], &ids());
    assert_eq!(d, Decision::Stored);
    assert_eq!(s.fab.recipients(FRAME_TOPOLOGY_PUSH), others(&s));
}
