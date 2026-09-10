//! The genesis of a new root over real sessions: it starts with nothing,
//! peers with a node of an existing subnet, its peering floods that subnet,
//! and the subnet's client resolves it through the anchor table and reaches
//! it on a fresh session.

mod common;

use common::*;
use rhtn_archive::record::Record;
use rhtn_archive::tx::{self, Seqno};
use rhtn_node::currency::{CurrencyReply, CurrencyRequest};
use rhtn_node::resolution::{AnchorEntry, AnchorTable, Ingestion, Path, REQUEST_CURRENCY, REQUEST_RESOLVE, ResolveReply, ResolveRequest, anchor_entry};
use rhtn_node::runtime::{LiveNode, pump_client};
use rhtn_node::store::Decision;
use rhtn_transport::session::*;
use std::sync::{Arc, Mutex};
use std::time::Duration;

const I: u64 = 30;

#[tokio::test]
async fn a_new_root_peers_floods_and_is_reached_over_real_sessions() {
    // an existing subnet: P (alice) root, N (bob) its infra child at index
    // 0, C (carol) a light client under N at index 2
    let mut s = Signers::new();
    let a_n = s.adopt("bob", "alice", "alice", &[0], 1);
    let a_c = s.adopt("carol", "bob", "alice", &[0, 2], 2);
    let records = [&a_n, &a_c];
    let infra = ["alice", "bob"];
    let mut p_view = view_of("alice", table_of("alice", &s, &records, &infra), "alice", &[], s.clock);
    p_view.set_slot(0, Some(kh("bob")), s.clock);
    let p = LiveNode::start(node_cfg("alice", I), p_view, ids(), AnchorTable::new(0, Ingestion::UnverifiedGossip));
    let mut n_view = view_of("bob", table_of("bob", &s, &records, &infra), "alice", &[0], s.clock);
    n_view.set_slot(2, Some(kh("carol")), s.clock);
    n_view.attached.insert(kh("carol"));
    let n = LiveNode::start(node_cfg("bob", I), n_view, ids(), AnchorTable::new(0, Ingestion::UnverifiedGossip));
    let ncli = client_cfg("bob");
    know(&ncli, "alice", p.addr);
    let AttachOutcome::Attached(upstream) = n.attach_upstream(&ncli, kh("alice")).await else { panic!("N attaches to P") };
    let ccfg = client_cfg("carol");
    know(&ccfg, "bob", n.addr);
    let AttachOutcome::Attached(mut c) = attach(&ccfg, &client_ep(), kh("bob"), n.addr, false).await else { panic!("C attaches to N") };
    let mut cv = view_of("carol", table_of("carol", &s, &records, &infra), "alice", &[0, 2], s.clock);
    cv.serving_node = Some(kh("bob"));
    let c_view = Arc::new(Mutex::new(cv));
    let _adj = pump_client(&mut c, kh("bob"), c_view.clone(), Arc::new(Mutex::new(ids())));
    tokio::time::sleep(Duration::from_millis(200)).await;

    // R (w2) comes into being: self-anchored, a table holding itself alone
    let r_view = view_of("w2", table_of("w2", &s, &[], &["w2"]), "w2", &[], s.clock);
    assert!(r_view.is_root());
    let r = LiveNode::start(node_cfg("w2", I), r_view, ids(), AnchorTable::new(0, Ingestion::UnverifiedGossip));
    assert!(!r.view.lock().unwrap().own_endpoints().is_empty(), "it published its own endpoint record at start");

    // R and N meet and peer; R holds a session to N for it.  Nothing waits
    // on a staple: R has none and no issuer to ask (design §12.6.5, §12.7.1)
    r.view.lock().unwrap().peers.insert(kh("bob"));
    n.view.lock().unwrap().peers.insert(kh("w2"));
    let rcli = client_cfg("w2");
    know(&rcli, "bob", n.addr);
    let r_to_n = match r.attach_upstream(&rcli, kh("bob")).await {
        AttachOutcome::Attached(x) => x,
        other => panic!("R reaches N: {other:?}"),
    };
    let pop = s.formation("bob", "w2");
    let body = r.view.lock().unwrap().propose_peering(&kh("bob"), &NetworkPoint::from_socket(n.addr).unwrap(), &pop.txid, &[rhtn_archive::genesis(&kh("bob"))]).expect("a naked root peers");
    let peering = Record::parse(&tx::envelope(tx::TYPE_PEERING, &body, &[&id("w2"), &id("bob")])).unwrap();
    assert_eq!(r.originate_transaction(&peering.bytes), Decision::Stored);

    // N stores it and floods it: up to P and down to C, over real sessions
    assert!(until(3000, || n.view.lock().unwrap().store.holds_txid(&peering.txid)).await, "N stored R's peering");
    assert!(until(3000, || p.view.lock().unwrap().store.holds_txid(&peering.txid)).await, "P received it from N");
    assert!(until(3000, || c_view.lock().unwrap().store.holds_txid(&peering.txid)).await, "C received it from N");
    assert!(c_view.lock().unwrap().peers_of(&kh("bob")).contains(&kh("w2")), "C now holds R's edge");
    assert_eq!(c_view.lock().unwrap().standing(&kh("w2")), 10.0, "and R has standing at C, one edge beyond N");

    // reachability: N's policy admits a root of nobody into its anchor
    // table (design §12.7.3), C asks N to resolve R, and N runs it on C's
    // behalf against R's real endpoint
    let entry = anchor_entry(&id("w2"), &[NetworkPoint::from_socket(r.addr).unwrap()], 0, Seqno { series: 1, counter: 1 });
    assert!(n.anchors.lock().unwrap().offer(AnchorEntry::parse(&entry).unwrap(), &ids()));
    let req = ResolveRequest { subject: kh("w2"), anchor: kh("w2"), path: Path::empty().bytes, nibbles: 0, nonce: [5; 16] };
    let bytes = c.request(REQUEST_RESOLVE, &req.encode()).await.expect("a reply");
    let reply = ResolveReply::decode(&bytes).unwrap();
    let ResolveReply::Serving { nonce, serving } = reply else { panic!("{reply:?}") };
    assert_eq!(nonce, [5; 16]);
    assert_eq!(serving.node, kh("w2"), "R answered for itself");
    assert!(serving.residual.is_empty());
    assert_eq!(serving.endpoints.first().map(|e| e.socket()), Some(r.addr));

    // and C reaches R on a fresh session of its own, which R accepts and answers
    let ccfg2 = client_cfg("carol");
    know(&ccfg2, "w2", r.addr);
    let AttachOutcome::Attached(cr) = attach(&ccfg2, &client_ep(), kh("w2"), r.addr, false).await else { panic!("C reaches R") };
    let bytes = cr.request(REQUEST_CURRENCY, &CurrencyRequest { subject: kh("carol"), nonce: [6; 16] }.encode()).await.expect("answered");
    assert_eq!(CurrencyReply::decode(&bytes).unwrap(), CurrencyReply::CannotIssue { nonce: [6; 16] }, "R holds no record of C and says so");
    let bytes = cr.request(REQUEST_RESOLVE, &req.encode()).await.expect("answered");
    assert!(matches!(ResolveReply::decode(&bytes).unwrap(), ResolveReply::Serving { .. }), "R resolves itself directly");
    drop(upstream);
    drop(r_to_n);
}
