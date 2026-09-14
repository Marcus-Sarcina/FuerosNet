//! The node's decisions over real sessions: a topology push crossing two
//! QUIC hops, a resolution and a currency request answered on request
//! streams, and what a running node replicates.  One entry per area the
//! fabric tests cover, end to end.

mod common;

use common::*;
use rhtn_node::currency::{CurrencyReply, CurrencyRequest, ROLE_PATRON};
use rhtn_node::resolution::{AnchorTable, Ingestion, Path, REQUEST_CURRENCY, REQUEST_RESOLVE, ResolveReply, ResolveRequest};
use rhtn_node::runtime::{LiveNode, pump_client};
use rhtn_node::view::NodeView;
use rhtn_transport::session::*;
use std::sync::{Arc, Mutex};
use std::time::Duration;

const I: u64 = 30;

/// No sessions at all: for a view that is only being set up.
struct Quiet;
impl rhtn_node::Adjacency for Quiet {
    fn peers(&self) -> Vec<rhtn_archive::Keyhash> {
        Vec::new()
    }
    fn send(&self, _: &rhtn_archive::Keyhash, _: u64, _: &[u8]) {}
    fn request(&self, _: &rhtn_archive::Keyhash, _: u64, _: &[u8]) -> bool {
        false
    }
}

/// P (alice) is the root; N (bob) is its infra child at index 0; C (carol)
/// is a light client under N at index 2.  N attaches upstream to P and C
/// attaches to N.
struct Tree {
    signers: Signers,
    p: Arc<LiveNode>,
    n: Arc<LiveNode>,
    upstream: Session,
    c: Session,
    c_view: Arc<Mutex<NodeView>>,
}

async fn tree() -> Tree {
    let mut s = Signers::new();
    let a_n = s.adopt("bob", "alice", "alice", &[0], 1);
    let a_c = s.adopt("carol", "bob", "alice", &[0, 2], 2);
    let records = [&a_n, &a_c];
    let infra = ["alice", "bob"];

    let mut pcfg = node_cfg("alice", I);
    pcfg.in_subtree = Arc::new(|_| true);
    let mut p_view = view_of("alice", table_of("alice", &s, &records, &infra), "alice", &[], s.clock);
    p_view.set_slot(0, Some(kh("bob")), s.clock);
    let p = LiveNode::start(pcfg, p_view, ids(), AnchorTable::new(0, Ingestion::UnverifiedGossip));

    let ncfg = node_cfg("bob", I);
    let mut n_view = view_of("bob", table_of("bob", &s, &records, &infra), "alice", &[0], s.clock);
    n_view.set_slot(2, Some(kh("carol")), s.clock);
    n_view.attached.insert(kh("carol"));
    let n = LiveNode::start(ncfg, n_view, ids(), AnchorTable::new(0, Ingestion::UnverifiedGossip));

    // N holds a session upstream to P
    let ncli = client_cfg("bob");
    know(&ncli, "alice", p.addr);
    let upstream = match n.attach_upstream(&ncli, kh("alice")).await {
        AttachOutcome::Attached(x) => x,
        other => panic!("{other:?}"),
    };
    assert_eq!(upstream.ack.mode, 0);

    // C attaches to N and keeps its own view
    let ccfg = client_cfg("carol");
    know(&ccfg, "bob", n.addr);
    let mut c = match attach(&ccfg, &client_ep(), kh("bob"), n.addr, false).await {
        AttachOutcome::Attached(x) => x,
        other => panic!("{other:?}"),
    };
    let mut cv = view_of("carol", table_of("carol", &s, &records, &infra), "alice", &[0, 2], s.clock);
    cv.serving_node = Some(kh("bob"));
    let c_view = Arc::new(Mutex::new(cv));
    let _adj = pump_client(&mut c, kh("bob"), c_view.clone(), Arc::new(Mutex::new(ids())));
    tokio::time::sleep(Duration::from_millis(200)).await;
    Tree { signers: s, p, n, upstream, c, c_view }
}

/// PRP-01 over real sessions: P originates an adoption in N's h_store; N
/// stores it and forwards it to its attached client, byte for byte.
#[tokio::test]
async fn a_push_crosses_two_real_sessions() {
    let mut t = tree().await;
    // **what P has already heard is not zero.**  N reconciled when it
    // attached (`wire-format.md` §10.1.3), which handed P the endpoint
    // record N published at start; the claim below is about the push, so
    // it is measured from here rather than from nothing
    let before = t.p.node.log.count(|e| matches!(e, Event::Received { frame_type: 5 }));
    // w1 adopted under N: its patron is N itself, so it is in N's h_store,
    // and one edge below P's subordinate, so in P's
    let obj = t.signers.adopt("w1", "bob", "alice", &[0, 3], 3);
    for v in [&t.p.view, &t.n.view, &t.c_view] {
        v.lock().unwrap().store.keep_presence(obj.field_hash(8).unwrap(), t.signers.store[&obj.field_hash(8).unwrap()].clone());
    }
    assert_eq!(t.p.originate_transaction(&obj.bytes), rhtn_node::store::Decision::Stored);
    // it reaches N on the upstream session, and C on its own
    let deadline = tokio::time::Instant::now() + Duration::from_secs(3);
    while tokio::time::Instant::now() < deadline && !t.n.view.lock().unwrap().store.holds_txid(&obj.txid) {
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    assert!(t.n.view.lock().unwrap().store.holds_txid(&obj.txid), "N stored it");
    assert!(t.n.view.lock().unwrap().table.patrons(&kh("w1")).contains(&kh("bob")), "and its table followed");
    assert_eq!(t.n.view.lock().unwrap().slots.get(&3).and_then(|s| s.occupant), Some(kh("w1")), "and its slot was written");
    let got = tokio::time::timeout(Duration::from_secs(3), async {
        loop {
            if t.c_view.lock().unwrap().store.holds_txid(&obj.txid) {
                break;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await;
    assert!(got.is_ok(), "C received it on its session with N");
    let held = t.c_view.lock().unwrap().store.transaction(&obj.txid).map(|r| r.bytes.clone()).unwrap();
    assert_eq!(held, obj.bytes, "byte for byte");
    // and P did not get it back: N's forwarding excluded the arrival session
    assert_eq!(
        t.p.node.log.count(|e| matches!(e, Event::Received { frame_type: 5 })),
        before,
        "nothing came back up to P"
    );
    drop(t.upstream);
}

/// RES-08 over a real request stream: C asks N to resolve a client two
/// levels down; N is on the path and answers with the residual suffix.
#[tokio::test]
async fn a_resolution_is_answered_on_a_request_stream() {
    let t = tree().await;
    let req = ResolveRequest { subject: kh("carol"), anchor: kh("alice"), path: Path::from_indices(&[0, 2]).bytes, nibbles: 2, nonce: [8; 16] };
    let bytes = t.c.request(REQUEST_RESOLVE, &req.encode()).await.expect("a reply");
    let reply = ResolveReply::decode(&bytes).unwrap();
    let ResolveReply::Serving { nonce, serving } = reply else { panic!("{reply:?}") };
    assert_eq!(nonce, [8; 16]);
    assert_eq!(serving.node, kh("bob"), "N answers for itself");
    assert_eq!(serving.residual.indices(), vec![2], "with the suffix that names C");
    assert!(serving.key_material.is_some());
    drop(t.upstream);
}

/// A resolution N cannot answer from its own tables is run on the
/// client's behalf: N dials the anchor from its table and follows the
/// referral to the serving node, over real request streams.
#[tokio::test]
async fn a_resolution_is_proxied_through_a_real_referral() {
    let t = tree().await;
    // another subnet: W (w1) root over V (w2) infra at index 4, serving X (c1) at index 1
    let mut s2 = Signers::new();
    let a_v = s2.adopt("w2", "w1", "w1", &[4], 1);
    let a_x = s2.adopt("c1", "w2", "w1", &[4, 1], 2);
    let recs = [&a_v, &a_x];
    let mut w_view = view_of("w1", table_of("w1", &s2, &recs, &["w1", "w2"]), "w1", &[], s2.clock);
    w_view.set_slot(4, Some(kh("w2")), s2.clock);
    let mut v_view = view_of("w2", table_of("w2", &s2, &recs, &["w1", "w2"]), "w1", &[4], s2.clock);
    v_view.set_slot(1, Some(kh("c1")), s2.clock);
    v_view.attached.insert(kh("c1"));
    let v = LiveNode::start(node_cfg("w2", I), v_view, ids(), AnchorTable::new(0, Ingestion::UnverifiedGossip));
    // W holds V's endpoint record, so it can refer
    let v_record = rhtn_node::resolution::endpoint_record(&id("w2"), &[NetworkPoint::from_socket(v.addr).unwrap()], rhtn_archive::tx::Seqno { series: 1, counter: 1 });
    w_view.take_object(&Quiet, &kh("w2"), rhtn_node::store::KIND_ENDPOINT_RECORD, &v_record, &ids());
    let w = LiveNode::start(node_cfg("w1", I), w_view, ids(), AnchorTable::new(0, Ingestion::UnverifiedGossip));
    // V publishes its own endpoints, so its serving answer names them
    v.view.lock().unwrap().take_object(&Quiet, &kh("w2"), rhtn_node::store::KIND_ENDPOINT_RECORD, &v_record, &ids());
    // N's anchor table has an entry for W
    let entry = rhtn_node::resolution::anchor_entry(&id("w1"), &[NetworkPoint::from_socket(w.addr).unwrap()], 50, rhtn_archive::tx::Seqno { series: 1, counter: 1 });
    assert!(t.n.anchors.lock().unwrap().offer(rhtn_node::resolution::AnchorEntry::parse(&entry).unwrap(), &ids()));
    // C asks N about X under W
    let req = ResolveRequest { subject: kh("c1"), anchor: kh("w1"), path: Path::from_indices(&[4, 1]).bytes, nibbles: 2, nonce: [9; 16] };
    let bytes = t.c.request(REQUEST_RESOLVE, &req.encode()).await.expect("a reply");
    let reply = ResolveReply::decode(&bytes).unwrap();
    let ResolveReply::Serving { nonce, serving } = reply else { panic!("{reply:?}") };
    assert_eq!(nonce, [9; 16], "under C's nonce");
    assert_eq!(serving.node, kh("w2"), "X's serving node");
    assert_eq!(serving.residual.indices(), vec![1]);
    assert_eq!(serving.endpoints.first().map(|e| e.socket()), Some(v.addr));
    drop(t.upstream);
}

/// CUR-01 over a real request stream: C asks its patron for currency and
/// gets an attestation the ladder issued.
#[tokio::test]
async fn a_currency_request_is_answered_by_the_patron() {
    let t = tree().await;
    let req = CurrencyRequest { subject: kh("carol"), nonce: [7; 16] };
    let bytes = t.c.request(REQUEST_CURRENCY, &req.encode()).await.expect("a reply");
    let reply = CurrencyReply::decode(&bytes).unwrap();
    let CurrencyReply::Attestation { nonce, bytes } = reply else { panic!("{reply:?}") };
    assert_eq!(nonce, [7; 16]);
    let a = rhtn_archive::currency::parse_attestation(&ids(), &bytes).unwrap();
    assert_eq!((a.subject, a.current, a.issuer, a.role), (kh("carol"), kh("carol"), kh("bob"), ROLE_PATRON));
    // and a subject N holds no record of gets code 1
    let bytes = t.c.request(REQUEST_CURRENCY, &CurrencyRequest { subject: kh("c2"), nonce: [6; 16] }.encode()).await.unwrap();
    assert_eq!(CurrencyReply::decode(&bytes).unwrap(), CurrencyReply::CannotIssue { nonce: [6; 16] });
    drop(t.upstream);
}

// acceptance: REP-16
#[tokio::test]
async fn a_running_node_replicates_its_store_and_never_its_mailbox() {
    let t = tree().await;
    // C leaves, and a message then waits for it at N's transport
    t.c.conn.close(0u32.into(), b"gone");
    drop(t.c);
    let deadline = tokio::time::Instant::now() + Duration::from_secs(3);
    while t.n.node.has_session(&kh("carol")) && tokio::time::Instant::now() < deadline {
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    assert!(!t.n.node.has_session(&kh("carol")));
    let queued = b"ciphertext for C".to_vec();
    t.n.node.enqueue(kh("carol"), queued.clone()).unwrap();
    assert_eq!(t.n.node.queued(&kh("carol")), 1, "N holds it in its mailbox");
    // C's adoption is in N's store; a presence record beside it
    let a_c = t.signers.store.values().find(|b| rhtn_archive::record::Record::parse(b).map(|r| r.field_hash(1) == Some(kh("carol"))).unwrap_or(false)).unwrap().clone();
    let a_c = rhtn_archive::record::Record::parse(&a_c).unwrap();
    t.n.originate_transaction(&a_c.bytes);
    let pop = t.signers.store[&a_c.field_hash(8).unwrap()].clone();
    t.n.view.lock().unwrap().store.keep_presence(a_c.field_hash(8).unwrap(), pop.clone());
    let payload = t.n.replication_payload();
    assert!(payload.iter().any(|i| matches!(i, rhtn_node::peering::Replicated::Topology { object, .. } if *object == a_c.bytes)), "C's adoption replicates");
    let carries_queue = payload.iter().any(|i| match i {
        rhtn_node::peering::Replicated::Topology { object, .. } | rhtn_node::peering::Replicated::TrustBearing { object } => object.windows(queued.len()).any(|w| w == queued),
        _ => false,
    });
    assert!(!carries_queue, "and the mailbox is no part of it");
    assert_eq!(t.n.node.queued(&kh("carol")), 1, "which still waits at N alone");
    drop(t.upstream);
}

/// Request streams are rate-limited per requester (`wire-format.md` §7.1):
/// past the allowance a request fails, and the allowance is the
/// operator's.
#[tokio::test]
async fn requests_past_the_allowance_fail_the_stream() {
    let mut s = Signers::new();
    let a_c = s.adopt("carol", "bob", "bob", &[2], 2);
    let mut n_view = view_of("bob", table_of("bob", &s, &[&a_c], &["bob"]), "bob", &[], s.clock);
    n_view.attached.insert(kh("carol"));
    let n = LiveNode::start_with(node_cfg("bob", I), n_view, ids(), AnchorTable::new(0, Ingestion::UnverifiedGossip), rhtn_node::runtime::RateLimit::new(3, Duration::from_secs(60)));
    let ccfg = client_cfg("carol");
    know(&ccfg, "bob", n.addr);
    let c = match attach(&ccfg, &client_ep(), kh("bob"), n.addr, false).await {
        AttachOutcome::Attached(x) => x,
        other => panic!("{other:?}"),
    };
    let mut answered = 0;
    let mut failed = 0;
    for i in 0..6u8 {
        match c.request(REQUEST_CURRENCY, &CurrencyRequest { subject: kh("carol"), nonce: [i; 16] }.encode()).await {
            Ok(bytes) => {
                assert!(matches!(CurrencyReply::decode(&bytes).unwrap(), CurrencyReply::Attestation { .. }));
                answered += 1;
            }
            Err(_) => failed += 1,
        }
    }
    assert_eq!((answered, failed), (3, 3), "three within the allowance, three past it");
    assert_eq!(n.limits.per_window, 3);
}
