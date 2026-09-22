//! Transport acceptance entries that need only the handshake: TRN-01, TRN-02
//! and TRN-05.  TRN-03 is in `delegation.rs`, since the bind it refuses is
//! made after the handshake (`wire-format.md` §9.1).  Two real QUIC endpoints on loopback.
//!
//! On observation: TRN-01 and TRN-02 describe reading the ClientHello.  Here
//! the same facts are established from the outcomes: a peer configured with a
//! single group can complete a handshake only with a peer offering it, so a
//! completed handshake against the profile's server proves the client offered
//! `X25519MLKEM768`, and a failed one against a classical-only test peer, in
//! each role, proves nothing weaker was offered or accepted.

use rhtn_crypto::identity::testkit::test_identity;
use rhtn_transport::tls::{self, Pins};
use rustls::crypto::aws_lc_rs::kx_group;
use std::net::SocketAddr;

fn loopback() -> SocketAddr {
    "127.0.0.1:0".parse().unwrap()
}

async fn accept_once(ep: &quinn::Endpoint) -> Result<quinn::Connection, quinn::ConnectionError> {
    ep.accept().await.expect("an incoming connection").await
}

// acceptance: TRN-05
#[tokio::test]
async fn trn_05_negotiates_rhtn_1_and_mutual_raw_public_keys() {
    let server = test_identity("bob");
    let client = test_identity("alice");
    let pins = Pins::new();
    pins.pin_identity(&server.public);
    let sep = tls::server_endpoint(&server, loopback()).unwrap();
    let cep = tls::client_endpoint(loopback()).unwrap();
    let addr = sep.local_addr().unwrap();
    let (s, c) = tokio::join!(accept_once(&sep), async {
        tls::dial(&cep, &client, &pins, &server.public.keyhash, addr)
            .unwrap()
            .await
    });
    let (s, c) = (
        s.expect("server side completes"),
        c.expect("client side completes"),
    );
    assert_eq!(tls::negotiated_alpn(&c).as_deref(), Some(tls::ALPN));
    assert_eq!(tls::negotiated_alpn(&s).as_deref(), Some(tls::ALPN));
    // the server authenticated the client's classical component, mutually
    assert_eq!(
        tls::peer_spki(&s).unwrap(),
        tls::spki_der(&client.public.ed)
    );
    assert_eq!(
        tls::peer_spki(&c).unwrap(),
        tls::spki_der(&server.public.ed)
    );
    assert_eq!(
        pins.keyhash_for_spki(&tls::peer_spki(&c).unwrap()),
        Some(server.public.keyhash)
    );
}

// acceptance: TRN-01
#[tokio::test]
async fn trn_01_offers_only_x25519mlkem768() {
    let server = test_identity("bob");
    let client = test_identity("alice");
    let pins = Pins::new();
    pins.pin_identity(&server.public);
    // a test peer that accepts only classical groups completes nothing with the profile's client
    let sep = tls::server_endpoint_with(
        tls::server_config_with(&server, vec![kx_group::X25519, kx_group::SECP256R1]),
        loopback(),
    )
    .unwrap();
    let cep = tls::client_endpoint(loopback()).unwrap();
    let addr = sep.local_addr().unwrap();
    let (s, c) = tokio::join!(accept_once(&sep), async {
        tls::dial(&cep, &client, &pins, &server.public.keyhash, addr)
            .unwrap()
            .await
    });
    assert!(
        c.is_err(),
        "the profile's client must not complete a handshake on a classical group"
    );
    assert!(s.is_err());
    // and with the profile's group on both sides the handshake completes
    let sep = tls::server_endpoint(&server, loopback()).unwrap();
    let addr = sep.local_addr().unwrap();
    let (s, c) = tokio::join!(accept_once(&sep), async {
        tls::dial(&cep, &client, &pins, &server.public.keyhash, addr)
            .unwrap()
            .await
    });
    assert!(s.is_ok() && c.is_ok());
}

// acceptance: TRN-02
#[tokio::test]
async fn trn_02_completes_no_handshake_with_a_classical_only_client() {
    let server = test_identity("bob");
    let client = test_identity("alice");
    let sep = tls::server_endpoint(&server, loopback()).unwrap();
    let cep = tls::client_endpoint(loopback()).unwrap();
    let addr = sep.local_addr().unwrap();
    let classical = tls::client_config_with(&client, vec![kx_group::X25519, kx_group::SECP256R1]);
    let (s, c) = tokio::join!(accept_once(&sep), async {
        tls::dial_with(&cep, classical, addr).unwrap().await
    });
    assert!(
        c.is_err(),
        "a client offering only X25519 and secp256r1 must be refused"
    );
    assert!(s.is_err(), "no session is established on the serving node");
}
