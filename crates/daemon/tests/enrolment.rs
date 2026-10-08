//! **An instance enrolled over its own out-of-band surface**
//! (`rhtn_daemon::enrolment`; `infra-client-requirements.md` §7, §8.2;
//! design §23.3): the daemon is a process here, started from a
//! configuration as an operator would start it, and nothing is put in its
//! delegations directory by hand.
//!
//! The exchange under test is the one a provisioning page performs: fetch
//! the public half of the key the instance minted, check it against the
//! token the configuration carried, sign a run over it, hand the run back.

use rhtn_crypto::delegation::issue_run;
use rhtn_crypto::identity::testkit::test_identity;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{SocketAddr, TcpStream};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

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

/// The token a provisioning page would put in the configuration it writes.
const TOKEN: [u8; 32] = [0x5a; 32];

struct Layout {
    dir: PathBuf,
    config: PathBuf,
    peers: PathBuf,
    delegations: PathBuf,
}

/// An instance for bob, with an enrolment surface on a port the operating
/// system picks.
fn layout(tag: &str, token: &[u8; 32]) -> Layout {
    let dir = std::env::temp_dir().join(format!("rhtnd-enrol-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let (config, peers) = (dir.join("rhtnd.conf"), dir.join("peers"));
    let delegations = dir.join("delegations");
    std::fs::write(
        dir.join("operator"),
        format!("{}\n", hex(&test_identity("bob").public.key_material())),
    )
    .unwrap();
    std::fs::write(&peers, "").unwrap();
    std::fs::write(
        &config,
        format!(
            "operator = \"{}\"\n\
             transport-key = \"{}\"\n\
             delegations = \"{}\"\n\
             listen = \"127.0.0.1:0\"\n\
             queue = \"{}\"\nprekeys = \"{}\"\ntopology = \"{}\"\narchive = \"{}\"\n\
             heartbeat = 30\ningestion = \"unverified-gossip\"\n\
             [allowance]\nrequests = 120\nseconds = 60\n\
             [enrolment]\nlisten = \"127.0.0.1:0\"\ntoken = \"{}\"\n",
            dir.join("operator").display(),
            dir.join("transport.key").display(),
            delegations.display(),
            dir.join("queue").display(),
            dir.join("prekeys").display(),
            dir.join("topology").display(),
            dir.join("archive").display(),
            hex(token),
        ),
    )
    .unwrap();
    Layout {
        dir,
        config,
        peers,
        delegations,
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
    fn lines(&self) -> Vec<String> {
        self.stderr.lock().unwrap().clone()
    }

    /// A stderr line containing `needle`, waited for.
    fn wait_line(&self, needle: &str, secs: u64) -> Option<String> {
        let end = Instant::now() + Duration::from_secs(secs);
        while Instant::now() < end {
            if let Some(l) = self.lines().into_iter().find(|l| l.contains(needle)) {
                return Some(l);
            }
            std::thread::sleep(Duration::from_millis(50));
        }
        None
    }

    /// Where the enrolment surface bound, from the line that announces it.
    fn enrolment_at(&self) -> SocketAddr {
        let line = self
            .wait_line("enrolment on", 20)
            .unwrap_or_else(|| panic!("no enrolment line; the daemon said {:?}", self.lines()));
        let after = line.split("enrolment on ").nth(1).expect("an address");
        after
            .split(':')
            .take(2)
            .collect::<Vec<_>>()
            .join(":")
            .parse()
            .expect("an address and port")
    }

    /// The QUIC address, once the daemon says it is serving. Only a daemon
    /// whose run is in force ever prints this.
    fn serving(&mut self) -> Option<String> {
        let mut line = String::new();
        // stdout's first line is `serving on <addr>`; a daemon still
        // waiting prints nothing, so the caller gives up by killing it
        match self.stdout.read_line(&mut line) {
            Ok(0) | Err(_) => None,
            Ok(_) => Some(line.trim().to_string()),
        }
    }

    fn stop(mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

/// One request over the enrolment surface, and the whole response.
fn ask(at: SocketAddr, head: &str, body: &[u8]) -> String {
    let mut s = TcpStream::connect(at).expect("the enrolment surface accepts");
    s.set_read_timeout(Some(Duration::from_secs(10))).unwrap();
    let request = format!(
        "{head}\r\nhost: {at}\r\ncontent-length: {}\r\nconnection: close\r\n\r\n",
        body.len()
    );
    s.write_all(request.as_bytes()).unwrap();
    s.write_all(body).unwrap();
    s.flush().unwrap();
    let mut out = String::new();
    let _ = s.read_to_string(&mut out);
    out
}

/// The value of `field` in a response body, which is `name value` a line.
fn field(response: &str, name: &str) -> Option<String> {
    response
        .lines()
        .find_map(|l| l.strip_prefix(&format!("{name} ")))
        .map(|v| v.trim().to_string())
}

fn status(response: &str) -> u16 {
    response
        .lines()
        .next()
        .and_then(|l| l.split(' ').nth(1).and_then(|c| c.parse().ok()))
        .unwrap_or(0)
}

/// **The exchange a provisioning page performs**, end to end: the instance
/// mints a key nobody gave it, proves it holds the token, and serves once
/// the run signed over that key is handed back.
#[test]
fn an_instance_is_enrolled_over_its_own_surface_and_then_serves() {
    let l = layout("happy", &TOKEN);
    let mut d = spawn(&l);
    let at = d.enrolment_at();

    // the fetch, with a nonce of this caller's choosing
    let nonce: [u8; 16] = [7; 16];
    let got = ask(
        at,
        &format!("GET /enrolment?nonce={} HTTP/1.1", hex(&nonce)),
        b"",
    );
    assert_eq!(200, status(&got), "the fetch is answered: {got}");
    let transport = field(&got, "transport").expect("a transport key");
    let shown = field(&got, "proof").expect("a proof");
    assert_eq!(
        "0",
        field(&got, "credentials").unwrap(),
        "nothing is in the run yet"
    );

    // **the proof is what makes the key the instance's and not an
    // intercepting answer's**: the client holds the token and recomputes it
    let key: [u8; 32] = unhex(&transport).try_into().expect("32 bytes");
    assert_eq!(
        hex(&rhtn_daemon::enrolment::proof(&TOKEN, &nonce, &key)),
        shown,
        "the proof is over this nonce and this key"
    );
    // and the key it published is the one it wrote beside its private half
    let beside = std::fs::read_to_string(l.dir.join("transport.pub")).expect("the public half");
    assert_eq!(beside.trim(), transport);

    // the operator signs a run over that key and hands it back
    let bob = test_identity("bob");
    let run = issue_run(&bob, &key, now(), 2);
    for d in &run {
        let put = ask(at, "PUT /enrolment/run HTTP/1.1", d);
        assert_eq!(200, status(&put), "the run is taken: {put}");
    }

    // which is the whole of what the instance was waiting for
    let serving = d
        .serving()
        .expect("the daemon serves once its run is in force");
    assert!(
        serving.contains("serving on 127.0.0.1:"),
        "it says where it serves: {serving}"
    );
    assert!(
        l.delegations.join(format!("run-{}", now())).exists()
            || std::fs::read_dir(&l.delegations).unwrap().count() >= 1,
        "the run it took is on disk, so a restart does not re-enrol"
    );

    // **the surface does not outlive the enrolment** (§8.2): what it was
    // for has happened, and a way in that answers nothing is still a way in
    let end = Instant::now() + Duration::from_secs(10);
    let mut closed = false;
    while Instant::now() < end {
        if TcpStream::connect(at).is_err() {
            closed = true;
            break;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    assert!(closed, "the enrolment surface at {at} is still accepting");

    d.stop();
    let _ = std::fs::remove_dir_all(&l.dir);
}

/// A fetch with no nonce, and a run nobody the instance knows signed: both
/// refused, and the second leaves nothing behind.
#[test]
fn a_nonceless_fetch_and_a_run_of_the_wrong_signer_are_refused() {
    let l = layout("refusals", &TOKEN);
    let d = spawn(&l);
    let at = d.enrolment_at();

    assert_eq!(
        400,
        status(&ask(at, "GET /enrolment HTTP/1.1", b"")),
        "a fetch carries a nonce"
    );
    assert_eq!(
        400,
        status(&ask(at, "GET /enrolment?nonce=00 HTTP/1.1", b"")),
        "a nonce is sixteen bytes"
    );
    assert_eq!(
        404,
        status(&ask(at, "GET /anything-else HTTP/1.1", b"")),
        "the surface answers two things only"
    );

    // a run alice signed over the instance's key: alice is not the operator
    let got = ask(
        at,
        &format!("GET /enrolment?nonce={} HTTP/1.1", hex(&[1u8; 16])),
        b"",
    );
    let key: [u8; 32] = unhex(&field(&got, "transport").unwrap())
        .try_into()
        .unwrap();
    let alice = test_identity("alice");
    let theirs = issue_run(&alice, &key, now(), 1);
    let put = ask(at, "PUT /enrolment/run HTTP/1.1", &theirs[0]);
    assert_eq!(400, status(&put), "a run of another signer: {put}");
    assert!(
        put.contains("not taken into the run"),
        "and it says why: {put}"
    );
    assert!(
        !Path::new(&l.delegations).exists()
            || std::fs::read_dir(&l.delegations).unwrap().count() == 0,
        "nothing a signature refused is on disk"
    );
    assert_eq!(
        400,
        status(&ask(at, "PUT /enrolment/run HTTP/1.1", b"")),
        "an empty run is not a run"
    );

    d.stop();
    let _ = std::fs::remove_dir_all(&l.dir);
}
