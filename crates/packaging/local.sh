#!/usr/bin/env bash
# =============================================================================
#  local.sh -- a node on this machine, enrolled, with its administration page
#  open to a browser.
#
#    packaging/local.sh up [--port <quic>] [--admin <port>] [--host-shell <port>]
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
#    6. prints the URL of the page the node then serves;
#    7. starts the host shell the node installs packages through.
#
#  **It is the developer's node, and a package goes on it as on a rented
#  one**: `rhtn node install` with the operator key below, the image by
#  digest, and the node asking its host's runner to pull and start it.
#  The host is `Containerfile.hostshell`, a container holding sshd and the
#  runner and taking the runner key alone, beside the node on this
#  machine's network as a launched node is beside its host
#  (`packaging/README.md`, "A package on a node on this machine"). Linux
#  only: it wants the host network a container shares there.
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
SHELL_NAME=rhtn-local-hostshell
SHELL_IMAGE=rhtn-hostshell:latest
QUIC=7447
ADMIN=7449
HOSTSHELL=7450

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
    --host-shell) HOSTSHELL="${2:-}"; shift 2 ;;
    *) echo "local.sh: unknown argument $1" >&2; exit 2 ;;
  esac
done

if [ "$cmd" = down ]; then
  "$ENGINE" rm -f "$NAME" "$SHELL_NAME" > /dev/null 2>&1 || true
  # the packages it started, which outlive it otherwise: the runner names
  # every one `rhtn-pkg-` and nothing else so
  for p in $("$ENGINE" ps -a --filter name=rhtn-pkg- --format '{{.Names}}'); do
    "$ENGINE" rm -f "$p" > /dev/null 2>&1 || true
  done
  # the node's own directories are its uid's and 0700, so they are removed
  # as root inside the image — over the whole of HOME_DIR rather than the
  # instance's directory alone, so residue an earlier layout left above it
  # goes too and `down` is not refused by its own past
  "$ENGINE" run --rm --user 0:0 -v "$HOME_DIR:/s" --entrypoint /bin/sh "$IMAGE" \
    -c 'rm -rf /s/*' > /dev/null 2>&1 || true
  rm -rf "$HOME_DIR"
  say "stopped, and its state removed"
  exit 0
fi
[ "$cmd" = up ] || { echo "local.sh: up or down" >&2; exit 2; }

TARGET="$WORKSPACE/target/release/rhtn"
[ -x "$TARGET" ] || TARGET="$WORKSPACE/target/debug/rhtn"
[ -x "$TARGET" ] || { say "build the cli first: cargo build -p rhtn-cli"; exit 2; }

# **the engine's own socket**, which the host shell drives packages
# through: docker's, or podman's docker-compatible one
case "$ENGINE" in
  podman) SOCKET="$(podman info --format '{{.Host.RemoteSocket.Path}}' 2> /dev/null || true)" ;;
  *) case "${DOCKER_HOST:-}" in
       unix://*) SOCKET="${DOCKER_HOST#unix://}" ;;
       *) SOCKET=/var/run/docker.sock ;;
     esac ;;
esac
[ -S "$SOCKET" ] || { say "no engine socket at '$SOCKET'; for podman, \`systemctl --user start podman.socket\`"; exit 2; }

"$ENGINE" image inspect "$IMAGE" > /dev/null 2>&1 || { say "building the image"; "$HERE/build.sh" > /dev/null; }
"$ENGINE" image inspect "$SHELL_IMAGE" > /dev/null 2>&1 || {
  say "building the host shell"
  "$ENGINE" build -q -f "$HERE/Containerfile.hostshell" -t "$SHELL_IMAGE" "$WORKSPACE" > /dev/null
}
"$ENGINE" rm -f "$NAME" "$SHELL_NAME" > /dev/null 2>&1 || true
mkdir -p "$STATE" "$OPERATOR_DIR" "$HOME_DIR/blocks"
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
# what it hosts, empty until the first install writes an entry: the
# node's own to rewrite, so made as its uid from inside the image
"$ENGINE" run --rm --user 10001:10001 -v "$STATE:/s" --entrypoint /bin/sh "$IMAGE" \
  -c ': > /s/hosting && chmod 0600 /s/hosting'

TOKEN="$(head -c 32 /dev/urandom | od -An -tx1 | tr -d ' \n')"
cat > "$STATE/rhtnd.conf" <<CONF
operator = "/var/lib/rhtn/operator.material"
transport-key = "/var/lib/rhtn/transport.key"
delegations = "/var/lib/rhtn/delegations"
endpoint-record = "/var/lib/rhtn/endpoint.record"
anchor-entry = "/var/lib/rhtn/anchor.entry"
listen = "127.0.0.1:$QUIC"
queue = "/var/lib/rhtn/queue"
keys = "/var/lib/rhtn/keys"
topology = "/var/lib/rhtn/topology"
archive = "/var/lib/rhtn/archive"
grants = "/var/lib/rhtn/grants"
resources = "/var/lib/rhtn/hosting"
state = "/var/lib/rhtn/state"
heartbeat = 30
ingestion = "unverified-gossip"

[allowance]
requests = 120
seconds = 60

[administration]
listen = "127.0.0.1:$ADMIN"
token = "$TOKEN"
host-shell = "127.0.0.1:$HOSTSHELL"
CONF

say "operator $KEYHASH; its key is in $OPERATOR_DIR and is not mounted"
# **on this machine's network, as a launched node is on its host's**: the
# host shell and every package it starts listen on the loopback, and the
# node reaches them there; both of its own ports are bound to 127.0.0.1
"$ENGINE" run -d --name "$NAME" --network=host \
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
  --token "$TOKEN" --endpoint "127.0.0.1:$QUIC"

for _ in $(seq 1 60); do
  "$ENGINE" logs "$NAME" 2>&1 | grep -q "serving on" && break
  sleep 0.5
done
if "$ENGINE" logs "$NAME" 2>&1 | grep -q "serving on"; then
  say "serving. Its administration page: http://127.0.0.1:$ADMIN/"
else
  say "it took the run but is not serving:"; "$ENGINE" logs "$NAME" 2>&1 | tail -8; exit 1
fi

# **the host shell**: root's one login runs the runner, with the line the
# operator's key derives, and the blocks the runner makes are kept here
# under the same path the engine is handed
"$TARGET" keys ssh "$OPERATOR" --runner > "$HOME_DIR/runner.pub"
"$ENGINE" run -d --name "$SHELL_NAME" --network=host --restart=always \
  -e RHTN_HOSTSHELL_PORT="$HOSTSHELL" -e RHTN_BLOCKS="$HOME_DIR/blocks" \
  -v "$SOCKET:/var/run/docker.sock" \
  -v "$HOME_DIR/runner.pub:/etc/rhtn/runner.pub:ro" \
  -v "$HOME_DIR/blocks:$HOME_DIR/blocks" \
  "$SHELL_IMAGE" > /dev/null
for _ in $(seq 1 20); do
  "$ENGINE" logs "$SHELL_NAME" 2>&1 | grep -q "Server listening" && break
  sleep 0.5
done
if "$ENGINE" logs "$SHELL_NAME" 2>&1 | grep -q "Server listening"; then
  say "its host shell answers on 127.0.0.1:$HOSTSHELL. Install with --identity $OPERATOR"
else
  say "the host shell did not start:"; "$ENGINE" logs "$SHELL_NAME" 2>&1 | tail -5; exit 1
fi
