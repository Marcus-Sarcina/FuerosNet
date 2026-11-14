#!/usr/bin/env bash
# =============================================================================
#  bench-address.sh -- the laptop's address as the bench knows it
#
#    bench-address.sh check          # the laptop's Wi-Fi address equals bench.conf, or exit 1
#    bench-address.sh apply          # re-point the bench to bench.conf: both nodes' configs,
#                                    # every phone's provision, and the agreements the
#                                    # kernels hold for the second node (the fieldtest lever)
#
#  Everything on the bench dials one address, and a DHCP lease moved it
#  once [2026-11-13]. `check` runs before every deploy; `apply` after the
#  laptop is pinned to the address bench.conf names.
# =============================================================================
set -u
HERE="$(cd "$(dirname "$0")" && pwd)"
ROOT="$(cd "$HERE/../.." && pwd)"
# shellcheck disable=SC1091
. "$HERE/bench.conf"
APP=com.comptus.fueros
say() { echo "bench-address: $*" >&2; }
die() { say "$*"; exit 1; }

current() { ip -4 addr show "$LAPTOP_DEV" 2> /dev/null | grep -o 'inet [0-9.]*' | cut -d' ' -f2 | head -1; }

cmd="${1:-check}"
case "$cmd" in
  check)
    now="$(current)"
    if [ "$now" = "$LAPTOP_ADDR" ]; then
      say "the laptop is $LAPTOP_ADDR on $LAPTOP_DEV, as bench.conf says"
      exit 0
    fi
    die "the laptop is '${now:-nothing}' on $LAPTOP_DEV but bench.conf says $LAPTOP_ADDR: pin one to the other (nmcli con mod ... ipv4.addresses, or edit bench.conf and run 'apply') before deploying"
    ;;
  apply)
    "$0" check || exit 1
    ADB="$(command -v adb || echo "$HOME/Android/Sdk/platform-tools/adb")"
    serials=()
    while IFS= read -r line; do
      case "$line" in ''|'#'*) continue ;; esac
      serials+=("${line%%=*}")
    done < "$HERE/phones.conf"
    # 1. the bench node binds the address
    conf="$ROOT/$BENCH_RUN/rhtnd.conf"
    [ -f "$conf" ] || die "no bench node config at $conf"
    sed -i -E "s|^listen = \"[0-9.]+:$BENCH_PORT\"|listen = \"$LAPTOP_ADDR:$BENCH_PORT\"|" "$conf"
    say "bench node config: $(grep '^listen' "$conf")"
    # 2. every phone's provision names it
    for s in "${serials[@]}"; do
      p="$ROOT/$BENCH_RUN/phones/$s/provision.json"
      [ -f "$p" ] || { say "$s: no provision.json under $BENCH_RUN; skipped"; continue; }
      python3 - "$p" "$LAPTOP_ADDR:$BENCH_PORT" <<'PY'
import json, sys
path, addr = sys.argv[1], sys.argv[2]
d = json.load(open(path)); d["addr"] = addr
json.dump(d, open(path, "w"), separators=(",", ":"))
PY
      blob="$(cat "$p")"
      "$ADB" -s "$s" shell am force-stop "$APP"
      "$ADB" -s "$s" shell am start -W -n "$APP/.ShellActivity" --es provision "'$blob'" > /dev/null 2>&1 \
        || say "$s: am start with the provision failed"
      say "$s: provision now names $LAPTOP_ADDR:$BENCH_PORT"
    done
    # 3. the agreements for the second node, where an identity holds one
    #    (the fieldtest lever; the identity in focus is the one that agreed)
    for spec in "${SECOND_AGREED[@]:-}"; do
      [ -n "$spec" ] || continue
      s="${spec%%:*}"; id="${spec#*:}"
      "$ADB" -s "$s" shell am start -W -n "$APP/.ShellActivity" --es active_identity "$id" \
        --es agree_to "$SECOND_NODE@$LAPTOP_ADDR:$SECOND_PORT" > /dev/null 2>&1 \
        || say "$s: the agree_to lever failed"
      say "$s/$id: agreement for the second node now $LAPTOP_ADDR:$SECOND_PORT"
    done
    say "apply done; restart both nodes and the phones' chains will attach"
    ;;
  *) die "check or apply" ;;
esac
