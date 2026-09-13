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
            "identity = {}\nlisten = 127.0.0.1:0\nqueue = {}\nprekeys = {}\ntopology = {}\narchive = {}\nheartbeat = 30\ningestion = unverified-gossip\nallowance = 120/60\n",
            dir.join("identity.key").display(),
            queue.display(),
            dir.join("prekeys").display(),
            dir.join("topology").display(),
            dir.join("archive").display()
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
    ["bob", "carol", "w1", "witness"].iter().map(|n| test_identity(n).public).collect()
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

/// A view for bob with the store loaded and nothing derived yet: what a
/// materialised copy has to agree with.
fn bare(cfg: &rhtn_daemon::config::Config) -> rhtn_node::view::NodeView {
    let me = std::sync::Arc::new(test_identity("bob"));
    let mut view = rhtn_node::view::NodeView::new(me.clone(), rhtn_archive::tx::Locator { anchor: me.public.keyhash, path: Vec::new(), nibbles: 0, seqno: rhtn_archive::tx::Seqno { series: 1, counter: 0 } });
    view.store = rhtn_node::store::TopologyStore::load(&cfg.topology).expect("the store reads");
    view
}

/// The routing slots as a view holds them: the number and who is in it.
type Slots = Vec<(u64, Option<[u8; 32]>)>;

/// The table and the slots a full replay of the whole store produces.
fn replayed(cfg: &rhtn_daemon::config::Config) -> (Vec<[u8; 32]>, Slots) {
    let mut view = bare(cfg);
    view.rebuild_from_store(&ids());
    let subs = view.table.subordinates(&kh("bob")).into_iter().collect();
    let slots = view.slots.iter().map(|(n, s)| (*n, s.occupant)).collect();
    (subs, slots)
}

// acceptance: DMN-11
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_wake_folds_in_what_arrived_since_rather_than_replaying_the_store() {
    use rhtn_archive::topology::{Restored, Snapshot};
    use rhtn_daemon::config::Config;
    use rhtn_daemon::service::Service;
    use rhtn_node::store::{Decision, KIND_TRANSACTION};
    let l = layout("materialise");
    std::fs::write(&l.peers, ["carol", "w1", "witness"].map(|n| format!("{}\n", hex(&test_identity(n).public.key_material()))).concat()).unwrap();
    let cfg = Config::read(&l.config).unwrap();
    let (pop, adoption) = adoption_under("bob", "carol");
    // one adoption accepted, then written out with the derived view beside
    // the store
    {
        let s = Service::start(&cfg, &l.peers).await.expect("starts");
        let mut view = s.node.view.lock().unwrap();
        view.store.keep_presence(rhtn_archive::record::Record::parse(&pop).unwrap().txid, pop.clone());
        let me = view.me();
        assert_eq!(view.take_object(&s.node.adjacency, &me, KIND_TRANSACTION, &adoption, &ids()), Decision::Stored);
        drop(view);
        s.persist().expect("persists");
    }
    let before = std::fs::read(cfg.topology.join("derived")).expect("the derived view is written");
    let snap = Snapshot::decode(&before).expect("and it reads");
    assert!(snap.high.is_some(), "the watermark names the transaction folded in");
    // a second adoption is added to the store without the derived view
    // being told: it sorts above the watermark, so a wake folds it alone
    let (pop2, adoption2) = adoption_under("bob", "w1");
    {
        let s = Service::start(&cfg, &l.peers).await.expect("starts again");
        let mut view = s.node.view.lock().unwrap();
        view.store.keep_presence(rhtn_archive::record::Record::parse(&pop2).unwrap().txid, pop2.clone());
        let me = view.me();
        assert_eq!(view.take_object(&s.node.adjacency, &me, KIND_TRANSACTION, &adoption2, &ids()), Decision::Stored);
        drop(view);
        s.persist().expect("persists");
    }
    // started once more: the second record is the only one folded, and the
    // view is what a full replay would have produced
    let s = Service::start(&cfg, &l.peers).await.expect("starts a third time");
    let view = s.node.view.lock().unwrap();
    let held: Vec<[u8; 32]> = view.table.subordinates(&kh("bob")).into_iter().collect();
    let slots: Slots = view.slots.iter().map(|(n, sl)| (*n, sl.occupant)).collect();
    drop(view);
    let (subs, replay_slots) = replayed(&cfg);
    assert_eq!(held, subs, "the same table a replay produces");
    assert_eq!(slots, replay_slots, "and the same slots");
    assert!(held.contains(&kh("carol")) && held.contains(&kh("w1")), "both bindings, one from the fold and one from the snapshot");
    // and the fold, run against the snapshot the second stop wrote, touches
    // nothing: the snapshot already accounts for the whole store
    {
        let snap = Snapshot::decode(&std::fs::read(cfg.topology.join("derived")).unwrap()).unwrap();
        assert_eq!(snap.folded, bare(&cfg).materialise().folded, "the digest names the whole store");
        let mut fresh = bare(&cfg);
        assert_eq!(fresh.restore_materialised(Some(&snap), &ids()), Restored::Current, "nothing replayed and nothing folded");
        assert_eq!(fresh.table.subordinates(&kh("bob")).into_iter().collect::<Vec<_>>(), subs);
    }
    let _ = std::fs::remove_dir_all(&l.dir);
}

// acceptance: DMN-12
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_derived_view_that_cannot_account_for_the_store_is_discarded_whole() {
    use rhtn_archive::topology::{Restored, Snapshot};
    use rhtn_daemon::config::Config;
    use rhtn_daemon::service::Service;
    use rhtn_node::store::{Decision, KIND_TRANSACTION};
    let l = layout("mismatch");
    std::fs::write(&l.peers, ["carol", "witness"].map(|n| format!("{}\n", hex(&test_identity(n).public.key_material()))).concat()).unwrap();
    let cfg = Config::read(&l.config).unwrap();
    let (pop, adoption) = adoption_under("bob", "carol");
    {
        let s = Service::start(&cfg, &l.peers).await.expect("starts");
        let mut view = s.node.view.lock().unwrap();
        view.store.keep_presence(rhtn_archive::record::Record::parse(&pop).unwrap().txid, pop.clone());
        let me = view.me();
        assert_eq!(view.take_object(&s.node.adjacency, &me, KIND_TRANSACTION, &adoption, &ids()), Decision::Stored);
        drop(view);
        s.persist().expect("persists");
    }
    let good = Snapshot::decode(&std::fs::read(cfg.topology.join("derived")).unwrap()).unwrap();
    let expected: Vec<[u8; 32]> = replayed(&cfg).0;
    let fresh = || bare(&cfg);
    // a view whose table bytes do not read
    let damaged = Snapshot { table: b"not a table".to_vec(), ..good.clone() };
    let mut v = fresh();
    assert_eq!(v.restore_materialised(Some(&damaged), &ids()), Restored::Replayed { replayed: 1 });
    assert_eq!(v.table.subordinates(&kh("bob")).into_iter().collect::<Vec<_>>(), expected, "the same answer a replay gives");
    // a view claiming more records than the store holds
    let other_input = Snapshot { folded: [9; 32], ..good.clone() };
    let mut v = fresh();
    assert_eq!(v.restore_materialised(Some(&other_input), &ids()), Restored::Replayed { replayed: 1 });
    assert_eq!(v.table.subordinates(&kh("bob")).into_iter().collect::<Vec<_>>(), expected);
    // a watermark above everything held is harmless, because the digest and
    // not the watermark is the test: every record falls below it, and the
    // digest still has to name exactly those records
    let ahead = Snapshot { high: Some((u64::MAX, [0xff; 32])), ..good.clone() };
    let mut v = fresh();
    assert_eq!(v.restore_materialised(Some(&ahead), &ids()), Restored::Current);
    assert_eq!(v.table.subordinates(&kh("bob")).into_iter().collect::<Vec<_>>(), expected);
    // and no view at all
    let mut v = fresh();
    assert_eq!(v.restore_materialised(None, &ids()), Restored::Replayed { replayed: 1 });
    assert_eq!(v.table.subordinates(&kh("bob")).into_iter().collect::<Vec<_>>(), expected);
    // the good one, for contrast: taken, and nothing replayed
    let mut v = fresh();
    assert_eq!(v.restore_materialised(Some(&good), &ids()), Restored::Current);
    assert_eq!(v.table.subordinates(&kh("bob")).into_iter().collect::<Vec<_>>(), expected);
    let _ = std::fs::remove_dir_all(&l.dir);
}

// acceptance: DMN-14
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn serving_a_one_time_key_does_not_stop_the_node_writing_its_state_back() {
    use rhtn_daemon::config::Config;
    use rhtn_daemon::service::Service;
    let l = layout("issuance-save");
    std::fs::write(&l.peers, ["carol", "witness"].map(|n| format!("{}\n", hex(&test_identity(n).public.key_material()))).concat()).unwrap();
    let cfg = Config::read(&l.config).unwrap();
    let s = Service::start(&cfg, &l.peers).await.expect("starts");
    {
        let mut view = s.node.view.lock().unwrap();
        let bundle = rhtn_archive::prekey::PrekeyBundle::build(&test_identity("carol"), rhtn_archive::prekey::CONSTRUCTION_PQXDH, b"reusable material", 1_800_000_000);
        view.prekeys.publish(&ids(), &bundle).expect("published");
        assert!(view.prekeys.stock(kh("carol"), vec![b"the only one-time key".to_vec()]), "stocked");
    }
    // a save before any issuance, so the failure below can only be the
    // issuance's
    s.persist().expect("persists before serving anything");

    let served = {
        let mut view = s.node.view.lock().unwrap();
        let now = view.now();
        let req = rhtn_archive::prekey::PrekeyRequest::One { subject: kh("carol"), one_time: true, nonce: [1; 16] }.encode();
        let reply = view.prekeys.answer(&kh("witness"), &req, now).expect("answered");
        rhtn_archive::prekey::PrekeyReply::decode(&reply).unwrap().one_time.is_some()
    };
    assert!(served, "a one-time key was served, which is the condition under test");

    // a bookkeeping file beside the pools, which is what the issuance
    // counter used to be: **a stray file is not a stale subject
    // directory**, and treating it as one turned every later save into
    // `NotADirectory` before the archive, the store or the derived view
    // reached disk
    std::fs::write(cfg.prekeys.join("prekeys").join("issued"), b"a file, not a subject").unwrap();

    // **ordinary issuance is not a condition that can stop a node
    // persisting** (`infra-client-requirements.md` §4.3): the archive, the
    // store and the derived view all have to reach disk afterwards
    s.persist().expect("persists after serving a one-time key");
    s.persist().expect("and again");
    assert!(cfg.topology.join("derived").exists(), "the derived view is written");
    assert!(cfg.archive.exists(), "and the archive");
    assert!(cfg.topology.join("tx").exists(), "and the topology store");
    let _ = std::fs::remove_dir_all(&l.dir);
}

// acceptance: DMN-16
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_derived_view_belonging_to_another_identity_is_discarded() {
    use rhtn_archive::topology::{Restored, Snapshot};
    use rhtn_daemon::config::Config;
    use rhtn_daemon::service::Service;
    use rhtn_node::store::{Decision, KIND_TRANSACTION};
    let l = layout("foreign-view");
    std::fs::write(&l.peers, ["carol", "witness"].map(|n| format!("{}\n", hex(&test_identity(n).public.key_material()))).concat()).unwrap();
    let cfg = Config::read(&l.config).unwrap();
    let (pop, adoption) = adoption_under("bob", "carol");
    {
        let s = Service::start(&cfg, &l.peers).await.expect("starts");
        let mut view = s.node.view.lock().unwrap();
        view.store.keep_presence(rhtn_archive::record::Record::parse(&pop).unwrap().txid, pop.clone());
        let me = view.me();
        assert_eq!(view.take_object(&s.node.adjacency, &me, KIND_TRANSACTION, &adoption, &ids()), Decision::Stored);
        drop(view);
        s.persist().expect("persists");
    }
    let expected = replayed(&cfg);
    assert_eq!(expected.0, vec![kh("carol")], "control: bob holds carol as a subordinate in slot 0");

    // carol's own view of the same adoption: the same records, a different
    // owner, and different subordinate slots — carol has none
    let carol = std::sync::Arc::new(test_identity("carol"));
    let mut theirs = rhtn_node::view::NodeView::new(carol.clone(), rhtn_archive::tx::Locator { anchor: carol.public.keyhash, path: Vec::new(), nibbles: 0, seqno: rhtn_archive::tx::Seqno { series: 1, counter: 0 } });
    theirs.store = rhtn_node::store::TopologyStore::load(&cfg.topology).unwrap();
    theirs.rebuild_from_store(&ids());
    let foreign: Snapshot = theirs.materialise();

    // **whose view this is, is this node's own answer.**  Installing
    // carol's would replace bob's slots with carol's, which are empty
    let mut v = bare(&cfg);
    assert_eq!(v.restore_materialised(Some(&foreign), &ids()), Restored::Replayed { replayed: 1 }, "another identity's derived view is discarded");
    assert_eq!(v.table.subordinates(&kh("bob")).into_iter().collect::<Vec<_>>(), expected.0, "and the node rebuilds its own");
    assert_eq!(v.slots.iter().map(|(n, s)| (*n, s.occupant)).collect::<Slots>(), expected.1, "with its own routing slots");
    let _ = std::fs::remove_dir_all(&l.dir);
}
