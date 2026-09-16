//! Recovery on the harness (design §9.1; `wire-format.md` §4.1): the
//! subject, holding its prior key, meets prior counterparties with its new
//! key; each is its own querier and recognises; the patron adopts on the
//! block; the old lines are sealed as the old key's last act.

mod common;

use common::harness::*;
use common::*;
use rhtn_archive::locator::{COUNTER_MAX, SignedLocator};
use rhtn_archive::record::Record;

use rhtn_client::ceremony::*;
use rhtn_client::device::ChannelKind;
use rhtn_client::query::{Basis, Response, Verdict, prior_key_of};
use rhtn_client::rotation::Rotation;
use rhtn_codec::cbor::*;
use rhtn_crypto::verify;

const PARTIES: [&str; 6] = ["alice", "alice2", "bob", "carol", "w1", "w2"];

/// alice meets bob and carol and is adopted under w1; then alice2, holding
/// alice's key and archive, recovers: meetings with bob and carol, adoption
/// under w1.  Returns the harness, the txid, and the log index where the
/// adoption's messages begin.
fn recovered() -> (Setup, [u8; 32], usize) {
    let mut s = setup(&PARTIES, &[ChannelKind::Nfc]);
    s.run("alice", "bob", &["w2"], &[]).unwrap();
    s.run("alice", "carol", &["w2"], &[]).unwrap();
    s.face_off("alice", "w1");
    s.h.run_adoption(kh("alice"), kh("w1"), vec![kh("w2")], vec![], 3).expect("alice adopted under w1");
    // alice's device is alice2's now: the prior key and its archive
    let old_archive = s.client("alice").archive.clone();
    s.client("alice2").rotation = Some(Box::new(Rotation::begin(Box::new(id("alice")), old_archive)));
    s.face_off("alice2", "bob");
    s.h.run_recovery_meeting(kh("alice2"), kh("bob")).expect("bob recognises");
    s.face_off("alice2", "carol");
    s.h.run_recovery_meeting(kh("alice2"), kh("carol")).expect("carol recognises");
    let at = s.h.log.len();
    let t = s.h.run_recovery_adoption(kh("alice2"), kh("w1"), 7).expect("adopted");
    (s, t, at)
}

fn responses_of(body: &[u8]) -> Vec<Vec<u8>> {
    let r6 = value_slice(body, 6).expect("field 6");
    let r2 = value_slice_at(body, r6.start, 2).expect("field 6.2");
    array_item_ranges(body, r2.start).unwrap().into_iter().map(|r| body[r].to_vec()).collect()
}

// acceptance: REC-01
#[test]
fn the_recovery_adoption_carries_both_halves_and_no_other_evidence_field() {
    let (mut s, t, at) = recovered();
    let env = s.client("w1").store.records[&t].clone();
    let rec = Record::parse(&env).unwrap();
    let body = &env[rec.body.clone()];
    assert!(value_slice(body, 6).is_some() && value_slice(body, 8).is_none() && value_slice(body, 9).is_none(), "field 6 and neither 8 nor 9");
    assert_eq!(rec.prior_key(), Some(kh("alice")));
    // responses sorted ascending by verifier keyhash, and a match among them
    let resps = responses_of(body);
    let verifiers: Vec<_> = resps.iter().map(|r| Response::read(r).unwrap().verifier).collect();
    assert_eq!(verifiers.len(), 2);
    assert!(verifiers.windows(2).all(|w| w[0] < w[1]), "ascending");
    assert!(resps.iter().any(|r| Response::read(r).unwrap().verdict == Verdict::Match));
    // field 6.3: a hybrid COSE_Sign by k1 over [k1, k2, P] under the successor tag
    let r6 = value_slice(body, 6).unwrap();
    let r3 = value_slice_at(body, r6.start, 3).unwrap();
    let item = parse_all(&body[r3.clone()]).unwrap();
    let Item::Array(cs) = &item else { panic!("not COSE_Sign") };
    let Item::Array(entries) = &cs[3] else { panic!() };
    assert_eq!(entries.len(), 2, "one Ed25519 and one ML-DSA-65 entry");
    // the corpus validator accepts the envelope
    verify::envelope(&ids(), &env).expect("verifies, successor statement included");
    // and a statement lifted onto another successor does not
    let mut k1 = env.clone();
    assert!(verify::adoption_evidence(&ids(), &k1[rec.body.clone()]).is_ok());
    let alice2 = kh("alice2");
    let pos = body.windows(32).position(|w| w == alice2).unwrap();
    let body_start = rec.body.start;
    k1[body_start + pos..body_start + pos + 32].copy_from_slice(&kh("w2"));
    assert!(verify::adoption_evidence(&ids(), &k1[rec.body.clone()]).is_err(), "names another successor");
    // the subject holds the record too, and so does the corpus's worked example
    assert!(s.client("alice2").store.records.contains_key(&t));
    verify::envelope(&ids(), &fixture("P-recovery-adoption")).expect("the corpus's recovery adoption verifies here");
    let _ = at;
}

// acceptance: REC-03
#[test]
fn a_recovery_response_is_signed_hybrid_by_the_verifier_and_its_consent_classically_by_the_new_key() {
    let (mut s, t, _) = recovered();
    let env = s.client("w1").store.records[&t].clone();
    let rec = Record::parse(&env).unwrap();
    for bytes in responses_of(&env[rec.body.clone()]) {
        let r = Response::read(&bytes).unwrap();
        // field 9: a detached COSE_Sign with two entries, both under V
        let r9 = value_slice(&bytes, 9).unwrap();
        let item = parse_all(&bytes[r9.clone()]).unwrap();
        let Item::Array(cs) = &item else { panic!("field 9 not an array") };
        assert!(matches!(&cs[2], Item::Null), "detached");
        let Item::Array(entries) = &cs[3] else { panic!() };
        assert_eq!(entries.len(), 2);
        verify::response(&ids(), &bytes, true).expect("both entries verify under V");
        assert!(verify::response(&ids(), &bytes, false).is_err(), "not a classical Sign1");
        // field 7: one classical Sign1 by S's new key
        let r7 = value_slice(&bytes, 7).unwrap();
        let citem = parse_all(&bytes[r7.clone()]).unwrap();
        let Item::Array(c) = &citem else { panic!() };
        assert_eq!(c.len(), 4);
        assert!(matches!(&c[3], Item::Bytes(_)), "a single signature, not entries");
        assert!(rhtn_client::query::consent_verifies(&id("alice2").public, &bytes[r7.clone()], &r.query_id));
        assert!(!rhtn_client::query::consent_verifies(&id("alice").public, &bytes[r7], &r.query_id), "not the old key");
        assert_eq!((r.verdict, r.basis, r.template_version), (Verdict::Match, Some(Basis::PersonalKnowledge), None));
    }
}

// acceptance: REC-04
#[test]
fn the_recovery_meeting_is_a_ceremony_in_which_the_verifier_is_its_own_querier() {
    let mut s = setup(&PARTIES, &[ChannelKind::Nfc]);
    s.run("alice", "bob", &["w2"], &[]).unwrap();
    let old_archive = s.client("alice").archive.clone();
    s.client("alice2").rotation = Some(Box::new(Rotation::begin(Box::new(id("alice")), old_archive)));
    s.face_off("alice2", "bob");
    let before = s.h.log.len();
    let resp = s.h.run_recovery_meeting(kh("alice2"), kh("bob")).unwrap();
    let log = &s.h.log[before..];
    // the ceremony opened: contributions crossed, channels ran, captures were sealed
    assert!(log.iter().any(|m| matches!(m.msg, Msg::Intent(_))));
    assert!(log.iter().any(|m| matches!(m.msg, Msg::CaptureKey(_))));
    // V generated the query: field 2 names V, addressed to V, under the meeting's own pre-commitment
    let q = log.iter().find_map(|m| if let Msg::ConsentRequest(q) = &m.msg { Some(q.clone()) } else { None }).expect("a query");
    assert_eq!((q.querier, q.verifier, q.subject), (kh("bob"), kh("bob"), kh("alice2")));
    let intents: Vec<_> = log.iter().filter_map(|m| if let Msg::Intent(i) = &m.msg { Some((m.from, i.contribution)) } else { None }).collect();
    let (ca, cb) = (intents.iter().find(|(f, _)| *f == kh("alice2")).unwrap().1, intents.iter().find(|(f, _)| *f == kh("bob")).unwrap().1);
    assert_eq!(q.ceremony_id, rhtn_client::keys::pre_commitment((&kh("alice2"), &ca), (&kh("bob"), &cb)), "the meeting's pre-commitment");
    // S's new key countersigned it
    let consent = log.iter().find_map(|m| if let Msg::Consent { consent, .. } = &m.msg { Some(consent.clone()) } else { None }).unwrap();
    assert!(rhtn_client::query::consent_verifies(&id("alice2").public, &consent, &q.query_id()));
    // V's response: selection basis 0, field 8 the prior key, about the new key
    let r = Response::read(&resp).unwrap();
    assert_eq!((r.selection_basis, r.subject, r.verifier, r.query_id), (0, kh("alice2"), kh("bob"), q.query_id()));
    assert_eq!(prior_key_of(&resp), Some(kh("alice")));
    // the meeting yielded no record
    assert_eq!(s.client("bob").archive.len(), 1);
    assert_eq!(s.client("alice2").archive.len(), 0);
    // the person recognised: the one question asked at the meeting
    assert!(s.prompts_asked("bob").iter().any(|q| q.starts_with("Do you recognise")));
    // and a verifier who never met the prior key asks nobody and answers nothing
    s.face_off("alice2", "carol");
    assert_eq!(s.h.run_recovery_meeting(kh("alice2"), kh("carol")), Err(Abort::NotRecognised));
    assert!(s.prompts_asked("carol").iter().all(|q| !q.starts_with("Do you recognise")));
}

#[test]
fn the_old_lines_are_sealed_before_the_recovery_adoption_goes_anywhere() {
    let (s, _t, at) = recovered();
    let log = &s.h.log[at..];
    let seal_at = log.iter().position(|m| matches!(m.msg, Msg::Seal(_))).expect("a seal");
    let record_at = log.iter().position(|m| matches!(m.msg, Msg::Record(_))).expect("the record");
    let proposal_at = log.iter().position(|m| matches!(m.msg, Msg::RecoveryProposal { .. })).unwrap();
    assert!(proposal_at < seal_at && seal_at < record_at, "statement, then seals, then the adoption");
    let Msg::Seal(seal) = &log[seal_at].msg else { unreachable!() };
    let sl = SignedLocator::parse(seal).unwrap();
    assert_eq!((sl.subject, sl.locator.seqno.counter, log[seal_at].to), (kh("alice"), COUNTER_MAX, kh("w1")));
    assert_eq!(s.h.clients[&kh("alice2")].prior_key(), None, "nothing signed by the old key follows");
}
