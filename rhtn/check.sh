#!/usr/bin/env bash
# =============================================================================
#  check.sh -- the code gate for the RHTN workspace.
#
#  1. The acceptance catalogue: fields, ids, citations, verbatim quotes,
#     gap coverage (tools/check.py; exit 1 on any flag).
#  2. The generated stubs are in sync with the catalogue: regenerate into a
#     temporary directory and diff.  A hand edit to tests/ fails here.
#  3. The workspace compiles and its live tests pass.  Stubs are #[ignore] and
#     are not run: they are the tests still owed, and `cargo test -- --ignored`
#     lists them by failing each one.
#
#  Every long job is fenced, as models/run-all.sh fences the prover: cargo is
#  niced and capped at 8 jobs.
# =============================================================================
set -u
HERE="$(cd "$(dirname "$0")" && pwd)"
fail=0

echo "=== 1. Acceptance catalogue ==="
if python3 "$HERE/acceptance/tools/check.py"; then :; else fail=1; fi

echo "=== 2. Generated stubs in sync ==="
tmp="$(mktemp -d)"
python3 "$HERE/acceptance/tools/gen_stubs.py" "$tmp" > /dev/null
if diff -r "$tmp" "$HERE/acceptance/tests" > /dev/null; then
  echo "  tests/ matches the catalogue"
else
  echo "  tests/ DIFFERS from the catalogue: regenerate with tools/gen_stubs.py"; fail=1
fi
rm -rf "$tmp"

echo "=== 3. Build and live tests ==="
if (cd "$HERE" && nice -n 19 cargo test -j 8 --workspace --quiet 2>&1 | tail -20); then
  echo "  cargo test: ok"
else
  echo "  cargo test: FAILED"; fail=1
fi

echo
if [ "$fail" -eq 0 ]; then echo "CODE GATE PASSES"; else echo "CODE GATE FAILED"; fi
exit "$fail"
