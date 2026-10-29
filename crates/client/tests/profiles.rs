//! The profiles this client publishes about itself: when, and at what
//! counter.

mod common;

use common::harness::setup;
use common::kh;
use rhtn_archive::profile::Profile;
use rhtn_client::ceremony::Msg;

const PARTIES: [&str; 4] = ["alice", "bob", "w1", "w2"];

/// The counters of every own profile a batch of messages carries.
fn published_at(msgs: &[Msg]) -> Vec<u64> {
    msgs.iter()
        .filter_map(|m| match m {
            Msg::PublishProfile(b) => Profile::parse(b).ok().map(|p| p.published_at),
            _ => None,
        })
        .collect()
}

/// A client with a position and a default name publishes that name to
/// the subnet it acts in at once, and **re-signs it at every attach at a
/// higher counter** (`light-client-requirements.md` §4.4) — the held bytes
/// resent unchanged are what a node refuses as not later
/// (`infra-client-requirements.md` §6.2), and a refusal fails the attach.
// acceptance: PRD-20
#[test]
fn a_held_profile_is_re_signed_at_a_higher_counter_at_each_attach() {
    let mut s = setup(&PARTIES);
    // nothing to publish to before a position: the name is kept and goes
    // out when there is a subnet to go to
    let none = s
        .client("alice")
        .set_default_alias("Alice")
        .expect("a name is kept");
    assert!(
        published_at(&none).is_empty(),
        "no subnet yet, so nowhere to publish"
    );

    s.adopted("bob", "alice", "w1", 1);
    let subnet = s.client("alice").subnets()[0];

    // the first attach publishes it, this client acting in a subnet now
    let first = published_at(&s.client("alice").attach(kh("w1"), &[]));
    assert_eq!(
        first.len(),
        1,
        "one profile of its own, for its one subnet: {first:?}"
    );
    assert_eq!(
        s.client("alice").own_alias(&subnet).as_deref(),
        Some("Alice"),
        "and holds it as its own profile there"
    );

    // each later attach carries it again, signed later than the last
    let second = published_at(&s.client("alice").attach(kh("w1"), &[]));
    let third = published_at(&s.client("alice").attach(kh("w1"), &[]));
    assert_eq!((second.len(), third.len()), (1, 1));
    assert!(
        first[0] < second[0] && second[0] < third[0],
        "each attach signs at a higher counter: {first:?} {second:?} {third:?}"
    );

    // the party adopted asks for the subnet's own name from its root,
    // the anchor of its position there, holding no anchoring record
    let asked: Vec<rhtn_archive::profile::ProfileRequest> = s
        .client("bob")
        .profile_requests()
        .iter()
        .filter_map(|m| match m {
            Msg::ProfileRequest(b) => rhtn_archive::profile::ProfileRequest::decode(b).ok(),
            _ => None,
        })
        .collect();
    assert!(
        asked
            .iter()
            .any(|r| r.kind == rhtn_archive::profile::KIND_SUBNET
                && r.subnet == subnet
                && r.subject == kh("alice")),
        "bob asks alice, the root, for the subnet's profile: {asked:?}"
    );

    // a name set for the subnet itself is not overwritten by the default
    s.client("alice")
        .publish_profile(subnet, 0, "Alice at home", None)
        .expect("published");
    s.client("alice").set_default_alias("A.").expect("kept");
    assert_eq!(
        s.client("alice").own_alias(&subnet).as_deref(),
        Some("Alice at home"),
        "an override stays an override"
    );
}
