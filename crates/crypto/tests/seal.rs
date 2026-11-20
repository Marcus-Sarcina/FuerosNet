//! The split seal's parts (design §14.2.4, `wire-format.md` §7.8.1):
//! X-Wing against its draft's own vectors, and the seal around it —
//! both secrets needed, either half altered breaking it, the tag one both
//! parties compute and nobody else does, and the encryption key the same
//! wherever the identity root is.

use rhtn_crypto::seal::{self, EncryptionSecret};
use rhtn_crypto::xwing;

fn bytes<const N: usize>(hex_str: &str) -> [u8; N] {
    hex::decode(hex_str)
        .expect("hex")
        .try_into()
        .expect("the length the draft states")
}

/// **X-Wing is the draft's, to the byte**: Appendix C of
/// `draft-connolly-cfrg-xwing-kem-11` (`data/xwing-11.json`, its three
/// vectors as published; the text's SHA-256 is recorded beside them).
/// The key expands from its seed to the published encapsulation key,
/// encapsulation from the published randomness gives the published
/// ciphertext and secret, and decapsulation recovers the secret.
#[test]
fn x_wing_reproduces_its_drafts_vectors() {
    let doc: serde_json::Value =
        serde_json::from_str(include_str!("data/xwing-11.json")).expect("the vectors");
    let vectors = doc["vectors"].as_array().expect("an array");
    assert_eq!(vectors.len(), 3);
    for v in vectors {
        let seed: [u8; 32] = bytes(v["seed"].as_str().unwrap());
        let public: [u8; xwing::PUBLIC_BYTES] = bytes(v["pk"].as_str().unwrap());
        let eseed: [u8; xwing::ESEED_BYTES] = bytes(v["eseed"].as_str().unwrap());
        let ct: [u8; xwing::CIPHERTEXT_BYTES] = bytes(v["ct"].as_str().unwrap());
        let ss: [u8; 32] = bytes(v["ss"].as_str().unwrap());
        let secret = xwing::Secret::from_seed(seed);
        assert_eq!(secret.public(), &public, "the encapsulation key");
        let (got_ss, got_ct) = xwing::encapsulate(&public, &eseed).expect("a valid key");
        assert_eq!(got_ct, ct, "the ciphertext");
        assert_eq!(*got_ss, ss, "the encapsulated secret");
        assert_eq!(*secret.decapsulate(&ct), ss, "the decapsulated secret");
    }
}

/// An encapsulation key whose ML-KEM half is out of range is refused
/// rather than encapsulated to (FIPS 203's input check; the draft's §5.4).
#[test]
fn an_invalid_encapsulation_key_is_refused() {
    let mut public = *xwing::Secret::from_seed([9; 32]).public();
    public[..2].copy_from_slice(&[0xff, 0xff]);
    assert!(xwing::encapsulate(&public, &[1; 64]).is_err());
}

struct Party {
    keyhash: [u8; 32],
    secret: EncryptionSecret,
}

fn party(tag: u8) -> Party {
    Party {
        keyhash: [tag; 32],
        secret: EncryptionSecret::from_identity_root(&[tag ^ 0x5a; 32]),
    }
}

/// One message from `a` to `b` through node `n` holding `node`, sealed and
/// opened end to end the way §7.8.1 states it.
#[test]
fn a_sealed_body_opens_only_with_both_halves_and_its_tag_proves_the_sender() {
    let (a, b) = (party(1), party(2));
    let node = xwing::Secret::from_seed([3; 32]);
    let node_kh = [4u8; 32];
    let n = 7u64;
    // sender
    let (ss_r, ct_r) = xwing::encapsulate(b.secret.kem_public(), &[5; 64]).unwrap();
    let (ss_n, ct_n) = xwing::encapsulate(node.public(), &[6; 64]).unwrap();
    let key = seal::message_key(&ss_r, &ss_n, &b.keyhash, &node_kh, n, &ct_r, &ct_n);
    let fields = b"fields one to five, as the client encodes them";
    let tk = a
        .secret
        .tag_key(&b.secret.auth_public(), &a.keyhash, &b.keyhash);
    let t = seal::tag(&tk, &ct_r, &ct_n, fields);
    let sealed = seal::seal(&key, [8; seal::NONCE_BYTES], fields);
    // node opens its own half; the recipient the rest
    let half = node.decapsulate(&ct_n);
    let mine = b.secret.decapsulate(&ct_r);
    let key_b = seal::message_key(&mine, &half, &b.keyhash, &node_kh, n, &ct_r, &ct_n);
    assert_eq!(
        seal::open(&key_b, &sealed).as_deref().map(|v| &v[..]),
        Some(&fields[..])
    );
    // the recipient computes the same tag key, and the tag verifies
    let tk_b = b
        .secret
        .tag_key(&a.secret.auth_public(), &a.keyhash, &b.keyhash);
    assert_eq!(*tk, *tk_b, "either party computes the tag key");
    assert!(seal::tag_verifies(&tk_b, &ct_r, &ct_n, fields, &t));
    // a third party's tag key differs, so its tag does not verify
    let c = party(9);
    let tk_c = c
        .secret
        .tag_key(&b.secret.auth_public(), &a.keyhash, &b.keyhash);
    assert!(!seal::tag_verifies(&tk_c, &ct_r, &ct_n, fields, &t));
    // the node's half alone, or the recipient's alone, opens nothing
    let wrong = [0u8; 32];
    let node_only = seal::message_key(&wrong, &half, &b.keyhash, &node_kh, n, &ct_r, &ct_n);
    let recipient_only = seal::message_key(&mine, &wrong, &b.keyhash, &node_kh, n, &ct_r, &ct_n);
    assert!(seal::open(&node_only, &sealed).is_none());
    assert!(seal::open(&recipient_only, &sealed).is_none());
    // the key binds the node key's number and the recipient
    let other_n = seal::message_key(&mine, &half, &b.keyhash, &node_kh, n + 1, &ct_r, &ct_n);
    assert!(seal::open(&other_n, &sealed).is_none());
    let other_r = seal::message_key(&mine, &half, &a.keyhash, &node_kh, n, &ct_r, &ct_n);
    assert!(seal::open(&other_r, &sealed).is_none());
    // a changed byte does not open, and a short one is refused
    let mut flipped = sealed.clone();
    *flipped.last_mut().unwrap() ^= 1;
    assert!(seal::open(&key_b, &flipped).is_none());
    assert!(seal::open(&key_b, &sealed[..20]).is_none());
    // a changed field fails the tag
    assert!(!seal::tag_verifies(
        &tk_b,
        &ct_r,
        &ct_n,
        b"another body",
        &t
    ));
}

/// **The encryption key is the identity root's**: the same root gives the
/// same keys, a paired device given the two secrets holds the same keys,
/// and another root gives others.
#[test]
fn the_encryption_key_follows_the_identity_root_and_crosses_as_two_secrets() {
    let root = [0x42u8; 32];
    let a = EncryptionSecret::from_identity_root(&root);
    let again = EncryptionSecret::from_identity_root(&root);
    assert_eq!(a.kem_public(), again.kem_public());
    assert_eq!(a.auth_public(), again.auth_public());
    let paired = EncryptionSecret::from_secrets(a.kem_seed(), &a.auth_secret());
    assert_eq!(paired.kem_public(), a.kem_public());
    assert_eq!(paired.auth_public(), a.auth_public());
    let other = EncryptionSecret::from_identity_root(&[0x43; 32]);
    assert_ne!(other.kem_public(), a.kem_public());
    assert_ne!(other.auth_public(), a.auth_public());
    assert_ne!(
        a.kem_seed(),
        &*a.auth_secret(),
        "two keys, not one used twice"
    );
}

/// **The encryption key is the vectors', to the byte** (`wire-format.md`
/// §2.5): alice's, issued here from her identity root, equals the one the
/// generator derived through kyber-py's ML-KEM and its own HKDF, so the
/// derivation is checked against an implementation that is not this one.
/// It verifies under alice and not under another, and field 1 names her.
#[test]
fn an_issued_encryption_key_matches_the_vectors_and_verifies_under_its_identity() {
    use rhtn_crypto::encryption_key;
    use rhtn_crypto::identity::testkit::test_identity;
    use rhtn_crypto::verify;
    let corpus: serde_json::Value =
        serde_json::from_str(include_str!("../../../test-vectors/corpus.json")).expect("corpus");
    let vector = corpus["entries"]
        .as_array()
        .unwrap()
        .iter()
        .find(|e| e["id"] == "P-encryption-key")
        .and_then(|e| e["hex"].as_str())
        .map(|h| hex::decode(h).unwrap())
        .expect("the vector");
    let alice = test_identity("alice");
    let issued = encryption_key::own(&alice);
    assert_eq!(issued, vector, "the vectors' bytes");
    let key = verify::encryption_key(std::slice::from_ref(&alice.public), &issued).expect("hers");
    assert_eq!(key.keyhash, alice.public.keyhash);
    let mine = EncryptionSecret::from_identity_root(alice.identity_root());
    assert_eq!(&key.kem, mine.kem_public());
    assert_eq!(key.auth, mine.auth_public());
    // carol's key does not verify as alice's (the corpus carries the
    // wrong-signer case as N-encryption-key-wrong-signer)
    let carol = test_identity("carol").public;
    assert!(verify::encryption_key(std::slice::from_ref(&carol), &issued).is_err());
}

fn corpus_hex(id: &str) -> Vec<u8> {
    let corpus: serde_json::Value =
        serde_json::from_str(include_str!("../../../test-vectors/corpus.json")).expect("corpus");
    corpus["entries"]
        .as_array()
        .unwrap()
        .iter()
        .find(|e| e["id"] == id)
        .and_then(|e| e["hex"].as_str())
        .map(|h| hex::decode(h).unwrap())
        .unwrap_or_else(|| panic!("{id} is in the corpus"))
}

// acceptance: PAY-13
/// **The canonical seal opens here** (`wire-format.md` §7.8.1;
/// `test-vectors/records.md`): alice's first message to carol, sealed by the
/// generator through kyber-py's ML-KEM and Python's own HKDF, HMAC and
/// AES-GCM, is opened by this crate as carol and the node would open it —
/// carol's half under the key her identity root derives, the node's under
/// its key's seed — and the tag verifies under the key alice's introduction
/// carries. Either half alone opens nothing.
#[test]
fn the_canonical_seal_opens_with_both_halves_and_its_tag_verifies() {
    use rhtn_codec::cbor::{Item, array_item_ranges, parse_all};
    use rhtn_crypto::identity::testkit::test_identity;
    let sealed = corpus_hex("P-sealed");
    let body = corpus_hex("P-sealed-body");
    let item = parse_all(&sealed).unwrap();
    let Item::Array(s) = &item else {
        panic!("a Sealed is an array")
    };
    let bytes_of = |it: &Item| match it {
        Item::Bytes(r) => sealed[r.clone()].to_vec(),
        _ => panic!("bytes"),
    };
    let n = rhtn_codec::cbor::as_uint(&s[0]).unwrap();
    let ct_r: [u8; xwing::CIPHERTEXT_BYTES] = bytes_of(&s[1]).try_into().unwrap();
    let ct_n: [u8; xwing::CIPHERTEXT_BYTES] = bytes_of(&s[2]).try_into().unwrap();
    let ciphertext = bytes_of(&s[3]);
    let (alice, bob, carol) = (
        test_identity("alice"),
        test_identity("bob"),
        test_identity("carol"),
    );
    let carols = EncryptionSecret::from_identity_root(carol.identity_root());
    let node_seed: [u8; 32] =
        rhtn_codec::cose::sha256(b"rhtn-test-vectors:node-key-seed:bob:carol:3");
    let node = xwing::Secret::from_seed(node_seed);
    let ss_r = carols.decapsulate(&ct_r);
    let ss_n = node.decapsulate(&ct_n);
    let key = seal::message_key(
        &ss_r,
        &ss_n,
        &carol.public.keyhash,
        &bob.public.keyhash,
        n,
        &ct_r,
        &ct_n,
    );
    let opened = seal::open(&key, &ciphertext).expect("both halves open it");
    assert_eq!(&opened[..], &body[..], "to the P-sealed-body fixture");
    let wrong = seal::message_key(
        &ss_r,
        &[0; 32],
        &carol.public.keyhash,
        &bob.public.keyhash,
        n,
        &ct_r,
        &ct_n,
    );
    assert!(
        seal::open(&wrong, &ciphertext).is_none(),
        "carol's half alone"
    );
    let wrong = seal::message_key(
        &[0; 32],
        &ss_n,
        &carol.public.keyhash,
        &bob.public.keyhash,
        n,
        &ct_r,
        &ct_n,
    );
    assert!(
        seal::open(&wrong, &ciphertext).is_none(),
        "the node's half alone"
    );
    // the tag: fields 1 to 5 are the body less its head and its tag
    let parts = array_item_ranges(&body, 0).unwrap();
    let fields = &body[parts[0].start..parts[5].start];
    let tag = &body[body.len() - 32..];
    let alices = EncryptionSecret::from_identity_root(alice.identity_root());
    let tk = carols.tag_key(
        &alices.auth_public(),
        &alice.public.keyhash,
        &carol.public.keyhash,
    );
    assert!(seal::tag_verifies(&tk, &ct_r, &ct_n, fields, tag));
}
