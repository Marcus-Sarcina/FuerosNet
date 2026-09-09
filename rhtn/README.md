# `rhtn/` — the RHTN implementation workspace

A Cargo workspace beside the specification, laid out by
`Robot/implementation-plan.md`.  The root documents are authoritative on every
protocol fact; code cites them and never the other way round.

| Member | What it is |
|---|---|
| `acceptance/` | The acceptance-test catalogue, its checker, and the stubs generated from it. Present from the start, so the tests owed are counted before any crate exists |

Crates arrive in the plan's milestone order: `rhtn-codec` and `rhtn-crypto`
first.  Each one implements catalogue entries and marks them
`// acceptance: XXX-NN`; the stub count falls as the implemented count rises.

**The gate is `./check.sh`.**  It checks the catalogue, checks that the generated
stubs match it, and builds and tests the workspace.  It is separate from
`models/run-all.sh`, which runs on a different cadence, and it fences cargo the
same way that script fences the prover.
