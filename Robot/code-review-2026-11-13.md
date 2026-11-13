# Code and test review, 2026-11-13

Asked for by the author after a fortnight of feature work in which several
rounds of adjustment did not produce the functional change expected
[author, 2026-11-13]. Four questions, answered in order, each from numbers
taken off the tree at `8dd1b56` and the bench record of the last week.
Assistant's document; nothing in the root cites it.

## 0. What was measured

| | Rust | Kotlin |
|---|---|---|
| Source lines | 90,000 (16 crates) | 22,929 (main) |
| Test lines | 58,000 | 8,763 |
| Tests | 1,031 (nextest) + 93 conformance | 380 |
| Largest files | `client/src/ceremony.rs` 7,618; `ffi/src/client.rs` 4,356; `node/src/federation.rs` 3,161; `codec/src/schema.rs` 3,135; `transport/src/session.rs` 2,989 | `Kernel.kt` 3,547; `Meet.kt` 1,647; `MeetActivity.kt` 1,353; `OperatorActivity.kt` 1,105; `ShellActivity.kt` 1,010 |
| Widest types | `Client` (ceremony.rs): 235 functions in three `impl` blocks | `Kernel` object: 157 functions |
| FFI surface | 158 exported methods; 97 called by the shell, ~10 by the CLI and harness, ~50 by nothing | — |
| Gate | fmt, clippy `-D warnings`, deny, 21 pins, 3 registers, ~20 min | 380 unit tests inside it |

Registers: `functional_tests.md` holds **570 live families**; `crates/acceptance/acceptance.json` holds **581 entries** (11 withdrawn), of which **566 of 570** carry an `// acceptance: ID` tag in a test; the untagged four are `MET-12`, `PRD-03`, `PRD-12`, `RSC-58` plus `CER-53`, `CER-55`, `PAY-13`, `SUB-14` by tag search (check.py counts 564 implemented).

## 1. Are the features and rules in automated tests?

**Inside a component, almost always. Across the seams, almost never — and every defect the bench found this week was at a seam.**

The week's bench findings, each with the test that would have caught it and did not exist:

| Found | Where | The missing test |
|---|---|---|
| Enrol by hand published `:7448` and attached to `Launch.QUIC_PORT` (7431) | shell ↔ ffi | that the address published and the address attached to are the same value |
| A member a record introduced could verify records but never attach (`Unbound`) | node ↔ transport pins | attach as an identity learned from a record; again after a restart (now `lifecycle.rs`) |
| A node restart, or a night's sleep, left every client dead until an app restart | shell ↔ ffi ↔ net | a session cut mid-run is attached again without input (now the backoff chain; still no automated test of it) |
| Every restart emptied every conversation | shell state | the shell's state survives a process restart (now `ThreadsTest`, which proves the codec, not the restart) |
| The invitation button did nothing on a second tap | shell | a refused attach can be tried again from the same screen |
| GXV-2's list named GXV's contact by GXV-2's kernel (bare keyhash) | shell model | a row held by one identity is named by that identity |
| The ceremony's subnet choice, the thread's sending identity, the contact list's stripe — all read the *focused* identity where the *holding* identity was meant | shell model | one notion of "whose" per row, tested once |
| Message as GXV-2 went "nowhere": the held session was not tried before a dial | adaptors courier | a reach that is a `Position` under an anchor whose node this client holds goes over that session (still open) |

The pattern: the Rust crates prove their own rules well (245 client tests, 208 node, 86 daemon, 73 transport; the hardware-free tab test in `adaptors/tests/direct.rs` is the right kind), and the shell's pure objects (`Meet`, `Identities`, `Serving`, `Threads`, `Colours`, `Chrome`, `Screen`) are tested as objects. **What has no automated home is the composition**: a shell driving an ffi `Participant` against an in-process daemon over the real transport. The pieces for it exist — `daemon/tests/delegated.rs` already attaches a transport client to a spawned daemon; `daemon/tests/scenarios.rs` runs multi-node scenes; `ffi/tests/boundary.rs` drives a `Participant` — but no test puts two `Participant`s and two daemons together, which is exactly the shape of T5–T7 and of every defect above.

Shell tests: 380, of which **12 pin source text** (`source("Kernel.kt").contains(...)`) rather than behaviour — those twelve are the step proofs for the browser model, bring-up, serving records, threads and colours, i.e. precisely the state transitions in `Kernel` that cannot be exercised on the JVM because `Kernel` is an Android-bound `object`. They prove the code is *present*, not that it *works*; the bench proved it works, defect by defect.

## 2. Are the tests aligned to the design documents?

**The acceptance catalogue is; the functional specification is not linked to anything.**

- Every acceptance entry cites document sections (`spec: ["design §14.1.1"]`), each rule quotes the sentence it tests, and `check.py` refuses a rule citing a section absent from its spec list. The 21 pins hold six documents and the vector corpus to the code. This half is in good order.
- `functional_tests.md`'s 570 families — "the functional requirements and test specification, a summation of the design as written" — are **cited by no acceptance entry and by no test mechanically**: 0 of 570 family ids appear in `acceptance.json`; 53 family ids appear in test doc comments informally (e.g. `SES-017` in `addresses.rs`). So the register the author treats as the test specification cannot say which of its rows has a test, and `countcheck`'s "0 flags" checks the counts, not the coverage. A row added this week (SES-019, the re-attach) has a `Test:` clause and no test; nothing reports that.
- The 12 `APP-` families belong to the applications repository and are uncited here by design; they should say so, or live there.

Recommendation: one link, mechanical — either acceptance entries carry the family ids they implement (`families: ["SES-017"]`) or tests tag family ids beside acceptance ids — and `check.py` reports families with no test. Until then "are all current features in tests" is answered by reading.

## 3. Are the codebases in good shape?

**Rust: sound in the small, two god objects in the large.** Kotlin: **one god object and a model that does not know whose a row is.**

### Rust

- `rhtn_client::ceremony::Client` is 235 functions across 7,618 lines: the ceremony state machine, contacts, payload routing, horizon, profiles, invitations, backup, resources, recovery, leaving, rotation and diagnostics all hang off one struct with one lock. The concern modules exist (`contacts.rs`, `horizon.rs`, `payload.rs`, `invitation.rs`…) but `Client` re-exposes each as methods, so every caller sees one 235-method surface and every change to any concern recompiles and re-locks the whole. Thirty-one `take_*` entry points are the arrival switchboard.
- `ffi/src/client.rs` mirrors it: 158 exported methods, ~50 with no caller in the shell, the CLI or the harness. Each `Refused` is a string (`Refused::new(format!(…))` ×166 across `client.rs` and `net.rs`): the shell matches on prose ("the serving node refused this client") where it needs to act on a kind.
- `ffi/src/net.rs` now carries two shapes for one idea: the original single session (`live`, `current`, `status`, failover `watch`) and the tabs map added this week beside it. One `BTreeMap<node, Attachment>` with the first as primary would halve the paths and remove the `primary: bool` threaded through `watch`.
- Smaller: 34 private `hex`/`id8` helpers across crates; `unwrap()` density highest in `node` (183) and `adaptors` (82), mostly `lock().unwrap()`; 69 undocumented public items, all in `model/src/lib.rs`, reported three times per gate as 207 "non-halting findings" — a standing number nobody reads any more; a doubled doc comment on `Courier::to_their_nodes` left by this week's edit.
- Good: clippy clean under `-D warnings`, `cargo deny` in the gate (it caught RUSTSEC-2026-0538 this morning), tests named as sentences with the rule quoted, the pins.

### Kotlin

- `Kernel` (3,547 lines, 157 functions) is the shell façade, the ceremony driver, the attach and re-attach logic, the identity index, the thread store, the field-test levers and the diagnostics. It is a Kotlin `object` holding Android `Context`, which is why its behaviour is proven by source pins.
- **The model has two notions of "whose"**: the focused identity (`Kernel.subnets()`, `Kernel.contacts()`, `Kernel.aliasOf()`, `Model.connection`) and the holding identity (`Kernel.subnetsHeld()`, `Identities.Owned.by`, `Front.Thread.by`, `Model.connectionOf`). Every multi-identity defect this week — the subnet choice, the sending identity, the bare keyhashes, the contact stripes — was a screen reading the first where the second was meant. The fix is structural: every row the model hands a screen carries `by`, and every lookup takes `by`; the focused-identity accessors become `held(by = active)`.
- Activities carry logic: `MeetActivity` has 84 branches and 26 `Kernel.` calls; `OperatorActivity` 42 and 19. The hand-enrol port defect lived in one of those branches.
- Duplication: eleven test files each define their own `module()` and six their own `source()`; `px()`/`dp()` helpers per Activity; `holderLabel` in one screen.
- Good: the small pure objects (`Meet` as a state machine with 67 behavioural tests, `Identities`, `Serving`, `Threads`, `Colours`, `Chrome`, `Screen`, `Json`) are the right pattern and are what the tests reach; all UI is built in code with one inset rule; no narrative strings since the plumbing pass.

## 4. Is non-private state modelled with retention and visibility rules?

**The rules are written; the model is not. The client keeps its state as one sealed blob rewritten whole on every change, and the shell's own state is plain files beside it.**

| State | Holder | Where / shape | Sealed | Retention rule | Scope |
|---|---|---|---|---|---|
| Archive records, store, payload (keys, sessions, ratchets, addresses), horizon, credential | kernel | one CBOR array of 7 parts, `files/kernel/<scope>/client`, **985 KB–2.6 MB rewritten after every arrival** (8 writes in 16 min on GXV) | yes, kernel key in the Keystore | none (§7.5.1) — correct | per identity |
| Sealed captures | kernel | inside the blob | yes | the only window (§7.5.1), enforced by the subject declining the key (`subject.rs`) | per identity |
| Sibling list | kernel | `siblings` | yes | until replaced | per identity |
| Serving record (tabs) | shell | `serving`, JSON | no | until the person changes it | per identity |
| Conversations | shell | `threads`, JSON, **plaintext** (new this week) | no | none | per identity |
| Identity index, seeds, custody key | shell | Keystore-wrapped | yes | — | device |
| Colours, provision, active identity | shell | plain / wrapped | mixed | — | device |
| Diagnostics | shell | `files/diag/*.jsonl`, 15 MB on GXV | no | newest 3 runs (`DiagFile.prune`) | device; field-test build only |
| Node store | daemon | one file per object under `tx/ own/ ep/ deleg/ ack/ seen/ derived/ proved/ conflicts/` | no | none; queue bounded by `queue-cap` (infra §2) | per node |

Findings:

- **The client blob**: positional CBOR arrays versioned by their length (the payload state went 7 → 10 fields this week; the decoder tries both), "whole or nothing" on read, written in full on every change. At 2.6 MB per write and ~30 writes an hour it is already the phone's busiest file, and a crash between the write and the rename of a 2.6 MB file is the one way a client loses everything at once. There is no schema document for it; `durable.rs` (76 lines) is the only description. The remedy is not a database: split by concern (archive, store, payload, horizon, captures) into files written when their own part changes, each with a version word, and say so in one place.
- **The shell's state is outside the sealed model.** Conversations are now kept in plaintext in the app's private directory while the kernel's state beside them is sealed under a Keystore key. The documents say nothing about the shell's own files; `light-client-requirements.md` §9 says the kernel seals what it persists and the shell keeps the key. Either the thread store goes through the kernel's seam (sealed with the rest) or §9 states what the shell may keep in the clear. This is an author's decision.
- **Rules exist where the design has them** — §7.5.1 (captures only), §13.7.1/§23.3 (device vs backup), infra §2 (queue) — and are implemented. What is missing is the inventory above *as a document*, so that a new holder (this week: `serving`, `threads`, `identity.colours`) has a rule to meet before it is written. Three of this week's holders were added by me with no rule stated.
- Visibility across identities on one device is enforced by scope directories (`files/kernel/<scope>/`), which is sound; the device-level files (`provision`, colours, the index) are the device's by design.

## 5. The debt, ranked

| # | Debt | Cost it has charged | Effort |
|---|---|---|---|
| 1 | No seam-level automated scenario (shell-shaped driver + ffi `Participant`s + in-process daemons) | every defect of the week found on the bench, each costing a gate and an install | 2–3 days to a first scenario (T5–T7's shape); the harness pieces exist |
| 2 | Two notions of "whose" in the shell model | four of the week's defects | 1–2 days: `by` on every row and lookup |
| 3 | `Client`/`Kernel` god objects | every change touches them; proofs by source pin | ongoing; start by moving the attach/re-attach and thread store out of `Kernel` |
| 4 | The client blob written whole | write amplification now; a total-loss failure mode | 2 days: split by concern with versions |
| 5 | Registers not linked (families ↔ tests) | "is it tested" is unanswerable mechanically | 1 day |
| 6 | String-typed `Refused` across the FFI | the shell matches prose | 1–2 days: a kind on `Refused` |
| 7 | `net.rs` carrying two attachment shapes | the re-attach and failover paths exist twice | 1 day |
| 8 | ~50 FFI exports with no caller; 69 undocumented `model` items; 34 hex helpers; doubled doc comment | noise in the gate's standing count; surface nobody owns | hours |
| 9 | Shell state outside the sealed model | a decision owed | author's call, then hours |

## 6. What I would do first

In this order, each gated: (1) the seam scenario, because it turns the bench's week into a test that runs in minutes; (2) `by` on every row; (3) the client blob split; (4) the registers linked. The god objects shrink as (1) and (2) pull the attach, thread and model code out of them; I would not start a decomposition for its own sake.

## 7. The veneer: rules in screens and sequences, cardinalities assumed, facts held only in process

[author, 2026-11-13]: *"a functional structure over a formal data model rather than an object structure with private state and assumptions around cardinality… a user can have many devices, many identities; an identity can exist in parallel on many devices and in many subnets; a contact and its DM history can exist in parallel for many identities; the light client and the node must be capable of independent reimplementation."* And the failure mode to root out: *"rules hard-coded into UI elements or specific process sequences and essential data never deserialized into a formal store, just annotated ephemerally in process state."*

Audited against that, file and line. Items marked † are this week's, mine.

### 7.1 Essential facts that live only in process state or in the shell

| Fact | Where it lives | What is wrong |
|---|---|---|
| **A conversation's history** (identity × party → messages) | `Front.convos` in `Front.kt:89`, written to `threads` by `Threads.kt` † | The kernel has no conversation store at all — `client/src/conversation.rs` is the *ceremony's* conversation on the wire. DM history, a fact the author lists as fractal (per identity, in parallel), exists only as a shell-side JSON file with no kernel model, no spec, and no way for another client to carry it. This week's persistence made it durable without making it modelled. |
| **Which node serves which address** (identity × subnet → node) | Twice: `PayloadState.serving_at` (sealed, `payload.rs:1359`) † and the shell's `serving` JSON (`Kernel.kt:87`, `Serving.kt`) † | Two stores of one fact, and the *shell's* is the one bring-up reads (`attachTabs`). The kernel's copy is written by the attach the shell drove. Neither is derived from the record that created the address. |
| **The identities a device holds** (identity × device) | the shell's `Identities.INDEX` | The kernel sees one identity per `Participant`; the relation "this device holds these identities" is the shell's file, so nothing in the kernel can reason over it (e.g. "do not offer my own identities as contacts" is a filter in a screen, `ConversationsActivity.kt`). |
| **"The peer"** | `Kernel.peerName`, `Kernel.peerKey()` from the provision; `ConversationActivity.kt:42` falls back to it | A singleton counterparty, from the bench's provision, used as the default where a screen was opened without saying whose thread it is. |
| **Connection state** | `Model.connection` (singular, `Model.kt:50`) beside `Model.connectionOf(by)` | The singular is what the foot and the catalog read. |
| **Aliases** | `Model.aliases`, `Kernel.aliasOf` — the focused participant's | Fixed this week by `aliasOfHeld(by, key)` †, i.e. by adding a second accessor rather than removing the first. |
| **The current meeting's choices** (which subnet, which side) | `Meet.kt` (`kind.adopt` at 120/1459, `scope` at 1501) | Held in the shell's state machine and handed to the kernel at proposal time. The ceremony's *choice* is a shell fact; a kernel restored mid-ceremony (`Restored`) does not have it. |

### 7.2 Rules that live in a screen or a sequence

| Rule | Where | Consequence seen |
|---|---|---|
| The address an enrolled instance is attached at | `OperatorActivity.kt:1009`: `"$ip:${Launch.QUIC_PORT}"` | T6: published `:7448`, attached `:7431` |
| Which subnet an adoption founds or joins | `MeetActivity.kt:746` offers `Kernel.subnets()` (the focused participant's) + `Scope.OWN`; `Meet.pendingChoice` decides | A screen computes a protocol choice from a focused-identity accessor |
| A member's admission at attach | pins seeded at start from the operator and the peers file (`service.rs`); fixed to learn from records this week | T7: `Unbound` |
| Re-attach after a loss | none until this week; now a backoff chain in `Kernel.reattachLater` † — in the shell, not the kernel | The kernel's `watch` judges the loss and stops; the shell dials again. The rule and its state (`reattaching`) are process state in the wrong layer. |
| Which subnet a reply goes back by (`last_from`) | recorded only for arrivals on a *tab* (`inbound_via`, `net.rs:672`); the serving node's own deliveries go through `inbound()` with no node (`net.rs:1047`, `:1628`) † | **The rule is implemented on one of two paths.** A reply to a message that came by the serving node is routed by the flood, never by "the last subnet it came by". Found by this audit; not yet seen on the bench because the direct path won every reply. |
| Profiles published at attach | `refreshed_profiles` publishes for `self.subnet()` — **the first position** (`ceremony.rs:4772`) | An identity in two subnets publishes its profile for one; MRK's bare-keyhash name for GXV-2 was partly this |
| What a new identity attaches to | `bringUp`: the provision for the focused identity, the serving record for the rest; `launchHeld` † | Correct by ruling, but a sequence: the same identity attaches differently depending on which thread brought it up |
| Contacts offered to write to | a filter in `ConversationsActivity.contacts()` † (`How.COUNTERPARTY`, not own identities) | A model rule expressed as a screen's list comprehension |

### 7.3 Cardinalities assumed singular

| Assumed | Where | The fractal fact |
|---|---|---|
| One serving node per client | `PayloadState.serving: Option<Keyhash>`; `Net.live: Option<Live>`, `current`, `status` (`net.rs:400`) | one per address — this week added `attachments`/`serving_at`/`tabs` *beside* the singletons rather than replacing them |
| One subnet per client | `Client::subnet()` (`ceremony.rs:4772`, 3 callers) | many |
| One focused participant as "the" participant | `Kernel.participant` (`Kernel.kt:66`) and every accessor over it | one in focus, many held |
| One peer, one meeting | `Kernel.peerName`, `Kernel.meet` | one meeting per camera is real; one peer is not |
| One identity per kernel | `Client` knows nothing of its siblings on the device | an identity in parallel on many devices needs the kernel to model *this device among its devices* (the bundle per device exists — `PayloadState.device`, `signed_bundle` — but "my other devices" does not) |

### 7.4 What the wire gets right and the local side does not

`wire-format.md` and the node are reimplementable from the documents: every object on the wire has a schema, a bound and a vector. The **local** side has none: the client blob is positional CBOR described by its decoder; the shell's files are described by their Kotlin; the FFI is described by `test-vectors/local-interfaces.md` as calls, not as a data model. So the second requirement — independent reimplementation of a light client — holds for the wire and fails for the state: a second client could speak to the node and could not carry this one's identities, addresses, contacts or conversations, because none of those is written down as data.

### 7.5 The shape to move to

A formal model, written down once, with its cardinalities stated and every rule a function over it. The entities the author's list implies, with the relations that carry the fractal:

```
Device(device_key)                          Identity(keyhash, material)
Holding(identity, device)                   -- many-to-many: an identity on many devices, a device holding many
Address(identity, subnet)                   -- an identity in many subnets; anchor, locator, series
Attachment(identity, device, node)          -- the session that address holds from this device; state, last seen
Node(keyhash, endpoints, operator)          -- a node is an identity's instance
Serving(identity, subnet) -> node           -- derived from Address and the records, never written by a screen
Party(identity, other)                      -- what one identity knows of another: how met, alias, number, profile
Thread(identity, other)                     -- one per identity, in parallel across identities
Message(thread, seq, mine, text, delivery)
Record(identity, txid, bytes)               -- the archive, by the identity that holds it
Capture(identity, other, ceremony)          -- sealed; the one window (§7.5.1)
LastFrom(identity, other) -> subnet         -- the routing fact, written at every arrival, on every path
```

Over which the rules are functions: `node_for(identity, other)` = `LastFrom` where held else every `Serving` of `other`'s addresses; `admits(node, identity)` = a record the node holds introduces it; `attach_all(identity, device)` = for each `Serving` of each `Address`; `offer_to_write(device)` = for each `Holding`, each `Party` met in person without a `Thread`. Each is a pure function with a test; each screen renders rows that carry `identity` and calls a function with it.

Where it lives: in the kernel, persisted per table (so a change to a thread does not rewrite the archive), with the shell as a projection that holds no fact of its own but the colours and the window geometry. The shell's `serving`, `threads` and the provision's "peer" go; `Front` and `Model` become views over kernel queries keyed by identity; `Net` becomes a map over `Attachment`.

### 7.6 Immediate corrections, independent of the restructure

1. `record_arrival` on the serving node's own path (`net.rs:1047`, `:1628`): the primary's collector tags its node too. Small; the rule is half-built without it.
2. `refreshed_profiles` over every address, not `self.subnet()`.
3. One store for the serving node: the kernel's `serving_at`, with the shell's `serving` file retired and bring-up reading the kernel.
4. `Kernel.peerKey()` as a fallback removed: a conversation screen opened without `by` and `peer` is a defect, not a default.
5. The re-attach moved into the kernel's `Net` (where the loss is judged), the shell told, not asked.

Each is a day or less and each removes one instance of the category; none is the restructure, which is the author's call on scope.
