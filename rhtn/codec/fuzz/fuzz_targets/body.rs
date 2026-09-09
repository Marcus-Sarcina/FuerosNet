//! A transaction body at the schema layer.
#![no_main]
use libfuzzer_sys::fuzz_target;
use rhtn_codec::cbor::{parse_all, reencode};
use rhtn_codec::schema::check_body;

fuzz_target!(|data: &[u8]| {
    if let Ok(item) = parse_all(data) {
        if check_body(data, &item).is_ok() {
            assert_eq!(reencode(&item, data), data);
        }
    }
});
