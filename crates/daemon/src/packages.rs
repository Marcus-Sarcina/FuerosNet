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

/// One generated `[[host]]` entry.
fn entry(hosting: &Path, p: &Package) -> String {
    format!(
        "\n[[host]]\nresource = \"{}\"\nowner = \"{}\"\nauthority = \"{}\"\nmanifest = \"{}\"\naddress = \"127.0.0.1:{}\"\n",
        hex(&p.resource),
        hex(&p.owner),
        p.authority,
        manifest_at(hosting, &p.resource).display(),
        p.port
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
        }
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
        assert!(image("ggcr.io/mzuech/fuerosnet/rhtn-testapp:abc12345").is_ok());
        assert!(image("rhtn-testapp@sha256:00ff").is_ok());
        for bad in ["", "-rm", "a;b", "a b", "a$b", "a|b"] {
            assert!(image(bad).is_err(), "`{bad}` is not an image reference");
        }
    }
}
