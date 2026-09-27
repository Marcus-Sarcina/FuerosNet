//! What the kernel persists, sealed under the storage key the application
//! keeps (`light-client-requirements.md` §9).
//!
//! **One construction for every platform, so a shell carries custody and no
//! cryptography**: AES-256-GCM under a 32-byte key the kernel mints, a
//! random 96-bit nonce per write, and the storage *name* as the associated
//! data, so a blob lifted from one file refuses to open as another. The
//! header names the construction, which is how a reader tells sealed state
//! from state written before the kernel sealed — the older shape carries no
//! header and is read as it always was.

use aws_lc_rs::aead::{AES_256_GCM, Aad, LessSafeKey, Nonce, UnboundKey};
use aws_lc_rs::rand::{SecureRandom, SystemRandom};
use zeroize::Zeroize;

/// The leading bytes of a sealed blob. Not a deterministic-CBOR item, so
/// nothing the kernel wrote before the seam can begin with it.
pub const HEADER: &[u8] = b"rhtn/1:sealed";

/// Whether `bytes` carry the sealed header.
pub fn is_sealed(bytes: &[u8]) -> bool {
    bytes.starts_with(HEADER)
}

/// Seal `bytes` as the contents of `name`.
pub fn seal(key: &[u8; 32], name: &str, bytes: &[u8]) -> Vec<u8> {
    let mut nonce = [0u8; 12];
    SystemRandom::new()
        .fill(&mut nonce)
        .expect("the system's random source");
    let mut body = bytes.to_vec();
    aead(key)
        .seal_in_place_append_tag(
            Nonce::assume_unique_for_key(nonce),
            Aad::from(name.as_bytes()),
            &mut body,
        )
        .expect("sealing appends a tag");
    let mut out = Vec::with_capacity(HEADER.len() + nonce.len() + body.len());
    out.extend_from_slice(HEADER);
    out.extend_from_slice(&nonce);
    out.extend_from_slice(&body);
    out
}

/// Open what [`seal`] wrote under `name`. `None` is an authentication
/// failure — the wrong key, another name's contents, or a damaged blob —
/// and never a partial read.
pub fn open(key: &[u8; 32], name: &str, bytes: &[u8]) -> Option<Vec<u8>> {
    let rest = bytes.strip_prefix(HEADER)?;
    if rest.len() < 12 + 16 {
        return None;
    }
    let nonce: [u8; 12] = rest[..12].try_into().unwrap();
    let mut body = rest[12..].to_vec();
    let took = aead(key)
        .open_in_place(
            Nonce::assume_unique_for_key(nonce),
            Aad::from(name.as_bytes()),
            &mut body,
        )
        .ok()
        .map(|p| p.to_vec());
    // the in-place buffer holds the plaintext past the copy — key material
    // among it, so it is wiped rather than left in freed memory (§3)
    body.zeroize();
    took
}

fn aead(k: &[u8; 32]) -> LessSafeKey {
    LessSafeKey::new(UnboundKey::new(&AES_256_GCM, k).expect("a 32-byte key"))
}
