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
(* WHAT A NODE HOLDS IS THE CHAIN, NOT A GENERATION.  wire 4.6.1: a node   *)
(* proves its current series "by presenting its adoption ... and each     *)
(* series reissue since", and "chain length is the order" -- where two    *)
(* chains share an adoption and one extends the other, the longer is      *)
(* current.  wire 4.6: the new series "MUST NOT be one the node has        *)
(* previously occupied", and that is "checkable by anyone holding the      *)
(* chain, who MUST reject a reissue naming a series already in it".  So    *)
(* the subject here has ONE chain, each node holds SOME PREFIX of it (the  *)
(* chain as it stood when that node last took an arrival), and a node's    *)
(* record is the tip of what it holds.                                     *)
(*                                                                         *)
(* AN EARLIER VERSION HELD A GENERATION NUMBER PER NODE and applied the     *)
(* series rule to the generations that node had itself recorded.  A        *)
(* cross-family review showed the gap: a node that had recorded 0 and then *)
(* 2 would take 1 as current, having never seen 1 -- a series the chain    *)
(* proving 2 already shows was occupied and left.  Every invariant stayed  *)
(* green, because "superseded" meant "what I moved off", and this node had *)
(* moved off nothing it was now recording.  With the chain held, the same  *)
(* arrival is a shorter chain and wire 4.6.1 orders it out; the mutation   *)
(* SupersessionDiscipline_OrderMutation.cfg drops that rule and expects    *)
(* the stale series to be issued for.                                      *)
(*                                                                         *)
(* This model shares the reading conventions at the top of PartitionMerge. *)
(***************************************************************************)

EXTENDS Naturals, FiniteSets, Sequences

CONSTANTS
  Nodes,        \* the issuers/serving nodes.  Each holds its OWN copy of the
                \* chain -- design 12.6.5: "a sibling issues a fresh
                \* attestation reflecting whatever rotation reached the
                \* replicated state", so there is no global current key and
                \* none is modelled.
  MaxGen,       \* series identifiers are 0..MaxGen
  MaxLen,       \* bound on the chain's length, for finiteness.  Under the
                \* series rule no series repeats, so MaxGen+1 loses nothing;
                \* the bound only bites in the mutation that drops the rule.
  SeriesCheck,  \* TRUE iff the series rule is observed -- by the subject
                \* (light-client 2: "never take a series reissue into a
                \* series you have occupied before") and by any node holding
                \* the chain (wire 4.6: reject a reissue naming a series
                \* already in it).  A constant because the whole point is to
                \* run it both ways.
  OrderCheck    \* TRUE iff wire 4.6.1's ordering is observed: chain length
                \* is the order, so a node never replaces the chain it holds
                \* with a shorter one.  Likewise run both ways.

NoKey == 0 - 1   \* not a series; "serving nothing"

Gens == 0..MaxGen

VARIABLES
  chain,     \* the subject's chain: its adoption's series, then the series
             \* entered by each reissue, in order
  held,      \* held[n] : the chain node n holds -- a prefix of `chain`
  superseded,\* superseded[n] : every series n knows the subject has moved off.
             \* A variable, not a derived set, and the distinction is the
             \* whole check: derive it from the chain n currently holds and
             \* a walk-back onto a shorter chain would forget the series it
             \* had learned were left.  Supersession is history and never
             \* un-happens.
  serving,   \* serving[n] : the series n is serving a session under, or NoKey
  badIssue,  \* set if an issuance ever named a series n had superseded
  badServe   \* set if service ever happened under a series n had superseded

vars == <<chain, held, superseded, serving, badIssue, badServe>>

Last(s)   == s[Len(s)]
Range(s)  == {s[i] : i \in 1..Len(s)}
NonTip(s) == {s[i] : i \in 1..(Len(s) - 1)}   \* every series the chain left

Record(n)     == Last(held[n])   \* the series n currently records
Superseded(n) == superseded[n]

Init ==
  /\ chain    = <<0>>
  /\ held     = [n \in Nodes |-> <<0>>]
  /\ superseded = [n \in Nodes |-> {}]
  /\ serving  = [n \in Nodes |-> NoKey]
  /\ badIssue = FALSE
  /\ badServe = FALSE

(***************************************************************************)
(* ISSUANCE.  Unguarded on purpose: the node issues for whatever it        *)
(* records, and the flag notices if that was a series it had already       *)
(* superseded.  infra-client: "issue only for the key you currently        *)
(* record ... nobody can check this for you".                              *)
(***************************************************************************)
Issue(n) ==
  /\ badIssue' = (badIssue \/ (Record(n) \in Superseded(n)))
  /\ UNCHANGED <<chain, held, superseded, serving, badServe>>

(***************************************************************************)
(* SERVICE -- a delivery or a trust-bearing operation on a live session.   *)
(* Also unguarded, for the same reason.                                    *)
(***************************************************************************)
Serve(n) ==
  /\ serving[n] # NoKey
  /\ badServe' = (badServe \/ (serving[n] \in Superseded(n)))
  /\ UNCHANGED <<chain, held, superseded, serving, badIssue>>

Attach(n) ==
  /\ serving[n] = NoKey
  /\ serving' = [serving EXCEPT ![n] = Record(n)]
  /\ UNCHANGED <<chain, held, superseded, badIssue, badServe>>

(***************************************************************************)
(* THE SUBJECT REISSUES into series g.  wire 4.6: the new series must not  *)
(* be one it has occupied before -- the light client's own rule, checked   *)
(* against its own chain.  Nothing arrives anywhere yet.                   *)
(***************************************************************************)
Reissue(g) ==
  /\ Len(chain) < MaxLen
  /\ g \in Gens
  /\ g # Last(chain)                       \* a reissue leaves one series for another
  /\ SeriesCheck => (g \notin Range(chain))
  /\ chain' = Append(chain, g)
  /\ UNCHANGED <<held, superseded, serving, badIssue, badServe>>

(***************************************************************************)
(* THE CHAIN ARRIVES AT ONE NODE, as it stood at some point: prefix k of   *)
(* the subject's chain.  Arrival is per node and unordered -- a node may   *)
(* see the chain's third state before its second, or never see the second *)
(* at all -- which is the ignorance design 12.6.5 prices, and the reason   *)
(* `held` is a function on Nodes rather than a single value.              *)
(*                                                                         *)
(* Two checks, each a constant: wire 4.6.1 orders by length, so a shorter *)
(* chain does not replace a longer one; wire 4.6 rejects a chain in which  *)
(* a series recurs.  Then design 12.6.5's two halves are the two           *)
(* remaining conjuncts: the record is OVERWRITTEN (the held chain is       *)
(* replaced, and every series it shows the subject left is superseded),   *)
(* and any session under the outgoing series TERMINATES ("sessions under  *)
(* the superseded credential terminate, and nothing further is delivered   *)
(* to it").                                                                *)
(***************************************************************************)
ChainArrives(n, k) ==
  /\ k \in 1..Len(chain)
  /\ k # Len(held[n])
  /\ OrderCheck => (k > Len(held[n]))
  /\ LET new == SubSeq(chain, 1, k)
     IN /\ SeriesCheck => (Cardinality(Range(new)) = Len(new))
        /\ held' = [held EXCEPT ![n] = new]
        /\ superseded' = [superseded EXCEPT ![n] =
             superseded[n] \cup NonTip(new) \cup ({Record(n)} \ {Last(new)})]
        /\ serving' = [serving EXCEPT ![n] = IF serving[n] = Record(n)
                                               THEN NoKey ELSE serving[n]]
  /\ UNCHANGED <<chain, badIssue, badServe>>

Next ==
  \/ \E n \in Nodes : Issue(n)
  \/ \E n \in Nodes : Serve(n)
  \/ \E n \in Nodes : Attach(n)
  \/ \E g \in Gens : Reissue(g)
  \/ \E n \in Nodes, k \in 1..MaxLen : ChainArrives(n, k)

Spec == Init /\ [][Next]_vars

(***************************************************************************)
(* PROPERTIES                                                              *)
(***************************************************************************)

TypeOK ==
  /\ chain \in Seq(Gens) /\ Len(chain) \in 1..MaxLen
  /\ held \in [Nodes -> Seq(Gens)] /\ \A n \in Nodes : Len(held[n]) >= 1
  /\ superseded \in [Nodes -> SUBSET Gens]
  /\ serving  \in [Nodes -> Gens \cup {NoKey}]
  /\ badIssue \in BOOLEAN
  /\ badServe \in BOOLEAN

\* Structural: what a node holds is always some past state of the subject's
\* chain.  The subject here is honest and has one chain; two chains that
\* diverge are patron equivocation, which wire 4.6.1 makes visible rather
\* than prevents, and which this model does not contain.
HeldIsAPrefix == \A n \in Nodes : held[n] = SubSeq(chain, 1, Len(held[n]))

\* The currency obligation.  Tamarin proves this only under a bound; here it
\* is checked over every reachable state of the instance.
NeverIssuedForASupersededKey == ~badIssue

\* The attach obligations, both halves of design 12.6.5's rule.
NothingServedAfterSupersession == ~badServe

\* The structural reason the two above hold, stated separately so a failure
\* says WHICH property broke: a node's record is never one it has superseded,
\* and it never serves under one.  These are what "enforced by replacement
\* rather than by a check" means concretely.
RecordIsNeverSuperseded  == \A n \in Nodes : Record(n) \notin Superseded(n)
ServingIsNeverSuperseded == \A n \in Nodes : serving[n] \notin Superseded(n)

=============================================================================
