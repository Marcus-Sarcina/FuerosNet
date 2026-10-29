#!/usr/bin/env bash
# =============================================================================
#  nodeless.sh -- a ceremony with no node, which is design §6.4's escape
#  (`Robot/field-test-diagnostics.md`, section 4).
#
#    nodeless.sh [--addr <ip>] [--port <n>] [--phone <serial>]... <label>
#    nodeless.sh stop
#
#  --phone names the two phones by adb serial, for a bench with more than
#  two attached; given none, the two on USB are taken, and more or fewer
#  than two is a refusal.
#
#  `field-run.sh` always provisions a node to attach to, and hands each
#  phone the other's full key material besides. **Both are the harness
#  cheating** [author, 2026-10-07]: the meeting transaction introduces an
#  unknown counterparty without external reference, so scanning the first
#  invitation must be the whole of what one device knows of another.
#
#  So this provisions **nothing about anybody**: no node, no address, no
#  peer, no nominees, and a `known` set that names only the device itself.
#  The shell stays detached (`Kernel.bringUp`) and learns who it is meeting
#  from the invitation code (`Kernel.takeBootstrap`); the kernel pins their
#  `KeyMaterial` from the optical contribution, which carries it in full
#  (`wire-format.md` §14.3.1, design §12.3).
#
#  **What such a pair forms is a witnessless record**, no witnesses and no
#  responses (`wire-format.md` §3.2, design §13.2).  Which *shape* depends
#  on the phones, and **both now work** [author, 2026-10-19: there is no
#  witness floor]:
#
#    * two freshly wiped phones form a **formation** record at their
#      genesis (§3.2 subtype 1);
#    * two phones that have met anyone form a **witnessless normal**
#      record, which says permanently that nobody attested.
#
#  The second used to be refused `NoWitness` and is the reason this script
#  once said both phones must be wiped.  **It does not say that any more**:
#  wiping is now a choice of which shape to prove, and the wipe costs the
#  phones' identities -- which is what the verification series is assembled
#  from, so do not spend them without meaning to.
#
#  Diagnostics still stream: the collector is a plain TCP listener on this
#  machine and has nothing to do with the node.
# =============================================================================
set -u

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT="$(cd "$HERE/../.." && pwd)"
APP=com.comptus.fueros
CHOSEN=()
PORT=7448
ADDR=""
LABEL=""

say() { echo "nodeless: $*" >&2; }
die() { say "$*"; exit 1; }

ADB="${ADB:-}"
if [ -z "$ADB" ]; then
  for c in "$HOME/Android/Sdk/platform-tools/adb" "$(command -v adb || true)"; do
    [ -n "$c" ] && [ -x "$c" ] && ADB="$c" && break
  done
fi
[ -n "$ADB" ] || die "no adb"

while [ $# -gt 0 ]; do
  case "$1" in
    --addr) ADDR="${2:-}"; shift 2 ;;
    --port) PORT="${2:-}"; shift 2 ;;
    --phone) CHOSEN+=("${2:-}"); shift 2 ;;
    stop) LABEL=stop; shift ;;
    *) LABEL="$1"; shift ;;
  esac
done
[ -n "$LABEL" ] || die "usage: nodeless.sh [--addr <ip>] <label>"

RUNS="$ROOT/runs"
CUR="$RUNS/current-nodeless"

if [ "$LABEL" = stop ]; then
  [ -L "$CUR" ] || die "no nodeless run in progress"
  RUN="$(readlink -f "$CUR")"
  if [ -f "$RUN/collector.pid" ]; then
    kill "$(cat "$RUN/collector.pid")" 2> /dev/null || true
  fi
  # the live stream is lossy; the pull is the record
  while read -r s st; do
    [ "$st" = device ] || continue
    case "$s" in *:*) continue ;; esac
    mkdir -p "$RUN/phones/$s"
    "$ADB" -s "$s" exec-out "run-as $APP sh -c 'cd files/diag && tar c .'" 2> /dev/null \
      | tar x -C "$RUN/phones/$s" 2> /dev/null \
      && say "$s: event files pulled"
  done < <("$ADB" devices | sed 1d)
  rm -f "$CUR"
  say "stopped; $RUN"
  echo "$RUN"
  exit 0
fi

if [ -z "$ADDR" ]; then
  ADDR="$(ip -4 -o addr show scope global 2> /dev/null | awk '$2 ~ /^w/ {print $4}' | cut -d/ -f1 | head -1)"
  [ -n "$ADDR" ] || die "could not guess this machine's address; pass --addr"
fi

COMMIT="$(cd "$ROOT" && git rev-parse --short HEAD 2> /dev/null || echo unknown)"
n=1
while [ -e "$RUNS/$COMMIT-$LABEL-$n" ]; do n=$((n + 1)); done
RUN="$RUNS/$COMMIT-$LABEL-$n"
mkdir -p "$RUN/phones"
ln -sfn "$RUN" "$CUR"
say "run $RUN"

# **no key material is read from anywhere.** The provision names nobody,
# so the phones are only addresses to push it to: whatever adb says is
# attached and running the app.
SERIALS=()
if [ "${#CHOSEN[@]}" -gt 0 ]; then
  # named: each must be attached, so a typo is refused here rather than
  # discovered as a provision that went nowhere
  for s in "${CHOSEN[@]}"; do
    "$ADB" devices | awk -v s="$s" '$1 == s && $2 == "device" {f=1} END {exit !f}' \
      || die "--phone $s: not attached as a device (adb devices)"
    SERIALS+=("$s")
  done
  [ "${#SERIALS[@]}" -eq 2 ] || die "need exactly two --phone, have ${#SERIALS[@]}"
else
  while read -r s st; do
    [ "$st" = device ] || continue
    case "$s" in *:*) continue ;; esac
    SERIALS+=("$s")
  done < <("$ADB" devices | sed 1d)
  [ "${#SERIALS[@]}" -eq 2 ] || die "need exactly two phones on USB, have ${#SERIALS[@]}: ${SERIALS[*]:-none}; name two with --phone"
fi

TARGET="$ROOT/crates/target/fieldtest/release"
[ -x "$TARGET/rhtn" ] || die "no $TARGET/rhtn; run field-setup.sh first"
"$TARGET/rhtn" diag collect --listen "0.0.0.0:$PORT" --into "$RUN/phones" > "$RUN/phones/collector.txt" 2>&1 &
echo $! > "$RUN/collector.pid"
sleep 1
say "collector: the phones stream to $ADDR:$PORT -> phones/<serial>.jsonl"

for i in 0 1; do
  s="${SERIALS[$i]}"
  mkdir -p "$RUN/phones/$s"
  # **nothing about anybody at all.** No `node` and no `addr`, so the
  # shell stays detached; no `peer`, so the counterparty comes from the
  # invitation code and nowhere else; no `known`, so the kernel starts
  # having met nobody and pins the first identity it reads off a screen;
  # no nominees, so no witness is asked for. This is a phone out of the
  # box, and it is all a phone out of the box may be told.
  printf '{"known":[],"nominees":[]}' > "$RUN/phones/$s/provision.json"
  blob="$(cat "$RUN/phones/$s/provision.json")"
  "$ADB" -s "$s" shell am force-stop "$APP"
  "$ADB" -s "$s" shell am start -W -n "$APP/.ShellActivity" --es provision "'$blob'" \
    --es diag_stream "$ADDR:$PORT" --es diag_serial "$s" > /dev/null 2>&1 \
    || say "$s: am start with the provision failed"
  sleep 3
  "$ADB" -s "$s" shell am force-stop "$APP"
  "$ADB" -s "$s" shell am start -W -n "$APP/.ShellActivity" > /dev/null 2>&1 || say "$s: am start failed"
  say "$s: provisioned knowing nobody — no node, no peer, no nominees"
done

say "holding; 'nodeless.sh stop' ends it and pulls the event files"
