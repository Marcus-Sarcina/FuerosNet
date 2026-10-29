# Field-test procedure: one run on the bench (2026-10-03)

How a run goes, for whoever runs it. Milestone M4 of
`Robot/field-test-diagnostics.md`. The script is `crates/tools/field-run.sh`
(`--help` prints its options and what a run directory holds). **There is no
tester's form** [author, 2026-10-23]: one was generated into every run
directory for thirty-five runs and filled in not once, and what a person
observed has always been written up per run in
`Robot/field-test-sequence.md` instead. That is where it goes.

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
are reachable by adb, on USB or over Wi-Fi at the address `phones.conf`
records (see *The phones*), because the script provisions them, follows
their logcat and pulls their files over adb; `adb devices` must list both
as `device` once connected.

## The phones

What a test phone needs, and what it does not:

- **Android 12 or later** (the app's `minSdk` is 31).
- **A camera**, for the optical anchor and the capture. Not demanded at
  install, but a ceremony cannot begin without one.
- **Bluetooth LE**, which is this shell's bearer for the intent, proximity,
  capture-key and prekey carriages, and the conversation's. The protocol names no bearer
  (`wire-format.md` §14.3.1 leaves it to the shell), so this is the shell's
  practical requirement and not the design's: a phone without BLE installs
  and attaches, and the Meet flow stops at the bearer with the reason
  named.
- **NFC is optional.** The design ranks it as one proximity channel among
  several (design §7.6.3) and the light client obtains the strongest the
  hardware has (`light-client-requirements.md` §1.3). A phone without NFC
  runs the ceremony on the optical pass alone; the proximity screen shows
  NFC as unavailable, the record carries the channel that was achieved, and
  the pre-sign warning that the channel was weaker than UWB is a true
  statement and not a fault. The manifest declares the camera, BLE and NFC
  with `required="false"` for this reason. **A pair with and without NFC is
  a useful first pair**: the unavailable path has not run on hardware.
- **UWB is not driven** by this shell whether or not the phone has it.
- **USB debugging** on, with the laptop authorised: `adb devices` must list
  the phone as `device`, not `unauthorized`. The scripts provision, follow
  and pull over adb. **The cable is needed only for setup**: `field-setup.sh`
  switches the phone's adb to TCP as well (`adb tcpip 5555`) and records its
  Wi-Fi address in `phones.conf`, and `field-run.sh` reaches a phone that
  is not on USB over Wi-Fi at that address, naming it by serial as before.
  A reboot undoes the TCP switch; `field-setup.sh --address-only` redoes
  it and refreshes the address without a wipe. On Samsung, accept the USB
  debugging prompt with *Always allow from this computer*, or every replug
  asks again.
- **Runtime permissions** for the camera, Bluetooth and NFC are asked for
  by the Meet screen when it first needs them; grant them on the phone.
  Nothing is granted over adb.

Nothing else is installed on a phone. The node, the witnesses and the
collector are the laptop's.

## Build and install

The laptop's binaries are built by the script on first use (cargo, into
`crates/target/fieldtest`, so the releasable binaries elsewhere are
untouched); `--no-build` skips that step.

The phones, once per commit or per wipe, from the repository root:

    crates/tools/field-setup.sh

builds everything field-test (the laptop's binaries, the native library and
binding, the fieldtest debug APK), uninstalls the app from each phone named
in `crates/tools/phones.conf`, which wipes its data, installs the fresh APK
and launches it, asks the tester to tap **Start a new identity** on each
phone's first-run screen, reads the public key material off that screen
through uiautomator every 5 s until it is there, and writes
`<serial>=<material>` back into `phones.conf`. The tester then writes the
phrase down (or not, on a bench phone), taps through, and types the phone's
default name; the material is already read by then.
`field-run.sh` reads that file when given no `--phone`, so step 1 and the
`--phone` arguments of step 2 below are done. `--no-build` installs what is
already built; serials on the command line override the file. The two
Galaxy S21+ testers are in the file [author, 2026-10-03].

By hand, the same from `crates/mobile/android`:

    tools/build-native.sh --fieldtest
    ./gradlew assembleFieldtestDebug
    adb -s <serial-A> install -r app/build/outputs/apk/fieldtest/debug/app-fieldtest-debug.apk
    adb -s <serial-B> install -r app/build/outputs/apk/fieldtest/debug/app-fieldtest-debug.apk

The debug build is what the pull needs: `run-as` reads a debuggable app's
private directory. A release-signed APK would leave only the share sheet.

**Prefer the by-hand `install -r` once the B series has begun** [author,
2026-10-22]. `field-setup.sh`'s uninstall is what wipes the data, and that
data is what B is made of: the adoptions, the met pairs and the sealed
captures a verifier needs from an earlier meeting. `install -r` replaces
the code and keeps all of it, and the identity survives with it — so the
material already in `phones.conf` stays valid and there is nothing to
re-read: the material shows once, on the first run, and never again on
demand (`light-client-requirements.md` §9). Use `field-setup.sh` when a run
genuinely wants phones at genesis, and `--address-only` when all that
changed is an address after a reboot.

What makes this safe to say rather than hope: nothing persisted changed
shape across the witness reorder. `durable.rs`, `store.rs`, `sealed.rs`
and `catalog.rs` were untouched by it, and the one edit to `device.rs` was
to comments. What *did* change is `local.rs`'s IntentExchange, from nine
fields to eight — which is not stored anywhere but **does** mean every
phone in a run must carry the same APK. A pair on two builds fails at the
intent, and it reads like a bearer fault rather than a version mismatch,
so install all of them or none.

## One run

1. On each phone, the first run shows the device's public key material
   once, about 4,000 hex characters, after **Start a new identity**.
   `field-setup.sh` has read it already; by hand, read it over adb rather
   than by eye while that screen is up:

        adb -s <serial> exec-out uiautomator dump /dev/tty | grep -oE '[0-9a-f]{400,}' | head -1

   The material is public; nothing secret is on that screen.

2. Start the run from the repository root, naming the run; the phones come
   from `phones.conf`, or name them:

        crates/tools/field-run.sh <label>
        crates/tools/field-run.sh --phone <serial-A>=<material-A> --phone <serial-B>=<material-B> <label>

   The script mints identities for the node and the witnesses, lists both
   phones in the node's peers file (the node admits only identities it
   knows), starts the node and the witnesses, starts the collector for the
   phones' live streams (TCP port 7448 on the same address; `--stream-port
   0` for none), provisions each phone with the other as its *payload*
   peer (the Conversations demo's; the ceremony learns who it meets from
   the codes, not from this), with **one nominee** — the first phone
   witness-1, the second witness-2 — since a phone at genesis has no
   horizon to nominate from and the record would otherwise be a formation
   (the bench standing in for the horizon; a witness is reached over the
   network and need not be present [author, 2026-10-05]). **The nominee
   travels as an introduction** — its material and its signed locator,
   asked of the instrument after its attach [author, 2026-10-24] — because
   a nominee is offered to the counterparty's prune by locator and material
   alone left the set empty (run `58a6883-a5-2`). **A nominee held as a
   contact is dialled directly** (design §12.6.3, the contact rule of
   2026-10-24): on the bench the phones and the witnesses share a network,
   so after the engagement crosses the relay the trace should read
   `pay.route path=direct`, and the node carries none of it. Then the
   collector as its stream target (the app is stopped and started twice;
   the second start is the one that attaches), starts the logcat captures,
   writes `run.json`, and holds. The run is named
   `<commit>-<label>-<n>`; its directory is `runs/<run>/`.

   While it holds, a second terminal can follow the run:

        crates/target/fieldtest/release/rhtn diag watch runs/<run>

   prints each ceremony step, refusal and abort from every file under the
   run directory (the phones' streams, the node's and the witnesses' logs)
   as it is appended, with its source and ms. **The stream is a view, not
   the record.** It is plaintext TCP on the tester's own LAN, so the bench
   must be a network the tester controls (the lines carry what the event
   file carries: identities as eight hex characters, never a frame, key or
   capture); and it is lossy: the phone keeps a backlog of 4,096 lines
   while the laptop is out of reach, drops the oldest beyond that and says
   so with a `diag.dropped` event, and anything that happens between the
   last line sent and a crash may not arrive. The file pulled from the
   phone at stop is complete, and where the two differ the file wins. The
   streamed files sit at `phones/<serial>.jsonl` beside the pulled
   `phones/<serial>/` directories and are not merged into the timeline.

3. Check that each phone's status line reads attached. Run the ceremony from
   Meet: one phone meets, the other scans the bootstrap QR, both read the
   brief and accept, then the captures and the exchange. **Write down what
   the screens showed as they go by** — the manual catalogue rows (the PRD
   series, TRV-12, TRV-13, MET-12) are the only coverage for what a screen
   shows, and a run's notes are the only record of them. They go into this
   run's entry in `Robot/field-test-sequence.md`, in your own words, saying
   which rows the run never reached as well as which it showed.

   **The order the screens go in, since the ceremony was reordered** (design
   §7.1.1): the nominees are engaged *before* the code is shown, so the
   first thing to look at is the line saying how many of the nominated are
   engaged and watching — on the Meet screen, before any code appears. If
   that line says nothing is engaged, the run will still complete and its
   record will be witnessless, which is valid and proves nothing about the
   engagement; stop and find out why rather than reading the rest. Then the
   codes cross, and as they do each nominee is sent the exchange's shape and
   then each part as it lands — that traffic is the run's subject and is
   not visible on either phone. The window opens with the request and the
   channels run inside it, so nobody is asked to hold the phones still and
   wait: the tap is armed while the two are already facing each other.
   Finally the Done screen names who witnessed and how the `nominated_by`
   split fell, or says plainly that nobody did.

4. On both phones press *Send diagnostics (field test)*, on the home screen
   or a Meet screen. The zip is written before the share sheet opens;
   dismiss the sheet and the file stays on the phone.

5. Stop the script: Ctrl-C in its terminal, or `crates/tools/field-run.sh
   stop` from another. It quits the witnesses, stops the node, pulls every
   event file and report zip from each phone's private directory, merges,
   and prints the run directory.

A second run needs step 2 again (new node identity, new provision), and
after a wipe `field-setup.sh` again, or step 1 by hand (new material).

## A nodeless run

No `field-run.sh`: stop any daemon (`crates/tools/field-run.sh stop`) and
leave the phones as the last run provisioned them. The node they name does
not answer, so each stays **detached** and the loop runs regardless
[author, 2026-10-07]; the nominees they name are unreachable, so each
engages nobody and the record is the **witnessless normal** shape. Since
2026-10-24 a detached client has a payload path of its own (design §6.4,
§12.6.3): its direct socket is bound at start, the two phones hand each
other **traversal candidates over the bearer** right behind the intent
(`Carriage.Phase.CANDIDATES`, the `CandidateHandover` of `wire-format.md`
§14.3.2) and the prekeys after the capture key, so the legs after the
record — an adoption, a peering — go point-to-point with no node anywhere.
Read `pay.route path=direct` on them, and `state: a direct path to … is
open` on the window screen.

There is no collector and no pull at stop: take each phone's newest
diagnostics file by hand when the run is over —

    adb -s <serial> shell run-as com.comptus.fueros ls -t files/diag | head -1
    adb -s <serial> exec-out run-as com.comptus.fueros cat files/diag/<file> > runs/<name>/phones/<serial>.jsonl

**The peer exchange and the tethered wipe cycle** (2026-10-24). Two
phones that hold a peering record reach for each other on the kernel's
clock — once a minute in this flavour — and at once when the default
network comes back; each dials the other at the candidates it last
offered, and on each contact sends its locator, asks for the blob the
other holds of it, and deposits a fresh blob where its dataset moved. Read
`peer.contact` (`how=dialled` or `held`), `peer.deposit` and `peer.held`,
then `peer.returned` and `peer.reconciled` on the side that lost
something.

**The wipe is the bench's act, over adb, and there is no control for it
on the phone** [author, 2026-10-24]. The cycle — forget a contact on one
phone, watch the peer's copy bring it back — is a step of the sequence
(`field-test-sequence.md`, A3b) in every setting the peering runs in, and
it is driven from the tethered machine with two intent extras, fieldtest
alone:

    adb -s <serial> shell am start -n com.comptus.fueros/.ShellActivity --es forget_contact <keyhash-hex>
    adb -s <serial> shell am start -n com.comptus.fueros/.ShellActivity --ez contact_peers true

**A third lever, for the verifier rows** (2026-10-25): drop every sealed
capture a phone holds of one party, so that, selected as that party's
verifier at the next meeting, it answers *unavailable* with no basis —
VER-008's case, which no meeting produces by itself, since a subject
introduces only parties it has met and every one of them sealed a
capture of it. The captures come back at the pair's next meeting.

    adb -s <serial> shell am start -n com.comptus.fueros/.ShellActivity --es forget_captures <subject-keyhash-hex>

Forget a contact *other than the peer* — the peer is what the direct path
is gated on — and take its full keyhash from the bench's own record of
who was introduced to that phone, since the phone shows eight characters.
The home screen's notes say what each did.

## Reading a stall or a leak

The field-test build carries three instruments for a screen that freezes
or a process that grows, all in the phone's stream (`phones/<serial>.jsonl`)
[bench, 2026-10-24]:

- **Every boundary call's duration** — `ffi.return` and `shell.return` carry
  `took_ms`. A call made from the thread that draws is a stall of that
  length: `opticalTrace` at a median of 100 ms and a maximum of 6.8 s, 210
  times in one exchange, was the frozen preview of run `1bd1c15-direct-1`.
  Read it as a table: method, count, median, maximum, over the step's window.
- **The main thread's watchdog** — `shell.stall` (warn) when a tick posted to
  the main looper has not run within a second, with the main thread's stack
  at that moment, and `shell.stall state=over held_ms=N` when it does. A
  stall with no `took_ms` beside it is one the boundary did not cause.
- **The heap on the turn's clock** — `shell.memory` with `java_kb` and
  `native_kb` beside every `kernel.turn`. A number that climbs across a run
  and does not come down is a leak; `adb shell dumpsys meminfo
  com.comptus.fueros` on the bench says the same from outside, and `adb
  shell am dumpheap com.comptus.fueros /data/local/tmp/h.hprof` takes the
  heap for Android Studio's profiler where a climb needs a culprit.

## Reading the arrival spread

What PRD-17 is judged on is not on either phone's screen: it is whether the
nominees' diagnostics arrive *spread across* the exchange or batched at its
end. The witnesses are the laptop's, so their files are in the run
directory and the merge puts them on one clock.

        grep 'cer.witness.watched' runs/<run>/timeline.txt

Each line carries `event=` (shape, inbound, outbound or confirmed) and
**`since_ms=`**, the offset from the moment that witness took the
engagement, on its own clock. Read the `since_ms` column down the page:

- **spread** is what the reorder is for — the four events at widely
  separated offsets, the shape early and `confirmed` last, the gaps between
  them the time the exchange actually took.
- **batched** is the failure this replaced: four offsets within a few
  milliseconds of each other, late. The burst that started this work was
  2.9 seconds wide and arrived whole at the end.

Two cautions, both of which would make a passing run look like a failing
one. `event="shape"`, `"inbound"` and `"outbound"` come from the optical
exchange and exist only where two real screens and cameras ran; a run whose
pair never completed an optical pass shows `confirmed` alone, and that is
the run's fault and not the kernel's. And the laptop witnesses are reached
over the network, so a slow LAN widens every offset equally — the shape to
look for is the *pattern* of gaps, not any one number.

`cer.witness.withheld` and `cer.witness.matured` are the window's own
record: a witness that was asked inside its window withheld the signature
and said how long it had waited, and signed when the window matured on its
own clock. On a `fieldtest` build that window is 30 seconds, not the
design's four minutes, which is why a run says nothing about CER-026.

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

If a phone's pull failed, `phones/<serial>.jsonl` (the stream as the
collector received it, lossy) and `phones/<serial>/logcat.txt` (the logcat
mirror) hold the same lines; the stream's file carries its own anchor in
its hello and merges as it is. The pulled file carries no anchor of its
own; the script builds one from the `shell.start` event, and a file without
that event sits at the head of the timeline by its own clock, which the
summary says. The
kernel's lines on a phone count from `Participant.start` and sit early by
that offset, which the `shell.call` for `Participant.start` gives.

**An empty `logcat.txt` is not a phone that did nothing.** The mirror is
`Timber.tag("fueros.diag").d(...)`, at debug level, and a ROM that drops
debug for a tag it does not know mirrors nothing while the app runs
perfectly: both TCL 30 Z units wrote their event files normally and
mirrored not one line (2026-10-22). `field-run.sh` now sets
`log.tag.fueros.diag` and its `persist.` form on every phone before it
launches the app, so this should not recur; by hand it is

        adb -s <serial> shell setprop persist.log.tag.fueros.diag D

and the app must be restarted, since the level is read as each line is
written. The event files pulled at stop were never affected — they are the
record, and this only decides whether `logcat.txt` and `rhtn diag watch`
are worth reading on that phone.

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
§7.5.1: whole years of 365 days, two by default). The figure is settable
since 2026-10-03: `field-run.sh --retention <years>` puts a
`retention_years` field in each phone's provision, the kernel takes it at
start (`Participant.set_retention_years`, refused during a ceremony), and
the brief states the figure the device will declare. The unit is whole
years on the wire, so the plan's one day is not expressible; **what a
field-test record should declare is the author's call**, and until he makes
it a run without `--retention` declares two years. Either way the wipe below
is what bounds the captures on the bench.

## The wipe between testers

On both phones, since each holds a capture of the other tester:
`crates/tools/field-setup.sh --no-build` uninstalls, reinstalls and reads the
new material; by hand,

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
