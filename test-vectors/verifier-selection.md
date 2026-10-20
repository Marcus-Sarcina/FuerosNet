# Verifier selection — the reasonableness criterion

Generated against `wire-format.md` `ce0078a95a10515d…`, `network-design.md` `e7f289d2058761d9…` and `light-client-requirements.md` `1df489e245f3776e…` (full hashes, producer and output hashes in `tools/spec-pins.json`). The design wins on any disagreement; a change to any pinned document stales these vectors.

**Canonical. Spec-derived, reproduced by the independent harness.** See
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

## The curated bundle (§5.4) — canonical bar 3

Alice hands bob a bundle of introductions (§2.3) at a hypothetical next
ceremony: for each prior counterparty she names, that party's key material
with the most recent `SignedLocator` she holds for them. No record of any
meeting travels, and no window applies (§5.3). **Six entries handed, one a
later locator for a party already introduced, one non-verifying, one of
alice herself** (each entry's bytes are in `corpus.json` under
`P-vs-bundle-1` to `P-vs-bundle-6`):

| # | Handed | Qualifies? |
|---|---|---|
| 1 | c1, locator seqno [2, 7] | yes |
| 2 | carol, locator seqno [1, 0] | yes |
| 3 | c1 again, locator seqno [2, 9] | counts **once** — one subject is one claim, and the greater counter in the same series stands (§5.3) |
| 4 | bob, locator seqno [1, 0] | yes toward *n* — but **bob is the current counterparty**, never a candidate for his own verification (§5.3) |
| 5 | alice's key material with c1's signed locator | **not in the pool** — the material does not hash to the locator's subject; an introduction failing its checks contributes nothing (§5.3, §5.4) |
| 6 | alice introducing herself | **not in the pool** — a party is not its own prior counterparty (§5.3) |

The arithmetic, stated so a harness can recompute it:

```
n = 3          (distinct qualifying subjects: c1, carol, bob)
candidates = 2 (distinct prior counterparties {c1, carol, bob} minus the current counterparty bob)
required = min(floor(3 / 2), 10, 2) = 1
```

**Understatement is free and self-defeating** (§5.4): alice handing only
`[carol]` yields `n = 1, candidates = 1, required = min(0, 10, 1) = 0` —
a smaller claim and a thinner record, and the counterparties withheld are
hers to name at any later ceremony.

**Fishing is one at a time** (§5.1, §7.10.2): where the bundles leave the
criterion unmet, a `FishingProposal` carries exactly one introduction — the
corpus's `P-fishing-proposal` is entry 1 above — and the parties alternate.

**The reasonableness reading** (§5.2): an evaluator comparing a finalized
record's response count against `required` learns whether the ceremony was
diligent — never whether the subject's history is complete. The criterion
gates nothing; the finalization must-accepts in `transactions.md` are the
positive proof.
