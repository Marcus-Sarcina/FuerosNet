# Android shell

Kotlin over `rhtn-ffi`. `tools/build-native.sh` cross-compiles
`librhtn_ffi.so` for `arm64-v8a` and `x86_64` and generates the Kotlin
binding from a host build of the same crate, both into directories the tree
does not carry; `./gradlew assembleReleasableDebug` then builds the APK (the
two flavours are below). The binding is generated without the `harness`
feature, which is never in a shell's build, so the load-time checksum probe
wants exactly the symbols the packaged library has. The application id
`com.comptus.fueros` is provisional.

**What is built**, screen by screen, against
`Robot/light-client-screens.md`:

| Screen | What it does |
|---|---|
| The shell | The launcher since 2026-10-29: the rail of orbs down the left (direct messages, the meeting, each joined subnet in its crown), the foot with this person's name, and the content between them. Brings the kernel up and takes the bench's provision. Each place carries the rows that reach the screens below and names what it does not hold yet |
| Attach | This device's own attach to a serving node, from Options: the nodes whose records it holds, or a key and an address typed, and a detach |
| Meet | The ceremony end to end: the invitation, both briefs, the optical exchange, the window, the guided capture, verifiers and witnesses, the review, the record, and the peering and adoption that follow |
| Conversations | From the rail's DM orb: a thread per party met, each sending to its own party, with an honest *sending* / *sent* / *unsent* and no *delivered* |
| Catalog | From inside a subnet: the sweep, what it holds with truncation and staleness said, and one request to a resource through the node that served its entry |
| Authority | From a subnet's options: the lines this key holds, what leaving one costs, and the departure behind two taps |
| Operator | From Options: an instance enrolled by hand or launched at a provider, its run renewed, its pages framed, the machine destroyed |

**Removed 2026-10-29** [author]: the old Home, a list of destinations; the Settings placeholder; and People, whose scanner and own-code button belong to the meeting screen and whose contacts belong to the Direct view, neither built yet — so taking a node's or a person's code is owed until the meeting screen's scanner reads whatever is put in front of it.

**What this shell still owes**, beyond rendering the client's states:

- The identity fork at first run: a new identity is minted and kept under
  this shell's own custody, and **restore and recover are not built** — the
  kernel exports neither recovery nor departure yet.
- Settings, which is a placeholder: the wake endpoint, the verifier limits
  and the seal cost.
- Encrypted backup under a key kept apart from the backup, and a plain
  statement of what losing the device costs (design §13.7.1).
- The privacy choices §5 requires the user be able to make, and those of
  §6's warnings that are not already in the ceremony's brief.
- The role view and registering a resource, neither of which the kernel
  exports.

The catalogue carries the obligations as PRD-02 to PRD-14, PRD-17 and
PRD-19, and marks none of them: a manual row closes on an observation, and
how one is recorded is open since the tester's form was removed.

## The two flavours

The shell has two product flavours in one dimension, named as the Cargo
features of `rhtn-ffi` are (`Robot/field-test-diagnostics.md`, section 4;
`crates/README.md` on the gating):

| Flavour | Native library | What it adds |
|---|---|---|
| `releasable` | `tools/build-native.sh` into `app/src/releasable/jniLibs` | nothing: no log tree, no event file, no live stream, no crash handler |
| `fieldtest` | `tools/build-native.sh --fieldtest` into `app/src/fieldtest/jniLibs` | the diagnostics below |

Build and install:

    tools/build-native.sh && ./gradlew assembleReleasableDebug
    tools/build-native.sh --fieldtest && ./gradlew assembleFieldtestDebug

The unit tests are one set for both flavours and the gate runs the
releasable variant, `testReleasableDebugUnitTest`
(`crates/tools/android-unit-tests.sh`). The flavour is not named `release`
because a flavour may not share a build type's name. `BuildConfig` carries
`FIELD_TEST`, `GIT_COMMIT` (from `git rev-parse --short HEAD` at build
time) and `SPEC_PINS` (the six pins of `crates/spec-pins.json`, twelve hex
characters each, as `name=prefix` pairs).

**What the fieldtest flavour does.** `FuerosApp` plants a Timber
`DebugTree` and points `Diag` at `filesDir/diag/<run>.jsonl`, where `<run>`
is the process start in seconds plus six random hex characters. The
kernel's lines (`Diagnostics.event`, one JSON object each, rendered by
`rhtn-client`'s field-test layer) and the shell's own events (`Diag.kt`,
layer `shell`) land in that one file through a bounded queue drained by one
writer thread, so the kernel's thread never waits on the disk; a full queue
drops and the drop count is written as `diag.dropped`. At 16 MB the file
rotates to `<run>.prev.jsonl`, replacing any earlier one, and the newest
three runs' files are kept. Every line is mirrored to logcat under
`fueros.diag`, so `adb logcat -s fueros.diag` follows a run live.

The `ms` field of a shell line counts from `FuerosApp.onCreate`; the
kernel's counts from `Participant.start`, and the shell's `shell.call` for
`Participant.start` gives the offset between the two.

The shell's events are the rows of the plan's section 3.6: `shell.call` and
`shell.return` around every kernel call in `Kernel.kt`, `shell.event` for
what the event loop takes, `meet.step`, `meet.stop` and `meet.note`,
`qr.shown` and `qr.read`, `camera`, `face.frames` and `face.timeout`, `ble`,
`bearer.packets` and `bearer.refused`, `permission`, `storage`,
`custody` and `crash`. Identities are eight hex characters
(`Diag.id8`), and free text that quotes a longer hex run is cut to eight
(`Diag.scrub`); no frame, key, seed, capture or storage payload has a path
into an event.

Crashes: `Crash` writes an uncaught exception's class, message and top
frames as a `crash` event, flushes the file, then hands the exception to
the handler that was there. The Rust side's panic hook in
`crates/ffi/src/diag.rs` writes a panic's message and location through the
same layer before the default hook runs.

**The run's header** (`Report.kt`): run id, commit, spec pins, device,
Android version, the radios present, flavour, app version and the
wall-clock anchor the `ms` fields count from. Every live stream opens with
it. **The bundle and its share sheet are gone** [author, 2026-10-07]: they
zipped the event files for `ACTION_SEND`, which on a phone with nothing
that accepts a zip is a save dialogue with nowhere to send it, and
`crates/tools/field-run.sh` already streams every event live and pulls
every file at stop. The run naming is M4's.

**The live stream** (`DiagStream.kt`, fieldtest only). Beside the file,
the same lines go to the bench over one plaintext TCP connection on the
tester's own LAN, where `rhtn diag collect` appends them to
`phones/<serial>.jsonl` and `rhtn diag watch` prints each step, refusal
and abort as it arrives. The first line of every connection is a
`diag.hello` event: the header's fields, the serial the bench gave the
phone, and `unix_ms`, so the collector's file carries its own anchor. The
backlog is 4,096 lines; when full the oldest is dropped and the count is
sent as `diag.dropped` with `sink: stream` where the gap is; the sink
reconnects with a growing backoff (1 s to 30 s) and says hello again. The
kernel's thread never waits on the socket. The stream is lossy by design
and the file pulled at the end of the run is the record.

The target arrives through the provisioning step
(`crates/tools/field-run.sh`), as two more extras on the same `am start`
that carries `provision`: `--es diag_stream <host:port>` and `--es
diag_serial <adb serial>`. `ShellActivity` hands them to
`FuerosApp.streamTo`, which keeps them in `filesDir/diag/stream.target`
(two lines: address, serial) and switches the stream at once; every later
cold start reads that file in `FuerosApp.onCreate` before the first event.
`diag_stream off` (or blank) deletes the file. In the releasable flavour
the extras are never read, nothing constructs a `DiagStream`, and no
target file is read or written; `pm clear` removes a kept target with the
rest of the app's storage.
