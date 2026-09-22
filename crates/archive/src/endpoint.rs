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
            series: as_uint(sq.first().ok_or("series")?).ok_or("series")? as u32,
            counter: as_uint(sq.get(1).ok_or("counter")?).ok_or("counter")? as u32,
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
            Err(verify::Failure::Invalid(_)) => Some(false),
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
