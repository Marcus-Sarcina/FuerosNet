#!/usr/bin/env bash
# =============================================================================
#  run-all.sh -- build and check every RHTN formal model, capture results.
#
#  Runs the three families in order and writes each tool's verdict to
#  results/.  Exits non-zero if any check fails, so this doubles as a
#  regression gate.
#
#  Tool locations: the models were developed against user-local installs
#  under ~/tools (no root needed).  Override with the env vars below if your
#  layout differs.
# =============================================================================
set -u

JAVA="${JAVA:-$HOME/tools/jdk-21.0.12.1+1-jre/bin/java}"
TLA_JAR="${TLA_JAR:-$HOME/tools/tla2tools.jar}"
TAMARIN="${TAMARIN:-$HOME/tools/tamarin-prover}"
MAUDE_DIR="${MAUDE_DIR:-$HOME/tools}"     # Maude must be on PATH for Tamarin

HERE="$(cd "$(dirname "$0")" && pwd)"
RESULTS="$HERE/results"
mkdir -p "$RESULTS"
fail=0

echo "=== 1. Trust-metric simulation (Python stdlib) ==="
if python3 "$HERE/simulation/flow_metric.py" > "$RESULTS/flow_metric.txt" 2>&1; then
  echo "  flow_metric.py: all assertions passed"
else
  echo "  flow_metric.py: FAILED (see results/flow_metric.txt)"; fail=1
fi

echo "=== 2. TLA+ distributed-systems models (TLC) ==="
# All three run with -deadlock: the models terminate (quiescence), which the
# default deadlock check would flag; the properties that matter are the
# invariants and temporal properties, checked regardless.
for m in PartitionMerge CurrencyEscalation CycleDetection; do
  out="$RESULTS/$m.txt"
  "$JAVA" -XX:+UseParallelGC -cp "$TLA_JAR" tlc2.TLC \
      -workers 4 -deadlock -metadir "/tmp/tlc_$m" \
      "$HERE/tla/$m.tla" > "$out" 2>&1
  if grep -q "Model checking completed. No error has been found." "$out"; then
    echo "  $m: no error"
  else
    echo "  $m: FAILED (see results/$m.txt)"; fail=1
  fi
done

# TWO TREES, because they prove different things and both are wanted:
#   wire-only/  -- properties a third party checks from bytes on the wire.
#   compliant/  -- that an honest client's OWN STATED OBLIGATIONS, in
#                  `light-client-requirements.md` and
#                  `infra-client-requirements.md`, are mutually coherent.
# A compliant/ lemma is NOT a claim that anyone can verify compliance
# remotely -- it is a claim that a node doing what it promised cannot reach
# a state its own commitments forbid.  See models/README.md.
echo "=== 3. Tamarin symbolic protocol models ==="
for spec in wire-only/attach wire-only/currency wire-only/recovery wire-only/ceremony \
            compliant/currency compliant/attach compliant/ceremony compliant/recovery; do
  t="${spec%%/*}-${spec##*/}"
  [ -f "$HERE/tamarin/$spec.spthy" ] || continue
  out="$RESULTS/$t.txt"
  # A TIMEOUT, because a theory whose search does not converge would
  # otherwise hang this gate forever -- which happened while rebuilding the
  # currency model on 2026-09-05.  A proof needing longer than this needs a
  # hint, not a longer wall clock.
  PATH="$MAUDE_DIR:$PATH" timeout "${TAMARIN_TIMEOUT:-600}" \
      "$TAMARIN" --prove "$HERE/tamarin/$spec.spthy" > "$out" 2>&1
  if [ $? -eq 124 ]; then
    echo "  $t: TIMED OUT after ${TAMARIN_TIMEOUT:-600}s (see results/$t.txt)"
    fail=1; continue
  fi
  # Every lemma line must read "verified"; any "falsified" is a failure.
  # A WELLFORMEDNESS failure is also a failure, and this is why: Tamarin
  # reports one as a WARNING, exits 0, and still prints "verified" for the
  # malformed lemma.  A free timepoint variable introduced by a careless
  # edit passed this gate once already (2026-09-05); grepping only for
  # verified/falsified cannot see it.
  if grep -qE 'wellformedness check(s)? failed' "$out"; then
    echo "  $t: WELLFORMEDNESS check failed (see results/$t.txt)"; fail=1
  elif grep -qE 'falsified' "$out"; then
    echo "  $t: FALSIFIED lemma present (see results/$t.txt)"; fail=1
  elif grep -qE 'verified' "$out"; then
    n=$(grep -cE 'verified \(' "$out")
    echo "  $t: $n lemmas verified"
  else
    echo "  $t: no verdict (see results/$t.txt)"; fail=1
  fi
done

echo
if [ "$fail" -eq 0 ]; then
  echo "ALL MODELS PASS"
else
  echo "SOME MODELS FAILED -- inspect results/"
fi
exit "$fail"
