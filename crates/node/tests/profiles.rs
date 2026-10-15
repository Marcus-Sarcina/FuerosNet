//! The profiles a node holds and serves (`wire-format.md` §7.11;
//! `infra-client-requirements.md` §6.2): taken only from the subject
//! named, replaced only by a later one, served to anyone, and kept across
//! a restart.

mod common;

use common::*;
use rhtn_archive::profile::*;
use rhtn_archive::submission::{
    ProfilePublication, SUBMISSION_ACCEPTED, SUBMISSION_REFUSED, SubmissionReply,
};
use rhtn_archive::topology::Table;

const SUBNET: [u8; 32] = [0xdb; 32];

fn publish(n: &mut rhtn_node::view::NodeView, peer: &str, profile: Vec<u8>, nonce: u8) -> u64 {
    let body = ProfilePublication {
        profile,
        nonce: [nonce; 16],
    }
    .encode();
    let reply = rhtn_node::submissions::profile(n, &ids(), &kh(peer), &body).expect("a reply");
    let r = SubmissionReply::decode(&reply).unwrap();
    assert_eq!(r.nonce, [nonce; 16], "the nonce is echoed");
    r.code
}

// acceptance: SES-32
#[test]
fn a_profile_is_taken_from_its_subject_alone_and_replaced_only_by_a_later_one() {
    let mut n = view("bob", Table::with_me(kh("bob")), "alice", &[]);
    let carols =
        |alias: &str, at: u64| Profile::build(&id("carol"), &SUBNET, KIND_OWN, alias, None, at);
    // carol's own is taken
    assert_eq!(
        publish(&mut n, "carol", carols("Carol", 10), 1),
        SUBMISSION_ACCEPTED
    );
    assert!(n.profiles.get(&kh("carol"), &SUBNET, KIND_OWN).is_some());
    // one naming another party is refused, however well signed
    let alices = Profile::build(&id("alice"), &SUBNET, KIND_OWN, "Alice", None, 10);
    assert_eq!(publish(&mut n, "carol", alices, 2), SUBMISSION_REFUSED);
    assert!(n.profiles.get(&kh("alice"), &SUBNET, KIND_OWN).is_none());
    // an earlier one does not replace what is held, nor an equal one
    assert_eq!(
        publish(&mut n, "carol", carols("Earlier", 9), 3),
        SUBMISSION_REFUSED
    );
    assert_eq!(
        publish(&mut n, "carol", carols("Same", 10), 4),
        SUBMISSION_REFUSED
    );
    assert_eq!(
        Profile::parse(n.profiles.get(&kh("carol"), &SUBNET, KIND_OWN).unwrap())
            .unwrap()
            .alias,
        "Carol"
    );
    // a later one does
    assert_eq!(
        publish(&mut n, "carol", carols("Caroline", 11), 5),
        SUBMISSION_ACCEPTED
    );
    assert_eq!(
        Profile::parse(n.profiles.get(&kh("carol"), &SUBNET, KIND_OWN).unwrap())
            .unwrap()
            .alias,
        "Caroline"
    );
    // the subnet's profile is taken from whoever signed it: that the
    // signer is the root is a convention the node does not check
    let league = Profile::build(
        &id("carol"),
        &SUBNET,
        KIND_SUBNET,
        "Bowling League",
        None,
        1,
    );
    assert_eq!(publish(&mut n, "carol", league, 6), SUBMISSION_ACCEPTED);
    assert_eq!(n.profiles.len(), 2, "one per (subject, subnet, kind)");
}

// acceptance: SES-33
#[test]
fn the_latest_profile_is_served_to_anyone_and_nothing_is_said_of_what_is_not_held() {
    let mut n = view("bob", Table::with_me(kh("bob")), "alice", &[]);
    let p = Profile::build(
        &id("carol"),
        &SUBNET,
        KIND_OWN,
        "Carol",
        Some((IMAGE_PNG, b"img")),
        5,
    );
    assert_eq!(publish(&mut n, "carol", p.clone(), 1), SUBMISSION_ACCEPTED);
    let ask = |subject: &str, kind: u64| {
        ProfileRequest {
            subject: kh(subject),
            subnet: SUBNET,
            kind,
            nonce: [3; 16],
        }
        .encode()
    };
    let r = ProfileReply::decode(&n.profiles.answer(&ask("carol", KIND_OWN)).unwrap()).unwrap();
    assert_eq!(r.nonce, [3; 16]);
    assert_eq!(
        r.profile.as_deref(),
        Some(p.as_slice()),
        "the profile, byte for byte"
    );
    let r = ProfileReply::decode(&n.profiles.answer(&ask("carol", KIND_SUBNET)).unwrap()).unwrap();
    assert_eq!((r.profile, r.code), (None, Some(FAIL_UNKNOWN)));
    let r = ProfileReply::decode(&n.profiles.answer(&ask("w1", KIND_OWN)).unwrap()).unwrap();
    assert_eq!((r.profile, r.code), (None, Some(FAIL_UNKNOWN)));
    assert!(
        n.profiles.answer(b"\x00").is_none(),
        "not a request: no reply"
    );
}

// acceptance: SES-32
#[test]
fn profiles_are_kept_where_the_store_is_and_come_back_after_a_restart() {
    let dir = std::env::temp_dir().join(format!("rhtn-profiles-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let mut s = rhtn_node::profiles::ProfileStore::at(&dir).unwrap();
    let p = Profile::build(&id("carol"), &SUBNET, KIND_OWN, "Carol", None, 5);
    s.publish(&ids(), &p).expect("taken");
    let again = rhtn_node::profiles::ProfileStore::at(&dir).unwrap();
    assert_eq!(again.get(&kh("carol"), &SUBNET, KIND_OWN), Some(&p));
    // and a later one written through replaces the file
    let later = Profile::build(&id("carol"), &SUBNET, KIND_OWN, "Caroline", None, 6);
    s.publish(&ids(), &later).expect("taken");
    let third = rhtn_node::profiles::ProfileStore::at(&dir).unwrap();
    assert_eq!(third.get(&kh("carol"), &SUBNET, KIND_OWN), Some(&later));
    assert_eq!(third.len(), 1);
    let _ = std::fs::remove_dir_all(&dir);
}
