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
#[derive(Debug, Default, Clone, PartialEq, Eq)]
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

// ---------------------------------------------------------------- on disk

/// Where a client's own state goes, and how it comes back.
///
/// **Three shapes, because the five maps have three lifetimes.** Records,
/// sealed captures and seeds are written once and never rewritten — the
/// archive is append-only (design §13.7.1) and so is everything keyed
/// beside it. Late responses, the arrivals that could not be attached and
/// the disclosure sets accumulate against a record, so their file is
/// rewritten when it grows.
///
/// **A seed is the only secret here** (design §7.5.2: the subject holds
/// the key to their own likeness on someone else's device), so its file is
/// written 0600 like the identity's. Sealed captures are ciphertext this
/// client cannot open and records are public by construction.
///
/// **Nothing ages out on load.** design §13.7.1 is explicit that
/// age-based flushing of live data is the wrong fix and **import** is
/// where over-retention leaks, so deletion belongs to the import path and
/// not here. The retention window itself is enforced where §7.5.1 puts it:
/// a subject past it declines to release a capture key, and the holder's
/// ciphertext becomes unopenable without anyone deleting anything.
impl ClientStore {
    pub fn save(&self, dir: &std::path::Path) -> std::io::Result<()> {
        write_once(&dir.join("records"), self.records.iter().map(|(t, b)| (*t, b.clone())), false)?;
        write_once(&dir.join("sealed"), self.sealed.iter().map(|(t, c)| (*t, encode_sealed(c))), false)?;
        write_once(&dir.join("seeds"), self.seeds.iter().map(|(t, s)| (*t, encode_seed(s))), true)?;
        rewrite(&dir.join("late"), self.late.iter().map(|(t, v)| (*t, encode_blobs(v))))?;
        rewrite(&dir.join("unattached"), self.unattached_late.iter().map(|(t, v)| (*t, encode_keys(v))))?;
        rewrite(&dir.join("disclosures"), self.disclosures.iter().map(|(t, d)| (*t, encode_disclosures(d))))?;
        Ok(())
    }

    /// The whole store as one document, for a backup to carry
    /// (`backup`): the same six encodings the directory holds, in one
    /// place, since a blob has no directory to spread them over.
    pub fn encode(&self) -> Vec<u8> {
        use rhtn_codec::encode::*;
        let mut out = Vec::new();
        emit_array_head(&mut out, 6);
        keyed(&mut out, self.records.iter().map(|(t, b)| (*t, b.clone())));
        keyed(&mut out, self.sealed.iter().map(|(t, c)| (*t, encode_sealed(c))));
        keyed(&mut out, self.seeds.iter().map(|(t, s)| (*t, encode_seed(s))));
        keyed(&mut out, self.late.iter().map(|(t, v)| (*t, encode_blobs(v))));
        keyed(&mut out, self.unattached_late.iter().map(|(t, v)| (*t, encode_keys(v))));
        keyed(&mut out, self.disclosures.iter().map(|(t, d)| (*t, encode_disclosures(d))));
        out
    }

    /// One back.  **Nothing partial**: a store that decoded five of six
    /// maps is not a store, and returning one would be the half-import a
    /// restore must not perform.
    pub fn decode(b: &[u8]) -> Option<ClientStore> {
        let item = rhtn_codec::cbor::parse_all(b).ok()?;
        let rhtn_codec::cbor::Item::Array(f) = &item else { return None };
        if f.len() != 6 {
            return None;
        }
        let m = |i: usize| unkeyed(b, &f[i]);
        let mut st = ClientStore::default();
        for (t, v) in m(0)? {
            st.records.insert(t, v);
        }
        for (t, v) in m(1)? {
            st.sealed.insert(t, decode_sealed(&v)?);
        }
        for (t, v) in m(2)? {
            st.seeds.insert(t, decode_seed(&v)?);
        }
        for (t, v) in m(3)? {
            st.late.insert(t, decode_blobs(&v)?);
        }
        for (t, v) in m(4)? {
            st.unattached_late.insert(t, decode_keys(&v)?);
        }
        for (t, v) in m(5)? {
            st.disclosures.insert(t, decode_disclosures(&v)?);
        }
        Some(st)
    }

    /// Read a store back.  **What no longer parses is skipped rather than
    /// failing the load**, the same posture the node's store takes: a
    /// client that cannot start because one file went bad has lost more
    /// than the file.
    pub fn load(dir: &std::path::Path) -> std::io::Result<ClientStore> {
        let mut st = ClientStore::default();
        for (t, b) in read_dir_of(&dir.join("records"))? {
            st.records.insert(t, b);
        }
        for (t, b) in read_dir_of(&dir.join("sealed"))? {
            if let Some(c) = decode_sealed(&b) {
                st.sealed.insert(t, c);
            }
        }
        for (t, b) in read_dir_of(&dir.join("seeds"))? {
            if let Some(s) = decode_seed(&b) {
                st.seeds.insert(t, s);
            }
        }
        for (t, b) in read_dir_of(&dir.join("late"))? {
            if let Some(v) = decode_blobs(&b) {
                st.late.insert(t, v);
            }
        }
        for (t, b) in read_dir_of(&dir.join("unattached"))? {
            if let Some(v) = decode_keys(&b) {
                st.unattached_late.insert(t, v);
            }
        }
        for (t, b) in read_dir_of(&dir.join("disclosures"))? {
            if let Some(d) = decode_disclosures(&b) {
                st.disclosures.insert(t, d);
            }
        }
        Ok(st)
    }
}

fn hex_of(t: &Txid) -> String {
    t.iter().map(|b| format!("{b:02x}")).collect()
}

fn unhex(s: &str) -> Option<Txid> {
    if s.len() != 64 {
        return None;
    }
    let mut out = [0u8; 32];
    for (i, b) in out.iter_mut().enumerate() {
        *b = u8::from_str_radix(s.get(i * 2..i * 2 + 2)?, 16).ok()?;
    }
    Some(out)
}

/// Written once and left alone: a file already there is the same bytes.
fn write_once(root: &std::path::Path, items: impl Iterator<Item = (Txid, Vec<u8>)>, private: bool) -> std::io::Result<()> {
    std::fs::create_dir_all(root)?;
    for (t, bytes) in items {
        let p = root.join(hex_of(&t));
        if p.exists() {
            continue;
        }
        std::fs::write(&p, &bytes)?;
        if private {
            restrict(&p)?;
        }
    }
    Ok(())
}

/// Rewritten each time, because what it holds grows against its record.
fn rewrite(root: &std::path::Path, items: impl Iterator<Item = (Txid, Vec<u8>)>) -> std::io::Result<()> {
    std::fs::create_dir_all(root)?;
    for (t, bytes) in items {
        std::fs::write(root.join(hex_of(&t)), &bytes)?;
    }
    Ok(())
}

#[cfg(unix)]
fn restrict(p: &std::path::Path) -> std::io::Result<()> {
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(p, std::fs::Permissions::from_mode(0o600))
}

#[cfg(not(unix))]
fn restrict(_: &std::path::Path) -> std::io::Result<()> {
    Ok(())
}

fn read_dir_of(root: &std::path::Path) -> std::io::Result<Vec<(Txid, Vec<u8>)>> {
    let mut out = Vec::new();
    let Ok(rd) = std::fs::read_dir(root) else { return Ok(out) };
    for e in rd.flatten() {
        if let Some(t) = unhex(&e.file_name().to_string_lossy()) {
            out.push((t, std::fs::read(e.path())?));
        }
    }
    Ok(out)
}

// The six encodings.  Deterministic CBOR, as everything written here is:
// a store that round-trips differently from how it was written is one
// whose files cannot be compared.

fn encode_sealed(c: &SealedCapture) -> Vec<u8> {
    use rhtn_codec::encode::*;
    let mut out = Vec::new();
    emit_array_head(&mut out, 7);
    emit_bstr(&mut out, &c.subject);
    emit_bstr(&mut out, &c.holder);
    emit_bstr(&mut out, &c.ceremony_id);
    emit_uint(&mut out, c.modality);
    emit_uint(&mut out, c.template_version);
    emit_bstr(&mut out, &c.nonce);
    emit_bstr(&mut out, &c.ciphertext);
    out
}

fn decode_sealed(b: &[u8]) -> Option<SealedCapture> {
    let a = array_of(b, 7)?;
    Some(SealedCapture {
        subject: kh_at(b, &a[0])?,
        holder: kh_at(b, &a[1])?,
        ceremony_id: kh_at(b, &a[2])?,
        modality: uint_at(&a[3])?,
        template_version: uint_at(&a[4])?,
        nonce: bytes_at(b, &a[5])?.try_into().ok()?,
        ciphertext: bytes_at(b, &a[6])?,
    })
}

fn encode_seed(s: &OwnSeed) -> Vec<u8> {
    use rhtn_codec::encode::*;
    let mut out = Vec::new();
    emit_array_head(&mut out, 4);
    emit_bstr(&mut out, &s.seed);
    emit_bstr(&mut out, &s.counterparty);
    emit_bstr(&mut out, &s.ceremony_id);
    emit_uint(&mut out, s.finalized_at);
    out
}

fn decode_seed(b: &[u8]) -> Option<OwnSeed> {
    let a = array_of(b, 4)?;
    Some(OwnSeed { seed: kh_at(b, &a[0])?, counterparty: kh_at(b, &a[1])?, ceremony_id: kh_at(b, &a[2])?, finalized_at: uint_at(&a[3])? })
}

fn encode_blobs(v: &[Vec<u8>]) -> Vec<u8> {
    use rhtn_codec::encode::*;
    let mut out = Vec::new();
    emit_array_head(&mut out, v.len());
    for b in v {
        emit_bstr(&mut out, b);
    }
    out
}

fn decode_blobs(b: &[u8]) -> Option<Vec<Vec<u8>>> {
    let item = rhtn_codec::cbor::parse_all(b).ok()?;
    let rhtn_codec::cbor::Item::Array(a) = &item else { return None };
    a.iter().map(|x| bytes_at(b, x)).collect()
}

fn encode_keys(v: &[Keyhash]) -> Vec<u8> {
    encode_blobs(&v.iter().map(|k| k.to_vec()).collect::<Vec<_>>())
}

fn decode_keys(b: &[u8]) -> Option<Vec<Keyhash>> {
    decode_blobs(b)?.into_iter().map(|v| v.try_into().ok()).collect()
}

fn encode_disclosures(d: &DisclosureSet) -> Vec<u8> {
    use rhtn_codec::encode::*;
    let mut out = Vec::new();
    emit_array_head(&mut out, 7);
    for one in d {
        emit_array_head(&mut out, 2);
        emit_bstr(&mut out, &one.salt);
        emit_bstr(&mut out, &one.value);
    }
    out
}

/// **The labels are not written.** A disclosure set is the seven labels in
/// their fixed order (`wire-format.md` §4.5.1), so storing them would be
/// storing the same seven strings against every record — and a file whose
/// labels disagreed with the order would be a set this client cannot use
/// anyway.
fn decode_disclosures(b: &[u8]) -> Option<DisclosureSet> {
    let a = array_of(b, 7)?;
    let mut out: Vec<crate::record::Disclosure> = Vec::with_capacity(7);
    for (i, item) in a.iter().enumerate() {
        let rhtn_codec::cbor::Item::Array(f) = item else { return None };
        if f.len() != 2 {
            return None;
        }
        out.push(crate::record::Disclosure {
            label: crate::record::LABELS[i],
            salt: bytes_at(b, &f[0])?.try_into().ok()?,
            value: bytes_at(b, &f[1])?,
        });
    }
    out.try_into().ok()
}

fn array_of(b: &[u8], n: usize) -> Option<Vec<rhtn_codec::cbor::Item>> {
    let item = rhtn_codec::cbor::parse_all(b).ok()?;
    let rhtn_codec::cbor::Item::Array(a) = &item else { return None };
    (a.len() == n).then(|| a.clone())
}

fn bytes_at(b: &[u8], it: &rhtn_codec::cbor::Item) -> Option<Vec<u8>> {
    match it {
        rhtn_codec::cbor::Item::Bytes(r) => Some(b[r.clone()].to_vec()),
        _ => None,
    }
}

fn kh_at(b: &[u8], it: &rhtn_codec::cbor::Item) -> Option<[u8; 32]> {
    bytes_at(b, it)?.try_into().ok()
}

fn uint_at(it: &rhtn_codec::cbor::Item) -> Option<u64> {
    match it {
        rhtn_codec::cbor::Item::Uint(n) => Some(*n),
        _ => None,
    }
}

/// One map as `[[txid, bytes], ...]`, in the key order a `BTreeMap` gives:
/// two stores holding the same thing encode the same bytes.
fn keyed(out: &mut Vec<u8>, items: impl ExactSizeIterator<Item = (Txid, Vec<u8>)>) {
    use rhtn_codec::encode::*;
    emit_array_head(out, items.len());
    for (t, v) in items {
        emit_array_head(out, 2);
        emit_bstr(out, &t);
        emit_bstr(out, &v);
    }
}

fn unkeyed(b: &[u8], it: &rhtn_codec::cbor::Item) -> Option<Vec<(Txid, Vec<u8>)>> {
    let rhtn_codec::cbor::Item::Array(a) = it else { return None };
    a.iter()
        .map(|x| {
            let rhtn_codec::cbor::Item::Array(p) = x else { return None };
            match p.as_slice() {
                [t, v] => Some((kh_at(b, t)?, bytes_at(b, v)?)),
                _ => None,
            }
        })
        .collect()
}
