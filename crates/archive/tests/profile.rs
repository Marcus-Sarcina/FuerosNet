//! A profile (`wire-format.md` §7.11): signed by its subject, bounded,
//! and read back whole; the fetch and its reply round-trip.

mod common;

use common::World;
use rhtn_archive::profile::*;

const SUBNET: [u8; 32] = [0xdb; 32];

#[test]
fn a_profile_is_signed_by_its_subject_and_reads_back_whole() {
    let w = World::new(&["alice", "bob"]);
    let alice = w.id("alice");
    let bytes = Profile::build(
        alice,
        &SUBNET,
        KIND_OWN,
        "Alice",
        Some((IMAGE_PNG, b"png")),
        1_800_000_000,
    );
    let p = Profile::parse(&bytes).expect("parses");
    assert_eq!(p.subject, w.kh("alice"));
    assert_eq!(p.subnet, SUBNET);
    assert_eq!(p.kind, KIND_OWN);
    assert_eq!(p.alias, "Alice");
    assert_eq!(p.image, Some((IMAGE_PNG, b"png".to_vec())));
    assert_eq!(p.published_at, 1_800_000_000);
    p.verify(&w.lookup()).expect("signed by alice");

    // without an image, and the subnet's own
    let bare = Profile::build(alice, &SUBNET, KIND_SUBNET, "Bowling League", None, 1);
    let b = Profile::parse(&bare).expect("parses");
    assert_eq!((b.kind, b.image), (KIND_SUBNET, None));

    // **a profile names whoever signed it**: bob can publish an "Alice",
    // and it is bob's profile, under bob's key, not a claim about alice
    let bobs = Profile::parse(&Profile::build(
        w.id("bob"),
        &SUBNET,
        KIND_OWN,
        "Alice",
        None,
        2,
    ))
    .unwrap();
    assert_eq!(bobs.subject, w.kh("bob"));
    bobs.verify(&w.lookup()).expect("signed by bob");

    // and a signature moved onto other fields fails: alice's signature
    // slot over a profile with a different alias
    let other = Profile::build(
        alice,
        &SUBNET,
        KIND_OWN,
        "Alicia",
        Some((IMAGE_PNG, b"png")),
        1_800_000_000,
    );
    let sig = rhtn_codec::cbor::value_slice(&bytes, 7).unwrap();
    let sig_other = rhtn_codec::cbor::value_slice(&other, 7).unwrap();
    let mut spliced = other[..sig_other.start].to_vec();
    spliced.extend_from_slice(&bytes[sig.clone()]);
    assert!(
        Profile::parse(&spliced)
            .unwrap()
            .verify(&w.lookup())
            .is_err()
    );
}

#[test]
fn a_profile_over_the_bounds_or_of_an_unknown_kind_is_malformed() {
    let w = World::new(&["alice"]);
    let alice = w.id("alice");
    let long = "x".repeat(65);
    assert!(Profile::parse(&Profile::build(alice, &SUBNET, KIND_OWN, &long, None, 1)).is_err());
    assert!(
        Profile::parse(&Profile::build(
            alice,
            &SUBNET,
            KIND_OWN,
            &"y".repeat(64),
            None,
            1
        ))
        .is_ok()
    );
    assert!(Profile::parse(&Profile::build(alice, &SUBNET, KIND_OWN, "", None, 1)).is_err());
    let big = vec![0u8; 65_537];
    assert!(
        Profile::parse(&Profile::build(
            alice,
            &SUBNET,
            KIND_OWN,
            "a",
            Some((IMAGE_JPEG, &big)),
            1
        ))
        .is_err()
    );
    let max = vec![0u8; 65_536];
    assert!(
        Profile::parse(&Profile::build(
            alice,
            &SUBNET,
            KIND_OWN,
            "a",
            Some((IMAGE_WEBP, &max)),
            1
        ))
        .is_ok()
    );
    assert!(
        Profile::parse(&Profile::build(
            alice,
            &SUBNET,
            KIND_OWN,
            "a",
            Some((4, b"x")),
            1
        ))
        .is_err()
    );
    assert!(Profile::parse(&Profile::build(alice, &SUBNET, 2, "a", None, 1)).is_err());
}

#[test]
fn a_fetch_and_its_reply_round_trip() {
    let w = World::new(&["alice"]);
    let req = ProfileRequest {
        subject: w.kh("alice"),
        subnet: SUBNET,
        kind: KIND_OWN,
        nonce: [7; 16],
    };
    assert_eq!(ProfileRequest::decode(&req.encode()).unwrap(), req);
    let profile = Profile::build(w.id("alice"), &SUBNET, KIND_OWN, "Alice", None, 1);
    let yes = ProfileReply {
        nonce: [7; 16],
        profile: Some(profile),
        code: None,
    };
    assert_eq!(ProfileReply::decode(&yes.encode()).unwrap(), yes);
    let no = ProfileReply {
        nonce: [7; 16],
        profile: None,
        code: Some(FAIL_UNKNOWN),
    };
    assert_eq!(ProfileReply::decode(&no.encode()).unwrap(), no);
    // one or the other
    let neither = ProfileReply {
        nonce: [7; 16],
        profile: None,
        code: None,
    };
    assert!(ProfileReply::decode(&neither.encode()).is_err());
}
