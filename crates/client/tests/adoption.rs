//! **What a client checks before it signs an adoption of itself**
//! (`light-client-requirements.md` §7; design §6.1.1, where an adoption
//! carries both signatures).
//!
//! The patron builds the body and the party being adopted is shown it.
//! Signing one puts this key under that patron, in a series, at an
//! address this device did not choose, so every field is checked against
//! what this client holds. A body that fails is refused with its reason,
//! which is an answer and not a fault.

mod common;

use common::harness::*;
use common::*;
use rhtn_archive::tx::{Adoption, Evidence, Locator, Seqno, adoption_body};
use rhtn_client::ceremony::{Abort, Adopting};

const PARTIES: [&str; 5] = ["alice", "bob", "carol", "w1", "w2"];

/// Where the patron sits, as the party it adopts holds it: the locator
/// its intent carried (`wire-format.md` §14.3.2).
fn held_position(s: &mut Setup, node: &str, patron: &str) -> Locator {
    let sl = s
        .client(node)
        .store
        .contacts
        .get(&kh(patron))
        .and_then(|c| c.locator.clone())
        .expect("the patron's own locator, from its intent");
    rhtn_archive::locator::SignedLocator::parse(&sl)
        .expect("a signed locator")
        .locator
}

/// A body the patron would never have built: every field the caller's,
/// over a meeting that did happen.
#[allow(clippy::too_many_arguments)]
fn body_of(
    node: &str,
    patron: &str,
    locator: Locator,
    timestamp: u64,
    evidence: Evidence,
    node_back: &[[u8; 32]],
    patron_back: &[[u8; 32]],
) -> Vec<u8> {
    adoption_body(&Adoption {
        node: kh(node),
        patron: kh(patron),
        locator,
        timestamp,
        key_material: None,
        evidence,
        presented_head: None,
        back: [node_back, patron_back],
        carrying: None,
    })
}

fn refusal(e: Abort) -> String {
    match e {
        Abort::NodeRefused(why) => why,
        other => panic!("not a refusal of the body: {other:?}"),
    }
}

// acceptance: CER-57
#[test]
fn the_body_the_patron_proposed_on_the_meeting_they_had_is_signed_and_taken() {
    let mut s = setup(&PARTIES);
    let t = s.adopted("bob", "alice", "w2", 1);
    // both hold it, and the subordinate knows where it now sits
    for n in ["bob", "alice"] {
        assert!(s.client(n).store.records.contains_key(&t), "{n} holds it");
    }
    // the subnet is named by the root's first adoption: this one
    let subnet = s.client("alice").subnets()[0];
    assert_eq!(subnet, t, "by its own txid");
    assert_eq!(subnet, s.client("bob").subnet().expect("bob is in it"));
    let pos = s
        .client("bob")
        .position_in(&subnet)
        .expect("a position in the patron's subnet");
    assert_eq!(pos.anchor, kh("alice"));
    assert_eq!(pos.nibbles, 1, "one hop below a root");
    assert_eq!(
        pos.seqno,
        Seqno {
            series: 1,
            counter: 0
        }
    );
}

// acceptance: CER-58
#[test]
fn a_body_adopting_another_party_is_not_this_clients_to_sign() {
    let mut s = setup(&PARTIES);
    let pop = s.met_in_person("bob", "alice", "w2");
    let where_alice = held_position(&mut s, "bob", "alice");
    // alice's body, adopting carol: bob is not its subject
    let body = body_of(
        "carol",
        "alice",
        Locator {
            anchor: where_alice.anchor,
            path: vec![0x10],
            nibbles: 1,
            seqno: Seqno {
                series: 1,
                counter: 0,
            },
            subnet: None,
        },
        s.clock.get() / 1000,
        Evidence::Presence(pop),
        &[pop],
        &[pop],
    );
    let why = refusal(s.client("bob").review_and_sign_adoption(&body).unwrap_err());
    assert!(why.contains("another party"), "{why}");
}

// acceptance: CER-58
#[test]
fn a_body_resting_on_a_meeting_with_somebody_else_is_refused() {
    let mut s = setup(&PARTIES);
    // bob met alice, and witnessed a meeting of alice and carol — so he
    // holds that record too, and it is not evidence of anything about
    // him. (A record he holds nothing of is refused one step earlier, for
    // not being held at all.)
    let theirs = s.met_in_person("bob", "alice", "w2");
    let elsewhere = s.met("alice", "carol", "bob");
    let where_alice = held_position(&mut s, "bob", "alice");
    let body = body_of(
        "bob",
        "alice",
        Locator {
            anchor: where_alice.anchor,
            path: vec![0x10],
            nibbles: 1,
            seqno: Seqno {
                series: 1,
                counter: 0,
            },
            subnet: None,
        },
        s.clock.get() / 1000,
        Evidence::Presence(elsewhere),
        &[theirs],
        &[elsewhere],
    );
    let why = refusal(s.client("bob").review_and_sign_adoption(&body).unwrap_err());
    assert!(why.contains("other parties"), "{why}");
}

// acceptance: CER-60
#[test]
fn a_series_this_key_has_occupied_is_refused() {
    let mut s = setup(&PARTIES);
    s.adopted("bob", "alice", "w2", 1);
    // carol proposes the series bob already occupies under alice
    let pop = s.met_in_person("bob", "carol", "w2");
    let node_back = s.client("bob").back_pointers();
    let body = s
        .client("carol")
        .propose_adoption_in(
            None,
            kh("bob"),
            &node_back,
            Adopting {
                evidence: Evidence::Presence(pop),
                series: 1,
                presented_head: None,
                key_material: None,
                carrying: None,
            },
        )
        .expect("the patron will propose it");
    let why = refusal(s.client("bob").review_and_sign_adoption(&body).unwrap_err());
    assert!(why.contains("series 1"), "{why}");
    // and series 2 is taken, the same patron and the same meeting
    let body = s
        .client("carol")
        .propose_adoption_in(
            None,
            kh("bob"),
            &node_back,
            Adopting {
                evidence: Evidence::Presence(pop),
                series: 2,
                presented_head: None,
                key_material: None,
                carrying: None,
            },
        )
        .expect("proposed");
    assert!(s.client("bob").review_and_sign_adoption(&body).is_ok());
}

// acceptance: CER-59
#[test]
fn an_address_that_is_not_the_patrons_to_issue_is_refused() {
    let mut s = setup(&PARTIES);
    let pop = s.met_in_person("bob", "alice", "w2");
    let where_alice = held_position(&mut s, "bob", "alice");
    let now = s.clock.get() / 1000;
    let at = |anchor, path: Vec<u8>, nibbles, subnet| {
        body_of(
            "bob",
            "alice",
            Locator {
                anchor,
                path,
                nibbles,
                seqno: Seqno {
                    series: 1,
                    counter: 0,
                },
                subnet,
            },
            now,
            Evidence::Presence(pop),
            &[pop],
            &[pop],
        )
    };
    // another subnet altogether
    let why = refusal(
        s.client("bob")
            .review_and_sign_adoption(&at(kh("carol"), vec![0x10], 1, None))
            .unwrap_err(),
    );
    assert!(why.contains("one hop below"), "{why}");
    // the patron's own subnet, two hops down: not a slot of the patron's
    // (named, as two hops down must be, so that the shape is what fails)
    let why = refusal(
        s.client("bob")
            .review_and_sign_adoption(&at(where_alice.anchor, vec![0x12], 2, Some(pop)))
            .unwrap_err(),
    );
    assert!(why.contains("one hop below"), "{why}");
    // and the address the patron would issue is taken
    assert!(
        s.client("bob")
            .review_and_sign_adoption(&at(where_alice.anchor, vec![0x10], 1, None))
            .is_ok()
    );
}

// acceptance: CER-61
#[test]
fn back_pointers_that_are_not_this_archives_are_refused() {
    let mut s = setup(&PARTIES);
    let pop = s.met_in_person("bob", "alice", "w2");
    let where_alice = held_position(&mut s, "bob", "alice");
    let body = body_of(
        "bob",
        "alice",
        Locator {
            anchor: where_alice.anchor,
            path: vec![0x10],
            nibbles: 1,
            seqno: Seqno {
                series: 1,
                counter: 0,
            },
            subnet: None,
        },
        s.clock.get() / 1000,
        Evidence::Presence(pop),
        &[rhtn_archive::genesis(&kh("bob"))],
        &[pop],
    );
    let why = refusal(s.client("bob").review_and_sign_adoption(&body).unwrap_err());
    assert!(why.contains("back-pointers"), "{why}");
}

// acceptance: CER-61
#[test]
fn a_timestamp_this_clock_cannot_hold_is_refused() {
    let mut s = setup(&PARTIES);
    let pop = s.met_in_person("bob", "alice", "w2");
    let where_alice = held_position(&mut s, "bob", "alice");
    let at = |when| {
        body_of(
            "bob",
            "alice",
            Locator {
                anchor: where_alice.anchor,
                path: vec![0x10],
                nibbles: 1,
                seqno: Seqno {
                    series: 1,
                    counter: 0,
                },
                subnet: None,
            },
            when,
            Evidence::Presence(pop),
            &[pop],
            &[pop],
        )
    };
    let now = s.clock.get() / 1000;
    let why = refusal(
        s.client("bob")
            .review_and_sign_adoption(&at(now + 86_400))
            .unwrap_err(),
    );
    assert!(why.contains("ahead of this clock"), "{why}");
    // and before the meeting it rests on
    let meeting = rhtn_archive::record::Record::parse(&s.client("bob").store.records[&pop])
        .expect("the meeting")
        .effective;
    let why = refusal(
        s.client("bob")
            .review_and_sign_adoption(&at(meeting - 1))
            .unwrap_err(),
    );
    assert!(why.contains("before the meeting"), "{why}");
}

// acceptance: CER-59
#[test]
fn a_patron_whose_own_position_is_not_held_is_refused() {
    let mut s = setup(&PARTIES);
    // a record of the meeting, and no intent: nothing says where alice
    // sits, so nothing says the address is hers to issue
    let pop = s.met("bob", "alice", "w2");
    let body = body_of(
        "bob",
        "alice",
        Locator {
            anchor: kh("alice"),
            path: vec![0x10],
            nibbles: 1,
            seqno: Seqno {
                series: 1,
                counter: 0,
            },
            subnet: None,
        },
        s.clock.get() / 1000,
        Evidence::Presence(pop),
        &[pop],
        &[pop],
    );
    let why = refusal(s.client("bob").review_and_sign_adoption(&body).unwrap_err());
    assert!(why.contains("no position of the patron's own"), "{why}");
}

/// **A recovery adoption is not signed on this path** (design §9.0): its
/// subject assembles the block from what its own meetings yielded, and
/// signs the body the patron builds over it with `sign_body`. A body
/// carrying somebody else's recovery block, shown to a client that
/// assembled nothing, is refused here.
// acceptance: CER-62
#[test]
fn a_body_carrying_a_recovery_block_is_refused_on_this_path() {
    let mut s = setup(&["alice", "alice2", "bob", "carol", "w1", "w2"]);
    s.met_in_person("alice", "bob", "w2");
    s.met_in_person("alice", "carol", "w2");
    s.adopted("alice", "w1", "w2", 3);
    // alice's device is alice2's now: the prior key and its archive
    let old_archive = s.client("alice").archive.clone();
    s.client("alice2").rotation = Some(Box::new(rhtn_client::rotation::Rotation::begin(
        Box::new(id("alice")),
        old_archive,
    )));
    s.face_off("alice2", "bob");
    s.h.run_recovery_meeting(kh("alice2"), kh("bob"))
        .expect("bob recognises");
    s.face_off("alice2", "carol");
    s.h.run_recovery_meeting(kh("alice2"), kh("carol"))
        .expect("carol recognises");
    let (block, head) = s
        .client("alice2")
        .recovery_block(&kh("w1"))
        .expect("a block from the meetings");
    let node_back = s.client("alice2").back_pointers();
    let material = s.client("alice2").public.key_material();
    let body = s
        .client("w1")
        .propose_adoption_in(
            None,
            kh("alice2"),
            &node_back,
            Adopting {
                evidence: Evidence::Recovery(block),
                series: 7,
                presented_head: head,
                key_material: Some(material),
                carrying: None,
            },
        )
        .expect("the patron proposes the recovery adoption");
    let why = refusal(
        s.client("alice2")
            .review_and_sign_adoption(&body)
            .unwrap_err(),
    );
    assert!(why.contains("recovery adoption"), "{why}");
}

/// **A patron placed in somebody else's subnet may still adopt under its
/// own key** (`wire-format.md` §2.1, design §13.8), and the party it
/// adopts accepts the address: the locator that party holds for the patron
/// is the patron's position elsewhere, which says nothing against a root
/// of the patron's own. On the bench MRK, adopted under GXV an hour before,
/// chose a new subnet under its own key and GXV refused the body against
/// MRK's position in GXV's subnet (run 9613069-a2-nodeless-reverse-1,
/// 2026-10-24).
#[test]
fn a_patron_placed_elsewhere_adopts_under_its_own_key_and_the_adopted_accepts() {
    let mut s = setup(&PARTIES);
    // bob sits under w1; what alice holds for bob is that position
    s.adopted("bob", "w1", "w2", 1);
    let pop = s.met_in_person("alice", "bob", "w2");
    let held = held_position(&mut s, "alice", "bob");
    assert_eq!(held.anchor, kh("w1"), "alice holds bob's position under w1");
    // bob adopts alice under its own key: a new subnet, named by nothing yet
    let back = s.client("alice").back_pointers();
    let body = s
        .client("bob")
        .propose_adoption_in(
            None,
            kh("alice"),
            &back,
            Adopting {
                evidence: Evidence::Presence(pop),
                series: 1,
                presented_head: None,
                key_material: None,
                carrying: None,
            },
        )
        .expect("bob proposes under its own key");
    let r = rhtn_codec::cbor::value_slice(&body, 3).expect("a locator");
    let loc = Locator::decode(&body[r]).expect("a locator");
    assert_eq!((loc.anchor, loc.nibbles, loc.subnet), (kh("bob"), 1, None));
    s.client("alice")
        .review_and_sign_adoption(&body)
        .expect("the address is the patron's to issue, under its own key");
}

// acceptance: TOP-54
/// **What a crown is drawn from** (`Robot/light-client-screens.md`): what
/// this key is in each subnet it holds a position in.
#[test]
fn a_key_knows_whether_it_roots_a_subnet_and_how_many_sit_under_it_there() {
    let mut s = setup(&PARTIES);
    // alice adopts bob, which names alice's subnet by that adoption's own
    // txid (design §13.8), and then carol into the same one
    let first = s.adopted("bob", "alice", "w2", 1);
    s.adopted("carol", "alice", "w2", 2);

    let standing = s.client("alice").subnet_rank();
    let here = standing
        .iter()
        .find(|x| x.subnet == first)
        .unwrap_or_else(|| panic!("alice's own subnet is among {standing:?}"));
    assert!(here.root, "alice is the root this subnet descends from");
    assert_eq!(here.subordinates, 2, "both the lines she granted");
    assert!(!here.operator, "and she has published no endpoint record");

    // **and the subordinate's own view is the other one**: bob is in the
    // same subnet, roots nothing, and holds nobody under him
    let his = s.client("bob").subnet_rank();
    let mine = his
        .iter()
        .find(|x| x.subnet == first)
        .unwrap_or_else(|| panic!("bob sits in it too: {his:?}"));
    assert!(!mine.root, "bob did not root it");
    assert_eq!(mine.subordinates, 0, "and nobody sits under him");
}

// acceptance: TOP-54
#[test]
fn a_line_that_ended_stops_being_counted_and_an_untold_one_still_counts() {
    let mut s = setup(&PARTIES);
    let first = s.adopted("bob", "alice", "w2", 1);
    let second = s.adopted("carol", "alice", "w2", 2);

    // **what alice granted counts before anything propagates back.** Her
    // own archive holds the adoptions she signed, and a patron whose
    // copy of the topology has not been told about them would otherwise
    // wear no crown at all — the wrong way round for a mark on a screen.
    assert_eq!(
        s.client("alice")
            .subnet_rank()
            .iter()
            .find(|x| x.subnet == first)
            .expect("her subnet")
            .subordinates,
        2,
    );

    // bob leaves, and the serving node floods both the adoptions and the
    // departure back to her — **both, because a departure settles against
    // the binding it ends**, and a copy never told of the adoption holds
    // the departure pending instead of applying it
    let left = s
        .client("bob")
        .depart(kh("alice"), None)
        .expect("he may leave");
    let flooded: Vec<Vec<u8>> = {
        let c = s.client("bob");
        vec![c.store.records[&left].clone()]
    };
    let adoptions: Vec<Vec<u8>> = {
        let c = s.client("alice");
        vec![
            c.store.records[&first].clone(),
            c.store.records[&second].clone(),
        ]
    };
    {
        let c = s.client("alice");
        for bytes in adoptions.iter().chain(flooded.iter()) {
            c.horizon.ingest(bytes, &c.known);
        }
    }

    let after = s.client("alice").subnet_rank();
    let here = after
        .iter()
        .find(|x| x.subnet == first)
        .expect("her subnet");
    assert!(here.root, "she is still its root");
    assert_eq!(here.subordinates, 1, "carol alone now: {after:?}");
}

// acceptance: TOP-55
/// **The gold circlet: an infra node of this key's own, in this subnet**
/// [author, 2026-10-28].
///
/// An endpoint record names a node and its addresses and **no subnet**
/// (`wire-format.md` §7.6); what ties one to a subnet is the line it is
/// signed on. And the instance is a device of this identity rather than
/// a key of its own, so the record is this key's.
#[test]
fn an_endpoint_record_on_a_subnets_own_line_is_what_makes_a_node_here() {
    let mut s = setup(&PARTIES);
    let subnet = s.adopted("bob", "alice", "w2", 1);

    // bob sits in it and runs nothing
    assert!(
        !s.client("bob")
            .subnet_rank()
            .iter()
            .find(|x| x.subnet == subnet)
            .expect("his subnet")
            .operator,
    );

    // he publishes where his instance answers, on the line he holds here
    let points = vec![{
        let mut p = Vec::new();
        rhtn_transport::session::NetworkPoint::new([127, 0, 0, 1], Some(7449)).encode(&mut p);
        p
    }];
    let rec = s
        .client("bob")
        .endpoint_record_in(Some(subnet), &points, 0)
        .expect("a record on this subnet's line");
    {
        let c = s.client("bob");
        assert_eq!(
            c.horizon.ingest_endpoint(&rec, &c.known),
            rhtn_client::horizon::Took::Applied,
            "and it comes back to him as it does to everyone",
        );
    }

    let his = s.client("bob").subnet_rank();
    let here = his.iter().find(|x| x.subnet == subnet).expect("his subnet");
    assert!(here.operator, "so he runs a node in this subnet: {his:?}");
    assert!(!here.root, "which is not the same as rooting it");
    assert_eq!(here.subordinates, 0, "nor as holding anybody under him");
}
