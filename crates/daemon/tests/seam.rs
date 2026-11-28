//! The seam the bench meets: the phone's kernel (`rhtn_ffi::client::
//! Participant`, the facade the Android and desktop shells bind to) against
//! the node `rhtnd` runs (`rhtn_daemon::service::Service`), started from a
//! configuration file and restarted on the state it wrote.
//!
//! **Neither side is a stand-in.**  `ffi/tests/boundary.rs` drives the
//! kernel against a `LiveNode` it builds, and `tests/lifecycle.rs` and
//! `tests/scenarios.rs` drive the daemon with raw sessions; this file is
//! where the kernel's attach and messaging meet the daemon's
//! configuration, persistence and restart, which is where the bench's
//! bugs have been.
//!
//! **Each daemon runs on a runtime of its own, and a stop ends that
//! runtime**: what the process held is written back first, as `run` does
//! on the way out, and then every task it had goes with it, so the restart
//! starts on nothing but what is on disk.  The first two tests serve the
//! restart at a fresh port and attach the kernel there; the last holds
//! that a restart takes back the port it served before.

mod common;

use rhtn_crypto::identity::testkit::test_identity;
use rhtn_daemon::config::Config;
use rhtn_daemon::service::Service;
use rhtn_ffi::client::Participant;
use rhtn_ffi::device::*;
use rhtn_ffi::net::Event;
use rhtn_ffi::types::*;
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// The daemon's identity, and the two people attached to it.
const NODE: &str = "bob";

/// A phone's hardware, as much of it as an attach and a message touch.
#[derive(Default)]
struct Shell {
    /// The app-private storage a phone gives the kernel, by name.
    store: Mutex<BTreeMap<String, Vec<u8>>>,
    /// The storage key the platform keeps (`light-client-requirements.md`
    /// §9).
    custody: Mutex<Option<Vec<u8>>>,
}

impl Custody for Shell {
    fn key(&self) -> Option<Vec<u8>> {
        self.custody.lock().unwrap().clone()
    }
    fn keep(&self, key: Vec<u8>) -> bool {
        *self.custody.lock().unwrap() = Some(key);
        true
    }
    fn unsealed(&self) -> bool {
        false
    }
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

impl Camera for Shell {
    fn capture(&self, _: Ask) -> Vec<u8> {
        vec![7u8; 64]
    }
    fn probe(&self) -> Vec<u8> {
        vec![7u8; 64]
    }
}

impl Clock for Shell {
    /// **The system clock, as a phone's is**: the daemon reads its own,
    /// and a kernel on a fixed instant would be a device the daemon
    /// disagrees with about the time.
    fn now_ms(&self) -> u64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64
    }
    fn wait_ms(&self, _ms: u64) {}
}

impl Random for Shell {
    fn fill(&self, n: u32) -> Vec<u8> {
        // real randomness: two kernels drawing one constant would seal
        // under the same ephemeral keys
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

fn platform() -> Arc<Platform> {
    let shell = Arc::new(Shell::default());
    Arc::new(Platform {
        camera: shell.clone(),
        clock: shell.clone(),
        random: shell.clone(),
        operator: shell.clone(),
        notices: shell.clone(),
        storage: shell.clone(),
        custody: shell,
        diagnostics: Arc::new(Silent),
        engine: None,
    })
}

/// The 64 bytes of seed `testkit` derives for `name`: what the kernel is
/// started from, and what the daemon reads from its identity file.
fn seeds(name: &str) -> Vec<u8> {
    let mut b =
        rhtn_codec::cose::sha256(format!("rhtn-test-vectors:{name}:ed25519-seed").as_bytes())
            .to_vec();
    b.extend_from_slice(&rhtn_codec::cose::sha256(
        format!("rhtn-test-vectors:{name}:ml-dsa-65-seed").as_bytes(),
    ));
    b
}

fn id(name: &str) -> Vec<u8> {
    test_identity(name).public.keyhash.to_vec()
}

fn kh(name: &str) -> [u8; 32] {
    test_identity(name).public.keyhash
}

fn hex(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect()
}

/// A kernel started for `name`, holding the material of everyone here.
fn kernel(name: &str) -> Arc<Participant> {
    let known = ["alice", NODE, "carol"]
        .iter()
        .map(|n| test_identity(n).public.key_material())
        .collect();
    Arc::new(Participant::start(seeds(name), known, platform()).expect("the kernel starts"))
}

/// A daemon's state directory and the two files it is started from.
struct Layout {
    dir: PathBuf,
    config: PathBuf,
    peers: PathBuf,
}

/// **A fresh directory holding an operator's configuration**, the
/// identity file and the peers file it names, and nothing else: every
/// other path the configuration names is the daemon's to create.  It
/// listens on loopback at `port`, 0 leaving the port to the kernel.
fn layout(tag: &str, port: u16) -> Layout {
    let dir = std::env::temp_dir().join(format!("rhtnd-seam-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let (config, peers, identity) = (
        dir.join("rhtnd.conf"),
        dir.join("peers"),
        dir.join("identity.key"),
    );
    std::fs::write(&identity, seeds(NODE)).unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&identity, std::fs::Permissions::from_mode(0o600)).unwrap();
    }
    // the two people the node takes sessions from, and never itself
    std::fs::write(
        &peers,
        ["alice", "carol"]
            .map(|n| format!("{}\n", hex(&test_identity(n).public.key_material())))
            .concat(),
    )
    .unwrap();
    std::fs::write(
        &config,
        format!(
            "identity = \"{}\"\nlisten = \"127.0.0.1:{}\"\nqueue = \"{}\"\nkeys = \"{}\"\ntopology = \"{}\"\narchive = \"{}\"\nheartbeat = 30\ningestion = \"unverified-gossip\"\n\n[allowance]\nrequests = 120\nseconds = 60\n",
            identity.display(),
            port,
            dir.join("queue").display(),
            dir.join("keys").display(),
            dir.join("topology").display(),
            dir.join("archive").display()
        ),
    )
    .unwrap();
    Layout { dir, config, peers }
}

/// A running daemon and the runtime that is its process.
struct Daemon {
    rt: tokio::runtime::Runtime,
    service: Service,
}

impl Daemon {
    /// **Started from the files alone**, as `rhtnd` is: the configuration
    /// read strictly and the service brought up on a runtime of its own.
    fn start(l: &Layout) -> Daemon {
        let cfg = Config::read(&l.config).expect("the configuration reads");
        let rt = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .enable_all()
            .build()
            .unwrap();
        let service = rt
            .block_on(Service::start(&cfg, &l.peers))
            .expect("the daemon starts");
        Daemon { rt, service }
    }

    /// Where it serves, as a phone is given it.
    fn addr(&self) -> String {
        self.service.node.addr.to_string()
    }

    /// Whether the daemon holds a session for `name`.
    fn holds(&self, name: &str) -> bool {
        self.service.node.node.sessions().contains(&kh(name))
    }

    /// What the daemon has queued for `name`.
    fn queued(&self, name: &str) -> usize {
        self.service.node.node.queued(&kh(name))
    }

    /// **Written back, then gone**: what `run` does on a signal, and then
    /// the end of the process, every task with it.  What is returned is
    /// what of the node is still alive afterwards, which in a process that
    /// had ended would be nothing.
    fn stop(self) -> Left {
        let Daemon { rt, service } = self;
        service.persist().expect("the daemon writes back");
        let (node, sessions, view, socket) = (
            Arc::downgrade(&service.node),
            Arc::downgrade(&service.node.node),
            Arc::downgrade(&service.node.view),
            Arc::downgrade(&service.node.traversal),
        );
        {
            let _inside = rt.enter();
            drop(service);
        }
        rt.shutdown_timeout(Duration::from_secs(5));
        Left {
            node: node.strong_count(),
            sessions: sessions.strong_count(),
            view: view.strong_count(),
            socket: socket.strong_count(),
        }
    }
}

/// **What of a stopped daemon's node is still referenced**, by part: the
/// node, its session layer, its view and the socket it served on.
#[derive(Debug, PartialEq, Eq)]
struct Left {
    node: usize,
    sessions: usize,
    view: usize,
    socket: usize,
}

/// Attach `p` to the daemon at `addr`, sweeping `population`.
fn attach(p: &Participant, addr: &str, population: &[&str]) -> rhtn_ffi::net::Attached {
    p.attach(
        id(NODE),
        vec![addr.to_string()],
        population.iter().map(|n| id(n)).collect(),
    )
    .expect("the kernel attaches to the daemon")
}

/// **The next payload `p` is handed, within ten seconds**: the kernel's
/// own notices of what changed, and a connection status, come up the same
/// stream and are kept for the failure message rather than taken as the
/// answer.
fn payload(p: &Participant) -> (Vec<u8>, Vec<u8>) {
    let end = Instant::now() + Duration::from_secs(10);
    let mut other = Vec::new();
    while Instant::now() < end {
        match p.next_event(1000) {
            Some(Event::Payload { from, kind, bytes }) => {
                assert_eq!(kind, KIND_APPLICATION, "the application's own kind");
                return (from, bytes);
            }
            Some(e) => other.push(e),
            None => {}
        }
    }
    panic!("no payload within ten seconds; what came instead: {other:?}")
}

// functional: OPS-009
/// **A kernel attaches to a daemon started from a configuration, and
/// attaches again once the daemon has restarted on the state it wrote.**
/// The restarted daemon holds no session until the kernel's second
/// attach, so the second session is the restarted daemon's own and not
/// the first one surviving it.
#[test]
fn a_kernel_attaches_to_rhtnd_and_again_after_it_restarts_on_its_own_state() {
    let l = layout("attach", 0);
    let daemon = Daemon::start(&l);
    let addr = daemon.addr();
    let alice = kernel("alice");
    assert!(!alice.attached(), "nothing is attached before an attach");

    let a = attach(&alice, &addr, &[]);
    assert_eq!(a.serving, id(NODE), "the daemon's identity, from its file");
    assert_eq!(a.queued, 0, "nothing waits for a first attach");
    assert!(alice.attached());
    assert!(
        daemon.holds("alice"),
        "the daemon holds the kernel's session"
    );

    // the daemon stops, and only what it wrote is left
    let _ = daemon.stop();
    for written in ["keys", "topology", "archive"] {
        assert!(
            l.dir.join(written).exists(),
            "the daemon wrote its {written} into the state directory"
        );
    }

    // started again from the same files, at the address it now serves
    let daemon = Daemon::start(&l);
    let addr = daemon.addr();
    assert!(
        !daemon.holds("alice"),
        "control: a restarted daemon holds no session before one is opened"
    );
    let a = attach(&alice, &addr, &[]);
    assert_eq!(a.serving, id(NODE), "the same identity after the restart");
    assert!(alice.attached());
    assert!(
        daemon.holds("alice"),
        "the kernel's second session is with the restarted daemon"
    );
    let _ = daemon.stop();
    let _ = std::fs::remove_dir_all(&l.dir);
}

// functional: OPS-009
/// **Two kernels attached to one daemon exchange direct messages through
/// it, and a message queued for one who is away survives the daemon's
/// restart**: it is read back from the queue directory, said to be
/// waiting at the next attach, and opens there attributed to its sender,
/// under the node key the daemon read back from its keys directory; the
/// reply after the restart reaches the sender.
#[test]
fn a_message_crosses_rhtnd_and_one_queued_for_an_absent_recipient_survives_its_restart() {
    let l = layout("message", 0);
    let daemon = Daemon::start(&l);
    let addr = daemon.addr();
    let (alice, carol) = (kernel("alice"), kernel("carol"));
    // each holds the other's card, as two who have met do
    for (p, q) in [(&alice, &carol), (&carol, &alice)] {
        p.take_introduction(q.own_introduction().expect("a card"))
            .expect("the card is taken");
    }
    attach(&carol, &addr, &[]);
    attach(&alice, &addr, &["carol"]);
    attach(&carol, &addr, &["alice"]);

    // a message each way, both through the daemon
    alice
        .send(id("carol"), KIND_APPLICATION, b"first".to_vec())
        .expect("sent");
    assert_eq!(
        payload(&carol),
        (id("alice"), b"first".to_vec()),
        "opened by the recipient and attributed to the sender"
    );
    carol
        .send(id("alice"), KIND_APPLICATION, b"second".to_vec())
        .expect("sent");
    assert_eq!(payload(&alice), (id("carol"), b"second".to_vec()));

    // carol goes away cleanly, and alice writes to her meanwhile: the
    // daemon queues it for her
    carol.detach().expect("detaches");
    let before = daemon.queued("carol");
    alice
        .send(id("carol"), KIND_APPLICATION, b"third".to_vec())
        .expect("queued at the daemon");
    assert_eq!(
        daemon.queued("carol"),
        before + 1,
        "queued for the person, who is away"
    );

    // the daemon restarts on the state it wrote
    let _ = daemon.stop();
    let daemon = Daemon::start(&l);
    let addr = daemon.addr();
    assert_eq!(
        daemon.queued("carol"),
        before + 1,
        "the queue was read back from the state directory"
    );

    // carol comes back, is told one is waiting, and it opens
    let a = attach(&carol, &addr, &[]);
    assert_eq!(a.queued, 1, "the restarted daemon says one is waiting");
    assert_eq!(
        payload(&carol),
        (id("alice"), b"third".to_vec()),
        "sealed before the restart, opened after it"
    );

    // and the conversation continues across the restarted daemon
    attach(&alice, &addr, &[]);
    carol
        .send(id("alice"), KIND_APPLICATION, b"fourth".to_vec())
        .expect("sent");
    assert_eq!(
        payload(&alice),
        (id("carol"), b"fourth".to_vec()),
        "the reply reaches the sender through the restarted daemon"
    );
    let _ = daemon.stop();
    let _ = std::fs::remove_dir_all(&l.dir);
}

/// **A daemon restarted in one process binds the port it served on
/// before**, as `rhtnd` restarted by its supervisor does: the phone holds
/// that address, and a restart that cannot take it again is a node that
/// has moved.  The first service is written back, dropped inside its
/// runtime, and the runtime shut down, which ends every task it spawned.
///
/// **This failed before `LiveNode` broke its own cycle on drop**: the
/// second start panicked with `AddrInUse`, the `Node` having been put into
/// the slot its own configuration's handlers hold a clone of, so the two
/// kept each other alive, and the `Node` the endpoint that holds the
/// socket.
#[test]
fn a_daemon_restarted_in_one_process_serves_the_port_it_served_before() {
    let l = layout("port", common::unclaimed_port());
    let daemon = Daemon::start(&l);
    let addr = daemon.addr();
    let alice = kernel("alice");
    attach(&alice, &addr, &[]);
    let left = daemon.stop();
    let again = std::panic::catch_unwind(|| Daemon::start(&l));
    let daemon = again.unwrap_or_else(|_| {
        panic!("the restart binds the port it served on before; still referenced: {left:?}")
    });
    assert_eq!(daemon.addr(), addr);
    let _ = daemon.stop();
    let _ = std::fs::remove_dir_all(&l.dir);
}
