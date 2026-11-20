//! A client reaching a party in a subnet it knows only by its anchor
//! (`wire-format.md` §7.7.1, §7.7.3; design §12.6.1; functional row
//! RES-016): the descent driven by the client itself through three real
//! infra nodes, the node key fetched from and the seal handed to the
//! serving node the last answer named — each as nobody, on a connection
//! minted for it (`wire-format.md` §9.1) — and the same path run back the
//! other way so two clients on subnets that share no node exchange
//! payload.

use rhtn_adaptors::beyond::{Beyond, Beyonder, Distant};
use rhtn_adaptors::serving::Answer;
use rhtn_archive::Keyhash;
use rhtn_archive::node_key::{NodeKey, NodeKeyReply, NodeKeyRequest, REQUEST_NODE_KEY};
use rhtn_archive::submission::{
    REQUEST_RELAY, RelaySubmission, SUBMISSION_ACCEPTED, SubmissionReply, unrelayed,
};
use rhtn_client::horizon::{Askable, Reach, Upstream};
use rhtn_node::resolution::{REQUEST_RESOLVE, ResolveRequest};
use rhtn_sim::subnets::{Fabric, id, kh, material, wan_plans};
use rhtn_transport::session::{NetworkPoint, Session};
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// The next delivery `to` collects from `node` opens, under `to`'s own
/// secrets and the node's half, to `what` sealed by `from`, its tag
/// verifying for `from`.  **On a remote fabric, the next one that does**:
/// the machines outlive the test and hold what an earlier test queued for
/// the same client, so what an earlier run left is read past, bounded; in
/// this process every fabric is fresh and the first delivery is the one.
async fn delivered(f: &Fabric, to: &mut Session, who: &str, node: &str, from: &str, what: &[u8]) {
    let mine = secret(who);
    let theirs = encryption_key(from);
    for _ in 0..8 {
        let d = tokio::time::timeout(Duration::from_secs(3), to.deliveries.recv())
            .await
            .unwrap_or_else(|_| panic!("delivered: {what:?} from {from}"))
            .expect("bytes");
        let r = unrelayed(&d).expect("a relayed payload");
        let (sealed, body) = rhtn_client::payload::open_relayed(&mine, &kh(who), &kh(node), &r)
            .expect("opens under the recipient's secrets and the node's half");
        if (body.sender, body.bytes.as_slice()) == (kh(from), what) {
            assert!(
                rhtn_client::payload::tag_verifies(&mine, &kh(who), &theirs, &sealed, &body),
                "attributed to {from} on opening"
            );
            return;
        }
        assert!(
            f.is_remote(),
            "a fresh fabric delivers {what:?} from {from} first, not {:?} from {}",
            body.bytes,
            hex::encode(&body.sender[..4])
        );
    }
    panic!("{what:?} from {from} was not among eight deliveries");
}

/// `name`'s encryption secrets, derived from its root.
fn secret(name: &str) -> rhtn_crypto::seal::EncryptionSecret {
    rhtn_crypto::seal::EncryptionSecret::from_identity_root(id(name).identity_root())
}

/// `name`'s encryption key, as its card carries it.
fn encryption_key(name: &str) -> rhtn_crypto::verify::EncryptionKey {
    rhtn_crypto::verify::encryption_key(
        &[id(name).public.clone()][..],
        &rhtn_crypto::encryption_key::own(&id(name)),
    )
    .expect("verifies")
}

/// **The node key `to`'s node holds for it, fetched as nobody** from the
/// node `reach` resolves to (`wire-format.md` §7.8): verified under that
/// node, as a sender holds it.
async fn node_key(b: &Beyond, to: &str, reach: Reach) -> rhtn_client::payload::NodeKeyHeld {
    let req = NodeKeyRequest::for_recipients(vec![kh(to)], [0x6b; 16]).expect("one");
    let (up, reply) = b
        .minted(kh(to), reach, REQUEST_NODE_KEY, req.encode())
        .await
        .expect("the serving node answered the fetch");
    let r = NodeKeyReply::decode(&reply).expect("a node-key reply");
    assert_eq!(r.nonce, [0x6b; 16]);
    let key = NodeKey::parse(&r.outcome.expect("a key")).expect("a node key");
    let node = rhtn_crypto::Identity::from_key_material(
        &up.key_material
            .clone()
            .unwrap_or_else(|| material_of(&up.node)),
    )
    .expect("the node's material");
    key.verify(&[node][..])
        .expect("signed by the node that answered");
    assert_eq!((key.fields.node, key.fields.recipient), (up.node, kh(to)));
    rhtn_client::payload::NodeKeyHeld {
        node: up.node,
        n: key.fields.n,
        public: Box::new(key.fields.public),
    }
}

/// The material of a fabric node known by keyhash.
fn material_of(k: &Keyhash) -> Vec<u8> {
    ["alice2", "bob", "c1", "c2", "c3"]
        .iter()
        .find(|n| kh(n) == *k)
        .map(|n| material(n))
        .expect("a node of the fabric")
}

/// **`from` seals `what` to `to` under `held` and hands it to the node
/// `reach` resolves to, as nobody** (`wire-format.md` §7.8.1, §7.10).
/// Whether the node took it, its reply echoing the submission's nonce.
async fn send(
    b: &Beyond,
    from: &str,
    to: &str,
    held: &rhtn_client::payload::NodeKeyHeld,
    reach: Reach,
    what: &[u8],
) -> bool {
    let me = kh(from);
    let mine = secret(from);
    let sender = rhtn_client::payload::Sender {
        keyhash: &me,
        secret: &mine,
        alias: from,
        introduction: None,
    };
    let mut fresh = |out: &mut [u8]| {
        for c in out.chunks_mut(32) {
            let r = rhtn_transport::tls::random_bytes::<32>();
            c.copy_from_slice(&r[..c.len()]);
        }
    };
    let sealed = rhtn_client::payload::seal_to(
        &sender,
        &kh(to),
        &encryption_key(to),
        held,
        rhtn_client::payload::KIND_APPLICATION,
        what,
        &mut fresh,
    )
    .expect("sealed");
    let nonce = rhtn_transport::tls::random_bytes::<16>();
    let body = RelaySubmission {
        recipient: kh(to),
        sealed,
        nonce,
    }
    .encode();
    let Some((up, reply)) = b.minted(kh(to), reach, REQUEST_RELAY, body).await else {
        return false;
    };
    assert_eq!(up.node, held.node, "handed to the node whose key sealed it");
    SubmissionReply::decode(&reply).is_ok_and(|r| r.nonce == nonce && r.code == SUBMISSION_ACCEPTED)
}

/// The two subnets of `wan_plans`: alice2 roots the home one with bob its
/// infra child at index 1 and alice and w1 light clients under bob; c1
/// roots the far one, c2 is infra under c1 at index 4, c3 infra under c2
/// at 2, c4 a light client under c3 at 1 and c5 a light client under c4
/// at 3 — two light clients served by c3, three infra nodes deep.  In
/// this process, or on the rented machines a manifest in
/// `RHTN_WAN_MANIFEST` names, where a node's own counts cannot be read.
fn fabric() -> Fabric {
    Fabric::for_tests(wan_plans(), 30, &[])
}

/// What a node holds queued for `who`, where the node is in this process.
fn queued(f: &Fabric, node: &str, who: &str) -> Option<usize> {
    f.subnet_of(node)
        .nodes
        .get(node)
        .map(|n| n.node.queued(&kh(who)))
}

/// Every ask the client made, in order: of whom, which request, and the
/// body, with the real dialler behind it.  **The client's own behaviour is
/// what is read here**: which nodes it asked and how often, which is what
/// the row's clauses are about.
struct Watched {
    inner: Arc<Beyonder>,
    asked: Mutex<Vec<(Keyhash, u64, Vec<u8>)>>,
    /// Which of those went as nobody, by their place in `asked`.
    minted: Mutex<Vec<usize>>,
}

impl Distant for Watched {
    fn ask<'a>(
        &'a self,
        node: Keyhash,
        key_material: Option<Vec<u8>>,
        at: Vec<Vec<u8>>,
        request: u64,
        body: Vec<u8>,
    ) -> Answer<'a, Option<Vec<u8>>> {
        self.asked
            .lock()
            .unwrap()
            .push((node, request, body.clone()));
        self.inner.ask(node, key_material, at, request, body)
    }

    fn ask_minted<'a>(
        &'a self,
        node: Keyhash,
        key_material: Option<Vec<u8>>,
        at: Vec<Vec<u8>>,
        request: u64,
        body: Vec<u8>,
    ) -> Answer<'a, Option<Vec<u8>>> {
        {
            let mut asked = self.asked.lock().unwrap();
            self.minted.lock().unwrap().push(asked.len());
            asked.push((node, request, body.clone()));
        }
        self.inner.ask_minted(node, key_material, at, request, body)
    }
}

/// Whether every fetch and submission went as nobody, and nothing else did.
fn minted_exactly_the_fetches_and_submissions(w: &Watched) -> bool {
    let minted = w.minted.lock().unwrap().clone();
    w.asked
        .lock()
        .unwrap()
        .iter()
        .enumerate()
        .all(|(i, (_, r, _))| {
            minted.contains(&i) == (*r == REQUEST_NODE_KEY || *r == REQUEST_RELAY)
        })
}

/// A view beyond `name`'s own node, dialling for real and watched.
fn beyond(f: &Fabric, name: &str) -> (Arc<Beyond>, Arc<Watched>) {
    let watched = Arc::new(Watched {
        inner: Beyonder::new(f.client_endpoint(), f.client_cfg(name)),
        asked: Mutex::new(Vec::new()),
        minted: Mutex::new(Vec::new()),
    });
    // a fresh nonce per logical resolution, as the client's own source
    // would give
    let counter = Arc::new(Mutex::new(0u8));
    let nonces = Arc::new(move || {
        let mut c = counter.lock().unwrap();
        *c += 1;
        [*c; 16]
    });
    (Beyond::new(watched.clone(), nonces), watched)
}

/// The reach of `to`, which this client holds as a bare position under
/// an anchor, with `ask` the nodes it could put the resolution to.
fn reach_by_anchor(f: &Fabric, to: &str, ask: Vec<Askable>) -> Reach {
    let (anchor, path, nibbles) = f.subnet_of(to).position(to);
    Reach {
        upstream: Upstream::Position {
            anchor,
            path,
            nibbles,
        },
        ask,
    }
}

/// The anchor of `to`'s subnet, as an outsider holds it: keyhash,
/// endpoints and material, from the entry alone.
fn anchor_of(f: &Fabric, to: &str) -> Askable {
    let (node, endpoints, key_material) = f.subnet_of(to).askable();
    Askable {
        node,
        endpoints,
        key_material,
    }
}

fn nodes_asked(w: &Watched) -> Vec<(Keyhash, u64)> {
    w.asked
        .lock()
        .unwrap()
        .iter()
        .map(|(n, r, _)| (*n, *r))
        .collect()
}

fn resolve_nonces(w: &Watched) -> Vec<[u8; 16]> {
    w.asked
        .lock()
        .unwrap()
        .iter()
        .filter(|(_, r, _)| *r == REQUEST_RESOLVE)
        .map(|(_, _, b)| ResolveRequest::decode(b).expect("a resolve request").nonce)
        .collect()
}

// functional: RES-016
// acceptance: RES-20
/// **The client drives the descent itself**, holding nothing of the far
/// subnet but its anchor's entry: the anchor refers it to c2, c2 to c3,
/// and c3 answers as c4's serving node with the residual that names c4;
/// the payload then goes to c3 and nowhere else.  A node that does not
/// answer moves the resolution to the next one held; one nonce serves the
/// whole resolution; a second message to the same party asks nobody; and
/// a party deeper under the same serving node resolves to it with a
/// longer residual under a nonce of its own.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_client_drives_the_descent_through_a_foreign_subnet_it_knows_by_its_anchor_alone() {
    let f = fabric();
    let (b, watched) = beyond(&f, "w1");

    // **first a node that does not answer**: an address nothing listens
    // on, under an identity the client can bind, ahead of the real anchor
    let dark = Askable {
        node: kh("dark"),
        endpoints: vec![NetworkPoint::new([127, 0, 0, 1], Some(9)).encode_bytes()],
        key_material: Some(material("dark")),
    };
    let reach = reach_by_anchor(&f, "c4", vec![dark, anchor_of(&f, "c4")]);
    let held = node_key(&b, "c4", reach.clone()).await;
    assert!(
        send(&b, "w1", "c4", &held, reach.clone(), b"for c4").await,
        "c3 took it"
    );
    assert_eq!(
        nodes_asked(&watched),
        vec![
            (kh("dark"), REQUEST_RESOLVE),
            (kh("c1"), REQUEST_RESOLVE),
            (kh("c2"), REQUEST_RESOLVE),
            (kh("c3"), REQUEST_RESOLVE),
            (kh("c3"), REQUEST_NODE_KEY),
            (kh("c3"), REQUEST_RELAY),
        ],
        "the dark node, then the anchor, each referral, and the serving node \
         it ended at, asked for its key and handed the seal"
    );
    let nonces = resolve_nonces(&watched);
    assert_eq!(nonces.len(), 4);
    assert!(
        nonces.iter().all(|n| *n == nonces[0]),
        "one nonce for the logical resolution, across every node asked"
    );
    if !f.is_remote() {
        assert_eq!(queued(&f, "c3", "c4"), Some(1), "queued where c4 collects");
        assert_eq!(queued(&f, "c1", "c4"), Some(0));
        assert_eq!(
            queued(&f, "bob", "c4"),
            Some(0),
            "never the sender's own node"
        );
    }

    // **inside the lifetime, nobody is asked where c4 is**, and the key
    // held seals the second
    assert!(send(&b, "w1", "c4", &held, reach, b"again").await);
    let asked = nodes_asked(&watched);
    assert_eq!(asked.len(), 7);
    assert_eq!(asked[6], (kh("c3"), REQUEST_RELAY), "the submission alone");
    if !f.is_remote() {
        assert_eq!(queued(&f, "c3", "c4"), Some(2));
    }

    // **a party deeper under the same serving node**: c5 sits under c4,
    // a light client, so c3 is its serving node too and answers with the
    // two-index residual; a resolution of its own, under its own nonce
    let reach = reach_by_anchor(&f, "c5", vec![anchor_of(&f, "c5")]);
    let held5 = node_key(&b, "c5", reach.clone()).await;
    assert!(send(&b, "w1", "c5", &held5, reach, b"for c5").await);
    let asked = nodes_asked(&watched);
    assert_eq!(
        &asked[7..],
        &[
            (kh("c1"), REQUEST_RESOLVE),
            (kh("c2"), REQUEST_RESOLVE),
            (kh("c3"), REQUEST_RESOLVE),
            (kh("c3"), REQUEST_NODE_KEY),
            (kh("c3"), REQUEST_RELAY),
        ]
    );
    let nonces = resolve_nonces(&watched);
    assert_ne!(
        nonces[0], nonces[4],
        "a fresh nonce for a fresh logical resolution"
    );
    assert!(nonces[4..].iter().all(|n| *n == nonces[4]));
    if !f.is_remote() {
        assert_eq!(queued(&f, "c3", "c5"), Some(1));
    }
    assert!(
        minted_exactly_the_fetches_and_submissions(&watched),
        "every fetch and submission as nobody, every resolution as w1"
    );

    // **what was queued reaches c4 when it attaches**, and opens there as
    // w1's
    let mut c4 = f.attach("c4").await;
    delivered(&f, &mut c4, "c4", "c3", "w1", b"for c4").await;
    delivered(&f, &mut c4, "c4", "c3", "w1", b"again").await;
    if !f.is_remote() {
        assert_eq!(
            queued(&f, "c3", "c4"),
            Some(2),
            "held after delivery: the cap or the retention removes, not a delivery"
        );
        assert_eq!(queued(&f, "c3", "c5"), Some(1), "c5 has not collected");
    }
}

// acceptance: PAY-27
/// **Two clients on subnets that share no node exchange payload**, each
/// holding the other's position and the other subnet's anchor and nothing
/// more: w1 reaches c4 through c1, c2 and c3, and c4 reaches w1 through
/// alice and bob.  Neither sender's own node is on the other's path.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn payload_crosses_between_clients_of_two_subnets_that_share_no_node() {
    let f = fabric();
    let mut w1 = f.attach("w1").await;
    let mut c4 = f.attach("c4").await;
    let (w1_beyond, w1_asked) = beyond(&f, "w1");
    let (c4_beyond, c4_asked) = beyond(&f, "c4");

    let to_c4 = reach_by_anchor(&f, "c4", vec![anchor_of(&f, "c4")]);
    let held = node_key(&w1_beyond, "c4", to_c4.clone()).await;
    assert!(
        send(
            &w1_beyond,
            "w1",
            "c4",
            &held,
            to_c4,
            b"hello from the home subnet"
        )
        .await
    );
    let to_w1 = reach_by_anchor(&f, "w1", vec![anchor_of(&f, "w1")]);
    let held = node_key(&c4_beyond, "w1", to_w1.clone()).await;
    assert!(
        send(
            &c4_beyond,
            "c4",
            "w1",
            &held,
            to_w1,
            b"hello from the far subnet"
        )
        .await
    );
    assert_eq!(
        nodes_asked(&c4_asked),
        vec![
            (kh("alice2"), REQUEST_RESOLVE),
            (kh("bob"), REQUEST_RESOLVE),
            (kh("bob"), REQUEST_NODE_KEY),
            (kh("bob"), REQUEST_RELAY),
        ],
        "the home anchor refers to bob, which serves w1 with the residual"
    );
    assert_eq!(
        nodes_asked(&w1_asked)
            .iter()
            .map(|(n, _)| *n)
            .collect::<Vec<_>>(),
        vec![kh("c1"), kh("c2"), kh("c3"), kh("c3"), kh("c3")]
    );
    // **neither sender's own node is asked anything**
    assert!(!nodes_asked(&w1_asked).iter().any(|(n, _)| *n == kh("bob")));
    assert!(!nodes_asked(&c4_asked).iter().any(|(n, _)| *n == kh("c3")));
    assert!(minted_exactly_the_fetches_and_submissions(&w1_asked));
    assert!(minted_exactly_the_fetches_and_submissions(&c4_asked));

    delivered(&f, &mut c4, "c4", "c3", "w1", b"hello from the home subnet").await;
    delivered(&f, &mut w1, "w1", "bob", "c4", b"hello from the far subnet").await;
}

// acceptance: RES-20
/// **A client's serving node resolves a foreign party on its behalf**
/// (`wire-format.md` §7.7.1's default): w1 holds no address in the far
/// subnet at all and puts the resolution to bob, whose anchor table holds
/// c1's entry; bob walks the far subnet's referrals itself and answers w1
/// with c3 as c4's serving node, and the payload goes to c3.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_serving_node_resolves_a_foreign_party_for_its_client_from_its_anchor_table() {
    let f = fabric();
    assert!(f.introduce("bob", 1), "bob's table takes c1's entry");
    let (b, watched) = beyond(&f, "w1");
    let bob = f.subnet_of("w1").addr_of("bob");
    let ask = vec![Askable {
        node: kh("bob"),
        endpoints: vec![NetworkPoint::from_socket(bob).unwrap().encode_bytes()],
        key_material: Some(material("bob")),
    }];
    let reach = reach_by_anchor(&f, "c4", ask);
    let held = node_key(&b, "c4", reach.clone()).await;
    assert!(
        send(&b, "w1", "c4", &held, reach, b"via bob").await,
        "c3 took it"
    );
    assert_eq!(
        nodes_asked(&watched),
        vec![
            (kh("bob"), REQUEST_RESOLVE),
            (kh("c3"), REQUEST_NODE_KEY),
            (kh("c3"), REQUEST_RELAY)
        ],
        "bob was asked once and answered with the serving node; c3 gave its key \
         and took the payload"
    );
    if !f.is_remote() {
        assert_eq!(queued(&f, "c3", "c4"), Some(1));
    }
}
