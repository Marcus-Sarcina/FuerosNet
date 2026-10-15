//! A profile (`wire-format.md` §7.11; design §12.8): an alias and, where
//! the person chooses, an image, per subnet, signed by the one it is
//! about, hosted by the serving node, and in no transaction.  The two
//! messages that fetch one are here too; the publication is with the
//! other submissions (`submission`).

use crate::{Keyhash, Txid};
use rhtn_codec::cbor::*;
use rhtn_codec::encode::*;
use rhtn_codec::schema;
use rhtn_crypto::verify::{self, Lookup};

/// Stream-1 request type of a fetch (`wire-format.md` §9.2), read-only.
pub const REQUEST_PROFILE: u64 = 14;
/// `Profile` field 3: the subject's own profile in that subnet.
pub const KIND_OWN: u64 = 0;
/// `Profile` field 3: the subnet's profile, signed by whoever publishes
/// it, by convention the root (design §12.8).
pub const KIND_SUBNET: u64 = 1;
/// Image formats (`Profile` field 5).
pub const IMAGE_PNG: u64 = 1;
/// JPEG.
pub const IMAGE_JPEG: u64 = 2;
/// WebP.
pub const IMAGE_WEBP: u64 = 3;
/// `ProfileReply` field 3: nothing held for that triple.
pub const FAIL_UNKNOWN: u64 = 0;
/// The request was refused.
pub const FAIL_REFUSED: u64 = 1;

/// A `Profile` as the network reads it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Profile {
    /// Field 1: whose it is, and the signer.
    pub subject: Keyhash,
    /// Field 2: the subnet it is for (design §13.8).
    pub subnet: Txid,
    /// Field 3: [`KIND_OWN`] or [`KIND_SUBNET`].
    pub kind: u64,
    /// Field 4: the alias.  **Display only, never matched.**
    pub alias: String,
    /// Field 5: the image, a format tag and its bytes, where one is
    /// carried.
    pub image: Option<(u64, Vec<u8>)>,
    /// Field 6: when it was published; a later one replaces an earlier.
    pub published_at: u64,
    /// The profile's own bytes, which the signature covers.
    pub bytes: Vec<u8>,
}

impl Profile {
    /// Build one, signed classically under the profile tag over fields
    /// 1 to 6 (§7.11).
    pub fn build(
        subject: &rhtn_crypto::SigningIdentity,
        subnet: &Txid,
        kind: u64,
        alias: &str,
        image: Option<(u64, &[u8])>,
        published_at: u64,
    ) -> Vec<u8> {
        let mut payload = Vec::new();
        emit_map_head(&mut payload, 5 + usize::from(image.is_some()));
        emit_uint(&mut payload, 1);
        emit_bstr(&mut payload, &subject.public.keyhash);
        emit_uint(&mut payload, 2);
        emit_bstr(&mut payload, subnet);
        emit_uint(&mut payload, 3);
        emit_uint(&mut payload, kind);
        emit_uint(&mut payload, 4);
        emit_tstr(&mut payload, alias);
        if let Some((format, bytes)) = image {
            emit_uint(&mut payload, 5);
            emit_array_head(&mut payload, 2);
            emit_uint(&mut payload, format);
            emit_bstr(&mut payload, bytes);
        }
        emit_uint(&mut payload, 6);
        emit_uint(&mut payload, published_at);
        let sig = subject.sign1_ed_unnamed(rhtn_codec::cose::aad::PROFILE, &payload);
        let mut out = Vec::new();
        emit_map_head(&mut out, 6 + usize::from(image.is_some()));
        out.extend_from_slice(&payload[1..]);
        emit_uint(&mut out, 7);
        out.extend_from_slice(&sig);
        out
    }

    /// Parse one, held to the `Profile` kind.
    pub fn parse(b: &[u8]) -> Result<Self, String> {
        let item = parse_all(b).map_err(|e| e.0)?;
        schema::check_kind(b, "Profile", &item).map_err(|e| e.0)?;
        let Item::Map(m) = &item else {
            return Err("not a map".into());
        };
        let bytes32 = |k: u64| -> Result<[u8; 32], String> {
            match map_get(m, k) {
                Some(Item::Bytes(r)) if r.len() == 32 => {
                    <[u8; 32]>::try_from(&b[r.clone()]).map_err(|_| format!("field {k}"))
                }
                _ => Err(format!("field {k}")),
            }
        };
        let alias = match map_get(m, 4) {
            Some(Item::Text(r)) => std::str::from_utf8(&b[r.clone()])
                .map_err(|_| "alias not utf-8")?
                .to_string(),
            _ => return Err("field 4".into()),
        };
        let image = match map_get(m, 5) {
            None => None,
            Some(Item::Array(a)) if a.len() == 2 => {
                let format = as_uint(&a[0]).ok_or("image format")?;
                let Item::Bytes(r) = &a[1] else {
                    return Err("image bytes".into());
                };
                Some((format, b[r.clone()].to_vec()))
            }
            Some(_) => return Err("field 5".into()),
        };
        Ok(Profile {
            subject: bytes32(1)?,
            subnet: bytes32(2)?,
            kind: map_get(m, 3).and_then(as_uint).ok_or("field 3")?,
            alias,
            image,
            published_at: map_get(m, 6).and_then(as_uint).ok_or("field 6")?,
            bytes: b.to_vec(),
        })
    }

    /// Signed by the subject it names.
    pub fn verify<L: Lookup + ?Sized>(&self, ids: &L) -> Result<(), String> {
        verify::record(ids, "Profile", &self.bytes).map_err(|e| e.to_string())
    }
}

/// A fetch (`wire-format.md` §7.11): one triple, read-only.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProfileRequest {
    /// The subject.
    pub subject: Keyhash,
    /// The subnet.
    pub subnet: Txid,
    /// The kind.
    pub kind: u64,
    /// The nonce the reply echoes.
    pub nonce: [u8; 16],
}

impl ProfileRequest {
    /// The request's bytes.
    pub fn encode(&self) -> Vec<u8> {
        let mut out = Vec::new();
        emit_map_head(&mut out, 4);
        emit_uint(&mut out, 1);
        emit_bstr(&mut out, &self.subject);
        emit_uint(&mut out, 2);
        emit_bstr(&mut out, &self.subnet);
        emit_uint(&mut out, 3);
        emit_uint(&mut out, self.kind);
        emit_uint(&mut out, 4);
        emit_bstr(&mut out, &self.nonce);
        out
    }

    /// A request from its bytes.
    pub fn decode(b: &[u8]) -> Result<Self, String> {
        let item = parse_all(b).map_err(|e| e.0)?;
        schema::check_unsigned(schema::Family::ProfileRequest, b, 0).map_err(|e| e.0)?;
        let Item::Map(m) = &item else {
            return Err("not a map".into());
        };
        let bytes32 = |k: u64| -> Result<[u8; 32], String> {
            match map_get(m, k) {
                Some(Item::Bytes(r)) if r.len() == 32 => {
                    <[u8; 32]>::try_from(&b[r.clone()]).map_err(|_| format!("field {k}"))
                }
                _ => Err(format!("field {k}")),
            }
        };
        let nonce = match map_get(m, 4) {
            Some(Item::Bytes(r)) if r.len() == 16 => {
                <[u8; 16]>::try_from(&b[r.clone()]).map_err(|_| "nonce")?
            }
            _ => return Err("field 4".into()),
        };
        Ok(ProfileRequest {
            subject: bytes32(1)?,
            subnet: bytes32(2)?,
            kind: map_get(m, 3).and_then(as_uint).ok_or("field 3")?,
            nonce,
        })
    }
}

/// The answer: the profile held for the triple, or a code.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProfileReply {
    /// The nonce, echoed.
    pub nonce: [u8; 16],
    /// The profile's bytes, where one is held.
    pub profile: Option<Vec<u8>>,
    /// The failure code where none is.
    pub code: Option<u64>,
}

impl ProfileReply {
    /// The reply's bytes.
    pub fn encode(&self) -> Vec<u8> {
        let mut out = Vec::new();
        emit_map_head(
            &mut out,
            1 + usize::from(self.profile.is_some()) + usize::from(self.code.is_some()),
        );
        emit_uint(&mut out, 1);
        emit_bstr(&mut out, &self.nonce);
        if let Some(p) = &self.profile {
            emit_uint(&mut out, 2);
            out.extend_from_slice(p);
        }
        if let Some(c) = self.code {
            emit_uint(&mut out, 3);
            emit_uint(&mut out, c);
        }
        out
    }

    /// A reply from its bytes.
    pub fn decode(b: &[u8]) -> Result<Self, String> {
        let item = parse_all(b).map_err(|e| e.0)?;
        schema::check_unsigned(schema::Family::ProfileReply, b, 0).map_err(|e| e.0)?;
        let Item::Map(m) = &item else {
            return Err("not a map".into());
        };
        let nonce = match map_get(m, 1) {
            Some(Item::Bytes(r)) if r.len() == 16 => {
                <[u8; 16]>::try_from(&b[r.clone()]).map_err(|_| "nonce")?
            }
            _ => return Err("field 1".into()),
        };
        let profile = value_slice(b, 2).map(|r| b[r].to_vec());
        let code = map_get(m, 3).and_then(as_uint);
        if profile.is_none() && code.is_none() {
            return Err("a reply carries the profile or a code".into());
        }
        Ok(ProfileReply {
            nonce,
            profile,
            code,
        })
    }
}
