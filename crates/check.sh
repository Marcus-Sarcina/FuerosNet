#!/usr/bin/env bash
# =============================================================================
#  check.sh -- the code gate for the RHTN workspace.
#
#  0. The specification pins: the code's and the vectors', non-mutating.
#  1. The acceptance catalogue: fields, ids, citations, verbatim quotes,
#     gap coverage (tools/check.py; exit 1 on any flag).
#  2. The generated stubs are in sync with the catalogue: regenerate into a
#     temporary directory and diff.  A hand edit to tests/ fails here.
#  3a. Every crate lints clean under clippy, all targets, warnings as errors.
#  3b. Licences and advisories under cargo-deny, against deny.toml.
#  3. The workspace compiles and its live tests pass.  Stubs are #[ignore] and
#     are not run: they are the tests still owed, and `cargo test -- --ignored`
#     lists them by failing each one.
#
#  Every long job is fenced, as models/run-all.sh fences the prover: cargo is
#  niced and capped at 8 jobs.
# =============================================================================
set -u
# A pipeline's status is its last command's, so `cargo ... | tail` would
# report tail's success whatever cargo did; pipefail makes the step's
# verdict cargo's own.
set -o pipefail
HERE="$(cd "$(dirname "$0")" && pwd)"
fail=0

echo "=== 0. Specification pins ==="
# Non-mutating: the documents the code last passed against, and the
# documents, tools and outputs the vectors were generated from, against the
# tree now.  A specification-only change fails here, which no other step
# would see (2026-09-21).  Re-pin the code with tools/pincheck.py --accept
# once the gate passes against the changed documents; the vector pin is
# generate.py's alone.
if python3 "$HERE/tools/pincheck.py"; then :; else fail=1; fi

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

echo "=== 3a. Lint ==="
# Every crate, every target, and a warning is a failure: clippy runs on the
# stable toolchain the workspace builds with.
if (cd "$HERE" && nice -n 19 cargo clippy -j 8 --workspace --all-targets --quiet -- -D warnings 2>&1 | tail -20); then
  echo "  cargo clippy: clean"
else
  echo "  cargo clippy: WARNINGS"; fail=1
fi

echo "=== 3b. Licences and advisories ==="
# cargo-deny against deny.toml: the allow-list is what the dependency tree
# carries, all permissive; copyleft fails by absence from it, and the
# advisory database is checked.  Absence of the tool is a failure, not a
# skip: a licence gate that silently did not run is no gate.
if cargo deny --version > /dev/null 2>&1; then
  if (cd "$HERE" && nice -n 19 cargo deny check 2>&1 | tail -12); then
    echo "  cargo deny: clean"
  else
    echo "  cargo deny: FAILED"; fail=1
  fi
else
  echo "  cargo-deny absent: cargo install cargo-deny --locked"; fail=1
fi

echo "=== 3. Build and live tests ==="
if (cd "$HERE" && nice -n 19 cargo test -j 8 --workspace --quiet 2>&1 | tail -20); then
  echo "  cargo test: ok"
else
  echo "  cargo test: FAILED"; fail=1
fi

echo "=== 4. Coverage-guided smoke (cargo-fuzz on nightly; skipped where absent) ==="
# Each decoder layer for a bounded time under libFuzzer with a memory cap:
# the memory budget the decoder entries parameterise, observed rather than
# assumed.  The long run is manual: cargo +nightly fuzz run <target> in
# codec/.  Absence of the toolchain is reported, not failed, so the gate
# still runs on a machine without it.
if cargo +nightly --version > /dev/null 2>&1 && cargo fuzz --version > /dev/null 2>&1; then
  python3 "$HERE/codec/fuzz/seed.py" > /dev/null
  for t in parse_all body envelope frame_control frame_request; do
    out="$HERE/codec/fuzz/smoke-$t.log"
    if (cd "$HERE/codec" && nice -n 19 cargo +nightly fuzz run "$t" -- \
          -max_total_time="${FUZZ_SMOKE_SECONDS:-20}" -rss_limit_mb=512 -timeout=1) > "$out" 2>&1; then
      echo "  $t: $(grep -oE 'Done [0-9]+ runs' "$out" | tail -1 | tr -d '\n'), no crash"
    else
      echo "  $t: CRASH or error (see codec/fuzz/smoke-$t.log)"; fail=1
    fi
  done
else
  echo "  nightly toolchain or cargo-fuzz absent: smoke skipped (seeded runs above still ran)"
fi

echo
if [ "$fail" -eq 0 ]; then echo "CODE GATE PASSES"; else echo "CODE GATE FAILED"; fi
exit "$fail"
