#!/usr/bin/env bash
# =============================================================================
#  check.sh -- the code gate for the RHTN workspace.
#
#  0. The specification pins: the code's and the vectors', non-mutating.
#  1. The acceptance catalogue: fields, ids, citations, verbatim quotes,
#     gap coverage (tools/check.py; exit 1 on any flag).
#  1b. Counts and citations stated in the documents, and model results
#      against their sources.
#  1c. The field-test checklist generator's test (tools/field-checklist.py).
#  1d. References, staleness and the condition matrix: non-halting, since
#      each reports a drift in the prose and not a defect in the code.
#  2. The generated stubs are in sync with the catalogue: regenerate into a
#     temporary directory and diff.  A hand edit to tests/ fails here.
#  3a. Every crate is rustfmt-clean at the default width, non-mutating; the
#      reviewer harness and the generated stubs are outside its reach.
#  3b. Every crate lints clean under clippy, all targets, warnings as errors
#      -- except `missing_docs`, which reports and does not halt, so one
#      missing comment cannot leave nine crates unlinted.
#  3b2. The same lint over the other feature flavours, which the default
#       build does not compile.  Linted, not built and not tested.
#  3c. Licences and advisories under cargo-deny, against deny.toml.
#  3. The workspace compiles and its live tests pass.  Stubs are #[ignore] and
#     are not run: they are the tests still owed, and `cargo test -- --ignored`
#     lists them by failing each one.  The production-size load tests are
#     #[ignore] too and are not stubs: tools/load-tests.sh runs them.
#  4. A bounded fuzz run on the decoders, under nightly where present.
#  5. The build cache swept of what this pass did not build (cargo-sweep,
#     stamped before step 0): cargo names artifacts by hash and deletes
#     nothing itself, so a pass otherwise leaves every superseded test
#     binary behind.
#
#  A NON-HALTING FINDING is reported on the final line and never blocks:
#  documentation a public item owes, and the three checkers of stage 1d.
#  The exit status is the blocking failures alone.  These are not stage
#  3c's advisories, which are cargo-deny's security advisories and do
#  block; the word there is cargo's own.
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
# A non-halting finding is one the gate reports and does not block on
# [author, 2026-10-05]: documentation a public item still owes, and the
# three document checkers of stage 1d.  A gate run is expensive, so a
# finding whose fix is additive and whose absence breaks nothing is better
# collected whole than met one at a time across as many runs.  The count
# rides the final line; it never changes the exit status.
#
# **Not stage 3c's advisories**, which are cargo-deny's security
# advisories and do block.  Two different things, and the word is cargo's
# there, so this one does not use it.
nonhalting=0

# The build cache is stamped before anything builds and swept after
# everything has: whatever the gate did not touch is a superseded artifact
# (cargo names each by a hash of its inputs and deletes nothing itself), and
# a full pass leaves one copy of everything.  cargo-sweep absent is a
# failure, not a skip.
if cargo sweep --version > /dev/null 2>&1; then
  (cd "$HERE" && cargo sweep --stamp > /dev/null 2>&1)
else
  echo "  cargo-sweep absent: cargo install cargo-sweep --locked"; fail=1
fi

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

echo "=== 1b. Counts and citations stated in the documents ==="
# A number in prose is a claim, and both of these were false while nothing
# checked them: functional_tests §10's prefix table drifted two behind its
# own rows, and four negative-vector citations pointed at rows that had been
# renumbered under them -- two of those inside pinned reason strings, so the
# corpus re-pinned the error on every run (2026-09-30).  citecheck's
# existence check only; its `--overlap` sweep is for a human to read.
if python3 "$HERE/../Robot/countcheck.py"; then :; else fail=1; fi
if python3 "$HERE/../test-vectors/tools/citecheck.py"; then :; else fail=1; fi
# A model result whose source changed after it was proved is a claim the
# tree no longer makes: stale results fail the gate here, where every
# commit meets it, as well as at the end of models/run-all.sh (2026-10-01).
if python3 "$HERE/../Robot/modelpincheck.py"; then :; else fail=1; fi

echo "=== 1c. The field-test checklist generator ==="
# tools/field-checklist.py writes the tester's form into every run directory
# from the catalogue's manual rows; its test runs it against the catalogue in
# the tree and against a synthetic one (2026-10-03).  Standard library only.
if python3 "$HERE/tools/test_field_checklist.py" 2>&1 | tail -2 | sed 's/^/  /'; then :; else fail=1; fi

echo "=== 1d. References, staleness and the condition matrix (non-halting) ==="
# `CLAUDE.md` states the reference rule first -- every §N in every
# document, in every direction, against actual headings, with no
# exemptions -- and until 2026-10-05 no stage ran it, so the one rule the
# working context puts first was enforced by nothing on a commit.  Its
# failure mode is a reference that resolves silently to the wrong
# document, which no amount of reading catches.
#
# None of the three halts [author, 2026-10-05]: each reports a drift in the
# prose rather than a defect in the code, and the fix is a text edit that
# blocks nothing.  refcheck and matrixcheck exit 1 on a flag; stalecheck
# only ever reports, so its summary is the whole of its result.
if python3 "$HERE/../Robot/refcheck.py"; then :; else nonhalting=$((nonhalting + 1)); fi
if (cd "$HERE" && python3 "$HERE/../Robot/matrixcheck.py"); then :; else nonhalting=$((nonhalting + 1)); fi
python3 "$HERE/../Robot/stalecheck.py" | tail -1 | sed 's/^/  /'

echo "=== 2. Generated stubs in sync ==="
tmp="$(mktemp -d)"
python3 "$HERE/acceptance/tools/gen_stubs.py" "$tmp" > /dev/null
# The generator owns the .rs files there; tests/rustfmt.toml is the tree's.
if diff -r -x rustfmt.toml "$tmp" "$HERE/acceptance/tests" > /dev/null; then
  echo "  tests/ matches the catalogue"
else
  echo "  tests/ DIFFERS from the catalogue: regenerate with tools/gen_stubs.py"; fail=1
fi
rm -rf "$tmp"

echo "=== 3a. Format ==="
# rustfmt at its default width [author, 2026-09-22], checked and never applied
# here.  conformance/rustfmt.toml turns formatting off for the reviewer
# harness (repaired only where it stops compiling), and acceptance/tests/rustfmt.toml
# for the generated stubs; the inner skip attribute is unstable on stable rustc.
if (cd "$HERE" && cargo fmt --all --check > /dev/null 2>&1); then
  echo "  cargo fmt: clean"
else
  echo "  cargo fmt: FAILED ($(cd "$HERE" && cargo fmt --all --check 2>/dev/null | grep -c '^Diff in') hunks; run cargo fmt --all)"; fail=1
fi

echo "=== 3b. Lint ==="
# Every crate, every target, and a warning is a failure -- with one lint
# exempted.  `missing_docs` is `warn` in every crate root, and `-D
# warnings` would make a missing comment an error: the crate then does not
# compile, and every crate above it in the graph goes unlinted in the same
# run, so one missing comment can hide a real lint in nine crates and
# finding the next one costs another gate run.  `--force-warn` exempts
# that one lint from the deny and nothing else from it, so the whole list
# arrives at once and the gate still blocks on everything that matters
# [author, 2026-10-05].
#
# The JSON is what separates the two verdicts; `Robot/tools/lintreport.py`
# renders a real diagnostic exactly as clippy would have and lists the
# owed comments apart from it, dropping the items `#[uniffi::export]`
# generates, which exist in no source file and which `--force-warn` would
# otherwise report.  The verdict stays clippy's own exit status.
lintjson="$(mktemp)"; linterr="$(mktemp)"
if (cd "$HERE" && nice -n 19 cargo clippy -j 8 --workspace --all-targets --quiet \
      --message-format=json -- -D warnings --force-warn missing-docs \
      > "$lintjson" 2> "$linterr"); then
  echo "  cargo clippy: clean"
else
  echo "  cargo clippy: WARNINGS"; fail=1
  # cargo's own failures live on stderr and are not diagnostics, so the
  # JSON would carry nothing to explain a resolver or feature error
  tail -5 "$linterr" | sed 's/^/  /'
fi
# The count comes from the tool and not from its output: the report
# carries diagnostics rendered by clippy, whose context lines reproduce
# arbitrary source, so any pattern grepped for here is a pattern some
# source file can contain.
owedfile="$(mktemp)"
python3 "$HERE/../Robot/tools/lintreport.py" --count-file "$owedfile" < "$lintjson"
nonhalting=$((nonhalting + $(cat "$owedfile")))
rm -f "$lintjson" "$linterr" "$owedfile"

echo "=== 3b2. Lint, the other feature flavours ==="
# The flavours compile code the default build never sees: the two
# JSON-line layers under `fieldtest`, and `ffi/src/harness.rs` under
# `harness`.  Each went undocumented through a whole sweep that reported
# itself clean, because nothing had compiled it.
#
# **Linted here and not built or tested** [author, 2026-10-05]: the
# fieldtest variant is rebuilt constantly while field-test features are
# being added, and a commit that touches none of them does not owe that
# effort.  `--all-features` is not the alternative -- `releasable` and
# `fieldtest` are two flavours of one build and never both
# (`crates/README.md`), so enabling both enables neither honestly.
flavourlint() {
  local label="$1"; shift
  local out err; out="$(mktemp)"; err="$(mktemp)"
  if (cd "$HERE" && nice -n 19 cargo clippy -j 8 --quiet "$@" --lib --bins \
        --message-format=json -- -D warnings --force-warn missing-docs \
        > "$out" 2> "$err"); then
    echo "  $label: clean"
  else
    echo "  $label: WARNINGS"; fail=1
    tail -5 "$err" | sed 's/^/  /'
  fi
  local owedfile
  owedfile="$(mktemp)"
  python3 "$HERE/../Robot/tools/lintreport.py" --count-file "$owedfile" < "$out"
  nonhalting=$((nonhalting + $(cat "$owedfile")))
  rm -f "$out" "$err" "$owedfile"
}
flavourlint "fieldtest" --no-default-features \
  -p rhtn-client -p rhtn-daemon -p rhtn-participant -p rhtn-ffi \
  --features rhtn-client/fieldtest,rhtn-daemon/fieldtest,rhtn-participant/fieldtest,rhtn-ffi/fieldtest
flavourlint "ffi harness, cli" -p rhtn-ffi --features harness,cli

echo "=== 3c. Licences and advisories ==="
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

echo "=== 4b. The Kotlin binding, generated, compiled and driven (skipped where no JDK or kotlinc) ==="
# milestone 13's second half: one generated binding compiling and one round
# trip through it, against a real node, from Kotlin.  Absent toolchains skip
# and say so; a failure fails the gate.
"$HERE/tools/kotlin-roundtrip.sh"; rc=$?
if [ "$rc" -eq 0 ]; then echo "  kotlin round trip: ok"
elif [ "$rc" -eq 3 ]; then :
else echo "  kotlin round trip: FAILED"; fail=1; fi

echo "=== 4c. The Android shell's unit tests (skipped where no SDK or JBR) ==="
# the shell's own logic on the JVM -- the screen contract -- run through
# Gradle, which is the shell's build and not the workspace's [author,
# 2026-09-27]. The APK and the native library stay outside the gate;
# absent toolchains skip and say so; a failure fails the gate.
"$HERE/tools/android-unit-tests.sh"; rc=$?
if [ "$rc" -eq 0 ]; then echo "  android unit tests: ok"
elif [ "$rc" -eq 3 ]; then :
else echo "  android unit tests: FAILED"; fail=1; fi

echo "=== 5. Build cache ==="
# **The build just ran, so what is stale is stale now** (`tools/sweep.sh`):
# the incremental sessions of builds nobody is troubleshooting, which
# nothing else collects, and whatever cargo-sweep can attribute to a build
# that is no longer current.  Housekeeping, so a failure here is said and
# never the gate's verdict.
#
# **This replaced a stage that had never swept anything** [2026-10-06]: it
# called `cargo sweep --file`, which reads a stamp file that nothing ever
# wrote, so every run failed with "failed to read stamp file" into a
# discarded stderr while the before/after numbers moved for other reasons
# and read like a sweep. 102 GiB of incremental sessions had accumulated
# behind it, which cargo-sweep does not reach at all.
if "$HERE/tools/sweep.sh"; then :; else echo "  build cache: sweep.sh refused"; fi

echo
# A non-halting finding never changes the exit status: it is reported so
# the count is visible on every run and cannot drift unnoticed, and it is
# the author's to act on when they choose.
if [ "$nonhalting" -gt 0 ]; then
  suffix=" ($nonhalting non-halting findings)"
else
  suffix=""
fi
if [ "$fail" -eq 0 ]; then
  echo "CODE GATE PASSES$suffix"
else
  echo "CODE GATE FAILED$suffix"
fi
exit "$fail"
