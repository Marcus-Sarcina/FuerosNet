# Verifier selection — the reasonableness criterion

Generated against `wire-format.md` `81a5dd6b7ea1ab00…`, `network-design.md` `b034b39d486665e7…` and `light-client-requirements.md` `6bb4286d5a2d9101…` (full hashes, producer and output hashes in `tools/spec-pins.json`). The design wins on any disagreement; a change to any pinned document stales these vectors.

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

## The curated bundle (§5.4) — canonical bar 3

Alice hands bob a bundle at a hypothetical next ceremony,
`started_at = 1790557200` (window floor 1727485200, exclusive both
ends; every record below finalized inside it). **Six entries handed, one a
duplicate and one non-verifying**:

| # | Handed | Qualifies? |
|---|---|---|
| 1 | formation record `262a7e58b63c11f5…` (alice–carol) | yes — formations count (§5.3) |
| 2 | ac1 `c7ceb10e39e61b3f…` (alice–carol) | yes |
| 3 | ac2 `41eda24193685689…` (alice–carol) | yes |
| 4 | ac1 again, byte-identical | counts **once** — duplicates dedupe by txid (§5.3) |
| 5 | normal record `b5b6227510ef730e…` (alice–bob) | yes — but **bob is the current counterparty**, never a candidate for his own verification (§5.3) |
| 6 | the formation envelope with any signed-body byte mutated | **not in the pool** — a record that fails its checks contributes nothing; there is no "incomplete", it is simply absent (§5.4) |

The arithmetic, stated so a harness can recompute it:

```
n = 4          (distinct qualifying records: 1, 2, 3, 5)
candidates = 1 (distinct prior counterparties {carol, bob} minus the current counterparty bob)
required = min(floor(4 / 2), 10, 1) = 1
```

**Witness-only does not qualify** (§5.3): the same normal record handed by
**w1** as subject names w1 only in field 4 — a witnessed ceremony's
participants met each other, not the witness. For w1 that bundle yields
`n = 0, candidates = 0, required = 0`.

**Understatement is free and self-defeating** (§5.4): alice handing only
`[ac1, npr]` yields `n = 2, candidates = 1, required = min(1, 10, 1) = 1` —
a smaller claim, a thinner record, and both withheld records remain
individually valid wherever else she presents them.

**The reasonableness reading** (§5.2): an evaluator comparing a finalized
record's response count against `required` learns whether the ceremony was
diligent — never whether the subject's history is complete. The criterion
gates nothing; the finalization must-accepts in `transactions.md` are the
positive proof.
