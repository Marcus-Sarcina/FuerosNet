# `tamarin/compliant/`

## 1. What this model demonstrates

- A participant that runs the nomination check signs no record attributing to it a witness it did not nominate (`no_signature_over_a_misattributed_nominee`).
- No nominee is drawn from the nominator's own neighbourhood (`no_nominee_from_the_nominators_own_neighbourhood`).
- Among participants who keep the signing discipline, an accepted record implies the meeting happened (`a_record_between_conforming_participants_implies_a_meeting`), and one meeting's signatures bind one roster (`a_conforming_signature_binds_one_roster`).
- Among verifiers who keep it, an accepted recovery implies a meeting (`recovery_between_conforming_parties_requires_a_meeting`), and a recognition names the key that meeting exchanged (`a_conforming_recognition_names_the_meeting_key`).
- Sealing a line before taking a reissue leaves nothing for a holder of the old key to supersede (`sealing_first_leaves_nothing_to_supersede`); reissuing first leaves a window (`reissuing_first_leaves_a_window`).
- A node serving as a failover sibling performs no trust-bearing operation (`no_trust_bearing_operation_on_a_sibling`).
- A key enters an issuer's record once, by one of two doors (`a_key_enters_a_record_once`, `a_record_key_entered_by_one_of_two_doors`).
- The non-compliant traces remain reachable: collusion produces a record with no meeting; a lying verifier completes a recovery; misattribution succeeds when the check is skipped; a thief beats a counterparty holding no chain; a sibling that has not received a rotation still issues for the old key.

## 2. What it cannot demonstrate

- That any node keeps these commitments. Every property is conditional on conformance, and no third party can check conformance.
- Anything cryptographic. Signatures are abstracted away.
- That issuance never follows supersession, or that nothing is served under a superseded credential. Both are checked in `tla/SupersessionDiscipline.tla`; here they hold only under a bound.
- That physical co-presence occurred, or that a recognition is truthful.
- That a rotation reaches any particular issuer. Records are per-node and replication lag is modelled, not resolved.
- That the reference client is the only conforming one. These are obligations, not an implementation.
