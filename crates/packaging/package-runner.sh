#!/bin/sh
# =============================================================================
#  rhtn-package-runner -- the one program an instance may run on its host.
#
#  **What this is for** [author, 2026-11-02].  An operator whose only window
#  on their machine is their light client must still be able to install a
#  resource on it, and a socket-backed package has to be *started* by
#  something with control of the machine.  The node cannot be that: its
#  container is given no engine socket and no privilege, because a seized
#  instance must not reach its host.  So the instance asks its own host,
#  over the loopback they share, with a key its operator's client hands it
#  for that one act -- and the host's `authorized_keys` binds that key to
#  this program, which is the whole of what the key can do.
#
#  **So this file is the vocabulary.**  It is deliberately small, it reads
#  its request from SSH_ORIGINAL_COMMAND, it never calls `eval`, and it
#  refuses anything it does not recognise.  Whoever reads it on a host can
#  see exactly what the instance may ask for.
#
#    start <name> <image-ref> [<argument> ...]   run a package's container
#    stop <name>                                 remove it
#    list                                        the packages running
#    logs <name> [<lines>]                       what one said
#    pull <image-ref>                            fetch an image
#    node-restart                                restart the node itself
#
#  **Three things it will not do.**  It will not touch a container whose
#  name it did not make: every package runs as `rhtn-pkg-<name>`, and
#  `node-restart` names the node's own container and takes no argument.  It
#  will not pass a flag to the engine: a package's arguments go after the
#  image reference, where the engine hands them to the container.  And it
#  will not run a shell for anybody.
# =============================================================================
set -eu

PREFIX=rhtn-pkg-
NODE=rhtnd
ENGINE="${RHTN_ENGINE:-podman}"
LOG=/var/log/rhtn-package-runner.log

# **a host that will not take the log is not a failure**: the redirection
# itself is what fails where the file cannot be made, so the whole of it is
# wrapped rather than the printf alone
say() {
  { printf '%s rhtn-package-runner: %s\n' "$(date -Is)" "$*" >> "$LOG"; } 2>/dev/null || true
}
refuse() { say "refused: $*"; echo "rhtn-package-runner: $*" >&2; exit 2; }

# **the request is the client's words, never a shell's.**  `set -f` keeps
# the split from globbing, and nothing below is expanded again.
request="${SSH_ORIGINAL_COMMAND:-}"
[ -n "$request" ] || refuse "no request; this key runs this program and nothing else"
case "$request" in
  *'`'* | *'$('* | *';'* | *'&'* | *'|'* | *'>'* | *'<'* | *'
'*) refuse "a request carries no shell punctuation" ;;
esac
set -f
# shellcheck disable=SC2086
set -- $request
verb="$1"
shift || true

# a name is this runner's to shape: lower case, bounded, and prefixed here
# so no request can name the node's own container or anything else on the
# machine
name_of() {
  case "$1" in
    "" ) refuse "that verb needs a name" ;;
    *[!a-z0-9_-]* ) refuse "a name is lower-case letters, digits, dash and underscore" ;;
  esac
  [ "${#1}" -le 30 ] || refuse "a name is at most 30 characters"
  printf '%s%s' "$PREFIX" "$1"
}

# an image reference: a registry path and a tag or digest, and nothing that
# could be read as a flag
image_of() {
  case "$1" in
    "" ) refuse "that verb needs an image reference" ;;
    -* ) refuse "an image reference is not a flag" ;;
    *[!A-Za-z0-9./:@_-]* ) refuse "an image reference carries unexpected characters" ;;
  esac
  printf '%s' "$1"
}

case "$verb" in
  start)
    name="$(name_of "${1:-}")"; shift
    image="$(image_of "${1:-}")"; shift
    # **every remaining word is the package's own argument**, handed to the
    # container after the image, where the engine does not read flags of
    # its own; each is still checked for a flag that would reach the engine
    for a in "$@"; do
      case "$a" in
        *[!A-Za-z0-9./:=,_@-]* ) refuse "an argument carries unexpected characters: $a" ;;
      esac
    done
    say "start $name $image $*"
    "$ENGINE" rm -f "$name" > /dev/null 2>&1 || true
    "$ENGINE" run -d --name "$name" --restart=always --network=host \
      --read-only --tmpfs /tmp --cap-drop=all --security-opt=no-new-privileges \
      "$image" "$@"
    ;;
  stop)
    name="$(name_of "${1:-}")"
    say "stop $name"
    "$ENGINE" rm -f "$name"
    ;;
  list)
    [ $# -eq 0 ] || refuse "list takes nothing"
    "$ENGINE" ps --filter "name=$PREFIX" --format '{{.Names}} {{.Image}} {{.Status}}'
    ;;
  logs)
    name="$(name_of "${1:-}")"; shift
    lines="${1:-40}"
    case "$lines" in *[!0-9]* ) refuse "a line count is a number" ;; esac
    [ "$lines" -le 500 ] || refuse "at most 500 lines"
    "$ENGINE" logs --tail "$lines" "$name" 2>&1
    ;;
  pull)
    image="$(image_of "${1:-}")"
    say "pull $image"
    "$ENGINE" pull "$image"
    ;;
  node-restart)
    [ $# -eq 0 ] || refuse "node-restart takes nothing"
    say "node-restart"
    "$ENGINE" restart "$NODE"
    ;;
  *)
    refuse "no such verb: $verb"
    ;;
esac
