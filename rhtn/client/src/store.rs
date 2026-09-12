//! The sealed capture store (design §7.5.2, §7.5.2.6 to §7.5.2.8;
//! `light-client-requirements.md` §1.3): one sealed capture per presence
//! record, under a key derived from the depicted subject's seed, which the
//! holder discards once the capture is sealed and regains only when the
//! subject releases it again.  The AEAD, its nonce and the framing are
//! undecided by the design (design §22.2), so they are parameters here with
//! the reference client's defaults; a test supplies its own.
//!
//! [`ClientStore`] is the client's persistent state, laid out so a test can
//! see what a compliant client holds: sealed captures of others, its own
//! seeds, its records and the late responses beside them, and never a
//! plaintext likeness or a released key.

use crate::{Keyhash, Txid};
use aws_lc_rs::aead::{AES_256_GCM, Aad, CHACHA20_POLY1305, LessSafeKey, Nonce, UnboundKey};
use aws_lc_rs::rand::{SecureRandom, SystemRandom};
use crate::record::DisclosureSet;
use std::collections::BTreeMap;

/// The AEAD a store seals under: a parameter (design §22.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Aead {
    Aes256Gcm,
    ChaCha20Poly1305,
}

/// The sealed store's parameters: the AEAD, and the template's fixed length
/// for the modality version in use (design §7.5.2.7), so a holder knows
/// what it is reading.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SealParams {
    pub aead: Aead,
    pub template_len: usize,
}

impl Default for SealParams {
    /// The reference client's defaults: AES-256-GCM and a 32-byte template,
    /// chosen values with nothing behind them yet (design §22.2).
    fn default() -> Self {
        SealParams { aead: Aead::Aes256Gcm, template_len: 32 }
    }
}

/// One captured frame, its metadata already stripped
/// (`light-client-requirements.md` §1.3), with the instant it was taken.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Frame {
    pub at_ms: u64,
    pub bytes: Vec<u8>,
}

/// What a capture holds in the clear: the template first, then the frames
/// (design §7.5.2.7).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Capture {
    pub modality: u64,
    pub template_version: u64,
    pub template: Vec<u8>,
    pub frames: Vec<Frame>,
}

/// A capture sealed under one key: whose likeness it is, who holds it, the
/// ceremony that sealed it, and the ciphertext.  Nothing here opens it.
/// It is bound to the ceremony and not to the record (design §7.5.2.6):
/// sealing happens at capture, before the record exists, and the record
/// is what the store files it under afterwards.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SealedCapture {
    pub subject: Keyhash,
    pub holder: Keyhash,
    pub ceremony_id: [u8; 32],
    pub modality: u64,
    pub template_version: u64,
    pub nonce: [u8; 12],
    pub ciphertext: Vec<u8>,
}

/// Why a sealed capture did not open: each is a decryption failure, which a
/// verifier reports as `inconclusive` and never as `no-match`
/// (design §7.5.2.7, `wire-format.md` §5.5).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OpenFailure {
    /// Shorter than a tag: nothing to authenticate.
    Truncated,
    /// The tag does not verify under this key.
    Unauthenticated,
    /// Authenticated bytes that are not a capture.
    Framing,
}

const SEAL_AAD_TAG: &[u8] = b"rhtn/1:sealed-capture";

fn aad_of(subject: &Keyhash, holder: &Keyhash, ceremony_id: &[u8; 32], modality: u64, template_version: u64) -> Vec<u8> {
    let mut aad = Vec::with_capacity(SEAL_AAD_TAG.len() + 96 + 16);
    aad.extend_from_slice(SEAL_AAD_TAG);
    aad.extend_from_slice(subject);
    aad.extend_from_slice(holder);
    aad.extend_from_slice(ceremony_id);
    aad.extend_from_slice(&modality.to_be_bytes());
    aad.extend_from_slice(&template_version.to_be_bytes());
    aad
}

fn key_of(aead: Aead, key: &[u8; 32]) -> LessSafeKey {
    let alg = match aead {
        Aead::Aes256Gcm => &AES_256_GCM,
        Aead::ChaCha20Poly1305 => &CHACHA20_POLY1305,
    };
    LessSafeKey::new(UnboundKey::new(alg, key).expect("a 32-byte key"))
}

/// Seal `capture` under `key`: the template, which must be exactly the
/// parameters' length, then each frame as its instant and its
/// length-prefixed bytes, under a fresh nonce.
pub fn seal(p: &SealParams, key: &[u8; 32], subject: Keyhash, holder: Keyhash, ceremony_id: [u8; 32], capture: &Capture) -> SealedCapture {
    assert_eq!(capture.template.len(), p.template_len, "the template has the modality version's fixed length");
    let mut plain = Vec::new();
    plain.extend_from_slice(&capture.template);
    for f in &capture.frames {
        plain.extend_from_slice(&f.at_ms.to_be_bytes());
        plain.extend_from_slice(&(f.bytes.len() as u32).to_be_bytes());
        plain.extend_from_slice(&f.bytes);
    }
    let mut nonce = [0u8; 12];
    SystemRandom::new().fill(&mut nonce).expect("the system's random source");
    let aad = aad_of(&subject, &holder, &ceremony_id, capture.modality, capture.template_version);
    key_of(p.aead, key).seal_in_place_append_tag(Nonce::assume_unique_for_key(nonce), Aad::from(&aad), &mut plain).expect("sealing cannot fail");
    SealedCapture { subject, holder, ceremony_id, modality: capture.modality, template_version: capture.template_version, nonce, ciphertext: plain }
}

/// Open a sealed capture under `key`.  Anything short of an authenticated,
/// well-framed capture is a decryption failure.
pub fn open(p: &SealParams, key: &[u8; 32], sealed: &SealedCapture) -> Result<Capture, OpenFailure> {
    if sealed.ciphertext.len() < 16 {
        return Err(OpenFailure::Truncated);
    }
    let mut buf = sealed.ciphertext.clone();
    let aad = aad_of(&sealed.subject, &sealed.holder, &sealed.ceremony_id, sealed.modality, sealed.template_version);
    let plain = key_of(p.aead, key).open_in_place(Nonce::assume_unique_for_key(sealed.nonce), Aad::from(&aad), &mut buf).map_err(|_| OpenFailure::Unauthenticated)?;
    if plain.len() < p.template_len {
        return Err(OpenFailure::Framing);
    }
    let template = plain[..p.template_len].to_vec();
    let mut frames = Vec::new();
    let mut at = p.template_len;
    while at < plain.len() {
        if plain.len() - at < 12 {
            return Err(OpenFailure::Framing);
        }
        let at_ms = u64::from_be_bytes(plain[at..at + 8].try_into().unwrap());
        let n = u32::from_be_bytes(plain[at + 8..at + 12].try_into().unwrap()) as usize;
        at += 12;
        if plain.len() - at < n {
            return Err(OpenFailure::Framing);
        }
        frames.push(Frame { at_ms, bytes: plain[at..at + n].to_vec() });
        at += n;
    }
    Ok(Capture { modality: sealed.modality, template_version: sealed.template_version, template, frames })
}

/// A subject's own seed for one of its records (design §7.5.2, §7.5.2.9):
/// what unlocks its likeness on that counterparty's device, with what a
/// grant needs to name the capture and derive its key.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OwnSeed {
    pub seed: [u8; 32],
    pub counterparty: Keyhash,
    pub ceremony_id: [u8; 32],
    pub finalized_at: u64,
}

/// A compliant client's persistent state (design §7.5.2, §7.5.2.9;
/// `light-client-requirements.md` §1.3): the sealed captures it holds of
/// others, by record; its own seeds, by record, which unlock its likeness
/// on others' devices; the presence records it keeps; and the late
/// responses kept beside them and discarded with them; and the disclosure
/// sets of its own records, which are ordinary record state
/// (`wire-format.md` §4.5.1.2).  No plaintext likeness and no released key
/// is ever written here.
#[derive(Debug, Default, Clone)]
pub struct ClientStore {
    pub sealed: BTreeMap<Txid, SealedCapture>,
    pub seeds: BTreeMap<Txid, OwnSeed>,
    pub records: BTreeMap<Txid, Vec<u8>>,
    pub late: BTreeMap<Txid, Vec<Vec<u8>>>,
    /// Late responses that verified and named a participant but could not
    /// be attached, by the record they were offered for, naming their
    /// verifier.  **A late response is a sign of the responder's
    /// reliability whatever became of it** [author, 2026-09-12], so the
    /// arrival is kept even where the evidence is refused.  The bytes are
    /// not: retention follows the record (`wire-format.md` §7.4), and
    /// keeping evidence this holder declined to admit would outlive the
    /// reason it was declined.
    pub unattached_late: BTreeMap<Txid, Vec<Keyhash>>,
    pub disclosures: BTreeMap<Txid, DisclosureSet>,
}

impl ClientStore {
    /// Whether `needle` appears anywhere in what the store holds: what a
    /// test asks to show that no key and no plaintext frame is at rest.
    pub fn holds_bytes(&self, needle: &[u8]) -> bool {
        let find = |hay: &[u8]| hay.windows(needle.len()).any(|w| w == needle);
        self.sealed.values().any(|s| find(&s.ciphertext) || find(&s.nonce))
            || self.seeds.values().any(|s| find(&s.seed))
            || self.records.values().any(|r| find(r))
            || self.late.values().flatten().any(|l| find(l))
    }

    /// Whether this client's own presence records name `k` as a
    /// participant: what refutes a claimed `met` (`wire-format.md` §5.6).
    pub fn has_met(&self, k: &Keyhash) -> bool {
        self.records.values().any(|r| rhtn_archive::record::Record::parse(r).is_ok_and(|rec| rec.participants().contains(k)))
    }

    /// Discard a record and everything kept beside it (`wire-format.md`
    /// §7.4: retention follows the record it supplements).
    pub fn discard_record(&mut self, txid: &Txid) {
        self.records.remove(txid);
        self.late.remove(txid);
        self.unattached_late.remove(txid);
        self.disclosures.remove(txid);
    }
}
