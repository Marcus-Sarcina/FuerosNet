//! **A package's storage, sealed against an image of its machine**
//! [author, 2026-11-22] (`infra-client-requirements.md` §9.4,
//! `light-client-requirements.md` §10): the key the operator's client
//! hands its instance with an install, and again with every reopening
//! after the machine restarts.
//!
//! **Derived from the signing seed and never stored** [author, 2026-11-22:
//! the phrase reopens the package]: a device restored from the phrase
//! derives the same key for the same resource, so losing the device that
//! sealed a package loses nothing, and the key is never kept to be found.

use crate::SigningIdentity;
use zeroize::Zeroizing;

/// The label the derivation expands under, the resource's keyhash after
/// it. **Pinned to the byte**: a client that derived it differently could
/// not reopen what it sealed.
pub const PACKAGE_SEAL_TAG: &[u8] = b"rhtn/1:package-at-rest";

/// The key's length: the 64 bytes a filesystem's AES-256-XTS contents key
/// and its filename key are derived from by the kernel.
pub const PACKAGE_SEAL_LEN: usize = 64;

/// **The key for one resource, from the identity's classical seed.**
/// HKDF-SHA256 under [`PACKAGE_SEAL_TAG`] and the resource's keyhash, one
/// way: the seed cannot be walked back from it, and one resource's key
/// opens no other's.
pub fn package_seal_key(
    ed_seed: &[u8; 32],
    resource: &[u8; 32],
) -> Zeroizing<[u8; PACKAGE_SEAL_LEN]> {
    let mut info = Vec::with_capacity(PACKAGE_SEAL_TAG.len() + resource.len());
    info.extend_from_slice(PACKAGE_SEAL_TAG);
    info.extend_from_slice(resource);
    let mut out = Zeroizing::new([0u8; PACKAGE_SEAL_LEN]);
    crate::kdf::hkdf_sha256(&[], ed_seed, &info, &mut out[..]);
    out
}

impl SigningIdentity {
    /// **This identity's key for one resource's sealed storage**, derived
    /// at every call and never stored.
    pub fn package_seal_key(&self, resource: &[u8; 32]) -> Zeroizing<[u8; PACKAGE_SEAL_LEN]> {
        package_seal_key(&self.ed_signing_key().to_bytes(), resource)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_same_seed_and_resource_derive_the_same_key_and_nothing_else_does() {
        let (a, b) = ([1u8; 32], [2u8; 32]);
        let (r, s) = ([3u8; 32], [4u8; 32]);
        assert_eq!(*package_seal_key(&a, &r), *package_seal_key(&a, &r));
        assert_ne!(*package_seal_key(&a, &r), *package_seal_key(&a, &s));
        assert_ne!(*package_seal_key(&a, &r), *package_seal_key(&b, &r));
        assert_ne!(
            package_seal_key(&a, &r)[..32],
            crate::ssh::host_control_seed(&a)[..]
        );
    }
}
