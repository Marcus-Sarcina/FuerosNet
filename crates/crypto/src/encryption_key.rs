//! Issuing an identity's encryption key (`wire-format.md` §2.5): the
//! X-Wing key a message to it is sealed to and the key that authenticates
//! what it seals, signed hybrid by the identity.  How a receiver checks
//! one is `verify::encryption_key`; this is the issuing side, the
//! identity's own device, which derives both keys from the root
//! ([`EncryptionSecret::from_identity_root`]).

use crate::SigningIdentity;
use crate::seal::EncryptionSecret;
use rhtn_codec::cose::aad;
use rhtn_codec::encode::*;

/// `signer`'s encryption key for `secret`: fields 1 to 3, and the hybrid
/// `COSE_Sign` over them as field 4.
pub fn issue(signer: &SigningIdentity, secret: &EncryptionSecret) -> Vec<u8> {
    let payload = payload(
        &signer.public.keyhash,
        secret.kem_public(),
        &secret.auth_public(),
    );
    let mut block = Vec::new();
    emit_array_head(&mut block, 4);
    emit_bstr(&mut block, b"");
    emit_map_head(&mut block, 0);
    emit_null(&mut block);
    emit_array_head(&mut block, 2);
    block.extend_from_slice(&signer.sign_entries_unnamed(aad::ENCRYPTION_KEY, &payload));
    let mut out = Vec::new();
    emit_map_head(&mut out, 4);
    // the payload is a three-entry map; its head is one byte (0xa3)
    out.extend_from_slice(&payload[1..]);
    emit_uint(&mut out, 4);
    out.extend_from_slice(&block);
    out
}

/// Fields 1 to 3 as the map the signature covers.
fn payload(keyhash: &[u8; 32], kem: &[u8], auth: &[u8; 32]) -> Vec<u8> {
    let mut out = Vec::new();
    emit_map_head(&mut out, 3);
    emit_uint(&mut out, 1);
    emit_bstr(&mut out, keyhash);
    emit_uint(&mut out, 2);
    emit_bstr(&mut out, kem);
    emit_uint(&mut out, 3);
    emit_bstr(&mut out, auth);
    out
}

/// `signer`'s own encryption key, both keys derived from its identity
/// root: what every device the phrase restores issues, to the byte.
pub fn own(signer: &SigningIdentity) -> Vec<u8> {
    issue(
        signer,
        &EncryptionSecret::from_identity_root(signer.identity_root()),
    )
}
