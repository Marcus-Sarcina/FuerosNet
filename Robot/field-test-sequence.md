# The field-test sequence: plan of record

Scoped with the author, 2026-10-10, after the first cold ceremony on
low-spec hardware (`82ffd1b`). The author's list, with the assistant's
additions and reorderings taken; two of the assistant's first readings were
wrong and are corrected below. **Robot/ is the assistant's workspace**: this
file is operational and is pruned or rewritten as runs land.

**Legend.** *proven* — passed in-process (a named test); *hardware* — passed
on phones or containers; *build* — code that does not exist yet and must
before the test can.

**The bench.** Four phones: two SM-G996U1 (S21: Android 15, API 35, NFC and
UWB) and two TCL 30 Z (4188C: Android 12, API 31, 2.7 GB, BLE only), all in
`crates/tools/phones.conf`. The cold bench is `nodeless.sh --phone A
--phone B <label>`; the node bench is `field-run.sh`, which provisions a
daemon, two instrument witnesses, and names a nominee for phones at
genesis. **The cold bench no longer needs phones at genesis** [author,
2026-10-19: there is no witness floor]: the `NoWitness` refusal is gone, so
two phones that have met others reach a witnessless *normal* record rather
than being refused, and the record says permanently that nobody attested
(`wire-format.md` §3.2). Wiping the two first (`field-setup.sh --conf` on a
file naming only those two keeps the others' identities) now **chooses the
shape** — a formation at genesis rather than a witnessless normal record —
instead of deciding whether the run can finish at all.

## Two axes, not one sequence

The ceremony's **variants** (plain, adoption either way, peering,
witnessed, verified) and its **setting** (nodeless, or with a node on the
bench) are independent: adoption and peering are participant acts that
work with or without an infra node — the design has light-client patrons
as ordinary ("a leaf node's patron may be a light client"; resolution walks
"up past any light-client patrons"), a root is self-anchored, and design §6 prices
a peering edge like a proof-of-presence edge because it "joins parties who
by construction share no position". The assistant first ordered a node
before adoption and read peering as node-to-node; both wrong.

## A. Ceremony variants — nodeless first, then with a node

| | Proves | Needs | Stands on |
|---|---|---|---|
| **A1 Mixed S21/TCL, plain** — **done, `82ffd1b-a1-mixed-1`**; range test still not run | the asymmetric case: a good camera on a dim screen and the reverse; the strongest-channel rule across unequal hardware (the TCL pair passed on Optical alone); **the range test**, still an S21 figure; first mixed-throughput data for the presentation-rate idea | nothing new | hardware: both symmetric pairs |
| **A2 Adoption, both directions** — **the scope-carrying pass done, `3cd8165-a2-carry-1-1` (2026-10-15)**: two meetings on one run. First GXV adopted ZU8 (record `89745eb3`, adoption `29b0c905`), so GXV rooted a scope; then GXV initiated as the client, was asked *submit as which scope?* and chose to bring it, 4VC scanned and was asked *adopt into which scope?* and chose the subnet it had adopted MRK into (`a9b13bcd…`); record `4a9d181b` with 4 signers (two witnesses this time), GXV offered series 2 "bringing scope 29b0c905…", 4VC proposed with field 10 (13 ms), GXV's kernel checked it as a scope it roots and signed (21 ms), both hold `b1232720`; the daemon stored it at 7240 bytes against 7170 for an adoption carrying nothing — the locator's subnet and field 10. **Reverse direction done, `3cd8165-a2-reverse-1-1` (2026-10-15)**: MRK initiated as the client, 4VC scanned and, holding no subnet, was answered *adopt into which scope?* unasked (a new subnet under its own key); record `c5692877` (3 signers, 90 s), the adoption `a9b13bcd` proposed by 4VC (25 ms), signed by MRK's kernel (14 ms) and held by both; MRK offered series 1 *as an individual*, rooting nothing. One defect, fixed: the direction frame's label was composed in the patron's frame ("under your own key"), now in the counterparty's ("a new subnet under c9b44cb4"). Still to run: a pass where the subordinate brings a scope. **Rerun after the rework, `1807b03-1807b03-a2-adoption-1-1` (2026-10-15)**: GXV patron as a lone root, ZU8 adopted; record `2a6bde34` with 3 signers (one laptop witness, the other side's nominee was the same one), the adoption `2b7a56fd` held by both and stored by the daemon (the presence record `OutOfStore` there, as in the earlier run); the direction frame carried the subnet's label ("into … a new subnet", worded better since), the body named no subnet as a lone root's first adoption must, and ZU8's kernel signed it (`cer.adoption.signed`, series 1). Both S21s logged two `sendmsg … I/O error` warnings at attach with tethering off, and attached in 1.3 s regardless. **First pass with a node, `722519a-a2-adoption-7`**: GXV patron, ZU8 adopted, record `d3ca1939` with 4 signers and both verifiers answering Match, then the adoption `de2b5961` held by both — the node offered series 1, the patron proposed, the node's kernel checked the body and signed it (`cer.adoption.signed`). **One defect, fixed**: the responder's brief said nothing about the patronage, reading this device's own D1a choice instead of the settled direction. Still to run: the reverse direction, the simultaneous-opposite case, and the nodeless pass — which **no longer needs two phones at genesis** [author, 2026-10-19]: on phones that have met, it finalises as a witnessless normal record carrying the adoption, which is the cheaper run and the one that keeps B's state. Wiping first would prove the formation shape instead, and that shape is already covered in-process by CER-50's first half and on hardware by the earlier cold runs, so the met-phones pass is the one to run and the genesis one only if the author wants the adoption proved under a formation too. Whether the legs carry registered kinds is still the author's to rule, and the answer does not change what the bench runs | `THEM_UNDER_ME` and `ME_UNDER_THEM` chosen at D1; a TCL under an S21 that is its own root; then D7's simultaneous-opposite case; then with the patron attached to a node | nothing new nodeless; a node for the second pass | proven: `adoption_under_slot` |
| **A3 Peering** — **the kernel act is built (2026-10-15), the shell is not**: `Client::propose_peering` / `review_and_sign_peering` / `take_peering` and the three over the ffi, a light client peering **by locator** as `wire-format.md` §4.4 field 3 admits, on a meeting of the two; `rhtn-archive` is the one emitter of the body now and the node's own wrapper calls it. Owed: a shell screen, which the Meet flow's `DONE` step is the place for, as D7's adoption is | a peering edge between two phones, nodeless; then with a node | a kernel act, then the shell | proven: peering priced and propagated in-process |
| **A5 The arrival spread, the witness reorder's own run** — **run `58a6883-a5-2` (2026-10-24, GXV–MRK, on the restructured machine): the spread is shown; the record came out witnessless, for a reason the bench owns.** Each side engages its own nominees *before* the ceremony-id exists and the counterparty keeps at most three of what it is offered (design §7.1.1), so every surviving witness is nominated by one side and selected by the other. What no in-process test can show is the **spread**: `cer.witness.watched` carries `since_ms` per arrival, and the shape/inbound/outbound events come from `OpticalExchange.take()`, which needs two real screens and cameras. A live adaptors run shows `event="confirmed" since_ms=819` and nothing before it, because the harness fixes the ceremony-id by hand and has no optical pass. **The verdict**: `since_ms` spread across the exchange rather than four offsets within milliseconds of each other at its end — the burst that started this work was 2.9 s wide and arrived whole. Reading it is `grep 'cer.witness.watched' runs/<run>/timeline.txt` (`Robot/field-test-procedure.md`, *Reading the arrival spread*). **One thing the run as configured does not prove**: `field-run.sh` provisions **one** nominee per phone, so the counterparty's prune keeps 1 of 1 and the truncation to three is never exercised — a question for the author, below **Result.** witness-1 saw 118 `cer.witness.watched` events, `since_ms` 5,213 → 22,750 (17.5 s wide), the shape first and inbound/outbound interleaved after it; witness-2 118, 2,866 → 21,359. Not a burst: the verdict holds. The regime did what it was restructured to do: the window opened 0.7 s after `OPTICAL → WINDOW`, the queries went at once (pool 0, none required), the hold ran 30 s in 41/45 ticks with a template after the first prompt at 5 s / 11 s, the seal at 30 s, the proposal 65 s after `cer.begin`, both to Done on record `03af6345`, signers=2. **Two defects.** (1) `cer.witness.set_sent offered=0` on both: `my_witness_set` offers a nominee only as an introduction, which travels by a held `SignedLocator`, and a nominee the bench *provisions* carries material and no locator — so each nominee was dismissed (`kept=0 dismissed=1`) and the record is witnessless; the runs where `offered=1` had nominees from a met party. The bench stands in for a horizon and must stand in for the locator too (hand the phone an introduction, as `take_introduction` already takes one), unless the author wants the kernel to resolve a nominee's locator through the node — a question put to him. (2) 57 (GXV) / 59 (MRK) trace publications were refused in the exchange's last eight seconds (`the serving node refused the publication`: the courier's relayed path returned false; the node's mailbox log shows only Stored/Delivered), and the witnesses' event kinds hold no `confirmed`, so the ceremony-id confirmation is among what did not arrive — open; the cause is not yet found. Also noted: `cer.witness.watched` carries an inner `"event"` key beside the line's own, so a JSON reader sees only the inner one (the timeline tool copes; rename it); `cer.continuity would_abandon=true` on both at 3.0 s / 9.7 s away of 30 s, which is the deferred design §7.5 figure doing what the deferral predicted. | the four `cer.witness.*` arrival events on a real optical pass; the window opening with the request (CER-032; PRD-18 is withdrawn with the channels); the Done screen's `nominated_by` split (PRD-19) | nothing new: the instrument is built and the diagnostics flow. GXV–MRK, the pair the author named; with ranging gone [author, 2026-10-24] the pair is a choice of screens and cameras, nothing else | proven in-process: CER-67's ordering, CER-032's window contents, CER-52's withholding |
| **A6 The witnessless normal record, nodeless** — **first attempt 2026-10-23: stopped, and found a defect no in-process test could reach.** Both TCLs completed the witness exchange — `cer.witness.pruned offered=0 kept=0` on each — and then aborted 48 ms later at `converse_open` with `Waiting(the counterparty's keep of this side's witness set)`, stopping at VERIFIERS. ZU8 sent its set at 84214180 and received the counterparty's at 84243341: **29 seconds to cross that bearer**, so the keeps were still in flight when the captures finished. `Meet.openWindow`, which runs during the channels, retries a `Waiting` and has a test saying so; `Meet.converse`'s fallback, which runs after them, treated every refusal as fatal — and after the channels nothing calls `openWindow` again. Fixed in `b091a81`: the fallback waits the `Waiting` out on its own thread, 1 s between asks, bounded by the flow's 60 s patience, any other refusal still fatal; Android tests 210 → 212. **Second attempt: a transient BLE failure before any of it** — 4VC scanned, found the peer at rssi −61, dialled once and got `status=133`, Android's generic `GATT_ERROR`; `seek` stops scanning the moment it sees the peer and dials once, so there is no path back to either scanning or dialling inside the 30 s link window, and the intent never crossed. Intermittent, not fixed: the first attempt crossed the intent on the same pair. **Third attempt reached the verifiers and hung there**, which found the deeper one — nodeless the conversation's messages arrive as bearer carriages with **no kernel event behind them** (zero `shell.event` lines in the whole run), and `Meet.arrived` was set only by `conversed()` and `answered()`, both event-driven. So `pollDue` was false for ever and `poll` never ran: the counterparty's responses were taken and saved while nothing proposed, and the `STALE_MS` watchdog that would have ended it sits *inside* that poll. Fixed in `99a58a7`: `Meet.carried()` from the bearer drain, and `pollDue` no longer puts the watchdog behind an arrival. **Only A6 reaches this** — with a node the conversation arrives as kernel events; A6 is the first run with no node *and* no verifier, the one shape where nothing wakes the flow and nothing times it out. Android 212 → 214. **DONE, `a6-witnessless-s21-4` (2026-10-24)**: `cer.propose witnesses=0 responses=0 **formation=false**` — a witnessless **normal** record, both phones at Review and Sign. The shape the lifted floor newly permits, refused `NoWitness` before 2026-10-19, now reached on hardware. It took seven attempts and each bought a defect no in-process test could reach, the last two being the ones that mattered: **arrival did not drive the machine** (MRK received the responses — `phase 7, 32 bytes, duplicates 0, refused 0` — and never proposed; `drainBearer` now polls what it drained, and the same leg proposed 586 ms later, `1e981bb`), and underneath it **the kernel's event loop had never run in any nodeless run at all**: the phones carried `node=5046da37 addr=192.168.0.8:7447` from an earlier `field-run`, nothing was listening there, and a failed attach returned out of `bringUp` before the loop — so no poll, no watchdog, no heartbeat, through all seven. Found by the loop instrument's silence, since `kernel.loop` is said once unconditionally and was absent. Fixed: a node named but unreachable is the detached case and the loop runs, which is the rule a provision naming *no* node already followed [author, 2026-10-07]. **Measured while there**: the whole ceremony is **5,462 bytes in 17 packets**, every transfer under 600 ms, so no delay in any run was ever the link [author, 2026-10-24]. **PRD-19's third case did NOT pass** and is unreachable by construction: `no_witness` is `initiator && waiting_on.is_none() && witnesses == 0 && !formation`, so a responder never qualifies and a proposer is proposable in the same breath — `poll` proposes and returns before the plain statement is said. The screens showed `witnesses: 0 of mine, 0 of theirs`, which is the first case's wording. Stubbed the same day [author, 2026-10-24: "we aren't doing UX yet"]: `no_witness` now holds on both sides once the sets are settled (`opened && witnesses_outstanding == 0 && witnesses == 0 && !formation`) and the shell says `state: no witness attests this meeting — witnesses=0, formation=false; record field 4 absent, permanent.` before any poll branch can return; developer wording until the UX pass. **Confirmation run with the loop alive (`0defdb9-a6-confirm-loop-1`, 2026-10-24): the loop is confirmed — `kernel.loop=1`, ~245 turns, heartbeats on both S21s — and the ceremony failed on a new defect the dead loop had been masking by luck of timing: with no nominee in the provision the kernel nominates from parties met, and nodeless the counterparty is learned from the code *after* `begin`, so MRK — whose only met party was GXV — offered GXV to GXV, GXV refused the set as "an introduction to myself", never sent a keep, and MRK waited out its 60 s (`the conversation could not open: Waiting(the counterparty's keep…)`). Fixed: the set drops the counterparty where it closes (`opening_id_fixed`, where the peer is known), the receiving side drops an introduction to itself rather than refusing the set, and `unreachable`'s `usize` underflow (`18446744073709551615` on MRK) is saturating. Regression test `the_counterparty_learned_after_begin_is_never_offered_as_a_witness`: on first writing it **passed with the fix neutered** and so proved nothing — `take_optical` already removes the counterparty from `my_nominees`, so a test that never engaged could not offer it. The bench had *engaged* before the code (`cer.witness.engaged engaged=1`), and the set closes from `engaged_to` once an engagement was attempted, which `take_optical` never touches: that is the exact gap. With `converse_engage` before the code the test fails without the fix on the bench's own first symptom, `Refused { kind: 21, "not a message of the conversation" }` — the trace confirmation sent to an engaged nominee who is the counterparty — and passes with it. **CLOSED — `6824de15`, 2026-10-24, run from the apps on GXV–MRK after `31a372b`**: `cer.finalize txid=6824de15 signers=2 mine=true` on both, `cer.propose witnesses=0 responses=0 formation=false`, REVIEW → DONE on both — a witnessless normal record held by both phones, and the first run with the loop, the watchdog, the heartbeat and the arrival-driven poll all live together (`kernel.loop=1`, heartbeats through every step to DONE, one benign `cer.abort` at `converse_propose` "the body is out and its signatures are awaited"). GXV engaged MRK before the code and dropped it at the close (`cer.witness.counterparty_dropped=1`); both offered 0, both pruned 0 and kept 0. **PRD-19's third case observed on both screens** (`state: no witness attests this meeting — witnesses=0, formation=false; record field 4 absent, permanent.`), developer wording. **One figure to carry into the regime question**: each side's set went out at the id and its keep came back **30 s later** (GXV 56.1 → 86.7 s, MRK 52.5 → 82.7 s) — not transfer, which is under 600 ms, but the counterparty not draining the set until its own capture had finished; it blocked nothing this run only because both offered nobody. No `runs/` directory, since the ceremony was started from the apps; the event files are kept under `runs/31a372b-a6-s21-5/phones/`. **Still to run** — **the run the lifted floor newly permits** and the cheapest on the bench. Two phones that have met anyone meet each other with no node: each side engages nobody, each sends an empty witness set, the counterparty answers with an empty keep, and the record finalises as a **witnessless normal** record with field 4 absent — saying permanently that nobody attested (`wire-format.md` §3.2). Before 2026-10-19 this was refused `NoWitness` and the only nodeless run available was between wiped phones, which cost the identities series B is assembled from. **Run it before A5**: no node, no provisioning, no wipe, the fewest moving parts of any run here, which makes it the cheapest way to learn that a freshly installed APK completes a ceremony at all — a failure in A5 first would not say whether the cause was the reorder, the install or the node | that a non-genesis pair finalises witnessless on hardware; **PRD-19's third case**, a witnessless record said plainly on both screens, which no run has reached | nothing new: `nodeless.sh`, two phones that have met, no wipe | proven: CER-50 (a pair meeting at genesis and then again past it, finalising witnessless); the empty set answered with an empty keep (`opening_id_fixed`) |
| **A4 Witnessed** — **done, `82ffd1b-a4-node-3`**, both pairs (S21s `41e88dba`, TCLs `1a2556fb`), 4 signers each, both bench witnesses attesting; the TCLs' first attempt found the bearer's server side ungated (fixed: `notifyDone`) | the far-node path, with-node only: the nominee must be reachable, and a nominee met nodeless has no route (`reach_for`); nomination from met parties is built (2026-10-11: `Client::met`, a witness both name signs once, the shell stops waiting where no witness can come) | a node on the bench; nomination from met parties | proven: the witness path in-process; `field-run.sh` names nominees |

## B. Verification

Every test here needs a **prior ceremony**: the verifier holds a sealed
capture of the subject from an earlier meeting. Four phones make this
possible; it is also why phones are **not wiped between runs** once B
begins. **The exception is gone** [author, 2026-10-19]: it used to read
"except the two a nodeless run requires at genesis", and no nodeless run
requires genesis any more. This was the scheduling conflict at the centre
of this plan — a nodeless run cost two phones' identities, and B is built
out of what those identities accumulate — and it has dissolved rather
than been resolved. All four phones keep their state indefinitely, and the
nodeless rows can be interleaved with B instead of competing with it.

With A having met B earlier, B meets C and A is selected:

- the same-face test the author ruled on 2026-10-06: A asked about B → **match**; A asked about a party it never met → **unavailable**, its own held evidence refuting the basis (VER-008)
- a mismatched face → **no-match**; an unusable or unreleased capture → **inconclusive** or **unavailable** with no basis (VER-012); an older modality against its legacy engine (VER-013)
- a wrong query hash or missing consent refused without a fabricated response (VER-004); the grant delivered straight to the verifier, bypassing querier and witnesses (VER-010); a grant before its query buffered briefly (VER-011)
- the response reaching both querier and subject (VER-015); a late reply kept private or as the optional late object (VER-016)
- **with a node** — a party met nodeless has no route afterwards (`reach_for` routes by position through infra; a ceremony's direct-path candidates are not kept), and between pairs sharing no party the pool is empty by construction (`wire-format.md` §5.3's "verifies alone"; `unverifiable` is never resolved). B1 on the cold bench showed both; **B1 with-node (`82ffd1b-a4-node-3`, `4c9d99b7`) confirmed the pool: `n:1 pool:0` on both sides, the handed record unverifiable for the co-signer never held**. **The ruling that gated this was already made** [author, 2026-10-11, confirmed 2026-10-15]: `wire-format.md` §5.4's bundle is introductions, each carrying the candidate's key material and the most recent locator the subject holds, so the key to verify a candidate's answer arrives with the candidate and `selection::pool` reads it. The `pool: 0` on `82ffd1b-a4-node-3` was who had met whom — each bundle named only the counterparty — and not a missing decision. **What B needs is a pairing**: two parties whose bundles name a third each has met, which the bench now has after three adoption runs

## C. Infrastructure

**C1 Launch a node.** The proven half: phone-to-container enrolment
(hardware, 2026-10-10). The **build** half: H1 — creating the instance from
the client; on the bench, the client launching a container on this machine
(`local.sh` is the stand-in); the cloud flow later and larger. ~~Also owed: renewal~~ — **built 2026-10-16**: the instance
reports `run-ends`, the next run opens exactly there, and the CLI, the
kernel and the Operator screen each do it (`light-client-requirements.md`
§10).

**C2 Node-to-node peering** — two containers; proven in-process
(`a_new_root_peers_floods_and_is_reached_over_real_sessions`).

**C3 The integrated chain**, as runs:
- patron attached to the node — proven (`attach_to`); hardware is the `field-run.sh` shape
- patron accesses a resource; a sub accesses one under a grant — the node side proven (hosted package served, grants by predicate, a resource on its own port); ~~build: the client's Catalog screen is a placeholder~~ — **built 2026-10-16**: `CatalogActivity` with a pure `Catalog` for the roll and `wire-format.md` §11.1's six statuses, and `Client::request_resource` behind it. What is still owed on the bench is the **grant-side** half, a sub reaching a resource under a grant, which needs a hosted package
- the node's address held as anchor by an unconnected user; that user reaches the sub through the patron's node — anchor entry and payload relay proven; on hardware, the "payload demo" peer with a third phone that knows only the anchor
- the node caches for an absent sub — `material_queues_for_a_client_marked_unreachable` and redelivery across a restart, proven; on hardware, airplane mode on the sub; the **failover sibling** case beside it, which has its own proven test

**C4 Two infra tiers, a light tier, traversal** — three containers and four
phones; `a_resolution_is_proxied_through_a_real_referral` is the proven
core; the `sim` crate already runs several daemons.

## D. Beyond

Recovery, rotation and departure on hardware — all with in-process tests,
none run on phones. The iOS client, which has no position until it exists.

## Order

A1 is done; a second plain pair was met deliberately (`82ffd1b-a2-adoption-1`, mislabelled) for series B. B1 nodeless hung at witness selection and showed that nothing met nodeless is reachable afterwards and no cross-pair verifier can qualify — so **B and A4 are with-node**, and the node bench (C1's proven half plus `field-run.sh`) is now the gate for nearly everything left. **[ruled, author, 2026-10-10]** witnesses are required where available and a witnessless meeting is not genesis-only; proceeding witnessless past genesis needs design §13.2 / `wire-format.md` §3.2 amended, which gates any nodeless meeting between non-genesis phones. **That amendment was made** [author, 2026-10-19] — no witness floor, design §13.2 reasoning from the same case the ruling was about — so the condition is met and the gate is lifted, and the rows above are re-planned against it below. **The node bench is up (2026-10-11, `82ffd1b-a4-node-1`), all four phones re-provisioned at genesis on it; the two met pairs are to be re-met there for B.** A4, B's with-node
half and all of C wait on a node being routine on the bench, which is C1's
proven half plus `field-run.sh`. B's nodeless half needs only a prior run's
state kept. Within C, the Catalog screen is the one build that gates a test
in the middle of a chain.

### Re-planned against the lifted gate, 2026-10-23

**What the lift actually bought.** Not a new capability — a nodeless
meeting between phones that have met others no longer has to be bought
with two identities. Before, every such run was preceded by a wipe, and
the wipe destroyed exactly what B is assembled from. That is why this
plan read as "the node bench gates nearly everything left": the cheap
setting was unusable without paying in the expensive currency. Three
things changed independently and together they settle it — the shell
stopped waiting where no witness can come (2026-10-11), the floor went
(2026-10-19), and an empty witness set is answered with an empty keep so
the prune completes rather than hangs (`sequence.rs`'s
`opening_id_fixed`, "an empty set too").

**Proven, not hoped**: CER-50 is two parties meeting at genesis and then
meeting again no longer at genesis, finalising as a witnessless normal
record. Its own comment names the field run that stopped on `NoWitness`
on 2026-10-04. So a nodeless bench run is now confirmation on hardware of
a path the suite holds, which is the right order for a bench.

**What is unblocked, in the order it should run:**

1. **A6 — a nodeless meeting between two phones that have met** — the run
   the gate used to forbid, and an **A row rather than a B one**, which is
   worth saying because it was B1 nodeless that hit the wall and the
   instinct is to re-run that. B1's *verification* purpose cannot be
   served nodeless at all: a party met nodeless has no route afterwards,
   so the verifier pool is empty by construction and no re-run changes
   that. What is newly runnable is the **ceremony variant** — does a
   meeting between non-genesis phones finalise witnessless on hardware,
   as CER-50 says it does in process. Cheapest run on the bench: no node,
   no wipe, two phones. What it should show is an empty witness set
   answered with an empty keep on both sides, `witnesses_outstanding`
   falling to nothing, a normal record with field 4 absent, and both
   screens saying plainly that nobody witnessed it — PRD-19's third case,
   which no run has reached, though for a duller reason than it sounds:
   PRD-19 postdated every run on the bench, appearing in none of the
   35 checklists that used to be generated under `runs/` — a form since
   removed, having never once been filled in [author, 2026-10-23].
2. **A2's nodeless adoption pass**, on phones that have met, as the row
   now says. Witnessless normal record carrying the adoption.
3. **Heading F's arrival spread** (A5) — **ready on the restructured
   machine** (2026-10-24): ranging is gone, the post-optical regime is one
   `WINDOW` step in which the capture's hold is turned tick by tick while
   the window opens, the queries go and the conversation runs beside it,
   and the proposal and the handover wait on the seal. `973b0e0` is the
   fallback point before either change. The field-test build is on GXV
   and MRK, installed over their state.

**What is not unblocked, and why.** A3's nodeless peering waits on the
shell, not the gate — the Meet flow's `DONE` step needs a screen. A4 is
with-node by construction, since a nominee has to be reachable and a
party met nodeless has no route. All of C needs the node. The lift moves
nothing in C or D.

**What the author still owns here.** Whether the nodeless adoption pass
should *also* be run at genesis, to prove the adoption under a formation
shape as well as under a witnessless normal one. The met-phones pass is
planned above because it is cheaper and keeps B's state; the genesis pass
proves a different record shape and costs two identities. Nobody has
asked for it, and this plan does not assume it.
