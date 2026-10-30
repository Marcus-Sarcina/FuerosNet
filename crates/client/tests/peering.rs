//! **A peering a light client makes** (`wire-format.md` §4.4; design §6.3).
//!
//! Peering is an edge between two parties who have met, carrying none of
//! the subnet's authority (design §15.1). A phone runs no instance, so
//! each party's field holds its locator rather than a network point —
//! "its network point where that party runs an instance, its locator
//! otherwise" [author, 2026-09-29] — and the presence record is required
//! unconditionally, peers sharing no prior relationship to draw on.

mod common;

use common::harness::*;
use common::*;
use rhtn_client::ceremony::Abort;

const PARTIES: [&str; 4] = ["alice", "bob", "w1", "w2"];

/// The locator in one of a peering body's address fields.
fn at(body: &[u8], field: u64) -> rhtn_archive::tx::Locator {
    let r = rhtn_codec::cbor::value_slice(body, field).expect("an address field");
    rhtn_archive::tx::Locator::decode(&body[r]).expect("a locator, since neither runs an instance")
}

/// A keyhash field of a body, past its own CBOR head.
fn key_at(body: &[u8], field: u64) -> [u8; 32] {
    let item = rhtn_codec::cbor::parse_all(body).expect("a body");
    let rhtn_codec::cbor::Item::Map(m) = &item else {
        panic!("a map")
    };
    match rhtn_codec::cbor::map_get(m, field) {
        Some(rhtn_codec::cbor::Item::Bytes(r)) => body[r.clone()].try_into().expect("32 bytes"),
        _ => panic!("field {field} is not a keyhash"),
    }
}

fn refusal(e: Abort) -> String {
    match e {
        Abort::NodeRefused(why) => why,
        other => panic!("not a refusal of the body: {other:?}"),
    }
}

// acceptance: REP-17
#[test]
fn two_parties_who_met_peer_by_locator_and_both_hold_the_record() {
    let mut s = setup(&PARTIES);
    // both are in a subnet, so both have a position to write down
    s.adopted("alice", "w1", "w2", 1);
    s.adopted("bob", "w1", "w2", 2);
    let pop = s.met_in_person("alice", "bob", "w2");

    let theirs = s.client("bob").back_pointers();
    let body = s
        .client("alice")
        .propose_peering(kh("bob"), pop, &theirs, Some(1 << 20))
        .expect("alice proposes");

    // the parties are in keyhash order and each field holds that party's
    // own position (§4.4): alice's as she holds it, bob's as she was
    // handed it
    let (lo, hi) = if kh("alice") <= kh("bob") {
        (kh("alice"), kh("bob"))
    } else {
        (kh("bob"), kh("alice"))
    };
    assert_eq!(key_at(&body, 1), lo);
    assert_eq!(key_at(&body, 2), hi);
    let field_of = |who: [u8; 32]| if who == lo { 3 } else { 4 };
    assert_eq!(
        at(&body, field_of(kh("alice"))).anchor,
        kh("w1"),
        "alice's position is in w1's subnet"
    );
    assert_eq!(at(&body, field_of(kh("bob"))).anchor, kh("w1"));

    // bob checks it against what he holds and signs; alice assembles
    let entries = s
        .client("bob")
        .review_and_sign_peering(&body)
        .expect("bob signs his own peering");
    let mine = s.client("alice").sign_body(&body).expect("alice signs");
    let env = rhtn_archive::tx::envelope_from_entries(
        rhtn_archive::tx::TYPE_PEERING,
        &body,
        &[(kh("alice"), mine), (kh("bob"), entries)],
    );
    let t = s
        .client("alice")
        .take_peering(&env)
        .expect("alice takes it");
    assert_eq!(s.client("bob").take_peering(&env).expect("bob takes it"), t);
    for n in ["alice", "bob"] {
        assert!(s.client(n).store.records.contains_key(&t), "{n} holds it");
    }
    // and each now counts the other among its peers, read from the record
    // and kept nowhere else (design §6.3)
    assert_eq!(s.client("alice").peers(), vec![kh("bob")]);
    assert_eq!(s.client("bob").peers(), vec![kh("alice")]);
    assert!(
        s.client("w1").peers().is_empty(),
        "a witness is nobody's peer"
    );
    // and a peering is no part of a position: both still sit where they sat
    let subnet = s.client("w1").subnets()[0];
    assert_eq!(
        s.client("alice").position_in(&subnet).map(|l| l.nibbles),
        Some(1)
    );
}

// acceptance: REP-17
#[test]
fn a_peering_rests_on_a_meeting_of_the_two_and_on_positions_each_holds() {
    let mut s = setup(&PARTIES);
    s.adopted("alice", "w1", "w2", 1);
    s.adopted("bob", "w1", "w2", 2);
    let theirs = s.client("bob").back_pointers();

    // a meeting of other parties is not evidence of this pair
    let elsewhere = s.met_in_person("alice", "w2", "w1");
    let why = refusal(
        s.client("alice")
            .propose_peering(kh("bob"), elsewhere, &theirs, None)
            .expect_err("refused"),
    );
    assert!(why.contains("other parties"), "{why}");

    // a record held nowhere here is refused one step earlier
    let why = refusal(
        s.client("alice")
            .propose_peering(kh("bob"), [9; 32], &theirs, None)
            .expect_err("refused"),
    );
    assert!(why.contains("held here"), "{why}");

    // and a peer whose position this client has never been handed has
    // nothing to write in its field
    let pop = s.met("bob", "w1", "w2");
    let unmet = s.met_in_person("alice", "bob", "w2");
    let _ = pop;
    s.client("alice")
        .store
        .contacts
        .get_mut(&kh("bob"))
        .unwrap()
        .locator = None;
    let why = refusal(
        s.client("alice")
            .propose_peering(kh("bob"), unmet, &theirs, None)
            .expect_err("refused"),
    );
    assert!(why.contains("no position is held"), "{why}");

    // a peering with oneself is not one
    assert!(matches!(
        s.client("alice")
            .propose_peering(kh("alice"), unmet, &theirs, None),
        Err(Abort::Malformed(_))
    ));
}

// acceptance: REP-17
#[test]
fn a_peering_body_is_refused_where_it_misstates_this_partys_own_side() {
    let mut s = setup(&PARTIES);
    s.adopted("alice", "w1", "w2", 1);
    s.adopted("bob", "w1", "w2", 2);
    let pop = s.met_in_person("alice", "bob", "w2");
    let theirs = s.client("bob").back_pointers();

    // a peering between two other parties is not bob's to sign
    let others = s.met_in_person("alice", "w2", "w1");
    let w2_back = s.client("w2").back_pointers();
    let elsewhere = s
        .client("alice")
        .propose_peering(kh("w2"), others, &w2_back, None)
        .expect("alice and w2 peer");
    let why = refusal(
        s.client("bob")
            .review_and_sign_peering(&elsewhere)
            .expect_err("refused"),
    );
    assert!(why.contains("other parties"), "{why}");

    // back-pointers that are not bob's archive's next
    let wrong = s
        .client("alice")
        .propose_peering(kh("bob"), pop, &[rhtn_archive::genesis(&kh("bob"))], None)
        .expect("proposed");
    let why = refusal(
        s.client("bob")
            .review_and_sign_peering(&wrong)
            .expect_err("refused"),
    );
    assert!(why.contains("back-pointers"), "{why}");

    // the body alice would build is signed
    let body = s
        .client("alice")
        .propose_peering(kh("bob"), pop, &theirs, None)
        .expect("proposed");
    assert!(s.client("bob").review_and_sign_peering(&body).is_ok());
}

/// Alice and bob, peered: the record held by both, as the test above
/// builds it.
fn peered(s: &mut common::harness::Setup) -> Vec<u8> {
    s.adopted("alice", "w1", "w2", 1);
    s.adopted("bob", "w1", "w2", 2);
    let pop = s.met_in_person("alice", "bob", "w2");
    let theirs = s.client("bob").back_pointers();
    let body = s
        .client("alice")
        .propose_peering(kh("bob"), pop, &theirs, Some(1 << 20))
        .expect("alice proposes");
    let entries = s
        .client("bob")
        .review_and_sign_peering(&body)
        .expect("bob signs");
    let mine = s.client("alice").sign_body(&body).expect("alice signs");
    let env = rhtn_archive::tx::envelope_from_entries(
        rhtn_archive::tx::TYPE_PEERING,
        &body,
        &[(kh("alice"), mine), (kh("bob"), entries)],
    );
    s.client("alice")
        .take_peering(&env)
        .expect("alice takes it");
    s.client("bob").take_peering(&env).expect("bob takes it");
    env
}

/// **What a peer holds, and what it gives back** (design §6.3, the author's
/// six steps of 2026-10-24): a blob under the owner's identity that the
/// peer cannot open; a contact the owner lost comes back from it on the
/// next contact; nothing the owner still holds is replaced; and the sealed
/// captures of others never cross.
// acceptance: TX-031, PAY-018
#[test]
fn a_peer_holds_a_blob_it_cannot_open_and_the_owner_recovers_a_lost_contact_from_it() {
    let mut s = setup(&PARTIES);
    let _ = peered(&mut s);
    assert!(
        s.client("alice").store.contacts.contains_key(&kh("w2")),
        "the witness is a contact"
    );
    // a sealed capture of somebody, which a peer must not be handed
    s.client("alice").store.sealed.insert(
        [9u8; 32],
        rhtn_client::store::SealedCapture {
            subject: kh("w2"),
            holder: kh("alice"),
            ceremony_id: [9u8; 32],
            modality: 1,
            template_version: 1,
            slots: vec![vec![1u8; 40]],
        },
    );

    // step 4: the deposit, as the arm that takes kind 27 keeps it
    let blob = s
        .client("alice")
        .export_for_peer()
        .expect("alice exports for a peer");
    assert!(
        s.client("bob").reconcile_from_peer(&blob).is_err(),
        "bob holds another key: the blob does not open for him"
    );
    s.client("bob")
        .store
        .held_for
        .insert(kh("alice"), blob.clone());
    let opened = s
        .client("alice")
        .import(&blob)
        .expect("the owner opens it")
        .0;
    assert!(
        opened.store.sealed.is_empty(),
        "the sealed captures of others did not cross"
    );
    assert!(opened.store.contacts.contains_key(&kh("w2")));

    // step 5: the macguffin goes
    assert!(s.client("alice").store.contacts.remove(&kh("w2")).is_some());

    // step 6: the return, reconciled
    let held = s.client("bob").store.held_for[&kh("alice")].clone();
    let r = s
        .client("alice")
        .reconcile_from_peer(&held)
        .expect("the owner takes its own blob");
    assert_eq!(r.contacts, 1, "the lost contact came back");
    assert_eq!(r.records, 0, "and no record was missing");
    assert!(s.client("alice").store.contacts.contains_key(&kh("w2")));

    // a second reconcile changes nothing: what is held is not replaced
    let r = s.client("alice").reconcile_from_peer(&held).expect("again");
    assert_eq!(r, rhtn_client::ceremony::Reconciled::default());

    // the bench's lever: every contact out, and the same return brings
    // every one back, each said to the shell as a change to the contacts
    let _ = s.client("alice").take_changed();
    let gone = s.client("alice").forget_contacts();
    assert!(
        gone >= 2,
        "the peer and the witnesses were contacts: {gone}"
    );
    assert!(s.client("alice").store.contacts.is_empty());
    assert!(
        s.client("alice")
            .take_changed()
            .contains(&rhtn_model::Noun::Contacts),
        "forgetting is a change"
    );
    let r = s
        .client("alice")
        .reconcile_from_peer(&held)
        .expect("the owner takes its own blob");
    assert_eq!(r.contacts, gone, "every forgotten contact came back");
    assert!(
        s.client("alice")
            .take_changed()
            .contains(&rhtn_model::Noun::Contacts),
        "and coming back is one"
    );
    assert_eq!(s.client("alice").forget_contacts(), gone);

    // and the greeting queues for a peer and for nobody else
    assert!(s.client("alice").greet_peer(kh("bob"), true).is_some());
    assert!(
        !s.client("alice").outbox().is_empty(),
        "a request and a deposit are queued"
    );
    assert!(
        s.client("alice").greet_peer(kh("w1"), true).is_none(),
        "a witness is not a peer"
    );
}

/// A session each way between two clients on the harness, as a bearer
/// handover gives it: each takes the other's prekeys, so a first message
/// opens with no node asked (`carriage.rs` has the full case).
fn sessions_between(s: &mut common::harness::Setup, a: &'static str, b: &'static str) {
    s.face_off(a, b);
    s.client(a).begin(Some(kh(b)), vec![], true).unwrap();
    s.client(b).begin(Some(kh(a)), vec![], false).unwrap();
    let oa = s.client(a).optical_contribution().unwrap();
    let ob = s.client(b).optical_contribution().unwrap();
    s.client(a).take_optical(&ob).unwrap();
    s.client(b).take_optical(&oa).unwrap();
    let ta = s.client(a).transcript_confirm().unwrap();
    let tb = s.client(b).transcript_confirm().unwrap();
    s.client(a).take_transcript(&tb).unwrap();
    s.client(b).take_transcript(&ta).unwrap();
    let ha = s.client(a).prekey_carriage().unwrap();
    let hb = s.client(b).prekey_carriage().unwrap();
    s.client(a).take_prekey_carriage(&hb).unwrap();
    s.client(b).take_prekey_carriage(&ha).unwrap();
    s.client(a).abandon();
    s.client(b).abandon();
}

/// Only `Msg::Payload`s, which is what a greeting queues: the locator,
/// the request, and the deposit where one goes.
fn payloads(msgs: &[rhtn_client::ceremony::Msg]) -> usize {
    use rhtn_client::ceremony::Msg;
    msgs.iter()
        .filter(|m| matches!(m, Msg::Payload { .. } | Msg::Relay { .. }))
        .count()
}

/// **A client restored from its phrase knows nobody and holds nothing;
/// its peers are what the serving node's replay names, and its dataset
/// is what a peer gives back** (design §6.3, §10.2).  A fresh alice,
/// handed the records the node holds of her — hers and bob's adoptions,
/// their peering — names bob as a peer before her archive is back; the
/// blob bob returns verifies under the material its own contacts carry,
/// and brings the records, the contacts, her name and bob as a peer of
/// her own.
// acceptance: TX-031
#[test]
fn a_restored_client_knows_its_peers_from_the_horizon_and_takes_its_dataset_from_the_blob() {
    let mut s = setup(&PARTIES);
    let peering = peered(&mut s);
    s.client("alice").set_default_alias("Alice").expect("named");
    // what the serving node holds of the two: every record either archive
    // holds, in the order they took effect, the peering among them
    let mut replay: std::collections::BTreeMap<(u64, [u8; 32]), (u64, Vec<u8>)> =
        std::collections::BTreeMap::new();
    for who in ["alice", "bob"] {
        for r in s.client(who).archive.records() {
            // the topology class does not carry presence records
            // (`wire-format.md` §10.1), so a node replays none
            if r.tx_type == rhtn_archive::tx::TYPE_PRESENCE {
                continue;
            }
            replay.insert((r.effective, r.txid), (r.tx_type, r.bytes.clone()));
        }
    }
    assert!(
        replay
            .values()
            .any(|(t, _)| *t == rhtn_archive::tx::TYPE_PEERING),
        "the peering is among them"
    );
    let blob = s
        .client("alice")
        .export_for_peer()
        .expect("alice's blob, as bob holds it");

    // a new process, the same identity, nothing else: what the phrase gives
    let mut fresh = setup(&["alice"]);
    let me = kh("alice");
    let c = fresh.client("alice");
    c.known.retain(|k| k.keyhash == me);
    assert!(c.peers().is_empty(), "nothing of her own: no peers");
    assert!(c.default_alias().is_none());
    // the serving node replays what it holds of her, and she can verify
    // none of it: every record is signed by somebody she does not hold
    // the material of, so each is held pending that material rather than
    // refused, as a node holds an object for a signer's key
    for (tx_type, rec) in replay.values() {
        let known = c.known.clone();
        let took = c.horizon.ingest(rec, &known);
        assert!(
            matches!(took, rhtn_client::horizon::Took::Pending),
            "the node's replay waits on material, type {tx_type}: {took:?}"
        );
    }
    assert_eq!(c.horizon.pending(), replay.len(), "all of it held");
    assert!(c.peers().is_empty(), "nobody is a peer she can name yet");
    // bob's first message carries his introduction: with his material the
    // peering — signed by the two of them alone — applies, and he is a
    // peer by the horizon alone, before anything of her own is back
    c.contact(
        &id("bob").public,
        rhtn_client::contacts::How::Introduced,
        true,
    );
    assert_eq!(
        c.peers(),
        vec![kh("bob")],
        "bob is a peer by the horizon alone"
    );
    assert!(
        c.horizon.pending() < replay.len(),
        "the peering applied; the adoptions wait on the witnesses"
    );
    assert!(
        c.greet_peer(kh("bob"), false).is_some(),
        "so he is greeted, for what he holds"
    );
    // with no session to him yet, the greeting asks for his bundle first
    // and waits (on the bench his own reopening has opened one by now)
    let out = c.outbox();
    assert!(
        out.iter()
            .any(|m| matches!(m, rhtn_client::ceremony::Msg::PrekeyRequest(_))),
        "the request waits on a session: {} messages",
        out.len()
    );
    assert_eq!(payloads(&out), 0, "nothing went on no session");
    let _ = peering;

    // and what bob holds comes back whole
    let r = c
        .reconcile_from_peer(&blob)
        .expect("her own blob opens under her key");
    assert_eq!(
        r.refused, 0,
        "every record verified: the contacts' material came first"
    );
    assert!(r.records >= 3, "adoptions and the peering: {}", r.records);
    assert!(r.contacts >= 2, "bob and the witnesses: {}", r.contacts);
    assert_eq!(c.default_alias(), Some("Alice"), "her name came back");
    assert!(
        c.known.iter().any(|k| k.keyhash == kh("bob")),
        "bob's material is hers to verify by"
    );
    assert_eq!(
        c.store.peers(&me),
        vec![kh("bob")],
        "and bob is a peer of her own records"
    );
    assert_eq!(
        c.horizon.pending(),
        0,
        "the witnesses' material came with the contacts"
    );
    assert!(
        c.take_changed().contains(&rhtn_model::Noun::Contacts),
        "the shell is told"
    );
}

/// **A peer silent for two rounds is opened to anew, and the deposit goes
/// again** (`light-client-requirements.md` §3's rule for a verifier that
/// lost its sessions, applied to a peer): the greeting on a held path
/// carries the deposit once per dataset; on an unheld one never; after
/// two rounds with no word back the session is set aside — the next
/// message asks for a bundle, as a first message does, and the harness
/// has no node to answer, so the asking is the proof — the deposit memory
/// with it, so the next held contact on a session deposits afresh; and
/// the silence allowed doubles.
// acceptance: TX-031
#[test]
fn a_peer_silent_for_two_rounds_is_opened_to_anew_and_deposited_to_again() {
    use rhtn_client::ceremony::Msg;
    let mut s = setup(&PARTIES);
    let _ = peered(&mut s);
    sessions_between(&mut s, "alice", "bob");
    let c = s.client("alice");
    let _ = c.outbox(); // the peering's own submission, not a greeting
    // round 1, held: locator, request, deposit
    assert!(c.greet_peer(kh("bob"), true).is_some());
    assert_eq!(payloads(&c.outbox()), 3, "locator, request and the deposit");
    // round 2, unheld, and the second of silence: the session is set
    // aside and the greeting waits on a bundle
    assert!(c.greet_peer(kh("bob"), false).is_some());
    let out = c.outbox();
    assert!(
        out.iter().any(|m| matches!(m, Msg::PrekeyRequest(_))),
        "reopened: a bundle is asked for, as for a first message: {out:?}"
    );
    assert_eq!(payloads(&out), 0, "nothing goes on a session set aside");
    // a session again (the bench's node would have answered the request),
    // and round 3 held: the deposit memory went with the session, so the
    // same dataset goes once more, with whatever had waited
    sessions_between(&mut s, "alice", "bob");
    let c = s.client("alice");
    let _ = c.outbox();
    assert!(c.greet_peer(kh("bob"), true).is_some());
    assert!(
        payloads(&c.outbox()) >= 3,
        "deposited again after the reopening"
    );
    // the silence allowed doubled: rounds 4 and 5 reopen nothing
    for _ in 0..2 {
        assert!(c.greet_peer(kh("bob"), true).is_some());
        let out = c.outbox();
        assert!(
            !out.iter().any(|m| matches!(m, Msg::PrekeyRequest(_))),
            "the session stands"
        );
        assert_eq!(payloads(&out), 2, "nothing to deposit: {out:?}");
    }
    // round 6 is the fourth of silence: reopened at the doubled interval
    // — on a bundle still held, so the first message goes at once and
    // the deposit with it; on none, a bundle is asked for first
    assert!(c.greet_peer(kh("bob"), true).is_some());
    let out = c.outbox();
    // on the harness, with no node to answer the bundle asked for in
    // round 2, the greeting parks behind that request: anything but the
    // plain two-payload greeting of a standing session is the reopening
    let n = payloads(&out);
    assert_ne!(
        n,
        2,
        "reopened at the doubled interval: {} messages",
        out.len()
    );
}
