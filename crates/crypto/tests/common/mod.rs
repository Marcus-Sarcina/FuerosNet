//! Shared helpers for the corpus-driven tests: fixture access, identities,
//! and one `decode` entry point per corpus layer, so a test presents bytes
//! "at whichever layer the entry's expect.kind names".

#![allow(dead_code)]

use rhtn_codec::cbor::*;
use rhtn_codec::envelope;
use rhtn_codec::frame::{self, Stream};
use rhtn_codec::schema::{self, Family};
use rhtn_crypto::identity::testkit::test_identity;
use rhtn_crypto::{Identity, verify};

pub const NAMES: [&str; 25] = [
    "alice", "bob", "carol", "alice2", "w1", "w2", "w3", "w4", "w5", "w6", "w7", "w8", "w9", "w10",
    "w11", "w12", "w13", "w14", "w15", "w16", "c1", "c2", "c3", "c4", "c5",
];

pub fn corpus() -> serde_json::Value {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../test-vectors/corpus.json"
    );
    serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap()
}

pub fn identities() -> Vec<Identity> {
    NAMES.iter().map(|n| test_identity(n).public).collect()
}

/// What a holder keeps: the identities, and the delegations the corpus
/// carries as accepted `Delegation` fixtures, held against their
/// delegating keyhash (`wire-format.md` §10.1).  A subtree acknowledgement
/// signed under an instance's delegated key verifies through this.
pub struct Held {
    pub ids: Vec<Identity>,
    pub delegations: Vec<verify::Delegation>,
}

impl verify::Lookup for Held {
    fn identity(&self, keyhash: &[u8]) -> Option<&Identity> {
        self.ids.iter().find(|i| i.keyhash == keyhash)
    }
    fn delegated_key(&self, keyhash: &[u8]) -> Option<[u8; 32]> {
        self.delegations
            .iter()
            .find(|d| d.keyhash == keyhash)
            .map(|d| d.key)
    }
}

pub fn held() -> Held {
    let ids = identities();
    let mut delegations = Vec::new();
    for f in fixtures() {
        if f.kind == "Delegation" && f.outcome == "accept" {
            delegations.push(
                verify::delegation(ids.as_slice(), &f.bytes)
                    .unwrap_or_else(|e| panic!("{}: {e}", f.id)),
            );
        }
    }
    Held { ids, delegations }
}

pub struct Fixture {
    pub id: String,
    pub kind: String,
    pub outcome: String,
    pub layer: String,
    /// The reply family a reply entry belongs to, as the corpus states it.
    pub family: String,
    pub bytes: Vec<u8>,
}

pub fn fixtures() -> Vec<Fixture> {
    corpus()["entries"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|e| e["class"] == "bytes")
        .map(|e| Fixture {
            id: e["id"].as_str().unwrap().to_string(),
            kind: e["expect"]["kind"].as_str().unwrap_or("").to_string(),
            outcome: e["expect"]["outcome"].as_str().unwrap_or("").to_string(),
            layer: e["expect"]["layer"].as_str().unwrap_or("").to_string(),
            family: e["family"].as_str().unwrap_or("").to_string(),
            bytes: hex::decode(e["hex"].as_str().unwrap()).unwrap(),
        })
        .collect()
}

pub fn fixture(id: &str) -> Fixture {
    fixtures()
        .into_iter()
        .find(|f| f.id == id)
        .unwrap_or_else(|| panic!("no fixture {id}"))
}

/// The reply family a corpus reply entry belongs to, as the corpus states
/// it on the entry.
pub fn reply_family(id: &str) -> Family {
    family_by_name(&fixture(id).family).unwrap_or_else(|| panic!("{id}: no reply family stated"))
}

/// A reply family by the name the wire gives it.
pub fn family_by_name(name: &str) -> Option<Family> {
    Some(match name {
        "ResolveReply" => Family::ResolveReply,
        "ArchiveReply" => Family::ArchiveReply,
        "PrekeyReply" => Family::PrekeyReply,
        "CatalogReply" => Family::CatalogReply,
        "ResourceResponse" => Family::ResourceResponse,
        "CurrencyReply" => Family::CurrencyReply,
        "ResourceRegistrationReply" => Family::ResourceRegistrationReply,
        "SubmissionReply" => Family::SubmissionReply,
        _ => return None,
    })
}

pub fn unsigned_family(kind: &str) -> Option<Family> {
    Some(match kind {
        "ResolveReply" => Family::ResolveReply,
        "CatalogReply" => Family::CatalogReply,
        "ResourceResponse" => Family::ResourceResponse,
        "ArchiveRequest" => Family::ArchiveRequest,
        "PrekeyBatchRequest" => Family::PrekeyRequestOrBatch,
        _ => return None,
    })
}

/// A frame parses on whichever stream accepts it (the type spaces overlap
/// and the fixtures do not say which stream carried them).
pub fn parse_frame(raw: &[u8]) -> Result<frame::Frame, Error> {
    match frame::parse(Stream::Control, raw) {
        Ok(f) if f.family.is_some() => Ok(f),
        _ => frame::parse(Stream::Request, raw),
    }
}

/// Present `bytes` to the decoder at the layer `kind` names.  `Ok` is the
/// structural verdict `valid`; `Err` is `malformed`.  Signatures are checked
/// for envelopes and presentations, so `valid` there means verified.
pub fn decode(ids: &[Identity], id: &str, kind: &str, bytes: &[u8]) -> Result<(), String> {
    match kind {
        "frame" => parse_frame(bytes).map(|_| ()).map_err(|e| e.0.into()),
        "reply" => {
            parse_all(bytes).map_err(|e| e.0)?;
            schema::check_unsigned(reply_family(id), bytes, 0).map_err(|e| e.0.into())
        }
        "envelope" => verify::envelope(ids, bytes)
            .map(|_| ())
            .map_err(|e| e.to_string()),
        "presentation" => verify::presentation(ids, bytes).map_err(|e| e.to_string()),
        "body" => {
            let item = parse_all(bytes).map_err(|e| e.0)?;
            schema::check_body(bytes, &item).map_err(|e| e.0.into())
        }
        "e2e-payload" => {
            parse_all(bytes).map_err(|e| e.0)?;
            let fam = if id == "P-e2e-02" {
                Family::LateResponse
            } else {
                Family::KeyGrant
            };
            schema::check_unsigned(fam, bytes, 0).map_err(|e| e.0.into())
        }
        k => {
            let item = parse_all(bytes).map_err(|e| e.0)?;
            if let Some(f) = unsigned_family(k) {
                return schema::check_unsigned(f, bytes, 0).map_err(|e| e.0.into());
            }
            schema::check_kind(bytes, k, &item).map_err(|e| e.0.into())
        }
    }
}

/// Structural decode only (no signatures), for the envelope layer.
pub fn decode_envelope_structure(bytes: &[u8]) -> Result<(), String> {
    envelope::parse(bytes).map(|_| ()).map_err(|e| e.0.into())
}

/// The bytes a re-encoder must reproduce: the payload behind a frame's
/// length prefix, or the object itself.
pub fn canonical_span<'a>(kind: &str, bytes: &'a [u8]) -> &'a [u8] {
    if kind == "frame" { &bytes[4..] } else { bytes }
}

/// Wrap a payload in a frame's u32 length prefix.
pub fn framed(payload: &[u8]) -> Vec<u8> {
    let mut out = (payload.len() as u32).to_be_bytes().to_vec();
    out.extend_from_slice(payload);
    out
}

/// The body (field 3) of an envelope fixture.
pub fn body_of(env: &[u8]) -> Vec<u8> {
    env[value_slice(env, 3).unwrap()].to_vec()
}

/// Replace the value of integer `key` in the top-level map at `at` (0 for a
/// standalone map) with `new_value`, keeping every other entry's bytes.
pub fn replace_value(b: &[u8], at: usize, key: u64, new_value: &[u8]) -> Vec<u8> {
    let r = value_slice_at(b, at, key).expect("key present");
    let mut out = b[..r.start].to_vec();
    out.extend_from_slice(new_value);
    out.extend_from_slice(&b[r.end..]);
    out
}

/// Append raw entries to the map at `at` (they must sort after the last
/// key), fixing the head count.  `entries` is the concatenation of encoded
/// key/value pairs; `count` how many pairs it holds.
pub fn append_entries(b: &[u8], at: usize, entries: &[u8], count: u64) -> Vec<u8> {
    let p = Parser { b };
    let (mt, n, adv) = p.head(at).unwrap();
    assert_eq!(mt, 5);
    let (_, end) = p.item(at).unwrap();
    let mut out = b[..at].to_vec();
    rhtn_codec::encode::emit_head(&mut out, 5, n + count);
    out.extend_from_slice(&b[at + adv..end]);
    out.extend_from_slice(entries);
    out.extend_from_slice(&b[end..]);
    out
}

/// Remove integer `key` from the map at `at`, fixing the head count.
pub fn remove_key(b: &[u8], at: usize, key: u64) -> Vec<u8> {
    let p = Parser { b };
    let (mt, n, _adv) = p.head(at).unwrap();
    assert_eq!(mt, 5);
    let entries = map_entry_ranges(b, at).unwrap();
    let (_, end) = p.item(at).unwrap();
    let mut out = b[..at].to_vec();
    rhtn_codec::encode::emit_head(&mut out, 5, n - 1);
    for (kr, vr) in entries {
        let (k, _) = p.item(kr.start).unwrap();
        if matches!(k, Item::Uint(x) if x == key) {
            continue;
        }
        out.extend_from_slice(&b[kr.start..vr.end]);
    }
    out.extend_from_slice(&b[end..]);
    out
}

/// An encoded `key: h'c0ffee'` entry for an unknown key.
pub fn ext_entry(key: u64, value: &[u8]) -> Vec<u8> {
    let mut e = Vec::new();
    rhtn_codec::encode::emit_uint(&mut e, key);
    rhtn_codec::encode::emit_bstr(&mut e, value);
    e
}
