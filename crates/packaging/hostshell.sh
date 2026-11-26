#!/bin/sh
# =============================================================================
#  rhtn-hostshell -- sshd on the loopback, taking the runner key alone
#  (`Containerfile.hostshell`).
#
#    RHTN_HOSTSHELL_PORT   where it listens on 127.0.0.1 (default 7450)
#    /etc/rhtn/runner.pub  the restricted line `rhtn keys ssh <operator key>
#                          --runner` printed, mounted read-only
# =============================================================================
set -eu

PORT="${RHTN_HOSTSHELL_PORT:-7450}"
LINE=/etc/rhtn/runner.pub

[ -s "$LINE" ] || { echo "rhtn-hostshell: no runner key at $LINE" >&2; exit 2; }
# **the forced command or nothing**: a line that would log in to a shell is
# refused here, so a mistaken mount cannot turn this into one
grep -q '^command="/usr/local/bin/rhtn-package-runner",' "$LINE" \
  || { echo "rhtn-hostshell: $LINE is not the runner's restricted line" >&2; exit 2; }

mkdir -p /root/.ssh
chmod 0700 /root/.ssh
cp "$LINE" /root/.ssh/authorized_keys
chmod 0600 /root/.ssh/authorized_keys
ssh-keygen -A > /dev/null

# **the runner's environment rides the forced command**: a session is
# given sshd's environment and not this container's, so the engine and the
# blocks are named here, and the runner holds RHTN_BLOCKS to its own
# characters
case "${RHTN_BLOCKS:-}" in *[!A-Za-z0-9/._-]*) echo "rhtn-hostshell: RHTN_BLOCKS carries unexpected characters" >&2; exit 2 ;; esac
RUN="env RHTN_ENGINE=${RHTN_ENGINE:-docker}${RHTN_BLOCKS:+ RHTN_BLOCKS=$RHTN_BLOCKS} /usr/local/bin/rhtn-package-runner"

exec /usr/sbin/sshd -D -e -p "$PORT" \
  -o ListenAddress=127.0.0.1 \
  -o "ForceCommand=$RUN" \
  -o PasswordAuthentication=no \
  -o KbdInteractiveAuthentication=no \
  -o PermitRootLogin=forced-commands-only
