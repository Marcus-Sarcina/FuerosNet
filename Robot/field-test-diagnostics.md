# Field-test diagnostics: what exists, what to log, and the plan (2026-10-02)

Working document. The author asked, with test phones on the way, what test
setup should be in place so that running a ceremony leaves log and bug data
for rework, and for a developable plan. Three read-only surveys of the kernel
and FFI, the node and daemon, and the Android shell are the evidence; file and
line references are as of this date.

## 1. What exists

**Kernel, client, FFI.** No logging or tracing crate in either crate. The
in-process harness records every message it moves (`Harness::send`,
`ceremony.rs` ~2937) with sender, recipient and the whole `Msg`, which clones
secrets and carries no timestamp or outcome. The kernel speaks to the shell
through `Notice`/`Told` (seven user-facing notices plus `Unknown`), `Event`
(connection status, payload, response copy, late response, answered,
conversed), `Status` and `Progress`. Nothing is a diagnostic channel: every
one is something the person is meant to see or act on.

**Node, daemon, transport.** No logging crate. The daemon has twelve output
sites through two macros (`say!` to stderr, `tell!` to stdout) and three
operator views. **The one structured event log in the tree is
`transport::session::Log`/`Event`**, recording attach, bind, heartbeat,
failover, frames skipped or discarded, dials and closes; it is off by default,
documented as a test facility, and the daemon hard-codes it off
(`service.rs:683`). `Config` has no log field. The participant instrument
prints `end` after each command; the simulator keeps counters and a stderr
tap, not a timeline. A long list of outcome types is computed and never
surfaced anywhere: `Decision`, `MemoOutcome`, `AckTaken`, `LocatorOutcome`,
`Staple`, `AskStep`, the queue's and the sandbox's `Refusal`s, `RowError`.
Several drops are silent by construction: held-list eviction, the deferred-ack
cap, an anchor offer declined, mailbox store and take, relay fallback after a
failed direct dial, and a sandbox refusal collapsed to `STATUS_UNAVAILABLE`
before anything could record why.

**Android shell.** Fourteen `android.util.Log` calls under one tag, eleven at
warning level for camera, BLE and NFC failures; no file logging, no crash
handler, no `Application` subclass. No build types, flavours or `BuildConfig`
fields. Nothing counts bearer packets, bytes, duplicates or assembly time;
nothing times a QR decode; the camera sets no exposure or anti-banding. Two
sites over-log today: `Kernel.kt:579` prints the whole public material at info
level and `AndroidShell.kt:92` prints every `Told` whole, identities included.
What a tester can see on screen (`Meet.note`, `Front.note`, dialogs, step
titles, stop reasons) has no log at all.

**The specification does not apply to tooling** [author, 2026-10-02]. The
design documents describe the released product's behaviour, not its
development tooling; where logging or test telemetry would conflict with a
commitment (no queue-event logs, lock-not-log aggregates, seizure surfaces),
the specification is read as irrelevant for that purpose. What follows from
the ruling instead: an existing logging framework rather than a library of our
own, hooks generous throughout the codebase, every hook compiled out of a
releasable build, and the commit gate running on the releasable flavour.

## 2. Redaction, by construction

Three classes. **Never**: seeds (`Active.seed`, `OwnSeed`, backup `Contents`,
`Participant.seeds`), capture keys (`Zeroizing` everywhere they are held,
`Msg::CaptureKey`, `CaptureKeyHandover.key`, `KeyGrant.key`), templates and
fuzzed profiles (`Capture.template`, `VerificationQuery.profile`, `RawFrame`,
any camera frame), captures at rest or in flight (`SealedCapture`,
`ClientStore`), consent signatures and grants, ratchet and session keys
(`Ratchet`, `PayloadKeys`, `OneTimePair`, `Prefetched`, `InitialMessage`), the
provider credential, the storage key (`Custody`), `Storage::write` payloads,
and any `Debug` of `Msg`, `KeyGrant`, `Capture`, `SealedCapture`,
`ClientStore`, `Contents`. **Truncated**: keyhashes, txids, query ids and
ceremony ids to their first eight hex characters. **Free**: enum variants,
counts, sizes, durations, kinds, step names, reason strings the code already
produces.

The rule is enforced by type, not by review: the event type takes only
`Redacted` fields built by constructors (`id8`, `count`, `ms`, `variant`,
`reason`), and a test asserts that no event emitted by a whole harness
ceremony contains any byte run from a known secret.

## 3. The loggable events

Each row: event, fields, where it fires, what gap it closes. Durations are
milliseconds since process start; the bundle header carries one wall-clock
anchor.

### 3.1 The boundary (every FFI call and callback)

| Event | Fields | Fires at |
|---|---|---|
| `ffi.call` / `ffi.return` | method, ms, ok or `Refused` reason | every `Participant` export (`ffi/src/client.rs` ~611 onward, ~90 methods) |
| `platform.proximity` | channel, outcome, resolution_m, duration | `Proximity::run` |
| `platform.camera` | frames, bytes, duration; never pixels | `Camera::capture` |
| `platform.operator` | question id, answer, had host | `Operator::ask`, `Consent.ask` (shown, yes, no, cancel, no-host false) |
| `platform.notice` | `Told` variant only | `Notices::told` |
| `platform.storage` | name, bytes, ok | `Storage::read/write` |
| `platform.custody` | present or created; never the key | `Custody::key/keep` |

### 3.2 The ceremony, in the kernel

| Event | Fields | Fires at |
|---|---|---|
| `cer.begin` | counterparty8, initiator, started_at | `begin` |
| `cer.intent.sent` / `.received` | nominees, bundle entries, continuations, retention, initiator | `intent`, `take_intent`, `take_intent_carriage` |
| `cer.optical.shown` / `.read` | which QR (contribution or confirm), bytes | `optical_contribution`, `take_optical`, `transcript_confirm`, `take_transcript` |
| `cer.id_fixed` | cid8, ms since begin | `take_intent` via `anchored_id` |
| `cer.anchor.refused` | which message, `ContributionMismatch` / `CeremonyIdMismatch` / `ClockFar` | every anchored take |
| `cer.proximity` | channel, outcome, resolution, duration; then strongest | `take_channels`, `achieved` |
| `cer.capture_key` | sent or received, ok; never the key | `capture_key_carriage`, `take_capture_key_carriage` |
| `cer.capture` | start, end, image_count, template version, duration, `TemplateLength` | `capture`, `guided_capture` |
| `cer.select` | pool, required, selected, bases, `NoCandidateRecognised` | `select_verifiers` |
| `cer.query` | qid8, verifier8, basis, issued / consented / refused (`ProbingRefused`) | `query_for`, `consent`, `take_query` |
| `cer.grant` | qid8, sent or `GrantOutcome` | `take_grant` |
| `cer.response` | qid8, verifier8, `Verdict`, `Basis`; or `Closed`/`Rejected` reason | `take_response`, `take_response_copy` |
| `cer.witness.asked` / `.answered` | witness8, flags or declined, evicted (bound 8) | `witness_request`, `take_witness_request` |
| `cer.back_pointers` | signer8, count | `back_pointers`, kind 15 in |
| `cer.propose` | witnesses, responses, ms since begin | `propose`, `converse_propose`; `Waiting(what)` and `NoWitness` |
| `cer.review` | ok or `RootMismatch` / `BackPointers` / `Refused(variant)` | `review_and_sign`, `witness_sign` |
| `cer.signed` | signer8, or refusal code 1 to 4 | `Signed`, kind 17 in |
| `cer.finalize` | txid8, signers, ms since begin | `finalize` |
| `cer.abandon` | `Abort` variant | `abandon`, every `Err(Abort)`; the shell's Stop reaches it through `Participant.abandon` |
| `cer.retention` | from, to (years) | `set_retention_years` |

### 3.3 The conversation and the payload path

| Event | Fields | Fires at |
|---|---|---|
| `pay.send` | kind, to8, device8, direct or relay, bytes | `send_payload`, `route` |
| `pay.receive` | kind, from8, `Dispatched` variant, `Conversed` variant | `receive_payload`, `converse_in` |
| `pay.error` | `PayloadError` variant (`NoSession`, `NoBundle`, `NotTheSender`, `Replayed`, `Malformed`, `Crypto`) | `receive_payload` |
| `pay.session` | opened / displaced / kept (lower keyhash), peer8, device8 | `Sessions::open/receive` |
| `pay.ratchet` | skipped n, over `MAX_SKIP` | `Ratchet::skip/decrypt` |
| `pay.bundle` | fetched, prefetched, pool exhausted | `take_prekey_reply`, `on_pool_report`, `restock` |
| `pay.stranger` | kind, from8 | `Conversed::Refused` |

### 3.4 Sessions and transport, on both ends

Already shaped by `transport::session::Event`: `Attached{mode}`, `Refused`,
`Unbound{why}`, `Superseded`, `Bound{how}`, `DelegationPresented`,
`HandshakeDone`, `EarlyDataSent`, `Received{frame_type}`, `PeerUnreachable`,
`Failover{to}`, `Skipped`, `Discarded`, `OverBound`, `Dialled`, `DialDone`,
`DirectOpened`, `Closed`. Missing and to add: failover begun and no target
(`AttachOutcome` carries the none case), relay fallback after a failed direct
dial (`runtime.rs` ~945), candidate counts and reflexive-address found at
`gather`.

### 3.5 The daemon and the node

| Event | Fields | Fires at |
|---|---|---|
| `node.object` | kind, from8, `Decision` variant, reason | `take_object` |
| `node.hold` | kind, trust_reducing, evicted | `hold` (silent today) |
| `node.release` | released n, still held n | `release_pending` |
| `node.ack` | `AckTaken` variant, deferred cap hit | `take_ack_object`, `release_deferred_acks` |
| `node.memo` | `MemoOutcome` variant | `receive_memo` |
| `node.resolve` | `Step` variant, failure code, disposition | `resolve`, `answer_resolution` |
| `node.anchor` | offered, taken or dropped with why (threshold, signature, seqno) | `AnchorTable::offer` (silent today) |
| `node.locator` | `LocatorOutcome` variant | `LocatorStore::offer` |
| `node.currency` | issued / cannot issue, `Staple` status, `AskStep` | `issue_currency`, `take_staple`, `ask_currency` |
| `node.replicate` | `Replicated` variant, `Delivery` variant | `replicate`, `send_payload` |
| `node.mailbox` | stored, delivered, redelivered, `AtCap` refused, recipient8 | `enqueue`, `drain` |
| `node.wake` | `Registered` variant, rung | `wake.rs` |
| `node.prekeys` | pool exhausted for whom8 | `take_exhausted` |
| `node.resource` | status code **with the sandbox `Refusal` recovered** (`Exhausted` / `Trapped` / `Oversized` before the collapse), roles refreshed | `Gateway::serve`, `refresh` |
| `node.catalog` | which of the four refusal branches; report received | `register`, `take_report` |
| `daemon.lifecycle` | start, config read, identity loaded, replayed n, credentials remaining, persist, shutdown; every `Startup` variant | `main.rs`, `service.rs` |

### 3.6 The shell

| Event | Fields | Fires at |
|---|---|---|
| `meet.step` | from, to, trigger (tap or kernel), ms | `Meet.advance`, every transition |
| `meet.stop` | reason | `Meet.stop` |
| `meet.note` | the line | `Meet.note`, `Front.note` |
| `qr.shown` / `qr.read` | which, bytes, facing, decode attempts, decode ms | `MeetActivity` render, `QrCamera.readOne/decode` |
| `camera` | open, close, error, template, exposure and anti-banding as set | `QrCamera`, `FaceCamera` |
| `face.frames` | delivered, timeouts (5 s open, 2 s frame), sizes; never pixels | `FaceCamera.open/frame` |
| `ble` | advertise, scan, connect, MTU negotiated, discovery, disconnect, every refusal | `BleBearer` |
| `bearer.packets` | per phase: sent, received, duplicates, refused with reason, bytes, assembly ms | `Bearer.carry`, `Reassembly.take`, `carriage` |
| `nfc` | reader mode on/off, tap outcome (pass, fail, unavailable, refused 0x6985, unknown 0x6D00), APDUs, latch timeout, HCE served, cache cleared | `ProximityChannels`, `NfcCeremonyService`, `NfcApdu` |
| `permission` | which, requested, granted or denied | `MeetActivity` (CAMERA today; BT never requested, only caught) |
| `storage` | name, bytes, ok | `AndroidShell.write/read` |
| `crash` | exception class, message, top frames; Rust panic message and location | new handlers |

### 3.7 The bench and the person

The thirteen product rows (PRD-01 to PRD-13), TRV-12, TRV-13 and MET-12 are
`kind: manual` or need two devices; their `then` texts are the checklist a
tester fills per run. The bundle header carries: run id, git commit, both pin
sets, device model, Android version, radios present, build flavour, tester,
counterparty's run id.

## 4. Architecture

**The framework is `tracing`** (MIT), with `tracing-subscriber` for the
sinks, in every Rust crate; Timber (Apache-2.0) in the shell. Hooks are the
`tracing` macros and spans at every inventory point, generous by instruction.
A small `diag` module in the client holds only the redaction helpers (`id8`,
counts, durations); it is not a logging library.

**Compiled out of releasable builds.** The releasable default feature sets
`tracing`'s static maximum level to off, so every macro is dead code the
compiler removes; a `fieldtest` feature leaves them in and adds the
subscriber. The gate builds the default. The mechanism and its verification
(an added event name absent from the releasable library's strings) are
recorded in `crates/README.md` by M1.

**Kernel to shell.** A ninth `Platform` trait, `Diagnostics::event(line:
String)`: under `fieldtest` a subscriber layer renders each event as one JSON
line and forwards it on the kernel's threads, the contract `Notices` already
has. The shell appends to `filesDir/diag/<run>.jsonl` through a bounded queue
with a 16 MB cap and mirrors to logcat under `fueros.diag`. A Rust panic hook
in `rhtn-ffi` writes the panic through the same layer before the process dies;
`Thread.setDefaultUncaughtExceptionHandler` does the same for Kotlin. The
harness's `Harness::send` log is retired in favour of the same events.

**Daemon and instrument.** A `[log]` table in `Config` (path, level) installs
a `tracing-subscriber` writing JSON lines to a file and turns on
`transport::session::Log`; `say!` stays for the human lines. `rhtnp` gains
`--log <path>`.

**Build flavour.** `productFlavors { release; fieldtest }`: `fieldtest` sets
`BuildConfig.FIELD_TEST`, `GIT_COMMIT`, `SPEC_PINS`, enables the sink, adds a
Report action to the Meet screens and a retention declaration of one day;
`release` compiles the sink to `Silent` and has no Report action.

**The bundle.** `Participant::diagnostics_bundle()` returns the ring plus the
header; the shell writes it with the file and the checklist into one zip in
app-private storage and offers the share sheet (`ACTION_SEND`) or leaves it
for `adb pull`. Captures, keys and seeds are excluded by the type, not by a
filter.

**The bench.** `crates/tools/field-run.sh`: starts `rhtnd` with `[log]`, two
`rhtnp` witnesses with `--log`, `adb logcat -s fueros.diag` per attached
phone, names the run `<commit>-<n>`, and after the run pulls both phones'
bundles and the daemon's file into `runs/<run>/`. A new `rhtn diag` CLI
subcommand merges the JSON lines by time into one timeline and prints a
summary: steps with durations, every refusal, every abort, counts per layer.
Two emulators plus the laptop daemon run the courier-path steps before the
phones arrive.

## 5. Milestones

| # | Work | Exit criterion | Size |
|---|---|---|---|
| M1 | `tracing` with the releasable/fieldtest gating; kernel instrumentation of the ceremony, payload, verifier and horizon tables; the FFI boundary span; `Diagnostics` FFI trait, silent by default; the redaction test over a whole harness ceremony | A harness ceremony yields a complete timeline under `fieldtest`; no secret byte run appears in any event; the releasable library carries no event strings; the gate stays green | ~1 day |
| M2 | Daemon `[log]`, session `Log` enabled, the node table's events, lifecycle; `rhtnp --log`; the `rhtn diag` merge and summary | A simulator ceremony across two daemon processes merges into one timeline with every `Decision` visible | ~1 day |
| M3 | Shell: flavour and `BuildConfig`, the sink and file writer, the boundary events, the shell table's counters, both crash handlers, permissions, the Report action and bundle; remove the two over-logging sites | An emulator pair runs the courier path and each produces a bundle that the merge tool reads | ~2 days |
| M4 | `field-run.sh`, the checklist form from the product rows, run naming, tester consent and the retention declaration, the wipe procedure between testers | One dry run on emulators collected end to end into `runs/` | ~half a day |
| M5 | First phones: the local exchanges on hardware, bundles from both, triage into the tracking file | Each failure in a run names the step, the refusal and the layer without a second run | the phones' arrival |

Sizes are estimates by file count, unmeasured. M2 and M3 are independent
after M1; M4 waits on M2 and M3.

## 6. Documents and catalogue

The diagnostics build is tooling, so the specification and the functional
tests gain nothing for it [author, 2026-10-02]. Catalogue: one TOOL row for
the merge tool's timeline and one APP row for the flavour split, since those
are engineering properties the gate can check; nothing for the product
documents.

## 7. Risks and costs

Identities truncated to eight hex characters remain linkable within one
bundle; acceptable for a test build, and the hooks compile out of a
releasable one. The FFI callback runs on kernel threads, as `Notices`
does; the shell's writer must be a bounded queue so a slow disk never blocks a
ceremony. The ring and the file are app-private and wiped with the app; the
bench keeps bundles on the laptop, not on the phones. Nothing here changes
what the protocol does, so the gate's spec pins are untouched by M1 to M4.

**Done 2026-10-02**: the two over-logging shell lines (the public material
at info level; whole notices with identities) are removed. M1 is under way.

**M1 done 2026-10-03.** `tracing` in client, FFI, archive and node: 83 hook
sites, 62 event names, every exported FFI method in an `ffi.call` span with an
`ffi.return` event, platform callbacks logged. Gating: `rhtn-client` and
`rhtn-ffi` default to `releasable`, which sets `tracing`'s and `log`'s static
maximum level to off (the workspace's quinn enables `tracing/log`, which
otherwise keeps a runtime branch); `fieldtest` leaves the hooks in and adds the
JSON-lines layer feeding the ninth platform trait `Diagnostics`. Verified by
`strings` on the releasable release library: zero event names or callsites,
against a positive control under `fieldtest`. The redaction test runs two
ceremonies, an adoption, a payload session and a recovery under a collecting
subscriber and asserts no eight-byte window of any seed, capture key, consent,
profile or session key appears, in hex or as a byte list: 218 events, 0 hits.
The gate passes on the releasable default. Next: M2 (daemon `[log]`, node
events, the merge tool) and M3 (the shell's writer, flavour, counters, crash
handlers, Report action), independent of each other.

**M2 and M3 done 2026-10-03**, run as two parallel tasks on disjoint files.
M2: `[log]` table (`path`, `level`) in the daemon's config, accepted and
ignored with one line under `releasable`, installing the JSON-lines
subscriber under `fieldtest`; hooks node 1 → 35, transport 0 → 6, daemon 0 →
16; the transport's existing session log raises a tracing event on every
push, so it is not switched to recording (which would hold every event in
memory for the session's life). `rhtnp --log`. `rhtn diag merge` merges files
by a `diag.anchor` event carrying `unix_ms`, prints the timeline, the
ceremony steps with inter-step ms, every refusal and abort, and counts per
layer and event; a fieldtest daemon test runs two `rhtnd` processes and
merges them. M3: Gradle flavours `releasable` and `fieldtest` (one
dimension; AGP forbids a flavour named `release`), each with its own native
library under `app/src/<flavour>/jniLibs/`, `build-native.sh --fieldtest`;
`BuildConfig.FIELD_TEST`, `GIT_COMMIT`, `SPEC_PINS`; Timber planted only in
`fieldtest`; the sink is a 4,096-entry queue drained by one writer thread to
`filesDir/diag/<run>.jsonl`, 16 MB rotation, three runs kept, drops counted;
72 shell event sites across every file in the inventory; both crash
handlers; the Report zip with `header.json` and `ACTION_SEND` through an
in-house provider; Bluetooth permissions now requested rather than caught.
Android suite 62 → 85. Found in passing and fixed after: the BLE client never
subscribed to the server's notifications, so server-to-client packets would
not have arrived on hardware; and the provisioning note still pointed at the
removed material log line. Costs: M2 48 min and about 275k tokens, M3 34 min
and about 240k, against M1's 105 min and 480k, as predicted. Next: M4, the
bench script, the checklist from the product rows and the run procedure.


**M4 done 2026-10-03.** `crates/tools/field-run.sh [--addr ip[:port]]
[--phone serial=material]... <label>` names the run `<commit>-<label>-<n>`,
builds the field-test `rhtnd`, `rhtnp` and `rhtn` into
`crates/target/fieldtest` (the releasable binaries elsewhere untouched),
mints a node identity and two witness identities, writes the peers files so
the node admits the phones named, starts the node on the laptop's routed
address (port 7447), attaches the two witnesses, provisions each named phone
over adb (the other phone as peer; the first witness when alone), follows
`adb logcat -s fueros.diag` per device, and on Ctrl-C, SIGTERM or
`field-run.sh stop` pulls every event file and report zip through `run-as`,
merges, and writes `timeline.txt`, `summary.txt`, `checklist.md` and
`run.json` into `runs/<run>/` (ignored). A phone's file carries no
`diag.anchor` of its own, so the merge copy under `merge/` gains one built
from `shell.start`'s `wall_ms`. `crates/tools/field-checklist.py` writes the
tester's form from the catalogue's twelve `kind: manual` rows (PRD-02 to
PRD-13; PRD-01 is a tombstone) plus TRV-12, TRV-13 and MET-12: 15 rows, each
with id, title, the `when`, the `then`, three boxes and a notes line; its
eight-case test is gate step 1c. `Robot/field-test-procedure.md` (1,197
words) carries the bench, the build and install commands, the run sequence,
the failure reading order, consent, the retention declaration and the wipe.
Dry runs: one without phones (3 sources, 420 events) and three on the
`fueros` AVD (x86_64, Android 16), the last of which installed the fieldtest
APK, read the material off the Conversations screen, provisioned, attached
(`attached · presents ...` on the status line), pressed Report, and pulled
three event files and the zip (header.json and one event file); 6 sources,
947 events. Found on the way: the retention the plan asks the flavour to
declare (one day) has no knob, `Participant.start` takes
`ceremony::Config::default()` with `retention_years = 2`, the unit is whole
years, and the brief's capture item refers to a retention "stated here" that
the screen does not state; at `[log] level = "debug"` quinn's own targets
are 329 of the daemon's 420 events in the first dry run, and its
`NewConnectionId` lines carry QUIC reset tokens; the emulator's quinn
reports `sendmsg` I/O errors (GSO) once per attach and still attaches; the
AVD has no front camera, so the capture step cannot run there. Next: M5.

**M4b done 2026-10-03.** A live stream beside the file, fieldtest only.
Shell: `DiagStream.kt` (no Android in it), a TCP client to the bench with
a 4,096-line backlog that drops its oldest and sends the count as
`diag.dropped` (`sink: stream`) where the gap is, reconnecting with a
backoff of 1 s to 30 s and saying hello again; the hello is a
`diag.hello` event carrying `Report.headerFields` (the `header.json`
fields, now one list for both), the adb serial the bench gave the phone,
and `unix_ms`, so the collector's file anchors itself. The bench gone is
noticed by a reader thread seeing EOF, not by the next write, which a
closed socket may still accept. The target arrives as two extras on the
provisioning `am start` (`diag_stream host:port`, `diag_serial`), is kept
in `filesDir/diag/stream.target` and read at the next cold start before
the first event; `FuerosApp` alone constructs the class, and the
releasable flavour reads neither extra nor file. Laptop: `rhtn diag
collect --listen <addr> --into <dir>` (std threads, one per connection,
no feature) appends each connection to `<dir>/<serial>.jsonl`, the hello
kept once per run and written again for a new run from the same phone,
one stdout line per connect and disconnect; `rhtn diag watch <dir>`
follows every `.jsonl` under a directory except `merge/` and `report/`,
prints each ceremony step, refusal and abort as its newline lands, placed
by its file's anchor (`diag.anchor` or `diag.hello`, which
`diag::parse_line` now treats alike) and shown with source and ms. The
parsing moved into `parse_line`, used by `parse`, the collector and the
watch. `field-run.sh --stream-port <port>` (7448; 0 for none) starts the
collector into `runs/<run>/phones/` after the witnesses, passes the target
in the provisioning step, stops the collector before the pull, and leaves
the streamed files out of the merge: the pull is the record. Tests: four
JVM tests over a loopback server (hello first and order; drop-oldest with
the count sent in place; reconnect with a line kept across the gap; the
target parsed, kept and forgotten), Android suite 91 to 95; three Rust
tests (two concurrent connections with one reconnect and a second run;
a connection without a hello filed by its address; a watch catching a
step appended with its newline, skipping `merge/`, and the loop form), CLI
suite 9 to 12. Verified: both flavours' debug APKs build, fmt and clippy
clean, and a loopback run of the collector with two fake phones and a
reconnect, a watch over the directory, a hand-appended refusal in a
daemon file, and a merge of the resulting files. Found on the way: the
first reconnect test hung the whole Gradle run because the accepted
sockets had no read timeout, and the stream raced the test's poll of its
own connected flag; a `Timeout` rule and socket timeouts make such a
failure visible in 30 s. Not done: `Build.getSerial()` needs a privileged
permission on API 29 and later, so the serial is the bench's, not the
phone's; a phone provisioned without `--phone` gets no stream.
