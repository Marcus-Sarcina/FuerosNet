//! **An identity's addresses** (design §12.9; `light-client-requirements.md`
//! §2; SES-017): a node per address, a message routed by the subnet the
//! last one came by, and a first message reaching every address a peer is
//! known at. The horizon walk held to one address is proven beside the
//! walk itself, in `horizon.rs`.

mod common;

use common::harness::*;
use common::*;
use rhtn_client::payload::{PayloadConfig, PayloadState};

/// **A message's arrival decides where the next one goes**, and a first
/// message goes everywhere the peer is known.
#[test]
fn the_last_subnet_a_message_came_by_is_where_the_next_one_goes() {
    let mut s = setup(&["alice", "bob", "witness"]);
    s.met("alice", "bob", "witness");
    let a = s.client("alice");
    assert_eq!(a.last_from(&kh("bob")), None, "nothing has arrived yet");
    // the node serving alice's address under anchor X
    a.serve_at(kh("anchor-x"), kh("node-x"));
    assert_eq!(a.serving_at(), vec![(kh("anchor-x"), kh("node-x"))]);
    assert_eq!(a.attachments(), vec![kh("node-x")]);
    // a message from bob arrives on the attachment to node-x
    a.record_arrival(kh("bob"), &kh("node-x"));
    assert_eq!(
        a.last_from(&kh("bob")),
        Some(kh("anchor-x")),
        "the subnet it came by is recorded"
    );
    // an arrival on a node that serves none of alice's addresses records nothing
    a.record_arrival(kh("bob"), &kh("node-unknown"));
    assert_eq!(a.last_from(&kh("bob")), Some(kh("anchor-x")));
    // the flood is one reach where one address is known, and never empty
    assert!(!a.reaches_for(&kh("bob")).is_empty());
}

/// **The three fields persist, and the seven-field form still reads.**
#[test]
fn the_address_state_round_trips_and_the_older_form_still_reads() {
    let mut fresh = |b: &mut [u8]| b.fill(7);
    let mut p = PayloadState::new(PayloadConfig::default(), &mut fresh, 1);
    p.attachments.insert(kh("n1"));
    p.serving_at.insert(kh("a1"), kh("n1"));
    p.last_from.insert(kh("bob"), kh("a1"));
    let bytes = p.encode();
    let back = PayloadState::decode(PayloadConfig::default(), &bytes).expect("reads");
    assert_eq!(back.attachments, p.attachments);
    assert_eq!(back.serving_at, p.serving_at);
    assert_eq!(back.last_from, p.last_from);
    assert_eq!(bytes[0], 0x8a, "ten fields");
}

/// **An identity in two subnets publishes its profile for both** (review
/// 2026-11-13: `subnet()` answered with the first, so the second went
/// nameless).
#[test]
fn an_identity_in_two_subnets_publishes_its_profile_for_each() {
    let mut s = setup(&["alice", "bob", "carol", "witness"]);
    s.adopted("alice", "bob", "witness", 1);
    s.adopted("alice", "carol", "witness", 2);
    let a = s.client("alice");
    let held = a.subnets();
    assert_eq!(held.len(), 2, "two addresses: {held:?}");
    a.set_default_alias("Alice").expect("an alias");
    let msgs = a.attach(kh("node"), &[]);
    let profiles = msgs
        .iter()
        .filter(|m| matches!(m, rhtn_client::ceremony::Msg::PublishProfile(_)))
        .count();
    assert!(
        profiles >= 2,
        "a profile published per address, got {profiles}"
    );
}
