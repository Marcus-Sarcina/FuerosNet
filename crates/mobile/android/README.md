# Android shell

Kotlin over `rhtn-ffi`. What is here is the skeleton: a Gradle project
whose one screen starts the kernel over the binding and shows what it
presents. `tools/build-native.sh` cross-compiles `librhtn_ffi.so` for
`arm64-v8a` and `x86_64` and generates the Kotlin binding from a host
build of the same crate, both into directories the tree does not carry;
`./gradlew assembleReleasableDebug` then builds the APK (the two flavours
are below). The binding is generated
without the `harness` feature, which is never in a shell's build, so the
load-time checksum probe wants exactly the symbols the packaged library
has. The seeds the skeleton mints on first launch stand in until the
ceremony exists; the application id `com.comptus.fueros` is provisional.

**What this shell owes**, beyond rendering the client's states:

- The proximity channels the device has, and no promotion of one that
  failed (`light-client-requirements.md` §1.3). Android exposes UWB and
  NFC to an application; a device lacking one reports it as unavailable
  rather than absent from the list.
- A guided camera capture whose frames cross the boundary as pixels, with
  whatever the platform's pipeline attached stripped at the boundary (§1.3).
- The privacy choices §5 requires the user be able to make, and the
  warnings §6 requires before anything irreversible.
- Encrypted backup under a key kept apart from the backup, and a plain
  statement of what losing the device costs (design §13.7.1).

The catalogue carries these as PRD-01 to PRD-05 and PRD-07 to PRD-09.

## The two flavours

The shell has two product flavours in one dimension, named as the Cargo
features of `rhtn-ffi` are (`Robot/field-test-diagnostics.md`, section 4;
`crates/README.md` on the gating):

| Flavour | Native library | What it adds |
|---|---|---|
| `releasable` | `tools/build-native.sh` into `app/src/releasable/jniLibs` | nothing: no log tree, no event file, no Report action, no crash handler |
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
`bearer.packets` and `bearer.refused`, `nfc`, `permission`, `storage`,
`custody`, `crash`, and `report.*`. Identities are eight hex characters
(`Diag.id8`), and free text that quotes a longer hex run is cut to eight
(`Diag.scrub`); no frame, key, seed, capture or storage payload has a path
into an event.

Crashes: `Crash` writes an uncaught exception's class, message and top
frames as a `crash` event, flushes the file, then hands the exception to
the handler that was there. The Rust side's panic hook in
`crates/ffi/src/diag.rs` writes a panic's message and location through the
same layer before the default hook runs.

The Report action, on the home screen and every Meet screen in this flavour
alone, zips the run's event files with a `header.json` (run id, commit,
spec pins, device, Android version, the radios present, flavour, app
version, the wall-clock anchor) under `filesDir/diag/report/` and offers
it with `ACTION_SEND` through `ReportProvider`, a one-directory read-only
content provider declared in the fieldtest manifest. The tester's checklist
and the run naming are M4's.
