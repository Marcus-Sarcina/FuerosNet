------------------------- MODULE CycleDetection -------------------------
(***************************************************************************)
(* Rootward-memo cycle detection under concurrent adoptions.               *)
(*                                                                         *)
(* WHAT THIS MODEL IS                                                      *)
(* design Section 15.2 / wire Section 10.2 detect a patron cycle -- a node *)
(* that has become its own ancestor -- with a minified "memo" that travels *)
(* rootward, and the cycle check is an IDENTITY comparison (field 1 = you)  *)
(* rather than path arithmetic.  The review plan's Stage 1.2 asks whether  *)
(* "cycle detection under concurrent adoptions" is correct.  The property: *)
(* whenever the patron graph actually contains a cycle, a memo eventually   *)
(* reaches a node it names as its own ancestor, which fires the check and  *)
(* (after confirming against its own records) issues a reason-5 disavowal  *)
(* that breaks the cycle -- even under message loss, with reconciliation   *)
(* (replay) as the only repair.                                            *)
(*                                                                         *)
(* EVIDENCE IS ABSTRACTED AWAY (design Section 6.1.1): every adoption      *)
(* modelled here is taken to be well-formed already.  Worth stating        *)
(* because the requirement makes cycles COSTLIER without making them       *)
(* impossible -- each concurrent adoption closing a loop must carry a      *)
(* meeting or a former patron's countersignature -- and cost is not what   *)
(* this theory is about.                                                   *)
(*                                                                         *)
(* HOW A CYCLE CAN FORM.  In a healthy network the patron relation is a    *)
(* forest.  A cycle needs concurrency: two adoptions, each individually    *)
(* legal against the adopter's LOCAL view, that together close a loop --   *)
(* A adopts B while, concurrently and out of A's view, a chain from B back *)
(* to A is being built.  design Section 3 makes the authority relation a   *)
(* tree within a subnet -- exactly one patron per non-root node, acyclic  *)
(* within a subtree -- but a stale local view lets the second adoption     *)
(* happen before the first has propagated.  This model builds exactly such *)
(* races and checks the memo catches them.                                 *)
(*                                                                         *)
(* WHICH EDGE IS CUT, and why the memo carries its arrival branch.  wire   *)
(* Section 10.2.4: the detector "disavows the direct subordinate that       *)
(* forwarded the memo to it"; design Section 18.2 bounds the replay attack  *)
(* the same way -- "the edge severed is the one that handed the memo over". *)
(* So a memo here records `from`, the node that handed it to its holder,    *)
(* and repair cuts that node and no other.  A cross-family review found the *)
(* earlier version choosing ANY direct subordinate that "climbs back to the *)
(* detector" -- a test every direct subordinate passes in one hop -- and    *)
(* showed that with four nodes the memos can be spent cutting innocent      *)
(* off-cycle children while the cycle stands.  Three nodes hid it, each    *)
(* cycle node having exactly one child.  CycleDetection_FourNodes.cfg is    *)
(* the regression and CycleDetection_Mutation.cfg switches the old choice   *)
(* back on, where CyclesResolve must fail.                                 *)
(*                                                                         *)
(* AND THE MEMO IS CHECKED AGAINST THE DETECTOR'S ROW.  wire Section 10.2  *)
(* has the detector act "once the memo is confirmed against its own        *)
(* records", and the memo carries what it claims -- field 2 here, `pat`.   *)
(* The earlier version carried `pat` and never compared it, confirming     *)
(* only that the detector was on SOME cycle.  Repairing the four-node     *)
(* instance exposed why that is not enough: a memo that has arrived and    *)
(* not yet been acted on can outlive the loop it travelled -- another      *)
(* memo's repair breaks that loop, the detector is re-adopted elsewhere    *)
(* and a new loop forms -- and the stale memo's arrival branch is then an  *)
(* innocent child.  A memo whose stated patron is no longer the detector's *)
(* patron is a replay of a past slot state, and stops.                     *)
(*                                                                         *)
(* WHAT IS NOT CLAIMED.  That every edge cut lies on a live cycle.  design *)
(* Section 18.2 accepts that a replayed memo matching the detector's       *)
(* current row can sever an edge, and bounds the damage rather than       *)
(* preventing it: the edge is the one that handed the memo over, reason 5, *)
(* re-adoption available.  A property asserting more than that was tried  *)
(* while repairing this file and TLC refuted it in two seconds; the claim  *)
(* checked here is the design's, that cycles resolve.                     *)
(*                                                                         *)
(* Shares the reading conventions of PartitionMerge.tla.                    *)
(***************************************************************************)

EXTENDS Naturals, FiniteSets, Sequences

CONSTANTS
  Nodes,       \* participating nodes
  None,        \* "no patron" sentinel
  MaxEvents,   \* bound on adoptions, for finiteness
  CutAnyChild  \* FALSE: repair cuts the subordinate that handed the memo
               \* over (the rule).  TRUE only in the mutation: any direct
               \* subordinate may be cut, which is the defect described above.

ASSUME None \notin Nodes

VARIABLES
  patron,      \* patron[c] : c's actual current patron (ground truth held
               \* by c itself), or None
  memos,       \* the multiset (modelled as a set of records, each unique
               \* by a serial number) of memos in flight or held.  A memo:
               \*   [about |-> the node whose position it reports,
               \*    pat   |-> that node's patron per the memo,
               \*    at    |-> the node currently HOLDING the memo,
               \*    from  |-> the node that handed it to `at` -- the
               \*             arrival branch, which repair cuts,
               \*    id    |-> a unique serial]
               \* Rootward travel is modelled by the memo's `at` moving up
               \* patron edges (the Forward action).
  disavowed,   \* set of {patron, child} edges cut by cycle-repair (reason
               \* 5).  A cut edge is removed from the patron relation.
  events,      \* adoption counter (bounds the model)
  serial       \* next unused memo id

vars == <<patron, memos, disavowed, events, serial>>

(***************************************************************************)
(* DERIVED: does the patron relation currently contain a cycle?            *)
(* We compute, for each node, whether following patron pointers returns to *)
(* it within |Nodes| steps.  TLC evaluates this by explicit iteration.     *)
(* RECURSIVE declares an operator that refers to itself -- TLA+ requires   *)
(* the forward declaration.                                                *)
(***************************************************************************)

RECURSIVE ReachesUp(_, _, _)
\* Starting from `cur`, following patron pointers, do we reach `target`
\* within `fuel` steps?  fuel caps the walk so a cycle cannot loop forever
\* inside the operator itself.
ReachesUp(cur, target, fuel) ==
  IF cur = None THEN FALSE
  ELSE IF cur = target THEN TRUE
  ELSE IF fuel = 0 THEN FALSE
  ELSE ReachesUp(patron[cur], target, fuel - 1)

\* c sits on a cycle iff, starting from ITS patron, we can climb back to c.
OnCycle(c) ==
  /\ patron[c] # None
  /\ ReachesUp(patron[c], c, Cardinality(Nodes))

HasCycle == \E c \in Nodes : OnCycle(c)

(***************************************************************************)
(* INITIAL STATE.                                                          *)
(***************************************************************************)

Init ==
  /\ patron = [c \in Nodes |-> None]
  /\ memos = {}
  /\ disavowed = {}
  /\ events = 0
  /\ serial = 0

(***************************************************************************)
(* ACTION: Adopt(c, p).  c is adopted by p and immediately originates a    *)
(* memo about itself (design Section 15.2: a membership change travels     *)
(* rootward).  The memo starts held at p -- the parent forwards it onward.  *)
(*                                                                         *)
(* The concurrency that creates cycles: we DO NOT forbid an adoption that  *)
(* closes a loop here.  In the real protocol the closing adoption is one   *)
(* the adopter believed legal against a stale view; the memo mechanism is  *)
(* what catches it AFTER the fact.  Forbidding it in the model would assume *)
(* the very global check the design deliberately does not have, and the    *)
(* memo would have nothing to detect.  We forbid only re-adopting a node   *)
(* that already has a patron (design Section 3.1.1's one-patron rule holds  *)
(* against the adopter's own knowledge, which for a direct child it has).  *)
(***************************************************************************)

Adopt(c, p) ==
  /\ events < MaxEvents
  /\ patron[c] = None
  /\ c # p
  /\ {p, c} \notin disavowed          \* a cut edge is not silently remade
  /\ patron' = [patron EXCEPT ![c] = p]
  /\ memos' = memos \cup
       {[about |-> c, pat |-> p, at |-> p, from |-> c, id |-> serial]}
  /\ serial' = serial + 1
  /\ events' = events + 1
  /\ UNCHANGED disavowed

(***************************************************************************)
(* ACTION: Forward(m).  A memo held at node u, where u is not the node the  *)
(* memo is about-ancestor-of yet, moves up one patron edge -- rootward     *)
(* travel (design Section 15.2).  Forwarding unchanged is the rule; the    *)
(* memo's `about`/`pat` never change, only `at`.  A memo whose holder has   *)
(* no patron has reached a root and simply stops (no new memo replaces it). *)
(*                                                                         *)
(* Loss is modelled by NOT forwarding -- a memo that never advances is the *)
(* lost memo.  Reconciliation (replay) is Forward firing later: the design *)
(* needs no distinct mechanism and neither does this model.                *)
(***************************************************************************)

Forward(m) ==
  /\ m \in memos
  /\ patron[m.at] # None
  /\ m.at # m.about                 \* a memo does not climb past its subject
  \* The memo advances to its holder's patron, and remembers who handed it
  \* over.  We replace the memo record with a copy whose `at` has moved up;
  \* same id, so it is the same memo.
  /\ LET moved == [m EXCEPT !.at = patron[m.at], !.from = m.at]
     IN memos' = (memos \ {m}) \cup {moved}
  /\ UNCHANGED <<patron, disavowed, events, serial>>

(***************************************************************************)
(* ACTION: DetectAndRepair(m).  The cycle check: a memo has arrived at the *)
(* very node it is about (m.at = m.about), meaning that node is its own    *)
(* ancestor -- field 1 identity match (wire Section 10.2: "If field 1 is   *)
(* you, a memo you originated has come back to you from below").  The       *)
(* detector confirms against its OWN records (here: the cycle genuinely     *)
(* exists, which the detector can verify because it is a party to its own   *)
(* patron edge) and cuts the edge on the arrival branch with a reason-5     *)
(* disavowal.                                                              *)
(*                                                                         *)
(* Which edge is cut: the detector disavows the direct subordinate that    *)
(* forwarded the memo to it (wire Section 10.2.4) -- `from`, the arrival    *)
(* branch.  That subordinate is on the cycle by construction: the memo     *)
(* climbed patron edges from its subject back to its subject, so every     *)
(* node it passed through lies on the loop, `from` last of all.  The       *)
(* detector's own outgoing membership as a child is not what it may cut    *)
(* (that is someone else's authority, design Section 6.2.2); it cuts a     *)
(* subordinate's edge, and it has exactly one candidate.                   *)
(*                                                                         *)
(* THE EARLIER TEST WAS VACUOUS.  It admitted any child whose patron is    *)
(* the detector "and which reaches the detector going up" -- but a direct  *)
(* child reaches its patron in one hop, so the second conjunct excluded    *)
(* nobody, and the choice among children was free.  CutAnyChild restores   *)
(* that freedom for the mutation and for nothing else.                     *)
(***************************************************************************)

DetectAndRepair(m) ==
  /\ m \in memos
  /\ m.at = m.about                 \* the memo returned to its subject: a cycle
  /\ m.pat = patron[m.about]        \* and it matches the detector's own row --
                                    \* a memo naming a patron the detector no
                                    \* longer has is a stale replay, and stops
  /\ OnCycle(m.about)               \* the loop is real, per those records
  /\ \E child \in Nodes :
        /\ patron[child] = m.about
        /\ CutAnyChild \/ child = m.from   \* the arrival branch, and only it
        /\ patron' = [patron EXCEPT ![child] = None]  \* reason-5 disavowal
        /\ disavowed' = disavowed \cup {{m.about, child}}
  \* The memo is consumed (it terminated at its subject, design Section 15.2).
  /\ memos' = memos \ {m}
  /\ UNCHANGED <<events, serial>>

Next ==
  \/ \E c \in Nodes, p \in Nodes : Adopt(c, p)
  \/ \E m \in memos : Forward(m)
  \/ \E m \in memos : DetectAndRepair(m)

(***************************************************************************)
(* FAIRNESS.  Forwarding and repair are weakly fair -- a memo that can      *)
(* advance eventually does (reconciliation guarantees replay), and a memo   *)
(* that has arrived at its subject on a real cycle eventually fires the     *)
(* repair.  Adoption is unfair: the property must hold for any adoption     *)
(* pattern.                                                                *)
(***************************************************************************)

\* A memo is identified by its serial: the record changes as it moves, so
\* fairness is stated per id rather than per record.  (The earlier version
\* quantified over every possible record; with `from` added that set grows
\* to |Nodes|^4 * (MaxEvents+1) conjuncts, for the same meaning.)
ForwardMemo(id) == \E m \in memos : m.id = id /\ Forward(m)
RepairMemo(id)  == \E m \in memos : m.id = id /\ DetectAndRepair(m)

Fairness ==
  /\ \A id \in 0..MaxEvents : WF_vars(ForwardMemo(id))
  /\ \A id \in 0..MaxEvents : WF_vars(RepairMemo(id))

Spec == Init /\ [][Next]_vars /\ Fairness

(***************************************************************************)
(* PROPERTIES.                                                             *)
(***************************************************************************)

TypeOK ==
  /\ patron \in [Nodes -> Nodes \cup {None}]
  /\ events \in 0..MaxEvents
  /\ serial \in 0..(MaxEvents)
  /\ \A m \in memos :
       /\ m.about \in Nodes /\ m.pat \in Nodes
       /\ m.at \in Nodes /\ m.from \in Nodes /\ m.id \in 0..MaxEvents

\* LIVENESS (the design's claim): a cycle never persists forever.  If a
\* cycle exists it is eventually broken -- reconciliation carries the memo
\* the whole way up and the detector cuts an edge.  <>[]~HasCycle would be
\* too strong (a NEW cycle may form later); the honest statement is that
\* the system does not get STUCK in a cycle: it is always the case that a
\* cycle is eventually resolved.  [](HasCycle => <>~HasCycle): whenever a
\* cycle holds, it is eventually gone.
CyclesResolve == [](HasCycle => <>(~HasCycle))

\* SAFETY, and why none is stated.  "Repair only cuts genuine cycles" is
\* what an earlier note here claimed the guard enforced, and it enforced
\* half: the DETECTOR is on a cycle, and nothing is said of the child.
\* The full claim is not the design's -- see the header -- so no operator
\* asserts it, and a definition that held only on the instances checked
\* would be the three-node mistake again.  What the action does enforce,
\* by its guard, is which edge: the one that handed the memo over.

=============================================================================
