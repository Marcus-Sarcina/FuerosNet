//! The kernel a shell binds to, over real nodes: failover driven inside
//! it, the status a screen reads, the sibling list kept across a restart,
//! and a cold start from that list with the serving node dark.

mod common;

use common::*;
use rhtn_ffi::client::Participant;
use rhtn_ffi::device::*;
use rhtn_ffi::net::{Event, PathPolicy, Status};
use rhtn_ffi::types::*;
use rhtn_node::resolution::{AnchorTable, Ingestion};
use rhtn_node::runtime::LiveNode;
use rhtn_transport::session::NetworkPoint;
use std::sync::{Arc, Mutex};

/// A phone with nothing to measure and app-private storage.
#[derive(Default)]
struct Shell {
    store: Mutex<std::collections::BTreeMap<String, Vec<u8>>>,
}

impl Proximity for Shell {
    fn supported(&self) -> Vec<Channel> {
        vec![]
    }
    fn run(&self, _: Channel, _: Vec<u8>) -> ChannelOutcome {
        ChannelOutcome::Unavailable
    }
    fn resolution_m(&self, _: Channel) -> Option<u64> {
        None
    }
}
impl Camera for Shell {
    fn capture(&self, _: Ask) -> Vec<u8> {
        vec![7; 64]
    }
}
impl Clock for Shell {
    fn now_ms(&self) -> u64 {
        SIM_EPOCH * 1000
    }
    fn wait_ms(&self, _: u64) {}
}
impl Random for Shell {
    fn fill(&self, n: u32) -> Vec<u8> {
        let mut out = Vec::with_capacity(n as usize);
        while out.len() < n as usize {
            out.extend_from_slice(&rhtn_transport::tls::random_bytes::<64>());
        }
        out.truncate(n as usize);
        out
    }
}
impl Operator for Shell {
    fn ask(&self, _: String) -> bool {
        true
    }
}
impl Notices for Shell {
    fn told(&self, _: Told) {}
}
impl Storage for Shell {
    fn read(&self, name: String) -> Option<Vec<u8>> {
        self.store.lock().unwrap().get(&name).cloned()
    }
    fn write(&self, name: String, bytes: Vec<u8>) -> bool {
        self.store.lock().unwrap().insert(name, bytes);
        true
    }
}

fn platform_of(s: Arc<Shell>) -> Arc<Platform> {
    Arc::new(Platform {
        proximity: s.clone(),
        camera: s.clone(),
        clock: s.clone(),
        random: s.clone(),
        operator: s.clone(),
        notices: s.clone(),
        storage: s,
    })
}

/// No sessions at all: for a view that is only being set up.
struct Quiet;
impl rhtn_node::Adjacency for Quiet {
    fn peers(&self) -> Vec<rhtn_archive::Keyhash> {
        Vec::new()
    }
    fn send(&self, _: &rhtn_archive::Keyhash, _: u64, _: &[u8]) {}
    fn request(&self, _: &rhtn_archive::Keyhash, _: u64, _: &[u8]) -> bool {
        false
    }
}

fn anchors() -> AnchorTable {
    AnchorTable::new(0, Ingestion::UnverifiedGossip)
}

fn material(n: &str) -> Vec<u8> {
    id(n).public.key_material()
}

fn idv(n: &str) -> Vec<u8> {
    kh(n).to_vec()
}

/// Failover is the kernel's (`light-client-requirements.md` §4): three
/// missed intervals move the session to the sibling the serving node
/// named, with nothing asked of the shell, which reads the change as
/// events and then as the status; the list is written with the state, and
/// a fresh process whose serving node is dark attaches to the sibling from
/// it, cold; a list that does not load whole is no list.
// acceptance: DMN-13
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn the_kernel_fails_over_to_the_named_sibling_and_starts_cold_from_the_list_it_kept() {
    let _serial = serial().await;
    // R (alice2) is the root; N (bob) and S (carol) are its infra
    // children, so siblings; alice is a light client under N
    let mut sg = Signers::new();
    let a_n = sg.adopt("bob", "alice2", "alice2", &[0], 1);
    let a_s = sg.adopt("carol", "alice2", "alice2", &[1], 2);
    let a_c = sg.adopt("alice", "bob", "alice2", &[0, 0], 3);
    let infra = ["alice2", "bob", "carol"];
    let records = [&a_n, &a_s, &a_c];
    let s_view = view_of(
        "carol",
        table_of("carol", &sg, &records, &infra),
        "alice2",
        &[1],
        sg.clock,
    );
    let s = LiveNode::start(node_cfg("carol", 1), s_view, ids(), anchors());
    let mut n_view = view_of(
        "bob",
        table_of("bob", &sg, &records, &infra),
        "alice2",
        &[0],
        sg.clock,
    );
    n_view.set_slot(0, Some(kh("alice")), sg.clock);
    n_view.attached.insert(kh("alice"));
    // N holds S's endpoint record: what makes S a sibling it can name with
    // an address, which is the only kind the ack carries
    let s_record = rhtn_node::resolution::endpoint_record(
        &id("carol"),
        &[NetworkPoint::from_socket(s.addr).unwrap()],
        rhtn_archive::tx::Seqno {
            series: 1,
            counter: 1,
        },
    );
    n_view.take_object(
        &Quiet,
        &kh("carol"),
        rhtn_node::store::KIND_ENDPOINT_RECORD,
        &s_record,
        &ids(),
    );
    // and then N goes silent: its heartbeats (frame type 3) are dropped on
    // the way out, so alice misses three one-second intervals
    let mut n_cfg = node_cfg("bob", 1);
    n_cfg.filter = Some(drop_heartbeats());
    let n = LiveNode::start(n_cfg, n_view, ids(), anchors());
    let n_addr = n.addr.to_string();
    let known: Vec<Vec<u8>> = ["alice", "bob", "carol", "alice2"]
        .iter()
        .map(|x| material(x))
        .collect();
    let seeds = |name: &str| {
        let mut b =
            rhtn_codec::cose::sha256(format!("rhtn-test-vectors:{name}:ed25519-seed").as_bytes())
                .to_vec();
        b.extend_from_slice(&rhtn_codec::cose::sha256(
            format!("rhtn-test-vectors:{name}:ml-dsa-65-seed").as_bytes(),
        ));
        b
    };
    let shell = Arc::new(Shell::default());
    let start = |shell: Arc<Shell>| {
        let (known, seeds) = (known.clone(), seeds("alice"));
        tokio::task::spawn_blocking(move || {
            Participant::start(seeds, known, platform_of(shell)).expect("starts")
        })
    };
    let alice = Arc::new(start(shell.clone()).await.unwrap());
    assert_eq!(alice.status(), Status::Detached);
    let a = {
        let (q, addr) = (alice.clone(), n_addr.clone());
        tokio::task::spawn_blocking(move || q.attach(idv("bob"), vec![addr], vec![]))
            .await
            .unwrap()
            .expect("attaches to N")
    };
    assert!(a.primary, "in N's subtree");
    assert_eq!(
        alice.status(),
        Status::Attached {
            serving: idv("bob"),
            primary: true
        }
    );
    let list = shell
        .read("siblings".into())
        .expect("the list N pushed is written");
    assert_eq!(
        rhtn_transport::session::decode_sibling_update(&list)
            .unwrap()
            .iter()
            .map(|r| r.keyhash)
            .collect::<Vec<_>>(),
        vec![kh("carol")],
        "S, with its address and material"
    );

    // a cold start from that list: a second process, N's address answering
    // nothing, attaches to S from what the first run kept, never having
    // held a session itself
    {
        let q = alice.clone();
        tokio::task::spawn_blocking(move || q.detach())
            .await
            .unwrap()
            .expect("detaches");
    }
    drop(alice);
    let alice2 = Arc::new(start(shell.clone()).await.unwrap());
    let a = {
        let q = alice2.clone();
        tokio::task::spawn_blocking(move || {
            q.attach(idv("bob"), vec!["127.0.0.1:1".into()], vec![])
        })
        .await
        .unwrap()
        .expect("attaches to the cached sibling, cold")
    };
    assert_eq!(a.serving, idv("carol"));
    assert!(!a.primary, "degraded, and the shell is told so");
    assert_eq!(
        alice2.status(),
        Status::Attached {
            serving: idv("carol"),
            primary: false
        }
    );
    {
        let q = alice2.clone();
        tokio::task::spawn_blocking(move || q.detach())
            .await
            .unwrap()
            .expect("detaches");
    }
    drop(alice2);

    // back on N, from the same storage, for the failover N's silence
    // forces
    let alice = Arc::new(start(shell.clone()).await.unwrap());
    {
        let (q, addr) = (alice.clone(), n_addr.clone());
        tokio::task::spawn_blocking(move || q.attach(idv("bob"), vec![addr], vec![]))
            .await
            .unwrap()
            .expect("attaches to N again");
    }

    // the failover happens inside the kernel; the shell sees it as events
    // and then as the status
    let next = |p: Arc<Participant>| tokio::task::spawn_blocking(move || p.next_event(15_000));
    assert_eq!(
        next(alice.clone()).await.unwrap(),
        Some(Event::Connection(Status::Reconnecting { from: idv("bob") })),
        "judged unreachable after three missed intervals"
    );
    assert_eq!(
        next(alice.clone()).await.unwrap(),
        Some(Event::Connection(Status::Attached {
            serving: idv("carol"),
            primary: false
        })),
        "landed on S, degraded: alice is not in S's subtree"
    );
    assert_eq!(
        alice.status(),
        Status::Attached {
            serving: idv("carol"),
            primary: false
        }
    );
    assert!(alice.attached());
    // the degraded session carries traffic: maintenance is a conversation
    // with the node now serving
    {
        let q = alice.clone();
        tokio::task::spawn_blocking(move || q.maintain())
            .await
            .unwrap()
            .expect("maintains on S");
    }
    {
        let q = alice.clone();
        tokio::task::spawn_blocking(move || q.detach())
            .await
            .unwrap()
            .expect("detaches");
    }
    assert_eq!(alice.status(), Status::Detached);
    drop(alice);

    // a list that does not load whole is no list: with it damaged and N
    // dark, the attach is refused and says why
    let broken = Arc::new(Shell::default());
    broken.write("siblings".into(), b"\xa1\x01\x81\x00".to_vec());
    let alice3 = Arc::new(start(broken).await.unwrap());
    let e = {
        let q = alice3.clone();
        tokio::task::spawn_blocking(move || {
            q.attach(idv("bob"), vec!["127.0.0.1:1".into()], vec![])
        })
        .await
        .unwrap()
        .expect_err("nothing to fail over to")
    };
    assert!(e.reason().contains("no cached siblings"), "{e}");
    assert_eq!(alice3.status(), Status::Detached);
    let _ = (n, s);
}

/// The direct path is joined behind the boundary (design §14.1.1,
/// `light-client-requirements.md` §5): two clients of one node inside each
/// other's horizon open it from their own sends and the node relays none
/// of what follows; the person's override holds in both directions, and
/// the catalog sweep rides the same session.
// acceptance: DMN-13
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn the_kernel_joins_the_direct_path_honours_the_override_and_sweeps_the_catalog() {
    let _serial = serial().await;
    // alice and carol are light clients under bob, siblings in its subtree
    // and inside each other's horizon; w1 is under w2, outside it
    let mut sg = Signers::new();
    let a_a = sg.adopt("alice", "bob", "bob", &[0], 1);
    let a_c = sg.adopt("carol", "bob", "bob", &[1], 2);
    let a_w = sg.adopt("w1", "w2", "w2", &[0], 3);
    let records = [&a_a, &a_c, &a_w];
    let mut view = view_of(
        "bob",
        table_of("bob", &sg, &records, &["bob", "w2"]),
        "bob",
        &[],
        sg.clock,
    );
    view.set_slot(0, Some(kh("alice")), sg.clock);
    view.set_slot(1, Some(kh("carol")), sg.clock);
    view.attached.insert(kh("alice"));
    view.attached.insert(kh("carol"));
    // bob serves one catalog entry: a resource of alice's, registered by
    // her
    let entry = rhtn_archive::catalog::CatalogEntry::build(
        &id("alice"),
        &rhtn_archive::catalog::EntryFields {
            resource: kh("w1"),
            service_type: "_rhtn-files._tcp".into(),
            instance: "alice's files".into(),
            endpoint: b"files.alice.example:4433".to_vec(),
            connect_scope: None,
            metadata: None,
            data_practice: Some(0),
        },
    );
    let registration = rhtn_archive::catalog::ResourceRegistration {
        entry,
        scope: None,
        nonce: [9; 16],
    }
    .encode();
    assert!(
        view.catalog
            .register(&ids(), &kh("alice"), &registration)
            .is_some(),
        "registered"
    );
    let n = LiveNode::start(node_cfg("bob", 30), view, ids(), anchors());
    for r in records {
        n.originate_transaction(&r.bytes);
    }
    // bob's store holds the records its table folded, so the shape reaches
    // the clients it serves at attach and their horizons place each other
    for r in records {
        n.originate_transaction(&r.bytes);
    }
    let n_addr = n.addr.to_string();
    let known: Vec<Vec<u8>> = ["alice", "bob", "carol", "w1", "w2"]
        .iter()
        .map(|x| material(x))
        .collect();
    let seeds = |name: &str| {
        let mut b =
            rhtn_codec::cose::sha256(format!("rhtn-test-vectors:{name}:ed25519-seed").as_bytes())
                .to_vec();
        b.extend_from_slice(&rhtn_codec::cose::sha256(
            format!("rhtn-test-vectors:{name}:ml-dsa-65-seed").as_bytes(),
        ));
        b
    };
    let start = |name: &'static str| {
        let (known, seeds) = (known.clone(), seeds(name));
        tokio::task::spawn_blocking(move || {
            Participant::start(seeds, known, platform_of(Arc::new(Shell::default())))
                .expect("starts")
        })
    };
    let alice = Arc::new(start("alice").await.unwrap());
    let carol = Arc::new(start("carol").await.unwrap());
    let attach = |p: Arc<Participant>, pop: Vec<Vec<u8>>| {
        let addr = n_addr.clone();
        tokio::task::spawn_blocking(move || p.attach(idv("bob"), vec![addr], pop))
    };
    attach(carol.clone(), vec![])
        .await
        .unwrap()
        .expect("carol attaches");
    attach(alice.clone(), vec![idv("carol")])
        .await
        .unwrap()
        .expect("alice attaches");
    attach(carol.clone(), vec![idv("alice")])
        .await
        .unwrap()
        .expect("carol sweeps");
    // **what bob replays is its own acts**: it countersigned both
    // adoptions, so they are its to keep and to hand a client that
    // attaches after they passed (`infra-client-requirements.md` §4.3
    // [author, 2026-09-23]), and the two clients place each other from them
    assert!(
        until(8000, || alice.distance(idv("carol")).is_some()
            && carol.distance(idv("alice")).is_some())
        .await,
        "each places the other in its horizon, from the acts bob is a party to"
    );
    assert_eq!(alice.path(), PathPolicy::Auto);
    assert!(!alice.direct_to(idv("carol")), "nothing offered yet");

    // the first send offers candidates over the relay and the path opens
    // both ways; what follows goes around the node
    let send = |p: Arc<Participant>, to: Vec<u8>, what: &'static [u8]| {
        tokio::task::spawn_blocking(move || p.send(to, KIND_APPLICATION, what.to_vec()))
    };
    let next = |p: Arc<Participant>| tokio::task::spawn_blocking(move || p.next_event(5000));
    send(alice.clone(), idv("carol"), b"first")
        .await
        .unwrap()
        .expect("sent");
    assert_eq!(
        next(carol.clone()).await.unwrap(),
        Some(Event::Payload {
            from: idv("alice"),
            bytes: b"first".to_vec()
        })
    );
    assert!(
        until(4000, || alice.direct_to(idv("carol"))
            && carol.direct_to(idv("alice")))
        .await,
        "both hold the path"
    );
    let relayed = n.node.relayed();
    assert!(
        relayed >= 1,
        "the candidates and the first message went through bob"
    );
    send(alice.clone(), idv("carol"), b"second")
        .await
        .unwrap()
        .expect("sent");
    assert_eq!(
        next(carol.clone()).await.unwrap(),
        Some(Event::Payload {
            from: idv("alice"),
            bytes: b"second".to_vec()
        })
    );
    send(carol.clone(), idv("alice"), b"third")
        .await
        .unwrap()
        .expect("sent");
    assert_eq!(
        next(alice.clone()).await.unwrap(),
        Some(Event::Payload {
            from: idv("carol"),
            bytes: b"third".to_vec()
        })
    );
    assert_eq!(n.node.relayed(), relayed, "bob relayed none of it");

    // the override, both ways: relay only sends through bob though the
    // path is held; direct only to a peer with no path is unsent
    alice.set_path(PathPolicy::RelayOnly);
    send(alice.clone(), idv("carol"), b"fourth")
        .await
        .unwrap()
        .expect("sent");
    assert_eq!(
        next(carol.clone()).await.unwrap(),
        Some(Event::Payload {
            from: idv("alice"),
            bytes: b"fourth".to_vec()
        })
    );
    assert_eq!(n.node.relayed(), relayed + 1, "through bob by choice");
    alice.set_path(PathPolicy::Auto);

    // the catalog: swept over the session, held with the node that served
    // it, and shown as values
    {
        let q = alice.clone();
        tokio::task::spawn_blocking(move || q.browse())
            .await
            .unwrap()
            .expect("swept");
    }
    let items = alice.catalog();
    assert_eq!(items.len(), 1);
    let it = &items[0];
    assert_eq!(it.node, idv("bob"));
    assert_eq!(it.resource, idv("w1"));
    assert_eq!(it.owner, idv("alice"));
    assert_eq!(it.service_type, "_rhtn-files._tcp");
    assert_eq!(it.data_practice, Some(0));
    assert!(!it.truncated && !it.stale);

    // direct only: to carol, whose path is held, still goes around bob
    alice.set_path(PathPolicy::DirectOnly);
    send(alice.clone(), idv("carol"), b"fifth")
        .await
        .unwrap()
        .expect("sent");
    assert_eq!(
        next(carol.clone()).await.unwrap(),
        Some(Event::Payload {
            from: idv("alice"),
            bytes: b"fifth".to_vec()
        })
    );
    assert_eq!(n.node.relayed(), relayed + 1);
    // carol leaves: the held path dies with her socket, and with the relay
    // forbidden the next message is unsent, and said so
    {
        let q = carol.clone();
        tokio::task::spawn_blocking(move || q.detach())
            .await
            .unwrap()
            .expect("detaches");
    }
    drop(carol);
    tokio::time::sleep(std::time::Duration::from_millis(300)).await;
    let e = send(alice.clone(), idv("carol"), b"nowhere")
        .await
        .unwrap()
        .expect_err("no path and no relay");
    assert!(e.reason().contains("unsent"), "{e}");
    assert_eq!(n.node.relayed(), relayed + 1, "and bob saw nothing");
    // the fallback, once allowed again: through bob, queued for carol
    alice.set_path(PathPolicy::Auto);
    send(alice.clone(), idv("carol"), b"sixth")
        .await
        .unwrap()
        .expect("relayed");
    assert_eq!(n.node.relayed(), relayed + 2);
    assert_eq!(n.node.queued(&kh("carol")), 1, "waiting for carol at bob");
}
