//! **The platform, where the platform is a desktop** (`rhtn_ffi::device`):
//! the objects the kernel takes from a shell, built in Rust because the
//! kernel is linked here directly and no binding stands between them.
//!
//! **A desktop performs no ceremony** (design §23.3: the identity seed sits
//! on the devices that perform ceremonies and nowhere else), so its camera
//! is never asked for a frame that matters and answers with nothing. The
//! rest are the desktop's own: its clock, its randomness, a directory for
//! what the kernel persists, and the window for the questions the kernel
//! puts to the person and the notices it raises.
//!
//! **Custody is handed in, not chosen here** (`light-client-requirements.md`
//! §9): where the storage key lives between runs is a declaration the
//! window's builder makes, and this module never falls back to one.

use rhtn_ffi::device::{
    Camera, Clock, Custody, Diagnostics, Notices, Operator, Platform, Random, Silent, Storage,
};
use rhtn_ffi::types::{Ask, Told};
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// Who answers a question the kernel puts to the person.
pub type Asker = Arc<dyn Fn(&str) -> bool + Send + Sync>;
/// Where a notice the kernel raises goes.
pub type Teller = Arc<dyn Fn(Told) + Send + Sync>;

/// The desktop's own objects: a directory for the kernel's state, and the
/// window's ear for its questions and notices.
pub struct Desk {
    dir: PathBuf,
    ask: Asker,
    tell: Teller,
}

impl Desk {
    /// A desk whose kernel state is written under `dir`, created owner-only
    /// where it is missing.
    pub fn new(dir: &Path, ask: Asker, tell: Teller) -> Result<Arc<Desk>, String> {
        create_private(dir)?;
        Ok(Arc::new(Desk {
            dir: dir.to_path_buf(),
            ask,
            tell,
        }))
    }

    /// The platform the kernel starts on, with `custody` keeping its
    /// storage key.
    pub fn platform(self: &Arc<Self>, custody: Arc<dyn Custody>) -> Arc<Platform> {
        Arc::new(Platform {
            camera: self.clone(),
            clock: self.clone(),
            random: self.clone(),
            operator: self.clone(),
            notices: self.clone(),
            storage: self.clone(),
            custody,
            diagnostics: diagnostics(&self.dir),
            engine: None,
        })
    }

    /// Where a name the kernel writes under lands: one file a name, and a
    /// name that would leave the directory is no name.
    fn at(&self, name: &str) -> Option<PathBuf> {
        let plain =
            !name.is_empty() && name != "." && name != ".." && !name.contains(['/', '\\', '\0']);
        plain.then(|| self.dir.join(name))
    }
}

/// **Where the kernel's diagnostic lines go**: in a field-test build,
/// `diag.log` in the state directory, appended a line at a time, as the
/// phone's go to logcat; a line that fails to write is lost.  A releasable
/// build's hooks are compiled out, and it writes nothing.
#[cfg(feature = "fieldtest")]
fn diagnostics(dir: &Path) -> Arc<dyn Diagnostics> {
    struct Log(std::sync::Mutex<std::fs::File>);
    impl Diagnostics for Log {
        fn event(&self, line: String) {
            use std::io::Write as _;
            if let Ok(mut f) = self.0.lock() {
                let _ = writeln!(f, "{line}");
            }
        }
    }
    match std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(dir.join(DIAG))
    {
        Ok(f) => Arc::new(Log(std::sync::Mutex::new(f))),
        Err(_) => Arc::new(Silent),
    }
}

/// A releasable build's: nothing.
#[cfg(not(feature = "fieldtest"))]
fn diagnostics(_: &Path) -> Arc<dyn Diagnostics> {
    Arc::new(Silent)
}

/// The field-test build's diagnostics file, in the state directory.
pub const DIAG: &str = "diag.log";

/// Make `dir` and its parents, the last readable by its owner alone.
pub fn create_private(dir: &Path) -> Result<(), String> {
    std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(dir, std::fs::Permissions::from_mode(0o700))
            .map_err(|e| format!("{}: {e}", dir.display()))?;
    }
    Ok(())
}

/// **Write `bytes` at `at` whole or not at all**: into a sibling first,
/// owner-only, then renamed over, so a crash mid-write leaves the last
/// state the kernel saved rather than half of a new one.
pub fn write_whole(at: &Path, bytes: &[u8]) -> std::io::Result<()> {
    use std::io::Write as _;
    let tmp = at.with_extension("partial");
    let mut opts = std::fs::OpenOptions::new();
    opts.write(true).create(true).truncate(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        opts.mode(0o600);
    }
    let mut f = opts.open(&tmp)?;
    f.write_all(bytes)?;
    f.sync_all()?;
    drop(f);
    std::fs::rename(&tmp, at)
}

impl Storage for Desk {
    fn read(&self, name: String) -> Option<Vec<u8>> {
        std::fs::read(self.at(&name)?).ok()
    }
    fn write(&self, name: String, bytes: Vec<u8>) -> bool {
        self.at(&name)
            .is_some_and(|at| write_whole(&at, &bytes).is_ok())
    }
}

impl Camera for Desk {
    /// Nothing: a desktop is never the device a ceremony captures on.
    fn capture(&self, _ask: Ask) -> Vec<u8> {
        Vec::new()
    }
    fn probe(&self) -> Vec<u8> {
        Vec::new()
    }
}

impl Clock for Desk {
    /// **The platform's clock is the clock** (`light-client-requirements.md`
    /// §1.2).
    fn now_ms(&self) -> u64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0)
    }
    fn wait_ms(&self, ms: u64) {
        std::thread::sleep(std::time::Duration::from_millis(ms));
    }
}

impl Random for Desk {
    fn fill(&self, n: u32) -> Vec<u8> {
        let mut out = Vec::with_capacity(n as usize);
        while out.len() < n as usize {
            out.extend_from_slice(&rhtn_transport::tls::random_bytes::<32>());
        }
        out.truncate(n as usize);
        out
    }
}

impl Operator for Desk {
    fn ask(&self, question: String) -> bool {
        (self.ask)(&question)
    }
}

impl Notices for Desk {
    fn told(&self, notice: Told) {
        (self.tell)(notice)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn desk(dir: &Path) -> Arc<Desk> {
        Desk::new(dir, Arc::new(|_| false), Arc::new(|_| {})).unwrap()
    }

    fn scratch(tag: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("rhtn-desk-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        d
    }

    #[test]
    fn what_is_written_reads_back_and_a_name_cannot_leave_the_directory() {
        let d = scratch("names");
        let s = desk(&d);
        assert!(s.write("state".into(), b"one".to_vec()));
        assert_eq!(s.read("state".into()).as_deref(), Some(&b"one"[..]));
        assert!(s.write("state".into(), b"two".to_vec()));
        assert_eq!(s.read("state".into()).as_deref(), Some(&b"two"[..]));
        for bad in ["", ".", "..", "../x", "a/b", "a\\b"] {
            assert!(!s.write(bad.into(), b"x".to_vec()), "{bad:?} is no name");
            assert_eq!(s.read(bad.into()), None);
        }
        assert!(
            !d.join("state.partial").exists(),
            "nothing half-written stays"
        );
        let _ = std::fs::remove_dir_all(&d);
    }

    #[cfg(unix)]
    #[test]
    fn the_directory_and_its_files_are_the_owners_alone() {
        use std::os::unix::fs::PermissionsExt;
        let d = scratch("modes");
        let s = desk(&d);
        assert!(s.write("state".into(), b"x".to_vec()));
        let mode = |p: &Path| std::fs::metadata(p).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode(&d), 0o700);
        assert_eq!(mode(&d.join("state")), 0o600);
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn the_person_and_the_notices_are_the_windows() {
        let d = scratch("ear");
        let heard = Arc::new(std::sync::Mutex::new(Vec::new()));
        let h = heard.clone();
        let s = Desk::new(
            &d,
            Arc::new(|q| q.contains("release")),
            Arc::new(move |t| h.lock().unwrap().push(t)),
        )
        .unwrap();
        assert!(s.ask("release it?".into()));
        assert!(!s.ask("anything else?".into()));
        s.told(Told::NoCandidateRecognised);
        assert_eq!(*heard.lock().unwrap(), vec![Told::NoCandidateRecognised]);
        assert_eq!(s.fill(70).len(), 70);
        let _ = std::fs::remove_dir_all(&d);
    }
}
