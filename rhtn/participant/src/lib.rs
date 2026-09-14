//! `rhtnp`: a participant an experiment can drive.
//!
//! **This is an instrument and not a product.** `Robot/implementation-plan.md`
//! puts the shells at milestone 14 and they wait on two decisions and a
//! toolchain; the client itself has been finished and tested for longer
//! than that, in a process that nothing outside a test ever started. What
//! was missing was never a library — it was something a person can run, so
//! that the chain from a ceremony to an adoption to a payload can be
//! *observed* crossing real sockets between separately configured
//! processes rather than asserted inside one.
//!
//! It therefore claims none of the product entries. PRD-01 to PRD-09 are
//! obligations about what a user is shown and when they are asked, and a
//! command read from standard input is not a person; an instrument that
//! marked them would be marking them falsely.
//!
//! **Nothing is kept.** There is no store on disk beyond the identity it
//! was started from: a run is a run. That is a limit worth stating rather
//! than working around, because a client that persisted here would be
//! making decisions about what a client persists, and those belong to the
//! shells.
//!
//! One command a line on standard input, one or more lines out, each
//! beginning with a word that says what it is. Everything a script needs
//! to tell the two apart is in that first word.

pub mod terminal;

use rhtn_ffi::client::Participant;
use rhtn_ffi::client::platform;
use rhtn_ffi::net::Wake;
use rhtn_ffi::types::{Channel, ChannelOutcome};
use std::path::Path;
use std::sync::Arc;
use terminal::{Terminal, hex, unhex};

/// The seeds an identity is derived from: the classical one and the
/// post-quantum one, in that order, as every other reader of one takes
/// them (design §5.1).
pub const IDENTITY_BYTES: usize = 64;

/// A running participant and the terminal it reaches the world through.
pub struct Instrument {
    client: Participant,
    shell: Arc<Terminal>,
}

/// What running one command came to.
pub enum Outcome {
    /// Lines to print, in order.
    Said(Vec<String>),
    /// The instrument was told to stop.
    Done,
}

impl Instrument {
    /// Start from an identity file and the peers it authenticates.
    ///
    /// **The identity is read and never minted**, for the reason the
    /// daemon's is: a participant that generates a key when its file is
    /// missing runs under an identity nobody has met, and the person
    /// running it would not know. `rhtn keys mint` is where one comes
    /// from.
    pub fn start(identity: &Path, peers: Option<&Path>) -> Result<Instrument, String> {
        let seeds = read_identity(identity)?;
        let known = match peers {
            None => Vec::new(),
            Some(p) => read_peers(p)?,
        };
        let shell = Arc::new(Terminal::default());
        let p = platform(shell.clone(), shell.clone(), shell.clone(), shell.clone(), shell.clone(), shell.clone());
        let client = Participant::start(seeds, known, p).map_err(|e| e.reason)?;
        Ok(Instrument { client, shell })
    }

    /// What this participant is called.
    pub fn me(&self) -> String {
        hex(&self.client.me())
    }

    /// Run one command.  Anything the client said on the way — a question
    /// it put to the person, a notice it raised — follows the answer, so a
    /// transcript reads in the order the work happened.
    pub fn run(&self, line: &str) -> Outcome {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            return Outcome::Said(Vec::new());
        }
        let f: Vec<&str> = line.split_whitespace().collect();
        if f[0] == "quit" {
            return Outcome::Done;
        }
        let mut out = match self.dispatch(&f) {
            Ok(lines) => lines,
            Err(e) => vec![format!("error {e}")],
        };
        out.extend(self.shell.drain());
        Outcome::Said(out)
    }

    fn dispatch(&self, f: &[&str]) -> Result<Vec<String>, String> {
        match f {
            ["help"] => Ok(HELP.lines().map(str::to_string).collect()),
            ["me"] => Ok(vec![format!("me {}", self.me())]),

            ["attached"] => Ok(vec![format!("attached {}", self.client.attached())]),

            ["attach", node, addresses, population @ ..] => {
                let node = id(node)?;
                let addrs: Vec<String> = addresses.split(',').map(str::to_string).collect();
                let pop: Result<Vec<Vec<u8>>, String> = population.iter().map(|p| id(p)).collect();
                let a = self.client.attach(node, addrs, pop?).map_err(|e| e.reason)?;
                Ok(vec![format!("attached serving={} primary={} queued={}", hex(&a.serving), a.primary, a.queued)])
            }

            ["maintain"] => {
                self.client.maintain().map_err(|e| e.reason)?;
                Ok(vec!["maintained".into()])
            }

            ["send", to, kind, body] => {
                let k: u64 = kind.parse().map_err(|_| format!("`{kind}` is not a payload kind"))?;
                let bytes = unhex(body).ok_or_else(|| format!("`{body}` is not hex"))?;
                self.client.send(id(to)?, k, bytes).map_err(|e| e.reason)?;
                Ok(vec!["sent".into()])
            }

            ["wake", "off"] => {
                self.client.wake(None).map_err(|e| e.reason)?;
                Ok(vec!["wake off".into()])
            }
            ["wake", url, key, rest @ ..] => {
                let lapses_at = match rest {
                    [] => None,
                    [t] => Some(t.parse::<u64>().map_err(|_| format!("`{t}` is not a time"))?),
                    _ => return Err("wake <url> <key-hex> [<lapses-at>]".into()),
                };
                let key = unhex(key).ok_or_else(|| format!("`{key}` is not hex"))?;
                self.client.wake(Some(Wake { url: (*url).to_string(), key, lapses_at })).map_err(|e| e.reason)?;
                Ok(vec!["wake set".into()])
            }

            ["events"] => self.events(0),
            ["events", ms] => self.events(ms.parse::<u64>().map_err(|_| format!("`{ms}` is not a count of milliseconds"))?),

            ["channel"] => Ok(self
                .shell
                .declarations()
                .iter()
                .map(|(c, o, m)| format!("channel {} {}{}", channel_name(*c), outcome_name(*o), m.map_or(String::new(), |m| format!(" {m}"))))
                .collect()),
            ["channel", name, outcome, rest @ ..] => {
                let c = channel_of(name)?;
                let metres = match rest {
                    [] => None,
                    [m] => Some(m.parse::<u64>().map_err(|_| format!("`{m}` is not a count of metres"))?),
                    _ => return Err("channel <name> <pass|fail|unavailable> [<metres>]".into()),
                };
                match *outcome {
                    "none" => self.shell.declare(c, None),
                    _ => self.shell.declare(c, Some((outcome_of(outcome)?, metres))),
                }
                Ok(vec![format!("channel {name} {outcome}")])
            }

            ["answer"] => Ok(vec![format!("answer {}", if self.shell.answering() { "yes" } else { "no" })]),
            ["answer", "yes"] => {
                self.shell.answers(true);
                Ok(vec!["answer yes".into()])
            }
            ["answer", "no"] => {
                self.shell.answers(false);
                Ok(vec!["answer no".into()])
            }

            _ => Err(format!("`{}` is not a command; `help` lists them", f.join(" "))),
        }
    }

    fn events(&self, ms: u64) -> Result<Vec<String>, String> {
        let mut out = Vec::new();
        while let Some(e) = self.client.next_event(ms) {
            out.push(match e {
                rhtn_ffi::net::Event::Payload { from, bytes } => format!("payload from={} bytes={}", hex(&from), hex(&bytes)),
                rhtn_ffi::net::Event::ResponseCopy { from, query, refused } => {
                    format!("response-copy from={} query={} refused={}", hex(&from), query.map_or("-".into(), |q| hex(&q)), refused.unwrap_or_else(|| "-".into()))
                }
                rhtn_ffi::net::Event::Late { from, record, refused } => {
                    format!("late from={} record={} refused={}", hex(&from), record.map_or("-".into(), |r| hex(&r)), refused.unwrap_or_else(|| "-".into()))
                }
            });
            // one pass over what is already waiting: a second wait would
            // make `events` a blocking command with no way back out
            if ms > 0 {
                break;
            }
        }
        out.push("events done".into());
        Ok(out)
    }
}

fn id(s: &str) -> Result<Vec<u8>, String> {
    let b = unhex(s).ok_or_else(|| format!("`{s}` is not hex"))?;
    if b.len() != 32 {
        return Err(format!("`{s}` is {} bytes, and an identity is 32", b.len()));
    }
    Ok(b)
}

fn channel_of(s: &str) -> Result<Channel, String> {
    match s {
        "uwb" => Ok(Channel::Uwb),
        "nfc" => Ok(Channel::Nfc),
        "optical" => Ok(Channel::Optical),
        "latency" => Ok(Channel::Latency),
        _ => Err(format!("`{s}` is not uwb, nfc, optical or latency")),
    }
}

fn channel_name(c: Channel) -> &'static str {
    match c {
        Channel::Uwb => "uwb",
        Channel::Nfc => "nfc",
        Channel::Optical => "optical",
        Channel::Latency => "latency",
    }
}

fn outcome_of(s: &str) -> Result<ChannelOutcome, String> {
    match s {
        "pass" => Ok(ChannelOutcome::Pass),
        "fail" => Ok(ChannelOutcome::Fail),
        "unavailable" => Ok(ChannelOutcome::Unavailable),
        _ => Err(format!("`{s}` is not pass, fail or unavailable")),
    }
}

fn outcome_name(o: ChannelOutcome) -> &'static str {
    match o {
        ChannelOutcome::Pass => "pass",
        ChannelOutcome::Fail => "fail",
        ChannelOutcome::Unavailable => "unavailable",
    }
}

/// Read the seeds an identity is derived from.
pub fn read_identity(path: &Path) -> Result<Vec<u8>, String> {
    let bytes = std::fs::read(path).map_err(|e| format!("{}: {e}", path.display()))?;
    if bytes.len() != IDENTITY_BYTES {
        return Err(format!("{} is {} bytes, not {IDENTITY_BYTES}", path.display(), bytes.len()));
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = std::fs::metadata(path).map_err(|e| format!("{}: {e}", path.display()))?.permissions().mode();
        if mode & 0o077 != 0 {
            return Err(format!("{} is readable beyond its owner (mode {:o})", path.display(), mode & 0o777));
        }
    }
    Ok(bytes)
}

/// Read the parties this participant authenticates: one hex `KeyMaterial`
/// blob a line, in the shape the daemon's peers file has, because it is
/// the same fact about the same kind of party (`wire-format.md` §9.1).
pub fn read_peers(path: &Path) -> Result<Vec<Vec<u8>>, String> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let mut out = Vec::new();
    for (i, raw) in text.lines().enumerate() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        out.push(unhex(line).ok_or_else(|| format!("line {}: not hex", i + 1))?);
    }
    Ok(out)
}

pub const HELP: &str = "\
help                      these lines
me                        this participant's keyhash
attach <node> <addr>[,<addr>] [<party>...]
                          attach to a serving node, sweeping the parties named
attached                  whether a session is held
maintain                  rotate, replenish and ask for what is due
send <to> <kind> <hex>    payload, over the direct path or through the node
wake <url> <key> [<at>]   where to be rung; `wake off` withdraws it
events [<ms>]             what arrived; waits <ms> for the first
channel                   what the hardware has been declared to do
channel <name> <outcome> [<m>]
                          declare it; `none` clears one back to unavailable
answer [yes|no]           the standing answer to a question put to the person
quit                      stop";
