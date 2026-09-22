//! STUN Binding to the byte (RFC 8489), and the traversal socket that
//! answers and asks it beside QUIC.

use rhtn_transport::stun::*;
use rhtn_transport::traversal::{TraversalSocket, endpoint, unwrap, wrap};
use std::net::SocketAddr;
use std::time::Duration;

/// RFC 5769 §2.2's sample response: transaction id b7e7a701bc34d686fa87dfae,
/// XOR-MAPPED-ADDRESS 192.0.2.1:32853.
#[test]
fn the_sample_xor_mapped_address_decodes_and_re_encodes() {
    let txid: [u8; 12] = [
        0xb7, 0xe7, 0xa7, 0x01, 0xbc, 0x34, 0xd6, 0x86, 0xfa, 0x87, 0xdf, 0xae,
    ];
    let want: SocketAddr = "192.0.2.1:32853".parse().unwrap();
    let resp = binding_response(&txid, want);
    // the attribute as the RFC prints it: 00 20 00 08 00 01 a1 47 e1 12 a6 43
    let attr = &resp[HEADER_BYTES..HEADER_BYTES + 12];
    assert_eq!(
        attr,
        &[
            0x00, 0x20, 0x00, 0x08, 0x00, 0x01, 0xa1, 0x47, 0xe1, 0x12, 0xa6, 0x43
        ]
    );
    let p = parse(&resp).expect("parses");
    assert_eq!(
        (p.kind, p.txid, p.mapped),
        (Binding::Response, txid, Some(want))
    );
    // the magic cookie and the length are where the RFC puts them
    assert_eq!(&resp[4..8], &MAGIC_COOKIE.to_be_bytes());
    assert_eq!(
        u16::from_be_bytes([resp[2], resp[3]]) as usize,
        resp.len() - HEADER_BYTES
    );
    // an IPv6 mapping round-trips too
    let six: SocketAddr = "[2001:db8::1]:4242".parse().unwrap();
    assert_eq!(
        parse(&binding_response(&txid, six)).unwrap().mapped,
        Some(six)
    );
}

#[test]
fn a_request_is_stun_a_quic_packet_is_not_and_a_bad_fingerprint_is_refused() {
    let txid = [7u8; 12];
    let req = binding_request(&txid);
    assert!(is_stun(&req));
    assert_eq!(
        parse(&req).unwrap(),
        Parsed {
            kind: Binding::Request,
            txid,
            mapped: None
        }
    );
    // QUIC: a long header starts with the high bit set, a short header with 0x40 set
    let mut quic_long = vec![0xC3u8; 40];
    quic_long[4..8].copy_from_slice(&MAGIC_COOKIE.to_be_bytes());
    assert!(!is_stun(&quic_long));
    let quic_short = vec![0x41u8; 40];
    assert!(!is_stun(&quic_short));
    // a flipped fingerprint byte: not a message
    let mut bad = req.clone();
    let n = bad.len();
    bad[n - 1] ^= 1;
    assert!(is_stun(&bad) && parse(&bad).is_none());
    // a length that disagrees with the datagram is not STUN at all
    let mut long = req.clone();
    long.push(0);
    assert!(!is_stun(&long));
    // the harness framing round-trips
    let a: SocketAddr = "127.0.0.1:7431".parse().unwrap();
    assert_eq!(unwrap(&wrap(a, b"payload")), Some((a, &b"payload"[..])));
}

#[tokio::test]
async fn a_serving_node_answers_stun_at_the_address_it_serves_quic_on_and_quic_still_passes() {
    use rhtn_crypto::identity::testkit::test_identity;
    use rhtn_transport::tls;
    let node = test_identity("alice");
    let server_sock = TraversalSocket::bind("127.0.0.1:0".parse().unwrap(), None).unwrap();
    let server_addr = server_sock.addr().unwrap();
    let server_ep = endpoint(
        server_sock.clone(),
        Some({
            let crypto =
                quinn::crypto::rustls::QuicServerConfig::try_from(tls::server_config(&node))
                    .unwrap();
            quinn::ServerConfig::with_crypto(std::sync::Arc::new(crypto))
        }),
    )
    .unwrap();
    let client_sock = TraversalSocket::bind("127.0.0.1:0".parse().unwrap(), None).unwrap();
    let client_ep = endpoint(client_sock.clone(), None).unwrap();
    // the reflexive address on loopback is the socket's own
    let seen = client_sock
        .reflexive(server_addr, Duration::from_secs(2))
        .await
        .unwrap();
    assert_eq!(seen, client_sock.addr().unwrap());
    assert_eq!(
        server_sock
            .answered
            .load(std::sync::atomic::Ordering::SeqCst),
        1
    );
    assert_eq!(
        client_sock.asked.load(std::sync::atomic::Ordering::SeqCst),
        1
    );
    // and a QUIC connection on the same two sockets still completes
    let me = test_identity("bob");
    let pins = tls::Pins::new();
    pins.pin_identity(&node.public);
    let accept = tokio::spawn(async move { server_ep.accept().await.unwrap().await.unwrap() });
    let conn = tls::dial(&client_ep, &me, &pins, &node.public.keyhash, server_addr)
        .unwrap()
        .await
        .expect("handshake over the traversal sockets");
    let _server_side = accept.await.unwrap();
    assert!(tls::peer_spki(&conn).is_some());
    // asking again during the session works the same, and nothing of it reached QUIC
    let again = client_sock
        .reflexive(server_addr, Duration::from_secs(2))
        .await
        .unwrap();
    assert_eq!(again, seen);
    assert_eq!(
        server_sock
            .answered
            .load(std::sync::atomic::Ordering::SeqCst),
        2
    );
    // a server that is not there: a timeout, not a hang
    let nowhere: SocketAddr = "127.0.0.1:9".parse().unwrap();
    assert!(
        client_sock
            .reflexive(nowhere, Duration::from_millis(300))
            .await
            .is_err()
    );
}

// acceptance: TRV-10
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_message_spanning_many_datagrams_crosses_the_traversal_socket_at_once() {
    use rhtn_crypto::identity::testkit::test_identity;
    use rhtn_transport::tls;
    let node = test_identity("alice");
    let sock = TraversalSocket::bind("127.0.0.1:0".parse().unwrap(), None).unwrap();
    let addr = sock.addr().unwrap();
    let server_ep = endpoint(
        sock.clone(),
        Some({
            let crypto =
                quinn::crypto::rustls::QuicServerConfig::try_from(tls::server_config(&node))
                    .unwrap();
            let mut q = quinn::ServerConfig::with_crypto(std::sync::Arc::new(crypto));
            q.transport_config(std::sync::Arc::new(tls::transport_config()));
            q
        }),
    )
    .unwrap();
    // the far side echoes each stream's length back, so a round trip
    // measures the whole message arriving and not just its first packet
    tokio::spawn(async move {
        while let Some(inc) = server_ep.accept().await {
            tokio::spawn(async move {
                let Ok(conn) = inc.await else { return };
                while let Ok((mut send, mut recv)) = conn.accept_bi().await {
                    let got = recv.read_to_end(1 << 20).await.unwrap_or_default();
                    let _ = send.write_all(&(got.len() as u32).to_be_bytes()).await;
                    let _ = send.finish();
                }
            });
        }
    });

    let me = test_identity("bob");
    let pins = tls::Pins::new();
    pins.pin_identity(&node.public);
    let client_ep = tls::client_endpoint("127.0.0.1:0".parse().unwrap()).unwrap();
    let conn = tls::dial(&client_ep, &me, &pins, &node.public.keyhash, addr)
        .unwrap()
        .await
        .expect("handshake");

    // **A datagram is not a message.**  The kernel's receive offload hands
    // several arrivals over in one buffer, and a socket that flattened them
    // would deliver the first packet of each batch and lose the rest; the
    // sender would then rebuild the message a retransmission at a time, on
    // a backoff, and a message of a few thousand bytes would take tens of
    // seconds instead of milliseconds.
    for n in [1_000usize, 25_000, 200_000] {
        let (mut s, mut r) = conn.open_bi().await.expect("stream");
        s.write_all(&vec![7u8; n]).await.expect("written");
        s.finish().expect("finished");
        let mut back = [0u8; 4];
        let echoed = tokio::time::timeout(Duration::from_secs(5), r.read_exact(&mut back)).await;
        assert!(
            echoed.is_ok(),
            "{n} bytes did not cross within five seconds: the socket is losing packets and the sender is backing off"
        );
        echoed.unwrap().expect("read");
        assert_eq!(
            u32::from_be_bytes(back) as usize,
            n,
            "every byte arrived, and once"
        );
    }
}
