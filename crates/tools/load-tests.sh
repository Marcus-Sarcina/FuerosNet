#!/usr/bin/env bash
# =============================================================================
#  load-tests.sh -- the production-size ceiling tests the gate skips.
#
#  TOP-46 fills what a holder keeps for a prerequisite that has not arrived
#  and checks the order at the ceiling.  The gate (check.sh) runs it at a
#  reduced ceiling; the runs at the production ceilings (768 in
#  archive/src/topology.rs, 256 in node/src/store.rs) sign and verify about
#  two thousand records and take about two minutes each in debug, so they
#  are #[ignore]d in the suite and run here, and only here.  Run this
#  whenever topology.rs or store.rs changes and before any release.
#
#  The two crates hold no other ignored tests, so `--ignored` here is
#  exactly the load tests.  Exits with cargo's status: a failing load test
#  fails the script.
# =============================================================================
set -u
set -o pipefail
HERE="$(cd "$(dirname "$0")/.." && pwd)"
cd "$HERE" || exit 1

echo "=== Load tests: the production-size ceilings the gate skips ==="
echo "  in $HERE"
echo "  cargo test -p rhtn-archive -p rhtn-node -- --ignored"
nice -n 19 cargo test -j 8 -p rhtn-archive -p rhtn-node -- --ignored
rc=$?
echo
if [ "$rc" -eq 0 ]; then echo "LOAD TESTS PASS"; else echo "LOAD TESTS FAILED"; fi
exit "$rc"
