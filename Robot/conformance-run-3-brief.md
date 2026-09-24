# Brief for the third conformance run

For the author to hand to the reviewer, who disregards `Robot/`. Written
2026-09-23.

**Commit:** the commit on `master` that carries this brief, the first
after `dbeaecd` (2026-09-23). The gate is green there: 19 specification
hashes match the code pin, the catalogue reports 441 of 457 implemented
with 0 flags, format, clippy and `cargo deny` clean, `cargo test` green,
the reviewer's own harness at `crates/conformance/` 67 passed, 0 failed.

**Authority**, as before: the five root documents and `functional_tests.md`.
`CLAUDE.md`, `Robot/` and `change-log.md` are withheld.

**What has moved since `4edfe52` (2026-09-15)**, in the order it landed:

- **Delegation** (`wire-format.md` §8.2, §9.1; design §23.3;
  `infra-client-requirements.md` §7): the three-way bind on every
  connection mode, the 48-hour window with the receiver's leeway, the
  verified-delegation cache, the ticket clamp; the node presenting on every
  `AttachAck` and verifying on `Attach`; the delegation as a topology-class
  object with a storage rule (§10.1.1); acknowledgements travelling as
  push kind 3; the seedless node and daemon (`rhtnd` with an operator's
  material, a minted transport key and a run of credentials); the
  seedless light client and the boundary's constructor for it.
- **Per-device sessions** (`wire-format.md` §7.8, §7.10; design §14.2.4):
  a bundle per device, a session with a device, the queue and pool per
  device, a device named by the key it presents.
- **Archive fetch** (`wire-format.md` §7.9): the frontier form, verification
  by reachability.
- **Cycle repair** as a removal and a vacancy memo (§10.2.4).
- **The kernel a shell binds to** (`crates/ffi`), which is where this run
  should reach and the last one could not:
  - durable state through a platform storage object, restored whole or the
    start refused; a restart with an established payload session and a
    message queued meanwhile; backup export and restore across the boundary
    (`light-client-requirements.md` §2, design §13.7.1);
  - failover driven inside the kernel after three missed intervals, the
    cold-start fallback from a persisted sibling list, a status and a
    connection event (`light-client-requirements.md` §4);
  - the direct path on a socket of the kernel's own, gated to the horizon
    and to the person's override in both directions
    (`light-client-requirements.md` §5; design §14.1.1);
  - the catalog sweep over the session (`light-client-requirements.md` §8).
  The tests that drive these over live nodes are `crates/ffi/tests/boundary.rs`
  and `crates/sim/tests/kernel.rs`; the reviewer will want its own.

**Two node-side rules the kernel work exposed**, both worth the reviewer's
independent test: the `AttachAck` mode is the node's determination from
its own topology (`wire-format.md` §8.2), where it was the transport's
default before; and a delivery in flight ends with its session rather than
holding a device's queue until the idle timer.

**On the previous run's blocked list.** M1 (no participant backup) is
half stale: the client carries the Argon2id-wrapped envelope and the
durable store, and the boundary exposes `export_backup`, `restore_backup`
and `save`; the terminal instrument `rhtnp` exposes none of them. D3
(external wake posting) is current. CER-37, CER-38 and CER-39 remain
deferred on hardware and platform key storage.

**Four readings the previous run could have taken as open are the
author's as of 2026-09-23**: the push kind for a delegation is 2; the
ceiling of eight bundles per `PrekeyReply` (`wire-format.md` §7.8) stands;
the device count a prekey fetch reveals is priced in design §19 (P39); and
the parties who may accept a `SubtreeAck` are those within the
acknowledgement's own reach, two edges from the acknowledged node (design
§11.2.1.1, `wire-format.md` §10.1.1), not the grandpatron's siblings or the
great-grandpatron.  A test of the acceptors should place them accordingly.

**The Kotlin binding exists** (2026-09-23): the facade is annotated for
`uniffi`, the generated Kotlin compiles, and `crates/ffi/kotlin/RoundTrip.kt`
drives two participants against a real node from Kotlin; the gate's step
4b runs it where a JDK and `kotlinc` are found.  Four boundary shapes
changed for the generator: `Refused` is an enum, tuples are records, the
platform crosses by reference.

**The functional document gained two rows on 2026-09-23** for the device
holding a delegation and no seed, APP-010 and MAIL-025, and the catalogue
their entries PAY-21 and DMN-27. The document is shared: rows are added as
the base documents define functionality, and the reviewer reviews and adds
what they discover; removal is the author's.

**The rule that keeps the harness evidence** (`crates/README.md`): the
workspace repairs what stops a reviewer's test compiling and never what it
asserts. Two files were repaired that way since the move, both for
signature changes (`tests/hosted.rs`, `tests/current.rs`, `tests/boundary.rs`).
