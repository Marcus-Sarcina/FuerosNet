//! The direct path from the client's own send (design §12.6.3, §14.1.1):
//! candidates offered as their own payload kind over the relay, the peer's
//! dialled, and payload crossing the connection with the serving node
//! carrying none of it.  For light clients on sockets of their own, and
//! for nodes that are participants on the nodes' own.

mod common;

use common::*;
use rhtn_adaptors::actor::Handle;
use rhtn_adaptors::courier::{Courier, Inlet};
use rhtn_adaptors::direct::{Control, Direct, Gate, LightDirect, NodeDirect, Reachable};
use rhtn_adaptors::serving::{Answer, Inboxes, LocalNode, Serving};
use rhtn_archive::Keyhash;
use rhtn_archive::submission::WakeEndpoint;
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
        Arc::new(Counting {
            inner,
            relays: AtomicUsize::new(0),
        })
    }
    fn relayed(&self) -> usize {
        self.relays.load(Ordering::SeqCst)
    }
}

impl Serving for Counting {
    fn node_device(&self) -> [u8; 32] {
        [0; 32]
    }
    fn me(&self) -> Keyhash {
        self.inner.me()
    }
    fn holds(&self, subject: &Keyhash) -> bool {
        self.inner.holds(subject)
    }
    fn serves(&self, client: &Keyhash) -> bool {
        self.inner.serves(client)
    }
    fn catalog<'a>(&'a self, from: Keyhash, body: &'a [u8]) -> Answer<'a, Option<Vec<u8>>> {
        self.inner.catalog(from, body)
    }
    fn publish<'a>(&'a self, bytes: &'a [u8]) -> Answer<'a, bool> {
        self.inner.publish(bytes)
    }
    fn stock<'a>(&'a self, subject: Keyhash, keys: Vec<Vec<u8>>) -> Answer<'a, bool> {
        self.inner.stock(subject, keys)
    }
    fn prekey<'a>(&'a self, from: Keyhash, body: &'a [u8]) -> Answer<'a, Option<Vec<u8>>> {
        self.inner.prekey(from, body)
    }
    fn publish_profile<'a>(&'a self, bytes: &'a [u8]) -> Answer<'a, bool> {
        self.inner.publish_profile(bytes)
    }
    fn profile<'a>(&'a self, body: &'a [u8]) -> Answer<'a, Option<Vec<u8>>> {
        self.inner.profile(body)
    }
    fn resource<'a>(&'a self, from: Keyhash, body: &'a [u8]) -> Answer<'a, Option<Vec<u8>>> {
        self.inner.resource(from, body)
    }
    fn relay<'a>(
        &'a self,
        from: Keyhash,
        to: Keyhash,
        bytes: Vec<u8>,
        device: [u8; 32],
    ) -> Answer<'a, bool> {
        self.relays.fetch_add(1, Ordering::SeqCst);
        self.inner.relay(from, to, bytes, device)
    }
    fn wake<'a>(&'a self, client: Keyhash, endpoint: Option<WakeEndpoint>) -> Answer<'a, bool> {
        self.inner.wake(client, endpoint)
    }

    fn propagate<'a>(&'a self, bytes: Vec<u8>) -> Answer<'a, bool> {
        self.inner.propagate(bytes)
    }
}

/// A light client beside `node` on a socket of its own; whether the path
/// may be direct is whether the client holds the peer as a contact.
fn light(
    name: &'static str,
    _node: &Arc<LiveNode>,
    serving: &Arc<dyn Serving>,
    inboxes: &Inboxes,
) -> Party {
    let inlet = Inlet::default();
    // the gate is the contact rule (design §12.6.3): a party this client
    // holds a contact for may be dialled, and a path opens to one that
    // holds this client — read from the client's own table, bound once
    // the client exists
    let slot: Arc<std::sync::Mutex<Option<rhtn_adaptors::actor::Handle>>> = Arc::default();
    let gate: Gate = {
        let slot = slot.clone();
        Arc::new(move |peer| {
            let peer = *peer;
            match slot.lock().unwrap().clone() {
                Some(h) => h.with_blocking(move |c| c.store.contacts.contains_key(&peer)),
                None => false,
            }
        })
    };
    let direct = LightDirect::bind(
        rhtn_transport::tls::Party::of(Arc::new(id(name))),
        pins(),
        Default::default(),
        Reachable::default(),
        loopback(),
        None,
        None,
        gate,
        inlet.inbound(),
        Control::none(),
    )
    .expect("bound");
    let reachable = direct.reachable();
    let handle = spawn_client(name, Config::default(), reachable.clone());
    *slot.lock().unwrap() = Some(handle.clone());
    let (courier, app) = Courier::new(handle.clone(), serving.clone(), Arc::new(direct));
    inlet.bind(courier.inbound());
    inboxes.host(kh(name), courier.inbound());
    Party {
        handle,
        courier,
        app,
        reachable,
    }
}

/// `p` holds `who` as a contact (design §12.6.3): what licenses the direct
/// path, each side for itself.
fn hold(p: &Party, who: &str) {
    let i = id(who).public.clone();
    p.handle
        .with_blocking(move |c| c.contact(&i, rhtn_client::contacts::How::Introduced, true));
}

/// A light client whose local decision refuses every direct path: what a
/// user choosing the relay leaves the client with.
fn light_refusing(name: &'static str, serving: &Arc<dyn Serving>, inboxes: &Inboxes) -> Party {
    let inlet = Inlet::default();
    let gate: Gate = Arc::new(|_| false);
    let direct = LightDirect::bind(
        rhtn_transport::tls::Party::of(Arc::new(id(name))),
        pins(),
        Default::default(),
        Reachable::default(),
        loopback(),
        None,
        None,
        gate,
        inlet.inbound(),
        Control::none(),
    )
    .expect("bound");
    let reachable = direct.reachable();
    let handle = spawn_client(name, Config::default(), reachable.clone());
    let (courier, app) = Courier::new(handle.clone(), serving.clone(), Arc::new(direct));
    inlet.bind(courier.inbound());
    inboxes.host(kh(name), courier.inbound());
    Party {
        handle,
        courier,
        app,
        reachable,
    }
}

/// A node that is a participant: its client on the node's own socket.
fn infra(
    name: &'static str,
    node: &Arc<LiveNode>,
    serving: &Arc<dyn Serving>,
    inboxes: &Inboxes,
) -> Party {
    let inlet = Inlet::default();
    let direct = NodeDirect::new(node.clone(), pins(), inlet.inbound());
    let reachable = direct.reachable();
    let handle = spawn_client(name, Config::default(), reachable.clone());
    let (courier, app) = Courier::new(handle.clone(), serving.clone(), direct);
    inlet.bind(courier.inbound());
    inboxes.host(kh(name), courier.inbound());
    Party {
        handle,
        courier,
        app,
        reachable,
    }
}

async fn next(
    app: &mut UnboundedReceiver<(Keyhash, Dispatched)>,
    ms: u64,
) -> Option<(Keyhash, Dispatched)> {
    tokio::time::timeout(Duration::from_millis(ms), app.recv())
        .await
        .ok()
        .flatten()
}

// acceptance: TRV-07
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn two_light_clients_beside_their_serving_node_join_the_direct_path_from_their_own_sends() {
    // carol and w1 adopted under bob, and each holds the other as a
    // contact, which is what licenses the direct path (design §12.6.3)
    let mut scene = Scene::new();
    scene.adopt("carol", "bob", "bob", &[0]);
    scene.adopt("w1", "bob", "bob", &[1]);
    let node = live_node("bob", scene.table("bob", &["bob"]), "bob", &[]);
    let inboxes = Inboxes::default();
    let counting = Counting::over(LocalNode::new(node.clone(), inboxes.clone()));
    let serving: Arc<dyn Serving> = counting.clone();
    let carol = light("carol", &node, &serving, &inboxes);
    let mut w1 = light("w1", &node, &serving, &inboxes);
    hold(&carol, "w1");
    hold(&w1, "carol");
    carol.courier.attach(vec![kh("w1")]).await;
    w1.courier.attach(vec![kh("carol")]).await;
    carol.courier.sweep(vec![kh("w1")]).await;
    // carol offers: her candidates go as their own kind over the relay;
    // w1 dials them, and offers its own back
    assert!(carol.courier.offer(kh("w1")).await);
    assert!(
        until(4000, || carol.reachable.holds(&kh("w1"))
            && w1.reachable.holds(&kh("carol")))
        .await,
        "both hold the path"
    );
    let relayed = counting.relayed();
    assert!(relayed >= 1, "the candidates went through the serving node");
    // payload from the client's own send takes the direct path, and the
    // serving node carries none of it
    carol
        .courier
        .send(kh("w1"), KIND_APPLICATION, b"over the direct path".to_vec())
        .await
        .expect("sent");
    let (from, d) = next(&mut w1.app, 3000).await.expect("delivered");
    assert_eq!(from, kh("carol"));
    assert!(
        matches!(&d, Dispatched::Application { bytes, .. } if bytes == b"over the direct path"),
        "{d:?}"
    );
    assert_eq!(counting.relayed(), relayed, "nothing more relayed");
    assert!(
        carol
            .handle
            .with_blocking(|c| c.device.direct.reachable(&kh("w1"))),
        "the client's own interface says so"
    );
    // a peer this client holds no contact for gets no offer at all:
    // nothing gathered, nothing sent (design §12.6.3)
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
    assert!(
        until(4000, || p1.reachable.holds(&kh("w2"))
            && p2.reachable.holds(&kh("w1")))
        .await,
        "both hold the path"
    );
    let relayed = c1.relayed() + c2.relayed();
    p1.courier
        .send(kh("w2"), KIND_APPLICATION, b"node to node".to_vec())
        .await
        .expect("sent");
    let (from, d) = next(&mut p2.app, 3000).await.expect("delivered");
    assert_eq!(from, kh("w1"));
    assert!(
        matches!(&d, Dispatched::Application { bytes, .. } if bytes == b"node to node"),
        "{d:?}"
    );
    assert_eq!(c1.relayed() + c2.relayed(), relayed, "nothing more relayed");
    assert_eq!(n1.direct_state(&kh("w2")), Some(true));
    assert!(
        p2.handle
            .with_blocking(|c| c.device.direct.reachable(&kh("w1")))
    );
}

// acceptance: TRV-08
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_direct_path_the_local_decision_refuses_opens_in_neither_direction() {
    // w1 holds carol as a contact, so the rule would allow it; the gate
    // installed on carol's side refuses anyway — the local decision design
    // §12.6.3 leaves each side, which a future blacklist would be
    let mut scene = Scene::new();
    scene.adopt("carol", "bob", "bob", &[0]);
    scene.adopt("w1", "bob", "bob", &[1]);
    let node = live_node("bob", scene.table("bob", &["bob"]), "bob", &[]);
    let inboxes = Inboxes::default();
    let counting = Counting::over(LocalNode::new(node.clone(), inboxes.clone()));
    let serving: Arc<dyn Serving> = counting.clone();
    let carol = light_refusing("carol", &serving, &inboxes);
    let mut w1 = light("w1", &node, &serving, &inboxes);
    hold(&w1, "carol");
    carol.courier.attach(vec![kh("w1")]).await;
    w1.courier.attach(vec![kh("carol")]).await;
    carol.courier.sweep(vec![kh("w1")]).await;
    // carol gathers nothing, so offers nothing
    assert!(
        !carol.courier.offer(kh("w1")).await,
        "nothing gathered for a peer the gate refuses"
    );
    // w1 offers its own: carol dials none of them
    assert!(w1.courier.offer(kh("carol")).await);
    assert!(
        !until(2500, || carol.reachable.holds(&kh("w1"))).await,
        "carol opens nothing"
    );
    // and w1 dialling carol directly is closed unheld
    assert!(
        !until(2500, || w1.reachable.holds(&kh("carol"))).await,
        "carol accepts nothing either"
    );
    // the payload still arrives, over the relay
    let relayed = counting.relayed();
    carol
        .courier
        .send(kh("w1"), KIND_APPLICATION, b"over the relay".to_vec())
        .await
        .expect("sent");
    let (from, d) = next(&mut w1.app, 3000).await.expect("delivered");
    assert_eq!(from, kh("carol"));
    assert!(
        matches!(&d, Dispatched::Application { bytes, .. } if bytes == b"over the relay"),
        "{d:?}"
    );
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
    assert!(
        !n2.permits_direct(&kh("w1")),
        "and in the other direction too"
    );
    // n2 gathers for itself, by asking for a peer it does permit: its own
    // candidates are what n1 would be given
    let cands = n2.gather().await;
    assert!(!cands.is_empty(), "n2 has candidates of its own");
    // n1 gathers nothing for n2, and opening on n2's candidates by hand
    // dials nothing either
    assert!(
        n1.prepare_direct(&kh("w2")).await.is_none(),
        "nothing gathered"
    );
    assert!(
        !n1.open_direct(kh("w2"), &pins(), &cands).await,
        "and nothing dialled"
    );
    assert_eq!(
        n1.direct_state(&kh("w2")),
        None,
        "no path was even attempted"
    );
    // nor does n2 accept what n1 opens: the same decision on the way in
    let c1 = n1.gather().await;
    assert!(!n2.open_direct(kh("w1"), &pins(), &c1).await);
    assert!(until(2000, || n2.direct_state(&kh("w1")).is_none()).await);
    // the adaptor above it reports the same, and its payload takes the relay
    let inboxes = Inboxes::default();
    let serving: Arc<dyn Serving> = LocalNode::new(n1.clone(), inboxes.clone());
    let p1 = infra("w1", &n1, &serving, &inboxes);
    assert!(
        !p1.courier.offer(kh("w2")).await,
        "no offer for a peer it may not reach"
    );
    assert!(!p1.reachable.holds(&kh("w2")));
}

/// A light client whose direct path binds by what its own horizon view
/// holds (`light-client-requirements.md` §4.2), presenting `credential`
/// where it is a delegated desktop.
fn light_holding(
    name: &'static str,
    _node: &Arc<LiveNode>,
    serving: &Arc<dyn Serving>,
    inboxes: &Inboxes,
    credential: Option<Arc<rhtn_transport::tls::Credential>>,
) -> Party {
    let inlet = Inlet::default();
    // the gate is the contact rule (design §12.6.3): a party this client
    // holds a contact for may be dialled, and a path opens to one that
    // holds this client — read from the client's own table, bound once
    // the client exists
    let slot: Arc<std::sync::Mutex<Option<rhtn_adaptors::actor::Handle>>> = Arc::default();
    let gate: Gate = {
        let slot = slot.clone();
        Arc::new(move |peer| {
            let peer = *peer;
            match slot.lock().unwrap().clone() {
                Some(h) => h.with_blocking(move |c| c.store.contacts.contains_key(&peer)),
                None => false,
            }
        })
    };
    let reachable_cell: Arc<std::sync::Mutex<Option<Reachable>>> = Arc::default();
    // the client first, so its view is what the socket binds by
    let handle = {
        let cell = reachable_cell.clone();
        let placeholder = Reachable::default();
        *cell.lock().unwrap() = Some(placeholder.clone());
        spawn_client(name, Config::default(), placeholder)
    };
    *slot.lock().unwrap() = Some(handle.clone());
    let mut binding = rhtn_transport::bind::Binding::default().with_held(Arc::new(handle.clone()));
    if let Some(c) = &credential {
        binding = binding.with_credential(c.clone());
    }
    let direct = LightDirect::bind(
        rhtn_transport::tls::Party::of(Arc::new(id(name))),
        pins(),
        binding,
        Reachable::default(),
        loopback(),
        None,
        None,
        gate,
        inlet.inbound(),
        Control::none(),
    )
    .expect("bound");
    let reachable = direct.reachable();
    let (courier, app) = Courier::new(handle.clone(), serving.clone(), Arc::new(direct));
    inlet.bind(courier.inbound());
    inboxes.host(kh(name), courier.inbound());
    Party {
        handle,
        courier,
        app,
        reachable,
    }
}

// acceptance: TOP-43
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn top_43_a_light_client_holds_delegations_and_binds_a_direct_peer_by_one() {
    use rhtn_crypto::delegation::issue;
    use rhtn_transport::tls::Credential;
    // carol and w1 adopted under bob and each other's contact; w1 runs
    // a delegated desktop under key k
    let mut scene = Scene::new();
    scene.adopt("carol", "bob", "bob", &[0]);
    scene.adopt("w1", "bob", "bob", &[1]);
    let node = live_node("bob", scene.table("bob", &["bob"]), "bob", &[]);
    let inboxes = Inboxes::default();
    let serving: Arc<dyn Serving> = LocalNode::new(node.clone(), inboxes.clone());
    // the earlier credential's window closed a minute ago; the later one,
    // opening then, is the one in force by the wall clock the bind reads
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();
    let t = now - 172_800 - 60;
    let w1 = id("w1");
    let desktop_key = Arc::new(Credential::from_seed(&[41; 32], kh("w1")));
    let d1 = issue(&w1, &desktop_key.public(), t);
    let d2 = issue(&w1, &desktop_key.public(), t + 172_800);
    desktop_key
        .add(std::slice::from_ref(&w1.public), &d2)
        .unwrap();
    let carol = light_holding("carol", &node, &serving, &inboxes, None);
    let mut desktop = light_holding("w1", &node, &serving, &inboxes, Some(desktop_key.clone()));
    hold(&carol, "w1");
    hold(&desktop, "carol");
    carol.courier.attach(vec![kh("w1")]).await;
    desktop.courier.attach(vec![kh("carol")]).await;
    carol.courier.sweep(vec![kh("w1")]).await;
    // A's view holds the neighbourhood the node pushed: the adoptions that
    // place w1 inside it
    let records: Vec<Vec<u8>> = scene.records.iter().map(|r| r.bytes.clone()).collect();
    carol
        .handle
        .with(move |c| {
            let known = c.known.clone();
            for r in &records {
                c.horizon.ingest(r, &known);
            }
            assert!(c.horizon.distance(&kh("w1")).is_some(), "w1 is placed");
        })
        .await;
    // N pushes D's delegation, then a later one over the same key: A's
    // horizon view holds one, the later
    let (a, b) = (d1.clone(), d2.clone());
    let (first, second, held, newest) = carol
        .handle
        .with(move |c| {
            let known = c.known.clone();
            let first = c.horizon.ingest_delegation(&a, &known);
            let second = c.horizon.ingest_delegation(&b, &known);
            (
                first,
                second,
                c.horizon.delegations(),
                c.horizon.delegation(&kh("w1")).map(|d| d.not_before),
            )
        })
        .await;
    assert_eq!(
        (first, second),
        (
            rhtn_client::horizon::Took::Applied,
            rhtn_client::horizon::Took::Applied
        )
    );
    assert_eq!(held, 1, "one delegation for D");
    assert_eq!(newest, Some(t + 172_800), "the later one");
    // and the earlier one arriving again is nothing new
    let a = d1.clone();
    assert_eq!(
        carol
            .handle
            .with(move |c| {
                let known = c.known.clone();
                c.horizon.ingest_delegation(&a, &known)
            })
            .await,
        rhtn_client::horizon::Took::Duplicate
    );
    // A dials D's desktop on the direct path: the key it presents, k, is
    // bound to D from the held delegation before any frame arrives, and
    // the desktop's own socket binds A by her pinned member
    assert!(carol.courier.offer(kh("w1")).await);
    assert!(
        until(4000, || carol.reachable.holds(&kh("w1"))
            && desktop.reachable.holds(&kh("carol")))
        .await,
        "both hold the path"
    );
    carol
        .courier
        .send(
            kh("w1"),
            KIND_APPLICATION,
            b"to the desktop, directly".to_vec(),
        )
        .await
        .expect("sent");
    let (from, d) = next(&mut desktop.app, 3000).await.expect("delivered");
    assert_eq!(from, kh("carol"));
    assert!(
        matches!(&d, Dispatched::Application { bytes, .. } if bytes == b"to the desktop, directly")
    );
}

/// A light client whose direct path runs the control streams off its own
/// kernel (`wire-format.md` §9.1): it introduces itself on a dial, takes
/// a dialler's introduction as a cold call, and answers a peer's prekey
/// request from its own material.  `forget` names the parties this
/// client is to know nothing of — neither pinned nor among what it
/// verifies by — which is what a client restored from its phrase is to
/// everybody.
fn light_with(
    name: &'static str,
    serving: &Arc<dyn Serving>,
    inboxes: &Inboxes,
    forget: &[&str],
) -> (Party, LightDirect) {
    let inlet = Inlet::default();
    let slot: Arc<std::sync::Mutex<Option<rhtn_adaptors::actor::Handle>>> = Arc::default();
    let gate: Gate = {
        let slot = slot.clone();
        Arc::new(move |peer| {
            let peer = *peer;
            match slot.lock().unwrap().clone() {
                Some(h) => h.with_blocking(move |c| c.store.contacts.contains_key(&peer)),
                None => false,
            }
        })
    };
    let pins = rhtn_transport::tls::Pins::new();
    for n in NAMES {
        if !forget.contains(&n) {
            pins.pin_identity(&id(n).public);
        }
    }
    let control = {
        let (s1, s2, s3) = (slot.clone(), slot.clone(), slot.clone());
        Control {
            introduce: Arc::new(move || {
                let h = s1.lock().unwrap().clone();
                Box::pin(async move {
                    match h {
                        Some(h) => h.with(|c| c.own_introduction().ok()).await,
                        None => None,
                    }
                })
            }),
            introduced: Arc::new(move |from, bytes| {
                let h = s2.lock().unwrap().clone();
                Box::pin(async move {
                    match h {
                        Some(h) => {
                            h.with(move |c| {
                                c.known.iter().any(|k| k.keyhash == from)
                                    || c.take_cold_introduction(&from, &bytes)
                            })
                            .await
                        }
                        None => false,
                    }
                })
            }),
            answer: Arc::new(move |from, bytes| {
                let h = s3.lock().unwrap().clone();
                Box::pin(async move {
                    match h {
                        Some(h) => {
                            h.with(move |c| c.answer_prekey_request(&from, &bytes))
                                .await
                        }
                        None => None,
                    }
                })
            }),
        }
    };
    let direct = LightDirect::bind(
        rhtn_transport::tls::Party::of(Arc::new(id(name))),
        pins,
        Default::default(),
        Reachable::default(),
        loopback(),
        None,
        None,
        gate,
        inlet.inbound(),
        control,
    )
    .expect("bound");
    let reachable = direct.reachable();
    let handle = spawn_client(name, Config::default(), reachable.clone());
    let gone: Vec<Keyhash> = forget.iter().map(|n| kh(n)).collect();
    handle.with_blocking(move |c| c.known.retain(|k| !gone.contains(&k.keyhash)));
    *slot.lock().unwrap() = Some(handle.clone());
    let (courier, app) = Courier::new(handle.clone(), serving.clone(), Arc::new(direct.clone()));
    inlet.bind(courier.inbound());
    inboxes.host(kh(name), courier.inbound());
    (
        Party {
            handle,
            courier,
            app,
            reachable,
        },
        direct,
    )
}

/// **A dialler the acceptor cannot bind presents its introduction first on
/// the connection, and is taken as a cold call; a peer's prekey request
/// is answered over the path** (`wire-format.md` §9.1, §7.8; author,
/// 2026-10-30: no node is needed for a recovery).  Bob knows nothing of
/// alice — not pinned, not among what he verifies by — and holds no
/// node.  Alice, holding bob as a contact, dials his socket: the handshake
/// presents a key bob binds by nothing, her introduction follows on the
/// first control stream, bob takes it, pins her and holds the path; and
/// her request for his bundle, on the same path, comes back answered from
/// his own material, one-time key and all.
// acceptance: TRN-29
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_stranger_that_introduces_itself_on_the_dial_is_held_and_handed_a_bundle_over_the_path() {
    let inboxes = Inboxes::default();
    let serving: Arc<dyn Serving> = Arc::new(rhtn_adaptors::serving::Nobody);
    let (bob, bob_direct) = light_with("bob", &serving, &inboxes, &["alice"]);
    let (alice, alice_direct) = light_with("alice", &serving, &inboxes, &[]);
    hold(&alice, "bob");
    assert!(
        !bob.handle
            .with_blocking(|c| c.known.iter().any(|k| k.keyhash == kh("alice"))),
        "bob has never heard of alice"
    );
    // bob's gate would gather nothing for a stranger, so alice is handed
    // his host address as a candidate exchange would carry it
    let cand = rhtn_transport::traversal::Candidate {
        kind: rhtn_transport::traversal::CandidateKind::Host,
        addr: bob_direct.addr().expect("bound"),
    };
    assert!(
        alice_direct.open(kh("bob"), vec![cand]).await,
        "alice holds the path: bob took her introduction"
    );
    assert!(
        until(3000, || bob.reachable.holds(&kh("alice"))).await,
        "and so does bob"
    );
    assert!(
        bob.handle
            .with_blocking(|c| c.store.contacts.contains_key(&kh("alice"))),
        "alice is a contact of bob's now, by the cold call"
    );
    // the bundle, asked of bob himself on the path he holds
    let bob_device = *id("bob").public.ed.as_bytes();
    let req = rhtn_client::payload::one_time_request(kh("bob"), bob_device, [7u8; 16]);
    let reply = alice_direct
        .request(kh("bob"), req)
        .await
        .expect("bob answered on the path");
    let r = rhtn_archive::prekey::PrekeyReply::decode(&reply).expect("a prekey reply");
    assert_eq!(r.nonce, [7u8; 16]);
    assert_eq!(r.bundles.len(), 1, "his signed bundle");
    assert!(r.one_time.is_some(), "and a one-time key");
    // a request for somebody else's material is not answered
    let other = rhtn_client::payload::one_time_request(kh("carol"), bob_device, [8u8; 16]);
    assert!(alice_direct.request(kh("bob"), other).await.is_none());
    let _ = alice;
}
