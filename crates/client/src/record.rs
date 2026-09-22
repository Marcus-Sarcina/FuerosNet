//! Record assembly and the checks a signer makes (`wire-format.md` §4.5,
//! §4.5.1, §7.4; `light-client-requirements.md` §1.1, §1.2, §1.4; design
//! §8.1.1): the body both participants and every witness sign, the
//! disclosure set and its root, a participant's refusals and notices before
//! signing, a witness's clock check, the presentation that withholds by
//! default, and a late response kept beside the record it supplements.

use crate::notice::{Notice, Notifier};
use crate::query::{LateResponse, Response};
use crate::store::ClientStore;
use crate::{Keyhash, Txid};
use rhtn_archive::record::Record;
use rhtn_archive::tx::{Witness, emit_back_pointers};
use rhtn_codec::cbor::*;
use rhtn_codec::cose::sha256;
use rhtn_codec::encode::*;
use rhtn_codec::schema;
use rhtn_crypto::verify::Lookup;
use std::collections::{BTreeMap, BTreeSet};

/// The seven disclosable labels, in ascending byte order
/// (`wire-format.md` §4.5.1.1).
pub const LABELS: [&str; 7] = [
    "capture",
    "location",
    "p0.integrity",
    "p0.retention",
    "p1.integrity",
    "p1.retention",
    "proximity",
];

/// One disclosable field: its label, a fresh 16-byte salt, and the CBOR the
/// field carries.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Disclosure {
    pub label: &'static str,
    pub salt: [u8; 16],
    pub value: Vec<u8>,
}

impl Disclosure {
    /// The `Disclosure` array `[salt, label, value]`, as it is hashed and
    /// as it travels revealed.
    pub fn encode(&self) -> Vec<u8> {
        let mut out = Vec::new();
        emit_array_head(&mut out, 3);
        emit_bstr(&mut out, &self.salt);
        emit_tstr(&mut out, self.label);
        out.extend_from_slice(&self.value);
        out
    }

    /// `SHA-256(0x00 || the encoded Disclosure)`.
    pub fn digest(&self) -> [u8; 32] {
        let mut pre = vec![0u8];
        pre.extend_from_slice(&self.encode());
        sha256(&pre)
    }
}

/// The seven disclosures, one per label in label order.
pub type DisclosureSet = [Disclosure; 7];

/// `SHA-256(0x01 || the seven digests in label order)`: body field 8.
pub fn disclosure_root(set: &DisclosureSet) -> [u8; 32] {
    let mut pre = vec![1u8];
    for d in set {
        pre.extend_from_slice(&d.digest());
    }
    sha256(&pre)
}

/// A disclosure set over the seven values, salted fresh.
pub fn disclosures(values: [Vec<u8>; 7], salts: [[u8; 16]; 7]) -> DisclosureSet {
    let mut it = values.into_iter().zip(salts).zip(LABELS);
    std::array::from_fn(|_| {
        let ((value, salt), label) = it.next().expect("seven");
        Disclosure { label, salt, value }
    })
}

/// A proximity value (`wire-format.md` §4.5): the channels tried, each
/// (kind, result, resolution in metres), and the strongest that passed.
pub fn proximity_value(channels: &[(u64, u64, Option<u64>)], strongest: u64) -> Vec<u8> {
    let mut out = Vec::new();
    emit_map_head(&mut out, 2);
    emit_uint(&mut out, 1);
    emit_array_head(&mut out, channels.len());
    for (kind, result, resolution) in channels {
        emit_map_head(&mut out, 2 + resolution.is_some() as usize);
        emit_uint(&mut out, 1);
        emit_uint(&mut out, *kind);
        emit_uint(&mut out, 2);
        emit_uint(&mut out, *result);
        if let Some(r) = resolution {
            emit_uint(&mut out, 3);
            emit_uint(&mut out, *r);
        }
    }
    emit_uint(&mut out, 2);
    emit_uint(&mut out, strongest);
    out
}

/// A capture value (`wire-format.md` §4.5): modality, image count, liveness
/// and its version.
pub fn capture_value(
    modality: u64,
    image_count: u64,
    liveness: u64,
    liveness_version: u64,
) -> Vec<u8> {
    let mut out = Vec::new();
    emit_map_head(&mut out, 4);
    for (k, v) in [
        (1u64, modality),
        (2, image_count),
        (3, liveness),
        (4, liveness_version),
    ] {
        emit_uint(&mut out, k);
        emit_uint(&mut out, v);
    }
    out
}

/// A location value with no assertions and no corroborations: the empty
/// case a record may carry (`wire-format.md` §4.5).
pub fn empty_location_value() -> Vec<u8> {
    let mut out = Vec::new();
    emit_map_head(&mut out, 2);
    emit_uint(&mut out, 1);
    emit_array_head(&mut out, 0);
    emit_uint(&mut out, 2);
    emit_array_head(&mut out, 0);
    out
}

/// A retention value: years (`wire-format.md` §4.5.1.1).
pub fn retention_value(years: u64) -> Vec<u8> {
    let mut out = Vec::new();
    emit_uint(&mut out, years);
    out
}

/// A client-integrity value: attested, scheme, no evidence.
pub fn integrity_value(attested: bool, scheme: u64) -> Vec<u8> {
    let mut out = Vec::new();
    emit_map_head(&mut out, 2);
    emit_uint(&mut out, 1);
    emit_bool(&mut out, attested);
    emit_uint(&mut out, 2);
    emit_uint(&mut out, scheme);
    out
}

/// What both participants and every witness sign: a normal presence
/// record's body, its responses those gathered, its root over the agreed
/// disclosure set.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Proposal {
    pub started_at: u64,
    pub finalized_at: u64,
    pub participants: [Keyhash; 2],
    pub witnesses: Vec<Witness>,
    /// Signed `VerifierResponse`s, in the order the body carries them.
    pub responses: Vec<Vec<u8>>,
    pub root: [u8; 32],
}

/// Sort responses as the body requires (`wire-format.md` §4.5 field 5):
/// by verifier keyhash, ties by subject keyhash.
pub fn sort_responses(responses: &mut [Vec<u8>]) {
    responses.sort_by_key(|r| {
        Response::read(r)
            .map(|x| (x.verifier, x.subject))
            .unwrap_or(([0; 32], [0; 32]))
    });
}

impl Proposal {
    /// The signers in envelope order: participants, then witnesses.
    pub fn signers(&self) -> Vec<Keyhash> {
        let mut s = self.participants.to_vec();
        s.extend(self.witnesses.iter().map(|w| w.keyhash));
        s
    }

    /// The body, with one back-pointer list per signer in signer order.
    pub fn body(&self, back: &[Vec<Txid>]) -> Vec<u8> {
        assert_eq!(
            back.len(),
            self.signers().len(),
            "one back-pointer list per signer"
        );
        let mut out = Vec::new();
        emit_map_head(&mut out, 7 + (!self.responses.is_empty()) as usize);
        emit_back_pointers(&mut out, back);
        emit_uint(&mut out, 1);
        emit_uint(&mut out, self.started_at);
        emit_uint(&mut out, 2);
        emit_uint(&mut out, self.finalized_at);
        emit_uint(&mut out, 3);
        emit_array_head(&mut out, 2);
        for p in &self.participants {
            emit_map_head(&mut out, 1);
            emit_uint(&mut out, 1);
            emit_bstr(&mut out, p);
        }
        emit_uint(&mut out, 4);
        emit_array_head(&mut out, self.witnesses.len());
        for w in &self.witnesses {
            emit_map_head(&mut out, 3);
            emit_uint(&mut out, 1);
            emit_bstr(&mut out, &w.keyhash);
            emit_uint(&mut out, 2);
            emit_bstr(&mut out, &w.nominated_by);
            emit_uint(&mut out, 3);
            emit_uint(&mut out, w.flags);
        }
        if !self.responses.is_empty() {
            emit_uint(&mut out, 5);
            emit_array_head(&mut out, self.responses.len());
            for r in &self.responses {
                out.extend_from_slice(r);
            }
        }
        emit_uint(&mut out, 6);
        emit_uint(&mut out, 0);
        emit_uint(&mut out, 8);
        emit_bstr(&mut out, &self.root);
        out
    }
}

/// Why a party will not sign a proposed body.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Refusal {
    /// A witness attributed to this party that it did not nominate
    /// (`light-client-requirements.md` §1.1).
    NomineeNotMine(Keyhash),
    /// A response this subject holds that the body omits
    /// (`light-client-requirements.md` §1.4).
    OmittedResponse([u8; 32]),
    /// A claimed start far from the witness's own clock
    /// (`light-client-requirements.md` §1.2).
    ClockFar { claimed: u64, observed: u64 },
}

/// What a participant checks before signing: every witness attributed to
/// it is one it nominated; every response it holds about itself is in the
/// body; and the person is told when its own nominees are absent or
/// outnumbered, which is no refusal.
pub fn participant_check(
    me: &Keyhash,
    p: &Proposal,
    my_nominees: &BTreeSet<Keyhash>,
    held: &BTreeMap<[u8; 32], Vec<u8>>,
    notifier: &dyn Notifier,
) -> Result<(), Refusal> {
    for w in &p.witnesses {
        if w.nominated_by == *me && !my_nominees.contains(&w.keyhash) {
            return Err(Refusal::NomineeNotMine(w.keyhash));
        }
    }
    let mine = p.witnesses.iter().filter(|w| w.nominated_by == *me).count();
    let theirs = p.witnesses.len() - mine;
    if mine == 0 || mine < theirs {
        notifier.notify(Notice::NomineesOutnumbered { mine, theirs });
    }
    for (qid, bytes) in held {
        if !p.responses.iter().any(|r| r == bytes) {
            return Err(Refusal::OmittedResponse(*qid));
        }
    }
    Ok(())
}

/// What a witness checks before signing: the claimed start is within its
/// tolerance of the time it observes.
pub fn witness_check(started_at: u64, observed: u64, tolerance: u64) -> Result<(), Refusal> {
    if started_at.abs_diff(observed) > tolerance {
        return Err(Refusal::ClockFar {
            claimed: started_at,
            observed,
        });
    }
    Ok(())
}

/// A presentation (`wire-format.md` §4.5.1.3): the envelope and exactly
/// seven slots in label order, each the disclosure revealed or its digest
/// withheld.  Nothing is revealed unless named.
pub fn present(envelope: &[u8], set: &DisclosureSet, reveal: &[&str]) -> Vec<u8> {
    let mut out = Vec::new();
    emit_array_head(&mut out, 2);
    out.extend_from_slice(envelope);
    emit_array_head(&mut out, 7);
    for d in set {
        if reveal.contains(&d.label) {
            out.extend_from_slice(&d.encode());
        } else {
            emit_bstr(&mut out, &d.digest());
        }
    }
    out
}

/// Keep a late response beside the record it supplements
/// (`wire-format.md` §7.4): the record must be held, the response must
/// verify, name one of the record's participants, and answer a query the
/// subject countersigned **for that ceremony**.  Nothing in the record
/// changes.
///
/// `consented` is the subject's consent by ceremony, not one flattened
/// set: a query id consented in another encounter authorises nothing
/// here, or a correctly signed response from one ceremony attaches to a
/// record from another.
///
/// Where the record's ceremony cannot be resolved — the holder keeps the
/// record but not the seed that names its ceremony — the response is
/// refused and its arrival recorded: **a late response is a sign of the
/// responder's reliability whatever became of it** [author, 2026-09-12].
/// Only a response that verified and named a participant is recorded;
/// anyone can manufacture one that does not, and a forgery is no signal.
pub fn take_late_response<L: Lookup + ?Sized>(
    store: &mut ClientStore,
    ids: &L,
    bytes: &[u8],
    consented: &BTreeMap<[u8; 32], BTreeSet<[u8; 32]>>,
) -> Result<Txid, String> {
    let late = LateResponse::decode(bytes)?;
    let Some(record) = store.records.get(&late.record) else {
        return Err("names a record not held".into());
    };
    let rec = Record::parse(record)?;
    if !rec.participants().contains(&late.subject) {
        return Err("subject is not a participant of the record".into());
    }
    let r = Response::read(&late.response)?;
    if r.subject != late.subject {
        return Err("response subject differs".into());
    }
    rhtn_crypto::verify::response(ids, &late.response, false).map_err(|e| e.to_string())?;
    // the ceremony is the record's, and consent is read within it
    let Some(seed) = store.seeds.get(&late.record) else {
        store
            .unattached_late
            .entry(late.record)
            .or_default()
            .push(r.verifier);
        return Err("the record's ceremony is not resolvable here".into());
    };
    let ceremony = seed.ceremony_id;
    if !consented
        .get(&ceremony)
        .is_some_and(|qs| qs.contains(&r.query_id))
    {
        return Err("a query the subject did not countersign for this record's ceremony".into());
    }
    store
        .late
        .entry(late.record)
        .or_default()
        .push(bytes.to_vec());
    Ok(late.record)
}

/// A presentation as a recipient reads it (`wire-format.md` §4.5.1.3,
/// §4.5.1.5): the record its envelope carries, verified; the fields
/// revealed, by label; the labels withheld, which are unknown and never a
/// default.
#[derive(Debug, Clone)]
pub struct Presented {
    pub record: Record,
    pub revealed: BTreeMap<&'static str, Vec<u8>>,
    pub withheld: Vec<&'static str>,
}

fn is_uint(it: &Item) -> bool {
    matches!(it, Item::Uint(_))
}

/// The value schema for one label (`wire-format.md` §4.5.1.1's table), and
/// the `strongest`-channel rule where `proximity` is revealed (§3.2).
fn check_value(label: &str, b: &[u8], item: &Item) -> Result<(), String> {
    match label {
        "capture" => {
            let Item::Map(m) = item else {
                return Err("capture not a map".into());
            };
            if m.len() != 4 || !(1..=4).all(|k| map_get(m, k).is_some_and(is_uint)) {
                return Err("capture shape".into());
            }
            let count = as_uint(map_get(m, 2).unwrap()).unwrap();
            if !(3..=5).contains(&count) {
                return Err("image count".into());
            }
            Ok(())
        }
        "location" => schema::check_kind(b, "LocationEvidence", item).map_err(|e| e.0.to_string()),
        "proximity" => {
            schema::check_kind(b, "Proximity", item).map_err(|e| e.0.to_string())?;
            let Item::Map(m) = item else { unreachable!() };
            let strongest = map_get(m, 2).and_then(as_uint).ok_or("strongest")?;
            let Some(Item::Array(ch)) = map_get(m, 1) else {
                unreachable!()
            };
            let mut passed = Vec::new();
            for c in ch {
                let Item::Map(cm) = c else {
                    return Err("channel not a map".into());
                };
                let kind = map_get(cm, 1).and_then(as_uint).ok_or("channel kind")?;
                let result = map_get(cm, 2).and_then(as_uint).ok_or("channel result")?;
                if !(1..=4).contains(&kind) || result > 2 {
                    return Err("channel out of range".into());
                }
                if result == 0 {
                    passed.push(kind);
                }
            }
            // ranking is UWB 1 > NFC 2 > optical 3 > latency 4
            if !passed.contains(&strongest) || passed.iter().any(|k| *k < strongest) {
                return Err("strongest is not the highest-ranked channel that passed".into());
            }
            Ok(())
        }
        "p0.retention" | "p1.retention" => {
            if is_uint(item) {
                Ok(())
            } else {
                Err("retention not uint".into())
            }
        }
        "p0.integrity" | "p1.integrity" => {
            let Item::Map(m) = item else {
                return Err("integrity not a map".into());
            };
            if !matches!(map_get(m, 1), Some(Item::Bool(_))) || !map_get(m, 2).is_some_and(is_uint)
            {
                return Err("integrity shape".into());
            }
            match map_get(m, 3) {
                None | Some(Item::Bytes(_)) if m.len() == 2 + map_get(m, 3).is_some() as usize => {
                    Ok(())
                }
                _ => Err("integrity evidence".into()),
            }
        }
        _ => Err("label outside the set".into()),
    }
}

/// Read a presentation: the envelope verified under the keys held, exactly
/// seven slots, each withheld digest taken as given and each revealed
/// disclosure hashed after its salt, label position and value schema are
/// checked, and the root recomputed against body field 8.  A mismatch is
/// malformed.
pub fn read_presentation<L: Lookup + ?Sized>(ids: &L, bytes: &[u8]) -> Result<Presented, String> {
    parse_all(bytes).map_err(|e| e.0.to_string())?;
    let outer = array_item_ranges(bytes, 0).ok_or("presentation not an array")?;
    if outer.len() != 2 {
        return Err("presentation is envelope plus slots".into());
    }
    let env = &bytes[outer[0].clone()];
    let record = Record::parse(env)?;
    rhtn_crypto::verify::envelope(ids, env).map_err(|e| e.to_string())?;
    let slots = array_item_ranges(bytes, outer[1].start).ok_or("slots not an array")?;
    if slots.len() != 7 {
        return Err("exactly seven slots".into());
    }
    let mut pre = vec![1u8];
    let mut out = Presented {
        record,
        revealed: BTreeMap::new(),
        withheld: Vec::new(),
    };
    for (i, slot) in slots.iter().enumerate() {
        let (item, _) = Parser { b: bytes }
            .item(slot.start)
            .map_err(|e| e.0.to_string())?;
        match &item {
            Item::Bytes(r) if r.len() == 32 => {
                pre.extend_from_slice(&bytes[r.clone()]);
                out.withheld.push(LABELS[i]);
            }
            Item::Array(a) if a.len() == 3 => {
                let (Item::Bytes(salt), Item::Text(label)) = (&a[0], &a[1]) else {
                    return Err("disclosure shape".into());
                };
                if salt.len() != 16 {
                    return Err("salt not 16 bytes".into());
                }
                if &bytes[label.clone()] != LABELS[i].as_bytes() {
                    return Err("revealed label differs from its slot".into());
                }
                let parts = array_item_ranges(bytes, slot.start).ok_or("disclosure parts")?;
                let value = &bytes[parts[2].clone()];
                let vitem = parse_all(value).map_err(|e| e.0.to_string())?;
                check_value(LABELS[i], value, &vitem)?;
                let mut d = vec![0u8];
                d.extend_from_slice(&bytes[slot.clone()]);
                pre.extend_from_slice(&sha256(&d));
                out.revealed.insert(LABELS[i], value.to_vec());
            }
            _ => return Err("slot is neither a digest nor a disclosure".into()),
        }
    }
    let body = &out.record.bytes[out.record.body.clone()];
    let r8 = value_slice(body, 8).ok_or("field 8")?;
    let (root, _) = Parser { b: body }
        .item(r8.start)
        .map_err(|e| e.0.to_string())?;
    let Item::Bytes(rr) = &root else {
        return Err("root not bytes".into());
    };
    if body[rr.clone()] != sha256(&pre) {
        return Err("recomputed root differs from body field 8".into());
    }
    Ok(out)
}
