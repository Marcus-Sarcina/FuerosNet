# `rhtn/` — the RHTN implementation workspace

A Cargo workspace beside the specification, laid out by
`Robot/implementation-plan.md`.  The root documents are authoritative on every
protocol fact; code cites them and never the other way round.

| Member | What it is |
|---|---|
| `acceptance/` | The acceptance-test catalogue, its checker, and the stubs generated from it. Present from the start, so the tests owed are counted before any crate exists |
| `codec/` | `rhtn-codec`: deterministic CBOR validated on the received bytes, the §1.3 bounds, envelopes and signer sets, frames and the unsigned message families. No signatures, no state |
| `crypto/` | `rhtn-crypto`: hybrid identities, signing, and verification of envelopes, records, responses and presentations over what the codec parses. The corpus test lives here because it needs both |

Crates arrive in the plan's milestone order: `rhtn-codec` and `rhtn-crypto`
first.  Each one implements catalogue entries and marks them
`// acceptance: XXX-NN`; the stub count falls as the implemented count rises.

**The gate is `./check.sh`.**  It checks the catalogue, checks that the generated
stubs match it, and builds and tests the workspace.  It is separate from
`models/run-all.sh`, which runs on a different cadence, and it fences cargo the
same way that script fences the prover.

**Robustness.** The gate runs DEC-01 and DEC-02: every proper prefix of every
accepted fixture, and 48 seeded mutations per fixture. The hours-long run is
`RHTN_FUZZ_SECONDS=3600 cargo test -p rhtn-crypto --test fuzz -- --nocapture`,
which stacks mutations and prints its seed. Coverage-guided fuzzing runs
beside it: `codec/fuzz/` holds five libFuzzer targets, one per decoder layer,
seeded from the accepted fixtures by `codec/fuzz/seed.py`; the gate runs each
for a bounded time with a memory cap when a nightly toolchain and `cargo-fuzz`
are present, and the long run is `cargo +nightly fuzz run <target>` in
`codec/`. Neither replaces the seeded runs, which are what the entries
specify; the guided search adds coverage on top.

