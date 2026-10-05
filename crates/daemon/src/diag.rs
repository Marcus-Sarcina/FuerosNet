//! The daemon's field-test sink (`Robot/field-test-diagnostics.md`,
//! section 4, *daemon and instrument*): the `[log]` table names a file,
//! and a field-test build writes every diagnostic event to it as one JSON
//! object per line.  `say!` stays for the human lines.
//!
//! **One line format across the set.** The renderer here is the client's
//! `diag::json::JsonLines`, copied rather than linked (a daemon has no
//! reason to carry the client), so the daemon's file, an instrument's file
//! and a phone's bundle read alike and `rhtn diag merge` reads them all:
//! `ms` since process start, `level`, `layer` (the event's target), `event`
//! (its name), then the event's fields in recording order.  The first line
//! of a file is the **anchor**, an event named `diag.anchor` whose
//! `unix_ms` field is the wall clock at the `ms` it carries; the merge tool
//! places every later line at `unix_ms + (line.ms - anchor.ms)`.  The
//! anchor is written by the installer, not raised through `tracing`, so the
//! configured level cannot filter it out.
//!
//! **A releasable build accepts the table and logs nothing.** Its hooks are
//! compiled out, so there is nothing to render; [`install`] says so once on
//! stderr and the daemon goes on.

use crate::config::LogConfig;

/// Install the sink the `[log]` table asks for.  In a releasable build,
/// one line to the operator and nothing else.
#[cfg(not(feature = "fieldtest"))]
pub fn install(cfg: &LogConfig) -> Result<(), String> {
    crate::say!(
        "rhtnd: [log] names {}; this is a releasable build and its diagnostics are compiled out, so nothing is written there",
        cfg.path.display()
    );
    Ok(())
}

/// Install the sink the `[log]` table asks for: the file opened for
/// append, the anchor line written, and the renderer set as the process's
/// subscriber at the configured level.  `off` installs nothing.
#[cfg(feature = "fieldtest")]
pub fn install(cfg: &LogConfig) -> Result<(), String> {
    use crate::config::LogLevel;
    use std::io::Write as _;
    use std::sync::Mutex;
    use tracing_subscriber::layer::SubscriberExt;

    // process start, for every line's `ms`, is now at the latest
    let _ = rhtn_node::diag::since_start();
    let max = match cfg.level {
        LogLevel::Off => return Ok(()),
        LogLevel::Error => tracing::level_filters::LevelFilter::ERROR,
        LogLevel::Warn => tracing::level_filters::LevelFilter::WARN,
        LogLevel::Info => tracing::level_filters::LevelFilter::INFO,
        LogLevel::Debug => tracing::level_filters::LevelFilter::DEBUG,
        LogLevel::Trace => tracing::level_filters::LevelFilter::TRACE,
    };
    let file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&cfg.path)
        .map_err(|e| format!("{}: {e}", cfg.path.display()))?;
    let file = Mutex::new(file);
    let write = move |line: String| {
        if let Ok(mut f) = file.lock() {
            // a failed write loses the line and never the daemon
            let _ = writeln!(f, "{line}");
        }
    };
    write(anchor_line("rhtnd"));
    let layer = JsonLines::new(write, max);
    tracing::subscriber::set_global_default(tracing_subscriber::registry().with(layer))
        .map_err(|e| format!("a subscriber is already installed: {e}"))
}

/// The anchor line a file begins with, in the renderer's own format.
#[cfg(feature = "fieldtest")]
pub fn anchor_line(process: &str) -> String {
    let unix_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0);
    json::render(&[
        (
            "ms".into(),
            serde_json::Value::from(rhtn_node::diag::since_start().0),
        ),
        ("level".into(), serde_json::Value::from("info")),
        ("layer".into(), serde_json::Value::from("diag")),
        ("event".into(), serde_json::Value::from("diag.anchor")),
        ("unix_ms".into(), serde_json::Value::from(unix_ms)),
        ("pid".into(), serde_json::Value::from(std::process::id())),
        ("process".into(), serde_json::Value::from(process)),
    ])
}

#[cfg(feature = "fieldtest")]
pub use json::JsonLines;

/// The renderer: every event as one JSON line, handed to a sink.  The
/// client's `diag::json`, with a level the layer answers `enabled` from so
/// the `[log] level` is honoured without a second filtering layer.
#[cfg(feature = "fieldtest")]
pub mod json {
    use serde_json::Value;
    use tracing::field::{Field, Visit};
    use tracing::level_filters::LevelFilter;
    use tracing::span::{Attributes, Id};
    use tracing::{Event, Metadata, Subscriber};
    use tracing_subscriber::Layer;
    use tracing_subscriber::layer::Context;
    use tracing_subscriber::registry::LookupSpan;

    /// A `tracing_subscriber` layer rendering each event as one JSON
    /// object on one line: `ms` since process start, `level`, `layer` (the
    /// event's target), `event` (its message, which is its name), the
    /// event's fields in the order recorded, and, inside a span, `span`
    /// with the span's name and fields.
    pub struct JsonLines<F> {
        sink: F,
        max: LevelFilter,
    }

    impl<F: Fn(String) + Send + Sync + 'static> JsonLines<F> {
        /// A layer writing each rendered line to `sink`, dropping an
        /// event less severe than `max`.
        pub fn new(sink: F, max: LevelFilter) -> JsonLines<F> {
            JsonLines { sink, max }
        }
    }

    /// A span's fields, kept in its extensions from `on_new_span`.
    struct SpanFields(Vec<(String, Value)>);

    /// Collects an event's or a span's fields in recording order.
    struct Fields<'a>(&'a mut Vec<(String, Value)>);

    impl Fields<'_> {
        fn put(&mut self, field: &Field, v: Value) {
            self.0.push((field.name().to_string(), v));
        }
    }

    impl Visit for Fields<'_> {
        fn record_f64(&mut self, field: &Field, value: f64) {
            self.put(field, Value::from(value));
        }
        fn record_i64(&mut self, field: &Field, value: i64) {
            self.put(field, Value::from(value));
        }
        fn record_u64(&mut self, field: &Field, value: u64) {
            self.put(field, Value::from(value));
        }
        fn record_i128(&mut self, field: &Field, value: i128) {
            self.put(field, Value::from(value.to_string()));
        }
        fn record_u128(&mut self, field: &Field, value: u128) {
            self.put(field, Value::from(value.to_string()));
        }
        fn record_bool(&mut self, field: &Field, value: bool) {
            self.put(field, Value::from(value));
        }
        fn record_str(&mut self, field: &Field, value: &str) {
            self.put(field, Value::from(value));
        }
        fn record_debug(&mut self, field: &Field, value: &dyn std::fmt::Debug) {
            self.put(field, Value::from(format!("{value:?}")));
        }
    }

    /// One JSON object from ordered pairs, keys in the order given.
    pub fn render(pairs: &[(String, Value)]) -> String {
        let mut out = String::from("{");
        for (i, (k, v)) in pairs.iter().enumerate() {
            if i > 0 {
                out.push(',');
            }
            out.push_str(&serde_json::to_string(k).unwrap_or_default());
            out.push(':');
            out.push_str(&serde_json::to_string(v).unwrap_or_default());
        }
        out.push('}');
        out
    }

    impl<S, F> Layer<S> for JsonLines<F>
    where
        S: Subscriber + for<'a> LookupSpan<'a>,
        F: Fn(String) + Send + Sync + 'static,
    {
        fn enabled(&self, meta: &Metadata<'_>, _: Context<'_, S>) -> bool {
            // Foreign crates (quinn, rustls) chatter at debug and quinn's
            // connection-id lines carry QUIC reset tokens; the file is for
            // this project's events, so other targets pass only at warn
            // and above whatever the configured level.
            // This project's events name a layer as their target (`node`,
            // `daemon`, `transport`, `cer`, `pay`); a dependency's events
            // carry its module path, which has `::` in it.
            let ours = !meta.target().contains("::");
            let cap = if ours {
                self.max
            } else {
                self.max.min(LevelFilter::WARN)
            };
            cap >= *meta.level()
        }

        fn max_level_hint(&self) -> Option<LevelFilter> {
            Some(self.max)
        }

        fn on_new_span(&self, attrs: &Attributes<'_>, id: &Id, ctx: Context<'_, S>) {
            let mut fields = Vec::new();
            attrs.record(&mut Fields(&mut fields));
            if let Some(span) = ctx.span(id) {
                span.extensions_mut().insert(SpanFields(fields));
            }
        }

        fn on_event(&self, event: &Event<'_>, ctx: Context<'_, S>) {
            let meta = event.metadata();
            let mut pairs: Vec<(String, Value)> = vec![
                ("ms".into(), Value::from(rhtn_node::diag::since_start().0)),
                (
                    "level".into(),
                    Value::from(meta.level().as_str().to_ascii_lowercase()),
                ),
                ("layer".into(), Value::from(meta.target())),
            ];
            let mut fields = Vec::new();
            event.record(&mut Fields(&mut fields));
            // the message is the event's name, and goes before its fields
            if let Some(i) = fields.iter().position(|(k, _)| k == "message") {
                let (_, name) = fields.remove(i);
                pairs.push(("event".into(), name));
            }
            pairs.extend(fields);
            if let Some(span) = ctx.event_scope(event).and_then(|mut s| s.next()) {
                let mut inner = vec![("name".to_string(), Value::from(span.name()))];
                if let Some(SpanFields(f)) = span.extensions().get::<SpanFields>() {
                    inner.extend(f.iter().cloned());
                }
                pairs.push(("span".into(), Value::Object(inner.into_iter().collect())));
            }
            (self.sink)(render(&pairs));
        }
    }
}
