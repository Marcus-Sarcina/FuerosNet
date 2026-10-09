//! **The second leg: the node carrying a request to a resource that holds
//! its own port.**
//!
//! `resource-requirements.md` §3 has two legs and only this one speaks
//! HTTP on the wire: "a local socket for a package hosted on the node;
//! **HTTPS, required**, where it crosses a network", carrying "ordinary
//! HTTP". §1 is what the arrangement is — "the infra node is the front
//! door; the resource is behind it … a reverse proxy where the node
//! carries the resource's traffic" — and this is the proxy's far side.
//!
//! **A resource here is an ordinary program.** It listens on a port and
//! serves its own users whatever it serves them; what reaches it through
//! this relay is a request the node has already authenticated and
//! authorised, carrying the credential of §2 and nothing of the network
//! besides. It reads that credential exactly as a SaaS product reads a
//! corporate gateway's headers, which is the whole benefit §1 claims for
//! the shape.
//!
//! **What the node does not do is interpret the answer** (§3:
//! "responses are ordinary HTTP. The node relays them; it does not
//! interpret them"). It must still know where one ends, so the framing is
//! read and, where a chunked body's framing would otherwise reach the
//! caller as a lie, rewritten — the same discipline §3.1 states for the
//! request, where the node "parses the HTTP message and re-serialises it".

use crate::resources::Backend;
use std::io::{Read, Write};
use std::net::{SocketAddr, TcpStream};
use std::time::Duration;

/// A response head longer than this is not one the node will carry: a
/// header block this size is a fault at the far end, and reading further
/// to find out is work an unauthenticated far end can ask for.
const MAX_HEAD: usize = 32 * 1024;

/// How long the node waits on the far end, at each step.
const PATIENCE: Duration = Duration::from_secs(5);

/// **A resource the node reaches over a socket**, as against one it runs
/// inside its own sandbox.
///
/// Which of the two a resource is "follows from where the resource runs,
/// so nothing needs declaring" (`infra-client-requirements.md` §10.6) —
/// here the operator's configuration names an address, and that is what
/// says it.
pub struct Relay {
    addr: SocketAddr,
    /// Bytes of response the node will carry back, matching what it
    /// carries for a sandboxed package (`rhtn_resources::Limits`): the two
    /// backends must agree, or the same resource answers differently for
    /// having been packaged differently.
    cap: usize,
}

impl Relay {
    /// Reach the resource at `addr`, carrying at most `cap` bytes back.
    pub fn to(addr: SocketAddr, cap: usize) -> Self {
        Relay { addr, cap }
    }

    /// Where it points, for the operator's page.
    pub fn addr(&self) -> SocketAddr {
        self.addr
    }
}

impl Backend for Relay {
    /// **Always, because asking costs a connection.** A probe here would
    /// open and drop a socket before every request to learn what the
    /// request itself is about to find out, and the gateway answers the
    /// caller the same way either road: a refusal from `handle` and a
    /// backend that is down are one status code on the wire.
    fn running(&self) -> bool {
        true
    }

    fn handle(&self, message: &[u8]) -> Result<Vec<u8>, String> {
        let mut s = TcpStream::connect_timeout(&self.addr, PATIENCE)
            .map_err(|e| format!("{}: {e}", self.addr))?;
        s.set_read_timeout(Some(PATIENCE))
            .map_err(|e| e.to_string())?;
        s.set_write_timeout(Some(PATIENCE))
            .map_err(|e| e.to_string())?;
        let _ = s.set_nodelay(true);
        s.write_all(message).map_err(|e| e.to_string())?;
        s.flush().map_err(|e| e.to_string())?;

        // the head first, because the framing is in it
        let mut buf = Vec::new();
        let mut chunk = [0u8; 8192];
        let head_end = loop {
            if let Some(i) = find(&buf, b"\r\n\r\n") {
                break i + 4;
            }
            if buf.len() > MAX_HEAD {
                return Err(format!(
                    "{}: a response head over {MAX_HEAD} bytes",
                    self.addr
                ));
            }
            match s.read(&mut chunk) {
                Ok(0) => return Err(format!("{}: closed before a response head", self.addr)),
                Ok(n) => buf.extend_from_slice(&chunk[..n]),
                Err(e) => return Err(format!("{}: {e}", self.addr)),
            }
        };
        let head = buf[..head_end].to_vec();
        let mut rest = buf[head_end..].to_vec();
        if !head.starts_with(b"HTTP/1.") {
            return Err(format!("{}: not an HTTP/1 response", self.addr));
        }

        // **one of three framings, and the response says which**
        // (RFC 9112 §6): a chunked body, a counted one, or one the far end
        // ends by closing.
        if let Some(te) = header(&head, b"transfer-encoding") {
            if !te.eq_ignore_ascii_case("chunked") {
                return Err(format!("{}: `transfer-encoding: {te}`", self.addr));
            }
            let body = self.dechunk(&mut s, rest)?;
            return Ok(reframed(&head, &body));
        }
        if let Some(len) = header(&head, b"content-length") {
            let want: usize = len
                .trim()
                .parse()
                .map_err(|_| format!("{}: `content-length: {len}`", self.addr))?;
            if want > self.cap {
                return Err(format!(
                    "{}: {want} bytes is over the node's {}",
                    self.addr, self.cap
                ));
            }
            while rest.len() < want {
                match s.read(&mut chunk) {
                    Ok(0) => {
                        return Err(format!(
                            "{}: closed {} bytes short",
                            self.addr,
                            want - rest.len()
                        ));
                    }
                    Ok(n) => rest.extend_from_slice(&chunk[..n]),
                    Err(e) => return Err(format!("{}: {e}", self.addr)),
                }
            }
            rest.truncate(want);
            let mut out = head;
            out.extend_from_slice(&rest);
            return Ok(out);
        }
        // nothing framed it, so the close does
        loop {
            if rest.len() > self.cap {
                return Err(format!("{}: over the node's {} bytes", self.addr, self.cap));
            }
            match s.read(&mut chunk) {
                Ok(0) => break,
                Ok(n) => rest.extend_from_slice(&chunk[..n]),
                Err(e) => return Err(format!("{}: {e}", self.addr)),
            }
        }
        let mut out = head;
        out.extend_from_slice(&rest);
        Ok(out)
    }
}

impl Relay {
    /// Read a chunked body into its bytes.
    ///
    /// **It is decoded rather than passed on** because the node answers on
    /// a transport that carries one message and no trailer: a caller
    /// handed `transfer-encoding: chunked` over that wire would be handed
    /// a framing that does not describe what arrived.
    fn dechunk(&self, s: &mut TcpStream, mut rest: Vec<u8>) -> Result<Vec<u8>, String> {
        let mut body = Vec::new();
        let mut chunk = [0u8; 8192];
        let mut at = 0usize;
        loop {
            // a size line, then that many bytes, then its own CRLF
            let line = loop {
                if let Some(i) = find(&rest[at..], b"\r\n") {
                    break String::from_utf8_lossy(&rest[at..at + i]).to_string();
                }
                match s.read(&mut chunk) {
                    Ok(0) => return Err(format!("{}: closed inside a chunked body", self.addr)),
                    Ok(n) => rest.extend_from_slice(&chunk[..n]),
                    Err(e) => return Err(format!("{}: {e}", self.addr)),
                }
            };
            at += line.len() + 2;
            let size = usize::from_str_radix(line.split(';').next().unwrap_or("").trim(), 16)
                .map_err(|_| format!("{}: `{line}` is not a chunk size", self.addr))?;
            if size == 0 {
                return Ok(body);
            }
            if body.len() + size > self.cap {
                return Err(format!("{}: over the node's {} bytes", self.addr, self.cap));
            }
            while rest.len() < at + size + 2 {
                match s.read(&mut chunk) {
                    Ok(0) => return Err(format!("{}: closed inside a chunk", self.addr)),
                    Ok(n) => rest.extend_from_slice(&chunk[..n]),
                    Err(e) => return Err(format!("{}: {e}", self.addr)),
                }
            }
            body.extend_from_slice(&rest[at..at + size]);
            at += size + 2;
        }
    }
}

/// The head again, with the framing it now has: the chunked header goes,
/// a count takes its place, and nothing else of the far end's answer is
/// touched.
fn reframed(head: &[u8], body: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    for line in head.split(|b| *b == b'\n') {
        let l = line.strip_suffix(b"\r").unwrap_or(line);
        if l.is_empty() {
            continue;
        }
        let lower = l.to_ascii_lowercase();
        if lower.starts_with(b"transfer-encoding:") || lower.starts_with(b"content-length:") {
            continue;
        }
        out.extend_from_slice(l);
        out.extend_from_slice(b"\r\n");
    }
    out.extend_from_slice(format!("content-length: {}\r\n\r\n", body.len()).as_bytes());
    out.extend_from_slice(body);
    out
}

/// One header's value from a response head, case-insensitively, or nothing.
fn header(head: &[u8], name: &[u8]) -> Option<String> {
    for line in head.split(|b| *b == b'\n') {
        let l = line.strip_suffix(b"\r").unwrap_or(line);
        let Some(i) = l.iter().position(|b| *b == b':') else {
            continue;
        };
        if l[..i].to_ascii_lowercase() == name {
            return Some(String::from_utf8_lossy(&l[i + 1..]).trim().to_string());
        }
    }
    None
}

fn find(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack.windows(needle.len()).position(|w| w == needle)
}
