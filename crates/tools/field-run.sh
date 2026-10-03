#!/usr/bin/env bash
# =============================================================================
#  field-run.sh -- one field-test run on the bench, end to end
#  (`Robot/field-test-diagnostics.md`, section 4, "the bench"; the tester's
#  procedure is `Robot/field-test-procedure.md`).
#
#    field-run.sh [options] <label>    start a run and hold it until Ctrl-C
#    field-run.sh stop                 end the run in progress, from any shell
#
#  Options:
#    --addr <ip[:port]>      where rhtnd serves. Default: the address this
#                            machine routes out of, port 7447.
#    --phone <serial>=<hex>  a phone's public key material, as its screen
#                            shows it while unprovisioned. The daemon admits
#                            it, the witnesses list it, and the phone is
#                            provisioned over adb to attach here, with the
#                            other phone as its peer (the first witness when
#                            it is alone). Repeatable. Given none, the
#                            phones come from crates/tools/phones.conf
#                            (<serial>=<material> per line, written by
#                            field-setup.sh); a line with no material is
#                            skipped and said.
#    --level <level>         the daemon's [log] level: off, error, warn, info,
#                            debug (default) or trace.
#    --stream-port <port>    where the phones stream their diagnostic lines
#                            live (`rhtn diag collect`), on the same address
#                            rhtnd serves; default 7448, 0 for no stream.
#                            Plaintext TCP on the tester's own network, and
#                            lossy: the pull at stop is the record.
#    --retention <years>     the retention each phone declares at its intent,
#                            in whole years (design §7.5.1), carried in the
#                            provision; default: the kernel's own, 2.
#    --no-build              do not run cargo; the binaries must exist.
#
#  The run is named <short commit>-<label>-<n> and everything it produces
#  lands in runs/<run>/ (ignored by git):
#    rhtnd.conf, daemon/        the daemon's configuration, identity, peers,
#                               state, stdout, stderr and diag.jsonl
#    witness-1/, witness-2/     each instrument's identity, peers, stdout,
#                               stderr and diag.jsonl
#    phones/<serial>/           logcat.txt (fueros.diag), provision.json where
#                               provisioned, every event file pulled from the
#                               app's private directory, any report zip and
#                               its contents under report/
#    phones/<serial>.jsonl      the live stream as the collector received it
#                               (its hello first, anchoring the file), and
#                               phones/collector.txt, one line per connect
#                               and disconnect. While the run holds,
#                               `rhtn diag watch runs/<run>` prints each
#                               step, refusal and abort as it arrives
#    merge/                     the files the merge read, one per source; a
#                               phone's copy gains a synthetic anchor built
#                               from its shell.start event when the file has
#                               none of its own
#    timeline.txt, summary.txt  `rhtn diag merge` over merge/*.jsonl
#    checklist.md               the tester's form (tools/field-checklist.py)
#    run.json                   commit, pins, devices, addresses, times
#    build.log, field-run.pid
#
#  Nothing here is a background service: the script holds the foreground
#  and stops what it started on Ctrl-C, SIGTERM or `field-run.sh stop`.
# =============================================================================
set -u
set -o pipefail

HERE="$(cd "$(dirname "$0")" && pwd)"
CRATES="$(dirname "$HERE")"
REPO="$(dirname "$CRATES")"
RUNS="$REPO/runs"
TARGET="$CRATES/target/fieldtest/release"
APP=com.comptus.fueros
DEFAULT_PORT=7447

say() { echo "field-run: $*" >&2; }
PIDS=()
kill_everything() {
  local p
  for p in ${PIDS[@]+"${PIDS[@]}"}; do kill -TERM "$p" 2> /dev/null || true; done
  sleep 0.5
  for p in ${PIDS[@]+"${PIDS[@]}"}; do kill -KILL "$p" 2> /dev/null || true; done
  if [ -n "${RUN:-}" ] && [ -f "$RUNS/current" ] && [ "$(cat "$RUNS/current")" = "$RUN" ]; then rm -f "$RUNS/current"; fi
}
die() {
  say "$*"
  kill_everything
  exit 1
}
# a write to an instrument that has gone is reported, not fatal
trap '' PIPE
# until the orderly stop is installed, an interruption kills what was started
trap 'die "interrupted"' INT TERM
need() {
  command -v "$1" > /dev/null 2>&1 || die "$1 is not installed or not on PATH; $2"
}

# adb: on PATH, or under the SDK the usual variables or directory name
find_adb() {
  if command -v adb > /dev/null 2>&1; then command -v adb; return 0; fi
  local d
  for d in "${ANDROID_HOME:-}" "${ANDROID_SDK_ROOT:-}" "$HOME/Android/Sdk"; do
    if [ -n "$d" ] && [ -x "$d/platform-tools/adb" ]; then echo "$d/platform-tools/adb"; return 0; fi
  done
  return 1
}

# ---------------------------------------------------------------- stop ------
if [ "${1:-}" = "stop" ]; then
  [ -f "$RUNS/current" ] || { say "no run is in progress ($RUNS/current is absent)"; exit 1; }
  dir="$(cat "$RUNS/current")"
  pid="$(cat "$dir/field-run.pid" 2> /dev/null || true)"
  if [ -z "$pid" ] || ! kill -0 "$pid" 2> /dev/null; then
    say "the run's script is not running any more; its directory is $dir"
    rm -f "$RUNS/current"
    exit 1
  fi
  say "stopping run in $dir (pid $pid)"
  # SIGTERM, not SIGINT: a script started as a background job has SIGINT
  # ignored and cannot trap it, and the stop must work there too
  kill -TERM "$pid"
  for _ in $(seq 1 240); do
    kill -0 "$pid" 2> /dev/null || break
    sleep 0.5
  done
  if kill -0 "$pid" 2> /dev/null; then say "pid $pid did not stop within 120 s"; exit 1; fi
  echo "$dir"
  exit 0
fi

# ------------------------------------------------------------- arguments -----
ADDR=""
LEVEL=debug
STREAM_PORT=7448
RETENTION=""
BUILD=yes
PHONE_SERIALS=()
PHONE_MATERIALS=()
LABEL=""
while [ $# -gt 0 ]; do
  case "$1" in
    --addr) ADDR="${2:-}"; shift 2 ;;
    --level) LEVEL="${2:-}"; shift 2 ;;
    --stream-port) STREAM_PORT="${2:-}"; shift 2 ;;
    --retention) RETENTION="${2:-}"; shift 2 ;;
    --no-build) BUILD=no; shift ;;
    --phone)
      p="${2:-}"
      # a trailing newline or a copy with upper-case hex is forgiven here
      p="$(printf '%s' "$p" | tr -d '\r\n' | tr 'A-F' 'a-f')"
      case "$p" in
        *=*) PHONE_SERIALS+=("${p%%=*}"); PHONE_MATERIALS+=("${p#*=}") ;;
        *) say "--phone takes <serial>=<hex material>"; exit 2 ;;
      esac
      shift 2 ;;
    -h | --help) awk '/^# =+$/ {n++; if (n == 2) exit; next} n == 1 {sub(/^# ?/, ""); print}' "$0"; exit 0 ;;
    -*) say "unknown option $1"; exit 2 ;;
    *)
      if [ -n "$LABEL" ]; then say "one label only"; exit 2; fi
      LABEL="$1"; shift ;;
  esac
done
[ -n "$LABEL" ] || { say "usage: field-run.sh [options] <label> | field-run.sh stop"; exit 2; }
# no --phone: the phones file field-setup.sh writes
if [ ${#PHONE_SERIALS[@]} -eq 0 ] && [ -f "$HERE/phones.conf" ]; then
  while IFS= read -r line; do
    line="${line%%#*}"; line="$(printf '%s' "$line" | tr -d ' \t\r')"
    [ -n "$line" ] || continue
    case "$line" in
      *=) say "phones.conf: ${line%=} has no material yet (run field-setup.sh); skipped" ;;
      *=*) PHONE_SERIALS+=("${line%%=*}"); PHONE_MATERIALS+=("$(printf '%s' "${line#*=}" | tr 'A-F' 'a-f')") ;;
      *) say "phones.conf: '$line' is not <serial>=<material>; skipped" ;;
    esac
  done < "$HERE/phones.conf"
  [ ${#PHONE_SERIALS[@]} -eq 0 ] || say "phones from phones.conf: ${PHONE_SERIALS[*]}"
fi
case "$LABEL" in
  *[!A-Za-z0-9_.-]*) say "the label is letters, digits, '.', '_' and '-'"; exit 2 ;;
esac
case "$LEVEL" in off | error | warn | info | debug | trace) ;; *) say "--level is off, error, warn, info, debug or trace"; exit 2 ;; esac
case "$STREAM_PORT" in '' | *[!0-9]*) say "--stream-port is a port number, 0 for none"; exit 2 ;; esac
[ "$STREAM_PORT" -le 65535 ] || { say "--stream-port is at most 65535"; exit 2; }
case "$RETENTION" in '' | [1-9]*) ;; *) say "--retention is a whole number of years, at least 1"; exit 2 ;; esac
case "$RETENTION" in *[!0-9]*) say "--retention is a whole number of years, at least 1"; exit 2 ;; esac
for m in ${PHONE_MATERIALS[@]+"${PHONE_MATERIALS[@]}"}; do
  case "$m" in *[!0-9a-f]*) say "a phone's material is lower-case hex"; exit 2 ;; esac
done

# ------------------------------------------------------- what is running -----
LOGCAT_PIDS=()
DAEMON_PID=""
COLLECTOR_PID=""
W_PID=("" "" "")
W_FD=("" "" "")
STOPPING=no

# wait for a process to leave, up to a number of half-seconds
await_exit() {
  local pid="$1" halves="$2"
  while [ "$halves" -gt 0 ]; do
    kill -0 "$pid" 2> /dev/null || return 0
    sleep 0.5
    halves=$((halves - 1))
  done
  return 1
}

# wait for a file to contain a string, up to a number of half-seconds; fail
# early when the process it belongs to is gone
await_text() {
  local file="$1" text="$2" halves="$3" pid="$4"
  while [ "$halves" -gt 0 ]; do
    grep -q -- "$text" "$file" 2> /dev/null && return 0
    kill -0 "$pid" 2> /dev/null || return 1
    sleep 0.5
    halves=$((halves - 1))
  done
  return 1
}

# ----------------------------------------------------------------- tools -----
need git "the run is named by the commit"
need cargo "the daemon and instrument are built with it"
need python3 "the checklist, run.json and the pull need it"
need awk "the merge output is split with it"
ADB="$(find_adb)" || die "adb is not on PATH and no SDK was found under \$ANDROID_HOME, \$ANDROID_SDK_ROOT or ~/Android/Sdk"
if [ -z "$ADDR" ]; then
  need ip "or pass --addr <ip[:port]>"
  ip4="$(ip -4 route get 1.1.1.1 2> /dev/null | awk '{for (i = 1; i <= NF; i++) if ($i == "src") print $(i + 1)}' | head -1)"
  [ -n "$ip4" ] || die "no routed IPv4 address found; pass --addr <ip[:port]>"
  ADDR="$ip4:$DEFAULT_PORT"
fi
case "$ADDR" in *:*) ;; *) ADDR="$ADDR:$DEFAULT_PORT" ;; esac

# ------------------------------------------------------------- the run -------
COMMIT="$(git -C "$REPO" rev-parse --short HEAD)" || die "not a git tree"
DIRTY=false
[ -z "$(git -C "$REPO" status --porcelain 2> /dev/null)" ] || DIRTY=true
mkdir -p "$RUNS"
if [ -f "$RUNS/current" ] && [ -f "$(cat "$RUNS/current")/field-run.pid" ] \
  && kill -0 "$(cat "$(cat "$RUNS/current")/field-run.pid")" 2> /dev/null; then
  die "a run is already in progress: $(cat "$RUNS/current"); stop it first"
fi
N=0
for d in "$RUNS/$COMMIT-$LABEL-"*; do
  [ -d "$d" ] || continue
  k="${d##*-}"
  case "$k" in *[!0-9]*) continue ;; esac
  [ "$k" -gt "$N" ] && N="$k"
done
N=$((N + 1))
RUN_NAME="$COMMIT-$LABEL-$N"
RUN="$RUNS/$RUN_NAME"
mkdir -p "$RUN/daemon" "$RUN/witness-1" "$RUN/witness-2" "$RUN/phones" "$RUN/merge"
echo $$ > "$RUN/field-run.pid"
echo "$RUN" > "$RUNS/current"
STARTED_AT="$(date -u +%Y-%m-%dT%H:%M:%SZ)"
say "run $RUN_NAME in $RUN"

# ----------------------------------------------------------------- build -----
if [ "$BUILD" = yes ]; then
  say "building the field-test daemon and instrument (cargo, into target/fieldtest)"
  (
    cd "$CRATES" && cargo build --release --target-dir "$CRATES/target/fieldtest" -p rhtn-cli \
      && cargo build --release --target-dir "$CRATES/target/fieldtest" -p rhtn-daemon -p rhtn-participant \
        --no-default-features --features rhtn-daemon/fieldtest,rhtn-participant/fieldtest
  ) > "$RUN/build.log" 2>&1 || { tail -20 "$RUN/build.log" >&2; die "the build failed; see $RUN/build.log"; }
fi
for b in rhtn rhtnd rhtnp; do
  [ -x "$TARGET/$b" ] || die "$TARGET/$b is missing; run without --no-build"
done
# a releasable binary accepts [log] and writes nothing: refuse to run one
grep -a -q 'daemon.lifecycle' "$TARGET/rhtnd" \
  || die "$TARGET/rhtnd is not a field-test build (no event names in it)"

# ------------------------------------------------------------ identities -----
mint() { # dir -> keyhash and material in globals
  "$TARGET/rhtn" keys mint "$1/identity.key" > "$1/identity.txt" || die "minting $1/identity.key"
  KEYHASH="$(awk '$1 == "keyhash" {print $2}' "$1/identity.txt")"
  MATERIAL="$(awk '$1 == "material" {print $2}' "$1/identity.txt")"
  [ -n "$KEYHASH" ] && [ -n "$MATERIAL" ] || die "rhtn keys mint said nothing useful for $1"
}
mint "$RUN/daemon"; DAEMON_KEYHASH="$KEYHASH"; DAEMON_MATERIAL="$MATERIAL"
mint "$RUN/witness-1"; W_KEYHASH1="$KEYHASH"; W_MATERIAL1="$MATERIAL"
mint "$RUN/witness-2"; W_KEYHASH2="$KEYHASH"; W_MATERIAL2="$MATERIAL"
PHONE_KEYHASHES=()
for m in ${PHONE_MATERIALS[@]+"${PHONE_MATERIALS[@]}"}; do
  # a keyhash is SHA-256 over the key material's bytes (wire-format.md §2.2)
  PHONE_KEYHASHES+=("$(python3 -c 'import hashlib, sys; print(hashlib.sha256(bytes.fromhex(sys.argv[1])).hexdigest())' "$m")")
done

# the peers each party authenticates: everybody but itself
{
  echo "$W_MATERIAL1"; echo "$W_MATERIAL2"
  for m in ${PHONE_MATERIALS[@]+"${PHONE_MATERIALS[@]}"}; do echo "$m"; done
} > "$RUN/daemon/peers"
{
  echo "$DAEMON_MATERIAL"; echo "$W_MATERIAL2"
  for m in ${PHONE_MATERIALS[@]+"${PHONE_MATERIALS[@]}"}; do echo "$m"; done
} > "$RUN/witness-1/peers"
{
  echo "$DAEMON_MATERIAL"; echo "$W_MATERIAL1"
  for m in ${PHONE_MATERIALS[@]+"${PHONE_MATERIALS[@]}"}; do echo "$m"; done
} > "$RUN/witness-2/peers"

# ----------------------------------------------------------- the daemon ------
cat > "$RUN/rhtnd.conf" <<EOF
# written by crates/tools/field-run.sh for run $RUN_NAME
identity = "$RUN/daemon/identity.key"
listen = "$ADDR"
queue = "$RUN/daemon/queue"
prekeys = "$RUN/daemon/prekeys"
topology = "$RUN/daemon/topology"
archive = "$RUN/daemon/archive"
heartbeat = 30
ingestion = "unverified-gossip"

[allowance]
requests = 120
seconds = 60

[log]
path = "$RUN/daemon/diag.jsonl"
level = "$LEVEL"
EOF
"$TARGET/rhtnd" "$RUN/rhtnd.conf" "$RUN/daemon/peers" > "$RUN/daemon/stdout.txt" 2> "$RUN/daemon/stderr.txt" &
DAEMON_PID=$!
PIDS+=("$DAEMON_PID")
if ! await_text "$RUN/daemon/stdout.txt" "serving on" 60 "$DAEMON_PID"; then
  cat "$RUN/daemon/stderr.txt" >&2
  die "rhtnd did not come up on $ADDR"
fi
say "rhtnd $(head -1 "$RUN/daemon/stdout.txt" | sed 's/^rhtnd: //'), keyhash ${DAEMON_KEYHASH:0:16}..."

# ---------------------------------------------------------- the witnesses ----
start_witness() { # n
  local n="$1" d="$RUN/witness-$1" fd
  rm -f "$d/stdin"
  mkfifo "$d/stdin" || die "mkfifo $d/stdin"
  "$TARGET/rhtnp" --log "$d/diag.jsonl" "$d/identity.key" "$d/peers" < "$d/stdin" > "$d/stdout.txt" 2> "$d/stderr.txt" &
  W_PID[n]=$!
  PIDS+=("${W_PID[n]}")
  # the writing end, held open for the run: the instrument reads commands
  # from it until `quit`
  exec {fd}> "$d/stdin"
  W_FD[n]=$fd
  echo "attach $DAEMON_KEYHASH $ADDR" >&"$fd"
  if ! await_text "$d/stdout.txt" "^end" 60 "${W_PID[n]}"; then
    cat "$d/stderr.txt" >&2
    die "witness-$n did not answer its attach"
  fi
  say "witness-$n: $(grep -m1 -E '^(attached|error)' "$d/stdout.txt")"
}
start_witness 1
start_witness 2

# -------------------------------------------------------- the collector -----
# the phones' live streams land here while the run holds; the pull at stop
# is still the record (`Robot/field-test-procedure.md`)
STREAM_ADDR=""
if [ "$STREAM_PORT" != 0 ]; then
  STREAM_ADDR="${ADDR%:*}:$STREAM_PORT"
  "$TARGET/rhtn" diag collect --listen "0.0.0.0:$STREAM_PORT" --into "$RUN/phones" > "$RUN/phones/collector.txt" 2>&1 &
  COLLECTOR_PID=$!
  PIDS+=("$COLLECTOR_PID")
  if ! await_text "$RUN/phones/collector.txt" "listening on" 20 "$COLLECTOR_PID"; then
    cat "$RUN/phones/collector.txt" >&2
    die "rhtn diag collect did not come up on port $STREAM_PORT"
  fi
  say "collector: the phones stream to $STREAM_ADDR -> phones/<serial>.jsonl (rhtn diag watch $RUN)"
fi

# --------------------------------------------------------------- phones ------
mapfile -t DEVICES < <("$ADB" devices 2> /dev/null | awk 'NR > 1 && $2 == "device" {print $1}')
DEVICE_MODELS=()
DEVICE_ANDROID=()
DEVICE_PROVISIONED=()
material_of() { # serial -> its index among the --phone arguments
  local i=0
  while [ "$i" -lt "${#PHONE_SERIALS[@]}" ]; do
    if [ "${PHONE_SERIALS[i]}" = "$1" ]; then echo "$i"; return 0; fi
    i=$((i + 1))
  done
  return 1
}
for s in ${PHONE_SERIALS[@]+"${PHONE_SERIALS[@]}"}; do
  found=no
  for d in ${DEVICES[@]+"${DEVICES[@]}"}; do [ "$d" = "$s" ] && found=yes; done
  [ "$found" = yes ] || die "--phone $s: no such device in 'adb devices'"
done
for s in ${DEVICES[@]+"${DEVICES[@]}"}; do
  mkdir -p "$RUN/phones/$s"
  model="$("$ADB" -s "$s" shell getprop ro.product.manufacturer 2> /dev/null | tr -d '\r') $("$ADB" -s "$s" shell getprop ro.product.model 2> /dev/null | tr -d '\r')"
  android="$("$ADB" -s "$s" shell getprop ro.build.version.release 2> /dev/null | tr -d '\r')"
  DEVICE_MODELS+=("$model")
  DEVICE_ANDROID+=("$android")
  provisioned=false
  if i="$(material_of "$s")"; then
    if ! "$ADB" -s "$s" shell pm path "$APP" 2> /dev/null | grep -q package:; then
      say "$s: $APP is not installed; not provisioned (install the fieldtest APK first)"
    else
      # the peer: the next phone given, or the first witness when alone
      count=${#PHONE_SERIALS[@]}
      if [ "$count" -gt 1 ]; then
        j=$(((i + 1) % count))
        peer_material="${PHONE_MATERIALS[j]}"; peer_keyhash="${PHONE_KEYHASHES[j]}"; peer_name="${PHONE_SERIALS[j]}"
      else
        peer_material="$W_MATERIAL1"; peer_keyhash="$W_KEYHASH1"; peer_name="witness-1"
      fi
      python3 - "$DAEMON_MATERIAL" "$peer_material" "${PHONE_MATERIALS[i]}" "$DAEMON_KEYHASH" "$ADDR" "$peer_keyhash" "$peer_name" "$RETENTION" > "$RUN/phones/$s/provision.json" <<'PY'
import json, sys
node_m, peer_m, own_m, node, addr, peer, name, retention = sys.argv[1:9]
blob = {"known": [node_m, peer_m, own_m], "node": node, "addr": addr,
        "peer": peer, "peer_name": name}
if retention:
    blob["retention_years"] = int(retention)
print(json.dumps(blob, separators=(",", ":")))
PY
      blob="$(cat "$RUN/phones/$s/provision.json")"
      # the launcher stores the extras and the kernel reads them on its
      # next cold start, so: stop, start with the extras, stop, start. The
      # stream target (diag_stream, diag_serial) is stored the same way and
      # read by the fieldtest flavour alone; `off` forgets an earlier one
      "$ADB" -s "$s" shell am force-stop "$APP"
      "$ADB" -s "$s" shell am start -W -n "$APP/.HomeActivity" --es provision "'$blob'" \
        --es diag_stream "${STREAM_ADDR:-off}" --es diag_serial "$s" > /dev/null 2>&1 \
        || say "$s: am start with the provision failed"
      sleep 3
      "$ADB" -s "$s" shell am force-stop "$APP"
      "$ADB" -s "$s" shell am start -W -n "$APP/.HomeActivity" > /dev/null 2>&1 || say "$s: am start failed"
      provisioned=true
      say "$s: provisioned to attach at $ADDR with peer $peer_name${STREAM_ADDR:+, streaming to $STREAM_ADDR}"
    fi
  fi
  DEVICE_PROVISIONED+=("$provisioned")
  "$ADB" -s "$s" logcat -c 2> /dev/null || true
  "$ADB" -s "$s" logcat -s fueros.diag > "$RUN/phones/$s/logcat.txt" 2>&1 &
  LOGCAT_PIDS+=("$!")
  PIDS+=("$!")
  say "$s ($model, Android $android): logcat fueros.diag -> phones/$s/logcat.txt"
done
[ ${#DEVICES[@]} -gt 0 ] || say "no Android device attached; the run is daemon and witnesses alone"

# ------------------------------------------------------- the checklist -------
python3 "$HERE/field-checklist.py" --run "$RUN_NAME" -o "$RUN/checklist.md" || die "the checklist generator failed"

# -------------------------------------------------------------- run.json -----
write_run_json() { # stopped_at or empty
  python3 - "$RUN/run.json" <<'PY'
import json, os, sys
env = os.environ
pins = json.load(open(env["SPEC_PINS"], encoding="utf-8"))
devices = []
for s, m, a, p in zip(env["D_SERIALS"].split("\n"), env["D_MODELS"].split("\n"),
                      env["D_ANDROID"].split("\n"), env["D_PROV"].split("\n")):
    if s:
        devices.append({"serial": s, "model": m, "android": a, "provisioned": p == "true"})
out = {
    "run": env["RUN_NAME"], "label": env["LABEL"], "n": int(env["N"]),
    "commit": env["COMMIT"], "dirty": env["DIRTY"] == "true",
    "spec_pins": pins.get("specs", {}), "pins_accepted": pins.get("accepted"),
    "started_at": env["STARTED_AT"], "stopped_at": env["STOPPED_AT"] or None,
    "daemon": {"addr": env["ADDR"], "keyhash": env["DAEMON_KEYHASH"], "log_level": env["LEVEL"]},
    "stream": {"addr": env["STREAM_ADDR"] or None, "lossy": True, "record": "phones/<serial>/"},
    "witnesses": [{"name": "witness-1", "keyhash": env["W_KEYHASH1"]},
                  {"name": "witness-2", "keyhash": env["W_KEYHASH2"]}],
    "devices": devices,
}
json.dump(out, open(sys.argv[1], "w", encoding="utf-8"), indent=1)
PY
}
export_run_json_env() {
  export SPEC_PINS="$CRATES/spec-pins.json" RUN_NAME LABEL N COMMIT DIRTY STARTED_AT ADDR DAEMON_KEYHASH LEVEL W_KEYHASH1 W_KEYHASH2 STREAM_ADDR
  export STOPPED_AT="${1:-}"
  D_SERIALS="$(printf '%s\n' ${DEVICES[@]+"${DEVICES[@]}"})"; export D_SERIALS
  D_MODELS="$(printf '%s\n' ${DEVICE_MODELS[@]+"${DEVICE_MODELS[@]}"})"; export D_MODELS
  D_ANDROID="$(printf '%s\n' ${DEVICE_ANDROID[@]+"${DEVICE_ANDROID[@]}"})"; export D_ANDROID
  D_PROV="$(printf '%s\n' ${DEVICE_PROVISIONED[@]+"${DEVICE_PROVISIONED[@]}"})"; export D_PROV
}
export_run_json_env ""
write_run_json

# ------------------------------------------------------------------ stop -----
pull_phone() { # serial
  local s="$1" dir="$RUN/phones/$1" f
  mkdir -p "$dir"
  # the app's private directory, readable through run-as on a debug build
  if ! "$ADB" -s "$s" shell run-as "$APP" ls files/diag > "$dir/ls.txt" 2>&1; then
    say "$s: run-as $APP failed ($(tr -d '\r' < "$dir/ls.txt" | head -1)); nothing pulled"
    return
  fi
  while IFS= read -r f; do
    f="${f%$'\r'}"
    case "$f" in
      *.jsonl) "$ADB" -s "$s" exec-out run-as "$APP" cat "files/diag/$f" > "$dir/$f" 2> /dev/null \
        && say "$s: pulled $f ($(wc -l < "$dir/$f") lines)" ;;
    esac
  done < "$dir/ls.txt"
  "$ADB" -s "$s" shell run-as "$APP" ls files/diag/report 2> /dev/null | tr -d '\r' | while IFS= read -r f; do
    case "$f" in
      *.zip)
        "$ADB" -s "$s" exec-out run-as "$APP" cat "files/diag/report/$f" > "$dir/$f" 2> /dev/null || continue
        mkdir -p "$dir/report"
        python3 -c 'import sys, zipfile; zipfile.ZipFile(sys.argv[1]).extractall(sys.argv[2])' "$dir/$f" "$dir/report" \
          && say "$s: pulled report $f ($(ls "$dir/report/events" 2> /dev/null | wc -l) event files)" ;;
    esac
  done
  rm -f "$dir/ls.txt"
}

# a phone's file carries no anchor of its own: build one from shell.start,
# whose wall_ms is the wall clock at the shell's ms 0, so the merge places
# its lines (the kernel's count from Participant.start and sit early by
# that offset, which shell.call for Participant.start gives)
anchored_copy() { # src dst
  python3 - "$1" "$2" <<'PY'
import json, sys
src, dst = sys.argv[1:3]
lines = open(src, encoding="utf-8", errors="replace").read().splitlines()
anchored, start = False, None
for l in lines:
    try:
        o = json.loads(l)
    except Exception:
        continue
    if o.get("event") == "diag.anchor" and "unix_ms" in o:
        anchored = True
    if start is None and o.get("event") == "shell.start" and "wall_ms" in o:
        start = o
with open(dst, "w", encoding="utf-8") as out:
    if not anchored and start is not None:
        out.write(json.dumps({"ms": 0, "level": "info", "layer": "diag", "event": "diag.anchor",
                              "unix_ms": start["wall_ms"], "process": "fueros",
                              "synthetic": True}) + "\n")
    for l in lines:
        out.write(l + "\n")
PY
}

stop_run() {
  [ "$STOPPING" = no ] || return
  STOPPING=yes
  trap '' INT TERM
  say "stopping"
  local n p s
  # the witnesses: `quit`, then the writing end closed
  for n in 1 2; do
    if [ -n "${W_FD[n]}" ]; then
      echo quit >&"${W_FD[n]}" 2> /dev/null || true
      eval "exec ${W_FD[n]}>&-"
    fi
    if [ -n "${W_PID[n]}" ] && ! await_exit "${W_PID[n]}" 20; then
      say "witness-$n did not quit; killed"; kill -KILL "${W_PID[n]}" 2> /dev/null || true
    fi
  done
  # the daemon: SIGTERM, and it writes its state back on the way out
  if [ -n "$DAEMON_PID" ]; then
    kill -TERM "$DAEMON_PID" 2> /dev/null || true
    if ! await_exit "$DAEMON_PID" 30; then say "rhtnd did not stop on SIGTERM; killed"; kill -KILL "$DAEMON_PID" 2> /dev/null || true; fi
  fi
  for p in ${LOGCAT_PIDS[@]+"${LOGCAT_PIDS[@]}"}; do kill -TERM "$p" 2> /dev/null || true; done
  # the collector: what it received stays in phones/<serial>.jsonl; the
  # pull below is the record
  if [ -n "$COLLECTOR_PID" ]; then
    kill -TERM "$COLLECTOR_PID" 2> /dev/null || true
    await_exit "$COLLECTOR_PID" 10 || kill -KILL "$COLLECTOR_PID" 2> /dev/null || true
    for f in "$RUN"/phones/*.jsonl; do
      [ -f "$f" ] || continue
      say "stream: $(basename "$f") ($(wc -l < "$f") lines received live)"
    done
  fi
  # the phones
  for s in ${DEVICES[@]+"${DEVICES[@]}"}; do pull_phone "$s"; done
  # the merge
  rm -f "$RUN"/merge/*.jsonl
  cp "$RUN/daemon/diag.jsonl" "$RUN/merge/daemon.jsonl" 2> /dev/null || say "the daemon wrote no diag.jsonl"
  for n in 1 2; do cp "$RUN/witness-$n/diag.jsonl" "$RUN/merge/witness-$n.jsonl" 2> /dev/null || say "witness-$n wrote no diag.jsonl"; done
  for s in ${DEVICES[@]+"${DEVICES[@]}"}; do
    for f in "$RUN/phones/$s/"*.jsonl; do
      [ -f "$f" ] || continue
      anchored_copy "$f" "$RUN/merge/phone-$s-$(basename "$f")"
    done
  done
  if ls "$RUN"/merge/*.jsonl > /dev/null 2>&1; then
    if "$TARGET/rhtn" diag merge "$RUN"/merge/*.jsonl > "$RUN/merge/out.txt" 2> "$RUN/merge/err.txt"; then
      # the tool prints the timeline, a blank line, then the summary, which
      # begins with the line `sources`
      : > "$RUN/timeline.txt"; : > "$RUN/summary.txt"
      awk -v T="$RUN/timeline.txt" -v S="$RUN/summary.txt" \
        'BEGIN {s = 0} /^sources$/ {s = 1} {if (s) print > S; else print > T}' "$RUN/merge/out.txt"
      sed -i -e :a -e '/^\n*$/{$d;N;ba' -e '}' "$RUN/timeline.txt" 2> /dev/null || true
      rm -f "$RUN/merge/out.txt" "$RUN/merge/err.txt"
      say "timeline.txt: $(wc -l < "$RUN/timeline.txt") events; summary.txt written"
    else
      cat "$RUN/merge/err.txt" >&2
      say "rhtn diag merge failed"
    fi
  fi
  export_run_json_env "$(date -u +%Y-%m-%dT%H:%M:%SZ)"
  write_run_json
  rm -f "$RUNS/current" "$RUN/field-run.pid" "$RUN"/witness-*/stdin
  say "done"
  echo "$RUN"
  exit 0
}
trap stop_run INT TERM

say "holding; Ctrl-C or 'field-run.sh stop' ends the run and collects it${STREAM_ADDR:+; '$TARGET/rhtn diag watch $RUN' follows it live}"
while :; do
  sleep 1
  if [ -n "$DAEMON_PID" ] && ! kill -0 "$DAEMON_PID" 2> /dev/null; then
    say "rhtnd exited on its own: $(tail -3 "$RUN/daemon/stderr.txt" | tr '\n' ' ')"
    DAEMON_PID=""
    stop_run
  fi
done
