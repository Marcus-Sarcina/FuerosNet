//! The operator's configuration: what a node cannot derive and must be told.
//!
//! Every field is something the crates beneath need and no document
//! supplies — where the identity lives, what the node serves on, and the
//! bounds that are the operator's to choose (`infra-client-requirements.md`
//! §1).  Nothing here is protocol: a field that could be derived from the
//! topology would be a second source of truth for it.
//!
//! **The format is TOML** [author, 2026-09-14].  The line-oriented
//! placeholder this replaced could not repeat a key, which is why the
//! packages a node hosts had to live in a file of their own, and its
//! positional values had nowhere to put a name: `allowance`, `upstream`
//! and the resource limits were each a small parser over a string.  They
//! are tables now.
//!
//! **It is strict, for the same reason the HTTP boundary is.** An unknown
//! key, a repeated key, a missing one or a value that does not parse is an
//! error naming its line. An operator who misspells a key is told, rather
//! than served a default they did not choose — and TOML's own message
//! lists the keys it expected, which the placeholder's could not.
//!
//! **What is validated here is what a document fixes or a wire refuses**,
//! never what deserialising already settled: the heartbeat's range, an
//! allowance that would serve nobody, a keyhash's form, and an ingestion
//! boundary that must be named rather than guessed.  Each of those carries
//! the line it was written on, because the fields they check are read
//! through [`toml::Spanned`].

use rhtn_archive::Keyhash;
use rhtn_node::resolution::Ingestion;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use toml::Spanned;

/// A node's configuration as its operator writes it.
///
/// There is no `Default`: a default listen address or queue cap would be a
/// policy choice made by omission, and the operator's obligations are
/// stated as choices (`infra-client-requirements.md` §1).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Config {
    /// Where the node's signing identity is read from.  It is never
    /// generated on start: a node that mints a key when the file is
    /// missing serves under an identity nobody has adopted, and its
    /// operator would not know.
    pub identity: PathBuf,
    /// The address the node serves QUIC on and answers STUN Binding
    /// requests at (`infra-client-requirements.md` §7, design §14.1.1).
    pub listen: SocketAddr,
    /// The patron this node attaches to, and where to reach it.  Absent at
    /// a root, which attaches to nobody.
    pub upstream: Option<(Keyhash, Vec<SocketAddr>)>,
    /// Where the queue's directory store lives, so what was accepted
    /// survives a restart (`infra-client-requirements.md` §2).
    pub queue: PathBuf,
    /// The cap on one recipient's queue, above which the newest is refused
    /// and its sender told (`infra-client-requirements.md` §2, design
    /// §14.1.6).  Absent is uncapped.
    pub queue_cap: Option<usize>,
    /// The heartbeat interval this node advertises, in seconds
    /// (`wire-format.md` §8.2 fixes the range at 1 to 3600).
    pub heartbeat_secs: u64,
    /// How this node takes anchor entries it did not verify itself
    /// (`infra-client-requirements.md` §4.1).
    pub ingestion: Ingestion,
    /// How many requests one peer may make in a window, and how long the
    /// window is in seconds.  The values are the operator's.
    pub request_allowance: (u32, u64),
    /// Where the prekey service's pools and bundles are kept, so a restart
    /// does not reissue a one-time key it already served
    /// (`wire-format.md` §7.8).
    pub prekeys: PathBuf,
    /// Where the topology store is kept: it is this node's seen-set, and a
    /// node that forgets it replays a forwarding wave
    /// (`infra-client-requirements.md` §4.3).
    pub topology: PathBuf,
    /// Where this node's own archive is kept.
    ///
    /// **Not the topology store, and not derived from it** [author,
    /// 2026-09-13].  The store holds what this node accepted about others
    /// and its horizon bounds it; the archive is this key's own signed
    /// history from its first transaction, which no horizon bounds
    /// (design §10, `wire-format.md` §3.1).
    pub archive: PathBuf,
    /// Where the packages this node hosts are listed, and who may reach
    /// them (`infra-client-requirements.md` §9, §10).  Absent hosts
    /// nothing, which §9 makes a conforming state rather than a defect:
    /// **not meeting a package's requirements is ordinary capacity**, and
    /// declining to host at all is the same fact.
    pub resources: Option<PathBuf>,
    /// What one request to a hosted package may spend: a memory ceiling in
    /// bytes and an instruction budget.  The values are the operator's; no
    /// document fixes either.
    pub resource_limits: Option<(usize, u64)>,
}

/// Why a configuration was refused: the line it was on, and what was wrong
/// with it.  Line 0 means the file as a whole — and TOML's own errors
/// carry their position inside the message, so a shape error reads as one
/// whether or not this field is set.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Invalid {
    pub line: usize,
    pub what: String,
}

impl std::fmt::Display for Invalid {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.line {
            0 => write!(f, "{}", self.what),
            n => write!(f, "line {n}: {}", self.what),
        }
    }
}

impl std::error::Error for Invalid {}

fn at(line: usize, what: impl Into<String>) -> Invalid {
    Invalid { line, what: what.into() }
}

/// 32 bytes from 64 hex digits, lower case only: an upper-case or
/// short-form keyhash is a different string and is refused rather than
/// normalised.
fn keyhash(s: &str) -> Option<Keyhash> {
    if s.len() != 64 || !s.bytes().all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c)) {
        return None;
    }
    let mut out = [0u8; 32];
    for (i, b) in out.iter_mut().enumerate() {
        *b = u8::from_str_radix(&s[i * 2..i * 2 + 2], 16).ok()?;
    }
    Some(out)
}

/// The file's own shape, before anything is checked against a document.
///
/// Separate from [`Config`] because the two answer different questions:
/// this one is what TOML can settle by itself, and `Config` is what the
/// crates beneath will be handed.  The fields read through `Spanned` are
/// the ones with a rule over them, so a refusal can say where.
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct File {
    identity: PathBuf,
    listen: Spanned<String>,
    queue: PathBuf,
    prekeys: PathBuf,
    topology: PathBuf,
    archive: PathBuf,
    heartbeat: Spanned<u64>,
    ingestion: Spanned<String>,
    allowance: Spanned<Allowance>,
    #[serde(rename = "queue-cap")]
    queue_cap: Option<usize>,
    upstream: Option<Spanned<Upstream>>,
    resources: Option<PathBuf>,
    #[serde(rename = "resource-limits")]
    resource_limits: Option<Spanned<Limits>>,
}

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct Allowance {
    requests: u32,
    seconds: u64,
}

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct Upstream {
    node: String,
    addresses: Vec<String>,
}

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct Limits {
    memory: usize,
    fuel: u64,
}

impl Config {
    /// Read a configuration from `path`.
    pub fn read(path: &Path) -> Result<Config, Invalid> {
        let text = std::fs::read_to_string(path).map_err(|e| at(0, format!("{}: {e}", path.display())))?;
        Config::parse(&text)
    }

    /// Parse a configuration.
    ///
    /// Deserialising settles the shape; what follows are the rules a
    /// document states and TOML cannot know.
    pub fn parse(text: &str) -> Result<Config, Invalid> {
        let f: File = toml::from_str(text).map_err(|e| shape(text, &e))?;

        let listen: SocketAddr = f.listen.get_ref().parse().map_err(|_| at(line_at(text, &f.listen), "`listen` is not an address and port"))?;

        let heartbeat_secs = *f.heartbeat.get_ref();
        if !(1..=3600).contains(&heartbeat_secs) {
            return Err(at(line_at(text, &f.heartbeat), "`heartbeat` is 1 to 3600 seconds (`wire-format.md` §8.2)"));
        }

        let ingestion = match f.ingestion.get_ref().as_str() {
            "verified-on-acceptance" => Ingestion::VerifiedOnAcceptance,
            "unverified-gossip" => Ingestion::UnverifiedGossip,
            _ => return Err(at(line_at(text, &f.ingestion), "`ingestion` is `verified-on-acceptance` or `unverified-gossip`")),
        };

        let a = f.allowance.get_ref();
        if a.requests == 0 || a.seconds == 0 {
            return Err(at(line_at(text, &f.allowance), "an `allowance` of zero admits nothing and serves nobody"));
        }
        let request_allowance = (a.requests, a.seconds);

        let upstream = match &f.upstream {
            None => None,
            Some(u) => {
                let n = line_at(text, u);
                let key = keyhash(&u.get_ref().node).ok_or_else(|| at(n, "`upstream.node` is not 64 lower-case hex digits"))?;
                let addrs: Result<Vec<SocketAddr>, Invalid> =
                    u.get_ref().addresses.iter().map(|a| a.parse::<SocketAddr>().map_err(|_| at(n, format!("`{a}` is not an address and port")))).collect();
                let addrs = addrs?;
                if addrs.is_empty() {
                    return Err(at(n, "`upstream` names no address"));
                }
                Some((key, addrs))
            }
        };

        let resource_limits = match &f.resource_limits {
            None => None,
            Some(l) => {
                let (memory, fuel) = (l.get_ref().memory, l.get_ref().fuel);
                if memory == 0 || fuel == 0 {
                    return Err(at(line_at(text, l), "a `resource-limits` of zero admits a package and then runs none of it"));
                }
                Some((memory, fuel))
            }
        };

        Ok(Config {
            identity: f.identity,
            listen,
            upstream,
            queue: f.queue,
            queue_cap: f.queue_cap,
            heartbeat_secs,
            ingestion,
            request_allowance,
            prekeys: f.prekeys,
            topology: f.topology,
            archive: f.archive,
            resources: f.resources,
            resource_limits,
        })
    }
}

/// Turn a deserialiser's complaint into this module's.
///
/// **Only the missing-field wording is rewritten.** Serde says "missing
/// field", which is true and says nothing about why there is no default;
/// the rest of what TOML reports — the key it did not expect, the ones it
/// did, the duplicate, the line and column — is better than anything this
/// module wrote by hand, and is passed through.
fn shape(text: &str, e: &toml::de::Error) -> Invalid {
    let m = e.message();
    let line = e.span().map_or(0, |s| line_of(text, s.start));
    if let Some(rest) = m.strip_prefix("missing field ") {
        return at(line, format!("{rest} is not set, and has no default"));
    }
    at(line, e.to_string())
}

/// The line a spanned value starts on.
fn line_at<T>(text: &str, s: &Spanned<T>) -> usize {
    line_of(text, s.span().start)
}

/// The 1-based line a byte offset falls on.
fn line_of(text: &str, at: usize) -> usize {
    text[..at.min(text.len())].bytes().filter(|b| *b == b'\n').count() + 1
}
