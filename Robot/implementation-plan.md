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

**One new top-level directory, `crates/`, a Cargo workspace.** The root keeps its
shape: the six documents, `CLAUDE.md`, `test-vectors/`, `models/`, `crates/`. No root
document cites `crates/`; the code cites the documents.

**The applications live in the workspace, and the shells beside it.**
`crates/daemon`, `crates/cli` and `crates/ffi` are Cargo members; `crates/mobile/android`
and `crates/mobile/ios` are Kotlin and Swift and cannot be. Keeping them here is not
a preference. The acceptance catalogue is the only mechanism binding this code to
the specification, and it binds by a filesystem walk: an entry counts as
implemented when a marker appears in a file `acceptance/tools/catalogue.py` finds
under the workspace. Eight of the nine manual product entries are the light client
application's and one is the operator's, so applications elsewhere could never
close one. Two further things break at a repository boundary: `crates/check.sh` is
one verdict over one workspace, and a change crossing the boundary could not be
green in a single run; and the conformance review copies the working tree at a
commit, so two trees would have to be paired by hand.

**The split has a trigger, not a date.** Move the applications to their own
repository when the library carries a published version and the code's spec pin
stops moving. They become consumers of a released crate rather than path
dependencies, the product entries travel with them, and the gate divides along a
seam that already exists. Two things keep that move cheap and are worth preserving
until it happens: the application tier depends on `rhtn-ffi` alone, and nothing in
the library depends on the applications. A repository boundary is not a licence
boundary, so nothing about the payload library's licence (section 7) is settled by
moving code between repositories.

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
| `rhtn-resources` | The component-model sandbox a hosted package runs in: what it may import, what one request may spend, and the `Backend` a gateway hands a request to. Catalog registration, query and lifecycle, the request evaluation order and refusal, the gateway and the host's export list landed in `rhtn-node` and `rhtn-archive` at milestone 10 (section 5) | design §11; `wire-format.md` §6, §11; `resource-requirements.md`; `infra-client-requirements.md` §9, §10 | Evaluation-order tests; sandbox capability tests |
| `rhtn-adaptors` | `rhtn-client` bound to what is local to its process: the client on a thread of its own, the node beside it as serving node, the direct payload path over the transport's socket (the client's own or the node's), a hosted verifier answered on the node's request stream, and the courier. The seams the documents leave unwritten, a serving node's leg to a client attached over the wire and a client's hand-off of payload to relay, are traits with the in-process implementation behind them | design §12.6.3, §14.1.1; `wire-format.md` §5.6, §7.7.2, §9.2 | Live tests over loopback QUIC for both kinds of client |
| `rhtn-sim` | In-process multi-node harness over localhost QUIC, with a datagram-level path harness (a UDP proxy or a recording socket) for loss, delay, replay and blackholing; scripted scenarios | design §12.3, §13, §15; the `tla/` models | The TLA+ invariants restated over the running code; the path harness replaces the frame-filter emulations in the session tests |
| `rhtn-participant` | `rhtnp`: a participant a person or a script can run. One command a line on standard input, the terminal standing in for the six platform objects, and nothing kept between runs. **An instrument and not a product**: it claims none of the product entries, because a command read from standard input is not a person | `light-client-requirements.md` §1.3, §3; design §14.1.0, §14.2.4 | PRT-01 onward; a ceremony and a payload between processes rather than inside one |
| `rhtn-daemon` | `rhtnd`: a node run from an operator's configuration. The configuration a node cannot derive; the lifecycle from start to signal to stop, losing no delivery in flight; and the operator's view of what the configuration exposes to the identities below it | `infra-client-requirements.md` §1, §2, §4.1, §7, §8, §10.6, §10.7; design §13, §14.1.2, §14.1.6 | PRD-06; a node started from a file serves a client and survives a restart |
| `rhtn-cli` | `rhtn`: decode what the wire carries with the parser a node uses, mint and inspect identities, and ask a running node the read-only questions | No obligation document requires a command line. What it may send is bounded by `wire-format.md` §9.2's read-only class: §7.7, §7.9, §6.4 | Every corpus object prints; a probe resolves, fetches and queries over a real session |
| `rhtn-ffi` | The one boundary the mobile shells bind to: the client's operations outward, the platform's camera, channels and clock inward, and the value types that cross. No decision is taken at the boundary that is not taken below it | `light-client-requirements.md` §1.3 | The facade compiles against both shells' generated bindings |
| `mobile/android`, `mobile/ios` | The light client application: the ceremony's channels and capture, the privacy choices, the warnings before anything irreversible, and encrypted backup. Kotlin and Swift, not Cargo members, not built by the gate | `light-client-requirements.md` §1.3, §5, §6; design §13.7.1 | PRD-01 to PRD-05 and PRD-07 to PRD-09 |

**The runner is absorbed, then retired.** `rhtn-codec` grows from the runner's
parser and takes `corpus.json` as its test suite. The runner stays until the crate
passes every entry, then goes, so there is one strict decoder in the tree. The
Python generator and harness stay as the other side of the differential pair.

### 2.2 Citation discipline in code

Doc comments cite the specification the way the models do: `design §12.6.5`,
`wire-format.md §4.1 field 8`. `Robot/modelrefcheck.py` scans `crates/` as well as
`models/`, with no exemptions, so a renumbered section fails the check rather than
leaving a stale citation in a comment. It reads `.rs`, `.py`, `.md` and `.toml`,
and the shells' `.kt` and `.swift`; it skips build output and the generated stubs,
whose citations `acceptance/tools/check.py` already checks verbatim against the
section text. The first run over the workspace checked 637 citations and flagged
none, and caught the first citation written after it.

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
- **`toml`** for the operator's two files, the `rhtnd` configuration and the
  hosting file [author, 2026-09-14]. Not for the package manifest, which is
  the package's own file and what §9.1's signing will sign — a signed object
  wants a canonical encoding and TOML has none.
- **`uniffi`** to expose `rhtn-client` to Kotlin and Swift when the mobile track
  starts. The ceremony's channels (`light-client-requirements.md` §1.3) need camera,
  NFC and UWB, which exist only there.
- **One gate, `crates/check.sh`**, in the shape of `models/run-all.sh`: format, lint,
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
   Done: `rhtn-archive` carries the chain, the walk and the table; the queue's
   store contract, its memory store and the supersession discipline sit in
   `rhtn-transport`'s node, which delivers from them, and the directory store a
   restarting node keeps moved to `rhtn-node` on 2026-09-10; the 46 ARC, TOP
   and QUE entries at this milestone pass.
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
   Done: `rhtn-policy` carries the reference flow metric over the graph an
   evaluator builds, the policy interface, and the conformance test; six
   regression cases are carried from the simulation with the fixed-graph
   fixture, and the simulation's committed E2 and E3 figures are reproduced
   by the test itself; `rhtn-node` consults its policy through one call and
   nowhere on its decision path. REP-07 is closed here. 165 of 256 entries
   pass.

**That is the initial implementation**: a headless node and client that adopt,
resolve, replicate and peer. `Robot/review-plan.md` Stage 2 becomes meaningful here,
and this is where to run it.

**Milestone 6, ceremony record and verification with the channels stubbed**
(design §24 steps 6 and 7). Exit: the 29 ceremony entries pass.
Done: `rhtn-client` carries the two constructions a ceremony fixes to the
byte, checked against the corpus; the sealed capture store, bound to the
ceremony and not to the record, since sealing precedes the record; the
query objects, selection by recognition, the verifier's automatic answer
and the subject's consent, counters and grant; record assembly with the
participant's refusals, the witness's clock check, the presentation that
withholds by default, and late responses beside a record; and the
ceremony itself over a device interface — proximity channels, camera,
clock, randomness, operator, notifier and engine — with an in-process
harness that carries the direct channel and logs every path. The engine
is a stand-in that hashes a frame's leading bytes (design §22.2 is
undecided); the sealed store's AEAD, nonce and framing are the
implementation's choice pending §7.5.2.10. All 29 ceremony entries pass;
224 of 286 entries pass. One reading is open to the author: design §7.3
says a verifier's operator is not told they were sampled, and design
§19.6 says a verifier is told, when asked, that answering records them;
the client raises the §19.6 disclosure as a notice naming no ceremony.

**Milestone 7, recovery** (design §24 step 8). Exit: the 14 recovery
entries pass.
Done: `rhtn-archive` carries the signed locator and the seal that closes
a line, and the series chain a subject presents and a holder reads,
extends only into a series it never held, and ranks by length;
`rhtn-node`'s locator store takes a chain and thereafter rejects the
abandoned lines whatever their counter, while a thief's seal freezes a
chainless holder's entry; the verifier checks an adoption's evidence
against the adoption's own fields as an entry point of its own, and a
patron runs it before countersigning, so a lifted successor or transfer
statement binds nothing; competing recoveries resolve per observer by the
patron it prefers; a plain rotation carries nothing and its memo names no
prior key. `rhtn-client` carries the subject's side: sealing every line as
the old key's last act, seal-then-reissue on suspicion into a fresh
series, the chain presented on request, and the recovery meeting on the
ceremony harness — the verifier its own querier, the new key consenting,
the person recognising, the hybrid response naming the prior key, the
adoption assembled on both halves and checked by the patron. All 14
recovery entries pass; 238 of 286 entries pass. Three readings are open to
the author and recorded in `Robot/review-tracking.md`: recognition by
personal knowledge only, the recovery verifier's person asked, and the
stream on which a chain is asked for.

**Milestone 8, payload encryption** (design §24 step 9). Exit: the 15
payload entries pass. Done, all but one: `rhtn-crypto` carries PQXDH to
the byte, instantiated with X25519, SHA-256 and ML-KEM-768 from
RustCrypto's `ml-kem` and `x25519-dalek`; `rhtn-archive` the prekey
objects; `rhtn-node` the prekey service — bundles held without reading,
reusable material served freely, a one-time key consumed once and only
when requested, rate-limited per requester per subject, the subject told
when its pool runs dry, and nothing persisted about who asked; and
`rhtn-client` the material published and stocked, the org swept as one
batch, a one-time key asked for only when opening a session, the session
opened on PQXDH and run on the Double Ratchet with the direct path or the
relay chosen per message, and the channel's dispatch of key grants, late
responses and application payload. 14 of the 15 payload entries pass; 252
of 286 entries pass. **PAY-13 is not marked**: the Triple Ratchet is the
Double Ratchet beside the Sparse Post-Quantum Ratchet, whose only
implementation is libsignal, AGPL, and whose adoption is the author's
decision (section 7); what stands is the Double Ratchet alone, and no
published vectors exist to match either construction against. Four
readings are open to the author and recorded in `Robot/review-tracking.md`:
the identity binding is classical, a sweep of one subject is a single
reusable request, a batch is answered with an array of replies, and how
bundles, one-time keys and the exhaustion notice reach the serving node.

**Milestone 9, the direct payload path** (design §24 step 9b). Exit: the
six traversal entries pass. Done: `rhtn-transport` carries STUN Binding to
the byte and a socket that serves QUIC and STUN together, so a serving
node is a STUN server at the address it already serves on and a leaf
learns the reflexive address of the very socket its direct connection
will use; candidates and their exchange; and the dial race that is the
connectivity check, the first QUIC handshake under the pinned key kept.
`rhtn-node`'s live node gathers only where design §12.6.3's decision says
the path may be direct, holds or remembers the failed path per peer, and
sends on it or through the relay without waiting. `rhtn-sim` carries an
emulated NAT with RFC 4787's mapping and filtering behaviours, so two
leaves behind endpoint-independent NATs punch through and two behind
address-and-port-dependent ones fall back to the relay. Control traffic
dials outward and never asks STUN. All six traversal entries pass; 258 of
292 entries pass. Two readings are open to the author and recorded in
`Robot/review-tracking.md`: TURN's function is the node's existing payload
relay rather than RFC 8656, and candidates travel on the relayed
end-to-end channel, so the wire carries no signalling object.

**Milestone 10, resources** (design §24 step 10). Exit: the 24 resource
entries pass. Done: `rhtn-archive` carries the resource objects — scopes,
the owner-signed entry, the resource-signed abuse report, the query, the
registration and the request with their replies — checked against the
corpus's fixtures; `rhtn-node` the catalog, registered over the owner's
own session, one owner per keyhash, replaced without an archive, filtered
at answer time by a scope evaluated over the table, ordered by resource
then owner with the continuation, the owner's signature returned
unchanged, and an asker outside the horizon given no reply; the gateway,
evaluating a request in the normative order from one snapshot, parsing
and re-serialising the HTTP message strictly, refusing the ambiguous,
routing by the resource, stripping the caller's `rhtn-*` headers and
presenting the pairwise principal, the roles, the audience and a session
minted per resource, handing off once and never retrying, and ending the
hosted session on a row change; the role table materialised and refusing
a row wider than 64; acknowledgements issued under policy and lapsing
with the relationship; the package host's two exports and nothing else;
and abuse reports stored for the owner and carried nowhere.
`rhtn-transport` proves a request stream is never processed in early
data and carries one request. `rhtn-client` sweeps the catalog and reads
a repeated continuation as truncation, routes to a brokered service only
where it matches the signed entry, shows the page as served, and
surfaces an unrecognised declaration. All 24 resource entries pass; 282
of 292 entries pass. Two readings are open to the author and recorded in
`Robot/review-tracking.md`: the package host is a binding table without
a component runtime, and a request to a brokered resource through the
node is answered unavailable.

That completes design §24's order. The mobile application is its own
track once the client core is stable; what remains in the catalogue is
PAY-13, on the library decision, and the nine manual product entries.

**Review after milestone 10** (2026-09-11). A read of every crate for gaps
and dangling stubs found no `todo!`, `unimplemented!` or ignored test, and
three things the milestones had walked past: the runtime dispatched no
archive request although `Archive::serve` had existed since milestone 3
(closed, with a live test); the direct path's candidates had no payload
kind of their own (closed: kind 3); and the crate table above still
promised `rhtn-resources` the catalog and the gateway (corrected). Left
open, each a decision or a milestone of its own: how a serving node
carries a verifier query to the light client it attaches, which the
documents do not say either; the join between `rhtn-client`'s direct-path
interface and `rhtn-transport`'s socket; and `rhtnd` and `rhtn`
themselves.

**Adaptors** (2026-09-11). The author's answer to the first two: build the
adaptors for what is local to the process, for both kinds of client.
`rhtn-adaptors` (section 2.1) hosts a client on a thread of its own and
binds it to the node beside it and to the transport's socket: a verifier
hosted in the process answers request type 4 on the node's request stream,
whether the node itself as a participant or a light client beside it, and
the direct path is joined from the client's own send, candidates as a
payload kind over the relay and the peer's dialled. Two entries, CER-30
and TRV-07. The seams the documents leave unwritten stay traits: a serving
node's leg to a client attached over the wire, and a client's hand-off of
payload to relay, which the wire has no frame for and no sender
attribution in.

### 5.1 The application tier

**Design §24's order ends at milestone 10.** What follows is the tier above
the library (section 2): two binaries, the boundary, and the shells. These
milestones answer to the obligation documents rather than to a step of the
design's order, and the catalogue's product entries are what they close.
Nothing here may decide protocol behaviour: a rule enforced in an
application and not in the library is absent from every other client, and
§1.1's test disposes of it.

**Milestone 11, the daemon** (`rhtn-daemon`). Exit: `rhtn-sim`'s scenarios
rerun against daemon processes rather than in-process nodes, and a daemon
restarted mid-scenario redelivers what it had accepted and nothing else.

Done (2026-09-12), against the real binary rather than an in-process node:
`rhtnd` starts from a configuration file and a peers file, serves a QUIC
session at the address it reports, and a restart redelivers what was
accepted on the next attach and nothing on the one after. The identity is
read and never minted, and one readable beyond its owner is refused. The
one-time pools and the topology store are read before a session can be
accepted and written back on a signal and on a sixty-second tick. DMN-01
and DMN-02. **Two gaps in the crates beneath had to be closed first**: a
`NodeConfig` had no listen address, so an operator's choice could not
reach `LiveNode`, which bound loopback unconditionally; and an `Identity`
could not be built from the `KeyMaterial` a peer is pinned by, which is
how a node is configured with peers it has never contacted.

**The exit criterion is met** (2026-09-13). `rhtn-sim` gained the harness it
needed: `Daemons` spawns several `rhtnd` processes, writes each an identity,
a peers file and a configuration naming its upstream, and reads back the
address the process reports, so a scenario can start, stop and restart any
of them by name. Two scenarios run against it, and each makes every claim
over a session or from what a process wrote rather than by reading a view:
a transaction crosses two processes and survives a restart of the second
(DMN-17), and a daemon answers a resolution on a stream from topology it
was pushed over a session (DMN-18).

**The finding that came out of it was in the library, not the daemon.** A
node's set of attached clients was populated by no production code — only
by tests reaching into the view — so a running daemon forwarded the flood to
none of its clients whatever its configuration said. The transport now calls
an `on_attach` hook as a session is inserted and removed, and the runtime
installs one that maintains the set. PRP-23 holds it.

**Which node the answer names was settled by the author** [2026-09-14]:
publishing an endpoint record is what marks a node infrastructure, which is
what `wire-format.md` §7.6's *published by infra nodes only* already said.
A stored record now marks its subject, the mark is re-derived on a rebuild
rather than kept beside the store, and there is no unmarking — §7.6 gives a
record a successor and no retraction. DMN-18 asserts the referral again, and
PRP-24 holds the marking on its own.

**Two more callers were missing under it**, both the same shape as the
attach hook. Nothing in a running node called `replay_to`, so §10.1.3's
reconciliation — *a replay of the same frames* — never ran: two parties that
connected after their records were made never exchanged them, and an
endpoint record published before a session existed reached nobody. A session
coming up now replays, in both directions. **The periodic half is still
absent**: §10.1.3 asks for a periodic reconciliation with siblings and the
patron, and its interval is an operator's number that no document states.

What it owes beyond the library. The identity is read and never minted: a
node that generates a key when its file is missing serves under an identity
nobody adopted, and its operator would not know. The queue's directory
store and the prekey service's pools load before the node serves, since
both are consumable state a restart must not reissue
(`infra-client-requirements.md` §2, `wire-format.md` §7.8). Shutdown on
SIGINT and SIGTERM refuses new sessions, lets deliveries in flight finish
and persists before exit; the store contract already leaves a half-made
delivery where it was, and the shutdown must not defeat it.

Two things it does not owe. **Hosting waits on `rhtn-resources`**, so a
daemon at this milestone brokers and does not host. And **PRD-06 closes on
a person reading a screen**, not on a tool: the binding view showing hosted
against brokered, the exposure the configuration creates for the identities
below, and the roles an accessing user holds without the predicates behind
them (`infra-client-requirements.md` §8, §10.6, §10.7).

**Milestone 12, the command line** (`rhtn-cli`). Exit: every object in
`test-vectors/corpus.json` decodes and prints from the binary, and a probe
resolves a locator, fetches an archive and queries a catalog over a real
session.

It sends nothing outside `wire-format.md` §9.2's read-only class. A prekey
fetch consumes a one-time key and a resource request has an application
effect; neither belongs behind a command whose purpose is to look. It
decodes with the parser a node uses and no other, because a second and
laxer decoder written for convenience would disagree with the first
invisibly.

Done (2026-09-12). Every byte-class corpus entry decodes and prints from
the binary under the kind the corpus declares, and a probe resolves,
fetches an archive and queries a catalog against a running node over a
real session. `inspect` prints the shape in diagnostic notation, an
envelope's derived txid and its signers, and reports a signed object no
key is held for as unverifiable rather than failing, which is §3.4's
distinction. `keys` mints an identity readable by its owner alone,
refuses to replace one, never prints a private half, and reproduces the
seeds `test-vectors/keys.md` derives. DMN-05 and DMN-06. The argument
parser is hand-rolled, for the reason the daemon's configuration format
is: section 7 lists the choice of one as the author's, and taking none
leaves it open.

**Milestone 13, the boundary** (`rhtn-ffi`). Exit: the facade covers the
ceremony, recovery, attach and payload, with the platform's channels,
camera and clock arriving as callbacks, and a generated binding for one
platform compiles against it.

The facade translates and never adjudicates. The camera's metadata is
stripped at the boundary (`light-client-requirements.md` §1.3), and the
platform's clock is the clock: a skew a shell corrected silently would move
a witness's tolerance check without saying so (§1.2).

Done in part (2026-09-13). The facade carries the value types, the
platform's six objects inward and the client's operations outward, and a
test drives a client through it from a shell's own hardware: the intent
crosses as fields, the channels come back strongest first with nothing
promoted, and every refusal is a value carrying its reason. DMN-10. The
one piece of real work at the crossing is that a shell hands over objects
usable from any thread while the client reaches its device through `Rc`
and never leaves its own, so each is wrapped once inside that thread.

**The facade now carries the ceremony too** (2026-09-14), which milestone
15 needed and which this milestone owed: channels exchanged, capture keys,
queries and grants, witness asks, the proposal, review and signature, and
finalisation, each as values in the shape `Intent` already had.

**The generator is `uniffi`** [author, 2026-09-16], which settles half of
what the exit criterion still owed. The facade is plain Rust with no borrow
and no generic across the boundary, which is what a generator reads, so
nothing in it has to change to be read. What remains is to generate a
binding for one platform and compile the facade against it.

**Milestone 14, the shells** (`mobile/android`, `mobile/ios`). Exit: PRD-01
to PRD-05 and PRD-07 to PRD-09 are marked, which first needs
`acceptance/tools/catalogue.py`'s walk extended to `.kt` and `.swift`. That
extension is the milestone's first commit, not an afterthought: until it is
made the catalogue cannot see the tier that closes its last entries.

**Milestone 15, the instrument** (`rhtn-participant`). Exit: two `rhtnp`
processes, driven by a script, complete a ceremony with witnesses and
verifiers, take an adoption from a running `rhtnd`, and exchange payload —
every step over real sockets, none of it inside one process.

**Why it is here and not in section 5's order.** Design §24's order ends at
the library, and the tier above it goes daemon, command line, boundary,
shells. Nothing in that list is a person using the network, and the shells
wait on two decisions and a toolchain. The client itself has been finished
and tested for longer than any of them, in a process that nothing outside a
test ever started. What was missing was never a library.

**It claims none of the product entries, deliberately.** PRD-01 to PRD-09
are obligations about what a user is shown and when they are asked; a
command read from standard input is not a person, and an instrument that
marked them would be marking them falsely.

Done in part (2026-09-14). The process starts from an identity file it
reads and never mints, attaches to a serving node, publishes and sweeps,
sends and receives payload, registers and withdraws a wake endpoint, and
runs maintenance. Two `rhtnp` processes and one `rhtnd` process exchange a
payload with nothing asserted from inside any of them. PRT-01, PRT-02.

**The terminal stands in for the platform's six objects, and says so.**
Clock and randomness are the machine's. The camera returns a frame that is
the same frame every time, which proves no less than a real one would while
design §22.2 leaves the biometric profile open and the reference engine
recognises nobody. The person is a standing answer rather than a prompt,
because standard input is the command channel and a question read from
there would race the script. **And every proximity channel is unavailable
until the instrument is told what happened** —
`light-client-requirements.md` §1.3 forbids presenting a weaker channel as
a stronger one, and a machine with no radio and no camera pointed at
anybody supports none. A declaration is evidence about a scenario rather
than about hardware, which is the whole difference between an instrument
and a client.

**The ceremony runs between processes** (2026-09-14). Four `rhtnp`
processes, two meeting and two nominated to witness, reach a record every
signer names the same. PRT-04. That needed the boundary to carry the rest
of what `rhtn-client` has — channels exchanged, capture keys, queries and
grants, witness asks, the proposal, review and signature, finalisation —
which is milestone 13's own work and is now done: the facade carries them
as values, following the shape `Intent` already had.

**A step's product is one token the instrument's operator carries.**
design §7 has the ceremony cross whatever channel the two devices have and
fixes no encoding, which is why `rhtn-ffi` carries it as fields; a shell
must therefore choose one, and a harness copying a token between two
processes is the analogue of a screen and a camera. The encoding is the
instrument's and is not protocol: two instruments agreeing on another would
interoperate with each other and nothing else, which is what §7 leaves open.

**The finding was in the client's own entry point.** A participant's key
was not in its own lookup, so it could not verify a record it had just
signed — the rule `wire-format.md` §3.4 states and the daemon already holds
its own identity to. Nothing had asked it of a client, because nothing had
ever started one outside a test that passed every key in.

**The adoption leg closed on the author's question, not on a decision**
[2026-09-14]. Asked why a position was an input at all, since adopting as
a newly minted root is ordinary and adoption into several trees is
expected, and the premise did not survive the asking: `Client` held
`position: Option<Locator>`, written by nothing but one line of one test
and read by `propose_adoption` alone, so no client could ever adopt
anybody. A client now self-anchors — `wire-format.md` §2.1 names the empty
path the self-anchor case and design §2 has a root name itself — and holds
one position per subnet, derived from its own archive the way a node
derives its own. PRT-05 runs a newly minted root adopting on the record it
just made, across four processes, with nothing configured on either side.

**The framing that had to be withdrawn was mine.** Standing as a root was
called a genesis fact an instrument should not mint; it is not one. A root
self-anchors by definition, `Locator::root` is a constructor in
`rhtn-archive`, and `rhtnd` mints exactly this for itself at every start.

**Order.** Milestones 11 and 12 are independent of each other and of 13; 13
gates 14, and 13 and 15 finish together. None of them gates what the library still owes, and what it owed
is now one item: PAY-13, which waits on the licence decision (section 7).

**`rhtn-resources`, built (2026-09-13).** The sandbox and the daemon's
hosting, in one commit each. `Sandbox::admit` compiles a component and
refuses one importing a dataset the node holds, a platform capability, or a
hook inside the host's own instance that the host does not have; `serve`
runs one request in its own store, under a memory ceiling and an
instruction budget, and reports a package that spent its budget apart from
one that broke — `infra-client-requirements.md` §9 makes those different
facts. `Hosted` is the `Backend` a `Gateway` already knew how to call.
RSC-30 to RSC-35.

**The daemon binds what its configuration names**, which nothing did
before: `Gateway::bind` had no production caller, so a `view.resources` was
empty at every node and every resource request answered refused. A
`resources` key names a hosting file, `host` and `grant` a line at a time,
and a `host` line names a **manifest** rather than a component — what a
package declares is the package's (§9.1), and an operator writing role
names into their own file would be declaring them on its behalf. The
manifest and the component are checked against each other in both
directions. Every package is admitted and every grant checked before any is
bound, so a file refused at its last line binds nothing from its first, and
a daemon that will not host what it was given says so and does not start.
DMN-19 to DMN-21.

**What is not here is the supply chain.** §9.1 names signing, provenance
and an update channel, and calls them a distribution problem rather than a
protocol one. None of the three is implemented and none is claimed: a
manifest that agrees with its component is not a manifest anybody vouched
for.

**Then the split** (section 2), on its trigger rather than on a date.

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

- Whether `crates/` lives in this repository (section 2) or in its own, and
  whether the trigger section 2 proposes for moving the applications out is
  the right one.
- ~~The configuration file's format for `rhtnd`~~ — **TOML** [author,
  2026-09-14], for the operator's two files; the peers file stays a list
  and the package manifest is left as it is, being the package's own and
  the thing a supply chain will sign. The argument parser for `rhtn` is
  still open. Each fixes a dependency, and neither is forced by any
  document.
- ~~Whether the operator's view (PRD-06) is a terminal on the host or a page
  served to the operator alone~~ — **a terminal, and a page in the light
  client** [author, 2026-09-16]. A terminal on the host is expected, but
  most administration is a page in the light client backed by an SSH
  session to the node. The frontend is the client's rather than the
  daemon's, so `rhtn-daemon` grows no frontend dependencies at all.
- The payload library and its licence (section 3).
- The post-quantum provider: RustCrypto now, aws-lc-rs when, or both behind the
  trait.
- ~~The binding generator for the mobile shells~~ — **uniffi** [author,
  2026-09-16]. MPL-2.0, whose copyleft reaches modifications to uniffi's
  own files and not what links them; the client carries libsignal's AGPL
  regardless.
- The shell framework for the ceremony track.
- Every item in design §22.2, each as it is reached: parameters, the payload
  integration decisions, the biometric profile, capture-key re-derivation, audit
  calibration, replication distance, the divergence notice, archive recovery, the
  queue cap.

---

## 8. Coverage: what is established, and what acceptance tests still owe

**The catalogue is `crates/acceptance/acceptance.json`**, one entry per acceptance
test, each quoting the sentence that justifies its expectation; `crates/check.sh`
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
| Local topology table: adoption, departure and disavowal effects; bindings as a set per relationship; subtree acknowledgement; formation and lifecycle; what a participant keeps of its own horizon and materialises so a wake need not replay it | design §6.2.1, §11.2.1, §13, §15.1.1; `wire-format.md` §7.5; `light-client-requirements.md` §4.2 | 3, 14 |
| Resolution as executed: anchor table entries and budget; locator series and counter acceptance; the resolution sequence; unsigned replies verified by the handshake; endpoint records; learning an infra child's endpoints; maintenance; what a query discloses | design §12.2, §12.3; `wire-format.md` §2.3, §7.2, §7.6, §7.7; `infra-client-requirements.md` §4 | 4 |
| Propagation as executed: the push forwarding rule; the horizon boundary; loss and replay; memo generation and rootward routing | design §15; `wire-format.md` §10.1, §10.2 | 4 |
| Replication and peering: the replication set, what replicates and when; peering transactions and ASN visibility | design §3.4, §12.7.5; `wire-format.md` §4.4 | 4 |
| Currency in the running system: staple lifetime and refresh, expiry handling, nothing waiting on a staple and never failing open on held knowledge, the ladder as executed | design §12.6.5, §12.6.5.1; `infra-client-requirements.md` §3; `wire-format.md` §7.1 | 4 |
| Trust metric conformance: fixed-graph expected scores; the policy interface boundary | design §16.1, §16.4 | 5 |
| Ceremony as executed: channel ranking with no upgrade; guided capture; the sealed store, key discard and release policy; a key grant only against a countersigned query; late responses; consent over the query id; disclosure defaults; metadata stripping; decryption failure reported as inconclusive; the local face store | design §7.2, §7.3, §7.5, §7.5.2, §7.6.3, §8.1.1; `wire-format.md` §5.6, §7.3, §7.4; `light-client-requirements.md` §1 | after 5 |
| Recovery as a transaction: block assembly with hybrid responses, the successor proof, the old-key proof, series reissue as executed, acceptance beyond the evidence gate | design §9; `wire-format.md` §4.1, §4.6 | after 5 |
| Payload: PQXDH and Triple Ratchet integration; prekey distribution and service; binding to the hybrid identity; payload demultiplexing | design §14.2, §14.2.4; `wire-format.md` §7.8; `infra-client-requirements.md` §6 | after 5 |
| Traversal: the serving node as STUN; the socket that answers it delivering every datagram QUIC sent; candidates gathered and exchanged with the peer alone; the direct path attempted first and the relay on failure; control traffic dialled outward without traversal; no traversal outside the horizon | design §14.1.1, §12.6.3; `wire-format.md` §9.2; `infra-client-requirements.md` §7 | after 5 |
| Resources: catalog registration, query, lifecycle and abuse reports; scope fields; request evaluation order and refusal; role table, predicates and templates; hosted-session termination on role change; no network bindings in the sandbox; package supply chain; gateways | design §11; `wire-format.md` §6, §11; `infra-client-requirements.md` §9, §10, §11; `resource-requirements.md` | after 5 |
| What a client hands its serving node: a publication taken only from its subject, a bounded deposit, a relay answered on taking and refused for a keyhash held no record of, one wake endpoint per relationship withdrawn as readily as registered | `wire-format.md` §7.10; `infra-client-requirements.md` §6.1; `light-client-requirements.md` §4.1; design §14.1.5 | 14 |
| Daemon lifecycle and the kernel boundary: a node started from a configuration and nothing else; the identity read and never minted; consumable state loaded before the first session and written back on the way out; a restart that redelivers what was accepted and no more; a shell driving the client with no protocol content crossing to it | `infra-client-requirements.md` §1, §2, §4.3, §7, §8.1; `light-client-requirements.md` §9; design §14.1.0 | 11, 13 |
| Product-level commitments: privacy choices, warnings before irreversible actions, client-side cycle handling, operator disclosure, retention and backup | `light-client-requirements.md` §5, §6, §7; `infra-client-requirements.md` §8; design §13.7.1 | manual, per release |

**Two things the matrix does not claim.** The models establish properties of their
own abstractions, so an established row still needs the implementation shown to
agree with the model, which is what the sim scenarios are for. And nothing in either
table validates the social claims; `Robot/review-plan.md` says why.
