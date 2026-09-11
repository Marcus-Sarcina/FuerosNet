//! The verifier's side of a query, in process: B holds sealed captures of
//! A from two records; C, having met A, queries B about A with A's consent;
//! A releases keys.  What B answers, closes and ignores.

mod common;

use common::*;
use rhtn_client::keys::capture_key;
use rhtn_client::query::*;
use rhtn_client::store::{Capture, ClientStore, Frame, OwnSeed, SealParams, seal};
use rhtn_client::verifier::*;
use rhtn_codec::encode::*;
use rhtn_crypto::verify;
use std::cell::RefCell;
use std::collections::BTreeSet;

/// A matcher that records every template it was shown.
struct Recording(RefCell<Vec<Vec<u8>>>);
impl Matcher for Recording {
    fn compare(&self, template: &[u8], profile: &[u8], _v: u64) -> Verdict {
        self.0.borrow_mut().push(template.to_vec());
        if template == profile { Verdict::Match } else { Verdict::NoMatch }
    }
}

/// B's store after two meetings with A, sealed under A's keys; A's seeds.
struct Setup {
    w: World,
    b_store: ClientStore,
    a_store: ClientStore,
    r1: [u8; 32],
    r2: [u8; 32],
    t1: Vec<u8>,
    t2: Vec<u8>,
    c1: [u8; 32],
    c2: [u8; 32],
}

fn setup() -> Setup {
    let mut w = World::new();
    let rec1 = w.meet("alice", "bob");
    let rec2 = w.meet("alice", "bob");
    let cb = w.meet("carol", "bob"); // C and B have met: C's claimed `met` holds
    let (c1, c2) = ([11u8; 32], [22u8; 32]);
    let (s1, s2) = ([31u8; 32], [32u8; 32]);
    let p = SealParams::default();
    let (t1, t2) = (vec![1u8; 32], vec![2u8; 32]);
    let mut b_store = ClientStore::default();
    for (rec, c, s, t) in [(&rec1, c1, s1, &t1), (&rec2, c2, s2, &t2)] {
        let key = capture_key(&s, &kh("alice"), &kh("bob"), &c);
        let cap = Capture { modality: 0, template_version: 1, template: t.clone(), frames: vec![Frame { at_ms: 1, bytes: b"a frame".to_vec() }] };
        b_store.sealed.insert(rec.txid, seal(&p, &key, rec.txid, kh("alice"), kh("bob"), c, &cap));
        b_store.records.insert(rec.txid, rec.bytes.clone());
    }
    b_store.records.insert(cb.txid, cb.bytes.clone());
    let mut a_store = ClientStore::default();
    for (rec, c, s) in [(&rec1, c1, s1), (&rec2, c2, s2)] {
        a_store.seeds.insert(rec.txid, OwnSeed { seed: s, counterparty: kh("bob"), ceremony_id: c, finalized_at: rec.effective });
        a_store.records.insert(rec.txid, rec.bytes.clone());
    }
    Setup { w, b_store, a_store, r1: rec1.txid, r2: rec2.txid, t1, t2, c1, c2 }
}

fn query(profile: &[u8], ceremony: [u8; 32], verifier: &str) -> VerificationQuery {
    VerificationQuery { subject: kh("alice"), querier: kh("carol"), ceremony_id: ceremony, profile: profile.to_vec(), template_version: 1, verifier: kh(verifier) }
}

fn request(q: &VerificationQuery, consenter: &str, basis: u64) -> Vec<u8> {
    QueryRequest { query: q.clone(), consent: consent(&id(consenter), &q.query_id()), selection_basis: basis }.encode()
}

fn grant(s: &Setup, record: [u8; 32], ceremony: [u8; 32], qid: [u8; 32]) -> Vec<u8> {
    let seed = s.a_store.seeds[&record].seed;
    KeyGrant { record, query_id: qid, key: capture_key(&seed, &kh("alice"), &kh("bob"), &ceremony) }.encode()
}

fn verdict_of(bytes: &[u8]) -> (Verdict, Option<Basis>, Option<u64>) {
    let r = Response::read(bytes).unwrap();
    verify::response(&ids(), bytes, false).expect("signed by B");
    (r.verdict, r.basis, r.template_version)
}

// acceptance: CER-13
#[test]
fn a_query_without_consent_or_not_addressed_to_me_closes_the_stream() {
    let s = setup();
    let me = id("bob");
    let m = Recording(RefCell::new(vec![]));
    let mut v = VerifierState::new(VerifierConfig::default());
    let cx = Verifying { me: &me, ids: &ids(), store: &s.b_store, matcher: &m };
    let q = query(&s.t2, [7; 32], "bob");
    // no consent beside the query: an empty signature slot
    let mut none = Vec::new();
    emit_array_head(&mut none, 3);
    none.extend_from_slice(&q.encode());
    emit_bstr(&mut none, b"");
    emit_uint(&mut none, 1);
    assert!(matches!(v.take_query(&cx, kh("carol"), &none, 0), QueryOutcome::Closed(_)));
    // consent that does not verify under the subject: bob's, not alice's
    assert!(matches!(v.take_query(&cx, kh("carol"), &request(&q, "bob", 1), 0), QueryOutcome::Closed(_)));
    // addressed to another verifier
    let other = query(&s.t2, [7; 32], "carol");
    assert!(matches!(v.take_query(&cx, kh("carol"), &request(&other, "alice", 1), 0), QueryOutcome::Closed(_)));
    // field 2 naming someone other than the authenticated requester
    assert!(matches!(v.take_query(&cx, kh("w1"), &request(&q, "alice", 1), 0), QueryOutcome::Closed(_)));
    assert!(m.0.borrow().is_empty(), "no comparison ran for any of them");
    assert!(v.awaiting().is_empty());
}

// acceptance: CER-28
#[test]
fn a_claimed_met_my_own_records_refute_is_answered_unavailable() {
    let s = setup();
    let me = id("bob");
    let m = Recording(RefCell::new(vec![]));
    let mut v = VerifierState::new(VerifierConfig::default());
    let cx = Verifying { me: &me, ids: &ids(), store: &s.b_store, matcher: &m };
    // w1 has never met bob, and claims to have
    let q = VerificationQuery { querier: kh("w1"), ..query(&s.t2, [7; 32], "bob") };
    let req = QueryRequest { query: q.clone(), consent: consent(&id("alice"), &q.query_id()), selection_basis: 0 }.encode();
    let QueryOutcome::Answered(a) = v.take_query(&cx, kh("w1"), &req, 0) else { panic!("answered") };
    assert_eq!(verdict_of(&a.to_querier), (Verdict::Unavailable, None, None));
    assert!(m.0.borrow().is_empty(), "no comparison ran");
    // carol, who has met bob, claiming met: the query proceeds and waits for its grant
    let q2 = query(&s.t2, [8; 32], "bob");
    assert_eq!(v.take_query(&cx, kh("carol"), &request(&q2, "alice", 0), 0), QueryOutcome::AwaitingGrant);
}

// acceptance: CER-12
#[test]
fn a_grant_naming_a_record_i_do_not_hold_yields_unavailable() {
    let s = setup();
    let me = id("bob");
    let m = Recording(RefCell::new(vec![]));
    let mut v = VerifierState::new(VerifierConfig::default());
    let cx = Verifying { me: &me, ids: &ids(), store: &s.b_store, matcher: &m };
    let q = query(&s.t2, [7; 32], "bob");
    assert_eq!(v.take_query(&cx, kh("carol"), &request(&q, "alice", 1), 0), QueryOutcome::AwaitingGrant);
    let g = KeyGrant { record: [99; 32], query_id: q.query_id(), key: [5; 32] }.encode();
    let GrantOutcome::Answered(a) = v.take_grant(&cx, kh("alice"), &g, 1) else { panic!("answered") };
    assert_eq!(verdict_of(&a.to_querier), (Verdict::Unavailable, None, None));
    assert!(m.0.borrow().is_empty());
}

// acceptance: CER-08
#[test]
fn only_the_capture_the_grant_names_is_opened() {
    let s = setup();
    let me = id("bob");
    let m = Recording(RefCell::new(vec![]));
    let mut v = VerifierState::new(VerifierConfig::default());
    let cx = Verifying { me: &me, ids: &ids(), store: &s.b_store, matcher: &m };
    let q = query(&s.t2, [7; 32], "bob");
    let qid = q.query_id();
    assert_eq!(v.take_query(&cx, kh("carol"), &request(&q, "alice", 1), 0), QueryOutcome::AwaitingGrant);
    let GrantOutcome::Answered(a) = v.take_grant(&cx, kh("alice"), &grant(&s, s.r2, s.c2, qid), 1) else { panic!("answered") };
    assert_eq!(verdict_of(&a.to_querier), (Verdict::Match, Some(Basis::PhotoMatch), Some(1)), "answered from R2's capture");
    assert_eq!(*m.0.borrow(), vec![s.t2.clone()], "R2's template and nothing else was compared");
    assert!(s.b_store.sealed.contains_key(&s.r1), "R1 is still sealed");
    let _ = s.t1;
}

// acceptance: CER-11
#[test]
fn a_grant_from_anyone_but_the_subject_is_rejected_and_the_first_stands() {
    let s = setup();
    let me = id("bob");
    let m = Recording(RefCell::new(vec![]));
    let mut v = VerifierState::new(VerifierConfig::default());
    let cx = Verifying { me: &me, ids: &ids(), store: &s.b_store, matcher: &m };
    let q = query(&s.t2, [7; 32], "bob");
    let qid = q.query_id();
    assert_eq!(v.take_query(&cx, kh("carol"), &request(&q, "alice", 1), 0), QueryOutcome::AwaitingGrant);
    // from carol: rejected, nothing opened
    assert!(matches!(v.take_grant(&cx, kh("carol"), &grant(&s, s.r2, s.c2, qid), 1), GrantOutcome::Rejected(_)));
    assert!(m.0.borrow().is_empty());
    // from alice: used
    let GrantOutcome::Answered(a) = v.take_grant(&cx, kh("alice"), &grant(&s, s.r2, s.c2, qid), 2) else { panic!("answered") };
    assert_eq!(verdict_of(&a.to_querier).0, Verdict::Match);
    // a differing second grant from alice for the same query changes nothing
    assert_eq!(v.take_grant(&cx, kh("alice"), &grant(&s, s.r1, s.c1, qid), 3), GrantOutcome::Ignored);
    assert_eq!(m.0.borrow().len(), 1, "no second comparison");
}

// acceptance: CER-10
#[test]
fn a_grant_unattached_to_a_countersigned_query_opens_nothing_and_is_discarded() {
    let s = setup();
    let me = id("bob");
    let m = Recording(RefCell::new(vec![]));
    let cfg = VerifierConfig { grant_buffer_ms: 1000, ..VerifierConfig::default() };
    let mut v = VerifierState::new(cfg);
    let cx = Verifying { me: &me, ids: &ids(), store: &s.b_store, matcher: &m };
    let q = query(&s.t2, [7; 32], "bob");
    let qid = q.query_id();
    // the grant first, naming a query B has not seen
    assert_eq!(v.take_grant(&cx, kh("alice"), &grant(&s, s.r2, s.c2, qid), 0), GrantOutcome::Buffered);
    assert_eq!(v.buffered(), vec![qid]);
    assert!(m.0.borrow().is_empty(), "nothing opened");
    // the bound elapses: the grant is gone unopened
    assert!(v.expire(&cx, 1000).is_empty());
    assert!(v.buffered().is_empty(), "discarded when the bound elapses");
    // the query arriving now finds no grant and waits
    assert_eq!(v.take_query(&cx, kh("carol"), &request(&q, "alice", 1), 1001), QueryOutcome::AwaitingGrant);
    assert!(m.0.borrow().is_empty());
}

// acceptance: CER-07
#[test]
fn no_released_key_is_retained_after_answering() {
    let s = setup();
    let me = id("bob");
    let m = Recording(RefCell::new(vec![]));
    let cfg = VerifierConfig { grant_buffer_ms: 1000, ..VerifierConfig::default() };
    let mut v = VerifierState::new(cfg);
    let cx = Verifying { me: &me, ids: &ids(), store: &s.b_store, matcher: &m };
    let q = query(&s.t2, [7; 32], "bob");
    assert_eq!(v.take_query(&cx, kh("carol"), &request(&q, "alice", 1), 0), QueryOutcome::AwaitingGrant);
    let g = grant(&s, s.r2, s.c2, q.query_id());
    let key = KeyGrant::decode(&g).unwrap().key;
    let GrantOutcome::Answered(_) = v.take_grant(&cx, kh("alice"), &g, 1) else { panic!("answered") };
    // the key is nowhere: not in the store, not in the verifier's state
    assert!(!s.b_store.holds_bytes(&key));
    assert!(v.buffered().is_empty() && v.awaiting().is_empty());
    assert!(!format!("{v:?}").contains(&hex::encode(key)) && !format!("{v:?}").contains(&format!("{key:?}")));
    // a second consented query about the same record with no new grant:
    // waits its bound, then unavailable
    let q2 = query(&s.t2, [8; 32], "bob");
    assert_eq!(v.take_query(&cx, kh("carol"), &request(&q2, "alice", 1), 10), QueryOutcome::AwaitingGrant);
    let answers = v.expire(&cx, 1010);
    assert_eq!(answers.len(), 1);
    assert_eq!(verdict_of(&answers[0].to_querier), (Verdict::Unavailable, None, None));
    assert_eq!(m.0.borrow().len(), 1, "compared once, under the one grant");
}

// acceptance: CER-15
#[test]
fn a_failed_decryption_is_inconclusive_never_no_match() {
    let mut s = setup();
    let me = id("bob");
    let m = Recording(RefCell::new(vec![]));
    let mut v = VerifierState::new(VerifierConfig::default());
    // R2's sealed capture truncated on disk
    let sealed = s.b_store.sealed.get_mut(&s.r2).unwrap();
    sealed.ciphertext.truncate(sealed.ciphertext.len() - 5);
    let cx = Verifying { me: &me, ids: &ids(), store: &s.b_store, matcher: &m };
    let q = query(&s.t2, [7; 32], "bob");
    let qid = q.query_id();
    assert_eq!(v.take_query(&cx, kh("carol"), &request(&q, "alice", 1), 0), QueryOutcome::AwaitingGrant);
    let GrantOutcome::Answered(a) = v.take_grant(&cx, kh("alice"), &grant(&s, s.r2, s.c2, qid), 1) else { panic!("answered") };
    assert_eq!(verdict_of(&a.to_querier), (Verdict::Inconclusive, Some(Basis::PhotoMatch), Some(1)), "basis 0 and the query's version");
    assert!(m.0.borrow().is_empty(), "no comparison ran on a capture that did not open");
}

// acceptance: CER-16
#[test]
fn a_second_different_profile_within_one_ceremony_is_rejected() {
    let s = setup();
    let me = id("bob");
    let m = Recording(RefCell::new(vec![]));
    let mut v = VerifierState::new(VerifierConfig::default());
    let cx = Verifying { me: &me, ids: &ids(), store: &s.b_store, matcher: &m };
    let ceremony = [7u8; 32];
    let q1 = query(&s.t2, ceremony, "bob");
    assert_eq!(v.take_query(&cx, kh("carol"), &request(&q1, "alice", 1), 0), QueryOutcome::AwaitingGrant);
    let GrantOutcome::Answered(_) = v.take_grant(&cx, kh("alice"), &grant(&s, s.r2, s.c2, q1.query_id()), 1) else { panic!("answered") };
    // the same ceremony, another profile, consented all the same
    let q2 = query(&s.t1, ceremony, "bob");
    assert!(matches!(v.take_query(&cx, kh("carol"), &request(&q2, "alice", 1), 2), QueryOutcome::Closed(_)));
    assert!(v.awaiting().is_empty(), "no second response for that ceremony");
    // the same query again is one already answered: closed, not answered twice
    let q4 = query(&s.t2, ceremony, "bob");
    assert_eq!(q4.query_id(), q1.query_id(), "one query per verifier per ceremony: this one repeats the first exactly");
    assert_eq!(v.take_query(&cx, kh("carol"), &request(&q4, "alice", 1), 3), QueryOutcome::Closed("already answered"));
    assert!(v.awaiting().is_empty());
}

// acceptance: CER-14
#[test]
fn a_copy_of_the_signed_response_goes_to_the_subject() {
    let s = setup();
    let me = id("bob");
    let m = Recording(RefCell::new(vec![]));
    let mut v = VerifierState::new(VerifierConfig::default());
    let cx = Verifying { me: &me, ids: &ids(), store: &s.b_store, matcher: &m };
    let q = query(&s.t2, [7; 32], "bob");
    assert_eq!(v.take_query(&cx, kh("carol"), &request(&q, "alice", 1), 0), QueryOutcome::AwaitingGrant);
    let GrantOutcome::Answered(a) = v.take_grant(&cx, kh("alice"), &grant(&s, s.r2, s.c2, q.query_id()), 1) else { panic!("answered") };
    assert_eq!(a.to_subject.0, kh("alice"));
    assert_eq!(a.to_subject.1, a.to_querier, "identical bytes to C and to A");
    let _ = s.w;
    let _: BTreeSet<u8> = BTreeSet::new();
}
