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
    /// The `[series, counter]` pair, both in the u32 range.
    Seqno,
    /// The two-key array §2.2 fixes exactly.
    KeyMaterial,
    /// A non-empty byte string of at most this many bytes.
    BstrMax(usize),
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
    /// One-time keys as deposited, opaque and bounded (`wire-format.md` §7.10).
    OneTimeKeys,
    Envelopes,
    Keyhashes,
    CurrencyAttestation,
    PrekeyBundle,
    CatalogEntry,
    Scope,
    VerifierResponse,
    /// A transport delegation (`wire-format.md` §8.2), a signed map.
    Delegation,
    /// A frontier: one or more txids (`wire-format.md` §7.9).
    Txids,
    /// Archive entries: an envelope map or a presentation array each
    /// (`wire-format.md` §7.9).
    ArchiveEntries,
    /// One bundle per device, at most eight (`wire-format.md` §7.8).
    PrekeyBundles,
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
    /// What a client hands its serving node (`wire-format.md` §7.10).
    PrekeyPublication,
    OneTimeDeposit,
    RelaySubmission,
    WakeRegistration,
    SubmissionReply,
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
    /// Control frame 7: a delegated peer's first frame on a connection that
    /// opens no session (`wire-format.md` §8.0, §8.2).
    Delegation,
}

use T::*;
const ATTACH: Fields = &[(1, true, Keyhash), (2, false, CurrencyAttestation), (3, true, Capabilities), (4, false, Delegation)];
const ATTACH_ACK: Fields = &[(1, true, Uint), (2, false, SiblingRefs), (3, true, Uint), (4, true, Uint), (5, true, Capabilities), (6, false, Delegation)];
const HEARTBEAT: Fields = &[(1, true, Uint), (2, true, Uint)];
const SIBLING_UPDATE: Fields = &[(1, false, SiblingRefs)];
const TOPOLOGY_PUSH: Fields = &[(1, true, Uint), (2, true, Bstr)];
const TOPOLOGY_MEMO: Fields = &[(1, true, Keyhash), (2, true, Locator), (3, true, Uint), (4, true, Uint), (5, false, Keyhash)];
const RESOLVE_REQUEST: Fields = &[(1, true, Keyhash), (2, true, Keyhash), (3, true, Path), (4, true, Nonce16)];
const ARCHIVE_REQUEST: Fields = &[(1, true, Keyhash), (2, false, Txids), (3, true, Uint), (4, false, Uint), (5, true, Nonce16)];
const PREKEY_REQUEST: Fields = &[(1, true, Keyhash), (2, true, Uint), (3, true, Nonce16), (4, false, Bytes32)];
const PREKEY_BATCH_REQUEST: Fields = &[(1, true, Keyhashes), (2, true, Nonce16)];
const CATALOG_QUERY: Fields = &[(1, false, Tstr(64)), (2, true, Nonce16)];
const RESOURCE_REQUEST: Fields = &[(1, true, Keyhash), (2, true, Bstr)];
const RESOURCE_REGISTRATION: Fields = &[(1, true, CatalogEntry), (2, false, Scope), (3, true, Nonce16)];
// §7.10: what a client hands its serving node.  The bundle and the keys are
// opaque here, as §7.8 makes them everywhere else.
const PREKEY_PUBLICATION: Fields = &[(1, true, PrekeyBundle), (2, true, Nonce16)];
const ONE_TIME_DEPOSIT: Fields = &[(1, true, OneTimeKeys), (2, true, Nonce16)];
const RELAY_SUBMISSION: Fields = &[(1, true, Keyhash), (2, true, Bstr), (3, true, Nonce16), (4, true, Bytes32)];
const WAKE_REGISTRATION: Fields = &[(1, true, Nonce16), (2, false, Tstr(2048)), (3, false, BstrMax(256)), (4, false, Uint)];
const SUBMISSION_REPLY: Fields = &[(1, true, Nonce16), (2, true, Uint)];
const CURRENCY_REQUEST: Fields = &[(1, true, Keyhash), (2, true, Nonce16)];
const RESOLVE_REPLY: Fields = &[(1, true, Nonce16), (2, true, Uint), (3, false, ServingInfra), (4, false, Uint), (5, false, Referral)];
const CATALOG_REPLY: Fields = &[(1, true, Nonce16), (2, true, CatalogEntries), (3, false, Tstr(64))];
const RESOURCE_RESPONSE: Fields = &[(1, true, Uint), (2, false, Bstr)];
const ARCHIVE_REPLY: Fields = &[(1, true, Nonce16), (2, true, ArchiveEntries), (3, true, Bool), (4, false, Txids)];
const PREKEY_REPLY: Fields = &[(1, true, Nonce16), (2, false, PrekeyBundles), (3, false, Bstr), (4, false, Uint)];
// §8.2: the transport key, the delegating keyhash, the window, and a hybrid
// `COSE_Sign` over fields 1 to 4; a signed map, so extensions above 5 are
// admitted by `check_map_signed`
const DELEGATION: Fields = &[(1, true, Bytes32), (2, true, Keyhash), (3, true, Uint), (4, true, Uint), (5, true, Any)];
pub const DELEGATION_WINDOW_SECONDS: u64 = 172_800;
const CURRENCY_REPLY: Fields = &[(1, true, Nonce16), (2, true, Uint), (3, false, CurrencyAttestation)];
const RESOURCE_REGISTRATION_REPLY: Fields = &[(1, true, Nonce16), (2, true, Uint)];
const KEY_GRANT: Fields = &[(1, true, Bytes32), (2, true, Bytes32), (3, true, Bytes32)];
const LATE_RESPONSE: Fields = &[(1, true, Bytes32), (2, true, Keyhash), (3, true, VerifierResponse)];
const LOCATOR: Fields = &[(1, true, Keyhash), (2, true, Path), (3, true, Seqno)];
const PATH: Fields = &[(1, true, Bstr), (2, true, Uint)];
const SIBLING_REF: Fields = &[(1, true, Keyhash), (2, true, NetworkPoints), (3, false, KeyMaterial)];
const NETWORK_POINT: Fields = &[(1, true, Bstr), (2, false, Uint), (3, false, Uint)];
const SERVING_INFRA: Fields = &[(1, true, Keyhash), (2, true, NetworkPoints), (3, true, Path), (4, false, KeyMaterial)];
const REFERRAL: Fields = &[(1, true, Keyhash), (2, true, NetworkPoints), (3, true, Uint), (4, false, KeyMaterial)];

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
        PrekeyPublication => PREKEY_PUBLICATION,
        OneTimeDeposit => ONE_TIME_DEPOSIT,
        RelaySubmission => RELAY_SUBMISSION,
        WakeRegistration => WAKE_REGISTRATION,
        SubmissionReply => SUBMISSION_REPLY,
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
        Delegation => DELEGATION,
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

/// The shape of a `COSE_Sign1` (§1): protected header bytes, an empty
/// unprotected header, a nil payload, and signature bytes.
pub fn sign1_shape(it: &Item) -> Result<(), Error> {
    match it {
        Item::Array(a) if a.len() == 4 && matches!(a[0], Item::Bytes(_)) && matches!(&a[1], Item::Map(u) if u.is_empty()) && matches!(a[2], Item::Null) && matches!(a[3], Item::Bytes(_)) => Ok(()),
        _ => Err(Error("signature slot is not a COSE_Sign1")),
    }
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
    // four-byte ASNs, per RFC 6793 as §4.4 cites it
    if map_get(m, 2).and_then(as_uint).is_some_and(|asn| asn > u32::MAX as u64) {
        return Err(Error("ASN outside the u32 range"));
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
        Seqno => {
            seqno_of(Some(&v))?;
        }
        KeyMaterial => {
            // §2.2 fixes the shape exactly, and a strict check already
            // exists; the schema is where it belongs, so a slot carrying
            // something else fails here rather than at pin time
            crate::cose::check_key_material(slice)?;
        }
        BstrMax(max) => match bs(b, &v).map(|s| s.len()) {
            Some(n) if n >= 1 && n <= max => {}
            _ => return Err(Error("bstr shape")),
        },
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
        OneTimeKeys => {
            let Item::Array(ref a) = v else { return Err(Error("one-time keys not array")) };
            if a.is_empty() || a.len() > 256 {
                return Err(Error("a deposit is 1 to 256 keys"));
            }
            if !a.iter().all(|k| matches!(k, Item::Bytes(_))) {
                return Err(Error("a one-time key is a byte string"));
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
        Delegation => nested("Delegation")?,
        Txids => {
            let Item::Array(ref a) = v else { return Err(Error("frontier not array")) };
            if a.is_empty() || a.len() > ARCHIVE_SUBSET_REFS {
                return Err(Error("frontier is one to 256 txids"));
            }
            for k in a {
                if bs(b, k).map(|s| s.len()) != Some(32) {
                    return Err(Error("txid width"));
                }
            }
        }
        ArchiveEntries => {
            // a map is an envelope and an array a presentation, an
            // envelope beside seven disclosure slots (`wire-format.md` §7.9,
            // §4.5.1); the signatures are the verifier's business
            let entries = array_item_ranges(b, at).ok_or(Error("archive entries not array"))?;
            if entries.len() > ARCHIVE_SUBSET_REFS {
                return Err(Error("archive reply over 256"));
            }
            for r in entries {
                let (it, _) = p.item(r.start)?;
                match it {
                    Item::Map(_) => {
                        crate::envelope::parse(&b[r.clone()])?;
                    }
                    Item::Array(ref parts) => {
                        if parts.len() != 2 || !matches!(parts[0], Item::Map(_)) || !matches!(&parts[1], Item::Array(s) if s.len() == 7) {
                            return Err(Error("presented record shape"));
                        }
                        let inner = array_item_ranges(b, r.start).ok_or(Error("presentation walk"))?;
                        crate::envelope::parse(&b[inner[0].clone()])?;
                    }
                    _ => return Err(Error("archive entry neither envelope nor presentation")),
                }
            }
        }
        PrekeyBundles => {
            let bundles = array_item_ranges(b, at).ok_or(Error("bundles not array"))?;
            if bundles.is_empty() || bundles.len() > PREKEY_BUNDLES_PER_REPLY {
                return Err(Error("one to eight bundles"));
            }
            for r in bundles {
                let s = &b[r.clone()];
                check_kind(s, "PrekeyBundle", &parse_all(s)?)?;
            }
        }
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
        // a control frame carrying a signed object: checked as the object
        Family::Delegation => {
            let (item, end) = p.item(at)?;
            check_kind(&b[at..end], "Delegation", &item)
        }
        Family::PrekeyRequestOrBatch => {
             let (Item::Map(ref m), _) = p.item(at)? else { return Err(Error("not a map")) };
            let schema = if matches!(map_get(m, 1), Some(Item::Array(_))) { PREKEY_BATCH_REQUEST } else { PREKEY_REQUEST };
            check_map(b, at, schema)?;
            if schema.len() == 4 {
                let mode = map_get(m, 2).and_then(as_uint).unwrap_or(0);
                if mode > 1 {
                    return Err(Error("prekey request mode"));
                }
                // a one-time key is consumed from one device's pool, so the
                // device is required when one is asked for (§7.8 field 4)
                if mode == 1 && map_get(m, 4).is_none() {
                    return Err(Error("a one-time key request names its device"));
                }
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
            // the same closed enumeration the verifier echoes into field 10
            // (`wire-format.md` §5.5, §5.6): a selector claiming
            // `3 discretionary fill` had no way to send it
            let (sb, _) = p.item(parts[2].start)?;
            if as_uint(&sb).unwrap_or(9) > 3 {
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
                Family::ArchiveReply => {
                    // the frontier is present iff more remain (§7.9)
                    if matches!(map_get(m, 3), Some(Item::Bool(true))) != map_get(m, 4).is_some() {
                        return Err(Error("frontier present iff more remain"));
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
                Family::WakeRegistration => {
                    // a withdrawal is field 2 absent, and its key and lapse
                    // absent with it: they describe an endpoint, and either
                    // without one is a shape with no meaning
                    // (`wire-format.md` §7.10)
                    if map_get(m, 2).is_none() && (map_get(m, 3).is_some() || map_get(m, 4).is_some()) {
                        return Err(Error("a withdrawal carries no key and no lapse"));
                    }
                    // an endpoint the node cannot encrypt to is one it
                    // cannot post the body of a doorbell to
                    if map_get(m, 2).is_some() && map_get(m, 3).is_none() {
                        return Err(Error("an endpoint without the key its body is encrypted to"));
                    }
                }
                Family::SubmissionReply => {
                    if map_get(m, 2).and_then(as_uint).unwrap_or(9) > 2 {
                        return Err(Error("submission code out of range"));
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
        "PrekeyBundle" => (6, aad::PREKEY, 1),
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
        "CurrencyAttestation" => Some(8),
        "PrekeyBundle" => Some(6),
        "AnchorEntry" | "SubtreeAck" | "AbuseReport" | "Witness" | "Delegation" => Some(5),
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
    // a standalone signed kind carries its signature slot, in COSE_Sign1's
    // shape (§7): a record without one is malformed, not unverified
    if let Some((slot, _, _)) = sign1_profile(kind)
        && let Item::Map(m) = item {
            sign1_shape(map_get(m, slot).ok_or(Error("signature slot required"))?)?;
        }
    match kind {
        "EndpointRecord" | "AnchorEntry" => {
            let r2 = value_slice(b, 2).ok_or(Error("field 2"))?;
            let pts = array_item_ranges(b, r2.start).ok_or(Error("network points not array"))?;
            if pts.is_empty() || pts.len() > NETWORK_POINTS_PER_RECORD {
                return Err(Error("network point count"));
            }
            for r in pts {
                network_point_at(b, r.start, true)?;
            }
            Ok(())
        }
        "VerifierResponse" => {
            let Item::Map(m) = item else { return Err(Error("not map")) };
            // fields 1, 2, 3 and 9 carry no `?` in the CDDL, and 9 is the
            // signature that authenticates the rest: a response reached
            // through a `LateResponse` or a presence body must not arrive
            // here with none of them (`wire-format.md` §4.5)
            for (f, w) in [(1u64, 32usize), (2, 32), (3, 32)] {
                if keyhash_at(b, m, f).map(|k| k.len()) != Some(w) {
                    return Err(Error("verifier response identifier width"));
                }
            }
            map_get(m, 9).ok_or(Error("verifier signature required"))?;
            if map_get(m, 4).and_then(as_uint).ok_or(Error("no result"))? > 3 {
                return Err(Error("result out of range"));
            }
            if map_get(m, 5).and_then(as_uint).is_some_and(|x| x > 2) {
                return Err(Error("basis out of range"));
            }
            // §4.5's conditional-field matrix: a basis where the verifier
            // evaluated and none where it did not, and a template version for
            // a photo basis and none otherwise
            let result = map_get(m, 4).and_then(as_uint).unwrap_or(9);
            let basis = map_get(m, 5).and_then(as_uint);
            if (result <= 2) != basis.is_some() {
                return Err(Error("basis does not follow the result"));
            }
            let version = map_get(m, 6).and_then(as_uint);
            if matches!(basis, Some(0) | Some(2)) != version.is_some() {
                return Err(Error("template version does not follow the basis"));
            }
            if version.is_some_and(|v| v > 65535) {
                return Err(Error("template version out of range"));
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
            // each channel's kind and outcome are closed enumerations, and
            // §1.2 rejects an unknown value in a known enumerated field
            // (`wire-format.md` §4.5)
            for c in ch {
                let Item::Map(cm) = c else { return Err(Error("channel not map")) };
                match map_get(cm, 1).and_then(as_uint) {
                    Some(1..=4) => {}
                    _ => return Err(Error("channel kind out of range")),
                }
                match map_get(cm, 2).and_then(as_uint) {
                    Some(0..=2) => {}
                    _ => return Err(Error("channel outcome out of range")),
                }
                match map_get(cm, 4) {
                    Some(Item::Bytes(r)) if !r.is_empty() && r.len() <= 128 => {}
                    None => {}
                    _ => return Err(Error("channel evidence out of range")),
                }
            }
            match map_get(m, 2).and_then(as_uint) {
                Some(1..=4) => {}
                _ => return Err(Error("strongest channel out of range")),
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
            // an entry is owner-signed and re-served byte for byte, so a
            // field this side admits and a conformant peer refuses would
            // propagate as a disagreement rather than stop here
            // (`wire-format.md` §6.1)
            for (f, w) in [(1u64, 32usize), (2, 32)] {
                if keyhash_at(b, m, f).map(|k| k.len()) != Some(w) {
                    return Err(Error("catalog entry keyhash width"));
                }
            }
            for (f, max) in [(3u64, 64usize), (4, 128)] {
                match map_get(m, f) {
                    Some(Item::Text(r)) if !r.is_empty() && r.len() <= max => {}
                    _ => return Err(Error("catalog entry text out of range")),
                }
            }
            for (f, max, required) in [(5u64, 256usize, true), (7, 1024, false)] {
                match map_get(m, f) {
                    Some(Item::Bytes(r)) if !r.is_empty() && r.len() <= max => {}
                    None if !required => {}
                    _ => return Err(Error("catalog entry byte string out of range")),
                }
            }
            Ok(())
        }
        "CurrencyAttestation" => {
            let Item::Map(m) = item else { return Err(Error("not map")) };
            if map_get(m, 5).and_then(as_uint).ok_or(Error("role"))? > 3 {
                return Err(Error("role out of range"));
            }
            map_get(m, 6).ok_or(Error("issuer required"))?;
            // field 8, the stapled delegation (§7.1): well-formed, and
            // delegated by the issuer field 6 names; that it names the key
            // field 7 was made under is the verifier's check
            if let Some(r8) = value_slice(b, 8) {
                let d = &b[r8.clone()];
                check_kind(d, "Delegation", &parse_all(d)?)?;
                let di = parse_all(d)?;
                let Item::Map(dm) = &di else { unreachable!() };
                let by = map_get(dm, 2).and_then(|it| bs(d, it)).map(|s| s.to_vec());
                let issuer = map_get(m, 6).and_then(|it| bs(b, it)).map(|s| s.to_vec());
                if by != issuer {
                    return Err(Error("a stapled delegation is the issuer's"));
                }
            }
            Ok(())
        }
        "Delegation" => {
            check_map_signed(b, 0, DELEGATION)?;
            let Item::Map(m) = item else { return Err(Error("not map")) };
            if bs(b, map_get(m, 1).ok_or(Error("field 1"))?).map(|s| s.len()) != Some(32) {
                return Err(Error("transport key width"));
            }
            let nb = map_get(m, 3).and_then(as_uint).ok_or(Error("not_before"))?;
            let na = map_get(m, 4).and_then(as_uint).ok_or(Error("not_after"))?;
            // exactly 48 hours: a window an issuer could lengthen would put
            // the seed back on the box under another name (§8.2)
            if na.checked_sub(nb) != Some(DELEGATION_WINDOW_SECONDS) {
                return Err(Error("a delegation's window is exactly 172,800 seconds"));
            }
            // a hybrid COSE_Sign: the detached container and two entries,
            // one per algorithm; a classical-only delegation is malformed
            let Some(Item::Array(cs)) = map_get(m, 5) else { return Err(Error("signature not COSE_Sign")) };
            if cs.len() != 4 || !matches!(cs[0], Item::Bytes(_)) || !matches!(&cs[1], Item::Map(u) if u.is_empty()) || !matches!(cs[2], Item::Null) {
                return Err(Error("COSE_Sign container departs from the profile"));
            }
            let Item::Array(entries) = &cs[3] else { return Err(Error("signature entries not array")) };
            if entries.len() != 2 {
                return Err(Error("a hybrid delegation carries one entry per algorithm"));
            }
            for e in entries {
                if !matches!(e, Item::Array(ea) if ea.len() == 3 && matches!(ea[0], Item::Bytes(_)) && matches!(&ea[1], Item::Map(u) if u.is_empty()) && matches!(ea[2], Item::Bytes(_))) {
                    return Err(Error("signature entry shape"));
                }
            }
            Ok(())
        }
        "ResolveReply" => check_unsigned(Family::ResolveReply, b, 0),
        "ResourceResponse" => check_unsigned(Family::ResourceResponse, b, 0),
        "ArchiveRequest" => check_unsigned(Family::ArchiveRequest, b, 0),
        "ArchiveReply" => check_unsigned(Family::ArchiveReply, b, 0),
        "PrekeyReply" => check_unsigned(Family::PrekeyReply, b, 0),
        "PrekeyBatchRequest" => check_unsigned(Family::PrekeyRequestOrBatch, b, 0),
        "CatalogReply" => check_unsigned(Family::CatalogReply, b, 0),
        "PrekeyBundle" => {
            let Item::Map(m) = item else { return Err(Error("not map")) };
            if let Some(Item::Bytes(r)) = map_get(m, 3)
                && r.len() > PREKEY_BUNDLE_BLOB {
                    return Err(Error("blob over 4KB"));
                }
            // field 5, the device, under the signature (§7.8)
            if bs(b, map_get(m, 5).ok_or(Error("device required"))?).map(|s| s.len()) != Some(32) {
                return Err(Error("device key width"));
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
            // the subject, the querier, the pre-commitment and the profile
            // all carry widths (`wire-format.md` §5.1), and an unbounded
            // profile is an unbounded allocation on a request stream
            for f in [1u64, 2, 3] {
                if keyhash_at(b, m, f).map(|k| k.len()) != Some(32) {
                    return Err(Error("query identifier width"));
                }
            }
            match map_get(m, 4) {
                Some(Item::Bytes(r)) if !r.is_empty() && r.len() <= 4096 => {}
                _ => return Err(Error("fuzzed profile out of range")),
            }
            map_get(m, 5).and_then(as_uint).ok_or(Error("template version required"))?;
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
        // what a node delivers for a relay submission (`wire-format.md`
        // §7.10): the submitter in front of the ciphertext, an array and
        // not a map
        "RelayedPayload" => match item {
            Item::Array(a) if a.len() == 2 => {
                match (&a[0], &a[1]) {
                    (Item::Bytes(f), Item::Bytes(_)) if f.len() == 32 => Ok(()),
                    _ => Err(Error("relayed payload shape")),
                }
            }
            _ => Err(Error("relayed payload is a two-element array")),
        },
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
    let (series, counter) = (as_uint(&a[0]).ok_or(Error("seqno series"))?, as_uint(&a[1]).ok_or(Error("seqno counter"))?);
    // both are U32 range (`wire-format.md` §2.3).  **Refused rather than
    // narrowed**: a value that does not fit is malformed, and truncating it
    // would leave two parties holding different beliefs about which series a
    // node is on with nothing raised on either side
    if series > u32::MAX as u64 || counter > u32::MAX as u64 {
        return Err(Error("seqno outside the u32 range"));
    }
    Ok((series, counter))
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
    if lists.is_empty() {
        return Err(Error("key 0 carries one list per signer, and there is a signer"));
    }
    for l in lists {
        let Item::Array(hs) = l else { return Err(Error("list")) };
        if hs.is_empty() || hs.len() > MERGE_BACK_POINTERS_PER_SIGNER {
            return Err(Error("back-pointer bound"));
        }
        // each entry is a 32-byte txid, and **a merge list is sorted**
        // (§3.1): one logical merge, one encoding, one txid
        let mut prev: Option<&[u8]> = None;
        for h in hs {
            let Item::Bytes(r) = h else { return Err(Error("back-pointer not a byte string")) };
            if r.len() != 32 {
                return Err(Error("back-pointer width"));
            }
            let this = &b[r.clone()];
            if prev.is_some_and(|p| this <= p) {
                return Err(Error("a merge list is sorted ascending and repeats nothing"));
            }
            prev = Some(this);
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
            if let Some(Item::Map(tm)) = map_get(m, 9) {
                check_transfer(b, m, tm)?;
            } else if map_get(m, 9).is_some() {
                return Err(Error("field 9 not a map"));
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

/// The consistency rule of a `Transfer` block (§4.1): the former patron
/// differs from the adopted node and from the new patron.  Identical keys
/// represent no transfer, the same rule and the same reason as a
/// `Recovery`'s prior key differing from field 1, and a node does not
/// vouch for its own move.  Checkable from the adoption alone, so it sits
/// beside that one rather than in a verifier that needs keys.
fn check_transfer(b: &[u8], m: &[(Item, Item)], tm: &[(Item, Item)]) -> Result<(), Error> {
    let node = keyhash_at(b, m, 1).ok_or(Error("node"))?;
    let patron = keyhash_at(b, m, 2).ok_or(Error("patron"))?;
    let former = keyhash_at(b, tm, 1).ok_or(Error("transfer former patron"))?;
    if former == node {
        return Err(Error("transfer former patron equals the adopted node"));
    }
    if former == patron {
        return Err(Error("transfer former patron equals the new patron"));
    }
    Ok(())
}

/// The consistency rules of a `Recovery` block (§4.1), each checkable
/// from the adoption alone: the prior key differs from the new one; every
/// Whether a signature object is a `COSE_Sign1` — one signature, and in
/// this profile therefore classical — rather than a `COSE_Sign` carrying
/// an entries array.
///
/// **Both are four-element arrays and the fourth element tells them
/// apart**: a `bstr` is the one signature of a `Sign1`, an array is a
/// `Sign`'s entries.
fn is_sign1(it: Option<&Item>) -> bool {
    matches!(it, Some(Item::Array(a)) if a.len() == 4 && matches!(a[3], Item::Bytes(_)))
}

/// Whether it is a hybrid `COSE_Sign`: an entries array, two of them, one
/// classical and one post-quantum (§3.5).
fn is_hybrid_sign(it: Option<&Item>) -> bool {
    matches!(it, Some(Item::Array(a)) if a.len() == 4 && matches!(&a[3], Item::Array(e) if e.len() == 2))
}

/// §4.5's rule for one verifier response, which depends on where the
/// response sits.
///
/// **The consent signature is classical everywhere**, recovery included:
/// the subject countersigns a query id, and that reliance expires with the
/// query. **Verifier authentication is the one exception** — a `COSE_Sign1`
/// in a presence record, a hybrid `COSE_Sign` inside a `Recovery`, because
/// a recovery induces a permanent identity change and that signature's
/// reliance never expires. "The field's type is fixed by where the
/// response sits."
fn check_response_signatures(x: &[(Item, Item)], in_recovery: bool) -> Result<(), Error> {
    if !is_sign1(map_get(x, 7)) {
        return Err(Error("a response's consent signature is not a classical COSE_Sign1"));
    }
    match in_recovery {
        false if !is_sign1(map_get(x, 9)) => Err(Error("a response's verifier signature is not a COSE_Sign1 in a presence record")),
        true if !is_hybrid_sign(map_get(x, 9)) => Err(Error("a response's verifier signature is not a hybrid COSE_Sign inside a recovery")),
        _ => Ok(()),
    }
}

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
        check_response_signatures(x, true)?;
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
        // **One set, one encoding** (§4.5): ascending by verifier keyhash,
        // ties by ascending subject, and no verifier twice for one subject
        // (§5.5).  Field 5 is inside the signed body, so an unsorted
        // encoding gives the same logical record a second txid — the same
        // rule `check_recovery` applies to the recovery form, which is why
        // its absence here was an asymmetry rather than a decision.
        let mut prev: Option<(&[u8], &[u8])> = None;
        for r in resp {
            let Item::Map(x) = r else { return Err(Error("response not map")) };
            check_kind(b, "VerifierResponse", r)?;
            let verifier = keyhash_at(b, x, 1).ok_or(Error("response verifier"))?;
            let subject = keyhash_at(b, x, 2).ok_or(Error("response subject"))?;
            if !keys.contains(&subject) {
                return Err(Error("a response names a subject who is not a participant"));
            }
            if verifier == subject {
                return Err(Error("a response's verifier is its subject"));
            }
            if prev.is_some_and(|p| (verifier, subject) <= p) {
                return Err(Error("responses unsorted, or one verifier twice for one subject"));
            }
            check_response_signatures(x, false)?;
            prev = Some((verifier, subject));
        }
    }
    Ok(())
}
