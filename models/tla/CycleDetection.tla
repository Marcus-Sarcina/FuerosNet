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
(* Shares the reading conventions of PartitionMerge.tla.                    *)
(***************************************************************************)

EXTENDS Naturals, FiniteSets, Sequences

CONSTANTS
  Nodes,       \* participating nodes
  None,        \* "no patron" sentinel
  MaxEvents    \* bound on adoptions, for finiteness

ASSUME None \notin Nodes

VARIABLES
  patron,      \* patron[c] : c's actual current patron (ground truth held
               \* by c itself), or None
  memos,       \* the multiset (modelled as a set of records, each unique
               \* by a serial number) of memos in flight or held.  A memo:
               \*   [about |-> the node whose position it reports,
               \*    pat   |-> that node's patron per the memo,
               \*    at    |-> the node currently HOLDING the memo,
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
       {[about |-> c, pat |-> p, at |-> p, id |-> serial]}
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
  \* The memo advances to its holder's patron.  We replace the memo record
  \* with a copy whose `at` has moved up; same id, so it is the same memo.
  /\ LET moved == [m EXCEPT !.at = patron[m.at]]
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
(* Which edge is cut: the detector disavows its own subordinate on the     *)
(* branch the memo came up -- in this minimal model, the detector breaks    *)
(* the cycle by cutting the patron edge OF the node whose adoption closed   *)
(* it.  We cut the edge (detector-as-patron -> the child that leads back    *)
(* around).  Concretely we remove the patron pointer that keeps the        *)
(* detector on the cycle: the detector's own outgoing membership as a       *)
(* child is not what it may cut (that is someone else's authority); it      *)
(* cuts a subordinate's edge.  The subordinate on the cycle is the node     *)
(* whose patron is the detector AND which reaches the detector going up.    *)
(***************************************************************************)

DetectAndRepair(m) ==
  /\ m \in memos
  /\ m.at = m.about                 \* the memo returned to its subject: a cycle
  /\ OnCycle(m.about)               \* confirmed against the detector's records
  \* Find the detector's direct subordinate that lies on the cycle: a child
  \* whose patron is the detector and which climbs back to the detector.
  /\ \E child \in Nodes :
        /\ patron[child] = m.about
        /\ ReachesUp(child, m.about, Cardinality(Nodes))
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

Fairness ==
  /\ \A m \in [about: Nodes, pat: Nodes, at: Nodes, id: 0..MaxEvents] :
       WF_vars(Forward(m))
  /\ \A m \in [about: Nodes, pat: Nodes, at: Nodes, id: 0..MaxEvents] :
       WF_vars(DetectAndRepair(m))

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
       /\ m.at \in Nodes /\ m.id \in 0..MaxEvents

\* LIVENESS (the design's claim): a cycle never persists forever.  If a
\* cycle exists it is eventually broken -- reconciliation carries the memo
\* the whole way up and the detector cuts an edge.  <>[]~HasCycle would be
\* too strong (a NEW cycle may form later); the honest statement is that
\* the system does not get STUCK in a cycle: it is always the case that a
\* cycle is eventually resolved.  [](HasCycle => <>~HasCycle): whenever a
\* cycle holds, it is eventually gone.
CyclesResolve == [](HasCycle => <>(~HasCycle))

\* SAFETY: repair only cuts genuine cycles.  An edge is disavowed for
\* reason 5 only in a step where the cut node was on a cycle -- the memo is
\* a hint confirmed against records (wire Section 10.2: "No node acts on
\* a memo alone").  We assert the contrapositive as an invariant on the
\* action's guard: disavowed edges only grow via DetectAndRepair, whose
\* guard requires OnCycle.  (Checked structurally by the guard; stated here
\* for the reader, and TypeOK plus the guard enforce it.)
\* No operator is defined for this either: it is enforced by the action's
\* guard rather than by a state predicate, and a definition equal to TRUE
\* would read as a checked property while checking nothing.

=============================================================================
