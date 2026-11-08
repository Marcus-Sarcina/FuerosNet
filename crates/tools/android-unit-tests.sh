#!/usr/bin/env bash
# The Android shell's JVM unit tests (mobile/android/app/src/test), run
# through Gradle: the screen contract and whatever else the shell's own
# logic grows.  Needs the JBR and an Android SDK the project's
# local.properties or ANDROID_HOME names.  Exits 3 where either is absent,
# which the gate reports as skipped, never as passed.
set -u
HERE="$(cd "$(dirname "$0")/.." && pwd)"
ANDROID="$HERE/mobile/android"
export JAVA_HOME="${JAVA_HOME:-$HOME/opt/android-studio/jbr}"
[ -x "$JAVA_HOME/bin/java" ] || { echo "  jbr absent: Android unit tests skipped"; exit 3; }
[ -f "$ANDROID/local.properties" ] || [ -n "${ANDROID_HOME:-}" ] \
  || { echo "  Android SDK absent: Android unit tests skipped"; exit 3; }
LOG="$ANDROID/app/build/gate-unit-tests.log"
mkdir -p "$ANDROID/app/build"
# the releasable flavour's variant: the test sources are one set for both
# flavours, and the gate builds the releasable
# **and the instrumentation source set is compiled, not run** [2026-11-08]:
# PRD-12 and RSC-58 rest on a real `WebView`'s behaviour, which wants a
# device the gate has not got -- but a test nothing compiles rots, and a
# rotted one is discovered at the moment somebody needs it. So the gate
# assembles it and the bench runs it
# (`./gradlew connectedFieldtestDebugAndroidTest`).
if (cd "$ANDROID" && ./gradlew --no-daemon -q testReleasableDebugUnitTest assembleFieldtestDebugAndroidTest > "$LOG" 2>&1); then
  # the count, not a state: read what actually ran
  python3 - "$ANDROID" <<'PY'
import glob, sys
import xml.etree.ElementTree as ET
tot = fail = 0
for f in glob.glob(sys.argv[1] + "/app/build/test-results/testReleasableDebugUnitTest/*.xml"):
    r = ET.parse(f).getroot()
    tot += int(r.get("tests", 0))
    fail += int(r.get("failures", 0)) + int(r.get("errors", 0))
print(f"  {tot} run, {fail} failed")
sys.exit(1 if fail or tot == 0 else 0)
PY
else
  tail -20 "$LOG"
  exit 1
fi
