//! **What a resource answered, and what may be shown of it**
//! (`light-client-requirements.md` §8; `wire-format.md` §11).
//!
//! §11's `ResourceResponse` carries one of six statuses and, where it
//! delivered, *the application response as an HTTP/1.1 message*. So a
//! client that means to show a resource's page has to read an HTTP
//! message, and **this is the one place that does it.**
//!
//! **Why here and not in the shell.** `wire-format.md` §11.2 names the
//! hazard the node's parse exists to close: two parties disagreeing about
//! where one message ends and the next begins, so that content one treats
//! as a body the other treats as a fresh message. That hazard is about a
//! boundary, not about a direction — a shell parsing the response while
//! the kernel hands it bytes would be exactly the second parser §11.2
//! forbids on the other side. The boundary's own rule says the same from
//! the other end: nothing above it parses a wire byte.
//!
//! **The discipline is §11.2's, applied to the response.** Reject rather
//! than normalise: a bare newline where the grammar wants CRLF, a folded
//! header, a duplicated length, a length beside a transfer-encoding,
//! bytes after the body.
//!
//! **Why a client checks at all, given the node does** [found and closed
//! 2026-10-28]. §11 has the node parse and re-serialise the response as
//! well as the request — field 2's *same reasoning as the request*, where
//! the request's own note is that opaque *means the node does not
//! interpret the application semantics, not that it forwards unexamined
//! input*. Writing this parser found that this workspace's node did not:
//! it handed the backend's bytes straight to `ResourceResponse`, so a
//! package emitting two HTTP messages reached a client with both. That is
//! closed (`rhtn-node`'s `http::parse_response`), and this stays: §11.2's
//! argument is about a party in the middle, and a client that trusted
//! every node to have done its half would be trusting the one party the
//! isolation is against. **What arrives from a conforming node is already
//! canonical**, so every refusal here now means a node that is not one.
//!
//! **Nothing is sniffed.** The declared media type decides what may be
//! shown, an absent one decides nothing, and a body is never inspected to
//! find out what it is. Guessing is how a page arrives as something the
//! person did not agree to look at.

/// What a client will show of a resource's answer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Shape {
    /// `text/html`: a page, to be framed.
    Page,
    /// `text/plain`: text, which is not a page and wants no frame.
    Text,
    /// **A typed ask of the client** (`crate::utility`): not shown at
    /// all, but read, and put to the person as a question.
    Ask,
    /// Anything else, named rather than rendered — and an absent
    /// declaration, which names nothing.
    Other,
}

/// A resource's answer, parsed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Answer {
    /// The application's own HTTP status.  **Not §11's status**, which is
    /// the node's answer about whether anything was delivered at all.
    pub http_status: u16,
    /// The declared media type, lower-cased with its parameters off, or
    /// none where the resource declared none.
    pub media: Option<String>,
    /// The declared charset, lower-cased, where there was one.
    pub charset: Option<String>,
    /// What may be shown of it.
    pub shape: Shape,
    /// The body as it arrived.
    pub body: Vec<u8>,
}

/// Why a message was not taken.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Refusal {
    /// No `CRLF CRLF` ends the header section.
    NoHeaderEnd,
    /// The status line is not `HTTP/1.1 ` and three digits.
    StatusLine,
    /// A header line is not `name: value`, or folds onto the next.
    Header(String),
    /// A header that decides the body's extent appears twice, or two that
    /// decide it differently appear together.
    Ambiguous(&'static str),
    /// The body is not the length the headers declared, or bytes follow
    /// it — which is a second message.
    Body(&'static str),
}

impl Refusal {
    /// The refusal in a sentence, for a shell to show.
    pub fn say(&self) -> String {
        match self {
            Refusal::NoHeaderEnd => "the answer's headers do not end".into(),
            Refusal::StatusLine => "the answer does not begin with an HTTP/1.1 status".into(),
            Refusal::Header(h) => format!("a header line cannot be read: {h}"),
            Refusal::Ambiguous(w) => format!("the answer's length is ambiguous: {w}"),
            Refusal::Body(w) => format!("the answer's body does not match its headers: {w}"),
        }
    }
}

/// The media types a client renders, and what each becomes.
fn shape_of(media: Option<&str>) -> Shape {
    match media {
        Some("text/html") => Shape::Page,
        Some("text/plain") => Shape::Text,
        Some(crate::utility::MEDIA) => Shape::Ask,
        _ => Shape::Other,
    }
}

/// Take one HTTP/1.1 response message: §11's field 2.
pub fn answer(bytes: &[u8]) -> Result<Answer, Refusal> {
    let end = find(bytes, b"\r\n\r\n").ok_or(Refusal::NoHeaderEnd)?;
    let head = &bytes[..end];
    let rest = &bytes[end + 4..];

    // **a bare newline is an ambiguity, not a dialect.** It is the oldest
    // way to make two parsers disagree about a boundary, and §11.2 has the
    // node reject it rather than normalise it; the client's half of the
    // same rule is to reject what reaches it that way.
    if head.contains(&b'\n') && !head.windows(2).any(|w| w == b"\r\n") {
        return Err(Refusal::Header(
            "a line ends without a carriage return".into(),
        ));
    }
    let mut lines = Vec::new();
    let mut at = 0usize;
    while at <= head.len() {
        let n = find(&head[at..], b"\r\n")
            .map(|i| at + i)
            .unwrap_or(head.len());
        let line = &head[at..n];
        if line.contains(&b'\n') || line.contains(&b'\r') {
            return Err(Refusal::Header(
                "a line ends without a carriage return".into(),
            ));
        }
        lines.push(line);
        if n == head.len() {
            break;
        }
        at = n + 2;
    }

    let status = status_of(lines.first().copied().unwrap_or(b""))?;

    let mut content_length: Option<u64> = None;
    let mut lengths = 0usize;
    let mut chunked = false;
    let mut encodings = 0usize;
    let mut content_type: Option<String> = None;
    let mut types = 0usize;
    for line in &lines[1..] {
        if line.is_empty() {
            return Err(Refusal::Header("an empty line inside the headers".into()));
        }
        // obs-fold: a continuation line, which two parsers join differently
        if line[0] == b' ' || line[0] == b'\t' {
            return Err(Refusal::Header("a header folded onto the next line".into()));
        }
        let colon = line
            .iter()
            .position(|&c| c == b':')
            .ok_or_else(|| Refusal::Header(lossy(line)))?;
        let name = &line[..colon];
        if name.is_empty() || name.iter().any(|&c| c == b' ' || c == b'\t') {
            return Err(Refusal::Header(lossy(line)));
        }
        let lower = name.to_ascii_lowercase();
        let value = String::from_utf8_lossy(&line[colon + 1..])
            .trim()
            .to_string();
        match lower.as_slice() {
            b"content-length" => {
                lengths += 1;
                content_length =
                    Some(value.parse::<u64>().map_err(|_| {
                        Refusal::Ambiguous("a content-length that is not a number")
                    })?);
            }
            b"transfer-encoding" => {
                encodings += 1;
                if !value.eq_ignore_ascii_case("chunked") {
                    return Err(Refusal::Ambiguous("a transfer-encoding other than chunked"));
                }
                chunked = true;
            }
            b"content-type" => {
                types += 1;
                content_type = Some(value);
            }
            _ => {}
        }
    }
    // §11.2's own list, from this side: conflicting or duplicated length,
    // and a length beside an encoding that also decides the extent
    if lengths > 1 {
        return Err(Refusal::Ambiguous("content-length twice"));
    }
    if encodings > 1 {
        return Err(Refusal::Ambiguous("transfer-encoding twice"));
    }
    if chunked && content_length.is_some() {
        return Err(Refusal::Ambiguous(
            "a content-length beside a transfer-encoding",
        ));
    }
    if types > 1 {
        return Err(Refusal::Ambiguous("content-type twice"));
    }

    let body = if chunked {
        dechunk(rest)?
    } else if let Some(n) = content_length {
        let n =
            usize::try_from(n).map_err(|_| Refusal::Body("a content-length beyond this device"))?;
        match rest.len().cmp(&n) {
            std::cmp::Ordering::Less => return Err(Refusal::Body("fewer bytes than declared")),
            // **bytes after the body are a second message**, which is the
            // whole of §11.2's hazard: one answer per response
            std::cmp::Ordering::Greater => {
                return Err(Refusal::Body("bytes after the declared body"));
            }
            std::cmp::Ordering::Equal => rest.to_vec(),
        }
    } else {
        // neither header: §11 has the stream carry one answer and close,
        // so what is left is the body and there is nothing after it
        rest.to_vec()
    };

    let (media, charset) = type_of(content_type.as_deref());
    Ok(Answer {
        http_status: status,
        shape: shape_of(media.as_deref()),
        media,
        charset,
        body,
    })
}

/// `HTTP/1.1 200 OK` — the version fixed, the code three digits, the
/// reason phrase ignored.  **No other version**: §11 names HTTP/1.1, and
/// a client that accepted another would be reading a message by rules it
/// had not been given.
fn status_of(line: &[u8]) -> Result<u16, Refusal> {
    const V: &[u8] = b"HTTP/1.1 ";
    if line.len() < V.len() + 3 || &line[..V.len()] != V {
        return Err(Refusal::StatusLine);
    }
    let digits = &line[V.len()..V.len() + 3];
    if !digits.iter().all(|c| c.is_ascii_digit()) {
        return Err(Refusal::StatusLine);
    }
    if line.len() > V.len() + 3 && line[V.len() + 3] != b' ' {
        return Err(Refusal::StatusLine);
    }
    let s = (digits[0] - b'0') as u16 * 100
        + (digits[1] - b'0') as u16 * 10
        + (digits[2] - b'0') as u16;
    Ok(s)
}

/// The media type and charset out of a `Content-Type`, each lower-cased,
/// the type with its parameters taken off.
fn type_of(value: Option<&str>) -> (Option<String>, Option<String>) {
    let Some(v) = value else {
        return (None, None);
    };
    let mut parts = v.split(';');
    let media = parts.next().unwrap_or("").trim().to_ascii_lowercase();
    let mut charset = None;
    for p in parts {
        let p = p.trim();
        if let Some(rest) = p
            .strip_prefix("charset=")
            .or_else(|| p.strip_prefix("CHARSET="))
            .or_else(|| {
                let (k, val) = p.split_once('=')?;
                k.eq_ignore_ascii_case("charset").then_some(val)
            })
        {
            charset = Some(rest.trim().trim_matches('"').to_ascii_lowercase());
        }
    }
    (if media.is_empty() { None } else { Some(media) }, charset)
}

/// Chunked transfer, decoded strictly: a hex size, CRLF, that many bytes,
/// CRLF, and a terminal zero chunk with an empty trailer section.
/// **A chunk extension or a trailer is refused**, each being a place two
/// parsers have differed.
fn dechunk(mut rest: &[u8]) -> Result<Vec<u8>, Refusal> {
    let mut out = Vec::new();
    loop {
        let n = find(rest, b"\r\n").ok_or(Refusal::Body("a chunk size that does not end"))?;
        let size = &rest[..n];
        if size.is_empty() || !size.iter().all(|c| c.is_ascii_hexdigit()) {
            return Err(Refusal::Body("a chunk size that is not plain hex"));
        }
        let want = usize::from_str_radix(&String::from_utf8_lossy(size), 16)
            .map_err(|_| Refusal::Body("a chunk size that is not plain hex"))?;
        rest = &rest[n + 2..];
        if want == 0 {
            // the terminal chunk, then an empty trailer section and nothing
            if rest != b"\r\n" {
                return Err(Refusal::Body("something after the last chunk"));
            }
            return Ok(out);
        }
        if rest.len() < want + 2 {
            return Err(Refusal::Body("a chunk shorter than its size"));
        }
        out.extend_from_slice(&rest[..want]);
        if &rest[want..want + 2] != b"\r\n" {
            return Err(Refusal::Body("a chunk not followed by a carriage return"));
        }
        rest = &rest[want + 2..];
    }
}

fn find(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack.windows(needle.len()).position(|w| w == needle)
}

fn lossy(line: &[u8]) -> String {
    String::from_utf8_lossy(&line[..line.len().min(64)]).to_string()
}
