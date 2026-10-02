//! The ceremony's conversation on the end-to-end channel (`wire-format.md`
//! §7.10.1 kinds 9 to 18, §7.10.2): what the two participants and their
//! witnesses send one another after the local exchanges (§14.3), as bytes.
//!
//! **These live beside [`crate::local`] for the same reason it does.** No
//! shell parses wire bytes: the boundary above this crate hands over opaque
//! byte strings under a kind tag, and every check a recipient makes is a
//! check against the ceremony's own state, which only this side holds.
//! Each structure that reuses an object defined elsewhere carries it
//! unchanged, as the section requires: a response, an archive entry, a
//! signer's entries and the body itself travel as the bytes they were
//! produced as, never re-encoded, because each is signed over its bytes.
//!
//! The in-process [`crate::ceremony::Msg`] is the same conversation as
//! values; [`crate::ceremony::conversation_payload`] and
//! [`crate::ceremony::conversation_from`] are the two directions between
//! the two.  A `FishingProposal` has a decoder here and no message there
//! yet, which that side says.

use crate::ceremony::WitnessRequest;
use crate::local::{bs, emit_channels, fixed32, read_channels};
use crate::record::{Disclosure, DisclosureSet, LABELS, Proposal, Refusal};
use crate::{Keyhash, Txid};
use rhtn_archive::tx::Witness;
use rhtn_codec::cbor::*;
use rhtn_codec::encode::*;
use rhtn_codec::schema;

/// The subject's answer to a consent request (§7.10.2, kind 10): the
/// query id and the classical `COSE_Sign1` over it (§5.6), the object the
/// verifier's response carries onward in its field 7.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConsentReply {
    pub query_id: [u8; 32],
    pub consent: Vec<u8>,
}

impl ConsentReply {
    pub fn encode(&self) -> Vec<u8> {
        let mut out = Vec::new();
        emit_array_head(&mut out, 2);
        emit_bstr(&mut out, &self.query_id);
        out.extend_from_slice(&self.consent);
        out
    }

    pub fn decode(b: &[u8]) -> Result<Self, String> {
        let it = checked(b, "ConsentReply")?;
        let Item::Array(a) = &it else {
            return Err("ConsentReply is not an array".into());
        };
        Ok(ConsentReply {
            query_id: fixed32(b, &a[0])?,
            consent: raw_items(b, 0)?.remove(1),
        })
    }
}

/// A bundle augmentation (§7.10.2, kind 11; design §8.1.2): one to 256
/// archive entries, each an envelope map or a presentation array (§7.9),
/// carried as the bytes they arrived as.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FishingProposal {
    pub entries: Vec<Vec<u8>>,
}

impl FishingProposal {
    pub fn encode(&self) -> Vec<u8> {
        let mut out = Vec::new();
        emit_array_head(&mut out, self.entries.len());
        for e in &self.entries {
            out.extend_from_slice(e);
        }
        out
    }

    pub fn decode(b: &[u8]) -> Result<Self, String> {
        checked(b, "FishingProposal")?;
        Ok(FishingProposal {
            entries: raw_items(b, 0)?,
        })
    }
}

impl WitnessRequest {
    /// The map a nominee is sent (§7.10.2, kind 12): the ceremony-id, the
    /// participants in body order, the claimed start, and the channels as
    /// measured.
    pub fn encode(&self) -> Vec<u8> {
        let mut out = Vec::new();
        emit_map_head(&mut out, 4);
        emit_uint(&mut out, 1);
        emit_bstr(&mut out, &self.ceremony_id);
        emit_uint(&mut out, 2);
        emit_array_head(&mut out, 2);
        for p in &self.participants {
            emit_bstr(&mut out, p);
        }
        emit_uint(&mut out, 3);
        emit_uint(&mut out, self.started_at);
        emit_uint(&mut out, 4);
        emit_channels(&mut out, &self.channels);
        out
    }

    pub fn decode(b: &[u8]) -> Result<Self, String> {
        let it = checked(b, "WitnessRequest")?;
        let Item::Map(m) = &it else {
            return Err("WitnessRequest is not a map".into());
        };
        let Some(Item::Array(parts)) = map_get(m, 2) else {
            return Err("participants not an array".into());
        };
        let participants: [Keyhash; 2] = [fixed32(b, &parts[0])?, fixed32(b, &parts[1])?];
        Ok(WitnessRequest {
            ceremony_id: fixed32(b, map_get(m, 1).ok_or("ceremony-id")?)?,
            participants,
            started_at: map_get(m, 3).and_then(as_uint).ok_or("started_at")?,
            channels: read_channels(map_get(m, 4).ok_or("channels")?)?,
        })
    }
}

/// The nominee's answer (§7.10.2, kind 13): witnessing with the attestation
/// bits it will set (Witness field 3, §3.2), or declining with nothing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WitnessAnswer {
    pub bits: Option<u64>,
}

impl WitnessAnswer {
    pub fn encode(&self) -> Vec<u8> {
        let mut out = Vec::new();
        emit_map_head(&mut out, 1 + usize::from(self.bits.is_some()));
        emit_uint(&mut out, 1);
        emit_bool(&mut out, self.bits.is_some());
        if let Some(bits) = self.bits {
            emit_uint(&mut out, 2);
            emit_uint(&mut out, bits);
        }
        out
    }

    pub fn decode(b: &[u8]) -> Result<Self, String> {
        let it = checked(b, "WitnessAnswer")?;
        let Item::Map(m) = &it else {
            return Err("WitnessAnswer is not a map".into());
        };
        // the checker has already tied the bits to the bool
        Ok(WitnessAnswer {
            bits: map_get(m, 2).and_then(as_uint),
        })
    }
}

/// What the participant that did not propose hands the proposer (§7.10.2,
/// kind 14): the signed responses it gathered, in the order the body sorts
/// them, each as the bytes its verifier signed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GatheredResponses {
    pub responses: Vec<Vec<u8>>,
}

impl GatheredResponses {
    pub fn encode(&self) -> Vec<u8> {
        let mut out = Vec::new();
        emit_array_head(&mut out, self.responses.len());
        for r in &self.responses {
            out.extend_from_slice(r);
        }
        out
    }

    pub fn decode(b: &[u8]) -> Result<Self, String> {
        checked(b, "GatheredResponses")?;
        let responses = raw_items(b, 0)?;
        // each is a `VerifierResponse` (§4.5), and is checked as one here
        // rather than at the body, where a bad one would refuse a record
        // every other signer had already reviewed
        for r in &responses {
            let item = parse_all(r).map_err(|e| e.0)?;
            schema::check_kind(r, "VerifierResponse", &item).map_err(|e| e.0)?;
        }
        Ok(GatheredResponses { responses })
    }
}

/// One signer's back-pointers for the body (§7.10.2, kind 15; §3.1): one
/// to eight txids.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BackPointers {
    pub txids: Vec<Txid>,
}

impl BackPointers {
    pub fn encode(&self) -> Vec<u8> {
        let mut out = Vec::new();
        emit_array_head(&mut out, self.txids.len());
        for t in &self.txids {
            emit_bstr(&mut out, t);
        }
        out
    }

    pub fn decode(b: &[u8]) -> Result<Self, String> {
        let it = checked(b, "BackPointers")?;
        let Item::Array(a) = &it else {
            return Err("BackPointers is not an array".into());
        };
        Ok(BackPointers {
            txids: a.iter().map(|t| fixed32(b, t)).collect::<Result<_, _>>()?,
        })
    }
}

/// What the proposer shows every signer (§7.10.2, kind 16): the body as
/// every signer will sign it, back-pointers in place, and the disclosure
/// set its root commits to with every slot revealed, so a signer recomputes
/// the root before signing (§4.5.1).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProposedBody {
    pub body: Vec<u8>,
    pub set: DisclosureSet,
}

impl ProposedBody {
    pub fn encode(&self) -> Vec<u8> {
        let mut out = Vec::new();
        emit_map_head(&mut out, 2);
        emit_uint(&mut out, 1);
        emit_bstr(&mut out, &self.body);
        emit_uint(&mut out, 2);
        emit_array_head(&mut out, self.set.len());
        for d in &self.set {
            out.extend_from_slice(&d.encode());
        }
        out
    }

    /// The slots come back in label order, which is the order the root is
    /// computed over (§4.5.1.1): a slot under another label, or out of
    /// order, is not the set the body's root commits to.
    pub fn decode(b: &[u8]) -> Result<Self, String> {
        let it = checked(b, "ProposedBody")?;
        let Item::Map(m) = &it else {
            return Err("ProposedBody is not a map".into());
        };
        let body = bs(b, map_get(m, 1).ok_or("body")?).ok_or("body")?;
        let set_at = value_slice(b, 2).ok_or("disclosures")?.start;
        let slots = array_item_ranges(b, set_at).ok_or("disclosures not an array")?;
        let mut set: Vec<Disclosure> = Vec::with_capacity(LABELS.len());
        for (r, label) in slots.into_iter().zip(LABELS) {
            let parts = array_item_ranges(b, r.start).ok_or("a disclosure is an array")?;
            let salt = parse_all(&b[parts[0].clone()]).map_err(|e| e.0)?;
            let salt = bs(&b[parts[0].clone()], &salt)
                .and_then(|s| <[u8; 16]>::try_from(s.as_slice()).ok())
                .ok_or("a salt is 16 bytes")?;
            let got = parse_all(&b[parts[1].clone()]).map_err(|e| e.0)?;
            let Item::Text(t) = &got else {
                return Err("a label is a text string".into());
            };
            if &b[parts[1].start + t.start..parts[1].start + t.end] != label.as_bytes() {
                return Err(format!("slot {label} carries another label"));
            }
            set.push(Disclosure {
                label,
                salt,
                value: b[parts[2].clone()].to_vec(),
            });
        }
        let set: DisclosureSet = set
            .try_into()
            .map_err(|_| String::from("exactly seven disclosures"))?;
        Ok(ProposedBody { body, set })
    }
}

/// A signer's answer to a proposed body (§7.10.2, kind 17): its two
/// envelope entries over the body (§3.5), classical then post-quantum, as
/// one byte string the way the signer produces them; or its refusal.
///
/// A refusal's particular is the witness keyhash or the query id; a clock
/// refusal carries none, so what the refusing party observed stays with
/// it and the particulars read back as zero.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SigningReply {
    pub reply: Result<Vec<u8>, Refusal>,
}

impl SigningReply {
    pub fn encode(&self) -> Vec<u8> {
        let mut out = Vec::new();
        match &self.reply {
            Ok(entries) => {
                emit_map_head(&mut out, 1);
                emit_uint(&mut out, 1);
                emit_array_head(&mut out, 2);
                out.extend_from_slice(entries);
            }
            Err(r) => {
                let (code, particular): (u64, Option<&[u8; 32]>) = match r {
                    Refusal::NomineeNotMine(k) => (1, Some(k)),
                    Refusal::OmittedResponse(q) => (2, Some(q)),
                    Refusal::ClockFar { .. } => (3, None),
                };
                emit_map_head(&mut out, 1 + usize::from(particular.is_some()));
                emit_uint(&mut out, 2);
                emit_uint(&mut out, code);
                if let Some(p) = particular {
                    emit_uint(&mut out, 3);
                    emit_bstr(&mut out, p);
                }
            }
        }
        out
    }

    pub fn decode(b: &[u8]) -> Result<Self, String> {
        let it = checked(b, "SigningReply")?;
        let Item::Map(m) = &it else {
            return Err("SigningReply is not a map".into());
        };
        if map_get(m, 1).is_some() {
            // the two entries, unwrapped from their array into the one
            // byte string the envelope builder takes
            let at = value_slice(b, 1).ok_or("entries")?.start;
            let items = array_item_ranges(b, at).ok_or("entries not an array")?;
            let entries = items.into_iter().flat_map(|r| b[r].to_vec()).collect();
            return Ok(SigningReply { reply: Ok(entries) });
        }
        let code = map_get(m, 2).and_then(as_uint).ok_or("refusal")?;
        let particular = map_get(m, 3).map(|p| fixed32(b, p)).transpose()?;
        let refusal = match (code, particular) {
            (1, Some(k)) => Refusal::NomineeNotMine(k),
            (2, Some(q)) => Refusal::OmittedResponse(q),
            (3, None) => Refusal::ClockFar {
                claimed: 0,
                observed: 0,
            },
            (1 | 2, None) => return Err("refusals 1 and 2 name their particular".into()),
            (3, Some(_)) => return Err("refusal 3 carries no particular".into()),
            _ => return Err("refusal out of range".into()),
        };
        Ok(SigningReply {
            reply: Err(refusal),
        })
    }
}

/// A proposed body back into the proposal and the back-pointer lists it
/// was built from, so a signer reviews it as the value the proposer held.
///
/// **The body must be one this client would have built**: what comes back
/// is re-emitted and compared to the bytes, so a body carrying anything
/// the proposal does not model is refused rather than signed with a part
/// of it unseen.
pub fn proposal_from_body(body: &[u8]) -> Result<(Proposal, Vec<Vec<Txid>>), String> {
    let it = parse_all(body).map_err(|e| e.0)?;
    // the body is checked as every archive will check it before anything
    // is read out of it, so this side is at least as strict as the schema
    // (the bargain `checked` below makes for every other kind)
    schema::check_body_of_type(body, &it, rhtn_archive::tx::TYPE_PRESENCE).map_err(|e| e.0)?;
    let Item::Map(m) = &it else {
        return Err("body is not a map".into());
    };
    let Some(Item::Array(lists)) = map_get(m, 0) else {
        return Err("back-pointers".into());
    };
    let mut back = Vec::with_capacity(lists.len());
    for l in lists {
        let Item::Array(hs) = l else {
            return Err("a back-pointer list is an array".into());
        };
        back.push(
            hs.iter()
                .map(|h| fixed32(body, h))
                .collect::<Result<Vec<Txid>, _>>()?,
        );
    }
    let Some(Item::Array(ps)) = map_get(m, 3) else {
        return Err("participants".into());
    };
    if ps.len() != 2 {
        return Err("two participants".into());
    }
    let participant = |p: &Item| -> Result<Keyhash, String> {
        let Item::Map(pm) = p else {
            return Err("a participant is a map".into());
        };
        fixed32(body, map_get(pm, 1).ok_or("participant keyhash")?)
    };
    let participants = [participant(&ps[0])?, participant(&ps[1])?];
    let Some(Item::Array(ws)) = map_get(m, 4) else {
        return Err("witnesses".into());
    };
    let mut witnesses = Vec::with_capacity(ws.len());
    for w in ws {
        let Item::Map(wm) = w else {
            return Err("a witness is a map".into());
        };
        witnesses.push(Witness {
            keyhash: fixed32(body, map_get(wm, 1).ok_or("witness keyhash")?)?,
            nominated_by: fixed32(body, map_get(wm, 2).ok_or("nominated_by")?)?,
            flags: map_get(wm, 3).and_then(as_uint).ok_or("witness flags")?,
        });
    }
    let responses = match value_slice(body, 5) {
        Some(r) => array_item_ranges(body, r.start)
            .ok_or("responses not an array")?
            .into_iter()
            .map(|r| body[r].to_vec())
            .collect(),
        None => Vec::new(),
    };
    let proposal = Proposal {
        started_at: map_get(m, 1).and_then(as_uint).ok_or("started_at")?,
        finalized_at: map_get(m, 2).and_then(as_uint).ok_or("finalized_at")?,
        participants,
        witnesses,
        responses,
        root: fixed32(body, map_get(m, 8).ok_or("root")?)?,
    };
    if back.len() != proposal.signers().len() {
        return Err("one back-pointer list per signer".into());
    }
    if proposal.body(&back) != body {
        return Err("the body carries what the proposal does not".into());
    }
    Ok((proposal, back))
}

// ---------------------------------------------------------------- helpers

/// Parse and put the bytes through the codec's own checker for that kind,
/// so what this module accepts is exactly what the schema accepts
/// (`crate::local` makes the same bargain, for the same reason).
fn checked(b: &[u8], kind: &'static str) -> Result<Item, String> {
    let item = parse_all(b).map_err(|e| e.0)?;
    schema::check_kind(b, kind, &item).map_err(|e| e.0)?;
    Ok(item)
}

/// The items of the array at `at`, each as the exact bytes it arrived as:
/// how a signed object inside a structure is carried without re-encoding.
fn raw_items(b: &[u8], at: usize) -> Result<Vec<Vec<u8>>, String> {
    let items = array_item_ranges(b, at).ok_or("not an array")?;
    Ok(items.into_iter().map(|r| b[r].to_vec()).collect())
}
