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
/// The most the listener reads of one request: an envelope's body bound
/// with room for its encoding and the head.
const MAX_REQUEST: usize = 2 * rhtn_node::federation::MAX_BODY + 8192;

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
    std::thread::Builder::new()
        .name("rhtn-federation".into())
        .spawn(move || {
            for stream in l.incoming() {
                let Ok(stream) = stream else { continue };
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
    let conn = rustls::ServerConnection::new(cfg).map_err(|e| e.to_string())?;
    let mut tls = rustls::StreamOwned::new(conn, tcp);
    let request = read_request(&mut tls)?;
    let (status, body) = answer(node, &request);
    let caller = tls
        .conn
        .peer_certificates()
        .and_then(|cs| cs.first())
        .and_then(|c| key_of_spki(c.as_ref()))
        .map(|k| hex(&k))
        .unwrap_or_else(|| "no key".into());
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
fn read_request<S: Read>(s: &mut S) -> Result<Vec<u8>, String> {
    let mut buf = Vec::new();
    let mut chunk = [0u8; 8192];
    let head_end = loop {
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
    if head_end + want > MAX_REQUEST {
        return Err(format!("a request of {want} bytes is over the bound"));
    }
    while buf.len() < head_end + want {
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
fn answer(node: &LiveNode, request: &[u8]) -> (u16, String) {
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
    let outcome = {
        let mut view = node.view.lock().unwrap();
        match view.federation.deliver(&to, e) {
            Ok(o) => o,
            Err(why) => return (409, why),
        }
    };
    if matches!(outcome, Outcome::New | Outcome::Superseded) {
        let mut view = node.view.lock().unwrap();
        let v = &mut *view;
        v.federation.pass(&v.resources);
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
    // close both end the read
    let _ = tls.read_to_end(&mut answer);
    let (status, text) = rhtn_node::federation::response_of(&answer);
    let word = String::from_utf8_lossy(&text).trim().to_string();
    if (200..300).contains(&status) {
        Ok(word)
    } else {
        Err(format!("answered {status}: {word}"))
    }
}
