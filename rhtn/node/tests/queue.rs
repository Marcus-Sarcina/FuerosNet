//! The node's mailbox on disk (QUE-04): what a restart leaves behind.

use rhtn_archive::Keyhash;
use rhtn_crypto::identity::testkit::test_identity;
use rhtn_node::queue::DirStore;
use rhtn_transport::session::*;
use rhtn_transport::tls::{self, Pins};
use std::collections::BTreeMap;
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};
use tokio::time::{Duration, timeout};

const NAMES: [&str; 3] = ["alice", "bob", "carol"];

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
        connect_timeout: Duration::from_millis(1500),
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

// acceptance: QUE-04
#[tokio::test]
async fn no_crash_recovery_copy_outlives_a_delivery() {
    let dir = std::env::temp_dir().join(format!("rhtn-queue-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let store = Arc::new(DirStore::new(&dir));
    let mut cfg = node_cfg("alice");
    cfg.queue = store.clone();
    let (node, addr, ep) = spawn_node(cfg);
    let sent = messages(3, "q4");
    for m in &sent {
        node.enqueue(kh("carol"), m.clone()).unwrap();
    }
    assert_eq!(store.all_files().len(), 3, "persisted while waiting");
    let mut s = attach_ok(&client_cfg("carol"), "alice", addr).await;
    assert_eq!(drain(&mut s).await.len(), 3);
    s.conn.close(0u32.into(), b"");
    drop(s);
    // the process dies without warning: the node and its endpoint go away,
    // and a new node starts from whatever the directory holds
    ep.close(0u32.into(), b"killed");
    drop(node);
    let mut cfg2 = node_cfg("alice");
    cfg2.queue = Arc::new(DirStore::new(&dir));
    let (node2, addr2, _ep2) = spawn_node(cfg2);
    let mut s2 = attach_ok(&client_cfg("carol"), "alice", addr2).await;
    assert_eq!(s2.ack.queued, 0);
    assert!(drain(&mut s2).await.is_empty());
    assert!(node2.queue_records(&kh("carol")).is_empty());
    let mut leftover = Vec::new();
    for entry in walkdir(&dir) {
        leftover.push(entry);
    }
    assert!(leftover.is_empty(), "none of the ciphertexts is present after the restart: {leftover:?}");
    let _ = std::fs::remove_dir_all(&dir);
}

fn walkdir(dir: &std::path::Path) -> Vec<std::path::PathBuf> {
    let mut out = Vec::new();
    if let Ok(rd) = std::fs::read_dir(dir) {
        for e in rd.flatten() {
            if e.path().is_dir() {
                out.extend(walkdir(&e.path()));
            } else {
                out.push(e.path());
            }
        }
    }
    out
}

