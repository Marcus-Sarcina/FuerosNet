//! Where a client sits, and what that lets it do.
//!
//! A subordinate's locator is its patron's path with a nibble added, so a
//! patron must know its own. **Holding none was never the right state**:
//! `wire-format.md` §2.1 names the empty path the self-anchor case and
//! design Appendix A has a root name itself, so a party with no ancestor
//! has a position like any other.

mod common;

use common::harness::{Setup, setup};
use common::kh;
use rhtn_client::ceremony::Adopting;
use rhtn_client::device::ChannelKind;

const PARTIES: [&str; 6] = ["alice", "bob", "carol", "w1", "w2", "w3"];

fn met(s: &mut Setup, a: &str, b: &str) {
    s.face_off(a, b);
}

// acceptance: CER-33
#[test]
fn a_client_self_anchors_until_it_is_adopted_and_a_newly_minted_root_can_adopt() {
    let mut s = setup(&PARTIES, &[ChannelKind::Nfc]);

    // **nothing has been configured and no ceremony has been run.** A
    // client's position under its own key is the self-anchor: the empty
    // path, because there is no ancestor to name.
    let root = s.client("alice").position_in(&kh("alice")).expect("a party with no ancestor names itself");
    assert_eq!(root.anchor, kh("alice"));
    assert_eq!((root.path.as_slice(), root.nibbles), (&[][..], 0), "the path from itself to itself has no hops");
    assert_eq!(s.client("alice").anchors(), vec![kh("alice")], "one subnet, its own");
    assert_eq!(s.client("alice").position_in(&kh("bob")), None, "and it sits in nobody else's");

    // so adopting as a newly minted root is ordinary: no genesis, no
    // configuration, nothing to be granted
    met(&mut s, "bob", "alice");
    s.h.run_adoption(kh("bob"), kh("alice"), vec![kh("w1")], vec![kh("w2")], 1).expect("a root adopts");

    // and the subordinate sits one hop below it, in the root's subnet
    let p = s.client("bob").position_in(&kh("alice")).expect("bob knows where it was put");
    assert_eq!(p.anchor, kh("alice"), "the root's subnet");
    assert_eq!(p.nibbles, 1, "one hop below the root");
    assert_eq!(s.client("bob").anchor(), kh("alice"), "and that is the subnet bob acts in now");
    assert!(s.client("bob").anchors().contains(&kh("alice")));
}

// acceptance: CER-34
#[test]
fn a_client_adopted_into_two_trees_holds_a_position_in_each() {
    let mut s = setup(&PARTIES, &[ChannelKind::Nfc]);

    // two roots, and bob adopted under both: `wire-format.md` §2.3 has one
    // series per patron relationship, so this is two lines and not a
    // replacement of one by the other
    met(&mut s, "bob", "alice");
    s.h.run_adoption(kh("bob"), kh("alice"), vec![kh("w1")], vec![kh("w2")], 1).expect("adopted under alice");
    met(&mut s, "bob", "carol");
    s.h.run_adoption(kh("bob"), kh("carol"), vec![kh("w1")], vec![kh("w3")], 2).expect("adopted under carol");

    let under_alice = s.client("bob").position_in(&kh("alice")).expect("a position in alice's subnet");
    let under_carol = s.client("bob").position_in(&kh("carol")).expect("and one in carol's");
    assert_eq!(under_alice.anchor, kh("alice"));
    assert_eq!(under_carol.anchor, kh("carol"));
    assert_eq!(under_alice.seqno.series, 1, "each line carries its own series");
    assert_eq!(under_carol.seqno.series, 2);
    let anchors = s.client("bob").anchors();
    for a in [kh("bob"), kh("alice"), kh("carol")] {
        assert!(anchors.contains(&a), "bob is in its own subnet and both patrons': {anchors:?}");
    }

    // **and which tree an adoption goes into is the patron's to say.**
    // Adopting under one anchor puts the subordinate in that subnet; the
    // other subnet cannot read an address issued in the first.
    met(&mut s, "w1", "bob");
    let pop = s.h.run(kh("w1"), kh("bob"), vec![kh("w2")], vec![kh("w3")]).expect("w1 meets bob");
    let back = s.client("bob").back_pointers();
    let what = || Adopting { evidence: rhtn_archive::tx::Evidence::Presence(pop), series: 1, presented_head: None, key_material: None };
    let in_alice = s.client("bob").propose_adoption_in(kh("alice"), kh("w1"), &back, what()).expect("into alice's subnet");
    let in_carol = s.client("bob").propose_adoption_in(kh("carol"), kh("w1"), &back, what()).expect("into carol's subnet");
    assert_ne!(in_alice, in_carol, "two subnets, two addresses");
    // the locator an adoption body carries is its field 3
    // (`wire-format.md` §4.1)
    let anchor_of = |body: &[u8]| {
        let r = rhtn_codec::cbor::value_slice(body, 3).expect("a locator");
        rhtn_archive::tx::Locator::decode(&body[r]).expect("a locator").anchor
    };
    assert_eq!(anchor_of(&in_alice), kh("alice"));
    assert_eq!(anchor_of(&in_carol), kh("carol"));

    // a subnet it is in neither of has no position and no address to issue
    let e = s.client("bob").propose_adoption_in(kh("w3"), kh("w1"), &back, what()).expect_err("refused");
    assert!(format!("{e:?}").contains("no position"), "{e:?}");
}
