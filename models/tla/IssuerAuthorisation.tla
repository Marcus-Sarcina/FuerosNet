----------------------- MODULE IssuerAuthorisation -----------------------
(***************************************************************************)
(* Whether a relying party may accept a currency attestation on an issuer  *)
(* authorisation it has since withdrawn.                                   *)
(*                                                                         *)
(* WHY THIS IS IN TLA+.  `tamarin/wire-only/currency.spthy` records        *)
(* authorisation as a PERSISTENT fact, so what it proves is that the       *)
(* relying party recorded this issuer in this role at some earlier point.  *)
(* Making that record revocable turns it into a capability read many times *)
(* and withdrawn once, and Tamarin's backward search does not terminate on *)
(* that shape -- the same consume-and-restore regress diagnosed for        *)
(* currency issuance and session serving. Attempted; it ran past two       *)
(* minutes without a verdict. Lifecycles of node-local records live here.  *)
(*                                                                         *)
(* WHY IT MATTERS.  The issuer roles are TOPOLOGY-DERIVED -- patron,       *)
(* sibling, grandpatron, down-line threshold -- and design 12.6.5.1's      *)
(* escalation ladder moves among those relationships as the topology       *)
(* changes. "Sibling" is a claim about the present. A model in which       *)
(* authorisation only ever accumulates cannot ask whether acceptance on a  *)
(* lapsed relationship should still be possible.                           *)
(*                                                                         *)
(* WHAT MAKES THE INVARIANT NON-VACUOUS.  `Accept` does not test the role  *)
(* against the record and then assert it matched. It accepts whatever the  *)
(* attestation claims, subject only to the reading selected by `StaleOK`,  *)
(* and a flag records whether what it accepted was in fact the currently   *)
(* recorded role. The invariant is therefore a claim about the two         *)
(* readings, not a restatement of a guard.                                 *)
(***************************************************************************)

EXTENDS FiniteSets

CONSTANTS
  Parties,    \* relying parties.  Records are per-observer (design 16.1);
              \* there is no global authorisation and none is modelled.
  Issuers,
  Roles,      \* {patron, sibling, grandpatron, downline}
  StaleOK     \* TRUE  -- acceptance may rest on any role ever recorded
              \* FALSE -- acceptance must match the currently recorded role

NoRole == "none"

VARIABLES
  recorded,     \* recorded[R][I] : the role R records for I now, or NoRole
  everRecorded, \* everRecorded[R][I] : every role R has ever recorded for I
  badAccept     \* set if an acceptance ever rested on a role not currently recorded

vars == <<recorded, everRecorded, badAccept>>

Init ==
  /\ recorded     = [R \in Parties |-> [I \in Issuers |-> NoRole]]
  /\ everRecorded = [R \in Parties |-> [I \in Issuers |-> {}]]
  /\ badAccept    = FALSE

Record(R, I, role) ==
  /\ role \in Roles
  /\ recorded'     = [recorded     EXCEPT ![R][I] = role]
  /\ everRecorded' = [everRecorded EXCEPT ![R][I] = everRecorded[R][I] \cup {role}]
  /\ UNCHANGED badAccept

(***************************************************************************)
(* WITHDRAWAL.  The relationship ends -- a subordinate departs, a          *)
(* disavowal lands, the ladder moves on.  The record goes; the HISTORY     *)
(* does not, which is the whole point: `everRecorded` is what a            *)
(* permanently-persistent model would still be consulting.                 *)
(***************************************************************************)
Withdraw(R, I) ==
  /\ recorded[R][I] # NoRole
  /\ recorded' = [recorded EXCEPT ![R][I] = NoRole]
  /\ UNCHANGED <<everRecorded, badAccept>>

(***************************************************************************)
(* ACCEPTANCE.  The attestation claims a role; the relying party decides   *)
(* whether to honour it. `StaleOK` selects which reading is in force, and  *)
(* the flag notices when what was honoured is not what is recorded now.    *)
(***************************************************************************)
Accept(R, I, role) ==
  /\ role \in Roles
  /\ IF StaleOK THEN role \in everRecorded[R][I]
                ELSE role = recorded[R][I]
  /\ badAccept' = (badAccept \/ (role # recorded[R][I]))
  /\ UNCHANGED <<recorded, everRecorded>>

Next ==
  \/ \E R \in Parties, I \in Issuers, role \in Roles : Record(R, I, role)
  \/ \E R \in Parties, I \in Issuers               : Withdraw(R, I)
  \/ \E R \in Parties, I \in Issuers, role \in Roles : Accept(R, I, role)

Spec == Init /\ [][Next]_vars

TypeOK ==
  /\ recorded     \in [Parties -> [Issuers -> Roles \cup {NoRole}]]
  /\ everRecorded \in [Parties -> [Issuers -> SUBSET Roles]]
  /\ badAccept    \in BOOLEAN

\* No acceptance ever rested on a role the relying party does not currently
\* record -- withdrawn, or replaced by a different role on the ladder.
NeverAcceptedOnALapsedAuthorisation == ~badAccept

=============================================================================
