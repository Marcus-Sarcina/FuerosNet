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
