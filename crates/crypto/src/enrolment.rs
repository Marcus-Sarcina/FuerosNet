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

// ---- the other direction: a client proving an act to its instance ------

/// The domain separation over an **act's** proof, distinct from
/// [`PROOF_INFO`] so a proof made in one direction can never serve the
/// other.
pub const ACT_INFO: &[u8] = b"rhtn/1:administration-act";

/// **The proof a client offers for a management act** [author,
/// 2026-11-02]: `HMAC-SHA256` under the instance's token over [`ACT_INFO`],
/// the caller's nonce, the method, the path and the body.
///
/// **Why the act and not the nonce alone.** A proof over a nonce would
/// carry to any request that arrived with it, so a captured one could be
/// replayed as a different act entirely. Binding the method, the path and
/// the body makes a proof worth exactly the act it was made for — which,
/// with a nonce the instance will not take twice, is the whole of what an
/// act needs to be authorised by something other than reaching the port.
///
/// **The token is still never sent**, as it is not in the other
/// direction: both parties know it because the operator's client wrote it
/// into the instance's configuration, and each proves knowledge of it.
#[must_use]
pub fn act_proof(
    token: &[u8; 32],
    nonce: &[u8],
    method: &str,
    path: &str,
    body: &[u8],
) -> [u8; 32] {
    let mut mac =
        <Hmac<Sha256> as KeyInit>::new_from_slice(token).expect("any length is a MAC key");
    mac.update(ACT_INFO);
    mac.update(&(nonce.len() as u32).to_be_bytes());
    mac.update(nonce);
    mac.update(&(method.len() as u32).to_be_bytes());
    mac.update(method.as_bytes());
    mac.update(&(path.len() as u32).to_be_bytes());
    mac.update(path.as_bytes());
    mac.update(&(body.len() as u32).to_be_bytes());
    mac.update(body);
    let mut out = [0u8; 32];
    out.copy_from_slice(&mac.finalize().into_bytes());
    out
}

/// Whether `shown` is the proof this token and nonce make over this act.
///
/// **Compared in constant time**, for the reason [`checks`] is.
#[must_use]
pub fn act_checks(
    token: &[u8; 32],
    nonce: &[u8],
    method: &str,
    path: &str,
    body: &[u8],
    shown: &[u8],
) -> bool {
    let mut mac =
        <Hmac<Sha256> as KeyInit>::new_from_slice(token).expect("any length is a MAC key");
    mac.update(ACT_INFO);
    mac.update(&(nonce.len() as u32).to_be_bytes());
    mac.update(nonce);
    mac.update(&(method.len() as u32).to_be_bytes());
    mac.update(method.as_bytes());
    mac.update(&(path.len() as u32).to_be_bytes());
    mac.update(path.as_bytes());
    mac.update(&(body.len() as u32).to_be_bytes());
    mac.update(body);
    mac.verify_slice(shown).is_ok()
}

#[cfg(test)]
mod act_tests {
    use super::*;

    // acceptance: DMN-36
    #[test]
    fn a_proof_is_worth_the_one_act_it_was_made_for() {
        let token = [3u8; 32];
        let nonce = [9u8; 16];
        let body = b"resource=aa&roles=reader";
        let p = act_proof(&token, &nonce, "POST", "/node/grant", body);
        assert!(act_checks(&token, &nonce, "POST", "/node/grant", body, &p));

        // not another path, another method, another body, another nonce,
        // or another token
        assert!(!act_checks(&token, &nonce, "POST", "/node/stop", body, &p));
        assert!(!act_checks(&token, &nonce, "PUT", "/node/grant", body, &p));
        assert!(!act_checks(
            &token,
            &nonce,
            "POST",
            "/node/grant",
            b"resource=bb",
            &p
        ));
        assert!(!act_checks(
            &token,
            &[8u8; 16],
            "POST",
            "/node/grant",
            body,
            &p
        ));
        assert!(!act_checks(
            &[4u8; 32],
            &nonce,
            "POST",
            "/node/grant",
            body,
            &p
        ));

        // **and never the other direction's proof**: the two tags part
        // them, so an instance's answer cannot authorise an act
        let theirs = proof(&token, &nonce, &[7u8; 32]);
        assert!(!act_checks(
            &token, &nonce, "GET", "/node", &[7u8; 32], &theirs
        ));
        assert_ne!(ACT_INFO, PROOF_INFO);

        // the fields cannot be slid into one another: a length precedes
        // each, so these two acts do not share a proof
        let a = act_proof(&token, &nonce, "POST", "/node/ab", b"c");
        let b = act_proof(&token, &nonce, "POST", "/node/a", b"bc");
        assert_ne!(a, b);
    }
}
