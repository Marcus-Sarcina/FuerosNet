# `tamarin/wire-only/`

## 1. What this model demonstrates

- A client that completes an attach authenticated the server it intended to reach (`server_authentication`), having checked that the `KeyMaterial` it pinned hashes to that identity and that the key the transport presented is that pair's classical half. Removing either check falsifies it.
- A serving node's bound session names a client that proved control of its key **over that server's own fresh challenge** (`client_authentication`) — the challenge is carried in the action and the lemma, so removing it from the signed payload falsifies them. The server runs the same two identity checks in the other direction.
- Queued material is released only to the transport-authenticated peer, never to a party that merely named a keyhash in `Attach` (`queued_data_reaches_only_the_authenticated_peer`). Removing the binding check falsifies it.
- A client's commitment to an attach is injective (`client_commit_is_injective`).
- An accepted presence record was signed, over the body as modelled, by every party the model names — both participants and the witness — or that party's key was stolen (`an_accepted_record_is_attributable`, `an_accepted_formation_is_attributable`). Removing a signature check falsifies it.
- **Every** verifier response a patron accepted names *that* successor and was genuinely signed by the verifier it names (`recognition_binds_the_successor`, quantified over the responses actually consumed rather than over the trace) and an old-key proof naming *that* patron (`successor_statement_binds_the_patron`).
- A recognition with no key authorisation never passes that gate (`recognition_alone_insufficient`).
- A transfer's signed statement names *that* destination (`transfer_statement_binds_the_destination`). Removing the destination falsifies it.
- A currency attestation binds subject, key and epoch to the issuer's signature, and acceptance follows an unexpired issuance (`currency_requires_unexpired_issuance`).
- The **accepted** attestation — subject, key, both timestamps, role and issuer as one signed tuple — is the one that was issued (`an_attestation_binds_its_issuer_and_role`), so an attestation issued as a sibling cannot be re-presented as a patron's. Removing the role from the signed tuple falsifies it.
- Acceptance names an issuer the relying party **recorded** as authorised for that subject in that role (`acceptance_names_an_issuer_this_party_recorded_as_authorised`). Removing the check falsifies it.
- Identities are hybrid and the halves fail separately: every attribution carve-out requires **both** halves of a signer to be compromised, so an attacker holding only a party's classical key cannot put that party on an accepted presence record, forge a verifier's recovery `match`, or countersign a transfer. Dropping any one post-quantum check falsifies the corresponding lemma.
- A recovery installs a successor that differs from the identity being recovered from (`a_recovery_never_installs_the_prior_identity`), compared at keyhash level.
- Acceptance requires a signed `match`, not merely an authentic verifier response (`acceptance_requires_a_signed_match`) — `no-match`, `inconclusive` and `unavailable` are signed by the same key over the same fields.
- Key material is bound to the identity that claims it **before any key from it is used**: acceptance takes candidate `KeyMaterial` and checks it hashes to the claimed keyhash. Removing that check falsifies the corresponding attribution lemma in each of the four theories.
- Every signature carries its wire role as COSE `external_aad`, separately from the payload, using the wire's own names — `rhtn/1:envelope` for presence and formation alike, `rhtn/1:verifier`, `rhtn/1:successor`, `rhtn/1:transfer`, `rhtn/1:currency`.
- Normal and formation records carry the **signed subtype in the body**, not different contexts — both are type-5 envelopes, as on the wire, and a formation's witness key is absent rather than empty.
- No verifier a recovery relies on is the subject itself (`no_relied_on_verifier_is_the_subject`), stated over the reliance action so it covers every response rather than whichever slot a check was written into. Removing the second slot's exclusion falsifies it.
- Every verifier response in a recovery block is validated, not only the first: responses name the new node, come from distinct verifiers, and the block needs at least one signed `match` **in either position**. A one-response block is also represented, which the wire's nonempty array permits. Dropping the second response's checks falsifies `forging_a_recognition_needs_both_verifier_halves`.
- Forging a verifier's recognition needs **both** halves, in both directions: the classical-only and post-quantum-only cases are separate lemmas, so each of the two signature checks is guarded.
- `issued_at` and `expires_at` are distinct signed fields and **both are guarded**: leaving `expires_at` unsigned falsifies `currency_requires_unexpired_issuance`, and leaving `issued_at` unsigned falsifies `the_accepted_issue_time_is_the_signed_one`.
- Each of the above holds against a legitimate principal signing whatever it likes with its own uncompromised key. Every signing context in all four theories has a rule permitting it, and the authentication properties are stated over *this key signed this term*, not over a protocol transition having been taken.

## 2. What it cannot demonstrate

- That participants named in a presence record were physically co-present. A legitimate keyholder may sign a body for a meeting that never happened.
- That a recovery meeting occurred, or that a verifier's recognition is truthful.
- Recovery blocks larger than two responses. The wire permits 32; two is enough to exercise the per-collection rules (subject, distinct verifiers, at-least-one-match) but not any rule whose failure needs three or more.
- The COSE structure itself — protected headers, `alg`, `kid` grouping, canonical CBOR. The role context is modelled; its encoding is not.
- A recovery verifier response's `query_id`, its field 7 subject-consent signature, and its selection basis. Entering the theory assumes a `recognise` term is only ever built from a wire `VerifierResponse` whose consent and consistency checks already passed; no refinement argument establishes that.
- That a complete recovery *transaction* was accepted. These are properties of one patron's evidence gate; the successor's envelope signature, the Adoption body, the signer set and the remaining structural checks are outside the theory.
- Attribution over the full wire body. The signed term is a projection — the wire body also carries timestamps, verifier responses, witness metadata and disclosure commitments — so a signature-coverage error distinguishing two bodies that agree on the projection is invisible here. No refinement argument connects the two.
- Anything about a multi-witness signer set. The wire permits up to 16 witnesses and 32 verifier responses; this carries one witness, so a validator that checked witness 1 of 2 and skipped the rest has no representation.
- That an issuer consulted the key it currently records.
- That an issuer's authorisation still holds when the attestation is accepted. The record is persistent here, so what is shown is that the relying party recorded that role at some earlier point; the withdrawal lifecycle is checked in `tla/IssuerAuthorisation.tla`.
- That any stated attestation lifetime elapsed. Expiry is event order, not duration, and acceptance-before-expiry is imposed by restriction rather than derived.
- That domain separation prevents any concrete cross-context replay. Every signature carries its role, but the payload structures modelled here are projections; the wire's own structures already differ (a `SuccessorStatement` is an array of keyhashes, a verifier signature covers a map), so no byte-level collision is exhibited. The role tag is a requirement this suite **observes**, not one it demonstrates the necessity of.
- That the signed record subtype prevents a cross-subtype transplant. A formation's witness key is absent on the wire, which distinguishes the two bodies before the subtype is read; the subtype's value is unambiguous semantics, not transplant prevention.
- The transfer rule that a former patron must differ from the node being transferred. `Old != New` is checked; `Old != nodekey` sits in the same keyhash-versus-stable-name split that suppresses the adoption's node/patron inequality.
- Wire §4.1's node/patron key inequality. Identity is carried here as a stable name, not as a keyhash, so the two are not comparable.
- Anything about a node's own records: current-key state, session termination, or queue retention.
- Availability, ordering, or delivery. The adversary controls the network entirely.
