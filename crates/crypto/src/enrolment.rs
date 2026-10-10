//! **The proof an instance offers for the transport key it minted.**
//!
//! `infra-client-requirements.md` §8.2 has an operator reach their own
//! instance out of band, and the hazard there is substitution: something
//! else answering at that address would offer *its* key, and a run signed
//! over it would delegate this identity to a stranger. The enrolment token
//! is what rules that out — it is known to the operator who wrote the
//! configuration and to the instance that read it, and **it is used as a
//! MAC key and never sent**.
//!
//! **Here rather than in either party** [2026-10-10]: the instance proves
//! and the operator's client checks, so a copy on each side would be two
//! implementations of one MAC. The instance is `rhtn-daemon`, and the
//! client is a light client that does not carry the daemon at all —
//! §8.3 has the client ship the provisioning pages, which is where the
//! checking happens.

use hkdf::hmac::{Hmac, KeyInit, Mac};
use sha2::Sha256;

/// The domain separation over the proof: a tag of this project's own, so
/// the token cannot be made to authenticate anything else that happens to
/// hash the same bytes.
pub const PROOF_INFO: &[u8] = b"rhtn/1:enrolment-proof";

/// The nonce a fetch must carry, in bytes. Sixteen: the caller chooses it
/// per fetch, and the proof is worthless for any other.
pub const NONCE_BYTES: usize = 16;

/// `HMAC-SHA256` under the enrolment token over [`PROOF_INFO`], the
/// caller's nonce, and the key.
///
/// **The nonce is what makes it unreplayable** and the key is what it is
/// about: a proof over one key does not carry to another, which is the
/// substitution the token is here to stop.
#[must_use]
pub fn proof(token: &[u8; 32], nonce: &[u8], key: &[u8; 32]) -> [u8; 32] {
    let mut mac =
        <Hmac<Sha256> as KeyInit>::new_from_slice(token).expect("any length is a MAC key");
    mac.update(PROOF_INFO);
    mac.update(nonce);
    mac.update(key);
    let mut out = [0u8; 32];
    out.copy_from_slice(&mac.finalize().into_bytes());
    out
}

/// Whether a proof is the one this token and nonce produce over this key.
///
/// **Compared in constant time**, which is what the MAC's own verifier is
/// for: a byte-at-a-time comparison on a value an attacker can influence
/// is the one way a MAC check is ordinarily lost.
#[must_use]
pub fn checks(token: &[u8; 32], nonce: &[u8], key: &[u8; 32], shown: &[u8]) -> bool {
    let mut mac =
        <Hmac<Sha256> as KeyInit>::new_from_slice(token).expect("any length is a MAC key");
    mac.update(PROOF_INFO);
    mac.update(nonce);
    mac.update(key);
    mac.verify_slice(shown).is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// **The bytes, pinned against an implementation that is not ours.**
    ///
    /// This MAC moved from `aws-lc-rs` to RustCrypto when it moved here,
    /// so that the operator's client and the instance read one definition
    /// — and a MAC that changed its bytes in the move would have made
    /// every instance already configured unreachable, silently, with the
    /// proof simply failing to check. The vector was computed with
    /// Python's `hmac`/`hashlib` rather than by running either.
    #[test]
    fn the_proof_is_hmac_sha256_over_the_tag_the_nonce_and_the_key() {
        let token: [u8; 32] = core::array::from_fn(|i| i as u8);
        let nonce = [0xAAu8; 16];
        let key: [u8; 32] = core::array::from_fn(|i| 0x11 + (i % 7) as u8);
        let want = "5e8cd7915378f93855145f7584d3de662750fc502a9bf617e1a9f5536d001887";
        let got: String = proof(&token, &nonce, &key)
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect();
        assert_eq!(want, got, "HMAC-SHA256(token, tag || nonce || key)");
        assert!(checks(&token, &nonce, &key, &proof(&token, &nonce, &key)));
    }

    /// **A proof over one key does not carry to another**, which is the
    /// substitution the token is here to stop, and neither does one under
    /// another nonce.
    #[test]
    fn a_proof_is_about_one_key_under_one_nonce() {
        let token = [7u8; 32];
        let nonce = [1u8; 16];
        let key = [2u8; 32];
        let p = proof(&token, &nonce, &key);
        assert!(checks(&token, &nonce, &key, &p));
        assert!(!checks(&token, &nonce, &[3u8; 32], &p), "another key");
        assert!(!checks(&token, &[9u8; 16], &key, &p), "another nonce");
        assert!(!checks(&[8u8; 32], &nonce, &key, &p), "another token");
        assert!(!checks(&token, &nonce, &key, &p[..31]), "a short proof");
    }
}
