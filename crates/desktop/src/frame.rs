//! **A resource's page, framed** (`light-client-requirements.md` §8)
//! [author, 2026-11-15]: the phone's frame (`ResourcePage.kt`,
//! `ResourceActivity.kt`), drawn the same way in a window of the desktop's
//! own — the pure part, so a test can hold it with no window present.
//!
//! **The policy is the phone's, and the engine is the one that differs.**
//!
//!  - **An origin of its own**: [`origin`] mints one per identity and
//!    resource under a scheme the window registers and a name no network
//!    resolves, and each such window has a web context of its own under
//!    [`storage`], so what a page stores is its own as that identity,
//!    kept, and gone when the directory is.
//!  - **The same resource, and nothing else**: every load inside that
//!    origin is composed here ([`compose`]) and carried by the kernel to the
//!    resource by the node that served it; a navigation anywhere else is
//!    not followed, and what a page fetches from anywhere else is refused
//!    by the policy [`served`] puts on every answer, which the engine
//!    enforces beside the page's own.
//!  - **Script by declaration and consent**: a page runs code only where
//!    its own policy names `script-src` and the person allowed it for that
//!    resource, asked once ([`SCRIPT_ASKS`]) and remembered
//!    ([`script_allowed`]). Otherwise the policy here says `script-src
//!    'none'`, which the engine holds whatever the markup carries.
//!  - **One object, no bridge**: a page that runs is given `rhtn.request`
//!    ([`bridge`]), the phone's object with the phone's answers, carried
//!    over the same scheme. Nothing of this client is published into the
//!    page, and the window has no message handler a page could reach.
//!
//! **Where the engine gives more than the phone's**: WebKitGTK hands the
//! scheme a request's body, so a form that posts, and a page's own `fetch`
//! with a body, are carried as they are; `rhtn.request` is there so a page
//! written for the phone runs here unchanged.

use rhtn_ffi::client::{PageShape, ResourceAnswer, ResourcePage};
use std::path::{Path, PathBuf};

/// The scheme every resource's origin is under.
pub const SCHEME: &str = "rhtnr";

/// The methods a frame carries: the phone's five.
pub const METHODS: [&str; 5] = ["GET", "POST", "PUT", "PATCH", "DELETE"];

/// **The reason a declined ask carries** (§8.1): a capability this device
/// has not got is a refusal saying so, never a silence.
pub const NO_UTILITY: &str = "this device has no such utility";

/// How many bytes of the digest an origin's one label carries: 62
/// characters, and a label holds 63.
pub const LABEL: usize = 31;

/// **The origin minted for a resource as one identity opens it**
/// [author, 2026-11-26]: one label, a digest of the identity and the
/// resource, under a name no network resolves, as the phone mints one
/// (`Frames.origin`). The page reads neither from its own address, and
/// what it stored as one identity is not there as another.
pub fn origin(by: &str, resource: &str) -> Result<String, String> {
    let (who, what) = (by.to_ascii_lowercase(), resource.to_ascii_lowercase());
    let keyhash = |hex: &str| hex.len() == 64 && hex.bytes().all(|b| b.is_ascii_hexdigit());
    if !keyhash(&who) {
        return Err("an identity is 64 hex digits".into());
    }
    if !keyhash(&what) {
        return Err("a resource is 64 hex digits".into());
    }
    let digest = rhtn_codec::cose::sha256(format!("{who} {what}").as_bytes());
    let label: String = digest[..LABEL].iter().map(|b| format!("{b:02x}")).collect();
    Ok(format!("{SCHEME}://{label}.rhtn.invalid"))
}

/// **Whether `url` is inside `origin`**: the origin itself, or a path
/// rooted under it. A prefix of the host is not inside.
pub fn inside(origin: &str, url: &str) -> bool {
    url == origin
        || url
            .strip_prefix(origin)
            .is_some_and(|rest| rest.starts_with(['/', '?', '#']))
}

/// **What a frame carries, or why it does not**: one of [`METHODS`], to a
/// path rooted on the resource and not under `/.rhtn/`, which is the
/// node's own and never a page's (`resource-requirements.md` §3.3).
pub fn carried(method: &str, path: &str) -> Result<(), String> {
    if !METHODS.contains(&method) {
        return Err(format!("a method this frame does not carry: {method}"));
    }
    if !path.starts_with('/') || path.starts_with("/.rhtn/") {
        return Err("a path is rooted on this resource and not under /.rhtn/".into());
    }
    Ok(())
}

/// **The application's own HTTP/1.1 request**, as the phone composes one
/// (`Gateway.requestAs`): the method and path, a host the node rewrites,
/// and a body's type and length where there is a body.
pub fn compose(method: &str, path: &str, media: Option<&str>, body: &[u8]) -> Vec<u8> {
    let mut head = format!("{method} {path} HTTP/1.1\r\nhost: resource\r\n");
    if !body.is_empty() || method != "GET" {
        if let Some(t) = media.filter(|t| !t.is_empty()) {
            head.push_str(&format!("content-type: {t}\r\n"));
        }
        head.push_str(&format!("content-length: {}\r\n", body.len()));
    }
    head.push_str("\r\n");
    let mut out = head.into_bytes();
    out.extend_from_slice(body);
    out
}

/// What the delivery status of `wire-format.md` §11.1 says, in the
/// phone's words.
pub fn delivery(status: u64) -> String {
    match status {
        0 => "delivered".into(),
        1 => "refused".into(),
        2 => "the resource is unavailable".into(),
        3 => "the request was malformed".into(),
        4 => "no subtree acknowledgement covers you".into(),
        5 => "no role of yours matches".into(),
        n => format!("status {n}"),
    }
}

/// **This client's sheet, which the page's own rules override**: the
/// phone's, rule for rule. Every selector is a bare element, so anything
/// the resource styles for itself wins on specificity alone.
pub const SHEET: &str = "html{background:#1C1F24;color:#E8EAED;-webkit-text-size-adjust:100%}\
body{margin:0;padding:16px;font:15px/1.5 -apple-system,Roboto,'Helvetica Neue',Arial,sans-serif}\
h1{font-size:21px;margin:0 0 10px}\
h2{font-size:17px;margin:18px 0 8px}\
h3{font-size:15px;margin:16px 0 6px}\
p,ul,ol,dl,blockquote,pre,table{margin:0 0 12px}\
a{color:#5FB3A1}\
small,figcaption{color:#9AA1AA}\
hr{border:0;border-top:1px solid #23272E;margin:16px 0}\
table{border-collapse:collapse;width:100%}\
th,td{border-bottom:1px solid #23272E;padding:8px 6px;text-align:left}\
th{color:#9AA1AA;font-weight:600;font-size:13px}\
pre,code,kbd{font-family:'Roboto Mono',monospace;font-size:13px}\
pre{overflow-x:auto;background:#15171A;padding:10px;border-radius:8px}\
img,video,canvas,svg{max-width:100%;height:auto}\
input,select,textarea,button{font:inherit;color:#E8EAED;background:#15171A;border:1px solid #3A3F47;border-radius:8px;padding:9px 10px}\
button{background:#2E3540;min-height:44px}\
blockquote{border-left:2px solid #23272E;padding-left:11px;color:#9AA1AA}";

/// **The page as the frame loads it**: this client's sheet, then the
/// resource's own markup unaltered — the phone's `styled`. The markup is
/// not rewritten: a client that edited somebody else's document to make it
/// safe would be asserting it understood the document.
pub fn styled(markup: &str) -> String {
    format!(
        "<!doctype html><html><head><meta name=\"viewport\" \
         content=\"width=device-width,initial-scale=1\"><style>{SHEET}</style></head>\
         <body>{markup}</body></html>"
    )
}

/// **The policy this client puts on every answer**, beside the page's
/// own: the engine holds both, so a page can narrow it and never widen
/// it. Everything is fetched from the page's own origin or from what it
/// made itself; script runs only where `script` says it may.
pub fn policy(script: bool) -> String {
    let scripts = if script {
        "script-src 'self' 'unsafe-inline' 'unsafe-eval' blob:"
    } else {
        "script-src 'none'"
    };
    format!(
        "default-src 'self' data: blob:; {scripts}; style-src 'self' 'unsafe-inline'; \
         form-action 'self'; base-uri 'self'; frame-ancestors 'self'"
    )
}

/// **One answer as the frame's engine receives it**: a status, the
/// headers, and the body.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Served {
    /// The HTTP status the engine is given.
    pub status: u16,
    /// Every header, in order; a name may repeat.
    pub headers: Vec<(String, String)>,
    /// The body.
    pub body: Vec<u8>,
}

/// The header a frame's answer carries where the kernel carried nothing,
/// saying why in the phone's words: `rhtn.request` reads it as its
/// `error`, percent-encoded, since a reason is not always ASCII.
pub const CARRIED: &str = "rhtn-carried";

/// The header marking an answer whose type was not declared, so
/// `rhtn.request` says `media: null` as the phone's does.
pub const UNDECLARED: &str = "rhtn-undeclared";

/// **An answer that carried nothing**, said as text and named in
/// [`CARRIED`].
pub fn not_carried(why: &str) -> Served {
    Served {
        status: 502,
        headers: vec![
            ("content-type".into(), "text/plain; charset=utf-8".into()),
            ("x-content-type-options".into(), "nosniff".into()),
            ("content-security-policy".into(), policy(false)),
            (CARRIED.into(), escape(why)),
        ],
        body: format!("{why}\n").into_bytes(),
    }
}

/// **Answered with nothing**: what a load outside the origin gets — an
/// empty body rather than an error the page could time.
pub fn nothing() -> Served {
    Served {
        status: 200,
        headers: vec![("content-type".into(), "text/plain".into())],
        body: Vec::new(),
    }
}

/// **Whether a request is the page moving to its answer** rather than its
/// script or its markup asking for one: a navigation is the one request
/// whose `accept` leads with `text/html`, and only a navigation shows its
/// answer as the page.
pub fn navigating(accept: Option<&str>) -> bool {
    accept.is_some_and(|a| a.trim_start().starts_with("text/html"))
}

/// **What the frame shows in place of an answer this desktop does not
/// show** (`light-client-requirements.md` §8): its declared type and its
/// size, never a guess, and where it went — offered for saving in the
/// desktop's own window, since nothing in the frame reaches a file — and
/// the way `back` to the page it was reached from, the frame having no
/// controls of its own [found 2026-11-27].
pub fn not_shown(page: &ResourcePage, back: &str) -> Served {
    let said = format!(
        "<p>The resource answered with {}. It is offered for saving in this desktop's own \
         window.</p>",
        escape_html(&named(page))
    );
    own_page(200, &said, back)
}

/// **A page moved to that was not carried**, said in the frame with the
/// way `back`, where a script's request gets [`not_carried`]'s text.
pub fn not_reached(why: &str, back: &str) -> Served {
    let mut s = own_page(502, &format!("<p>{}</p>", escape_html(why)), back);
    s.headers.push((CARRIED.into(), escape(why)));
    s
}

/// **Where a page of the frame's own leads back to**: the last page the
/// frame showed, a path of its own origin, else its first.
pub fn way_back(back: &str) -> String {
    let back = if back.starts_with('/') && !back.starts_with("//") {
        back
    } else {
        "/"
    };
    format!(
        "<p><a href=\"{}\">Back to the page you came from</a></p>",
        escape_html(back)
    )
}

/// One of the frame's own pages, running nothing.
fn own_page(status: u16, said: &str, back: &str) -> Served {
    Served {
        status,
        headers: vec![
            ("content-type".into(), "text/html; charset=utf-8".into()),
            ("x-content-type-options".into(), "nosniff".into()),
            ("content-security-policy".into(), policy(false)),
        ],
        body: styled(&format!("{said}{}", way_back(back))).into_bytes(),
    }
}

/// **Whether an answer is a page the frame shows**, and so where its own
/// pages lead back to: a navigation answered with markup.
pub fn shows(s: &Served) -> bool {
    (200..300).contains(&s.status)
        && s.headers
            .iter()
            .any(|(n, v)| n == "content-type" && v.starts_with("text/html"))
}

/// **The question an `import` ask is put to the person as** (§8.1): the
/// resource's purpose and retention in its own words, marked as its, and
/// the types it takes. A yes opens the file picker; the file picked is
/// what is handed over, and nothing else of this computer is read.
pub fn import_question(name: &str, purpose: &str, retention: &str, media: &[String]) -> String {
    let types = if media.is_empty() {
        "any type".to_string()
    } else {
        media.join(" or ")
    };
    question(
        name,
        &format!("for a file you pick ({types})"),
        purpose,
        retention,
        "Pick a file",
    )
}

/// **One ask put to the person**: what it asks for, then its purpose and
/// its retention as the resource's own words, marked as words this
/// desktop cannot hold it to, then the act.
pub fn question(name: &str, what: &str, purpose: &str, retention: &str, act: &str) -> String {
    format!(
        "{name} asks {what}. It says it is for: \u{201c}{purpose}\u{201d}, and that it keeps it: \
         \u{201c}{retention}\u{201d}. Those are its words, which this desktop cannot hold it to. \
         {act}?"
    )
}

/// **What the person reads of a payload they are asked to sign**: the
/// text itself where it is text, and otherwise its size and its first
/// bytes, never a summary of the resource's.
pub fn payload_shown(payload: &[u8]) -> String {
    match std::str::from_utf8(payload) {
        Ok(t) if !t.chars().any(|c| c.is_control() && c != '\n' && c != '\t') => {
            format!("\u{201c}{t}\u{201d}")
        }
        _ => {
            let head: String = payload
                .iter()
                .take(32)
                .map(|b| format!("{b:02x}"))
                .collect();
            format!(
                "{} bytes that are not text, beginning {head}{}",
                payload.len(),
                if payload.len() > 32 { "\u{2026}" } else { "" }
            )
        }
    }
}

/// **Why a paired desktop declines a signature** [author, 2026-11-24]: a
/// restored desktop signs and a paired one does not, the signing device
/// being the phone.
pub const PAIRED_SIGNS_NOT: &str = "this desktop was paired from the phone and signs nothing; \
                                     the phone signs";

/// **The media types a scan may be handed over as here**: the images the
/// ask takes, or every image where it names none or takes anything.
pub fn scan_media(asked: &[String]) -> Vec<String> {
    if asked.is_empty() || asked.iter().any(|a| a == "*/*") {
        return vec!["image/*".into()];
    }
    asked
        .iter()
        .filter(|a| a.starts_with("image/"))
        .cloned()
        .collect()
}

/// **Which resources have more waiting than when last seen, with a line
/// to say so** [author, 2026-11-23], as the phone's `Notify.rising`: the
/// count is the resource's own, a rise is something new, and a resource
/// that gave no line has nothing to put in a notice. `seen` is by the
/// resource in hex.
#[must_use]
pub fn rising<'a>(
    seen: &std::collections::BTreeMap<String, u64>,
    held: &'a [rhtn_model::Resource],
) -> Vec<&'a rhtn_model::Resource> {
    held.iter()
        .filter(|r| {
            r.notice.is_some()
                && u64::from(r.waiting) > seen.get(&hex_of(&r.resource)).copied().unwrap_or(0)
        })
        .collect()
}

/// Lower-case hex.
fn hex_of(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect()
}

/// **Whether this resource holds the person's standing permission to
/// show its notifications** (§8.1's `notify`), kept per resource as the
/// phone keeps it.
pub fn notify_allowed(state: &Path, resource: &str) -> bool {
    std::fs::read_to_string(dir(state, resource).join("notify"))
        .map(|t| t.trim() == "allowed")
        .unwrap_or(false)
}

/// The permission, kept for the resource.
pub fn keep_notify(state: &Path, resource: &str) -> Result<(), String> {
    let d = dir(state, resource);
    crate::platform::create_private(&d)?;
    std::fs::write(d.join("notify"), "allowed\n").map_err(|e| format!("{}: {e}", d.display()))
}

/// **Whether an ask takes `media`**, as the phone decides it: an ask
/// naming no type takes what it is given, and `*/*` or `image/*` take
/// what they say.
pub fn takes(asked: &[String], media: &str) -> bool {
    let major = media.split('/').next().unwrap_or("");
    asked.is_empty()
        || asked
            .iter()
            .any(|a| a == media || a == "*/*" || *a == format!("{major}/*"))
}

/// **The name a save dialog suggests** for what `name` answered with: the
/// resource's name with only letters, digits, dots, dashes and
/// underscores kept, and an extension from the declared type's subtype
/// where that is a short word — `text/csv` gives `.csv` — and none where
/// nothing useful is declared.
pub fn suggested(name: &str, media: Option<&str>) -> String {
    let stem: String = name
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_') {
                c
            } else {
                '-'
            }
        })
        .collect();
    let stem = stem.trim_matches(|c| c == '-' || c == '.');
    let stem = if stem.is_empty() { "resource" } else { stem };
    let ext = media
        .and_then(|m| m.split(';').next())
        .and_then(|m| m.trim().split('/').nth(1))
        .map(|s| s.split('+').next().unwrap_or(s).to_ascii_lowercase())
        .filter(|s| (1..=8).contains(&s.len()) && s.chars().all(|c| c.is_ascii_alphanumeric()))
        .filter(|s| s != "octet")
        .map(|s| if s == "plain" { "txt".to_string() } else { s });
    match ext {
        Some(e) if !stem.ends_with(&format!(".{e}")) => format!("{stem}.{e}"),
        _ => stem.to_string(),
    }
}

/// Markup-safe text.
fn escape_html(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

/// **What the engine is given for one answer**, `allowed` being whether
/// the person allowed this resource's code. The page's own type is the one
/// declared and nothing else; an undeclared one is octet-stream with
/// sniffing off, so the engine renders nothing it guessed at.
pub fn served(answer: &ResourceAnswer, allowed: bool) -> Served {
    match (undelivered(answer), &answer.page) {
        (None, Some(page)) => page_served(page, allowed),
        (why, _) => not_carried(&why.unwrap_or_default()),
    }
}

/// **Why an answer carried no page**, where it did not: not delivered,
/// not readable, or nothing.
pub fn undelivered(answer: &ResourceAnswer) -> Option<String> {
    if answer.status != 0 {
        return Some(format!("not delivered: {}", delivery(answer.status)));
    }
    if let Some(why) = &answer.unreadable {
        return Some(format!("the answer could not be read: {why}"));
    }
    answer
        .page
        .is_none()
        .then(|| "the resource answered with nothing".to_string())
}

fn page_served(page: &ResourcePage, allowed: bool) -> Served {
    let script = allowed && page.runs_script;
    let mut headers = Vec::new();
    let utf8 = page.charset.as_deref().is_none_or(|c| c == "utf-8");
    match &page.media {
        Some(m) => headers.push((
            "content-type".into(),
            match &page.charset {
                Some(c) => format!("{m}; charset={c}"),
                None => m.clone(),
            },
        )),
        None => {
            headers.push(("content-type".into(), "application/octet-stream".into()));
            headers.push((UNDECLARED.into(), "1".into()));
        }
    }
    headers.push(("x-content-type-options".into(), "nosniff".into()));
    headers.push(("content-security-policy".into(), policy(script)));
    if let Some(p) = &page.policy {
        headers.push(("content-security-policy".into(), p.clone()));
    }
    let body = match (&page.shape, std::str::from_utf8(&page.body)) {
        (PageShape::Page, Ok(markup)) if utf8 => styled(markup).into_bytes(),
        _ => page.body.clone(),
    };
    Served {
        status: u16::try_from(page.http_status)
            .ok()
            .filter(|s| (100..=599).contains(s))
            .unwrap_or(502),
        headers,
        body,
    }
}

/// **What a first answer that is not shown is, said to the person**: its
/// declared type and its size, since *this is a PDF of 40 kB* tells them
/// what happened where *could not display* does not (§8).
pub fn named(page: &ResourcePage) -> String {
    let what = page
        .media
        .as_deref()
        .map_or("nothing declared".to_string(), |m| format!("`{m}`"));
    let n = page.body.len();
    let size = if n < 1024 {
        format!("{n} bytes")
    } else {
        format!("{} kB", n.div_ceil(1024))
    };
    format!("an answer of {what}, {size}, which this desktop does not show")
}

/// **The question, once per resource**, where a page declares that it
/// runs code and the person has not answered yet: the phone's words.
pub const SCRIPT_ASKS: &str = "This resource's pages run their own code. In its window, code \
     reaches the resource it came from and nothing else — not this desktop, not your files, \
     not another resource — and keeps storage of its own on this computer. Allow it for this \
     resource?";

/// **Where one resource's frames keep what they keep**: the person's
/// answer about its code, and beneath it what its pages stored as each
/// identity ([`storage`]), in a directory of its own, so forgetting the
/// resource is removing one directory.
pub fn dir(state: &Path, resource: &str) -> PathBuf {
    state.join("frames").join(resource.to_ascii_lowercase())
}

/// **Where a resource's pages store what they store as one identity**
/// [author, 2026-11-26]: a web context of its own, so nothing a page kept
/// as one identity is read as another.
pub fn storage(state: &Path, by: &str, resource: &str) -> PathBuf {
    dir(state, resource).join(by.to_ascii_lowercase())
}

/// **Whether the person allowed this resource's pages to run their
/// code**: their answer once they gave one, `None` before — and `None` is
/// what puts the question.
pub fn script_allowed(state: &Path, resource: &str) -> Option<bool> {
    match std::fs::read_to_string(dir(state, resource).join("script"))
        .ok()?
        .trim()
    {
        "allowed" => Some(true),
        "refused" => Some(false),
        _ => None,
    }
}

/// The person's answer, kept for the resource.
pub fn keep_script(state: &Path, resource: &str, allowed: bool) -> Result<(), String> {
    let d = dir(state, resource);
    crate::platform::create_private(&d)?;
    std::fs::write(
        d.join("script"),
        if allowed { "allowed\n" } else { "refused\n" },
    )
    .map_err(|e| format!("{}: {e}", d.display()))
}

/// **`rhtn.request`, as the phone publishes it** (`ResourceBridge.kt`):
/// `request(method, path, type, body)` answers with one JSON string —
/// `status`, `media` and `body` as base64, or `error` where the kernel
/// carried nothing. Synchronous, as the phone's is, and carried over the
/// frame's own scheme to its own origin, so the kernel is still the only
/// network. Installed only where the page may run.
pub fn bridge(origin: &str) -> String {
    format!(
        r#"(() => {{
  const ORIGIN = {origin:?};
  const METHODS = {methods};
  const fail = error => JSON.stringify({{ error }});
  const b64 = s => {{
    let bin = '';
    for (let i = 0; i < s.length; i++) bin += String.fromCharCode(s.charCodeAt(i) & 0xff);
    return btoa(bin);
  }};
  const request = (method, path, type, body) => {{
    const m = String(method).toUpperCase();
    if (!METHODS.includes(m)) return fail('a method this frame does not carry: ' + method);
    const p = String(path);
    if (!p.startsWith('/') || p.startsWith('/.rhtn/'))
      return fail('a path is rooted on this resource and not under /.rhtn/');
    const x = new XMLHttpRequest();
    x.open(m, ORIGIN + p, false);
    x.overrideMimeType('text/plain; charset=x-user-defined');
    if (body != null && type != null) x.setRequestHeader('content-type', String(type));
    try {{ x.send(body == null ? null : String(body)); }}
    catch (e) {{ return fail('the kernel did not carry the request'); }}
    const carried = x.getResponseHeader('{carried}');
    if (carried) return fail(decodeURIComponent(carried));
    const media = x.getResponseHeader('{undeclared}') ? null
      : (x.getResponseHeader('content-type') || '').split(';')[0].trim().toLowerCase() || null;
    return JSON.stringify({{ status: x.status, media, body: b64(x.responseText) }});
  }};
  Object.defineProperty(window, 'rhtn', {{ value: Object.freeze({{ request }}) }});
}})();"#,
        methods = serde_json::to_string(&METHODS).expect("five strings"),
        carried = CARRIED,
        undeclared = UNDECLARED,
    )
}

/// Percent-encoding for a header value, as `decodeURIComponent` reads it.
fn escape(s: &str) -> String {
    s.bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' | b' ' | b':' => {
                (b as char).to_string()
            }
            _ => format!("%{b:02X}"),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    #[test]
    fn a_navigation_is_told_by_what_it_accepts() {
        assert!(navigating(Some(
            "text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8"
        )));
        assert!(!navigating(Some("*/*")), "a script's request");
        assert!(!navigating(None));
    }

    #[test]
    fn a_save_suggests_the_name_and_the_declared_kind() {
        assert_eq!(
            suggested("testapp.internal", Some("text/csv")),
            "testapp.internal.csv"
        );
        assert_eq!(
            suggested("A shop / ledger", Some("text/plain; charset=utf-8")),
            "A-shop---ledger.txt"
        );
        assert_eq!(suggested("x", Some("application/octet-stream")), "x");
        assert_eq!(suggested("x", Some("image/svg+xml")), "x.svg");
        assert_eq!(
            suggested("report.pdf", Some("application/pdf")),
            "report.pdf"
        );
        assert_eq!(suggested("///", None), "resource");
    }

    #[test]
    fn what_is_not_shown_is_named_and_runs_nothing() {
        let p = ResourcePage {
            http_status: 200,
            media: Some("text/csv".into()),
            charset: None,
            shape: PageShape::Other,
            policy: None,
            runs_script: false,
            body: vec![b'a'; 812],
        };
        let s = not_shown(&p, "/checks?x=\"1\"");
        let body = String::from_utf8(s.body.clone()).unwrap();
        assert!(body.contains("`text/csv`, 812 bytes"), "{body}");
        assert!(
            body.contains("<a href=\"/checks?x=&quot;1&quot;\">Back to the page you came from</a>"),
            "{body}"
        );
        assert!(
            s.headers
                .iter()
                .any(|(n, v)| n == "content-security-policy" && v.contains("script-src 'none'"))
        );
    }

    use super::*;

    const R: &str = "AB00000000000000000000000000000000000000000000000000000000000001";
    const ME: &str = "1100000000000000000000000000000000000000000000000000000000000002";

    fn page(
        media: Option<&str>,
        shape: PageShape,
        body: &str,
        policy: Option<&str>,
    ) -> ResourceAnswer {
        ResourceAnswer {
            status: 0,
            page: Some(ResourcePage {
                http_status: 200,
                media: media.map(String::from),
                charset: None,
                shape,
                runs_script: policy.is_some_and(|p| p.contains("script-src")),
                policy: policy.map(String::from),
                body: body.as_bytes().to_vec(),
            }),
            unreadable: None,
            ask: None,
        }
    }

    fn header<'a>(s: &'a Served, name: &str) -> Vec<&'a str> {
        s.headers
            .iter()
            .filter(|(n, _)| n == name)
            .map(|(_, v)| v.as_str())
            .collect()
    }

    #[test]
    fn a_resource_has_an_origin_of_its_own_and_a_prefix_of_it_is_outside() {
        let o = origin(ME, R).unwrap();
        // one label: a digest, the same at every start, saying neither
        // who nor what
        let label = o
            .strip_prefix("rhtnr://")
            .and_then(|h| h.strip_suffix(".rhtn.invalid"))
            .expect("the scheme and the name");
        assert_eq!(label.len(), 62);
        assert!(label.bytes().all(|b| b.is_ascii_hexdigit()));
        assert_eq!(o, origin(&ME.to_uppercase(), &R.to_uppercase()).unwrap());
        assert!(!o.contains(&R.to_lowercase()[..16]) && !o.contains(&ME[..16]));
        assert!(inside(&o, &o));
        assert!(inside(&o, &format!("{o}/a?b")));
        assert!(!inside(&o, &format!("{o}.evil/")));
        assert!(!inside(&o, "https://example.com/"));
        assert!(origin(ME, "ab").is_err());
        assert!(origin(ME, &"g".repeat(64)).is_err());
        assert!(origin("ab", R).is_err());
    }

    // functional: CAT-035
    #[test]
    fn one_resource_as_two_identities_is_two_origins_and_two_stores() {
        let other = "2".repeat(64);
        assert_ne!(origin(ME, R).unwrap(), origin(&other, R).unwrap());
        assert_ne!(origin(ME, R).unwrap(), origin(R, ME).unwrap());
        let state = Path::new("/state");
        let (mine, theirs) = (storage(state, ME, R), storage(state, &other, R));
        assert_ne!(mine, theirs);
        // both under the resource's one directory, beside its answer
        // about code, so forgetting the resource is still one removal
        assert!(mine.starts_with(dir(state, R)) && theirs.starts_with(dir(state, R)));
        assert_eq!(storage(state, &ME.to_uppercase(), R), mine);
    }

    #[test]
    fn a_frame_carries_the_five_methods_to_rooted_paths_and_never_under_rhtn() {
        assert!(carried("GET", "/").is_ok());
        assert!(carried("DELETE", "/x?y=1").is_ok());
        assert!(carried("HEAD", "/").is_err());
        assert!(carried("GET", "x").is_err());
        assert!(carried("POST", "/.rhtn/federation").is_err());
    }

    #[test]
    fn a_request_is_composed_as_the_phone_composes_one() {
        assert_eq!(
            compose("GET", "/a", None, b""),
            b"GET /a HTTP/1.1\r\nhost: resource\r\n\r\n"
        );
        assert_eq!(
            compose(
                "POST",
                "/f",
                Some("application/x-www-form-urlencoded"),
                b"a=1"
            ),
            b"POST /f HTTP/1.1\r\nhost: resource\r\ncontent-type: \
              application/x-www-form-urlencoded\r\ncontent-length: 3\r\n\r\na=1"
                .to_vec()
        );
        assert_eq!(
            compose("DELETE", "/d", None, b""),
            b"DELETE /d HTTP/1.1\r\nhost: resource\r\ncontent-length: 0\r\n\r\n"
        );
    }

    #[test]
    fn a_page_that_declares_no_script_runs_none_whatever_the_person_allowed() {
        let s = served(
            &page(Some("text/html"), PageShape::Page, "<p>hi</p>", None),
            true,
        );
        assert_eq!(header(&s, "content-security-policy").len(), 1);
        assert!(header(&s, "content-security-policy")[0].contains("script-src 'none'"));
        let body = String::from_utf8(s.body).unwrap();
        assert!(body.starts_with("<!doctype html>") && body.ends_with("<p>hi</p></body></html>"));
    }

    #[test]
    fn a_page_that_declares_script_runs_only_where_allowed_and_under_its_own_policy_too() {
        let a = page(
            Some("text/html"),
            PageShape::Page,
            "<p>hi</p>",
            Some("script-src 'self'"),
        );
        let refused = served(&a, false);
        assert!(header(&refused, "content-security-policy")[0].contains("script-src 'none'"));
        let allowed = served(&a, true);
        let policies = header(&allowed, "content-security-policy");
        assert_eq!(policies.len(), 2, "ours beside the page's own");
        assert!(policies[0].contains("script-src 'self'"));
        assert_eq!(policies[1], "script-src 'self'");
    }

    #[test]
    fn an_undeclared_type_is_never_guessed_at() {
        let s = served(&page(None, PageShape::Other, "<p>hi</p>", None), true);
        assert_eq!(header(&s, "content-type"), ["application/octet-stream"]);
        assert_eq!(header(&s, "x-content-type-options"), ["nosniff"]);
        assert_eq!(header(&s, UNDECLARED), ["1"]);
        assert_eq!(s.body, b"<p>hi</p>", "not styled, not touched");
    }

    #[test]
    fn what_the_kernel_did_not_carry_is_said_in_the_phones_words() {
        let mut a = page(Some("text/html"), PageShape::Page, "", None);
        a.status = 5;
        let s = served(&a, true);
        assert_eq!(s.status, 502);
        assert_eq!(
            header(&s, CARRIED),
            ["not delivered: no role of yours matches"]
        );
        a.status = 0;
        a.page = None;
        a.unreadable = Some("a folded header — refused".into());
        assert_eq!(
            header(&served(&a, true), CARRIED),
            ["the answer could not be read: a folded header %E2%80%94 refused"]
        );
    }

    #[test]
    fn the_frames_own_pages_lead_back_within_the_origin() {
        assert!(way_back("/checks").contains("href=\"/checks\""));
        assert!(way_back("//elsewhere.example/").contains("href=\"/\""));
        assert!(way_back("https://elsewhere.example/").contains("href=\"/\""));
        let s = not_reached("not delivered: timed out", "/checks");
        assert_eq!(s.status, 502);
        assert!(!shows(&s));
        assert_eq!(header(&s, CARRIED), ["not delivered: timed out"]);
        let body = String::from_utf8(s.body).unwrap();
        assert!(
            body.contains("<p>not delivered: timed out</p>") && body.contains("href=\"/checks\"")
        );
    }

    #[test]
    fn an_answer_not_shown_is_named_with_its_type_and_size() {
        let a = page(
            Some("application/pdf"),
            PageShape::Other,
            &"x".repeat(40_000),
            None,
        );
        assert_eq!(
            named(a.page.as_ref().unwrap()),
            "an answer of `application/pdf`, 40 kB, which this desktop does not show"
        );
    }

    /// **A notice is for a count that rose and came with a line**
    /// [author, 2026-11-23], as the phone's `Notify.rising`.
    // functional: CAT-030
    #[test]
    fn a_resource_is_noticed_where_its_count_rose_and_it_gave_a_line() {
        let held = |res: u8, waiting: u32, notice: Option<&str>| rhtn_model::Resource {
            node: [1; 32],
            resource: [res; 32],
            owner: [2; 32],
            service_type: "rhtn-http".into(),
            instance: format!("r{res}"),
            endpoint: Vec::new(),
            data_practice: None,
            truncated: false,
            stale: false,
            waiting,
            notice: notice.map(String::from),
            icon_format: None,
            icon: None,
        };
        let all = vec![
            held(3, 2, Some("two new")),
            held(4, 5, None),
            held(5, 1, Some("the same")),
            held(6, 0, Some("fell")),
        ];
        let seen: std::collections::BTreeMap<String, u64> =
            [("05".repeat(32), 1), ("06".repeat(32), 4)]
                .into_iter()
                .collect();
        let rose: Vec<&str> = rising(&seen, &all)
            .iter()
            .map(|r| r.instance.as_str())
            .collect();
        assert_eq!(
            rose,
            ["r3"],
            "a rise with a line; no line, no rise and a fall say nothing"
        );
    }

    #[test]
    fn the_answer_about_code_is_kept_per_resource() {
        let t = std::env::temp_dir().join(format!("rhtn-frame-{}", std::process::id()));
        assert_eq!(script_allowed(&t, R), None);
        keep_script(&t, R, true).unwrap();
        assert_eq!(script_allowed(&t, R), Some(true));
        assert_eq!(script_allowed(&t, &R.to_ascii_lowercase()), Some(true));
        keep_script(&t, R, false).unwrap();
        assert_eq!(script_allowed(&t, R), Some(false));
        std::fs::remove_dir_all(&t).unwrap();
    }

    #[test]
    fn an_import_takes_what_it_named_and_says_whose_words_it_shows() {
        let asked = vec!["image/*".to_string(), "application/pdf".to_string()];
        assert!(takes(&asked, "image/png"));
        assert!(takes(&asked, "application/pdf"));
        assert!(!takes(&asked, "text/plain"));
        assert!(takes(&[], "text/plain"));
        assert!(takes(&["*/*".to_string()], "text/plain"));
        let q = import_question("Ledger", "your receipts", "a year", &asked);
        assert!(q.contains("image/* or application/pdf"), "{q}");
        assert!(
            q.contains("\u{201c}your receipts\u{201d}") && q.contains("its words"),
            "{q}"
        );
        assert!(import_question("Ledger", "p", "r", &[]).contains("any type"));
    }

    #[test]
    fn a_payload_is_shown_as_it_is_and_a_scan_is_an_image() {
        assert_eq!(payload_shown(b"pay 4 EUR"), "\u{201c}pay 4 EUR\u{201d}");
        let raw = payload_shown(&[0u8, 1, 2]);
        assert!(
            raw.starts_with("3 bytes that are not text, beginning 000102"),
            "{raw}"
        );
        assert!(!raw.contains('\u{2026}'));
        assert!(payload_shown(&[0xffu8; 40]).ends_with('\u{2026}'));
        assert_eq!(scan_media(&[]), vec!["image/*".to_string()]);
        assert_eq!(
            scan_media(&["application/pdf".into(), "image/png".into()]),
            vec!["image/png".to_string()]
        );
        assert!(scan_media(&["application/pdf".into()]).is_empty());
    }

    #[test]
    fn a_notify_permission_is_kept_for_its_resource() {
        let d = std::env::temp_dir().join(format!("rhtn-desktop-notify-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        assert!(!notify_allowed(&d, R));
        keep_notify(&d, R).unwrap();
        assert!(notify_allowed(&d, R));
        assert!(!notify_allowed(&d, ME), "another resource holds none");
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn the_bridge_names_the_origin_and_publishes_one_object() {
        let o = origin(ME, R).unwrap();
        let js = bridge(&o);
        assert!(js.contains(&format!("const ORIGIN = \"{o}\"")));
        assert!(js.contains(r#"["GET","POST","PUT","PATCH","DELETE"]"#));
        assert_eq!(js.matches("defineProperty").count(), 1);
    }
}
