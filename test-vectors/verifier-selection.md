# Verifier selection — recomputation

Generated against `wire-format.md` SHA-256 `05165711f6ee749fcdce938447a425052f979fa47e7cadddcfe27b4be6ef21ad` and `network-design.md` SHA-256 `9973a20365dc1832c5b497b872b685d3ebf3ebcb0461299c5528852454dc5084` — the design wins on any disagreement, so a design-only semantic change also stales these vectors. Regenerate after any change to either.

**Draft. Spec-derived, unverified by an implementation.** See
[README.md](README.md). All inputs are raw byte concatenations hashed with
SHA-256 — §5 states the convention: raw concatenation of the named byte
strings, injective because every component after the domain tag is fixed-length.

## Nonce commitment (§5.1)

`commitment = SHA-256("rhtn/1:nonce-commit" || witness_keyhash || nonce)`,
nonce exactly 32 bytes.

## Nonce derivation (§5.2.1)

Nonces are **derived, not fresh**, and the construction is **normative for
conforming clients** [author, 2026-09-01]: `nonce = HMAC-SHA-256(witness_secret,
"rhtn/1:wnonce" || min(a,b) || max(a,b) || window_ordinal)`, ordinal as 8 bytes
big-endian (§5.2.1). This table is therefore a **client-conformance vector** —
a conforming implementation reproduces it exactly — while remaining, like every
client-side rule, unenforceable from the wire: nothing downstream of the reveal
can tell which PRF ran, and what wire-side validation covers is the commitment
equation (§5.1) and the seed (§5.3), which later sections exercise against the
revealed values. Same window, same nonce — the anti-grinding stability the rule
exists for. Test secrets are
`SHA-256("rhtn-test-vectors:<name>:witness-secret")`:

| Witness | witness_secret | Derived nonce | Commitment |
|---|---|---|---|
| w1 | `89c2c5f5cc1413e7463b4aaa74a326ea97528403bd75c76ce9cb690fa38afa09` | `0bc651a024674169926210159703f8932de6bf50f5e70d3e8f004fd631ad64de` | `bb7fd4a1c8bab3f434d6734e758a351a0d6be9b7211352d0934c0602f307a77c` |
| w2 | `822f8238b421166b8a097f2771690f5afd8df853e9c022192ff40c2098b18d5f` | `ff3f1bf03bfdd6bf0a79dec957017b5c4fe762f6d78fc43acb72127e65147490` | `63c2c9c848cdc29524f58cb749e640dc5c9c2abe0c560648be3363d313267605` |
| w3 | `cb15d49a84376164def11fb02537d7fb8c3dabdf69301f44193af8fa9309631f` | `f982459b367ea5e2f183b846d39503fbe16a74326eab3212a723306c17f40b7f` | `4b1e8a0dd106dcf0e51cef8f649a839ce0c2d9e9f9ff05b6f36c5cfadd27711a` |

## Seed (§5.3)

Ceremony: participants alice and carol, `started_at = 1767268800`, so
`window_ordinal = floor(1767268800 / 86400) = 20454`, encoded as 8
bytes big-endian: `0000000000004fe6`.

Participants canonicalise bytewise: min is
carol, max is
alice. Witnesses in ascending keyhash
order: w1, w3, w2.

Seed preimage (284 bytes = 20-byte tag + 32 + 32 + 8 + 3 × 64):

```
7268746e2f313a76657269666965722d736565642535e9a92cef24e674fe0b7d
b0cfd67762cf55953336bca8cc7db8ab05049ced435c987e0caa65d2c4edefb7
5db354b8678d3014fdce71596db0abb9b730e9a90000000000004fe669644d3e
698b7c54622f1b8045880d696bcd98d459981f696e8bc76c8d1c5d570bc651a0
24674169926210159703f8932de6bf50f5e70d3e8f004fd631ad64deb821af99
267d56d032bd36070b68336492a16797d3811752a8ee2edd8a40f370f982459b
367ea5e2f183b846d39503fbe16a74326eab3212a723306c17f40b7fbe9d9075
4088a297896519ccc1eed484d2fa0d255d47bc0d51fc9972c47b1b9cff3f1bf0
3bfdd6bf0a79dec957017b5c4fe762f6d78fc43acb72127e65147490
```

seed: `c9d56659e9b5e3f01f85a65c223f634907b129c07c3ffbc2b18130a58ab32d65`

## Threshold (§5.4)

`required(subject) = min(floor(n / 2), 10, |candidates|)`:

| n | candidates | required |
|---|---|---|
| 0 | 0 | 0 |
| 1 | 1 | 0 |
| 2 | 1 | 1 |
| 3 | 2 | 1 |
| 7 | 5 | 3 |
| 20 | 1 | 1 |
| 25 | 12 | 10 |

## Sampling (§5.4)

`rank(c) = SHA-256(seed || subject || c)`, subject alice, candidates c1–c5;
ascending rank, ties by ascending keyhash.

| Candidate | rank |
|---|---|
| c1 | `c3c9a73485e40f08104b6c0f79096447e732fbf8f4d5a8db1d31e37820023244` |
| c2 | `f7a79e015812a21de92b774bf280dff96cc7c477747527fddbdbe75b0a2493c4` |
| c3 | `5d2ee2fbb9254e6f65e2a17c8630301d2499c29dd7031abe4f00ba89c29dadad` |
| c4 | `bd781a2fabbfa4cb12ab6a7a2f949cefff932ab2547bf59b4abe446c23a5c752` |
| c5 | `0fd332f9eab5464c438503501b8dcea9540e3301b9bbade2390bc7508c75ac9b` |

Rank order: c5 < c3 < c4 < c1 < c2.

With n = 7 and these five candidates, `required = 3`;
**selected: c5, c3, c4**. Exactly that many are queried (§5.4).

## Window boundaries (§5.3.1)

With `started_at = 1767268800`, the 730-day window is
`1767268800 − 63072000 = 1704196800 < finalized_at < 1767268800` —
open at the far end, closed at the near end:

| Prior record `finalized_at` | Counted? |
|---|---|
| 1704196800 (exactly 730 days) | no — boundary instant is out |
| 1704196801 | yes |
| 1767268799 | yes |
| 1767268800 | no — not before `started_at` |
