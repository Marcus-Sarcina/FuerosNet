//! The reference policy's reading of a presented archive (MET-03).

use rhtn_archive::record::Record;
use rhtn_archive::tx::{TYPE_PRESENCE, formation_body};
use rhtn_archive::{Keyhash, genesis};
use rhtn_codec::cose;
use rhtn_codec::encode::*;
use rhtn_crypto::identity::testkit::test_identity;
use rhtn_policy::archive::evaluate_archive;
use rhtn_policy::{Evidence, Policy, ReferenceMetric};

fn kh(n: &str) -> Keyhash {
    test_identity(n).public.keyhash
}

/// A presence record between `a` and `b` whose signatures are placeholders:
/// the evaluation reads participants, and verification is not what it does.
fn presence(a: Keyhash, b: Keyhash, t: u64) -> Record {
    let root =
        cose::sha256(&[a, b, t.to_be_bytes().to_vec().try_into().unwrap_or([0; 32])].concat());
    let (ba, bb) = (vec![genesis(&a)], vec![genesis(&b)]);
    let body = formation_body([&ba, &bb], [&a, &b], t, t + 600, &root);
    let mut signers = [a, b];
    signers.sort();
    let mut out = Vec::new();
    emit_map_head(&mut out, 4);
    emit_uint(&mut out, 1);
    emit_uint(&mut out, 1);
    emit_uint(&mut out, 2);
    emit_uint(&mut out, TYPE_PRESENCE);
    emit_uint(&mut out, 3);
    out.extend_from_slice(&body);
    emit_uint(&mut out, 4);
    emit_array_head(&mut out, 4);
    emit_bstr(&mut out, b"");
    emit_map_head(&mut out, 0);
    emit_null(&mut out);
    emit_array_head(&mut out, signers.len() * 2);
    for sgn in &signers {
        for alg in [cose::ALG_EDDSA, cose::ALG_ML_DSA_65] {
            emit_array_head(&mut out, 3);
            emit_bstr(&mut out, &cose::protected_header(alg, sgn));
            emit_map_head(&mut out, 0);
            emit_bstr(&mut out, &[0u8; 64]);
        }
    }
    Record::parse(&out).expect("well-formed")
}

fn stranger(i: u32) -> Keyhash {
    cose::sha256(format!("stranger:{i}").as_bytes())
}

// acceptance: MET-03
#[test]
fn only_the_intersection_with_known_identities_is_weighed() {
    // P's evidence: its subnet, and three identities beyond the horizon it
    // knows through a horizon member's meetings
    let m = ReferenceMetric::default();
    let (p, h, k1, k2, k3, subject) = (
        kh("p"),
        kh("h"),
        kh("k1"),
        kh("k2"),
        kh("k3"),
        kh("subject"),
    );
    let mut ev = Evidence::new(p);
    ev.adopt(p, h);
    ev.meet(h, k1);
    ev.meet(h, k2);
    ev.adopt(k2, k3);
    let weights: Vec<f64> = [k1, k2, k3].iter().map(|k| m.score(&ev, k)).collect();
    assert_eq!(
        weights,
        [10.0, 10.0, 8.0],
        "each recognised counterparty at what P's own graph pushes to it: the two met at the horizon's edge take the edge, the one behind them its relay"
    );
    assert!(!ev.knows(&subject));

    let known: Vec<Record> = [k1, k2, k3]
        .iter()
        .enumerate()
        .map(|(i, k)| presence(subject, *k, 1_800_000_000 + i as u64))
        .collect();
    let strangers: Vec<Record> = (0..1000)
        .map(|i| presence(subject, stranger(i), 1_800_100_000 + i as u64))
        .collect();
    let alone = evaluate_archive(&m, &ev, &subject, &known);
    assert_eq!(
        (alone.weighed.len(), alone.ignored, alone.total),
        (3, 0, 28.0)
    );

    let mut presented = known.clone();
    presented.extend(strangers.iter().cloned());
    let with_strangers = evaluate_archive(&m, &ev, &subject, &presented);
    assert_eq!(
        with_strangers.total, alone.total,
        "the standing equals that from the three known records alone"
    );
    assert_eq!(with_strangers.weighed, alone.weighed);
    assert_eq!(
        with_strangers.ignored, 1000,
        "not weighed less: not weighed"
    );

    presented
        .extend((1000..1500).map(|i| presence(subject, stranger(i), 1_800_200_000 + i as u64)));
    let more = evaluate_archive(&m, &ev, &subject, &presented);
    assert_eq!(
        more.total, alone.total,
        "appending records naming only unknown identities leaves it unchanged"
    );
    assert_eq!(more.ignored, 1500);

    // records among strangers only, none naming the subject's counterparty
    // as someone P knows, inform nothing at all
    let nothing = evaluate_archive(&m, &ev, &subject, &strangers);
    assert_eq!((nothing.weighed.len(), nothing.total), (0, 0.0));
}

/// Verifier responses are sought, and their absence is something an
/// evaluator weighs, never a requirement (design §6.4): a presence record
/// carrying no response at all is evidence, weighed by the observer's
/// graph like any other, and the reference metric reads structure and not
/// responses (design §16.1).  Section 2.6 of the note asked for this by
/// test; every record `presence` builds here carries no response.
#[test]
fn a_presence_record_with_no_verifier_response_is_still_weighed() {
    let m = ReferenceMetric::default();
    let (p, h, k, subject) = (kh("p"), kh("h"), kh("k"), kh("subject"));
    let mut ev = Evidence::new(p);
    ev.adopt(p, h);
    ev.meet(h, k);
    let none = presence(subject, k, 1_800_000_000);
    // the body is the envelope's field 3; a normal record's responses
    // would be its key 5, absent here
    let body = rhtn_codec::cbor::value_slice(&none.bytes, 3).expect("a body");
    assert!(
        rhtn_codec::cbor::value_slice_at(&none.bytes, body.start, 5).is_none(),
        "the record carries no verifier response"
    );
    let read = evaluate_archive(&m, &ev, &subject, std::slice::from_ref(&none));
    assert_eq!(read.weighed.len(), 1, "weighed, not refused");
    assert!(read.total > 0.0, "and confers standing");
    assert_eq!(read.ignored, 0);
}
