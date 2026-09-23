#!/usr/bin/env bash
# One round trip through the generated Kotlin binding (ffi/kotlin/RoundTrip.kt)
# against a real serving node.  Needs a JDK and the Kotlin compiler: found on
# PATH, or under ~/opt/android-studio/jbr and ~/opt/kotlinc where the local
# install put them.  Exits 3 where either is absent, which the gate reports
# as skipped, never as passed.
set -u
HERE="$(cd "$(dirname "$0")/.." && pwd)"
export JAVA_HOME="${JAVA_HOME:-$HOME/opt/android-studio/jbr}"
export PATH="$JAVA_HOME/bin:$HOME/opt/kotlinc/bin:$PATH"
JNA="${RHTN_JNA_JAR:-$HOME/opt/downloads/jna-5.17.0.jar}"
for tool in java kotlinc; do
  command -v "$tool" > /dev/null 2>&1 || { echo "  $tool absent: Kotlin round trip skipped"; exit 3; }
done
[ -f "$JNA" ] || { echo "  jna jar absent at $JNA: Kotlin round trip skipped"; exit 3; }
OUT="$HERE/target/uniffi"
mkdir -p "$OUT/kotlin" "$OUT/build"
(cd "$HERE" && cargo build -q -p rhtn-ffi --features harness) || exit 1
(cd "$HERE" && cargo run -q -p rhtn-ffi --features cli,harness --bin uniffi-bindgen -- \
  generate --library target/debug/librhtn_ffi.so --language kotlin --out-dir "$OUT/kotlin" --no-format) || exit 1
kotlinc -cp "$JNA" "$OUT/kotlin/uniffi/rhtn_ffi/rhtn_ffi.kt" "$HERE/ffi/kotlin/RoundTrip.kt" \
  -include-runtime -d "$OUT/build/roundtrip.jar" 2> "$OUT/build/kotlinc.log" || { grep -m5 error "$OUT/build/kotlinc.log"; exit 1; }
java --enable-native-access=ALL-UNNAMED -cp "$OUT/build/roundtrip.jar:$JNA" -Djna.library.path="$HERE/target/debug" RoundTripKt
