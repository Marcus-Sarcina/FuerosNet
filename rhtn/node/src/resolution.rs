//! Resolution and the anchor table (`wire-format.md` §7.2, §7.6, §7.7;
//! design §12).
//!
//! Descent is through infrastructure only, iterative with referrals, and
//! every reply is unsigned: a wrong answer costs a failed dial, never a
//! false identity (§7.7.3).

use crate::view::NodeView;
use crate::{Adjacency, Keyhash};
use rhtn_archive::tx::{Order, Seqno, compare};
use rhtn_codec::cbor::*;
use rhtn_codec::encode::*;
use rhtn_codec::schema::{self, Family};
use rhtn_crypto::verify::{self, Lookup};
use std::collections::BTreeMap;

/// Stream-1 request types (`wire-format.md` §9.2).
pub const REQUEST_RESOLVE: u64 = 1;
pub const REQUEST_CURRENCY: u64 = 8;

/// Reply codes for `ResolveReply` field 2 (`wire-format.md` §7.7.3).
pub const REPLY_SERVING: u64 = 0;
pub const REPLY_FAILURE: u64 = 1;
pub const REPLY_REFERRAL: u64 = 2;

/// Failure codes for field 4, with the disposition each implies.
pub const FAIL_NO_SUCH_CHILD: u64 = 0;
pub const FAIL_NOT_AUTHORITATIVE: u64 = 1;
pub const FAIL_UNAVAILABLE: u64 = 2;
pub const FAIL_REFUSED: u64 = 3;

/// What a requester does with a failure code (`wire-format.md` §7.7.3).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Disposition {
    ReResolve,
    Retry,
    Terminal,
}

pub fn disposition(code: u64) -> Disposition {
    match code {
        FAIL_UNAVAILABLE => Disposition::Retry,
        FAIL_REFUSED => Disposition::Terminal,
        _ => Disposition::ReResolve,
    }
}

/// A `NetworkPoint` (`wire-format.md` §4.4) is the transport's type; the
/// node adds nothing to it.
pub use rhtn_transport::session::NetworkPoint;

fn emit_points(out: &mut Vec<u8>, points: &[NetworkPoint]) {
    emit_array_head(out, points.len());
    for p in points {
        p.encode(out);
    }
}

fn decode_points(b: &[u8], at: usize) -> Result<Vec<NetworkPoint>, String> {
    array_item_ranges(b, at).ok_or("points walk")?.iter().map(|r| NetworkPoint::decode_bytes(&b[r.clone()])).collect()
}

/// An anchor-relative path (`wire-format.md` §2.1): 4-bit hop indices, high
/// nibble first, counted in nibbles rather than bytes.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Path {
    pub bytes: Vec<u8>,
    pub nibbles: u64,
}

impl Path {
    /// The self-anchor case: a root names itself and the path has no hops.
    pub fn empty() -> Self {
        Path::default()
    }

    pub fn from_indices(ix: &[u8]) -> Self {
        let mut bytes = vec![0u8; ix.len().div_ceil(2)];
        for (i, v) in ix.iter().enumerate() {
            if i % 2 == 0 {
                bytes[i / 2] |= (v & 0x0f) << 4;
            } else {
                bytes[i / 2] |= v & 0x0f;
            }
        }
        Path { bytes, nibbles: ix.len() as u64 }
    }

    pub fn indices(&self) -> Vec<u8> {
        (0..self.nibbles as usize).map(|i| if i % 2 == 0 { self.bytes[i / 2] >> 4 } else { self.bytes[i / 2] & 0x0f }).collect()
    }

    pub fn len(&self) -> usize {
        self.nibbles as usize
    }

    pub fn is_empty(&self) -> bool {
        self.nibbles == 0
    }

    /// The suffix from index `from`, which is what a serving answer returns.
    pub fn suffix(&self, from: usize) -> Path {
        Path::from_indices(&self.indices()[from.min(self.len())..])
    }

    pub fn is_prefix_of(&self, other: &Path) -> bool {
        other.len() >= self.len() && other.indices()[..self.len()] == self.indices()[..]
    }

    pub fn emit(&self, out: &mut Vec<u8>) {
        emit_map_head(out, 2);
        emit_uint(out, 1);
        emit_bstr(out, &self.bytes);
        emit_uint(out, 2);
        emit_uint(out, self.nibbles);
    }

    pub fn decode(b: &[u8]) -> Result<Self, String> {
        let item = parse_all(b).map_err(|e| e.0)?;
        let Item::Map(m) = &item else { return Err("path not a map".into()) };
        let bytes = match map_get(m, 1) {
            Some(Item::Bytes(r)) => b[r.clone()].to_vec(),
            _ => return Err("path field 1".into()),
        };
        Ok(Path { bytes, nibbles: map_get(m, 2).and_then(as_uint).ok_or("path field 2")? })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolveRequest {
    pub subject: Keyhash,
    /// The anchor the path is relative to; without it the path is
    /// uninterpretable (`wire-format.md` §7.7.3).
    pub anchor: Keyhash,
    pub path: Vec<u8>,
    pub nibbles: u64,
    /// Fresh per logical resolution, and reused across endpoint retries for
    /// that same resolution.
    pub nonce: [u8; 16],
}

impl ResolveRequest {
    pub fn path(&self) -> Path {
        Path { bytes: self.path.clone(), nibbles: self.nibbles }
    }
    pub fn encode(&self) -> Vec<u8> {
        let mut out = Vec::new();
        emit_map_head(&mut out, 4);
        emit_uint(&mut out, 1);
        emit_bstr(&mut out, &self.subject);
        emit_uint(&mut out, 2);
        emit_bstr(&mut out, &self.anchor);
        emit_uint(&mut out, 3);
        self.path().emit(&mut out);
        emit_uint(&mut out, 4);
        emit_bstr(&mut out, &self.nonce);
        out
    }
    pub fn decode(b: &[u8]) -> Result<Self, String> {
        parse_all(b).map_err(|e| e.0)?;
        schema::check_unsigned(Family::ResolveRequest, b, 0).map_err(|e| e.0)?;
        let item = parse_all(b).map_err(|e| e.0)?;
        let Item::Map(m) = &item else { return Err("not a map".into()) };
        let kh = |k: u64| match map_get(m, k) {
            Some(Item::Bytes(r)) if r.len() == 32 => <[u8; 32]>::try_from(&b[r.clone()]).ok(),
            _ => None,
        };
        let r3 = value_slice(b, 3).ok_or("field 3")?;
        let p = Path::decode(&b[r3])?;
        let nonce = match map_get(m, 4) {
            Some(Item::Bytes(r)) if r.len() == 16 => <[u8; 16]>::try_from(&b[r.clone()]).map_err(|_| "nonce")?,
            _ => return Err("field 4".into()),
        };
        Ok(ResolveRequest { subject: kh(1).ok_or("field 1")?, anchor: kh(2).ok_or("field 2")?, path: p.bytes, nibbles: p.nibbles, nonce })
    }
}

/// The terminal answer: the serving infra node and the residual suffix that
/// identifies the client to it (`wire-format.md` §7.7.3).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServingInfra {
    pub node: Keyhash,
    pub endpoints: Vec<NetworkPoint>,
    pub residual: Path,
    pub key_material: Option<Vec<u8>>,
}

/// A referral: the next hop, its endpoints, and the indices it advances past
/// counted from the referring node's own position.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Referral {
    pub next: Keyhash,
    pub endpoints: Vec<NetworkPoint>,
    /// MUST be ≥ 1: a referral that advances nothing is a loop.
    pub advances: u64,
    pub key_material: Option<Vec<u8>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResolveReply {
    Serving { nonce: [u8; 16], serving: ServingInfra },
    Failure { nonce: [u8; 16], code: u64 },
    Referral { nonce: [u8; 16], referral: Referral },
}

impl ResolveReply {
    pub fn nonce(&self) -> [u8; 16] {
        match self {
            ResolveReply::Serving { nonce, .. } | ResolveReply::Failure { nonce, .. } | ResolveReply::Referral { nonce, .. } => *nonce,
        }
    }

    pub fn encode(&self) -> Vec<u8> {
        let mut out = Vec::new();
        emit_map_head(&mut out, 3);
        emit_uint(&mut out, 1);
        emit_bstr(&mut out, &self.nonce());
        match self {
            ResolveReply::Serving { serving, .. } => {
                emit_uint(&mut out, 2);
                emit_uint(&mut out, REPLY_SERVING);
                emit_uint(&mut out, 3);
                emit_map_head(&mut out, 3 + serving.key_material.is_some() as usize);
                emit_uint(&mut out, 1);
                emit_bstr(&mut out, &serving.node);
                emit_uint(&mut out, 2);
                emit_points(&mut out, &serving.endpoints);
                emit_uint(&mut out, 3);
                serving.residual.emit(&mut out);
                if let Some(km) = &serving.key_material {
                    emit_uint(&mut out, 4);
                    out.extend_from_slice(km);
                }
            }
            ResolveReply::Failure { code, .. } => {
                emit_uint(&mut out, 2);
                emit_uint(&mut out, REPLY_FAILURE);
                emit_uint(&mut out, 4);
                emit_uint(&mut out, *code);
            }
            ResolveReply::Referral { referral, .. } => {
                emit_uint(&mut out, 2);
                emit_uint(&mut out, REPLY_REFERRAL);
                emit_uint(&mut out, 5);
                emit_map_head(&mut out, 3 + referral.key_material.is_some() as usize);
                emit_uint(&mut out, 1);
                emit_bstr(&mut out, &referral.next);
                emit_uint(&mut out, 2);
                emit_points(&mut out, &referral.endpoints);
                emit_uint(&mut out, 3);
                emit_uint(&mut out, referral.advances);
                if let Some(km) = &referral.key_material {
                    emit_uint(&mut out, 4);
                    out.extend_from_slice(km);
                }
            }
        }
        out
    }

    pub fn decode(b: &[u8]) -> Result<Self, String> {
        parse_all(b).map_err(|e| e.0)?;
        schema::check_unsigned(Family::ResolveReply, b, 0).map_err(|e| e.0)?;
        let item = parse_all(b).map_err(|e| e.0)?;
        let Item::Map(m) = &item else { return Err("not a map".into()) };
        let nonce = match map_get(m, 1) {
            Some(Item::Bytes(r)) if r.len() == 16 => <[u8; 16]>::try_from(&b[r.clone()]).map_err(|_| "nonce")?,
            _ => return Err("field 1".into()),
        };
        let kh_at = |seg: &[u8], k: u64| -> Option<Keyhash> {
            let r = value_slice(seg, k)?;
            let it = parse_all(&seg[r.clone()]).ok()?;
            let Item::Bytes(br) = &it else { return None };
            <[u8; 32]>::try_from(&seg[r.start + br.start..r.start + br.end]).ok()
        };
        match map_get(m, 2).and_then(as_uint).ok_or("field 2")? {
            REPLY_SERVING => {
                let r3 = value_slice(b, 3).ok_or("field 3")?;
                let seg = &b[r3];
                let pts = decode_points(seg, value_slice(seg, 2).ok_or("endpoints")?.start)?;
                let residual = Path::decode(&seg[value_slice(seg, 3).ok_or("residual")?])?;
                let km = value_slice(seg, 4).map(|r| seg[r].to_vec());
                Ok(ResolveReply::Serving { nonce, serving: ServingInfra { node: kh_at(seg, 1).ok_or("serving node")?, endpoints: pts, residual, key_material: km } })
            }
            REPLY_FAILURE => Ok(ResolveReply::Failure { nonce, code: map_get(m, 4).and_then(as_uint).ok_or("field 4")? }),
            REPLY_REFERRAL => {
                let r5 = value_slice(b, 5).ok_or("field 5")?;
                let seg = &b[r5];
                let pts = decode_points(seg, value_slice(seg, 2).ok_or("endpoints")?.start)?;
                let advances = {
                    let r = value_slice(seg, 3).ok_or("advances")?;
                    as_uint(&parse_all(&seg[r]).map_err(|e| e.0)?).ok_or("advances uint")?
                };
                let km = value_slice(seg, 4).map(|r| seg[r].to_vec());
                Ok(ResolveReply::Referral { nonce, referral: Referral { next: kh_at(seg, 1).ok_or("next hop")?, endpoints: pts, advances, key_material: km } })
            }
            _ => Err("unknown reply code".into()),
        }
    }
}

/// An `AnchorEntry` (`wire-format.md` §7.2) as the table holds it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AnchorEntry {
    pub anchor: Keyhash,
    pub endpoints: Vec<NetworkPoint>,
    pub subtree_size: u64,
    pub seqno: Seqno,
    pub bytes: Vec<u8>,
}

impl AnchorEntry {
    pub fn parse(b: &[u8]) -> Result<Self, String> {
        let item = parse_all(b).map_err(|e| e.0)?;
        schema::check_kind(b, "AnchorEntry", &item).map_err(|e| e.0)?;
        let Item::Map(m) = &item else { return Err("not a map".into()) };
        let anchor: Keyhash = match map_get(m, 1) {
            Some(Item::Bytes(r)) if r.len() == 32 => <[u8; 32]>::try_from(&b[r.clone()]).map_err(|_| "anchor")?,
            _ => return Err("field 1".into()),
        };
        let endpoints = decode_points(b, value_slice(b, 2).ok_or("field 2")?.start)?;
        let Some(Item::Array(sq)) = map_get(m, 4) else { return Err("field 4".into()) };
        Ok(AnchorEntry {
            anchor,
            endpoints,
            subtree_size: map_get(m, 3).and_then(as_uint).ok_or("field 3")?,
            seqno: Seqno { series: as_uint(sq.first().ok_or("series")?).ok_or("series")? as u32, counter: as_uint(sq.get(1).ok_or("counter")?).ok_or("counter")? as u32 },
            bytes: b.to_vec(),
        })
    }

    /// Verifiable only by a holder that already has the anchor's key
    /// (`wire-format.md` §7.2): the entry carries the keyhash, not the key.
    pub fn signature_checks<L: Lookup + ?Sized>(&self, ids: &L) -> Option<bool> {
        ids.identity(&self.anchor)?;
        verify::record(ids, "AnchorEntry", &self.bytes).ok()
    }
}

/// Whether the table verified entries on acceptance or holds unverified
/// gossip checked at contact.  The boundary must be explicit
/// (`wire-format.md` §7.2), so it is a field rather than a convention.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Ingestion {
    /// Entries are verified on acceptance, possible only where the key is
    /// already pinned.
    VerifiedOnAcceptance,
    /// The table holds unverified gossip; verification happens on contact.
    UnverifiedGossip,
}

/// The anchor table: an index, not a credential store (`wire-format.md`
/// §7.2).  Which anchors a node caches is its own policy (design §12.7.3).
pub struct AnchorTable {
    entries: BTreeMap<Keyhash, AnchorEntry>,
    /// design §12.7.3: ignore roots with fewer than `threshold` subordinates.
    /// A per-node policy, never a protocol constant.
    pub threshold: u64,
    pub ingestion: Ingestion,
}

impl Default for AnchorTable {
    fn default() -> Self {
        AnchorTable { entries: BTreeMap::new(), threshold: 0, ingestion: Ingestion::UnverifiedGossip }
    }
}

impl AnchorTable {
    pub fn new(threshold: u64, ingestion: Ingestion) -> Self {
        AnchorTable { entries: BTreeMap::new(), threshold, ingestion }
    }

    pub fn get(&self, anchor: &Keyhash) -> Option<&AnchorEntry> {
        self.entries.get(anchor)
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn anchors(&self) -> Vec<Keyhash> {
        self.entries.keys().copied().collect()
    }

    /// Take a gossiped entry.  Below the node's own threshold it is not
    /// retained; within a series a strictly greater counter replaces.
    pub fn offer<L: Lookup + ?Sized>(&mut self, entry: AnchorEntry, ids: &L) -> bool {
        if entry.subtree_size < self.threshold {
            return false;
        }
        if self.ingestion == Ingestion::VerifiedOnAcceptance && entry.signature_checks(ids) != Some(true) {
            return false;
        }
        match self.entries.get(&entry.anchor) {
            Some(held) if compare(held.seqno, entry.seqno) != Order::Newer => false,
            _ => {
                self.entries.insert(entry.anchor, entry);
                true
            }
        }
    }
}

/// Why a resolution was not sent.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NotResolvable {
    /// The locator's anchor is absent from the local table: a caller-side
    /// condition, not a wire failure (`wire-format.md` §7.7.3).
    AnchorAbsent(Keyhash),
}

/// What a requester decided about a reply.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Step {
    /// The resolution is done.
    Arrived(ServingInfra),
    /// Continue from the referral's next hop.
    Continue(Referral),
    /// The reply is malformed: a referral advancing nothing is a loop, and
    /// one advancing past the path's end is malformed (`wire-format.md` §7.7.3).
    Malformed(String),
    Failed { code: u64, disposition: Disposition },
    /// The nonce does not echo the request's.
    WrongNonce,
}

/// Read a reply against the request it answers, from `position` indices
/// already consumed along the path.
pub fn step(req: &ResolveRequest, consumed: usize, reply: &ResolveReply) -> Step {
    if reply.nonce() != req.nonce {
        return Step::WrongNonce;
    }
    match reply {
        ResolveReply::Serving { serving, .. } => Step::Arrived(serving.clone()),
        ResolveReply::Failure { code, .. } => Step::Failed { code: *code, disposition: disposition(*code) },
        ResolveReply::Referral { referral, .. } => {
            if referral.advances < 1 {
                return Step::Malformed("a referral that advances nothing is a loop".into());
            }
            if consumed + referral.advances as usize > req.path().len() {
                return Step::Malformed("referral advances past the path's end".into());
            }
            Step::Continue(referral.clone())
        }
    }
}

/// How a resolution begun by this node is being carried.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Carried {
    /// A light client's: sent to its serving node, which resolves on its
    /// behalf and returns the result (`wire-format.md` §7.7.1).
    Delegated(Keyhash),
    /// An infra node's own: the first hop is the anchor, dialled from the
    /// table's entry, or reached on a session where one exists.
    Direct { sent_on_session: bool },
}

/// What a serving node does with a resolution request from a client.
#[derive(Debug, Clone)]
pub enum ClientResolution {
    /// The node is an ancestor on the path and answers from its own
    /// tables, as it would for anyone.
    Answered(ResolveReply),
    /// The node resolves on the client's behalf: it drives this resolution
    /// against the anchor and every referral, and returns the result under
    /// the client's nonce.
    Proxied(Resolution),
}

impl NodeView {
    /// Begin a resolution for `subject` under `anchor` and `path`.  A light
    /// client sends it to its serving node and never to the anchor; an
    /// infra node begins it from its anchor table, and a locator whose
    /// anchor is absent from that table is a caller-side condition with no
    /// request sent (`wire-format.md` §7.7.1, §7.7.3).
    pub fn resolve(&self, adj: &dyn Adjacency, anchors: &AnchorTable, subject: Keyhash, anchor: Keyhash, path: Path, nonce: [u8; 16]) -> Result<(Resolution, Carried), NotResolvable> {
        let req = ResolveRequest { subject, anchor, path: path.bytes.clone(), nibbles: path.nibbles, nonce };
        if let Some(serving) = self.serving_node {
            adj.send(&serving, REQUEST_RESOLVE, &req.encode());
            let r = Resolution { request: req, consumed: 0, hops: vec![serving], endpoints: Vec::new(), arrived: None };
            return Ok((r, Carried::Delegated(serving)));
        }
        let r = Resolution::begin(anchors, subject, anchor, path, nonce)?;
        let on_session = adj.has_session(&anchor);
        if on_session {
            adj.send(&anchor, REQUEST_RESOLVE, &r.request.encode());
        }
        Ok((r, Carried::Direct { sent_on_session: on_session }))
    }

    /// A resolution request from an attached client (`wire-format.md`
    /// §7.7.1): answered from this node's own tables where it is on the
    /// path, and otherwise resolved on the client's behalf.
    pub fn resolve_for_client(&self, anchors: &AnchorTable, req: &ResolveRequest) -> Result<ClientResolution, NotResolvable> {
        let mine = self.position_in(&req.anchor).map(|p| Path { bytes: p.path.clone(), nibbles: p.nibbles });
        if let Some(my) = mine {
            if my.is_prefix_of(&req.path()) {
                return Ok(ClientResolution::Answered(self.answer_resolution(req)));
            }
        }
        let r = Resolution::begin(anchors, req.subject, req.anchor, req.path(), req.nonce)?;
        Ok(ClientResolution::Proxied(r))
    }

    /// The reply a serving node returns to its client once a proxied
    /// resolution has run: the serving answer under the client's nonce, or
    /// the failure the last hop gave.
    pub fn reply_for_client(&self, r: &Resolution, last: Option<&ResolveReply>) -> ResolveReply {
        match (&r.arrived, last) {
            (Some(si), _) => ResolveReply::Serving { nonce: r.request.nonce, serving: si.clone() },
            (None, Some(ResolveReply::Failure { code, .. })) => ResolveReply::Failure { nonce: r.request.nonce, code: *code },
            (None, _) => ResolveReply::Failure { nonce: r.request.nonce, code: FAIL_NOT_AUTHORITATIVE },
        }
    }

    /// Answer a resolution from this node's own position and tables.
    /// Nothing about the requester is retained (design §12.6.5).
    pub fn answer_resolution(&self, req: &ResolveRequest) -> ResolveReply {
        let nonce = req.nonce;
        // a node bound in several subnets has a position in each
        let Some(pos) = self.position_in(&req.anchor) else {
            return ResolveReply::Failure { nonce, code: FAIL_NOT_AUTHORITATIVE };
        };
        let my = Path { bytes: pos.path.clone(), nibbles: pos.nibbles };
        let full = req.path();
        if !my.is_prefix_of(&full) {
            return ResolveReply::Failure { nonce, code: FAIL_NOT_AUTHORITATIVE };
        }
        let consumed = my.len();
        let residual = full.suffix(consumed);
        // the target is this node itself: a root's self-anchored locator
        // resolves to an empty residual (design §12.6.1)
        if residual.is_empty() {
            return self.serving_answer(nonce, Path::empty());
        }
        let next_index = residual.indices()[0];
        let Some(child) = self.child_at(next_index) else {
            return ResolveReply::Failure { nonce, code: FAIL_NO_SUCH_CHILD };
        };
        // an attached client, or a light-client chain below this node: this
        // node is the serving infra node and answers authoritatively with
        // the residual suffix (`wire-format.md` §7.7.2)
        if !self.table.is_infra(&child) {
            return self.serving_answer(nonce, residual);
        }
        // an infra child: refer from its endpoint record, before ever
        // contacting it (`infra-client-requirements.md` §4.4)
        match self.store.endpoint(&child) {
            Some(er) => {
                let endpoints = er.endpoints.iter().filter_map(|p| NetworkPoint::decode_bytes(p).ok()).collect();
                ResolveReply::Referral { nonce, referral: Referral { next: child, endpoints, advances: 1, key_material: None } }
            }
            None => ResolveReply::Failure { nonce, code: FAIL_NOT_AUTHORITATIVE },
        }
    }

    /// A serving answer names this node's own endpoints, from the record
    /// it published.  A node with none published cannot be reached past the
    /// resolution, so it reports itself unavailable rather than emitting an
    /// answer the schema rejects.
    fn serving_answer(&self, nonce: [u8; 16], residual: Path) -> ResolveReply {
        let endpoints = self.own_endpoints();
        if endpoints.is_empty() {
            return ResolveReply::Failure { nonce, code: FAIL_UNAVAILABLE };
        }
        ResolveReply::Serving {
            nonce,
            serving: ServingInfra { node: self.me(), endpoints, residual, key_material: Some(self.identity.public.key_material()) },
        }
    }

    /// This node's own published endpoints, from its own endpoint record.
    pub fn own_endpoints(&self) -> Vec<NetworkPoint> {
        self.store.endpoint(&self.me()).map(|er| er.endpoints.iter().filter_map(|p| NetworkPoint::decode_bytes(p).ok()).collect()).unwrap_or_default()
    }

    /// The subordinate this node holds at child index `ix`.
    pub fn child_at(&self, ix: u8) -> Option<Keyhash> {
        self.slots.get(&(ix as u64)).and_then(|s| s.occupant)
    }

    /// Publish this node's own endpoint record for a series
    /// (`wire-format.md` §7.6).  Republishing an unchanged list replays the
    /// record already held rather than consuming a number.
    pub fn publish_endpoints(&mut self, endpoints: &[NetworkPoint], seqno: Seqno) -> Vec<u8> {
        let me = self.me();
        let points: Vec<Vec<u8>> = endpoints.iter().map(|p| p.encode_bytes()).collect();
        if let Some(held) = self.store.endpoint_in(&me, seqno.series) {
            if held.endpoints == points {
                return held.bytes.clone();
            }
        }
        let mut payload = Vec::new();
        emit_map_head(&mut payload, 3);
        emit_uint(&mut payload, 1);
        emit_bstr(&mut payload, &me);
        emit_uint(&mut payload, 2);
        emit_points(&mut payload, endpoints);
        emit_uint(&mut payload, 3);
        seqno.emit(&mut payload);
        let sig = self.identity.sign1_ed_unnamed(rhtn_codec::cose::aad::ENDPOINTS, &payload);
        let mut out = Vec::new();
        emit_map_head(&mut out, 4);
        out.extend_from_slice(&payload[1..]);
        emit_uint(&mut out, 4);
        out.extend_from_slice(&sig);
        out
    }
}

/// Build an `EndpointRecord` for `identity` at `seqno` (`wire-format.md` §7.6).
pub fn endpoint_record(identity: &rhtn_crypto::SigningIdentity, endpoints: &[NetworkPoint], seqno: Seqno) -> Vec<u8> {
    let mut payload = Vec::new();
    emit_map_head(&mut payload, 3);
    emit_uint(&mut payload, 1);
    emit_bstr(&mut payload, &identity.public.keyhash);
    emit_uint(&mut payload, 2);
    emit_points(&mut payload, endpoints);
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

/// An `AnchorEntry` for `identity` (`wire-format.md` §7.2).
pub fn anchor_entry(identity: &rhtn_crypto::SigningIdentity, endpoints: &[NetworkPoint], subtree_size: u64, seqno: Seqno) -> Vec<u8> {
    let mut payload = Vec::new();
    emit_map_head(&mut payload, 4);
    emit_uint(&mut payload, 1);
    emit_bstr(&mut payload, &identity.public.keyhash);
    emit_uint(&mut payload, 2);
    emit_points(&mut payload, endpoints);
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

// ---------------------------------------------------------------- locators

/// A `SignedLocator` (`wire-format.md` §2.3) as a holder keeps it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SignedLocator {
    pub subject: Keyhash,
    pub locator: rhtn_archive::tx::Locator,
    pub bytes: Vec<u8>,
}

impl SignedLocator {
    pub fn parse(b: &[u8]) -> Result<Self, String> {
        let item = parse_all(b).map_err(|e| e.0)?;
        schema::check_kind(b, "SignedLocator", &item).map_err(|e| e.0)?;
        let Item::Map(m) = &item else { return Err("not a map".into()) };
        let subject: Keyhash = match map_get(m, 1) {
            Some(Item::Bytes(r)) if r.len() == 32 => <[u8; 32]>::try_from(&b[r.clone()]).map_err(|_| "subject")?,
            _ => return Err("field 1".into()),
        };
        let r2 = value_slice(b, 2).ok_or("field 2")?;
        let locator = rhtn_archive::tx::Locator::decode(&b[r2])?;
        Ok(SignedLocator { subject, locator, bytes: b.to_vec() })
    }

    /// Signed by the subject; a bare `Locator` presented alone is rejected.
    pub fn verify<L: Lookup + ?Sized>(&self, ids: &L) -> Result<bool, String> {
        verify::record(ids, "SignedLocator", &self.bytes)
    }
}

/// Build a `SignedLocator` for `identity`'s own position.
pub fn signed_locator(identity: &rhtn_crypto::SigningIdentity, locator: &rhtn_archive::tx::Locator) -> Vec<u8> {
    let mut payload = Vec::new();
    emit_map_head(&mut payload, 2);
    emit_uint(&mut payload, 1);
    emit_bstr(&mut payload, &identity.public.keyhash);
    emit_uint(&mut payload, 2);
    locator.emit(&mut payload);
    let sig = identity.sign1_ed_unnamed(rhtn_codec::cose::aad::LOCATOR, &payload);
    let mut out = Vec::new();
    emit_map_head(&mut out, 3);
    out.extend_from_slice(&payload[1..]);
    emit_uint(&mut out, 3);
    out.extend_from_slice(&sig);
    out
}

/// What a holder's store did with a freshness-bearing record
/// (`negative-vectors.md`'s `state_action`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LocatorOutcome {
    Installed,
    Replaced,
    /// A lower counter in a series already held.
    IgnoredStale,
    /// The identical record already held.
    Replay,
    /// Equal `seqno`, different contents: the pair is malformed and neither
    /// is current (`wire-format.md` §7.7.3).
    Conflict,
    /// A different series: the two do not rank, and a reader holding both
    /// has learned nothing (`wire-format.md` §2.3).
    Incomparable,
    Malformed(String),
}

/// What a holder can do about reaching a subject.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Reach {
    /// The one locator this holder ranks current.
    Dial(rhtn_archive::tx::Locator),
    /// Neither of a conflicting pair is current; a fresh locator is needed
    /// (`wire-format.md` §7.7.3).
    MustReResolve,
    /// Two series and no §4.6 chain to rank them.
    Indeterminate,
    /// Nothing held.
    Unknown,
}

/// The locators a holder keeps for other parties, one line per series
/// (`wire-format.md` §2.3).
#[derive(Default)]
pub struct LocatorStore {
    held: BTreeMap<(Keyhash, u32), SignedLocator>,
    conflicts: std::collections::BTreeSet<(Keyhash, u32, u32)>,
    /// Series this holder has been shown a §4.6 chain for.
    proved: std::collections::BTreeSet<(Keyhash, u32)>,
}

impl LocatorStore {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn prove_series(&mut self, subject: Keyhash, series: u32) {
        self.proved.insert((subject, series));
    }

    /// Offer a signed locator; it must verify under its own subject.
    pub fn offer<L: Lookup + ?Sized>(&mut self, bytes: &[u8], ids: &L) -> LocatorOutcome {
        let sl = match SignedLocator::parse(bytes) {
            Ok(s) => s,
            Err(e) => return LocatorOutcome::Malformed(e),
        };
        match sl.verify(ids) {
            Ok(true) => {}
            Ok(false) => return LocatorOutcome::Malformed("subject signature fails".into()),
            Err(e) => return LocatorOutcome::Malformed(e),
        }
        let sq = sl.locator.seqno;
        if self.conflicts.contains(&(sl.subject, sq.series, sq.counter)) {
            return LocatorOutcome::Conflict;
        }
        let key = (sl.subject, sq.series);
        match self.held.get(&key) {
            None => {
                let other_series = self.held.keys().any(|(s, _)| *s == sl.subject);
                self.held.insert(key, sl);
                if other_series { LocatorOutcome::Incomparable } else { LocatorOutcome::Installed }
            }
            Some(held) => match compare(held.locator.seqno, sq) {
                Order::Newer => {
                    self.held.insert(key, sl);
                    LocatorOutcome::Replaced
                }
                Order::Older => LocatorOutcome::IgnoredStale,
                Order::Incomparable => LocatorOutcome::Incomparable,
                Order::Same => {
                    if held.bytes == sl.bytes {
                        return LocatorOutcome::Replay;
                    }
                    self.held.remove(&key);
                    self.conflicts.insert((sl.subject, sq.series, sq.counter));
                    LocatorOutcome::Conflict
                }
            },
        }
    }

    /// Every locator held for `subject`, one per series.
    pub fn all(&self, subject: &Keyhash) -> Vec<&SignedLocator> {
        self.held.iter().filter(|((s, _), _)| s == subject).map(|(_, v)| v).collect()
    }

    pub fn in_series(&self, subject: &Keyhash, series: u32) -> Option<&SignedLocator> {
        self.held.get(&(*subject, series))
    }

    /// Whether a conflict retired a `(subject, seqno)` pair.
    pub fn conflicted(&self, subject: &Keyhash) -> bool {
        self.conflicts.iter().any(|(s, _, _)| s == subject)
    }

    /// The one locator this holder ranks current, and what to do when it
    /// ranks none.
    pub fn reach(&self, subject: &Keyhash) -> Reach {
        let held = self.all(subject);
        if held.is_empty() {
            return if self.conflicted(subject) { Reach::MustReResolve } else { Reach::Unknown };
        }
        if self.conflicted(subject) && held.len() == 1 {
            // the conflicted line is retired; another proved series may stand
            let s = held[0].locator.seqno.series;
            if !self.proved.contains(&(*subject, s)) {
                return Reach::MustReResolve;
            }
        }
        match held.len() {
            1 => Reach::Dial(held[0].locator.clone()),
            _ => {
                let proved: Vec<_> = held.iter().filter(|l| self.proved.contains(&(*subject, l.locator.seqno.series))).collect();
                match proved.len() {
                    1 => Reach::Dial(proved[0].locator.clone()),
                    _ => Reach::Indeterminate,
                }
            }
        }
    }
}

// ---------------------------------------------------------------- resolving

/// One logical resolution, driven by its caller: the nonce is fresh per
/// resolution and reused across endpoint retries (`wire-format.md` §7.7.3).
#[derive(Debug, Clone)]
pub struct Resolution {
    pub request: ResolveRequest,
    /// Path indices consumed so far, from referrals.
    pub consumed: usize,
    /// The hops queried, in order.
    pub hops: Vec<Keyhash>,
    pub endpoints: Vec<NetworkPoint>,
    pub arrived: Option<ServingInfra>,
}

impl Resolution {
    /// Begin a resolution.  A locator whose anchor is absent from the table
    /// is a caller-side condition and no request is sent
    /// (`wire-format.md` §7.7.3).
    pub fn begin(anchors: &AnchorTable, subject: Keyhash, anchor: Keyhash, path: Path, nonce: [u8; 16]) -> Result<Resolution, NotResolvable> {
        let entry = anchors.get(&anchor).ok_or(NotResolvable::AnchorAbsent(anchor))?;
        Ok(Resolution {
            request: ResolveRequest { subject, anchor, path: path.bytes, nibbles: path.nibbles, nonce },
            consumed: 0,
            hops: vec![anchor],
            endpoints: entry.endpoints.clone(),
            arrived: None,
        })
    }

    /// The party to query next, and the endpoints to try for it.
    pub fn next_hop(&self) -> (Keyhash, &[NetworkPoint]) {
        (*self.hops.last().unwrap(), &self.endpoints)
    }

    /// Take a reply; a referral advances the resolution, a serving answer
    /// ends it, and a malformed reply is not followed.
    pub fn take(&mut self, reply: &ResolveReply) -> Step {
        let s = step(&self.request, self.consumed, reply);
        match &s {
            Step::Arrived(si) => self.arrived = Some(si.clone()),
            Step::Continue(r) => {
                self.consumed += r.advances as usize;
                self.hops.push(r.next);
                self.endpoints = r.endpoints.clone();
            }
            _ => {}
        }
        s
    }
}

// ---------------------------------------------------------------- contact

/// Why a contact attempt at one endpoint failed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EndpointFailure {
    pub endpoint: NetworkPoint,
    pub why: String,
}

/// The outcome of contacting a party a resolution named.
#[derive(Debug)]
pub enum Contact {
    /// The handshake completed against the key this caller intended to reach.
    Reached(quinn::Connection),
    /// Every listed endpoint failed.  This is a failed contact with the
    /// party the caller meant, never a contact with somebody else: the
    /// requester authenticates the subject it intended to reach, so a wrong
    /// address produces a handshake failure rather than a silent
    /// misdirection (`wire-format.md` §7.7.3, §9.1).
    Failed { target: Keyhash, attempts: Vec<EndpointFailure> },
}

/// Try a published endpoint list as alternatives, in the publisher's
/// preference order, and stop at the first that authenticates as `target`.
/// An implementation MUST try others on failure, or a single unreachable
/// first entry becomes a permanent outage for that peer
/// (`wire-format.md` §7.7.3).  How long to wait is local policy.
pub async fn contact(
    endpoints: &[NetworkPoint],
    ep: &quinn::Endpoint,
    me: &rhtn_crypto::SigningIdentity,
    pins: &rhtn_transport::tls::Pins,
    target: &Keyhash,
    per_endpoint: std::time::Duration,
) -> Contact {
    let mut attempts = Vec::new();
    for e in endpoints {
        let connecting = match rhtn_transport::tls::dial(ep, me, pins, target, e.socket()) {
            Ok(c) => c,
            Err(err) => {
                attempts.push(EndpointFailure { endpoint: e.clone(), why: format!("{err:?}") });
                continue;
            }
        };
        match tokio::time::timeout(per_endpoint, connecting).await {
            Ok(Ok(conn)) => return Contact::Reached(conn),
            Ok(Err(err)) => attempts.push(EndpointFailure { endpoint: e.clone(), why: format!("{err}") }),
            Err(_) => attempts.push(EndpointFailure { endpoint: e.clone(), why: "no answer".into() }),
        }
    }
    Contact::Failed { target: *target, attempts }
}
