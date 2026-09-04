# Test vectors — DRAFT

**Status: spec-derived; five clean-room review rounds by a second model
family (2026-08-31 through 2026-09-01, each reproducing the generator's output
byte-for-byte and finding no arithmetic error); verified by no independent
implementation.** These vectors were written from the
specifications alone, which is exactly the condition `wire-format.md` §13 warns
about: *vectors written from the spec alone encode the spec's own mistakes.*
That is their purpose — **a disagreement between a vector and the
specification is a finding against one of them**, and either answer is
progress. Both rounds so far produced specification fixes.

**Pinned**: wire-format.md `792845f55790dc6f7dfbf1c3c189f670ff8345726ae067246b916790ad2af947` · network-design.md `676c4dd9a053edf446ef01b3a13ae60d6e22ec7834943c5f23b3d27964a8128f`

**Scope**: wire-format/protocol **interoperability** vectors.
`light-client-requirements.md` is pinned alongside the two protocol documents
because client rules shape fixture expectations even where they are not
themselves vectorable — the nonce-derivation conformance vector this scope
once named was retired 2026-09-01 with deterministic selection, its section
with it. This is
not a certification suite for client and operator behavioural commitments,
which are deliberately unenforceable from bytes (design §1.1).

The generated documents are produced by `tools/generate.py` (Python 3 with
the `cryptography` package for Ed25519 and **`dilithium-py` for ML-DSA-65**);
re-running it reproduces them byte-for-byte. **`tools/verify.py` is the
independent harness** — its own decoder and Sig_structure reconstruction,
sharing no code with the generator — re-deriving every key, verifying every
signature (ML-DSA under pyca `cryptography`'s independent implementation where
available), and checking **every generated mutation and arithmetic claim it
currently reaches** — not the prose-described fixtures, which await bar 6; run
it after any regeneration. `tools/spec-pins.json` gates generation: a changed specification
needs `--accept-spec-change` — which asserts the audit of **both** the
generator constructions and the hand-authored fixture semantics — a changed
generator **or verification harness** needs `--accept-generator-change`, a
missing pin file `--bootstrap-pins`; it records the producer's, the
harness's, and every output's SHA-256. **Each generated file pins the SHA-256 of both
`network-design.md` and `wire-format.md`** — the design wins on any
disagreement, so a design-only semantic change stales these vectors with the
wire pin still green; a stale pin of either means regenerate before trusting a
vector. This file and `negative-vectors.md` are authored by hand.

| File | Contents |
|---|---|
| `keys.md` | The synthetic test identities — **both components real**: Ed25519 and ML-DSA-65 keypairs from stated seeds, `KeyMaterial` encodings, keyhashes |
| `primitives.md` | Deterministic CBOR atoms, seqno, path, Locator, two complete `SignedLocator` signatures — the second a must-accept same-series counter jump — and genesis back-pointers |
| `transactions.md` | **Positive body vectors** (body + txid) for the six archive transaction types including peering, a formation-subtype presence record — now a **fully integrated object**: real §4.5.1 disclosure root, its type-5 envelope, and three verified presentations — and adversarial variants: signer-order/kid-order divergence, a two-head merge, must-accept disavowal-code and smaller-series-reissue cases, and an unknown-extension adoption with its envelope. **Envelope vectors exist for two shapes**: the adoption (two signers, four entries) and the departure (one signer, two entries); the other types have bodies only |
| `records.md` | One known-answer signature per **signing** context — complete 2026-09-02, each with its wrong-signer analogue |
| `messages.md` | The unsigned message families (bar 9): every framed message's positive encoding, replies and transient payloads, and the session-trace table |
| `corpus.json` | The machine-readable corpus (bar 6): every fixture under a stable id with class, exact bytes and a structured expect — no harness parses Markdown headings as an interface |
| `runner-rs/` | An independent Rust corpus runner: own strict-CBOR parser, identities re-derived from the seed recipe, every envelope/record/presentation signature verified through `ed25519-dalek` and RustCrypto `ml-dsa` — the fixtures' ML-DSA signatures were made by dilithium-py, so agreement is cross-implementation. 143 corpus entries pass; see its README |
| `verifier-selection.md` | The reasonableness criterion — `required()` table rows generated from the formula — and the window boundaries. *The nonce, seed and rank vectors retired 2026-09-01 with deterministic selection* |
| `negative-vectors.md` | Conformance fixtures against a **structured result model** (structural / signatures / chain / per-subject selection / effectiveness / evidentiary), in byte-level, context-dependent, method, and must-accept sections |

## What every vector assumes

- **Identities are synthetic, deterministic, and real for both components**
  (2026-09-01). The keygen recipe is implementation-independent:
  `xi = SHA-256("rhtn-test-vectors:<name>:ml-dsa-65-seed")`, keypair =
  **FIPS 204 `ML-DSA-65.KeyGen_internal(xi)`**; signing is the deterministic
  variant with empty context (`wire-format.md` §2.2). At generation time a
  **second, independent ML-DSA implementation** re-derived every public key
  from the stated seeds and verified every signature — the recipe is not an
  oracle over this generator's output.
- **Every signature in the suite is real.** All envelopes are final; the
  verification harness checks all of them, both algorithms, including the
  mutation-must-fail property across all four entries of the
  unknown-extension envelope.
- Deterministic CBOR per RFC 8949 §4.2 as profiled by `wire-format.md` §1.

## Interpretations taken

**None remain open.** The last — whether a path may be empty — was ruled
2026-09-01: **roots legitimately self-anchor**, the empty path
`{1: h'', 2: 0}` is its one encoding, §2.1 now says so, and D13 is the
generated must-accept: a root's complete self-anchored `SignedLocator`.

Every earlier byte-level choice is likewise closed: The last — whether "transaction types" means the six
archive transactions — was ruled 2026-09-01: **type 6 is retired** with a
tombstone row, the abuse report is §6.3's standalone signed object, and the
suite's reading was confirmed. Its signature-context vector stays queued in
`records.md` under the signed contexts.

Every byte-level choice is likewise closed: Every choice this suite had to make where the
specification under-determined the bytes has been ruled on and written into the
specification [author, 2026-08-31 through 2026-09-01]:

- **"Canonical CBOR of fields X–Y" means the map** of exactly those fields —
  one global sentence in §1 governing all eight signed objects, chosen partly
  because a map is debuggable where a concatenation is not.
- **The witness-nonce PRF was normatively HMAC-SHA-256** while it lived — the
  construction and its section retired 2026-09-01 with deterministic
  selection.
- **The genesis value hashes the raw 32 keyhash bytes**, not a CBOR encoding
  (§3.1).
- **§5's hash inputs are raw concatenations**, stated once with the injectivity
  argument that makes the convention safe there and nowhere else.
- **A merge back-pointer list is sorted ascending bytewise** (§3.1) — one
  logical merge, one encoding, one txid. E11 is the negative complement.

A future vector that needs a choice the specification does not force reopens
this section; until then, every byte in the suite follows from the text.

## Determined by the profile — stated for the record

Items reviews confirmed the specification does fix: nested COSE untagged with
detached payloads and an empty outer protected header (§1, §3.5); `kid` as the
raw 32-byte keyhash in the protected header (§3.5); the body map carried
directly in envelope field 3 (§3); `Participant = {1: keyhash}` (§4.5).

## Findings against the specification

Found by drafting (2026-08-31): **§3.1's signer-order table had no row for
series reissue.** Fixed same day.

Found by review round 1 (2026-08-31), all verified and fixed: **§5.4's
witness-only sentence carried pre-migration field numbers**; **§4.5.2's
seed-inputs row named a nonexistent key 11** (now 3, 4, 7); **§1.1's context
table was missing `rhtn/1:endpoints`** and now also distinguishes the hash/PRF
tag family.

Found by review round 2 (2026-08-31): **E8's second instantiation was wrong
too** — `ClientIntegrity.scheme` is an open namespace (*"a validator checks
only the shapes"*), so an unknown scheme is structurally acceptable; E8 now
uses the presence `subtype`, a genuinely closed enumeration. The row's history
is itself a finding: the spec's closed-enum default has enough exceptions that
each instantiation must be checked against its field.

## Open for the author

**Nothing.** The last item — whether `query_id` needed a domain tag — closed
2026-09-01 with §1.1's **hash-disjointness invariant**: the four untagged
hashes (`txid`, `keyhash`, `query_id`, genesis) have pairwise structurally
disjoint preimage languages, and any future hashed object must stay disjoint
or carry a tag. The suite's positive hash vectors are the invariant's living
witnesses — a txid preimage begins `a? 00`, a keyhash preimage begins `82`, a
genesis preimage is exactly 32 bytes — and the `query_id` vector joined them
2026-09-02 with the recovery adoption's query/consent set (canonical bar 4).

Both fourth-review contradictions were ruled 2026-09-01: **absence is the
encoding of an unanswered query** — `pending` left the enum, the threshold
sizes the sample without gating finalization, and late replies are the
participants' private information (V7 is the must-accept complement) — and
**the 730-day window is exclusive at both ends**, previously completed
ceremonies only. Everything else accumulated across four review rounds is
ruled and applied — see `review-tracking.md`.

## The over-strictness stress family

**Accumulated from the 0.6 clean-room rounds** [author, 2026-09-02]: every
case where an implementer adopted logic stricter than the specification —
divergences the prose demonstrably did not prevent, so the suite must catch
them. A conforming implementation passes each; an over-strict one fails
loudly.

| Origin | Over-strict reading | Stress fixture |
|---|---|---|
| resolution U4 | never dial a referral hop without key material | TR11 — dial it, disclosing nothing beyond the query |
| resolution (documented trap) | require a consumed-equals-length arrival equation | TR14 — a deeper-caching node's early `ServingInfra` is complete |
| attach (heartbeat sketch) | accept only the exact expected counter | TR12 — a gapped beat resets liveness; TR7 states the rule |
| attach (noted trap) | infer degraded mode from having dialled a sibling | TR13 — the server's mode determination is authoritative |
| presence #11 | field 5 required-with-empty-array; reject the absent spelling | P-fin-absent (valid absent) + N-responses-empty-array / T30 (the empty spelling rejects) |
| presence #3 | one Proximity entry per channel kind | P-channel-retry / D19 — optical failed, retried, passed |
| presence phase 1 (first run) | reject counters that skip (`+1` contiguity) | D6 — `[5,42] → [5,100]` is a valid supersession |
| general (frame decode) | closed deserializing enum over control-frame types | TR1 — unknown frames are skipped, the session survives |
| general (enum posture) | reject open-registry values | D1 (disavowal band), D3 (location method), D4 (witness bits), greased capabilities |
| general (finalization) | enforce a response minimum | V7's fixtures — a lone no-match and no responses at all both finalize |
| catalog U3 (0.6.6) | require a locally installed backend for every registration | `P-frame-16` + `P-catalog` — a brokered entry whose endpoint belongs to the external service registers validly |
| authorization U9 (0.6.7) | terminate the caller's whole transport session on a role-row change | TR16 — retire the resource-facing identifier; the transport and other resources' hosted sessions survive |
| memo cycle check (documented trap, 0.6.10) | test cycle by path containment | TR21 — your path is a prefix on every legitimate hop; the test is field-1 identity |

The family grows with every implementation round: when a divergence recurs
despite the spec deciding it, the deciding fixture lands here.

## The canonical bar

What must exist before promotion, merging both reviews' requirements. Applied
already this round: the structured result model, the two-document pin, the E8
correction, positive peering and `EndpointRecord`, five COSE-profile
negatives (S10–S14) plus the context-tag method rule, the seqno jump and
smaller-series reissue must-accepts, the fully-signed unknown-extension
fixture with its mutation complement, and four Recovery cross-binding
negatives (T9–T12). Still open:

1. ~~Real deterministic ML-DSA-65 test keypairs~~ **DONE 2026-09-01**: the
   recipe is stated above, the wholesale regeneration is complete, no
   placeholder slots remain, and a second implementation confirmed keygen and
   signatures. What canonical status still awaits is unchanged in kind: an
   independent implementation reproducing the *whole suite*.
2. ~~A normal-subtype presence record~~ **DONE 2026-09-02**: alice–bob,
   sixteen witnesses (the ceiling), participant/witness/kid orders all
   different, three classical responses covering the `selection_basis`
   matrix, a real disclosure set with a witness corroboration, the 36-entry
   envelope — and the optionals adoption's field 8 now references it (V9
   pass; the mismatch case moved to the divergence adoption, V9a). The
   harness verifies every signature and every binding.
3. ~~The curated-bundle fixture~~ **DONE 2026-09-02**
   (`verifier-selection.md`): a six-entry bundle over real fixtures — the
   repeated counterparty (formation + ac1 + ac2, one candidate), the
   duplicate txid (once), the current counterparty (qualifies, never a
   candidate), the mutated non-verifying record (absent, not "incomplete") —
   plus the witness-only and understatement cases and the reasonableness
   reading, all with harness-recomputed arithmetic. The out-of-window
   exclusive boundary is the window table above it. *The diamond,
   committed-predecessor, bundle-minus-one and selection-binding cases of
   earlier revisions dissolved with the chaining and determinism they
   tested.*
4. ~~Full query → consent → response vectors~~ **DONE 2026-09-02**: both
   authentication forms — Recovery/hybrid (the recovery adoption) and
   presence/classical (the normal record's three responses, with a worked
   ceremony-form query where the querier is the counterparty) — and the
   `selection_basis` matrix: 0/1/2 positively across the normal record's
   responses, 0-only inside a `Recovery` block, T27 carrying the malformed
   cases. The harness verifies every signature in both forms.
5. ~~The selective-disclosure construction~~ **DONE 2026-09-02**: both
   records' presentation sets recompute, and the negative family is in the
   corpus — six-slot, eight-slot, label-position mismatch, 15-byte salt,
   flipped withheld digest, mutated revealed value
   (`N-disclosure-*`).
6. ~~Machine-instantiable fixtures~~ **DONE 2026-09-02** (`corpus.json`,
   format `rhtn-test-corpus/1`): every fixture carries a stable id and one of
   four classes — **bytes** (exact hex), **unit** (a deterministic recipe
   where the identity set or a 100 KB+ body makes full bytes unreasonable),
   **trace** (session event sequences with required actions), **context**
   (named fixture inputs with an expected evaluation) — plus a structured
   expect: outcome, kind, rejection layer (`cbor` / `schema` / `semantic` /
   `session`) and reason. The reference harness executes the encoding layer
   of every byte entry and the arithmetic of every context; layers above
   that are the implementation under test's to find, which is the corpus's
   purpose.
7. ~~The boundary sweep~~ **DONE 2026-09-02** (`B-*` in the corpus): every
   bound in the declared scope with byte fixtures at the bound and past it —
   back-pointers, witnesses, path nibbles, unknown keys and values, audits,
   NetworkPoint lists and ports, prekey blobs, catalog total size, scope
   lists, capabilities, siblings, channels, asserted locations,
   corroborations, geohash lengths and case, archive bounds, the
   finalization gap at 86,400/86,401 — with the two over-identity-set counts
   and the two 100 KB frame bounds as unit recipes.
8. ~~The remaining signed contexts~~ **DONE 2026-09-02**: every
   domain-separation tag has a known-answer signature — currency, catalog,
   abuse, anchor, subtree-ack and prekey in `records.md`; successor, verifier
   and consent in `transactions.md`; locator, endpoints and envelope already
   present — and the **cross-context substitution family is live** (S24): the
   harness verifies every `records.md` signature fails under a neighbouring
   tag, the check that catches two real paths sharing a hard-coded AAD while
   S12's artificial empty-AAD case still passes.
9. ~~The unsigned message families~~ **DONE 2026-09-02** (`messages.md`):
   seventeen positive frames (control and request, correctly typed and
   length-prefixed), fourteen replies and transient payloads (the capture key
   grant and late-response wrapper included), embedded objects byte-identical
   to their signed fixtures, and the **session-trace table** TR1–TR8 —
   skip_frame, defer_until_handshake, close_stream, session_survives,
   fail_attach, the 64 KiB/256 KiB bound split, heartbeat-interval liveness
   and the invalid-attestation must-accept (sixth review).
10. ~~Signer-to-role binding~~ **DONE 2026-09-02**: every standalone signed
    object carries its wrong-signer analogue, and the transaction half is
    `N-envelope-wrong-signers` — a cryptographically valid envelope whose
    kids are alice and carol over a body naming alice and bob. The defect is
    always the same: a valid signature under a key the object does not
    name.
11. ~~Finalization semantics on the normal record~~ **DONE 2026-09-02**:
    the main record finalizes over match/match/`unavailable`; the
    must-accepts carry a record finalized on a lone `no-match` and one with
    key 5 absent entirely — the threshold sizes the sample and gates nothing
    (V7 instantiated). *(The commitment-mismatch and bundle-binding cases
    retired 2026-09-01 with the machinery they tested; `inconclusive` is
    exercised positively at the response level by T19–T22's legal-combination
    matrix when the corpus format lands.)*
12. ~~The enumeration/extension matrix~~ **DONE 2026-09-02** (`N-enum-*`,
    `D-enum-*`): unknown-value byte fixtures for every closed enumeration —
    result, basis, selection_basis, disavowal 64, currency role, resolve
    code, push kind, memo slot, resource status, attach mode — and
    must-accepts for the open namespaces (location method, witness reserved
    bits, greased capabilities, unknown extensions), the unsigned families'
    closed codes included; the right-code-wins evaluation order remains a
    trace concern (TR-class).
13. ~~Optionals-exercised positives~~ **DONE 2026-09-02**: the sweep found
    six optionals no positive decoded — connect_scope, catalog-reply
    truncation, archive head and stop-timestamp, archive-reply continuation,
    channel resolution and session binding, integrity evidence — now
    `P-*` corpus entries; the recovery and normal records had already closed
    adoption fields 5–8 and the response optionals.
14. ~~The schema-shape matrix~~ **DONE 2026-09-02** (`N-shape-*`):
    missing-required and wrong-major-type fixtures across the core schemas —
    adoption, presence, response, currency, catalog, Attach, frame arity,
    Locator, and the 31/33-byte keyhash widths — seeded by E15–E18 and now
    exact bytes in the corpus.

## Open for the author

**Nothing.** The ninth review's two questions were ruled 2026-09-01: **a field
equal to its stated default MUST be omitted** — §1 carries the scalar analogue
of the optional-empty rule, writing 7431 out is malformed (E22), and §7.6's
distinctness never meets one destination twice — and **unknown extension
values are opaque encoded slices, preserved and never interpreted**:
uninterpretable state kept for a reader that may understand it later, any
deterministic CBOR item admissible (D18). Every question accumulated across
nine review rounds is ruled and applied.

## Recorded assumptions of the harness schema

Different in kind from encoding interpretations — none changes generated
bytes; each is a decision the machine-readable corpus depends on (seventh
review):

1. **Reference evaluation lands in `checks[...]`** — `fail` for a dereference
   that does not establish the claim, `unverifiable(unfetchable)` when the
   object cannot be fetched; `effective` stays topology/grant, `chain` stays
   predecessor history (V9/V9b instantiate it).
2. **Fixture identity is machine-readable**: stable IDs carrying bytes,
   context and expected dimensions; generated Markdown is presentation, never
   a harness interface.
3. **`e_map` implements RFC 8949 §4.2.1's bytewise-encoded-key order** via
   unique-key pair sorting — correct generally, exercised here only over uint
   keys and the COSE structures' fixed labels.
4. **A unit-fixture class exists** for conformance requirements no natural
   wire input can instantiate (C3's tie-break); prose is not their permanent
   home.

## Reviewing this draft

The intended review asks three questions of every vector: does the encoding
follow from the cited section with no unstated choice; where a choice was
unavoidable, is it listed under *Interpretations*; and is there a malformed
input a decoder would plausibly accept that `negative-vectors.md` misses?
Checking arithmetic matters less than checking derivations — the arithmetic is
mechanical, the derivations are where a spec mistake would be encoded.
