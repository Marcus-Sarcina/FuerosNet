//! What a running node carries from its view into its serving state, and
//! what a restarted node derives from the store it kept: a recovery ends
//! the old credential's service (QUE-18), and endpoint publication takes
//! its counter from the record already held (RES-17).

mod common;

use common::*;
use rhtn_node::resolution::{AnchorTable, Ingestion, NetworkPoint};
use rhtn_node::runtime::LiveNode;
use rhtn_node::store::{Decision, TopologyStore};
use rhtn_transport::session::*;

const I: u64 = 30;

fn anchors() -> AnchorTable {
    AnchorTable::new(0, Ingestion::UnverifiedGossip)
}

// acceptance: QUE-18
#[tokio::test]
async fn a_recovery_the_node_applies_ends_the_old_credentials_service() {
    // bob is a root serving alice, its light client at index 0
    let mut s = Signers::new();
    let a_a = s.adopt("alice", "bob", "bob", &[0], 1);
    let records = [&a_a];
    let infra = ["bob"];
    let mut v = view_of("bob", table_of("bob", &s, &records, &infra), "bob", &[], s.clock);
    v.set_slot(0, Some(kh("alice")), s.clock);
    v.attached.insert(kh("alice"));
    let n = LiveNode::start(node_cfg("bob", I), v, ids(), anchors());
    let acfg = client_cfg("alice");
    know(&acfg, "bob", n.addr);
    let AttachOutcome::Attached(_alice) = attach(&acfg, &client_ep(), kh("bob"), n.addr, false).await else { panic!("alice attaches") };
    assert!(until(2000, || n.node.has_session(&kh("alice"))).await, "served");
    assert!(!n.node.is_superseded(&kh("alice")));

    // alice2 recovers alice's identity under bob, carol recognising the
    // holder; bob originates the adoption as the countersigning patron
    let rec = s.recover("alice", "alice2", "bob", "carol", ("bob", &[1], 2));
    assert_eq!(n.originate_transaction(&rec.bytes), Decision::Stored);
    assert!(n.view.lock().unwrap().is_superseded(&kh("alice")), "the table knows");
    assert!(n.node.is_superseded(&kh("alice")), "so does the serving state, with no separate call");
    assert!(until(3000, || !n.node.has_session(&kh("alice"))).await, "alice's session ended");
    assert!(matches!(attach(&acfg, &client_ep(), kh("bob"), n.addr, false).await, AttachOutcome::Refused), "a fresh attach as alice is refused");
    assert_eq!(n.node.enqueue(kh("alice"), b"late".to_vec()), Err(rhtn_transport::queue::Refusal::Superseded));

    // a node starting from the store that holds the recovery knows it
    // before its first session
    let dir = std::env::temp_dir().join(format!("rhtn-restart-sup-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    n.view.lock().unwrap().store.save(&dir).unwrap();
    let mut v2 = view_of("bob", table_of("bob", &s, &[&a_a, &rec], &infra), "bob", &[], s.clock);
    v2.store = TopologyStore::load(&dir).unwrap();
    let n2 = LiveNode::start(node_cfg("bob", I), v2, ids(), anchors());
    assert!(n2.node.is_superseded(&kh("alice")), "seeded from the store at start");
    assert!(matches!(attach(&acfg, &client_ep(), kh("bob"), n2.addr, false).await, AttachOutcome::Refused));
    let _ = std::fs::remove_dir_all(&dir);
}

// acceptance: RES-17
#[tokio::test]
async fn a_restarted_node_advances_its_endpoint_counter_only_for_a_new_address() {
    let mut s = Signers::new();
    let a_n = s.adopt("bob", "alice", "alice", &[0], 1);
    let records = [&a_n];
    let infra = ["alice", "bob"];
    // first life: the first record opens at counter 1, one past the
    // adoption's counter 0
    let n1 = LiveNode::start(node_cfg("bob", I), view_of("bob", table_of("bob", &s, &records, &infra), "alice", &[0], s.clock), ids(), anchors());
    let first = n1.view.lock().unwrap().store.endpoint(&kh("bob")).expect("published at start").clone();
    assert_eq!((first.seqno.series, first.seqno.counter), (1, 1));
    assert_eq!(n1.view.lock().unwrap().position.seqno.counter, 1, "the position advanced with it");
    let dir = std::env::temp_dir().join(format!("rhtn-restart-ep-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    n1.view.lock().unwrap().store.save(&dir).unwrap();

    // second life on a fresh loopback port, from the saved store: the held
    // record names the old address, so the new one takes the next counter
    let mut v2 = view_of("bob", table_of("bob", &s, &records, &infra), "alice", &[0], s.clock);
    v2.store = TopologyStore::load(&dir).unwrap();
    let n2 = LiveNode::start(node_cfg("bob", I), v2, ids(), anchors());
    assert_ne!(n2.addr, n1.addr);
    let second = n2.view.lock().unwrap().store.endpoint(&kh("bob")).unwrap().clone();
    assert_eq!((second.seqno.series, second.seqno.counter), (1, 2), "a changed address advances the counter");
    assert_eq!(n2.view.lock().unwrap().position.seqno.counter, 2);
    let own: Vec<std::net::SocketAddr> = n2.view.lock().unwrap().own_endpoints().iter().map(|e| e.socket()).collect();
    assert_eq!(own, vec![n2.addr], "the current address is what the store serves");

    // an unchanged list replays the record already held, spending nothing
    let point = NetworkPoint::from_socket(n2.addr).unwrap();
    let again = n2.view.lock().unwrap().publish_own_endpoints(&kh("alice"), &[point]).unwrap();
    assert_eq!(again, second.bytes);
    assert_eq!(n2.view.lock().unwrap().position.seqno.counter, 2);
    let _ = std::fs::remove_dir_all(&dir);
}

// acceptance: CUR-18
#[tokio::test]
async fn a_running_node_asks_currency_up_its_session_and_takes_the_answer() {
    // P (alice) root over N (bob) at index 0 and X (carol) at index 1; N
    // holds a session upstream to P, and asks P about X
    let mut s = Signers::new();
    let a_n = s.adopt("bob", "alice", "alice", &[0], 1);
    let a_x = s.adopt("carol", "alice", "alice", &[1], 2);
    let records = [&a_n, &a_x];
    let infra = ["alice", "bob"];
    let mut p_view = view_of("alice", table_of("alice", &s, &records, &infra), "alice", &[], s.clock);
    p_view.set_slot(0, Some(kh("bob")), s.clock);
    p_view.set_slot(1, Some(kh("carol")), s.clock);
    let p = LiveNode::start(node_cfg("alice", I), p_view, ids(), anchors());
    let n = LiveNode::start(node_cfg("bob", I), view_of("bob", table_of("bob", &s, &records, &infra), "alice", &[0], s.clock), ids(), anchors());
    let ncli = client_cfg("bob");
    know(&ncli, "alice", p.addr);
    let AttachOutcome::Attached(_up) = n.attach_upstream(&ncli, kh("alice")).await else { panic!("N attaches to P") };
    assert!(!n.view.lock().unwrap().staples.contains_key(&kh("carol")));
    let ask = n.view.lock().unwrap().ask_currency(&n.adjacency, kh("carol"), None, Some(kh("alice")), [8; 16]).expect("asked up the session");
    assert_eq!(ask.asked, vec![kh("alice")]);
    assert!(until(4000, || n.view.lock().unwrap().staples.contains_key(&kh("carol"))).await, "P's attestation came back on the request stream and is N's staple");
    assert!(n.view.lock().unwrap().asks.is_empty(), "the ask is settled");
    // P serves N and holds no session as N's client, so P cannot ask N
    assert!(p.view.lock().unwrap().ask_currency(&p.adjacency, kh("carol"), Some(kh("bob")), None, [9; 16]).is_none());
}
