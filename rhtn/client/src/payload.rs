//! Payload confidentiality on the client's side (design §14.2, §14.2.4;
//! `wire-format.md` §7.8; `light-client-requirements.md` §3): the key
//! agreement material a client publishes and keeps stocked, what it
//! prefetches and what it asks for only when opening a session, the
//! session itself, and the channel's framing that tells a protocol object
//! from application payload.
//!
//! What is decided here and not in the documents (design §22.2): the
//! bundle's blob and the one-time key's encoding, both this
//! construction's; the identity binding, classical, since the bundle's
//! signature is classical by the wire and a post-quantum signature does
//! not fit its bound; and the channel's framing, one tag on the initial
//! message and one on every plaintext.

use crate::Keyhash;
use crate::ratchet::Ratchet;
use rhtn_archive::prekey::{CONSTRUCTION_PQXDH, PrekeyBundle, PrekeyRequest};
use rhtn_codec::cbor::*;
use rhtn_codec::encode::*;
use rhtn_crypto::SigningIdentity;
use rhtn_crypto::pqxdh::{self, DhPublic, DhSecret, KemPublic, KemSecret, TheirBundle};
use rhtn_crypto::verify::Lookup;
use std::collections::{BTreeMap, BTreeSet};

/// The client's own numbers (`light-client-requirements.md` §3: cadence and
/// last-resort policy are the client's).
#[derive(Debug, Clone)]
pub struct PayloadConfig {
    /// How many one-time keys a fresh pool holds.
    pub pool_target: usize,
    /// Replenish when the pool the node reports falls below this.
    pub replenish_below: usize,
    /// How long a signed prekey serves before it is rotated.
    pub signed_prekey_interval_s: u64,
}

impl Default for PayloadConfig {
    fn default() -> Self {
        PayloadConfig { pool_target: 20, replenish_below: 5, signed_prekey_interval_s: 7 * 86_400 }
    }
}

/// A source of seeds: the device's randomness.
pub type Fresh<'a> = &'a mut dyn FnMut(&mut [u8]);

fn seed32(fresh: Fresh) -> [u8; 32] {
    let mut s = [0u8; 32];
    fresh(&mut s);
    s
}

fn seed64(fresh: Fresh) -> [u8; 64] {
    let mut s = [0u8; 64];
    fresh(&mut s);
    s
}

/// One one-time key pair, of both kinds, held until served once.
#[derive(Debug, Clone)]
pub struct OneTimePair {
    pub id: u32,
    pub dh: DhSecret,
    pub kem: KemSecret,
}

/// The material a client holds for PQXDH: its identity DH key, the signed
/// prekey and the post-quantum signed prekey in force, and the one-time
/// pairs not yet served.
#[derive(Debug, Clone)]
pub struct PayloadKeys {
    pub ik: DhSecret,
    pub spk_id: u32,
    pub spk: DhSecret,
    pub spk_since: u64,
    pub pqspk_id: u32,
    pub pqspk: KemSecret,
    /// Signed prekeys retired but kept a while for sessions opened against them.
    pub retired: BTreeMap<u32, (DhSecret, KemSecret)>,
    pub one_time: BTreeMap<u32, OneTimePair>,
    next_id: u32,
    pub published_at: Option<u64>,
}

impl PayloadKeys {
    pub fn generate(fresh: Fresh, now: u64) -> Self {
        PayloadKeys { ik: DhSecret::from_seed(seed32(fresh)), spk_id: 1, spk: DhSecret::from_seed(seed32(fresh)), spk_since: now, pqspk_id: 1, pqspk: KemSecret::from_seed(seed64(fresh)), retired: BTreeMap::new(), one_time: BTreeMap::new(), next_id: 1, published_at: None }
    }

    /// The reusable material as the bundle's blob carries it.
    pub fn blob(&self) -> Blob {
        Blob { ik: self.ik.public(), spk_id: self.spk_id, spk: self.spk.public(), pqspk_id: self.pqspk_id, pqspk: self.pqspk.public() }
    }

    /// The signed bundle to publish (`wire-format.md` §7.8).
    pub fn bundle(&mut self, id: &SigningIdentity, now: u64) -> Vec<u8> {
        self.published_at = Some(now);
        PrekeyBundle::build(id, CONSTRUCTION_PQXDH, &self.blob().encode(), now)
    }

    /// Mint `n` one-time pairs and return their public halves, as uploaded.
    pub fn one_time_keys(&mut self, n: usize, fresh: Fresh) -> Vec<Vec<u8>> {
        let mut out = Vec::new();
        for _ in 0..n {
            let id = self.next_id;
            self.next_id += 1;
            let pair = OneTimePair { id, dh: DhSecret::from_seed(seed32(fresh)), kem: KemSecret::from_seed(seed64(fresh)) };
            out.push(OneTimeKey { id, dh: pair.dh.public(), kem: pair.kem.public() }.encode());
            self.one_time.insert(id, pair);
        }
        out
    }

    /// The one-time pair `id` names, still held.  A receiver opens with
    /// this and spends the key with `take_one_time` once the message it
    /// opened has authenticated.
    pub fn one_time(&self, id: u32) -> Option<&OneTimePair> {
        self.one_time.get(&id)
    }

    /// Take a one-time pair by id: it is gone from the store, and a second
    /// initial message naming it finds nothing.
    pub fn take_one_time(&mut self, id: u32) -> Option<OneTimePair> {
        self.one_time.remove(&id)
    }

    pub fn due_for_rotation(&self, now: u64, cfg: &PayloadConfig) -> bool {
        now.saturating_sub(self.spk_since) >= cfg.signed_prekey_interval_s
    }

    /// Rotate the signed prekeys: fresh keys under fresh ids, the old ones
    /// retired but still answering sessions opened against them.
    pub fn rotate_signed_prekey(&mut self, now: u64, fresh: Fresh) {
        let old = (std::mem::replace(&mut self.spk, DhSecret::from_seed(seed32(fresh))), std::mem::replace(&mut self.pqspk, KemSecret::from_seed(seed64(fresh))));
        self.retired.insert(self.spk_id, old);
        self.spk_id += 1;
        self.pqspk_id += 1;
        self.spk_since = now;
    }

    fn signed_prekeys(&self, spk_id: u32) -> Option<(&DhSecret, &KemSecret)> {
        if spk_id == self.spk_id {
            Some((&self.spk, &self.pqspk))
        } else {
            self.retired.get(&spk_id).map(|(d, k)| (d, k))
        }
    }
}

/// The bundle's blob: the identity DH key, the signed prekey and the
/// post-quantum signed prekey, each prekey under an id the initial message
/// names.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Blob {
    pub ik: DhPublic,
    pub spk_id: u32,
    pub spk: DhPublic,
    pub pqspk_id: u32,
    pub pqspk: KemPublic,
}

impl Blob {
    pub fn encode(&self) -> Vec<u8> {
        let mut out = Vec::new();
        emit_map_head(&mut out, 5);
        emit_uint(&mut out, 1);
        emit_bstr(&mut out, &self.ik.0);
        emit_uint(&mut out, 2);
        emit_uint(&mut out, self.spk_id as u64);
        emit_uint(&mut out, 3);
        emit_bstr(&mut out, &self.spk.0);
        emit_uint(&mut out, 4);
        emit_uint(&mut out, self.pqspk_id as u64);
        emit_uint(&mut out, 5);
        emit_bstr(&mut out, &self.pqspk.0);
        out
    }

    pub fn decode(b: &[u8]) -> Result<Self, String> {
        let item = parse_all(b).map_err(|e| e.0)?;
        let Item::Map(m) = &item else { return Err("blob not a map".into()) };
        let k32 = |k: u64| -> Result<DhPublic, String> {
            match map_get(m, k) {
                Some(Item::Bytes(r)) if r.len() == 32 => Ok(DhPublic(<[u8; 32]>::try_from(&b[r.clone()]).unwrap())),
                _ => Err(format!("blob field {k}")),
            }
        };
        let pqspk = match map_get(m, 5) {
            Some(Item::Bytes(r)) if r.len() == pqxdh::KEM_PUBLIC_BYTES => KemPublic(b[r.clone()].to_vec()),
            _ => return Err("blob field 5".into()),
        };
        Ok(Blob { ik: k32(1)?, spk_id: map_get(m, 2).and_then(as_uint).ok_or("blob field 2")? as u32, spk: k32(3)?, pqspk_id: map_get(m, 4).and_then(as_uint).ok_or("blob field 4")? as u32, pqspk })
    }
}

/// A one-time key as served: its id and both public halves.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OneTimeKey {
    pub id: u32,
    pub dh: DhPublic,
    pub kem: KemPublic,
}

impl OneTimeKey {
    pub fn encode(&self) -> Vec<u8> {
        let mut out = Vec::new();
        emit_map_head(&mut out, 3);
        emit_uint(&mut out, 1);
        emit_uint(&mut out, self.id as u64);
        emit_uint(&mut out, 2);
        emit_bstr(&mut out, &self.dh.0);
        emit_uint(&mut out, 3);
        emit_bstr(&mut out, &self.kem.0);
        out
    }

    pub fn decode(b: &[u8]) -> Result<Self, String> {
        let item = parse_all(b).map_err(|e| e.0)?;
        let Item::Map(m) = &item else { return Err("one-time key not a map".into()) };
        let dh = match map_get(m, 2) {
            Some(Item::Bytes(r)) if r.len() == 32 => DhPublic(<[u8; 32]>::try_from(&b[r.clone()]).unwrap()),
            _ => return Err("one-time key field 2".into()),
        };
        let kem = match map_get(m, 3) {
            Some(Item::Bytes(r)) if r.len() == pqxdh::KEM_PUBLIC_BYTES => KemPublic(b[r.clone()].to_vec()),
            _ => return Err("one-time key field 3".into()),
        };
        Ok(OneTimeKey { id: map_get(m, 1).and_then(as_uint).ok_or("one-time key field 1")? as u32, dh, kem })
    }
}

/// A bundle read on the client's side: verified under the subject's
/// identity, its blob decoded.
#[derive(Debug, Clone)]
pub struct Prefetched {
    pub subject: Keyhash,
    pub published_at: u64,
    pub blob: Blob,
}

/// Read a bundle a serving node handed over: signed by the subject it
/// names, PQXDH, and a blob this construction reads.
pub fn read_bundle<L: Lookup + ?Sized>(ids: &L, bytes: &[u8]) -> Result<Prefetched, String> {
    let b = PrekeyBundle::parse(bytes)?;
    if b.verify(ids).is_err() {
        return Err("bundle signature fails".into());
    }
    if b.construction != CONSTRUCTION_PQXDH {
        return Err("not PQXDH".into());
    }
    Ok(Prefetched { subject: b.subject, published_at: b.published_at, blob: Blob::decode(&b.blob)? })
}

/// The batch request for a population, in ascending order and without
/// repeats (`wire-format.md` §7.8).
pub fn batch_request(population: &[Keyhash], nonce: [u8; 16]) -> Vec<u8> {
    let mut subjects = population.to_vec();
    subjects.sort();
    subjects.dedup();
    PrekeyRequest::Batch { subjects, nonce }.encode()
}

/// The one-time key request for one subject: sent only when a session is
/// being opened.
pub fn one_time_request(subject: Keyhash, nonce: [u8; 16]) -> Vec<u8> {
    PrekeyRequest::One { subject, one_time: true, nonce }.encode()
}

// --------------------------------------------------------- the channel

/// The first message of a session: the initiator's identity and ephemeral
/// keys, the KEM ciphertext, the ids of the prekeys used, and the first
/// ratchet message.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InitialMessage {
    pub ik: DhPublic,
    pub ek: DhPublic,
    pub kem_ciphertext: Vec<u8>,
    pub spk_id: u32,
    pub pqspk_id: u32,
    pub opk_id: Option<u32>,
    pub first: Vec<u8>,
}

/// Channel tags: what a byte string on the end-to-end channel is.
pub const CHANNEL_INITIAL: u64 = 0;
pub const CHANNEL_MESSAGE: u64 = 1;

/// Plaintext kinds, the demultiplexing this construction settles on
/// (design §14.2.4.6): a protocol object or application payload.
pub const KIND_APPLICATION: u64 = 0;
pub const KIND_KEY_GRANT: u64 = 1;
pub const KIND_LATE_RESPONSE: u64 = 2;
/// The direct path's candidates, exchanged with the peer alone over the
/// relayed channel (design §12.6.3, §14.1.1).
pub const KIND_CANDIDATES: u64 = 3;
/// The verifier's copy of its signed response, to the subject over the
/// association the grant established (`wire-format.md` §5.6).
pub const KIND_RESPONSE_COPY: u64 = 4;

/// A plaintext with its kind in front.
pub fn wrap(kind: u64, bytes: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    emit_array_head(&mut out, 2);
    emit_uint(&mut out, kind);
    emit_bstr(&mut out, bytes);
    out
}

pub fn unwrap(plaintext: &[u8]) -> Result<(u64, Vec<u8>), String> {
    let parts = array_item_ranges(plaintext, 0).ok_or("plaintext not an array")?;
    if parts.len() != 2 {
        return Err("plaintext is kind and bytes".into());
    }
    let p = Parser { b: plaintext };
    let (k, _) = p.item(parts[0].start).map_err(|e| e.0)?;
    let (v, _) = p.item(parts[1].start).map_err(|e| e.0)?;
    match (as_uint(&k), &v) {
        (Some(kind), Item::Bytes(r)) => Ok((kind, plaintext[r.clone()].to_vec())),
        _ => Err("plaintext shape".into()),
    }
}

fn channel(tag: u64, bytes: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    emit_array_head(&mut out, 2);
    emit_uint(&mut out, tag);
    emit_bstr(&mut out, bytes);
    out
}

fn unchannel(b: &[u8]) -> Result<(u64, Vec<u8>), String> {
    unwrap(b)
}

impl InitialMessage {
    pub fn encode(&self) -> Vec<u8> {
        let mut out = Vec::new();
        emit_map_head(&mut out, 6 + self.opk_id.is_some() as usize);
        emit_uint(&mut out, 1);
        emit_bstr(&mut out, &self.ik.0);
        emit_uint(&mut out, 2);
        emit_bstr(&mut out, &self.ek.0);
        emit_uint(&mut out, 3);
        emit_bstr(&mut out, &self.kem_ciphertext);
        emit_uint(&mut out, 4);
        emit_uint(&mut out, self.spk_id as u64);
        emit_uint(&mut out, 5);
        emit_uint(&mut out, self.pqspk_id as u64);
        if let Some(o) = self.opk_id {
            emit_uint(&mut out, 6);
            emit_uint(&mut out, o as u64);
        }
        emit_uint(&mut out, 7);
        emit_bstr(&mut out, &self.first);
        out
    }

    pub fn decode(b: &[u8]) -> Result<Self, String> {
        let item = parse_all(b).map_err(|e| e.0)?;
        let Item::Map(m) = &item else { return Err("initial message not a map".into()) };
        let k32 = |k: u64| -> Result<DhPublic, String> {
            match map_get(m, k) {
                Some(Item::Bytes(r)) if r.len() == 32 => Ok(DhPublic(<[u8; 32]>::try_from(&b[r.clone()]).unwrap())),
                _ => Err(format!("initial message field {k}")),
            }
        };
        let bytes = |k: u64| -> Result<Vec<u8>, String> {
            match map_get(m, k) {
                Some(Item::Bytes(r)) => Ok(b[r.clone()].to_vec()),
                _ => Err(format!("initial message field {k}")),
            }
        };
        Ok(InitialMessage {
            ik: k32(1)?,
            ek: k32(2)?,
            kem_ciphertext: bytes(3)?,
            spk_id: map_get(m, 4).and_then(as_uint).ok_or("field 4")? as u32,
            pqspk_id: map_get(m, 5).and_then(as_uint).ok_or("field 5")? as u32,
            opk_id: map_get(m, 6).and_then(as_uint).map(|v| v as u32),
            first: bytes(7)?,
        })
    }
}

/// Why a session could not be opened or a message read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PayloadError {
    NoSession,
    NoBundle,
    /// An initial message whose identity key is not the one bound to the
    /// sender it names (design §14.2.4.2).
    NotTheSender,
    Malformed(String),
    Crypto(String),
}

impl std::fmt::Display for PayloadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PayloadError::NoSession => write!(f, "no session with that peer"),
            PayloadError::NoBundle => write!(f, "no bundle for that peer"),
            PayloadError::NotTheSender => write!(f, "the initial message's identity key is not the named sender's"),
            PayloadError::Malformed(s) => write!(f, "malformed: {s}"),
            PayloadError::Crypto(s) => write!(f, "{s}"),
        }
    }
}

/// The sessions a client holds, one per peer, and the bundles it
/// prefetched.
#[derive(Default)]
pub struct Sessions {
    pub ratchets: BTreeMap<Keyhash, Ratchet>,
    pub prefetched: BTreeMap<Keyhash, Prefetched>,
}

impl Sessions {
    /// Open a session to `to` on its bundle and, where served, a one-time
    /// key, and encrypt the first plaintext: the channel's initial message.
    pub fn open(&mut self, keys: &PayloadKeys, to: Keyhash, their: &Prefetched, one_time: Option<&OneTimeKey>, fresh: Fresh, plaintext: &[u8]) -> Result<Vec<u8>, PayloadError> {
        let ek = DhSecret::from_seed(seed32(fresh));
        let bundle = TheirBundle { ik: &their.blob.ik, spk: &their.blob.spk, pqspk: &their.blob.pqspk, opk: one_time.map(|o| &o.dh), pqopk: one_time.map(|o| &o.kem) };
        let init = pqxdh::initiate(&keys.ik, &ek, &bundle, seed32(fresh)).map_err(PayloadError::Crypto)?;
        let ad = pqxdh::associated_data(&keys.ik.public(), &their.blob.ik);
        let mut r = Ratchet::initiator(init.sk, their.blob.spk, DhSecret::from_seed(seed32(fresh)), ad);
        let first = r.encrypt(plaintext).map_err(PayloadError::Crypto)?;
        self.ratchets.insert(to, r);
        let msg = InitialMessage { ik: keys.ik.public(), ek: init.ek, kem_ciphertext: init.kem_ciphertext, spk_id: their.blob.spk_id, pqspk_id: their.blob.pqspk_id, opk_id: one_time.map(|o| o.id), first };
        Ok(channel(CHANNEL_INITIAL, &msg.encode()))
    }

    /// Encrypt on an open session.
    pub fn send(&mut self, to: &Keyhash, plaintext: &[u8]) -> Result<Vec<u8>, PayloadError> {
        let r = self.ratchets.get_mut(to).ok_or(PayloadError::NoSession)?;
        let m = r.encrypt(plaintext).map_err(PayloadError::Crypto)?;
        Ok(channel(CHANNEL_MESSAGE, &m))
    }

    pub fn has_session(&self, peer: &Keyhash) -> bool {
        self.ratchets.contains_key(peer)
    }

    /// Read what arrived on the channel from `from`: an initial message
    /// opens a session on this client's prekeys, consuming the one-time
    /// pair it names; a message decrypts on the session held.
    pub fn receive(&mut self, keys: &mut PayloadKeys, from: Keyhash, bytes: &[u8], fresh: Fresh) -> Result<Vec<u8>, PayloadError> {
        let (tag, inner) = unchannel(bytes).map_err(PayloadError::Malformed)?;
        let mut next = |out: &mut [u8; 32]| fresh(out);
        match tag {
            CHANNEL_INITIAL => {
                let m = InitialMessage::decode(&inner).map_err(PayloadError::Malformed)?;
                // The sender is the keyhash the channel names; the identity
                // key is the one that keyhash published and signed for
                // (design §14.2.4.2).  An initial message whose key is not
                // the one bound to the name is not that party's, whatever
                // it decrypts to, and the transport authenticating a relay
                // hop says nothing about who wrote this.  Held under no
                // binding, it cannot be attributed and is not opened.
                let bound = self.prefetched.get(&from).ok_or(PayloadError::NoBundle)?.blob.ik;
                if bound != m.ik {
                    return Err(PayloadError::NotTheSender);
                }
                let (spk, pqspk) = keys.signed_prekeys(m.spk_id).ok_or(PayloadError::Crypto("unknown signed prekey".into()))?;
                let (spk, pqspk) = (spk.clone(), pqspk.clone());
                // Used now, spent later: the pair opens the message and is
                // removed once that message authenticates.  Removing it
                // first lets one corrupted copy destroy the key the real
                // message needs, and PQXDH deletes it after decryption.
                let one_time = match m.opk_id {
                    Some(id) => Some(keys.one_time(id).ok_or(PayloadError::Crypto("one-time key already used or unknown".into()))?.clone()),
                    None => None,
                };
                let me = pqxdh::Responder { ik: &keys.ik, spk: &spk, pqspk: &pqspk, opk: one_time.as_ref().map(|o| &o.dh), pqopk: one_time.as_ref().map(|o| &o.kem) };
                let sk = pqxdh::respond(&me, &m.ik, &m.ek, &m.kem_ciphertext).map_err(PayloadError::Crypto)?;
                let ad = pqxdh::associated_data(&m.ik, &keys.ik.public());
                let mut r = Ratchet::responder(sk, spk, ad);
                let mut seed = || {
                    let mut s = [0u8; 32];
                    next(&mut s);
                    s
                };
                let pt = r.decrypt(&m.first, &mut seed).map_err(PayloadError::Crypto)?;
                // authenticated: the key is spent and the session committed
                if let Some(o) = &one_time {
                    keys.take_one_time(o.id);
                }
                self.ratchets.insert(from, r);
                Ok(pt)
            }
            CHANNEL_MESSAGE => {
                let r = self.ratchets.get_mut(&from).ok_or(PayloadError::NoSession)?;
                let mut seed = || {
                    let mut s = [0u8; 32];
                    next(&mut s);
                    s
                };
                r.decrypt(&inner, &mut seed).map_err(PayloadError::Crypto)
            }
            _ => Err(PayloadError::Malformed("unknown channel tag".into())),
        }
    }
}

/// Everything the client keeps for payload: its numbers, its material,
/// its sessions, the serving node it publishes to, the plaintexts waiting
/// for a one-time key to arrive, and the one-time requests in flight.
pub struct PayloadState {
    pub cfg: PayloadConfig,
    pub keys: PayloadKeys,
    pub sessions: Sessions,
    pub serving: Option<Keyhash>,
    pub pending: BTreeMap<Keyhash, Vec<Vec<u8>>>,
    pub outstanding: BTreeMap<[u8; 16], Keyhash>,
    /// What the node last reported of the pool, or what was uploaded.
    pub pool_reported: usize,
    /// Peers who sent an initial message this client could not attribute
    /// for want of their binding.  Routine maintenance asks for it, so a
    /// peer that published after this client's last sweep is reachable on
    /// its next attempt (design §14.2.4.2).
    pub wanted: BTreeSet<Keyhash>,
}

impl PayloadState {
    pub fn new(cfg: PayloadConfig, fresh: Fresh, now: u64) -> Self {
        PayloadState { keys: PayloadKeys::generate(fresh, now), cfg, sessions: Sessions::default(), serving: None, pending: BTreeMap::new(), outstanding: BTreeMap::new(), pool_reported: 0, wanted: BTreeSet::new() }
    }
}
