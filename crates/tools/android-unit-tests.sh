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
if (cd "$ANDROID" && ./gradlew --no-daemon -q testDebugUnitTest > "$LOG" 2>&1); then
  # the count, not a state: read what actually ran
  python3 - "$ANDROID" <<'PY'
import glob, sys
import xml.etree.ElementTree as ET
tot = fail = 0
for f in glob.glob(sys.argv[1] + "/app/build/test-results/testDebugUnitTest/*.xml"):
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
