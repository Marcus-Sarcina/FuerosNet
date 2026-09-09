# `tamarin/compliant/`

## 1. What this model demonstrates

- A participant that runs the nomination check signs no record attributing to it a witness it did not nominate (`no_signature_over_a_misattributed_nominee`).
- No nominee is drawn from the nominator's own neighbourhood (`no_nominee_from_the_nominators_own_neighbourhood`).
- Among participants who keep the signing discipline, an accepted record implies the meeting happened (`a_record_between_conforming_participants_implies_a_meeting`), and one meeting's signatures bind one roster (`a_conforming_signature_binds_one_roster`). The carve-out names the participants the record names, so the claim holds for them beside any number of parties elsewhere who did not keep it.
- Among verifiers who keep it, an accepted recovery implies a meeting (`recovery_between_conforming_parties_requires_a_meeting`), and a recognition names the key that meeting exchanged (`a_conforming_recognition_names_the_meeting_key`). The carve-out names the verifier the patron relied on, not every verifier in the trace.
- Sealing a line before taking a reissue leaves nothing for a holder of the old key to supersede (`sealing_first_leaves_nothing_to_supersede`); reissuing first leaves a window (`reissuing_first_leaves_a_window`).
- A node serving as a failover sibling performs no trust-bearing operation (`no_trust_bearing_operation_on_a_sibling`).
- A server that defers TLS 1.3 0-RTT early data binds session state only after the handshake **of the connection that carried the early data** has completed (`a_conforming_server_binds_only_after_the_handshake`); a completed handshake on an earlier connection with the same client admits nothing. Letting any completed handshake serve falsifies it, which the gate runs.
- Queued material is delivered only under the credential it was queued for (`delivery_is_to_the_credential_it_was_queued_for`): the queue carries the recipient keyhash, as design §14.1.6 has it, so an item queued under a credential later superseded does not go out under the successor. Ignoring the queued credential at delivery falsifies it, which the gate runs; the supersession half is in the bounded companion.
- A key enters an issuer's record once, by one of two doors (`a_key_enters_a_record_once`, `a_record_key_entered_by_one_of_two_doors`).
- The non-compliant traces remain reachable: collusion produces a record with no meeting; a lying verifier completes a recovery; misattribution succeeds when the check is skipped; a thief who signs in a line **after** the subject sealed it is still taken by a counterparty holding no chain, the thief's own record and not some record; a sibling that has not received a rotation still issues for the old key; a server that acts on early data re-binds a replayed `Attach`, and binds off a connection whose handshake never completes.

## 2. What it cannot demonstrate

- That any node keeps these commitments. Every property is conditional on conformance, and no third party can check conformance.
- Anything cryptographic. Signatures are abstracted away.
- That issuance never follows supersession, or that nothing is served under a superseded credential. Both are checked in `tla/SupersessionDiscipline.tla`; here they hold only under a bound. The bound lives in a `.bounded` fragment that `run-all.sh` splices into the theory before its final `end`, so the rules have one source; the fragment is not a theory on its own.
- That physical co-presence occurred, or that a recognition is truthful.
- That a rotation reaches any particular issuer. Records are per-node and replication lag is modelled, not resolved.
- That the reference client is the only conforming one. These are obligations, not an implementation.
