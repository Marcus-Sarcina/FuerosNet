//! The detector opens the ladder over real sessions (design §12.6.5.1,
//! §14.1.2): what a node's own sessions settle about its peers feeds its
//! currency state, stamped with its own clock, and the rungs open by how
//! long the outage has lasted.

mod common;

use common::*;
use rhtn_archive::currency::parse_attestation;
use rhtn_node::currency::{CurrencyReply, CurrencyRequest, ROLE_GRANDPATRON};
use rhtn_node::resolution::{AnchorTable, Ingestion, REQUEST_CURRENCY};
use rhtn_node::runtime::LiveNode;
use rhtn_transport::session::*;

const I: u64 = 1;

// acceptance: CUR-16
#[tokio::test]
async fn the_detector_feeds_the_ladder_and_the_rungs_open_by_outage_duration() {
    let _serial = serial().await;
    // G (alice) is the root; P (bob) at 0 and P' (w1) at 1 are its infra
    // children; S (carol) is P's subordinate at [0, 2]
    let mut s = Signers::new();
    let a_p = s.adopt("bob", "alice", "alice", &[0], 1);
    let a_p2 = s.adopt("w1", "alice", "alice", &[1], 2);
    let a_s = s.adopt("carol", "bob", "alice", &[0, 2], 3);
    let records = [&a_p, &a_p2, &a_s];
    let infra = ["alice", "bob", "w1", "carol"];
    let mut g_view = view_of(
        "alice",
        table_of("alice", &s, &records, &infra),
        "alice",
        &[],
        s.clock,
    );
    g_view.set_slot(0, Some(kh("bob")), s.clock);
    g_view.set_slot(1, Some(kh("w1")), s.clock);
    let g = LiveNode::start(
        node_cfg("alice", I),
        g_view,
        ids(),
        AnchorTable::new(0, Ingestion::UnverifiedGossip),
    );
    // P's heartbeats stop in both directions: toward G on its upstream
    // session, and toward S on the side it serves
    let mut pcfg = node_cfg("bob", I);
    pcfg.filter = Some(drop_heartbeats());
    let mut p_view = view_of(
        "bob",
        table_of("bob", &s, &records, &infra),
        "alice",
        &[0],
        s.clock,
    );
    p_view.set_slot(2, Some(kh("carol")), s.clock);
    let p = LiveNode::start(
        pcfg,
        p_view,
        ids(),
        AnchorTable::new(0, Ingestion::UnverifiedGossip),
    );
    let p2 = LiveNode::start(
        node_cfg("w1", I),
        view_of(
            "w1",
            table_of("w1", &s, &records, &infra),
            "alice",
            &[1],
            s.clock,
        ),
        ids(),
        AnchorTable::new(0, Ingestion::UnverifiedGossip),
    );
    let sn = LiveNode::start(
        node_cfg("carol", I),
        view_of(
            "carol",
            table_of("carol", &s, &records, &infra),
            "alice",
            &[0, 2],
            s.clock,
        ),
        ids(),
        AnchorTable::new(0, Ingestion::UnverifiedGossip),
    );
    let mut pcli = client_cfg("bob");
    pcli.filter = Some(drop_heartbeats());
    know(&pcli, "alice", g.addr);
    let AttachOutcome::Attached(_p_up) = p.attach_upstream(&pcli, kh("alice")).await else {
        panic!("P attaches to G")
    };
    let mut p2cli = client_cfg("w1");
    p2cli.filter = Some(drop_heartbeats());
    know(&p2cli, "alice", g.addr);
    let AttachOutcome::Attached(_p2_up) = p2.attach_upstream(&p2cli, kh("alice")).await else {
        panic!("P' attaches to G")
    };
    let scli = client_cfg("carol");
    know(&scli, "bob", p.addr);
    let AttachOutcome::Attached(_s_up) = sn.attach_upstream(&scli, kh("bob")).await else {
        panic!("S attaches to P")
    };
    assert!(
        g.currency.lock().unwrap().unreachable.is_empty(),
        "nobody is dark before the detector settles"
    );
    // three missed intervals: G's detector marks P and P' on the sessions it
    // serves, and S's marks P on the session it holds upstream
    assert!(
        until(8000, || g.currency.lock().unwrap().unreachable.len() == 2).await,
        "G's ladder holds P and P' dark"
    );
    assert!(
        until(8000, || sn
            .currency
            .lock()
            .unwrap()
            .unreachable
            .contains_key(&kh("bob")))
        .await,
        "S's ladder holds P dark"
    );
    assert_eq!(
        g.currency.lock().unwrap().unreachable[&kh("bob")],
        g.view.lock().unwrap().now(),
        "since the node's own clock"
    );
    // a caller asks G for S's currency: within the durations nothing is
    // issued, since the staple S holds still stands
    let ccfg = client_cfg("c1");
    know(&ccfg, "alice", g.addr);
    let AttachOutcome::Attached(c) = attach(&ccfg, &client_ep(), kh("alice"), g.addr, false).await
    else {
        panic!("C attaches to G")
    };
    let ask = |n: u8| {
        CurrencyRequest {
            subject: kh("carol"),
            nonce: [n; 16],
        }
        .encode()
    };
    let bytes = c
        .request(REQUEST_CURRENCY, &ask(1))
        .await
        .expect("answered");
    assert_eq!(
        CurrencyReply::decode(&bytes).unwrap(),
        CurrencyReply::CannotIssue { nonce: [1; 16] },
        "the grandpatron waits out the days"
    );
    // two days on, both still dark: the grandpatron rung is open
    let later = g.view.lock().unwrap().now() + 2 * 86_400;
    g.view.lock().unwrap().set_now(later);
    let bytes = c
        .request(REQUEST_CURRENCY, &ask(2))
        .await
        .expect("answered");
    let CurrencyReply::Attestation { bytes, .. } = CurrencyReply::decode(&bytes).unwrap() else {
        panic!("issued")
    };
    let a = parse_attestation(&ids(), &bytes).unwrap();
    assert_eq!(
        (a.role, a.issuer, a.subject),
        (ROLE_GRANDPATRON, kh("alice"), kh("carol"))
    );
}

// acceptance: CUR-17
#[tokio::test]
async fn a_running_node_issues_at_its_configured_clock() {
    use std::sync::atomic::{AtomicU64, Ordering};
    // P (alice) over S (bob); the view is built at the signers' clock, and
    // the node runs on a clock the test moves
    let mut s = Signers::new();
    let a_s = s.adopt("bob", "alice", "alice", &[0], 1);
    let p_view = view_of(
        "alice",
        table_of("alice", &s, &[&a_s], &["alice"]),
        "alice",
        &[],
        s.clock,
    );
    let clock = std::sync::Arc::new(AtomicU64::new(100));
    let reader = clock.clone();
    let mut cfg = node_cfg("alice", I);
    cfg.clock = std::sync::Arc::new(move || reader.load(Ordering::SeqCst));
    let p = LiveNode::start(
        cfg,
        p_view,
        ids(),
        AnchorTable::new(0, Ingestion::UnverifiedGossip),
    );
    assert_eq!(
        p.view.lock().unwrap().now(),
        100,
        "the view runs on the configured clock"
    );
    let ccfg = client_cfg("c1");
    know(&ccfg, "alice", p.addr);
    let AttachOutcome::Attached(c) = attach(&ccfg, &client_ep(), kh("alice"), p.addr, false).await
    else {
        panic!("C attaches to P")
    };
    let ask = |n: u8| {
        CurrencyRequest {
            subject: kh("bob"),
            nonce: [n; 16],
        }
        .encode()
    };
    for (n, t) in [(1u8, 100u64), (2, 101), (3, 4242)] {
        clock.store(t, Ordering::SeqCst);
        let bytes = c
            .request(REQUEST_CURRENCY, &ask(n))
            .await
            .expect("answered");
        let CurrencyReply::Attestation { bytes, .. } = CurrencyReply::decode(&bytes).unwrap()
        else {
            panic!("issued")
        };
        let a = parse_attestation(&ids(), &bytes).unwrap();
        assert_eq!(a.issued_at, t, "issued at the clock's reading, request {n}");
    }
}
