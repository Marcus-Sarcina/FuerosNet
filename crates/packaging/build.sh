#!/usr/bin/env bash
# =============================================================================
#  build.sh -- build the node's image.
#
#    packaging/build.sh [--flavour releasable|fieldtest] [--tag <tag>] [--push]
#
#  Tagged with the commit by default, so an image on a host can be traced back
#  to the tree that produced it; `latest` moves with it.
#
#  --push sends both tags to the registry a launch pulls from
#  (`rhtn_launch::REGISTRY_DEFAULT`, overridable with RHTN_REGISTRY). **A rented
#  instance runs what the registry holds**, so a node launched from a phone
#  carries the daemon that was pushed and not the tree on this machine.
# =============================================================================
set -euo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
WORKSPACE="$(cd "$HERE/.." && pwd)"
FLAVOUR=releasable
TAG=""
PUSH=no
# the registry a launch pulls from, so a hand-built image can stand in for
# CI's (`launch::REGISTRY_DEFAULT`); $RHTN_REGISTRY names another
REGISTRY="${RHTN_REGISTRY:-docker.io/sarcina}"
NAME="${RHTN_IMAGE:-rhtnd}"

while [ $# -gt 0 ]; do
  case "$1" in
    --flavour) FLAVOUR="${2:-}"; shift 2 ;;
    --tag) TAG="${2:-}"; shift 2 ;;
    --push) PUSH=yes; shift ;;
    -h|--help) sed -n '2,12p' "$0" | sed 's/^# \?//'; exit 0 ;;
    *) echo "build.sh: unknown argument $1" >&2; exit 2 ;;
  esac
done

ENGINE="${ENGINE:-}"
if [ -z "$ENGINE" ]; then
  for c in podman docker; do command -v "$c" > /dev/null 2>&1 && ENGINE="$c" && break; done
fi
[ -n "$ENGINE" ] || { echo "build.sh: no podman or docker" >&2; exit 2; }

COMMIT="$(cd "$WORKSPACE/.." && git rev-parse --short HEAD 2> /dev/null || echo unknown)"
DIRTY=""
(cd "$WORKSPACE/.." && git diff --quiet 2> /dev/null) || DIRTY="-dirty"
[ -n "$TAG" ] || TAG="${COMMIT}${DIRTY}-${FLAVOUR}"

echo "build.sh: $ENGINE build -> $NAME:$TAG (flavour $FLAVOUR, workspace $WORKSPACE)" >&2
"$ENGINE" build \
  -f "$HERE/Containerfile" \
  --build-arg "FLAVOUR=$FLAVOUR" \
  -t "$NAME:$TAG" \
  -t "$NAME:latest" \
  "$WORKSPACE"

echo "$NAME:$TAG"

if [ "$PUSH" = yes ]; then
  for t in "$TAG" latest; do
    echo "build.sh: $ENGINE push $REGISTRY/$NAME:$t" >&2
    "$ENGINE" tag "$NAME:$t" "$REGISTRY/$NAME:$t"
    "$ENGINE" push "$REGISTRY/$NAME:$t"
  done
fi
