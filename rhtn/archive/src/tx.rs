//! Transaction construction (`wire-format.md` §3.1, §3.5, §4): bodies with
//! their chain back-pointers, and the envelope over them.  Everything here
//! emits deterministic CBOR through `rhtn-codec`'s encoder and signs with
//! `rhtn-crypto`; nothing here consults state, which is `chain`'s job.

use crate::{Keyhash, Txid};
use rhtn_codec::cose::aad;
use rhtn_codec::encode::*;
use rhtn_crypto::SigningIdentity;

pub const TYPE_ADOPTION: u64 = 1;
pub const TYPE_DEPARTURE: u64 = 2;
pub const TYPE_DISAVOWAL: u64 = 3;
pub const TYPE_PEERING: u64 = 4;
pub const TYPE_PRESENCE: u64 = 5;
pub const TYPE_REISSUE: u64 = 7;

/// `{series, counter}` (`wire-format.md` §2.3), encoded `[series, counter]`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Seqno {
    pub series: u32,
    pub counter: u32,
}

impl Seqno {
    pub fn emit(&self, out: &mut Vec<u8>) {
        emit_array_head(out, 2);
        emit_uint(out, self.series as u64);
        emit_uint(out, self.counter as u64);
    }
}

/// How two seqnos rank (`wire-format.md` §2.3): comparable only within one
/// series, strictly greater is newer, equal is the same slot, and across
/// series there is no order.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Order {
    Newer,
    Same,
    Older,
    Incomparable,
}

/// Rank `next` against `prev`.
pub fn compare(prev: Seqno, next: Seqno) -> Order {
    if prev.series != next.series {
        Order::Incomparable
    } else if next.counter > prev.counter {
        Order::Newer
    } else if next.counter == prev.counter {
        Order::Same
    } else {
        Order::Older
    }
}

/// A `Locator` (`wire-format.md` §2.3): anchor, nibble path, seqno.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Locator {
    pub anchor: Keyhash,
    /// Packed 4-bit hop indices, high nibble first.
    pub path: Vec<u8>,
    pub nibbles: u64,
    pub seqno: Seqno,
}

impl Locator {
    /// A root's self-anchored locator: empty path, the given seqno.
    pub fn root(anchor: Keyhash, seqno: Seqno) -> Self {
        Locator { anchor, path: Vec::new(), nibbles: 0, seqno }
    }
    pub fn emit(&self, out: &mut Vec<u8>) {
        emit_map_head(out, 3);
        emit_uint(out, 1);
        emit_bstr(out, &self.anchor);
        emit_uint(out, 2);
        emit_map_head(out, 2);
        emit_uint(out, 1);
        emit_bstr(out, &self.path);
        emit_uint(out, 2);
        emit_uint(out, self.nibbles);
        emit_uint(out, 3);
        self.seqno.emit(out);
    }
}

impl Locator {
    /// Decode a `Locator` map (`wire-format.md` §2.3).
    pub fn decode(b: &[u8]) -> Result<Locator, String> {
        use rhtn_codec::cbor::*;
        let item = parse_all(b).map_err(|e| e.0)?;
        let Item::Map(m) = &item else { return Err("locator not a map".into()) };
        let anchor: Keyhash = match map_get(m, 1) {
            Some(Item::Bytes(r)) if r.len() == 32 => <[u8; 32]>::try_from(&b[r.clone()]).map_err(|_| "anchor width")?,
            _ => return Err("locator field 1".into()),
        };
        let Some(Item::Map(pm)) = map_get(m, 2) else { return Err("locator field 2".into()) };
        let path = match map_get(pm, 1) {
            Some(Item::Bytes(r)) => b[r.clone()].to_vec(),
            _ => return Err("path field 1".into()),
        };
        let nibbles = map_get(pm, 2).and_then(as_uint).ok_or("path field 2")?;
        // the packed-path invariant (`wire-format.md` §2.1), at this
        // decoding boundary as at the schema's
        rhtn_codec::schema::packed_path(&path, nibbles).map_err(|e| e.0)?;
        let Some(Item::Array(sq)) = map_get(m, 3) else { return Err("locator field 3".into()) };
        let seqno = Seqno {
            series: as_uint(sq.first().ok_or("series")?).ok_or("series")? as u32,
            counter: as_uint(sq.get(1).ok_or("counter")?).ok_or("counter")? as u32,
        };
        Ok(Locator { anchor, path, nibbles, seqno })
    }
}

/// Key 0: one list per required signer, in signer order; a list longer than
/// one is a merge and is sorted ascending bytewise (`wire-format.md` §3.1).
#[allow(clippy::needless_range_loop)]
pub fn emit_back_pointers(out: &mut Vec<u8>, lists: &[Vec<Txid>]) {
    emit_uint(out, 0);
    emit_array_head(out, lists.len());
    for l in lists {
        let mut l = l.clone();
        l.sort();
        emit_array_head(out, l.len());
        for h in &l {
            emit_bstr(out, h);
        }
    }
}

/// The evidence an adoption carries, exactly one of three (design §6.1.1).
#[derive(Debug, Clone)]
pub enum Evidence {
    /// Field 8: a presence record between the two parties, by txid.
    Presence(Txid),
    /// Field 9: the former patron's transfer statement.
    Transfer { former: Keyhash, block: Vec<u8> },
    /// Field 6: an encoded `Recovery` map.
    Recovery(Vec<u8>),
}

/// An adoption body (`wire-format.md` §4.1).
pub struct Adoption<'a> {
    pub node: Keyhash,
    pub patron: Keyhash,
    pub locator: Locator,
    pub timestamp: u64,
    pub key_material: Option<Vec<u8>>,
    pub evidence: Evidence,
    /// Field 7: the head of the archive prefix presented, if any.
    pub presented_head: Option<Txid>,
    /// Back-pointers: node's list, then patron's list.
    pub back: [&'a [Txid]; 2],
}

pub fn adoption_body(a: &Adoption) -> Vec<u8> {
    let mut out = Vec::new();
    let n = 5 + a.key_material.is_some() as usize + a.presented_head.is_some() as usize + 1;
    emit_map_head(&mut out, n);
    emit_back_pointers(&mut out, &[a.back[0].to_vec(), a.back[1].to_vec()]);
    emit_uint(&mut out, 1);
    emit_bstr(&mut out, &a.node);
    emit_uint(&mut out, 2);
    emit_bstr(&mut out, &a.patron);
    emit_uint(&mut out, 3);
    a.locator.emit(&mut out);
    emit_uint(&mut out, 4);
    emit_uint(&mut out, a.timestamp);
    if let Some(km) = &a.key_material {
        emit_uint(&mut out, 5);
        out.extend_from_slice(km);
    }
    if let Evidence::Recovery(block) = &a.evidence {
        emit_uint(&mut out, 6);
        out.extend_from_slice(block);
    }
    if let Some(h) = &a.presented_head {
        emit_uint(&mut out, 7);
        emit_bstr(&mut out, h);
    }
    match &a.evidence {
        Evidence::Presence(t) => {
            emit_uint(&mut out, 8);
            emit_bstr(&mut out, t);
        }
        Evidence::Transfer { former, block } => {
            emit_uint(&mut out, 9);
            emit_map_head(&mut out, 2);
            emit_uint(&mut out, 1);
            emit_bstr(&mut out, former);
            emit_uint(&mut out, 2);
            out.extend_from_slice(block);
        }
        Evidence::Recovery(_) => {}
    }
    out
}

/// The former patron's `TransferStatement` block (`wire-format.md` §4.1):
/// a `COSE_Sign` by the former patron over `[node, former, new_patron]`
/// with `external_aad = "rhtn/1:transfer"`.
pub fn transfer_block(former: &SigningIdentity, node: &Keyhash, new_patron: &Keyhash) -> Vec<u8> {
    let mut stmt = Vec::new();
    emit_array_head(&mut stmt, 3);
    emit_bstr(&mut stmt, node);
    emit_bstr(&mut stmt, &former.public.keyhash);
    emit_bstr(&mut stmt, new_patron);
    sign_block(former, aad::TRANSFER, &stmt)
}

/// A `COSE_Sign` by one hybrid signer: empty protected header, no
/// unprotected entries, nil payload, two entries (`wire-format.md` §3.5).
pub fn sign_block(signer: &SigningIdentity, tag: &[u8], payload: &[u8]) -> Vec<u8> {
    let mut s = Vec::new();
    emit_array_head(&mut s, 4);
    emit_bstr(&mut s, b"");
    emit_map_head(&mut s, 0);
    emit_null(&mut s);
    emit_array_head(&mut s, 2);
    s.extend_from_slice(&signer.sign_entries_unnamed(tag, payload));
    s
}

/// A verifier response (`wire-format.md` §4.5) as a recovery carries it:
/// the subject's consent over the query id, `result = 0` (match), and the
/// verifier's hybrid signature over the map minus field 9; field 8 names the
/// prior key whose holder the verifier recognised.
pub fn recovery_response(verifier: &SigningIdentity, subject: &SigningIdentity, qid: &[u8; 32], prior: &Keyhash) -> Vec<u8> {
    let consent = subject.sign1_ed_unnamed(aad::CONSENT, qid);
    recovery_response_with_consent(verifier, &subject.public.keyhash, qid, &consent, prior)
}

/// The same response with the subject's consent supplied, as a verifier
/// issues it: the subject's client signed the consent, the verifier signs
/// the rest.
pub fn recovery_response_with_consent(verifier: &SigningIdentity, subject: &Keyhash, qid: &[u8; 32], consent: &[u8], prior: &Keyhash) -> Vec<u8> {
    let mut payload = Vec::new();
    emit_map_head(&mut payload, 8);
    emit_uint(&mut payload, 1);
    emit_bstr(&mut payload, &verifier.public.keyhash);
    emit_uint(&mut payload, 2);
    emit_bstr(&mut payload, subject);
    emit_uint(&mut payload, 3);
    emit_bstr(&mut payload, qid);
    emit_uint(&mut payload, 4);
    emit_uint(&mut payload, 0);
    emit_uint(&mut payload, 5);
    emit_uint(&mut payload, 1);
    emit_uint(&mut payload, 7);
    payload.extend_from_slice(consent);
    emit_uint(&mut payload, 8);
    emit_bstr(&mut payload, prior);
    emit_uint(&mut payload, 10);
    emit_uint(&mut payload, 0);
    let sig9 = sign_block(verifier, aad::VERIFIER, &payload);
    // splice field 9 in before field 10
    let r10 = rhtn_codec::cbor::value_slice(&payload, 10).unwrap();
    let key10_at = r10.start - 1;
    let (_, _, adv) = rhtn_codec::cbor::Parser { b: &payload }.head(0).unwrap();
    let mut out = Vec::new();
    emit_map_head(&mut out, 9);
    out.extend_from_slice(&payload[adv..key10_at]);
    emit_uint(&mut out, 9);
    out.extend_from_slice(&sig9);
    out.extend_from_slice(&payload[key10_at..]);
    out
}

/// A `Recovery` block (`wire-format.md` §4.1): the prior key, the verifier
/// responses sorted by verifier keyhash, and the old key's successor
/// statement over `[prior, new, patron]`.
pub fn recovery_block(old: &SigningIdentity, new_key: &Keyhash, patron: &Keyhash, mut responses: Vec<Vec<u8>>) -> Vec<u8> {
    responses.sort_by_key(|r| rhtn_codec::cbor::value_slice(r, 1).map(|s| r[s.clone()].to_vec()));
    let mut stmt = Vec::new();
    emit_array_head(&mut stmt, 3);
    emit_bstr(&mut stmt, &old.public.keyhash);
    emit_bstr(&mut stmt, new_key);
    emit_bstr(&mut stmt, patron);
    let proof = sign_block(old, aad::SUCCESSOR, &stmt);
    let mut out = Vec::new();
    emit_map_head(&mut out, 3);
    emit_uint(&mut out, 1);
    emit_bstr(&mut out, &old.public.keyhash);
    emit_uint(&mut out, 2);
    emit_array_head(&mut out, responses.len());
    for r in &responses {
        out.extend_from_slice(r);
    }
    emit_uint(&mut out, 3);
    out.extend_from_slice(&proof);
    out
}

/// A departure body (`wire-format.md` §4.2): the node leaving `patron` at
/// `seqno`, optionally with a reason code.
pub fn departure_body(back: &[Txid], node: &Keyhash, patron: &Keyhash, seqno: Seqno, timestamp: u64, reason: Option<u64>) -> Vec<u8> {
    let mut out = Vec::new();
    emit_map_head(&mut out, 5 + reason.is_some() as usize);
    emit_back_pointers(&mut out, &[back.to_vec()]);
    emit_uint(&mut out, 1);
    emit_bstr(&mut out, node);
    emit_uint(&mut out, 2);
    emit_bstr(&mut out, patron);
    emit_uint(&mut out, 3);
    seqno.emit(&mut out);
    emit_uint(&mut out, 4);
    emit_uint(&mut out, timestamp);
    if let Some(r) = reason {
        emit_uint(&mut out, 5);
        emit_uint(&mut out, r);
    }
    out
}

/// A disavowal body (`wire-format.md` §4.3): `patron` ending `node`'s slot,
/// with the reason code in the 64-value banded space.
pub fn disavowal_body(back: &[Txid], patron: &Keyhash, node: &Keyhash, timestamp: u64, code: Option<u64>) -> Vec<u8> {
    let mut out = Vec::new();
    emit_map_head(&mut out, 4 + code.is_some() as usize);
    emit_back_pointers(&mut out, &[back.to_vec()]);
    emit_uint(&mut out, 1);
    emit_bstr(&mut out, patron);
    emit_uint(&mut out, 2);
    emit_bstr(&mut out, node);
    emit_uint(&mut out, 3);
    emit_uint(&mut out, timestamp);
    if let Some(c) = code {
        emit_uint(&mut out, 4);
        emit_uint(&mut out, c);
    }
    out
}

/// A series reissue body (`wire-format.md` §4.6): the series left at the
/// counter it reached, and the new series at counter 0.
pub fn reissue_body(back: [&[Txid]; 2], node: &Keyhash, patron: &Keyhash, leaving: Seqno, new_series: u32, timestamp: u64) -> Vec<u8> {
    let mut out = Vec::new();
    emit_map_head(&mut out, 6);
    emit_back_pointers(&mut out, &[back[0].to_vec(), back[1].to_vec()]);
    emit_uint(&mut out, 1);
    emit_bstr(&mut out, node);
    emit_uint(&mut out, 2);
    emit_bstr(&mut out, patron);
    emit_uint(&mut out, 3);
    leaving.emit(&mut out);
    emit_uint(&mut out, 4);
    Seqno { series: new_series, counter: 0 }.emit(&mut out);
    emit_uint(&mut out, 5);
    emit_uint(&mut out, timestamp);
    out
}

/// A witness on a normal presence record (`wire-format.md` §4.5): its
/// keyhash, the participant who nominated it, and its attestation bits
/// (bit 0 protocol_ran, bit 1 both_responsive, bit 2 latency_bound).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Witness {
    pub keyhash: Keyhash,
    pub nominated_by: Keyhash,
    pub flags: u64,
}

/// A normal presence record body (`wire-format.md` §4.5): subtype 0, the
/// two participants, the witnesses in field 4, no responses, and the
/// disclosure root.  Key 0 carries one list per signer, the participants
/// first and then the witnesses in field 4's order.
pub fn presence_record_body(back: &[Vec<Txid>], participants: [&Keyhash; 2], witnesses: &[Witness], started_at: u64, finalized_at: u64, root: &[u8; 32]) -> Vec<u8> {
    let mut out = Vec::new();
    emit_map_head(&mut out, 7);
    emit_back_pointers(&mut out, back);
    emit_uint(&mut out, 1);
    emit_uint(&mut out, started_at);
    emit_uint(&mut out, 2);
    emit_uint(&mut out, finalized_at);
    emit_uint(&mut out, 3);
    emit_array_head(&mut out, 2);
    for p in participants {
        emit_map_head(&mut out, 1);
        emit_uint(&mut out, 1);
        emit_bstr(&mut out, p);
    }
    emit_uint(&mut out, 4);
    emit_array_head(&mut out, witnesses.len());
    for w in witnesses {
        emit_map_head(&mut out, 3);
        emit_uint(&mut out, 1);
        emit_bstr(&mut out, &w.keyhash);
        emit_uint(&mut out, 2);
        emit_bstr(&mut out, &w.nominated_by);
        emit_uint(&mut out, 3);
        emit_uint(&mut out, w.flags);
    }
    emit_uint(&mut out, 6);
    emit_uint(&mut out, 0);
    emit_uint(&mut out, 8);
    emit_bstr(&mut out, root);
    out
}

/// A formation presence record body (`wire-format.md` §4.5, design §13.2):
/// subtype 1, no witnesses, no responses, the two participants in field 3's
/// order and the disclosure root the client computed.
pub fn formation_body(back: [&[Txid]; 2], participants: [&Keyhash; 2], started_at: u64, finalized_at: u64, root: &[u8; 32]) -> Vec<u8> {
    let mut out = Vec::new();
    emit_map_head(&mut out, 6);
    emit_back_pointers(&mut out, &[back[0].to_vec(), back[1].to_vec()]);
    emit_uint(&mut out, 1);
    emit_uint(&mut out, started_at);
    emit_uint(&mut out, 2);
    emit_uint(&mut out, finalized_at);
    emit_uint(&mut out, 3);
    emit_array_head(&mut out, 2);
    for p in participants {
        emit_map_head(&mut out, 1);
        emit_uint(&mut out, 1);
        emit_bstr(&mut out, p);
    }
    emit_uint(&mut out, 6);
    emit_uint(&mut out, 1);
    emit_uint(&mut out, 8);
    emit_bstr(&mut out, root);
    out
}

/// The envelope over a body (`wire-format.md` §3, §3.5): version 1, the
/// type, the body, and a `COSE_Sign` whose entries sort by `kid` then
/// classical before post-quantum.
pub fn envelope(tx_type: u64, body: &[u8], signers: &[&SigningIdentity]) -> Vec<u8> {
    let entries: Vec<(Keyhash, Vec<u8>)> = signers.iter().map(|s| (s.public.keyhash, s.sign_entries(aad::ENVELOPE, body))).collect();
    envelope_from_entries(tx_type, body, &entries)
}

/// The same envelope from entries each signer produced on its own, as a
/// ceremony gathers them (`wire-format.md` §3): each signer's two entries
/// over the body under `rhtn/1:envelope`, placed in ascending keyhash
/// order whatever order they arrived in.
pub fn envelope_from_entries(tx_type: u64, body: &[u8], entries: &[(Keyhash, Vec<u8>)]) -> Vec<u8> {
    let mut entries: Vec<&(Keyhash, Vec<u8>)> = entries.iter().collect();
    entries.sort_by_key(|(k, _)| *k);
    let mut out = Vec::new();
    emit_map_head(&mut out, 4);
    emit_uint(&mut out, 1);
    emit_uint(&mut out, 1);
    emit_uint(&mut out, 2);
    emit_uint(&mut out, tx_type);
    emit_uint(&mut out, 3);
    out.extend_from_slice(body);
    emit_uint(&mut out, 4);
    emit_array_head(&mut out, 4);
    emit_bstr(&mut out, b"");
    emit_map_head(&mut out, 0);
    emit_null(&mut out);
    emit_array_head(&mut out, entries.len() * 2);
    for (_, e) in entries {
        out.extend_from_slice(e);
    }
    out
}

/// A `SubtreeAck` (`wire-format.md` §7.5): the grandpatron's classical
/// signature over fields 1-4 with `external_aad = "rhtn/1:subtree-ack"`.
pub fn subtree_ack(grandpatron: &SigningIdentity, adoption: &Txid, node: &Keyhash, timestamp: u64) -> Vec<u8> {
    let mut payload = Vec::new();
    emit_map_head(&mut payload, 4);
    emit_uint(&mut payload, 1);
    emit_bstr(&mut payload, adoption);
    emit_uint(&mut payload, 2);
    emit_bstr(&mut payload, &grandpatron.public.keyhash);
    emit_uint(&mut payload, 3);
    emit_bstr(&mut payload, node);
    emit_uint(&mut payload, 4);
    emit_uint(&mut payload, timestamp);
    let sig = grandpatron.sign1_ed_unnamed(aad::SUBTREE_ACK, &payload);
    let mut out = Vec::new();
    emit_map_head(&mut out, 5);
    out.extend_from_slice(&payload[1..]);
    emit_uint(&mut out, 5);
    out.extend_from_slice(&sig);
    out
}

/// A currency attestation (`wire-format.md` §7.1) by `issuer` about
/// `subject`, naming `current` as its current key.
pub fn currency_attestation(issuer: &SigningIdentity, subject: &Keyhash, current: &Keyhash, issued_at: u64, expires_at: u64, role: u64) -> Vec<u8> {
    let mut payload = Vec::new();
    emit_map_head(&mut payload, 6);
    emit_uint(&mut payload, 1);
    emit_bstr(&mut payload, subject);
    emit_uint(&mut payload, 2);
    emit_bstr(&mut payload, current);
    emit_uint(&mut payload, 3);
    emit_uint(&mut payload, issued_at);
    emit_uint(&mut payload, 4);
    emit_uint(&mut payload, expires_at);
    emit_uint(&mut payload, 5);
    emit_uint(&mut payload, role);
    emit_uint(&mut payload, 6);
    emit_bstr(&mut payload, &issuer.public.keyhash);
    let sig = issuer.sign1_ed_unnamed(aad::CURRENCY, &payload);
    let mut out = Vec::new();
    emit_map_head(&mut out, 7);
    out.extend_from_slice(&payload[1..]);
    emit_uint(&mut out, 7);
    out.extend_from_slice(&sig);
    out
}
