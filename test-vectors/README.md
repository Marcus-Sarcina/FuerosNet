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

**Pinned**: wire-format.md `2cb9ce091981a99d3aa4221cb0bb39060f65f16d505520989089d379282f70fd` · network-design.md `67f5d245694bcd47ad6755a66494ab629035d9517ddca2a10a8ad1e71b98c96d`

**Scope**: wire-format/protocol **interoperability** vectors. This is not a
certification suite for client and operator behavioural commitments, which are
deliberately unenforceable from bytes (design §1.1) and live in the
requirements documents.

The generated documents are produced by `tools/generate.py` (Python 3 with
the `cryptography` package for Ed25519 and **`dilithium-py` for ML-DSA-65**);
re-running it reproduces them byte-for-byte. **Each generated file pins the SHA-256 of both
`network-design.md` and `wire-format.md`** — the design wins on any
disagreement, so a design-only semantic change stales these vectors with the
wire pin still green; a stale pin of either means regenerate before trusting a
vector. This file and `negative-vectors.md` are authored by hand.

| File | Contents |
|---|---|
| `keys.md` | The synthetic test identities — **both components real**: Ed25519 and ML-DSA-65 keypairs from stated seeds, `KeyMaterial` encodings, keyhashes |
| `primitives.md` | Deterministic CBOR atoms, seqno, path, Locator, two complete `SignedLocator` signatures — the second a must-accept same-series counter jump — and genesis back-pointers |
| `transactions.md` | **Positive body vectors** (body + txid) for the six archive transaction types including peering, a formation-subtype presence record, and adversarial variants: signer-order/kid-order divergence, a two-head merge, must-accept disavowal-code and smaller-series-reissue cases, and an unknown-extension adoption with its envelope. **Envelope vectors exist for two shapes**: the adoption (two signers, four entries) and the departure (one signer, two entries); the other types have bodies only |
| `records.md` | One known-answer signature per **signing** context — `EndpointRecord` complete; the remaining signed contexts queued, and the **unsigned** §7/§8 message encodings explicitly separated so nobody generates signatures the specification does not define |
| `verifier-selection.md` | §5.2.1 nonce derivation (**HMAC-SHA-256, normative for clients — a conformance vector**), commitments, the seed preimage and seed, the `required()` table, hash-rank sampling, window boundaries |
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

**Nothing.** The last item — whether `query_id` needed a domain tag — closed
2026-09-01 with §1.1's **hash-disjointness invariant**: the four untagged
hashes (`txid`, `keyhash`, `query_id`, genesis) have pairwise structurally
disjoint preimage languages, and any future hashed object must stay disjoint
or carry a tag. The suite's positive hash vectors are the invariant's living
witnesses — a txid preimage begins `a? 00`, a keyhash preimage begins `82`, a
genesis preimage is exactly 32 bytes — and a `query_id` vector joins them with
the query/consent set (canonical bar 4).

Both fourth-review contradictions were ruled 2026-09-01: **absence is the
encoding of an unanswered query** — `pending` left the enum, the threshold
sizes the sample without gating finalization, and late replies are the
participants' private information (V7 is the must-accept complement) — and
**the 730-day window is exclusive at both ends**, previously completed
ceremonies only. Everything else accumulated across four review rounds is
ruled and applied — see `review-tracking.md`.

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
2. **A normal-subtype presence record** whose participant, witness, seed and
   `kid` orders all deliberately differ, with witnesses, embedded responses,
   and its 36-entry envelope.
3. **Real history behind that record** — a merge forming a **diamond** (an
   in-window qualifying presence transaction reachable through *both* merge
   heads, asserted to contribute **one** to *n*, which is the fixture that
   fails chain-oriented code with no visited set), a repeated counterparty, a
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
6. **Machine-instantiable fixtures throughout**: every byte-level, context
   and must-accept case resolves to **exact bytes, or an unambiguous
   deterministic mutation of a named positive vector**, plus machine-readable
   expected dimensions in the structured result model. Until then the negative
   suite is a conformance-test *specification*, not yet a corpus — two
   implementations should not each have to construct a malformed input before
   testing it (fifth review).
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
11. **Finalization semantics on the normal record**: must-accept records
    finalized on `no-match`, `inconclusive`, `unavailable`, and on **absent
    selected slots** — the threshold sizes the sample and does not gate
    finalization, `pending` having left the enum entirely (V7); the
    commitment-mismatch fixture (V5); and the committed-predecessor trap — a
    post-ceremony backfilled head that would change *n*, with the expected
    selection unchanged.

## Reviewing this draft

The intended review asks three questions of every vector: does the encoding
follow from the cited section with no unstated choice; where a choice was
unavoidable, is it listed under *Interpretations*; and is there a malformed
input a decoder would plausibly accept that `negative-vectors.md` misses?
Checking arithmetic matters less than checking derivations — the arithmetic is
mechanical, the derivations are where a spec mistake would be encoded.
