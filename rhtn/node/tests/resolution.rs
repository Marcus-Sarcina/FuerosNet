//! Resolution entries (RES): the anchor table, the descent through
//! infrastructure, referrals and endpoint records, and the freshness rules
//! a holder applies to locators.

mod common;

use common::*;
use rhtn_archive::tx::{Locator, Seqno};
use rhtn_node::resolution::*;
use rhtn_node::store::KIND_ENDPOINT_RECORD;
use rhtn_node::resolution::{Carried, ClientResolution};
use rhtn_node::view::NodeView;
use std::sync::Arc;

fn nonce(n: u8) -> [u8; 16] {
    [n; 16]
}

/// A (alice) is the root anchor; C (bob) is its infra child at index 1; X
/// (carol) is a light client attached to C at index 2.
struct Tree {
    a: NodeView,
    c: NodeView,
    afab: Arc<Fabric>,
    c_record: Vec<u8>,
    a_entry: Vec<u8>,
}

fn tree() -> Tree {
    let mut w = World::new();
    let (a_c, _) = w.adopt("bob", "alice", 1);
    let (a_x, _) = w.adopt("carol", "bob", 2);
    let a_table = table_with(kh("alice"), &w, &[&a_c, &a_x], &["alice", "bob"]);
    let c_table = table_with(kh("bob"), &w, &[&a_c, &a_x], &["alice", "bob"]);

    let mut a = view("alice", a_table, "alice", &[]);
    a.set_slot(1, Some(kh("bob")), w.clock);
    let mut c = view("bob", c_table, "alice", &[1]);
    c.set_slot(2, Some(kh("carol")), w.clock);
    c.attached.insert(kh("carol"));

    // C publishes its endpoint record; it reaches A by the topology class
    let c_record = endpoint_record(&id("bob"), &[point(1, 7001)], Seqno { series: 1, counter: 3 });
    let afab = Fabric::with(&[kh("bob")]);
    let cfab = Fabric::with(&[kh("alice"), kh("carol")]);
    a.take_object(&*afab, &kh("bob"), KIND_ENDPOINT_RECORD, &c_record, &ids());
    // and each publishes its own, so a serving answer can carry endpoints
    let a_record = endpoint_record(&id("alice"), &[point(1, 7000)], Seqno { series: 1, counter: 1 });
    a.take_object(&*afab, &kh("bob"), KIND_ENDPOINT_RECORD, &a_record, &ids());
    c.take_object(&*cfab, &kh("alice"), KIND_ENDPOINT_RECORD, &c_record, &ids());
    let a_entry = anchor_entry(&id("alice"), &[point(1, 7000)], 200, Seqno { series: 1, counter: 1 });
    afab.clear();
    Tree { a, c, afab, a_entry, c_record }
}

/// The path from A to X: index 1 into C, then index 2 into X.
fn path_to_x() -> Path {
    Path::from_indices(&[1, 2])
}

/// A lookup that records every identity it is asked for, so a test can see
/// whether a path ever reached for a key.
struct Recorder {
    pins: Vec<rhtn_crypto::Identity>,
    asked: std::cell::RefCell<Vec<[u8; 32]>>,
}

impl rhtn_crypto::verify::Lookup for Recorder {
    fn identity(&self, keyhash: &[u8]) -> Option<&rhtn_crypto::Identity> {
        self.asked.borrow_mut().push(keyhash.try_into().unwrap());
        self.pins.iter().find(|i| i.keyhash == keyhash)
    }
}

// acceptance: RES-01
#[test]
fn a_gossiped_anchor_is_a_starting_point_and_is_never_pinned() {
    let t = tree();
    // the requester holds the entry and no key material for A
    let mut anchors = AnchorTable::new(0, Ingestion::UnverifiedGossip);
    let entry = AnchorEntry::parse(&t.a_entry).unwrap();
    let unpinned = Recorder { pins: ids().into_iter().filter(|i| i.keyhash != kh("alice")).collect(), asked: Default::default() };
    assert!(entry.signature_checks(&unpinned).is_none(), "nothing lets the recipient check it on receipt");
    unpinned.asked.borrow_mut().clear();
    assert!(anchors.offer(entry, &unpinned));
    assert!(unpinned.asked.borrow().is_empty(), "a gossip table asks for no key on ingestion");
    let mut r = Resolution::begin(&anchors, kh("carol"), kh("alice"), path_to_x(), nonce(1)).unwrap();
    assert_eq!(r.next_hop().0, kh("alice"));
    assert_eq!(r.next_hop().1[0].socket().port(), 7000, "an endpoint taken from the entry");
    // A refers onward
    let reply = t.a.answer_resolution(&ResolveRequest::decode(&r.request.encode()).unwrap());
    match r.take(&reply) {
        Step::Continue(rf) => assert_eq!(rf.next, kh("bob")),
        other => panic!("{other:?}"),
    }
    // C answers authoritatively
    let reply = t.c.answer_resolution(&r.request);
    match r.take(&reply) {
        Step::Arrived(si) => {
            assert_eq!(si.node, kh("bob"));
            assert_eq!(si.residual.indices(), vec![2]);
        }
        other => panic!("{other:?}"),
    }
    assert!(r.arrived.is_some(), "the resolution completes at the serving node");
    // at no point did the requester reach for A's key: nothing in beginning,
    // following the referral or arriving asked the lookup for anyone
    assert!(unpinned.asked.borrow().is_empty(), "no pin for A was created or sought: {:?}", unpinned.asked.borrow().len());
    assert!(unpinned.pins.iter().all(|i| i.keyhash != kh("alice")));
}

// acceptance: RES-02
#[test]
fn anchor_entries_below_the_nodes_own_threshold_are_not_retained() {
    let n = 50u64;
    let mut anchors = AnchorTable::new(n, Ingestion::UnverifiedGossip);
    let r1 = anchor_entry(&id("w6"), &[point(1, 7010)], n - 1, Seqno { series: 1, counter: 1 });
    let r2 = anchor_entry(&id("w7"), &[point(1, 7011)], n, Seqno { series: 1, counter: 1 });
    assert!(!anchors.offer(AnchorEntry::parse(&r1).unwrap(), &ids()), "below n");
    assert!(anchors.offer(AnchorEntry::parse(&r2).unwrap(), &ids()), "at n");
    assert!(anchors.get(&kh("w7")).is_some());
    assert!(anchors.get(&kh("w6")).is_none());
    assert_eq!(
        Resolution::begin(&anchors, kh("carol"), kh("w6"), path_to_x(), nonce(2)).unwrap_err(),
        NotResolvable::AnchorAbsent(kh("w6"))
    );
    assert!(Resolution::begin(&anchors, kh("carol"), kh("w7"), path_to_x(), nonce(2)).is_ok());
}

// acceptance: RES-03
#[test]
fn a_locator_whose_anchor_is_absent_sends_no_request() {
    let anchors = AnchorTable::new(0, Ingestion::UnverifiedGossip);
    let err = Resolution::begin(&anchors, kh("carol"), kh("alice"), path_to_x(), nonce(3)).unwrap_err();
    assert_eq!(err, NotResolvable::AnchorAbsent(kh("alice")), "a caller-side condition, not a wire failure");
    // and through the node's own entry point nothing goes on any session
    let mut w = World::new();
    let n = view("w8", table_with(kh("w8"), &w, &[], &["w8"]), "w8", &[]);
    let _ = w.tick();
    let fab = Fabric::with(&[kh("alice"), kh("bob")]);
    assert_eq!(n.resolve(&*fab, &anchors, kh("carol"), kh("alice"), path_to_x(), nonce(3)).unwrap_err(), NotResolvable::AnchorAbsent(kh("alice")));
    assert_eq!(fab.frames().len(), 0, "no ResolveRequest is emitted on any connection");
}

fn fixture(id: &str) -> Vec<u8> {
    let c: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/../../test-vectors/corpus.json")).unwrap()).unwrap();
    let e = c["entries"].as_array().unwrap().iter().find(|e| e["id"] == id).unwrap();
    hex::decode(e["hex"].as_str().unwrap()).unwrap()
}

/// The V12 conflict partner (`test-vectors/primitives.md`): alice's
/// `[5, 100]` locator with a different path, rebuilt here and checked
/// against the bytes the vectors document.
fn v12() -> Vec<u8> {
    let documented = "a30158208410def778a5de3a25991aba399716bc8eccfda9ad57d4ea8a0c8dcfc852aa6a02a30158206bcf8a3e8899fc206bc603744414d58b01db857986d82f611b0794ac9c32c37502a201412702020382051864038443a10127a0f658405a7e4a1e7fed8b75c3703dbb7fdbf134f2f200d3e48a949e466a96fff45a0730d30aee8ab60f5483d063fbd5b19d58269d9f358f56bc7a726ebeb4aab60b0903";
    let built = signed_locator(&id("alice"), &Locator { anchor: kh("bob"), path: vec![0x27], nibbles: 2, seqno: Seqno { series: 5, counter: 100 } });
    assert_eq!(hex::encode(&built), documented, "the rebuilt partner is the documented one");
    built
}

// acceptance: RES-04
#[test]
fn a_same_series_counter_jump_supersedes() {
    let mut store = LocatorStore::new();
    let base = fixture("P-signedlocator");
    let jump = fixture("P-signedlocator-jump");
    assert_eq!(store.offer(&base, &ids()), LocatorOutcome::Installed);
    assert_eq!(SignedLocator::parse(&base).unwrap().locator.seqno, Seqno { series: 5, counter: 42 });
    assert_eq!(store.offer(&jump, &ids()), LocatorOutcome::Replaced);
    let Reach::Dial(loc) = store.reach(&kh("alice")) else { panic!("{:?}", store.reach(&kh("alice"))) };
    assert_eq!(loc.seqno, Seqno { series: 5, counter: 100 });
    assert_eq!(store.in_series(&kh("alice"), 5).unwrap().bytes, jump, "the [5, 42] locator is no longer current");
}

// acceptance: RES-05
#[test]
fn equal_seqnos_with_different_paths_leave_neither_current() {
    let mut store = LocatorStore::new();
    let jump = fixture("P-signedlocator-jump");
    assert_eq!(store.offer(&jump, &ids()), LocatorOutcome::Installed);
    let partner = v12();
    assert_ne!(partner, jump);
    assert_eq!(SignedLocator::parse(&partner).unwrap().locator.seqno, SignedLocator::parse(&jump).unwrap().locator.seqno);
    assert_eq!(store.offer(&partner, &ids()), LocatorOutcome::Conflict);
    assert!(store.all(&kh("alice")).is_empty(), "neither is current");
    assert_eq!(store.reach(&kh("alice")), Reach::MustReResolve, "a re-resolution rather than a dial to either path");
}

// acceptance: RES-06
#[test]
fn two_series_do_not_rank_and_the_holder_says_so() {
    let mut store = LocatorStore::new();
    let jump = fixture("P-signedlocator-jump");
    store.offer(&jump, &ids());
    let other = signed_locator(&id("alice"), &Locator { anchor: kh("bob"), path: vec![0x91], nibbles: 2, seqno: Seqno { series: 9, counter: 3 } });
    assert_eq!(store.offer(&other, &ids()), LocatorOutcome::Incomparable);
    assert_eq!(store.in_series(&kh("alice"), 5).unwrap().bytes, jump, "the [5, 100] locator is not marked stale");
    assert!(store.in_series(&kh("alice"), 9).is_some(), "and the newcomer is not installed as fresher");
    assert_eq!(store.reach(&kh("alice")), Reach::Indeterminate);
    // a §4.6 chain for one series is what resolves it
    store.prove_series(kh("alice"), 9);
    let Reach::Dial(loc) = store.reach(&kh("alice")) else { panic!() };
    assert_eq!(loc.seqno.series, 9);
}

// acceptance: RES-07
#[test]
fn a_light_clients_resolution_goes_through_its_serving_node() {
    let t = tree();
    // L is carol, attached to S (bob); L holds no anchor entry
    let mut l = view("carol", t.c.table.clone_for(kh("carol")), "alice", &[1, 2]);
    l.serving_node = Some(kh("bob"));
    let lfab = Fabric::with(&[kh("bob")]);
    let lanchors = AnchorTable::new(0, Ingestion::UnverifiedGossip);
    assert!(lanchors.is_empty(), "L holds none");
    // X sits beneath another anchor, W (w7), that S holds an entry for
    let x_path = Path::from_indices(&[4, 1]);
    let (mut lr, carried) = l.resolve(&*lfab, &lanchors, kh("w5"), kh("w7"), x_path.clone(), nonce(7)).unwrap();
    assert_eq!(carried, Carried::Delegated(kh("bob")));
    let sent: Vec<_> = lfab.frames();
    assert_eq!(sent.len(), 1, "L's only request");
    assert_eq!(sent[0].to, kh("bob"), "on its session with S");
    assert_eq!(sent[0].frame_type, REQUEST_RESOLVE);
    let at_s = ResolveRequest::decode(&sent[0].body).unwrap();
    assert!(lfab.to(&kh("w7"), REQUEST_RESOLVE).is_empty(), "L opens no connection to W");
    // S takes L's request and resolves on L's behalf from its own table
    let mut anchors = AnchorTable::new(0, Ingestion::UnverifiedGossip);
    anchors.offer(AnchorEntry::parse(&anchor_entry(&id("w7"), &[point(7, 7007)], 50, Seqno { series: 1, counter: 1 })).unwrap(), &ids());
    let ClientResolution::Proxied(mut r) = t.c.resolve_for_client(&anchors, &at_s).unwrap() else { panic!("S is not on W's path") };
    assert_eq!(r.next_hop().0, kh("w7"), "S sends a ResolveRequest to an endpoint of W");
    assert_eq!(r.request.nonce, nonce(7), "under the client's nonce");
    // W refers to its child w6, which serves X
    let mut w7 = view("w7", table_with(kh("w7"), &t_world(), &[], &["w7", "w6"]), "w7", &[]);
    w7.set_slot(4, Some(kh("w6")), 1);
    let w6_record = endpoint_record(&id("w6"), &[point(6, 7006)], Seqno { series: 1, counter: 1 });
    w7.take_object(&*Fabric::with(&[]), &kh("w6"), KIND_ENDPOINT_RECORD, &w6_record, &ids());
    let reply = w7.answer_resolution(&r.request);
    assert!(matches!(r.take(&reply), Step::Continue(_)), "S follows the referral");
    let mut w6 = view("w6", table_with(kh("w6"), &t_world(), &[], &["w7", "w6"]), "w7", &[4]);
    w6.set_slot(1, Some(kh("w5")), 1);
    w6.attached.insert(kh("w5"));
    let reply = w6.answer_resolution(&r.request);
    assert!(matches!(r.take(&reply), Step::Arrived(_)));
    // and S returns the result to L, who is done
    let to_l = t.c.reply_for_client(&r, Some(&reply));
    match lr.take(&to_l) {
        Step::Arrived(si) => {
            assert_eq!(si.node, kh("w6"), "L receives X's serving node");
            assert_eq!(si.residual.indices(), vec![1], "and the residual path");
        }
        other => panic!("{other:?}"),
    }
}

fn t_world() -> World {
    World::new()
}

// acceptance: RES-08
#[test]
fn a_client_two_levels_down_is_answered_with_the_residual_suffix() {
    let mut w = World::new();
    let (a_p, _) = w.adopt("bob", "alice", 1); // P, a light client, under S
    let (a_l, _) = w.adopt("carol", "bob", 2); // L under P
    let table = table_with(kh("alice"), &w, &[&a_p, &a_l], &["alice"]);
    let mut s = view("alice", table, "alice", &[]);
    s.set_slot(3, Some(kh("bob")), w.clock);
    s.attached.insert(kh("carol"));
    assert!(!s.table.is_infra(&kh("bob")), "P is a light client");
    assert_eq!(s.table.serving_node(&kh("carol")), Some(kh("alice")), "L attaches to S at depth two");
    let sfab = Fabric::with(&[kh("carol")]);
    let req = ResolveRequest { subject: kh("carol"), anchor: kh("alice"), path: Path::from_indices(&[3, 4]).bytes, nibbles: 2, nonce: nonce(8) };
    let reply = s.answer_resolution(&req);
    let ResolveReply::Serving { serving, .. } = &reply else { panic!("{reply:?}") };
    assert_eq!(serving.node, kh("alice"), "field 1 is S");
    assert_eq!(serving.residual.indices(), vec![3, 4], "the non-empty suffix identifying L beneath S");
    assert_eq!(sfab.count(REQUEST_RESOLVE), 0, "no request is forwarded to P");
}

// acceptance: RES-09
#[test]
fn a_roots_self_anchored_locator_resolves_to_an_empty_residual() {
    // the fixture: bob names himself as anchor with the empty path
    let sl = SignedLocator::parse(&fixture("P-signedlocator-root")).unwrap();
    assert_eq!(sl.subject, kh("bob"));
    assert_eq!(sl.locator.anchor, kh("bob"));
    assert_eq!(sl.locator.nibbles, 0);
    let mut w = World::new();
    let (a_sub, _) = w.adopt("carol", "bob", 1);
    let table = table_with(kh("bob"), &w, &[&a_sub], &["bob"]);
    let mut r = view("bob", table, "bob", &[]);
    r.set_slot(0, Some(kh("carol")), w.clock);
    let req = ResolveRequest { subject: kh("bob"), anchor: kh("bob"), path: sl.locator.path.clone(), nibbles: 0, nonce: nonce(9) };
    let reply = r.answer_resolution(&req);
    let ResolveReply::Serving { serving, .. } = &reply else { panic!("{reply:?}") };
    assert_eq!(serving.node, kh("bob"));
    assert!(serving.residual.is_empty(), "R itself is the addressed party");
}

// acceptance: RES-10
#[test]
fn a_referral_comes_from_the_childs_endpoint_record_before_any_contact() {
    let t = tree();
    // S (alice) holds C's record by topology push, no key material for C
    let unpinned: Vec<_> = ids().into_iter().filter(|i| i.keyhash != kh("bob")).collect();
    let er = rhtn_node::store::EndpointRecord::parse(&t.c_record).unwrap();
    assert!(er.signature_checks(&unpinned).is_none(), "S cannot check it yet");
    let req = ResolveRequest { subject: kh("carol"), anchor: kh("alice"), path: path_to_x().bytes, nibbles: 2, nonce: nonce(10) };
    let reply = t.a.answer_resolution(&req);
    let ResolveReply::Referral { referral, .. } = &reply else { panic!("{reply:?}") };
    assert_eq!(referral.next, kh("bob"));
    assert_eq!(referral.endpoints, er.endpoints.iter().map(|p| NetworkPoint::decode_bytes(p).unwrap()).collect::<Vec<_>>());
    assert!(referral.advances >= 1);
    assert_eq!(t.afab.frames().len(), 0, "S opens no connection to C before replying");
}

// acceptance: RES-11
#[test]
fn a_referral_that_advances_nothing_is_rejected() {
    let t = tree();
    let mut anchors = AnchorTable::new(0, Ingestion::UnverifiedGossip);
    anchors.offer(AnchorEntry::parse(&t.a_entry).unwrap(), &ids());
    let three = Path::from_indices(&[1, 2, 3]);
    let mut r = Resolution::begin(&anchors, kh("w5"), kh("alice"), three, nonce(11)).unwrap();
    let bad = ResolveReply::Referral {
        nonce: nonce(11),
        referral: Referral { next: kh("w9"), endpoints: vec![point(1, 7099)], advances: 0, key_material: None },
    };
    let before = r.hops.clone();
    match r.take(&bad) {
        Step::Malformed(_) => {}
        other => panic!("{other:?}"),
    }
    assert_eq!(r.hops, before, "the named next hop is not dialled");
    // and one advancing past the path's end is malformed too
    let past = ResolveReply::Referral {
        nonce: nonce(11),
        referral: Referral { next: kh("w9"), endpoints: vec![point(1, 7099)], advances: 4, key_material: None },
    };
    assert!(matches!(r.take(&past), Step::Malformed(_)));
    assert_eq!(r.hops, before);
}

// acceptance: RES-14
#[test]
fn a_departed_child_is_removed_and_its_path_fails_rather_than_redirecting() {
    let mut t = tree();
    // C departs from A and is adopted by an unrelated patron
    let dep = t.w_depart();
    assert_eq!(t.a.child_at(1), Some(kh("bob")), "the child table holds C at index 1");
    let d = t.a.take_object(&*t.afab, &kh("bob"), rhtn_node::store::KIND_TRANSACTION, &dep, &ids());
    assert_eq!(d, rhtn_node::store::Decision::Stored);
    assert!(!t.a.table.subordinates(&kh("alice")).contains(&kh("bob")), "C has left the subtree");
    let req = ResolveRequest { subject: kh("carol"), anchor: kh("alice"), path: path_to_x().bytes, nibbles: 2, nonce: nonce(14) };
    let reply = t.a.answer_resolution(&req);
    match &reply {
        ResolveReply::Failure { code, .. } => assert!(*code == FAIL_NO_SUCH_CHILD || *code == FAIL_NOT_AUTHORITATIVE, "code {code}"),
        other => panic!("no referral is returned: {other:?}"),
    }
    assert_eq!(t.a.child_at(1), None, "S's child table no longer lists C");
}

impl Tree {
    /// C's departure from A, signed and encoded.
    fn w_depart(&mut self) -> Vec<u8> {
        let mut w = World::new();
        w.adopt("bob", "alice", 1);
        w.depart("bob", "alice", Seqno { series: 1, counter: 1 }).bytes
    }
}

// acceptance: RES-15
#[test]
fn a_greater_counter_replaces_the_endpoint_list_reordering_included() {
    let mut t = tree();
    let e1 = point(1, 7001);
    let e2 = point(2, 7002);
    let first = endpoint_record(&id("bob"), &[e1.clone(), e2.clone()], Seqno { series: 1, counter: 4 });
    let second = endpoint_record(&id("bob"), &[e2.clone(), e1.clone()], Seqno { series: 1, counter: 5 });
    assert_ne!(first, second, "reordering alone is a change");
    t.a.take_object(&*t.afab, &kh("bob"), KIND_ENDPOINT_RECORD, &first, &ids());
    t.a.take_object(&*t.afab, &kh("bob"), KIND_ENDPOINT_RECORD, &second, &ids());
    let req = ResolveRequest { subject: kh("carol"), anchor: kh("alice"), path: path_to_x().bytes, nibbles: 2, nonce: nonce(15) };
    let ResolveReply::Referral { referral, .. } = t.a.answer_resolution(&req) else { panic!() };
    assert_eq!(referral.endpoints, vec![e2, e1], "in the new record's order");
}

// acceptance: RES-16
#[test]
fn answering_a_resolution_keeps_no_record_of_who_asked_about_whom() {
    let t = tree();
    let req = ResolveRequest { subject: kh("carol"), anchor: kh("alice"), path: path_to_x().bytes, nibbles: 2, nonce: nonce(16) };
    let before = state_digest(&t.a);
    let reply = t.a.answer_resolution(&req);
    assert!(matches!(reply, ResolveReply::Referral { .. }));
    let after = state_digest(&t.a);
    assert_eq!(before, after, "answering changes nothing the node holds");
    // the request names the subject and never the querier, so no pairing is
    // even representable in what was received
    let encoded = req.encode();
    assert!(!encoded.windows(32).any(|w| w == kh("w5")), "the querier is not named in the request");
    // and nothing retained carries the nonce
    assert!(!after.windows(16).any(|w| w == req.nonce), "no request nonce survives the answer");
}

/// Everything the node holds, flattened: the store's objects, its slots and
/// its memo table.  Two digests differing means the node retained something.
fn state_digest(v: &NodeView) -> Vec<u8> {
    let mut out = Vec::new();
    for (kind, bytes) in v.store.objects() {
        out.push(kind as u8);
        out.extend_from_slice(&bytes);
    }
    for (slot, s) in &v.slots {
        out.extend_from_slice(&slot.to_be_bytes());
        out.extend_from_slice(&s.occupant.unwrap_or([0; 32]));
        out.extend_from_slice(&s.timestamp.to_be_bytes());
    }
    for ((p, slot), s) in &v.memo_table {
        out.extend_from_slice(p);
        out.extend_from_slice(&slot.to_be_bytes());
        out.extend_from_slice(&s.occupant.unwrap_or([0; 32]));
    }
    for k in v.attached.iter().chain(v.peers.iter()) {
        out.extend_from_slice(k);
    }
    out
}

