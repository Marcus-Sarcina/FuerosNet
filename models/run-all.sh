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
# All run with -deadlock: the models terminate (quiescence), which the
# default deadlock check would flag; the properties that matter are the
# invariants and temporal properties, checked regardless.
for m in PartitionMerge CurrencyEscalation CycleDetection SupersessionDiscipline IssuerAuthorisation; do
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
# THE MUTATION, and it must FAIL.  SupersessionDiscipline's invariant holds
# because the SUBJECT is separately forbidden to re-occupy an abandoned series
# (light-client-requirements.md) -- a rule in another document binding another
# party.  Setting SeriesCheck = FALSE removes that assumption, and the record
# can then walk back onto a generation the node had already superseded.
#
# If TLC reports NO error here, the compliant-world result has stopped
# depending on the cross-document rule and has become true for some other
# reason -- which would mean the model no longer says what it claims.  So a
# clean run is a FAILURE of this gate.
echo "=== 2b. Mutations: each must FAIL ==="
# A mutation that stops violating means the invariant it belongs to has quietly
# stopped depending on the rule that holds it up, and a clean run here is
# therefore a FAILURE of this gate.
#   SupersessionDiscipline -- drops the light client's series rule.
#   IssuerAuthorisation    -- lets acceptance rest on any role ever recorded,
#                             which is what a permanently-persistent
#                             authorisation record amounts to.
for mut in "SupersessionDiscipline:NeverIssuedForASupersededKey" \
           "IssuerAuthorisation:NeverAcceptedOnALapsedAuthorisation"; do
  mm="${mut%%:*}"; inv="${mut##*:}"
  mout="$RESULTS/${mm}_Mutation.txt"
  "$JAVA" -XX:+UseParallelGC -cp "$TLA_JAR" tlc2.TLC \
      -workers 4 -deadlock -metadir "/tmp/tlc_${mm}_mut" \
      -config "$HERE/tla/${mm}_Mutation.cfg" \
      "$HERE/tla/${mm}.tla" > "$mout" 2>&1
  if grep -q "Invariant $inv is violated" "$mout"; then
    echo "  ${mm}_Mutation: violated as expected"
  else
    echo "  ${mm}_Mutation: DID NOT VIOLATE (see results/${mm}_Mutation.txt)"
    fail=1
  fi
done

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

# ---------------------------------------------------------------------------
# 3b. BOUNDED COMPANIONS.  Three obligations -- currency's
# `no_issuance_for_a_key_this_issuer_superseded` and attach's two supersession
# lemmas -- do not discharge over unbounded traces: the issuing and serving
# rules consume a linear capability and restore it, so the search for that
# fact's origin regresses through unboundedly many prior operations.
#
# The bound is applied by APPENDING a fragment to the real theory here, rather
# than by keeping a second copy of the rules.  The rules therefore have exactly
# one source, and a change to them is picked up by the bounded check on the
# next run.  A hand-maintained copy would have drifted.
#
# A pass here means: no counterexample in any trace within the bound.  It is
# not the unbounded claim and must not be quoted as one.
echo "=== 3b. Bounded companions (see the .bounded fragments) ==="
for spec in compliant/currency compliant/attach; do
  frag="$HERE/tamarin/$spec.bounded"
  [ -f "$frag" ] || continue
  t="bounded-${spec##*/}"
  out="$RESULTS/$t.txt"
  tmp="$RESULTS/$t.spthy"
  # theory minus its LAST `end` line, then the fragment, then `end`
  awk '{lines[NR]=$0} END{for(i=NR;i>=1;i--) if(lines[i]=="end"){last=i;break}
       for(i=1;i<=NR;i++) if(i!=last) print lines[i]}' "$HERE/tamarin/$spec.spthy" > "$tmp"
  cat "$frag" >> "$tmp"; echo "" >> "$tmp"; echo "end" >> "$tmp"
  PATH="$MAUDE_DIR:$PATH" timeout "${TAMARIN_TIMEOUT:-600}" \
      "$TAMARIN" --prove "$tmp" > "$out" 2>&1
  if [ $? -eq 124 ]; then
    echo "  $t: TIMED OUT after ${TAMARIN_TIMEOUT:-600}s (see results/$t.txt)"; fail=1; continue
  fi
  if grep -qE 'wellformedness check(s)? failed' "$out"; then
    echo "  $t: WELLFORMEDNESS check failed (see results/$t.txt)"; fail=1
  elif grep -qE 'falsified|analysis incomplete' "$out"; then
    echo "  $t: FALSIFIED or INCOMPLETE (see results/$t.txt)"; fail=1
  elif grep -qE 'verified' "$out"; then
    n=$(grep -cE 'verified \(' "$out")
    echo "  $t: $n lemmas verified (bounded)"
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
