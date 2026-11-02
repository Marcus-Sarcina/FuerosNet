//! **The one program this instance may run on its host**, and how it asks
//! for it [author, 2026-11-02].
//!
//! ## Why an instance asks its host anything at all
//!
//! `infra-client-requirements.md` §9 has a package distributed as an image,
//! and `resource-requirements.md` §3 has a resource that holds its own port
//! reached by the node over a local socket. **Something with control of the
//! machine has to start it**, and the node is deliberately not that: its
//! container is given no engine socket and no privilege, because design
//! §18.1 plans for a seized instance and an instance that could reach its
//! host would make that seizure the host's.
//!
//! The operator could do it from a shell, and did. **But the operator we
//! are building for has no shell** [author, 2026-11-02]: "we have to assume
//! operator-users whose only interaction with cloud administration is
//! through our own app interface". So the act has to be reachable from the
//! app, which reaches only the administration surface, which is the node.
//!
//! ## So the node asks, and the host decides what may be asked
//!
//! The instance shares its host's network namespace (`rhtn_launch::user_data`
//! runs it `--network=host`), so **the host's shell is on the loopback they
//! share**. The node opens an ssh session to it carrying a key its
//! operator's client hands it *for that one act* — and the host's
//! `authorized_keys` binds that key to `rhtn-package-runner` with
//! `command=`, so the whole of what the key can do is written down on the
//! host (`rhtn_crypto::ssh::RUNNER_OPTIONS`,
//! `crates/packaging/package-runner.sh`).
//!
//! **The node never holds the key.** It arrives with the request, is
//! written to a file only this process may read, used once, and removed.
//! An instance seized between two acts holds nothing that would let it ask
//! its host for anything, which is the property that makes this safe to
//! have at all.
//!
//! **And it is not a shell.** Three things stand between a request and the
//! machine: this module refuses a word that is not a bare token, the
//! `command=` option means the host runs the runner whatever the session
//! asks for, and the runner refuses a verb it does not know. Each is
//! enough on its own; the first is here so that a refusal reaches the
//! operator as a sentence rather than as a failed session.

use std::io::Write;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};
use zeroize::Zeroizing;

/// **How long one act may take.** A pull fetches an image over the
/// machine's own network, which is the slow part and is not this
/// project's to make faster; a start and a stop are immediate. Bounded
/// because a session that never ends would hold the surface, which serves
/// one caller at a time.
pub const DEADLINE: Duration = Duration::from_secs(240);

/// How long the session may spend reaching the host. The host is on the
/// loopback, so this is a bound on "sshd is not there" rather than on a
/// network.
pub const CONNECT_SECONDS: u64 = 5;

/// How often the wait looks at the child. A pull takes seconds at least,
/// so there is nothing to gain from looking more often than this and a
/// poll is what the standard library gives without a runtime.
const LOOK: Duration = Duration::from_millis(100);

/// **The host's shell, as this instance may reach it**: where the host's
/// sshd answers on the loopback they share.
///
/// **A node without one installs nothing**, which is the honest state of a
/// node whose operator gave it no `host-shell`: a node that pretended
/// otherwise would be telling its operator an install had failed for a
/// reason it had made up.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Shell {
    /// Where the host's sshd answers.
    pub at: SocketAddr,
}

/// The user the runner key is installed for: root's `authorized_keys`,
/// because that is who the provider's image gives a shell to and who the
/// launch writes the two keys into (`rhtn_launch::user_data`).
pub const USER: &str = "root";

impl Shell {
    /// **Reaching the host over anything but the loopback is refused.**
    /// The session carries a private key and asks for control of a
    /// machine; this leg is plain ssh to a host whose key nothing here
    /// pins, which is defensible between a container and the machine it
    /// is running on and is not defensible anywhere else. An operator
    /// whose shell is elsewhere is told so rather than having their key
    /// sent there.
    pub fn of(at: SocketAddr) -> Result<Shell, String> {
        if !at.ip().is_loopback() {
            return Err(format!(
                "`{at}` is not a loopback address: an instance asks its own host and no other \
                 machine, and this session's host key is nothing this node pins"
            ));
        }
        Ok(Shell { at })
    }

    /// Ask the host's runner for one act: the verb and its arguments, as
    /// the runner's own vocabulary names them.
    ///
    /// `key` is the private half of the restricted key, as the operator's
    /// client sent it, held no longer than the call.
    pub fn ask(&self, key: &str, request: &[&str]) -> Result<String, String> {
        let line = bare(request)?;
        let held = Secret::write(key)?;
        // **-F /dev/null and no known hosts**: this process has no home
        // directory to read a configuration from and keeps no host keys,
        // so every option the session runs under is on this line and
        // nothing of it survives the call
        let mut child = Command::new(client())
            .args([
                "-F",
                "/dev/null",
                "-i",
                &held.path.display().to_string(),
                "-o",
                "IdentitiesOnly=yes",
                "-o",
                "BatchMode=yes",
                "-o",
                "StrictHostKeyChecking=no",
                "-o",
                "UserKnownHostsFile=/dev/null",
                "-o",
                "GlobalKnownHostsFile=/dev/null",
                "-o",
                "LogLevel=ERROR",
                "-o",
                &format!("ConnectTimeout={CONNECT_SECONDS}"),
                "-p",
                &self.at.port().to_string(),
                &format!("{USER}@{}", self.at.ip()),
                &line,
            ])
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| format!("this image has no ssh client, or it would not run: {e}"))?;
        // **waited for by hand, and killed at the deadline.** The surface
        // takes one caller at a time and this call is the caller, so the
        // bound has to be on the child rather than on a future nothing
        // will poll while this blocks.
        let began = Instant::now();
        loop {
            match child.try_wait() {
                Err(e) => return Err(format!("the session could not be waited for: {e}")),
                Ok(Some(_)) => break,
                Ok(None) => {}
            }
            if began.elapsed() > DEADLINE {
                let _ = child.kill();
                let _ = child.wait();
                return Err(format!(
                    "the host did not answer `{line}` within {}s",
                    DEADLINE.as_secs()
                ));
            }
            std::thread::sleep(LOOK);
        }
        let out = child
            .wait_with_output()
            .map_err(|e| format!("the session's output could not be read: {e}"))?;
        let said = format!(
            "{}{}",
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        );
        if out.status.success() {
            return Ok(said);
        }
        // **255 is ssh's own**: the session never reached the runner, so
        // the operator is looking at a key or at sshd and not at a verb
        let code = out.status.code().unwrap_or(-1);
        Err(match code {
            255 => format!(
                "the host did not take the session: {}. The key this act carried is the \
                 restricted one the launch installed, and the host answers on {}",
                said.trim(),
                self.at
            ),
            _ => format!("the host refused `{line}`: {}", said.trim()),
        })
    }
}

/// **The ssh client this node runs**, which is the one in its image
/// (`crates/packaging/Containerfile` installs `openssh-client` for this
/// and nothing else).
///
/// `RHTN_SSH_CLIENT` names another, which is how the tests stand a
/// recorder in the client's place and how an operator whose image
/// carries a different one points at it. **It is not a privilege**: a
/// caller who can set this node's environment is already inside the
/// process, and the request is still only ever the runner's vocabulary
/// ([`bare`]).
pub fn client() -> String {
    std::env::var("RHTN_SSH_CLIENT").unwrap_or_else(|_| "ssh".into())
}

/// **What may be sent as a request**: bare tokens, joined by single
/// spaces, which is what `SSH_ORIGINAL_COMMAND` carries and what the
/// runner splits.
///
/// The runner refuses the same things and would refuse these; this is
/// here so a request that could never be taken is refused where the
/// operator is standing rather than after a session to the host.
fn bare(request: &[&str]) -> Result<String, String> {
    if request.is_empty() {
        return Err("an act names a verb".into());
    }
    for word in request {
        if word.is_empty() {
            return Err("an act carries no empty word".into());
        }
        if !word
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"./:=,_@-".contains(&b))
        {
            return Err(format!(
                "`{word}` is not a bare token: an act carries letters, digits and \
                 `. / : = , _ @ -` and nothing a shell would read"
            ));
        }
    }
    Ok(request.join(" "))
}

/// **A private key on disk for the length of one act**: written where
/// only this process may read it, and removed when the call returns
/// whichever way it goes.
struct Secret {
    path: PathBuf,
}

impl Secret {
    /// Write the key at 0600, failing rather than reusing a path that
    /// already exists — a key written over a file somebody else made
    /// would be a key in a place this node did not choose.
    fn write(key: &str) -> Result<Secret, String> {
        let body = Zeroizing::new(normalised(key));
        let dir = std::env::temp_dir();
        let path = dir.join(format!(
            "rhtn-runner-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        ));
        let mut options = std::fs::OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let mut file = options
            .open(&path)
            .map_err(|e| format!("{}: {e}", path.display()))?;
        file.write_all(body.as_bytes())
            .and_then(|()| file.flush())
            .map_err(|e| format!("{}: {e}", path.display()))?;
        Ok(Secret { path })
    }
}

impl Drop for Secret {
    fn drop(&mut self) {
        // **overwritten before it is removed**: the bytes are a private
        // key, and an unlink leaves them where the filesystem put them
        if let Ok(len) = std::fs::metadata(&self.path).map(|m| m.len())
            && let Ok(mut f) = std::fs::OpenOptions::new().write(true).open(&self.path)
        {
            let _ = f.write_all(&vec![0u8; len as usize]);
            let _ = f.flush();
        }
        let _ = std::fs::remove_file(&self.path);
    }
}

/// **A key as ssh insists on reading one**: the client's lines, and a
/// final newline, which `ssh` requires and a form field loses.
fn normalised(key: &str) -> String {
    let mut out: String = key.replace("\r\n", "\n").trim_end().to_string();
    out.push('\n');
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    // acceptance: DMN-37
    #[test]
    fn an_act_carries_bare_tokens_and_nothing_a_shell_would_read() {
        assert_eq!(
            bare(&["start", "abc", "ggcr.io/x/y:1"]).unwrap(),
            "start abc ggcr.io/x/y:1"
        );
        assert_eq!(bare(&["list"]).unwrap(), "list");
        for bad in [
            "a;b", "a|b", "a&b", "a`b", "a$(b)", "a>b", "a<b", "a b", "a\nb", "a'b", "a\"b",
        ] {
            assert!(
                bare(&["start", bad]).is_err(),
                "`{bad}` is not a bare token"
            );
        }
        assert!(bare(&[]).is_err());
        assert!(bare(&["start", ""]).is_err());
    }

    // acceptance: DMN-37
    #[test]
    fn a_host_this_instance_does_not_share_is_refused() {
        assert!(Shell::of("127.0.0.1:22".parse().unwrap()).is_ok());
        assert!(Shell::of("[::1]:22".parse().unwrap()).is_ok());
        let far = Shell::of("203.0.113.9:22".parse().unwrap());
        assert!(far.is_err());
        assert!(far.unwrap_err().contains("loopback"));
    }

    // acceptance: DMN-37
    #[test]
    fn the_key_is_on_disk_for_one_act_and_at_0600() {
        let held = Secret::write("-----BEGIN OPENSSH PRIVATE KEY-----\nabc\n").unwrap();
        let path = held.path.clone();
        assert!(path.exists());
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = std::fs::metadata(&path).unwrap().permissions().mode();
            assert_eq!(mode & 0o777, 0o600, "a private key is the owner's alone");
        }
        // what ssh insists on: the key's own lines and a final newline
        let held_text = std::fs::read_to_string(&path).unwrap();
        assert!(held_text.ends_with("abc\n"));
        drop(held);
        assert!(!path.exists(), "nothing of the key outlives the act");
    }

    // acceptance: DMN-37
    #[test]
    fn a_key_a_form_mangled_is_read_as_ssh_reads_one() {
        assert_eq!(normalised("a\r\nb"), "a\nb\n");
        assert_eq!(normalised("a\nb\n\n\n"), "a\nb\n");
        assert_eq!(normalised("a"), "a\n");
    }
}
