//! COSE profile pieces that are pure encoding (§1, §1.1, §1.4, §2.2):
//! the `Sig_structure` a signature is computed over, the `KeyMaterial` array
//! an identity is hashed from, and content addressing.

use crate::encode::*;
use sha2::{Digest, Sha256};

/// SHA-256, the one hash this profile uses: content addressing (§1.4),
/// the keyhash over `KeyMaterial` (§2.2), and the derivations the design
/// names.  Here so that nothing else in the crate reaches for a digest.
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

/// The classical component's algorithm, EdDSA over Ed25519 (§2.2).
pub const ALG_EDDSA: i64 = -8;
/// The post-quantum component's algorithm, ML-DSA-65 (§2.2).  **Every
/// algorithm this profile admits is negative** (§2.2): EdDSA is −8 and
/// ML-DSA-44, -65 and -87 are −48, −49 and −50, so a field holding one
/// cannot be a `uint`.
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

/// An ML-DSA-65 public key's length in bytes (FIPS 204).
pub const ML_DSA_65_PUBLIC_BYTES: usize = 1952;

/// The shape `KeyMaterial` has and no other (`wire-format.md` §2.2):
/// exactly two keys, classical then post-quantum; the first exactly kty
/// OKP, crv Ed25519 and a 32-byte x; the second exactly kty AKP, alg
/// ML-DSA-65 and a public key of the algorithm's length.  Any other label,
/// order or size is a different encoding, and so a different identity or
/// none: a single key cannot represent one.
pub fn check_key_material(km: &[u8]) -> Result<(), crate::cbor::Error> {
    use crate::cbor::{Error, Item, parse_all};
    let item = parse_all(km)?;
    let Item::Array(a) = &item else {
        return Err(Error("key material not an array"));
    };
    if a.len() != 2 {
        return Err(Error("key material is exactly two keys"));
    }
    let (Item::Map(c), Item::Map(q)) = (&a[0], &a[1]) else {
        return Err(Error("a key is not a map"));
    };
    let width = |it: &Item| match it {
        Item::Bytes(r) => Some(r.len()),
        _ => None,
    };
    let classical = c.len() == 3
        && matches!((&c[0].0, &c[0].1), (Item::Uint(1), Item::Uint(1)))
        && matches!((&c[1].0, &c[1].1), (Item::Neg(-1), Item::Uint(6)))
        && matches!(&c[2].0, Item::Neg(-2))
        && width(&c[2].1) == Some(32);
    if !classical {
        return Err(Error(
            "the classical key is not Ed25519 with exactly kty, crv and x",
        ));
    }
    let post_quantum = q.len() == 3
        && matches!((&q[0].0, &q[0].1), (Item::Uint(1), Item::Uint(7)))
        && matches!(
            (&q[1].0, &q[1].1),
            (Item::Uint(3), Item::Neg(ALG_ML_DSA_65))
        )
        && matches!(&q[2].0, Item::Neg(-1))
        && width(&q[2].1) == Some(ML_DSA_65_PUBLIC_BYTES);
    if !post_quantum {
        return Err(Error(
            "the post-quantum key is not ML-DSA-65 with exactly kty, alg and pub",
        ));
    }
    Ok(())
}

/// The keyhash covers both components (design §5.1).
pub fn keyhash(ed_pub: &[u8; 32], pq_pub: &[u8]) -> [u8; 32] {
    sha256(&key_material(ed_pub, pq_pub))
}

/// The role tags of §1.1 (`external_aad`).
///
/// **One tag per role, and a signature made for one role cannot be read as
/// another**: the tag is covered by the signature, so moving a signed
/// structure into a context that expects a different tag makes it verify
/// as nothing.  §1.1 is the table these come from and is authoritative on
/// the set; a structure with no tag of its own does not get one invented
/// here.
pub mod aad {
    /// A transaction envelope (§1.1, §3).
    pub const ENVELOPE: &[u8] = b"rhtn/1:envelope";
    /// A verifier's response (§1.1, §4.5).
    pub const VERIFIER: &[u8] = b"rhtn/1:verifier";
    /// A subject's consent to a query about it (§1.1, §4.5).
    pub const CONSENT: &[u8] = b"rhtn/1:consent";
    /// A currency attestation (§1.1, §7.1).
    pub const CURRENCY: &[u8] = b"rhtn/1:currency";
    /// A catalog entry (§1.1, §6.1).
    pub const CATALOG: &[u8] = b"rhtn/1:catalog";
    /// An abuse report (§1.1, §6.3).
    pub const ABUSE: &[u8] = b"rhtn/1:abuse";
    /// A standalone locator (§1.1, §2.3).
    pub const LOCATOR: &[u8] = b"rhtn/1:locator";
    /// An anchor entry (§1.1, §7.2).
    pub const ANCHOR: &[u8] = b"rhtn/1:anchor";
    /// A node's endpoint record (§1.1, §7.6).
    pub const ENDPOINTS: &[u8] = b"rhtn/1:endpoints";
    /// A prekey bundle (§1.1, §7.8).
    pub const PREKEY: &[u8] = b"rhtn/1:prekey";
    /// A subtree acknowledgement (§1.1, §7.5).
    pub const SUBTREE_ACK: &[u8] = b"rhtn/1:subtree-ack";
    /// An old key's successor statement (§1.1, §4.1).
    pub const SUCCESSOR: &[u8] = b"rhtn/1:successor";
    /// A former patron's transfer statement (§1.1, §4.1).
    pub const TRANSFER: &[u8] = b"rhtn/1:transfer";
    /// The fourteenth: a transport delegation (`wire-format.md` §8.2).
    pub const DELEGATION: &[u8] = b"rhtn/1:delegation";
}
