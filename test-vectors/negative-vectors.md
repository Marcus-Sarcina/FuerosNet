# Negative and conformance vectors

**Draft. Spec-derived, unverified by an implementation.** See
[README.md](README.md).

## The result model is structured, not a single status

The specification's validation semantics do not reduce to accept/reject, and —
as the second external review observed — they do not reduce to *one* scalar
outcome either: its own cases include "valid for one subject, unverifiable for
the other." A conforming harness reports a **verification result with
independent dimensions**, and each fixture below constrains only the dimensions
it names:

| Dimension | Values | Where stated |
|---|---|---|
| `structural` | valid · **malformed** | `wire-format.md` §1, §3.2 |
| `signatures` | verified · unverifiable(signer) — per signer, a missing key is a third outcome, not failure | §3.4 |
| `chain` | complete · **incomplete** — an unfetchable predecessor is a fact, not an error | §3.4 |
| `selection[subject]` | verified · unverifiable — **per subject**; a holder with one archive verifies one half | §5.5 |
| `effective` | yes · **no** — structurally valid may still grant nothing for this evaluator | §6.8 |
| `evidentiary` | free-form flags (fork-detected, …) — findings against history, not against the object | §3.2 |

**Over-strictness is non-conformance too**: section D fixtures MUST be
accepted, and a decoder rejecting them fails conformance exactly as one
accepting section A fixtures does.

Byte-level fixtures (section A) decide `structural` from the input alone.
Context fixtures (section B) are `bytes + context + external state + expected
result`. Section C is method requirements — obligations on how a decoder
works, not inputs.

## A. Byte-level — expected: `structural = malformed`

### Encoding layer (`wire-format.md` §1)

| # | Input | Violation |
|---|---|---|
| E1 | `18 0a` for the integer 10 | Non-shortest form. Valid: `0a` |
| E2 | `9f 05 18 2a ff` for `[5, 42]` | Indefinite-length array. Valid: `82 05 18 2a` |
| E3 | `a2 01 00 01 00` | Duplicate map key — reject **before materialising the map** (C1) |
| E4 | `a2 02 00 01 00` | Protocol map keys out of ascending order |
| E5 | A peering body whose optional field 7 is present as `80` (empty array) | An OPTIONAL field whose value would be empty MUST be omitted, never encoded empty — the positive peering vector shows the omission |
| E6 | A body carrying 17 unknown extension keys | Exceeds the 16-per-map bound |
| E7 | An unknown extension key whose encoded value is 1,025 bytes | Exceeds the 1,024-byte encoded-slice bound |
| E8 | A presence record with `subtype = 2` | §1's unknown-enum rule on a closed enumeration: field 6 is `0 normal, 1 formation` and load-bearing (§4.5). **Choosing the field matters** — two earlier instantiations of this row were wrong: disavowal codes are a banded exception (D1), and `ClientIntegrity.scheme` is an **open** namespace ("a validator checks only the shapes", §4.5) |
| E10 | The extended adoption (`transactions.md`) with its unknown key's value mutated `c0ffee` → `c0ffef` | Unknown retained keys are **covered by every signature** — all four entries MUST fail. The unmutated envelope is must-accept D2 |
| T8 | A disavowal with reason code **64** | Outside the 0–63 code space (§4.3) — malformed for that reason, **not** because it is unassigned; unassigned in-range codes are accepted (D1) |

### Primitives (§2)

| # | Input | Violation |
|---|---|---|
| P1 | seqno encoded as `18 2a` (a single integer) | seqno is `[series, counter]`, never one integer |
| P2 | seqno `[5, 4294967296]` | counter exceeds u32 range |
| P3 | Path nibble value 11 (`b` in the packed bytes) | Nibble values MUST be 0–9 |
| P4 | Path `{1: h'314155', 2: 5}` | Odd nibble count with nonzero trailing low nibble |
| P5 | Path `{1: h'31415000', 2: 5}` | Packed bytes ≠ `ceil(5/2)` = 3 — surplus is malformed, not ignorable |
| P7 | A `COSE_Key` in `KeyMaterial` carrying a fourth label (`kid`) | Exactly three labels per component — any addition is a different identity |
| P8 | `KeyMaterial` ordered `[post-quantum, classical]` | Fixed order, classical first |

### COSE profile, envelope and signer set (§1, §3.5, §3.6)

| # | Input | Violation |
|---|---|---|
| S1 | A `COSE_Signature` with a nonempty unprotected header | Malformed, not ignored |
| S2 | `kid` placed in the unprotected header | Envelope entries carry `kid` in the **protected** header |
| S3 | A signer group with one entry | Two entries per logical signer, one classical, one post-quantum |
| S4 | A signer group with two entries both `alg = -8` | Two entries of the same `alg` |
| S5 | Entries ordered PQ-before-classical, or groups out of `kid` order | Canonical order (§3.5); the order-divergence adoption is the positive complement |
| S6 | An adoption envelope with three logical signers | Exactly the signers the type requires |
| S7 | Envelope `{1: 2, …}` | Unknown schema version MUST be rejected |
| S8 | A non-canonical envelope whose body verifies | Canonicality covers the whole envelope |
| S9 | An embedded `COSE_Sign1` — a presence-context `VerifierResponse` (field 9) or a consent signature — carrying a `kid` | Embedded objects omit `kid`; the `Recovery` old-key proof is a hybrid `COSE_Sign` (§4.1), and the omission rule reaches its entries since `Recovery` field 1 names the prior key |
| S10 | A `COSE_Sign`/`COSE_Sign1` whose payload slot carries the body instead of `null` | All signatures are **detached** (§1) — an embedded payload gives one object two encodings, and the copy could disagree |
| S11 | A nested COSE object carrying a CBOR tag (18/98) | Nested COSE is **untagged** (§1); tolerating both breaks canonicality |
| S12 | An envelope entry verifying under `external_aad = ""` or a wrong tag | Every context carries its own tag and the verifier reconstructs it **from context, never from the message** (§1.1, C2) — a generic COSE library left at defaults accepts this |
| S13 | A protected header carrying any parameter beyond `alg` (+ `kid` where required) | "Nothing else appears in either header" (§3.5) — an extra parameter changes the protected bytes and is malformed, not a tolerable extension |
| S14 | An envelope entry with `alg = -7` (ES256) | Outside the profile: any `alg` other than −8/−49 in a signer group is malformed (§3.5) |

### Presence-record structural rules (§3.2, §3.3)

| # | Input | Violation |
|---|---|---|
| R1 | The two participant identities equal | MUST differ |
| R2 | `finalized_at` < `started_at` | MUST be ≥ |
| R3 | `finalized_at` − `started_at` = 86,401 s | MUST NOT exceed 24 hours |
| R4 | A normal-subtype record with zero witnesses | Zero witnesses is the formation case only |
| R5 | The same witness keyhash twice | Witness identities MUST be distinct |
| R6 | A participant listed among the witnesses | Not an independent witness |
| R7 | A witness whose `nominated_by` is neither participant | MUST be one of the two |
| R8 | A formation record whose key 0 is not exactly `[SHA-256(signer keyhash)]` per signer | Structural genesis rule |
| R11 | A revealed `proximity` whose `strongest` lacks `result = pass`, or with a higher-ranked passing channel | Checked only on reveal; withheld → `selection`-style unverifiable, never valid, never malformed |

### Transaction and record bindings (§4)

| # | Input | Violation |
|---|---|---|
| T1 | A `NetworkPoint` with a 16-byte address | v1 demands IPv4 |
| T2 | A `NetworkPoint` with port 0 | Never a destination |
| T3 | An adoption whose carried `KeyMaterial` hashes to something other than field 1's keyhash | A recipient would pin the wrong key |
| T4 | A `Recovery` response whose field 8 ≠ the claimed prior key | Evidence about one identity under a claim about another |
| T5 | A `Recovery` response whose `subject` ≠ the newly adopted node | Continuity is attested *to* the new key |
| T6 | Two responses from one verifier **for one subject** | One `(subject, verifier)` slot each (§5.5); one identity answering once per participant is two slots and valid |
| T9 | A `Recovery` with an empty response array | Schema requires at least one (§4.1) |
| T10 | A `Recovery` whose responses carry no `match` | "At least one, and at least one `match`" (§4.1) |
| T11 | A `Recovery` old-key proof that is `COSE_Sign1`, or a `COSE_Sign` with a single entry | The old identity is hybrid: one logical signer, two entries (§4.1, §3.5) |
| T12 | An old-key successor statement whose `new_key` ≠ the enclosing adoption's field 1 | Field 1 is "the ONLY successor authorised" (§4.1) — the cross-object binding that stops one observed proof authorising competing successors |
| T13 | An adoption whose fields 1 and 2 are equal | Self-adoption is the degenerate cycle, and the one a validator sees from the record alone (§4.1, design §6.2.5) |
| T14 | A `Recovery` whose `prior_key` equals the enclosing adoption's field 1 | A same-key Recovery is vacuous evidence (§4.1) — the retained-key, lost-archive case is served by archive fetch, fresh adoption and merge, never by Recovery |

## B. Context-dependent — bytes plus external state, structured result

| # | Input | Context | Expected result |
|---|---|---|---|
| P6 | A bare `Locator` | Standalone, at introduction | `structural = malformed` — standalone carriage requires `SignedLocator` |
| P6b | The same bare `Locator` | Inside a transaction body | accepted — the envelope authenticates it |
| R9 | A structurally perfect formation record | Evaluator holds the key's real chain | `structural = valid`, `evidentiary = fork-detected` (§3.2) |
| R9b | The same record | No history held | `structural = valid` — a no-history validator cannot tell (§3.2) |
| R10 | `started_at` precedes the committed predecessor's effective time | Predecessor supplied and verified | `structural = malformed` (§3.3) |
| R10b | The same record | Predecessor unfetchable | `chain = incomplete` — not malformed (§3.4) |
| V1 | A record needing threshold recomputation for both subjects | One subject's bundle held | `selection[A] = verified`, `selection[B] = unverifiable` (§5.5) |
| V2 | A scope naming a position the evaluator cannot compute | Topology does not reach it | `structural = valid`, `effective = no` — MUST NOT reject (§6.8) |
| V3 | A signer's key material never seen and no resolver | Any signed object | `signatures = unverifiable(that signer)` — a third outcome; collapsing it into invalid discards a fetchable object (§3.4) |

## C. Method requirements — how a decoder works

| # | Requirement | Rule |
|---|---|---|
| E9 | Validate the bytes as received; decode-then-re-encode equality is non-conforming | It erases the evidence it should find (§1) |
| C1 | Duplicate-key rejection happens before the map is materialised | Parsing into a map first collapses the duplicate (§1) |
| C2 | The domain-separation tag is derived from the verification context, **never** read from the message | §1.1 — a tag taken from content lets the message choose its own role; pairs with S12 |

## D. Must-accept — over-strictness is non-conformance

| # | Input | Rule |
|---|---|---|
| D1 | The disavowal with unassigned in-range code 40 (`transactions.md`) | §4.3's banded exception: retained and evaluated by band |
| D2 | The adoption carrying unknown key `99: h'c0ffee'`, **with its full envelope** (`transactions.md`) | §1: preserved, re-serialised, and covered — the signatures verify over bytes including the unknown key; E10 is the mutation complement |
| D3 | A `LocationEvidence` method value of 9 | The location-method registry is deliberately open (§4.5) |
| D4 | A `Witness.attestation` with a reserved bit (3+) set | Reserved bits retained; interpret only 0–2 (§4.5) |
| D5 | Verifier responses in any array order | Array order is not canonicalised (§5.4) |
| D6 | The counter-jump `SignedLocator` pair, `[5, 42]` → `[5, 100]` (`primitives.md`) | Strictly greater, **not** previous+1 (§2.3) — contiguity checks reject valid supersessions |
| D7 | The reissue to a numerically smaller series, `0xDEADBEEF` → `2` (`transactions.md`) | `series` is an arbitrary label, never ordered (§2.3) — generation-counter implementations fail here |

## Resolved: the seed sentence is a writer commitment

**"A record carrying a keystream seed would be malformed" was untestable as
written, and the specification now states it as what it is** [author,
2026-09-01]: no conforming client writes a seed anywhere in a record, extension
keys included, where no validator could recognise one. By design there is no
fixture — the rule binds writers, and the security rests on the seed never
needing to leave the device.

## Scope

This suite is **wire-format/protocol interoperability**, not a certification
of client and operator behavioural commitments — those are deliberately
unenforceable from bytes (design §1.1) and live in the requirements documents.
Covered positively so far: the six transaction types **including peering**, the
formation-subtype presence record, `SignedLocator`, `EndpointRecord`. Not yet
covered: the remaining §7 standalone objects (currency attestation, anchor
entry, capture key grant, late response, subtree acknowledgement, resolution,
prekey distribution, archive fetch), session messages, catalog and abuse
objects, topology frames, resource requests. The boundary sweep covers **every
bound within the declared scope** — back-pointers 1/8/9, witnesses 16/17,
responses 32/33, path 24/25, unknown keys 16/17, values 1024/1025, u32/u64
edges, epoch saturation, ordinal transitions; bounds belonging to out-of-scope
objects (prekey blobs, catalog sizes, capabilities, sibling lists, audit
history, scope lists, corroborations, proximity channels, asserted locations,
recovery responses) join the sweep when their objects do.
