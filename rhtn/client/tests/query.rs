//! The query objects against the corpus: the worked query and its frame,
//! the key grant and the late response, and the responses a record carries.

mod common;

use common::*;
use rhtn_client::query::*;
use rhtn_codec::cbor::*;
use rhtn_crypto::verify;

#[test]
fn the_worked_query_and_its_frame_decode_and_the_consent_verifies() {
    // the standalone query fixture: its id recomputes; it is the recovery
    // ceremony's query about alice2, addressed to carol
    let q = VerificationQuery::decode(&fixture("P-verification-query")).unwrap();
    assert_eq!(q.subject, kh("alice2"));
    assert_eq!(q.verifier, kh("carol"));
    assert_eq!(q.encode(), fixture("P-verification-query"), "re-encodes to the vector");
    // the type-4 frame: [4, [query, consent, basis]] behind a length prefix
    let frame = fixture("P-frame-13");
    let payload = &frame[4..];
    let outer = array_item_ranges(payload, 0).unwrap();
    let (t, _) = Parser { b: payload }.item(outer[0].start).unwrap();
    assert_eq!(as_uint(&t), Some(REQUEST_VERIFIER_QUERY));
    let req = QueryRequest::decode(&payload[outer[1].clone()]).unwrap();
    assert!(consent_verifies(&id("alice").public, &req.consent, &req.query.query_id()), "alice's consent over the id");
    assert!(!consent_verifies(&id("bob").public, &req.consent, &req.query.query_id()));
    let mut other = req.query.query_id();
    other[0] ^= 1;
    assert!(!consent_verifies(&id("alice").public, &req.consent, &other));
    assert!(req.selection_basis <= 2);
    assert_eq!(req.encode(), payload[outer[1].clone()].to_vec(), "re-encodes to the vector");
    // a consent minted here verifies the same way
    let mine = consent(&id("carol"), &req.query.query_id());
    assert!(consent_verifies(&id("carol").public, &mine, &req.query.query_id()));
}

#[test]
fn the_grant_and_the_late_response_decode_and_round_trip() {
    let g = KeyGrant::decode(&fixture("P-e2e-01")).unwrap();
    assert_eq!(hex::encode(g.key), "6157379db20e9b35da24fbab9ab4c8bc8676dc8f8c22c89d2b1fe2fea7b2985c", "the known capture key");
    assert_eq!(g.record, rhtn_codec::cose::txid(&body_of(&fixture("P-alice-c1-record"))), "names the prior alice-c1 record");
    assert_eq!(g.encode(), fixture("P-e2e-01"));
    let l = LateResponse::decode(&fixture("P-e2e-02")).unwrap();
    assert_eq!(l.record, rhtn_codec::cose::txid(&body_of(&fixture("P-normal-record"))), "supplements the normal record");
    let r = Response::read(&l.response).unwrap();
    assert_eq!(r.subject, l.subject);
    assert_eq!(r.verdict, Verdict::Inconclusive);
    verify::response(&ids(), &l.response, false).expect("the late response verifies under its verifier");
    assert_eq!(l.encode(), fixture("P-e2e-02"));
}

#[test]
fn a_response_signed_here_reads_back_and_verifies_like_the_records() {
    // every response the normal record carries reads and verifies
    let body = body_of(&fixture("P-normal-record"));
    let r5 = value_slice(&body, 5).unwrap();
    let mut count = 0;
    for rr in array_item_ranges(&body, r5.start).unwrap() {
        let r = Response::read(&body[rr.clone()]).unwrap();
        assert!(matches!(r.verdict, Verdict::Match | Verdict::NoMatch | Verdict::Inconclusive | Verdict::Unavailable));
        verify::response(&ids(), &body[rr], false).unwrap();
        count += 1;
    }
    assert_eq!(count, 3, "the normal record carries three responses");
    // and one minted here: a photo match with a template version, consent by the subject
    let q = VerificationQuery { subject: kh("alice"), querier: kh("carol"), ceremony_id: [9; 32], profile: vec![1, 2, 3], template_version: 7, verifier: kh("bob") };
    let c = consent(&id("alice"), &q.query_id());
    let resp = Response { verifier: kh("bob"), subject: kh("alice"), query_id: q.query_id(), verdict: Verdict::Match, basis: Some(Basis::PhotoMatch), template_version: Some(7), consent: c, selection_basis: 0 };
    let bytes = resp.sign(&id("bob"));
    assert_eq!(Response::read(&bytes).unwrap(), resp);
    verify::response(&ids(), &bytes, false).unwrap();
    assert!(verify::response(&vec![id("alice").public, id("carol").public], &bytes, false).is_err(), "not under another key");
    // unavailable carries no basis and no version
    let un = Response { verdict: Verdict::Unavailable, basis: None, template_version: None, ..resp.clone() };
    let bytes = un.sign(&id("bob"));
    assert_eq!(Response::read(&bytes).unwrap(), un);
    verify::response(&ids(), &bytes, false).unwrap();
    // inconclusive after a failed decryption: basis 0 and the query's version
    let inc = Response { verdict: Verdict::Inconclusive, basis: Some(Basis::PhotoMatch), template_version: Some(7), ..resp.clone() };
    verify::response(&ids(), &inc.sign(&id("bob")), false).unwrap();
}
