//! `--log <file>`: where a field-test build's diagnostic events go
//! (`Robot/field-test-diagnostics.md`, section 4, *daemon and instrument*).
//!
//! The FFI installs the client's renderer on the `Diagnostics` object it is
//! handed, and hands it one JSON line per event; this module is the object
//! that appends those lines to the file, and the anchor event the merge
//! tool places the file by: `diag.anchor`, whose `unix_ms` is the wall
//! clock at the `ms` the renderer stamps it with.  The format is the
//! daemon's and the phone's (`crates/daemon/src/diag.rs` states it).
//!
//! **A releasable build writes nothing.** Its hooks are compiled out and
//! its `Diagnostics` object hears nothing, so `--log` is accepted, said to
//! be inert on stderr, and the sink stays [`Silent`](rhtn_ffi::device::Silent).

use rhtn_ffi::device::Diagnostics;
use std::path::Path;
use std::sync::Arc;

/// The `Diagnostics` object for `--log`: the file, appended a line at a
/// time.  A line that fails to write is lost and the instrument goes on.
#[cfg(feature = "fieldtest")]
pub struct FileSink(std::sync::Mutex<std::fs::File>);

#[cfg(feature = "fieldtest")]
impl Diagnostics for FileSink {
    fn event(&self, line: String) {
        use std::io::Write as _;
        if let Ok(mut f) = self.0.lock() {
            let _ = writeln!(f, "{line}");
        }
    }
}

/// The sink `--log` asks for: the file opened for append in a field-test
/// build.
#[cfg(feature = "fieldtest")]
pub fn sink(log: Option<&Path>) -> Result<Arc<dyn Diagnostics>, String> {
    match log {
        None => Ok(Arc::new(rhtn_ffi::device::Silent)),
        Some(p) => {
            let f = std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(p)
                .map_err(|e| format!("--log {}: {e}", p.display()))?;
            Ok(Arc::new(FileSink(std::sync::Mutex::new(f))))
        }
    }
}

/// The sink `--log` asks for, in a releasable build: nothing, said once.
#[cfg(not(feature = "fieldtest"))]
pub fn sink(log: Option<&Path>) -> Result<Arc<dyn Diagnostics>, String> {
    if let Some(p) = log {
        eprintln!(
            "rhtnp: --log {}: this is a releasable build and its diagnostics are compiled out, so nothing is written there",
            p.display()
        );
    }
    Ok(Arc::new(rhtn_ffi::device::Silent))
}

/// Raise the anchor: the one event that carries the wall clock, which the
/// merge tool uses to place every other line of the file.  Compiled out
/// with the rest in a releasable build.
pub fn anchor() {
    let unix_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0);
    tracing::info!(
        target: "diag",
        unix_ms,
        pid = std::process::id(),
        process = "rhtnp",
        "diag.anchor"
    );
}
