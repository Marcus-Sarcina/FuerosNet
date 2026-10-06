//! `rhtn diag merge`: several field-test files into one timeline
//! (`Robot/field-test-diagnostics.md`, section 4, *the bench*).
//!
//! **The line format is the renderer's**, stated in
//! `crates/daemon/src/diag.rs` and `crates/client/src/diag.rs`: one JSON
//! object per line with `ms` (milliseconds since that process started),
//! `level`, `layer`, `event`, then the event's own fields, and `span` where
//! the event fired inside one.  Nothing here depends on `tracing`; this is
//! a reader of files.
//!
//! **The anchor.** Each process's clock starts at zero, so a file is placed
//! on the wall clock by its anchor: the first line whose `event` is
//! `diag.anchor` (or a streamed file's `diag.hello`), whose `unix_ms` field
//! is the wall clock at the `ms` that line carries.  Every line of the file sits at `unix_ms + (line.ms -
//! anchor.ms)`, before the anchor as well as after it, since the arithmetic
//! does not care where in the file the anchor fell (the daemon writes it
//! first; an instrument raises it once its renderer is up).  A file with no
//! anchor is placed with its `ms` taken as wall time, which sorts it before
//! everything anchored and is said in the summary, so a bundle from a
//! build that forgot the anchor still reads, in its own order.
//!
//! The summary after the timeline: each source and its anchor, the
//! ceremony steps per source with the time between them, every refusal and
//! abort, and counts per layer and per event name.

use serde_json::Value;
use std::collections::BTreeMap;
use std::fmt::Write as _;

/// One rendered event, read back.
#[derive(Debug, Clone, PartialEq)]
pub struct Line {
    /// Which file it came from, as the caller named it.
    pub source: String,
    /// The line's position in its file, 1-based, for a stable order among
    /// equal times.
    pub n: usize,
    /// The event's own monotonic time, in milliseconds.
    pub ms: u64,
    /// Its level, as the renderer wrote it.
    pub level: String,
    /// The layer that raised it.
    pub layer: String,
    /// The event's name.
    pub event: String,
    /// Every other key, sorted by name, which is the order the JSON
    /// reader hands them back in.
    pub fields: Vec<(String, Value)>,
    /// Where on the wall clock the merge placed it, in Unix milliseconds;
    /// `ms` itself where the file had no anchor.
    pub wall: u64,
}

/// One file, parsed.
#[derive(Debug, Clone, PartialEq)]
pub struct Source {
    /// The file, as the caller named it.
    pub name: String,
    /// `(anchor.ms, anchor.unix_ms)`, where the file has one.
    pub anchor: Option<(u64, u64)>,
    /// The events it carried, in file order.
    pub lines: Vec<Line>,
    /// Lines that were not a JSON object with the four fields, and were
    /// skipped: a truncated last line, a stray human line.
    pub skipped: usize,
}

/// The name of the anchor event and its wall-clock field.
pub const ANCHOR_EVENT: &str = "diag.anchor";
/// The field on that event carrying the wall clock.
pub const ANCHOR_FIELD: &str = "unix_ms";
/// A phone's live stream opens with this event, which carries `unix_ms`
/// too and so anchors the collector's file (`live.rs`).
pub const HELLO_EVENT: &str = "diag.hello";

/// The **first** `"ms"` in a raw line, read from the text.
///
/// **Because a line can carry two, and the parser keeps the wrong one**
/// [reviewer, 2026-10-01]. Until 2026-10-06 the emitters wrote a duration
/// under `ms` as well, so an event with one reads
/// `{"ms":65032, ..., "ms":10851}`; JSON permits it, `serde_json` keeps the
/// last, and the merge sorted such an event by its duration — which put
/// `cer.capture` before `cer.begin` in the flagship artefact of the field
/// runs. The emitters now write `took_ms`, and this keeps **the logs
/// already on disk** reading correctly, the timestamp being the first
/// field every emitter writes.
///
/// Scanned rather than parsed: a parser that kept the first would have to
/// replace `serde_json`'s map, and nothing else about these lines needs
/// that.
fn first_ms(raw: &str) -> Option<u64> {
    let at = raw.find("\"ms\":")? + 5;
    let rest = raw[at..].trim_start();
    let end = rest.find(|c: char| !c.is_ascii_digit())?;
    rest[..end].parse().ok()
}

/// One raw line, read back: the [`Line`] and, where the line is an anchor
/// (a `diag.anchor`, or a stream's `diag.hello`, carrying `unix_ms`), the
/// anchor `(ms, unix_ms)`.  `None` for a line that is not a JSON object
/// with the four fields.  `wall` is `ms`; the caller places it.
pub fn parse_line(source: &str, n: usize, raw: &str) -> Option<(Line, Option<(u64, u64)>)> {
    let raw = raw.trim();
    if raw.is_empty() {
        return None;
    }
    let Value::Object(map) = serde_json::from_str::<Value>(raw).ok()? else {
        return None;
    };
    let (ms, level, layer, event) = (
        first_ms(raw).or_else(|| map.get("ms").and_then(Value::as_u64))?,
        map.get("level").and_then(Value::as_str)?,
        map.get("layer").and_then(Value::as_str)?,
        map.get("event").and_then(Value::as_str)?,
    );
    let anchor = if event == ANCHOR_EVENT || event == HELLO_EVENT {
        map.get(ANCHOR_FIELD)
            .and_then(Value::as_u64)
            .map(|unix_ms| (ms, unix_ms))
    } else {
        None
    };
    let fields = map
        .iter()
        .filter(|(k, _)| !matches!(k.as_str(), "ms" | "level" | "layer" | "event"))
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect();
    Some((
        Line {
            source: source.to_string(),
            n,
            ms,
            level: level.to_string(),
            layer: layer.to_string(),
            event: event.to_string(),
            fields,
            wall: ms,
        },
        anchor,
    ))
}

/// Parse one file's text.  `wall` is filled in by [`merge`].
pub fn parse(name: &str, text: &str) -> Source {
    let mut lines = Vec::new();
    let mut skipped = 0;
    let mut anchor = None;
    for (i, raw) in text.lines().enumerate() {
        if raw.trim().is_empty() {
            continue;
        }
        let Some((line, at)) = parse_line(name, i + 1, raw) else {
            skipped += 1;
            continue;
        };
        if anchor.is_none() && at.is_some() {
            anchor = at;
        }
        lines.push(line);
    }
    Source {
        name: name.to_string(),
        anchor,
        lines,
        skipped,
    }
}

/// Place every source's lines on the wall clock and order them: by wall
/// time, then by source name, then by position in the file.
pub fn merge(sources: &[Source]) -> Vec<Line> {
    let mut out = Vec::new();
    for s in sources {
        for l in &s.lines {
            let mut l = l.clone();
            l.wall = match s.anchor {
                Some((at_ms, unix_ms)) => unix_ms.wrapping_add(l.ms).wrapping_sub(at_ms),
                None => l.ms,
            };
            out.push(l);
        }
    }
    out.sort_by(|a, b| {
        a.wall
            .cmp(&b.wall)
            .then_with(|| a.source.cmp(&b.source))
            .then_with(|| a.n.cmp(&b.n))
    });
    out
}

/// A field's value on one line of the timeline.
fn show(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        other => other.to_string(),
    }
}

/// The timeline: one line per event, at its offset from the first event
/// in the merge.
pub fn render_timeline(lines: &[Line]) -> String {
    let start = lines.first().map(|l| l.wall).unwrap_or(0);
    let width = lines
        .iter()
        .map(|l| l.source.len())
        .max()
        .unwrap_or(0)
        .max(6);
    let mut out = String::new();
    for l in lines {
        let _ = write!(
            out,
            "{:>10}  {:<width$}  {:<5}  {:<10}  {}",
            format!("+{}", l.wall - start),
            l.source,
            l.level,
            l.layer,
            l.event
        );
        for (k, v) in &l.fields {
            let _ = write!(out, "  {k}={}", show(v));
        }
        out.push('\n');
    }
    out
}

/// Whether a line is a refusal or an abort: said by its level, by its
/// name, or by a field whose value names one of the refusing variants the
/// node and the client render (`Refused(...)`, `Malformed(...)`,
/// `Failed(...)`, `Unbound(...)`, `Conflict(...)`).
pub fn is_refusal(l: &Line) -> bool {
    if l.level == "warn" || l.level == "error" {
        return true;
    }
    if l.event == ANCHOR_EVENT {
        return false;
    }
    let name = l.event.as_str();
    if [
        "refused",
        "abort",
        "failed",
        "unbound",
        "exhausted",
        "evicted",
        "declined",
    ]
    .iter()
    .any(|w| name.contains(w))
    {
        return true;
    }
    l.fields.iter().any(|(_, v)| {
        v.as_str().is_some_and(|s| {
            [
                "Refused",
                "Malformed",
                "Failed",
                "Unbound",
                "Conflict",
                "Abort",
            ]
            .iter()
            .any(|p| s.starts_with(p))
        })
    })
}

/// Whether a line is a ceremony step: the client's `cer` layer, and the
/// shell's `meet` layer once M3 lands.
pub fn is_ceremony_step(l: &Line) -> bool {
    l.layer == "cer" || l.layer == "meet"
}

/// The summary: sources, ceremony steps with the time between them, every
/// refusal and abort, counts per layer and per event.
pub fn render_summary(sources: &[Source], lines: &[Line]) -> String {
    let mut out = String::new();
    let start = lines.first().map(|l| l.wall).unwrap_or(0);

    let _ = writeln!(out, "sources");
    for s in sources {
        let anchored = match s.anchor {
            Some((at_ms, unix_ms)) => format!("anchored: unix_ms {unix_ms} at ms {at_ms}"),
            None => "no anchor: placed by its own ms, before everything anchored".to_string(),
        };
        let _ = writeln!(
            out,
            "  {:<12} {} lines, {} skipped, {anchored}",
            s.name,
            s.lines.len(),
            s.skipped
        );
    }

    let _ = writeln!(out, "\nceremony steps");
    let mut any = false;
    for s in sources {
        let mut prev: Option<u64> = None;
        let mut first: Option<u64> = None;
        for l in lines
            .iter()
            .filter(|l| l.source == s.name && is_ceremony_step(l))
        {
            any = true;
            let since = prev.map(|p| l.wall.saturating_sub(p)).unwrap_or(0);
            first.get_or_insert(l.wall);
            let _ = write!(
                out,
                "  {:<12} {:>10}  {:>7}  {}",
                s.name,
                format!("+{}", l.wall - start),
                format!("{since} ms"),
                l.event
            );
            for (k, v) in &l.fields {
                if k != "span" {
                    let _ = write!(out, "  {k}={}", show(v));
                }
            }
            out.push('\n');
            prev = Some(l.wall);
        }
        if let (Some(f), Some(p)) = (first, prev) {
            let _ = writeln!(out, "  {:<12} {} ms from first step to last", s.name, p - f);
        }
    }
    if !any {
        let _ = writeln!(out, "  none");
    }

    let _ = writeln!(out, "\nrefusals and aborts");
    let refusals: Vec<&Line> = lines.iter().filter(|l| is_refusal(l)).collect();
    if refusals.is_empty() {
        let _ = writeln!(out, "  none");
    }
    for l in refusals {
        let _ = write!(
            out,
            "  {:>10}  {:<12} {:<5}  {}",
            format!("+{}", l.wall - start),
            l.source,
            l.level,
            l.event
        );
        for (k, v) in &l.fields {
            if k != "span" {
                let _ = write!(out, "  {k}={}", show(v));
            }
        }
        out.push('\n');
    }

    let mut per_layer: BTreeMap<&str, usize> = BTreeMap::new();
    let mut per_event: BTreeMap<&str, usize> = BTreeMap::new();
    for l in lines {
        *per_layer.entry(l.layer.as_str()).or_default() += 1;
        *per_event.entry(l.event.as_str()).or_default() += 1;
    }
    let _ = writeln!(out, "\ncounts per layer");
    for (layer, n) in &per_layer {
        let _ = writeln!(out, "  {n:>6}  {layer}");
    }
    let _ = writeln!(out, "\ncounts per event");
    let mut events: Vec<(&str, usize)> = per_event.into_iter().collect();
    events.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(b.0)));
    for (event, n) in events {
        let _ = writeln!(out, "  {n:>6}  {event}");
    }
    let _ = writeln!(
        out,
        "\n{} events from {} sources",
        lines.len(),
        sources.len()
    );
    out
}

/// The label a file carries in the output: its file name, or, where two
/// files share one (every daemon's is `diag.jsonl`), the parent directory
/// and the file name.
pub fn labels(paths: &[&str]) -> Vec<String> {
    let name = |p: &str, parents: usize| -> String {
        let path = std::path::Path::new(p);
        let mut parts: Vec<String> = path
            .components()
            .rev()
            .take(parents + 1)
            .map(|c| c.as_os_str().to_string_lossy().to_string())
            .collect();
        parts.reverse();
        parts.join("/")
    };
    let short: Vec<String> = paths.iter().map(|p| name(p, 0)).collect();
    paths
        .iter()
        .zip(&short)
        .map(|(p, s)| {
            if short.iter().filter(|o| *o == s).count() > 1 {
                name(p, 1)
            } else {
                s.clone()
            }
        })
        .collect()
}

/// `rhtn diag merge <file>...`: the timeline, then the summary.
pub fn merge_files(paths: &[&str]) -> Result<String, String> {
    if paths.is_empty() {
        return Err("diag merge <file>...".into());
    }
    let mut sources = Vec::new();
    for (p, name) in paths.iter().zip(labels(paths)) {
        let text = std::fs::read_to_string(p).map_err(|e| format!("{p}: {e}"))?;
        sources.push(parse(&name, &text));
    }
    let lines = merge(&sources);
    let mut out = render_timeline(&lines);
    out.push('\n');
    out.push_str(&render_summary(&sources, &lines));
    Ok(out)
}
