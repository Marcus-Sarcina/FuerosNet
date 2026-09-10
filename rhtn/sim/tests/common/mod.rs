//! Loopback nodes and clients for the session, queue and replication
//! entries, with the datagram path between them where a test needs one.

#![allow(dead_code)]

use rhtn_crypto::identity::testkit::test_identity;
use rhtn_crypto::SigningIdentity;
use rhtn_transport::session::*;
use rhtn_transport::tls::{self, Pins};
use std::collections::BTreeMap;
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};

pub const NAMES: [&str; 8] = ["alice", "bob", "carol", "alice2", "w1", "w2", "c1", "c2"];

pub fn id(n: &str) -> SigningIdentity {
    test_identity(n)
}

pub fn kh(n: &str) -> [u8; 32] {
    test_identity(n).public.keyhash
}

pub fn pins() -> Pins {
    let p = Pins::new();
    for n in NAMES {
        p.pin_identity(&test_identity(n).public);
    }
    p
}

pub fn loopback() -> SocketAddr {
    "127.0.0.1:0".parse().unwrap()
}

pub fn node_cfg(name: &str, interval: u64) -> NodeConfig {
    NodeConfig::defaults(Arc::new(id(name)), pins(), interval)
}

pub fn client_cfg(name: &str) -> ClientConfig {
    ClientConfig {
        identity: Arc::new(id(name)),
        pins: pins(),
        capabilities: BTreeMap::new(),
        attestation: None,
        filter: None,
        sibling_cache: Arc::new(Mutex::new(Vec::new())),
        addresses: Arc::new(Mutex::new(Default::default())),
        tls: Arc::new(Mutex::new(Default::default())),
        connect_timeout: std::time::Duration::from_millis(1500),
    }
}

pub fn client_ep() -> quinn::Endpoint {
    tls::client_endpoint(loopback()).unwrap()
}

/// A `SiblingRef` for `name` at `addr`, carrying its key material so a
/// client that has never contacted it can authenticate it (§8.2).
pub fn sibling_ref(name: &str, addr: SocketAddr) -> SiblingRef {
    let std::net::IpAddr::V4(v4) = addr.ip() else { panic!("v4") };
    SiblingRef {
        keyhash: kh(name),
        endpoints: vec![NetworkPoint { ip: v4.octets(), asn: None, port: Some(addr.port() as u64) }],
        key_material: Some(id(name).public.key_material()),
    }
}

/// Note an address for a keyhash in the client's own address book.
pub fn know(cfg: &ClientConfig, name: &str, addr: SocketAddr) {
    cfg.addresses.lock().unwrap().entry(kh(name)).or_default().push(addr);
}

/// A filter that drops every heartbeat, so a node can fall silent on the
/// detector's path while still sending everything else.
pub fn drop_heartbeats() -> OutboundFilter {
    Arc::new(|frame_type, bytes| if frame_type == FRAME_HEARTBEAT { None } else { Some(bytes.to_vec()) })
}

/// Every delivery that arrives within `ms`.
pub async fn drain(s: &mut Session, ms: u64) -> Vec<Vec<u8>> {
    let mut got = Vec::new();
    while let Ok(Some(b)) = tokio::time::timeout(std::time::Duration::from_millis(ms), s.deliveries.recv()).await {
        got.push(b);
    }
    got
}

pub fn messages(k: usize, tag: &str) -> Vec<Vec<u8>> {
    (0..k).map(|i| format!("{tag}:{i}").into_bytes()).collect()
}
