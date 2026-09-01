# Test vectors — DRAFT

**Status: spec-derived; two clean-room review rounds by a second model family
(2026-08-31, both reran the generator byte-for-byte and found no arithmetic
error); verified by no implementation.** These vectors were written from the
specifications alone, which is exactly the condition `wire-format.md` §13 warns
about: *vectors written from the spec alone encode the spec's own mistakes.*
That is their purpose — **a disagreement between a vector and the
specification is a finding against one of them**, and either answer is
progress. Both rounds so far produced specification fixes.

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
| `transactions.md` | Bodies and txids for all six transaction types **including peering**, a formation-subtype presence record, and adversarial variants: signer-order/kid-order divergence, a two-head merge, a must-accept unassigned disavowal code, a must-accept reissue to a numerically smaller series, and a fully-signed unknown-extension adoption. Plus the adoption's full four-entry envelope |
| `records.md` | §7 standalone signed objects, one known-answer signature per domain-separation context — `EndpointRecord` complete; the rest queued |
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

Places where the specification under-determines the bytes. **Each is a review
target**: if the choice is wrong, the vector is wrong; if the specification
permits more than one reading, the specification needs a sentence.

1. **Genesis back-pointer input.** §3.1's `SHA-256(the signer's keyhash)` is
   computed over the **raw 32 keyhash bytes**, not a CBOR `bstr` wrapping.
2. **Verifier-selection hash inputs.** All §5 constructions are **raw byte
   concatenations** — ASCII tags, raw hashes, the 8-byte big-endian ordinal —
   with no CBOR framing.
3. **Merge back-pointer list order.** §3.1 states no order; **ascending
   bytewise** is used. Worth a rule: without one, the same logical merge has
   multiple valid txids.

Two former interpretations were resolved into the specification [author,
2026-09-01]: **"canonical CBOR of fields X–Y" now means the map of exactly
those fields** — one global sentence in §1, chosen partly because a map is
debuggable where a concatenation is not — and **§5.2.1's construction is
normatively HMAC-SHA-256 with the ordinal as 8 bytes big-endian**, making the
nonce table a client-conformance vector.

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

Rulings of 2026-08-31/09-01 closed the earlier queue: the seed sentence is a
writer commitment; the fields-X–Y payload is the map, globally; HMAC-SHA-256 is
normative for §5.2.1; self-adoption is malformed (T13); `prior_key` MUST differ
from the new key (T14). Still open:

1. **Interpretations 1–3 above**, each a one-sentence specification fix if the
   reading is confirmed — merge-list order is the one with consequences, since
   without a rule one logical merge has several valid txids.
2. **Do the other two-party types reject the degenerate pair** (departure,
   disavowal, peering, series reissue with field 1 = field 2)? Reissue also
   raises whether a root, having no patron, can reissue at all.

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
8. **The remaining §7 objects** in `records.md` — one known-answer signature
   per domain-separation context, which is also the sweep that catches a
   missing table row like `rhtn/1:endpoints`.

## Reviewing this draft

The intended review asks three questions of every vector: does the encoding
follow from the cited section with no unstated choice; where a choice was
unavoidable, is it listed under *Interpretations*; and is there a malformed
input a decoder would plausibly accept that `negative-vectors.md` misses?
Checking arithmetic matters less than checking derivations — the arithmetic is
mechanical, the derivations are where a spec mistake would be encoded.
