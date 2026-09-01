# Verifier selection — the reasonableness criterion

Generated against `wire-format.md` `9b1e4cd3573e2fe9…`, `network-design.md` `1143db49dc63d3bf…` and `light-client-requirements.md` `7cb488428ded2ce9…` (full hashes, producer and output hashes in `tools/spec-pins.json`). The design wins on any disagreement; a change to any pinned document stales these vectors.

**Draft. Spec-derived, unverified by an implementation.** See
[README.md](README.md). *The nonce-commitment, seed and hash-rank vectors that
lived here were retired 2026-09-01 with deterministic selection
(`wire-format.md` §5): selection is by recognition — the selector's own
judgment over the handed bundle — and produces nothing recomputable to vector.
What remains checkable is the arithmetic below.*

## The reasonableness criterion (§5.2)

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

## Window boundaries (§5.3)

With `started_at = 1767268800`, the 730-day window is
`1767268800 − 63072000 = 1704196800 < finalized_at < 1767268800` —
**exclusive at both ends: previously completed ceremonies only** (§5.3):

| Prior record `finalized_at` | Counted? |
|---|---|
| 1704196800 (exactly 730 days) | no — boundary instant is out |
| 1704196801 | yes |
| 1767268799 | yes |
| 1767268800 | no — not before `started_at` |
