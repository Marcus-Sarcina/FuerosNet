//! The ceremony's opening over the bearer, between two live clients
//! (`wire-format.md` §14.3): the optical exchange, then the intent, the
//! proximity outcomes, the candidates and the capture keys, every one of
//! them as the bytes the shell carries and none of them as fields.

mod common;

use common::harness::*;
use common::*;
use rhtn_client::ceremony::*;
use rhtn_client::device::*;

/// What a bearer is, for a test: a byte string handed from one device to
/// the other. Nothing else is modelled, because nothing else is promised —
/// §14.3.1 ranks bearers by locality and relies on none of them for
/// integrity.
fn carry(bytes: &[u8]) -> Vec<u8> {
    bytes.to_vec()
}

// acceptance: CER-44
#[test]
fn the_opening_crosses_as_bytes_and_each_check_is_made_on_this_side() {
    let mut s = setup(&["alice", "bob"], &[ChannelKind::Nfc]);
    s.face_off("alice", "bob");
    s.client("alice")
        .begin(Some(kh("bob")), vec![], true)
        .expect("alice begins");
    s.client("bob")
        .begin(Some(kh("alice")), vec![], false)
        .expect("bob begins");

    // STEP 1: each shows its contribution, each reads the other's off the
    // screen. The bytes are the QR's payload and nothing above this
    // boundary reads them.
    let oa = s.client("alice").optical_contribution().unwrap();
    let ob = s.client("bob").optical_contribution().unwrap();
    assert_eq!(
        s.client("alice").take_optical(&carry(&ob)).unwrap(),
        kh("bob"),
        "the QR names who showed it"
    );
    assert_eq!(
        s.client("bob").take_optical(&carry(&oa)).unwrap(),
        kh("alice")
    );

    // STEP 2: each shows the ceremony-id it computed, and checks the
    // other's against its own. Agreement is what fixes it.
    let ta = s.client("alice").transcript_confirm().unwrap();
    let tb = s.client("bob").transcript_confirm().unwrap();
    let cid_a = s.client("alice").take_transcript(&carry(&tb)).unwrap();
    let cid_b = s.client("bob").take_transcript(&carry(&ta)).unwrap();
    assert_eq!(cid_a, cid_b, "one ceremony, two devices, the same id");
    assert_eq!(s.client("alice").ceremony_id(), Some(cid_a));

    // THE BEARER carries the intent, echoing the contribution the screen
    // showed. Alice's bundle is empty here, so one carriage holds it.
    let ia = s.client("alice").intent_carriage().unwrap();
    let ib = s.client("bob").intent_carriage().unwrap();
    assert_eq!(ia.len(), 1, "an empty bundle fits in one carriage");
    let carried: Vec<Vec<u8>> = ib.iter().map(|m| carry(m)).collect();
    assert_eq!(
        s.client("alice")
            .take_intent_carriage(kh("bob"), &carried)
            .unwrap(),
        0,
        "no continuations followed"
    );
    let carried: Vec<Vec<u8>> = ia.iter().map(|m| carry(m)).collect();
    s.client("bob")
        .take_intent_carriage(kh("alice"), &carried)
        .unwrap();

    // the channels, anchored, and the counterparty weighs what they say
    let pa = s.client("alice").proximity_carriage().unwrap();
    let pb = s.client("bob").proximity_carriage().unwrap();
    s.client("alice").take_proximity(&carry(&pb)).unwrap();
    s.client("bob").take_proximity(&carry(&pa)).unwrap();

    // and the candidates: an address to dial, with the anchor the local
    // carriage adds and nothing else
    let bare = vec![0x81, 0x83, 0x00, 0x44, 10, 0, 0, 1, 0x19, 0x1f, 0x90];
    let handover = s.client("alice").candidate_carriage(bare.clone()).unwrap();
    assert_eq!(
        s.client("bob")
            .take_candidate_carriage(&carry(&handover))
            .unwrap(),
        bare,
        "what comes out is the bare array the payload path carries"
    );

    // AT CAPTURE TIME THE KEYS CROSS, one each way (design §7.5.2.6): the
    // anchor comes off at the check and what is left is the key this side
    // seals its captures of the other beneath
    let ka = s.client("alice").capture_key_carriage().unwrap();
    let kb = s.client("bob").capture_key_carriage().unwrap();
    let bobs = s
        .client("alice")
        .take_capture_key_carriage(&carry(&kb))
        .unwrap();
    assert_eq!(
        bobs,
        s.client("bob").capture_key().unwrap(),
        "bob's key, as bob derived it"
    );
    let alices = s
        .client("bob")
        .take_capture_key_carriage(&carry(&ka))
        .unwrap();
    assert_eq!(alices, s.client("alice").capture_key().unwrap());
    assert_ne!(bobs, alices, "each key is its holder's own");
    s.client("alice")
        .capture(bobs)
        .expect("alice seals under bob's key");
    s.client("bob")
        .capture(alices)
        .expect("bob seals under alice's key");
}

// acceptance: CER-44
#[test]
fn a_bearer_that_disagrees_with_the_screen_stops_the_ceremony() {
    let mut s = setup(&["alice", "bob", "carol"], &[ChannelKind::Nfc]);
    s.face_off("alice", "bob");
    s.client("alice")
        .begin(Some(kh("bob")), vec![], true)
        .unwrap();
    s.client("bob")
        .begin(Some(kh("alice")), vec![], false)
        .unwrap();
    // carol runs her own ceremony with bob, so her carriage is real and
    // wrong rather than malformed
    s.client("carol")
        .begin(Some(kh("bob")), vec![], true)
        .unwrap();

    let oa = s.client("alice").optical_contribution().unwrap();
    let ob = s.client("bob").optical_contribution().unwrap();
    let oc = s.client("carol").optical_contribution().unwrap();

    // A QR THAT NAMES SOMEBODY ELSE is not this ceremony's.
    assert!(
        matches!(s.client("alice").take_optical(&oc), Err(Abort::NotActive)),
        "the QR names carol, and alice is meeting bob"
    );

    s.client("alice").take_optical(&ob).unwrap();
    s.client("bob").take_optical(&oa).unwrap();
    s.client("carol").take_optical(&ob).unwrap();

    // A CEREMONY-ID THAT IS NOT MINE is where a man in the middle shows.
    let tc = s.client("carol").transcript_confirm().unwrap();
    assert!(
        matches!(
            s.client("alice").take_transcript(&tc),
            Err(Abort::CeremonyIdMismatch)
        ),
        "carol computed a different ceremony and alice refuses it"
    );

    let ta = s.client("alice").transcript_confirm().unwrap();
    let tb = s.client("bob").transcript_confirm().unwrap();
    s.client("alice").take_transcript(&tb).unwrap();
    s.client("bob").take_transcript(&ta).unwrap();

    // AN INTENT ECHOING A CONTRIBUTION THE SCREEN DID NOT SHOW: the bearer
    // disagrees with the screen (§14.3.2), and this is the check the shell
    // used to be left to make.
    let mut forged = s.client("bob").intent_carriage().unwrap();
    forged[0] = {
        let mut i = rhtn_client::local::IntentExchange::decode(&forged[0]).unwrap();
        i.contribution = [7u8; 16];
        i.encode()
    };
    assert!(
        matches!(
            s.client("alice").take_intent_carriage(kh("bob"), &forged),
            Err(Abort::ContributionMismatch)
        ),
        "the echo does not match what alice read off bob's screen"
    );

    // AN ANCHORED MESSAGE FROM ANOTHER CEREMONY, in every anchored kind.
    s.client("carol").take_transcript(&tb).unwrap_err();
    let cid_c = rhtn_client::local::TranscriptConfirm::decode(&tc)
        .unwrap()
        .ceremony_id;
    let stray = rhtn_client::local::ProximityOutcomes {
        ceremony_id: cid_c,
        channels: vec![ChannelOutcome {
            kind: ChannelKind::Nfc,
            result: ChannelResult::Pass,
            resolution_m: None,
        }],
    }
    .encode();
    assert!(matches!(
        s.client("alice").take_proximity(&stray),
        Err(Abort::CeremonyIdMismatch)
    ));
    let stray = rhtn_client::local::CandidateHandover {
        ceremony_id: cid_c,
        candidates: vec![0x81, 0x83, 0x00, 0x44, 10, 0, 0, 1, 0x19, 0x1f, 0x90],
    }
    .encode();
    assert!(matches!(
        s.client("alice").take_candidate_carriage(&stray),
        Err(Abort::CeremonyIdMismatch)
    ));
    // a key from another ceremony is refused, and carol's real one no
    // less than an invented one (§14.3.2)
    let stray = rhtn_client::local::CaptureKeyHandover {
        ceremony_id: cid_c,
        key: [5u8; 32].into(),
    }
    .encode();
    assert!(matches!(
        s.client("alice").take_capture_key_carriage(&stray),
        Err(Abort::CeremonyIdMismatch)
    ));

    // and bytes that are not a §14.3 object at all
    assert!(matches!(
        s.client("alice").take_proximity(&[0x00]),
        Err(Abort::Malformed(_))
    ));
}

/// Two different refusals for two different states: no ceremony, and a
/// ceremony whose id the counterparty's contribution has not fixed yet.
/// A carriage taken before the screens have agreed is the second, and a
/// shell that reads it as the first would be told its live meeting is
/// not there.
#[test]
fn an_anchor_asked_for_before_the_id_is_fixed_is_its_own_refusal() {
    let mut s = setup(&["alice", "bob"], &[ChannelKind::Nfc]);
    assert!(matches!(
        s.client("alice").capture_key_carriage(),
        Err(Abort::NotActive)
    ));
    s.face_off("alice", "bob");
    s.client("alice")
        .begin(Some(kh("bob")), vec![], true)
        .unwrap();
    s.client("bob")
        .begin(Some(kh("alice")), vec![], false)
        .unwrap();
    // begun, and the id not yet fixed: the contributions have not crossed
    assert!(matches!(
        s.client("alice").capture_key_carriage(),
        Err(Abort::NoCeremonyId)
    ));
    assert!(matches!(
        s.client("alice").capture_key(),
        Err(Abort::NoCeremonyId)
    ));
    let stray = rhtn_client::local::CaptureKeyHandover {
        ceremony_id: [7u8; 32],
        key: [5u8; 32].into(),
    }
    .encode();
    assert!(matches!(
        s.client("alice").take_capture_key_carriage(&stray),
        Err(Abort::NoCeremonyId)
    ));
    // the contributions cross, and the id is still not fixed: agreement on
    // the second QR is what fixes it (§14.3.1), not the first
    let oa = s.client("alice").optical_contribution().unwrap();
    let ob = s.client("bob").optical_contribution().unwrap();
    s.client("alice").take_optical(&ob).unwrap();
    s.client("bob").take_optical(&oa).unwrap();
    assert!(matches!(
        s.client("alice").capture_key_carriage(),
        Err(Abort::NoCeremonyId)
    ));
    let ta = s.client("alice").transcript_confirm().unwrap();
    let tb = s.client("bob").transcript_confirm().unwrap();
    s.client("alice").take_transcript(&tb).unwrap();
    s.client("bob").take_transcript(&ta).unwrap();
    // fixed: the same calls answer
    assert!(s.client("alice").capture_key_carriage().is_ok());
}
