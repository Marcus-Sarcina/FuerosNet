//! The two decoder entries that need constructed objects with real
//! signatures: the derived envelope ceiling (DEC-05) and three objects at a
//! §1.3 ceiling no fixture reaches (DEC-06).  Identities come from the
//! test-vector keygen recipe; extra verifiers are minted under fresh names.

mod common;
use common::*;
use rhtn_codec::bounds;
use rhtn_codec::cbor::*;
use rhtn_codec::cose::{self, aad};
use rhtn_codec::encode::*;
use rhtn_codec::envelope;
use rhtn_crypto::identity::testkit::test_identity;
use rhtn_crypto::{Identity, SigningIdentity, verify};

fn signers() -> Vec<SigningIdentity> {
    let mut v: Vec<SigningIdentity> = NAMES.iter().map(|n| test_identity(n)).collect();
    for i in 1..=32 {
        v.push(test_identity(&format!("v{i:02}")));
    }
    v
}

fn publics(s: &[SigningIdentity]) -> Vec<Identity> {
    s.iter().map(|x| x.public.clone()).collect()
}

fn by_name<'a>(s: &'a [SigningIdentity], name: &str) -> &'a SigningIdentity {
    let kh = test_identity(name).public.keyhash;
    s.iter().find(|x| x.public.keyhash == kh).unwrap()
}

fn by_keyhash<'a>(s: &'a [SigningIdentity], kh: &[u8]) -> &'a SigningIdentity {
    s.iter()
        .find(|x| x.public.keyhash == kh)
        .expect("a test identity")
}

/// `{1: 1, 2: type, 3: body, 4: [h'', {}, null, [entries]]}`.
fn build_envelope(tx_type: u64, body: &[u8], signers: &[&SigningIdentity]) -> Vec<u8> {
    // entries in the canonical order (`wire-format.md` §3.5): by kid, each
    // signer's classical entry before its post-quantum one
    let mut signers: Vec<&SigningIdentity> = signers.to_vec();
    signers.sort_by_key(|s| s.public.keyhash);
    let mut out = Vec::new();
    emit_map_head(&mut out, 4);
    emit_uint(&mut out, 1);
    emit_uint(&mut out, 1);
    emit_uint(&mut out, 2);
    emit_uint(&mut out, tx_type);
    emit_uint(&mut out, 3);
    out.extend_from_slice(body);
    emit_uint(&mut out, 4);
    emit_array_head(&mut out, 4);
    emit_bstr(&mut out, b"");
    emit_map_head(&mut out, 0);
    emit_null(&mut out);
    emit_array_head(&mut out, signers.len() * 2);
    for s in signers {
        out.extend_from_slice(&s.sign_entries(aad::ENVELOPE, body));
    }
    out
}

fn kh_entry(out: &mut Vec<u8>, key: u64, kh: &[u8; 32]) {
    emit_uint(out, key);
    emit_bstr(out, kh);
}

/// A verifier response about `subject` by `verifier`: consent by the subject
/// over the query id, the verifier's signature over the map minus field 9,
/// classical or hybrid as the enclosing object requires.
fn build_response(
    verifier: &SigningIdentity,
    subject: &SigningIdentity,
    qid: &[u8; 32],
    prior: Option<&[u8; 32]>,
    hybrid: bool,
) -> Vec<u8> {
    build_response_with(verifier, subject, qid, prior, hybrid, false)
}

/// A verifier response; with `named`, its embedded signatures carry a
/// `kid` the enclosing structure already supplies, which `wire-format.md`
/// §3.5 makes malformed.
fn build_response_with(
    verifier: &SigningIdentity,
    subject: &SigningIdentity,
    qid: &[u8; 32],
    prior: Option<&[u8; 32]>,
    hybrid: bool,
    named: bool,
) -> Vec<u8> {
    let consent = if named {
        subject.sign1_ed(aad::CONSENT, qid)
    } else {
        subject.sign1_ed_unnamed(aad::CONSENT, qid)
    };
    let mut payload = Vec::new();
    emit_map_head(&mut payload, if prior.is_some() { 8 } else { 7 });
    kh_entry(&mut payload, 1, &verifier.public.keyhash);
    kh_entry(&mut payload, 2, &subject.public.keyhash);
    kh_entry(&mut payload, 3, qid);
    emit_uint(&mut payload, 4);
    emit_uint(&mut payload, 0);
    emit_uint(&mut payload, 5);
    emit_uint(&mut payload, 1);
    emit_uint(&mut payload, 7);
    payload.extend_from_slice(&consent);
    if let Some(p) = prior {
        kh_entry(&mut payload, 8, p);
    }
    emit_uint(&mut payload, 10);
    emit_uint(&mut payload, 0);
    let sig9 = if hybrid {
        let mut s = Vec::new();
        emit_array_head(&mut s, 4);
        emit_bstr(&mut s, b"");
        emit_map_head(&mut s, 0);
        emit_null(&mut s);
        emit_array_head(&mut s, 2);
        s.extend_from_slice(&if named {
            verifier.sign_entries(aad::VERIFIER, &payload)
        } else {
            verifier.sign_entries_unnamed(aad::VERIFIER, &payload)
        });
        s
    } else if named {
        verifier.sign1_ed(aad::VERIFIER, &payload)
    } else {
        verifier.sign1_ed_unnamed(aad::VERIFIER, &payload)
    };
    // insert field 9 before field 10, bumping the head count
    let r10 = value_slice(&payload, 10).unwrap();
    let key10_at = r10.start - 1;
    let mut out = Vec::new();
    emit_map_head(&mut out, if prior.is_some() { 9 } else { 8 });
    let (_, _, adv) = (Parser { b: &payload }).head(0).unwrap();
    out.extend_from_slice(&payload[adv..key10_at]);
    emit_uint(&mut out, 9);
    out.extend_from_slice(&sig9);
    out.extend_from_slice(&payload[key10_at..]);
    out
}

/// A normal presence record body: back-pointer lists per signer, the
/// fixture's timestamps and root, the given participants, witnesses and
/// responses.
fn presence_body(
    fixture_body: &[u8],
    participants: [&SigningIdentity; 2],
    witnesses: &[(&SigningIdentity, &SigningIdentity)],
    responses: &[Vec<u8>],
) -> Vec<u8> {
    let r0 = value_slice(fixture_body, 0).unwrap();
    let first_list =
        fixture_body[array_item_ranges(fixture_body, r0.start).unwrap()[0].clone()].to_vec();
    let val = |k: u64| fixture_body[value_slice(fixture_body, k).unwrap()].to_vec();
    let mut b = Vec::new();
    emit_map_head(&mut b, if responses.is_empty() { 7 } else { 8 });
    emit_uint(&mut b, 0);
    emit_array_head(&mut b, 2 + witnesses.len());
    for _ in 0..2 + witnesses.len() {
        b.extend_from_slice(&first_list);
    }
    emit_uint(&mut b, 1);
    b.extend_from_slice(&val(1));
    emit_uint(&mut b, 2);
    b.extend_from_slice(&val(2));
    emit_uint(&mut b, 3);
    emit_array_head(&mut b, 2);
    for p in participants {
        emit_map_head(&mut b, 1);
        kh_entry(&mut b, 1, &p.public.keyhash);
    }
    emit_uint(&mut b, 4);
    emit_array_head(&mut b, witnesses.len());
    for (w, nominated_by) in witnesses {
        emit_map_head(&mut b, 3);
        kh_entry(&mut b, 1, &w.public.keyhash);
        kh_entry(&mut b, 2, &nominated_by.public.keyhash);
        emit_uint(&mut b, 3);
        emit_uint(&mut b, 7);
    }
    if !responses.is_empty() {
        emit_uint(&mut b, 5);
        emit_array_head(&mut b, responses.len());
        for r in responses {
            b.extend_from_slice(r);
        }
    }
    emit_uint(&mut b, 6);
    emit_uint(&mut b, 0);
    emit_uint(&mut b, 8);
    b.extend_from_slice(&val(8));
    b
}

fn sort_responses(mut rs: Vec<Vec<u8>>) -> Vec<Vec<u8>> {
    let key = |r: &[u8]| {
        let v = value_slice(r, 1).unwrap();
        let s = value_slice(r, 2).unwrap();
        (r[v].to_vec(), r[s].to_vec())
    };
    rs.sort_by_key(|r| key(r));
    rs
}

// acceptance: DEC-05
#[test]
fn dec_05_presence_record_at_the_derived_ceiling_of_36_entries_is_accepted() {
    let s = signers();
    let ids = publics(&s);
    let alice = by_name(&s, "alice");
    let bob = by_name(&s, "bob");
    let ws: Vec<&SigningIdentity> = (1..=16).map(|i| by_name(&s, &format!("w{i}"))).collect();
    let witnesses: Vec<(&SigningIdentity, &SigningIdentity)> = ws
        .iter()
        .enumerate()
        .map(|(i, w)| (*w, if i % 2 == 0 { alice } else { bob }))
        .collect();
    let fixture_body = body_of(&fixture("P-alice-c1-record").bytes);
    let body = presence_body(&fixture_body, [alice, bob], &witnesses, &[]);
    let mut all: Vec<&SigningIdentity> = vec![alice, bob];
    all.extend(ws.iter().copied());
    let env = build_envelope(5, &body, &all);
    let parsed = verify::envelope(&ids, &env).expect("36-entry envelope verifies");
    assert_eq!(parsed.entries.len(), 36);
    assert_eq!(bounds::envelope_entry_ceiling(5), Some(36));
    assert_eq!(parsed.signers.len(), 18);
    // one more witness would exceed the derived ceiling, at the body first
    let w17 = test_identity("v01");
    let mut more = witnesses.clone();
    more.push((&w17, alice));
    let body17 = presence_body(&fixture_body, [alice, bob], &more, &[]);
    assert!(rhtn_codec::schema::check_body(&body17, &parse_all(&body17).unwrap()).is_err());
    let mut all17 = all.clone();
    all17.push(&w17);
    assert!(envelope::parse(&build_envelope(5, &body17, &all17)).is_err());
}

// acceptance: DEC-06
#[test]
fn dec_06_objects_exactly_at_a_ceiling_no_fixture_reaches_are_accepted() {
    let s = signers();
    let ids = publics(&s);

    // (a) a Capabilities value of exactly 1024 bytes in an Attach
    let payload = fixture("P-frame-01").bytes[4..].to_vec();
    let body_at = array_item_ranges(&payload, 0).unwrap()[1].start;
    let caps = |n: usize| {
        let mut c = Vec::new();
        emit_map_head(&mut c, 1);
        emit_uint(&mut c, 0x2dfe_1d3f_8c17_a01f);
        emit_bstr(&mut c, &vec![0u8; n]);
        c
    };
    assert!(
        parse_frame(&framed(&replace_value(&payload, body_at, 3, &caps(1024)))).is_ok(),
        "1024-byte value"
    );
    assert!(
        parse_frame(&framed(&replace_value(&payload, body_at, 3, &caps(1025)))).is_err(),
        "1025-byte value"
    );

    // (b) a presence record with 32 responses, 16 per subject
    let alice = by_name(&s, "alice");
    let bob = by_name(&s, "bob");
    let w1 = by_name(&s, "w1");
    let verifiers: Vec<&SigningIdentity> =
        (1..=32).map(|i| by_name(&s, &format!("v{i:02}"))).collect();
    let qid = |label: &str| cose::sha256(label.as_bytes());
    let mut rs = Vec::new();
    for (i, v) in verifiers.iter().enumerate() {
        let subject = if i < 16 { alice } else { bob };
        rs.push(build_response(
            v,
            subject,
            &qid(&format!("q{i}")),
            None,
            false,
        ));
    }
    let rs = sort_responses(rs);
    let fixture_body = body_of(&fixture("P-alice-c1-record").bytes);
    let body = presence_body(&fixture_body, [alice, bob], &[(w1, alice)], &rs);
    let env = build_envelope(5, &body, &[alice, bob, w1]);
    verify::envelope(&ids, &env).expect("32 responses verify");
    let mut rs33 = rs.clone();
    rs33.push(build_response(
        by_name(&s, "carol"),
        alice,
        &qid("q33"),
        None,
        false,
    ));
    let body33 = presence_body(
        &fixture_body,
        [alice, bob],
        &[(w1, alice)],
        &sort_responses(rs33),
    );
    assert!(
        rhtn_codec::schema::check_body(&body33, &parse_all(&body33).unwrap()).is_err(),
        "33 responses"
    );

    // (c) a recovery adoption with 32 hybrid responses
    let rec = fixture("P-recovery-adoption").bytes;
    let body = body_of(&rec);
    let kh =
        |r: std::ops::Range<usize>| -> [u8; 32] { body[r.start + 2..r.end].try_into().unwrap() };
    let new_key = by_keyhash(&s, &kh(value_slice(&body, 1).unwrap()));
    let patron = by_keyhash(&s, &kh(value_slice(&body, 2).unwrap()));
    let r6 = value_slice(&body, 6).unwrap();
    let prior_kh = kh(value_slice_at(&body, r6.start, 1).unwrap());
    let mut hs = Vec::new();
    for (i, v) in verifiers.iter().enumerate() {
        hs.push(build_response(
            v,
            new_key,
            &qid(&format!("r{i}")),
            Some(&prior_kh),
            true,
        ));
    }
    let hs = sort_responses(hs);
    let mut arr = Vec::new();
    emit_array_head(&mut arr, hs.len());
    for r in &hs {
        arr.extend_from_slice(r);
    }
    let recovery = replace_value(&body[r6.clone()], 0, 2, &arr);
    let body32 = replace_value(&body, 0, 6, &recovery);
    let env32 = build_envelope(1, &body32, &[new_key, patron]);
    verify::envelope(&ids, &env32).expect("recovery with 32 hybrid responses verifies");
    let mut hs33 = hs.clone();
    hs33.push(build_response(
        by_name(&s, "carol"),
        new_key,
        &qid("r33"),
        Some(&prior_kh),
        true,
    ));
    let mut arr33 = Vec::new();
    emit_array_head(&mut arr33, 33);
    for r in sort_responses(hs33) {
        arr33.extend_from_slice(&r);
    }
    let body33 = replace_value(&body, 0, 6, &replace_value(&body[r6.clone()], 0, 2, &arr33));
    assert!(
        rhtn_codec::schema::check_body(&body33, &parse_all(&body33).unwrap()).is_err(),
        "33 recovery responses"
    );
}

/// The envelope's signature entries in `order`, by index.
fn with_entries_in_order(env: &[u8], order: &[usize]) -> Vec<u8> {
    let r4 = value_slice(env, 4).unwrap();
    let outer = array_item_ranges(env, r4.start).unwrap();
    let ents = array_item_ranges(env, outer[3].start).unwrap();
    let mut out = env[..ents[0].start].to_vec();
    for &k in order {
        out.extend_from_slice(&env[ents[k].clone()]);
    }
    out.extend_from_slice(&env[ents.last().unwrap().end..]);
    out
}

/// Entry `i` with part `part` (0 protected, 1 unprotected, 2 signature)
/// replaced by `bytes`.
fn with_entry_part(env: &[u8], i: usize, part: usize, bytes: &[u8]) -> Vec<u8> {
    let r4 = value_slice(env, 4).unwrap();
    let outer = array_item_ranges(env, r4.start).unwrap();
    let ents = array_item_ranges(env, outer[3].start).unwrap();
    let parts = array_item_ranges(env, ents[i].start).unwrap();
    let mut out = env[..parts[part].start].to_vec();
    out.extend_from_slice(bytes);
    out.extend_from_slice(&env[parts[part].end..]);
    out
}

// acceptance: DEC-17
#[test]
fn dec_17_entries_out_of_canonical_order_or_with_extra_headers_are_rejected() {
    let s = signers();
    let ids = publics(&s);
    let env = fixture("P-adopt-min").bytes.clone();
    assert!(envelope::parse(&env).is_ok());
    // post-quantum before classical within one signer
    let swapped = with_entries_in_order(&env, &[1, 0, 2, 3]);
    assert!(
        envelope::parse(&swapped)
            .unwrap_err()
            .0
            .contains("canonical order")
    );
    // the second signer's group before the first's
    assert!(envelope::parse(&with_entries_in_order(&env, &[2, 3, 0, 1])).is_err());
    // one entry twice, and one missing: a duplicate at one place in the order
    assert!(envelope::parse(&with_entries_in_order(&env, &[0, 0, 2, 3])).is_err());
    // a nonempty unprotected header is malformed, not ignored
    let unprot = with_entry_part(&env, 0, 1, &[0xa1, 0x05, 0x00]);
    assert!(
        envelope::parse(&unprot)
            .unwrap_err()
            .0
            .contains("unprotected")
    );
    // a third protected header entry beyond alg and kid
    let parsed = envelope::parse(&env).unwrap();
    let mut prot = env[parsed.entries[0].protected.clone()].to_vec();
    prot[0] = 0xa3;
    prot.extend_from_slice(&[0x05, 0x00]);
    let mut wrapped = Vec::new();
    emit_bstr(&mut wrapped, &prot);
    let extra = with_entry_part(&env, 0, 0, &wrapped);
    assert!(
        envelope::parse(&extra)
            .unwrap_err()
            .0
            .contains("protected header")
    );
    // an embedded signature carrying a kid the enclosing structure already
    // supplies, in the verifier block and in the consent
    let (v, sub) = (by_name(&s, "alice"), by_name(&s, "bob"));
    let qid = [7u8; 32];
    assert!(verify::response(&ids, &build_response(v, sub, &qid, None, true), true).is_ok());
    assert!(verify::response(&ids, &build_response(v, sub, &qid, None, false), false).is_ok());
    assert!(
        verify::response(
            &ids,
            &build_response_with(v, sub, &qid, None, true, true),
            true
        )
        .is_err(),
        "a kid in an embedded signature"
    );
    assert!(
        verify::response(
            &ids,
            &build_response_with(v, sub, &qid, None, false, true),
            false
        )
        .is_err(),
        "a kid in a standalone consent"
    );
}

/// Item `i` of the COSE array at `at` in `b` replaced by `bytes`.
fn with_cose_item(b: &[u8], at: usize, i: usize, bytes: &[u8]) -> Vec<u8> {
    let parts = array_item_ranges(b, at).unwrap();
    let mut out = b[..parts[i].start].to_vec();
    out.extend_from_slice(bytes);
    out.extend_from_slice(&b[parts[i].end..]);
    out
}

// acceptance: DEC-19
#[test]
fn dec_19_a_cose_container_departing_from_the_profile_is_rejected() {
    let s = signers();
    let ids = publics(&s);
    let h00: [u8; 2] = [0x41, 0x00];

    // (a) the envelope's own COSE_Sign: empty protected, empty unprotected,
    // nil payload.  A payload carried, or a header filled, is malformed with
    // every signature untouched
    let env = fixture("P-adopt-min").bytes.clone();
    let r4 = value_slice(&env, 4).unwrap();
    assert!(envelope::parse(&env).is_ok());
    assert!(
        envelope::parse(&with_cose_item(&env, r4.start, 2, &h00))
            .unwrap_err()
            .0
            .contains("payload"),
        "payload present"
    );
    assert!(
        envelope::parse(&with_cose_item(&env, r4.start, 0, &h00))
            .unwrap_err()
            .0
            .contains("protected"),
        "outer protected filled"
    );
    assert!(
        envelope::parse(&with_cose_item(&env, r4.start, 1, &[0xa1, 0x05, 0x00]))
            .unwrap_err()
            .0
            .contains("unprotected"),
        "outer unprotected filled"
    );

    // (b) an embedded COSE_Sign: the former patron's transfer statement.
    // The body is re-signed by the node and patron each time, so what fails
    // is the block and not the envelope
    let xfer = fixture("P-transfer-adoption").bytes.clone();
    let body = body_of(&xfer);
    let kh =
        |r: std::ops::Range<usize>| -> [u8; 32] { body[r.start + 2..r.end].try_into().unwrap() };
    let node = by_keyhash(&s, &kh(value_slice(&body, 1).unwrap()));
    let patron = by_keyhash(&s, &kh(value_slice(&body, 2).unwrap()));
    let r9 = value_slice(&body, 9).unwrap();
    let block = body[value_slice_at(&body, r9.start, 2).unwrap()].to_vec();
    let rebuilt =
        |blk: &[u8]| build_envelope(1, &replace_value(&body, r9.start, 2, blk), &[node, patron]);
    verify::envelope(&ids, &rebuilt(&block)).expect("the fixture's block, re-enveloped, verifies");
    let outer = array_item_ranges(&block, 0).unwrap();
    let ents = array_item_ranges(&block, outer[3].start).unwrap();
    let (ed, pq) = (
        block[ents[0].clone()].to_vec(),
        block[ents[1].clone()].to_vec(),
    );
    let with_entries = |es: &[&[u8]]| {
        let mut out = block[..outer[3].start].to_vec();
        emit_array_head(&mut out, es.len());
        for e in es {
            out.extend_from_slice(e);
        }
        out
    };
    let empty = [0x84, 0x40, 0xa0, 0xf6, 0x80];
    let e = verify::envelope(&ids, &rebuilt(&empty))
        .unwrap_err()
        .to_string();
    assert!(e.contains("exactly two entries"), "an empty block: {e}");
    assert!(
        verify::envelope(&ids, &rebuilt(&with_entries(&[&ed])))
            .unwrap_err()
            .to_string()
            .contains("exactly two entries"),
        "a lone entry"
    );
    assert!(
        verify::envelope(&ids, &rebuilt(&with_entries(&[&ed, &ed])))
            .unwrap_err()
            .to_string()
            .contains("canonical order"),
        "one algorithm twice"
    );
    assert!(
        verify::envelope(&ids, &rebuilt(&with_entries(&[&pq, &ed])))
            .unwrap_err()
            .to_string()
            .contains("canonical order"),
        "post-quantum first"
    );
    assert!(
        verify::envelope(&ids, &rebuilt(&with_cose_item(&block, 0, 2, &h00)))
            .unwrap_err()
            .to_string()
            .contains("container"),
        "block payload present"
    );
    assert!(
        verify::envelope(&ids, &rebuilt(&with_cose_item(&block, 0, 0, &h00)))
            .unwrap_err()
            .to_string()
            .contains("container"),
        "block outer protected filled"
    );

    // (c) a standalone COSE_Sign1: the endpoint record's signature slot
    let er = fixture("P-endpointrecord").bytes.clone();
    assert_eq!(verify::record(&ids, "EndpointRecord", &er), Ok(()));
    let r4 = value_slice(&er, 4).unwrap();
    assert!(
        verify::record(
            &ids,
            "EndpointRecord",
            &with_cose_item(&er, r4.start, 2, &h00)
        )
        .is_err(),
        "sign1 payload present"
    );
}

// acceptance: DEC-24
#[test]
fn dec_24_the_public_verifier_refuses_what_the_record_parser_refuses() {
    let s = signers();
    let ids = publics(&s);
    let env = fixture("P-adopt-min").bytes.clone();
    let body = body_of(&env);
    let kh =
        |r: std::ops::Range<usize>| -> [u8; 32] { body[r.start + 2..r.end].try_into().unwrap() };
    let node = by_keyhash(&s, &kh(value_slice(&body, 1).unwrap()));
    let patron = by_keyhash(&s, &kh(value_slice(&body, 2).unwrap()));
    verify::envelope(&ids, &build_envelope(1, &body, &[node, patron]))
        .expect("the fixture's body, re-enveloped, verifies");
    // the same body without its evidence, genuinely signed: every
    // signature verifies, and the body rule refuses it first
    let bare = remove_key(&body, 0, 8);
    let e = verify::envelope(&ids, &build_envelope(1, &bare, &[node, patron]))
        .unwrap_err()
        .to_string();
    assert!(
        e.contains("body") && e.contains("exactly one of fields 6, 8 and 9"),
        "{e}"
    );
}

// acceptance: DEC-25
#[test]
fn dec_25_a_classical_signature_declaring_another_algorithm_is_refused() {
    let s = signers();
    let ids = publics(&s);
    // (a) a standalone record: the node's genuine Ed25519 signature over a
    // header that declares ML-DSA
    let er = fixture("P-endpointrecord").bytes.clone();
    let node_kh: [u8; 32] = {
        let r = value_slice(&er, 1).unwrap();
        er[r.start + 2..r.end].try_into().unwrap()
    };
    let node = by_keyhash(&s, &node_kh);
    let payload = map_without_key(&er, 4).unwrap();
    let prot = cose::protected_alg(cose::ALG_ML_DSA_65);
    let sig = node.sign_ed(&cose::sig_structure_sign1(&prot, aad::ENDPOINTS, &payload));
    let mut sign1 = Vec::new();
    emit_array_head(&mut sign1, 4);
    emit_bstr(&mut sign1, &prot);
    emit_map_head(&mut sign1, 0);
    emit_null(&mut sign1);
    emit_bstr(&mut sign1, &sig);
    assert_eq!(
        verify::record(&ids, "EndpointRecord", &er),
        Ok(()),
        "the fixture verifies"
    );
    let e = verify::record(&ids, "EndpointRecord", &replace_value(&er, 0, 4, &sign1))
        .unwrap_err()
        .to_string();
    assert!(e.contains("algorithm"), "{e}");
    // (b) a consent signature inside a verifier response, the same way
    let (verifier, subject) = (by_name(&s, "carol"), by_name(&s, "alice"));
    let qid = cose::sha256(b"dec-25");
    let resp = build_response(verifier, subject, &qid, None, false);
    verify::response(&ids, &resp, false).expect("the response verifies");
    let cprot = cose::protected_alg(cose::ALG_ML_DSA_65);
    let csig = subject.sign_ed(&cose::sig_structure_sign1(&cprot, aad::CONSENT, &qid));
    let mut consent = Vec::new();
    emit_array_head(&mut consent, 4);
    emit_bstr(&mut consent, &cprot);
    emit_map_head(&mut consent, 0);
    emit_null(&mut consent);
    emit_bstr(&mut consent, &csig);
    let e = verify::response(&ids, &replace_value(&resp, 0, 7, &consent), false)
        .unwrap_err()
        .to_string();
    assert!(e.contains("consent"), "{e}");
}

// acceptance: DEC-26
#[test]
fn a_presentation_reports_a_key_it_lacks_the_way_its_siblings_do() {
    // the three verifiers over signed objects report the same way: a key
    // the holder does not have is that key's absence, named, and not a
    // failure of the object (`wire-format.md` §3.4)
    let pres = fixture("P-presented-minimal").bytes;
    let held = identities();
    assert_eq!(
        verify::presentation(&held, &pres),
        Ok(()),
        "it verifies under the keys it names"
    );
    // held by nobody: the envelope inside it names a signer, and that is
    // what comes back
    let none: Vec<Identity> = Vec::new();
    match verify::presentation(&none, &pres) {
        Err(verify::Failure::MissingKey(k)) => {
            assert_eq!(k.len(), 32, "the keyhash of the signer that is missing");
            // and the envelope on its own says the same about the same key
            let outer = array_item_ranges(&pres, 0).unwrap();
            assert!(
                matches!(verify::envelope(&none, &pres[outer[0].clone()]), Err(verify::Failure::MissingKey(e)) if e == k)
            );
        }
        other => panic!("a missing key is not a refusal: {other:?}"),
    }
    // a presentation whose root does not match is invalid, which is the
    // other arm and stays one
    let mut broken = pres.clone();
    let last = broken.len() - 1;
    broken[last] ^= 1;
    assert!(matches!(
        verify::presentation(&held, &broken),
        Err(verify::Failure::Invalid(_))
    ));
}
