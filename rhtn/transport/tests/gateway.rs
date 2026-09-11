//! A resource request on the session (`wire-format.md` §11, §9.2;
//! `resource-requirements.md` §3): never processed from early data, one
//! request per stream.

use rhtn_archive::catalog::{ResourceRequest, ResourceResponse, STATUS_DELIVERED};
use rhtn_codec::bounds;
use rhtn_codec::schema::Family;
use rhtn_crypto::identity::testkit::test_identity;
use rhtn_transport::session::*;
use rhtn_transport::tls::{self, Pins};
use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};
use tokio::time::{Duration, Instant, sleep};

fn pins_for(names: &[&str]) -> Pins {
    let p = Pins::new();
    for n in names {
        p.pin_identity(&test_identity(n).public);
    }
    p
}

fn client_cfg(name: &str) -> ClientConfig {
    ClientConfig {
        identity: Arc::new(test_identity(name)),
        pins: pins_for(&["alice", "bob"]),
        capabilities: BTreeMap::new(),
        attestation: None,
        filter: None,
        sibling_cache: Arc::new(Mutex::new(Vec::new())),
        addresses: Arc::new(Mutex::new(Default::default())),
        tls: Arc::new(Mutex::new(Default::default())),
        connect_timeout: Duration::from_millis(1500),
        on_reachability: None,
        log: Log::recording(),
    }
}

// acceptance: RSC-11
#[tokio::test]
async fn a_resource_request_is_never_processed_from_early_data_and_a_stream_carries_one() {
    let handled: Arc<Mutex<Vec<Instant>>> = Arc::default();
    let mut cfg = NodeConfig::defaults(Arc::new(test_identity("bob")), pins_for(&["alice", "bob"]), 30);
    cfg.log = Log::recording();
    let h = handled.clone();
    cfg.on_request = Some(Arc::new(move |_peer, family, _body| {
        let h = h.clone();
        Box::pin(async move {
            if family == Family::ResourceRequest {
                h.lock().unwrap().push(Instant::now());
                Some(ResourceResponse { status: STATUS_DELIVERED, body: Some(b"HTTP/1.1 200 OK\r\ncontent-length: 0\r\n\r\n".to_vec()) }.encode())
            } else {
                None
            }
        })
    }));
    let ep = tls::server_endpoint(&cfg.identity, "127.0.0.1:0".parse().unwrap()).unwrap();
    let addr = ep.local_addr().unwrap();
    let node = Node::new(cfg);
    tokio::spawn(node.clone().serve(ep));
    let ccfg = client_cfg("alice");
    let cep = tls::client_endpoint("127.0.0.1:0".parse().unwrap()).unwrap();
    let target = test_identity("bob").public.keyhash;
    // an earlier session leaves a resumption ticket
    let AttachOutcome::Attached(first) = attach(&ccfg, &cep, target, addr, false).await else { panic!("first attach") };
    first.conn.close(quinn::VarInt::from_u32(0), b"");
    sleep(Duration::from_millis(300)).await;
    // the second dial: the Attach and a ResourceRequest both in 0-RTT early data
    let connecting = tls::dial_with(&cep, ccfg.tls_for(&target).unwrap(), addr).unwrap();
    let Ok((conn, accepted)) = connecting.into_0rtt() else { panic!("a resumption ticket: early data is possible, otherwise this test proves nothing") };
    let (mut s0, _r0) = conn.open_bi().await.unwrap();
    s0.write_all(&control_frame(1, &encode_attach(&ccfg.identity.public.keyhash, None, &ccfg.capabilities))).await.unwrap();
    let (mut rs, mut rr) = conn.open_bi().await.unwrap();
    let req = ResourceRequest { resource: [7; 32], message: b"GET / HTTP/1.1\r\nhost: x\r\n\r\n".to_vec() }.encode();
    rs.write_all(&control_frame(6, &req)).await.unwrap();
    rs.finish().unwrap();
    let sent = Instant::now();
    assert!(accepted.await, "the server took the early data");
    let handshake_done = Instant::now();
    let FrameRead::Payload(p) = read_frame(&mut rr, bounds::REQUEST_FRAME_BYTES).await else { panic!("a response") };
    assert_eq!(ResourceResponse::decode(&p).unwrap().status, STATUS_DELIVERED);
    let at = handled.lock().unwrap()[0];
    assert!(at >= sent, "handled after it was sent");
    assert!(at >= handshake_done, "no backend traffic before the handshake completed: handled {:?} before completion {:?}", at, handshake_done);
    // two requests on one stream: only the first is answered as an exchange
    let (mut s2, mut r2) = conn.open_bi().await.unwrap();
    s2.write_all(&control_frame(6, &req)).await.unwrap();
    s2.write_all(&control_frame(6, &req)).await.unwrap();
    s2.finish().unwrap();
    let FrameRead::Payload(p1) = read_frame(&mut r2, bounds::REQUEST_FRAME_BYTES).await else { panic!("the first is answered") };
    assert_eq!(ResourceResponse::decode(&p1).unwrap().status, STATUS_DELIVERED);
    assert!(matches!(read_frame(&mut r2, bounds::REQUEST_FRAME_BYTES).await, FrameRead::Closed(_)), "the stream ends after one exchange");
    sleep(Duration::from_millis(200)).await;
    assert_eq!(handled.lock().unwrap().len(), 2, "one per stream, never a second on one stream");
}
