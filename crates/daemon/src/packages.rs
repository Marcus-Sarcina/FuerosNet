//! **What this node installed on its own host, and where that is written**
//! [author, 2026-11-02].
//!
//! ## A third owner, and why there had to be one
//!
//! [`crate::hosting`] states the rule this module changes: "the hosting
//! file says what this node hosts; the operator writes it and the node
//! never rewrites it". That was right while installing a package was
//! something an operator did with a shell. **It is wrong for the operator
//! this project is for**, whose only window on their machine is their
//! light client: for them the node *is* the package manager, and a package
//! manager that cannot write down what it installed has installed nothing.
//!
//! So the ownership is divided rather than reversed, as it already is
//! between the hosting file and [`crate::grants`]:
//!
//! | File | Whose | What it says |
//! |---|---|---|
//! | the hosting file, above [`SENTINEL`] | the operator's, untouched | the packages they put there by hand |
//! | the hosting file, below [`SENTINEL`] | the node's, generated | the packages installed through its surface |
//! | `packages.toml` | the node's | the record those entries are generated from |
//! | `relationships.toml` | the node's | whom each of those federates with, generated into its entry |
//! | `packages/<resource>.manifest` | the package's, relayed | what the package declares (§9.1) |
//!
//! **The generated region is derived and never read back.** Every line of
//! it comes from `packages.toml`, so an operator who edits it loses the
//! edit at the next install and nothing subtler happens — which is why the
//! sentinel says so in words. An operator who wants an entry of their own
//! writes it above the line, where the node does not look.
//!
//! **The manifest is the package's own declaration, relayed** (`infra-client-requirements.md`
//! §9.1 puts the capability declaration in the manifest that ships with
//! the package). The node writes it where [`crate::hosting::apply`] will
//! read it and checks nothing in it itself: `apply` already refuses a
//! manifest that declares what this host does not have, and a grant
//! naming a role the package never declared.

use rhtn_archive::Keyhash;
use std::path::{Path, PathBuf};

/// **The line the node owns everything below.** An operator reading their
/// hosting file meets this before it rather than afterwards.
pub const SENTINEL: &str = "# ---- installed through this node's own administration surface.";

/// The second line of it: what an operator may do about it.
pub const SENTINEL_WHY: &str = "# ---- The node generates every line below from its packages.toml \
                                and rewrites\n# ---- them whenever a package is installed or \
                                removed, so an edit here is lost.\n# ---- Entries of your own go \
                                above this line, where the node does not look.";

/// One package this node installed through its surface.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Package {
    /// The resource it is hosted as, which its clients name.
    pub resource: Keyhash,
    /// Who owns it: the identity whose horizon §11's membership gate is
    /// evaluated against.
    pub owner: Keyhash,
    /// The authority the resource is reached under, as its `host` header.
    pub authority: String,
    /// The image the host pulled, as the registry names it.
    pub image: String,
    /// Where it listens on the loopback this node and the package share.
    pub port: u16,
    /// Where the host mounted a block of the package's own inside its
    /// container, where it did (`infra-client-requirements.md` §9.3): the
    /// runner's `block` line on the start, kept so the node tells the
    /// instance that path as `rhtn-block` in place of its surface.
    pub block: Option<String>,
    /// **Its storage sealed against an image of the machine**
    /// (`infra-client-requirements.md` §9.4) [author, 2026-11-22]: chosen
    /// when its storage was provisioned and kept as long as that is.
    pub sealed: bool,
}

impl Package {
    /// **The container's name on the host**: derived from the resource and
    /// not chosen, so a second install of one package replaces it rather
    /// than running beside it, and a removal names the same thing an
    /// install made.
    ///
    /// Half the keyhash, because the runner bounds a name at thirty
    /// characters and prefixes its own: sixteen hex digits is more than
    /// enough to not collide among the packages one machine hosts, and
    /// the whole of it is in the hosting file beside this.
    pub fn name(resource: &Keyhash) -> String {
        hex(resource).chars().take(16).collect()
    }

    /// The arguments the host starts the image with.
    ///
    /// **What a socket-backed package is told at startup**: where to
    /// listen, and which resource it is answering as. Nothing else: the
    /// node tells a package what its host provides over the control path
    /// §3.3 already has (`rhtn_node::federation`'s relationships
    /// exchange), and a second channel for the same facts would be a
    /// second thing to keep in step.
    pub fn arguments(&self) -> Vec<String> {
        vec![
            "--listen".into(),
            format!("127.0.0.1:{}", self.port),
            "--resource".into(),
            hex(&self.resource),
        ]
    }
}

/// Where the node's record of its own installs is kept, beside the
/// hosting file it generates into.
pub fn record_at(hosting: &Path) -> PathBuf {
    beside(hosting, "packages.toml")
}

/// Where a relayed manifest is kept.
pub fn manifest_at(hosting: &Path, resource: &Keyhash) -> PathBuf {
    beside(hosting, "packages").join(format!("{}.manifest", hex(resource)))
}

fn beside(hosting: &Path, name: &str) -> PathBuf {
    match hosting.parent() {
        Some(d) if !d.as_os_str().is_empty() => d.join(name),
        _ => PathBuf::from(name),
    }
}

#[derive(serde::Deserialize, serde::Serialize)]
struct Record {
    #[serde(default)]
    package: Vec<Entry>,
}

#[derive(serde::Deserialize, serde::Serialize)]
#[serde(deny_unknown_fields)]
struct Entry {
    resource: String,
    owner: String,
    authority: String,
    image: String,
    port: u16,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    block: Option<String>,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    sealed: bool,
}

/// **Read what this node installed**, or nothing where it has installed
/// nothing: an absent record is the ordinary state of a node whose
/// operator has not reached for a package yet.
pub fn read(hosting: &Path) -> Result<Vec<Package>, String> {
    let path = record_at(hosting);
    let text = match std::fs::read_to_string(&path) {
        Ok(t) => t,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(format!("{}: {e}", path.display())),
    };
    let record: Record =
        toml::from_str(&text).map_err(|e| format!("{}: {}", path.display(), e.message()))?;
    let mut out = Vec::new();
    for e in &record.package {
        out.push(Package {
            resource: keyhash(&e.resource)
                .ok_or_else(|| format!("{}: `{}` is not a resource", path.display(), e.resource))?,
            owner: keyhash(&e.owner)
                .ok_or_else(|| format!("{}: `{}` is not an owner", path.display(), e.owner))?,
            authority: e.authority.clone(),
            image: e.image.clone(),
            port: e.port,
            block: e.block.clone(),
            sealed: e.sealed,
        });
    }
    Ok(out)
}

/// **Write the record, and generate the hosting file's lower half from
/// it.**
///
/// Both or neither, as a grant act is: the record is written first
/// because it is the authority, and a hosting file that could not be
/// written leaves the record where it was. The caller reloads afterwards
/// and puts both back where the reload refuses.
pub fn write(hosting: &Path, packages: &[Package]) -> Result<(), String> {
    let path = record_at(hosting);
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    }
    let record = Record {
        package: packages
            .iter()
            .map(|p| Entry {
                resource: hex(&p.resource),
                owner: hex(&p.owner),
                authority: p.authority.clone(),
                image: p.image.clone(),
                port: p.port,
                block: p.block.clone(),
                sealed: p.sealed,
            })
            .collect(),
    };
    let text = toml::to_string(&record).map_err(|e| format!("{}: {e}", path.display()))?;
    std::fs::write(&path, text).map_err(|e| format!("{}: {e}", path.display()))?;
    compose(hosting, packages)
}

/// **The hosting file as the two owners leave it**: the operator's lines
/// above [`SENTINEL`], and one entry per installed package below.
pub fn compose(hosting: &Path, packages: &[Package]) -> Result<(), String> {
    let related = read_relations(hosting)?;
    let held = std::fs::read_to_string(hosting).unwrap_or_default();
    let theirs = above(&held);
    let mut out = theirs.to_string();
    if !out.is_empty() && !out.ends_with('\n') {
        out.push('\n');
    }
    out.push_str(SENTINEL);
    out.push('\n');
    out.push_str(SENTINEL_WHY);
    out.push('\n');
    for p in packages {
        out.push_str(&entry(hosting, p));
        out.push_str(&relationships(&p.resource, &related));
    }
    std::fs::write(hosting, out).map_err(|e| format!("{}: {e}", hosting.display()))
}

/// The operator's own half of the file: everything before the sentinel,
/// or the whole of it where there is none.
pub fn above(held: &str) -> &str {
    match held.find(SENTINEL) {
        Some(i) => &held[..i],
        None => held,
    }
}

/// **How one installed package is related to another instance**
/// (`infra-client-requirements.md` §9.2: each "written in your hosting
/// configuration beside the resource").
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Related {
    /// The patron's instance; a resource has at most one.
    Patron,
    /// A subordinate's instance.
    Subordinate,
    /// A member of the named kin group.
    Kin(String),
}

impl Related {
    /// **What an act names it as**: `patron`, `subordinate`, or `kin`
    /// with the group's name beside it.
    pub fn named(relation: &str, group: &str) -> Result<Related, String> {
        match (relation, group.is_empty()) {
            ("patron", true) => Ok(Related::Patron),
            ("subordinate", true) => Ok(Related::Subordinate),
            // **checked here and not only on the re-read**, since the
            // name is written into the hosting file as a key
            ("kin", false)
                if group.len() <= rhtn_node::federation::MAX_KIN_NAME
                    && group.bytes().all(|c| {
                        c.is_ascii_lowercase() || c.is_ascii_digit() || c == b'_' || c == b'-'
                    }) =>
            {
                Ok(Related::Kin(group.to_string()))
            }
            ("kin", false) => Err(format!(
                "`{group}` is not a kin group name: [a-z0-9_-], 1 to {} bytes",
                rhtn_node::federation::MAX_KIN_NAME
            )),
            ("kin", true) => Err("a kin relationship names its group".into()),
            ("patron" | "subordinate", false) => {
                Err("only a kin relationship names a group".into())
            }
            (other, _) => Err(format!(
                "`{other}` is not a relationship: patron, subordinate or kin"
            )),
        }
    }
}

impl std::fmt::Display for Related {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Related::Patron => write!(f, "patron"),
            Related::Subordinate => write!(f, "subordinate"),
            Related::Kin(g) => write!(f, "kin {g}"),
        }
    }
}

/// **One federation relationship of a package this node installed**, set
/// by its operator's `relate` act.
///
/// **Kept apart from the package's own record**, as its grants are: a
/// removal and an install of one resource is how a package is upgraded,
/// and an upgrade that dropped whom it federates with would be a
/// federation its operator never withdrew going silent.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Relation {
    /// The installed package it belongs to.
    pub resource: Keyhash,
    /// How the two are related.
    pub related: Related,
    /// The other instance, by its resource keyhash.
    pub instance: Keyhash,
    /// **Where another node hosts it**: that node's federation listener
    /// and the raw public key it presents, as its operator gave them
    /// (`rhtn_node::federation::Location`). None where this node hosts it,
    /// or where nobody has said yet.
    pub at: Option<(std::net::SocketAddr, Keyhash)>,
}

/// Where the node keeps the relationships it was asked to set.
pub fn relations_at(hosting: &Path) -> PathBuf {
    beside(hosting, "relationships.toml")
}

#[derive(serde::Deserialize, serde::Serialize, Default)]
struct Relations {
    #[serde(default)]
    relation: Vec<RelationEntry>,
}

#[derive(serde::Deserialize, serde::Serialize)]
#[serde(deny_unknown_fields)]
struct RelationEntry {
    resource: String,
    relation: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    group: String,
    instance: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    address: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    presents: Option<String>,
}

/// **Read the relationships set on installed packages**, or none where
/// none were set.
pub fn read_relations(hosting: &Path) -> Result<Vec<Relation>, String> {
    let path = relations_at(hosting);
    let text = match std::fs::read_to_string(&path) {
        Ok(t) => t,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(format!("{}: {e}", path.display())),
    };
    let held: Relations =
        toml::from_str(&text).map_err(|e| format!("{}: {}", path.display(), e.message()))?;
    let mut out = Vec::new();
    for e in &held.relation {
        let bad = |what: &str| format!("{}: `{}` is not {what}", path.display(), e.resource);
        let at = match (&e.address, &e.presents) {
            (None, None) => None,
            (Some(a), Some(k)) => Some((
                a.parse()
                    .map_err(|_| bad("located at an address and port"))?,
                keyhash(k).ok_or_else(|| bad("located with a 64-hex key"))?,
            )),
            _ => return Err(bad("located with both an address and a key")),
        };
        out.push(Relation {
            resource: keyhash(&e.resource).ok_or_else(|| bad("a resource"))?,
            related: Related::named(&e.relation, &e.group)
                .map_err(|why| format!("{}: {why}", path.display()))?,
            instance: keyhash(&e.instance).ok_or_else(|| bad("related to an instance"))?,
            at,
        });
    }
    Ok(out)
}

/// **Write the relationships, and generate the hosting file's lower half
/// again** with them in it. The caller reloads afterwards and puts both
/// back where the reload refuses, as an install does.
pub fn write_relations(
    hosting: &Path,
    relations: &[Relation],
    packages: &[Package],
) -> Result<(), String> {
    let path = relations_at(hosting);
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    }
    let held = Relations {
        relation: relations
            .iter()
            .map(|r| {
                let (relation, group) = match &r.related {
                    Related::Patron => ("patron", String::new()),
                    Related::Subordinate => ("subordinate", String::new()),
                    Related::Kin(g) => ("kin", g.clone()),
                };
                RelationEntry {
                    resource: hex(&r.resource),
                    relation: relation.into(),
                    group,
                    instance: hex(&r.instance),
                    address: r.at.map(|(a, _)| a.to_string()),
                    presents: r.at.map(|(_, k)| hex(&k)),
                }
            })
            .collect(),
    };
    let text = toml::to_string(&held).map_err(|e| format!("{}: {e}", path.display()))?;
    std::fs::write(&path, text).map_err(|e| format!("{}: {e}", path.display()))?;
    compose(hosting, packages)
}

/// **One package's relationships, in the hosting file's own terms**
/// (`crate::hosting`): written after its entry's plain keys, since a
/// table opened beneath `[[host]]` takes every key that follows it.
///
/// One location an instance: where two relationships to it say where it
/// is, the first says it, and the act refuses a second that differs.
fn relationships(resource: &Keyhash, related: &[Relation]) -> String {
    let mine: Vec<&Relation> = related.iter().filter(|r| r.resource == *resource).collect();
    let mut out = String::new();
    if let Some(r) = mine.iter().find(|r| r.related == Related::Patron) {
        out.push_str(&format!("patron = \"{}\"\n", hex(&r.instance)));
    }
    let subordinates: Vec<String> = mine
        .iter()
        .filter(|r| r.related == Related::Subordinate)
        .map(|r| format!("\"{}\"", hex(&r.instance)))
        .collect();
    if !subordinates.is_empty() {
        out.push_str(&format!("subordinates = [{}]\n", subordinates.join(", ")));
    }
    let mut kin: std::collections::BTreeMap<&str, Vec<String>> = Default::default();
    for r in &mine {
        if let Related::Kin(g) = &r.related {
            kin.entry(g)
                .or_default()
                .push(format!("\"{}\"", hex(&r.instance)));
        }
    }
    if !kin.is_empty() {
        out.push_str("[host.kin]\n");
        for (g, members) in kin {
            out.push_str(&format!("{g} = [{}]\n", members.join(", ")));
        }
    }
    let mut at: std::collections::BTreeMap<String, (std::net::SocketAddr, Keyhash)> =
        Default::default();
    for r in &mine {
        if let Some(loc) = r.at {
            at.entry(hex(&r.instance)).or_insert(loc);
        }
    }
    if !at.is_empty() {
        out.push_str("[host.at]\n");
        for (i, (a, k)) in at {
            out.push_str(&format!(
                "\"{i}\" = {{ address = \"{a}\", key = \"{}\" }}\n",
                hex(&k)
            ));
        }
    }
    out
}

/// One generated `[[host]]` entry.
fn entry(hosting: &Path, p: &Package) -> String {
    let mut out = format!(
        "\n[[host]]\nresource = \"{}\"\nowner = \"{}\"\nauthority = \"{}\"\nmanifest = \"{}\"\naddress = \"127.0.0.1:{}\"\n",
        hex(&p.resource),
        hex(&p.owner),
        p.authority,
        manifest_at(hosting, &p.resource).display(),
        p.port
    );
    if let Some(b) = &p.block {
        out.push_str(&format!("block = \"{b}\"\n"));
    }
    if p.sealed {
        out.push_str("sealed = true\n");
    }
    out
}

/// **The icon beside the manifest at `manifest`**, where the package's
/// installation delivered one (`infra-client-requirements.md` §9.4)
/// [author, 2026-11-25]: a file named as the manifest is, with the
/// extension that says its type. The first of the three kinds found is
/// the icon; one whose size is outside the bound is refused by name, since
/// a file this node quietly passed over would be a package shown without
/// the icon its operator believes it has.
pub fn read_icon(manifest: &Path) -> Result<Option<rhtn_node::resources::Icon>, String> {
    for (media, ext) in rhtn_node::resources::ICON_KINDS {
        let at = manifest.with_extension(ext);
        match std::fs::read(&at) {
            Ok(bytes) => {
                return rhtn_node::resources::Icon::of(media, bytes)
                    .map(Some)
                    .map_err(|why| format!("{}: {why}", at.display()));
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(format!("{}: {e}", at.display())),
        }
    }
    Ok(None)
}

/// **Put `icon` beside the manifest at `manifest`, in the place of any
/// icon there**: none removes what was there, so an installation that
/// delivered no icon leaves the package with none rather than with its
/// predecessor's.
pub fn write_icon(
    manifest: &Path,
    icon: Option<&rhtn_node::resources::Icon>,
) -> Result<(), String> {
    for (_, ext) in rhtn_node::resources::ICON_KINDS {
        let at = manifest.with_extension(ext);
        match std::fs::remove_file(&at) {
            Ok(()) => {}
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(format!("{}: {e}", at.display())),
        }
    }
    let Some(icon) = icon else {
        return Ok(());
    };
    let ext = rhtn_node::resources::ICON_KINDS
        .iter()
        .find(|(m, _)| *m == icon.media)
        .map(|(_, e)| *e)
        .ok_or_else(|| format!("`{}` is not an icon's type", icon.media))?;
    let at = manifest.with_extension(ext);
    std::fs::write(&at, &icon.bytes).map_err(|e| format!("{}: {e}", at.display()))
}

/// **What the host said of a package's icon** (`packaging/package-runner.sh`,
/// `icon`): `icon <type> <bytes>` and then the file as one line of
/// base64url, or `icon none`. The icon where the reply carries one this
/// node will keep, and nothing where it carries none, or one whose bytes
/// are not the count it named, or the host's runner predates the verb.
#[must_use]
pub fn icon_said(reply: &str) -> Option<rhtn_node::resources::Icon> {
    let mut lines = reply.lines();
    let head = lines.find(|l| l.starts_with("icon "))?;
    let mut words = head.split_whitespace().skip(1);
    let media = words.next().filter(|m| *m != "none")?;
    let count: usize = words.next()?.parse().ok()?;
    let bytes = rhtn_node::federation::unbase64url(lines.next()?.trim())?;
    if bytes.len() != count {
        return None;
    }
    rhtn_node::resources::Icon::of(media, bytes).ok()
}

/// **What the host said of a block on the start** (`infra-client-requirements.md`
/// §9.3): the runner's first line is `block <path> <bytes>` where it
/// mounted one, `block none <why>` where it could not, and nothing of a
/// block where none was asked for or the runner is older than the block.
/// The path, where there is one, and the sentence the operator reads.
#[must_use]
pub fn block_said(reply: &str, asked: bool) -> (Option<String>, String) {
    if !asked {
        return (None, "none: the package declared no block".into());
    }
    for line in reply.lines() {
        let Some(rest) = line.strip_prefix("block ") else {
            continue;
        };
        if let Some(why) = rest.strip_prefix("none") {
            return (None, format!("the host's surface: {}", why.trim()));
        }
        let mut words = rest.split_whitespace();
        if let (Some(path), Some(bytes)) = (words.next(), words.next())
            && path.starts_with('/')
            && bytes.bytes().all(|b| b.is_ascii_digit())
        {
            return (
                Some(path.to_string()),
                format!("{path}, mounted by the host at {bytes} bytes"),
            );
        }
    }
    (
        None,
        "the host's surface: the host's runner said nothing of a block, so it is older \
         than the block (2026-11-15)"
            .into(),
    )
}

/// **An authority as a `host` header may carry one**: a name, in lower
/// case, with no port and no path. The resource sees it and
/// `resource-requirements.md` §3 has a target with more than one reading
/// refused; an authority with two readings is the same hazard one field
/// earlier.
pub fn authority(s: &str) -> Result<String, String> {
    if s.is_empty() || s.len() > 253 {
        return Err("an authority is between 1 and 253 characters".into());
    }
    if !s
        .bytes()
        .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'.' || b == b'-')
    {
        return Err(format!(
            "`{s}` is not an authority: lower-case letters, digits, dots and dashes, with no \
             port and no path"
        ));
    }
    if s.starts_with('.') || s.ends_with('.') || s.contains("..") {
        return Err(format!("`{s}` has an empty label"));
    }
    Ok(s.to_string())
}

/// **An image reference as a registry names one**, and nothing a shell or
/// the engine would read as a flag: the runner refuses the same, and this
/// is the refusal the operator is standing in front of.
pub fn image(s: &str) -> Result<String, String> {
    if s.is_empty() || s.len() > 512 {
        return Err("an image reference is between 1 and 512 characters".into());
    }
    if s.starts_with('-') {
        return Err("an image reference is not a flag".into());
    }
    if !s
        .bytes()
        .all(|b| b.is_ascii_alphanumeric() || b"./:@_-".contains(&b))
    {
        return Err(format!(
            "`{s}` is not an image reference: a registry path and a tag or digest"
        ));
    }
    // **by digest, never by tag** [author, 2026-11-15]: a tag names whatever
    // the registry holds under it today, and an install that pulled one
    // would have installed something nobody looked at.  The digest is what
    // a signature or a provenance record will pin to
    // (`infra-client-requirements.md` §9.1); until those exist it is the
    // one name an operator can check against what they were shown.
    let Some((path, digest)) = s.rsplit_once("@sha256:") else {
        return Err(format!(
            "`{s}` names no digest: install an image as `<registry path>@sha256:<64 hex>`, \
             the digest the publisher showed you, never a tag"
        ));
    };
    if path.is_empty() || path.contains('@') {
        return Err(format!(
            "`{s}` is not an image reference: a registry path before `@sha256:`"
        ));
    }
    if digest.len() != 64 || !digest.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err(format!("`{s}`: a digest is 64 hex digits after `@sha256:`"));
    }
    Ok(s.to_string())
}

fn hex(b: &[u8; 32]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect()
}

fn keyhash(s: &str) -> Option<Keyhash> {
    if s.len() != 64 {
        return None;
    }
    let mut out = [0u8; 32];
    for (i, b) in out.iter_mut().enumerate() {
        let pair = s.get(i * 2..i * 2 + 2)?;
        if pair.bytes().any(|c| c.is_ascii_uppercase()) {
            return None;
        }
        *b = u8::from_str_radix(pair, 16).ok()?;
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn kh(n: u8) -> Keyhash {
        [n; 32]
    }

    /// A directory of this run's own, as the daemon's tests make one.
    struct Dir(PathBuf);

    impl Dir {
        fn new(tag: &str) -> Dir {
            let at = std::env::temp_dir().join(format!(
                "rhtnd-packages-{tag}-{}-{:?}",
                std::process::id(),
                std::thread::current().id()
            ));
            let _ = std::fs::remove_dir_all(&at);
            std::fs::create_dir_all(&at).expect("a directory of this run's own");
            Dir(at)
        }

        fn path(&self) -> &Path {
            &self.0
        }
    }

    impl Drop for Dir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn one(n: u8, port: u16) -> Package {
        Package {
            resource: kh(n),
            owner: kh(1),
            authority: "testapp.internal".into(),
            image: "ggcr.io/mzuech/fuerosnet/rhtn-testapp:abc12345".into(),
            port,
            block: None,
            sealed: false,
        }
    }

    // functional: PKG-020
    #[test]
    fn the_hosts_block_line_is_read_and_a_mounted_block_goes_into_the_entry() {
        let (path, said) = block_said("block /rhtn-block 4096\nabc123\n", true);
        assert_eq!(path.as_deref(), Some("/rhtn-block"));
        assert!(said.contains("4096 bytes"), "{said}");
        let (path, said) = block_said("block none / is not mounted with prjquota\nabc\n", true);
        assert_eq!(path, None);
        assert!(said.contains("prjquota"), "{said}");
        let (path, said) = block_said("abc123\n", true);
        assert_eq!(path, None);
        assert!(said.contains("older"), "{said}");
        let (path, said) = block_said("block /rhtn-block 4096\n", false);
        assert_eq!(
            path, None,
            "a block the package did not declare is not taken"
        );
        assert!(said.starts_with("none"), "{said}");
        // and the entry carries the path, so the node tells it
        let dir = Dir::new("block-entry");
        let hosting = dir.path().join("hosting");
        let mut p = one(2, 7002);
        p.block = Some("/rhtn-block".into());
        write(&hosting, &[p.clone()]).unwrap();
        let text = std::fs::read_to_string(&hosting).unwrap();
        assert!(text.contains("block = \"/rhtn-block\"\n"), "{text}");
        assert_eq!(read(&hosting).unwrap(), vec![p]);
        let bare = one(3, 7003);
        write(&hosting, std::slice::from_ref(&bare)).unwrap();
        let text = std::fs::read_to_string(&hosting).unwrap();
        assert!(!text.contains("block ="), "{text}");
        assert_eq!(read(&hosting).unwrap(), vec![bare]);
    }

    // acceptance: DMN-38
    #[test]
    fn the_operators_own_lines_survive_every_install_and_removal() {
        let dir = Dir::new("operators-half");
        let hosting = dir.path().join("hosting");
        let theirs = "# mine, by hand\n[[host]]\nresource = \"aa\"\n";
        std::fs::write(&hosting, theirs).unwrap();

        write(&hosting, &[one(7, 7460)]).unwrap();
        let held = std::fs::read_to_string(&hosting).unwrap();
        assert!(held.starts_with(theirs), "the operator's half is untouched");
        assert!(held.contains(SENTINEL));
        assert!(held.contains(&format!("\"{}\"", hex(&kh(7)))));
        assert!(held.contains("address = \"127.0.0.1:7460\""));

        // a second install generates both and still leaves their half
        write(&hosting, &[one(7, 7460), one(8, 7461)]).unwrap();
        let held = std::fs::read_to_string(&hosting).unwrap();
        assert!(held.starts_with(theirs));
        assert_eq!(
            held.matches("[[host]]").count(),
            3,
            "theirs and two of ours"
        );
        assert_eq!(held.matches(SENTINEL).count(), 1, "one sentinel, ever");

        // and a removal of everything leaves their half and the sentinel
        write(&hosting, &[]).unwrap();
        let held = std::fs::read_to_string(&hosting).unwrap();
        assert!(held.starts_with(theirs));
        assert_eq!(held.matches("[[host]]").count(), 1, "theirs alone");
    }

    // acceptance: DMN-38
    #[test]
    fn what_was_installed_is_read_back_as_it_was_written() {
        let dir = Dir::new("read-back");
        let hosting = dir.path().join("hosting");
        assert!(read(&hosting).unwrap().is_empty(), "nothing installed yet");
        let these = vec![one(7, 7460), one(8, 7461)];
        write(&hosting, &these).unwrap();
        assert_eq!(read(&hosting).unwrap(), these);
    }

    // acceptance: DMN-38
    #[test]
    fn the_generated_half_is_a_hosting_file_the_node_reads() {
        let dir = Dir::new("generated-half");
        let hosting = dir.path().join("hosting");
        write(&hosting, &[one(7, 7460)]).unwrap();
        // the file `hosting::apply` parses, parsed: the sentinel is a
        // comment and the generated entries are tables
        let held = std::fs::read_to_string(&hosting).unwrap();
        let v: toml::Value = toml::from_str(&held).expect("the whole file is TOML");
        let hosts = v.get("host").and_then(|h| h.as_array()).expect("[[host]]");
        assert_eq!(hosts.len(), 1);
        assert_eq!(
            hosts[0].get("manifest").and_then(|m| m.as_str()).unwrap(),
            manifest_at(&hosting, &kh(7)).display().to_string()
        );
    }

    // functional: OPS-021
    #[test]
    fn a_relationship_set_on_an_installed_package_is_its_hosting_entrys_and_outlives_a_reinstall() {
        let dir = Dir::new("relations");
        let hosting = dir.path().join("hosting");
        let far: std::net::SocketAddr = "203.0.113.9:7452".parse().unwrap();
        let related = vec![
            Relation {
                resource: kh(7),
                related: Related::Patron,
                instance: kh(20),
                at: Some((far, kh(30))),
            },
            Relation {
                resource: kh(7),
                related: Related::Subordinate,
                instance: kh(21),
                at: None,
            },
            Relation {
                resource: kh(7),
                related: Related::Kin("guild".into()),
                instance: kh(22),
                at: None,
            },
            // a second relationship to one instance says where it is once
            Relation {
                resource: kh(7),
                related: Related::Kin("guild".into()),
                instance: kh(20),
                at: Some((far, kh(30))),
            },
            Relation {
                resource: kh(8),
                related: Related::Subordinate,
                instance: kh(23),
                at: None,
            },
        ];
        let both = [one(7, 7460), one(8, 7461)];
        write_relations(&hosting, &related, &both).unwrap();
        assert_eq!(read_relations(&hosting).unwrap(), related);
        // **read back by what the node reads the hosting file with**
        let rs = crate::hosting::relationships(&hosting).expect("the entries parse");
        let of = |n: u8| {
            rs.iter()
                .find(|(r, _)| *r == kh(n))
                .map(|(_, r)| r.clone())
                .unwrap()
        };
        assert_eq!(of(7).patron, Some(kh(20)));
        assert_eq!(
            of(7).subordinates.iter().copied().collect::<Vec<_>>(),
            vec![kh(21)]
        );
        assert_eq!(
            of(7).kin["guild"].iter().copied().collect::<Vec<_>>(),
            vec![kh(20), kh(22)]
        );
        assert_eq!(
            of(8).subordinates.iter().copied().collect::<Vec<_>>(),
            vec![kh(23)]
        );
        assert_eq!(of(8).patron, None);
        let at = crate::hosting::locations(&hosting).expect("the locations parse");
        assert_eq!(at.len(), 1, "{at:?}");
        assert_eq!(at[0].0, kh(20));
        assert_eq!((at[0].1.address, at[0].1.key), (far, kh(30)));
        // **a removal and an install keep them**, as they keep its grants
        write(&hosting, &[one(8, 7461)]).unwrap();
        assert!(
            crate::hosting::relationships(&hosting)
                .unwrap()
                .iter()
                .all(|(r, _)| *r != kh(7))
        );
        write(&hosting, &both).unwrap();
        let again = crate::hosting::relationships(&hosting).unwrap();
        assert_eq!(again, rs, "the reinstalled entry carries what was set");
    }

    #[test]
    fn a_relationship_is_named_as_the_hosting_file_names_it() {
        assert_eq!(Related::named("patron", ""), Ok(Related::Patron));
        assert_eq!(Related::named("subordinate", ""), Ok(Related::Subordinate));
        assert_eq!(
            Related::named("kin", "guild"),
            Ok(Related::Kin("guild".into()))
        );
        assert!(Related::named("kin", "").is_err());
        assert!(Related::named("patron", "guild").is_err());
        assert!(Related::named("kin", "Guild Hall").is_err());
        assert!(Related::named("kin", &"g".repeat(33)).is_err());
        assert!(Related::named("cousin", "").is_err());
    }

    // acceptance: DMN-38
    #[test]
    fn a_name_is_the_resources_and_fits_what_the_runner_takes() {
        let name = Package::name(&kh(0xab));
        assert_eq!(name, "abababababababab");
        assert_eq!(name.len(), 16);
        assert!(
            name.bytes()
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit())
        );
        // the runner's own bound, with its prefix
        assert!(format!("rhtn-pkg-{name}").len() <= 30 + "rhtn-pkg-".len());
        assert_ne!(name, Package::name(&kh(0xcd)));
    }

    // acceptance: DMN-38
    #[test]
    fn a_package_is_told_where_to_listen_and_which_resource_it_is() {
        let p = one(7, 7460);
        assert_eq!(
            p.arguments(),
            vec![
                "--listen".to_string(),
                "127.0.0.1:7460".to_string(),
                "--resource".to_string(),
                hex(&kh(7))
            ]
        );
    }

    // acceptance: DMN-38
    #[test]
    fn an_authority_and_an_image_are_what_they_say_they_are() {
        assert!(authority("testapp.internal").is_ok());
        assert!(authority("a").is_ok());
        for bad in [
            "",
            "Testapp.internal",
            "testapp.internal:80",
            "testapp/internal",
            ".testapp",
            "testapp.",
            "a..b",
            "test app",
        ] {
            assert!(authority(bad).is_err(), "`{bad}` is not an authority");
        }
        // functional: PKG-019
        let digest = "ab".repeat(32);
        assert!(
            image(&format!(
                "ggcr.io/mzuech/fuerosnet/rhtn-testapp@sha256:{digest}"
            ))
            .is_ok()
        );
        assert!(image(&format!("rhtn-testapp@sha256:{digest}")).is_ok());
        // **a tag is not an install** [author, 2026-11-15]: by digest alone
        for bad in [
            "ggcr.io/mzuech/fuerosnet/rhtn-testapp:abc12345",
            "rhtn-testapp:latest",
            "rhtn-testapp",
            "rhtn-testapp@sha256:00ff",
            "@sha256:0000000000000000000000000000000000000000000000000000000000000000",
        ] {
            assert!(image(bad).is_err(), "`{bad}` is not an install by digest");
        }
        for bad in ["", "-rm", "a;b", "a b", "a$b", "a|b"] {
            assert!(image(bad).is_err(), "`{bad}` is not an image reference");
        }
    }
}
