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
    /// **What stands in for the image's ssh client**
    /// (`rhtn_daemon::runner::client`): a recorder, so the whole install
    /// path can be exercised without a machine to install on. Absent
    /// leaves the daemon reaching for the real one, which is what it
    /// does in the field.
    ssh_client: Option<PathBuf>,
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
    layout_on(tag, token, quic_port, "127.0.0.1:0")
}

/// The same with the surface bound where the caller says, so a test can
/// reach it from an address that is not the loopback — which is where an
/// act must prove itself (`Surface::authorised`).
fn layout_on(tag: &str, token: &[u8; 32], quic_port: u16, surface: &str) -> Layout {
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
    // an empty hosting file hosts nothing, which §9 makes a conforming
    // state; a test that wants a resource writes over it before the
    // daemon starts
    std::fs::write(dir.join("hosting"), "").unwrap();
    std::fs::write(
        &config,
        format!(
            "operator = \"{}\"\n\
             transport-key = \"{}\"\n\
             delegations = \"{}\"\n\
             listen = \"127.0.0.1:{}\"\n\
             endpoint-record = \"{}\"\nanchor-entry = \"{}\"\n\
             queue = \"{}\"\nprekeys = \"{}\"\ntopology = \"{}\"\narchive = \"{}\"\n\
             resources = \"{}\"\ngrants = \"{}\"\n\
             heartbeat = 30\ningestion = \"unverified-gossip\"\n\
             [allowance]\nrequests = 120\nseconds = 60\n\
             [administration]\nlisten = \"{}\"\ntoken = \"{}\"\ntoken-file = \"{}\"\n\
             host-shell = \"127.0.0.1:22\"\n",
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
            dir.join("hosting").display(),
            dir.join("grants").display(),
            surface,
            hex(token),
            dir.join("administration.token").display(),
        ),
    )
    .unwrap();
    Layout {
        dir,
        config,
        peers,
        delegations,
        ssh_client: None,
    }
}

struct Daemon {
    child: Child,
    stderr: Arc<Mutex<Vec<String>>>,
    stdout: BufReader<std::process::ChildStdout>,
}

fn spawn(l: &Layout) -> Daemon {
    let mut command = Command::new(env!("CARGO_BIN_EXE_rhtnd"));
    command.arg(&l.config).arg(&l.peers);
    if let Some(client) = &l.ssh_client {
        command.env("RHTN_SSH_CLIENT", client);
    }
    let mut child = command
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
    // **and when that run ends, which is where a renewal opens**
    // (`infra-client-requirements.md` §7): the last credential's
    // `not_after`, so an operator need not guess and the run stays
    // contiguous (`wire-format.md` §8.2)
    let ends: u64 = field(&after, "run-ends").unwrap().parse().unwrap();
    assert!(
        ends > now(),
        "the run ends in the future, at {ends}, now being {}",
        now()
    );
    // two 48-hour credentials from `now()`, so the end is two windows out
    assert!(
        ends >= now() + 2 * 172_800 - 60 && ends <= now() + 2 * 172_800 + 60,
        "two windows of 48 hours: {ends} against {}",
        now() + 2 * 172_800
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
    enrolled_with(tag, quic, |_| {})
}

/// The same, with a chance to write files the daemon reads at start and
/// to shape the layout itself.
fn enrolled_with(
    tag: &str,
    quic: u16,
    before: impl FnOnce(&mut Layout),
) -> (Layout, Daemon, SocketAddr) {
    let mut l = layout(tag, &TOKEN, quic);
    before(&mut l);
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

/// **A node comes up with no anchor entry written** [2026-10-25]. The
/// first instance launched from a phone was enrolled with none, took its
/// run, and then refused every start with "anchor.entry: No such file or
/// directory" until the read was changed: the path the configuration names
/// says where an entry will go once the operator signs one, not that one is
/// there. Enrolment has signed the entry unconditionally since the same day
/// (§4.4), so this is now the case of a restart before the entry arrived,
/// and of an instance enrolled before the rule.
#[test]
fn a_node_enrolled_with_no_subtree_claimed_serves_and_serves_again_after_a_restart() {
    let l = layout("no-anchor", &TOKEN, 37452);
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
    let point = NetworkPoint::new([127, 0, 0, 1], Some(37452));
    let record = endpoint_record(&bob, std::slice::from_ref(&point), seqno(1));
    assert_eq!(
        200,
        status(&ask(at, "PUT /node/endpoint-record HTTP/1.1", &record))
    );
    // no anchor entry: nothing claimed
    for c in issue_run(&bob, &key, now(), 1) {
        assert_eq!(200, status(&ask(at, "PUT /node/run HTTP/1.1", &c)));
    }
    assert!(
        d.serving().is_some(),
        "it serves with no anchor entry written; the daemon said {:?}",
        d.lines()
    );
    assert!(
        !l.dir.join("anchor.entry").exists(),
        "the test wrote no entry, so the daemon must not have needed one"
    );
    d.stop();

    // the restart a container's policy makes, on the same state
    let mut again = spawn(&l);
    assert!(
        again.serving().is_some(),
        "it serves again on the same state; the daemon said {:?}",
        again.lines()
    );
    again.stop();
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
    // **the control bounces the act to the client** [author, 2026-11-02]:
    // the page carries no form, because a form cannot compute the MAC an
    // act off the loopback proves itself with (`rhtn_daemon::acts`)
    assert!(
        body(&page).contains("rhtn-act:acknowledge"),
        "and the act is offered as a control: {}",
        body(&page)
    );
    assert!(
        !body(&page).contains("<form"),
        "and the page carries no form at all: {}",
        body(&page)
    );
    // and the label is what the act does rather than what is true
    assert!(
        body(&page).contains("Countersign every adoption"),
        "{}",
        body(&page)
    );

    // the act, through the one route a client needs
    let done = ask(at, "POST /node/act/acknowledge HTTP/1.1", b"acknowledge=on");
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

    // a reload re-reads the hosting file, which here names nothing to
    // host — §9 makes that a conforming state rather than a defect
    let r = ask(at, "POST /node/reload HTTP/1.1", b"");
    assert_eq!(200, status(&r), "re-read: {r}");
    assert!(r.contains("0 package(s) bound"), "{r}");

    // and a grant act with no resource bound has nothing to act on
    let r = ask(at, "POST /node/grant HTTP/1.1", b"resource=09&roles=reader");
    assert_eq!(400, status(&r), "a resource is 64 hex digits: {r}");

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

/// **A grant is made, narrowed, withdrawn and taken from the page**, which
/// is OPS-012's "explicit management acts" over an operator's predicates
/// in `infra-client-requirements.md` §10.3's vocabulary.
///
/// And it persists: a grant is a table update [ruled, author, 2026-10-09],
/// so unlike the node's own configuration it is written back. An act that
/// did not survive a restart would not be one.
#[test]
fn a_grant_is_made_and_withdrawn_from_the_page_and_is_written_back() {
    const RES: &str = "0909090909090909090909090909090909090909090909090909090909090909";
    let owner = hex(&test_identity("bob").public.keyhash);
    let (l, d, at) = enrolled_with("grants", 37450, |l| {
        std::fs::write(l.dir.join("echo.wasm"), rhtn_sim::packages::echo()).unwrap();
        std::fs::write(
            l.dir.join("shop.manifest"),
            "roles = [\"reader\", \"writer\"]\n\
             imports = [\"rhtn/1:request\", \"rhtn/1:response\"]\n\
             component = \"echo.wasm\"\n\
             \n\
             [[template]]\n\
             name = \"org-read\"\n\
             label = \"Everyone in my org may read\"\n\
             roles = [\"reader\"]\n\
             \n\
             [[template.where]]\n\
             of = \"grandclients\"\n",
        )
        .unwrap();
        std::fs::write(
            l.dir.join("hosting"),
            format!(
                "[[host]]\nresource = \"{RES}\"\nowner = \"{}\"\n\
                 authority = \"shop.internal\"\nmanifest = \"{}\"\n",
                hex(&test_identity("bob").public.keyhash),
                l.dir.join("shop.manifest").display()
            ),
        )
        .unwrap();
    });
    let tab = format!("GET /?resource={RES} HTTP/1.1");

    // **nothing is granted, and the page says so in both places** (§7):
    // the role list marks what nobody holds, and the grant section is
    // empty rather than absent
    let page = ask(at, &tab, b"");
    assert!(page.contains("nothing is granted here"), "{page}");
    assert!(
        page.contains("reader (nobody)") && page.contains("writer (nobody)"),
        "§7: a role bound to nobody is visible, not a silent default: {page}"
    );
    assert!(
        page.contains(&format!("rhtn-act:grant?resource={RES}")),
        "the page carries the control, with the resource as a hint: {page}"
    );
    assert!(
        page.contains("rhtn-act:grant-template") && page.contains("org-read"),
        "§10.4: the package's own grant is offered: {page}"
    );

    // a grant over everyone, which is the absence of a clause
    let made = ask(
        at,
        "POST /node/act/grant HTTP/1.1",
        format!("resource={RES}&roles=reader&of=everyone").as_bytes(),
    );
    assert_eq!(200, status(&made), "{made}");
    let page = ask(at, &tab, b"");
    assert!(
        page.contains("reader to every member of my trust horizon"),
        "it reads back in the operator's own vocabulary: {page}"
    );
    assert!(
        !page.contains("reader (nobody)"),
        "and the role is no longer bound to nobody: {page}"
    );

    // **written to the node's own table**, so a restart takes it
    let kept = std::fs::read_to_string(l.dir.join("grants").join(format!("{RES}.toml")))
        .expect("the node wrote its grants");
    assert!(
        kept.contains("[[grant]]") && kept.contains("\"reader\""),
        "{kept}"
    );

    // narrowing is §7.1's conjunction
    assert_eq!(
        200,
        status(&ask(
            at,
            "POST /node/grant/narrow HTTP/1.1",
            format!("resource={RES}&at=0&of=most-trusted&with=10").as_bytes()
        ))
    );
    let page = ask(at, &tab, b"");
    assert!(
        page.contains("reader to my 10 most trusted"),
        "the clause narrowed the grant it was given: {page}"
    );

    // a role the package never declared is refused, and nothing moves
    let no = ask(
        at,
        "POST /node/grant HTTP/1.1",
        format!("resource={RES}&roles=admin&of=clients").as_bytes(),
    );
    assert_eq!(400, status(&no), "{no}");
    assert!(no.contains("not a role this package declared"), "{no}");

    // withdrawing takes the grant and its rows
    assert_eq!(
        200,
        status(&ask(
            at,
            "POST /node/grant/drop HTTP/1.1",
            format!("resource={RES}&at=0").as_bytes()
        ))
    );
    assert!(
        ask(at, &tab, b"").contains("nothing is granted here"),
        "the grant is gone"
    );

    // and the package's own grant is the operator's click, not the author's
    assert_eq!(
        200,
        status(&ask(
            at,
            "POST /node/grant/template HTTP/1.1",
            format!("resource={RES}&name=org-read").as_bytes()
        ))
    );
    assert!(
        ask(at, &tab, b"").contains("reader to my clients and grand-clients"),
        "the template's own grant, in the same words it was offered in"
    );

    // a resource nothing hosts, and a clause that is not one
    let bad = ask(
        at,
        "POST /node/grant HTTP/1.1",
        format!("resource={owner}&roles=reader&of=clients").as_bytes(),
    );
    assert_eq!(404, status(&bad), "{bad}");
    let bad = ask(
        at,
        "POST /node/grant HTTP/1.1",
        format!("resource={RES}&roles=reader&of=everybody").as_bytes(),
    );
    assert_eq!(400, status(&bad), "{bad}");
    assert!(bad.contains("is not a clause"), "{bad}");

    d.stop();
}

/// **A run is renewed from where it ends, not from now**
/// (`infra-client-requirements.md` §7; `wire-format.md` §8.2's contiguous
/// run). The instance says where that is, so the operator's client need
/// not guess — and a run signed from the clock again overlaps what is
/// held and is refused.
// acceptance: DMN-28
#[tokio::test]
async fn dmn_28_a_renewal_opens_where_the_run_ends_and_an_overlap_is_refused() {
    let l = layout("renew", &TOKEN, 37449);
    let d = spawn(&l);
    let at = d.surface_at();
    let bob = test_identity("bob");

    // enrolled with two credentials, as an instance is
    let got = ask(
        at,
        &format!("GET /node?nonce={} HTTP/1.1", hex(&[1u8; 16])),
        b"",
    );
    let key: [u8; 32] = unhex(&field(&got, "transport").unwrap())
        .as_slice()
        .try_into()
        .expect("a transport key");
    for c in issue_run(&bob, &key, now(), 2) {
        assert_eq!(200, status(&ask(at, "PUT /node/run HTTP/1.1", &c)));
    }
    let after = ask(
        at,
        &format!("GET /node?nonce={} HTTP/1.1", hex(&[2u8; 16])),
        b"",
    );
    let ends: u64 = field(&after, "run-ends").unwrap().parse().unwrap();
    assert_eq!("2", field(&after, "credentials").unwrap());

    // **the renewal opens exactly there**, and is taken
    for c in issue_run(&bob, &key, ends, 3) {
        let put = ask(at, "PUT /node/run HTTP/1.1", &c);
        assert_eq!(200, status(&put), "the renewal is taken: {put}");
    }
    let third = ask(
        at,
        &format!("GET /node?nonce={} HTTP/1.1", hex(&[3u8; 16])),
        b"",
    );
    assert_eq!("5", field(&third, "credentials").unwrap(), "two and three");
    let ends_now: u64 = field(&third, "run-ends").unwrap().parse().unwrap();
    assert_eq!(
        ends_now,
        ends + 3 * 172_800,
        "the run ends three 48-hour windows later"
    );

    // and a run that straddles hours already held is the enrolling-twice
    // case `Credential::add` refuses. **The identical window re-sent is
    // not that case** and is taken once, silently, as it always was — so
    // this one opens mid-window, which no held credential matches
    let overlap = issue_run(&bob, &key, now() + 86_400, 1);
    let put = ask(at, "PUT /node/run HTTP/1.1", &overlap[0]);
    assert_eq!(400, status(&put), "an overlapping credential: {put}");
    assert!(body(&put).contains("not taken into the run"), "{put}");
}

/// **The directive plane from the operator's surface**
/// (`rhtn_node::federation`; `resource-design/directive-surface.md`): a
/// hosted instance with a configured patron takes an envelope from that
/// patron at `PUT /node/resource/<hex>/envelope`, refuses one from a
/// stranger, and shows what it holds at `GET …/federation`. The resource
/// here is brokered, so nothing is delivered onward: the plane's bookkeeping
/// is what this test reads.
// acceptance: DMN-32
#[test]
fn an_envelope_is_delivered_from_the_surface_and_the_plane_is_readable() {
    use rhtn_node::federation::Envelope;
    const RES: &str = "0c0c0c0c0c0c0c0c0c0c0c0c0c0c0c0c0c0c0c0c0c0c0c0c0c0c0c0c0c0c0c0c";
    const PATRON: &str = "0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d";
    let (l, mut d, at) = enrolled_with("federation", 37453, |l| {
        let m = l.dir.join("front.manifest");
        std::fs::write(&m, "roles = [\"reader\"]\n").unwrap();
        std::fs::write(
            l.dir.join("hosting"),
            format!(
                "[[host]]\nresource = \"{RES}\"\nowner = \"{}\"\nauthority = \"club.internal\"\n\
                 manifest = \"{}\"\npatron = \"{PATRON}\"\n",
                hex(&test_identity("bob").public.keyhash),
                m.display()
            ),
        )
        .unwrap();
    });
    assert!(d.serving().is_some(), "{:?}", d.lines());

    let before = ask(
        at,
        &format!("GET /node/resource/{RES}/federation HTTP/1.1"),
        b"",
    );
    assert_eq!(200, status(&before), "{before}");
    assert!(
        before.contains(&format!("\"patron\":\"{PATRON}\"")),
        "{before}"
    );
    assert!(before.contains("\"held\":[]"), "{before}");

    let from_patron = Envelope::live([0x0d; 32], "policy-1", "rhtn/directive@1", 1, b"{\"x\":1}");
    let body = from_patron.to_json().to_string();
    let r = ask(
        at,
        &format!("PUT /node/resource/{RES}/envelope HTTP/1.1"),
        body.as_bytes(),
    );
    assert_eq!(200, status(&r), "{r}");
    assert!(r.contains("New"), "{r}");
    let again = ask(
        at,
        &format!("PUT /node/resource/{RES}/envelope HTTP/1.1"),
        body.as_bytes(),
    );
    assert!(again.contains("Duplicate"), "{again}");

    let stranger = Envelope::live([0x0e; 32], "policy-1", "rhtn/directive@1", 1, b"{\"x\":1}");
    let r = ask(
        at,
        &format!("PUT /node/resource/{RES}/envelope HTTP/1.1"),
        stranger.to_json().to_string().as_bytes(),
    );
    assert_eq!(
        409,
        status(&r),
        "a stranger is not a configured relationship: {r}"
    );

    let r = ask(
        at,
        &format!("PUT /node/resource/{RES}/envelope HTTP/1.1"),
        b"not json",
    );
    assert_eq!(400, status(&r), "{r}");

    let after = ask(
        at,
        &format!("GET /node/resource/{RES}/federation HTTP/1.1"),
        b"",
    );
    assert!(after.contains("\"record\":\"policy-1\""), "{after}");
    assert!(
        after.contains("\"waiting\":1"),
        "brokered, so it waits: {after}"
    );

    let r = ask(
        at,
        &format!("POST /node/resource/{RES}/federate HTTP/1.1"),
        b"",
    );
    assert_eq!(200, status(&r), "{r}");

    // the page carries the same, for the operator reading rather than
    // scripting
    let page = ask(at, &format!("GET /?resource={RES} HTTP/1.1"), b"");
    assert!(page.contains("<h2>Federation</h2>"), "{page}");
    assert!(page.contains("policy-1"), "{page}");

    // **and it says whether custody is kept at all**: this node has no
    // `[federation] store`, so every answer it gives a sending node is a
    // promise it cannot keep across a restart, and it says so in all
    // three places an operator looks
    assert!(
        after.contains("\"custody\":\"in memory\""),
        "the fetch says what it is keeping: {after}"
    );
    assert!(page.contains("IN MEMORY"), "and so does the page: {page}");
    assert!(
        d.lines()
            .iter()
            .any(|l| l.contains("no `[federation] store` is configured")),
        "and so did the start: {:?}",
        d.lines()
    );

    let r = ask(at, "GET /node/resource/zz/federation HTTP/1.1", b"");
    assert_eq!(400, status(&r), "{r}");
    d.stop();
    let _ = std::fs::remove_dir_all(&l.dir);
}

/// **The fetch states the subtree this instance counts, and the page says
/// a re-signed entry is owed where its entry claims another number**
/// (`infra-client-requirements.md` §4.4; [author, 2026-10-26]). The size
/// is the instance's own fact, so the instance states it and the client
/// signs it; here the entry delivered at enrolment claims a subtree of 3
/// while nothing is beneath this node, which is the same comparison a
/// grown subtree makes from the other side.
#[test]
fn the_fetch_says_the_subtree_it_counts_and_the_page_owes_a_re_signed_entry() {
    let (l, mut d, at) = enrolled_with("subtree", 37454, |_| {});
    assert!(d.serving().is_some(), "{:?}", d.lines());

    // the entry `enrolled_with` delivered claims 3 (its fixture's size)
    let got = ask(
        at,
        &format!("GET /node?nonce={} HTTP/1.1", hex(&[5u8; 16])),
        b"",
    );
    assert_eq!(
        "0",
        field(&got, "subtree").expect("the count is stated"),
        "nothing is beneath this node: {got}"
    );
    assert_eq!("3", field(&got, "anchor-claims").expect("what it claims"));
    assert_eq!("1", field(&got, "anchor-seqno").expect("at which counter"));
    let points = field(&got, "anchor-points").expect("the points it names");
    assert!(!points.is_empty(), "so a re-signing moves the size alone");

    let page = ask(at, "GET / HTTP/1.1", b"");
    assert!(
        page.contains("re-signed anchor entry"),
        "the page owes one: {page}"
    );
    assert!(
        page.contains("claiming a subtree of 3") && page.contains("0 beneath this node now"),
        "and says both numbers: {page}"
    );

    // re-signed at the next counter over the same points, and the page
    // stops asking
    let bob = test_identity("bob");
    let point = NetworkPoint::new([127, 0, 0, 1], Some(37454));
    let entry = anchor_entry(&bob, std::slice::from_ref(&point), 0, seqno(2));
    assert_eq!(
        200,
        status(&ask(at, "PUT /node/anchor-entry HTTP/1.1", &entry))
    );
    let page = ask(at, "GET / HTTP/1.1", b"");
    assert!(
        !page.contains("re-signed anchor entry"),
        "nothing is owed once it claims what the node counts: {page}"
    );
    d.stop();
    let _ = std::fs::remove_dir_all(&l.dir);
}

/// **An enrolled instance whose surface is bound on every interface**,
/// which is what a rented one binds: enrolled over the loopback, where
/// arriving is the authorisation, and then reachable from this machine's
/// own address, where an act must prove itself.
fn enrolled_on(tag: &str, quic: u16, token: &[u8; 32]) -> (Layout, Daemon, SocketAddr) {
    let l = layout_on(tag, token, quic, "0.0.0.0:0");
    let mut d = spawn(&l);
    let port = d.surface_at().port();
    let at: SocketAddr = (std::net::Ipv4Addr::LOCALHOST, port).into();
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
    assert!(d.serving().is_some(), "it serves once enrolled");
    (l, d, at)
}

/// This machine's own outbound address, for reaching a surface from
/// somewhere that is not the loopback.  **Nothing is sent**: a connected
/// UDP socket only tells the kernel to pick a route, and the test says so
/// and skips where there is none.
fn outbound_address() -> Option<std::net::IpAddr> {
    let s = std::net::UdpSocket::bind("0.0.0.0:0").ok()?;
    s.connect("192.0.2.1:9").ok()?;
    let ip = s.local_addr().ok()?.ip();
    (!ip.is_loopback() && !ip.is_unspecified()).then_some(ip)
}

/// A nonce and the proof an act carries, as a query.
fn proved(token: &[u8; 32], method: &str, path: &str, body: &[u8], nonce: &[u8; 16]) -> String {
    let signed = format!("{path}?nonce={}", hex(nonce));
    let p = rhtn_crypto::enrolment::act_proof(token, nonce, method, &signed, body);
    format!("{signed}&proof={}", hex(&p))
}

// acceptance: DMN-36
#[test]
fn an_act_that_did_not_arrive_on_the_loopback_proves_itself_under_the_token() {
    let Some(ip) = outbound_address() else {
        eprintln!("no outbound address on this machine; the off-loopback half is skipped");
        return;
    };
    let token = [0x5au8; 32];
    let (_l, d, at) = enrolled_on("proof", 7489, &token);
    let away: SocketAddr = (ip, at.port()).into();

    // **on the loopback, arriving is the authorisation**, which is the
    // rule §8.2 rests on and holds exactly there
    let on_loopback = ask(at, "POST /node/acknowledge HTTP/1.1", b"acknowledge=on");
    assert!(on_loopback.starts_with("HTTP/1.1 200"), "{on_loopback}");

    // **from anywhere else it is not**: this is the surface a rented
    // instance binds, where reaching the port is evidence of nothing
    let bare = ask(away, "POST /node/acknowledge HTTP/1.1", b"acknowledge=off");
    assert!(bare.starts_with("HTTP/1.1 401"), "{bare}");
    assert!(body(&bare).contains("nonce and proof"), "{bare}");

    // with the proof it is taken
    let nonce = [1u8; 16];
    let target = proved(
        &token,
        "POST",
        "/node/acknowledge",
        b"acknowledge=off",
        &nonce,
    );
    let took = ask(away, &format!("POST {target} HTTP/1.1"), b"acknowledge=off");
    assert!(took.starts_with("HTTP/1.1 200"), "{took}");

    // **and a proof is worth the one act**: the same nonce again is
    // refused, and so is a proof made for another path or another body
    let again = ask(away, &format!("POST {target} HTTP/1.1"), b"acknowledge=off");
    assert!(again.starts_with("HTTP/1.1 401"), "{again}");
    assert!(body(&again).contains("has been used"), "{again}");

    let elsewhere = proved(
        &token,
        "POST",
        "/node/acknowledge",
        b"acknowledge=on",
        &[2u8; 16],
    );
    let wrong_body = ask(
        away,
        &format!("POST {elsewhere} HTTP/1.1"),
        b"acknowledge=off",
    );
    assert!(wrong_body.starts_with("HTTP/1.1 401"), "{wrong_body}");

    let other_token = proved(
        &[0u8; 32],
        "POST",
        "/node/acknowledge",
        b"acknowledge=on",
        &[3u8; 16],
    );
    let wrong_token = ask(
        away,
        &format!("POST {other_token} HTTP/1.1"),
        b"acknowledge=on",
    );
    assert!(wrong_token.starts_with("HTTP/1.1 401"), "{wrong_token}");

    // the fetch stays open: it is the bootstrap, and what it discloses is
    // the public half and a proof over it
    let fetch = ask(
        away,
        &format!("GET /node?nonce={} HTTP/1.1", hex(&[4u8; 16])),
        b"",
    );
    assert!(fetch.starts_with("HTTP/1.1 200"), "{fetch}");

    d.stop();
}

// acceptance: DMN-36
#[test]
fn the_token_is_replaced_at_enrolment_and_the_replacement_survives_a_restart() {
    let Some(ip) = outbound_address() else {
        eprintln!("no outbound address on this machine; this test needs one");
        return;
    };
    let old = [0x11u8; 32];
    let fresh = [0x22u8; 32];
    let (l, d, at) = enrolled_on("rotate", 7491, &old);
    let away: SocketAddr = (ip, at.port()).into();

    // **the replacement is set under the old token**, so a caller who
    // cannot prove the old one cannot set a new one
    let body_text = format!("token={}", hex(&fresh));
    let unproved = ask(away, "POST /node/token HTTP/1.1", body_text.as_bytes());
    assert!(unproved.starts_with("HTTP/1.1 401"), "{unproved}");

    let target = proved(
        &old,
        "POST",
        "/node/token",
        body_text.as_bytes(),
        &[7u8; 16],
    );
    let rotated = ask(
        away,
        &format!("POST {target} HTTP/1.1"),
        body_text.as_bytes(),
    );
    assert!(rotated.starts_with("HTTP/1.1 200"), "{rotated}");

    // kept where the next start will find it, and not readable by anyone
    // else on the machine
    let kept = l.dir.join("administration.token");
    assert_eq!(
        std::fs::read_to_string(&kept).unwrap().trim(),
        hex(&fresh),
        "the replacement is written"
    );
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = std::fs::metadata(&kept).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o600, "the token in force is 0600");
    }

    // **the old token is spent**: an act proved under it is refused, and
    // one proved under the replacement is taken
    let stale = proved(
        &old,
        "POST",
        "/node/acknowledge",
        b"acknowledge=on",
        &[8u8; 16],
    );
    let refused = ask(away, &format!("POST {stale} HTTP/1.1"), b"acknowledge=on");
    assert!(refused.starts_with("HTTP/1.1 401"), "{refused}");
    let now = proved(
        &fresh,
        "POST",
        "/node/acknowledge",
        b"acknowledge=on",
        &[9u8; 16],
    );
    let took = ask(away, &format!("POST {now} HTTP/1.1"), b"acknowledge=on");
    assert!(took.starts_with("HTTP/1.1 200"), "{took}");

    // **and it survives a restart**, which is why it is refused where
    // there is nowhere to keep it
    d.stop();
    let mut again = spawn(&l);
    let away: SocketAddr = (ip, again.surface_at().port()).into();
    // the restarted instance reads its run before it serves, and an act
    // waits on that as any caller would
    assert!(
        again.serving().is_some(),
        "it serves again from its own files"
    );
    let after = proved(
        &fresh,
        "POST",
        "/node/acknowledge",
        b"acknowledge=off",
        &[10u8; 16],
    );
    let still = ask(away, &format!("POST {after} HTTP/1.1"), b"acknowledge=off");
    assert!(still.starts_with("HTTP/1.1 200"), "{still}");
    let stale = proved(
        &old,
        "POST",
        "/node/acknowledge",
        b"acknowledge=on",
        &[11u8; 16],
    );
    let refused = ask(away, &format!("POST {stale} HTTP/1.1"), b"acknowledge=on");
    assert!(refused.starts_with("HTTP/1.1 401"), "{refused}");
    again.stop();
}

/// **A recorder in the ssh client's place** (`rhtn_daemon::runner::client`).
///
/// It writes the request it was given — the last argument, which is what
/// `SSH_ORIGINAL_COMMAND` would carry — one line per act, and answers as
/// the runner would. `refuse` is a verb it fails on, so a test can have
/// the host decline one act and nothing else.
fn recorder(dir: &Path, refuse: &str) -> PathBuf {
    let log = dir.join("asked");
    let at = dir.join("ssh-recorder");
    std::fs::write(
        &at,
        format!(
            "#!/bin/sh\n\
             for a in \"$@\"; do last=\"$a\"; done\n\
             printf '%s\\n' \"$last\" >> '{}'\n\
             case \"$last\" in\n\
             \x20 {refuse}*) echo \"the host refused it\" >&2; exit 2 ;;\n\
             esac\n\
             echo \"ok: $last\"\n",
            log.display()
        ),
    )
    .expect("a recorder");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&at, std::fs::Permissions::from_mode(0o755)).unwrap();
    }
    at
}

/// What the host was asked for, in order.
fn asked_of_host(dir: &Path) -> Vec<String> {
    std::fs::read_to_string(dir.join("asked"))
        .unwrap_or_default()
        .lines()
        .map(str::to_string)
        .collect()
}

/// A private key as the operator's client would send one. **Nothing
/// verifies it here**: the host does, and the recorder is the host.
const RUNNER_KEY: &str =
    "-----BEGIN OPENSSH PRIVATE KEY-----\nbm90aGluZw==\n-----END OPENSSH PRIVATE KEY-----";

/// The body of an install act, form-encoded as a client sends it.
fn install_body(resource: &str, owner: &str, image: &str, port: u16, manifest: &str) -> Vec<u8> {
    let escape = |s: &str| {
        s.bytes()
            .map(|b| match b {
                b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                    (b as char).to_string()
                }
                _ => format!("%{b:02X}"),
            })
            .collect::<String>()
    };
    format!(
        "resource={resource}&owner={owner}&authority=testapp.internal&image={}&port={port}&manifest={}&key={}",
        escape(image),
        escape(manifest),
        escape(RUNNER_KEY)
    )
    .into_bytes()
}

// acceptance: DMN-39
#[test]
fn a_package_is_installed_on_the_hosts_shell_and_the_node_then_hosts_it() {
    let (l, d, at) = enrolled_with("install", 37455, |l| {
        l.ssh_client = Some(recorder(&l.dir, "none"));
    });
    let resource = hex(&[0xa7u8; 32]);
    let owner = hex(&test_identity("bob").public.keyhash);
    let image = "ggcr.io/mzuech/fuerosnet/rhtn-testapp:abc12345";

    // nothing installed yet, and the surface says so
    let none = ask(at, "GET /node/packages HTTP/1.1", b"");
    assert_eq!(field(&none, "packages").as_deref(), Some("0"), "{none}");

    let said = ask(
        at,
        "POST /node/package/install HTTP/1.1",
        &install_body(&resource, &owner, image, 37460, "roles = [\"reader\"]\n"),
    );
    assert!(said.starts_with("HTTP/1.1 200"), "{said}");
    assert_eq!(field(&said, "installed").as_deref(), Some(&resource[..]));
    assert_eq!(field(&said, "address").as_deref(), Some("127.0.0.1:37460"));
    // **no grant**: §7 has a package's roles start bound to nobody, and
    // the answer says so rather than leaving the operator to find out
    assert_eq!(field(&said, "grants").as_deref(), Some("none"));

    // **the host was asked for the image before anything was written,
    // and for the package afterwards**
    let asked = asked_of_host(&l.dir);
    assert_eq!(
        asked,
        vec![
            format!("pull {image}"),
            format!(
                "start {} {image} --listen 127.0.0.1:37460 --resource {resource}",
                &resource[..16]
            ),
        ],
        "the vocabulary, in order"
    );

    // the manifest where the node reads it, and the package's own words
    let manifest = l.dir.join("packages").join(format!("{resource}.manifest"));
    assert_eq!(
        std::fs::read_to_string(&manifest).expect("the manifest is written"),
        "roles = [\"reader\"]\n"
    );

    // the record, and the half of the hosting file generated from it
    let hosting = std::fs::read_to_string(l.dir.join("hosting")).unwrap();
    assert!(
        hosting.contains(rhtn_daemon::packages::SENTINEL),
        "{hosting}"
    );
    assert!(
        hosting.contains(&format!("resource = \"{resource}\"")),
        "{hosting}"
    );
    assert!(
        hosting.contains("address = \"127.0.0.1:37460\""),
        "{hosting}"
    );
    let record = std::fs::read_to_string(l.dir.join("packages.toml")).unwrap();
    assert!(record.contains(&resource), "{record}");

    // **and the node has re-read it**, so it is hosting the resource: the
    // surface's own listing says so, and so does the page
    let listed = ask(at, "GET /node/packages HTTP/1.1", b"");
    assert_eq!(field(&listed, "packages").as_deref(), Some("1"), "{listed}");
    let page = ask(at, "GET / HTTP/1.1", b"");
    assert!(page.contains(&resource), "the page shows what it hosts");

    // **a second install of the same resource replaces it** rather than
    // running beside it, which is how a package is upgraded
    let again = ask(
        at,
        "POST /node/package/install HTTP/1.1",
        &install_body(
            &resource,
            &owner,
            "ggcr.io/mzuech/fuerosnet/rhtn-testapp:def67890",
            37460,
            "roles = [\"reader\"]\n",
        ),
    );
    assert!(again.starts_with("HTTP/1.1 200"), "{again}");
    let listed = ask(at, "GET /node/packages HTTP/1.1", b"");
    assert_eq!(field(&listed, "packages").as_deref(), Some("1"), "{listed}");

    // removed: the host is asked to stop it, the entry goes, and what the
    // operator decided about it is kept
    let gone = ask(
        at,
        "POST /node/package/remove HTTP/1.1",
        format!(
            "resource={resource}&key={}",
            RUNNER_KEY.replace('\n', "%0A")
        )
        .as_bytes(),
    );
    assert!(gone.starts_with("HTTP/1.1 200"), "{gone}");
    assert!(
        asked_of_host(&l.dir)
            .iter()
            .any(|a| a == &format!("stop {}", &resource[..16])),
        "the host was asked to stop it"
    );
    let listed = ask(at, "GET /node/packages HTTP/1.1", b"");
    assert_eq!(field(&listed, "packages").as_deref(), Some("0"), "{listed}");
    assert!(!manifest.exists(), "the relayed manifest goes with it");

    d.stop();
    let _ = std::fs::remove_dir_all(&l.dir);
}

// acceptance: DMN-39
#[test]
fn an_install_the_node_or_the_host_refuses_leaves_the_configuration_as_it_was() {
    let (l, d, at) = enrolled_with("install-undo", 37456, |l| {
        l.ssh_client = Some(recorder(&l.dir, "start"));
    });
    // the operator's own line, which no install may touch
    let theirs = "# mine, by hand\n";
    std::fs::write(l.dir.join("hosting"), theirs).unwrap();
    let resource = hex(&[0xb3u8; 32]);
    let owner = hex(&test_identity("bob").public.keyhash);
    let image = "ggcr.io/mzuech/fuerosnet/rhtn-testapp:abc12345";

    // **a manifest this host will not admit**: refused before the package
    // is ever started, so the host is asked for the image and no more
    let refused = ask(
        at,
        "POST /node/package/install HTTP/1.1",
        &install_body(
            &resource,
            &owner,
            image,
            37461,
            "roles = [\"reader\"]\nimports = [\"rhtn/1:nothing-like-this\"]\n",
        ),
    );
    assert!(refused.starts_with("HTTP/1.1 400"), "{refused}");
    assert!(body(&refused).contains("nothing changed"), "{refused}");
    assert_eq!(
        asked_of_host(&l.dir),
        vec![format!("pull {image}")],
        "nothing was started"
    );
    assert_eq!(
        rhtn_daemon::packages::above(&std::fs::read_to_string(l.dir.join("hosting")).unwrap()),
        theirs,
        "the operator's own half is as they left it"
    );
    let listed = ask(at, "GET /node/packages HTTP/1.1", b"");
    assert_eq!(field(&listed, "packages").as_deref(), Some("0"), "{listed}");

    // **and a host that will not start it** puts the entry back too: the
    // recorder above refuses every `start`
    let undone = ask(
        at,
        "POST /node/package/install HTTP/1.1",
        &install_body(&resource, &owner, image, 37461, "roles = [\"reader\"]\n"),
    );
    assert!(undone.starts_with("HTTP/1.1 502"), "{undone}");
    assert!(body(&undone).contains("nothing changed"), "{undone}");
    let listed = ask(at, "GET /node/packages HTTP/1.1", b"");
    assert_eq!(field(&listed, "packages").as_deref(), Some("0"), "{listed}");
    assert_eq!(
        rhtn_daemon::packages::above(&std::fs::read_to_string(l.dir.join("hosting")).unwrap()),
        theirs
    );
    assert!(
        !l.dir
            .join("packages")
            .join(format!("{resource}.manifest"))
            .exists(),
        "the manifest it wrote is gone with the rest"
    );

    d.stop();
    let _ = std::fs::remove_dir_all(&l.dir);
}

// acceptance: DMN-39
#[test]
fn an_install_says_what_it_was_missing_rather_than_half_doing_it() {
    let (l, d, at) = enrolled_with("install-refusals", 37457, |l| {
        l.ssh_client = Some(recorder(&l.dir, "none"));
    });
    let resource = hex(&[0xc1u8; 32]);
    let owner = hex(&test_identity("bob").public.keyhash);
    let image = "ggcr.io/mzuech/fuerosnet/rhtn-testapp:abc12345";
    let manifest = "roles = [\"reader\"]\n";

    let refusing = |body_: Vec<u8>| ask(at, "POST /node/package/install HTTP/1.1", &body_);

    // **no key**: an instance holds none of its own, and is handed one
    let no_key = refusing(
        format!(
            "resource={resource}&owner={owner}&authority=testapp.internal&image={image}&port=37462&manifest=roles"
        )
        .into_bytes(),
    );
    assert!(no_key.starts_with("HTTP/1.1 400"), "{no_key}");
    assert!(body(&no_key).contains("restricted ssh key"), "{no_key}");

    // a key that is not one
    let wrong = refusing(
        format!("resource={resource}&owner={owner}&authority=testapp.internal&image={image}&port=37462&manifest=roles&key=hello")
            .into_bytes(),
    );
    assert!(wrong.starts_with("HTTP/1.1 400"), "{wrong}");
    assert!(body(&wrong).contains("OpenSSH private key"), "{wrong}");

    // **no manifest**: what a package declares is the package's, and this
    // node will not compose one on its behalf
    let bare = refusing(install_body(&resource, &owner, image, 37462, ""));
    assert!(bare.starts_with("HTTP/1.1 400"), "{bare}");
    assert!(body(&bare).contains("own manifest"), "{bare}");

    // an authority, an image and a port that are not what they say
    for (what, body_) in [
        (
            "authority",
            format!(
                "resource={resource}&owner={owner}&authority=Shop%2Finternal&image={image}&port=1&manifest=x&key={}",
                RUNNER_KEY.replace('\n', "%0A")
            ),
        ),
        (
            "image",
            format!(
                "resource={resource}&owner={owner}&authority=a.b&image=-rm&port=1&manifest=x&key={}",
                RUNNER_KEY.replace('\n', "%0A")
            ),
        ),
        (
            "port",
            format!(
                "resource={resource}&owner={owner}&authority=a.b&image={image}&port=0&manifest=x&key={}",
                RUNNER_KEY.replace('\n', "%0A")
            ),
        ),
    ] {
        let said = refusing(body_.into_bytes());
        assert!(said.starts_with("HTTP/1.1 400"), "{what}: {said}");
    }

    // **the surface's own port is not a package's**
    let mine = refusing(install_body(&resource, &owner, image, at.port(), manifest));
    assert!(mine.starts_with("HTTP/1.1 400"), "{mine}");
    assert!(body(&mine).contains("administration surface"), "{mine}");

    // **two resources at one address is one resource answering for both**
    assert!(
        refusing(install_body(&resource, &owner, image, 37462, manifest))
            .starts_with("HTTP/1.1 200")
    );
    let other = hex(&[0xc2u8; 32]);
    let clash = refusing(install_body(&other, &owner, image, 37462, manifest));
    assert!(clash.starts_with("HTTP/1.1 400"), "{clash}");
    assert!(body(&clash).contains("already listens"), "{clash}");

    // **a package this node did not install is not its to remove**: the
    // hosting file's upper half is the operator's
    let theirs = ask(
        at,
        "POST /node/package/remove HTTP/1.1",
        format!("resource={other}&key={}", RUNNER_KEY.replace('\n', "%0A")).as_bytes(),
    );
    assert!(theirs.starts_with("HTTP/1.1 404"), "{theirs}");
    assert!(body(&theirs).contains("by hand"), "{theirs}");

    // nothing was asked of the host but the one install that succeeded
    assert_eq!(asked_of_host(&l.dir).len(), 2, "one pull and one start");

    d.stop();
    let _ = std::fs::remove_dir_all(&l.dir);
}

// acceptance: DMN-39
#[test]
fn an_instance_with_no_shell_on_its_host_installs_nothing_and_says_which_line_is_missing() {
    // **a serving node, and no way to reach its host**: an instance
    // provisioned before the host could be asked for anything names no
    // `host-shell`, and the install it cannot make must say which line
    let (l, d, at) = enrolled_with("install-no-shell", 37458, |l| {
        let conf = std::fs::read_to_string(&l.config).unwrap();
        std::fs::write(
            &l.config,
            conf.replace("host-shell = \"127.0.0.1:22\"\n", ""),
        )
        .unwrap();
    });
    let said = ask(
        at,
        "POST /node/package/install HTTP/1.1",
        &install_body(&hex(&[1u8; 32]), &hex(&[2u8; 32]), "a/b:1", 37463, "x"),
    );
    assert!(said.starts_with("HTTP/1.1 409"), "{said}");
    assert!(body(&said).contains("host-shell"), "{said}");
    assert!(
        body(&said).contains("reason it invented"),
        "it says why rather than failing for an invented one: {said}"
    );
    d.stop();
    let _ = std::fs::remove_dir_all(&l.dir);
}

// acceptance: DMN-39
#[test]
fn a_host_shell_that_is_not_this_instances_own_is_refused_in_the_configuration() {
    let l = layout("install-far-shell", &TOKEN, 37459);
    let conf = std::fs::read_to_string(&l.config).unwrap();
    std::fs::write(
        &l.config,
        conf.replace(
            "host-shell = \"127.0.0.1:22\"",
            "host-shell = \"203.0.113.9:22\"",
        ),
    )
    .unwrap();
    let d = spawn(&l);
    // it does not come up at all: a key that would be sent to another
    // machine is refused where the operator wrote it
    assert!(
        d.wait_line("loopback", 20).is_some(),
        "the daemon said {:?}",
        d.lines()
    );
    d.stop();
    let _ = std::fs::remove_dir_all(&l.dir);
}

// acceptance: DMN-36
#[test]
fn the_three_routes_that_carry_a_signature_need_no_proof_from_anywhere() {
    // **the regression this is written against** [2026-11-02]: the act
    // proof was applied to every route, including the three that verify
    // the operator's own signature — so a client enrolling a launched
    // instance from a phone met 401 on the very exchange that would have
    // given it something to prove with.
    let Some(ip) = outbound_address() else {
        eprintln!("no outbound address on this machine; this test needs one");
        return;
    };
    let token = [0x77u8; 32];
    let l = layout_on("unproved-records", &token, 37464, "0.0.0.0:0");
    let mut d = spawn(&l);
    let port = d.surface_at().port();
    let away: SocketAddr = (ip, port).into();

    // the bootstrap, from off the loopback: it is the one read that
    // discloses only what is public, and proves the instance to the client
    let got = ask(
        away,
        &format!("GET /node?nonce={} HTTP/1.1", hex(&[2u8; 16])),
        b"",
    );
    assert_eq!(200, status(&got), "{got}");
    let key: [u8; 32] = unhex(&field(&got, "transport").expect("a key"))
        .try_into()
        .expect("32 bytes");

    // and the three that carry a signature, with no nonce and no proof
    let bob = test_identity("bob");
    let point = NetworkPoint::new([127, 0, 0, 1], Some(37464));
    let record = endpoint_record(&bob, std::slice::from_ref(&point), seqno(1));
    let entry = anchor_entry(&bob, std::slice::from_ref(&point), 3, seqno(1));
    assert_eq!(
        200,
        status(&ask(away, "PUT /node/endpoint-record HTTP/1.1", &record)),
        "a record the operator signed authorises itself"
    );
    assert_eq!(
        200,
        status(&ask(away, "PUT /node/anchor-entry HTTP/1.1", &entry))
    );
    for c in issue_run(&bob, &key, now(), 1) {
        assert_eq!(200, status(&ask(away, "PUT /node/run HTTP/1.1", &c)));
    }
    assert!(
        d.serving().is_some(),
        "it serves, enrolled from off the loopback"
    );

    // **and what carries no signature still proves itself there**: the
    // exemption is the three, not the surface
    let open = ask(away, "POST /node/acknowledge HTTP/1.1", b"acknowledge=on");
    assert_eq!(401, status(&open), "{open}");
    // as does a run that is not this operator's
    let eve = test_identity("eve");
    for c in issue_run(&eve, &key, now(), 1) {
        let refused = ask(away, "PUT /node/run HTTP/1.1", &c);
        assert_eq!(
            400,
            status(&refused),
            "a stranger's run is refused: {refused}"
        );
    }

    d.stop();
    let _ = std::fs::remove_dir_all(&l.dir);
}

// acceptance: DMN-40
#[test]
fn an_instance_declares_what_it_can_be_asked_for_and_one_route_invokes_it() {
    const RES: &str = "aa00000000000000000000000000000000000000000000000000000000000001";
    let (l, d, at) = enrolled_with("declared", 37465, |l| {
        l.ssh_client = Some(recorder(&l.dir, "none"));
        let manifest = l.dir.join("declared.manifest");
        std::fs::write(
            &manifest,
            "roles = [\"reader\", \"writer\"]\n\n[[admin]]\nname = \"rename\"\nlabel = \"Rename \
             this instance\"\n\n[[admin.parameter]]\nname = \"title\"\nlabel = \"its name\"\n\
             type = \"text\"\nmax = 40\n",
        )
        .unwrap();
        std::fs::write(
            l.dir.join("hosting"),
            format!(
                "[[host]]\nresource = \"{RES}\"\nowner = \"{}\"\nauthority = \
                 \"shop.internal\"\nmanifest = \"{}\"\n",
                hex(&test_identity("bob").public.keyhash),
                manifest.display()
            ),
        )
        .unwrap();
    });

    // **the declaration, which is what a client draws its screens from**
    let said = ask(at, "GET /node/acts HTTP/1.1", b"");
    assert_eq!(200, status(&said), "{said}");
    let d_json: serde_json::Value =
        serde_json::from_str(body(&said)).expect("the declaration is JSON");
    assert_eq!(
        d_json["surface"], "rhtn/administration@1",
        "it names the vocabulary it is written in"
    );

    // the node's own acts, each with what a client must bring
    let acts = d_json["node"].as_array().expect("the node's acts");
    let find = |name: &str| {
        acts.iter()
            .find(|a| a["name"] == name)
            .unwrap_or_else(|| panic!("{name} is declared"))
            .clone()
    };
    assert_eq!(find("reload")["brings"], "nothing");
    assert_eq!(find("stop")["brings"], "nothing");
    // **the act that reaches the machine says the client brings the key**,
    // which is the whole reason the acts left the page: the frame is
    // isolated from the seed it is derived from
    assert_eq!(find("install")["brings"], "host-key");
    assert_eq!(find("running")["brings"], "host-key");
    // and the key is never a field a person is asked for
    for a in acts {
        let names: Vec<&str> = a["parameters"]
            .as_array()
            .unwrap()
            .iter()
            .map(|p| p["name"].as_str().unwrap())
            .collect();
        assert!(!names.contains(&"key"), "{} declares key", a["name"]);
    }
    // the install's fields are the ones the route reads, so nothing is
    // translated between a declaration and the act it describes
    let installing = find("install");
    let install: Vec<&str> = installing["parameters"]
        .as_array()
        .unwrap()
        .iter()
        .map(|p| p["name"].as_str().unwrap())
        .collect();
    assert_eq!(
        install,
        vec![
            "resource",
            "owner",
            "authority",
            "image",
            "port",
            "manifest"
        ]
    );

    // **the resource's acts are of this instant**: the roles come from
    // the package's manifest, and with no grant standing there is nothing
    // to narrow or withdraw
    let per = d_json["resources"].as_array().expect("per resource");
    assert_eq!(per.len(), 1);
    assert_eq!(per[0]["resource"], RES);
    let theirs = per[0]["acts"].as_array().unwrap();
    let named: Vec<&str> = theirs.iter().map(|a| a["name"].as_str().unwrap()).collect();
    assert!(named.contains(&"grant"), "{named:?}");
    assert!(!named.contains(&"grant-drop"), "nothing to withdraw yet");
    // the package's own declared operation travels in the same shape
    assert!(named.contains(&"rename"), "{named:?}");
    // a grant's roles are the package's, as a set rather than a choice
    let grant = theirs.iter().find(|a| a["name"] == "grant").unwrap();
    let roles = grant["parameters"]
        .as_array()
        .unwrap()
        .iter()
        .find(|p| p["name"] == "roles")
        .expect("roles");
    assert_eq!(roles["kind"], "several");
    assert_eq!(
        roles["of"],
        serde_json::json!(["reader", "writer"]),
        "the roles the package declared and no others"
    );
    // **and it is not of this build**: a grant made adds what may be
    // narrowed
    assert_eq!(
        200,
        status(&ask(
            at,
            "POST /node/act/grant HTTP/1.1",
            format!("resource={RES}&roles=reader&of=everyone").as_bytes()
        ))
    );
    let again: serde_json::Value =
        serde_json::from_str(body(&ask(at, "GET /node/acts HTTP/1.1", b""))).unwrap();
    let named: Vec<&str> = again["resources"][0]["acts"]
        .as_array()
        .unwrap()
        .iter()
        .map(|a| a["name"].as_str().unwrap())
        .collect();
    assert!(named.contains(&"grant-drop"), "{named:?}");
    assert!(named.contains(&"grant-narrow"), "{named:?}");

    // **one route invokes any of them**, and an act's declared fields are
    // its body
    assert_eq!(
        200,
        status(&ask(at, "POST /node/act/reload HTTP/1.1", b"")),
        "a node act"
    );
    assert_eq!(
        200,
        status(&ask(
            at,
            "POST /node/act/grant-drop HTTP/1.1",
            format!("resource={RES}&at=0").as_bytes()
        )),
        "a resource act"
    );

    // **an unknown name sends a client to the declaration** rather than
    // guessing: it is a client built against another version
    let unknown = ask(at, "POST /node/act/fly HTTP/1.1", b"");
    assert_eq!(404, status(&unknown), "{unknown}");
    assert!(body(&unknown).contains("/node/acts"), "{unknown}");
    assert!(
        body(&unknown).contains("rhtn/administration@1"),
        "{unknown}"
    );

    d.stop();
    let _ = std::fs::remove_dir_all(&l.dir);
}

// acceptance: DMN-40
#[test]
fn a_declared_act_is_proved_as_any_act_is_and_not_otherwise() {
    let Some(ip) = outbound_address() else {
        eprintln!("no outbound address on this machine; this test needs one");
        return;
    };
    let token = [0x3cu8; 32];
    let (_l, d, at) = enrolled_on("declared-proof", 37466, &token);
    let away: SocketAddr = (ip, at.port()).into();

    // the declaration is a read of this instance's own state, so it is an
    // act like any other: refused off the loopback without a proof
    assert_eq!(401, status(&ask(away, "GET /node/acts HTTP/1.1", b"")));
    let target = proved(&token, "GET", "/node/acts", b"", &[21u8; 16]);
    let said = ask(away, &format!("GET {target} HTTP/1.1"), b"");
    assert_eq!(200, status(&said), "{said}");

    // and so is an invocation
    let body_ = b"acknowledge=on";
    assert_eq!(
        401,
        status(&ask(away, "POST /node/act/acknowledge HTTP/1.1", body_))
    );
    let target = proved(&token, "POST", "/node/act/acknowledge", body_, &[22u8; 16]);
    let done = ask(away, &format!("POST {target} HTTP/1.1"), body_);
    assert_eq!(200, status(&done), "{done}");

    d.stop();
}
