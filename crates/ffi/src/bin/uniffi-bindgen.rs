//! The binding generator's command line, run from the workspace:
//! `cargo run -p rhtn-ffi --features cli --bin uniffi-bindgen generate
//! --library target/debug/librhtn_ffi.so --language kotlin --out-dir <dir>`.
fn main() {
    uniffi::uniffi_bindgen_main()
}
