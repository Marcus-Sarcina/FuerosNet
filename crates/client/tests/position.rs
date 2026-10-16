//! Where a client sits, and what that lets it do.
//!
//! A subordinate's locator is its patron's path with a nibble added, so a
//! patron must know its own. **Holding none was never the right state**:
//! `wire-format.md` §2.1 names the empty path the self-anchor case and
//! design Appendix A has a root name itself, so a party with no ancestor
//! has a position like any other.

mod common;

use common::harness::setup;
use common::{id, ids, kh};
use rhtn_client::ceremony::Adopting;
use rhtn_client::device::ChannelKind;

const PARTIES: [&str; 6] = ["alice", "bob", "carol", "w1", "w2", "w3"];

/// The subnet under `root`'s own key: its scope id there, the txid of its
/// first adoption as a lone root (design §13.8).
fn subnet_under(s: &mut common::harness::Setup, root: &str) -> rhtn_archive::Txid {
    s.client(root).subnets()[0]
}

// acceptance: CER-33
// acceptance: TOP-49
#[test]
fn a_client_self_anchors_until_it_is_adopted_and_a_newly_minted_root_can_adopt() {
    let mut s = setup(&PARTIES, &[ChannelKind::Nfc]);

    // **nothing has been configured and no ceremony has been run.** A
    // client's position under its own key is the self-anchor: the empty
    // path, because there is no ancestor to name.
    let root = s.client("alice").own_position();
    assert_eq!(root.anchor, kh("alice"));
    assert_eq!(
        (root.path.as_slice(), root.nibbles),
        (&[][..], 0),
        "the path from itself to itself has no hops"
    );
    // **and a lone identity is in no subnet** (design §13.8): the one
    // under its key is named by its first adoption's txid, and none has
    // happened
    assert_eq!(root.subnet, None);
    assert!(
        s.client("alice").subnets().is_empty(),
        "no subnet until it adopts"
    );
    assert_eq!(
        s.client("alice").position_in(&[7; 32]),
        None,
        "and it sits in nobody else's"
    );

    // so adopting as a newly minted root is ordinary: no genesis, no
    // configuration, nothing to be granted
    let first = s.adopted("bob", "alice", "w1", 1);

    // **the adoption named the subnet by its own txid**, and the root
    // holds the name from then on
    let subnet = subnet_under(&mut s, "alice");
    assert_eq!(subnet, first, "the root's scope id is its first adoption");
    assert_eq!(s.client("alice").own_position().subnet, Some(subnet));
    assert_eq!(
        s.client("alice").subnets(),
        vec![subnet],
        "one subnet, its own"
    );

    // and the subordinate sits one hop below it, in the root's subnet
    let p = s
        .client("bob")
        .position_in(&subnet)
        .expect("bob knows where it was put");
    assert_eq!(p.anchor, kh("alice"), "the root's subnet");
    assert_eq!(p.subnet, Some(subnet), "and it says so");
    assert_eq!(p.nibbles, 1, "one hop below the root");
    assert_eq!(
        s.client("bob").subnet(),
        Some(subnet),
        "and that is the subnet bob acts in now"
    );
    assert!(s.client("bob").subnets().contains(&subnet));
}

// acceptance: CER-34
// acceptance: TOP-49
// acceptance: TOP-50
#[test]
fn a_client_adopted_into_two_trees_holds_a_position_in_each() {
    let mut s = setup(&PARTIES, &[ChannelKind::Nfc]);

    // two roots, and bob adopted under both: `wire-format.md` §2.3 has one
    // series per patron relationship, so this is two lines and not a
    // replacement of one by the other
    s.adopted("bob", "alice", "w1", 1);
    s.adopted("bob", "carol", "w1", 2);
    let alices = subnet_under(&mut s, "alice");
    let carols = subnet_under(&mut s, "carol");
    assert_ne!(alices, carols, "two roots, two subnets");

    let under_alice = s
        .client("bob")
        .position_in(&alices)
        .expect("a position in alice's subnet");
    let under_carol = s
        .client("bob")
        .position_in(&carols)
        .expect("and one in carol's");
    assert_eq!(under_alice.anchor, kh("alice"));
    assert_eq!(under_carol.anchor, kh("carol"));
    assert_eq!(
        under_alice.seqno.series, 1,
        "each line carries its own series"
    );
    assert_eq!(under_carol.seqno.series, 2);
    let mut subnets = s.client("bob").subnets();
    subnets.sort();
    let mut both = vec![alices, carols];
    both.sort();
    assert_eq!(
        subnets, both,
        "bob is in both patrons' subnets, and has adopted nobody into its own"
    );

    // **and which tree an adoption goes into is the patron's to say.**
    // Adopting under one anchor puts the subordinate in that subnet; the
    // other subnet cannot read an address issued in the first.
    let pop = s.met("w1", "bob", "w2");
    let back = s.client("bob").back_pointers();
    let what = || Adopting {
        evidence: rhtn_archive::tx::Evidence::Presence(pop),
        series: 1,
        presented_head: None,
        key_material: None,
        carrying: None,
    };
    let in_alice = s
        .client("bob")
        .propose_adoption_in(Some(alices), kh("w1"), &back, what())
        .expect("into alice's subnet");
    let in_carol = s
        .client("bob")
        .propose_adoption_in(Some(carols), kh("w1"), &back, what())
        .expect("into carol's subnet");
    assert_ne!(in_alice, in_carol, "two subnets, two addresses");
    // the locator an adoption body carries is its field 3
    // (`wire-format.md` §4.1), and it names the subnet it is in (§2.3)
    let locator_of = |body: &[u8]| {
        let r = rhtn_codec::cbor::value_slice(body, 3).expect("a locator");
        rhtn_archive::tx::Locator::decode(&body[r]).expect("a locator")
    };
    assert_eq!(locator_of(&in_alice).anchor, kh("alice"));
    assert_eq!(locator_of(&in_alice).subnet, Some(alices));
    assert_eq!(locator_of(&in_carol).anchor, kh("carol"));
    assert_eq!(locator_of(&in_carol).subnet, Some(carols));

    // and as a lone root under its own key, which no adoption has named
    // yet, the body names no subnet: its own txid will (design §13.8)
    let as_root = s
        .client("bob")
        .propose_adoption_in(None, kh("w1"), &back, what())
        .expect("under its own key");
    assert_eq!(locator_of(&as_root).anchor, kh("bob"));
    assert_eq!(locator_of(&as_root).subnet, None);

    // **bob's scope in each subnet is latent**: named by its first
    // adoption there, carried by no locator, and nothing yet
    assert_eq!(s.client("bob").scope_in(&alices), None);

    // a subnet it is in none of has no position and no address to issue
    let e = s
        .client("bob")
        .propose_adoption_in(Some([7; 32]), kh("w1"), &back, what())
        .expect_err("refused");
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
        s.adopted(n, "alice", "w2", 1);
        let subnet = subnet_under(&mut s, "alice");
        let loc = s.client(n).position_in(&subnet).expect("where it was put");
        assert_eq!(loc.anchor, me, "in the patron's own subnet");
        assert_eq!(loc.nibbles, 1, "one hop below a root");
        assert!(
            issued.insert(loc.path[0] >> 4),
            "index {} was issued twice",
            loc.path[0] >> 4
        );
    }
    assert_eq!(
        issued.len(),
        3,
        "three subordinates, three indices: {issued:?}"
    );
    assert!(
        issued.iter().all(|i| *i <= 9),
        "and every one a nibble the path may carry"
    );
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
        common::harness::device(
            vec![],
            std::rc::Rc::new(std::cell::Cell::new(1_790_000_000_000u64)),
            11,
            0,
        )
        .0,
    );
    // a subordinate's first transaction points at the genesis value its
    // own key derives (`wire-format.md` §3.1)
    let back = |n: &str| vec![rhtn_archive::genesis(&kh(n))];
    let what = || Adopting {
        evidence: rhtn_archive::tx::Evidence::Presence([9; 32]),
        series: 1,
        presented_head: None,
        key_material: None,
        carrying: None,
    };

    // ten adoptions this client signed as patron, each one landing in its
    // archive, which is where it reads its own issuance from
    let subordinates = [
        "bob", "carol", "w1", "w2", "w3", "w4", "c1", "c2", "c3", "c4",
    ];
    let mut issued = std::collections::BTreeSet::new();
    for n in subordinates {
        let body = c
            .propose_adoption_in(None, kh(n), &back(n), what())
            .unwrap_or_else(|e| panic!("{n}: {e:?}"));
        let rec = w.commit(rhtn_archive::tx::TYPE_ADOPTION, &body, &[n, "alice"]);
        let loc = rec.locator().expect("a locator");
        assert!(
            issued.insert(loc.path[0] >> 4),
            "index {} was issued twice",
            loc.path[0] >> 4
        );
        c.take_adoption(&rec.bytes)
            .unwrap_or_else(|e| panic!("{n}: {e:?}"));
    }
    assert_eq!(
        issued,
        (0..10u8).collect(),
        "ten subordinates take the ten indices a nibble has"
    );

    // **the eleventh is refused, and refused by the party that can tell.**
    // A holder elsewhere may not have the other ten and could not check
    // this; the patron always can, which is design §1.1's test answered in
    // the patron's favour.
    let e = c
        .propose_adoption_in(None, kh("c5"), &back("c5"), what())
        .expect_err("no slot left");
    assert!(format!("{e:?}").contains("ten subordinate slots"), "{e:?}");
}

/// Exit is a right, not a request (`wire-format.md` §4.2, design §6.2):
/// one signature, nobody to ask, nothing that can come back refusing.
// acceptance: CER-42
// acceptance: TOP-50
#[test]
fn a_client_leaves_a_patron_on_its_own_signature_and_holds_no_position_there_after() {
    use rhtn_archive::record::Record;
    use rhtn_archive::tx::TYPE_DEPARTURE;
    use rhtn_client::ceremony::{Abort, Msg};

    let mut s = setup(&PARTIES, &[ChannelKind::Nfc]);
    s.adopted("bob", "alice", "w1", 1);
    s.adopted("bob", "carol", "w1", 2);
    let alices = subnet_under(&mut s, "alice");
    let carols = subnet_under(&mut s, "carol");
    let under_alice = s
        .client("bob")
        .position_in(&alices)
        .expect("a position in alice's subnet");
    let _ = s.client("bob").outbox();

    let txid = s
        .client("bob")
        .depart(kh("alice"), None)
        .expect("nobody to ask");

    // the envelope carries one signature, is the departing node's, and
    // advances the counter within the series it names
    let posted = s.client("bob").outbox();
    let [Msg::Record(bytes)] = posted.as_slice() else {
        panic!("one record posted, got {posted:?}")
    };
    let rec = Record::parse(bytes).expect("parses");
    assert_eq!(rec.txid, txid);
    assert_eq!(rec.tx_type, TYPE_DEPARTURE);
    assert_eq!(rec.field_hash(1), Some(kh("bob")), "the departing node");
    assert_eq!(
        rec.field_hash(2),
        Some(kh("alice")),
        "the patron being left"
    );
    let seq = rec.seqno().expect("a sequence");
    assert_eq!(
        seq.series, under_alice.seqno.series,
        "within the current series"
    );
    assert_eq!(
        seq.counter,
        under_alice.seqno.counter + 1,
        "at the next counter"
    );

    // and bob is no longer at the address it left, while the other line
    // is untouched: one series per patron relationship
    // (and this pair's txids are the adversarial order for the fold:
    // the departure sorts before the adoption it ends)
    assert_eq!(
        s.client("bob").position_in(&alices),
        None,
        "no position in the subnet it left"
    );
    assert!(
        s.client("bob").position_in(&carols).is_some(),
        "and carol's is not disturbed"
    );
    assert_eq!(
        s.client("bob").subnet(),
        Some(carols),
        "the subnet it acts in is the one it is still in"
    );
    assert!(
        s.client("bob").scope_in(&alices).is_none(),
        "bob had no subordinate under alice, so nothing re-roots"
    );

    // leaving a patron it has no relationship with is refused, and
    // leaving the same one twice is the same refusal
    assert!(
        matches!(
            s.client("bob").depart(kh("w3"), None),
            Err(Abort::NoRelationship(_))
        ),
        "no relationship to leave"
    );
    assert!(
        matches!(
            s.client("bob").depart(kh("alice"), None),
            Err(Abort::NoRelationship(_))
        ),
        "already left"
    );
}

/// **A caller that names no nominee is handed the parties this client has
/// met** [ruled, author, 2026-10-10: witnesses are required where
/// available]. On the bench two phones that had each met somebody began
/// with `nominees: 0` and the body waited on a witness never asked.
#[test]
fn a_client_that_is_handed_no_nominee_nominates_the_parties_it_has_met() {
    let mut s = setup(&["alice", "bob", "carol"], &[ChannelKind::Nfc]);
    assert!(
        s.client("alice").met(None).is_empty(),
        "nobody met, nobody to nominate"
    );
    s.met("alice", "bob", "carol");
    s.met("alice", "carol", "bob");
    let pool = s.client("alice").met(None);
    assert!(
        pool.contains(&kh("bob")) && pool.contains(&kh("carol")),
        "{pool:?}"
    );
    // the counterparty of this meeting is never its own witness
    assert_eq!(vec![kh("carol")], s.client("alice").met(Some(&kh("bob"))));
    // and a ceremony begun with nobody named nominates from them
    s.client("alice")
        .begin(Some(kh("bob")), vec![], true)
        .expect("begun");
    let (mine, _) = s.client("alice").nominees();
    assert_eq!(
        vec![kh("carol")],
        mine,
        "bob is the counterparty, carol the witness"
    );
    s.client("alice").abandon();
    // a caller that does name nominees is not overridden
    s.client("alice")
        .begin(Some(kh("bob")), vec![kh("carol"), kh("bob")], true)
        .expect("begun");
    assert_eq!(
        2,
        s.client("alice").nominees().0.len(),
        "the caller's list stands as given"
    );
}

/// **Whom a client has met is folded at every start.** The acquaintance
/// is derived from the records and was folded only at a finalize, so a
/// restarted client had met nobody until its next ceremony ended: it
/// nominated nobody and recognised none of a bundle it had every right
/// to [author, 2026-10-11: behaviour must not depend on what this session
/// did first]. Both restorers fold it.
#[test]
fn a_restarted_client_has_still_met_whom_it_met() {
    let mut s = setup(&["alice", "bob"], &[ChannelKind::Nfc]);
    s.met("alice", "bob", "carol");
    assert_eq!(s.client("alice").met(None), vec![kh("bob")]);
    let blob = s.client("alice").durable();
    let clock = s.clock.clone();

    // from the durable document
    let (dev, _h) = common::harness::device(vec![ChannelKind::Nfc], clock.clone(), 17, 0);
    let mut again = rhtn_client::ceremony::Client::new(id("alice"), ids(), Default::default(), dev);
    again.restore_durable(&blob).expect("opens whole");
    assert_eq!(again.met(None), vec![kh("bob")], "restored: still met bob");

    // and from a directory
    let dir = std::env::temp_dir().join(format!("rhtn-met-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    s.client("alice").save(&dir).expect("saved");
    let (dev, _h) = common::harness::device(vec![ChannelKind::Nfc], clock, 18, 0);
    let at = rhtn_client::ceremony::Client::at(&dir, id("alice"), ids(), Default::default(), dev)
        .expect("opens from the directory");
    assert_eq!(at.met(None), vec![kh("bob")], "loaded: still met bob");
    let _ = std::fs::remove_dir_all(&dir);
}

/// **A client's own code, for somebody to scan** (`wire-format.md` §2.4):
/// its introduction, and the anchor of its own locator with the record of
/// where that anchor answers. Refused where it holds no such record,
/// because a card that could not be acted on is not worth showing
/// [author, 2026-10-13: "one code should carry the introduction and its
/// anchor records"].
#[test]
fn a_client_makes_a_card_of_its_own_once_it_knows_where_its_anchor_answers() {
    let mut s = setup(&PARTIES, &[ChannelKind::Nfc]);
    let patron = kh("alice");
    s.adopted("bob", "alice", "w2", 1);

    // bob sits under alice and can say so, but has no address for her yet
    assert!(s.client("bob").own_locator().is_ok());
    assert!(
        s.client("bob").own_card().is_err(),
        "no record of where the anchor answers: nothing to show"
    );

    // alice publishes where she answers; bob takes it
    let mut point = Vec::new();
    rhtn_transport::session::NetworkPoint::new([127, 0, 0, 1], Some(7447)).encode(&mut point);
    let record = rhtn_archive::endpoint::endpoint_record(
        &id("alice"),
        &[point],
        rhtn_archive::tx::Seqno {
            series: 1,
            counter: 1,
        },
    );
    s.client("bob").take_endpoint(&record).expect("alice's own");
    let card = s.client("bob").own_card().expect("a card to show");

    // and carol, who has met nobody, can act on that one code
    let who = s
        .client("carol")
        .take_anchored_introduction(&card)
        .expect("one code");
    assert_eq!(who, kh("bob"));
    let reach = s.client("carol").reach_for(&kh("bob"));
    assert_eq!(
        reach.ask.first().map(|a| a.node),
        Some(patron),
        "carol starts the walk at bob's anchor, which the card named"
    );
}

/// **A scope brought under a patron dissolves into the subnet joined, and
/// stands alone again on departure** (design §13.8; `wire-format.md` §4.1
/// field 10); a body naming a scope this key does not root is refused.
// acceptance: TOP-51
#[test]
fn a_scope_brought_under_a_patron_dissolves_and_returns_on_departure() {
    use rhtn_client::ceremony::Abort;
    let mut s = setup(&PARTIES, &[ChannelKind::Nfc]);
    // alice roots a scope: her first adoption, carol under her
    let scope = s.adopted("carol", "alice", "w1", 1);
    assert_eq!(s.client("alice").subnets(), vec![scope]);
    assert_eq!(s.client("alice").scopes_rooted(), vec![scope]);
    // bob, a lone root, adopts alice, who brings her scope
    let pop = s.met_in_person("alice", "bob", "w2");
    let node_back = s.client("alice").back_pointers();
    // series 2: alice's first line is the one she opened as carol's patron
    let bring = |carrying| Adopting {
        evidence: rhtn_archive::tx::Evidence::Presence(pop),
        series: 2,
        presented_head: None,
        key_material: None,
        carrying,
    };
    // a body naming a scope alice does not root is refused by alice
    let wrong = s
        .client("bob")
        .propose_adoption_in(None, kh("alice"), &node_back, bring(Some([7; 32])))
        .expect("the patron copies the claim");
    let e = s
        .client("alice")
        .review_and_sign_adoption(&wrong)
        .expect_err("not a scope she roots");
    assert!(
        matches!(e, Abort::NodeRefused(ref why) if why.contains("does not root")),
        "{e:?}"
    );
    // the right one is signed, and the record taken by both
    let body = s
        .client("bob")
        .propose_adoption_in(None, kh("alice"), &node_back, bring(Some(scope)))
        .expect("proposed");
    let entries = s
        .client("alice")
        .review_and_sign_adoption(&body)
        .expect("alice roots it");
    let theirs = s.client("bob").sign_body(&body).expect("bob signs");
    let env = rhtn_archive::tx::envelope_from_entries(
        rhtn_archive::tx::TYPE_ADOPTION,
        &body,
        &[(kh("alice"), entries), (kh("bob"), theirs)],
    );
    let t = s.client("bob").take_adoption(&env).expect("bob takes");
    assert_eq!(
        s.client("alice").take_adoption(&env).expect("alice takes"),
        t
    );
    // while adopted, alice's scope is no subnet of its own: one position,
    // in bob's subnet, which is named by this adoption
    let bobs = s.client("bob").subnets()[0];
    assert_eq!(bobs, t);
    assert_eq!(
        s.client("alice").subnets(),
        vec![bobs],
        "the scope dissolved into bob's"
    );
    assert!(
        s.client("alice").scopes_rooted().is_empty(),
        "nothing left to bring"
    );
    assert_eq!(s.client("alice").position_in(&scope), None);
    // alice departs bob: her scope stands alone again, by the same id
    s.client("alice")
        .depart(kh("bob"), None)
        .expect("nobody to ask");
    assert_eq!(s.client("alice").subnets(), vec![scope]);
    assert_eq!(
        s.client("alice")
            .position_in(&scope)
            .map(|l| (l.anchor, l.nibbles, l.subnet)),
        Some((kh("alice"), 0, Some(scope)))
    );
}
