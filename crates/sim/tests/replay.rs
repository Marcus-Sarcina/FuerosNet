//! The replay half of TRN-16, which milestone 2 deferred: a client's 0-RTT
//! first flight, captured off the path and replayed as a second connection
//! whose handshake never completes.

mod common;

use common::*;
use rhtn_sim::path::Path;
use rhtn_sim::scenario::Running;
use rhtn_transport::session::*;
use std::time::Duration;

const I: u64 = 30;

fn ack_at(log: &Log) -> Option<tokio::time::Instant> {
    log.events().into_iter().find(|(_, e)| matches!(e, Event::Received { frame_type: 2 })).map(|(t, _)| t)
}

fn handshake_at(log: &Log) -> Option<tokio::time::Instant> {
    log.events().into_iter().find(|(_, e)| matches!(e, Event::HandshakeDone { .. })).map(|(t, _)| t)
}

// acceptance: TRN-16
#[tokio::test]
async fn a_replayed_first_flight_binds_nothing() {
    let s = Running::start(node_cfg("alice", I));
    let path = Path::open(s.addr).await.unwrap();
    let cfg = client_cfg("carol");
    know(&cfg, "alice", path.addr);

    // a completed session leaves C holding a resumption ticket
    let first = match attach(&cfg, &client_ep(), kh("alice"), path.addr, false).await {
        AttachOutcome::Attached(x) => x,
        other => panic!("{other:?}"),
    };
    assert_eq!(first.ack.mode, 0);
    first.conn.close(0u32.into(), b"done");
    drop(first);
    tokio::time::sleep(Duration::from_millis(200)).await;

    // S holds a queued item for C
    let item = b"queued for C".to_vec();
    s.node.enqueue(kh("carol"), item.clone()).unwrap();
    assert_eq!(s.node.queued(&kh("carol")), 1);
    let attaches_before = s.node.log.count(|e| matches!(e, Event::Attached { .. }));

    // connection A carries the Attach as 0-RTT early data, and the path
    // records every datagram the client sends
    path.to_server.capture(true);
    let ep = client_ep();
    let mut a = match attach(&cfg, &ep, kh("alice"), path.addr, true).await {
        AttachOutcome::Attached(x) => x,
        other => panic!("{other:?}"),
    };
    path.to_server.capture(false);
    // the Attach rode early data, and A's handshake then completed
    assert_eq!(a.log.count(|e| *e == Event::EarlyDataSent), 1, "the Attach rode 0-RTT early data");
    let hs = handshake_at(&a.log).expect("A's handshake completed");
    // the early-data packets are what the client sent before that instant
    let flight = path.to_server.captured_before(hs);
    assert!(!flight.is_empty(), "the first flight was recorded");
    assert!(flight.len() < path.to_server.captured().len(), "and the 1-RTT packets that followed the handshake are not part of it");

    // on A, nothing was acted on before the handshake of the connection
    // that carried the early data completed
    let ack = ack_at(&a.log).expect("an AttachAck followed");
    assert!(ack >= hs, "no AttachAck before A's handshake completed");
    let delivered = drain(&mut a, 700).await;
    assert_eq!(delivered, vec![item.clone()], "the queued item is delivered on A alone");
    assert_eq!(s.node.queued(&kh("carol")), 0);

    // the recording is replayed as a second connection, from a fresh source
    // address, whose handshake nothing completes
    let attaches_after_a = s.node.log.count(|e| matches!(e, Event::Attached { .. }));
    assert_eq!(attaches_after_a, attaches_before + 1, "A bound once");
    let sent = path.replay_to_server(&flight).await.unwrap();
    assert_eq!(sent, flight.len());
    tokio::time::sleep(Duration::from_millis(600)).await;

    // B bound nothing: no further Attach was acted on, and nothing was
    // delivered under it
    assert_eq!(
        s.node.log.count(|e| matches!(e, Event::Attached { .. })),
        attaches_after_a,
        "no AttachAck is ever sent for the replay"
    );
    assert_eq!(s.node.queued(&kh("carol")), 0, "the queued item is not re-delivered");
    assert!(drain(&mut a, 300).await.is_empty(), "and nothing further arrives on A");
    // the deferral is what stands behind this: a server reads nothing
    // before the handshake of the connection that carried the early data,
    // so the replayed Attach was never read at all
    assert_eq!(s.node.log.count(|e| matches!(e, Event::Received { frame_type: 1 })), attaches_after_a, "B's Attach was never read");
}

/// The premise TRN-16 rests on, asserted rather than assumed: the second
/// dial to the same node really does carry its Attach as 0-RTT early data.
#[tokio::test]
async fn the_second_dial_carries_early_data() {
    let s = Running::start(node_cfg("alice", I));
    let cfg = client_cfg("carol");
    know(&cfg, "alice", s.addr);
    let first = match attach(&cfg, &client_ep(), kh("alice"), s.addr, false).await {
        AttachOutcome::Attached(x) => x,
        other => panic!("{other:?}"),
    };
    first.conn.close(0u32.into(), b"done");
    drop(first);
    tokio::time::sleep(Duration::from_millis(200)).await;
    let a = match attach(&cfg, &client_ep(), kh("alice"), s.addr, true).await {
        AttachOutcome::Attached(x) => x,
        other => panic!("{other:?}"),
    };
    assert_eq!(a.log.count(|e| *e == Event::EarlyDataSent), 1, "the Attach rode 0-RTT early data");
    let accepted = a.log.events().into_iter().any(|(_, e)| matches!(e, Event::HandshakeDone { early_accepted: true }));
    assert!(accepted, "and the server took it");
}
