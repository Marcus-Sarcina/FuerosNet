//! Per-kind structural rules: the unsigned message families (§6, §7, §8,
//! §10, §11), the standalone signed records (§7), transaction bodies (§4),
//! and the extension bounds of §1.3.  Everything here decides from the bytes
//! and the parsed tree; nothing consults state.

use crate::bounds::*;
use crate::cbor::*;

/// Field types the unsigned schemas use.
#[derive(Clone, Copy)]
pub enum T {
    Keyhash,
    Nonce16,
    Bytes32,
    Uint,
    Bool,
    Bstr,
    Tstr(usize),
    Any,
    Locator,
    Path,
    Capabilities,
    SiblingRefs,
    NetworkPoints,
    ServingInfra,
    Referral,
    CatalogEntries,
    Envelopes,
    Keyhashes,
    CurrencyAttestation,
    PrekeyBundle,
    CatalogEntry,
    Scope,
    VerifierResponse,
}

/// One schema: (key, required, type).
pub type Fields = &'static [(u64, bool, T)];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Family {
    Attach,
    AttachAck,
    Heartbeat,
    SiblingUpdate,
    TopologyPush,
    TopologyMemo,
    ResolveRequest,
    ArchiveRequest,
    PrekeyRequestOrBatch,
    VerifierQuery,
    CatalogQuery,
    ResourceRequest,
    ResourceRegistration,
    CurrencyRequest,
    ResolveReply,
    CatalogReply,
    ResourceResponse,
    ArchiveReply,
    PrekeyReply,
    CurrencyReply,
    ResourceRegistrationReply,
    KeyGrant,
    LateResponse,
}

use T::*;
const ATTACH: Fields = &[(1, true, Keyhash), (2, false, CurrencyAttestation), (3, true, Capabilities)];
const ATTACH_ACK: Fields = &[(1, true, Uint), (2, false, SiblingRefs), (3, true, Uint), (4, true, Uint), (5, true, Capabilities)];
const HEARTBEAT: Fields = &[(1, true, Uint), (2, true, Uint)];
const SIBLING_UPDATE: Fields = &[(1, false, SiblingRefs)];
const TOPOLOGY_PUSH: Fields = &[(1, true, Uint), (2, true, Bstr)];
const TOPOLOGY_MEMO: Fields = &[(1, true, Keyhash), (2, true, Locator), (3, true, Uint), (4, true, Uint), (5, false, Keyhash)];
const RESOLVE_REQUEST: Fields = &[(1, true, Keyhash), (2, true, Keyhash), (3, true, Path), (4, true, Nonce16)];
const ARCHIVE_REQUEST: Fields = &[(1, true, Keyhash), (2, false, Bytes32), (3, true, Uint), (4, false, Uint), (5, true, Nonce16)];
const PREKEY_REQUEST: Fields = &[(1, true, Keyhash), (2, true, Uint), (3, true, Nonce16)];
const PREKEY_BATCH_REQUEST: Fields = &[(1, true, Keyhashes), (2, true, Nonce16)];
const CATALOG_QUERY: Fields = &[(1, false, Tstr(64)), (2, true, Nonce16)];
const RESOURCE_REQUEST: Fields = &[(1, true, Keyhash), (2, true, Bstr)];
const RESOURCE_REGISTRATION: Fields = &[(1, true, CatalogEntry), (2, false, Scope), (3, true, Nonce16)];
const CURRENCY_REQUEST: Fields = &[(1, true, Keyhash), (2, true, Nonce16)];
const RESOLVE_REPLY: Fields = &[(1, true, Nonce16), (2, true, Uint), (3, false, ServingInfra), (4, false, Uint), (5, false, Referral)];
const CATALOG_REPLY: Fields = &[(1, true, Nonce16), (2, true, CatalogEntries), (3, false, Tstr(64))];
const RESOURCE_RESPONSE: Fields = &[(1, true, Uint), (2, false, Bstr)];
const ARCHIVE_REPLY: Fields = &[(1, true, Nonce16), (2, true, Envelopes), (3, true, Bool), (4, false, Bytes32)];
const PREKEY_REPLY: Fields = &[(1, true, Nonce16), (2, false, PrekeyBundle), (3, false, Bstr), (4, false, Uint)];
const CURRENCY_REPLY: Fields = &[(1, true, Nonce16), (2, true, Uint), (3, false, CurrencyAttestation)];
const RESOURCE_REGISTRATION_REPLY: Fields = &[(1, true, Nonce16), (2, true, Uint)];
const KEY_GRANT: Fields = &[(1, true, Bytes32), (2, true, Bytes32), (3, true, Bytes32)];
const LATE_RESPONSE: Fields = &[(1, true, Bytes32), (2, true, Keyhash), (3, true, VerifierResponse)];
const LOCATOR: Fields = &[(1, true, Keyhash), (2, true, Path), (3, true, Any)];
const PATH: Fields = &[(1, true, Bstr), (2, true, Uint)];
const SIBLING_REF: Fields = &[(1, true, Keyhash), (2, true, NetworkPoints), (3, false, Any)];
const NETWORK_POINT: Fields = &[(1, true, Bstr), (2, false, Uint), (3, false, Uint)];
const SERVING_INFRA: Fields = &[(1, true, Keyhash), (2, true, NetworkPoints), (3, true, Path), (4, false, Any)];
const REFERRAL: Fields = &[(1, true, Keyhash), (2, true, NetworkPoints), (3, true, Uint), (4, false, Any)];

pub fn fields(f: Family) -> Option<Fields> {
    use Family::*;
    Some(match f {
        Attach => ATTACH,
        AttachAck => ATTACH_ACK,
        Heartbeat => HEARTBEAT,
        SiblingUpdate => SIBLING_UPDATE,
        TopologyPush => TOPOLOGY_PUSH,
        TopologyMemo => TOPOLOGY_MEMO,
        ResolveRequest => RESOLVE_REQUEST,
        ArchiveRequest => ARCHIVE_REQUEST,
        CatalogQuery => CATALOG_QUERY,
        ResourceRequest => RESOURCE_REQUEST,
        ResourceRegistration => RESOURCE_REGISTRATION,
        CurrencyRequest => CURRENCY_REQUEST,
        ResolveReply => RESOLVE_REPLY,
        CatalogReply => CATALOG_REPLY,
        ResourceResponse => RESOURCE_RESPONSE,
        ArchiveReply => ARCHIVE_REPLY,
        PrekeyReply => PREKEY_REPLY,
        CurrencyReply => CURRENCY_REPLY,
        ResourceRegistrationReply => RESOURCE_REGISTRATION_REPLY,
        KeyGrant => KEY_GRANT,
        LateResponse => LATE_RESPONSE,
        PrekeyRequestOrBatch | VerifierQuery => return None,
    })
}

fn bs<'a>(b: &'a [u8], it: &Item) -> Option<&'a [u8]> {
    match it {
        Item::Bytes(r) => Some(&b[r.clone()]),
        _ => None,
    }
}

/// An unsigned map at offset `at` against its schema: no unknown keys
/// (§1.2), every required key present, every value of its type, nested maps
/// likewise.  Walks the received bytes, so nested values are checked at
/// their own offsets.
pub fn check_map(b: &[u8], at: usize, schema: Fields) -> Result<(), Error> {
    check_map_with(b, at, schema, false)
}

/// A map nested in a signed object at offset `at` against its schema:
/// every required key present and every known value of its type, with
/// unknown keys kept as the extensions §1.2 preserves, bounded by the
/// caller's extension count.
pub fn check_map_signed(b: &[u8], at: usize, schema: Fields) -> Result<(), Error> {
    check_map_with(b, at, schema, true)
}

fn check_map_with(b: &[u8], at: usize, schema: Fields, signed: bool) -> Result<(), Error> {
    let p = Parser { b };
    if p.head(at)?.0 != 5 {
        return Err(Error("not a map"));
    }
    let entries = map_entry_ranges(b, at).ok_or(Error("map walk"))?;
    let mut present: Vec<u64> = Vec::new();
    for (kr, vr) in entries {
        let (k, _) = p.item(kr.start)?;
        let Item::Uint(key) = k else { return Err(Error("map key not uint")) };
        let Some((_, _, t)) = schema.iter().find(|(sk, _, _)| *sk == key) else {
            if signed {
                continue;
            }
            return Err(Error("unknown key on an unsigned message"));
        };
        check_type(b, vr.start, *t)?;
        present.push(key);
    }
    for (key, required, _) in schema {
        if *required && !present.contains(key) {
            return Err(Error("required field absent"));
        }
    }
    Ok(())
}

/// One `NetworkPoint` map (§4.4): its fields, a four-byte address, and a
/// port in range that is not the default written out; in a signed object
/// its unknown keys are extensions.
fn network_point_at(b: &[u8], at: usize, signed: bool) -> Result<(), Error> {
    check_map_with(b, at, NETWORK_POINT, signed)?;
    let (Item::Map(ref m), _) = (Parser { b }).item(at)? else { unreachable!() };
    if bs(b, map_get(m, 1).unwrap()).map(|s| s.len()) != Some(4) {
        return Err(Error("address width"));
    }
    if let Some(port) = map_get(m, 3).and_then(as_uint)
        && (port == 0 || port > 65535 || port == 7431) {
            return Err(Error("port invalid"));
        }
    Ok(())
}

/// The packed-path invariant (§2.1): at most 24 nibbles, exactly
/// `ceil(nibbles / 2)` bytes, every nibble 0-9, and on an odd count the
/// unused low nibble of the last byte zero.  One logical path has one
/// encoding; a path that departs from this is malformed, and a decoder
/// that admitted one would index past its bytes.
pub fn packed_path(packed: &[u8], nibbles: u64) -> Result<(), Error> {
    if nibbles > PATH_NIBBLES {
        return Err(Error("path over 24 nibbles"));
    }
    let n = nibbles as usize;
    if packed.len() != n.div_ceil(2) {
        return Err(Error("path byte length is not ceil(nibbles / 2)"));
    }
    for i in 0..n {
        let v = if i % 2 == 0 { packed[i / 2] >> 4 } else { packed[i / 2] & 0x0f };
        if v > 9 {
            return Err(Error("path nibble over 9"));
        }
    }
    if n % 2 == 1 && packed[n / 2] & 0x0f != 0 {
        return Err(Error("path pad nibble not zero"));
    }
    Ok(())
}

/// The value at offset `at` against type `t`.
pub fn check_type(b: &[u8], at: usize, t: T) -> Result<(), Error> {
    let p = Parser { b };
    let (v, end) = p.item(at)?;
    let slice = &b[at..end];
    let nested = |kind: &str| -> Result<(), Error> { check_kind(slice, kind, &parse_all(slice)?) };
    match t {
        Keyhash | Bytes32 => {
            if bs(b, &v).map(|s| s.len()) != Some(32) {
                return Err(Error("keyhash width"));
            }
        }
        Nonce16 => {
            if bs(b, &v).map(|s| s.len()) != Some(16) {
                return Err(Error("nonce width"));
            }
        }
        Uint => {
            as_uint(&v).ok_or(Error("not uint"))?;
        }
        Bool => {
            if !matches!(v, Item::Bool(_)) {
                return Err(Error("not bool"));
            }
        }
        Bstr => {
            bs(b, &v).ok_or(Error("not bstr"))?;
        }
        Tstr(max) => match v {
             Item::Text(ref r) if !r.is_empty() && r.len() <= max => {}
            _ => return Err(Error("tstr shape")),
        },
        Any => {}
        Locator => check_map(b, at, LOCATOR)?,
        Path => {
            check_map(b, at, PATH)?;
            let Item::Map(ref m) = v else { unreachable!() };
            let n = map_get(m, 2).and_then(as_uint).ok_or(Error("path nibble count"))?;
            let Some(packed) = map_get(m, 1).and_then(|it| bs(b, it)) else { return Err(Error("path not bstr")) };
            packed_path(packed, n)?;
        }
        Capabilities => {
             let Item::Map(ref m) = v else { return Err(Error("capabilities not map")) };
            if m.len() > CAPABILITIES_ENTRIES {
                return Err(Error("over 64 capability entries"));
            }
            for (k, val) in m {
                as_uint(k).ok_or(Error("capability id not uint"))?;
                let Some(s) = bs(b, val) else { return Err(Error("capability value not bstr")) };
                if s.len() > CAPABILITIES_VALUE_BYTES {
                    return Err(Error("capability value over 1024"));
                }
            }
        }
        SiblingRefs => {
            let refs = array_item_ranges(b, at).ok_or(Error("sibling refs not array"))?;
            if refs.is_empty() || refs.len() > SIBLING_REFS {
                return Err(Error("sibling ref count"));
            }
            for r in refs {
                check_map(b, r.start, SIBLING_REF)?;
            }
        }
        NetworkPoints => {
            let pts = array_item_ranges(b, at).ok_or(Error("network points not array"))?;
            if pts.is_empty() || pts.len() > NETWORK_POINTS_PER_RECORD {
                return Err(Error("network point count"));
            }
            for r in pts {
                network_point_at(b, r.start, false)?;
            }
        }
        ServingInfra => check_map(b, at, SERVING_INFRA)?,
        Referral => {
            check_map(b, at, REFERRAL)?;
             let Item::Map(ref m) = v else { unreachable!() };
            if map_get(m, 3).and_then(as_uint) == Some(0) {
                return Err(Error("referral advances nothing"));
            }
        }
        CatalogEntries => {
            let entries = array_item_ranges(b, at).ok_or(Error("entries not array"))?;
            if entries.len() > CATALOG_REPLY_ENTRIES {
                return Err(Error("catalog reply over 111 entries"));
            }
            for r in entries {
                let es = &b[r];
                check_kind(es, "CatalogEntry", &parse_all(es)?)?;
            }
        }
        Envelopes => {
             let Item::Array(ref a) = v else { return Err(Error("envelopes not array")) };
            if a.len() > ARCHIVE_SUBSET_REFS {
                return Err(Error("archive reply over 256"));
            }
        }
        Keyhashes => {
             let Item::Array(ref a) = v else { return Err(Error("population not array")) };
            if a.len() < 2 || a.len() > 256 {
                return Err(Error("population out of range"));
            }
            let mut prev: Option<&[u8]> = None;
            for k in a {
                let Some(s) = bs(b, k) else { return Err(Error("population entry")) };
                if s.len() != 32 {
                    return Err(Error("keyhash width"));
                }
                if prev.is_some_and(|q| s <= q) {
                    return Err(Error("population unsorted"));
                }
                prev = Some(s);
            }
        }
        CurrencyAttestation => nested("CurrencyAttestation")?,
        PrekeyBundle => nested("PrekeyBundle")?,
        CatalogEntry => nested("CatalogEntry")?,
        Scope => nested("Scope")?,
        VerifierResponse => nested("VerifierResponse")?,
    }
    Ok(())
}

/// The family-level check a frame or reply body at offset `at` must pass.
pub fn check_unsigned(f: Family, b: &[u8], at: usize) -> Result<(), Error> {
    let p = Parser { b };
    match f {
        Family::PrekeyRequestOrBatch => {
             let (Item::Map(ref m), _) = p.item(at)? else { return Err(Error("not a map")) };
            let schema = if matches!(map_get(m, 1), Some(Item::Array(_))) { PREKEY_BATCH_REQUEST } else { PREKEY_REQUEST };
            check_map(b, at, schema)?;
            if schema.len() == 3 && map_get(m, 2).and_then(as_uint).unwrap_or(0) > 1 {
                return Err(Error("prekey request mode"));
            }
            Ok(())
        }
        Family::VerifierQuery => {
            let parts = array_item_ranges(b, at).ok_or(Error("request-4 body not array"))?;
            if parts.len() != 3 {
                return Err(Error("request-4 arity"));
            }
            let qs = &b[parts[0].clone()];
            check_kind(qs, "VerificationQuery", &parse_all(qs)?)?;
            let (sb, _) = p.item(parts[2].start)?;
            if as_uint(&sb).unwrap_or(9) > 2 {
                return Err(Error("selection basis out of range"));
            }
            Ok(())
        }
        _ => {
            let schema = fields(f).unwrap();
            check_map(b, at, schema)?;
             let (Item::Map(ref m), _) = p.item(at)? else { unreachable!() };
            let m = &m;
            match f {
                Family::ArchiveRequest => {
                    let mx = map_get(m, 3).and_then(as_uint).unwrap_or(0);
                    if !(1..=256).contains(&mx) {
                        return Err(Error("max_records out of range"));
                    }
                }
                Family::ResolveReply => {
                    let code = map_get(m, 2).and_then(as_uint).unwrap_or(9);
                    if code > 2 {
                        return Err(Error("code out of range"));
                    }
                    let want = [3u64, 4, 5][code as usize];
                    for k in [3u64, 4, 5] {
                        if map_get(m, k).is_some() != (k == want) {
                            return Err(Error("reply field does not follow its code"));
                        }
                    }
                }
                Family::ResourceResponse => {
                    let s = map_get(m, 1).and_then(as_uint).unwrap_or(9);
                    if s > 5 {
                        return Err(Error("status out of range"));
                    }
                    if (s == 0) != map_get(m, 2).is_some() {
                        return Err(Error("response body does not follow its status"));
                    }
                }
                Family::CurrencyReply => {
                    let code = map_get(m, 2).and_then(as_uint).unwrap_or(9);
                    if code > 1 {
                        return Err(Error("code out of range"));
                    }
                    if (code == 0) != map_get(m, 3).is_some() {
                        return Err(Error("attestation does not follow its code"));
                    }
                }
                Family::ResourceRegistrationReply => {
                    if map_get(m, 2).and_then(as_uint).unwrap_or(9) > 1 {
                        return Err(Error("code out of range"));
                    }
                }
                Family::PrekeyReply => {
                    if map_get(m, 2).is_some() == map_get(m, 4).is_some() {
                        return Err(Error("bundle and failure code are alternatives"));
                    }
                    if map_get(m, 4).and_then(as_uint).unwrap_or(0) > 1 {
                        return Err(Error("failure code out of range"));
                    }
                }
                Family::TopologyPush => {
                    if map_get(m, 1).and_then(as_uint).unwrap_or(9) > 1 {
                        return Err(Error("body kind out of range"));
                    }
                }
                Family::TopologyMemo => {
                    if map_get(m, 3).and_then(as_uint).unwrap_or(99) > 9 {
                        return Err(Error("memo slot outside the nibble range"));
                    }
                }
                Family::AttachAck => {
                    if map_get(m, 1).and_then(as_uint).unwrap_or(9) > 1 {
                        return Err(Error("attach mode out of range"));
                    }
                    let iv = map_get(m, 3).and_then(as_uint).unwrap_or(0);
                    if !(1..=3600).contains(&iv) {
                        return Err(Error("heartbeat interval outside 1..=3600"));
                    }
                }
                _ => {}
            }
            Ok(())
        }
    }
}

/// (signature slot, role tag, signer field) per standalone signed kind (§7).
pub fn sign1_profile(kind: &str) -> Option<(u64, &'static [u8], u64)> {
    use crate::cose::aad;
    Some(match kind {
        "CurrencyAttestation" => (7, aad::CURRENCY, 6),
        "CatalogEntry" => (8, aad::CATALOG, 2),
        "AbuseReport" => (5, aad::ABUSE, 1),
        "AnchorEntry" => (5, aad::ANCHOR, 1),
        "SubtreeAck" => (5, aad::SUBTREE_ACK, 2),
        "PrekeyBundle" => (5, aad::PREKEY, 1),
        "EndpointRecord" => (4, aad::ENDPOINTS, 1),
        "SignedLocator" => (3, aad::LOCATOR, 1),
        _ => return None,
    })
}

/// Unknown-extension bounds for one signed map (§1.3): at most 16 keys the
/// schema does not name, each value at most 1024 encoded bytes.  `known`
/// says which keys the map's schema defines.  Values were already parsed
/// under the profile's rules, so a non-deterministic slice never reaches here.
pub fn extension_bounds(b: &[u8], map_at: usize, known: impl Fn(u64) -> bool) -> Result<(), Error> {
    let entries = map_entry_ranges(b, map_at).ok_or(Error("not a map"))?;
    let p = Parser { b };
    let mut unknown = 0usize;
    for (kr, vr) in entries {
        let (k, _) = p.item(kr.start)?;
        let is_known = matches!(k, Item::Uint(x) if known(x));
        if !is_known {
            unknown += 1;
            if vr.len() > UNKNOWN_VALUE_BYTES {
                return Err(Error("extension value over 1024"));
            }
        }
    }
    if unknown > UNKNOWN_KEYS_PER_MAP {
        return Err(Error("over 16 extension keys"));
    }
    Ok(())
}

/// Extension bounds for the value at `key` of the map at `map_at`: the map
/// itself, or every map in an array there.  §1.3 counts per map, so a
/// nested map is bounded on its own.
fn nested_extension_bounds(b: &[u8], map_at: usize, key: u64, known: &dyn Fn(u64) -> bool) -> Result<(), Error> {
    let Some(r) = value_slice_at(b, map_at, key) else { return Ok(()) };
    let p = Parser { b };
    match p.item(r.start)?.0 {
        Item::Map(_) => extension_bounds(b, r.start, known),
        Item::Array(_) => {
            for er in array_item_ranges(b, r.start).ok_or(Error("nested array walk"))? {
                if matches!(p.item(er.start)?.0, Item::Map(_)) {
                    extension_bounds(b, er.start, known)?;
                }
            }
            Ok(())
        }
        _ => Ok(()),
    }
}

/// The extension bounds a signed record carries (§1.3, per map): the
/// record's own map, and the maps nested in it, each against the keys its
/// schema names.
fn record_extension_bounds(b: &[u8], kind: &str, item: &Item) -> Result<(), Error> {
    if !matches!(item, Item::Map(_)) {
        return Ok(());
    }
    let top: Option<u64> = match kind {
        "CurrencyAttestation" => Some(7),
        "AnchorEntry" | "SubtreeAck" | "PrekeyBundle" | "AbuseReport" | "Witness" => Some(5),
        "EndpointRecord" => Some(4),
        "CatalogEntry" => Some(9),
        "KeyGrant" | "SignedLocator" | "LateResponse" | "Locator" | "NetworkPoint" => Some(3),
        "VerifierResponse" => Some(10),
        _ => None,
    };
    let Some(top) = top else { return Ok(()) };
    extension_bounds(b, 0, |k| (1..=top).contains(&k))?;
    match kind {
        // network points, and a locator, are maps of their own
        "AnchorEntry" | "EndpointRecord" | "SignedLocator" => nested_extension_bounds(b, 0, 2, &|k| (1..=3).contains(&k)),
        _ => Ok(()),
    }
}

/// Per-kind validation for the signed records and transaction bodies the
/// corpus names.  `b` holds the bytes the item's ranges index.
pub fn check_kind(b: &[u8], kind: &str, item: &Item) -> Result<(), Error> {
    record_extension_bounds(b, kind, item)?;
    match kind {
        "VerifierResponse" => {
            let Item::Map(m) = item else { return Err(Error("not map")) };
            if map_get(m, 4).and_then(as_uint).ok_or(Error("no result"))? > 3 {
                return Err(Error("result out of range"));
            }
            if map_get(m, 5).and_then(as_uint).is_some_and(|x| x > 2) {
                return Err(Error("basis out of range"));
            }
            match map_get(m, 10).and_then(as_uint) {
                Some(sb) if sb > 3 => return Err(Error("selection_basis out of range")),
                Some(_) => {}
                None => return Err(Error("selection_basis required")),
            }
            map_get(m, 7).ok_or(Error("consent required"))?;
            Ok(())
        }
        "Locator" => check_type(b, 0, Locator),
        "SignedLocator" => {
            let Item::Map(m) = item else { return Err(Error("not map")) };
            if bs(b, map_get(m, 1).ok_or(Error("subject"))?).map(|s| s.len()) != Some(32) {
                return Err(Error("keyhash width"));
            }
            Ok(())
        }
        "NetworkPoint" => {
            let Item::Map(m) = item else { return Err(Error("not map")) };
            if let Some(port) = map_get(m, 3).and_then(as_uint)
                && (port == 0 || port > 65535 || port == 7431) {
                    return Err(Error("port invalid"));
                }
            Ok(())
        }
        "LocationEvidence" => {
            let Item::Map(m) = item else { return Err(Error("not map")) };
            let Item::Array(asserted) = map_get(m, 1).ok_or(Error("asserted"))? else {
                return Err(Error("asserted not array"));
            };
            if asserted.len() > ASSERTED_LOCATIONS_PER_RECORD {
                return Err(Error("asserted over 4"));
            }
            for a in asserted {
                let Item::Map(am) = a else { return Err(Error("assert map")) };
                let Item::Text(g) = map_get(am, 2).ok_or(Error("geohash"))? else {
                    return Err(Error("geohash not tstr"));
                };
                let gh = &b[g.clone()];
                if !(gh.len() == 3 || gh.len() == 4) || !gh.iter().all(|c| b"0123456789bcdefghjkmnpqrstuvwxyz".contains(c)) {
                    return Err(Error("geohash malformed"));
                }
            }
            if let Some(Item::Array(cor)) = map_get(m, 2)
                && cor.len() > CORROBORATIONS_PER_RECORD {
                    return Err(Error("corroborations over 16"));
                }
            Ok(())
        }
        "Proximity" => {
            let Item::Map(m) = item else { return Err(Error("not map")) };
            let Item::Array(ch) = map_get(m, 1).ok_or(Error("channels"))? else {
                return Err(Error("channels not array"));
            };
            if ch.is_empty() || ch.len() > PROXIMITY_CHANNELS_PER_RECORD {
                return Err(Error("channel count"));
            }
            Ok(())
        }
        "Scope" => match item {
            Item::Uint(3) => Err(Error("retired tag")),
            Item::Uint(_) => Ok(()),
            Item::Array(a) if a.len() == 2 => {
                if let Item::Array(list) = &a[1] {
                    if list.len() > EXPLICIT_SCOPE_KEYHASHES {
                        return Err(Error("scope list over 256"));
                    }
                    let mut prev: Option<&[u8]> = None;
                    for k in list {
                        let kb = bs(b, k).ok_or(Error("scope entry"))?;
                        if prev.is_some_and(|p| kb <= p) {
                            return Err(Error("scope list unsorted/dup"));
                        }
                        prev = Some(kb);
                    }
                }
                Ok(())
            }
            _ => Err(Error("scope shape")),
        },
        "Capabilities" => check_type(b, 0, Capabilities),
        "CatalogEntry" => {
            let Item::Map(m) = item else { return Err(Error("not map")) };
            if b.len() > CATALOG_ENTRY_BYTES {
                return Err(Error("entry over 2048"));
            }
            for f in [1u64, 2, 3, 4, 5, 8] {
                map_get(m, f).ok_or(Error("missing required"))?;
            }
            Ok(())
        }
        "CurrencyAttestation" => {
            let Item::Map(m) = item else { return Err(Error("not map")) };
            if map_get(m, 5).and_then(as_uint).ok_or(Error("role"))? > 3 {
                return Err(Error("role out of range"));
            }
            map_get(m, 6).ok_or(Error("issuer required"))?;
            Ok(())
        }
        "ResolveReply" => check_unsigned(Family::ResolveReply, b, 0),
        "ResourceResponse" => check_unsigned(Family::ResourceResponse, b, 0),
        "ArchiveRequest" => check_unsigned(Family::ArchiveRequest, b, 0),
        "PrekeyBatchRequest" => check_unsigned(Family::PrekeyRequestOrBatch, b, 0),
        "CatalogReply" => check_unsigned(Family::CatalogReply, b, 0),
        "PrekeyBundle" => {
            let Item::Map(m) = item else { return Err(Error("not map")) };
            if let Some(Item::Bytes(r)) = map_get(m, 3)
                && r.len() > PREKEY_BUNDLE_BLOB {
                    return Err(Error("blob over 4KB"));
                }
            Ok(())
        }
        "VerificationQuery" => {
            let Item::Map(m) = item else { return Err(Error("not map")) };
            let qid = match map_get(m, 6) {
                Some(Item::Bytes(r)) => b[r.clone()].to_vec(),
                _ => return Err(Error("no query_id")),
            };
            let f15 = map_without_key(b, 6).ok_or(Error("fields 1-5 and 7"))?;
            if crate::cose::sha256(&f15).to_vec() != qid {
                return Err(Error("query_id does not recompute"));
            }
            match map_get(m, 7) {
                Some(Item::Bytes(v)) if v.len() == 32 => {}
                _ => return Err(Error("field 7 (addressed verifier) required")),
            }
            if map_get(m, 5).and_then(as_uint).is_some_and(|v| v > 65535) {
                return Err(Error("template version over uint16"));
            }
            Ok(())
        }
        "Witness" => {
            let Item::Map(m) = item else { return Err(Error("not map")) };
            if map_get(m, 4).is_some() || map_get(m, 5).is_some() {
                return Err(Error("retired witness key"));
            }
            Ok(())
        }
        "body" => check_body(b, item),
        _ => Ok(()),
    }
}

/// A transaction body (§4) met without its envelope, as a fixture presents
/// one: the type its shape implies, then that type's rules.  Inside an
/// envelope the type is named, and [`check_body_of_type`] takes it from
/// there rather than guessing.
pub fn check_body(b: &[u8], item: &Item) -> Result<(), Error> {
    let Item::Map(m) = item else { return Err(Error("not map")) };
    let tx_type = match map_get(m, 3) {
        Some(Item::Map(_)) if matches!(map_get(m, 4), Some(Item::Map(_))) => 4,
        Some(Item::Map(_)) => 1,
        Some(Item::Array(a)) if a.iter().all(|x| matches!(x, Item::Map(_))) && map_get(m, 6).is_some() => 5,
        Some(Item::Array(_)) if matches!(map_get(m, 4), Some(Item::Array(_))) => 7,
        Some(Item::Array(_)) => 2,
        Some(Item::Uint(_)) => 3,
        _ => return Err(Error("field 3 required")),
    };
    check_body_of_type(b, item, tx_type)
}

/// A `seqno` pair (§2.3): `[series, counter]`, two uints.
fn seqno_of(it: Option<&Item>) -> Result<(u64, u64), Error> {
    let Some(Item::Array(a)) = it else { return Err(Error("seqno not an array")) };
    if a.len() != 2 {
        return Err(Error("seqno arity"));
    }
    Ok((as_uint(&a[0]).ok_or(Error("seqno series"))?, as_uint(&a[1]).ok_or(Error("seqno counter"))?))
}

/// The 32-byte value at `key` of the map `m` over `b`, where there is one.
fn keyhash_at<'a>(b: &'a [u8], m: &[(Item, Item)], key: u64) -> Option<&'a [u8]> {
    match map_get(m, key) {
        Some(Item::Bytes(r)) if r.len() == 32 => Some(&b[r.clone()]),
        _ => None,
    }
}

/// A transaction body against the rules of the type its envelope names
/// (§4): the fields the type requires in the shapes it gives them, and
/// every consistency rule a validator checks from the object alone.
pub fn check_body_of_type(b: &[u8], item: &Item, tx_type: u64) -> Result<(), Error> {
    let Item::Map(m) = item else { return Err(Error("not map")) };
    let Item::Array(lists) = map_get(m, 0).ok_or(Error("key 0"))? else {
        return Err(Error("key0 not array"));
    };
    for l in lists {
        let Item::Array(hs) = l else { return Err(Error("list")) };
        if hs.is_empty() || hs.len() > MERGE_BACK_POINTERS_PER_SIGNER {
            return Err(Error("back-pointer bound"));
        }
    }
    // extension bounds on the body map (§1.3): bodies name keys 0 through 9
    extension_bounds(b, 0, |k| k <= 9)?;
    // every two-party type names two distinct parties in fields 1 and 2
    // (§4.1): a node cannot hold authority over itself
    if matches!(tx_type, 1 | 2 | 3 | 4 | 7) {
        let (Some(a), Some(p)) = (keyhash_at(b, m, 1), keyhash_at(b, m, 2)) else {
            return Err(Error("fields 1 and 2 are keyhashes"));
        };
        if a == p {
            return Err(Error("the two parties are one identity"));
        }
    }
    match tx_type {
        1 => {
            let Some(Item::Map(loc)) = map_get(m, 3) else { return Err(Error("adoption field 3 not a locator")) };
            let r3 = value_slice(b, 3).ok_or(Error("field 3"))?;
            // the locator's own shape, its packed path included (§2.1,
            // §2.3), its unknown keys the extensions a signed body keeps
            check_map_signed(b, r3.start, LOCATOR)?;
            extension_bounds(b, r3.start, |k| (1..=3).contains(&k))?;
            map_get(m, 4).and_then(as_uint).ok_or(Error("adoption timestamp uint"))?;
            if seqno_of(map_get(loc, 3))?.1 != 0 {
                return Err(Error("adoption counter not 0"));
            }
            // exactly one evidence form (§4.1, design §6.1.1): a recovery's
            // own block, a presence record's txid, or a former patron's
            // statement; none, or more than one, is malformed
            if [6u64, 8, 9].iter().filter(|k| map_get(m, **k).is_some()).count() != 1 {
                return Err(Error("an adoption carries exactly one of fields 6, 8 and 9"));
            }
            nested_extension_bounds(b, 0, 6, &|k| (1..=3).contains(&k))?;
            nested_extension_bounds(b, 0, 9, &|k| (1..=2).contains(&k))?;
            if let Some(Item::Map(rm)) = map_get(m, 6) {
                let r6 = value_slice(b, 6).ok_or(Error("field 6"))?;
                nested_extension_bounds(b, r6.start, 2, &|k| (1..=10).contains(&k))?;
                check_recovery(b, m, rm)?;
            } else if map_get(m, 6).is_some() {
                return Err(Error("field 6 not a map"));
            }
            if map_get(m, 8).is_some() && keyhash_at(b, m, 8).is_none() {
                return Err(Error("field 8 not a txid"));
            }
            if let Some(Item::Map(tm)) = map_get(m, 9) {
                if keyhash_at(b, tm, 1).is_none() {
                    return Err(Error("transfer field 1 not a keyhash"));
                }
                if !matches!(map_get(tm, 2), Some(Item::Array(_))) {
                    return Err(Error("transfer field 2 not a COSE_Sign"));
                }
            } else if map_get(m, 9).is_some() {
                return Err(Error("field 9 not a map"));
            }
            Ok(())
        }
        2 => {
            seqno_of(map_get(m, 3))?;
            map_get(m, 4).and_then(as_uint).ok_or(Error("departure timestamp uint"))?;
            if map_get(m, 5).is_some() && map_get(m, 5).and_then(as_uint).ok_or(Error("reason code uint"))? > 63 {
                return Err(Error("departure reason code out of space"));
            }
            Ok(())
        }
        3 => {
            map_get(m, 3).and_then(as_uint).ok_or(Error("disavowal timestamp uint"))?;
            if map_get(m, 4).is_some() && map_get(m, 4).and_then(as_uint).ok_or(Error("code uint"))? > 63 {
                return Err(Error("disavowal code out of space"));
            }
            Ok(())
        }
        4 => {
            // both network points, each in the shape §4.4 gives them
            for k in [3u64, 4] {
                let r = value_slice(b, k).ok_or(Error("peering network point"))?;
                network_point_at(b, r.start, true)?;
            }
            let r3 = value_slice(b, 3).ok_or(Error("field 3"))?;
            extension_bounds(b, r3.start, |k| (1..=3).contains(&k))?;
            nested_extension_bounds(b, 0, 4, &|k| (1..=3).contains(&k))?;
            map_get(m, 5).and_then(as_uint).ok_or(Error("peering timestamp uint"))?;
            if let Some(Item::Array(audits)) = map_get(m, 7)
                && audits.len() > PEERING_AUDIT_HISTORY {
                    return Err(Error("audits over 8"));
                }
            // the proof of presence between the peers is required, and
            // unconditionally so (§4.4)
            if keyhash_at(b, m, 8).is_none() {
                return Err(Error("a peering carries its presence record in field 8"));
            }
            Ok(())
        }
        5 => check_presence(b, m, lists),
        7 => {
            seqno_of(map_get(m, 3))?;
            if seqno_of(map_get(m, 4))?.1 != 0 {
                return Err(Error("reissue new series counter not 0"));
            }
            map_get(m, 5).and_then(as_uint).ok_or(Error("reissue timestamp uint"))?;
            Ok(())
        }
        _ => Err(Error("unknown transaction type")),
    }
}

/// The consistency rules of a `Recovery` block (§4.1), each checkable
/// from the adoption alone: the prior key differs from the new one; every
/// response names the new key as its subject and the prior key in field 8;
/// no verifier is its own subject; responses sort by verifier with no
/// repeat; each claims the met basis; and at least one is a match.
fn check_recovery(b: &[u8], m: &[(Item, Item)], rm: &[(Item, Item)]) -> Result<(), Error> {
    let node = keyhash_at(b, m, 1).ok_or(Error("node"))?;
    let prior = keyhash_at(b, rm, 1).ok_or(Error("recovery prior key"))?;
    if prior == node {
        return Err(Error("recovery prior key equals the new key"));
    }
    let Some(Item::Array(resp)) = map_get(rm, 2) else { return Err(Error("recovery responses not array")) };
    if resp.is_empty() {
        return Err(Error("a recovery carries at least one response"));
    }
    if resp.len() > VERIFIER_RESPONSES_PER_RECOVERY {
        return Err(Error("recovery responses over 32"));
    }
    if !matches!(map_get(rm, 3), Some(Item::Array(_))) {
        return Err(Error("recovery old-key proof not a COSE_Sign"));
    }
    let mut prev: Option<&[u8]> = None;
    let mut matched = false;
    for r in resp {
        let Item::Map(x) = r else { return Err(Error("response not map")) };
        let verifier = keyhash_at(b, x, 1).ok_or(Error("response verifier"))?;
        let subject = keyhash_at(b, x, 2).ok_or(Error("response subject"))?;
        if subject != node {
            return Err(Error("a response names a subject other than the adopted node"));
        }
        if verifier == subject {
            return Err(Error("a response's verifier is its subject"));
        }
        if keyhash_at(b, x, 8) != Some(prior) {
            return Err(Error("a response's field 8 differs from the prior key"));
        }
        if map_get(x, 10).and_then(as_uint) != Some(0) {
            return Err(Error("a recovery response's selection basis is not met"));
        }
        if map_get(x, 4).and_then(as_uint) == Some(0) {
            matched = true;
        }
        if prev.is_some_and(|p| verifier <= p) {
            return Err(Error("recovery responses unsorted or repeating a verifier"));
        }
        prev = Some(verifier);
    }
    if !matched {
        return Err(Error("a recovery carries at least one match"));
    }
    Ok(())
}

/// A presence record's body rules (§3.2, §4.5): two distinct participants,
/// the subtype's conditional fields, the witness floor, the responses
/// bound, and a formation's genesis back-pointers.
fn check_presence(b: &[u8], m: &[(Item, Item)], lists: &[Item]) -> Result<(), Error> {
    nested_extension_bounds(b, 0, 3, &|k| k == 1)?;
    nested_extension_bounds(b, 0, 4, &|k| (1..=5).contains(&k))?;
    nested_extension_bounds(b, 0, 5, &|k| (1..=10).contains(&k))?;
    if map_get(m, 7).is_some() {
        return Err(Error("retired body key 7"));
    }
    map_get(m, 8).ok_or(Error("presence field 8 required"))?;
    let Some(Item::Array(parts)) = map_get(m, 3) else { return Err(Error("participants not array")) };
    if parts.len() != 2 {
        return Err(Error("a presence record names two participants"));
    }
    let mut keys = Vec::new();
    for p in parts {
        let Item::Map(pm) = p else { return Err(Error("participant not map")) };
        keys.push(keyhash_at(b, pm, 1).ok_or(Error("participant keyhash"))?);
    }
    if keys[0] == keys[1] {
        return Err(Error("the two participant identities are one"));
    }
    let sub = map_get(m, 6).and_then(as_uint).ok_or(Error("subtype uint"))?;
    if sub > 1 {
        return Err(Error("subtype out of range"));
    }
    let s = map_get(m, 1).and_then(as_uint).ok_or(Error("started_at uint"))?;
    let f = map_get(m, 2).and_then(as_uint).ok_or(Error("finalized_at uint"))?;
    if f < s || f - s > 86_400 {
        return Err(Error("finalization gap"));
    }
    if sub == 1 {
        // a formation (§3.2, §4.5): no witnesses, no responses, and each
        // participant's key 0 list exactly the genesis value, a key
        // appearing in at most one formation record, its first
        if map_get(m, 4).is_some() || map_get(m, 5).is_some() {
            return Err(Error("a formation carries no witnesses and no responses"));
        }
        if lists.len() != 2 {
            return Err(Error("a formation carries one back-pointer list per participant"));
        }
        for (l, k) in lists.iter().zip(&keys) {
            let Item::Array(hs) = l else { return Err(Error("list")) };
            let genesis = crate::cose::sha256(k);
            let ok = hs.len() == 1 && matches!(&hs[0], Item::Bytes(r) if b[r.clone()] == genesis[..]);
            if !ok {
                return Err(Error("a formation's back-pointers are the genesis value"));
            }
        }
        return Ok(());
    }
    // a normal record carries witnesses, one to sixteen, and at least one
    // attesting that the protocol ran and both were responsive
    let Some(Item::Array(ws)) = map_get(m, 4) else { return Err(Error("a normal record carries witnesses")) };
    if ws.is_empty() || ws.len() > WITNESSES_PER_RECORD {
        return Err(Error("witnesses out of 1..=16"));
    }
    let mut affirmative = false;
    for w in ws {
        let Item::Map(wm) = w else { return Err(Error("witness not map")) };
        if map_get(wm, 4).is_some() || map_get(wm, 5).is_some() {
            return Err(Error("retired witness key"));
        }
        if keyhash_at(b, wm, 1).is_none() {
            return Err(Error("witness keyhash"));
        }
        // the nominator is one of the two participants (§3.2)
        match keyhash_at(b, wm, 2) {
            Some(n) if keys.contains(&n) => {}
            _ => return Err(Error("a witness's nominator is not a participant")),
        }
        if map_get(wm, 3).and_then(as_uint).is_some_and(|x| x & 3 == 3) {
            affirmative = true;
        }
    }
    if !affirmative {
        return Err(Error("witness floor: no affirmative attestation"));
    }
    if let Some(Item::Array(resp)) = map_get(m, 5) {
        if resp.len() > VERIFIER_RESPONSES_PER_RECORD {
            return Err(Error("responses over 32"));
        }
        if resp.is_empty() {
            return Err(Error("empty response array must be omitted"));
        }
    }
    Ok(())
}
