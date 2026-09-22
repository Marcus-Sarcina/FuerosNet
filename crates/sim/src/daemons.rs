//! Several `rhtnd` processes, as a scenario drives them.
//!
//! `scenario::Running` starts a node inside the test's own process, which
//! is what most of this crate's scenarios want: they reach into a view, or
//! they inject at the datagram layer, and neither is possible across a
//! process boundary.  This is the other harness, for the claims that are
//! about **the binary an operator actually runs**
//! (`Robot/implementation-plan.md`'s milestone 11): its configuration file,
//! its own copy of the state, the signal it stops on, and what it holds
//! when it comes back.
//!
//! **A daemon is addressed after it starts, never before.**  Its
//! configuration says `listen = 127.0.0.1:0` and it prints where it bound,
//! so a patron is started first and a subordinate is configured with the
//! address the patron reported.  That is the order a real deployment has,
//! and it is why the harness hands out addresses rather than assigning
//! them.

use rhtn_crypto::SigningIdentity;
use rhtn_crypto::identity::testkit::test_identity;
use std::io::{BufRead, BufReader};
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

/// One running `rhtnd`, with the files it was started from.
pub struct Daemon {
    pub name: String,
    pub keyhash: [u8; 32],
    pub addr: SocketAddr,
    pub dir: PathBuf,
    pub config: PathBuf,
    pub peers: PathBuf,
    child: Option<Child>,
}

impl Daemon {
    pub fn identity(&self) -> SigningIdentity {
        test_identity(&self.name)
    }

    /// The topology store's directory, for a scenario that reads what the
    /// process wrote rather than what it holds.
    pub fn topology(&self) -> PathBuf {
        self.dir.join("topology")
    }

    pub fn archive(&self) -> PathBuf {
        self.dir.join("archive")
    }

    /// Whether the process is still up.
    pub fn running(&mut self) -> bool {
        match self.child.as_mut() {
            None => false,
            Some(c) => c.try_wait().ok().flatten().is_none(),
        }
    }
}

/// A set of `rhtnd` processes under one directory, stopped and removed
/// when the set is dropped.
pub struct Daemons {
    exe: PathBuf,
    root: PathBuf,
    /// In the order they were started, which is the order they are stopped.
    daemons: Vec<Daemon>,
    /// Hosting files written before a daemon starts, by name.  A scenario
    /// that gives a daemon packages writes them first, because the daemon
    /// admits them before it serves.
    hosting: Vec<String>,
}

impl Daemons {
    /// A set rooted at a fresh directory.  `exe` is the `rhtnd` binary,
    /// which a caller passes as `env!("CARGO_BIN_EXE_rhtnd")`: only the
    /// package that declares the binary is told where cargo put it.
    pub fn new(exe: impl Into<PathBuf>, tag: &str) -> Daemons {
        let root = std::env::temp_dir().join(format!("rhtn-daemons-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).expect("a directory for the set");
        Daemons {
            exe: exe.into(),
            root,
            daemons: Vec::new(),
            hosting: Vec::new(),
        }
    }

    /// The directory `name`'s files live in, made if it is not there yet,
    /// so a scenario can put a package beside the configuration before the
    /// process reads either.
    pub fn dir(&self, name: &str) -> PathBuf {
        let d = self.root.join(name);
        std::fs::create_dir_all(&d).expect("a directory for the daemon");
        d
    }

    /// Write `name`'s hosting file, to be named by the configuration when
    /// it starts.  Called before `start`.
    pub fn hosting(&mut self, name: &str, text: &str) -> PathBuf {
        let p = self.dir(name).join("hosting");
        std::fs::write(&p, text).expect("the hosting file");
        self.hosting.push(name.to_string());
        p
    }

    /// The identity, peers and configuration one daemon starts from.
    ///
    /// One writer for both `start` and `refuses`, because a second copy
    /// drifts: the bare keys must precede every table header or TOML puts
    /// them inside the last one, and that is easy to get right once and
    /// wrong twice.
    fn write_files(
        &self,
        name: &str,
        dir: &Path,
        peers: &[&str],
        up: Option<&(String, SocketAddr)>,
    ) -> (PathBuf, PathBuf) {
        let (config, peers_path) = (dir.join("rhtnd.conf"), dir.join("peers"));
        write_identity(&dir.join("identity.key"), name);
        let list: String = peers
            .iter()
            .filter(|p| **p != name)
            .map(|p| format!("{}\n", hex(&test_identity(p).public.key_material())))
            .collect();
        std::fs::write(&peers_path, list).expect("the peers file");
        let mut text = String::new();
        if self.hosting.iter().any(|h| h == name) {
            text.push_str(&format!(
                "resources = \"{}\"\n",
                dir.join("hosting").display()
            ));
        }
        text.push_str(&format!(
            "identity = \"{}\"\nlisten = \"127.0.0.1:0\"\nqueue = \"{}\"\nprekeys = \"{}\"\ntopology = \"{}\"\narchive = \"{}\"\nheartbeat = 30\ningestion = \"unverified-gossip\"\n\n[allowance]\nrequests = 120\nseconds = 60\n",
            dir.join("identity.key").display(),
            dir.join("queue").display(),
            dir.join("prekeys").display(),
            dir.join("topology").display(),
            dir.join("archive").display()
        ));
        if let Some((key, addr)) = up {
            text.push_str(&format!(
                "\n[upstream]\nnode = \"{key}\"\naddresses = [\"{addr}\"]\n"
            ));
        }
        std::fs::write(&config, text).expect("the configuration");
        (config, peers_path)
    }

    /// Start `name`, authenticating `peers`, with `upstream` as its patron
    /// where it has one.  Returns where it bound.
    ///
    /// The peers file names the parties this node authenticates and never
    /// itself: an operator should not have to list their own key.
    pub fn start(&mut self, name: &str, peers: &[&str], upstream: Option<&str>) -> SocketAddr {
        let up = upstream.map(|u| {
            let d = self.get(u);
            (hex(&d.keyhash), d.addr)
        });
        let dir = self.root.join(name);
        std::fs::create_dir_all(&dir).expect("a directory for the daemon");
        let (config, peers_path) = self.write_files(name, &dir, peers, up.as_ref());
        let mut d = Daemon {
            name: name.to_string(),
            keyhash: test_identity(name).public.keyhash,
            addr: "127.0.0.1:0".parse().unwrap(),
            dir,
            config,
            peers: peers_path,
            child: None,
        };
        d.addr = spawn(&self.exe, &d.config, &d.peers, &mut d.child);
        let addr = d.addr;
        self.daemons.push(d);
        addr
    }

    /// Start `name` expecting it not to come up, and return what it said
    /// on the way out.  A configuration a daemon will not accept is part
    /// of what a configuration file is for, and the message is the whole
    /// of what an operator gets.
    pub fn refuses(&mut self, name: &str, peers: &[&str]) -> String {
        let dir = self.dir(name);
        let (config, peers_path) = self.write_files(name, &dir, peers, None);
        let out = Command::new(&self.exe)
            .arg(&config)
            .arg(&peers_path)
            .output()
            .expect("the daemon runs");
        assert!(
            !out.status.success(),
            "{name} was expected not to start, and it did"
        );
        format!(
            "{}{}",
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        )
    }

    pub fn get(&self, name: &str) -> &Daemon {
        self.daemons
            .iter()
            .find(|d| d.name == name)
            .unwrap_or_else(|| panic!("no daemon {name}"))
    }

    /// Whether `name`'s process is still up.
    pub fn running(&mut self, name: &str) -> bool {
        let i = self.index(name);
        self.daemons[i].running()
    }

    fn index(&self, name: &str) -> usize {
        self.daemons
            .iter()
            .position(|d| d.name == name)
            .unwrap_or_else(|| panic!("no daemon {name}"))
    }

    /// SIGTERM, then wait: the daemon writes its state back on the way out,
    /// and a scenario that stopped one is entitled to find that state.
    pub fn stop(&mut self, name: &str) {
        let i = self.index(name);
        if let Some(child) = self.daemons[i].child.take() {
            terminate(child, true);
        }
    }

    /// Start it again from the same files, and take the address it reports.
    ///
    /// **The address may differ**, since the configuration asks for an
    /// ephemeral port: a scenario re-reads it rather than assuming the old
    /// one, which is what a client holding a stale locator has to do too.
    pub fn restart(&mut self, name: &str) -> SocketAddr {
        self.stop(name);
        let i = self.index(name);
        let (config, peers) = (
            self.daemons[i].config.clone(),
            self.daemons[i].peers.clone(),
        );
        let mut child = None;
        let addr = spawn(&self.exe, &config, &peers, &mut child);
        self.daemons[i].child = child;
        self.daemons[i].addr = addr;
        addr
    }

    pub fn names(&self) -> Vec<String> {
        self.daemons.iter().map(|d| d.name.clone()).collect()
    }
}

impl Drop for Daemons {
    fn drop(&mut self) {
        // a scenario that failed leaves processes behind otherwise, and a
        // stray daemon holds a port and a directory
        for d in &mut self.daemons {
            if let Some(child) = d.child.take() {
                terminate(child, false);
            }
        }
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

fn spawn(exe: &Path, config: &Path, peers: &Path, slot: &mut Option<Child>) -> SocketAddr {
    let mut child = Command::new(exe)
        .arg(config)
        .arg(peers)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap_or_else(|e| panic!("{} starts: {e}", exe.display()));
    let mut out = BufReader::new(child.stdout.take().expect("stdout"));
    let mut err = child.stderr.take().expect("stderr");
    let mut line = String::new();
    out.read_line(&mut line)
        .expect("the daemon says where it is serving");
    // a daemon that refused its configuration said why before it exited,
    // and a harness that swallows that turns every configuration mistake
    // into the same unhelpful failure
    if line.trim().is_empty() {
        use std::io::Read;
        let mut said = String::new();
        let _ = err.read_to_string(&mut said);
        let _ = child.wait();
        panic!("the daemon did not start: {}", said.trim());
    }
    let addr = line
        .trim()
        .rsplit_once(' ')
        .unwrap_or_else(|| panic!("the daemon's first line names an address, not {line:?}"))
        .1
        .parse()
        .unwrap_or_else(|e| panic!("an address and port in {line:?}: {e}"));
    *slot = Some(child);
    addr
}

/// Stop one process.  `expect_clean` where the scenario stopped it on
/// purpose and the exit status is part of what is being tested.
fn terminate(mut child: Child, expect_clean: bool) {
    #[cfg(unix)]
    unsafe {
        kill(child.id() as i32, 15);
    }
    let end = Instant::now() + Duration::from_secs(10);
    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                if expect_clean {
                    assert!(
                        status.success(),
                        "the daemon exits cleanly on SIGTERM: {status:?}"
                    );
                }
                return;
            }
            Ok(None) if Instant::now() < end => std::thread::sleep(Duration::from_millis(50)),
            _ => {
                let _ = child.kill();
                let _ = child.wait();
                if expect_clean {
                    panic!("the daemon did not stop on SIGTERM");
                }
                return;
            }
        }
    }
}

#[cfg(unix)]
unsafe extern "C" {
    fn kill(pid: i32, sig: i32) -> i32;
}

pub fn write_identity(path: &Path, name: &str) {
    let mut bytes =
        rhtn_codec::cose::sha256(format!("rhtn-test-vectors:{name}:ed25519-seed").as_bytes())
            .to_vec();
    bytes.extend_from_slice(&rhtn_codec::cose::sha256(
        format!("rhtn-test-vectors:{name}:ml-dsa-65-seed").as_bytes(),
    ));
    std::fs::write(path, &bytes).expect("the identity file");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        // the daemon refuses one readable beyond its owner
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600)).expect("owner-only");
    }
}

/// Lower-case hex, which is the only form a keyhash is written in.
pub fn hex(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect()
}
