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
#    start <name> <image-ref> [block=<bytes>] [<argument> ...]
#                                                run a package's container,
#                                                with a bounded block of its
#                                                own mounted where it can be
#    stop <name>                                 remove it
#    list                                        the packages running
#    logs <name> [<lines>]                       what one said
#    pull <image-ref>                            fetch an image
#    node-restart                                restart the node itself
#    node-upgrade <image-ref>                    replace the node on a new image
#    version                                     which runner this is
#    runner-refresh                              take a newer runner, signed by
#                                                the operator, on standard input
#
#  **Why `node-upgrade` exists at all** [author, 2026-11-04]: "the node
#  operators do not own their address space and are taking a randomly
#  assigned IP from the cloud provider, meaning that relaunch costs them
#  their reachability until the new IP propagates."  A new node image must
#  therefore be installable on the machine already serving, and
#  `node-restart` cannot do it: it restarts the same container, which holds
#  the image it was created from whatever `pull` has since fetched.
#
#  **It is ordered in the one safe sequence.**  The node is asking for its
#  own container to be destroyed, so it never reads the reply, and a step
#  that failed after the removal would leave a machine with no node and an
#  operator with no shell.  So: pull first and refuse on failure, with the
#  running node untouched; only then replace, detached from this session so
#  the caller's death cannot take the replacement with it.
#
#  **Three things it will not do.**  It will not touch a container whose
#  name it did not make: every package runs as `rhtn-pkg-<name>`, and
#  `node-restart` names the node's own container and takes no argument.  It
#  will not pass a flag to the engine: a package's arguments go after the
#  image reference, where the engine hands them to the container.  And it
#  will not run a shell for anybody.
#
#  **It is the host's, and the node does not rewrite it** [author,
#  2026-11-16].  This file is the limit on what an instance may ask its
#  host, so a newer one is taken only signed by the operator's own key --
#  the one in root's authorized_keys with no forced command, which no
#  instance ever holds -- under the namespace rhtn-runner@1, checked with
#  ssh-keygen before anything is written.  The node carries the bytes and
#  cannot forge the signature; a runner older than this one is refused, so
#  nothing is taken back; and one that does not parse is refused.  A
#  machine whose runner predates this verb cannot take one, and a relaunch
#  gives it the current runner.
#
#  **A block of the package's own, bounded by the filesystem** [author,
#  2026-11-15].  A package on a socket of its own may need a filesystem --
#  ported software, a database -- and `block=<bytes>` after the image asks
#  for one: a directory under RHTN_BLOCKS, kept to that many bytes by an
#  ext4 project quota, mounted at /rhtn-block in the container.  The
#  ceiling is the kernel's and holds on every write.  **Where the host
#  cannot keep one** -- not ext4, not mounted with `prjquota`, no quota
#  tools -- the start goes on without it and the first line of the reply
#  says `block none` and why, so the node tells the package its surface
#  instead; where it can, the line says `block /rhtn-block <bytes>`.  A
#  stop leaves the directory: the block is destroyed by an operator and
#  never by a restart or a reinstall.
# =============================================================================
set -eu

# **which runner this is**: a client compares it with the one it carries
# and tells its operator when a newer one is available
RUNNER_VERSION=2
RUNNER_SELF="${RHTN_RUNNER_SELF:-/usr/local/bin/rhtn-package-runner}"
AUTHORIZED="${RHTN_AUTHORIZED_KEYS:-/root/.ssh/authorized_keys}"
NAMESPACE=rhtn-runner@1

PREFIX=rhtn-pkg-
NODE=rhtnd
ENGINE="${RHTN_ENGINE:-podman}"
LOG=/var/log/rhtn-package-runner.log
BLOCKS="${RHTN_BLOCKS:-/var/lib/rhtn-blocks}"
MOUNT_AT=/rhtn-block

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

# a stable project id per block, from a file beside the blocks
project_id() {
  ids="$BLOCKS/.projects"
  touch "$ids"
  id="$(awk -v n="$1" '$1==n{print $2}' "$ids")"
  if [ -z "$id" ]; then
    last="$(awk 'BEGIN{m=10000}{if($2>m)m=$2}END{print m}' "$ids")"
    id=$((last+1))
    printf '%s %s\n' "$1" "$id" >> "$ids"
  fi
  printf '%s' "$id"
}

# **provision one bounded block, or say why not** (above): prints the
# reply's first line and leaves the mount in BLOCK_VOLUME, empty where
# there is none.  Nothing here refuses the start: a host that cannot keep
# a block is ordinary capacity, said rather than failed.
block_for() {
  BLOCK_VOLUME=""
  bytes="$2"
  case "$bytes" in ''|*[!0-9]*) refuse "block= takes a byte count" ;; esac
  [ "${#bytes}" -le 15 ] || refuse "block= is more bytes than a host has"
  [ "$bytes" -ge 4096 ] || refuse "a block is at least 4096 bytes"
  case "$BLOCKS" in *[!A-Za-z0-9/._-]*) refuse "RHTN_BLOCKS carries unexpected characters" ;; esac
  if ! mkdir -p "$BLOCKS" 2>/dev/null; then echo "block none $BLOCKS cannot be made"; return 0; fi
  chmod 0700 "$BLOCKS"
  fs="$(findmnt -T "$BLOCKS" -no FSTYPE 2>/dev/null || true)"
  opts="$(findmnt -T "$BLOCKS" -no OPTIONS 2>/dev/null || true)"
  mnt="$(findmnt -T "$BLOCKS" -no TARGET 2>/dev/null || true)"
  if [ "$fs" != ext4 ]; then
    echo "block none $BLOCKS is on ${fs:-no filesystem findmnt knows}, and a bounded block wants ext4 mounted with prjquota"; return 0
  fi
  case ",$opts," in *,prjquota,*) ;; *) echo "block none $mnt is not mounted with prjquota, so no ceiling could be kept"; return 0 ;; esac
  if ! command -v chattr > /dev/null 2>&1 || ! command -v setquota > /dev/null 2>&1; then
    echo "block none chattr and setquota are wanted on this host and not installed"; return 0
  fi
  dir="$BLOCKS/$1"
  mkdir -p "$dir"
  id="$(project_id "$1")"
  if ! chattr -p "$id" +P "$dir" 2>/dev/null; then echo "block none the project could not be set on $dir"; return 0; fi
  kb=$(( (bytes + 1023) / 1024 ))
  if ! setquota -P "$id" 0 "$kb" 0 0 "$mnt" 2>/dev/null; then echo "block none the quota could not be set on $mnt"; return 0; fi
  chmod 0777 "$dir"
  BLOCK_VOLUME="$dir:$MOUNT_AT:rw"
  echo "block $MOUNT_AT $bytes"
  return 0
}

case "$verb" in
  start)
    name="$(name_of "${1:-}")"; shift
    image="$(image_of "${1:-}")"; shift
    # **a block, where the package asked for one**: consumed here, never
    # handed to the container as an argument
    BLOCK_VOLUME=""
    case "${1:-}" in
      block=*) block_for "$name" "${1#block=}"; shift ;;
    esac
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
    # shellcheck disable=SC2086
    "$ENGINE" run -d --name "$name" --restart=always --network=host \
      --read-only --tmpfs /tmp --cap-drop=all --security-opt=no-new-privileges \
      ${BLOCK_VOLUME:+-v "$BLOCK_VOLUME"} \
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
  node-upgrade)
    image="$(image_of "${1:-}")"
    shift
    [ $# -eq 0 ] || refuse "node-upgrade takes one image reference"
    say "node-upgrade $image"
    # **the pull comes first and its failure changes nothing.**  A machine
    # whose node was removed before an image was in hand is a machine with
    # no node, reachable only by a shell its operator does not have.
    "$ENGINE" pull "$image" || refuse "that image could not be pulled; the node is untouched"
    # the digest now in hand, so a replacement that will not start can be
    # put back on what was serving a moment ago
    was="$("$ENGINE" inspect --format '{{.Image}}' "$NODE" 2> /dev/null || true)"
    # **detached, because this kills the container that asked for it**: the
    # node's own ssh client dies with its container, and a replacement that
    # died with the session would never be created.  The arguments are
    # `rhtn_launch`'s own, which its cloud-init wrote at creation, and a
    # test holds the two to each other.
    # nothing is interpolated into the script below: the values go through
    # the environment, so an image reference cannot become syntax
    RHTN_UP_ENGINE="$ENGINE" RHTN_UP_NODE="$NODE" RHTN_UP_NEW="$image" \
      RHTN_UP_OLD="$was" setsid sh -c '
      run() {
        "$RHTN_UP_ENGINE" rm -f "$RHTN_UP_NODE" > /dev/null 2>&1 || true
        "$RHTN_UP_ENGINE" run -d --name "$RHTN_UP_NODE" --restart=always \
          --network=host -v /var/lib/rhtn:/var/lib/rhtn "$1" > /dev/null 2>&1
      }
      run "$RHTN_UP_NEW" && exit 0
      # **the replacement would not start, so what was serving goes back.**
      # A machine with no node is reachable by a shell its operator does
      # not have; a machine on its previous image is one they can try
      # again from.  [assistant chose this; the alternative is to leave it
      # down and visibly broken -- the author to rule]
      [ -n "$RHTN_UP_OLD" ] || exit 1
      run "$RHTN_UP_OLD"
    ' < /dev/null > /dev/null 2>&1 &
    say "replacing the node on $image; its surface answers again when it is up"
    ;;
  version)
    [ $# -eq 0 ] || refuse "version takes nothing"
    printf 'runner %s %s\n' "$RUNNER_VERSION" "$(sha256sum "$RUNNER_SELF" 2> /dev/null | cut -c1-64)"
    ;;
  runner-refresh)
    [ $# -eq 0 ] || refuse "runner-refresh takes nothing; the signature and the runner come on standard input"
    command -v ssh-keygen > /dev/null 2>&1 || refuse "ssh-keygen is wanted to check the signature and this host has none; nothing changed"
    work="$(mktemp -d)" || refuse "there is no room to take a runner; nothing changed"
    trap 'rm -rf "$work"' EXIT
    # **the signature, then the runner, byte for byte**: what was signed
    # is what is written, so nothing here rewrites a line of it
    head -c 262144 > "$work/in"
    end="$(grep -n -m1 '^-----END SSH SIGNATURE-----$' "$work/in" | cut -d: -f1)"
    [ -n "$end" ] || refuse "standard input carries a signature and then the runner; nothing changed"
    head -n "$end" "$work/in" > "$work/sig"
    tail -n "+$((end + 1))" "$work/in" > "$work/new"
    [ -s "$work/new" ] || refuse "no runner followed the signature; nothing changed"
    # the operator's own key: the plain line, never the runner key's,
    # which carries a forced command and is the one an instance is handed
    awk '/^ssh-ed25519 /{ print "operator " $1 " " $2 }' "$AUTHORIZED" > "$work/allowed" 2> /dev/null || true
    [ -s "$work/allowed" ] || refuse "this host holds no operator key to check a runner against; nothing changed"
    ssh-keygen -Y verify -f "$work/allowed" -I operator -n "$NAMESPACE" -s "$work/sig" < "$work/new" > /dev/null 2>&1 \
      || refuse "that runner is not signed by this machine's operator; nothing changed"
    sh -n "$work/new" || refuse "that runner does not parse; nothing changed"
    new="$(sed -n 's/^RUNNER_VERSION=\([0-9][0-9]*\)$/\1/p' "$work/new" | head -n 1)"
    [ -n "$new" ] || refuse "that runner names no version; nothing changed"
    [ "$new" -ge "$RUNNER_VERSION" ] || refuse "that runner is version $new and this one is $RUNNER_VERSION, and a runner is not taken back; nothing changed"
    cp "$work/new" "$RUNNER_SELF.new" && chmod 0755 "$RUNNER_SELF.new" && mv -f "$RUNNER_SELF.new" "$RUNNER_SELF" \
      || refuse "the runner could not be written; nothing changed"
    say "runner-refresh $new"
    printf 'runner %s %s\n' "$new" "$(sha256sum "$RUNNER_SELF" | cut -c1-64)"
    ;;
  *)
    refuse "no such verb: $verb"
    ;;
esac
