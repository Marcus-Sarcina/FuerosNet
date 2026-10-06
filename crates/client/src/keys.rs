//! The three constructions a ceremony fixes to the byte (design §7.5.2.6,
//! `wire-format.md` §14.3.2): the contributory pre-commitment both parties
//! compute, the capture key a subject derives for one holder's sealed
//! capture of them, and the key the local session is encrypted under.  Both
//! clients must produce the same bytes, so each is stated exactly and
//! checked against the vectors' known answers.
//!
//! **The pre-commitment and the session key share their inputs and must
//! not share a construction.** Both are over the two contributions, but one
//! names a value every carriage carries in the open and the other is a
//! secret; a single label would make the public one a distinguisher for the
//! secret. The labels are therefore distinct and that is the whole reason
//! they are [author, 2026-10-05].

use crate::Keyhash;
use aws_lc_rs::hkdf;
use aws_lc_rs::rand::{SecureRandom, SystemRandom};
use rhtn_codec::cose::sha256;
use zeroize::Zeroizing;

/// The ASCII tag the pre-commitment hashes first.
pub const CEREMONY_TAG: &[u8] = b"rhtn/1:ceremony";
/// The ASCII tag the capture key's info begins with.
pub const CAPTURE_TAG: &[u8] = b"rhtn/1:capture";
/// The ASCII tag the local session's key hashes first (`wire-format.md`
/// §14.3.2).  Distinct from [`CEREMONY_TAG`] over the same two
/// contributions: see this module's header.
pub const SESSION_TAG: &[u8] = b"rhtn/1:ceremony-session";

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

/// The local session's key (`wire-format.md` §14.3.2): SHA-256 of the
/// session tag followed by the two 16-byte contributions in the same
/// ascending participant-keyhash order the pre-commitment uses.
///
/// **The contributions are the secret and the ceremony-id is not.** Each
/// crosses one screen and nowhere else, while the pre-commitment derived
/// from them is shown as the second code and quoted by every carriage; a
/// party that read neither screen holds neither contribution and cannot
/// derive this. Which is what makes the key worth having: it closes the
/// gap between radio range and the range of reading a phone screen, and
/// nothing more. A co-present adversary that read both codes has it, and
/// `wire-format.md` §14.3.1's physical premise is still the whole of the
/// resistance past that point.
///
/// **One screen is not enough**, which is why both contributions are
/// inputs rather than the pre-commitment being reused:
/// `models/tamarin/local/exchange.spthy` proves that leaking one leaves
/// both the confidentiality and the integrity of a carriage standing.
pub fn session_key(a: (&Keyhash, &[u8; 16]), b: (&Keyhash, &[u8; 16])) -> Zeroizing<[u8; 32]> {
    let (first, second) = if a.0 <= b.0 { (a.1, b.1) } else { (b.1, a.1) };
    let mut pre = Zeroizing::new(Vec::with_capacity(SESSION_TAG.len() + 32));
    pre.extend_from_slice(SESSION_TAG);
    pre.extend_from_slice(first);
    pre.extend_from_slice(second);
    Zeroizing::new(sha256(&pre))
}

/// The capture key (design §7.5.2.6): HKDF-SHA-256 with an empty salt, the
/// subject's seed as the keying material, and as info the tag followed by
/// the raw subject and holder keyhashes and the ceremony pre-commitment,
/// 32 bytes out.  Bound to subject, holder and ceremony, so a key released
/// to one holder opens nobody else's copy and no later ceremony's.
pub fn capture_key(
    seed: &[u8; 32],
    subject: &Keyhash,
    holder: &Keyhash,
    ceremony_id: &[u8; 32],
) -> Zeroizing<[u8; 32]> {
    let mut info = Vec::with_capacity(CAPTURE_TAG.len() + 96);
    info.extend_from_slice(CAPTURE_TAG);
    info.extend_from_slice(subject);
    info.extend_from_slice(holder);
    info.extend_from_slice(ceremony_id);
    let prk = hkdf::Salt::new(hkdf::HKDF_SHA256, &[]).extract(seed);
    // wiped when dropped, wherever it is carried to: the key decrypts a
    // likeness of a person, and no holder keeps it past its use
    let mut out = Zeroizing::new([0u8; 32]);
    prk.expand(&[&info], hkdf::HKDF_SHA256)
        .expect("32 bytes is within HKDF's bound")
        .fill(&mut out[..])
        .expect("filled");
    out
}

/// A subject's seed for one ceremony: 32 random bytes only the subject
/// holds (design §7.5.2), kept in its own record of the transaction.
pub fn random_seed() -> [u8; 32] {
    let mut s = [0u8; 32];
    SystemRandom::new()
        .fill(&mut s)
        .expect("the system's random source");
    s
}

/// A participant's contribution to the pre-commitment: 16 random bytes.
pub fn random_contribution() -> [u8; 16] {
    let mut c = [0u8; 16];
    SystemRandom::new()
        .fill(&mut c)
        .expect("the system's random source");
    c
}

#[cfg(test)]
mod tests {
    use super::*;

    fn kh(b: u8) -> Keyhash {
        [b; 32]
    }

    /// **The two constructions over the same inputs must not agree.** The
    /// pre-commitment is public and the session key is secret; if one label
    /// were reused the public value would be the secret.
    #[test]
    fn the_session_key_is_not_the_pre_commitment() {
        let (a, b) = ((kh(1), [7u8; 16]), (kh(2), [9u8; 16]));
        let pre = pre_commitment((&a.0, &a.1), (&b.0, &b.1));
        let key = session_key((&a.0, &a.1), (&b.0, &b.1));
        assert_ne!(pre, *key, "a shared label would publish the session key");
    }

    /// Both sides compute one value whichever order they hold the pair in,
    /// which is what the keyhash ordering is for.
    #[test]
    fn the_session_key_does_not_depend_on_the_caller_s_order() {
        let (a, b) = ((kh(1), [7u8; 16]), (kh(2), [9u8; 16]));
        assert_eq!(
            *session_key((&a.0, &a.1), (&b.0, &b.1)),
            *session_key((&b.0, &b.1), (&a.0, &a.1))
        );
    }

    /// Either contribution changing changes the key, so one party cannot
    /// fix it alone.
    #[test]
    fn either_contribution_changes_the_session_key() {
        let (a, b) = ((kh(1), [7u8; 16]), (kh(2), [9u8; 16]));
        let base = session_key((&a.0, &a.1), (&b.0, &b.1));
        assert_ne!(*base, *session_key((&a.0, &[8u8; 16]), (&b.0, &b.1)));
        assert_ne!(*base, *session_key((&a.0, &a.1), (&b.0, &[8u8; 16])));
    }
}
