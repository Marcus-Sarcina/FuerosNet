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
//! interpret them"). It must still know where one ends, because it is
//! reading from a socket — so it asks [`crate::http::extent`] how the
//! head says the body is delimited, reads that much, and hands up what
//! it read.
//!
//! **It does not re-frame, and that is a change of 2026-10-28.** It used
//! to scan the head itself, decode a chunked body and write the head out
//! again with a count — which was the gateway's parse done a second
//! time, in a second place, from a second reading of the same headers.
//! `wire-format.md` §11.2 is about exactly that: two parties must not
//! disagree about where a message ends. So the framing decision is
//! [`crate::http::extent`]'s, the normalising is
//! [`crate::http::serialise_response`]'s, and what is left here is
//! reading bytes off a wire and bounding them.

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

        // **one of three framings, and one reader of which**
        // ([`crate::http::extent`], RFC 9112 §6): a chunked body, a
        // counted one, or one the far end ends by closing.
        match crate::http::extent(&head).map_err(|e| format!("{}: {e:?}", self.addr))? {
            crate::http::Extent::Chunked => {
                // **read to the end of the chunking, and hand the
                // chunking up**: decoding it here and rewriting the head
                // was this file deciding the framing a second time
                let read = self.to_last_chunk(&mut s, rest)?;
                let mut out = head;
                out.extend_from_slice(&read);
                Ok(out)
            }
            crate::http::Extent::Length(want) => {
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
                Ok(out)
            }
            crate::http::Extent::ToClose => {
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
    }
}

impl Relay {
    /// **Read until the chunking ends, and keep every byte of it.**
    ///
    /// A socket gives no end of its own, so the chunk sizes are walked to
    /// find one — that is reading, and it is why this cannot be left to
    /// the parser. **What it does not do is decode**: the chunking is
    /// handed up as it arrived and
    /// [`crate::http::parse_response`] joins it, so the body's extent is
    /// decided in one place rather than agreed in two.
    ///
    /// The cap is counted against the bytes actually read, which is what
    /// an unauthenticated far end can make this node spend.
    fn to_last_chunk(&self, s: &mut TcpStream, mut rest: Vec<u8>) -> Result<Vec<u8>, String> {
        let mut chunk = [0u8; 8192];
        let mut at = 0usize;
        loop {
            // a size line, then that many bytes, then its own CRLF
            let line = loop {
                if let Some(i) = find(&rest[at..], b"\r\n") {
                    break String::from_utf8_lossy(&rest[at..at + i]).to_string();
                }
                if rest.len() > self.cap {
                    return Err(format!("{}: over the node's {} bytes", self.addr, self.cap));
                }
                match s.read(&mut chunk) {
                    Ok(0) => return Err(format!("{}: closed inside a chunked body", self.addr)),
                    Ok(n) => rest.extend_from_slice(&chunk[..n]),
                    Err(e) => return Err(format!("{}: {e}", self.addr)),
                }
            };
            at += line.len() + 2;
            // the size is read to walk the body, not to judge it: an
            // extension or a trailer is the parser's to refuse, and
            // refusing it here would be a second opinion
            let size = usize::from_str_radix(line.split(';').next().unwrap_or("").trim(), 16)
                .map_err(|_| format!("{}: `{line}` is not a chunk size", self.addr))?;
            if size == 0 {
                // the terminal chunk's own CRLF, so the message is whole
                // when the parser sees it
                while rest.len() < at + 2 {
                    match s.read(&mut chunk) {
                        Ok(0) => break,
                        Ok(n) => rest.extend_from_slice(&chunk[..n]),
                        Err(e) => return Err(format!("{}: {e}", self.addr)),
                    }
                }
                rest.truncate((at + 2).min(rest.len()));
                return Ok(rest);
            }
            if at + size + 2 > self.cap {
                return Err(format!("{}: over the node's {} bytes", self.addr, self.cap));
            }
            while rest.len() < at + size + 2 {
                match s.read(&mut chunk) {
                    Ok(0) => return Err(format!("{}: closed inside a chunk", self.addr)),
                    Ok(n) => rest.extend_from_slice(&chunk[..n]),
                    Err(e) => return Err(format!("{}: {e}", self.addr)),
                }
            }
            at += size + 2;
        }
    }
}

fn find(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack.windows(needle.len()).position(|w| w == needle)
}
