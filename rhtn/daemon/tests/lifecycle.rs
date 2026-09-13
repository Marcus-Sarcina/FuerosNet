//! `rhtnd` as a process (`infra-client-requirements.md` §1, §2): started
//! from a configuration file, serving a real session, and restarted
//! without losing what it had accepted or delivering it twice.

use rhtn_crypto::identity::testkit::test_identity;
use rhtn_node::queue::DirStore;
use rhtn_transport::queue::{QueueStore, Queued};
use rhtn_transport::session::*;
use rhtn_transport::tls::{self, Pins};
use std::collections::BTreeMap;
use std::io::{BufRead, BufReader};
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};
use std::time::Duration;

fn kh(n: &str) -> [u8; 32] {
    test_identity(n).public.keyhash
}

/// The seeds `testkit` derives, written where the daemon reads them: the
/// daemon loads the same identity a test pins, without the testkit ever
/// reaching production code.
fn write_identity(path: &Path, name: &str) {
    let mut bytes = rhtn_codec::cose::sha256(format!("rhtn-test-vectors:{name}:ed25519-seed").as_bytes()).to_vec();
    bytes.extend_from_slice(&rhtn_codec::cose::sha256(format!("rhtn-test-vectors:{name}:ml-dsa-65-seed").as_bytes()));
    std::fs::write(path, &bytes).unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600)).unwrap();
    }
}

struct Layout {
    dir: PathBuf,
    config: PathBuf,
    peers: PathBuf,
    queue: PathBuf,
}

fn layout(tag: &str) -> Layout {
    let dir = std::env::temp_dir().join(format!("rhtnd-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let (config, peers, queue) = (dir.join("rhtnd.conf"), dir.join("peers"), dir.join("queue"));
    write_identity(&dir.join("identity.key"), "bob");
    std::fs::write(&peers, format!("{}\n", hex(&test_identity("alice").public.key_material()))).unwrap();
    std::fs::write(
        &config,
        format!(
            "identity = {}\nlisten = 127.0.0.1:0\nqueue = {}\nprekeys = {}\ntopology = {}\nheartbeat = 30\ningestion = unverified-gossip\nallowance = 120/60\n",
            dir.join("identity.key").display(),
            queue.display(),
            dir.join("prekeys").display(),
            dir.join("topology").display()
        ),
    )
    .unwrap();
    Layout { dir, config, peers, queue }
}

fn hex(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect()
}

/// Start the daemon and wait for the address it bound.
fn start(l: &Layout) -> (Child, SocketAddr) {
    let mut child = Command::new(env!("CARGO_BIN_EXE_rhtnd"))
        .arg(&l.config)
        .arg(&l.peers)
        .stdout(Stdio::piped())
        .spawn()
        .expect("rhtnd starts");
    let mut out = BufReader::new(child.stdout.take().unwrap());
    let mut line = String::new();
    out.read_line(&mut line).expect("the daemon says where it is serving");
    let addr = line.trim().rsplit_once(' ').expect("an address").1.parse().expect("an address and port");
    (child, addr)
}

/// SIGTERM, then wait: the daemon writes its state back on the way out.
fn stop(mut child: Child) {
    #[cfg(unix)]
    unsafe {
        libc_kill(child.id() as i32);
    }
    let end = std::time::Instant::now() + Duration::from_secs(10);
    loop {
        if let Some(s) = child.try_wait().unwrap() {
            assert!(s.success(), "the daemon exits cleanly on SIGTERM: {s:?}");
            return;
        }
        if std::time::Instant::now() > end {
            let _ = child.kill();
            panic!("the daemon did not stop on SIGTERM");
        }
        std::thread::sleep(Duration::from_millis(50));
    }
}

#[cfg(unix)]
unsafe fn libc_kill(pid: i32) {
    unsafe extern "C" {
        fn kill(pid: i32, sig: i32) -> i32;
    }
    unsafe { kill(pid, 15) };
}

fn client_cfg(name: &str, target: [u8; 32], addr: SocketAddr) -> ClientConfig {
    let pins = Pins::new();
    pins.pin_identity(&test_identity("alice").public);
    pins.pin_identity(&test_identity("bob").public);
    let book = std::collections::HashMap::from([(target, vec![addr])]);
    ClientConfig {
        identity: Arc::new(test_identity(name)),
        pins,
        capabilities: BTreeMap::new(),
        attestation: None,
        filter: None,
        sibling_cache: Arc::new(Mutex::new(Vec::new())),
        addresses: Arc::new(Mutex::new(book)),
        tls: Arc::new(Mutex::new(Default::default())),
        connect_timeout: Duration::from_millis(2000),
        on_reachability: None,
        log: Log::recording(),
    }
}

/// Attach as alice and take everything delivered within `ms`.
async fn collect(addr: SocketAddr, ms: u64) -> Vec<Vec<u8>> {
    let cfg = client_cfg("alice", kh("bob"), addr);
    let ep = tls::client_endpoint("127.0.0.1:0".parse().unwrap()).unwrap();
    let AttachOutcome::Attached(mut s) = attach(&cfg, &ep, kh("bob"), addr, false).await else { panic!("alice attaches to the daemon") };
    let mut got = Vec::new();
    let end = tokio::time::Instant::now() + Duration::from_millis(ms);
    while let Ok(Some(b)) = tokio::time::timeout_at(end, s.deliveries.recv()).await {
        got.push(b);
    }
    got
}

// acceptance: DMN-02
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn the_daemon_serves_from_a_configuration_and_redelivers_across_a_restart_exactly_once() {
    let l = layout("restart");
    // it starts from the file alone, and serves a real session
    let (child, addr) = start(&l);
    assert!(collect(addr, 400).await.is_empty(), "nothing is waiting yet");
    stop(child);
    // something is accepted for alice while the daemon is down: the queue's
    // directory store is what a restart finds
    DirStore::new(&l.queue).push(Queued { ciphertext: b"kept across the restart".to_vec(), recipient: kh("alice"), arrival: 1 });
    let (child, addr) = start(&l);
    let got = collect(addr, 1500).await;
    assert_eq!(got, vec![b"kept across the restart".to_vec()], "redelivered on the next attach");
    stop(child);
    // delete on delivery survived the restart too: a second attach finds
    // nothing, and the store holds nothing (`infra-client-requirements.md` §2)
    assert!(DirStore::new(&l.queue).list(&kh("alice")).is_empty(), "nothing recoverable is left");
    let (child, addr) = start(&l);
    assert!(collect(addr, 600).await.is_empty(), "and it is not delivered twice");
    stop(child);
    let _ = std::fs::remove_dir_all(&l.dir);
}

// acceptance: DMN-01
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn the_daemon_refuses_to_start_on_an_identity_it_would_have_to_mint_or_share() {
    let l = layout("identity");
    let key = l.dir.join("identity.key");
    // absent
    std::fs::remove_file(&key).unwrap();
    let out = Command::new(env!("CARGO_BIN_EXE_rhtnd")).arg(&l.config).arg(&l.peers).output().unwrap();
    assert!(!out.status.success());
    assert!(String::from_utf8_lossy(&out.stderr).contains("identity"), "it says what it could not read");
    // present and readable by the rest of the host
    write_identity(&key, "bob");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&key, std::fs::Permissions::from_mode(0o644)).unwrap();
        let out = Command::new(env!("CARGO_BIN_EXE_rhtnd")).arg(&l.config).arg(&l.peers).output().unwrap();
        assert!(!out.status.success());
        assert!(String::from_utf8_lossy(&out.stderr).contains("beyond its owner"));
    }
    let _ = std::fs::remove_dir_all(&l.dir);
}

/// A presence record between two parties and the adoption it supports,
/// signed as those parties would sign them.  The daemon is the patron, so
/// it countersigns the adoption and must verify its own signature.
fn adoption_under(patron: &str, child: &str) -> (Vec<u8>, Vec<u8>) {
    use rhtn_archive::record::Record;
    use rhtn_archive::tx::*;
    let (p, c, w) = (test_identity(patron), test_identity(child), test_identity("witness"));
    let g = |n: &str| vec![rhtn_archive::genesis(&test_identity(n).public.keyhash)];
    let root = rhtn_codec::cose::sha256(b"a meeting");
    let pop = presence_record_body(&[g(patron), g(child), g("witness")], [&p.public.keyhash, &c.public.keyhash], &[Witness { keyhash: w.public.keyhash, nominated_by: p.public.keyhash, flags: 3 }], 1_800_000_000, 1_800_000_600, &root);
    let pop = envelope(TYPE_PRESENCE, &pop, &[&p, &c, &w]);
    let pop_id = Record::parse(&pop).unwrap().txid;
    let (bp, bc) = (vec![pop_id], vec![pop_id]);
    let path = rhtn_node::resolution::Path::from_indices(&[0]);
    let a = Adoption {
        node: c.public.keyhash,
        patron: p.public.keyhash,
        locator: Locator { anchor: p.public.keyhash, path: path.bytes, nibbles: path.nibbles, seqno: Seqno { series: 1, counter: 0 } },
        timestamp: 1_800_003_600,
        key_material: None,
        evidence: Evidence::Presence(pop_id),
        presented_head: None,
        back: [&bc, &bp],
    };
    (pop, envelope(TYPE_ADOPTION, &adoption_body(&a), &[&c, &p]))
}

// acceptance: DMN-03
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_restart_rebuilds_the_routing_view_and_verifies_this_nodes_own_signature() {
    use rhtn_daemon::config::Config;
    use rhtn_daemon::service::Service;
    use rhtn_node::store::{Decision, KIND_TRANSACTION};
    let l = layout("rebuild");
    // the peers file names the other signers and never this node: an
    // operator should not have to list themselves
    std::fs::write(
        &l.peers,
        ["carol", "witness"].map(|n| format!("{}\n", hex(&test_identity(n).public.key_material()))).concat(),
    )
    .unwrap();
    let cfg = Config::read(&l.config).unwrap();
    let (pop, adoption) = adoption_under("bob", "carol");
    let slot = {
        let s = Service::start(&cfg, &l.peers).await.expect("starts");
        let mut view = s.node.view.lock().unwrap();
        let pop_id = rhtn_archive::record::Record::parse(&pop).unwrap().txid;
        view.store.keep_presence(pop_id, pop.clone());
        let me = view.me();
        let d = view.take_object(&s.node.adjacency, &me, KIND_TRANSACTION, &adoption, &ids());
        assert_eq!(d, Decision::Stored, "the adoption verifies without the peers file naming this node");
        let slot = view.slot_of(&kh("carol")).expect("the child occupies a slot");
        drop(view);
        s.persist().expect("persists");
        slot
    };
    // started again from those bytes alone
    let s = Service::start(&cfg, &l.peers).await.expect("starts again");
    let view = s.node.view.lock().unwrap();
    assert!(view.table.subordinates(&kh("bob")).contains(&kh("carol")), "the binding is back");
    assert_eq!(view.slot_of(&kh("carol")), Some(slot), "and the slot it was in");
    assert_eq!(view.child_at(slot as u8), Some(kh("carol")));
    drop(view);
    let _ = std::fs::remove_dir_all(&l.dir);
}

fn ids() -> Vec<rhtn_crypto::Identity> {
    ["bob", "carol", "witness"].iter().map(|n| test_identity(n).public).collect()
}

/// A disavowal by `patron` of `node`, on `back`, as that patron signs it.
fn disavowal(patron: &str, node: &str, back: &[[u8; 32]], t: u64) -> Vec<u8> {
    use rhtn_archive::tx::*;
    let body = disavowal_body(back, &kh(patron), &kh(node), t, None);
    envelope(TYPE_DISAVOWAL, &body, &[&test_identity(patron)])
}

// acceptance: DMN-08
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_restart_puts_back_this_nodes_own_chain_and_its_current_series() {
    use rhtn_daemon::config::Config;
    use rhtn_daemon::service::Service;
    use rhtn_node::store::{Decision, KIND_TRANSACTION};
    let l = layout("continuity");
    std::fs::write(&l.peers, ["carol", "witness"].map(|n| format!("{}\n", hex(&test_identity(n).public.key_material()))).concat()).unwrap();
    let cfg = Config::read(&l.config).unwrap();
    let (pop, adoption) = adoption_under("bob", "carol");
    let signed = {
        let s = Service::start(&cfg, &l.peers).await.expect("starts");
        let mut view = s.node.view.lock().unwrap();
        let pop_id = rhtn_archive::record::Record::parse(&pop).unwrap().txid;
        view.store.keep_presence(pop_id, pop.clone());
        let me = view.me();
        assert_eq!(view.take_object(&s.node.adjacency, &me, KIND_TRANSACTION, &adoption, &ids()), Decision::Stored);
        // bob disavows carol, on the back-pointers its own archive gives
        let back = view.archive.next_back_pointers();
        let d = disavowal("bob", "carol", &back, 1_800_007_200);
        assert_eq!(view.take_object(&s.node.adjacency, &me, KIND_TRANSACTION, &d, &ids()), Decision::Stored);
        view.archive.append(rhtn_archive::record::Record::parse(&d).unwrap()).expect("its own record");
        let txid = rhtn_archive::record::Record::parse(&d).unwrap().txid;
        assert_eq!(view.archive.next_back_pointers(), vec![txid], "the chain runs through it");
        drop(view);
        s.persist().expect("persists");
        txid
    };
    // started again from those bytes alone
    let s = Service::start(&cfg, &l.peers).await.expect("starts again");
    let view = s.node.view.lock().unwrap();
    assert_eq!(view.archive.next_back_pointers(), vec![signed], "the next record extends the published chain, not genesis");
    assert_ne!(view.archive.next_back_pointers(), vec![rhtn_archive::genesis(&kh("bob"))]);
    drop(view);
    let _ = std::fs::remove_dir_all(&l.dir);
}
