//! The subnet a locator names (`wire-format.md` §2.3 field 4; design
//! §13.8): present on every locator two or more hops down, absent on the
//! self-anchored locator of a node that has adopted nobody and on the
//! locator of a root's first adoption, whose own txid names the subnet.

use rhtn_archive::tx::{Locator, Seqno};

const ANCHOR: [u8; 32] = [0xa1; 32];
const SUBNET: [u8; 32] = [0xdb; 32];

fn seqno() -> Seqno {
    Seqno {
        series: 1,
        counter: 0,
    }
}

// acceptance: TOP-48
#[test]
fn a_positioned_locator_names_its_subnet_and_one_without_is_malformed() {
    let positioned = Locator {
        anchor: ANCHOR,
        path: vec![0x31],
        nibbles: 2,
        seqno: seqno(),
        subnet: Some(SUBNET),
    };
    let mut bytes = Vec::new();
    positioned.emit(&mut bytes);
    let back = Locator::decode(&bytes).expect("a positioned locator with its subnet decodes");
    assert_eq!(back, positioned);
    assert_eq!(back.subnet, Some(SUBNET), "and the subnet is read back");
    // the schema reads the same rule at its own boundary
    let item = rhtn_codec::cbor::parse_all(&bytes).expect("cbor");
    assert!(rhtn_codec::schema::check_kind(&bytes, "Locator", &item).is_ok());

    // **the same position with no field 4 is malformed**: two hops down
    // puts the node in some subnet, and a locator that does not say which
    // is not one anybody can keep apart from another
    let mut unnamed = Vec::new();
    Locator {
        subnet: None,
        ..positioned.clone()
    }
    .emit(&mut unnamed);
    let e = Locator::decode(&unnamed).expect_err("a positioned locator names no subnet");
    assert!(e.contains("names no subnet"), "{e}");
    let item = rhtn_codec::cbor::parse_all(&unnamed).expect("cbor");
    assert!(rhtn_codec::schema::check_kind(&unnamed, "Locator", &item).is_err());

    // **one hop down may be the root's first adoption**, the record whose
    // own txid names the subnet, so it decodes without one
    let mut first = Vec::new();
    Locator {
        path: vec![0x30],
        nibbles: 1,
        subnet: None,
        ..positioned.clone()
    }
    .emit(&mut first);
    let back = Locator::decode(&first).expect("a root's first adoption carries no subnet");
    assert_eq!(back.subnet, None);
    let item = rhtn_codec::cbor::parse_all(&first).expect("cbor");
    assert!(rhtn_codec::schema::check_kind(&first, "Locator", &item).is_ok());
}

// acceptance: TOP-48
#[test]
fn a_lone_identity_is_self_anchored_in_no_subnet_and_a_root_may_name_one() {
    // a node that has adopted nobody: the empty path, and nothing to name
    let lone = Locator::root(ANCHOR, seqno());
    let mut bytes = Vec::new();
    lone.emit(&mut bytes);
    let back = Locator::decode(&bytes).expect("a lone identity's locator decodes");
    assert_eq!(back.subnet, None);
    assert_eq!(back.nibbles, 0);

    // a root that has adopted somebody names the subnet under it, at the
    // same empty path
    let root = Locator::root_in(ANCHOR, seqno(), Some(SUBNET));
    let mut bytes = Vec::new();
    root.emit(&mut bytes);
    let back = Locator::decode(&bytes).expect("a root's locator decodes");
    assert_eq!(back.subnet, Some(SUBNET));
    assert_eq!((back.path.as_slice(), back.nibbles), (&[][..], 0));

    // and a subnet of the wrong width is refused as the field, not as
    // a missing one
    let mut odd = Vec::new();
    rhtn_codec::encode::emit_map_head(&mut odd, 4);
    rhtn_codec::encode::emit_uint(&mut odd, 1);
    rhtn_codec::encode::emit_bstr(&mut odd, &ANCHOR);
    rhtn_codec::encode::emit_uint(&mut odd, 2);
    rhtn_codec::encode::emit_map_head(&mut odd, 2);
    rhtn_codec::encode::emit_uint(&mut odd, 1);
    rhtn_codec::encode::emit_bstr(&mut odd, &[]);
    rhtn_codec::encode::emit_uint(&mut odd, 2);
    rhtn_codec::encode::emit_uint(&mut odd, 0);
    rhtn_codec::encode::emit_uint(&mut odd, 3);
    seqno().emit(&mut odd);
    rhtn_codec::encode::emit_uint(&mut odd, 4);
    rhtn_codec::encode::emit_bstr(&mut odd, &[0xdb; 31]);
    let e = Locator::decode(&odd).expect_err("31 bytes is not a txid");
    assert!(e.contains("field 4"), "{e}");
}
