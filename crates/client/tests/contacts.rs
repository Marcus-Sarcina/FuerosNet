//! Contacts (`light-client-requirements.md` §4.3): captured from every
//! party whose locator becomes visible, persisted with the material that
//! verifies them, and pruned toward what still resolves.

mod common;

use common::harness::setup;
use common::{id, ids, kh};
use rhtn_archive::locator::signed_locator;
use rhtn_archive::tx::{Locator, Seqno};
use rhtn_client::contacts::{Contact, How, prune, upstream_held};
use rhtn_client::device::ChannelKind;
use std::collections::BTreeMap;

// acceptance: TOP-040
#[test]
fn a_meeting_leaves_each_party_a_contact_of_the_other_with_a_locator() {
    let mut s = setup(&["alice", "bob"], &[ChannelKind::Nfc]);
    s.run("alice", "bob", &[], &[]).expect("alice meets bob");
    let c = s
        .client("alice")
        .store
        .contacts
        .get(&kh("bob"))
        .expect("bob is a contact")
        .clone();
    assert_eq!(c.how, How::Counterparty);
    assert_eq!(
        c.material,
        id("bob").public.key_material(),
        "the material pinned from the screen"
    );
    assert!(c.locator.is_some(), "and the locator bob's intent carried");
    assert!(
        c.count >= 2,
        "pinned, then the intent: dealt with more than once"
    );
    assert_eq!(
        s.client("alice").introductions().len(),
        1,
        "which is what alice can introduce"
    );
    let d = s
        .client("bob")
        .store
        .contacts
        .get(&kh("alice"))
        .expect("alice is bob's");
    assert!(d.locator.is_some());
}

// acceptance: TOP-040
#[test]
fn an_introduced_stranger_is_a_contact_with_material_to_verify_by() {
    let mut s = setup(&["alice", "bob", "carol", "w1"], &[ChannelKind::Nfc]);
    s.run("alice", "bob", &[], &[]).expect("alice meets bob");
    // carol has never met alice; bob's bundle introduces her. bob is past
    // his genesis, so this meeting needs a witness: w1, named by both
    s.run("bob", "carol", &["w1"], &["w1"])
        .expect("bob meets carol");
    assert_eq!(
        s.client("bob").store.contacts.get(&kh("w1")).map(|c| c.how),
        Some(How::Nominee),
        "a nominee that answered is a contact"
    );
    let c = s
        .client("carol")
        .store
        .contacts
        .get(&kh("alice"))
        .expect("alice introduced to carol")
        .clone();
    assert_eq!(c.how, How::Introduced);
    assert!(c.locator.is_some());
    assert!(
        s.client("carol")
            .known
            .iter()
            .any(|k| k.keyhash == kh("alice")),
        "and carol verifies alice by the material the introduction carried"
    );
    assert_eq!(
        s.client("carol").introductions().len(),
        1,
        "carol introduces bob, whom she met; alice, whom she was only told of, she cannot vouch for"
    );
}

// acceptance: TOP-040
#[test]
fn contacts_survive_a_restart_and_introductions_still_build() {
    let mut s = setup(&["alice", "bob"], &[ChannelKind::Nfc]);
    s.run("alice", "bob", &[], &[]).expect("alice meets bob");
    let blob = s.client("alice").durable();
    let (dev, _h) = common::harness::device(vec![ChannelKind::Nfc], s.clock.clone(), 23, 0);
    // started knowing nobody, as a phone is
    let mut again =
        rhtn_client::ceremony::Client::new(id("alice"), vec![], Default::default(), dev);
    again.restore_durable(&blob).expect("opens whole");
    assert!(
        again.known.iter().any(|k| k.keyhash == kh("bob")),
        "bob's material came back with the contact"
    );
    assert_eq!(
        again.introductions().len(),
        1,
        "and bob can be introduced again"
    );
    assert_eq!(again.met(None), vec![kh("bob")]);
}

/// A contact of `who` placed under `anchor` at `path` nibbles, dealt with
/// `count` times, last at `last_at`.
fn placed(
    who: &str,
    anchor: &str,
    path: &[u8],
    nibbles: u64,
    last_at: u64,
    count: u64,
) -> (rhtn_archive::Keyhash, Contact) {
    let loc = Locator {
        anchor: kh(anchor),
        path: path.to_vec(),
        nibbles,
        seqno: Seqno {
            series: 1,
            counter: 0,
        },
    };
    let mut c = Contact::new(id(who).public.key_material(), How::Introduced, last_at);
    c.locator = Some(signed_locator(&id(who), &loc));
    c.count = count;
    (kh(who), c)
}

// acceptance: TOP-041
#[test]
fn pruning_removes_what_an_upstream_still_discovers_first_and_the_lonely_last() {
    let mut contacts: BTreeMap<rhtn_archive::Keyhash, Contact> = BTreeMap::new();
    // alice is an anchor held here; w1 and w2 sit under her; w3 sits under
    // w2 (w2 being infrastructure on w3's path); carol is her own anchor
    // with nobody above her that this client holds
    for (k, c) in [
        placed("alice", "alice", &[], 0, 100, 5),
        placed("w1", "alice", &[0x10], 1, 10, 1),
        placed("w2", "alice", &[0x20], 1, 50, 3),
        placed("w3", "alice", &[0x21], 2, 20, 1),
        placed("carol", "carol", &[], 0, 5, 1),
    ] {
        contacts.insert(k, c);
    }
    assert!(
        upstream_held(&contacts, &kh("w1")),
        "alice, the anchor, is held"
    );
    assert!(upstream_held(&contacts, &kh("w3")), "w2 sits on w3's path");
    assert!(
        !upstream_held(&contacts, &kh("carol")),
        "nobody above carol is held"
    );
    assert!(
        !upstream_held(&contacts, &kh("alice")),
        "an anchor is not its own upstream"
    );

    // keep three: carol ranks lowest of all (last 5, once) but goes last,
    // since pruning her is losing her; w1 (10) then w3 (20) go first
    let mut p = contacts.clone();
    assert_eq!(prune(&mut p, 3), 2);
    assert!(
        p.contains_key(&kh("carol")) && p.contains_key(&kh("alice")) && p.contains_key(&kh("w2"))
    );
    // keep one: the discoverable all go, then the rest in rank order —
    // carol (5), then alice (100) stays as the most dealt with
    let mut q = contacts.clone();
    assert_eq!(prune(&mut q, 1), 4);
    assert!(q.contains_key(&kh("alice")));
    // under the bound nothing moves
    let mut r = contacts.clone();
    assert_eq!(prune(&mut r, 5), 0);
    assert_eq!(r, contacts);
}

// acceptance: TOP-040
#[test]
fn a_contact_round_trips_through_the_store_and_a_bare_locator_is_not_one() {
    let (k, c) = placed("bob", "alice", &[0x30], 1, 77, 4);
    assert_eq!(Contact::decode(&c.encode()).as_ref(), Some(&c));
    let bare = c.locator.clone().unwrap();
    assert!(
        Contact::decode(&bare).is_none(),
        "a bare SignedLocator is not a contact"
    );
    let mut st = rhtn_client::store::ClientStore::default();
    st.contacts.insert(k, c.clone());
    let back = rhtn_client::store::ClientStore::decode(&st.encode()).unwrap();
    assert_eq!(back.contacts.get(&k), Some(&c));
    let _ = ids();
}

/// **Taking an introduction is the one way a party arrives without a
/// meeting** (`wire-format.md` §2.3; design §12.3 Case 0, where the
/// locator travels with the key out of band and no lookup occurs). The
/// client review of 2026-10-11 found the kernel had no call for it at
/// all, so a `People` screen had nothing to call [2026-10-13].
#[test]
fn an_introduction_taken_leaves_an_identity_a_contact_and_a_position() {
    let mut s = setup(&["alice", "bob", "w1"], &[ChannelKind::Nfc]);
    // bob sits under w1, and says so
    let under = Locator {
        anchor: kh("w1"),
        path: vec![0x10],
        nibbles: 2,
        seqno: Seqno {
            series: 1,
            counter: 4,
        },
    };
    let intro = rhtn_archive::locator::introduction(
        &id("bob").public.key_material(),
        &signed_locator(&id("bob"), &under),
    );

    let a = s.client("alice");
    assert_eq!(a.take_introduction(&intro).expect("bob arrives"), kh("bob"));
    let c = a.store.contacts.get(&kh("bob")).expect("a contact for bob");
    assert_eq!(c.how, How::Introduced);
    assert!(c.locator.is_some(), "the locator came with the key");
    assert!(
        c.endpoint_record.is_none(),
        "a person answers nowhere; only a node has an address"
    );

    // and alice can now say where bob sits, which is what a resolution is
    // made of — she holds no horizon record of him
    let reach = a.reach_for(&kh("bob"));
    match reach.upstream {
        rhtn_client::horizon::Upstream::Position {
            anchor, nibbles, ..
        } => {
            assert_eq!((anchor, nibbles), (kh("w1"), 2));
        }
        other => panic!("a position and nothing more: {other:?}"),
    }
    assert!(
        reach.ask.is_empty(),
        "and nobody to ask it of yet: no address for the anchor is held"
    );

    // an introduction to oneself, and one whose bytes are not one, are
    // refused without changing anything
    let before = a.store.contacts.len();
    assert!(
        a.take_introduction(&rhtn_archive::locator::introduction(
            &id("alice").public.key_material(),
            &signed_locator(&id("alice"), &under),
        ))
        .is_err()
    );
    assert!(a.take_introduction(b"not an introduction").is_err());
    assert_eq!(a.store.contacts.len(), before);
}

/// **The anchor's address is looked up in the contacts** (design §12.3
/// Case 1 step 2). The horizon is not the place for it: it holds
/// endpoints for its own neighbourhood and refuses a record for anything
/// further off, so an anchor a stranger names is held here or nowhere
/// [2026-10-13].
#[test]
fn an_endpoint_record_gives_an_introduced_party_an_address_to_start_from() {
    let mut s = setup(&["alice", "bob", "w1"], &[ChannelKind::Nfc]);
    let under = Locator {
        anchor: kh("w1"),
        path: vec![0x10],
        nibbles: 2,
        seqno: Seqno {
            series: 1,
            counter: 4,
        },
    };
    let intro = rhtn_archive::locator::introduction(
        &id("bob").public.key_material(),
        &signed_locator(&id("bob"), &under),
    );
    // w1 publishes where it answers, and signs it: a `NetworkPoint` as
    // the record carries one, since the parser checks what it is
    let point = |port: u64| {
        let mut p = Vec::new();
        rhtn_transport::session::NetworkPoint::new([127, 0, 0, 1], Some(port)).encode(&mut p);
        p
    };
    let points = vec![point(5001)];
    let record = |counter: u32| {
        rhtn_archive::endpoint::endpoint_record(&id("w1"), &points, Seqno { series: 1, counter })
    };

    let a = s.client("alice");
    a.take_introduction(&intro).expect("bob arrives");
    assert_eq!(a.take_endpoint(&record(2)).expect("w1's own"), kh("w1"));
    let c = a.store.contacts.get(&kh("w1")).expect("a contact for w1");
    assert_eq!(c.how, How::Infra);
    let points_of = |c: &Contact| {
        rhtn_archive::endpoint::EndpointRecord::parse(c.endpoint_record.as_deref().unwrap())
            .unwrap()
            .endpoints
    };
    assert_eq!(points_of(c), points.clone());

    // the walk now starts where the design says it starts: at the anchor
    let reach = a.reach_for(&kh("bob"));
    assert_eq!(
        reach.ask.first().map(|x| (x.node, x.endpoints.clone())),
        Some((kh("w1"), points.clone())),
        "the anchor is the first asked"
    );

    // §10.1.2's ordering holds: a greater counter in the series replaces,
    // an older one does not
    let newer = vec![point(5002)];
    let moved = rhtn_archive::endpoint::endpoint_record(
        &id("w1"),
        &newer,
        Seqno {
            series: 1,
            counter: 5,
        },
    );
    a.take_endpoint(&moved).expect("taken");
    assert_eq!(points_of(&a.store.contacts[&kh("w1")]), newer);
    a.take_endpoint(&record(2)).expect("taken, and ignored");
    assert_eq!(
        points_of(&a.store.contacts[&kh("w1")]),
        newer,
        "an older record does not move the address"
    );

    // a record for a node whose key is not held cannot be checked, and is
    // not taken on trust
    let stranger = rhtn_archive::endpoint::endpoint_record(
        &rhtn_crypto::identity::testkit::test_identity("a party nobody here holds"),
        &points,
        Seqno {
            series: 1,
            counter: 1,
        },
    );
    assert!(a.take_endpoint(&stranger).is_err());
}

/// **One code carries the introduction and its anchor's records**
/// [author, 2026-10-13]. An introduction names an anchor by keyhash and a
/// path, and carries neither the anchor's key material nor any address,
/// so a holder with no node of its own could not act on one: §12.3 Case 1
/// needs "an address to start from" at Case 0's meeting. The anchored
/// introduction of `wire-format.md` §2.4 carries both, and a client can
/// make its own.
#[test]
fn a_card_carries_an_introduction_and_the_records_of_its_anchor() {
    let mut s = setup(&["alice", "bob", "w1"], &[ChannelKind::Nfc]);
    let point = {
        let mut p = Vec::new();
        rhtn_transport::session::NetworkPoint::new([127, 0, 0, 1], Some(5001)).encode(&mut p);
        p
    };
    let record = rhtn_archive::endpoint::endpoint_record(
        &id("w1"),
        std::slice::from_ref(&point),
        Seqno {
            series: 1,
            counter: 3,
        },
    );
    // bob sits under w1 and holds where w1 answers, so bob can make a card
    let under = Locator {
        anchor: kh("w1"),
        path: vec![0x10],
        nibbles: 2,
        seqno: Seqno {
            series: 1,
            counter: 1,
        },
    };
    let card = rhtn_archive::locator::anchored_introduction(
        &rhtn_archive::locator::introduction(
            &id("bob").public.key_material(),
            &signed_locator(&id("bob"), &under),
        ),
        &id("w1").public.key_material(),
        &record,
        None,
    );

    // alice takes the one code, and holds both halves of what it said
    let a = s.client("alice");
    assert_eq!(
        a.take_anchored_introduction(&card).expect("one code"),
        kh("bob")
    );
    assert!(a.store.contacts[&kh("bob")].locator.is_some());
    assert!(a.store.contacts[&kh("w1")].endpoint_record.is_some());

    // so the walk to bob starts at w1, which is the whole point of the code
    let reach = a.reach_for(&kh("bob"));
    assert!(matches!(
        reach.upstream,
        rhtn_client::horizon::Upstream::Position { anchor, .. } if anchor == kh("w1")
    ));
    assert_eq!(
        reach.ask.first().map(|x| (x.node, x.endpoints.clone())),
        Some((kh("w1"), vec![point])),
        "the anchor, at the address its own record gave"
    );

    // a card whose anchor material is not the anchor the locator names is
    // not this kind at all: the codec refuses it before any signature
    let crossed = rhtn_archive::locator::anchored_introduction(
        &rhtn_archive::locator::introduction(
            &id("bob").public.key_material(),
            &signed_locator(&id("bob"), &under),
        ),
        &id("alice").public.key_material(),
        &record,
        None,
    );
    assert!(a.take_anchored_introduction(&crossed).is_err());
}

/// **A node the walk went through becomes a contact** (§7.7's descent;
/// `light-client-requirements.md` §4.3 asks for "the infrastructure you
/// went through to reach any of them"). Keeping them is what makes the
/// pruning effective later [author, 2026-10-14]: §4.3 drops a contact
/// that stays discoverable from an upstream it holds, and these are those
/// upstreams.
#[test]
fn a_hop_of_the_walk_is_filed_with_the_address_it_was_heard_at() {
    let mut s = setup(&["alice", "bob", "w1"], &[ChannelKind::Nfc]);
    let point = |port: u64| {
        let mut p = Vec::new();
        rhtn_transport::session::NetworkPoint::new([127, 0, 0, 1], Some(port)).encode(&mut p);
        p
    };
    let a = s.client("alice");

    // a hop whose referral carried its material is filed under it
    a.note_hop(
        kh("w1"),
        Some(id("w1").public.key_material()),
        vec![point(7001)],
    );
    let c = a.store.contacts.get(&kh("w1")).expect("the hop is held");
    assert_eq!((c.how, c.heard.clone()), (How::Infra, vec![point(7001)]));
    assert!(
        c.endpoint_record.is_none(),
        "a referral's addresses are the referrer's word, not a publication"
    );

    // heard again elsewhere: both addresses, each once
    a.note_hop(
        kh("w1"),
        Some(id("w1").public.key_material()),
        vec![point(7001), point(7002)],
    );
    assert_eq!(
        a.store.contacts[&kh("w1")].heard,
        vec![point(7001), point(7002)]
    );

    // and a locator this client later holds for that node routes through
    // what it heard, where nothing signed is held
    let under = Locator {
        anchor: kh("w1"),
        path: vec![0x10],
        nibbles: 2,
        seqno: Seqno {
            series: 1,
            counter: 1,
        },
    };
    a.take_introduction(&rhtn_archive::locator::introduction(
        &id("bob").public.key_material(),
        &signed_locator(&id("bob"), &under),
    ))
    .expect("bob arrives");
    let reach = a.reach_for(&kh("bob"));
    assert_eq!(
        reach.ask.first().map(|x| (x.node, x.endpoints.len())),
        Some((kh("w1"), 2)),
        "the anchor, at the addresses the walk heard for it"
    );

    // **a hop whose referral carried no material is kept by keyhash
    // alone** [author, 2026-10-14]: §7.7.3 leaves that field optional and
    // the session does not supply it, since a dial presents the classical
    // key where the material is the classical and post-quantum pair the
    // keyhash is taken over (`wire-format.md` §2.2)
    let known_before = a.known.len();
    a.note_hop(kh("w2"), None, vec![point(7003)]);
    let c = a.store.contacts.get(&kh("w2")).expect("kept by keyhash");
    assert!(!c.named(), "a name and a place to try, nothing more");
    assert_eq!(c.heard, vec![point(7003)]);
    assert_eq!(
        a.known.len(),
        known_before,
        "and not something this client now verifies by"
    );
    // material that names another party is no better than none
    a.note_hop(kh("w3"), Some(id("w2").public.key_material()), vec![]);
    assert!(!a.store.contacts[&kh("w3")].named());
    // nor is such a contact ever introduced, even were a locator held
    assert!(
        !a.introductions()
            .iter()
            .any(|i| i.windows(4).any(|w| w == &kh("w2")[..4])),
        "a contact with no material is not handed on"
    );
}
