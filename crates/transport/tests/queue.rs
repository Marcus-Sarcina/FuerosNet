//! Queue entries (QUE) over real loopback QUIC, the minted connection
//! (`wire-format.md` §9.1), and the serving-node computation a light
//! client under a light-client patron makes (TOP-13).

use rhtn_archive::record::Record;
use rhtn_archive::topology::{Supersession, Table};
use rhtn_archive::tx::*;
use rhtn_archive::{Keyhash, genesis};
use rhtn_crypto::identity::testkit::test_identity;
use rhtn_transport::queue::{Queued, Refusal};

mod common;
use common::{SealKeys, delivered, opaque};
use rhtn_transport::session::*;
use rhtn_transport::tls::{self, Party, Pins};
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
    let mut cfg = NodeConfig::defaults(Arc::new(test_identity(name)), pins(), 30);
    cfg.log = Log::recording();
    cfg
}

fn client_cfg(name: &str) -> ClientConfig {
    ClientConfig {
        me: Party::of(Arc::new(test_identity(name))),
        pins: pins(),
        bind: Default::default(),
        capabilities: BTreeMap::new(),
        attestation: None,
        filter: None,
        sibling_cache: Arc::new(Mutex::new(Vec::new())),
        addresses: Arc::new(Mutex::new(Default::default())),
        cursors: Default::default(),
        tls: Arc::new(Mutex::new(Default::default())),
        connect_timeout: std::time::Duration::from_millis(1500),
        on_reachability: None,
        log: Log::recording(),
    }
}

fn spawn_node(cfg: NodeConfig) -> (Arc<Node>, SocketAddr, quinn::Endpoint) {
    let ep = tls::server_endpoint(cfg.presenter(), loopback()).unwrap();
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

/// The sealed messages a run of deliveries carried, each read out of its
/// `RelayedPayload`.
fn seals(deliveries: &[Vec<u8>]) -> Vec<Vec<u8>> {
    deliveries.iter().map(|d| delivered(d).1).collect()
}

/// `k` sealed messages of one size, told apart by `tag`.
fn messages(k: usize, tag: &str) -> Vec<Vec<u8>> {
    (0..k)
        .map(|i| opaque(format!("{tag}:{i:02}:{}", "x".repeat(40)).as_bytes()))
        .collect()
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
    for (i, m) in messages(k, "q1").into_iter().enumerate() {
        assert_eq!(node.enqueue(kh("carol"), m, 1), Ok(i as u64 + 1));
    }
    let s = attach_ok(&client_cfg("carol"), "alice", addr).await;
    assert_eq!(s.ack.queued, k as u64);
}

// acceptance: QUE-02
#[tokio::test]
async fn queued_ciphertext_is_delivered_unchanged_with_the_nodes_half() {
    let keys = SealKeys::new(kh("carol"), kh("alice"));
    let mut cfg = node_cfg("alice");
    cfg.open_half = Some(keys.open_half());
    let (node, addr, _ep) = spawn_node(cfg);
    let bodies: Vec<Vec<u8>> = (0..6).map(|i| format!("q2:{i}").into_bytes()).collect();
    let sent: Vec<Vec<u8>> = bodies
        .iter()
        .enumerate()
        .map(|(i, b)| keys.seal(1, b, i as u8))
        .collect();
    for m in &sent {
        node.enqueue(kh("carol"), m.clone(), 1).unwrap();
    }
    let mut s = attach_ok(&client_cfg("carol"), "alice", addr).await;
    let got = drain(&mut s).await;
    assert_eq!(got.len(), sent.len());
    assert!(same_set(&seals(&got), &sent), "byte for byte");
    // each beside the half that opens it
    let mut opened: Vec<Vec<u8>> = got.iter().map(|d| keys.open(d).expect("opens")).collect();
    opened.sort();
    assert_eq!(opened, bodies);
}

// acceptance: QUE-03
#[tokio::test]
async fn a_device_naming_what_it_holds_is_handed_only_what_is_newer() {
    let (node, addr, _ep) = spawn_node(node_cfg("alice"));
    let sent = messages(4, "q3");
    for m in &sent {
        node.enqueue(kh("carol"), m.clone(), 1).unwrap();
    }
    let ccfg = client_cfg("carol");
    let mut s = attach_ok(&ccfg, "alice", addr).await;
    let first = drain(&mut s).await;
    assert_eq!(first.len(), 4, "no field 5: everything held");
    let newest = first.iter().map(|d| delivered(d).0).max().unwrap();
    s.conn.close(0u32.into(), b"");
    drop(s);
    sleep(Duration::from_millis(200)).await;
    let late = opaque(b"q3: the one after");
    let late_at = node.enqueue(kh("carol"), late.clone(), 1).unwrap();
    assert!(late_at > newest, "a later arrival takes a greater position");
    // the device names the newest position it received (Attach field 5)
    ccfg.cursors.lock().unwrap().insert(kh("alice"), newest);
    let mut s2 = attach_ok(&ccfg, "alice", addr).await;
    assert_eq!(s2.ack.queued, 1, "field 4 counts what is newer");
    let second = drain(&mut s2).await;
    assert_eq!(seals(&second), vec![late.clone()], "exactly the newer one");
    // and nothing was removed by having been delivered
    let mut held: Vec<Vec<u8>> = node
        .queue_records(&kh("carol"))
        .into_iter()
        .map(|q| q.sealed)
        .collect();
    held.sort();
    let mut all = sent.clone();
    all.push(late);
    all.sort();
    assert_eq!(held, all, "all k + 1 still held");
}

// acceptance: QUE-05
#[tokio::test]
async fn a_waiting_message_carries_the_minimum() {
    let now = Arc::new(Mutex::new(1_900_000_000u64));
    let mut cfg = node_cfg("alice");
    let c = now.clone();
    cfg.clock = Arc::new(move || *c.lock().unwrap());
    let (node, addr, _ep) = spawn_node(cfg);
    let m = opaque(b"one message from X");
    let position = node.enqueue(kh("carol"), m.clone(), 7).unwrap();
    let recs = node.queue_records(&kh("carol"));
    assert_eq!(recs.len(), 1);
    // the record is exactly these six fields, and nothing names or locates
    // X; destructured whole, so a seventh field fails to compile here
    let Queued {
        sealed,
        recipient,
        n,
        position: at,
        arrival,
        delivered_at,
    } = recs[0].clone();
    assert_eq!(sealed, m);
    assert_eq!(recipient, kh("carol"));
    assert_eq!(n, 7, "the node key it was sealed to");
    assert_eq!(at, position);
    assert_eq!(arrival, 1_900_000_000);
    assert_eq!(delivered_at, None, "not yet collected");
    // collected later: the time of first delivery is added, and nothing
    // naming the device that collected it
    *now.lock().unwrap() += 60;
    let mut s = attach_ok(&client_cfg("carol"), "alice", addr).await;
    assert_eq!(drain(&mut s).await.len(), 1);
    sleep(Duration::from_millis(100)).await;
    assert_eq!(
        node.queue_records(&kh("carol")),
        vec![Queued {
            sealed: m,
            recipient: kh("carol"),
            n: 7,
            position,
            arrival: 1_900_000_000,
            delivered_at: Some(1_900_000_060),
        }]
    );
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
        assert!(node.enqueue(kh("carol"), m.clone(), 1).is_ok());
    }
    let extra = opaque(b"cap:04:the one over the cap, nothing delivered");
    assert_eq!(
        node.enqueue(kh("carol"), extra.clone(), 1),
        Err(Refusal::AtCap),
        "X is told, distinguishably"
    );
    let mut s = attach_ok(&client_cfg("carol"), "alice", addr).await;
    assert_eq!(s.ack.queued, 4);
    let got = seals(&drain(&mut s).await);
    assert!(same_set(&got, &msgs));
    assert!(!got.contains(&extra));
}

// acceptance: QUE-07
#[tokio::test]
async fn a_flood_displaces_nothing_already_queued() {
    let (node, addr, _ep, msgs) = capped_node(3);
    for m in &msgs {
        node.enqueue(kh("carol"), m.clone(), 1).unwrap();
    }
    let flood = messages(5 * 3, "attacker");
    for f in &flood {
        assert_eq!(node.enqueue(kh("carol"), f.clone(), 1), Err(Refusal::AtCap));
    }
    let mut s = attach_ok(&client_cfg("carol"), "alice", addr).await;
    assert_eq!(s.ack.queued, 3);
    let got = seals(&drain(&mut s).await);
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
    let m = opaque(b"before the absence");
    node.enqueue(kh("carol"), m.clone(), 1).unwrap();
    let d = 20 * 365 * 86_400; // twenty years
    *now.lock().unwrap() += d;
    sleep(Duration::from_millis(100)).await;
    assert_eq!(node.expire(), 0, "an undelivered message has no retention");
    let mut s = attach_ok(&client_cfg("carol"), "alice", addr).await;
    assert_eq!(s.ack.queued, 1);
    assert_eq!(seals(&drain(&mut s).await), vec![m]);
}

// acceptance: QUE-09
#[tokio::test]
async fn the_cap_is_per_subordinate() {
    let (node, addr, _ep, msgs) = capped_node(3);
    for m in &msgs {
        node.enqueue(kh("carol"), m.clone(), 1).unwrap();
    }
    assert_eq!(
        node.enqueue(kh("carol"), msgs[0].clone(), 1),
        Err(Refusal::AtCap),
        "C1 is at its cap"
    );
    let for_c2 = opaque(b"for c2");
    assert!(
        node.enqueue(kh("bob"), for_c2.clone(), 1).is_ok(),
        "C2's mail is accepted"
    );
    let mut s = attach_ok(&client_cfg("bob"), "alice", addr).await;
    assert_eq!(s.ack.queued, 1);
    assert_eq!(seals(&drain(&mut s).await), vec![for_c2]);
}

/// A validated recovery adoption of `new` under `patron` claiming `old`,
/// with `verifier` recognising the subject: the evidence a node takes.
fn recovery_evidence(old: &str, new: &str, patron: &str, verifier: &str) -> Supersession {
    let (o, n, p, v) = (
        test_identity(old),
        test_identity(new),
        test_identity(patron),
        test_identity(verifier),
    );
    let qid = rhtn_codec::cose::sha256(b"recovery query");
    let resp = recovery_response(&v, &n, &qid, &o.public.keyhash);
    let block = recovery_block(&o, &n.public.keyhash, &p.public.keyhash, vec![resp]);
    let a = Adoption {
        node: n.public.keyhash,
        patron: p.public.keyhash,
        locator: Locator::root(
            p.public.keyhash,
            Seqno {
                series: 4,
                counter: 0,
            },
        ),
        timestamp: 1_800_000_000,
        key_material: None,
        evidence: Evidence::Recovery(block),
        presented_head: None,
        back: [&[genesis(&n.public.keyhash)], &[genesis(&p.public.keyhash)]],
        carrying: None,
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
    assert_eq!(
        node.enqueue(kh("bob"), opaque(b"late"), 1),
        Err(Refusal::Superseded)
    );
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
        node.enqueue(kh("bob"), m, 1).unwrap();
    }
    node.supersede(recovery_evidence("bob", "carol", "alice", "w1"));
    let outcome = attach(&client_cfg("bob"), &client_ep(), kh("alice"), addr, false).await;
    assert!(
        !matches!(outcome, AttachOutcome::Attached(_)),
        "no AttachAck: {outcome:?}"
    );
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
        node.enqueue(kh("bob"), m.clone(), 1).unwrap();
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
        let fresh =
            !heads.contains_key(&n.public.keyhash) && !heads.contains_key(&p.public.keyhash);
        let w = test_identity("w1");
        let pop = if fresh {
            let pop_body = formation_body(
                [&[bn], &[bp]],
                [&n.public.keyhash, &p.public.keyhash],
                1_800_000_000,
                1_800_000_600,
                &[7u8; 32],
            );
            Record::parse(&envelope(TYPE_PRESENCE, &pop_body, &[&n, &p])).unwrap()
        } else {
            let witness = Witness {
                keyhash: w.public.keyhash,
                nominated_by: n.public.keyhash,
                flags: 3,
            };
            let back = vec![vec![bn], vec![bp], vec![head(&w.public.keyhash)]];
            let pop_body = presence_record_body(
                &back,
                [&n.public.keyhash, &p.public.keyhash],
                &[witness],
                1_800_000_000,
                1_800_000_600,
                &[7u8; 32],
            );
            let rec = Record::parse(&envelope(TYPE_PRESENCE, &pop_body, &[&n, &p, &w])).unwrap();
            heads.insert(w.public.keyhash, rec.txid);
            rec
        };
        let mut st = store.clone();
        st.insert(pop.txid, pop.bytes.clone());
        let a = Adoption {
            node: n.public.keyhash,
            patron: p.public.keyhash,
            locator: Locator::root(
                p.public.keyhash,
                Seqno {
                    series: 1,
                    counter: 0,
                },
            ),
            timestamp: 1_800_001_000,
            key_material: None,
            evidence: Evidence::Presence(pop.txid),
            presented_head: None,
            back: [&[pop.txid], &[pop.txid]],
            carrying: None,
        };
        let rec = Record::parse(&envelope(TYPE_ADOPTION, &adoption_body(&a), &[&n, &p])).unwrap();
        heads.insert(n.public.keyhash, rec.txid);
        heads.insert(p.public.keyhash, rec.txid);
        table.apply(&rec, &ids, &st, None).unwrap();
    };
    adopt("bob", "alice", &mut table);
    adopt("carol", "bob", &mut table);
    // B's client holds the same topology and computes its serving node
    assert_eq!(
        table.serving_node(&kh("carol")),
        Some(kh("alice")),
        "past the light-client patron to I"
    );
    let shared = Arc::new(Mutex::new(table));
    let mut cfg = node_cfg("alice");
    let t = shared.clone();
    cfg.in_subtree = Arc::new(move |k| t.lock().unwrap().downline_contains(&kh("alice"), k));
    let (node, addr, _ep) = spawn_node(cfg);
    let s = attach_ok(&client_cfg("carol"), "alice", addr).await;
    assert_eq!(s.ack.mode, 0, "primary");
    assert!(node.has_session(&kh("carol")));
    let path = shared.lock().unwrap().attach_client(&kh("carol")).unwrap();
    assert_eq!(
        path,
        vec![kh("bob"), kh("carol")],
        "listed by path, beneath A"
    );
    assert!(
        shared
            .lock()
            .unwrap()
            .attached_clients()
            .contains_key(&kh("carol"))
    );
}

// acceptance: QUE-19
#[tokio::test]
async fn a_delivery_the_device_never_took_is_handed_again_and_nothing_is_lost() {
    let (node, addr, _ep) = spawn_node(node_cfg("alice"));
    let sent = messages(40, "q19");
    for m in &sent {
        node.enqueue(kh("carol"), m.clone(), 1).unwrap();
    }
    assert_eq!(node.queued(&kh("carol")), 40);
    // C attaches with no field 5, takes what arrives in a moment, and its
    // connection dies
    let ccfg = client_cfg("carol");
    let mut s = attach_ok(&ccfg, "alice", addr).await;
    let first = timeout(Duration::from_secs(3), s.deliveries.recv())
        .await
        .expect("a first delivery")
        .unwrap();
    s.conn.close(0u32.into(), b"gone");
    let mut got1 = vec![first];
    got1.extend(drain(&mut s).await);
    drop(s);
    sleep(Duration::from_millis(500)).await;
    // taken in position order, from the first
    let positions: Vec<u64> = got1.iter().map(|d| delivered(d).0).collect();
    let newest = *positions.last().unwrap();
    assert_eq!(positions, (1..=newest).collect::<Vec<_>>(), "in order");
    assert!(newest < 40, "the drain stopped where the connection died");
    // nothing was removed by the delivery
    assert_eq!(
        node.queued(&kh("carol")),
        40,
        "the store still holds all forty"
    );
    // C attaches again naming the newest position it took
    ccfg.cursors.lock().unwrap().insert(kh("alice"), newest);
    let got2 = {
        let mut s2 = attach_ok(&ccfg, "alice", addr).await;
        assert_eq!(s2.ack.queued, 40 - newest);
        drain(&mut s2).await
    };
    assert_eq!(
        got2.len() as u64,
        40 - newest,
        "the next session gets exactly what was never taken"
    );
    assert!(got2.iter().all(|d| delivered(d).0 > newest));
    let mut all = seals(&got1);
    all.extend(seals(&got2));
    assert!(same_set(&all, &sent), "nothing accepted was lost");
    assert_eq!(node.queued(&kh("carol")), 40);
}

// acceptance: QUE-19
#[tokio::test]
async fn a_message_for_an_attached_client_is_stored_and_delivered() {
    // the immediate path is the same path: accepted means stored, and the
    // store keeps it once taken, marked delivered
    let (node, addr, _ep) = spawn_node(node_cfg("alice"));
    let mut s = attach_ok(&client_cfg("carol"), "alice", addr).await;
    let sent = messages(3, "q19b");
    for m in &sent {
        node.enqueue(kh("carol"), m.clone(), 1).unwrap();
        assert!(
            node.queue_records(&kh("carol"))
                .iter()
                .any(|q| q.sealed == *m),
            "stored on acceptance"
        );
    }
    let got = seals(&drain(&mut s).await);
    assert!(same_set(&got, &sent));
    let recs = node.queue_records(&kh("carol"));
    assert_eq!(recs.len(), 3, "delivery removes nothing");
    assert!(
        recs.iter().all(|q| q.delivered_at.is_some()),
        "each marked delivered: {recs:?}"
    );
}

/// The memory store, every call passed through: what a test wraps to
/// change one behaviour.
macro_rules! pass_through {
    () => {
        fn next_position(&self, k: &[u8; 32]) -> u64 {
            self.0.next_position(k)
        }
        fn recipients(&self) -> Vec<[u8; 32]> {
            self.0.recipients()
        }
        fn mark_delivered(&self, k: &[u8; 32], position: u64, at: u64) -> bool {
            self.0.mark_delivered(k, position, at)
        }
        fn remove(&self, k: &[u8; 32], position: u64) -> bool {
            self.0.remove(k, position)
        }
        fn drop_all(&self, k: &[u8; 32]) {
            self.0.drop_all(k)
        }
    };
}

// acceptance: QUE-20
#[test]
fn two_submissions_at_once_cannot_both_take_the_room_for_one() {
    use rhtn_transport::queue::{MemoryStore, QueueStore, Queued};
    /// The memory store, slow to answer what it holds, so two submissions
    /// that were not serialised would both read it empty.
    struct Slow(MemoryStore);
    impl QueueStore for Slow {
        pass_through!();
        fn push(&self, q: Queued) -> bool {
            self.0.push(q)
        }
        fn list(&self, k: &[u8; 32]) -> Vec<Queued> {
            let held = self.0.list(k);
            std::thread::sleep(Duration::from_millis(60));
            held
        }
        fn block(&self) -> usize {
            4096
        }
    }
    let mut cfg = node_cfg("alice");
    cfg.queue_cap = Some(4096);
    cfg.queue = Arc::new(Slow(MemoryStore::default()));
    let node = Node::new(cfg);
    let small = |b: u8| opaque(&[b; 8]);
    let results = std::thread::scope(|s| {
        let a = s.spawn(|| node.enqueue(kh("carol"), small(1), 1));
        let b = s.spawn(|| node.enqueue(kh("carol"), small(2), 1));
        [a.join().unwrap(), b.join().unwrap()]
    });
    assert_eq!(
        results.iter().filter(|r| r.is_ok()).count(),
        1,
        "one fits, the other is refused: {results:?}"
    );
    assert!(results.contains(&Err(Refusal::AtCap)));
    assert_eq!(node.queued(&kh("carol")), 1);
}

// acceptance: QUE-22
#[test]
fn a_submission_the_store_will_not_take_is_refused_and_never_acknowledged() {
    use rhtn_transport::queue::{MemoryStore, QueueStore, Queued, Refusal};
    /// A store with no room on the disk: everything else works, and `push`
    /// says no. The on-disk store used to panic here, which took the whole
    /// node down with the disk, and a store that returned quietly would
    /// have lost mail the node had already answered `accepted` for.
    struct Full(MemoryStore);
    impl QueueStore for Full {
        pass_through!();
        fn push(&self, _q: Queued) -> bool {
            false
        }
        fn list(&self, k: &[u8; 32]) -> Vec<Queued> {
            self.0.list(k)
        }
    }
    let mut cfg = node_cfg("alice");
    cfg.queue = Arc::new(Full(MemoryStore::default()));
    let node = Node::new(cfg);
    assert_eq!(
        node.enqueue(kh("carol"), opaque(&[1]), 1),
        Err(Refusal::NotStored),
        "the answer says the node took it, so a node that could not store \
         it does not answer accepted (`wire-format.md` §7.10)"
    );
    // and nothing is left claiming to be queued
    assert_eq!(node.queued(&kh("carol")), 0);
}

/// A store that costs a block of `B` per item, as a directory store does.
struct Blocky(rhtn_transport::queue::MemoryStore, usize);
impl rhtn_transport::queue::QueueStore for Blocky {
    pass_through!();
    fn push(&self, item: Queued) -> bool {
        self.0.push(item)
    }
    fn list(&self, k: &[u8; 32]) -> Vec<Queued> {
        self.0.list(k)
    }
    fn block(&self) -> usize {
        self.1
    }
}

fn record(who: Keyhash, position: u64, sealed: Vec<u8>) -> Queued {
    Queued {
        sealed,
        recipient: who,
        n: 1,
        position,
        arrival: position,
        delivered_at: None,
    }
}

/// **The cap means the disk a queue consumes, not the bytes it holds**
/// [author, 2026-11-06] (design §14.1.6; `infra-client-requirements.md`
/// §2).
///
/// A store keeping a file per item consumes a block per item, so a cap
/// read against content promised an operator a fraction of the disk the
/// queue actually took: the bench node held 3.1 MB of content in 5.2 MB of
/// blocks. `QueueStore::occupied` rounds each item to the store's block, so
/// the number an operator configures is the number of bytes of disk they
/// are lending.
#[test]
fn the_cap_counts_the_disk_a_queue_occupies_and_not_its_content() {
    use rhtn_transport::queue::{MemoryStore, QueueStore};
    let store = Blocky(MemoryStore::default(), 4096);
    let who = kh("carol");
    // **three small messages cost three blocks, not three hundred bytes**
    for n in 0..3u8 {
        assert!(store.push(record(who, u64::from(n) + 1, vec![n; 100])));
    }
    assert_eq!(
        store.occupied(&who),
        3 * 4096,
        "a hundred bytes in a block costs a block"
    );
    // and a store that holds nothing on a disk rounds nothing, because
    // there is no disk for the cap to be about
    let plain = MemoryStore::default();
    for n in 0..3u8 {
        plain.push(record(who, u64::from(n) + 1, vec![n; 100]));
    }
    assert_eq!(plain.occupied(&who), 300, "in memory, content is the cost");

    // **an item larger than a block rounds up and not down**: two blocks
    // for anything past the first, which is what a sender can push against
    let big = Blocky(MemoryStore::default(), 4096);
    big.push(record(who, 1, vec![0; 4097]));
    assert_eq!(big.occupied(&who), 2 * 4096);
}

/// A sealed message whose encoding is exactly `len` bytes.
fn sealed_of_len(len: usize, fill: u8) -> Vec<u8> {
    let overhead = opaque(&[]).len();
    (len.saturating_sub(overhead + 4)..len)
        .map(|b| opaque(&vec![fill; b]))
        .find(|s| s.len() == len)
        .expect("a body length that encodes to the size")
}

// acceptance: QUE-25
#[test]
fn each_message_the_incoming_included_counts_as_the_blocks_it_occupies() {
    // B is the store's block, and the cap two of them; a sealed message is
    // at least two X-Wing ciphertexts, so "small" here is a few kilobytes
    // and B is sized well above it
    const B: usize = 16 * 1024;
    let mut cfg = node_cfg("alice");
    cfg.queue_cap = Some(2 * B);
    cfg.queue = Arc::new(Blocky(Default::default(), B));
    let node = Node::new(cfg);
    let small: Vec<Vec<u8>> = (0..3).map(|i| opaque(&[i; 200])).collect();
    assert!(small.iter().all(|m| m.len() < B / 4));
    assert!(node.enqueue(kh("carol"), small[0].clone(), 1).is_ok());
    assert!(node.enqueue(kh("carol"), small[1].clone(), 1).is_ok());
    let content: usize = small.iter().map(Vec::len).sum();
    assert!(
        content < B,
        "three hold well under 2B of content: {content}"
    );
    assert_eq!(
        node.enqueue(kh("carol"), small[2].clone(), 1),
        Err(Refusal::AtCap),
        "the third is a third block, over a cap of two"
    );
    assert_eq!(node.queued(&kh("carol")), 2);

    // separately, with C's queue empty: one of B + 1 bytes is two blocks
    node.cfg.queue.drop_all(&kh("carol"));
    assert_eq!(node.queued(&kh("carol")), 0);
    let over_one = sealed_of_len(B + 1, 7);
    assert_eq!(over_one.len(), B + 1);
    assert!(
        node.enqueue(kh("carol"), over_one, 1).is_ok(),
        "two blocks fit a cap of two"
    );
    assert_eq!(
        node.enqueue(kh("carol"), small[0].clone(), 1),
        Err(Refusal::AtCap),
        "and the small one after it is a third block, refused"
    );
}

// acceptance: QUE-24
#[tokio::test]
async fn at_the_cap_the_oldest_delivered_goes_before_anything_is_refused() {
    let k = 5;
    let msgs = messages(k, "q24");
    let mut cfg = node_cfg("alice");
    cfg.queue_cap = Some(msgs.iter().map(Vec::len).sum());
    let removed: Arc<Mutex<Vec<[u8; 32]>>> = Arc::default();
    let r = removed.clone();
    cfg.on_removed = Some(Arc::new(move |who, _n| r.lock().unwrap().push(*who)));
    let (node, addr, _ep) = spawn_node(cfg);
    // the oldest two are queued and delivered to a device of C
    node.enqueue(kh("carol"), msgs[0].clone(), 1).unwrap();
    node.enqueue(kh("carol"), msgs[1].clone(), 1).unwrap();
    {
        let mut s = attach_ok(&client_cfg("carol"), "alice", addr).await;
        assert_eq!(drain(&mut s).await.len(), 2);
        s.conn.close(0u32.into(), b"");
    }
    sleep(Duration::from_millis(200)).await;
    for m in &msgs[2..] {
        node.enqueue(kh("carol"), m.clone(), 1).unwrap();
    }
    let delivered_positions = |node: &Node| -> Vec<u64> {
        node.queue_records(&kh("carol"))
            .iter()
            .filter(|q| q.delivered_at.is_some())
            .map(|q| q.position)
            .collect()
    };
    assert_eq!(
        delivered_positions(&node),
        vec![1, 2],
        "k held, two delivered"
    );
    assert_eq!(node.queued(&kh("carol")), k);
    // X submits two more: each accepted, the oldest delivered going first
    let extra = messages(3, "x24");
    assert!(node.enqueue(kh("carol"), extra[0].clone(), 1).is_ok());
    assert_eq!(delivered_positions(&node), vec![2], "position 1 went first");
    assert!(node.enqueue(kh("carol"), extra[1].clone(), 1).is_ok());
    assert!(delivered_positions(&node).is_empty(), "then position 2");
    assert_eq!(
        *removed.lock().unwrap(),
        vec![kh("carol"); 2],
        "two removals, each told to the key holder"
    );
    assert_eq!(node.queued(&kh("carol")), k);
    // the third: everything held is undelivered, so it is refused
    assert_eq!(
        node.enqueue(kh("carol"), extra[2].clone(), 1),
        Err(Refusal::AtCap)
    );
    // a device attaching with no field 5 is handed the k now held, every
    // undelivered one among them
    let mut s = attach_ok(&client_cfg("carol"), "alice", addr).await;
    assert_eq!(s.ack.queued, k as u64);
    let got = seals(&drain(&mut s).await);
    let mut expected: Vec<Vec<u8>> = msgs[2..].to_vec();
    expected.extend_from_slice(&extra[..2]);
    assert!(same_set(&got, &expected), "the k held, none of the removed");
}

/// **A connection under a key minted for it may ask for a node key or
/// submit, and nothing else** (`wire-format.md` §9.1): the node serves it
/// as nobody, and any other request fails its stream.
#[tokio::test]
async fn a_minted_connection_may_fetch_a_node_key_or_submit_and_nothing_else() {
    use rhtn_archive::node_key::NodeKeyRequest;
    use rhtn_archive::submission::RelaySubmission;
    use rhtn_codec::schema::Family;
    type Seen = Arc<Mutex<Vec<([u8; 32], [u8; 32], Family)>>>;
    let seen: Seen = Arc::default();
    let mut cfg = node_cfg("alice");
    let sink = seen.clone();
    cfg.on_request = Some(Arc::new(move |peer, device, family, _body| {
        sink.lock().unwrap().push((peer, device, family));
        Box::pin(async { Some(b"answered".to_vec()) })
    }));
    let (node, addr, _ep) = spawn_node(cfg);
    let ccfg = client_cfg("bob");
    let ep = client_ep();
    let ask = NodeKeyRequest::One {
        recipient: kh("carol"),
        nonce: [1; 16],
    }
    .encode();
    let submit = RelaySubmission {
        recipient: kh("carol"),
        sealed: opaque(b"for carol"),
        nonce: [2; 16],
    }
    .encode();
    let conn = connect_minted(&ccfg, &ep, kh("alice"), addr).await.unwrap();
    assert_eq!(request_on(&conn, 3, &ask).await.unwrap(), b"answered");
    assert_eq!(request_on(&conn, 11, &submit).await.unwrap(), b"answered");
    // a request of any other family fails its stream, unanswered
    let wake = rhtn_archive::submission::WakeRegistration::withdrawal([3; 16]).encode();
    assert!(
        request_on(&conn, 12, &wake).await.is_err(),
        "no wake registration"
    );
    {
        let seen = seen.lock().unwrap();
        assert_eq!(seen.len(), 2, "{seen:?}");
        assert!(
            seen.iter().all(|(peer, _, _)| *peer == MINTED),
            "served as nobody"
        );
        assert_eq!(seen[0].2, Family::NodeKeyRequestOrBatch);
        assert_eq!(seen[1].2, Family::RelaySubmission);
        assert_ne!(seen[0].1, test_identity("bob").public.ed.to_bytes());
    }
    // a second minted connection presents a different key: nothing ties
    // the two asks together
    let again = connect_minted(&ccfg, &ep, kh("alice"), addr).await.unwrap();
    request_on(&again, 3, &ask).await.unwrap();
    {
        let seen = seen.lock().unwrap();
        assert_eq!(seen.len(), 3);
        assert_ne!(seen[2].1, seen[0].1, "a fresh key per connection");
    }
    // and a minted connection whose first request is anything else is
    // not served at all
    let other = connect_minted(&ccfg, &ep, kh("alice"), addr).await.unwrap();
    assert!(request_on(&other, 12, &wake).await.is_err());
    assert_eq!(seen.lock().unwrap().len(), 3, "the handler saw nothing");
    assert!(!node.has_session(&MINTED));
    assert!(!node.has_session(&kh("bob")));
}

/// **A delegated device's minted connection presents no delegation**
/// (`wire-format.md` §9.1): its own names the device's key and not the
/// minted one, and the node, reading it first, would refuse the
/// connection as unbound — a paired desktop's node-key fetch, refused so
/// [bench, 2026-11-23].
#[tokio::test]
async fn a_delegated_devices_minted_connection_presents_no_delegation() {
    use rhtn_archive::node_key::NodeKeyRequest;
    let mut cfg = node_cfg("alice");
    cfg.on_request = Some(Arc::new(|_, _, _, _| {
        Box::pin(async { Some(b"answered".to_vec()) })
    }));
    let (_node, addr, _ep) = spawn_node(cfg);
    let bob = test_identity("bob");
    let desk = tls::Credential::from_seed(&[51; 32], kh("bob"));
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();
    desk.add(
        std::slice::from_ref(&bob.public),
        &rhtn_crypto::delegation::issue(&bob, &desk.public(), now - 60),
    )
    .unwrap();
    let ccfg = client_cfg("bob").with_credential(Arc::new(desk));
    let ask = NodeKeyRequest::One {
        recipient: kh("carol"),
        nonce: [4; 16],
    }
    .encode();
    let conn = connect_minted(&ccfg, &client_ep(), kh("alice"), addr)
        .await
        .expect("the minted connection is bound");
    assert_eq!(request_on(&conn, 3, &ask).await.unwrap(), b"answered");
    assert!(
        !ccfg
            .log
            .events()
            .iter()
            .any(|(_, e)| matches!(e, Event::DelegationPresented)),
        "no delegation went out under the minted key"
    );
}
