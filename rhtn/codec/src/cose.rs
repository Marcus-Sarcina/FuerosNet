//! COSE profile pieces that are pure encoding (§1, §1.1, §1.4, §2.2):
//! the `Sig_structure` a signature is computed over, the `KeyMaterial` array
//! an identity is hashed from, and content addressing.

use crate::encode::*;
use sha2::{Digest, Sha256};

pub fn sha256(b: &[u8]) -> [u8; 32] {
    Sha256::digest(b).into()
}

/// `txid = SHA-256(deterministic CBOR of the body map)` (§1.4).
pub fn txid(body: &[u8]) -> [u8; 32] {
    sha256(body)
}

/// RFC 9052 `Sig_structure` for `COSE_Sign` (multi-signer, detached payload):
/// `["Signature", body_protected = h'', sign_protected, external_aad, payload]`.
pub fn sig_structure_sign(protected: &[u8], aad: &[u8], payload: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    emit_array_head(&mut out, 5);
    emit_tstr(&mut out, "Signature");
    emit_bstr(&mut out, b"");
    emit_bstr(&mut out, protected);
    emit_bstr(&mut out, aad);
    emit_bstr(&mut out, payload);
    out
}

/// RFC 9052 `Sig_structure` for `COSE_Sign1`:
/// `["Signature1", protected, external_aad, payload]`.
pub fn sig_structure_sign1(protected: &[u8], aad: &[u8], payload: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    emit_array_head(&mut out, 4);
    emit_tstr(&mut out, "Signature1");
    emit_bstr(&mut out, protected);
    emit_bstr(&mut out, aad);
    emit_bstr(&mut out, payload);
    out
}

/// COSE algorithm identifiers this profile uses.
pub const ALG_EDDSA: i64 = -8;
pub const ALG_ML_DSA_65: i64 = -49;

/// A protected header `{1: alg, 4: kid}` as deterministic CBOR.
pub fn protected_header(alg: i64, kid: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    emit_map_head(&mut out, 2);
    emit_uint(&mut out, 1);
    emit_neg(&mut out, alg);
    emit_uint(&mut out, 4);
    emit_bstr(&mut out, kid);
    out
}

/// A protected header `{1: alg}` with no `kid`, for a signature whose
/// surrounding structure already names the signer (§3.5).
pub fn protected_alg(alg: i64) -> Vec<u8> {
    let mut out = Vec::new();
    emit_map_head(&mut out, 1);
    emit_uint(&mut out, 1);
    emit_neg(&mut out, alg);
    out
}

/// `KeyMaterial = [ COSE_Key(OKP, Ed25519), COSE_Key(AKP, ML-DSA-65) ]` (§2.2),
/// labels in the bytewise order the profile requires.
pub fn key_material(ed_pub: &[u8; 32], pq_pub: &[u8]) -> Vec<u8> {
    let mut km = Vec::new();
    emit_array_head(&mut km, 2);
    // classical COSE_Key: kty 1 (OKP), crv -1 = 6 (Ed25519), x -2
    emit_map_head(&mut km, 3);
    emit_uint(&mut km, 1);
    emit_uint(&mut km, 1);
    emit_neg(&mut km, -1);
    emit_uint(&mut km, 6);
    emit_neg(&mut km, -2);
    emit_bstr(&mut km, ed_pub);
    // post-quantum COSE_Key: kty 7 (AKP), alg 3 = -49, pub -1
    emit_map_head(&mut km, 3);
    emit_uint(&mut km, 1);
    emit_uint(&mut km, 7);
    emit_uint(&mut km, 3);
    emit_neg(&mut km, ALG_ML_DSA_65);
    emit_neg(&mut km, -1);
    emit_bstr(&mut km, pq_pub);
    km
}

/// The keyhash covers both components (design §5.1).
pub fn keyhash(ed_pub: &[u8; 32], pq_pub: &[u8]) -> [u8; 32] {
    sha256(&key_material(ed_pub, pq_pub))
}

/// The role tags of §1.1 (`external_aad`).
pub mod aad {
    pub const ENVELOPE: &[u8] = b"rhtn/1:envelope";
    pub const VERIFIER: &[u8] = b"rhtn/1:verifier";
    pub const CONSENT: &[u8] = b"rhtn/1:consent";
    pub const CURRENCY: &[u8] = b"rhtn/1:currency";
    pub const CATALOG: &[u8] = b"rhtn/1:catalog";
    pub const ABUSE: &[u8] = b"rhtn/1:abuse";
    pub const LOCATOR: &[u8] = b"rhtn/1:locator";
    pub const ANCHOR: &[u8] = b"rhtn/1:anchor";
    pub const ENDPOINTS: &[u8] = b"rhtn/1:endpoints";
    pub const PREKEY: &[u8] = b"rhtn/1:prekey";
    pub const SUBTREE_ACK: &[u8] = b"rhtn/1:subtree-ack";
    pub const SUCCESSOR: &[u8] = b"rhtn/1:successor";
    pub const TRANSFER: &[u8] = b"rhtn/1:transfer";
}
