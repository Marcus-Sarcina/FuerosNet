//! §9.1's three-way bind and the delegation that carries it
//! (`wire-format.md` §8.2): what an instance presents, what a dialler
//! accepts, and every refusal the wire names.  Real loopback QUIC on both
//! sides; a raw peer where the wire's rule is what a conforming peer would
//! never send.

use rhtn_codec::encode::*;
use rhtn_crypto::delegation::{issue, issue_classical_only, issue_with_window};
use rhtn_crypto::identity::testkit::test_identity;
use rhtn_crypto::verify;
use rhtn_transport::bind::{Binding, Bound, HeldMap, Refusal};
use rhtn_transport::session::*;
use rhtn_transport::tls::{self, Credential, Party, Pins, Presenter};
use std::collections::BTreeMap;
use std::net::SocketAddr;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use tokio::time::Duration;

const WINDOW: u64 = rhtn_codec::schema::DELEGATION_WINDOW_SECONDS;

fn loopback() -> SocketAddr {
    "127.0.0.1:0".parse().unwrap()
}

fn now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs()
}

/// A clock a test moves.
fn movable(t: u64) -> (tls::Clock, Arc<AtomicU64>) {
    let cell = Arc::new(AtomicU64::new(t));
    let c = cell.clone();
    (Arc::new(move || c.load(Ordering::SeqCst)), cell)
}

fn kh(name: &str) -> [u8; 32] {
    test_identity(name).public.keyhash
}

fn pins_for(names: &[&str]) -> Pins {
    let p = Pins::new();
    for n in names {
        p.pin_identity(&test_identity(n).public);
    }
    p
}

fn node_cfg(name: &str, interval: u64) -> NodeConfig {
    let mut cfg = NodeConfig::defaults(
        Arc::new(test_identity(name)),
        pins_for(&["alice", "bob", "carol", "c1", "c2", "w1"]),
        interval,
    );
    cfg.log = Log::recording();
    cfg
}

fn client_cfg(name: &str) -> ClientConfig {
    ClientConfig {
        me: Party::of(Arc::new(test_identity(name))),
        pins: pins_for(&["alice", "bob", "carol", "c1", "c2", "w1"]),
        bind: Default::default(),
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

/// A serving node on loopback, presenting what its configuration says.
fn spawn_node(cfg: NodeConfig) -> (Arc<Node>, SocketAddr) {
    let ep = tls::server_endpoint(cfg.presenter(), loopback()).unwrap();
    let addr = ep.local_addr().unwrap();
    let node = Node::new(cfg);
    tokio::spawn(node.clone().serve(ep));
    (node, addr)
}

fn client_ep() -> quinn::Endpoint {
    tls::client_endpoint(loopback()).unwrap()
}

/// An instance's credential for `name`: a transport key from `seed`, and
/// the delegation `name`'s own identity signed over it, opening at `t`.
fn instance(name: &str, seed: u8, t: u64, clock: tls::Clock) -> Arc<Credential> {
    let id = test_identity(name);
    let cred = Credential::from_seed(&[seed; 32], id.public.keyhash).with_clock(clock);
    cred.add(
        std::slice::from_ref(&id.public),
        &issue(&id, &cred.public(), t),
    )
    .expect("the identity's own delegation over the minted key");
    Arc::new(cred)
}

fn events(log: &Log) -> Vec<Event> {
    log.events().into_iter().map(|(_, e)| e).collect()
}

fn count(log: &Log, f: impl Fn(&Event) -> bool) -> usize {
    events(log).iter().filter(|e| f(e)).count()
}

/// A raw serving peer presenting `me`: accepts one connection, reads its
/// first frame on the first stream, and answers with `ack`.
fn raw_ack_server(me: Presenter, ack: Vec<u8>) -> SocketAddr {
    let ep = tls::server_endpoint(me, loopback()).unwrap();
    let addr = ep.local_addr().unwrap();
    tokio::spawn(async move {
        let conn = ep.accept().await.unwrap().await.unwrap();
        let (mut s, mut r) = conn.accept_bi().await.unwrap();
        let _ = read_frame(&mut r, 65536).await;
        let _ = s.write_all(&control_frame(FRAME_ATTACH_ACK, &ack)).await;
        tokio::time::sleep(Duration::from_secs(2)).await;
        drop(conn);
    });
    addr
}

fn ack_with(delegation: Option<Vec<u8>>) -> Vec<u8> {
    AttachAck {
        mode: 0,
        siblings: vec![],
        interval: 5,
        queued: 0,
        capabilities: BTreeMap::new(),
        delegation,
    }
    .encode()
}

/// A currency request (`wire-format.md` §7.1), which a node with no
/// handler answers "cannot issue" with the nonce: a request any node
/// answers, for a round trip on a connection that opens no session.
fn currency_request(subject: &[u8; 32], nonce: [u8; 16]) -> Vec<u8> {
    let mut out = Vec::new();
    emit_map_head(&mut out, 2);
    emit_uint(&mut out, 1);
    emit_bstr(&mut out, subject);
    emit_uint(&mut out, 2);
    emit_bstr(&mut out, &nonce);
    out
}
const REQUEST_CURRENCY: u64 = 8;

// acceptance: TRN-03
#[tokio::test]
async fn trn_03_refuses_the_session_when_nothing_binds_the_presented_key() {
    // the endpoint the client believes is bob's is answered by carol's own
    // key: a valid Ed25519 key, not the member pinned for bob, and carol
    // presents no delegation by bob
    let (node, addr) = spawn_node(node_cfg("carol", 5));
    let cfg = client_cfg("alice");
    match attach(&cfg, &client_ep(), kh("bob"), addr, false).await {
        AttachOutcome::Unbound(Refusal::Unbound) => {}
        other => panic!("a refused session, not a session with bob: {other:?}"),
    }
    // nothing beyond the Attach was sent: carol's node received one frame
    // and then the close; no session state stands on either side
    tokio::time::sleep(Duration::from_millis(300)).await;
    let received: Vec<u64> = events(&node.log)
        .into_iter()
        .filter_map(|e| match e {
            Event::Received { frame_type } => Some(frame_type),
            _ => None,
        })
        .collect();
    assert_eq!(
        received,
        vec![FRAME_ATTACH],
        "the Attach and nothing after it"
    );
    assert_eq!(count(&node.log, |e| matches!(e, Event::Closed)), 1);
    assert!(
        !node.has_session(&kh("alice")),
        "carol's node holds no session"
    );
    assert!(!node.has_session(&kh("bob")));
}

// acceptance: SES-27
#[tokio::test]
async fn ses_27_every_attachack_from_an_instance_carries_its_delegation() {
    let t = now();
    let cred = instance("bob", 1, t, tls::system_clock());
    let d = cred.current().unwrap().raw;
    let (node, addr) = spawn_node(node_cfg("bob", 5).with_credential(cred));
    let cfg = client_cfg("alice");
    for _ in 0..2 {
        let AttachOutcome::Attached(s) = attach(&cfg, &client_ep(), kh("bob"), addr, false).await
        else {
            panic!("attached to the instance")
        };
        assert_eq!(s.ack.delegation.as_deref(), Some(d.as_slice()));
        assert!(matches!(
            events(&s.log)
                .iter()
                .find(|e| matches!(e, Event::Bound { .. })),
            Some(Event::Bound {
                how: Bound::Presented
            })
        ));
        s.conn.close(0u32.into(), b"");
    }
    assert_eq!(count(&node.log, |e| matches!(e, Event::Attached { .. })), 2);
    // a node holding its own identity key sends field 6 absent
    let (_own, addr) = spawn_node(node_cfg("carol", 5));
    let AttachOutcome::Attached(s) = attach(&cfg, &client_ep(), kh("carol"), addr, false).await
    else {
        panic!("attached to the node")
    };
    assert_eq!(s.ack.delegation, None);
    assert!(matches!(
        events(&s.log)
            .iter()
            .find(|e| matches!(e, Event::Bound { .. })),
        Some(Event::Bound { how: Bound::Pinned })
    ));
}

// acceptance: TRN-23
#[tokio::test]
async fn trn_23_refuses_a_delegation_naming_a_key_other_than_the_one_presented() {
    let t = now();
    // bob's valid delegation over k1, captured by an adversary...
    let k1 = Credential::from_seed(&[1; 32], kh("bob"));
    let captured = issue(&test_identity("bob"), &k1.public(), t);
    // ...who presents its own key k2 and sends the captured delegation
    let k2 = Arc::new(Credential::from_seed(&[2; 32], kh("bob")));
    let addr = raw_ack_server(Presenter::Delegated(k2.clone()), ack_with(Some(captured)));
    let cfg = client_cfg("alice");
    match attach(&cfg, &client_ep(), kh("bob"), addr, false).await {
        AttachOutcome::Unbound(Refusal::WrongKey) => {}
        other => panic!("refused on field 1 against the presented key: {other:?}"),
    }
    assert_eq!(count(&cfg.log, |e| matches!(e, Event::Attached { .. })), 0);
    assert!(
        !cfg.bind.cached(&k1.public()) && !cfg.bind.cached(&k2.public()),
        "a delegation that binds nothing is not cached"
    );
}

// acceptance: TRN-24
#[tokio::test]
async fn trn_24_refuses_a_delegation_by_another_keyhash_than_the_one_sought() {
    // the peer presents k under a valid delegation whose field 2 is carol (T), not bob (S)
    let t = now();
    let carol = instance("carol", 3, t, tls::system_clock());
    let addr = raw_ack_server(
        Presenter::Delegated(carol.clone()),
        ack_with(Some(carol.current().unwrap().raw)),
    );
    let cfg = client_cfg("alice"); // pins both S and T
    match attach(&cfg, &client_ep(), kh("bob"), addr, false).await {
        AttachOutcome::Unbound(Refusal::WrongKeyhash) => {}
        other => panic!("refused: nothing bound to S or to T: {other:?}"),
    }
    assert_eq!(count(&cfg.log, |e| matches!(e, Event::Attached { .. })), 0);
}

// acceptance: TRN-21
#[test]
fn trn_21_a_window_that_is_not_exactly_48_hours_is_malformed() {
    let t = 1_800_000_000;
    let bob = test_identity("bob");
    let k = Credential::from_seed(&[4; 32], kh("bob")).public();
    let pins = pins_for(&["bob"]);
    let (clock, _) = movable(t + 5);
    let b = Binding::default().with_clock(clock);
    for off in [WINDOW - 1, WINDOW + 1] {
        let raw = issue_with_window(&bob, &k, t, t + off);
        assert!(
            matches!(
                b.verify_presented(&pins, Some(&kh("bob")), &k, &raw),
                Err(Refusal::Malformed(_))
            ),
            "not_after = not_before + {off} is malformed"
        );
    }
    let raw = issue_with_window(&bob, &k, t, t + WINDOW);
    assert!(
        b.verify_presented(&pins, Some(&kh("bob")), &k, &raw)
            .is_ok(),
        "exactly 172,800 decodes, and binds within the clock"
    );
}

// acceptance: TRN-22
#[tokio::test]
async fn trn_22_binds_within_the_leeway_and_refuses_beyond_it_and_the_dialler_retries() {
    let t = 1_800_000_000u64;
    let bob = test_identity("bob");
    let k = Credential::from_seed(&[5; 32], kh("bob")).public();
    let pins = pins_for(&["bob"]);
    let (clock, _) = movable(t);
    let windows = [
        (t + 9, true),
        (t + 11, false),
        (t - 9 - WINDOW, true),
        (t - 11 - WINDOW, false),
    ];
    let at_default = Binding::default().with_clock(clock.clone());
    let at_sixty = Binding::default().with_clock(clock).with_leeway(60);
    for (nb, binds) in windows {
        let raw = issue(&bob, &k, nb);
        let r = at_default.verify_presented(&pins, Some(&kh("bob")), &k, &raw);
        assert_eq!(
            r.is_ok(),
            binds,
            "not_before {nb} at the default leeway: {r:?}"
        );
        if !binds {
            assert_eq!(r, Err(Refusal::Window));
        }
        assert!(
            at_sixty
                .verify_presented(&pins, Some(&kh("bob")), &k, &raw)
                .is_ok(),
            "at a leeway of 60 all four bind"
        );
    }
    // the dialler retries a connection refused on the window: alice's
    // desktop attaches under a delegation opening 11 s ahead of bob's clock,
    // and bob's clock is 20 s further on when the retry arrives
    let (node_clock, cell) = movable(t);
    let mut cfg = node_cfg("bob", 5);
    cfg.bind = cfg.bind.with_clock(node_clock);
    let (node, addr) = spawn_node(cfg);
    let alice = test_identity("alice");
    let desktop = Arc::new(Credential::from_seed(&[6; 32], kh("alice")));
    desktop
        .add(
            std::slice::from_ref(&alice.public),
            &issue(&alice, &desktop.public(), t + 11),
        )
        .unwrap();
    let ccfg = client_cfg("alice").with_credential(desktop);
    let ep = client_ep();
    // the node's clock advances once the first refusal has been recorded
    let watch = node.log.clone();
    tokio::spawn(async move {
        loop {
            if count(&watch, |e| matches!(e, Event::Unbound { .. })) > 0 {
                cell.store(t + 20, Ordering::SeqCst);
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    });
    match attach_any(&ccfg, &ep, kh("bob"), &[addr], false).await {
        AttachOutcome::Attached(s) => {
            assert!(node.has_session(&kh("alice")));
            s.conn.close(0u32.into(), b"");
        }
        other => panic!("attached on the retry: {other:?}"),
    }
    assert_eq!(
        count(&node.log, |e| matches!(
            e,
            Event::Unbound {
                why: Refusal::Window
            }
        )),
        1,
        "one window refusal, then the retry bound"
    );
}

// acceptance: TRN-25
#[tokio::test]
async fn trn_25_rejects_a_classical_only_delegation_as_malformed() {
    let t = now();
    let bob = test_identity("bob");
    let cred = Arc::new(Credential::from_seed(&[7; 32], kh("bob")));
    let raw = issue_classical_only(&bob, &cred.public(), t);
    let pins = pins_for(&["bob"]);
    let b = Binding::default();
    assert!(matches!(
        b.verify_presented(&pins, Some(&kh("bob")), &cred.public(), &raw),
        Err(Refusal::Malformed(_))
    ));
    assert!(!b.cached(&cred.public()), "not cached");
    // and the session it arrives on is refused
    let addr = raw_ack_server(Presenter::Delegated(cred.clone()), ack_with(Some(raw)));
    let cfg = client_cfg("alice");
    match attach(&cfg, &client_ep(), kh("bob"), addr, false).await {
        AttachOutcome::Unbound(Refusal::Malformed(_)) => {}
        other => panic!("refused as malformed: {other:?}"),
    }
    assert!(!cfg.bind.cached(&cred.public()));
}

// acceptance: TRN-26
#[tokio::test]
async fn trn_26_verifies_a_delegation_once_per_credential_not_once_per_connection() {
    let t = now();
    let cred = instance("bob", 8, t, tls::system_clock());
    let k = cred.public();
    let (_node, addr) = spawn_node(node_cfg("bob", 5).with_credential(cred));
    let cfg = client_cfg("alice");
    let ep = client_ep();
    for _ in 0..2 {
        let AttachOutcome::Attached(s) = attach(&cfg, &ep, kh("bob"), addr, false).await else {
            panic!("attached")
        };
        s.conn.close(0u32.into(), b"");
    }
    assert_eq!(cfg.bind.verified.load(Ordering::SeqCst), 1);
    assert!(cfg.bind.cached(&k));
    // bob re-provisioned under a different key k': verified again
    let again = instance("bob", 9, t, tls::system_clock());
    let (_node2, addr2) = spawn_node(node_cfg("bob", 5).with_credential(again));
    let AttachOutcome::Attached(s) = attach(&cfg, &ep, kh("bob"), addr2, false).await else {
        panic!("attached under k'")
    };
    s.conn.close(0u32.into(), b"");
    assert_eq!(cfg.bind.verified.load(Ordering::SeqCst), 2);
}

// acceptance: TRN-27
#[tokio::test]
async fn trn_27_a_resumption_after_the_delegation_lapses_is_refused_and_the_next_credential_serves()
{
    let t = 1_800_000_000u64;
    let (clock, cell) = movable(t + WINDOW - 3600); // one hour of validity left
    let cred = instance("bob", 10, t, clock.clone());
    let mut ncfg = node_cfg("bob", 5);
    ncfg.bind = ncfg.bind.with_clock(clock.clone());
    let (_node, addr) = spawn_node(ncfg.with_credential(cred.clone()));
    let mut cfg = client_cfg("alice");
    cfg.bind = cfg.bind.with_clock(clock);
    let ep = client_ep();
    let AttachOutcome::Attached(s) = attach(&cfg, &ep, kh("bob"), addr, false).await else {
        panic!("first attach")
    };
    s.conn.close(0u32.into(), b"");
    // the clock passes not_after plus the leeway; the client resumes 0-RTT
    // on what it holds, and the delegation the instance still presents
    // binds nothing now
    cell.store(t + WINDOW + 20, Ordering::SeqCst);
    match attach(&cfg, &ep, kh("bob"), addr, true).await {
        AttachOutcome::Unbound(Refusal::Window) => {}
        other => panic!("refused on the window: {other:?}"),
    }
    // the next credential of the run comes into force, and a fresh
    // handshake under it succeeds
    let bob = test_identity("bob");
    cred.add(
        std::slice::from_ref(&bob.public),
        &issue(&bob, &cred.public(), t + WINDOW),
    )
    .unwrap();
    let AttachOutcome::Attached(s) = attach(&cfg, &ep, kh("bob"), addr, false).await else {
        panic!("attached under the next credential")
    };
    let d = verify::delegation(
        std::slice::from_ref(&bob.public),
        s.ack.delegation.as_deref().unwrap(),
    )
    .unwrap();
    assert_eq!(d.not_before, t + WINDOW);
}

// acceptance: TRN-18
#[tokio::test]
async fn trn_18_binds_by_a_held_delegation_with_no_attach_and_no_frame() {
    let t = now();
    let cred = instance("bob", 11, t, tls::system_clock());
    let raw = cred.current().unwrap().raw;
    let (_node, addr) = spawn_node(node_cfg("bob", 5).with_credential(cred));
    // A holds B's delegation from a TopologyPush
    let held = Arc::new(HeldMap::default());
    assert!(held.keep(
        verify::delegation(std::slice::from_ref(&test_identity("bob").public), &raw).unwrap()
    ));
    let mut cfg = client_cfg("alice");
    cfg.bind = cfg.bind.with_held(held);
    let ep = client_ep();
    let (conn, how) = connect_request_only(&cfg, &ep, kh("bob"), addr)
        .await
        .expect("bound by the held delegation");
    assert_eq!(how, Bound::Held);
    assert_eq!(
        count(&cfg.log, |e| matches!(e, Event::Received { .. })),
        0,
        "waited for no frame"
    );
    assert_eq!(cfg.bind.verified.load(Ordering::SeqCst), 0);
    // the request goes, and the unsigned reply is accepted as B's
    let nonce = [7u8; 16];
    let reply = request_on(
        &conn,
        REQUEST_CURRENCY,
        &currency_request(&kh("alice"), nonce),
    )
    .await
    .expect("answered");
    assert!(
        reply.windows(16).any(|w| w == nonce),
        "the reply carries the nonce"
    );
}

// acceptance: TRN-19
#[tokio::test]
async fn trn_19_reads_the_delegation_frame_first_and_caches_it_against_the_key() {
    let t = now();
    let cred = instance("bob", 12, t, tls::system_clock());
    let k = cred.public();
    let (node, addr) = spawn_node(node_cfg("bob", 5).with_credential(cred));
    let cfg = client_cfg("alice"); // bob's KeyMaterial, no delegation
    let ep = client_ep();
    let (conn, how) = connect_request_only(&cfg, &ep, kh("bob"), addr)
        .await
        .expect("bound by the frame");
    assert_eq!(how, Bound::Presented);
    let evs = events(&cfg.log);
    let first_frame = evs
        .iter()
        .position(|e| matches!(e, Event::Received { frame_type: 7 }))
        .expect("the delegation frame was read");
    let first_sent = evs.iter().position(|e| matches!(e, Event::Sent { .. }));
    assert!(
        first_sent.is_none_or(|s| s > first_frame),
        "nothing was sent before the frame"
    );
    let reply = request_on(
        &conn,
        REQUEST_CURRENCY,
        &currency_request(&kh("alice"), [1; 16]),
    )
    .await
    .expect("answered after the bind");
    assert!(!reply.is_empty());
    assert!(cfg.bind.cached(&k));
    assert_eq!(
        count(&node.log, |e| matches!(e, Event::DelegationPresented)),
        1
    );
    // a second connection under k waits for no frame
    let (_conn2, how) = connect_request_only(&cfg, &ep, kh("bob"), addr)
        .await
        .expect("bound from the cache");
    assert_eq!(how, Bound::Presented);
    assert_eq!(
        count(&cfg.log, |e| matches!(e, Event::Received { frame_type: 7 })),
        1,
        "the frame was read once"
    );
    assert_eq!(cfg.bind.verified.load(Ordering::SeqCst), 1);
}

// acceptance: TRN-20
#[tokio::test]
async fn trn_20_refuses_an_unbound_key_and_attributes_no_request_before_the_frame() {
    // B presents an unpinned key and its first frame is a Heartbeat
    let ep = tls::server_endpoint(test_identity("carol"), loopback()).unwrap();
    let addr = ep.local_addr().unwrap();
    tokio::spawn(async move {
        let conn = ep.accept().await.unwrap().await.unwrap();
        let (mut s, _) = conn.open_bi().await.unwrap();
        let _ = s
            .write_all(&control_frame(3, &encode_heartbeat(1, 0)))
            .await;
        tokio::time::sleep(Duration::from_secs(2)).await;
    });
    let mut cfg = client_cfg("alice");
    cfg.pins = pins_for(&["bob"]);
    let err = connect_request_only(&cfg, &client_ep(), kh("bob"), addr)
        .await
        .expect_err("refused");
    assert!(err.starts_with("unbound"), "{err}");
    assert_eq!(count(&cfg.log, |e| matches!(e, Event::Sent { .. })), 0);

    // delegated client C sends a verifier query before any delegation frame
    let t = now();
    let (node, addr) = spawn_node(node_cfg("bob", 5));
    let carol = test_identity("carol");
    let c = Arc::new(Credential::from_seed(&[13; 32], kh("carol")));
    c.add(
        std::slice::from_ref(&carol.public),
        &issue(&carol, &c.public(), t),
    )
    .unwrap();
    let ccfg = client_cfg("carol").with_credential(c);
    let tls_cfg = ccfg.tls_for(&kh("bob")).unwrap();
    let ep = client_ep();
    let conn = tls::dial_with(&ep, tls_cfg, addr).unwrap().await.unwrap();
    let (mut s, mut r) = conn.open_bi().await.unwrap();
    // a type-4 request naming C in field 2, as its first and only frame
    let mut query = Vec::new();
    emit_map_head(&mut query, 2);
    emit_uint(&mut query, 1);
    emit_bstr(&mut query, &[9; 16]);
    emit_uint(&mut query, 2);
    emit_bstr(&mut query, &kh("carol"));
    s.write_all(&control_frame(4, &query)).await.unwrap();
    s.finish().unwrap();
    let answer = read_frame(&mut r, 65536).await;
    assert!(
        !matches!(answer, FrameRead::Payload(_)),
        "no verifier answers it: {answer:?}"
    );
    tokio::time::sleep(Duration::from_millis(200)).await;
    assert_eq!(count(&node.log, |e| matches!(e, Event::Bound { .. })), 0);
    assert_eq!(count(&node.log, |e| matches!(e, Event::RequestOnly)), 0);
}

/// A pinned node reaching another for requests alone opens no stream 0:
/// its first stream is a request stream, answered as its own.
#[tokio::test]
async fn a_pinned_peer_is_served_requests_with_no_session_and_no_frame() {
    let (node, addr) = spawn_node(node_cfg("bob", 5));
    let cfg = client_cfg("alice");
    let ep = client_ep();
    let (conn, how) = connect_request_only(&cfg, &ep, kh("bob"), addr)
        .await
        .unwrap();
    assert_eq!(how, Bound::Pinned);
    for i in 0..2u8 {
        let reply = request_on(
            &conn,
            REQUEST_CURRENCY,
            &currency_request(&kh("alice"), [i; 16]),
        )
        .await
        .expect("answered");
        assert!(reply.windows(16).any(|w| w == [i; 16]));
    }
    assert_eq!(count(&node.log, |e| matches!(e, Event::RequestOnly)), 1);
    assert!(!node.has_session(&kh("alice")));
}

/// Alice's desktop: a transport key under alice's own delegation, opening
/// at `t`.
fn desktop(seed: u8, t: u64) -> Arc<Credential> {
    let alice = test_identity("alice");
    let c = Credential::from_seed(&[seed; 32], kh("alice"));
    c.add(
        std::slice::from_ref(&alice.public),
        &issue(&alice, &c.public(), t),
    )
    .unwrap();
    Arc::new(c)
}

// acceptance: SES-26
#[tokio::test]
async fn ses_26_a_subjects_devices_are_several_sessions_each_named_by_its_key() {
    let t = now();
    type Seen = Arc<Mutex<Vec<([u8; 32], [u8; 32])>>>;
    let seen: Seen = Arc::default();
    let mut cfg = node_cfg("bob", 5);
    let sink = seen.clone();
    cfg.on_request = Some(Arc::new(move |peer, device, _family, _body| {
        sink.lock().unwrap().push((peer, device));
        Box::pin(async { None })
    }));
    let (node, addr) = spawn_node(cfg);
    let ep = client_ep();
    // the phone presents alice's classical component
    let phone = client_cfg("alice");
    let AttachOutcome::Attached(s1) = attach(&phone, &ep, kh("bob"), addr, false).await else {
        panic!("the phone attaches")
    };
    // the desktop presents a delegated key under alice's delegation, field 1 = alice
    let d = desktop(31, t);
    let laptop = client_cfg("alice").with_credential(d.clone());
    let AttachOutcome::Attached(s2) = attach(&laptop, &ep, kh("bob"), addr, false).await else {
        panic!("the desktop attaches")
    };
    let phone_key = test_identity("alice").public.ed.to_bytes();
    let mut devices = node.devices_of(&kh("alice"));
    devices.sort();
    let mut expected = vec![phone_key, d.public()];
    expected.sort();
    assert_eq!(
        devices, expected,
        "two sessions for alice, one per presented key"
    );
    assert!(node.has_session_for(&kh("alice"), &phone_key));
    assert!(node.has_session_for(&kh("alice"), &d.public()));
    // a request from each session is attributed to the device it arrived on
    let _ = request_on(
        &s1.conn,
        REQUEST_CURRENCY,
        &currency_request(&kh("alice"), [1; 16]),
    )
    .await;
    let _ = request_on(
        &s2.conn,
        REQUEST_CURRENCY,
        &currency_request(&kh("alice"), [2; 16]),
    )
    .await;
    tokio::time::sleep(Duration::from_millis(200)).await;
    let seen = seen.lock().unwrap().clone();
    assert!(seen.contains(&(kh("alice"), phone_key)), "{seen:?}");
    assert!(seen.contains(&(kh("alice"), d.public())), "{seen:?}");
    // neither session ended the other
    assert!(!s1.conn.close_reason().is_some());
    assert!(!s2.conn.close_reason().is_some());
}

// acceptance: QUE-21
#[tokio::test]
async fn que_21_a_ciphertext_waits_for_the_device_it_names_and_reaches_that_session_alone() {
    let t = now();
    let (node, addr) = spawn_node(node_cfg("bob", 5));
    let phone_key = test_identity("alice").public.ed.to_bytes();
    let d2 = desktop(32, t);
    // both devices offline: a submission for alice naming the phone (d1)
    node.enqueue_for(kh("alice"), phone_key, b"for the phone".to_vec())
        .unwrap();
    assert_eq!(node.queued(&kh("alice")), 1);
    // d2 attaches, drains and detaches: told nothing waits, given nothing
    let ep = client_ep();
    let laptop = client_cfg("alice").with_credential(d2.clone());
    let AttachOutcome::Attached(mut s2) = attach(&laptop, &ep, kh("bob"), addr, false).await else {
        panic!("the desktop attaches")
    };
    assert_eq!(s2.ack.queued, 0, "field 4 is 0 for d2");
    assert!(
        tokio::time::timeout(Duration::from_millis(500), s2.deliveries.recv())
            .await
            .is_err(),
        "d2 receives nothing"
    );
    s2.conn.close(0u32.into(), b"");
    assert_eq!(node.queued(&kh("alice")), 1, "still waiting for d1");
    // then d1 attaches
    let phone = client_cfg("alice");
    let AttachOutcome::Attached(mut s1) = attach(&phone, &ep, kh("bob"), addr, false).await else {
        panic!("the phone attaches")
    };
    assert_eq!(s1.ack.queued, 1, "field 4 is 1 for d1");
    let got = tokio::time::timeout(Duration::from_secs(3), s1.deliveries.recv())
        .await
        .expect("delivered")
        .expect("open");
    assert_eq!(got, b"for the phone");
    tokio::time::sleep(Duration::from_millis(200)).await;
    assert_eq!(node.queued(&kh("alice")), 0);
}
