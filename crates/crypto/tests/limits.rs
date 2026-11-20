//! Three rows no test had held by itself: the finalization gap at its
//! edges (TX-023), the txid as the body's digest and nothing else's
//! (SIG-001), and the signing domains, a signature made under one
//! verifying as nothing under any other (SIG-005).  Each is read against
//! the codec and crypto crates directly rather than through a fixture
//! that happens to pass.

mod common;

use common::*;
use rhtn_codec::cbor::*;
use rhtn_codec::cose::{self, aad};
use rhtn_codec::encode::*;
use rhtn_codec::schema;
use rhtn_crypto::identity::testkit::test_identity;
use rhtn_crypto::verify;
use std::collections::BTreeSet;

fn uint(v: u64) -> Vec<u8> {
    let mut out = Vec::new();
    emit_uint(&mut out, v);
    out
}

/// The structural check alone, as the corpus runs a body-class entry.
fn body_check(b: &[u8]) -> Result<(), String> {
    let item = parse_all(b).map_err(|e| e.0.to_string())?;
    schema::check_body(b, &item).map_err(|e| e.0.to_string())
}

// functional: TX-023
/// **`finalized_at` is at least `started_at` and at most a day after it**
/// (`wire-format.md` §3.2), and the structural check compares neither
/// with any clock.  The corpus holds the two bounds as vectors (R2, R3,
/// D10, D11); this walks the edge from one body so the rule is seen to
/// turn at exactly 86,400 seconds, and a body a decade hence is accepted
/// as the vector's own date is.
#[test]
fn the_finalization_gap_admits_zero_and_a_day_and_refuses_a_second_more() {
    let good = fixture("B-finalization-gap-86400").bytes;
    let item = parse_all(&good).unwrap();
    let Item::Map(m) = &item else {
        panic!("a body is a map")
    };
    let started = map_get(m, 1).and_then(as_uint).expect("started_at");
    let finalized = map_get(m, 2).and_then(as_uint).expect("finalized_at");
    assert_eq!(finalized - started, 86_400, "the vector sits on the bound");
    assert_eq!(body_check(&good), Ok(()));

    let gap = |d: i64| replace_value(&good, 0, 2, &uint((started as i64 + d) as u64));
    assert_eq!(body_check(&gap(0)), Ok(()), "finalized_at == started_at");
    assert_eq!(body_check(&gap(1)), Ok(()));
    assert_eq!(body_check(&gap(86_400)), Ok(()), "exactly 24 hours");
    assert_eq!(
        body_check(&gap(86_401)),
        Err("finalization gap".into()),
        "one second past the day"
    );
    assert_eq!(
        body_check(&gap(-1)),
        Err("finalization gap".into()),
        "finalized before started"
    );

    // **no clock is consulted**: the same gap a decade on is accepted,
    // where a check against the receiver's time would have refused a
    // record from its future
    let decade = 10 * 365 * 86_400;
    let hence = replace_value(
        &replace_value(&good, 0, 1, &uint(started + decade)),
        0,
        2,
        &uint(started + decade + 86_400),
    );
    assert_eq!(
        body_check(&hence),
        Ok(()),
        "structural acceptance reads no clock"
    );
}

/// `transactions.md`'s minimal adoption: the body as printed there, and
/// the txid the independent harness computed for it.
const ADOPT_MIN_BODY: &str = "\
a6008281582016dbefb7a6a629a8901cd16199491e66105f38f9876c62cdf7d8\
6f5b2b8d4a1481582016dbefb7a6a629a8901cd16199491e66105f38f9876c62\
cdf7d86f5b2b8d4a140158208410def778a5de3a25991aba399716bc8eccfda9\
ad57d4ea8a0c8dcfc852aa6a0258206bcf8a3e8899fc206bc603744414d58b01\
db857986d82f611b0794ac9c32c37503a40158206bcf8a3e8899fc206bc60374\
4414d58b01db857986d82f611b0794ac9c32c37502a201433141500205038205\
000458203f438083da65968113a3ddace3e7f735d8607a8edb392cbbb069aca5\
a92cd6fa041a69d99d1008582016dbefb7a6a629a8901cd16199491e66105f38\
f9876c62cdf7d86f5b2b8d4a14";
const ADOPT_MIN_TXID: &str = "cd57106b2c0e8ec3811a49d56bf069fa2eebdd0e6de4445e7d48c0458bf8ae0c";

// functional: SIG-001
/// **A txid is SHA-256 of the body map and of nothing else**
/// (`wire-format.md` §1.4): the vector's stated id is recomputed from the
/// body, an extension on the body moves it, and a change inside the
/// signature array leaves it where it was while the envelope stops
/// verifying.
#[test]
fn a_txid_is_the_digest_of_the_body_and_of_nothing_else() {
    let body = hex::decode(ADOPT_MIN_BODY).unwrap();
    assert_eq!(
        hex::encode(cose::txid(&body)),
        ADOPT_MIN_TXID,
        "the id transactions.md states for its adoption"
    );
    let env = fixture("P-adopt-min").bytes;
    assert_eq!(body_of(&env), body, "the corpus envelope carries that body");
    let ids = held();
    verify::envelope(&ids, &env).expect("the vector verifies as it stands");

    // an extension on the body is part of what is hashed
    let extended = append_entries(&body, 0, &ext_entry(100, b"\xc0\xff\xee"), 1);
    assert_ne!(
        cose::txid(&extended),
        cose::txid(&body),
        "a body extension moves the id"
    );

    // the signature array is not: a bit flipped in the last signature
    // leaves the body, and so the id, exactly as they were, and the
    // envelope no longer verifies
    let sigs = value_slice(&env, 4).expect("field 4, the COSE_Sign");
    let mut altered = env.clone();
    altered[sigs.end - 1] ^= 0x01;
    assert_eq!(body_of(&altered), body, "the body bytes are untouched");
    assert_eq!(cose::txid(&body_of(&altered)), cose::txid(&body));
    assert!(
        verify::envelope(&ids, &altered).is_err(),
        "but the envelope is invalid"
    );
}

/// The signing domains of `wire-format.md` §1.1, as the codec names them:
/// the sixteen the table lists.
const DOMAINS: [&[u8]; 16] = [
    aad::ENVELOPE,
    aad::VERIFIER,
    aad::CONSENT,
    aad::CURRENCY,
    aad::CATALOG,
    aad::ABUSE,
    aad::LOCATOR,
    aad::ANCHOR,
    aad::ENDPOINTS,
    aad::NODE_KEY,
    aad::ENCRYPTION_KEY,
    aad::SUBTREE_ACK,
    aad::SUCCESSOR,
    aad::TRANSFER,
    aad::DELEGATION,
    aad::PROFILE,
];

// functional: SIG-005
/// **A signature made for one domain verifies as nothing in another**
/// (`wire-format.md` §1.1): the tag is `external_aad`, covered by the
/// signature, so the same key over the same payload under each of the
/// domains yields a signature that passes under its own tag alone.  The
/// classical `COSE_Sign1` is walked over every pair; the hybrid
/// `COSE_Sign` entries are checked across one pair, which is the same
/// construction with a second algorithm.
#[test]
fn a_signature_made_for_one_domain_verifies_as_nothing_in_another() {
    let distinct: BTreeSet<&[u8]> = DOMAINS.iter().copied().collect();
    assert_eq!(distinct.len(), DOMAINS.len(), "no two domains share a tag");
    for d in DOMAINS {
        assert!(
            d.starts_with(b"rhtn/1:"),
            "{:?}",
            String::from_utf8_lossy(d)
        );
    }
    let named_by_the_table = [
        "envelope",
        "verifier",
        "consent",
        "currency",
        "catalog",
        "abuse",
        "locator",
        "anchor",
        "endpoints",
        "node-key",
        "encryption-key",
        "profile",
        "subtree-ack",
        "successor",
        "transfer",
        "delegation",
    ];
    for n in named_by_the_table {
        assert!(
            distinct.contains(format!("rhtn/1:{n}").as_bytes()),
            "§1.1's `{n}` is a domain the codec names"
        );
    }

    let alice = test_identity("alice");
    let payload = b"the same bytes under every tag";
    let prot = cose::protected_alg(cose::ALG_EDDSA);
    for (i, made_under) in DOMAINS.iter().enumerate() {
        let s1 = alice.sign1_ed_unnamed(made_under, payload);
        let items = array_item_ranges(&s1, 0).expect("a COSE_Sign1 array");
        assert_eq!(items.len(), 4, "[protected, {{}}, null, sig]");
        let sig = &s1[items[3].end - 64..items[3].end];
        for (j, read_under) in DOMAINS.iter().enumerate() {
            let tbs = cose::sig_structure_sign1(&prot, read_under, payload);
            assert_eq!(
                alice.public.verify_ed(sig, &tbs),
                i == j,
                "made under {:?}, read under {:?}",
                String::from_utf8_lossy(made_under),
                String::from_utf8_lossy(read_under)
            );
        }
    }

    // the hybrid entries: classical then post-quantum, each over the
    // `COSE_Sign` structure with the tag inside it
    let entries = alice.sign_entries_unnamed(aad::ENVELOPE, payload);
    let mut wrapped = Vec::new();
    emit_array_head(&mut wrapped, 2);
    wrapped.extend_from_slice(&entries);
    let two = array_item_ranges(&wrapped, 0).expect("two entries");
    let pq_entry = &wrapped[two[1].clone()];
    let parts = array_item_ranges(pq_entry, 0).expect("[protected, {}, sig]");
    let pq_sig = &pq_entry[parts[2].end - 3309..parts[2].end];
    let prot_pq = cose::protected_alg(cose::ALG_ML_DSA_65);
    assert!(alice.public.verify_pq(
        pq_sig,
        &cose::sig_structure_sign(&prot_pq, aad::ENVELOPE, payload)
    ));
    for other in [aad::VERIFIER, aad::DELEGATION, aad::PROFILE] {
        assert!(
            !alice
                .public
                .verify_pq(pq_sig, &cose::sig_structure_sign(&prot_pq, other, payload)),
            "an envelope signature read as {:?}",
            String::from_utf8_lossy(other)
        );
    }
}
