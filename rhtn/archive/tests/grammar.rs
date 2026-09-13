//! Three rules a decoder applies before anything reads the value
//! (`wire-format.md` §1.2, §2, §7.10): a structured field is the object
//! the grammar names, a value outside a declared range is refused rather
//! than narrowed, and a conditional field stands or falls with the field
//! it describes.
//!
//! Each of these was a divergence between the grammar and this decoder
//! that no round trip could see, because a decoder that is wrong the same
//! way on the way out and the way in agrees with itself.

use rhtn_archive::prekey::PrekeyBundle;
use rhtn_archive::submission::*;
use rhtn_archive::tx::{Locator, Seqno};
use rhtn_codec::cbor::*;
use rhtn_codec::encode::*;
use rhtn_crypto::identity::testkit::test_identity;

fn bundle(name: &str) -> Vec<u8> {
    PrekeyBundle::build(&test_identity(name), 1, b"reusable material", 1_800_000_000)
}

// acceptance: DEC-28
#[test]
fn a_structured_field_is_the_object_the_grammar_names() {
    let b = bundle("alice");
    // as `wire-format.md` §7.10 declares it: field 1 is the bundle map
    let good = PrekeyPublication { bundle: b.clone(), nonce: [1; 16] }.encode();
    let item = parse_all(&good).expect("parses");
    let Item::Map(ref m) = item else { panic!("a map") };
    assert!(matches!(map_get(m, 1), Some(Item::Map(_))), "field 1 is the bundle, spliced in");
    assert_eq!(PrekeyPublication::decode(&good).unwrap().bundle, b);

    // the same bundle wrapped in a byte string: the shape a decoder that
    // wraps on the way out and unwraps on the way in would produce, and
    // would then accept from itself
    let mut wrapped = Vec::new();
    emit_map_head(&mut wrapped, 2);
    emit_uint(&mut wrapped, 1);
    emit_bstr(&mut wrapped, &b);
    emit_uint(&mut wrapped, 2);
    emit_bstr(&mut wrapped, &[1u8; 16]);
    assert!(PrekeyPublication::decode(&wrapped).is_err(), "the wrapping is refused");

    // and the reverse direction, which is the half a self-test cannot
    // reach: the bytes this side emits are what a peer applying the
    // grammar accepts
    assert_ne!(good, wrapped, "the two encodings are not the same bytes");
    assert!(rhtn_codec::schema::check_unsigned(rhtn_codec::schema::Family::PrekeyPublication, &good, 0).is_ok());
    assert!(rhtn_codec::schema::check_unsigned(rhtn_codec::schema::Family::PrekeyPublication, &wrapped, 0).is_err());
}

// acceptance: DEC-29
#[test]
fn a_value_outside_its_declared_range_is_refused_not_narrowed() {
    let anchor = test_identity("bob").public.keyhash;
    let mut inside = Vec::new();
    Locator { anchor, path: Vec::new(), nibbles: 0, seqno: Seqno { series: u32::MAX, counter: u32::MAX } }.emit(&mut inside);
    let back = Locator::decode(&inside).expect("the top of the range is inside it");
    assert_eq!((back.seqno.series, back.seqno.counter), (u32::MAX, u32::MAX));

    // one past the top, hand-built because the encoder cannot express it
    for (series, counter) in [(1u64 << 32, 0u64), (0, 1u64 << 32)] {
        let mut out = Vec::new();
        emit_map_head(&mut out, 3);
        emit_uint(&mut out, 1);
        emit_bstr(&mut out, &anchor);
        emit_uint(&mut out, 2);
        emit_map_head(&mut out, 2);
        emit_uint(&mut out, 1);
        emit_bstr(&mut out, &[]);
        emit_uint(&mut out, 2);
        emit_uint(&mut out, 0);
        emit_uint(&mut out, 3);
        emit_array_head(&mut out, 2);
        emit_uint(&mut out, series);
        emit_uint(&mut out, counter);
        let e = Locator::decode(&out).expect_err("outside the u32 range");
        assert!(e.contains("u32 range"), "{e}");
        // and the schema refuses it too, so nothing reaches the value by
        // another path
        assert!(rhtn_codec::schema::check_type(&out, 0, rhtn_codec::schema::T::Locator).is_err());
        // **narrowing would have been silent**: the value a truncating
        // decoder reads is not the value that was sent, and two parties
        // would then disagree about which series a node is on with no
        // error raised on either side
        let sent = series.max(counter);
        assert_ne!(sent as u32 as u64, sent, "narrowing changes the value, which is why it is refused");
    }
}

// acceptance: DEC-30
#[test]
fn a_conditional_field_stands_or_falls_with_the_field_it_describes() {
    let nonce = [4u8; 16];
    let endpoint = || WakeEndpoint { url: "https://push.example/rhtn/a3f9".into(), key: vec![7; 32], lapses_at: None };

    // the two shapes the grammar admits
    let registered = WakeRegistration::of(nonce, Some(endpoint())).encode();
    assert_eq!(WakeRegistration::decode(&registered).unwrap().endpoint.as_deref(), Some("https://push.example/rhtn/a3f9"));
    let withdrawn = WakeRegistration::of(nonce, None).encode();
    let w = WakeRegistration::decode(&withdrawn).expect("a withdrawal is the nonce alone");
    assert!(w.withdraws() && w.key.is_none() && w.lapses_at.is_none());

    // a key with no endpoint to attach it to
    let orphan_key = WakeRegistration { nonce, endpoint: None, key: Some(vec![7; 32]), lapses_at: None }.encode();
    assert!(WakeRegistration::decode(&orphan_key).is_err(), "a key describes an endpoint that is not there");
    // a lapse with no endpoint either
    let orphan_lapse = WakeRegistration { nonce, endpoint: None, key: None, lapses_at: Some(1_800_090_000) }.encode();
    assert!(WakeRegistration::decode(&orphan_lapse).is_err(), "and so does a lapse");
    // an endpoint with nothing to encrypt the body to
    let no_key = WakeRegistration { nonce, endpoint: Some("https://push.example/x".into()), key: None, lapses_at: None }.encode();
    assert!(WakeRegistration::decode(&no_key).is_err(), "an endpoint nothing can be sent to");

    // every one of them fails at the decoder, before any node policy runs
    for bytes in [&orphan_key, &orphan_lapse, &no_key] {
        assert!(rhtn_codec::schema::check_unsigned(rhtn_codec::schema::Family::WakeRegistration, bytes, 0).is_err());
    }
}
