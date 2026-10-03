#!/usr/bin/env bash
# =============================================================================
#  field-setup.sh -- put a fresh field-test build on the test phones and
#  collect their key material for field-run.sh
#  (`Robot/field-test-procedure.md`, "The phones" and "Build and install")
#
#    field-setup.sh [options] [<serial> ...]
#
#  For the phones named (default: every serial in crates/tools/phones.conf):
#    1. builds the field-test binaries: the laptop's daemon, instrument and
#       CLI into crates/target/fieldtest, the phone's native library and
#       binding, and the fieldtest debug APK;
#    2. uninstalls the app from each phone, which wipes its data with it
#       (the kernel's seeds, records and sealed captures, the provision,
#       the event files), so each phone starts with a new identity;
#    3. installs the fresh APK and launches it, which mints the identity;
#    4. asks the tester to open Conversations, where an unprovisioned
#       device shows its public key material, then reads that material
#       through uiautomator every 5 s until it is there;
#    5. writes <serial>=<material> into phones.conf, which field-run.sh
#       reads when it is given no --phone.
#
#  Options:
#    --no-build      install what is already built; the APK must exist.
#    --conf <file>   the phones file to read and write (default:
#                    crates/tools/phones.conf).
#
#  Needs adb with every phone on USB, debugging enabled and the laptop
#  authorised (`adb devices` lists each as `device`); cargo, cargo-ndk and
#  an NDK for the native library; the JBR under ~/opt/android-studio/jbr or
#  $JAVA_HOME for Gradle. The material read is public, as the screen that
#  shows it says; nothing secret leaves the phone here.
# =============================================================================
set -u
set -o pipefail

HERE="$(cd "$(dirname "$0")" && pwd)"
CRATES="$(dirname "$HERE")"
ANDROID="$CRATES/mobile/android"
APK="$ANDROID/app/build/outputs/apk/fieldtest/debug/app-fieldtest-debug.apk"
APP=com.comptus.fueros
CONF="$HERE/phones.conf"
BUILD=yes
INTERVAL=5

say() { echo "field-setup: $*" >&2; }
die() { say "$*"; exit 1; }
need() { command -v "$1" > /dev/null 2>&1 || die "$1 is not installed or not on PATH; $2"; }
find_adb() {
  if command -v adb > /dev/null 2>&1; then command -v adb; return 0; fi
  local d
  for d in "${ANDROID_HOME:-}" "${ANDROID_SDK_ROOT:-}" "$HOME/Android/Sdk"; do
    if [ -n "$d" ] && [ -x "$d/platform-tools/adb" ]; then echo "$d/platform-tools/adb"; return 0; fi
  done
  return 1
}

# ------------------------------------------------------------- arguments -----
SERIALS=()
while [ $# -gt 0 ]; do
  case "$1" in
    --no-build) BUILD=no; shift ;;
    --conf) CONF="${2:-}"; shift 2 ;;
    -h | --help) awk '/^# =+$/ {n++; if (n == 2) exit; next} n == 1 {sub(/^# ?/, ""); print}' "$0"; exit 0 ;;
    -*) die "unknown option $1" ;;
    *) SERIALS+=("$1"); shift ;;
  esac
done

# the phones file: one `serial=material` per line, material empty until
# read; comments and blank lines kept as they are
conf_serials() {
  [ -f "$CONF" ] || return 0
  sed -n 's/^[[:space:]]*\([A-Za-z0-9_.:-]\{1,\}\)[[:space:]]*=.*$/\1/p' "$CONF" | awk '!seen[$0]++'
}
if [ ${#SERIALS[@]} -eq 0 ]; then
  while IFS= read -r s; do [ -n "$s" ] && SERIALS+=("$s"); done < <(conf_serials)
fi
[ ${#SERIALS[@]} -gt 0 ] || die "no phone named and none in $CONF; pass a serial or add one as <serial>= to the file"

ADB="$(find_adb)" || die "adb is not on PATH and no SDK was found under \$ANDROID_HOME, \$ANDROID_SDK_ROOT or ~/Android/Sdk"
for s in "${SERIALS[@]}"; do
  state="$("$ADB" devices | awk -v s="$s" '$1 == s {print $2}')"
  case "$state" in
    device) ;;
    "") die "$s is not listed by adb devices; is it on USB with debugging enabled?" ;;
    *) die "$s is listed as '$state', not 'device'; authorise the laptop on the phone" ;;
  esac
done
say "phones: ${SERIALS[*]}"

# ----------------------------------------------------------------- build -----
if [ "$BUILD" = yes ]; then
  need cargo "the daemon, instrument and CLI are built with it"
  export JAVA_HOME="${JAVA_HOME:-$HOME/opt/android-studio/jbr}"
  [ -x "$JAVA_HOME/bin/java" ] || die "no JDK at $JAVA_HOME; set JAVA_HOME for Gradle"
  say "building the field-test daemon, instrument and CLI (cargo, into target/fieldtest)"
  (
    cd "$CRATES" && cargo build --release --target-dir "$CRATES/target/fieldtest" -p rhtn-cli \
      && cargo build --release --target-dir "$CRATES/target/fieldtest" -p rhtn-daemon -p rhtn-participant \
        --no-default-features --features rhtn-daemon/fieldtest,rhtn-participant/fieldtest
  ) || die "the laptop build failed"
  say "building the phone's native library and binding (fieldtest)"
  "$ANDROID/tools/build-native.sh" --fieldtest || die "the native build failed"
  say "building the fieldtest debug APK (Gradle)"
  (cd "$ANDROID" && ./gradlew --no-daemon -q assembleFieldtestDebug) || die "the APK build failed"
fi
[ -f "$APK" ] || die "$APK is missing; run without --no-build"

# ------------------------------------------------- wipe, install, launch -----
for s in "${SERIALS[@]}"; do
  model="$("$ADB" -s "$s" shell getprop ro.product.model 2> /dev/null | tr -d '\r')"
  say "$s ($model): uninstalling $APP (and its data)"
  # not installed is not a failure
  "$ADB" -s "$s" uninstall "$APP" > /dev/null 2>&1 || true
  say "$s: installing $(basename "$APK")"
  "$ADB" -s "$s" install -r "$APK" > /dev/null || die "$s: the install failed"
  # the first start mints the identity; the material shows in Conversations
  "$ADB" -s "$s" shell am start -W -n "$APP/.HomeActivity" > /dev/null 2>&1 || die "$s: the app did not start"
done

# ------------------------------------------------------- the material -----
read_material() {
  # the Conversations screen prints the material as one line of hex;
  # uiautomator's dump of the screen is the way to read it without a
  # typo. Nothing shorter than 400 hex characters is it.
  "$ADB" -s "$1" exec-out uiautomator dump /dev/tty 2> /dev/null \
    | grep -oE '[0-9a-f]{400,}' | head -1
}
# rewrite one serial's line in the conf (every line naming it, should a
# hand edit have left two), appending where absent
put_material() {
  local s="$1" m="$2" tmp
  tmp="$(mktemp)"
  if [ -f "$CONF" ]; then
    awk -v s="$s" -v m="$m" '
      BEGIN { done = 0 }
      { line = $0; sub(/^[[:space:]]+/, "", line) }
      index(line, s "=") == 1 { if (!done) print s "=" m; done = 1; next }
      { print }
      END { if (!done) print s "=" m }' "$CONF" > "$tmp"
  else
    printf '# Test phones for field-run.sh: <adb serial>=<public key material, hex>.\n# Written by field-setup.sh; the material changes with every wipe.\n%s=%s\n' "$s" "$m" > "$tmp"
  fi
  mv "$tmp" "$CONF"
}

declare -A MATERIAL=()
for s in "${SERIALS[@]}"; do
  model="$("$ADB" -s "$s" shell getprop ro.product.model 2> /dev/null | tr -d '\r')"
  echo
  echo "  On $s ($model): unlock the phone and tap Conversations."
  echo "  The screen shows this device's public key material; this script reads it."
  echo
  n=0
  while :; do
    sleep "$INTERVAL"
    n=$((n + 1))
    m="$(read_material "$s" || true)"
    if [ -n "$m" ]; then
      MATERIAL["$s"]="$m"
      say "$s: material read on attempt $n (${#m} hex characters)"
      break
    fi
    say "$s: not on screen yet (attempt $n); trying again in ${INTERVAL}s"
  done
done

for s in "${SERIALS[@]}"; do put_material "$s" "${MATERIAL[$s]}"; done
say "written to $CONF:"
for s in "${SERIALS[@]}"; do echo "  $s=${MATERIAL[$s]:0:16}…" >&2; done
echo
echo "Next, from the repository root (the phones are read from $CONF):"
echo
echo "    crates/tools/field-run.sh <label>"
echo
