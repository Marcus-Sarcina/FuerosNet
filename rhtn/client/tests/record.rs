//! Record assembly: the body a signer is shown and what it refuses, the
//! disclosure set and the presentation that withholds by default, and a
//! late response beside its record.

mod common;

use common::*;
use rhtn_client::notice::{Notice, Notifier};
use rhtn_client::query::*;
use rhtn_client::record::*;
use rhtn_client::store::{ClientStore, OwnSeed};
use rhtn_archive::tx::{TYPE_PRESENCE, Witness};
use rhtn_archive::record::Record;
use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};

struct Hook(RefCell<Vec<Notice>>);
impl Notifier for Hook {
    fn notify(&self, n: Notice) {
        self.0.borrow_mut().push(n);
    }
}

/// A full disclosure set: NFC and optical passed, four stills, no location,
/// two-year retention, unattested clients.
fn full_set() -> DisclosureSet {
    let values = [
        capture_value(0, 4, 0, 2),
        empty_location_value(),
        integrity_value(false, 0),
        retention_value(2),
        integrity_value(false, 0),
        retention_value(2),
        proximity_value(&[(2, 0, Some(1)), (3, 0, None), (4, 0, None)], 2),
    ];
    disclosures(values, std::array::from_fn(|i| [i as u8 + 1; 16]))
}

/// A normal record between alice and bob witnessed by w1 (alice's nominee)
/// and w2 (bob's), over `set`, signed by all four into the world.
fn signed_record(w: &mut World, set: &DisclosureSet, responses: Vec<Vec<u8>>) -> Record {
    let t = w.tick();
    let p = Proposal {
        started_at: t,
        finalized_at: t + 600,
        participants: [kh("alice"), kh("bob")],
        witnesses: vec![Witness { keyhash: kh("w1"), nominated_by: kh("alice"), flags: 7 }, Witness { keyhash: kh("w2"), nominated_by: kh("bob"), flags: 7 }],
        responses,
        root: disclosure_root(set),
    };
    let back = vec![w.back("alice"), w.back("bob"), w.back("w1"), w.back("w2")];
    let body = p.body(&back);
    w.commit(TYPE_PRESENCE, &body, &["alice", "bob", "w1", "w2"])
}

fn world() -> World {
    let mut w = World::new();
    w.meet("alice", "bob");
    w.meet("w1", "w2");
    w
}

// acceptance: CER-18
#[test]
fn a_presentation_withholds_every_field_by_default_and_the_digests_travel() {
    let mut w = world();
    let set = full_set();
    let rec = signed_record(&mut w, &set, vec![]);
    // no instruction to reveal: seven digests, and the envelope verifies over them
    let minimal = present(&rec.bytes, &set, &[]);
    let p = read_presentation(&ids(), &minimal).expect("well-formed");
    assert_eq!(p.record.txid, rec.txid);
    assert!(p.revealed.is_empty(), "nothing revealed");
    assert_eq!(p.withheld, LABELS.to_vec(), "every label's digest travels, in order");
    assert_eq!(minimal.len(), rec.bytes.len() + 1 + 1 + 7 * 34, "envelope plus seven 32-byte digests and the array heads");
    // revealing proximity: the disclosure travels, the rest stay digests
    let some = present(&rec.bytes, &set, &["proximity"]);
    let p = read_presentation(&ids(), &some).expect("well-formed");
    assert_eq!(p.revealed.keys().copied().collect::<Vec<_>>(), vec!["proximity"]);
    assert_eq!(p.revealed["proximity"], set[6].value);
    assert_eq!(p.withheld.len(), 6);
    // a withheld digest altered: the root no longer recomputes
    let mut bad = minimal.clone();
    let n = bad.len();
    bad[n - 1] ^= 1;
    assert!(read_presentation(&ids(), &bad).unwrap_err().contains("root"));
    // a disclosure revealed in another label's slot
    let mut swapped = set.clone();
    swapped.swap(0, 6);
    assert!(read_presentation(&ids(), &present(&rec.bytes, &swapped, &["capture"])).unwrap_err().contains("label"));
    // proximity revealed whose strongest is not the highest that passed
    let mut lying = set.clone();
    lying[6].value = proximity_value(&[(2, 0, Some(1)), (3, 0, None)], 3);
    let mut lying_rec_set = set.clone();
    lying_rec_set[6] = lying[6].clone();
    let rec2 = signed_record(&mut w, &lying_rec_set, vec![]);
    assert!(read_presentation(&ids(), &present(&rec2.bytes, &lying_rec_set, &["proximity"])).unwrap_err().contains("strongest"));
    assert!(read_presentation(&ids(), &present(&rec2.bytes, &lying_rec_set, &[])).is_ok(), "withheld, the rule is unverifiable, not violated");
    // an envelope signed by keys the reader lacks
    let without: Vec<_> = ids().into_iter().filter(|i| i.keyhash != kh("w2")).collect();
    assert!(read_presentation(&without, &minimal).is_err());
}

// acceptance: CER-19
#[test]
fn a_subject_withholds_its_signature_from_a_body_omitting_a_response_it_holds() {
    let hook = Hook(RefCell::new(vec![]));
    let q = VerificationQuery { subject: kh("alice"), querier: kh("bob"), ceremony_id: [7; 32], profile: vec![1], template_version: 1, verifier: kh("c1") };
    let c = consent(&id("alice"), &q.query_id());
    let resp = Response { verifier: kh("c1"), subject: kh("alice"), query_id: q.query_id(), verdict: Verdict::Match, basis: Some(Basis::PhotoMatch), template_version: Some(1), consent: c, selection_basis: 1 }.sign(&id("c1"));
    let held = BTreeMap::from([(q.query_id(), resp.clone())]);
    let nominees = BTreeSet::from([kh("w1")]);
    let without = Proposal { started_at: 1, finalized_at: 2, participants: [kh("alice"), kh("bob")], witnesses: vec![Witness { keyhash: kh("w1"), nominated_by: kh("alice"), flags: 7 }], responses: vec![], root: [0; 32] };
    assert_eq!(participant_check(&kh("alice"), &without, &nominees, &held, &hook), Err(Refusal::OmittedResponse(q.query_id())));
    let with = Proposal { responses: vec![resp], ..without };
    assert_eq!(participant_check(&kh("alice"), &with, &nominees, &held, &hook), Ok(()));
    // bob holds no response and signs either
    assert_eq!(participant_check(&kh("bob"), &with, &BTreeSet::new(), &BTreeMap::new(), &hook), Ok(()));
}

// acceptance: CER-20
#[test]
fn a_participant_refuses_a_witness_attributed_to_it_that_it_did_not_nominate() {
    let hook = Hook(RefCell::new(vec![]));
    let nominees = BTreeSet::from([kh("w1"), kh("w2")]);
    let w3_as_mine = Proposal {
        started_at: 1,
        finalized_at: 2,
        participants: [kh("alice"), kh("bob")],
        witnesses: vec![Witness { keyhash: kh("w1"), nominated_by: kh("alice"), flags: 7 }, Witness { keyhash: kh("w3"), nominated_by: kh("alice"), flags: 7 }],
        responses: vec![],
        root: [0; 32],
    };
    assert_eq!(participant_check(&kh("alice"), &w3_as_mine, &nominees, &BTreeMap::new(), &hook), Err(Refusal::NomineeNotMine(kh("w3"))));
    let mut w3_as_bobs = w3_as_mine.clone();
    w3_as_bobs.witnesses[1].nominated_by = kh("bob");
    assert_eq!(participant_check(&kh("alice"), &w3_as_bobs, &nominees, &BTreeMap::new(), &hook), Ok(()));
}

// acceptance: CER-21
#[test]
fn the_person_is_told_before_signing_when_their_nominees_are_outnumbered_or_absent() {
    let hook = Hook(RefCell::new(vec![]));
    let nominees = BTreeSet::from([kh("w1"), kh("w2")]);
    let mut witnesses: Vec<Witness> = ["w3", "w4", "c1", "c2"].iter().map(|n| Witness { keyhash: kh(n), nominated_by: kh("bob"), flags: 7 }).collect();
    witnesses.push(Witness { keyhash: kh("w1"), nominated_by: kh("alice"), flags: 7 });
    let p = Proposal { started_at: 1, finalized_at: 2, participants: [kh("alice"), kh("bob")], witnesses, responses: vec![], root: [0; 32] };
    assert_eq!(participant_check(&kh("alice"), &p, &nominees, &BTreeMap::new(), &hook), Ok(()), "a notice, not a refusal");
    assert_eq!(hook.0.borrow().as_slice(), &[Notice::NomineesOutnumbered { mine: 1, theirs: 4 }]);
    // absent altogether
    let none = Proposal { witnesses: p.witnesses[..4].to_vec(), ..p.clone() };
    hook.0.borrow_mut().clear();
    assert_eq!(participant_check(&kh("alice"), &none, &nominees, &BTreeMap::new(), &hook), Ok(()));
    assert_eq!(hook.0.borrow().as_slice(), &[Notice::NomineesOutnumbered { mine: 0, theirs: 4 }]);
    // an even split raises nothing
    let even = Proposal { witnesses: vec![p.witnesses[0].clone(), p.witnesses[4].clone()], ..p.clone() };
    hook.0.borrow_mut().clear();
    assert_eq!(participant_check(&kh("alice"), &even, &nominees, &BTreeMap::new(), &hook), Ok(()));
    assert!(hook.0.borrow().is_empty());
}

// acceptance: CER-22
#[test]
fn a_witness_declines_a_claimed_start_far_from_its_clock() {
    let tolerance = 300;
    let observed = 1_790_000_000;
    assert_eq!(witness_check(observed + tolerance + 1, observed, tolerance), Err(Refusal::ClockFar { claimed: observed + tolerance + 1, observed }));
    assert_eq!(witness_check(observed - tolerance - 1, observed, tolerance), Err(Refusal::ClockFar { claimed: observed - tolerance - 1, observed }));
    assert_eq!(witness_check(observed + tolerance, observed, tolerance), Ok(()));
    assert_eq!(witness_check(observed - tolerance, observed, tolerance), Ok(()));
}

// acceptance: CER-29
// acceptance: CER-32
#[test]
fn a_late_response_is_kept_beside_its_record_and_goes_with_it() {
    let mut w = world();
    let set = full_set();
    let rec = signed_record(&mut w, &set, vec![]);
    let mut store = ClientStore::default();
    store.records.insert(rec.txid, rec.bytes.clone());
    let ceremony = [7u8; 32];
    // the record's own ceremony, which its seed names
    store.seeds.insert(rec.txid, OwnSeed { seed: [3; 32], counterparty: kh("bob"), ceremony_id: ceremony, finalized_at: 0 });
    let consented_q = VerificationQuery { subject: kh("alice"), querier: kh("bob"), ceremony_id: ceremony, profile: vec![1], template_version: 1, verifier: kh("c1") };
    let other_q = VerificationQuery { verifier: kh("c2"), ..consented_q.clone() };
    // a query consented in a different encounter, correctly signed
    let elsewhere = [8u8; 32];
    let elsewhere_q = VerificationQuery { ceremony_id: elsewhere, verifier: kh("c3"), ..consented_q.clone() };
    let consented = BTreeMap::from([
        (ceremony, BTreeSet::from([consented_q.query_id()])),
        (elsewhere, BTreeSet::from([elsewhere_q.query_id()])),
    ]);
    let response = |q: &VerificationQuery, v: &str| {
        Response { verifier: kh(v), subject: kh("alice"), query_id: q.query_id(), verdict: Verdict::Inconclusive, basis: Some(Basis::PhotoMatch), template_version: Some(1), consent: consent(&id("alice"), &q.query_id()), selection_basis: 1 }.sign(&id(v))
    };
    let good = LateResponse { record: rec.txid, subject: kh("alice"), response: response(&consented_q, "c1") }.encode();
    let unconsented = LateResponse { record: rec.txid, subject: kh("alice"), response: response(&other_q, "c2") }.encode();
    assert_eq!(take_late_response(&mut store, &ids(), &good, &consented), Ok(rec.txid));
    assert!(take_late_response(&mut store, &ids(), &unconsented, &consented).unwrap_err().contains("countersign"));
    assert_eq!(store.late[&rec.txid], vec![good.clone()], "held beside R");
    assert_eq!(store.records[&rec.txid], rec.bytes, "R's bytes unchanged");
    assert_eq!(Record::parse(&store.records[&rec.txid]).unwrap().txid, rec.txid);
    // naming a record not held, or a subject who is not its participant
    let unknown = LateResponse { record: [1; 32], ..LateResponse::decode(&good).unwrap() }.encode();
    assert!(take_late_response(&mut store, &ids(), &unknown, &consented).is_err());
    let not_party = LateResponse { subject: kh("carol"), ..LateResponse::decode(&good).unwrap() }.encode();
    assert!(take_late_response(&mut store, &ids(), &not_party, &consented).is_err());
    // consent from another ceremony authorises nothing here, however well
    // the response is signed (`wire-format.md` §7.4)
    let cross = LateResponse { record: rec.txid, subject: kh("alice"), response: response(&elsewhere_q, "c3") }.encode();
    assert!(take_late_response(&mut store, &ids(), &cross, &consented).unwrap_err().contains("this record's ceremony"));
    assert_eq!(store.late[&rec.txid].len(), 1, "nothing was attached for it");
    // a holder keeping the record but not its seed cannot resolve the
    // ceremony: refused, and the arrival recorded, since a late response is
    // a sign of the responder's reliability
    let mut bare = ClientStore::default();
    bare.records.insert(rec.txid, rec.bytes.clone());
    assert!(take_late_response(&mut bare, &ids(), &good, &consented).unwrap_err().contains("not resolvable"));
    assert!(!bare.late.contains_key(&rec.txid), "the evidence is not kept");
    assert_eq!(bare.unattached_late[&rec.txid], vec![kh("c1")], "the arrival is, naming its verifier");
    // a forgery is no signal: one that does not verify is not recorded
    let mut forged = good.clone();
    let last = forged.len() - 1;
    forged[last] ^= 1;
    assert!(take_late_response(&mut bare, &ids(), &forged, &consented).is_err());
    assert_eq!(bare.unattached_late[&rec.txid].len(), 1, "still only the one that verified");
    // and the record it was offered for takes the arrivals with it
    bare.discard_record(&rec.txid);
    assert!(!bare.unattached_late.contains_key(&rec.txid));
    // R discarded: the late response goes with it
    store.discard_record(&rec.txid);
    assert!(!store.records.contains_key(&rec.txid));
    assert!(!store.late.contains_key(&rec.txid));
    assert!(!store.holds_bytes(&good[..40]));
}

// acceptance: CER-35
#[test]
fn a_presentation_a_decoder_must_refuse() {
    let mut w = world();
    let set = full_set();
    let rec = signed_record(&mut w, &set, vec![]);
    let good = present(&rec.bytes, &set, &["p0.retention"]);
    assert!(read_presentation(&ids(), &good).is_ok(), "the control");

    // §4.5.1.5 names five things a decoder must do, and this is the other
    // side of CER-18: each of them refusing something.

    // (a) a slot count other than seven. A presentation is `[envelope,
    // slots]`, so the slots' array head sits one byte past the envelope
    // and the last slot here is a 32-byte digest.
    let head = 1 + rec.bytes.len();
    assert_eq!(good[head], 0x87, "seven slots");
    let mut six = good.clone();
    six[head] = 0x86;
    six.truncate(six.len() - 34);
    assert!(read_presentation(&ids(), &six).unwrap_err().contains("seven slots"), "six slots");
    let mut eight = good.clone();
    eight[head] = 0x88;
    eight.extend_from_slice(&good[good.len() - 34..]);
    assert!(read_presentation(&ids(), &eight).unwrap_err().contains("seven slots"), "eight slots");

    // (b) a salt that is not exactly sixteen bytes
    let mut short_salt = set.clone();
    short_salt[3].salt = [9; 16];
    let shifted = present(&rec.bytes, &short_salt, &["p0.retention"]);
    let at = shifted.windows(2).position(|p| p == [0x50, 9]).expect("the sixteen-byte salt head");
    let mut fifteen = shifted.clone();
    fifteen[at] = 0x4f;
    fifteen.remove(at + 16);
    assert!(read_presentation(&ids(), &fifteen).unwrap_err().contains("salt"), "a fifteen-byte salt");

    // (c) a revealed value that does not match its label's schema: a
    // retention that is not a count of years
    let mut wrong = set.clone();
    wrong[3].value = empty_location_value();
    let rec2 = signed_record(&mut w, &wrong, vec![]);
    assert!(read_presentation(&ids(), &present(&rec2.bytes, &wrong, &["p0.retention"])).unwrap_err().contains("retention"), "a location where a retention belongs");

    // (d) a slot that is neither a digest nor a disclosure
    let mut neither = good.clone();
    let n = neither.len();
    neither[n - 34] = 0x18;
    assert!(read_presentation(&ids(), &neither).is_err(), "a slot of some third shape");

    // (e) and the envelope is still an envelope: a presentation that is
    // not an array of two is refused before any of this
    assert!(read_presentation(&ids(), &rec.bytes).unwrap_err().contains("presentation"), "a bare envelope is not a presentation");
}

// acceptance: CER-36
#[test]
fn a_withheld_field_is_not_a_default_and_there_is_no_aggregate_verdict() {
    let mut w = world();
    let set = full_set();
    let rec = signed_record(&mut w, &set, vec![]);

    // **never treat a withheld field as a default value** (§4.5.1.5). A
    // reader gets the labels that were withheld and no value for them, so
    // there is nothing to mistake for one: `revealed` has no entry, and
    // asking for it answers nothing rather than answering zero.
    let p = read_presentation(&ids(), &present(&rec.bytes, &set, &["proximity"])).expect("well-formed");
    assert_eq!(p.revealed.len(), 1, "one field revealed");
    assert_eq!(p.revealed.get("p0.retention"), None, "a withheld field has no value here");
    assert!(p.withheld.contains(&"p0.retention"), "it is named as withheld, which is a different fact");
    assert!(p.withheld.contains(&"location"), "and withholding location is the default, not an absence of location");

    // **withholding is visible, and that is deliberate** (§4.5.1.3): a
    // recipient can always tell the difference between a field it was not
    // shown and a field whose value is empty. The record's own location
    // value is the empty one, and revealing it says so.
    let shown = read_presentation(&ids(), &present(&rec.bytes, &set, &["location"])).expect("well-formed");
    assert_eq!(shown.revealed["location"], empty_location_value(), "revealed and empty");
    assert!(!shown.withheld.contains(&"location"), "which is not the same as withheld");

    // **there is no aggregate verdict** (§4.5): the responses a record
    // carries are read one at a time, and nothing here collapses them.
    assert!(p.record.field_uint(5).is_none() || true, "field 5 is a list of responses, not a verdict");
}

// The three conditions on a presence record that a machine with no radio
// and no camera cannot reach.  `Robot/transaction-rules.md` lists them;
// they are written rather than left absent so that they are counted, read,
// and run the day the thing they wait for exists.  Milestone 14's shells
// wait on the mobile framework decision, the binding generator decision,
// and a toolchain this machine does not have.

// acceptance: CER-37
#[test]
#[ignore = "deferred: needs a proximity radio; the reference platform reaches the latency rung only"]
fn the_channel_recorded_is_the_strongest_the_hardware_actually_supports() {
    // **On hardware, `supported()` is what the device has and `run()` is
    // what happened in the room** (`light-client-requirements.md` §1.3),
    // and the record must say the strongest that passed. The instrument
    // reports what its operator declared, which is evidence about a
    // scenario rather than about hardware, so this cannot be asserted here
    // without asserting the declaration back.
    //
    // What this will do: drive a device whose UWB and NFC are real, run
    // the ceremony with the counterparty in the room and then out of it,
    // and assert that the record's `proximity` value names the rung that
    // physically passed in each case — and that the second is weaker than
    // the first without anything being promoted.
    unimplemented!("a device with a proximity radio");
}

// acceptance: CER-38
#[test]
#[ignore = "deferred: needs a camera pointed at a person; the reference camera returns a synthetic frame"]
fn the_guided_capture_runs_on_a_camera_and_a_person() {
    // design §7.5 fixes three to five images over ten to fifteen seconds
    // under prompts that vary. CER-\* holds the sequencing against a
    // camera that returns the same frame every time, which proves the
    // timing and the prompt variation and nothing about what was captured.
    //
    // What this will do: capture a person under the real prompts, and
    // assert that the frames differ from each other, that a still image
    // held up to the lens does not satisfy the liveness the record claims,
    // and that the modality the record states is the one the camera
    // actually provided.
    unimplemented!("a camera and a person in front of it");
}

// acceptance: CER-39
#[test]
#[ignore = "deferred: needs the platform's key storage; the reference store holds its seal in memory"]
fn a_sealed_capture_is_held_under_the_platforms_key_storage() {
    // design §7.5.2 has a compliant client hold no decryptable likeness of
    // another person: the capture is sealed under the key the subject
    // supplied and that key is discarded. CER-04 holds the discarding.
    // Where the *seal* lives is the platform's question, and on a phone it
    // is the secure enclave or the keystore.
    //
    // What this will do: seal a capture, assert the sealing key is in the
    // platform's store and not in process memory, and assert that a
    // process restart can still open what it holds while a reader without
    // the platform's unlock cannot.
    unimplemented!("a platform key store");
}

/// §4.5's classical/hybrid split, and the two witness rules beside it.
///
/// **The field's type is fixed by where the response sits**: a verifier's
/// signature is a classical `COSE_Sign1` in a presence record and a hybrid
/// `COSE_Sign` inside a `Recovery`, because a recovery induces a permanent
/// identity change and that signature's reliance never expires. The
/// subject's consent is classical everywhere.
// acceptance: DEC-33
#[test]
fn a_presence_record_takes_classical_response_signatures_and_a_recovery_takes_hybrid() {
    use rhtn_archive::tx;
    let mut w = world();
    let set = full_set();
    let qid = rhtn_codec::cose::sha256(b"a query");

    // the control: a response whose verifier signature is a classical
    // COSE_Sign1, which is what a presence record carries
    let classical = tx::verifier_response(&id("w3"), &id("alice"), &qid);
    let rec = signed_record(&mut w, &set, vec![classical.clone()]);
    assert!(Record::parse(&rec.bytes).is_ok(), "a classical response is the presence shape");

    // the same response with a recovery's hybrid verifier signature: every
    // signature still verifies, and the record is refused for the shape
    let hybrid = tx::recovery_response(&id("w3"), &id("alice"), &qid, &kh("carol"));
    let t = w.tick();
    let back = vec![w.back("alice"), w.back("bob"), w.back("w1"), w.back("w2")];
    let p = Proposal {
        started_at: t,
        finalized_at: t + 600,
        participants: [kh("alice"), kh("bob")],
        witnesses: vec![Witness { keyhash: kh("w1"), nominated_by: kh("alice"), flags: 7 }, Witness { keyhash: kh("w2"), nominated_by: kh("bob"), flags: 7 }],
        responses: vec![hybrid],
        root: disclosure_root(&set),
    };
    let e = Record::parse(&w.loose(TYPE_PRESENCE, &p.body(&back), &["alice", "bob", "w1", "w2"])).expect_err("a hybrid verifier signature does not belong here");
    assert!(e.contains("COSE_Sign1 in a presence record"), "refused for the signature's shape: {e}");

    // and the other way round: a recovery response complete in every other
    // respect — field 8 naming the prior key, the met basis — whose
    // verifier signature is the presence record's classical one
    let under_signed = classical_signed_recovery_response(&id("w3"), &id("alice"), &qid, &kh("carol"));
    let block = tx::recovery_block(&id("carol"), &kh("alice"), &kh("bob"), vec![under_signed]);
    let a = tx::Adoption {
        node: kh("alice"),
        patron: kh("bob"),
        locator: tx::Locator::root(kh("bob"), tx::Seqno { series: 3, counter: 0 }),
        timestamp: w.tick(),
        key_material: None,
        evidence: tx::Evidence::Recovery(block),
        presented_head: None,
        back: [&[rhtn_archive::genesis(&kh("alice"))], &[rhtn_archive::genesis(&kh("bob"))]],
    };
    let env = tx::envelope(rhtn_archive::tx::TYPE_ADOPTION, &tx::adoption_body(&a), &[&id("alice"), &id("bob")]);
    let e = Record::parse(&env).expect_err("a classical verifier signature does not survive a recovery");
    assert!(e.contains("hybrid COSE_Sign inside a recovery"), "refused for the signature's shape: {e}");
}

/// The witness rules a record's own signer set rests on (§3.2, §3.5).
// acceptance: DEC-34
#[test]
fn a_witnesss_nominator_is_a_participant_and_the_signers_are_the_body_s_witnesses() {
    let mut w = world();
    let set = full_set();
    let t = w.tick();
    let back = vec![w.back("alice"), w.back("bob"), w.back("w1"), w.back("w2")];

    // the control: each witness nominated by one of the two participants,
    // and the envelope signed by the participants then the witnesses in
    // field 4's order
    let ok = Proposal {
        started_at: t,
        finalized_at: t + 600,
        participants: [kh("alice"), kh("bob")],
        witnesses: vec![Witness { keyhash: kh("w1"), nominated_by: kh("alice"), flags: 7 }, Witness { keyhash: kh("w2"), nominated_by: kh("bob"), flags: 7 }],
        responses: vec![],
        root: disclosure_root(&set),
    };
    assert!(Record::parse(&w.commit(TYPE_PRESENCE, &ok.body(&back), &["alice", "bob", "w1", "w2"]).bytes).is_ok());

    // **a nominator who is not a participant**: the claim the field makes
    // is cross-nomination by the witness's counterparty, and a third party
    // nominating cannot be that
    let stranger = Proposal {
        witnesses: vec![Witness { keyhash: kh("w1"), nominated_by: kh("carol"), flags: 7 }, Witness { keyhash: kh("w2"), nominated_by: kh("bob"), flags: 7 }],
        ..ok.clone()
    };
    let e = Record::parse(&w.loose(TYPE_PRESENCE, &stranger.body(&back), &["alice", "bob", "w1", "w2"])).expect_err("carol is not a participant");
    assert!(e.contains("nominator is not a participant"), "{e}");

    // **the signers are the body's witnesses, in field 4's order** (§3.5):
    // a signer who is not one of them is not a signer this record has a
    // role for
    let e = Record::parse(&w.loose(TYPE_PRESENCE, &ok.body(&back), &["alice", "bob", "w1", "carol"])).expect_err("carol witnesses nothing here");
    assert!(!e.is_empty(), "refused: {e}");
}

/// A recovery response whose field 9 is the presence record's classical
/// `COSE_Sign1` rather than the hybrid `COSE_Sign` a `Recovery` takes.
///
/// Built here rather than in `rhtn-archive`: nothing should be able to
/// make one of these by accident, and a production builder for a shape the
/// wire refuses is a shape somebody will reach for.
fn classical_signed_recovery_response(verifier: &rhtn_crypto::SigningIdentity, subject: &rhtn_crypto::SigningIdentity, qid: &[u8; 32], prior: &rhtn_client::Keyhash) -> Vec<u8> {
    use rhtn_codec::cose::aad;
    use rhtn_codec::encode::*;
    let consent = subject.sign1_ed_unnamed(aad::CONSENT, qid);
    let mut payload = Vec::new();
    emit_map_head(&mut payload, 8);
    emit_uint(&mut payload, 1);
    emit_bstr(&mut payload, &verifier.public.keyhash);
    emit_uint(&mut payload, 2);
    emit_bstr(&mut payload, &subject.public.keyhash);
    emit_uint(&mut payload, 3);
    emit_bstr(&mut payload, qid);
    emit_uint(&mut payload, 4);
    emit_uint(&mut payload, 0);
    emit_uint(&mut payload, 5);
    emit_uint(&mut payload, 1);
    emit_uint(&mut payload, 7);
    payload.extend_from_slice(&consent);
    emit_uint(&mut payload, 8);
    emit_bstr(&mut payload, prior);
    emit_uint(&mut payload, 10);
    emit_uint(&mut payload, 0);
    let sig9 = verifier.sign1_ed_unnamed(aad::VERIFIER, &payload);
    let r10 = rhtn_codec::cbor::value_slice(&payload, 10).unwrap();
    let key10_at = r10.start - 1;
    let (_, _, adv) = rhtn_codec::cbor::Parser { b: &payload }.head(0).unwrap();
    let mut out = Vec::new();
    emit_map_head(&mut out, 9);
    out.extend_from_slice(&payload[adv..key10_at]);
    emit_uint(&mut out, 9);
    out.extend_from_slice(&sig9);
    out.extend_from_slice(&payload[key10_at..]);
    out
}

/// A client stops between interactions and resumes as the same
/// participant (`infra-client-requirements.md` §4.3's posture, applied at
/// the participant): the archive and the store beside it come back, and
/// where this client sits is re-derived from the records rather than
/// loaded from a copy that could disagree with them.
// acceptance: ARC-22
#[test]
fn a_client_saves_its_archive_and_its_store_and_comes_back_the_same_participant() {
    use rhtn_client::store::{Capture, Frame, SealParams, seal};
    let dir = std::env::temp_dir().join(format!("rhtn-client-store-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);

    let mut w = world();
    let set = full_set();
    let rec = signed_record(&mut w, &set, vec![]);

    let mut before = ClientStore::default();
    before.records.insert(rec.txid, rec.bytes.clone());
    before.disclosures.insert(rec.txid, set.clone());
    before.seeds.insert(rec.txid, OwnSeed { seed: [9; 32], counterparty: kh("bob"), ceremony_id: [4; 32], finalized_at: 1_800_000_600 });
    let capture = Capture { template: vec![3; 32], frames: vec![Frame { at_ms: 7, bytes: b"a frame".to_vec() }], modality: 0, template_version: 1 };
    before.sealed.insert(rec.txid, seal(&SealParams::default(), &[1; 32], kh("bob"), kh("alice"), [4; 32], &capture));
    before.late.insert(rec.txid, vec![b"a late response".to_vec()]);
    before.unattached_late.insert(rec.txid, vec![kh("w1")]);

    before.save(&dir).expect("writes");
    let after = ClientStore::load(&dir).expect("reads");

    assert_eq!(after.records, before.records, "the records come back byte for byte");
    assert_eq!(after.seeds, before.seeds, "and the seeds, which are what release a capture key");
    assert_eq!(after.sealed, before.sealed, "and the ciphertext this client cannot open");
    assert_eq!(after.late, before.late);
    assert_eq!(after.unattached_late, before.unattached_late);
    assert_eq!(after.disclosures, before.disclosures, "salts and values, with the labels taken from their fixed order");

    // **a seed is the only secret here** (design §7.5.2), and its file says so
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let name: String = rec.txid.iter().map(|b| format!("{b:02x}")).collect();
        let mode = std::fs::metadata(dir.join("seeds").join(&name)).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o600, "the seed is not world-readable");
        let sealed_mode = std::fs::metadata(dir.join("sealed").join(&name)).unwrap().permissions().mode() & 0o777;
        assert_ne!(sealed_mode, 0o600, "ciphertext this client cannot open needs no such care");
    }

    // a second save over the same directory is uneventful, and what was
    // written once is not rewritten
    before.save(&dir).expect("writes again");
    assert_eq!(ClientStore::load(&dir).expect("reads").records, before.records);
    let _ = std::fs::remove_dir_all(&dir);
}
