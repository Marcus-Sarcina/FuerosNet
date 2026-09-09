//! The envelope's structure: version, type, body, signer set, entries.
#![no_main]
use libfuzzer_sys::fuzz_target;
use rhtn_codec::cbor::{parse_all, reencode};
use rhtn_codec::envelope;

fuzz_target!(|data: &[u8]| {
    if envelope::parse(data).is_ok() {
        let item = parse_all(data).expect("an envelope that parsed is CBOR");
        assert_eq!(reencode(&item, data), data);
    }
});
