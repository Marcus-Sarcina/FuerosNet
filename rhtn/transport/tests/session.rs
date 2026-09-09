//! Session acceptance entries over real loopback QUIC: the node role and
//! the client role each exercised against the implementation on the other
//! side or against a raw test peer that speaks only what the test needs.

use rhtn_codec::cbor::*;
use rhtn_codec::encode::*;
use rhtn_codec::frame::{self, Stream};
use rhtn_crypto::identity::testkit::test_identity;
use rhtn_transport::session::*;
use rhtn_transport::tls::{self, Pins};
use std::collections::BTreeMap;
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};
use tokio::time::{Duration, Instant, sleep};

fn loopback() -> SocketAddr {
    "127.0.0.1:0".parse().unwrap()
}

fn pins_for(names: &[&str]) -> Pins {
    let p = Pins::new();
    for n in names {
        p.pin_identity(&test_identity(n).public);
    }
    p
}

fn node_cfg(name: &str, interval: u64) -> NodeConfig {
    NodeConfig {
        identity: Arc::new(test_identity(name)),
        pins: pins_for(&["alice", "bob", "carol", "c1", "c2", "w1"]),
        interval_secs: interval,
        siblings: Vec::new(),
        capabilities: BTreeMap::from([(capability_id("rhtn/core:max-archive-batch"), vec![0x01, 0x00])]),
        policy: Arc::new(|_| true),
        in_subtree: Arc::new(|_| true),
        filter: None,
    }
}

fn client_cfg(name: &str) -> ClientConfig {
    ClientConfig {
        identity: Arc::new(test_identity(name)),
        pins: pins_for(&["alice", "bob", "carol", "c1", "c2", "w1"]),
        capabilities: BTreeMap::from([(capability_id("rhtn/core:max-archive-batch"), vec![0x00, 0x40])]),
        attestation: None,
        filter: None,
        sibling_cache: Arc::new(Mutex::new(Vec::new())),
        addresses: Arc::new(Mutex::new(Default::default())),
    }
}

/// A serving node on loopback; returns it with its address.
fn spawn_node(cfg: NodeConfig) -> (Arc<Node>, SocketAddr) {
    let ep = tls::server_endpoint(&cfg.identity, loopback()).unwrap();
    let addr = ep.local_addr().unwrap();
    let node = Node::new(cfg);
    tokio::spawn(node.clone().serve(ep));
    (node, addr)
}

fn client_ep() -> quinn::Endpoint {
    tls::client_endpoint(loopback()).unwrap()
}

/// A raw peer that dials a node and speaks stream 0 by hand.
async fn raw_dial(me: &str, target: &str, addr: SocketAddr) -> (quinn::Connection, quinn::SendStream, quinn::RecvStream) {
    let ep = client_ep();
    let id = test_identity(me);
    let pins = pins_for(&[target]);
    let conn = tls::dial(&ep, &id, &pins, &test_identity(target).public.keyhash, addr).unwrap().await.unwrap();
    let (s, r) = conn.open_bi().await.unwrap();
    std::mem::forget(ep);
    (conn, s, r)
}

/// A raw serving peer: accepts one connection and its stream 0.
async fn raw_accept(ep: &quinn::Endpoint) -> (quinn::Connection, quinn::SendStream, quinn::RecvStream) {
    let conn = ep.accept().await.unwrap().await.unwrap();
    let (s, r) = conn.accept_bi().await.unwrap();
    (conn, s, r)
}

fn ack_body(interval: u64) -> Vec<u8> {
    AttachAck { mode: 0, siblings: vec![], interval, queued: 0, capabilities: BTreeMap::new() }.encode()
}

fn sent_frames(log: &Log, t: u64) -> Vec<Vec<u8>> {
    log.events().into_iter().filter_map(|(_, e)| match e { Event::Sent { frame_type, bytes } if frame_type == t => Some(bytes), _ => None }).collect()
}

fn heartbeat_counter(frame: &[u8]) -> Option<u64> {
    let f = frame::parse(Stream::Control, frame).ok()?;
    let Item::Map(m) = &f.body_item else { return None };
    map_get(m, 1).and_then(as_uint)
}

// acceptance: SES-01
#[tokio::test]
async fn ses_01_primary_mode_for_a_client_under_this_node() {
    let (node, addr) = spawn_node(node_cfg("bob", 7));
    let cfg = client_cfg("alice");
    let AttachOutcome::Attached(s) = attach(&cfg, &client_ep(), node.cfg.identity.public.keyhash, addr, false).await else { panic!("attached") };
    assert_eq!(s.ack.mode, 0);
    assert_eq!(s.ack.interval, 7);
}

// acceptance: SES-02
#[tokio::test]
async fn ses_02_accepts_a_client_whose_patron_is_a_light_client() {
    // N adopted P (a light client) who adopted C: N's own topology says C is in its subtree
    let mut cfg = node_cfg("bob", 5);
    let c = test_identity("c1").public.keyhash;
    cfg.in_subtree = Arc::new(move |kh| *kh == c);
    let (node, addr) = spawn_node(cfg);
    let ccfg = client_cfg("c1");
    match attach(&ccfg, &client_ep(), node.cfg.identity.public.keyhash, addr, false).await {
        AttachOutcome::Attached(s) => assert_eq!(s.ack.mode, 0),
        other => panic!("walk-up client was not attached: {other:?}"),
    }
}

// acceptance: TRN-04
#[tokio::test]
async fn trn_04_attach_naming_another_identity_gets_no_ack_and_no_delivery() {
    let (node, addr) = spawn_node(node_cfg("bob", 5));
    let b = test_identity("carol").public.keyhash;
    node.enqueue(b, b"for carol".to_vec());
    let (conn, mut send, mut recv) = raw_dial("alice", "bob", addr).await;
    let body = encode_attach(&b, None, &BTreeMap::new());
    send.write_all(&control_frame(FRAME_ATTACH, &body)).await.unwrap();
    let r = read_frame(&mut recv, 65536).await;
    assert!(matches!(r, FrameRead::Closed(_)), "no frame of any kind in reply, got {r:?}");
    assert_eq!(node.queued(&b), 1, "carol's queue is unchanged");
    assert!(!node.has_session(&b));
    let _ = conn;
}

// acceptance: TRN-06
#[tokio::test]
async fn trn_06_frames_stream_0_as_length_over_typed_array_matching_the_fixtures() {
    let corpus: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/../../test-vectors/corpus.json")).unwrap()).unwrap();
    let fixture = |id: &str| -> Vec<u8> {
        let e = corpus["entries"].as_array().unwrap().iter().find(|e| e["id"] == id).unwrap();
        hex::decode(e["hex"].as_str().unwrap()).unwrap()
    };
    // the corpus derives its named capability from the bare name
    let named = capability_id("max-archive-batch");
    // the corpus Attach: identity, attestation and named capability
    let attach_fx = fixture("P-frame-01");
    let payload = &attach_fx[4..];
    let parts = array_item_ranges(payload, 0).unwrap();
    let body_fx = &payload[parts[1].clone()];
    let __bm_item = parse_all(body_fx).unwrap();
    let Item::Map(bm) = &__bm_item else { panic!() };
    let kh: [u8; 32] = match map_get(&bm, 1) { Some(Item::Bytes(r)) => body_fx[r.clone()].try_into().unwrap(), _ => panic!() };
    let attestation = body_fx[value_slice(body_fx, 2).unwrap()].to_vec();
    let caps_fx = decode_capabilities(body_fx, map_get(&bm, 3).unwrap());
    let named_value = caps_fx.get(&named).expect("the fixture carries the named capability").clone();
    let names = ["alice", "bob", "carol", "alice2", "c1", "c2", "c3", "c4", "c5", "w1"];
    let who = names.iter().find(|n| test_identity(n).public.keyhash == kh).expect("fixture identity is a test identity");
    // the corpus AttachAck: mode, siblings, queued and named capability; interval is the test's
    let ack_fx = fixture("P-frame-02");
    let ap = &ack_fx[4..];
    let aparts = array_item_ranges(ap, 0).unwrap();
    let ack_body_fx = &ap[aparts[1].clone()];
    let ack_fx_decoded = AttachAck::decode(ack_body_fx, &parse_all(ack_body_fx).unwrap()).unwrap();
    let mut ncfg = node_cfg("bob", ack_fx_decoded.interval);
    ncfg.siblings = ack_fx_decoded.siblings.clone();
    ncfg.capabilities = ack_fx_decoded.capabilities.iter().filter(|(k, _)| **k == named).map(|(k, v)| (*k, v.clone())).collect();
    ncfg.pins.pin_identity(&test_identity(who).public);
    let (node, addr) = spawn_node(ncfg);
    let mut ccfg = client_cfg(who);
    ccfg.pins.pin_identity(&test_identity("bob").public);
    ccfg.attestation = Some(attestation);
    ccfg.capabilities = BTreeMap::from([(named, named_value)]);
    let AttachOutcome::Attached(s) = attach(&ccfg, &client_ep(), test_identity("bob").public.keyhash, addr, false).await else { panic!("attached") };
    // the client's first frame on stream 0
    let sent = sent_frames(&s.log, FRAME_ATTACH);
    let first = &sent[0];
    let n = u32::from_be_bytes(first[..4].try_into().unwrap()) as usize;
    assert_eq!(n, first.len() - 4);
    let __a_item = parse_all(&first[4..]).unwrap();
    let Item::Array(a) = &__a_item else { panic!("typed array") };
    assert_eq!(as_uint(&a[0]), Some(1));
    let strip_grease = |body: &[u8]| -> Vec<u8> {
        let __m_item = parse_all(body).unwrap();
        let Item::Map(m) = &__m_item else { panic!() };
        let caps = decode_capabilities(body, map_get(&m, 3).unwrap());
        let kept: BTreeMap<u64, Vec<u8>> = caps.into_iter().filter(|(k, _)| *k == named).collect();
        let r3 = value_slice(body, 3).unwrap();
        let mut out = body[..r3.start].to_vec();
        out.extend_from_slice(&encode_capabilities(&kept));
        out.extend_from_slice(&body[r3.end..]);
        out
    };
    let ranges = array_item_ranges(&first[4..], 0).unwrap();
    assert_eq!(strip_grease(&first[4..][ranges[1].clone()]), strip_grease(body_fx), "Attach body equals the fixture minus greasing");
    // the node's reply
    sleep(Duration::from_millis(200)).await;
    let acks = sent_frames(&node.log, FRAME_ATTACH_ACK);
    let ack = &acks[0];
    let __a_item = parse_all(&ack[4..]).unwrap();
    let Item::Array(a) = &__a_item else { panic!() };
    assert_eq!(as_uint(&a[0]), Some(2));
    let aranges = array_item_ranges(&ack[4..], 0).unwrap();
    let strip_ack = |body: &[u8]| -> Vec<u8> {
        let mut d = AttachAck::decode(body, &parse_all(body).unwrap()).unwrap();
        d.capabilities.retain(|k, _| *k == named);
        d.encode()
    };
    assert_eq!(strip_ack(&ack[4..][aranges[1].clone()]), strip_ack(ack_body_fx), "AttachAck body equals the fixture minus greasing");
}

fn unknown_frame_at_bound() -> Vec<u8> {
    // [99, bstr] exactly 65,536 bytes: 1 (array) + 2 (uint 99) + 3 (bstr head 0x59 + u16) + payload
    let payload_len = 65_536 - 1 - 2 - 3;
    let mut body = Vec::new();
    emit_bstr(&mut body, &vec![0u8; payload_len]);
    let f = control_frame(99, &body);
    assert_eq!(f.len() - 4, 65_536);
    f
}

// acceptance: TRN-07
#[tokio::test]
async fn trn_07_unknown_frame_at_the_bound_is_skipped_before_the_ack_and_mid_session() {
    // as client: a raw server sends the unknown frame, then a valid ack
    let sep = tls::server_endpoint(&test_identity("bob"), loopback()).unwrap();
    let saddr = sep.local_addr().unwrap();
    let server = tokio::spawn(async move {
        let (_c, mut s, mut r) = raw_accept(&sep).await;
        let FrameRead::Payload(_) = read_frame(&mut r, 65536).await else { panic!("attach") };
        s.write_all(&unknown_frame_at_bound()).await.unwrap();
        s.write_all(&control_frame(FRAME_ATTACH_ACK, &ack_body(30))).await.unwrap();
        sleep(Duration::from_secs(1)).await;
    });
    let cfg = client_cfg("alice");
    let AttachOutcome::Attached(s) = attach(&cfg, &client_ep(), test_identity("bob").public.keyhash, saddr, false).await else { panic!("attached after the unknown frame") };
    assert_eq!(s.ack.mode, 0);
    assert_eq!(s.log.count(|e| matches!(e, Event::Skipped { frame_type: 99 })), 1);
    server.abort();
    // as node: a raw client sends it mid-session, then a request on a new stream is answered
    let (node, addr) = spawn_node(node_cfg("bob", 30));
    let (conn, mut send, mut recv) = raw_dial("alice", "bob", addr).await;
    send.write_all(&control_frame(FRAME_ATTACH, &encode_attach(&test_identity("alice").public.keyhash, None, &BTreeMap::new()))).await.unwrap();
    let FrameRead::Payload(p) = read_frame(&mut recv, 65536).await else { panic!("ack") };
    assert!(matches!(classify(&p), Control::Known(rhtn_codec::schema::Family::AttachAck, ..)));
    send.write_all(&unknown_frame_at_bound()).await.unwrap();
    let (mut rs, mut rr) = conn.open_bi().await.unwrap();
    let mut req = Vec::new();
    emit_map_head(&mut req, 2);
    emit_uint(&mut req, 1);
    emit_bstr(&mut req, &[7u8; 32]);
    emit_uint(&mut req, 2);
    emit_bstr(&mut req, &[9u8; 16]);
    let mut payload = Vec::new();
    emit_array_head(&mut payload, 2);
    emit_uint(&mut payload, 8);
    payload.extend_from_slice(&req);
    let mut fr = (payload.len() as u32).to_be_bytes().to_vec();
    fr.extend_from_slice(&payload);
    rs.write_all(&fr).await.unwrap();
    rs.finish().unwrap();
    let FrameRead::Payload(reply) = read_frame(&mut rr, 262_144).await else { panic!("request answered after the unknown frame") };
    let __m_item = parse_all(&reply).unwrap();
    let Item::Map(m) = &__m_item else { panic!() };
    assert_eq!(map_get(&m, 2).and_then(as_uint), Some(1));
    // the request path is independent of stream 0, so the unknown frame may still be in flight
    for _ in 0..40 {
        if node.log.count(|e| matches!(e, Event::Skipped { frame_type: 99 })) == 1 {
            break;
        }
        sleep(Duration::from_millis(50)).await;
    }
    assert_eq!(node.log.count(|e| matches!(e, Event::Skipped { frame_type: 99 })), 1);
    assert!(conn.close_reason().is_none(), "the session survived the unknown frame");
}

// acceptance: TRN-08
#[tokio::test]
async fn trn_08_declared_length_over_the_bound_ends_the_session() {
    // as client
    let sep = tls::server_endpoint(&test_identity("bob"), loopback()).unwrap();
    let saddr = sep.local_addr().unwrap();
    let server = tokio::spawn(async move {
        let (c, mut s, mut r) = raw_accept(&sep).await;
        let FrameRead::Payload(_) = read_frame(&mut r, 65536).await else { panic!("attach") };
        s.write_all(&control_frame(FRAME_ATTACH_ACK, &ack_body(30))).await.unwrap();
        sleep(Duration::from_millis(200)).await;
        let mut over = 65_537u32.to_be_bytes().to_vec();
        over.extend_from_slice(&[0u8; 64]);
        s.write_all(&over).await.unwrap();
        c.closed().await
    });
    let cfg = client_cfg("alice");
    let AttachOutcome::Attached(s) = attach(&cfg, &client_ep(), test_identity("bob").public.keyhash, saddr, false).await else { panic!() };
    let reason = tokio::time::timeout(Duration::from_secs(3), s.conn.closed()).await.expect("client closes the connection");
    assert!(matches!(reason, quinn::ConnectionError::LocallyClosed), "closed by the client: {reason:?}");
    assert_eq!(s.log.count(|e| matches!(e, Event::OverBound)), 1);
    server.abort();
    // as node
    let (node, addr) = spawn_node(node_cfg("bob", 30));
    let (conn, mut send, mut recv) = raw_dial("alice", "bob", addr).await;
    send.write_all(&control_frame(FRAME_ATTACH, &encode_attach(&test_identity("alice").public.keyhash, None, &BTreeMap::new()))).await.unwrap();
    let FrameRead::Payload(_) = read_frame(&mut recv, 65536).await else { panic!("ack") };
    let mut over = 65_537u32.to_be_bytes().to_vec();
    over.extend_from_slice(&[0u8; 64]);
    send.write_all(&over).await.unwrap();
    let reason = tokio::time::timeout(Duration::from_secs(3), conn.closed()).await.expect("node closes the connection");
    assert!(matches!(reason, quinn::ConnectionError::ApplicationClosed(_)), "{reason:?}");
    assert_eq!(node.log.count(|e| matches!(e, Event::OverBound)), 1);
}

fn sibling(name: &str, port: u16) -> SiblingRef {
    let id = test_identity(name).public;
    SiblingRef { keyhash: id.keyhash, endpoints: vec![NetworkPoint { ip: [127, 0, 0, 1], asn: None, port: Some(port as u64) }], key_material: Some(id.key_material()) }
}

// acceptance: TRN-09
#[tokio::test]
async fn trn_09_malformed_sibling_update_is_discarded_whole_and_the_next_one_applies() {
    let l1 = vec![sibling("carol", 4001)];
    let l2 = vec![sibling("c1", 4002), sibling("c2", 4003)];
    let sep = tls::server_endpoint(&test_identity("bob"), loopback()).unwrap();
    let saddr = sep.local_addr().unwrap();
    let l1s = l1.clone();
    let l2s = l2.clone();
    let server = tokio::spawn(async move {
        let (_c, mut s, mut r) = raw_accept(&sep).await;
        let FrameRead::Payload(_) = read_frame(&mut r, 65536).await else { panic!("attach") };
        let ack = AttachAck { mode: 0, siblings: l1s, interval: 30, queued: 0, capabilities: BTreeMap::new() };
        s.write_all(&control_frame(FRAME_ATTACH_ACK, &ack.encode())).await.unwrap();
        sleep(Duration::from_millis(200)).await;
        let mut text = Vec::new();
        emit_tstr(&mut text, "not a sibling update");
        s.write_all(&control_frame(FRAME_SIBLING_UPDATE, &text)).await.unwrap();
        sleep(Duration::from_millis(300)).await;
        s.write_all(&control_frame(FRAME_SIBLING_UPDATE, &encode_sibling_update(&l2s))).await.unwrap();
        sleep(Duration::from_secs(2)).await;
    });
    let cfg = client_cfg("alice");
    let AttachOutcome::Attached(s) = attach(&cfg, &client_ep(), test_identity("bob").public.keyhash, saddr, false).await else { panic!() };
    assert_eq!(*cfg.sibling_cache.lock().unwrap(), l1);
    sleep(Duration::from_millis(400)).await;
    assert_eq!(*cfg.sibling_cache.lock().unwrap(), l1, "still L after the malformed update");
    assert!(!s.conn.close_reason().is_some(), "connection still open");
    sleep(Duration::from_millis(400)).await;
    assert_eq!(*cfg.sibling_cache.lock().unwrap(), l2, "L2 after the valid update");
    assert_eq!(s.log.count(|e| matches!(e, Event::Discarded)), 1);
    server.abort();
}

async fn client_attaches_on_raw_ack(ack: AttachAck) -> Session {
    let sep = tls::server_endpoint(&test_identity("bob"), loopback()).unwrap();
    let saddr = sep.local_addr().unwrap();
    tokio::spawn(async move {
        let (_c, mut s, mut r) = raw_accept(&sep).await;
        let FrameRead::Payload(_) = read_frame(&mut r, 65536).await else { panic!("attach") };
        s.write_all(&control_frame(FRAME_ATTACH_ACK, &ack.encode())).await.unwrap();
        sleep(Duration::from_secs(2)).await;
    });
    let cfg = client_cfg("alice");
    match attach(&cfg, &client_ep(), test_identity("bob").public.keyhash, saddr, false).await {
        AttachOutcome::Attached(s) => s,
        other => panic!("not attached: {other:?}"),
    }
}

async fn node_acks_raw_attach(caps: BTreeMap<u64, Vec<u8>>) -> Arc<Node> {
    let (node, addr) = spawn_node(node_cfg("bob", 30));
    let (_conn, mut send, mut recv) = raw_dial("alice", "bob", addr).await;
    send.write_all(&control_frame(FRAME_ATTACH, &encode_attach(&test_identity("alice").public.keyhash, None, &caps))).await.unwrap();
    let FrameRead::Payload(p) = read_frame(&mut recv, 65536).await else { panic!("no ack") };
    assert!(matches!(classify(&p), Control::Known(rhtn_codec::schema::Family::AttachAck, ..)));
    node
}

// acceptance: TRN-10
#[tokio::test]
async fn trn_10_unrecognised_capability_id_is_tolerated_in_both_roles() {
    let unknown = BTreeMap::from([(0x9e3779b97f4a7c15u64, vec![1u8; 8])]);
    let s = client_attaches_on_raw_ack(AttachAck { mode: 0, siblings: vec![], interval: 30, queued: 0, capabilities: unknown.clone() }).await;
    assert!(s.conn.close_reason().is_none());
    let node = node_acks_raw_attach(unknown).await;
    assert_eq!(node.log.count(|e| matches!(e, Event::Attached { .. })), 1);
}

// acceptance: TRN-11
#[tokio::test]
async fn trn_11_empty_capabilities_and_no_grease_still_attach_in_both_roles() {
    let s = client_attaches_on_raw_ack(AttachAck { mode: 0, siblings: vec![], interval: 30, queued: 0, capabilities: BTreeMap::new() }).await;
    assert!(s.conn.close_reason().is_none());
    let node = node_acks_raw_attach(BTreeMap::new()).await;
    assert_eq!(node.log.count(|e| matches!(e, Event::Attached { .. })), 1);
}

// acceptance: TRN-12
#[tokio::test]
async fn trn_12_every_session_carries_a_fresh_greased_parameter_in_both_roles() {
    let named = capability_id("rhtn/core:max-archive-batch");
    let (node, addr) = spawn_node(node_cfg("bob", 30));
    let mut grease_ids = Vec::new();
    for _ in 0..2 {
        let cfg = client_cfg("alice");
        let AttachOutcome::Attached(s) = attach(&cfg, &client_ep(), test_identity("bob").public.keyhash, addr, false).await else { panic!() };
        let at = &sent_frames(&s.log, FRAME_ATTACH)[0];
        let f = frame::parse(Stream::Control, at).unwrap();
        let Item::Map(m) = &f.body_item else { panic!() };
        let caps = decode_capabilities(&at[4..], map_get(m, 3).unwrap());
        let greased: Vec<_> = caps.iter().filter(|(k, _)| **k != named).collect();
        assert_eq!(greased.len(), 1);
        assert_eq!(greased[0].1.len(), 8);
        grease_ids.push(*greased[0].0);
        let ack_greased: Vec<u64> = s.ack.capabilities.iter().filter(|(k, v)| **k != named && v.len() == 8).map(|(k, _)| *k).collect();
        assert_eq!(ack_greased.len(), 1, "the node greases too");
        grease_ids.push(ack_greased[0]);
    }
    assert_ne!(grease_ids[0], grease_ids[2], "client grease differs between sessions");
    assert_ne!(grease_ids[1], grease_ids[3], "node grease differs between sessions");
    let _ = node;
}

// acceptance: TRN-13
#[tokio::test]
async fn trn_13_refusal_is_close_code_1_with_no_frame() {
    let mut cfg = node_cfg("bob", 30);
    let alice = test_identity("alice").public.keyhash;
    cfg.policy = Arc::new(move |kh| *kh != alice);
    let (node, addr) = spawn_node(cfg);
    let (conn, mut send, mut recv) = raw_dial("alice", "bob", addr).await;
    send.write_all(&control_frame(FRAME_ATTACH, &encode_attach(&alice, None, &BTreeMap::new()))).await.unwrap();
    match read_frame(&mut recv, 65536).await {
        FrameRead::Closed(Some(quinn::ConnectionError::ApplicationClosed(ac))) => assert_eq!(ac.error_code.into_inner(), 1),
        other => panic!("expected close code 1 and no frame, got {other:?}"),
    }
    assert_eq!(node.log.count(|e| matches!(e, Event::Sent { .. })), 0, "no frame of any type in reply");
    let _ = conn;
}

// acceptance: TRN-14
#[tokio::test]
async fn trn_14_close_code_1_ends_the_attempt_without_trying_other_endpoints_or_siblings() {
    let mut cfg = node_cfg("bob", 30);
    let alice = test_identity("alice").public.keyhash;
    cfg.policy = Arc::new(move |kh| *kh != alice);
    let (_node, addr) = spawn_node(cfg);
    // a second endpoint and a sibling, both counting handshakes
    let count = Arc::new(Mutex::new(0usize));
    let mut extra_addrs = Vec::new();
    let mut tasks = Vec::new();
    for name in ["bob", "carol"] {
        let ep = tls::server_endpoint(&test_identity(name), loopback()).unwrap();
        extra_addrs.push(ep.local_addr().unwrap());
        let c = count.clone();
        tasks.push(tokio::spawn(async move {
            while let Some(inc) = ep.accept().await {
                *c.lock().unwrap() += 1;
                let _ = inc.await;
            }
        }));
    }
    let ccfg = client_cfg("alice");
    *ccfg.sibling_cache.lock().unwrap() = vec![sibling("carol", extra_addrs[1].port())];
    ccfg.addresses.lock().unwrap().insert(test_identity("carol").public.keyhash, vec![extra_addrs[1]]);
    let outcome = attach_any(&ccfg, &client_ep(), test_identity("bob").public.keyhash, &[addr, extra_addrs[0]], false).await;
    assert!(matches!(outcome, AttachOutcome::Refused), "{outcome:?}");
    sleep(Duration::from_secs(1)).await;
    assert_eq!(*count.lock().unwrap(), 0, "no second endpoint and no sibling was dialled");
    for t in tasks {
        t.abort();
    }
}

// acceptance: SES-03
#[tokio::test]
async fn ses_03_first_heartbeat_after_one_interval_at_counter_0_then_one_per_interval() {
    let (node, addr) = spawn_node(node_cfg("bob", 1));
    let cfg = client_cfg("alice");
    let t0 = Instant::now();
    let AttachOutcome::Attached(s) = attach(&cfg, &client_ep(), test_identity("bob").public.keyhash, addr, false).await else { panic!() };
    sleep(Duration::from_millis(3400)).await;
    for (log, who) in [(&s.log, "client"), (&node.log, "node")] {
        let beats: Vec<(Instant, u64)> = log.events().into_iter().filter_map(|(t, e)| match e {
            Event::Sent { frame_type: 3, bytes } => Some((t, heartbeat_counter(&bytes).unwrap())),
            _ => None,
        }).collect();
        assert!(beats.len() >= 3, "{who}: {} beats", beats.len());
        assert_eq!(beats[0].1, 0, "{who}: first counter");
        assert!(beats[0].0 >= t0 + Duration::from_millis(950), "{who}: first beat no earlier than one interval");
        assert_eq!((beats[1].1, beats[2].1), (1, 2));
        let gap = beats[2].0 - beats[1].0;
        assert!(gap > Duration::from_millis(900) && gap < Duration::from_millis(1100), "{who}: one per interval, gap {gap:?}");
    }
}

// acceptance: SES-04
#[tokio::test]
async fn ses_04_one_lost_beat_does_not_fail_over() {
    let mut cfg = node_cfg("bob", 1);
    cfg.siblings = vec![sibling("carol", 4009)];
    cfg.filter = Some(Arc::new(|t, bytes| if t == 3 && heartbeat_counter(bytes) == Some(1) { None } else { Some(bytes.to_vec()) }));
    let (_node, addr) = spawn_node(cfg);
    let ccfg = client_cfg("alice");
    let AttachOutcome::Attached(s) = attach(&ccfg, &client_ep(), test_identity("bob").public.keyhash, addr, false).await else { panic!() };
    sleep(Duration::from_millis(6500)).await;
    assert_eq!(*s.reach.lock().unwrap(), Reachability::Reachable, "the client never judged the node unreachable");
    assert_eq!(s.log.count(|e| matches!(e, Event::Failover { .. } | Event::PeerUnreachable)), 0);
    let sent = s.log.count(|e| matches!(e, Event::Sent { frame_type: 3, .. }));
    assert!(sent >= 5, "the client kept sending its own beats: {sent}");
    let received = s.log.count(|e| matches!(e, Event::Received { frame_type: 3 }));
    assert!(received >= 5, "beats after the dropped one were received: {received}");
}

// acceptance: SES-05
#[tokio::test]
async fn ses_05_malformed_heartbeats_count_as_absence_and_three_misses_govern() {
    let (node, addr) = spawn_node(node_cfg("bob", 1));
    let mut ccfg = client_cfg("alice");
    ccfg.filter = Some(Arc::new(|t, bytes| {
        if t == 3 {
            let mut text = Vec::new();
            emit_tstr(&mut text, "beat?");
            Some(control_frame(3, &text))
        } else {
            Some(bytes.to_vec())
        }
    }));
    let alice = test_identity("alice").public.keyhash;
    let AttachOutcome::Attached(s) = attach(&ccfg, &client_ep(), test_identity("bob").public.keyhash, addr, false).await else { panic!() };
    sleep(Duration::from_millis(2500)).await;
    assert_eq!(node.reachability(&alice), Some(Reachability::Reachable), "not before three full intervals");
    assert!(node.log.count(|e| matches!(e, Event::Discarded)) >= 1, "malformed beats were discarded, not fatal");
    assert!(s.conn.close_reason().is_none(), "no close on a malformed frame");
    sleep(Duration::from_millis(1200)).await;
    assert_eq!(node.reachability(&alice), Some(Reachability::Unreachable), "after the third full interval");
}

// acceptance: SES-13
#[tokio::test]
async fn ses_13_unreachable_after_three_full_intervals_of_silence() {
    let (node, addr) = spawn_node(node_cfg("bob", 1));
    let mut ccfg = client_cfg("alice");
    // the blackhole starts after the attach: the filter lets Attach through and drops all else
    ccfg.filter = Some(Arc::new(|t, bytes| if t == 1 { Some(bytes.to_vec()) } else { None }));
    let alice = test_identity("alice").public.keyhash;
    let AttachOutcome::Attached(_s) = attach(&ccfg, &client_ep(), test_identity("bob").public.keyhash, addr, false).await else { panic!() };
    for ms in [500, 1500, 2500] {
        sleep(Duration::from_millis(if ms == 500 { 500 } else { 1000 })).await;
        assert_eq!(node.reachability(&alice), Some(Reachability::Reachable), "reachable at {ms} ms");
    }
    sleep(Duration::from_millis(1100)).await;
    assert_eq!(node.reachability(&alice), Some(Reachability::Unreachable), "unreachable after the third full interval");
}
