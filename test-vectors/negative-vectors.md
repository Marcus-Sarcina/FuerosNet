# Negative and conformance vectors

**Draft. Spec-derived, unverified by an implementation.** See
[README.md](README.md).

**Pinned**: wire-format.md `a78aff6252a3b05aff6ac7e99eb74c10b5a87a8359314a7e5d1636a2455707a8` · network-design.md `a3061f8f785f0105bd269bff71b16df478e1e311316875872c11f9179d228914`

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
| `signatures[signer, alg]` | verified · **failed** · unverifiable(key) — per signer **and algorithm**; a bad signature and a missing key are different outcomes, and E10 needs the first | §3.4, §3.5 |
| `chain` | complete · **incomplete** — an unfetchable predecessor is a fact, not an error | §3.4 |
| `weight[subject]` | the evaluator's own recognition of that subject's responders — never a validation verdict, and never recomputable by a distant party (§5.7). *Replaced `selection[subject]` when deterministic selection was retired, 2026-09-01* | §5.5, §5.7 |
| `checks[name]` | pass · fail · unverifiable(reason) — per named rule **and per reference evaluation**: the `strongest`-channel check when revealed; `checks[proof_of_presence]` and every other dereference (§3.4's evaluation step) — `fail` when the referenced object does not establish what the citing object claims, `unverifiable(unfetchable)` when it cannot be fetched. `effective` stays topology/grant evaluation; `chain` stays predecessor history — neither absorbs reference evaluation, by decision (seventh review) | §3.2, §3.4, §4.5.1 |
| `effective` | yes · **no** — structurally valid may still grant nothing for this evaluator | §6.8 |
| `evidentiary` | free-form flags (fork-detected, …) — findings against history, not against the object | §3.2 |
| `state_action` | install · replace · replay · ignore_stale · **conflict** · incomparable — what a holder's store does with an individually valid freshness-bearing record (§2.3, §7.6). The dimension the seqno fixtures constrain: D6 → replace, D12 → replay, V6/V12 → conflict, V11 → incomparable, a lower-counter same-series record → ignore_stale (ninth review) | §2.3, §7.6, §7.7.3 |

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
| E10 | The extended adoption (`transactions.md`) with its unknown key's value mutated `c0ffee` → `c0ffef` | Unknown retained keys are covered by every signature: **all four entries fail**, both algorithms — verified at generation time under two independent implementations. The unmutated envelope is D2 |
| E11 | A merge back-pointer list out of ascending bytewise order | §3.1: one logical merge, one encoding, one txid — the positive merge vector shows the sorted form |
| T8 | A disavowal with reason code **64** | Outside the 0–63 code space (§4.3) — malformed for that reason, **not** because it is unassigned; unassigned in-range codes are accepted (D1) |
| E19 | A `bstr`, `tstr`, array or map with a non-shortest **length** header (`58 05` for a 5-byte string) | §1's shortest-form rule covers every header, not only integer values — a decoder canonicalising integers alone accepts these (ninth review) |
| E20 | An indefinite-length map, byte string or text string | E2's rule across every major type — a decoder rejecting indefinite arrays can still accept indefinite maps |
| E21 | A `tstr` carrying invalid UTF-8 | Major type 3 is defined over UTF-8; undecodable text is malformed, not lenient-decoded |
| E15 | A `Locator` missing its `seqno` (key 3) | Required-field matrix: a decoder that defaults missing fields passes every present-field vector |
| E16 | A transaction `timestamp` encoded as a byte string | Wrong-major-type matrix: coercive decoding is silent divergence |
| E17 | A `NetworkPoint` missing its address (key 1) | Required-field matrix |
| E18 | An `EndpointRecord` missing its signature (key 4) | Required-field matrix — absence of the one field that authenticates the rest |
| E12 | An otherwise-valid body carrying map key `-1` or `"x"` | §1: protocol map keys are unsigned integers — an arbitrary CBOR key is malformed, **not** an "unknown extension key", which is always a uint. A generic CBOR map type preserves either indistinguishably |
| E13 | The extended `EndpointRecord` (`records.md`, D8) with its extension value mutated `c0ffee` → `c0ffef` | §1's coverage rule on the **standalone `COSE_Sign1` path**: the unknown key is inside the reconstructed payload, so the signature fails — the Sign1 analogue of E10 |
| E14 | The extended `SignedLocator` (`primitives.md`, D9) with key 4's value mutated | Same rule, and the fixture that catches a slot-inference bug from the other side: the mutated key must have been in the payload for the signature to fail |
| S18 | An envelope with type **6** | Retired: the schema is removed and nothing remains to validate *as* (§4's tombstone). The number is reserved, the bytes are not readmitted |
| S19 | An envelope with type **9** | Unassigned type value — §1's unknown-enum rule on field 2 |

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
| P9 | `KeyMaterial` with one component, or three | Exactly two, classical then post-quantum (§2.2) |
| P15 | An adoption whose key 0 carries one back-pointer list, or three | Exactly **one list per required signer** (§3.1) — two for an adoption |
| P16 | A departure whose key 0 carries two lists | One required signer, one list (§3.1) |
| P17 | A back-pointer entry of 31 or 33 bytes | Each entry is a 32-byte transaction hash (§3.1) |
| P18 | seqno `[5]` or `[5, 42, 7]` | Exactly two elements, `[series, counter]` (§2.3) — the arity complement of P1's scalar case |
| P10 | A component missing a required label (`x` absent; `pub` absent) | RFC 9053 / RFC 9964 requirements, restated by the profile (§2.2) |
| P11 | Wrong parameter values: classical `kty ≠ 1` or `crv ≠ 6`; post-quantum `kty ≠ 7` or `alg ≠ -49` | The profile fixes all four (§2.2); a generic COSE library accepts other curves and algorithms |
| P12 | A post-quantum component carrying `priv` (label −2) | RFC 9964 forbids it in a public key, and the profile inherits the rule (§2.2) |
| P13 | Classical `x` not exactly 32 bytes | An Ed25519 public key is 32 bytes; any other width is not one |
| P14 | Post-quantum `pub` not exactly 1,952 bytes | ML-DSA-65's public-key size; another width is another parameter set or garbage (§2.2) |

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
| S15 | A `COSE_Sign` whose **outer** protected header is nonempty | §3.5: the outer protected header is empty — a `COSE_Sign` carries no signature of its own, so it has no algorithm to name; generic libraries permit content there |
| S16 | A `COSE_Sign` whose **outer** unprotected header is nonempty | §3.5's nonempty-unprotected rule covers every COSE header, the outer pair included |
| S24 | A protected header whose **embedded map** (inside its `bstr`) is non-canonical | Canonicality reaches the serialisation inside the header bstr — outer-envelope canonical parsing does not establish it (ninth review) |
| S17 | A disavowal signed by field 2's identity (the subordinate) instead of field 1's — right entry count, right algorithms, wrong party | Signer role is inferred by comparing `kid` to the body's fields (§3.5), and the type requires exactly its own signers: the correct *shape* from the wrong *identity* is malformed. Every type needs this fixture with a real second identity (canonical bar) |
| S20 | A `SignedLocator` or `EndpointRecord` whose `COSE_Sign1` protected header carries a `kid` | The surrounding object names the signer (§3.5); a second copy could disagree with the first. A decoder with separate `COSE_Sign`/`COSE_Sign1` paths can enforce this on one and not the other |
| S21 | A standalone `COSE_Sign1` with a nonempty unprotected header | §3.5's rule covers every COSE header — the Sign1 path included, not only envelope entries |
| S22 | A `SignedLocator` or `EndpointRecord` signed with `alg = -49` | Structurally a valid algorithm, **context-forbidden**: both objects are classical-only by profile (§2.3, §7.6) — their relevance expires with the next update, so the post-quantum horizon does not apply |
| S23 | The wrong-signer `SignedLocator` of `primitives.md` — field 1 names alice, the signature is bob's and cryptographically valid | The binding is the defect: the signature MUST verify under **the key the object names**, never under whatever key it happens to verify under. Generated bytes. **The analogue for every remaining standalone signed object landed 2026-09-02** (`records.md`, canonical bar 10): currency, catalog, abuse, anchor, subtree-ack, prekey — the subtree-ack's wrong signer is the patron, exactly the party that must not substitute for the grandpatron |
| S24 | Any standalone signature verified under another context's `external_aad` — e.g. the currency attestation's under `rhtn/1:anchor` | §1.1's domain separation is load-bearing, not decorative: the harness checks every `records.md` signature fails under a neighbouring tag. An implementation sharing one hard-coded AAD across two real paths passes S12's empty-AAD case and fails here |

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
| R12 | A formation-subtype record carrying a witness array | Formation records omit fields 4 and 5 entirely (§3.2); their absence is half of what separates a bootstrap from an ordinary meeting |
| R13 | A formation-subtype record carrying verifier responses | Same rule, field 5 |
| R14 | ~~window-ordinal mismatch~~ | **Retired 2026-09-01** with body key 7 and deterministic selection; the id is not reused. *The note this row briefly carried — key 7 as a preserved unknown extension — was superseded 2026-09-02 by the tombstone ruling*: a decoder meeting a retired number REJECTS (§4.5, T29) |
| R11 | A revealed `proximity` whose `strongest` lacks `result = pass`, or with a higher-ranked passing channel | **Revealed and violated → `structural = malformed`** — it is a structural rule like any other, and §3.2 now says so explicitly. Withheld → `checks[strongest] = unverifiable(withheld)`, never valid and never malformed. *An earlier revision of this row said the revealed case was a failed check rather than malformed — the sixth review caught the drift* |

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
| T15 | An old-key successor statement whose `patron_key` ≠ the enclosing adoption's field 2 | The symmetric assembly error: §4.1 requires a verifier to check **both** bindings and reject on mismatch — the right successor under the wrong patron is still a forged assembly |
| T16 | A series reissue whose field 4 counter is not 0 | §4.6: the new series opens at counter 0 |
| T24 | A non-presence transaction whose `timestamp` precedes a committed predecessor's effective time — the temporal bridge | §3.3's monotonicity now binds **every** type; a merge variant where only one head violates the bound MUST also reject |
| T17 | An `EndpointRecord` listing the same `NetworkPoint` twice | §7.6: entries are distinct; a repetition is malformed — it expresses nothing the order does not already say |
| T18 | A `NetworkPoint` with port 65536 | u16 range (§4.4); 65535 is the valid ceiling and MUST be accepted |
| E22 | A `NetworkPoint` writing the default port out — `3: 7431` | §1's default-omission rule [author, 2026-09-01]: a field equal to its stated default MUST be omitted; omission is the one spelling, which is what keeps §7.6's distinct-entries rule decidable |
| T13 | Any two-party transaction whose two identity fields are equal — adoption, departure, disavowal, peering, series reissue | The degenerate pair is rejected across every two-party type (§4.1), as equal participants already are for presence (§3.2, R1). Self-adoption is also the degenerate cycle — the one a validator sees from the record alone (design §6.2.5) |
| T14 | A `Recovery` whose `prior_key` equals the enclosing adoption's field 1 | A same-key Recovery is vacuous evidence (§4.1) — the retained-key, lost-archive case is served by archive fetch, fresh adoption and merge, never by Recovery |
| T25 | A **non-recovery** adoption carrying neither field 8 nor field 9 | design §6.1.1: every adoption rests on a proof of presence, a former patron's countersignature, or — where it is a recovery — its own `Recovery` block. None of the three is not a weak adoption, it is a malformed one |
| T26 | An adoption carrying **more than one** of fields 6, 8 and 9 — including a recovery that also names a presence record | They are alternatives, not a floor and a bonus (§4.1). An object offering two answers to one question invites a validator to pick, and two validators may pick differently |
| T27 | A `TransferStatement` whose `node_key` ≠ the enclosing adoption's field 1 | The countersignature would vouch for a different node than the one being adopted (§4.1) |
| T28 | A `TransferStatement` whose `new_patron_key` ≠ the enclosing adoption's field 2 | The assembly error T12 and T15 close for recovery, one field over: a countersignature naming no destination — or the wrong one — authorises moves its signer never approved |
| T29 | A `Transfer` whose `former_patron_key` equals the enclosing adoption's field 2, or its field 1 | Identical keys represent no transfer, the rule `prior_key` carries in a `Recovery` (§4.1); and a node does not vouch for its own move |
| T30 | A `Transfer` field 2 that is a `COSE_Sign1`, or a `COSE_Sign` with a single entry | The former patron is hybrid: one logical signer, two entries (§3.5). The same shape as T11 |
| T31 | A peering record with no field 8 | design §6.3: peering is always priced in a meeting, and §4.1's field-9 alternative cannot apply — peers share no prior relationship |

### VerifierResponse conditional-field matrix (§4.5)

`basis` and `template_version` are conditionally required, and the conditions
are load-bearing — a verifier who has not evaluated asserts no basis, and a
photo comparison without its template version is unverifiable as evidence:

| # | Input | Violation |
|---|---|---|
| T19 | `result` 0–2 with `basis` absent | `basis` REQUIRED for evaluated results |
| T20 | `result` 3 (`unavailable`) with `basis` present | MUST be absent — no truthful value existed. *There is no result 4: `pending` left the enum on 2026-09-01 — an unreachable verifier's slot is absent, and a response claiming result 4 is an unknown enum value, rejected per E8's rule* |
| T21 | `basis` 0 or 2 with `template_version` absent | REQUIRED for photo bases |
| T22 | `basis` 1 (or absent) with `template_version` present | MUST be absent — there is no template in personal knowledge |
| T23 | ~~response from an unselected verifier~~ | **Retired 2026-09-01**: no selected set exists apart from the selector's judgment (§5.5); any consenting-queried verifier's response is structurally fine. The id is not reused |
| T27 | A response whose `selection_basis` (field 10) is absent, or carries a value outside 0–3 | §5.5: required, closed enumeration — the selector's claim of met / in-horizon / reachable / discretionary. Tier-aligned renumbering 2026-09-03: met and merely-in-horizon are different security facts (A23) and the record retains the difference |
| T29 | A presence body carrying retired key 7, or a `Witness` carrying retired key 4 or 5 | §4.5 [2026-09-02]: retired numbers are tombstones, not extension space — a decoder meeting one rejects, the §4 type-6 rule. An unknown key is one the schema never assigned; a retired key is one it remembers |
| T30 | A normal presence record whose key 5 is an **empty array** | §4.5 [2026-09-02]: zero responses omit the key — the empty-array spelling gives one logical record two encodings. The over-strictness complement: an implementation reading field 5 as required-with-empty emits this and rejects the valid absent spelling |
| T31 | A normal presence record none of whose witnesses set both `protocol_ran` and `both_responsive` | §3.2 [author, 2026-09-03]: the witness floor counts only entries with bits 0 and 1 both set — a witness attesting nothing (or latency alone) is partial evidence, present but not corroborating, and a normal record with no affirmative witness fails. The uncorroborated meeting remains expressible as a formation record, visible as what it is. `latency_bound` (bit 2) stays genuinely optional: the existing bits-3 fixtures are positives |
| T25 | A response whose `subject` names neither participant | §5.5's binding: the subject must be one of the record's two participants — with history held, malformed |
| T26 | A response transplanted under a `query_id` the subject never countersigned | §5.5's binding: the query_id must match one the subject consented to — a valid response to a different query is a forged slot |
| T28 | A presence record or `Recovery` block whose verifier responses are not sorted ascending by verifier keyhash, ties by ascending subject keyhash | §4.5/§4.1 [2026-09-02]: the witness rule — one set, one encoding; the tie is D15's one-verifier-both-participants case. An implementation preserving assembly order emits bytes a validator rejects |

### What a client hands its serving node (§7.10)

The four submission types carry two conditional rules, and both are the
decoder's rather than a node's policy:

| # | Input | Violation |
|---|---|---|
| T32 | A `WakeRegistration` with field 2 absent and field 3 or field 4 present | §7.10: a withdrawal carries no key and no lapse. The key and the lapse describe an endpoint, so either without one is a shape with no meaning |
| T33 | A `WakeRegistration` carrying field 2 with field 3 absent | §7.10's mirror: the body is encrypted to the key before it is posted, so an endpoint with no key is an endpoint nothing can be sent to |
| T34 | A `PrekeyPublication` whose field 1 is a byte string wrapping the bundle rather than the bundle map | §7.10 declares field 1 as `PrekeyBundle`, the object §7.8 defines, spliced in as `PrekeyReply` field 2 splices it. The wrapping round-trips against itself and fails against every conformant peer |
| T35 | An `OneTimeDeposit` whose field 1 is an empty array, or carries more than 256 entries, or holds an entry that is not a byte string | §7.10's `[ 1*256 bstr ]` |
| T36 | A `SubmissionReply` whose field 2 is outside 0–2 | §7.10: accepted, refused, over a bound, and §1's unknown-enum rule on a closed enumeration |
| T40 | A `RelaySubmission` without field 4 | §7.10 [2026-09-22]: a session is with a device, and the node cannot tell from the ciphertext which; the recipient device is required |
| T41 | A `PrekeyRequest` with field 2 = 1 and field 4 absent | §7.8 [2026-09-22]: a one-time key is consumed from one device's pool, so the device is required when one is asked for |
| T42 | A `PrekeyReply` whose field 2 carries nine bundles | §7.8: at most eight, a chosen ceiling beside design §23.3's count of about three |
| T43 | An `ArchiveRequest` whose field 2 is an empty array | §7.9 [2026-09-17]: the frontier is `[ + txid ]`; absent means the holder's newest record, empty means nothing |
| T44 | An `ArchiveReply` with field 4 present and field 3 false | §7.9: the frontier is present iff more remain |

### Transport delegation (§8.2)

The credential an instance presents in place of a seed; a decoder's checks
before any receiver's clock or pin is consulted:

| # | Input | Violation |
|---|---|---|
| T45 | A `Delegation` whose field 4 is not exactly 172,800 seconds after field 3 | §8.2 [author, 2026-09-21]: the window is exactly 48 hours, and a window an issuer could lengthen would put the seed back on the box under another name |
| T46 | A `Delegation` whose field 5 carries one signature entry | §8.2: hybrid because the delegating identity is; a classical-only delegation is malformed |
| T47 | A `Delegation` whose field 5 verifies under an identity other than field 2 | §8.2: the delegating identity signs; the wrong-signer analogue in `records.md` |
| T48 | A `CurrencyAttestation` whose field 8 names a key other than the one that signed field 7 | §7.1 [2026-09-22]: a verifier checks field 7 against the key field 8 names |

### `CatalogEntry` field widths (§6.1)

An entry is owner-signed and re-served byte for byte, so a width one side
admits and another refuses propagates rather than stopping where it arrived:

| # | Input | Violation |
|---|---|---|
| T37 | A `CatalogEntry` whose field 3 is empty or over 64 bytes, or field 4 empty or over 128 | §6.1's `tstr .size` bounds on service type and instance name |
| T38 | A `CatalogEntry` whose field 5 is empty or over 256 bytes, or field 7 present and empty or over 1024 | §6.1's `bstr .size` bounds on the connection endpoint and the metadata analogue |
| T39 | A `CatalogEntry` whose field 1 or field 2 is not 32 bytes | §6.1: both are keyhashes, and the total-size bound does not imply the field widths |

## B. Context-dependent — bytes plus external state, structured result

| # | Input | Context | Expected result |
|---|---|---|---|
| P6 | A bare `Locator` | Standalone, at introduction | `structural = malformed` — standalone carriage requires `SignedLocator` |
| P6b | The same bare `Locator` | Inside a transaction body | accepted — the envelope authenticates it |
| R9 | A structurally perfect formation record | Evaluator holds the key's real chain | `structural = valid`, `evidentiary = fork-detected` (§3.2) |
| R9b | The same record | No history held | `structural = valid` — a no-history validator cannot tell (§3.2) |
| R10 | `started_at` precedes the committed predecessor's effective time | Predecessor supplied and verified | `structural = malformed` (§3.3) |
| R10b | The same record | Predecessor unfetchable | `chain = incomplete` — not malformed (§3.4) |
| V1 | ~~per-subject selection recomputation~~ | — | **Retired 2026-09-01** with deterministic selection; per-subject evaluation survives as `weight[subject]` — recognition, not recomputation (§5.7). The id is not reused |
| V2 | A scope naming a position the evaluator cannot compute | Topology does not reach it | `structural = valid`, `effective = no` — MUST NOT reject (§6.8) |
| V3 | A signer's key material never seen and no resolver | Any signed object | `signatures[signer] = unverifiable(key)` — a third outcome; collapsing it into invalid discards a fetchable object (§3.4) |
| V4 | A series reissue naming a series the node previously occupied | Evaluator holds the node's chain | **Reject** — §4.6: a chain-holder MUST reject a reissue naming a series already in the chain; reuse brings the abandoned line's high counters back into comparison |
| V5 | ~~witness commitment mismatch~~ | — | **Retired 2026-09-01**: witness nonce commitments left the schema with deterministic selection. The id is not reused |
| V6 | The two `EndpointRecord`s of `records.md`'s conflict pair | Both held | **Malformed condition, not a tie** — equal `seqno`, different contents (§7.6, §7.7.3); a reader MUST NOT prefer either. The pair can only mean equivocation or a key in two hands. **Aftermath ruled 2026-09-02** (wire §10.1): malformed names the PAIR — on discovery the holder retains neither as current, forwards nothing further for that `(subject, seqno)`, and repairs by re-resolution; first-wins would let arrival order split the network's view. `state_action = conflict` |
| V7 | A normal presence record carrying fewer responses than §5.2's criterion suggests | The subject's bundle, for *n* | **Accepted** — the criterion sizes diligence and gates nothing (§5.2, §5.5): a thin response set is visible evidence weight, not a defect. A decoder enforcing a response minimum rejects valid records and is non-conforming. **Instantiated 2026-09-02**: `transactions.md`'s finalization must-accepts — a record finalized on a lone no-match, and one with key 5 absent entirely |
| V8 | A perfectly valid, correctly signed envelope supplied in answer to a request for a **different** txid | The requested txid | **Content-address mismatch** — `chain` cannot advance through it. §5.3 counts a record only when canonicality, **content address**, and signatures all check; this isolates the txid recomputation from signature verification, catching an implementation that trusts its storage index instead of hashing what it received |
| V9 | The optionals-exercised adoption (`transactions.md`), field 8 dereferenced | The normal alice–bob presence record (bar 2, 2026-09-02) | `structural = valid`, **`checks[proof_of_presence] = pass`** — the record verifies and names exactly these two parties (§3.4) |
| V9a | The divergence adoption (`transactions.md`), field 8 dereferenced | The referenced alice–carol formation record | `structural = valid`, **`checks[proof_of_presence] = fail`** — the record exists and verifies but names alice–carol, not this adoption's pair (§3.4: dereference confirms the record *names these two parties*) |
| V9b | The same adoption | The referenced record unfetchable | `structural = valid`, `checks[proof_of_presence] = unverifiable(unfetchable)` — the §3.4 posture: neither confirmed nor failed |
| V10 | A valid reissue followed by an otherwise-valid high-counter record in the **abandoned** series | The reissue chain held | **Reject the old-series record at any counter** — §4.6.1: a chain-holder knows which series were abandoned and MUST reject records in them whatever their counter. A different bug from V4's reuse: here the old record, not the reissue, is the attack |
| V11 | Two current-looking records in **different** series for one subject | No reissue chain held | **Neither ranks** — series are unordered (§2.3); currency is proved by the presented chain (§4.6.1), never by comparing series values. An implementation picking the numerically larger series fails here. **Storage posture ruled 2026-09-02** (wire §10.1): an unproved-series record is neither stored nor forwarded until its series proves current — so this pair pends or drops, and re-resolution repairs. With a chain for **each** — two patron relationships, one record per line (§7.6) — holding both is the correct end state, not a conflict. `state_action = incomparable` |
| V12 | The counter-jump `SignedLocator` and its conflict partner (`primitives.md`) — same subject, same `[5,100]`, different paths | Both held | `state_action = conflict` — the **locator-path** analogue of V6: equal `seqno`, different contents, no tie to break (§2.3, §7.7.3). Separate decoding routes need the rule separately. Same aftermath as V6 (wire §7.7.3, 2026-09-02): retain neither, re-resolve |

## C. Method requirements — how a decoder works

| # | Requirement | Rule |
|---|---|---|
| E9 | Validate the bytes as received; decode-then-re-encode equality is non-conforming | It erases the evidence it should find (§1) |
| C1 | Duplicate-key rejection happens before the map is materialised | Parsing into a map first collapses the duplicate (§1) |
| C2 | The domain-separation tag is derived from the verification context, **never** read from the message | §1.1 — a tag taken from content lets the message choose its own role; pairs with S12 |
| C3 | ~~rank tie-break comparator~~ | **Retired 2026-09-01** with hash-rank sampling. The id is not reused; the *unit fixture class* it motivated stays in the corpus schema |

## D. Must-accept — over-strictness is non-conformance

| # | Input | Rule |
|---|---|---|
| D1 | The disavowal with unassigned in-range code 40 (`transactions.md`) | §4.3's banded exception: retained and evaluated by band |
| D2 | The adoption carrying unknown key `99: h'c0ffee'`, with its full envelope (`transactions.md`) | §1: preserved, re-serialised, and covered — `structural = valid` and **all four signatures** verify over bytes including the unknown key. E10 is the mutation complement |
| D3 | A `LocationEvidence` method value of 9 | The location-method registry is deliberately open (§4.5) |
| D4 | A `Witness.attestation` with a reserved bit (3+) set | Reserved bits retained; interpret only 0–2 (§4.5) |
| D5 | ~~Verifier responses in any array order~~ | **Retired 2026-09-02**: responses sort ascending by verifier keyhash — the witness rule (§4.5, §4.1) — so arbitrary order is now T28's malformed case. The §5.5 citation was also an overreach: no such non-canonicalisation sentence existed. The id is not reused |
| D6 | The counter-jump `SignedLocator` pair, `[5, 42]` → `[5, 100]` (`primitives.md`) | Strictly greater, **not** previous+1 (§2.3) — contiguity checks reject valid supersessions |
| D7 | The reissue to a numerically smaller series, `0xDEADBEEF` → `2` (`transactions.md`) | `series` is an arbitrary label, never ordered (§2.3) — generation-counter implementations fail here |
| D8 | The `EndpointRecord` carrying unknown key `99: h'c0ffee'` (`records.md`) | §1: preserved and **covered on the standalone `COSE_Sign1` path** — the signature verifies over the payload including the unknown key. E13 is the mutation complement |
| D9 | The `SignedLocator` carrying unknown key **4** (`primitives.md`) | The extension sits directly above the signature slot: an implementation inferring the slot from key magnitude misreads it. Schema-fixed slots, never inference. E14 is the mutation complement |
| D10 | A presence record with `finalized_at == started_at` | The exact boundary of R2's rule: ≥ admits equality |
| D11 | A presence record with `finalized_at − started_at` exactly 86,400 s | The exact boundary of R3's rule: 24 hours is the last admissible gap; 86,401 (R3) is the first malformed one |
| D12 | The identical `EndpointRecord` received twice — same `seqno`, same contents | §7.6: republishing an unchanged set replays the record; idempotent reconciliation, **not** V6's equal-`seqno` conflict, which requires differing contents |
| D13 | The root's self-anchored `SignedLocator` (`primitives.md`) — anchor = the node itself, path `{1: h'', 2: 0}` | **Roots legitimately self-anchor** (§2.1) [author, 2026-09-01]: the empty path is the zero-hop case, and a decoder asserting a minimum path length rejects every root's locator |
| D14 | A normal presence record in which one identity is both a witness (envelope signer) and a verifier (embedded response) | One identity, one logical signer per capacity (§3.2) — the roles are different objects, and rejecting the overlap is over-strict |
| D15 | One verifier answering once for **each** participant — two `(subject, verifier)` slots | §5.5: only duplicate slots are malformed; a verifier may have met both (T6's positive complement). The two slots sort by ascending subject keyhash (§4.5's tie rule, 2026-09-02) |
| D16 | A record whose witnesses all carry the same `nominated_by` | Wire-valid (§3.2 requires only that each names a participant); the nomination split is the reference client's warning, never a validity condition (`light-client-requirements.md` §1.0) |
| D17 | The counter-jump pair read as state: `[5,42]` then `[5,100]` | `state_action = replace` — and the reverse order is `ignore_stale`, not an error: absence of prior state is acceptable and staleness is ordinary (§2.3) |
| D18 | A bounded unknown extension whose value is a tagged item, float, or any other deterministic CBOR item outside the schemas' types | §1 [author, 2026-09-01]: **opaque encoded slices, preserved and never interpreted** — uninterpretable state kept for a reader that may understand it later. An implementation reconstructing extensions through a typed model drops what it cannot type, and fails here |
| D19 | A `Proximity` disclosure carrying the same channel kind twice — failed, then retried and passing | §4.5 [2026-09-02]: a kind may repeat; a retried channel is two measurements, both evidence. A validator imposing one-entry-per-kind rejects a valid record (`P-channel-retry`) |
| D21 | The recovery adoption (`transactions.md`), which carries **neither** field 8 nor field 9 | Its evidence is field 6 (design §6.1.1): a recovery's presence half is embedded, not referenced. A decoder demanding field 8 on every adoption rejects every recovery |
| D22 | A `CurrencyAttestation` whose field 2 differs from field 1 (`P-currency-successor`) | §7.1 [author, 2026-09-18]: field 2 is what the issuer vouches is live, a rotation-or-fork signal, never a redirect; a decoder that required equality would remove fork detection from the currency path (CUR-20) |
| D23 | A `CurrencyAttestation` signed under the issuer's identity with no field 8 (`records.md`) | §7.1: field 8 is present iff the signature is under a delegated key; an issuer holding its seed carries none |
| D24 | An `Attach` with field 4 absent, or an `AttachAck` with field 6 absent | §8.2: the seed-holding device presents its identity's classical member and owes no delegation |
| D25 | An `ArchiveReply` whose field 2 holds an array beside maps (`P-archive-reply-presented`) | §7.9: a presence record arrives presented; a map is an envelope and an array a presentation, no discriminator |
| D20 | An adoption carrying a valid field 9 `Transfer` and **no** field 8 (`transactions.md`) | design §6.1.1: the countersignature is a required alternative, not a lesser one. A decoder that demands a presence record on every adoption rejects every lateral and vertical shift (§6.2.3) |

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

**Covered positively**: body and txid vectors for the six transaction types;
full envelopes for the adoption family, the formation and normal presence
records, the finalization must-accepts, the alice–carol pair and the recovery
adoption; every standalone signed context with its wrong-signer analogue;
every unsigned message family (`messages.md`); presentations of both records;
and the machine-readable corpus (`corpus.json`) carrying all of it under
stable ids.

**The boundary sweep is present** (canonical bar 7, 2026-09-02): `B-*`
corpus entries instantiate the list below at and past each bound, with the
over-identity-set counts and 100 KB frame bounds as unit recipes. The target
list is
**every bound belonging to a covered object**, which includes bounds of
covered objects' fields previously mislabelled out-of-scope: back-pointers
1/8/9, witnesses 16/17, responses 32/33, path 24/25, unknown keys 16/17,
extension values 1024/1025, **peering audit history 8/9 and `NetworkPoint`
lists 0/1 and 8/9 per endpoint record** (the list is `1*8` — both bounds have
an invalid neighbour), the `NetworkPoint` port at 65535/65536,
u32/u64 edges, epoch saturation, ordinal transitions, **the presence
finalization gap at 0 / 86,400 / 86,401 seconds (D10/D11/R3)**, and
fixed-width keyhash fields at 31/32/33 bytes wherever one appears. Bounds of genuinely uncovered objects (prekey blobs, catalog
sizes, capabilities, sibling lists, scope lists, corroborations, proximity
channels, asserted locations, recovery responses) join when their objects do.
