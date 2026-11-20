//! What a client hands a node (`wire-format.md` §7.10): over its session,
//! its profile, its horizon and where to be rung; and as nobody, on a
//! connection under a key minted for it (§9.1), a sealed message for a
//! recipient the node serves.  Every party here is real — a running node,
//! light clients attached to it over QUIC, and the request types on the
//! wire between them.

mod common;

use common::*;
use rhtn_adaptors::attached::AttachedNode;
use rhtn_adaptors::serving::Serving;
use rhtn_archive::Keyhash;
use rhtn_archive::node_key::REQUEST_NODE_KEY;
use rhtn_archive::profile::{KIND_OWN, Profile, ProfileReply, ProfileRequest};
use rhtn_archive::submission::*;
use rhtn_archive::tx::{Locator, Seqno};
use rhtn_node::resolution::{AnchorTable, Ingestion, Path};
use rhtn_node::runtime::LiveNode;
use rhtn_node::view::NodeView;
use rhtn_transport::session::*;
use std::sync::Arc;
use std::sync::atomic::{AtomicU8, Ordering};

/// The subnet the fixtures' positions are in (`wire-format.md` §2.3 field 4).
const SUBNET: [u8; 32] = [0xdb; 32];

/// A node for `name` whose configuration a test may adjust before it
/// starts: the served set is the operator's, and these tests exercise it.
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
            subnet: Some(SUBNET),
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

/// **One request of `node` as nobody** (`wire-format.md` §9.1): a
/// connection under a transport key minted for it, one request, closed.
async fn as_nobody(node: &Arc<LiveNode>, request: u64, body: &[u8]) -> Option<Vec<u8>> {
    // the identity in the configuration is never presented: the pins are
    // what is used, to bind the node
    let cfg = client_cfg("w4");
    let conn = connect_minted(&cfg, &client_ep(), node.me(), node.addr)
        .await
        .expect("the node is reached and bound");
    let reply = request_on(&conn, request, body).await.ok();
    conn.close(quinn::VarInt::from_u32(0), b"minted");
    reply
}

/// What `from` seals to `to` under the key `node` holds for `to`, fetched
/// from it as nobody.
async fn sealed_at(node: &Arc<LiveNode>, from: &str, to: &str, bytes: &[u8]) -> Vec<u8> {
    let reply = as_nobody(node, REQUEST_NODE_KEY, &node_key_request(to, [0x4b; 16]))
        .await
        .expect("a node-key reply");
    seal_under(from, to, &reply, bytes, None)
}

/// A relay submission's bytes.
fn relay(to: &str, sealed: &[u8], nonce: [u8; 16]) -> Vec<u8> {
    RelaySubmission {
        recipient: kh(to),
        sealed: sealed.to_vec(),
        nonce,
    }
    .encode()
}

/// The code a reply carries, its nonce held to the one sent.
fn reply_code(reply: &[u8], nonce: [u8; 16]) -> u64 {
    let r = SubmissionReply::decode(reply).expect("a submission reply");
    assert_eq!(r.nonce, nonce, "the reply echoes the nonce of the request");
    r.code
}

// acceptance: SUB-04
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_relay_submission_is_answered_on_taking_and_collected_later() {
    let node = node_with("bob", |_, _| {});
    let carol = attach_light("carol", &node).await;
    // w1 has never connected: the answer cannot be about delivery
    let sealed = sealed_at(&node, "carol", "w1", b"for w1").await;
    let n = [7u8; 16];
    let reply = as_nobody(&node, REQUEST_RELAY, &relay("w1", &sealed, n))
        .await
        .expect("answered");
    assert_eq!(reply_code(&reply, n), SUBMISSION_ACCEPTED);
    assert!(
        !node.node.sessions().contains(&kh("w1")),
        "answered with w1 nowhere"
    );
    assert_eq!(node.node.queued(&kh("w1")), 1, "taken, and waiting");
    // w1 attaches and collects it: the seal carol made, unchanged, with
    // the node's half beside it, and carol inside it
    let mut w1 = attach_light("w1", &node).await;
    let delivered = tokio::time::timeout(std::time::Duration::from_secs(3), w1.deliveries.recv())
        .await
        .expect("delivered")
        .expect("bytes");
    let r = unrelayed(&delivered).expect("a relayed payload");
    assert_eq!(r.sealed, sealed, "the ciphertext unchanged");
    let body = open_as("w1", &node.me(), &delivered).expect("opens, and its tag verifies");
    assert_eq!(
        (body.sender, body.bytes.as_slice()),
        (carol.me, b"for w1".as_slice()),
        "carol named inside the seal, where only w1 reads it"
    );
    // **delivery removes nothing** (design §14.1.6): the message is marked
    // delivered and stays until the cap or the retention takes it
    assert_eq!(node.node.queued(&kh("w1")), 1, "held after delivery");
    assert!(
        node.node.queue_records(&kh("w1"))[0].delivered_at.is_some(),
        "and marked delivered"
    );
}

// acceptance: SUB-05
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_relay_for_a_keyhash_the_node_holds_no_record_of_is_refused() {
    // this node has no record of w2 and serves everyone else
    let node = node_with("bob", |c, _| c.serves = Arc::new(|k| *k != kh("w2")));
    let _carol = attach_light("carol", &node).await;
    // a seal for w2 under a key some other node holds for it: the most a
    // sender could hold, since this node will make none for w2
    let elsewhere = node_with("carol", |_, _| {});
    let for_w2 = sealed_at(&elsewhere, "carol", "w2", b"x").await;
    let unknown = [1u8; 16];
    let reply = as_nobody(&node, REQUEST_RELAY, &relay("w2", &for_w2, unknown))
        .await
        .expect("answered");
    assert_eq!(reply_code(&reply, unknown), SUBMISSION_REFUSED);
    assert_eq!(
        node.node.queued(&kh("w2")),
        0,
        "nothing queued for a stranger"
    );
    // w1 is served and merely absent: a different answer, and the message
    // waits (design §14.1.2)
    let for_w1 = sealed_at(&node, "carol", "w1", b"x").await;
    let offline = [2u8; 16];
    let reply = as_nobody(&node, REQUEST_RELAY, &relay("w1", &for_w1, offline))
        .await
        .expect("answered");
    assert_eq!(reply_code(&reply, offline), SUBMISSION_ACCEPTED);
    assert_eq!(node.node.queued(&kh("w1")), 1);
}

/// A node that answers every sealed message under the nonce of another
/// submission, having taken it: what a reply crossed with somebody
/// else's looks like from the sender's side.
struct Misanswering(Arc<dyn Serving>);

impl Serving for Misanswering {
    fn me(&self) -> Keyhash {
        self.0.me()
    }
    fn serves(&self, client: &Keyhash) -> bool {
        self.0.serves(client)
    }
    fn minted<'a>(
        &'a self,
        request: u64,
        body: Vec<u8>,
    ) -> rhtn_adaptors::serving::Answer<'a, Option<Vec<u8>>> {
        Box::pin(async move {
            let reply = self.0.minted(request, body).await?;
            if request != REQUEST_RELAY {
                return Some(reply);
            }
            let r = SubmissionReply::decode(&reply).ok()?;
            Some(SubmissionReply::code([0xee; 16], r.code).encode())
        })
    }
    fn teach<'a>(&'a self, f: Vec<Vec<u8>>) -> rhtn_adaptors::serving::Answer<'a, usize> {
        self.0.teach(f)
    }
    fn publish_profile<'a>(&'a self, b: &'a [u8]) -> rhtn_adaptors::serving::Answer<'a, bool> {
        self.0.publish_profile(b)
    }
    fn assert_horizon<'a>(
        &'a self,
        b: &'a [u8],
        n: [u8; 16],
    ) -> rhtn_adaptors::serving::Answer<'a, bool> {
        self.0.assert_horizon(b, n)
    }
    fn profile<'a>(&'a self, b: &'a [u8]) -> rhtn_adaptors::serving::Answer<'a, Option<Vec<u8>>> {
        self.0.profile(b)
    }
    fn catalog<'a>(
        &'a self,
        from: Keyhash,
        b: &'a [u8],
    ) -> rhtn_adaptors::serving::Answer<'a, Option<Vec<u8>>> {
        self.0.catalog(from, b)
    }
    fn resource<'a>(
        &'a self,
        from: Keyhash,
        b: &'a [u8],
    ) -> rhtn_adaptors::serving::Answer<'a, Option<Vec<u8>>> {
        self.0.resource(from, b)
    }
    fn wake<'a>(
        &'a self,
        client: Keyhash,
        e: Option<WakeEndpoint>,
    ) -> rhtn_adaptors::serving::Answer<'a, bool> {
        self.0.wake(client, e)
    }
    fn propagate<'a>(&'a self, b: Vec<u8>) -> rhtn_adaptors::serving::Answer<'a, bool> {
        self.0.propagate(b)
    }
}

/// **A reply that echoes another submission's nonce is not this one's
/// answer** (`wire-format.md` §7.10): the sender reports the message
/// unsent rather than taken, though the node in fact took it — the nonce
/// is all that ties a reply to what was sent. The same node answering
/// under the right nonce is a message taken.
// acceptance: PAY-24
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_reply_echoing_another_submissions_nonce_is_not_an_answer_to_this_one() {
    let mut scene = Scene::new();
    scene.adopt("alice", "bob", "bob", &[0]);
    scene.adopt("w1", "bob", "bob", &[1]);
    let node = live_node("bob", scene.table("bob", &["bob"]), "bob", &[]);
    let inboxes = rhtn_adaptors::serving::Inboxes::default();
    let honest: Arc<dyn Serving> =
        rhtn_adaptors::serving::LocalNode::new(node.clone(), inboxes.clone());
    let crossed: Arc<dyn Serving> = Arc::new(Misanswering(honest.clone()));
    let alice = host("alice", &crossed, &inboxes, 0);
    let w1 = host("w1", &honest, &inboxes, 0);
    exchange_cards(&[("alice", &alice), ("w1", &w1)]).await;
    for p in [&alice, &w1] {
        p.courier.attach(vec![kh("alice"), kh("w1")]).await;
    }
    let before = node.node.queued(&kh("w1"));
    let carried = alice
        .courier
        .send(
            kh("w1"),
            rhtn_client::payload::KIND_APPLICATION,
            b"crossed".to_vec(),
        )
        .await
        .expect("sealed");
    assert!(
        carried
            .refused
            .iter()
            .any(|m| matches!(m, rhtn_client::ceremony::Msg::Sealed { to, .. } if *to == kh("w1"))),
        "reported unsent: {carried:?}"
    );
    assert!(
        node.node.queued(&kh("w1")) > before,
        "though the node did take it, which the sender cannot know"
    );
    // the control: the same node, answering under the nonce sent
    let carried = w1
        .courier
        .send(
            kh("alice"),
            rhtn_client::payload::KIND_APPLICATION,
            b"straight".to_vec(),
        )
        .await
        .expect("sealed");
    assert!(carried.complete(), "{carried:?}");
}

// acceptance: SUB-06
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn one_wake_endpoint_per_relationship_replaced_on_re_registration() {
    let node = node_with("bob", |_, _| {});
    let carol = attach_light("carol", &node).await;
    let e1 = WakeEndpoint {
        url: "https://push.example/one".into(),
        key: vec![1; 32],
        signing_key: [3; 32],
        lapses_at: None,
    };
    let e2 = WakeEndpoint {
        url: "https://push.example/two".into(),
        key: vec![2; 32],
        signing_key: [3; 32],
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
        signing_key: [3; 32],
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
        signing_key: None,
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
    // room for one small seal per recipient and not two, so a second for
    // w1, with the first never delivered, is over the bound
    let node = node_with("bob", |c, _| {
        c.serves = Arc::new(|k| *k != kh("w2"));
        c.queue_cap = Some(3000);
    });
    let carol = attach_light("carol", &node).await;
    let elsewhere = node_with("carol", |_, _| {});
    let for_w2 = sealed_at(&elsewhere, "carol", "w2", b"x").await;
    let for_w1 = sealed_at(&node, "carol", "w1", b"x").await;
    let again_w1 = sealed_at(&node, "carol", "w1", b"y").await;
    assert!(for_w1.len() < 3000 && for_w1.len() + again_w1.len() > 3000);
    let profile = |who: &str, nonce: [u8; 16]| {
        ProfilePublication {
            profile: Profile::build(&id(who), &SUBNET, KIND_OWN, who, None, 1_800_000_000),
            nonce,
        }
        .encode()
    };
    // **the relay goes as nobody** (`wire-format.md` §9.1) and the rest on
    // the session, which is where each is sent from; the reply's shape is
    // the same on either
    let sent: Vec<(bool, u64, [u8; 16], Vec<u8>)> = vec![
        (true, REQUEST_RELAY, [3; 16], relay("w2", &for_w2, [3; 16])),
        (true, REQUEST_RELAY, [6; 16], relay("w1", &for_w1, [6; 16])),
        (
            true,
            REQUEST_RELAY,
            [7; 16],
            relay("w1", &again_w1, [7; 16]),
        ),
        (
            false,
            REQUEST_WAKE,
            [4; 16],
            WakeRegistration::of(
                [4; 16],
                Some(WakeEndpoint {
                    url: "https://push.example/x".into(),
                    key: vec![1; 32],
                    signing_key: [3; 32],
                    lapses_at: None,
                }),
            )
            .encode(),
        ),
        (
            false,
            REQUEST_PROFILE_PUBLICATION,
            [1; 16],
            profile("carol", [1; 16]),
        ),
        // somebody else's profile, which the node will not take from carol
        (
            false,
            REQUEST_PROFILE_PUBLICATION,
            [2; 16],
            profile("alice", [2; 16]),
        ),
    ];
    let mut codes = Vec::new();
    for (minted, rt, nonce, body) in sent {
        let bytes = if minted {
            as_nobody(&node, rt, &body).await.expect("answered")
        } else {
            carol.session.request(rt, &body).await.expect("answered")
        };
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
            SUBMISSION_REFUSED,
            SUBMISSION_ACCEPTED,
            SUBMISSION_OVER_BOUND,
            SUBMISSION_ACCEPTED,
            SUBMISSION_ACCEPTED,
            SUBMISSION_REFUSED
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

// acceptance: SES-32
// acceptance: SES-33
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_profile_is_published_over_the_wire_and_fetched_by_anyone() {
    let node = node_with("bob", |_, _| {});
    let carol = attach_light("carol", &node).await;
    let alice = attach_light("alice", &node).await;
    // carol's own, published by carol: taken
    let carols = Profile::build(
        &id("carol"),
        &SUBNET,
        KIND_OWN,
        "Carol",
        None,
        1_800_000_000,
    );
    assert!(
        carol.serving.publish_profile(&carols).await,
        "the node takes it"
    );
    // alice's, published by carol: refused, and nothing held for alice
    let alices = Profile::build(
        &id("alice"),
        &SUBNET,
        KIND_OWN,
        "Alice",
        None,
        1_800_000_000,
    );
    assert!(!carol.serving.publish_profile(&alices).await);
    // anyone fetches carol's, read-only, and alice's is unknown
    let fetch = |subject: &str| {
        ProfileRequest {
            subject: kh(subject),
            subnet: SUBNET,
            kind: KIND_OWN,
            nonce: [4; 16],
        }
        .encode()
    };
    let reply = alice
        .serving
        .profile(&fetch("carol"))
        .await
        .expect("answered");
    let r = ProfileReply::decode(&reply).unwrap();
    assert_eq!(r.profile.as_deref(), Some(carols.as_slice()));
    let reply = alice
        .serving
        .profile(&fetch("alice"))
        .await
        .expect("answered");
    let r = ProfileReply::decode(&reply).unwrap();
    assert_eq!(
        (r.profile, r.code),
        (None, Some(rhtn_archive::profile::FAIL_UNKNOWN))
    );
}

/// **A resource reached through the node hosting it, over a real session**
/// (`wire-format.md` §11.1): the request crosses as a request frame, the
/// node evaluates the asker's reach before the resource sees anything
/// (§11.2), and the status comes back as §11.1 fixes it. A resource this
/// node hosts none of is refused, which is the honest answer and not a
/// fault of the path.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_resource_request_crosses_a_session_and_comes_back_with_its_status() {
    let node = node_with("bob", |_, _| {});
    let carol = attach_light("carol", &node).await;
    let body = rhtn_archive::catalog::ResourceRequest {
        resource: [0x1a; 32],
        message: b"GET / HTTP/1.1\r\nhost: r1\r\n\r\n".to_vec(),
    }
    .encode();
    let reply = carol
        .serving
        .resource(carol.me, &body)
        .await
        .expect("the node answers a request frame");
    let r = rhtn_archive::catalog::ResourceResponse::decode(&reply).expect("a response");
    assert_eq!(
        r.status,
        rhtn_archive::catalog::STATUS_REFUSED,
        "a resource this node hosts none of: refused, and nothing invented"
    );
    assert_eq!(r.body, None, "and no application answer with it");
}
