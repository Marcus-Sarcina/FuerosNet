//! The ceremony's local interfaces, against the canonical vectors
//! (`wire-format.md` §14.3; `test-vectors/local-interfaces.md`).
//!
//! **The point of this file is byte equality with the corpus.** An encoder
//! the shell now depends on is worth only what its bytes are worth, and the
//! corpus is the independent statement of them: it is generated from the
//! specification by a tool that shares no code with this crate, so a
//! mismatch here means one of the two is wrong and neither gets to decide
//! which.

use rhtn_client::local::*;
use serde_json::Value;

fn corpus() -> Vec<Value> {
    let p = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../test-vectors/corpus.json"
    );
    let v: Value = serde_json::from_str(&std::fs::read_to_string(p).expect("the corpus")).unwrap();
    v["entries"].as_array().expect("entries").clone()
}

fn fixture(id: &str) -> Vec<u8> {
    let entries = corpus();
    let e = entries
        .iter()
        .find(|e| e["id"] == id)
        .unwrap_or_else(|| panic!("{id} is in the corpus"));
    let hex = e["hex"]
        .as_str()
        .unwrap_or_else(|| panic!("{id} carries bytes"));
    (0..hex.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&hex[i..i + 2], 16).unwrap())
        .collect()
}

fn kh(hex: &str) -> [u8; 32] {
    let mut out = [0u8; 32];
    for (i, b) in out.iter_mut().enumerate() {
        *b = u8::from_str_radix(&hex[i * 2..i * 2 + 2], 16).unwrap();
    }
    out
}

/// alice and bob of the vectors (`test-vectors/keys.md`).
const ALICE: &str = "8410def778a5de3a25991aba399716bc8eccfda9ad57d4ea8a0c8dcfc852aa6a";

// acceptance: DEC-36
#[test]
fn the_optical_objects_encode_to_the_canonical_bytes_and_read_back() {
    let fx = fixture("P-optical-contribution-alice");
    let read = OpticalContribution::decode(&fx).expect("the canonical bytes decode");
    assert_eq!(
        read.device,
        kh(ALICE),
        "alice's keyhash is the hash of the key material in field 2"
    );
    assert_eq!(
        rhtn_crypto::Identity::from_key_material(&read.material).map(|i| i.keyhash),
        Some(kh(ALICE)),
        "the first QR carries the full key material (design s12.3)"
    );
    assert_eq!(
        read.encode(),
        fx,
        "and what this crate encodes is what the corpus holds"
    );

    let fx = fixture("P-transcript-confirm");
    let read = TranscriptConfirm::decode(&fx).expect("decodes");
    assert_eq!(read.encode(), fx);
    // the ceremony-id the other objects anchor to
    let cid = read.ceremony_id;

    let fx = fixture("P-candidate-handover");
    let read = CandidateHandover::decode(&fx).expect("decodes");
    assert_eq!(read.ceremony_id, cid);
    assert_eq!(
        read.candidates,
        fixture("P-candidates-bare"),
        "the anchor comes off and the bare payload-path array is what is left"
    );
    assert_eq!(read.encode(), fx);

    let fx = fixture("P-capture-key-handover");
    let read = CaptureKeyHandover::decode(&fx).expect("decodes");
    assert_eq!(read.ceremony_id, cid, "anchored to the same ceremony");
    assert_eq!(
        &read.key[..],
        &fx[fx.len() - 32..],
        "the key is the last 32 bytes, as design §7.5.2.6 derives it"
    );
    assert_eq!(*read.encode(), fx);
}

// acceptance: DEC-36
#[test]
fn the_intent_and_its_continuation_encode_to_the_canonical_bytes() {
    let fx = fixture("P-intent-exchange");
    let read = IntentExchange::decode(&fx).expect("the canonical intent decodes");
    // the nominees no longer ride the intent [author, 2026-10-19]
    assert_eq!(read.bundle.len(), 1, "one introduction carried");
    assert_eq!(read.retention_years, 2);
    assert!(read.initiator);
    assert_eq!(read.continuations, 0, "it fits in one carriage");
    assert_eq!(read.encode(), fx);

    let fx = fixture("P-bundle-continuation");
    let read = BundleContinuation::decode(&fx).expect("decodes");
    assert_eq!(read.index, 1, "numbered from one");
    assert_eq!(read.entries.len(), 1);
    assert_eq!(read.encode(), fx);
}

// functional: CER-020
// acceptance: DEC-36
#[test]
fn a_bundle_past_one_carriage_is_split_and_put_back_together() {
    let cid = TranscriptConfirm::decode(&fixture("P-transcript-confirm"))
        .unwrap()
        .ceremony_id;
    let entry = IntentExchange::decode(&fixture("P-intent-exchange"))
        .unwrap()
        .bundle[0]
        .clone();
    // 600 entries: one exchange at the 256 ceiling and two continuations,
    // 256 and 88 (`wire-format.md` §5.4 -- the bundle has no ceiling, its
    // carriage does)
    let intent = IntentExchange {
        bundle: vec![entry; 600],
        continuations: 0,
        ..IntentExchange::decode(&fixture("P-intent-exchange")).unwrap()
    };
    let carriage = intent_carriage(&intent, &cid);
    assert_eq!(carriage.len(), 3, "an exchange and two continuations");
    let first = IntentExchange::decode(&carriage[0]).expect("the exchange decodes");
    assert_eq!(first.bundle.len(), 256, "the carriage is full");
    assert_eq!(first.continuations, 2, "and it says how many follow");
    let c2 = BundleContinuation::decode(&carriage[2]).unwrap();
    assert_eq!((c2.index, c2.entries.len()), (2, 88));

    let (whole, taken) = read_intent(&carriage, &cid).expect("reads back");
    assert_eq!((whole.bundle.len(), taken), (600, 2));
    assert_eq!(whole.bundle, intent.bundle, "entry for entry");

    // A CONTINUATION ANCHORED ELSEWHERE ENDS THE BUNDLE, and does not
    // refuse the ceremony (§5.4): n is what the receiver holds.
    let mut stray = carriage.clone();
    stray[1] = BundleContinuation {
        ceremony_id: [9u8; 32],
        index: 1,
        entries: vec![intent.bundle[0].clone()],
    }
    .encode();
    let (short, taken) = read_intent(&stray, &cid).expect("still a ceremony");
    assert_eq!((short.bundle.len(), taken), (256, 0), "n is what is held");

    // and so does one out of order: a missing continuation is not a gap to
    // be guessed at
    let mut gap = carriage.clone();
    gap.swap(1, 2);
    let (short, taken) = read_intent(&gap, &cid).expect("still a ceremony");
    assert_eq!((short.bundle.len(), taken), (256, 0));

    // THE DECLARED COUNT IS THE CAP (§14.3.2): an exchange that says one
    // continuation follows is read with one, and a second -- however well
    // anchored and numbered -- is not this bundle's.  Without this a
    // counterparty could stream continuations for as long as the
    // receiver's memory lasted
    let mut declared_one = carriage.clone();
    declared_one[0] = IntentExchange {
        continuations: 1,
        ..first
    }
    .encode();
    let (capped, taken) = read_intent(&declared_one, &cid).expect("still a ceremony");
    assert_eq!(
        (capped.bundle.len(), taken),
        (512, 1),
        "the second continuation is not read"
    );
    assert_eq!(&capped.bundle[..], &intent.bundle[..512]);
}

// acceptance: DEC-36
#[test]
fn an_object_the_schema_refuses_does_not_decode_here_either() {
    // the version the §14.3 encodings do not know (`wire-format.md` §14.3.2)
    assert!(
        OpticalContribution::decode(&fixture("N-optical-contribution-version-2")).is_err(),
        "an unknown version is refused by the decoder, not only by the checker"
    );
    assert!(
        OpticalContribution::decode(&fixture("N-optical-contribution-15")).is_err(),
        "a 15-byte contribution"
    );
    // the capture key: short by a byte, and under a version nobody knows
    assert!(
        CaptureKeyHandover::decode(&fixture("N-capture-key-handover-31")).is_err(),
        "a 31-byte capture key"
    );
    assert!(
        CaptureKeyHandover::decode(&fixture("N-capture-key-handover-version-2")).is_err(),
        "version 2 of the capture-key handover"
    );
    // and the anchored kinds are not interchangeable, each checking against
    // its own rule
    assert!(CandidateHandover::decode(&fixture("P-capture-key-handover")).is_err());
    assert!(BundleContinuation::decode(&fixture("P-intent-exchange")).is_err());
    // the responder's intent of the pair: the other flag, an empty bundle
    let r = IntentExchange::decode(&fixture("P-intent-exchange-responder")).expect("decodes");
    let i = IntentExchange::decode(&fixture("P-intent-exchange")).expect("decodes");
    assert!(
        i.initiator && !r.initiator,
        "the pair disagree on who began"
    );
    assert!(r.bundle.is_empty());
    assert!(CaptureKeyHandover::decode(&fixture("P-transcript-confirm")).is_err());
    assert!(CaptureKeyHandover::decode(&fixture("P-candidate-handover")).is_err());
    assert!(TranscriptConfirm::decode(&fixture("P-capture-key-handover")).is_err());
    assert!(CandidateHandover::decode(&fixture("P-capture-key-handover")).is_err());
}

/// **The pairing's two objects** (`wire-format.md` §14.3.3): the desktop's
/// `DeviceIntroduction` and the phone's `DeviceCredential` encode to the
/// canonical bytes, and the credential carries alice's two encryption
/// secrets, the ones her own seed derives.
#[test]
fn the_device_introduction_and_credential_encode_to_the_canonical_bytes() {
    let fx = fixture("P-device-introduction");
    let intro = DeviceIntroduction::decode(&fx).expect("the canonical introduction decodes");
    assert_eq!(intro.encode(), fx, "byte equality with the corpus");

    let fx = fixture("P-device-credential");
    let cred = DeviceCredential::decode(&fx).expect("the canonical credential decodes");
    assert_eq!(&cred.encode()[..], &fx[..], "byte equality with the corpus");
    assert_eq!(cred.run.len(), 1, "one delegation");
    let alice = rhtn_crypto::identity::testkit::test_identity("alice");
    assert_eq!(hex(&alice.public.keyhash), ALICE, "the vectors' alice");
    let own = rhtn_crypto::seal::EncryptionSecret::from_identity_root(alice.identity_root());
    assert_eq!(
        *cred.kem_seed,
        *own.kem_seed(),
        "her X-Wing seed, as her seed derives it"
    );
    assert_eq!(
        *cred.auth,
        *own.auth_secret(),
        "her sealing-authentication key, as her seed derives it"
    );
    assert!(
        !format!("{cred:?}").contains(&hex(&cred.kem_seed[..])),
        "a credential's debug form shows nothing of the secrets"
    );

    // the run's ceiling is ninety days, forty-five windows
    let b = fixture("B-credential-delegations-45");
    assert_eq!(
        DeviceCredential::decode(&b)
            .expect("at the ceiling")
            .run
            .len(),
        45
    );
}

/// What the checker refuses of the pairing's objects, each against the
/// corpus's own negative.
#[test]
fn the_device_introduction_and_credential_refuse_what_they_should() {
    for id in ["N-device-introduction-31", "N-device-introduction-bundle"] {
        assert!(DeviceIntroduction::decode(&fixture(id)).is_err(), "{id}");
    }
    for id in ["N-device-credential-seed-31", "N-credential-delegations-46"] {
        assert!(DeviceCredential::decode(&fixture(id)).is_err(), "{id}");
    }
    assert!(DeviceCredential::decode(&fixture("P-device-introduction")).is_err());
    assert!(DeviceIntroduction::decode(&fixture("P-device-credential")).is_err());
}

// functional: NET-024
// functional: NET-027
/// The session's envelope round-trips, and refuses what it should.
#[test]
fn a_sealed_carriage_opens_only_under_its_own_key_and_ceremony() {
    let key = [3u8; 32];
    let cid = [7u8; 32];
    let plain = IntentExchange {
        contribution: [1u8; 16],
        bundle: vec![],
        started_at: 5,
        retention_years: 2,
        initiator: true,
        continuations: 0,
        locator: fixture("P-signedlocator"),
        encryption_key: fixture("P-encryption-key"),
        number: None,
        node: None,
    }
    .encode();

    let sealed = seal_carriage(&key, &cid, &plain);
    assert_ne!(sealed, plain, "the bearer carries ciphertext");
    assert!(
        !sealed.windows(plain.len()).any(|w| w == plain),
        "the plaintext is not in the sealed bytes"
    );
    assert_eq!(
        *open_carriage(&key, &cid, &sealed).expect("opens"),
        plain,
        "and opens to exactly what went in"
    );

    // a different ceremony's key, which is what actually separates them
    assert!(
        open_carriage(&[4u8; 32], &cid, &sealed).is_none(),
        "another key"
    );
    // the associated data states that binding as well
    assert!(
        open_carriage(&key, &[8u8; 32], &sealed).is_none(),
        "another ceremony-id"
    );
    // a flipped byte anywhere
    for i in [0, 6, 13, sealed.len() - 1] {
        let mut bad = sealed.clone();
        bad[i] ^= 1;
        assert!(
            open_carriage(&key, &cid, &bad).is_none(),
            "byte {i} flipped"
        );
    }
    // and plaintext from a peer that does not encrypt
    assert!(
        open_carriage(&key, &cid, &plain).is_none(),
        "a peer sending plaintext fails to open, which is the refusal a \
         version disagreement should produce"
    );
}

// functional: NET-024
/// Two sealings of one plaintext differ, so the nonce is doing its work.
#[test]
fn sealing_twice_gives_different_bytes() {
    let (key, cid, plain) = ([3u8; 32], [7u8; 32], b"the same object".to_vec());
    let a = seal_carriage(&key, &cid, &plain);
    let b = seal_carriage(&key, &cid, &plain);
    assert_ne!(a, b, "a random nonce per carriage");
    assert_eq!(*open_carriage(&key, &cid, &a).unwrap(), plain);
    assert_eq!(*open_carriage(&key, &cid, &b).unwrap(), plain);
}

// functional: NET-024
/// **The §14.3.2 vector, pinned on this side of the seam** [reviewer,
/// 2026-10-08]. `verify.py` re-derives the key and opens the carriage with
/// primitives of its own; until this test the Rust sealing — the one
/// implementation — was held to nothing fixed, so a divergence would have
/// passed the whole gate and surfaced as a ceremony failing at the bearer.
#[test]
fn the_session_key_and_the_sealed_carriage_match_the_fixed_vector() {
    use rhtn_client::keys::session_key;
    let first = hex16("d43a0b07379cf934c8b7e4b54629f95c");
    let second = hex16("57c54784788abe36a304be6d2eff43d4");
    // the vector orders the contributions by participant keyhash, so any
    // two keyhashes in that order reproduce it, either way round
    let (lo, hi) = ([0u8; 32], [1u8; 32]);
    let key = session_key((&lo, &first), (&hi, &second));
    assert_eq!(
        hex(&*key),
        "41e5809dfcbfe09a7019e7aa1180705532527c68ec69e243926451417261d9d4"
    );
    assert_eq!(*session_key((&hi, &second), (&lo, &first)), *key);
    let cid = kh("cb8ea88ad0a089017394c291f918217c4dc8d754a4639eb024f23662e5ca2b18");
    let sealed = hexbytes(concat!(
        "5de3c068484e4dfcbbd92cd75ed67f3179fe2114ea5ac6e25e06e6b8c6262409",
        "3d2f3bacf815836d9d29958b7aab3e3df07d4cd446db024f40994a36b438b074",
        "7ee651b917b1d7dcc3e0ff1f1d819796567beac38cb68b78149d058183029bb0",
        "8c37",
    ));
    assert_eq!(sealed.len(), 98);
    let plain = open_carriage(&key, &cid, &sealed).expect("the fixed carriage opens");
    assert_eq!(&plain[..], &fixture("P-capture-key-handover")[..]);
    // another ceremony's key — the same contributions the other way round
    // is one — does not open it, and neither does the right key under
    // another ceremony's id
    let other = session_key((&lo, &second), (&hi, &first));
    assert!(open_carriage(&other, &cid, &sealed).is_none());
    assert!(open_carriage(&key, &[9u8; 32], &sealed).is_none());
}

fn hex16(s: &str) -> [u8; 16] {
    let mut out = [0u8; 16];
    out.copy_from_slice(&hexbytes(s));
    out
}

fn hexbytes(s: &str) -> Vec<u8> {
    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&s[i..i + 2], 16).unwrap())
        .collect()
}

fn hex(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect()
}

// functional: NET-033
/// **An intent carries a number or does not, and both are well formed**
/// (`wire-format.md` §14.3.2's tenth field; NET-033) [author, 2026-11-10]:
/// a handset without a subscription sends the nine fields it always sent,
/// so nothing has to be true of the counterparty's build.
#[test]
fn an_intent_carries_a_number_or_does_not() {
    let plain = |number: Option<String>| IntentExchange {
        contribution: [1u8; 16],
        bundle: vec![],
        started_at: 5,
        retention_years: 2,
        initiator: true,
        continuations: 0,
        locator: fixture("P-signedlocator"),
        encryption_key: fixture("P-encryption-key"),
        number,
        node: None,
    };
    let with = plain(Some("+15550001111".into())).encode();
    let without = plain(None).encode();
    assert_eq!(with[0], 0x8a, "ten fields with a number");
    assert_eq!(without[0], 0x89, "nine without");
    assert_eq!(
        IntentExchange::decode(&with)
            .expect("reads")
            .number
            .as_deref(),
        Some("+15550001111")
    );
    assert_eq!(
        IntentExchange::decode(&without).expect("reads").number,
        None
    );
    assert!(
        !without.windows(2).any(|w| w == b"+1"),
        "an intent with no number carries nothing of one"
    );

    // **bounded**: a number past the bound is not one
    let long = "+".to_string() + &"9".repeat(32);
    let over = plain(Some(long)).encode();
    assert!(
        IntentExchange::decode(&over).is_err(),
        "33 characters is refused rather than truncated"
    );
}

/// A bare `NetworkPoint` for 10.0.0.4:7432 (`wire-format.md` §4.4): the
/// address at key 1 and the port at key 3, no provider and no zone.
const POINT: [u8; 11] = [0xa2, 0x01, 0x44, 10, 0, 0, 4, 0x03, 0x19, 0x1d, 0x08];

// functional: NET-036
/// **An intent carries the node at its eleventh field, where it answers and
/// whose it is, and the tenth is null where no number comes before it**
/// (`wire-format.md` §14.3.2) [author, 2026-11-21; author, 2026-11-22]:
/// the array is positional, so a node without a number needs the number's
/// place held, and null there with no node after it is a second spelling
/// of nine fields and refused. The eleventh is the bare point and the
/// 32-byte keyhash the node answers as, and nothing else.
#[test]
fn an_intent_carries_a_node_after_a_number_or_a_null_in_its_place() {
    // the keyhash the node answers as: the sender's, or one further up
    // its chain
    const NODE: [u8; 32] = [7; 32];
    let plain = |number: Option<String>, point: Option<Vec<u8>>| IntentExchange {
        contribution: [1u8; 16],
        bundle: vec![],
        started_at: 5,
        retention_years: 2,
        initiator: true,
        continuations: 0,
        locator: fixture("P-signedlocator"),
        encryption_key: fixture("P-encryption-key"),
        number,
        node: point.map(|p| (p, NODE)),
    };
    // the eleventh field as it is written: a two-array, the point, then a
    // 32-byte string
    let carried = |p: &[u8]| [&[0x82][..], p, &[0x58, 0x20][..], &NODE[..]].concat();
    let neither = plain(None, None).encode();
    let number = plain(Some("+15550001111".into()), None).encode();
    let node = plain(None, Some(POINT.to_vec())).encode();
    let both = plain(Some("+15550001111".into()), Some(POINT.to_vec())).encode();
    assert_eq!(neither[0], 0x89, "nine fields with neither, as before");
    assert_eq!(number[0], 0x8a, "ten with a number alone, as before");
    assert_eq!(node[0], 0x8b, "eleven with a node");
    assert_eq!(both[0], 0x8b, "and eleven with both");
    assert!(
        node.ends_with(&[&[0xf6][..], &carried(&POINT)].concat()),
        "null, then the point and whose it is"
    );
    assert!(
        both.ends_with(&carried(&POINT))
            && !both.ends_with(&[&[0xf6][..], &carried(&POINT)].concat()),
        "the number in its place, then the node"
    );
    for (bytes, num, pt) in [
        (&neither, None, None),
        (&number, Some("+15550001111"), None),
        (&node, None, Some(&POINT[..])),
        (&both, Some("+15550001111"), Some(&POINT[..])),
    ] {
        let read = IntentExchange::decode(bytes).expect("reads");
        assert_eq!(read.number.as_deref(), num);
        assert_eq!(
            read.node.as_ref().map(|(p, k)| (p.as_slice(), *k)),
            pt.map(|p| (p, NODE))
        );
        assert_eq!(&read.encode(), bytes, "one spelling each");
    }

    // **null with nothing after it** is nine fields spelled twice
    let mut null_alone = neither.clone();
    null_alone[0] = 0x8a;
    null_alone.push(0xf6);
    assert!(
        IntentExchange::decode(&null_alone).is_err(),
        "null at ten alone"
    );

    // **the zone is not a key of an unsigned point** (§4.4)
    let zoned: Vec<u8> = [
        &[
            0xa3, 0x01, 0x44, 10, 0, 0, 4, 0x02, 0x19, 0xfb, 0xff, 0x04, 0x64,
        ][..],
        b"nyc1",
    ]
    .concat();
    let with_zone = plain(None, Some(zoned)).encode();
    assert!(
        IntentExchange::decode(&with_zone).is_err(),
        "a zone at eleven"
    );
    // and the default port is not written out, nor port zero
    for port in [&[0x19, 0x1d, 0x07][..], &[0x00][..]] {
        let p = [&POINT[..8], port].concat();
        let bad = plain(None, Some(p)).encode();
        assert!(IntentExchange::decode(&bad).is_err(), "port {port:02x?}");
    }

    // **the eleventh is the pair and only the pair**: a bare point, a
    // keyhash of another width, or a third entry is refused
    let at_eleven = |field: Vec<u8>| {
        let mut b = neither.clone();
        b[0] = 0x8b;
        b.push(0xf6);
        b.extend_from_slice(&field);
        b
    };
    assert!(
        IntentExchange::decode(&at_eleven(carried(&POINT))).is_ok(),
        "the pair reads"
    );
    for (field, why) in [
        (POINT.to_vec(), "a point with no keyhash"),
        (
            [&[0x82][..], &POINT[..], &[0x58, 0x1f][..], &NODE[..31]].concat(),
            "a 31-byte keyhash",
        ),
        (
            [&[0x82][..], &POINT[..], &[0x58, 0x21][..], &NODE[..], &[0]].concat(),
            "a 33-byte keyhash",
        ),
        (
            [&[0x82][..], &POINT[..], &[0x78, 0x20][..], &NODE[..]].concat(),
            "a keyhash as text",
        ),
        (
            [&[0x81][..], &POINT[..]].concat(),
            "a point alone in the array",
        ),
        (
            [&[0x83][..], &carried(&POINT)[1..], &[0xf6]].concat(),
            "a third entry",
        ),
    ] {
        assert!(IntentExchange::decode(&at_eleven(field)).is_err(), "{why}");
    }

    // a twelfth field is not one
    let mut twelve = both.clone();
    twelve[0] = 0x8c;
    twelve.push(0xf6);
    assert!(IntentExchange::decode(&twelve).is_err(), "twelve fields");
}
