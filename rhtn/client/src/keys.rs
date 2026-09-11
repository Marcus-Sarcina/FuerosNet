//! The two constructions a ceremony fixes to the byte (design §7.5.2.6): the
//! contributory pre-commitment both parties compute, and the capture key a
//! subject derives for one holder's sealed capture of them.  Both clients
//! must produce the same bytes, so each is stated exactly and checked
//! against the vectors' known answers.

use crate::Keyhash;
use aws_lc_rs::hkdf;
use aws_lc_rs::rand::{SecureRandom, SystemRandom};
use rhtn_codec::cose::sha256;

/// The ASCII tag the pre-commitment hashes first.
pub const CEREMONY_TAG: &[u8] = b"rhtn/1:ceremony";
/// The ASCII tag the capture key's info begins with.
pub const CAPTURE_TAG: &[u8] = b"rhtn/1:capture";

/// The ceremony pre-commitment (design §7.5.2.6): SHA-256 of the tag
/// followed by the two participants' 16-byte contributions in ascending
/// participant-keyhash order.  Either party's honest randomness makes it
/// unique, so neither can force a repeat.
pub fn pre_commitment(a: (&Keyhash, &[u8; 16]), b: (&Keyhash, &[u8; 16])) -> [u8; 32] {
    let (first, second) = if a.0 <= b.0 { (a.1, b.1) } else { (b.1, a.1) };
    let mut pre = Vec::with_capacity(CEREMONY_TAG.len() + 32);
    pre.extend_from_slice(CEREMONY_TAG);
    pre.extend_from_slice(first);
    pre.extend_from_slice(second);
    sha256(&pre)
}

/// The capture key (design §7.5.2.6): HKDF-SHA-256 with an empty salt, the
/// subject's seed as the keying material, and as info the tag followed by
/// the raw subject and holder keyhashes and the ceremony pre-commitment,
/// 32 bytes out.  Bound to subject, holder and ceremony, so a key released
/// to one holder opens nobody else's copy and no later ceremony's.
pub fn capture_key(seed: &[u8; 32], subject: &Keyhash, holder: &Keyhash, ceremony_id: &[u8; 32]) -> [u8; 32] {
    let mut info = Vec::with_capacity(CAPTURE_TAG.len() + 96);
    info.extend_from_slice(CAPTURE_TAG);
    info.extend_from_slice(subject);
    info.extend_from_slice(holder);
    info.extend_from_slice(ceremony_id);
    let prk = hkdf::Salt::new(hkdf::HKDF_SHA256, &[]).extract(seed);
    let mut out = [0u8; 32];
    prk.expand(&[&info], hkdf::HKDF_SHA256).expect("32 bytes is within HKDF's bound").fill(&mut out).expect("filled");
    out
}

/// A subject's seed for one ceremony: 32 random bytes only the subject
/// holds (design §7.5.2), kept in its own record of the transaction.
pub fn random_seed() -> [u8; 32] {
    let mut s = [0u8; 32];
    SystemRandom::new().fill(&mut s).expect("the system's random source");
    s
}

/// A participant's contribution to the pre-commitment: 16 random bytes.
pub fn random_contribution() -> [u8; 16] {
    let mut c = [0u8; 16];
    SystemRandom::new().fill(&mut c).expect("the system's random source");
    c
}
