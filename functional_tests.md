# Functional requirements and test specification

Prepared 2026-09-14 against implementation commit `75943dabf9a40fab489662aa9b2810c7363e0883` and the working-tree design documents fingerprinted at the end of this file.

This document inventories the software in `crates/` and specifies functional tests for **implemented and planned** behavior. It is a test specification, not a test execution report or a declaration that existing code conforms. Component status describes source structure only. No production tests were run to prepare this document. The previous conformance review is not the source of requirements.

The authoritative input is the five root design/requirements documents below. `CLAUDE.md`, `Robot/`, and `change-log.md` are excluded. Implementation comments, existing tests, and README files help locate components; they do not override the design. Planned mobile applications, full payload ratcheting, packaging, backup, and deployment integrations remain in scope here even where they were excluded from earlier conformance reviews.

Navigation: [component inventory](#2-implementation-structure-and-major-components) · [component test families](#3-component-requirements-by-transaction-and-internal-operation) · [wire and dispatch ledger](#4-wire-schema-and-dispatch-coverage-ledger) · [integration scenarios](#5-cross-component-transaction-and-lifecycle-scenarios) · [retention](#7-retention-and-authority-checklist-by-state-class) · [source coverage](#8-source-coverage-and-planned-scope) · [open decisions](#9-specification-conflicts-and-decisions-needed-for-complete-test-oracles).

## 1. Authority, test interpretation, and shared fixtures

| Abbreviation | Source | Responsibility |
|---|---|---|
| D | [network-design.md](network-design.md) | Role invariants, protocol behavior, reference policy, privacy, lifecycle, scope and open decisions. |
| W | [wire-format.md](wire-format.md) | Concrete encoding, schemas, signature domains, message dispatch and structural rejection rules. |
| L | [light-client-requirements.md](light-client-requirements.md) | Participant kernel, device integration, local storage and user-facing obligations. |
| I | [infra-client-requirements.md](infra-client-requirements.md) | Infrastructure serving, persistence, hosting and operator behavior. |
| R | [resource-requirements.md](resource-requirements.md) | Resource boundary, credentials, HTTP, applications and packages. |

References such as `W §4.1` point to numbered sections in these files. D Appendix A distinguishes shared-evidence invariants from local commitments. A more specific encoding does not silently repeal a design invariant; disagreements are recorded in §9 below. This document does not create protocol requirements to fill gaps.

Each numbered row is a test family, often with several required cases. Expand every listed variation into executable or manual cases. A passing happy path alone does not satisfy the row. Record requirement ID, exact input, policy/profile parameters, observed output, state before/after, and retained artifacts. Use these result states: **pass, fail, not implemented, blocked by specification, not exercised**. Do not report a blocked or unimplemented test as passing.

| Kind | Meaning and appropriate evidence |
|---|---|
| S | Shared-evidence/protocol requirement. Test received bytes, authenticated identities, signatures and state transitions. |
| C | Reference client, infrastructure, or resource commitment. Test local execution and retained state; a remote peer cannot necessarily enforce it. |
| P | Reference policy or configurable behavior. Fix and disclose the policy under test; do not treat another permitted policy as a malformed protocol. |
| E | Engineering test derived from a specified property, or implementation integration contract. Useful verification, not a new wire rule. |
| O | Requirement whose complete oracle needs an open decision. Test settled parts now and keep the unresolved part explicitly blocked. |

Shared fixtures must include: independently generated hybrid identities; two trees with infrastructure and two light levels; siblings, grandparents, cousins and external peers; a depth-24 path; genesis and established participants; multiple patron bindings; forked/merged archives; current and abandoned series; a holder missing keys or predecessors; deterministic wall and monotonic clocks; available/unavailable witnesses and verifiers; offline and superseded recipients; an owner distinct from its host; hostile HTTP clients and packages; simulated loss, reordering, replay, NAT and restart. Secrets and captures in fixtures must be synthetic. Cryptographic expected bytes must not be computed solely by the implementation being tested.

For **every wire object listed in §4**, apply the common schema suite: remove each required field; exercise each optional field absent and present; substitute each wrong CBOR major type; test every numeric and length boundary and one value outside it; exercise every enum member and an unknown member; duplicate, reorder and misassociate entries; truncate at every framing boundary; append trailing data; corrupt signed bytes; and verify the specified failure scope. Apply semantic cross-field rules separately from syntax. For each rejected input, verify no unauthorized durable state change, forwarding, credential, key release or success acknowledgement. Do not make all failures close a connection: stream, frame and session failure scopes differ.

Boundary tests use minimum, minimum−1 when representable, maximum, maximum+1, empty, singleton and full-size forms. Byte bounds measure encoded bytes where specified, not characters or decoded content. Optional empty/default omission and required empty values are different cases. Test simultaneous operations and crash points whenever acceptance spends a one-time value, acknowledges durable custody, changes an archive head, or changes authority. Local numeric choices are test parameters unless a source fixes them.

## 2. Implementation structure and major components

The Cargo workspace has **15 crates**. Components below follow responsibilities rather than assuming one crate per protocol role. Infrastructure operators also participate through the ordinary participant kernel; they are not a separate identity species.

| Component | Implementation locations under `crates/` | Present structure and planned work | Test families |
|---|---|---|---|
| Encoding and signed-object boundary | `codec/src/{cbor,encode,bounds,frame,cose,schema,envelope}.rs` | Implemented deterministic codec, schemas, COSE and framing. | ENC, SIG, SCH |
| Identity and cryptographic primitives | `crypto/src/{identity,verify,pqxdh}.rs` | Hybrid identity, verification and PQXDH support present; integration profiles still require the open decisions noted below. | SIG, PAY |
| Transactions and identity archive | `archive/src/{tx,record,chain,walk,series,topology,endpoint,locator,currency,prekey,catalog,submission}.rs` | Typed transactions, verification, archive traversal and state objects present. | TX, ARC, TOP, CUR |
| Transport and connectivity | `transport/src/{tls,session,queue,stun,traversal}.rs` | QUIC/TLS, session framing, queue support and traversal present. | NET, SES, RES, MAIL |
| Infrastructure node | `node/src/` | Store, materialized view, propagation, resolution, currency, peering, prekeys, wake, queue, submissions, catalog, resources, HTTP and trust runtime present. | TOP, RES, CUR, MAIL, CAT, GAT, OPS |
| Participant kernel | `client/src/` | Ceremony, selection, verifier, capture keys, archive, recovery, payload, Double Ratchet, horizon, catalog, trust and notices present. Full Triple Ratchet/SPQR and complete product deployment remain planned. | CER, VER, CAP, REC, PAY, UX |
| Network adaptors | `adaptors/src/{serving,attached,actor,verifier,courier,direct}.rs` | Real serving/attached and actor-to-network paths, verifier courier and direct path integrations present. | SES, VER, MAIL, PAY, INT |
| Observer-relative policy | `policy/src/{evidence,flow,landscape,archive,series,conformance,policy}.rs` | Typed evidence, flow metric, landscape, archive and policy evaluation present. | POL |
| Resource gateway and package runtime | `node/src/{catalog,resources,http}.rs`, `resources/src/lib.rs` | Gateway/catalog/roles present. **A Wasmtime component sandbox is implemented**, including admission, request/response bindings and bounded execution. Full distribution, provenance/update conventions and external adapters remain implementation work. | CAT, GAT, PKG |
| Continuous operator process | `daemon/src/{config,service,hosting,operator,main}.rs` | Daemon startup/configuration, persistence and hosting integration present. | OPS |
| Application boundary | `ffi/src/{client,device,net,types}.rs` | Plain Rust facade with typed events and device bridges present. Foreign binding generator is not yet adopted. | APP |
| Android and iOS applications | `mobile/android/`, `mobile/ios/` | Planned platform shells; directories currently contain README descriptions, not shipping applications. Camera/radio/OS wake, backup and consent/warning UX need platform implementation and device validation. | APP, UX, CAP |
| Inspection and key tools | `cli/src/{inspect,keys,probe,main}.rs` | CLI inspection, identity tooling and network probes present. | TOOL |
| Runnable participant instrument | `participant/src/{carry,terminal,main,lib}.rs` | `rhtnp` drives the FFI participant across real sockets. Explicitly an experiment instrument with ephemeral state beyond its supplied identity, not a persistent user product. | TOOL, INT |
| Simulation and acceptance support | `sim/src/{path,nat,mesh,scenario,daemons,participants,packages}.rs`, `acceptance/` | Network/process/package simulation and acceptance support present. Existing acceptance catalogue is not treated as an exhaustive requirements source. | INT, VAL |

## 3. Component requirements by transaction and internal operation

## 3.1 Encoding, identities and signature verification

### Deterministic encoding and bounded decoding

| ID | Kind | Requirement and functional test oracle | Authority |
|---|---|---|---|
| ENC-001 | S | Validate canonicality on the bytes actually received. Reject non-shortest integers/lengths, indefinite items, duplicated map keys and noncanonical ordering before a lossy map decoder can erase evidence; a canonical equivalent of a rejected encoding does not make it valid. | W §§1, 1.2, 3.6 |
| ENC-002 | S | Protocol maps use unsigned integer keys in ascending order. Accept the standardized negative COSE key labels where required; do not apply the protocol-map restriction to standardized COSE structures or opaque extension values. | W §§1.2, 2.2 |
| ENC-003 | S | Omit optional empty arrays/maps and explicitly stated defaults; reject their prohibited explicit encodings. Accept required empty maps/arrays and empty top-level request objects where their schema permits them. | W §§1.2, 8.1 |
| ENC-004 | S | Preserve unknown fields in signed maps, including their canonical encoded value slices, through parse, storage, verification and forwarding. Accept all allowed deterministic CBOR value types; do not interpret unknown nested values as protocol maps. | W §§1, 1.2, 10.1 |
| ENC-005 | S | Accept at most 16 unknown keys per extensible map and at most 1024 encoded bytes per unknown value; reject excess even when the decoded payload looks small. Known fields do not consume the extension count. | W §1.3 |
| ENC-006 | S | Reject unknown map keys in unsigned protocol messages and reject retired identifiers where expressly reserved; forward compatibility of signed objects does not authorize unsigned extensions or retired fields. | W §§1.2, 4, 4.5, 6.6 |
| ENC-007 | S | Require version 1 wherever a version is carried. Validate the complete envelope, including signatures and extensions, rather than only its body. | W §3.6 |
| ENC-008 | S | Enforce each array and object bound in the schema ledger. Derive envelope signer capacity from the transaction's roles, then double it for hybrid signature entries; accept the legal 18-person/36-entry presence maximum when other bounds permit it. | W §§1.3, 3.5 |
| ENC-009 | S | Decode a path as exactly ceil(nibbles/2) bytes with high nibble first; only digits 0–9, maximum 24 digits, and zero unused low nibble. Accept empty self-anchor paths and reject all padding/length mismatches. | W §2.1 |
| ENC-010 | S | Never truncate a signed locator's full path. Permit prefix aggregation only for unsigned routing hints where specified; test odd and even path lengths through every encoder. | W §§2.1, 2.3; D §12.6 |
| ENC-011 | S | NetworkPoint uses exactly four IPv4 bytes, optional u32 ASN, and UDP port 1–65535; omitted port means 7431 and explicit default is omitted canonically. Reject IPv6-sized addresses in this version. | W §§2, 9.2 |
| ENC-012 | S | Keep timestamps and advisory queue counts in the full u64 range, series and locator counters in u32, and heartbeat counters in u64. Reject overflow instead of narrowing, wrapping, or treating large unsigned values as negative. | W §§2, 2.3, 3.3, 8.2 |
| ENC-013 | E | Bound allocation and parsing work before trusting declared lengths. Deep or oversized hostile inputs must return the specified error without a panic, unbounded allocation or corrupting a subsequent valid message. No new on-wire nesting limit is inferred here. | W §§1.3, 8.0, 9.2 |

### Content addressing, hybrid identity and authentication

| ID | Kind | Requirement and functional test oracle | Authority |
|---|---|---|---|
| SIG-001 | S | Compute transaction IDs as SHA-256 of the canonical body map excluding the envelope signatures. Changing body extensions changes the ID; changing signature-array representation does not change the body ID but can invalidate the envelope. | W §§1.4, 3 |
| SIG-002 | S | Compute keyhash from the canonical two-key KeyMaterial encoding; compute genesis from the raw keyhash bytes. Keep key, transaction and query preimage languages disjoint; test against independent fixed vectors. | W §§1.1, 2.2, 3.1 |
| SIG-003 | S | KeyMaterial is exactly classical then PQ COSE_Key. Enforce Ed25519 labels/curve/32-byte public key and ML-DSA-65 labels/algorithm/public-key size; reject swapped, extra, private, kid-bearing and mismatched material. | W §2.2 |
| SIG-004 | S | Use actual untagged detached COSE_Sign/COSE_Sign1 with nil payload and the specified Sig_structure, not a bespoke signature over concatenated application bytes. Reject tagged/attached alternatives and invalid protected/unprotected headers. | W §§1, 3.5 |
| SIG-005 | S | Reconstruct external AAD from the receiving context. Exercise all 14 domains: envelope, verifier, consent, currency, catalog, abuse, locator, anchor, endpoints, prekey, subtree-ack, successor, transfer and delegation, each prefixed rhtn/1:. A signature valid in one domain must fail in another. | W §1.1 |
| SIG-006 | S | For each logical envelope signer require exactly one Ed25519 entry (algorithm −8) and one ML-DSA-65 entry (−49); both verify. Remove/corrupt either half, add an extra signer, duplicate a half or relabel an algorithm: reject. | W §3.5; D §5.1 |
| SIG-007 | S | Envelope entries carry the protected 32-byte kid and algorithm, empty unprotected maps, and the specified empty outer protected map. Sort by raw kid bytes then classical before PQ; do not introduce signer-role fields. | W §3.5 |
| SIG-008 | S | Determine the exact signer set from transaction roles, including presence witnesses but excluding embedded verifiers. Check keyhash-to-key binding before accepting any signature; role aliases must not silently reduce a required distinct-party set. | W §§3.2, 3.5, 4 |
| SIG-009 | S | Ordinary embedded responses, consents and standalone signed current-state objects use their specified classical Sign1 form with kid omitted when another field supplies the identity. Do not require an invented PQ half or accept an incorrect envelope-shaped substitute. | W §§1, 5.6, 6–7; D §5.1 |
| SIG-010 | S | Old-key successor proofs, transfer consent and recovery verifier responses use both classical and PQ components. Exercise valid ordinary classical responses separately from recovery responses requiring hybrid authentication. | W §§4.1, 4.5; D §§5.1, 9.0.1 |
| SIG-011 | S | Produce deterministic ML-DSA signatures with the selected profile. Verify independent Ed25519/ML-DSA/COSE vectors and reject altered messages, keys and signature lengths. | W §§1, 2.2, 3.5; D §§5, 5.2 |
| SIG-012 | S | Missing key material yields Unverifiable with the missing identity, not structurally Invalid or verified. A known key with a bad signature is invalid; later supplying the proper key can resolve the former without changing stored bytes. | W §3.4 |
| SIG-013 | C | Keep structural validity, signature availability, current effectiveness, archive completeness, disclosure and observer trust as separate results. A well-formed but stale, incomplete, thin or untrusted object must not be mislabeled malformed. | W §3.4; I §5; D Appendix A |

### Adoption, relationship termination, peering and series reissue

| ID | Kind | Requirement and functional test oracle | Authority |
|---|---|---|---|
| TX-001 | S | Adoption is signed by distinct node and patron, starts the relationship counter at zero, and has exactly one of PoP reference, Recovery or Transfer authorization. Test none, every pair, all three and each sole valid choice. | W §4.1; D §6.1 |
| TX-002 | S | Optional new-node KeyMaterial must hash to the adopted node; adoption's archive-head field identifies one offered head rather than an arbitrary subset substituted for complete history presentation. | W §4.1; D §10.1 |
| TX-003 | C | Before ordinary adoption, both people authorize the live interaction and the prospective patron evaluates the offered archive to its chosen lookback. Unknown/missing history is exposed for a policy decision, not silently assumed complete. | D §§6.1.1, 10.1; I §5; D Appendix A.3 |
| TX-004 | S | The structural validator checks PoP reference shape without dereferencing it. An evaluator with the referred record verifies that it is valid and names both adopting parties; a fully withheld presentation remains usable for this purpose. | W §§3.4, 4.1, 4.5.2 |
| TX-005 | S | Transfer consent binds node, former patron and new patron under the transfer domain and both former-patron signatures. Reject a substituted destination, node, signer, or reuse for another tuple; enforce required party distinctness. | W §4.1; D §6.2.3 |
| TX-006 | C | Treat whether the signing former patron actually held the relationship as a separate state/evidence check. Where transfer authorizes a nearby move, do not require a fresh presence ceremony solely because the move is represented as adoption. | D §§6.1.1, 6.2.3; W §4.1 |
| TX-007 | S | Departure is a unilateral node-signed transaction naming the patron and advancing the binding's sequence; require both halves of that node's signature, never a patron signature. | W §4.2; D §6.2.1 |
| TX-008 | C | After effective departure retain the departing node's downline and make it a root, remove old routing authority, and let stale paths fail. Patron refusal or unavailability cannot veto the signed departure. | D §§6.2.1, 12.6.2 |
| TX-009 | S | Disavowal is signed only by the patron and names the terminated subordinate relationship; it does not advance or require the subject's private counter/signature. Apply its specified timestamp ordering to the relevant slot/state. | W §4.3; D §6.2.2 |
| TX-010 | S | Accept reason codes 0–63, including unspecified values inside the without-prejudice (0–31) and with-prejudice (32–63) bands; reject outside-band values. Preserve codes without inventing allegations beyond their meaning. | W §§4.2–4.3 |
| TX-011 | C | Disavowal ends authority without a notice period but does not erase the subject, its downline or signed history. A stale disavowal must not terminate a later valid readoption merely because it names the same identity. | D §6.2.2; W §4.3 |
| TX-012 | C | Exercise lateral/vertical transfer with departure delivered before adoption, after adoption, and not yet delivered. Neither unilateral departure nor destination adoption is silently made conditional on receiving the other. | D §6.2.3; W §4.2.1 |
| TX-013 | S | Peering has two distinct infrastructure endpoints, both hybrid signatures, a shared PoP reference and exactly one NetworkPoint per endpoint. Validate the optional backup commitment and bounded audit history without treating peering as an adoption. | W §4.4; D §6.3 |
| TX-014 | C | Establish peering across trees without changing patron slots, horizon membership or resource grants. Zero peers is supported; recommending at least two fault-diverse peers is not a structural admission requirement. | D §§6.3, 12.7.5; I §1 |
| TX-015 | S | A series reissue is signed by node and patron, binds old sequence and new sequence with counter zero, and chooses a never-used series for that relationship. Reject same/reused series, nonzero initial counter, wrong party or missing old lineage. | W §4.6 |
| TX-016 | C | Series reissue preserves identity and position while sealing the prior sequence line. Exercise reissue after suspected compromise and after introducing another binding; other relationships are not accidentally advanced. | D §9.2; W §§2.3, 4.6 |
| TX-017 | S | Series values are opaque labels, not increasing versions. Compare counters only within the proven same series, accept higher counters without requiring +1, and do not choose the numerically largest series. | W §§2.3, 4.6.1 |
| TX-018 | C | A root cannot forge a type-7 node-equals-patron reissue. Its internal archive rollup uses a real predecessor, not a new genesis assertion; its unilateral locator update does not become an archive transaction. | W §§2.3, 4.6; D §§9.2, 10 |

### Presence-record creation and structural validation

| ID | Kind | Requirement and functional test oracle | Authority |
|---|---|---|---|
| TX-019 | S | A presence record has exactly two distinct ordered participants. Witnesses are unique, ordered, outside that pair and each nominates one of the participants; reject duplicate, overlapping and falsely assigned roles. | W §§3.2, 4.5 |
| TX-020 | S | A normal presence record has at least one witness whose observation bits include both parties. Other witness entries may be partial. Retain reserved observation bits without assigning them invented meanings. | W §§3.2, 4.5 |
| TX-021 | S | A formation record has both participants' genesis predecessors and no witness or verifier-response arrays. Keep its subtype permanently distinguishable; do not later upgrade it because more evidence appears. | W §3.2; D §§13.1–13.2 |
| TX-022 | S | Every participant and witness signs the complete immutable presence body, while embedded verifier signatures remain evidence rather than envelope signer roles. Removing a body response invalidates the collected envelope signatures. | W §§3.5, 4.5; D §8.2 |
| TX-023 | S | Require finalized_at ≥ started_at and a difference no greater than 86,400 seconds; exercise zero and exact-24-hour records. Structural acceptance does not compare either timestamp with the receiver's clock. | W §§3.2–3.3 |
| TX-024 | S | Ordinary verifier responses name a participant as subject and a different identity as verifier. Allow one verifier to answer for both subjects, but reject duplicate (verifier, subject) responses and enforce verifier-then-subject ordering. | W §§3.2, 4.5 |
| TX-025 | S | Match, no-match and inconclusive responses require a comparison basis; unavailable omits it. Photo/both bases require template version, personal-only omits it, and unavailable omits it. prior_key is confined to recovery. | W §4.5 |
| TX-026 | S | Validate ClientIntegrity's declared attestation fields, CaptureSummary's 3–5 image count/modality/liveness/version, proximity channel types/outcomes and optional binding lengths. Do not infer that a syntactically present attestation proves a device trustworthy. | W §4.5; D §§7.5, 7.8 |
| TX-027 | S | Location assertions have supported geohash lengths 3 or 4 and canonical alphabet; method is extensible. Corroborations name actual witnesses and describe witness-relative radius, not precise coordinates. Enforce record-level location/corroboration maxima. | W §4.5; D §§7.6.2, 7.7 |
| TX-028 | S | When proximity is disclosed, strongest must identify a passing channel and no stronger disclosed channel may have passed: UWB, NFC, optical, then latency. Repeated channel entries are allowed; do not reject repetition solely as duplicate type. | W §§3.2, 4.5; D §7.6.3 |
| TX-029 | S | Apply the presence type's retired-field rejection independently of its unknown-extension allowance, including body key 7 and retired witness keys 4/5. | W §§1.2, 4.5 |
| TX-030 | C | A participant mints its own departure: one signature, no countersignature sought, no refusal that can arrive, and the series naming the relationship it ends. Nothing ranks a departure by its counter; a repeat is caught by its transaction identifier. Refuse where the archive shows no current relationship with that patron, including one already left. | W §§4.2, 2.3; D §6.2 |

### Archive linking, presentation, fetch and retention

| ID | Kind | Requirement and functional test oracle | Authority |
|---|---|---|---|
| ARC-001 | S | Each transaction has exactly one predecessor list per envelope signer in schema role order: adoption node/patron, departure node, disavowal patron, peering A/B, presence ordered participants then witnesses, reissue node/patron. Signature byte order is not this role order. | W §§3.1, 3.5 |
| ARC-002 | S | Each signer list contains 1–8 unique, strictly ordered heads. Accept multi-head merges, reject empty/duplicate/overbound lists, and require all claimed predecessor references to match the signer's chain when available. | W §§3.1, 3.4; D §10.3 |
| ARC-003 | S | A transaction's chronology floor is started_at for presence and timestamp otherwise; it must be at least every predecessor's effective time (finalized_at for presence, timestamp otherwise), across every signer and merge head. | W §3.3 |
| ARC-004 | C | Keep one identity archive across all patron bindings. Every transaction the identity signs advances its archive, including witnessed ceremonies; merely issuing an embedded verifier response does not add a presence signer back-pointer. | D §§10.0–10.3; W §§3.1, 3.5 |
| ARC-005 | C | Persist all concurrent heads and merge them when constructing the next signed event. Test independently produced branches and nonconsecutive counter values without imposing a fictitious total order between branches. | D §10.3; W §3.1 |
| ARC-006 | C | Preserve original canonical transaction bytes, signatures and content IDs in storage, export, fetch and replay. A parser's normalization must not silently manufacture a new historical record. | D §10.1; W §§1.4, 10.1 |
| ARC-007 | C | Treat a supplied earlier head as a deliberate truncation of recent history, not proof that it is the subject's latest head. From the chosen head, walk all required predecessor branches to the evaluator's lookback rather than accepting hand-picked older records as a complete archive. | D §10.1; I §5; W §7.9 |
| ARC-008 | C | Report an unavailable predecessor as incomplete/unverifiable history, a hash mismatch as invalid linkage, and a verified checkpoint/genesis as the appropriate stopping condition. Surface which branch or key prevented completion. | W §3.4; D §10.1; I §5 |
| ARC-009 | S | Archive requests bind subject, optional head, count 1–256, optional age stop and nonce; replies bind nonce, head-first reverse order and more flag. Verify the requested head rather than trusting the holder's unrelated answer. | W §7.9 |
| ARC-010 | C | Paginate from the frontier the previous reply returned, never from a single oldest item; preserve and visit every unresolved branch without infinite loops or a lost frontier. Do not confuse a short batch or a holder refusal with a short-lived archive. | W §7.9; D §10.3 |
| ARC-011 | C | Fetch infrastructure archives through request type 2 and light-client archives end to end from their holder. A server may refuse or return fewer records; enforce both count and request/reply frame limits without generating an oversized reply. | W §§7.9, 9.2; D §10.1 |
| ARC-012 | C | Permit checkpoint pruning only for an eligible whole old prefix beyond 730 days; do not excise individual inconvenient records from a retained interval. Keep the proof needed to identify the retained root and its relationship to history. | D §§10.1–10.2; L §2 |
| ARC-013 | C | Keep archive retention, capture retention, seed retention and selective disclosure as different decisions. Withholding a field or presenting an older head must not delete it; deleting an expired capture does not delete signed history. | D §§7.5.2, 8.1.1, 10.2; L §2 |
| ARC-014 | C | Present current-series proof as the originating adoption followed by its reissues. A longer consistent prefix supersedes an older proof; divergent proofs expose patron equivocation and do not yield a numeric-series winner. | W §4.6.1 |
| ARC-015 | C | Reject stale updates from an abandoned series even if their counters are enormous. Admit no endpoint state or forwarding until the corresponding series is proven current; keep independent proven bindings separate. | W §§4.6.1, 7.6, 10.1 |
| ARC-016 | C | The subject serves archive requests for its own archive over the peer-to-peer channel; no infrastructure node serves it on a subject's behalf, and a request naming another subject is answered empty. The fetcher verifies the walk rather than trusting it: the first record is the head requested, each record's back-pointers reach the one after it, and every signature is checked before anything is kept. An attestation delivery carrying no nonce the fetcher issued is not taken. | L §2; W §7.9; D §15 |
| ARC-017 | C | A participant's own store persists beside its archive with the lifetimes its contents have: records, sealed captures and seeds written once, and material that grows against a record rewritten. The seed that releases a capture key is the only secret among them and is not world-readable; ciphertext the holder cannot open needs no such care. Positions are re-derived from the archive rather than stored, so a restored archive reaches the same answer. | D §§13.7.1, 7.5.2; L §2 |

## 3.2 Infrastructure topology, propagation, addressing and currency

### Topology, horizon maintenance and materialized state

| ID | Kind | Requirement and functional test oracle | Authority |
|---|---|---|---|
| TOP-001 | C | Enforce one patron per relationship within a subnet and at most ten direct subordinates, counting infrastructure children too. Separate bindings in different subnets must not create a protocol-wide unique-patron rule. | D §§3.1–3.3 |
| TOP-002 | C | Require infrastructure before exceeding two consecutive light subordinate levels below the nearest infrastructure node; promotion preserves the node's identity and position. Test infrastructure inserted at the boundary and a third unsupported light level. | D §3.3 |
| TOP-003 | C | Form a new subnet with at least one infrastructure participant; support a peerless root and recommend a second independent infrastructure node. Root choice is reversible by ordinary departure, not a privileged permanent status. | D §§13.1–13.5 |
| TOP-004 | C | Compute h_store=2 by a two-edge adoption walk, including siblings and ancestors rather than just descendants. In a full internal neighborhood it can contain 221 identities including self; a serving node's 110 light descendants are a different bound. | D §§15.1, Appendix B.2; L §4.2 |
| TOP-005 | C | Horizon membership is symmetric for the adoption walk and does not follow peering. Distinguish membership from trust weight, serving responsibility and process-and-discard reach; a three-edge cousin is outside h_store. | D §§15.1, 16.2.1 |
| TOP-006 | C | Maintain the participant's own horizon locally from accepted topology, including the full path through an intervening light patron. Preserve enough validated path/key/distance data to reconstruct it while that patron is offline. | D §15.1.1; L §4.2 |
| TOP-007 | C | Topology changes invalidate departed horizon members and affected routes, catalogs, role inputs and direct-path eligibility. Serving-node updates override stale local guesses; do not retain obsolete relationships indefinitely for convenience. | D §§12.5, 15.1.1; L §§4.2, 8 |
| TOP-008 | E | The materialized view must equal replay of the authoritative accepted record set for the same owner and policy inputs. Test incremental updates versus full replay after adoption, termination, merge, recovery and reissue. | D §§10, 15; L §4.2; I §§4, 10.2 |
| TOP-009 | E | Treat persisted derived views as caches: reject a corrupt, stale or wrong-owner snapshot as a whole and rebuild. Persist accepted authoritative facts before publishing a derived success; a failed cache write must not rewrite signed history. | D §§10, 15; I §§4, 10.2 |
| TOP-010 | O | Keep h_process=3 distinct from persistent storage and do not use it to enlarge disclosure or create an undocumented flooding mode. Exact processing behavior beyond the defined messages needs an implementation profile. | D §15.1 |

### TopologyPush receipt, storage, forwarding and repair

| ID | Kind | Requirement and functional test oracle | Authority |
|---|---|---|---|
| TOP-011 | S | Dispatch TopologyPush kind 0 as a signed transaction, kind 1 as an EndpointRecord, kind 2 as a Delegation and kind 3 as a SubtreeAck, preserving the enclosed bytes; store a Delegation when it verifies under its delegating keyhash and keep the newest per keyhash. Reject malformed known push bodies and unsupported kind values with the specified frame-level behavior. | W §§8.2, 10.1 |
| TOP-012 | C | Store a pushed topology-class transaction only when valid and its type-defined subject is in the receiver's h_store; exercise adoption, departure, disavowal and reissue subjects plus either peering endpoint. Do not use all signers as interchangeable propagation subjects. | W §10.1.1 |
| TOP-013 | C | Forward if and only if the item was newly stored, to authenticated adjacent patron/children/peers/serving node/attached clients except the sender. No hop count or TTL is added, and a rejected or out-of-horizon item is not forwarded. | W §10.1.1 |
| TOP-036 | C | A participant keeps of a transaction it was no party to the identifier, the effective time and what the evaluation came to — the shape, the addresses, a patron's determination — and not the act. Its durable state carries those facts, and a state written before them carrying whole records is read and converted. A derived copy that cannot account for the facts is discarded whole and earned again by propagation and fetch, never by a fold with no input. | L §4.2; D §15.1.1; W §7.9 |
| TOP-035 | C | Persist the seen fact and the table it produced, never a transaction the node was no party to: after a restart the identifier suppresses a second forwarding wave, the body and any cited evidence are gone, and an upgrade migrates what an earlier version wrote before removing it: each body under the former layout yields its identifier and effective time, a body this node signed moves to its retained own acts, and only the foreign bodies and the evidence go. Replay to a new adjacency carries current-state objects and the node's own signed acts alone; a co-signed transaction is filed in the node's own archive and its own position read from there. | I §4.3; D §15.1.1; W §§10.1.1, 10.1.3 |
| TOP-014 | C | Missing transaction signer keys leave a pending unverifiable object that is not flooded as verified. Distinguish the explicitly allowed unverified endpoint routing hint from a verified topology transaction. | W §§3.4, 7.6, 10.1 |
| TOP-015 | C | Deduplicate transactions by txid against retained store state, not a short-lived packet cache; replay after restart or around a cycle must not create another forwarding wave. Do not discard a legitimate missing predecessor merely because a descendant was seen. | W §10.1.2 |
| TOP-016 | C | For an endpoint's proven series, a higher counter replaces older state, an exact equal-counter duplicate is inert, and a different equal-counter value makes neither candidate current. Report/re-resolve equivocation instead of arrival-order selection or forwarding the conflict as a new current endpoint. | W §§7.6, 10.1.2 |
| TOP-017 | C | There is no per-push acknowledgement/retry protocol. Missing predecessors can be fetched and periodic neighbor repair can replay the same defined frames; test reconvergence after a lost push without inventing new wire messages. | W §10.1.3 |
| TOP-018 | C | Keep topology replication separate from mailbox replication. Patron/sibling redundancy and optional peer backup do not copy ciphertext queues, grants, raw captures or an entire user's privately held archive as ordinary topology gossip. | D §§3.4, 14.1.6, 15; I §2 |

### Origination and attestation separation

| ID | Kind | Requirement and functional test oracle | Authority |
|---|---|---|---|
| TOP-019 | C | Originate topology only for relationships/state the local participant is party to, while allowing an authenticated relay to forward an unchanged stored object it did not sign. Requiring every forwarding peer to be an original signer would incorrectly stop the flood. | W §10.1 |
| TOP-020 | C | Presence attestations are pulled for an actual evaluator's need, not treated as topology transactions to flood or speculative evidence to push. Do not leak the full locally held archive/capture bundle merely because a neighbor attaches. | D §15; L §4; W §§4, 10.1 |

### Rootward memo and cycle repair

| ID | Kind | Requirement and functional test oracle | Authority |
|---|---|---|---|
| TOP-021 | S | A memo has only patron identity, that patron's locator, slot 0–9, the underlying transaction timestamp, and optional current occupant. An absent occupant means empty; reject malformed fields and unsigned unknown additions. | W §§8.2, 10.2 |
| TOP-022 | C | Emit rootward hints for adoption, departure and disavowal only. Do not place peering IPs, EndpointRecords, reasons, recovery links or fresh local-clock timestamps into the memo stream. | W §10.2; D §15.2 |
| TOP-023 | C | Send through the applicable nearest infrastructure ancestor, including light-patron paths; stop at the root and drop a memo for another anchor/tree. Peering is not a route for rootward index propagation. | W §10.2; D §§15.2, 19.4 |
| TOP-024 | C | If maintaining the memo table, key it by (patron, slot), retain current empty tombstones, replace only with a later timestamp and keep the first arrival on an equal-time tie. Do not forward an update already held at equal or greater time. | W §10.2.2 |
| TOP-025 | C | The optional table represents current slots, not an update history. Bound retention to its subtree role; permit ancestor key-to-position lookup only within that subtree without turning it into a public cross-subnet directory. | D §§12.4, 15.2.1; W §10.2.2 |
| TOP-026 | C | A memo alone never proves misconduct or authorizes disavowal. Confirm positive locally verifiable signed relationship state before acting; forged, inconsistent, stale or unconfirmed hints cause no cut. | W §§10.2.3–10.2.4; D §15.2 |
| TOP-027 | C | Detect cycles by identity relationships, not path-prefix coincidence. For a confirmed distant cycle cut the appropriate direct subordinate edge using cycle-repair reason 5; an infrastructure proxy must not sign an attached client's departure/disavowal on its behalf. | W §10.2.4; D §6.2.5; L §7 |
| TOP-028 | C | For a mutual-adoption cycle involving the local human, surface the choice of intended direction; for a distant verified cycle perform the specified automatic repair. Mere uncertainty about a relationship must not trigger a cut. | D §6.2.5; L §7 |
| TOP-029 | C | Detect duplicate occupancy from current memo state, notify down the other implicated branch as specified, and confirm signed local facts there. Do not compare unrelated patrons' timestamps as a network-wide order. | W §§10.2.1–10.2.4 |
| TOP-030 | S | A patron issues a locator index none of its open subordinates holds, and a holder keeps one occupant per patron slot per subnet: an adoption naming a filled slot is not stored and the incumbent stays. The eleventh subordinate has no index left to name. | D §3.1; W §2.1 |
| TOP-031 | C | On an ending that leaves the party no patron in that subnet, re-anchor it on itself at the empty path and shorten every path beneath it by the prefix that reached it. An ending leaving a patron standing moves nothing. A later adoption clears the self-anchor; paths beneath an ancestor that moves are not translated. | D §§12.1, 12.6.2; L §4.2; W §4.2 |
| TOP-032 | C | Accept an EndpointRecord from a signer nothing had marked infrastructure: publication is the publisher's commitment and a receiver cannot distinguish an infra node from a light client that signed one. Holding the record is what marks the publisher; the bound that applies is the horizon. | W §7.6; D §1.1 |
| TOP-033 | C | A storage decision admits only what the node's own materialized state can hold: refuse before retaining or forwarding, never apply the refusal afterwards. A forwarding node vouches with its storage decision, so storing and then rejecting floods what it has itself declined. Cover slot occupancy, and distinguish a held record replaying from a live conflict. | W §§10.1.1–10.1.2; D §3.1 |
| TOP-034 | C | Exercise the endpoint-line rules at every holder that keeps one, node and participant alike: exact duplicate, older, incomparable, equal-sequence retirement of both contents, and an unproved second series. A rule stated once and implemented twice must be tested at both, including across a cache restore, where the retirements travel with the addresses. | W §10.1.2; L §4.2 |

### Locators, anchors, endpoint publication and iterative resolution

| ID | Kind | Requirement and functional test oracle | Authority |
|---|---|---|---|
| RES-001 | S | A signed locator binds subject, anchor, complete path and the relationship sequence under the locator domain. Reject signature or identity mismatches; a well-formed empty path can identify the anchor itself. | W §2.3; D §12.1 |
| RES-002 | C | Introductions supply a locator/key binding out of band; a bare unknown keyhash is not a global lookup query. On an unknown anchor, fail locally without spraying queries; a higher known anchor or reintroduction is the remedy. | D §§12.2–12.4; W §7.7 |
| RES-003 | C | Hold the required child infrastructure routes/endpoints and last verified positions, and resolve through infrastructure only. A light caller initiates through its serving node; do not descend by querying intermediate light participants. | I §§4.1–4.2; W §§7.7.1–7.7.2 |
| RES-004 | S | Each resolution exchange carries the full unmodified target anchor/path and nonce. Validate echoed nonce and reply alternative; a referral must advance to an infrastructure prefix on the requested path without overshooting or looping. | W §7.7.3 |
| RES-005 | C | A node with a deeper cached serving result may answer directly. Terminal resolution distinguishes the serving infrastructure itself from a residual path for one of its attached clients; preserve that residual for final delivery. | W §7.7.3; D §12.6.1 |
| RES-006 | S | ServingInfra supplies full KeyMaterial that hashes to its named identity; pin and authenticate it before any non-query service. Reject key substitution even when the IP or classical TLS key is familiar. | W §§7.7.3, 9.1 |
| RES-007 | C | An unpinned anchor/referrer may be used for the narrowly permitted unauthenticated resolution query, never for attach, payload, key submission or resource traffic. Do not confuse a usable hint with authenticated authority. | W §7.7.3; D §§12.2, 12.6.1 |
| RES-008 | C | Try alternate NetworkPoints for the same authority on connection failure. A fresh logical query gets a fresh nonce; retrying its endpoints preserves correlation to that query. A forged/unrelated response must not update the cache. | W §7.7.3; I §4 |
| RES-009 | C | Distinguish failure codes: no child, no authority (re-resolve), temporary failure (policy retry) and policy refusal. A node's refusal is not proof that the destination does not exist; avoid inventing redirect semantics. | W §7.7.3 |
| RES-010 | C | Stale paths fail without an address-translation service. Exercise adoption/departure, subtree moves and slot reuse; never deliver to a different key merely because it now occupies the old slot. | D §§12.6.2, 12.7.7; W §7.7 |
| RES-011 | C | Anchors are ordinary ancestors with locally cached entries, not a privileged tier. Support zero anchors and per-node caching policy, guideline S≈500,000 and hysteresis where used, without rejecting smaller legitimate anchors. | D §§12.2, 12.7.3; W §7.2 |
| RES-012 | C | Publish signed anchor/endpoint state and validate it when the owner's material is known. Preserve the verification status of unknown-key routing hints rather than treating a cached ASN or IP as authenticated identity. | W §§7.2, 7.6; I §4.4 |
| RES-013 | C | Increment an endpoint/locator counter for a real changed or reordered endpoint publication; replay of unchanged state need not mint a new update. Track freshness within each proven relationship series, including simultaneous subnet bindings. | W §§2.3, 7.6 |
| RES-014 | C | Use explicit lifetimes for positive/negative resolution, contact locators and intermediate addresses; expire stale entries and briefly cache negative results. Cache expiry affects performance, not currency security or signed historical truth. | D §§12.5, 12.6.4; I §4.3; L §5 |
| RES-015 | C | Resolve only for an actual communication need or specified maintenance. A speculative per-click query can disclose interest; do not turn local catalog browsing into resolution traffic. | D §12.6.1; I §4.5; L §8 |

### Currency issuance, refresh, use and supersession

| ID | Kind | Requirement and functional test oracle | Authority |
|---|---|---|---|
| CUR-001 | S | Verify CurrencyAttestation subject, issuer, role, validity interval and classical signature under the currency domain. Exercise each defined issuer role 0 patron, 1 sibling, 2 grandpatron and 3 downline. | W §7.1 |
| CUR-002 | C | Issue automatically under standing policy only for a presently recognized relationship. A past record, unsupported issuer role or already superseded identity is not enough to mint fresh current standing. | I §3; D §12.6.5 |
| CUR-003 | C | Treat bad-signature, wrong-subject or expired currency attached to Attach as absent, not a TLS authentication failure. An authenticated participant remains able to attach, refresh and operate without current currency. | W §§7.1, 8.2; D §12.6.5 |
| CUR-004 | C | Use currency to select the currently addressed key, not as a universal connectivity permission. Genesis identities have no inherited standing to attest, and unattested/root operation remains supported. | D §§12.6.5, 12.7.1–12.7.2 |
| CUR-005 | C | Before introduction reuse a fresh local attestation or seek renewal; distinguish an explicit no-issue response from no answer. Refresh via the introducer first where specified, then use the defined CurrencyRequest path without exposing a separate querier field. | D §12.6.5; W §7.1 |
| CUR-006 | S | CurrencyRequest and CurrencyReply bind subject and nonce and expose only the defined attestation/no-issue alternatives. Reject unsolicited, wrong-nonce or structurally invalid replies; do not add a recovery explanation to a no-issue answer. | W §7.1 |
| CUR-007 | P | Treat the chosen roughly ten-hour lifetime as a security parameter and respect actual signed expiry. Test exact validity boundaries and outage; cache policy must not extend a signed lifetime to mask unavailable issuers. | D §§12.6.4–12.6.5; W §7.1 |
| CUR-008 | C | On known local supersession stop accepting/issuing old-key state immediately even if an old staple is unexpired; discard related sessions and authority where required. Outside that observer's horizon do not expose a global old-to-new redirect. | D §9.0.2; I §3 |
| CUR-009 | P | Exercise renewal fallback through eligible patron siblings/grandpatron and optional light-patron delegation/root downline support. Standing policy controls patience and eligible support; no invented universal quorum is allowed. Encoding gaps are O-012. | D §§12.6.5, 12.7.2; I §3 |
| CUR-010 | C | Concurrent valid recovery heirs do not gain a single global currency winner. Keep local lineage choice and contradictory evidence visible; absence of currency cannot freeze every other operation during an issuer outage. | D §§9.0.2, 12.6.5 |
| CUR-011 | S | A currency attestation whose current key (field 2) differs from its subject (field 1) is a valid message and is accepted, not rejected for the difference: field 2 equal to field 1 is not required. This is the honest rotation-report and fork-signal form. Acceptance CUR-20. | W §7.1 |
| CUR-012 | C | A currency attestation never redirects the addressed key. A relying party addresses the key it holds by participant-authored evidence, a signed locator or a recovery adoption, and reads field 2 only as a rotation/fork signal; a lone attestation naming a differing current key does not move the addressed key onto field 2. Acceptance CUR-19. | W §7.1; D §§12.1, 12.6.5 |

## 3.3 Transport, sessions, mailbox, prekeys and wake

### QUIC/TLS, framing, stream dispatch and replay safety

| ID | Kind | Requirement and functional test oracle | Authority |
|---|---|---|---|
| NET-001 | S | Use QUIC over TLS 1.3, ALPN rhtn/1 and the X25519MLKEM768 key exchange group only; incompatible group/ALPN/version negotiation fails without classical-only downgrade. | W §§9.1–9.2; D §§5, 14.1.3 |
| NET-002 | S | Authenticate the presented raw Ed25519 SPKI against the keyhash intended: it is the classical member of previously validated full hybrid KeyMaterial for that keyhash, or the attach that follows carries that keyhash's delegation naming it; refuse the session when neither holds. A classical key alone cannot reconstruct or establish trust in the pair; reject a different PQ half with the same claimed identity. | W §§9.1, 8.2 |
| NET-003 | S | Frame control messages with four-byte big-endian length and canonical CBOR [type, body], at most 65,536 bytes. Unknown control types are skipped; malformed known bodies are discarded whole while the previous state and session survive. | W §§8.0, 9.2 |
| NET-004 | S | A declared over-limit frame ends the session as specified before allocating its body. Truncated frames, split reads and multiple frames in one read are handled without losing framing alignment or accepting partial state. | W §§8.0, 9.2 |
| NET-005 | S | Each bidirectional request stream carries one typed request and its length-framed untyped reply. Enforce the 262,144-byte request/reply frame ceiling, permit answering before EOF, and reject a second request/trailing request content on that stream. | W §9.2 |
| NET-006 | S | Unknown request types reset their stream, not the whole connection. Before a tag can be decoded reset malformed requests; after the tag is known use that request type's specified reset/error behavior rather than a generic invented reply. | W §§9.2, 11 |
| NET-007 | S | Gate replayable 0-RTT to explicitly read-only resolution, archive and catalog queries. Defer/reject Attach, one-time prekey consumption, verifier processing, registration, payload submission, wake changes and every resource request until handshake confirmation. | W §9.2 |
| NET-008 | C | Permit QUIC migration and outbound establishment through NAT without requiring a public incoming listener on a light device. Keep hop-authenticated peer identity stable across a valid address change. | D §§14.1.1–14.1.3; W §9 |
| NET-009 | S | Capabilities is a required map that may be empty, up to 64 entries and 1024-byte values. Derive parameter IDs from the first eight big-endian bytes of SHA-256(rhtn/cap: plus the namespaced name); unknown IDs/opaque values are ignored safely. | W §8.1 |
| NET-010 | C | Emit the required per-session random unrecognized capability ID with an eight-byte random value, avoiding known IDs. Absence of optional capabilities must not prevent the base protocol from working. | W §8.1.1 |
| NET-011 | E | Run fragmented/read-coalesced control and request streams concurrently and isolate an individual stream's failure. A malicious large frame or bad request must not be mistaken for a valid later message on a different stream. | W §§8.0, 9.2 |

### Attach, sibling state, heartbeat and failover

| ID | Kind | Requirement and functional test oracle | Authority |
|---|---|---|---|
| SES-001 | S | Attach's keyhash names the identity whose classical member is the transport-authenticated key, or Attach field 4 carries that identity's delegation naming the presented key; reject a mismatch, and reject a delegation naming any other key, outside its window, or by another keyhash. Server determines primary versus degraded from its own topology; a client cannot claim a mode by sending stale local relationship information. | W §§8.2, 9.1 |
| SES-002 | C | Serve the infrastructure node's light descendants across both allowed levels, including a client whose immediate patron is light. The serving node and the social patron remain distinct roles. | I §1; D §§3.3, 14.2.1 |
| SES-003 | S | AttachAck fixes a heartbeat interval of 1–3600 seconds, advisory local u64 queued count and server capabilities. Reject zero, overflow or out-of-range interval and do not silently change the interval during that session. | W §8.2 |
| SES-004 | S | A sibling list has at most nine unique identities other than the receiving client, each with 1–8 endpoints. Reject duplicate/self entries or any malformed entry as a whole rather than installing a partial list. | W §8.2 |
| SES-005 | C | Sender omits a sibling's full KeyMaterial only if that sender previously supplied it to that client. Receiver can use any validated matching pin regardless of provenance; without material or a prior pin the sibling is unusable, never an unauthenticated fallback. | W §8.2 |
| SES-006 | C | Every successful AttachAck, including a failover Ack, and every valid SiblingUpdate replaces the complete cached list. Absent list clears it; a malformed update leaves the old list untouched and session alive. | W §8.2 |
| SES-007 | C | Persist the sibling list together with its usable key pins for first failover after restart. An invalid/partial persisted list must not produce speculative unauthenticated dials; initialize it safely and refresh on attach. | L §4; W §8.2 |
| SES-008 | S | Each side independently sends heartbeat sequence 0,1,… at the negotiated interval, with the first beat after one interval. Advisory timestamps and locator counters do not determine liveness. End the session at u64 exhaustion rather than wrapping. | W §8.2; D §14.1.2 |
| SES-009 | C | Any valid not-previously-seen heartbeat resets the monotonic receipt clock even with gaps; duplicates do not. Three elapsed missed intervals cause failure, not three missing sequence numbers, and ordinary payload traffic cannot keep a silent heartbeat peer alive. | W §8.2; D §14.1.2 |
| SES-010 | E | Test exact three-interval deadline, delayed scheduling, suspend/resume and a heartbeat readable at the deadline. Process available valid heartbeat input before declaring a false outage and do not send a burst of fabricated missed beats as live observations. | D §14.1.2; W §8.2 |
| SES-011 | C | On a dark primary try cached authenticated siblings/endpoints in the defined order. A policy-refusal close code 1 from the primary is not an outage and must not trigger a policy-bypassing sibling search. | D §14.1.2; W §9.2 |
| SES-012 | C | Treat a sibling policy refusal as refusal by that node and try remaining eligible nodes; distinguish no Ack/other close as endpoint failure. Remain visibly degraded after successful failover rather than automatically probing/failing back to the primary mid-session. | W §9.2; L §4 |
| SES-013 | C | Do not fabricate patron countersignatures or standing in degraded mode. Queue counts refer only to the responding node, so zero on a sibling is never shown as proof that the dark primary's mailbox is empty. | D §14.1.2; W §8.2 |
| SES-014 | C | A participant can open authenticated request-only sessions to resource hosts and other allowed infrastructure nodes without attaching to them or stealing an attachment. Separate service connection lifetime from social/serving membership. | W §11.1; L §8 |

### Cold-start outage

| ID | Kind | Requirement and functional test oracle | Authority |
|---|---|---|---|
| SES-015 | C | If the primary cannot be reached during initial attach after restart, use the persisted validated sibling list immediately under endpoint retry policy. Do not wait for three heartbeat misses on a session that never existed. | L §4; D §14.1.2 |

### Prekey publication, opaque deposit and ciphertext relay

| ID | Kind | Requirement and functional test oracle | Authority |
|---|---|---|---|
| MAIL-001 | S | Prekey publication request 9 carries the signed subject bundle and nonce. Bind publisher to authenticated subject, validate the outer signed object and allowed construction/size, and do not require infrastructure to parse opaque PQXDH cryptographic internals. | W §§7.8, 7.10; I §§6, 6.1 |
| MAIL-002 | S | One-time deposit request 10 accepts a bounded array of 1–256 opaque prekeys under the authenticated relationship. Enforce total frame size and reply nonce/code; do not silently trim an oversized or partially failed deposit into a success. | W §7.10; I §6.1 |
| MAIL-003 | C | Store the current reusable prekey bundle, replace it according to the specified publication lifecycle, and serve it repeatedly without consumption. A one-time prekey is returned at most once, including concurrent requests and process restart. | W §7.8; I §6 |
| MAIL-004 | S | A single prekey request/reply binds subject and nonce and distinguishes success, unknown and refused. Return one-time material only when requested and available; reusable-bundle success can still occur on one-time exhaustion. | W §7.8 |
| MAIL-005 | S | Batch prekey requests hold 2–256 subjects in the specified sorted order. Validate subject/nonce association and per-subject result, without allowing duplicate targets to multiply consumption or unsolicited keys to populate a cache. | W §7.8 |
| MAIL-006 | P | Apply prekey rate limits per requester and per subject so one requester cannot exhaust a victim's pool by spreading requests, and many requesters cannot bypass a subject budget. Values are policy parameters; do not persist a detailed request history. | I §6; D §14.2.4 |
| MAIL-007 | C | Notify the recipient when its one-time prekey supply needs replenishment, and support configured reusable/last-resort behavior while explicitly distinguishing weaker first-message forward secrecy. No unlimited automatic batch is inferred from an empty pool. | I §6; D §14.2.4 |
| MAIL-008 | S | Relay request 11 carries recipient, opaque ciphertext and nonce. Unknown recipient or unavailable custody returns a defined failure; known offline recipient can be accepted into custody. A code-0 submission reply means accepted, not end-user delivery. | W §7.10; I §6.1 |
| MAIL-009 | C | Preserve ciphertext unchanged across serving nodes, queue and RelayedPayload. The peer/key hint is a routing hint, not authenticated authorship; use end-to-end authentication for the delivered sender identity. | W §§7.10, 8.2; D §14.2 |
| MAIL-010 | S | SubmissionReply echoes the request nonce and exactly the defined codes 0 accepted, 1 refused, 2 unknown. Do not append invented explanatory fields to unsigned replies or report storage failure as accepted. | W §7.10 |
| MAIL-011 | E | Persist one-time consumption/custody before acknowledging success; inject failure between validation, durable write and reply. Retrying after an ambiguous reply may not disclose the same one-time prekey twice or lose acknowledged stored ciphertext. | I §§2, 6; W §§7.8, 7.10 |
| MAIL-025 | C | A device's prekey bundle is made over material that device generated and signed by the identity on the ceremony device, naming the device under the signature; the signer refuses a payload naming another subject, and the device refuses a signed bundle that does not verify under its identity, names another device, or is not over its current material. A device whose material is unsigned publishes nothing, stocks its own pool, and offers the payload again when its material rotates. | W §7.8; D §§14.2.4, 23.3; L §3 |

### Mailbox retention, delivery and external wake

| ID | Kind | Requirement and functional test oracle | Authority |
|---|---|---|---|
| MAIL-012 | C | Mailbox stores only ciphertext, recipient keyhash and arrival time needed for custody; it does not retain a sender/recipient activity ledger or decrypted payload/profile. Apply the same minimization to logs and diagnostics. | D §§14.1.6, 19.5; I §2 |
| MAIL-013 | C | Hold queued ciphertext indefinitely while the recipient is offline, bounded by a per-recipient storage cap. At the cap refuse the newest incoming item and tell its sender; never evict older accepted mail or silently age it out. | D §14.1.6; I §2 |
| MAIL-014 | C | On delivery remove ciphertext immediately with no recoverable journal, backup or crash-recovery copy. Verify disk and recovery paths as well as the live queue API; exact delivered/ack boundary must follow the selected transport implementation without claiming an unspecified end-user receipt. | D §§14.1.6, 19.5; I §2 |
| MAIL-015 | C | Restart recovers accepted undelivered queue entries but not already deleted ones. A live-send failure follows the custody/queue policy and surfaces refusal when storage is unavailable. | D §14.1.6; I §2 |
| MAIL-016 | C | Siblings hold no replicated mailbox state. Failover serves only the sibling's own queue; leave a dark primary's pending mail pending rather than falsely reporting delivery or automatically inventing an upline queue. | D §§14.1.6, 23.1; I §2 |
| MAIL-017 | C | After recognized key supersession stop old-key delivery and authority as specified; do not drain the old queue to an unverified claimant of succession. The new identity must authenticate independently. | D §9.0.2; I §§2–3 |
| MAIL-018 | S | WakeRegistration request 12 binds nonce and optional endpoint/key/expiry. An endpoint requires its authentication key; withdrawal omits all three. Enforce endpoint/key size bounds and reject orphan key/expiry fields. | W §7.10 |
| MAIL-019 | C | Retain one optional wake registration per serving relationship, refresh by replacement, withdraw on request, expire by the stated policy and forget it when the relationship ends. A foreign authenticated node cannot overwrite another subject's registration. | I §6.1; W §7.10 |
| MAIL-020 | C | The serving node receives a client-supplied external endpoint and authentication key, not APNs/FCM credentials. It sends a content-free doorbell without sender, message count or payload and does not operate a new protocol-wide push service. | D §14.1.5; I §6.1; L §4.1 |
| MAIL-021 | C | Default to foreground reconnect when no wake service is configured. A delivered doorbell only invites reconnection; it does not imply queued mail was read or delivered. Surface user control of the external service. | D §§14.1.4–14.1.5; L §4 |
| MAIL-022 | O | Exercise actual external wake posting and the iOS relay only against a documented chosen integration profile. Keep endpoint registration and content minimization testable now; exact service bindings and shipping relay behavior remain deployment work. | D §14.1.5; I §12; L §10 |

### Client prefetch and issuance privacy

| ID | Kind | Requirement and functional test oracle | Authority |
|---|---|---|---|
| MAIL-023 | C | Prefetch reusable material for the whole eligible trust horizon as a visibly batched maintenance operation, rather than a targeted request for every browsing or contact interest. A batch never requests, returns or consumes a one-time key. | D §14.2.4; W §7.8; L §3 |
| MAIL-024 | C | Request one-time material for actual session initiation and report its exhaustion without concealing the resulting first-message downgrade. Per-requester/per-subject issuance budgets must remain effective across concurrent callers and the selected persistence profile. | D §14.2.4; W §7.8; I §6 |

## 3.4 Participant ceremonies, verifier exchanges, captures, recovery and payload

### Live ceremony orchestration and witness participation

| ID | Kind | Requirement and functional test oracle | Authority |
|---|---|---|---|
| CER-001 | C | Require both participants' intentional live participation before asserting their presence or adoption. Ordinary witness/verifier work and infrastructure bookkeeping run automatically under standing policy rather than repeatedly asking an operator to approve traffic. | D §§7.1, Appendix A.3; L §1 |
| CER-002 | C | Each participant nominates witnesses from the counterparty's neighborhood using branch-spread/random selection followed by availability probing, rather than choosing from an advertised willing-witness list. Report unavailability and imbalance. | D §7.1.1; L §1.1 |
| CER-003 | C | Keep nomination attribution through collection and signing. A participant refuses a final body claiming it nominated someone it did not nominate; do not silently relabel the other participant's choices to make the record look balanced. | W §4.5; L §1.1 |
| CER-004 | C | Witness clients observe the protocol evidence, responsiveness and local clock tolerance automatically and record which participant(s) they observed. They do not claim their operators physically attended the meeting. | D §§6.4, 7.1; L §1.2 |
| CER-005 | C | Establish the common ceremony ID before capture from both independently contributed 16-byte random values, ordered by participant keyhash, using SHA-256 with rhtn/1:ceremony. Changing either contribution or order changes the ID; do not substitute the later transaction ID. | D §7.5.2; L §1.3 |
| CER-006 | C | Run the strongest available proximity mechanisms honestly and retain pass/fail/unavailable distinctions. Optical exchange binds keys and transcript; low-end devices can complete a weaker ceremony without claiming absent NFC/UWB success. | D §7.6.3; L §1.3 |
| CER-007 | C | Treat latency as a coarse upper-bound consistency signal using the prescribed minimum/sample treatment, not fine location or proof of short distance. UWB/NFC/optical evidence and integrity status remain visible to weighting. | D §§7.6, 7.6.3; L §1.3 |
| CER-008 | C | Do not collect/use Bluetooth RSSI or a radio-environment fingerprint as the protocol's co-presence proof. Retain only the selected coarse geohash evidence, not raw precise coordinates in the signed record. | D §§7.6.1–7.7; L §1.3 |
| CER-009 | C | Finalize a record with the responses actually received, within the 24-hour structural bound, without inventing signatures from unavailable participants/witnesses/verifiers. Failure to reach a verifier target is visible thin evidence, not an indefinitely pending mandatory protocol quorum. | W §§3.2, 5.5; D §13.5 |
| CER-010 | C | Participant veto includes checking every held verifier response against the proposed final body. A signer holding an omitted response refuses to sign; a holder lacking that response cannot claim to have detected the omission. | W §§5.5, 5.7; D §8.1.2 |

### Curated bundles, verifier selection and fishing

| ID | Kind | Requirement and functional test oracle | Authority |
|---|---|---|---|
| CER-011 | S | A offered record qualifies only if it is an individually valid presence naming the subject as participant and finalized strictly inside (max(0, started_at−730 days), started_at). Exercise exact boundaries, formation records and witness-only participation. | W §§5.3–5.4 |
| CER-012 | C | Accept qualifying records across series without walking their archive chain as a selection prerequisite. Count distinct txids as n and distinct former counterparties as candidates, excluding the current counterparty; repeated records or meetings cannot inflate candidate identities. | W §§5.3–5.4 |
| CER-013 | C | Keep n an asserted count over the subject's curated bundle, not a remotely proven lifetime count. Do not crawl a subject's whole archive or require a complete bundle to participate. | W §§5.2–5.4; D §8.1.2 |
| CER-014 | C | Seek min(floor(n/2), 10, number of distinct candidates) verifiers per subject. Test n=0,1,2, odd n, large n, repeated counterparties and small candidate pools; do not mistakenly cap the combined two-subject response set at ten. | W §§1.3, 5.2 |
| CER-015 | C | Selector chooses the other participant's verifiers: prefer people it personally met, then its horizon, then the specified reachable acquaintance/horizon expansions limited to one further edge; discretionary fill is explicitly labeled. | W §5.1; D §8.1.2; L §1.4 |
| CER-016 | C | Preserve selection basis 0 met, 1 horizon, 2 reachable, 3 fill in each query/response. It is the selector's claim, not an assertion that the verifier independently knows the selector's entire graph. | W §§4.5, 5.1, 5.6 |
| CER-017 | C | Fishing proposes additional qualifying records directly from the subject's retained material under the same elective disclosure choice. Do not secretly enlarge a local/remote history store or impose completeness after the first bundle. | D §8.1.2; W §5.4 |
| CER-018 | P | Continue fishing until the chosen responsive threshold/patience or participant decision, accepting mutually known eligible additions according to the stated procedure. Distinguish no response, explicit refusal and unavailable evidence; do not silently represent policy-filtered candidates as offline. | D §8.1.2; L §1.4 |
| CER-019 | C | Show the actual selected verifiers, bases, coverage and absence of familiar candidates before signing. Sparse evidence may lower third-party weight while still yielding a valid personally meaningful relationship. | D §§8.1.2, 16.1; L §§1.4, 6 |
| CER-020 | C | Do not invent a protocol byte/count cap for the entire off-record curated bundle. Any local operational truncation is explicit to the user and selector and must not misstate n or completeness. | W §5.4 |

### VerifierQuery, consent, key grants and response processing

| ID | Kind | Requirement and functional test oracle | Authority |
|---|---|---|---|
| VER-001 | S | Query ID is SHA-256 of the canonical VerifierQuery map without key 6 and binds subject, querier, ceremony, fuzzed profile, template version and addressed verifier. Recompute before accepting consent; any substituted field invalidates the authorization. | W §5.6 |
| VER-002 | S | Subject consent is the specified classical Sign1 over raw query ID under rhtn/1:consent. Wrong subject, payload encoding, domain, profile/version, ceremony or verifier is not reusable consent. | W §§1.1, 5.6 |
| VER-003 | S | Verifier request type 4 contains [query, consent, selection basis]. Authenticate its peer as the query's querier and verify that the request is addressed to this verifier before processing profile data or releasing any result. | W §§5.6, 9.2 |
| VER-004 | C | Invalid/missing consent, wrong query hash or identity mismatch yields the defined refusal/reset behavior without a fabricated signed match/no-match response. Do not call the matcher or open held captures first. | W §5.6; D §7.4.2; L §1.4 |
| VER-005 | C | Subject authorizes one fuzzed profile/template version under the active ceremony pre-commitment and retains the state needed to prevent variants. Changing the addressed verifier requires its own query ID/consent but does not authorize changing that committed profile. | D §7.4.2; W §5.6 |
| VER-006 | P | Enforce verifier-side requester and subject rate limits, plus subject-side per-requester aggregate consent/grant limits across verifiers. Test distributed requester/verifier combinations and reset at the configured ceremony window. | D §7.4.1; I §6; L §1.4 |
| VER-007 | C | Keep only the expiring anti-oracle counters needed for that window, not a durable history of who queried whom. A subject-facing probing notice reports meaningful behavior without turning every ordinary packet into an approval prompt. | D §§7.4.1, Appendix A.3; L §1.4 |
| VER-008 | C | If selection basis says met and the verifier's own held evidence refutes it, return unavailable as specified; other bases are echoed as claims rather than independently certified graph reachability. | W §§4.5, 5.6 |
| VER-009 | S | Capture KeyGrant binds record txid, query ID and exactly 32 key bytes; receive it only from the authenticated subject over end-to-end transport. It is not a signed archive transaction or a grant that the querier may substitute. | W §7.3; D §7.5.2 |
| VER-010 | C | Deliver a grant directly to the selected verifier, bypassing querier and witnesses. Match it to active subject consent, subject/holder identity and held capture; an unsolicited grant is never permission to issue an unrelated response. | D §7.5.2; W §§5.6, 7.3 |
| VER-011 | P | A grant arriving before its query is buffered unopened for a short bounded local period. The first authenticated grant stands; exact duplicates and conflicting later grants do not replace its key or extend authorization. | W §7.3 |
| VER-012 | C | Missing capture, unreleased key or unsupported template version produces unavailable with no comparison basis/version. Authenticated but unusable/truncated ciphertext produces inconclusive rather than a biometric no-match. | D §§7.4.4, 7.5.2; W §4.5 |
| VER-013 | C | Compare a supported older modality with its corresponding legacy engine; do not reinterpret old templates using a new incompatible matcher. For photo/both results carry the query's template version and appropriate basis. | D §7.5; L §§1.3–1.4; W §4.5 |
| VER-014 | S | Sign only the defined bounded response and preserve subject, verifier, querier, ceremony/query evidence and selection fields as specified. Ordinary results are match, no-match, inconclusive or unavailable, never a numeric similarity oracle. | W §§4.5, 5.6; D §§7.3–7.4 |
| VER-015 | C | Send the signed response to both querier and subject so either can retain it for final-record veto. Delivery to one side is not proof the other received it; adversarial dropping must expose that evidence limit. | W §§5.5–5.7; L §1.4 |
| VER-016 | C | Retain late replies privately for the participants or as the optional LateVerifierResponse object targeting the original presence record. Do not amend its txid/signatures, extend the parent's retention or falsely count it as present at finalization. | W §7.4; D §8.1.2 |
| VER-017 | S | Validate a late response's target record, subject, original consent and ceremony linkage. Reject attaching a genuine response to an unrelated record or changing its addressed query while preserving its signature. | W §7.4 |
| VER-018 | C | A verifier does not routinely ask its human operator to compare faces; automated comparison uses already-held material. In-person personal recognition during recovery is a distinct authorized human act. | D §§7.3, 9.0.1, Appendix A.3; L §1.4 |

### Capture acquisition, encryption, lifecycle and selective disclosure

| ID | Kind | Requirement and functional test oracle | Authority |
|---|---|---|---|
| CAP-001 | C | Guide a 3–5 image sequence over roughly 10–15 seconds with varied randomized movement prompts rather than an uncontrolled burst. Record actual modality and liveness pass/fail/not-performed, including unavailable hardware paths. | D §§7.1, 7.5; L §1.3 |
| CAP-002 | E | Evaluate actual camera/liveness integration against printed images, screens, replayed and generated video and varied devices/conditions. Motion prompts alone are not evidence of resistance; report measured false accepts/rejects rather than claiming an untested percentage. | D §§7.1, 20.1–20.2; L §1.3 |
| CAP-003 | C | Crop faces and strip EXIF, GPS, timestamps and device identifiers before sealing. Warn about residual visual background and do not retain unneeded full frames or debugging thumbnails outside the controlled lifecycle. | D §§7.5, 19.4; L §§1.3, 1.5 |
| CAP-004 | C | Generate a 32-byte per-subject capture seed and derive the holder-specific key with HKDF-SHA256, empty salt, and info rhtn/1:capture plus subject keyhash, holder keyhash and ceremony ID in the specified order. Changing any binding changes the key. | D §7.5.2; L §1.3 |
| CAP-005 | C | Seal the fixed-length canonical template first and images under the same authenticated encryption context for that presence. Keep separately sealed captures from different ceremonies; do not overwrite all prior evidence with the latest portrait. | D §7.5.2; L §§1.3, 2 |
| CAP-006 | C | Erase plaintext captures, temporary profiles and derived decryption keys after sealing/comparison. The holder retains ciphertext; subject-controlled seeds remain separate and never appear in network records or unencrypted infrastructure state. | D §7.5.2; L §§1.3–1.4, 2 |
| CAP-007 | C | Default capture retention to two 365-day years, implement expiry deletion, and allow the subject's explicit shorter/longer release choices. Withheld/deleted material yields unavailable; do not silently reuse another ceremony's release. | D §§7.5.1–7.5.2; L §§2, 5 |
| CAP-008 | C | Default to the most recent eligible capture key where the subject chooses release, while allowing appropriate explicit selection. Explain that withholding cannot revoke a malicious holder's retained plaintext and that deletion may reduce future continuity/recovery evidence. | D §7.5.2; L §§2, 5–6 |
| CAP-009 | S | Commit exactly seven disclosures in lexicographic label order: capture, location, p0.integrity, p0.retention, p1.integrity, p1.retention, proximity. Each revealed value binds its 16-byte salt, label and correctly typed value. | W §4.5.1 |
| CAP-010 | S | Compute leaf SHA-256(0x00 plus canonical [salt,label,value]) and root SHA-256(0x01 plus the seven leaf digests). Verify against independent vectors; altered salt, label, value, slot or prefix must not preserve a valid commitment. | W §4.5.1 |
| CAP-011 | S | PresentedRecord preserves the original signed envelope and has exactly seven revealed-or-32-byte-digest slots. Test all 128 disclosure subsets, including all-withheld; reject swapped slots, wrong count, wrong labels/types and digest mismatch. | W §§4.5.1–4.5.2 |
| CAP-012 | C | Do not fill withheld fields with zero, empty or default values. Structural signature/PoP use works without disclosures except the strongest-channel check, which becomes unavailable when its evidence is withheld. | W §§3.4, 4.5.2 |
| CAP-013 | C | At creation share the required disclosure set with the two participants; witnesses contribute their own corroboration without receiving the participant disclosures, and verifiers receive only authorized fuzzed profiles. Preserve the audience matrix in W §4.5.2 for every exchange. | W §4.5.2; L §1.5 |
| CAP-014 | C | Keep biometrics, precise coordinates, raw latency samples and secret seeds out of signed/network records, including opaque extensions emitted by the reference client. Receiver acceptance of a syntactic extension does not authorize the producer to leak secrets. | D §§7.2, 7.6–7.7, 19; L §1.3 |
| CAP-015 | O | Before interoperable capture tests, pin the canonical template encoding/length, modality and matcher versions, fuzzing algorithm and authenticated-encryption profile. Current fixed fixtures cannot settle these open choices by themselves. | D §§7.4.4, 7.5.2, 22.2; W §13; L §10 |

### Capture-profile validation target

| ID | Kind | Requirement and functional test oracle | Authority |
|---|---|---|---|
| CAP-016 | O | Evaluate the chosen fuzzing scheme against D §7.5's approximate 99% discrimination target while limiting the information yielded by repeated comparisons. Record dataset, extractor, modality, threshold and attack model; the prose's evidentiary-confidence comparison is not an executable legal or statistical threshold. | D §§7.4.1, 7.5, 20.1; L §1.3 |

### Recovery adoption, portability and local lineage choice

| ID | Kind | Requirement and functional test oracle | Authority |
|---|---|---|---|
| REC-001 | S | Recovery adoption names distinct old/new keys and binds the old hybrid successor proof to [prior_key,new_key,new_patron] under the successor domain. Reject swapped/omitted parties or an old proof replayed to another patron. | W §4.1; D §9.0.1 |
| REC-002 | S | Require 1–32 valid recovery responses from claimed prior counterparties with at least one match, unique and sorted by verifier, subject=new key and prior_key=old key. Each uses hybrid signatures and the recovery-specific consent/query linkage. | W §4.1 |
| REC-003 | C | Recovery requires both key possession and present in-person recognition; neither an old secret alone nor face recognition without the old-key proof suffices. If the old key is irretrievably lost, enter the fresh-identity/adoption path without inherited recovery standing. | D §§9.0.1, 9.1; W §4.1 |
| REC-004 | C | Personal recognition remains usable when old encrypted photographs have expired, with its basis labeled honestly and the human recognizing the person now. Do not synthesize a photo-match basis or automated human affirmation. | D §§9.0.1, 9.1; W §§4.1, 4.5 |
| REC-005 | C | Recovery changes the identity binding inside each observer's relevant horizon and terminates old sessions/currency/authorizations there. Test simultaneous old and new connections and future grant/submission attempts under the superseded key. | D §9.0.2; I §3 |
| REC-006 | C | Outside that horizon the two identities remain unrelated unless history is voluntarily presented. Resolution failure and currency no-issue do not reveal an old-to-new redirect, new locator or explanation of rotation. | D §§9.0.2, 12.6.2 |
| REC-007 | C | Repeated legitimate recovery heirs are independently verifiable evidence, not a global-consensus fork contest. Observer trust in the adopting patrons chooses its supported lineage; record the alternative instead of declaring both archives forged. | D §9.0.2 |
| REC-008 | P | Attribute misconduct to its actor and to the vouched claims/patrons as appropriate, not indiscriminately to every heir sharing earlier history. History import is weighted through the observer's recognized intersections rather than inherited as a global score. | D §§9.0.2, 16.7 |
| REC-009 | C | Rotate separately per subnet, preserving independent binding series and disclosure choices. Moving or abandoning an identity without inherited history must not silently invoke Recovery or link identities globally. | D §§9.0, 13.6; W §4.1 |
| REC-010 | C | Offer archive presentation and backup restoration as the second factor for portability; possession of only the signing key must not fabricate missing history. An unavailable prior patron does not gain a veto over unilateral departure or fresh adoption. | D §§9.3–9.4, 10.2, 13.7.1 |
| REC-011 | O | Implement the required local notification of competing recovery patrons without broadcasting a global recovery map. Exact notification transport/payload needs a documented profile; do not add a transaction type to obtain it. | D §9.0.2 |
| REC-012 | C | A thief holding only the subject key cannot reissue a series, which is countersigned. A stolen device composes key, archive and capture seeds: with a colluding counterparty nothing positive is forged, but both custody legs are theirs, so an adverse response becomes absence and the record finalises thin. Weigh a thin response set against the subject's own claimed count; do not report it as unqualified success. | D §18.3; W §§4.6, 5.5–5.6 |

### Asynchronous encryption, direct path and application payload

| ID | Kind | Requirement and functional test oracle | Authority |
|---|---|---|---|
| PAY-001 | C | Bootstrap end-to-end sessions asynchronously from PQXDH prekeys using X25519 and ML-KEM-768, binding encryption material to the authenticated hybrid identity while keeping signing keys separate from encryption keys. | D §§14.2.2–14.2.4; L §3 |
| PAY-002 | C | Support both one-time and last-resort initiation, making weaker initial forward secrecy visible when one-time material is absent. A serving node that substitutes prekeys must not silently impersonate the recipient. | D §14.2.4; W §7.8 |
| PAY-003 | O | The required ongoing construction is Triple Ratchet combining Double Ratchet and SPQR, not Double Ratchet alone. Test the implemented classical path separately, then require pinned upstream combined-construction vectors, binding and state transitions before claiming full completion. | D §14.2.4; L §3 |
| PAY-004 | E | For the selected encryption profile test tamper, wrong peer, stale/replayed messages, lost and reordered messages, skipped-key bounds, restart and state rollback. No rejected payload produces unauthenticated application plaintext or reuses consumed encryption state. | D §14.2.4; L §§2–3 |
| PAY-005 | C | Default to direct payload only after horizon eligibility and successful traversal for online parties; on traversal failure or out-of-horizon routing use serving-node relays. Anchor resolution nodes carry queries, not application payload merely because they were on the lookup path. | D §§12.6.3, 14.1.1; L §§3–5 |
| PAY-006 | C | Apply each user's direct/relay privacy choice in both send and receive directions before releasing endpoint candidates. Mutual horizon policy is the default, not permission to ignore an explicit user override or invent an absolute prohibition beyond it. | D §12.6.3; L §5 |
| PAY-007 | C | Discover reflexive candidates with STUN on the actual QUIC socket and exchange traversal candidates through authenticated end-to-end control. Test NAT mapping/filtering combinations, CGNAT failure and relay fallback without exposing an IP before the chosen admission policy. | D §14.1.1; L §3 |
| PAY-008 | C | Relay from sender's serving node to recipient's serving node and finally the recipient; keep queued/delivered payload encrypted end to end, including verifier profiles and capture grants. Hop TLS is not a substitute for that encryption. | D §§12.6.3, 14.2; L §3 |
| PAY-009 | C | A direct-path failure or disabled capability does not disable base messaging; report relay/degraded mode through the application facade. A relay refusal is delivered to the caller rather than silently counted as a sent message. | D §§14.1.1, 14.2; L §§3–4, 9 |
| PAY-010 | C | Group send is client-side fanout over the selected depth/horizon, with each recipient's independent end-to-end session. Do not promise a network group identity, atomic broadcast, global ordering or implicit read receipts. | D §14.3 |
| PAY-011 | O | Pin payload envelope/type demultiplexing, ratchet key-state serialization and upstream PQXDH hybrid-auth binding before independent interoperability claims. Temporary local adapters are not themselves the settled wire contract. | D §14.2.4; W §13; L §10 |

## 3.5 Observer-relative trust and policy evaluation

### Evidence graph, flow capacity, ordering and history import

| ID | Kind | Requirement and functional test oracle | Authority |
|---|---|---|---|
| POL-001 | P | Evaluate from a named observer's held evidence and own recognition, never from a purported globally complete graph or globally authoritative reputation score. The same records may legitimately produce different results for different observers. | D §§16.1, 17.1 |
| POL-002 | C | Preserve typed provenance for adoption, presence, peering, verifier answers, selection bases, recovery and reliability. A signature's validity, someone's personal recognition and a cached topology edge must not collapse into one undifferentiated fact. | D §§16.1–16.2.1; W §3.4 |
| POL-003 | P | In the reference graph, the observer's whole adoption horizon is distance one with horizon-internal transit collapsed; separately include its own directly known acquaintances. Internal parent paths must not add artificial hop decay or choke points. | D §16.2.1 |
| POL-004 | P | Sibling membership can place a node in the horizon, but a sibling relationship is not an additional trust-capacity edge. Test that materializing a sibling list or memo changes reachability data without manufacturing social trust. | D §§15.1, 16.2.1 |
| POL-005 | P | Collapse multiple relation kinds and repeated meetings between the same pair into one capacity connection. Repeated peering/adoption/PoP records for that pair cannot multiply its trust budget. | D §§16.2.1, 16.3 |
| POL-006 | P | Outside the observer's horizon, all admitted edge kinds use the specified common capacity treatment. Do not assign extra trust just because an edge is peering, infrastructure, expensive, continuously online or has a favorable ASN. | D §§16.2.1, 16.3, 16.6 |
| POL-007 | P | Visible peering evidence can expose its far endpoint only when the required near endpoint is in the observer's horizon. Learning H–G must not recursively crawl G–X merely because G has become known. | D §§16.2.1, 16.3.1 |
| POL-008 | P | Use only admitted evidence and bounded recognition expansion. Missing hidden edges cannot be guessed from the topology, a presented archive's length or the claim that strangers are well connected. Disconnected candidates receive zero unsupported flow. | D §§16.1–16.3, 16.7 |
| POL-009 | P | Compute individual reachability/flow and setwise shared-capacity allocation. For a fixed cut between the observer and an attacker region, adding arbitrarily many identities behind it must not increase total allocated capacity beyond that cut. | D §§16.2, 16.3.1, 17.3 |
| POL-010 | P | Capacity accounting is within an evaluation, not a durable balance consumed across unrelated later runs. Repeating the same evaluation from the same snapshot produces the same capacity result without exhausting a user's future standing. | D §16.2 |
| POL-011 | P | Allocate by available flow descending, then landscape distance ascending, then explicit consideration order for exact ties. Permuting hash-map/adjacency iteration must not change true ordering except where the caller's consideration order is the defined tie-break. | D §16.2 |
| POL-012 | P | Keep in-horizon membership/role access from accidentally spending an out-of-horizon shared cut. Distinguish that resource eligibility from the flow allocated when evaluating distant assertions. | D §§11.2, 16.2.1 |
| POL-013 | P | Weight an imported archive through trusted intersections and adopting vouchers, not number of records, age alone, cumulative past scores or arbitrary self-authored attestations. Presentation does not permanently graft every disclosed edge into the live trust graph. | D §§10.1, 16.7 |
| POL-014 | P | Recovery credit is limited by its corroborating/vouching paths, with no invented extra rotation discount. An actor's later offense cannot retroactively condemn independent heirs solely through a shared prefix. | D §§9.0.2, 16.7 |
| POL-015 | P | Decay affects current derived standing according to explicit local policy, not the immutable truth that an old signed meeting occurred. Test long inactivity and occasional genuine activity without silently deleting all old relations. | D §16.5 |
| POL-016 | P | Aggregate no-match evidence with the chosen negative threshold/inconclusive band and evidence provenance. A single cross-device mismatch is not automatically proof of fraud; unavailable is not a no-match. | D §§7.4.3–7.4.4, 16.1 |
| POL-017 | P | Keep infrastructure reliability separately usable for service choices while leaving social trust unchanged when only uptime, hardware, cloud cost or ASN metadata changes. Apparent provider diversity is not proof of independent control. | D §§3.3, 6.3, 16.6 |
| POL-018 | P | Permit other openly declared policies without labeling their differing scores structurally invalid. Test the reference policy's invariants separately from wire conformance and disclose when an alternative loses the reference Sybil bound. | D §16.4; D Appendix A.4 |
| POL-019 | E | For a decay alternative, show that λ < 1/f is necessary but not sufficient when acquaintance degree exceeds subordinate fanout. Include an 11-branch example with λ=.095 and a fixed-cut flow example rather than assuming f bounds every graph degree. | D §§16.2, 16.4, 21 |

### Selection-claim provenance

| ID | Kind | Requirement and functional test oracle | Authority |
|---|---|---|---|
| POL-020 | P | A verifier signature authenticates carriage of selection_basis but does not endorse that claim. Re-signing the same selector assertion must not increase its weight; evaluate it by the observer's own recognition of the selector and available evidence. | W §5.6; D §§8.1.2, 16.1 |
| POL-021 | P | A member of a disavowing patron's horizon defaults to that patron's determination and does not order a with-prejudice disavowal against a departure of the same party. A banded code yields no standing and no admission; an unbanded or absent one alleges nothing. Read the determination from the records held, not from whichever object closed the binding. A declared variant may order the pair instead. | D §§18.5, 16.4; W §4.3 |

## 3.6 Resource registration, discovery, credentials, requests and packages

### Resource identity, ownership, registration and catalog lifecycle

| ID | Kind | Requirement and functional test oracle | Authority |
|---|---|---|---|
| CAT-001 | C | A resource has its own key but is not a participant node: no subordinate slot, patron relationship, archive standing or social-flow contribution arises from creating one. Its owner is a participant distinct from a possibly different hosting node. | D §§11.0, 11.2; R §§4, 12 |
| CAT-002 | C | Allow an infrastructure owner to host locally and a light owner to register with appropriate infrastructure; evaluate access against the owner's horizon, not automatically the host's larger horizon. | D §§11.2, 11.5; R §12; I §11 |
| CAT-003 | S | CatalogEntry binds resource, owner, type, name, opaque endpoint, metadata, optional connect scope/data practice and owner signature under rhtn/1:catalog. Enforce byte limits and field conditions from W §6.1 without treating display name as a unique identifier. | W §6.1 |
| CAT-004 | S | Registration request type 7 carries entry, optional discover scope and nonce. Bind authenticated owner to the entry/signature; well-formed unauthorized/bad-signature/owner-conflict cases return refused, while malformed request encoding follows the defined stream reset rule. | W §6.2 |
| CAT-005 | C | Registering a valid broker-only entry does not require a running local package backend. Conversely a locally installed package need not publish a catalog entry; installation, registration and discovery are separate operations. | W §§6.2, 6.5; R §6 |
| CAT-006 | C | Hold only one current entry per resource key per host and one accepted owner there. Replacement is by valid applied registration, not a user-controlled timestamp ordering; delete superseded current catalog state rather than building a historical directory. | W §§6.5, 6.7; I §11 |
| CAT-007 | C | Conflicting ownership accepted at different hosts remains separately attributable; do not invent a global uniqueness registry. The unioning client must not merge unlike owners into a false single authoritative resource. | W §6.7 |
| CAT-008 | C | An absent discover scope means self on initial registration and preserves the existing requested scope on replacement; hosting policy may narrow scope. Exercise explicit changes and withdrawal/self-only behavior without promising control beyond the host's stated policy. | W §§6.2, 6.5–6.6 |
| CAT-009 | S | Scope forms are self, bounded down/up, siblings, Dunbar and an explicit sorted unique list of 1–256 keyhashes. Reject retired scope type 3, invalid arity/depth/type and oversized/duplicate lists. | W §6.6 |
| CAT-010 | C | A structurally valid scope that the evaluator cannot compute is ineffective, never an authorization grant by default. Resource membership is still the outer gate for named/explicit assignments. | W §6.8; D §11.4; R §7.1.1 |
| CAT-011 | C | Catalog entries and registrations are current private service state, not archive transactions or topology floods. Do not advance a participant archive or disseminate resource membership through TopologyPush. | D §§10.0, 11.5; W §§6, 10 |

### Catalog querying, pagination, privacy and user presentation

| ID | Kind | Requirement and functional test oracle | Authority |
|---|---|---|---|
| CAT-012 | S | CatalogQuery type 5 contains optional 1–64-byte type filter and nonce; compare the type byte-exactly without case folding or Unicode normalization. Reject malformed unsigned fields and mismatched response nonce. | W §6.4 |
| CAT-013 | C | Answer only for the node and the clients it serves, applying the requested type, owner membership and permitted discover scopes. An outsider's refused stream is distinct from a successful empty result. | W §6.4; I §11 |
| CAT-014 | S | Sort CatalogReply by resource then owner, limit to 111 entries and obey the overall frame ceiling. Include a continuation hint only when entries were omitted and name the first omitted type as specified. | W §§1.3, 6.4 |
| CAT-015 | C | A continuation is a type-filter hint, not a generic offset token. If a full repeated type remains truncated, mark incomplete and stop looping; do not falsely promise an exhaustive unbounded type result. | W §6.4; L §8 |
| CAT-016 | C | On joining, periodically, after relevant connect failure and on manual refresh, sweep the user's horizon and union eligible answers, including entries hosted for another owner. Cache one sweep for local browsing instead of issuing a query on every UI action. | D §11.5; L §8 |
| CAT-017 | C | Treat one host's refusal, timeout or truncated answer as that host's partial/stale state; do not erase valid answers from other hosts or claim an incomplete sweep is globally complete. | L §8; I §11 |
| CAT-018 | C | Give catalog caches explicit expiry and mark stale results. A cached discoverable entry never grants connect permission, and an omitted connect scope means unknown rather than unconditional allow/deny. | W §§6.1, 6.5; D §12.5; L §8 |
| CAT-019 | C | Display owner identity and claimed data practice before use, distinguish absent declaration from explicit unstated and unknown future codes, and do not certify a self-reported no-log claim. | W §6.1; L §§6, 8; R §7.4 |
| CAT-020 | C | Show only the accessing user's assigned roles/available services, not other principals' role rows or hidden eligibility predicates. Keep operator configuration and user catalog presentation as different interfaces. | R §7.4; I §10.7; L §8 |
| CAT-021 | C | Service unavailability does not silently transfer ownership or broaden discovery. Retain appropriate stale information for explanation while a host is down and re-resolve/requery according to policy. | D §11.5.1; L §8 |
| CAT-022 | S | Every reply echoes the nonce of the request that drew it, and an asker refuses one that does not without reading anything out of it. Cover registration, catalog, prekey, currency, archive and submission replies alike; a refused reply leaves its query outstanding. | W §§6.2, 6.4, 7.1, 7.8–7.10 |

### Membership gates, subtree acknowledgements and role tables

| ID | Kind | Requirement and functional test oracle | Authority |
|---|---|---|---|
| GAT-001 | C | Resource access always starts with membership in the owner's current horizon. A named assignment, broad scope, high trust score or host relationship cannot grant access from outside it; movement immediately affects future authorization. | D §§11.2, 11.4; I §10.1; R §7.1.1 |
| GAT-002 | S | Verify SubtreeAck as a standalone classical signed object under rhtn/1:subtree-ack, bound to adoption txid, admitted identity, issuer and timestamp. It is not a third adoption envelope signature. | W §7.5; D §11.2.1 |
| GAT-003 | C | Before positional access above the relevant patron, require the matching acknowledgement from the appropriate current grandpatron/accepted authority according to policy. Wrong adoption, stale relationship or unrelated issuer must not unlock access. | D §11.2.1; I §10.1 |
| GAT-004 | C | Issue acknowledgements automatically under standing policy; lack of one blocks the relevant resource access, not adoption validity or the existence of a relationship. Remove its effect when the relationship ends without inventing a revocation transaction. | D §11.2.1; W §7.5; I §10.1 |
| GAT-005 | P | A local policy may accept another trusted acknowledgement where the design permits; individual named assignments follow their specified exemption from positional acknowledgment but remain inside the owner-membership gate. Test positional and named paths separately. | D §11.2.1; I §10.1; R §7.1.2 |
| GAT-006 | C | Materialize roles per (resource, principal/member) from current configuration, membership and evidence; request processing reads a row, not arbitrary live predicate execution. Build a consistent snapshot before issuing credentials. | I §10.2; R §7.2.1 |
| GAT-007 | C | Support declared structural depth/tier, tenure/date, named assignment, absolute rank/top-k and relative quantile predicates. Do not expose raw trust-score thresholds as the reference role authoring model. | I §10.3; R §§7.1–7.2 |
| GAT-008 | C | Predicates and templates make their population explicit; show its name and current member count before binding. Users can inspect/edit templates rather than installing opaque code that secretly grants roles. | I §10.4; R §§7.2.1, 7.3 |
| GAT-009 | C | Recompute on evidence, membership and configuration changes and on scheduled time boundaries for tenure/expiry. Relative predicates reevaluate their population; absolute-rank updates must also remove displaced members. | I §§10.2–10.3; R §7.2.1 |
| GAT-010 | C | Named exceptions are per resource, visible and optionally time-limited; expiry removes the role on time even without another incoming network message. A departed named member cannot keep access through an exception. | I §§10.1–10.3; R §7.1.2 |
| GAT-011 | C | Reject undeclared package roles and overbound role configurations rather than silently granting arbitrary labels. Credential application-role sets have at most 64 distinct lowercase tokens of 1–32 ASCII letters/digits/underscore/hyphen; discover/connect are gateway gates, not emitted app roles. | W §11; I §10; R §§2–3, 7 |
| GAT-012 | C | Owner movement preserves its own downline resource relationships where membership still holds, removes old upline eligibility and admits new eligible neighbors. Hosting migration is separate and may temporarily leave a light owner's resource unavailable. | D §11.2.2; R §12 |
| GAT-013 | C | Any role-set change retires that principal's resource session, including additions and removals; next request gets fresh authorization and a new resource session ID. Do not mutate privileges inside the old session or close unrelated QUIC/resource sessions. | I §10.5; R §7; D §11.4 |
| GAT-014 | C | A membership or role change during an in-flight hosted request lets that request complete under its entry snapshot, retires the resource-facing session so the next request is evaluated afresh, and leaves the transport connection untouched. Assert all three. | I §§10.1, 10.5 |
| GAT-015 | C | For a brokered external service, prevent future establishment when membership/roles are lost, but do not claim the network can revoke an already established vendor session. Disclose that limitation at first use. | D §§11.2, 19.4; I §10.5; L §6 |

### Pairwise credentials and gateway request evaluation

| ID | Kind | Requirement and functional test oracle | Authority |
|---|---|---|---|
| GAT-016 | S | Derive principal as SHA-256(rhtn/1:pairwise plus raw resource keyhash plus raw user keyhash), with no separators or display encodings substituted. It is stable for that pair across hosts/migration and different across resources or users. | D §11.0.2; R §2 |
| GAT-017 | C | Present only pairwise principal, resource audience, assigned application roles and opaque session to a resource. Do not provide raw participant keyhash, archive, social graph, role predicates, liveness or verifier evidence. | D §§11.0.2–11.0.4; R §§2, 9 |
| GAT-018 | C | Resource credentials are audience-bound and cannot cross the boundary twice. Reject another resource's credential and do not let a package act upstream as the user; any onward call uses its separately configured resource authority. | D §11.0.4; R §§2, 10–13 |
| GAT-019 | S | ResourceRequest type 6 carries its addressed resource and serialized HTTP/1.1 bytes on a fresh bidirectional stream over rhtn/1. Do not substitute HTTP/3 ALPN or require the caller to attach to the hosting node. | W §§9.2, 11–11.1; R §3 |
| GAT-020 | C | Evaluate the ordered gateway ladder on one consistent snapshot: decode body → existing binding → owner membership → subtree acknowledgement where required → connect row → valid HTTP → running backend. Exercise each pair of simultaneous failures to verify the earlier decision wins. | W §11 |
| GAT-021 | S | Return code 1 for absent resource or out-of-owner-membership, 4 for missing required acknowledgement, 5 for denied connect, 3 for malformed body/HTTP and 2 for authorized but unavailable backend. Do not let HTTP parsing/backend probing reveal existence before earlier gates. | W §11 |
| GAT-022 | S | Success code 0 carries the serialized HTTP response, even an application-level HTTP error. A nonzero gateway error is one final bodyless reply; split success across bounded frames as specified and never concatenate multiple HTTP messages. | W §11; R §3 |
| GAT-023 | C | Do not automatically retry a failed application/backend request: it may already have side effects. Report unavailable versus HTTP application failure without returning a misleading success or duplicating a write. | W §11; R §3 |
| GAT-024 | S | Trusted headers encode principal and audience as unpadded base64url of 32 bytes, roles as a comma-separated deduplicated set, and session as an opaque resource-specific value. Accept an empty roles field and do not prescribe an undocumented session width. | W §11; R §3 |
| GAT-025 | C | A session ID identifies the principal's resource session, not the QUIC connection or a network-wide user session. Simultaneous resources have separate IDs; role retirement for one leaves the other's state intact. | I §10.5; R §§2–3 |
| GAT-026 | C | Do not expose topology, eligibility predicates or detailed membership failure through reply metadata. Code indistinguishability for missing/outside-scope is required; a constant-time implementation is not specified or claimed by this test. | W §11; R §§1–3 |

### HTTP parsing, header integrity and external gateways

| ID | Kind | Requirement and functional test oracle | Authority |
|---|---|---|---|
| GAT-027 | C | Parse one complete HTTP/1.1 request before rebuilding a sanitized backend request. Reject request smuggling forms: duplicate/conflicting lengths, ambiguous Transfer-Encoding/Content-Length, invalid chunking, malformed request line, obsolete folding, premature end and trailing second messages. | W §11.2; R §§3, 3.1 |
| GAT-028 | C | Reject unsupported CONNECT, Upgrade and Expect: 100-continue handling at this interface rather than opening a tunnel or an interim-message path around authorization. | W §11.2; R §3 |
| GAT-029 | C | Strip every caller-supplied RHTN-* header case-insensitively, including duplicate/mixed-case variants, before inserting authenticated values. A nonconforming backend must never see attacker-chosen credential headers. | W §11.2; R §3.1 |
| GAT-030 | C | Route solely by the selected resource binding. Ignore caller Host/absolute-target authority as a destination selector, normalize to the correct origin request and configured backend Host, and reject syntax that cannot be reconstructed unambiguously. | W §11.2; R §3 |
| GAT-031 | C | Apply credential parsing without depending on header order, capitalization or role ordering. Applications enforce their declared role vocabulary and audience rather than trusting a display string as authority. | R §§2–3 |
| GAT-032 | C | Require TLS for the cross-network HTTP leg to an external backend; plain HTTP is permitted only for the specified local hosting boundary. Preserve the distinction between encrypted network transport and a deliberately local backend channel. | R §3; I §9 |
| GAT-033 | C | For brokered authentication, hand off to client-to-vendor TLS by default so the node does not carry application content. If proxying is intentionally selected, show that different hosting/privacy model before binding/use. | R §§5.2, 11; I §10.6; D §11.7 |
| GAT-034 | C | Validate the configured broker/service endpoint against the owner-signed resource entry and configured authority. Do not follow arbitrary caller-supplied destinations or imply that a signed catalog listing certifies the vendor. | R §§3, 11; W §6.1 |
| GAT-035 | O | Standard OIDC/SAML/IdP adapters are planned integration paths, not permission to invent claims or bypass the gateway. Each selected adapter must document audience, role mapping, session lifetime and external-consumer limitations before its conformance tests are executable. | R §§5, 10–13 |
| GAT-036 | C | An ending removes rows and sessions for the departed party and for everything that reached the owner's horizon only through it, on every bound resource including those carrying no standing grant. Membership is the outer gate whoever wrote the row; do not enumerate a subtree to find them. | I §10.2; D §11.4 |
| GAT-037 | C | Distinguish a row a standing grant produced from one an operator set for a named member. Replacing the grant rewrites what it wrote and retires those sessions; the operator's own row is untouched and other resources are undisturbed. Re-applying an unchanged grant disturbs nothing. | I §§10.2, 10.5 |

### Package admission, isolation, federation and abuse reporting

| ID | Kind | Requirement and functional test oracle | Authority |
|---|---|---|---|
| PKG-001 | C | Package manifests declare needed compute/storage/hardware/address/client capabilities, application roles and inspectable role templates. Admission checks support/capacity explicitly; refusing unsupported resources is not a base-network conformance failure. | R §§5, 8; I §§9–9.1 |
| PKG-002 | C | Keep package role requests subordinate to the owner's declared role configuration. No package may invent a new privileged gateway role, expand its owner horizon or silently install an uninspectable predicate. | R §§7.3, 8; I §§9, 10.4 |
| PKG-003 | C | The package API exposes only its credential-bearing request traffic and response facilities. There are no host hooks for topology, archive fetch, transaction signing, queues, prekey requests, liveness or role-evaluation inputs, even with apparently narrow scopes. | R §9; I §9.2 |
| PKG-004 | E | For the implemented Wasmtime sandbox, exercise valid component admission, missing handle entry, unknown imports and ambient/WASI access attempts. Denied imports remain unavailable at runtime; request state contains no privileged node handles. | R §9; I §9.2; crates/resources/src/lib.rs |
| PKG-005 | E | Exhaust guest fuel, memory, table and response bounds; trap and misuse host bindings. The request fails within configured limits without killing node service, leaking another request's credential or issuing an unbounded response. Numeric defaults are local choices, not wire constants. | R §9; I §9.2; crates/resources/src/lib.rs |
| PKG-006 | E | Run hostile packages beside an active node and unrelated resources, test concurrent/sequential isolation and backend failure mapping, and inspect filesystem/network/host-binding exposure. Passing an import allowlist test alone does not prove full sandbox containment. | D §§18.2, 19.4, 20.2; I §9.2 |
| PKG-007 | O | Package format, publisher provenance, update channels, dependency compatibility and installation policy need concrete profiles. Test signed provenance or reproducible distribution only when the chosen scheme is specified; do not label the current host ABI a settled portable package ecosystem. | R §8; I §§9.1, 12 |
| PKG-008 | C | Conforming extension support does not promise arbitrary native-program/device-driver support. State unmet hardware, anti-cheat, vendor-login, token-binding and client-fingerprint dependencies rather than pretending every application can be transparently ported. | R §§5–5.2 |
| PKG-009 | C | Federation composes separately owned, horizon-bounded local resource instances; it does not expand one resource into a global scope or expose network evidence to the federated application. | D §11.0.1; R §4.1 |
| PKG-010 | S | AbuseReport is a resource-signed standalone object with category 0–5 and optional detail bounded at 1024 bytes. Validate the resource key/domain and category; it is not a participant-signed archive transaction. | W §6.3; D §11.6 |
| PKG-011 | C | Deliver abuse reports privately to the relevant owner for local action. Do not flood them, add resource assertions to social trust, or append them to identity archives; application detail may identify a complainant but there is no extra protocol reporter identity field. | D §§11.0.3, 11.6; W §6.3 |
| PKG-012 | C | A local application-report bridge may submit the resource's bounded report, but must not hand a package privileged participant signing keys or general network transaction primitives. Receipt does not imply an automatic punishment or an unspecified report-response protocol. | R §9; I §9.2; W §6.3 |

### Resource-side session and portability behavior

| ID | Kind | Requirement and functional test oracle | Authority |
|---|---|---|---|
| PKG-013 | C | A resource tolerates a hosted session ending and subsequent requests under a new opaque ID; it must not keep treating the retired ID as a continuing source of roles. This does not invent a separate gateway teardown notification protocol. | I §10.5; R §§2–3 |
| PKG-014 | C | A conforming portable package consumes the declared credential/request contract without requiring a particular node vendor or direct network-state access. Exercise it on another conforming host/profile; unsupported capabilities cause explicit admission refusal rather than bypassing isolation. | R §§5, 8–9; I §9 |

## 3.7 Application boundary, mobile product, storage and operator tools

### Single kernel and platform integration

| ID | Kind | Requirement and functional test oracle | Authority |
|---|---|---|---|
| APP-001 | C | Keep identity, private keys, archive, horizon, sessions, all routine protocol encoding/decoding and network I/O inside one participant kernel. Continuous infrastructure and intermittent/mobile operation use that same protocol behavior. | D §14.1.0; L §9 |
| APP-002 | C | Application shell calls typed operations and receives verdicts, distances, modes, decrypted messages with authenticated attribution, and explicit refusal reasons. It does not routinely dial, hold sockets, route ciphertext or parse/sign protocol frames. | L §9 |
| APP-003 | C | Permit only named opaque pass-through interactions needed for platform transport, including optical display/camera exchange and specified import/entropy paths. The shell transfers bytes without interpreting cryptographic structure; this exception is not a generic raw-protocol API. | D §14.1.0; L §9 |
| APP-004 | C | Hardware flows inward through camera, proximity, clock, randomness and user-consent interfaces. The shell returns what the hardware/person actually supplied, including errors and unavailable capabilities, rather than fabricating successful consent or measurements. | L §§1.3, 9 |
| APP-005 | C | Keep checks in the kernel rather than only the FFI facade so alternate frontends get the same security decisions. Foreign bindings translate values and preserve errors without truncating IDs, unsigned counters, byte lengths or optional absence. | D §14.1.0; L §9 |
| APP-006 | C | Provide typed refusal values for missing sessions, malformed local arguments, rejected endpoints and remote refusal. The application need not infer success/failure from silence or inspect a raw reply to display the outcome. | L §9 |
| APP-007 | C | Accept a user-selected external wake endpoint/key through the platform into the kernel; acquisition and service account ownership stay outside it. Do not secretly register with a vendor merely because the participant started. | L §§4.1, 9; D §14.1.5 |
| APP-008 | C | On mobile suspension/offline termination retain the persistent identity, archive and obligations needed for later reconnect; perform background work only within OS capabilities. Foreground operation remains useful with no push permission. | D §§14.1.0, 14.1.4–14.1.6; L §§2, 4 |
| APP-009 | O | Supply Android camera/radio/keystore/lifecycle and iOS equivalents plus generated bindings, then test on actual supported devices. Rust facade/instrument tests cannot satisfy product consent, camera, OS wake or durable backup claims. | L §§1, 2, 4, 9–10; D §§14.1.4, 24 |
| APP-010 | C | A device holding a delegation and no seed starts from the identity's key material, a transport seed of its own and the run the ceremony device signed; it attaches under the delegation, is a payload endpoint of its own with material it generated, and refuses by name every act of the identity key: consent, a verifier's answer, a body, witness and departure signatures, a recovery response, and delegation issuance. Refuse a start with no delegation or with a delegation by another identity. | D §23.3; W §§7.8, 8.2; L §§3, 4.2 |

### Human authorization, privacy choices and warnings

| ID | Kind | Requirement and functional test oracle | Authority |
|---|---|---|---|
| UX-001 | C | At ceremony capture time tell both people what identity, timing, relationship and disclosed fields become durable and who may later read them. The notice appears at the action, not only in terms of service or an after-the-fact log. | L §1.5; D §19.6 |
| UX-002 | C | At capture, disclose to the participant what the record will contain and who will be able to read it. Assert that **no** per-query notice is given to a witness or a verifier: both roles are automatic and owed no warning. Absence of a user notice is not detectable by a wire validator. | D §§19.6, Appendix A.3 |
| UX-003 | C | Warn about missing familiar verifiers, witness imbalance, unavailable evidence and weak proximity/integrity before signing, without presenting a legitimate degraded ceremony as cryptographically malformed. | L §§1.1–1.4; D §§7.6.3, 13.5 |
| UX-004 | C | Expose independent direct-versus-relay choices for inbound and outbound traffic and explain peer-IP disclosure versus serving-node communication-graph disclosure. Verify the choice changes actual routing, not only a UI preference. | L §5; D §§12.6.3, 19.4 |
| UX-005 | C | Encourage a second independent adoption early, while reachable, and explain peerless/infrastructure concentration exposure where relevant. Do not force it as an extra wire admission requirement. | L §6; D §§13.3, 18.4 |
| UX-006 | C | Before any user-initiated authority change, compute and warn about loss of currently used resource access; include transfers, disavowals/other applicable changes, not only departure. A stale permission list must be identified rather than used to promise continued access. | L §6; D §11.2 |
| UX-007 | C | Before accepting a new patron list the user's resources with upward-reaching policies and the newly affected people. Show expanded names and counts; a count alone is insufficient for confirming a role binding. | L §§6, 8; I §10.4 |
| UX-008 | C | At identity choice, warn that an unlinked identity abandons prior standing or that a linked recovery sacrifices unlinkability. Explain capture-key withholding/deletion and backup loss costs before irreversible action. | L §§2, 6; D §§9, 13.7 |
| UX-009 | C | For role configuration show names as the primary view and, for quantiles, the named population and moving cutoff; offer absolute top-k where that matches intent. Show actual current roles to users and filter unusable services rather than exposing hidden policy internals. | L §8; R §§7.2–7.4 |
| UX-010 | C | On first brokered resource use show owner, signed-service binding, hosting/proxy model and the fact a vendor session can outlive membership. Do not imply owner-signed metadata proves no logging or trustworthy external conduct. | L §8; I §10.6; R §§10–12 |
| UX-011 | C | Provide deliberate catalog refresh and per-host stale/truncated indicators. Browsing cached items must open no connection; execute a sweep with at most one active query session at a time and close it after use. | L §8 |
| UX-012 | C | Show degraded/failover mode and local mailbox-count meaning, actual message sender attribution, missing evidence and refusal reason. Never display a hint as authenticated identity or a submission acknowledgement as read/delivery confirmation. | L §§4, 9; W §§7.10, 8.2 |
| UX-013 | E | Validate notices and choices by walking the real application on each supported platform, including cancellation, denial of camera/radio/push permission and screen-reader/visible-content inspection as supported. Command-line automation is evidence of kernel behavior, not of a human seeing/understanding a warning. | L §§1.5, 5–6, 8–9; D §19.6 |

### Endpoint identity presentation

| ID | Kind | Requirement and functional test oracle | Authority |
|---|---|---|---|
| UX-014 | C | Show whether the resolved endpoint is the infrastructure operator itself (empty residual path, the .0/actual presentation) or a person reached behind it. Do not encode or infer a permanent infra/light type from the locator, because promotion can happen without movement. | L §4; D §12.6.1 |

### Persistent participant state, backup, startup and operator control

| ID | Kind | Requirement and functional test oracle | Authority |
|---|---|---|---|
| OPS-001 | C | Persist identity keys, retained archive, capture seeds and retained sealed captures with independent expiry semantics. A client can stop between interactions and resume without silently becoming a different participant or losing acknowledged archive obligations. | L §2; D §§10.2, 13.7.1, 14.1.0 |
| OPS-002 | C | Encrypt archive backup with a random data-encryption key and authenticated encryption; derive the wrapping key from the user's passphrase with Argon2id. Store the wrapped key with the blob, not the unwrapped wrapping/data key in an equivalent plaintext sidecar. | D §13.7.1; L §2 |
| OPS-003 | C | Make key wrapping replaceable independently of archive encryption so future recovery mechanisms can be added without changing the archive format's entire content model. Do not implement unspecified threshold shares as a claimed v1 requirement. | D §13.7.1 |
| OPS-004 | E | Test wrong passphrase, tampered ciphertext/header/wrapped key, truncated backup and interrupted restore: fail without silently importing partial authoritative state or replacing an intact identity. Verify successful export/import recovers the intended subject and retained heads. | D §§10.2, 13.7.1; L §2 |
| OPS-005 | C | Backup and restore scan all retained archive/seed/capture material, including multiple ceremonies and merge branches. Expired captures are deleted under retention rules rather than resurrected by a bulk copy; source backup/key availability remains explicit. | D §§7.5.2, 13.7.1; L §2 |
| OPS-006 | C | Require/support a usable backup for an evictable host and explain the archive's second-factor role; a conventional cloud file sync is not automatically a correct encrypted backup/expiry implementation. | D §§10.2, 13.7.1, 18.1 |
| OPS-007 | O | Do not promise cross-device deletion, seed rederivation after incomplete recovery or automatic archive retrieval from an attacker-accessible service. Document which device/store holds each class and keep unsettled multi-device/restore behavior blocked. | D §§22.2, 23.3; L §10 |
| OPS-008 | E | Daemon and participant instrument load the explicitly supplied identity and validate its material; missing/corrupt state fails visibly instead of automatically minting a new identity under the same operator configuration. Key creation remains an explicit tool/user operation. | D §§5.1, 10.2, 14.1.0; crates/daemon/src/config.rs; crates/participant/src/lib.rs |
| OPS-009 | E | Start the continuous service from validated configuration and authoritative durable state before admitting requests; replay topology, queue and prekeys consistently. Test a failed startup/restart without silently accepting requests under an empty replacement state. | I §§1–7; D §14.1.0 |
| OPS-010 | E | Persistence failures propagate through mutation/submission results and operational status. Inject disk-full/failed-write/crash boundaries around archive heads, current endpoints, queue custody, prekeys and role cache publication; success must not conceal lost acknowledged obligations. | I §§2, 4, 6, 10; L §2 |
| OPS-011 | C | Operator status displays read-only state summaries, identities/counts/times and actual availability. The status interface does not provide arbitrary frame composition, traffic replay, signature creation or manual packet approval controls. | I §§8–8.1 |
| OPS-012 | C | Keep operator configuration of its own hosting, predicates and standing policies as explicit management acts, distinct from traffic bookkeeping. Running infra also requires the ordinary participant client for its own human ceremonies. | I §8.1; D Appendix A.2–A.3 |
| OPS-013 | C | Disclose what infrastructure can see: routing/relationship/queue metadata, hosting content where proxied, and provider/peering concentration. Do not claim end-to-end content access where encryption prevents it or anonymity that rootward indexing does not provide. | I §8; D §§18.1–18.2, 19 |
| OPS-014 | C | Store and display reliability observations separately from social standing, bound audit/cache histories as specified, and expire non-authoritative contact/session/capability history explicitly. Diagnostic logging must not recreate prohibited durable verifier/communication logs. | D §§16.6, 19.4–19.5; L §4; I §§2, 6, 8 |
| OPS-015 | O | Encrypted cross-tree peer backup/audit needs its concrete storage, challenge and recovery profile. Test declared commitments and bounded audit evidence now; challenge success is availability evidence, not proof that a peer permanently possesses an independent copy. | D §§3.4, 6.3; W §4.4; I §12 |
| OPS-016 | C | An upgrade removes durable state a prior version wrote that the current rules forbid, not only stopping the write. Name the artifacts, remove them on load with the failure visible, and leave unrelated operator files alone. A rule satisfied only for state created after the upgrade is not satisfied. | I §§4, 6; D §19.4 |
| OPS-017 | P | Run both repair paths §10.1.3 names: the replay on a new adjacency, and the periodic replay to siblings and the patron. The interval is an operator parameter with a stated default and bound, and a setting that turns it off is a policy choice rather than a refusal. Neither path is a mechanism beyond propagation: the same frames, suppressed as duplicates where already held. | W §§10.1.2–10.1.3; D §21.1 |
| OPS-018 | C | The retention scan runs on import and not over live data, covering device migration and manual copies as well as restore. A capture expires on its subject's declared window rather than its holder's; records, disclosure sets, the archive and the identity survive it. What was discarded is reported rather than swallowed. | D §§13.7.1, 7.5.1; L §2 |

### CLI, participant instrument and test-support integration

| ID | Kind | Requirement and functional test oracle | Authority |
|---|---|---|---|
| TOOL-001 | E | CLI inspection uses the shared canonical parser and verifier, reports missing material separately from invalid signatures, and preserves transaction IDs and full-width fields. Inspection does not mutate archive/topology state. | W §§1–3; crates/cli/src/inspect.rs |
| TOOL-002 | E | Key tooling explicitly mints/imports/exports the intended hybrid identity with validated lengths and secure output handling. Public inspection does not accidentally print secret seeds; this is an implementation safeguard supporting the specified identity boundary. | D §5.1; crates/cli/src/keys.rs |
| TOOL-003 | E | Read-only probes for resolution, archive and catalog bind replies to requests and show refusal, incomplete history and truncation honestly. A probe must not make a registration, spend a prekey or attach merely to inspect an unrelated read-only service. | W §§6.4, 7.7, 7.9, 9.2; crates/cli/src/probe.rs |
| TOOL-004 | E | Drive rhtnp through supplied identity and pins, ceremony/adoption/message operations and typed output. Validate malformed commands, failed connections and explicit stop without treating stdin input as proof of human consent. | L §9; crates/participant/src/{lib,terminal,carry}.rs |
| TOOL-005 | E | Declare the instrument's ephemeral-state limit in any experiment result. Restarting it does not test a shipping client's durable archive, retention, backup or mobile lifecycle, even if the real-socket scenario otherwise succeeds. | D §§10.2, 14.1.0; crates/participant/src/lib.rs |
| TOOL-006 | E | Simulation controls loss, duplication, delay, partition, NAT, node churn, process termination and hostile packages independently of protocol decisions. Repeat seeded scenarios and retain inputs/results so a reported success can be reproduced. | D §§20, 23.4; crates/sim/src/ |
| TOOL-007 | E | Acceptance results map to this document's IDs and actual assertions. Generated stubs, empty scenario bodies, self-roundtrip vectors and capability markers cannot be counted as independently verified functional coverage. | D §23.4; crates/acceptance/ |

## 4. Wire-schema and dispatch coverage ledger

The following ledger makes the common field-by-field mutation suite in §1 mandatory for **every named schema**, including nested maps that do not have a network request of their own. Fields listed here identify coverage targets; the cited W schema remains the source for numeric keys and exact presence conditions. Do not implement a second, simplified wire specification from this summary.

### Primitive, transaction and evidence schema coverage

| ID | Kind | Requirement and functional test oracle | Authority |
|---|---|---|---|
| SCH-001 | S | Cover keyhash/txid/query ID/genesis hash (32-byte values), timestamp (u64), seqno {series,counter} (u32 each), path, NetworkPoint, COSE_Key and KeyMaterial; test raw-byte lexicographic ordering rather than text rendering order. | W §§1–2, 4.4 |
| SCH-002 | S | Cover Locator {anchor,path,seqno} and SignedLocator {subject,locator,signature}; a bare locator is allowed inside its authenticated transaction but rejected as a standalone introduction. | W §2.3 |
| SCH-003 | S | Cover Envelope version/type/body/signature fields and common body key 0; transaction types 1,2,3,4,5,7 only, with retired 6 rejected. All two-party types reject equal parties, including one-signer departure and disavowal. | W §§3–4.1 |
| SCH-004 | S | Cover Adoption node/patron/locator/time/optional new-node material/head and exclusive evidence choice, nested Transfer and Recovery, and detached SuccessorStatement and TransferStatement. Recovery additionally requires verifier≠subject, querier=verifier where query is held, selection_basis=0 and at least one match among its responses. | W §4.1 |
| SCH-005 | S | Cover Departure departing node/patron/new sequence/time/reason and Disavowal patron/subject/time/reason in their exact schemas, without importing an unsupported subject sequence into disavowal. | W §§4.2–4.3 |
| SCH-006 | S | Cover Peering parties/time/PoP, its two NetworkPoints, optional backup commitment and Audit entries; cover SeriesReissue node/patron/old sequence/new sequence/time without inventing a locator move. | W §§4.4, 4.6 |
| SCH-007 | S | Cover Presence start/finalization/participants/witnesses/responses/subtype/disclosure root; Participant keyhash; Witness identity/nominator/observation bits. Enforce conditional array absence and retired body/witness keys independently of extension handling. | W §§3.2, 4.5 |
| SCH-008 | S | Cover ClientIntegrity bool/scheme/evidence; Proximity channels/strongest; Channel kind/outcome/resolution/binding; Capture modality/count/liveness/algorithm; LocationEvidence assertions/corroborations; Asserted method/geohash; Corroboration witness/method/radius. Required empty location lists remain legal. | W §4.5 |
| SCH-009 | S | Cover Disclosure salt/label/value, seven DisclosureSlots and PresentedRecord. Retention disclosure is an unsigned number of years, not a timestamp or silently narrowed enum. Require fresh salts at production and exact root verification at receipt. | W §4.5.1 |
| SCH-010 | S | Cover VerificationQuery subject/querier/ceremony/fuzzed profile/template version/query ID/addressed verifier, VerifierResponse parties/result/basis/version/query ID/consent/prior key/selection basis/signature, and their ordinary versus recovery conditions. The query itself is not embedded in the signed presence body. | W §§4.5, 5.6 |
| SCH-011 | S | Cover KeyGrant txid/query ID/key and LateResponse txid/subject/response; do not add an envelope or a general anonymous assertion channel. Validate consent only to the extent the holder possesses the required query/ceremony evidence. | W §§7.3–7.4 |

### Service-object, session and resource schema coverage

| ID | Kind | Requirement and functional test oracle | Authority |
|---|---|---|---|
| SCH-012 | S | Cover Scope's scalar/tagged alternatives and CatalogEntry resource/owner/type/name/endpoint/connect scope/metadata/data practice/signature. Metadata SHOULD avoid duplicating endpoint data, but semantic duplication of an opaque blob is not a structural reject rule. | W §§6.1, 6.6 |
| SCH-013 | S | Cover ResourceRegistration entry/discover scope/nonce, its recorded/refused reply, CatalogQuery filter/nonce, CatalogReply nonce/entries/continuation, and AbuseReport resource/time/category/detail/signature. Retain unknown data_practice values without converting them into no-log. | W §§6.1–6.4 |
| SCH-014 | S | Cover CurrencyAttestation subject/current key/issued/expires/role/issuer/signature and CurrencyRequest subject/nonce, CurrencyReply nonce/status/conditional attestation. No reply field may identify a separate querier or a successor-lookup explanation. | W §7.1 |
| SCH-015 | S | Cover AnchorEntry keyhash/ordered endpoints/u64 subtree size/sequence/signature; EndpointRecord subject/sequence/endpoints/signature; SubtreeAck adoption/grandpatron/admitted node/time/signature. Signed unknown fields participate in each map's signature. | W §§7.2, 7.5–7.6 |
| SCH-016 | S | Cover ResolveRequest anchor/path/nonce and ResolveReply nonce/result/conditional Referral or ServingInfra or failure; cover every nested referral prefix/authority/endpoints and terminal identity/material/path/sequence field as defined. Mutually exclusive reply alternatives must not coexist. | W §7.7.3 |
| SCH-017 | S | Cover PrekeyBundle subject/construction/reusable blob/published time/signature; PrekeyRequest subject/mode/nonce; PrekeyBatchRequest sorted population/nonce; PrekeyReply nonce/conditional bundle/one-time key/failure. Failure codes apply iff bundle absent and one-time key never appears on a reusable-only sweep. | W §7.8 |
| SCH-018 | S | Cover ArchiveRequest subject / optional frontier / count / age stop / nonce, and ArchiveReply nonce / records / more / returned frontier. An empty required record list is legal; absence of required more/nonce is not. A record is a `PresentedRecord` for a presence record and a bare `Envelope` otherwise. | W §7.9 |
| SCH-019 | S | Cover PrekeyPublication bundle/nonce, OneTimeDeposit opaque keys/nonce, RelaySubmission recipient/ciphertext/nonce, WakeRegistration nonce/conditional endpoint/key/expiry, SubmissionReply nonce/code, and the exact two-element RelayedPayload array. | W §7.10 |
| SCH-020 | S | Cover Capabilities, Attach identity/optional currency/capabilities/optional delegation, AttachAck mode/siblings/interval/u64 queue count/capabilities/optional delegation, Delegation key/keyhash/window/hybrid signature, SiblingRef identity/endpoints/material, Heartbeat counter/time and SiblingUpdate optional replacement list. | W §§8.1–8.2 |
| SCH-021 | S | Cover TopologyPush kind/encoded object and TopologyMemo patron/locator/slot/source timestamp/optional occupant. Control type numbers are 1 Attach, 2 Ack, 3 Heartbeat, 4 SiblingUpdate, 5 Push, 6 Memo, 7 Delegation; numeric spaces are not interchangeable with transaction/request tags. | W §§8.0–8.2, 10 |
| SCH-022 | S | Cover ResourceRequest resource/HTTP bytes and ResourceResponse status/conditional HTTP bytes, all statuses 0–5 and response streaming conditions. Unknown fields in either unsigned map are malformed even when the enclosed HTTP is valid. | W §§11–11.2 |

### Required structural boundary matrix

For every row, test the stated boundary and one beyond it; all applicable limits apply together. A maximum legal inner object may still be too large for a particular enclosing frame. Do not promise that separate per-field maxima can always be combined into one transportable record.

| Value/container | Required boundary or special cases | Authority |
|---|---|---|
| Hashes, grant keys, ceremony commitment | 32 bytes; nonces and disclosure salts 16 bytes where specified. | W §§2, 4.5–7 |
| Hybrid public/signature sizes | Ed25519 public 32/signature 64; ML-DSA-65 public 1952/signature 3309 bytes; exact COSE key profile. | W §§2.2, 3.5 |
| Path | 0–24 nibbles, 0–9 values, exact packed length, odd low padding zero. | W §2.1 |
| NetworkPoint | IPv4 4 bytes; ASN u32; optional port 1–65535 with 7431 default omitted. | W §§2, 4.4, 9.2 |
| Envelope roles | Per type, not universal constant; presence ≤18 logical signers and ≤36 hybrid entries. | W §§1.3, 3.5 |
| Signer back-pointers | 1–8 unique ordered heads per required signer. | W §3.1 |
| Archive subset references / fetch count | ≤256 references; requested batch 1–256. Curated presence bundle has no independent protocol-wide cap. | W §§1.3, 5.4, 7.9 |
| Recovery verifier responses | 1–32, unique verifiers, at least one match, hybrid response signatures. | W §§1.3, 4.1 |
| Presence witnesses / responses | Normal 1–16 witnesses, 0–32 responses (omit key at zero); formation neither. | W §§1.3, 3.2, 4.5 |
| Presence duration | 0–86,400 seconds; chronology is distinct from local clock tolerance. | W §§3.2–3.3 |
| Capture / channels | Images 3–5; channels 1–8, optional session binding 1–128 bytes; repeated kinds legal. | W §4.5 |
| Location | Asserted 0–4; corroborations 0–16; geohash exactly 3 or 4 canonical ASCII characters. | W §§1.3, 4.5 |
| Query / attestation evidence | Profile 1–4096 bytes, template version u16; optional integrity evidence 1–1024 bytes with no extra attested-presence condition. | W §§4.5, 5.6 |
| Disclosures | Exactly seven slots; salt 16 bytes, label 1–32 bytes and one of the seven names, withheld digest 32. | W §4.5.1 |
| Explicit scope | 1–256 ordered unique identities; retired tag 3 rejected. | W §6.6 |
| Catalog entry / answer | Entry total ≤2048 encoded bytes; reply 0–111 entries; service type/filter/continuation 1–64 bytes, name 1–128, endpoint 1–256, optional metadata 1–1024. | W §§1.3, 6.1, 6.4 |
| Endpoint collections | Anchor, endpoint and sibling records 1–8; peering exactly one NetworkPoint per party. | W §§1.3, 4.4, 7.2, 7.6, 8.2 |
| Backup audits | At most eight in peering evidence. | W §§1.3, 4.4 |
| Prekey carrier / batch | Reusable blob ≤4096 bytes; batch 2–256 subjects; one-time deposit 1–256 blobs and overall frame limit. | W §§1.3, 7.8, 7.10 |
| Wake registration | Endpoint 1–2048 bytes and authentication key 1–256 bytes when present; withdrawal omits endpoint/key/expiry. | W §7.10 |
| Capabilities | Required map may be empty; ≤64 entries, each value ≤1024 bytes; grease adds one unknown ID with 8 bytes. | W §8.1 |
| Siblings / heartbeat | 0–9 siblings, absent key for none; interval 1–3600 seconds; failure at three missed intervals, not counter gaps. | W §8.2 |
| Extensions | Signed extensible map ≤16 unknown keys, each encoded value ≤1024 bytes; unsigned messages reject unknown keys. | W §§1.2–1.3 |
| Framing | Control ≤65,536 bytes; request and reply frame ≤262,144 bytes; four-byte BE length excludes the prefix. | W §§8.0, 9.2 |
| Resource roles | ≤64 application roles, token 1–32 lowercase ASCII letters/digits/underscore/hyphen; empty set legal. | W §11; R §3 |
| Abuse detail | Optional 1–1024-byte detail; category 0–5. | W §6.3 |

Closed enums reject unassigned values unless their definition explicitly says otherwise. Required forward-compatible cases are unknown location methods, unknown data-practice values, unknown capabilities, reserved witness observation bits and unspecified reason codes **inside** the 0–63 range. ClientIntegrity evidence presence and channel resolution/binding presence are syntactic options, not newly inferred truth constraints.

### Request and storage participation matrix

“History” means the identity archive, not an implementation's operational durable store.

| Wire transaction / exchange | Initiator → processor/participants | Result / lifetime | Primary tests |
|---|---|---|---|
| Tx 1 adoption: ordinary / transfer / recovery | Node and patron; former patron or recovery verifier supplies evidence | Hybrid envelope; relevant signers' history and horizon topology. | TX, REC, ARC, TOP |
| Tx 2 departure | Departing node → relevant topology holders | Unilateral envelope; identity history and topology; rootward hint. | TX, TOP |
| Tx 3 disavowal | Patron → relevant topology holders | Unilateral envelope; patron history and topology; rootward hint. | TX, TOP |
| Tx 4 peering | Two infrastructure participants | Bilateral envelope; history and eligible topology; no rootward memo. | TX, TOP, POL |
| Tx 5 presence | Two participants and witnesses; embedded verifiers | Attestation archive; elective presentations/pull, not ordinary topology flood. | CER, VER, CAP, ARC |
| Tx 7 series reissue | Node and patron | Hybrid envelope/history; proof of current series accompanies evaluation on request. | TX, ARC, TOP |
| Request 1 resolution | Caller/serving resolver → successive infrastructure authorities | Nonce-bound reply/hint; expiring routing cache, no history. | RES, NET |
| Request 2 archive fetch | Evaluator → infrastructure holder | Bounded history batch; chosen head/lookback and completeness state. | ARC, NET |
| Request 3 single/batch prekeys | Sender/client → serving prekey service | Reusable material and optional one-time consumption only for targeted request; no query history. | MAIL, PAY |
| Request 4 verification | Authenticated selector → addressed verifier, with subject consent | Signed response plus subject copy; query/counter lifetime is bounded. Recovery is local reverse querying, not this network request. | VER, REC |
| Request 5 catalog | Participant → horizon infrastructure hosts | Current owner-signed entries, bounded cached view; no history. | CAT |
| Request 6 resource | Participant → resource host gateway → backend | Authorized HTTP response and per-resource session; application state is resource-owned. | GAT, PKG |
| Request 7 registration | Authenticated owner → serving host | Current catalog entry/discover request; no archive transaction or topology broadcast. | CAT |
| Request 8 currency | Introduced caller → eligible issuer | Attestation/no-issue; request/reply discarded, fresh staple may be used locally. | CUR |
| Request 9 prekey publication | Client → serving node | Current reusable bundle; SubmissionReply. | MAIL |
| Request 10 one-time deposit | Client → serving node | Opaque expendable supply; SubmissionReply. | MAIL |
| Request 11 relay submission | Sender → serving/recipient custody path | Ciphertext custody/refusal, not delivery receipt; queue retention rule. | MAIL, PAY |
| Request 12 wake registration | Client → serving node | One optional external endpoint per relationship; SubmissionReply. | MAIL, APP |
| Control 1–4 | Attached peers, heartbeat both directions | Attach/Ack/Heartbeat/SiblingUpdate session state and durable sibling cache. | SES |
| Control 5 | Adjacent authenticated topology peers | Store-if-eligible, byte-identical forward iff newly stored. | TOP |
| Control 6 | Child-side topology observer → ancestors | Current rootward hint/table, not signed evidence or global directory. | TOP |
| Unidirectional payload / queue drain | Serving node or direct end-to-end peer → recipient | Ciphertext until endpoint decryption; queued copy deleted on delivery. | MAIL, PAY |
| KeyGrant / subject response copy | Subject ↔ addressed verifier over E2E association | Ephemeral authorized key/result exchange; no new archive transaction. | VER, CAP |
| SignedLocator / AnchorEntry / EndpointRecord | Named subject → introducer or routing holders | Current-state signatures; only EndpointRecord has the defined push kind. | RES, TOP |
| SubtreeAck | Grandpatron node → holder/evaluator | Separate signed membership evidence; discard on lapse. | GAT |
| LateResponse | Verifier/holder → willing evaluator | Supplemental evidence retained no longer than parent record. | VER |
| AbuseReport | Resource → its owner locally | Private signed application report; no network carriage or trust contribution. | PKG |

## 5. Cross-component transaction and lifecycle scenarios

These scenarios verify that individually correct modules remain correct when joined through the daemon, adaptors and FFI. Each scenario inherits the field, negative and persistence cases of its referenced families. Use real separate processes/sockets where practical; reserve hardware and human-notice claims for device/application testing.

### End-to-end scenarios

| ID | Kind | Requirement and functional test oracle | Authority |
|---|---|---|---|
| INT-001 | E | Bootstrap two fresh participants with at least one infrastructure node: conduct a formation meeting, sign/store it, adopt one under the other, attach, publish locator/prekeys and exchange encrypted payload. Check genesis back-pointers, permanent formation subtype, routing and absence of invented witnesses. | D §§13–14; TX, CER, ARC, SES, MAIL, PAY |
| INT-002 | E | Admit an established participant through a normal ceremony: fetch a deliberately incomplete archive, surface its gap, gather known/unknown verifiers, deliver grants/results to both parties, exercise omission veto, finalize and adopt. Show structural validity, completeness and policy standing separately. | D §§6–10; TX, ARC, CER, VER, CAP, POL |
| INT-003 | E | Have one verifier answer for both participants while another is offline. Order and retain both subject-specific responses, accept the legal thin result and never count that verifier as an envelope signer or require two distinct verifier identities for those two answers. | W §§3.5, 4.5, 5; TX, CER, VER |
| INT-004 | E | Repeat a legitimate transfer with the former patron reachable and unreachable. Valid transfer evidence replaces the meeting where allowed; without that evidence use ordinary presence-based adoption. Permute departure/adoption delivery without a manufactured atomic transfer protocol. | D §6.2.3; TX, TOP, ARC |
| INT-005 | E | Create two independent subnet bindings, advance endpoints in one, reissue its series and replay high-counter old-series updates. Other-binding counters/disclosures remain independent and stale routing does not win by numeric series order. | D §§9.2, 13.6; TX, ARC, TOP, RES |
| INT-006 | E | Recover a compromised key with valid old proof plus an in-person prior-counterparty match, then try old-key attach, currency, queue delivery and resource use. Observe local supersession and outside-horizon unexplained stale-path failure. | D §9; REC, CUR, SES, MAIL, GAT |
| INT-007 | E | Produce two valid recovery heirs vouched by different patrons and give observers different recognized evidence. Both archives remain valid; lineage/current standing is observer-relative and misconduct is not automatically inherited by the other heir. | D §§9.0.2, 16.7; REC, POL, CUR |
| INT-008 | E | Sign concurrent offline archive branches, reconnect and merge, fetch with small batches and a missing branch, restart the holder, then finish the walk. No branch silently disappears and no invented total order or genesis erases history. | D §10.3; ARC, OPS |
| INT-009 | E | Disavow/depart a subtree while a neighbor is offline, lose an initial push, repair by replay/fetch, and check all retained views agree. Downline stays with the root; stale positions fail and no mailbox state is replicated with topology. | D §§6.2, 15; TOP, ARC, RES, MAIL |
| INT-010 | E | Introduce a mutual cycle and a distant longer cycle, then forged/stale memo hints. Humans choose direction only for the mutual live proposal; distant automatic cuts require signed confirmation and are signed by the actual authoritative participant. | D §§6.2.5, 15.2; TOP, UX |
| INT-011 | E | Break the primary after successful attach, replay heartbeat duplicates, deliver a later counter with gaps and suspend/resume the client. Failover occurs on three elapsed missed intervals only, authenticates cached siblings and preserves the dark-mailbox distinction. | D §14.1.2; NET, SES, MAIL |
| INT-012 | E | Keep the primary reachable but policy-refusing and contrast that with a dark endpoint and a refusing sibling. Verify policy refusal cannot be bypassed by treating it as liveness failure, and an endpoint failure does not corrupt the complete sibling cache. | W §§8.2, 9.2; SES |
| INT-013 | E | Send to an offline client through PQXDH prekeys, race two one-time requests, restart the serving node, reconnect and drain. Accepted ciphertext survives before delivery, one-time material is not reissued, and all delivered queue copies disappear. | D §§14.1.6, 14.2.4; MAIL, PAY, OPS |
| INT-014 | E | Fill one recipient's queue while another has capacity. Refuse newest excess with a correctly bound response, retain old accepted mail indefinitely, and verify sender-facing failure without a global queue outage or hidden eviction. | I §2; MAIL |
| INT-015 | E | Register, refresh, expire and withdraw wake endpoints while toggling OS push permission. A content-free doorbell reconnects through the kernel; no message content or mailbox count leaks to the external service and default foreground use still works. | D §§14.1.4–14.1.5; MAIL, APP, UX |
| INT-016 | E | Exercise direct communication across supported NAT pairs, then blocked traversal, out-of-horizon peers and both users' relay overrides. Candidate disclosure follows the choice and every completed payload has authenticated end-to-end attribution. | D §§12.6.3, 14.1.1, 14.2; PAY, NET, APP |
| INT-017 | E | Register two owners' resources on one host and conflicting resource claims on separate hosts. Sweep a horizon with an offline host and more than 111 entries of one type; preserve ownership, stale/truncated portions and stop repeated continuation loops. | W §6; CAT, UX |
| INT-018 | E | Access an existing resource from inside/outside its owner's horizon with missing Ack, denied connect, malformed HTTP and stopped backend. Exercise all combinations to verify the ordered error ladder and absence of preauthorization backend requests. | W §11; GAT |
| INT-019 | E | Change a user's roles, cross a tenure boundary and move the owner while two resources share one QUIC connection. Retire only affected resource sessions, recompute grants and issue new session IDs without carrying old permissions forward. A request already running completes under its entry snapshot; the transport connection is untouched. | I §10; GAT, TOP, UX |
| INT-020 | E | Spoof trusted headers and Host/absolute URI, send request-smuggling variants and make the backend fail after an application write. No spoofed identity reaches it, requests route only to the configured binding, and the write is not automatically retried. | R §3; W §11.2; GAT |
| INT-021 | E | Install a hostile package while attachment, heartbeat, prekey and other resource traffic are active. Denied imports and bounded execution prevent access to privileged records or denial of the whole node; report any residual failure without claiming absolute sandbox proof. | I §9.2; PKG, NET, MAIL, OPS |
| INT-022 | E | Backup retained multi-ceremony captures/seeds and merged archive, expire one capture, restore through the chosen supported path and test malformed/wrong-passphrase backup. Preserve subject identity/history and expiry; do not convert missing private state into a fraudulent continuity claim. | D §§10.2, 13.7.1; OPS, ARC, CAP, REC |
| INT-023 | E | Hold a fixed honest-to-attacker cut while increasing fake identities, repeated meetings and parallel peering behind it. Compare individual versus aggregate allocation and assert reference setwise capacity stays bounded without suppressing legitimate horizon membership. | D §§16.2–16.3, 17.3; POL, TOP |
| INT-024 | E | Replay every mutable request as 0-RTT, then send the same legitimate request after confirmation. No pre-confirmation state/budget is spent; permitted read-only queries remain usable and failures stay confined to the defined frame/stream/session scope. | W §§8–9; NET, MAIL, CAT, VER, GAT |

## 6. Privacy, evidence and verification obligations

### Cross-cutting verification and operational evidence

| ID | Kind | Requirement and functional test oracle | Authority |
|---|---|---|---|
| VAL-001 | C | Inspect retained artifacts at participant, witness, verifier, serving node, ancestor and resource separately. Each receives only its specified data class: no global biography/face database, no archive inside a resource credential, no raw profile in the presence body. | D §§7.2, 11.0, 15, 19; L §1.5; I §9.2 |
| VAL-002 | C | Verify that selective disclosure hides only its seven committed fields. Identities, witnesses, verifier relationships, start/finalization times and subtype remain visible; UI and documentation must not promise those are hidden by withholding location. | D §§8.1.1, 19.3; W §4.5.1 |
| VAL-003 | C | Verify separate lifetimes for immutable history, current topology/endpoint/catalog state, lapsed acknowledgement, query counters, late responses, queue ciphertext and expiring caches. A single generic retain-forever database policy does not satisfy these different obligations. | D §§10, 14.1.6, 19.4–19.5; W §§6.5, 7.1, 7.4–7.5 |
| VAL-004 | C | Currency queries/replies are answered and discarded without a durable introduction log; allow only needed fresh attestation use and expiring operational state. Stateless no-issue and timeout remain distinguishable to the caller. | W §7.1; CUR |
| VAL-005 | C | No infrastructure actor can sign subordinate transactions using only gateway/hosting privileges. Conversely disclose that a compromised gateway can impersonate role credentials for its resources; do not claim participant signatures prevent that separate access-authority risk. | D §§18.2, 19; R §10 |
| VAL-006 | E | Validate independence of expected cryptographic bytes using externally generated/pinned reference vectors for the adopted algorithms. Existing draft/self-generated vectors establish regressions but not independent interoperability or cryptographic proof. | D §§5.2, 23.4; W §§1–3 |
| VAL-007 | E | Measure queue cap, heartbeat interval, retry patience, cache TTL, prekey cadence, replication depth and rescore cadence under disclosed settings. Enforce fixed wire bounds while making local operating choices explicit and reproducible. | D §§21–21.1; I §12 |
| VAL-008 | E | Test performance-sensitive assumptions separately from functional pass/fail: face matching/liveness attacks, traversal success/relay load, churn versus user traffic, witness availability, recovery/backup usability and hostile package containment. Do not invent pass thresholds from approximate design estimates. | D §§20.1–20.2, 21 |
| VAL-009 | E | Track D §20's behavioral/economic/security assumptions as unvalidated hypotheses unless measured by the relevant experiment. Functional tests cannot prove social adoption, physical investigation cost, cloud price, human scarcity or general resistance to composition. | D §§1.2, 17–20 |
| VAL-010 | E | Preserve missing coverage honestly: simulated camera/consent proves only interface handling; instrument restart does not prove product persistence; successful Double Ratchet does not prove SPQR; a stub acceptance ID does not prove an implemented requirement. | D §§14.2.4, 23.3–23.4; L §§9–10 |
| VAL-011 | C | Test every obligation at the party that can actually check it. A rule aimed at a party the checker shares no state with is a commitment, not an enforced constraint, and must be exercised as one: assert the honest party's behaviour, not that a remote party was prevented. This is the distinction the S/C/P kinds rest on. | D §1.1; Appendix A |

## 7. Retention and authority checklist by state class

Use this table alongside OPS/VAL tests during export, restart, migration and deletion. A local store may implement several classes together only if it preserves their different lifetimes and visibility.

| State | Authority / location | Required lifetime and exposure |
|---|---|---|
| Private identity material | Participant kernel; explicit protected backup | Survives intended restart; not disclosed by routine API, protocol or resource binding. |
| Identity archive and merge heads | Signer/holder; elective archive presentation | Retained history, eligible whole-prefix checkpointing; original signatures and branch information preserved. |
| Capture plaintext / temporary profile / decryption key | Ceremony or authorized verifier operation | Erased after sealing/comparison; no diagnostic/debug/backup leak. |
| Sealed capture / subject seed | Holder ciphertext / subject-controlled private state | Independent per-capture retention and backup; default two years for capture, release choice distinct from storage. |
| Selective-disclosure salts/values | Authorized record holders | Retained according to parent record and disclosure choice; withholding is not deletion. |
| Consent / anti-oracle counters / early grant | Endpoint's active bounded operation | Enforce one-profile/authorized-verifier binding; expire window/buffer; no permanent query ledger. |
| Verifier response / late response | Participants or voluntary evaluator | Signed evidence; late supplement does not outlive its record or retroactively modify it. |
| Current topology / current series proof | Eligible horizon holder | Validate against signed authority, retain relevant evidence, prune obsolete derived membership; proofs are not guessed from series magnitude. |
| Materialized view / routing cache | Local derived state | Rebuildable; bounded/expiring where specified; never overrides authenticated facts. |
| Rootward memo table | Ancestor within that subnet | Optional current (patron,slot) rows/tombstones, no historical update log or cross-subnet/IP index. |
| Sibling list and public pins | Attached client kernel | Complete replacement, persisted for dark-primary failover; unusable without validated matching material. |
| Queue | Custodial serving node only | Ciphertext, recipient, arrival time; indefinite until delivery, cap refuses newest; immediate deletion including recovery copies after delivery; no sibling replica. |
| Reusable / one-time prekeys | Recipient's serving node | Current reusable publication; one-time consumed once; no detailed fetch, publication or deposit history, and no speculative batch depletion (I §1's cross-class rule). |
| Wake endpoint/auth key | Serving relationship | At most one optional current registration; replace/withdraw/expire/forget on end; external service gets doorbell only. |
| Currency query/reply | Live issuer/caller exchange | Process/discard; fresh attestations can be stapled/reused until their signed expiry, never extended stale. |
| Catalog entry and discovery request | Current host, signed by owner | One current entry per resource there; superseded entries removed; host may narrow discovery. |
| Catalog/contact/session/capability caches | Client | Explicit expiry, per-host stale/truncated visibility; not a permanent interest log. |
| SubtreeAck | Resource-authorizing holder | Current adoption/grandpatron relation only; discard on lapse. |
| Role row / hosted session | Host for one resource and principal | Derived from current gate/policy snapshot; current rows only, no history of row changes (I §10.2); role changes retire the affected resource session. |
| External broker session | External service/client | Network can prevent future establishment, cannot promise revocation of existing vendor session. |
| Resource application data / abuse detail | Application and owner | Outside participant archive/trust; package sees its traffic/credential only; declared data practice is a claim, not enforced truth. |

## 8. Source coverage and planned scope

This index accounts for the root document sections, including rationale that does not itself supply a functional oracle. A section covered by a family still needs every applicable case in that family; the index is not a claim of executed coverage.

| Source sections | Coverage / treatment |
|---|---|
| D preface, document set, §§1–2 | Authority and evidence distinctions in §1 here; observer-relative policy, privacy and no invented enforcement in POL/VAL. Thesis, security floor and economic claims remain hypotheses. |
| D §§3–4 | TOP/SES/RES/INT; component inventory and planned/deferred boundaries below. |
| D §5 | ENC/SIG/NET/PAY/VAL; upstream profile decisions O-011. |
| D §§6–8 | TX/CER/VER/CAP/ARC/UX; exact schemas SCH. |
| D §9 | REC/TX/CUR/POL/OPS; O-012 for unsettled integrations. |
| D §10 | ARC/OPS/TOP/VAL. |
| D §11 | CAT/GAT/PKG/UX; owner scope, pairwise boundary, disclosure and revocation tests. |
| D §12 | RES/CUR/PAY/TOP; O-007's remaining gap, the unpinned intermediate/anchor authentication profile. |
| D §13 | TX/REC/OPS/INT/UX; formation, multiple bindings and backup. |
| D §14 | NET/SES/MAIL/PAY/APP; live device/wake/full ratchet remain planned integrations. |
| D §15 | TOP/ARC/VAL; explicit horizon counts. |
| D §16 | POL/GAT/REC; distinction from topology and alternative policies. |
| D §§17–20 | POL/VAL/UX/PKG/INT; threat experiments, accepted privacy/composition costs and empirical assumptions are not converted into fictional guarantees. |
| D §21 | Boundary matrix, parameterized policy/performance cases and VAL; chosen values distinguished from derived limits. |
| D §22 | Open register below; settled functional parts remain testable. |
| D §§23–24 | Planned inventory, scope exclusions, APP/TOOL/VAL; suggested build order is not a new protocol dependency. |
| D Appendix A | Kind classifications, automatic versus human acts, local versus shared-evidence checks throughout. |
| D Appendix B | Relevant surviving decisions covered by ENC/CAP/TOP/POL; rejected alternatives are not requirements to implement. |
| W §§1–3 | ENC/SIG/SCH/TX/ARC; all canonicality, signature and structural matrices. |
| W §§4–5 | TX/CER/VER/CAP/REC; all six live transaction types, recovery/transfer variants and nested evidence. |
| W §6 | CAT/PKG/SCH; current service state, scope, replies, private local report. |
| W §7 | CUR/RES/VER/MAIL/GAT/ARC/SCH; all named standalone objects and services. |
| W §§8–10 | NET/SES/TOP/SCH; control/request dispatch and propagation. |
| W §11 | GAT/SCH/INT; gateway ordering, HTTP and sessions. |
| W §§12–13 | Estimates are not new ceilings; explicit bounds tested, open integration choices recorded. |
| L §§1–2 | CER/VER/CAP/ARC/REC/OPS/UX. |
| L §§3–4 | PAY/SES/MAIL/TOP/APP. |
| L §§5–7 | UX/CAP/PAY/TOP; choice and warning timing. |
| L §§8–10 | CAT/GAT/UX/APP/OPS and open multi-device/restore work. |
| I §§1–4 | TOP/SES/MAIL/CUR/RES/OPS. |
| I §§5–8 | ARC/MAIL/NET/OPS; headless read-only management boundary. |
| I §§9–12 | PKG/GAT/CAT/UX; package/profile and in-flight ambiguities recorded. |
| R §§1–4 | CAT/GAT/POL/PKG; credentials, HTTP, owner scope and federation. |
| R §§5–6 | PKG/CAT/GAT; extensions versus arbitrary programs and registration versus presentation. |
| R §7 | GAT/CAT/UX; all predicate classes, named grants and visible roles. |
| R §§8–13 | PKG/GAT/OPS/VAL; packaging/isolation, IdP and gateway risks, owner-host split; external consumer policy expressly outside the network's specification. |

### Planned versus deliberately deferred

Planned work **is included**: shipping Android/iOS shells and bindings, actual device capture/proximity/liveness validation, complete durable participant backup/restore, full PQXDH/Triple Ratchet integration, external wake deployment, interoperable package distribution/provenance and selected broker/IdP adapters. A requirement is not removed merely because source has no implementation yet.

The design deliberately does **not** require implementing the following as v1 functionality: arbitrary global identity search, global reputation or consensus, IPv6 wire addresses before its representation is chosen, hard-fork forwarding/upline mailbox resilience, autonomous participants replacing live human presence, unrestricted native applications/device hooks, a mandatory global service/role/claim registry, or unspecified multi-device synchronization beyond archive merge. Multi-device persistence/deletion remains an explicit dependency for any product claiming it. Independent canonical test vectors are a validation deliverable, not evidence supplied by a self-roundtrip test.

D §20's empirical assumptions, claimed costs and social/economic predictions need separate research or measurements. Approximate 35-KB records, expected population/load, face error rates, relay fraction, operator prices and the 500,000-node anchor guideline must not be turned into unsupported mandatory performance thresholds.

## 9. Specification conflicts and decisions needed for complete test oracles

The settled behavior described in the requirement rows should be tested now. These entries identify the narrow unresolved part; they do not make the entire subsystem untestable. Where a current W schema is explicit but D contains different prose, codec tests record what W encodes while design conformance remains qualified until the texts agree.

| ID | Sources and issue | Settled behavior / decision still needed |
|---|---|---|
| O-001 | **RESOLVED 2026-09-17.** I §10.5 no longer resets running requests and says so — *"A request already in flight is not reached into (§10.1)"* — and D §11.4 now separates three things where it used to run them together. | A running request completes under the state it started with; the resource-facing hosted session is retired, so the next request is evaluated afresh under a new identifier; the `rhtn/1` transport connection is untouched. |
| O-002 | **RESOLVED 2026-09-17.** D §11.4 now reads *"permission does not extend past the horizon at all, scope or grant alike"*, and §11.2's gate sits behind both. | There is no out-of-horizon grant path. Current membership in the owner's trust horizon is the outer gate for every form of access. |
| O-003 | **RESOLVED 2026-09-17.** D §9.0.1 is scoped to a rotation that carries its inheritance, and §9.0's old-key sentence with it. W §4.1 field 6 is `? Recovery`, present iff this is a recovery adoption. | A plain rotation is unlinked adoption carrying nothing. A linked rotation and a recovery are one procedure and need both factors. Keep them distinct in APIs and wording. |
| O-004 | **RESOLVED 2026-09-17.** D §6.4 and W §4.5 agree: witnesses are PRESENT on a normal record and ABSENT on a formation, verifier responses ABSENT always on a formation. §6.4's threshold language now says responses are sought and their absence weighed (§8.1.2) rather than required. | Formation is genesis-only and witness- and verifier-free; a normal record requires its witness condition. A missing verifier threshold is not a schema failure. |
| O-005 | **RESOLVED 2026-09-17.** D §15.2 now reads *"confirms it against its own records before acting — never by fetching"*, matching W §10.2.3. | Never act on a memo alone. Confirmation uses the detector's own records; no acquisition path is needed, because the detector is never a stranger to the transaction it is checking. |
| O-006 | **RESOLVED 2026-09-17.** W §7.9 field 2 is `[ * ArchiveEntry ]` with `ArchiveEntry = PresentedRecord / Envelope`, and both continuation fields are frontiers rather than single txids. | A presence record carries its holder's disclosure choice; every other type is a bare envelope. Pagination walks a DAG frontier, and verification is reachability rather than batch order. |
| O-007 | **Contradiction RESOLVED 2026-09-17**; a gap remains. D §12.2 now says full keys arrive at contact and are never fetched during resolution, citing W §7.2. | Still open, and not a contradiction: the authentication profile for an unpinned intermediate or anchor. Only the explicitly allowed query may use an unpinned routing authority; terminal service requires a validated pin. |
| O-008 | **RESOLVED 2026-09-17.** D §16.3 now calls peering *"a transaction both peers sign (§6.3), carried in the topology class"*, and D §11.6 says an abuse report is not one of W §4's typed envelopes and carries no type number. | Use the typed bilateral peering transaction and the standalone local `AbuseReport`. |
| O-009 | **RESOLVED 2026-09-17.** I §10.2 now names two rank classes rather than one: an absolute rank has a fixed cutoff and a contested last place, so an entrant above the line displaces a member whose row changes without that member changing. | Re-score the whole table for a relative rank and at least the displaced row for an absolute one; the changed member alone is enough for every other class. Never leave a displaced member authorised. |
| O-010 | **RESOLVED 2026-09-17.** D §19.6 now reads *"Neither is owed a warning"* of a witness and a verifier, agreeing with §7.3 and Appendix A.3. The obligation was withdrawn as a drafter gloss, not relocated. | No per-query notice to a witness or verifier: consent to perform protocol actions is given by choosing to use the network. The capture-time disclosure runs to the ceremony participant. |
| O-011 | D §§7.4–7.5, 14.2.4, 22 and W §13 leave upstream cryptographic/capture integration choices to implementation: template form/length, matcher/fuzz/version profile, AEAD capture profile, payload typing and complete PQXDH/Triple Ratchet binding. | Test fixed schemas and implemented primitives now. Publish selected interoperable profiles and independent vectors before claiming complete capture/payload interoperability; Double Ratchet alone is insufficient for the full construction. |
| O-012 | D §§9.0.2, 12.6.5, 12.7.2 specify competing-recovery notices, light-patron delegation and optional downline-threshold currency; W §7.1 carries one issuer signature and does not define these complete exchanges/quorum proofs. | Preserve local lineage choice and explicit issuer roles; never invent a global winner or universal threshold. Define concrete notice/delegation/support profiles where implemented. |
| O-013 | W §7.8 defines PrekeyBatchRequest but no named complete batch-reply container/subject-association schema. | Sweeps request reusable material only and consume no one-time supply. Specify exact batch result framing/order and failure association for independent clients. |
| O-014 | D §§3.4, 6.3 and I §12 describe encrypted peer backup/audits without a complete transfer/challenge/recovery wire profile. | Peering schema, bounded audit evidence and separation from queues/resources are testable. Select an interoperable backup protocol before claiming durable independent backup or recovery from it. |
| O-015 | **Narrowed 2026-09-17.** D §23.3 now settles which device holds what — seeds on the ceremony device, archives and sealed captures and caches where the storage is, deletion state following the seed — and the privacy register marks the question answered. What it does not settle is restoration. | Which devices retain archives, seeds and captures is decided; test it. Still open: what a restored backup may resurrect, how deletion propagates across devices, and how a user is shown an archive spanning several. |
| O-016 | R §8 and I §§9.1, 12 leave package distribution/provenance/update and selected external identity adapters open. D §14.1.5 requires a platform deployment for external wake, including iOS relay support. | Implemented sandbox/HTTP/registration behavior remains testable. Record the chosen packaging, broker claims and wake posting/relay profile; do not silently equate a working local ABI with a shipping integration. |

Open local parameters are not all specification defects: cache TTLs, queue cap, clock tolerance, retry patience, anti-oracle budgets, inactivity decay, matcher threshold/inconclusive band and periodic rescore cadence are deliberately configurable. Tests declare values, check their bounds and behavior, and distinguish a locally selected policy from an on-wire constant. No peer can be required to use the reference score merely because this suite evaluates it.

## 10. Document baseline and coverage totals

This specification contains **464 numbered requirement/test families** across **26 ID prefixes**, in addition to the dispatch, boundary, retention and source-coverage matrices. They specify work to verify; they do not report executed passes.

| Prefix | Families |
|---|---:|
| ENC | 13 |
| SIG | 13 |
| TX | 30 |
| ARC | 17 |
| TOP | 36 |
| RES | 15 |
| CUR | 12 |
| NET | 11 |
| SES | 15 |
| MAIL | 25 |
| CER | 20 |
| VER | 18 |
| CAP | 16 |
| REC | 12 |
| PAY | 11 |
| POL | 21 |
| CAT | 22 |
| GAT | 37 |
| PKG | 14 |
| APP | 10 |
| UX | 14 |
| OPS | 18 |
| TOOL | 7 |
| SCH | 22 |
| INT | 24 |
| VAL | 11 |

| Design input | SHA-256 of reviewed bytes |
|---|---|
| [network-design.md](network-design.md) | `c26d228c35cfdbe3ede873e9549591a04eebae659d4c513f5c4da38fb3dce015` |
| [wire-format.md](wire-format.md) | `5fb34586d8b9d970c64daa5fc14ae05fc8c3ac7a1c3fe1510a1463c2975b31db` |
| [light-client-requirements.md](light-client-requirements.md) | `ec6526baaefee2a7de3e3e1a8eb4354a219f1b6ff12d672061584917af263781` |
| [infra-client-requirements.md](infra-client-requirements.md) | `780b188301c668cdf8b35fe506678b305bcf073289536461c70fda7a3e7254d4` |
| [resource-requirements.md](resource-requirements.md) | `54edf8e85eacee7b68e69c0f8b971ae83c19b05e284ae3187bf14d6846ee6f8d` |

Implementation inventory: `crates/Cargo.toml` and the source directories listed in §2 at the stated commit. Production code and prior conformance-review files were not modified to prepare this document. Rebaseline hashes and affected test families when the specifications change.
