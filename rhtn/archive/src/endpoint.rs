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
use rhtn_codec::schema;
use rhtn_crypto::verify::{self, Lookup};

/// An `EndpointRecord` (`wire-format.md` §7.6) as a holder reads it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EndpointRecord {
    pub node: Keyhash,
    /// The publisher's preference order; entries are distinct.
    pub endpoints: Vec<Vec<u8>>,
    pub seqno: Seqno,
    pub bytes: Vec<u8>,
}

impl EndpointRecord {
    pub fn parse(b: &[u8]) -> Result<Self, String> {
        let item = parse_all(b).map_err(|e| e.0)?;
        schema::check_kind(b, "EndpointRecord", &item).map_err(|e| e.0)?;
        let Item::Map(m) = &item else { return Err("not a map".into()) };
        let node: Keyhash = match map_get(m, 1) {
            Some(Item::Bytes(r)) if r.len() == 32 => b[r.clone()].try_into().map_err(|_| "node")?,
            _ => return Err("field 1".into()),
        };
        let r2 = value_slice(b, 2).ok_or("field 2")?;
        let endpoints: Vec<Vec<u8>> = array_item_ranges(b, r2.start).ok_or("endpoints walk")?.into_iter().map(|r| b[r].to_vec()).collect();
        if endpoints.is_empty() || endpoints.len() > 8 {
            return Err("endpoint count".into());
        }
        let mut sorted = endpoints.clone();
        sorted.sort();
        sorted.dedup();
        if sorted.len() != endpoints.len() {
            return Err("a repeated NetworkPoint is malformed".into());
        }
        let Some(Item::Array(sq)) = map_get(m, 3) else { return Err("field 3".into()) };
        let seqno = Seqno { series: as_uint(sq.first().ok_or("series")?).ok_or("series")? as u32, counter: as_uint(sq.get(1).ok_or("counter")?).ok_or("counter")? as u32 };
        Ok(EndpointRecord { node, endpoints, seqno, bytes: b.to_vec() })
    }

    /// Self-signed by the node it names; checkable only by a holder that
    /// already has the key (`wire-format.md` §7.6).  Unknown only for want
    /// of the key: a signature that fails or is malformed under a key this
    /// holder has is a failure, never gossip.
    pub fn signature_checks<L: Lookup + ?Sized>(&self, ids: &L) -> Option<bool> {
        match verify::record(ids, "EndpointRecord", &self.bytes) {
            Ok(()) => Some(true),
            Err(verify::Failure::MissingKey(_)) => None,
            Err(verify::Failure::Invalid(_)) => Some(false),
        }
    }
}

