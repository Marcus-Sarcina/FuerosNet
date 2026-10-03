# Field-test procedure: one run on the bench (2026-10-03)

How a run goes, for whoever runs it. Milestone M4 of
`Robot/field-test-diagnostics.md`. The script is `crates/tools/field-run.sh`
(`--help` prints its options and what a run directory holds); the tester's
form comes from `crates/tools/field-checklist.py`.

## The bench

One laptop and two phones on the same Wi-Fi network. The laptop runs the
serving node (`rhtnd`, field-test build), two instrument witnesses
(`rhtnp --log`) attached to it, one `adb logcat` per phone, and at the end
the merge. The phones run the `fieldtest` flavour of the shell and attach to
the laptop's node over the LAN; the ceremony itself runs between the phones
on their own channels.

The script takes the laptop's address from its default route and serves UDP
on port 7447; `--addr <ip[:port]>` overrides both. The access point must not
isolate its clients, and a laptop firewall must admit UDP 7447. Both phones
are on USB with debugging enabled, because the script provisions them,
follows their logcat and pulls their files over adb; `adb devices` must list
both as `device`.

## Build and install

The laptop's binaries are built by the script on first use (cargo, into
`crates/target/fieldtest`, so the releasable binaries elsewhere are
untouched); `--no-build` skips that step.

The shell, once per commit, from `crates/mobile/android`:

    tools/build-native.sh --fieldtest
    ./gradlew assembleFieldtestDebug
    adb -s <serial-A> install -r app/build/outputs/apk/fieldtest/debug/app-fieldtest-debug.apk
    adb -s <serial-B> install -r app/build/outputs/apk/fieldtest/debug/app-fieldtest-debug.apk

The debug build is what the pull needs: `run-as` reads a debuggable app's
private directory. A release-signed APK would leave only the share sheet.

## One run

1. On each phone, launch the app and open Conversations. While the device is
   unprovisioned that screen shows its public key material, about 2,400 hex
   characters. Read it over adb rather than by eye:

        adb -s <serial> exec-out uiautomator dump /dev/tty | grep -oE '[0-9a-f]{400,}' | head -1

   The material is public; nothing secret is on that screen.

2. Start the run from the repository root, naming the run and both phones:

        crates/tools/field-run.sh --phone <serial-A>=<material-A> --phone <serial-B>=<material-B> <label>

   The script mints identities for the node and the witnesses, lists both
   phones in the node's peers file (the node admits only identities it
   knows), starts the node and the witnesses, provisions each phone with the
   other as its peer (the app is stopped and started twice; the second start
   is the one that attaches), starts the logcat captures, writes
   `checklist.md` and `run.json`, and holds. The run is named
   `<commit>-<label>-<n>`; its directory is `runs/<run>/`.

3. Check that each phone's status line reads attached. Run the ceremony from
   Meet: one phone meets, the other scans the bootstrap QR, both read the
   brief and accept, then the captures and the exchange. Fill `checklist.md`
   as the screens go by; a row is *not reached* when the run ends before its
   step.

4. On both phones press *Send diagnostics (field test)*, on the home screen
   or a Meet screen. The zip is written before the share sheet opens;
   dismiss the sheet and the file stays on the phone.

5. Stop the script: Ctrl-C in its terminal, or `crates/tools/field-run.sh
   stop` from another. It quits the witnesses, stops the node, pulls every
   event file and report zip from each phone's private directory, merges,
   and prints the run directory.

A second run needs step 2 again (new node identity, new provision), and
after a wipe step 1 as well (new material).

## When a run fails

Read `summary.txt` first. *Refusals and aborts* lists every warning-level
event, every event named for a refusal or abort, and every field whose value
begins `Refused`, `Malformed`, `Failed`, `Unbound`, `Conflict` or `Abort`,
each with its source and time. *Ceremony steps* lists each side's `cer` and
`meet` events with the milliseconds between them, so a stall shows as a gap.
Then open `timeline.txt`, find the first abort or refusal, and read the step
before it: the second column names the source (node, witness or phone), the
fourth the layer (`cer`, `meet`, `shell`, `ffi`, `transport`, `node`,
`daemon`), which says where to look.

If a phone's pull failed, `phones/<serial>/logcat.txt` is the live mirror of
the same lines. The phone's file carries no anchor of its own; the script
builds one from the `shell.start` event, and a file without that event sits
at the head of the timeline by its own clock, which the summary says. The
kernel's lines on a phone count from `Participant.start` and sit early by
that offset, which the `shell.call` for `Participant.start` gives.

## Tester consent

The capture step photographs a face: each phone takes a few frames of the
other person through its selfie camera, under guidance on screen. Say this
before the first run, and get a yes; a tester may stop at any step.

What is stored and where: each phone keeps its capture of the other person,
sealed under a key only that person can derive, in the app's private
storage (`filesDir/kernel`; the shell's own keys are wrapped by the Android
Keystore). No image enters the record, the bundle, the node, the witnesses
or logcat: identities appear in events as eight hex characters, and no
frame, key, seed, capture or payload has a path into an event
(`Robot/field-test-diagnostics.md`, section 2; `client/tests/diag.rs` checks
the kernel's events for any run of its secrets). The bundle is `header.json`
and the event files. The run directory on the laptop holds the bundles, the
node's and witnesses' logs and state, and the bench identities minted for
that run; nothing from a camera.

So: the tester's face stays on the counterparty's phone until that phone is
wiped; it leaves neither phone; the bench keeps event logs that name the
tester's device by eight hex characters.

## The retention declaration

Every record declares a retention period at the ceremony's intent (design
§7.5.1: whole years of 365 days, two by default). The plan asks the
fieldtest flavour to declare one day. It does not today. `Participant.start`
(`crates/ffi/src/client.rs`) builds the kernel's `ceremony::Config::default()`,
whose `retention_years` is 2 (`crates/client/src/ceremony.rs`), and the
shell has no knob; the field is whole years on the wire, so one day is not
expressible without a decision from the author (the unit, a flavour-only
override, or neither). The brief's capture item (`Meet.kt`, UX-002) refers to
"the retention stated here" and the screen states no figure. Until both are
settled, a field-test record declares two years and the wipe below is what
bounds the captures on the bench.

## The wipe between testers

On both phones, since each holds a capture of the other tester:

    adb -s <serial> shell pm clear com.comptus.fueros

This deletes the app's private storage: the kernel's state under
`filesDir/kernel` (seeds, records, sealed captures, the provision), the event
files and report zips under `filesDir/diag`, the app's Keystore entries, and
the runtime permissions it was granted. The app stays installed and starts
next time as a new device with new seeds, so the next run begins at step 1.

What it leaves: the device's logcat buffer, which holds the `fueros.diag`
mirror until it rotates (`adb -s <serial> logcat -c` clears it); the laptop's
`runs/` directories, with bundles and logs; and the node's state under
`runs/<run>/daemon`, which may hold records the phones pushed, by identity
and never a capture. Delete the run directory when its triage is done.
