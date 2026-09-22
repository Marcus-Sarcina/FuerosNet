//! `rhtnd` as an instance (design §23.3, `infra-client-requirements.md`
//! §7): no seed, a transport key it minted, a run its operator's client
//! signed, and the operator-signed records it serves in place of any it
//! would mint.  The daemon is a process here, started from a
//! configuration file, as the operator would start it.

use rhtn_crypto::delegation::issue_run;
use rhtn_crypto::identity::testkit::test_identity;
use rhtn_crypto::verify;
use rhtn_node::propagation::decode_push;
use rhtn_node::resolution::{NetworkPoint, anchor_entry, endpoint_record};
use rhtn_node::store::{KIND_DELEGATION, KIND_ENDPOINT_RECORD};
use rhtn_transport::session::*;
use rhtn_transport::tls::{self, Party, Pins};
use std::collections::BTreeMap;
use std::io::{BufRead, BufReader};
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

const WINDOW: u64 = rhtn_codec::schema::DELEGATION_WINDOW_SECONDS;

fn kh(n: &str) -> [u8; 32] {
    test_identity(n).public.keyhash
}

fn hex(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect()
}

fn unhex(s: &str) -> Vec<u8> {
    (0..s.len() / 2)
        .map(|i| u8::from_str_radix(&s[i * 2..i * 2 + 2], 16).unwrap())
        .collect()
}

fn now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs()
}

struct Layout {
    dir: PathBuf,
    config: PathBuf,
    peers: PathBuf,
    key: PathBuf,
    delegations: PathBuf,
}

/// An instance for bob: bob's material as the operator, alice as a peer,
/// and whatever records `extra` names in the configuration.
fn layout(tag: &str, extra: &str) -> Layout {
    let dir = std::env::temp_dir().join(format!("rhtnd-deleg-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let (config, peers) = (dir.join("rhtnd.conf"), dir.join("peers"));
    let (key, delegations) = (dir.join("transport.key"), dir.join("delegations"));
    std::fs::write(
        dir.join("operator"),
        format!("{}\n", hex(&test_identity("bob").public.key_material())),
    )
    .unwrap();
    std::fs::write(
        &peers,
        format!("{}\n", hex(&test_identity("alice").public.key_material())),
    )
    .unwrap();
    std::fs::write(
        &config,
        format!(
            "operator = \"{}\"\ntransport-key = \"{}\"\ndelegations = \"{}\"\nlisten = \"127.0.0.1:0\"\nqueue = \"{}\"\nprekeys = \"{}\"\ntopology = \"{}\"\narchive = \"{}\"\nheartbeat = 30\ningestion = \"unverified-gossip\"\n{extra}\n[allowance]\nrequests = 120\nseconds = 60\n",
            dir.join("operator").display(),
            key.display(),
            delegations.display(),
            dir.join("queue").display(),
            dir.join("prekeys").display(),
            dir.join("topology").display(),
            dir.join("archive").display(),
        ),
    )
    .unwrap();
    Layout {
        dir,
        config,
        peers,
        key,
        delegations,
    }
}

/// Write a run into the delegations directory.
fn write_run(dir: &Path, run: &[Vec<u8>]) {
    std::fs::create_dir_all(dir).unwrap();
    for (i, d) in run.iter().enumerate() {
        std::fs::write(dir.join(format!("{i:03}")), d).unwrap();
    }
}

struct Daemon {
    child: Child,
    stderr: Arc<Mutex<Vec<String>>>,
    stdout: BufReader<std::process::ChildStdout>,
}

fn spawn(l: &Layout) -> Daemon {
    let mut child = Command::new(env!("CARGO_BIN_EXE_rhtnd"))
        .arg(&l.config)
        .arg(&l.peers)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("rhtnd starts");
    let stderr = Arc::new(Mutex::new(Vec::new()));
    let sink = stderr.clone();
    let err = child.stderr.take().unwrap();
    std::thread::spawn(move || {
        for line in BufReader::new(err).lines().map_while(Result::ok) {
            sink.lock().unwrap().push(line);
        }
    });
    let stdout = BufReader::new(child.stdout.take().unwrap());
    Daemon {
        child,
        stderr,
        stdout,
    }
}

impl Daemon {
    /// The address, once the daemon says it is serving.
    fn address(&mut self) -> SocketAddr {
        let mut line = String::new();
        self.stdout
            .read_line(&mut line)
            .expect("the daemon says where it is serving");
        let Some((_, addr)) = line.trim().rsplit_once(' ') else {
            std::thread::sleep(Duration::from_millis(200));
            panic!(
                "no address in {line:?}; the daemon said: {:?}",
                self.stderr.lock().unwrap()
            );
        };
        addr.parse().expect("an address and port")
    }
    fn stderr_has(&self, needle: &str) -> bool {
        self.stderr
            .lock()
            .unwrap()
            .iter()
            .any(|l| l.contains(needle))
    }
    fn wait_stderr(&self, needle: &str, secs: u64) -> bool {
        let end = Instant::now() + Duration::from_secs(secs);
        while Instant::now() < end {
            if self.stderr_has(needle) {
                return true;
            }
            std::thread::sleep(Duration::from_millis(50));
        }
        false
    }
    fn stop(mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn client_cfg(name: &str) -> ClientConfig {
    let pins = Pins::new();
    for n in ["alice", "bob"] {
        pins.pin_identity(&test_identity(n).public);
    }
    ClientConfig {
        me: Party::of(Arc::new(test_identity(name))),
        pins,
        bind: Default::default(),
        capabilities: BTreeMap::new(),
        attestation: None,
        filter: None,
        sibling_cache: Arc::new(Mutex::new(Vec::new())),
        addresses: Arc::new(Mutex::new(Default::default())),
        tls: Arc::new(Mutex::new(Default::default())),
        connect_timeout: Duration::from_secs(120),
        on_reachability: None,
        log: Log::default(),
    }
}

async fn attach_to(addr: SocketAddr) -> Session {
    let ep = tls::client_endpoint("127.0.0.1:0".parse().unwrap()).unwrap();
    match attach(&client_cfg("alice"), &ep, kh("bob"), addr, false).await {
        AttachOutcome::Attached(s) => s,
        other => panic!("alice attaches: {other:?}"),
    }
}

fn wait_for(path: &Path, secs: u64) -> bool {
    let end = Instant::now() + Duration::from_secs(secs);
    while Instant::now() < end {
        if path.exists() {
            return true;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    false
}

// acceptance: DMN-23
#[tokio::test]
async fn dmn_23_the_instance_mints_its_key_and_serves_the_whole_run_over_it() {
    let l = layout("mint", "");
    let mut d = spawn(&l);
    // started with no transport key: the daemon generates one and says
    // what its public half is, then waits for the run
    let pub_path = l.key.with_extension("pub");
    assert!(wait_for(&pub_path, 10), "the public half appears");
    assert!(d.wait_stderr("transport key ", 10));
    let public: [u8; 32] = unhex(std::fs::read_to_string(&pub_path).unwrap().trim())
        .try_into()
        .unwrap();
    assert!(
        d.stderr_has(&hex(&public)),
        "only the public half appears in what it sends the operator's client"
    );
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(
            std::fs::metadata(&l.key).unwrap().permissions().mode() & 0o777,
            0o600,
            "the private half stays on the instance"
        );
    }
    // the operator's client signs the run over that key: 45 credentials,
    // contiguous, the first opening so that the boundary passes shortly
    let start = now() - WINDOW + 12;
    let run = issue_run(&test_identity("bob"), &public, start, 45);
    assert_eq!(run.len(), 45);
    let parsed: Vec<_> = run
        .iter()
        .map(|r| verify::delegation_fields(r).unwrap())
        .collect();
    for w in parsed.windows(2) {
        assert_eq!(
            w[0].not_after, w[1].not_before,
            "each not_before is the previous not_after"
        );
    }
    write_run(&l.delegations, &run);
    let addr = d.address();
    // a session before the boundary presents the key under the first credential
    let s1 = attach_to(addr).await;
    assert_eq!(tls::peer_key(&s1.conn), Some(public));
    assert_eq!(s1.ack.delegation.as_deref(), Some(run[0].as_slice()));
    s1.conn.close(0u32.into(), b"");
    // and one after it presents the same key under the next
    tokio::time::sleep(Duration::from_secs(14)).await;
    let s2 = attach_to(addr).await;
    assert_eq!(tls::peer_key(&s2.conn), Some(public));
    assert_eq!(s2.ack.delegation.as_deref(), Some(run[1].as_slice()));
    d.stop();
    let _ = std::fs::remove_dir_all(&l.dir);
}

/// A pre-minted key, so the run can be signed before the daemon starts.
fn premint(l: &Layout, seed: u8) -> [u8; 32] {
    std::fs::write(&l.key, [seed; 32]).unwrap();
    tls::Credential::from_seed(&[seed; 32], kh("bob")).public()
}

// acceptance: DMN-24
#[tokio::test]
async fn dmn_24_the_operator_is_told_before_the_last_credential() {
    let l = layout("notice", "");
    let public = premint(&l, 21);
    // a run at its tail: three credentials, the first in force
    write_run(
        &l.delegations,
        &issue_run(&test_identity("bob"), &public, now() - 60, 3),
    );
    let mut d = spawn(&l);
    let _ = d.address();
    assert!(
        d.wait_stderr("3 credentials remain in the run", 10),
        "raised while more than one remains, naming how many: {:?}",
        d.stderr.lock().unwrap()
    );
    d.stop();
    // a long run raises nothing at start
    let l2 = layout("quiet", "");
    let public = premint(&l2, 22);
    write_run(
        &l2.delegations,
        &issue_run(&test_identity("bob"), &public, now() - 60, 45),
    );
    let mut d2 = spawn(&l2);
    let _ = d2.address();
    std::thread::sleep(Duration::from_millis(300));
    assert!(!d2.stderr_has("remain in the run"));
    d2.stop();
    let _ = std::fs::remove_dir_all(&l.dir);
    let _ = std::fs::remove_dir_all(&l2.dir);
}

fn seqno(counter: u32) -> rhtn_archive::tx::Seqno {
    rhtn_archive::tx::Seqno { series: 1, counter }
}

// acceptance: DMN-25
#[tokio::test]
async fn dmn_25_the_instance_mints_no_record_and_reports_a_moved_address() {
    let dir =
        std::env::temp_dir().join(format!("rhtnd-deleg-moved-records-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    // the operator's record names an address other than the one served on
    let bob = test_identity("bob");
    let record = endpoint_record(
        &bob,
        &[NetworkPoint::new([127, 0, 0, 1], Some(9))],
        seqno(1),
    );
    let record_path = dir.join("endpoint.record");
    std::fs::write(&record_path, &record).unwrap();
    let l = layout(
        "moved",
        &format!("endpoint-record = \"{}\"\n", record_path.display()),
    );
    let public = premint(&l, 23);
    write_run(&l.delegations, &issue_run(&bob, &public, now() - 60, 45));
    let mut d = spawn(&l);
    let addr = d.address();
    assert!(
        d.wait_stderr(
            "the address has moved and a new operator-signed record is owed",
            5
        ),
        "{:?}",
        d.stderr.lock().unwrap()
    );
    // what it pushes to a session: the operator's record and its own
    // delegation, and nothing signed by the daemon
    let mut s = attach_to(addr).await;
    let mut kinds = Vec::new();
    let end = Instant::now() + Duration::from_secs(5);
    while Instant::now() < end && kinds.len() < 2 {
        match tokio::time::timeout(Duration::from_secs(1), s.frames.recv()).await {
            Ok(Some((5, body))) => {
                let (kind, obj) = decode_push(&body).unwrap();
                if kind == KIND_ENDPOINT_RECORD {
                    assert_eq!(obj, record, "the operator-signed record, byte for byte");
                }
                if kind == KIND_DELEGATION {
                    let d = verify::delegation(std::slice::from_ref(&bob.public), &obj).unwrap();
                    assert_eq!(d.key, public);
                }
                assert!(
                    kind == KIND_ENDPOINT_RECORD || kind == KIND_DELEGATION,
                    "kind {kind}"
                );
                kinds.push(kind);
            }
            Ok(Some(_)) => {}
            _ => break,
        }
    }
    kinds.sort();
    assert_eq!(kinds, vec![KIND_ENDPOINT_RECORD, KIND_DELEGATION]);
    d.stop();
    let _ = std::fs::remove_dir_all(&l.dir);
    let _ = std::fs::remove_dir_all(&dir);
}

// acceptance: DMN-26
#[tokio::test]
async fn dmn_26_the_operator_signed_record_and_entry_are_what_the_instance_serves() {
    let dir =
        std::env::temp_dir().join(format!("rhtnd-deleg-given-records-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let bob = test_identity("bob");
    let point = NetworkPoint::new([127, 0, 0, 1], Some(7));
    let record = endpoint_record(&bob, std::slice::from_ref(&point), seqno(1));
    let entry = anchor_entry(&bob, std::slice::from_ref(&point), 3, seqno(1));
    let (rp, ap) = (dir.join("endpoint.record"), dir.join("anchor.entry"));
    std::fs::write(&rp, &record).unwrap();
    std::fs::write(&ap, &entry).unwrap();
    let l = layout(
        "given",
        &format!(
            "endpoint-record = \"{}\"\nanchor-entry = \"{}\"\n",
            rp.display(),
            ap.display()
        ),
    );
    let public = premint(&l, 24);
    write_run(&l.delegations, &issue_run(&bob, &public, now() - 60, 45));
    let mut d = spawn(&l);
    let addr = d.address();
    let mut s = attach_to(addr).await;
    // the record it pushes verifies under the operator's identity and not
    // under the delegated key; the anchor entry was taken the same way, or
    // the daemon would not have started (below)
    let mut saw_record = false;
    let end = Instant::now() + Duration::from_secs(5);
    while Instant::now() < end && !saw_record {
        match tokio::time::timeout(Duration::from_secs(1), s.frames.recv()).await {
            Ok(Some((5, body))) => {
                let (kind, obj) = decode_push(&body).unwrap();
                if kind == KIND_ENDPOINT_RECORD {
                    assert_eq!(obj, record);
                    assert_eq!(
                        verify::record(std::slice::from_ref(&bob.public), "EndpointRecord", &obj),
                        Ok(())
                    );
                    saw_record = true;
                }
            }
            Ok(Some(_)) => {}
            _ => break,
        }
    }
    assert!(
        saw_record,
        "the given endpoint record is pushed in the topology class"
    );
    d.stop();
    // an entry signed by someone other than the operator is refused at start
    let bad = anchor_entry(
        &test_identity("carol"),
        std::slice::from_ref(&point),
        3,
        seqno(1),
    );
    std::fs::write(&ap, &bad).unwrap();
    let d2 = spawn(&l);
    assert!(
        d2.wait_stderr("not this node's anchor entry signed by its operator", 10),
        "{:?}",
        d2.stderr.lock().unwrap()
    );
    d2.stop();
    let _ = std::fs::remove_dir_all(&l.dir);
    let _ = std::fs::remove_dir_all(&dir);
}
