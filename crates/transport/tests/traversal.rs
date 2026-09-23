//! The direct path's connectivity checks (design §14.1.1, which defers
//! to RFC 8445): the peer's candidates dialled in the check list's order,
//! one started per pacing interval unless the one before has ended, never
//! two in one instant.  Real loopback sockets: a live peer and two that
//! swallow packets without answering, so a check against them neither
//! completes nor fails inside the test's window.

use rhtn_crypto::identity::testkit::test_identity;
use rhtn_transport::bind::Binding;
use rhtn_transport::session::{Event, Log};
use rhtn_transport::tls::{self, Pins};
use rhtn_transport::traversal::{Candidate, CandidateKind, TA, check_list, connect_direct};
use std::net::SocketAddr;
use std::time::Duration;

fn loopback() -> SocketAddr {
    "127.0.0.1:0".parse().unwrap()
}

/// A socket that takes packets and answers nothing: a candidate whose
/// check neither completes nor fails.
fn black_hole() -> (std::net::UdpSocket, SocketAddr) {
    let s = std::net::UdpSocket::bind(loopback()).unwrap();
    let a = s.local_addr().unwrap();
    (s, a)
}

fn dials(log: &Log) -> Vec<(SocketAddr, u64)> {
    log.events()
        .into_iter()
        .filter_map(|(_, e)| match e {
            Event::Dialled { addr, at_ms } => Some((addr, at_ms)),
            _ => None,
        })
        .collect()
}

// acceptance: TRV-11
#[tokio::test]
async fn trv_11_candidate_pairs_are_checked_in_the_rfcs_order_and_at_its_pace() {
    // the check list orders by priority: the host candidates before the
    // server-reflexive one, ties in the order offered, one address once
    let (hole_a, a) = black_hole();
    let (hole_b, b) = black_hole();
    let sep = tls::server_endpoint(test_identity("bob"), loopback()).unwrap();
    let live = sep.local_addr().unwrap();
    tokio::spawn(async move {
        while let Some(inc) = sep.accept().await {
            tokio::spawn(async move {
                if let Ok(c) = inc.await {
                    tokio::time::sleep(Duration::from_secs(5)).await;
                    drop(c);
                }
            });
        }
    });
    let offered = vec![
        Candidate {
            kind: CandidateKind::ServerReflexive,
            addr: b,
        },
        Candidate {
            kind: CandidateKind::Host,
            addr: a,
        },
        Candidate {
            kind: CandidateKind::Host,
            addr: live,
        },
        Candidate {
            kind: CandidateKind::Host,
            addr: a,
        },
    ];
    let list: Vec<SocketAddr> = check_list(&offered).iter().map(|c| c.addr).collect();
    assert_eq!(
        list,
        vec![a, live, b],
        "hosts first, in offered order, once each"
    );

    // the initiator runs its checks: the first dial is the highest
    // priority pair; the live host is dialled only once Ta has passed on
    // the black hole before it; the handshake with it completes and the
    // reflexive candidate is never dialled
    let pins = Pins::new();
    pins.pin_identity(&test_identity("bob").public);
    let ep = tls::client_endpoint(loopback()).unwrap();
    let log = Log::recording();
    let conn = connect_direct(
        &ep,
        &test_identity("alice"),
        &pins,
        &Binding::default(),
        &test_identity("bob").public.keyhash,
        &offered,
        Duration::from_secs(5),
        &log,
    )
    .await
    .expect("the live candidate connects");
    assert_eq!(conn.remote_address(), live);
    let d = dials(&log);
    assert_eq!(
        d.iter().map(|(x, _)| *x).collect::<Vec<_>>(),
        vec![a, live],
        "the highest-priority pair first, the live one after it, the reflexive one never: {d:?}"
    );
    assert!(
        d[1].1 >= d[0].1 + TA.as_millis() as u64,
        "a lower-priority pair is dialled only after the pacing interval: {d:?}"
    );
    assert!(d[1].1 > d[0].1, "no two pairs in the same instant");
    drop((hole_a, hole_b));

    // and an implementation that dialled all three at once would show
    // three dials at one instant: here every dial has a later time than
    // the one before
    let times: Vec<u64> = d.iter().map(|(_, t)| *t).collect();
    assert!(times.windows(2).all(|w| w[1] > w[0]));
}

/// A check that fails at once lets the next start without waiting for Ta:
/// a closed port answers unreachable and the check ends.
#[tokio::test]
async fn a_check_that_ends_early_lets_the_next_start_before_the_interval() {
    let (hole, closed) = black_hole();
    drop(hole); // the port is closed: a dial there ends with a refusal
    let sep = tls::server_endpoint(test_identity("bob"), loopback()).unwrap();
    let live = sep.local_addr().unwrap();
    tokio::spawn(async move {
        while let Some(inc) = sep.accept().await {
            tokio::spawn(async move {
                if let Ok(c) = inc.await {
                    tokio::time::sleep(Duration::from_secs(5)).await;
                    drop(c);
                }
            });
        }
    });
    let offered = vec![
        Candidate {
            kind: CandidateKind::Host,
            addr: closed,
        },
        Candidate {
            kind: CandidateKind::Host,
            addr: live,
        },
    ];
    let pins = Pins::new();
    pins.pin_identity(&test_identity("bob").public);
    let ep = tls::client_endpoint(loopback()).unwrap();
    let log = Log::recording();
    let conn = connect_direct(
        &ep,
        &test_identity("alice"),
        &pins,
        &Binding::default(),
        &test_identity("bob").public.keyhash,
        &offered,
        Duration::from_secs(5),
        &log,
    )
    .await;
    assert!(conn.is_some());
    let d = dials(&log);
    assert_eq!(
        d.iter().map(|(x, _)| *x).collect::<Vec<_>>(),
        vec![closed, live]
    );
    // whichever came first, the end of the failed check or Ta, the second
    // dial waited for no longer than the interval
    assert!(d[1].1 <= d[0].1 + TA.as_millis() as u64 + 20, "{d:?}");
}
