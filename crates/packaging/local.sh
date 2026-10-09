#!/usr/bin/env bash
# =============================================================================
#  local.sh -- a node on this machine, enrolled, with its administration page
#  open to a browser.
#
#    packaging/local.sh up [--port <quic>] [--admin <port>]
#    packaging/local.sh down
#
#  What it does, which is what a provisioning page will do:
#    1. builds the image if it is not built;
#    2. mints an operator identity, the one that stays off the instance;
#    3. writes a configuration naming that operator, a transport key the
#       instance will mint, and an [administration] surface with a token;
#    4. runs the container, which mints its key and waits;
#    5. `rhtn node enrol` fetches the key, checks the proof, signs a run and
#       the two records, and hands them back;
#    6. prints the URL of the page the node then serves.
#
#  **The operator's key is kept outside the volume**, which is the whole of
#  design §23.3: runs/local-node/operator/ is this machine's and never
#  mounted, runs/local-node/instance/ is what the container sees. An
#  instance that could read its operator's seed would be the arrangement
#  this design exists to avoid.
#
#  `down` and `up` again is a new node with a new identity.
# =============================================================================
set -euo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
WORKSPACE="$(cd "$HERE/.." && pwd)"
ROOT="$(cd "$WORKSPACE/.." && pwd)"
HOME_DIR="$ROOT/runs/local-node"
OPERATOR_DIR="$HOME_DIR/operator"
STATE="$HOME_DIR/instance"
NAME=rhtn-local-node
IMAGE="${RHTN_IMAGE:-rhtnd}:latest"
QUIC=7447
ADMIN=7449

ENGINE="${ENGINE:-}"
if [ -z "$ENGINE" ]; then
  for c in podman docker; do command -v "$c" > /dev/null 2>&1 && ENGINE="$c" && break; done
fi
[ -n "$ENGINE" ] || { echo "local.sh: no podman or docker" >&2; exit 2; }

say() { echo "local.sh: $*" >&2; }

cmd="${1:-up}"; shift || true
while [ $# -gt 0 ]; do
  case "$1" in
    --port) QUIC="${2:-}"; shift 2 ;;
    --admin) ADMIN="${2:-}"; shift 2 ;;
    *) echo "local.sh: unknown argument $1" >&2; exit 2 ;;
  esac
done

if [ "$cmd" = down ]; then
  "$ENGINE" rm -f "$NAME" > /dev/null 2>&1 || true
  # the node's own directories are its uid's and 0700
  "$ENGINE" run --rm --user 0:0 -v "$STATE:/s" --entrypoint /bin/sh "$IMAGE" \
    -c 'rm -rf /s/*' > /dev/null 2>&1 || true
  rm -rf "$HOME_DIR"
  say "stopped, and its state removed"
  exit 0
fi
[ "$cmd" = up ] || { echo "local.sh: up or down" >&2; exit 2; }

TARGET="$WORKSPACE/target/release/rhtn"
[ -x "$TARGET" ] || TARGET="$WORKSPACE/target/debug/rhtn"
[ -x "$TARGET" ] || { say "build the cli first: cargo build -p rhtn-cli"; exit 2; }

"$ENGINE" image inspect "$IMAGE" > /dev/null 2>&1 || { say "building the image"; "$HERE/build.sh" > /dev/null; }
"$ENGINE" rm -f "$NAME" > /dev/null 2>&1 || true
mkdir -p "$STATE" "$OPERATOR_DIR"
chmod 0777 "$STATE"
chmod 0700 "$OPERATOR_DIR"

# **the operator's key is minted outside the volume and stays there.** That
# is the whole of design §23.3: the key that signs as the operator lives on
# the operator's own device, and the instance holds a credential instead. Of
# the operator, the volume gets the public `KeyMaterial` and nothing else.
OPERATOR="$OPERATOR_DIR/operator.key"
if [ ! -f "$OPERATOR" ]; then
  "$TARGET" keys mint "$OPERATOR" > "$OPERATOR_DIR/operator.txt"
fi
MATERIAL="$(awk '$1 == "material" {print $2}' "$OPERATOR_DIR/operator.txt")"
KEYHASH="$(awk '$1 == "keyhash" {print $2}' "$OPERATOR_DIR/operator.txt")"
printf '%s\n' "$MATERIAL" > "$STATE/operator.material"
: > "$STATE/peers"

TOKEN="$(head -c 32 /dev/urandom | od -An -tx1 | tr -d ' \n')"
cat > "$STATE/rhtnd.conf" <<CONF
operator = "/var/lib/rhtn/operator.material"
transport-key = "/var/lib/rhtn/transport.key"
delegations = "/var/lib/rhtn/delegations"
endpoint-record = "/var/lib/rhtn/endpoint.record"
anchor-entry = "/var/lib/rhtn/anchor.entry"
listen = "0.0.0.0:$QUIC"
queue = "/var/lib/rhtn/queue"
prekeys = "/var/lib/rhtn/prekeys"
topology = "/var/lib/rhtn/topology"
archive = "/var/lib/rhtn/archive"
grants = "/var/lib/rhtn/grants"
heartbeat = 30
ingestion = "unverified-gossip"

[allowance]
requests = 120
seconds = 60

[administration]
listen = "0.0.0.0:$ADMIN"
token = "$TOKEN"
CONF

say "operator $KEYHASH; its key is in $OPERATOR_DIR and is not mounted"
"$ENGINE" run -d --name "$NAME" \
  -p "127.0.0.1:$ADMIN:$ADMIN" -p "127.0.0.1:$QUIC:$QUIC/udp" \
  -v "$STATE:/var/lib/rhtn" "$IMAGE" > /dev/null
say "container up; waiting for it to mint a transport key"

for _ in $(seq 1 60); do
  "$ENGINE" logs "$NAME" 2>&1 | grep -q "surface on" && break
  sleep 0.5
done
if ! "$ENGINE" logs "$NAME" 2>&1 | grep -q "surface on"; then
  say "the surface did not open:"; "$ENGINE" logs "$NAME" 2>&1 | tail -5; exit 1
fi

say "enrolling it: fetch the key, check the proof, sign the run and the records"
"$TARGET" node enrol "$OPERATOR" "127.0.0.1:$ADMIN" \
  --token "$TOKEN" --endpoint "127.0.0.1:$QUIC" --anchor 1

for _ in $(seq 1 60); do
  "$ENGINE" logs "$NAME" 2>&1 | grep -q "serving on" && break
  sleep 0.5
done
if "$ENGINE" logs "$NAME" 2>&1 | grep -q "serving on"; then
  say "serving. Its administration page: http://127.0.0.1:$ADMIN/"
else
  say "it took the run but is not serving:"; "$ENGINE" logs "$NAME" 2>&1 | tail -8; exit 1
fi
