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
    // the bound is what this holds, not the window: a witness inside its
    // window withholds every signature, which `a_witness_will_not_sign_
    // before_its_window_has_passed` is the test of [author, 2026-10-21]
    w.cfg.witness_window_s = 0;
    let req = |i: u64| WitnessRequest {
        ceremony_id: [i as u8; 32],
        participants: participants(kh("alice"), kh("bob")),
        started_at: now - 100 + i,
        channels: vec![],
        handle: [0xe0 | i as u8; 32],
    };
    // each request closes an engagement this witness watched [author,
    // 2026-10-19]: engaged just before asked, inside the engagement bound
    let engage = |w: &mut rhtn_client::ceremony::Client, i: u64| {
        assert!(w.take_engagement(
            kh("alice"),
            &rhtn_client::conversation::Engagement {
                handle: [0xe0 | i as u8; 32],
                started_at: now - 100 + i,
                counterparty: Some(kh("bob")),
            },
        ));
    };
    for i in 0..bound {
        engage(w, i);
        assert_eq!(w.take_witness_request(&req(i)), Some(3));
    }
    // asked again for the first: the same answer, and nothing displaced
    assert_eq!(w.take_witness_request(&req(0)), Some(3));
    // one past the bound: taken, and the oldest claimed start is gone
    engage(w, bound);
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

// acceptance: CER-70
#[test]
fn a_request_binds_to_the_engagement_watched_and_its_ceremony_id() {
    use rhtn_client::conversation::{Engagement, Trace, TraceEvent};
    let mut s = setup(&EVERYONE, &[ChannelKind::Nfc]);
    let now = s.clock.get() / 1000;
    let w = s.client("w1");
    let engagement = |h: u8| Engagement {
        handle: [h; 32],
        started_at: now,
        counterparty: Some(kh("bob")),
    };
    let req = |h: u8, cid: u8| WitnessRequest {
        ceremony_id: [cid; 32],
        participants: participants(kh("alice"), kh("bob")),
        started_at: now,
        channels: vec![],
        handle: [h; 32],
    };
    // a request whose engagement this client never watched binds nothing
    // it observed: declined
    assert_eq!(w.take_witness_request(&req(1, 10)), None, "from nowhere");
    // engaged and confirmed under one ceremony-id, then asked under
    // another: the sequence watched is not the ceremony asked about
    assert!(w.take_engagement(kh("alice"), &engagement(2)));
    assert!(w.take_trace(
        kh("alice"),
        &Trace {
            handle: [2; 32],
            event: TraceEvent::Confirmed([20; 32]),
        }
    ));
    assert_eq!(
        w.take_witness_request(&req(2, 21)),
        None,
        "a disagreeing id"
    );
    // the trace is the engaging party's alone
    assert!(!w.take_trace(
        kh("carol"),
        &Trace {
            handle: [2; 32],
            event: TraceEvent::Confirmed([20; 32]),
        }
    ));
    // a request naming a meeting the engagement did not: declined
    assert!(w.take_engagement(kh("alice"), &engagement(3)));
    let mut other = req(3, 30);
    other.participants = participants(kh("alice"), kh("carol"));
    assert_eq!(w.take_witness_request(&other), None, "another meeting");
    // agreeing on the handle, the id and the parties: answered as always
    assert!(w.take_engagement(kh("alice"), &engagement(4)));
    assert!(w.take_trace(
        kh("alice"),
        &Trace {
            handle: [4; 32],
            event: TraceEvent::Confirmed([40; 32]),
        }
    ));
    assert_eq!(w.take_witness_request(&req(4, 40)), Some(3), "it agrees");
}

// acceptance: CER-67
#[test]
fn the_opening_reaches_a_nominee_before_acceptance_and_one_event_at_a_time() {
    use rhtn_client::conversation::{Trace, TraceEvent};
    let mut s = setup(&EVERYONE, &[ChannelKind::Nfc]);
    s.face_off("alice", "bob");
    let (a, b) = (kh("alice"), kh("bob"));
    let ib = s
        .client("bob")
        .begin(Some(a), vec![], false)
        .expect("bob begins");
    s.client("alice")
        .begin(Some(b), vec![kh("w1")], true)
        .expect("alice begins");
    // **the engagement exists before the counterparty has accepted
    // anything**: no intent has been taken either way, so no ceremony-id
    // is fixed, and the handle is what the nominee watches under
    let e = s.client("alice").engagement().expect("an engagement");
    assert_eq!(e.counterparty, Some(b));
    assert_eq!(
        s.client("alice").witness_request().map(|_| ()),
        Err(Abort::NoCeremonyId),
        "there is nothing to ask a witness yet"
    );
    assert!(s.client("w1").take_engagement(a, &e));
    // the exchange's shape, one event per message and never batched: each
    // is recorded as it lands, in order, on the witness's own clock
    let events = [
        TraceEvent::Shape { mine: 7, theirs: 5 },
        TraceEvent::Inbound(0),
        TraceEvent::Outbound(0),
        TraceEvent::Inbound(1),
    ];
    for (i, event) in events.into_iter().enumerate() {
        s.clock.set(s.clock.get() + 500);
        assert!(s.client("w1").take_trace(
            a,
            &Trace {
                handle: e.handle,
                event,
            }
        ));
        assert_eq!(
            s.client("w1").observed_trace(&e.handle).len(),
            i + 1,
            "one arrival per event"
        );
    }
    let watched = s.client("w1").observed_trace(&e.handle);
    assert_eq!(
        watched.iter().map(|(code, _, _)| *code).collect::<Vec<_>>(),
        vec![0, 1, 2, 1],
        "the events in the order they were sent"
    );
    assert!(
        watched.windows(2).all(|w| w[0].2 < w[1].2),
        "spread across the witness's own clock, never batched: {watched:?}"
    );
    // the ids agree: the intent fixes the ceremony-id, the confirmation
    // carries it, and the request repeats it beside the handle (CER-70)
    let cid = s
        .client("alice")
        .take_intent(b, &ib)
        .expect("alice takes bob's intent");
    assert!(s.client("w1").take_trace(
        a,
        &Trace {
            handle: e.handle,
            event: TraceEvent::Confirmed(cid),
        }
    ));
    let req = s.client("alice").witness_request().expect("a request");
    assert_eq!(req.ceremony_id, cid);
    assert_eq!(req.handle, e.handle, "the request closes the engagement");
    assert_eq!(s.client("w1").take_witness_request(&req), Some(3));
}

// acceptance: CER-52
/// **A witness will not sign before the window has passed on its own
/// clock** (design §7.1 item 9; `light-client-requirements.md` §1.2)
/// [author, 2026-10-21].  The wait is withheld rather than refused — no
/// value names waiting (`wire-format.md` §7.10.2) — and it is measured
/// from this client's own receipt of the request, so a participant's claim
/// about elapsed time neither shortens nor lengthens it.
///
/// **The four-minute wall clock this row was deferred for is not needed**:
/// the harness clock is this client's clock, and moving it is what the
/// witness is waiting on.
#[test]
fn a_witness_will_not_sign_before_its_window_has_passed() {
    use rhtn_archive::tx::Witness;
    use rhtn_client::conversation::Engagement;
    use rhtn_client::record::Proposal;
    let mut s = setup(&EVERYONE, &[ChannelKind::Nfc]);
    let window_s = 30;
    s.client("w1").cfg.witness_window_s = window_s;
    let now = s.clock.get() / 1000;
    let w = s.client("w1");
    assert!(w.take_engagement(
        kh("alice"),
        &Engagement {
            handle: [9; 32],
            started_at: now,
            counterparty: Some(kh("bob")),
        },
    ));
    let req = WitnessRequest {
        ceremony_id: [9; 32],
        participants: participants(kh("alice"), kh("bob")),
        started_at: now,
        channels: vec![],
        handle: [9; 32],
    };
    assert_eq!(w.take_witness_request(&req), Some(3), "it agreed to attest");
    let proposal = Proposal {
        started_at: now,
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
    let back = vec![s.client("w1").back_pointers(); 3];
    // inside the window: withheld, and withheld is not refused
    assert_eq!(
        s.client("w1")
            .witness_sign(&proposal, &back)
            .map(|_| ())
            .map_err(|e| matches!(e, Abort::Waiting(_))),
        Err(true),
        "a witness inside its window withholds"
    );
    // **a longer claimed elapsed time does not shorten it**: the claim is
    // the parties', and this clock is what the witness measures
    let mut older = proposal.clone();
    older.started_at = now - 3_600;
    assert!(
        s.client("w1").witness_sign(&older, &back).is_err(),
        "a claim of an hour buys nothing"
    );
    // one second short: still withheld
    s.clock.set(s.clock.get() + (window_s - 1) * 1_000);
    assert!(
        s.client("w1").witness_sign(&proposal, &back).is_err(),
        "a second short is short"
    );
    // and past it: signed
    s.clock.set(s.clock.get() + 1_000);
    assert!(
        s.client("w1").witness_sign(&proposal, &back).is_ok(),
        "the window has passed on its own clock"
    );
    // a client whose window is zero does not wait at all, which is what
    // the in-process harness runs on
    let mut t = setup(&EVERYONE, &[ChannelKind::Nfc]);
    t.client("w1").cfg.witness_window_s = 0;
    let tnow = t.clock.get() / 1000;
    let e2 = Engagement {
        handle: [8; 32],
        started_at: tnow,
        counterparty: Some(kh("bob")),
    };
    assert!(t.client("w1").take_engagement(kh("alice"), &e2));
    let req2 = WitnessRequest {
        ceremony_id: [8; 32],
        participants: participants(kh("alice"), kh("bob")),
        started_at: tnow,
        channels: vec![],
        handle: [8; 32],
    };
    assert_eq!(t.client("w1").take_witness_request(&req2), Some(3));
    let mut p2 = proposal.clone();
    p2.started_at = tnow;
    p2.finalized_at = tnow;
    let back2 = vec![t.client("w1").back_pointers(); 3];
    assert!(
        t.client("w1").witness_sign(&p2, &back2).is_ok(),
        "a window of zero is the check off"
    );
}
