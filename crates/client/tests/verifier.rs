//! The verifier's side of a query, in process: B holds sealed captures of
//! A from two records; C, having met A, queries B about A with A's consent;
//! A releases keys.  What B answers, closes and ignores.

mod common;

use common::*;
use rhtn_client::keys::capture_key;
use rhtn_client::query::*;
use rhtn_client::store::{Capture, ClientStore, Frame, OwnCapture, SealParams, seal};
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
        if template == profile {
            Verdict::Match
        } else {
            Verdict::NoMatch
        }
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
    // one root for every capture of alice: the key is derived from her
    // identity, not from anything kept per ceremony (design §7.5.2.6)
    let a_root = *rhtn_crypto::identity::testkit::test_identity("alice").identity_root();
    let p = SealParams::default();
    let (t1, t2) = (vec![1u8; 32], vec![2u8; 32]);
    let mut b_store = ClientStore::default();
    for (rec, c, t) in [(&rec1, c1, &t1), (&rec2, c2, &t2)] {
        let key = capture_key(&a_root, &kh("alice"), &kh("bob"), &c);
        let cap = Capture {
            modality: 0,
            template_version: 1,
            template: t.clone(),
            frames: vec![Frame {
                at_ms: 1,
                bytes: b"a frame".to_vec(),
            }],
        };
        b_store
            .sealed
            .insert(rec.txid, seal(&p, &key, kh("alice"), kh("bob"), c, &cap));
        b_store.records.insert(rec.txid, rec.bytes.clone());
    }
    b_store.records.insert(cb.txid, cb.bytes.clone());
    let mut a_store = ClientStore::default();
    for (rec, c) in [(&rec1, c1), (&rec2, c2)] {
        a_store.own.insert(
            rec.txid,
            OwnCapture {
                counterparty: kh("bob"),
                ceremony_id: c,
                finalized_at: rec.effective,
            },
        );
        a_store.records.insert(rec.txid, rec.bytes.clone());
    }
    Setup {
        w,
        b_store,
        a_store,
        r1: rec1.txid,
        r2: rec2.txid,
        t1,
        t2,
        c1,
        c2,
    }
}

fn query(profile: &[u8], ceremony: [u8; 32], verifier: &str) -> VerificationQuery {
    VerificationQuery {
        subject: kh("alice"),
        querier: kh("carol"),
        ceremony_id: ceremony,
        profile: profile.to_vec(),
        template_version: 1,
        verifier: kh(verifier),
    }
}

fn request(q: &VerificationQuery, consenter: &str, basis: u64) -> Vec<u8> {
    QueryRequest {
        query: q.clone(),
        consent: consent(&id(consenter), &q.query_id()),
        selection_basis: basis,
    }
    .encode()
}

fn grant(s: &Setup, record: [u8; 32], ceremony: [u8; 32], qid: [u8; 32]) -> Vec<u8> {
    let _ = &s.a_store.own[&record];
    let root = *rhtn_crypto::identity::testkit::test_identity("alice").identity_root();
    KeyGrant {
        record,
        query_id: qid,
        key: capture_key(&root, &kh("alice"), &kh("bob"), &ceremony),
    }
    .encode()
    .to_vec()
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
    let cx = Verifying {
        seal: &SealParams::default(),
        me: &me,
        ids: &ids(),
        store: &s.b_store,
        matcher: &m,
    };
    let q = query(&s.t2, [7; 32], "bob");
    // no consent beside the query: an empty signature slot
    let mut none = Vec::new();
    emit_array_head(&mut none, 3);
    none.extend_from_slice(&q.encode());
    emit_bstr(&mut none, b"");
    emit_uint(&mut none, 1);
    assert!(matches!(
        v.take_query(&cx, kh("carol"), &none, 0),
        QueryOutcome::Closed(_)
    ));
    // consent that does not verify under the subject: bob's, not alice's
    assert!(matches!(
        v.take_query(&cx, kh("carol"), &request(&q, "bob", 1), 0),
        QueryOutcome::Closed(_)
    ));
    // addressed to another verifier
    let other = query(&s.t2, [7; 32], "carol");
    assert!(matches!(
        v.take_query(&cx, kh("carol"), &request(&other, "alice", 1), 0),
        QueryOutcome::Closed(_)
    ));
    // field 2 naming someone other than the authenticated requester
    assert!(matches!(
        v.take_query(&cx, kh("w1"), &request(&q, "alice", 1), 0),
        QueryOutcome::Closed(_)
    ));
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
    let cx = Verifying {
        seal: &SealParams::default(),
        me: &me,
        ids: &ids(),
        store: &s.b_store,
        matcher: &m,
    };
    // w1 has never met bob, and claims to have
    let q = VerificationQuery {
        querier: kh("w1"),
        ..query(&s.t2, [7; 32], "bob")
    };
    let req = QueryRequest {
        query: q.clone(),
        consent: consent(&id("alice"), &q.query_id()),
        selection_basis: 0,
    }
    .encode();
    let QueryOutcome::Answered(a) = v.take_query(&cx, kh("w1"), &req, 0) else {
        panic!("answered")
    };
    assert_eq!(
        verdict_of(&a.to_querier),
        (Verdict::Unavailable, None, None)
    );
    assert!(m.0.borrow().is_empty(), "no comparison ran");
    // carol, who has met bob, claiming met: the query proceeds and waits for its grant
    let q2 = query(&s.t2, [8; 32], "bob");
    assert_eq!(
        v.take_query(&cx, kh("carol"), &request(&q2, "alice", 0), 0),
        QueryOutcome::AwaitingGrant
    );
}

// acceptance: CER-12
#[test]
fn a_grant_naming_a_record_i_do_not_hold_yields_unavailable() {
    let s = setup();
    let me = id("bob");
    let m = Recording(RefCell::new(vec![]));
    let mut v = VerifierState::new(VerifierConfig::default());
    let cx = Verifying {
        seal: &SealParams::default(),
        me: &me,
        ids: &ids(),
        store: &s.b_store,
        matcher: &m,
    };
    let q = query(&s.t2, [7; 32], "bob");
    assert_eq!(
        v.take_query(&cx, kh("carol"), &request(&q, "alice", 1), 0),
        QueryOutcome::AwaitingGrant
    );
    let g = KeyGrant {
        record: [99; 32],
        query_id: q.query_id(),
        key: [5; 32].into(),
    }
    .encode();
    let GrantOutcome::Answered(a) = v.take_grant(&cx, kh("alice"), &g, 1) else {
        panic!("answered")
    };
    assert_eq!(
        verdict_of(&a.to_querier),
        (Verdict::Unavailable, None, None)
    );
    assert!(m.0.borrow().is_empty());
}

// acceptance: CER-08
#[test]
fn only_the_capture_the_grant_names_is_opened() {
    let s = setup();
    let me = id("bob");
    let m = Recording(RefCell::new(vec![]));
    let mut v = VerifierState::new(VerifierConfig::default());
    let cx = Verifying {
        seal: &SealParams::default(),
        me: &me,
        ids: &ids(),
        store: &s.b_store,
        matcher: &m,
    };
    let q = query(&s.t2, [7; 32], "bob");
    let qid = q.query_id();
    assert_eq!(
        v.take_query(&cx, kh("carol"), &request(&q, "alice", 1), 0),
        QueryOutcome::AwaitingGrant
    );
    let GrantOutcome::Answered(a) = v.take_grant(&cx, kh("alice"), &grant(&s, s.r2, s.c2, qid), 1)
    else {
        panic!("answered")
    };
    assert_eq!(
        verdict_of(&a.to_querier),
        (Verdict::Match, Some(Basis::PhotoMatch), Some(1)),
        "answered from R2's capture"
    );
    assert_eq!(
        *m.0.borrow(),
        vec![s.t2.clone()],
        "R2's template and nothing else was compared"
    );
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
    let cx = Verifying {
        seal: &SealParams::default(),
        me: &me,
        ids: &ids(),
        store: &s.b_store,
        matcher: &m,
    };
    let q = query(&s.t2, [7; 32], "bob");
    let qid = q.query_id();
    assert_eq!(
        v.take_query(&cx, kh("carol"), &request(&q, "alice", 1), 0),
        QueryOutcome::AwaitingGrant
    );
    // from carol: rejected, nothing opened
    assert!(matches!(
        v.take_grant(&cx, kh("carol"), &grant(&s, s.r2, s.c2, qid), 1),
        GrantOutcome::Rejected(_)
    ));
    assert!(m.0.borrow().is_empty());
    // from alice: used
    let GrantOutcome::Answered(a) = v.take_grant(&cx, kh("alice"), &grant(&s, s.r2, s.c2, qid), 2)
    else {
        panic!("answered")
    };
    assert_eq!(verdict_of(&a.to_querier).0, Verdict::Match);
    // a differing second grant from alice for the same query changes nothing
    assert_eq!(
        v.take_grant(&cx, kh("alice"), &grant(&s, s.r1, s.c1, qid), 3),
        GrantOutcome::Ignored
    );
    assert_eq!(m.0.borrow().len(), 1, "no second comparison");
}

// acceptance: CER-10
#[test]
fn a_grant_unattached_to_a_countersigned_query_opens_nothing_and_is_discarded() {
    let s = setup();
    let me = id("bob");
    let m = Recording(RefCell::new(vec![]));
    let cfg = VerifierConfig {
        grant_buffer_ms: 1000,
        ..VerifierConfig::default()
    };
    let mut v = VerifierState::new(cfg);
    let cx = Verifying {
        seal: &SealParams::default(),
        me: &me,
        ids: &ids(),
        store: &s.b_store,
        matcher: &m,
    };
    let q = query(&s.t2, [7; 32], "bob");
    let qid = q.query_id();
    // the grant first, naming a query B has not seen
    assert_eq!(
        v.take_grant(&cx, kh("alice"), &grant(&s, s.r2, s.c2, qid), 0),
        GrantOutcome::Buffered
    );
    assert_eq!(v.buffered(), vec![qid]);
    assert!(m.0.borrow().is_empty(), "nothing opened");
    // the bound elapses: the grant is gone unopened
    assert!(v.expire(&cx, 1000).is_empty());
    assert!(v.buffered().is_empty(), "discarded when the bound elapses");
    // the query arriving now finds no grant and waits
    assert_eq!(
        v.take_query(&cx, kh("carol"), &request(&q, "alice", 1), 1001),
        QueryOutcome::AwaitingGrant
    );
    assert!(m.0.borrow().is_empty());
}

// acceptance: CER-07
#[test]
fn no_released_key_is_retained_after_answering() {
    let s = setup();
    let me = id("bob");
    let m = Recording(RefCell::new(vec![]));
    let cfg = VerifierConfig {
        grant_buffer_ms: 1000,
        ..VerifierConfig::default()
    };
    let mut v = VerifierState::new(cfg);
    let cx = Verifying {
        seal: &SealParams::default(),
        me: &me,
        ids: &ids(),
        store: &s.b_store,
        matcher: &m,
    };
    let q = query(&s.t2, [7; 32], "bob");
    assert_eq!(
        v.take_query(&cx, kh("carol"), &request(&q, "alice", 1), 0),
        QueryOutcome::AwaitingGrant
    );
    let g = grant(&s, s.r2, s.c2, q.query_id());
    let key = KeyGrant::decode(&g).unwrap().key;
    let GrantOutcome::Answered(_) = v.take_grant(&cx, kh("alice"), &g, 1) else {
        panic!("answered")
    };
    // the key is nowhere: not in the store, not in the verifier's state
    assert!(!s.b_store.holds_bytes(&key[..]));
    assert!(v.buffered().is_empty() && v.awaiting().is_empty());
    assert!(
        !format!("{v:?}").contains(&hex::encode(*key))
            && !format!("{v:?}").contains(&format!("{key:?}"))
    );
    // a second consented query about the same record with no new grant:
    // waits its bound, then unavailable
    let q2 = query(&s.t2, [8; 32], "bob");
    assert_eq!(
        v.take_query(&cx, kh("carol"), &request(&q2, "alice", 1), 10),
        QueryOutcome::AwaitingGrant
    );
    let answers = v.expire(&cx, 1010);
    assert_eq!(answers.len(), 1);
    assert_eq!(
        verdict_of(&answers[0].to_querier),
        (Verdict::Unavailable, None, None)
    );
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
    let n = sealed.slots[0].len();
    sealed.slots[0].truncate(n - 5);
    let cx = Verifying {
        seal: &SealParams::default(),
        me: &me,
        ids: &ids(),
        store: &s.b_store,
        matcher: &m,
    };
    let q = query(&s.t2, [7; 32], "bob");
    let qid = q.query_id();
    assert_eq!(
        v.take_query(&cx, kh("carol"), &request(&q, "alice", 1), 0),
        QueryOutcome::AwaitingGrant
    );
    let GrantOutcome::Answered(a) = v.take_grant(&cx, kh("alice"), &grant(&s, s.r2, s.c2, qid), 1)
    else {
        panic!("answered")
    };
    assert_eq!(
        verdict_of(&a.to_querier),
        (Verdict::Inconclusive, Some(Basis::PhotoMatch), Some(1)),
        "basis 0 and the query's version"
    );
    assert!(
        m.0.borrow().is_empty(),
        "no comparison ran on a capture that did not open"
    );
}

// acceptance: CER-16
#[test]
fn a_second_different_profile_within_one_ceremony_is_rejected() {
    let s = setup();
    let me = id("bob");
    let m = Recording(RefCell::new(vec![]));
    let mut v = VerifierState::new(VerifierConfig::default());
    let cx = Verifying {
        seal: &SealParams::default(),
        me: &me,
        ids: &ids(),
        store: &s.b_store,
        matcher: &m,
    };
    let ceremony = [7u8; 32];
    let q1 = query(&s.t2, ceremony, "bob");
    assert_eq!(
        v.take_query(&cx, kh("carol"), &request(&q1, "alice", 1), 0),
        QueryOutcome::AwaitingGrant
    );
    let GrantOutcome::Answered(_) =
        v.take_grant(&cx, kh("alice"), &grant(&s, s.r2, s.c2, q1.query_id()), 1)
    else {
        panic!("answered")
    };
    // the same ceremony, another profile, consented all the same
    let q2 = query(&s.t1, ceremony, "bob");
    assert!(matches!(
        v.take_query(&cx, kh("carol"), &request(&q2, "alice", 1), 2),
        QueryOutcome::Closed(_)
    ));
    assert!(
        v.awaiting().is_empty(),
        "no second response for that ceremony"
    );
    // the same query again is one already answered: closed, not answered twice
    let q4 = query(&s.t2, ceremony, "bob");
    assert_eq!(
        q4.query_id(),
        q1.query_id(),
        "one query per verifier per ceremony: this one repeats the first exactly"
    );
    assert_eq!(
        v.take_query(&cx, kh("carol"), &request(&q4, "alice", 1), 3),
        QueryOutcome::Closed("already answered")
    );
    assert!(v.awaiting().is_empty());
}

// acceptance: CER-16
/// **A ceremony has two subjects, and one verifier may be asked about
/// both.** The pre-commitment is one per ceremony and both parties
/// countersign it, so carol asking about alice and alice asking about
/// carol carry the same ceremony with different subjects, each profile
/// countersigned by its own subject (VER-005). Pinned per ceremony alone,
/// the second was closed: on the bench a verifier both parties had chosen
/// answered one query and refused the other [2026-10-12].
#[test]
fn each_of_a_ceremonys_two_subjects_gets_its_own_profile() {
    let mut s = setup();
    let me = id("bob");
    let m = Recording(RefCell::new(vec![]));
    let mut v = VerifierState::new(VerifierConfig::default());

    // bob holds a capture of carol too, from a meeting of their own, and
    // carol holds its seed
    let cb = s.w.meet("carol", "bob");
    let mut b_store = s.b_store.clone();
    b_store.records.insert(cb.txid, cb.bytes.clone());
    let (cc, sc) = ([33u8; 32], [34u8; 32]);
    let tc = vec![3u8; 32];
    let key = capture_key(&sc, &kh("carol"), &kh("bob"), &cc);
    let cap = Capture {
        modality: 0,
        template_version: 1,
        template: tc.clone(),
        frames: vec![Frame {
            at_ms: 1,
            bytes: b"a frame".to_vec(),
        }],
    };
    b_store.sealed.insert(
        cb.txid,
        seal(
            &SealParams::default(),
            &key,
            kh("carol"),
            kh("bob"),
            cc,
            &cap,
        ),
    );
    let cx = Verifying {
        seal: &SealParams::default(),
        me: &me,
        ids: &ids(),
        store: &b_store,
        matcher: &m,
    };

    let ceremony = [7u8; 32];
    // carol asks bob about alice, alice consenting
    let q1 = query(&s.t2, ceremony, "bob");
    assert_eq!(
        v.take_query(&cx, kh("carol"), &request(&q1, "alice", 1), 0),
        QueryOutcome::AwaitingGrant
    );
    let GrantOutcome::Answered(a1) =
        v.take_grant(&cx, kh("alice"), &grant(&s, s.r2, s.c2, q1.query_id()), 1)
    else {
        panic!("alice's side is answered")
    };
    assert_eq!(verdict_of(&a1.to_querier).0, Verdict::Match);

    // and alice asks bob about carol under the very same ceremony, carol
    // consenting: a second subject, not a second profile of the first
    let q2 = VerificationQuery {
        subject: kh("carol"),
        querier: kh("alice"),
        ..query(&tc, ceremony, "bob")
    };
    assert_eq!(
        v.take_query(&cx, kh("alice"), &request(&q2, "carol", 1), 2),
        QueryOutcome::AwaitingGrant,
        "the other subject's query is taken, not closed"
    );
    let g = KeyGrant {
        record: cb.txid,
        query_id: q2.query_id(),
        key: capture_key(&sc, &kh("carol"), &kh("bob"), &cc),
    }
    .encode()
    .to_vec();
    let GrantOutcome::Answered(a2) = v.take_grant(&cx, kh("carol"), &g, 3) else {
        panic!("carol's side is answered too")
    };
    assert_eq!(verdict_of(&a2.to_querier).0, Verdict::Match);

    // what the rule still refuses: a second, different profile of one of
    // those subjects in that ceremony
    let q3 = query(&s.t1, ceremony, "bob");
    assert_eq!(
        v.take_query(&cx, kh("carol"), &request(&q3, "alice", 1), 4),
        QueryOutcome::Closed("a second profile for one subject in one ceremony")
    );
}

// acceptance: CER-56
/// **One ceremony at a time may ask about a subject.** Two ceremonies
/// holding a probe on one subject at once is the oracle leakage the rule
/// exists for [author, 2026-10-13], and it is not the same thing as the
/// profile pin: the pin stops several schemas under one pre-commitment,
/// this stops concurrent pre-commitments multiplying what one ceremony's
/// price buys. Successive ceremonies are not touched — §7.4.1 prices
/// those at one probe each — so the lock lifts when the question is
/// answered, either way.
#[test]
fn only_one_ceremony_at_a_time_may_ask_about_a_subject() {
    let s = setup();
    let me = id("bob");
    let m = Recording(RefCell::new(vec![]));
    let mut v = VerifierState::new(VerifierConfig::default());
    let cx = Verifying {
        seal: &SealParams::default(),
        me: &me,
        ids: &ids(),
        store: &s.b_store,
        matcher: &m,
    };
    // carol asks about alice and the query waits for its grant
    let q1 = query(&s.t2, [1; 32], "bob");
    assert_eq!(
        v.take_query(&cx, kh("carol"), &request(&q1, "alice", 1), 0),
        QueryOutcome::AwaitingGrant
    );
    // another ceremony about the same subject, while that one is open:
    // refused, whoever asks and whatever profile it carries
    let q2 = query(&s.t1, [2; 32], "bob");
    assert_eq!(
        v.take_query(&cx, kh("carol"), &request(&q2, "alice", 1), 1),
        QueryOutcome::Closed("another ceremony is verifying this subject")
    );
    let q3 = VerificationQuery {
        querier: kh("w1"),
        ..query(&s.t2, [2; 32], "bob")
    };
    assert_eq!(
        v.take_query(&cx, kh("w1"), &request(&q3, "alice", 1), 2),
        QueryOutcome::Closed("another ceremony is verifying this subject"),
        "another querier's ceremony is refused too, and the same profile \
         under another pre-commitment is still a second probe"
    );
    assert!(m.0.borrow().is_empty(), "nothing was compared for either");

    // the first question is answered, and the next ceremony may ask
    let GrantOutcome::Answered(_) =
        v.take_grant(&cx, kh("alice"), &grant(&s, s.r2, s.c2, q1.query_id()), 3)
    else {
        panic!("answered")
    };
    assert_eq!(
        v.take_query(&cx, kh("carol"), &request(&q2, "alice", 1), 4),
        QueryOutcome::AwaitingGrant,
        "successive ceremonies are priced at a ceremony each, not refused"
    );
    // and that second ceremony's own pin holds within itself
    let q4 = query(&s.t1, [2; 32], "bob");
    assert_eq!(q4.query_id(), q2.query_id(), "the same query repeated");
    let other_profile = query(&s.t2, [2; 32], "bob");
    assert_eq!(
        v.take_query(&cx, kh("carol"), &request(&other_profile, "alice", 1), 5),
        QueryOutcome::Closed("a second profile for one subject in one ceremony")
    );
}

// acceptance: CER-08
/// **The length the store seals with is the length the verifier reads
/// with.** A sealed capture is `template[template_len] || frames`, and the
/// length is not in the record: whoever opens it splits at the number it
/// holds. The verifier held a second copy of those parameters that nothing
/// kept in step, so a 512-byte template was written by the capture path
/// and split at 32 here, the rest read as frame headers — every
/// version-2 capture came back as one that would not open, which the
/// bench showed as `inconclusive` against a verifier that held a good
/// capture of the person in front of the camera [2026-10-12].
#[test]
fn a_capture_sealed_at_the_stores_template_length_opens_at_the_verifier() {
    let mut s = setup();
    let me = id("bob");
    let m = Recording(RefCell::new(vec![]));
    // the engine's numbers: a 512-byte template, compared under version 2
    let p512 = SealParams { template_len: 512 };
    let rec = s.w.meet("alice", "bob");
    let (cid, seed) = ([44u8; 32], [45u8; 32]);
    let template = vec![7u8; 512];
    let key = capture_key(&seed, &kh("alice"), &kh("bob"), &cid);
    let cap = Capture {
        modality: 0,
        template_version: 2,
        template: template.clone(),
        frames: vec![Frame {
            at_ms: 9,
            bytes: b"a frame of the person".to_vec(),
        }],
    };
    let mut b_store = s.b_store.clone();
    b_store.records.insert(rec.txid, rec.bytes.clone());
    b_store.sealed.insert(
        rec.txid,
        seal(&p512, &key, kh("alice"), kh("bob"), cid, &cap),
    );
    let cfg = VerifierConfig {
        template_versions: BTreeSet::from([2]),
        ..VerifierConfig::default()
    };
    let mut v = VerifierState::new(cfg);
    let cx = Verifying {
        seal: &p512,
        me: &me,
        ids: &ids(),
        store: &b_store,
        matcher: &m,
    };
    // the profile is that template, so the recording matcher calls it a
    // match if and only if what it was shown is what was sealed
    let q = VerificationQuery {
        template_version: 2,
        ..query(&template, [8; 32], "bob")
    };
    assert_eq!(
        v.take_query(&cx, kh("carol"), &request(&q, "alice", 1), 0),
        QueryOutcome::AwaitingGrant
    );
    let g = KeyGrant {
        record: rec.txid,
        query_id: q.query_id(),
        key,
    }
    .encode()
    .to_vec();
    let GrantOutcome::Answered(a) = v.take_grant(&cx, kh("alice"), &g, 1) else {
        panic!("answered")
    };
    assert_eq!(
        verdict_of(&a.to_querier),
        (Verdict::Match, Some(Basis::PhotoMatch), Some(2)),
        "the capture opened and compared"
    );
    assert_eq!(
        m.0.borrow().as_slice(),
        &[template],
        "and what the matcher was shown is the whole template, not a prefix"
    );
}

// acceptance: CER-15
/// **A capture taken under another template version is unavailable, and is
/// not opened.** The verifier here compares under versions 1 and 2 both,
/// holds a version-1 capture, and is asked under version 2: the answer
/// carries no basis and no version (VER-012), the old template is not
/// reinterpreted under the new matcher (VER-013), and `inconclusive` —
/// which names a capture that would not read — is not what this produces.
/// On the bench the cross-version comparison answered inconclusive
/// [2026-10-12].
#[test]
fn a_capture_of_another_template_version_is_unavailable_and_unopened() {
    let s = setup();
    let me = id("bob");
    let m = Recording(RefCell::new(vec![]));
    let cfg = VerifierConfig {
        template_versions: BTreeSet::from([1, 2]),
        ..VerifierConfig::default()
    };
    let mut v = VerifierState::new(cfg);
    let cx = Verifying {
        seal: &SealParams::default(),
        me: &me,
        ids: &ids(),
        store: &s.b_store,
        matcher: &m,
    };
    let q = VerificationQuery {
        template_version: 2,
        ..query(&s.t2, [7; 32], "bob")
    };
    assert_eq!(
        v.take_query(&cx, kh("carol"), &request(&q, "alice", 1), 0),
        QueryOutcome::AwaitingGrant,
        "the version it declares is one this verifier compares under"
    );
    let GrantOutcome::Answered(a) =
        v.take_grant(&cx, kh("alice"), &grant(&s, s.r2, s.c2, q.query_id()), 1)
    else {
        panic!("answered")
    };
    assert_eq!(
        verdict_of(&a.to_querier),
        (Verdict::Unavailable, None, None),
        "unavailable, with no comparison basis and no version"
    );
    assert!(
        m.0.borrow().is_empty(),
        "the version-1 template was never shown to the version-2 matcher"
    );
}

// acceptance: CER-14
#[test]
fn a_copy_of_the_signed_response_goes_to_the_subject() {
    let s = setup();
    let me = id("bob");
    let m = Recording(RefCell::new(vec![]));
    let mut v = VerifierState::new(VerifierConfig::default());
    let cx = Verifying {
        seal: &SealParams::default(),
        me: &me,
        ids: &ids(),
        store: &s.b_store,
        matcher: &m,
    };
    let q = query(&s.t2, [7; 32], "bob");
    assert_eq!(
        v.take_query(&cx, kh("carol"), &request(&q, "alice", 1), 0),
        QueryOutcome::AwaitingGrant
    );
    let GrantOutcome::Answered(a) =
        v.take_grant(&cx, kh("alice"), &grant(&s, s.r2, s.c2, q.query_id()), 1)
    else {
        panic!("answered")
    };
    assert_eq!(a.to_subject.0, kh("alice"));
    assert_eq!(
        a.to_subject.1, a.to_querier,
        "identical bytes to C and to A"
    );
    let _ = s.w;
    let _: BTreeSet<u8> = BTreeSet::new();
}
