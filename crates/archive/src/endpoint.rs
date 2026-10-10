//! The node endpoint record (`wire-format.md` §7.6).
//!
//! **It lives here because both sides of the network read one.** A node
//! holds them for the infrastructure in its horizon and a participant
//! holds them for the same reason — routing around a node that is not
//! answering is a thing a client must do without asking anyone
//! (`light-client-requirements.md` §4.2) — and a type only one of them
//! could name would have to be duplicated to be shared.

use crate::Keyhash;
use crate::tx::Seqno;
use rhtn_codec::cbor::*;
use rhtn_codec::encode::*;
use rhtn_codec::schema;
use rhtn_crypto::verify::{self, Lookup};

/// An `EndpointRecord` (`wire-format.md` §7.6) as a holder reads it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EndpointRecord {
    /// Field 1: the node whose endpoints these are, and the signer.
    pub node: Keyhash,
    /// The publisher's preference order; entries are distinct.
    pub endpoints: Vec<Vec<u8>>,
    /// The sequence number, which orders two records for one node.
    pub seqno: Seqno,
    /// The record's own bytes, which the signature covers.
    pub bytes: Vec<u8>,
}

impl EndpointRecord {
    /// Parse an endpoint record and verify the node's signature over it.
    pub fn parse(b: &[u8]) -> Result<Self, String> {
        let item = parse_all(b).map_err(|e| e.0)?;
        schema::check_kind(b, "EndpointRecord", &item).map_err(|e| e.0)?;
        let Item::Map(m) = &item else {
            return Err("not a map".into());
        };
        let node: Keyhash = match map_get(m, 1) {
            Some(Item::Bytes(r)) if r.len() == 32 => b[r.clone()].try_into().map_err(|_| "node")?,
            _ => return Err("field 1".into()),
        };
        let r2 = value_slice(b, 2).ok_or("field 2")?;
        let endpoints: Vec<Vec<u8>> = array_item_ranges(b, r2.start)
            .ok_or("endpoints walk")?
            .into_iter()
            .map(|r| b[r].to_vec())
            .collect();
        if endpoints.is_empty() || endpoints.len() > 8 {
            return Err("endpoint count".into());
        }
        let mut sorted = endpoints.clone();
        sorted.sort();
        sorted.dedup();
        if sorted.len() != endpoints.len() {
            return Err("a repeated NetworkPoint is malformed".into());
        }
        let Some(Item::Array(sq)) = map_get(m, 3) else {
            return Err("field 3".into());
        };
        let seqno = Seqno {
            series: u32::try_from(as_uint(sq.first().ok_or("series")?).ok_or("series")?)
                .map_err(|_| "series out of u32 range")?,
            counter: u32::try_from(as_uint(sq.get(1).ok_or("counter")?).ok_or("counter")?)
                .map_err(|_| "counter out of u32 range")?,
        };
        Ok(EndpointRecord {
            node,
            endpoints,
            seqno,
            bytes: b.to_vec(),
        })
    }

    /// Self-signed by the node it names; checkable only by a holder that
    /// already has the key (`wire-format.md` §7.6).  Unknown only for want
    /// of the key: a signature that fails or is malformed under a key this
    /// holder has is a failure, never gossip.
    pub fn signature_checks<L: Lookup + ?Sized>(&self, ids: &L) -> Option<bool> {
        match verify::record(ids, "EndpointRecord", &self.bytes) {
            Ok(()) => Some(true),
            Err(verify::Failure::MissingKey(_)) => None,
            // an endpoint record is never signed under a delegated key
            // (§7.6), so this arm is unreachable in fact; it is a failure
            Err(verify::Failure::MissingDelegation(_)) | Err(verify::Failure::Invalid(_)) => {
                Some(false)
            }
        }
    }
}

/// What `wire-format.md` §10.1.2 says about an arriving `EndpointRecord`,
/// given what the holder already has.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Line {
    /// The current record for that subject and series: store it.
    Current,
    /// Already held, superseded by what is held, or in a series that does
    /// not rank against the held one.  Nothing changes and nothing is
    /// forwarded.
    Duplicate,
    /// Equal `seqno` with different signed contents.  **The pair is
    /// malformed, not the later arrival**: which arrived first is an
    /// accident of the path, so the holder retains neither as current and
    /// repairs by re-resolving (§7.7).
    Conflict,
    /// A second line for a subject already holding one, in a series no
    /// §4.6 chain has proved current.  Neither stored nor forwarded; it
    /// enters when its prerequisite does.
    Unproved,
}

/// §10.1.2's decision, as a function of what the holder holds.
///
/// **One rule, two holders.** A node keeps these in its topology store and
/// a participant keeps them in its own copy of its horizon; the two kept
/// different amounts of this rule until a review found the client taking
/// an equivocating pair and an unproved series. Deciding here and storing
/// separately leaves the storage to each and the rule to neither.
///
/// `held` is what the holder has for this subject *and series*;
/// `holds_another_series` whether it has a line for the subject under any
/// other; `series_proved` whether it has been shown a chain for this one;
/// `conflicted` whether this `(subject, series, counter)` was already
/// retired by an earlier conflict.
#[must_use]
pub fn decide(
    held: Option<&EndpointRecord>,
    arriving: &EndpointRecord,
    conflicted: bool,
    holds_another_series: bool,
    series_proved: bool,
) -> Line {
    if conflicted {
        // the pair is retired: nothing further is current for it, and
        // nothing further is forwarded
        return Line::Duplicate;
    }
    match held {
        Some(h) => match crate::tx::compare(h.seqno, arriving.seqno) {
            crate::tx::Order::Older | crate::tx::Order::Incomparable => Line::Duplicate,
            crate::tx::Order::Same if h.bytes == arriving.bytes => Line::Duplicate,
            crate::tx::Order::Same => Line::Conflict,
            crate::tx::Order::Newer => Line::Current,
        },
        // **a subject's first line is taken as gossip**, there being
        // nothing to rank it against and nothing a chain could say yet
        None if holds_another_series && !series_proved => Line::Unproved,
        None => Line::Current,
    }
}

/// **This identity's endpoint record**, signed by it
/// (`wire-format.md` §7.6).
///
/// **Here because the signer is a light client** [2026-10-10].
/// `infra-client-requirements.md` §4.4 is categorical that an instance
/// cannot mint one — "the signature on both is its operator's" — so a
/// change of address is something the operator's client signs, and §8.3
/// has that client ship the provisioning pages it signs them on. The
/// points arrive **already encoded**, which is how [`EndpointRecord`]
/// reads them back: what a point is belongs to the transport, and nothing
/// here needs to know.
#[must_use]
pub fn endpoint_record(
    identity: &rhtn_crypto::SigningIdentity,
    points: &[Vec<u8>],
    seqno: Seqno,
) -> Vec<u8> {
    let mut payload = Vec::new();
    emit_map_head(&mut payload, 3);
    emit_uint(&mut payload, 1);
    emit_bstr(&mut payload, &identity.public.keyhash);
    emit_uint(&mut payload, 2);
    emit_array_head(&mut payload, points.len());
    for p in points {
        payload.extend_from_slice(p);
    }
    emit_uint(&mut payload, 3);
    seqno.emit(&mut payload);
    let sig = identity.sign1_ed_unnamed(rhtn_codec::cose::aad::ENDPOINTS, &payload);
    let mut out = Vec::new();
    emit_map_head(&mut out, 4);
    out.extend_from_slice(&payload[1..]);
    emit_uint(&mut out, 4);
    out.extend_from_slice(&sig);
    out
}

/// **This identity's anchor entry**, signed by it: the same points, with
/// the subtree it claims.
#[must_use]
pub fn anchor_entry(
    identity: &rhtn_crypto::SigningIdentity,
    points: &[Vec<u8>],
    subtree_size: u64,
    seqno: Seqno,
) -> Vec<u8> {
    let mut payload = Vec::new();
    emit_map_head(&mut payload, 4);
    emit_uint(&mut payload, 1);
    emit_bstr(&mut payload, &identity.public.keyhash);
    emit_uint(&mut payload, 2);
    emit_array_head(&mut payload, points.len());
    for p in points {
        payload.extend_from_slice(p);
    }
    emit_uint(&mut payload, 3);
    emit_uint(&mut payload, subtree_size);
    emit_uint(&mut payload, 4);
    seqno.emit(&mut payload);
    let sig = identity.sign1_ed_unnamed(rhtn_codec::cose::aad::ANCHOR, &payload);
    let mut out = Vec::new();
    emit_map_head(&mut out, 5);
    out.extend_from_slice(&payload[1..]);
    emit_uint(&mut out, 5);
    out.extend_from_slice(&sig);
    out
}
