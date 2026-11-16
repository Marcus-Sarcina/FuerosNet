//! **The doorbell rings as RFC 8030 has it** (design §14.1.5;
//! `infra-client-requirements.md` §6.1; SUB-14, MAIL-020): the body is the
//! RFC 8291 construction, held to the RFC's own vector; the token verifies
//! under the lent key; and a loopback endpoint receives one post per ring,
//! signed and encrypted, carrying nothing.

use aws_lc_rs::{agreement, signature};
use rhtn_daemon::doorbell::{Bell, rfc8291_decrypt, rfc8291_encrypt, unbase64url, vapid_token};
use rhtn_node::wake::Endpoint;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::sync::{Arc, mpsc};

fn b64(s: &str) -> Vec<u8> {
    unbase64url(s).expect("the vector's base64url")
}

/// RFC 8291 Appendix A, value for value.
// functional: MAIL-020
#[test]
fn the_encryption_matches_the_rfcs_published_vector() {
    let plaintext = b64("V2hlbiBJIGdyb3cgdXAsIEkgd2FudCB0byBiZSBhIHdhdGVybWVsb24");
    assert_eq!(plaintext, b"When I grow up, I want to be a watermelon");
    let as_private = agreement::PrivateKey::from_private_key(
        &agreement::ECDH_P256,
        &b64("yfWPiYE-n46HLnH0KqZOF1fJJU3MYrct3AELtAQ-oRw"),
    )
    .unwrap();
    let ua_public = b64(
        "BCVxsr7N_eNgVRqvHtD0zTZsEc6-VV-JvLexhqUzORcxaOzi6-AYWXvTBHm4bjyPjs7Vd8pZGH6SRpkNtoIAiw4",
    );
    let salt: [u8; 16] = b64("DGv6ra1nlYgDCS1FRnbzlw").try_into().unwrap();
    let auth = b64("BTBZMqHH6r4Tts7J_aSIgg");
    let body = rfc8291_encrypt(&as_private, &salt, &ua_public, &auth, &plaintext).unwrap();
    let expected = b64(
        "DGv6ra1nlYgDCS1FRnbzlwAAEABBBP4z9KsN6nGRTbVYI_c7VJSPQTBtkgcy27mlmlMoZIIgDll6e3vCYLocInmYWAmS6TlzAC8wEqKK6PBru3jl7A_yl95bQpu6cVPTpK4Mqgkf1CXztLVBSt2Ks3oZwbuwXPXLWyouBWLVWGNWQexSgSxsj_Qulcy4a-fN",
    );
    assert_eq!(body, expected, "the body, header and record alike");
    // and the user agent opens it with its own key
    let ua_private = agreement::PrivateKey::from_private_key(
        &agreement::ECDH_P256,
        &b64("q1dXpw3UpT5VOmu_cf_v6ih07Aems3njxI-JWgLcM94"),
    )
    .unwrap();
    assert_eq!(
        rfc8291_decrypt(&ua_private, &auth, &body).unwrap(),
        plaintext
    );
}

/// RFC 8292: the token's signature verifies under the public point the
/// `k` parameter carries, and the claims name the origin and the expiry.
// functional: MAIL-020
#[test]
fn the_token_verifies_under_the_lent_key_and_names_the_origin() {
    let key = [0x5a; 32];
    let (token, k) = vapid_token(&key, "https://push.example", 1_800_000_000, None).unwrap();
    let parts: Vec<&str> = token.split('.').collect();
    assert_eq!(parts.len(), 3);
    assert_eq!(b64(parts[0]), br#"{"typ":"JWT","alg":"ES256"}"#.to_vec());
    assert_eq!(
        b64(parts[1]),
        br#"{"aud":"https://push.example","exp":1800000000}"#.to_vec()
    );
    let public = b64(&k);
    assert_eq!(public.len(), 65);
    let input = format!("{}.{}", parts[0], parts[1]);
    signature::UnparsedPublicKey::new(&signature::ECDSA_P256_SHA256_FIXED, &public)
        .verify(input.as_bytes(), &b64(parts[2]))
        .expect("ES256 over the signing input");
    let (with, _) = vapid_token(
        &key,
        "https://push.example:8443",
        7,
        Some("mailto:op@example"),
    )
    .unwrap();
    assert_eq!(
        b64(with.split('.').nth(1).unwrap()),
        br#"{"aud":"https://push.example:8443","exp":7,"sub":"mailto:op@example"}"#.to_vec()
    );
}

/// A loopback push service: TLS under the transport's raw-key profile,
/// one request read whole, answered 201, handed back.
fn service(cfg: rustls::ServerConfig) -> (u16, mpsc::Receiver<Vec<u8>>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let (tx, rx) = mpsc::channel();
    let cfg = Arc::new(cfg);
    std::thread::spawn(move || {
        for tcp in listener.incoming() {
            let Ok(tcp) = tcp else { break };
            let conn = rustls::ServerConnection::new(cfg.clone()).unwrap();
            let mut tls = rustls::StreamOwned::new(conn, tcp);
            let mut buf = Vec::new();
            let mut tmp = [0u8; 4096];
            loop {
                match tls.read(&mut tmp) {
                    Ok(0) => break,
                    Ok(n) => {
                        buf.extend_from_slice(&tmp[..n]);
                        if complete(&buf) {
                            break;
                        }
                    }
                    Err(_) => break,
                }
            }
            let _ = tls.write_all(
                b"HTTP/1.1 201 Created\r\ncontent-length: 0\r\nconnection: close\r\n\r\n",
            );
            tls.conn.send_close_notify();
            let _ = tls.flush();
            let _ = tx.send(buf);
        }
    });
    (port, rx)
}

fn complete(buf: &[u8]) -> bool {
    let Some(i) = buf.windows(4).position(|w| w == b"\r\n\r\n") else {
        return false;
    };
    let head = String::from_utf8_lossy(&buf[..i]).to_ascii_lowercase();
    let len = head
        .lines()
        .find_map(|l| l.strip_prefix("content-length:"))
        .and_then(|v| v.trim().parse::<usize>().ok())
        .unwrap_or(0);
    buf.len() >= i + 4 + len
}

fn split(req: &[u8]) -> (String, Vec<u8>) {
    let i = req.windows(4).position(|w| w == b"\r\n\r\n").unwrap();
    (
        String::from_utf8_lossy(&req[..i]).to_string(),
        req[i + 4..].to_vec(),
    )
}

/// **One post per ring, carrying nothing** (SUB-14): the request is signed
/// with the lent key, its body opens under the client's own key to an
/// empty plaintext, and the headers say what the service needs and no
/// more.
// acceptance: SUB-14
// functional: MAIL-020
#[test]
fn a_ring_posts_a_signed_empty_body_to_the_endpoint_and_nothing_else() {
    let node = Arc::new(rhtn_crypto::identity::testkit::test_identity("node"));
    let mut server = rhtn_transport::tls::server_config(rhtn_transport::tls::Presenter::from(node));
    server.alpn_protocols.clear();
    let (port, got) = service(server);
    let me = Arc::new(rhtn_crypto::identity::testkit::test_identity("bob"));
    let mut client = rhtn_transport::tls::client_config_with(
        rhtn_transport::tls::Presenter::from(me),
        vec![rustls::crypto::aws_lc_rs::kx_group::X25519MLKEM768],
    );
    client.alpn_protocols.clear();
    let bell = Bell::with_tls(Arc::new(client), None);

    // the client's keys, as its registration carried them
    let ua_private = agreement::PrivateKey::generate(&agreement::ECDH_P256).unwrap();
    let ua_public = ua_private.compute_public_key().unwrap();
    let auth = [0x11u8; 16];
    let mut key = ua_public.as_ref().to_vec();
    key.extend_from_slice(&auth);
    let signing_key = [0x5a; 32];
    let e = Endpoint {
        url: format!("https://127.0.0.1:{port}/push/v1/abc"),
        key,
        signing_key,
        lapses_at: Some(1_800_000_000 + 600),
    };
    let now = 1_800_000_000;
    for _ in 0..2 {
        assert_eq!(bell.ring(&e, now).unwrap(), 201);
    }
    for _ in 0..2 {
        let req = got
            .recv_timeout(std::time::Duration::from_secs(10))
            .expect("a post");
        let (head, body) = split(&req);
        let lower = head.to_ascii_lowercase();
        assert!(head.starts_with("POST /push/v1/abc HTTP/1.1\r\n"), "{head}");
        assert!(lower.contains("content-encoding: aes128gcm"), "{head}");
        assert!(
            lower.contains("ttl: 600"),
            "the lapse bounds the ttl: {head}"
        );
        let auth_line = head
            .lines()
            .find(|l| l.to_ascii_lowercase().starts_with("authorization:"))
            .expect("signed");
        let (t, k) = auth_line
            .split_once("t=")
            .map(|(_, r)| r.split_once(", k=").unwrap())
            .unwrap();
        let parts: Vec<&str> = t.split('.').collect();
        signature::UnparsedPublicKey::new(&signature::ECDSA_P256_SHA256_FIXED, &b64(k))
            .verify(
                format!("{}.{}", parts[0], parts[1]).as_bytes(),
                &b64(parts[2]),
            )
            .expect("signed with the lent key");
        let claims = String::from_utf8(b64(parts[1])).unwrap();
        assert!(
            claims.contains(&format!("\"aud\":\"https://127.0.0.1:{port}\"")),
            "{claims}"
        );
        assert!(
            !claims.contains("sub"),
            "no contact unless the operator supplies one"
        );
        // the body opens under the client's key to nothing at all
        assert_eq!(
            rfc8291_decrypt(&ua_private, &auth, &body).unwrap(),
            Vec::<u8>::new()
        );
        assert!(
            !head.contains("bob") && !head.contains("node"),
            "no sender, no count: {head}"
        );
    }
    assert!(got.try_recv().is_err(), "two rings, two posts, no more");
    // an endpoint that is not https is refused before anything is sent
    let plain = Endpoint {
        url: "http://127.0.0.1:1/x".into(),
        ..e.clone()
    };
    assert!(bell.ring(&plain, now).is_err());
}
