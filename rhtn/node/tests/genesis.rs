//! The genesis of a new root, what it propagates, and how existing nodes
//! reach it.  A root is emergent and ordinary (design §3.1): it self-anchors
//! with an empty path (`wire-format.md` §2.1), it needs no issuer and
//! nothing it does waits on a staple (design §12.7.1, §12.6.5), its edges
//! travel as topology like anyone's (design §15.1), and it is cached by
//! whoever's anchor policy admits it (design §12.7.3).

mod common;

use common::*;
use rhtn_archive::record::Record;
use rhtn_archive::tx::{self, Locator, Seqno};
use rhtn_node::currency::{Gate, Requirement, Staple};
use rhtn_node::propagation::FRAME_TOPOLOGY_PUSH;
use rhtn_node::resolution::{AnchorEntry, AnchorTable, Carried, Ingestion, Path, REQUEST_RESOLVE, ResolveReply, ResolveRequest, Step, anchor_entry};
use rhtn_node::store::{Decision, KIND_ENDPOINT_RECORD, KIND_TRANSACTION};
use rhtn_node::view::NodeView;
use std::collections::BTreeSet;
use std::sync::Arc;

/// A root that has just come into being: its own anchor, an empty path, no
/// patron, no subordinates, no history.
fn new_root(name: &str) -> NodeView {
    let mut r = NodeView::new(Arc::new(id(name)), Locator::root(kh(name), Seqno { series: 1, counter: 0 }));
    r.set_now(1_800_000_000 + 7200);
    r
}

/// An existing subnet: alice is its root, bob alice's infra child, carol
/// bob's infra child.  Each holds both adoptions.
fn subnet() -> (World, NodeView, NodeView, NodeView) {
    let mut w = World::new();
    let (a_bob, _) = w.adopt("bob", "alice", 1);
    let (a_carol, _) = w.adopt("carol", "bob", 2);
    let infra = ["alice", "bob", "carol"];
    let mut alice = view("alice", table_with(kh("alice"), &w, &[&a_bob, &a_carol], &infra), "alice", &[]);
    let mut bob = view("bob", table_with(kh("bob"), &w, &[&a_bob, &a_carol], &infra), "alice", &[1]);
    let mut carol = view("carol", table_with(kh("carol"), &w, &[&a_bob, &a_carol], &infra), "alice", &[1, 2]);
    for v in [&mut alice, &mut bob, &mut carol] {
        v.set_now(w.clock + 7200);
    }
    (w, alice, bob, carol)
}

#[test]
fn a_new_root_comes_into_being_needing_nothing() {
    let mut r = new_root("w9");
    let me = kh("w9");
    assert!(r.is_root());
    assert_eq!(r.patron(), None);
    assert_eq!(r.anchor(), me, "a root self-anchors");
    assert_eq!(r.position.nibbles, 0, "with an empty path");
    assert!(r.table.is_root(&me));
    assert!(r.table.subordinates(&me).is_empty());
    assert_eq!(r.archive.next_back_pointers(), vec![rhtn_archive::genesis(&me)], "its chain begins at genesis");
    assert_eq!(r.evidence().known(), BTreeSet::from([me]), "it knows nobody");
    // currency is vacuous, not missing (design §12.7.1): no staple, no issuer
    // to ask, and nothing waits on either (design §12.6.5)
    assert_eq!(r.staple_for(&ids(), &me, &[]), Staple::Absent);
    let fab = Fabric::with(&[]);
    assert_eq!(r.require_currency(&*fab, &ids(), &me, None, None), Requirement::Settled(Gate::Proceed));
    assert!(fab.frames().is_empty(), "nobody is asked");
}

#[test]
fn a_new_root_s_peering_propagates_through_its_peer_into_an_existing_subnet() {
    let (mut w, mut alice, mut bob, mut carol) = subnet();
    let mut r = new_root("w9");
    // R publishes its endpoint record into its own store
    let er = r.publish_endpoints(&[point(9, 7009)], Seqno { series: 1, counter: 1 });
    let rfab = Fabric::with(&[kh("bob")]);
    assert_eq!(r.take_object(&*rfab, &kh("w9"), KIND_ENDPOINT_RECORD, &er, &ids()), Decision::Stored);
    assert_eq!(r.own_endpoints(), vec![point(9, 7009)]);
    // R and bob meet, and R proposes the peering: a naked root, no staple,
    // nobody's authority required (design §6.3, §12.6.5)
    let pop = w.meet("bob", "w9");
    let body = r.propose_peering(&kh("bob"), &point(2, 7002), &pop.txid, &[rhtn_archive::genesis(&kh("bob"))]).expect("a naked root peers");
    let peering = Record::parse(&tx::envelope(tx::TYPE_PEERING, &body, &[&id("w9"), &id("bob")])).unwrap();
    r.peers.insert(kh("bob"));
    bob.peers.insert(kh("w9"));
    // R originates it on its one session
    assert_eq!(r.originate_push(&*rfab, KIND_TRANSACTION, &peering.bytes, &ids()), Decision::Stored);
    assert_eq!(rfab.recipients(FRAME_TOPOLOGY_PUSH), BTreeSet::from([kh("bob")]));
    // bob stores it, a peering with an endpoint in its horizon, and forwards
    // it up and down but never back
    let bfab = Fabric::with(&[kh("alice"), kh("carol"), kh("w9")]);
    assert_eq!(bob.take_object(&*bfab, &kh("w9"), KIND_TRANSACTION, &peering.bytes, &ids()), Decision::Stored);
    assert_eq!(bfab.recipients(FRAME_TOPOLOGY_PUSH), BTreeSet::from([kh("alice"), kh("carol")]));
    assert_eq!(bob.peers_of(&kh("w9")), BTreeSet::from([kh("bob")]));
    // alice and carol store it too: bob is inside their horizons
    let afab = Fabric::with(&[kh("bob")]);
    let cfab = Fabric::with(&[kh("bob")]);
    assert_eq!(alice.take_object(&*afab, &kh("bob"), KIND_TRANSACTION, &peering.bytes, &ids()), Decision::Stored);
    assert_eq!(carol.take_object(&*cfab, &kh("bob"), KIND_TRANSACTION, &peering.bytes, &ids()), Decision::Stored);
    assert_eq!(afab.count(FRAME_TOPOLOGY_PUSH) + cfab.count(FRAME_TOPOLOGY_PUSH), 0, "nothing goes back the way it came");
    assert!(carol.peers_of(&kh("bob")).contains(&kh("w9")), "a node two hops from R holds R's edge");
    // R's address travels in the peering record itself
    let held = carol.peerings().into_iter().find(|p| p.a == kh("w9") || p.b == kh("w9")).expect("held");
    assert!([held.a_point, held.b_point].contains(&point(9, 7009)));
    // and the new root has standing at existing nodes: one edge beyond bob,
    // at the edge's capacity
    assert_eq!((alice.standing(&kh("w9")), carol.standing(&kh("w9"))), (10.0, 10.0));
    // what does not propagate is R's endpoint record, whose subject is in
    // nobody's horizon: reaching a stranger is the anchor table's business
    assert_eq!(bob.take_object(&*bfab, &kh("w9"), KIND_ENDPOINT_RECORD, &er, &ids()), Decision::OutOfStore);
}

#[test]
fn an_existing_node_reaches_a_new_root_through_its_anchor_table() {
    let (_w, _alice, bob, _carol) = subnet();
    let mut r = new_root("w9");
    let er = r.publish_endpoints(&[point(9, 7009)], Seqno { series: 1, counter: 1 });
    r.take_object(&*Fabric::with(&[]), &kh("w9"), KIND_ENDPOINT_RECORD, &er, &ids());
    // R's anchor entry: a subtree of nobody
    let entry = AnchorEntry::parse(&anchor_entry(&id("w9"), &[point(9, 7009)], 0, Seqno { series: 1, counter: 1 })).unwrap();
    // a node whose policy ignores roots below one subordinate does not
    // cache it (design §12.7.3)
    let mut strict = AnchorTable::new(1, Ingestion::VerifiedOnAcceptance);
    assert!(!strict.offer(entry.clone(), &ids()));
    // one whose policy admits it does, verifying on acceptance since it
    // holds R's key
    let mut anchors = AnchorTable::new(0, Ingestion::VerifiedOnAcceptance);
    assert!(anchors.offer(entry, &ids()));
    // bob resolves R: anchor R, empty path, sent to R on the session it holds
    let bfab = Fabric::with(&[kh("w9")]);
    let (mut res, carried) = bob.resolve(&*bfab, &anchors, kh("w9"), kh("w9"), Path::empty(), [3; 16]).unwrap();
    assert!(matches!(carried, Carried::Direct { sent_on_session: true }));
    let sent = bfab.requests_to(&kh("w9"), REQUEST_RESOLVE);
    assert_eq!(sent.len(), 1);
    let req = ResolveRequest::decode(&sent[0]).unwrap();
    assert_eq!((req.subject, req.anchor, req.nibbles), (kh("w9"), kh("w9"), 0));
    // R answers for itself: a self-anchored locator resolves to an empty
    // residual and R's own endpoints (design §12.6.1)
    let reply = r.answer_resolution(&req);
    let ResolveReply::Serving { nonce, serving } = &reply else { panic!("{reply:?}") };
    assert_eq!(*nonce, [3; 16]);
    assert_eq!(serving.node, kh("w9"));
    assert!(serving.residual.is_empty());
    assert_eq!(serving.endpoints, vec![point(9, 7009)]);
    assert!(matches!(res.take(&reply), Step::Arrived(_)));
    assert_eq!(res.arrived.as_ref().map(|s| s.node), Some(kh("w9")));
    // without the entry nothing is sent: a caller-side condition
    // (`wire-format.md` §7.7.1)
    let empty = AnchorTable::new(0, Ingestion::UnverifiedGossip);
    assert!(bob.resolve(&*bfab, &empty, kh("w9"), kh("w9"), Path::empty(), [4; 16]).is_err());
    assert_eq!(bfab.requests_to(&kh("w9"), REQUEST_RESOLVE).len(), 1, "no second request left");
}
