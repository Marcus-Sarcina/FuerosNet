//! Who signs a standalone classical record (`wire-format.md` §7): the
//! identity's own classical member, or the transport key that identity
//! delegated (§8.2).  A subtree acknowledgement and a currency attestation
//! are topology state and not a person's act, so the key that
//! authenticates the node signs them [author, 2026-09-21]; an envelope is
//! never signed this way (design §10: the archive is the true key's).

use crate::SigningIdentity;
use ed25519_dalek::Signer as _;
use rhtn_codec::cose;
use rhtn_codec::encode::*;

/// A signer of a standalone classical `COSE_Sign1` record whose signer
/// field names `keyhash`.
pub trait Sign1 {
    /// The identity the record names as its signer.
    fn keyhash(&self) -> [u8; 32];
    /// A classical `COSE_Sign1` without a `kid` over `payload` under `aad`.
    fn sign1_ed_unnamed(&self, aad: &[u8], payload: &[u8]) -> Vec<u8>;
}

impl Sign1 for SigningIdentity {
    fn keyhash(&self) -> [u8; 32] {
        self.public.keyhash
    }
    fn sign1_ed_unnamed(&self, aad: &[u8], payload: &[u8]) -> Vec<u8> {
        SigningIdentity::sign1_ed_unnamed(self, aad, payload)
    }
}

/// A delegated transport key signing for the identity that delegated it:
/// what an instance holds in place of the seed (design §23.3).
pub struct DelegatedSigner {
    keyhash: [u8; 32],
    key: ed25519_dalek::SigningKey,
}

impl DelegatedSigner {
    pub fn new(keyhash: [u8; 32], key: ed25519_dalek::SigningKey) -> Self {
        DelegatedSigner { keyhash, key }
    }
    /// The transport key: what a delegation's field 1 names.
    pub fn public(&self) -> [u8; 32] {
        self.key.verifying_key().to_bytes()
    }
}

impl Sign1 for DelegatedSigner {
    fn keyhash(&self) -> [u8; 32] {
        self.keyhash
    }
    fn sign1_ed_unnamed(&self, aad: &[u8], payload: &[u8]) -> Vec<u8> {
        let prot = cose::protected_alg(cose::ALG_EDDSA);
        let tbs = cose::sig_structure_sign1(&prot, aad, payload);
        let sig = self.key.sign(&tbs).to_bytes().to_vec();
        let mut out = Vec::new();
        emit_array_head(&mut out, 4);
        emit_bstr(&mut out, &prot);
        emit_map_head(&mut out, 0);
        emit_null(&mut out);
        emit_bstr(&mut out, &sig);
        out
    }
}
