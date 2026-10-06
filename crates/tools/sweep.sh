#!/usr/bin/env bash
# =============================================================================
#  sweep.sh -- bound the build cache, keeping the last few builds
#
#    sweep.sh [options]
#
#  Options:
#    --days N        keep every incremental session touched in the last N
#                    days (default 3).
#    --keep K        and, whatever its age, the K most recent sessions of
#                    each crate (default 2), so a crate nobody has built
#                    this week still rebuilds incrementally.
#    --deps-days D   pass --time D to cargo-sweep for the rest of the
#                    target directory (default 30). Dependency artifacts
#                    are what make a rebuild cheap and they are a hundredth
#                    of the size of the thing above, so they are kept far
#                    longer.
#    --maxsize SIZE  after the above, if the target is still larger than
#                    SIZE, remove the oldest incremental sessions until it
#                    is not. Units as cargo-sweep's: MB unless stated,
#                    e.g. 40GB. Off by default: the age bound is what holds
#                    the size down, and a cap that fights it would prune
#                    sessions the operator just asked to keep.
#    --target DIR    the target directory (default: the one beside this
#                    script's crates/).
#    --wait S        seconds to wait for cargo's lock before giving up
#                    (default 90).
#    --dry-run       say what would go and remove nothing.
#
#  **Why this exists when `cargo sweep` is installed.** cargo-sweep manages
#  the artifacts cargo fingerprints -- `deps/`, `build/`, `.fingerprint/`
#  -- and does not reach `incremental/` at all: with 102 GiB of incremental
#  sessions on this machine, `cargo sweep --maxsize 20GB` offered to free
#  14.5 GiB and could not get near the bound [2026-10-06]. Nothing collects
#  `incremental/`: rustc replaces a session only when the same crate is
#  rebuilt under the same fingerprint, so every feature flavour, every
#  `--all-targets` pass and every clippy run leaves a directory of its own
#  behind for ever. That is the growth, and this prunes it.
#
#  **Only this workspace's own crates are in there.** Cargo builds
#  dependencies without incremental, so nothing here costs a dependency
#  rebuild: what a prune costs is one slower build of our own code, and
#  what it buys is a cache that stops growing.
#
#  Nothing is pruned while a build holds cargo's lock: the sessions this
#  would delete are the ones it is writing. A lock held now is waited for
#  briefly -- an editor's check in the background is seconds, and skipping
#  the prune for it would mean skipping most of them -- and a lock held
#  past that is a build worth leaving alone.
# =============================================================================
set -u
set -o pipefail

HERE="$(cd "$(dirname "$0")" && pwd)"
CRATES="$(dirname "$HERE")"

DAYS=3
KEEP=2
DEPS_DAYS=30
MAXSIZE=""
TARGET="$CRATES/target"
WAIT=90
DRY=no

say() { echo "sweep: $*" >&2; }
die() { say "$*"; exit 1; }

while [ $# -gt 0 ]; do
  case "$1" in
    --days) DAYS="${2:-}"; shift 2 ;;
    --keep) KEEP="${2:-}"; shift 2 ;;
    --deps-days) DEPS_DAYS="${2:-}"; shift 2 ;;
    --maxsize) MAXSIZE="${2:-}"; shift 2 ;;
    --target) TARGET="${2:-}"; shift 2 ;;
    --wait) WAIT="${2:-}"; shift 2 ;;
    --dry-run) DRY=yes; shift ;;
    -h | --help) sed -n '2,46p' "$0" | sed 's/^# \{0,1\}//'; exit 0 ;;
    *) die "unknown option: $1 (--help for what there is)" ;;
  esac
done

case "$DAYS$KEEP$DEPS_DAYS$WAIT" in *[!0-9]*) die "--days, --keep, --deps-days and --wait are whole numbers" ;; esac
[ -d "$TARGET" ] || { echo "sweep: no target directory at $TARGET; nothing to do"; exit 0; }

# ------------------------------------------------------------------ lock -----
# Cargo's own lock, one per profile directory.  Held by any build in
# progress; taken here so a prune and a build cannot overlap in either
# order.  Waited for, because the lock is held by anything that compiles —
# an editor's background check as much as a build — and a prune that gave
# up the instant one was running would be skipped most times it ran.  Past
# the wait it gives up: a build that long is one to leave alone, and the
# next run will prune what this one did not.
LOCKS=()
while IFS= read -r l; do LOCKS+=("$l"); done < <(find "$TARGET" -maxdepth 2 -name .cargo-lock 2>/dev/null)
HELD=()
for l in ${LOCKS[@]+"${LOCKS[@]}"}; do
  # appended to, never truncated: the file is cargo's and the lock is all
  # this wants from it
  exec {fd}>> "$l" || continue
  if flock -w "$WAIT" "$fd"; then
    HELD+=("$fd")
  else
    echo "  build cache: a build has held $l for over ${WAIT}s; nothing pruned"
    exit 0
  fi
done

before_mb=$(du -sm "$TARGET" 2>/dev/null | cut -f1)

# --------------------------------------------------- the session caches ------
# One directory per (crate, fingerprint); deleting a whole one is safe,
# since the next build starts a fresh session in its place.
pruned=0
freed_mb=0
kept=0
for inc in $(find "$TARGET" -maxdepth 3 -type d -name incremental 2>/dev/null); do
  # mtime, size and path per session directory, newest first per crate:
  # the selection is python's because "the K newest of each crate, and
  # everything newer than N days" is two rules over one list
  mapfile -t doomed < <(
    find "$inc" -maxdepth 1 -mindepth 1 -type d -printf '%T@\t%p\n' 2>/dev/null |
      DAYS="$DAYS" KEEP="$KEEP" python3 -c '
import collections, os, re, sys, time

days, keep = int(os.environ["DAYS"]), int(os.environ["KEEP"])
cutoff = time.time() - days * 86400
rows = []
for line in sys.stdin:
    mtime, path = line.rstrip("\n").split("\t", 1)
    rows.append((float(mtime), path))
# the crate is the name without its fingerprint: `rhtn_ffi-1kpez86yptply`
by_crate = collections.defaultdict(list)
for mtime, path in rows:
    crate = re.sub(r"-[0-9a-z]+$", "", os.path.basename(path))
    by_crate[crate].append((mtime, path))
for sessions in by_crate.values():
    sessions.sort(reverse=True)
    # kept: the K newest of this crate, plus any session touched since the
    # cutoff.  Everything else is a build nobody is troubleshooting
    for n, (mtime, path) in enumerate(sessions):
        if n < keep or mtime >= cutoff:
            continue
        print(path)
'
  )
  here=$(find "$inc" -maxdepth 1 -mindepth 1 -type d 2>/dev/null | wc -l)
  kept=$((kept + here - ${#doomed[@]}))
  [ "${#doomed[@]}" -gt 0 ] || continue
  # one `du` for the lot: four thousand of them is minutes of stat calls
  mb=$(printf '%s\0' ${doomed[@]+"${doomed[@]}"} |
    du -sm --files0-from=- 2>/dev/null | awk '{t += $1} END {print t + 0}')
  freed_mb=$((freed_mb + ${mb:-0}))
  pruned=$((pruned + ${#doomed[@]}))
  if [ "$DRY" = no ]; then
    printf '%s\0' ${doomed[@]+"${doomed[@]}"} | xargs -0 rm -rf --
  fi
done

# ----------------------------------------------------- the rest of target ----
# cargo-sweep's own domain: artifacts it can attribute to a build that is
# no longer current.  A failure here is said and not fatal -- the sessions
# above are the growth, and this is the smaller half.
swept="skipped (cargo-sweep is not installed)"
if cargo sweep --version > /dev/null 2>&1; then
  args=(--time "$DEPS_DAYS")
  [ "$DRY" = yes ] && args+=(--dry-run)
  # **the project path, not the target directory**: cargo-sweep reads the
  # manifest to find the target itself, and handed a target directory it
  # finds no project and says nothing
  if out="$(cd "$CRATES" && cargo sweep "${args[@]}" 2>&1)"; then
    swept="$(printf '%s' "$out" | grep -oE '(Would clean|Cleaned):.*' | tail -1)"
    [ -n "$swept" ] || swept="nothing older than ${DEPS_DAYS} days"
  else
    swept="refused: $(printf '%s' "$out" | tr '\n' ' ' | tail -c 120)"
  fi
fi

# ------------------------------------------------------------- the bound -----
if [ -n "$MAXSIZE" ] && cargo sweep --version > /dev/null 2>&1; then
  args=(--maxsize "$MAXSIZE")
  [ "$DRY" = yes ] && args+=(--dry-run)
  (cd "$CRATES" && cargo sweep "${args[@]}" > /dev/null 2>&1) ||
    say "the size bound could not be applied; the age bound above stands"
fi

after_mb=$(du -sm "$TARGET" 2>/dev/null | cut -f1)
verb="pruned"; [ "$DRY" = yes ] && verb="would prune"
echo "  build cache: $verb $pruned incremental session(s), ${freed_mb} MB, keeping $kept (the ${KEEP} newest per crate and anything under ${DAYS} days)"
echo "  cargo sweep: $swept"
echo "  target: ${before_mb} MB -> ${after_mb} MB"

for fd in ${HELD[@]+"${HELD[@]}"}; do flock -u "$fd" 2> /dev/null || true; done
exit 0
