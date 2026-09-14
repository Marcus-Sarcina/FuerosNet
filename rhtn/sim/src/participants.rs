//! Several `rhtnp` processes, as a scenario drives them.
//!
//! The companion to [`crate::daemons`], and for the same reason: a claim
//! about the client is worth more made against **the binary a person
//! runs** than against a `Client` a test holds.  What that buys here is
//! the part no in-process harness can show — that a client's whole
//! conversation with its serving node crosses a socket, out of a process
//! that was started from an identity file and told nothing else.
//!
//! **A command is a line and an answer ends with `end`.**  The instrument
//! prints a variable number of lines and a harness cannot guess how many,
//! so it reads until the terminator.

use crate::daemons::{hex, write_identity};
use rhtn_crypto::SigningIdentity;
use rhtn_crypto::identity::testkit::test_identity;
use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};

/// One running `rhtnp`, and the pipe a scenario talks to it down.
pub struct Party {
    pub name: String,
    pub keyhash: [u8; 32],
    pub dir: PathBuf,
    stdin: ChildStdin,
    stdout: BufReader<ChildStdout>,
    child: Child,
}

impl Party {
    /// Run one command and return what it said, without the terminator.
    pub fn tell(&mut self, command: &str) -> Vec<String> {
        writeln!(self.stdin, "{command}").unwrap_or_else(|e| panic!("{}: {e}", self.name));
        self.stdin.flush().expect("flushed");
        let mut out = Vec::new();
        loop {
            let mut line = String::new();
            let n = self.stdout.read_line(&mut line).unwrap_or_else(|e| panic!("{}: {e}", self.name));
            if n == 0 {
                panic!("{} stopped mid-command after {out:?}", self.name);
            }
            let line = line.trim_end().to_string();
            if line == "end" {
                return out;
            }
            out.push(line);
        }
    }

    /// Run one command and insist it did not refuse.
    pub fn must(&mut self, command: &str) -> Vec<String> {
        let out = self.tell(command);
        assert!(!out.iter().any(|l| l.starts_with("error ")), "{}: `{command}` refused: {out:?}", self.name);
        out
    }

    pub fn identity(&self) -> SigningIdentity {
        test_identity(&self.name)
    }
}

impl Drop for Party {
    fn drop(&mut self) {
        let _ = writeln!(self.stdin, "quit");
        let _ = self.stdin.flush();
        let _ = self.child.wait();
    }
}

/// A set of `rhtnp` processes under one directory, stopped and removed
/// when the set is dropped.
pub struct Participants {
    exe: PathBuf,
    root: PathBuf,
    parties: Vec<Party>,
}

impl Participants {
    /// A set rooted at a fresh directory.  `exe` is the `rhtnp` binary,
    /// which a caller passes as `env!("CARGO_BIN_EXE_rhtnp")`.
    pub fn new(exe: impl Into<PathBuf>, tag: &str) -> Participants {
        let root = std::env::temp_dir().join(format!("rhtn-parties-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).expect("a directory for the set");
        Participants { exe: exe.into(), root, parties: Vec::new() }
    }

    /// Start `name`, authenticating `peers` and never itself.
    pub fn start(&mut self, name: &str, peers: &[&str]) -> &mut Party {
        let dir = self.root.join(name);
        std::fs::create_dir_all(&dir).expect("a directory for the party");
        let identity = dir.join("identity.key");
        write_identity(&identity, name);
        let list: String = peers.iter().filter(|p| **p != name).map(|p| format!("{}\n", hex(&test_identity(p).public.key_material()))).collect();
        std::fs::write(dir.join("peers"), list).expect("the peers file");

        let mut child = Command::new(&self.exe)
            .arg(&identity)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .unwrap_or_else(|e| panic!("{} starts: {e}", self.exe.display()));
        let stdin = child.stdin.take().expect("stdin");
        let mut stdout = BufReader::new(child.stdout.take().expect("stdout"));
        let mut first = String::new();
        stdout.read_line(&mut first).expect("the participant says who it is");
        let said = first.trim().rsplit_once(' ').unwrap_or_else(|| panic!("the first line names a keyhash, not {first:?}")).1.to_string();
        assert_eq!(said, hex(&test_identity(name).public.keyhash), "the process runs under the identity it was given");

        self.parties.push(Party { name: name.to_string(), keyhash: test_identity(name).public.keyhash, dir, stdin, stdout, child });
        self.parties.last_mut().expect("just pushed")
    }

    pub fn get(&mut self, name: &str) -> &mut Party {
        self.parties.iter_mut().find(|p| p.name == name).unwrap_or_else(|| panic!("no party {name}"))
    }
}

impl Drop for Participants {
    fn drop(&mut self) {
        self.parties.clear();
        let _ = std::fs::remove_dir_all(&self.root);
    }
}
