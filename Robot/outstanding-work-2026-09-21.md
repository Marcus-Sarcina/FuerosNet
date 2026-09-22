# Outstanding work before the kernel-facing API is committed — working note

**For:** Reconfigurable-Hierarchic Trust Network
**As of:** 2026-09-21, `master` at `0c67a2f`
**Status:** working note, revised once against an external review the same day
(section 10), and updated after the signing-authority rulings were applied to the
documents, the catalogue and the client the same evening (`change-log.md`
2026-09-21). Section 8 lists the readings and decisions that are the author's;
nothing below settles one.
**Standing:** the same as `review-tracking.md`. **Nothing in the root may cite
this file.** Items that survive move into the catalogue, the plan or the
documents and are cited from there; this file keeps the survey.

**Why now.** Passes 0.6 and 0.8 of cycle 3 are set aside for the time being
[author, 2026-09-21]: adequate reviewer responses have become hard to obtain
under the reviewers' security overrides. The programme therefore stops at 0.7
and 0.8.1 for this cycle, and the question becomes what the workspace owes the
documents as they now stand, and what the application tier needs from the
library. This note is that survey, made by running every checker and reading
the code against each change since the last code commit rather than by
reading the plan's own claims about itself.

**What the title means, after review.** Shell scaffolding, navigation and
build setup can start at any time; nothing here forbids them. What must not
happen is committing the boundary a shell binds to while the decisions in
section 8 are open, since a rule fixed in a shell to work around an unsettled
kernel is absent from every other client (design §1.1). Section 9 therefore
sorts every item into three milestones: before the kernel-facing API is
committed, during the first shell, and before a conforming release.

**The distinction the review drew, kept here.** *A component exists*, *a
running kernel drives it*, and *a shell can use it through the supported
boundary* are three different facts. The acceptance count establishes the
first. Several items below are the second and third at behaviours the first
already covers.

---

## 1. State as found

Every number below was produced on 2026-09-21.

| Check | Result |
|---|---|
| `Robot/refcheck.py` | 2,233 references across 5 documents, Robot/ citations across 6; 0 flags |
| `Robot/modelrefcheck.py` | 1,190 citations across 249 files in `models/` and `crates/`; 0 flags |
| `Robot/stalecheck.py` | 1 + 10 + 3 = 14, the baseline. A diagnostic, not proof that nothing else is stale: section 2.2 names two sentences it cannot see |
| `crates/acceptance/tools/check.py` | 405 of 415 implemented, 0 flags; 2 withdrawn, 3 deferred (counted as implemented); stubs in sync |
| `Robot/matrixcheck.py`, run from `crates/` as `python3 ../Robot/matrixcheck.py` | 87 conditions: 51 both sides, 33 one side, 0 neither, 3 deferred; 0 flags |
| `crates/check.sh` | **CODE GATE PASSES**: clippy clean, 494 live tests and 13 ignored, five fuzz targets with no crash |
| `test-vectors/tools/verify.py` | ALL CHECKS PASS, **against a stale pin** (section 3) |
| `conformance-review/` harness | 61 of 67 pass; the six-test `daemon` target **does not compile** (section 5) |

The mechanical picture is green against the wrong baseline. The last
substantive code commit is `83ea784` (2026-09-15), plus CUR-19 and CUR-20 on
09-18. **Twenty-seven commits landed on the root documents between 09-16 and
09-18**; two change what the wire carries (the delegation and the archive
reply) and one the authentication step's rule. `test-vectors/tools/spec-pins.json`
is stale on all three pinned documents, the corpus encodes the old
`wire-format.md` §7.9, and `rhtn-codec` agrees with the corpus rather than the
specification. **Nothing in the code gate would have said so** (section 6):
the gate checks quotes, stubs, lint, tests and fuzzing, and compares no
specification hash.

---

## 2. Implementation updates owed to specification changes since 09-15

### 2.1 Transport delegation

design §23.3 and design §14.1.3; `wire-format.md` §8.2, `wire-format.md` §9.1 and
`wire-format.md` §1.1; `infra-client-requirements.md` §4.4 and
`infra-client-requirements.md` §7; `light-client-requirements.md` §4.1 and
`light-client-requirements.md` §2. Landed 2026-09-16 and 09-17.

**None of it is in the code.** `grep -ri delegat crates --include=*.rs` hits
three lines, all comments in `node/src/resolution.rs`. What the code carries
instead:

- `crates/codec/src/cose.rs` lines 138–150: **thirteen** signing domains;
  `wire-format.md` §1.1 lists fourteen.
- `crates/codec/src/schema.rs` lines 82–83: `ATTACH` has no field 4 and
  `ATTACH_ACK` no field 6.
- `crates/transport/src/tls.rs` line 120: the dialling side fails the handshake
  on *presented raw public key is not the pinned classical member*, with no
  deferral to the attach. `wire-format.md` §9.1 now binds by either check and
  refuses the session only when neither holds.
- `crates/daemon/src/config.rs` lines 42–46: `rhtnd` reads the signing seed.
  design §23.3: an instance holds a delegated credential and never the seed.
- `crates/node/src/resolution.rs` lines 652 and 671: the node signs its own
  endpoint records and anchor entries. `infra-client-requirements.md` §4.4: an
  instance cannot mint its own; the signature is the operator's, made on the
  device that holds the key.

**The surgery is not fully specified, and the code is where that shows.**
Three statements, any two of which can hold: design §23.3 keeps the signing
key on the ceremony device; `wire-format.md` §8.2 has the delegated transport
key *sign nothing beyond the handshake*; and the documents require a node to
produce identity signatures unattended, which the code does by reading the
seed. Built from every `aad::` use outside tests:

| Context | Who signs, per the documents | Where the code signs it today | Unattended? |
|---|---|---|---|
| `endpoints`, `anchor` | the node's identity (`wire-format.md` §7.6 and `wire-format.md` §7.2) | `crates/node/src/resolution.rs` lines 641–682 | rare; provisionable in advance |
| `currency` | the issuer, per attestation (`wire-format.md` §7.1; `infra-client-requirements.md` §3) | `crates/node/src/currency.rs` line 345, `self.identity` | yes, on every issuance. **Ruled: no change needed; the staple carries the delegation** [author, 2026-09-21] |
| `subtree-ack` | *the grandpatron's node, not its operator* (`wire-format.md` §7.5) | `crates/archive/src/topology.rs` line 565, `AckIssuer.identity` | yes, by the section's own words. **Ruled: the temporary key; the adoption is recognised locally at once** [author, 2026-09-21] |
| cycle repair, today a type-3 disavowal | `wire-format.md` §10.2.4 | `crates/node/src/propagation.rs` line 716 mints the envelope | yes. **Ruled: a removal, not a disavowal; the vacancy memo suffices; §10.2.4 reworded** [author, 2026-09-21] |
| `prekey` | the subject (`wire-format.md` §7.8) | `crates/archive/src/prekey.rs` line 38, `SigningIdentity` | a delegated desktop or instrument rotating its bundle has no seed |
| `catalog` | the owner (`wire-format.md` §6.1) | `crates/archive/src/catalog.rs` line 183 | an owner registering from a delegated device |
| `locator` | the subject (`wire-format.md` §2.3) | `crates/archive` | a delegated device changing address, or sealing |
| `envelope`, `consent`, `verifier` | the participant | `crates/client` | the ceremony device, by design |

`app-requirements-notes.md` line 188 says a light client's identity key is used
in three contexts; the table has six a participant device uses and four a node
does. **Each row needs its executing device, its authorised signer and its
offline behaviour decided, and the documents reconciled, before the model,
the vectors or the code are written** (section 8). Substituting the transport
key into any row changes that row's authorisation rule; keeping the identity
key on the instance contradicts design §23.3; requiring the phone online for
every row ends unattended operation and the 90-day run.

**The bind is specified at the attach only, and four production paths never
attach.** `wire-format.md` §9.1's two-branch check names *the attach that
follows*. These authenticate by the TLS pin alone and open no session:

- `crates/node/src/resolution.rs` line 933, `contact`: a resolution or
  request-only connection is `Reached` at the handshake.
- `crates/transport/src/traversal.rs` line 359, `connect_direct`: the direct
  path keeps the first completed handshake.
- `crates/adaptors/src/direct.rs` line 138, `hold`: an inbound direct
  connection is identified by its pinned classical SPKI.
- `crates/transport/src/session.rs` line 844, `handle`: a direct payload path
  is served without a session opening.

**Ruled [author, 2026-09-21], and applied.** The direct path is inside the
horizon (design §12.6.3), where the peer's delegation is already held from the
topology class, so paths 2 to 4 bind by a held delegation with no attach. For
request-only contact from outside the horizon, a delegated peer presents its
delegation first on every connection: `Attach` field 4 and `AttachAck` field 6
on a session, control frame type 7 first on stream 0 otherwise; a dialler
whose pinned check fails and who holds no delegation waits for that frame and
refuses the connection on anything else, and a server attributes a delegated
client's requests only after it (`wire-format.md` §8.0, §8.2, §9.1;
`infra-client-requirements.md` §7; `light-client-requirements.md` §4.1).
Entries TRN-18, TRN-19 and TRN-20. The model (section 4) now has one rule for
every connection mode to state. One consequence to confirm in code: a light
client's horizon view must store delegations, since a phone dialling a
delegated desktop is the direct-path case.

By crate, once section 8's decisions are made:

| Crate | Owed |
|---|---|
| `rhtn-codec` | `Delegation` (five fields, hybrid `COSE_Sign` over 1–4); `Attach` field 4; `AttachAck` field 6; `rhtn/1:delegation` as the fourteenth AAD; the delegation's topology-class carriage [author, 2026-09-21] |
| `rhtn-crypto` | Issue and verify a hybrid delegation; refuse a classical-only one |
| `rhtn-transport` | The three-way bind on both sides (pinned member; a delegation held from the topology class; the delegation presented first on the connection, as control frame 7 where no session opens), on every connection mode including `contact`, `connect_direct`, `hold` and the serving socket's direct branch. A verified-delegation cache keyed by transport key. **A resumption ticket clamped to the delegation's remaining validity** (design §14.1.3), which needs a lifecycle that keeps and uses resumption state (section 7). TRN-03, TRN-04 and RES-12 were re-derived on 09-17 (`fd306c5`) and pass today only because the code fails at the stricter branch; they are rewritten when the deferral exists |
| `rhtn-node` | Present the delegation on every `AttachAck`; verify a client's on `Attach`; flood its own delegation on issuance and hold others' as current state in the topology store, replaced by `not_before`; verify acknowledgements and attestations against a held delegation; cycle repair as a slot removal and a vacancy memo, not a type-3 envelope; a currency staple carrying the issuer's delegation; the seen-set as txids with the fold as the state |
| `rhtn-daemon` | The identity file becomes what section 8 decides the instance holds, plus a forward-dated run of 45 × 48 h (`infra-client-requirements.md` §7); the *say so while it is still long* notice to the operator; refuse to serve on a lapsed credential; operator-signed objects as configuration |
| `rhtn-client` | Issue delegations; verify and cache the node's delegation on attach (`light-client-requirements.md` §4.1); sign what the table assigns to the ceremony device; `backup::Contents` (`crates/client/src/backup.rs` line 95) gains the provider-credential slot `light-client-requirements.md` §2 requires |
| `rhtn-ffi` | Delegation issuance and import across the boundary, and a constructor for a device that holds a delegation and no seed (`crates/ffi/src/client.rs` line 273 takes 64 bytes of seed and nothing else) |
| `rhtn-participant` | Whatever mode section 8 gives an instrument |
| `rhtn-cli` | `inspect` decodes a `Delegation`; `keys` may mint a transport keypair |
| acceptance | No entry exists. Needed: issue, present, verify; negatives for a delegation naming a key other than the one presented (the replay), one outside its window, one by a different keyhash, one with a classical-only signature; the cache hit; a ticket outliving the delegation refused; `AttachAck` field 6 present from an instance; the run held and the notice raised; one entry per non-attach path as decided |

### 2.2 Archive fetch — a breaking wire change, and two sentences it left behind

`wire-format.md` §7.9, landed in `7e625d2` (2026-09-17). The specification now has
`ArchiveRequest` field 2 as a frontier `[+ txid]`, `ArchiveReply` field 2 as
`PresentedRecord / Envelope`, field 4 as a frontier present iff field 3, and
verification by reachability against the requested frontier. The code has the
old shape throughout:

- `crates/codec/src/schema.rs` line 89 (`ARCHIVE_REQUEST` field 2 a single
  `Bytes32`) and line 106 (`ARCHIVE_REPLY` field 2 `Envelopes` only, field 4
  `Bytes32`).
- `crates/archive/src/chain.rs` lines 20–27 (`head: Option<Txid>`) and 68–74
  (`continue_from: Option<Txid>`).
- `crates/archive/src/walk.rs` line 206: `verify_batch` still fails on *first
  record is not the requested head*.

**Two root sentences still state the old rule**, found by search rather than
by the staleness sweep: `light-client-requirements.md` §2 lines 284–285, *each
returned record's back-pointers must match the record following it, and the
first must match the head requested*; and `wire-format.md` line 720 (§3), *each
presented record's back-pointers match the record following it*. `7e625d2`
fixed the instance and not the claim. Both go into the specification repair
before the oracle is derived from it.

Reaches: codec; archive (`serve`, `verify_batch`); node; client, including
ARC-20's subject-served archive and the holder's presentation choice under
`wire-format.md` §4.5.1; `rhtn probe`; the generator and the corpus (section 3).
**A live round trip is a separate fact from the new shapes**: the client's
`serve_archive` (`crates/client/src/ceremony.rs` line 1152) queues its reply in
the outbox, which nothing sends until a maintenance call drains it (section 7).

### 2.3 The design §19.6 disclosure, withdrawn and still emitted — **applied 2026-09-21**

design §19.6 (`9ac3081`, 2026-09-17) now says *neither is owed a warning*, and that
a per-query notice would convey who is meeting whom. The client still emits
`Notice::RecordDisclosure` for `Role::Verifier` and `Role::Witness` at
`crates/client/src/ceremony.rs` lines 543, 596 and 736;
`crates/client/tests/ceremony.rs` lines 212–237 assert them; CER-26's `then`
still requires a disclosure *to each witness and verifier at the moment it is
asked*; CER-17's interpretation still describes the tension between design
§7.3 and design §19.6 as open. Keep the participant's notice, remove the other
two, narrow the FFI `Told` role and the terminal rendering, and re-derive both
entries.

### 2.4 Catalogue entries requoted but not re-derived — **ARC-08, ARC-09, ARC-11, DMN-09, CER-26 re-derived 2026-09-21; the ICE three wait on section 8**

`ed25768` (2026-09-17) replaced the `rule` quotes of nineteen entries to keep
`check.py` green and re-derived none of their `given`, `when` or `then`. Eleven
are punctuation. Eight carry a semantic change:

| Entry | What is stale |
|---|---|
| ARC-08 | `then` still says the first envelope's txid equals the requested head and each record names the one that follows it |
| ARC-09 | title still *do not match the records that follow* |
| ARC-11 | requested head, now a frontier |
| DMN-09 | the batch's own links, now reachability |
| CER-26 | section 2.3 |
| TRV-02 | design §14.1.1 now reads *ICE checks candidate pairs in priority order* |
| TRV-07 | the same |
| PAY-15 | the same: *using the serving node where none does* |

`check.py` cannot see this class: it checks quotes against the text, not
expectations against quotes.

**The three ICE entries are not only stale text.** `crates/transport/src/traversal.rs`
line 359 dials every candidate at once and keeps the first handshake, citing
RFC 8445 §7 with QUIC's handshake as the check. design §14.1.1's sentence was
written in cycle 3's 0.1 pass as a factual correction about what ICE does.
Whether it is a description of ICE or a requirement on this implementation is
a reading for the author (section 8); re-deriving the entries without settling
it would quote a rule the code does not follow.

### 2.5 The no-history rule for every request class

`infra-client-requirements.md` §1 (`31c39d0`, 2026-09-18) states process-and-discard
once for every request class, naming catalog queries, currency fallbacks,
publications and deposits that had no statement before; `infra-client-requirements.md`
§10.2 says the role table keeps no history of replaced rows; design §19.8
registers the composition as C23. The code predates the wording. Owed: an
audit of `crates/node/src/catalog.rs` and `currency.rs` for per-requester
retention; a check that the row provenance added for G01 on 09-15
(`resources.rs`) is provenance and not row history; and positive/negative
pairs for each newly named class.

### 2.6 Confirmations, low cost

- design §10.1's pruning is anchored to the patron-countersigned reissue
  (`ad675f7`). `crates/archive/src/chain.rs` line 125 already prunes only at a
  reissue. Confirm with a test rather than by reading.
- `wire-format.md` §10.2.2's memo-clock note (`ad675f7`) is prose only.
- The coherence commits `e5c966e` and `79867ea` (2026-09-17) settled twelve
  contradictions in prose the code was written against. Four to confirm by
  test rather than by reading: design §11.4, a running request completes and
  only the resource-facing session ends (`crates/node/src/resources.rs` line
  216 drops the row and the session); design §11.4, permission past the
  horizon is refused *scope or grant alike*, where `Gateway::set_row` checks
  roles and `refresh` removes rows outside `horizon(owner, 2)` afterwards, so a
  named row for an outside party may stand until the next refresh; design
  §6.4, verifier responses are sought and their absence weighed, never
  required; design §15.2, a memo conflict is confirmed against the node's own
  records and never fetched (`crates/node/src/resolution.rs` line 832 does not
  fetch). `resource-requirements.md` §7.2.1's absolute-rank displacement is
  **settled** (`infra-client-requirements.md` §10.2 names both rank classes;
  O-009 resolved 2026-09-17) and unbuilt: section 6.

---

## 3. Test vectors: the pin is stale on all three documents

`spec-pins.json` pins `wire-format.md` `7862d2…`, `network-design.md` `bc59e9…`
and `light-client-requirements.md` `04db8a…`; the current hashes are `5fd2a0…`,
`15ba43…` and `a10d76…`. `test-vectors/tools/generate.py` line 2401 (`r_archive`)
and line 3094 (`P-archive-reply-continued`) encode a single-txid continuation.
Regenerating needs `--accept-spec-change`, which asserts an audit of the
hand-authored fixtures across the whole delta. That audit is what 0.6 phase 2
would have done independently; with 0.6 set aside it is in-house work.

**The pin is a whole-document hash, so it advances once, for the whole delta.**
An archive-only regeneration would record the current hashes over a corpus
whose delegation constructions are still absent, and the pin would then say
*audited* of fixtures nobody had written. Either every fixture the delta
touches is reconciled in one regeneration, or the partial state is represented
explicitly rather than by a green pin. Section 9 takes the first.

Owed, in that one regeneration:

- The `wire-format.md` §7.9 shapes, plus a merge-crossing batch with a two-txid
  frontier and a presented presence-record entry.
- A known-answer signature for the fourteenth domain, `rhtn/1:delegation`,
  with its wrong-signer analogue, in `records.md`.
- `Attach` with field 4 and `AttachAck` with field 6 in `messages.md`.
- Negative vectors for the delegation classes in section 2.1.
- The prepared `P-currency-successor` (tracking, 2026-09-18).
- Then `crates/crypto/tests/corpus.rs` and DEC-01/DEC-02 rerun against the
  new corpus.

**Decision (section 8):** `test-vectors/runner-rs` was to be retired once the
codec passed every entry; update it for the new shapes, or retire it now.

---

## 4. Models — **done 2026-09-22**

`models/tamarin/wire-only/attach.spthy` now carries the delegated bind in
both directions and by both of the ruling's binds: `Delegate`, a delegation
the identity signs over a fresh transport key; `Server_Respond_Delegated`
and `Delegated_Client_Answers`, the delegation presented first; `Client_Holds_
Delegation` and `Client_Finish_Held`, the delegation already held from the
topology class; `Server_Bind_Delegated`; `Compromise_Instance`, a seized
instance as design §18.1's residual stated as a carve-out. The comparison
the wire states, *field 1 against the key the handshake presented*, is its
own term on both sides, and the gate now runs three mutations of it that
must falsify: the client-side comparison, the server-side comparison, and
the held path's verification under the pinned material. Eleven lemmas verify
(seven before); `a_delegated_bind_names_a_key_the_identity_delegated` is the
new all-traces property, with three exists-trace guards against vacuity.
`run-all.sh`: ALL MODELS PASS, 38 wire-only lemmas, ten mutations falsified.
What the theory says it does not reach: the window and leeway (no clock),
the connection modes (one exchange), the flood (a held delegation arrives by
`In`), and the run. The record below is what section 4 asked for.


Results in `models/results/` are dated 2026-09-13. `models/tamarin/wire-only/attach.spthy`
CHECK 2 (line 238) models only the pinned-classical-half bind; the delegated
bind, now every instance's normal case and the thing design §18.1's narrowed
residual rests on, has no rule and no lemma. Owed: a `Client_Finish_Delegated`
rule; a lemma that a commit under presented key *k* implies the intended
identity signed a delegation naming *k* within the window; a mutant dropping the
key check, which must find the replay; **and the bind stated once for every
connection mode, since the ruling of 2026-09-21 made the delegation the first
frame on any connection rather than a property of the attach** (section 2.1);
then a rerun of `models/run-all.sh` for the
rest of the suite against the current text. Rerunning the unchanged theories
establishes nothing about the mechanisms they omit.

`models/tamarin/wire-only/currency.spthy` line 246 and `models/README.md` line
285 state the premise `fc8f131` withdrew: *a staple decides which key to
address*. The lemmas do not assert it (`Accept_Currency` records the
attestation's key as accepted and proves binding, issuer and expiry), so this
is a comment and a README to restate, not a lemma to repair; the ruling is
that the relying party addresses a key the participant authored and reads
field 2 only as a rotation-or-fork signal.

---

## 5. Reviewer artefacts, and what the set-aside passes cost

- **`conformance-review/`** was frozen at `4edfe52` (2026-09-15) with status
  files reporting 59 passed and 8 failed and citing `rhtn/` paths. **Moved
  2026-09-22** [author]: the harness is `crates/conformance/`, a workspace
  member the gate runs; its one broken initializer (`tests/daemon.rs` line 22,
  `Config` without `reconcile_secs`) was repaired at the daemon's own default
  under the rule that assertions are the reviewer's; 67 of 67 pass in the
  workspace. The reports at that commit stay out of the tree, their findings
  dispositioned in `review-tracking.md` as before.
- **`functional_tests.md`** already carries the delegation rows (NET-002,
  SES-001, SCH-020, SIG-005) and CUR-011/CUR-012. Its companion
  `conformance-review/blocked-tests.md` M1 (no participant backup) is half
  stale since 09-15: `rhtn-client` carries the Argon2id-wrapped envelope and
  the store on disk, and neither the FFI nor `rhtnp` exposes export or
  import. Its D3 (no external wake delivery) is current: section 6.
- **Its open-decision register** (`functional_tests.md` §9, O-001 onward) is the
  one place the specification's remaining profile gaps are enumerated, and a
  blanket citation of design §22.2 does not replace it. Each row owes a
  disposition in section 8's terms: O-007 (an unpinned intermediate's
  authentication profile), O-013 (the prekey batch reply's framing and
  failure association), O-014 (the peer backup and audit profile), O-011,
  O-012, O-015 and O-016 (capture and payload interoperability, competing-
  recovery notices and optional currency mechanisms, restore semantics, the
  selected broker, package and wake integrations). Optional mechanisms are
  marked optional rather than turned into blockers.
- **Both artefacts are untracked.** The root rule sends process files to
  `Robot/`, and the reviewer disregards `Robot/`. Placement is the author's
  (section 8).
- **What 0.6 and 0.8 would have given.** 0.6: an independent implementation
  attempt against the delegation and archive text, and the vector-as-oracle
  pass of its phase 2. 0.8: every adversary but the patron (0.8.1 ran). The
  code-facing substitute that has worked twice is the reviewer's conformance
  harness, which reads the specification and writes tests against the crates.
  **A third conformance run belongs after sections 2.1 and 2.2, at a commit
  where the pins are green, and it should reach the runtime paths section 7
  integrates rather than only the existing harness.** Stage 2's human
  cryptographic audit and Stage 3's red team against a running system remain
  as `review-plan.md` has them.

---

## 6. Library debt independent of the specification delta

**Drivers that nothing in a running kernel calls.** Each is a rule
implemented in one component with tests supplying what production does not,
the shape the 2026-09-14 review named:

- **Subtree acknowledgement is never issued or taken.** `crates/node/src/propagation.rs`
  line 294 applies every stored record with `issuer = None`; `AckIssuer` is
  constructed in tests alone; `Table::take_ack` has no runtime caller. The
  gateway (`crates/node/src/resources.rs` line 269) refuses a grandchild
  without the host's acknowledgement, so a running daemon admits no grandchild
  to any resource. Depends on section 2.1's signing decision for `subtree-ack`
  and is not closed by it.
- **Evaluation has no production driver on either side.** `keep_presence` is
  called only from `crates/sim/src/mesh.rs`; `NodeView::evaluate` and
  `standing` (`crates/node/src/trust.rs`) and the client's pair
  (`crates/client/src/trust.rs` lines 66–71) are called from tests alone.
- **The verifier's leg to a client behind its serving node is unwritten**, in
  the documents and the code alike (`crates/adaptors/src/verifier.rs` header;
  `crates/node/src/runtime.rs` line 416 fails a type-4 stream with no hosted
  verifier). A verifier on a separate phone is unreachable. Exposing
  `take_query` and `take_grant` through the FFI transports nothing to them.
  Owed: the leg settled, then a query, consent, grant, answer and subject copy
  demonstrated with the verifier in a separate process behind its node, with
  the unavailable and timeout outcomes.
- **External wake delivery does not exist.** `WakeRegister::doorbell`
  (`crates/node/src/wake.rs` line 121) has no production caller; registration,
  storage and withdrawal are built, the posting worker and the OS relay are
  not. Push is opt-in (design §14.1.4): this is a missing implementation of an
  offered feature, to be built or explicitly deferred, and not grounds for a
  new disclosure.

**The gate promised more than it checks.** `implementation-plan.md` section 3
names format, lint, tests, corpus, fuzz, deny and **a specification pin for
the code**. `crates/check.sh` runs catalogue, stubs, clippy, tests and the fuzz
smoke. It has no format step, no `cargo-deny` (not installed; no `deny.toml`),
and no comparison of the current specification hashes with the vector pin or
with a pin of its own. Repinning once (section 3) leaves the next
specification-only change invisible in exactly the way this one was. Owed: a
non-mutating freshness check in the required gate, over the specification
hashes and the fixture producer and outputs, with explicit acknowledgement
kept for rebasing; the format step; and the licence gate before the
application tier pulls dependencies.

**Also owed:**

- **PAY-13.** The Triple Ratchet's only implementation is libsignal, AGPL. The
  author's decision (`implementation-plan.md` section 7).
- **33 one-sided conditions** in `transaction-rules.md`. Eighteen have no
  positive, the dangerous direction: `cose-profile`, `declared-algorithm`,
  `body-matches-type`, `two-parties-differ`, `nested-structures`,
  `duplicate-is-not-news`, `presence-is-between-these-two`,
  `evidence-verified-before-counted`, `transfer-parties-differ`,
  `plain-rotation-carries-nothing`, `evidence-present`,
  `asn-does-not-move-standing`, `participants-differ`,
  `normal-record-is-witnessed`, `never-reissue-into-an-occupied-series`,
  `reject-an-occupied-series`, `abandoned-series-is-dead`,
  `seal-freezes-a-chainless-holder`. Fifteen have no negative.
- **Resource predicates** stop at named rows and standing horizon membership.
  The structural, rank, quantile, tenure and date evaluator and scheduled
  recomputation are unbuilt; the rule for both rank classes is settled
  (section 2.6).
- **The package supply chain** of `infra-client-requirements.md` §9.1
  (signing, provenance, an update channel) is unimplemented and unclaimed.
- ~~`crates/README.md` lists `cli/` and `daemon/` twice~~ — removed 2026-09-22.
- `implementation-plan.md` is stale in two places: line 588 says the library
  owes one item, PAY-13, which this section contradicts; and section 7's
  PRD-06 bullet (lines 657–665) still says everyday administration *rides the
  authenticated session*, written before `834a379` put administration out of
  band (`infra-client-requirements.md` §8.2) and the pages on the node
  (`infra-client-requirements.md` §8.3). Both to be aligned in the author's
  words before the plan is handed to an implementer.

---

## 7. What the application tier needs from the library first

The gap is not a list of missing exports. It is that the running kernel a
shell would bind to does not yet do several things the components beneath it
do in tests.

- **Durable operation is more than a backup slot.** `Participant::start`
  (`crates/ffi/src/client.rs` line 273) always builds `Client::new`, with no
  open-or-save lifecycle and no storage callback among the platform's six
  objects (`crates/ffi/src/device.rs` line 87, no key storage; CER-39 stays
  ignored). `Client::save` (`crates/client/src/ceremony.rs` line 865) writes
  the archive and the capture store and **not** `PayloadState` (line 353 makes
  it fresh on every construction): the identity DH key, the signed prekeys in
  force and retired, and the one-time pairs (`crates/client/src/payload.rs`
  line 71) are lost with the process, and with them the means to open
  ciphertext already queued at the serving node, while an archive-backup test
  passes. `Client::import` (line 911) opens and scans and installs nothing; a
  complete restore is owed. Owed: what survives restart, suspension, device
  restore and credential replacement, stated; the persistent lifecycle and
  the platform storage seam; and a test that restarts with an established
  payload session and queued messages, an interrupted restore, and the
  specified import scan. Which live secrets belong in a portable backup is an
  allocation to state, not assume.
- **The FFI client does not fail over and keeps no session cache.**
  `Net::attach` (`crates/ffi/src/net.rs` line 134) builds an empty sibling cache
  and TLS cache on every call, calls `attach_any` for the one node named, and
  never reaches `Session::failover` (`crates/transport/src/session.rs` line
  1396) or the cold-start fallback. Its events carry payload, not connection
  state; `Participant::attached` reports that a session object exists.
  `light-client-requirements.md` §4 requires persistent sibling information,
  fallback when the serving node is already down, and degraded-mode
  reporting; the routines exist and the running client does not use them.
  The resumption-ticket clamp of section 2.1 needs this same lifecycle to
  retain resumption state at all. Owed: failover driven inside the kernel,
  the caches retained, status values exposed, failure and restart tested
  through `Participant`, and a scheduling contract for maintenance and prekey
  refresh, since `serve_archive`'s reply (section 2.2) and every other outbox
  item waits on a `maintain` call the FFI exposes and nothing drives.
- **The FFI client always relays.** `start` installs `NoDirectPath`
  (`crates/ffi/src/client.rs` line 299) and `attach` installs `NoDirect`
  (`crates/ffi/src/net.rs` line 164); `LightDirect` exists and is not joined
  here. PRD-01's override in both directions cannot be built on this entry
  point. Owed: the direct-path adaptor bound to the facade with its horizon
  and override policy; successful direct use, forced relay, forced direct and
  fallback exercised through the boundary; section 2.4's reading settled
  first; delegated authentication on the direct path per section 2.1.
- **Catalog and resources have no transport behind the facade.** `browse`
  emits `Msg::CatalogQuery` (`crates/client/src/ceremony.rs` line 1093) and the
  courier (`crates/adaptors/src/courier.rs` line 174, `carry`) has no branch
  for it, returning it in `Carried::left`. The facade has no resource
  registration, access, or role view. Owed: a capability matrix from each
  intended screen or action to a kernel operation, a working adaptor, a
  result or event, and an acceptance case, covering catalog sweep, refresh
  and staleness, resource access and registration, roles, identity lifecycle
  and configurable policy. The facade fixes `Config::default()` and
  `HashEngine` (`crates/ffi/src/device.rs` line 106); the contract must say
  how retention and policy settings, a real biometric engine and identity
  initialisation enter the kernel, or shells will choose bypasses.
- **The FFI surface, as exports.** It exposes the ceremony, attach, payload
  and wake, and **no recovery, backup export or import, catalog sweep,
  departure, or standing and evaluation**; `rhtn-client` has 5, 6, 8, 1 and 1
  public functions for those respectively. Milestone 13's exit criterion
  names recovery. Add delegation issuance and import (section 2.1).
- **`uniffi`** was decided on 2026-09-16 and is absent from `crates/ffi/Cargo.toml`.
  The remaining half of milestone 13 is one generated binding compiling
  against the facade, and a real round trip through it rather than
  compilation alone.
- **`acceptance/tools/catalogue.py`** extended to `.kt` and `.swift`,
  milestone 14's first commit.
- **The administration channel.** `infra-client-requirements.md` §8.2 and
  `infra-client-requirements.md` §8.3: the node serves its own pages, the
  client ships only provisioning pages in a frame isolated from its keys,
  archive and captures, and *this document set does not specify the channel*.
  `app-requirements-notes.md` section 5 still holds *what state does it read,
  at what rate, and over which stream* open. Decide before `rhtn-daemon`
  grows a page server.
- **The two client modes** (before and after an instance) and their
  state-keyed notices (`app-requirements-notes.md` section 1).
- **Provider ordering** (`light-client-requirements.md` §6) needs the peering
  records' far endpoints and ASNs collected from the client's horizon and an
  association from provider or zone to ASN, with unknown concentration
  handled explicitly; an ASN alone does not establish provider or region
  independence.
- **The physical ceremony.** CER-37, CER-38 and CER-39 are deferred on
  hardware; `HashEngine` and `ByteEquality` stand in for the engine
  (design §22.2 leaves the profile open); `crates/mobile/` holds three
  READMEs. The API and build seams belong before the first shell; hardware
  acceptance closes during it.
- **The product entries** PRD-01 to PRD-09 close on a person reading a screen,
  and they predate 2026-09-16. The catalogue has no entry for the product
  obligations landed that day: *prompt for a backup until one exists* and the
  provider credential in the envelope (`light-client-requirements.md` §2),
  provider ordering by concentration (`light-client-requirements.md` §6), the
  run-length notice to the operator (`infra-client-requirements.md` §7), and
  the administration pages and their frame (`infra-client-requirements.md`
  §8.3). PRD-07 covers backup mechanics only. The envelope's contents, the
  delegation's timing and the frame's isolation take executable tests; the
  ordering, the prompt and the notice take product review as well.

---

## 8. Readings and decisions that are the author's

- **Signing authority under delegation.** Three statements, any two of
  which can hold: design §23.3 and `infra-client-requirements.md` §7 keep the
  seed off the instance; `wire-format.md` §8.2 has the delegated key sign
  nothing beyond the handshake; and `wire-format.md` §7.5 (an acknowledgement
  *issued by the grandpatron's node, not by its operator*), `wire-format.md`
  §7.1 with `infra-client-requirements.md` §3 (currency, unattended) and
  `wire-format.md` §10.2.4 (the automatic disavowal) have the node sign
  unattended. The code holds all three by reading the seed.

  **Ruled in part [author, 2026-09-21].** The rootward memo and the
  cycle-repair disavowal are signed by the infra node's temporary key; the
  adoption proceeds and is recognised in the local horizon immediately, the
  acknowledgement with it; currency needs no change. Read against the text:
  the memo carries no signature (`wire-format.md` §10.2's schema), so its
  authenticity is the session's and that row is closed; the acknowledgement
  and the attestation are classical-only `COSE_Sign1` and the temporary key
  can sign them as they stand, the acknowledgement's main verifier being the
  issuing node's own gateway; *no change* for currency is read as the same
  rule, the delegated key signing for the issuer. `wire-format.md` §8.2's
  *signs nothing beyond the handshake* and the three verification rules
  change to say so.

  **Ruled further [author, 2026-09-21], in three parts.** (i) Topology
  changes persist only as the updated topology, never as a time series;
  disavowal, departure and their reason data do not travel beyond the trust
  horizon, the rest of the tree seeing add and remove memos and updating its
  routing tables as locators reach it. (ii) **A cycle repair is not a
  disavowal.** The patron's own archive is within its horizon and carries the
  patron's own acts, so the rule is unbroken by it. (iii) **The delegation is
  a topology-class object**, flooded to the horizon on issuance and held as
  current state; the archive is for a node's own history alone, and a
  third party's transaction flooding the horizon never enters a
  non-involved member's archive. The store keeping txids as its seen-set
  with the fold as the state is fine.

  **What this settles.** The true key signs the archive: adoption
  countersignature, disavowal, departure, reissue, presence, a person's acts
  on the ceremony device, hybrid and durable. The temporary key signs or
  authenticates the topology: acknowledgement, currency, cycle repair,
  memos, and the delegation itself, each current state that replaces its
  predecessor and is nobody's history. `wire-format.md` §10.1 already floods
  transactions within the horizon and memos beyond, so the wire agrees on
  scope; design §10's list of archive-advancing transactions is unchanged
  once a cycle repair is not among them.

  **What the documents and code say today, to be changed.** `wire-format.md`
  §10.2.4 has the detecting node *disavow* the forwarding subordinate with
  reason code 5 under §4.3, and `crates/node/src/propagation.rs` line 716
  mints that type-3 envelope; both become a topology-state change. The wire
  gains the delegation's carriage in the topology class under §10.1's
  forwarding rule, replaced by `not_before` and held in the topology store;
  `wire-format.md` §8.2's *signs nothing beyond the handshake* and the
  verification rules of §7.5 and §7.1 say a key the issuer delegated
  suffices; `crates/node/src/store.rs` keeps txids and the fold.

  **Ruled [author, 2026-09-21], closing the signing question.** The staple
  carries the delegation. The vacancy memo suffices to tell a cut
  subordinate. `wire-format.md` §10.2.4 needs a wording change: the
  subordinate is *removed*; *disavow* is a synonym for that removal, and the
  term is overloaded with the §4.3 transaction, so §10.2.4 stops using it.

  **The document pass this makes**, one commit, step 0 of milestone A:
  `wire-format.md` §8.2 (the delegated key signs topology state, not only
  the handshake); §7.5 and §7.1 (a key the issuer delegated verifies;
  `CurrencyAttestation` or its introduction carries the issuer's delegation);
  §10.1 (the delegation as a topology-class object, forwarded under the
  rule, replaced by `not_before`, held in the topology store); §10.2.4
  (removal, the vacancy memo, no transaction and no reason code); §4.3's
  reason code 5 kept as a tombstone if it names the cycle case, numbers in
  registers never being reused; and the two stale §7.9 sentences of section
  2.2. Then the sweep by search: *automatic disavowal* at `wire-format.md`
  line 4185 and wherever design §15.2 and §6.2 describe the cycle case;
  PRP-12 (*disavow the subordinate*) and PRP-18 re-derived;
  `crates/node/src/propagation.rs` lines 153, 716 and 722 from a type-3
  envelope to a slot removal and its memo. The participant half, a
  delegated desktop or instrument signing `prekey` (`wire-format.md` §7.8),
  `catalog` (`wire-format.md` §6.1) or `locator` (`wire-format.md` §2.3),
  which `app-requirements-notes.md` line 188's count of three contexts
  missed, is answered by the flooded delegation, the verifiers of all three
  being horizon members; for prekeys it is also which device holds the
  private halves, which is the one residue of this item.
- ~~**The binding on a connection that never attaches.**~~ **Ruled
  [author, 2026-09-21]: the delegated peer presents its delegation first on
  every connection**, the attach's fields being that rule's instance on a
  session and control frame type 7 its instance where no session opens; the
  direct path, being inside the horizon, binds by the delegation already held
  from the topology class. Applied to `wire-format.md` §8.0, §8.2 and §9.1,
  `infra-client-requirements.md` §7 and `light-client-requirements.md` §4.1;
  RES-12, TRN-03 and TRN-04 requoted; TRN-18 to TRN-20 added (section 2.1).
- ~~**The delegation window.**~~ **Ruled [author, 2026-09-21]:** the decoder
  enforces exactly 48 hours; times against the Unix epoch; credentials in a
  run finish-to-start; the receiver's clock check carries a configurable
  leeway, default 10 seconds, relaxed for high-latency links; a connection
  refused on the window is retried. Applied to `wire-format.md` §8.2; TRN-21
  and TRN-22.
- ~~**Who mints the transport keypair.**~~ **Ruled [author, 2026-09-21]:** the
  instance mints its own and sends the public half to be signed, prior art
  being OpenSSH; one key serves the whole run, the instance being trusted for
  the cache. Applied to `wire-format.md` §8.2 and
  `infra-client-requirements.md` §7; DMN-23.
- ~~**Candidate-pair order**~~ **Ruled [author, 2026-09-22]: defer to the
  RFC.** design §14.1.1 now says the order is RFC 8445's and binds this
  implementation; TRV-02, TRV-07 and PAY-15 keep their outcome assertions and
  TRV-11 holds the order, which `connect_direct` does not yet keep.
- ~~**The open-decision register**~~ **Ruled [author, 2026-09-22], no
  objection to the reading offered:** O-007 and O-013 before the kernel-facing
  API, both being wire interoperability; O-015's restoration during the first
  shell; O-011, O-012, O-014 and O-016 before release, with O-012's light-patron
  delegation and down-line threshold and O-014's peer backup optional, as the
  design has them.
- ~~**`runner-rs`**~~ **Ruled [author, 2026-09-22]: retired and purged**, so
  that any dependency on it surfaces; none did. `wire-format.md` §13,
  `test-vectors/README.md`, the corpus test's header and the plan say so.
- ~~**Placement** of `functional_tests.md`~~ **Ruled [author, 2026-09-22]:**
  `Robot/` is scratch and context and no part of the deliverable;
  `functional_tests.md` is part of the deliverable design, lives in the root
  and is tracked like any core design document, having been constructed by the
  reviewer with `Robot/` withheld so that it summarises the design as written.
  `CLAUDE.md` names it as the seventh document and it is staged.
  **`conformance-review/`, ruled 2026-09-22 [author]:** tooling, not design;
  the harness is a workspace member at `crates/conformance/` so the gate runs
  the reviewer's tests on every change and a reproduced finding stays a
  regression; the reports at a commit are not in the tree. The rule that
  keeps it evidence, stated in the crate's manifest and `crates/README.md`:
  we repair what stops a reviewer's test compiling and never what it asserts.
- ~~**The first shell's slice**~~ **Ruled [author, 2026-09-22]:** payload
  first, then the ceremony, then infra provisioning, then the resource round
  trip, each because the next cannot exist without it; any part may be taken
  up as it comes up, the shared primitives written aware of the whole system
  rather than revised and left to break their callers. Recorded at
  `implementation-plan.md` milestone 14 and in section 9.
- Still open from `implementation-plan.md` section 7: the payload library and
  its licence, the post-quantum provider, the shell framework, `rhtn`'s
  argument parser, and the design §22.2 items.

---

## 9. Order: acceptance criteria first, in three milestones

**Why the oracles come first.** The corpus is the codec's test suite
(`implementation-plan.md` section 2.1) and `crates/crypto/tests/corpus.rs` is
the differential pair's Rust half, so a regenerated corpus is what turns
sections 2.1 and 2.2 into failing tests and the code change is whatever makes
them pass; written the other way, the generator is fitted to the codec and the
pair proves agreement with itself. The models are the cheapest verification
(`implementation-plan.md` section 5), and section 8.1 there has an acceptance
test for a modelled function only show that the implementation agrees with
the model. The catalogue already works this way: entries are counted before a
crate exists and code marks them. And the plan's two standing rules (section
4) say that a decision made during implementation hardens in code faster than
in prose.

**One rule survives, narrowed.** `implementation-plan.md` section 2 has a
specification change and the code it forces land in one commit so the tree is
coherent at every commit; and section 3 above has the vector pin advance once
for the whole delta. So the corpus is regenerated once, for both mechanisms,
and lands with the codec and crypto changes that make it green, one commit;
the behavioural code above the codec then lands per mechanism against its
catalogue entries. Models and catalogue entries are outside the code gate and
land on their own.

### Milestone A — before the kernel-facing API is committed

0. **The decisions the oracles depend on** (section 8): signing authority,
   the non-attach authentication boundary, the window, the keypair's minting,
   the candidate-pair reading, the register's dispositions, `runner-rs`, and
   the first shell's slice. Each is a document edit in the author's words;
   `light-client-requirements.md` §2 and `wire-format.md` §3 lose their two
   stale sentences (section 2.2) in the same pass. **Done 2026-09-21** but for
   the candidate-pair reading, the register's dispositions, `runner-rs` and
   the first shell's slice; the light client is a holder of delegations
   (`light-client-requirements.md` §4.2; TOP-43). **The prekey question was
   ruled 2026-09-22**: a delegated device generates its own payload keys and
   the ceremony device signs their public halves; a session is with a device;
   landed in design §23.3 and §14.2.4. **Its carriage was ruled the same day**:
   a device is named by the key it presents in a handshake, *necessary to
   allow multi-device on root nodes* [author, 2026-09-22]; applied to
   `wire-format.md` §7.8 (`device`, bundle field 5 under the signature, the
   reply an array of at most eight), §7.10 (the submission names the device;
   publication, deposit and wake belong to the session's device), §8.2 (an
   attach speaks for a device), design §14.1.6 (a queue per device),
   `light-client-requirements.md` §4.1 and `infra-client-requirements.md` §6.
   PAY-01, PAY-04, PAY-06, PAY-12, SUB-02 and QUE-05 re-derived and five of
   their tests unmarked; SUB-12, PAY-20, QUE-21 and SES-26 added. Two numbers
   in it are the assistant's pending the author: the ceiling of eight
   bundles per reply, and the register row for a fetch revealing a subject's
   device count, stated in §7.8 and not yet priced in design §19.
1. **Models** (section 4). The delegated bind, the non-attach modes covered or
   excluded by name, the mutant, the currency premise restated, `run-all.sh`
   rerun. A gap found here goes back to step 0.
2. **Catalogue** (sections 2.1, 2.3, 2.4, 2.5, and one entry per driver in
   section 6). Stubs regenerated, `check.py` at 0 flags; the implemented count
   falls, which is the owed work counted. **Done 2026-09-22**: 395 of 455.
   Beyond the entries the rulings wrote as they landed, twenty-two more: the
   delegation's remaining mechanics (TRN-23 to TRN-27, SES-27, DMN-24 to
   DMN-26, DEC-35, PRP-28), the no-history pairs for the newly named classes
   and the role table (RSC-41, RSC-42, SUB-13), one entry per caller-less
   driver in section 6 (TOP-44 acknowledgements, MET-12 evaluation, SUB-14
   wake delivery), the provider credential in the envelope (ARC-28), and the
   four product obligations of 09-16 as manual entries (PRD-10 to PRD-13).
   Not entered: the verifier's leg to a client behind its serving node, which
   the documents leave unwritten and so gives nothing to quote; it stays in
   section 6 as owed to the documents first.
3. **Vectors, once, landed with the codec** (section 3): the audit of the
   hand-authored fixtures across the whole delta; the generator extended for
   both mechanisms; `--accept-spec-change`; `verify.py` green; then
   `rhtn-codec` and `rhtn-crypto` until `corpus.rs` is green. One commit. Write
   the Rust from the specification and not from `generate.py`: the corpus
   test is evidence only while the two readings are independent.
4. **The gate** (section 6): the freshness check, so step 3 cannot go stale
   silently again; the format step; `cargo-deny`.
5. **Behavioural code per mechanism**, each against its entries: archive,
   node, client and `probe` for `wire-format.md` §7.9; transport, node,
   daemon, client and FFI for the delegation, on the signing table as decided;
   the design §19.6 notices removed; the no-history audit; section 2.6's
   confirmations as tests.
6. **The kernel a shell binds to** (section 7): the durable lifecycle and
   storage seam, failover and status inside the kernel, the maintenance
   contract, the direct path joined, the catalog branch in the courier, the
   capability matrix, the delegation-holding constructor, `uniffi` with one
   binding compiling and one round trip through it, the catalogue walk over
   `.kt` and `.swift`, and the administration-channel decision. Whichever of
   section 6's drivers the chosen slice needs: the verifier leg for a
   ceremony-first slice; the acknowledgement driver, evaluation and the
   resource paths for a resource-first one.
7. **The reviewer harness** (section 5): the one initializer, the status
   files, and a third conformance run at a commit where the pins are green,
   reaching the paths step 6 integrated.

**Exit for milestone A, through generated bindings**: a shell opens durable
state, attaches with the supported credential, observes its connection status,
survives a process restart with an established payload session and queued
messages, and performs its slice's one complete action — **payload between two
phones, each a device of its own** [author, 2026-09-22], the ceremony,
provisioning and the resource round trip following in that order during
milestone B. `implementation-plan.md`
lines 588 and 657–665 are corrected before the plan is handed over.

### Milestone B — during the first shell

PRD-01 to PRD-09 and the 2026-09-16 product obligations (section 7); CER-37,
CER-38 and CER-39 on real hardware; the biometric profile decided against real
engines (design §22.2); the second driver family from section 6 that milestone
A's slice did not need; external wake delivery, or its explicit deferral; the
remaining FFI exports.

### Milestone C — before a conforming release

PAY-13 and the payload library's licence; the fifteen and eighteen one-sided
conditions; the predicate evaluator and scheduled recomputation; the package
supply chain; the optional profiles from the register as dispositioned;
`review-plan.md` Stage 2's cryptographic audit and Stage 3's red team; the
split of the applications into their own repository on the plan's trigger.

---

## 10. Review of this note (2026-09-21)

An external review of the first draft returned twelve findings and eight
corrections to individual claims. Each was verified against the tree before
being applied, and every one held. Recorded here because the review file was
removed after incorporation.

| Finding | Substance | Where it landed |
|---|---|---|
| R01 | Removing the instance's seed needs a complete signing-authority decision; the node signs currency, subtree acknowledgements and cycle-repair disavowals unattended, and a delegated device signs prekeys, catalog entries and locators | Section 2.1's table; section 8's first item; step 0 |
| R02 | The bind is specified at the attach only; four production paths authenticate by TLS pin and never attach | Section 2.1; section 4; section 8 |
| R03 | A verifier behind its serving node is unreachable; the leg is unwritten in documents and code | Section 6 |
| R04 | Durable operation: no lifecycle or storage seam at the FFI, payload state never persisted, import installs nothing | Section 7 |
| R05 | The FFI client neither fails over nor keeps its caches; no connection-state events | Section 7 |
| R06 | The FFI client always relays; the transport dials all candidates at once against design §14.1.1's wording | Sections 2.4, 7, 8 |
| R07 | Subtree acknowledgement has no production issuer or taker; the gateway refuses grandchildren | Section 6 |
| R08 | Catalog and resource work needs a courier branch, a maintenance driver and a capability matrix, not exports | Section 7 |
| R09 | Wake registration exists; delivery does not | Section 6 |
| R10 | Two root sentences still state the old `wire-format.md` §7.9 rule; the open-decision register needs dispositions; `implementation-plan.md` is stale in two places | Sections 2.2, 5, 6, 8 |
| R11 | The gate has no freshness check, no format step and no `cargo-deny`; a whole-document pin cannot advance for half a delta | Sections 3, 6; step 4 |
| R12 | The order dropped section 6 and had no exit test; the work belongs in three milestones | Section 9 |

Corrections applied: O-009 is settled, not open; *ceremonies are the one thing
that needs the seed* was wrong (R01); the instance-cannot-mint rule is
`infra-client-requirements.md` §4.4, not `infra-client-requirements.md` §4.3; provider ordering needs
collection and an ASN association, not ASN exposure alone; new product
obligations take executable tests and product review, not only a manual
entry; `matrixcheck.py` runs from `crates/`; the six daemon tests share one
broken initializer; *fourteenth known-answer signature* means the fourteenth
domain, not a fourteenth standalone fixture.

Not adopted: nothing. The review's recommendation that shell scaffolding can
start now is taken as the meaning of the title.
