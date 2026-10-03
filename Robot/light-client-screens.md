# The light client's screens: processes, steps, and what each screen owes

Working document, 2026-09-27, for review before the interface is built. The
Android light client first (`app-requirements-notes.md` section 1). Each
process lists its steps, the screen elements each step needs, the kernel
operation beneath it (`capability-matrix.md` is the contract), and the
obligation that put the element there. **Owed** marks what the kernel does
not yet carry. The four questions this sheet opened with were ruled on
2026-09-27; the rulings are woven in and recorded at the end.

Two rules govern every screen before any is drawn:

- **Refusals render their reason** (`light-client-requirements.md` §9): every
  kernel answer is a value or a `Refused` with words in it, and the screen
  shows the words, never a silence to interpret.
- **Every notice knows which of two classes it is in**
  (`app-requirements-notes.md` section 1): *state-keyed* — rendered from a
  condition, gone when the condition is, no memory of having prompted
  (the backup nudge, the second-adoption encouragement, the operator frame);
  or *persisted-flag* — shown once, before the act, and remembered
  (PRD-08's first-brokered-use notice is the only one of this kind so far).

## A. First run and identity

| Step | Elements | Beneath | Owes |
|---|---|---|---|
| A1 The fork | Three doors: **New identity**, **Restore a backup**, **Recover** (Owed: recovery has no kernel export). Copy at the fork, not after it: an unlinked path abandons prior standing; a linked recovery gives up unlinkability — one choice seen from opposite sides | `start` / `restore_backup` / Owed | PRD-03, UX-008; `light-client-requirements.md` §6 |
| A2 New identity | No screen. Seeds minted silently, kept under the shell's own custody, never crossing the storage seam | `start(seeds, …)`; `seal("seeds")` | APP-011 |
| A3 Restore | Envelope picker; passphrase field (the key kept apart from the device); import scan report naming anything past its retention; result framed **"recovered on trust"**, never *verified-complete* | `restore_backup(blob, secret)` | PRD-07, PRD-09; ARC-24 |

## B. Home — navigation first, status peripheral [author, 2026-09-27]

| Element | Beneath | Owes |
|---|---|---|
| Status line: detached / attached, primary / degraded on a sibling / reconnecting / lost — with the failover's meaning stated, and the mailbox count explained as *queued at the node*, never as delivered/read | `status`, `Event::Connection`, `Attached.queued` | UX-012; `light-client-requirements.md` §4 |
| Presented key, and the payload construction phrased by this shell | `presented_key`, `payload_construction` | MAIL-027 |
| **Backup nudge** (state-keyed): until a backup exists, then gone | export state | PRD-10 |
| **Second adoption nudge** (state-keyed): while subnet plurality is absent and the user can still reach one freely; explains eclipse, never a wire requirement | `places`/`anchors` | PRD-04, UX-005; `light-client-requirements.md` §6 |
| **Operator frame** (state-keyed): appears iff an instance is attached (Owed: the mode query — *does this identity run an instance*) | Owed | `infra-client-requirements.md` §8.3 |
| Navigation: Conversations · Meet · People · Catalog · Settings · Operator (conditional) | | |

## C. Conversations

| Step | Elements | Beneath | Owes |
|---|---|---|---|
| C1 Thread list | Peers with held sessions; per-peer direct-path chip — **status only**: the path order is fixed and nobody chooses (design §12.6.3) | `direct_to` | |
| C2 Thread | Messages with **actual sender attribution** — never a hint styled as authenticated identity; unsent/refused states carrying their reason; a submission acknowledgement never presented as delivery | `send`, `next_event`, `Event::Payload` | UX-012 |
| C3 Late arrivals | A late verifier response or response copy labelled as what it is | `Event::Late`, `Event::ResponseCopy` | `light-client-requirements.md` §1.4 |

## D. Meet — the ceremony (design §7.1's order; both devices run it)

| Step | Elements | Beneath | Owes |
|---|---|---|---|
| D1 Intent | **Two halves, and two different cameras** [author, 2026-09-29 — see the handshake ruling below]. D1a, the initiator: a *meet someone* command, then the transaction-type choice, then a **bootstrap QR** on screen carrying the basics intent needs — this device's identifier and the type of transaction. D1b, the responder: a *scan a QR code* command reading that QR with the **rear-facing camera**. Nothing of the ceremony's own anchor crosses here | `begin`, `nominees` | design §7.1.1 |
| D1.5 The brief and the acceptance | **Both devices show the transaction and both people answer** [author, 2026-09-29]: once the bootstrap QR is scanned, each device describes what is about to happen with every necessary warning and asks its own user to accept or refuse. Acceptance is what reveals the intention QR and the turn-the-phone instruction. **Every warning and choice, front-loaded** [author, 2026-09-27]: from D2 on the device faces away from its user and takes no input, so everything a person must read or decide comes before any capture or exchange — the capture disclosure (what becomes durable, who may read it, the retention period), the disclosure-slot choices, and the adoption's authority consequences where D1 chose it. Witness cross-nomination runs beneath (each nominates from the *other's* neighbourhood — no picker for one's own) | | UX-001, UX-002; `light-client-requirements.md` §§1.5, 6 |
| D2 Optical exchange | The **intention QR**, mutual and on the **selfie cameras**, each device reading the other's off the screen it is looking at: the contributions and then the ceremony-id both compute and check (`wire-format.md` §14.3.1). Entered from the acceptance at D1.5 with an **instruction and illustration to turn the phone to face the counterparty** [author, 2026-09-29], which is the moment the device stops being the user's; retry path | shell carries; `take_intent` once the bearer has the intent | design §7.1 step 3; `wire-format.md` §14.3 |
| D3 Proximity | Channel attempt UWB → NFC; the **channel recorded is the strongest the hardware supports**; result chips; continue-with-weaker is allowed and shown as the parties' own assurance | `proximity`, `take_channels` | CER-18; design §7.6.3 |
| D4 Capture | Guided capture of the **other** person: 3–5 frames, randomised prompts ("turn slightly…"), voice or tone cues only — the disclosures were read at D1.5, the device faces away; capture keys handed and captures sealed beneath, silently | `capture_key`, `capture` | `light-client-requirements.md` §1.3 |
| D5 Verifiers | The counterparty's handed bundle rendered for **selection by recognition**: prefer people met or shared-horizon, fish for common acquaintances, fill the rest deliberately; live progress as responses land (match / no-match / inconclusive / unavailable) against `min(floor(n/2), 10, |candidates|)` | `select_verifiers`, `query_for`, `request`, `responses`, `gathered` | design §7.1 step 6 and design §8.1.2 |
| D6 Review and sign | **Each party reviews the other's selection before signing**; the pre-sign warnings: missing familiar verifiers, witness imbalance, unavailable evidence, weak proximity/integrity — a degraded ceremony presented as degraded, **never as malformed** | `review_and_sign`, `sign_body`, `finalize` | UX-003; design §7.1 step 7 |
| D7 After | Record txid, carried up; the adoption D1 chose proposed and taken; **simultaneous opposite adoptions resolved by asking the two to choose a direction**, never reported as a protocol failure | `propose_adoption`, `take_adoption`, `position_in` | PRD-05; `light-client-requirements.md` §7 |
| D∅ The passive roles | **No screen at all.** A witness or verifier is asked nothing and warned of nothing — the absence is asserted, not forgotten; at most a quiet activity log | `take_witness_ask`, `witness_sign`, `take_query`, `take_grant` | UX-002; design §19.6 |

### The meeting handshake, as the author specified it [author, 2026-09-29]

Given verbatim, because it settles a sequencing question the shell could not
settle for itself:

1. User A selects the *meet someone* command. A dialogue asks the kind of
   transaction. **"Regular meeting" is the default**, with a checkbox *"Ask
   this person to backup my user data"*, and a separate option making it a
   patronage action, which then prompts **"I will be the Patron"** or
   **"I will be the Client"**.
2. A's device displays a QR carrying the basics intent requires: A's
   identifier and the type of transaction.
3. User B activates *scan a QR code* and scans it — **the rear-facing camera,
   not the selfie cam**.
4. Both devices then describe the transaction with any necessary warnings and
   ask their respective users to accept or refuse.
5. Once a user accepts, the screen shows the **intention QR** with
   instructions and an illustration to **turn the phone to face the
   counterparty**.
6. Once both devices have the counterparty's intention QR in view of their
   respective **selfie cams**, the rest of the ceremony proceeds.

**What this settles.** The shell had `intentExchanged()` completing D1 before
the optical step, following design §7.1's numbered summary — which cannot hold
once `wire-format.md` §14.3.2's `IntentExchange` echoes an optical
contribution that D2 has not yet shown. The ruling separates the two: a
**bootstrap** QR at D1 carrying only identifier and transaction type, read
rear-camera and one-directional, and the **anchor** exchange at D2, mutual and
selfie-camera. Acceptance sits between them, which is also where the
front-loading ruling wanted every warning, and the turn-the-phone moment is
now explicit rather than implied.

**Two consequences for the state machine.** D1 completes on the bootstrap
being scanned and both descriptions being shown, not on an intent arriving;
and the transaction type is chosen at D1a by the initiator, before the
responder has seen anything, so the responder's D1.5 is the first place they
can refuse it. **Either party may be the initiator** [author, 2026-10-02]: the
shell offers both controls, and which one a user chose has no bearing on the
outcome or on the polarity of an adoption; design §7.1 says so.

**Built 2026-10-01.** `Meet.Step.INTENT` now completes on `crossBootstrap()`
and not on an intent; `Meet.Role` says which side of the bootstrap a device
is on and therefore which camera it uses; `Meet.Kind` carries the author's
dialogue — a regular meeting by default, the *"ask this person to backup my
user data"* checkbox, and patronage in a named direction. The brief's
contents and the pre-sign warnings are **inventories the model computes
from the ceremony's own facts** (`Meet.brief()`, `Meet.presign()`), so a
screen cannot show a reassurance it has not earned and a test can say which
items a given ceremony owes. **The rows these answer**: UX-001 and UX-002 at
D1.5, PRD-02 where D1a chose an adoption, UX-003 at D6, PRD-05 at D7.

The **bootstrap object is the shell's own** and deliberately not a
`wire-format.md` §14.3 one: that section fixes the *anchored* exchange, and
the bootstrap carries only
design §7.1.1's basics between two people who have not yet exchanged a
contribution. Version byte, kind, backup flag, identifier — thirty-five
bytes, and the shell reads it because there is nothing in it for the kernel
to check.

**The checkbox is peering, and peering carries the backup** — I had this
wrong on first reading and the author corrected it [2026-09-29].
`network-design.md` §6.3 rules it [author, 2026-09-04]: a peer persists the
other's data as an encrypted backup, "the one thing a peer gets that an
ordinary acquaintance does not." The author's shape, confirmed against the
documents: peering is a condition between users on the social graph; where
one or both are light clients it carries the backup function and the trust
distance the required meeting already gives; where both run instances it
carries the topology function as well. Three points confirmed [author,
2026-09-29]: seeds never leave the ceremony device (design §23.3), so a full
backup still excludes them; the shortened trust distance comes from the
meeting, not the edge (design §6.3's collapse rule); a peer is not in one's
horizon by virtue of peering (design §6.3).

**Ruled, and the checkbox has a mechanism** [author, 2026-09-29]. A
light-client peering produces a `wire-format.md` §4.4 record; its endpoint
field is a network point for an instance and a locator for a light client;
peers keep each other's endpoint current by `EndpointRecord` or
`SignedLocator` on the end-to-end channel, with no peering reissued for an
address change; adding an instance sends its endpoint to peers, and two
instances route directly. The checkbox therefore means: after the meeting's
record, propose a type-4 peering naming it in field 8, and take on the backup
obligation `light-client-requirements.md` §2 now states.

**D2 through D4 are hands-off** [author, 2026-09-27]: the device faces the
counterparty and receives no user input, which is why D1.5 front-loads every
warning and choice. Every D step has a **denial path**: camera refused,
radios refused, the counterparty walking away — each renders as a value with
a reason and a way back (UX-013 validates these by walking them).

## E. People and neighbourhood

| Step | Elements | Beneath | Owes |
|---|---|---|---|
| E1 The horizon | **Users, never devices** [author, 2026-09-27]: every endpoint of a person sits behind one displayed representation, and the infra/non-infra distinction is an icon state on the person — read from current topology, never a stored species (promotion changes it). Positions, distance, reachability; the operator-itself versus person-behind distinction stays (the `.0` presentation), rendered on the person | `places`, `distance`, `reachable`, `resolvable` | UX-014 |
| E2 Accept a patron | An in-horizon adoption offer — a non-PoP transfer — **arrives as a notification with accept/deny** [author, 2026-09-27]; the acceptance shows the upward-reaching policy list with **expanded names and counts** — a count alone is insufficient — framed as a reminder, the policy being the user's own | Owed in part (standing/trust export) | PRD-02, UX-007; `light-client-requirements.md` §6 |
| E3 End or alter authority | The computed warning: which currently used resources this revokes — transfers and disavowals included, not departure alone; a stale permission list **identified as stale**, never silently used | Owed (`depart` has no export; the access computation) | PRD-02, UX-006; `light-client-requirements.md` §6 |
| E4 Recovery | Owed whole (five client functions, no export); the identity-fork warning of A1 applies here | Owed | REC-*, UX-008 |

## F. Catalog and resources

| Step | Elements | Beneath | Owes |
|---|---|---|---|
| F1 Browse | Cached entries render **without opening a connection**; per-host *stale* and *truncated* chips; a deliberate Refresh, one query session at a time | `catalog`, then `browse` | UX-011; `light-client-requirements.md` §8 |
| F2 Entry | Owner, the signed-service binding, hosting/proxy model, endpoint kind (E1's rule) | `CatalogItem` | UX-010, UX-014 |
| F3 First brokered use | The one **persisted-flag** interstitial: the vendor's session can outlive membership; owner-signed metadata proves neither no-logging nor trustworthy conduct | shell flag | PRD-08, UX-010 |
| F4 Roles | Names as the primary view; a quantile shown with its named population and moving cutoff; absolute top-k where that is the intent; unusable services filtered, not exposed | Owed (role view has no kernel call) | UX-009 |
| F5 Register a resource | Owed (node has `CatalogService::register`; no kernel call) | Owed | `light-client-requirements.md` §8 |

## G. Settings

| Element | Beneath | Owes |
|---|---|---|
| Retention, verifier limits, seal cost | **Owed**: `Config::default()` fixed at start | `capability-matrix.md` §1 |
| Wake endpoint: the user's chosen service, handed in as URL and key, withdrawable | `wake(Some(..))`, `wake(None)` | `light-client-requirements.md` §4.1 |
| Backup export: passphrase **kept apart from the device**, the loss-cost copy, the envelope out | `export_backup(secret)` | PRD-07 |

## H. Operator (mode-keyed: exists only while an instance is attached)

| Step | Elements | Beneath | Owes |
|---|---|---|---|
| H1 Provision an instance | Provider and zone choices **ordered by concentration inside this operator's own horizon**, least first, crowded marked and never hidden; and the caveat said: the ordering speaks to one operator's redundancy, not the network's | Owed in part | PRD-11; `light-client-requirements.md` §6 |
| H2 Delegations | The run's issuance and renewal state (45 × 48 h, contiguous); what the delegated key signs and what stays on this phone | `delegate`, `presented_key` | `infra-client-requirements.md` §7 |
| H3 The node's pages | **Provisioning pages only, in a frame isolated from the client's keys, archive and sealed captures**; the administration channel awaits the author's decision | Owed | PRD-12, PRD-13; `infra-client-requirements.md` §8.3 |
| H4 Operator disclosure | What subordinates are exposed to, the hosting model, the roles users hold | Owed | PRD-06; `infra-client-requirements.md` §8 |

## I. A second device (phone and desktop, two ends of one flow)

| Step | Elements | Beneath |
|---|---|---|
| I1 Device shows its key | `presented_key`, rendered to carry (QR or file) | |
| I2 Phone issues the run | Scan/receive the key; `delegate(key, not_before, count)`; carry the run back on the shell's own channel | |
| I3 Device's bundle | `bundle_to_sign` → phone `sign_device_bundle` → device `take_signed_bundle`; then attach under the delegation | |

## What this flow exposes as owed at the kernel

From the capability matrix, confirmed by this pass: settings/config;
biometric matcher; recovery; departure; standing and evaluation; resource
registration and access; the mode query (*does this identity run an
instance*); and the mode query alone now that the path ruling landed. The
administration channel is the author's open decision.

## The rulings (all four questions, author, 2026-09-27)

- **The path is not a choice.** `light-client-requirements.md` §5's override
  requirement was struck as invalid: direct is preferred, the relay is the
  fallback, and neither is user-overridable or warned. PRD-01, UX-004,
  PAY-006 and REP-14 withdrew with it; the kernel's `set_path`/`PathPolicy`
  and the courier's person-switches were removed; `direct_to` stays as
  status.
- **Adoption is chosen at D1** (meet alone, or meet-and-adopt with its
  direction), and an in-horizon offer — a non-PoP transfer — arrives as a
  notification with accept/deny (E2).
- **Everything a person reads or decides is front-loaded at D1.5**, after
  the intent choice and before any capture or exchange: from D2 on the
  device faces away and takes no input. D2–D4 are hands-off; D5–D7 are
  back in hand.
- **Home is navigation first**, status on the periphery.
- **The horizon shows users, never devices**: one representation per
  person, all endpoints behind it, infra/non-infra an icon state read from
  current topology.

## The conversation on the courier: what the shell calls next (2026-10-02)

The kernel now drives the ceremony's conversation over the payload path
(`crates/client/src/sequence.rs`); the shell's VERIFIERS, REVIEW and DONE
steps still run the old in-process path and need rewiring. The FFI offers
`converseOpen()`, `converseQueries()`, `converseGathered()`,
`conversePropose()`, `progress()` and `Event.Conversed { from, what, record }`.
Suggested wiring: on entering VERIFIERS call `converseOpen` then
`converseQueries`; when `progress().queriesOutstanding == 0` the responder
calls `converseGathered` and the initiator `conversePropose`, retrying the
latter while it refuses with *waiting*; REVIEW shows the body when the
`Event.Conversed` for kind 16 arrives and signs through the existing call;
DONE on the event that carries `record`. No timer runs in the kernel: when
to propose is the shell's, and a witness that has not answered by then is
left out.

