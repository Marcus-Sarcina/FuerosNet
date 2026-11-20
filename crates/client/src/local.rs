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
use aws_lc_rs::aead::{AES_256_GCM, Aad, LessSafeKey, Nonce, UnboundKey};
use aws_lc_rs::rand::{SecureRandom, SystemRandom};
use rhtn_codec::cbor::*;
use rhtn_codec::encode::*;
use rhtn_codec::schema;
use zeroize::Zeroizing;

/// The first QR each device shows (§14.3.2): who is showing it, and the
/// 16-byte contribution the ceremony-id is derived from.
#[derive(Clone, PartialEq, Eq)]
pub struct OpticalContribution {
    /// The device's identity in full (§2.2's `KeyMaterial`): the first
    /// contact pins it (design §12.3), and its hash is the keyhash.
    pub material: Vec<u8>,
    /// The keyhash, computed from the material on decode.
    pub device: Keyhash,
    /// The 16 bytes this device contributes to the ceremony-id.
    pub contribution: [u8; 16],
}

impl std::fmt::Debug for OpticalContribution {
    /// Everything but the contribution, which is half of the session key
    /// (§14.3.2) and one `{:?}` away from a log line: the field is named
    /// and its value is not, as `ceremony::Intent` does [reviewer,
    /// 2026-10-08].
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("OpticalContribution")
            .field("material", &self.material.len())
            .field("device", &self.device)
            .field("contribution", &"<redacted>")
            .finish()
    }
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

/// What one device of an identity shows another for a dataset hand-over
/// (§14.3.3): the identity both hold, this device's contribution to the
/// pairing key, and its archive heads, so the other sends what these do not
/// reach.
#[derive(Clone, PartialEq, Eq)]
pub struct DatasetContribution {
    /// The identity's key material in full (§2.2).
    pub material: Vec<u8>,
    /// Its keyhash, computed from the material on decode.
    pub device: Keyhash,
    /// The 16 bytes this device contributes to the pairing key.
    pub contribution: [u8; 16],
    /// This device's archive heads.
    pub heads: Vec<crate::Txid>,
}

impl std::fmt::Debug for DatasetContribution {
    /// The contribution is half of the pairing key, and is not printed.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DatasetContribution")
            .field("material", &self.material.len())
            .field("device", &self.device)
            .field("contribution", &"<redacted>")
            .field("heads", &self.heads.len())
            .finish()
    }
}

impl DatasetContribution {
    /// The bytes of this `DatasetContribution`, as §14.3.3 composes them.
    pub fn encode(&self) -> Vec<u8> {
        let mut out = Vec::new();
        emit_array_head(&mut out, 4);
        emit_uint(&mut out, 1);
        out.extend_from_slice(&self.material);
        emit_bstr(&mut out, &self.contribution);
        emit_array_head(&mut out, self.heads.len());
        for h in &self.heads {
            emit_bstr(&mut out, h);
        }
        out
    }

    /// Read a `DatasetContribution` from `b`; an error naming what did not
    /// read.
    pub fn decode(b: &[u8]) -> Result<Self, String> {
        let item = parse_all(b).map_err(|e| format!("DatasetContribution: {}", e.0))?;
        let Item::Array(f) = &item else {
            return Err("DatasetContribution: not an array".into());
        };
        let [v, _, c, hs] = f.as_slice() else {
            return Err("DatasetContribution: not four fields".into());
        };
        if !matches!(v, Item::Uint(1)) {
            return Err("DatasetContribution: not version 1".into());
        }
        let ranges = array_item_ranges(b, 0).ok_or("DatasetContribution: item ranges")?;
        let material = b[ranges[1].clone()].to_vec();
        let device = rhtn_crypto::Identity::from_key_material(&material)
            .ok_or("the key material does not read as an identity")?
            .keyhash;
        let contribution = fixed16(b, c)?;
        let Item::Array(heads) = hs else {
            return Err("DatasetContribution: heads are not an array".into());
        };
        let heads = heads
            .iter()
            .map(|h| match h {
                Item::Bytes(r) if r.len() == 32 => b[r.clone()]
                    .try_into()
                    .map_err(|_| "DatasetContribution: a head".to_string()),
                _ => Err("DatasetContribution: a head is not 32 bytes".into()),
            })
            .collect::<Result<Vec<crate::Txid>, String>>()?;
        Ok(DatasetContribution {
            material,
            device,
            contribution,
            heads,
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
/// contribution, the first carriage of its bundle of introductions, its
/// clock, the retention it commits to, who initiated, how many
/// `BundleContinuation` messages follow, the sender's own
/// `SignedLocator` and its `EncryptionKey`.  The nominees no longer ride it [author, 2026-10-19]:
/// the sets travel by locator once the id has fixed (§7.10.2, kind 22).
#[derive(Clone, PartialEq, Eq)]
pub struct IntentExchange {
    /// The sender's own contribution, echoed.
    pub contribution: [u8; 16],
    /// The first carriage of its bundle: introductions (§2.3, §5.4).
    pub bundle: Vec<Vec<u8>>,
    /// Its clock, in milliseconds since the epoch.
    pub started_at: u64,
    /// The retention it commits to, in years.
    pub retention_years: u64,
    /// Whether this side initiated.
    pub initiator: bool,
    /// How many `BundleContinuation` messages follow.
    pub continuations: u64,
    /// The sender's own `SignedLocator`, the most recent it has signed.
    pub locator: Vec<u8>,
    /// The sender's own `EncryptionKey` (§2.5).
    pub encryption_key: Vec<u8>,
    /// **The number this device reports for itself**, where it has one
    /// and the platform gave it [author, 2026-11-10]: *"Pull from the
    /// handset at meeting time and pass it to the counterparty during the
    /// bluetooth session. Do not include it in the transaction record or
    /// send it to witnesses/verifiers. Store with the other contact
    /// information in the counterparty's device so it gets preserved
    /// through standard peering and backup."*
    ///
    /// **Nullable, and nothing asks for it.** A handset with no SIM, a
    /// carrier that does not provision the number, or a platform that
    /// refuses it all come to the same thing: none sent, nobody prompted
    /// (`light-client-requirements.md` §1.5).
    pub number: Option<String>,
}

impl std::fmt::Debug for IntentExchange {
    /// Everything but the echoed contribution, for the reason
    /// [`OpticalContribution`]'s gives.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("IntentExchange")
            .field("contribution", &"<redacted>")
            .field("bundle", &self.bundle.len())
            .field("started_at", &self.started_at)
            .field("retention_years", &self.retention_years)
            .field("initiator", &self.initiator)
            .field("continuations", &self.continuations)
            .field("locator", &self.locator.len())
            .field("encryption_key", &self.encryption_key.len())
            // **the number is not printed**: it is the counterparty's,
            // and a log is not where it belongs
            .field("number", &self.number.is_some())
            .finish()
    }
}

impl IntentExchange {
    /// The bytes of this `IntentExchange`, as §14.3.2 composes them.
    pub fn encode(&self) -> Vec<u8> {
        let mut out = Vec::new();
        // **nine fields, or ten with a number**: a device with none
        // sends what it always sent, so nothing has to be true of the
        // counterparty's build for the exchange to work
        emit_array_head(&mut out, if self.number.is_some() { 10 } else { 9 });
        emit_uint(&mut out, 1);
        emit_bstr(&mut out, &self.contribution);
        emit_array_head(&mut out, self.bundle.len());
        for e in &self.bundle {
            out.extend_from_slice(e);
        }
        emit_uint(&mut out, self.started_at);
        emit_uint(&mut out, self.retention_years);
        emit_bool(&mut out, self.initiator);
        emit_uint(&mut out, self.continuations);
        out.extend_from_slice(&self.locator);
        out.extend_from_slice(&self.encryption_key);
        if let Some(n) = &self.number {
            emit_tstr(&mut out, n);
        }
        out
    }

    /// Read a `IntentExchange` from `b`; an error naming what did not read.
    pub fn decode(b: &[u8]) -> Result<Self, String> {
        let it = checked(b, "IntentExchange")?;
        let a = fields(&it);
        Ok(IntentExchange {
            contribution: fixed16(b, &a[1])?,
            bundle: entries(b, 2)?,
            started_at: as_uint(&a[3]).ok_or("started_at")?,
            retention_years: as_uint(&a[4]).ok_or("retention")?,
            initiator: matches!(a[5], Item::Bool(true)),
            continuations: as_uint(&a[6]).ok_or("continuations")?,
            locator: raw_field(b, 7)?,
            encryption_key: raw_field(b, 8)?,
            number: match a.get(9) {
                Some(Item::Text(r)) => Some(String::from_utf8_lossy(&b[r.clone()]).into_owned()),
                _ => None,
            },
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
            locator: intent.locator.clone(),
            encryption_key: intent.encryption_key.clone(),
            number: intent.number.clone(),
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

/// **What a device with no camera shows** (§14.3.3): the transport key the
/// delegations will name, the offer's secret, and where the ceremony device
/// reaches it.  The secret keys the session everything after crosses
/// ([`DeviceSession`]) and crosses no other way.
#[derive(Clone, PartialEq, Eq)]
pub struct DeviceOffer {
    /// The raw transport key this device minted.
    pub device: [u8; 32],
    /// Drawn fresh per offer.
    pub secret: [u8; 32],
    /// Where the ceremony device's bearer reaches this one, in that
    /// bearer's own form.
    pub reach: String,
}

impl std::fmt::Debug for DeviceOffer {
    /// The secret keys the session, and is not printed.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DeviceOffer")
            .field("device", &self.device)
            .field("secret", &"<redacted>")
            .field("reach", &self.reach)
            .finish()
    }
}

impl DeviceOffer {
    /// The bytes of this `DeviceOffer`, as §14.3.3 composes them.
    pub fn encode(&self) -> Vec<u8> {
        let mut out = Vec::new();
        emit_array_head(&mut out, 4);
        emit_uint(&mut out, 1);
        emit_bstr(&mut out, &self.device);
        emit_bstr(&mut out, &self.secret);
        emit_tstr(&mut out, &self.reach);
        out
    }

    /// Read a `DeviceOffer` from `b`; an error naming what did not read.
    pub fn decode(b: &[u8]) -> Result<Self, String> {
        let item = parse_all(b).map_err(|e| format!("DeviceOffer: {}", e.0))?;
        let Item::Array(f) = &item else {
            return Err("DeviceOffer: not an array".into());
        };
        let [v, d, s, r] = f.as_slice() else {
            return Err("DeviceOffer: not four fields".into());
        };
        if !matches!(v, Item::Uint(1)) {
            return Err("DeviceOffer: not version 1".into());
        }
        let Item::Text(t) = r else {
            return Err("DeviceOffer: the reach is not text".into());
        };
        let reach = std::str::from_utf8(&b[t.clone()])
            .map_err(|_| "DeviceOffer: the reach is not UTF-8")?
            .to_string();
        if reach.is_empty() || reach.len() > 255 {
            return Err("DeviceOffer: the reach is 1 to 255 bytes".into());
        }
        Ok(DeviceOffer {
            device: fixed32(b, d)?,
            secret: fixed32(b, s)?,
            reach,
        })
    }

    /// The session this offer keys.
    pub fn session(&self) -> DeviceSession {
        DeviceSession {
            key: crate::keys::device_session_key(&self.secret),
            device: self.device,
        }
    }
}

/// **The session a device with no camera and the ceremony device speak
/// on** (§14.3.3): §14.3.2's sealing, keyed from the offer's secret, with
/// the offer's device key as the associated data.  Held for the offer's
/// lifetime and no longer: dropping it is letting the key go.
pub struct DeviceSession {
    key: Zeroizing<[u8; 32]>,
    device: [u8; 32],
}

impl DeviceSession {
    /// The device the offer named.
    pub fn device(&self) -> [u8; 32] {
        self.device
    }

    /// One carriage, sealed.
    pub fn seal(&self, plaintext: &[u8]) -> Vec<u8> {
        seal_carriage(&self.key, &self.device, plaintext)
    }

    /// One carriage opened, or nothing: from a party that did not read the
    /// offer, or under an offer since replaced.
    pub fn open(&self, sealed: &[u8]) -> Option<Zeroizing<Vec<u8>>> {
        open_carriage(&self.key, &self.device, sealed)
    }
}

/// **The ceremony device's first message on the device session** (§14.3.3):
/// the identity, and the run's first delegation, which the device stands
/// up on to mint the material its introduction offers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeviceStart {
    /// The identity's key material in full (§2.2).
    pub material: Vec<u8>,
    /// The run's first `Delegation` (§8.2).
    pub delegation: Vec<u8>,
}

impl DeviceStart {
    /// The bytes of this `DeviceStart`, as §14.3.3 composes them.
    pub fn encode(&self) -> Vec<u8> {
        let mut out = Vec::new();
        emit_array_head(&mut out, 3);
        emit_uint(&mut out, 1);
        out.extend_from_slice(&self.material);
        emit_bstr(&mut out, &self.delegation);
        out
    }

    /// Read a `DeviceStart` from `b`; an error naming what did not read.
    pub fn decode(b: &[u8]) -> Result<Self, String> {
        let item = parse_all(b).map_err(|e| format!("DeviceStart: {}", e.0))?;
        let Item::Array(f) = &item else {
            return Err("DeviceStart: not an array".into());
        };
        let [v, _, d] = f.as_slice() else {
            return Err("DeviceStart: not three fields".into());
        };
        if !matches!(v, Item::Uint(1)) {
            return Err("DeviceStart: not version 1".into());
        }
        let ranges = array_item_ranges(b, 0).ok_or("DeviceStart: item ranges")?;
        let material = b[ranges[1].clone()].to_vec();
        rhtn_crypto::Identity::from_key_material(&material)
            .ok_or("DeviceStart: the key material does not read as an identity")?;
        let delegation = bs(b, d)
            .filter(|d| !d.is_empty() && d.len() <= 4096)
            .ok_or("DeviceStart: a delegation is 1 to 4096 bytes")?;
        Ok(DeviceStart {
            material,
            delegation,
        })
    }
}

/// **A delegated device's introduction** (§14.3.3): the transport key it
/// minted, which the delegations will name.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeviceIntroduction {
    /// The raw transport key the delegations name.
    pub device: [u8; 32],
}

impl DeviceIntroduction {
    /// The bytes of this `DeviceIntroduction`, as §14.3.3 composes them.
    pub fn encode(&self) -> Vec<u8> {
        let mut out = Vec::new();
        emit_array_head(&mut out, 2);
        emit_uint(&mut out, 1);
        emit_bstr(&mut out, &self.device);
        out
    }

    /// Read a `DeviceIntroduction` from `b`, through the schema's own check.
    pub fn decode(b: &[u8]) -> Result<Self, String> {
        let item = checked(b, "DeviceIntroduction")?;
        let f = fields(&item);
        Ok(DeviceIntroduction {
            device: fixed32(b, &f[1])?,
        })
    }
}

/// **The ceremony device's answer** (§14.3.3): the run of delegations,
/// and the identity's two encryption secrets, so the device opens what is
/// sealed to the person and authenticates what it sends as the person's
/// [author, 2026-11-19]. **The secrets are wiped when dropped.**
#[derive(Clone, PartialEq, Eq)]
pub struct DeviceCredential {
    /// The run, contiguous 48-hour windows (§8.2).
    pub run: Vec<Vec<u8>>,
    /// The identity's X-Wing decapsulation seed (§2.5).
    pub kem_seed: Zeroizing<[u8; 32]>,
    /// The identity's sealing-authentication private key (§2.5).
    pub auth: Zeroizing<[u8; 32]>,
}

impl std::fmt::Debug for DeviceCredential {
    /// The run's length and nothing of the secrets.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DeviceCredential")
            .field("run", &self.run.len())
            .field("secrets", &"<redacted>")
            .finish()
    }
}

impl DeviceCredential {
    /// The bytes of this `DeviceCredential`, as §14.3.3 composes them;
    /// wiped when dropped, since they carry the secrets.
    pub fn encode(&self) -> Zeroizing<Vec<u8>> {
        let mut out = Zeroizing::new(Vec::new());
        emit_array_head(&mut out, 4);
        emit_uint(&mut out, 1);
        emit_array_head(&mut out, self.run.len());
        for d in &self.run {
            emit_bstr(&mut out, d);
        }
        emit_bstr(&mut out, &self.kem_seed[..]);
        emit_bstr(&mut out, &self.auth[..]);
        out
    }

    /// Read a `DeviceCredential` from `b`, through the schema's own check.
    pub fn decode(b: &[u8]) -> Result<Self, String> {
        let item = checked(b, "DeviceCredential")?;
        let f = fields(&item);
        let Item::Array(run) = &f[1] else {
            return Err("DeviceCredential: the run is not an array".into());
        };
        Ok(DeviceCredential {
            run: run
                .iter()
                .map(|d| bs(b, d).ok_or("DeviceCredential: a delegation".to_string()))
                .collect::<Result<_, _>>()?,
            kem_seed: Zeroizing::new(fixed32(b, &f[2])?),
            auth: Zeroizing::new(fixed32(b, &f[3])?),
        })
    }
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
/// The bytes of top-level field `field`, as they lie.
fn raw_field(b: &[u8], field: usize) -> Result<Vec<u8>, String> {
    let top = array_item_ranges(b, 0).ok_or("not an array")?;
    let r = top.get(field).ok_or("no such field")?;
    Ok(b[r.clone()].to_vec())
}

fn entries(b: &[u8], field: usize) -> Result<Vec<Vec<u8>>, String> {
    let top = array_item_ranges(b, 0).ok_or("not an array")?;
    let at = top.get(field).ok_or("no such field")?.start;
    let items = array_item_ranges(b, at).ok_or("bundle not an array")?;
    Ok(items.into_iter().map(|r| b[r].to_vec()).collect())
}

// -------------------------------------------------- the session's envelope

/// Seal one carriage under the local session's key (`wire-format.md`
/// §14.3.2).
///
/// **One encryption, at the session layer.** This wraps a whole encoded
/// object and nothing inside it is encrypted again: the objects above are
/// the plaintexts, their fields and checks exactly as specified, and the
/// `CaptureKeyHandover` is protected because it rides this rather than
/// because anything wraps it twice [author, 2026-10-05].
///
/// AES-256-GCM under the 32 bytes [`crate::keys::session_key`] derives,
/// with a random 96-bit nonce per carriage carried in front of the
/// ciphertext, matching [`crate::sealed`]'s construction so the kernel has
/// one AEAD shape and the shell has none.
///
/// **The ceremony-id is the associated data, and that is belt over
/// braces.** The key is already derived from this ceremony's two
/// contributions and so is unique to it, which is what actually stops a
/// carriage opening in another ceremony. The associated data states the
/// binding rather than creating it.
///
/// **The key always exists by the time a carriage moves.** Both
/// contributions cross optically before the first carriage does, so there
/// is no ordering in which a sender holds half a key.
pub fn seal_carriage(key: &[u8; 32], ceremony_id: &[u8; 32], plaintext: &[u8]) -> Vec<u8> {
    let mut nonce = [0u8; 12];
    SystemRandom::new()
        .fill(&mut nonce)
        .expect("the system's random source");
    let mut body = plaintext.to_vec();
    carriage_aead(key)
        .seal_in_place_append_tag(
            Nonce::assume_unique_for_key(nonce),
            Aad::from(ceremony_id),
            &mut body,
        )
        .expect("sealing appends a tag");
    let mut out = Vec::with_capacity(nonce.len() + body.len());
    out.extend_from_slice(&nonce);
    out.extend_from_slice(&body);
    out
}

/// Open one carriage, or nothing.
///
/// **A failure here is not distinguishable from a wrong key, a wrong
/// ceremony or a tampered byte, and does not need to be**: §14.3.1's
/// anchor checks are what tell a receiver *why* a carriage is refused, and
/// they run on the plaintext this returns. What this says is only whether
/// the bytes came from a holder of the session key.
///
/// A peer that sends plaintext fails here, which is the refusal a version
/// disagreement should produce: `wire-format.md` §14.3.2 has the session
/// encrypted, so unencrypted bytes are a shell this one cannot speak to.
pub fn open_carriage(
    key: &[u8; 32],
    ceremony_id: &[u8; 32],
    sealed: &[u8],
) -> Option<Zeroizing<Vec<u8>>> {
    if sealed.len() < 12 {
        return None;
    }
    let (n, body) = sealed.split_at(12);
    let mut buf = Zeroizing::new(body.to_vec());
    let opened = carriage_aead(key)
        .open_in_place(
            Nonce::assume_unique_for_key(n.try_into().ok()?),
            Aad::from(ceremony_id),
            &mut buf,
        )
        .ok()?
        .len();
    buf.truncate(opened);
    Some(buf)
}

fn carriage_aead(key: &[u8; 32]) -> LessSafeKey {
    LessSafeKey::new(UnboundKey::new(&AES_256_GCM, key).expect("a 32-byte key"))
}
