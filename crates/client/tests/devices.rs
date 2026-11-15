//! Two devices of one identity brought to one dataset (design §23.3, §10.2,
//! §10.3; `light-client-requirements.md` §4.2; `wire-format.md` §14.3.3;
//! functional row APP-014): the device that is behind is handed what the
//! other holds across the local interface alone, after both holders say
//! yes; a fork is kept as two heads the next record merges; the sealed
//! captures cross as ciphertext and open nowhere new; and no seed crosses,
//! so a device given the dataset signs nothing it could not sign before.

mod common;

use common::harness::*;
use common::*;
use rhtn_archive::tx::{TYPE_PRESENCE, Witness, presence_record_body};
use rhtn_client::ceremony::{Abort, Client, Config, Reconciled};
use rhtn_client::device::{Device, Operator};
use rhtn_client::local::DatasetContribution;
use rhtn_client::query::VerificationQuery;
use rhtn_client::store::{SealParams, open};
use rhtn_client::verifier::GrantOutcome;
use std::cell::Cell;
use std::rc::Rc;

/// Whether `needle` appears anywhere in `hay`.
fn find(hay: &[u8], needle: &[u8]) -> bool {
    hay.windows(needle.len()).any(|w| w == needle)
}

/// The two seeds alice's test identity is derived from, and the root they
/// derive: the three secrets that must appear in no dataset.
fn alice_secrets() -> Vec<Vec<u8>> {
    let ed = rhtn_codec::cose::sha256(b"rhtn-test-vectors:alice:ed25519-seed");
    let pq = rhtn_codec::cose::sha256(b"rhtn-test-vectors:alice:ml-dsa-65-seed");
    vec![
        ed.to_vec(),
        pq.to_vec(),
        id("alice").identity_root().to_vec(),
    ]
}

/// A person who says no to everything.
struct Refuser;
impl Operator for Refuser {
    fn ask(&self, _: &str) -> bool {
        false
    }
}

/// A device whose holder refuses every question.
fn refusing(clock: Rc<Cell<u64>>, seed: u64) -> Device {
    let (mut dev, _) = common::harness::device(clock, seed, 0);
    dev.operator = Rc::new(Refuser);
    dev
}

/// The two devices exchange contributions, each reading the other's off
/// its screen, in the order given.  What each learned of the other's heads.
fn pair(
    first: &mut Client,
    second: &mut Client,
) -> (Vec<rhtn_archive::Txid>, Vec<rhtn_archive::Txid>) {
    let a = first.dataset_contribution();
    let b = second.dataset_contribution();
    let seconds_heads = first
        .take_dataset_contribution(&b)
        .expect("the same identity");
    let firsts_heads = second
        .take_dataset_contribution(&a)
        .expect("the same identity");
    (seconds_heads, firsts_heads)
}

// functional: NET-019
/// The contribution reads back as it was composed (`wire-format.md`
/// §14.3.3): version 1, the identity's material, the contribution and the
/// heads; one naming no identity does not read.
#[test]
fn a_dataset_contribution_encodes_as_the_wire_says_and_reads_back() {
    let c = DatasetContribution {
        material: id("alice").public.key_material(),
        device: kh("alice"),
        contribution: [7; 16],
        heads: vec![[1; 32], [2; 32]],
    };
    let bytes = c.encode();
    assert_eq!(bytes[0], 0x84, "an array of four");
    assert_eq!(bytes[1], 0x01, "version 1");
    assert_eq!(DatasetContribution::decode(&bytes).unwrap(), c);
    let empty = DatasetContribution {
        heads: Vec::new(),
        ..c.clone()
    };
    assert_eq!(DatasetContribution::decode(&empty.encode()).unwrap(), empty);
    assert!(DatasetContribution::decode(&bytes[..bytes.len() - 1]).is_err());
    assert!(
        format!("{c:?}").contains("<redacted>"),
        "the contribution is never printed"
    );
}

// functional: APP-014
// acceptance: ARC-30
/// **A device offline across several transactions holds all of them after
/// one hand-over**, and the whole store with them: alice's phone met bob in
/// person and carol and w1 by record while her desktop, which holds no
/// seed, was away.  Paired across the table and both holders asked, the
/// desktop holds every record, every contact and the sealed capture as
/// ciphertext, and can sign, back up and release no more than before, the
/// replicated capture staying shut.
#[test]
fn a_device_that_was_away_is_brought_to_the_whole_dataset_once_both_holders_say_yes() {
    let mut s = setup(&["alice", "bob", "carol", "w1"]);
    let in_person = s.met_in_person("alice", "bob", "w1");
    s.met("alice", "carol", "w1");
    s.met("alice", "w1", "bob");
    let (dev, handles) = common::harness::device(s.clock.clone(), 77, 0);
    let mut desk = Client::delegated(
        id("alice").public,
        [0x42; 32],
        ids(),
        Config::default(),
        dev,
    );
    assert!(desk.archive.is_empty() && desk.store.sealed.is_empty());
    let asked_before = s.prompts_asked("alice").len();

    // **nothing moves before the pairing**: no pairing, then half a pairing
    assert_eq!(
        desk.take_dataset_carriage(b"anything").unwrap_err(),
        Abort::NotActive
    );
    let _ = desk.dataset_contribution();
    assert_eq!(
        desk.take_dataset_carriage(b"anything").unwrap_err(),
        Abort::NoCeremonyId
    );
    assert_eq!(
        s.client("alice").dataset_carriage().unwrap_err(),
        Abort::NotActive
    );

    // **a contribution from another identity's device is refused**
    let bobs = s.client("bob").dataset_contribution();
    assert_eq!(
        desk.take_dataset_contribution(&bobs).unwrap_err(),
        Abort::NotMine("a device of another identity")
    );

    let phone = s.client("alice");
    assert_eq!(phone.archive.len(), 3);
    assert!(
        phone.store.sealed.contains_key(&in_person),
        "the phone holds bob's capture, sealed"
    );
    let (desk_heads, phone_heads) = pair(phone, &mut desk);
    assert!(desk_heads.is_empty(), "the desktop has nothing");
    assert_eq!(phone_heads, phone.archive.heads());

    // **the phone's holder says yes, and the dataset is sealed**
    let sealed = phone.dataset_carriage().expect("the holder said yes");
    for secret in alice_secrets() {
        assert!(!find(&sealed, &secret), "no seed and no root cross");
    }
    assert_eq!(
        s.prompts_asked("alice").len(),
        asked_before + 1,
        "the phone's holder was asked once"
    );

    // **a tampered carriage is refused**, after the desk's holder was asked
    let mut tampered = sealed.clone();
    let last = tampered.len() - 1;
    tampered[last] ^= 1;
    assert_eq!(
        desk.take_dataset_carriage(&tampered).unwrap_err(),
        Abort::Unsealable
    );
    assert_eq!(handles.person.0.borrow().len(), 1);
    assert!(desk.archive.is_empty(), "nothing landed");

    let took = desk.take_dataset_carriage(&sealed).expect("taken");
    assert_eq!(
        handles.person.0.borrow().len(),
        2,
        "the desk's holder was asked once more"
    );
    assert_eq!(took.records, 3, "every record the desktop lacked");
    assert_eq!(took.refused, 0);
    let phone = s.client("alice");
    assert_eq!(desk.archive.heads(), phone.archive.heads());
    assert_eq!(desk.archive.len(), 3);
    assert_eq!(
        desk.store, phone.store,
        "the whole store, captures included"
    );

    // **a replica of a capture whose key the subject has not released does
    // not open here**: the desktop holds the ciphertext and no key to it
    let sealed_capture = &desk.store.sealed[&in_person];
    assert!(open(&SealParams::default(), &[0; 32], sealed_capture).is_err());
    assert!(
        desk.capture_key().is_err(),
        "no key is derived on a device without the seed"
    );

    // **no ability to sign as the identity, and none to release a key**
    assert!(!desk.holds_seed());
    assert_eq!(desk.sign_body(b"a body").unwrap_err(), Abort::NoSeed);
    assert_eq!(
        desk.export().unwrap_err(),
        rhtn_client::backup::Failure::NoKey
    );
    let q = VerificationQuery {
        subject: kh("alice"),
        querier: kh("carol"),
        ceremony_id: [1; 32],
        profile: vec![],
        template_version: 1,
        verifier: kh("bob"),
    };
    assert!(desk.consent(&q).is_none(), "no consent, so no grant");
    assert_eq!(
        desk.take_grant(kh("carol"), b"grant"),
        GrantOutcome::Rejected("this device holds no seed")
    );

    // **the same carriage again changes nothing**
    assert_eq!(
        desk.take_dataset_carriage(&sealed).unwrap(),
        Reconciled::default()
    );
}

// functional: APP-014
// acceptance: ARC-30
/// **A no on either side is the refusal**: a receiving holder who declines
/// takes nothing and nothing is opened; a sending holder who declines
/// composes nothing.
#[test]
fn a_holder_who_declines_moves_nothing_on_either_side() {
    let mut s = setup(&["alice", "bob", "w1"]);
    s.met("alice", "bob", "w1");
    let refusing_desk = refusing(s.clock.clone(), 78);
    let mut desk = Client::delegated(
        id("alice").public,
        [0x43; 32],
        ids(),
        Config::default(),
        refusing_desk,
    );
    pair(s.client("alice"), &mut desk);
    let sealed = s
        .client("alice")
        .dataset_carriage()
        .expect("the phone's holder said yes");
    assert_eq!(
        desk.take_dataset_carriage(&sealed).unwrap_err(),
        Abort::Declined
    );
    assert!(desk.archive.is_empty() && desk.store.contacts.is_empty());

    // and the other way: a phone whose holder refuses hands nothing over
    let refusing_phone = refusing(s.clock.clone(), 79);
    let mut phone = Client::new(id("alice"), ids(), Config::default(), refusing_phone);
    let mut other = Client::delegated(id("alice").public, [0x44; 32], ids(), Config::default(), {
        let (d, _) = common::harness::device(s.clock.clone(), 80, 0);
        d
    });
    pair(&mut phone, &mut other);
    assert_eq!(phone.dataset_carriage().unwrap_err(), Abort::Declined);
}

// functional: APP-014
// acceptance: ARC-30
/// **A fork merges rather than overwriting**: two devices of alice, each
/// holding the seed, sign a record apiece from one shared head while
/// apart.  Paired and handed each other's dataset, each holds both heads
/// and the other's record alone was sent; the next record either signs
/// carries both back-pointers and leaves one head, and the device that did
/// not sign it is brought level by a plain append.
#[test]
fn two_devices_that_signed_apart_merge_on_contact_and_the_next_record_closes_the_fork() {
    let clock = Rc::new(Cell::new(1_790_000_000_000u64));
    let (d1, _h1) = common::harness::device(clock.clone(), 1, 0);
    let (d2, _h2) = common::harness::device(clock.clone(), 2, 0);
    let mut a1 = Client::new(id("alice"), ids(), Config::default(), d1);
    let mut a2 = Client::new(id("alice"), ids(), Config::default(), d2);
    let mut w = World::new();

    // the shared prefix: alice and bob's formation, held by both devices
    let r0 = w.meet("alice", "bob");
    clock.set((r0.effective + 60) * 1000);
    a1.finalize(&r0.bytes, None).expect("the phone");
    a2.finalize(&r0.bytes, None).expect("the tablet");

    // apart: each device signs a meeting from the same head
    let signed = |c: &Client, other: &str, witness: &str, w: &mut World| {
        let t = w.tick();
        let back = vec![
            c.archive.next_back_pointers(),
            w.back(other),
            w.back(witness),
        ];
        let root = rhtn_codec::cose::sha256(format!("apart:{other}:{t}").as_bytes());
        let witness_entry = Witness {
            keyhash: kh(witness),
            nominated_by: kh("alice"),
            flags: 3,
        };
        let body = presence_record_body(
            &back,
            [&kh("alice"), &kh(other)],
            &[witness_entry],
            t,
            t + 600,
            &root,
        );
        clock.set((t + 660) * 1000);
        w.loose(TYPE_PRESENCE, &body, &["alice", other, witness])
    };
    let r1 = signed(&a1, "carol", "w1", &mut w);
    let r2 = signed(&a2, "w1", "carol", &mut w);
    a1.finalize(&r1, None).expect("r1 on the phone");
    a2.finalize(&r2, None).expect("r2 on the tablet");
    assert_eq!(a1.archive.len(), 2);
    assert_eq!(a2.archive.len(), 2);
    assert_ne!(a1.archive.heads(), a2.archive.heads(), "forked");

    // contact: paired once, each hands the other what it lacks, one record
    pair(&mut a1, &mut a2);
    let to_tablet = a1.dataset_carriage().unwrap();
    let took = a2.take_dataset_carriage(&to_tablet).unwrap();
    assert_eq!(
        (took.records, took.refused),
        (1, 0),
        "r1 alone: r0 was reached from the tablet's head"
    );
    let to_phone = a2.dataset_carriage().unwrap();
    assert_eq!(a1.take_dataset_carriage(&to_phone).unwrap().records, 1);
    for c in [&a1, &a2] {
        assert_eq!(c.archive.len(), 3);
        assert!(c.archive.is_forked(), "both heads held, neither dropped");
        assert_eq!(
            c.archive.next_back_pointers().len(),
            2,
            "the next record is a merge"
        );
    }
    assert_eq!(a1.archive.heads(), a2.archive.heads());

    // the next record the phone signs carries both heads (design §10.3)
    let r3 = signed(&a1, "bob", "w1", &mut w);
    a1.finalize(&r3, None).expect("the merge");
    assert!(!a1.archive.is_forked());
    assert_eq!(a1.archive.len(), 4);
    let merged = a1.archive.get(&a1.archive.heads()[0]).unwrap();
    assert_eq!(
        merged.back_pointers_of(&kh("alice")).unwrap().len(),
        2,
        "two back-pointers under alice's key"
    );

    // the tablet, simply short now, is brought level by one append on a
    // fresh pairing, whose contribution carries its heads as they stand
    pair(&mut a1, &mut a2);
    let to_tablet = a1.dataset_carriage().unwrap();
    let took = a2.take_dataset_carriage(&to_tablet).unwrap();
    assert_eq!(took.records, 1);
    assert!(!a2.archive.is_forked());
    assert_eq!(a2.archive.heads(), a1.archive.heads());
}
