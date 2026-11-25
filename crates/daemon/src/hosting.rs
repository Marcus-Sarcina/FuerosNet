//! The packages this node hosts, and who may reach them.
//!
//! Two obligations meet here. `infra-client-requirements.md` §9 has the
//! operator decide what to host and confine what they admit, and §10 has
//! them hold a role table the gateway consults by lookup. Both are
//! configuration rather than protocol: nothing a peer sees is decided in
//! this file, and a node that hosts nothing is conforming.
//!
//! The file is TOML, and stays a file of its own now that it need not be:
//! what a node hosts and who may reach it is what changes most often, and
//! an operator regenerating it should not be rewriting the identity, the
//! listen address and the paths beside it.
//!
//! ```toml
//! [[host]]
//! resource  = "<keyhash>"
//! owner     = "<keyhash>"
//! authority = "shop.internal"
//! manifest  = "packages/shop/manifest"
//! ```
//!
//! **An instance's federation relationships are configured here too**
//! (`rhtn_node::federation`; `resource-design/federation-model.md` §6:
//! "topology follows patronage"), beside the resource they belong to:
//!
//! ```toml
//! [[host]]
//! resource     = "<keyhash>"
//! patron       = "<the patron's instance, a resource keyhash>"
//! subordinates = ["<a subordinate's instance>", "..."]
//! [host.kin]
//! guild = ["<a kin instance>", "..."]
//! ```
//!
//! Each names another *instance* by its resource keyhash, never a
//! person or a node. Nothing here is checked against the topology: the
//! relationship is the operator's statement of whom their instance
//! federates with, and the node carries envelopes within it and no
//! further ([`relationships`]).
//!
//! **Where a related instance another node hosts is reached** is written
//! beside it too ([`locations`]; `rhtn_node::federation::Location`): that
//! node's federation listener and the raw public key it presents, as its
//! operator gave them:
//!
//! ```toml
//! [host.at]
//! "<a related instance>" = { address = "203.0.113.9:7452", key = "<64 hex>" }
//! ```
//!
//! **Who may reach a package is not written here** ([`crate::grants`]): a
//! grant is a row of the node's own table, which an act on the operator's
//! surface rewrites, and a hosting file that still carries one is refused.
//!
//! **A `host` line names a manifest, not a component.** What a package
//! declares is the package's, and §9.1 puts the declaration in the
//! manifest that ships with it; an operator writing role names into their
//! own configuration would be declaring them on the package's behalf. The
//! manifest sits beside its component and is `key = value` a line at a
//! time:
//!
//! ```text
//! roles = reader,writer
//! imports = rhtn/1:request,rhtn/1:response
//! component = echo.wasm
//! ```
//!
//! **What it declares is checked against what the component does**, in
//! both directions. A manifest importing a binding this host does not have
//! is refused by `rhtn_node::resources::instantiate`; a component reaching
//! for one its manifest did not declare is refused here. Neither is a
//! supply chain (§9.1) — that is signing and provenance and is not this
//! file — but a declaration nothing compares against declares nothing.
//!
//! `connect` in a grant is the gate `resource-requirements.md` §3 reserves
//! for the node's own evaluation, which is why it appears here and never
//! reaches the package: the roles a package is told about are the rest of
//! the list. A grant without it is a row that cannot connect, which is a
//! thing an operator may want to write down.

use rhtn_archive::Keyhash;
use rhtn_node::federation::{Location, Relationships};
use rhtn_node::grant::{Clause, Grant, Template};
use rhtn_node::relay::Relay;
use rhtn_node::resources::{
    Backend, Binding, Gateway, Kind, Manifest, Operation, Parameter, instantiate,
};
use rhtn_resources::{Hosted, Limits, Sandbox};
use std::collections::BTreeSet;
use std::fmt::Write;
use std::path::Path;
use std::sync::Arc;

/// Why a hosting file was refused: the line it was on, and what was wrong
/// with it.  Line 0 means the file as a whole, which is where everything
/// but a shape error lands — a package that will not run is about the
/// package and not about a line in this file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Refused {
    /// The line it was on; 0 means the file as a whole.
    pub line: usize,
    /// What was wrong with it.
    pub what: String,
}

impl std::fmt::Display for Refused {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.line {
            0 => write!(f, "{}", self.what),
            n => write!(f, "line {n}: {}", self.what),
        }
    }
}

impl std::error::Error for Refused {}

pub(crate) fn at(line: usize, what: impl Into<String>) -> Refused {
    Refused {
        line,
        what: what.into(),
    }
}

fn keyhash(s: &str) -> Option<Keyhash> {
    if s.len() != 64
        || !s
            .bytes()
            .all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c))
    {
        return None;
    }
    let mut out = [0u8; 32];
    for (i, b) in out.iter_mut().enumerate() {
        *b = u8::from_str_radix(&s[i * 2..i * 2 + 2], 16).ok()?;
    }
    Some(out)
}

/// The file's own shape, before any package is read.
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct File {
    #[serde(default)]
    host: Vec<HostEntry>,
}

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct HostEntry {
    resource: String,
    owner: String,
    authority: String,
    manifest: std::path::PathBuf,
    /// Where the resource listens, where it holds its own port rather
    /// than running inside this node's sandbox
    /// (`resource-requirements.md` §3's second leg,
    /// `rhtn_node::relay::Relay`).
    ///
    /// **Which of the two a resource is follows from this**, which is
    /// §10.6's "it follows from where the resource runs, so nothing needs
    /// declaring": an address and the node proxies, a component and the
    /// node runs it, neither and the node brokers.
    address: Option<String>,
    /// Where the host mounted a block of the resource's own inside its
    /// container, where it did (`infra-client-requirements.md` §9.3)
    /// [author, 2026-11-15]: told to the instance as `rhtn-block` in
    /// place of the surface, and nothing provisioned here for it.
    block: Option<String>,
    /// **Its storage sealed against an image of the machine**
    /// (`infra-client-requirements.md` §9.4) [author, 2026-11-22]: kept in
    /// one directory under the host's filesystem encryption, and served
    /// only while its key is in the host's kernel.
    #[serde(default)]
    sealed: bool,
    /// **Where grants used to be written**, kept only to say where they
    /// went: this file is the operator's and the node never rewrites it,
    /// and a grant is a table update an act on the operator's surface
    /// does rewrite ([`crate::grants`]).
    #[serde(default)]
    grant: Vec<GrantEntry>,
    /// The patron's instance this resource federates with
    /// ([`rhtn_node::federation::Relationships`]).
    patron: Option<String>,
    /// The subordinates' instances.
    #[serde(default)]
    subordinates: Vec<String>,
    /// Named kin groups.
    #[serde(default)]
    kin: std::collections::BTreeMap<String, Vec<String>>,
    /// Where related instances other nodes host are reached
    /// ([`locations`]).
    #[serde(default)]
    at: std::collections::BTreeMap<String, AtEntry>,
}

/// One related instance's node: its federation listener and the key it
/// presents.
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct AtEntry {
    address: String,
    key: String,
}

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct GrantEntry {
    roles: Vec<String>,
    /// The clauses a member must satisfy, all of them. **Absent grants to
    /// every member of the owner's horizon** — §10.1's outer gate and
    /// nothing further.
    #[serde(default, rename = "where")]
    clauses: Vec<ClauseEntry>,
    /// When it lapses, in seconds by the node's clock; absent is never
    /// (`resource-requirements.md` §7.1.2).
    #[serde(default)]
    until: Option<u64>,
}

/// One clause of §10.3's vocabulary as it is written down.
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ClauseEntry {
    of: String,
    edges: Option<usize>,
    n: Option<usize>,
    percent: Option<u8>,
    when: Option<u64>,
    who: Option<String>,
}

/// Read one clause, refusing a key that means nothing where it is written.
///
/// **The same rule the administrative parameters follow**: `of` decides
/// which other keys have a meaning, and a package or operator who wrote
/// one that does not believes something the host will not do.
fn clause_of(c: &ClauseEntry) -> Result<Clause, String> {
    let only = |keys: [bool; 5], names: &str| -> Result<(), String> {
        let given = [
            c.edges.is_some(),
            c.n.is_some(),
            c.percent.is_some(),
            c.when.is_some(),
            c.who.is_some(),
        ];
        for (i, g) in given.iter().enumerate() {
            if *g && !keys[i] {
                return Err(format!("`{}` takes {names}", c.of));
            }
        }
        Ok(())
    };
    let need =
        |what: Option<usize>, key: &str| what.ok_or_else(|| format!("`{}` names `{key}`", c.of));
    match c.of.as_str() {
        "clients" => {
            only([false; 5], "no other key")?;
            Ok(Clause::Clients)
        }
        "grandclients" => {
            only([false; 5], "no other key")?;
            Ok(Clause::Grandclients)
        }
        "distance" => {
            only([true, false, false, false, false], "`edges`")?;
            Ok(Clause::AtDistance {
                edges: need(c.edges, "edges")?,
            })
        }
        "most-trusted" => {
            only([false, true, false, false, false], "`n`")?;
            Ok(Clause::MostTrusted { n: need(c.n, "n")? })
        }
        "top-fraction" => {
            only([false, false, true, false, false], "`percent`")?;
            Ok(Clause::TopFraction {
                percent: c
                    .percent
                    .ok_or_else(|| format!("`{}` names `percent`", c.of))?,
            })
        }
        "joined-before" => {
            only([false, false, false, true, false], "`when`")?;
            Ok(Clause::JoinedBefore {
                when: c.when.ok_or_else(|| format!("`{}` names `when`", c.of))?,
            })
        }
        "named" => {
            only([false, false, false, false, true], "`who`")?;
            let who = c.who.as_deref().unwrap_or_default();
            Ok(Clause::Named {
                who: keyhash(who)
                    .ok_or_else(|| format!("`{who}` is not 64 lower-case hex digits"))?,
            })
        }
        other => Err(format!(
            "`{other}` is not a clause; they are clients, grandclients, \
             distance, most-trusted, top-fraction, joined-before and named"
        )),
    }
}

/// The grant an entry describes.
pub(crate) fn grant_of(g: &GrantEntry) -> Result<Grant, String> {
    let mut roles = BTreeSet::new();
    for r in &g.roles {
        roles.insert(r.clone());
    }
    let mut clauses = Vec::new();
    for c in &g.clauses {
        clauses.push(clause_of(c)?);
    }
    Ok(Grant {
        roles,
        clauses,
        until: g.until,
    })
}

/// **Where related instances other nodes host are reached**, from the
/// `[host.at]` tables of the hosting file ([`rhtn_node::federation::Location`]):
/// each names an instance, the federation listener of the node hosting
/// it, and the raw public key that node presents, which the far operator
/// gave. One instance is at one node: the same instance located twice
/// with different particulars is refused.
pub fn locations(path: &Path) -> Result<Vec<(Keyhash, Location)>, Refused> {
    let text =
        std::fs::read_to_string(path).map_err(|e| at(0, format!("{}: {e}", path.display())))?;
    let file: File = toml::from_str(&text).map_err(|e| at(0, e.to_string()))?;
    let mut out: Vec<(Keyhash, Location)> = Vec::new();
    for h in &file.host {
        for (instance, where_) in &h.at {
            let k = keyhash(instance).ok_or_else(|| {
                at(
                    0,
                    format!(
                        "`{}`: `{instance}` is not an instance (64 lower-case hex digits)",
                        h.resource
                    ),
                )
            })?;
            let address: std::net::SocketAddr = where_.address.parse().map_err(|_| {
                at(
                    0,
                    format!(
                        "`{instance}`: `{}` is not an address and port",
                        where_.address
                    ),
                )
            })?;
            let key = keyhash(&where_.key).ok_or_else(|| {
                at(
                    0,
                    format!("`{instance}`: `key` is 64 lower-case hex digits"),
                )
            })?;
            let loc = Location { address, key };
            if let Some((_, other)) = out.iter().find(|(i, _)| *i == k)
                && *other != loc
            {
                return Err(at(
                    0,
                    format!(
                        "`{instance}` is located twice, differently: one instance is at one node"
                    ),
                ));
            }
            out.push((k, loc));
        }
    }
    out.dedup();
    Ok(out)
}

/// **The federation relationships the hosting file configures**, per
/// resource ([`rhtn_node::federation::Relationships`]): read from the
/// same file as [`apply`], after it, so the planes follow the bindings.
/// Every resource the file hosts gets relationships, empty where it
/// names none, so a plane exists for each and an operator's surface can
/// deliver to it.
pub fn relationships(path: &Path) -> Result<Vec<(Keyhash, Relationships)>, Refused> {
    let text =
        std::fs::read_to_string(path).map_err(|e| at(0, format!("{}: {e}", path.display())))?;
    let file: File = toml::from_str(&text).map_err(|e| at(0, e.to_string()))?;
    let mut out = Vec::new();
    for h in &file.host {
        let res = keyhash(&h.resource).ok_or_else(|| {
            at(
                0,
                format!("`{}` is not 64 lower-case hex digits", h.resource),
            )
        })?;
        let one = |s: &String| {
            keyhash(s).ok_or_else(|| {
                at(
                    0,
                    format!(
                        "`{}`: `{s}` is not an instance (64 lower-case hex digits)",
                        h.resource
                    ),
                )
            })
        };
        let mut r = Relationships {
            patron: h.patron.as_ref().map(one).transpose()?,
            ..Default::default()
        };
        for s in &h.subordinates {
            r.subordinates.insert(one(s)?);
        }
        for (name, members) in &h.kin {
            let mut set = std::collections::BTreeSet::new();
            for s in members {
                set.insert(one(s)?);
            }
            r.kin.insert(name.clone(), set);
        }
        if r.all().contains(&res) {
            return Err(at(
                0,
                format!("`{}` names itself as a relationship", h.resource),
            ));
        }
        if let Some(why) = r.refused() {
            return Err(at(0, format!("`{}`: {why}", h.resource)));
        }
        out.push((res, r));
    }
    Ok(out)
}

/// What applying the hosting file bound, and what each instance is to be
/// told of its host (`infra-client-requirements.md` §9.3): the surface
/// version, and where its block and its log are — carried on the
/// relationships exchange, which is repeated until the instance takes it.
pub struct Applied {
    /// How many resources were bound.
    pub bound: usize,
    /// Per resource, the host facts as headers.
    pub facts: Vec<(Keyhash, Vec<(String, String)>)>,
    /// Per resource, the block and the log provisioned for it, so that
    /// [`Applied::over`] can tell the instance where to reach them.
    #[allow(clippy::type_complexity)]
    pub provisioned: Vec<(
        Keyhash,
        Option<Arc<rhtn_resources::Block>>,
        Option<Arc<rhtn_resources::LogFile>>,
        [u8; 32],
    )>,
    /// **Where this file's sandboxed instances say what waits for a
    /// member** (`rhtn/1:notices`), lent the node's sink by
    /// [`Applied::over`].
    pub notices: Arc<std::sync::Mutex<Option<crate::host::Notices>>>,
    /// **The sealed resources whose key is not in the kernel**
    /// (`infra-client-requirements.md` §9.4): bound so they answer as a
    /// package that is not running, with nothing of them provisioned and
    /// no federation plane, so an envelope for one is refused and its
    /// sender keeps it.
    pub closed: Vec<Keyhash>,
    /// **The media port each package declared**
    /// (`infra-client-requirements.md` §9.3) [author, 2026-11-24], so
    /// [`Applied::reached`] can tell it where this machine is reached.
    pub media: Vec<(Keyhash, u16)>,
}

/// **A sealed package whose key is not in the host's kernel**
/// (`infra-client-requirements.md` §9.4): it answers as a package that is
/// not running, and runs no jobs.
struct Closed;

impl Backend for Closed {
    fn handle(&self, _: &[u8]) -> Result<Vec<u8>, String> {
        Err("sealed and waiting for its operator".into())
    }
    fn running(&self) -> bool {
        false
    }
}

impl Applied {
    /// **Name the surface each instance reaches its block, its log and its
    /// notices over** (`infra-client-requirements.md` §9.3,
    /// `resource-requirements.md` §3.3): every instance is registered,
    /// since any may count what waits for a member [author, 2026-11-23],
    /// and is told `rhtn-notices`, `rhtn-block` and `rhtn-log` where it has
    /// them, and the `rhtn-secret` that says which instance is calling.
    ///
    /// **Called by whoever applied the hosting file**, because the facts
    /// are only true once this node is serving them: a URL told before
    /// anything was bound would name nothing. The surface also forgets
    /// every instance this file did not bind, so an instance the operator
    /// stopped hosting loses its block with the binding and not after it.
    pub fn over(&mut self, surface: &crate::host::HostSurface) {
        surface.lend(self.notices.clone());
        let mine: Vec<Keyhash> = self.provisioned.iter().map(|(r, _, _, _)| *r).collect();
        surface.retain(&mine);
        for (res, block, log, secret) in &self.provisioned {
            let secret = surface.register_with(*res, block.clone(), log.clone(), *secret);
            let Some((_, own)) = self.facts.iter_mut().find(|(r, _)| r == res) else {
                continue;
            };
            if block.is_some() {
                own.push(("rhtn-block".into(), surface.block_url()));
            }
            if log.is_some() {
                own.push(("rhtn-log".into(), surface.log_url()));
            }
            own.push(("rhtn-notices".into(), surface.notices_url()));
            own.push(("rhtn-secret".into(), crate::host::hex_secret(&secret)));
        }
    }

    /// **Tell each package that declared a media port where this machine
    /// is reached** (`infra-client-requirements.md` §9.3) [author,
    /// 2026-11-24]: `rhtn-media`, its port at each address this node's
    /// own endpoint record names, in the record's order. A package
    /// composes what it offers a member's client from that, having no
    /// other way to learn an address its operator never typed.
    ///
    /// **A node that publishes no address tells none**, and the package
    /// has nowhere to offer.
    pub fn reached(&mut self, at: &[std::net::IpAddr]) {
        if at.is_empty() {
            return;
        }
        for (res, port) in &self.media {
            let said = at
                .iter()
                .map(|ip| std::net::SocketAddr::new(*ip, *port).to_string())
                .collect::<Vec<_>>()
                .join(" ");
            let fact = ("rhtn-media".to_string(), said);
            match self.facts.iter_mut().find(|(r, _)| r == res) {
                Some((_, own)) => own.push(fact),
                None => self.facts.push((*res, vec![fact])),
            }
        }
    }
}

/// **The addresses this node's own endpoint record names**, in its order
/// and each once: where a member's client reaches this machine, and so
/// where it reaches a media port held on it ([`Applied::reached`]).
/// Nothing where the node holds no record or cannot read the one it has,
/// which its start and its surface each refuse in their own words.
pub fn reached_at(record: Option<&Path>) -> Vec<std::net::IpAddr> {
    let Some(bytes) = record.and_then(|p| std::fs::read(p).ok()) else {
        return Vec::new();
    };
    let Ok(record) = rhtn_node::store::EndpointRecord::parse(&bytes) else {
        return Vec::new();
    };
    let mut at = Vec::new();
    for point in &record.endpoints {
        if let Ok(np) = rhtn_node::resolution::NetworkPoint::decode_bytes(point) {
            let ip = np.socket().ip();
            if !at.contains(&ip) {
                at.push(ip);
            }
        }
    }
    at
}

/// A block's ceiling where the manifest imports the block and budgets
/// nothing: a package's own claim is expected, and this is the host's
/// floor for one that made none.
pub const DEFAULT_BLOCK_BYTES: u64 = 64 << 20;
/// A log's retention where the manifest names none.
pub const DEFAULT_LOG_BYTES: u64 = 1 << 20;

/// Read `path` and bind what it names into `gateway`.
///
/// **Every package is admitted before any is bound**, and every grant is
/// checked before that. A file naming one package the sandbox refuses, or
/// one role the package never declared, leaves the node hosting none of
/// them rather than some of them: a half-applied configuration is one the
/// operator did not write, and `service.rs` reports rather than repairs.
pub fn apply(
    gateway: &mut Gateway,
    path: &Path,
    limits: Limits,
    grants_dir: Option<&Path>,
    state: Option<&Path>,
) -> Result<Applied, Refused> {
    apply_in(gateway, path, limits, grants_dir, state, None, &[])
}

/// [`apply`], with the directory the node keeps federation custody in, so
/// an open sealed resource's plane is linked into its sealed directory
/// beside its block, its log and its jobs (§9.4); and with the UDP ports
/// this node itself holds, which no package's media port may be (§9.3).
pub fn apply_in(
    gateway: &mut Gateway,
    path: &Path,
    limits: Limits,
    grants_dir: Option<&Path>,
    state: Option<&Path>,
    federation: Option<&Path>,
    held: &[u16],
) -> Result<Applied, Refused> {
    let text =
        std::fs::read_to_string(path).map_err(|e| at(0, format!("{}: {e}", path.display())))?;
    let file: File = toml::from_str(&text).map_err(|e| {
        at(
            e.span().map_or(0, |s| {
                text[..s.start.min(text.len())]
                    .bytes()
                    .filter(|b| *b == b'\n')
                    .count()
                    + 1
            }),
            e.to_string(),
        )
    })?;

    let mut hosts: Vec<(Keyhash, Binding)> = Vec::new();
    let mut grants: Vec<(Keyhash, Vec<Grant>)> = Vec::new();
    let mut facts: Vec<(Keyhash, Vec<(String, String)>)> = Vec::new();
    let notices: Arc<std::sync::Mutex<Option<crate::host::Notices>>> =
        Arc::new(std::sync::Mutex::new(None));
    #[allow(clippy::type_complexity)]
    let mut provisioned: Vec<(
        Keyhash,
        Option<Arc<rhtn_resources::Block>>,
        Option<Arc<rhtn_resources::LogFile>>,
        [u8; 32],
    )> = Vec::new();
    // **two resources at one address is a collision, not a choice.**
    // Nothing partitions ports across package authors and nothing could,
    // so where the operator names the address the file is the only place
    // the clash can be seen.  Where the node comes to assign addresses
    // itself — §9's image, once it launches what it hosts — this is the
    // table it will assign into.
    let mut addresses: Vec<(std::net::SocketAddr, String)> = Vec::new();
    let mut closed: Vec<Keyhash> = Vec::new();
    let mut media: Vec<(Keyhash, u16)> = Vec::new();

    for h in &file.host {
        let named = |what: &str| format!("{}: {what}", h.manifest.display());
        let res = keyhash(&h.resource).ok_or_else(|| {
            at(
                0,
                format!("`{}` is not 64 lower-case hex digits", h.resource),
            )
        })?;
        let owner = keyhash(&h.owner)
            .ok_or_else(|| at(0, format!("`{}` is not 64 lower-case hex digits", h.owner)))?;
        if hosts.iter().any(|(r, _)| *r == res) {
            return Err(at(0, format!("`{}` is hosted twice", h.resource)));
        }
        let (manifest, component) = read_manifest(&h.manifest).map_err(|e| at(0, named(&e)))?;
        instantiate(&manifest).map_err(|e| at(0, named(&e)))?;
        // **one media port is one package's** (`infra-client-requirements.md`
        // §9.3) [author, 2026-11-24]: declared by a package on a socket
        // of its own, the only kind with a network to hold one on, and
        // refused where this node or another package already holds it.
        // The same reasoning as an address below: nothing partitions
        // ports across package authors, so this is where a clash is seen
        if let Some(port) = manifest.media {
            if h.address.is_none() {
                return Err(at(
                    0,
                    named(
                        "the package declares a media port, and only a package on a socket \
                         of its own has a network to hold one on",
                    ),
                ));
            }
            if held.contains(&port) {
                return Err(at(
                    0,
                    named(&format!(
                        "the package declares media port {port}, which this node itself holds"
                    )),
                ));
            }
            if let Some((first, _)) = media.iter().find(|(_, p)| *p == port) {
                return Err(at(
                    0,
                    named(&format!(
                        "the package declares media port {port}, which `{}` already holds: \
                         one port is one package's",
                        hex_of(first)
                    )),
                ));
            }
            media.push((res, port));
        }
        // **sealed storage is served only while it is open** (§9.4): its
        // directory sealed at installation, its key in the kernel since
        // the machine started; open, what it keeps is linked into it
        let shut = match (h.sealed, state) {
            (false, _) => false,
            (true, None) => {
                return Err(at(
                    0,
                    format!(
                        "`{}` is sealed, and a sealed package's storage is under the node's \
                         state directory, which this node keeps none of",
                        h.resource
                    ),
                ));
            }
            (true, Some(root)) => {
                if h.block.is_some() {
                    return Err(at(
                        0,
                        format!(
                            "`{}` is sealed and names a mounted block: a mounted block is the \
                             host's filesystem and not this node's to seal",
                            h.resource
                        ),
                    ));
                }
                // **installed sealed and not yet given its key**, which its
                // operator's client brings when it next attaches: closed
                // until then, unless storage it was never sealed over is
                // already there
                if !crate::sealing::is_sealed(root, &res)
                    && let Some(plain) = crate::sealing::unsealed_storage(root, &res)
                {
                    return Err(at(
                        0,
                        format!(
                            "`{}` is sealed and {} was provisioned unsealed: storage is \
                             sealed when it is provisioned",
                            h.resource,
                            plain.display()
                        ),
                    ));
                }
                let shut = !crate::sealing::is_open(root, &res);
                if !shut {
                    crate::sealing::link(root, federation, &res).map_err(|e| at(0, e))?;
                }
                shut
            }
        };
        if shut {
            closed.push(res);
        }
        // **the host surface the package asked for, provisioned**
        // (`infra-client-requirements.md` §9.3): a block where it budgeted
        // or imports one, a log where it imports or budgets one, each
        // under the node's state directory and named to the instance; a
        // node with no such directory cannot host a package that needs
        // one, which §9 makes ordinary capacity, said at installation
        // **the instance's secret** (`resource-requirements.md` §3.1): the
        // host establishes it when it starts the instance, presents it on
        // every hop to a socket of the instance's own, and takes it on the
        // instance's requests to its block and log
        let secret = instance_secret(state, &res);
        let wants_block = !shut
            && (manifest.budget.block_bytes.is_some()
                || manifest.imports.iter().any(|i| i == "rhtn/1:block"));
        let wants_log = !shut
            && (manifest.budget.log_bytes.is_some()
                || manifest.imports.iter().any(|i| i == "rhtn/1:log"));
        let env = match (state, wants_block || wants_log) {
            (None, true) => {
                return Err(at(
                    0,
                    named(
                        "the package needs a storage block or a log, and this node keeps no \
                         state directory for its packages (`state` in the configuration): \
                         it cannot host it",
                    ),
                ));
            }
            (Some(root), _) => {
                let mut env = rhtn_resources::Env::default();
                let mut own: Vec<(String, String)> = vec![(
                    "rhtn-host".into(),
                    rhtn_node::resources::HOST_VERSIONS.join(" "),
                )];
                // **the ceiling is a fact of the instance's and goes with
                // the facts; where to reach the block is the surface's to
                // say** [author, 2026-11-05], so `rhtn-block` and
                // `rhtn-log` are added by [`Applied::over`] and not here:
                // a path on this node's filesystem is not something a
                // package on a socket of its own can write
                // (`infra-client-requirements.md` §9.3)
                let mut kept_block = None;
                let mut kept_log = None;
                if wants_block {
                    let quota = manifest.budget.block_bytes.unwrap_or(DEFAULT_BLOCK_BYTES);
                    own.push(("rhtn-block-bytes".into(), quota.to_string()));
                    if let Some(path) = &h.block {
                        // **mounted by the host** [author, 2026-11-15]: a
                        // filesystem of the instance's own, bounded by the
                        // host's quota and reached at this path inside its
                        // container; nothing is provisioned here, and the
                        // surface keeps no block for it
                        own.push(("rhtn-block".into(), path.clone()));
                    } else {
                        let dir = root.join("blocks").join(&h.resource);
                        let block = rhtn_resources::Block::provision(&dir, quota)
                            .map_err(|e| at(0, format!("{}: {e}", dir.display())))?;
                        let block = Arc::new(block);
                        kept_block = Some(block.clone());
                        env.block = Some(block);
                    }
                }
                if wants_log {
                    let retain = manifest.budget.log_bytes.unwrap_or(DEFAULT_LOG_BYTES);
                    let file = root.join("logs").join(format!("{}.log", h.resource));
                    let log = rhtn_resources::LogFile::at(&file, retain)
                        .map_err(|e| at(0, format!("{}: {e}", file.display())))?;
                    own.push(("rhtn-log-bytes".into(), retain.to_string()));
                    let log = Arc::new(log);
                    kept_log = Some(log.clone());
                    env.log = Some(log);
                }
                provisioned.push((res, kept_block, kept_log, secret));
                facts.push((res, own));
                env
            }
            (None, false) => rhtn_resources::Env::default(),
        };
        let mut env = env;
        // the host as the package's egress, to the destinations it
        // declared and nowhere else (§9.3)
        if manifest.imports.iter().any(|i| i == "rhtn/1:fetch") {
            env.fetch = Some(Arc::new(crate::fetch::Outbound::new(&manifest.fetch)));
        }
        // its jobs, kept on disk beside its block and bounded as it
        // declared (§9.3): a node with no state directory cannot keep them
        if !shut && manifest.imports.iter().any(|i| i == "rhtn/1:jobs") {
            let Some(root) = state else {
                return Err(at(
                    0,
                    named(
                        "the package schedules jobs, and this node keeps no state directory \
                         for its packages (`state` in the configuration): it cannot host it",
                    ),
                ));
            };
            let dir = root.join("jobs").join(&h.resource);
            let table = rhtn_resources::Jobs::provision(
                &dir,
                manifest.budget.jobs.unwrap_or(rhtn_resources::DEFAULT_JOBS),
                manifest
                    .budget
                    .job_seconds_per_day
                    .unwrap_or(rhtn_resources::DEFAULT_JOB_SECONDS_PER_DAY),
            )
            .map_err(|e| at(0, format!("{}: {e}", dir.display())))?;
            env.jobs = Some(Arc::new(table));
        }
        // **what it counts as waiting for a member** (§9.3), said to the
        // node through the slot this file's instances share, under this
        // instance's own name
        if !shut && manifest.imports.iter().any(|i| i == "rhtn/1:notices") {
            let (slot, instance) = (notices.clone(), res);
            env.notices = Some(Arc::new(move |principal, count, line| {
                let sink = slot.lock().unwrap().clone();
                match sink {
                    Some(s) => s(instance, principal, count, line),
                    None => Err("this node is not up yet".into()),
                }
            }));
        }
        // **a resource is reached one way, and the file says which**
        // (§10.6): an address and this node proxies to it, a component
        // and this node runs it.  Both is a contradiction rather than a
        // choice, and the operator is told so rather than having one of
        // them silently win
        // a mounted block is a socket package's: a component in this
        // node's sandbox reaches its block as a binding, and a path inside
        // a container it does not have names nothing
        if let Some(b) = &h.block
            && h.address.is_none()
        {
            return Err(at(
                0,
                named(&format!(
                    "`block = \"{b}\"` names a mount inside a container, and this resource runs \
                     in the node's sandbox, which has none"
                )),
            ));
        }
        let backend: Option<Arc<dyn rhtn_node::resources::Backend>> = match (&h.address, &component)
        {
            (Some(_), Some(c)) => {
                return Err(at(
                    0,
                    named(&format!(
                        "an address and a component at {}: a resource this node proxies to \
                             is not one it runs",
                        c.display()
                    )),
                ));
            }
            (Some(a), None) => {
                let addr: std::net::SocketAddr = a
                    .parse()
                    .map_err(|e| at(0, format!("`{a}` is not an address: {e}")))?;
                // **the relay speaks plain HTTP, so it may only speak it
                // to a local socket** (`resource-requirements.md` §3:
                // "HTTPS, required, where it crosses a network", and
                // "plain HTTP is permitted only on a local socket to a
                // package the node hosts itself").  The node already read
                // the request, so what the far leg protects is everyone
                // else: an operator relaying to a resource elsewhere must
                // not put an authenticated principal and an application
                // body on the open network.  Refused rather than carried,
                // until that leg can be given TLS
                if !addr.ip().is_loopback() {
                    return Err(at(
                        0,
                        format!(
                            "`{a}` is not a local address: this leg carries plain HTTP, and \
                             `resource-requirements.md` §3 requires HTTPS wherever it \
                             crosses a network"
                        ),
                    ));
                }
                if let Some((_, first)) = addresses.iter().find(|(x, _)| *x == addr) {
                    return Err(at(
                        0,
                        format!(
                            "`{a}` is where `{first}` already listens: two resources at one address \
                             is one resource answering for both"
                        ),
                    ));
                }
                addresses.push((addr, h.resource.clone()));
                Some(Arc::new(
                    Relay::to(addr, limits.response).presenting(secret),
                ))
            }
            (None, Some(c)) => {
                let bytes = std::fs::read(c).map_err(|e| at(0, format!("{}: {e}", c.display())))?;
                let sandbox = Sandbox::admit(&bytes, limits)
                    .map_err(|e| at(0, format!("{}: {e}", c.display())))?;
                let mut declared: Vec<String> = manifest.imports.clone();
                declared.sort();
                declared.dedup();
                if sandbox.reaches() != declared {
                    return Err(at(
                        0,
                        named(&format!(
                            "the manifest declares {declared:?} and the component reaches {:?}",
                            sandbox.reaches()
                        )),
                    ));
                }
                Some(Arc::new(Hosted::with(sandbox, env)))
            }
            // **brokered**: the node authenticates and the caller's
            // traffic never passes through it (design §11.7, §10.6)
            (None, None) => None,
        };
        // a brokered resource carries no traffic here and keeps nothing to
        // seal, so only a backend is replaced
        let backend = match backend {
            Some(_) if shut => Some(Arc::new(Closed) as Arc<dyn Backend>),
            other => other,
        };

        if !h.grant.is_empty() {
            return Err(at(
                0,
                format!(
                    "`{}` carries grants: who may reach a resource is the node's own table \
                     now, under the `grants` directory, because an act on the \
                     operator's surface rewrites it and this file is never rewritten",
                    h.resource
                ),
            ));
        }
        // **read from the node's own table, not from this file.** The
        // grants are still checked against the package before anything is
        // bound, which is the same invariant from a different source
        let mut written = Vec::new();
        for grant in match grants_dir {
            Some(d) => crate::grants::read_for(d, &res)?,
            None => Vec::new(),
        } {
            // the gateway refuses it too, but by then this file has
            // already bound its earlier entries, and a half-applied
            // configuration is one the operator did not write
            if let Some(why) = grant.refused(&manifest.roles) {
                return Err(at(0, why.to_string()));
            }
            written.push(grant);
        }
        grants.push((res, written));
        hosts.push((
            res,
            Binding {
                owner,
                authority: h.authority.clone(),
                backend,
                declared: manifest,
            },
        ));
    }

    let bound = hosts.len();
    for (res, binding) in hosts {
        gateway.bind(res, binding);
    }
    for (res, written) in grants {
        gateway
            .set_grants(res, written)
            .map_err(|e| at(0, e.to_string()))?;
    }
    Ok(Applied {
        bound,
        facts,
        provisioned,
        notices,
        closed,
        media,
    })
}

/// TOML's own message with the line it is about, as `config.rs` renders
/// one: `Display` adds a caret diagram that does not belong in a
/// single-line refusal, and `message()` is the sentence without it.
fn manifest_error(text: &str, e: &toml::de::Error) -> String {
    let line = e
        .span()
        .map(|s| {
            text[..s.start.min(text.len())]
                .bytes()
                .filter(|b| *b == b'\n')
                .count()
                + 1
        })
        .unwrap_or(0);
    let m = e.message();
    let said = match m.strip_prefix("missing field ") {
        Some(rest) => format!("{rest} is not set, and has no default"),
        None => m.to_string(),
    };
    if line > 0 {
        format!("manifest line {line}: {said}")
    } else {
        format!("manifest: {said}")
    }
}

/// Read a package's manifest, and where its component sits where it has
/// one.
///
/// **TOML, for the reason the configuration is** (`config.rs`): the
/// line-oriented format this replaced could not repeat a key, and an
/// administrative operation is a repeated table with named fields and
/// parameters of its own — the same shape that moved the operator's
/// configuration off that format. Strict for the reason that one is too:
/// an unknown key or a missing one is an error, because a manifest the
/// host quietly repairs declares something the package did not.
pub(crate) fn read_manifest(path: &Path) -> Result<(Manifest, Option<std::path::PathBuf>), String> {
    #[derive(serde::Deserialize)]
    #[serde(deny_unknown_fields)]
    struct File {
        /// The component, where the node runs the resource itself.
        /// **Absent for a resource that holds its own port**, which the
        /// node reaches over a socket instead and never compiles.
        component: Option<String>,
        #[serde(default)]
        roles: Vec<String>,
        #[serde(default)]
        imports: Vec<String>,
        /// The host-surface versions the package tolerates
        /// (`rhtn_node::resources::HOST_VERSIONS`).
        #[serde(default)]
        host: Vec<String>,
        /// What it asks to be bounded at (`infra-client-requirements.md`
        /// §9.3).
        #[serde(default)]
        budget: BudgetEntry,
        /// The destinations it may fetch from, delivered with the package.
        #[serde(default)]
        fetch: Vec<String>,
        /// The one UDP port it holds on its host's machine for media
        /// (`resource-requirements.md` §3) [author, 2026-11-24].
        media_port: Option<u16>,
        /// The endpoints elsewhere its pages stream with, and whether
        /// they stream between its members' devices: its own statements,
        /// read by its operator [author, 2026-11-25].
        #[serde(default)]
        media_endpoints: Vec<String>,
        #[serde(default)]
        media_members: bool,
        #[serde(default)]
        admin: Vec<Op>,
        #[serde(default)]
        template: Vec<Tpl>,
    }
    #[derive(serde::Deserialize, Default)]
    #[serde(deny_unknown_fields)]
    struct BudgetEntry {
        block_bytes: Option<u64>,
        log_bytes: Option<u64>,
        jobs: Option<u64>,
        requests_per_minute: Option<u64>,
        job_seconds_per_day: Option<u64>,
    }
    /// A grant the package ships ready-made
    /// (`infra-client-requirements.md` §10.4), in the same clause
    /// vocabulary an operator's own grant is written in — which is the
    /// requirement, not a convenience: a template an operator cannot read
    /// back is the opaque configuration §7.3 refuses.
    #[derive(serde::Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Tpl {
        name: String,
        label: String,
        roles: Vec<String>,
        #[serde(default, rename = "where")]
        clauses: Vec<ClauseEntry>,
    }
    #[derive(serde::Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Op {
        name: String,
        label: String,
        #[serde(default)]
        help: String,
        #[serde(default)]
        parameter: Vec<Param>,
    }
    #[derive(serde::Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Param {
        name: String,
        label: String,
        #[serde(rename = "type")]
        kind: String,
        low: Option<i64>,
        high: Option<i64>,
        max: Option<usize>,
        #[serde(default)]
        of: Vec<String>,
    }

    let text = std::fs::read_to_string(path).map_err(|e| format!("{e}"))?;
    let f: File = toml::from_str(&text).map_err(|e| manifest_error(&text, &e))?;

    let mut admin = Vec::new();
    for op in f.admin {
        let mut parameters = Vec::new();
        for p in op.parameter {
            // **the type decides which other keys mean anything**, and a
            // key that means nothing where it is written is a package
            // author believing something the host does not do
            let kind = match p.kind.as_str() {
                "flag" => Kind::Flag,
                "number" => Kind::Number {
                    low: p
                        .low
                        .ok_or_else(|| format!("`{}`: a number names `low`", p.name))?,
                    high: p
                        .high
                        .ok_or_else(|| format!("`{}`: a number names `high`", p.name))?,
                },
                "text" => Kind::Text {
                    max: p
                        .max
                        .ok_or_else(|| format!("`{}`: text names `max`, its length", p.name))?,
                },
                "choice" => Kind::Choice { of: p.of.clone() },
                "several" => Kind::Several { of: p.of.clone() },
                "keyhash" => Kind::Keyhash,
                other => {
                    return Err(format!(
                        "`{}`: `{other}` is not a parameter type; they are \
                         flag, number, text, choice, several and keyhash",
                        p.name
                    ));
                }
            };
            parameters.push(Parameter {
                name: p.name,
                label: p.label,
                kind,
            });
        }
        admin.push(Operation {
            name: op.name,
            label: op.label,
            help: op.help,
            parameters,
        });
    }
    let mut templates = Vec::new();
    for t in f.template {
        templates.push(Template {
            name: t.name,
            label: t.label,
            grant: grant_of(&GrantEntry {
                roles: t.roles,
                clauses: t.clauses,
                // a template ships no expiry: taking one is the operator's
                // act, and so is bounding it
                until: None,
            })?,
        });
    }
    let manifest = Manifest {
        roles: f.roles.into_iter().collect(),
        imports: f.imports,
        admin,
        templates,
        host: f.host,
        budget: rhtn_node::resources::Budget {
            block_bytes: f.budget.block_bytes,
            log_bytes: f.budget.log_bytes,
            jobs: f.budget.jobs,
            requests_per_minute: f.budget.requests_per_minute,
            job_seconds_per_day: f.budget.job_seconds_per_day,
        },
        fetch: f.fetch,
        media: f.media_port,
        media_endpoints: f.media_endpoints,
        media_members: f.media_members,
    };
    // relative to the manifest, so a package is a directory an operator
    // can move without rewriting what is inside it
    let dir = path.parent().unwrap_or(Path::new("."));
    Ok((manifest, f.component.map(|c| dir.join(c))))
}

/// **Read a resource's own grants file**, which carries grants and nothing
/// else ([`crate::grants`]).
pub(crate) fn parse_grants(text: &str) -> Result<Vec<GrantEntry>, String> {
    #[derive(serde::Deserialize)]
    #[serde(deny_unknown_fields)]
    struct File {
        #[serde(default)]
        grant: Vec<GrantEntry>,
    }
    let f: File = toml::from_str(text).map_err(|e| e.to_string())?;
    Ok(f.grant)
}

/// **One grant as its file says it**, which is the inverse of what
/// [`parse_grants`] reads: a grant written out and read back is the grant
/// it was, and the test that says so is what keeps an act from quietly
/// changing what it persisted.
///
/// The names and the one parameter each clause takes come from the
/// language itself (`rhtn_node::grant::VOCABULARY`), so nothing here
/// decides what a clause is called.
pub(crate) fn grant_toml(g: &Grant) -> String {
    let mut out = String::from("\n[[grant]]\nroles = [");
    let roles: Vec<String> = g.roles.iter().map(|r| format!("\"{r}\"")).collect();
    out.push_str(&roles.join(", "));
    out.push_str("]\n");
    if let Some(t) = g.until {
        let _ = writeln!(out, "until = {t}");
    }
    for c in &g.clauses {
        let of = c.of();
        let _ = writeln!(out, "\n  [[grant.where]]\n  of = \"{of}\"");
        if let (Some(key), Some(v)) = (Clause::named(of).and_then(|n| n.takes), c.parameter()) {
            // a keyhash is a string where a count is a number, and the
            // vocabulary says which this clause carries
            let _ = if matches!(c, Clause::Named { .. }) {
                writeln!(out, "  {key} = \"{v}\"")
            } else {
                writeln!(out, "  {key} = {v}")
            };
        }
    }
    out
}

/// **The secret the host establishes for an instance** [author,
/// 2026-11-15]: kept under the state directory, one file per resource,
/// closed to everything but the host, minted on first need and read back
/// after a restart so an instance the runner restarts with its original
/// arguments still holds the secret the node presents.  Without a state
/// directory it is minted once per process and said so, since nothing
/// could keep it.
pub fn instance_secret(state: Option<&Path>, res: &Keyhash) -> [u8; 32] {
    use std::sync::{Mutex, OnceLock};
    static MINTED: OnceLock<Mutex<std::collections::BTreeMap<Keyhash, [u8; 32]>>> = OnceLock::new();
    let mint = || {
        let mut s = [0u8; 32];
        use aws_lc_rs::rand::SecureRandom as _;
        if aws_lc_rs::rand::SystemRandom::new().fill(&mut s).is_err() {
            crate::say!("rhtnd: no randomness for an instance's secret");
        }
        s
    };
    let Some(root) = state else {
        let held = MINTED.get_or_init(|| Mutex::new(Default::default()));
        let mut held = held.lock().unwrap();
        return *held.entry(*res).or_insert_with(mint);
    };
    let dir = root.join("secrets");
    let file = dir.join(format!("{}.secret", hex_of(res)));
    if let Ok(text) = std::fs::read_to_string(&file)
        && let Some(bytes) = unhex32(text.trim())
    {
        return bytes;
    }
    let s = mint();
    let _ = std::fs::create_dir_all(&dir);
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o700));
    }
    if std::fs::write(&file, crate::host::hex_secret(&s)).is_ok() {
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o600));
        }
    } else {
        crate::say!(
            "rhtnd: {}: an instance's secret could not be kept; it will not survive a restart",
            file.display()
        );
    }
    s
}

fn hex_of(k: &Keyhash) -> String {
    k.iter().map(|b| format!("{b:02x}")).collect()
}

fn unhex32(s: &str) -> Option<[u8; 32]> {
    if s.len() != 64 {
        return None;
    }
    let mut out = [0u8; 32];
    for (i, o) in out.iter_mut().enumerate() {
        *o = u8::from_str_radix(&s[2 * i..2 * i + 2], 16).ok()?;
    }
    Some(out)
}
