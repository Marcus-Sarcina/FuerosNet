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
        2,
        "carol can now introduce bob and alice both"
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
