//! The sealed capture store: a capture opens under its key and under no
//! other, and anything short of an authenticated, whole capture is a
//! decryption failure, never a likeness and never evidence.

use rhtn_client::keys::capture_key;
use rhtn_client::store::{
    Capture, ClientStore, Frame, NONCE_LEN, OpenFailure, SLOT_VERSION, SealParams, open, seal,
    slot_label,
};

fn capture(p: &SealParams) -> Capture {
    Capture {
        modality: 0,
        template_version: 1,
        template: vec![7u8; p.template_len],
        frames: vec![
            Frame {
                at_ms: 1000,
                bytes: b"frame one".to_vec(),
            },
            Frame {
                at_ms: 6000,
                bytes: b"frame two, longer".to_vec(),
            },
            Frame {
                at_ms: 12_000,
                bytes: vec![],
            },
        ],
    }
}

// acceptance: CER-64
#[test]
fn a_capture_opens_under_its_key_alone() {
    let p = SealParams::default();
    let c = capture(&p);
    let key = [1u8; 32];
    let sealed = seal(&p, &key, [3; 32], [4; 32], [5; 32], &c);
    assert_eq!(open(&p, &key, &sealed), Ok(c.clone()));
    assert_eq!(
        open(&p, &[9u8; 32], &sealed),
        Err(OpenFailure::Unauthenticated),
        "another key"
    );
    // **one slot per piece** (design §7.5.2.6): the template, then one per
    // frame, each framed as a version byte, its 24-byte nonce and its
    // ciphertext, and no nonce repeated
    assert_eq!(sealed.slots.len(), 1 + c.frames.len());
    assert!(sealed.slots.iter().all(|s| s[0] == SLOT_VERSION));
    let nonces: std::collections::BTreeSet<&[u8]> =
        sealed.slots.iter().map(|s| &s[1..1 + NONCE_LEN]).collect();
    assert_eq!(nonces.len(), sealed.slots.len(), "a nonce per sealing");
    // no frame in the clear anywhere in it
    assert!(
        !sealed
            .slots
            .iter()
            .any(|s| s.windows(9).any(|w| w == b"frame one"))
    );
}

/// **The three ways a sealed slot refuses** (functional row CAP-015).
// acceptance: CER-65
#[test]
fn a_slot_from_elsewhere_and_a_slot_out_of_place_both_refuse() {
    let p = SealParams::default();
    let c = capture(&p);
    let key = [1u8; 32];
    let sealed = seal(&p, &key, [3; 32], [4; 32], [5; 32], &c);

    // 1. a slot sealed under another ceremony's pre-commitment: the
    //    pre-commitment is the associated data, so it does not open here
    let elsewhere = seal(&p, &key, [3; 32], [4; 32], [6; 32], &c);
    let mut lifted = sealed.clone();
    lifted.slots[1] = elsewhere.slots[1].clone();
    assert_eq!(
        open(&p, &key, &lifted),
        Err(OpenFailure::Unauthenticated),
        "a slot from another ceremony"
    );
    let mut relabelled = sealed.clone();
    relabelled.ceremony_id = [6; 32];
    assert_eq!(
        open(&p, &key, &relabelled),
        Err(OpenFailure::Unauthenticated),
        "the ceremony is the associated data"
    );

    // 2. the template's slot presented as an image's: the label is the
    //    position's, so neither opens where the other sat
    let mut swapped = sealed.clone();
    swapped.slots.swap(0, 1);
    assert_eq!(
        open(&p, &key, &swapped),
        Err(OpenFailure::Unauthenticated),
        "a template slot read as an image slot"
    );
    assert_ne!(slot_label(0), slot_label(1));

    // 3. truncated or tampered: a decryption failure and never a likeness
    let mut short = sealed.clone();
    let n = short.slots[1].len();
    short.slots[1].truncate(n - 1);
    assert_eq!(open(&p, &key, &short), Err(OpenFailure::Unauthenticated));
    short.slots[1].truncate(8);
    assert_eq!(open(&p, &key, &short), Err(OpenFailure::Truncated));
    let mut flipped = sealed.clone();
    flipped.slots[2][1 + NONCE_LEN] ^= 1;
    assert_eq!(open(&p, &key, &flipped), Err(OpenFailure::Unauthenticated));
    let mut unknown = sealed.clone();
    unknown.slots[0][0] = SLOT_VERSION + 1;
    assert_eq!(open(&p, &key, &unknown), Err(OpenFailure::Framing));
    // a template of the wrong length for its version is framing, not a
    // short template (design §7.5.2.7)
    let wider = SealParams { template_len: 64 };
    assert_eq!(open(&wider, &key, &sealed), Err(OpenFailure::Framing));
}

/// **Subject and holder are bound by the key, not by the associated data**
/// (design §7.5.2.6): editing the holder in one's own store changes
/// nothing, and a key derived for another holder opens nothing.  CER-04
/// claims the derivation itself; this is the consequence in the store.
#[test]
fn the_holder_is_bound_by_the_key_it_was_handed() {
    let p = SealParams::default();
    let c = capture(&p);
    let (subject, bob, carol, cid) = ([3u8; 32], [4u8; 32], [7u8; 32], [5u8; 32]);
    let root = [1u8; 32];
    let for_bob = capture_key(&root, &subject, &bob, &cid);
    let for_carol = capture_key(&root, &subject, &carol, &cid);
    assert_ne!(*for_bob, *for_carol);
    let sealed = seal(&p, &for_bob, subject, bob, cid, &c);
    assert_eq!(open(&p, &for_bob, &sealed), Ok(c.clone()));
    assert_eq!(
        open(&p, &for_carol, &sealed),
        Err(OpenFailure::Unauthenticated),
        "the key released to one holder opens nobody else's copy"
    );
    // the holder field is the store's own label and outside the seal
    let mut reholder = sealed.clone();
    reholder.holder = carol;
    assert_eq!(
        open(&p, &for_bob, &reholder),
        Ok(c),
        "a holder editing its own label still reads its own capture"
    );
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
    st.late
        .entry([8; 32])
        .or_default()
        .push(b"late response".to_vec());
    assert!(!st.holds_bytes(b"frame one"), "no plaintext frame at rest");
    assert!(!st.holds_bytes(&key), "no key at rest");
    assert!(st.holds_bytes(b"late response"));
    st.discard_record(&[8; 32]);
    assert!(!st.holds_bytes(b"late response"), "gone with the record");
    assert!(st.records.is_empty());
}
