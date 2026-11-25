//! **A second face of the bench package: content larger than one message**
//! (`resource-requirements.md` §3) [author, 2026-11-24].
//!
//! What it is for: the path to a package carries one request and one
//! answer, each bounded, so large and continuous content goes as parts —
//! a file as numbered parts the page fetches in turn, video as segments of
//! a few seconds each. This serves both, with a page that fetches them,
//! checks what arrived, plays the segments through the engine's media
//! source, and posts what it measured back, so a bench run is read from
//! this instance's log without a camera on the screen.
//!
//! **Pure, as [`crate::App`] is**: [`Stream::handle`] takes the bytes of
//! one request and gives the bytes of one response. It reads no header of
//! the node's and keeps nothing about who asked: every member is served
//! the same bytes.

use crate::{ICON, ICON_PATH, read_request};

/// The bytes in one part.
pub const PART_BYTES: usize = 1 << 20;

/// The parts the file is served in: 32 MiB in all, twice the bound on one
/// message (`wire-format.md` §11), so the whole could not be one answer.
pub const PARTS: usize = 32;

/// What the segments are, as a media source is asked for it.
pub const CODEC: &str = "video/mp4; codecs=\"avc1.42C00D\"";

/// The clip's initialisation segment: what the stream is, and no frame.
pub const INIT: &[u8] = include_bytes!("../res/stream/init.mp4");

/// **The clip's media segments**, two seconds each: a test pattern with
/// its own clock drawn on it, so a frame on the screen says how far the
/// playback got.
pub const SEGMENTS: [&[u8]; 10] = [
    include_bytes!("../res/stream/seg0.m4s"),
    include_bytes!("../res/stream/seg1.m4s"),
    include_bytes!("../res/stream/seg2.m4s"),
    include_bytes!("../res/stream/seg3.m4s"),
    include_bytes!("../res/stream/seg4.m4s"),
    include_bytes!("../res/stream/seg5.m4s"),
    include_bytes!("../res/stream/seg6.m4s"),
    include_bytes!("../res/stream/seg7.m4s"),
    include_bytes!("../res/stream/seg8.m4s"),
    include_bytes!("../res/stream/seg9.m4s"),
];

/// **The policy the page declares**: it runs script, which a member's
/// client then asks them about once
/// (`light-client-requirements.md` §8), and plays media it assembled.
pub const POLICY: &str = "default-src 'self' blob:; script-src 'unsafe-inline'; \
                          style-src 'unsafe-inline'; media-src 'self' blob:";

/// **Part `n` of the file**: [`PART_BYTES`] of a generator seeded by the
/// part's number, so every part differs, none compresses, and the page's
/// check fails on a part repeated, dropped or cut short.
#[must_use]
pub fn part(n: usize) -> Vec<u8> {
    let mut x = 0x9e37_79b9_7f4a_7c15u64 ^ ((n as u64 + 1) << 32 | (n as u64 + 1));
    let mut out = Vec::with_capacity(PART_BYTES);
    while out.len() < PART_BYTES {
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        out.extend_from_slice(&x.to_le_bytes());
    }
    out.truncate(PART_BYTES);
    out
}

/// **Adler-32, carried from one part to the next**: the check the page
/// keeps as the parts arrive, written out here and in the page's script
/// alike since a frame's origin need not offer the engine's own digests.
#[must_use]
pub fn check(carried: u32, bytes: &[u8]) -> u32 {
    let (mut a, mut b) = (carried & 0xffff, carried >> 16);
    for byte in bytes {
        a = (a + u32::from(*byte)) % 65521;
        b = (b + a) % 65521;
    }
    (b << 16) | a
}

/// The check over every part in order, which is what the page must reach.
#[must_use]
pub fn wanted_check() -> u32 {
    (0..PARTS).fold(1, |c, n| check(c, &part(n)))
}

/// The whole clip in one piece: what a page that fetched nothing in parts
/// would be handed.
#[must_use]
pub fn clip() -> Vec<u8> {
    let mut out = INIT.to_vec();
    for s in SEGMENTS {
        out.extend_from_slice(s);
    }
    out
}

/// The streaming face: the page's own figures, and what it has to say.
pub struct Stream {
    want: u32,
    /// What happened, one JSON line each: the socket's caller prints them.
    pub said: Vec<String>,
}

impl Default for Stream {
    fn default() -> Self {
        Self::new()
    }
}

impl Stream {
    /// A face ready to serve, the file's check worked out once.
    #[must_use]
    pub fn new() -> Stream {
        Stream {
            want: wanted_check(),
            said: Vec::new(),
        }
    }

    /// **One request in, one response out** (`wire-format.md` §11).
    pub fn handle(&mut self, request: &[u8]) -> Vec<u8> {
        let Some((method, target, _, body)) = read_request(request) else {
            return answer(
                400,
                "text/plain",
                None,
                b"this is not one HTTP/1.1 request\n".to_vec(),
            );
        };
        match (method.as_str(), target.as_str()) {
            // its host's facts, taken so that nothing else is held back
            // (`resource-requirements.md` §3.3); this face uses none
            ("PUT", "/.rhtn/relationships") => answer(204, "text/plain", None, Vec::new()),
            ("GET", ICON_PATH) => answer(200, "image/png", None, ICON.to_vec()),
            ("GET", "/") => {
                self.said.push("{\"what\":\"stream.page\"}".into());
                answer(
                    200,
                    "text/html; charset=utf-8",
                    Some(POLICY),
                    page(self.want).into_bytes(),
                )
            }
            ("GET", "/clip.mp4") => answer(200, "video/mp4", None, clip()),
            ("GET", "/init.mp4") => answer(200, "video/mp4", None, INIT.to_vec()),
            ("GET", t) if t.starts_with("/part/") => match t["/part/".len()..].parse::<usize>() {
                Ok(n) if n < PARTS => answer(200, "application/octet-stream", None, part(n)),
                _ => answer(404, "text/plain", None, b"no such part\n".to_vec()),
            },
            ("GET", t) if t.starts_with("/seg/") => match t["/seg/".len()..].parse::<usize>() {
                Ok(n) if n < SEGMENTS.len() => answer(200, "video/mp4", None, SEGMENTS[n].to_vec()),
                _ => answer(404, "text/plain", None, b"no such segment\n".to_vec()),
            },
            // **what the page measured**, kept as the page said it where
            // it is one line of JSON and named as unreadable otherwise
            ("POST", "/report") => {
                let got = String::from_utf8(body).unwrap_or_default();
                let one_object = got.starts_with('{')
                    && got.ends_with('}')
                    && got.len() <= 4096
                    && !got.chars().any(char::is_control);
                self.said.push(if one_object {
                    format!("{{\"what\":\"stream.report\",\"got\":{got}}}")
                } else {
                    "{\"what\":\"stream.report\",\"got\":\"unreadable\"}".into()
                });
                answer(200, "text/plain", None, b"taken\n".to_vec())
            }
            ("GET", _) => answer(
                404,
                "text/plain",
                None,
                b"nothing is served there\n".to_vec(),
            ),
            _ => answer(
                405,
                "text/plain",
                None,
                b"this face takes GET, and POST to /report\n".to_vec(),
            ),
        }
    }
}

/// One response, whole, with the page's policy where it has one.
fn answer(status: u16, media: &str, policy: Option<&str>, body: Vec<u8>) -> Vec<u8> {
    let reason = match status {
        200 => "OK",
        204 => "No Content",
        400 => "Bad Request",
        404 => "Not Found",
        405 => "Method Not Allowed",
        _ => "Unknown",
    };
    let mut head = format!("HTTP/1.1 {status} {reason}\r\ncontent-type: {media}\r\n");
    if let Some(p) = policy {
        head.push_str(&format!("content-security-policy: {p}\r\n"));
    }
    head.push_str(&format!("content-length: {}\r\n\r\n", body.len()));
    let mut out = head.into_bytes();
    out.extend_from_slice(&body);
    out
}

/// **The page**: the parts fetched in turn and checked, the whole clip
/// asked for in one piece, the segments appended to a media source and
/// played; every figure shown in a table and posted to `/report`.
#[must_use]
pub fn page(want: u32) -> String {
    PAGE.replace("__PARTS__", &PARTS.to_string())
        .replace("__WANT__", &want.to_string())
        .replace("__SEGS__", &SEGMENTS.len().to_string())
        .replace("__CODEC__", &CODEC.replace('"', "\\\""))
}

const PAGE: &str = r#"<!doctype html>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>Content in parts</title>
<style>
body { font: 15px sans-serif; margin: 16px; }
td { padding: 2px 12px 2px 0; vertical-align: top; }
video { width: 100%; max-width: 480px; background: #000; }
</style>
<h1>Content in parts</h1>
<p>A file of __PARTS__ MiB fetched one part at a time, and a clip of
__SEGS__ segments played as they arrive. Nothing here is one message.</p>
<table id="t"></table>
<video id="v" muted playsinline controls></video>
<script>
(() => {
  const PARTS = __PARTS__, WANT = __WANT__, SEGS = __SEGS__, CODEC = "__CODEC__";
  const out = {};
  const row = (k, v) => {
    out[k] = v;
    let r = document.getElementById('r-' + k);
    if (!r) {
      r = document.getElementById('t').insertRow();
      r.id = 'r-' + k;
      r.insertCell().textContent = k;
      r.insertCell();
    }
    r.cells[1].textContent = String(v);
  };
  const get = p => new Promise((ok, no) => {
    const x = new XMLHttpRequest();
    x.open('GET', p, true);
    x.responseType = 'arraybuffer';
    x.onload = () => x.status === 200
      ? ok(new Uint8Array(x.response))
      : no(new Error(p + ' answered ' + x.status));
    x.onerror = () => no(new Error(p + ' was not carried'));
    x.send();
  });
  const report = () => {
    try {
      if (window.rhtn && rhtn.request)
        rhtn.request('POST', '/report', 'application/json', JSON.stringify(out));
    } catch (e) { row('report_error', String(e)); }
  };
  async function parts() {
    let a = 1, b = 0, n = 0, spent = 0;
    for (let i = 0; i < PARTS; i++) {
      const t0 = performance.now();
      const p = await get('/part/' + i);
      spent += performance.now() - t0;
      n += p.length;
      for (let j = 0; j < p.length; j++) {
        a += p[j]; if (a >= 65521) a -= 65521;
        b += a; if (b >= 65521) b -= 65521;
      }
      row('parts', (i + 1) + ' of ' + PARTS);
    }
    row('parts_bytes', n);
    row('parts_seconds', (spent / 1000).toFixed(2));
    row('parts_mib_per_s', (n / 1048576 / (spent / 1000)).toFixed(2));
    row('parts_check', (((b << 16) | a) >>> 0) === WANT ? 'matches' : 'DIFFERS');
  }
  function whole() {
    return new Promise(ok => {
      const w = document.createElement('video');
      w.muted = true;
      w.preload = 'auto';
      let answered = false;
      const done = said => {
        if (answered) return;
        answered = true;
        row('whole_clip', said);
        ok();
      };
      w.onloadeddata = () => done('decoded, ' + w.videoWidth + 'x' + w.videoHeight);
      w.onerror = () => done('refused, code ' + (w.error ? w.error.code : '?'));
      setTimeout(() => done('no answer in 10 s'), 10000);
      w.src = '/clip.mp4';
    });
  }
  async function segments() {
    if (!window.MediaSource || !MediaSource.isTypeSupported(CODEC)) {
      row('segments', 'this engine offers no media source for ' + CODEC);
      return;
    }
    const v = document.getElementById('v'), ms = new MediaSource();
    v.src = URL.createObjectURL(ms);
    await new Promise(r => ms.addEventListener('sourceopen', r, { once: true }));
    const sb = ms.addSourceBuffer(CODEC);
    const add = buf => new Promise((ok, no) => {
      sb.addEventListener('updateend', ok, { once: true });
      sb.addEventListener('error', () => no(new Error('the engine refused a segment')), { once: true });
      sb.appendBuffer(buf);
    });
    const t0 = performance.now();
    let n = 0;
    await add(await get('/init.mp4'));
    for (let i = 0; i < SEGS; i++) {
      const s = await get('/seg/' + i);
      n += s.length;
      await add(s);
      row('segments', (i + 1) + ' of ' + SEGS + ' appended');
      if (i === 0) v.play().catch(e => row('play', String(e)));
    }
    ms.endOfStream();
    row('segments_bytes', n);
    row('segments_seconds', ((performance.now() - t0) / 1000).toFixed(2));
    row('buffered_seconds', v.buffered.length ? v.buffered.end(v.buffered.length - 1).toFixed(2) : '0');
    await new Promise(r => setTimeout(r, 5000));
    row('played_seconds', v.currentTime.toFixed(2));
  }
  (async () => {
    row('engine', navigator.userAgent);
    try { await parts(); } catch (e) { row('parts_error', e.message); }
    report();
    try { await whole(); } catch (e) { row('whole_error', e.message); }
    try { await segments(); } catch (e) { row('segments_error', e.message); }
    report();
  })();
})();
</script>
"#;
