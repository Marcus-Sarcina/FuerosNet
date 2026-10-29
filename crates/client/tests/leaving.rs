//! What leaving costs (`light-client-requirements.md` §6): the resources
//! an ending takes away, named before it happens, with the limits of the
//! answer carried beside it.

mod common;

use common::harness::*;
use common::*;
use rhtn_archive::catalog::{CatalogEntry, EntryFields};
use rhtn_archive::tx::Seqno;
use rhtn_client::horizon::Took;
use rhtn_client::leaving::{Limit, LossReason};

const PARTIES: [&str; 4] = ["alice", "bob", "w1", "w2"];

/// A catalog entry `owner` signed, as a node would have served it.
fn entry(owner: &str, resource: [u8; 32], instance: &str) -> Vec<u8> {
    CatalogEntry::build(
        &id(owner),
        &EntryFields {
            resource,
            service_type: "rhtn-forum".into(),
            instance: instance.into(),
            endpoint: b"e".to_vec(),
            connect_scope: None,
            metadata: None,
            data_practice: None,
        },
    )
}

fn res(n: u8) -> [u8; 32] {
    [n; 32]
}

/// **`node` adopted under `patron`, and the record propagated back to
/// `node`** — which is how a live client comes to hold its own binding:
/// the serving node floods the adoption and the client folds it
/// (`crates/adaptors`' attached loop). Without this step a client's
/// propagated copy has no edge of its own in it, and the question of
/// what ending one would cost has nothing to subtract.
fn adopted_and_propagated(s: &mut Setup, node: &str, patron: &str, w: &str, series: u32) {
    let t = s.adopted(node, patron, w, series);
    let c = s.client(node);
    let bytes = c.store.records[&t].clone();
    assert_eq!(
        c.horizon.ingest(&bytes, &c.known),
        Took::Applied,
        "the adoption reaches the subordinate's own copy"
    );
}

// acceptance: RSC-53
#[test]
fn leaving_a_patron_names_the_resources_it_takes_and_keeps_the_rest() {
    let mut s = setup(&PARTIES);
    adopted_and_propagated(&mut s, "bob", "alice", "w2", 1);

    // two resources bob can reach: one served and owned by the patron,
    // one of his own
    let (alice, bob) = (kh("alice"), kh("bob"));
    {
        let c = s.client("bob");
        c.catalog
            .portion(alice)
            .entries
            .insert(res(1), entry("alice", res(1), "the patron's board"));
        c.catalog
            .portion(bob)
            .entries
            .insert(res(2), entry("bob", res(2), "his own"));
    }

    let cost = s
        .client("bob")
        .cost_of_leaving(&alice)
        .expect("there is a relationship to leave");

    assert!(!cost.is_free(), "leaving a patron is not free");
    assert!(
        cost.leaving.contains(&alice),
        "the patron leaves the horizon"
    );
    assert_eq!(cost.losing.len(), 1, "one resource goes");
    let lost = &cost.losing[0];
    assert_eq!(lost.resource, res(1));
    assert_eq!(lost.node, alice, "reached through the patron");
    assert_eq!(lost.reason, LossReason::HostLeaves);
    assert_eq!(lost.instance, "the patron's board");
    assert_eq!(cost.keeping, 1, "his own is his own");

    // **the list says what it is.** A resource's access is its own
    // predicate and this client holds none, so the cost is a floor: a
    // host still in view may refuse once the line is gone.
    assert!(
        cost.limits.contains(&Limit::PredicateNotHeld),
        "always, because it is always true: {:?}",
        cost.limits
    );
}

// acceptance: RSC-54
#[test]
fn a_portion_that_could_not_be_refreshed_is_marked_rather_than_used_quietly() {
    let mut s = setup(&PARTIES);
    adopted_and_propagated(&mut s, "bob", "alice", "w2", 1);
    let alice = kh("alice");
    {
        let c = s.client("bob");
        let p = c.catalog.portion(alice);
        p.entries
            .insert(res(1), entry("alice", res(1), "the patron's board"));
        p.stale = true;
        p.truncated = true;
    }
    let cost = s.client("bob").cost_of_leaving(&alice).expect("a patron");
    assert!(
        cost.limits.contains(&Limit::Stale { node: alice }),
        "the portion is stale and the list says so: {:?}",
        cost.limits
    );
    assert!(
        cost.limits.contains(&Limit::Truncated { node: alice }),
        "and that the node held more than this client ever saw"
    );
    assert_eq!(
        cost.losing.len(),
        1,
        "a stale list is still the best answer there is, and it is shown"
    );
}

// acceptance: RSC-55
#[test]
fn there_is_no_cost_quoted_for_a_departure_that_would_be_refused() {
    let mut s = setup(&PARTIES);
    adopted_and_propagated(&mut s, "bob", "alice", "w2", 1);
    // w1 is not bob's patron, and `depart` would refuse it: so does this
    let e = s
        .client("bob")
        .cost_of_leaving(&kh("w1"))
        .expect_err("no such relationship");
    assert!(
        matches!(e, rhtn_client::ceremony::Abort::NoRelationship(p) if p == kh("w1")),
        "the same refusal the departure gives: {e:?}"
    );
}

// acceptance: RSC-54
#[test]
fn infrastructure_in_the_horizon_that_was_never_swept_is_named_as_a_gap() {
    let mut s = setup(&PARTIES);
    adopted_and_propagated(&mut s, "bob", "alice", "w2", 1);
    let alice = kh("alice");

    // **holding an endpoint record is what says a party is
    // infrastructure** (`wire-format.md` §7.6), so the patron becomes a
    // node whose catalog bob could have asked for and never did.
    let point = rhtn_transport::session::NetworkPoint::new([127, 0, 0, 1], Some(7449));
    let rec = rhtn_node::resolution::endpoint_record(
        &id("alice"),
        std::slice::from_ref(&point),
        Seqno {
            series: 1,
            counter: 0,
        },
    );
    {
        let c = s.client("bob");
        assert_eq!(
            c.horizon.ingest_endpoint(&rec, &c.known),
            Took::Applied,
            "the patron publishes where it answers"
        );
    }

    let cost = s.client("bob").cost_of_leaving(&alice).expect("a patron");
    assert!(
        cost.limits.contains(&Limit::Unswept { node: alice }),
        "what it serves is not in the list at all, and the list says so: {:?}",
        cost.limits
    );
    assert!(
        cost.losing.is_empty(),
        "and nothing is claimed about resources never seen"
    );
    assert!(
        !cost.is_free(),
        "the patron still leaves the horizon, which is a cost in itself"
    );
}

// acceptance: RSC-53
#[test]
fn a_resource_another_node_still_serves_is_not_counted_as_lost() {
    let mut s = setup(&PARTIES);
    adopted_and_propagated(&mut s, "bob", "alice", "w2", 1);
    let (alice, bob) = (kh("alice"), kh("bob"));

    // **one resource, two ways to it**: bob owns it and both the patron
    // and bob himself serve its entry.
    {
        let c = s.client("bob");
        c.catalog
            .portion(alice)
            .entries
            .insert(res(1), entry("bob", res(1), "the rota"));
        c.catalog
            .portion(bob)
            .entries
            .insert(res(1), entry("bob", res(1), "the rota"));
    }

    let cost = s.client("bob").cost_of_leaving(&alice).expect("a patron");
    assert!(
        cost.losing.is_empty(),
        "one way closing is not the loss of the resource: {:?}",
        cost.losing
    );
    assert_eq!(cost.keeping, 1, "counted once, and kept");

    // and when the only remaining way is the one that closes, it goes
    {
        let c = s.client("bob");
        c.catalog.portion(bob).entries.clear();
    }
    let cost = s.client("bob").cost_of_leaving(&alice).expect("a patron");
    assert_eq!(cost.losing.len(), 1, "the last way closed: {cost:?}");
    assert_eq!(cost.keeping, 0);
}

// acceptance: RSC-53
#[test]
fn a_resource_whose_owner_goes_is_lost_though_its_host_stays() {
    let mut s = setup(&PARTIES);
    adopted_and_propagated(&mut s, "bob", "alice", "w2", 1);
    let (alice, bob) = (kh("alice"), kh("bob"));

    // bob serves the entry himself, so the host is him and stays; the
    // owner is the patron, and the patron is what leaving loses
    {
        let c = s.client("bob");
        c.catalog
            .portion(bob)
            .entries
            .insert(res(3), entry("alice", res(3), "hers, through him"));
    }
    let cost = s.client("bob").cost_of_leaving(&alice).expect("a patron");
    assert_eq!(cost.losing.len(), 1, "{cost:?}");
    assert_eq!(cost.losing[0].node, bob, "the host is still in view");
    assert_eq!(cost.losing[0].owner, alice);
    assert_eq!(
        cost.losing[0].reason,
        LossReason::OwnerLeaves,
        "and that is a different thing to be told than a host gone"
    );
}
