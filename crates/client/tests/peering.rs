//! **A peering a light client makes** (`wire-format.md` §4.4; design §6.3).
//!
//! Peering is an edge between two parties who have met, carrying none of
//! the subnet's authority (design §15.1). A phone runs no instance, so
//! each party's field holds its locator rather than a network point —
//! "its network point where that party runs an instance, its locator
//! otherwise" [author, 2026-09-29] — and the presence record is required
//! unconditionally, peers sharing no prior relationship to draw on.

mod common;

use common::harness::*;
use common::*;
use rhtn_client::ceremony::Abort;

const PARTIES: [&str; 4] = ["alice", "bob", "w1", "w2"];

/// The locator in one of a peering body's address fields.
fn at(body: &[u8], field: u64) -> rhtn_archive::tx::Locator {
    let r = rhtn_codec::cbor::value_slice(body, field).expect("an address field");
    rhtn_archive::tx::Locator::decode(&body[r]).expect("a locator, since neither runs an instance")
}

/// A keyhash field of a body, past its own CBOR head.
fn key_at(body: &[u8], field: u64) -> [u8; 32] {
    let item = rhtn_codec::cbor::parse_all(body).expect("a body");
    let rhtn_codec::cbor::Item::Map(m) = &item else {
        panic!("a map")
    };
    match rhtn_codec::cbor::map_get(m, field) {
        Some(rhtn_codec::cbor::Item::Bytes(r)) => body[r.clone()].try_into().expect("32 bytes"),
        _ => panic!("field {field} is not a keyhash"),
    }
}

fn refusal(e: Abort) -> String {
    match e {
        Abort::NodeRefused(why) => why,
        other => panic!("not a refusal of the body: {other:?}"),
    }
}

// acceptance: REP-17
#[test]
fn two_parties_who_met_peer_by_locator_and_both_hold_the_record() {
    let mut s = setup(&PARTIES);
    // both are in a subnet, so both have a position to write down
    s.adopted("alice", "w1", "w2", 1);
    s.adopted("bob", "w1", "w2", 2);
    let pop = s.met_in_person("alice", "bob", "w2");

    let theirs = s.client("bob").back_pointers();
    let body = s
        .client("alice")
        .propose_peering(kh("bob"), pop, &theirs, Some(1 << 20))
        .expect("alice proposes");

    // the parties are in keyhash order and each field holds that party's
    // own position (§4.4): alice's as she holds it, bob's as she was
    // handed it
    let (lo, hi) = if kh("alice") <= kh("bob") {
        (kh("alice"), kh("bob"))
    } else {
        (kh("bob"), kh("alice"))
    };
    assert_eq!(key_at(&body, 1), lo);
    assert_eq!(key_at(&body, 2), hi);
    let field_of = |who: [u8; 32]| if who == lo { 3 } else { 4 };
    assert_eq!(
        at(&body, field_of(kh("alice"))).anchor,
        kh("w1"),
        "alice's position is in w1's subnet"
    );
    assert_eq!(at(&body, field_of(kh("bob"))).anchor, kh("w1"));

    // bob checks it against what he holds and signs; alice assembles
    let entries = s
        .client("bob")
        .review_and_sign_peering(&body)
        .expect("bob signs his own peering");
    let mine = s.client("alice").sign_body(&body).expect("alice signs");
    let env = rhtn_archive::tx::envelope_from_entries(
        rhtn_archive::tx::TYPE_PEERING,
        &body,
        &[(kh("alice"), mine), (kh("bob"), entries)],
    );
    let t = s
        .client("alice")
        .take_peering(&env)
        .expect("alice takes it");
    assert_eq!(s.client("bob").take_peering(&env).expect("bob takes it"), t);
    for n in ["alice", "bob"] {
        assert!(s.client(n).store.records.contains_key(&t), "{n} holds it");
    }
    // and each now counts the other among its peers, read from the record
    // and kept nowhere else (design §6.3)
    assert_eq!(s.client("alice").peers(), vec![kh("bob")]);
    assert_eq!(s.client("bob").peers(), vec![kh("alice")]);
    assert!(
        s.client("w1").peers().is_empty(),
        "a witness is nobody's peer"
    );
    // and a peering is no part of a position: both still sit where they sat
    let subnet = s.client("w1").subnets()[0];
    assert_eq!(
        s.client("alice").position_in(&subnet).map(|l| l.nibbles),
        Some(1)
    );
}

// acceptance: REP-17
#[test]
fn a_peering_rests_on_a_meeting_of_the_two_and_on_positions_each_holds() {
    let mut s = setup(&PARTIES);
    s.adopted("alice", "w1", "w2", 1);
    s.adopted("bob", "w1", "w2", 2);
    let theirs = s.client("bob").back_pointers();

    // a meeting of other parties is not evidence of this pair
    let elsewhere = s.met_in_person("alice", "w2", "w1");
    let why = refusal(
        s.client("alice")
            .propose_peering(kh("bob"), elsewhere, &theirs, None)
            .expect_err("refused"),
    );
    assert!(why.contains("other parties"), "{why}");

    // a record held nowhere here is refused one step earlier
    let why = refusal(
        s.client("alice")
            .propose_peering(kh("bob"), [9; 32], &theirs, None)
            .expect_err("refused"),
    );
    assert!(why.contains("held here"), "{why}");

    // and a peer whose position this client has never been handed has
    // nothing to write in its field
    let pop = s.met("bob", "w1", "w2");
    let unmet = s.met_in_person("alice", "bob", "w2");
    let _ = pop;
    s.client("alice")
        .store
        .contacts
        .get_mut(&kh("bob"))
        .unwrap()
        .locator = None;
    let why = refusal(
        s.client("alice")
            .propose_peering(kh("bob"), unmet, &theirs, None)
            .expect_err("refused"),
    );
    assert!(why.contains("no position is held"), "{why}");

    // a peering with oneself is not one
    assert!(matches!(
        s.client("alice")
            .propose_peering(kh("alice"), unmet, &theirs, None),
        Err(Abort::Malformed(_))
    ));
}

// acceptance: REP-17
#[test]
fn a_peering_body_is_refused_where_it_misstates_this_partys_own_side() {
    let mut s = setup(&PARTIES);
    s.adopted("alice", "w1", "w2", 1);
    s.adopted("bob", "w1", "w2", 2);
    let pop = s.met_in_person("alice", "bob", "w2");
    let theirs = s.client("bob").back_pointers();

    // a peering between two other parties is not bob's to sign
    let others = s.met_in_person("alice", "w2", "w1");
    let w2_back = s.client("w2").back_pointers();
    let elsewhere = s
        .client("alice")
        .propose_peering(kh("w2"), others, &w2_back, None)
        .expect("alice and w2 peer");
    let why = refusal(
        s.client("bob")
            .review_and_sign_peering(&elsewhere)
            .expect_err("refused"),
    );
    assert!(why.contains("other parties"), "{why}");

    // back-pointers that are not bob's archive's next
    let wrong = s
        .client("alice")
        .propose_peering(kh("bob"), pop, &[rhtn_archive::genesis(&kh("bob"))], None)
        .expect("proposed");
    let why = refusal(
        s.client("bob")
            .review_and_sign_peering(&wrong)
            .expect_err("refused"),
    );
    assert!(why.contains("back-pointers"), "{why}");

    // the body alice would build is signed
    let body = s
        .client("alice")
        .propose_peering(kh("bob"), pop, &theirs, None)
        .expect("proposed");
    assert!(s.client("bob").review_and_sign_peering(&body).is_ok());
}

/// Alice and bob, peered: the record held by both, as the test above
/// builds it.
fn peered(s: &mut common::harness::Setup) {
    s.adopted("alice", "w1", "w2", 1);
    s.adopted("bob", "w1", "w2", 2);
    let pop = s.met_in_person("alice", "bob", "w2");
    let theirs = s.client("bob").back_pointers();
    let body = s
        .client("alice")
        .propose_peering(kh("bob"), pop, &theirs, Some(1 << 20))
        .expect("alice proposes");
    let entries = s
        .client("bob")
        .review_and_sign_peering(&body)
        .expect("bob signs");
    let mine = s.client("alice").sign_body(&body).expect("alice signs");
    let env = rhtn_archive::tx::envelope_from_entries(
        rhtn_archive::tx::TYPE_PEERING,
        &body,
        &[(kh("alice"), mine), (kh("bob"), entries)],
    );
    s.client("alice")
        .take_peering(&env)
        .expect("alice takes it");
    s.client("bob").take_peering(&env).expect("bob takes it");
}

/// **What a peer holds, and what it gives back** (design §6.3, the author's
/// six steps of 2026-10-24): a blob under the owner's identity that the
/// peer cannot open; a contact the owner lost comes back from it on the
/// next contact; nothing the owner still holds is replaced; and the sealed
/// captures of others never cross.
// acceptance: TX-031, PAY-018
#[test]
fn a_peer_holds_a_blob_it_cannot_open_and_the_owner_recovers_a_lost_contact_from_it() {
    let mut s = setup(&PARTIES);
    peered(&mut s);
    assert!(
        s.client("alice").store.contacts.contains_key(&kh("w2")),
        "the witness is a contact"
    );
    // a sealed capture of somebody, which a peer must not be handed
    s.client("alice").store.sealed.insert(
        [9u8; 32],
        rhtn_client::store::SealedCapture {
            subject: kh("w2"),
            holder: kh("alice"),
            ceremony_id: [9u8; 32],
            modality: 1,
            template_version: 1,
            slots: vec![vec![1u8; 40]],
        },
    );

    // step 4: the deposit, as the arm that takes kind 27 keeps it
    let blob = s
        .client("alice")
        .export_for_peer()
        .expect("alice exports for a peer");
    assert!(
        s.client("bob").reconcile_from_peer(&blob).is_err(),
        "bob holds another key: the blob does not open for him"
    );
    s.client("bob")
        .store
        .held_for
        .insert(kh("alice"), blob.clone());
    let opened = s
        .client("alice")
        .import(&blob)
        .expect("the owner opens it")
        .0;
    assert!(
        opened.store.sealed.is_empty(),
        "the sealed captures of others did not cross"
    );
    assert!(opened.store.contacts.contains_key(&kh("w2")));

    // step 5: the macguffin goes
    assert!(s.client("alice").store.contacts.remove(&kh("w2")).is_some());

    // step 6: the return, reconciled
    let held = s.client("bob").store.held_for[&kh("alice")].clone();
    let r = s
        .client("alice")
        .reconcile_from_peer(&held)
        .expect("the owner takes its own blob");
    assert_eq!(r.contacts, 1, "the lost contact came back");
    assert_eq!(r.records, 0, "and no record was missing");
    assert!(s.client("alice").store.contacts.contains_key(&kh("w2")));

    // a second reconcile changes nothing: what is held is not replaced
    let r = s.client("alice").reconcile_from_peer(&held).expect("again");
    assert_eq!(r, rhtn_client::ceremony::Reconciled::default());

    // and the greeting queues for a peer and for nobody else
    assert!(s.client("alice").greet_peer(kh("bob")).is_some());
    assert!(
        !s.client("alice").outbox().is_empty(),
        "a request and a deposit are queued"
    );
    assert!(
        s.client("alice").greet_peer(kh("w1")).is_none(),
        "a witness is not a peer"
    );
}
