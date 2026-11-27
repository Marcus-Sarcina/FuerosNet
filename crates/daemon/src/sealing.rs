//! **A package's storage sealed against an image of its machine**
//! [author, 2026-11-22] (`infra-client-requirements.md` §9.4): one
//! directory per sealed package under the host's own filesystem
//! encryption, the key handed by the operator's client with the act and
//! given to the host's kernel, never to the disk.
//!
//! **Why the kernel and not this process**: the kernel keeps the key for as
//! long as the machine runs, so a restart of this node leaves the package
//! open and a restart of the machine closes it, which is the line an image
//! draws. Adding a key for a directory of one's own is an ordinary user's
//! act on an ext4 filesystem set up to allow it, so the node holds no
//! privilege for it.
//!
//! **Where everything the package keeps goes.** The block, the log, the
//! jobs and the federation plane stay at the paths the rest of the node
//! already uses, and each is a link into the sealed directory: while the
//! package is closed every link leads nowhere, and nothing else in the node
//! has to know.

use rhtn_archive::Keyhash;
use std::path::{Path, PathBuf};

/// The key's length, as the client derives it (`rhtn_crypto::package_seal`).
pub const KEY_LEN: usize = 64;

/// The file whose presence says the directory is open: with the key
/// absent, a plaintext name finds nothing.
const OPEN_MARK: &str = "open";

/// The sealed directory for one resource, under the node's state.
pub fn dir(state: &Path, resource: &Keyhash) -> PathBuf {
    state.join("sealed").join(hex(resource))
}

/// Whether this resource's storage was sealed when it was provisioned.
pub fn is_sealed(state: &Path, resource: &Keyhash) -> bool {
    dir(state, resource).is_dir()
}

/// Whether its key is in the kernel, so the package may be served.
pub fn is_open(state: &Path, resource: &Keyhash) -> bool {
    dir(state, resource).join(OPEN_MARK).is_file()
}

/// **Seal a resource's storage at installation**: an empty directory, the
/// key added and a policy under it set, and the mark written inside it.
/// A host whose filesystem cannot seal says so, which is ordinary
/// capacity and not a fault.
pub fn seal(state: &Path, resource: &Keyhash, key: &[u8; KEY_LEN]) -> Result<(), String> {
    let at = dir(state, resource);
    if at.exists() {
        return Err(format!("{} is already sealed", at.display()));
    }
    for plain in plain_paths(state, resource) {
        if std::fs::symlink_metadata(&plain).is_ok() {
            return Err(format!(
                "{} was provisioned unsealed, and storage is sealed when it is provisioned: \
                 destroy it to install the package sealed",
                plain.display()
            ));
        }
    }
    let root = at.parent().expect("under sealed/");
    std::fs::create_dir_all(root).map_err(|e| format!("{}: {e}", root.display()))?;
    std::fs::create_dir(&at).map_err(|e| format!("{}: {e}", at.display()))?;
    let sealed = sys::add_key(root, key).and_then(|id| sys::set_policy(&at, &id));
    if let Err(why) = sealed {
        let _ = std::fs::remove_dir(&at);
        return Err(format!(
            "this host's filesystem cannot seal {}: {why}",
            at.display()
        ));
    }
    std::fs::write(at.join(OPEN_MARK), b"").map_err(|e| format!("{}: {e}", at.display()))
}

/// **Reopen a closed resource with the key its operator brought**: added,
/// and taken back out where it is not the directory's own, so a wrong key
/// changes nothing.
pub fn open(state: &Path, resource: &Keyhash, key: &[u8; KEY_LEN]) -> Result<(), String> {
    let at = dir(state, resource);
    if !at.is_dir() {
        return Err(format!("{} is not sealed here", hex(resource)));
    }
    let root = at.parent().expect("under sealed/");
    let want = sys::policy_of(&at).map_err(|e| format!("{}: {e}", at.display()))?;
    let got = sys::add_key(root, key).map_err(|e| format!("{}: {e}", root.display()))?;
    if got != want {
        let _ = sys::remove_key(root, &got);
        return Err("the key brought is not this package's".into());
    }
    if !at.join(OPEN_MARK).is_file() {
        return Err(format!(
            "{} has its key and no mark: it was not sealed here",
            at.display()
        ));
    }
    Ok(())
}

/// **The sealed packages that are closed**, as the node tells its own
/// operator at each attach (`wire-format.md` §8.2 field 8): installed
/// sealed, and either never given their key or closed by the machine's
/// restart.
pub fn closed(state: &Path, hosting: &Path) -> Vec<Keyhash> {
    crate::packages::read(hosting)
        .unwrap_or_default()
        .into_iter()
        .filter(|p| p.sealed && !is_open(state, &p.resource))
        .map(|p| p.resource)
        .collect()
}

/// **Take a key the operator brought on their session** for a package
/// installed sealed: its storage sealed under it where it never was, and
/// opened where the machine's restart closed it. A resource not installed
/// sealed is refused, so nothing is sealed that the operator did not
/// choose to.
pub fn take(
    state: &Path,
    hosting: &Path,
    resource: &Keyhash,
    key: &[u8; KEY_LEN],
) -> Result<(), String> {
    let installed = crate::packages::read(hosting)?
        .into_iter()
        .any(|p| p.sealed && p.resource == *resource);
    if !installed {
        return Err(format!("{} is not installed sealed here", hex(resource)));
    }
    if !is_sealed(state, resource) {
        seal(state, resource, key)
    } else if !is_open(state, resource) {
        open(state, resource, key)
    } else {
        Ok(())
    }
}

/// **Link the paths the node keeps for an open sealed resource** into its
/// directory, creating what is missing: the surface block, the log, the
/// jobs and, where the node keeps custody, the federation plane.
pub fn link(state: &Path, federation: Option<&Path>, resource: &Keyhash) -> Result<(), String> {
    let at = dir(state, resource);
    // the directories made inside first, since a link to nothing is one
    // nothing can be created through; the log is a file its writer makes
    let mut pairs: Vec<(PathBuf, PathBuf, bool)> = vec![
        (
            state.join("blocks").join(hex(resource)),
            at.join("block"),
            true,
        ),
        (
            state.join("logs").join(format!("{}.log", hex(resource))),
            at.join("log"),
            false,
        ),
        (
            state.join("jobs").join(hex(resource)),
            at.join("jobs"),
            true,
        ),
    ];
    if let Some(f) = federation {
        pairs.push((f.join(hex(resource)), at.join("federation"), true));
    }
    for (path, target, directory) in pairs {
        if directory {
            std::fs::create_dir_all(&target).map_err(|e| format!("{}: {e}", target.display()))?;
        }
        match std::fs::read_link(&path) {
            Ok(t) if t == target => continue,
            Ok(t) => {
                return Err(format!(
                    "{} leads to {}, not {}",
                    path.display(),
                    t.display(),
                    target.display()
                ));
            }
            Err(_) if std::fs::symlink_metadata(&path).is_ok() => {
                return Err(format!(
                    "{} is unsealed storage of a sealed package",
                    path.display()
                ));
            }
            Err(_) => {}
        }
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| format!("{}: {e}", parent.display()))?;
        }
        std::os::unix::fs::symlink(&target, &path)
            .map_err(|e| format!("{}: {e}", path.display()))?;
    }
    Ok(())
}

/// **Storage a resource already has unsealed**, where any: the first path
/// it occupies, which sealing it now would have to go over.
pub fn unsealed_storage(state: &Path, resource: &Keyhash) -> Option<PathBuf> {
    plain_paths(state, resource)
        .into_iter()
        .find(|p| std::fs::symlink_metadata(p).is_ok() && std::fs::read_link(p).is_err())
}

/// The paths an unsealed resource's storage would occupy.
fn plain_paths(state: &Path, resource: &Keyhash) -> [PathBuf; 3] {
    [
        state.join("blocks").join(hex(resource)),
        state.join("logs").join(format!("{}.log", hex(resource))),
        state.join("jobs").join(hex(resource)),
    ]
}

fn hex(k: &Keyhash) -> String {
    k.iter().map(|b| format!("{b:02x}")).collect()
}

/// **The kernel's three requests** (`linux/fscrypt.h`, version 2
/// policies): add a key and learn its identifier, set a directory's
/// policy under one, read a directory's, and remove a key again.
#[cfg(target_os = "linux")]
mod sys {
    use std::os::fd::AsRawFd;
    use std::path::Path;

    const SET_POLICY: libc::c_ulong = 0x800c_6613;
    const GET_POLICY_EX: libc::c_ulong = 0xc009_6616;
    const ADD_KEY: libc::c_ulong = 0xc050_6617;
    const REMOVE_KEY: libc::c_ulong = 0xc040_6618;
    const SPEC_IDENTIFIER: u32 = 2;
    /// AES-256-XTS for contents and AES-256-CTS for names, padded to 16.
    const POLICY: [u8; 4] = [2, 1, 4, 2];

    fn ioctl(at: &Path, request: libc::c_ulong, arg: &mut [u8]) -> Result<(), String> {
        let f = std::fs::File::open(at).map_err(|e| e.to_string())?;
        // SAFETY: `arg` is the request's own structure, sized for it, and
        // lives across the call
        let r = unsafe { libc::ioctl(f.as_raw_fd(), request as _, arg.as_mut_ptr()) };
        if r < 0 {
            return Err(std::io::Error::last_os_error().to_string());
        }
        Ok(())
    }

    pub fn add_key(fs: &Path, key: &[u8; super::KEY_LEN]) -> Result<[u8; 16], String> {
        let mut arg = zeroize::Zeroizing::new(vec![0u8; 80 + key.len()]);
        arg[0..4].copy_from_slice(&SPEC_IDENTIFIER.to_ne_bytes());
        arg[40..44].copy_from_slice(&(key.len() as u32).to_ne_bytes());
        arg[80..].copy_from_slice(key);
        ioctl(fs, ADD_KEY, &mut arg)?;
        Ok(arg[8..24].try_into().expect("sixteen bytes"))
    }

    pub fn remove_key(fs: &Path, id: &[u8; 16]) -> Result<(), String> {
        let mut arg = [0u8; 64];
        arg[0..4].copy_from_slice(&SPEC_IDENTIFIER.to_ne_bytes());
        arg[8..24].copy_from_slice(id);
        ioctl(fs, REMOVE_KEY, &mut arg)
    }

    pub fn set_policy(dir: &Path, id: &[u8; 16]) -> Result<(), String> {
        let mut arg = [0u8; 24];
        arg[..4].copy_from_slice(&POLICY);
        arg[8..].copy_from_slice(id);
        ioctl(dir, SET_POLICY, &mut arg)
    }

    pub fn policy_of(dir: &Path) -> Result<[u8; 16], String> {
        let mut arg = [0u8; 32];
        arg[..8].copy_from_slice(&24u64.to_ne_bytes());
        ioctl(dir, GET_POLICY_EX, &mut arg)?;
        if arg[8] != 2 {
            return Err("its policy is not one this node sets".into());
        }
        Ok(arg[16..32].try_into().expect("sixteen bytes"))
    }
}

/// Elsewhere no filesystem here can seal, which is ordinary capacity.
#[cfg(not(target_os = "linux"))]
mod sys {
    use std::path::Path;
    const WHY: &str = "only Linux's filesystem encryption is used";
    pub fn add_key(_: &Path, _: &[u8; super::KEY_LEN]) -> Result<[u8; 16], String> {
        Err(WHY.into())
    }
    pub fn remove_key(_: &Path, _: &[u8; 16]) -> Result<(), String> {
        Err(WHY.into())
    }
    pub fn set_policy(_: &Path, _: &[u8; 16]) -> Result<(), String> {
        Err(WHY.into())
    }
    pub fn policy_of(_: &Path) -> Result<[u8; 16], String> {
        Err(WHY.into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Dir(PathBuf);
    impl Dir {
        fn new(tag: &str) -> Dir {
            let d = std::env::temp_dir().join(format!("rhtn-sealing-{tag}-{}", std::process::id()));
            let _ = std::fs::remove_dir_all(&d);
            std::fs::create_dir_all(&d).expect("a directory");
            Dir(d)
        }
    }
    impl Drop for Dir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn a_directory_is_open_only_while_its_mark_is_found() {
        let d = Dir::new("mark");
        let r = [7u8; 32];
        assert!(!is_sealed(&d.0, &r) && !is_open(&d.0, &r));
        std::fs::create_dir_all(dir(&d.0, &r)).expect("a directory");
        assert!(
            is_sealed(&d.0, &r) && !is_open(&d.0, &r),
            "a closed one lists no names"
        );
        std::fs::write(dir(&d.0, &r).join(OPEN_MARK), b"").expect("the mark");
        assert!(is_open(&d.0, &r));
    }

    #[test]
    fn a_link_is_refused_over_unsealed_storage_and_taken_where_it_leads_home() {
        let d = Dir::new("link");
        let r = [7u8; 32];
        std::fs::create_dir_all(dir(&d.0, &r)).expect("a directory");
        link(&d.0, None, &r).expect("linked");
        link(&d.0, None, &r).expect("and again, finding its own");
        let e = Dir::new("link-plain");
        std::fs::create_dir_all(dir(&e.0, &r)).expect("a directory");
        std::fs::create_dir_all(e.0.join("jobs").join(hex(&r))).expect("plain jobs");
        assert!(
            link(&e.0, None, &r)
                .expect_err("refused")
                .contains("unsealed storage")
        );
    }

    #[test]
    fn sealing_over_storage_provisioned_unsealed_is_refused_and_leaves_nothing() {
        let d = Dir::new("over");
        let r = [7u8; 32];
        std::fs::create_dir_all(d.0.join("blocks").join(hex(&r))).expect("a plain block");
        let why = seal(&d.0, &r, &[1; KEY_LEN]).expect_err("refused");
        assert!(why.contains("provisioned unsealed"), "{why}");
        assert!(!dir(&d.0, &r).exists());
    }

    fn hosting(d: &Dir, sealed: bool, r: &Keyhash) -> PathBuf {
        let h = d.0.join("hosting");
        let p = crate::packages::Package {
            resource: *r,
            owner: [1; 32],
            authority: "shop.internal".into(),
            name: None,
            image: "reg.example/x/pkg:abc".into(),
            port: 9,
            block: None,
            sealed,
        };
        crate::packages::write(&h, &[p]).expect("written");
        h
    }

    // functional: OPS-033
    #[test]
    fn only_a_package_installed_sealed_is_told_closed_or_given_a_key() {
        let d = Dir::new("closed");
        let r = [7u8; 32];
        let plain = hosting(&d, false, &r);
        assert!(
            closed(&d.0, &plain).is_empty(),
            "an unsealed package is never closed"
        );
        let why = take(&d.0, &plain, &r, &[1; KEY_LEN]).expect_err("refused");
        assert!(why.contains("not installed sealed"), "{why}");
        assert!(
            !dir(&d.0, &r).exists(),
            "nothing sealed the operator did not choose"
        );

        let sealed = hosting(&d, true, &r);
        assert_eq!(closed(&d.0, &sealed), vec![r], "closed until its key comes");
        std::fs::create_dir_all(dir(&d.0, &r)).expect("a directory");
        std::fs::write(dir(&d.0, &r).join(OPEN_MARK), b"").expect("the mark");
        assert!(closed(&d.0, &sealed).is_empty(), "and not once it is open");
        take(&d.0, &sealed, &r, &[1; KEY_LEN]).expect("an open one takes nothing more");
    }

    /// **The kernel's half**, where the filesystem under the temporary
    /// directory has ext4's `encrypt` feature: sealed, closed by removing
    /// the key, and a wrong key refused and taken back out.
    #[test]
    #[ignore = "needs ext4 with the encrypt feature under the temporary directory"]
    fn the_kernel_seals_closes_and_reopens_a_directory() {
        let d = Dir::new("kernel");
        let r = [7u8; 32];
        let key = [5u8; KEY_LEN];
        seal(&d.0, &r, &key).expect("sealed");
        std::fs::write(dir(&d.0, &r).join("plain"), b"inside").expect("written inside");
        let root = d.0.join("sealed");
        let id = sys::policy_of(&dir(&d.0, &r)).expect("its policy");
        sys::remove_key(&root, &id).expect("closed");
        assert!(!is_open(&d.0, &r), "no name is found while it is closed");
        assert!(open(&d.0, &r, &[6u8; KEY_LEN]).is_err(), "not its key");
        open(&d.0, &r, &key).expect("reopened");
        assert_eq!(
            std::fs::read(dir(&d.0, &r).join("plain")).expect("read"),
            b"inside"
        );
        sys::remove_key(&root, &id).expect("closed again");
    }
}
