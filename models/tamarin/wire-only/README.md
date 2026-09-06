# `tamarin/wire-only/`

## 1. What this model demonstrates

- A client that completes an attach authenticated the server it intended to reach (`server_authentication`).
- A serving node's bound session names a client that proved control of its key over that server's own challenge (`client_authentication`).
- Queued material is released only to the transport-authenticated peer, never to a party that merely named a keyhash in `Attach` (`queued_data_reaches_only_the_authenticated_peer`). Removing the binding check falsifies it.
- A client's commitment to an attach is injective (`client_commit_is_injective`).
- An accepted presence record was signed, over that exact body, by every party it names — or that party's key was stolen (`an_accepted_record_is_attributable`, `an_accepted_formation_is_attributable`). Removing a signature check falsifies it.
- An accepted recovery rests on a recognition naming *that* successor (`recognition_binds_the_successor`) and on an old-key proof naming *that* patron (`successor_statement_binds_the_patron`).
- A recognition with no key authorisation never recovers (`recognition_alone_insufficient`).
- A transfer's signed statement names *that* destination (`transfer_statement_binds_the_destination`). Removing the destination falsifies it.
- A currency attestation binds subject, key and epoch to the issuer's signature, and acceptance follows an unexpired issuance (`currency_requires_unexpired_issuance`).
- Each of the above holds against a legitimate principal signing whatever it likes with its own uncompromised key.

## 2. What it cannot demonstrate

- That participants named in a presence record were physically co-present. A legitimate keyholder may sign a body for a meeting that never happened.
- That a recovery meeting occurred, or that a verifier's recognition is truthful.
- That an issuer consulted the key it currently records, or that it was authorised for that subject by any rung of the escalation ladder. There is no patron-of relation and no issuer role.
- That any stated attestation lifetime elapsed. Expiry is event order, not duration, and acceptance-before-expiry is imposed by restriction rather than derived.
- That an `Attach` was not processed as TLS 1.3 0-RTT early data.
- Wire §4.1's node/patron key inequality. Identity is carried here as a stable name, not as a keyhash, so the two are not comparable.
- Anything about a node's own records: current-key state, session termination, or queue retention.
- Availability, ordering, or delivery. The adversary controls the network entirely.
