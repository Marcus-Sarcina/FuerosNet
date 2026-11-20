//! HKDF-SHA-256 (RFC 5869), checked against its Appendix A in
//! `tests/primitives.rs`.

use hkdf::Hkdf;
use sha2::Sha256;

/// Extract under `salt` and expand under `info` into `out`.  An empty
/// salt is RFC 5869's default of a hash-length string of zeros.
pub fn hkdf_sha256(salt: &[u8], ikm: &[u8], info: &[u8], out: &mut [u8]) {
    Hkdf::<Sha256>::new(Some(salt), ikm)
        .expand(info, out)
        .expect("within HKDF's bound");
}
