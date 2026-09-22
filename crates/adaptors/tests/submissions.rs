//! What a client hands the node serving it, over a session
//! (`wire-format.md` §7.10): its bundle, its one-time keys, payload to
//! carry, and where to be rung.  Every party here is real — a running
//! node, a light client attached to it over QUIC, and the four request
//! types on the wire between them.

mod common;

use common::*;
use rhtn_adaptors::attached::AttachedNode;
use rhtn_adaptors::serving::Serving;
use rhtn_archive::Keyhash;
use rhtn_archive::prekey::*;
use rhtn_archive::submission::*;
use rhtn_archive::tx::{Locator, Seqno};
use rhtn_node::resolution::{AnchorTable, Ingestion, Path};
use rhtn_node::runtime::LiveNode;
use rhtn_node::view::NodeView;
use rhtn_transport::session::*;
use std::sync::Arc;
use std::sync::atomic::{AtomicU8, Ordering};

/// A node for `name` whose configuration a test may adjust before it
/// starts: the served set and the prekey bounds are the two an operator
/// chooses and these tests exercise.
fn node_with(name: &str, tweak: impl FnOnce(&mut NodeConfig, &mut NodeView)) -> Arc<LiveNode> {
    let p = Path::from_indices(&[]);
    let mut view = NodeView::new(
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
    tweak(&mut cfg, &mut view);
    LiveNode::start(
        cfg,
        view,
        ids(),
        AnchorTable::new(0, Ingestion::UnverifiedGossip),
    )
}

/// A light client attached to `node` over the wire, as the serving node it
/// speaks to.  Nonces count up so a test can name the one it expects.
struct Light {
    serving: Arc<AttachedNode>,
    session: Arc<Session>,
    /// Taken out of the session before it was shared, since collecting
    /// what the node delivers needs the receiving end to itself.
    deliveries: tokio::sync::mpsc::UnboundedReceiver<Vec<u8>>,
    me: Keyhash,
}

async fn attach_light(name: &str, node: &Arc<LiveNode>) -> Light {
    let cfg = client_cfg(name);
    know(&cfg, "bob", node.addr);
    let AttachOutcome::Attached(mut s) =
        attach(&cfg, &client_ep(), node.me(), node.addr, false).await
    else {
        panic!("{name} attaches")
    };
    let deliveries = std::mem::replace(&mut s.deliveries, tokio::sync::mpsc::unbounded_channel().1);
    let session = Arc::new(s);
    let held = session.clone();
    let n = Arc::new(AtomicU8::new(1));
    let serving = AttachedNode::new(
        node.me(),
        Arc::new(move || Some(held.clone())),
        Arc::new(move || [n.fetch_add(1, Ordering::SeqCst); 16]),
    );
    Light {
        serving,
        session,
        deliveries,
        me: kh(name),
    }
}

fn bundle(name: &str) -> Vec<u8> {
    PrekeyBundle::build(
        &id(name),
        CONSTRUCTION_PQXDH,
        b"reusable material",
        1_800_000_000,
        &[0u8; 32],
    )
}

fn keys(n: usize) -> Vec<Vec<u8>> {
    (0..n).map(|i| format!("otk-{i}").into_bytes()).collect()
}

/// One submission by hand, so a test can see the reply's own code rather
/// than the accepted-or-not the trait returns.
async fn code(l: &Light, request_type: u64, nonce: [u8; 16], body: Vec<u8>) -> Option<u64> {
    l.serving.reply(request_type, nonce, body).await
}

// acceptance: SUB-01
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_publication_is_taken_only_from_the_subject_the_bundle_names() {
    let node = node_with("bob", |_, _| {});
    let carol = attach_light("carol", &node).await;
    // carol offers alice's bundle: the node it names is not the node that
    // sent it
    let n1 = [9u8; 16];
    let alices = PrekeyPublication {
        bundle: bundle("alice"),
        nonce: n1,
    }
    .encode();
    assert_eq!(
        code(&carol, REQUEST_PREKEY_PUBLICATION, n1, alices).await,
        Some(SUBMISSION_REFUSED)
    );
    assert!(
        node.view
            .lock()
            .unwrap()
            .prekeys
            .bundle(&kh("alice"))
            .is_none(),
        "nothing held for alice"
    );
    // carol's own is taken
    let n2 = [10u8; 16];
    let own = PrekeyPublication {
        bundle: bundle("carol"),
        nonce: n2,
    }
    .encode();
    assert_eq!(
        code(&carol, REQUEST_PREKEY_PUBLICATION, n2, own).await,
        Some(SUBMISSION_ACCEPTED)
    );
    assert!(
        node.view
            .lock()
            .unwrap()
            .prekeys
            .bundle(&carol.me)
            .is_some(),
        "carol's is held"
    );
}

// acceptance: SUB-02
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_publication_and_a_deposit_are_held_and_then_served_to_anyone() {
    let node = node_with("bob", |_, _| {});
    let carol = attach_light("carol", &node).await;
    // field 1 is the bundle object `wire-format.md` §7.10 names, not a byte
    // string wrapping it: a peer applying the CDDL would refuse the wrapping,
    // and a round trip against ourselves cannot tell the difference
    let published = PrekeyPublication {
        bundle: bundle("carol"),
        nonce: [1; 16],
    }
    .encode();
    let rhtn_codec::cbor::Item::Map(ref m) = rhtn_codec::cbor::parse_all(&published).unwrap()
    else {
        panic!("a map")
    };
    assert!(
        matches!(
            rhtn_codec::cbor::map_get(m, 1),
            Some(rhtn_codec::cbor::Item::Map(_))
        ),
        "field 1 is the bundle, spliced in"
    );
    assert_eq!(
        PrekeyPublication::decode(&published).unwrap().bundle,
        bundle("carol"),
        "and it reads back whole"
    );

    assert!(
        carol.serving.publish(&bundle("carol")).await,
        "the node takes the bundle"
    );
    assert!(
        carol.serving.stock(carol.me, keys(3)).await,
        "and the deposit"
    );
    // a fetch by a different client returns what carol published, byte for
    // byte, and one of the keys it deposited
    let alice = attach_light("alice", &node).await;
    let req = PrekeyRequest::One {
        subject: carol.me,
        one_time: true,
        nonce: [1; 16],
        device: Some([0u8; 32]),
    }
    .encode();
    let reply = PrekeyReply::decode(
        &alice
            .session
            .request(REQUEST_PREKEY, &req)
            .await
            .expect("answered"),
    )
    .unwrap();
    assert_eq!(
        reply.bundles.first().map(|b| b.as_slice()),
        Some(bundle("carol").as_slice())
    );
    let served = reply.one_time.expect("a one-time key");
    assert!(
        keys(3).contains(&served),
        "served {served:?}, deposited {:?}",
        keys(3)
    );
    assert_eq!(
        node.view.lock().unwrap().prekeys.pool_size(&carol.me),
        2,
        "one spent, two left"
    );
}

// acceptance: SUB-03
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_deposit_past_the_nodes_bound_is_refused_whole() {
    // this node lends a pool of five
    let node = node_with("bob", |_, v| {
        v.prekeys.cfg.pool = 5;
        v.prekeys.cfg.per_deposit = 4;
    });
    let carol = attach_light("carol", &node).await;
    let n1 = [1u8; 16];
    assert_eq!(
        code(
            &carol,
            REQUEST_ONE_TIME_DEPOSIT,
            n1,
            OneTimeDeposit {
                keys: keys(3),
                nonce: n1
            }
            .encode()
        )
        .await,
        Some(SUBMISSION_ACCEPTED)
    );
    // three more would make six, past the pool it lends
    let n2 = [2u8; 16];
    assert_eq!(
        code(
            &carol,
            REQUEST_ONE_TIME_DEPOSIT,
            n2,
            OneTimeDeposit {
                keys: keys(3),
                nonce: n2
            }
            .encode()
        )
        .await,
        Some(SUBMISSION_OVER_BOUND)
    );
    assert_eq!(
        node.view.lock().unwrap().prekeys.pool_size(&carol.me),
        3,
        "the pool is what it was: nothing was trimmed in"
    );
    // and one deposit larger than the per-deposit bound, whatever the pool
    // holds, is refused on its own count
    let n3 = [3u8; 16];
    assert_eq!(
        code(
            &carol,
            REQUEST_ONE_TIME_DEPOSIT,
            n3,
            OneTimeDeposit {
                keys: keys(5),
                nonce: n3
            }
            .encode()
        )
        .await,
        Some(SUBMISSION_OVER_BOUND)
    );
    assert_eq!(node.view.lock().unwrap().prekeys.pool_size(&carol.me), 3);
}

// acceptance: SUB-04
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_relay_submission_is_answered_on_taking_and_collected_later() {
    let node = node_with("bob", |_, _| {});
    let carol = attach_light("carol", &node).await;
    // w1 has never connected: the answer cannot be about delivery
    let n = [7u8; 16];
    let body = RelaySubmission {
        recipient: kh("w1"),
        ciphertext: b"sealed".to_vec(),
        nonce: n,
        device: [0; 32],
    }
    .encode();
    assert_eq!(
        code(&carol, REQUEST_RELAY, n, body).await,
        Some(SUBMISSION_ACCEPTED)
    );
    assert_eq!(node.node.queued(&kh("w1")), 1, "taken, and waiting");
    // w1 attaches and collects it, with carol named in front, unchanged
    let mut w1 = attach_light("w1", &node).await;
    let delivered = tokio::time::timeout(std::time::Duration::from_secs(3), w1.deliveries.recv())
        .await
        .expect("delivered")
        .expect("bytes");
    assert_eq!(unrelayed(&delivered), Some((carol.me, b"sealed".to_vec())));
    assert_eq!(
        node.node.queued(&kh("w1")),
        0,
        "no copy outlives the delivery"
    );
}

// acceptance: SUB-05
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_relay_for_a_keyhash_the_node_holds_no_record_of_is_refused() {
    // this node has no record of w2 and serves everyone else
    let node = node_with("bob", |c, _| c.serves = Arc::new(|k| *k != kh("w2")));
    let carol = attach_light("carol", &node).await;
    let unknown = [1u8; 16];
    let body = RelaySubmission {
        recipient: kh("w2"),
        ciphertext: b"sealed".to_vec(),
        nonce: unknown,
        device: [0; 32],
    }
    .encode();
    assert_eq!(
        code(&carol, REQUEST_RELAY, unknown, body).await,
        Some(SUBMISSION_REFUSED)
    );
    assert_eq!(
        node.node.queued(&kh("w2")),
        0,
        "nothing queued for a stranger"
    );
    // w1 is served and merely absent: a different answer, and the message
    // waits (design §14.1.2)
    let offline = [2u8; 16];
    let body = RelaySubmission {
        recipient: kh("w1"),
        ciphertext: b"sealed".to_vec(),
        nonce: offline,
        device: [0; 32],
    }
    .encode();
    assert_eq!(
        code(&carol, REQUEST_RELAY, offline, body).await,
        Some(SUBMISSION_ACCEPTED)
    );
    assert_eq!(node.node.queued(&kh("w1")), 1);
}

// acceptance: SUB-06
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn one_wake_endpoint_per_relationship_replaced_on_re_registration() {
    let node = node_with("bob", |_, _| {});
    let carol = attach_light("carol", &node).await;
    let e1 = WakeEndpoint {
        url: "https://push.example/one".into(),
        key: vec![1; 32],
        lapses_at: None,
    };
    let e2 = WakeEndpoint {
        url: "https://push.example/two".into(),
        key: vec![2; 32],
        lapses_at: Some(1_900_000_000),
    };
    assert!(carol.serving.wake(carol.me, Some(e1)).await);
    assert!(carol.serving.wake(carol.me, Some(e2.clone())).await);
    let view = node.view.lock().unwrap();
    let held = view.wake.get(&carol.me).expect("one endpoint");
    assert_eq!(
        (held.url.as_str(), held.key.as_slice(), held.lapses_at),
        (e2.url.as_str(), e2.key.as_slice(), e2.lapses_at),
        "the second replaced the first"
    );
    assert_eq!(
        view.wake.holders(),
        vec![carol.me],
        "and only carol's is held"
    );
}

// acceptance: SUB-07
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_registration_with_no_endpoint_withdraws_and_a_key_without_one_is_malformed() {
    let node = node_with("bob", |_, _| {});
    let carol = attach_light("carol", &node).await;
    let e = WakeEndpoint {
        url: "https://push.example/one".into(),
        key: vec![1; 32],
        lapses_at: None,
    };
    assert!(carol.serving.wake(carol.me, Some(e)).await);
    assert!(node.view.lock().unwrap().wake.get(&carol.me).is_some());
    // no endpoint withdraws, and the node says it took the withdrawal
    assert!(carol.serving.wake(carol.me, None).await);
    assert!(
        node.view.lock().unwrap().wake.get(&carol.me).is_none(),
        "forgotten"
    );
    // a key with no endpoint to attach it to never reaches the register:
    // the decoder refuses the shape and the stream fails
    let n = [4u8; 16];
    let orphan = WakeRegistration {
        nonce: n,
        endpoint: None,
        key: Some(vec![3; 32]),
        lapses_at: None,
    }
    .encode();
    assert!(
        WakeRegistration::decode(&orphan).is_err(),
        "the decoder refuses it here too"
    );
    assert!(
        carol.session.request(REQUEST_WAKE, &orphan).await.is_err(),
        "and at the node"
    );
    assert!(node.view.lock().unwrap().wake.get(&carol.me).is_none());
}

// acceptance: SUB-08
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn every_reply_echoes_its_own_nonce_and_says_nothing_else() {
    let node = node_with("bob", |c, v| {
        v.prekeys.cfg.pool = 2;
        c.serves = Arc::new(|k| *k != kh("w2"));
    });
    let carol = attach_light("carol", &node).await;
    let sent: Vec<(u64, [u8; 16], Vec<u8>)> = vec![
        (
            REQUEST_PREKEY_PUBLICATION,
            [1; 16],
            PrekeyPublication {
                bundle: bundle("carol"),
                nonce: [1; 16],
            }
            .encode(),
        ),
        (
            REQUEST_ONE_TIME_DEPOSIT,
            [2; 16],
            OneTimeDeposit {
                keys: keys(9),
                nonce: [2; 16],
            }
            .encode(),
        ),
        (
            REQUEST_RELAY,
            [3; 16],
            RelaySubmission {
                recipient: kh("w2"),
                ciphertext: b"x".to_vec(),
                nonce: [3; 16],
                device: [0; 32],
            }
            .encode(),
        ),
        (
            REQUEST_WAKE,
            [4; 16],
            WakeRegistration::of(
                [4; 16],
                Some(WakeEndpoint {
                    url: "https://push.example/x".into(),
                    key: vec![1; 32],
                    lapses_at: None,
                }),
            )
            .encode(),
        ),
    ];
    let mut codes = Vec::new();
    for (rt, nonce, body) in sent {
        let bytes = carol.session.request(rt, &body).await.expect("answered");
        let r = SubmissionReply::decode(&bytes).expect("a reply");
        assert_eq!(
            r.nonce, nonce,
            "each reply echoes the nonce of the request that drew it"
        );
        assert!(r.code <= 2);
        // two fields and no more: the reply carries the nonce and the code
        let rhtn_codec::cbor::Item::Map(ref m) = rhtn_codec::cbor::parse_all(&bytes).unwrap()
        else {
            panic!("a map")
        };
        assert_eq!(m.len(), 2, "nothing further");
        codes.push(r.code);
    }
    assert_eq!(
        codes,
        vec![
            SUBMISSION_ACCEPTED,
            SUBMISSION_OVER_BOUND,
            SUBMISSION_REFUSED,
            SUBMISSION_ACCEPTED
        ],
        "one of each answer, and no reason with any of them"
    );
    // and a client that ignored the nonce would be reading somebody
    // else's answer: a reply that does not echo is not this one's
    let mismatched = SubmissionReply::code([5; 16], SUBMISSION_ACCEPTED).encode();
    assert_eq!(SubmissionReply::decode(&mismatched).unwrap().nonce, [5; 16]);
}

// ------------------------------- what a serving node propagates to a client

/// A record that adopts `node` under `patron`, signed by both, on a
/// meeting between them.
fn adoption(scene: &mut Scene, node: &str, patron: &str, path: &[u8]) -> Vec<u8> {
    scene.adopt(node, patron, "bob", path).bytes
}

// acceptance: TOP-24
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn what_the_serving_node_floods_reaches_the_clients_horizon_and_nothing_else_does() {
    let mut scene = Scene::new();
    let alice = adoption(&mut scene, "alice", "bob", &[0]);
    let carol = adoption(&mut scene, "carol", "bob", &[1]);
    let handle = spawn_client(
        "alice",
        rhtn_client::ceremony::Config::default(),
        rhtn_adaptors::direct::Reachable::default(),
    );
    let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
    let task = rhtn_adaptors::attached::follow(handle.clone(), rx);

    // stream 0's topology push is what a client learns from
    tx.send((
        rhtn_adaptors::attached::FRAME_TOPOLOGY_PUSH,
        rhtn_node::propagation::encode_push(rhtn_node::store::KIND_TRANSACTION, &alice),
    ))
    .unwrap();
    tx.send((
        rhtn_adaptors::attached::FRAME_TOPOLOGY_PUSH,
        rhtn_node::propagation::encode_push(rhtn_node::store::KIND_TRANSACTION, &carol),
    ))
    .unwrap();
    assert!(
        until(3000, || handle.with_blocking(|c| c.horizon.records()) == 2).await,
        "both records reach the horizon"
    );
    let placed = handle.with_blocking(|c| {
        (
            c.horizon
                .places_of(&kh("carol"))
                .into_iter()
                .cloned()
                .collect::<Vec<_>>(),
            c.horizon.distance(&kh("carol")),
        )
    });
    assert_eq!(
        placed.0.iter().map(|p| p.anchor).collect::<Vec<_>>(),
        vec![kh("bob")],
        "carol is placed from what was flooded, in the one subnet it was flooded in"
    );
    assert_eq!(
        placed.1,
        Some(1),
        "and is one edge away: a sibling under the same patron"
    );

    // a memo is a node's routing aid and not a client's, and a push
    // carrying something other than a transaction is not topology either
    let memo = adoption(&mut scene, "w1", "carol", &[1, 0]);
    tx.send((
        6,
        rhtn_node::propagation::encode_push(rhtn_node::store::KIND_TRANSACTION, &memo),
    ))
    .unwrap();
    tx.send((
        rhtn_adaptors::attached::FRAME_TOPOLOGY_PUSH,
        rhtn_node::propagation::encode_push(99, &memo),
    ))
    .unwrap();
    tx.send((
        rhtn_adaptors::attached::FRAME_TOPOLOGY_PUSH,
        b"not a push".to_vec(),
    ))
    .unwrap();
    // one that is topology, to show the reader survived the three above
    let w2 = adoption(&mut scene, "w2", "alice", &[0, 0]);
    tx.send((
        rhtn_adaptors::attached::FRAME_TOPOLOGY_PUSH,
        rhtn_node::propagation::encode_push(rhtn_node::store::KIND_TRANSACTION, &w2),
    ))
    .unwrap();
    assert!(
        until(3000, || handle.with_blocking(|c| c.horizon.records()) == 3).await,
        "the fourth is taken"
    );
    assert_eq!(
        handle.with_blocking(|c| c.horizon.distance(&kh("w1"))),
        None,
        "and the memo's record never arrived"
    );
    assert_eq!(
        handle.with_blocking(|c| c.horizon.distance(&kh("w2"))),
        Some(1),
        "the subordinate did"
    );

    // the follower ends with the session that fed it
    drop(tx);
    assert!(
        tokio::time::timeout(std::time::Duration::from_secs(3), task)
            .await
            .is_ok(),
        "the task ends when the channel closes"
    );
}
