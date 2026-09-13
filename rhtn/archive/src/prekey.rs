//! The prekey objects the network carries (`wire-format.md` §7.8): the
//! bundle a subject publishes and a serving node holds without reading,
//! the two request forms, structurally distinct, and the reply.  The
//! service that holds and serves them is `rhtn-node`'s; the material inside
//! the bundle is `rhtn-client`'s.

use crate::Keyhash;
use rhtn_codec::cbor::*;
use rhtn_codec::encode::*;
use rhtn_codec::schema::{self, Family};
use rhtn_crypto::verify::{self, Lookup};

/// Stream-1 request type for both request forms (`wire-format.md` §9.2).
pub const REQUEST_PREKEY: u64 = 3;
/// `PrekeyBundle` field 2: PQXDH.
pub const CONSTRUCTION_PQXDH: u64 = 1;
/// `PrekeyReply` field 4.
pub const FAIL_UNKNOWN_SUBJECT: u64 = 0;
pub const FAIL_REFUSED: u64 = 1;
/// `PrekeyRequest` field 2.
pub const MODE_REUSABLE: u64 = 0;
pub const MODE_ONE_TIME: u64 = 1;

/// A `PrekeyBundle` as the network reads it: the subject, the construction,
/// when it was published, and the blob it does not read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PrekeyBundle {
    pub subject: Keyhash,
    pub construction: u64,
    pub blob: Vec<u8>,
    pub published_at: u64,
    pub bytes: Vec<u8>,
}

impl PrekeyBundle {
    /// Build one for `subject`, signed classically under the prekey tag over
    /// fields 1 to 4 (§7.8).
    pub fn build(subject: &rhtn_crypto::SigningIdentity, construction: u64, blob: &[u8], published_at: u64) -> Vec<u8> {
        let mut payload = Vec::new();
        emit_map_head(&mut payload, 4);
        emit_uint(&mut payload, 1);
        emit_bstr(&mut payload, &subject.public.keyhash);
        emit_uint(&mut payload, 2);
        emit_uint(&mut payload, construction);
        emit_uint(&mut payload, 3);
        emit_bstr(&mut payload, blob);
        emit_uint(&mut payload, 4);
        emit_uint(&mut payload, published_at);
        let sig = subject.sign1_ed_unnamed(rhtn_codec::cose::aad::PREKEY, &payload);
        let mut out = Vec::new();
        emit_map_head(&mut out, 5);
        out.extend_from_slice(&payload[1..]);
        emit_uint(&mut out, 5);
        out.extend_from_slice(&sig);
        out
    }

    /// Parse the fields the network reads; the blob stays bytes.
    pub fn parse(b: &[u8]) -> Result<Self, String> {
        let item = parse_all(b).map_err(|e| e.0)?;
        schema::check_kind(b, "PrekeyBundle", &item).map_err(|e| e.0)?;
        let Item::Map(m) = &item else { return Err("not a map".into()) };
        let subject = match map_get(m, 1) {
            Some(Item::Bytes(r)) if r.len() == 32 => <[u8; 32]>::try_from(&b[r.clone()]).map_err(|_| "subject")?,
            _ => return Err("field 1".into()),
        };
        let construction = map_get(m, 2).and_then(as_uint).ok_or("field 2")?;
        let blob = match map_get(m, 3) {
            Some(Item::Bytes(r)) => b[r.clone()].to_vec(),
            _ => return Err("field 3".into()),
        };
        let published_at = map_get(m, 4).and_then(as_uint).ok_or("field 4")?;
        Ok(PrekeyBundle { subject, construction, blob, published_at, bytes: b.to_vec() })
    }

    /// Signed by the subject it names.
    pub fn verify<L: Lookup + ?Sized>(&self, ids: &L) -> Result<(), String> {
        verify::record(ids, "PrekeyBundle", &self.bytes).map_err(|e| e.to_string())
    }
}

/// The two request forms, structurally distinct (§7.8): one subject, with
/// or without a one-time key; or a population swept for reusable material.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PrekeyRequest {
    One { subject: Keyhash, one_time: bool, nonce: [u8; 16] },
    Batch { subjects: Vec<Keyhash>, nonce: [u8; 16] },
}

impl PrekeyRequest {
    pub fn encode(&self) -> Vec<u8> {
        let mut out = Vec::new();
        match self {
            PrekeyRequest::One { subject, one_time, nonce } => {
                emit_map_head(&mut out, 3);
                emit_uint(&mut out, 1);
                emit_bstr(&mut out, subject);
                emit_uint(&mut out, 2);
                emit_uint(&mut out, if *one_time { MODE_ONE_TIME } else { MODE_REUSABLE });
                emit_uint(&mut out, 3);
                emit_bstr(&mut out, nonce);
            }
            PrekeyRequest::Batch { subjects, nonce } => {
                emit_map_head(&mut out, 2);
                emit_uint(&mut out, 1);
                emit_array_head(&mut out, subjects.len());
                for s in subjects {
                    emit_bstr(&mut out, s);
                }
                emit_uint(&mut out, 2);
                emit_bstr(&mut out, nonce);
            }
        }
        out
    }

    pub fn decode(b: &[u8]) -> Result<Self, String> {
        parse_all(b).map_err(|e| e.0)?;
        schema::check_unsigned(Family::PrekeyRequestOrBatch, b, 0).map_err(|e| e.0)?;
        let item = parse_all(b).map_err(|e| e.0)?;
        let Item::Map(m) = &item else { return Err("not a map".into()) };
        let kh = |it: &Item| -> Result<Keyhash, String> {
            match it {
                Item::Bytes(r) if r.len() == 32 => <[u8; 32]>::try_from(&b[r.clone()]).map_err(|_| "keyhash".into()),
                _ => Err("keyhash".into()),
            }
        };
        match map_get(m, 1) {
            Some(Item::Array(a)) => {
                let subjects: Vec<Keyhash> = a.iter().map(kh).collect::<Result<_, _>>()?;
                if subjects.windows(2).any(|w| w[0] >= w[1]) {
                    return Err("population not ascending".into());
                }
                let nonce = match map_get(m, 2) {
                    Some(Item::Bytes(r)) if r.len() == 16 => <[u8; 16]>::try_from(&b[r.clone()]).map_err(|_| "nonce")?,
                    _ => return Err("nonce".into()),
                };
                Ok(PrekeyRequest::Batch { subjects, nonce })
            }
            Some(it) => {
                let subject = kh(it)?;
                let one_time = map_get(m, 2).and_then(as_uint).ok_or("mode")? == MODE_ONE_TIME;
                let nonce = match map_get(m, 3) {
                    Some(Item::Bytes(r)) if r.len() == 16 => <[u8; 16]>::try_from(&b[r.clone()]).map_err(|_| "nonce")?,
                    _ => return Err("nonce".into()),
                };
                Ok(PrekeyRequest::One { subject, one_time, nonce })
            }
            None => Err("field 1".into()),
        }
    }
}

/// A `PrekeyReply` (§7.8): the nonce echoed, the bundle where the subject
/// is held, a one-time key where requested and available, or a failure
/// code where no bundle is held.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PrekeyReply {
    pub nonce: [u8; 16],
    pub bundle: Option<Vec<u8>>,
    pub one_time: Option<Vec<u8>>,
    pub code: Option<u64>,
}

impl PrekeyReply {
    pub fn encode(&self) -> Vec<u8> {
        let mut out = Vec::new();
        emit_map_head(&mut out, 1 + self.bundle.is_some() as usize + self.one_time.is_some() as usize + self.code.is_some() as usize);
        emit_uint(&mut out, 1);
        emit_bstr(&mut out, &self.nonce);
        if let Some(b) = &self.bundle {
            emit_uint(&mut out, 2);
            out.extend_from_slice(b);
        }
        if let Some(k) = &self.one_time {
            emit_uint(&mut out, 3);
            emit_bstr(&mut out, k);
        }
        if let Some(c) = self.code {
            emit_uint(&mut out, 4);
            emit_uint(&mut out, c);
        }
        out
    }

    pub fn decode(b: &[u8]) -> Result<Self, String> {
        parse_all(b).map_err(|e| e.0)?;
        schema::check_unsigned(Family::PrekeyReply, b, 0).map_err(|e| e.0)?;
        let item = parse_all(b).map_err(|e| e.0)?;
        let Item::Map(m) = &item else { return Err("not a map".into()) };
        let nonce = match map_get(m, 1) {
            Some(Item::Bytes(r)) if r.len() == 16 => <[u8; 16]>::try_from(&b[r.clone()]).map_err(|_| "nonce")?,
            _ => return Err("nonce".into()),
        };
        let bundle = value_slice(b, 2).map(|r| b[r].to_vec());
        let one_time = match map_get(m, 3) {
            Some(Item::Bytes(r)) => Some(b[r.clone()].to_vec()),
            _ => None,
        };
        let code = map_get(m, 4).and_then(as_uint);
        Ok(PrekeyReply { nonce, bundle, one_time, code })
    }
}

/// The reply to a batch: one `PrekeyReply` per subject named, in the
/// request's order, each echoing the nonce and never carrying a one-time
/// key.  `wire-format.md` §7.8 defines the single reply and not the
/// batch's; an array of them is this implementation's reading.
pub fn encode_batch_reply(replies: &[PrekeyReply]) -> Vec<u8> {
    let mut out = Vec::new();
    emit_array_head(&mut out, replies.len());
    for r in replies {
        out.extend_from_slice(&r.encode());
    }
    out
}

pub fn decode_batch_reply(b: &[u8]) -> Result<Vec<PrekeyReply>, String> {
    parse_all(b).map_err(|e| e.0)?;
    let parts = array_item_ranges(b, 0).ok_or("batch reply not an array")?;
    parts.into_iter().map(|r| PrekeyReply::decode(&b[r])).collect()
}
