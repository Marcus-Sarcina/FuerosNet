//! The operator's configuration: what a node cannot derive and must be told.
//!
//! Every field is something the crates beneath need and no document
//! supplies — where the identity lives, what the node serves on, and the
//! bounds that are the operator's to choose (`infra-client-requirements.md`
//! §1).  Nothing here is protocol: a field that could be derived from the
//! topology would be a second source of truth for it.
//!
//! **The format fixes no dependency**, which is the point: the plan lists
//! the choice of one as the author's, so the placeholder is a line-oriented
//! `key = value` file parsed here in a few dozen lines.  Swapping in TOML
//! or JSON later replaces [`Config::parse`] and nothing else.
//!
//! **It is strict, for the same reason the HTTP boundary is.** An unknown
//! key, a repeated key, a missing one or a value that does not parse is an
//! error naming its line. An operator who misspells a key is told, rather
//! than served a default they did not choose.

use rhtn_archive::Keyhash;
use rhtn_node::resolution::Ingestion;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};

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
}

/// Why a configuration was refused: the line it was on, and what was wrong
/// with it.  Line 0 means the file as a whole.
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

impl Config {
    /// Read a configuration from `path`.
    pub fn read(path: &Path) -> Result<Config, Invalid> {
        let text = std::fs::read_to_string(path).map_err(|e| at(0, format!("{}: {e}", path.display())))?;
        Config::parse(&text)
    }

    /// Parse a configuration.  Blank lines and lines whose first
    /// non-blank character is `#` are ignored; every other line is
    /// `key = value`.
    pub fn parse(text: &str) -> Result<Config, Invalid> {
        let mut seen: Vec<(String, String, usize)> = Vec::new();
        for (i, raw) in text.lines().enumerate() {
            let n = i + 1;
            let line = raw.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let Some((k, v)) = line.split_once('=') else {
                return Err(at(n, "not `key = value`"));
            };
            let (k, v) = (k.trim().to_string(), v.trim().to_string());
            if k.is_empty() {
                return Err(at(n, "empty key"));
            }
            if let Some((_, _, first)) = seen.iter().find(|(x, _, _)| *x == k) {
                return Err(at(n, format!("`{k}` was already set on line {first}")));
            }
            seen.push((k, v, n));
        }
        let take = |key: &str| seen.iter().find(|(k, _, _)| k == key).map(|(_, v, n)| (v.as_str(), *n));
        let need = |key: &str| take(key).ok_or_else(|| at(0, format!("`{key}` is not set, and has no default")));

        // an unknown key is a refusal, not something to ignore: a
        // misspelled one would otherwise take a value the operator meant
        // to set
        const KEYS: [&str; 10] =
            ["identity", "listen", "upstream", "queue", "queue-cap", "heartbeat", "ingestion", "allowance", "prekeys", "topology"];
        if let Some((k, _, n)) = seen.iter().find(|(k, _, _)| !KEYS.contains(&k.as_str())) {
            return Err(at(*n, format!("`{k}` is not a configuration key")));
        }

        let (identity, _) = need("identity")?;
        let (listen, ln) = need("listen")?;
        let listen: SocketAddr = listen.parse().map_err(|_| at(ln, "`listen` is not an address and port"))?;
        let (queue, _) = need("queue")?;
        let (prekeys, _) = need("prekeys")?;
        let (topology, _) = need("topology")?;

        let upstream = match take("upstream") {
            None => None,
            Some((v, n)) => {
                let (key, addrs) = v.split_once(char::is_whitespace).ok_or_else(|| at(n, "`upstream` is a keyhash then its addresses"))?;
                let key = keyhash(key.trim()).ok_or_else(|| at(n, "`upstream` keyhash is not 64 lower-case hex digits"))?;
                let addrs: Result<Vec<SocketAddr>, Invalid> = addrs
                    .split(',')
                    .map(|a| a.trim().parse::<SocketAddr>().map_err(|_| at(n, format!("`{}` is not an address and port", a.trim()))))
                    .collect();
                let addrs = addrs?;
                if addrs.is_empty() {
                    return Err(at(n, "`upstream` names no address"));
                }
                Some((key, addrs))
            }
        };

        let queue_cap = match take("queue-cap") {
            None => None,
            Some((v, n)) => Some(v.parse::<usize>().map_err(|_| at(n, "`queue-cap` is not a count"))?),
        };

        let (heartbeat_secs, hn) = need("heartbeat")?;
        let heartbeat_secs: u64 = heartbeat_secs.parse().map_err(|_| at(hn, "`heartbeat` is not a count of seconds"))?;
        if !(1..=3600).contains(&heartbeat_secs) {
            return Err(at(hn, "`heartbeat` is 1 to 3600 seconds (`wire-format.md` §8.2)"));
        }

        let (ingestion, in_) = need("ingestion")?;
        let ingestion = match ingestion {
            "verified-on-acceptance" => Ingestion::VerifiedOnAcceptance,
            "unverified-gossip" => Ingestion::UnverifiedGossip,
            _ => return Err(at(in_, "`ingestion` is `verified-on-acceptance` or `unverified-gossip`")),
        };

        let (allowance, an) = need("allowance")?;
        let (count, window) = allowance.split_once('/').ok_or_else(|| at(an, "`allowance` is `requests/seconds`"))?;
        let count: u32 = count.trim().parse().map_err(|_| at(an, "`allowance` request count is not a number"))?;
        let window: u64 = window.trim().parse().map_err(|_| at(an, "`allowance` window is not a count of seconds"))?;
        if count == 0 || window == 0 {
            return Err(at(an, "`allowance` of zero admits nothing and serves nobody"));
        }

        Ok(Config {
            identity: PathBuf::from(identity),
            listen,
            upstream,
            queue: PathBuf::from(queue),
            queue_cap,
            heartbeat_secs,
            ingestion,
            request_allowance: (count, window),
            prekeys: PathBuf::from(prekeys),
            topology: PathBuf::from(topology),
        })
    }
}
