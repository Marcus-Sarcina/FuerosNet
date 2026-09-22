//! Two phones behind two NATs (design §14.1.1): what a STUN server at the
//! serving node tells each, and whether a QUIC connection between them can
//! be punched through.

use rhtn_crypto::identity::testkit::test_identity;
use rhtn_sim::nat::{Filtering, Mapping, Nat};
use rhtn_transport::tls;
use rhtn_transport::traversal::{TraversalSocket, endpoint};
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

fn server_config(name: &str) -> quinn::ServerConfig {
    let crypto =
        quinn::crypto::rustls::QuicServerConfig::try_from(tls::server_config(test_identity(name)))
            .unwrap();
    quinn::ServerConfig::with_crypto(Arc::new(crypto))
}

/// A STUN server on the far side of the NATs: a serving node's socket.
async fn stun_server() -> (Arc<TraversalSocket>, SocketAddr, quinn::Endpoint) {
    let s = TraversalSocket::bind("127.0.0.1:0".parse().unwrap(), None).unwrap();
    let addr = s.addr().unwrap();
    let ep = endpoint(s.clone(), Some(server_config("w1"))).unwrap();
    (s, addr, ep)
}

#[tokio::test]
async fn behind_an_endpoint_independent_nat_the_reflexive_address_is_one_mapping_for_every_destination()
 {
    let nat = Nat::start(Mapping::EndpointIndependent, Filtering::AddressDependent)
        .await
        .unwrap();
    let (_s1, stun1, _e1) = stun_server().await;
    let (_s2, stun2, _e2) = stun_server().await;
    let client = TraversalSocket::bind("127.0.0.1:0".parse().unwrap(), Some(nat.inside)).unwrap();
    let _ep = endpoint(client.clone(), None).unwrap();
    let a = client
        .reflexive(stun1, Duration::from_secs(2))
        .await
        .unwrap();
    let b = client
        .reflexive(stun2, Duration::from_secs(2))
        .await
        .unwrap();
    assert_eq!(a, b, "the same external port whichever server asks");
    assert_ne!(a, client.addr().unwrap(), "and not the inside address");
    assert_eq!(nat.mappings().len(), 1);
    assert!(nat.mappings().contains(&a));
}

#[tokio::test]
async fn behind_an_address_and_port_dependent_nat_each_destination_sees_a_different_mapping() {
    let nat = Nat::start(
        Mapping::AddressAndPortDependent,
        Filtering::AddressAndPortDependent,
    )
    .await
    .unwrap();
    let (_s1, stun1, _e1) = stun_server().await;
    let (_s2, stun2, _e2) = stun_server().await;
    let client = TraversalSocket::bind("127.0.0.1:0".parse().unwrap(), Some(nat.inside)).unwrap();
    let _ep = endpoint(client.clone(), None).unwrap();
    let a = client
        .reflexive(stun1, Duration::from_secs(2))
        .await
        .unwrap();
    let b = client
        .reflexive(stun2, Duration::from_secs(2))
        .await
        .unwrap();
    assert_ne!(
        a, b,
        "what one server reports is not what another would reach"
    );
    assert_eq!(nat.mappings().len(), 2);
}

/// Two clients behind NATs of the given behaviour, each having learned its
/// reflexive address from the STUN server, dial each other's reflexive
/// address at once; whether either handshake completes.
async fn punch(mapping: Mapping, filtering: Filtering) -> bool {
    let (nat_a, nat_b) = (
        Nat::start(mapping, filtering).await.unwrap(),
        Nat::start(mapping, filtering).await.unwrap(),
    );
    let (_s, stun, _e) = stun_server().await;
    let (ida, idb) = (test_identity("alice"), test_identity("bob"));
    let sa = TraversalSocket::bind("127.0.0.1:0".parse().unwrap(), Some(nat_a.inside)).unwrap();
    let sb = TraversalSocket::bind("127.0.0.1:0".parse().unwrap(), Some(nat_b.inside)).unwrap();
    // each endpoint both accepts and dials: a hole punched from either side
    let ea = endpoint(sa.clone(), Some(server_config("alice"))).unwrap();
    let eb = endpoint(sb.clone(), Some(server_config("bob"))).unwrap();
    for ep in [ea.clone(), eb.clone()] {
        tokio::spawn(async move {
            while let Some(incoming) = ep.accept().await {
                tokio::spawn(async move {
                    let _ = incoming.await;
                });
            }
        });
    }
    let ra = sa.reflexive(stun, Duration::from_secs(2)).await.unwrap();
    let rb = sb.reflexive(stun, Duration::from_secs(2)).await.unwrap();
    let pins = tls::Pins::new();
    pins.pin_identity(&ida.public);
    pins.pin_identity(&idb.public);
    let dial_a = tls::dial(&ea, &ida, &pins, &idb.public.keyhash, rb).unwrap();
    let dial_b = tls::dial(&eb, &idb, &pins, &ida.public.keyhash, ra).unwrap();
    let t = Duration::from_millis(1500);
    let (a, b) = tokio::join!(
        tokio::time::timeout(t, dial_a),
        tokio::time::timeout(t, dial_b)
    );
    matches!(a, Ok(Ok(_))) || matches!(b, Ok(Ok(_)))
}

#[tokio::test]
async fn hole_punching_succeeds_through_endpoint_independent_nats() {
    assert!(
        punch(Mapping::EndpointIndependent, Filtering::AddressDependent).await,
        "the reflexive addresses are reachable and each side's first packet opens the other's filter"
    );
}

#[tokio::test]
async fn hole_punching_fails_through_address_and_port_dependent_nats() {
    assert!(
        !punch(
            Mapping::AddressAndPortDependent,
            Filtering::AddressAndPortDependent
        )
        .await,
        "the mapping the STUN server saw is not the one a peer reaches"
    );
}
