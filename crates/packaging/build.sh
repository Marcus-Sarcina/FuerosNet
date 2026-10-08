#!/usr/bin/env bash
# =============================================================================
#  build.sh -- build the node's image.
#
#    packaging/build.sh [--flavour releasable|fieldtest] [--tag <tag>]
#
#  Tagged with the commit by default, so an image on a host can be traced back
#  to the tree that produced it; `latest` moves with it.
# =============================================================================
set -euo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
WORKSPACE="$(cd "$HERE/.." && pwd)"
FLAVOUR=releasable
TAG=""
NAME="${RHTN_IMAGE:-rhtnd}"

while [ $# -gt 0 ]; do
  case "$1" in
    --flavour) FLAVOUR="${2:-}"; shift 2 ;;
    --tag) TAG="${2:-}"; shift 2 ;;
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
