//! **The host surface a socket-backed instance calls**
//! (`infra-client-requirements.md` §9.3, `resource-requirements.md` §3.3):
//! its storage block, and a write to its log.
//!
//! A package in the sandbox reaches these as the bindings it imported, and
//! the host is the write path either way — which is the whole reason this
//! module exists [author, 2026-11-05]. §9.3 makes the byte ceiling the
//! host's to enforce **on every write**, with the refusal naming it; a
//! package writing the host's filesystem directly is one the host does not
//! interpose on, so the rule would bind nobody and what named the ceiling
//! would be the operating system's `ENOSPC` rather than the node's answer.
//! Asked for over a surface, one implementation — `rhtn_resources::Block` —
//! enforces it for both kinds of package, and nothing is reserved in
//! advance on a machine whose storage is finite.
//!
//! **What says which instance is calling is a secret, not the path.** Every
//! instance is told one on the relationships exchange, beside where to
//! reach this surface, and the instance is resolved *from* the secret. So
//! there is no instance to name in a request and none that could be named:
//! an instance that could address another's block would be something to
//! defend against, where one that cannot ask for it needs no defence.
//!
//! **Where it binds is the loopback, on whatever port is free.** Nothing
//! off this machine may reach it, the instances it serves are on the same
//! machine by `hosting.rs`'s own rule (plain HTTP may only cross a local
//! socket), and the port is told rather than agreed in advance — so there
//! is no number for a configuration, a firewall rule or a document to
//! carry, and no default to be wrong. Reaching the port is evidence of
//! nothing on its own, as §3.3 says of the leg running the other way: the
//! secret is the whole of the authorisation.

use rhtn_archive::Keyhash;
use rhtn_resources::{Block, LogFile};
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

/// The longest request this surface reads, body included.  A block write
/// is one HTTP message, so this bounds how much may be written at once —
/// not how large a block may be, which is the manifest's ceiling.
const MAX_REQUEST: usize = 1 << 20;

/// How long one caller may take to send its request.
const CALLER_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(10);

/// What one instance may reach here.
struct Held {
    /// The instance, which the secret resolves to.
    instance: Keyhash,
    /// Its secret, as it presents it.
    secret: [u8; 32],
    /// Its block, where it has one.
    block: Option<Arc<Block>>,
    /// Its log, where it has one.
    log: Option<Arc<LogFile>>,
}

/// **What a presented secret resolved to**: the instance it belongs to and
/// what that instance may reach.  A named type because the request path
/// carries all three together and a tuple of them reads as nothing.
struct Reached {
    instance: Keyhash,
    block: Option<Arc<Block>>,
    log: Option<Arc<LogFile>>,
}

/// The surface, bound and holding what each instance may reach.
pub struct HostSurface {
    listener: TcpListener,
    bound: SocketAddr,
    held: Mutex<Vec<Held>>,
}

impl HostSurface {
    /// Bind on the loopback, on a free port.  **The listener is bound
    /// here rather than in [`HostSurface::serve`]** because the port is
    /// part of what the relationships exchange tells an instance, and
    /// hosting is applied before anything is served: a surface whose
    /// address was not yet known would be told to nobody.
    pub async fn bind() -> std::io::Result<Arc<HostSurface>> {
        let listener = TcpListener::bind(SocketAddr::from(([127, 0, 0, 1], 0))).await?;
        let bound = listener.local_addr()?;
        Ok(Arc::new(HostSurface {
            listener,
            bound,
            held: Mutex::new(Vec::new()),
        }))
    }

    /// Where it bound.
    #[must_use]
    pub fn bound(&self) -> SocketAddr {
        self.bound
    }

    /// The URL an instance is told for its block.
    #[must_use]
    pub fn block_url(&self) -> String {
        format!("http://{}/block", self.bound)
    }

    /// The URL an instance is told for its log.
    #[must_use]
    pub fn log_url(&self) -> String {
        format!("http://{}/log", self.bound)
    }

    /// **What this instance presents, minted once and kept while it is
    /// hosted.**  Re-applying the hosting file does not rotate a secret an
    /// instance still holds: the facts are re-pushed on every change
    /// (`resource-requirements.md` §3.3), and an instance that had not yet
    /// taken the new push would otherwise be refused at its own block for
    /// as long as that took.
    pub fn register(
        &self,
        instance: Keyhash,
        block: Option<Arc<Block>>,
        log: Option<Arc<LogFile>>,
    ) -> [u8; 32] {
        let mut held = self.held.lock().unwrap();
        if let Some(h) = held.iter_mut().find(|h| h.instance == instance) {
            h.block = block;
            h.log = log;
            return h.secret;
        }
        let mut secret = [0u8; 32];
        {
            use aws_lc_rs::rand::SecureRandom as _;
            // a surface that could not mint a secret must not fall back to
            // a guessable one: an all-zero secret matches nothing, since a
            // registration that failed here is one no instance was told
            if aws_lc_rs::rand::SystemRandom::new()
                .fill(&mut secret)
                .is_err()
            {
                crate::say!(
                    "rhtnd: no randomness for a hosted instance's secret; its block is unreachable"
                );
                return [0u8; 32];
            }
        }
        held.push(Held {
            instance,
            secret,
            block,
            log,
        });
        secret
    }

    /// **Forget every instance not in `these`**: an instance the operator
    /// stopped hosting keeps nothing here, and its secret stops working
    /// with the binding rather than after it.
    pub fn retain(&self, these: &[Keyhash]) {
        self.held
            .lock()
            .unwrap()
            .retain(|h| these.contains(&h.instance));
    }

    /// Accept until dropped.
    pub async fn serve(self: Arc<Self>) {
        tracing::info!(
            target: "daemon",
            step = "host_surface_open",
            listen = %self.bound,
            "daemon.lifecycle"
        );
        loop {
            let Ok((stream, _)) = self.listener.accept().await else {
                continue;
            };
            let me = self.clone();
            // one instance's slow request is its own and not everyone's
            tokio::spawn(async move {
                let _ = tokio::time::timeout(CALLER_TIMEOUT * 3, me.exchange(stream)).await;
            });
        }
    }

    /// One connection: read a request, answer it, close.
    async fn exchange(&self, mut stream: TcpStream) {
        let began = std::time::Instant::now();
        let mut buf = Vec::new();
        let (head, body) = loop {
            let mut chunk = [0u8; 4096];
            let Some(left) = CALLER_TIMEOUT.checked_sub(began.elapsed()) else {
                let _ = answer(&mut stream, 408, "the request did not all arrive\n").await;
                return;
            };
            match tokio::time::timeout(left, stream.read(&mut chunk)).await {
                Err(_) => {
                    let _ = answer(&mut stream, 408, "the request did not all arrive\n").await;
                    return;
                }
                Ok(Ok(0)) => return,
                Ok(Ok(n)) => buf.extend_from_slice(&chunk[..n]),
                Ok(Err(_)) => return,
            }
            if buf.len() > MAX_REQUEST {
                let _ = answer(
                    &mut stream,
                    413,
                    "the request is larger than this surface takes\n",
                )
                .await;
                return;
            }
            if let Some(whole) = complete(&buf) {
                break whole;
            }
        };
        let (code, body) = self.act(&head, &body);
        let _ = answer_bytes(&mut stream, code, &body).await;
    }

    /// The act itself, given the request's head and body: a code and what
    /// to answer with.  **Separated from the socket** so the tests drive
    /// what the surface decides rather than how it reads.
    fn act(&self, head: &str, body: &[u8]) -> (u16, Vec<u8>) {
        let mut lines = head.lines();
        let Some(request) = lines.next() else {
            return (400, b"no request line\n".to_vec());
        };
        let mut parts = request.split(' ');
        let (Some(method), Some(target)) = (parts.next(), parts.next()) else {
            return (400, b"no method and target\n".to_vec());
        };
        // **a target with more than one reading is refused outright**, the
        // rule `resource-requirements.md` §3.3 already states for the leg
        // running the other way: an escape or a dot segment is a target
        // this surface and the caller could read differently
        if target.contains('%') || target.contains("/.") {
            return (400, b"that target has more than one reading\n".to_vec());
        }
        let Some(secret) = header(head, "rhtn-secret").and_then(unhex32) else {
            return (401, b"present rhtn-secret\n".to_vec());
        };
        let Some(Reached {
            instance,
            block,
            log,
        }) = self.resolve(&secret)
        else {
            // nothing of which instance, or whether one exists: an
            // unknown secret and a known one for a package with no block
            // are the same answer
            return (401, b"that secret reaches nothing here\n".to_vec());
        };
        let (path, query) = match target.split_once('?') {
            Some((p, q)) => (p, q),
            None => (target, ""),
        };
        let said = |code: u16, s: String| (code, s.into_bytes());
        match (method, path) {
            // the block's own operations (§9.3)
            ("GET", "/block") => {
                let Some(b) = block else {
                    return (404, b"this instance has no block\n".to_vec());
                };
                said(200, b.list().join("\n") + "\n")
            }
            ("POST", "/log") => {
                let Some(l) = log else {
                    return (404, b"this instance has no log\n".to_vec());
                };
                let Ok(line) = std::str::from_utf8(body) else {
                    return (400, b"a log line is text\n".to_vec());
                };
                // the host stamps it, which `LogFile::write` does
                l.write(line.trim_end_matches(['\n', '\r']));
                (204, Vec::new())
            }
            // **the log is outward only** (§9.3): the package writes and
            // the operator reads, so there is nothing here to read back
            ("GET", "/log") => (
                405,
                b"the log is written here and read by your operator\n".to_vec(),
            ),
            _ => {
                let Some(name) = path.strip_prefix("/block/") else {
                    return (404, b"not a path this surface has\n".to_vec());
                };
                let Some(b) = block else {
                    return (404, b"this instance has no block\n".to_vec());
                };
                self.on_block(method, name, query, body, &b, instance)
            }
        }
    }

    /// One act against one instance's block.
    fn on_block(
        &self,
        method: &str,
        name: &str,
        query: &str,
        body: &[u8],
        block: &Block,
        instance: Keyhash,
    ) -> (u16, Vec<u8>) {
        let (name, tail) = match name.split_once('/') {
            Some((n, t)) => (n, t),
            None => (name, ""),
        };
        let said = |code: u16, s: String| (code, s.into_bytes());
        match (method, tail) {
            ("GET", "size") => match block.size(name) {
                Ok(n) => said(200, format!("{n}\n")),
                Err(e) => said(404, format!("{e}\n")),
            },
            ("POST", "sync") => match block.sync(name) {
                Ok(()) => (204, Vec::new()),
                Err(e) => said(404, format!("{e}\n")),
            },
            ("GET", "") => {
                let offset = number(query, "offset").unwrap_or(0);
                let len = number(query, "len").unwrap_or(MAX_REQUEST as u64);
                match block.read(name, offset, len.min(MAX_REQUEST as u64) as usize) {
                    Ok(bytes) => (200, bytes),
                    Err(e) => said(404, format!("{e}\n")),
                }
            }
            ("PUT", "") => {
                let offset = number(query, "offset").unwrap_or(0);
                match block.write(name, offset, body) {
                    Ok(after) => said(200, format!("{after}\n")),
                    Err(e) => {
                        // **the ceiling is not a fault of the host's**, and
                        // the refusal names it because `Block::write`
                        // already does (§9.3)
                        let over = block.would_exceed(name, offset, body.len()).is_some();
                        if over {
                            tracing::info!(
                                target: "daemon",
                                instance = %hex8(&instance),
                                "daemon.block_over_ceiling"
                            );
                        }
                        said(if over { 413 } else { 400 }, format!("{e}\n"))
                    }
                }
            }
            ("DELETE", "") => match block.remove(name) {
                Ok(()) => (204, Vec::new()),
                Err(e) => said(404, format!("{e}\n")),
            },
            _ => (405, b"not something this surface does\n".to_vec()),
        }
    }

    /// The instance a secret resolves to, compared without leaking where
    /// it stopped matching.
    fn resolve(&self, secret: &[u8; 32]) -> Option<Reached> {
        let held = self.held.lock().unwrap();
        let mut found = None;
        for h in held.iter() {
            if aws_lc_rs::constant_time::verify_slices_are_equal(&h.secret, secret).is_ok() {
                found = Some(Reached {
                    instance: h.instance,
                    block: h.block.clone(),
                    log: h.log.clone(),
                });
            }
        }
        found
    }
}

/// The head and body of a complete HTTP/1.1 message, where `buf` holds
/// one: headers to the blank line, then `content-length` bytes.
fn complete(buf: &[u8]) -> Option<(String, Vec<u8>)> {
    let end = buf.windows(4).position(|w| w == b"\r\n\r\n")?;
    let head = String::from_utf8_lossy(&buf[..end]).to_string();
    let want = header(&head, "content-length")
        .and_then(|v| v.trim().parse::<usize>().ok())
        .unwrap_or(0);
    let from = end + 4;
    if buf.len() < from + want {
        return None;
    }
    Some((head, buf[from..from + want].to_vec()))
}

/// One header's value, case-insensitively.
fn header(head: &str, name: &str) -> Option<String> {
    head.lines().skip(1).find_map(|l| {
        let (k, v) = l.split_once(':')?;
        k.trim()
            .eq_ignore_ascii_case(name)
            .then(|| v.trim().to_string())
    })
}

/// One `name=<digits>` from a query string.
fn number(query: &str, name: &str) -> Option<u64> {
    query.split('&').find_map(|p| {
        let (k, v) = p.split_once('=')?;
        (k == name).then(|| v.parse().ok())?
    })
}

fn unhex32(s: String) -> Option<[u8; 32]> {
    if s.len() != 64 {
        return None;
    }
    let mut out = [0u8; 32];
    for (i, b) in out.iter_mut().enumerate() {
        *b = u8::from_str_radix(s.get(i * 2..i * 2 + 2)?, 16).ok()?;
    }
    Some(out)
}

fn hex8(k: &Keyhash) -> String {
    k.iter().take(4).map(|b| format!("{b:02x}")).collect()
}

/// The secret as an instance is told it.
#[must_use]
pub fn hex_secret(s: &[u8; 32]) -> String {
    s.iter().map(|b| format!("{b:02x}")).collect()
}

async fn answer(stream: &mut TcpStream, code: u16, body: &str) -> std::io::Result<()> {
    answer_bytes(stream, code, body.as_bytes()).await
}

async fn answer_bytes(stream: &mut TcpStream, code: u16, body: &[u8]) -> std::io::Result<()> {
    let head = format!(
        "HTTP/1.1 {code} \r\ncontent-length: {}\r\nconnection: close\r\n\r\n",
        body.len()
    );
    stream.write_all(head.as_bytes()).await?;
    stream.write_all(body).await?;
    stream.flush().await
}
