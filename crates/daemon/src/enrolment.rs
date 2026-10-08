//! **The enrolment listener**: how an instance's operator learns the public
//! half of the transport key it minted, and hands back the run signed over
//! it (`infra-client-requirements.md` §7; design §23.3).
//!
//! **Out of band, and not protocol.** §8.2 settles that administration is
//! reached "the way any other server is reached", and that no request type
//! carries a command and none is owed one. So this is a surface of its own
//! on a port of its own, speaking HTTP/1.1 through the strict parser the
//! gateway already uses ([`rhtn_node::http`]), and nothing here appears on
//! the wire the protocol defines.
//!
//! **It lives only until the run is in force.** An administration surface
//! outliving the one thing it was for would be a standing way in for no
//! further purpose, so [`crate::service::Service::start`] drops it the
//! moment a credential is in force.
//!
//! ## The two legs, and why only one of them needs a secret
//!
//! **Handing a run in needs no authority of its own.** A run is signed by
//! the operator, and [`Credential::add`] verifies that it is the
//! operator's, over *this* transport key, and a delegation — so bytes from
//! any caller are either the operator's run or refused, and the listener
//! can take them from anybody.
//!
//! **Handing the public half out does.** An on-path answer carrying an
//! attacker's key would have the operator's client sign a run over it, and
//! the attacker would then serve as the operator's node. The only thing a
//! client can share with a host that does not exist yet is what it wrote
//! into the instance's configuration, so enrolment carries a **one-time
//! token** there — and the token is used as a MAC key and never sent: the
//! client chooses a nonce, and the answer carries the key with
//! `HMAC-SHA256(token, info || nonce || key)` beside it.
//!
//! **The token is not the transport key and must not be mistaken for one.**
//! Its whole power is to answer one fetch inside one window. The transport
//! private key never leaves the instance, which is what design §23.3 is for
//! and what keeps the credential §8.2 warns "can destroy the instance and
//! bill its owner" clear of the instance's identity.
//!
//! **The cryptography is here rather than in `rhtn-crypto`** on purpose:
//! that crate carries what the wire fixes, and this is administration which
//! no document gives an encoding. A second implementation of the node owes
//! nothing to the shape below.

use aws_lc_rs::hmac;
use rhtn_crypto::Identity;
use rhtn_node::http::{self, Reject};
use rhtn_transport::tls::Credential;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

/// The domain separation over the proof: a tag of this project's own, so
/// the token cannot be made to authenticate anything else that happens to
/// hash the same bytes.
pub const PROOF_INFO: &[u8] = b"rhtn/1:enrolment-proof";

/// The most one enrolment request may carry. A run's delegations are a few
/// hundred bytes each; this is room for a generous batch and no more, so a
/// caller cannot spend the instance's memory before it serves anything.
pub const MAX_REQUEST: usize = 64 * 1024;

/// How long one caller may hold the listener. Enrolment is a fetch and a
/// push; a connection that takes longer than this is not enrolling.
pub const CALLER_TIMEOUT: Duration = Duration::from_secs(10);

/// The nonce a fetch must carry, in bytes. Sixteen: the client chooses it
/// per fetch, and the proof is worthless for any other.
pub const NONCE_BYTES: usize = 16;

/// The proof an instance returns beside its transport public key:
/// `HMAC-SHA256` under the enrolment token over [`PROOF_INFO`], the
/// caller's nonce, and the key.
///
/// **The nonce is what makes it unreplayable** and the key is what it is
/// about: a proof over one key does not carry to another, which is the
/// substitution the token is here to stop.
pub fn proof(token: &[u8; 32], nonce: &[u8], key: &[u8; 32]) -> [u8; 32] {
    let mut ctx = hmac::Context::with_key(&hmac::Key::new(hmac::HMAC_SHA256, token));
    ctx.update(PROOF_INFO);
    ctx.update(nonce);
    ctx.update(key);
    let mut out = [0u8; 32];
    out.copy_from_slice(ctx.sign().as_ref());
    out
}

/// An instance offering enrolment: what it needs to answer a fetch and to
/// take a run.
pub struct Enrolling {
    /// Where it listens, from the configuration's `[enrolment]`.
    pub listen: SocketAddr,
    /// The one-time token, used as a MAC key and never sent.
    pub token: [u8; 32],
    /// The transport credential, for its public half and to verify a run
    /// into.
    pub credential: Arc<Credential>,
    /// The operator whose signature a run must carry.
    pub operator: Identity,
    /// Where a taken run is written, so it survives a restart: the same
    /// directory `Service::start`'s loop reads.
    pub delegations: PathBuf,
}

impl Enrolling {
    /// Serve until the caller drops this future.
    ///
    /// **One caller at a time.** Enrolment is one short exchange and the
    /// instance is serving nothing else yet, so there is nothing to gain
    /// from concurrency and a bound on it is one fewer thing to spend.
    pub async fn serve(self) {
        let listener = match TcpListener::bind(self.listen).await {
            Ok(l) => l,
            Err(e) => {
                crate::say!("rhtnd: enrolment: {}: {e}", self.listen);
                return;
            }
        };
        // **the address it actually bound**, which an operator needs when
        // the configuration named port 0, as the node's own line does
        let bound = listener.local_addr().unwrap_or(self.listen);
        tracing::info!(
            target: "daemon",
            step = "enrolment_open",
            listen = %bound,
            "daemon.lifecycle"
        );
        crate::say!(
            "rhtnd: enrolment on {bound}: fetch /enrolment?nonce=<32 hex> for the transport key, \
             PUT the signed run to /enrolment/run"
        );
        loop {
            let Ok((stream, from)) = listener.accept().await else {
                continue;
            };
            // a caller that stalls holds nothing: the exchange is bounded
            // and the next one is waiting
            let _ = tokio::time::timeout(CALLER_TIMEOUT, self.exchange(stream, from)).await;
        }
        // unreachable: the listener runs until it is dropped, which is what
        // a credential in force does to it
    }

    /// One connection: read a request, answer it, close.
    async fn exchange(&self, mut stream: TcpStream, from: SocketAddr) {
        let mut buf = Vec::new();
        let req = loop {
            let mut chunk = [0u8; 4096];
            match stream.read(&mut chunk).await {
                Ok(0) => return,
                Ok(n) => buf.extend_from_slice(&chunk[..n]),
                Err(_) => return,
            }
            if buf.len() > MAX_REQUEST {
                let _ = answer(
                    &mut stream,
                    413,
                    "the request is larger than enrolment takes\n",
                )
                .await;
                return;
            }
            match http::parse(&buf) {
                Ok(r) => break r,
                // the message has not all arrived: these two are the
                // parser saying so, and everything else is a refusal
                Err(Reject::Malformed("no end of headers"))
                | Err(Reject::Malformed("body short")) => {
                    continue;
                }
                Err(e) => {
                    tracing::warn!(
                        target: "daemon",
                        step = "enrolment_refused",
                        from = %from,
                        why = ?e,
                        "daemon.lifecycle"
                    );
                    let _ = answer(&mut stream, 400, "the request does not read\n").await;
                    return;
                }
            }
        };
        let (code, body) = self.answer_to(&req);
        tracing::info!(
            target: "daemon",
            step = "enrolment_request",
            from = %from,
            method = %req.method,
            target = %req.target,
            code,
            "daemon.lifecycle"
        );
        let _ = answer(&mut stream, code, &body).await;
    }

    /// What a request comes to: the status and the body, both decided
    /// without touching the socket so a test can hold this alone.
    pub fn answer_to(&self, req: &http::Request) -> (u16, String) {
        let (path, query) = match req.target.split_once('?') {
            Some((p, q)) => (p, Some(q)),
            None => (req.target.as_str(), None),
        };
        match (req.method.as_str(), path) {
            ("GET", "/enrolment") => self.fetch(query),
            ("PUT", "/enrolment/run") | ("POST", "/enrolment/run") => self.take_run(&req.body),
            ("GET", _) | ("PUT", _) | ("POST", _) => (
                404,
                "enrolment answers GET /enrolment and PUT /enrolment/run\n".into(),
            ),
            _ => (405, "enrolment answers GET and PUT\n".into()),
        }
    }

    /// The public half, with the proof that this instance holds the token.
    fn fetch(&self, query: Option<&str>) -> (u16, String) {
        let nonce = query
            .and_then(|q| {
                q.split('&')
                    .find_map(|kv| kv.strip_prefix("nonce="))
                    .map(str::to_string)
            })
            .and_then(|h| unhex(&h));
        let Some(nonce) = nonce.filter(|n| n.len() == NONCE_BYTES) else {
            return (
                400,
                format!("a fetch carries `nonce=` of {NONCE_BYTES} bytes in hex\n"),
            );
        };
        let key = self.credential.public();
        let remaining = self.credential.issued().len();
        (
            200,
            format!(
                "transport {}\nproof {}\ncredentials {}\n",
                hex(&key),
                hex(&proof(&self.token, &nonce, &key)),
                remaining
            ),
        )
    }

    /// A run, verified and kept.
    ///
    /// **Verified at the door rather than on the next read.** `read_run`
    /// already reports and skips what it cannot take, so writing first
    /// would work — but then a caller could fill the instance's disk with
    /// bytes nobody signed, and the operator would learn of it from a log
    /// rather than from the refusal they are standing in front of.
    fn take_run(&self, body: &[u8]) -> (u16, String) {
        if body.is_empty() {
            return (
                400,
                "a run is the delegation's bytes, and this was empty\n".into(),
            );
        }
        let d = match self
            .credential
            .add(std::slice::from_ref(&self.operator), body)
        {
            Ok(d) => d,
            Err(e) => return (400, format!("not taken into the run: {e}\n")),
        };
        if let Err(e) = std::fs::create_dir_all(&self.delegations) {
            return (500, format!("{}: {e}\n", self.delegations.display()));
        }
        // named by the window it opens, so a run arriving in pieces lands
        // as its own files and a credential re-sent overwrites itself
        let at = self.delegations.join(format!("run-{}", d.not_before));
        if let Err(e) = std::fs::write(&at, body) {
            return (500, format!("{}: {e}\n", at.display()));
        }
        let held = self.credential.issued().len();
        tracing::info!(
            target: "daemon",
            step = "enrolment_run",
            not_before = d.not_before,
            not_after = d.not_after,
            held,
            "daemon.lifecycle"
        );
        (
            200,
            format!(
                "taken {}..{}\ncredentials {held}\n",
                d.not_before, d.not_after
            ),
        )
    }
}

/// One response and no keep-alive: enrolment is one exchange a connection,
/// which is also what the parser admits (`Reject::MoreThanOne`).
async fn answer(stream: &mut TcpStream, code: u16, body: &str) -> std::io::Result<()> {
    let reason = match code {
        200 => "OK",
        400 => "Bad Request",
        404 => "Not Found",
        405 => "Method Not Allowed",
        413 => "Content Too Large",
        _ => "Internal Server Error",
    };
    let head = format!(
        "HTTP/1.1 {code} {reason}\r\n\
         content-type: text/plain; charset=utf-8\r\n\
         content-length: {}\r\n\
         connection: close\r\n\r\n",
        body.len()
    );
    stream.write_all(head.as_bytes()).await?;
    stream.write_all(body.as_bytes()).await?;
    stream.flush().await
}

fn hex(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect()
}

fn unhex(s: &str) -> Option<Vec<u8>> {
    if !s.len().is_multiple_of(2) || s.is_empty() {
        return None;
    }
    (0..s.len() / 2)
        .map(|i| u8::from_str_radix(&s[i * 2..i * 2 + 2], 16).ok())
        .collect()
}
