#!/bin/sh
# =============================================================================
#  The image's entrypoint: check what the operator supplied, then become the
#  node.  `exec`, so rhtnd is pid 1 and takes SIGTERM itself — the lifecycle
#  that loses no delivery in flight is the daemon's own and must not be behind
#  a shell that would swallow the signal.
#
#  **IT INVENTS NO CONFIGURATION.**  `rhtn-daemon`'s `Config` has no `Default`
#  on purpose: "a default listen address or queue cap would be a policy choice
#  made by omission, and the operator's obligations are stated as choices"
#  (`infra-client-requirements.md` §1).  An entrypoint that filled in the
#  missing fields would be making those choices where nobody could see them.
#  So a configuration is required, and what writes it is the client's
#  provisioning pages (`infra-client-requirements.md` §8.3).
#
#  What it does do is create the one file the daemon requires to exist and an
#  operator has no reason to write: `peers`, empty, which authenticates nobody
#  in particular.  A node accepts any raw public key from an attaching client
#  (`wire-format.md` §9.1) and learns it there, so an empty peers file is a
#  true statement about a node that has adopted nobody and not a default
#  standing in for a choice.
# =============================================================================
set -eu

STATE="${RHTN_STATE:-/var/lib/rhtn}"
CONF="${RHTN_CONF:-$STATE/rhtnd.conf}"
PEERS="${RHTN_PEERS:-$STATE/peers}"

say() { echo "rhtnd-entrypoint: $*" >&2; }

if [ ! -d "$STATE" ]; then
  say "$STATE is not a directory: mount the state volume there"
  exit 2
fi
if [ ! -w "$STATE" ]; then
  say "$STATE is not writable by uid $(id -u): chown the volume to the image's user"
  exit 2
fi

if [ ! -f "$CONF" ]; then
  say "no configuration at $CONF."
  say "The node cannot supply one: every field is a choice its operator makes"
  say "(infra-client-requirements.md §1), and the pages that compose it ship"
  say "with the client (§8.3). An instance's configuration names:"
  say "  operator, transport-key, delegations   -- design §23.3: no seed here"
  say "  listen, queue, prekeys, topology, archive"
  say "  heartbeat, ingestion, [allowance]"
  say "and, once the operator's client has signed them, endpoint-record and"
  say "anchor-entry (infra-client-requirements.md §4.4)."
  exit 2
fi

if [ ! -f "$PEERS" ]; then
  : > "$PEERS"
  say "created an empty $PEERS: this node authenticates nobody in particular,"
  say "and an attaching client is authenticated by the key it presents."
fi

# **The first boot is the daemon's business, not this script's.**  An instance
# mints its own transport key when the file is absent, writes the public half
# beside it as `.pub`, says so, and then waits for the run its operator's
# client signs over that key (`rhtn-daemon`'s `transport_credential` and the
# provisioning loop in `Service::start`; `infra-client-requirements.md` §7).
# Nothing here needs to arrange that, and anything here that tried would be a
# second place the enrolment is described.
exec /usr/local/bin/rhtnd "$CONF" "$PEERS"
