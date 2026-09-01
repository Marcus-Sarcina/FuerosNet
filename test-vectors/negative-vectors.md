# Negative and conformance vectors

**Draft. Spec-derived, unverified by an implementation.** See
[README.md](README.md).

**Rejection is not the only outcome, and a harness that models only
accept/reject will misreport conformance.** The specification distinguishes at
least five results, and each fixture below names the one it expects:

| Outcome | Meaning | Where stated |
|---|---|---|
| **malformed** | The bytes violate a structural rule; reject | `wire-format.md` §1, §3.2 |
| **unverifiable** | Sound encoding, but a required input (history, key material) is absent; neither valid nor invalid, and the missing party is named | §3.4, §5.5 |
| **incomplete** | A chain's presented records verify but a predecessor cannot be fetched; a fact for the caller, not a failure | §3.4 |
| **ineffective** | Structurally valid, grants or changes nothing for this evaluator | §6.8 |
| **accepted** | A conforming decoder MUST take it — over-strictness is its own non-conformance | §1, §4.3 |

Byte-level fixtures (section A) decide from the input alone. Context fixtures
(section B) need external state, and the fixture format for them is
`bytes + context + expected outcome`. Section C is method requirements —
obligations on how a decoder works, not inputs. Section D is must-accept.

## A. Byte-level — expected outcome: malformed

### Encoding layer (`wire-format.md` §1)

| # | Input | Violation |
|---|---|---|
| E1 | `18 0a` for the integer 10 | Non-shortest form. Valid: `0a` |
| E2 | `9f 05 18 2a ff` for `[5, 42]` | Indefinite-length array. Valid: `82 05 18 2a` |
| E3 | `a2 01 00 01 00` | Duplicate map key — reject **before materialising the map**, or the duplicate collapses and the check passes vacuously |
| E4 | `a2 02 00 01 00` | Protocol map keys out of ascending order |
| E5 | A peering body whose optional field 7 is present as `80` (empty array) | An OPTIONAL field whose value would be empty MUST be omitted, never encoded empty |
| E6 | A body carrying 17 unknown extension keys | Exceeds the 16-per-map bound |
| E7 | An unknown extension key whose encoded value is 1,025 bytes | Exceeds the 1,024-byte encoded-slice bound |
| E8 | A `ClientIntegrity.scheme` or comparable closed enumeration carrying an unassigned value | §1's unknown-enum rule: a value the decoder cannot interpret cannot be evaluated. **Disavowal reason codes and location methods are explicit exceptions** — see D1 and D3; a fixture for this rule must use a field that is *not* excepted |
| T8 | A disavowal with reason code **64** | Outside the 0–63 code space (§4.3). Malformed for that reason — **not** because it is unassigned; unassigned in-range codes are accepted (D1) |

### Primitives (§2)

| # | Input | Violation |
|---|---|---|
| P1 | seqno encoded as `18 2a` (a single integer) | seqno is `[series, counter]`, never one integer |
| P2 | seqno `[5, 4294967296]` | counter exceeds u32 range |
| P3 | Path nibble value 11 (`b` in the packed bytes) | Nibble values MUST be 0–9 |
| P4 | Path `{1: h'314155', 2: 5}` | Odd nibble count with nonzero trailing low nibble (`55` — the final low nibble MUST be 0) |
| P5 | Path `{1: h'31415000', 2: 5}` | Packed bytes ≠ `ceil(5/2)` = 3 — trailing surplus bytes are malformed, not ignorable |
| P7 | A `COSE_Key` in `KeyMaterial` carrying a fourth label (`kid`) | Exactly three labels per component — any addition changes the keyhash and therefore the identity |
| P8 | `KeyMaterial` ordered `[post-quantum, classical]` | Fixed order, classical first; reordering is a different identity, not an equivalent encoding |

### Envelope and signer set (§3.5, §3.6)

| # | Input | Violation |
|---|---|---|
| S1 | A `COSE_Signature` with a nonempty unprotected header | Malformed, not ignored — "ignored" applies to unknown map keys, never to COSE headers |
| S2 | `kid` placed in the unprotected header | Envelope entries carry `kid` in the **protected** header, covered by the signature |
| S3 | A signer group with one entry | Every logical signer contributes exactly two entries, one classical, one post-quantum |
| S4 | A signer group with two entries both `alg = -8` | Two entries of the same `alg` |
| S5 | Entries ordered PQ-before-classical within a group, or groups out of `kid` order | Canonical order: sort by `kid`, then classical before post-quantum. The order-divergence adoption in `transactions.md` is the positive complement — entry order there is *not* signer order |
| S6 | An adoption envelope with three logical signers | Exactly the signers the type requires — no extras, no duplicates |
| S7 | Envelope `{1: 2, …}` | Unknown schema version MUST be rejected, not best-efforted |
| S8 | A non-canonical envelope whose body verifies | Canonicality applies to the whole envelope; reject even when the body is valid |
| S9 | An embedded `COSE_Sign1` — a presence-context `VerifierResponse` (field 9) or a subject-consent signature — carrying a `kid` | Embedded objects omit `kid`: the surrounding structure names the signer, and a second copy could disagree with the first. **The old-key proof inside `Recovery` is a hybrid `COSE_Sign`, not a `COSE_Sign1`** (§4.1); the same omission rule reaches its contained entries, since `Recovery` field 1 names the prior key |

### Presence-record structural rules (§3.2, §3.3)

| # | Input | Violation |
|---|---|---|
| R1 | The two participant identities equal | MUST differ |
| R2 | `finalized_at` < `started_at` | `finalized_at` MUST be ≥ `started_at` |
| R3 | `finalized_at` − `started_at` = 86,401 s | MUST NOT exceed 24 hours — the structural bound on chronology poisoning |
| R4 | A normal-subtype record with zero witnesses | Zero witnesses is the formation case and nothing else |
| R5 | The same witness keyhash twice | Witness identities MUST be distinct |
| R6 | A participant listed among the witnesses | A participant witnessing their own ceremony is not a witness |
| R7 | A witness whose `nominated_by` is neither participant | MUST be one of the two participants |
| R8 | A formation record whose key 0 is not exactly `[SHA-256(signer keyhash)]` per signer | A formation record's back-pointers are the genesis value, structurally |
| R11 | A revealed `proximity` whose `strongest` names a channel without `result = pass`, or with a higher-ranked passing channel present | The one structural rule checked only on reveal; withheld, the record reports **unverifiable** for this rule — never valid and never malformed |

### Transaction-specific (§4)

| # | Input | Violation |
|---|---|---|
| T1 | A `NetworkPoint` with a 16-byte address | v1 demands IPv4; 16 bytes is malformed, not "IPv6 accepted early" |
| T2 | A `NetworkPoint` with port 0 | Zero is never a destination |
| T3 | An adoption whose carried `KeyMaterial` hashes to something other than field 1's keyhash | The hash MUST equal the keyhash of the party described, or a recipient pins the wrong key |
| T4 | A `Recovery` whose response's field 8 ≠ the claimed prior key | Evidence about one old identity embedded under a claim about another |
| T5 | A `Recovery` response whose `subject` ≠ the newly adopted node | The verifier is attesting continuity *to* the new key |
| T6 | Two responses from one verifier **for one subject** in one record | One verifier occupies one `(subject, verifier)` slot (§5.5). **One identity answering once for each participant is two slots and is valid** — a verifier may have met both |

## B. Context-dependent — bytes plus external state

| # | Input | Context | Expected outcome |
|---|---|---|---|
| P6 | A bare `Locator` | Presented standalone (at introduction) | **malformed** — standalone carriage requires `SignedLocator` |
| P6b | The same bare `Locator` | Inside a transaction body | **accepted** — the envelope signature authenticates it |
| R9 | A structurally perfect formation record for a key | Evaluator holds that key's real chain, which has a predecessor | **Fork detected** — evidentiary, per §3.2: the forgery is visible against the real chain |
| R9b | The same record | Evaluator holds no history for the key | **accepted** structurally — §3.2 says a validator holding no history cannot tell, and the forgery's weakness is evidentiary, not structural |
| R10 | A record whose `started_at` precedes its committed predecessor's effective time | The predecessor is supplied and verifies | **malformed** — §3.3's monotonicity |
| R10b | The same record | The predecessor cannot be fetched | **incomplete** — §3.4: a fact for the caller, not a malformed transaction |
| V1 | A presence record needing threshold recomputation for both subjects | Evaluator holds one subject's bundle only | **valid for one subject, unverifiable for the other** — per-subject validation, §5.5; collapsing the two overclaims what was checked |
| V2 | A catalog-entry scope naming a position the evaluator cannot compute | Evaluator's topology does not reach it | **ineffective** — structurally valid, grants nothing, MUST NOT be rejected (§6.8) |

## C. Method requirements — how a decoder works, not what it reads

| # | Requirement | Rule |
|---|---|---|
| E9 | Validate the bytes as received; a decode-then-re-encode equality check is non-conforming | It erases the evidence it is meant to find — duplicate keys collapse, unknown-key encodings are lost (§1) |
| C1 | Duplicate-key rejection happens before the map is materialised | A decoder parsing into a map type first has already collapsed the duplicate (§1) |

## D. Must-accept — over-strictness is non-conformance

| # | Input | Rule |
|---|---|---|
| D1 | The disavowal with unassigned in-range code 40 (`transactions.md`) | §4.3's exception: in-range codes are retained and evaluated by band, never rejected |
| D2 | The adoption carrying unknown extension key `99: h'c0ffee'` (`transactions.md`) | §1: bounded unknown keys are preserved, re-serialised, and covered by txid and signatures |
| D3 | A `LocationEvidence` method value of 9 | The location-method registry is deliberately open (§4.5): unassigned values are retained and left uninterpreted |
| D4 | A `Witness.attestation` with a reserved bit (3+) set | Reserved bits are retained; a decoder interprets only bits 0–2 (§4.5) |
| D5 | Verifier responses in any array order | Deterministic CBOR does not order array elements; different orders are different valid bodies (§5.4) |

## Specification testability gap, flagged for the author

**"A record carrying a keystream seed would be malformed" (§4.1) cannot be
turned into a decoder test.** Unknown extension keys are preserved by rule, and
a decoder cannot know that an unknown 32-byte value is semantically a seed —
so the sentence is unenforceable as written (design §1.1's own test). Either a
reserved key range makes it checkable, or the sentence is a client commitment
about what conforming *writers* emit, and should say so. Until ruled on, no
fixture exists for it (the former T7).

## Not covered here

Session messages, catalog and abuse objects, topology frames, and resource
requests have their own reject rules — out of scope for this draft along with
their positive vectors, and the boundary sweep (back-pointers 8/9, witnesses
16/17, responses 32/33, path 24/25, unknown keys 16/17, values 1024/1025) is
queued in README's canonical bar.
