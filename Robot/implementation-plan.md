# Implementation Plan

**For:** Reconfigurable-Hierarchic Trust Network
**Produces:** a Rust workspace beside the specification, built in the order design §24 gives
**Created:** 2026-09-09
**Status:** proposal. Section 7 lists the decisions that are the author's; nothing below settles one.

---

## How to read this

The specification is finished enough to build against: design §22 says that nothing
open blocks writing code, and classifies what each open item does block. This plan
says where the code goes, what each part answers to, how it is tested, and in what
order it is built. Section 8 is the coverage matrix: what the formal models and the
test vectors already establish, and what acceptance tests still have to cover.

Every section reference here names its document, since `Robot/refcheck.py` resolves
an unqualified section number against the citing file's own headings.

---

## 1. What the documents already decide

- **Language and stack.** design §5 fixes the primitives and design §5.2 names the
  Rust crates that supply them: rustls and quinn for QUIC with `X25519MLKEM768` and
  RFC 7250 raw public keys, RustCrypto `ml-dsa` and `ml-kem` for the post-quantum
  half, aws-lc-rs as the audited native alternative. The conformance runner at
  `test-vectors/runner-rs` is Rust, and a Rust 1.98 toolchain is installed on the
  development machine. Rust is therefore taken as decided rather than proposed.
- **Native and browser are separate targets** (design §5.2). The browser has no
  production path for the transport profile. Nothing below targets it.
- **Build order.** design §24, steps 1 to 10. Section 5 follows it and adds exit
  criteria.
- **Normative force.** Only what a recipient can check from the wire is a MUST
  (design Appendix A.4). The three requirements documents state commitments a
  conforming reference client keeps. The code's tests are the only place those
  commitments become checkable, which is why section 8 exists.
- **What is not built now.** Everything in design §23, and the ceremony hardware
  and browser conformance, are later tracks.

---

## 2. Repository layout

**One repository.** The test-vector pin gate (`test-vectors/tools/spec-pins.json`)
works by hashing the specification files; the code's conformance is pinned the same
way, and a specification change and the code change it forces land in one commit.
Reviewers copy the working tree, so the tree stays coherent at every commit.

**One new top-level directory, `rhtn/`, a Cargo workspace.** The root keeps its
shape: the six documents, `CLAUDE.md`, `test-vectors/`, `models/`, `rhtn/`. No root
document cites `rhtn/`; the code cites the documents.

### 2.1 Crates, and what each answers to

| Crate | Implements | Answers to | Test oracle |
|---|---|---|---|
| `rhtn-codec` | Deterministic CBOR, domain separation, structural bounds, content addressing; primitives; the common envelope and structural verification; the six transaction types; verifier-selection fields; catalog entry encoding; attestations and records; session messages and frames; topology push and memo encodings; resource request encoding | `wire-format.md` §1, §2, §3, §4, §5.5, §6.1, §7, §8, §10.1, §10.2, §11 | `test-vectors/corpus.json`, `negative-vectors.md` classes A to D, fuzzing, property tests |
| `rhtn-crypto` | Keyhash over both components; hybrid sign and verify per signing context; SHA-256; provider trait so the unaudited crates can be replaced | design §5, §5.1, §5.2; `wire-format.md` §1.1, §1.4, §2.2 | `records.md` known-answer signatures with wrong-signer analogues; `keys.md` seed recipe |
| `rhtn-archive` | The chain and back-pointers from the first transaction; genesis and ordinals; timestamps and monotonicity; series reissue and chain ordering; fork detection at the inquirer; archive fetch; evaluation of a presented archive; the local topology table | design §6, §9.0.2, §10, §13; `wire-format.md` §3.1, §3.3, §4.6, §7.9; `infra-client-requirements.md` §5; `light-client-requirements.md` §2 | Property tests over chain rules; `tla/SupersessionDiscipline` invariants as tests; corpus presentations |
| `rhtn-transport` | QUIC and TLS 1.3 with the one named group; raw public key mutual authentication; ALPN; control-frame framing and capabilities; 0-RTT deferral; attach bound to the authenticated identity; close codes for refusal | design §14.1.1, §14.1.3; `wire-format.md` §8.0, §8.1, §9; `infra-client-requirements.md` §7; `light-client-requirements.md` §4 | The `compliant/attach` and `wire-only/attach` obligations as executable tests; two-node interop |
| `rhtn-node` | Serving; queue; currency issuance and escalation; resolution and anchor table; endpoint records; subtree acknowledgement; prekey service; sibling replication; topology propagation and the rootward memo; role table and hosted-session lifecycle; catalog | `infra-client-requirements.md` §1 to §4, §6, §10, §11; design §3.4, §12, §14.1.2, §14.1.6, §15; `wire-format.md` §6, §7.1, §7.2, §7.5, §7.6, §7.7, §7.8, §10 | Sim scenarios from `tla/`; `wire-only/currency` and `tla/IssuerAuthorisation` as tests; queue and heartbeat tests (section 8) |
| `rhtn-client` | Session and failover; archive handling; horizon; verifier selection, consent and key grants; ceremony state machine with device I/O behind an interface; recovery assembly; payload encryption integration; resource requests; cycle handling | `light-client-requirements.md` §1 to §8; design §7, §8, §9, §14.2, §15; `wire-format.md` §5, §7.3, §7.4, §8.2, §11 | `compliant/ceremony` and `compliant/recovery` obligations as tests; verifier-selection vectors; sealed-store tests (section 8) |
| `rhtn-policy` | The reference flow metric; the conformance test; the policy interface | design §16, §17; `models/simulation/flow_metric.py` | The four regression cases carried across; fixed-graph expected scores |
| `rhtn-resources` (later) | Catalog registration, query and lifecycle; request evaluation order and refusal; sandbox with no network bindings; packaging; gateways | design §11; `wire-format.md` §6, §11; `resource-requirements.md`; `infra-client-requirements.md` §9, §10 | Evaluation-order tests; sandbox capability tests |
| `rhtn-sim` | In-process multi-node harness over localhost QUIC, with a datagram-level path harness (a UDP proxy or a recording socket) for loss, delay, replay and blackholing; scripted scenarios | design §12.3, §13, §15; the `tla/` models | The TLA+ invariants restated over the running code; the path harness replaces the frame-filter emulations in the session tests |
| `rhtnd`, `rhtn` | The node daemon and the developer CLI | | Smoke tests |

**The runner is absorbed, then retired.** `rhtn-codec` grows from the runner's
parser and takes `corpus.json` as its test suite. The runner stays until the crate
passes every entry, then goes, so there is one strict decoder in the tree. The
Python generator and harness stay as the other side of the differential pair.

### 2.2 Citation discipline in code

Doc comments cite the specification the way the models do: `design §12.6.5`,
`wire-format.md §4.1 field 8`. `Robot/modelrefcheck.py` is extended to scan `rhtn/`, with no
exemptions, so a renumbered section fails the check rather than leaving a stale
citation in a comment.

**Decisions made during implementation go into the specification and
`change-log.md`, not into code comments.** design §22.2 lists them. A code comment
that settles one of them is the drafting-history failure in a new medium.

---

## 3. Tooling

Kept to what earns its place:

- **`cargo test`** with `proptest` for round-trips, field reordering, locator
  truncation at nibble boundaries, and chain rules.
- **`cargo-fuzz`** on the CBOR decoder, continuously. `Robot/review-plan.md` Stage 2
  names it the highest-value target: malformed input from strangers is the largest
  untrusted surface.
- **`cargo-deny`** for licences and advisories. The natural PQXDH and Triple Ratchet
  source, libsignal, is AGPL, which is a decision (section 7) before design §24
  step 9.
- **`wasmtime`** for the component-model sandbox when `rhtn-resources` arrives
  (`infra-client-requirements.md` §9.2).
- **`uniffi`** to expose `rhtn-client` to Kotlin and Swift when the mobile track
  starts. The ceremony's channels (`light-client-requirements.md` §1.3) need camera,
  NFC and UWB, which exist only there.
- **One gate, `rhtn/check.sh`**, in the shape of `models/run-all.sh`: format, lint,
  tests, corpus, a bounded fuzz run, deny. Separate from the model gate, which runs
  on a different cadence.
- **A spec pin for the code**, in the shape of the test-vector pin: the gate records
  the specification hashes it last passed against and fails when they change without
  an acknowledged re-run.
- **Every long job is fenced** as the prover now is: a core cap and a heap ceiling
  under `nice`. A fuzzer or a full build on 32 cores reproduces the 2026-09-08
  lock-up otherwise.
- **CI**, if hosted, on the remote's GitLab.

---

## 4. Which model does what

- **Byte-exact work uses the strongest model with the largest context**:
  `rhtn-codec`, `rhtn-crypto`, `rhtn-transport`. The failure mode is a plausible
  misreading of one clause of a 4,000-line wire format, and the model must hold the
  wire format and the governing design chapter at once.
- **Bulk mechanical work uses a cheaper model**: test scaffolding, the simulation
  port, CLI plumbing.
- **Differential testing needs an implementer who shares nothing with the first.**
  `Robot/review-plan.md` Stage 2 wants spec conformance checked by an implementer
  who did not write the spec, and `wire-format.md` §13 records that the generator
  and the runner share an author. A different model family writing a small decoder
  cold from the specification, with no access to `rhtn-codec` or its prompts, is the
  cheapest independent implementer available. It is a throwaway, not a product.
- **The cryptographic audit is human** (`Robot/review-plan.md` Stage 2). No pass by
  any model substitutes.
- **Two standing rules.** The design §22.2 items are the author's decisions, and
  the recorded failure pattern is an assistant filling a gap with something
  plausible; code hardens such a fill faster than prose. And no model runs two
  long jobs side by side on the development machine.

---

## 5. Milestones

Following design §24, with the cheapest verification first. Each has an exit
criterion that is a tool's verdict.

1. **Workspace and codec.** `rhtn-codec` and `rhtn-crypto`. Exit: every
   `corpus.json` entry passes; the fuzzer has run for hours without a crash;
   property tests in the gate.
2. **Transport** (design §24 step 1). Two nodes, one authenticated message, attach
   bound to the transport identity, early data deferred. Exit: the attach
   obligations pass as executable tests. This step also retires the design's one
   dependency on third-party facts (section 6).
3. **Archive and topology** (design §24 step 2). Adoption, departure, disavowal,
   the chain from the first transaction, the local topology table. Exit:
   `wire-format.md` §3.1 chain rules and the supersession invariants pass as tests.
   Done: `rhtn-archive` carries the chain, the walk and the table, and the
   queue with its supersession discipline sits in `rhtn-transport`'s node until
   `rhtn-node` has a second reason to exist; the 46 ARC, TOP and QUE entries at
   this milestone pass.
4. **Gossip, resolution, replication, peering** (design §24 steps 3 and 4).
   `rhtn-sim` with partition-and-merge convergence as its first scripted scenario,
   built on a datagram-level path harness that can drop, delay, replay and
   blackhole packets. The harness closes TRN-16 by capturing a client's 0-RTT
   first flight and replaying it as a second connection, and it replaces the
   frame filters the session tests stand on the path with. Exit: the
   `tla/PartitionMerge` invariants hold over the running code, and TRN-16 passes.
   Done: `rhtn-node` carries propagation, resolution, currency and peering,
   and its `LiveNode` binds them to real sessions through the transport's
   control and request hooks; `rhtn-sim` carries the path harness, the mesh
   the model is restated over, and end-to-end tests of one entry per area
   over loopback QUIC. The exit criterion is met — both safety invariants and
   the convergence property hold over the running code, and TRN-16 passes
   with its 0-RTT premise asserted rather than assumed. One entry is
   deferred: REP-07 needs the reference metric and belongs with milestone 5.
   The topology store persists across a restart and request streams are
   rate-limited per requester; the gate lints every crate.
5. **Reference metric** (design §24 step 5). Exit: the four regression cases and
   the fixed-graph conformance test pass.

**That is the initial implementation**: a headless node and client that adopt,
resolve, replicate and peer. `Robot/review-plan.md` Stage 2 becomes meaningful here,
and this is where to run it.

Then, in order: ceremony record and verification with the channels stubbed (design §24
steps 6 and 7), recovery (step 8), payload encryption (step 9), ICE (step 9b),
resources (step 10). The mobile application is its own track once the client core
is stable.

---

## 6. Risks to retire, in order

1. **The transport profile's third-party facts** (design §5.2): that rustls exposes
   `X25519MLKEM768` and RFC 7250 raw public keys, and that quinn's 0-RTT behaves as
   `wire-format.md` §9.1 assumes. Milestone 2 tested these and they hold. One more
   emerged there: rustls's stateful session store hands out single-use resumption
   tickets, so a replayed 0-RTT first flight fails at the TLS layer before the
   session layer's deferral is reached. That is a property of the stateful store,
   not of TLS; a deployment moving to stateless tickets would rest on the deferral
   alone. Milestone 4's replay test is what turns this from a citation into a test.
2. **Post-quantum crate maturity** (design §5.2). Unaudited. Isolated behind
   `rhtn-crypto`'s provider trait so a swap touches one crate.
3. **The canonical biometric profile** (design §22.2). Cross-client verification
   depends on the whole set: extractor, template, fuzzing, matcher, sealed-store
   parameters. Parked behind an interface in `rhtn-client`; decided against real
   candidate engines, not in the abstract.
4. **Payload library licence.** libsignal is AGPL.
5. **Object sizes** (design §5). Envelopes near 8 KB and records near 35 KB at
   ML-DSA-65 are within budget; the anchor table stores keyhashes for this reason.
   Measured, not assumed, once `rhtn-codec` exists.

---

## 7. Decisions that are the author's

- Whether `rhtn/` lives in this repository (section 2) or in its own.
- The payload library and its licence (section 3).
- The post-quantum provider: RustCrypto now, aws-lc-rs when, or both behind the
  trait.
- The mobile framework for the ceremony track.
- Every item in design §22.2, each as it is reached: parameters, the payload
  integration decisions, the biometric profile, capture-key re-derivation, audit
  calibration, replication distance, the divergence notice, archive recovery, the
  queue cap.

---

## 8. Coverage: what is established, and what acceptance tests still owe

**The catalogue is `rhtn/acceptance/acceptance.json`**, one entry per acceptance
test, each quoting the sentence that justifies its expectation; `rhtn/check.sh`
verifies every citation and quote against the specification and reports coverage of
the rows below.

**Established today** means a model or fixture in the tree asserts it and the gate
re-derives it. An acceptance test for such a function only has to show the
implementation agrees with the fixture or model. **Owed** means nothing in the tree
checks it and the implementation's test suite is the first check.

### 8.1 Established by the models and vectors

| Function | Established by |
|---|---|
| Encoding, primitives, envelopes, signer sets, bodies, txids, standalone signatures, presentations, session-message encodings | `corpus.json`, `negative-vectors.md` classes A to D |
| Verifier-selection formula and window boundaries | `verifier-selection.md` |
| Endpoint authentication, attach binding, queued data to the authenticated peer | `tamarin/wire-only/attach` |
| Presence-record attributability, hybrid halves failing separately | `tamarin/wire-only/ceremony` |
| Recovery evidence gate: response, successor and transfer bindings | `tamarin/wire-only/recovery` |
| Currency attestation binding, unexpired issuance, recorded issuer authorisation | `tamarin/wire-only/currency`, `tla/IssuerAuthorisation` |
| Supersession discipline: no issuance, service or attach after supersession | `tla/SupersessionDiscipline`, `tamarin/compliant/attach` and `currency` bounded companions |
| Escalation ladder liveness; issue fresh, never extend | `tla/CurrencyEscalation` |
| Memo cycle detection and repair | `tla/CycleDetection` |
| Convergence after partition, self-truth, no invention | `tla/PartitionMerge` |
| Cut bound, conservation, the branching condition | `simulation/flow_metric.py` |
| Nomination checks, signing discipline, seal-before-reissue, sibling role gate, 0-RTT deferral, queue by credential | `tamarin/compliant/*` |

### 8.2 Owed: acceptance tests with no model or fixture behind them

| Function | Specified in | Milestone |
|---|---|---|
| Decoder robustness: no crash or resource exhaustion on malformed input; global structural bounds; unknown-field preservation; over-strictness rejected | `wire-format.md` §1.2, §1.3; `negative-vectors.md` class D | 1 |
| Transport profile as executed: only the named group offered; raw-public-key mutual authentication; ALPN; refusal as a close code; capability tolerance and greasing; control-frame framing; connection migration | `wire-format.md` §8.0, §8.1, §8.2, §9.1, §9.2; design §14.1.3 | 2 |
| Session lifecycle: heartbeat; three missed intervals then a degraded session on a sibling; the next fresh attach returns to the actual patron; unreachable marking; resumption | design §14.1.2; `light-client-requirements.md` §4 | 2, 4 |
| Queue as executed: cap, crash copies, metadata minimum, enqueue for an offline client, offline versus no record | design §14.1.6, §7.4.3; `infra-client-requirements.md` §2 | 3, 4 |
| Archive chain as executed: genesis form, ordinals, prefix verification, timestamps and monotonicity, archive fetch, the reference policy's evaluation of a presented archive, fork detection at the inquirer | design §10, §9.0.2; `wire-format.md` §3.1, §3.3, §7.9; `infra-client-requirements.md` §5 | 3 |
| Local topology table: adoption, departure and disavowal effects; bindings as a set per relationship; subtree acknowledgement; formation and lifecycle | design §6.2.1, §11.2.1, §13; `wire-format.md` §7.5 | 3 |
| Resolution as executed: anchor table entries and budget; locator series and counter acceptance; the resolution sequence; unsigned replies verified by the handshake; endpoint records; learning an infra child's endpoints; maintenance; what a query discloses | design §12.2, §12.3; `wire-format.md` §2.3, §7.2, §7.6, §7.7; `infra-client-requirements.md` §4 | 4 |
| Propagation as executed: the push forwarding rule; the horizon boundary; loss and replay; memo generation and rootward routing | design §15; `wire-format.md` §10.1, §10.2 | 4 |
| Replication and peering: the replication set, what replicates and when; peering transactions and ASN visibility | design §3.4, §12.7.5; `wire-format.md` §4.4 | 4 |
| Currency in the running system: staple lifetime and refresh, expiry handling, fail-open for staleness and never for held knowledge, the ladder as executed | design §12.6.5, §12.6.5.1; `infra-client-requirements.md` §3; `wire-format.md` §7.1 | 4 |
| Trust metric conformance: fixed-graph expected scores; the policy interface boundary | design §16.1, §16.4 | 5 |
| Ceremony as executed: channel ranking with no upgrade; guided capture; the sealed store, key discard and release policy; a key grant only against a countersigned query; late responses; consent over the query id; disclosure defaults; metadata stripping; decryption failure reported as inconclusive; the local face store | design §7.2, §7.3, §7.5, §7.5.2, §7.6.3, §8.1.1; `wire-format.md` §5.6, §7.3, §7.4; `light-client-requirements.md` §1 | after 5 |
| Recovery as a transaction: block assembly with hybrid responses, the successor proof, the old-key proof, series reissue as executed, acceptance beyond the evidence gate | design §9; `wire-format.md` §4.1, §4.6 | after 5 |
| Payload: PQXDH and Triple Ratchet integration; prekey distribution and service; binding to the hybrid identity; payload demultiplexing | design §14.2, §14.2.4; `wire-format.md` §7.8; `infra-client-requirements.md` §6 | after 5 |
| Resources: catalog registration, query, lifecycle and abuse reports; scope fields; request evaluation order and refusal; role table, predicates and templates; hosted-session termination on role change; no network bindings in the sandbox; package supply chain; gateways | design §11; `wire-format.md` §6, §11; `infra-client-requirements.md` §9, §10, §11; `resource-requirements.md` | after 5 |
| Product-level commitments: privacy choices, warnings before irreversible actions, client-side cycle handling, operator disclosure, retention and backup | `light-client-requirements.md` §5, §6, §7; `infra-client-requirements.md` §8; design §13.7.1 | manual, per release |

**Two things the matrix does not claim.** The models establish properties of their
own abstractions, so an established row still needs the implementation shown to
agree with the model, which is what the sim scenarios are for. And nothing in either
table validates the social claims; `Robot/review-plan.md` says why.
