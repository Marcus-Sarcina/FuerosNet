//! The sealed capture store (design §7.5.2, §7.5.2.6 to §7.5.2.8;
//! `light-client-requirements.md` §1.3): one sealed capture per presence
//! record, under a key derived from the depicted subject's identity root,
//! which the holder discards once the capture is sealed and regains only
//! when the subject releases it again.
//!
//! **The AEAD is XChaCha20-Poly1305, and the design fixes it** [author,
//! 2026-10-16]: each slot of the capture is sealed on its own under a
//! 24-byte nonce drawn at random and stored in front of its ciphertext,
//! with the ceremony's pre-commitment and the slot's ASCII label as
//! associated data, framed as one version byte then the nonce then the
//! ciphertext (design §7.5.2.6).  **One key opens every slot** and the
//! label is its position's: `template` first, then `image1`, `image2` and
//! so on (design §7.5.2.7), so a slot lifted from another ceremony and a
//! template presented as an image both fail to open where they land.
//!
//! [`ClientStore`] is the client's persistent state, laid out so a test can
//! see what a compliant client holds: sealed captures of others, its own
//! own captures, its records and the late responses beside them, and never a
//! plaintext likeness or a released key.

use crate::record::DisclosureSet;
use crate::{Keyhash, Txid};
use aws_lc_rs::rand::{SecureRandom, SystemRandom};
use chacha20poly1305::aead::{Aead as _, KeyInit, Payload};
use chacha20poly1305::{XChaCha20Poly1305, XNonce};
use std::collections::BTreeMap;

/// The framing's version byte, first of every sealed slot
/// (design §7.5.2.6).
pub const SLOT_VERSION: u8 = 1;
/// The nonce's width: XChaCha20-Poly1305's extended nonce, which is what
/// lets it be drawn at random rather than counted (design §7.5.2.6).
pub const NONCE_LEN: usize = 24;

/// The sealed store's parameters: the template's fixed length for the
/// modality version in use (design §7.5.2.7), so a holder knows what it is
/// reading.  **The AEAD is not among them**: design §7.5.2.6 fixes it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SealParams {
    /// The template's fixed length for the modality version in use.
    pub template_len: usize,
}

impl Default for SealParams {
    /// A 32-byte template, which is the stand-in engine's
    /// ([`crate::device::HashEngine`]).  A platform supplying a real
    /// engine states its own length.
    fn default() -> Self {
        SealParams { template_len: 32 }
    }
}

/// The ASCII label of slot `i`: `template` first, then `image1` onward
/// (design §7.5.2.6).  It is the slot's position, so a slot moved to
/// another position does not open there.
pub fn slot_label(i: usize) -> String {
    if i == 0 {
        "template".to_string()
    } else {
        format!("image{i}")
    }
}

/// One captured frame, its metadata already stripped
/// (`light-client-requirements.md` §1.3), with the instant it was taken.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Frame {
    /// When it was taken, in milliseconds since the epoch.
    pub at_ms: u64,
    /// The pixels, their metadata already stripped.
    pub bytes: Vec<u8>,
}

/// What a capture holds in the clear: the template first, then the frames
/// (design §7.5.2.7).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Capture {
    /// Which modality was captured.
    pub modality: u64,
    /// The template version it was derived under.
    pub template_version: u64,
    /// The template itself, of the modality version's fixed length.
    pub template: Vec<u8>,
    /// The frames behind it.
    pub frames: Vec<Frame>,
}

/// A capture sealed under one key: whose likeness it is, who holds it, the
/// ceremony that sealed it, and the ciphertext.  Nothing here opens it.
/// It is bound to the ceremony and not to the record (design §7.5.2.6):
/// sealing happens at capture, before the record exists, and the record
/// is what the store files it under afterwards.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SealedCapture {
    /// Whose likeness it is.
    pub subject: Keyhash,
    /// Who holds it.
    pub holder: Keyhash,
    /// The ceremony that sealed it.
    pub ceremony_id: [u8; 32],
    /// Which modality was captured.
    pub modality: u64,
    /// The template version it was derived under.
    pub template_version: u64,
    /// The sealed slots, the template's first and then one per frame,
    /// each framed as one version byte, its 24-byte nonce and its
    /// ciphertext (design §7.5.2.6).  The label a slot was sealed under
    /// is its position's ([`slot_label`]), so the order is part of what
    /// authenticates.
    pub slots: Vec<Vec<u8>>,
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

/// The associated data a slot is sealed under: the ceremony's 32-byte
/// pre-commitment then the slot's ASCII label (design §7.5.2.6).
///
/// **Subject and holder are bound by the key and not here.** The capture
/// key's own derivation names both along with the ceremony
/// ([`crate::keys::capture_key`]), so what the associated data has left to
/// do is separate the slots of one capture from each other.  Modality and
/// template version sit outside both: a holder editing its own copy of
/// those reads a template under the wrong version and its own matcher
/// answers `unavailable`, which costs nobody else.
fn aad_of(ceremony_id: &[u8; 32], label: &str) -> Vec<u8> {
    let mut aad = Vec::with_capacity(32 + label.len());
    aad.extend_from_slice(ceremony_id);
    aad.extend_from_slice(label.as_bytes());
    aad
}

fn seal_slot(key: &[u8; 32], ceremony_id: &[u8; 32], i: usize, plain: &[u8]) -> Vec<u8> {
    let mut nonce = [0u8; NONCE_LEN];
    SystemRandom::new()
        .fill(&mut nonce)
        .expect("the system's random source");
    let aad = aad_of(ceremony_id, &slot_label(i));
    let ct = XChaCha20Poly1305::new(key.into())
        .encrypt(
            &XNonce::try_from(&nonce[..]).expect("24 bytes"),
            Payload {
                msg: plain,
                aad: &aad,
            },
        )
        .expect("sealing cannot fail");
    let mut out = Vec::with_capacity(1 + NONCE_LEN + ct.len());
    out.push(SLOT_VERSION);
    out.extend_from_slice(&nonce);
    out.extend_from_slice(&ct);
    out
}

fn open_slot(
    key: &[u8; 32],
    ceremony_id: &[u8; 32],
    i: usize,
    framed: &[u8],
) -> Result<Vec<u8>, OpenFailure> {
    // a version byte, a nonce and at least a tag
    if framed.len() < 1 + NONCE_LEN + 16 {
        return Err(OpenFailure::Truncated);
    }
    if framed[0] != SLOT_VERSION {
        return Err(OpenFailure::Framing);
    }
    let aad = aad_of(ceremony_id, &slot_label(i));
    XChaCha20Poly1305::new(key.into())
        .decrypt(
            &XNonce::try_from(&framed[1..1 + NONCE_LEN]).expect("24 bytes"),
            Payload {
                msg: &framed[1 + NONCE_LEN..],
                aad: &aad,
            },
        )
        .map_err(|_| OpenFailure::Unauthenticated)
}

/// Seal `capture` under `key`, one slot at a time: the template first,
/// which must be exactly the parameters' length, then each frame as its
/// instant and its bytes.  Each slot takes its own nonce and its own
/// label (design §7.5.2.6).
pub fn seal(
    p: &SealParams,
    key: &[u8; 32],
    subject: Keyhash,
    holder: Keyhash,
    ceremony_id: [u8; 32],
    capture: &Capture,
) -> SealedCapture {
    assert_eq!(
        capture.template.len(),
        p.template_len,
        "the template has the modality version's fixed length"
    );
    let mut slots = Vec::with_capacity(1 + capture.frames.len());
    slots.push(seal_slot(key, &ceremony_id, 0, &capture.template));
    for (i, f) in capture.frames.iter().enumerate() {
        let mut plain = Vec::with_capacity(8 + f.bytes.len());
        plain.extend_from_slice(&f.at_ms.to_be_bytes());
        plain.extend_from_slice(&f.bytes);
        slots.push(seal_slot(key, &ceremony_id, i + 1, &plain));
    }
    SealedCapture {
        subject,
        holder,
        ceremony_id,
        modality: capture.modality,
        template_version: capture.template_version,
        slots,
    }
}

/// Open a sealed capture under `key`.  Anything short of an authenticated,
/// well-framed capture is a decryption failure.
pub fn open(
    p: &SealParams,
    key: &[u8; 32],
    sealed: &SealedCapture,
) -> Result<Capture, OpenFailure> {
    let Some(first) = sealed.slots.first() else {
        return Err(OpenFailure::Framing);
    };
    let template = open_slot(key, &sealed.ceremony_id, 0, first)?;
    if template.len() != p.template_len {
        return Err(OpenFailure::Framing);
    }
    let mut frames = Vec::with_capacity(sealed.slots.len() - 1);
    for (i, slot) in sealed.slots.iter().enumerate().skip(1) {
        let plain = open_slot(key, &sealed.ceremony_id, i, slot)?;
        if plain.len() < 8 {
            return Err(OpenFailure::Framing);
        }
        frames.push(Frame {
            at_ms: u64::from_be_bytes(plain[..8].try_into().unwrap()),
            bytes: plain[8..].to_vec(),
        });
    }
    Ok(Capture {
        modality: sealed.modality,
        template_version: sealed.template_version,
        template,
        frames,
    })
}

/// A capture of the subject that a counterparty holds (design §7.5.2,
/// §7.5.2.9): what a grant needs to name it and derive its key.
///
/// **It holds no secret** [author, 2026-10-17]. This carried a per-ceremony
/// seed, which was the one secret in a store; design §7.5.2.6 now derives
/// the capture key from the identity root, so what is kept per capture is
/// the counterparty, the ceremony it is bound to and when the record
/// finalized — the three inputs a grant needs, none of them private.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct OwnCapture {
    /// Whose device holds the capture.
    pub counterparty: Keyhash,
    /// The ceremony the capture is bound to, which the key derives under.
    pub ceremony_id: [u8; 32],
    /// When the record finalized, which is what retention is counted from.
    pub finalized_at: u64,
}

/// A compliant client's persistent state (design §7.5.2, §7.5.2.9;
/// `light-client-requirements.md` §1.3): the sealed captures it holds of
/// others, by record; the captures of itself that others hold, by record,
/// which its identity root unlocks; the presence records it keeps; and the late
/// responses kept beside them and discarded with them; and the disclosure
/// sets of its own records, which are ordinary record state
/// (`wire-format.md` §4.5.1.2).  No plaintext likeness and no released key
/// is ever written here.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct ClientStore {
    /// The sealed captures this client holds of others, by record.
    pub sealed: BTreeMap<Txid, SealedCapture>,
    /// The captures of itself that others hold, by record: what a grant
    /// names, the key being derived rather than stored (design §7.5.2.6).
    pub own: BTreeMap<Txid, OwnCapture>,
    /// The presence records it keeps.
    pub records: BTreeMap<Txid, Vec<u8>>,
    /// Late responses attached to a record, kept and discarded with it.
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
    /// The disclosure sets of this client's own records, which are
    /// ordinary record state (`wire-format.md` §4.5.1.2).
    pub disclosures: BTreeMap<Txid, DisclosureSet>,
    /// The contacts this client keeps (`light-client-requirements.md`
    /// §4.3): every party whose locator became visible, with the material
    /// that verifies it, bounded and pruned toward what still resolves.
    pub contacts: BTreeMap<Keyhash, crate::contacts::Contact>,
    /// This client's own profiles, by subnet (`wire-format.md` §7.11):
    /// what it publishes to its serving node at attach and on change.
    pub profiles: BTreeMap<Txid, Vec<u8>>,
    /// The subnets' own profiles held, by subnet: this client's where it
    /// is the root that set one, and otherwise the latest fetched.
    pub subnet_profiles: BTreeMap<Txid, Vec<u8>>,
}

impl ClientStore {
    /// Whether `needle` appears anywhere in what the store holds: what a
    /// test asks to show that no key and no plaintext frame is at rest.
    pub fn holds_bytes(&self, needle: &[u8]) -> bool {
        let find = |hay: &[u8]| hay.windows(needle.len()).any(|w| w == needle);
        self.sealed
            .values()
            .any(|s| s.slots.iter().any(|slot| find(slot)))
            || self.records.values().any(|r| find(r))
            || self.late.values().flatten().any(|l| find(l))
    }

    /// Whether this client's own presence records name `k` as a
    /// participant: what refutes a claimed `met` (`wire-format.md` §5.6).
    pub fn has_met(&self, k: &Keyhash) -> bool {
        self.records.values().any(|r| {
            rhtn_archive::record::Record::parse(r).is_ok_and(|rec| rec.participants().contains(k))
        })
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
/// sealed captures and own-capture rows are written once and never rewritten — the
/// archive is append-only (design §13.7.1) and so is everything keyed
/// beside it. Late responses, the arrivals that could not be attached and
/// the disclosure sets accumulate against a record, so their file is
/// rewritten when it grows.
///
/// **Nothing here is secret** [author, 2026-10-17], so no file is written
/// 0600. This store held one secret, a per-ceremony capture seed; design
/// §7.5.2.6 derives the capture key from the identity root instead, so a
/// row keeps the counterparty, the ceremony id and the finalization time —
/// the three inputs a grant needs, none of them private. Sealed captures
/// are ciphertext this client cannot open and records are public by
/// construction.
///
/// **Nothing ages out on load.** design §13.7.1 is explicit that
/// age-based flushing of live data is the wrong fix and **import** is
/// where over-retention leaks, so deletion belongs to the import path and
/// not here. The retention window itself is enforced where §7.5.1 puts it:
/// a subject past it declines to release a capture key, and the holder's
/// ciphertext becomes unopenable without anyone deleting anything.
impl ClientStore {
    /// Write the store under `dir`, one file per collection.
    pub fn save(&self, dir: &std::path::Path) -> std::io::Result<()> {
        write_once(
            &dir.join("records"),
            self.records.iter().map(|(t, b)| (*t, b.clone())),
            false,
        )?;
        write_once(
            &dir.join("sealed"),
            self.sealed.iter().map(|(t, c)| (*t, encode_sealed(c))),
            false,
        )?;
        // not private: a row holds the counterparty, the ceremony id and
        // the finalization time, and the key derives from the identity
        // root (design §7.5.2.6)
        write_once(
            &dir.join("own"),
            self.own.iter().map(|(t, s)| (*t, encode_own(s))),
            false,
        )?;
        rewrite(
            &dir.join("late"),
            self.late.iter().map(|(t, v)| (*t, encode_blobs(v))),
        )?;
        rewrite(
            &dir.join("unattached"),
            self.unattached_late
                .iter()
                .map(|(t, v)| (*t, encode_keys(v))),
        )?;
        rewrite(
            &dir.join("disclosures"),
            self.disclosures
                .iter()
                .map(|(t, d)| (*t, encode_disclosures(d))),
        )?;
        rewrite(
            &dir.join("contacts"),
            self.contacts.iter().map(|(k, c)| (*k, c.encode())),
        )?;
        rewrite(
            &dir.join("profiles"),
            self.profiles.iter().map(|(k, b)| (*k, b.clone())),
        )?;
        rewrite(
            &dir.join("subnet-profiles"),
            self.subnet_profiles.iter().map(|(k, b)| (*k, b.clone())),
        )?;
        Ok(())
    }

    /// The whole store as one document, for a backup to carry
    /// (`backup`): the same six encodings the directory holds, in one
    /// place, since a blob has no directory to spread them over.
    pub fn encode(&self) -> Vec<u8> {
        use rhtn_codec::encode::*;
        let mut out = Vec::new();
        emit_array_head(&mut out, 9);
        keyed(&mut out, self.records.iter().map(|(t, b)| (*t, b.clone())));
        keyed(
            &mut out,
            self.sealed.iter().map(|(t, c)| (*t, encode_sealed(c))),
        );
        keyed(&mut out, self.own.iter().map(|(t, s)| (*t, encode_own(s))));
        keyed(
            &mut out,
            self.late.iter().map(|(t, v)| (*t, encode_blobs(v))),
        );
        keyed(
            &mut out,
            self.unattached_late
                .iter()
                .map(|(t, v)| (*t, encode_keys(v))),
        );
        keyed(
            &mut out,
            self.disclosures
                .iter()
                .map(|(t, d)| (*t, encode_disclosures(d))),
        );
        keyed(
            &mut out,
            self.contacts.iter().map(|(k, c)| (*k, c.encode())),
        );
        keyed(&mut out, self.profiles.iter().map(|(k, b)| (*k, b.clone())));
        keyed(
            &mut out,
            self.subnet_profiles.iter().map(|(k, b)| (*k, b.clone())),
        );
        out
    }

    /// One back.  **Nothing partial**: a store that decoded five of six
    /// maps is not a store, and returning one would be the half-import a
    /// restore must not perform.
    pub fn decode(b: &[u8]) -> Option<ClientStore> {
        let item = rhtn_codec::cbor::parse_all(b).ok()?;
        let rhtn_codec::cbor::Item::Array(f) = &item else {
            return None;
        };
        // six collections before locators were kept (2026-10-11), seven
        // since, nine with profiles (2026-10-15): a store written by an
        // earlier shape opens with none of what it did not hold
        if !matches!(f.len(), 6 | 7 | 9) {
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
            st.own.insert(t, decode_own(&v)?);
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
        if f.len() >= 7 {
            // a seventh collection that held bare locators (2026-10-11,
            // for a day) reads as no contacts: nothing there verified
            for (k, v) in m(6)? {
                if let Some(c) = crate::contacts::Contact::decode(&v) {
                    st.contacts.insert(k, c);
                }
            }
        }
        if f.len() == 9 {
            for (k, v) in m(7)? {
                st.profiles.insert(k, v);
            }
            for (k, v) in m(8)? {
                st.subnet_profiles.insert(k, v);
            }
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
        for (t, b) in read_dir_of(&dir.join("own"))? {
            if let Some(s) = decode_own(&b) {
                st.own.insert(t, s);
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
        for (k, b) in read_dir_of(&dir.join("contacts"))? {
            if let Some(c) = crate::contacts::Contact::decode(&b) {
                st.contacts.insert(k, c);
            }
        }
        for (k, b) in read_dir_of(&dir.join("profiles"))? {
            st.profiles.insert(k, b);
        }
        for (k, b) in read_dir_of(&dir.join("subnet-profiles"))? {
            st.subnet_profiles.insert(k, b);
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
fn write_once(
    root: &std::path::Path,
    items: impl Iterator<Item = (Txid, Vec<u8>)>,
    private: bool,
) -> std::io::Result<()> {
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
fn rewrite(
    root: &std::path::Path,
    items: impl Iterator<Item = (Txid, Vec<u8>)>,
) -> std::io::Result<()> {
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
    let Ok(rd) = std::fs::read_dir(root) else {
        return Ok(out);
    };
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
    emit_array_head(&mut out, 6);
    emit_bstr(&mut out, &c.subject);
    emit_bstr(&mut out, &c.holder);
    emit_bstr(&mut out, &c.ceremony_id);
    emit_uint(&mut out, c.modality);
    emit_uint(&mut out, c.template_version);
    emit_array_head(&mut out, c.slots.len());
    for slot in &c.slots {
        emit_bstr(&mut out, slot);
    }
    out
}

fn decode_sealed(b: &[u8]) -> Option<SealedCapture> {
    let a = array_of(b, 6)?;
    let rhtn_codec::cbor::Item::Array(slots) = &a[5] else {
        return None;
    };
    Some(SealedCapture {
        subject: kh_at(b, &a[0])?,
        holder: kh_at(b, &a[1])?,
        ceremony_id: kh_at(b, &a[2])?,
        modality: uint_at(&a[3])?,
        template_version: uint_at(&a[4])?,
        slots: slots
            .iter()
            .map(|it| bytes_at(b, it))
            .collect::<Option<Vec<_>>>()?,
    })
}

fn encode_own(s: &OwnCapture) -> Vec<u8> {
    use rhtn_codec::encode::*;
    let mut out = Vec::new();
    emit_array_head(&mut out, 3);
    emit_bstr(&mut out, &s.counterparty);
    emit_bstr(&mut out, &s.ceremony_id);
    emit_uint(&mut out, s.finalized_at);
    out
}

fn decode_own(b: &[u8]) -> Option<OwnCapture> {
    let a = array_of(b, 3)?;
    Some(OwnCapture {
        counterparty: kh_at(b, &a[0])?,
        ceremony_id: kh_at(b, &a[1])?,
        finalized_at: uint_at(&a[2])?,
    })
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
    let rhtn_codec::cbor::Item::Array(a) = &item else {
        return None;
    };
    a.iter().map(|x| bytes_at(b, x)).collect()
}

fn encode_keys(v: &[Keyhash]) -> Vec<u8> {
    encode_blobs(&v.iter().map(|k| k.to_vec()).collect::<Vec<_>>())
}

fn decode_keys(b: &[u8]) -> Option<Vec<Keyhash>> {
    decode_blobs(b)?
        .into_iter()
        .map(|v| v.try_into().ok())
        .collect()
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
        let rhtn_codec::cbor::Item::Array(f) = item else {
            return None;
        };
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
    let rhtn_codec::cbor::Item::Array(a) = &item else {
        return None;
    };
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
    let rhtn_codec::cbor::Item::Array(a) = it else {
        return None;
    };
    a.iter()
        .map(|x| {
            let rhtn_codec::cbor::Item::Array(p) = x else {
                return None;
            };
            match p.as_slice() {
                [t, v] => Some((kh_at(b, t)?, bytes_at(b, v)?)),
                _ => None,
            }
        })
        .collect()
}
