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
- **The gutter: none on the optical exchange, very narrow everywhere else**
  [author, 2026-10-27]. D2 gives the symbol every pixel it can have, which
  is a decision about what that screen is for and not an oversight; every
  other screen carries a narrow margin so its lines do not run to the glass.
  **Fixed 2026-10-27**: a screen that set its padding and
  `fitsSystemWindows` on the same view had that padding *replaced* by the
  window insets, so seven screens lost their margin by accident while D2,
  the one screen that should have none, had the largest in the
  application. **And the number landed 2026-10-29**, the drawing's own
  rather than a guess at what narrow means: the artboards set 14 against a
  390-wide phone, so `Screen.GUTTER_DP` is 14. It stood at zero while
  nothing was drawn to the design, which read as text flush to the glass
  the moment a screen was dark.

## The shell: a rail, a foot, and one content area [author, 2026-10-27]

**The core graphical design is a derivative of common chat applications,
especially Discord and Slack**, with Discord's landing page as the
template. That fixes the navigation, and the sections below become screens
*within* it rather than destinations of their own.

| Part | What it holds |
|---|---|
| **The rail**, down the left | Direct messages and the proof-of-presence interface, pinned at the top; below them a scrolling selection of the subnets this person has joined. **Each carries its unseen count** at its upper right, direct messages included [author, 2026-10-27] |
| **The foot**, across the bottom | This person's icon beside a horizontal bar carrying their alias. The icon goes to **Options** (below), the bar **goes home**, which is where this departs from Discord |
| **The content**, everything else | One of the three views below, or a thread |

**Home view** is the landing state on loading and on returning by the alias
bar: a reverse-chronological scroll of high-priority notices from every
joined subnet and every resource in them. **Each says where it is from, in
full** [author, 2026-10-27]:

> *[patron alias]*'s *[subnet alias]* *[resource name]*: *[notice type]*

**A board thread is the same line with the thread in place of the
resource** [author, 2026-10-27]:

> *[patron alias]*'s *[subnet alias]*: *[thread title]*
> *[most recent poster's alias]*: *[the start of their message]*…

**That form is the point rather than decoration.** With the core
applications distributed, many subnets will carry resources of
substantially similar name and description, so the person has to know which
context they are seeing one in; the subnet's icon beside the row is the
glance, and the line is the whole of it. A notice with nothing behind it in
a resource or a thread — an adoption offer — drops that one segment.

**And what is never a notice** [author, 2026-10-27], because the client
does it without asking anybody:

| Not a notice | What happens instead |
|---|---|
| Being nominated to witness, and attesting | The client answers in the background. Consent to a verifier's query is already bound to the ceremony the person is standing in (D5), so the ceremony screen tells them rather than asking |
| Answering as somebody else's verifier | The same: a background act of this client, reported where the ceremony reports it and nowhere else |
| An instance wanting its run renewed | The client delivers a fresh batch the moment it hears, with no screen involved. The instance still tells its operator before the last credential (`infra-client-requirements.md` §7) and this client is still the only thing that can answer (`light-client-requirements.md` §10); what that section does **not** ask for is a person in the loop |

**The adoption offer stays**, and it is the one exception the author named:
an **in-horizon transfer**, not a meeting. E2 below already rules that one
arrives as a notification with accept and deny [author, 2026-09-27]. Every
adoption that is a meeting happens at the meeting and raises nothing here.

**A subnet is the whole hierarchy descending from one root**, with its own
name and its own id, and **the trust horizon is the local part of one**
[author, 2026-10-27] — the thing loosely called *the local subnet* in
conversation. The design already says exactly this: a subnet is named by a
record, its id is its root's scope id (design §13.8), and its alias and
logo are signed by the root (design §12.8). **So nothing on a board ever
comes from another subnet**: a post from four tiers up is the same subnet
seen from further away, which is what the scope count re-expresses.

**What this person is in a subnet is drawn around its icon** [author,
2026-10-27], in the network's own crown — hierarchic trees branching from
their apexes, which is what the mark is made of:

| Around the icon | What it says |
|---|---|
| A thin **silver circlet** | At least one subordinate here |
| A **golden circlet** | An instance of this person's runs here |
| A **full pointed crown** | This person is the subnet's root |

**The orb flowers out of its crown** [author, 2026-10-27], which is the
whole of how the two are proportioned. The subnet's mark is a **circle
filling the rail**, a point clear of each edge; **the crown is far smaller**
and sits on the orb's lower face, its band about three fifths of the orb's
width. So the orb is not encircled: the crown is a cup at its foot.

**The circlets are plain bands**, silver or gold, nothing on them. **Only
the full crown carries points**, and they are **plain triangles** as the
mark's own are, leaning with the ellipse they stand on. **Of its eight,
only the three at the front are visible**; the rest, and the back of the
band, are behind the orb's body and never drawn to the screen.

**The three standings overlap** — a root usually has subordinates — and the
drawing shows the highest that applies, root before operator before
subordinates, **which is an assumption and not a ruling**.

**The rail is a russet field** [author, 2026-10-27], the ground the mark is
presented on, so the crowns read as gold and silver on their own colour
rather than on a neutral dark.

**Two things the orb's size settles by itself**, there being no gap left
either side of it: the subnet in view is marked by a ring drawn inside its
own edge rather than by a pip beside it, and an unseen count sits inside
the orb's upper right rather than overlapping its edge.

**Resources carry their own unseen counts** in the subnet view, one per
block, as the rail's icons do.

**The container is built, 2026-10-28** (`ShellActivity`, with [`Chrome`]
deciding and [`Palette`] colouring): one screen, the russet rail down the
left and the bar along the foot, the content between them changing, and
the places that take the whole screen hiding both. **It is reached from
Home while it is partial** rather than being the launcher, since making
it the way in before it can draw the content views would take screens
away; one line moves it when they land. **The platform's chrome is
gone** with it — no title bar saying *Fueros* above the design, and the
system bars in the shell's own colours rather than a light grey bar under
a dark foot.

**All three crowns are real since 2026-10-28** (`subnet_standing`), and
**all three come off records this key already holds** [author,
2026-10-28], each off the one that settles it:

| Crown | What it is read from |
|---|---|
| **Full** | This key is the **patron named in the subnet's anchoring record** — a subnet's id being the txid of the adoption that first gave its root a subordinate (design §13.8), which the root holds, having signed it |
| **Gold** | This key published an **endpoint record on the line it holds in that subnet**. The record names a node and its addresses and no subnet (`wire-format.md` §7.6), so the series it is signed on is what ties it to one; **the instance is a device of this identity** under a delegation, not a key of its own, so the record is this key's |
| **Silver** | An **adoption this key granted** into that subnet that nothing has since ended |

A line counts until something says it ended, rather than counting only
where the propagated copy says it is open: the adoptions are this key's
own and a patron should not be left standing for nothing for want of
news about them.

**What the rail still cannot draw**, named on the screen rather than
faked: the unseen counts, nothing tracking read state anywhere, and the
content itself, which is the next piece. The orbs, the crowns, the foot
and the chrome rule are real.

**Subnet view** is what a subnet's icon opens: content from that subnet
alone, the content area split between navigation buttons to that subnet's
resources and a scrolling view of its chat — **a BBS-style single board**,
one board per subnet, with message propagation along subnet-hierarchical
scope.

**Every post says how far it travels** [author, 2026-10-27], in one of four
forms:

| Scope | Where it goes |
|---|---|
| *this subnet* | Here and nowhere else |
| *and below* | Here and every subnet beneath this one |
| *and above* | Here and up the line of patronage |
| *+3/-5* | A count each way: three levels up and five levels down |

**The default is `+2/-2`** [author, 2026-10-27], and the selector adds or
removes a level on either side. **The thread then carries the original
post's scope through every reply**, so a reply screen states the scope
rather than offering it.

**And the count is read from wherever the reader stands** [author,
2026-10-27]. A post written `+3/-3` is `+2/-4` to the writer's patron and
`+4/-2` to their subordinates: a step up takes one off the top and adds one
to the bottom, a step down does the reverse. **The band of subnets it
reaches never moves; only the coordinates do**, which is why two people
quoting one thread read different numbers and are both right. The named
forms are the ones with no number to move: *this subnet* is `+0/-0` and
stays it, and *and below* reaches a whole subtree from wherever it is read.

**The board is this shell's own, not an application** [author,
2026-10-28], which reverses what this sheet said while the question was
open. It is built over payload messaging, **with distant nodes relaying so
that one post fans out without a bound inside one subnet**. And it is not
in the protocol, for a reason that decides where it can live: a board
persists data that is not transaction records, and the protocol's
persistence is archives of those. **A message type may be added to the
protocol for it**; the board itself cannot be.

**The arithmetic is two separate jobs, and conflating them was wrong**
[author, 2026-10-28]:

- **The scope as displayed is the board application's**, and it is
  per reader rather than per post, **because one node may serve users at
  several layers of the hierarchy** — so the same post needs a different
  tag for each of them and no single stamp at the node could be right for
  all. The client renders the scope in the reader's own coordinates,
  offers the selector on a new thread, and states the inherited one on a
  reply.
- **Whether to forward is a separate decision**, and it belongs to
  **either the board application or the node kernel** [open, the
  author's]. It is not a hop count run down blindly: the test is whether
  **further nodes are needed to reach recipients in range of that
  message**. A node serving readers at three layers may satisfy a post
  from its own users and forward nothing, where the same post at a node
  serving one layer has to go on.

**Four things the ruling leaves open**, each the author's — the first
named by him, the rest asked in the output of 2026-10-28:

0. **Where the forwarding decision sits**, between the board application
   and the node kernel. What it has to answer either way is whether any
   recipient in range is beyond the nodes already reached.

1. **What protects a post.** Payload today is a per-device ratchet sealed
   pairwise (design §12.6.5, §23.3), and a poster cannot hold a session
   with every member of a subnet — it does not know the membership, the
   horizon being two edges of it. So a relayed post cannot be that
   object. Signed and readable within the subnet is the shape a BBS
   implies, and it makes a node a holder of content where payload makes
   it a carrier of ciphertext: design §18.1's seized node would hold the
   board.
2. **Where the scrollback lives**, and how a client that was away gets
   what it missed. A node-held log per subnet, fetched as a mailbox is,
   is the reading that makes a board a board rather than a chat nobody
   can scroll.
3. **How long a node keeps one**, and whether a reader's own copy outlives
   the node's.

**Navigating into a subnet is the only way to its resources** [author,
2026-10-27], for the reason the notice line carries its context: a resource
named out of its subnet is a name that several subnets will share. So the
catalog is not a place a person goes. The sweep that fills it is still the
client's (F), and what it returns is sorted into the subnet each entry
belongs to.

**Direct view** is what the direct-messages icon opens: a reverse
chronological list of recent chat activity. **It is also the contacts
list** [author, 2026-10-27], with a search bar at its top that finds a
contact by alias or by hash — so everyone this device holds is reachable
from here whether or not anything has been said, and opening one is how a
thread starts. No separate roll and no separate compose control.

**One scanner, and what the code was decides** [author, 2026-10-27].
*Scan* is the first option on the meeting screen, and it reads whatever is
put in front of it: **a locator is kept** and the screen says *[alias]
saved to contacts*; **an invitation goes on into the meeting flow**. The
kernel already parses which it is rather than being told (`take_intake`,
E1), so neither the person nor the screen has to know in advance. The name
in that confirmation is the keyhash until the party publishes one, by
`light-client-requirements.md` §4.3's rule that no name is shown before it
is received.

**Options** is what the icon in the foot opens, and it is the home of
everything that is about this person or this device rather than about a
subnet [author, 2026-10-27]: **the alias and the profile image**, **the
client's settings**, **backup and recovery**, and **the instances this
operator runs**. It takes the whole screen with a back control, as a thread
does, the foot being what it would otherwise sit on top of.

**Options holds only what pertains to every context this person exists in**
[author, 2026-10-27]. Anything about one subnet belongs to that subnet's
own options, below.

**A default name, made at creation, with a per-subnet override where one is
wanted** [author, 2026-10-27]. The wire is unchanged — a client publishes
an alias and an image **per subnet it holds a position in**
(`light-client-requirements.md` §4.4, UX-015) — and the default is what
goes to each of them unless that subnet has an override. So Options carries
one name and picture at the top, the list of overrides beneath it, and
**the bar along the foot carries the default**.

**Subnet options** are reached by **selecting the subnet's name at the top
of its own view** [author, 2026-10-27], and hold what is true of that
subnet alone:

| Who sees it | What is there |
|---|---|
| Everybody | Notices from this subnet, activated or suppressed |
| Everybody | **The patronage upline from here**, scrollable: this person, their patron, and so on to the root |
| A patron here | What is open to a patron, among it **the count of their downline: the direct lines they have issued here and no deeper figure** [author, 2026-10-28] — a patron signed every adoption it issued, so that count is exact, where a subtree count would be only as far as the propagated copy reaches and would read low for anyone with depth below them. **No cost of ending one is shown**, that act being a disavowal and mintable only by a node holding its own seed |
| An operator here | A way to their instance's administration pages, which the operator's own section reaches too |
| **The root** | **The subnet's name becomes editable**, its logo with it — the root of the hierarchy the subnet *is*, which `light-client-requirements.md` §4.4 makes the root's by convention while nothing enforces it, so the screen says so rather than presenting it as a rule |

**A resource takes the screen** [author, 2026-10-27], entered from its
subnet's view or from a notice. The rail and the foot drop away as they do
for a thread, leaving the system's own navigation, a title bar with a back
control, and **the rest for the resource to fill**. The title bar carries
the context the notice line carries, since two subnets will hold resources
of the same name. **What the resource has not styled for itself, the
application styles to match**: background, type and colour as everywhere
else, so an untagged page does not arrive looking like a different
application.

**What a resource's page may do is owed, and is not a styling question.**
The node's own administration pages are already framed with no script, no
bridge, no file or content access, no storage, and navigation confined to
that one origin (H3a, PRD-12). A resource is somebody else's code by the
same argument, and a frame for it wants the same list settled once rather
than per application.

**A thread takes the whole screen.** Selecting a direct thread or a board
discussion hides the rail and the foot, so the screen is the conversation,
with a back control in the title header returning to the view it came from.

**Dark by default, light an option** [author, 2026-10-27], under
Appearance in Options. The colour scheme and the type may both be revisited
later; what is fixed is that the dark one is what a person meets first and
that the light one is theirs to choose, not the platform's to impose.

**Nothing in the shell is open** but one thing the drawing settled on its
own: which standing wins where the crowns overlap. The launcher icon is
still the platform's default and is the other place the mark belongs.

---

## A. First run and identity

| Step | Elements | Beneath | Owes |
|---|---|---|---|
| A1 The fork | Three doors: **New identity**, **Restore a backup**, **Recover**. Copy at the fork, not after it: an unlinked path abandons prior standing; a linked recovery gives up unlinkability — one choice seen from opposite sides. **The third door asks for the prior seed and, optionally, the backup** [2026-10-27]: the seed because `wire-format.md` §4.1 admits no variant of the `Recovery` block, the backup because `light-client-requirements.md` §6 has recovery restore standing and not history, so a person who kept neither is at the first door rather than the third | `start` / `restore_backup` / `recovering` | PRD-03, UX-008; `light-client-requirements.md` §6 |
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
| **The status line is the way to the connection** [2026-10-27]: tapping it opens the attach screen — the nodes whose endpoint records this device holds, or a node's key and the address it answers at, and a detach. Built because there was no other way in: the only attach was the one the kernel performs at a cold start from the bench's provision blob, so an installed app could meet whoever was in front of it and could never reach a node, a verifier beyond the people present, a mailbox or a catalog. Not a seventh navigation row, the six above being the author's | `reachable`, `attach`, `detach` | `light-client-requirements.md` §4 |

## C. Conversations

| Step | Elements | Beneath | Owes |
|---|---|---|---|
| C1 Thread list | Peers with held sessions; per-peer direct-path chip — **status only**: the path order is fixed and nobody chooses (design §12.6.3) | `direct_to` | |
| C2 Thread | Messages with **actual sender attribution** — never a hint styled as authenticated identity; unsent/refused states carrying their reason; a submission acknowledgement never presented as delivery | `send`, `next_event`, `Event::Payload` | UX-012 |
| C3 Late arrivals | A late verifier response or response copy labelled as what it is | `Event::Late`, `Event::ResponseCopy` | `light-client-requirements.md` §1.4 |

## D. Meet — the ceremony (design §7.1's order; both devices run it)

| Step | Elements | Beneath | Owes |
|---|---|---|---|
| D1 Intent | **Two halves, and two different cameras** [author, 2026-09-29 — see the handshake ruling below]. D1a, the initiator: a *meet someone* command, then the transaction-type choice, then a **bootstrap QR** on screen carrying the basics intent needs — this device's identifier and the type of transaction, and since 2026-10-15 **which subnet** the adoption is into and the inviter's alias and the subnet's as **the inviter's claim** (`Bootstrap.kt`, version 2; version 1 still reads). **The scope question is its own screen, in two versions** [author, 2026-10-15]: *Adopt into which scope?* for the would-be patron (which of its subnets, a new one under its own key among them) and *Submit as which scope?* for the would-be subordinate (which scope it roots it brings, or none, joining as an individual) — shown for the initiator between the choice of transaction and the invitation, and for the party that read it between the scan and the acceptance; a device with nothing to choose never sees it (`Meet.pendingChoice`). D1b, the responder: a *scan a QR code* command reading that QR with the **rear-facing camera**. Nothing of the ceremony's own anchor crosses here | `begin`, `nominees` | design §7.1.1 |
| D1.5 The brief and the acceptance | **Both devices show the transaction and both people answer** [author, 2026-09-29]: once the bootstrap QR is scanned, each device describes what is about to happen with every necessary warning and asks its own user to accept or refuse. Acceptance is what reveals the intention QR and the turn-the-phone instruction. **Every warning and choice, front-loaded** [author, 2026-09-27]: from D2 on the device faces away from its user and takes no input, so everything a person must read or decide comes before any capture or exchange — the capture disclosure (what becomes durable, who may read it, the retention period), the disclosure-slot choices, and the adoption's authority consequences where D1 chose it. **The brief reads the settled direction and not this device's own choice** [bench, 2026-10-15]: the kind is the initiator's and the responder reads it off the bootstrap, so a responder briefed from its own D1a was shown a regular meeting while the other phone proposed to adopt it. Where both asked to be the patron the brief says so and names both consequences, the question being D7's to settle (PRD-05). **The acceptance states which subnet** [author, 2026-10-15]: the party being adopted is told "*[patron alias]*'s *[subnet alias]*" from the invitation's claim, and that it is a claim until the signed profile replaces it (UX-016); the responder as patron chooses which of its subnets at its brief, the first place it can, and nothing is accepted before. Witness cross-nomination runs beneath (each nominates from the *other's* neighbourhood — no picker for one's own) | | UX-001, UX-002; `light-client-requirements.md` §§1.5, 6 |
| D2 Optical exchange | The **intention QR**, mutual and on the **selfie cameras**, each device reading the other's off the screen it is looking at: the contributions and then the ceremony-id both compute and check (`wire-format.md` §14.3.1). Entered from the acceptance at D1.5 with an **instruction and illustration to turn the phone to face the counterparty** [author, 2026-09-29], which is the moment the device stops being the user's; retry path. **No margin here** [author, 2026-10-27]: the symbol and the band that reads theirs take the full width, which is what this screen is for | shell carries; `take_intent` once the bearer has the intent | design §7.1 step 3; `wire-format.md` §14.3 |
| D3 Proximity | Channel attempt UWB → NFC; the **channel recorded is the strongest the hardware supports**; result chips; continue-with-weaker is allowed and shown as the parties' own assurance | `proximity`, `take_channels` | CER-18; design §7.6.3 |
| D4 Capture | Guided capture of the **other** person: 3–5 frames, randomised prompts ("turn slightly…"), voice or tone cues only — the disclosures were read at D1.5, the device faces away; capture keys handed and captures sealed beneath, silently | `capture_key`, `capture` | `light-client-requirements.md` §1.3 |
| D5 Verifiers | The counterparty's handed bundle rendered for **selection by recognition**: prefer people met or shared-horizon, fish for common acquaintances, fill the rest deliberately; live progress as responses land (match / no-match / inconclusive / unavailable) against `min(floor(n/2), 10, |candidates|)` | `converse_open`, `converse_queries`, `progress`, `Event::Answered`; `select_verifiers` for the rows | design §7.1 step 6 and design §8.1.2 |
| D6 Review and sign | **Each party reviews the other's selection before signing**; the pre-sign warnings: missing familiar verifiers, witness imbalance, unavailable evidence, weak proximity/integrity — a degraded ceremony presented as degraded, **never as malformed** | `converse_gathered`, `converse_propose`, `Event::Conversed`; the kernel's `review_and_sign` runs as the body arrives | UX-003; design §7.1 step 7 |
| D7 After | Record txid, carried up; the adoption D1 chose proposed and taken — **wired 2026-10-15**: the responder states the direction, the party being adopted offers a series, the patron proposes, the other reviews and signs or refuses with its reason, the patron assembles and both take the record, and the screen shows where each now sits; **simultaneous opposite adoptions resolved by asking the two to choose a direction**, never reported as a protocol failure, with *"neither"* a real answer | `propose_adoption`, `review_and_sign_adoption`, `adoption_envelope`, `take_adoption`, `fresh_series`, `anchor`, `position_in` | PRD-05, CER-57 to CER-62; `light-client-requirements.md` §7 |
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
| E1 The horizon | **Users, never devices** [author, 2026-09-27]: every endpoint of a person sits behind one displayed representation, and the infra/non-infra distinction is an icon state on the person — read from current topology, never a stored species (promotion changes it). Positions, distance, reachability; the operator-itself versus person-behind distinction stays (the `.0` presentation), rendered on the person | `places`, `distance`, `reachable`, `resolvable` | UX-014. **And a question of 2026-10-29, the author's**: the new navigation gives this no home. The subnet's own options draw the patronage upline and the count below; the Direct screen draws the people you hold, searchable. Between them they may be the whole of it — in which case E1 is not owed a screen but satisfied by two — or a view of the neighbourhood as a shape is still wanted somewhere |
| E2 Accept a patron | An in-horizon adoption offer — a non-PoP transfer — **arrives as a notification with accept/deny** [author, 2026-09-27]; the acceptance shows the upward-reaching policy list with **expanded names and counts** — a count alone is insufficient — framed as a reminder, the policy being the user's own | `standing`, `evaluate` [2026-10-27] | PRD-02, UX-007; `light-client-requirements.md` §6 |
| E3 End or alter authority | The computed warning: which currently used resources this revokes — transfers and disavowals included, not departure alone; a stale permission list **identified as stale**, never silently used | **Built, 2026-10-28** (`LeavingActivity`, behind a row on the harness's People screen — **which has no successor** [author, 2026-10-29]: its scanner and its own-code button are the meeting screen's, its held contacts are the Direct screen's, so this row moves to the subnet upline where the artboard already puts it): the lines this key holds, the cost of ending one on tapping it, and the departure behind two taps. `relationships`, `depart` [2026-10-27]; `cost_of_leaving(patron)` [2026-10-28], which is the warning's content: what goes, what stays, who leaves the horizon, and in `limits` every reason the list is a floor — a predicate no client holds, a stale or truncated portion, a node in the horizon never swept. **The empty list is two answers and the screen says which**: a cost of nothing leads with *nothing this device can see goes with it*, a cost that could not be computed with *cannot be worked out on this device*, and the limits print either way. **The act is not gated on the cost having been read** — leaving is a right (design §6.2) — and asks twice because it cannot be undone. **A transfer is the same call named on the former patron**, the adoption that carries it ending that line; a disavowal is minted by a node holding its own seed (`rhtn-node`'s `disavow`), so it is the operator's surface and not this one. **The patron's own side carries no cost at all** [author, 2026-10-28]: a subnet's options show the count of your downline there and nothing about what ending one would revoke. **A party is named by their published alias and their keyhash together** [author, 2026-10-28] — both, an alias being what somebody signed about themselves and the act not being undoable; the keyhash alone where no alias has reached this device. **Drawn on the canvas too** (`NewLeave`, reached by tapping your patron in the subnet's own upline), with the three answers on a tweak | PRD-02, UX-006; `light-client-requirements.md` §6 |
| E4 Recovery | Whole across the boundary [2026-10-27]: entered at A1's third door, driven from either side; the identity-fork warning of A1 applies here | `recovering`, `recovery_query`, `recognise`, `prior_key`, `take_recovery_response`, `recovery_block`, `seal_old_lines`, `present_chain` | REC-*, UX-008 |

## F. Catalog and resources

**This is no longer a destination** [author, 2026-10-27]. Under the shell
above, a resource is reached by navigating into the subnet it belongs to
and nowhere else, so what this section describes is where the sweep's
results are *shown* — the resource strip at the top of a subnet view — and
no longer a screen of its own. F1 to F3 below describe the browse as it was
built; F4 and F5 are still owed and still have no kernel call behind them.

**Built, 2026-10-16** (`CatalogActivity`, `Catalog`): the sweep
(`Client::browse`), what it holds with each entry's type, instance, owner
and serving node, **a truncated or stale portion said rather than
smoothed over** (`wire-format.md` §6.4), and one request to a resource
through the node that served its entry (`wire-format.md` §11.1) with the six statuses in
words. The request is the simplest HTTP/1.1 there is; an application's own
screen is the resource's business. **And since 2026-10-28 a resource's
answer is read rather than quoted**: the kernel parses the one HTTP
message, the frame policy of F6 below decides what may be shown of it,
and the catalog's own line is the diagnostic beside that. Owed: the
grant-side cases (a sub reaching one under a grant), which need a hosted
package on the bench.

| Step | Elements | Beneath | Owes |
|---|---|---|---|
| F1 Browse | Cached entries render **without opening a connection**; per-host *stale* and *truncated* chips; a deliberate Refresh, one query session at a time | `catalog`, then `browse` | UX-011; `light-client-requirements.md` §8 |
| F2 Entry | Owner, the signed-service binding, hosting/proxy model, endpoint kind (E1's rule) | `CatalogItem` | UX-010, UX-014 |
| F3 First brokered use | The one **persisted-flag** interstitial: the vendor's session can outlive membership; owner-signed metadata proves neither no-logging nor trustworthy conduct | shell flag | PRD-08, UX-010 |
| F4 Roles | Names as the primary view; a quantile shown with its named population and moving cutoff; absolute top-k where that is the intent; unusable services filtered, not exposed | Owed (role view has no kernel call) | UX-009 |
| **F6 A resource's page** | **Built, 2026-10-28** (`ResourcePage` holds the policy, `ResourceActivity` the frame). The declared type decides: `text/html` is framed, `text/plain` is drawn as text and takes no frame, everything else and everything undeclared is named with its size and not rendered — **nothing is sniffed**. The frame reaches nothing: no script, no bridge, no file or content access, no storage past the screen, no fetch, no navigation, and a null base so the document is same-origin with nothing. The app's sheet goes beneath the page's own and the markup is never rewritten | `request_resource` [2026-10-16], whose answer the kernel now reads: the HTTP message is parsed in `rhtn-client`'s `resource`, so no shell is the second parser `wire-format.md` §11.2 forbids on the node's side | CAT-023..025, RSC-56..58; `light-client-requirements.md` §8 |
| F5 Register a resource | Owed (node has `CatalogService::register`; no kernel call) | Owed | `light-client-requirements.md` §8 |

## G. Settings

**These are Options' second group** [author, 2026-10-27], reached by the
icon in the foot rather than by a destination of their own. The elements
and what each owes are unchanged.

| Element | Beneath | Owes |
|---|---|---|
| **Appearance: dark or light** [author, 2026-10-27], dark being what a person meets first | **Built, 2026-10-28** (`Palette`), and **every screen wears it since 2026-10-29** (`@style/Fueros`): the title bar the design replaces is gone, the system bars continue the screen, and a `TextView` that sets no colour draws in the shell's ink rather than the platform's 4.0:1. A theme is what restyles thirteen Activities without editing thirteen Activities, and what keeps a fourteenth from being written in the platform's defaults by accident; `PaletteTest` ties its colours to `Palette`'s so neither drifts. The dark set is sampled from the artboards; **the light set is derived rather than drawn** and wants redrawing. **Owed**: the switch itself, which is an Options row, and a second theme for it | |
| Retention: the years this device declares at its intent, stated in the brief | `retention_years`, `set_retention_years` (refused during a ceremony); the field-test build takes it from the provision | design §7.5.1 |
| Verifier limits, seal cost | **Owed**: `Config::default()` fixed at start | `capability-matrix.md` §1 |
| Wake endpoint: the user's chosen service, handed in as URL and key, withdrawable | `wake(Some(..))`, `wake(None)` | `light-client-requirements.md` §4.1 |
| Backup export: passphrase **kept apart from the device**, the loss-cost copy, the envelope out | `export_backup(secret)` | PRD-07 |

## H. Operator (mode-keyed: exists only while an instance is attached)

| Step | Elements | Beneath | Owes |
|---|---|---|---|
| H1 Provision an instance | **Built, 2026-10-25, the DigitalOcean shape first** (`Launch`, `OperatorActivity`'s launch section): a provider and its credential — a token the operator makes at the provider's own page, which the screen opens with the steps shown beside it (2026-10-25; an OAuth flow was built and withdrawn the same day, nothing registered to the author ships) — a region and a size, the token minted; **ran live 2026-10-25** (droplet 612585146, nyc1, enrolled from GXV; `Robot/field-test-sequence.md` C1), the machine image asked of the provider rather than named, the token field masked and the cloud-init script composed here, the machine polled for its address, the surface polled until it answers, then H3's enrolment unchanged; the registry overridable. **Hetzner is the second, built 2026-10-26** (`rhtn_launch::Hetzner`, AS24940, read off its published spec rather than guessed): a token is a project's and carries Read & Write rather than scopes, memory comes in gigabytes, a price as a string of euros per location, an image's architecture as a field rather than a suffix, and a server type listed where it has been withdrawn — each absorbed in the adapter, with the screen now choosing between providers, least crowded first. Not yet run live. Owed: Vultr, Njalla and BitLaunch on the same shape, AWS and GCP under their signing. Provider and zone choices **ordered by concentration inside this operator's own horizon**, least first, crowded marked and never hidden; and the caveat said: the ordering speaks to one operator's redundancy, not the network's. **The kernel half is built (2026-10-16)**: `Horizon::asns_seen` reads the ASNs of every peering this client holds and `Client::providers_by_concentration` orders a caller's candidates least-first, hiding none and scoring none. **Owed**: the screen, and the list of candidates to order — nothing supplies providers and zones yet, the cloud flow being "later and larger" (`Robot/field-test-sequence.md` C1). **No crowding threshold is set**: the count is reported and the screen says it, since what counts as crowded is nobody's figure yet | Owed in part | PRD-11; `light-client-requirements.md` §6 |
| H2 Delegations | The run's issuance and renewal state (45 × 48 h, contiguous); what the delegated key signs and what stays on this phone. **A renewal is not asked for and not announced** [author, 2026-10-27]: the client signs the next batch from where the held run ends as soon as the instance says it is near the last, so this screen reports what happened rather than prompting for it | `delegate`, `presented_key` | `infra-client-requirements.md` §7 |
| H3 Enrol an instance | **Built, 2026-10-10** (`OperatorActivity`): the address and token out of band, the fetch, **the proof checked before anything is signed**, then `infra-client-requirements.md` §4.4's two records and the run, each step narrated. **Several instances, 2026-10-15** [author]: the screen lists every instance this operator administers under the subnet each serves, chosen at enrolment where the operator holds positions in several (`light-client-requirements.md` §10), and infers none from a position; the list is the shell's own state. **Owed**: the two records and the run are signed over the position this device acts in, not the chosen subnet's — a kernel `endpoint_record_in(subnet)` is wanted before an operator in two subnets enrols in the second | `enrolment_proof_checks`, `network_point`, `endpoint_record`, `anchor_entry`, `delegate` | PRD-13; `infra-client-requirements.md` §8.2, §8.3, §4.4 |
| H3a The node's own pages | **Built, 2026-10-10** (`NodePageActivity`): the node serves them and this frames them — no script, no bridge, no file or content access, no storage, and navigation confined to the instance's own origin | | PRD-12; `infra-client-requirements.md` §8.3; design §18.1, §23.3 |
| H4 Operator disclosure | What subordinates are exposed to, the hosting model, the roles users hold | Owed | PRD-06; `infra-client-requirements.md` §8 |

## I. A second device (phone and desktop, two ends of one flow)

| Step | Elements | Beneath |
|---|---|---|
| I1 Device shows its key | `presented_key`, rendered to carry (QR or file) | |
| I2 Phone issues the run | Scan/receive the key; `delegate(key, not_before, count)`; carry the run back on the shell's own channel | |
| I3 Device's bundle | `bundle_to_sign` → phone `sign_device_bundle` → device `take_signed_bundle`; then attach under the delegation | |

## What this flow exposes as owed at the kernel

From the capability matrix, confirmed by this pass: settings and config
other than the retention, which is settable since 2026-10-03; the biometric
matcher; resource registration and the role view; and the mode query
(*does this identity run an instance*). **Recovery, departure, standing and
evaluation crossed on 2026-10-27**, entering a recovery with them, and what
leaving costs on 2026-10-28 — so **nothing in section E is owed at the
kernel or the shell any more**, E3's screen having followed its
computation the same day.

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

## The conversation on the courier: what the shell calls (2026-10-02, wired 2026-10-03)

The kernel drives the ceremony's conversation over the payload path
(`crates/client/src/sequence.rs`), and the Android shell's VERIFIERS,
REVIEW and DONE steps now run on it; the in-process `selectVerifiers` and
`queryFor` path and the review screen's walkthrough sign control are gone.
`Meet` stays pure Kotlin: the kernel reaches it through a `Meet.Courier`
(open, queries, gathered, propose, progress), which `Kernel` implements over
the binding and `MeetConversationTest` with a fake.

**What each step calls.**

- **VERIFIERS.** As the captures seal (`Kernel.drainBearer`, the capture
  phase), `startConversation` runs off the bearer's thread: `nominees()`
  and `selectVerifiers()` for the rows the screen shows, then
  `converseOpen()` and `converseQueries()`. The second selection is for
  display only and agrees with the kernel's own, since the selection is a
  sort and a fill with nothing random in it (`selection.rs`); its cost is
  a second `cer.select` event and, where it fires, a second
  `NoCandidateRecognised` notice. The kernel's event loop polls
  `progress()` every tick (the 2 s `nextEvent`) and after every event while
  the flow is open. With `queriesOutstanding == 0`, **or the person having
  said to go on, or sixty seconds having passed since the queries went out
  or the last answer landed** (`Meet.PATIENCE_MS`; the kernel runs no
  timer, and the design asks for none: a verifier that does not answer
  within the ceremony does not appear, design §7.1), the responder calls
  `converseGathered()` once; the initiator calls `conversePropose()`,
  retried next tick while it refuses `Waiting(...)` (the wait shown and
  said once per reason), and a missing witness is surfaced to the person
  and gates nothing [author, 2026-10-19: there is no floor] — the body
  goes out witnessless and the record says so permanently. Any other refusal stops the meeting
  with the kernel's code. Verdicts land as before through
  `Event::Answered`; witness answers show from `progress().attesting` and
  `.declined` against the nominee lists, a declined witness shown as
  declined.
- **REVIEW.** Entered on `progress().proposed`: at the initiator when
  `conversePropose()` went; at the responder when the body (kind 16) has
  arrived and the kernel has reviewed and signed it, which is what its
  `Event::Conversed` reports. The screen shows the signers, the signatures
  held so far at the proposer, and the UX-003 warnings, says that the
  signature has gone, and offers Stop. Nothing there signs: the kernel's
  `review_and_sign` ran as the body arrived.
- **DONE.** On the `Event::Conversed` whose `step` is `Finalized`, from
  the verifiers or the review. `Event::Conversed` carries a `step`, the
  kernel's `Conversed` as a UniFFI enum (`Conversation`), and the flow
  switches on it (`Meet.Turn`, pure Kotlin, mapped in `Kernel.turnOf`): a
  `Reviewed` with a refusal is this device refusing the body, a `Signed`
  with one is a signer refusing the body this device proposed, each a stop
  with the wire code (`SigningRefusal.code`, 1 to 4, `wire-format.md`
  §7.10.2) and the kernel's words; everything else is the notice line's,
  in the `what` string the event still carries.
- **STOP.** `Meet.stop` from any live step calls `Meet.onStopped`, which
  the kernel sets to `Participant.abandon()`: the kernel's ceremony ends
  with the screen's, so nothing of the conversation is answered after the
  person has left it. A finished meeting is not abandoned; the kernel
  closed its own at `finalize`. Nobody is told: `wire-format.md` §7.10.1
  has no message for it, and the counterparty and witnesses read the
  silence.

**The link before the intent.** `offer` and `seek` return as the radio
starts, not as the peer subscribes, and a packet sent before then went
nowhere while reporting success. The intent, proximity and capture-key
phases now each wait on `BleBearer.ready()` (`Bearer.awaitReady`, 30 s,
`Kernel.awaitLink`) before sending; the wait is a `ble` event (`op: link`,
`state: awaited` or `timeout`, with the milliseconds) and a timeout stops
the meeting with the phase named. A send the radio refuses is a stop with
its reason, not a note.

**Diagnostics.** Every new kernel call crosses `Kernel.call`, so
`shell.call`/`shell.return` bracket `nominees`, `selectVerifiers`,
`converseOpen`, `converseQueries`, `converseGathered`, `conversePropose`
and `progress`; `meet.step` fires with trigger `kernel` for VERIFIERS to
REVIEW and for the move to DONE. The `progress` poll is one bracketed call
per tick while the flow is open, including at review.

**What remains.**

- UWB is not among the channels the shell runs (NFC and the optical pass
  only); the proximity screen says so.
- iOS: nothing is built.
- `Event::Conversed.from` is not used by the shell: the `step` names the
  signer or witness where one matters.
- Not run on devices. The emulator AVD `fueros` boots the fieldtest APK
  (52 s to boot, the kernel starts, no crash at launch), but an emulator
  has no BLE or NFC, so D2's bearer, D3 and everything after are out of
  its reach; the first pass through the courier needs two phones on the
  bench (`Robot/field-test-procedure.md`).


## The codes name the counterparty (2026-10-04)

The first field run showed the meeting bound to the provision's peer: the
kernel's *start a ceremony with X?* fired as Meet opened, before any code
had crossed, and the bootstrap was merely checked against the provision
[author, 2026-10-04: "these phone identities are hard-coded into the meeting
logic, not discovered through the QR offer"]. Beneath it the kernel's known
identities were fixed at start, so two phones not provisioned for each
other could not have met at all, against design §12.3's first contact,
where the key travels in the QR at the meeting and is pinned there.

**Now.** The first optical code carries the device's full `KeyMaterial`
(`wire-format.md` §14.3.1–.2; the author chose the key in the QR over a
hash-on-screen, bytes-on-bearer split, the code being shown as large as
the screen allows). `Client::begin` takes an optional counterparty: the
responder names the initiator from the bootstrap it read, the initiator
names nobody, and `take_optical` fixes the counterparty from the first code,
pins its identity into the client's known set and the transport's pins, and
refuses a later code naming anyone else (`Abort::NotActive`, as before). The
shell's `Meet` starts with no counterparty; `Meet.counterparty(key)` learns
it from the bootstrap (responder) or the first optical code (initiator) and
refuses a second, different key; `counterpartyName` reads *the other
person* until then. The kernel's `begin` moved from `startMeet` to the
brief's Accept (`Meet.onAccepted`), and `AndroidShell.ask` answers the
kernel's one question yes where the brief was accepted, so the person is
asked once, at D1.5. The optical screen waits on `Meet.begun()` for the
code. The QR payload is base45 (RFC 9285), QR's alphanumeric mode, so the
2 KB first code fits at error correction M (177 modules); the symbol scales
to the screen's width. The provision's peer stays the payload demo's.

Not yet: design §12.3 has the locator travel with the key; the first code
carries no `SignedLocator`, the phones having none. Reading a 177-module
code on the selfie camera at arm's length is untested on hardware.
