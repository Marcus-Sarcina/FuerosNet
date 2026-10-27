# Directive surface, federation implementation and resource-suite plan — review

Reviewed 26 October 2026 against core commit `8852d5d` and the current Resource design files.

**Assessment.** The architecture is feasible: application records and policy can use a bounded HTTP carrier without exposing FN topology or transaction primitives to packages. The carrier/application split, configured instance relationships, immutable signed fields, operator visibility, and a storage-first vertical implementation sequence are useful foundations. Excluding the speculative VC machinery improves the prospects of delivering the suite.

The implementation is suitable for developing fixtures against a provisional interface, but is not yet a dependable boundary for real directives or a basis for the stronger guarantees now stated in the root requirements. Authentication happens too late or not at all; acknowledgements do not represent durable custody; retries can violate revision ordering; configuration changes do not reliably revoke queued work; and package execution can block the core node. The plan recognises several stub limitations, but incorrectly treats application idempotence as a solution to message loss and delays important safety work until after field deployment.

This review proposes changes. It does not change the core specifications, implementation, or plan. Its nine findings comprise six P1 items to resolve before field use of the directive plane and three P2 items for the contract and plan revision. Priorities refer to this new resource plane, not to every operation in FuerosNet.

**Evidence and limits**

- Read the directive surface, implementation plan, federation model and relevant component-library sections; resource requirements §3.1–3.3, infra requirements §9.2 and the four new functional rows; the node federation module, daemon listener/carrier, gateway integration, hosting/reload path and relevant tests.
- Existing node federation unit tests: **8 passed**.
- Existing daemon administration, federation and hosting integration suites: **20 passed** (7 + 1 + 12). The initial sandboxed attempt could not bind localhost; the rerun with local sockets enabled passed. This was an environment restriction, not a code failure.
- Specification pin check: **21 checked, 0 stale**. This checks recorded hashes, not completeness of behavioural coverage.
- Added **10 executable counterexample probes**, all reproduced their asserted problematic behaviour. These are review probes whose success confirms the current defect, not acceptance tests certifying correctness. No core source files were changed.
- Did not run the entire workspace gate or perform a full network-security audit. Network origin-authentication and crash-loss findings are established by code-path inspection plus state-level probes; the report does not claim an executed remote attack or exhaustive crash testing.

Probes: [Rust source](/home/aspiringsavage/Documents/FuerosNet/resource-design/directive-review-probes-2026-10-26.rs), [runner](/home/aspiringsavage/Documents/FuerosNet/resource-design/run-directive-review-probes.py). Run `python3 resource-design/run-directive-review-probes.py` from the repository root. It builds the existing node library offline with eight Cargo jobs, compiles the probes into a temporary directory, and runs them. Expected reviewed behaviour: 10 passing counterexample tests. Fixes should make the relevant counterexample assertions fail, after which the intended invariant should become a normal regression test.

**R01 — P1: a configured origin is a claim, not an authenticated sender**

Evidence: [listener exchange](/home/aspiringsavage/Documents/FuerosNet/crates/daemon/src/federation.rs:140), [listener dispatch](/home/aspiringsavage/Documents/FuerosNet/crates/daemon/src/federation.rs:219), [origin relationship check](/home/aspiringsavage/Documents/FuerosNet/crates/node/src/federation.rs:882), [raw client-key verifier](/home/aspiringsavage/Documents/FuerosNet/crates/transport/src/tls.rs:532).

The sending node pins the receiver's TLS key, which protects delivery to the intended node. In the reverse direction, the listener accepts any valid raw client key. It extracts that key only for logging after `answer()` has acted on the envelope. `answer()` passes the envelope's self-declared origin into `Federation::deliver`; no check binds that origin to the authenticated caller or to a verified instance signature.

Consequently, a caller with its own key and knowledge of a receiving instance and one of its configured peers can submit an unsigned envelope claiming that peer as origin. This is more than the explicitly documented absence of application signature verification: `Plane::offer` changes held revision state before the application can reject the signature. An unsigned revision `u64::MAX` is accepted as new; a legitimate revision 1 is subsequently classified as older and never delivered. The first probe reproduces this poisoning using the actual library. Even implementing Phase 4's application-side verifier would leave this suppression problem.

**Proposed correction:** establish authenticated ingress before mutating authoritative revision/conflict state. There are two viable divisions of responsibility:

- authenticate and authorise a hosting-node key for the specific originating instance at ingress, while the application still checks the instance signature; or
- keep unverified envelopes in a bounded staging queue and let a verified application decision promote them into accepted record state.

If instance signatures are to be the protection even against a dishonest hosting node, only verifying the claimed content before authoritative suppression, or staging until that verification, establishes it. Pinning a sender's node alone is a narrower guarantee. Specify which guarantee the product requires; do not describe a caller-chosen origin as authenticated in the meantime. Add a third-node test using a valid but unrelated TLS key and a forged configured origin, plus a forged high revision followed by a legitimate lower revision.

**R02 — P1: a reserved URL is not sufficient proof that a request is a host control message**

Evidence: [gateway guard](/home/aspiringsavage/Documents/FuerosNet/crates/node/src/resources.rs:537), [resource requirements §3.3](/home/aspiringsavage/Documents/FuerosNet/resource-requirements.md:197), and the adjacent §3.1 deployment rule.

The gateway tests the raw parsed target with `starts_with("/.rhtn/")`. Its parser preserves percent-encoded path characters. A member's `PUT /%2erhtn/envelope` therefore passes the guard and reaches a backend that decodes it as `/.rhtn/envelope`. A probe exercises the actual gateway and a decoding backend and confirms delivery. Other router normalisations need explicit treatment; a golden-byte test of the serializer does not establish agreement between parsers and routers. [RFC 3986](https://www.rfc-editor.org/rfc/rfc3986.html#section-6.2.2.2) describes percent-encoding normalisation, including unreserved characters such as the dot.

There is a second, independent qualification: a socket-backed resource may be reachable by another local process. §3.1 correctly says path and asserted headers alone cannot authenticate that connection, while §3.3's unconditional “from your host and from nothing else” statement reads as if the reserved URL solves it.

**Proposed correction:** use an explicit node-only message classification that the gateway strips from member requests and inserts only for internal control exchanges; require applications to validate it over an authenticated or exclusively accessible host connection. Define an exact path-normalisation contract and reject ambiguous aliases consistently. Preserve §3.1's direct-access protection for socket services, using an appropriate authenticated local channel or installation secret where needed. Test encoded/dot-segment variants against each supported backend router and direct socket access. Distinguish normal member traffic from host control traffic even though both traverse the same HTTP connection mechanism.

**R03 — P1: the acknowledgement chain can lose a directive permanently across a restart**

Evidence: [node pending queue](/home/aspiringsavage/Documents/FuerosNet/crates/node/src/federation.rs:839), [remote carrier removes acknowledged items](/home/aspiringsavage/Documents/FuerosNet/crates/daemon/src/federation.rs:272), [daemon persistence](/home/aspiringsavage/Documents/FuerosNet/crates/daemon/src/service.rs:920), [plan Phase 4](/home/aspiringsavage/Documents/FuerosNet/resource-design/implementation-plan.md:88).

The receiver may acknowledge a newly held envelope even if its application is unavailable or brokered and the envelope remains only in the node's memory. The sender removes its pending copy after the 2xx. If the receiving node restarts before application acceptance, neither side necessarily retains a copy. The sending node can also lose its own pending queue on restart.

Application idempotence addresses duplicate effects *if a message is delivered again*. It cannot recreate a message that no surviving component knows must be resent. The plan's claim that instance-side stores make the current restart case safe is therefore incorrect. The existing two-node test restarts the sender with a **fresh application origin that emits again**; this does not establish recovery of the original pending message.

The application outbox has the same custody gap. `GET /.rhtn/outbox` provides no explicit per-item acknowledgement or rejection to the application. Deleting an item when serving the GET risks loss before node admission; keeping it forever creates repeated admissions and potentially fills the bounded pending queue. Emissions returned with a successful incoming delivery can also be refused without a durable result returned to the emitting application.

**Proposed correction:** specify each acknowledgement: accepted into durable transport custody, applied by the application, or refused. Use a durable application outbox with stable item IDs and a durable receiver inbox; acknowledge only after the receiving component has committed responsibility. Make local application mutation, incoming deduplication and receipt/outbox creation atomic where they share a store. Use retry plus idempotent application effects; do not promise transport-level exactly-once delivery. Test crashes before and after each custody transfer, including both sender and receiver restart and a lost acknowledgement. This is local durable messaging, not distributed consensus.

**R04 — P1: retries and backpressure violate the stated record-delivery guarantees**

Evidence: [offer and held state](/home/aspiringsavage/Documents/FuerosNet/crates/node/src/federation.rs:588), [declined delivery](/home/aspiringsavage/Documents/FuerosNet/crates/node/src/federation.rs:686), [pass and local forwarding](/home/aspiringsavage/Documents/FuerosNet/crates/node/src/federation.rs:901).

Three distinct reproduced cases contradict the claim that a resource sees each revision once and never receives a stale copy:

1. Queue revisions 1 and 2. Let the application reject the first attempt at 1 and accept 2. The next pass delivers 1 again. The observed sequence is **1, 2, 1**. If 2 were a withdrawal, the stale retry could reintroduce a live record unless the application independently enforces revision state.
2. Fill a local receiver's inbox to 256. A new sender emission is removed from its pending list even though `target.offer` refuses it for capacity. The refusal is transient output; it is not retained in the sender's dropped queue. Across-node forwarding, by contrast, retains failed sends.
3. After 32 declined attempts, a delivery is dropped but its revision remains held. Reoffering the identical envelope is classified as duplicate and does not requeue it, even if the application has recovered.

**Proposed correction:** separate admitted, waiting, applied and terminally failed states. Preserve pending work on retryable refusal. Define per-record ordering: either block later revisions behind an earlier one, or explicitly supersede queued older snapshots; do not accidentally implement both. A dead-letter/retry action must requeue an unapplied record without requiring its origin to invent a new semantic revision. The application should still enforce idempotent, monotonic application because an acknowledgement can be lost. Amend the root requirements and PKG-016 to state the achievable guarantee and test these cases.

**R05 — P1: configuration changes do not revoke queued authority**

Evidence: [relationship reconfiguration](/home/aspiringsavage/Documents/FuerosNet/crates/node/src/federation.rs:577), [delivery pass](/home/aspiringsavage/Documents/FuerosNet/crates/node/src/federation.rs:901), [hosting reload](/home/aspiringsavage/Documents/FuerosNet/crates/daemon/src/administration.rs:592).

The relation is captured when an envelope enters the inbox. Removing the patron before delivery does not change it: the envelope is still handed to the application with `rhtn-relation: patron`. Pending remote work likewise remains eligible for `abroad()` after its destination is removed, if a location is retained. Also, a failed `PUT /.rhtn/relationships` does not stop envelope delivery or outbox polling, so the application can act without accepting the current relationship snapshot. Three probes reproduce these cases.

Reload adds/configures planes but never calls `Federation::forget` for resources no longer hosted. Rebinding a fresh backend under an unchanged relationship set does not reset `relationships_told`. Thus the claim that relationships are announced on binding/reload is not reliably met. The configuration is also applied in several steps rather than committed as one validated snapshot.

**Proposed correction:** give each applied relationship configuration an epoch; validate the complete reload, reconcile removed instances, and atomically install it. Reset backend initialisation on rebinding even if relationship values are unchanged. Do not deliver until the relevant snapshot is accepted. Re-evaluate pending ingress and egress against current relationships, or require an explicit operator decision to grandfather work from an earlier epoch. Cancellation/holding should remain visible. Include restart, rebind, removal, patron-to-kin changes and failed reload in lifecycle tests.

**R06 — P1: package processing can block the core node, and listener admission is unbounded**

Evidence: [federate under the node view lock](/home/aspiringsavage/Documents/FuerosNet/crates/daemon/src/service.rs:881), [listener-triggered pass](/home/aspiringsavage/Documents/FuerosNet/crates/daemon/src/federation.rs:250), [listener thread creation](/home/aspiringsavage/Documents/FuerosNet/crates/daemon/src/federation.rs:100), [socket backend](/home/aspiringsavage/Documents/FuerosNet/crates/node/src/relay.rs:76).

`Federation::pass` invokes resource backends synchronously while holding `node.view`. A pass may perform hundreds of exchanges. A slow socket service or expensive confined package therefore holds the shared node state lock while other protocol and administration work waits. Incoming federation deliveries can trigger a pass over **all** hosted instances, multiplying this cost. Releasing the lock for outbound inter-node transport does not fix the local backend calls under it.

The public listener creates one OS thread per accepted connection, with no concurrency or per-peer admission cap. A five-second timeout on each read is not a total exchange deadline; a slow stream can make incremental progress indefinitely. The sender also uses unbounded `read_to_end` for a peer's response. Finally, `head_end + want` uses unchecked arithmetic on a received content length, which can overflow before the size check in the listener parser.

**Proposed correction:** dispatch bounded per-instance work outside the core view lock, then commit results with a configuration/record-version check. Add fair scheduling, concurrency and byte limits, a total deadline, per-peer admission quotas and an aggregate resource-plane budget. Bound response bodies and use checked arithmetic before allocation or framing. A test should keep one package stalled while another package, ordinary node traffic and administrative cancellation continue. This is a necessary isolation property for co-hosting, not a later throughput optimisation.

**R07 — P2: the interface needs an explicit v1 scope and several contract decisions**

Evidence: [surface envelope and bounds](/home/aspiringsavage/Documents/FuerosNet/resource-design/directive-surface.md:91), [envelope validation](/home/aspiringsavage/Documents/FuerosNet/crates/node/src/federation.rs:185), [emission rule](/home/aspiringsavage/Documents/FuerosNet/crates/node/src/federation.rs:703).

The interface can be a useful first-hop carrier, but it does not implement the full application federation model yet:

- **Onward forwarding:** emitted envelopes must have the emitting instance as origin, while ingress requires the original origin to be a direct configured relationship. A → B → C cannot preserve A's signed envelope under those rules. Multi-hop parent/child propagation needs separate immediate-sender and origin semantics, or an explicit restriction to one hop in v1. Reissuing a new record under B is a different provenance model. Local delivery appends a path element; the remote carrier currently sends the path unchanged. There is no path-membership loop check, only a length cap.
- **Inline body requirement:** a live envelope with no body is accepted. A later complete copy is suppressed as duplicate; a probe confirms this. If offers-then-pull is deferred, require the inline body. If it is introduced, model offer, body acquisition and application acceptance as distinct states.
- **Capacity:** 4,096 held identities is a lifetime ceiling per instance until restart, with no retirement or compaction contract. A shared record carrier needs a retention/recovery policy; simply forgetting tombstones would weaken replay protection. Application quotas and durable compact state should be designed together.
- **Keys:** specify instance public-key introduction, proof of possession, signature algorithm, rotation, restoration and maximum encoding size. The 512-byte signature cap should follow the chosen scheme rather than silently choose it. “Instance keypairs enter the manifest” in Phase 4 must distinguish public metadata/key handles from private per-instance custody. Private identities should be provisioned per instance, not shipped in a reusable package manifest.
- **Wire compatibility:** version the host exchanges themselves, define unknown-field and schema-version behaviour, and publish independent canonical-byte vectors. Current tests show that route changes leave signed bytes unchanged; they do not establish an interoperable byte fixture.
- **Conflicts:** `Held` stores only the latest revision, digest and state. If the node is expected to detect forks against older revisions retained by the application, it cannot do so from this state alone. Scope the node's guarantee to the retained watermark, or retain/query the necessary history. Also specify whether a schema change at the same revision is a conflict; the present duplicate test does not compare schema.

There are smaller documentation inconsistencies: the prose says sixteen path entries are refused, while code refuses more than sixteen; unconfigured emitted destinations are described as held but `emit` rejects them; refusals are said to be visible on the page but `Passed.refused` is not retained in the plane. Correct these against the intended contract rather than expanding implementation to satisfy accidental wording.

**R08 — P2: the new functional requirements exceed the demonstrated test coverage**

Evidence: [PKG-015/016](/home/aspiringsavage/Documents/FuerosNet/functional_tests.md:654), [OPS-021/022](/home/aspiringsavage/Documents/FuerosNet/functional_tests.md:732), [hosting test](/home/aspiringsavage/Documents/FuerosNet/crates/daemon/tests/hosting.rs:783), [two-node test](/home/aspiringsavage/Documents/FuerosNet/crates/daemon/tests/federation.rs:197).

The tests are useful executable examples and passed, but several requirements remain unproved:

| Requirement | Current evidence | Missing evidence |
|---|---|---|
| PKG-015: socket and sandbox parity | Socket-backed instances plus fake carriers | All three control exchanges through a real Wasmtime guest; encoded aliases and direct-access protection |
| PKG-016: record rules and bounded envelopes | Basic offer outcomes, digest checks, local crossing | Retry ordering, lost acknowledgements, same-revision withdrawal, queue saturation, bodyless live record, independently fixed signature vectors |
| OPS-021: reload and instance relationships | Configuration reader and administration happy path | Changed/removed relationships with queued work, backend rebind, removed instance, failed reload rollback |
| OPS-022: outage and later delivery | Correct/wrong destination pin; fresh sender origin after restart | Same queued item surviving outage/restart, receiver custody crash, unrelated caller impersonating a configured origin |

No acceptance-catalogue entries matched the reserved paths, federation route, origin/relation header or configured-relationship vocabulary, and the new tests carry no corresponding acceptance markers. The functional rows and ordinary tests exist, but the catalogue-driven debt tracking has not been extended to cover them. Add traceable acceptance entries and adversarial tests; do not count a green existing gate as complete coverage of the new rows.

The statement that everything on the directive-surface page is implemented and held to by tests should be narrowed to the behaviour actually exercised. In particular, known in-memory persistence limitations need to qualify the root delivery guarantees, not only a working interface document.

**R09 — P2: the plan is a credible roadmap, but its first milestones need stronger contracts and less simultaneous scope**

Evidence: [implementation plan](/home/aspiringsavage/Documents/FuerosNet/resource-design/implementation-plan.md:15).

The sequence Commons → Exchange → Academy/Press → Forge is sensible: it exercises general collaboration, then scarce-resource transactions, then domain-specific records. The second-template reuse test is more meaningful than building all fifty original use cases. Keeping Git and other heavy native systems external is also practical.

Recommended revisions:

1. **Phase 0 should prove transactions and recovery, not just write/read persistence.** Define isolation, atomic compare-and-set/transactions, record-plus-outbox commit, crash recovery, quotas, blob ownership/garbage collection, schema migration, backup/restore and cancellation. A fresh Wasmtime `Store` per request can remain; persistent application data need not live in the VM instance. Host-managed SQLite is a plausible implementation, but the application interface must actually expose the required atomic operations. SQLite provides local transactional guarantees; arbitrary sequences of independent host calls do not inherit them automatically. [SQLite transactional behaviour](https://www.sqlite.org/transactional.html).
2. **Clarify the permitted host surface.** New storage/job bindings change the present exact request/response-only import surface and need a versioned contract plus updated tests. They need not expose forbidden FN state. Conversely, the proposed service bindings are not the only way to preserve confinement: a carefully restricted filesystem capability is another design option supported by Wasmtime. That is an alternative to evaluate, not a recommendation to expose a general filesystem. [Wasmtime preopened directories](https://docs.wasmtime.dev/api/wasmtime_wasi/struct.WasiCtxBuilder.html#method.preopened_dir).
3. **Move authentication and durable delivery into the foundation milestone.** Signature scheme design appears in Phase 1 but verification closes only in Phase 4, after field deployment starts in Phase 2. At minimum, Phase 2 must keep federation disabled or restricted to a clearly labelled experimental environment until the required ingress, custody and lifecycle properties pass. Prefer settling them alongside storage.
4. **Use one primary runtime first, with an adapter parity test.** Requiring the entire first product to run in both Wasmtime and socket form immediately increases the test and operational surface. A common application interface plus a deliberately small parity fixture proves portability earlier; the full second deployment profile can follow when the first is usable. A socket service can still have OS-level confinement; “first party” alone is not an isolation mechanism.
5. **Do not make the whole framework precede its first application.** Build the minimum store, permissions, audit and outbox alongside Clubhouse. Phase 1 currently asks for booking-capacity acceptance before the actual booking domain appears in Phase 3; distinguish a transactional storage fixture from the later booking invariant. Phase 2 also needs an explicit delivery plan for UI, object-filtered search, attachment lifecycle and client integration, not just the back-end crate list.
6. **Make Exchange test domain correctness as well as reuse.** “No Tier-1 component copied” is helpful but insufficient. Require concurrent capacity enforcement, permission changes while work is queued, cancellation/refund/adjustment semantics where applicable, and ledger balancing. A double-entry ledger can have a small implementation but its correctness obligations are not reduced by its line count.
7. **Keep application semantics in the Resource library.** Directive evaluation, conflict presentation, object visibility and compliance evidence belong in the package. The core node should own the minimum authenticated, bounded, durable transport/hosting contract. Reuse the same library if useful, but avoid making every future record policy or schema extension a core-node release requirement. This also makes the eventual repository separation easier.

A more concrete foundation contract should include a policy example: product-fixed visibility, local configuration and patron directives combine without widening protected records; incompatible directives yield a visible conflict; a receipt identifies the directive digest, configuration epoch and actual application result. “Received”, “accepted”, “applied” and “compliant” should not be one status.

**Proposed implementation sequence**

| Milestone | Deliverable | Exit evidence |
|---|---|---|
| Foundation contract | Versioned package API, trusted control channel, per-instance transactional state and key custody | Host-path spoofing refused; cross-instance isolation; transaction rollback; restart recovery |
| Reliable first-hop carrier | Authenticated ingress, durable inbox/outbox, explicit acknowledgement meanings, revocation and bounded scheduling | Forged-origin rejection; crash matrix; queue pressure; no stale policy application; one slow package cannot block the node |
| Clubhouse vertical slice | Discussion, event, attachment, restricted search, notifications, export in one primary runtime | End-user task completion and permission tests; restore and upgrade of an actual instance |
| Exchange composition | Lending and repair workflow using the shared library | Capacity races, cancellation, role separation and audit/ledger invariants; demonstrated reuse |
| Broader suite | Academy, Press and scoped Forge; optional brokers and second runtime | Domain-specific acceptance criteria, supported module combinations, custody and compatibility rules |

This preserves the intended product scope and most of the ordering. The important change is to establish what an authenticated, accepted and recoverable message means before application authors and field users depend on it.
