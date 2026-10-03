//! `rhtn diag collect` and `rhtn diag watch`: two phones streaming at
//! once, one of them reconnecting, each into its own file with the hello
//! kept once; and a watch that prints a step event the moment it is
//! appended to a followed file (`Robot/field-test-diagnostics.md`, M4b).

use rhtn_cli::diag::parse;
use rhtn_cli::live::{Watcher, collect, file_id, file_stem, watch};
use std::io::Write;
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

fn hello(serial: &str, run: &str) -> String {
    format!(
        r#"{{"ms":12,"level":"info","layer":"shell","event":"diag.hello","serial":"{serial}","run":"{run}","unix_ms":1700000020000,"device":"Acme Phone"}}"#
    )
}

fn until(what: impl Fn() -> bool, why: &str) {
    let deadline = Instant::now() + Duration::from_secs(10);
    while !what() {
        assert!(Instant::now() < deadline, "{why}: not within 10 s");
        std::thread::sleep(Duration::from_millis(10));
    }
}

fn collector(dir: &Path) -> (SocketAddr, Arc<Mutex<Vec<String>>>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    let log: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
    let sink = log.clone();
    let dir = dir.to_path_buf();
    std::thread::spawn(move || {
        let f: Arc<dyn Fn(String) + Send + Sync> =
            Arc::new(move |s: String| sink.lock().unwrap().push(s));
        let _ = collect(listener, dir, f);
    });
    (addr, log)
}

fn send(s: &mut TcpStream, line: &str) {
    s.write_all(line.as_bytes()).unwrap();
    s.write_all(b"\n").unwrap();
    s.flush().unwrap();
}

#[test]
fn two_phones_stream_at_once_and_one_reconnects_into_the_same_file() {
    let tmp = tempdir("collect");
    let (addr, log) = collector(&tmp);
    let count = |word: &str| {
        log.lock()
            .unwrap()
            .iter()
            .filter(|l| l.starts_with(word))
            .count()
    };

    // A connects and sends two lines; B connects while A is still up
    let mut a = TcpStream::connect(addr).unwrap();
    send(&mut a, &hello("R58MA", "run-a"));
    send(
        &mut a,
        r#"{"ms":100,"level":"info","layer":"meet","event":"meet.step","to":"BRIEF"}"#,
    );
    send(
        &mut a,
        r#"{"ms":200,"level":"info","layer":"shell","event":"qr.shown","bytes":312}"#,
    );
    let mut b = TcpStream::connect(addr).unwrap();
    send(&mut b, &hello("R58MB", "run-b"));
    for i in 0..3 {
        send(
            &mut b,
            &format!(
                r#"{{"ms":{},"level":"info","layer":"shell","event":"ble","n":{i}}}"#,
                300 + i
            ),
        );
    }
    until(|| count("connect") == 2, "two connects");
    drop(b);
    until(|| count("disconnect") == 1, "B's disconnect");
    send(
        &mut a,
        r#"{"ms":900,"level":"warn","layer":"cer","event":"cer.abort","abort":"NoProximity"}"#,
    );
    drop(a);
    until(|| count("disconnect") == 2, "A's disconnect");

    // A again, the same run: the hello is not written twice
    let mut a2 = TcpStream::connect(addr).unwrap();
    send(&mut a2, &hello("R58MA", "run-a"));
    send(
        &mut a2,
        r#"{"ms":1500,"level":"info","layer":"meet","event":"meet.step","to":"CAPTURE"}"#,
    );
    until(|| count("connect") == 3, "A's reconnect");
    drop(a2);
    until(|| count("disconnect") == 3, "A's second disconnect");

    let a_text = std::fs::read_to_string(tmp.join("R58MA.jsonl")).unwrap();
    let b_text = std::fs::read_to_string(tmp.join("R58MB.jsonl")).unwrap();
    let a_lines: Vec<&str> = a_text.lines().collect();
    assert_eq!(a_lines.len(), 5, "{a_text}");
    assert_eq!(a_lines[0], hello("R58MA", "run-a"));
    assert_eq!(
        a_text.matches("diag.hello").count(),
        1,
        "the hello is kept once across the reconnect: {a_text}"
    );
    assert!(a_lines[3].contains("cer.abort"), "{a_text}");
    assert!(a_lines[4].contains("CAPTURE"), "{a_text}");
    let b_lines: Vec<&str> = b_text.lines().collect();
    assert_eq!(b_lines.len(), 4, "{b_text}");
    assert_eq!(b_lines[0], hello("R58MB", "run-b"));

    // the hello anchors the file for the merge tool
    let parsed = parse("R58MA", &a_text);
    assert_eq!(parsed.anchor, Some((12, 1_700_000_020_000)));
    assert_eq!(parsed.skipped, 0);

    // what the collector said
    let said = log.lock().unwrap().clone();
    let connects: Vec<&String> = said.iter().filter(|l| l.starts_with("connect")).collect();
    assert!(
        connects[0].contains("R58MA") && connects[0].contains("run run-a"),
        "{said:?}"
    );
    assert!(connects[1].contains("R58MB"), "{said:?}");
    assert!(connects[2].contains("reconnect"), "{said:?}");
    assert!(
        said.iter().any(|l| l.starts_with("disconnect")
            && l.contains("R58MB")
            && l.contains("after 4 lines")),
        "{said:?}"
    );
    // a second run from the same phone writes its own hello to the same file
    let mut a3 = TcpStream::connect(addr).unwrap();
    send(&mut a3, &hello("R58MA", "run-a2"));
    drop(a3);
    until(|| count("disconnect") == 4, "the new run's disconnect");
    let a_text = std::fs::read_to_string(tmp.join("R58MA.jsonl")).unwrap();
    assert_eq!(a_text.matches("diag.hello").count(), 2, "{a_text}");
}

#[test]
fn a_connection_without_a_hello_is_filed_by_its_address() {
    let tmp = tempdir("nohello");
    let (addr, log) = collector(&tmp);
    let mut c = TcpStream::connect(addr).unwrap();
    let local = c.local_addr().unwrap();
    send(
        &mut c,
        r#"{"ms":5,"level":"info","layer":"shell","event":"shell.start"}"#,
    );
    drop(c);
    until(
        || {
            log.lock()
                .unwrap()
                .iter()
                .any(|l| l.starts_with("disconnect"))
        },
        "disconnect",
    );
    let text =
        std::fs::read_to_string(tmp.join(format!("{}.jsonl", file_stem(&local.to_string()))))
            .unwrap();
    assert_eq!(text.lines().count(), 1);
    assert!(text.contains("shell.start"));
    assert_eq!(file_id(None, &local), file_stem(&local.to_string()));
    assert_eq!(file_stem("R58M/../x y"), "R58M_.._x_y");
    assert_eq!(file_stem(".."), "unnamed");
}

#[test]
fn watch_prints_a_step_event_the_moment_it_is_appended() {
    let tmp = tempdir("watch");
    std::fs::create_dir_all(tmp.join("phones")).unwrap();
    std::fs::create_dir_all(tmp.join("daemon")).unwrap();
    std::fs::create_dir_all(tmp.join("merge")).unwrap();
    // a daemon file present from the start, with its anchor and one event
    // that is neither a step nor a refusal
    std::fs::write(
        tmp.join("daemon/diag.jsonl"),
        concat!(
            r#"{"ms":2,"level":"info","layer":"diag","event":"diag.anchor","unix_ms":1700000010000}"#, "\n",
            r#"{"ms":40,"level":"info","layer":"daemon","event":"daemon.lifecycle","what":"serving"}"#, "\n",
        ),
    )
    .unwrap();
    // a copy under merge/ is not followed
    std::fs::write(
        tmp.join("merge/daemon.jsonl"),
        r#"{"ms":1,"level":"warn","layer":"cer","event":"cer.abort","abort":"Copied"}"#,
    )
    .unwrap();

    let mut w = Watcher::new(&tmp);
    let mut out = Vec::new();
    assert_eq!(w.poll(&mut out).unwrap(), 0);
    let text = String::from_utf8(out.clone()).unwrap();
    assert!(text.contains("following daemon/diag.jsonl"), "{text}");
    assert!(!text.contains("Copied"), "{text}");

    // a phone's streamed file appears, anchored by its hello
    let phone = tmp.join("phones/R58MA.jsonl");
    std::fs::write(&phone, format!("{}\n", hello("R58MA", "run-a"))).unwrap();
    assert_eq!(w.poll(&mut out).unwrap(), 0);
    // a step is appended, and a line is printed for it with source and ms;
    // a partial line waits for its newline
    let mut f = std::fs::OpenOptions::new()
        .append(true)
        .open(&phone)
        .unwrap();
    write!(f, r#"{{"ms":1012,"level":"info","layer":"meet","event":"meet.step","from":"BRIEF","to":"OPTICAL","trigger":"tap"}}"#).unwrap();
    f.flush().unwrap();
    assert_eq!(
        w.poll(&mut out).unwrap(),
        0,
        "a line without its newline is not yet a line"
    );
    writeln!(f).unwrap();
    writeln!(
        f,
        r#"{{"ms":1300,"level":"info","layer":"shell","event":"qr.shown","bytes":312}}"#
    )
    .unwrap();
    f.flush().unwrap();
    assert_eq!(w.poll(&mut out).unwrap(), 1);
    let text = String::from_utf8(out.clone()).unwrap();
    let step = text.lines().find(|l| l.contains("meet.step")).unwrap();
    // the hello's unix_ms 1700000020000 at ms 12 places ms 1012 at +0 of
    // what the watch has seen anchored
    assert!(step.starts_with("        +0  phones/R58MA.jsonl"), "{step}");
    assert!(step.contains("ms=1012"), "{step}");
    assert!(
        step.contains("from=BRIEF  to=OPTICAL  trigger=tap"),
        "{step}"
    );
    assert!(!text.contains("qr.shown"), "{text}");

    // a refusal in the daemon's file, 500 ms after the step on the wall clock
    let mut d = std::fs::OpenOptions::new()
        .append(true)
        .open(tmp.join("daemon/diag.jsonl"))
        .unwrap();
    writeln!(d, r#"{{"ms":11502,"level":"info","layer":"node","event":"node.object","decision":"Refused(NotASlot)"}}"#).unwrap();
    d.flush().unwrap();
    assert_eq!(w.poll(&mut out).unwrap(), 1);
    let text = String::from_utf8(out.clone()).unwrap();
    let refused = text.lines().find(|l| l.contains("node.object")).unwrap();
    assert!(
        refused.starts_with("      +500  daemon/diag.jsonl"),
        "{refused}"
    );
    assert!(refused.contains("decision=Refused(NotASlot)"), "{refused}");

    // the loop form: a thread follows the directory until told to stop
    let stop = Arc::new(AtomicBool::new(false));
    let shared: Arc<Mutex<Vec<u8>>> = Arc::new(Mutex::new(Vec::new()));
    struct Shared(Arc<Mutex<Vec<u8>>>);
    impl Write for Shared {
        fn write(&mut self, b: &[u8]) -> std::io::Result<usize> {
            self.0.lock().unwrap().extend_from_slice(b);
            Ok(b.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }
    let (dir, s2, out2) = (tmp.clone(), stop.clone(), shared.clone());
    let h =
        std::thread::spawn(move || watch(&dir, &mut Shared(out2), &s2, Duration::from_millis(20)));
    until(
        || {
            String::from_utf8_lossy(&shared.lock().unwrap())
                .contains("following phones/R58MA.jsonl")
        },
        "the watch picks up the files",
    );
    writeln!(
        f,
        r#"{{"ms":3000,"level":"warn","layer":"cer","event":"cer.abort","abort":"NoProximity"}}"#
    )
    .unwrap();
    f.flush().unwrap();
    until(
        || {
            String::from_utf8_lossy(&shared.lock().unwrap())
                .contains("cer.abort  abort=NoProximity")
        },
        "the abort is printed",
    );
    stop.store(true, Ordering::Relaxed);
    h.join().unwrap().unwrap();
}

fn tempdir(tag: &str) -> std::path::PathBuf {
    let d = std::env::temp_dir().join(format!(
        "rhtn-live-{tag}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&d).unwrap();
    d
}
