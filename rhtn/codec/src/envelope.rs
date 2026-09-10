//! The transaction envelope (§3): `{1: version, 2: type, 3: body, 4: COSE_Sign}`,
//! its signer set derived from the body per type (§3.5), and the entries a
//! verifier must check.  Structural only; signatures are `rhtn-crypto`'s.

use crate::cbor::*;
use std::ops::Range;

/// One `COSE_Signature` entry: protected header bytes, the `kid` and `alg`
/// it carries, and the signature bytes.  Ranges index the envelope bytes.
#[derive(Debug, Clone)]
pub struct Entry {
    pub protected: Range<usize>,
    pub kid: Vec<u8>,
    pub alg: i64,
    pub signature: Range<usize>,
}

#[derive(Debug, Clone)]
pub struct Envelope {
    pub version: u64,
    pub tx_type: u64,
    /// The body's encoded bytes, which every signature covers.
    pub body: Range<usize>,
    pub body_item: Item,
    /// Logical signers in body order, as keyhashes.
    pub signers: Vec<Vec<u8>>,
    pub entries: Vec<Entry>,
}

pub const KEY_VERSION: u64 = 1;
pub const KEY_TYPE: u64 = 2;
pub const KEY_BODY: u64 = 3;
pub const KEY_SIGNATURES: u64 = 4;

fn bs<'a>(b: &'a [u8], it: &Item) -> &'a [u8] {
    match it {
        Item::Bytes(r) | Item::Text(r) => &b[r.clone()],
        _ => &[],
    }
}

/// The signer set the body implies for a transaction type (§3.5).
pub fn signer_set(b: &[u8], tx_type: u64, body: &[(Item, Item)]) -> Result<Vec<Vec<u8>>, Error> {
    let mut signers = Vec::new();
    match tx_type {
        1 | 4 | 7 => {
            for f in [1u64, 2] {
                signers.push(bs(b, map_get(body, f).ok_or(Error("missing role field"))?).to_vec());
            }
        }
        2 | 3 => signers.push(bs(b, map_get(body, 1).ok_or(Error("missing role field"))?).to_vec()),
        5 => {
            let Item::Array(parts) = map_get(body, 3).ok_or(Error("no participants"))? else {
                return Err(Error("participants not array"));
            };
            for pi in parts {
                let Item::Map(pm) = pi else { return Err(Error("participant not map")) };
                signers.push(bs(b, map_get(pm, 1).ok_or(Error("participant keyhash"))?).to_vec());
            }
            if let Some(Item::Array(ws)) = map_get(body, 4) {
                for w in ws {
                    let Item::Map(wm) = w else { return Err(Error("witness not map")) };
                    signers.push(bs(b, map_get(wm, 1).ok_or(Error("witness keyhash"))?).to_vec());
                }
            }
        }
        _ => return Err(Error("unknown transaction type")),
    }
    Ok(signers)
}

/// Parse an envelope structurally.  The envelope map sits outside every
/// signature's coverage, so it admits no unknown keys (§1.2's rule for
/// unsigned material); the body is signed and keeps its unknown keys.
pub fn parse(b: &[u8]) -> Result<Envelope, Error> {
    let item = parse_all(b)?;
    let Item::Map(top) = &item else { return Err(Error("envelope not a map")) };
    for (k, _) in top {
        match k {
            Item::Uint(1..=4) => {}
            _ => return Err(Error("unknown envelope key")),
        }
    }
    let version = map_get(top, KEY_VERSION).and_then(as_uint).ok_or(Error("no version"))?;
    if version != 1 {
        return Err(Error("version != 1"));
    }
    let tx_type = map_get(top, KEY_TYPE).and_then(as_uint).ok_or(Error("no type"))?;
    let body = value_slice(b, KEY_BODY).ok_or(Error("no body"))?;
    let p = Parser { b };
    let (body_item, bend) = p.item(body.start)?;
    if bend != body.end {
        return Err(Error("body length disagreement"));
    }
    let Item::Map(body_map) = &body_item else { return Err(Error("body not map")) };
    let signers = signer_set(b, tx_type, body_map)?;

    let sigs = value_slice(b, KEY_SIGNATURES).ok_or(Error("no signatures"))?;
    let Item::Array(cs) = map_get(top, KEY_SIGNATURES).ok_or(Error("no signatures"))? else {
        return Err(Error("cose not array"));
    };
    if cs.len() != 4 {
        return Err(Error("cose arity"));
    }
    let Item::Array(ents) = &cs[3] else { return Err(Error("entries not array")) };
    if let Some(ceiling) = crate::bounds::envelope_entry_ceiling(tx_type)
        && ents.len() > ceiling {
            return Err(Error("entries over the derived ceiling"));
        }
    if ents.len() != signers.len() * 2 {
        return Err(Error("entry count != 2x signers"));
    }
    // walk the entries array by ranges so protected/signature slices are exact
    let outer = array_item_ranges(b, sigs.start).ok_or(Error("cose walk"))?;
    let entry_ranges = array_item_ranges(b, outer[3].start).ok_or(Error("entries walk"))?;
    let mut entries = Vec::new();
    for (e, er) in ents.iter().zip(entry_ranges) {
        let Item::Array(ea) = e else { return Err(Error("entry not array")) };
        if ea.len() != 3 {
            return Err(Error("entry arity"));
        }
        let parts = array_item_ranges(b, er.start).ok_or(Error("entry walk"))?;
        let (Item::Bytes(pr), Item::Bytes(sr)) = (&ea[0], &ea[2]) else {
            return Err(Error("entry protected/signature not bstr"));
        };
        let _ = parts;
        let prot = &b[pr.clone()];
         let Item::Map(ref pm) = parse_all(prot)? else { return Err(Error("protected not map")) };
        let alg = pm
            .iter()
            .find_map(|(k, v)| match (k, v) {
                (Item::Uint(1), Item::Neg(a)) => Some(*a),
                _ => None,
            })
            .ok_or(Error("no alg"))?;
        let kid = pm
            .iter()
            .find_map(|(k, v)| match (k, v) {
                (Item::Uint(4), Item::Bytes(r)) => Some(prot[r.clone()].to_vec()),
                _ => None,
            })
            .ok_or(Error("no kid"))?;
        if !signers.contains(&kid) {
            return Err(Error("kid outside body signer set"));
        }
        entries.push(Entry { protected: pr.clone(), kid, alg, signature: sr.clone() });
    }
    Ok(Envelope { version, tx_type, body, body_item, signers, entries })
}
