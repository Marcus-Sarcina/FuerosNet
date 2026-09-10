//! Replication and peering entries (REP): peering records as evidence about
//! concentration, peerlessness, the direct payload path, and what siblings
//! replicate.

mod common;

use common::*;
use rhtn_archive::record::Record;
use rhtn_archive::tx::{self, Seqno};
use rhtn_node::peering::*;
use rhtn_node::propagation::{FRAME_TOPOLOGY_MEMO, FRAME_TOPOLOGY_PUSH, encode_push};
use rhtn_node::resolution::NetworkPoint;
use rhtn_node::store::{Decision, KIND_TRANSACTION};
use rhtn_node::view::NodeView;
use rhtn_node::Keyhash;
use std::cell::RefCell;
use std::collections::BTreeSet;
use std::sync::{Arc, Mutex};

/// A peering transaction between two infra nodes, each asserting its own
/// address and ASN.
fn peering(a: &str, b: &str, a_point: NetworkPoint, b_point: NetworkPoint) -> Record {
    let (ia, ib) = (id(a), id(b));
    let pop = rhtn_codec::cose::sha256(format!("pop:{a}:{b}").as_bytes());
    let back_a = vec![rhtn_archive::genesis(&ia.public.keyhash)];
    let back_b = vec![rhtn_archive::genesis(&ib.public.keyhash)];
    let body = peering_body([&back_a, &back_b], &ia.public.keyhash, &ib.public.keyhash, &a_point, &b_point, 1_800_000_000, Some(1 << 20), &pop);
    Record::parse(&tx::envelope(tx::TYPE_PEERING, &body, &[&ia, &ib])).expect("well-formed")
}

/// An observer in A's horizon: A (bob) under root (alice), B (w5) elsewhere.
fn observer() -> (World, NodeView, Arc<Fabric>) {
    let mut w = World::new();
    let (a_a, _) = w.adopt("bob", "alice", 1);
    let (a_o, _) = w.adopt("carol", "alice", 2);
    let table = table_with(kh("carol"), &w, &[&a_a, &a_o], &["alice", "bob", "w5"]);
    let mut o = view("carol", table, "alice", &[1]);
    o.now = w.clock;
    let fab = Fabric::with(&[kh("alice"), kh("bob")]);
    (w, o, fab)
}

// acceptance: REP-05
#[test]
fn a_self_asserted_asn_is_not_validated_against_its_address() {
    let (_w, mut o, fab) = observer();
    // ASNs that correspond to no public routing data for these addresses
    let a_point = NetworkPoint::new([203, 0, 113, 7], Some(7431)).with_asn(64_512);
    let b_point = NetworkPoint::new([198, 51, 100, 9], Some(7431)).with_asn(64_513);
    let rec = peering("bob", "w5", a_point.clone(), b_point.clone());
    let d = o.take_object(&*fab, &kh("alice"), KIND_TRANSACTION, &rec.bytes, &ids());
    assert_eq!(d, Decision::Stored, "no validation failure is reported");
    let p = &o.peerings()[0];
    assert_eq!(p.a_point.asn, Some(64_512), "exactly those asserted");
    assert_eq!(p.b_point.asn, Some(64_513));
    assert_eq!(p.a_point.ip, [203, 0, 113, 7]);
}

// acceptance: REP-06
#[test]
fn a_stored_peering_record_yields_both_addresses_and_both_asns() {
    let (_w, mut o, fab) = observer();
    let a_point = NetworkPoint::new([203, 0, 113, 7], Some(7431)).with_asn(64_496);
    let b_point = NetworkPoint::new([198, 51, 100, 9], Some(7432)).with_asn(64_497);
    let rec = peering("bob", "w5", a_point.clone(), b_point.clone());
    o.take_object(&*fab, &kh("alice"), KIND_TRANSACTION, &rec.bytes, &ids());
    let p = &o.peerings()[0];
    assert_eq!((p.a, p.b), (kh("bob"), kh("w5")));
    assert_eq!(p.a_point, a_point);
    assert_eq!(p.b_point, b_point);
    assert_eq!(p.concentrated(), Some(false), "concentration is observable, not asserted");
    let same = peering("bob", "w5", a_point.clone(), b_point.clone().with_asn(64_496));
    assert_eq!(Peering::from_record(&same).unwrap().concentrated(), Some(true));
}

// acceptance: REP-08
#[test]
fn a_peering_generates_no_rootward_memo() {
    let (_w, mut a, fab) = observer();
    let rec = peering("carol", "w5", NetworkPoint::new([203, 0, 113, 1], None).with_asn(1), NetworkPoint::new([203, 0, 113, 2], None).with_asn(2));
    fab.clear();
    // A pushes the peering into its horizon
    assert_eq!(a.originate_push(&*fab, KIND_TRANSACTION, &rec.bytes, &ids()), Decision::Stored);
    assert!(a.store.holds_txid(&rec.txid), "the originator holds what it signed");
    assert!(fab.count(FRAME_TOPOLOGY_PUSH) > 0, "it floods as topology class");
    assert_eq!(fab.count(FRAME_TOPOLOGY_MEMO), 0, "and no memo is sent to P");
    // nothing in the memo table names B's network point
    let mut g = view("alice", a.table.clone_for(kh("alice")), "alice", &[]);
    let gfab = Fabric::with(&[kh("carol")]);
    let d = g.receive_push(&*gfab, &kh("carol"), &encode_push(KIND_TRANSACTION, &rec.bytes), &ids());
    assert_eq!(d, Decision::Stored, "G took the peering as topology");
    assert!(g.memo_table.is_empty(), "and its memo table gains no entry");
    assert_eq!(gfab.count(FRAME_TOPOLOGY_MEMO), 0, "and it sends none up");
}

// acceptance: REP-09
#[test]
fn peerlessness_is_derivable_from_held_topology() {
    let (_w, mut o, fab) = observer();
    // J (bob) has one peering record at O; I (w5) has none
    let rec = peering("bob", "w1", NetworkPoint::new([203, 0, 113, 1], None), NetworkPoint::new([203, 0, 113, 2], None));
    o.take_object(&*fab, &kh("alice"), KIND_TRANSACTION, &rec.bytes, &ids());
    assert_eq!(o.peers_of(&kh("bob")), BTreeSet::from([kh("w1")]), "J with one");
    assert_eq!(o.peers_of(&kh("w5")), BTreeSet::new(), "I with zero");
    assert!(o.peerless_infra(&kh("w5")), "and the zero is surfaced rather than omitted");
    assert!(!o.peerless_infra(&kh("bob")));
}

// acceptance: REP-10
#[test]
fn a_root_with_no_peers_can_still_adopt() {
    let mut w = World::new();
    let table = table_with(kh("alice"), &w, &[], &["alice"]);
    let mut i = view("alice", table, "alice", &[]);
    i.now = w.clock;
    assert!(i.is_root());
    assert_eq!(i.peers_of(&kh("alice")), BTreeSet::new(), "no peering records");
    let pop = w.formation("alice", "bob");
    let body = i
        .propose_adoption(&kh("bob"), rhtn_node::currency::Staple::Current, tx::Evidence::Presence(pop.txid), 3, &[rhtn_archive::genesis(&kh("bob"))])
        .expect("no step refuses or defers it for want of a peering record");
    let rec = i.countersign_adoption(&body, &id("bob")).unwrap();
    assert_eq!(rec.tx_type, tx::TYPE_ADOPTION);
    let fab = Fabric::with(&[kh("bob")]);
    assert_eq!(i.take_object(&*fab, &kh("bob"), KIND_TRANSACTION, &rec.bytes, &ids()), Decision::Stored);
}

/// Who carried what, to whom.
type Carried = (Option<Keyhash>, Keyhash, Vec<u8>);

#[derive(Default)]
struct Carriers(Mutex<Vec<Carried>>);

impl PayloadSink for Carriers {
    fn carry(&self, carrier: Option<Keyhash>, to: &Keyhash, bytes: &[u8]) {
        self.0.lock().unwrap().push((carrier, *to, bytes.to_vec()));
    }
}

impl Carriers {
    fn saw(&self) -> Vec<(Option<Keyhash>, Keyhash)> {
        self.0.lock().unwrap().iter().map(|(c, t, _)| (*c, *t)).collect()
    }
}

/// L1 (carol) and L2 (w1) are siblings under P (bob); serving node S (alice).
fn siblings_scene() -> (World, NodeView) {
    let mut w = World::new();
    let (a_p, _) = w.adopt("bob", "alice", 1);
    let (a1, _) = w.adopt("carol", "bob", 2);
    let (a2, _) = w.adopt("w1", "bob", 3);
    let table = table_with(kh("carol"), &w, &[&a_p, &a1, &a2], &["alice"]);
    let mut l1 = view("carol", table, "alice", &[0, 1]);
    l1.serving_node = Some(kh("alice"));
    l1.now = w.clock;
    (w, l1)
}

// acceptance: REP-11
#[test]
fn payload_inside_the_horizon_takes_the_direct_path() {
    let (_w, l1) = siblings_scene();
    assert!(l1.table.horizon(&kh("carol"), 2).contains(&kh("w1")), "distance 1, inside the h = 2 store");
    let sink = Carriers::default();
    let d = l1.send_payload(&kh("w1"), PathOverride::None, true, &sink, b"hello");
    assert_eq!(d, Delivery::Direct { to: kh("w1") });
    assert_eq!(sink.saw(), vec![(None, kh("w1"))], "neither serving node carries the bytes");
}

/// L1 (carol) under P1 (bob) under root; L2 (w5) under P2 (w6) under a
/// different root, outside each other's horizon.
fn distant_scene() -> (World, NodeView) {
    let mut w = World::new();
    let (a_p1, _) = w.adopt("bob", "alice", 1);
    let (a_l1, _) = w.adopt("carol", "bob", 2);
    let (a_p2, _) = w.adopt("w6", "w7", 3);
    let (a_l2, _) = w.adopt("w5", "w6", 4);
    let table = table_with(kh("carol"), &w, &[&a_p1, &a_l1, &a_p2, &a_l2], &["alice", "w7"]);
    let mut l1 = view("carol", table, "alice", &[0, 1]);
    l1.serving_node = Some(kh("alice"));
    l1.now = w.clock;
    (w, l1)
}

// acceptance: REP-12
#[test]
fn payload_outside_the_horizon_is_relayed() {
    let (_w, l1) = distant_scene();
    assert!(!l1.table.horizon(&kh("carol"), 2).contains(&kh("w5")), "outside each other's h = 2 horizon");
    let sink = Carriers::default();
    let d = l1.send_payload(&kh("w5"), PathOverride::None, true, &sink, b"hello");
    assert_eq!(d, Delivery::Relayed { via: kh("alice"), to: kh("w5") });
    assert_eq!(sink.saw(), vec![(Some(kh("alice")), kh("w5"))], "S1 carries it as ciphertext");
    assert!(!matches!(d, Delivery::Direct { .. }), "L1 opens no connection to L2's address");
}

// acceptance: REP-13
#[test]
fn a_peering_edge_does_not_extend_the_horizon_for_the_direct_path() {
    let (_w, mut l1) = distant_scene();
    // A and B peer; the edge is stored and confers no scope
    let rec = peering("alice", "w7", NetworkPoint::new([203, 0, 113, 1], None), NetworkPoint::new([203, 0, 113, 2], None));
    let fab = Fabric::with(&[kh("alice")]);
    l1.take_object(&*fab, &kh("alice"), KIND_TRANSACTION, &rec.bytes, &ids());
    assert_eq!(l1.peers_of(&kh("alice")), BTreeSet::from([kh("w7")]), "the peering is held");
    assert!(!l1.table.horizon(&kh("carol"), 2).contains(&kh("w5")), "M is absent from L's h = 2 store");
    let sink = Carriers::default();
    let d = l1.send_payload(&kh("w5"), PathOverride::None, true, &sink, b"hello");
    assert_eq!(d, Delivery::Relayed { via: kh("alice"), to: kh("w5") }, "no direct connection is attempted");
}

// acceptance: REP-14
#[test]
fn a_user_override_of_the_direct_path_default_is_honoured() {
    let (_w, l1) = distant_scene();
    let sink = Carriers::default();
    let d = l1.send_payload(&kh("w5"), PathOverride::ForceDirect, true, &sink, b"hello");
    assert_eq!(d, Delivery::Direct { to: kh("w5") });
    assert_eq!(sink.saw(), vec![(None, kh("w5"))], "neither serving node carries it");
    // and the other default is overridable too
    let (_w2, near) = siblings_scene();
    let sink2 = Carriers::default();
    let d2 = near.send_payload(&kh("w1"), PathOverride::ForceRelayed, true, &sink2, b"hello");
    assert_eq!(d2, Delivery::Relayed { via: kh("alice"), to: kh("w1") });
}

/// A sibling that takes replicated state, and records what it took.
#[derive(Default)]
struct Sibling {
    me: Keyhash,
    taken: RefCell<Vec<Replicated>>,
}

impl Replica for Sibling {
    fn take(&self, item: &Replicated) {
        self.taken.borrow_mut().push(item.clone());
    }
    fn who(&self) -> Keyhash {
        self.me
    }
}

/// What the replication payload is built from, at the view level; the
/// running node's test in `rhtn-sim` is the entry.
#[test]
fn siblings_replicate_topology_and_history_and_no_queue_state() {
    let mut w = World::new();
    let (a_n, _) = w.adopt("bob", "alice", 1);
    let (a_c, pop) = w.adopt("carol", "bob", 2);
    let table = table_with(kh("bob"), &w, &[&a_n, &a_c], &["alice", "bob", "w1"]);
    let mut n = view("bob", table, "alice", &[0]);
    n.attached.insert(kh("carol"));
    let fab = Fabric::with(&[kh("alice"), kh("carol"), kh("w1")]);
    n.take_object(&*fab, &kh("alice"), KIND_TRANSACTION, &a_c.bytes, &ids());
    n.store.keep_presence(pop.txid, pop.bytes.clone());
    // a message waits for C at N, and it is no part of what replicates
    let queued = b"ciphertext for C".to_vec();
    let sib = Sibling { me: kh("w1"), taken: Default::default() };
    let replicas: Vec<&dyn Replica> = vec![&sib];
    for item in n.replication_payload() {
        n.replicate(&replicas, &item);
    }
    n.replicate(&replicas, &Replicated::TrustBearing { object: pop.bytes.clone() });
    let taken = sib.taken.borrow().clone();
    assert!(taken.iter().any(|i| matches!(i, Replicated::Topology { object, .. } if *object == a_c.bytes)), "C's adoption");
    assert!(taken.iter().any(|i| matches!(i, Replicated::TrustBearing { object } if *object == pop.bytes)), "and its presence record");
    assert!(!taken.iter().any(|i| match i {
        Replicated::Topology { object, .. } => *object == queued,
        Replicated::TrustBearing { object } => *object == queued,
        _ => false,
    }), "no queued message and no record that one is waiting");
    assert_eq!(sib.who(), kh("w1"));
}

// acceptance: REP-04
#[test]
fn a_move_inside_the_replication_horizon_needs_no_archive_presentation() {
    let mut w = World::new();
    let (a_p, _) = w.adopt("bob", "alice", 1); // P under root
    let (a_t, _) = w.adopt("w1", "alice", 2); // T, P's sibling
    let (a_l, _) = w.adopt("carol", "bob", 3); // L under P
    let table = table_with(kh("w1"), &w, &[&a_p, &a_t, &a_l], &["alice", "bob", "w1"]);
    let mut t = view("w1", table, "alice", &[1]);
    t.now = w.clock;
    assert!(t.inside_replication_horizon(&kh("bob"), &kh("w1")), "T is P's sibling");
    // L presents T an adoption carrying P's countersignature: a lateral shift
    let block = tx::transfer_block(&id("bob"), &kh("carol"), &kh("w1"));
    let back_l = vec![rhtn_archive::genesis(&kh("carol"))];
    let back_t = t.archive.next_back_pointers();
    let a = tx::Adoption {
        node: kh("carol"),
        patron: kh("w1"),
        locator: tx::Locator { anchor: kh("alice"), path: pack_path(&[1, 0]), nibbles: 2, seqno: Seqno { series: 9, counter: 0 } },
        timestamp: t.now,
        key_material: None,
        evidence: tx::Evidence::Transfer { former: kh("bob"), block },
        presented_head: None,
        back: [&back_l, &back_t],
    };
    let rec = Record::parse(&tx::envelope(tx::TYPE_ADOPTION, &tx::adoption_body(&a), &[&id("carol"), &id("w1")])).unwrap();
    assert!(rec.field_hash(7).is_none(), "no archive presentation accompanies it");
    let fab = Fabric::with(&[kh("alice"), kh("carol")]);
    // T completes it: nothing is fetched and no ceremony is required
    let d = t.take_object(&*fab, &kh("carol"), KIND_TRANSACTION, &rec.bytes, &ids());
    assert_eq!(d, Decision::Stored);
    assert!(t.table.patrons(&kh("carol")).contains(&kh("w1")), "the adoption completes");
    assert_eq!(fab.count(rhtn_node::resolution::REQUEST_RESOLVE), 0);
    let archive_requests = fab.frames().into_iter().filter(|f| f.frame_type == 2).count();
    assert_eq!(archive_requests, 0, "no archive-fetch request");
}

// acceptance: REP-03
#[test]
fn a_sibling_attests_what_its_replicated_state_holds() {
    use rhtn_node::currency::*;
    let mut w = World::new();
    let (a_p, _) = w.adopt("bob", "alice", 1); // P under root
    let (a_t, _) = w.adopt("w1", "alice", 2); // T, P's sibling
    let (a_l, _) = w.adopt("carol", "bob", 3); // L under P
    let table = table_with(kh("w1"), &w, &[&a_p, &a_t, &a_l], &["alice", "bob", "w1"]);
    let mut t = view("w1", table, "alice", &[1]);
    t.now = w.clock;
    // T holds P's replicated record for L; P has since countersigned a
    // rotation that has not reached T
    let mut cur = CurrencyState::default();
    cur.unreachable.insert(kh("bob"));
    assert_eq!(t.rung_for(&cur, &kh("carol")), Some(Rung::Sibling));
    let bytes = t.issue_currency(&cur, &kh("carol")).expect("T issues");
    let a = rhtn_archive::currency::parse_attestation(&ids(), &bytes).unwrap();
    assert_eq!(a.role, 1, "field 5 is 1");
    assert_eq!(a.current, kh("carol"), "the key T's replicated state holds");
    assert_eq!(a.issuer, kh("w1"));
    // the reply carries no indication that P's state differs
    let reply = t.answer_currency(&cur, &CurrencyRequest { subject: kh("carol"), nonce: [3; 16] });
    let encoded = reply.encode();
    assert!(!encoded.windows(32).any(|x| x == kh("alice2")), "no successor P knows about appears");
    assert_eq!(CurrencyReply::decode(&encoded).unwrap(), reply, "the lag is visible only as the secondhand role");
}
