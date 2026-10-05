//! `rhtn diag collect` and `rhtn diag watch`: the live side of the bench
//! (`Robot/field-test-diagnostics.md`, M4b).
//!
//! **Collect** listens on a TCP port of the laptop's LAN address and takes
//! any number of connections, one per phone, each a stream of the same
//! JSON lines the phone's event file receives (`DiagStream.kt`).  The first
//! line of a connection is the hello: a `diag.hello` event carrying the
//! phone's run id, the serial the bench gave it, the bundle header's
//! fields and `unix_ms`, so it doubles as the file's anchor.  Each
//! connection's lines are appended to `<dir>/<serial-or-run>.jsonl`; a
//! reconnect from the same run appends to the same file and its hello is
//! kept once.  The stream is plaintext on the tester's own network and
//! lossy by design (the phone drops its oldest lines when the bench is
//! away for long, and says so with `diag.dropped`); the file pulled from
//! the phone at the end of the run is the record.
//!
//! **Watch** follows every `*.jsonl` under a directory (the collector's
//! files, and the daemon's and witnesses' where the directory is the run's)
//! and prints each new ceremony-step, refusal or abort event as it is
//! appended, with its source and its `ms`, placed on the wall clock by its
//! file's anchor as [`crate::diag::merge`] places it.  It is a tail, not a
//! merge: lines are printed in the order they arrive, each with its own
//! offset, and the merge at the end of the run is the ordered record.

use crate::diag::{Line, is_ceremony_step, is_refusal, parse_line};
use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::io::{self, BufRead, BufReader, Read, Seek, SeekFrom, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

/// The event a connection opens with.
pub const HELLO_EVENT: &str = "diag.hello";

/// A name safe as a file stem: letters, digits, `.`, `_` and `-`; anything
/// else becomes `_`; at most 64 characters.
pub fn file_stem(s: &str) -> String {
    let mut out: String = s
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-') {
                c
            } else {
                '_'
            }
        })
        .take(64)
        .collect();
    if out.is_empty() || out.chars().all(|c| c == '.') {
        out = "unnamed".into();
    }
    out
}

fn field<'a>(l: &'a Line, k: &str) -> Option<&'a str> {
    l.fields
        .iter()
        .find(|(key, _)| key == k)
        .and_then(|(_, v)| v.as_str())
}

/// The file a connection's lines go to: by the hello's `serial`, else its
/// `run`, else the peer's address.
pub fn file_id(hello: Option<&Line>, peer: &SocketAddr) -> String {
    let from_hello = hello.and_then(|h| {
        field(h, "serial")
            .filter(|s| !s.trim().is_empty())
            .or_else(|| field(h, "run"))
    });
    file_stem(
        &from_hello
            .map(str::to_string)
            .unwrap_or_else(|| peer.to_string()),
    )
}

/// Whether the file already carries a hello for this run, so a reconnect's
/// hello is not written a second time.
fn has_hello_for(path: &Path, run: Option<&str>) -> bool {
    let Ok(text) = std::fs::read_to_string(path) else {
        return false;
    };
    text.lines().enumerate().any(|(i, raw)| {
        parse_line("", i + 1, raw)
            .is_some_and(|(l, _)| l.event == HELLO_EVENT && field(&l, "run") == run)
    })
}

/// One connection, to its end: the hello placed, every line appended and
/// flushed as it comes.  `log` takes the connect and disconnect lines.
pub fn serve_connection(stream: TcpStream, dir: &Path, log: &dyn Fn(String)) -> io::Result<()> {
    let peer = stream.peer_addr()?;
    let _ = stream.set_read_timeout(Some(Duration::from_secs(600)));
    let mut reader = BufReader::new(stream);
    let mut first = String::new();
    if reader.read_line(&mut first)? == 0 {
        log(format!("connect {peer}: closed before a hello"));
        return Ok(());
    }
    let first = first.trim_end_matches(['\r', '\n']).to_string();
    let hello = parse_line("", 1, &first)
        .map(|(l, _)| l)
        .filter(|l| l.event == HELLO_EVENT);
    let id = file_id(hello.as_ref(), &peer);
    let path = dir.join(format!("{id}.jsonl"));
    let run = hello
        .as_ref()
        .and_then(|h| field(h, "run"))
        .map(str::to_string);
    let keep_hello = hello.is_none() || !has_hello_for(&path, run.as_deref());
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)?;
    log(format!(
        "connect {peer} {id} -> {} ({})",
        path.display(),
        match (&hello, keep_hello) {
            (None, _) => "no hello: first line kept as it is".to_string(),
            (Some(_), true) => format!("run {}", run.as_deref().unwrap_or("?")),
            (Some(_), false) => format!("run {}, reconnect", run.as_deref().unwrap_or("?")),
        }
    ));
    let mut n = 0usize;
    if keep_hello {
        writeln!(file, "{first}")?;
        file.flush()?;
        n += 1;
    }
    let mut line = String::new();
    loop {
        line.clear();
        match reader.read_line(&mut line) {
            Ok(0) => break,
            Ok(_) => {
                let l = line.trim_end_matches(['\r', '\n']);
                if l.is_empty() {
                    continue;
                }
                writeln!(file, "{l}")?;
                file.flush()?;
                n += 1;
            }
            Err(e) => {
                log(format!("disconnect {peer} {id} after {n} lines: {e}"));
                return Ok(());
            }
        }
    }
    log(format!("disconnect {peer} {id} after {n} lines"));
    Ok(())
}

/// Accept connections until the listener fails, one thread each.
pub fn collect(
    listener: TcpListener,
    dir: PathBuf,
    log: Arc<dyn Fn(String) + Send + Sync>,
) -> io::Result<()> {
    std::fs::create_dir_all(&dir)?;
    for stream in listener.incoming() {
        let stream = stream?;
        let dir = dir.clone();
        let log = log.clone();
        std::thread::spawn(move || {
            if let Err(e) = serve_connection(stream, &dir, &*log) {
                log(format!("connection failed: {e}"));
            }
        });
    }
    Ok(())
}

/// `rhtn diag collect --listen <addr> --into <dir>`: runs until killed.
pub fn collect_main(listen: &str, into: &str) -> Result<String, String> {
    let listener = TcpListener::bind(listen).map_err(|e| format!("listen on {listen}: {e}"))?;
    let dir = PathBuf::from(into);
    std::fs::create_dir_all(&dir).map_err(|e| format!("{into}: {e}"))?;
    let addr = listener.local_addr().map_err(|e| e.to_string())?;
    println!("listening on {addr}, writing into {}", dir.display());
    let log: Arc<dyn Fn(String) + Send + Sync> = Arc::new(|s: String| {
        println!("{s}");
        let _ = io::stdout().flush();
    });
    collect(listener, dir, log).map_err(|e| e.to_string())?;
    Ok(String::new())
}

/// One followed file.
struct Tail {
    name: String,
    offset: u64,
    partial: String,
    n: usize,
    anchor: Option<(u64, u64)>,
}

/// The directory being followed.
pub struct Watcher {
    dir: PathBuf,
    tails: BTreeMap<PathBuf, Tail>,
    first_wall: Option<u64>,
}

impl Watcher {
    /// A watcher over `dir`, following nothing until it is swept.
    pub fn new(dir: impl Into<PathBuf>) -> Self {
        Watcher {
            dir: dir.into(),
            tails: BTreeMap::new(),
            first_wall: None,
        }
    }

    /// Every `*.jsonl` under the directory, except under a `merge`
    /// directory (the copies the stop-time merge reads) and a pulled
    /// bundle's `report` directory, both of which repeat other files.
    fn files(&self) -> Vec<PathBuf> {
        fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
            let Ok(rd) = std::fs::read_dir(dir) else {
                return;
            };
            for e in rd.flatten() {
                let p = e.path();
                let name = e.file_name().to_string_lossy().to_string();
                if p.is_dir() {
                    if name != "merge" && name != "report" {
                        walk(&p, out);
                    }
                } else if name.ends_with(".jsonl") {
                    out.push(p);
                }
            }
        }
        let mut out = Vec::new();
        walk(&self.dir, &mut out);
        out.sort();
        out
    }

    /// Whether a line is printed: a ceremony step, a refusal or an abort.
    pub fn interesting(l: &Line) -> bool {
        is_ceremony_step(l) || is_refusal(l) || l.event.contains("abort")
    }

    /// Read what every file gained since the last poll and print the
    /// interesting lines.  Returns how many were printed.
    pub fn poll(&mut self, out: &mut dyn Write) -> io::Result<usize> {
        let mut printed = 0;
        for path in self.files() {
            let name = path
                .strip_prefix(&self.dir)
                .unwrap_or(&path)
                .to_string_lossy()
                .to_string();
            let fresh = !self.tails.contains_key(&path);
            let tail = self.tails.entry(path.clone()).or_insert_with(|| Tail {
                name: name.clone(),
                offset: 0,
                partial: String::new(),
                n: 0,
                anchor: None,
            });
            if fresh {
                writeln!(out, "following {name}")?;
            }
            let Ok(meta) = std::fs::metadata(&path) else {
                continue;
            };
            if meta.len() < tail.offset {
                // truncated or replaced: start it over
                tail.offset = 0;
                tail.partial.clear();
                tail.n = 0;
                tail.anchor = None;
                writeln!(out, "{name}: shrank, read again from its start")?;
            }
            if meta.len() == tail.offset {
                continue;
            }
            let mut f = std::fs::File::open(&path)?;
            f.seek(SeekFrom::Start(tail.offset))?;
            let mut buf = Vec::new();
            f.read_to_end(&mut buf)?;
            tail.offset += buf.len() as u64;
            tail.partial.push_str(&String::from_utf8_lossy(&buf));
            let mut rest = std::mem::take(&mut tail.partial);
            while let Some(i) = rest.find('\n') {
                let raw: String = rest.drain(..=i).collect();
                tail.n += 1;
                let Some((mut l, anchor)) = parse_line(&tail.name, tail.n, raw.trim_end()) else {
                    continue;
                };
                if tail.anchor.is_none() && anchor.is_some() {
                    tail.anchor = anchor;
                }
                l.wall = match tail.anchor {
                    Some((at_ms, unix_ms)) => unix_ms.wrapping_add(l.ms).wrapping_sub(at_ms),
                    None => l.ms,
                };
                if !Self::interesting(&l) {
                    continue;
                }
                if tail.anchor.is_some() {
                    self.first_wall.get_or_insert(l.wall);
                }
                writeln!(
                    out,
                    "{}",
                    render(&l, self.first_wall, tail.anchor.is_some())
                )?;
                printed += 1;
            }
            tail.partial = rest;
        }
        Ok(printed)
    }
}

/// One printed line: the offset from the first anchored event the watch
/// saw (or `unanchored`), the source, the line's own `ms`, level, layer,
/// event and fields.
fn render(l: &Line, first_wall: Option<u64>, anchored: bool) -> String {
    let at = match (anchored, first_wall) {
        (true, Some(f)) => format!("+{}", l.wall.saturating_sub(f)),
        _ => "unanchored".to_string(),
    };
    let mut s = format!(
        "{at:>10}  {:<28} ms={:<9} {:<5} {:<6} {}",
        l.source, l.ms, l.level, l.layer, l.event
    );
    for (k, v) in &l.fields {
        if k != "span" {
            let shown = match v {
                serde_json::Value::String(t) => t.clone(),
                other => other.to_string(),
            };
            let _ = write!(s, "  {k}={shown}");
        }
    }
    s
}

/// Follow `dir` until `stop` is set, polling every `interval`.
pub fn watch(
    dir: &Path,
    out: &mut dyn Write,
    stop: &AtomicBool,
    interval: Duration,
) -> io::Result<()> {
    let mut w = Watcher::new(dir);
    while !stop.load(Ordering::Relaxed) {
        w.poll(out)?;
        out.flush()?;
        std::thread::sleep(interval);
    }
    w.poll(out)?;
    out.flush()
}

/// `rhtn diag watch <dir>`: runs until killed.
pub fn watch_main(dir: &str) -> Result<String, String> {
    let path = Path::new(dir);
    if !path.is_dir() {
        return Err(format!("{dir} is not a directory"));
    }
    let stop = AtomicBool::new(false);
    let stdout = io::stdout();
    let mut out = stdout.lock();
    watch(path, &mut out, &stop, Duration::from_millis(250)).map_err(|e| e.to_string())?;
    Ok(String::new())
}
