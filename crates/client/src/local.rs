//! The ceremony's local interfaces (`wire-format.md` §14.3): the objects two
//! devices in each other's presence exchange, and nothing a network carries.
//!
//! **These live here so that no shell parses wire bytes.** The boundary
//! above this crate hands over opaque byte strings and carries them on
//! whatever bearer the two devices have — a QR on a screen, a local radio,
//! a fetch through FuerosNet (§14.3.1 ranks them by locality). What it must
//! never do is read them: a second parser above the kernel is the hazard
//! design §11.2 names, and every check §14.3 requires is a check against
//! the ceremony's own state, which only this side holds.
//!
//! **The anchor authenticates nobody** (§14.3.1), and these functions claim
//! nothing more than the section does. A check against the anchor catches a
//! bearer that *contradicts* the screens; a co-present adversary that has
//! read them passes every one. What stands in its place is §14.1's
//! property, and it is physical.

use crate::Keyhash;
use crate::device::{ChannelKind, ChannelOutcome, ChannelResult};
use rhtn_codec::cbor::*;
use rhtn_codec::encode::*;
use rhtn_codec::schema;
use zeroize::Zeroizing;

/// The first QR each device shows (§14.3.2): who is showing it, and the
/// 16-byte contribution the ceremony-id is derived from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpticalContribution {
    /// The device's identity in full (§2.2's `KeyMaterial`): the first
    /// contact pins it (design §12.3), and its hash is the keyhash.
    pub material: Vec<u8>,
    /// The keyhash, computed from the material on decode.
    pub device: Keyhash,
    /// The 16 bytes this device contributes to the ceremony-id.
    pub contribution: [u8; 16],
}

impl OpticalContribution {
    /// The bytes of this `OpticalContribution`, as §14.3.2 composes them.
    pub fn encode(&self) -> Vec<u8> {
        let mut out = Vec::new();
        emit_array_head(&mut out, 3);
        emit_uint(&mut out, 1);
        out.extend_from_slice(&self.material);
        emit_bstr(&mut out, &self.contribution);
        out
    }

    /// Read a `OpticalContribution` from `b`; an error naming what did not read.
    pub fn decode(b: &[u8]) -> Result<Self, String> {
        let it = checked(b, "OpticalContribution")?;
        let a = fields(&it);
        // the schema fixed the shape: array head, version 1, a KeyMaterial,
        // then the 16-byte contribution as the last 17 bytes; the material
        // is what sits between
        let contribution = fixed16(b, &a[2])?;
        let material = b[2..b.len() - 17].to_vec();
        let device = rhtn_crypto::Identity::from_key_material(&material)
            .ok_or("the key material does not read as an identity")?
            .keyhash;
        Ok(OpticalContribution {
            material,
            device,
            contribution,
        })
    }
}

/// The second QR, once both contributions are in (§14.3.2): the ceremony-id
/// this device computed, for the other to check against its own.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TranscriptConfirm {
    /// The ceremony-id this device computed from both contributions.
    pub ceremony_id: [u8; 32],
}

impl TranscriptConfirm {
    /// The bytes of this `TranscriptConfirm`, as §14.3.2 composes them.
    pub fn encode(&self) -> Vec<u8> {
        let mut out = Vec::new();
        emit_array_head(&mut out, 2);
        emit_uint(&mut out, 1);
        emit_bstr(&mut out, &self.ceremony_id);
        out
    }

    /// Read a `TranscriptConfirm` from `b`; an error naming what did not read.
    pub fn decode(b: &[u8]) -> Result<Self, String> {
        let it = checked(b, "TranscriptConfirm")?;
        let a = fields(&it);
        Ok(TranscriptConfirm {
            ceremony_id: fixed32(b, &a[1])?,
        })
    }
}

/// What the bearer carries first (§14.3.2): the sender's echoed
/// contribution, its nominees, the first carriage of its evidence bundle,
/// its clock, the retention it commits to, who initiated, and how many
/// `BundleContinuation` messages follow.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IntentExchange {
    /// The sender's own contribution, echoed.
    pub contribution: [u8; 16],
    /// Whom it nominates to witness.
    pub nominees: Vec<Keyhash>,
    /// The first carriage of its evidence bundle.
    pub bundle: Vec<Vec<u8>>,
    /// Its clock, in milliseconds since the epoch.
    pub started_at: u64,
    /// The retention it commits to, in years.
    pub retention_years: u64,
    /// Whether this side initiated.
    pub initiator: bool,
    /// How many `BundleContinuation` messages follow.
    pub continuations: u64,
}

impl IntentExchange {
    /// The bytes of this `IntentExchange`, as §14.3.2 composes them.
    pub fn encode(&self) -> Vec<u8> {
        let mut out = Vec::new();
        emit_array_head(&mut out, 8);
        emit_uint(&mut out, 1);
        emit_bstr(&mut out, &self.contribution);
        emit_array_head(&mut out, self.nominees.len());
        for n in &self.nominees {
            emit_bstr(&mut out, n);
        }
        emit_array_head(&mut out, self.bundle.len());
        for e in &self.bundle {
            out.extend_from_slice(e);
        }
        emit_uint(&mut out, self.started_at);
        emit_uint(&mut out, self.retention_years);
        emit_bool(&mut out, self.initiator);
        emit_uint(&mut out, self.continuations);
        out
    }

    /// Read a `IntentExchange` from `b`; an error naming what did not read.
    pub fn decode(b: &[u8]) -> Result<Self, String> {
        let it = checked(b, "IntentExchange")?;
        let a = fields(&it);
        Ok(IntentExchange {
            contribution: fixed16(b, &a[1])?,
            nominees: keyhashes(b, &a[2])?,
            bundle: entries(b, 3)?,
            started_at: as_uint(&a[4]).ok_or("started_at")?,
            retention_years: as_uint(&a[5]).ok_or("retention")?,
            initiator: matches!(a[6], Item::Bool(true)),
            continuations: as_uint(&a[7]).ok_or("continuations")?,
        })
    }
}

/// The bundle past its first carriage (§14.3.2, §5.4): anchored to the
/// ceremony-id and numbered from one, so a receiver tells a missing
/// continuation from the end of the bundle.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BundleContinuation {
    /// The ceremony this carriage belongs to.
    pub ceremony_id: [u8; 32],
    /// Which carriage this is, numbered from one.
    pub index: u64,
    /// The entries it carries.
    pub entries: Vec<Vec<u8>>,
}

impl BundleContinuation {
    /// The bytes of this `BundleContinuation`, as §14.3.2 composes them.
    pub fn encode(&self) -> Vec<u8> {
        let mut out = Vec::new();
        emit_array_head(&mut out, 4);
        emit_uint(&mut out, 1);
        emit_bstr(&mut out, &self.ceremony_id);
        emit_uint(&mut out, self.index);
        emit_array_head(&mut out, self.entries.len());
        for e in &self.entries {
            out.extend_from_slice(e);
        }
        out
    }

    /// Read a `BundleContinuation` from `b`; an error naming what did not read.
    pub fn decode(b: &[u8]) -> Result<Self, String> {
        let it = checked(b, "BundleContinuation")?;
        let a = fields(&it);
        Ok(BundleContinuation {
            ceremony_id: fixed32(b, &a[1])?,
            index: as_uint(&a[2]).ok_or("index")?,
            entries: entries(b, 3)?,
        })
    }
}

/// What the distance channels measured (§14.3.2), anchored: the §4.5
/// `Channel` maps exactly as a record carries them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProximityOutcomes {
    /// The ceremony these measurements belong to.
    pub ceremony_id: [u8; 32],
    /// What each channel measured, as a record carries them.
    pub channels: Vec<ChannelOutcome>,
}

impl ProximityOutcomes {
    /// The bytes of this `ProximityOutcomes`, as §14.3.2 composes them.
    pub fn encode(&self) -> Vec<u8> {
        let mut out = Vec::new();
        emit_array_head(&mut out, 3);
        emit_uint(&mut out, 1);
        emit_bstr(&mut out, &self.ceremony_id);
        emit_channels(&mut out, &self.channels);
        out
    }

    /// Read a `ProximityOutcomes` from `b`; an error naming what did not read.
    pub fn decode(b: &[u8]) -> Result<Self, String> {
        let it = checked(b, "ProximityOutcomes")?;
        let a = fields(&it);
        Ok(ProximityOutcomes {
            ceremony_id: fixed32(b, &a[1])?,
            channels: read_channels(&a[2])?,
        })
    }
}

/// The record's own `Channel` maps (§4.5) as an array: kind, result, and
/// the claimed resolution only where the client claims one.  One emitter
/// for every object that carries the channels as measured, here and on the
/// conversation (§7.10.2), so the two cannot drift apart.
pub(crate) fn emit_channels(out: &mut Vec<u8>, channels: &[ChannelOutcome]) {
    emit_array_head(out, channels.len());
    for c in channels {
        let n = 2 + usize::from(c.resolution_m.is_some());
        emit_map_head(out, n);
        emit_uint(out, 1);
        emit_uint(out, c.kind.code());
        emit_uint(out, 2);
        emit_uint(out, c.result as u64);
        if let Some(m) = c.resolution_m {
            emit_uint(out, 3);
            emit_uint(out, m);
        }
    }
}

/// The channels back from their array.
pub(crate) fn read_channels(it: &Item) -> Result<Vec<ChannelOutcome>, String> {
    let Item::Array(cs) = it else {
        return Err("channels not an array".into());
    };
    let mut channels = Vec::new();
    for c in cs {
        let Item::Map(m) = c else {
            return Err("channel not a map".into());
        };
        channels.push(ChannelOutcome {
            kind: map_get(m, 1)
                .and_then(as_uint)
                .and_then(ChannelKind::from_code)
                .ok_or("channel kind")?,
            result: match map_get(m, 2).and_then(as_uint) {
                Some(0) => ChannelResult::Pass,
                Some(1) => ChannelResult::Fail,
                Some(2) => ChannelResult::Unavailable,
                _ => return Err("channel result".into()),
            },
            resolution_m: map_get(m, 3).and_then(as_uint),
        });
    }
    Ok(channels)
}

/// Candidates on the ceremony's channels (§14.3.2, design §12.6.3): the
/// bare `[ 1*8 Candidate ]` array the payload path also carries, with the
/// anchor the local carriage adds.
///
/// The candidates stay the encoded array the transport produces: what is
/// handed over is an address to dial, and this side neither builds nor
/// reads one — reachability is settled by the dial.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CandidateHandover {
    /// The ceremony these candidates belong to.
    pub ceremony_id: [u8; 32],
    /// The encoded candidate array, as the transport produced it.
    pub candidates: Vec<u8>,
}

impl CandidateHandover {
    /// The bytes of this `CandidateHandover`, as §14.3.2 composes them.
    pub fn encode(&self) -> Vec<u8> {
        let mut out = Vec::new();
        emit_array_head(&mut out, 3);
        emit_uint(&mut out, 1);
        emit_bstr(&mut out, &self.ceremony_id);
        out.extend_from_slice(&self.candidates);
        out
    }

    /// Read a `CandidateHandover` from `b`; an error naming what did not read.
    pub fn decode(b: &[u8]) -> Result<Self, String> {
        let it = checked(b, "CandidateHandover")?;
        let a = fields(&it);
        let mut candidates = Vec::new();
        emit_array_head(&mut candidates, array_len(&a[2])?);
        let Item::Array(cs) = &a[2] else {
            return Err("candidates not an array".into());
        };
        for c in cs {
            let Item::Array(f) = c else {
                return Err("candidate not an array".into());
            };
            emit_array_head(&mut candidates, 3);
            emit_uint(&mut candidates, as_uint(&f[0]).ok_or("candidate kind")?);
            emit_bstr(&mut candidates, &bs(b, &f[1]).ok_or("candidate address")?);
            emit_uint(&mut candidates, as_uint(&f[2]).ok_or("candidate port")?);
        }
        Ok(CandidateHandover {
            ceremony_id: fixed32(b, &a[1])?,
            candidates,
        })
    }
}

/// The capture key, handed across at capture time (§14.3.2, design
/// §7.5.2.6): the 32 bytes the sender derived for the captures the
/// receiver holds of it, under the anchor the other local objects carry.
/// One crosses each way, and the receiver seals its captures beneath it
/// and lets it go once sealed (design §7.5.2).
///
/// **The key decrypts a likeness of a person, so nothing here keeps it**:
/// the key, and the encoded bytes that carry it, are wiped when dropped.
///
/// The anchor buys this what it buys the others and no more: a key from
/// another ceremony is refused here, and a co-present party quoting the
/// public ceremony-id is excluded by §14.1's property, not by this check.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CaptureKeyHandover {
    /// The ceremony this key belongs to; one from another is refused.
    pub ceremony_id: [u8; 32],
    /// The 32 bytes the sender derived, wiped when dropped.
    pub key: Zeroizing<[u8; 32]>,
}

impl CaptureKeyHandover {
    /// The bytes of this `CaptureKeyHandover`, as §14.3.2 composes them.
    pub fn encode(&self) -> Zeroizing<Vec<u8>> {
        let mut out = Zeroizing::new(Vec::new());
        emit_array_head(&mut out, 3);
        emit_uint(&mut out, 1);
        emit_bstr(&mut out, &self.ceremony_id);
        emit_bstr(&mut out, &self.key[..]);
        out
    }

    /// Read a `CaptureKeyHandover` from `b`; an error naming what did not read.
    pub fn decode(b: &[u8]) -> Result<Self, String> {
        let it = checked(b, "CaptureKeyHandover")?;
        let a = fields(&it);
        Ok(CaptureKeyHandover {
            ceremony_id: fixed32(b, &a[1])?,
            key: Zeroizing::new(fixed32(b, &a[2])?),
        })
    }
}

/// An intent and its bundle as the carriage set the bearer moves: the
/// exchange first, then one `BundleContinuation` per 256 entries beyond it
/// (§5.4, §14.3.2).
///
/// **Truncation is the sender's act and visible as one** (§5.4): a sender
/// that hands fewer entries than it holds produces a smaller *n*, and
/// nothing here hides that from the receiver.
pub fn intent_carriage(intent: &IntentExchange, ceremony_id: &[u8; 32]) -> Vec<Vec<u8>> {
    let cap = rhtn_codec::bounds::INTENT_BUNDLE_ENTRIES;
    let mut carriages = intent.bundle.chunks(cap);
    let first = carriages.next().unwrap_or(&[]).to_vec();
    let rest: Vec<Vec<Vec<u8>>> = carriages.map(<[Vec<u8>]>::to_vec).collect();
    let mut out = vec![
        IntentExchange {
            bundle: first,
            continuations: rest.len() as u64,
            nominees: intent.nominees.clone(),
            ..*intent
        }
        .encode(),
    ];
    for (i, entries) in rest.into_iter().enumerate() {
        out.push(
            BundleContinuation {
                ceremony_id: *ceremony_id,
                index: i as u64 + 1,
                entries,
            }
            .encode(),
        );
    }
    out
}

/// The carriage set back to one intent: the exchange, then its
/// continuations in order.  **The bundle a receiver evaluates is what it
/// accepted** (§5.4): a continuation whose anchor or index is wrong, or one
/// that never came, ends the bundle there with the entries held, and the
/// ceremony is not refused over it.  **And the exchange says how many
/// follow** (§14.3.2's `continuations`): a message past that count is not
/// this bundle's, whatever it carries, so a sender cannot stream
/// continuations for as long as the receiver's memory lasts -- the count
/// was on the wire for exactly this, and a receiver that did not consult it
/// had no bound at all.
pub fn read_intent(
    carriage: &[Vec<u8>],
    ceremony_id: &[u8; 32],
) -> Result<(IntentExchange, usize), String> {
    let (first, rest) = carriage.split_first().ok_or("no intent in the carriage")?;
    let mut intent = IntentExchange::decode(first)?;
    let declared = intent.continuations;
    let mut taken = 0usize;
    for (i, bytes) in rest.iter().enumerate() {
        if i as u64 >= declared {
            break;
        }
        let Ok(c) = BundleContinuation::decode(bytes) else {
            break;
        };
        // anchored to this ceremony, and consecutive from one
        if &c.ceremony_id != ceremony_id || c.index != i as u64 + 1 {
            break;
        }
        intent.bundle.extend(c.entries);
        taken += 1;
    }
    Ok((intent, taken))
}

// ---------------------------------------------------------------- helpers

/// Parse and put the bytes through the codec's own checker for that kind,
/// so what this module accepts is exactly what the schema accepts: an
/// encoder and a validator that disagree is two definitions of the object.
fn checked(b: &[u8], kind: &'static str) -> Result<Item, String> {
    let item = parse_all(b).map_err(|e| e.0)?;
    schema::check_kind(b, kind, &item).map_err(|e| e.0)?;
    if !matches!(item, Item::Array(_)) {
        return Err(format!("{kind} is not an array"));
    }
    Ok(item)
}

/// The fields of an array the checker has already approved: its arity is
/// what that kind's rule fixed, so indexing is safe past `checked`.
fn fields(it: &Item) -> &[Item] {
    match it {
        Item::Array(a) => a,
        _ => &[],
    }
}

pub(crate) fn fixed32(b: &[u8], it: &Item) -> Result<[u8; 32], String> {
    bs(b, it)
        .and_then(|s| <[u8; 32]>::try_from(s.as_slice()).ok())
        .ok_or_else(|| "a 32-byte field is not 32 bytes".into())
}

fn fixed16(b: &[u8], it: &Item) -> Result<[u8; 16], String> {
    bs(b, it)
        .and_then(|s| <[u8; 16]>::try_from(s.as_slice()).ok())
        .ok_or_else(|| "a 16-byte field is not 16 bytes".into())
}

pub(crate) fn bs(b: &[u8], it: &Item) -> Option<Vec<u8>> {
    match it {
        Item::Bytes(r) => Some(b[r.clone()].to_vec()),
        _ => None,
    }
}

fn array_len(it: &Item) -> Result<usize, String> {
    match it {
        Item::Array(a) => Ok(a.len()),
        _ => Err("not an array".into()),
    }
}

fn keyhashes(b: &[u8], it: &Item) -> Result<Vec<Keyhash>, String> {
    let Item::Array(a) = it else {
        return Err("nominees not an array".into());
    };
    a.iter().map(|n| fixed32(b, n)).collect()
}

/// The bundle entries as the exact bytes they arrived as.
///
/// An `ArchiveEntry` is **a CBOR item, not a byte string** (§7.9: an
/// envelope is a map and a presentation an array, "so the two need no
/// discriminator"), and each is a verifiable object of its own — §5.3
/// requires every one to verify alone. So an entry is carried by its byte
/// range and never re-encoded: re-encoding an object whose signature covers
/// its bytes is how a valid record becomes an invalid one.
///
/// `field` is the entry array's index among the object's own fields, which
/// is how the ranges are found: the parsed tree gives the scalars and the
/// offsets give the opaque items.
fn entries(b: &[u8], field: usize) -> Result<Vec<Vec<u8>>, String> {
    let top = array_item_ranges(b, 0).ok_or("not an array")?;
    let at = top.get(field).ok_or("no such field")?.start;
    let items = array_item_ranges(b, at).ok_or("bundle not an array")?;
    Ok(items.into_iter().map(|r| b[r].to_vec()).collect())
}
