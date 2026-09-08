------------------------- MODULE PartitionMerge -------------------------
(***************************************************************************)
(* Partition-and-merge convergence for RHTN topology propagation.          *)
(*                                                                         *)
(* WHAT THIS MODEL IS                                                      *)
(* The review plan's Stage 1.2 names this the first thing to model: "the   *)
(* whole design rests on it and it has never been tested against anything  *)
(* but argument."  The question: after adoptions, departures, message      *)
(* loss and network partitions, do nodes' LOCAL views of the topology      *)
(* converge once the network heals -- using only the mechanisms the design *)
(* actually provides (flooding, and reconciliation-as-replay, design       *)
(* Section 15 / wire Section 10.1: "no acknowledgement and no retry ...    *)
(* reconciliation is a replay of the same frames")?                        *)
(*                                                                         *)
(* A NOTE ON READING TLA+ (for a first-time reader)                        *)
(* A TLA+ specification describes a STATE MACHINE:                         *)
(*   - VARIABLES declare the state.                                        *)
(*   - Init is a predicate saying which states are legal starting points.  *)
(*   - Next is a predicate over pairs of states: unprimed variables (x)    *)
(*     refer to the current state, primed ones (x') to the next state.     *)
(*     Next is a disjunction of ACTIONS; each action says "this step is    *)
(*     allowed when ... and afterwards the variables are ...".             *)
(*   - The model checker TLC explores EVERY reachable state and checks     *)
(*     INVARIANTS (predicates that must hold in every state) and TEMPORAL  *)
(*     PROPERTIES (predicates over whole executions, e.g. "eventually      *)
(*     always converged").                                                 *)
(* Operators used below, spelled out at first use:                         *)
(*   /\ and \/        logical AND and OR (also used as bullet lists)       *)
(*   ~ , =>           NOT, IMPLIES                                         *)
(*   \A x \in S : P   for all x in S, P holds                              *)
(*   \E x \in S : P   there exists x in S with P                           *)
(*   f[x]             function application                                 *)
(*   [x \in S |-> e]  the function mapping each x in S to e                *)
(*   [f EXCEPT ![x] = e]  a copy of f with f[x] replaced by e; inside e,   *)
(*                        @ abbreviates the old value f[x]                 *)
(*   S \cup T, \in, \subseteq   set union, membership, subset              *)
(*   CHOOSE x \in S : P   some fixed x in S satisfying P (deterministic)   *)
(*   UNCHANGED <<a,b>>    a' = a and b' = b                                *)
(***************************************************************************)

EXTENDS Naturals, FiniteSets

CONSTANTS
  Nodes,      \* the set of participating nodes, e.g. {"a","b","c","d"}
  None,       \* a sentinel meaning "no patron"; not a member of Nodes
  MaxEvents   \* bound on topology events, so the state space is finite

ASSUME None \notin Nodes

(***************************************************************************)
(* TRANSACTIONS.                                                           *)
(* A topology transaction is modelled as a record with four fields.        *)
(* [k |-> ...] is TLA+ record syntax; t.k reads a field.                   *)
(*   kind : "adopt" or "depart"                                            *)
(*   subj : the child whose position changes -- the SUBJECT, exactly wire  *)
(*          Section 10.1's storage rule ("a node stores a topology-class   *)
(*          transaction when its subject falls within its h_store")        *)
(*   pat  : the patron named by the transaction                            *)
(*   n    : an ordinal, COMPARED ONLY WITHIN ONE RELATIONSHIP.  wire      *)
(*          Section 2.3: "One series per patron relationship is the        *)
(*          expected shape -- a node bound under two patrons keeps two",   *)
(*          and design Section 19.4's P36 makes within-series the only     *)
(*          comparison and cross-series unrankable.  A single counter is   *)
(*          therefore admissible as the value: nothing here ever compares  *)
(*          two ordinals from different relationships.  An earlier version *)
(*          ordered every transaction about a subject on one scale, which  *)
(*          ranked records the design says cannot be ranked.               *)
(***************************************************************************)

Tx == [kind : {"adopt","depart"}, subj : Nodes, pat : Nodes, n : 1..MaxEvents]

VARIABLES
  bindings, \* bindings[c] : the SET of patrons c ITSELF currently
            \* acknowledges.  This is c's own ground truth -- the design
            \* has no shared state, and everything any OTHER node believes
            \* about c is derived from transactions in that node's own
            \* store.
            \*
            \* A SET, not a scalar, and design Section 6.2.1 is why: "a
            \* node that adopts elsewhere remains in the old subtree's view
            \* indefinitely, since ADOPTION SAYS NOTHING ABOUT EXISTING
            \* BINDINGS".  Moving is "adopt at the destination, depart the
            \* origin, in either order, with no requirement to do both"
            \* (Section 6.2), so holding two at once is an ordinary state
            \* and holding two indefinitely is a permitted one.  An earlier
            \* version had one scalar overwritten by each adoption, so the
            \* adopt-first order it claimed to model could not be
            \* completed: after adopting at the destination there was no
            \* origin left to depart.  A cross-family review found it.
            \*
            \* No transaction names this set -- Section 6.2: "the protocol
            \* has no concept of a node's SET of patrons" -- and none here
            \* does either: every action below names one relationship.
  store,    \* store[u] : the set of transactions u holds (its archive
            \* slice of topology-class objects, wire Section 10.1)
  events,   \* how many topology events have occurred (bounds the model)
  severed   \* the set of UNORDERED pairs {u,v} whose link is currently
            \* down -- the partition.  {u,v} is a two-element set, so the
            \* relation is symmetric by construction.

vars == <<bindings, store, events, severed>>

(***************************************************************************)
(* DERIVED NOTIONS.                                                        *)
(***************************************************************************)

\* The latest transaction u's store holds about ONE RELATIONSHIP (c under
\* p), if any.  "Latest" = highest ordinal within that relationship, which
\* is the only comparison the design permits (see Tx above).
LatestRel(u, c, p) ==
  LET about == {t \in store[u] : t.subj = c /\ t.pat = p}
  IN IF about = {} THEN None
     ELSE CHOOSE t \in about : \A s \in about : s.n <= t.n

\* u's VIEW of who c's patrons are: what u would answer if asked, from
\* u's own store alone -- the relationships whose latest record u holds is
\* an adoption.  This is the per-observer view the design insists on;
\* there is no global topology variable anywhere in this specification.
ViewPatrons(u, c) ==
  {p \in Nodes : LET t == LatestRel(u, c, p)
                 IN t # None /\ t.kind = "adopt"}

\* Communication is possible between u and v when their link is not
\* severed.  In the real system adjacency is the sessions a node holds
\* anyway (wire Section 10.1); at this model's scale (three nodes, see the
\* .cfg) every pair that isn't partitioned can exchange frames, and the
\* two-edge horizon contains everyone -- so the h_store storage test is always
\* satisfied and is deliberately NOT modelled.  (Horizon SCOPING is
\* exercised quantitatively by the flow simulator's E4; this model is
\* about ordering, loss and partition.)
CanTalk(u, v) == u # v /\ {u,v} \notin severed

(***************************************************************************)
(* INITIAL STATE: no relationships, empty stores, full connectivity.       *)
(***************************************************************************)

Init ==
  /\ bindings = [c \in Nodes |-> {}]
  /\ store  = [u \in Nodes |-> {}]
  /\ events = 0
  /\ severed = {}

(***************************************************************************)
(* ACTION: Adopt(c, p).  c is adopted by p.  Both parties sign the real    *)
(* transaction, so both hold it immediately (wire Section 10.1: "a node    *)
(* pushes topology it is a party to"); everyone else learns by flood or    *)
(* reconciliation.  The two must be able to talk (an adoption is a live    *)
(* bilateral act).                                                         *)
(*                                                                         *)
(* EVIDENCE IS ABSTRACTED AWAY, and since design Section 6.1.1 that is an  *)
(* abstraction rather than a faithful model of an optional field: every    *)
(* adoption now carries a proof of presence or a former patron's           *)
(* countersignature, and one carrying neither is malformed.  This theory   *)
(* is about ORDERING, LOSS and PARTITION, so it models adoptions that are  *)
(* already well-formed; whether the evidence checks out is a validation    *)
(* question the ceremony and recovery theories cover.  Nothing here would  *)
(* change if it were modelled -- convergence is about which transactions   *)
(* reach whom, not about why they were admitted.                           *)
(*                                                                         *)
(* AN ADOPTION ADDS A RELATIONSHIP AND ENDS NONE.  design Section 6.2.1:   *)
(* "a node that adopts elsewhere remains in the old subtree's view         *)
(* indefinitely, since adoption says nothing about existing bindings."     *)
(* So c need not be patronless, and the binding it already had SURVIVES    *)
(* this step -- which is what makes the adopt-first transfer order         *)
(* representable, since the origin is still there to be departed.  Two     *)
(* adoption transactions for one subject in flight across a partition at   *)
(* once is exactly the case worth checking here.                           *)
(*                                                                         *)
(* Supersession is per relationship: LatestRel picks the highest ordinal   *)
(* WITHIN one relationship, so a later record about (c under p) supersedes *)
(* an earlier one about (c under p) in every store that holds both, and no *)
(* record about (c under q) is ranked against either.                      *)
(***************************************************************************)

Adopt(c, p) ==
  /\ events < MaxEvents
  /\ p \notin bindings[c]
  /\ c # p
  /\ CanTalk(c, p)
  /\ LET t == [kind |-> "adopt", subj |-> c, pat |-> p, n |-> events + 1]
     IN /\ bindings' = [bindings EXCEPT ![c] = @ \cup {p}]
        /\ store'  = [store  EXCEPT ![c] = @ \cup {t},
                                    ![p] = @ \cup {t}]
        /\ events' = events + 1
        /\ UNCHANGED severed

(***************************************************************************)
(* ACTION: Depart(c, p).  Unilateral: the old patron does not sign (design *)
(* Section 6.2's escape hatch), so at the moment of signing only c itself  *)
(* holds the departure.  Everyone else -- the old patron included --       *)
(* learns of it by propagation.  This asymmetry is exactly what makes      *)
(* partition behaviour worth checking.                                     *)
(*                                                                         *)
(* IT NAMES THE RELATIONSHIP BEING ENDED, which is what completes the      *)
(* adopt-first transfer: having adopted at the destination, c departs the  *)
(* ORIGIN, not whichever binding it happens to hold latest.  An earlier    *)
(* version departed a scalar and so could only ever end the newest.        *)
(***************************************************************************)

Depart(c, p) ==
  /\ events < MaxEvents
  /\ p \in bindings[c]
  /\ LET t == [kind |-> "depart", subj |-> c, pat |-> p, n |-> events + 1]
     IN /\ bindings' = [bindings EXCEPT ![c] = @ \ {p}]
        /\ store'  = [store EXCEPT ![c] = @ \cup {t}]
        /\ events' = events + 1
        /\ UNCHANGED severed

(***************************************************************************)
(* ACTION: Gossip(u, v).  One transaction crosses one live link.  This is  *)
(* the flood, reduced to its essential property: best-effort, unordered,   *)
(* unacknowledged (wire Section 10.1: "no acknowledgement and no retry").  *)
(* Loss needs no separate action -- a message that is never gossiped IS    *)
(* the lost message.  Reconciliation (wire Section 10.1.3: "a replay of    *)
(* the same frames") is also this action: replaying a frame later is       *)
(* indistinguishable from delivering it late, which is why the design      *)
(* needs no distinct repair mechanism and this model none either.          *)
(***************************************************************************)

Gossip(u, v) ==
  /\ CanTalk(u, v)
  /\ \E t \in store[u] :
       /\ t \notin store[v]
       /\ store' = [store EXCEPT ![v] = @ \cup {t}]
  /\ UNCHANGED <<bindings, events, severed>>

(***************************************************************************)
(* ACTIONS: Partition and Heal.  The adversary (or the world) may sever    *)
(* any link and may restore it.  Bounding partitions is unnecessary:       *)
(* the temporal property below only promises convergence on executions     *)
(* where the network is EVENTUALLY healed and stays healed, which is the   *)
(* only honest claim available -- against a permanent partition the        *)
(* design promises nothing (design Section 3.1: subnets are partitionable  *)
(* and self-sufficient).                                                   *)
(***************************************************************************)

Partition(u, v) ==
  /\ u # v
  /\ {u,v} \notin severed
  /\ severed' = severed \cup {{u,v}}
  /\ UNCHANGED <<bindings, store, events>>

Heal(u, v) ==
  /\ {u,v} \in severed
  /\ severed' = severed \ {{u,v}}
  /\ UNCHANGED <<bindings, store, events>>

Next ==
  \/ \E c \in Nodes, p \in Nodes : Adopt(c, p)
  \/ \E c \in Nodes, p \in Nodes : Depart(c, p)
  \/ \E u \in Nodes, v \in Nodes : Gossip(u, v)
  \/ \E u \in Nodes, v \in Nodes : Partition(u, v)
  \/ \E u \in Nodes, v \in Nodes : Heal(u, v)

(***************************************************************************)
(* FAIRNESS.  A bare Next allows the execution in which nothing further    *)
(* ever happens, so liveness properties need fairness assumptions:         *)
(*   WF_vars(A) -- weak fairness: if action A stays enabled forever, it    *)
(*   eventually happens.  We assume weak fairness of Gossip (frames that   *)
(*   CAN cross a live link eventually do -- the design's reconciliation    *)
(*   behaviour) and of Heal (partitions eventually mend -- the healing     *)
(*   assumption the convergence claim is conditional on).  We assume NO    *)
(*   fairness for Adopt/Depart/Partition: the claim must hold however      *)
(*   few or many of those occur.                                           *)
(***************************************************************************)

Fairness ==
  /\ \A u \in Nodes, v \in Nodes : WF_vars(Gossip(u, v))
  /\ \A u \in Nodes, v \in Nodes : WF_vars(Heal(u, v))

Spec == Init /\ [][Next]_vars /\ Fairness

(***************************************************************************)
(* PROPERTIES.                                                             *)
(***************************************************************************)

\* Type correctness -- the everything-is-what-it-claims invariant that
\* catches modelling typos before they masquerade as protocol results.
TypeOK ==
  /\ bindings \in [Nodes -> SUBSET Nodes]
  /\ store \in [Nodes -> SUBSET Tx]
  /\ events \in 0..MaxEvents
  /\ severed \subseteq {{u,v} : u \in Nodes, v \in Nodes}

\* SAFETY 1: a subject is never behind its own store.  c's own patron
\* state always equals what c's own store implies -- design 16.1's "each
\* node computes its own trust ... from the transactions it observes"
\* starts with the node's view of itself being
\* right.  If this ever failed the model (or the design reading) would be
\* broken at the root.
SelfTruth == \A c \in Nodes : ViewPatrons(c, c) = bindings[c]

\* SAFETY 2: views never invent.  Every transaction a node holds
\* genuinely occurred -- it is in its subject's own store.  Gossip copies,
\* never creates; this pins it.  Over EVERY held transaction: an earlier
\* version checked only the latest per subject, under which a store could
\* carry an invented older transaction beneath a genuine latest one and
\* pass -- weaker than the name, as a cross-family review put it.
NoInvention ==
  \A u \in Nodes : \A t \in store[u] : t \in store[t.subj]

\* CONVERGENCE.  The design's claim, restated for a no-shared-state
\* system: once the network is healed and stays healed, every pair of
\* nodes eventually agrees about every subject, forever.  In temporal
\* logic:  []P means "P holds from now on"; <>P means "eventually P";
\* <>[]P means "from some point on, P holds forever".  The implication
\* below is over whole executions: IF the run eventually keeps the
\* network healed THEN it eventually keeps all views agreed.
Agreed ==
  \A u \in Nodes, v \in Nodes, c \in Nodes :
    ViewPatrons(u, c) = ViewPatrons(v, c)

\* AND ON TOPOLOGY QUIESCENCE, the second half of the antecedent, which
\* the event budget previously supplied in silence.  `events` advances on
\* every adoption and departure and on nothing else, so this says: from
\* some point on, no topology change occurs.  Without it the implication
\* is false for an indefinitely active topology -- each update propagates,
\* fair gossip satisfied, while the next is signed before the last has
\* reached everyone, so there need never be a point after which all views
\* agree forever.  A cross-family review put it exactly that way.
\* MaxEvents makes the conjunct true in every execution of this instance,
\* which is the point: what is checked is convergence UNDER quiescence,
\* and saying so is the difference between the result and its shadow.
Quiesces == <>[][events' = events]_vars

Convergence == (<>[](severed = {}) /\ Quiesces) => <>[]Agreed

=============================================================================
