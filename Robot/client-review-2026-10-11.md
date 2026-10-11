# The client, examined on two questions (2026-10-11)

Asked by the author after a day of bench fixes: **(1)** have the fixes
landed as tuning of robust abstractions, or as epicycles over broken
ones; **(2)** does every form of meeting run with neither participant
attached to a node or holding a patron, and does the installed app's
own UI suffice to bootstrap a person into full participation — intake a
locator, discover anchors from it, meet or adopt as either party, launch
a node — for whatever of that is built. Read from the code, not from
memory; file:line as of `b26eee1` plus today's uncommitted work.

## 1. Cruft: what the day's fixes did to the abstractions

The day's fixes, by where they landed:

| Fix | Landed as | Verdict |
|---|---|---|
| Nomination from met parties | `Client::met` over the acquaintance the kernel already keeps; `begin` falls back to it | tuning |
| A witness both name signs once | one dedupe in `propose`, the funnel both drivers use | tuning |
| The shell stops waiting where no witness can come | `Meet.poll`'s `NoWitness` branch reads the nominee lists | tuning, but of a polling design (below) |
| Notifications gated on `onNotificationSent` | `awaitPermit(gate, op)` serves both sides; the write gate's loop became that helper | **simplification** |
| The advertisement named by the ceremony | `uuidFor(ceremony)`; `quiet()` | tuning |
| A completed exchange is news | `doneBefore` added to a two-count comparison in `MeetActivity` | **epicycle** — see 1.1 |
| Store opens six or seven collections | `decode` accepts either | tuning |
| The bundle as introductions | `selection::pool` rewritten over one type; the window and `unverifiable` deleted | **simplification** |

Three places where the accumulation is real, each with the abstraction
that would absorb it:

### 1.1 The optical screen judges "news" by hand, and keeps being wrong

`MeetActivity` (≈1050 lines, 14 dated fix-comments, the most of any
file) drives one `OpticalExchange` from **two callbacks** — the decode
callback and the tracking-row callback — and each decides for itself
whether anything changed: `heldBefore`, `theyHeldBefore`, now
`doneBefore`, and the tracking path's own `before`. Today's hang was the
third defect of this family (2026-10-08 the header re-opening a part;
2026-10-10 the mirror rule; 2026-10-11 `done()` flipping with no count
moving). The underlying abstraction is sound — `OpticalExchange` is a
pure state machine with a clear `take`/`takeTracking`/`done` surface —
but the *screen* reimplements its change detection. **Absorb it:** give
`OpticalExchange` a monotonic `version` bumped by every mutating method,
and have the screen redraw and run its `done()` branch whenever the
version it last drew differs. The three `*Before` variables and the
early returns go; the camera's identical-frame suppression stays, now
harmless.

### 1.2 The conversation is polled, from two drivers

`Meet.poll` is called by the turn timer **and** by every conversation
event; the proposer asked `conversePropose` 28 times in 3 s on the bench
and the kernel answered `Waiting("the counterparty's responses")` each
time — a refusal used as a poll result. The state the poll manages
(`opening`, `opened`, `gathered`, `waitingOn`, `noWitness`,
`queriesOutAt`, `lastAnswerAt`, `goOn`, `wentOnWithout`: nine fields,
Meet.kt:359–388) exists to decide *when to try again*. The kernel
already knows when a proposal becomes possible: it holds the
conversation and emits `Conversed` events. **Absorb it:** the kernel
exposes `progress()` already; make the shell attempt `propose` only on
a `Conversed` arrival or the patience timer, never on the turn, and let
the kernel say "proposable" in `Progress` rather than refusing with a
string the shell parses (`refused.startsWith("Waiting(")`,
Meet.kt:923). Half the nine fields fall away.

### 1.3 Two drivers of one ceremony in the kernel

`Harness::run` (ceremony.rs:4048–4463, ≈415 lines) walks the ceremony
by hand with its own `Msg` enum — intents, channels, captures, witness
requests, proposal, signatures, finalization — while `conversation.rs`
(480 lines) is the driver the shell actually uses. Both reach the same
kernel methods, which is why the dedupe fix in `propose` covered both;
but the harness builds its witness list itself (ceremony.rs:4036–4052)
and the conversation builds its own, so a defect in gathering appears
in one and not the other, and every test in `client/tests` exercises
the path the phones do not run. **Absorb it:** the harness should drive
the conversation — deliver kinds 9–18 between in-process clients
through `conversation_from`/`carriages` — and `Msg` should shrink to
what the conversation does not carry (the local exchanges). This is the
largest item and the one most worth doing before series C.

### 1.4 Things that look like epicycles and are not

- `OpticalExchange.mine()` asked only of the contribution (OpticalExchange.kt:247–267): the transcript is identical on both sides by construction, so a reflection and the genuine code are one code. That is a fact about the object, stated once. Fine.
- `take()`'s "a code of the next exchange means they have everything" (OpticalExchange.kt:275): the next exchange's first frame is a stronger statement than any header of this one. Fine.
- The pre-33 BLE paths (`write`, `notify`, `onCharacteristicChanged` ×2): the platform has two APIs. Fine.
- The formation rule in `propose` (witnessless only at genesis): correct to the documents as written; the ruling that a witnessless meeting is not genesis-only is **owed to design §13.2 / `wire-format.md` §3.2** and until amended the kernel is right to refuse.

## 2. Nodeless operation and the installed app's bootstrap

### 2.1 The hard dependency: no provision, no event loop

`Kernel.bringUp` (Kernel.kt:947–957): with no `provision` blob stored,
the shell prints "run `payload-peer` on the workstation and hand its
PROVISION line back" and **returns before the event loop**. The loop is
what drives every ceremony step after the optical exchange (it pumps
`nextEvent`, drains the bearer, runs the conversation). So the installed
app, untouched by the harness, can start the kernel and **cannot meet
anyone**. Every run to date — nodeless included — went through
`nodeless.sh` or `field-run.sh` writing that blob. `nodeless.sh` even
says what it provisions: "nothing about anybody … a `known` set that
names only the device itself". That blob exists to get past this
`return`.

**This is the one finding that fails the author's second test outright,
and the fix is small:** a device with no provision is the ordinary
device. Start the kernel with `known = [self]`, stay detached, run the
loop. The provision then becomes what it says it is — a bench
convenience for a peer name, nominees, a node and a stream — and
`nodeless.sh` provisions nothing at all.

### 2.2 What the UI offers against the four functions asked for

| Function | State |
|---|---|
| **Intake a locator** | No screen. `People` is a placeholder (HomeActivity.kt:46). The kernel has no call either: `known` is set at `Participant.start` and grows only from verified introductions inside a ceremony; nothing takes an introduction from outside one. The kernel needs `take_introduction(bytes)` (an `Introduction`: key material + signed locator, exactly the bundle's unit) and the shell a way to scan or paste one. |
| **Discover anchors from it** | Not built. `reach_for` (ceremony.rs) resolves only through `horizon.upstream_of`, which reads places and endpoint records the horizon holds — filled by `attach`/`sweep`, i.e. by a serving node. A handed locator's `anchor` is never consulted; a client holds no anchor-address table of its own. So **a nodeless participant cannot reach a witness or verifier at all**, however the locator came — which is the bench's "unreachable nodeless" finding restated as a design gap: design §12.3 Case 1 ("Bob looks up `anchor` in his local table for an address to start from") has no local table in the kernel. |
| **Meet as either party** | Built; both roles; works nodeless **at genesis only** (2.3). |
| **Adopt as either party** | Kernel: `propose_adoption`, `sign_body`, `take_adoption` exist and are exposed over the ffi. Shell: the kind is chosen at D1 and carried in the brief; nothing proposes or takes the body after the record (D7 unwired). Not operable from the UI. |
| **Launch a node** | `Operator` enrols an **existing** instance given its address and token (OperatorActivity.kt:26–35, "later these arrive from a provider's launch flow (H1)"). Creating one is not built. |
| **Converse** | `Conversations` opens only on the provisioned `peerKey` (ConversationActivity.kt:36, Front.kt:77). After a meeting, the counterparty is `adopt(who)`'d into `peer`, so one met party is reachable; there is no list of met parties to pick from. |

### 2.3 Which meetings work nodeless today

- Two phones at genesis: yes (the formation shape), given the provision blob of 2.1.
- Any phone past genesis: **no**. `propose` refuses `NoWitness` unless the ceremony is a formation; a nominee can be named but not reached (2.2's anchor gap); so a nodeless pair where either has met before stops at the verifiers. This is the 2026-10-10 ruling ("if each participant can see that no witness is available, they should proceed") waiting on design §13.2 / `wire-format.md` §3.2 — and even once amended, a nodeless pair with reachable witnesses needs the anchor table.
- Verifiers nodeless: same gap. The introductions now carry the locator to reach a verifier by; nothing reads it for routing.

### 2.4 Harness-only inputs still wired into the shell

`provision.known` (witness material — with introductions this is only
needed for nominees named by the bench), `provision.nominees` (no UI to
nominate; the kernel fills from met parties otherwise), `provision.peer`
/ `peer_name` (the chat's recipient), `provision.node` / `addr`
(attach; the operator screen is the user's path to this),
`diag_stream`/`diag_serial` (fieldtest only, fine).

## 2.5 [author, 2026-10-11] The persistence principle and the contacts policy

Given on reading 2.2's anchor-table finding (verbatim in
`Robot/review-tracking.md`, this date): a ceremony resolves back to the
live state as it was, plus contacts, the record and the meeting status,
all persisted; and clients capture every locator that becomes visible
— witnesses, verifiers, upline nodes, proposed-and-rejected candidates
— up to a storage limit, pruning least-interacted first toward a compact
set of anchors. The anchor table of 2.2 is this contacts store read for
routing. Owed: the document text (light-client beside `light-client-requirements.md` §4.2) and an audit
of live state that outlives a ceremony (kernel `active`, `acquaintance`,
`witnessing`, `payload.handed`; shell `Kernel` singletons and `Meet`).

## 3. Recommended order

1. **2.1** — run the loop without a provision (hours). Then wipe one phone and prove a cold meeting from the installed app alone.
2. **1.1** — version the exchange; delete the hand-rolled change detection (hours).
3. **2.2 intake + anchor table** — `take_introduction` in the kernel, a `People` screen that scans or pastes one, and a client-held anchor-address table that `reach_for` consults when the horizon has nothing (the design's local table, design §12.3 Case 1). This is what makes a nodeless participant able to reach a nominee or verifier through *their* node. Needs the author on what populates the table (the introduction? a node's address learned at attach? both).
4. **1.2** — the conversation driven by events, not polled.
5. **1.3** — the harness drives the conversation. Before series C.
6. design §13.2 / `wire-format.md` §3.2 — the witnessless-past-genesis amendment, the author's.
7. D7 adoption in the shell; H1 node launch — builds already on the plan.

## 4. The persistence audit (2026-10-11)

Every holder of live state, kernel and shell, and what becomes of it when
a ceremony ends or the app restarts. **Persisted** = written and read
back; **scoped** = dropped when the thing it belongs to ends; **residue**
= outlives what it was for and changes later behaviour.

### 4.1 Kernel (`rhtn_client::Client`)

| State | Lifetime | Verdict |
|---|---|---|
| `archive`, `store` (sealed, seeds, records, late, disclosures, locators), `payload` (sessions, prefetched, handed), `horizon` (seen, snapshot, delegations), `provider_credential` | `durable()` / `at()` | persisted |
| `positions` | re-derived from the archive at `at()` (`adopt_own_positions`) | derived, correct |
| `active` (the ceremony: counterparty, contributions, nominees, their bundle, seed, channels, sealed capture, issued queries, responses, conversation) | `take_if(mine)` at finalize; `None` at abandon | scoped, correct |
| `subject.responses`, `subject` window | cleared at finalize and abandon | scoped, correct |
| `witnessing` (ceremonies this client attests) | retained-out at the finalize that closes them; removed on decline | scoped, correct |
| **`acquaintance`** (met / horizon / reachable) | `Acquaintance::default()` at `new()`; folded from records only at a finalize | **residue, fixed today**: a restarted client had met nobody until its next ceremony ended — `met()` empty, no nomination, no candidate recognised. `at()` now folds it at every start |
| **`known`** (key material, `Vec<Identity>`) | supplied by the caller at start; grows at the optical pin and from every verified introduction; **not in `durable()`** | **residue in the other direction**: the material a ceremony taught is lost at restart, so held locators can no longer be verified or introduced and a met counterparty's later signatures cannot be checked. A phone meets GXV today and tomorrow holds GXV's locator with nothing to verify it under. This is the contacts store's half that does not exist yet |
| `store.locators` | persisted | half a contact: the locator without its material (above) |
| `cfg.retention_years` | set by the shell at start from the provision | fine; becomes a setting when the UI has one |

### 4.2 Shell (`Kernel` object, `Meet`, activities)

| State | Lifetime | Verdict |
|---|---|---|
| `meet` (the `Meet` state machine, its `OpticalExchange`) | one per meeting; `null` at `stopMeet` | scoped — but `startMeet` returned a DONE or STOPPED meeting to the next screen; **fixed today**: a finished meeting gives way |
| `opticalTaken`, `carriage`, `ble`, `bearerWork`, `sentCarriages` | reset at `stopMeet` / `carryIntent` | scoped, correct |
| **`peer` / `peerKey` / `peerName`** (the chat's recipient) | set from the provision at start; overwritten by `adopt(who)` at every optical read; never persisted | **residue**: which party the conversation screen opens on depends on which meeting this session ran last, and reverts to the provision at restart. The right shape is a contacts list the person picks from, which 2.2 already asks for |
| `nominees` | from the provision | bench-only; the kernel fills from met parties otherwise |
| `ProximityChannels` (cached tap by ceremony-id) | keyed by id; another id starts from nothing | scoped, correct |
| `QrCamera` (`lastPayload`, watchdog clock) | one per `MeetActivity`; closed with it | scoped, correct |
| `Front` (notes, status) | display | fine |

### 4.3 What follows

The two findings that matter are one thing: **material and locator are
persisted apart, and only the locator survives**. The contacts store of
`light-client-requirements.md` §4.3 — `{keyhash → material, signed locator, last dealt
with, how learned}`, bounded, pruned least-interacted first — replaces
`store.locators` and the in-memory growth of `known`, is written like
the records are, and is what `introductions()`, `hold_locator`,
`reach_for`'s anchor lookup and the shell's conversation screen all read.
Capture points, per the policy: the intent (counterparty), every
introduction in a bundle (selected or not), every nominee and verifier
that answered, the infrastructure traversed, a fishing candidate
declined. That is the next build.
