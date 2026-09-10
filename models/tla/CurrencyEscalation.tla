----------------------- MODULE CurrencyEscalation -----------------------
(***************************************************************************)
(* The currency-attestation escalation ladder under patron outage.         *)
(*                                                                         *)
(* WHAT THIS MODEL IS                                                      *)
(* design Section 12.6.5.1 defines an escalation ladder for keeping a       *)
(* node's key-currency fresh when its patron is unreachable:               *)
(*    patron  ->  sibling  ->  grandpatron  ->  re-adopt elsewhere.        *)
(* The review plan's Stage 1.2 asks whether the ladder can DEADLOCK: reach *)
(* a state where the node needs a fresh attestation, the honest limit      *)
(* does not apply, yet NO issuer can act.  This model answers that, and    *)
(* checks the section's rule "issue fresh; never extend stale" against     *)
(* the alternative the section names and rejects.                          *)
(*                                                                         *)
(* WHAT THIS MODEL DOES NOT CARRY, stated so the scope is visible.         *)
(*                                                                         *)
(* The plan's OTHER question -- can two different issuers produce          *)
(* conflicting CURRENT attestations that a relying party cannot tell       *)
(* apart -- is not asked here.  The subject holds ONE attestation, so two  *)
(* current answers are not a state this model has, and an earlier version  *)
(* that presented the single slot as answering the question was claiming  *)
(* what it had assumed.  The design's own answer is elsewhere (design      *)
(* Section 9.0.2 treats two current answers as a fork, to be made visible).*)
(*                                                                         *)
(* The design's escalation table has a FOURTH rung -- "adopt at a new      *)
(* patron" for extended or permanent outage -- and a fifth path for         *)
(* light-client patrons, who "pre-delegate issuance to their own patron at *)
(* adoption time" because they carry no uptime commitment.  Neither is     *)
(* modelled.  Both end the unattested state by means other than the        *)
(* issuing ladder, so the liveness claim here is STRICTLY WEAKER than      *)
(* the design's: the ladder does not deadlock while an ISSUER can serve,   *)
(* and nothing is said about the re-adoption remedy.                       *)
(*                                                                         *)
(* Key rotation is not modelled either, and that bounds what "fresh" can   *)
(* mean here: a fresh attestation from an issuer and an extension of that  *)
(* same issuer's earlier attestation are the same state.  What CAN be      *)
(* distinguished is whether the party the attestation names was in a       *)
(* position to sign it at that moment, and that is what FreshOnly checks.  *)
(*                                                                         *)
(* This model shares the TLA+ reading conventions spelled out at the top   *)
(* of PartitionMerge.tla; only NEW idioms are re-explained here.           *)
(*                                                                         *)
(* THE ABSTRACTION OF TIME.  A symbolic model has no clock, and it does    *)
(* not need one: "current vs expired" is EVENT ORDER, not wall time        *)
(* (design Section 12.6.5 derives the lifetime from a compromise window,   *)
(* but the LADDER's correctness is about who may issue, not about how many *)
(* hours pass).  We model the AGE of the held attestation: a counter that  *)
(* Tick advances and every issuance resets, with the attestation current   *)
(* iff age < LIFETIME.  The counter saturates at LIFETIME -- "expired" has *)
(* no degrees -- so the state space is finite AND cyclic: expiry and       *)
(* renewal can alternate forever, which is the behaviour the liveness      *)
(* question is about.                                                      *)
(*                                                                         *)
(* AN EARLIER VERSION HAD AN ABSOLUTE CLOCK BOUNDED AT MaxClock, and a      *)
(* cross-family review showed what that did: once the clock reached its   *)
(* bound Tick was disabled, an attestation stamped there could never       *)
(* expire, and the liveness property "eventually always current" came     *)
(* true because time had stopped rather than because the ladder worked.   *)
(* The same review showed the property was wrong for unbounded time too:  *)
(* weak fairness lets the patron issue only AFTER each expiry, so          *)
(* "eventually always current" is false, and what the ladder actually     *)
(* guarantees is RECOVERY -- every expiry is followed by renewal while a   *)
(* rung can serve.  That is the property stated now.                       *)
(***************************************************************************)

EXTENDS Naturals, FiniteSets

CONSTANTS
  Subject,       \* the one node whose currency we track (a model scalar)
  Patron,        \* its patron
  Siblings,      \* set of the patron's other children (the sibling issuers)
  Grandpatron,   \* the patron's patron
  LIFETIME,      \* attestation lifetime in ticks (design Section 12.6.5)
  GraceOK        \* FALSE: the rule.  TRUE only in the mutation: the held
                 \* attestation may be EXTENDED while the patron is down --
                 \* the grace period design Section 12.6.5.1 names and
                 \* rejects.

\* The set of parties who can, under SOME rung of the ladder, issue an
\* attestation for Subject.  Not the Subject itself: a node does not vouch
\* for its own currency (design Section 12.6.5 -- the issuer is the node's
\* PATRON or a stand-in for it).
Issuers == {Patron} \cup Siblings \cup {Grandpatron}

VARIABLES
  age,         \* ticks since the held attestation was issued, saturating
               \* at LIFETIME (= expired)
  reachable,   \* reachable[i] : is issuer i currently up?  The outage is
               \* modelled by flipping these.  Subject is always "up" --
               \* it is the party trying to stay current.
  att          \* the single attestation Subject holds.  A record with
               \* fields: issuer, secondhand (TRUE if issued by a stand-in,
               \* design Section 12.6.5.1's "marked secondhand and weighted
               \* slightly lower").

vars == <<age, reachable, att>>

\* Is Subject's held attestation current?
Current == age < LIFETIME

\* Is issuer i positioned to sign for Subject right now -- up, and on the
\* rung whose conditions design Section 12.6.5.1 states?  The three issue
\* actions below are exactly these conditions plus the write.
CanIssue(i) ==
  \/ i = Patron /\ reachable[Patron]
  \/ i \in Siblings /\ reachable[i] /\ ~reachable[Patron]
  \/ i = Grandpatron /\ reachable[Grandpatron] /\ ~reachable[Patron]
       /\ \A s \in Siblings : ~reachable[s]

Init ==
  /\ age = 0
  /\ reachable = [i \in Issuers |-> TRUE]
  \* Subject starts with a fresh attestation from its own patron.
  /\ att = [issuer |-> Patron, secondhand |-> FALSE]

(***************************************************************************)
(* ACTION: Tick.  The held attestation ages by one.  Nothing else changes; *)
(* the passage of time is what turns a current attestation stale.  Stops   *)
(* at LIFETIME: an expired attestation does not get more expired, and      *)
(* there is nothing further for time to do until something is issued.     *)
(***************************************************************************)

Tick ==
  /\ age < LIFETIME
  /\ age' = age + 1
  /\ UNCHANGED <<reachable, att>>

(***************************************************************************)
(* ACTIONS: GoDown / ComeUp.  Any issuer may fail and recover.  The ladder *)
(* exists precisely for the states these produce.                          *)
(***************************************************************************)

GoDown(i) ==
  /\ reachable[i]
  /\ reachable' = [reachable EXCEPT ![i] = FALSE]
  /\ UNCHANGED <<age, att>>

ComeUp(i) ==
  /\ ~reachable[i]
  /\ reachable' = [reachable EXCEPT ![i] = TRUE]
  /\ UNCHANGED <<age, att>>

(***************************************************************************)
(* THE LADDER.  Each rung is an action that issues a FRESH attestation     *)
(* (age reset to 0), enabled only under the conditions design Section      *)
(* 12.6.5.1 states for that rung -- CanIssue, rung by rung.               *)
(***************************************************************************)

\* Rung 1: the patron issues, whenever it is reachable.  Firsthand.
IssuePatron ==
  /\ CanIssue(Patron)
  /\ att' = [issuer |-> Patron, secondhand |-> FALSE]
  /\ age' = 0
  /\ UNCHANGED reachable

\* Rung 2: a sibling issues from replicated state, but ONLY when the patron
\* is down (design Section 12.6.5.1: siblings stand in during outage).
\* Marked secondhand.  Siblings are independent ON THE HONESTY AXIS, which
\* design Section 12.6.5.1 scopes to adversaries that can only attack
\* availability: a sibling cannot be MADE to lie by the patron being down --
\* which is exactly why this rung is safe and a grace-period extension
\* would not be.
IssueSibling(s) ==
  /\ s \in Siblings
  /\ CanIssue(s)
  /\ att' = [issuer |-> s, secondhand |-> TRUE]
  /\ age' = 0
  /\ UNCHANGED reachable

\* Rung 3: the grandpatron issues, only when the patron AND all siblings
\* are down.  It holds the adoption record anyway, being inside the h=2
\* storage horizon (design Section 12.6.5.1).  Marked secondhand.
IssueGrandpatron ==
  /\ CanIssue(Grandpatron)
  /\ att' = [issuer |-> Grandpatron, secondhand |-> TRUE]
  /\ age' = 0
  /\ UNCHANGED reachable

(***************************************************************************)
(* THE REJECTED ALTERNATIVE, present so the mutation can switch it on.     *)
(* design Section 12.6.5.1: "The tempting fix is a grace period -- extend  *)
(* the last attestation while the patron is verifiably down.  Do not."    *)
(* This is that: the held attestation is made young again with nobody     *)
(* signing anything.  Disabled unless GraceOK.                             *)
(***************************************************************************)

GracePeriod ==
  /\ GraceOK
  /\ ~reachable[Patron]
  /\ age' = 0
  /\ UNCHANGED <<reachable, att>>

Next ==
  \/ Tick
  \/ \E i \in Issuers : GoDown(i)
  \/ \E i \in Issuers : ComeUp(i)
  \/ IssuePatron
  \/ \E s \in Siblings : IssueSibling(s)
  \/ IssueGrandpatron
  \/ GracePeriod

(***************************************************************************)
(* FAIRNESS.  For the liveness question we assume the reachable issuers    *)
(* eventually act (weak fairness on every issue action) and that time      *)
(* keeps passing while there is time to pass (weak fairness on Tick).  We  *)
(* do NOT assume fairness of GoDown/ComeUp: outages happen or not,         *)
(* adversarially.                                                          *)
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
  /\ age \in 0..LIFETIME
  /\ reachable \in [Issuers -> BOOLEAN]
  /\ att.issuer \in Issuers
  /\ att.secondhand \in BOOLEAN

\* SAFETY: "issue fresh; never extend stale."  Stated over steps: whenever
\* the held attestation gets younger, the issuer it names was positioned to
\* sign it in that step -- up, and on a rung whose conditions held.  A
\* renewal nobody could have signed is an extension, and the grace period
\* is exactly that: the patron is down, so the attestation it issued is
\* prolonged by no signature at all.  Under GraceOK this fails on the first
\* extension of a patron-issued attestation; under the rule every reset is
\* an issue action, whose guard is CanIssue.
\*
\* What this does NOT distinguish, said in the header: with no key rotation
\* in the model, a live issuer re-signing and that same issuer's old
\* attestation being stretched are one state.  An earlier version checked
\* "issued <= clock", which every state satisfied for free, and the README
\* credited it with the rule.
FreshOnly ==
  [][age' < age => CanIssue(att'.issuer)]_vars

\* SAFETY (the deadlock question, restated as an invariant): the design's
\* HONEST LIMIT (design Section 12.6.5.1) says that if the patron AND all
\* siblings AND the grandpatron are unreachable, the node legitimately goes
\* unattested -- no issuance path exists and that is accepted, not a bug,
\* and nothing the node does waits on it (design Section 12.6.5).  So
\* "deadlock" would be a state where the held attestation is EXPIRED, at
\* least one issuer up the ladder IS reachable, and yet no issue action is
\* enabled.  This invariant asserts that state is unreachable: whenever the
\* held attestation is expired and some rung's issuer is up, some issuer
\* can act.
SomeIssuerCanAct ==
  LET anyUp == \/ reachable[Patron]
               \/ \E s \in Siblings : reachable[s]
               \/ reachable[Grandpatron]
  IN (~Current /\ anyUp) => \E i \in Issuers : CanIssue(i)

\* LIVENESS (the deadlock question, the real claim): while a rung can
\* serve, the ladder RECOVERS -- every expiry is followed by renewal.  One
\* property per rung, each conditioned on that rung being the operative one
\* from some point on; together they say the ladder makes progress from
\* whichever rung outage leaves it.  `~Current => <>Current` under `[]` is
\* "leads to": whenever expired, eventually current again.  Per sibling
\* rather than "some sibling", because weak fairness on an action needs it
\* continuously enabled, and two siblings flapping in alternation keep
\* either from being so -- which is a real behaviour, not a modelling
\* artefact, and the property does not claim it.
LadderMakesProgress ==
  <>[](reachable[Patron]) => [](~Current => <>Current)

SiblingRungServes ==
  \A s \in Siblings :
    <>[](~reachable[Patron] /\ reachable[s]) => [](~Current => <>Current)

GrandpatronRungServes ==
  <>[](~reachable[Patron] /\ reachable[Grandpatron]
       /\ \A s \in Siblings : ~reachable[s])
    => [](~Current => <>Current)

=============================================================================
