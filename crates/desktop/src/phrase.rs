//! **A desktop restored from the phrase** [author, 2026-11-18] (design
//! §23.3): the words back as the identity, so the desktop is a signing
//! device of it as the phone is, for the desktop the phone never shares a
//! network with.
//!
//! **The same phrase as the phone's, to the bit** (its `Identity.kt`):
//! BIP-39's shape — 24 words from the standard's list of 2048, 32 bytes of
//! entropy and an 8-bit checksum off its SHA-256 — and the kernel's two
//! seeds derived from that master by HKDF-SHA256 with no salt under the
//! label `fueros identity seeds v1`, 64 bytes out.  Both are held to one
//! known answer computed apart from either.  **Above the kernel**, as
//! `light-client-requirements.md` §9 has a phrase: the kernel starts on
//! the 64 bytes, and nothing below this file knows a phrase was typed.

use aws_lc_rs::hkdf;
use zeroize::Zeroizing;

/// Words in a phrase.
pub const WORDS: usize = 24;

/// The label the two seeds derive under; changing it is a new identity.
const SEEDS_INFO: &[u8] = b"fueros identity seeds v1";

/// The standard's list, the file the phone reads too.
const LIST: &str = include_str!("../../mobile/android/app/src/main/assets/bip39/english.txt");

fn list() -> Vec<&'static str> {
    LIST.lines()
        .map(str::trim)
        .filter(|w| !w.is_empty())
        .collect()
}

/// **The kernel's seeds from what was typed**, or why not: a count other
/// than 24, a word not on the list by its position, or a checksum that
/// does not hold — a phrase typed wrong is fixed by knowing where.
pub fn seeds(typed: &str) -> Result<Zeroizing<Vec<u8>>, String> {
    let words = list();
    let given: Vec<String> = typed.split_whitespace().map(|w| w.to_lowercase()).collect();
    if given.len() != WORDS {
        return Err(format!(
            "a phrase is {WORDS} words; that is {}",
            given.len()
        ));
    }
    let mut idx = Vec::with_capacity(WORDS);
    let mut unknown = Vec::new();
    for (i, w) in given.iter().enumerate() {
        match words.binary_search(&w.as_str()) {
            Ok(n) => idx.push(n as u32),
            Err(_) => unknown.push(format!("{}: {w}", i + 1)),
        }
    }
    if !unknown.is_empty() {
        return Err(format!("not on the list — {}", unknown.join(", ")));
    }
    // 24 × 11 = 264 bits: 256 of entropy, then 8 of checksum
    let mut bits = Zeroizing::new(Vec::with_capacity(264));
    for n in &idx {
        for k in (0..11).rev() {
            bits.push((n >> k) & 1 == 1);
        }
    }
    let byte = |at: usize| (0..8).fold(0u8, |b, k| (b << 1) | u8::from(bits[at + k]));
    let master = Zeroizing::new((0..32).map(|i| byte(i * 8)).collect::<Vec<u8>>());
    if rhtn_codec::cose::sha256(&master)[0] != byte(256) {
        return Err("the words do not check: one of them is wrong, or two are swapped".into());
    }
    Ok(derive(&master))
}

/// The two seeds from the 32-byte master: HKDF-SHA256, no salt, the
/// fixed label, 64 bytes.
fn derive(master: &[u8]) -> Zeroizing<Vec<u8>> {
    struct Len64;
    impl hkdf::KeyType for Len64 {
        fn len(&self) -> usize {
            64
        }
    }
    let prk = hkdf::Salt::new(hkdf::HKDF_SHA256, &[]).extract(master);
    let mut out = Zeroizing::new(vec![0u8; 64]);
    prk.expand(&[SEEDS_INFO], Len64)
        .and_then(|okm| okm.fill(&mut out))
        .expect("64 bytes is within HKDF-SHA256's reach");
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The standard's first vector, read through: 32 zero bytes are
    /// "abandon" twenty-three times and "art".
    fn zero_phrase() -> String {
        let mut w = vec!["abandon"; 23];
        w.push("art");
        w.join(" ")
    }

    #[test]
    fn the_list_is_the_standards_and_sorted() {
        let w = list();
        assert_eq!(w.len(), 2048);
        assert!(w.windows(2).all(|p| p[0] < p[1]), "searched by halving");
    }

    // functional: OPS-019
    #[test]
    fn the_seeds_are_the_phones_to_the_bit() {
        // the known answer the phone's IdentityTest holds too, computed
        // apart from both (RFC 5869 over HMAC-SHA256)
        let master: Vec<u8> = (0..32).collect();
        let hex: String = derive(&master).iter().map(|b| format!("{b:02x}")).collect();
        assert_eq!(
            hex,
            "5ce51bf7091a7288ab3642a5ba655c147a531a87bab674b62e448ca8bb4df15a\
             f5b3e2b796ee3793b021b9e591840a45a7c7aecc0fb67a9065cb790a4d1c7d08"
        );
        assert_eq!(*seeds(&zero_phrase()).unwrap(), *derive(&[0u8; 32]));
        assert_eq!(
            *seeds(&format!("  {}  ", zero_phrase().to_uppercase())).unwrap(),
            *derive(&[0u8; 32]),
            "spacing and case are not the phrase"
        );
    }

    #[test]
    fn a_phrase_typed_wrong_says_where() {
        assert!(seeds("abandon art").unwrap_err().contains("that is 2"));
        let mut w = vec!["abandon"; 23];
        w.push("zzzz");
        assert!(seeds(&w.join(" ")).unwrap_err().contains("24: zzzz"));
        let mut w = vec!["abandon"; 23];
        w.push("ability");
        assert!(seeds(&w.join(" ")).unwrap_err().contains("do not check"));
    }
}
