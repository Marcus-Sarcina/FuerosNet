//! Reaching the node that serves the recipient (design §12.6.3;
//! `wire-format.md` §7.7, §7.8, §7.10, §9.1).
//!
//! **A light client does not need its patron or its serving node to
//! address a distant node for which it holds a locator** [author,
//! 2026-10-05].  Every party here is real: two running nodes, a client
//! that is attached to neither of them, and the fetch and the submission
//! on the wire between it and the node that serves the recipient, each on
//! a connection under a key minted for it.

mod common;

use common::*;
use rhtn_adaptors::beyond::{Beyond, Beyonder};
use rhtn_archive::Keyhash;
use rhtn_archive::node_key::{FAIL_NOT_SERVED, NodeKeyReply, REQUEST_NODE_KEY};
use rhtn_archive::submission::{
    REQUEST_RELAY, RelaySubmission, SUBMISSION_ACCEPTED, SUBMISSION_REFUSED, SubmissionReply,
    unrelayed,
};
use rhtn_archive::tx::{Locator, Seqno};
use rhtn_node::resolution::{AnchorTable, Ingestion, Path};
use rhtn_node::runtime::LiveNode;
use rhtn_node::view::NodeView;
use rhtn_transport::bind::Bound;
use rhtn_transport::session::*;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

/// The subnet the fixtures' positions are in (`wire-format.md` §2.3 field 4).
const SUBNET: [u8; 32] = [0xdb; 32];

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
            subnet: Some(SUBNET),
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

/// `name`'s connections bound to nobody: each a key minted for it.
fn minted_on(node: &Arc<LiveNode>) -> usize {
    node.node
        .log
        .count(|e| matches!(e, Event::Bound { how: Bound::Minted }))
}

/// Hand `sealed` for `to` to the node `reach` names, as nobody: the code
/// it answered with, its echo of the nonce checked.
async fn submit(b: &Beyond, to: &str, reach: rhtn_client::horizon::Reach, sealed: &[u8]) -> u64 {
    let nonce = [0x5a; 16];
    let body = RelaySubmission {
        recipient: kh(to),
        sealed: sealed.to_vec(),
        nonce,
    }
    .encode();
    let (_, reply) = b
        .minted(kh(to), reach, REQUEST_RELAY, body)
        .await
        .expect("the node answered");
    let r = SubmissionReply::decode(&reply).expect("a submission reply");
    assert_eq!(r.nonce, nonce, "the reply echoes the submission's nonce");
    r.code
}

// acceptance: PAY-24
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn payload_goes_to_the_node_that_serves_the_recipient_not_the_sender_s_own() {
    let bob = node_serving("bob", &["alice"]);
    let carol = node_serving("carol", &["w1"]);
    let alice = beyond("alice");

    // the key w1's node holds for it, fetched from that node as nobody
    let (up, reply) = alice
        .minted(
            kh("w1"),
            known("carol", &[carol.addr]),
            REQUEST_NODE_KEY,
            node_key_request("w1", [1; 16]),
        )
        .await
        .expect("carol answers");
    assert_eq!(up.node, kh("carol"));
    let sealed = seal_under("alice", "w1", &reply, b"for w1", None);

    // **the counterfactual.** w1 is carol's client, so bob holds no
    // record of it: asked for its key, bob has none to give, and handed
    // the seal, bob refuses it — which is what a sender routed through its
    // own node is left with
    let (_, refused) = alice
        .minted(
            kh("w1"),
            known("bob", &[bob.addr]),
            REQUEST_NODE_KEY,
            node_key_request("w1", [2; 16]),
        )
        .await
        .expect("bob answers");
    assert_eq!(
        NodeKeyReply::decode(&refused).unwrap().outcome,
        Err(FAIL_NOT_SERVED),
        "bob serves nobody by that name"
    );
    assert_eq!(
        submit(&alice, "w1", known("bob", &[bob.addr]), &sealed).await,
        SUBMISSION_REFUSED,
        "a node that serves nobody by that name does not take it"
    );
    assert_eq!(bob.node.queued(&kh("w1")), 0);

    // and the same seal, handed to the node that does serve w1
    assert_eq!(
        submit(&alice, "w1", known("carol", &[carol.addr]), &sealed).await,
        SUBMISSION_ACCEPTED,
        "carol takes what is for its own client"
    );
    assert_eq!(carol.node.queued(&kh("w1")), 1, "taken, and waiting");
    assert_eq!(bob.node.queued(&kh("w1")), 0, "and bob still holds nothing");

    // **alice is attached to neither node.** Nothing in this test opened
    // a session with a serving node, which is the whole of the point:
    // every ask went on a connection of its own, under a key minted for it
    // (`wire-format.md` §9.1), and no attach preceded any of them
    assert_eq!(
        minted_on(&carol),
        2,
        "the fetch and the submission at carol"
    );
    assert_eq!(minted_on(&bob), 2, "the fetch and the submission at bob");
    assert!(carol.node.sessions().is_empty() && bob.node.sessions().is_empty());

    let (_held, mut w1) = collecting("w1", &carol).await;
    let delivered = tokio::time::timeout(Duration::from_secs(3), w1.recv())
        .await
        .expect("delivered")
        .expect("bytes");
    let r = unrelayed(&delivered).expect("a relayed payload");
    assert_eq!(r.sealed, sealed, "exactly the seal alice handed over");
    assert_ne!(*r.ss_n, [0u8; 32], "with carol's half beside it");
    let body = open_as("w1", &kh("carol"), &delivered).expect("opens and its tag verifies");
    assert_eq!(
        (body.sender, body.bytes.as_slice()),
        (kh("alice"), b"for w1".as_slice()),
        "attributed to alice on opening, by w1 and not by the node"
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
    let (_, reply) = alice
        .minted(
            kh("w1"),
            known("carol", &[dead, carol.addr]),
            REQUEST_NODE_KEY,
            node_key_request("w1", [3; 16]),
        )
        .await
        .expect("the second endpoint answered the fetch");
    let sealed = seal_under("alice", "w1", &reply, b"sealed", None);
    assert_eq!(
        submit(&alice, "w1", known("carol", &[dead, carol.addr]), &sealed).await,
        SUBMISSION_ACCEPTED,
        "and the submission"
    );
    assert_eq!(carol.node.queued(&kh("w1")), 1);
}
