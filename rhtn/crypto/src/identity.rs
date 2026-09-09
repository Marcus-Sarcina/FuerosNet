use ed25519_dalek::{Signature as EdSig, Signer, Verifier, VerifyingKey as EdVk};
use ml_dsa::MlDsa65;
use ml_dsa::signature::Keypair as _;
use rhtn_codec::cose;

/// The public half: what a counterparty pins.
#[derive(Clone)]
pub struct Identity {
    pub keyhash: [u8; 32],
    pub ed: EdVk,
    pub pq: ml_dsa::VerifyingKey<MlDsa65>,
}

/// The private half.
pub struct SigningIdentity {
    pub public: Identity,
    ed_sk: ed25519_dalek::SigningKey,
    pq_sk: ml_dsa::ExpandedSigningKey<MlDsa65>,
}

impl Identity {
    pub fn from_public(ed: EdVk, pq: ml_dsa::VerifyingKey<MlDsa65>) -> Self {
        let keyhash = cose::keyhash(ed.as_bytes(), pq.encode().as_ref());
        Identity { keyhash, ed, pq }
    }

    /// The `KeyMaterial` array whose hash is this identity (`wire-format.md` §2.2).
    pub fn key_material(&self) -> Vec<u8> {
        cose::key_material(self.ed.as_bytes(), self.pq.encode().as_ref())
    }

    pub fn verify_ed(&self, sig: &[u8], tbs: &[u8]) -> bool {
        let Ok(s) = EdSig::from_slice(sig) else { return false };
        self.ed.verify(tbs, &s).is_ok()
    }

    pub fn verify_pq(&self, sig: &[u8], tbs: &[u8]) -> bool {
        let Ok(s) = ml_dsa::Signature::<MlDsa65>::try_from(sig) else { return false };
        self.pq.verify_with_context(tbs, &[], &s)
    }
}

impl SigningIdentity {
    /// From a 32-byte Ed25519 seed and a 32-byte ML-DSA seed (FIPS 204
    /// `KeyGen_internal`).
    pub fn from_seeds(ed_seed: &[u8; 32], pq_seed: &[u8; 32]) -> Self {
        let ed_sk = ed25519_dalek::SigningKey::from_bytes(ed_seed);
        let seed: ml_dsa::Seed = (*pq_seed).into();
        let pq_vk = ml_dsa::SigningKey::<MlDsa65>::from_seed(&seed).verifying_key();
        let pq_sk = ml_dsa::ExpandedSigningKey::<MlDsa65>::from_seed(&seed);
        let public = Identity::from_public(ed_sk.verifying_key(), pq_vk);
        SigningIdentity { public, ed_sk, pq_sk }
    }

    /// The classical signing key, for a transport that presents it as a raw
    /// public key (`wire-format.md` §9.1).
    pub fn ed_signing_key(&self) -> &ed25519_dalek::SigningKey {
        &self.ed_sk
    }

    pub fn sign_ed(&self, tbs: &[u8]) -> Vec<u8> {
        self.ed_sk.sign(tbs).to_bytes().to_vec()
    }

    /// Deterministic ML-DSA-65 with empty context (`wire-format.md` §2.2).
    pub fn sign_pq(&self, tbs: &[u8]) -> Vec<u8> {
        self.pq_sk
            .sign_deterministic(tbs, &[])
            .expect("empty context is always admissible")
            .encode()
            .to_vec()
    }

    /// One logical signer's two `COSE_Signature` entries over a `COSE_Sign`
    /// payload: classical then post-quantum, each `[protected, {}, sig]`.
    pub fn sign_entries(&self, aad: &[u8], payload: &[u8]) -> Vec<u8> {
        use rhtn_codec::encode::*;
        let mut out = Vec::new();
        for alg in [cose::ALG_EDDSA, cose::ALG_ML_DSA_65] {
            let prot = cose::protected_header(alg, &self.public.keyhash);
            let tbs = cose::sig_structure_sign(&prot, aad, payload);
            let sig = if alg == cose::ALG_EDDSA { self.sign_ed(&tbs) } else { self.sign_pq(&tbs) };
            emit_array_head(&mut out, 3);
            emit_bstr(&mut out, &prot);
            emit_map_head(&mut out, 0);
            emit_bstr(&mut out, &sig);
        }
        out
    }

    /// A classical `COSE_Sign1` `[protected, {}, null, sig]` over `payload`.
    pub fn sign1_ed(&self, aad: &[u8], payload: &[u8]) -> Vec<u8> {
        use rhtn_codec::encode::*;
        let prot = cose::protected_header(cose::ALG_EDDSA, &self.public.keyhash);
        let tbs = cose::sig_structure_sign1(&prot, aad, payload);
        let sig = self.sign_ed(&tbs);
        let mut out = Vec::new();
        emit_array_head(&mut out, 4);
        emit_bstr(&mut out, &prot);
        emit_map_head(&mut out, 0);
        emit_null(&mut out);
        emit_bstr(&mut out, &sig);
        out
    }
}

/// The test-vector identity recipe (`test-vectors/README.md`): seeds are
/// SHA-256 of a stated string per name.  Not for production keys.
pub mod testkit {
    use super::SigningIdentity;
    use rhtn_codec::cose::sha256;

    pub fn test_identity(name: &str) -> SigningIdentity {
        let ed_seed = sha256(format!("rhtn-test-vectors:{name}:ed25519-seed").as_bytes());
        let pq_seed = sha256(format!("rhtn-test-vectors:{name}:ml-dsa-65-seed").as_bytes());
        SigningIdentity::from_seeds(&ed_seed, &pq_seed)
    }
}
