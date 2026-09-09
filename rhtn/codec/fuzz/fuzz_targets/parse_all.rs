//! Any bytes get a verdict without a panic; anything the parser accepts
//! re-encodes to itself (DEC-02's property, coverage-guided).
#![no_main]
use libfuzzer_sys::fuzz_target;
use rhtn_codec::cbor::{parse_all, reencode};

fuzz_target!(|data: &[u8]| {
    if let Ok(item) = parse_all(data) {
        assert_eq!(reencode(&item, data), data);
    }
});
