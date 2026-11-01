#!/usr/bin/env bash
# =============================================================================
#  testapp.sh -- install the utility test resource on a node, local or rented.
#
#    packaging/testapp.sh build  [--tag <tag>]
#    packaging/testapp.sh install [--host <ssh-destination>] [options]
#    packaging/testapp.sh remove  [--host <ssh-destination>] [options]
#    packaging/testapp.sh logs    [--host <ssh-destination>] [--lines <n>]
#
#  Options for install and remove:
#    --state <dir>       the node's state, where its configuration and its
#                        hosting file live. Default: the local node's
#                        (runs/local-node/instance), or /var/lib/rhtn with
#                        --host, which is where a launched instance keeps it.
#    --node <container>  the node's container, restarted so it re-reads the
#                        hosting file. Default rhtn-local-node, or rhtnd
#                        with --host, which is what the launch names it.
#    --port <port>       where the resource listens on the loopback the two
#                        containers share. Default 7460.
#    --owner <keyhash>   who owns the resource. Default: the node's own
#                        operator, read from its operator.material -- which
#                        is the identity whose client provisioned it, so
#                        the resource is reachable by that identity and by
#                        whoever is within two steps of it in this node's
#                        topology (`wire-format.md` §11's membership gate).
#
#  **It needs a shell on the machine**, which a launch now provides: the
#  machine takes the ssh key its operator's client derives from their
#  signing seed (`rhtn_crypto::ssh`, `light-client-requirements.md` §10).
#  Take the private half from the phone -- Operator, *Show this identity's
#  node-control ssh key* -- or from `rhtn keys ssh <identity> --private
#  <file>` where the identity is a file, then
#  `RHTN_SSH='-i <file>' testapp.sh install --host root@<machine>`.
#  A machine launched before that key existed has whatever way in its
#  operator arranged at the provider.
#
#  **What installing a package is** (`infra-client-requirements.md` §9, §10):
#  the operator puts the package where their node can reach it, writes one
#  entry in the hosting file naming the resource, its owner, the authority
#  and the address, writes a grant saying who may reach it, and the node
#  re-reads both. Nothing of it is a protocol act and nothing of it touches
#  the operator's seed. This script is that, done twice over -- on a node on
#  this machine, and on a rented one over ssh -- so the two are the same
#  procedure and not two stories.
#
#  The resource's keyhash is minted once and kept in the node's own state as
#  `testapp.resource`, so a second install finds the same resource and a
#  client that was given the keyhash keeps working.
# =============================================================================
set -euo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
WORKSPACE="$(cd "$HERE/.." && pwd)"
ROOT="$(cd "$WORKSPACE/.." && pwd)"
NAME="${RHTN_TESTAPP_IMAGE:-rhtn-testapp}"
NODE_IMAGE="${RHTN_IMAGE:-rhtnd}:latest"
CONTAINER=rhtn-testapp
PORT=7460
HOST=""
STATE=""
NODE=""
OWNER=""
TAG=""
LINES=40

say() { echo "testapp.sh: $*" >&2; }
die() { echo "testapp.sh: $*" >&2; exit 1; }

ENGINE="${ENGINE:-}"
if [ -z "$ENGINE" ]; then
  for c in podman docker; do command -v "$c" > /dev/null 2>&1 && ENGINE="$c" && break; done
fi
[ -n "$ENGINE" ] || die "no podman or docker on this machine"

cmd="${1:-}"; shift || true
while [ $# -gt 0 ]; do
  case "$1" in
    --host) HOST="${2:-}"; shift 2 ;;
    --state) STATE="${2:-}"; shift 2 ;;
    --node) NODE="${2:-}"; shift 2 ;;
    --port) PORT="${2:-}"; shift 2 ;;
    --owner) OWNER="${2:-}"; shift 2 ;;
    --tag) TAG="${2:-}"; shift 2 ;;
    --lines) LINES="${2:-}"; shift 2 ;;
    -h|--help) sed -n '2,40p' "$0" | sed 's/^# \?//'; exit 0 ;;
    *) die "unknown argument $1" ;;
  esac
done

# the defaults differ by where the node is, and only by that
if [ -n "$HOST" ]; then
  [ -n "$STATE" ] || STATE=/var/lib/rhtn
  [ -n "$NODE" ] || NODE=rhtnd
  # a rented machine gets podman from the launch's cloud-init
  REMOTE_ENGINE=podman
else
  [ -n "$STATE" ] || STATE="$ROOT/runs/local-node/instance"
  [ -n "$NODE" ] || NODE=rhtn-local-node
  REMOTE_ENGINE="$ENGINE"
fi

# ---------------------------------------------------------------- remote -----
# One shell, wherever the node is: locally this runs here, with --host it
# runs there. Every step below is written once because of it.
there() { # command...
  if [ -n "$HOST" ]; then
    # **not BatchMode**: a rented machine launched from a phone has no key
    # of this machine's on it (`rhtn_launch::Plan` carries none), so the
    # operator's shell is the provider's console, a key they added there,
    # or the root password the provider sent them -- and a password prompt
    # has to be able to appear
    ssh ${RHTN_SSH:-} "$HOST" "$@"
  else
    bash -c "$@"
  fi
}

# **Writing into the node's state needs the node's uid**, which is 10001 and
# its directory is 0700 (`packaging/Containerfile`). On a rented machine the
# operator is root and chowns; on this machine nobody is root, so the file is
# written from inside the image as uid 0 over the mounted volume, which is
# what `local.sh down` already does to remove it.
put() { # path mode <<< contents
  local path="$1" mode="${2:-0600}" body
  body="$(cat)"
  if [ -n "$HOST" ]; then
    printf '%s' "$body" | there "umask 077; cat > '$path' && chown 10001:10001 '$path' && chmod $mode '$path'"
  else
    printf '%s' "$body" | "$ENGINE" run --rm -i --user 0:0 -v "$STATE:/s" \
      --entrypoint /bin/sh "$NODE_IMAGE" \
      -c "cat > '/s/$(basename "$path")' && chown 10001:10001 '/s/$(basename "$path")' && chmod $mode '/s/$(basename "$path")'"
  fi
}

get() { # path -> its contents, or empty
  if [ -n "$HOST" ]; then
    there "cat '$1' 2> /dev/null || true"
  else
    "$ENGINE" run --rm --user 0:0 -v "$STATE:/s" --entrypoint /bin/sh "$NODE_IMAGE" \
      -c "cat '/s/$(basename "$1")' 2> /dev/null || true"
  fi
}

run_in_state() { # a shell line run as root over the node's state
  if [ -n "$HOST" ]; then
    there "$1"
  else
    "$ENGINE" run --rm --user 0:0 -v "$STATE:/s" --entrypoint /bin/sh "$NODE_IMAGE" \
      -c "${1//$STATE/\/s}"
  fi
}

# ----------------------------------------------------------------- build -----
build() {
  local commit dirty tag
  commit="$(cd "$ROOT" && git rev-parse --short HEAD 2> /dev/null || echo unknown)"
  dirty=""
  (cd "$ROOT" && git diff --quiet 2> /dev/null) || dirty="-dirty"
  tag="${TAG:-${commit}${dirty}}"
  say "$ENGINE build -> $NAME:$tag"
  "$ENGINE" build -f "$HERE/Containerfile.testapp" -t "$NAME:$tag" -t "$NAME:latest" "$WORKSPACE"
}

have_image() { "$ENGINE" image inspect "$NAME:latest" > /dev/null 2>&1; }

# Whether the node is serving, within twenty seconds of a restart: a
# configuration it refuses is said in its own log and nowhere else.
node_serving() {
  local i
  for i in $(seq 1 40); do
    if there "$REMOTE_ENGINE logs --tail 40 $NODE 2>&1" | grep -q "serving on"; then
      return 0
    fi
    if there "$REMOTE_ENGINE logs --tail 40 $NODE 2>&1" | grep -q "parse error\|unknown field"; then
      return 1
    fi
    sleep 0.5
  done
  return 1
}

# --------------------------------------------------------------- install -----
install_it() {
  have_image || build
  # the resource: minted once and kept with the node, so a second install
  # is the same resource and the keyhash a client holds keeps working
  local resource
  resource="$(get "$STATE/testapp.resource" | tr -d '[:space:]')"
  if [ -z "$resource" ]; then
    local mint
    mint="$(mktemp -d)"
    # as uid 0 over a directory of this machine's: the image's own user
    # cannot write where it was not chowned, and this is a scratch mint
    "$ENGINE" run --rm --user 0:0 -v "$mint:/m" --entrypoint /usr/local/bin/rhtn "$NODE_IMAGE" \
      keys mint /m/resource.key > "$mint/resource.txt" \
      || die "could not mint the resource's identity"
    resource="$(awk '$1 == "keyhash" {print $2}' "$mint/resource.txt")"
    [ -n "$resource" ] || die "the mint said no keyhash"
    printf '%s\n' "$resource" | put "$STATE/testapp.resource" 0600
    rm -rf "$mint"
    say "resource minted: $resource"
  else
    say "resource already installed: $resource"
  fi

  # the owner: this node's own operator unless the caller named another
  local owner="$OWNER"
  if [ -z "$owner" ]; then
    local material
    material="$(get "$STATE/operator.material" | tr -d '[:space:]')"
    [ -n "$material" ] || die "no operator.material in $STATE: is this a node's state directory?"
    owner="$("$ENGINE" run --rm --entrypoint /usr/local/bin/rhtn-testapp "$NAME:latest" keyhash "$material")" \
      || die "could not derive the owner's keyhash from the node's operator material"
  fi
  say "owner: $owner"

  # the manifest, the hosting entry and the grant: the whole of what
  # installing a package is on the node's side
  printf 'roles = ["reader"]\n' | put "$STATE/testapp.manifest" 0600
  local entry
  entry="$(printf '# rhtn-testapp\n[[host]]\nresource = "%s"\nowner = "%s"\nauthority = "testapp.internal"\nmanifest = "/var/lib/rhtn/testapp.manifest"\naddress = "127.0.0.1:%s"\n' "$resource" "$owner" "$PORT")"
  local hosting
  hosting="$(get "$STATE/hosting")"
  case "$hosting" in
    *"# rhtn-testapp"*) hosting="${hosting%%# rhtn-testapp*}" ;;
  esac
  printf '%s%s' "$hosting" "$entry" | put "$STATE/hosting" 0600
  # **a grant with no clause admits every member of the owner's horizon**,
  # which is §10.1's outer gate and nothing further
  run_in_state "mkdir -p '$STATE/grants' '$STATE/state' && chown 10001:10001 '$STATE/grants' '$STATE/state'"
  if [ -n "$HOST" ]; then
    printf '[[grant]]\nroles = ["reader"]\n' | there "umask 077; cat > '$STATE/grants/$resource.toml' && chown 10001:10001 '$STATE/grants/$resource.toml'"
  else
    printf '[[grant]]\nroles = ["reader"]\n' | "$ENGINE" run --rm -i --user 0:0 -v "$STATE:/s" \
      --entrypoint /bin/sh "$NODE_IMAGE" \
      -c "cat > '/s/grants/$resource.toml' && chown 10001:10001 '/s/grants/$resource.toml'"
  fi

  # **a node provisioned before a node could host anything** names neither
  # the hosting file nor the state directory (`rhtn_launch::node_config`
  # gained both on 2026-10-31): add them rather than reprovision, which is
  # what an operator would do by hand and is the one edit to a
  # configuration this script makes
  local conf
  conf="$(get "$STATE/rhtnd.conf")"
  local added=""
  case "$conf" in
    *"resources = "*) ;;
    *) added="resources = \"/var/lib/rhtn/hosting\"" ;;
  esac
  case "$conf" in
    *"state = "*) ;;
    *) added="${added:+$added$'\n'}state = \"/var/lib/rhtn/state\"" ;;
  esac
  if [ -n "$added" ]; then
    # the keys go above the first table, which is where TOML wants them
    printf '%s' "$conf" | awk -v add="$added" '
      !done && /^\[/ { print add; done = 1 }
      { print }
      END { if (!done) print add }
    ' | put "$STATE/rhtnd.conf" 0600
    say "configuration: added the keys a hosting node needs"
  fi

  if [ -n "$HOST" ]; then
    say "sending the image to $HOST"
    "$ENGINE" save "$NAME:latest" | there "$REMOTE_ENGINE load" > /dev/null
  fi
  there "$REMOTE_ENGINE rm -f $CONTAINER > /dev/null 2>&1 || true"

  # **the node re-reads what it hosts**, and it is restarted before the
  # package starts rather than after: where the package shares the node's
  # network namespace (below), restarting the node would take the
  # package's network with it
  there "$REMOTE_ENGINE restart $NODE" > /dev/null || die "could not restart $NODE"
  if ! node_serving; then
    say "$NODE did not come back up. What it said:"
    there "$REMOTE_ENGINE logs --tail 12 $NODE" >&2 || true
    die "the node refused its configuration or did not start; a daemon older than the \`state\` key refuses it, and the cure is a current image"
  fi
  say "$NODE restarted, so it has re-read its hosting file"

  # **the package's network is the node's loopback**, whichever way the
  # node has one: a launched instance runs on the host's network
  # (`rhtn_launch::user_data`), so the package does too; a node that
  # publishes ports instead keeps its own namespace, and the package joins
  # it, since 127.0.0.1 has to mean the same thing to both of them
  local mode
  mode="$(there "$REMOTE_ENGINE inspect -f '{{.HostConfig.NetworkMode}}' $NODE" | tr -d '[:space:]')"
  local network="--network=host"
  case "$mode" in
    host) ;;
    *) network="--network=container:$NODE"
       say "the node keeps its own network namespace ($mode), so the package joins it" ;;
  esac
  there "$REMOTE_ENGINE run -d --name $CONTAINER --restart=always $network $NAME:latest --listen 127.0.0.1:$PORT --resource $resource" > /dev/null
  say "the resource is running as $CONTAINER on 127.0.0.1:$PORT"
  echo
  echo "  resource  $resource"
  echo "  owner     $owner"
  echo
  say "give that resource keyhash to a client of this node's: Catalog, paste it, ask for /"
}

remove_it() {
  there "$REMOTE_ENGINE rm -f $CONTAINER > /dev/null 2>&1 || true"
  local hosting
  hosting="$(get "$STATE/hosting")"
  case "$hosting" in
    *"# rhtn-testapp"*) printf '%s' "${hosting%%# rhtn-testapp*}" | put "$STATE/hosting" 0600 ;;
  esac
  there "$REMOTE_ENGINE restart $NODE" > /dev/null || true
  say "removed, and $NODE restarted without it"
}

case "$cmd" in
  build) build ;;
  install) install_it ;;
  remove) remove_it ;;
  logs) there "$REMOTE_ENGINE logs --tail $LINES $CONTAINER" ;;
  *) die "build, install, remove or logs" ;;
esac
