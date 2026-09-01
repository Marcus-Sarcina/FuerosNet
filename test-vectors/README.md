# Test vectors — DRAFT

**Status: spec-derived; three clean-room review rounds by a second model
family (2026-08-31 / 2026-09-01, each reran the generator byte-for-byte and
found no arithmetic error); verified by no implementation.** These vectors were written from the
specifications alone, which is exactly the condition `wire-format.md` §13 warns
about: *vectors written from the spec alone encode the spec's own mistakes.*
That is their purpose — **a disagreement between a vector and the
specification is a finding against one of them**, and either answer is
progress. Both rounds so far produced specification fixes.

**Pinned**: wire-format.md `7be675cc87635c26845a785436ec3e6ff071ede74a1497a06ff6e50cd7e7f5a6` · network-design.md `0ed4d17e2c3cab09230169ebcb7be14e56cd9d318bff77d0e63276eed2ff5ccd`

**Scope**: wire-format/protocol **interoperability** vectors. This is not a
certification suite for client and operator behavioural commitments, which are
deliberately unenforceable from bytes (design §1.1) and live in the
requirements documents.

The generated documents are produced by `tools/generate.py` (Python 3 + the
`cryptography` package for Ed25519); re-running it reproduces them
byte-for-byte. **Each generated file pins the SHA-256 of both
`network-design.md` and `wire-format.md`** — the design wins on any
disagreement, so a design-only semantic change stales these vectors with the
wire pin still green; a stale pin of either means regenerate before trusting a
vector. This file and `negative-vectors.md` are authored by hand.

| File | Contents |
|---|---|
| `keys.md` | The synthetic test identities: Ed25519 keys (real), ML-DSA-65 public keys (structurally valid, cryptographically meaningless), `KeyMaterial` encodings, keyhashes |
| `primitives.md` | Deterministic CBOR atoms, seqno, path, Locator, two complete `SignedLocator` signatures — the second a must-accept same-series counter jump — and genesis back-pointers |
| `transactions.md` | **Positive body vectors** (body + txid) for the six archive transaction types including peering, a formation-subtype presence record, and adversarial variants: signer-order/kid-order divergence, a two-head merge, must-accept disavowal-code and smaller-series-reissue cases, and an unknown-extension adoption with its envelope. **Envelope vectors exist for two shapes**: the adoption (two signers, four entries) and the departure (one signer, two entries); the other types have bodies only |
| `records.md` | One known-answer signature per **signing** context — `EndpointRecord` complete; the remaining signed contexts queued, and the **unsigned** §7/§8 message encodings explicitly separated so nobody generates signatures the specification does not define |
| `verifier-selection.md` | §5.2.1 nonce derivation (**HMAC-SHA-256, normative for clients — a conformance vector**), commitments, the seed preimage and seed, the `required()` table, hash-rank sampling, window boundaries |
| `negative-vectors.md` | Conformance fixtures against a **structured result model** (structural / signatures / chain / per-subject selection / effectiveness / evidentiary), in byte-level, context-dependent, method, and must-accept sections |

## What every vector assumes

- **Identities are synthetic and deterministic** — derivation rules in
  `keys.md`. The ML-DSA-65 public keys are *not* valid lattice keys; nothing in
  an encoding or hashing vector depends on key validity.
- **No ML-DSA-65 implementation was available in the drafting environment**;
  PQ slots carry their exact `Sig_structure` and envelopes including them are
  STRUCTURAL. **Canonical promotion is a wholesale regeneration** — real
  keypairs change every keyhash — and requires a **deterministic keygen recipe
  first** (canonical bar 1), so a second implementation derives the same keys
  independently rather than taking generator output as an oracle.
- Deterministic CBOR per RFC 8949 §4.2 as profiled by `wire-format.md` §1.

## Interpretations taken

**None remain open.** The last — whether "transaction types" means the six
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
- **§5.2.1's construction is normatively HMAC-SHA-256**, ordinal 8 bytes
  big-endian — the nonce table is a client-conformance vector.
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

Two specification contradictions, surfaced by the fourth review, and one small
observation:

1. **`pending` verifier responses are not constructible as specified.** Design
   §7.4.3 has queries to an offline light client *queue at its patron and
   return `pending`*; the wire makes `pending` a `VerifierResponse.result`
   whose field 9 is signed **by the verifier** — who is, by construction,
   offline. No patron-authenticated pending object exists. §5.5 counts
   `pending` toward finalization, so the gap is load-bearing. Options: a
   patron-authenticated pending variant (the queue-holder attests the query
   was delivered); or `pending` never appears in a record and the slot's
   absence is the encoding — which changes §5.5's counting rule; or something
   else.
2. **The 730-day window's prose contradicts its formula.** §5.3.1 says *"open
   at the far end and closed at the near end"* then defines
   `lower < finalized_at < started_at` — strict at **both** ends, and the
   vector implements the formula. If strict-at-near is intended (a record
   finalising at the ceremony's own start instant is not *prior*), the word
   "closed" should go; if inclusion is intended, the formula and the vector
   change.
3. *Observation*: `query_id` is an undomained SHA-256 of a CBOR map (§4.5) —
   the only such hash in the profile, now that §1.1 notes the tag families.
   Worth a tag, or a sentence saying why not.

Everything else accumulated across four review rounds is ruled and applied —
see `review-tracking.md`.

## The canonical bar

What must exist before promotion, merging both reviews' requirements. Applied
already this round: the structured result model, the two-document pin, the E8
correction, positive peering and `EndpointRecord`, five COSE-profile
negatives (S10–S14) plus the context-tag method rule, the seqno jump and
smaller-series reissue must-accepts, the fully-signed unknown-extension
fixture with its mutation complement, and four Recovery cross-binding
negatives (T9–T12). Still open:

1. **Real deterministic ML-DSA-65 test keypairs** — recipe first (exact
   seed → keypair procedure, e.g. FIPS 204 seed-based keygen from the labelled
   hash), then wholesale regeneration with no placeholder slots. *(Blocked on
   tooling here.)*
2. **A normal-subtype presence record** whose participant, witness, seed and
   `kid` orders all deliberately differ, with witnesses, embedded responses,
   and its 36-entry envelope.
3. **Real history behind that record** — a merge, a repeated counterparty, a
   witness-only transaction, a formation record, an out-of-window record, the
   current counterparty — so *n* and candidates are **derived by DAG
   traversal**; including the two traps the wire format states: only
   cryptographically verified history counts, and an unavailable predecessor
   makes selection unverifiable unless beyond the 730-day pruning boundary.
4. **Full `VerificationQuery` → `query_id` → consent → `VerifierResponse`
   vectors**, both authentication forms (presence/classical,
   Recovery/hybrid), and a complete Recovery adoption as its own target.
5. **The selective-disclosure construction**: a real seven-slot digest list
   producing the record's field-8 root; full, partial and empty
   presentations; negatives for wrong slot count, duplicate or wrong labels,
   wrong salt width, root mismatch, and mutation of a revealed field.
6. **Machine-readable context fixtures** carrying `bytes + context + external
   state + expected result` with the structured result model.
7. **The boundary sweep** at every bound in the declared scope (list at the
   end of `negative-vectors.md`).
8. **The remaining signed contexts** in `records.md` — one known-answer
   signature per domain-separation context, which is also the sweep that
   catches a missing table row like `rhtn/1:endpoints`.
9. **The unsigned message families** — positive known-answer encodings plus
   each family's characteristic malformed and must-accept cases: control
   frames and `Attach`/`AttachAck`, heartbeat and sibling updates, resolution,
   archive/prekey/catalog requests, topology push and memo, resource
   request/response. The families differ on exactly the points a generic
   implementation gets wrong: unknown control-frame types versus unknown
   request types extend differently, and stream 0's 64 KB bound is not §9.2's
   256 KB.
10. **Signer-to-role binding per type** (S17's generalisation): every
    transaction type gets an envelope or a context fixture in which the wrong
    real identity signs with the right shape.
11. **Finalization semantics on the normal record**: an omitted selected slot
    (the selected set and the threshold are the same size); must-accept
    records finalized on `no-match`, `inconclusive`, `unavailable` — and
    `pending` once its construction is ruled; the commitment-mismatch fixture
    (V5); and the committed-predecessor trap — a post-ceremony backfilled
    head that would change *n*, with the expected selection unchanged.

## Reviewing this draft

The intended review asks three questions of every vector: does the encoding
follow from the cited section with no unstated choice; where a choice was
unavoidable, is it listed under *Interpretations*; and is there a malformed
input a decoder would plausibly accept that `negative-vectors.md` misses?
Checking arithmetic matters less than checking derivations — the arithmetic is
mechanical, the derivations are where a spec mistake would be encoded.
