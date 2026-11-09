//! **The primitives PQXDH is instantiated with, against the vectors their
//! own specifications publish** (PAY-13; design §14.2.4.2).
//!
//! **Why these and not the composition.** PQXDH and the Double Ratchet are
//! adopted rather than designed here (design §14.2.4), and their
//! specifications are prose with formal analysis — neither publishes
//! known-answer vectors. The primitives they are instantiated with do, and
//! those are the part of *"matching the adopted specifications"* that can
//! be checked against somebody else's arithmetic rather than against our
//! own [author, 2026-11-08]. The composition is held by this project's own
//! canonical vectors and its independent harness, which is the bar as the
//! author restated it on 2026-10-02.
//!
//! **The hex below is published, not computed here.** That is the whole
//! point of it: a vector this project produced would only say that the
//! implementation agrees with itself.

use rhtn_crypto::pqxdh::{DhPublic, DhSecret, KemSecret, hkdf_sha256};

fn hex(s: &str) -> Vec<u8> {
    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&s[i..i + 2], 16).expect("hex"))
        .collect()
}

fn h32(s: &str) -> [u8; 32] {
    hex(s).try_into().expect("32 bytes")
}

/// **RFC 7748 §6.1**, the X25519 Diffie-Hellman test vector: Alice's and
/// Bob's private and public keys, and the shared secret both sides reach.
///
/// Published in the RFC that specifies the curve, which is what makes it
/// evidence: it is the same arithmetic every other implementation is
/// checked against.
#[test]
fn x25519_agrees_with_rfc_7748_section_6_1() {
    let a_priv = h32("77076d0a7318a57d3c16c17251b26645df4c2f87ebc0992ab177fba51db92c2a");
    let a_pub = h32("8520f0098930a754748b7ddcb43ef75a0dbf3a0d26381af4eba4a98eaa9b4e6a");
    let b_priv = h32("5dab087e624a8a4b79e17f8b83800ee66f3bb1292618b6fd1c2f8b27ff88e0eb");
    let b_pub = h32("de9edb7d7b7dc1b4d35b61c2ece435373f8343c85b78674dadfc7e146f882b4f");
    let shared = h32("4a5d9d5ba4ce2de1728e3bf480350f25e07e21c947d19e3376f09b3c1e161742");

    let alice = DhSecret::from_seed(a_priv);
    let bob = DhSecret::from_seed(b_priv);
    assert_eq!(alice.public().0, a_pub, "Alice's public key, RFC 7748 §6.1");
    assert_eq!(bob.public().0, b_pub, "Bob's public key, RFC 7748 §6.1");

    // **and both sides reach it**, which is the property the agreement is
    // used for rather than one direction of the arithmetic
    assert_eq!(
        alice.agree(&DhPublic(b_pub)),
        shared,
        "the shared secret, RFC 7748 §6.1"
    );
    assert_eq!(
        bob.agree(&DhPublic(a_pub)),
        shared,
        "and from the other side"
    );
}

/// **RFC 5869 Appendix A.1**, the HKDF-SHA-256 test case the KDF is built
/// on: a 22-byte IKM, a 13-byte salt, a 10-byte info and 42 bytes out.
#[test]
fn hkdf_sha256_agrees_with_rfc_5869_case_1() {
    let ikm = hex("0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b");
    let salt = hex("000102030405060708090a0b0c");
    let info = hex("f0f1f2f3f4f5f6f7f8f9");
    let want = hex("3cb25f25faacd57a90434f64d0362f2a\
         2d2d0a90cf1a5a4c5db02d56ecc4c5bf\
         34007208d5b887185865");
    let mut out = vec![0u8; want.len()];
    hkdf_sha256(&salt, &ikm, &info, &mut out);
    assert_eq!(out, want, "OKM, RFC 5869 A.1");
}

/// **FIPS 203's parameter set for ML-KEM-768**, which is what §14.2.4.2
/// names: the sizes are what tell 768 from 512 and 1024, and getting the
/// parameter set wrong is the failure this catches.
///
/// **The published known-answer vectors are not here, and the reason is
/// said rather than left blank**: NIST's are distributed as ACVP JSON
/// rather than in the standard's text, and pulling that corpus in is a
/// decision about what this repository carries — not something to do
/// quietly inside a test. What is checked is the parameter set and the
/// round trip, which is every claim PAY-13 makes about the KEM except the
/// arithmetic itself.
#[test]
fn ml_kem_768_has_the_sizes_fips_203_fixes_and_round_trips() {
    // FIPS 203 Table 2, ML-KEM-768: encapsulation key 1184 bytes,
    // ciphertext 1088, shared secret 32
    let secret = KemSecret::from_seed([7u8; 64]);
    let public = secret.public();
    assert_eq!(public.0.len(), 1184, "ML-KEM-768 encapsulation key");

    let (ciphertext, theirs) = secret
        .public()
        .encapsulate([3u8; 32])
        .expect("encapsulates");
    assert_eq!(ciphertext.len(), 1088, "ML-KEM-768 ciphertext");
    assert_eq!(theirs.len(), 32, "ML-KEM shared secret");

    let mine = secret.decapsulate(&ciphertext).expect("decapsulates");
    assert_eq!(mine, theirs, "both sides reach the same secret");

    // a ciphertext of the wrong length is not a 768 ciphertext
    assert!(
        secret.decapsulate(&ciphertext[..1087]).is_err(),
        "a short ciphertext is refused rather than truncated into one"
    );
    // **and the seed is what regenerates it**, which is what lets a device
    // keep 64 bytes instead of a decapsulation key
    assert_eq!(
        KemSecret::from_seed([7u8; 64]).public().0,
        public.0,
        "the same seed, the same key"
    );
}

/// **The profile string PQXDH is instantiated under** (design §14.2.4.2):
/// the construction is parameterised over a DH and a KEM, so the label has
/// to name which — and a profile that drifted from the primitives above
/// would be two implementations calling themselves one.
#[test]
fn the_profile_names_the_primitives_it_is_instantiated_with() {
    let info = String::from_utf8(rhtn_crypto::pqxdh::INFO.to_vec()).expect("ascii");
    assert!(info.contains("X25519"), "the DH is named: {info}");
    assert!(info.contains("ML-KEM-768"), "and the KEM: {info}");
    assert!(info.contains("SHA-256"), "and the hash: {info}");
}
