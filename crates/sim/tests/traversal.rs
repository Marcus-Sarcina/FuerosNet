//! The direct payload path over real sockets (design §14.1.1, §12.6.3;
//! `wire-format.md` §9.2; `infra-client-requirements.md` §7): two leaves
//! behind emulated NATs, their serving nodes as STUN, candidates
//! exchanged through the relay, the direct path attempted first and the
//! relay taken when it fails, control traffic never traversing, and
//! nothing outside the horizon.

mod common;

use common::*;
use rhtn_node::peering::PathOverride;
use rhtn_node::resolution::{AnchorTable, Ingestion};
use rhtn_node::runtime::{LiveDelivery, LiveNode};
use rhtn_sim::nat::{Filtering, Mapping, Nat};
use rhtn_transport::session::*;
use rhtn_transport::traversal::{CandidateKind, decode_candidates, encode_candidates};
use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::time::Duration;

const I: u64 = 30;

/// Root alice (S1) with infra child bob (S2, index 1); leaves carol under
/// alice (index 0) and w1 under bob (index 1, 0): siblings' children, two
/// edges apart, inside each other's h = 2 horizon.  Far away, w2 under c2
/// under c1, another root.
struct Scene {
    s: Signers,
    s1: Arc<LiveNode>,
    s2: Arc<LiveNode>,
    l1: Arc<LiveNode>,
    l2: Arc<LiveNode>,
    l1_up: Session,
    l2_up: Session,
    nat1: Arc<Nat>,
    nat2: Arc<Nat>,
}

async fn scene(mapping: Mapping, filtering: Filtering) -> Scene {
    let mut s = Signers::new();
    let a_s2 = s.adopt("bob", "alice", "alice", &[1], 1);
    let a_l1 = s.adopt("carol", "alice", "alice", &[0], 2);
    let a_l2 = s.adopt("w1", "bob", "alice", &[1, 0], 3);
    let a_p2 = s.adopt("c2", "c1", "c1", &[0], 4);
    let a_l3 = s.adopt("w2", "c2", "c1", &[0, 0], 5);
    let records = [&a_s2, &a_l1, &a_l2, &a_p2, &a_l3];
    let infra = ["alice", "bob", "c1", "c2"];
    let mut v1 = view_of(
        "alice",
        table_of("alice", &s, &records, &infra),
        "alice",
        &[],
        s.clock,
    );
    v1.set_slot(1, Some(kh("bob")), s.clock);
    v1.set_slot(0, Some(kh("carol")), s.clock);
    let s1 = LiveNode::start(
        node_cfg("alice", I),
        v1,
        ids(),
        AnchorTable::new(0, Ingestion::UnverifiedGossip),
    );
    let mut v2 = view_of(
        "bob",
        table_of("bob", &s, &records, &infra),
        "alice",
        &[1],
        s.clock,
    );
    v2.set_slot(0, Some(kh("w1")), s.clock);
    let s2 = LiveNode::start(
        node_cfg("bob", I),
        v2,
        ids(),
        AnchorTable::new(0, Ingestion::UnverifiedGossip),
    );
    let nat1 = Nat::start(mapping, filtering).await.unwrap();
    let nat2 = Nat::start(mapping, filtering).await.unwrap();
    let mut c1 = node_cfg("carol", I);
    c1.nat = Some(nat1.inside);
    let mut lv1 = view_of(
        "carol",
        table_of("carol", &s, &records, &infra),
        "alice",
        &[0],
        s.clock,
    );
    lv1.serving_node = Some(kh("alice"));
    let l1 = LiveNode::start(
        c1,
        lv1,
        ids(),
        AnchorTable::new(0, Ingestion::UnverifiedGossip),
    );
    let mut c2 = node_cfg("w1", I);
    c2.nat = Some(nat2.inside);
    let mut lv2 = view_of(
        "w1",
        table_of("w1", &s, &records, &infra),
        "alice",
        &[1, 0],
        s.clock,
    );
    lv2.serving_node = Some(kh("bob"));
    let l2 = LiveNode::start(
        c2,
        lv2,
        ids(),
        AnchorTable::new(0, Ingestion::UnverifiedGossip),
    );
    // each leaf attaches to its serving node through its NAT: an outward
    // dial, and the session's address is what the leaf will ask STUN at
    let cfg1 = client_cfg("carol");
    know(&cfg1, "alice", s1.addr);
    let AttachOutcome::Attached(l1_up) = l1.attach_upstream(&cfg1, kh("alice")).await else {
        panic!("L1 attaches to S1")
    };
    let cfg2 = client_cfg("w1");
    know(&cfg2, "bob", s2.addr);
    let AttachOutcome::Attached(l2_up) = l2.attach_upstream(&cfg2, kh("bob")).await else {
        panic!("L2 attaches to S2")
    };
    Scene {
        s,
        s1,
        s2,
        l1,
        l2,
        l1_up,
        l2_up,
        nat1,
        nat2,
    }
}

/// Carry candidates from `from` to `to` the way the harness carries every
/// message between leaves that hold no direct path: through the
/// recipient's serving node, which queues or delivers.
fn relay(sc: &Scene, to: &str, bytes: Vec<u8>) {
    let node = if to == "carol" { &sc.s1 } else { &sc.s2 };
    node.node
        .enqueue(kh(to), bytes)
        .expect("the serving node relays");
}

async fn next_delivery(s: &mut Session) -> Option<Vec<u8>> {
    tokio::time::timeout(Duration::from_secs(3), s.deliveries.recv())
        .await
        .ok()
        .flatten()
}

// acceptance: TRV-01
#[tokio::test]
async fn candidates_carry_the_host_address_and_the_reflexive_address_the_serving_node_saw() {
    let sc = scene(Mapping::EndpointIndependent, Filtering::AddressDependent).await;
    assert_eq!(
        sc.s1.traversal.answered.load(Ordering::SeqCst),
        0,
        "nothing asked yet"
    );
    let cs = sc.l1.gather().await;
    assert_eq!(
        sc.l1.traversal.asked.load(Ordering::SeqCst),
        1,
        "one Binding Request to S1"
    );
    assert_eq!(
        sc.s1.traversal.answered.load(Ordering::SeqCst),
        1,
        "answered at the address it serves QUIC on"
    );
    assert_eq!(
        cs.iter().map(|c| c.kind).collect::<Vec<_>>(),
        vec![CandidateKind::Host, CandidateKind::ServerReflexive]
    );
    assert_eq!(
        cs[0].addr, sc.l1.addr,
        "the host address is the socket's own"
    );
    assert_ne!(cs[1].addr, sc.l1.addr);
    // The mapping is that socket's own, asked for by inside address: the
    // NAT holds one per inside socket here, and this node has a second
    // socket behind the same NAT for its outward dials, so the map's own
    // order says nothing about which is which.
    assert_eq!(
        sc.nat1.mappings_for(sc.l1.addr),
        vec![cs[1].addr],
        "endpoint-independent: one mapping for that socket, and it is the reflexive address"
    );
    assert_eq!(decode_candidates(&encode_candidates(&cs)).unwrap(), cs);
    let _ = &sc.s;
}

/// The whole exchange: each leaf gathers, hands its candidates to the
/// other through the relay, and both open the direct path at once.
async fn exchange_and_open(sc: &mut Scene) -> (bool, bool) {
    let c1 = sc.l1.gather().await;
    let c2 = sc.l2.gather().await;
    relay(sc, "w1", encode_candidates(&c1));
    relay(sc, "carol", encode_candidates(&c2));
    let got2 = decode_candidates(
        &next_delivery(&mut sc.l2_up)
            .await
            .expect("L2 receives L1's candidates"),
    )
    .unwrap();
    let got1 = decode_candidates(
        &next_delivery(&mut sc.l1_up)
            .await
            .expect("L1 receives L2's candidates"),
    )
    .unwrap();
    assert_eq!((got1, got2), (c2.clone(), c1.clone()));
    let p = pins();
    let (a, b) = tokio::join!(
        sc.l1.open_direct(kh("w1"), &p, &c2),
        sc.l2.open_direct(kh("carol"), &p, &c1)
    );
    (a, b)
}

// acceptance: TRV-02
#[tokio::test]
async fn the_direct_path_is_attempted_first_and_the_relay_taken_when_the_checks_fail() {
    // endpoint-independent NATs: a candidate pair connects
    let mut sc = scene(Mapping::EndpointIndependent, Filtering::AddressDependent).await;
    let (a, b) = exchange_and_open(&mut sc).await;
    assert!(a || b, "a connection completes on a candidate pair");
    let mut inbox2 = sc.l2.take_direct_deliveries();
    let mut inbox1 = sc.l1.take_direct_deliveries();
    let (sender, receiver, inbox) = if a {
        (&sc.l1, "w1", &mut inbox2)
    } else {
        (&sc.l2, "carol", &mut inbox1)
    };
    let relayed = std::cell::Cell::new(0);
    let d = sender
        .send_payload_live(kh(receiver), b"direct".to_vec(), &|_| {
            relayed.set(relayed.get() + 1)
        })
        .await;
    assert_eq!(d, LiveDelivery::Direct);
    let (from, bytes) = tokio::time::timeout(Duration::from_secs(3), inbox.recv())
        .await
        .unwrap()
        .unwrap();
    assert_eq!((from, bytes.as_slice()), (sender.me(), &b"direct"[..]));
    assert_eq!(relayed.get(), 0);
    assert_eq!(
        sc.s1.node.queued(&kh("carol")) + sc.s2.node.queued(&kh("w1")),
        0,
        "neither serving node carries the bytes"
    );
    drop(sc);
    // address-and-port-dependent NATs: the checks fail within the bound and the relay carries it
    let mut sc = scene(
        Mapping::AddressAndPortDependent,
        Filtering::AddressAndPortDependent,
    )
    .await;
    let (a, b) = exchange_and_open(&mut sc).await;
    assert!(!a && !b, "no candidate pair connects");
    assert_eq!(sc.l1.direct_state(&kh("w1")), Some(false));
    let d = sc
        .l1
        .send_payload_live(kh("w1"), b"relayed".to_vec(), &|bytes| {
            relay(&sc, "w1", bytes)
        })
        .await;
    assert_eq!(d, LiveDelivery::Relayed);
    assert_eq!(
        next_delivery(&mut sc.l2_up).await.as_deref(),
        Some(&b"relayed"[..]),
        "delivered through L2's serving node"
    );
}

// acceptance: TRV-03
#[tokio::test]
async fn the_relay_carries_every_message_in_order_while_the_direct_path_stays_defeated() {
    let mut sc = scene(
        Mapping::AddressAndPortDependent,
        Filtering::AddressAndPortDependent,
    )
    .await;
    let (a, b) = exchange_and_open(&mut sc).await;
    assert!(!a && !b);
    let asked = sc.l1.traversal.asked.load(Ordering::SeqCst);
    let started = tokio::time::Instant::now();
    for i in 0..5 {
        let d = sc
            .l1
            .send_payload_live(kh("w1"), format!("m{i}").into_bytes(), &|bytes| {
                relay(&sc, "w1", bytes)
            })
            .await;
        assert_eq!(d, LiveDelivery::Relayed);
    }
    assert!(
        started.elapsed() < Duration::from_secs(1),
        "no send waited on a retry of the direct path"
    );
    for i in 0..5 {
        assert_eq!(
            next_delivery(&mut sc.l2_up).await,
            Some(format!("m{i}").into_bytes()),
            "in order"
        );
    }
    assert_eq!(
        sc.l1.traversal.asked.load(Ordering::SeqCst),
        asked,
        "no gathering between sends"
    );
    assert_eq!(
        sc.l1.direct_state(&kh("w1")),
        Some(false),
        "the failure stands until the next opening"
    );
}

// acceptance: TRV-04
#[tokio::test]
async fn control_traffic_dials_outward_and_never_traverses() {
    let sc = scene(
        Mapping::AddressAndPortDependent,
        Filtering::AddressAndPortDependent,
    )
    .await;
    // attached through the NAT, heartbeating; a resolution and a currency
    // ask go up the session
    tokio::time::sleep(Duration::from_millis(300)).await;
    let _ = sc.l1_up.outbound.send((3, vec![]));
    sc.l1.view.lock().unwrap().require_currency(
        &sc.l1.adjacency,
        &ids(),
        &kh("w1"),
        None,
        Some(kh("alice")),
    );
    tokio::time::sleep(Duration::from_millis(300)).await;
    assert_eq!(
        sc.l1.traversal.asked.load(Ordering::SeqCst),
        0,
        "no STUN request left L1"
    );
    assert_eq!(
        sc.s1.traversal.answered.load(Ordering::SeqCst),
        0,
        "S1 answered none"
    );
    assert_eq!(
        sc.l2.traversal.asked.load(Ordering::SeqCst)
            + sc.s2.traversal.answered.load(Ordering::SeqCst),
        0
    );
    assert!(
        sc.nat1.forwarded.load(Ordering::SeqCst) > 0,
        "the session itself crossed the NAT on the outward dial"
    );
}

// acceptance: TRV-05
#[tokio::test]
async fn nothing_is_gathered_and_no_address_dialled_for_a_peer_outside_the_horizon() {
    let sc = scene(Mapping::EndpointIndependent, Filtering::EndpointIndependent).await;
    let far = kh("w2");
    assert!(
        !sc.l1
            .view
            .lock()
            .unwrap()
            .table
            .horizon(&kh("carol"), 2)
            .contains(&far)
    );
    assert!(
        sc.l1.prepare_direct(&far).await.is_none(),
        "no candidates for a peer outside the horizon"
    );
    assert_eq!(
        sc.l1.traversal.asked.load(Ordering::SeqCst),
        0,
        "no STUN request"
    );
    assert_eq!(sc.l1.direct_state(&far), None, "nothing dialled");
    let d = sc.l1.send_payload_live(far, b"far".to_vec(), &|_| {}).await;
    assert_eq!(d, LiveDelivery::Relayed);
    // and inside the horizon the same call gathers
    assert!(sc.l1.prepare_direct(&kh("w1")).await.is_some());
    assert_eq!(sc.l1.traversal.asked.load(Ordering::SeqCst), 1);
}

// acceptance: TRV-06
#[tokio::test]
async fn the_reflexive_addresses_travel_only_between_the_two_leaves() {
    let mut sc = scene(Mapping::EndpointIndependent, Filtering::AddressDependent).await;
    let (a, b) = exchange_and_open(&mut sc).await;
    assert!(a || b);
    let refl: Vec<Vec<u8>> = [&sc.nat1, &sc.nat2]
        .iter()
        .flat_map(|n| n.mappings())
        .map(|a| match a.ip() {
            std::net::IpAddr::V4(ip) => {
                let mut v = ip.octets().to_vec();
                v.extend_from_slice(&a.port().to_be_bytes());
                v
            }
            _ => unreachable!(),
        })
        .collect();
    tokio::time::sleep(Duration::from_millis(300)).await;
    for node in [&sc.s1, &sc.s2] {
        let replicated: Vec<Vec<u8>> = node
            .replication_payload()
            .into_iter()
            .map(|r| format!("{r:?}").into_bytes())
            .collect();
        let v = node.view.lock().unwrap();
        let mut state: Vec<Vec<u8>> = v.store.objects().into_iter().map(|(_, o)| o).collect();
        state.extend(
            v.own_endpoints()
                .into_iter()
                .map(|p| format!("{p:?}").into_bytes()),
        );
        state.extend(replicated);
        for bytes in &state {
            for r in &refl {
                assert!(
                    !bytes.windows(r.len()).any(|w| w == r.as_slice()),
                    "a reflexive address in topology state at a serving node"
                );
            }
        }
    }
    let _ = PathOverride::None;
}
