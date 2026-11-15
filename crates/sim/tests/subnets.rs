//! A client reaching a party in a subnet it knows only by its anchor
//! (`wire-format.md` §7.7.1, §7.7.3; design §12.6.1; functional row
//! RES-016): the descent driven by the client itself through three real
//! infra nodes, the payload handed to the serving node the last answer
//! named, and the same path run back the other way so two clients on
//! subnets that share no node exchange payload.

use rhtn_adaptors::beyond::{Beyond, Beyonder, Distant};
use rhtn_adaptors::serving::Answer;
use rhtn_archive::Keyhash;
use rhtn_archive::submission::{REQUEST_RELAY, unrelayed};
use rhtn_client::horizon::{Askable, Reach, Upstream};
use rhtn_node::resolution::{REQUEST_RESOLVE, ResolveRequest};
use rhtn_sim::subnets::{Fabric, Plan, kh, material};
use rhtn_transport::session::NetworkPoint;
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// The home subnet: alice roots it, bob is her infra child at index 1,
/// carol a light client under her at 0 and w1 a light client under bob
/// at 0.  The far subnet: c1 roots it, c2 is infra under c1 at index 4,
/// c3 infra under c2 at 2, c4 a light client under c3 at 1 and c5 a
/// light client under c4 at 3 — two light clients served by c3, three
/// infra nodes deep.
fn fabric() -> Fabric {
    Fabric::build(
        vec![
            Plan::new("alice")
                .infra("bob", "alice", 1)
                .light("carol", "alice", 0)
                .light("w1", "bob", 0),
            Plan::new("c1")
                .infra("c2", "c1", 4)
                .infra("c3", "c2", 2)
                .light("c4", "c3", 1)
                .light("c5", "c4", 3),
        ],
        30,
    )
}

/// Every ask the client made, in order: of whom, which request, and the
/// body, with the real dialler behind it.  **The client's own behaviour is
/// what is read here**: which nodes it asked and how often, which is what
/// the row's clauses are about.
struct Watched {
    inner: Arc<Beyonder>,
    asked: Mutex<Vec<(Keyhash, u64, Vec<u8>)>>,
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
}

/// A view beyond `name`'s own node, dialling for real and watched.
fn beyond(f: &Fabric, name: &str) -> (Arc<Beyond>, Arc<Watched>) {
    let watched = Arc::new(Watched {
        inner: Beyonder::new(f.client_endpoint(), f.client_cfg(name)),
        asked: Mutex::new(Vec::new()),
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
    let far = f.subnet_of("c4");
    let (b, watched) = beyond(&f, "w1");

    // **first a node that does not answer**: an address nothing listens
    // on, under an identity the client can bind, ahead of the real anchor
    let dark = Askable {
        node: kh("dark"),
        endpoints: vec![NetworkPoint::new([127, 0, 0, 1], Some(9)).encode_bytes()],
        key_material: Some(material("dark")),
    };
    let reach = reach_by_anchor(&f, "c4", vec![dark, anchor_of(&f, "c4")]);
    assert!(
        b.carry(
            kh("c4"),
            [0; 32],
            b"for c4".to_vec(),
            reach.clone(),
            Some(kh("bob"))
        )
        .await,
        "c3 took it"
    );
    assert_eq!(
        nodes_asked(&watched),
        vec![
            (kh("dark"), REQUEST_RESOLVE),
            (kh("c1"), REQUEST_RESOLVE),
            (kh("c2"), REQUEST_RESOLVE),
            (kh("c3"), REQUEST_RESOLVE),
            (kh("c3"), REQUEST_RELAY),
        ],
        "the dark node, then the anchor, each referral, and the serving node it ended at"
    );
    let nonces = resolve_nonces(&watched);
    assert_eq!(nonces.len(), 4);
    assert!(
        nonces.iter().all(|n| *n == nonces[0]),
        "one nonce for the logical resolution, across every node asked"
    );
    assert_eq!(
        far.node("c3").node.queued(&kh("c4")),
        1,
        "queued where c4 collects"
    );
    assert_eq!(far.node("c1").node.queued(&kh("c4")), 0);
    assert_eq!(
        f.subnet_of("w1").node("bob").node.queued(&kh("c4")),
        0,
        "never the sender's own node"
    );

    // **inside the lifetime, nobody is asked where c4 is**
    assert!(
        b.carry(kh("c4"), [0; 32], b"again".to_vec(), reach, Some(kh("bob")))
            .await
    );
    let asked = nodes_asked(&watched);
    assert_eq!(asked.len(), 6);
    assert_eq!(asked[5], (kh("c3"), REQUEST_RELAY), "the submission alone");
    assert_eq!(far.node("c3").node.queued(&kh("c4")), 2);

    // **a party deeper under the same serving node**: c5 sits under c4,
    // a light client, so c3 is its serving node too and answers with the
    // two-index residual; a resolution of its own, under its own nonce
    let reach = reach_by_anchor(&f, "c5", vec![anchor_of(&f, "c5")]);
    assert!(
        b.carry(
            kh("c5"),
            [0; 32],
            b"for c5".to_vec(),
            reach,
            Some(kh("bob"))
        )
        .await
    );
    let asked = nodes_asked(&watched);
    assert_eq!(
        &asked[6..],
        &[
            (kh("c1"), REQUEST_RESOLVE),
            (kh("c2"), REQUEST_RESOLVE),
            (kh("c3"), REQUEST_RESOLVE),
            (kh("c3"), REQUEST_RELAY),
        ]
    );
    let nonces = resolve_nonces(&watched);
    assert_ne!(
        nonces[0], nonces[4],
        "a fresh nonce for a fresh logical resolution"
    );
    assert!(nonces[4..].iter().all(|n| *n == nonces[4]));
    assert_eq!(far.node("c3").node.queued(&kh("c5")), 1);

    // **what was queued reaches c4 when it attaches**, named by its sender
    let mut c4 = f.attach("c4").await;
    let mut got = Vec::new();
    while got.len() < 2 {
        let d = tokio::time::timeout(Duration::from_secs(3), c4.deliveries.recv())
            .await
            .expect("delivered")
            .expect("bytes");
        got.push(unrelayed(&d).expect("a relayed payload"));
    }
    assert_eq!(got[0].0, kh("w1"));
    assert_eq!(got[0].1, b"for c4".to_vec());
    assert_eq!(got[1].1, b"again".to_vec());
    assert_eq!(
        far.node("c3").node.queued(&kh("c4")),
        0,
        "no copy outlives delivery"
    );
    assert_eq!(
        far.node("c3").node.queued(&kh("c5")),
        1,
        "c5 has not collected"
    );
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

    assert!(
        w1_beyond
            .carry(
                kh("c4"),
                [0; 32],
                b"hello from the home subnet".to_vec(),
                reach_by_anchor(&f, "c4", vec![anchor_of(&f, "c4")]),
                Some(kh("bob")),
            )
            .await
    );
    assert!(
        c4_beyond
            .carry(
                kh("w1"),
                [0; 32],
                b"hello from the far subnet".to_vec(),
                reach_by_anchor(&f, "w1", vec![anchor_of(&f, "w1")]),
                Some(kh("c3")),
            )
            .await
    );
    assert_eq!(
        nodes_asked(&c4_asked),
        vec![
            (kh("alice"), REQUEST_RESOLVE),
            (kh("bob"), REQUEST_RESOLVE),
            (kh("bob"), REQUEST_RELAY),
        ],
        "the home anchor refers to bob, which serves w1 with the residual"
    );
    assert_eq!(
        nodes_asked(&w1_asked)
            .iter()
            .map(|(n, _)| *n)
            .collect::<Vec<_>>(),
        vec![kh("c1"), kh("c2"), kh("c3"), kh("c3")]
    );

    let to_c4 = tokio::time::timeout(Duration::from_secs(3), c4.deliveries.recv())
        .await
        .expect("delivered to c4")
        .expect("bytes");
    let to_w1 = tokio::time::timeout(Duration::from_secs(3), w1.deliveries.recv())
        .await
        .expect("delivered to w1")
        .expect("bytes");
    let (from, what, _) = unrelayed(&to_c4).unwrap();
    assert_eq!(
        (from, what),
        (kh("w1"), b"hello from the home subnet".to_vec())
    );
    let (from, what, _) = unrelayed(&to_w1).unwrap();
    assert_eq!(
        (from, what),
        (kh("c4"), b"hello from the far subnet".to_vec())
    );
}
