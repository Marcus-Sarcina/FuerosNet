//! The ceremony's opening over the bearer, between two live clients
//! (`wire-format.md` §14.3): the optical exchange, then the intent, the
//! candidates and the capture keys, every one of
//! them as the bytes the shell carries and none of them as fields.

mod common;

use common::harness::*;
use common::*;
use rhtn_client::ceremony::*;

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
    let mut s = setup(&["alice", "bob"]);
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
    let mut s = setup(&["alice", "bob", "carol"]);
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
    // **The forger must hold the session key**, which §14.3.1 concedes a
    // party that read both screens does.  Since §14.3.2 every carriage is
    // sealed, so this is the only adversary that reaches the echo check at
    // all: anybody else is refused as Unsealable first.
    let mut forged = s.client("bob").intent_carriage().unwrap();
    forged[0] = {
        let plain = s.client("alice").open_as_co_present(&forged[0]).unwrap();
        let mut i = rhtn_client::local::IntentExchange::decode(&plain).unwrap();
        i.contribution = [7u8; 16];
        s.client("alice").seal_as_co_present(&i.encode()).unwrap()
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
    // **Two layers now, and the key is the first.**  Carol's own carriage
    // is sealed under carol's session key, so alice cannot open it: the
    // per-ceremony key separates the two before any anchor is compared.
    let stray = rhtn_client::local::CandidateHandover {
        ceremony_id: cid_c,
        candidates: vec![0x81, 0x83, 0x00, 0x44, 10, 0, 0, 1, 0x19, 0x1f, 0x90],
    }
    .encode();
    assert!(matches!(
        s.client("alice").take_candidate_carriage(&stray),
        Err(Abort::Unsealable)
    ));
    // and the anchor check is still what catches a holder of alice's key
    // re-anchoring another ceremony's message, which is the attack
    // §14.3.1 concedes and the theory proves reachable
    let resealed = s.client("alice").seal_as_co_present(&stray).unwrap();
    assert!(matches!(
        s.client("alice").take_candidate_carriage(&resealed),
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
        Err(Abort::Unsealable)
    ));
    let resealed = s.client("alice").seal_as_co_present(&stray).unwrap();
    assert!(matches!(
        s.client("alice").take_capture_key_carriage(&resealed),
        Err(Abort::CeremonyIdMismatch)
    ));

    // and bytes that are not a §14.3 object at all, in both layers: raw
    // garbage never reaches the decoder, and garbage a key holder sealed
    // does
    assert!(matches!(
        s.client("alice").take_capture_key_carriage(&[0x00]),
        Err(Abort::Unsealable)
    ));
    let sealed_garbage = s.client("alice").seal_as_co_present(&[0x00]).unwrap();
    assert!(matches!(
        s.client("alice").take_capture_key_carriage(&sealed_garbage),
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
    let mut s = setup(&["alice", "bob"]);
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

/// **The prekey handover, and what it is for** (`wire-format.md` §14.3.4):
/// two devices that have just met hand each other a signed bundle and one
/// one-time key across the local session, and can then open an end-to-end
/// channel with **no node asked for anything** — which is the case design
/// §6.4's escape leaves behind, a pair holding a record and otherwise
/// unable to say a word to each other.
#[test]
fn a_handover_opens_an_end_to_end_channel_with_no_node_asked() {
    let mut s = setup(&["alice", "bob"]);
    s.face_off("alice", "bob");
    s.client("alice")
        .begin(Some(kh("bob")), vec![], true)
        .unwrap();
    s.client("bob")
        .begin(Some(kh("alice")), vec![], false)
        .unwrap();
    let oa = s.client("alice").optical_contribution().unwrap();
    let ob = s.client("bob").optical_contribution().unwrap();
    s.client("alice").take_optical(&ob).unwrap();
    s.client("bob").take_optical(&oa).unwrap();
    let ta = s.client("alice").transcript_confirm().unwrap();
    let tb = s.client("bob").transcript_confirm().unwrap();
    s.client("alice").take_transcript(&tb).unwrap();
    s.client("bob").take_transcript(&ta).unwrap();

    // each hands over its bundle and a one-time key, sealed under the
    // local session like every other carriage
    let ha = s
        .client("alice")
        .prekey_carriage()
        .expect("alice hands over");
    let hb = s.client("bob").prekey_carriage().expect("bob hands over");
    s.client("alice")
        .take_prekey_carriage(&hb)
        .expect("alice takes bob's");
    s.client("bob")
        .take_prekey_carriage(&ha)
        .expect("bob takes alice's");

    // and now a message opens a session with no `PrekeyRequest` at all:
    // nothing here is addressed to a node
    let msgs = s
        .client("alice")
        .send_payload(
            kh("bob"),
            rhtn_client::payload::KIND_APPLICATION,
            b"met you just now",
        )
        .expect("a session opens from what was handed over");
    assert!(
        !msgs.iter().any(|m| matches!(m, Msg::PrekeyRequest(_))),
        "nothing is asked of a node: {msgs:?}"
    );
    assert_eq!(msgs.len(), 1, "one initial message, to bob's one device");

    // bob opens it, which is the whole claim
    let bytes = match msgs.first() {
        Some(Msg::Relay { bytes, .. }) | Some(Msg::Payload { bytes, .. }) => bytes,
        other => panic!("an initial message for bob: {other:?}"),
    };
    let d = s
        .client("bob")
        .receive_payload(kh("alice"), bytes)
        .expect("bob opens it");
    assert!(
        matches!(&d, Dispatched::Application { bytes, .. } if bytes == b"met you just now"),
        "what crossed: {d:?}"
    );
}

/// A handover of another ceremony is refused, and one whose bundle does
/// not verify is refused however it arrived: the signature is what makes
/// the material trustworthy and never the channel.
#[test]
fn a_handover_is_checked_against_this_ceremony_and_its_own_signature() {
    let mut s = setup(&["alice", "bob", "carol"]);
    s.face_off("alice", "bob");
    s.client("alice")
        .begin(Some(kh("bob")), vec![], true)
        .unwrap();
    s.client("bob")
        .begin(Some(kh("alice")), vec![], false)
        .unwrap();
    let oa = s.client("alice").optical_contribution().unwrap();
    let ob = s.client("bob").optical_contribution().unwrap();
    s.client("alice").take_optical(&ob).unwrap();
    s.client("bob").take_optical(&oa).unwrap();
    let ta = s.client("alice").transcript_confirm().unwrap();
    let tb = s.client("bob").transcript_confirm().unwrap();
    s.client("alice").take_transcript(&tb).unwrap();
    s.client("bob").take_transcript(&ta).unwrap();

    // bob's own handover, offered back to bob: sealed under the session
    // they share, so it opens — and names bob where alice is expected
    let hb = s.client("bob").prekey_carriage().unwrap();
    let why = s
        .client("bob")
        .take_prekey_carriage(&hb)
        .expect_err("refused");
    assert!(
        format!("{why:?}").contains("another party") || format!("{why:?}").contains("Malformed"),
        "a bundle for the wrong party: {why:?}"
    );

    // and bytes from no session of this ceremony do not open at all
    let refused = s
        .client("alice")
        .take_prekey_carriage(&[7u8; 64])
        .expect_err("refused");
    assert!(matches!(refused, Abort::Unsealable), "{refused:?}");
}

// functional: NET-033
/// **The number a device reports for itself crosses with the intent, both
/// ways, and lands on the counterparty's contact** (NET-033;
/// `light-client-requirements.md` §1.5; `wire-format.md` §14.3.2's ninth
/// field) [author, 2026-11-10].
///
/// **One side declares one and the other does not**, which is the shape
/// the bench has: a handset without a subscription sends eight fields, and
/// a meeting where only one number crosses is unremarkable in every other
/// respect.
#[test]
fn the_number_a_device_reports_crosses_with_the_intent_and_lands_on_the_contact() {
    let mut s = setup(&["alice", "bob"]);
    s.face_off("alice", "bob");
    // alice's handset answered; bob's did not, which is the ordinary case
    s.client("alice")
        .declare_number(Some("+15550001111".into()));
    s.client("bob").declare_number(None);
    s.client("alice")
        .begin(Some(kh("bob")), vec![], true)
        .expect("alice begins");
    s.client("bob")
        .begin(Some(kh("alice")), vec![], false)
        .expect("bob begins");

    let oa = s.client("alice").optical_contribution().unwrap();
    let ob = s.client("bob").optical_contribution().unwrap();
    s.client("alice").take_optical(&carry(&ob)).unwrap();
    s.client("bob").take_optical(&carry(&oa)).unwrap();
    let ta = s.client("alice").transcript_confirm().unwrap();
    let tb = s.client("bob").transcript_confirm().unwrap();
    s.client("alice").take_transcript(&carry(&tb)).unwrap();
    s.client("bob").take_transcript(&carry(&ta)).unwrap();

    let ia = s.client("alice").intent_carriage().unwrap();
    let ib = s.client("bob").intent_carriage().unwrap();
    // the carriages are sealed, so what they carry is asserted where it
    // lands rather than read off the wire here; the encoding itself is
    // `local.rs`'s `an_intent_carries_a_number_or_does_not`
    let carried: Vec<Vec<u8>> = ia.iter().map(|m| carry(m)).collect();
    s.client("bob")
        .take_intent_carriage(kh("alice"), &carried)
        .unwrap();
    let carried: Vec<Vec<u8>> = ib.iter().map(|m| carry(m)).collect();
    s.client("alice")
        .take_intent_carriage(kh("bob"), &carried)
        .unwrap();

    // bob keeps what alice's device reported, with the rest of what it
    // holds of alice
    assert_eq!(
        s.client("bob").numbers(),
        vec![(kh("alice"), "+15550001111".to_string())],
        "the number is on bob's contact for alice"
    );
    // and alice holds none for bob, nothing having been sent or invented
    assert!(
        s.client("alice").numbers().is_empty(),
        "a device that sent no number leaves none behind"
    );
    // **and it is not in the card bob would hand alice on in** (§2.3):
    // bob's own bundle entries carry key material and locators alone
    for intro in s.client("bob").introductions() {
        assert!(
            !intro.windows(12).any(|w| w == b"+15550001111"),
            "an introduction carries no number"
        );
    }
}

// functional: NET-025
/// **The sender's encryption key crosses with its intent, both ways, and
/// lands on the counterparty's contact** (`wire-format.md` §14.3.2, §2.5):
/// after a ceremony that needed no node, each side holds the key the
/// other's identity root derives, so it can write afterwards with no fetch
/// of it. An intent whose key names another identity, or whose signature
/// fails, is refused whole and leaves nothing behind.
#[test]
fn the_encryption_key_crosses_with_the_intent_and_a_bad_one_refuses_it() {
    let mut s = setup(&["alice", "bob"]);
    s.face_off("alice", "bob");
    s.client("alice")
        .begin(Some(kh("bob")), vec![], true)
        .expect("alice begins");
    s.client("bob")
        .begin(Some(kh("alice")), vec![], false)
        .expect("bob begins");
    let oa = s.client("alice").optical_contribution().unwrap();
    let ob = s.client("bob").optical_contribution().unwrap();
    s.client("alice").take_optical(&carry(&ob)).unwrap();
    s.client("bob").take_optical(&carry(&oa)).unwrap();
    let ta = s.client("alice").transcript_confirm().unwrap();
    let tb = s.client("bob").transcript_confirm().unwrap();
    s.client("alice").take_transcript(&carry(&tb)).unwrap();
    s.client("bob").take_transcript(&carry(&ta)).unwrap();

    let ib = s.client("bob").intent().unwrap();
    let before = s.client("alice").ceremony_id();
    // alice's key in bob's intent names another identity; bob's own with
    // a byte of its signature changed does not verify under him
    let anothers = rhtn_crypto::encryption_key::own(&id("alice"));
    let mut tampered = rhtn_crypto::encryption_key::own(&id("bob"));
    *tampered.last_mut().unwrap() ^= 1;
    for (why, ek) in [("another's", anothers), ("tampered", tampered)] {
        let bad = Intent {
            encryption_key: ek,
            ..ib.clone()
        };
        let got = s.client("alice").take_intent(kh("bob"), &bad);
        assert!(
            matches!(got, Err(Abort::Malformed(_))),
            "{why}: refused, got {got:?}"
        );
        assert!(
            s.client("alice")
                .store
                .contacts
                .get(&kh("bob"))
                .and_then(|c| c.encryption_key.as_ref())
                .is_none(),
            "{why}: no key is kept"
        );
        assert_eq!(
            s.client("alice").ceremony_id(),
            before,
            "{why}: the ceremony stands where it stood"
        );
    }

    let ia = s.client("alice").intent_carriage().unwrap();
    let ib = s.client("bob").intent_carriage().unwrap();
    let carried: Vec<Vec<u8>> = ia.iter().map(|m| carry(m)).collect();
    s.client("bob")
        .take_intent_carriage(kh("alice"), &carried)
        .unwrap();
    let carried: Vec<Vec<u8>> = ib.iter().map(|m| carry(m)).collect();
    s.client("alice")
        .take_intent_carriage(kh("bob"), &carried)
        .unwrap();
    for (holder, of) in [("alice", "bob"), ("bob", "alice")] {
        assert_eq!(
            s.client(holder).store.contacts[&kh(of)].encryption_key,
            Some(rhtn_crypto::encryption_key::own(&id(of))),
            "{holder} holds the key {of}'s root derives"
        );
    }
}

// functional: NET-028
/// **A carriage that will not open says nothing about why** (`wire-format.md`
/// §14.3.2's Lifetime row; design §7.1).  The session key lives exactly as
/// long as the ceremony, so a device whose ceremony has stopped has let it
/// go and refuses the next step for want of a key, before reading a byte of
/// it.  A device that never began, one that finished and one that stopped
/// therefore answer alike, and a device live in some other ceremony answers
/// with the one other thing a receiver can say about bytes it holds no key
/// for.  Nothing in any refusal names a ceremony or a counterparty.
#[test]
fn a_carriage_to_a_device_without_the_key_is_refused_without_saying_why() {
    let mut s = setup(&["alice", "bob", "carol", "c1", "c2", "c3", "c4"]);
    // alice and bob are through the local exchanges; a conversation step
    // waits on alice's side for bob
    s.meet_locally("alice", "bob", &[], &[])
        .expect("the local exchanges");
    let steps = s.client("alice").carriages();
    assert!(
        !steps.is_empty(),
        "the exchanges leave alice with a step for bob"
    );
    let step = steps[0].clone();
    // c1 and c2 finished one: the record arrived and the ceremony closed
    // with it
    s.met_in_person("c1", "c2", "w1");
    // c3 and c4 are live in a ceremony of their own
    s.meet_locally("c3", "c4", &[], &[])
        .expect("another pair's exchanges");
    let _ = s.client("c3").carriages();

    // bob, live, opens it
    let live = s.client("bob").take_conversation_carriage(&step);
    assert!(
        live.is_ok(),
        "the device holding the key reads the step: {live:?}"
    );

    // bob stops; the same bytes are now refused as a device that never
    // began refuses them, and as one that finished does
    s.client("bob").abandon();
    let stopped = s
        .client("bob")
        .take_conversation_carriage(&step)
        .unwrap_err();
    let never = s
        .client("carol")
        .take_conversation_carriage(&step)
        .unwrap_err();
    let finished = s
        .client("c1")
        .take_conversation_carriage(&step)
        .unwrap_err();
    assert_eq!(stopped, Abort::NotActive, "refused for want of a key");
    assert_eq!(never, stopped, "a device that never began answers the same");
    assert_eq!(finished, stopped, "and so does one that finished");
    for (who, e) in [("bob", &stopped), ("carol", &never), ("c1", &finished)] {
        assert_eq!(
            format!("{e:?}"),
            "NotActive",
            "{who}'s refusal carries nothing"
        );
        assert_eq!(
            s.client(who).ceremony_id(),
            None,
            "{who} holds no ceremony afterwards"
        );
    }

    // a device live in another ceremony holds a key, and not this one: the
    // carriage does not open, which is all it can say
    let elsewhere = s
        .client("c3")
        .take_conversation_carriage(&step)
        .unwrap_err();
    assert_eq!(elsewhere, Abort::Unsealable);
    assert_eq!(format!("{elsewhere:?}"), "Unsealable");
}
