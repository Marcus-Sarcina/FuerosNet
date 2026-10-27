//! **The directive plane between nodes** (`rhtn_node::federation`;
//! `infra-client-requirements.md` §9.2; `resource-requirements.md` §3.3):
//! HTTPS node to node, instance to instance in meaning [author,
//! 2026-10-26].
//!
//! Two halves. The **listener** takes envelopes for the instances this
//! node hosts from other nodes and offers each into the instance's plane
//! exactly as the administration surface does, so §5's bookkeeping and
//! the relationship check are applied once, here, whatever carried the
//! envelope. The **carrier** takes what the plane holds pending for an
//! instance the operator located ([`rhtn_node::federation::Location`])
//! and delivers it to that node's listener, once per tick until the far
//! node answers that it took it.
//!
//! **Raw public keys, both ways, as the protocol's own transport presents
//! them** (`rhtn_transport::tls`, RFC 7250): the listener presents this
//! node's transport key; the carrier presents its own and **pins the key
//! the operator wrote beside the far instance**, so a listener presenting
//! another key is not delivered to, whatever its address. No certificate
//! authority is involved and none is wanted: the far operator gave the key
//! with the address, as they gave the enrolment token.
//!
//! **Plain HTTP/1.1 inside the TLS**, one request per connection:
//! `PUT /federation/envelope` with `{"to": "<instance hex>", "envelope":
//! {…}}`, answered with the outcome's word. The listener does not read a
//! body beyond the plane's envelope bound, and does not keep a connection.
//!
//! **What is given up** is offline delivery: HTTPS reaches a node that is
//! up. A pending envelope is retried each tick, which federation traffic —
//! operator to operator, minutes being fine — can bear; if a package ever
//! needs the mailbox for it, that is the node-as-payload-endpoint
//! alternative the design keeps open.

use rhtn_archive::Keyhash;
use rhtn_node::federation::{Envelope, Outcome};
use rhtn_node::runtime::LiveNode;
use rhtn_transport::tls::{Presenter, client_config_with, key_of_spki, server_config_with};
use rustls::crypto::aws_lc_rs;
use serde_json::{Value, json};
use std::io::{Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::sync::Arc;
use std::time::Duration;

/// The ALPN the listener and the carrier agree on: HTTP/1.1, not the
/// protocol's own, so a node's QUIC endpoint and this listener are never
/// confused for one another.
pub const ALPN: &[u8] = b"http/1.1";
/// The path an envelope is put to.
pub const PATH: &str = "/federation/envelope";
/// How long either side waits on the other, at each step.
const PATIENCE: Duration = Duration::from_secs(5);
/// **How long one exchange may take in total** [2026-10-27]: the
/// per-read timeout above is not a deadline, because a caller that sends
/// one byte before each timeout makes progress for ever.
const DEADLINE: Duration = Duration::from_secs(30);
/// **How many exchanges this listener will have in flight** [2026-10-27]:
/// a thread each and no cap meant a caller could ask for as many threads
/// as it liked. Over this, a connection is accepted and closed, which
/// says *later* rather than leaving it to hang.
const AT_ONCE: usize = 32;
/// The most the listener reads of one request: an envelope's body bound
/// with room for its encoding and the head.
const MAX_REQUEST: usize = 2 * rhtn_node::federation::MAX_BODY + 8192;
/// The most the carrier reads of one answer, which carries an outcome's
/// word and nothing else.
const MAX_ANSWER: usize = 8192;

fn hex(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect()
}

fn unhex32(s: &str) -> Option<Keyhash> {
    if s.len() != 64 || !s.bytes().all(|c| c.is_ascii_hexdigit()) {
        return None;
    }
    let mut out = [0u8; 32];
    for (i, b) in out.iter_mut().enumerate() {
        *b = u8::from_str_radix(&s[i * 2..i * 2 + 2], 16).ok()?;
    }
    Some(out)
}

/// The server configuration: this node's raw key presented, a raw key
/// required of the caller, HTTP/1.1 agreed.
fn server_config(me: &Presenter) -> Arc<rustls::ServerConfig> {
    let mut cfg = server_config_with(me, vec![aws_lc_rs::kx_group::X25519MLKEM768]);
    cfg.alpn_protocols = vec![ALPN.to_vec()];
    cfg.max_early_data_size = 0;
    Arc::new(cfg)
}

/// The client configuration: this node's raw key presented, any raw key
/// admitted at the handshake and checked against the pin after it.
fn client_config(me: &Presenter) -> Arc<rustls::ClientConfig> {
    let mut cfg = client_config_with(me, vec![aws_lc_rs::kx_group::X25519MLKEM768]);
    cfg.alpn_protocols = vec![ALPN.to_vec()];
    cfg.enable_early_data = false;
    Arc::new(cfg)
}

/// **Open the listener** on `listen` for `node`, presenting `me`, and
/// serve it from a thread of its own for as long as the process lives.
/// The address bound comes back, which a configuration naming port 0
/// needs.
pub fn listen(
    listen: SocketAddr,
    me: &Presenter,
    node: Arc<LiveNode>,
) -> std::io::Result<SocketAddr> {
    let l = TcpListener::bind(listen)?;
    let bound = l.local_addr()?;
    let cfg = server_config(me);
    let in_flight = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    std::thread::Builder::new()
        .name("rhtn-federation".into())
        .spawn(move || {
            for stream in l.incoming() {
                let Ok(stream) = stream else { continue };
                // **a cap on what one listener will carry at once**: the
                // alternative is a thread for every connection a caller
                // cares to open
                if in_flight.load(std::sync::atomic::Ordering::Relaxed) >= AT_ONCE {
                    tracing::warn!(
                        target: "daemon",
                        step = "federation_in",
                        refused = "at once",
                        "daemon.lifecycle"
                    );
                    continue;
                }
                in_flight.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                let counted = in_flight.clone();
                let cfg = cfg.clone();
                let node = node.clone();
                // one thread per exchange: an exchange is one request
                // and short, and a caller that stalls holds only its own
                std::thread::spawn(move || {
                    let from = stream.peer_addr().ok();
                    let (status, body) = match exchange(stream, cfg, &node) {
                        Ok(answer) => answer,
                        Err(why) => (400, why),
                    };
                    // the operator's line, as for a run taken: what came
                    // in, for whom, from which key
                    if status == 200 {
                        crate::tell!("rhtnd: federation: {body}");
                    } else {
                        crate::say!("rhtnd: federation: {status}: {body}");
                    }
                    tracing::info!(
                        target: "daemon",
                        step = "federation_in",
                        from = ?from,
                        status,
                        said = %body.trim_end(),
                        "daemon.lifecycle"
                    );
                    counted.fetch_sub(1, std::sync::atomic::Ordering::Relaxed);
                });
            }
        })?;
    Ok(bound)
}

/// One request from another node: the handshake, the request read whole,
/// the envelope offered, the outcome written back.
fn exchange(
    tcp: TcpStream,
    cfg: Arc<rustls::ServerConfig>,
    node: &LiveNode,
) -> Result<(u16, String), String> {
    tcp.set_read_timeout(Some(PATIENCE))
        .map_err(|e| e.to_string())?;
    tcp.set_write_timeout(Some(PATIENCE))
        .map_err(|e| e.to_string())?;
    let by = std::time::Instant::now() + DEADLINE;
    let conn = rustls::ServerConnection::new(cfg).map_err(|e| e.to_string())?;
    let mut tls = rustls::StreamOwned::new(conn, tcp);
    let request = read_request(&mut tls, by)?;
    // **the caller is established before anything it sent is acted on**
    // [2026-10-27]: this key used to be read after the envelope had
    // already been offered, which meant it was in the log and not in the
    // decision
    let key = tls
        .conn
        .peer_certificates()
        .and_then(|cs| cs.first())
        .and_then(|c| key_of_spki(c.as_ref()));
    let (status, body) = answer(node, &request, key);
    let caller = key.map(|k| hex(&k)).unwrap_or_else(|| "no key".into());
    let text = format!("{body}\n");
    let _ = tls.write_all(
        format!(
            "HTTP/1.1 {status} {}\r\ncontent-type: text/plain\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{text}",
            if status == 200 { "OK" } else { "Refused" },
            text.len()
        )
        .as_bytes(),
    );
    let _ = tls.flush();
    tls.conn.send_close_notify();
    let _ = tls.flush();
    Ok((status, format!("{body} (caller key {caller})")))
}

/// Read one HTTP/1.1 request whole: the head, then `content-length`
/// bytes, within the bound.
fn read_request<S: Read>(s: &mut S, by: std::time::Instant) -> Result<Vec<u8>, String> {
    let mut buf = Vec::new();
    let mut chunk = [0u8; 8192];
    let head_end = loop {
        if std::time::Instant::now() > by {
            return Err("the exchange took longer than this listener waits".into());
        }
        if let Some(i) = buf.windows(4).position(|w| w == b"\r\n\r\n") {
            break i + 4;
        }
        if buf.len() > 32 * 1024 {
            return Err("a request head over 32 KiB".into());
        }
        match s.read(&mut chunk) {
            Ok(0) => return Err("closed before a request head".into()),
            Ok(n) => buf.extend_from_slice(&chunk[..n]),
            Err(e) => return Err(e.to_string()),
        }
    };
    let head = String::from_utf8_lossy(&buf[..head_end]).to_string();
    let want: usize = head
        .lines()
        .find_map(|l| {
            let (n, v) = l.split_once(':')?;
            n.trim()
                .eq_ignore_ascii_case("content-length")
                .then(|| v.trim().parse().ok())
                .flatten()
        })
        .unwrap_or(0);
    // **checked, because the length is the caller's** [2026-10-27]: added
    // to the head's size it can wrap, and a wrapped sum passes a bound
    // the real size does not
    let whole = head_end
        .checked_add(want)
        .ok_or("a content length that does not fit an address")?;
    if whole > MAX_REQUEST {
        return Err(format!("a request of {want} bytes is over the bound"));
    }
    while buf.len() < whole {
        if std::time::Instant::now() > by {
            return Err("the exchange took longer than this listener waits".into());
        }
        match s.read(&mut chunk) {
            Ok(0) => return Err("closed before the body ended".into()),
            Ok(n) => buf.extend_from_slice(&chunk[..n]),
            Err(e) => return Err(e.to_string()),
        }
    }
    buf.truncate(head_end + want);
    Ok(buf)
}

/// The listener's one route.
///
/// **A claimed origin is not an authenticated sender** [2026-10-27]. The
/// envelope says which instance it comes from; what the handshake
/// establishes is which *node* is speaking. The two are bound here, by
/// the key the operator wrote beside that instance
/// (`rhtn_node::federation::Location`), **before the plane is touched** —
/// because `Plane::offer` moves a record's revision watermark, so an
/// envelope accepted on a caller's word alone could take revision
/// `u64::MAX` and leave every legitimate revision after it classified as
/// older and never delivered. A signature the application would later
/// reject does not undo that.
///
/// So an operator expecting envelopes from an instance configures where
/// that instance's node is, which is the same `[host.at]` entry they
/// already write to send to it. An envelope arriving for an instance with
/// no such entry is refused with that said: the operator's own surface
/// (`PUT /node/resource/<hex>/envelope`) remains the way to hand one over
/// by hand, where reaching the port is the authorisation (§8.2).
fn answer(node: &LiveNode, request: &[u8], caller: Option<[u8; 32]>) -> (u16, String) {
    let req = match rhtn_node::http::parse(request) {
        Ok(r) => r,
        Err(e) => return (400, format!("not a request this listener reads: {e:?}")),
    };
    if req.method != "PUT" && req.method != "POST" {
        return (405, format!("this listener answers PUT {PATH}"));
    }
    if req.target != PATH {
        return (404, format!("this listener answers PUT {PATH}"));
    }
    let v: Value = match serde_json::from_slice(&req.body) {
        Ok(v) => v,
        Err(e) => return (400, format!("the body is not JSON: {e}")),
    };
    let Some(to) = v.get("to").and_then(Value::as_str).and_then(unhex32) else {
        return (400, "`to` is missing or not 64 hex digits".into());
    };
    let e = match v.get("envelope").map(Envelope::from_json) {
        Some(Ok(e)) => e,
        Some(Err(why)) => return (400, format!("not an envelope: {why}")),
        None => return (400, "`envelope` is missing".into()),
    };
    let (record, origin8) = (e.record.clone(), rhtn_node::diag::id8(&e.origin));
    let configured = node.view.lock().unwrap().federation.location_of(&e.origin);
    match (configured.map(|l| l.key), caller) {
        (Some(expected), Some(presented)) if expected == presented => {}
        (Some(_), Some(presented)) => {
            return (
                403,
                format!(
                    "the caller presented key {} and the node configured for {origin8} presents \
                     another: nothing was offered",
                    hex(&presented)
                ),
            );
        }
        (Some(_), None) => {
            return (
                403,
                "the caller presented no raw public key: nothing was offered".into(),
            );
        }
        (None, _) => {
            return (
                403,
                format!(
                    "no node is configured for {origin8}, the instance this envelope claims as \
                     its origin, so nothing authenticates the claim: write its node's listener \
                     and key beside the relationship, or hand the envelope over the operator's \
                     own surface"
                ),
            );
        }
    }
    let outcome = {
        let mut view = node.view.lock().unwrap();
        match view.federation.deliver(&to, e) {
            Ok(o) => o,
            Err(why) => return (409, why),
        }
    };
    if matches!(
        outcome,
        Outcome::New | Outcome::Superseded | Outcome::Requeued
    ) {
        rounds(node);
    }
    let status = match &outcome {
        Outcome::Conflict => 409,
        Outcome::Refused(_) => 400,
        _ => 200,
    };
    (
        status,
        format!(
            "{outcome:?}: `{record}` from {origin8} for {}",
            rhtn_node::diag::id8(&to)
        ),
    )
}

/// **How many rounds of the plane one call runs**: a bound, because each
/// round asks every hosted instance something and an instance that
/// always has more to say must not hold the caller open.
pub const ROUNDS: usize = 32;

/// **The plane's local rounds: plan, perform, commit** — and **never a
/// backend call while the node's own state is locked** [2026-10-27].
///
/// A pass used to make the backend calls itself, under `node.view`, so a
/// slow socket service or an expensive confined package stopped every
/// other thing the node was doing for as long as it took to answer, and
/// an incoming delivery could trigger a pass over every hosted instance
/// at once. Each round now plans under the lock, performs its exchanges
/// holding nothing, and commits under it again; one delivery an instance
/// a round keeps a slow instance to its own round rather than everyone's.
pub fn rounds(node: &LiveNode) -> rhtn_node::federation::Passed {
    use rhtn_node::federation::{Answer, Passed};
    let mut all = Passed::default();
    for _ in 0..ROUNDS {
        let work = {
            let mut view = node.view.lock().unwrap();
            let v = &mut *view;
            v.federation.work(&v.resources)
        };
        if work.is_empty() {
            break;
        }
        let answers: Vec<Answer> = work
            .into_iter()
            .map(|e| {
                let answer = e
                    .backend
                    .handle(&e.message)
                    .map(|bytes| rhtn_node::federation::response_of(&bytes));
                Answer {
                    exchange: e,
                    answer,
                }
            })
            .collect();
        let round = {
            let mut view = node.view.lock().unwrap();
            view.federation.done(answers)
        };
        // **telling an instance its relationships is something moving**:
        // counted as nothing, the first round of a fresh plane looked
        // quiet and the loop stopped before anything was asked of it
        let quiet = round.told + round.delivered + round.declined + round.emitted + round.carried
            == 0
            && round.refused.is_empty();
        all.told += round.told;
        all.delivered += round.delivered;
        all.declined += round.declined;
        all.emitted += round.emitted;
        all.carried += round.carried;
        all.refused.extend(round.refused);
        // nothing moved: the instances have said all they have to say,
        // and asking again would only poll their outboxes
        if quiet {
            break;
        }
    }
    all
}

/// **Carry what is pending for located instances** to their nodes, once
/// each; what the far node took is no longer pending. Returns how many
/// were taken and the reasons the rest were not, for the log.
pub fn carry(node: &LiveNode, me: &Presenter) -> (usize, Vec<String>) {
    let abroad = node.view.lock().unwrap().federation.abroad();
    if abroad.is_empty() {
        return (0, Vec::new());
    }
    let cfg = client_config(me);
    let (mut taken, mut failed) = (0, Vec::new());
    for a in abroad {
        match deliver(&cfg, &a) {
            Ok(word) => {
                taken += 1;
                node.view
                    .lock()
                    .unwrap()
                    .federation
                    .carried(&a.from, &a.to, &a.envelope);
                crate::tell!(
                    "rhtnd: federation: carried `{}` to {} at {}: {word}",
                    a.envelope.record,
                    rhtn_node::diag::id8(&a.to),
                    a.at.address
                );
                tracing::info!(
                    target: "daemon",
                    step = "federation_out",
                    to8 = %rhtn_node::diag::id8(&a.to),
                    at = %a.at.address,
                    outcome = %word,
                    "daemon.lifecycle"
                );
            }
            Err(why) => failed.push(format!(
                "{} at {}: {why}",
                rhtn_node::diag::id8(&a.to),
                a.at.address
            )),
        }
    }
    (taken, failed)
}

/// One delivery: connect, shake hands, check the pin, put, read the word.
fn deliver(
    cfg: &Arc<rustls::ClientConfig>,
    a: &rhtn_node::federation::Abroad,
) -> Result<String, String> {
    let tcp = TcpStream::connect_timeout(&a.at.address, PATIENCE).map_err(|e| e.to_string())?;
    tcp.set_read_timeout(Some(PATIENCE))
        .map_err(|e| e.to_string())?;
    tcp.set_write_timeout(Some(PATIENCE))
        .map_err(|e| e.to_string())?;
    let name = rustls_pki_types::ServerName::IpAddress(a.at.address.ip().into());
    let conn = rustls::ClientConnection::new(cfg.clone(), name).map_err(|e| e.to_string())?;
    let mut tls = rustls::StreamOwned::new(conn, tcp);
    // the handshake completes on the first write; the pin is checked
    // before anything that matters is written, which is the request
    tls.flush().map_err(|e| format!("handshake: {e}"))?;
    while tls.conn.is_handshaking() {
        tls.conn
            .complete_io(&mut tls.sock)
            .map_err(|e| format!("handshake: {e}"))?;
    }
    let presented = tls
        .conn
        .peer_certificates()
        .and_then(|cs| cs.first())
        .and_then(|c| key_of_spki(c.as_ref()))
        .ok_or("the far node presented no raw public key")?;
    if presented != a.at.key {
        tls.conn.send_close_notify();
        return Err(format!(
            "the node there presented key {}, not the one configured",
            hex(&presented)
        ));
    }
    let body = json!({"to": hex(&a.to), "envelope": a.envelope.to_json()}).to_string();
    let request = format!(
        "PUT {PATH} HTTP/1.1\r\nhost: {}\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}",
        a.at.address,
        body.len()
    );
    tls.write_all(request.as_bytes())
        .map_err(|e| e.to_string())?;
    tls.flush().map_err(|e| e.to_string())?;
    let mut answer = Vec::new();
    // the far end closes after its answer; a close_notify or a plain
    // close both end the read. **Bounded** [2026-10-27]: what comes back
    // is one word, and reading to the end of whatever a far node cares
    // to send is not a bound at all
    let _ = Read::take(&mut tls, MAX_ANSWER as u64).read_to_end(&mut answer);
    let (status, text) = rhtn_node::federation::response_of(&answer);
    let word = String::from_utf8_lossy(&text).trim().to_string();
    if (200..300).contains(&status) {
        Ok(word)
    } else {
        Err(format!("answered {status}: {word}"))
    }
}
