//! `rhtnd` as a process (`infra-client-requirements.md` §1, §2): started
//! from a configuration file, serving a real session, and restarted
//! without losing what it had accepted or delivering it twice.

use rhtn_crypto::identity::testkit::test_identity;
use rhtn_node::queue::DirStore;
use rhtn_transport::queue::QueueStore;
use rhtn_transport::session::*;
use rhtn_transport::tls::{self, Party, Pins};
use std::collections::BTreeMap;
use std::io::{BufRead, BufReader};
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// The subnet the fixtures' positions are in (`wire-format.md` §2.3 field 4).
const SUBNET: [u8; 32] = [0xdb; 32];

fn kh(n: &str) -> [u8; 32] {
    test_identity(n).public.keyhash
}

/// The seeds `testkit` derives, written where the daemon reads them: the
/// daemon loads the same identity a test pins, without the testkit ever
/// reaching production code.
fn write_identity(path: &Path, name: &str) {
    let mut bytes =
        rhtn_codec::cose::sha256(format!("rhtn-test-vectors:{name}:ed25519-seed").as_bytes())
            .to_vec();
    bytes.extend_from_slice(&rhtn_codec::cose::sha256(
        format!("rhtn-test-vectors:{name}:ml-dsa-65-seed").as_bytes(),
    ));
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
    std::fs::write(
        &peers,
        format!("{}\n", hex(&test_identity("alice").public.key_material())),
    )
    .unwrap();
    std::fs::write(
        &config,
        format!(
            "identity = \"{}\"\nlisten = \"127.0.0.1:0\"\nqueue = \"{}\"\nkeys = \"{}\"\ntopology = \"{}\"\narchive = \"{}\"\nheartbeat = 30\ningestion = \"unverified-gossip\"\n\n[allowance]\nrequests = 120\nseconds = 60\n",
            dir.join("identity.key").display(),
            queue.display(),
            dir.join("keys").display(),
            dir.join("topology").display(),
            dir.join("archive").display()
        ),
    )
    .unwrap();
    Layout {
        dir,
        config,
        peers,
        queue,
    }
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
    out.read_line(&mut line)
        .expect("the daemon says where it is serving");
    let addr = line
        .trim()
        .rsplit_once(' ')
        .expect("an address")
        .1
        .parse()
        .expect("an address and port");
    // the pipe stays open and drained: closed, the daemon's next print
    // would be a broken pipe and its end
    std::thread::spawn(move || for _ in out.lines().map_while(Result::ok) {});
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
        me: Party::of(Arc::new(test_identity(name))),
        pins,
        bind: Default::default(),
        capabilities: BTreeMap::new(),
        attestation: None,
        filter: None,
        sibling_cache: Arc::new(Mutex::new(Vec::new())),
        addresses: Arc::new(Mutex::new(book)),
        cursors: Default::default(),
        tls: Arc::new(Mutex::new(Default::default())),
        connect_timeout: Duration::from_millis(2000),
        on_reachability: None,
        log: Log::recording(),
    }
}

/// Attach as alice, naming `cursor` where the device holds one, and take
/// everything delivered within `ms`.
async fn collect(addr: SocketAddr, ms: u64, cursor: Option<u64>) -> Vec<Vec<u8>> {
    let cfg = client_cfg("alice", kh("bob"), addr);
    if let Some(c) = cursor {
        cfg.cursors.lock().unwrap().insert(kh("bob"), c);
    }
    let ep = tls::client_endpoint("127.0.0.1:0".parse().unwrap()).unwrap();
    let AttachOutcome::Attached(mut s) = attach(&cfg, &ep, kh("bob"), addr, false).await else {
        panic!("alice attaches to the daemon")
    };
    let mut got = Vec::new();
    let end = tokio::time::Instant::now() + Duration::from_millis(ms);
    while let Ok(Some(b)) = tokio::time::timeout_at(end, s.deliveries.recv()).await {
        got.push(b);
    }
    // the device goes away: a session left open would take what arrives next
    s.conn.close(0u32.into(), b"");
    s.task.abort();
    tokio::time::sleep(Duration::from_millis(200)).await;
    got
}

/// An identity's encryption secrets for a test: the node opens nothing,
/// so the recipient's own are whatever the test says they are.
fn secret(name: &str) -> rhtn_crypto::seal::EncryptionSecret {
    rhtn_crypto::seal::EncryptionSecret::from_identity_root(&rhtn_codec::cose::sha256(
        format!("lifecycle root:{name}").as_bytes(),
    ))
}

/// **One request of the daemon on a connection under a key minted for
/// it** (`wire-format.md` §9.1). The key is `mint`'s, a test identity the
/// daemon has never been told of, so a test can look for it afterwards
/// (`test_identity(mint).public.ed`); a real sender draws a fresh one.
async fn ask_minted(addr: SocketAddr, mint: &str, request: u64, body: &[u8]) -> Vec<u8> {
    let me = tls::Presenter::Own {
        keyhash: MINTED,
        ed: Box::new(test_identity(mint).ed_signing_key().clone()),
    };
    let pins = Pins::new();
    pins.pin_identity(&test_identity("bob").public);
    let cfg = tls::client_config(me, &pins, &kh("bob")).expect("bob is pinned");
    let ep = tls::client_endpoint("127.0.0.1:0".parse().unwrap()).unwrap();
    let conn = tls::dial_with(&ep, cfg, addr)
        .expect("dials")
        .await
        .expect("connects");
    let reply = request_on(&conn, request, body).await.expect("answered");
    conn.close(0u32.into(), b"");
    reply
}

/// `to`'s newest node key at the daemon, asked under `mint`'s key.
async fn node_key(addr: SocketAddr, mint: &str, to: &str, nonce: [u8; 16]) -> Vec<u8> {
    use rhtn_archive::node_key::*;
    let ask = NodeKeyRequest::for_recipients(vec![kh(to)], nonce).unwrap();
    let reply = ask_minted(addr, mint, REQUEST_NODE_KEY, &ask.encode()).await;
    NodeKeyReply::decode(&reply)
        .unwrap()
        .outcome
        .expect("a key for a recipient the daemon serves")
}

/// **Send `bytes` to `to` at the daemon as a client does**: the node key
/// fetched and the seal submitted, each on a connection minted for it. The
/// reply's code.
async fn send(addr: SocketAddr, from: &str, to: &str, bytes: &[u8]) -> u64 {
    use rhtn_archive::submission::*;
    use rhtn_client::payload::{KIND_APPLICATION, NodeKeyHeld, Sender, seal_to};
    let record = node_key(addr, "w1", to, [1; 16]).await;
    let key = rhtn_archive::node_key::NodeKey::parse(&record).unwrap();
    let theirs = secret(to);
    let their = rhtn_crypto::verify::EncryptionKey {
        keyhash: kh(to),
        kem: *theirs.kem_public(),
        auth: theirs.auth_public(),
    };
    let mine = secret(from);
    let sealed = seal_to(
        &Sender {
            keyhash: &kh(from),
            secret: &mine,
            alias: from,
            card: None,
        },
        &kh(to),
        &their,
        &NodeKeyHeld {
            node: key.fields.node,
            n: key.fields.n,
            public: Box::new(key.fields.public),
        },
        KIND_APPLICATION,
        bytes,
        &mut |b: &mut [u8]| {
            let r = tls::random_bytes::<64>();
            b.copy_from_slice(&r[..b.len()]);
        },
    )
    .unwrap();
    let body = RelaySubmission {
        recipient: kh(to),
        sealed,
        nonce: [2; 16],
    }
    .encode();
    let reply = ask_minted(addr, "w2", REQUEST_RELAY, &body).await;
    SubmissionReply::decode(&reply).unwrap().code
}

/// What one delivery opens to, for `to`, from the daemon.
fn opened(to: &str, delivery: &[u8]) -> (u64, Vec<u8>) {
    let r = rhtn_archive::submission::unrelayed(delivery).expect("a relayed payload");
    let (_, body) = rhtn_client::payload::open_relayed(&secret(to), &kh(to), &kh("bob"), &r)
        .expect("opens with the daemon's half");
    (r.position, body.bytes)
}

// acceptance: DMN-02
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn the_daemon_serves_from_a_configuration_and_keeps_a_delivered_message_across_a_restart() {
    let l = layout("restart");
    // it starts from the file alone, and serves a real session
    let (child, addr) = start(&l);
    assert!(
        collect(addr, 400, None).await.is_empty(),
        "nothing is waiting yet"
    );
    // a message is accepted for alice, who is away, and the daemon is
    // stopped with a signal with it waiting in the queue
    assert_eq!(
        send(addr, "carol", "alice", b"kept across the restart").await,
        rhtn_archive::submission::SUBMISSION_ACCEPTED
    );
    stop(child);
    let held = DirStore::new(&l.queue).list(&kh("alice"));
    assert_eq!(held.len(), 1, "waiting on disk");
    assert_eq!(held[0].delivered_at, None);
    // restarted, a device of alice's attaches: delivered on the first attach
    let (child, addr) = start(&l);
    let got = collect(addr, 1500, None).await;
    assert_eq!(got.len(), 1);
    let (position, bytes) = opened("alice", &got[0]);
    assert_eq!(
        bytes, b"kept across the restart",
        "redelivered on the next attach"
    );
    stop(child);
    let first = DirStore::new(&l.queue).list(&kh("alice"));
    assert_eq!(first.len(), 1, "still held once delivered");
    let delivered_at = first[0]
        .delivered_at
        .expect("its first delivery is recorded");
    // stopped and restarted, the device attaches naming the position it was
    // handed: nothing
    let (child, addr) = start(&l);
    assert!(
        collect(addr, 600, Some(position)).await.is_empty(),
        "the second attach is handed nothing"
    );
    stop(child);
    let after = DirStore::new(&l.queue).list(&kh("alice"));
    assert_eq!(after.len(), 1, "the message is still held");
    assert_eq!(
        after[0].delivered_at,
        Some(delivered_at),
        "its thirty days count from the first delivery, not restarted by the stop"
    );
    let _ = std::fs::remove_dir_all(&l.dir);
}

// acceptance: DMN-01
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn the_daemon_refuses_to_start_on_an_identity_it_would_have_to_mint_or_share() {
    let l = layout("identity");
    let key = l.dir.join("identity.key");
    // absent
    std::fs::remove_file(&key).unwrap();
    let out = Command::new(env!("CARGO_BIN_EXE_rhtnd"))
        .arg(&l.config)
        .arg(&l.peers)
        .output()
        .unwrap();
    assert!(!out.status.success());
    assert!(
        String::from_utf8_lossy(&out.stderr).contains("identity"),
        "it says what it could not read"
    );
    // present and readable by the rest of the host
    write_identity(&key, "bob");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&key, std::fs::Permissions::from_mode(0o644)).unwrap();
        let out = Command::new(env!("CARGO_BIN_EXE_rhtnd"))
            .arg(&l.config)
            .arg(&l.peers)
            .output()
            .unwrap();
        assert!(!out.status.success());
        assert!(String::from_utf8_lossy(&out.stderr).contains("beyond its owner"));
    }
    let _ = std::fs::remove_dir_all(&l.dir);
}

/// A presence record between two parties and the adoption it supports,
/// signed as those parties would sign them.  The daemon is the patron, so
/// it countersigns the adoption and must verify its own signature.
/// An adoption of `child` under `patron` in that patron's slot `slot`.
///
/// **The slot is the caller's to keep distinct** (design §3.1): a patron's
/// ten slots hold one occupant each, so a harness reusing one would be
/// offering a table a record it cannot store.
fn adoption_under_slot(patron: &str, child: &str, slot: u8) -> (Vec<u8>, Vec<u8>) {
    adoption_under_slot_carrying(patron, child, slot, false)
}

/// The same, carrying the child's key material where asked, which is what
/// introduces the child to a node that has never held it.
fn adoption_under_slot_carrying(
    patron: &str,
    child: &str,
    slot: u8,
    material: bool,
) -> (Vec<u8>, Vec<u8>) {
    use rhtn_archive::record::Record;
    use rhtn_archive::tx::*;
    let (p, c, w) = (
        test_identity(patron),
        test_identity(child),
        test_identity("witness"),
    );
    let g = |n: &str| vec![rhtn_archive::genesis(&test_identity(n).public.keyhash)];
    let root = rhtn_codec::cose::sha256(b"a meeting");
    let pop = presence_record_body(
        &[g(patron), g(child), g("witness")],
        [&p.public.keyhash, &c.public.keyhash],
        &[Witness {
            keyhash: w.public.keyhash,
            nominated_by: p.public.keyhash,
            flags: 3,
        }],
        1_800_000_000,
        1_800_000_600,
        &root,
    );
    let pop = envelope(TYPE_PRESENCE, &pop, &[&p, &c, &w]);
    let pop_id = Record::parse(&pop).unwrap().txid;
    let (bp, bc) = (vec![pop_id], vec![pop_id]);
    let path = rhtn_node::resolution::Path::from_indices(&[slot]);
    let a = Adoption {
        node: c.public.keyhash,
        patron: p.public.keyhash,
        locator: Locator {
            anchor: p.public.keyhash,
            path: path.bytes,
            nibbles: path.nibbles,
            seqno: Seqno {
                series: 1,
                counter: 0,
            },
            subnet: Some(SUBNET),
        },
        timestamp: 1_800_003_600,
        key_material: material.then(|| c.public.key_material()),
        evidence: Evidence::Presence(pop_id),
        presented_head: None,
        back: [&bc, &bp],
        carrying: None,
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
        ["carol", "witness"]
            .map(|n| format!("{}\n", hex(&test_identity(n).public.key_material())))
            .concat(),
    )
    .unwrap();
    let cfg = Config::read(&l.config).unwrap();
    let (pop, adoption) = adoption_under_slot("bob", "carol", 0);
    let slot = {
        let s = Service::start(&cfg, &l.peers).await.expect("starts");
        let mut view = s.node.view.lock().unwrap();
        let pop_id = rhtn_archive::record::Record::parse(&pop).unwrap().txid;
        view.store.keep_presence(pop_id, pop.clone());
        let me = view.me();
        let d = view.take_object(&s.node.adjacency, &me, KIND_TRANSACTION, &adoption, &ids());
        assert_eq!(
            d,
            Decision::Stored,
            "the adoption verifies without the peers file naming this node"
        );
        let slot = view
            .slot_of(&kh("carol"))
            .expect("the child occupies a slot");
        drop(view);
        s.persist().expect("persists");
        slot
    };
    // started again from those bytes alone
    let s = Service::start(&cfg, &l.peers).await.expect("starts again");
    let view = s.node.view.lock().unwrap();
    assert!(
        view.table.subordinates(&kh("bob")).contains(&kh("carol")),
        "the binding is back"
    );
    assert_eq!(
        view.slot_of(&kh("carol")),
        Some(slot),
        "and the slot it was in"
    );
    assert_eq!(view.child_at(slot as u8), Some(kh("carol")));
    drop(view);
    let _ = std::fs::remove_dir_all(&l.dir);
}

/// design §15.2.2: the downline table is kept across a restart, the part
/// of it memos gave being in no store to rebuild from.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_restart_finds_the_downline_table_as_it_was_left() {
    use rhtn_daemon::config::Config;
    use rhtn_daemon::service::Service;
    use rhtn_node::store::{Decision, KIND_TRANSACTION};
    let l = layout("downline");
    std::fs::write(
        &l.peers,
        ["carol", "witness"]
            .map(|n| format!("{}\n", hex(&test_identity(n).public.key_material())))
            .concat(),
    )
    .unwrap();
    let cfg = Config::read(&l.config).unwrap();
    let (pop, adoption) = adoption_under_slot("bob", "carol", 0);
    let before = {
        let s = Service::start(&cfg, &l.peers).await.expect("starts");
        let mut view = s.node.view.lock().unwrap();
        let pop_id = rhtn_archive::record::Record::parse(&pop).unwrap().txid;
        view.store.keep_presence(pop_id, pop.clone());
        let me = view.me();
        let d = view.take_object(&s.node.adjacency, &me, KIND_TRANSACTION, &adoption, &ids());
        assert_eq!(d, Decision::Stored);
        // the subordinate in slot 0 from the signed adoption, and beneath
        // it what only memos told this node of, four levels down
        assert!(view.downline.filled(&[0]));
        assert!(view.downline.set(&[0, 4, 2], 7, true, 1_800_000_000));
        assert_eq!(view.downline.depth(&[]), 4);
        let before = view.downline.clone();
        drop(view);
        s.persist().expect("persists");
        before
    };
    let kept = std::fs::read_to_string(cfg.topology.join("downline")).expect("written beside");
    assert!(kept.contains("0427 f 1800000000"), "{kept}");
    // started again from those bytes alone
    let s = Service::start(&cfg, &l.peers).await.expect("starts again");
    assert_eq!(s.node.view.lock().unwrap().downline, before);
    drop(s);
    // a file that does not read is refused whole: the table comes back
    // from the store alone, reading shallow, and the node starts
    std::fs::write(cfg.topology.join("downline"), "0 f 1\nnot a row\n").unwrap();
    let s = Service::start(&cfg, &l.peers)
        .await
        .expect("starts on a bad file");
    let view = s.node.view.lock().unwrap();
    assert!(view.downline.filled(&[0]), "the signed part is rebuilt");
    assert_eq!(view.downline.depth(&[]), 1, "the memos' part is not");
    drop(view);
    let _ = std::fs::remove_dir_all(&l.dir);
}

fn ids() -> Vec<rhtn_crypto::Identity> {
    ["bob", "carol", "w1", "witness"]
        .iter()
        .map(|n| test_identity(n).public)
        .collect()
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
    std::fs::write(
        &l.peers,
        ["carol", "witness"]
            .map(|n| format!("{}\n", hex(&test_identity(n).public.key_material())))
            .concat(),
    )
    .unwrap();
    let cfg = Config::read(&l.config).unwrap();
    let (pop, adoption) = adoption_under_slot("bob", "carol", 0);
    let signed = {
        let s = Service::start(&cfg, &l.peers).await.expect("starts");
        let mut view = s.node.view.lock().unwrap();
        let pop_id = rhtn_archive::record::Record::parse(&pop).unwrap().txid;
        view.store.keep_presence(pop_id, pop.clone());
        let me = view.me();
        assert_eq!(
            view.take_object(&s.node.adjacency, &me, KIND_TRANSACTION, &adoption, &ids()),
            Decision::Stored
        );
        // bob disavows carol, on the back-pointers its own archive gives
        let back = view.archive.next_back_pointers();
        let d = disavowal("bob", "carol", &back, 1_800_007_200);
        assert_eq!(
            view.take_object(&s.node.adjacency, &me, KIND_TRANSACTION, &d, &ids()),
            Decision::Stored
        );
        view.archive
            .append(rhtn_archive::record::Record::parse(&d).unwrap())
            .expect("its own record");
        let txid = rhtn_archive::record::Record::parse(&d).unwrap().txid;
        assert_eq!(
            view.archive.next_back_pointers(),
            vec![txid],
            "the chain runs through it"
        );
        drop(view);
        s.persist().expect("persists");
        txid
    };
    // started again from those bytes alone
    let s = Service::start(&cfg, &l.peers).await.expect("starts again");
    let view = s.node.view.lock().unwrap();
    assert_eq!(
        view.archive.next_back_pointers(),
        vec![signed],
        "the next record extends the published chain, not genesis"
    );
    assert_ne!(
        view.archive.next_back_pointers(),
        vec![rhtn_archive::genesis(&kh("bob"))]
    );
    drop(view);
    let _ = std::fs::remove_dir_all(&l.dir);
}

/// A view for bob with the store loaded and nothing derived yet: what a
/// materialised copy has to agree with.
fn bare(cfg: &rhtn_daemon::config::Config) -> rhtn_node::view::NodeView {
    let me = std::sync::Arc::new(test_identity("bob"));
    let mut view = rhtn_node::view::NodeView::new(
        me.clone(),
        rhtn_archive::tx::Locator {
            anchor: me.public.keyhash,
            path: Vec::new(),
            nibbles: 0,
            seqno: rhtn_archive::tx::Seqno {
                series: 1,
                counter: 0,
            },
            subnet: Some(SUBNET),
        },
    );
    view.store = rhtn_node::store::TopologyStore::load(&cfg.topology).expect("the store reads");
    view
}

/// The routing slots as a view holds them: the number and who is in it.
type Slots = Vec<(u64, Option<[u8; 32]>)>;

/// The table and the slots a full replay of the whole store produces.
/// What a view restored from the derived copy holds.  **Not a replay**: a
/// node keeps the seen-set and the table it produced, never a third
/// party's acts (`infra-client-requirements.md` §4.3, design §15.1.1
/// [author, 2026-09-23]), so there is no record set to fold again.
fn restored(cfg: &rhtn_daemon::config::Config) -> (Vec<[u8; 32]>, Slots) {
    let mut view = bare(cfg);
    let snap = std::fs::read(cfg.topology.join("derived"))
        .ok()
        .and_then(|b| rhtn_archive::topology::Snapshot::decode(&b));
    view.restore_materialised(snap.as_ref(), &ids());
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
    std::fs::write(
        &l.peers,
        ["carol", "w1", "witness"]
            .map(|n| format!("{}\n", hex(&test_identity(n).public.key_material())))
            .concat(),
    )
    .unwrap();
    let cfg = Config::read(&l.config).unwrap();
    let (pop, adoption) = adoption_under_slot("bob", "carol", 0);
    // one adoption accepted, then written out with the derived view beside
    // the store
    {
        let s = Service::start(&cfg, &l.peers).await.expect("starts");
        let mut view = s.node.view.lock().unwrap();
        view.store.keep_presence(
            rhtn_archive::record::Record::parse(&pop).unwrap().txid,
            pop.clone(),
        );
        let me = view.me();
        assert_eq!(
            view.take_object(&s.node.adjacency, &me, KIND_TRANSACTION, &adoption, &ids()),
            Decision::Stored
        );
        drop(view);
        s.persist().expect("persists");
    }
    let before = std::fs::read(cfg.topology.join("derived")).expect("the derived view is written");
    let snap = Snapshot::decode(&before).expect("and it reads");
    assert!(
        snap.high.is_some(),
        "the watermark names the transaction folded in"
    );
    // a second adoption is added to the store without the derived view
    // being told: it sorts above the watermark, so a wake folds it alone
    let (pop2, adoption2) = adoption_under_slot("bob", "w1", 1);
    {
        let s = Service::start(&cfg, &l.peers).await.expect("starts again");
        let mut view = s.node.view.lock().unwrap();
        view.store.keep_presence(
            rhtn_archive::record::Record::parse(&pop2).unwrap().txid,
            pop2.clone(),
        );
        let me = view.me();
        assert_eq!(
            view.take_object(&s.node.adjacency, &me, KIND_TRANSACTION, &adoption2, &ids()),
            Decision::Stored
        );
        drop(view);
        s.persist().expect("persists");
    }
    // started once more: the second record is the only one folded, and the
    // view is what a full replay would have produced
    let s = Service::start(&cfg, &l.peers)
        .await
        .expect("starts a third time");
    let view = s.node.view.lock().unwrap();
    let held: Vec<[u8; 32]> = view.table.subordinates(&kh("bob")).into_iter().collect();
    let slots: Slots = view.slots.iter().map(|(n, sl)| (*n, sl.occupant)).collect();
    drop(view);
    let (subs, replay_slots) = restored(&cfg);
    assert_eq!(held, subs, "the same table a replay produces");
    assert_eq!(slots, replay_slots, "and the same slots");
    assert!(
        held.contains(&kh("carol")) && held.contains(&kh("w1")),
        "both bindings, one from the fold and one from the snapshot"
    );
    // and the fold, run against the snapshot the second stop wrote, touches
    // nothing: the snapshot already accounts for the whole store
    {
        let snap = Snapshot::decode(&std::fs::read(cfg.topology.join("derived")).unwrap()).unwrap();
        assert_eq!(
            snap.folded,
            bare(&cfg).materialise().folded,
            "the digest names the whole store"
        );
        let mut fresh = bare(&cfg);
        assert_eq!(
            fresh.restore_materialised(Some(&snap), &ids()),
            Restored::Current,
            "nothing replayed and nothing folded"
        );
        assert_eq!(
            fresh
                .table
                .subordinates(&kh("bob"))
                .into_iter()
                .collect::<Vec<_>>(),
            subs
        );
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
    std::fs::write(
        &l.peers,
        ["carol", "witness"]
            .map(|n| format!("{}\n", hex(&test_identity(n).public.key_material())))
            .concat(),
    )
    .unwrap();
    let cfg = Config::read(&l.config).unwrap();
    let (pop, adoption) = adoption_under_slot("bob", "carol", 0);
    {
        let s = Service::start(&cfg, &l.peers).await.expect("starts");
        let mut view = s.node.view.lock().unwrap();
        view.store.keep_presence(
            rhtn_archive::record::Record::parse(&pop).unwrap().txid,
            pop.clone(),
        );
        let me = view.me();
        assert_eq!(
            view.take_object(&s.node.adjacency, &me, KIND_TRANSACTION, &adoption, &ids()),
            Decision::Stored
        );
        drop(view);
        s.persist().expect("persists");
    }
    let good = Snapshot::decode(&std::fs::read(cfg.topology.join("derived")).unwrap()).unwrap();
    // **a discarded view leaves nothing**, where it once left a replay:
    // a node keeps the seen-set and the table it produced, never a third
    // party's acts, so the fold has no input to run again (design §15.1.1,
    // `infra-client-requirements.md` §4.3 [author, 2026-09-23]).  What the
    // good view restores is the control
    assert_eq!(
        restored(&cfg).0,
        vec![kh("carol")],
        "control: the view restores it"
    );
    let expected: Vec<[u8; 32]> = Vec::new();
    let fresh = || bare(&cfg);
    // a view whose table bytes do not read
    let damaged = Snapshot {
        table: b"not a table".to_vec(),
        ..good.clone()
    };
    let mut v = fresh();
    assert_eq!(
        v.restore_materialised(Some(&damaged), &ids()),
        Restored::Replayed { replayed: 0 }
    );
    assert_eq!(
        v.table
            .subordinates(&kh("bob"))
            .into_iter()
            .collect::<Vec<_>>(),
        expected,
        "the damaged view is discarded and nothing stands in for it"
    );
    // a view claiming more records than the store holds
    let other_input = Snapshot {
        folded: [9; 32],
        ..good.clone()
    };
    let mut v = fresh();
    assert_eq!(
        v.restore_materialised(Some(&other_input), &ids()),
        Restored::Replayed { replayed: 0 }
    );
    assert_eq!(
        v.table
            .subordinates(&kh("bob"))
            .into_iter()
            .collect::<Vec<_>>(),
        expected
    );
    // a watermark above everything held is harmless, because the digest and
    // not the watermark is the test: every record falls below it, and the
    // digest still has to name exactly those records
    let ahead = Snapshot {
        high: Some((u64::MAX, [0xff; 32])),
        ..good.clone()
    };
    let mut v = fresh();
    assert_eq!(
        v.restore_materialised(Some(&ahead), &ids()),
        Restored::Current
    );
    assert_eq!(
        v.table
            .subordinates(&kh("bob"))
            .into_iter()
            .collect::<Vec<_>>(),
        vec![kh("carol")]
    );
    // and no view at all
    let mut v = fresh();
    assert_eq!(
        v.restore_materialised(None, &ids()),
        Restored::Replayed { replayed: 0 }
    );
    assert_eq!(
        v.table
            .subordinates(&kh("bob"))
            .into_iter()
            .collect::<Vec<_>>(),
        expected
    );
    // the good one, for contrast: taken, and the binding with it
    let mut v = fresh();
    assert_eq!(
        v.restore_materialised(Some(&good), &ids()),
        Restored::Current
    );
    assert_eq!(
        v.table
            .subordinates(&kh("bob"))
            .into_iter()
            .collect::<Vec<_>>(),
        vec![kh("carol")]
    );
    let _ = std::fs::remove_dir_all(&l.dir);
}

// acceptance: PAY-09
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn nothing_the_daemon_keeps_says_who_asked_for_whose_node_key() {
    use rhtn_archive::node_key::*;
    use rhtn_daemon::config::Config;
    use rhtn_daemon::service::Service;
    let l = layout("node-key-askers");
    std::fs::write(
        &l.peers,
        ["alice", "carol"]
            .map(|n| format!("{}\n", hex(&test_identity(n).public.key_material())))
            .concat(),
    )
    .unwrap();
    let cfg = Config::read(&l.config).unwrap();
    let s = Service::start(&cfg, &l.peers).await.expect("starts");
    let addr = s.node.addr;
    // several requests and a batch, each on a connection under a key
    // minted for it: w3 to w6 are those keys, never told to the daemon
    let a = node_key(addr, "w3", "alice", [1; 16]).await;
    let c = node_key(addr, "w4", "carol", [2; 16]).await;
    assert_eq!(node_key(addr, "w5", "alice", [3; 16]).await, a);
    let both = NodeKeyRequest::for_recipients(vec![kh("alice"), kh("carol")], [4; 16]).unwrap();
    let replies =
        decode_batch_reply(&ask_minted(addr, "w6", REQUEST_NODE_KEY, &both.encode()).await)
            .unwrap();
    let mut served: Vec<Vec<u8>> = replies.into_iter().map(|r| r.outcome.unwrap()).collect();
    served.sort();
    let mut expected = vec![a, c];
    expected.sort();
    assert_eq!(served, expected, "the batch is answered from the same keys");
    // a stray file beside the keys is not a recipient, and does not stop
    // the node writing its state back
    std::fs::create_dir_all(cfg.keys.join("node-keys")).unwrap();
    std::fs::write(
        cfg.keys.join("node-keys").join("stray"),
        b"a file, not a recipient",
    )
    .unwrap();
    s.persist().expect("written back");
    s.persist().expect("and again");
    assert!(
        cfg.topology.join("derived").exists(),
        "the derived view is written"
    );
    // every byte the daemon keeps, in its names and its files
    let mut all = Vec::new();
    for f in walk(&l.dir) {
        all.extend(f.to_string_lossy().as_bytes());
        all.push(0);
        all.extend(std::fs::read(&f).unwrap_or_default());
        all.push(0);
    }
    for who in ["w3", "w4", "w5", "w6"] {
        let id = test_identity(who);
        for (what, v) in [
            ("keyhash", id.public.keyhash.to_vec()),
            ("presented key", id.public.ed.to_bytes().to_vec()),
        ] {
            let h = hex(&v);
            assert!(
                !all.windows(v.len()).any(|w| w == v.as_slice()),
                "{who}'s {what} raw"
            );
            assert!(
                !all.windows(h.len()).any(|w| w == h.as_bytes()),
                "{who}'s {what} in hex"
            );
        }
    }
    // which subjects were asked for, and how often, is not kept either: one
    // key file per recipient, alice's asked for three times
    for (who, asked) in [("alice", 3), ("carol", 2)] {
        let files = walk(&cfg.keys.join("node-keys").join(hex(&kh(who))));
        assert_eq!(
            files.len(),
            1,
            "{who}, asked {asked} times: one key and nothing else"
        );
    }
    let _ = std::fs::remove_dir_all(&l.dir);
}

fn walk(dir: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    if let Ok(rd) = std::fs::read_dir(dir) {
        for e in rd.flatten() {
            if e.path().is_dir() {
                out.extend(walk(&e.path()));
            } else {
                out.push(e.path());
            }
        }
    }
    out
}

// acceptance: DMN-16
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_derived_view_belonging_to_another_identity_is_discarded() {
    use rhtn_archive::topology::{Restored, Snapshot};
    use rhtn_daemon::config::Config;
    use rhtn_daemon::service::Service;
    use rhtn_node::store::{Decision, KIND_TRANSACTION};
    let l = layout("foreign-view");
    std::fs::write(
        &l.peers,
        ["carol", "witness"]
            .map(|n| format!("{}\n", hex(&test_identity(n).public.key_material())))
            .concat(),
    )
    .unwrap();
    let cfg = Config::read(&l.config).unwrap();
    let (pop, adoption) = adoption_under_slot("bob", "carol", 0);
    {
        let s = Service::start(&cfg, &l.peers).await.expect("starts");
        let mut view = s.node.view.lock().unwrap();
        view.store.keep_presence(
            rhtn_archive::record::Record::parse(&pop).unwrap().txid,
            pop.clone(),
        );
        let me = view.me();
        assert_eq!(
            view.take_object(&s.node.adjacency, &me, KIND_TRANSACTION, &adoption, &ids()),
            Decision::Stored
        );
        drop(view);
        s.persist().expect("persists");
    }
    let expected = restored(&cfg);
    assert_eq!(
        expected.0,
        vec![kh("carol")],
        "control: bob holds carol as a subordinate in slot 0"
    );

    // carol's own view of the same adoption: the same records, a different
    // owner, and different subordinate slots — carol has none
    let carol = std::sync::Arc::new(test_identity("carol"));
    let mut theirs = rhtn_node::view::NodeView::new(
        carol.clone(),
        rhtn_archive::tx::Locator {
            anchor: carol.public.keyhash,
            path: Vec::new(),
            nibbles: 0,
            seqno: rhtn_archive::tx::Seqno {
                series: 1,
                counter: 0,
            },
            subnet: Some(SUBNET),
        },
    );
    theirs.store = rhtn_node::store::TopologyStore::load(&cfg.topology).unwrap();
    let foreign: Snapshot = theirs.materialise();

    // **whose view this is, is this node's own answer.**  Installing
    // carol's would replace bob's slots with carol's, which are empty
    let mut v = bare(&cfg);
    assert_eq!(
        v.restore_materialised(Some(&foreign), &ids()),
        Restored::Replayed { replayed: 0 },
        "another identity's derived view is discarded"
    );
    assert_eq!(
        v.table
            .subordinates(&kh("bob"))
            .into_iter()
            .collect::<Vec<_>>(),
        Vec::<[u8; 32]>::new(),
        "and nothing of theirs is installed under this node"
    );
    assert_eq!(
        v.slots
            .iter()
            .map(|(n, s)| (*n, s.occupant))
            .collect::<Slots>(),
        Slots::new(),
        "and none of their routing slots either"
    );
    let _ = std::fs::remove_dir_all(&l.dir);
}

// functional: SES-018
/// **A member a record introduces is pinned when it arrives, and again
/// from the store at start** (`infra-client-requirements.md` §1; SES-018):
/// carol is in no peers file, so the node holds nothing of her until the
/// adoption carrying her material arrives on the node's own session, and
/// from then on she may attach — across a restart too, where a node that
/// re-read its peers file alone refused the member it had learned
/// [bench, 2026-11-12].
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_member_a_record_introduces_is_pinned_on_arrival_and_again_after_a_restart() {
    use rhtn_daemon::config::Config;
    use rhtn_daemon::service::Service;
    use rhtn_node::store::KIND_TRANSACTION;
    let l = layout("introduced");
    std::fs::write(
        &l.peers,
        format!("{}\n", hex(&test_identity("witness").public.key_material())),
    )
    .unwrap();
    let cfg = Config::read(&l.config).unwrap();
    let (pop, adoption) = adoption_under_slot_carrying("bob", "carol", 0, true);
    {
        let s = Service::start(&cfg, &l.peers).await.expect("starts");
        let pins = s.node.node.cfg.pins.clone();
        assert!(
            pins.classical_key(&kh("carol")).is_none(),
            "carol is nobody to this node yet"
        );
        let me = {
            let mut view = s.node.view.lock().unwrap();
            let pop_id = rhtn_archive::record::Record::parse(&pop).unwrap().txid;
            view.store.keep_presence(pop_id, pop.clone());
            view.me()
        };
        // the record arrives as the operator's own push would, on the control path
        let on_control = s
            .node
            .node
            .cfg
            .on_control
            .clone()
            .expect("a control handler");
        on_control(
            me,
            5,
            rhtn_archive::topology::encode_push(KIND_TRANSACTION, &adoption),
        );
        assert!(
            pins.classical_key(&kh("carol")).is_some(),
            "pinned on arrival"
        );
        assert!(
            s.node
                .view
                .lock()
                .unwrap()
                .store
                .transactions()
                .any(|r| r.carried_identity().is_some()),
            "the record is held with the material it carries"
        );
        s.persist().expect("persists");
    }
    let s = Service::start(&cfg, &l.peers).await.expect("starts again");
    assert!(
        s.node.node.cfg.pins.classical_key(&kh("carol")).is_some(),
        "pinned again from the store at start"
    );
    let _ = std::fs::remove_dir_all(&l.dir);
}

// functional: SES-018
/// **A sibling's member is learned from a push by any node in this node's
/// horizon, and pinned once its record is stored**
/// (`infra-client-requirements.md` §4.4) [author, 2026-11-18: "All
/// identities within the trust horizon are trusted"]: bob and alice are
/// both pat's, alice adopts carol, and pat's node relays the adoption to
/// bob. carol is two edges from bob — a sibling edge and an adoption — so
/// the record is within bob's horizon, and carol may fail over to bob
/// when alice's node stops answering. The same record from a node outside
/// the horizon introduces nobody.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_siblings_member_relayed_by_the_patron_is_pinned_and_a_stranger_introduces_nobody() {
    use rhtn_daemon::config::Config;
    use rhtn_daemon::service::Service;
    use rhtn_node::store::KIND_TRANSACTION;
    let l = layout("sibling-introduced");
    std::fs::write(
        &l.peers,
        ["pat", "witness"]
            .map(|n| format!("{}\n", hex(&test_identity(n).public.key_material())))
            .concat(),
    )
    .unwrap();
    let cfg = Config::read(&l.config).unwrap();
    let s = Service::start(&cfg, &l.peers).await.expect("starts");
    let pins = s.node.node.cfg.pins.clone();
    let on_control = s
        .node
        .node
        .cfg
        .on_control
        .clone()
        .expect("a control handler");
    let me = s.node.view.lock().unwrap().me();
    let push = |from: [u8; 32], (pop, adoption): (Vec<u8>, Vec<u8>)| {
        {
            let mut view = s.node.view.lock().unwrap();
            let pop_id = rhtn_archive::record::Record::parse(&pop).unwrap().txid;
            view.store.keep_presence(pop_id, pop);
        }
        on_control(
            from,
            5,
            rhtn_archive::topology::encode_push(KIND_TRANSACTION, &adoption),
        );
    };
    let held = |who: &str| {
        s.node
            .view
            .lock()
            .unwrap()
            .store
            .transactions()
            .any(|r| r.carried_identity().is_some_and(|id| id.keyhash == kh(who)))
    };
    // the operator's replay places bob and his sibling under pat
    push(me, adoption_under_slot_carrying("pat", "bob", 0, true));
    push(me, adoption_under_slot_carrying("pat", "alice", 1, true));
    assert!(held("alice"), "the sibling is in the table");

    // a node outside the horizon relays alice's adoption of carol
    let stranger = kh("mallory");
    push(
        stranger,
        adoption_under_slot_carrying("alice", "carol", 0, true),
    );
    assert!(!held("carol"), "a stranger's push is not stored");
    assert!(
        pins.classical_key(&kh("carol")).is_none(),
        "and introduces nobody"
    );

    // pat's node relays the same adoption
    push(
        kh("pat"),
        adoption_under_slot_carrying("alice", "carol", 0, true),
    );
    assert!(
        held("carol"),
        "the sibling's member is within two edges and stored"
    );
    assert!(pins.classical_key(&kh("carol")).is_some(), "and pinned");
    let _ = std::fs::remove_dir_all(&l.dir);
}

/// **A doorbell registered before a restart is rung after it**
/// (`infra-client-requirements.md` §6.1): the daemon keeps the register
/// beside its node keys, written as each registration is taken, so the
/// stop needs no write-back to keep it; and a withdrawal stays withdrawn.
// functional: MAIL-020
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_wake_registration_is_rung_after_a_restart_and_a_withdrawal_stays_gone() {
    use rhtn_archive::submission::{
        SUBMISSION_ACCEPTED, SubmissionReply, WakeEndpoint, WakeRegistration,
    };
    use rhtn_daemon::config::Config;
    use rhtn_daemon::service::Service;
    let l = layout("wake");
    let cfg = Config::read(&l.config).unwrap();
    let device = [9u8; 32];
    let register = |view: &mut rhtn_node::view::NodeView, who: &str, nonce: u8, at| {
        let body = WakeRegistration::of([nonce; 16], at).encode();
        let reply = rhtn_node::submissions::wake(view, &kh(who), &device, &body)
            .expect("a registration is answered");
        assert_eq!(
            SubmissionReply::decode(&reply).unwrap().code,
            SUBMISSION_ACCEPTED
        );
    };
    {
        let s = Service::start(&cfg, &l.peers).await.expect("starts");
        let mut view = s.node.view.lock().unwrap();
        register(
            &mut view,
            "alice",
            1,
            Some(WakeEndpoint {
                url: "https://push.example/rhtn/a3f9".into(),
                key: vec![7; 65],
                signing_key: [3; 32],
                lapses_at: Some(1_900_000_000),
            }),
        );
        register(
            &mut view,
            "carol",
            2,
            Some(WakeEndpoint {
                url: "https://push.example/rhtn/c0c0".into(),
                key: vec![8; 65],
                signing_key: [4; 32],
                lapses_at: None,
            }),
        );
        register(&mut view, "carol", 3, None);
        // stopped with nothing written back
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let dir = cfg.keys.join("wake");
        let mode = |p: &Path| std::fs::metadata(p).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode(&dir), 0o700, "the directory is the owner's alone");
        let files = walk(&dir);
        assert_eq!(files.len(), 1, "alice's endpoint, and carol's is gone");
        assert_eq!(mode(&files[0]), 0o600, "and so is the file");
    }
    let s = Service::start(&cfg, &l.peers).await.expect("starts again");
    let view = s.node.view.lock().unwrap();
    assert!(
        view.ring_for(&kh("alice"), &device, true),
        "the doorbell registered before the restart is owed after it"
    );
    let rung = view.take_rings();
    assert_eq!(rung.len(), 1);
    assert_eq!(rung[0].url, "https://push.example/rhtn/a3f9");
    assert_eq!(rung[0].key, vec![7; 65]);
    assert_eq!(
        rung[0].signing_key, [3; 32],
        "with the key the post is signed with, or it could not be rung"
    );
    assert!(
        !view.ring_for(&kh("carol"), &device, true),
        "a withdrawal before the restart stays withdrawn after it"
    );
    drop(view);
    let _ = std::fs::remove_dir_all(&l.dir);
}
