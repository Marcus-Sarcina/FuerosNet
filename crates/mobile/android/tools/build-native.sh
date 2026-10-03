#!/bin/sh
# Build `librhtn_ffi.so` for the app's ABIs and generate the Kotlin
# binding, into the directories `.gitignore` keeps out of the tree.
# The binding is generated from a HOST build of the same library, as
# `crates/tools/kotlin-roundtrip.sh` generates it; the two builds are of
# one crate at one commit, so the interface cannot differ.
#
# Two flavours of the library, one per Gradle product flavour
# (`Robot/field-test-diagnostics.md`, section 4):
#   build-native.sh               the releasable default, with every
#                                 diagnostic hook compiled out, into
#                                 app/src/releasable/jniLibs
#   build-native.sh --fieldtest   `--no-default-features --features
#                                 fieldtest`, into app/src/fieldtest/jniLibs
# The binding is one shape for both: the features change what the
# library does, not what it exports.
#
# Needs: rustup targets aarch64-linux-android and x86_64-linux-android,
# cargo-ndk, an NDK under $ANDROID_NDK_HOME (or the SDK's newest), cmake.
set -e
HERE="$(cd "$(dirname "$0")/.." && pwd)"
CRATES="$(cd "$HERE/../.." && pwd)"
export ANDROID_NDK_HOME="${ANDROID_NDK_HOME:-$(ls -d "$HOME"/Android/Sdk/ndk/* | sort -V | tail -1)}"

FLAVOUR=releasable
FEATURES=""
for arg in "$@"; do
  case "$arg" in
    --fieldtest) FLAVOUR=fieldtest; FEATURES="--no-default-features --features fieldtest" ;;
    --releasable) FLAVOUR=releasable; FEATURES="" ;;
    *) echo "usage: $0 [--releasable|--fieldtest]" >&2; exit 2 ;;
  esac
done

# a library left under main/ by an earlier version of this script would be
# packaged into both flavours beneath the flavour's own
rm -rf "$HERE/app/src/main/jniLibs"

cd "$CRATES"
# shellcheck disable=SC2086
cargo ndk -t arm64-v8a -t x86_64 -o "$HERE/app/src/$FLAVOUR/jniLibs" build -p rhtn-ffi --release $FEATURES
cargo build -q -p rhtn-ffi
cargo run -q -p rhtn-ffi --features cli --bin uniffi-bindgen -- \
  generate --library target/debug/librhtn_ffi.so --language kotlin \
  --out-dir "$HERE/app/src/generated/kotlin" --no-format
echo "native library ($FLAVOUR) and binding written"
