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
(* THE MEMO IS THE PATRON'S, AND SO IS THE CHECK.  design Section 15.2:    *)
(* "A memo is a patron's statement about one of its own subordinate slots: *)
(* it names the patron, the patron's position, which slot, when, and who   *)
(* is in it."  wire Section 10.2.1: "field 1 names the patron it speaks    *)
(* for.  If field 1 is you, a memo you originated has come back to you     *)
(* from below."  So field 1 is the PATRON, the detector is the patron, and *)
(* the confirmation is against the patron's own slot row -- design Section *)
(* 18.2 says exactly that of the replay case, a captured memo "matches     *)
(* that patron's own row".                                                 *)
(*                                                                         *)
(* AN EARLIER VERSION NAMED THE OCCUPANT.  Its memo carried `about = c`,   *)
(* the adopted child, fired when the memo reached the CHILD, and confirmed *)
(* by asking whether the child's own patron pointer still agreed.  A       *)
(* cross-family review found it: that is a different mechanism, detecting  *)
(* from the opposite end of the same loop, and the two-node cycle hides    *)
(* the difference because either end cuts one of the two edges.  The memo  *)
(* now names its patron, travels from the patron's own patron rootward,    *)
(* and fires where field 1 comes home.                                     *)
(*                                                                         *)
(* WHAT THE DETECTOR MAY CHECK, and nothing more.  It holds the memo and   *)
(* its own row.  It does NOT hold the loop: no node can see a cycle it is  *)
(* part of, which is why the memo exists.  So the guard is the row and     *)
(* not a global cycle test -- an earlier version required the cycle to be  *)
(* genuine, a check Section 1.1 puts beyond the party asked to make it.    *)
(* The consequence is design Section 18.2's accepted case, now reachable   *)
(* here: a memo matching the current row can sever an edge that is on no   *)
(* cycle, bounded rather than prevented -- "the edge severed is the one    *)
(* that handed the memo over, and the disavowal is reason code 5 with      *)
(* re-adoption available".                                                 *)
(*                                                                         *)
(* WHAT IS NOT CLAIMED.  That every edge cut lies on a live cycle; see     *)
(* above.  A property asserting it was tried while repairing this file and *)
(* TLC refuted it in two seconds.  The claim checked here is the design's, *)
(* that cycles resolve.                                                    *)
(*                                                                         *)
(* ONE RELATION STANDS FOR THE RELATIONSHIP.  `patron` is read both as the *)
(* child's binding and as the patron's slot row, so the two cannot diverge *)
(* here.  They can in the design -- a departure is unilateral (Section     *)
(* 6.2), so a patron's row can name an occupant that has left -- and this  *)
(* model has nothing to say about that case, because it models no          *)
(* departure.  Nor does it model the timestamp rule that stops a replayed  *)
(* memo at the first table-holding hop (wire Section 10.2).                *)
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
               \*   [pat   |-> FIELD 1: the patron the memo speaks for,
               \*    occ   |-> who is in the slot it reports,
               \*    at    |-> the node currently HOLDING the memo,
               \*    from  |-> the node that handed it to `at` -- the
               \*             arrival branch, which repair cuts,
               \*    id    |-> a unique serial]
               \* The patron's position and the memo's timestamp (design
               \* Section 15.2) are not modelled: no path arithmetic is
               \* involved in the cycle check (wire Section 10.2.1), and
               \* the timestamp serves the forwarding rule this model
               \* omits.
               \* Rootward travel is modelled by the memo's `at` moving up
               \* patron edges (the Forward action).
  disavowed,   \* set of {patron, child} edges cut by cycle-repair (reason
               \* 5).  A cut edge is removed from the patron relation.  It is
               \* a RECORD OF WHAT WAS CUT and nothing more: reason 5 is
               \* "without prejudice" (wire Section 10.2.4) and design
               \* Section 18.2 bounds the replay case partly ON re-adoption
               \* remaining available, so nothing here refuses to remake a
               \* cut edge.  An earlier version made this a permanent
               \* blacklist, which removed edges from the model's future for
               \* good and made cycles easier to be rid of than the protocol
               \* makes them.
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
(* ACTION: Adopt(c, p).  c is adopted by p, and P originates a memo about  *)
(* its own slot (design Section 15.2: a membership change travels          *)
(* rootward, and the memo is the patron's statement about its subordinate  *)
(* slot).  "A receiving node forwards the memo to its own patron" (wire    *)
(* Section 10.2), so the memo's first hop is p's patron and that is where  *)
(* it starts here.  A root has no patron and forwarding stops there, so a  *)
(* rootless adopter originates no travelling memo -- which costs nothing:  *)
(* the adoption that CLOSES a cycle is always one whose adopter already    *)
(* has a patron chain leading back to the adoptee, so its memo travels.    *)
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
  /\ patron' = [patron EXCEPT ![c] = p]
  /\ memos' = memos \cup
       (IF patron[p] = None
          THEN {}                     \* p is a root: forwarding stops there
          ELSE {[pat |-> p, occ |-> c, at |-> patron[p], from |-> p,
                 id |-> serial]})
  /\ serial' = serial + 1
  /\ events' = events + 1
  /\ UNCHANGED disavowed

(***************************************************************************)
(* ACTION: Forward(m).  A memo held at a node that is not the patron it    *)
(* names moves up one patron edge -- rootward travel (design Section       *)
(* 15.2).  Forwarding unchanged is the rule; the memo's `pat`/`occ` never  *)
(* change, only `at` and the arrival branch.  A memo whose holder has no   *)
(* patron has reached a root and simply stops.                             *)
(*                                                                         *)
(* Loss is modelled by NOT forwarding -- a memo that never advances is the *)
(* lost memo.  Reconciliation (replay) is Forward firing later: the design *)
(* needs no distinct mechanism and neither does this model.                *)
(***************************************************************************)

Forward(m) ==
  /\ m \in memos
  /\ patron[m.at] # None
  /\ m.at # m.pat                   \* field 1 is not this holder: keep climbing
  \* The memo advances to its holder's patron, and remembers who handed it
  \* over.  We replace the memo record with a copy whose `at` has moved up;
  \* same id, so it is the same memo.
  /\ LET moved == [m EXCEPT !.at = patron[m.at], !.from = m.at]
     IN memos' = (memos \ {m}) \cup {moved}
  /\ UNCHANGED <<patron, disavowed, events, serial>>

(***************************************************************************)
(* ACTION: DetectAndRepair(m).  The cycle check: a memo has arrived at the *)
(* patron it names (m.at = m.pat) -- field 1 identity match (wire Section  *)
(* 10.2.1: "If field 1 is you, a memo you originated has come back to you  *)
(* from below").  The detector confirms the memo against its OWN slot row  *)
(* -- it still holds that occupant -- and cuts the edge on the arrival     *)
(* branch with a reason-5 disavowal.                                       *)
(*                                                                         *)
(* Which edge is cut: the detector disavows the direct subordinate that    *)
(* forwarded the memo to it (wire Section 10.2.4) -- `from`, the arrival    *)
(* branch.  The memo climbed patron edges from the detector's own patron   *)
(* back to the detector, so the nodes it passed lie on that walk, `from`   *)
(* last of all.  The                                                       *)
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
  /\ m.at = m.pat                   \* field 1 is you: a memo you originated
                                    \* has come back to you from below
  /\ patron[m.occ] = m.pat          \* confirmed against your own row: you
                                    \* still hold that occupant in that slot
  /\ \E child \in Nodes :
        /\ patron[child] = m.pat
        /\ CutAnyChild \/ child = m.from   \* the arrival branch, and only it
        /\ patron' = [patron EXCEPT ![child] = None]  \* reason-5 disavowal
        /\ disavowed' = disavowed \cup {{m.pat, child}}
  \* The memo terminates here (wire Section 10.2.1: the check "fires at any
  \* depth, on the memo alone, and terminates the memo there").
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
       /\ m.pat \in Nodes /\ m.occ \in Nodes
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
