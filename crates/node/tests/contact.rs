//! The two resolution entries that need a real handshake: what a wrong
//! address costs, and what a second endpoint is for.

mod common;

use common::*;
use rhtn_node::resolution::*;
use rhtn_transport::session::ClientConfig;
use rhtn_transport::tls::{self, Party, Pins};
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};
use std::time::Duration;

fn loopback() -> SocketAddr {
    "127.0.0.1:0".parse().unwrap()
}

fn pins_for(names: &[&str]) -> Pins {
    let p = Pins::new();
    for n in names {
        p.pin_identity(&id(n).public);
    }
    p
}

/// The requester's configuration: `me`, holding `pins`, waiting
/// `per_endpoint` on each address.
fn requester(me: &str, pins: Pins, per_endpoint: Duration) -> ClientConfig {
    ClientConfig {
        me: Party::of(Arc::new(id(me))),
        pins,
        bind: Default::default(),
        capabilities: Default::default(),
        attestation: None,
        filter: None,
        sibling_cache: Arc::new(Mutex::new(Vec::new())),
        addresses: Arc::new(Mutex::new(Default::default())),
        tls: Arc::new(Mutex::new(Default::default())),
        connect_timeout: per_endpoint,
        on_reachability: None,
        log: rhtn_transport::session::Log::recording(),
    }
}

/// A server that answers as `name`, and the address it listens on.
fn serve(name: &str) -> (quinn::Endpoint, SocketAddr) {
    let ep = tls::server_endpoint(id(name), loopback()).unwrap();
    let addr = ep.local_addr().unwrap();
    let listening = ep.clone();
    tokio::spawn(async move {
        while let Some(inc) = listening.accept().await {
            tokio::spawn(async move {
                let _ = inc.await;
            });
        }
    });
    (ep, addr)
}

fn as_point(addr: SocketAddr) -> NetworkPoint {
    let std::net::IpAddr::V4(v4) = addr.ip() else {
        panic!("v4")
    };
    NetworkPoint::new(v4.octets(), Some(addr.port() as u64))
}

// acceptance: RES-12
#[tokio::test]
async fn a_wrong_address_fails_the_bind_and_never_names_the_subject() {
    // the requester means to reach S (bob) and holds S's KeyMaterial
    let (_y_ep, y_addr) = serve("carol"); // Y answers here, with Y's own key
    let pins = pins_for(&["bob"]);
    assert!(
        pins.classical_key(&kh("carol")).is_none(),
        "Y is not pinned"
    );
    // a reply names S's keyhash but lists Y's endpoint
    let reply = ResolveReply::Serving {
        nonce: [12; 16],
        serving: ServingInfra {
            node: kh("bob"),
            endpoints: vec![as_point(y_addr)],
            residual: Path::empty(),
            key_material: None,
        },
    };
    let ResolveReply::Serving { serving, .. } = &reply else {
        panic!()
    };
    let client = tls::client_endpoint(loopback()).unwrap();
    let cfg = requester("alice", pins, Duration::from_secs(2));
    let out = contact(&serving.endpoints, &client, &cfg, &serving.node).await;
    match out {
        Contact::Failed { target, attempts } => {
            assert_eq!(target, kh("bob"), "reported as a failed contact with S");
            assert_eq!(attempts.len(), 1);
            // the handshake completed under Y's key; nothing bound it to S:
            // not the pin, nothing held, and no delegation frame arrived
            assert!(
                attempts[0].why.starts_with("unbound"),
                "refused as unbound, got {}",
                attempts[0].why
            );
        }
        Contact::Reached(_) => {
            panic!("the presented key is not the classical member of S's pinned KeyMaterial")
        }
    }
    // nothing was sent on the connection: the requester's log shows no
    // frame out, and no request
    assert!(
        !cfg.log
            .events()
            .iter()
            .any(|(_, e)| matches!(e, rhtn_transport::session::Event::Sent { .. })),
        "nothing is sent to a peer nothing binds"
    );
}

// acceptance: RES-13
#[tokio::test]
async fn the_second_endpoint_is_tried_when_the_first_does_not_answer() {
    let (_s_ep, live) = serve("bob");
    // a dead port, first in the publisher's preference order
    let dead = {
        let sock = std::net::UdpSocket::bind("127.0.0.1:0").unwrap();
        let a = sock.local_addr().unwrap();
        drop(sock);
        a
    };
    let endpoints = vec![as_point(dead), as_point(live)];
    let pins = pins_for(&["bob"]);
    let client = tls::client_endpoint(loopback()).unwrap();
    let cfg = requester("alice", pins.clone(), Duration::from_millis(700));
    let out = contact(&endpoints, &client, &cfg, &kh("bob")).await;
    match out {
        Contact::Reached(conn) => {
            assert_eq!(tls::negotiated_alpn(&conn).as_deref(), Some(&b"rhtn/1"[..]));
            let spki = tls::peer_spki(&conn).expect("peer key");
            assert_eq!(
                pins.keyhash_for_spki(&spki),
                Some(kh("bob")),
                "the handshake completes against S's pinned key"
            );
        }
        Contact::Failed { attempts, .. } => {
            panic!("a single unreachable first entry became a permanent outage: {attempts:?}")
        }
    }
}
