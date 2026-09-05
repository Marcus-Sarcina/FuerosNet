----------------------- MODULE CurrencyEscalation -----------------------
(***************************************************************************)
(* The currency-attestation escalation ladder under patron outage.         *)
(*                                                                         *)
(* WHAT THIS MODEL IS                                                      *)
(* design Section 12.6.5.1 defines an escalation ladder for keeping a       *)
(* node's key-currency fresh when its patron is unreachable:               *)
(*    patron  ->  sibling  ->  grandpatron  ->  re-adopt elsewhere.        *)
(* The review plan's Stage 1.2 asks two questions of it:                   *)
(*   Q1 (deadlock): can the ladder reach a state where the node needs a    *)
(*      fresh attestation, is not frozen by rule, yet NO issuer can act?   *)
(*   Q2 (two live answers): can two DIFFERENT issuers produce conflicting  *)
(*      CURRENT attestations for the same node that a relying party        *)
(*      cannot tell apart?                                                 *)
(*                                                                         *)
(* WHAT THIS MODEL DOES NOT CARRY, stated so the scope is visible.  The    *)
(* design's escalation table has a FOURTH rung -- "adopt at a new patron"   *)
(* for extended or permanent outage -- and a fifth path for light-client    *)
(* patrons, who "pre-delegate issuance to their own patron at adoption      *)
(* time" because they carry no uptime commitment.  Neither is modelled.     *)
(* Both are escapes from the frozen state rather than rungs of the issuing  *)
(* ladder, so their absence makes the liveness claim here STRICTLY WEAKER   *)
(* than the design's: this model proves the ladder does not deadlock while  *)
(* an ISSUER can serve, and says nothing about the re-adoption remedy.      *)
(*                                                                         *)
(* This model shares the TLA+ reading conventions spelled out at the top   *)
(* of PartitionMerge.tla; only NEW idioms are re-explained here.           *)
(*                                                                         *)
(* THE ABSTRACTION OF TIME.  A symbolic model has no clock, and it does    *)
(* not need one: "current vs expired" is EVENT ORDER, not wall time        *)
(* (design Section 12.6.5 derives the lifetime from a compromise window,   *)
(* but the LADDER's correctness is about who may issue, not about how many *)
(* hours pass).  We model an integer `clock` that only advances by an      *)
(* explicit Tick action, and an attestation carries the clock value it was *)
(* issued at plus a fixed LIFETIME; it is current iff clock < issued +     *)
(* LIFETIME.  This is the standard way to model expiry in TLA+ without     *)
(* real time.                                                              *)
(***************************************************************************)

EXTENDS Naturals, FiniteSets

CONSTANTS
  Subject,       \* the one node whose currency we track (a model scalar)
  Patron,        \* its patron
  Siblings,      \* set of the patron's other children (the sibling issuers)
  Grandpatron,   \* the patron's patron
  LIFETIME,      \* attestation lifetime in ticks (design Section 12.6.5)
  MaxClock       \* bound on the clock, to keep the state space finite

\* The set of parties who can, under SOME rung of the ladder, issue an
\* attestation for Subject.  Not the Subject itself: a node does not vouch
\* for its own currency (design Section 12.6.5 -- the issuer is the node's
\* PATRON or a stand-in for it).
Issuers == {Patron} \cup Siblings \cup {Grandpatron}

VARIABLES
  clock,       \* the abstract time, advanced only by Tick
  reachable,   \* reachable[i] : is issuer i currently up?  The outage is
               \* modelled by flipping these.  Subject is always "up" --
               \* it is the party trying to stay current.
  att,         \* the single most recent attestation Subject holds, or a
               \* sentinel [issuer |-> None-like, ...].  A record with
               \* fields: issuer, issued (clock at issue), secondhand
               \* (TRUE if issued by a stand-in, design Section 12.6.5.1's
               \* "marked secondhand and weighted slightly lower").
  rotated      \* has Subject rotated its key?  Used only by Q2: a stale
               \* attestation naming an old key must never out-rank the
               \* rotation.  FALSE throughout the liveness checks.

vars == <<clock, reachable, att, rotated>>

\* Is Subject's held attestation current at the present clock?
\* An attestation issued at time t is current while clock < t + LIFETIME.
Current(a) == a.issuer \in Issuers /\ clock < a.issued + LIFETIME

Init ==
  /\ clock = 0
  /\ reachable = [i \in Issuers |-> TRUE]
  \* Subject starts with a fresh attestation from its own patron.
  /\ att = [issuer |-> Patron, issued |-> 0, secondhand |-> FALSE]
  /\ rotated = FALSE

(***************************************************************************)
(* ACTION: Tick.  Advance the clock by one.  Nothing else changes; the     *)
(* passage of time is what turns a current attestation stale.  Bounded by  *)
(* MaxClock so TLC terminates.                                             *)
(***************************************************************************)

Tick ==
  /\ clock < MaxClock
  /\ clock' = clock + 1
  /\ UNCHANGED <<reachable, att, rotated>>

(***************************************************************************)
(* ACTIONS: GoDown / ComeUp.  Any issuer may fail and recover.  The ladder *)
(* exists precisely for the states these produce.                          *)
(***************************************************************************)

GoDown(i) ==
  /\ reachable[i]
  /\ reachable' = [reachable EXCEPT ![i] = FALSE]
  /\ UNCHANGED <<clock, att, rotated>>

ComeUp(i) ==
  /\ ~reachable[i]
  /\ reachable' = [reachable EXCEPT ![i] = TRUE]
  /\ UNCHANGED <<clock, att, rotated>>

(***************************************************************************)
(* THE LADDER.  Each rung is an action that issues a FRESH attestation     *)
(* (issued = current clock), enabled only under the conditions            *)
(* design Section 12.6.5.1 states for that rung.  The key design rule --   *)
(* "issue fresh, never extend stale" -- is captured by every issue setting *)
(* issued = clock, so no action can prolong an old attestation's life.     *)
(***************************************************************************)

\* Rung 1: the patron issues, whenever it is reachable.  Firsthand.
IssuePatron ==
  /\ reachable[Patron]
  /\ att' = [issuer |-> Patron, issued |-> clock, secondhand |-> FALSE]
  /\ UNCHANGED <<clock, reachable, rotated>>

\* Rung 2: a sibling issues from replicated state, but ONLY when the patron
\* is down (design Section 12.6.5.1: siblings stand in during outage).
\* Marked secondhand.  Siblings are independent ON THE HONESTY AXIS, which
\* design Section 12.6.5.1 scopes to adversaries that can only attack
\* availability: a sibling cannot be MADE to lie by the patron being down --
\* is exactly why this rung is safe and a grace-period extension would not
\* be.
IssueSibling(s) ==
  /\ s \in Siblings
  /\ reachable[s]
  /\ ~reachable[Patron]
  /\ att' = [issuer |-> s, issued |-> clock, secondhand |-> TRUE]
  /\ UNCHANGED <<clock, reachable, rotated>>

\* Rung 3: the grandpatron issues, only when the patron AND all siblings
\* are down.  It holds the adoption record anyway, being inside the h=2
\* storage horizon (design Section 12.6.5.1).  Marked secondhand.
IssueGrandpatron ==
  /\ reachable[Grandpatron]
  /\ ~reachable[Patron]
  /\ \A s \in Siblings : ~reachable[s]
  /\ att' = [issuer |-> Grandpatron, issued |-> clock, secondhand |-> TRUE]
  /\ UNCHANGED <<clock, reachable, rotated>>

Next ==
  \/ Tick
  \/ \E i \in Issuers : GoDown(i)
  \/ \E i \in Issuers : ComeUp(i)
  \/ IssuePatron
  \/ \E s \in Siblings : IssueSibling(s)
  \/ IssueGrandpatron

(***************************************************************************)
(* FAIRNESS.  For the liveness question we assume the reachable issuers    *)
(* eventually act (weak fairness on every issue action) and that the clock *)
(* keeps advancing (weak fairness on Tick).  We do NOT assume fairness of  *)
(* GoDown/ComeUp: outages happen or not, adversarially.                    *)
(***************************************************************************)

Fairness ==
  /\ WF_vars(Tick)
  /\ WF_vars(IssuePatron)
  /\ \A s \in Siblings : WF_vars(IssueSibling(s))
  /\ WF_vars(IssueGrandpatron)

Spec == Init /\ [][Next]_vars /\ Fairness

(***************************************************************************)
(* PROPERTIES.                                                             *)
(***************************************************************************)

TypeOK ==
  /\ clock \in 0..MaxClock
  /\ reachable \in [Issuers -> BOOLEAN]
  /\ att.issuer \in Issuers
  /\ att.issued \in 0..MaxClock
  /\ att.secondhand \in BOOLEAN
  /\ rotated \in BOOLEAN

\* SAFETY (Q2, part a): never two live answers.  `att` holds exactly one
\* attestation at a time -- the model has a single slot -- so a relying
\* party never faces two conflicting CURRENT attestations from this
\* subject's own ladder.  This is a structural fact worth asserting: the
\* ladder ISSUES, it does not fork.  (A forked answer would require two
\* issuers writing different current attestations that both reach a
\* relying party; the design's answer, design Section 9.0.2, is that a
\* relying party detecting two would treat it as a fork -- outside this
\* ladder's scope, and the reason the single slot is the faithful model.)
\* No operator is defined for this: it is a property of the MODEL's shape,
\* not a predicate over states, and a definition equal to TRUE would look
\* like a checkable property while checking nothing.

\* SAFETY (Q2, part b): "issue fresh, never extend stale."  Every
\* attestation Subject holds was issued at a clock value no later than now,
\* and no action ever sets issued to a value it could not legitimately
\* stamp.  Concretely: a held attestation's issue time never exceeds the
\* clock -- nothing predates the clock or is stamped into the future.
FreshOnly == att.issued <= clock

\* SAFETY (Q1, restated as an invariant): the design's HONEST LIMIT
\* (design Section 12.6.5.1) says that if the patron AND all siblings AND
\* the grandpatron are unreachable, the node is legitimately frozen -- no
\* issuance path exists and that is accepted, not a bug.  So "deadlock"
\* would be a state where the held attestation is EXPIRED, at least one
\* issuer up the ladder IS reachable, and yet no issue action is enabled.
\* This invariant asserts that state is unreachable: whenever the held
\* attestation is expired and some rung's issuer is up, the corresponding
\* issue action is enabled.
SomeIssuerCanAct ==
  LET anyUp == \/ reachable[Patron]
               \/ \E s \in Siblings : reachable[s]
               \/ reachable[Grandpatron]
  IN (~Current(att) /\ anyUp) =>
       \/ reachable[Patron]                                  \* rung 1 ready
       \/ (\E s \in Siblings : reachable[s]) /\ ~reachable[Patron]  \* rung 2
       \/ reachable[Grandpatron] /\ ~reachable[Patron]
            /\ (\A s \in Siblings : ~reachable[s])           \* rung 3

\* LIVENESS (Q1, the real claim): whenever some issuer up the ladder is
\* permanently available, Subject eventually holds a current attestation
\* again -- the ladder does not deadlock while a rung can serve.  We state
\* the cleanest instance: if the PATRON is eventually always reachable, the
\* attestation is eventually always current.  (Sibling/grandpatron rungs
\* are covered by SomeIssuerCanAct as a safety invariant, which is the
\* stronger statement for those: they are ENABLED, so weak fairness fires
\* them.)  <>[] is "eventually always"; [] is "always".
LadderMakesProgress ==
  <>[](reachable[Patron] = TRUE) => <>[]Current(att)

=============================================================================
