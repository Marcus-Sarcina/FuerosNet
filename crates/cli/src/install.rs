//! **Install a package on a node from outside it**, over the one channel
//! an operator reaches their instance on (`rhtn_daemon::administration`,
//! `infra-client-requirements.md` §9.4).
//!
//! **This is the light client's work, as an instrument.** In a product the
//! operator's app makes this act: it holds the signing seed the restricted
//! ssh key is derived from, and §8.3 gives the client the provisioning
//! surface. What is here drives the same routes with the same proof, so an
//! install can be made and watched from a development machine — and so the
//! two are the same procedure rather than two stories.
//!
//! **Nothing here signs anything.** An install is not a protocol act: it
//! writes a hosting entry, relays the package's manifest, and asks the
//! node to ask its own host to start an image. The one piece of key
//! material it touches is the restricted ssh key, derived from the
//! operator's seed and bound by the host's `authorized_keys` to
//! `rhtn-package-runner` and nothing else (`rhtn_crypto::ssh`).

use rhtn_crypto::SigningIdentity;
use std::io::{Read, Write};
use std::net::{SocketAddr, TcpStream, ToSocketAddrs};
use std::path::PathBuf;
use std::time::Duration;

/// **How long an install may take.** The node asks its host to pull an
/// image, which is minutes where it is anything; the surface's own bound
/// is `rhtn_daemon::administration::PACKAGE_TIMEOUT`, and this is that
/// with room for the exchange around it.
const SLOW: Duration = Duration::from_secs(320);

/// How long everything else may take.
const QUICK: Duration = Duration::from_secs(15);

/// What an install was asked for.
#[derive(Default)]
pub struct Ask {
    /// Where the surface answers, as `host:port`.
    pub at: String,
    /// The token acts are proved under, where the act does not arrive on
    /// the loopback. Absent is a caller on the machine itself.
    pub token: Option<[u8; 32]>,
    /// The operator's identity file, which the restricted key is derived
    /// from.
    pub identity: Option<PathBuf>,
    /// The resource the package is hosted as. `rhtn keys mint` makes one.
    pub resource: Option<String>,
    /// Who owns it: whose horizon §11's membership gate is evaluated
    /// against. The node's own operator, ordinarily.
    pub owner: Option<String>,
    /// The authority it is reached under, as its `host` header.
    pub authority: Option<String>,
    /// The image the host pulls.
    pub image: Option<String>,
    /// Where it listens on the node's loopback.
    pub port: Option<u16>,
    /// The package's own manifest, which is relayed rather than composed
    /// here (`infra-client-requirements.md` §9.1).
    pub manifest: Option<PathBuf>,
    /// How many lines of a package's output to read.
    pub lines: Option<usize>,
}

/// Install the package `ask` describes, and say what the node and its
/// host answered.
pub fn install(ask: &Ask) -> Result<String, String> {
    let key = runner_key(ask)?;
    let want = |name: &str, v: &Option<String>| {
        v.clone()
            .ok_or_else(|| format!("`node install` names --{name}"))
    };
    let manifest =
        match &ask.manifest {
            None => return Err(
                "`node install` names --manifest <file>, which is the package's own declaration"
                    .into(),
            ),
            Some(p) => std::fs::read_to_string(p).map_err(|e| format!("{}: {e}", p.display()))?,
        };
    let body = form(&[
        ("resource", &want("resource", &ask.resource)?),
        ("owner", &want("owner", &ask.owner)?),
        ("authority", &want("authority", &ask.authority)?),
        ("image", &want("image", &ask.image)?),
        (
            "port",
            &ask.port.ok_or("`node install` names --port")?.to_string(),
        ),
        ("manifest", &manifest),
        ("key", &key),
    ]);
    act(ask, "/node/package/install", &body, SLOW)
}

/// Remove one.
pub fn remove(ask: &Ask) -> Result<String, String> {
    let key = runner_key(ask)?;
    let resource = ask
        .resource
        .clone()
        .ok_or("`node remove` names --resource")?;
    let body = form(&[("resource", &resource), ("key", &key)]);
    act(ask, "/node/package/remove", &body, SLOW)
}

/// What the node's record says it installed, which needs no key.
pub fn packages(ask: &Ask) -> Result<String, String> {
    let addr = resolve(&ask.at)?;
    let target = proved(ask, "GET", "/node/packages", b"")?;
    let answer = exchange(addr, &format!("GET {target} HTTP/1.1"), b"", QUICK)?;
    said(&answer)
}

/// What the host says is running, or what one package wrote: the runner's
/// own `list` and `logs`, which need the key.
pub fn running(ask: &Ask, logs: bool) -> Result<String, String> {
    let key = runner_key(ask)?;
    let (path, body) = if logs {
        let resource = ask.resource.clone().ok_or("`node logs` names --resource")?;
        let lines = ask.lines.unwrap_or(40).to_string();
        (
            "/node/package/logs",
            form(&[("resource", &resource), ("lines", &lines), ("key", &key)]),
        )
    } else {
        ("/node/package/running", form(&[("key", &key)]))
    };
    act(ask, path, &body, QUICK)
}

/// One act on the surface: proved where it does not arrive on the
/// loopback, and reported whole, since what the host said is the half an
/// operator came to read.
fn act(ask: &Ask, path: &str, body: &[u8], budget: Duration) -> Result<String, String> {
    let addr = resolve(&ask.at)?;
    let target = proved(ask, "POST", path, body)?;
    let answer = exchange(addr, &format!("POST {target} HTTP/1.1"), body, budget)?;
    said(&answer)
}

/// **The restricted key, derived rather than stored**
/// (`rhtn_crypto::ssh::host_runner_key`): the private half, which is what
/// opens the one session the host's `authorized_keys` binds to the
/// runner.
///
/// It is read out of the operator's identity file for this act and is in
/// no file of its own.
fn runner_key(ask: &Ask) -> Result<String, String> {
    let path = ask
        .identity
        .as_ref()
        .ok_or("this act names --identity <file>, which the restricted ssh key is derived from")?;
    let bytes = std::fs::read(path).map_err(|e| format!("{}: {e}", path.display()))?;
    if bytes.len() != crate::keys::IDENTITY_BYTES {
        return Err(format!(
            "{} is {} bytes, not {}",
            path.display(),
            bytes.len(),
            crate::keys::IDENTITY_BYTES
        ));
    }
    let (ed, pq): ([u8; 32], [u8; 32]) = (
        bytes[..32].try_into().unwrap(),
        bytes[32..].try_into().unwrap(),
    );
    let id = SigningIdentity::from_seeds(&ed, &pq);
    Ok(id.host_runner_key().private_key)
}

/// The target an act carries: the path, and a nonce and proof where this
/// caller is not on the instance's own loopback
/// (`rhtn_crypto::enrolment::act_proof`).
fn proved(ask: &Ask, method: &str, path: &str, body: &[u8]) -> Result<String, String> {
    let Some(token) = ask.token else {
        return Ok(path.to_string());
    };
    let nonce = rhtn_transport::tls::random_bytes::<{ rhtn_crypto::enrolment::NONCE_BYTES }>();
    let signed = format!("{path}?nonce={}", hex(&nonce));
    let p = rhtn_crypto::enrolment::act_proof(&token, &nonce, method, &signed, body);
    Ok(format!("{signed}&proof={}", hex(&p)))
}

/// `application/x-www-form-urlencoded`, as the surface reads it.
fn form(fields: &[(&str, &String)]) -> Vec<u8> {
    fields
        .iter()
        .map(|(k, v)| format!("{k}={}", escape(v)))
        .collect::<Vec<_>>()
        .join("&")
        .into_bytes()
}

fn escape(s: &str) -> String {
    s.bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                (b as char).to_string()
            }
            _ => format!("%{b:02X}"),
        })
        .collect()
}

/// The body, or the status and the body where it was a refusal: an
/// install that did not happen is read for why, so the whole of what the
/// node said is carried rather than its first line.
fn said(answer: &str) -> Result<String, String> {
    let code = status(answer);
    let text = body(answer).to_string();
    if code == 200 {
        Ok(text)
    } else {
        Err(format!("{code}\n{text}"))
    }
}

fn exchange(
    addr: SocketAddr,
    line: &str,
    body_bytes: &[u8],
    budget: Duration,
) -> Result<String, String> {
    let mut s =
        TcpStream::connect_timeout(&addr, QUICK).map_err(|e| format!("connect {addr}: {e}"))?;
    s.set_read_timeout(Some(budget)).ok();
    s.set_write_timeout(Some(QUICK)).ok();
    let head = format!(
        "{line}\r\nhost: {addr}\r\ncontent-length: {}\r\nconnection: close\r\n\r\n",
        body_bytes.len()
    );
    s.write_all(head.as_bytes())
        .and_then(|()| s.write_all(body_bytes))
        .and_then(|()| s.flush())
        .map_err(|e| format!("write {addr}: {e}"))?;
    let mut out = String::new();
    s.read_to_string(&mut out)
        .map_err(|e| format!("read {addr}: {e}"))?;
    Ok(out)
}

fn resolve(at: &str) -> Result<SocketAddr, String> {
    if at.is_empty() {
        return Err("this act names --at <host:port>".into());
    }
    at.to_socket_addrs()
        .map_err(|e| format!("{at}: {e}"))?
        .next()
        .ok_or_else(|| format!("{at}: no address"))
}

fn status(response: &str) -> u16 {
    response
        .split_whitespace()
        .nth(1)
        .and_then(|c| c.parse().ok())
        .unwrap_or(0)
}

fn body(response: &str) -> &str {
    match response.find("\r\n\r\n") {
        Some(i) => &response[i + 4..],
        None => response,
    }
}

fn hex(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect()
}
