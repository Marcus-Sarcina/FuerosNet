#!/bin/sh
# Build `librhtn_ffi.so` for the app's ABIs and generate the Kotlin
# binding, into the two directories `.gitignore` keeps out of the tree.
# The binding is generated from a HOST build of the same library, as
# `crates/tools/kotlin-roundtrip.sh` generates it; the two builds are of
# one crate at one commit, so the interface cannot differ.
#
# Needs: rustup targets aarch64-linux-android and x86_64-linux-android,
# cargo-ndk, an NDK under $ANDROID_NDK_HOME (or the SDK's newest), cmake.
set -e
HERE="$(cd "$(dirname "$0")/.." && pwd)"
CRATES="$(cd "$HERE/../.." && pwd)"
export ANDROID_NDK_HOME="${ANDROID_NDK_HOME:-$(ls -d "$HOME"/Android/Sdk/ndk/* | sort -V | tail -1)}"

cd "$CRATES"
cargo ndk -t arm64-v8a -t x86_64 -o "$HERE/app/src/main/jniLibs" build -p rhtn-ffi --release
cargo build -q -p rhtn-ffi
cargo run -q -p rhtn-ffi --features cli --bin uniffi-bindgen -- \
  generate --library target/debug/librhtn_ffi.so --language kotlin \
  --out-dir "$HERE/app/src/generated/kotlin" --no-format
echo "native library and binding written"
