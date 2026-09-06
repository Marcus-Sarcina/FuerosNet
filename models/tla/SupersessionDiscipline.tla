--------------------- MODULE SupersessionDiscipline ---------------------
(***************************************************************************)
(* The supersession discipline: what an honest node stops doing once its   *)
(* record of a subject has moved.                                          *)
(*                                                                         *)
(* WHY THIS IS IN TLA+ AND NOT IN TAMARIN.  Three obligations resisted     *)
(* proof in `tamarin/compliant/` -- currency's "issue only for the key you *)
(* currently record" and attach's two supersession rules.  The cause was   *)
(* isolated to a three-rule theory: a rule that CONSUMES A LINEAR FACT AND *)
(* RESTORES IT (issuing does not change what a node records; serving does  *)
(* not end a binding) makes Tamarin's backward search for that fact's      *)
(* origin regress through unboundedly many prior operations.  Deleting the *)
(* restore made the same lemma verify in four steps; adding it back, with  *)
(* or without induction, did not terminate.                                *)
(*                                                                         *)
(* That is a statement about the tool, not about the claim.  These are     *)
(* SAFETY PROPERTIES OF A MUTABLE LOCAL STATE MACHINE, which is what TLC   *)
(* enumerates for a living.  So they are checked here, exhaustively over a *)
(* finite instance, rather than left bounded in a prover that cannot see   *)
(* them.                                                                   *)
(*                                                                         *)
(* WHAT MAKES THE INVARIANTS NON-VACUOUS.  `Issue` does NOT guard on the   *)
(* key being current -- it issues whatever the record holds, and a flag    *)
(* records whether that key had already been superseded.  The invariant is *)
(* therefore a claim about the state machine's SHAPE, not a restatement of *)
(* an action's precondition.  design 12.6.5 calls this "enforced by        *)
(* replacement rather than by a check", and that is exactly the thing      *)
(* being tested: that replacement suffices, with no check anywhere.        *)
(*                                                                         *)
(* This model shares the reading conventions at the top of PartitionMerge. *)
(***************************************************************************)

EXTENDS Naturals, FiniteSets

CONSTANTS
  Nodes,        \* the issuers/serving nodes.  Each holds its OWN record --
                \* design 12.6.5: "a sibling issues a fresh attestation
                \* reflecting whatever rotation reached the replicated state",
                \* so there is no global current key and none is modelled.
  MaxGen,       \* key generations are 0..MaxGen
  SeriesCheck   \* TRUE iff the SUBJECT observes light-client's "never take a
                \* series reissue into a series you have occupied before".
                \* A constant because the whole point is to run it both ways.

NoKey == 0 - 1   \* not a generation; "serving nothing"

Gens == 0..MaxGen

VARIABLES
  record,    \* record[n] : the generation node n currently records
  everHeld,  \* everHeld[n] : every generation n has ever recorded
  superseded,\* superseded[n] : every generation n has recorded AND MOVED OFF.
             \* A variable, not a derived set, and the distinction is the
             \* whole check: derive it as `everHeld \ {record}` and the
             \* current generation is excluded BY DEFINITION, so a walk-back
             \* onto a superseded generation would silently satisfy the
             \* invariant.  Supersession is history and never un-happens.
  serving,   \* serving[n] : the generation n is serving a session under, or NoKey
  badIssue,  \* set if an issuance ever named a generation n had superseded
  badServe   \* set if service ever happened under a generation n had superseded

vars == <<record, everHeld, superseded, serving, badIssue, badServe>>

Superseded(n) == superseded[n]

Init ==
  /\ record   = [n \in Nodes |-> 0]
  /\ everHeld = [n \in Nodes |-> {0}]
  /\ superseded = [n \in Nodes |-> {}]
  /\ serving  = [n \in Nodes |-> NoKey]
  /\ badIssue = FALSE
  /\ badServe = FALSE

(***************************************************************************)
(* ISSUANCE.  Unguarded on purpose: the node issues for whatever it        *)
(* records, and the flag notices if that was a generation it had already   *)
(* superseded.  infra-client: "issue only for the key you currently        *)
(* record ... nobody can check this for you".                              *)
(***************************************************************************)
Issue(n) ==
  /\ badIssue' = (badIssue \/ (record[n] \in Superseded(n)))
  /\ UNCHANGED <<record, everHeld, superseded, serving, badServe>>

(***************************************************************************)
(* SERVICE -- a delivery or a trust-bearing operation on a live session.   *)
(* Also unguarded, for the same reason.                                    *)
(***************************************************************************)
Serve(n) ==
  /\ serving[n] # NoKey
  /\ badServe' = (badServe \/ (serving[n] \in Superseded(n)))
  /\ UNCHANGED <<record, everHeld, superseded, serving, badIssue>>

Attach(n) ==
  /\ serving[n] = NoKey
  /\ serving' = [serving EXCEPT ![n] = record[n]]
  /\ UNCHANGED <<record, everHeld, superseded, badIssue, badServe>>

(***************************************************************************)
(* THE ADOPTION ARRIVES AT ONE NODE.  This is the whole discipline in one  *)
(* action, and design 12.6.5's two halves are the two conjuncts: the       *)
(* record is OVERWRITTEN, and any session under the outgoing generation    *)
(* TERMINATES ("sessions under the superseded credential terminate, and    *)
(* nothing further is delivered to it").                                   *)
(*                                                                         *)
(* Arrival is per node.  Nothing here touches any other node's state, so   *)
(* a sibling the adoption has not reached keeps its old record and goes on *)
(* issuing -- correctly.  That is the ignorance design 12.6.5 prices, and  *)
(* the reason `record` is a function on Nodes rather than a single value.  *)
(***************************************************************************)
AdoptionArrives(n, g) ==
  /\ g \in Gens
  /\ g # record[n]
  /\ SeriesCheck => (g \notin everHeld[n])
  /\ record'   = [record   EXCEPT ![n] = g]
  /\ everHeld' = [everHeld EXCEPT ![n] = everHeld[n] \cup {g}]
  /\ superseded' = [superseded EXCEPT ![n] = superseded[n] \cup {record[n]}]
  /\ serving'  = [serving  EXCEPT ![n] = IF serving[n] = record[n]
                                           THEN NoKey ELSE serving[n]]
  /\ UNCHANGED <<badIssue, badServe>>

Next ==
  \/ \E n \in Nodes : Issue(n)
  \/ \E n \in Nodes : Serve(n)
  \/ \E n \in Nodes : Attach(n)
  \/ \E n \in Nodes, g \in Gens : AdoptionArrives(n, g)

Spec == Init /\ [][Next]_vars

(***************************************************************************)
(* PROPERTIES                                                              *)
(***************************************************************************)

TypeOK ==
  /\ record   \in [Nodes -> Gens]
  /\ everHeld \in [Nodes -> SUBSET Gens]
  /\ superseded \in [Nodes -> SUBSET Gens]
  /\ serving  \in [Nodes -> Gens \cup {NoKey}]
  /\ badIssue \in BOOLEAN
  /\ badServe \in BOOLEAN

\* The currency obligation.  Tamarin proves this only under a bound; here it
\* is checked over every reachable state of the instance.
NeverIssuedForASupersededKey == ~badIssue

\* The attach obligations, both halves of design 12.6.5's rule.
NothingServedAfterSupersession == ~badServe

\* The structural reason the two above hold, stated separately so a failure
\* says WHICH property broke: a node's record is never one it has superseded,
\* and it never serves under one.  These are what "enforced by replacement
\* rather than by a check" means concretely.
RecordIsNeverSuperseded  == \A n \in Nodes : record[n] \notin Superseded(n)
ServingIsNeverSuperseded == \A n \in Nodes : serving[n] \notin Superseded(n)

=============================================================================
