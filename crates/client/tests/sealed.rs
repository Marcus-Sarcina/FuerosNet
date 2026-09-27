//! The at-rest seal: one construction, custody elsewhere
//! (`light-client-requirements.md` §9).

use rhtn_client::sealed::{HEADER, is_sealed, open, seal};

const KEY: [u8; 32] = [7u8; 32];

#[test]
fn what_is_sealed_opens_under_its_key_and_name() {
    let blob = seal(&KEY, "client", b"the durable state");
    assert!(is_sealed(&blob), "the header names the construction");
    assert_eq!(
        open(&KEY, "client", &blob).as_deref(),
        Some(&b"the durable state"[..])
    );
}

#[test]
fn another_key_does_not_open_it() {
    let blob = seal(&KEY, "client", b"the durable state");
    assert_eq!(open(&[8u8; 32], "client", &blob), None);
}

#[test]
fn another_names_contents_do_not_open_as_this_one() {
    // the name is the associated data: a blob lifted from one file refuses
    // to open as another, so swapped files fail closed
    let blob = seal(&KEY, "siblings", b"an address list");
    assert_eq!(open(&KEY, "client", &blob), None);
}

#[test]
fn a_damaged_blob_refuses_whole() {
    let mut blob = seal(&KEY, "client", b"the durable state");
    let last = blob.len() - 1;
    blob[last] ^= 1;
    assert_eq!(open(&KEY, "client", &blob), None);
    assert_eq!(open(&KEY, "client", &blob[..blob.len() - 2]), None);
    assert_eq!(
        open(&KEY, "client", HEADER),
        None,
        "a bare header is no blob"
    );
}

#[test]
fn state_written_before_the_seam_carries_no_header() {
    // the kernel's pre-seam durable blob is deterministic CBOR, which can
    // never begin with the header's bytes
    assert!(!is_sealed(&[0x87]), "an array head is not a sealed blob");
    assert!(!is_sealed(b""), "nothing is not a sealed blob");
}

#[test]
fn two_seals_of_one_plaintext_differ() {
    // a fresh nonce per write: equal states do not advertise their equality
    let (a, b) = (seal(&KEY, "client", b"same"), seal(&KEY, "client", b"same"));
    assert_ne!(a, b);
    assert_eq!(open(&KEY, "client", &a), open(&KEY, "client", &b));
}
