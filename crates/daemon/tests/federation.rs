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

/// **The key a node presents**, which a test can know in advance: a node
/// holding its own seed presents that identity's classical member, so
/// both sides of a two-node test can be configured for each other before
/// either starts ([`rhtn_transport::tls::Presenter`]).
fn presents(who: &str) -> String {
    hex(&test_identity(who)
        .ed_signing_key()
        .verifying_key()
        .to_bytes())
}

fn node(tag: &str, who: &str, port: u16, hosting: &str) -> Node {
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
             seconds = 60\n\n[federation]\nlisten = \"127.0.0.1:{port}\"\n\
             store = \"{}\"\n",
            dir.join("identity.key").display(),
            dir.join("queue").display(),
            dir.join("prekeys").display(),
            dir.join("topology").display(),
            dir.join("archive").display(),
            hosting_path.display(),
            dir.join("grants").display(),
            dir.join("federation").display(),
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

/// Ports fixed, so each node can be configured for the other before
/// either of them starts.
const A_PORT: u16 = 37461;
const B_PORT: u16 = 37462;

// acceptance: DMN-30
#[test]
fn a_receipt_crosses_from_the_subordinates_node_to_the_patrons_over_tls_with_the_key_pinned() {
    let bob = hex(&test_identity("bob").public.keyhash);
    let alice = hex(&test_identity("alice").public.keyhash);
    let manifest_text = "roles = [\"reader\"]\n";
    let (a_key, b_key) = (presents("bob"), presents("alice"));

    // **node A: the patron's instance**, brokered, with the subordinate
    // named *and located* — an envelope arriving for an instance whose
    // origin has no node configured is refused, because nothing
    // authenticates the claim
    let a = node("a", "bob", A_PORT, "");
    let ma = a.dir.join("front.manifest");
    std::fs::write(&ma, manifest_text).unwrap();
    std::fs::write(
        &a.hosting,
        format!(
            "[[host]]\nresource = \"{PA}\"\nowner = \"{bob}\"\nauthority = \"hall.internal\"\n\
             manifest = \"{}\"\nsubordinates = [\"{SB}\"]\n\
             [host.at]\n\"{SB}\" = {{ address = \"127.0.0.1:{B_PORT}\", key = \"{b_key}\" }}\n",
            ma.display()
        ),
    )
    .unwrap();
    let ra = Running::start(&a);
    let (a_addr, said_key) = ra.federation();
    assert_eq!(a_addr.port(), A_PORT);
    assert_eq!(said_key, a_key, "a node presents its identity's own key");

    // **node B: the subordinate's instance** on a socket of its own, its
    // patron located at node A — first with the wrong key, which the
    // sender refuses to deliver to
    let (origin_addr, _handed) = origin();
    let b = node("b", "alice", B_PORT, "");
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

/// **A caller with a key of its own cannot speak for an instance it does
/// not host** [2026-10-27]: the listener binds the key the handshake
/// established to the node configured for the claimed origin *before* the
/// plane is touched, so a forged origin changes no record's revision.
///
/// This is the shape of the defect it closes: an envelope claiming the
/// patron as origin, at revision `u64::MAX`, from a caller holding a
/// perfectly valid but unrelated key. Taken, it would have set the
/// record's watermark to the largest revision there is, and every
/// legitimate revision after it would have been classified as older and
/// never delivered — a record suppressed for good by a stranger, which no
/// application-side signature check could have undone.
// acceptance: DMN-29
#[test]
fn a_node_that_does_not_host_the_origin_cannot_speak_for_it() {
    use rhtn_node::federation::Envelope;
    const PORT: u16 = 37463;
    let bob = hex(&test_identity("bob").public.keyhash);
    let a = node("forged", "bob", PORT, "");
    let m = a.dir.join("front.manifest");
    std::fs::write(&m, "roles = [\"reader\"]\n").unwrap();
    // the patron's instance is hosted here with two relationships: a
    // subordinate at a node presenting alice's key, and kin at one
    // presenting carol's
    const KIN: &str = "0e0e0e0e0e0e0e0e0e0e0e0e0e0e0e0e0e0e0e0e0e0e0e0e0e0e0e0e0e0e0e0e";
    std::fs::write(
        &a.hosting,
        format!(
            "[[host]]\nresource = \"{PA}\"\nowner = \"{bob}\"\nauthority = \"hall.internal\"\n\
             manifest = \"{}\"\nsubordinates = [\"{SB}\"]\n\
             [host.kin]\nguild = [\"{KIN}\"]\n\
             [host.at]\n\"{SB}\" = {{ address = \"127.0.0.1:1\", key = \"{}\" }}\n\
             \"{KIN}\" = {{ address = \"127.0.0.1:2\", key = \"{}\" }}\n",
            m.display(),
            presents("alice"),
            presents("carol")
        ),
    )
    .unwrap();
    let r = Running::start(&a);
    let (at, _) = r.federation();

    // a caller with its own valid key — carol's, which is nobody's node
    // here — claiming the configured subordinate as origin
    let forged = Envelope::live(
        [0x0b; 32],
        "policy-1",
        rhtn_node::federation::SCHEMA_DIRECTIVE,
        u64::MAX,
        b"{\"mine\":true}",
    );
    let (status, said) = put_envelope(at, "carol", PA, &forged);
    assert_eq!(status, 403, "{said}");
    assert!(
        said.contains("presented key") || said.contains("no node is configured"),
        "it says which claim it could not authenticate: {said}"
    );

    // **a node configured for one instance cannot speak for another**:
    // alice's key is the subordinate's node, not the kin instance's
    let as_kin = Envelope::live(
        [0x0e; 32],
        "policy-1",
        rhtn_node::federation::SCHEMA_DIRECTIVE,
        1,
        b"{\"theirs\":true}",
    );
    let (status, said) = put_envelope(at, "alice", PA, &as_kin);
    assert_eq!(status, 403, "{said}");
    assert!(said.contains("presented key"), "{said}");

    // nothing was held, so the legitimate revision that follows is new
    let legitimate = Envelope::live(
        [0x0b; 32],
        "policy-1",
        rhtn_node::federation::SCHEMA_DIRECTIVE,
        1,
        b"{\"retention_floor_days\":90}",
    );
    let (status, said) = put_envelope(at, "alice", PA, &legitimate);
    assert_eq!(status, 200, "{said}");
    assert!(said.contains("New"), "the record was not poisoned: {said}");

    // **a path already on an envelope is metadata, not authority**: it
    // changes nothing about who may deliver, and the origin's own node
    // still may
    let mut travelled = Envelope::live(
        [0x0b; 32],
        "policy-2",
        rhtn_node::federation::SCHEMA_DIRECTIVE,
        1,
        b"{\"second\":true}",
    );
    travelled.path = vec![[0x0e; 32]];
    let (status, said) = put_envelope(at, "alice", PA, &travelled);
    assert_eq!(status, 200, "{said}");
    assert!(said.contains("New"), "{said}");
    // and carol, who is somebody's node but not this origin's, still
    // cannot deliver it
    let (status, said) = put_envelope(at, "carol", PA, &travelled);
    assert_eq!(status, 403, "{said}");
    r.stop();
    let _ = std::fs::remove_dir_all(&a.dir);
}

/// Put one envelope to a node's federation listener, presenting the key
/// of the named test identity: the sender's half of the exchange, written
/// out here so a test can be the stranger.
fn put_envelope(
    at: SocketAddr,
    as_who: &str,
    to: &str,
    e: &rhtn_node::federation::Envelope,
) -> (u16, String) {
    use rhtn_transport::tls::{Presenter, client_config_with};
    use std::sync::Arc;
    let me = Presenter::own(&test_identity(as_who));
    let mut cfg = client_config_with(
        &me,
        vec![rustls::crypto::aws_lc_rs::kx_group::X25519MLKEM768],
    );
    cfg.alpn_protocols = vec![b"http/1.1".to_vec()];
    cfg.enable_early_data = false;
    let tcp = std::net::TcpStream::connect(at).expect("the listener accepts");
    tcp.set_read_timeout(Some(Duration::from_secs(10))).unwrap();
    let name = rustls_pki_types::ServerName::IpAddress(at.ip().into());
    let conn = rustls::ClientConnection::new(Arc::new(cfg), name).expect("a client");
    let mut tls = rustls::StreamOwned::new(conn, tcp);
    let body = serde_json::json!({"to": to, "envelope": e.to_json()}).to_string();
    let request = format!(
        "PUT /federation/envelope HTTP/1.1\r\nhost: {at}\r\ncontent-type: application/json\r\n\
         content-length: {}\r\nconnection: close\r\n\r\n{body}",
        body.len()
    );
    tls.write_all(request.as_bytes()).expect("written");
    tls.flush().ok();
    let mut answer = Vec::new();
    let _ = tls.read_to_end(&mut answer);
    let text = String::from_utf8_lossy(&answer).to_string();
    let status = text
        .lines()
        .next()
        .and_then(|l| l.split(' ').nth(1))
        .and_then(|c| c.parse().ok())
        .unwrap_or(0);
    (status, text)
}

/// **A restart finds what the node had taken** (`Custody`;
/// `infra-client-requirements.md` §9.2, OPS-022): an envelope admitted
/// and not yet applied is still the node's to deliver on the other side
/// of a stop, and the watermark that refuses a replay is still there.
///
/// The defect this closes: the planes were in memory, so a `2xx` said *I
/// have it* when nothing had it. The sender dropped its copy on that
/// answer and no surviving component knew the message had to be sent
/// again — which application idempotence cannot fix, since it makes a
/// redelivery harmless and cannot recreate a message nobody knows to
/// resend.
// acceptance: DMN-31
#[test]
fn an_envelope_admitted_and_not_applied_survives_a_restart() {
    use rhtn_node::federation::Envelope;
    const PORT: u16 = 37464;
    let bob = hex(&test_identity("bob").public.keyhash);
    let a = node("custody", "bob", PORT, "");
    let m = a.dir.join("front.manifest");
    std::fs::write(&m, "roles = [\"reader\"]\n").unwrap();
    // brokered, so nothing takes the envelope off the node's hands: it
    // stays admitted, which is the state a restart has to find. Its
    // patron is located at a node presenting alice's key, which is what
    // the sender below holds.
    std::fs::write(
        &a.hosting,
        format!(
            "[[host]]\nresource = \"{PA}\"\nowner = \"{bob}\"\nauthority = \"hall.internal\"\n\
             manifest = \"{}\"\npatron = \"{SB}\"\n\
             [host.at]\n\"{SB}\" = {{ address = \"127.0.0.1:1\", key = \"{}\" }}\n",
            m.display(),
            presents("alice")
        ),
    )
    .unwrap();

    let r = Running::start(&a);
    let (at, _) = r.federation();
    let e = Envelope::live(
        [0x0b; 32],
        "policy-1",
        rhtn_node::federation::SCHEMA_DIRECTIVE,
        7,
        b"{\"retention_floor_days\":90}",
    );
    let (status, said) = put_envelope(at, "alice", PA, &e);
    assert_eq!(status, 200, "{said}");
    assert!(said.contains("New"), "{said}");
    r.stop();

    // and again, on the same files
    let again = Running::start(&a);
    let (at, _) = again.federation();
    let picked = again
        .line(Duration::from_secs(20), |l| {
            l.starts_with("rhtnd: federation: 1 envelope(s) to deliver")
        })
        .unwrap_or_else(|| {
            panic!(
                "it did not pick up what it was holding; it said {:?}",
                again.lines.lock().unwrap()
            )
        });
    assert!(picked.contains("out of custody"), "{picked}");

    // **the watermark survived too**, so the same revision is a
    // duplicate and an older one is older — not, as an in-memory plane
    // would have had it, both of them new
    let (status, said) = put_envelope(at, "alice", PA, &e);
    assert_eq!(status, 200, "{said}");
    assert!(said.contains("Duplicate"), "{said}");
    let older = Envelope::live(
        [0x0b; 32],
        "policy-1",
        rhtn_node::federation::SCHEMA_DIRECTIVE,
        6,
        b"{\"retention_floor_days\":30}",
    );
    let (_, said) = put_envelope(at, "alice", PA, &older);
    assert!(
        said.contains("Older"),
        "the replay protection is durable: {said}"
    );
    again.stop();
    let _ = std::fs::remove_dir_all(&a.dir);
}
