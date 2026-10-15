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
    /// The subject the locator is for, and the signer.
    pub subject: Keyhash,
    /// The locator itself.
    pub locator: Locator,
    /// The object's own bytes, which the signature covers.
    pub bytes: Vec<u8>,
}

impl SignedLocator {
    /// Parse a signed locator and verify the subject's signature over it.
    pub fn parse(b: &[u8]) -> Result<Self, String> {
        let item = parse_all(b).map_err(|e| e.0)?;
        schema::check_kind(b, "SignedLocator", &item).map_err(|e| e.0)?;
        let Item::Map(m) = &item else {
            return Err("not a map".into());
        };
        let subject: Keyhash = match map_get(m, 1) {
            Some(Item::Bytes(r)) if r.len() == 32 => {
                <[u8; 32]>::try_from(&b[r.clone()]).map_err(|_| "subject")?
            }
            _ => return Err("field 1".into()),
        };
        let r2 = value_slice(b, 2).ok_or("field 2")?;
        let locator = Locator::decode(&b[r2])?;
        Ok(SignedLocator {
            subject,
            locator,
            bytes: b.to_vec(),
        })
    }

    /// Signed by the subject; a bare `Locator` presented alone is rejected.
    pub fn verify<L: Lookup + ?Sized>(&self, ids: &L) -> Result<(), String> {
        verify::record(ids, "SignedLocator", &self.bytes).map_err(|e| e.to_string())
    }
}

/// An `Introduction` (`wire-format.md` §2.3): a third party's locator handed
/// with its key, as a bundle carries one (§5.4) and a fishing proposal is
/// one (§7.10.2).  Parsed, the material names an identity, the locator's
/// subject is that identity's hash, and the signature verifies under it;
/// nothing else about the subject is said.
#[derive(Clone)]
pub struct Introduction {
    /// The subject, from the carried material.
    pub identity: rhtn_crypto::Identity,
    /// The material as carried, for handing onward.
    pub material: Vec<u8>,
    /// The locator, verified under the material.
    pub locator: SignedLocator,
}

impl PartialEq for Introduction {
    /// Two introductions are the same when their bytes are: the identity
    /// is a function of the material.
    fn eq(&self, other: &Self) -> bool {
        self.material == other.material && self.locator == other.locator
    }
}
impl Eq for Introduction {}

impl std::fmt::Debug for Introduction {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Introduction")
            .field("subject", &self.identity.keyhash)
            .field("locator", &self.locator.locator)
            .finish()
    }
}

impl Introduction {
    /// Parse an introduction and verify it: the codec's shape check binds
    /// the material to the locator's subject, and the locator's signature
    /// is checked under that material and nothing else.
    pub fn parse(b: &[u8]) -> Result<Self, String> {
        let item = parse_all(b).map_err(|e| e.0)?;
        schema::check_kind(b, "Introduction", &item).map_err(|e| e.0)?;
        let parts = array_item_ranges(b, 0).ok_or("not an array")?;
        let material = b[parts[1].clone()].to_vec();
        let identity = rhtn_crypto::Identity::from_key_material(&material)
            .ok_or("the key material does not name an identity")?;
        let locator = SignedLocator::parse(&b[parts[2].clone()])?;
        locator.verify(std::slice::from_ref(&identity))?;
        Ok(Introduction {
            identity,
            material,
            locator,
        })
    }
}

/// An `AnchoredIntroduction` (`wire-format.md` §2.4) as a holder reads it:
/// an introduction, and the anchor its locator names with the records
/// that say where that anchor answers.
///
/// **What a code handed over at a meeting carries** [author, 2026-10-13]:
/// "one code should carry the introduction and its anchor records". An
/// introduction names an anchor and a path and no address, and a holder
/// with no node of its own has nothing to dial without these.
#[derive(Clone)]
pub struct AnchoredIntroduction {
    /// The introduction itself.
    pub introduction: Introduction,
    /// The anchor's identity, from its key material.
    pub anchor: rhtn_crypto::Identity,
    /// The anchor's endpoint record, verified under that material.
    pub endpoint: crate::endpoint::EndpointRecord,
    /// Its anchor entry, where one came.
    pub entry: Option<Vec<u8>>,
}

impl AnchoredIntroduction {
    /// Parse and verify one: the codec's shape check binds the anchor
    /// material to the anchor the locator names and each record to that
    /// node, the introduction verifies under its own material, and the
    /// endpoint record verifies under the anchor's.
    pub fn parse(b: &[u8]) -> Result<Self, String> {
        let item = parse_all(b).map_err(|e| e.0)?;
        schema::check_kind(b, "AnchoredIntroduction", &item).map_err(|e| e.0)?;
        let parts = array_item_ranges(b, 0).ok_or("not an array")?;
        let introduction = Introduction::parse(&b[parts[1].clone()])?;
        let material = b[parts[2].clone()].to_vec();
        let anchor = rhtn_crypto::Identity::from_key_material(&material)
            .ok_or("the anchor's key material does not name an identity")?;
        let endpoint = crate::endpoint::EndpointRecord::parse(&b[parts[3].clone()])?;
        if endpoint.signature_checks(std::slice::from_ref(&anchor)) != Some(true) {
            return Err("the endpoint record is not the anchor's".into());
        }
        let entry = match &b[parts[4].clone()] {
            [0xf6] => None,
            bytes => Some(bytes.to_vec()),
        };
        Ok(AnchoredIntroduction {
            introduction,
            anchor,
            endpoint,
            entry,
        })
    }
}

/// Compose an `AnchoredIntroduction` from an introduction, the key
/// material of the anchor its locator names, and that anchor's published
/// records.  The entry is optional; everything else is not.
pub fn anchored_introduction(
    introduction: &[u8],
    anchor_material: &[u8],
    endpoint_record: &[u8],
    anchor_entry: Option<&[u8]>,
) -> Vec<u8> {
    let mut out = Vec::new();
    emit_array_head(&mut out, 5);
    emit_uint(&mut out, 1);
    out.extend_from_slice(introduction);
    out.extend_from_slice(anchor_material);
    out.extend_from_slice(endpoint_record);
    match anchor_entry {
        Some(e) => out.extend_from_slice(e),
        None => out.push(0xf6),
    }
    out
}

/// Compose an `Introduction` from a subject's key material and a
/// Compose an `Introduction` from a subject's key material and a
/// `SignedLocator` of theirs, as a holder hands it on.
pub fn introduction(material: &[u8], signed_locator: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    emit_array_head(&mut out, 3);
    emit_uint(&mut out, 1);
    out.extend_from_slice(material);
    out.extend_from_slice(signed_locator);
    out
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
    let sealed = position.at(crate::tx::Seqno {
        series,
        counter: COUNTER_MAX,
    });
    signed_locator(identity, &sealed)
}
