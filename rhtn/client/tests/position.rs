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

// acceptance: CER-40
#[test]
fn a_patron_issues_each_subordinate_its_own_index_and_stops_at_ten() {
    let mut s = setup(&PARTIES, &[ChannelKind::Nfc]);
    let me = kh("alice");

    // **ten slots and no eleventh.** design §3.1 gives every node at most
    // f = 10 subordinates and `wire-format.md` §2.1 gives a path nibble
    // the values 0 to 9: the same ten counted twice, because the index
    // *is* the slot. Two subordinates at one index would be two parties at
    // one address.
    let mut issued = std::collections::BTreeSet::new();
    for n in ["bob", "carol", "w1"] {
        met(&mut s, n, "alice");
        s.h.run_adoption(kh(n), me, vec![kh("w2")], vec![kh("w3")], 1).unwrap_or_else(|e| panic!("{n}: {e:?}"));
        let loc = s.client(n).position_in(&me).expect("where it was put");
        assert_eq!(loc.anchor, me, "in the patron's own subnet");
        assert_eq!(loc.nibbles, 1, "one hop below a root");
        assert!(issued.insert(loc.path[0] >> 4), "index {} was issued twice", loc.path[0] >> 4);
    }
    assert_eq!(issued.len(), 3, "three subordinates, three indices: {issued:?}");
    assert!(issued.iter().all(|i| *i <= 9), "and every one a nibble the path may carry");
}

// acceptance: CER-41
#[test]
fn an_eleventh_subordinate_has_no_slot_to_be_put_in() {
    // a patron with no ancestor, adopting from its own subnet
    let mut w = common::World::new();
    let mut c = rhtn_client::ceremony::Client::new(
        common::id("alice"),
        common::ids(),
        Default::default(),
        common::harness::device(vec![], std::rc::Rc::new(std::cell::Cell::new(1_790_000_000_000u64)), 11, 0).0,
    );
    let me = kh("alice");
    // a subordinate's first transaction points at the genesis value its
    // own key derives (`wire-format.md` §3.1)
    let back = |n: &str| vec![rhtn_archive::genesis(&kh(n))];
    let what = || Adopting { evidence: rhtn_archive::tx::Evidence::Presence([9; 32]), series: 1, presented_head: None, key_material: None };

    // ten adoptions this client signed as patron, each one landing in its
    // archive, which is where it reads its own issuance from
    let subordinates = ["bob", "carol", "w1", "w2", "w3", "w4", "c1", "c2", "c3", "c4"];
    let mut issued = std::collections::BTreeSet::new();
    for n in subordinates {
        let body = c.propose_adoption_in(me, kh(n), &back(n), what()).unwrap_or_else(|e| panic!("{n}: {e:?}"));
        let rec = w.commit(rhtn_archive::tx::TYPE_ADOPTION, &body, &[n, "alice"]);
        let loc = rec.locator().expect("a locator");
        assert!(issued.insert(loc.path[0] >> 4), "index {} was issued twice", loc.path[0] >> 4);
        c.take_adoption(&rec.bytes).unwrap_or_else(|e| panic!("{n}: {e:?}"));
    }
    assert_eq!(issued, (0..10u8).collect(), "ten subordinates take the ten indices a nibble has");

    // **the eleventh is refused, and refused by the party that can tell.**
    // A holder elsewhere may not have the other ten and could not check
    // this; the patron always can, which is design §1.1's test answered in
    // the patron's favour.
    let e = c.propose_adoption_in(me, kh("c5"), &back("c5"), what()).expect_err("no slot left");
    assert!(format!("{e:?}").contains("ten subordinate slots"), "{e:?}");
}
