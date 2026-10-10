//! **The two records an operator signs about a node's reachability**: its
//! endpoint record (`wire-format.md` §7.6) and its anchor entry.
//!
//! **Here because the signer is a light client** [2026-10-10].
//! `infra-client-requirements.md` §4.4 is categorical that an instance
//! cannot mint either — "the signature on both is its operator's" — so a
//! change of address is something the operator's client signs, and §8.3
//! has that client ship the provisioning pages it signs them on. A builder
//! living in the infra crate would be one the light client cannot reach,
//! and the client is the party that needs it.
//!
//! They sit beside [`crate::session::NetworkPoint`], which is the type
//! both of them are mostly made of.

use crate::session::NetworkPoint;
use rhtn_archive::tx::Seqno;
use rhtn_codec::encode::*;

/// The points, as an array of them.
pub fn emit_points(out: &mut Vec<u8>, points: &[NetworkPoint]) {
    emit_array_head(out, points.len());
    for p in points {
        p.encode(out);
    }
}

/// **This identity's endpoint record**, over the points as the transport
/// knows them: encoded here, and signed by `rhtn_archive`, which reads
/// them back the same way.
#[must_use]
pub fn endpoint_record(
    identity: &rhtn_crypto::SigningIdentity,
    endpoints: &[NetworkPoint],
    seqno: Seqno,
) -> Vec<u8> {
    rhtn_archive::endpoint::endpoint_record(identity, &encoded(endpoints), seqno)
}

/// **This identity's anchor entry**, likewise, with the subtree it claims.
#[must_use]
pub fn anchor_entry(
    identity: &rhtn_crypto::SigningIdentity,
    endpoints: &[NetworkPoint],
    subtree_size: u64,
    seqno: Seqno,
) -> Vec<u8> {
    rhtn_archive::endpoint::anchor_entry(identity, &encoded(endpoints), subtree_size, seqno)
}

/// Each point, encoded as one.
#[must_use]
pub fn encoded(points: &[NetworkPoint]) -> Vec<Vec<u8>> {
    points
        .iter()
        .map(|p| {
            let mut out = Vec::new();
            p.encode(&mut out);
            out
        })
        .collect()
}
