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
use std::process::{Child, Command};

fn runner() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../packaging/package-runner.sh")
}

/// Hand a runner its standard input, as ssh would.
///
/// **A runner that refuses a request from its arguments exits without
/// reading it**, as it should, so the write meets a closed pipe whenever
/// the runner has already gone. Under a loaded run it sometimes had, and
/// the test failed on the write rather than on anything the runner said
/// [2026-11-18]. A closed pipe is what a caller sees of such a refusal,
/// and the exit status and stderr say the rest; any other failure to
/// write is the test's own.
fn feed(child: &mut Child, input: &[u8]) {
    use std::io::Write;
    match child.stdin.take().unwrap().write_all(input) {
        Ok(()) => {}
        Err(e) if e.kind() == std::io::ErrorKind::BrokenPipe => {}
        Err(e) => panic!("writing the runner's standard input: {e}"),
    }
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
        self.ask_with(request, b"")
    }

    /// The same with `input` on standard input, as ssh carries it, against
    /// a copy of the runner at `self.dir/runner` and the operator lines in
    /// `self.dir/authorized_keys`.
    fn ask_with(&self, request: &str, input: &[u8]) -> (i32, String, String) {
        let me = self.dir.join("runner");
        if !me.exists() {
            std::fs::copy(runner(), &me).unwrap();
        }
        let mut child = Command::new("sh")
            .arg(&me)
            .env("SSH_ORIGINAL_COMMAND", request)
            .env("RHTN_ENGINE", &self.engine)
            .env("RHTN_BLOCKS", self.dir.join("blocks"))
            .env("RHTN_RUNNER_SELF", &me)
            .env("RHTN_AUTHORIZED_KEYS", self.dir.join("authorized_keys"))
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .expect("sh runs the runner");
        feed(&mut child, input);
        let out = child.wait_with_output().expect("the runner ends");
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

// functional: OPS-029
#[test]
fn a_runner_is_taken_only_signed_by_the_operators_key_and_never_older() {
    let h = Host::new("refresh");
    let operator = rhtn_crypto::ssh::HostControlKey::from_seed(&[5u8; 32]);
    let runner_key =
        rhtn_crypto::ssh::HostControlKey::from_seed_named(&[6u8; 32], "rhtn-host-runner");
    std::fs::write(
        h.dir.join("authorized_keys"),
        format!(
            "{}\n{}\n",
            operator.public_line,
            runner_key.restricted_line()
        ),
    )
    .unwrap();
    // the version the script names, so the test holds the rule and not
    // the number of the day
    let v = rhtn_launch::runner_version();
    let (code, out, _) = h.ask("version");
    assert_eq!(code, 0);
    assert!(out.starts_with(&format!("runner {v} ")), "{out}");
    let before = std::fs::read(h.dir.join("runner")).unwrap();
    let at = format!("RUNNER_VERSION={v}");

    let newer = String::from_utf8(before.clone())
        .unwrap()
        .replace(&at, &format!("RUNNER_VERSION={}", v + 1));
    let ns = rhtn_crypto::ssh::RUNNER_NAMESPACE;
    // signed by the runner key, which an instance holds: refused, nothing written
    let forged = rhtn_crypto::ssh::sshsig(&[6u8; 32], ns, newer.as_bytes());
    let (code, _, err) = h.ask_with("runner-refresh", format!("{forged}{newer}").as_bytes());
    assert_eq!(code, 2, "{err}");
    assert!(
        err.contains("not signed by this machine's operator"),
        "{err}"
    );
    assert_eq!(
        std::fs::read(h.dir.join("runner")).unwrap(),
        before,
        "nothing changed"
    );
    // signed by the operator over other bytes: refused
    let other = rhtn_crypto::ssh::sshsig(&[5u8; 32], ns, b"#!/bin/sh\n");
    let (code, _, _) = h.ask_with("runner-refresh", format!("{other}{newer}").as_bytes());
    assert_eq!(code, 2);
    // signed by the operator under another namespace: refused
    let elsewhere = rhtn_crypto::ssh::sshsig(&[5u8; 32], "file", newer.as_bytes());
    let (code, _, _) = h.ask_with("runner-refresh", format!("{elsewhere}{newer}").as_bytes());
    assert_eq!(code, 2);
    // older: refused even signed
    let older = String::from_utf8(before.clone())
        .unwrap()
        .replace(&at, &format!("RUNNER_VERSION={}", v - 1));
    let sig = rhtn_crypto::ssh::sshsig(&[5u8; 32], ns, older.as_bytes());
    let (code, _, err) = h.ask_with("runner-refresh", format!("{sig}{older}").as_bytes());
    assert_eq!(code, 2, "{err}");
    assert!(err.contains("not taken back"), "{err}");
    assert_eq!(std::fs::read(h.dir.join("runner")).unwrap(), before);
    // signed by the operator and newer: taken, byte for byte, and it says so
    let sig = rhtn_crypto::ssh::sshsig(&[5u8; 32], ns, newer.as_bytes());
    let (code, out, err) = h.ask_with("runner-refresh", format!("{sig}{newer}").as_bytes());
    assert_eq!(code, 0, "{err}");
    assert!(out.starts_with(&format!("runner {} ", v + 1)), "{out}");
    assert_eq!(
        std::fs::read_to_string(h.dir.join("runner")).unwrap(),
        newer
    );
    let (_, out, _) = h.ask("version");
    assert!(out.starts_with(&format!("runner {} ", v + 1)), "{out}");
}

/// **A host for the move**: an engine that writes what it was asked to a
/// log and answers `inspect` with an image, an ssh that keeps its
/// arguments and what came on its standard input, and a root holding a
/// node's state where a machine keeps it.
struct Moving {
    dir: PathBuf,
}

impl Moving {
    fn new(name: &str) -> Moving {
        let dir = std::env::temp_dir().join(format!("rhtn-move-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let root = dir.join("root");
        std::fs::create_dir_all(root.join("var/lib/rhtn")).unwrap();
        std::fs::create_dir_all(root.join("var/lib/rhtn-blocks/res1")).unwrap();
        std::fs::write(root.join("var/lib/rhtn/rhtnd.conf"), "token = \"old\"\n").unwrap();
        std::fs::write(root.join("var/lib/rhtn-blocks/res1/data"), "kept").unwrap();
        let engine = dir.join("engine");
        std::fs::write(
            &engine,
            format!(
                "#!/bin/sh\necho \"$*\" >> {log}\n[ \"$1\" = inspect ] && echo sha256:img\nexit 0\n",
                log = dir.join("engine.log").display()
            ),
        )
        .unwrap();
        let ssh = dir.join("ssh");
        std::fs::write(
            &ssh,
            format!(
                "#!/bin/sh\necho \"$*\" > {args}\ncat > {sent}\n[ -e {fail} ] && {{ echo refused; exit 1; }}\necho received\n",
                args = dir.join("ssh.args").display(),
                sent = dir.join("sent.tar").display(),
                fail = dir.join("ssh.fail").display()
            ),
        )
        .unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            for f in [&engine, &ssh] {
                std::fs::set_permissions(f, std::fs::Permissions::from_mode(0o755)).unwrap();
            }
        }
        Moving { dir }
    }

    fn ask(&self, request: &str, input: &[u8]) -> (i32, String, String) {
        let me = self.dir.join("runner");
        if !me.exists() {
            std::fs::copy(runner(), &me).unwrap();
        }
        let uid = String::from_utf8(Command::new("id").arg("-u").output().unwrap().stdout).unwrap();
        let mut child = Command::new("sh")
            .arg(&me)
            .env("SSH_ORIGINAL_COMMAND", request)
            .env("RHTN_ENGINE", self.dir.join("engine"))
            .env("RHTN_SSH", self.dir.join("ssh"))
            .env("RHTN_ROOT", self.dir.join("root"))
            .env("RHTN_BLOCKS", self.dir.join("root/var/lib/rhtn-blocks"))
            .env("RHTN_UID", uid.trim())
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .expect("sh runs the runner");
        feed(&mut child, input);
        let out = child.wait_with_output().expect("the runner ends");
        (
            out.status.code().unwrap_or(-1),
            String::from_utf8_lossy(&out.stdout).to_string(),
            String::from_utf8_lossy(&out.stderr).to_string(),
        )
    }

    fn engine(&self) -> String {
        std::fs::read_to_string(self.dir.join("engine.log")).unwrap_or_default()
    }

    /// Wait for the detached send to finish: the engine's last word on the
    /// node, `rm` where it went and `start` where it did not.
    fn settled(&self) -> String {
        for _ in 0..100 {
            let e = self.engine();
            if e.contains("rm -f rhtnd") || e.contains("start rhtnd") {
                return e;
            }
            std::thread::sleep(std::time::Duration::from_millis(50));
        }
        self.engine()
    }
}

impl Drop for Moving {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

const RUNNER_KEY: &str = "-----BEGIN OPENSSH PRIVATE KEY-----\nb3BlbnNzaC1rZXktdjEAAAAA\n-----END OPENSSH PRIVATE KEY-----\n";
const RECEIVER: &str = "ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIBzsS0bOBhbs";

// functional: OPS-030
/// **The node goes to the one machine pinned, in one stream, and stops
/// here**: the receiver's host key is the only one accepted, the state and
/// the blocks travel together, and the node here is removed once it has
/// landed — started again where it did not.
#[test]
fn a_node_is_sent_only_to_the_receiver_pinned_and_stops_here_once_it_lands() {
    let h = Moving::new("send");
    // no pin, no send: nothing is stopped
    let (code, _, err) = h.ask("node-send 203.0.113.7", RUNNER_KEY.as_bytes());
    assert_eq!(code, 2, "{err}");
    assert!(err.contains("pinned or not sent to"), "{err}");
    assert!(h.engine().is_empty(), "nothing touched: {}", h.engine());
    for bad in [
        "node-send",
        "node-send 203.0.113.7 extra",
        "node-send host.example",
    ] {
        let (code, _, err) = h.ask(bad, format!("{RUNNER_KEY}{RECEIVER}\n").as_bytes());
        assert_eq!(code, 2, "{bad}: {err}");
    }
    assert!(h.engine().is_empty());

    let (code, out, err) = h.ask(
        "node-send 203.0.113.7",
        format!("{RUNNER_KEY}{RECEIVER}\n").as_bytes(),
    );
    assert_eq!(code, 0, "{err}");
    assert!(out.contains("sending the node to 203.0.113.7"), "{out}");
    let e = h.settled();
    assert!(e.starts_with("stop rhtnd\n"), "the node stops first: {e}");
    assert!(e.contains("rm -f rhtnd"), "and is removed once sent: {e}");
    let args = std::fs::read_to_string(h.dir.join("ssh.args")).unwrap();
    assert!(args.contains("-o StrictHostKeyChecking=yes"), "{args}");
    assert!(args.contains("-o IdentitiesOnly=yes"), "{args}");
    assert!(args.ends_with("root@203.0.113.7 node-receive\n"), "{args}");
    // the stream holds the state and the blocks under a machine's names
    let listed = Command::new("tar")
        .arg("-tf")
        .arg(h.dir.join("sent.tar"))
        .output()
        .unwrap();
    let listed = String::from_utf8_lossy(&listed.stdout).to_string();
    assert!(listed.contains("var/lib/rhtn/rhtnd.conf"), "{listed}");
    assert!(listed.contains("var/lib/rhtn-blocks/res1/data"), "{listed}");

    // a send that does not land puts the node back
    let h = Moving::new("send-fails");
    std::fs::write(h.dir.join("ssh.fail"), "").unwrap();
    let (code, _, err) = h.ask(
        "node-send 203.0.113.7",
        format!("{RUNNER_KEY}{RECEIVER}\n").as_bytes(),
    );
    assert_eq!(code, 0, "{err}");
    let e = h.settled();
    assert!(e.contains("start rhtnd"), "started again: {e}");
    assert!(!e.contains("rm -f rhtnd"), "and not removed: {e}");
}

// functional: OPS-030
/// **A node received takes the place of the one serving, on this
/// machine's image**, what it replaced kept beside it; what is not a
/// node's state changes nothing.
#[test]
fn a_node_received_replaces_the_one_serving_and_keeps_what_it_replaced() {
    // the arrival, made as a sender makes it
    let from = Moving::new("arrival");
    std::fs::write(
        from.dir.join("root/var/lib/rhtn/rhtnd.conf"),
        "token = \"moved\"\n",
    )
    .unwrap();
    let tar = Command::new("tar")
        .arg("-C")
        .arg(from.dir.join("root"))
        .args(["-cf", "-", "var/lib/rhtn", "var/lib/rhtn-blocks"])
        .output()
        .unwrap()
        .stdout;

    let h = Moving::new("receive");
    let (code, _, err) = h.ask("node-receive", b"not a tar");
    assert_eq!(code, 2, "{err}");
    assert!(
        h.engine().lines().all(|l| !l.starts_with("rm")),
        "{}",
        h.engine()
    );
    let state = h.dir.join("root/var/lib/rhtn");
    assert_eq!(
        std::fs::read_to_string(state.join("rhtnd.conf")).unwrap(),
        "token = \"old\"\n"
    );

    let (code, out, err) = h.ask("node-receive", &tar);
    assert_eq!(code, 0, "{err}");
    assert!(out.contains("received the node"), "{out}");
    assert_eq!(
        std::fs::read_to_string(state.join("rhtnd.conf")).unwrap(),
        "token = \"moved\"\n",
        "the arrival serves"
    );
    let e = h.engine();
    assert!(e.contains("rm -f rhtnd"), "{e}");
    assert!(
        e.contains("run -d --name rhtnd --restart=always --network=host"),
        "{e}"
    );
    assert!(
        e.trim_end().ends_with("sha256:img"),
        "on this machine's image: {e}"
    );
    let kept: Vec<String> = std::fs::read_dir(h.dir.join("root/var/lib"))
        .unwrap()
        .map(|d| d.unwrap().file_name().to_string_lossy().to_string())
        .collect();
    assert!(
        kept.iter().any(|n| n.starts_with("rhtn.replaced-")),
        "what it replaced is kept: {kept:?}"
    );
    assert_eq!(
        std::fs::read_to_string(h.dir.join("root/var/lib/rhtn-blocks/res1/data")).unwrap(),
        "kept"
    );
}
