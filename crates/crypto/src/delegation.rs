//! Issuing a transport delegation (`wire-format.md` §8.2): the ceremony
//! device signs, hybrid, over a transport key another device or an
//! instance minted, for a window of exactly 48 hours.  What the object
//! means and how a receiver checks it is `verify::delegation`; this is the
//! issuing side, which is the identity's own device (design §23.3).

use crate::SigningIdentity;
use rhtn_codec::cose::aad;
use rhtn_codec::encode::*;
use rhtn_codec::schema::DELEGATION_WINDOW_SECONDS;

/// A delegation naming `key` for the window opening at `not_before`,
/// signed by `signer` (field 2 is the signer's keyhash).
pub fn issue(signer: &SigningIdentity, key: &[u8; 32], not_before: u64) -> Vec<u8> {
    issue_with_window(
        signer,
        key,
        not_before,
        not_before + DELEGATION_WINDOW_SECONDS,
    )
}

/// The run an operator's client signs for an instance
/// (`infra-client-requirements.md` §7): `count` credentials, contiguous,
/// each `not_before` the previous `not_after`, the first opening at
/// `start`.
pub fn issue_run(
    signer: &SigningIdentity,
    key: &[u8; 32],
    start: u64,
    count: usize,
) -> Vec<Vec<u8>> {
    (0..count as u64)
        .map(|i| issue(signer, key, start + i * DELEGATION_WINDOW_SECONDS))
        .collect()
}

/// A delegation with an arbitrary window: what a test needs to show that
/// a receiver refuses one not exactly 172,800 seconds long.  Not for
/// issuing.
pub fn issue_with_window(
    signer: &SigningIdentity,
    key: &[u8; 32],
    not_before: u64,
    not_after: u64,
) -> Vec<u8> {
    let payload = payload(key, &signer.public.keyhash, not_before, not_after);
    let mut block = Vec::new();
    emit_array_head(&mut block, 4);
    emit_bstr(&mut block, b"");
    emit_map_head(&mut block, 0);
    emit_null(&mut block);
    emit_array_head(&mut block, 2);
    block.extend_from_slice(&signer.sign_entries_unnamed(aad::DELEGATION, &payload));
    with_block(&payload, &block)
}

/// A delegation carrying the classical signature alone: malformed by
/// `wire-format.md` §8.2, and what a test presents to show it refused.
pub fn issue_classical_only(signer: &SigningIdentity, key: &[u8; 32], not_before: u64) -> Vec<u8> {
    use rhtn_codec::cose;
    let payload = payload(
        key,
        &signer.public.keyhash,
        not_before,
        not_before + DELEGATION_WINDOW_SECONDS,
    );
    let prot = cose::protected_alg(cose::ALG_EDDSA);
    let tbs = cose::sig_structure_sign(&prot, aad::DELEGATION, &payload);
    let sig = signer.sign_ed(&tbs);
    let mut block = Vec::new();
    emit_array_head(&mut block, 4);
    emit_bstr(&mut block, b"");
    emit_map_head(&mut block, 0);
    emit_null(&mut block);
    emit_array_head(&mut block, 1);
    emit_array_head(&mut block, 3);
    emit_bstr(&mut block, &prot);
    emit_map_head(&mut block, 0);
    emit_bstr(&mut block, &sig);
    with_block(&payload, &block)
}

/// Fields 1 to 4 as the map the signature covers.
fn payload(key: &[u8; 32], keyhash: &[u8; 32], not_before: u64, not_after: u64) -> Vec<u8> {
    let mut out = Vec::new();
    emit_map_head(&mut out, 4);
    emit_uint(&mut out, 1);
    emit_bstr(&mut out, key);
    emit_uint(&mut out, 2);
    emit_bstr(&mut out, keyhash);
    emit_uint(&mut out, 3);
    emit_uint(&mut out, not_before);
    emit_uint(&mut out, 4);
    emit_uint(&mut out, not_after);
    out
}

/// The five-field map: the payload's entries with field 5 appended.
fn with_block(payload: &[u8], block: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    emit_map_head(&mut out, 5);
    // the payload is a four-entry map; its head is one byte (0xa4)
    out.extend_from_slice(&payload[1..]);
    emit_uint(&mut out, 5);
    out.extend_from_slice(block);
    out
}
