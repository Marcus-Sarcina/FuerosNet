//! **The directive plane between two nodes** (`rhtn_daemon::federation`;
//! `infra-client-requirements.md` §9.2; OPS-022): node A hosts the
//! patron's instance, node B hosts the subordinate's as an HTTP origin on
//! a port of its own; the receipt B's instance emits is carried to A over
//! TLS with A's key pinned, and A holds it for the patron. A wrong key
//! written for A leaves it pending with the refusal said.

use rhtn_crypto::identity::testkit::test_identity;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{SocketAddr, TcpListener};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

const PA: &str = "0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a";
const SB: &str = "0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b";

fn hex(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect()
}

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

/// A node holding its own seed, with a hosting file and a federation
/// listener on a port the system picks.
struct Node {
    dir: PathBuf,
    config: PathBuf,
    peers: PathBuf,
    hosting: PathBuf,
}

fn node(tag: &str, who: &str, hosting: &str) -> Node {
    let dir = std::env::temp_dir().join(format!("rhtnd-fed-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    write_identity(&dir.join("identity.key"), who);
    let peers = dir.join("peers");
    std::fs::write(&peers, "").unwrap();
    let hosting_path = dir.join("hosting");
    std::fs::write(&hosting_path, hosting).unwrap();
    let config = dir.join("rhtnd.conf");
    std::fs::write(
        &config,
        format!(
            "identity = \"{}\"\nlisten = \"127.0.0.1:0\"\nqueue = \"{}\"\nprekeys = \"{}\"\n\
             topology = \"{}\"\narchive = \"{}\"\nresources = \"{}\"\ngrants = \"{}\"\n\
             heartbeat = 30\ningestion = \"unverified-gossip\"\n\n[allowance]\nrequests = 120\n\
             seconds = 60\n\n[federation]\nlisten = \"127.0.0.1:0\"\n",
            dir.join("identity.key").display(),
            dir.join("queue").display(),
            dir.join("prekeys").display(),
            dir.join("topology").display(),
            dir.join("archive").display(),
            hosting_path.display(),
            dir.join("grants").display(),
        ),
    )
    .unwrap();
    Node {
        dir,
        config,
        peers,
        hosting: hosting_path,
    }
}

/// The daemon running, its stdout and stderr lines kept.
struct Running {
    child: Child,
    lines: Arc<Mutex<Vec<String>>>,
}

impl Running {
    fn start(n: &Node) -> Running {
        let mut child = Command::new(env!("CARGO_BIN_EXE_rhtnd"))
            .arg(&n.config)
            .arg(&n.peers)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("rhtnd starts");
        let lines = Arc::new(Mutex::new(Vec::new()));
        for reader in [
            Box::new(BufReader::new(child.stdout.take().unwrap())) as Box<dyn Read + Send>,
            Box::new(BufReader::new(child.stderr.take().unwrap())),
        ] {
            let sink = lines.clone();
            std::thread::spawn(move || {
                for line in BufReader::new(reader).lines().map_while(Result::ok) {
                    sink.lock().unwrap().push(line);
                }
            });
        }
        Running { child, lines }
    }

    /// The first line satisfying `pred`, waiting up to `within`.
    fn line(&self, within: Duration, pred: impl Fn(&str) -> bool) -> Option<String> {
        let end = Instant::now() + within;
        while Instant::now() < end {
            if let Some(l) = self.lines.lock().unwrap().iter().find(|l| pred(l)) {
                return Some(l.clone());
            }
            std::thread::sleep(Duration::from_millis(50));
        }
        None
    }

    /// Where its federation listener is, and the key it presents, from
    /// its own startup line.
    fn federation(&self) -> (SocketAddr, String) {
        let l = self
            .line(Duration::from_secs(20), |l| {
                l.starts_with("rhtnd: federation on ")
            })
            .unwrap_or_else(|| panic!("no federation line; said {:?}", self.lines.lock().unwrap()));
        let rest = l.trim_start_matches("rhtnd: federation on ");
        let (addr, key) = rest.split_once(", key ").expect("addr, key");
        (addr.parse().expect("an address"), key.trim().to_string())
    }

    fn stop(mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

impl Drop for Running {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

/// The subordinate's instance: an HTTP origin that takes its
/// relationships, emits one receipt to its patron on the first outbox
/// poll, and answers everything else with an empty 200. Serves until the
/// test ends.
fn origin() -> (SocketAddr, Arc<Mutex<Vec<String>>>) {
    let l = TcpListener::bind("127.0.0.1:0").expect("a port");
    let addr = l.local_addr().unwrap();
    let got = Arc::new(Mutex::new(Vec::new()));
    let keep = got.clone();
    std::thread::spawn(move || {
        let mut emitted = false;
        for stream in l.incoming() {
            let Ok(mut c) = stream else { continue };
            let mut buf = vec![0u8; 65536];
            let n = c.read(&mut buf).unwrap_or(0);
            let text = String::from_utf8_lossy(&buf[..n]).to_string();
            let line = text.lines().next().unwrap_or("").to_string();
            keep.lock().unwrap().push(text);
            let body = if line.starts_with("GET /.rhtn/outbox") && !emitted {
                emitted = true;
                let receipt = rhtn_node::federation::Envelope::live(
                    [0x0b; 32],
                    "receipt-policy-1",
                    rhtn_node::federation::SCHEMA_RECEIPT,
                    1,
                    b"{\"applied\":true}",
                );
                serde_json::json!({"emit": [{"to": "patron", "envelope": receipt.to_json()}]})
                    .to_string()
            } else {
                String::new()
            };
            let _ = c.write_all(
                format!(
                    "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\n\r\n{body}",
                    body.len()
                )
                .as_bytes(),
            );
        }
    });
    (addr, got)
}

#[test]
fn a_receipt_crosses_from_the_subordinates_node_to_the_patrons_over_tls_with_the_key_pinned() {
    let bob = hex(&test_identity("bob").public.keyhash);
    let alice = hex(&test_identity("alice").public.keyhash);
    let manifest_text = "roles = [\"reader\"]\n";

    // node A: the patron's instance, brokered, with the subordinate named
    let a = node("a", "bob", "");
    let ma = a.dir.join("front.manifest");
    std::fs::write(&ma, manifest_text).unwrap();
    std::fs::write(
        &a.hosting,
        format!(
            "[[host]]\nresource = \"{PA}\"\nowner = \"{bob}\"\nauthority = \"hall.internal\"\n\
             manifest = \"{}\"\nsubordinates = [\"{SB}\"]\n",
            ma.display()
        ),
    )
    .unwrap();
    let ra = Running::start(&a);
    let (a_addr, a_key) = ra.federation();
    assert_eq!(a_key.len(), 64, "{a_key}");

    // node B: the subordinate's instance on a socket of its own, its
    // patron located at node A — first with the wrong key
    let (origin_addr, _handed) = origin();
    let b = node("b", "alice", "");
    let mb = b.dir.join("front.manifest");
    std::fs::write(&mb, manifest_text).unwrap();
    let hosting_b = |key: &str, origin_addr: SocketAddr| {
        format!(
            "[[host]]\nresource = \"{SB}\"\nowner = \"{alice}\"\nauthority = \"club.internal\"\n\
             manifest = \"{}\"\naddress = \"{origin_addr}\"\npatron = \"{PA}\"\n\
             [host.at]\n\"{PA}\" = {{ address = \"{a_addr}\", key = \"{key}\" }}\n",
            mb.display()
        )
    };
    let wrong = "ff".repeat(32);
    std::fs::write(&b.hosting, hosting_b(&wrong, origin_addr)).unwrap();
    let rb = Running::start(&b);
    let refused = rb
        .line(Duration::from_secs(30), |l| {
            l.contains("federation:") && l.contains("presented key")
        })
        .unwrap_or_else(|| panic!("no refusal; B said {:?}", rb.lines.lock().unwrap()));
    assert!(
        refused.contains(&a_key),
        "the refusal names the key actually presented: {refused}"
    );
    assert!(
        ra.lines
            .lock()
            .unwrap()
            .iter()
            .all(|l| !l.contains("receipt-policy-1")),
        "nothing was delivered under the wrong pin: {:?}",
        ra.lines.lock().unwrap()
    );
    rb.stop();

    // the same node B with A's key as A printed it: the first tick's pass
    // asks the origin, which emits once more (its emitted flag is per
    // origin, so a fresh origin is used), and the carrier takes it across
    let (origin_addr, handed2) = origin();
    std::fs::write(&b.hosting, hosting_b(&a_key, origin_addr)).unwrap();
    let rb = Running::start(&b);
    let carried = rb
        .line(Duration::from_secs(30), |l| {
            l.starts_with("rhtnd: federation: carried `receipt-policy-1`")
        })
        .unwrap_or_else(|| panic!("not carried; B said {:?}", rb.lines.lock().unwrap()));
    assert!(carried.contains(&a_addr.to_string()), "{carried}");
    let took = ra
        .line(Duration::from_secs(10), |l| {
            l.starts_with("rhtnd: federation: New: `receipt-policy-1`")
        })
        .unwrap_or_else(|| panic!("A did not take it; A said {:?}", ra.lines.lock().unwrap()));
    assert!(took.contains("for 0a0a0a0a"), "{took}");
    assert!(
        took.contains("caller key"),
        "the caller presented a raw key: {took}"
    );
    // the origin was told its relationships and asked for its outbox
    let seen = handed2.lock().unwrap().clone();
    assert!(
        seen.iter()
            .any(|t| t.starts_with("PUT /.rhtn/relationships")),
        "{seen:?}"
    );
    assert!(
        seen.iter().any(|t| t.starts_with("GET /.rhtn/outbox")),
        "{seen:?}"
    );
    rb.stop();
    ra.stop();
    let _ = std::fs::remove_dir_all(&a.dir);
    let _ = std::fs::remove_dir_all(&b.dir);
}
