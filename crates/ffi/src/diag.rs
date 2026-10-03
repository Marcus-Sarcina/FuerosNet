//! The boundary's diagnostics (`Robot/field-test-diagnostics.md`, section
//! 3.1): every exported `Participant` method as a span with its outcome,
//! the platform's storage and custody as events, and, in the field-test
//! flavour, the renderer that hands each event to the platform's
//! [`Diagnostics`](crate::device::Diagnostics) object as one JSON line.
//!
//! **Compiled out of a releasable build** with the kernel's hooks: the
//! `releasable` feature sets `tracing/max_level_off` (see `rhtn_client::diag`
//! and `crates/README.md`), and [`install`] is a no-op there.
//!
//! **The span does not cross the thread.** The client runs on a thread of
//! its own (`rhtn-adaptors`), so the kernel's events raised while a method
//! runs carry no `span`; what brackets them is the `ffi.call` and
//! `ffi.return` pair, by time.

use crate::device::Storage;
use crate::types::Refused;
use std::sync::Arc;

/// One exported method that can refuse: `ffi.call` on the way in,
/// `ffi.return` on the way out with the elapsed milliseconds and, where
/// the answer was a refusal, its reason.  The body is the closure, so a
/// method wrapped reads as it did; `?` and `return` inside leave the
/// closure, which is the method's own result.
pub(crate) fn call<T>(
    method: &'static str,
    f: impl FnOnce() -> Result<T, Refused>,
) -> Result<T, Refused> {
    let span = span(method);
    let _entered = span.enter();
    let started = clock();
    tracing::debug!(target: "ffi", method, "ffi.call");
    let out = f();
    match &out {
        Ok(_) => {
            tracing::info!(target: "ffi", method, ms = elapsed(started), ok = true, "ffi.return")
        }
        Err(e) => tracing::warn!(
            target: "ffi",
            method,
            ms = elapsed(started),
            ok = false,
            refused = e.reason(),
            "ffi.return"
        ),
    }
    out
}

/// The same for a method that cannot refuse.
pub(crate) fn call_plain<T>(method: &'static str, f: impl FnOnce() -> T) -> T {
    let span = span(method);
    let _entered = span.enter();
    let started = clock();
    tracing::debug!(target: "ffi", method, "ffi.call");
    let out = f();
    tracing::info!(target: "ffi", method, ms = elapsed(started), ok = true, "ffi.return");
    out
}

/// The method's span, made only where spans are enabled: a disabled span
/// made by the macro would still hold its callsite's metadata, and so keep
/// the span's name and field in a releasable binary.
fn span(method: &'static str) -> tracing::Span {
    if tracing::span_enabled!(target: "ffi", tracing::Level::INFO) {
        tracing::info_span!(target: "ffi", "ffi.call", method)
    } else {
        tracing::Span::none()
    }
}

/// The clock is read only in a build that has events to put the reading in.
fn clock() -> Option<std::time::Instant> {
    tracing::event_enabled!(target: "ffi", tracing::Level::INFO).then(std::time::Instant::now)
}

fn elapsed(started: Option<std::time::Instant>) -> u64 {
    started.map_or(0, |t| t.elapsed().as_millis() as u64)
}

/// The platform's storage with the two events on it: the name, the size
/// and whether it landed, never the bytes.
pub(crate) struct TracedStorage(pub(crate) Arc<dyn Storage>);

impl Storage for TracedStorage {
    fn read(&self, name: String) -> Option<Vec<u8>> {
        let shown = tracing::event_enabled!(target: "platform", tracing::Level::DEBUG)
            .then(|| name.clone());
        let out = self.0.read(name);
        if let Some(name) = shown {
            tracing::debug!(
                target: "platform",
                op = "read",
                name = %name,
                bytes = out.as_ref().map_or(0, Vec::len),
                ok = out.is_some(),
                "platform.storage"
            );
        }
        out
    }

    fn write(&self, name: String, bytes: Vec<u8>) -> bool {
        let shown = tracing::event_enabled!(target: "platform", tracing::Level::DEBUG)
            .then(|| (name.clone(), bytes.len()));
        let ok = self.0.write(name, bytes);
        if let Some((name, bytes)) = shown {
            tracing::debug!(target: "platform", op = "write", name = %name, bytes, ok, "platform.storage");
        }
        ok
    }
}

/// Point the renderer at the platform's `Diagnostics` object.  The
/// renderer is installed once per process; a later start re-points it, so
/// a shell restarted in one process is the one that hears.  A no-op in a
/// releasable build.
#[cfg(feature = "fieldtest")]
pub(crate) fn install(sink: Arc<dyn crate::device::Diagnostics>) {
    use std::sync::{Once, RwLock};
    use tracing_subscriber::layer::SubscriberExt;

    static SINK: RwLock<Option<Arc<dyn crate::device::Diagnostics>>> = RwLock::new(None);
    static ONCE: Once = Once::new();

    // process start, for the `ms` field, is the first start
    let _ = rhtn_client::diag::since_start();
    if let Ok(mut s) = SINK.write() {
        *s = Some(sink);
    }
    ONCE.call_once(|| {
        let layer = rhtn_client::diag::json::JsonLines::new(|line: String| {
            if let Ok(s) = SINK.read()
                && let Some(d) = s.as_ref()
            {
                d.event(line);
            }
        });
        // another subscriber already set (a test's, say) keeps its place
        let _ = tracing::subscriber::set_global_default(tracing_subscriber::registry().with(layer));
        panic_hook();
    });
}

/// The Rust side's crash handler (plan, section 3.6, the `crash` row): a
/// panic's message and location as one event through the same layer,
/// before the hook that was there prints it and the process dies.  The
/// message is what the panicking code wrote, a reason string by the plan's
/// redaction classes.  Guarded against a panic raised while the sink
/// itself runs, which would otherwise re-enter this hook without end.
#[cfg(feature = "fieldtest")]
fn panic_hook() {
    use std::cell::Cell;
    thread_local! {
        static IN_HOOK: Cell<bool> = const { Cell::new(false) };
    }
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        if !IN_HOOK.with(|f| f.replace(true)) {
            let payload = info.payload();
            let message = payload
                .downcast_ref::<&str>()
                .map(|s| (*s).to_string())
                .or_else(|| payload.downcast_ref::<String>().cloned())
                .unwrap_or_default();
            let (file, line) = info.location().map_or(("", 0), |l| (l.file(), l.line()));
            tracing::error!(
                target: "ffi",
                kind = "panic",
                message = %message,
                file,
                line,
                "crash"
            );
            IN_HOOK.with(|f| f.set(false));
        }
        previous(info);
    }));
}

#[cfg(not(feature = "fieldtest"))]
pub(crate) fn install(_: Arc<dyn crate::device::Diagnostics>) {}
