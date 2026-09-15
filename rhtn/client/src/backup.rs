//! Backup: envelope encryption over everything a participant loses with
//! the device (design §13.7.1).
//!
//! **The blob is the aggregation the design avoids everywhere else** —
//! every subnet key plus photographs of everyone the user has met, in one
//! file — so it is encrypted under a key not stored alongside it. The
//! construction is the standard one §13.7.1 names and nothing invented: a
//! random data key encrypts the payload, a key-encryption key derived from
//! a passphrase encrypts the data key, and the KEK is never written.
//!
//! **How the KEK is protected is a seam, deliberately.** §13.7.1:
//! *"passphrase in v1, hardware token later, split shares later still —
//! none of which changes the format"*. So [`Wrap`] names the method and
//! the header carries it, and a reader that meets a method it does not
//! implement says so rather than guessing.
//!
//! **What this is not.** Nothing here splits the KEK among counterparties.
//! §13.7.1 costs that honestly — plain Shamir is not verifiable, social
//! recovery has a poor practical record, and share holders would have to
//! be disjoint from the §9.1 quorum — and leaves it for later. A
//! `Wrap` variant is where it would go.

use crate::store::ClientStore;
use aws_lc_rs::aead::{AES_256_GCM, Aad, LessSafeKey, Nonce, UnboundKey};
use aws_lc_rs::rand::{SecureRandom, SystemRandom};
use rhtn_codec::cbor::{Item, parse_all};
use rhtn_codec::encode::*;

/// This format's version.  A reader refuses what it does not know rather
/// than reading a later format as though it were this one.
pub const VERSION: u64 = 1;

/// The external_aad both AEAD steps carry, so a wrapped key lifted from
/// one backup does not open another's payload.
const AAD_WRAP: &[u8] = b"rhtn/1:backup-wrap";
const AAD_BODY: &[u8] = b"rhtn/1:backup-body";

/// Argon2id's cost, which nobody has stated.
///
/// **The operator's numbers** (design §21.1's posture for every such
/// value), carried in the header because a reader must use what the writer
/// used. The defaults are the RFC 9106 second recommendation — 64 MiB,
/// three passes — chosen as a value a phone can afford and recorded as
/// chosen rather than derived.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Cost {
    pub m_kib: u32,
    pub passes: u32,
    pub lanes: u32,
}

impl Default for Cost {
    fn default() -> Self {
        Cost { m_kib: 65_536, passes: 3, lanes: 1 }
    }
}

/// How the key-encryption key is derived from what the user supplies.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Wrap {
    /// Argon2id over a passphrase (design §13.7.1's named KDF: memory-hard,
    /// which raises the cost of GPU and specialised-hardware attack in
    /// proportion to the parameters chosen rather than preventing it).
    Passphrase { salt: [u8; 16], cost: Cost },
}

impl Wrap {
    /// A fresh passphrase wrap at the given cost.
    pub fn passphrase(cost: Cost) -> Wrap {
        let mut salt = [0u8; 16];
        SystemRandom::new().fill(&mut salt).expect("the system's random source");
        Wrap::Passphrase { salt, cost }
    }

    /// The key-encryption key this wrap derives from `secret`.
    fn kek(&self, secret: &[u8]) -> Result<[u8; 32], Failure> {
        match self {
            Wrap::Passphrase { salt, cost } => {
                let params = argon2::Params::new(cost.m_kib, cost.passes, cost.lanes, Some(32)).map_err(|_| Failure::Cost)?;
                let a = argon2::Argon2::new(argon2::Algorithm::Argon2id, argon2::Version::V0x13, params);
                let mut out = [0u8; 32];
                a.hash_password_into(secret, salt, &mut out).map_err(|_| Failure::Cost)?;
                Ok(out)
            }
        }
    }
}

/// What a backup holds: everything a device loss takes away.
///
/// **The identity is in here and not in the store** (`ClientStore` writes
/// no key): a backup is where the two are deliberately together, which is
/// exactly why it is enveloped.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Contents {
    /// The two seeds an identity is derived from, classical then
    /// post-quantum, as `rhtn keys` writes them.
    pub seeds: Option<[[u8; 32]; 2]>,
    /// The archive's records, as bytes.
    pub records: Vec<Vec<u8>>,
    pub store: ClientStore,
}

/// Why a backup did not open.  **Each names which step failed**, because
/// a wrong passphrase and a tampered payload are different things to tell
/// a person, and both are authentication failures.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Failure {
    /// Not this format, or a later one.
    Version(u64),
    /// A wrap method this reader does not implement.
    UnsupportedWrap(u64),
    /// The bytes are not a backup: truncated, or not the shape.
    Malformed(&'static str),
    /// The data key did not unwrap: the secret is wrong, or the header or
    /// the wrapped key was altered.
    Secret,
    /// The data key unwrapped and the payload did not authenticate: the
    /// ciphertext was altered.
    Payload,
    /// The payload opened and is not a backup's contents.
    Contents,
    /// The cost parameters are ones this reader cannot run.
    Cost,
}

impl std::fmt::Display for Failure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Failure::Version(v) => write!(f, "backup version {v} is not one this reader knows"),
            Failure::UnsupportedWrap(k) => write!(f, "wrap method {k} is not one this reader implements"),
            Failure::Malformed(w) => write!(f, "not a backup: {w}"),
            Failure::Secret => write!(f, "the data key did not unwrap: wrong secret, or an altered header"),
            Failure::Payload => write!(f, "the payload did not authenticate: altered ciphertext"),
            Failure::Contents => write!(f, "the payload opened and is not a backup's contents"),
            Failure::Cost => write!(f, "the cost parameters are ones this reader cannot run"),
        }
    }
}

const WRAP_PASSPHRASE: u64 = 0;

/// Write a backup: a random data key over the payload, the data key
/// wrapped under the KEK `wrap` derives from `secret`, and the KEK kept
/// nowhere.
pub fn export(contents: &Contents, wrap: &Wrap, secret: &[u8]) -> Result<Vec<u8>, Failure> {
    let rng = SystemRandom::new();
    let mut data_key = [0u8; 32];
    let (mut wrap_nonce, mut body_nonce) = ([0u8; 12], [0u8; 12]);
    for b in [&mut data_key[..], &mut wrap_nonce[..], &mut body_nonce[..]] {
        rng.fill(b).expect("the system's random source");
    }

    let header = encode_header(wrap, &wrap_nonce, &body_nonce);
    // the header is the wrap's aad, so altering the cost or the salt after
    // the fact makes the data key refuse to unwrap rather than silently
    // deriving a different one
    let mut wrapped = data_key.to_vec();
    key(&wrap.kek(secret)?)
        .seal_in_place_append_tag(Nonce::assume_unique_for_key(wrap_nonce), Aad::from(aad(AAD_WRAP, &header)), &mut wrapped)
        .map_err(|_| Failure::Secret)?;

    let mut body = encode_contents(contents);
    key(&data_key)
        .seal_in_place_append_tag(Nonce::assume_unique_for_key(body_nonce), Aad::from(aad(AAD_BODY, &header)), &mut body)
        .map_err(|_| Failure::Payload)?;

    let mut out = Vec::new();
    emit_array_head(&mut out, 3);
    emit_bstr(&mut out, &header);
    emit_bstr(&mut out, &wrapped);
    emit_bstr(&mut out, &body);
    Ok(out)
}

/// Read one back.  **Nothing is returned from a backup that did not
/// authenticate whole**: a partial import is how a restore replaces an
/// intact identity with half of an old one.
pub fn import(blob: &[u8], secret: &[u8]) -> Result<Contents, Failure> {
    let item = parse_all(blob).map_err(|_| Failure::Malformed("does not parse"))?;
    let Item::Array(parts) = &item else { return Err(Failure::Malformed("not an array")) };
    let [h, w, b] = parts.as_slice() else { return Err(Failure::Malformed("not three fields")) };
    let (header, wrapped, body) = (raw(blob, h)?, raw(blob, w)?, raw(blob, b)?);
    let (wrap, wrap_nonce, body_nonce) = decode_header(&header)?;

    let mut key_buf = wrapped.clone();
    let opened = key(&wrap.kek(secret)?)
        .open_in_place(Nonce::assume_unique_for_key(wrap_nonce), Aad::from(aad(AAD_WRAP, &header)), &mut key_buf)
        .map_err(|_| Failure::Secret)?;
    let data_key: [u8; 32] = opened.try_into().map_err(|_| Failure::Secret)?;

    let mut body_buf = body.clone();
    let plain = key(&data_key)
        .open_in_place(Nonce::assume_unique_for_key(body_nonce), Aad::from(aad(AAD_BODY, &header)), &mut body_buf)
        .map_err(|_| Failure::Payload)?;
    decode_contents(plain).ok_or(Failure::Contents)
}

/// The header a reader needs and nothing that opens it.
fn encode_header(wrap: &Wrap, wrap_nonce: &[u8; 12], body_nonce: &[u8; 12]) -> Vec<u8> {
    let mut out = Vec::new();
    match wrap {
        Wrap::Passphrase { salt, cost } => {
            emit_array_head(&mut out, 7);
            emit_uint(&mut out, VERSION);
            emit_uint(&mut out, WRAP_PASSPHRASE);
            emit_bstr(&mut out, salt);
            emit_uint(&mut out, cost.m_kib as u64);
            emit_uint(&mut out, cost.passes as u64);
            emit_uint(&mut out, cost.lanes as u64);
            let mut nonces = Vec::new();
            nonces.extend_from_slice(wrap_nonce);
            nonces.extend_from_slice(body_nonce);
            emit_bstr(&mut out, &nonces);
        }
    }
    out
}

fn decode_header(b: &[u8]) -> Result<(Wrap, [u8; 12], [u8; 12]), Failure> {
    let item = parse_all(b).map_err(|_| Failure::Malformed("header does not parse"))?;
    let Item::Array(f) = &item else { return Err(Failure::Malformed("header not an array")) };
    let version = f.first().and_then(uint).ok_or(Failure::Malformed("no version"))?;
    if version != VERSION {
        return Err(Failure::Version(version));
    }
    let method = f.get(1).and_then(uint).ok_or(Failure::Malformed("no wrap method"))?;
    if method != WRAP_PASSPHRASE {
        return Err(Failure::UnsupportedWrap(method));
    }
    if f.len() != 7 {
        return Err(Failure::Malformed("passphrase header is seven fields"));
    }
    let salt: [u8; 16] = raw(b, &f[2])?.try_into().map_err(|_| Failure::Malformed("salt is not 16 bytes"))?;
    let cost = Cost {
        m_kib: uint(&f[3]).and_then(|v| u32::try_from(v).ok()).ok_or(Failure::Malformed("memory cost"))?,
        passes: uint(&f[4]).and_then(|v| u32::try_from(v).ok()).ok_or(Failure::Malformed("passes"))?,
        lanes: uint(&f[5]).and_then(|v| u32::try_from(v).ok()).ok_or(Failure::Malformed("lanes"))?,
    };
    let nonces = raw(b, &f[6])?;
    if nonces.len() != 24 {
        return Err(Failure::Malformed("nonces are two of twelve bytes"));
    }
    let (w, y) = nonces.split_at(12);
    Ok((Wrap::Passphrase { salt, cost }, w.try_into().unwrap(), y.try_into().unwrap()))
}

fn encode_contents(c: &Contents) -> Vec<u8> {
    let mut out = Vec::new();
    emit_array_head(&mut out, 3);
    match &c.seeds {
        Some([a, b]) => {
            emit_array_head(&mut out, 2);
            emit_bstr(&mut out, a);
            emit_bstr(&mut out, b);
        }
        None => emit_array_head(&mut out, 0),
    }
    emit_array_head(&mut out, c.records.len());
    for r in &c.records {
        emit_bstr(&mut out, r);
    }
    emit_bstr(&mut out, &c.store.encode());
    out
}

fn decode_contents(b: &[u8]) -> Option<Contents> {
    let item = parse_all(b).ok()?;
    let Item::Array(f) = &item else { return None };
    let [s, r, st] = f.as_slice() else { return None };
    let Item::Array(sa) = s else { return None };
    let seeds = match sa.as_slice() {
        [] => None,
        [a, b2] => Some([raw(b, a).ok()?.try_into().ok()?, raw(b, b2).ok()?.try_into().ok()?]),
        _ => return None,
    };
    let Item::Array(ra) = r else { return None };
    let records: Vec<Vec<u8>> = ra.iter().map(|x| raw(b, x).ok()).collect::<Option<_>>()?;
    Some(Contents { seeds, records, store: ClientStore::decode(&raw(b, st).ok()?)? })
}

fn key(k: &[u8; 32]) -> LessSafeKey {
    LessSafeKey::new(UnboundKey::new(&AES_256_GCM, k).expect("a 32-byte key"))
}

/// The tag and the header together: a wrapped key or a payload lifted from
/// one backup does not open another's, and an altered header is an
/// authentication failure rather than a different derivation.
fn aad(tag: &[u8], header: &[u8]) -> Vec<u8> {
    let mut out = tag.to_vec();
    out.extend_from_slice(header);
    out
}

fn raw(b: &[u8], it: &Item) -> Result<Vec<u8>, Failure> {
    match it {
        Item::Bytes(r) => Ok(b[r.clone()].to_vec()),
        _ => Err(Failure::Malformed("expected a byte string")),
    }
}

fn uint(it: &Item) -> Option<u64> {
    match it {
        Item::Uint(n) => Some(*n),
        _ => None,
    }
}
