//! The daemon's lifecycle: start a node from a configuration, serve until
//! signalled, and stop without losing what was accepted.
//!
//! Everything a peer observes is decided in `rhtn-node`, `rhtn-transport`
//! and the crates beneath them. What is decided here is only when those
//! crates are handed their state and when they are asked to write it back.

use crate::config::Config;
use rhtn_archive::Keyhash;
use rhtn_crypto::{Identity, SigningIdentity};
use rhtn_node::prekeys::{PrekeyConfig, PrekeyService};
use rhtn_node::resolution::AnchorTable;
use rhtn_node::runtime::{LiveNode, RateLimit};
use rhtn_node::store::TopologyStore;
use rhtn_node::view::NodeView;
use rhtn_transport::session::NodeConfig;
use rhtn_transport::tls::Pins;
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

/// Why a daemon would not start.  Every one of these is reported and none
/// is worked around: a node that repairs its own configuration serves
/// something its operator did not write.
#[derive(Debug)]
pub enum Startup {
    /// The identity file is absent, the wrong length, or readable by
    /// somebody other than its owner.
    Identity(String),
    /// A peer this node must authenticate could not be pinned.
    Peers(String),
    /// State that must survive a restart could not be read.
    State(String),
}

impl std::fmt::Display for Startup {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Startup::Identity(s) => write!(f, "identity: {s}"),
            Startup::Peers(s) => write!(f, "peers: {s}"),
            Startup::State(s) => write!(f, "state: {s}"),
        }
    }
}

impl std::error::Error for Startup {}

/// The two seeds an identity is derived from, in order: the classical one
/// and the post-quantum one (design §5.1).  Sixty-four bytes, and a file
/// of any other length is refused rather than padded or truncated.
pub const IDENTITY_BYTES: usize = 64;

/// Read the node's identity.
///
/// **It is never generated here.** A node that mints a key when its file
/// is missing serves under an identity nobody has adopted, its subordinates
/// cannot reach it, and its operator is not told any of that.
///
/// On Unix the file must not be readable by group or other. A signing key
/// the rest of the host can read is not this node's alone, and the check
/// costs nothing.
pub fn read_identity(path: &Path) -> Result<SigningIdentity, Startup> {
    let bytes = std::fs::read(path).map_err(|e| Startup::Identity(format!("{}: {e}", path.display())))?;
    if bytes.len() != IDENTITY_BYTES {
        return Err(Startup::Identity(format!("{} is {} bytes, not {IDENTITY_BYTES}", path.display(), bytes.len())));
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = std::fs::metadata(path).map_err(|e| Startup::Identity(format!("{}: {e}", path.display())))?.permissions().mode();
        if mode & 0o077 != 0 {
            return Err(Startup::Identity(format!("{} is readable beyond its owner (mode {:o})", path.display(), mode & 0o777)));
        }
    }
    let (ed, pq): ([u8; 32], [u8; 32]) = (bytes[..32].try_into().unwrap(), bytes[32..].try_into().unwrap());
    Ok(SigningIdentity::from_seeds(&ed, &pq))
}

/// Read the peers this node authenticates: one hex `KeyMaterial` blob per
/// line, blank lines and `#` comments ignored.
///
/// **A keyhash alone cannot be pinned.** The transport authenticates a
/// peer by its raw public key (`wire-format.md` §9.1), so what a node must
/// hold is the key material itself; the keyhash is derived from it and
/// checked against it. There is no fetch path for material a node lacks,
/// which is why it is configuration and not discovery.
pub fn read_peers(path: &Path) -> Result<Vec<Identity>, Startup> {
    let text = std::fs::read_to_string(path).map_err(|e| Startup::Peers(format!("{}: {e}", path.display())))?;
    let mut out = Vec::new();
    for (i, raw) in text.lines().enumerate() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let bytes = hex_bytes(line).ok_or_else(|| Startup::Peers(format!("line {}: not hex", i + 1)))?;
        let id = Identity::from_key_material(&bytes).ok_or_else(|| Startup::Peers(format!("line {}: not a KeyMaterial array", i + 1)))?;
        out.push(id);
    }
    Ok(out)
}

fn hex_bytes(s: &str) -> Option<Vec<u8>> {
    if !s.len().is_multiple_of(2) {
        return None;
    }
    (0..s.len() / 2).map(|i| u8::from_str_radix(&s[i * 2..i * 2 + 2], 16).ok()).collect()
}

/// A running daemon: the node, and the state it must write back.
pub struct Service {
    pub node: Arc<LiveNode>,
    /// Where the prekey pools and the topology store are kept.  Held so
    /// shutdown writes them to the same places startup read them from.
    prekeys: std::path::PathBuf,
    topology: std::path::PathBuf,
    /// The upstream session, held for as long as the daemon runs: dropping
    /// it ends the attachment.
    _upstream: Option<rhtn_transport::session::Session>,
}

impl Service {
    /// Start a node from `cfg`.
    ///
    /// The order is not arbitrary. Consumable state is read before the
    /// first session can be accepted, because a node that serves before it
    /// has loaded its one-time pools reissues keys it already served
    /// (`wire-format.md` §7.8), and one that serves before it has loaded
    /// its topology store replays a forwarding wave into every cycle in
    /// its horizon (`infra-client-requirements.md` §4.3).
    pub async fn start(cfg: &Config, peers: &Path) -> Result<Service, Startup> {
        let me = Arc::new(read_identity(&cfg.identity)?);
        let known = read_peers(peers)?;
        let pins = Pins::new();
        pins.pin_identity(&me.public);
        for id in &known {
            pins.pin_identity(id);
        }
        // consumable state first, and its absence is a first start rather
        // than a failure: a directory that is not there yet is empty
        let prekeys = match PrekeyService::load(&cfg.prekeys, PrekeyConfig::default()) {
            Ok(p) => p,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => PrekeyService::new(PrekeyConfig::default()),
            Err(e) => return Err(Startup::State(format!("{}: {e}", cfg.prekeys.display()))),
        };
        let store = match TopologyStore::load(&cfg.topology) {
            Ok(s) => s,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => TopologyStore::new(),
            Err(e) => return Err(Startup::State(format!("{}: {e}", cfg.topology.display()))),
        };
        std::fs::create_dir_all(&cfg.queue).map_err(|e| Startup::State(format!("{}: {e}", cfg.queue.display())))?;
        let queue = Arc::new(rhtn_node::queue::DirStore::new(&cfg.queue));

        let mut view = NodeView::new(me.clone(), position_of(&me.public.keyhash));
        view.store = store;
        view.prekeys = prekeys;
        if let Some((patron, _)) = &cfg.upstream {
            view.serving_node = Some(*patron);
        }

        let mut node_cfg = NodeConfig::defaults(me.clone(), pins.clone(), cfg.heartbeat_secs);
        node_cfg.listen = Some(cfg.listen);
        node_cfg.queue = queue;
        node_cfg.queue_cap = cfg.queue_cap;
        // the wall clock, so issuance, outage stamps and the ladder's
        // intervals read time that moves
        node_cfg.clock = Arc::new(|| std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0));
        let (count, window) = cfg.request_allowance;
        let limits = RateLimit::new(count, Duration::from_secs(window));
        let anchors = AnchorTable::new(0, cfg.ingestion);
        let node = LiveNode::start_with(node_cfg, view, known.clone(), anchors, limits);

        // upstream, where the configuration names one: a root attaches to
        // nobody (design §14.1.2)
        let upstream = match &cfg.upstream {
            None => None,
            Some((patron, addrs)) => {
                let ccfg = client_config(me.clone(), pins, addrs, patron);
                match node.attach_upstream(&ccfg, *patron).await {
                    rhtn_transport::session::AttachOutcome::Attached(s) => Some(s),
                    // an upstream that will not have us is reported and not
                    // fatal: the node still serves what it holds, and a
                    // later attach is the operator's to make
                    other => {
                        eprintln!("rhtnd: upstream {}: {other:?}", hex8(patron));
                        None
                    }
                }
            }
        };
        Ok(Service { node, prekeys: cfg.prekeys.clone(), topology: cfg.topology.clone(), _upstream: upstream })
    }

    /// Routine maintenance: one-time keys whose window has passed are
    /// dropped (`wire-format.md` §7.8).
    ///
    /// **The subjects whose pools ran dry are drained and not delivered.**
    /// `infra-client-requirements.md` §6 obliges a node to tell them, and
    /// no wire object carries it. Until one is specified the daemon can
    /// only take them off the list, which is recorded rather than hidden.
    pub fn maintain(&self) -> Vec<Keyhash> {
        let mut view = self.node.view.lock().unwrap();
        let now = view.now();
        view.prekeys.expire(now);
        view.prekeys.take_exhausted()
    }

    /// Write back what a restart must find.  Called on the way out, and
    /// safe to call more than once.
    pub fn persist(&self) -> Result<(), std::io::Error> {
        let view = self.node.view.lock().unwrap();
        view.prekeys.save(&self.prekeys)?;
        view.store.save(&self.topology)
    }

    /// Serve until signalled, then stop.
    ///
    /// SIGINT and SIGTERM both mean stop. What is in flight finishes on
    /// its own: a delivery half made leaves its message in the store,
    /// which the store contract already guarantees, so the shutdown's only
    /// duty is to write back before the process ends.
    pub async fn run(self) -> Result<(), std::io::Error> {
        let mut term = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())?;
        let mut tick = tokio::time::interval(Duration::from_secs(60));
        tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        loop {
            tokio::select! {
                _ = tokio::signal::ctrl_c() => break,
                _ = term.recv() => break,
                _ = tick.tick() => {
                    for dry in self.maintain() {
                        eprintln!("rhtnd: one-time pool exhausted for {}", hex8(&dry));
                    }
                    // the store is written on the tick as well as on the
                    // way out, so a kill that never reaches the handler
                    // loses at most one interval
                    self.persist()?;
                }
            }
        }
        self.persist()
    }
}

/// A node with no adoption yet is its own anchor at the empty path: the
/// position a root holds, and what a subordinate carries until its
/// adoption is stored (`wire-format.md` §2.3).
fn position_of(me: &Keyhash) -> rhtn_archive::tx::Locator {
    rhtn_archive::tx::Locator { anchor: *me, path: Vec::new(), nibbles: 0, seqno: rhtn_archive::tx::Seqno { series: 1, counter: 0 } }
}

fn client_config(
    me: Arc<SigningIdentity>,
    pins: Pins,
    addrs: &[std::net::SocketAddr],
    patron: &Keyhash,
) -> rhtn_transport::session::ClientConfig {
    let book: std::collections::HashMap<[u8; 32], Vec<std::net::SocketAddr>> = std::collections::HashMap::from([(*patron, addrs.to_vec())]);
    rhtn_transport::session::ClientConfig {
        identity: me,
        pins,
        capabilities: Default::default(),
        attestation: None,
        filter: None,
        sibling_cache: Arc::new(std::sync::Mutex::new(Vec::new())),
        addresses: Arc::new(std::sync::Mutex::new(book)),
        tls: Arc::new(std::sync::Mutex::new(Default::default())),
        connect_timeout: Duration::from_secs(5),
        on_reachability: None,
        log: Default::default(),
    }
}

fn hex8(k: &Keyhash) -> String {
    k[..4].iter().map(|b| format!("{b:02x}")).collect()
}
