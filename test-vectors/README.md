# Test vectors — DRAFT

**Status: spec-derived; reviewed clean-room by a second model family
(2026-08-31); verified by no implementation.** These vectors were written from
`wire-format.md` alone, which is exactly the condition `wire-format.md` §13
warns about: *vectors written from the spec alone encode the spec's own
mistakes.* That is their purpose — **a disagreement between a vector and the
specification is a finding against one of them**, and either answer is
progress. The first external review confirmed the arithmetic (it reran the
generator byte-for-byte and independently spot-checked the adoption txid and
the `SignedLocator` signature) and directed the rest of this file's structure.

The generated documents are produced by `tools/generate.py` (Python 3 + the
`cryptography` package for Ed25519); re-running it reproduces them
byte-for-byte, and **each generated file carries the SHA-256 of
`wire-format.md` it was generated against** — a stale pin means regenerate
before trusting a vector. This file and `negative-vectors.md` are authored by
hand.

| File | Contents |
|---|---|
| `keys.md` | The synthetic test identities: Ed25519 keys (real), ML-DSA-65 public keys (structurally valid, cryptographically meaningless), `KeyMaterial` encodings, keyhashes |
| `primitives.md` | Deterministic CBOR atoms, seqno, path, Locator, a complete end-to-end `SignedLocator` signature, genesis back-pointers |
| `transactions.md` | Bodies and txids for adoption, departure, disavowal, series reissue, a formation-subtype presence record, and four deliberately adversarial variants: an adoption whose signer order and kid order diverge, a departure carrying a merge, a must-accept unassigned disavowal code, a must-accept unknown extension key. Plus the adoption's full four-entry envelope |
| `verifier-selection.md` | §5.2.1 nonce **derivation** (HMAC-SHA-256, stable within a window), commitments, the seed preimage and seed, the `required()` table, hash-rank sampling with a worked selection, window-boundary cases |
| `negative-vectors.md` | Conformance fixtures in five outcome classes — malformed, unverifiable, incomplete, ineffective, **must-accept** — split into byte-level, context-dependent, and method-requirement sections |

## What every vector assumes

- **Identities are synthetic and deterministic** — derivation rules at the top
  of `keys.md`. The ML-DSA-65 public keys are *not* valid lattice keys; nothing
  in an encoding or hashing vector depends on key validity.
- **No ML-DSA-65 implementation was available in the drafting environment**, so
  post-quantum signature values are absent: every PQ slot carries its exact
  `Sig_structure` and envelope bytes including them are marked STRUCTURAL.
  **Consequence, per the external review: canonical promotion is a wholesale
  regeneration** — real keypairs change every keyhash and therefore most bodies,
  txids and signatures. That is by design: every computed value flows from the
  generator, so the regeneration is mechanical once real deterministic ML-DSA
  test keys exist.
- Deterministic CBOR per RFC 8949 §4.2 as profiled by `wire-format.md` §1.

## Interpretations taken

Places where the specification under-determines the bytes. **Each is a review
target**: if the choice is wrong, the vector is wrong; if the specification
permits more than one reading, the specification needs a sentence.

1. **`SignedLocator` payload.** §2.3's "canonical CBOR of fields 1–2" is read
   as the deterministic CBOR of the **two-entry map** `{1: subject, 2: Locator}`.
   A concatenation or array reading is also defensible. The spec should fix one.
2. **Genesis back-pointer input.** §3.1's `SHA-256(the signer's keyhash)` is
   computed over the **raw 32 keyhash bytes**, not a CBOR `bstr` wrapping.
3. **Verifier-selection hash inputs.** All §5 constructions are **raw byte
   concatenations** — ASCII tags, raw hashes, the 8-byte big-endian ordinal —
   with no CBOR framing.
4. **§5.2.1's PRF ordinal encoding.** The nonce derivation includes
   `window_ordinal` but does not say how it is encoded; **8 bytes big-endian**
   is used, matching §5.3's seed layout.
5. **Merge back-pointer list order.** §3.1 states no order for a multi-entry
   list; **ascending bytewise** is used. Worth a rule: without one, the same
   logical merge has multiple valid txids.

## Determined by the profile — stated for the record

The first review demoted four items originally listed as interpretations; the
specification does fix them: nested COSE objects untagged with detached
payloads and an empty outer protected header (§1, §3.5); `kid` as the raw
32-byte keyhash in the protected header (§3.5); the body map carried directly
in envelope field 3 (§3); `Participant = {1: keyhash}` with the disclosable
values living only in the disclosure set (§4.5).

## Findings against the specification

Found by drafting (2026-08-31):

1. **§3.1's signer-order table had no row for series reissue.** Fixed same day.

Found by the first external review (2026-08-31), all verified and fixed:

2. **§5.4's witness-only sentence carried pre-migration field numbers** —
   "names them in field 8, not field 4" for what are now fields 4 and 3.
3. **§4.5.2's seed-inputs row named body fields 4, 8, 11**; the presence body
   has no key 11. Now fields 3, 4, 7.
4. **§1.1's context table was missing `rhtn/1:endpoints`** (§7.6's endpoint
   record), and did not distinguish the hash/PRF tag family from the
   `external_aad` signing contexts. Both corrected.

Open for the author:

5. **§4.1's "a record carrying a seed would be malformed" is untestable at a
   decoder** — unknown keys are preserved by rule and nothing marks a seed as
   one. Reserve a key range, or restate it as a writer commitment
   (`negative-vectors.md`, testability gap).

## The canonical bar

What the first review requires before these vectors can be called canonical,
in its priority order — this replaces the earlier not-yet-covered list:

1. A cryptographically complete adoption envelope using real deterministic
   ML-DSA-65 test keypairs — no placeholder slots. *(Blocked on tooling in the
   drafting environment; first item once an implementation exists.)*
2. A normal-subtype presence record whose participant, witness, seed and `kid`
   orders all deliberately differ. *(The formation record and the divergence
   adoption now cover two of the four orders pairwise.)*
3. Real history behind that record — a merge, a repeated counterparty, a
   witness-only transaction, a formation record, an out-of-window record, the
   current counterparty — so *n* and the candidate set are **derived by DAG
   traversal**, not supplied as arithmetic inputs.
4. Full `VerificationQuery` → `query_id` → consent → `VerifierResponse`
   vectors, both authentication forms (presence/classical, Recovery/hybrid).
5. A real seven-slot selective-disclosure set producing the presence record's
   actual field-8 root, with full, partial and empty presentations.
6. Context-fixture files carrying `bytes + context + external state + expected
   outcome` machine-readably (the outcome classes are already in
   `negative-vectors.md`).
7. Boundary sweep at every explicit bound: back-pointers 1/8/9, witnesses
   16/17, responses 32/33, path 24/25, unknown keys 16/17, values 1024/1025,
   u32/u64 edges, epoch saturation, ordinal transitions.
8. More must-accept vectors alongside the four now present — over-strict
   decoders otherwise look conforming.

## Reviewing this draft

The intended review asks three questions of every vector: does the encoding
follow from the cited section with no unstated choice; where a choice was
unavoidable, is it listed under *Interpretations*; and is there a malformed
input a decoder would plausibly accept that `negative-vectors.md` misses?
Checking arithmetic matters less than checking derivations — the arithmetic is
mechanical, the derivations are where a spec mistake would be encoded.
