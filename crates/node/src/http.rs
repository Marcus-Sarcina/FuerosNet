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
    /// The request method.
    pub method: String,
    /// Origin-form target: path and query.
    pub target: String,
    /// Header names lowercased, in the order received, the caller's
    /// `host`, framing and `rhtn-*` headers removed.
    pub headers: Vec<(String, String)>,
    /// The request body.
    pub body: Vec<u8>,
}

/// Why a message is not forwarded: each answered `malformed request`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Reject {
    /// The request does not read.
    Malformed(&'static str),
    /// The request names more than one thing it could be for.
    Ambiguous(&'static str),
    /// A tunnel, a protocol switch or an interim response: not one request
    /// and one response.
    NotOneExchange(&'static str),
    /// The parse yielded more than one message.
    MoreThanOne,
}

fn is_token(s: &str) -> bool {
    !s.is_empty()
        && s.bytes()
            .all(|c| c.is_ascii_alphanumeric() || b"!#$%&'*+-.^_`|~".contains(&c))
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
    hay[from..]
        .windows(needle.len())
        .position(|w| w == needle)
        .map(|p| p + from)
}

/// The path of an origin-form target, without its query: what is routed
/// on, and the only part whose spelling this parser constrains.
fn path_of(target: &str) -> &str {
    target.split('?').next().unwrap_or(target)
}

/// Why this path has more than one reading, where it has.
///
/// Three spellings are refused, each of which another parser may read
/// differently from this one: a percent-escape of an unreserved character
/// (RFC 3986 §2.3, which §6.2.2.2 says decodes to itself), a
/// percent-escape of a path separator, and a dot segment.
fn ambiguous(path: &str) -> Option<&'static str> {
    let b = path.as_bytes();
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%' {
            if i + 2 >= b.len() {
                return Some("target percent-escape");
            }
            let hex = |c: u8| match c {
                b'0'..=b'9' => Some(c - b'0'),
                b'a'..=b'f' => Some(c - b'a' + 10),
                b'A'..=b'F' => Some(c - b'A' + 10),
                _ => None,
            };
            let (Some(h), Some(l)) = (hex(b[i + 1]), hex(b[i + 2])) else {
                return Some("target percent-escape");
            };
            let c = h * 16 + l;
            if c.is_ascii_alphanumeric() || matches!(c, b'-' | b'.' | b'_' | b'~') {
                return Some("target escapes an unreserved character");
            }
            if matches!(c, b'/' | b'\\') {
                return Some("target escapes a path separator");
            }
            i += 3;
            continue;
        }
        i += 1;
    }
    if path.split('/').any(|seg| seg == "." || seg == "..") {
        return Some("target carries a dot segment");
    }
    None
}

/// Parse one HTTP/1.1 request, strictly (RFC 9112 with every tolerance
/// removed).
pub fn parse(bytes: &[u8]) -> Result<Request, Reject> {
    let head_end = find(bytes, b"\r\n\r\n", 0).ok_or(Reject::Malformed("no end of headers"))?;
    let head =
        std::str::from_utf8(&bytes[..head_end]).map_err(|_| Reject::Malformed("head not ascii"))?;
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
    } else if let Some(rest) = target
        .strip_prefix("http://")
        .or_else(|| target.strip_prefix("https://"))
    {
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
    // **the path has one reading, or it is refused** [2026-10-27].
    //
    // A target is forwarded as the caller wrote it, and the thing at the
    // far end routes on it: a node that forwards two spellings of one
    // path has handed the routing decision to whichever of the two
    // parsers reads it. `PUT /%2erhtn/envelope` passed a guard on
    // `/.rhtn/` and reached a backend that decoded it back, which is how
    // a member reached a path reserved for the node
    // ([`crate::federation`]). RFC 3986 §6.2.2.2 calls the decoding a
    // normalisation; rather than normalise and hope the far end agrees,
    // the ambiguous spelling is refused and the normal form is the only
    // one that travels.
    if let Some(why) = ambiguous(path_of(&target)) {
        return Err(Reject::Malformed(why));
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
                let n: usize = value
                    .parse()
                    .map_err(|_| Reject::Malformed("content-length"))?;
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
            "connection"
                if value
                    .to_ascii_lowercase()
                    .split(',')
                    .any(|v| v.trim() == "upgrade") =>
            {
                return Err(Reject::NotOneExchange("Upgrade"));
            }
            _ => {
                if lname.starts_with("rhtn-") {
                    // the caller's assertion in the trusted namespace: gone
                    continue;
                }
                if headers.iter().any(|(n, _)| *n == lname)
                    && matches!(lname.as_str(), "authorization" | "content-type")
                {
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
    Ok(Request {
        method: method.to_string(),
        target,
        headers,
        body,
    })
}

/// Decode a chunked body strictly: hex sizes without extensions, exact
/// CRLFs, an empty last chunk and no trailers.
fn decode_chunked(b: &[u8]) -> Result<(Vec<u8>, usize), Reject> {
    let mut out = Vec::new();
    let mut at = 0;
    loop {
        let eol = find(b, b"\r\n", at).ok_or(Reject::Malformed("chunk size line"))?;
        let size_str =
            std::str::from_utf8(&b[at..eol]).map_err(|_| Reject::Malformed("chunk size"))?;
        if size_str.is_empty()
            || size_str.contains(';')
            || !size_str.bytes().all(|c| c.is_ascii_hexdigit())
        {
            return Err(Reject::Ambiguous("chunk size"));
        }
        let size =
            usize::from_str_radix(size_str, 16).map_err(|_| Reject::Malformed("chunk size"))?;
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
        let end = at
            .checked_add(size)
            .ok_or(Reject::Malformed("chunk size over the message"))?;
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
    /// Who the credential speaks for.
    pub principal: [u8; 32],
    /// The application roles it carries.
    pub roles: Vec<String>,
    /// Which resource it is for, so one resource's credential is not another's.
    pub audience: Keyhash,
    /// The hosted session it belongs to.
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
        let b = [
            chunk[0],
            *chunk.get(1).unwrap_or(&0),
            *chunk.get(2).unwrap_or(&0),
        ];
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
    let mut out = format!(
        "{} {} HTTP/1.1\r\nhost: {}\r\n",
        req.method, req.target, authority
    )
    .into_bytes();
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
    let Some(end) = find(message, b"\r\n\r\n", 0) else {
        return Vec::new();
    };
    let head = String::from_utf8_lossy(&message[..end]);
    head.split("\r\n")
        .skip(1)
        .filter_map(|l| {
            l.split_once(':')
                .map(|(n, v)| (n.trim().to_ascii_lowercase(), v.trim().to_string()))
        })
        .collect()
}

// ---- the response, parsed and re-serialised the same way -------------

/// A response as the parser produced it: what the asking client will see,
/// once this node has written it out canonically.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Response {
    /// The application's status code.
    pub status: u16,
    /// The reason phrase, kept where the package gave a readable one.
    pub reason: String,
    /// Header names lowercased, in the order received, the framing
    /// headers removed — they are rewritten from the body's own length.
    /// **Repeats are kept**: `set-cookie` and `link` are repeatable, and
    /// a node that collapsed them would be deciding the application's
    /// semantics rather than its framing.
    pub headers: Vec<(String, String)>,
    /// The body, de-chunked where it arrived chunked.
    pub body: Vec<u8>,
}

/// **Parse one HTTP/1.1 response from a backend** (`wire-format.md` §11
/// field 2, *same reasoning as the request*; §11.2).
///
/// **Less strict than [`parse`], deliberately.** The request parser's
/// strictness protects a backend this node cannot control, so an
/// ambiguity has to be refused rather than guessed at. Here this node is
/// the party that writes the canonical form, so it can be tolerant
/// wherever its own output removes the ambiguity: a package ending its
/// lines with a bare LF is read and written back with CRLF, and a chunked
/// body is joined and written back under a length. **What is refused is
/// what no re-serialisation can make unambiguous**: two lengths, a length
/// beside an encoding, an encoding this node does not decode, a folded
/// header, a control character in a value, malformed chunking, and bytes
/// after the body — which is §11.2's *more than one message, and that is
/// the attack*.
///
/// **No declared type, encoding or body content is refused.** A body is
/// bytes under a length to this parser; what a package may answer with is
/// untouched by it.
///
/// **One header family is dropped**: `rhtn-*`, so that the namespace
/// means the same thing in both directions as `resource-requirements.md`
/// §3.1 makes it mean on the way in.
pub fn parse_response(bytes: &[u8]) -> Result<Response, Reject> {
    // the head ends at a blank line, whichever way the package ends its
    // lines: a package is not required to agree with this parser about
    // that, because this parser is what decides what travels
    let (head_end, sep) = match (find(bytes, b"\r\n\r\n", 0), find(bytes, b"\n\n", 0)) {
        (Some(a), Some(b)) if a <= b => (a, 4),
        (Some(a), None) => (a, 4),
        (None, Some(b)) => (b, 2),
        (Some(_), Some(b)) => (b, 2),
        (None, None) => return Err(Reject::Malformed("no end of headers")),
    };
    let head = std::str::from_utf8(&bytes[..head_end])
        .map_err(|_| Reject::Malformed("response head not ascii"))?;
    if !head.is_ascii() {
        return Err(Reject::Malformed("response head not ascii"));
    }
    let mut lines = head.split('\n').map(|l| l.strip_suffix('\r').unwrap_or(l));
    let status_line = lines.next().ok_or(Reject::Malformed("no status line"))?;
    let rest_of_line = status_line
        .strip_prefix("HTTP/1.1 ")
        .ok_or(Reject::Malformed("response version"))?;
    let (code, reason) = match rest_of_line.split_once(' ') {
        Some((c, r)) => (c, r),
        None => (rest_of_line, ""),
    };
    if code.len() != 3 || !code.bytes().all(|c| c.is_ascii_digit()) {
        return Err(Reject::Malformed("response status code"));
    }
    let status: u16 = code
        .parse()
        .map_err(|_| Reject::Malformed("response status code"))?;
    // **an interim response is not the one answer** (`wire-format.md`
    // §11.2's *reject anything asking for an exchange other than one
    // request and one response*): a 1xx precedes the real one, and
    // `ResourceResponse` carries one
    if (100..200).contains(&status) {
        return Err(Reject::NotOneExchange("an interim 1xx response"));
    }
    if !reason.is_empty() && !is_field_value(reason) {
        return Err(Reject::Malformed("response reason phrase"));
    }

    let mut headers: Vec<(String, String)> = Vec::new();
    let (mut content_length, mut chunked) = (None::<usize>, false);
    for line in lines {
        if line.starts_with(' ') || line.starts_with('\t') {
            return Err(Reject::Ambiguous("obsolete line folding"));
        }
        let colon = line
            .find(':')
            .ok_or(Reject::Malformed("response header line"))?;
        let (name, value) = (&line[..colon], line[colon + 1..].trim_matches([' ', '\t']));
        if !is_token(name) {
            return Err(Reject::Malformed("response header name"));
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
                content_length = Some(
                    value
                        .parse()
                        .map_err(|_| Reject::Malformed("response content-length"))?,
                );
            }
            "transfer-encoding" => {
                if chunked || !value.eq_ignore_ascii_case("chunked") {
                    return Err(Reject::Ambiguous("transfer-encoding"));
                }
                chunked = true;
            }
            "upgrade" => return Err(Reject::NotOneExchange("Upgrade")),
            "connection"
                if value
                    .to_ascii_lowercase()
                    .split(',')
                    .any(|v| v.trim() == "upgrade") =>
            {
                return Err(Reject::NotOneExchange("Upgrade"));
            }
            // **two content-types leave undecided what may be shown**, and
            // a client is told the declared type decides
            // (`light-client-requirements.md` §8)
            "content-type" if headers.iter().any(|(n, _)| n == "content-type") => {
                return Err(Reject::Ambiguous("duplicate content-type"));
            }
            _ => {
                // **the namespace is the node's in both directions**
                // [2026-10-28]. `resource-requirements.md` §3.1 has a
                // member's `rhtn-*` headers stripped on the way in, so
                // that a resource seeing one is seeing its host; the same
                // rule on the way out is what makes *an `rhtn-*` header
                // on this path was written by the node* a sentence that
                // is true rather than undefined. Nothing reads one off a
                // response today, which is the reason to settle it now:
                // a reader added later should not have to ask who could
                // have written it, and no package exists yet to break.
                //
                // **Stripped rather than refused**, as the member's are:
                // a stray header is a defect in the package and the
                // operator is told, where refusing the whole answer
                // would take the resource down over a header nobody
                // reads. The host path is untouched — a request under
                // `/.rhtn/` never reaches this parse
                // ([`crate::federation`]), and its answers are read by
                // `federation::response_of`.
                if lname.starts_with("rhtn-") {
                    tracing::warn!(
                        target: "node",
                        header = %lname,
                        "node.resource.response.rhtn_stripped"
                    );
                    continue;
                }
                headers.push((lname, value.to_string()))
            }
        }
    }
    if content_length.is_some() && chunked {
        return Err(Reject::Ambiguous("content-length and transfer-encoding"));
    }
    let rest = &bytes[head_end + sep..];
    let (body, consumed) = if chunked {
        decode_chunked(rest)?
    } else if let Some(n) = content_length {
        if rest.len() < n {
            return Err(Reject::Malformed("response body short"));
        }
        (rest[..n].to_vec(), n)
    } else {
        // **neither header, so the backend's own answer delimits it.**
        // `Backend::handle` returns one buffer, and that buffer's end is
        // the message's end — which is why this is not the ambiguity the
        // same case is on a connection. Written back under a length, so
        // nothing downstream has to decide it either.
        (rest.to_vec(), rest.len())
    };
    if consumed != rest.len() {
        return Err(Reject::MoreThanOne);
    }
    Ok(Response {
        status,
        reason: reason.to_string(),
        headers,
        body,
    })
}

/// Re-serialise a response from the parse: the status line, the headers as
/// parsed, one length taken from the body itself, and the body.
///
/// **This is what makes the tolerances above safe.** Whatever the package
/// wrote, what leaves here is one message whose extent is declared
/// exactly once and agrees with its own content, so no party downstream
/// has a second reading available to it.
pub fn serialise_response(r: &Response) -> Vec<u8> {
    let mut out = if r.reason.is_empty() {
        format!("HTTP/1.1 {}\r\n", r.status).into_bytes()
    } else {
        format!("HTTP/1.1 {} {}\r\n", r.status, r.reason).into_bytes()
    };
    for (n, v) in &r.headers {
        out.extend_from_slice(format!("{n}: {v}\r\n").as_bytes());
    }
    out.extend_from_slice(format!("content-length: {}\r\n\r\n", r.body.len()).as_bytes());
    out.extend_from_slice(&r.body);
    out
}
