//! The subject's side: consent within a window, the counters that lock and
//! then vanish, the grant against the most recent eligible capture, and the
//! response copies held.

mod common;

use common::*;
use rhtn_client::notice::{Notice, Notifier};
use rhtn_client::query::*;
use rhtn_client::store::{ClientStore, OwnSeed};
use rhtn_client::subject::*;
use std::cell::RefCell;

struct Hook(RefCell<Vec<Notice>>);
impl Notifier for Hook {
    fn notify(&self, n: Notice) {
        self.0.borrow_mut().push(n);
    }
}

/// Alice's seeds for two records with bob, R1 then R2.
fn alice_store() -> (ClientStore, [u8; 32], [u8; 32], u64) {
    let mut w = World::new();
    let r1 = w.meet("alice", "bob");
    let r2 = w.meet("alice", "bob");
    let mut store = ClientStore::default();
    for (rec, c, s) in [(&r1, [11u8; 32], [31u8; 32]), (&r2, [22u8; 32], [32u8; 32])] {
        store.seeds.insert(
            rec.txid,
            OwnSeed {
                seed: s,
                counterparty: kh("bob"),
                ceremony_id: c,
                finalized_at: rec.effective,
            },
        );
        store.records.insert(rec.txid, rec.bytes.clone());
    }
    (store, r1.txid, r2.txid, r2.effective)
}

fn query(ceremony: [u8; 32], verifier: &str, profile: &[u8]) -> VerificationQuery {
    VerificationQuery {
        subject: kh("alice"),
        querier: kh("carol"),
        ceremony_id: ceremony,
        profile: profile.to_vec(),
        template_version: 1,
        verifier: kh(verifier),
    }
}

// acceptance: CER-09
#[test]
fn the_grant_names_the_most_recent_eligible_capture_by_default() {
    let (store, r1, r2, t2) = alice_store();
    let me = id("alice");
    let subject = SubjectState::new(SubjectConfig::default());
    let now = t2 + 86_400;
    let g = subject
        .grant_for(&me, &store, &kh("bob"), [5; 32], now)
        .expect("bob holds eligible captures");
    assert_eq!(g.record, r2, "R2, the more recent");
    assert_eq!(g.query_id, [5; 32]);
    assert_eq!(
        eligible_captures(&store, &kh("bob"), subject.cfg.retention_seconds, now),
        vec![r2, r1],
        "most recent first"
    );
    // R1 past the retention horizon: R2 alone is eligible
    let short = SubjectState::new(SubjectConfig {
        retention_seconds: 1800,
        ..SubjectConfig::default()
    });
    let soon = t2 + 600;
    assert_eq!(eligible_captures(&store, &kh("bob"), 1800, soon), vec![r2]);
    assert_eq!(
        short
            .grant_for(&me, &store, &kh("bob"), [5; 32], soon)
            .map(|g| g.record),
        Some(r2)
    );
    // both past it: nothing to grant, and that reads as unavailability
    let past = t2 + 1800;
    assert_eq!(
        short.grant_for(&me, &store, &kh("bob"), [5; 32], past),
        None
    );
    // R2's seed discarded with its record: R1 is the most recent left
    let mut fewer = store.clone();
    fewer.seeds.remove(&r2);
    assert_eq!(
        subject
            .grant_for(&me, &fewer, &kh("bob"), [5; 32], now)
            .map(|g| g.record),
        Some(r1)
    );
    // a verifier alice never met holds nothing
    assert_eq!(
        subject.grant_for(&me, &store, &kh("carol"), [5; 32], now),
        None
    );
    // the key is the capture key for that record's seed, subject and holder
    let s = &store.seeds[&r2];
    assert_eq!(
        g.key,
        rhtn_client::keys::capture_key(&s.seed, &kh("alice"), &kh("bob"), &s.ceremony_id)
    );
}

// acceptance: CER-25
#[test]
fn the_aggregate_is_a_lock_for_the_window_and_nothing_after() {
    let me = id("alice");
    let hook = Hook(RefCell::new(vec![]));
    let mut subject = SubjectState::new(SubjectConfig {
        per_requester_limit: 2,
        ..SubjectConfig::default()
    });
    let ceremony = [7u8; 32];
    subject.open_window(ceremony);
    let (q1, q2, q3) = (
        query(ceremony, "bob", b"p"),
        query(ceremony, "w1", b"p"),
        query(ceremony, "w2", b"p"),
    );
    assert!(subject.consent_to(&me, &q1, &hook).is_some());
    assert!(subject.consent_to(&me, &q2, &hook).is_some());
    assert_eq!(
        subject.consent_to(&me, &q3, &hook),
        None,
        "the third from carol is over the limit: no consent, so no grant"
    );
    assert!(
        hook.0.borrow().contains(&Notice::ProbingRefused {
            requester: kh("carol")
        }),
        "refused visibly"
    );
    assert_eq!(
        hook.0
            .borrow()
            .iter()
            .filter(|n| matches!(n, Notice::QuerySurfaced { .. }))
            .count(),
        2,
        "each consented query was surfaced"
    );
    // another requester has its own allowance
    let other = VerificationQuery {
        querier: kh("w3"),
        ..query(ceremony, "w4", b"p")
    };
    assert!(subject.consent_to(&me, &other, &hook).is_some());
    // the window closes: the state holds no requester and no count
    subject.close_window();
    assert!(!subject.window_open());
    let dump = format!("{subject:?}");
    assert!(
        !dump.contains(&format!("{:?}", kh("carol"))),
        "no record of the requester remains"
    );
    assert!(!dump.contains(&format!("{:?}", kh("w3"))));
    assert!(!dump.contains("counters: {"), "no counter remains: {dump}");
    // what remains names the queries consented to, which is what a late
    // response is checked against, and nothing about who asked
    assert_eq!(subject.consented(&ceremony).len(), 3);
    // and nothing is granted outside a window
    assert_eq!(
        subject.consent_to(&me, &query([8; 32], "bob", b"p"), &hook),
        None
    );
}

#[test]
fn consent_is_bound_to_the_window_the_subject_and_the_ceremonys_one_profile() {
    let me = id("alice");
    let hook = Hook(RefCell::new(vec![]));
    let mut subject = SubjectState::new(SubjectConfig::default());
    let ceremony = [7u8; 32];
    subject.open_window(ceremony);
    // about someone else: never
    assert_eq!(
        subject.consent_to(
            &me,
            &VerificationQuery {
                subject: kh("bob"),
                ..query(ceremony, "w1", b"p")
            },
            &hook
        ),
        None
    );
    // another ceremony than the open window's
    assert_eq!(
        subject.consent_to(&me, &query([9; 32], "w1", b"p"), &hook),
        None
    );
    // the first profile fixes the ceremony's one
    let c = subject
        .consent_to(&me, &query(ceremony, "w1", b"p"), &hook)
        .unwrap();
    assert!(consent_verifies(
        &me.public,
        &c,
        &query(ceremony, "w1", b"p").query_id()
    ));
    assert_eq!(
        subject.consent_to(&me, &query(ceremony, "w2", b"q"), &hook),
        None,
        "a second profile within one ceremony"
    );
    assert!(
        subject
            .consent_to(&me, &query(ceremony, "w2", b"p"), &hook)
            .is_some()
    );
    assert!(
        hook.0
            .borrow()
            .iter()
            .all(|n| !matches!(n, Notice::ProbingRefused { .. })),
        "none of these was a refusal for probing"
    );
}

#[test]
fn a_response_copy_is_held_only_for_a_consented_query_under_its_verifier() {
    let me = id("alice");
    let hook = Hook(RefCell::new(vec![]));
    let mut subject = SubjectState::new(SubjectConfig::default());
    let ceremony = [7u8; 32];
    subject.open_window(ceremony);
    let q = query(ceremony, "bob", b"p");
    let c = subject.consent_to(&me, &q, &hook).unwrap();
    let resp = Response {
        verifier: kh("bob"),
        subject: kh("alice"),
        query_id: q.query_id(),
        verdict: Verdict::Match,
        basis: Some(Basis::PhotoMatch),
        template_version: Some(1),
        consent: c.clone(),
        selection_basis: 1,
    };
    let bytes = resp.sign(&id("bob"));
    assert_eq!(
        subject.take_response_copy(&me, &ids(), &bytes),
        Ok(q.query_id())
    );
    assert_eq!(subject.held_responses(), vec![bytes.clone()]);
    // signed by someone other than the verifier it names
    let forged = resp.sign(&id("carol"));
    assert!(subject.take_response_copy(&me, &ids(), &forged).is_err());
    // answering a query alice never consented to
    let other = query(ceremony, "w1", b"p");
    let stray = Response {
        verifier: kh("w1"),
        query_id: other.query_id(),
        consent: consent(&me, &other.query_id()),
        ..resp.clone()
    };
    assert!(
        subject
            .take_response_copy(&me, &ids(), &stray.sign(&id("w1")))
            .is_err()
    );
    // about someone else
    let about_bob = Response {
        subject: kh("bob"),
        ..resp.clone()
    };
    assert!(
        subject
            .take_response_copy(&me, &ids(), &about_bob.sign(&id("bob")))
            .is_err()
    );
    assert_eq!(subject.held_responses().len(), 1);
    // discarding the ceremony forgets the consents
    subject.discard_ceremony(&ceremony);
    assert!(subject.consented(&ceremony).is_empty());
}
