//! **The operator's side of a node's administration surface**
//! (`rhtn_daemon::administration`): fetch the transport key an instance
//! minted, check the proof that it is that instance's, sign a run over it,
//! and hand back the run and the two records §4.4 says an instance cannot
//! sign for itself.
//!
//! **This is the provisioning page's work, as an instrument.** In a product
//! it is the light client that does this — `infra-client-requirements.md`
//! §8.3 has the client ship the provisioning pages, because they must work
//! before any node exists — and the page is not written yet. What is here
//! drives the same surface over the same routes, so a node can be stood up
//! and signed on a development machine.
//!
//! **It is the one thing in this binary that changes a state.** Everything
//! else `rhtn` does is read-only by §9.2's class; this crosses no protocol
//! request at all, being out-of-band administration (§8.2), and what it
//! hands over carries the operator's own signature.

use rhtn_crypto::SigningIdentity;
use rhtn_crypto::delegation::issue_run;
use rhtn_node::resolution::{NetworkPoint, anchor_entry, endpoint_record};
use std::io::{Read, Write};
use std::net::{SocketAddr, TcpStream, ToSocketAddrs};
use std::time::Duration;

/// How long one exchange with the surface may take.
const TIMEOUT: Duration = Duration::from_secs(15);

/// What `enrol` was asked to do.
pub struct Ask {
    /// Where the surface answers, as `host:port`.
    pub at: String,
    /// The token the configuration carried, 32 bytes of hex.
    pub token: [u8; 32],
    /// How many credentials the run carries. Each is 48 hours
    /// (`wire-format.md` §8.2), so the default is a fortnight.
    pub credentials: usize,
    /// The address to name in an endpoint record, where one is wanted.
    pub endpoint: Option<SocketAddr>,
}

/// **Renew a run before it lapses** (`infra-client-requirements.md` §7):
/// the instance says where its run ends, and the next run opens exactly
/// there, contiguous as §8.2 requires. The proof is checked again first —
/// a key that is not this instance's gets no signature of this operator's,
/// renewal or not.
///
/// **And the anchor entry is re-signed where the subtree has moved under
/// it** [author, 2026-10-26]. §4.4 has the entry re-signed "when the
/// subtree size does [change]", and the size is the instance's own count
/// rather than a number its operator could know ([`Seen::subtree`]): so
/// the renewal, which is the visit an operator already makes, compares
/// the count against what the entry claims and signs a new one at the
/// next counter over the addresses the old one named. The endpoint record
/// is not touched — an address change is the operator's to state, and
/// nothing here knows one happened.
pub fn renew(me: &SigningIdentity, ask: &Ask) -> Result<String, String> {
    let addr = resolve(&ask.at)?;
    let mut out = String::new();
    let seen = checked(addr, &ask.token, &mut out)?;
    if seen.ends == 0 {
        return Err(format!(
            "the instance at {} holds no run to renew: `node enrol` signs its first",
            ask.at
        ));
    }
    out.push_str(&format!(
        "the run ends at {}; the next opens there\n",
        seen.ends
    ));
    let run = issue_run(me, &seen.key, seen.ends, ask.credentials);
    for (i, d) in run.iter().enumerate() {
        let said = push(addr, "/node/run", d)?;
        out.push_str(&format!("credential {} of {}: {said}\n", i + 1, run.len()));
    }
    out.push_str(&reanchored(me, addr, ask, &seen)?);
    Ok(out)
}

/// The anchor entry, re-signed where the instance's count differs from
/// what its entry claims; a line saying so either way.
fn reanchored(
    me: &SigningIdentity,
    addr: SocketAddr,
    ask: &Ask,
    seen: &Seen,
) -> Result<String, String> {
    let Some((claims, seqno, points)) = &seen.anchor else {
        return Ok("no anchor entry is signed here: `node enrol` signs the first\n".into());
    };
    if *claims == seen.subtree {
        return Ok(format!(
            "anchor entry: it claims a subtree of {claims} and counts {}, so it stands\n",
            seen.subtree
        ));
    }
    // the addresses the standing entry names, so a re-signing moves the
    // size and nothing else; where it named none, the one in hand
    let points: Vec<NetworkPoint> = if points.is_empty() {
        vec![point_of(ask.endpoint.unwrap_or(addr))]
    } else {
        points.clone()
    };
    // **the same series, or it supersedes nothing**: a sequence number
    // orders two entries only within one series (`wire-format.md` §2.3)
    let next = seqno
        .counter
        .checked_add(1)
        .ok_or("the entry's counter is at its last")?;
    let entry = anchor_entry(
        me,
        &points,
        seen.subtree,
        rhtn_archive::tx::Seqno {
            series: seqno.series,
            counter: next,
        },
    );
    let said = push(addr, "/node/anchor-entry", &entry)?;
    Ok(format!(
        "anchor entry re-signed at seqno {next}: a subtree of {} where it claimed {claims}: \
         {said}\n",
        seen.subtree
    ))
}

/// What the fetch said, once its proof has checked.
struct Seen {
    key: [u8; 32],
    /// When the run it holds ends, or 0 where it holds none.
    ends: u64,
    /// The subtree this instance counts beneath itself now (§4.4).
    subtree: u64,
    /// What its anchor entry claims, the sequence number it carries and
    /// the points it names, where one is signed.
    anchor: Option<(u64, rhtn_archive::tx::Seqno, Vec<NetworkPoint>)>,
}

/// The fetch and its proof, which both commands do first.
fn checked(addr: SocketAddr, token: &[u8; 32], out: &mut String) -> Result<Seen, String> {
    let nonce: [u8; 16] = rhtn_transport::tls::random_bytes::<16>();
    let got = ask_surface(
        addr,
        &format!("GET /node?nonce={} HTTP/1.1", hex(&nonce)),
        b"",
    )?;
    if status(&got) != 200 {
        return Err(format!("the surface refused the fetch:\n{}", body(&got)));
    }
    let key_hex = field(&got, "transport").ok_or("the answer carries no transport key")?;
    let shown = field(&got, "proof").ok_or("the answer carries no proof")?;
    let key: [u8; 32] = unhex(&key_hex)
        .and_then(|b| b.try_into().ok())
        .ok_or("the transport key is not 32 bytes of hex")?;
    let want = hex(&rhtn_daemon::administration::proof(token, &nonce, &key));
    if want != shown {
        return Err(format!(
            "**the proof did not check**: something answered at {addr} that does not hold the \
             token this configuration carried, so the key it offered is not this instance's. \
             Nothing was signed.\n  offered {key_hex}\n  proof   {shown}\n  expected {want}"
        ));
    }
    out.push_str(&format!("transport key {key_hex}, proof checks\n"));
    let ends = field(&got, "run-ends")
        .and_then(|s| s.parse::<u64>().ok())
        .unwrap_or(0);
    let subtree = field(&got, "subtree")
        .and_then(|s| s.parse::<u64>().ok())
        .unwrap_or(0);
    let anchor = match (
        field(&got, "anchor-claims").and_then(|s| s.parse::<u64>().ok()),
        field(&got, "anchor-seqno").and_then(|s| s.parse::<u32>().ok()),
    ) {
        (Some(claims), Some(counter)) => {
            let points: Vec<NetworkPoint> = field(&got, "anchor-points")
                .unwrap_or_default()
                .split(',')
                .filter(|p| !p.is_empty())
                .filter_map(unhex)
                .filter_map(|b| NetworkPoint::decode_bytes(&b).ok())
                .collect();
            // **the same series, or it supersedes nothing**: a sequence
            // number orders two entries only within one series
            // (`wire-format.md` §2.3), so a re-signing stays in the one
            // the standing entry carries
            let series = field(&got, "anchor-series")
                .and_then(|s| s.parse::<u32>().ok())
                .unwrap_or(0);
            Some((claims, rhtn_archive::tx::Seqno { series, counter }, points))
        }
        _ => None,
    };
    Ok(Seen {
        key,
        ends,
        subtree,
        anchor,
    })
}

/// Run the exchange, and say what happened at each step.
pub fn enrol(me: &SigningIdentity, ask: &Ask) -> Result<String, String> {
    let addr = resolve(&ask.at)?;
    let mut out = String::new();

    // **the fetch, and the proof over it.** A nonce of this caller's
    // choosing is what makes the answer unreplayable, and the token it is
    // checked under is what makes the key this instance's rather than
    // whatever answered (`rhtn_daemon::administration`).
    let seen = checked(addr, &ask.token, &mut out)?;

    // **the records first and the run last**: taking the run is what ends
    // the instance's wait, and the node then reads the two records from the
    // paths its configuration names (§4.4)
    if let Some(ep) = ask.endpoint {
        let point = point_of(ep);
        let record = endpoint_record(me, std::slice::from_ref(&point), seqno(1));
        let said = push(addr, "/node/endpoint-record", &record)?;
        out.push_str(&format!("endpoint record naming {ep}: {said}\n"));
    }
    // **the anchor entry is always signed at enrolment** [author,
    // 2026-10-25], with the subtree a fresh instance has beneath it
    {
        let size = rhtn_node::resolution::FRESH_SUBTREE;
        let point = point_of(ask.endpoint.unwrap_or(addr));
        let entry = anchor_entry(me, std::slice::from_ref(&point), size, seqno(1));
        let said = push(addr, "/node/anchor-entry", &entry)?;
        out.push_str(&format!(
            "anchor entry, a subtree of {size} beneath it: {said}\n"
        ));
    }
    let run = issue_run(me, &seen.key, now(), ask.credentials);
    for (i, d) in run.iter().enumerate() {
        let said = push(addr, "/node/run", d)?;
        out.push_str(&format!("credential {} of {}: {said}\n", i + 1, run.len()));
    }
    out.push_str(&format!(
        "\nthe node is enrolled; its administration page is at http://{addr}/\n"
    ));
    Ok(out)
}

/// One PUT, and the first line of what came back.
fn push(addr: SocketAddr, path: &str, body_bytes: &[u8]) -> Result<String, String> {
    let r = ask_surface(addr, &format!("PUT {path} HTTP/1.1"), body_bytes)?;
    let said = body(&r).lines().next().unwrap_or("").to_string();
    if status(&r) != 200 {
        return Err(format!("{path}: {} {said}", status(&r)));
    }
    Ok(said)
}

/// One exchange: the request line, a host and a length, and the response.
fn ask_surface(addr: SocketAddr, line: &str, body_bytes: &[u8]) -> Result<String, String> {
    let mut s =
        TcpStream::connect_timeout(&addr, TIMEOUT).map_err(|e| format!("connect {addr}: {e}"))?;
    s.set_read_timeout(Some(TIMEOUT)).ok();
    s.set_write_timeout(Some(TIMEOUT)).ok();
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
    at.to_socket_addrs()
        .map_err(|e| format!("{at}: {e}"))?
        .next()
        .ok_or_else(|| format!("{at}: no address"))
}

fn point_of(a: SocketAddr) -> NetworkPoint {
    match a.ip() {
        std::net::IpAddr::V4(v4) => NetworkPoint::new(v4.octets(), Some(u64::from(a.port()))),
        // a v6 instance is a case this instrument does not carry: the point
        // encoding is the node's and this is a bench tool, said rather than
        // guessed at
        std::net::IpAddr::V6(_) => NetworkPoint::new([127, 0, 0, 1], Some(u64::from(a.port()))),
    }
}

fn seqno(counter: u32) -> rhtn_archive::tx::Seqno {
    rhtn_archive::tx::Seqno { series: 0, counter }
}

fn now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn status(response: &str) -> u16 {
    response
        .lines()
        .next()
        .and_then(|l| l.split(' ').nth(1).and_then(|c| c.parse().ok()))
        .unwrap_or(0)
}

fn body(response: &str) -> &str {
    match response.find("\r\n\r\n") {
        Some(i) => &response[i + 4..],
        None => response,
    }
}

fn field(response: &str, name: &str) -> Option<String> {
    body(response)
        .lines()
        .find_map(|l| l.strip_prefix(&format!("{name} ")))
        .map(|v| v.trim().to_string())
}

/// The token or any other fixed-width hex an argument carries.
pub fn hex32(s: &str) -> Option<[u8; 32]> {
    unhex(s)?.try_into().ok()
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
