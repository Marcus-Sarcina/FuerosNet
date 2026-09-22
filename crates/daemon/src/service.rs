//! The daemon's lifecycle: start a node from a configuration, serve until
//! signalled, and stop without losing what was accepted.
//!
//! Everything a peer observes is decided in `rhtn-node`, `rhtn-transport`
//! and the crates beneath them. What is decided here is only when those
//! crates are handed their state and when they are asked to write it back.

use crate::config::Config;
use rhtn_archive::Keyhash;
use rhtn_archive::topology::{Restored, Snapshot};
use rhtn_crypto::{Identity, SigningIdentity};
use rhtn_node::prekeys::{PrekeyConfig, PrekeyService};
use rhtn_node::resolution::AnchorTable;
use rhtn_node::runtime::{LiveNode, RateLimit};
use rhtn_node::store::TopologyStore;
use rhtn_node::view::NodeView;
use rhtn_resources::Limits;
use rhtn_transport::session::NodeConfig;
use rhtn_transport::tls::Pins;
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

/// The derived view, kept in the topology directory beside the records it
/// is a fold of.  A store copied without it simply replays.
const DERIVED: &str = "derived";

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
    /// A package the configuration named could not be hosted.
    Hosting(String),
}

impl std::fmt::Display for Startup {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Startup::Identity(s) => write!(f, "identity: {s}"),
            Startup::Peers(s) => write!(f, "peers: {s}"),
            Startup::State(s) => write!(f, "state: {s}"),
            Startup::Hosting(s) => write!(f, "hosting: {s}"),
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
    let bytes =
        std::fs::read(path).map_err(|e| Startup::Identity(format!("{}: {e}", path.display())))?;
    if bytes.len() != IDENTITY_BYTES {
        return Err(Startup::Identity(format!(
            "{} is {} bytes, not {IDENTITY_BYTES}",
            path.display(),
            bytes.len()
        )));
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = std::fs::metadata(path)
            .map_err(|e| Startup::Identity(format!("{}: {e}", path.display())))?
            .permissions()
            .mode();
        if mode & 0o077 != 0 {
            return Err(Startup::Identity(format!(
                "{} is readable beyond its owner (mode {:o})",
                path.display(),
                mode & 0o777
            )));
        }
    }
    let (ed, pq): ([u8; 32], [u8; 32]) = (
        bytes[..32].try_into().unwrap(),
        bytes[32..].try_into().unwrap(),
    );
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
    let text = std::fs::read_to_string(path)
        .map_err(|e| Startup::Peers(format!("{}: {e}", path.display())))?;
    let mut out = Vec::new();
    for (i, raw) in text.lines().enumerate() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let bytes =
            hex_bytes(line).ok_or_else(|| Startup::Peers(format!("line {}: not hex", i + 1)))?;
        let id = Identity::from_key_material(&bytes)
            .ok_or_else(|| Startup::Peers(format!("line {}: not a KeyMaterial array", i + 1)))?;
        out.push(id);
    }
    Ok(out)
}

fn hex_bytes(s: &str) -> Option<Vec<u8>> {
    if !s.len().is_multiple_of(2) {
        return None;
    }
    (0..s.len() / 2)
        .map(|i| u8::from_str_radix(&s[i * 2..i * 2 + 2], 16).ok())
        .collect()
}

/// A running daemon: the node, and the state it must write back.
pub struct Service {
    pub node: Arc<LiveNode>,
    /// Where the prekey pools and the topology store are kept.  Held so
    /// shutdown writes them to the same places startup read them from.
    prekeys: std::path::PathBuf,
    topology: std::path::PathBuf,
    archive: std::path::PathBuf,
    /// Whether this node runs a backend of its own for any resource, which
    /// is what an operator is told (`infra-client-requirements.md` §10.6).
    /// True when the hosting file bound at least one package.
    hosts_resources: bool,
    /// The upstream session, held for as long as the daemon runs: dropping
    /// it ends the attachment.
    _upstream: Option<rhtn_transport::session::Session>,
    /// The credential an instance runs under, and where its run is read
    /// from; none for a node holding its seed.
    credential: Option<(Arc<rhtn_transport::tls::Credential>, std::path::PathBuf)>,
    /// The count last noticed to the operator, so a notice is raised once
    /// per count.
    noticed: std::sync::Mutex<Option<usize>>,
}

/// The operator is told while this many credentials or fewer remain in
/// the run: two weeks of 48-hour credentials, well before the last one
/// (`infra-client-requirements.md` §7).  A default, not a rule.
pub const NOTICE_AT_CREDENTIALS: usize = 7;

/// How often an instance waiting for its first credential looks again.
const PROVISIONING_POLL: Duration = Duration::from_secs(1);

/// The operator's `KeyMaterial` from a hex file: the identity an instance
/// speaks as.
pub fn read_operator(path: &Path) -> Result<Identity, Startup> {
    let text = std::fs::read_to_string(path)
        .map_err(|e| Startup::Identity(format!("{}: {e}", path.display())))?;
    let bytes = hex_bytes(text.trim())
        .ok_or_else(|| Startup::Identity(format!("{}: not hex", path.display())))?;
    Identity::from_key_material(&bytes).ok_or_else(|| {
        Startup::Identity(format!(
            "{}: not a KeyMaterial (`wire-format.md` §2.2)",
            path.display()
        ))
    })
}

/// The instance's transport keypair: read from `path`, or minted there
/// when absent, with the public half written beside it as `.pub` for the
/// operator's client to sign over.  Only that half leaves the instance
/// (`infra-client-requirements.md` §7) [author, 2026-09-21].
pub fn transport_credential(
    path: &Path,
    operator: &Identity,
) -> Result<Arc<rhtn_transport::tls::Credential>, Startup> {
    use rhtn_transport::tls::Credential;
    let cred = match std::fs::read(path) {
        Ok(bytes) => {
            let seed: [u8; 32] = bytes.as_slice().try_into().map_err(|_| {
                Startup::Identity(format!("{}: a transport key is 32 bytes", path.display()))
            })?;
            Credential::from_seed(&seed, operator.keyhash)
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            let seed = rhtn_transport::tls::random_bytes::<32>();
            std::fs::write(path, seed)
                .map_err(|e| Startup::Identity(format!("{}: {e}", path.display())))?;
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let _ = std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600));
            }
            Credential::from_seed(&seed, operator.keyhash)
        }
        Err(e) => return Err(Startup::Identity(format!("{}: {e}", path.display()))),
    };
    let public: String = cred.public().iter().map(|b| format!("{b:02x}")).collect();
    let pub_path = path.with_extension("pub");
    std::fs::write(&pub_path, format!("{public}\n"))
        .map_err(|e| Startup::Identity(format!("{}: {e}", pub_path.display())))?;
    Ok(Arc::new(cred))
}

/// Take every credential in `dir` into the run, verified under the
/// operator; one that is not the operator's, not over this key, or not a
/// delegation is reported and left.  Returns how many the run holds.
pub fn read_run(
    dir: &Path,
    cred: &rhtn_transport::tls::Credential,
    operator: &Identity,
) -> Result<usize, Startup> {
    std::fs::create_dir_all(dir).map_err(|e| Startup::State(format!("{}: {e}", dir.display())))?;
    let mut names: Vec<_> = std::fs::read_dir(dir)
        .map_err(|e| Startup::State(format!("{}: {e}", dir.display())))?
        .flatten()
        .map(|e| e.path())
        .collect();
    names.sort();
    for p in names {
        let Ok(bytes) = std::fs::read(&p) else {
            continue;
        };
        if let Err(e) = cred.add(std::slice::from_ref(operator), &bytes) {
            eprintln!("rhtnd: {}: not taken into the run: {e}", p.display());
        }
    }
    Ok(cred.issued().len())
}

/// How many credentials of the run are still ahead of `now`, the one in
/// force included.
fn credentials_remaining(cred: &rhtn_transport::tls::Credential, now: u64) -> usize {
    cred.issued().iter().filter(|i| i.not_after > now).count()
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
        // the seed, or the credential an instance holds instead of it
        // (design §23.3)
        let (public, signer, credential) = match (&cfg.identity, &cfg.operator) {
            (Some(path), _) => {
                let me = Arc::new(read_identity(path)?);
                (me.public.clone(), Some(me), None)
            }
            (None, Some(op)) => {
                let operator = read_operator(op)?;
                let key = cfg
                    .transport_key
                    .as_ref()
                    .ok_or_else(|| Startup::Identity("an instance names `transport-key`".into()))?;
                let dir = cfg
                    .delegations
                    .as_ref()
                    .ok_or_else(|| Startup::Identity("an instance names `delegations`".into()))?;
                let cred = transport_credential(key, &operator)?;
                // provisioning: the public half is out, and the run arrives
                // when the operator's client has signed it.  An instance
                // serves nothing before a credential is in force, and one
                // whose run has lapsed is the same case
                let public_hex: String = cred.public().iter().map(|b| format!("{b:02x}")).collect();
                let mut said = false;
                loop {
                    read_run(dir, &cred, &operator)?;
                    if cred.remaining().is_some_and(|r| r > 0) {
                        break;
                    }
                    if !said {
                        eprintln!(
                            "rhtnd: transport key {public_hex}; no credential in force: waiting for the run in {}",
                            dir.display()
                        );
                        said = true;
                    }
                    tokio::time::sleep(PROVISIONING_POLL).await;
                }
                (operator, None, Some((cred, dir.clone())))
            }
            (None, None) => {
                return Err(Startup::Identity(
                    "neither a seed nor an operator to run delegated for".into(),
                ));
            }
        };
        // This node's own public key is in the lookup, and not because the
        // peers file listed it: a node countersigns adoptions of its own
        // subordinates, and one that cannot verify its own signature holds
        // those records unverifiable for want of a key it is holding
        // (`wire-format.md` §3.4).  Asking an operator to list themselves
        // would make a working configuration depend on remembering to.
        let mut known = vec![public.clone()];
        known.extend(read_peers(peers)?);
        let pins = Pins::new();
        pins.pin_identity(&public);
        for id in &known {
            pins.pin_identity(id);
        }
        let me_keyhash = public.keyhash;
        let party = match &credential {
            Some((c, _)) => rhtn_transport::tls::Party::of(c.clone()),
            None => rhtn_transport::tls::Party::of(signer.clone().expect("a seed or a credential")),
        };
        // consumable state first, and its absence is a first start rather
        // than a failure: a directory that is not there yet is empty
        // kept at its directory rather than snapshotted into it: a
        // one-time key is spent on disk before its reply goes out, so a
        // stop between snapshots cannot bring a served key back
        let prekeys = PrekeyService::at(&cfg.prekeys, PrekeyConfig::default())
            .map_err(|e| Startup::State(format!("{}: {e}", cfg.prekeys.display())))?;
        let store = match TopologyStore::load(&cfg.topology) {
            Ok(s) => s,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => TopologyStore::new(),
            Err(e) => return Err(Startup::State(format!("{}: {e}", cfg.topology.display()))),
        };
        std::fs::create_dir_all(&cfg.queue)
            .map_err(|e| Startup::State(format!("{}: {e}", cfg.queue.display())))?;
        let queue = Arc::new(rhtn_node::queue::DirStore::new(&cfg.queue));

        let archive = rhtn_archive::chain::Archive::load(&cfg.archive, me_keyhash)
            .map_err(|e| Startup::State(format!("{}: {e}", cfg.archive.display())))?;

        let mut view = match (&signer, &credential) {
            (Some(me), _) => NodeView::new(me.clone(), position_of(&me_keyhash)),
            (None, Some((c, _))) => {
                NodeView::delegated(public.clone(), c.clone(), position_of(&me_keyhash))
            }
            (None, None) => unreachable!("settled above"),
        };
        view.store = store;
        // the standing acknowledgement policy: every adoption under one of
        // this node's subordinates, or none (`infra-client-requirements.md`
        // §10.1)
        if cfg.acknowledge {
            view.ack_policy = Some(Arc::new(|_, _| true));
        }
        view.prekeys = prekeys;
        // this key's own signed history, kept apart from the seen-set: the
        // horizon bounds one and nothing bounds the other [author,
        // 2026-09-13]
        view.archive = archive;
        // the store holds records; the table, the slots and this node's own
        // position are derived from them, and a restart that loaded one
        // without the others would hold relationships it could not route on.
        // The derived state is written out beside the store, so a wake
        // folds in what arrived since rather than replaying the history
        // [author, 2026-09-13]; a snapshot that cannot account for what the
        // store holds is discarded and the whole fold runs
        let snap = std::fs::read(cfg.topology.join(DERIVED))
            .ok()
            .and_then(|b| Snapshot::decode(&b));
        match view.restore_materialised(snap.as_ref(), &known) {
            Restored::Replayed { replayed } if snap.is_some() => {
                eprintln!(
                    "rhtnd: the derived state did not match the store; replayed {replayed} records"
                );
            }
            _ => {}
        }
        if let Some((patron, _)) = &cfg.upstream {
            view.serving_node = Some(*patron);
        }

        // the packages this node was told to host, admitted before it
        // serves: a node that binds a resource it cannot run answers a
        // request with an unavailable it could have refused at start
        let hosts_resources = match &cfg.resources {
            None => false,
            Some(path) => {
                let (memory, fuel) = cfg
                    .resource_limits
                    .unwrap_or((Limits::default().memory, Limits::default().fuel));
                let limits = Limits {
                    memory,
                    fuel,
                    ..Limits::default()
                };
                let bound = crate::hosting::apply(&mut view.resources, path, limits)
                    .map_err(|e| Startup::Hosting(format!("{}: {e}", path.display())))?;
                bound > 0
            }
        };

        let mut node_cfg =
            NodeConfig::defaults_for(party.clone(), pins.clone(), cfg.heartbeat_secs);
        node_cfg.listen = Some(cfg.listen);
        node_cfg.queue = queue;
        node_cfg.queue_cap = cfg.queue_cap;
        // the wall clock, so issuance, outage stamps and the ladder's
        // intervals read time that moves
        node_cfg.clock = Arc::new(|| {
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0)
        });
        let (count, window) = cfg.request_allowance;
        let limits = RateLimit::new(count, Duration::from_secs(window));
        let mut anchors = AnchorTable::new(0, cfg.ingestion);
        // the operator-signed records an instance serves in place of any
        // it would mint (`infra-client-requirements.md` §4.4): each must
        // verify under the operator's identity, and neither ever under the
        // delegated key
        let given_record = match &cfg.endpoint_record {
            None => None,
            Some(p) => {
                let bytes = std::fs::read(p)
                    .map_err(|e| Startup::State(format!("{}: {e}", p.display())))?;
                let er = rhtn_node::store::EndpointRecord::parse(&bytes)
                    .map_err(|e| Startup::State(format!("{}: {e}", p.display())))?;
                if er.node != me_keyhash || er.signature_checks(&known) != Some(true) {
                    return Err(Startup::State(format!(
                        "{}: not this node's endpoint record signed by its operator",
                        p.display()
                    )));
                }
                // the address the record names against the one served on
                let named: Vec<std::net::SocketAddr> = er
                    .endpoints
                    .iter()
                    .filter_map(|b| rhtn_node::resolution::NetworkPoint::decode_bytes(b).ok())
                    .map(|np| np.socket())
                    .collect();
                if !named.contains(&cfg.listen) {
                    eprintln!(
                        "rhtnd: the listen address {} is not one the endpoint record names; the address has moved and a new operator-signed record is owed",
                        cfg.listen
                    );
                }
                Some(bytes)
            }
        };
        if let Some(p) = &cfg.anchor_entry {
            let bytes =
                std::fs::read(p).map_err(|e| Startup::State(format!("{}: {e}", p.display())))?;
            let entry = rhtn_node::resolution::AnchorEntry::parse(&bytes)
                .map_err(|e| Startup::State(format!("{}: {e}", p.display())))?;
            if entry.anchor != me_keyhash || !anchors.offer(entry, &known) {
                return Err(Startup::State(format!(
                    "{}: not this node's anchor entry signed by its operator",
                    p.display()
                )));
            }
        }
        let node = LiveNode::start_with(node_cfg, view, known.clone(), anchors, limits);
        if let Some(bytes) = &given_record {
            node.originate(rhtn_node::store::KIND_ENDPOINT_RECORD, bytes);
        }
        // an instance's credential in force goes to its horizon at once
        // (`wire-format.md` §8.2, §10.1)
        if credential.is_some() {
            let ids = node.ids.lock().unwrap().clone();
            node.view
                .lock()
                .unwrap()
                .push_credential(&node.adjacency, ids.as_slice());
        }

        // **§10.1.3's second repair path**, started here because its
        // interval is the operator's number.  A new adjacency is the
        // first repair path and the node does that itself; this is the
        // one for a frame missed while the session was already up.
        // Zero is an operator saying its links do not lose frames.
        if cfg.reconcile_secs > 0 {
            rhtn_node::runtime::reconcile_every(
                node.view.clone(),
                node.adjacency.clone(),
                Duration::from_secs(cfg.reconcile_secs),
            );
        }

        // upstream, where the configuration names one: a root attaches to
        // nobody (design §14.1.2)
        let upstream = match &cfg.upstream {
            None => None,
            Some((patron, addrs)) => {
                let ccfg = client_config(party.clone(), pins, addrs, patron);
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
        let service = Service {
            node,
            prekeys: cfg.prekeys.clone(),
            topology: cfg.topology.clone(),
            archive: cfg.archive.clone(),
            hosts_resources,
            _upstream: upstream,
            credential,
            noticed: std::sync::Mutex::new(None),
        };
        service.mind_the_run();
        Ok(service)
    }

    /// An instance's run, looked at: new credentials taken from the
    /// directory, the one now in force pushed if it was not, and the
    /// operator told while more than one remains
    /// (`infra-client-requirements.md` §7).  Nothing for a node holding
    /// its seed.
    pub fn mind_the_run(&self) {
        let Some((cred, dir)) = &self.credential else {
            return;
        };
        let operator = self.node.view.lock().unwrap().public.clone();
        let _ = read_run(dir, cred, &operator);
        let ids = self.node.ids.lock().unwrap().clone();
        self.node
            .view
            .lock()
            .unwrap()
            .push_credential(&self.node.adjacency, ids.as_slice());
        let now = self.node.view.lock().unwrap().now();
        let remaining = credentials_remaining(cred, now);
        let mut noticed = self.noticed.lock().unwrap();
        if remaining <= NOTICE_AT_CREDENTIALS && *noticed != Some(remaining) {
            *noticed = Some(remaining);
            let end = cred.run_end().unwrap_or(now);
            eprintln!(
                "rhtnd: {remaining} credential{} remain in the run, which ends at {end}: sign the next run",
                if remaining == 1 { "" } else { "s" }
            );
        }
    }

    /// What this node's configuration exposes the identities below it to
    /// (`infra-client-requirements.md` §8).  A node always relays payload
    /// for the clients it serves, which is what a serving node is for
    /// (design §14.1.6).
    pub fn exposure(&self) -> crate::operator::ExposureView {
        let view = self.node.view.lock().unwrap();
        let me = view.me();
        let subordinates = view.table.subordinates(&me).len();
        crate::operator::ExposureView::new(
            subordinates,
            true,
            self.hosts_resources,
            view.serving_node.is_some(),
        )
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
        view.archive.save(&self.archive)?;
        view.store.save(&self.topology)?;
        // the derived state last, so it is never newer than the store it
        // claims to be a fold of: a snapshot ahead of its store would be
        // discarded on the next start, which costs a replay, while a
        // snapshot behind is exactly the case the watermark handles
        std::fs::write(self.topology.join(DERIVED), view.materialise().encode())
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
                    self.mind_the_run();
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
    rhtn_archive::tx::Locator {
        anchor: *me,
        path: Vec::new(),
        nibbles: 0,
        seqno: rhtn_archive::tx::Seqno {
            series: 1,
            counter: 0,
        },
    }
}

fn client_config(
    me: rhtn_transport::tls::Party,
    pins: Pins,
    addrs: &[std::net::SocketAddr],
    patron: &Keyhash,
) -> rhtn_transport::session::ClientConfig {
    let book: std::collections::HashMap<[u8; 32], Vec<std::net::SocketAddr>> =
        std::collections::HashMap::from([(*patron, addrs.to_vec())]);
    let mut bind = rhtn_transport::bind::Binding::default();
    if let Some(c) = me.credential() {
        bind = bind.with_credential(c.clone());
    }
    rhtn_transport::session::ClientConfig {
        me,
        pins,
        bind,
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
