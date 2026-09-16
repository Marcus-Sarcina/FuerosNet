//! One parsed transaction: the facts a chain walker and a topology table
//! need, taken from an envelope after structural verification.

use crate::tx::*;
use crate::{Keyhash, Txid};
use rhtn_codec::cbor::*;
use rhtn_codec::cose;
use rhtn_codec::envelope;
use rhtn_codec::schema;
use rhtn_crypto::verify::{self, Lookup};

/// Whether a record's signatures were checked (`wire-format.md` §3.4):
/// verified, unverifiable for want of a signer's key, or failing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SigStatus {
    Verified,
    Unverifiable { missing: Keyhash },
    Invalid(String),
}

#[derive(Debug, Clone)]
pub struct Record {
    pub bytes: Vec<u8>,
    pub txid: Txid,
    pub tx_type: u64,
    /// Logical signers in signer order (`wire-format.md` §3.1's table).
    pub signers: Vec<Keyhash>,
    /// Key 0: one back-pointer list per signer, in signer order.
    pub back: Vec<Vec<Txid>>,
    /// The record's own time for §3.3's floor: `started_at` for a presence
    /// record, the transaction timestamp otherwise.
    pub time: u64,
    /// The record's effective time as a predecessor: `finalized_at` for a
    /// presence record, the timestamp otherwise.
    pub effective: u64,
    pub body: std::ops::Range<usize>,
}

fn kh(b: &[u8], it: &Item) -> Option<Keyhash> {
    match it {
        Item::Bytes(r) if r.len() == 32 => b[r.clone()].try_into().ok(),
        _ => None,
    }
}

impl Record {
    /// Structural parse: envelope shape, signer set, body rules
    /// (`wire-format.md` §3, §4).  Signatures are `verify`'s.
    pub fn parse(bytes: &[u8]) -> Result<Record, String> {
        let env = envelope::parse(bytes).map_err(|e| format!("envelope: {e}"))?;
        // the body rules index the body's own bytes
        let body_bytes = &bytes[env.body.clone()];
        let body_item = parse_all(body_bytes).map_err(|e| format!("body: {e}"))?;
        schema::check_body_of_type(body_bytes, &body_item, env.tx_type).map_err(|e| format!("body: {e}"))?;
        let Item::Map(m) = &env.body_item else { return Err("body".into()) };
        let Item::Array(lists) = map_get(m, 0).ok_or("key 0")? else { return Err("key 0".into()) };
        if lists.len() != env.signers.len() {
            return Err("back-pointer lists != signers".into());
        }
        let mut back = Vec::new();
        for l in lists {
            let Item::Array(hs) = l else { return Err("list".into()) };
            let mut v = Vec::new();
            for h in hs {
                v.push(kh(bytes, h).ok_or("back-pointer width")?);
            }
            if v.len() > 1 && !v.windows(2).all(|w| w[0] < w[1]) {
                return Err("merge list not sorted ascending".into());
            }
            back.push(v);
        }
        let u = |k: u64| map_get(m, k).and_then(as_uint);
        let (time, effective) = match env.tx_type {
            TYPE_PRESENCE => (u(1).ok_or("started_at")?, u(2).ok_or("finalized_at")?),
            TYPE_ADOPTION | TYPE_DEPARTURE => {
                let t = u(4).ok_or("timestamp")?;
                (t, t)
            }
            TYPE_DISAVOWAL => {
                let t = u(3).ok_or("timestamp")?;
                (t, t)
            }
            TYPE_PEERING | TYPE_REISSUE => {
                let t = u(5).ok_or("timestamp")?;
                (t, t)
            }
            _ => return Err("unknown type".into()),
        };
        let mut signers = Vec::new();
        for s in &env.signers {
            signers.push(<[u8; 32]>::try_from(s.as_slice()).map_err(|_| "signer width")?);
        }
        Ok(Record { bytes: bytes.to_vec(), txid: cose::txid(&bytes[env.body.clone()]), tx_type: env.tx_type, signers, back, time, effective, body: env.body })
    }

    /// Verify the signatures against pinned identities: a missing signer is
    /// the third outcome, not a failure (`wire-format.md` §3.4).
    pub fn check_signatures<L: Lookup + ?Sized>(&self, ids: &L) -> SigStatus {
        for s in &self.signers {
            if ids.identity(s).is_none() {
                return SigStatus::Unverifiable { missing: *s };
            }
        }
        // an embedded signer's key — a former patron, a prior key, a
        // verifier — is resolved inside verification, and its absence is
        // the same third outcome as an envelope signer's
        match verify::envelope(ids, &self.bytes) {
            Ok(_) => SigStatus::Verified,
            Err(verify::Failure::MissingKey(k)) => match <[u8; 32]>::try_from(k.as_slice()) {
                Ok(missing) => SigStatus::Unverifiable { missing },
                Err(_) => SigStatus::Invalid("a signer named by a value that is not a keyhash".into()),
            },
            Err(verify::Failure::Invalid(e)) => SigStatus::Invalid(e),
        }
    }

    pub fn body_map(&self) -> Vec<(Item, Item)> {
        match parse_all(&self.bytes[self.body.clone()]) {
            Ok(Item::Map(ref m)) => m.clone(),
            _ => Vec::new(),
        }
    }

    /// A body field as a 32-byte value.
    pub fn field_hash(&self, key: u64) -> Option<[u8; 32]> {
        let body = &self.bytes[self.body.clone()];
        let r = value_slice(body, key)?;
        let Ok(it) = parse_all(&body[r.clone()]) else { return None };
        let Item::Bytes(br) = &it else { return None };
        body[r.start + br.start..r.start + br.end].try_into().ok()
    }

    pub fn field_uint(&self, key: u64) -> Option<u64> {
        map_get(&self.body_map(), key).and_then(as_uint)
    }

    /// The back-pointer list this record carries for `signer`, if it signed.
    pub fn back_pointers_of(&self, signer: &Keyhash) -> Option<&[Txid]> {
        self.signers.iter().position(|s| s == signer).map(|i| self.back[i].as_slice())
    }

    /// A series reissue is the checkpoint a presentation may root at
    /// (design §10.1); it is patron-countersigned by construction.
    pub fn is_checkpoint(&self) -> bool {
        self.tx_type == TYPE_REISSUE
    }

    /// The seqno an adoption's locator or a departure carries.
    pub fn seqno(&self) -> Option<Seqno> {
        let m = self.body_map();
        let sq = match self.tx_type {
            TYPE_ADOPTION => match map_get(&m, 3)? {
                Item::Map(loc) => map_get(loc, 3)?.clone(),
                _ => return None,
            },
            TYPE_DEPARTURE => map_get(&m, 3)?.clone(),
            TYPE_REISSUE => map_get(&m, 4)?.clone(),
            _ => return None,
        };
        let Item::Array(a) = &sq else { return None };
        Some(Seqno { series: as_uint(a.first()?)? as u32, counter: as_uint(a.get(1)?)? as u32 })
    }

    /// The seqno a reissue leaves (field 3).
    pub fn seqno_left(&self) -> Option<Seqno> {
        if self.tx_type != TYPE_REISSUE {
            return None;
        }
        let m = self.body_map();
        let Item::Array(a) = map_get(&m, 3)? else { return None };
        Some(Seqno { series: as_uint(a.first()?)? as u32, counter: as_uint(a.get(1)?)? as u32 })
    }

    /// The locator an adoption or peering carries in field 3.
    pub fn locator(&self) -> Option<Locator> {
        if self.tx_type != TYPE_ADOPTION {
            return None;
        }
        let body = &self.bytes[self.body.clone()];
        let r = value_slice(body, 3)?;
        Locator::decode(&body[r]).ok()
    }

    /// The two participants of a presence record (field 3).
    pub fn participants(&self) -> Vec<Keyhash> {
        let m = self.body_map();
        let body = &self.bytes[self.body.clone()];
        let mut out = Vec::new();
        if let Some(Item::Array(ps)) = map_get(&m, 3) {
            for p in ps {
                if let Item::Map(pm) = p
                    && let Some(k) = map_get(pm, 1).and_then(|it| kh(body, it)) {
                        out.push(k);
                    }
            }
        }
        out
    }

    /// The prior key a recovery adoption claims (field 6.1).
    pub fn prior_key(&self) -> Option<Keyhash> {
        if self.tx_type != TYPE_ADOPTION {
            return None;
        }
        let body = &self.bytes[self.body.clone()];
        let r6 = value_slice(body, 6)?;
        let r1 = value_slice_at(body, r6.start, 1)?;
        let Ok(it) = parse_all(&body[r1.clone()]) else { return None };
        let Item::Bytes(br) = &it else { return None };
        body[r1.start + br.start..r1.start + br.end].try_into().ok()
    }
}
