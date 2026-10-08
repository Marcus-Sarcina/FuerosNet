//! **An instance enrolled over its own out-of-band surface**
//! (`rhtn_daemon::administration`; `infra-client-requirements.md` §7, §8.2;
//! design §23.3): the daemon is a process here, started from a
//! configuration as an operator would start it, and nothing is put in its
//! delegations directory by hand.
//!
//! The exchange under test is the one a provisioning page performs: fetch
//! the public half of the key the instance minted, check it against the
//! token the configuration carried, sign a run over it, hand the run back.

use rhtn_crypto::delegation::issue_run;
use rhtn_crypto::identity::testkit::test_identity;
use rhtn_node::resolution::{NetworkPoint, anchor_entry, endpoint_record};
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

/// The first sequence number of a series, which is what a first record
/// carries.
fn seqno(counter: u32) -> rhtn_archive::tx::Seqno {
    rhtn_archive::tx::Seqno { series: 0, counter }
}

/// An instance for bob, with its operator's surface on a port the
/// operating system picks.
///
/// **`quic_port` is named rather than left to the system** because
/// `Service::start` compares the address it serves on against the one the
/// endpoint record names and says so when they differ, so a test that
/// delivers a record has to know the port in advance. One per test: these
/// run in one binary, and two daemons cannot hold one port.
fn layout(tag: &str, token: &[u8; 32], quic_port: u16) -> Layout {
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
             listen = \"127.0.0.1:{}\"\n\
             endpoint-record = \"{}\"\nanchor-entry = \"{}\"\n\
             queue = \"{}\"\nprekeys = \"{}\"\ntopology = \"{}\"\narchive = \"{}\"\n\
             heartbeat = 30\ningestion = \"unverified-gossip\"\n\
             [allowance]\nrequests = 120\nseconds = 60\n\
             [administration]\nlisten = \"127.0.0.1:0\"\ntoken = \"{}\"\n",
            dir.join("operator").display(),
            dir.join("transport.key").display(),
            delegations.display(),
            quic_port,
            dir.join("endpoint.record").display(),
            dir.join("anchor.entry").display(),
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

    /// Where the operator's surface bound, from the line announcing it.
    fn surface_at(&self) -> SocketAddr {
        let line = self
            .wait_line("surface on", 20)
            .unwrap_or_else(|| panic!("no surface line; the daemon said {:?}", self.lines()));
        let after = line.split("surface on ").nth(1).expect("an address");
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

    /// Whether the process has gone, and with what.
    fn gone(&mut self) -> Option<std::process::ExitStatus> {
        self.child.try_wait().ok().flatten()
    }

    fn stop(mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

impl Drop for Daemon {
    /// **A test that fails must not leave a node holding a port.** `stop`
    /// is on the success path only, so a panicking assertion used to leak
    /// the child — and the next run of this file met `AddrInUse` from the
    /// previous run's daemon rather than from anything it did itself.
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

/// One request over the operator's surface, and the whole response.
fn ask(at: SocketAddr, head: &str, body: &[u8]) -> String {
    let mut s = TcpStream::connect(at).expect("the surface accepts");
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

/// What came after the headers.
fn body(response: &str) -> &str {
    match response.find("\r\n\r\n") {
        Some(i) => &response[i + 4..],
        None => response,
    }
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
    const QUIC: u16 = 37447;
    let l = layout("happy", &TOKEN, QUIC);
    let mut d = spawn(&l);
    let at = d.surface_at();

    // **before it serves, the page says why** (§8.3): an operator who opens
    // the surface on a fresh instance should read what it is waiting for
    // rather than an empty table
    let waiting = ask(at, "GET / HTTP/1.1", b"");
    assert_eq!(200, status(&waiting), "the page is served: {waiting}");
    assert!(
        waiting.contains("text/html"),
        "and it is a page, not a line: {waiting}"
    );
    assert!(
        waiting.contains("Not serving yet") && waiting.contains("WANTED"),
        "it says what it wants: {}",
        body(&waiting)
    );

    // the fetch, with a nonce of this caller's choosing
    let nonce: [u8; 16] = [7; 16];
    let got = ask(
        at,
        &format!("GET /node?nonce={} HTTP/1.1", hex(&nonce)),
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
    // **one fetch says what it still wants**, so a page drives the exchange
    // from the answer rather than from steps written down elsewhere
    assert_eq!("wanted", field(&got, "endpoint-record").unwrap());
    assert_eq!("wanted", field(&got, "anchor-entry").unwrap());

    // **the proof is what makes the key the instance's and not an
    // intercepting answer's**: the client holds the token and recomputes it
    let key: [u8; 32] = unhex(&transport).try_into().expect("32 bytes");
    assert_eq!(
        hex(&rhtn_daemon::administration::proof(&TOKEN, &nonce, &key)),
        shown,
        "the proof is over this nonce and this key"
    );
    // and the key it published is the one it wrote beside its private half
    let beside = std::fs::read_to_string(l.dir.join("transport.pub")).expect("the public half");
    assert_eq!(beside.trim(), transport);

    // **the two records go first and the run last** (§4.4): the records are
    // what `Service::start` reads after the wait, and taking the run is
    // what ends the wait — so a run handed over first would have the node
    // look for a file the page had not pushed yet
    let bob = test_identity("bob");

    // **the two records an instance cannot sign for itself** (§4.4), over
    // the same surface and needing no token either: each carries the
    // operator's signature, which is the whole of what admits it
    let point = NetworkPoint::new([127, 0, 0, 1], Some(QUIC.into()));
    let record = endpoint_record(&bob, std::slice::from_ref(&point), seqno(1));
    let entry = anchor_entry(&bob, std::slice::from_ref(&point), 3, seqno(1));
    let put = ask(at, "PUT /node/endpoint-record HTTP/1.1", &record);
    assert_eq!(200, status(&put), "the endpoint record is taken: {put}");
    assert!(
        put.contains("endpoints 1"),
        "and it says what it took: {put}"
    );
    let put = ask(at, "PUT /node/anchor-entry HTTP/1.1", &entry);
    assert_eq!(200, status(&put), "the anchor entry is taken: {put}");
    assert!(put.contains("subtree 3"), "and it says what it took: {put}");

    // the next fetch says it holds them, which is what a page watches
    let again = ask(
        at,
        &format!("GET /node?nonce={} HTTP/1.1", hex(&[9u8; 16])),
        b"",
    );
    assert_eq!("held", field(&again, "endpoint-record").unwrap());
    assert_eq!("held", field(&again, "anchor-entry").unwrap());
    assert_eq!(
        "enrolling",
        field(&again, "phase").unwrap(),
        "still waiting: nothing has handed it a run yet"
    );

    // and now the run, which is what ends the wait
    let run = issue_run(&bob, &key, now(), 2);
    for d in &run {
        let put = ask(at, "PUT /node/run HTTP/1.1", d);
        assert_eq!(200, status(&put), "the run is taken: {put}");
    }

    // which is the whole of what the instance was waiting for
    let serving = match d.serving() {
        Some(l) => l,
        None => {
            // the reader thread may not hold the last lines yet, and a
            // daemon that refused its configuration says why on the way out
            std::thread::sleep(Duration::from_millis(500));
            panic!(
                "it did not serve; exit {:?}; the daemon said {:?}",
                d.gone(),
                d.lines()
            )
        }
    };
    assert!(
        serving.contains("serving on 127.0.0.1:"),
        "it says where it serves: {serving}"
    );
    assert!(
        std::fs::read_dir(&l.delegations).unwrap().count() >= 1,
        "the run it took is on disk, so a restart does not re-enrol"
    );
    // **and it came up holding both records**, which it could only do
    // because `start` reads them after the loop that waits for the run: a
    // record delivered during enrolment needs no restart to take effect
    assert!(
        !d.lines()
            .iter()
            .any(|l| l.contains("is not one the endpoint record names")),
        "the record it took names the address it serves on: {:?}",
        d.lines()
    );
    assert_eq!(
        std::fs::read(l.dir.join("endpoint.record")).unwrap(),
        record,
        "the endpoint record on disk is the one that was signed"
    );
    assert_eq!(
        std::fs::read(l.dir.join("anchor.entry")).unwrap(),
        entry,
        "and so is the anchor entry"
    );

    // **and the surface outlives the enrolment** [author, 2026-10-08],
    // because what it is for does not happen once: §4.4 has an endpoint
    // record re-signed whenever the address set changes, §7 has the run
    // renewed before it lapses, and an instance with no way to be handed a
    // new record "publishes nothing until" its operator is reachable
    let after = ask(
        at,
        &format!("GET /node?nonce={} HTTP/1.1", hex(&[3u8; 16])),
        b"",
    );
    assert_eq!(
        200,
        status(&after),
        "the surface is still answering: {after}"
    );
    assert_eq!(
        "serving",
        field(&after, "phase").unwrap(),
        "and says which phase it is in now"
    );
    assert_eq!(
        "2",
        field(&after, "credentials").unwrap(),
        "the two credentials of the run it was handed"
    );

    // **and once it serves, the page is the node's own state** (§8.3): what
    // it is, what it holds, who is attached, and §8's exposure disclosure
    let page = ask(at, "GET / HTTP/1.1", b"");
    assert_eq!(200, status(&page), "the page is served: {page}");
    assert!(page.contains("text/html"), "as html: {page}");
    let text = body(&page);
    for want in [
        "serving as",
        "a credential its operator's client signed",
        "subordinates",
        "an operator-signed record at seqno 1",
        "identities directly below",
    ] {
        assert!(text.contains(want), "the page states {want:?}: {text}");
    }
    // **OPS-011's prohibition, which is not "no controls"** [corrected,
    // 2026-10-08]: what the status interface may not provide is "arbitrary
    // frame composition, traffic replay, signature creation or manual
    // packet approval", and OPS-012 has management acts kept as acts. So
    // what is asserted is that nothing executes here — a page that ran
    // script could compose anything — and not that nothing may be pressed.
    assert!(
        !text.contains("<script") && !text.contains("javascript:"),
        "the page runs nothing: {text}"
    );

    // **the address moves**: the operator signs a record for the new one,
    // and the node publishes it without being restarted
    let moved = NetworkPoint::new([127, 0, 0, 1], Some(u64::from(QUIC) + 100));
    let next = endpoint_record(&bob, std::slice::from_ref(&moved), seqno(2));
    let put = ask(at, "PUT /node/endpoint-record HTTP/1.1", &next);
    assert_eq!(200, status(&put), "the re-signed record is taken: {put}");
    assert!(put.contains("seqno 2"), "and it says which: {put}");
    assert_eq!(
        std::fs::read(l.dir.join("endpoint.record")).unwrap(),
        next,
        "the file a restart would read is the new record"
    );

    // **a replay changes nothing**: the node's own supersession discipline
    // refuses a seqno no newer than the one it holds, and the file follows
    // the node rather than the caller
    let put = ask(at, "PUT /node/endpoint-record HTTP/1.1", &record);
    assert_eq!(409, status(&put), "the superseded record is refused: {put}");
    assert_eq!(
        std::fs::read(l.dir.join("endpoint.record")).unwrap(),
        next,
        "and the file still holds the newer one"
    );
    // while re-sending the one it holds is a no-op rather than a refusal,
    // so a client unsure whether its push landed may simply push again
    let put = ask(at, "PUT /node/endpoint-record HTTP/1.1", &next);
    assert_eq!(200, status(&put), "an identical re-send: {put}");
    assert!(put.contains("already held"), "and it says so: {put}");

    d.stop();
    let _ = std::fs::remove_dir_all(&l.dir);
}

/// A fetch with no nonce, and a run nobody the instance knows signed: both
/// refused, and the second leaves nothing behind.
#[test]
fn a_nonceless_fetch_and_a_run_of_the_wrong_signer_are_refused() {
    let l = layout("refusals", &TOKEN, 37448);
    let d = spawn(&l);
    let at = d.surface_at();

    assert_eq!(
        400,
        status(&ask(at, "GET /node HTTP/1.1", b"")),
        "a fetch carries a nonce"
    );
    assert_eq!(
        400,
        status(&ask(at, "GET /node?nonce=00 HTTP/1.1", b"")),
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
        &format!("GET /node?nonce={} HTTP/1.1", hex(&[1u8; 16])),
        b"",
    );
    let key: [u8; 32] = unhex(&field(&got, "transport").unwrap())
        .try_into()
        .unwrap();
    let alice = test_identity("alice");
    let theirs = issue_run(&alice, &key, now(), 1);
    let put = ask(at, "PUT /node/run HTTP/1.1", &theirs[0]);
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
        status(&ask(at, "PUT /node/run HTTP/1.1", b"")),
        "an empty run is not a run"
    );

    // **the two records are refused on the same terms**: about somebody
    // else, signed by somebody else, or not a record at all
    let point = NetworkPoint::new([127, 0, 0, 1], Some(37448));
    let theirs = endpoint_record(&alice, std::slice::from_ref(&point), seqno(1));
    let put = ask(at, "PUT /node/endpoint-record HTTP/1.1", &theirs);
    assert_eq!(
        400,
        status(&put),
        "an endpoint record of another node: {put}"
    );
    assert!(put.contains("another node"), "and it says so: {put}");
    let theirs = anchor_entry(&alice, std::slice::from_ref(&point), 3, seqno(1));
    assert_eq!(
        400,
        status(&ask(at, "PUT /node/anchor-entry HTTP/1.1", &theirs)),
        "an anchor entry of another subnet"
    );
    assert_eq!(
        400,
        status(&ask(
            at,
            "PUT /node/endpoint-record HTTP/1.1",
            b"not a record"
        )),
        "bytes that are not a record"
    );
    assert!(
        !l.dir.join("endpoint.record").exists() && !l.dir.join("anchor.entry").exists(),
        "nothing a signature refused is on disk"
    );

    d.stop();
    let _ = std::fs::remove_dir_all(&l.dir);
}

/// Enrol an instance and leave it serving: the exchange the happy path
/// walks through, as a helper for the tests that need a node rather than
/// the enrolment.
fn enrolled(tag: &str, quic: u16) -> (Layout, Daemon, SocketAddr) {
    let l = layout(tag, &TOKEN, quic);
    let mut d = spawn(&l);
    let at = d.surface_at();
    let got = ask(
        at,
        &format!("GET /node?nonce={} HTTP/1.1", hex(&[2u8; 16])),
        b"",
    );
    let key: [u8; 32] = unhex(&field(&got, "transport").expect("a key"))
        .try_into()
        .expect("32 bytes");
    let bob = test_identity("bob");
    let point = NetworkPoint::new([127, 0, 0, 1], Some(u64::from(quic)));
    let record = endpoint_record(&bob, std::slice::from_ref(&point), seqno(1));
    let entry = anchor_entry(&bob, std::slice::from_ref(&point), 3, seqno(1));
    assert_eq!(
        200,
        status(&ask(at, "PUT /node/endpoint-record HTTP/1.1", &record))
    );
    assert_eq!(
        200,
        status(&ask(at, "PUT /node/anchor-entry HTTP/1.1", &entry))
    );
    for c in issue_run(&bob, &key, now(), 1) {
        assert_eq!(200, status(&ask(at, "PUT /node/run HTTP/1.1", &c)));
    }
    let said = d.lines();
    assert!(
        d.serving().is_some(),
        "it serves once enrolled; the daemon said {said:?}"
    );
    (l, d, at)
}

/// **The management acts** (OPS-012, and §8.1 as it actually reads): a
/// standing policy changed from the page, and a stop that takes the path a
/// signal takes. Neither composes a frame, replays traffic, makes a
/// signature or approves a packet, which is what OPS-011 forbids; and
/// neither carries a key, which is what lets a desktop holding no seed
/// administer [author, 2026-10-08].
#[test]
fn a_standing_policy_is_changed_from_the_page_and_a_stop_is_the_signal_s_path() {
    let (l, mut d, at) = enrolled("acts", 37449);

    // the page offers the act, and says where the policy stands
    let page = ask(at, "GET / HTTP/1.1", b"");
    assert!(body(&page).contains("acknowledges no"), "off to begin with");
    assert!(
        body(&page).contains("/node/acknowledge"),
        "and the act is offered: {}",
        body(&page)
    );

    let done = ask(at, "POST /node/acknowledge HTTP/1.1", b"acknowledge=on");
    assert_eq!(200, status(&done), "the act is taken: {done}");
    let page = ask(at, "GET / HTTP/1.1", b"");
    assert!(
        body(&page).contains("acknowledges yes"),
        "and the page shows it in force: {}",
        body(&page)
    );
    // **the configuration on disk is untouched**: it is a file its operator
    // owns, and a daemon that rewrote it would be choosing what their next
    // start says
    let conf = std::fs::read_to_string(&l.config).unwrap();
    assert!(
        !conf.contains("acknowledge"),
        "the configuration is the operator's: {conf}"
    );
    assert_eq!(
        400,
        status(&ask(
            at,
            "POST /node/acknowledge HTTP/1.1",
            b"acknowledge=maybe"
        )),
        "the policy is on or off"
    );

    // a reload with no `resources` named is a refusal that says so
    let r = ask(at, "POST /node/reload HTTP/1.1", b"");
    assert_eq!(409, status(&r), "nothing to re-read: {r}");

    // **stop takes the signal's path**, so the process ends of its own
    // accord and writes back what it holds
    assert_eq!(200, status(&ask(at, "POST /node/stop HTTP/1.1", b"")));
    let end = Instant::now() + Duration::from_secs(20);
    let mut gone = None;
    while Instant::now() < end {
        if let Some(s) = d.gone() {
            gone = Some(s);
            break;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    let gone = gone.unwrap_or_else(|| panic!("it did not stop; the daemon said {:?}", d.lines()));
    assert!(gone.success(), "and it stopped cleanly: {gone:?}");

    let _ = std::fs::remove_dir_all(&l.dir);
}
