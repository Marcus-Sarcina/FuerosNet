# `rhtn/` — the RHTN implementation workspace

A Cargo workspace beside the specification, laid out by
`Robot/implementation-plan.md`.  The root documents are authoritative on every
protocol fact; code cites them and never the other way round.

| Member | What it is |
|---|---|
| `acceptance/` | The acceptance-test catalogue, its checker, and the stubs generated from it. Present from the start, so the tests owed are counted before any crate exists |
| `codec/` | `rhtn-codec`: deterministic CBOR validated on the received bytes, the §1.3 bounds, envelopes and signer sets, frames and the unsigned message families. No signatures, no state |
| `crypto/` | `rhtn-crypto`: hybrid identities, signing, and verification of envelopes, records, responses and presentations over what the codec parses. The corpus test lives here because it needs both, and PQXDH with X25519 and ML-KEM-768 for the payload sessions |
| `archive/` | `rhtn-archive`: the chain and its back-pointers, archive fetch and backward verification, the local topology table a node keeps of the bindings it has verified, the signed locator and the seal that closes a line, and the series chain a subject presents and a holder ranks, and the prekey objects the network carries without reading |
| `transport/` | `rhtn-transport`: QUIC and TLS 1.3 sessions with raw-public-key mutual authentication, control frames, 0-RTT deferral, attach, heartbeats and failover |
| `node/` | `rhtn-node`: what a node decides above the session — the topology store and the forwarding rule, the rootward memo, resolution and the anchor table, currency and its ladder, peering and replication, the mailbox on disk — and `LiveNode`, which binds them to real sessions, and the prekey service: bundles held opaque, one-time keys consumed once and rate-limited, the subject told when its pool runs dry |
| `sim/` | `rhtn-sim`: an in-process multi-node harness with a datagram-level path that drops, delays, blackholes and replays, and the mesh the TLA+ models are restated over |
| `client/` | `rhtn-client`: the participant client above the session — the two constructions a ceremony fixes to the byte, the sealed capture store a compliant holder cannot open unaided, verifier selection by recognition, consent and key grants, the verifier's and the subject's sides of a query, record assembly and the checks a signer makes, the ceremony over device I/O behind an interface, and recovery: the meeting where a prior counterparty is its own querier, the adoption on both halves, and the subject's own lines sealed, reissued and proved by their chain; and payload confidentiality: prekeys published and stocked, the org swept and a one-time key asked for only when opening, PQXDH sessions on the Double Ratchet, the direct path or the relay, and the channel's dispatch |
| `policy/` | `rhtn-policy`: the reference flow metric over the graph an evaluator builds, the policy interface a node consults, and the conformance test that reports what a substitute policy gives up. `cargo run -p rhtn-policy --example report` prints the report for the reference and a decay policy |

Crates arrived in the plan's milestone order, `rhtn-codec` and `rhtn-crypto`
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

