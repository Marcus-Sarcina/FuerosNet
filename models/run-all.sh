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

# THE FENCE ON THE PROVER.  Tamarin's runtime defaults to every core and no
# heap ceiling.  On 2026-09-08 two non-terminating searches, run side by side
# during a diagnosis, exhausted 94 GB in ten minutes and locked the machine
# before either reached its 900 s timeout -- a runaway search fills memory
# faster than it spends wall clock, so a timeout alone is no protection.
#   -N8   costs nothing measurable: the bounded attach companion took 76.5 s
#         on 8 cores and 76.6 s on 32.
#   -M64g is thirteen times the largest terminating footprint measured
#         (4.9 GB, the same companion) and turns a runaway into a "Heap
#         exhausted" exit within seconds, which the no-verdict branches
#         below report as a failure.
# nice keeps the desktop usable while the wire-only recovery family runs.
TAMARIN_RTS="${TAMARIN_RTS:-+RTS -N8 -M64g -RTS}"

HERE="$(cd "$(dirname "$0")" && pwd)"
RESULTS="$HERE/results"
mkdir -p "$RESULTS"
fail=0

# FRESHNESS EVIDENCE FOR THE RESULTS [2026-09-30].  `results/` holds each
# tool's verdict and nothing said which source it was the verdict ON, so a
# result outliving an edit to its theory read exactly like a current one and
# "the results match the sources" was checkable only by reading them.  Each
# run now records the source it proved and that source's hash; the manifest
# is `results/sources.json`, and `Robot/modelpincheck.py` says whether any
# result is stale.
MANIFEST="$RESULTS/.sources"
: > "$MANIFEST"
stamp() {  # stamp <result file> <source file...>
  local out="$1"; shift
  local src
  for src in "$@"; do
    [ -f "$src" ] || continue
    printf '%s\t%s\t%s\n' "$(basename "$out")" \
      "$(python3 -c 'import os,sys; print(os.path.relpath(sys.argv[1], sys.argv[2]))' "$src" "$HERE")" \
      "$(sha256sum "$src" | cut -d" " -f1)" >> "$MANIFEST"
  done
}

echo "=== 1. Trust-metric simulation (Python stdlib) ==="
stamp "$RESULTS/flow_metric.txt" "$HERE/simulation/flow_metric.py"
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
  stamp "$out" "$HERE/tla/$m.tla" "$HERE/tla/$m.cfg"
  "$JAVA" -XX:+UseParallelGC -cp "$TLA_JAR" tlc2.TLC \
      -workers 4 -deadlock -metadir "/tmp/tlc_$m" \
      "$HERE/tla/$m.tla" > "$out" 2>&1
  if grep -q "Model checking completed. No error has been found." "$out"; then
    echo "  $m: no error"
  else
    echo "  $m: FAILED (see results/$m.txt)"; fail=1
  fi
done

# FURTHER INSTANCES of a module above, each a configuration the default
# instance is too small to exercise.
#   CycleDetection_FourNodes -- a cycle node with a child NOT on the cycle,
#                               so which subordinate repair cuts is a choice.
for pair in "CycleDetection:CycleDetection_FourNodes"; do
  m="${pair%%:*}"; cfg="${pair##*:}"
  out="$RESULTS/$cfg.txt"
  stamp "$out" "$HERE/tla/$m.tla" "$HERE/tla/$cfg.cfg"
  "$JAVA" -XX:+UseParallelGC -cp "$TLA_JAR" tlc2.TLC \
      -workers 4 -deadlock -metadir "/tmp/tlc_$cfg" \
      -config "$HERE/tla/$cfg.cfg" "$HERE/tla/$m.tla" > "$out" 2>&1
  if grep -q "Model checking completed. No error has been found." "$out"; then
    echo "  $cfg: no error"
  else
    echo "  $cfg: FAILED (see results/$cfg.txt)"; fail=1
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
# THE MUTATIONS, and each must FAIL.  Every TLA+ result above holds because
# of a rule stated somewhere in the documents -- the series rule, chain-length
# ordering, current rather than permanent authorisation, fresh issuance
# rather than a grace period, cutting the subordinate that handed the memo
# over.  Each mutation switches one such rule off by a constant.
#
# If TLC reports NO error for one, the compliant-world result has stopped
# depending on that rule and has become true for some other reason -- which
# would mean the model no longer says what it claims.  So a clean run is a
# FAILURE of this gate.
echo "=== 2b. Mutations: each must FAIL ==="
# A mutation that stops violating means the invariant it belongs to has quietly
# stopped depending on the rule that holds it up, and a clean run here is
# therefore a FAILURE of this gate.
#   SupersessionDiscipline_Mutation      -- drops the series rule (light
#                                           client and chain holder alike).
#   SupersessionDiscipline_OrderMutation -- drops wire 4.6.1's chain-length
#                                           ordering, so a shorter chain
#                                           replaces a longer one.
#   IssuerAuthorisation_Mutation         -- lets acceptance rest on any role
#                                           ever recorded, which is what a
#                                           permanently-persistent
#                                           authorisation record amounts to.
#   CurrencyEscalation_Mutation          -- allows the grace period design
#                                           12.6.5.1 rejects: the attestation
#                                           is extended while the patron is
#                                           down, with nobody signing.
#   CycleDetection_Mutation              -- lets repair cut any direct
#                                           subordinate rather than the one
#                                           that handed the memo over.
# Each entry is module:configuration:what TLC must report -- an invariant
# name, an action property name, or "Temporal" for a temporal property.
for mut in "SupersessionDiscipline:SupersessionDiscipline_Mutation:NeverIssuedForASupersededKey" \
           "SupersessionDiscipline:SupersessionDiscipline_OrderMutation:NeverIssuedForASupersededKey" \
           "IssuerAuthorisation:IssuerAuthorisation_Mutation:NeverAcceptedOnALapsedAuthorisation" \
           "CurrencyEscalation:CurrencyEscalation_Mutation:FreshOnly" \
           "CycleDetection:CycleDetection_Mutation:Temporal"; do
  IFS=: read -r mm cfg inv <<< "$mut"
  mout="$RESULTS/$cfg.txt"
  stamp "$mout" "$HERE/tla/$mm.tla" "$HERE/tla/$cfg.cfg"
  "$JAVA" -XX:+UseParallelGC -cp "$TLA_JAR" tlc2.TLC \
      -workers 4 -deadlock -metadir "/tmp/tlc_$cfg" \
      -config "$HERE/tla/$cfg.cfg" \
      "$HERE/tla/$mm.tla" > "$mout" 2>&1
  if [ "$inv" = "Temporal" ]; then
    pat="Temporal properties were violated"
  else
    pat="Invariant $inv is violated\|Action property $inv is violated"
  fi
  if grep -q "$pat" "$mout"; then
    echo "  $cfg: violated as expected"
  else
    echo "  $cfg: DID NOT VIOLATE (see results/$cfg.txt)"
    fail=1
  fi
done

echo "=== 3. Tamarin symbolic protocol models ==="
#   local/     -- a two-party exchange across two channels of different
#                  strength (`wire-format.md` §14.3), where the question is
#                  which guarantees come from the bytes and which from the
#                  physical premise.  Neither tree above fits: there is no
#                  third party checking bytes, and no single-party obligation.
for spec in wire-only/attach wire-only/currency wire-only/recovery wire-only/ceremony \
            wire-only/query \
            compliant/currency compliant/attach compliant/ceremony compliant/recovery \
            local/exchange; do
  t="${spec%%/*}-${spec##*/}"
  [ -f "$HERE/tamarin/$spec.spthy" ] || continue
  out="$RESULTS/$t.txt"
  stamp "$out" "$HERE/tamarin/$spec.spthy"
  # A TIMEOUT, because a theory whose search does not converge would
  # otherwise hang this gate forever -- which happened while rebuilding the
  # currency model on 2026-09-05.  A proof needing longer than this needs a
  # hint, not a longer wall clock.
  PATH="$MAUDE_DIR:$PATH" timeout "${TAMARIN_TIMEOUT:-600}" \
      nice -n 19 "$TAMARIN" $TAMARIN_RTS --derivcheck-timeout=60 --prove "$HERE/tamarin/$spec.spthy" > "$out" 2>&1
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
  # the theory AND the fragment: a companion is the verdict on both, and
  # an edit to either stales it (this site is the one modelpincheck.py
  # caught unstamped on its first run, 2026-09-30)
  stamp "$out" "$HERE/tamarin/$spec.spthy" "$frag"
  # theory minus its LAST `end` line, then the fragment, then `end`
  awk '{lines[NR]=$0} END{for(i=NR;i>=1;i--) if(lines[i]=="end"){last=i;break}
       for(i=1;i<=NR;i++) if(i!=last) print lines[i]}' "$HERE/tamarin/$spec.spthy" > "$tmp"
  cat "$frag" >> "$tmp"; echo "" >> "$tmp"; echo "end" >> "$tmp"
  PATH="$MAUDE_DIR:$PATH" timeout "${TAMARIN_TIMEOUT:-600}" \
      nice -n 19 "$TAMARIN" $TAMARIN_RTS --derivcheck-timeout=60 --prove "$tmp" > "$out" 2>&1
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

# ---------------------------------------------------------------------------
# 3c. THEORY MUTATIONS, and each must FALSIFY.  `wire-format.md` writes
# several bindings as a comparison the validator MUST make -- "an unchecked
# binding is the same as no binding" -- and until 2026-09-08 this suite got
# those bindings by naming one variable in both places, so no mutation could
# remove them and a green lemma meant only that the model could not write the
# error down.  Each entry below deletes ONE comparison by replacing it with a
# tautology of the same shape, leaving every signature valid, and the named
# lemma must then falsify.  A clean run is a FAILURE of this gate.
#
# Replacing rather than deleting the line is deliberate: an earlier mutation
# script left a dangling comma and Tamarin rejected the theory, which reads
# from the outside exactly like a mutation that worked.
#   compliant/attach, 0-RTT   -- the conforming server binds early data off
#                                ANY completed handshake with that client,
#                                not the one on the connection that carried
#                                it: the association wire 9.1 exists for.
#   compliant/attach, queue   -- delivery ignores which credential an item was
#                                queued for, so an item queued under a
#                                superseded key goes out under its successor.
#   compliant/attach, memory  -- supersession ends the sessions and KEEPS the
#                                binding record: the node that terminates and
#                                forgets, then serves a reconnect.  Proved
#                                against the bounded companion (fifth field),
#                                since the lemma regresses unbounded.
#   compliant/ceremony, roster -- the signer checks the first attributed
#                                witness twice and the second never: validate
#                                one element, authorise the collection.
#   wire-only/attach, delegated -- three, added 2026-09-22 with the delegated
#                                bind (wire 8.2, 9.1).  Two delete the
#                                comparison "field 1 against the key the
#                                handshake presented", client side and server
#                                side: a captured delegation then completes a
#                                handshake under the adversary's own key.  The
#                                third drops the check that a HELD delegation
#                                verifies under the pinned material: the
#                                adversary then delegates to itself.
# A fifth field "bounded" splices the theory's .bounded fragment into the
# mutant before proving, for a lemma that only discharges under the bound.
# The substitution's from/to fields may carry \n for a line break.
MUTATIONS=(
  'wire-only/recovery|successor_statement_binds_the_patron|Eq(stmtPat, $P)|Eq($P, $P)'
  'wire-only/recovery|recognition_binds_the_successor|Eq(respNew1, adoptNew)|Eq(adoptNew, adoptNew)'
  'wire-only/recovery|transfer_statement_binds_the_destination|Eq(stmtNew, $New)|Eq($New, $New)'
  'compliant/attach|a_conforming_server_binds_only_after_the_handshake|Handshaken($N, $C, conn), !EarlyData($N, $C, conn, a)|Handshaken($N, $C, connT), !EarlyData($N, $C, conn, a)'
  'compliant/attach|delivery_is_to_the_credential_it_was_queued_for|Queued($N, $C, k, m) ]|Queued($N, $C, kq, m) ]'
  'compliant/attach|no_attach_after_supersession|--[ EvidenceHeld($N, $C, k), Terminated($N, $C, k) ]->\n    [ ]|--[ EvidenceHeld($N, $C, k), Terminated($N, $C, k) ]->\n    [ Binding($N, $C, k) ]|bounded'
  'compliant/ceremony|no_signature_over_a_misattributed_nominee|, !Nomination($P, $Other, $W2) ]|, !Nomination($P, $Other, $W1) ]'
  'wire-only/attach|a_delegated_bind_names_a_key_the_identity_delegated|, Eq(named, presented)             // (c)|, Eq(presented, presented)         // (c)'
  'wire-only/attach|a_delegated_bind_names_a_key_the_identity_delegated|    , Eq(namedC, presentedC)\n    , Eq(verify(csig|    , Eq(presentedC, presentedC)\n    , Eq(verify(csig'
  'wire-only/attach|a_delegated_bind_names_a_key_the_identity_delegated|Eq(verify(del, <'"'"'delegation'"'"', k, $S>, fst(km)), true)|Eq(true, true)'
  # local/exchange -- the receiver stops comparing the bearer's anchor field
  # with the ceremony-id it derived optically, so the adversary supplies one
  # nobody derived.  The check is the whole of what §14.3.2 buys, the anchor
  # being public; with it gone the exchange keeps no binding at all.
  'local/exchange|an_accepted_anchor_was_derived_by_both|Eq(anchor, cid)                 // the anchor|Eq(cid, cid)                     // the anchor'
  # wire-only/query -- drop the field-7 comparison, which is the whole of
  # what makes a consent an authorization rather than bearer paper (s5.6):
  # one consented query then reaches every verifier the selector cares to
  # try, which is the attack that sentence names.
  'wire-only/query|an_answer_had_the_subjects_consent_for_this_verifier|Eq(addressed, $V)                  // (a) addressed to this verifier|Eq($V, $V)                          // (a) addressed to this verifier'
)

echo "=== 3c. Theory mutations: each must FALSIFY ==="
# Several mutations may target ONE lemma -- the delegated bind has three --
# so the index is part of the name [2026-09-30]: without it each overwrote
# the last, and two of the three left nothing to inspect.
mi=0
for m in "${MUTATIONS[@]}"; do
  IFS='|' read -r spec lem from to bounded <<< "$m"
  mi=$((mi + 1))
  t="${spec%%/*}-${spec##*/}"
  src="$HERE/tamarin/$spec.spthy"
  frag="$HERE/tamarin/$spec.bounded"
  mfile="$RESULTS/mutant-${mi}-${t}-${lem}.spthy"
  mout="$RESULTS/mutant-${mi}-${t}-${lem}.txt"
  stamp "$mout" "$src"
  # The substitution is literal and MUST land: a mutation that matched nothing
  # would prove the unmutated theory and report a clean pass.
  if ! python3 -c '
import sys
src, dst, frm, to, bounded, frag = sys.argv[1:7]
frm = frm.replace("\\n", "\n"); to = to.replace("\\n", "\n")
s = open(src).read()
if frm not in s:
    sys.exit("pattern absent: " + frm)
s = s.replace(frm, to)
if bounded == "bounded":
    lines = s.split("\n"); last = max(i for i, l in enumerate(lines) if l == "end")
    s = "\n".join(lines[:last]) + "\n" + open(frag).read() + "\nend\n"
open(dst, "w").write(s)
' "$src" "$mfile" "$from" "$to" "${bounded:-}" "$frag"; then
    echo "  ${lem}: MUTATION DID NOT APPLY (pattern absent)"; fail=1; continue
  fi
  PATH="$MAUDE_DIR:$PATH" timeout "${TAMARIN_TIMEOUT:-600}" \
      nice -n 19 "$TAMARIN" $TAMARIN_RTS --derivcheck-timeout=60 --prove="$lem" "$mfile" > "$mout" 2>&1
  if [ $? -eq 124 ]; then
    echo "  ${lem}: TIMED OUT (see results/$(basename "$mout"))"; fail=1; continue
  fi
  if grep -qE "${lem}.*falsified" "$mout"; then
    echo "  ${lem}: falsified as expected"
  else
    echo "  ${lem}: DID NOT FALSIFY (see results/$(basename "$mout"))"; fail=1
  fi
done

# The manifest, from what each run stamped: result file, source proved, and
# that source's hash at the moment it was proved.
python3 - "$MANIFEST" "$RESULTS/sources.json" <<'PY'
import json, sys
rows = {}
for line in open(sys.argv[1]):
    out, src, h = line.rstrip('\n').split('\t')
    rows.setdefault(out, {})[src] = h
json.dump({'results': rows}, open(sys.argv[2], 'w'), indent=2, sort_keys=True)
open(sys.argv[2], 'a').write('\n')
PY
rm -f "$MANIFEST"
echo "  results/sources.json: $(python3 -c 'import json,sys; print(len(json.load(open(sys.argv[1]))["results"]))' "$RESULTS/sources.json") results stamped with the sources they prove"
# And the freshness check over what was just stamped, so a run that left a
# stale or unstamped result -- a source edited while the prover ran, a
# result file nothing stamped -- fails this gate rather than waiting for
# someone to run the checker [2026-10-01].
if python3 "$HERE/../Robot/modelpincheck.py"; then :; else fail=1; fi

echo
if [ "$fail" -eq 0 ]; then
  echo "ALL MODELS PASS"
else
  echo "SOME MODELS FAILED -- inspect results/"
fi
exit "$fail"
