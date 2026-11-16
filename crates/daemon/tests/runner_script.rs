//! **The runner's vocabulary, driven as sshd would drive it** with the
//! engine stood in for: `SSH_ORIGINAL_COMMAND` carries the request and a
//! script in the engine's place prints what it was asked to run.
//!
//! The mounted block itself (`block /rhtn-block <bytes>`) needs a host
//! whose filesystem is ext4 mounted with project quotas, which a test
//! machine is not assumed to be; what is held here is the other half —
//! the refusal said rather than failed, the token consumed, the engine's
//! line untouched by it — and the bench holds the mount.

use std::path::PathBuf;
use std::process::Command;

fn runner() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../packaging/package-runner.sh")
}

struct Host {
    dir: PathBuf,
    engine: PathBuf,
}

impl Host {
    fn new(name: &str) -> Host {
        let dir = std::env::temp_dir().join(format!("rhtn-runner-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("blocks")).unwrap();
        let engine = dir.join("engine");
        std::fs::write(
            &engine,
            "#!/bin/sh\nprintf 'engine:'; for a in \"$@\"; do printf ' [%s]' \"$a\"; done; echo\n",
        )
        .unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&engine, std::fs::Permissions::from_mode(0o755)).unwrap();
        }
        Host { dir, engine }
    }

    /// One request, as the restricted key would carry it: exit status,
    /// stdout, stderr.
    fn ask(&self, request: &str) -> (i32, String, String) {
        let out = Command::new("sh")
            .arg(runner())
            .env("SSH_ORIGINAL_COMMAND", request)
            .env("RHTN_ENGINE", &self.engine)
            .env("RHTN_BLOCKS", self.dir.join("blocks"))
            .output()
            .expect("sh runs the runner");
        (
            out.status.code().unwrap_or(-1),
            String::from_utf8_lossy(&out.stdout).to_string(),
            String::from_utf8_lossy(&out.stderr).to_string(),
        )
    }
}

impl Drop for Host {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

// functional: PKG-020
#[test]
fn a_block_is_asked_for_after_the_image_and_said_rather_than_failed_where_the_host_cannot_keep_one()
{
    let h = Host::new("block");
    // no block asked: the engine's line as it always was
    let (code, out, err) = h.ask("start demo reg.io/x@sha256:abc --listen 127.0.0.1:1 --secret ff");
    assert_eq!(code, 0, "{err}");
    assert!(
        out.starts_with("engine: [run] [-d] [--name] [rhtn-pkg-demo] [--restart=always]"),
        "{out}"
    );
    assert!(
        out.contains("[reg.io/x@sha256:abc] [--listen] [127.0.0.1:1] [--secret] [ff]"),
        "{out}"
    );
    assert!(!out.contains("block"), "{out}");

    // a block asked on a filesystem this test cannot assume bounds one:
    // the first line says so, and the start goes on without it
    let (code, out, err) = h.ask("start demo reg.io/x@sha256:abc block=4096 --listen 127.0.0.1:1");
    assert_eq!(code, 0, "{err}");
    let mut lines = out.lines();
    let first = lines.next().unwrap_or_default();
    let engine = lines.next().unwrap_or_default();
    assert!(
        first.starts_with("block none ") || first.starts_with("block /rhtn-block 4096"),
        "the first line is the block's: {out}"
    );
    if first.starts_with("block none ") {
        assert!(!engine.contains("[-v]"), "nothing mounted: {engine}");
    } else {
        assert!(engine.contains("[-v]"), "mounted: {engine}");
    }
    assert!(
        !engine.contains("block=4096"),
        "the token is the runner's and never the package's argument: {engine}"
    );
    assert!(
        engine.ends_with("[reg.io/x@sha256:abc] [--listen] [127.0.0.1:1]"),
        "{engine}"
    );

    // and a count that is not one is refused before anything runs
    for bad in ["block=12x", "block=100", "block="] {
        let (code, out, err) = h.ask(&format!("start demo reg.io/x@sha256:abc {bad}"));
        assert_eq!(code, 2, "{bad}: {out}{err}");
        assert!(!out.contains("engine:"), "{bad}: nothing ran: {out}");
        assert!(err.contains("rhtn-package-runner:"), "{bad}: {err}");
    }
}
