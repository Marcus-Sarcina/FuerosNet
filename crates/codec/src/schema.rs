//! Per-kind structural rules: the unsigned message families (§6, §7, §8,
//! §10, §11), the standalone signed records (§7), transaction bodies (§4),
//! and the extension bounds of §1.3.  Everything here decides from the bytes
//! and the parsed tree; nothing consults state.

use crate::bounds::*;
use crate::cbor::*;

/// Field types the unsigned schemas use.
#[derive(Clone, Copy)]
pub enum T {
    /// A 32-byte keyhash (§2.2).
    Keyhash,
    /// A 16-byte nonce.
    Nonce16,
    /// A 32-byte string with no further meaning fixed here.
    Bytes32,
    /// An unsigned integer.
    Uint,
    /// A boolean.
    Bool,
    /// A byte string of any length the enclosing bound allows.
    Bstr,
    /// The `[series, counter]` pair, both in the u32 range.
    Seqno,
    /// The two-key array §2.2 fixes exactly.
    KeyMaterial,
    /// A non-empty byte string of at most this many bytes.
    BstrMax(usize),
    /// A text string of at most this many bytes.
    Tstr(usize),
    /// Unchecked: any well-formed item. For a field whose shape is the
    /// enclosing decoder's to settle, not the generic walk's.
    Any,
    /// A locator map (§2.3).
    Locator,
    /// A path map, its nibble count bounded by
    /// [`PATH_NIBBLES`](crate::bounds::PATH_NIBBLES).
    Path,
    /// A capabilities map, bounded in entries and in each value's bytes
    /// (§1.3).
    Capabilities,
    /// An array of sibling references, at most
    /// [`SIBLING_REFS`](crate::bounds::SIBLING_REFS) (§1.3).
    SiblingRefs,
    /// An array of network points (§4.4), bounded by
    /// [`NETWORK_POINTS_PER_RECORD`](crate::bounds::NETWORK_POINTS_PER_RECORD)
    /// where the enclosing structure is an anchor entry or an endpoint
    /// record (§7.2, §7.6).
    NetworkPoints,
    /// A serving-infrastructure map.
    ServingInfra,
    /// A referral map, which may carry the referred node's key material.
    Referral,
    /// An array of catalog entries, each held to the `CatalogEntry` kind
    /// and to §1.3's byte bound.
    CatalogEntries,
    /// One-time keys as deposited, opaque and bounded (`wire-format.md` §7.10).
    OneTimeKeys,
    /// An array of transaction envelopes (§3).
    Envelopes,
    /// An array of keyhashes.
    Keyhashes,
    /// A currency attestation (§7.1), checked as its own kind.
    CurrencyAttestation,
    /// A prekey bundle (§7.8), checked as its own kind.
    PrekeyBundle,
    /// One catalog entry (§6.1), checked as its own kind.
    CatalogEntry,
    /// A scope, checked as its own kind: what a catalog entry's access
    /// rule names.
    Scope,
    /// A verifier's signed response (§4.5), checked as its own kind.
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

/// The unsigned message families this profile defines, one per schema in
/// [`fields`].  **A family is how a decoder knows which schema to hold a
/// map to**, and the mapping from a frame's type number to a family is the
/// stream's (§8.0, §9.2) rather than this enum's: the same number names
/// different families on the control and request streams.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Family {
    /// Control frame 1: a client attaching to its serving node.
    Attach,
    /// Control frame 2: the node's answer to an attach.
    AttachAck,
    /// Control frame 3: the session heartbeat.
    Heartbeat,
    /// Control frame 4: a replacement sibling list.
    SiblingUpdate,
    /// Control frame 5: topology pushed to an attached client.
    TopologyPush,
    /// Control frame 6: a topology memo.
    TopologyMemo,
    /// Request frame 1: resolve an identity to where it is served.
    ResolveRequest,
    /// Request frame 2: fetch archive entries (§7.9).
    ArchiveRequest,
    /// Request frame 3: prekeys for one subject, or a batch across a
    /// population (§7.8).
    PrekeyRequestOrBatch,
    /// Request frame 4: a query put to a verifier (§4.5).
    VerifierQuery,
    /// Request frame 5: a catalog query (§6.1).
    CatalogQuery,
    /// Request frame 6: a request to a resource.
    ResourceRequest,
    /// Request frame 7: registering a resource in the catalog.
    ResourceRegistration,
    /// What a client hands its serving node (`wire-format.md` §7.10).
    PrekeyPublication,
    /// Request frame 10: one-time keys deposited with the serving node.
    OneTimeDeposit,
    /// Request frame 11: a message handed to a node to relay (§7.10).
    RelaySubmission,
    /// Request frame 12: where to ring this client (§14.1.5).
    WakeRegistration,
    /// The answer to a submission: accepted, over bound, or refused.
    SubmissionReply,
    /// Request frame 8: a currency attestation requested (§7.1).
    CurrencyRequest,
    /// The answer to a [`ResolveRequest`](Family::ResolveRequest).
    ResolveReply,
    /// The answer to a [`CatalogQuery`](Family::CatalogQuery), bounded by
    /// [`CATALOG_REPLY_ENTRIES`](crate::bounds::CATALOG_REPLY_ENTRIES).
    CatalogReply,
    /// The answer to a [`ResourceRequest`](Family::ResourceRequest).
    ResourceResponse,
    /// The answer to an [`ArchiveRequest`](Family::ArchiveRequest) (§7.9).
    ArchiveReply,
    /// The answer to a [`PrekeyRequestOrBatch`](Family::PrekeyRequestOrBatch),
    /// one bundle per device (§7.8).
    PrekeyReply,
    /// The answer to a [`CurrencyRequest`](Family::CurrencyRequest).
    CurrencyReply,
    /// The answer to a
    /// [`ResourceRegistration`](Family::ResourceRegistration).
    ResourceRegistrationReply,
    /// A subject releasing a capture key to a holder (§7.3), carried on
    /// the end-to-end payload path as kind 1 (§7.10.1).
    KeyGrant,
    /// A verifier's response arriving after finalization (§7.4), kind 2 on
    /// the payload path (§7.10.1).
    LateResponse,
    /// Control frame 7: a delegated peer's first frame on a connection that
    /// opens no session (`wire-format.md` §8.0, §8.2).
    Delegation,
}

use T::*;
const ATTACH: Fields = &[
    (1, true, Keyhash),
    (2, false, CurrencyAttestation),
    (3, true, Capabilities),
    (4, false, Delegation),
];
const ATTACH_ACK: Fields = &[
    (1, true, Uint),
    (2, false, SiblingRefs),
    (3, true, Uint),
    (4, true, Uint),
    (5, true, Capabilities),
    (6, false, Delegation),
];
const HEARTBEAT: Fields = &[(1, true, Uint), (2, true, Uint)];
const SIBLING_UPDATE: Fields = &[(1, false, SiblingRefs)];
const TOPOLOGY_PUSH: Fields = &[(1, true, Uint), (2, true, Bstr)];
const TOPOLOGY_MEMO: Fields = &[
    (1, true, Keyhash),
    (2, true, Locator),
    (3, true, Uint),
    (4, true, Uint),
    (5, false, Keyhash),
];
const RESOLVE_REQUEST: Fields = &[
    (1, true, Keyhash),
    (2, true, Keyhash),
    (3, true, Path),
    (4, true, Nonce16),
];
const ARCHIVE_REQUEST: Fields = &[
    (1, true, Keyhash),
    (2, false, Txids),
    (3, true, Uint),
    (4, false, Uint),
    (5, true, Nonce16),
];
const PREKEY_REQUEST: Fields = &[
    (1, true, Keyhash),
    (2, true, Uint),
    (3, true, Nonce16),
    (4, false, Bytes32),
];
const PREKEY_BATCH_REQUEST: Fields = &[(1, true, Keyhashes), (2, true, Nonce16)];
const CATALOG_QUERY: Fields = &[(1, false, Tstr(64)), (2, true, Nonce16)];
const RESOURCE_REQUEST: Fields = &[(1, true, Keyhash), (2, true, Bstr)];
const RESOURCE_REGISTRATION: Fields = &[
    (1, true, CatalogEntry),
    (2, false, Scope),
    (3, true, Nonce16),
];
// §7.10: what a client hands its serving node.  The bundle and the keys are
// opaque here, as §7.8 makes them everywhere else.
const PREKEY_PUBLICATION: Fields = &[(1, true, PrekeyBundle), (2, true, Nonce16)];
const ONE_TIME_DEPOSIT: Fields = &[(1, true, OneTimeKeys), (2, true, Nonce16)];
const RELAY_SUBMISSION: Fields = &[
    (1, true, Keyhash),
    (2, true, Bstr),
    (3, true, Nonce16),
    (4, true, Bytes32),
];
const WAKE_REGISTRATION: Fields = &[
    (1, true, Nonce16),
    (2, false, Tstr(2048)),
    (3, false, BstrMax(256)),
    (4, false, Uint),
];
const SUBMISSION_REPLY: Fields = &[(1, true, Nonce16), (2, true, Uint)];
const CURRENCY_REQUEST: Fields = &[(1, true, Keyhash), (2, true, Nonce16)];
const RESOLVE_REPLY: Fields = &[
    (1, true, Nonce16),
    (2, true, Uint),
    (3, false, ServingInfra),
    (4, false, Uint),
    (5, false, Referral),
];
const CATALOG_REPLY: Fields = &[
    (1, true, Nonce16),
    (2, true, CatalogEntries),
    (3, false, Tstr(64)),
];
const RESOURCE_RESPONSE: Fields = &[(1, true, Uint), (2, false, Bstr)];
const ARCHIVE_REPLY: Fields = &[
    (1, true, Nonce16),
    (2, true, ArchiveEntries),
    (3, true, Bool),
    (4, false, Txids),
];
const PREKEY_REPLY: Fields = &[
    (1, true, Nonce16),
    (2, false, PrekeyBundles),
    (3, false, Bstr),
    (4, false, Uint),
];
// §8.2: the transport key, the delegating keyhash, the window, and a hybrid
// `COSE_Sign` over fields 1 to 4; a signed map, so extensions above 5 are
// admitted by `check_map_signed`
const DELEGATION: Fields = &[
    (1, true, Bytes32),
    (2, true, Keyhash),
    (3, true, Uint),
    (4, true, Uint),
    (5, true, Any),
];
/// A transport delegation's window, **exactly** 48 hours (§8.2): field 4
/// is this many seconds after field 3 and no other value is well-formed.
///
/// 48 hours is what one credential costs when a device is separated from
/// the key that signs them, and the run an instance is provisioned with is
/// 45 of them, 90 days end to end (design §12.6.5).
pub const DELEGATION_WINDOW_SECONDS: u64 = 172_800;
const CURRENCY_REPLY: Fields = &[
    (1, true, Nonce16),
    (2, true, Uint),
    (3, false, CurrencyAttestation),
];
const RESOURCE_REGISTRATION_REPLY: Fields = &[(1, true, Nonce16), (2, true, Uint)];
const KEY_GRANT: Fields = &[(1, true, Bytes32), (2, true, Bytes32), (3, true, Bytes32)];
const LATE_RESPONSE: Fields = &[
    (1, true, Bytes32),
    (2, true, Keyhash),
    (3, true, VerifierResponse),
];
const LOCATOR: Fields = &[(1, true, Keyhash), (2, true, Path), (3, true, Seqno)];
const PATH: Fields = &[(1, true, Bstr), (2, true, Uint)];
const SIBLING_REF: Fields = &[
    (1, true, Keyhash),
    (2, true, NetworkPoints),
    (3, false, KeyMaterial),
];
const NETWORK_POINT: Fields = &[(1, true, Bstr), (2, false, Uint), (3, false, Uint)];
const SERVING_INFRA: Fields = &[
    (1, true, Keyhash),
    (2, true, NetworkPoints),
    (3, true, Path),
    (4, false, KeyMaterial),
];
const REFERRAL: Fields = &[
    (1, true, Keyhash),
    (2, true, NetworkPoints),
    (3, true, Uint),
    (4, false, KeyMaterial),
];

/// The schema for a family: its keys, which are required, and each one's
/// type.  `None` for a family with no table here, which is a family whose
/// shape is checked by its own decoder rather than by the generic walk.
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
        let Item::Uint(key) = k else {
            return Err(Error("map key not uint"));
        };
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
    key_material_names_its_party(b, at, schema)
}

/// §3.4: **carried `KeyMaterial` hashes to the keyhash of the party it
/// describes.**  Every schema here that carries key material carries it
/// beside that party's keyhash in field 1 — a sibling reference, a
/// referral, a `ServingInfra` — and each exists so a recipient holding no
/// pin can authenticate the party from the material. Unchecked, a sender
/// presents one identity's keyhash beside another's key and the recipient
/// pins the wrong key, which is the one thing the field is for.
///
/// Stated over the schema rather than per message, so a map that gains a
/// `KeyMaterial` field gets the rule with it [2026-09-30].
fn key_material_names_its_party(b: &[u8], at: usize, schema: Fields) -> Result<(), Error> {
    let carries_km = schema.iter().any(|(_, _, t)| matches!(t, KeyMaterial));
    if !carries_km {
        return Ok(());
    }
    let entries = map_entry_ranges(b, at).ok_or(Error("map walk"))?;
    let p = Parser { b };
    let mut named: Option<&[u8]> = None;
    let mut material: Option<&[u8]> = None;
    for (kr, vr) in &entries {
        let (Item::Uint(key), _) = p.item(kr.start)? else {
            continue;
        };
        let Some((_, _, t)) = schema.iter().find(|(sk, _, _)| *sk == key) else {
            continue;
        };
        match t {
            Keyhash if key == 1 => named = bs(b, &p.item(vr.start)?.0),
            KeyMaterial => material = Some(&b[vr.clone()]),
            _ => {}
        }
    }
    match (named, material) {
        (Some(kh), Some(km)) if crate::cose::sha256(km) != kh => Err(Error(
            "carried key material does not hash to the keyhash beside it",
        )),
        // material with no keyhash to check it against is not this rule's
        // to refuse: the schema's own required-field check covers it
        _ => Ok(()),
    }
}

/// The shape of a `COSE_Sign1` (§1): protected header bytes, an empty
/// unprotected header, a nil payload, and signature bytes.
pub fn sign1_shape(it: &Item) -> Result<(), Error> {
    match it {
        Item::Array(a)
            if a.len() == 4
                && matches!(a[0], Item::Bytes(_))
                && matches!(&a[1], Item::Map(u) if u.is_empty())
                && matches!(a[2], Item::Null)
                && matches!(a[3], Item::Bytes(_)) =>
        {
            Ok(())
        }
        _ => Err(Error("signature slot is not a COSE_Sign1")),
    }
}

/// One `NetworkPoint` map (§4.4): its fields, a four-byte address, and a
/// port in range that is not the default written out; in a signed object
/// its unknown keys are extensions.
fn network_point_at(b: &[u8], at: usize, signed: bool) -> Result<(), Error> {
    check_map_with(b, at, NETWORK_POINT, signed)?;
    let (Item::Map(ref m), _) = (Parser { b }).item(at)? else {
        unreachable!()
    };
    if bs(b, map_get(m, 1).unwrap()).map(|s| s.len()) != Some(4) {
        return Err(Error("address width"));
    }
    if let Some(port) = map_get(m, 3).and_then(as_uint)
        && (port == 0 || port > 65535 || port == 7431)
    {
        return Err(Error("port invalid"));
    }
    // four-byte ASNs, per RFC 6793 as §4.4 cites it
    if map_get(m, 2)
        .and_then(as_uint)
        .is_some_and(|asn| asn > u32::MAX as u64)
    {
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
        let v = if i % 2 == 0 {
            packed[i / 2] >> 4
        } else {
            packed[i / 2] & 0x0f
        };
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
            let n = map_get(m, 2)
                .and_then(as_uint)
                .ok_or(Error("path nibble count"))?;
            let Some(packed) = map_get(m, 1).and_then(|it| bs(b, it)) else {
                return Err(Error("path not bstr"));
            };
            packed_path(packed, n)?;
        }
        Capabilities => {
            let Item::Map(ref m) = v else {
                return Err(Error("capabilities not map"));
            };
            if m.len() > CAPABILITIES_ENTRIES {
                return Err(Error("over 64 capability entries"));
            }
            for (k, val) in m {
                as_uint(k).ok_or(Error("capability id not uint"))?;
                let Some(s) = bs(b, val) else {
                    return Err(Error("capability value not bstr"));
                };
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
            let Item::Array(ref a) = v else {
                return Err(Error("one-time keys not array"));
            };
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
            let Item::Array(ref a) = v else {
                return Err(Error("envelopes not array"));
            };
            if a.len() > ARCHIVE_SUBSET_REFS {
                return Err(Error("archive reply over 256"));
            }
        }
        Keyhashes => {
            let Item::Array(ref a) = v else {
                return Err(Error("population not array"));
            };
            if a.len() < 2 || a.len() > 256 {
                return Err(Error("population out of range"));
            }
            let mut prev: Option<&[u8]> = None;
            for k in a {
                let Some(s) = bs(b, k) else {
                    return Err(Error("population entry"));
                };
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
            let Item::Array(ref a) = v else {
                return Err(Error("frontier not array"));
            };
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
                        if parts.len() != 2
                            || !matches!(parts[0], Item::Map(_))
                            || !matches!(&parts[1], Item::Array(s) if s.len() == 7)
                        {
                            return Err(Error("presented record shape"));
                        }
                        let inner =
                            array_item_ranges(b, r.start).ok_or(Error("presentation walk"))?;
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
            let (Item::Map(ref m), _) = p.item(at)? else {
                return Err(Error("not a map"));
            };
            let schema = if matches!(map_get(m, 1), Some(Item::Array(_))) {
                PREKEY_BATCH_REQUEST
            } else {
                PREKEY_REQUEST
            };
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
            let (Item::Map(ref m), _) = p.item(at)? else {
                unreachable!()
            };
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
                    if map_get(m, 2).is_none()
                        && (map_get(m, 3).is_some() || map_get(m, 4).is_some())
                    {
                        return Err(Error("a withdrawal carries no key and no lapse"));
                    }
                    // an endpoint the node cannot encrypt to is one it
                    // cannot post the body of a doorbell to
                    if map_get(m, 2).is_some() && map_get(m, 3).is_none() {
                        return Err(Error(
                            "an endpoint without the key its body is encrypted to",
                        ));
                    }
                }
                Family::SubmissionReply => {
                    if map_get(m, 2).and_then(as_uint).unwrap_or(9) > 2 {
                        return Err(Error("submission code out of range"));
                    }
                }
                Family::TopologyPush => {
                    // 0 a signed transaction, 1 an EndpointRecord, 2 a
                    // Delegation, 3 a SubtreeAck (§10.1)
                    if map_get(m, 1).and_then(as_uint).unwrap_or(9) > 3 {
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
fn nested_extension_bounds(
    b: &[u8],
    map_at: usize,
    key: u64,
    known: &dyn Fn(u64) -> bool,
) -> Result<(), Error> {
    let Some(r) = value_slice_at(b, map_at, key) else {
        return Ok(());
    };
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
        "AnchorEntry" | "EndpointRecord" | "SignedLocator" => {
            nested_extension_bounds(b, 0, 2, &|k| (1..=3).contains(&k))
        }
        _ => Ok(()),
    }
}

/// The §4.5 `Channel` maps, exactly as a record's proximity field and a
/// `ProximityOutcomes` exchange (`wire-format.md` §14.3.2) both carry
/// them.  Each channel's kind and outcome are closed enumerations, and
/// §1.2 rejects an unknown value in a known enumerated field.
///
/// `signed` picks which of §1's two rules an unknown key falls under:
/// inside a signed record it is an extension the holder preserves, and
/// the caller bounds it; on an unsigned message it is refused.
fn channels_ok(ch: &[Item], signed: bool) -> Result<(), Error> {
    for c in ch {
        let Item::Map(cm) = c else {
            return Err(Error("channel not map"));
        };
        if !signed {
            let known = cm
                .iter()
                .filter(|(k, _)| matches!(k, Item::Uint(1..=4)))
                .count();
            if known != cm.len() {
                return Err(Error("unknown key on an unsigned message"));
            }
        }
        match map_get(cm, 1).and_then(as_uint) {
            Some(1..=4) => {}
            _ => return Err(Error("channel kind out of range")),
        }
        match map_get(cm, 2).and_then(as_uint) {
            Some(0..=2) => {}
            _ => return Err(Error("channel outcome out of range")),
        }
        // field 3 is `? uint` with no presence condition (§4.5): whether a
        // kind carries a claimed resolution is the client's claim and
        // policy weighs it, but what is carried is a uint or the field is
        // malformed -- it was the one `Channel` field nothing checked
        // [2026-09-30]
        match map_get(cm, 3) {
            Some(Item::Uint(_)) | None => {}
            _ => return Err(Error("channel resolution is a uint")),
        }
        match map_get(cm, 4) {
            Some(Item::Bytes(r)) if !r.is_empty() && r.len() <= 128 => {}
            None => {}
            _ => return Err(Error("channel evidence out of range")),
        }
    }
    Ok(())
}

/// A `Candidate` array (`wire-format.md` §14.3.2): one to eight, each
/// `[kind, address, port]` — kind a closed enumeration, the address 4
/// bytes or 16 and nothing between, and port zero never a destination.
fn candidates_ok(item: &Item) -> Result<(), Error> {
    let Item::Array(cs) = item else {
        return Err(Error("candidates not array"));
    };
    if cs.is_empty() || cs.len() > CANDIDATES_PER_EXCHANGE {
        return Err(Error("one to eight candidates"));
    }
    for c in cs {
        let Item::Array(f) = c else {
            return Err(Error("candidate not array"));
        };
        if f.len() != 3 {
            return Err(Error("a candidate has three fields"));
        }
        match as_uint(&f[0]) {
            Some(0 | 1) => {}
            _ => return Err(Error("candidate kind out of range")),
        }
        match &f[1] {
            Item::Bytes(r) if r.len() == 4 || r.len() == 16 => {}
            _ => return Err(Error("an address is 4 or 16 bytes")),
        }
        match as_uint(&f[2]) {
            Some(1..=65535) => {}
            _ => return Err(Error("candidate port")),
        }
    }
    Ok(())
}

/// One `COSE_Signature` entry as an envelope carries it (§3.5):
/// `[protected, unprotected, signature]`, the headers a byte string and an
/// empty map, the signature a byte string.
fn cose_signature_shape(it: &Item) -> Result<(), Error> {
    match it {
        Item::Array(a)
            if a.len() == 3
                && matches!(a[0], Item::Bytes(_))
                && matches!(&a[1], Item::Map(u) if u.is_empty())
                && matches!(a[2], Item::Bytes(_)) =>
        {
            Ok(())
        }
        _ => Err(Error("an entry is not a COSE_Signature")),
    }
}

/// A `PrekeyBundle`'s payload, fields 1 to 5 (`wire-format.md` §7.8): the
/// subject and the device each 32 bytes, the construction and the
/// publication time uints, the reusable material a byte string within the
/// blob bound.  Its signature slot is the business of whoever knows
/// whether the bundle in hand is signed yet.
fn prekey_bundle_payload_ok(b: &[u8], m: &[(Item, Item)]) -> Result<(), Error> {
    if keyhash_at(b, m, 1).is_none() {
        return Err(Error("subject keyhash width"));
    }
    map_get(m, 2)
        .and_then(as_uint)
        .ok_or(Error("construction required"))?;
    match map_get(m, 3) {
        Some(Item::Bytes(r)) if r.len() <= PREKEY_BUNDLE_BLOB => {}
        Some(Item::Bytes(_)) => return Err(Error("blob over 4KB")),
        _ => return Err(Error("material required")),
    }
    map_get(m, 4)
        .and_then(as_uint)
        .ok_or(Error("published_at required"))?;
    // field 5, the device, under the signature (§7.8)
    if keyhash_at(b, m, 5).is_none() {
        return Err(Error("device key width"));
    }
    Ok(())
}

/// A carried `PrekeyBundle` blob (`wire-format.md` §14.3.3): parses as a
/// map, within the ceiling, its signature slot present or absent as the
/// carriage requires, and its payload a `PrekeyBundle`'s on either side
/// of the signature: a credential's is the whole signed record, an
/// introduction's the same map before the identity signs it.
fn device_bundle_ok(b: &[u8], f: &Item, signed: bool) -> Result<(), Error> {
    let Item::Bytes(r) = f else {
        return Err(Error("bundle not bstr"));
    };
    if r.is_empty() || r.len() > DEVICE_BUNDLE_BYTES {
        return Err(Error("bundle over the ceiling"));
    }
    let bundle = &b[r.clone()];
    let inner = parse_all(bundle).map_err(|_| Error("bundle does not parse"))?;
    let Item::Map(m) = &inner else {
        return Err(Error("bundle not a map"));
    };
    match (signed, map_get(m, 6).is_some()) {
        (true, true) | (false, false) => {}
        (true, false) => return Err(Error("a credential's bundle carries field 6")),
        (false, true) => return Err(Error("an introduction's bundle omits field 6")),
    }
    if signed {
        check_kind(bundle, "PrekeyBundle", &inner)
    } else {
        record_extension_bounds(bundle, "PrekeyBundle", &inner)?;
        prekey_bundle_payload_ok(bundle, m)
    }
}

/// Per-kind validation for the signed records and transaction bodies the
/// corpus names.  `b` holds the bytes the item's ranges index.
pub fn check_kind(b: &[u8], kind: &str, item: &Item) -> Result<(), Error> {
    record_extension_bounds(b, kind, item)?;
    // a standalone signed kind carries its signature slot, in COSE_Sign1's
    // shape (§7): a record without one is malformed, not unverified
    if let Some((slot, _, _)) = sign1_profile(kind)
        && let Item::Map(m) = item
    {
        sign1_shape(map_get(m, slot).ok_or(Error("signature slot required"))?)?;
    }
    match kind {
        "EndpointRecord" | "AnchorEntry" => {
            let r2 = value_slice(b, 2).ok_or(Error("field 2"))?;
            let pts = array_item_ranges(b, r2.start).ok_or(Error("network points not array"))?;
            if pts.is_empty() || pts.len() > NETWORK_POINTS_PER_RECORD {
                return Err(Error("network point count"));
            }
            let mut seen: std::collections::BTreeSet<&[u8]> = std::collections::BTreeSet::new();
            for r in pts {
                network_point_at(b, r.start, true)?;
                // §7.6's distinct-entries rule: one destination listed twice
                // is malformed, not merely redundant
                if !seen.insert(&b[r.clone()]) {
                    return Err(Error("network point listed twice"));
                }
            }
            // the signer's keyhash is 32 bytes and the counters are §2.3's
            // seqno pair: the fields beside the network points, which were
            // the only ones read [2026-10-01]
            let Item::Map(m) = item else {
                return Err(Error("not map"));
            };
            if keyhash_at(b, m, 1).map(|k| k.len()) != Some(32) {
                return Err(Error("signer keyhash width"));
            }
            if kind == "AnchorEntry" {
                map_get(m, 3)
                    .and_then(as_uint)
                    .ok_or(Error("subtree size is a uint"))?;
                seqno_of(map_get(m, 4))?;
            } else {
                seqno_of(map_get(m, 3))?;
            }
            Ok(())
        }
        "VerifierResponse" => {
            let Item::Map(m) = item else {
                return Err(Error("not map"));
            };
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
            // §5.5 narrows the matrix for one result [2026-09-02]:
            // `inconclusive` is a capture in hand that could not be read,
            // and it carries basis 0 and the query's template version --
            // the attempted mechanism, not a claim that comparison ran.
            // Personal knowledge cannot be inconclusive: nothing was
            // attempted that could fail to be read.
            if result == 2 && basis != Some(0) {
                return Err(Error("an inconclusive carries the photo basis"));
            }
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
            let Item::Map(m) = item else {
                return Err(Error("not map"));
            };
            if bs(b, map_get(m, 1).ok_or(Error("subject"))?).map(|s| s.len()) != Some(32) {
                return Err(Error("keyhash width"));
            }
            Ok(())
        }
        "NetworkPoint" => {
            let Item::Map(m) = item else {
                return Err(Error("not map"));
            };
            if let Some(port) = map_get(m, 3).and_then(as_uint)
                && (port == 0 || port > 65535 || port == 7431)
            {
                return Err(Error("port invalid"));
            }
            Ok(())
        }
        "LocationEvidence" => {
            let Item::Map(m) = item else {
                return Err(Error("not map"));
            };
            let Item::Array(asserted) = map_get(m, 1).ok_or(Error("asserted"))? else {
                return Err(Error("asserted not array"));
            };
            if asserted.len() > ASSERTED_LOCATIONS_PER_RECORD {
                return Err(Error("asserted over 4"));
            }
            for a in asserted {
                let Item::Map(am) = a else {
                    return Err(Error("assert map"));
                };
                let Item::Text(g) = map_get(am, 2).ok_or(Error("geohash"))? else {
                    return Err(Error("geohash not tstr"));
                };
                let gh = &b[g.clone()];
                if !(gh.len() == 3 || gh.len() == 4)
                    || !gh
                        .iter()
                        .all(|c| b"0123456789bcdefghjkmnpqrstuvwxyz".contains(c))
                {
                    return Err(Error("geohash malformed"));
                }
            }
            if let Some(Item::Array(cor)) = map_get(m, 2)
                && cor.len() > CORROBORATIONS_PER_RECORD
            {
                return Err(Error("corroborations over 16"));
            }
            Ok(())
        }
        "Proximity" => {
            let Item::Map(m) = item else {
                return Err(Error("not map"));
            };
            let Item::Array(ch) = map_get(m, 1).ok_or(Error("channels"))? else {
                return Err(Error("channels not array"));
            };
            if ch.is_empty() || ch.len() > PROXIMITY_CHANNELS_PER_RECORD {
                return Err(Error("channel count"));
            }
            // a record's channels are under its signature, so their unknown
            // keys are kept and bounded like any signed map's (§1.3)
            nested_extension_bounds(b, 0, 1, &|k| (1..=4).contains(&k))?;
            channels_ok(ch, true)?;
            match map_get(m, 2).and_then(as_uint) {
                Some(1..=4) => {}
                _ => return Err(Error("strongest channel out of range")),
            }
            Ok(())
        }
        // ---- the local device-to-device interfaces (`wire-format.md` §14.3)
        "OpticalContribution" => {
            let Item::Array(a) = item else {
                return Err(Error("not array"));
            };
            if a.len() != 3 {
                return Err(Error("three fields"));
            }
            if as_uint(&a[0]) != Some(1) {
                return Err(Error("version 1"));
            }
            // the contribution is the last field, so the key material is
            // everything between the version and it
            let end = match &a[2] {
                Item::Bytes(r) if r.len() == 16 && r.end == b.len() => r.start - 1,
                _ => return Err(Error("a contribution is 16 bytes")),
            };
            if !matches!(a[1], Item::Array(_)) || end <= 2 {
                return Err(Error("the key material is a KeyMaterial array"));
            }
            // §2.2 fixes the shape exactly: the first contact pins from it
            crate::cose::check_key_material(&b[2..end])?;
            Ok(())
        }
        "TranscriptConfirm" => {
            let Item::Array(a) = item else {
                return Err(Error("not array"));
            };
            if a.len() != 2 {
                return Err(Error("two fields"));
            }
            if as_uint(&a[0]) != Some(1) {
                return Err(Error("version 1"));
            }
            match &a[1] {
                Item::Bytes(r) if r.len() == 32 => {}
                _ => return Err(Error("a ceremony-id is 32 bytes")),
            }
            Ok(())
        }
        "IntentExchange" => {
            let Item::Array(a) = item else {
                return Err(Error("not array"));
            };
            if a.len() != 8 {
                return Err(Error("eight fields"));
            }
            if as_uint(&a[0]) != Some(1) {
                return Err(Error("version 1"));
            }
            match &a[1] {
                Item::Bytes(r) if r.len() == 16 => {}
                _ => return Err(Error("an echoed contribution is 16 bytes")),
            }
            let Item::Array(noms) = &a[2] else {
                return Err(Error("nominees not array"));
            };
            if noms.len() > INTENT_NOMINEES {
                return Err(Error("nominees over 64"));
            }
            for n in noms {
                match n {
                    Item::Bytes(r) if r.len() == 32 => {}
                    _ => return Err(Error("a nominee is a keyhash")),
                }
            }
            let Item::Array(bundle) = &a[3] else {
                return Err(Error("bundle not array"));
            };
            if bundle.len() > INTENT_BUNDLE_ENTRIES {
                return Err(Error("bundle carriage over 256"));
            }
            as_uint(&a[4]).ok_or(Error("started_at"))?;
            as_uint(&a[5]).ok_or(Error("retention"))?;
            match &a[6] {
                Item::Bool(_) => {}
                _ => return Err(Error("initiator is a bool")),
            }
            // the bundle itself has no ceiling (§5.4); its carriage does,
            // and the rest follows in numbered continuations
            as_uint(&a[7]).ok_or(Error("continuations is a count"))?;
            Ok(())
        }
        "BundleContinuation" => {
            let Item::Array(a) = item else {
                return Err(Error("not array"));
            };
            if a.len() != 4 {
                return Err(Error("four fields"));
            }
            if as_uint(&a[0]) != Some(1) {
                return Err(Error("version 1"));
            }
            match &a[1] {
                Item::Bytes(r) if r.len() == 32 => {}
                _ => return Err(Error("a ceremony-id is 32 bytes")),
            }
            if as_uint(&a[2]).is_none_or(|i| i == 0) {
                return Err(Error("a continuation's index starts at one"));
            }
            let Item::Array(bundle) = &a[3] else {
                return Err(Error("bundle not array"));
            };
            if bundle.is_empty() || bundle.len() > INTENT_BUNDLE_ENTRIES {
                return Err(Error("a continuation carries one to 256 entries"));
            }
            Ok(())
        }
        "ProximityOutcomes" => {
            let Item::Array(a) = item else {
                return Err(Error("not array"));
            };
            if a.len() != 3 {
                return Err(Error("three fields"));
            }
            if as_uint(&a[0]) != Some(1) {
                return Err(Error("version 1"));
            }
            match &a[1] {
                Item::Bytes(r) if r.len() == 32 => {}
                _ => return Err(Error("a ceremony-id is 32 bytes")),
            }
            let Item::Array(ch) = &a[2] else {
                return Err(Error("channels not array"));
            };
            if ch.is_empty() || ch.len() > PROXIMITY_CHANNELS_PER_RECORD {
                return Err(Error("one to eight channels"));
            }
            channels_ok(ch, false)
        }
        "Candidates" => candidates_ok(item),
        "CandidateHandover" => {
            let Item::Array(a) = item else {
                return Err(Error("not array"));
            };
            if a.len() != 3 {
                return Err(Error("three fields"));
            }
            if as_uint(&a[0]) != Some(1) {
                return Err(Error("version 1"));
            }
            match &a[1] {
                Item::Bytes(r) if r.len() == 32 => {}
                _ => return Err(Error("a ceremony-id is 32 bytes")),
            }
            candidates_ok(&a[2])
        }
        "CaptureKeyHandover" => {
            let Item::Array(a) = item else {
                return Err(Error("not array"));
            };
            if a.len() != 3 {
                return Err(Error("three fields"));
            }
            if as_uint(&a[0]) != Some(1) {
                return Err(Error("version 1"));
            }
            match &a[1] {
                Item::Bytes(r) if r.len() == 32 => {}
                _ => return Err(Error("a ceremony-id is 32 bytes")),
            }
            match &a[2] {
                Item::Bytes(r) if r.len() == 32 => {}
                _ => return Err(Error("a capture key is 32 bytes")),
            }
            Ok(())
        }
        // `wire-format.md` §14.3.4: the sender's prekey material, carried
        // across the local session so a co-present pair can open an
        // end-to-end channel afterwards with no node's help.  The bundle
        // and the one-time key are opaque here: each is the bytes §7.8
        // publishes and verifies under its own signature, which is where
        // their shape is checked.  The one-time key is optional because
        // §7.8 lets a pool run dry.
        "PrekeyHandover" => {
            let Item::Array(a) = item else {
                return Err(Error("not array"));
            };
            if a.len() != 4 && a.len() != 5 {
                return Err(Error("four fields, or five with a one-time key"));
            }
            if as_uint(&a[0]) != Some(1) {
                return Err(Error("version 1"));
            }
            match &a[1] {
                Item::Bytes(r) if r.len() == 32 => {}
                _ => return Err(Error("a ceremony-id is 32 bytes")),
            }
            match &a[2] {
                Item::Bytes(r) if r.len() == 32 => {}
                _ => return Err(Error("a device is 32 bytes")),
            }
            match &a[3] {
                Item::Bytes(r) if !r.is_empty() => {}
                _ => return Err(Error("a bundle is a non-empty byte string")),
            }
            if let Some(k) = a.get(4) {
                match k {
                    Item::Bytes(r) if !r.is_empty() => {}
                    _ => return Err(Error("a one-time key is a non-empty byte string")),
                }
            }
            Ok(())
        }
        "DeviceIntroduction" => {
            let Item::Array(a) = item else {
                return Err(Error("not array"));
            };
            if a.len() != 3 {
                return Err(Error("three fields"));
            }
            if as_uint(&a[0]) != Some(1) {
                return Err(Error("version 1"));
            }
            match &a[1] {
                Item::Bytes(r) if r.len() == 32 => {}
                _ => return Err(Error("a device key is 32 bytes")),
            }
            device_bundle_ok(b, &a[2], false)
        }
        "DeviceCredential" => {
            let Item::Array(a) = item else {
                return Err(Error("not array"));
            };
            if a.len() != 3 {
                return Err(Error("three fields"));
            }
            if as_uint(&a[0]) != Some(1) {
                return Err(Error("version 1"));
            }
            let Item::Array(dels) = &a[1] else {
                return Err(Error("delegations not array"));
            };
            if dels.is_empty() || dels.len() > DELEGATIONS_PER_CREDENTIAL {
                return Err(Error("one to forty-five delegations"));
            }
            for d in dels {
                match d {
                    Item::Bytes(r) if !r.is_empty() && r.len() <= DELEGATION_ENTRY_BYTES => {}
                    _ => return Err(Error("a delegation entry over the ceiling")),
                }
            }
            device_bundle_ok(b, &a[2], true)
        }
        // ---- the ceremony's conversation (`wire-format.md` §7.10.2)
        "ConsentReply" => {
            let Item::Array(a) = item else {
                return Err(Error("not array"));
            };
            if a.len() != 2 {
                return Err(Error("two fields"));
            }
            match &a[0] {
                Item::Bytes(r) if r.len() == 32 => {}
                _ => return Err(Error("a query_id is 32 bytes")),
            }
            sign1_shape(&a[1])
        }
        "FishingProposal" => {
            let Item::Array(entries) = item else {
                return Err(Error("not array"));
            };
            if entries.is_empty() || entries.len() > FISHING_PROPOSAL_ENTRIES {
                return Err(Error("a fishing proposal carries one to 256 entries"));
            }
            // an entry is an envelope map or a presentation array (§7.9)
            for e in entries {
                if !matches!(e, Item::Map(_) | Item::Array(_)) {
                    return Err(Error("an archive entry is a map or an array"));
                }
            }
            Ok(())
        }
        "WitnessRequest" => {
            let Item::Map(m) = item else {
                return Err(Error("not map"));
            };
            if m.len() != 4 || !(1..=4).all(|k| map_get(m, k).is_some()) {
                return Err(Error("keys 1 to 4"));
            }
            if keyhash_at(b, m, 1).map(|k| k.len()) != Some(32) {
                return Err(Error("a ceremony-id is 32 bytes"));
            }
            let Some(Item::Array(parts)) = map_get(m, 2) else {
                return Err(Error("participants not array"));
            };
            if parts.len() != 2 {
                return Err(Error("two participants"));
            }
            for p in parts {
                match p {
                    Item::Bytes(r) if r.len() == 32 => {}
                    _ => return Err(Error("a participant is a keyhash")),
                }
            }
            map_get(m, 3).and_then(as_uint).ok_or(Error("started_at"))?;
            let Some(Item::Array(ch)) = map_get(m, 4) else {
                return Err(Error("channels not array"));
            };
            if ch.len() > WITNESS_REQUEST_CHANNELS {
                return Err(Error("channels over eight"));
            }
            channels_ok(ch, false)
        }
        "WitnessAnswer" => {
            let Item::Map(m) = item else {
                return Err(Error("not map"));
            };
            let witnessing = match map_get(m, 1) {
                Some(Item::Bool(w)) => *w,
                _ => return Err(Error("witnessing is a bool")),
            };
            // the bits travel only with an acceptance: a declining answer
            // that carries them is malformed, not merely odd
            match (witnessing, map_get(m, 2)) {
                (true, Some(Item::Uint(_))) | (false, None) => {}
                (true, _) => return Err(Error("a witnessing answer carries its bits")),
                (false, _) => return Err(Error("a declining answer carries no bits")),
            }
            if m.len() != 1 + usize::from(witnessing) {
                return Err(Error("keys 1 and 2 only"));
            }
            Ok(())
        }
        "GatheredResponses" => {
            let Item::Array(rs) = item else {
                return Err(Error("not array"));
            };
            if rs.len() > GATHERED_RESPONSES {
                return Err(Error("responses over 32"));
            }
            for r in rs {
                if !matches!(r, Item::Map(_)) {
                    return Err(Error("a response is a map"));
                }
            }
            Ok(())
        }
        "BackPointers" => {
            let Item::Array(hs) = item else {
                return Err(Error("not array"));
            };
            if hs.is_empty() || hs.len() > MERGE_BACK_POINTERS_PER_SIGNER {
                return Err(Error("a signer sends one to eight back-pointers"));
            }
            for h in hs {
                match h {
                    Item::Bytes(r) if r.len() == 32 => {}
                    _ => return Err(Error("a back-pointer is 32 bytes")),
                }
            }
            Ok(())
        }
        "ProposedBody" => {
            let Item::Map(m) = item else {
                return Err(Error("not map"));
            };
            if m.len() != 2 {
                return Err(Error("keys 1 and 2"));
            }
            let Some(Item::Bytes(r)) = map_get(m, 1) else {
                return Err(Error("the body is a byte string"));
            };
            // the body is §4.5's map as every signer will sign it, so it is
            // checked here as every archive will check it: a signer shown
            // a body the archives refuse has nothing to sign
            let body = &b[r.clone()];
            let inner = parse_all(body).map_err(|_| Error("the body does not parse"))?;
            check_body_of_type(body, &inner, 5)?;
            let Some(Item::Array(set)) = map_get(m, 2) else {
                return Err(Error("disclosures not array"));
            };
            // every slot revealed, so a signer recomputes the root (§4.5.1)
            if set.len() != 7 {
                return Err(Error("exactly seven disclosures"));
            }
            for d in set {
                let Item::Array(f) = d else {
                    return Err(Error("a disclosure is an array"));
                };
                if f.len() != 3 {
                    return Err(Error("a disclosure has three fields"));
                }
                match &f[0] {
                    Item::Bytes(r) if r.len() == 16 => {}
                    _ => return Err(Error("a salt is 16 bytes")),
                }
                if !matches!(f[1], Item::Text(_)) {
                    return Err(Error("a label is a text string"));
                }
            }
            Ok(())
        }
        "SigningReply" => {
            let Item::Map(m) = item else {
                return Err(Error("not map"));
            };
            let entries = map_get(m, 1);
            let refusal = map_get(m, 2);
            let particular = map_get(m, 3);
            // exactly one of the entries and the refusal, and the
            // particular only beside the refusal
            match (entries, refusal) {
                (Some(_), Some(_)) => return Err(Error("entries or a refusal, never both")),
                (None, None) => return Err(Error("entries or a refusal")),
                _ => {}
            }
            if let Some(e) = entries {
                let Item::Array(es) = e else {
                    return Err(Error("entries not array"));
                };
                if es.len() != SIGNING_REPLY_ENTRIES {
                    return Err(Error("a signer's two entries"));
                }
                for x in es {
                    cose_signature_shape(x)?;
                }
            }
            let code = match refusal.map(as_uint) {
                None => None,
                Some(Some(c @ 1..=4)) => Some(c),
                Some(_) => return Err(Error("refusal out of range")),
            };
            // the particular names refusal 1's witness or refusal 2's
            // query, and nothing else carries one
            match (code, particular) {
                (Some(1 | 2), Some(Item::Bytes(r))) if r.len() == 32 => {}
                (Some(1 | 2), Some(_)) => return Err(Error("a particular is 32 bytes")),
                (Some(1 | 2), None) => {
                    return Err(Error("refusals 1 and 2 name their particular"));
                }
                (Some(_), Some(_)) => return Err(Error("refusals 3 and 4 carry no particular")),
                (None, Some(_)) => {
                    return Err(Error("a particular accompanies a refusal and nothing else"));
                }
                (_, None) => {}
            }
            let expected = 1 + usize::from(particular.is_some());
            if m.len() != expected {
                return Err(Error("keys 1 to 3 only"));
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
            let Item::Map(m) = item else {
                return Err(Error("not map"));
            };
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
            // field 6, where present, is a `Scope` (§6.1) and is checked as
            // one -- the one field of the entry nothing read [2026-10-01];
            // field 9 is `? uint` whose unknown values are retained (§6.1),
            // so its type alone is checked
            if let Some(r6) = value_slice(b, 6) {
                let sc = &b[r6.clone()];
                check_kind(sc, "Scope", &parse_all(sc)?)?;
            }
            match map_get(m, 9) {
                Some(Item::Uint(_)) | None => {}
                _ => return Err(Error("data_practice is a uint")),
            }
            Ok(())
        }
        "CurrencyAttestation" => {
            let Item::Map(m) = item else {
                return Err(Error("not map"));
            };
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
            let Item::Map(m) = item else {
                return Err(Error("not map"));
            };
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
            let Some(Item::Array(cs)) = map_get(m, 5) else {
                return Err(Error("signature not COSE_Sign"));
            };
            if cs.len() != 4
                || !matches!(cs[0], Item::Bytes(_))
                || !matches!(&cs[1], Item::Map(u) if u.is_empty())
                || !matches!(cs[2], Item::Null)
            {
                return Err(Error("COSE_Sign container departs from the profile"));
            }
            let Item::Array(entries) = &cs[3] else {
                return Err(Error("signature entries not array"));
            };
            if entries.len() != 2 {
                return Err(Error("a hybrid delegation carries one entry per algorithm"));
            }
            for e in entries {
                if !matches!(e, Item::Array(ea) if ea.len() == 3 && matches!(ea[0], Item::Bytes(_)) && matches!(&ea[1], Item::Map(u) if u.is_empty()) && matches!(ea[2], Item::Bytes(_)))
                {
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
        "SubtreeAck" => {
            let Item::Map(m) = item else {
                return Err(Error("not map"));
            };
            // fields 1 to 4 (`wire-format.md` §7.5): the adoption's txid,
            // the acknowledging grandpatron and the node admitted, each 32
            // bytes, then a timestamp.  Field 5's shape is the profile's
            // business above
            for (f, what) in [
                (1u64, "an adoption txid is 32 bytes"),
                (2, "a grandpatron keyhash is 32 bytes"),
                (3, "an admitted node's keyhash is 32 bytes"),
            ] {
                if keyhash_at(b, m, f).is_none() {
                    return Err(Error(what));
                }
            }
            map_get(m, 4)
                .and_then(as_uint)
                .ok_or(Error("a subtree ack carries its timestamp"))?;
            Ok(())
        }
        "PrekeyBundle" => {
            let Item::Map(m) = item else {
                return Err(Error("not map"));
            };
            prekey_bundle_payload_ok(b, m)
        }
        "VerificationQuery" => {
            let Item::Map(m) = item else {
                return Err(Error("not map"));
            };
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
            map_get(m, 5)
                .and_then(as_uint)
                .ok_or(Error("template version required"))?;
            Ok(())
        }
        "Witness" => {
            let Item::Map(m) = item else {
                return Err(Error("not map"));
            };
            if map_get(m, 4).is_some() || map_get(m, 5).is_some() {
                return Err(Error("retired witness key"));
            }
            Ok(())
        }
        "body" => check_body(b, item),
        // what a node delivers for a relay submission (`wire-format.md`
        // §7.10): the submitter in front of the ciphertext, an array and
        // not a map.  The submitter's binding is a third element where the
        // node holds one, and optional because it holds none for a device
        // that published none
        "RelayedPayload" => match item {
            Item::Array(a) if a.len() == 2 || a.len() == 3 => match (&a[0], &a[1], a.get(2)) {
                (Item::Bytes(f), Item::Bytes(_), None) if f.len() == 32 => Ok(()),
                (Item::Bytes(f), Item::Bytes(_), Some(Item::Bytes(_))) if f.len() == 32 => Ok(()),
                _ => Err(Error("relayed payload shape")),
            },
            _ => Err(Error("relayed payload is a two- or three-element array")),
        },
        _ => Ok(()),
    }
}

/// A transaction body (§4) met without its envelope, as a fixture presents
/// one: the type its shape implies, then that type's rules.  Inside an
/// envelope the type is named, and [`check_body_of_type`] takes it from
/// there rather than guessing.
pub fn check_body(b: &[u8], item: &Item) -> Result<(), Error> {
    let Item::Map(m) = item else {
        return Err(Error("not map"));
    };
    let tx_type = match map_get(m, 3) {
        Some(Item::Map(_)) if matches!(map_get(m, 4), Some(Item::Map(_))) => 4,
        Some(Item::Map(_)) => 1,
        Some(Item::Array(a))
            if a.iter().all(|x| matches!(x, Item::Map(_))) && map_get(m, 6).is_some() =>
        {
            5
        }
        Some(Item::Array(_)) if matches!(map_get(m, 4), Some(Item::Array(_))) => 7,
        Some(Item::Array(_)) => 2,
        Some(Item::Uint(_)) => 3,
        _ => return Err(Error("field 3 required")),
    };
    check_body_of_type(b, item, tx_type)
}

/// A `seqno` pair (§2.3): `[series, counter]`, two uints.
fn seqno_of(it: Option<&Item>) -> Result<(u64, u64), Error> {
    let Some(Item::Array(a)) = it else {
        return Err(Error("seqno not an array"));
    };
    if a.len() != 2 {
        return Err(Error("seqno arity"));
    }
    let (series, counter) = (
        as_uint(&a[0]).ok_or(Error("seqno series"))?,
        as_uint(&a[1]).ok_or(Error("seqno counter"))?,
    );
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
    let Item::Map(m) = item else {
        return Err(Error("not map"));
    };
    let Item::Array(lists) = map_get(m, 0).ok_or(Error("key 0"))? else {
        return Err(Error("key0 not array"));
    };
    if lists.is_empty() {
        return Err(Error(
            "key 0 carries one list per signer, and there is a signer",
        ));
    }
    for l in lists {
        let Item::Array(hs) = l else {
            return Err(Error("list"));
        };
        if hs.is_empty() || hs.len() > MERGE_BACK_POINTERS_PER_SIGNER {
            return Err(Error("back-pointer bound"));
        }
        // each entry is a 32-byte txid, and **a merge list is sorted**
        // (§3.1): one logical merge, one encoding, one txid
        let mut prev: Option<&[u8]> = None;
        for h in hs {
            let Item::Bytes(r) = h else {
                return Err(Error("back-pointer not a byte string"));
            };
            if r.len() != 32 {
                return Err(Error("back-pointer width"));
            }
            let this = &b[r.clone()];
            if prev.is_some_and(|p| this <= p) {
                return Err(Error(
                    "a merge list is sorted ascending and repeats nothing",
                ));
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
    // §3.4, the rule's own example: an adoption carrying the adopted
    // node's key material in field 5 names that node in field 1, and the
    // material MUST hash to it.  Otherwise a sender presents one
    // identity's keyhash beside another's key and a recipient pinning from
    // the transaction pins the wrong key [2026-09-30].
    if tx_type == 1
        && let Some(r5) = value_slice(b, 5)
        && let Some(adopted) = keyhash_at(b, m, 1)
        && crate::cose::sha256(&b[r5]) != adopted
    {
        return Err(Error(
            "the adopted node's key material does not hash to field 1",
        ));
    }
    match tx_type {
        1 => {
            let Some(Item::Map(loc)) = map_get(m, 3) else {
                return Err(Error("adoption field 3 not a locator"));
            };
            let r3 = value_slice(b, 3).ok_or(Error("field 3"))?;
            // the locator's own shape, its packed path included (§2.1,
            // §2.3), its unknown keys the extensions a signed body keeps
            check_map_signed(b, r3.start, LOCATOR)?;
            extension_bounds(b, r3.start, |k| (1..=3).contains(&k))?;
            map_get(m, 4)
                .and_then(as_uint)
                .ok_or(Error("adoption timestamp uint"))?;
            if seqno_of(map_get(loc, 3))?.1 != 0 {
                return Err(Error("adoption counter not 0"));
            }
            // exactly one evidence form (§4.1, design §6.1.1): a recovery's
            // own block, a presence record's txid, or a former patron's
            // statement; none, or more than one, is malformed
            if [6u64, 8, 9]
                .iter()
                .filter(|k| map_get(m, **k).is_some())
                .count()
                != 1
            {
                return Err(Error(
                    "an adoption carries exactly one of fields 6, 8 and 9",
                ));
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
            map_get(m, 4)
                .and_then(as_uint)
                .ok_or(Error("departure timestamp uint"))?;
            if map_get(m, 5).is_some()
                && map_get(m, 5)
                    .and_then(as_uint)
                    .ok_or(Error("reason code uint"))?
                    > 63
            {
                return Err(Error("departure reason code out of space"));
            }
            Ok(())
        }
        3 => {
            map_get(m, 3)
                .and_then(as_uint)
                .ok_or(Error("disavowal timestamp uint"))?;
            if map_get(m, 4).is_some()
                && map_get(m, 4).and_then(as_uint).ok_or(Error("code uint"))? > 63
            {
                return Err(Error("disavowal code out of space"));
            }
            Ok(())
        }
        4 => {
            // each endpoint is a network point where that peer runs an
            // instance and a locator otherwise (§4.4), told apart by key 1:
            // only a 32-byte anchor keyhash is a locator, and everything
            // else is judged as a network point — so a malformed address
            // still fails as an address ("address width"), not as a shape
            // nobody claimed
            for k in [3u64, 4] {
                let r = value_slice(b, k).ok_or(Error("peering endpoint"))?;
                let is_locator = matches!(
                    map_get(m, k),
                    Some(Item::Map(fm)) if matches!(map_get(fm, 1), Some(Item::Bytes(k1)) if k1.len() == 32)
                );
                if is_locator {
                    check_type(b, r.start, Locator)?;
                } else {
                    network_point_at(b, r.start, true)?;
                }
            }
            let r3 = value_slice(b, 3).ok_or(Error("field 3"))?;
            extension_bounds(b, r3.start, |k| (1..=3).contains(&k))?;
            nested_extension_bounds(b, 0, 4, &|k| (1..=3).contains(&k))?;
            map_get(m, 5)
                .and_then(as_uint)
                .ok_or(Error("peering timestamp uint"))?;
            if let Some(Item::Array(audits)) = map_get(m, 7)
                && audits.len() > PEERING_AUDIT_HISTORY
            {
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
            map_get(m, 5)
                .and_then(as_uint)
                .ok_or(Error("reissue timestamp uint"))?;
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

/// Whether a signature object is a `COSE_Sign1` — one signature, and in
/// this profile therefore classical — rather than a `COSE_Sign` carrying
/// an entries array.
///
/// **Both are four-element arrays and the fourth element tells them
/// apart**: a `bstr` is the one signature of a `Sign1`, an array is a
/// `Sign`'s entries.  The shape asked of the `Sign1` is [`sign1_shape`]'s,
/// the one every signature slot is held to, so the two never disagree.
fn is_sign1(it: Option<&Item>) -> bool {
    it.is_some_and(|x| sign1_shape(x).is_ok())
}

/// Whether it is a hybrid `COSE_Sign` in the envelope's shape (§1, §3.5):
/// protected header bytes, an empty unprotected header, a nil payload,
/// and an entries array of two `COSE_Signature`s, one classical and one
/// post-quantum.
fn is_hybrid_sign(it: Option<&Item>) -> bool {
    matches!(it, Some(Item::Array(a))
        if a.len() == 4
            && matches!(a[0], Item::Bytes(_))
            && matches!(&a[1], Item::Map(u) if u.is_empty())
            && matches!(a[2], Item::Null)
            && matches!(&a[3], Item::Array(e)
                if e.len() == 2 && e.iter().all(|x| cose_signature_shape(x).is_ok())))
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
        return Err(Error(
            "a response's consent signature is not a classical COSE_Sign1",
        ));
    }
    match in_recovery {
        false if !is_sign1(map_get(x, 9)) => Err(Error(
            "a response's verifier signature is not a COSE_Sign1 in a presence record",
        )),
        true if !is_hybrid_sign(map_get(x, 9)) => Err(Error(
            "a response's verifier signature is not a hybrid COSE_Sign inside a recovery",
        )),
        _ => Ok(()),
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
    let Some(Item::Array(resp)) = map_get(rm, 2) else {
        return Err(Error("recovery responses not array"));
    };
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
        let Item::Map(x) = r else {
            return Err(Error("response not map"));
        };
        let verifier = keyhash_at(b, x, 1).ok_or(Error("response verifier"))?;
        let subject = keyhash_at(b, x, 2).ok_or(Error("response subject"))?;
        if subject != node {
            return Err(Error(
                "a response names a subject other than the adopted node",
            ));
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
    // and it is a 32-byte digest, the width the verifier will slice
    if keyhash_at(b, m, 8).is_none() {
        return Err(Error("presence field 8 is a 32-byte digest"));
    }
    let Some(Item::Array(parts)) = map_get(m, 3) else {
        return Err(Error("participants not array"));
    };
    if parts.len() != 2 {
        return Err(Error("a presence record names two participants"));
    }
    let mut keys = Vec::new();
    for p in parts {
        let Item::Map(pm) = p else {
            return Err(Error("participant not map"));
        };
        keys.push(keyhash_at(b, pm, 1).ok_or(Error("participant keyhash"))?);
    }
    if keys[0] == keys[1] {
        return Err(Error("the two participant identities are one"));
    }
    let sub = map_get(m, 6)
        .and_then(as_uint)
        .ok_or(Error("subtype uint"))?;
    if sub > 1 {
        return Err(Error("subtype out of range"));
    }
    let s = map_get(m, 1)
        .and_then(as_uint)
        .ok_or(Error("started_at uint"))?;
    let f = map_get(m, 2)
        .and_then(as_uint)
        .ok_or(Error("finalized_at uint"))?;
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
            return Err(Error(
                "a formation carries one back-pointer list per participant",
            ));
        }
        for (l, k) in lists.iter().zip(&keys) {
            let Item::Array(hs) = l else {
                return Err(Error("list"));
            };
            let genesis = crate::cose::sha256(k);
            let ok =
                hs.len() == 1 && matches!(&hs[0], Item::Bytes(r) if b[r.clone()] == genesis[..]);
            if !ok {
                return Err(Error("a formation's back-pointers are the genesis value"));
            }
        }
        return Ok(());
    }
    // a normal record carries witnesses, one to sixteen, and at least one
    // attesting that the protocol ran and both were responsive
    let Some(Item::Array(ws)) = map_get(m, 4) else {
        return Err(Error("a normal record carries witnesses"));
    };
    if ws.is_empty() || ws.len() > WITNESSES_PER_RECORD {
        return Err(Error("witnesses out of 1..=16"));
    }
    let mut affirmative = false;
    // §3.2: a witness is named once, and is neither participant
    let mut seen_witnesses: std::collections::BTreeSet<_> = std::collections::BTreeSet::new();
    for w in ws {
        let Item::Map(wm) = w else {
            return Err(Error("witness not map"));
        };
        if map_get(wm, 4).is_some() || map_get(wm, 5).is_some() {
            return Err(Error("retired witness key"));
        }
        let wk = keyhash_at(b, wm, 1).ok_or(Error("witness keyhash"))?;
        if keys.contains(&wk) {
            return Err(Error("a witness is one of the participants"));
        }
        if !seen_witnesses.insert(wk) {
            return Err(Error("a witness is named twice"));
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
            let Item::Map(x) = r else {
                return Err(Error("response not map"));
            };
            check_kind(b, "VerifierResponse", r)?;
            let verifier = keyhash_at(b, x, 1).ok_or(Error("response verifier"))?;
            let subject = keyhash_at(b, x, 2).ok_or(Error("response subject"))?;
            // field 8 is `prior_key`, confined to a recovery (§4.1, §5.5):
            // on a presence record's response it is malformed
            if map_get(x, 8).is_some() {
                return Err(Error("a presence response carries no prior_key"));
            }
            if !keys.contains(&subject) {
                return Err(Error("a response names a subject who is not a participant"));
            }
            if verifier == subject {
                return Err(Error("a response's verifier is its subject"));
            }
            if prev.is_some_and(|p| (verifier, subject) <= p) {
                return Err(Error(
                    "responses unsorted, or one verifier twice for one subject",
                ));
            }
            check_response_signatures(x, false)?;
            prev = Some((verifier, subject));
        }
    }
    Ok(())
}

#[cfg(test)]
mod shapes {
    //! The shapes the kinds above are held to, each built by hand so a
    //! test says exactly which byte it is about.  Nothing here is signed:
    //! a signature slot carries the profile's shape and arbitrary bytes.
    use super::*;
    use crate::encode::*;

    fn checked(kind: &str, b: &[u8]) -> Result<(), Error> {
        check_kind(b, kind, &parse_all(b)?)
    }

    fn bstr(out: &mut Vec<u8>, n: usize, fill: u8) {
        emit_bstr(out, &vec![fill; n]);
    }

    /// A `COSE_Sign1` in the profile's shape, with `unprotected` keys in
    /// its unprotected header: zero is the shape, anything else is not.
    fn sign1(out: &mut Vec<u8>, unprotected: usize) {
        emit_array_head(out, 4);
        emit_bstr(out, &[0xa0]);
        emit_map_head(out, unprotected);
        for k in 0..unprotected {
            emit_uint(out, k as u64 + 1);
            emit_uint(out, 0);
        }
        emit_null(out);
        emit_bstr(out, &[7u8; 64]);
    }

    /// One `COSE_Signature` entry as an envelope carries it.
    fn signature(out: &mut Vec<u8>) {
        emit_array_head(out, 3);
        emit_bstr(out, &[0xa0]);
        emit_map_head(out, 0);
        emit_bstr(out, &[7u8; 64]);
    }

    fn subtree_ack(node_width: usize, timestamp_as_text: bool) -> Vec<u8> {
        let mut o = Vec::new();
        emit_map_head(&mut o, 5);
        emit_uint(&mut o, 1);
        bstr(&mut o, 32, 1);
        emit_uint(&mut o, 2);
        bstr(&mut o, 32, 2);
        emit_uint(&mut o, 3);
        bstr(&mut o, node_width, 3);
        emit_uint(&mut o, 4);
        if timestamp_as_text {
            emit_tstr(&mut o, "now");
        } else {
            emit_uint(&mut o, 1_700_000_000);
        }
        emit_uint(&mut o, 5);
        sign1(&mut o, 0);
        o
    }

    #[test]
    fn a_subtree_ack_is_held_to_its_fields() {
        assert!(checked("SubtreeAck", &subtree_ack(32, false)).is_ok());
        assert!(
            checked("SubtreeAck", &subtree_ack(31, false)).is_err(),
            "the admitted node's keyhash is 32 bytes"
        );
        assert!(
            checked("SubtreeAck", &subtree_ack(32, true)).is_err(),
            "a timestamp is a uint"
        );
    }

    fn refusal(particular: usize) -> Vec<u8> {
        let mut o = Vec::new();
        emit_map_head(&mut o, 2);
        emit_uint(&mut o, 2);
        emit_uint(&mut o, 1);
        emit_uint(&mut o, 3);
        bstr(&mut o, particular, 9);
        o
    }

    #[test]
    fn a_signing_reply_particular_is_exactly_32_bytes() {
        assert!(checked("SigningReply", &refusal(32)).is_ok());
        assert!(checked("SigningReply", &refusal(31)).is_err());
        assert!(checked("SigningReply", &refusal(33)).is_err());
    }

    fn refusal_code(code: u64, particular: Option<usize>) -> Vec<u8> {
        let mut o = Vec::new();
        emit_map_head(&mut o, 1 + usize::from(particular.is_some()));
        emit_uint(&mut o, 2);
        emit_uint(&mut o, code);
        if let Some(n) = particular {
            emit_uint(&mut o, 3);
            bstr(&mut o, n, 9);
        }
        o
    }

    /// Refusals 1 and 2 name a witness or a query; 3 and 4 name nothing;
    /// and the codes run 1 to 4 (`wire-format.md` §7.10.2).
    #[test]
    fn a_signing_reply_refusal_carries_its_particular_only_where_it_names_one() {
        for code in [1, 2] {
            assert!(checked("SigningReply", &refusal_code(code, Some(32))).is_ok());
            assert!(checked("SigningReply", &refusal_code(code, None)).is_err());
        }
        for code in [3, 4] {
            assert!(checked("SigningReply", &refusal_code(code, None)).is_ok());
            assert!(checked("SigningReply", &refusal_code(code, Some(32))).is_err());
        }
        assert!(checked("SigningReply", &refusal_code(0, None)).is_err());
        assert!(checked("SigningReply", &refusal_code(5, None)).is_err());
    }

    fn proposed_body(body: &[u8]) -> Vec<u8> {
        let mut o = Vec::new();
        emit_map_head(&mut o, 2);
        emit_uint(&mut o, 1);
        emit_bstr(&mut o, body);
        emit_uint(&mut o, 2);
        emit_array_head(&mut o, 7);
        for label in [
            "capture",
            "location",
            "p0.integrity",
            "p0.retention",
            "p1.integrity",
            "p1.retention",
            "proximity",
        ] {
            emit_array_head(&mut o, 3);
            bstr(&mut o, 16, 4);
            emit_tstr(&mut o, label);
            emit_uint(&mut o, 0);
        }
        o
    }

    #[test]
    fn a_proposed_body_carries_a_body_the_archives_would_take() {
        // an empty map is a byte string, and no presence body; the
        // corpus's `P-proposed-body` is the one that passes
        assert!(checked("ProposedBody", &proposed_body(&[0xa0])).is_err());
        assert!(checked("ProposedBody", &proposed_body(&[])).is_err());
    }

    /// A `PrekeyBundle` with fields 1 to 5, 6 as well when `signed`, less
    /// the one `drop` names.
    fn bundle(signed: bool, drop: Option<u64>) -> Vec<u8> {
        let last = if signed { 6 } else { 5 };
        let keys: Vec<u64> = (1..=last).filter(|k| Some(*k) != drop).collect();
        let mut o = Vec::new();
        emit_map_head(&mut o, keys.len());
        for k in keys {
            emit_uint(&mut o, k);
            match k {
                1 | 5 => bstr(&mut o, 32, k as u8),
                2 => emit_uint(&mut o, 1),
                3 => bstr(&mut o, 100, 3),
                4 => emit_uint(&mut o, 1_700_000_000),
                _ => sign1(&mut o, 0),
            }
        }
        o
    }

    fn introduction(bundle: &[u8]) -> Vec<u8> {
        let mut o = Vec::new();
        emit_array_head(&mut o, 3);
        emit_uint(&mut o, 1);
        bstr(&mut o, 32, 8);
        emit_bstr(&mut o, bundle);
        o
    }

    fn credential(bundle: &[u8]) -> Vec<u8> {
        let mut o = Vec::new();
        emit_array_head(&mut o, 3);
        emit_uint(&mut o, 1);
        emit_array_head(&mut o, 1);
        bstr(&mut o, 40, 9);
        emit_bstr(&mut o, bundle);
        o
    }

    #[test]
    fn a_carried_bundle_is_a_prekey_bundle_on_either_side_of_its_signature() {
        assert!(checked("DeviceIntroduction", &introduction(&bundle(false, None))).is_ok());
        assert!(checked("DeviceCredential", &credential(&bundle(true, None))).is_ok());
        // an empty map parses and carries no field 6, and is no bundle
        assert!(checked("DeviceIntroduction", &introduction(&[0xa0])).is_err());
        for k in 1..=5 {
            assert!(
                checked("DeviceIntroduction", &introduction(&bundle(false, Some(k)))).is_err(),
                "an introduction's bundle without field {k}"
            );
            assert!(
                checked("DeviceCredential", &credential(&bundle(true, Some(k)))).is_err(),
                "a credential's bundle without field {k}"
            );
        }
        // the signature slot stays on the side the carriage puts it
        assert!(checked("DeviceIntroduction", &introduction(&bundle(true, None))).is_err());
        assert!(checked("DeviceCredential", &credential(&bundle(false, None))).is_err());
    }

    fn channel(unknown_key: bool) -> Vec<u8> {
        let mut o = Vec::new();
        emit_map_head(&mut o, if unknown_key { 3 } else { 2 });
        emit_uint(&mut o, 1);
        emit_uint(&mut o, 2);
        emit_uint(&mut o, 2);
        emit_uint(&mut o, 0);
        if unknown_key {
            emit_uint(&mut o, 9);
            emit_uint(&mut o, 1);
        }
        o
    }

    fn outcomes(ch: &[u8]) -> Vec<u8> {
        let mut o = Vec::new();
        emit_array_head(&mut o, 3);
        emit_uint(&mut o, 1);
        bstr(&mut o, 32, 5);
        emit_array_head(&mut o, 1);
        o.extend_from_slice(ch);
        o
    }

    fn witness_request(ch: &[u8]) -> Vec<u8> {
        let mut o = Vec::new();
        emit_map_head(&mut o, 4);
        emit_uint(&mut o, 1);
        bstr(&mut o, 32, 5);
        emit_uint(&mut o, 2);
        emit_array_head(&mut o, 2);
        bstr(&mut o, 32, 6);
        bstr(&mut o, 32, 7);
        emit_uint(&mut o, 3);
        emit_uint(&mut o, 1_700_000_000);
        emit_uint(&mut o, 4);
        emit_array_head(&mut o, 1);
        o.extend_from_slice(ch);
        o
    }

    fn proximity(ch: &[u8]) -> Vec<u8> {
        let mut o = Vec::new();
        emit_map_head(&mut o, 2);
        emit_uint(&mut o, 1);
        emit_array_head(&mut o, 1);
        o.extend_from_slice(ch);
        emit_uint(&mut o, 2);
        emit_uint(&mut o, 2);
        o
    }

    #[test]
    fn an_unknown_channel_key_is_refused_unsigned_and_kept_signed() {
        assert!(checked("ProximityOutcomes", &outcomes(&channel(false))).is_ok());
        assert!(checked("ProximityOutcomes", &outcomes(&channel(true))).is_err());
        assert!(checked("WitnessRequest", &witness_request(&channel(false))).is_ok());
        assert!(checked("WitnessRequest", &witness_request(&channel(true))).is_err());
        // a record's proximity is under its signature: the key is kept
        assert!(checked("Proximity", &proximity(&channel(false))).is_ok());
        assert!(checked("Proximity", &proximity(&channel(true))).is_ok());
    }

    #[test]
    fn one_notion_of_a_sign1_for_the_slots_and_the_responses() {
        let mut ok = Vec::new();
        sign1(&mut ok, 0);
        let mut bad = Vec::new();
        sign1(&mut bad, 1);
        let ok = parse_all(&ok).unwrap();
        let bad = parse_all(&bad).unwrap();
        assert!(is_sign1(Some(&ok)) && sign1_shape(&ok).is_ok());
        assert!(!is_sign1(Some(&bad)) && sign1_shape(&bad).is_err());
        let mut hybrid = Vec::new();
        emit_array_head(&mut hybrid, 4);
        emit_bstr(&mut hybrid, &[]);
        emit_map_head(&mut hybrid, 0);
        emit_null(&mut hybrid);
        emit_array_head(&mut hybrid, 2);
        signature(&mut hybrid);
        signature(&mut hybrid);
        let hybrid = parse_all(&hybrid).unwrap();
        assert!(is_hybrid_sign(Some(&hybrid)) && !is_sign1(Some(&hybrid)));
        assert!(!is_hybrid_sign(Some(&ok)));
    }
}
