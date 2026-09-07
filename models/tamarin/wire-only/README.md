# `tamarin/wire-only/`

## 1. What this model demonstrates

- A client that completes an attach authenticated the server it intended to reach (`server_authentication`), having checked that the `KeyMaterial` it pinned hashes to that identity and that the key the transport presented is that pair's classical half. Removing either check falsifies it.
- A serving node's bound session names a client that proved control of its key **over that server's own fresh challenge** (`client_authentication`) — the challenge is carried in the action and the lemma, so removing it from the signed payload falsifies them. The server runs the same two identity checks in the other direction.
- Queued material is released only to the transport-authenticated peer, never to a party that merely named a keyhash in `Attach` (`queued_data_reaches_only_the_authenticated_peer`). Removing the binding check falsifies it.
- A client's commitment to an attach is injective (`client_commit_is_injective`).
- An accepted presence record was signed, over the body as modelled, by every party the model names — both participants and the witness — or that party's key was stolen (`an_accepted_record_is_attributable`, `an_accepted_formation_is_attributable`). Removing a signature check falsifies it.
- A patron that accepts recovery evidence did so on a recognition naming *that* successor (`recognition_binds_the_successor`) and an old-key proof naming *that* patron (`successor_statement_binds_the_patron`).
- A recognition with no key authorisation never passes that gate (`recognition_alone_insufficient`).
- A transfer's signed statement names *that* destination (`transfer_statement_binds_the_destination`). Removing the destination falsifies it.
- A currency attestation binds subject, key and epoch to the issuer's signature, and acceptance follows an unexpired issuance (`currency_requires_unexpired_issuance`).
- An attestation binds its issuer identity and claimed role, so one issued as a sibling cannot be re-presented as a patron's (`an_attestation_binds_its_issuer_and_role`). Removing the role from the signed tuple falsifies it.
- Acceptance names an issuer the relying party **recorded** as authorised for that subject in that role (`acceptance_names_an_issuer_this_party_recorded_as_authorised`). Removing the check falsifies it.
- Identities are hybrid, and the halves fail separately. An attacker holding only a party's classical key cannot put that party on an accepted presence record (`classical_compromise_alone_does_not_forge`) nor forge a verifier's recovery `match` (`forging_a_recognition_needs_both_verifier_halves`). Recovery's field 7 is modelled classical, as the specification deliberately leaves it.
- Each of the above holds against a legitimate principal signing whatever it likes with its own uncompromised key. Every signing context in all four theories has a rule permitting it, and the authentication properties are stated over *this key signed this term*, not over a protocol transition having been taken.

## 2. What it cannot demonstrate

- That participants named in a presence record were physically co-present. A legitimate keyholder may sign a body for a meeting that never happened.
- That a recovery meeting occurred, or that a verifier's recognition is truthful.
- That a complete recovery *transaction* was accepted. These are properties of one patron's evidence gate; the successor's envelope signature, the Adoption body, the signer set and the remaining structural checks are outside the theory.
- Attribution over the full wire body. The signed term is a projection — the wire body also carries timestamps, verifier responses, witness metadata and disclosure commitments — so a signature-coverage error distinguishing two bodies that agree on the projection is invisible here. No refinement argument connects the two.
- Anything about a multi-witness signer set. The wire permits up to 16 witnesses and 32 verifier responses; this carries one witness, so a validator that checked witness 1 of 2 and skipped the rest has no representation.
- That an issuer consulted the key it currently records.
- That an issuer's authorisation still holds when the attestation is accepted. The record is persistent here, so what is shown is that the relying party recorded that role at some earlier point; the withdrawal lifecycle is checked in `tla/IssuerAuthorisation.tla`.
- That any stated attestation lifetime elapsed. Expiry is event order, not duration, and acceptance-before-expiry is imposed by restriction rather than derived.
- Wire §4.1's node/patron key inequality. Identity is carried here as a stable name, not as a keyhash, so the two are not comparable.
- Anything about a node's own records: current-key state, session termination, or queue retention.
- Availability, ordering, or delivery. The adversary controls the network entirely.
