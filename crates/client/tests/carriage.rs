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

// functional: NET-033
/// **The number a device reports for itself crosses with the intent, both
/// ways, and lands on the counterparty's contact** (NET-033;
/// `light-client-requirements.md` §1.5; `wire-format.md` §14.3.2's ninth
/// field) [author, 2026-11-10].
///
/// **One side declares one and the other does not**, which is the shape
/// the bench has: a handset without a subscription sends nine fields, and
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

/// A bare `NetworkPoint` for 10.0.0.4:7432 (`wire-format.md` §4.4).
const POINT: [u8; 11] = [0xa2, 0x01, 0x44, 10, 0, 0, 4, 0x03, 0x19, 0x1d, 0x08];

/// **The opening over the bearer, every message as bytes**: the screens,
/// then both intents, between `a` who showed the invitation and `b` who
/// read it; what each declared beforehand rides its own.
fn opened(s: &mut Setup, a: &str, b: &str) {
    s.face_off(a, b);
    s.client(a)
        .begin(Some(kh(b)), vec![], true)
        .expect("begins");
    s.client(b)
        .begin(Some(kh(a)), vec![], false)
        .expect("begins");
    let oa = s.client(a).optical_contribution().unwrap();
    let ob = s.client(b).optical_contribution().unwrap();
    s.client(a).take_optical(&carry(&ob)).unwrap();
    s.client(b).take_optical(&carry(&oa)).unwrap();
    let ta = s.client(a).transcript_confirm().unwrap();
    let tb = s.client(b).transcript_confirm().unwrap();
    s.client(a).take_transcript(&carry(&tb)).unwrap();
    s.client(b).take_transcript(&carry(&ta)).unwrap();
    let ia = s.client(a).intent_carriage().unwrap();
    let ib = s.client(b).intent_carriage().unwrap();
    let carried: Vec<Vec<u8>> = ia.iter().map(|m| carry(m)).collect();
    s.client(b).take_intent_carriage(kh(a), &carried).unwrap();
    let carried: Vec<Vec<u8>> = ib.iter().map(|m| carry(m)).collect();
    s.client(a).take_intent_carriage(kh(b), &carried).unwrap();
}

// functional: NET-036
/// **The patron's node crosses beside its intent and is held by the member
/// as an invitation once the adoption is** [author, 2026-11-21]: *"auto-
/// invite via the bluetooth channel during an adoption into a subnet where
/// a node already exists"* — held exactly as a link's is, unproved, for
/// the patron's node on the subnet the adoption placed the member in, and
/// still the member's one tap. Nothing is agreed and nothing attaches.
#[test]
fn a_patrons_node_rides_the_adoption_meeting_and_is_held_as_an_invitation() {
    use rhtn_model::Standing;
    let mut s = setup(&["alice", "bob", "witness"]);
    // bob adopts alice; bob's own node serves the subnet, and alice has
    // no number to be sent a link at
    s.client("bob")
        .declare_node(Some((POINT.to_vec(), kh("bob"))))
        .unwrap();
    s.client("alice").declare_node(None).unwrap();
    opened(&mut s, "alice", "bob");
    assert!(
        s.client("alice").relocations().is_empty(),
        "nothing is held before the adoption says whose member this is"
    );
    let pop = s.met("alice", "bob", "witness");
    s.h.adopt(kh("alice"), kh("bob"), pop, 1)
        .expect("bob adopts alice");

    let a = s.client("alice");
    let r = a.relocation(&kh("bob")).expect("held for bob's node");
    assert_eq!(
        (r.standing, r.from.is_empty(), r.to.as_str()),
        (Standing::Unproved, true, "10.0.0.4:7432"),
        "a first invitation, as a link lands one"
    );
    assert!(
        a.agreed().is_empty(),
        "nothing agreed before the person answers"
    );
    let row = a
        .subnets_held()
        .into_iter()
        .find(|r| r.patron == Some(kh("bob")))
        .expect("the subnet bob placed alice in");
    assert_eq!(
        row.relocation.map(|r| r.node),
        Some(kh("bob")),
        "on the subnet the adoption placed her in"
    );
    assert_eq!(a.take_adoption_invited(), Some(kh("bob")), "told once");
    assert_eq!(a.take_adoption_invited(), None);
    assert!(
        s.client("bob").relocations().is_empty(),
        "the patron holds no invitation of its own"
    );
}

/// **The opening as [`opened`]'s, with `b`'s intent reaching `a` as `node`
/// rewrote its eleventh field**: a sender whose field is not what its own
/// kernel would carry, as another build could send it. The intent crosses
/// as the struct the bearer's bytes decode to.
fn opened_carrying(
    s: &mut Setup,
    a: &str,
    b: &str,
    node: impl FnOnce(
        Option<rhtn_client::local::CarriedNode>,
    ) -> Option<rhtn_client::local::CarriedNode>,
) {
    s.face_off(a, b);
    s.client(a)
        .begin(Some(kh(b)), vec![], true)
        .expect("begins");
    s.client(b)
        .begin(Some(kh(a)), vec![], false)
        .expect("begins");
    let oa = s.client(a).optical_contribution().unwrap();
    let ob = s.client(b).optical_contribution().unwrap();
    s.client(a).take_optical(&carry(&ob)).unwrap();
    s.client(b).take_optical(&carry(&oa)).unwrap();
    let ta = s.client(a).transcript_confirm().unwrap();
    let tb = s.client(b).transcript_confirm().unwrap();
    s.client(a).take_transcript(&carry(&tb)).unwrap();
    s.client(b).take_transcript(&carry(&ta)).unwrap();
    let ia = s.client(a).intent_carriage().unwrap();
    let mut ib = s.client(b).intent().unwrap();
    ib.node = node(ib.node.take());
    let carried: Vec<Vec<u8>> = ia.iter().map(|m| carry(m)).collect();
    s.client(b).take_intent_carriage(kh(a), &carried).unwrap();
    s.client(a).take_intent(kh(b), &ib).unwrap();
}

/// `who`'s adoption by `by`, as `who`'s archive holds it.
fn adoption_of(s: &mut Setup, who: &str, by: &str) -> Vec<u8> {
    s.client(who)
        .archive
        .records()
        .find(|r| {
            r.tx_type == rhtn_archive::tx::TYPE_ADOPTION
                && r.field_hash(1) == Some(kh(who))
                && r.field_hash(2) == Some(kh(by))
        })
        .map(|r| r.bytes.clone())
        .expect("the adoption is held")
}

/// The subnet `who` holds a position in under `patron`.
fn subnet_under(s: &mut Setup, who: &str, patron: &str) -> rhtn_client::Txid {
    s.client(who)
        .subnets_held()
        .into_iter()
        .find(|r| r.patron == Some(kh(patron)))
        .expect("a position under that patron")
        .id
}

// functional: NET-036
/// **A patron with no node of its own carries its patron's, with what
/// proves it** [author, 2026-11-22]: *"If it's necessary to prove the
/// patronage chain to the node operator, that should only require the
/// patron's adoption by the node operator, not every contact or
/// transaction."* Carol roots the subnet and runs its node; bob, under
/// her, has none, and adopting alice carries carol's node with his
/// adoption by carol and carol's material. Alice, **who holds nothing of
/// carol beforehand**, verifies that adoption, pins carol's material and
/// holds the node as a first invitation on that subnet — carol being no
/// patron of hers and nothing agreed with her. She files nothing of bob's
/// relationship and makes carol no contact.
#[test]
fn a_patron_without_a_node_carries_its_patrons_with_its_adoption() {
    use rhtn_model::Standing;
    let mut s = setup(&["alice", "bob", "carol", "witness"]);
    s.stranger("alice", "carol");
    s.adopted("bob", "carol", "witness", 1);
    let subnet = subnet_under(&mut s, "bob", "carol");
    s.client("bob")
        .declare_node(Some((POINT.to_vec(), kh("carol"))))
        .expect("carol is bob's patron, and he holds both");
    s.client("alice").declare_node(None).unwrap();
    opened(&mut s, "alice", "bob");
    let knows_carol = |s: &mut Setup| {
        s.client("alice")
            .known
            .iter()
            .any(|k| k.keyhash == kh("carol"))
    };
    assert!(
        !knows_carol(&mut s),
        "nothing of carol before the adoption says whose member alice is"
    );
    let pop = s.met("alice", "bob", "witness");
    s.h.adopt(kh("alice"), kh("bob"), pop, 1)
        .expect("bob adopts alice");

    assert!(knows_carol(&mut s), "carol's material, from the meeting");
    let carols = adoption_of(&mut s, "bob", "carol");
    let a = s.client("alice");
    let row = a
        .subnets_held()
        .into_iter()
        .find(|r| r.patron == Some(kh("bob")))
        .expect("the subnet bob placed alice in");
    assert_eq!(row.id, subnet, "carol's, through bob");
    let r = a.relocation(&kh("carol")).expect("held for carol's node");
    assert_eq!(
        (r.standing, r.from.is_empty(), r.to.as_str()),
        (Standing::Unproved, true, "10.0.0.4:7432"),
        "a first invitation, unproved"
    );
    assert_eq!(
        row.relocation.map(|r| r.node),
        Some(kh("carol")),
        "on the subnet the adoption placed her in"
    );
    assert!(a.relocation(&kh("bob")).is_none(), "nothing for bob's");
    assert!(
        a.agreed().is_empty(),
        "nothing agreed before the person answers"
    );
    assert!(
        a.contacts_held().iter().all(|c| c.keyhash != kh("carol")),
        "carol is no contact of alice's"
    );
    assert!(
        a.archive.records().all(|r| r.bytes != carols),
        "bob's adoption by carol is not filed in alice's archive"
    );
    assert_eq!(a.take_adoption_invited(), Some(kh("carol")), "told once");
    assert!(
        s.client("bob").relocations().is_empty(),
        "the patron holds no invitation of its own"
    );
}

// functional: NET-036
/// **A patron's node whose adoption does not prove it leaves nothing**
/// [author, 2026-11-22]: material not that of the patron the adoption
/// names, an adoption of somebody else, one into another subnet than the
/// member's, or one whose signature fails. Each member keeps no invitation
/// and pins nothing; the genuine carriage, in the same arrangement, is
/// held, so each refusal is of what the forgery changed.
#[test]
fn a_patrons_node_on_an_adoption_that_does_not_prove_it_leaves_nothing() {
    use rhtn_client::local::CarriedNode;
    let members = ["alice", "c3", "c4", "c5", "alice2"];
    let mut s = setup(&[
        "alice", "alice2", "bob", "carol", "c1", "c2", "c3", "c4", "c5", "witness",
    ]);
    for m in members {
        for whom in ["carol", "c1", "c2"] {
            s.stranger(m, whom);
        }
    }
    // carol roots one subnet, with bob and c2 under her; c1 roots
    // another, with bob under it too
    s.adopted("bob", "carol", "witness", 1);
    s.adopted("c2", "carol", "witness", 1);
    s.adopted("bob", "c1", "witness", 2);
    // the subnet bob adopts into is the one he acts in; the other patron's
    // adoption of him is the one from another subnet
    let acting = s.client("bob").acting_position();
    let in_carols = s
        .client("bob")
        .subnets_held()
        .into_iter()
        .find(|r| r.patron == Some(kh("carol")))
        .and_then(|r| r.position)
        .is_some_and(|p| p.anchor == acting.anchor && p.path == acting.path);
    let (here, elsewhere) = if in_carols {
        ("carol", "c1")
    } else {
        ("c1", "carol")
    };
    let material = |who: &str| id(who).public.key_material();
    let genuine = adoption_of(&mut s, "bob", here);
    let mut broken = genuine.clone();
    *broken.last_mut().unwrap() ^= 1;
    let carried = [
        (genuine.clone(), material(here), "genuine"),
        (genuine, material("witness"), "material not the patron's"),
        (
            adoption_of(&mut s, "c2", "carol"),
            material("carol"),
            "an adoption of somebody else",
        ),
        (
            adoption_of(&mut s, "bob", elsewhere),
            material(elsewhere),
            "an adoption into another subnet",
        ),
        (broken, material(here), "a signature that fails"),
    ];
    for (member, (adoption, material, why)) in members.into_iter().zip(carried) {
        opened_carrying(&mut s, member, "bob", |_| {
            Some(CarriedNode::Patrons {
                point: POINT.to_vec(),
                adoption,
                material,
            })
        });
        let pop = s.met(member, "bob", "witness");
        s.h.adopt(kh(member), kh("bob"), pop, 1)
            .expect("bob adopts the member");
        let m = s.client(member);
        let pinned: Vec<&str> = ["carol", "c1", "c2", "witness"]
            .into_iter()
            .filter(|w| m.known.iter().any(|k| k.keyhash == kh(w)))
            .collect();
        if why == "genuine" {
            assert!(m.relocation(&kh(here)).is_some(), "{why}: held");
            assert_eq!(pinned, [here, "witness"], "{why}: the patron's pinned");
            assert_eq!(m.take_adoption_invited(), Some(kh(here)), "{why}");
            continue;
        }
        assert!(m.relocations().is_empty(), "{why}: no invitation");
        assert_eq!(m.take_adoption_invited(), None, "{why}");
        assert_eq!(pinned, ["witness"], "{why}: nothing pinned");
    }
}

// functional: NET-036
/// **The sender carries only its own node or a patron's, and a patron's
/// only with what proves it** [author, 2026-11-22]: declaring a node that
/// is neither is refused, as is a patron's whose material is not held,
/// and the refusal clears what was declared before it, so the meeting
/// carries nothing.
#[test]
fn a_node_declared_is_own_or_a_patrons_with_what_proves_it() {
    let mut s = setup(&["alice", "bob", "carol", "witness"]);
    assert!(
        s.client("bob")
            .declare_node(Some((POINT.to_vec(), kh("carol"))))
            .is_err(),
        "carol is no patron of bob's yet"
    );
    s.adopted("bob", "carol", "witness", 1);
    assert!(
        s.client("bob")
            .declare_node(Some((POINT.to_vec(), kh("witness"))))
            .is_err(),
        "a party met is not a patron"
    );
    s.client("bob")
        .declare_node(Some((POINT.to_vec(), kh("bob"))))
        .expect("his own");
    // the material off what bob verifies by, as a client that never held
    // it would stand
    s.client("bob").known.retain(|k| k.keyhash != kh("carol"));
    assert!(
        s.client("bob")
            .declare_node(Some((POINT.to_vec(), kh("carol"))))
            .is_err(),
        "a patron whose material is not held"
    );
    // and nothing is left of the own node declared before the refusal
    opened(&mut s, "alice", "bob");
    let pop = s.met("alice", "bob", "witness");
    s.h.adopt(kh("alice"), kh("bob"), pop, 1)
        .expect("bob adopts alice");
    assert!(s.client("alice").relocations().is_empty());
    assert_eq!(s.client("alice").take_adoption_invited(), None);
}

// functional: NET-036
/// **A node from anybody who is not this identity's patron after the
/// ceremony is dropped with nothing kept**: one carried at a meeting with
/// no adoption, the sender's own or its patron's with what proves it,
/// which a later adoption at another meeting does not revive, and one the
/// member's own side declared.
#[test]
fn a_node_from_anybody_but_the_adopting_patron_leaves_nothing() {
    let mut s = setup(&["alice", "bob", "carol", "witness"]);
    s.stranger("alice", "carol");
    s.adopted("bob", "carol", "witness", 1);
    // a regular meeting: bob carries a node, nobody is adopted
    s.client("bob")
        .declare_node(Some((POINT.to_vec(), kh("bob"))))
        .unwrap();
    opened(&mut s, "alice", "bob");
    s.met("alice", "bob", "witness");
    assert!(s.client("alice").relocations().is_empty());
    assert_eq!(s.client("alice").take_adoption_invited(), None);
    // and his patron's, with what proves it, at a meeting adopting
    // nobody: it introduces nobody, and nothing of carol is pinned
    s.client("bob")
        .declare_node(Some((POINT.to_vec(), kh("carol"))))
        .unwrap();
    opened(&mut s, "alice", "bob");
    s.met("alice", "bob", "witness");
    assert!(s.client("alice").relocations().is_empty());
    assert_eq!(s.client("alice").take_adoption_invited(), None);
    assert!(
        s.client("alice")
            .known
            .iter()
            .all(|k| k.keyhash != kh("carol")),
        "carol's material is not kept from a meeting adopting nobody"
    );

    // the adoption that follows rests on a meeting of its own, which
    // carried none, so the first meeting's node is gone with it
    s.client("bob").declare_node(None).unwrap();
    // and the member declaring one is not the patron offering it
    s.client("alice")
        .declare_node(Some((POINT.to_vec(), kh("alice"))))
        .unwrap();
    opened(&mut s, "alice", "bob");
    let pop = s.met("alice", "bob", "witness");
    s.h.adopt(kh("alice"), kh("bob"), pop, 1)
        .expect("bob adopts alice");
    for who in ["alice", "bob"] {
        assert!(
            s.client(who).relocations().is_empty(),
            "{who} holds nothing"
        );
        assert_eq!(s.client(who).take_adoption_invited(), None, "{who}");
    }
}

// functional: NET-036
/// **What is declared is one bare point**: one with a zone, or bytes that
/// are not a point, is refused and nothing is declared.
#[test]
fn a_node_declared_is_a_bare_point_or_nothing() {
    let mut s = setup(&["alice"]);
    let a = s.client("alice");
    let zoned: Vec<u8> = [
        &[
            0xa3, 0x01, 0x44, 10, 0, 0, 4, 0x02, 0x19, 0xfb, 0xff, 0x04, 0x64,
        ][..],
        b"nyc1",
    ]
    .concat();
    let me = kh("alice");
    assert!(
        a.declare_node(Some((zoned, me))).is_err(),
        "a zone rides signed records only"
    );
    assert!(
        a.declare_node(Some((vec![0xa0], me))).is_err(),
        "an empty map"
    );
    assert!(
        a.declare_node(Some(([&POINT[..], &[0x00]].concat(), me)))
            .is_err(),
        "trailing bytes"
    );
    assert!(a.declare_node(Some((POINT.to_vec(), me))).is_ok());
    assert!(a.declare_node(None).is_ok(), "and cleared");
}
