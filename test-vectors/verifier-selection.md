# Verifier selection — recomputation

Generated against `wire-format.md` `52cd1de9c3e3d7ac…`, `network-design.md` `76786811432739e5…` and `light-client-requirements.md` `309399ef6a9c1a66…` (full hashes, producer and output hashes in `tools/spec-pins.json`). The design wins on any disagreement; a change to any pinned document stales these vectors.

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
| w1 | `89c2c5f5cc1413e7463b4aaa74a326ea97528403bd75c76ce9cb690fa38afa09` | `ed636d115386e563b15b484421720ab02bd6bce6c8acc21eeb7a462dab214c98` | `562311c183df51dc00b119c43ea95f15ab1128b117044c310a24e72d0a69776b` |
| w2 | `822f8238b421166b8a097f2771690f5afd8df853e9c022192ff40c2098b18d5f` | `6758ec6f50fd830dffda3341ca6e38905304157a7bf3ecca73311010a72b6f13` | `3bac64d11886fe0ec4b083841d29236ba75ea1f9ca726ddfa7b121307a1e67d9` |
| w3 | `cb15d49a84376164def11fb02537d7fb8c3dabdf69301f44193af8fa9309631f` | `7690cc8f98388f90cb28f70f93e8fd2f295f8e063702b2de9da52219bfda6220` | `e4b98bf69b9103610b4bb8e6465a1b5d71169a7b6e5c67ff7beb97b90bf99421` |

## Seed (§5.3)

Ceremony: participants alice and carol, `started_at = 1767268800`, so
`window_ordinal = floor(1767268800 / 86400) = 20454`, encoded as 8
bytes big-endian: `0000000000004fe6`.

Participants canonicalise bytewise: min is
carol, max is
alice. Witnesses in ascending keyhash
order: w2, w1, w3.

Seed preimage (284 bytes = 20-byte tag + 32 + 32 + 8 + 3 × 64):

```
7268746e2f313a76657269666965722d7365656471591ef14497c49bd95fc61e
243ccc9e2d041f54d210e89ad6654f61554a2cf28410def778a5de3a25991aba
399716bc8eccfda9ad57d4ea8a0c8dcfc852aa6a0000000000004fe65c5f9a52
ddbd9c7cfd08d87c82183a5b730379cc55df76ab85a6e06b54bf5ab36758ec6f
50fd830dffda3341ca6e38905304157a7bf3ecca73311010a72b6f13efe78768
68352e6f8cabae9ba591a3e5ca61cf64950d19eb40ab82120738587fed636d11
5386e563b15b484421720ab02bd6bce6c8acc21eeb7a462dab214c98f7415dbb
4ce281fcd37b40f25b258537f7fcccd439520b77d86077c76f9e39df7690cc8f
98388f90cb28f70f93e8fd2f295f8e063702b2de9da52219bfda6220
```

seed: `d4c19494395e4086de4e7ee86a4e3cf3d0e50074b573c0ff0027308af723e48f`

## Threshold (§5.4)

`required(subject) = min(floor(n / 2), 10, |candidates|)` (rows generated from
the formula itself):

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
| c1 | `a2f5a2d2f7fc2e93706eff467589a148258a8ab32fc701bbff11adc20283b643` |
| c2 | `9e076f01d45186c364e73701d29df036f64f78d272044d64b9c34065b45d2022` |
| c3 | `085c15696ea8595bbf82aa2d14b296c101acd2d5fc58e76702953033945a3466` |
| c4 | `ae07fd96b21b130fd2ccfc41cbf83e8d8544f45e0917ccfbb1afc69711b00d90` |
| c5 | `85186af09cc76df956bd37057009148fdf1fc584ff9992fc68861b594a74cd16` |

Rank order: c3 < c5 < c2 < c1 < c4.

With n = 7 and these five candidates, `required = 3`;
**selected: c3, c5, c2**. Exactly that many are queried (§5.4).

## Window boundaries (§5.3.1)

With `started_at = 1767268800`, the 730-day window is
`1767268800 − 63072000 = 1704196800 < finalized_at < 1767268800` —
**exclusive at both ends: previously completed ceremonies only** (§5.3.1):

| Prior record `finalized_at` | Counted? |
|---|---|
| 1704196800 (exactly 730 days) | no — boundary instant is out |
| 1704196801 | yes |
| 1767268799 | yes |
| 1767268800 | no — not before `started_at` |
