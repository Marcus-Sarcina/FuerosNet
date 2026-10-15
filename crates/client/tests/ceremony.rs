//! **What the kernel does with one call at a time**: the retention a
//! person sets between ceremonies, the intent that claims a side it may
//! not, the pinning of a stranger from a first QR, the re-send of a
//! query nobody answered, and a witness's bound on the ceremonies it
//! holds at once.
//!
//! The ceremony's conversation and the record it reaches are asserted
//! where they happen — beside a node, over the sessions
//! (`adaptors/tests/ceremony.rs` and `adaptors/tests/conversation.rs`).
//! A meeting needed here as *evidence* is `Setup::met` (the record of
//! one, arriving) or `Setup::met_in_person` (the local exchanges, then
//! the record), neither of which is a ceremony run.

mod common;

use common::harness::*;
use common::*;
use rhtn_client::ceremony::*;
use rhtn_client::device::*;

const EVERYONE: [&str; 6] = ["alice", "bob", "carol", "w1", "w2", "w3"];

/// **The scene a selection needs**: carol has met bob, alice has met bob
/// and c1 in person, and then alice and carol meet — so carol, who
/// recognises bob among alice's two introduced parties, must seek one
/// verifier and he is the one she can ask.  The log is left holding the
/// last meeting only.
fn three_meetings(channels: &[ChannelKind]) -> (Setup, [u8; 32], [u8; 32], [u8; 32]) {
    let mut s = setup(&["alice", "bob", "carol", "c1", "w1", "w2", "w3"], channels);
    let cb = s.met("carol", "bob", "w1");
    // **in person**, so bob holds a capture of alice and she holds the
    // key to it: a verifier asked about a subject it has only a record of
    // could do nothing but close the query
    let ab = s.met_in_person("alice", "bob", "w1");
    // a second prior counterparty for alice: *n* counts the parties she
    // introduces, so carol must seek a verifier only once alice names
    // two — c1, whom carol has not met, so bob is the one she recognises.
    // In person again, because a party is introduced by the locator its
    // intent carried (`Client::take_intent`): a record alone introduces
    // nobody and *n* would still be one
    let _ac1 = s.met_in_person("alice", "c1", "w1");
    let before = s.h.log.len();
    let ac = s.met("alice", "carol", "w2");
    s.h.log.drain(..before);
    (s, cb, ab, ac)
}

/// **A query or a grant unanswered at half the patience goes again, on
/// a fresh session, once.** A verifier that lost its sessions refuses
/// everything sent on the old ones and has nothing to say so on; the
/// selector's query and the subject's grant are the two messages an
/// answer is owed to, and each side re-sends its own. Here nothing
/// answers at all, so both go again at 30 s and neither a third time;
/// with no node beside them a one-time key is asked for the verifier and
/// the bytes wait on it, as any first send does.
#[test]
fn a_query_and_a_grant_unanswered_at_half_the_patience_go_again_once() {
    let (mut s, _, _, _) = three_meetings(&[ChannelKind::Nfc]);
    // alice and carol again, driven by hand as far as the queries and
    // no further, so that nothing answers them
    s.face_off("alice", "carol");
    let (a, c) = (kh("alice"), kh("carol"));
    let ia = s
        .client("alice")
        .begin(Some(c), vec![kh("w2")], true)
        .unwrap();
    let ic = s
        .client("carol")
        .begin(Some(a), vec![kh("w3")], false)
        .unwrap();
    s.client("carol").take_intent(a, &ia).unwrap();
    s.client("alice").take_intent(c, &ic).unwrap();
    let ch = s.client("alice").proximity().unwrap();
    s.client("carol").take_channels(&ch).unwrap();
    let ka = s.client("alice").capture_key().unwrap();
    let kc = s.client("carol").capture_key().unwrap();
    s.client("carol").capture(ka).unwrap();
    s.client("alice").capture(kc).unwrap();
    // carol selects bob for alice and queries him; alice consents and
    // releases the key
    let picked = s.client("carol").select_verifiers().unwrap();
    assert_eq!(picked.len(), 1, "bob, whom carol recognises");
    let (v, basis) = picked[0];
    assert_eq!(v, kh("bob"));
    let q = s.client("carol").query_for(v).unwrap();
    let (consent, grant) = s.client("alice").consent(&q).expect("alice consents");
    assert!(grant.is_some(), "a key released to bob");
    let _req = s.client("carol").request(&q, consent, basis);

    let asks = |msgs: Vec<Msg>| {
        msgs.iter()
            .filter(|m| matches!(m, Msg::PrekeyRequest(_)))
            .count()
    };
    // before half the patience, maintenance sends nothing to bob
    s.clock.set(s.clock.get() + 29_000);
    assert_eq!(asks(s.client("carol").maintain()), 0);
    assert_eq!(asks(s.client("alice").maintain()), 0);
    // at half, each re-sends its own: a one-time key asked for bob, the
    // bytes waiting on it
    s.clock.set(s.clock.get() + 1_000);
    assert_eq!(asks(s.client("carol").maintain()), 1, "the query, again");
    assert_eq!(asks(s.client("alice").maintain()), 1, "the grant, again");
    // and not a third time: from here the person decides
    s.clock.set(s.clock.get() + 60_000);
    assert_eq!(asks(s.client("carol").maintain()), 0);
    assert_eq!(asks(s.client("alice").maintain()), 0);
}

/// The retention a client declares is its own to set (design §7.5.1;
/// `light-client-requirements.md` §1.3), in whole years, before a ceremony
/// and not during one, whose intent has already stated it.
#[test]
fn the_declared_retention_is_the_persons_to_set_between_ceremonies() {
    let mut s = setup(&["alice", "bob"], &[ChannelKind::Nfc]);
    let a = s.client("alice");
    assert_eq!(a.retention_years(), 2, "the default");
    assert_eq!(
        a.set_retention_years(0),
        Err("a retention is at least one year")
    );
    a.set_retention_years(1).expect("one year is a retention");
    assert_eq!(a.retention_years(), 1);
    let intent = a
        .begin(Some(kh("bob")), vec![], true)
        .expect("alice begins");
    assert_eq!(intent.retention_years, 1, "the intent carries what was set");
    assert!(
        a.set_retention_years(3).is_err(),
        "not while a ceremony is under way"
    );
    assert_eq!(a.retention_years(), 1);
    a.abandon();
    a.set_retention_years(3).expect("and again once it is over");
    assert_eq!(a.intent().err(), Some(Abort::NotActive));
    assert_eq!(a.retention_years(), 3);
}

/// Two clients that have never met (design §12.3's first contact): the
/// first QR carries the key, the first code read fixes the counterparty,
/// and the record verifies under what was pinned there.
// acceptance: CER-49
#[test]
fn strangers_pin_each_other_from_the_first_qr_and_the_first_code_fixes_the_counterparty() {
    let mut s = setup(&EVERYONE, &[ChannelKind::Nfc]);
    // neither holds the other: what each knows is itself and the witnesses
    s.client("alice").known.retain(|i| i.keyhash != kh("bob"));
    s.client("bob").known.retain(|i| i.keyhash != kh("alice"));
    // the initiator names nobody: it shows the first invite and learns who
    // read it from their QR; the responder read the bootstrap and may name
    let ia = s
        .client("alice")
        .begin(None, vec![], true)
        .expect("alice begins");
    assert_eq!(
        s.client("alice").proximity().err(),
        Some(Abort::NoCounterparty)
    );
    let ib = s
        .client("bob")
        .begin(Some(kh("alice")), vec![], false)
        .expect("bob begins");
    let qa = s.client("alice").optical_contribution().unwrap();
    let qb = s.client("bob").optical_contribution().unwrap();
    assert!(
        qa.len() > 2000,
        "the first QR carries the full key: {} bytes",
        qa.len()
    );
    assert_eq!(s.client("alice").take_optical(&qb), Ok(kh("bob")));
    assert_eq!(s.client("bob").take_optical(&qa), Ok(kh("alice")));
    assert!(
        s.client("alice")
            .known
            .iter()
            .any(|i| i.keyhash == kh("bob")),
        "alice pinned bob"
    );
    assert!(
        s.client("bob")
            .known
            .iter()
            .any(|i| i.keyhash == kh("alice")),
        "bob pinned alice"
    );
    // a second first-QR naming a third party is a party who is not in front of me
    let qc = {
        let c = s.client("carol");
        c.begin(None, vec![], true).unwrap();
        let q = c.optical_contribution().unwrap();
        c.abandon();
        q
    };
    assert_eq!(s.client("alice").take_optical(&qc), Err(Abort::NotActive));
    // and the ceremony runs to a record on what was pinned
    let ta = s.client("alice").transcript_confirm().unwrap();
    let tb = s.client("bob").transcript_confirm().unwrap();
    s.client("alice").take_transcript(&tb).unwrap();
    s.client("bob").take_transcript(&ta).unwrap();
    s.client("bob").take_intent(kh("alice"), &ia).unwrap();
    s.client("alice").take_intent(kh("bob"), &ib).unwrap();
    let _ = (ia, ib);
    s.h.log.clear();
    s.client("alice").abandon();
    s.client("bob").abandon();
    // and a record of the two of them verifies under what was pinned:
    // `finalize` reads the envelope against what this client knows, which
    // for each of these two is the key the other's first QR carried
    let txid = s.met_in_person("alice", "bob", "w1");
    assert!(s.client("bob").store.records.contains_key(&txid));
}

/// A witness holds at most `PENDING_WITNESS_REQUESTS` ceremonies at once,
/// the holder's own bound: one more displaces the one with the oldest
/// claimed start, which it then cannot sign; asked twice for one ceremony,
/// it answers the same.
#[test]
fn a_witness_holds_a_bounded_number_of_pending_requests_and_the_oldest_goes_first() {
    use rhtn_archive::tx::Witness;
    use rhtn_client::record::Proposal;
    use rhtn_client::sequence::PENDING_WITNESS_REQUESTS;
    let mut s = setup(&EVERYONE, &[ChannelKind::Nfc]);
    let now = s.clock.get() / 1000;
    let bound = PENDING_WITNESS_REQUESTS as u64;
    let w = s.client("w1");
    let req = |i: u64| WitnessRequest {
        ceremony_id: [i as u8; 32],
        participants: participants(kh("alice"), kh("bob")),
        started_at: now - 100 + i,
        channels: vec![],
    };
    for i in 0..bound {
        assert_eq!(w.take_witness_request(&req(i)), Some(3));
    }
    // asked again for the first: the same answer, and nothing displaced
    assert_eq!(w.take_witness_request(&req(0)), Some(3));
    // one past the bound: taken, and the oldest claimed start is gone
    assert_eq!(w.take_witness_request(&req(bound)), Some(3));
    let proposal = |i: u64| Proposal {
        started_at: now - 100 + i,
        finalized_at: now,
        participants: participants(kh("alice"), kh("bob")),
        witnesses: vec![Witness {
            keyhash: kh("w1"),
            nominated_by: kh("alice"),
            flags: 3,
        }],
        responses: vec![],
        formation: false,
        root: [0; 32],
    };
    let back = vec![w.back_pointers(); 3];
    assert_eq!(
        w.witness_sign(&proposal(0), &back).map(|_| ()),
        Err(Abort::NotActive),
        "the oldest is no longer held"
    );
    assert!(w.witness_sign(&proposal(1), &back).is_ok());
    assert!(w.witness_sign(&proposal(bound), &back).is_ok());
}
