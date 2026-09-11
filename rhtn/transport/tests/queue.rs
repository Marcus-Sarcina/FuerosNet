//! Queue entries (QUE) over real loopback QUIC, and the serving-node
//! computation a light client under a light-client patron makes (TOP-13).

use rhtn_archive::record::Record;
use rhtn_archive::topology::{Supersession, Table};
use rhtn_archive::tx::*;
use rhtn_archive::{Keyhash, genesis};
use rhtn_crypto::identity::testkit::test_identity;
use rhtn_transport::queue::{Queued, Refusal};
use rhtn_transport::session::*;
use rhtn_transport::tls::{self, Pins};
use std::collections::BTreeMap;
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};
use tokio::time::{Duration, sleep, timeout};

const NAMES: [&str; 8] = ["alice", "bob", "carol", "alice2", "w1", "w2", "w3", "c1"];

fn loopback() -> SocketAddr {
    "127.0.0.1:0".parse().unwrap()
}

fn pins() -> Pins {
    let p = Pins::new();
    for n in NAMES {
        p.pin_identity(&test_identity(n).public);
    }
    p
}

fn kh(n: &str) -> Keyhash {
    test_identity(n).public.keyhash
}

fn node_cfg(name: &str) -> NodeConfig {
    NodeConfig::defaults(Arc::new(test_identity(name)), pins(), 30)
}

fn client_cfg(name: &str) -> ClientConfig {
    ClientConfig {
        identity: Arc::new(test_identity(name)),
        pins: pins(),
        capabilities: BTreeMap::new(),
        attestation: None,
        filter: None,
        sibling_cache: Arc::new(Mutex::new(Vec::new())),
        addresses: Arc::new(Mutex::new(Default::default())),
        tls: Arc::new(Mutex::new(Default::default())),
        connect_timeout: std::time::Duration::from_millis(1500),
        on_reachability: None,
    }
}

fn spawn_node(cfg: NodeConfig) -> (Arc<Node>, SocketAddr, quinn::Endpoint) {
    let ep = tls::server_endpoint(&cfg.identity, loopback()).unwrap();
    let addr = ep.local_addr().unwrap();
    let node = Node::new(cfg);
    tokio::spawn(node.clone().serve(ep.clone()));
    (node, addr, ep)
}

fn client_ep() -> quinn::Endpoint {
    tls::client_endpoint(loopback()).unwrap()
}

async fn attach_ok(cfg: &ClientConfig, target: &str, addr: SocketAddr) -> Session {
    match attach(cfg, &client_ep(), kh(target), addr, false).await {
        AttachOutcome::Attached(s) => s,
        other => panic!("{other:?}"),
    }
}

/// Read every drain delivery that arrives within a short window.
async fn drain(s: &mut Session) -> Vec<Vec<u8>> {
    let mut got = Vec::new();
    while let Ok(Some(b)) = timeout(Duration::from_millis(400), s.deliveries.recv()).await {
        got.push(b);
    }
    got
}

fn messages(k: usize, tag: &str) -> Vec<Vec<u8>> {
    (0..k).map(|i| format!("{tag}:{i}:{}", "x".repeat(40)).into_bytes()).collect()
}

fn same_set(a: &[Vec<u8>], b: &[Vec<u8>]) -> bool {
    let mut a = a.to_vec();
    let mut b = b.to_vec();
    a.sort();
    b.sort();
    a == b
}

// acceptance: QUE-01
#[tokio::test]
async fn queued_messages_are_counted_at_the_next_attach() {
    let (node, addr, _ep) = spawn_node(node_cfg("alice"));
    let k = 5;
    for m in messages(k, "q1") {
        assert_eq!(node.enqueue(kh("carol"), m), Ok(()));
    }
    let s = attach_ok(&client_cfg("carol"), "alice", addr).await;
    assert_eq!(s.ack.queued, k as u64);
}

// acceptance: QUE-02
#[tokio::test]
async fn queued_ciphertext_is_delivered_unchanged() {
    let (node, addr, _ep) = spawn_node(node_cfg("alice"));
    let sent = messages(6, "q2");
    for m in &sent {
        node.enqueue(kh("carol"), m.clone()).unwrap();
    }
    let mut s = attach_ok(&client_cfg("carol"), "alice", addr).await;
    let got = drain(&mut s).await;
    assert_eq!(got.len(), sent.len());
    assert!(same_set(&got, &sent), "byte for byte");
}

// acceptance: QUE-03
#[tokio::test]
async fn delete_on_delivery_leaves_nothing_for_a_second_attach() {
    let (node, addr, _ep) = spawn_node(node_cfg("alice"));
    let sent = messages(4, "q3");
    for m in &sent {
        node.enqueue(kh("carol"), m.clone()).unwrap();
    }
    let mut s = attach_ok(&client_cfg("carol"), "alice", addr).await;
    assert_eq!(drain(&mut s).await.len(), 4);
    s.conn.close(0u32.into(), b"");
    drop(s);
    sleep(Duration::from_millis(200)).await;
    assert!(node.queue_records(&kh("carol")).is_empty(), "no copy in the store");
    let mut s2 = attach_ok(&client_cfg("carol"), "alice", addr).await;
    assert_eq!(s2.ack.queued, 0);
    assert!(drain(&mut s2).await.is_empty());
}

// acceptance: QUE-05
#[tokio::test]
async fn a_waiting_message_carries_the_minimum() {
    let mut cfg = node_cfg("alice");
    cfg.clock = Arc::new(|| 1_900_000_000);
    let (node, _addr, _ep) = spawn_node(cfg);
    let m = b"one message from X".to_vec();
    node.enqueue(kh("carol"), m.clone()).unwrap();
    let recs = node.queue_records(&kh("carol"));
    assert_eq!(recs.len(), 1);
    // the record is exactly these three fields, and nothing names or locates X
    let Queued { ciphertext, recipient, arrival } = recs[0].clone();
    assert_eq!(ciphertext, m);
    assert_eq!(recipient, kh("carol"));
    assert_eq!(arrival, 1_900_000_000);
    assert_eq!(recs[0], Queued { ciphertext: m, recipient: kh("carol"), arrival: 1_900_000_000 });
}

/// A node whose per-subordinate cap fits exactly `k` of the test messages.
fn capped_node(k: usize) -> (Arc<Node>, SocketAddr, quinn::Endpoint, Vec<Vec<u8>>) {
    let msgs = messages(k, "cap");
    let mut cfg = node_cfg("alice");
    cfg.queue_cap = Some(msgs.iter().map(|m| m.len()).sum());
    let (node, addr, ep) = spawn_node(cfg);
    (node, addr, ep, msgs)
}

// acceptance: QUE-06
#[tokio::test]
async fn at_the_cap_the_newest_is_refused_and_the_rest_kept() {
    let (node, addr, _ep, msgs) = capped_node(4);
    for m in &msgs {
        assert_eq!(node.enqueue(kh("carol"), m.clone()), Ok(()));
    }
    let extra = b"cap:4:the one over".to_vec();
    assert_eq!(node.enqueue(kh("carol"), extra.clone()), Err(Refusal::AtCap), "X is told, distinguishably");
    let mut s = attach_ok(&client_cfg("carol"), "alice", addr).await;
    assert_eq!(s.ack.queued, 4);
    let got = drain(&mut s).await;
    assert!(same_set(&got, &msgs));
    assert!(!got.contains(&extra));
}

// acceptance: QUE-07
#[tokio::test]
async fn a_flood_displaces_nothing_already_queued() {
    let (node, addr, _ep, msgs) = capped_node(3);
    for m in &msgs {
        node.enqueue(kh("carol"), m.clone()).unwrap();
    }
    let flood = messages(5 * 3, "attacker");
    for f in &flood {
        assert_eq!(node.enqueue(kh("carol"), f.clone()), Err(Refusal::AtCap));
    }
    let mut s = attach_ok(&client_cfg("carol"), "alice", addr).await;
    assert_eq!(s.ack.queued, 3);
    let got = drain(&mut s).await;
    assert!(same_set(&got, &msgs), "the legitimate three, none replaced");
}

// acceptance: QUE-08
#[tokio::test]
async fn a_message_survives_an_absence_of_any_length() {
    let now = Arc::new(Mutex::new(1_800_000_000u64));
    let mut cfg = node_cfg("alice");
    let c = now.clone();
    cfg.clock = Arc::new(move || *c.lock().unwrap());
    let (node, addr, _ep) = spawn_node(cfg);
    node.enqueue(kh("carol"), b"before the absence".to_vec()).unwrap();
    let d = 20 * 365 * 86_400; // twenty years
    *now.lock().unwrap() += d;
    sleep(Duration::from_millis(100)).await;
    let mut s = attach_ok(&client_cfg("carol"), "alice", addr).await;
    assert_eq!(s.ack.queued, 1);
    assert_eq!(drain(&mut s).await, vec![b"before the absence".to_vec()]);
}

// acceptance: QUE-09
#[tokio::test]
async fn the_cap_is_per_subordinate() {
    let (node, addr, _ep, msgs) = capped_node(3);
    for m in &msgs {
        node.enqueue(kh("carol"), m.clone()).unwrap();
    }
    assert_eq!(node.enqueue(kh("carol"), msgs[0].clone()), Err(Refusal::AtCap), "C1 is at its cap");
    assert_eq!(node.enqueue(kh("bob"), b"for c2".to_vec()), Ok(()), "C2's mail is accepted");
    let mut s = attach_ok(&client_cfg("bob"), "alice", addr).await;
    assert_eq!(s.ack.queued, 1);
    assert_eq!(drain(&mut s).await, vec![b"for c2".to_vec()]);
}

/// A validated recovery adoption of `new` under `patron` claiming `old`,
/// with `verifier` recognising the subject: the evidence a node takes.
fn recovery_evidence(old: &str, new: &str, patron: &str, verifier: &str) -> Supersession {
    let (o, n, p, v) = (test_identity(old), test_identity(new), test_identity(patron), test_identity(verifier));
    let qid = rhtn_codec::cose::sha256(b"recovery query");
    let resp = recovery_response(&v, &n, &qid, &o.public.keyhash);
    let block = recovery_block(&o, &n.public.keyhash, &p.public.keyhash, vec![resp]);
    let a = Adoption {
        node: n.public.keyhash,
        patron: p.public.keyhash,
        locator: Locator::root(p.public.keyhash, Seqno { series: 4, counter: 0 }),
        timestamp: 1_800_000_000,
        key_material: None,
        evidence: Evidence::Recovery(block),
        presented_head: None,
        back: [&[genesis(&n.public.keyhash)], &[genesis(&p.public.keyhash)]],
    };
    let env = envelope(TYPE_ADOPTION, &adoption_body(&a), &[&n, &p]);
    let rec = Record::parse(&env).unwrap();
    let ids: Vec<_> = NAMES.iter().map(|x| test_identity(x).public).collect();
    Supersession::from_record(&rec, &ids).expect("validated recovery")
}

// acceptance: QUE-14
#[tokio::test]
async fn a_running_session_ends_on_supersession_evidence() {
    let (node, addr, _ep) = spawn_node(node_cfg("alice"));
    let mut s = attach_ok(&client_cfg("bob"), "alice", addr).await;
    assert!(node.has_session(&kh("bob")));
    let ev = recovery_evidence("bob", "carol", "alice", "w1");
    assert_eq!((ev.superseded, ev.successor), (kh("bob"), kh("carol")));
    node.supersede(ev);
    // nothing reaches the session after the evidence was taken
    assert_eq!(node.enqueue(kh("bob"), b"late".to_vec()), Err(Refusal::Superseded));
    let closed = timeout(Duration::from_secs(3), s.conn.closed()).await;
    assert!(closed.is_ok(), "N closes C's connection");
    assert!(drain(&mut s).await.is_empty());
    assert!(!node.has_session(&kh("bob")));
}

// acceptance: QUE-15
#[tokio::test]
async fn nothing_is_served_to_a_superseded_credential() {
    let (node, addr, _ep) = spawn_node(node_cfg("alice"));
    for m in messages(3, "q15") {
        node.enqueue(kh("bob"), m).unwrap();
    }
    node.supersede(recovery_evidence("bob", "carol", "alice", "w1"));
    let outcome = attach(&client_cfg("bob"), &client_ep(), kh("alice"), addr, false).await;
    assert!(!matches!(outcome, AttachOutcome::Attached(_)), "no AttachAck: {outcome:?}");
    assert_eq!(node.log.count(|e| matches!(e, Event::Attached { .. })), 0);
    assert_eq!(node.log.count(|e| *e == Event::Superseded), 1);
    assert_eq!(node.log.count(|e| matches!(e, Event::Delivered { .. })), 0);
}

// acceptance: QUE-16
#[tokio::test]
async fn a_successor_collects_nothing_queued_for_its_predecessor() {
    let (node, addr, _ep) = spawn_node(node_cfg("alice"));
    let old_mail = messages(3, "q16");
    for m in &old_mail {
        node.enqueue(kh("bob"), m.clone()).unwrap();
    }
    node.supersede(recovery_evidence("bob", "carol", "alice", "w1"));
    let mut s = attach_ok(&client_cfg("carol"), "alice", addr).await;
    assert_eq!(s.ack.queued, 0);
    let got = drain(&mut s).await;
    assert!(got.is_empty());
    assert!(got.iter().all(|g| !old_mail.contains(g)));
}

// acceptance: TOP-13
#[tokio::test]
async fn a_light_client_under_a_light_client_attaches_to_the_nearest_infra() {
    // I (alice, infra) over A (bob, light) over B (carol, light)
    let mut table = Table::with_me(kh("alice"));
    table.mark_infra(kh("alice"));
    let ids: Vec<_> = NAMES.iter().map(|x| test_identity(x).public).collect();
    let store: BTreeMap<[u8; 32], Vec<u8>> = BTreeMap::new();
    let mut heads: BTreeMap<Keyhash, [u8; 32]> = BTreeMap::new();
    let mut adopt = |node: &str, patron: &str, table: &mut Table| {
        let (n, p) = (test_identity(node), test_identity(patron));
        let head = |k: &Keyhash| heads.get(k).copied().unwrap_or(genesis(k));
        let (bn, bp) = (head(&n.public.keyhash), head(&p.public.keyhash));
        // fresh keys form; an established key meets on a witnessed normal
        // record, a key forming only once (`wire-format.md` §3.2)
        let fresh = !heads.contains_key(&n.public.keyhash) && !heads.contains_key(&p.public.keyhash);
        let w = test_identity("w1");
        let pop = if fresh {
            let pop_body = formation_body([&[bn], &[bp]], [&n.public.keyhash, &p.public.keyhash], 1_800_000_000, 1_800_000_600, &[7u8; 32]);
            Record::parse(&envelope(TYPE_PRESENCE, &pop_body, &[&n, &p])).unwrap()
        } else {
            let witness = Witness { keyhash: w.public.keyhash, nominated_by: n.public.keyhash, flags: 3 };
            let back = vec![vec![bn], vec![bp], vec![head(&w.public.keyhash)]];
            let pop_body = presence_record_body(&back, [&n.public.keyhash, &p.public.keyhash], &[witness], 1_800_000_000, 1_800_000_600, &[7u8; 32]);
            let rec = Record::parse(&envelope(TYPE_PRESENCE, &pop_body, &[&n, &p, &w])).unwrap();
            heads.insert(w.public.keyhash, rec.txid);
            rec
        };
        let mut st = store.clone();
        st.insert(pop.txid, pop.bytes.clone());
        let a = Adoption {
            node: n.public.keyhash,
            patron: p.public.keyhash,
            locator: Locator::root(p.public.keyhash, Seqno { series: 1, counter: 0 }),
            timestamp: 1_800_001_000,
            key_material: None,
            evidence: Evidence::Presence(pop.txid),
            presented_head: None,
            back: [&[pop.txid], &[pop.txid]],
        };
        let rec = Record::parse(&envelope(TYPE_ADOPTION, &adoption_body(&a), &[&n, &p])).unwrap();
        heads.insert(n.public.keyhash, rec.txid);
        heads.insert(p.public.keyhash, rec.txid);
        table.apply(&rec, &ids, &st, None).unwrap();
    };
    adopt("bob", "alice", &mut table);
    adopt("carol", "bob", &mut table);
    // B's client holds the same topology and computes its serving node
    assert_eq!(table.serving_node(&kh("carol")), Some(kh("alice")), "past the light-client patron to I");
    let shared = Arc::new(Mutex::new(table));
    let mut cfg = node_cfg("alice");
    let t = shared.clone();
    cfg.in_subtree = Arc::new(move |k| t.lock().unwrap().downline_contains(&kh("alice"), k));
    let (node, addr, _ep) = spawn_node(cfg);
    let s = attach_ok(&client_cfg("carol"), "alice", addr).await;
    assert_eq!(s.ack.mode, 0, "primary");
    assert!(node.has_session(&kh("carol")));
    let path = shared.lock().unwrap().attach_client(&kh("carol")).unwrap();
    assert_eq!(path, vec![kh("bob"), kh("carol")], "listed by path, beneath A");
    assert!(shared.lock().unwrap().attached_clients().contains_key(&kh("carol")));
}
