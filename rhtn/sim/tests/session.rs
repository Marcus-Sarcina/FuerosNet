//! Session, queue and replication entries that need real packets: failover
//! and its detector, the mailbox across an absence, and what a sibling
//! holds.

mod common;

use common::*;
use rhtn_sim::path::Path;
use rhtn_sim::scenario::Running;
use rhtn_transport::queue::Refusal;
use rhtn_transport::session::*;
use std::sync::Arc;
use std::time::Duration;

/// One second is the schema's floor for the interval (§8.2's 1..=3600), so
/// three missed intervals is three seconds.
const I: u64 = 1;

fn secs(n: u64) -> Duration {
    Duration::from_secs(n)
}

/// N (alice) serving C (carol), with S (bob) as N's sibling in failover.
struct Pair {
    n: Running,
    s: Running,
    cfg: ClientConfig,
    ep: quinn::Endpoint,
}

fn pair(n_filter: Option<OutboundFilter>) -> Pair {
    let s = Running::start({
        let mut c = node_cfg("bob", I);
        // S determines from its own topology that C is not in its subtree
        c.in_subtree = Arc::new(|_| false);
        c
    });
    let mut ncfg = node_cfg("alice", I);
    ncfg.siblings = {
        let list = vec![sibling_ref("bob", s.addr)];
        std::sync::Arc::new(move || list.clone())
    };
    ncfg.filter = n_filter;
    let n = Running::start(ncfg);
    let cfg = client_cfg("carol");
    know(&cfg, "alice", n.addr);
    know(&cfg, "bob", s.addr);
    Pair { n, s, cfg, ep: client_ep() }
}

// acceptance: SES-08
#[tokio::test]
async fn three_missed_intervals_move_the_client_to_a_sibling() {
    let _serial = serial().await;
    let p = pair(Some(drop_heartbeats()));
    let AttachOutcome::Attached(mut c) = attach(&p.cfg, &p.ep, kh("alice"), p.n.addr, false).await else { panic!() };
    assert_eq!(c.ack.mode, 0, "primary at first");
    assert_eq!(c.ack.siblings.len(), 1, "N's ack listed S with key material");
    let out = tokio::time::timeout(secs(20), c.failover(&p.cfg, &p.ep)).await.expect("failover within the bound");
    let AttachOutcome::Attached(deg) = out else { panic!("{out:?}") };
    assert_eq!(deg.ack.mode, 1, "S answers with mode 1");
    assert!(deg.degraded());
    assert!(p.n.node.log.count(|e| *e == Event::PeerUnreachable) + c.log.count(|e| *e == Event::PeerUnreachable) > 0);
}

// acceptance: SES-06
#[tokio::test]
async fn payload_does_not_reset_the_detector() {
    let _serial = serial().await;
    let p = pair(Some(drop_heartbeats()));
    let AttachOutcome::Attached(mut c) = attach(&p.cfg, &p.ep, kh("alice"), p.n.addr, false).await else { panic!() };
    // N keeps sending payload on unidirectional streams and no Heartbeat
    let node = p.n.node.clone();
    let pump = tokio::spawn(async move {
        for i in 0..40u32 {
            let _ = node.enqueue(kh("carol"), format!("payload {i}").into_bytes());
            tokio::time::sleep(Duration::from_millis(250)).await;
        }
    });
    let out = tokio::time::timeout(secs(20), c.failover(&p.cfg, &p.ep)).await.expect("incidental activity did not conceal the failure");
    pump.abort();
    let AttachOutcome::Attached(deg) = out else { panic!("{out:?}") };
    assert_eq!(deg.ack.mode, 1);
}

// acceptance: SES-07
#[tokio::test]
async fn misses_are_counted_by_elapsed_intervals_not_by_callbacks() {
    let _serial = serial().await;
    let p = partitioned().await;
    let AttachOutcome::Attached(mut c) = attach(&p.cfg, &client_ep(), kh("alice"), p.path.addr, false).await else { panic!() };
    // the session is running: heartbeats have been exchanged both ways
    tokio::time::sleep(secs(I) + Duration::from_millis(300)).await;
    assert!(c.log.count(|e| matches!(e, Event::Received { frame_type: 3 })) > 0, "in a session");
    // N stops answering: the client sees silence rather than a close
    p.path.partition(true);
    // and the client's process is suspended, so no timer callback is
    // delivered while the monotonic clock runs on
    std::thread::sleep(secs(3 * I + 1));
    let started = std::time::Instant::now();
    // the count is what the rule is about: on waking, three intervals have
    // elapsed on the monotonic clock and the peer is judged unreachable
    // without waiting for three more callbacks
    while *c.reach.lock().unwrap() != Reachability::Unreachable {
        assert!(started.elapsed() < secs(I), "judged unreachable within one interval of waking, not after three more");
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    let judged = started.elapsed();
    assert!(judged < secs(I), "{judged:?}");
    // and the failover that follows reaches the sibling
    let out = tokio::time::timeout(secs(15), c.failover(&p.cfg, &client_ep())).await.expect("failover follows");
    let AttachOutcome::Attached(deg) = out else { panic!("{out:?}") };
    assert_eq!(deg.ack.mode, 1);
}

// acceptance: SES-09
#[tokio::test]
async fn a_restarted_client_uses_its_persisted_sibling_list_at_once() {
    let _serial = serial().await;
    let p = pair(None);
    let AttachOutcome::Attached(c) = attach(&p.cfg, &p.ep, kh("alice"), p.n.addr, false).await else { panic!() };
    let persisted = p.cfg.sibling_cache.lock().unwrap().clone();
    assert_eq!(persisted.len(), 1);
    drop(c);
    p.n.go_dark();
    // C starts afresh, carrying only what it persisted
    let fresh = client_cfg("carol");
    *fresh.sibling_cache.lock().unwrap() = persisted;
    know(&fresh, "alice", p.n.addr);
    know(&fresh, "bob", p.s.addr);
    let started = std::time::Instant::now();
    let out = fresh_attach(&fresh, &client_ep(), kh("alice"), false).await;
    let AttachOutcome::Attached(s) = out else { panic!("{out:?}") };
    assert_eq!(s.ack.mode, 1, "S accepts a sibling's client in failover");
    assert!(started.elapsed() < secs(3 * I), "without first waiting three intervals: {:?}", started.elapsed());
}

// acceptance: SES-10
#[tokio::test]
async fn a_degraded_session_produces_no_countersignature() {
    let _serial = serial().await;
    let p = pair(None);
    p.n.go_dark();
    let fresh = client_cfg("carol");
    *fresh.sibling_cache.lock().unwrap() = vec![sibling_ref("bob", p.s.addr)];
    know(&fresh, "alice", p.n.addr);
    know(&fresh, "bob", p.s.addr);
    let AttachOutcome::Attached(mut deg) = fresh_attach(&fresh, &client_ep(), kh("alice"), false).await else { panic!() };
    assert!(deg.degraded());
    assert!(!deg.may_countersign(), "the client will not ask a sibling to countersign");
    // C submits a subnet-scoped transaction that needs its patron N's
    // countersignature: an adoption body naming N as patron.  S is not the
    // party the body names, so an envelope S signs is not that transaction
    // and does not parse as one: no transaction countersigned by S exists.
    let mut signers = Signers::new();
    let pop = signers.formation("alice", "carol");
    let body = {
        let bn = vec![rhtn_archive::genesis(&kh("carol"))];
        let bp = vec![rhtn_archive::genesis(&kh("alice"))];
        let a = rhtn_archive::tx::Adoption {
            node: kh("carol"),
            patron: kh("alice"),
            locator: rhtn_archive::tx::Locator::root(kh("alice"), rhtn_archive::tx::Seqno { series: 5, counter: 0 }),
            timestamp: signers.clock,
            key_material: None,
            evidence: rhtn_archive::tx::Evidence::Presence(pop.txid),
            presented_head: None,
            back: [&bn, &bp],
        };
        rhtn_archive::tx::adoption_body(&a)
    };
    let mut s_view = view_of("bob", rhtn_archive::topology::Table::with_me(kh("bob")), "alice", &[1], signers.clock);
    assert!(s_view.countersign_adoption(&body, &id("carol"), &ids()).is_none(), "S's signature over a body naming N is no transaction at all");
    let by_n = rhtn_archive::tx::envelope(rhtn_archive::tx::TYPE_ADOPTION, &body, &[&id("carol"), &id("alice")]);
    assert!(rhtn_archive::record::Record::parse(&by_n).is_ok(), "whereas N's would be");
    // and payload flows on the same session
    p.s.node.enqueue(kh("carol"), b"payload".to_vec()).unwrap();
    let got = drain(&mut deg, 700).await;
    assert_eq!(got, vec![b"payload".to_vec()], "the payload is carried");
}

// acceptance: SES-11
#[tokio::test]
async fn there_is_no_automatic_failback() {
    let _serial = serial().await;
    let p = pair(Some(drop_heartbeats()));
    let AttachOutcome::Attached(mut c) = attach(&p.cfg, &p.ep, kh("alice"), p.n.addr, false).await else { panic!() };
    let AttachOutcome::Attached(deg) = tokio::time::timeout(secs(20), c.failover(&p.cfg, &p.ep)).await.unwrap() else { panic!() };
    assert!(deg.degraded());
    let attaches_before = p.n.node.log.count(|e| matches!(e, Event::Attached { .. }));
    // N is reachable again and stays so for six intervals
    tokio::time::sleep(secs(6 * I)).await;
    let attaches_after = p.n.node.log.count(|e| matches!(e, Event::Attached { .. }));
    assert_eq!(attaches_after, attaches_before, "C opens no connection to N");
    assert!(deg.conn.close_reason().is_none(), "the degraded session continues");
    let beats = deg.log.count(|e| matches!(e, Event::Received { frame_type: 3 }));
    assert!(beats > 0, "and keeps exchanging heartbeats with S");
}

// acceptance: SES-12
#[tokio::test]
async fn the_next_fresh_attach_tries_the_serving_node_first() {
    let _serial = serial().await;
    let p = pair(None);
    // C holds a degraded session on S
    let AttachOutcome::Attached(deg) = attach(&p.cfg, &p.ep, kh("bob"), p.s.addr, false).await else { panic!() };
    assert!(deg.degraded());
    deg.conn.close(0u32.into(), b"done");
    drop(deg);
    // N has since become reachable again: the fresh attach goes there first
    let before = p.s.node.log.count(|e| matches!(e, Event::Attached { .. }));
    let AttachOutcome::Attached(s) = fresh_attach(&p.cfg, &client_ep(), kh("alice"), false).await else { panic!() };
    assert_eq!(s.ack.mode, 0, "N answers with mode 0");
    assert!(p.n.node.has_session(&kh("carol")));
    assert_eq!(p.s.node.log.count(|e| matches!(e, Event::Attached { .. })), before, "S was not dialled");
}

/// N and S siblings, with a datagram path between C and N the test can
/// blackhole.
struct Partitioned {
    n: Running,
    s: Running,
    cfg: ClientConfig,
    path: Path,
}

async fn partitioned() -> Partitioned {
    let s = Running::start({
        let mut c = node_cfg("bob", I);
        c.in_subtree = Arc::new(|_| false);
        c
    });
    let sib = s.node.clone();
    let mut ncfg = node_cfg("alice", I);
    ncfg.siblings = {
        let list = vec![sibling_ref("bob", s.addr)];
        std::sync::Arc::new(move || list.clone())
    };
    // N replicates a client's reachability to its sibling
    ncfg.replicate = Some(Arc::new(move |client, r| sib.note_reachability(client, r)));
    let n = Running::start(ncfg);
    let path = Path::open(n.addr).await.unwrap();
    let cfg = client_cfg("carol");
    know(&cfg, "alice", path.addr);
    know(&cfg, "bob", s.addr);
    Partitioned { n, s, cfg, path }
}

// acceptance: SES-14
// acceptance: REP-01
#[tokio::test]
async fn a_clients_unreachability_replicates_to_the_siblings() {
    let _serial = serial().await;
    let p = partitioned().await;
    let AttachOutcome::Attached(c) = attach(&p.cfg, &client_ep(), kh("alice"), p.path.addr, false).await else { panic!() };
    assert_eq!(p.n.node.reachability(&kh("carol")), Some(Reachability::Reachable));
    // every packet from C to N is blackholed for three full intervals
    p.path.to_server.blackhole(true);
    tokio::time::sleep(secs(3 * I + 2)).await;
    assert_eq!(p.n.node.reachability(&kh("carol")), Some(Reachability::Unreachable), "N marks C unreachable");
    assert_eq!(p.s.node.reachability(&kh("carol")), Some(Reachability::Unreachable), "and S reads the same");
    assert!(p.path.to_server.dropped() > 0);
    drop(c);
}

// acceptance: QUE-10
#[tokio::test]
async fn material_queues_for_a_client_marked_unreachable() {
    let _serial = serial().await;
    let p = partitioned().await;
    let AttachOutcome::Attached(c) = attach(&p.cfg, &client_ep(), kh("alice"), p.path.addr, false).await else { panic!() };
    p.path.partition(true);
    tokio::time::sleep(secs(3 * I + 2)).await;
    assert_eq!(p.n.node.reachability(&kh("carol")), Some(Reachability::Unreachable));
    drop(c);
    // X submits one message while C is away
    p.n.node.enqueue(kh("carol"), b"while away".to_vec()).unwrap();
    p.path.partition(false);
    // C performs a fresh attach
    let mut back = match fresh_attach(&p.cfg, &client_ep(), kh("alice"), false).await {
        AttachOutcome::Attached(s) => s,
        other => panic!("{other:?}"),
    };
    assert_eq!(back.ack.queued, 1);
    assert_eq!(drain(&mut back, 700).await, vec![b"while away".to_vec()]);
}

// acceptance: QUE-11
#[tokio::test]
async fn an_offline_subordinate_differs_from_a_keyhash_with_no_record() {
    let _serial = serial().await;
    let mut cfg = node_cfg("alice", I);
    // N serves C and holds no record of Z
    cfg.serves = Arc::new(|k| *k != kh("w2"));
    let n = Running::start(cfg);
    let ccfg = client_cfg("carol");
    know(&ccfg, "alice", n.addr);
    let for_c = n.node.enqueue(kh("carol"), b"for C".to_vec());
    let for_z = n.node.enqueue(kh("w2"), b"for Z".to_vec());
    assert_eq!(for_c, Ok(()));
    assert_eq!(for_z, Err(Refusal::NoRecord), "a different answer from the one X received for C");
    assert!(n.node.queue_records(&kh("w2")).is_empty(), "nothing is queued for Z");
    let s = match attach(&ccfg, &client_ep(), kh("alice"), n.addr, false).await {
        AttachOutcome::Attached(s) => s,
        other => panic!("{other:?}"),
    };
    assert_eq!(s.ack.queued, 1, "C's next AttachAck carries 1");
}

// acceptance: QUE-12
// acceptance: REP-02
#[tokio::test]
async fn a_failover_sibling_holds_no_queue_state() {
    let _serial = serial().await;
    let p = pair(None);
    let msgs = messages(3, "at N");
    for m in &msgs {
        p.n.node.enqueue(kh("carol"), m.clone()).unwrap();
    }
    p.n.go_dark();
    let fresh = client_cfg("carol");
    *fresh.sibling_cache.lock().unwrap() = vec![sibling_ref("bob", p.s.addr)];
    know(&fresh, "alice", p.n.addr);
    know(&fresh, "bob", p.s.addr);
    let mut deg = match fresh_attach(&fresh, &client_ep(), kh("alice"), false).await {
        AttachOutcome::Attached(s) => s,
        other => panic!("{other:?}"),
    };
    assert_eq!(deg.ack.mode, 1, "field 1 is 1");
    assert_eq!(deg.ack.queued, 0, "field 4 does not count N's mailbox");
    assert!(drain(&mut deg, 700).await.is_empty(), "S delivers nothing");
    assert!(p.s.node.queue_records(&kh("carol")).is_empty(), "S's store holds no ciphertext for C");
    assert_eq!(p.n.node.queued(&kh("carol")), 3, "and N still holds all three");
}

// acceptance: QUE-13
#[tokio::test]
async fn a_client_collects_from_its_own_serving_node_when_it_returns() {
    let _serial = serial().await;
    let s = Running::start({
        let mut c = node_cfg("bob", I);
        c.in_subtree = Arc::new(|_| false);
        c
    });
    let ncfg = {
        let mut c = node_cfg("alice", I);
        c.siblings = {
        let list = vec![sibling_ref("bob", s.addr)];
        std::sync::Arc::new(move || list.clone())
    };
        c
    };
    let n = Running::start(ncfg);
    let path = Path::open(n.addr).await.unwrap();
    let cfg = client_cfg("carol");
    know(&cfg, "alice", path.addr);
    know(&cfg, "bob", s.addr);
    // C has attached to N before, so it holds N's sibling list
    let first = match fresh_attach(&cfg, &client_ep(), kh("alice"), false).await {
        AttachOutcome::Attached(x) => x,
        other => panic!("{other:?}"),
    };
    assert_eq!(first.ack.mode, 0);
    first.conn.close(0u32.into(), b"done");
    drop(first);
    tokio::time::sleep(Duration::from_millis(200)).await;
    // k messages are queued at N, and N then goes dark
    let msgs = messages(4, "waiting");
    for m in &msgs {
        n.node.enqueue(kh("carol"), m.clone()).unwrap();
    }
    path.partition(true);
    let deg = match fresh_attach(&cfg, &client_ep(), kh("alice"), false).await {
        AttachOutcome::Attached(x) => x,
        other => panic!("{other:?}"),
    };
    assert!(deg.degraded(), "C holds a degraded session on the sibling");
    // N returns; C's session on S ends and it attaches afresh
    path.partition(false);
    deg.conn.close(0u32.into(), b"done");
    drop(deg);
    let mut back = match fresh_attach(&cfg, &client_ep(), kh("alice"), false).await {
        AttachOutcome::Attached(x) => x,
        other => panic!("{other:?}"),
    };
    assert_eq!(back.ack.mode, 0, "C attaches to N");
    assert_eq!(back.ack.queued, 4);
    let got = drain(&mut back, 900).await;
    assert_eq!(got.len(), 4);
    let mut a = got.clone();
    let mut b = msgs.clone();
    a.sort();
    b.sort();
    assert_eq!(a, b, "all k ciphertexts are delivered");
}

// acceptance: REP-15
#[tokio::test]
async fn the_pushed_replication_set_is_the_serving_nodes_siblings() {
    let _serial = serial().await;
    // R (alice2) over S (alice), T1 (w1) and T2 (w2), all infra; P (bob), a
    // light client, under S; Q (c1), P's light-client sibling, under S too;
    // L (carol) under P
    let mut sg = Signers::new();
    let a_s = sg.adopt("alice", "alice2", "alice2", &[0], 1);
    let a_t1 = sg.adopt("w1", "alice2", "alice2", &[1], 2);
    let a_t2 = sg.adopt("w2", "alice2", "alice2", &[2], 3);
    let a_p = sg.adopt("bob", "alice", "alice2", &[0, 0], 4);
    let a_q = sg.adopt("c1", "alice", "alice2", &[0, 1], 5);
    let a_l = sg.adopt("carol", "bob", "alice2", &[0, 0, 0], 6);
    let s_view = view_of("alice", table_of("alice", &sg, &[&a_s, &a_t1, &a_t2, &a_p, &a_q, &a_l], &["alice2", "alice", "w1", "w2"]), "alice2", &[0], sg.clock);
    let t1 = Running::start(node_cfg("w1", I));
    let t2 = Running::start(node_cfg("w2", I));
    let known = [(kh("w1"), t1.addr), (kh("w2"), t2.addr)];
    // the list S pushes is derived from its own table: its siblings, not P's
    let set = s_view.replication_set();
    assert_eq!(set, [kh("w1"), kh("w2")].into_iter().collect());
    assert!(!set.contains(&kh("c1")), "Q is P's sibling, not S's");
    let mut cfg = node_cfg("alice", I);
    cfg.siblings = {
        let list: Vec<rhtn_transport::session::SiblingRef> =
            set.iter().map(|k| known.iter().find(|(x, _)| x == k).map(|(_, a)| sibling_ref(if *k == kh("w1") { "w1" } else { "w2" }, *a)).unwrap()).collect();
        std::sync::Arc::new(move || list.clone())
    };
    let s = Running::start(cfg);
    let ccfg = client_cfg("carol");
    know(&ccfg, "alice", s.addr);
    let sess = match attach(&ccfg, &client_ep(), kh("alice"), s.addr, false).await {
        AttachOutcome::Attached(x) => x,
        other => panic!("{other:?}"),
    };
    let named: std::collections::BTreeSet<[u8; 32]> = sess.ack.siblings.iter().map(|r| r.keyhash).collect();
    assert_eq!(named, [kh("w1"), kh("w2")].into_iter().collect(), "T1 and T2 with their endpoints");
    assert!(!named.contains(&kh("c1")), "and not the patron's sibling Q");
    assert!(sess.ack.siblings.iter().all(|r| r.key_material.is_some()));
}
