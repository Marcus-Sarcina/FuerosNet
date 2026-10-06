//! Reaching a node that does not serve this client (design §12.6.3;
//! `wire-format.md` §7.7, §7.10).
//!
//! **A light client does not need its patron or its serving node to
//! address a distant node for which it holds a locator** [author,
//! 2026-10-05].  Every party here is real: two running nodes, a client
//! that is attached to neither of them, and the submission on the wire
//! between it and the node that serves the recipient.

mod common;

use common::*;
use rhtn_adaptors::beyond::{Beyond, Beyonder};
use rhtn_archive::Keyhash;
use rhtn_archive::submission::unrelayed;
use rhtn_archive::tx::{Locator, Seqno};
use rhtn_node::resolution::{AnchorTable, Ingestion, Path};
use rhtn_node::runtime::LiveNode;
use rhtn_node::view::NodeView;
use rhtn_transport::session::*;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

/// A node for `name` that serves exactly `clients` and nobody else.
///
/// **The served set is the operator's** (`infra-client-requirements.md`
/// §2): a node queues for a party it has a record of and refuses one it
/// has not, and a fixture serving everyone could not tell the two apart.
fn node_serving(name: &str, clients: &[&str]) -> Arc<LiveNode> {
    let p = Path::from_indices(&[]);
    let view = NodeView::new(
        Arc::new(id(name)),
        Locator {
            anchor: kh(name),
            path: p.bytes,
            nibbles: p.nibbles,
            seqno: Seqno {
                series: 1,
                counter: 0,
            },
        },
    );
    let mut cfg = NodeConfig::defaults(Arc::new(id(name)), pins(), 30);
    cfg.log = Log::recording();
    let served: Vec<Keyhash> = clients.iter().map(|c| kh(c)).collect();
    cfg.serves = Arc::new(move |k| served.contains(k));
    LiveNode::start(
        cfg,
        view,
        ids(),
        AnchorTable::new(0, Ingestion::UnverifiedGossip),
    )
}

/// The reach of a party this client can already place: the node that
/// serves them, at `at`, with nothing to resolve.
fn known(node: &str, at: &[SocketAddr]) -> rhtn_client::horizon::Reach {
    rhtn_client::horizon::Reach {
        upstream: rhtn_client::horizon::Upstream::Known {
            node: kh(node),
            endpoints: at
                .iter()
                .map(|a| NetworkPoint::new([127, 0, 0, 1], Some(a.port() as u64)).encode_bytes())
                .collect(),
            key_material: None,
        },
        ask: Vec::new(),
    }
}

/// A view of the network beyond, for `name`, dialling from its own
/// endpoint with its own pins.
fn beyond(name: &str) -> Arc<Beyond> {
    let dialler = Beyonder::new(client_ep(), client_cfg(name));
    Beyond::new(dialler, Arc::new(|| [5; 16]))
}

/// A light client attached to `node` over the wire: the session, which
/// the caller holds for as long as it wants the connection, and the
/// deliveries taken out of it.
async fn collecting(
    name: &str,
    node: &Arc<LiveNode>,
) -> (Session, tokio::sync::mpsc::UnboundedReceiver<Vec<u8>>) {
    let cfg = client_cfg(name);
    cfg.addresses
        .lock()
        .unwrap()
        .entry(node.me())
        .or_default()
        .push(node.addr);
    let AttachOutcome::Attached(mut s) =
        attach(&cfg, &client_ep(), node.me(), node.addr, false).await
    else {
        panic!("{name} attaches")
    };
    let deliveries = std::mem::replace(&mut s.deliveries, tokio::sync::mpsc::unbounded_channel().1);
    (s, deliveries)
}

// acceptance: PAY-24
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn payload_goes_to_the_node_that_serves_the_recipient_not_the_sender_s_own() {
    let bob = node_serving("bob", &["alice"]);
    let carol = node_serving("carol", &["w1"]);
    let alice = beyond("alice");

    // **the counterfactual first.** w1 is carol's client, so bob holds no
    // record of it: handed this submission, bob would refuse it, which is
    // what a sender routed through its own node is left with
    assert!(
        !alice
            .carry(
                kh("w1"),
                [0; 32],
                b"sealed".to_vec(),
                known("bob", &[bob.addr]),
                None,
            )
            .await,
        "a node that serves nobody by that name does not take it"
    );
    assert_eq!(bob.node.queued(&kh("w1")), 0);

    // and the same payload, handed to the node that does serve w1
    assert!(
        alice
            .carry(
                kh("w1"),
                [0; 32],
                b"sealed".to_vec(),
                known("carol", &[carol.addr]),
                Some(kh("bob")),
            )
            .await,
        "carol takes what is for its own client"
    );
    assert_eq!(carol.node.queued(&kh("w1")), 1, "taken, and waiting");

    // **alice is attached to neither node.** Nothing in this test opened
    // a session with a serving node, which is the whole of the point: the
    // dial is request-only (`wire-format.md` §8.2, §9.1) and no attach
    // precedes it.
    let (_held, mut w1) = collecting("w1", &carol).await;
    let delivered = tokio::time::timeout(Duration::from_secs(3), w1.recv())
        .await
        .expect("delivered")
        .expect("bytes");
    let (who, what, _) = unrelayed(&delivered).expect("a relayed payload");
    assert_eq!(
        (who, what),
        (kh("alice"), b"sealed".to_vec()),
        "alice named in front of her own ciphertext, by the node that took it"
    );
    assert_eq!(carol.node.queued(&kh("w1")), 0, "no copy outlives it");
}

// acceptance: PAY-25
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn the_node_already_serving_this_client_is_not_dialled_again() {
    let carol = node_serving("carol", &["w1"]);
    let alice = beyond("alice");

    // carol serves w1 *and* this client: there is a session up already,
    // and the caller's next term is the one that uses it
    assert!(
        !alice
            .carry(
                kh("w1"),
                [0; 32],
                b"sealed".to_vec(),
                known("carol", &[carol.addr]),
                Some(kh("carol")),
            )
            .await,
        "the answer is no, so the caller falls through to its serving node"
    );
    assert_eq!(
        carol.node.queued(&kh("w1")),
        0,
        "and nothing was dialled or submitted"
    );
}

// acceptance: RES-19
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_node_s_endpoints_are_alternatives_and_a_dead_first_one_is_not_an_outage() {
    let carol = node_serving("carol", &["w1"]);
    let alice = beyond("alice");
    // a port nothing answers on, ahead of the one that does: §7.7.3
    // requires the list be treated as alternatives, or a single
    // unreachable first entry becomes a permanent outage for that node
    let dead: SocketAddr = "127.0.0.1:9".parse().unwrap();
    assert!(
        alice
            .carry(
                kh("w1"),
                [0; 32],
                b"sealed".to_vec(),
                known("carol", &[dead, carol.addr]),
                Some(kh("bob")),
            )
            .await,
        "the second endpoint answered"
    );
    assert_eq!(carol.node.queued(&kh("w1")), 1);
}
