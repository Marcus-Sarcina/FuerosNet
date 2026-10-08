#!/usr/bin/env bash
# =============================================================================
#  smoke.sh -- the image, booted in each of the two shapes a node takes.
#
#    packaging/smoke.sh [<image>]
#
#  Four assertions, each a thing an operator would otherwise find out on a
#  rented host:
#
#    1. No configuration is a refusal naming what is missing, not a node
#       serving under choices nobody made (`infra-client-requirements.md` §1).
#    2. A node holding its seed comes up and says what it serves on.
#    3. An INSTANCE holding no seed (design §23.3) mints its transport key,
#       publishes the public half, and waits for the run its operator's client
#       signs over that key (`infra-client-requirements.md` §7).  That is the
#       enrolment handshake's node half, and the public half it prints is what
#       the client signs.
#    4. The transport key it minted is readable by its owner alone.
#
#  **The two that serve run detached.**  A node that is working does not exit,
#  so each is started with its output to the engine's log, waited for by the
#  line it is expected to print, and removed.
# =============================================================================
set -euo pipefail

IMAGE="${1:-${RHTN_IMAGE:-rhtnd}:latest}"
ENGINE="${ENGINE:-}"
if [ -z "$ENGINE" ]; then
  for c in podman docker; do command -v "$c" > /dev/null 2>&1 && ENGINE="$c" && break; done
fi
[ -n "$ENGINE" ] || { echo "smoke.sh: no podman or docker" >&2; exit 2; }

WORK="$(mktemp -d)"
NAMES=()
cleanup() {
  for n in ${NAMES[@]+"${NAMES[@]}"}; do "$ENGINE" rm -f "$n" > /dev/null 2>&1 || true; done
  # the node's own state directories are 0700 and its uid's, so this host
  # cannot unlink what is inside them: the removal goes through a container
  "$ENGINE" run --rm --user 0:0 -v "$WORK:/w" --entrypoint /bin/sh "$IMAGE" \
    -c 'rm -rf /w/*' > /dev/null 2>&1 || true
  rm -rf "$WORK"
}
trap cleanup EXIT

fail=0
ok()  { echo "  ok   $*"; }
bad() { echo "  FAIL $*"; fail=1; }

# the image runs as its own uid and the state volume is its to write; a bind
# mount from this host is opened wide for the length of this test alone
state() { local d="$WORK/$1"; mkdir -p "$d"; chmod 0777 "$d"; echo "$d"; }

# `rhtn` inside the image, for the identities the configurations name
minting() { "$ENGINE" run --rm --entrypoint /usr/local/bin/rhtn -v "$1:/var/lib/rhtn" "$IMAGE" "${@:2}"; }

# start detached, and wait up to `secs` for `want` in the log
awaiting() {
  local name="$1" dir="$2" want="$3" secs="${4:-25}"
  NAMES+=("$name")
  "$ENGINE" run -d --name "$name" -v "$dir:/var/lib/rhtn" "$IMAGE" > /dev/null
  local i=0
  while [ "$i" -lt "$((secs * 2))" ]; do
    if "$ENGINE" logs "$name" 2>&1 | grep -q -- "$want"; then return 0; fi
    sleep 0.5; i=$((i + 1))
  done
  return 1
}
logs() { "$ENGINE" logs "$1" 2>&1 | tail -5; }

echo "smoke.sh: $IMAGE under $ENGINE"

# ---- 1. no configuration is a named refusal --------------------------------
d="$(state empty)"
out="$("$ENGINE" run --rm -v "$d:/var/lib/rhtn" "$IMAGE" 2>&1 || true)"
if grep -q "no configuration at" <<< "$out" && grep -q "transport-key" <<< "$out"; then
  ok "no configuration: refused, naming what an instance must have"
else
  bad "expected a refusal naming the fields; got: $(tail -3 <<< "$out")"
fi

conf() {  # conf <dir> <head lines...>
  local d="$1"; shift
  { printf '%s\n' "$@"
    cat <<'TAIL'
listen = "0.0.0.0:7447"
queue = "/var/lib/rhtn/queue"
prekeys = "/var/lib/rhtn/prekeys"
topology = "/var/lib/rhtn/topology"
archive = "/var/lib/rhtn/archive"
heartbeat = 30
ingestion = "unverified-gossip"

[allowance]
requests = 120
seconds = 60
TAIL
  } > "$d/rhtnd.conf"
}

# ---- 2. a node holding its seed serves ------------------------------------
d="$(state seeded)"
if ! minting "$d" keys mint /var/lib/rhtn/identity.key > "$d/identity.txt" 2>&1; then
  bad "could not mint an identity in the image: $(tail -2 "$d/identity.txt")"
else
  conf "$d" 'identity = "/var/lib/rhtn/identity.key"'
  if awaiting rhtn-smoke-seeded "$d" "serving on 0.0.0.0:7447"; then
    ok "a node holding its seed serves, and says where"
  else
    bad "seeded node did not come up: $(logs rhtn-smoke-seeded)"
  fi
fi

# ---- 3 and 4. an instance mints its key and waits for its run -------------
d="$(state instance)"
if ! minting "$d" keys mint /var/lib/rhtn/operator.key > "$d/operator.txt" 2>&1; then
  bad "could not mint an operator identity: $(tail -2 "$d/operator.txt")"
else
  awk '$1 == "material" {print $2}' "$d/operator.txt" > "$d/operator.material"
  conf "$d" \
    'operator = "/var/lib/rhtn/operator.material"' \
    'transport-key = "/var/lib/rhtn/transport.key"' \
    'delegations = "/var/lib/rhtn/delegations"'
  if awaiting rhtn-smoke-instance "$d" "waiting for the run"; then
    ok "an instance waits for its run rather than serving unenrolled"
  else
    bad "instance did not wait: $(logs rhtn-smoke-instance)"
  fi
  # **`transport.pub`, not `transport.key.pub`**: the daemon writes the public
  # half with `Path::with_extension`, which replaces the extension rather than
  # appending to it, so the name an operator collects is the key's stem
  pub="$(tr -d '\n' < "$d/transport.pub" 2> /dev/null || true)"
  if [ "${#pub}" -ge 64 ] && "$ENGINE" logs rhtn-smoke-instance 2>&1 | grep -q "transport key $pub"; then
    ok "it published the public half to sign over (${pub:0:16}…)"
  else
    bad "no transport public half: transport.pub held '${pub:0:32}'"
  fi
  mode="$(stat -c '%a' "$d/transport.key" 2> /dev/null || echo "?")"
  if [ "$mode" = "600" ]; then
    ok "the transport key it minted is 0600"
  else
    bad "the transport key is $mode, not 0600"
  fi
fi

echo
if [ "$fail" -eq 0 ]; then echo "smoke.sh: the image boots in both shapes"; else echo "smoke.sh: FAILED"; fi
exit "$fail"
