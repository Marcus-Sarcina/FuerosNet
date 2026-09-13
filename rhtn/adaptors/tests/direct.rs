//! The direct path from the client's own send (design §12.6.3, §14.1.1):
//! candidates offered as their own payload kind over the relay, the peer's
//! dialled, and payload crossing the connection with the serving node
//! carrying none of it.  For light clients on sockets of their own, and
//! for nodes that are participants on the nodes' own.

mod common;

use common::*;
use rhtn_adaptors::actor::Handle;
use rhtn_adaptors::courier::{Courier, Inlet};
use rhtn_adaptors::direct::{Direct, Gate, LightDirect, NodeDirect, Reachable};
use rhtn_adaptors::serving::{Inboxes, LocalNode, Serving};
use rhtn_archive::Keyhash;
use rhtn_client::ceremony::{Config, Dispatched};
use rhtn_client::payload::KIND_APPLICATION;
use rhtn_node::runtime::LiveNode;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;
use tokio::sync::mpsc::UnboundedReceiver;

struct Party {
    handle: Handle,
    courier: Arc<Courier>,
    app: UnboundedReceiver<(Keyhash, Dispatched)>,
    reachable: Reachable,
}

/// A serving node that counts what it relays.
struct Counting {
    inner: Arc<dyn Serving>,
    relays: AtomicUsize,
}

impl Counting {
    fn over(inner: Arc<dyn Serving>) -> Arc<Counting> {
        Arc::new(Counting { inner, relays: AtomicUsize::new(0) })
    }
    fn relayed(&self) -> usize {
        self.relays.load(Ordering::SeqCst)
    }
}

impl Serving for Counting {
    fn me(&self) -> Keyhash {
        self.inner.me()
    }
    fn holds(&self, subject: &Keyhash) -> bool {
        self.inner.holds(subject)
    }
    fn serves(&self, client: &Keyhash) -> bool {
        self.inner.serves(client)
    }
    fn publish(&self, bytes: &[u8]) -> bool {
        self.inner.publish(bytes)
    }
    fn stock(&self, subject: Keyhash, keys: Vec<Vec<u8>>) {
        self.inner.stock(subject, keys)
    }
    fn prekey(&self, from: Keyhash, body: &[u8]) -> Option<Vec<u8>> {
        self.inner.prekey(from, body)
    }
    fn relay(&self, from: Keyhash, to: Keyhash, bytes: Vec<u8>) -> bool {
        self.relays.fetch_add(1, Ordering::SeqCst);
        self.inner.relay(from, to, bytes)
    }
}

/// A light client beside `node` on a socket of its own; whether the path
/// may be direct is what the node's table says of its horizon.
fn light(name: &'static str, node: &Arc<LiveNode>, serving: &Arc<dyn Serving>, inboxes: &Inboxes) -> Party {
    let inlet = Inlet::default();
    let gate: Gate = {
        let node = node.clone();
        let me = kh(name);
        Arc::new(move |peer| node.view.lock().unwrap().table.horizon(&me, 2).contains(peer))
    };
    let direct = LightDirect::bind(Arc::new(id(name)), pins(), loopback(), None, None, gate, inlet.inbound()).expect("bound");
    let reachable = direct.reachable();
    let handle = spawn_client(name, Config::default(), reachable.clone());
    let (courier, app) = Courier::new(handle.clone(), serving.clone(), Arc::new(direct));
    inlet.bind(courier.inbound());
    inboxes.host(kh(name), courier.inbound());
    Party { handle, courier, app, reachable }
}

/// A light client whose local decision refuses every direct path: what a
/// user choosing the relay leaves the client with.
fn light_refusing(name: &'static str, serving: &Arc<dyn Serving>, inboxes: &Inboxes) -> Party {
    let inlet = Inlet::default();
    let gate: Gate = Arc::new(|_| false);
    let direct = LightDirect::bind(Arc::new(id(name)), pins(), loopback(), None, None, gate, inlet.inbound()).expect("bound");
    let reachable = direct.reachable();
    let handle = spawn_client(name, Config::default(), reachable.clone());
    let (courier, app) = Courier::new(handle.clone(), serving.clone(), Arc::new(direct));
    inlet.bind(courier.inbound());
    inboxes.host(kh(name), courier.inbound());
    Party { handle, courier, app, reachable }
}

/// A node that is a participant: its client on the node's own socket.
fn infra(name: &'static str, node: &Arc<LiveNode>, serving: &Arc<dyn Serving>, inboxes: &Inboxes) -> Party {
    let inlet = Inlet::default();
    let direct = NodeDirect::new(node.clone(), pins(), inlet.inbound());
    let reachable = direct.reachable();
    let handle = spawn_client(name, Config::default(), reachable.clone());
    let (courier, app) = Courier::new(handle.clone(), serving.clone(), direct);
    inlet.bind(courier.inbound());
    inboxes.host(kh(name), courier.inbound());
    Party { handle, courier, app, reachable }
}

async fn next(app: &mut UnboundedReceiver<(Keyhash, Dispatched)>, ms: u64) -> Option<(Keyhash, Dispatched)> {
    tokio::time::timeout(Duration::from_millis(ms), app.recv()).await.ok().flatten()
}

// acceptance: TRV-07
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn two_light_clients_beside_their_serving_node_join_the_direct_path_from_their_own_sends() {
    // carol and w1 adopted under bob: siblings, inside each other's horizon
    let mut scene = Scene::new();
    scene.adopt("carol", "bob", "bob", &[0]);
    scene.adopt("w1", "bob", "bob", &[1]);
    let node = live_node("bob", scene.table("bob", &["bob"]), "bob", &[]);
    let inboxes = Inboxes::default();
    let counting = Counting::over(LocalNode::new(node.clone(), inboxes.clone()));
    let serving: Arc<dyn Serving> = counting.clone();
    let carol = light("carol", &node, &serving, &inboxes);
    let mut w1 = light("w1", &node, &serving, &inboxes);
    carol.courier.attach(vec![kh("w1")]).await;
    w1.courier.attach(vec![kh("carol")]).await;
    carol.courier.sweep(vec![kh("w1")]).await;
    // carol offers: her candidates go as their own kind over the relay;
    // w1 dials them, and offers its own back
    assert!(carol.courier.offer(kh("w1")).await);
    assert!(until(4000, || carol.reachable.holds(&kh("w1")) && w1.reachable.holds(&kh("carol"))).await, "both hold the path");
    let relayed = counting.relayed();
    assert!(relayed >= 1, "the candidates went through the serving node");
    // payload from the client's own send takes the direct path, and the
    // serving node carries none of it
    carol.courier.send(kh("w1"), KIND_APPLICATION, b"over the direct path".to_vec()).await.expect("sent");
    let (from, d) = next(&mut w1.app, 3000).await.expect("delivered");
    assert_eq!(from, kh("carol"));
    assert!(matches!(&d, Dispatched::Application(b) if b == b"over the direct path"), "{d:?}");
    assert_eq!(counting.relayed(), relayed, "nothing more relayed");
    assert!(carol.handle.with_blocking(|c| c.device.direct.reachable(&kh("w1"))), "the client's own interface says so");
    // a peer outside the horizon gets no offer at all: nothing gathered,
    // nothing sent (design §12.6.3)
    assert!(!carol.courier.offer(kh("w2")).await);
    assert_eq!(counting.relayed(), relayed);
}

// acceptance: TRV-07
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn two_nodes_that_are_participants_join_the_direct_path_on_their_own_sockets() {
    // w2 adopted under w1: each inside the other's horizon
    let mut scene = Scene::new();
    scene.adopt("w2", "w1", "w1", &[0]);
    let n1 = live_node("w1", scene.table("w1", &["w1", "w2"]), "w1", &[]);
    let n2 = live_node("w2", scene.table("w2", &["w1", "w2"]), "w1", &[0]);
    // each node is its own serving node, and reaches the other for what
    // it does not hold
    let inboxes = Inboxes::default();
    let s1 = LocalNode::new(n1.clone(), inboxes.clone());
    let s2 = LocalNode::new(n2.clone(), inboxes.clone());
    s1.reach_beyond(s2.clone());
    s2.reach_beyond(s1.clone());
    let (c1, c2) = (Counting::over(s1), Counting::over(s2));
    let (serving1, serving2): (Arc<dyn Serving>, Arc<dyn Serving>) = (c1.clone(), c2.clone());
    let p1 = infra("w1", &n1, &serving1, &inboxes);
    let mut p2 = infra("w2", &n2, &serving2, &inboxes);
    p1.courier.attach(vec![kh("w2")]).await;
    p2.courier.attach(vec![kh("w1")]).await;
    p1.courier.sweep(vec![kh("w2")]).await;
    assert!(p1.courier.offer(kh("w2")).await);
    assert!(until(4000, || p1.reachable.holds(&kh("w2")) && p2.reachable.holds(&kh("w1"))).await, "both hold the path");
    let relayed = c1.relayed() + c2.relayed();
    p1.courier.send(kh("w2"), KIND_APPLICATION, b"node to node".to_vec()).await.expect("sent");
    let (from, d) = next(&mut p2.app, 3000).await.expect("delivered");
    assert_eq!(from, kh("w1"));
    assert!(matches!(&d, Dispatched::Application(b) if b == b"node to node"), "{d:?}");
    assert_eq!(c1.relayed() + c2.relayed(), relayed, "nothing more relayed");
    assert_eq!(n1.direct_state(&kh("w2")), Some(true));
    assert!(p2.handle.with_blocking(|c| c.device.direct.reachable(&kh("w1"))));
}

// acceptance: TRV-08
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_direct_path_the_local_decision_refuses_opens_in_neither_direction() {
    // carol and w1 are siblings under bob, so the horizon would allow it;
    // the gate installed here refuses anyway, as a relay preference does
    let mut scene = Scene::new();
    scene.adopt("carol", "bob", "bob", &[0]);
    scene.adopt("w1", "bob", "bob", &[1]);
    let node = live_node("bob", scene.table("bob", &["bob"]), "bob", &[]);
    let inboxes = Inboxes::default();
    let counting = Counting::over(LocalNode::new(node.clone(), inboxes.clone()));
    let serving: Arc<dyn Serving> = counting.clone();
    let carol = light_refusing("carol", &serving, &inboxes);
    let mut w1 = light("w1", &node, &serving, &inboxes);
    carol.courier.attach(vec![kh("w1")]).await;
    w1.courier.attach(vec![kh("carol")]).await;
    carol.courier.sweep(vec![kh("w1")]).await;
    // carol gathers nothing, so offers nothing
    assert!(!carol.courier.offer(kh("w1")).await, "nothing gathered for a peer the gate refuses");
    // w1 offers its own: carol dials none of them
    assert!(w1.courier.offer(kh("carol")).await);
    assert!(!until(2500, || carol.reachable.holds(&kh("w1"))).await, "carol opens nothing");
    // and w1 dialling carol directly is closed unheld
    assert!(!until(2500, || w1.reachable.holds(&kh("carol"))).await, "carol accepts nothing either");
    // the payload still arrives, over the relay
    let relayed = counting.relayed();
    carol.courier.send(kh("w1"), KIND_APPLICATION, b"over the relay".to_vec()).await.expect("sent");
    let (from, d) = next(&mut w1.app, 3000).await.expect("delivered");
    assert_eq!(from, kh("carol"));
    assert!(matches!(&d, Dispatched::Application(b) if b == b"over the relay"), "{d:?}");
    assert!(counting.relayed() > relayed, "the serving node carried it");
}

// acceptance: TRV-09
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn an_infrastructure_node_opens_no_direct_path_it_would_not_have_gathered_for() {
    // two roots with no relationship between them: each is outside the
    // other's horizon, and each holds the other's transport pin
    let scene = Scene::new();
    let n1 = live_node("w1", scene.table("w1", &["w1"]), "w1", &[]);
    let n2 = live_node("w2", scene.table("w2", &["w2"]), "w2", &[]);
    assert!(!n1.permits_direct(&kh("w2")), "outside the horizon");
    assert!(!n2.permits_direct(&kh("w1")), "and in the other direction too");
    // n2 gathers for itself, by asking for a peer it does permit: its own
    // candidates are what n1 would be given
    let cands = n2.gather().await;
    assert!(!cands.is_empty(), "n2 has candidates of its own");
    // n1 gathers nothing for n2, and opening on n2's candidates by hand
    // dials nothing either
    assert!(n1.prepare_direct(&kh("w2")).await.is_none(), "nothing gathered");
    assert!(!n1.open_direct(kh("w2"), &pins(), &cands).await, "and nothing dialled");
    assert_eq!(n1.direct_state(&kh("w2")), None, "no path was even attempted");
    // nor does n2 accept what n1 opens: the same decision on the way in
    let c1 = n1.gather().await;
    assert!(!n2.open_direct(kh("w1"), &pins(), &c1).await);
    assert!(until(2000, || n2.direct_state(&kh("w1")).is_none()).await);
    // the adaptor above it reports the same, and its payload takes the relay
    let inboxes = Inboxes::default();
    let serving: Arc<dyn Serving> = LocalNode::new(n1.clone(), inboxes.clone());
    let p1 = infra("w1", &n1, &serving, &inboxes);
    assert!(!p1.courier.offer(kh("w2")).await, "no offer for a peer it may not reach");
    assert!(!p1.reachable.holds(&kh("w2")));
}
