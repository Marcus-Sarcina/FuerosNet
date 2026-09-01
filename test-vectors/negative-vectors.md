# Negative vectors — inputs a conforming decoder MUST reject

**Draft. Spec-derived, unverified by an implementation.** See
[README.md](README.md). Each row names the violated rule; where bytes make the
case sharper they are given. "The valid form" refers to the corresponding
positive vector in this folder.

## Encoding layer (`wire-format.md` §1)

| # | Input | Violation |
|---|---|---|
| E1 | `18 0a` for the integer 10 | Non-shortest form. Valid: `0a` |
| E2 | `9f 05 18 2a ff` for `[5, 42]` | Indefinite-length array. Valid: `82 05 18 2a` |
| E3 | `a2 01 00 01 00` | Duplicate map key — reject **before materialising the map**, or the duplicate collapses and the check passes vacuously |
| E4 | `a2 02 00 01 00` | Protocol map keys out of ascending order |
| E5 | A peering body whose optional field 7 is present as `80` (empty array) | An OPTIONAL field whose value would be empty MUST be omitted, never encoded empty |
| E6 | A body carrying 17 unknown extension keys | Exceeds the 16-per-map bound |
| E7 | An unknown extension key whose encoded value is 1,025 bytes | Exceeds the 1,024-byte encoded-slice bound |
| E8 | A known enumerated field carrying an unassigned value (disavowal reason 64) | Unknown *values* in enumerated fields are rejected; unknown *keys* are preserved — the two rules must not be conflated |
| E9 | Decode-then-re-encode equality used as the canonicality check | Forbidden method: it erases the evidence (duplicate keys collapse, unknown-key encodings are lost). Validate the received bytes |

## Primitives (§2)

| # | Input | Violation |
|---|---|---|
| P1 | seqno encoded as `18 2a` (a single integer) | seqno is `[series, counter]`, never one integer |
| P2 | seqno `[5, 4294967296]` | counter exceeds u32 range |
| P3 | Path nibble value 11 (`b` in the packed bytes) | Nibble values MUST be 0–9 |
| P4 | Path `{1: h'314155', 2: 5}` | Odd nibble count with nonzero trailing low nibble (`55` — the final low nibble MUST be 0) |
| P5 | Path `{1: h'31415000', 2: 5}` | Packed bytes ≠ `ceil(5/2)` = 3 — trailing surplus bytes are malformed, not ignorable |
| P6 | A bare `Locator` presented standalone | Standalone carriage requires `SignedLocator`; a bare locator MUST be rejected |
| P7 | A `COSE_Key` in `KeyMaterial` carrying a fourth label (`kid`) | Exactly three labels per component — any addition changes the keyhash and therefore the identity |
| P8 | `KeyMaterial` ordered `[post-quantum, classical]` | Fixed order, classical first; reordering is a different identity, not an equivalent encoding |

## Envelope and signer set (§3.5, §3.6)

| # | Input | Violation |
|---|---|---|
| S1 | A `COSE_Signature` with a nonempty unprotected header | Malformed, not ignored — "ignored" applies to unknown map keys, never to COSE headers |
| S2 | `kid` placed in the unprotected header | Envelope entries carry `kid` in the **protected** header, covered by the signature |
| S3 | A signer group with one entry | Every logical signer contributes exactly two entries, one classical, one post-quantum |
| S4 | A signer group with two entries both `alg = -8` | Two entries of the same `alg` |
| S5 | Entries ordered PQ-before-classical within a group, or groups out of `kid` order | Canonical order: sort by `kid`, then classical before post-quantum |
| S6 | An adoption envelope with three logical signers | Exactly the signers the type requires — no extras, no duplicates |
| S7 | Envelope `{1: 2, …}` | Unknown schema version MUST be rejected, not best-efforted |
| S8 | A non-canonical envelope whose body verifies | Canonicality applies to the whole envelope; reject even when the body is valid |
| S9 | An embedded `COSE_Sign1` (verifier response, recovery proof) carrying a `kid` | Embedded objects omit `kid` — the surrounding structure names the signer, and a second copy could disagree with the first |

## Presence-record structural rules (§3.2, §3.3)

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
| R9 | A key appearing in a second formation record | At most one formation record per key: its first |
| R10 | `started_at` earlier than the `finalized_at` of the record its back-pointer names | Effective-time monotonicity against the committed back-pointer — the rule that closes backdating |
| R11 | A revealed `proximity` whose `strongest` names a channel without `result = pass`, or with a higher-ranked passing channel present | The one structural rule checked only on reveal; withheld, the record reports unverifiable, never valid and never malformed |

## Transaction-specific (§4)

| # | Input | Violation |
|---|---|---|
| T1 | A `NetworkPoint` with a 16-byte address | v1 demands IPv4; 16 bytes is malformed, not "IPv6 accepted early" |
| T2 | A `NetworkPoint` with port 0 | Zero is never a destination |
| T3 | An adoption whose carried `KeyMaterial` hashes to something other than field 1's keyhash | The hash MUST equal the keyhash of the party described, or a recipient pins the wrong key |
| T4 | A `Recovery` whose response's field 8 ≠ the claimed prior key | Evidence about one old identity embedded under a claim about another |
| T5 | A `Recovery` response whose `subject` ≠ the newly adopted node | The verifier is attesting continuity *to* the new key |
| T6 | Two responses from one verifier in one record | One verifier occupies one slot |
| T7 | A record carrying a keystream seed field | Seeds are local and never on the wire; a record carrying one is malformed |

## Not covered here

Session messages, catalog and abuse objects, topology frames, and resource
requests have their own reject rules (unknown map keys on unsigned messages,
`CONNECT`/`Upgrade` on the resource path, and others) — out of scope for this
draft along with their positive vectors.
