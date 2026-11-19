//! **Custody is the desktop keyring** [author, 2026-11-18]: the Secret
//! Service (GNOME Keyring, KWallet), which holds a secret under the
//! person's login and opens it with their session.  Two secrets are kept
//! there and nothing else: the storage key the kernel seals its state
//! under (`light-client-requirements.md` §9), and the seed of the
//! transport key this desktop presents, which the kernel needs before it
//! can start and so cannot keep for itself.
//!
//! **No keyring, no start.**  A desktop with no Secret Service refuses to
//! start rather than keeping either secret in a file: failed custody is
//! never a quiet fall to plaintext, and this desktop declares itself
//! sealed.  The window says why, from [`Keyring::why`].
//!
//! **A key is never overwritten.**  The kernel mints a storage key where
//! [`Custody::key`] answers nothing, and a keyring that was locked or
//! unreachable when asked would otherwise have a fresh key written over
//! the one the state is sealed under.  So a key is kept only where the
//! keyring says plainly that none is held.

use rhtn_ffi::device::Custody;
use std::path::Path;
use std::sync::Mutex;
use zeroize::Zeroizing;

/// The service every entry is filed under.
const SERVICE: &str = "FuerosNet";

/// One desktop's secrets in the keyring, named by the directory its state
/// is kept in, so two installations under one login keep two sets.
pub struct Keyring {
    dir: String,
    why: Mutex<Option<String>>,
}

/// What the keyring answered for one secret.
enum Held {
    Some(Zeroizing<Vec<u8>>),
    None,
    Unreachable(String),
}

impl Keyring {
    /// The keyring's entries for the desktop whose state is under `dir`.
    pub fn new(dir: &Path) -> Keyring {
        Keyring {
            dir: dir.display().to_string(),
            why: Mutex::new(None),
        }
    }

    /// Why custody last failed, for the window to say.
    pub fn why(&self) -> Option<String> {
        self.why.lock().unwrap().clone()
    }

    fn entry(&self, name: &str) -> Result<keyring::Entry, String> {
        keyring::Entry::new(SERVICE, &format!("{name}@{}", self.dir)).map_err(|e| e.to_string())
    }

    fn read(&self, name: &str) -> Held {
        let entry = match self.entry(name) {
            Ok(e) => e,
            Err(e) => return Held::Unreachable(e),
        };
        match entry.get_secret() {
            Ok(b) => Held::Some(Zeroizing::new(b)),
            Err(keyring::Error::NoEntry) => Held::None,
            Err(e) => Held::Unreachable(e.to_string()),
        }
    }

    /// Keep `secret` under `name` where nothing is held there; false, and
    /// the reason kept for [`Self::why`], otherwise.
    fn keep_new(&self, name: &str, secret: &[u8]) -> bool {
        let refused = match self.read(name) {
            Held::Some(_) => Some(format!(
                "the keyring already holds a {name}; it is not replaced"
            )),
            Held::Unreachable(e) => Some(format!("the keyring could not be read: {e}")),
            Held::None => self
                .entry(name)
                .and_then(|e| e.set_secret(secret).map_err(|e| e.to_string()))
                .err()
                .map(|e| format!("the keyring would not keep the {name}: {e}")),
        };
        let ok = refused.is_none();
        *self.why.lock().unwrap() = refused;
        ok
    }

    /// **The transport seed this desktop presents its key from**: what a
    /// pairing drew and kept, or nothing before the first.  An error where
    /// the keyring cannot be read, which is not the same as no seed.
    pub fn seed(&self) -> Result<Option<Zeroizing<[u8; 32]>>, String> {
        match self.read("transport-seed") {
            Held::Some(b) => {
                let s: [u8; 32] = b
                    .as_slice()
                    .try_into()
                    .map_err(|_| "the kept transport seed is not 32 bytes".to_string())?;
                Ok(Some(Zeroizing::new(s)))
            }
            Held::None => Ok(None),
            Held::Unreachable(e) => Err(format!("the keyring could not be read: {e}")),
        }
    }

    /// Keep the transport seed a first pairing drew.
    pub fn keep_seed(&self, seed: &[u8; 32]) -> Result<(), String> {
        if self.keep_new("transport-seed", seed) {
            Ok(())
        } else {
            Err(self.why().unwrap_or_default())
        }
    }
}

impl Custody for Keyring {
    fn key(&self) -> Option<Vec<u8>> {
        match self.read("storage-key") {
            Held::Some(b) => Some(b.to_vec()),
            Held::None => None,
            Held::Unreachable(e) => {
                *self.why.lock().unwrap() = Some(format!("the keyring could not be read: {e}"));
                None
            }
        }
    }

    fn keep(&self, key: Vec<u8>) -> bool {
        let key = Zeroizing::new(key);
        self.keep_new("storage-key", &key)
    }

    fn unsealed(&self) -> bool {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Against this login's own keyring, so ignored by default: run with
    /// `cargo test -p rhtn-desktop --features gui -- --ignored custody`.
    /// Its entries are filed under a scratch directory and removed after.
    #[test]
    #[ignore = "writes to the login keyring"]
    fn custody_keeps_once_reads_back_and_never_overwrites() {
        let dir = std::env::temp_dir().join(format!("rhtn-custody-{}", std::process::id()));
        let k = Keyring::new(&dir);
        assert_eq!(k.key(), None, "nothing held yet");
        assert!(matches!(k.seed(), Ok(None)));
        assert!(k.keep(vec![7u8; 32]), "{:?}", k.why());
        assert_eq!(k.key(), Some(vec![7u8; 32]));
        assert!(!k.keep(vec![9u8; 32]), "a held key is not replaced");
        assert!(k.why().unwrap().contains("not replaced"));
        assert_eq!(k.key(), Some(vec![7u8; 32]));
        k.keep_seed(&[5u8; 32]).unwrap();
        assert_eq!(k.seed().unwrap().as_deref(), Some(&[5u8; 32]));
        for name in ["storage-key", "transport-seed"] {
            k.entry(name).unwrap().delete_credential().unwrap();
        }
        assert_eq!(k.key(), None, "removed");
    }
}
