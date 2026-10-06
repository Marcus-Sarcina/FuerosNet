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

// acceptance: owed, pending the §14.3.4 corpus fixture
/// The prekey handover round-trips, with and without a one-time key.
///
/// **Not against the corpus, because the corpus has no fixture for it
/// yet** (`test-vectors/README.md` says five of six). So this asserts the
/// encoder against itself and the checker against malformed input, which
/// is strictly less than every other test in this file claims. The byte
/// equality this file exists for is owed and is not here.
#[test]
fn the_prekey_handover_round_trips_with_and_without_a_one_time_key() {
    let full = PrekeyHandover {
        ceremony_id: [7u8; 32],
        device: [9u8; 32],
        bundle: vec![0xa1, 0x01, 0x02],
        one_time: Some(vec![0xb2, 0x03]),
    };
    assert_eq!(
        PrekeyHandover::decode(&full.encode()).expect("decodes"),
        full
    );

    // §7.8 lets a pool run dry, so an absent one-time key is the empty
    // pool and not a malformed object
    let dry = PrekeyHandover {
        one_time: None,
        ..full.clone()
    };
    let bytes = dry.encode();
    assert_eq!(PrekeyHandover::decode(&bytes).expect("decodes"), dry);
    assert_ne!(bytes, full.encode(), "the fifth element is present or not");
}

/// What the checker refuses.
#[test]
fn the_prekey_handover_refuses_what_it_should() {
    use rhtn_codec::encode::*;
    let mk = |n: usize, ver: u64, cid: &[u8], dev: &[u8], bundle: &[u8]| {
        let mut out = Vec::new();
        emit_array_head(&mut out, n);
        emit_uint(&mut out, ver);
        emit_bstr(&mut out, cid);
        emit_bstr(&mut out, dev);
        emit_bstr(&mut out, bundle);
        out
    };
    assert!(
        PrekeyHandover::decode(&mk(4, 2, &[7u8; 32], &[9u8; 32], &[1])).is_err(),
        "version 2"
    );
    assert!(
        PrekeyHandover::decode(&mk(4, 1, &[7u8; 31], &[9u8; 32], &[1])).is_err(),
        "a 31-byte ceremony-id"
    );
    assert!(
        PrekeyHandover::decode(&mk(4, 1, &[7u8; 32], &[9u8; 31], &[1])).is_err(),
        "a 31-byte device"
    );
    assert!(
        PrekeyHandover::decode(&mk(4, 1, &[7u8; 32], &[9u8; 32], &[])).is_err(),
        "an empty bundle"
    );
    assert!(
        PrekeyHandover::decode(&mk(3, 1, &[7u8; 32], &[9u8; 32], &[1])).is_err(),
        "three fields"
    );
}
