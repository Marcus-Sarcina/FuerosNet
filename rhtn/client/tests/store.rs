//! The sealed capture store: a capture opens under its key and under no
//! other, and anything short of an authenticated, whole capture is a
//! decryption failure, never a likeness and never evidence.

use rhtn_client::store::{Aead, Capture, ClientStore, Frame, OpenFailure, SealParams, open, seal};

fn capture(p: &SealParams) -> Capture {
    Capture {
        modality: 0,
        template_version: 1,
        template: vec![7u8; p.template_len],
        frames: vec![Frame { at_ms: 1000, bytes: b"frame one".to_vec() }, Frame { at_ms: 6000, bytes: b"frame two, longer".to_vec() }, Frame { at_ms: 12_000, bytes: vec![] }],
    }
}

#[test]
fn a_capture_opens_under_its_key_alone() {
    for aead in [Aead::Aes256Gcm, Aead::ChaCha20Poly1305] {
        let p = SealParams { aead, template_len: 32 };
        let c = capture(&p);
        let key = [1u8; 32];
        let sealed = seal(&p, &key, [3; 32], [4; 32], [5; 32], &c);
        assert_eq!(open(&p, &key, &sealed), Ok(c.clone()));
        assert_eq!(open(&p, &[9u8; 32], &sealed), Err(OpenFailure::Unauthenticated), "another key");
        // the ciphertext carries no frame in the clear
        assert!(!sealed.ciphertext.windows(9).any(|w| w == b"frame one"));
        // a truncated store is a failure to decrypt
        let mut short = sealed.clone();
        short.ciphertext.truncate(short.ciphertext.len() - 1);
        assert_eq!(open(&p, &key, &short), Err(OpenFailure::Unauthenticated));
        short.ciphertext.truncate(8);
        assert_eq!(open(&p, &key, &short), Err(OpenFailure::Truncated));
        // and so is a flipped byte, or a store re-labelled for another record
        let mut flipped = sealed.clone();
        flipped.ciphertext[0] ^= 1;
        assert_eq!(open(&p, &key, &flipped), Err(OpenFailure::Unauthenticated));
        let mut relabelled = sealed.clone();
        relabelled.ceremony_id = [6; 32];
        assert_eq!(open(&p, &key, &relabelled), Err(OpenFailure::Unauthenticated), "the ceremony is in the seal");
        let mut reholder = sealed.clone();
        reholder.holder = [7; 32];
        assert_eq!(open(&p, &key, &reholder), Err(OpenFailure::Unauthenticated), "and so is the holder");
    }
}

#[test]
fn the_store_shows_what_it_holds_and_discards_beside_a_record() {
    let p = SealParams::default();
    let c = capture(&p);
    let key = [1u8; 32];
    let sealed = seal(&p, &key, [3; 32], [4; 32], [5; 32], &c);
    let mut st = ClientStore::default();
    st.sealed.insert([2; 32], sealed);
    st.records.insert([8; 32], b"record bytes".to_vec());
    st.late.entry([8; 32]).or_default().push(b"late response".to_vec());
    assert!(!st.holds_bytes(b"frame one"), "no plaintext frame at rest");
    assert!(!st.holds_bytes(&key), "no key at rest");
    assert!(st.holds_bytes(b"late response"));
    st.discard_record(&[8; 32]);
    assert!(!st.holds_bytes(b"late response"), "gone with the record");
    assert!(st.records.is_empty());
}
