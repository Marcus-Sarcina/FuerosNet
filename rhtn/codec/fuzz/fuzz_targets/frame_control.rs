//! A complete frame, length prefix included, on the control stream.
#![no_main]
use libfuzzer_sys::fuzz_target;
use rhtn_codec::cbor::{parse_all, reencode};
use rhtn_codec::frame::{self, Stream};

fuzz_target!(|data: &[u8]| {
    if frame::parse(Stream::Control, data).is_ok() {
        let payload = &data[4..];
        let item = parse_all(payload).expect("a frame that parsed is CBOR");
        assert_eq!(reencode(&item, payload), payload);
    }
});
