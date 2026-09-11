//! A `SignedLocator` (`wire-format.md` §2.3): a subject's own position,
//! self-signed, as a holder keeps it and as a subject seals a line with it
//! (§4.6).

use crate::Keyhash;
use crate::tx::Locator;
use rhtn_codec::cbor::*;
use rhtn_codec::encode::*;
use rhtn_codec::schema;
use rhtn_crypto::verify::{self, Lookup};

/// A `SignedLocator` as a holder keeps it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SignedLocator {
    pub subject: Keyhash,
    pub locator: Locator,
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
        let locator = Locator::decode(&b[r2])?;
        Ok(SignedLocator { subject, locator, bytes: b.to_vec() })
    }

    /// Signed by the subject; a bare `Locator` presented alone is rejected.
    pub fn verify<L: Lookup + ?Sized>(&self, ids: &L) -> Result<bool, String> {
        verify::record(ids, "SignedLocator", &self.bytes).map_err(|e| e.to_string())
    }
}

/// Build a `SignedLocator` for `identity`'s own position.
pub fn signed_locator(identity: &rhtn_crypto::SigningIdentity, locator: &Locator) -> Vec<u8> {
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

/// The top of a counter's range (`wire-format.md` §4.6): the value a seal
/// takes, after which nothing in the series supersedes it.
pub const COUNTER_MAX: u32 = u32::MAX;

/// Seal one line (`wire-format.md` §4.6; design §9.0): the subject's own
/// locator in `series`, unchanged in anchor and path, at the maximum
/// counter.  Unilateral, needing no patron; final for everyone who takes
/// it, the subject included.
pub fn seal(identity: &rhtn_crypto::SigningIdentity, position: &Locator, series: u32) -> Vec<u8> {
    let sealed = Locator { anchor: position.anchor, path: position.path.clone(), nibbles: position.nibbles, seqno: crate::tx::Seqno { series, counter: COUNTER_MAX } };
    signed_locator(identity, &sealed)
}
