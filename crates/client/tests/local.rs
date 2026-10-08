//! The ceremony's local interfaces, against the canonical vectors
//! (`wire-format.md` §14.3; `test-vectors/local-interfaces.md`).
//!
//! **The point of this file is byte equality with the corpus.** An encoder
//! the shell now depends on is worth only what its bytes are worth, and the
//! corpus is the independent statement of them: it is generated from the
//! specification by a tool that shares no code with this crate, so a
//! mismatch here means one of the two is wrong and neither gets to decide
//! which.

use rhtn_client::device::{ChannelKind, ChannelOutcome, ChannelResult};
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

    let fx = fixture("P-proximity-outcomes");
    let read = ProximityOutcomes::decode(&fx).expect("decodes");
    assert_eq!(read.ceremony_id, cid, "anchored to the same ceremony");
    assert_eq!(
        read.channels,
        vec![
            ChannelOutcome {
                kind: ChannelKind::Nfc,
                result: ChannelResult::Pass,
                resolution_m: None,
            },
            ChannelOutcome {
                kind: ChannelKind::Optical,
                result: ChannelResult::Pass,
                resolution_m: None,
            },
        ],
        "the record's own channels: NFC pass then optical pass"
    );
    assert_eq!(read.encode(), fx);

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
    assert_eq!(read.nominees.len(), 1, "carol nominated");
    assert_eq!(read.bundle.len(), 1, "one prior record carried");
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
    assert!(ProximityOutcomes::decode(&fixture("P-candidate-handover")).is_err());
    assert!(CandidateHandover::decode(&fixture("P-proximity-outcomes")).is_err());
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

/// The prekey handover encodes to the canonical bytes, with and without a
/// one-time key.
#[test]
fn the_prekey_handover_encodes_to_the_canonical_bytes() {
    let fx = fixture("P-prekey-handover");
    let read = PrekeyHandover::decode(&fx).expect("the canonical handover decodes");
    assert_eq!(read.encode(), fx, "byte equality with the corpus");
    assert!(
        read.one_time.is_some(),
        "the canonical one carries a one-time key"
    );
    // the bundle is records.md's bytes unchanged, not a reinvention: it is
    // long enough to be a signed COSE object rather than a stub
    assert!(read.bundle.len() > 64, "a signed bundle, reused");

    // §7.8 lets a pool run dry, so an absent one-time key is the empty
    // pool and not a malformed object
    let dry = fixture("P-prekey-handover-dry");
    let read = PrekeyHandover::decode(&dry).expect("the dry handover decodes");
    assert_eq!(read.encode(), dry);
    assert_eq!(read.one_time, None);
    assert_ne!(dry, fx, "the fifth element is present or not");
}

/// What the checker refuses, each against the corpus's own negative.
#[test]
fn the_prekey_handover_refuses_what_it_should() {
    for id in [
        "N-prekey-handover-device-31",
        "N-prekey-handover-anchor-31",
        "N-prekey-handover-empty-bundle",
        "N-prekey-handover-version-2",
    ] {
        assert!(PrekeyHandover::decode(&fixture(id)).is_err(), "{id}");
    }
}

/// The session's envelope round-trips, and refuses what it should.
#[test]
fn a_sealed_carriage_opens_only_under_its_own_key_and_ceremony() {
    let key = [3u8; 32];
    let cid = [7u8; 32];
    let plain = IntentExchange {
        contribution: [1u8; 16],
        nominees: vec![],
        bundle: vec![],
        started_at: 5,
        retention_years: 2,
        initiator: true,
        continuations: 0,
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
    let sealed = hexbytes(
        "5de3c068484e4dfcbbd92cd758d67f3179fe2114ea5ac6e25e06e6b8c62624093d2f3bacf815836d9d29958b7aab3e3df07de4e2ecdc0a73fb3d77809a20ff582a0ec97db3e13d1d1b642100cbc9d8594b4c8e491373668bce783a84f363e46ab3e0ae8248f32479194454c0c729dfbec5663825650a8a4ab38e40f59659c2343b17b3814dba1b171d1125e00c9430d71b3736f6166be8fee2957d74440b3b921b5044be54c96c520e233f88601f0d4d86f5c2ef384e8485302291ac2d32cff50bce00d3460716b33c74410c52a588f23bc2f2a58b6cf6e7b05f895243f69ecf2eb5682905c07ee95c404c85b1918d074e333064f530163930c1f3dd169323be766a89163a35bc7a6566cfa19e5dbbc3954d19e346d6fe13e9aa953a97e746cf9d5a38444773054a3bab6edf3234fb6333b26eb76d3f5231532365c956014640461d7efedf02fb632ec9eef11aa5a40058f09bbae449fdb72f2e25b05309f5521b5e",
    );
    assert_eq!(sealed.len(), 354);
    let plain = open_carriage(&key, &cid, &sealed).expect("the fixed carriage opens");
    assert_eq!(&plain[..], &fixture("P-prekey-handover")[..]);
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
