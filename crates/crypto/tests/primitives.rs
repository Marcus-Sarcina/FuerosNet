//! **The primitives the split seal is instantiated with, against the
//! vectors their own specifications publish** (PAY-13; design §14.2.4.2).
//!
//! **Why these and not the composition.** The split seal is composed here
//! from standard parts (design §14.2.4.6), and the composition has no
//! published vectors. X-Wing's are checked in `tests/seal.rs`; the
//! primitives beneath it publish their own, and those are the part that
//! can be checked against somebody else's arithmetic rather than against
//! our own [author, 2026-11-08]. The composition is held by this project's
//! own canonical vectors and its independent harness, which is the bar as
//! the author restated it on 2026-10-02.
//!
//! **The hex below is published, not computed here.** That is the whole
//! point of it: a vector this project produced would only say that the
//! implementation agrees with itself.

use rhtn_crypto::kdf::hkdf_sha256;
use x25519_dalek::{PublicKey, StaticSecret};

fn hex(s: &str) -> Vec<u8> {
    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&s[i..i + 2], 16).expect("hex"))
        .collect()
}

fn h32(s: &str) -> [u8; 32] {
    hex(s).try_into().expect("32 bytes")
}

// functional: VAL-006
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

    let alice = StaticSecret::from(a_priv);
    let bob = StaticSecret::from(b_priv);
    assert_eq!(
        PublicKey::from(&alice).to_bytes(),
        a_pub,
        "Alice's public key, RFC 7748 §6.1"
    );
    assert_eq!(
        PublicKey::from(&bob).to_bytes(),
        b_pub,
        "Bob's public key, RFC 7748 §6.1"
    );

    // **and both sides reach it**, which is the property the agreement is
    // used for rather than one direction of the arithmetic
    assert_eq!(
        alice.diffie_hellman(&PublicKey::from(b_pub)).to_bytes(),
        shared,
        "the shared secret, RFC 7748 §6.1"
    );
    assert_eq!(
        bob.diffie_hellman(&PublicKey::from(a_pub)).to_bytes(),
        shared,
        "and from the other side"
    );
}

// functional: VAL-006
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

// functional: VAL-006
/// **X-Wing's sizes are ML-KEM-768's and X25519's together**, which is
/// what §14.2.4.2 names: the sizes are what tell 768 from 512 and 1024,
/// and getting the parameter set wrong is the failure this catches. The
/// arithmetic is X-Wing's own published vectors, in `tests/seal.rs`,
/// which run ML-KEM-768 end to end.
#[test]
fn x_wing_has_the_sizes_fips_203_and_rfc_7748_fix_and_round_trips() {
    use rhtn_crypto::xwing;
    // FIPS 203 Table 2, ML-KEM-768: encapsulation key 1184 bytes,
    // ciphertext 1088; X25519's 32 each
    assert_eq!(xwing::PUBLIC_BYTES, 1184 + 32);
    assert_eq!(xwing::CIPHERTEXT_BYTES, 1088 + 32);
    let secret = xwing::Secret::from_seed([7u8; 32]);
    let (theirs, ct) = xwing::encapsulate(secret.public(), &[3u8; 64]).expect("encapsulates");
    assert_eq!(*secret.decapsulate(&ct), *theirs, "both sides reach it");
    // **and the seed is what regenerates it**, which is what lets a
    // device keep 32 bytes instead of a decapsulation key
    assert_eq!(
        xwing::Secret::from_seed([7u8; 32]).public(),
        secret.public(),
        "the same seed, the same key"
    );
}

// functional: SIG-011
/// **RFC 8032 §7.1, TEST 1 and TEST 2**: the Ed25519 signing key, the
/// public key it expands to, the message and the signature, as the RFC
/// prints them.  The profile's classical component is this arithmetic
/// (`wire-format.md` §2.2), and an altered message, key or signature
/// length verifies as nothing.
#[test]
fn ed25519_agrees_with_rfc_8032_section_7_1() {
    use rhtn_crypto::identity::sign_ed_seed;
    use rhtn_crypto::verify::verify_ed_raw;
    let vectors = [
        (
            "9d61b19deffd5a60ba844af492ec2cc44449c5697b326919703bac031cae7f60",
            "d75a980182b10ab7d54bfed3c964073a0ee172f3daa62325af021a68f707511a",
            "",
            "e5564300c360ac729086e2cc806e828a84877f1eb8e5d974d873e065224901555fb8821590a33bacc61e39701cf9b46bd25bf5f0595bbe24655141438e7a100b",
        ),
        (
            "4ccd089b28ff96da9db6c346ec114e0f5b8a319f35aba624da8cf6ed4fb8a6fb",
            "3d4017c3e843895a92b70aa74d1b7ebc9c982ccf2ec4968cc0cd55f12af4660c",
            "72",
            "92a009a9f0d4cab8720e820b5f642540a2b27b5416503f8fb3762223ebdb69da085ac1e43e15996e458f3613d0f11d8c387b2eaeb4302aeeb00d291612bb0c00",
        ),
    ];
    for (sk, pk, msg, sig) in vectors {
        let (sk, pk, msg, sig) = (h32(sk), h32(pk), hex(msg), hex(sig));
        let (public, signature) = sign_ed_seed(&sk, &msg);
        assert_eq!(public, pk, "the public key the RFC prints");
        assert_eq!(signature.to_vec(), sig, "the signature the RFC prints");
        assert!(verify_ed_raw(&pk, &sig, &msg));
        let mut other_msg = msg.clone();
        other_msg.push(0x00);
        assert!(!verify_ed_raw(&pk, &sig, &other_msg), "an altered message");
        let mut other_key = pk;
        other_key[0] ^= 0x01;
        assert!(!verify_ed_raw(&other_key, &sig, &msg), "an altered key");
        assert!(!verify_ed_raw(&pk, &sig[..63], &msg), "a 63-byte signature");
        let mut long = sig.clone();
        long.push(0x00);
        assert!(!verify_ed_raw(&pk, &long, &msg), "a 65-byte signature");
        let mut flipped = sig.clone();
        flipped[10] ^= 0x01;
        assert!(!verify_ed_raw(&pk, &flipped, &msg), "an altered signature");
    }
}

// functional: SIG-011
/// **ML-DSA-65 as the profile fixes it** (`wire-format.md` §2.2):
/// deterministic signing with the empty context, so one key over one
/// message yields one signature; the sizes FIPS 204 Table 2 gives the
/// parameter set, 1952-byte public keys and 3309-byte signatures; and
/// refusal of an altered message, another key, a flipped byte and a
/// signature of the wrong length.  **No independent known answer is
/// checked here**: FIPS 204 publishes none in the standard itself, and the
/// ACVP and Wycheproof sets are files this repository does not carry.
#[test]
fn ml_dsa_65_is_deterministic_has_fips_204_sizes_and_rejects_alterations() {
    use rhtn_crypto::SigningIdentity;
    let id = SigningIdentity::from_seeds(&[0x11; 32], &[0x22; 32]);
    let msg = b"signed twice, the same both times";
    let first = id.sign_pq(msg);
    let second = id.sign_pq(msg);
    assert_eq!(first, second, "deterministic");
    assert_eq!(first.len(), 3309, "FIPS 204 Table 2: ML-DSA-65 signature");
    assert_eq!(
        id.public.pq.encode().len(),
        1952,
        "FIPS 204 Table 2: ML-DSA-65 public key"
    );
    assert!(id.public.verify_pq(&first, msg));
    assert!(
        !id.public.verify_pq(&first, b"another message"),
        "an altered message"
    );
    let other = SigningIdentity::from_seeds(&[0x11; 32], &[0x33; 32]);
    assert!(!other.public.verify_pq(&first, msg), "another key");
    let mut flipped = first.clone();
    flipped[100] ^= 0x01;
    assert!(!id.public.verify_pq(&flipped, msg), "an altered signature");
    assert!(
        !id.public.verify_pq(&first[..3308], msg),
        "a short signature"
    );
    let mut long = first.clone();
    long.push(0x00);
    assert!(!id.public.verify_pq(&long, msg), "a long signature");
}

// functional: SIG-011
/// **The `COSE_Sign1` `Sig_structure`, byte for byte**: RFC 9052 §4.4
/// defines it as `["Signature1", protected, external_aad, payload]` in
/// deterministic CBOR, and RFC 8152 Appendix C.2.1's example signs the
/// protected header `h'a10126'` over the payload `"This is the content."`.
/// The RFC prints the inputs and not the structure; the bytes asserted are
/// §4.4's encoding of them, written out by hand rather than by this crate.
#[test]
fn the_sign1_structure_is_rfc_9052s_encoding_of_rfc_8152s_example() {
    use rhtn_codec::cose;
    let got = cose::sig_structure_sign1(&hex("a10126"), b"", b"This is the content.");
    let want = concat!(
        "84",                                         // array(4)
        "6a5369676e617475726531",                     // "Signature1"
        "43a10126",                                   // the protected header, as a bstr
        "40",                                         // external_aad, empty
        "54546869732069732074686520636f6e74656e742e"  // "This is the content."
    );
    assert_eq!(got, hex(want));
}

fn hex_of(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect()
}

// functional: SIG-011
/// **Wycheproof's ML-DSA-65 vectors**, a cited subset
/// (`data/wycheproof-mldsa65.json`, from C2SP's repository under
/// Apache-2.0, case ids as published): the public key a seed expands to
/// and the deterministic signature over each message, which is somebody
/// else's arithmetic for the profile's post-quantum component
/// (`wire-format.md` §2.2); and a verifier walked over valid signatures,
/// wrong lengths, modified bytes, a non-canonical hint encoding, an
/// infinity-norm violation and a public key of the wrong length. Cases
/// carrying a context string are not in the subset: the profile signs
/// under the empty context only.
#[test]
fn ml_dsa_65_agrees_with_wycheproofs_vectors() {
    use rhtn_crypto::{Identity, SigningIdentity};
    let v: serde_json::Value =
        serde_json::from_str(include_str!("data/wycheproof-mldsa65.json")).expect("the subset");
    assert_eq!(v["algorithm"], "ML-DSA-65");
    let mut cases = 0;

    // signing: the seed is FIPS 204's ξ, which is what the profile's
    // second seed is
    for g in v["sign"].as_array().expect("sign groups") {
        let seed = h32(g["privateSeed"].as_str().unwrap());
        let id = SigningIdentity::from_seeds(&[0u8; 32], &seed);
        assert_eq!(
            hex_of(id.public.pq.encode().as_ref()),
            g["publicKey"].as_str().unwrap(),
            "the public key the seed expands to"
        );
        for t in g["tests"].as_array().unwrap() {
            let (msg, sig) = (
                hex(t["msg"].as_str().unwrap()),
                hex(t["sig"].as_str().unwrap()),
            );
            let valid = t["result"] == "valid";
            if valid {
                assert_eq!(
                    id.sign_pq(&msg),
                    sig,
                    "tcId {}: {}",
                    t["tcId"],
                    t["comment"]
                );
            }
            assert_eq!(
                id.public.verify_pq(&sig, &msg),
                valid,
                "tcId {}: {}",
                t["tcId"],
                t["comment"]
            );
            cases += 1;
        }
    }

    // verifying: a published key, which may itself be malformed
    let ed = ed25519_dalek::VerifyingKey::from_bytes(&h32(
        "d75a980182b10ab7d54bfed3c964073a0ee172f3daa62325af021a68f707511a",
    ))
    .expect("RFC 8032's key, standing in for the classical half");
    for g in v["verify"].as_array().expect("verify groups") {
        let pk = hex(g["publicKey"].as_str().unwrap());
        let tests = g["tests"].as_array().unwrap();
        let decoded = ml_dsa::EncodedVerifyingKey::<ml_dsa::MlDsa65>::try_from(&pk[..])
            .ok()
            .map(|enc| ml_dsa::VerifyingKey::<ml_dsa::MlDsa65>::decode(&enc));
        let Some(pq) = decoded else {
            for t in tests {
                assert_eq!(
                    t["result"],
                    "invalid",
                    "a key of {} bytes verifies nothing",
                    pk.len()
                );
                cases += 1;
            }
            continue;
        };
        let id = Identity::from_public(ed, pq);
        for t in tests {
            let (msg, sig) = (
                hex(t["msg"].as_str().unwrap()),
                hex(t["sig"].as_str().unwrap()),
            );
            assert_eq!(
                id.verify_pq(&sig, &msg),
                t["result"] == "valid",
                "tcId {}: {}",
                t["tcId"],
                t["comment"]
            );
            cases += 1;
        }
    }
    assert_eq!(cases, 14, "the subset as extracted");
}
