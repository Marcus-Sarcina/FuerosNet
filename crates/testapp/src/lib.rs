//! **A test resource that exercises the utility request, end to end**
//! (`resource-requirements.md` §3.4; `light-client-requirements.md` §8.1).
//!
//! What it is for: one package, hosted by a real node, that asks a real
//! client for every utility in turn, takes each answer back, verifies the
//! claims that carry one, and hands the person a file at the end. It is
//! the application half of the contract, written as an application writes
//! it — an ordinary HTTP server behind the gateway, reading the credential
//! headers the node inserts and answering with the typed asks.
//!
//! **Everything here is pure.** [`App::handle`] takes the bytes of one
//! request and gives back the bytes of one response, so the whole
//! sequence is driven in a unit test; `main.rs` is the socket and the log
//! and nothing else. The tests compose the client's half with the kernel's
//! own `utility` module and parse it here, which is what holds the two
//! halves of §3.4 against each other without a phone in the room.

use rhtn_codec::cose::sha256;
use std::collections::BTreeMap;

/// The media type a typed ask is declared under (`light-client-requirements.md`
/// §8.1).  **Written out rather than imported**: an application is written
/// against the published contract, and a test that borrowed the kernel's
/// constant would not notice the contract moving.
pub const ASK_MEDIA: &str = "application/vnd.rhtn.utility+json";

/// The tag the claim's signed array opens with (§3.4).
pub const CLAIM_TAG: &str = "rhtn/utility@1";

/// One step of the sequence: the utility, the token that names it coming
/// back, and the words the person reads.
pub struct Step {
    /// The utility asked for.
    pub utility: &'static str,
    /// This application's own name for the ask, echoed in the answer.
    pub token: &'static str,
    /// What it is for, in this application's words.
    pub purpose: &'static str,
    /// What this application says it does with the result.
    pub retention: &'static str,
    /// The utility's own fields, as a JSON fragment with a leading comma,
    /// or empty.
    pub fields: &'static str,
    /// For `sign`: the exact bytes the person approves.
    pub payload: Option<&'static str>,
}

/// **The sequence, in the order the bench walks it.** `notify` first, so
/// the asks after it are posted as notifications; the two the shell
/// cannot give (`live`, and a scan a resource will take only as a PDF)
/// are in it deliberately, because a refusal carrying its reason is as
/// much of the contract as a result is; `export` last, because its answer
/// is the file rather than another ask.
pub const STEPS: &[Step] = &[
    Step {
        utility: "notify",
        token: "t1-notify",
        purpose: "to tell you when an order of yours is ready at the counter",
        retention: "until you take it back",
        fields: "",
        payload: None,
    },
    Step {
        utility: "tag",
        token: "t2-tag",
        purpose: "to read the code on the thing you are returning",
        retention: "with the return, for a year",
        fields: "",
        payload: None,
    },
    Step {
        utility: "check-in",
        token: "t3-checkin",
        purpose: "to mark you as arrived at the counter",
        retention: "for the day, then deleted",
        fields: ",\"channel\":\"qr\"",
        payload: None,
    },
    Step {
        utility: "location",
        token: "t4-place",
        purpose: "to send the van to the right street",
        retention: "for the delivery, then deleted",
        fields: ",\"precision\":7",
        payload: None,
    },
    Step {
        utility: "capture",
        token: "t5-capture",
        purpose: "to photograph the meter for the reading",
        retention: "ninety days",
        fields: ",\"media\":[\"image/jpeg\"]",
        payload: None,
    },
    Step {
        utility: "scan",
        token: "t6-scan",
        purpose: "to take the delivery note you signed",
        retention: "seven years, as the law wants",
        fields: ",\"media\":[\"image/jpeg\"]",
        payload: None,
    },
    Step {
        utility: "sign",
        token: "t7-sign",
        purpose: "to accept the work on this order as done",
        retention: "with the job, indefinitely",
        fields: "",
        payload: Some("I accept the work on order 42 as complete."),
    },
    Step {
        utility: "import",
        token: "t8-import",
        purpose: "to attach a photograph you already have",
        retention: "with the order",
        fields: "",
        payload: None,
    },
    // **the two this shell cannot give**, asked for on purpose: a live
    // session it has no media stack for, and a scan a resource will take
    // only as a PDF. Each must come back a refusal with words in it.
    Step {
        utility: "live",
        token: "t9-live",
        purpose: "to open the counter's video desk for two minutes",
        retention: "not kept",
        fields: ",\"seconds\":120",
        payload: None,
    },
    Step {
        utility: "scan",
        token: "t10-pdf",
        purpose: "to take the delivery note as a searchable PDF",
        retention: "seven years, as the law wants",
        fields: ",\"media\":[\"application/pdf\"]",
        payload: None,
    },
    Step {
        utility: "export",
        token: "t11-export",
        purpose: "to hand you the record of this test",
        retention: "yours once you have saved it",
        fields: "",
        payload: None,
    },
];

/// The path every answer is posted back to, with the step's token on the
/// end so a crossed answer is visible rather than silent.
pub fn return_path(token: &str) -> String {
    format!("/u/{token}")
}

/// The ask for one step, as the JSON object §3.4 describes.
pub fn ask(step: &Step) -> String {
    let mut out = String::new();
    out.push('{');
    out.push_str(&format!("\"utility\":\"{}\",", step.utility));
    out.push_str(&format!("\"ask\":\"{}\",", step.purpose));
    out.push_str(&format!("\"retention\":\"{}\",", step.retention));
    out.push_str(&format!("\"return\":\"{}\",", return_path(step.token)));
    out.push_str(&format!("\"token\":\"{}\"", step.token));
    if let Some(p) = step.payload {
        out.push_str(&format!(",\"payload\":\"{}\"", hex(p.as_bytes())));
    }
    out.push_str(step.fields);
    out.push('}');
    out
}

/// What came back for one step.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Returned {
    /// The utility the client says it answered.
    pub utility: String,
    /// The token it echoed.
    pub token: String,
    /// When the person answered, by their clock, in milliseconds.
    pub at: u64,
    /// Whether anything was given.
    pub given: bool,
    /// Why not, where it was declined.
    pub reason: Option<String>,
    /// The declared type of a file result.
    pub media: Option<String>,
    /// The result's bytes, where one was carried.
    pub result: Option<Vec<u8>>,
    /// The digest the claim is over.
    pub digest: Option<[u8; 32]>,
    /// The application key the claim verifies under.
    pub key: Option<[u8; 32]>,
    /// The signature.
    pub claim: Option<Vec<u8>>,
}

/// What this application made of a claim.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Claimed {
    /// Nothing was claimed and nothing should have been.
    Nothing,
    /// The claim is over what came back, and it holds.
    Verified,
    /// Something is wrong with it, said.
    Failed(String),
}

/// Take one answer out of the `multipart/form-data` the kernel composed
/// (§3.4's parts).  **The node has already parsed and re-serialised the
/// message** (`wire-format.md` §11.2), so what arrives here is canonical
/// and this reads the body alone.
pub fn parse_answer(content_type: &str, body: &[u8]) -> Result<Returned, String> {
    let boundary = content_type
        .split(';')
        .map(str::trim)
        .find_map(|p| p.strip_prefix("boundary="))
        .ok_or("the content type names no boundary")?
        .trim_matches('"')
        .to_string();
    let mut out = Returned::default();
    let delimiter = format!("--{boundary}");
    let mut seen = 0usize;
    for piece in split(body, delimiter.as_bytes()) {
        // the preamble before the first delimiter, and the epilogue after
        // the last, are not parts
        let piece = strip_prefix(piece, b"\r\n").unwrap_or(piece);
        if piece.is_empty() || piece.starts_with(b"--") {
            continue;
        }
        let end = find(piece, b"\r\n\r\n").ok_or("a part with no end of headers")?;
        let head = String::from_utf8_lossy(&piece[..end]).to_string();
        let value = trim_end(&piece[end + 4..], b"\r\n");
        let name = head
            .split("name=\"")
            .nth(1)
            .and_then(|r| r.split('"').next())
            .ok_or("a part with no name")?
            .to_string();
        let media = head.to_ascii_lowercase().lines().find_map(|l| {
            l.strip_prefix("content-type:")
                .map(|v| v.trim().to_string())
        });
        seen += 1;
        if seen > 32 {
            return Err("more parts than an answer carries".into());
        }
        let text = || String::from_utf8_lossy(value).to_string();
        match name.as_str() {
            "utility" => out.utility = text(),
            "token" => out.token = text(),
            "at" => out.at = text().parse().map_err(|_| "`at` is not a number")?,
            "outcome" => match text().as_str() {
                "given" => out.given = true,
                "declined" => out.given = false,
                other => return Err(format!("an outcome that is neither: {other}")),
            },
            "reason" => out.reason = Some(text()),
            "result" => {
                out.result = Some(value.to_vec());
                out.media = media;
            }
            "digest" => {
                out.digest = Some(unhex32(&text()).ok_or("the digest is not 32 hex bytes")?)
            }
            "key" => out.key = Some(unhex32(&text()).ok_or("the key is not 32 hex bytes")?),
            "claim" => out.claim = Some(unhex(&text()).ok_or("the claim is not hex")?),
            // a part this application does not know is not a failure: the
            // contract may gain one, and the ones it names are here
            _ => {}
        }
    }
    if out.utility.is_empty() || out.token.is_empty() {
        return Err("the answer names no utility or no token".into());
    }
    Ok(out)
}

/// The bytes a claim signs (§3.4): the CBOR array the client signed, built
/// from this side's own view of what it asked and what came back.
pub fn claim_tbs(
    audience: &[u8; 32],
    utility: &str,
    token: &str,
    digest: &[u8; 32],
    at: u64,
) -> Vec<u8> {
    use rhtn_codec::encode::*;
    let mut out = Vec::with_capacity(128);
    emit_array_head(&mut out, 6);
    emit_tstr(&mut out, CLAIM_TAG);
    emit_bstr(&mut out, audience);
    emit_tstr(&mut out, utility);
    emit_tstr(&mut out, token);
    emit_bstr(&mut out, digest);
    emit_uint(&mut out, at);
    out
}

/// **What this application makes of what came back**: whether the utility
/// and token are the ones asked for, whether the digest is over the result
/// it was handed, and whether the signature verifies for *this* resource.
pub fn judge(me: &[u8; 32], step: &Step, got: &Returned) -> Claimed {
    if got.token != step.token {
        return Claimed::Failed(format!(
            "the answer echoes {} and the ask said {}",
            got.token, step.token
        ));
    }
    if got.utility != step.utility {
        return Claimed::Failed(format!(
            "the answer names {} and the ask said {}",
            got.utility, step.utility
        ));
    }
    let signs = matches!(step.utility, "capture" | "sign");
    let claimed = got.digest.is_some() || got.key.is_some() || got.claim.is_some();
    if !got.given {
        return if claimed {
            Claimed::Failed("a refusal carrying a claim".into())
        } else {
            Claimed::Nothing
        };
    }
    if !signs {
        return if claimed {
            Claimed::Failed(format!(
                "{} carries a claim, which nothing signs",
                step.utility
            ))
        } else {
            Claimed::Nothing
        };
    }
    let (Some(digest), Some(key), Some(claim)) = (got.digest, got.key, got.claim.as_deref()) else {
        return Claimed::Failed("a capture or a signature with no claim on it".into());
    };
    // what the digest must be over: the result where one came back, and
    // the exact payload the ask carried for a signature
    let over: Vec<u8> = match (step.payload, got.result.as_deref()) {
        (Some(p), _) => p.as_bytes().to_vec(),
        (None, Some(r)) => r.to_vec(),
        (None, None) => return Claimed::Failed("nothing to have signed".into()),
    };
    if sha256(&over) != digest {
        return Claimed::Failed("the digest is not over what came back".into());
    }
    if !rhtn_crypto::verify::verify_ed_raw(
        &key,
        claim,
        &claim_tbs(me, step.utility, step.token, &digest, got.at),
    ) {
        return Claimed::Failed("the signature does not verify for this resource".into());
    }
    Claimed::Verified
}

/// One client's walk through the sequence.
#[derive(Debug, Default, Clone)]
pub struct Walk {
    /// The step it is on.
    pub at: usize,
    /// What each answered step gave, in order.
    pub taken: Vec<(String, String, Claimed, Option<String>, usize)>,
}

/// The test resource: one walk per principal the gateway names.
#[derive(Debug, Default)]
pub struct App {
    /// This resource's own keyhash, which every claim is addressed to.
    pub me: [u8; 32],
    /// Where each client has got to.
    pub walks: BTreeMap<String, Walk>,
    /// Every line this application would write to its log.
    pub said: Vec<String>,
}

impl App {
    /// The test resource for the keyhash it is hosted under.
    pub fn new(me: [u8; 32]) -> App {
        App {
            me,
            walks: BTreeMap::new(),
            said: Vec::new(),
        }
    }

    /// **One request in, one response out** (`wire-format.md` §11): the
    /// whole application, so a test drives the sequence with no socket.
    pub fn handle(&mut self, request: &[u8]) -> Vec<u8> {
        let Some((method, target, headers, body)) = read_request(request) else {
            return response(
                400,
                "text/plain",
                b"this is not one HTTP/1.1 request\n".to_vec(),
            );
        };
        let principal = header(&headers, "rhtn-principal").unwrap_or_else(|| "unknown".into());
        let roles = header(&headers, "rhtn-roles").unwrap_or_default();
        match method.as_str() {
            "GET" => {
                let walk = self.walks.entry(principal.clone()).or_default();
                if walk.at >= STEPS.len() {
                    let page = summary(&principal, &roles, walk);
                    self.say(&principal, "summary", "");
                    return response(200, "text/html", page.into_bytes());
                }
                let step = &STEPS[walk.at];
                self.say(&principal, "ask", step.token);
                response(200, ASK_MEDIA, ask(step).into_bytes())
            }
            "POST" => {
                let token = target.strip_prefix("/u/").unwrap_or("").to_string();
                let kind = header(&headers, "content-type").unwrap_or_default();
                let walk = self.walks.entry(principal.clone()).or_default();
                let Some(step) = STEPS.get(walk.at) else {
                    self.say(&principal, "late", &token);
                    return response(409, "text/plain", b"this walk is finished\n".to_vec());
                };
                if token != step.token {
                    let said = format!(
                        "this walk is on {} and that answer names {token}\n",
                        step.token
                    );
                    self.say(&principal, "crossed", &token);
                    return response(409, "text/plain", said.into_bytes());
                }
                let got = match parse_answer(&kind, &body) {
                    Ok(g) => g,
                    Err(why) => {
                        self.say(&principal, "unreadable", &why);
                        return response(400, "text/plain", format!("{why}\n").into_bytes());
                    }
                };
                let verdict = judge(&self.me, step, &got);
                let size = got.result.as_ref().map(|r| r.len()).unwrap_or(0);
                let shown = got.media.clone().or_else(|| {
                    got.result
                        .as_ref()
                        .map(|r| String::from_utf8_lossy(r).to_string())
                });
                let walk = self.walks.get_mut(&principal).expect("entered above");
                walk.taken.push((
                    step.utility.to_string(),
                    if got.given {
                        "given".into()
                    } else {
                        format!("declined: {}", got.reason.clone().unwrap_or_default())
                    },
                    verdict.clone(),
                    shown,
                    size,
                ));
                walk.at += 1;
                let now = walk.at;
                let done = now >= STEPS.len();
                self.say(
                    &principal,
                    if got.given { "given" } else { "declined" },
                    &format!("{} {:?} {size}", step.token, verdict),
                );
                // **the file at the end**: an `export` the person agreed
                // to is answered with the record itself, declared as what
                // it is so their client offers to save it rather than
                // drawing it
                if done {
                    let walk = self.walks.get(&principal).expect("entered above");
                    return response(200, "text/csv", record(&principal, walk).into_bytes());
                }
                let next = &STEPS[now];
                self.say(&principal, "ask", next.token);
                response(200, ASK_MEDIA, ask(next).into_bytes())
            }
            _ => response(
                405,
                "text/plain",
                b"this resource takes GET and POST\n".to_vec(),
            ),
        }
    }

    fn say(&mut self, principal: &str, what: &str, detail: &str) {
        self.said.push(format!(
            "{{\"who\":\"{}\",\"what\":\"{what}\",\"detail\":\"{}\"}}",
            &principal[..principal.len().min(16)],
            detail.replace('"', "'")
        ));
    }
}

/// The page a finished walk is shown on a later request.
pub fn summary(principal: &str, roles: &str, walk: &Walk) -> String {
    let mut out = String::from("<h1>The utility test</h1>");
    out.push_str(&format!(
        "<p>You are <code>{}…</code>, holding <code>{}</code>. \
         Every utility this application asked for, and what came back.</p>",
        &principal[..principal.len().min(16)],
        if roles.is_empty() { "no role" } else { roles }
    ));
    out.push_str(
        "<table><tr><th>utility</th><th>outcome</th><th>claim</th><th>what</th><th>bytes</th></tr>",
    );
    for (utility, outcome, claimed, shown, size) in &walk.taken {
        out.push_str(&format!(
            "<tr><td>{utility}</td><td>{outcome}</td><td>{}</td><td>{}</td><td>{size}</td></tr>",
            match claimed {
                Claimed::Nothing => "none".to_string(),
                Claimed::Verified => "verified".to_string(),
                Claimed::Failed(why) => format!("FAILED: {why}"),
            },
            shown
                .clone()
                .unwrap_or_default()
                .chars()
                .take(60)
                .collect::<String>(),
        ));
    }
    out.push_str("</table>");
    out
}

/// The record handed over as the file: the same thing in a form that
/// leaves the device.
pub fn record(principal: &str, walk: &Walk) -> String {
    let mut out = String::from("utility,outcome,claim,what,bytes\n");
    for (utility, outcome, claimed, shown, size) in &walk.taken {
        out.push_str(&format!(
            "{utility},{},{},{},{size}\n",
            outcome.replace(',', ";"),
            match claimed {
                Claimed::Nothing => "none".to_string(),
                Claimed::Verified => "verified".to_string(),
                Claimed::Failed(why) => format!("failed: {}", why.replace(',', ";")),
            },
            shown
                .clone()
                .unwrap_or_default()
                .replace(',', ";")
                .chars()
                .take(60)
                .collect::<String>(),
        ));
    }
    out.push_str(&format!("# for {principal}\n"));
    out
}

// ---- the small amount of HTTP an application of this size needs --------

/// One response, framed by its length, which is what §11 carries.
pub fn response(status: u16, media: &str, body: Vec<u8>) -> Vec<u8> {
    let reason = match status {
        200 => "OK",
        400 => "Bad Request",
        405 => "Method Not Allowed",
        409 => "Conflict",
        _ => "Unknown",
    };
    let mut out = format!(
        "HTTP/1.1 {status} {reason}\r\ncontent-type: {media}\r\ncontent-length: {}\r\n\r\n",
        body.len()
    )
    .into_bytes();
    out.extend_from_slice(&body);
    out
}

type Parsed = (String, String, Vec<(String, String)>, Vec<u8>);

/// Read one request: the method, the target, the headers lower-cased, and
/// the body its length declares.
pub fn read_request(bytes: &[u8]) -> Option<Parsed> {
    let end = find(bytes, b"\r\n\r\n")?;
    let head = std::str::from_utf8(&bytes[..end]).ok()?;
    let mut lines = head.split("\r\n");
    let mut first = lines.next()?.split(' ');
    let method = first.next()?.to_string();
    let target = first.next()?.to_string();
    let mut headers = Vec::new();
    for line in lines {
        let (name, value) = line.split_once(':')?;
        headers.push((name.trim().to_ascii_lowercase(), value.trim().to_string()));
    }
    let want: usize = headers
        .iter()
        .find(|(n, _)| n == "content-length")
        .and_then(|(_, v)| v.parse().ok())
        .unwrap_or(0);
    let body = bytes[end + 4..].to_vec();
    if body.len() < want {
        return None;
    }
    Some((method, target, headers, body[..want].to_vec()))
}

/// How long the whole request is, where its head has arrived: the reader
/// in `main.rs` keeps reading until it has this much.
pub fn wanted(bytes: &[u8]) -> Option<usize> {
    let end = find(bytes, b"\r\n\r\n")?;
    let head = std::str::from_utf8(&bytes[..end])
        .ok()?
        .to_ascii_lowercase();
    let length: usize = head
        .lines()
        .find_map(|l| l.strip_prefix("content-length:"))
        .and_then(|v| v.trim().parse().ok())
        .unwrap_or(0);
    Some(end + 4 + length)
}

fn header(headers: &[(String, String)], name: &str) -> Option<String> {
    headers
        .iter()
        .find(|(n, _)| n == name)
        .map(|(_, v)| v.clone())
}

fn find(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack.windows(needle.len()).position(|w| w == needle)
}

fn split<'a>(body: &'a [u8], delimiter: &[u8]) -> Vec<&'a [u8]> {
    let mut out = Vec::new();
    let mut at = 0usize;
    while let Some(i) = find(&body[at..], delimiter) {
        out.push(&body[at..at + i]);
        at += i + delimiter.len();
    }
    out.push(&body[at..]);
    out
}

fn strip_prefix<'a>(b: &'a [u8], prefix: &[u8]) -> Option<&'a [u8]> {
    b.starts_with(prefix).then(|| &b[prefix.len()..])
}

fn trim_end<'a>(b: &'a [u8], suffix: &[u8]) -> &'a [u8] {
    if b.ends_with(suffix) {
        &b[..b.len() - suffix.len()]
    } else {
        b
    }
}

/// Lower-case hex.
pub fn hex(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect()
}

/// Bytes out of hex.
pub fn unhex(s: &str) -> Option<Vec<u8>> {
    if !s.len().is_multiple_of(2) {
        return None;
    }
    (0..s.len() / 2)
        .map(|i| u8::from_str_radix(s.get(i * 2..i * 2 + 2)?, 16).ok())
        .collect()
}

fn unhex32(s: &str) -> Option<[u8; 32]> {
    unhex(s)?.try_into().ok()
}
