//! The gateway's HTTP/1.1 parse and re-serialisation (`wire-format.md`
//! §11.2; `resource-requirements.md` §3, §3.1): the node parses the
//! message fully, rejects rather than normalises anything ambiguous,
//! emits exactly one message per request, routes by the resource and
//! never by what the caller wrote, strips the caller's `rhtn-*` headers,
//! and inserts its own credential from its parse.  Canonical means from
//! this parser, deterministically; nothing signs these bytes.

use crate::Keyhash;
use rhtn_codec::cose::sha256;

/// A request as the parser produced it: what the backend will see, before
/// the credential is added.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Request {
    pub method: String,
    /// Origin-form target: path and query.
    pub target: String,
    /// Header names lowercased, in the order received, the caller's
    /// `host`, framing and `rhtn-*` headers removed.
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
}

/// Why a message is not forwarded: each answered `malformed request`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Reject {
    Malformed(&'static str),
    Ambiguous(&'static str),
    /// A tunnel, a protocol switch or an interim response: not one request
    /// and one response.
    NotOneExchange(&'static str),
    /// The parse yielded more than one message.
    MoreThanOne,
}

fn is_token(s: &str) -> bool {
    !s.is_empty() && s.bytes().all(|c| c.is_ascii_alphanumeric() || b"!#$%&'*+-.^_`|~".contains(&c))
}

/// A header value is visible characters, spaces and horizontal tabs (RFC
/// 9110 §5.5).  **A bare LF or CR is the whole hazard this boundary
/// exists to close**: this parser ends a line at CRLF, a backend may end
/// one at LF, and a value carrying either would reach that backend as a
/// header line of the caller's own writing — `rhtn-roles` among them,
/// which `resource-requirements.md` §3.1 says defeats the gateway
/// entirely.  Rejected rather than stripped: `wire-format.md` §11.2
/// refuses the ambiguous instead of normalising it.
fn is_field_value(s: &str) -> bool {
    s.bytes().all(|c| c == b'\t' || (0x20..=0x7e).contains(&c))
}

fn find(hay: &[u8], needle: &[u8], from: usize) -> Option<usize> {
    hay[from..].windows(needle.len()).position(|w| w == needle).map(|p| p + from)
}

/// Parse one HTTP/1.1 request, strictly (RFC 9112 with every tolerance
/// removed).
pub fn parse(bytes: &[u8]) -> Result<Request, Reject> {
    let head_end = find(bytes, b"\r\n\r\n", 0).ok_or(Reject::Malformed("no end of headers"))?;
    let head = std::str::from_utf8(&bytes[..head_end]).map_err(|_| Reject::Malformed("head not ascii"))?;
    if !head.is_ascii() {
        return Err(Reject::Malformed("head not ascii"));
    }
    let mut lines = head.split("\r\n");
    let request_line = lines.next().ok_or(Reject::Malformed("no request line"))?;
    let parts: Vec<&str> = request_line.split(' ').collect();
    if parts.len() != 3 {
        return Err(Reject::Malformed("request line"));
    }
    let (method, target, version) = (parts[0], parts[1], parts[2]);
    if !is_token(method) {
        return Err(Reject::Malformed("method"));
    }
    if version != "HTTP/1.1" {
        return Err(Reject::Malformed("version"));
    }
    if method == "CONNECT" {
        return Err(Reject::NotOneExchange("CONNECT"));
    }
    // the target: origin form kept, absolute form reduced to it, anything
    // else refused — the resource keyhash routes, not the target
    let target = if target.starts_with('/') {
        target.to_string()
    } else if let Some(rest) = target.strip_prefix("http://").or_else(|| target.strip_prefix("https://")) {
        match rest.find('/') {
            Some(i) => rest[i..].to_string(),
            None => "/".to_string(),
        }
    } else {
        return Err(Reject::Malformed("target form"));
    };
    if target.bytes().any(|c| c <= b' ' || c == 0x7f) {
        return Err(Reject::Malformed("target characters"));
    }
    let mut headers: Vec<(String, String)> = Vec::new();
    let (mut content_length, mut chunked, mut host_seen) = (None::<usize>, false, false);
    for line in lines {
        if line.starts_with(' ') || line.starts_with('\t') {
            return Err(Reject::Ambiguous("obsolete line folding"));
        }
        let colon = line.find(':').ok_or(Reject::Malformed("header line"))?;
        let (name, value) = (&line[..colon], line[colon + 1..].trim_matches([' ', '\t']));
        if !is_token(name) {
            return Err(Reject::Malformed("header name"));
        }
        if !is_field_value(value) {
            return Err(Reject::Ambiguous("control character in a header value"));
        }
        let lname = name.to_ascii_lowercase();
        match lname.as_str() {
            "content-length" => {
                if content_length.is_some() || value.contains(',') {
                    return Err(Reject::Ambiguous("duplicate content-length"));
                }
                let n: usize = value.parse().map_err(|_| Reject::Malformed("content-length"))?;
                content_length = Some(n);
            }
            "transfer-encoding" => {
                if chunked || !value.eq_ignore_ascii_case("chunked") {
                    return Err(Reject::Ambiguous("transfer-encoding"));
                }
                chunked = true;
            }
            "host" => {
                if host_seen {
                    return Err(Reject::Ambiguous("duplicate host"));
                }
                host_seen = true;
            }
            "upgrade" => return Err(Reject::NotOneExchange("Upgrade")),
            "expect" => return Err(Reject::NotOneExchange("Expect")),
            "connection" if value.to_ascii_lowercase().split(',').any(|v| v.trim() == "upgrade") => return Err(Reject::NotOneExchange("Upgrade")),
            _ => {
                if lname.starts_with("rhtn-") {
                    // the caller's assertion in the trusted namespace: gone
                    continue;
                }
                if headers.iter().any(|(n, _)| *n == lname) && matches!(lname.as_str(), "authorization" | "content-type") {
                    return Err(Reject::Ambiguous("duplicate singleton header"));
                }
                headers.push((lname, value.to_string()));
            }
        }
    }
    if !host_seen {
        return Err(Reject::Malformed("no host"));
    }
    if content_length.is_some() && chunked {
        return Err(Reject::Ambiguous("content-length and transfer-encoding"));
    }
    let body_start = head_end + 4;
    let rest = &bytes[body_start..];
    let (body, consumed) = if chunked {
        decode_chunked(rest)?
    } else {
        let n = content_length.unwrap_or(0);
        if rest.len() < n {
            return Err(Reject::Malformed("body short"));
        }
        (rest[..n].to_vec(), n)
    };
    if consumed != rest.len() {
        // bytes after the message: a second message, which is the attack
        return Err(Reject::MoreThanOne);
    }
    Ok(Request { method: method.to_string(), target, headers, body })
}

/// Decode a chunked body strictly: hex sizes without extensions, exact
/// CRLFs, an empty last chunk and no trailers.
fn decode_chunked(b: &[u8]) -> Result<(Vec<u8>, usize), Reject> {
    let mut out = Vec::new();
    let mut at = 0;
    loop {
        let eol = find(b, b"\r\n", at).ok_or(Reject::Malformed("chunk size line"))?;
        let size_str = std::str::from_utf8(&b[at..eol]).map_err(|_| Reject::Malformed("chunk size"))?;
        if size_str.is_empty() || size_str.contains(';') || !size_str.bytes().all(|c| c.is_ascii_hexdigit()) {
            return Err(Reject::Ambiguous("chunk size"));
        }
        let size = usize::from_str_radix(size_str, 16).map_err(|_| Reject::Malformed("chunk size"))?;
        at = eol + 2;
        if size == 0 {
            if b.get(at..at.saturating_add(2)) != Some(b"\r\n") {
                return Err(Reject::Ambiguous("trailers"));
            }
            return Ok((out, at + 2));
        }
        // a length is a claim about bytes that are here: added to the
        // offset unchecked it overflows, which is a panic in one build
        // profile and a wrapped slice in another
        let end = at.checked_add(size).ok_or(Reject::Malformed("chunk size over the message"))?;
        let data = b.get(at..end).ok_or(Reject::Malformed("chunk short"))?;
        out.extend_from_slice(data);
        at = end;
        if b.get(at..at.saturating_add(2)) != Some(b"\r\n") {
            return Err(Reject::Malformed("chunk end"));
        }
        at += 2;
    }
}

/// The credential the gateway presents (`resource-requirements.md` §2).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Credential {
    pub principal: [u8; 32],
    pub roles: Vec<String>,
    pub audience: Keyhash,
    pub session: [u8; 16],
}

/// `SHA-256("rhtn/1:pairwise" ‖ resource ‖ user)` (design §11.0.2).
pub fn pairwise_principal(resource: &Keyhash, user: &Keyhash) -> [u8; 32] {
    let mut pre = b"rhtn/1:pairwise".to_vec();
    pre.extend_from_slice(resource);
    pre.extend_from_slice(user);
    sha256(&pre)
}

/// RFC 4648 §5 without padding.
pub fn base64url(bytes: &[u8]) -> String {
    const A: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";
    let mut out = String::new();
    for chunk in bytes.chunks(3) {
        let b = [chunk[0], *chunk.get(1).unwrap_or(&0), *chunk.get(2).unwrap_or(&0)];
        let n = (b[0] as u32) << 16 | (b[1] as u32) << 8 | b[2] as u32;
        out.push(A[(n >> 18) as usize & 63] as char);
        out.push(A[(n >> 12) as usize & 63] as char);
        if chunk.len() > 1 {
            out.push(A[(n >> 6) as usize & 63] as char);
        }
        if chunk.len() > 2 {
            out.push(A[n as usize & 63] as char);
        }
    }
    out
}

/// Re-serialise from the parse: origin-form target, the backend's
/// authority as `host`, the caller's headers as parsed, the body under its
/// exact length, and the credential last.
pub fn serialise(req: &Request, authority: &str, cred: &Credential) -> Vec<u8> {
    let mut out = format!("{} {} HTTP/1.1\r\nhost: {}\r\n", req.method, req.target, authority).into_bytes();
    for (n, v) in &req.headers {
        out.extend_from_slice(format!("{n}: {v}\r\n").as_bytes());
    }
    out.extend_from_slice(format!("content-length: {}\r\n", req.body.len()).as_bytes());
    out.extend_from_slice(format!("rhtn-principal: {}\r\n", base64url(&cred.principal)).as_bytes());
    out.extend_from_slice(format!("rhtn-roles: {}\r\n", cred.roles.join(",")).as_bytes());
    out.extend_from_slice(format!("rhtn-audience: {}\r\n", base64url(&cred.audience)).as_bytes());
    out.extend_from_slice(format!("rhtn-session: {}\r\n\r\n", base64url(&cred.session)).as_bytes());
    out.extend_from_slice(&req.body);
    out
}

/// The headers of a serialised message, lowercased, for a backend or a
/// test to read.
pub fn headers_of(message: &[u8]) -> Vec<(String, String)> {
    let Some(end) = find(message, b"\r\n\r\n", 0) else { return Vec::new() };
    let head = String::from_utf8_lossy(&message[..end]);
    head.split("\r\n").skip(1).filter_map(|l| l.split_once(':').map(|(n, v)| (n.trim().to_ascii_lowercase(), v.trim().to_string()))).collect()
}
