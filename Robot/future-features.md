# Future features

**What is wanted and not built.** Not a decision register — `deferred-decisions.md`
holds the questions that are the author's and are waiting on him. This holds
features he has asked for whose turn has not come, each with what already exists to
build on, so the work can start cold.

A feature leaves this file when it lands (the change-log records it) or when he
withdraws it (say so in the row rather than deleting it, so a reference resolves).

Every section reference names its document, since `Robot/refcheck.py` resolves an
unqualified section number against the citing file's own headings.

---

## 1. One scan interface, for everything a code can carry [author, 2026-11-02]

**Asked for alongside the package offering**: *"we'll want to build that out with
accepting a locator or a meeting invite into a consolidated QR scan interface."*

**Today there are two readers and no front door.** `QrCamera` (zxing, with the
focus and exposure work already done) is instantiated in `MeetActivity` for the
ceremony's optical exchange and in `ResourceActivity`, each for its own screen's
purpose. Nothing scans a code **outside** a flow that was already underway — so a
person holding a code has to know which screen to open before they can read it,
which is backwards.

**What one interface would take.** The dispatch already exists in two places and
needs no new parsing:

| What the code carries | What reads it now | Where it should land |
|---|---|---|
| an `AnchoredIntroduction` — somebody's key and locator with the anchor it names and where that anchor answers (`wire-format.md` §2.4), which is what a code handed over at a meeting carries [author, 2026-10-13] | `Kernel.takeIntake`, **called from no screen** | the contact, taken |
| a bare `Introduction` (`wire-format.md` §2.3) | the same call, which decides by parsing | the same |
| a node's `EndpointRecord` (`wire-format.md` §7.6) | the same | a node this client can reach |
| a package offering, `fueros://install?v=1&…` (`rhtn_client::offering`; `infra-client-requirements.md` §9.4) | `OfferingActivity`, reached by a tapped link or a paste | the install screen, pre-filled |

**The shape that follows.** One camera screen, opened from the shell rather than
from inside a flow; it reads a code, asks the kernel what it is, and takes the
person to the screen for that kind. `Kernel.takeIntake` is explicit that the kernel
decides by parsing *"so a person scans a code without being asked what kind it
is"* — and that rule should hold across the package offering too, which is the
whole of what consolidating means here.

**What it must not become.** A scanner that acts on what it read. Every one of
these is untrusted content from outside the client: an offering fills a screen its
reader confirms (`light-client-requirements.md` §10), and an introduction is taken
as a contact and not as a relationship. The consolidation is of the *reading*, not
of the consent.

**Also owed with it**: the meeting invite's own path. D2's optical exchange
(`Robot/light-client-screens.md` section D) is a live handshake between two phones
and is not this; what a scan should take is the card one of them hands over
(`Kernel.ownCard`), which is made and never read by a screen today.

---

## 2. The launch screen's pickers constrain each other [author, 2026-11-02]

**Asked for from the bench**: *"the 'fetch regions and sizes' control on the
launch screen populates every size and every region, but many combinations result
in a rejected request for that size not being available on that region. If there
is a way to query currently available instance sizes by region, that would be
helpful."*

**There is, and the client already has the answer in hand.** This wants no new
request and no new provider call: **both adapters already parse which regions each
size is offered in**, and it already reaches the shell.

| | where the availability comes from | what is kept |
|---|---|---|
| DigitalOcean | `/v2/sizes` returns each size with its own `regions` array | `Size.regions`, parsed, and sizes marked unavailable are dropped |
| Hetzner | `/v1/server_types` returns each type with `locations`, each flagged `available`; the prices' locations where a type carries no list | `Size.regions`, the unavailable locations filtered out, and a deprecated or non-x86 type dropped |
| the ffi | `LaunchSize.regions` | carried |
| the shell | `Launch.Size(…, regions: List<String>)` | **held and not used** |

So the gap is only in the drawing: the two pickers are populated independently
from lists that already know how they intersect. **The work is to make one
constrain the other** — picking a region narrows the sizes to those offering it,
picking a size narrows the regions, and whichever was chosen first stays chosen.

**And to say why a combination is absent rather than only hiding it**, which is
this project's rule for every other refusal (`light-client-requirements.md` §9:
a refusal renders its reason). A size that exists but is offered nowhere the
operator can reach is a different fact from a size that does not exist, and an
operator comparing prices will want to know which they are looking at.

**One thing to check before trusting it.** A provider's own list is a statement
about the catalogue and not a reservation: DigitalOcean can still refuse a
creation for capacity in a region that lists the size. So the constraint removes
the combinations that are *known* wrong; a refusal at creation stays possible and
must keep arriving in the provider's own words, as it does now.

---

## 3. The enrolment form fills itself [author, 2026-11-02]

**Asked for from the bench**, after enrolling a machine whose launch had not
enrolled it: the form had to be filled by hand, and **the one field an operator
cannot fill is the token** — their client minted it, keeps it, and never shows
it, so a launch that got as far as creating a machine leaves them with a form
they are unable to complete.

**Everything it wants is already held.** This is population, not retrieval:

| field | where its value already is | what the form does today |
|---|---|---|
| the instance's surface | the instance row the launch wrote (`instances`, first column) | defaults to `10.0.2.2:7449`, a development address |
| its enrolment token | `token:<surface>` in the operator preferences, written by the launch (`rememberToken`) and read by `tokenFor` for a renewal | **blank, and unobtainable by the operator** — no screen shows it |
| credentials | — | `7`, which is right |
| the address to publish | the machine's address, and `QUIC_PORT` beside it | blank, which signs no endpoint record at all |

**So the shape is: enrolling is an act on an instance, not a form beside one.**
An instance in the list that holds no run already offers *Renew its run*, which
refuses because there is nothing to renew (`Kernel.enrol` says so). The same row
should offer **Enrol it** where the run is absent, filling the surface from the
row, the token from the store, and the published address from the row's own
machine — leaving the operator the choice of how many credentials and the one
tap. The hand-entry form stays for an instance this client did not launch, which
is what it was written for.

**Two things to keep while doing it.** The proof must still be checked before
anything is signed — the whole point of the exchange is that a stranger at that
address gets no signature (`infra-client-requirements.md` §8.3) — and the
published address is a choice rather than a default: a machine reachable at an
address its provider gave is the ordinary case, and an operator behind their own
routing may want another. Fill it; do not fix it.

**And it would have been noticed sooner with the launch's own silence fixed**
(UX-024): what sent an operator to this form was a launch that created a machine
and could not enrol it, which is now a verdict with a cause rather than a form
to fill in the dark.

---

## 4. The attach form fills itself, and a member learns the node without a cable [author, 2026-11-02]

**Asked for from the bench**, with the harder half named in the same breath:
*"we'll need to figure out how to obtain server information on the other phones
without side-loading."* Filling the form on the operator's own device is
bookkeeping; getting the same two values onto a subordinate's phone is a
protocol question.

**The form wants two things** (`AttachActivity`): the serving node's **identity
keyhash**, 64 hex — not its transport key, which is what the screen already
shows beside *detached* and is a different value — and an address it answers on.
A member with one and not the other attaches to nothing.

### On the operator's own device: already held

| field | where it already is |
|---|---|
| the node's keyhash | **it is the operator's own identity**: a delegated instance speaks as its operator, so this device's own keyhash is the answer |
| the address | the instance row the launch wrote, with `QUIC_PORT` rather than the surface's port |

So for the one device that launched the instance, this is the same shape as
feature 3: an act on the instance row — *Attach this device to it* — rather than
a form beside it. **The two ports are the trap**: the surface answers on 7449
and the node on 7431, the row holds the first, and an attach to it waits
forever. Whatever fills this must take the address apart and put the right port
back.

### On a member's device: the protocol already carries it

**This is not a new mechanism and should not become one.** `wire-format.md`
§2.4's **anchored introduction** is *"somebody's key and locator together with
the anchor that locator names and where it answers"* — which is exactly these
two values, and `Kernel.takeIntake` already parses it, and `Kernel.ownCard`
already makes one. What is missing is only a screen that reads one, **which is
feature 1 of this list**: the consolidated scan. So the answer to the question
asked is that feature 1 delivers it, and the two should be built together.

**Three paths, and what each is good for:**

- **A code from the operator's phone, scanned by the member** (feature 1). Works
  with no network between them and no infrastructure at all, which is the case
  that matters: a member who can reach no node cannot be *told* where one is by
  the network. This is the bootstrap.
- **A re-signed endpoint record through the topology** (`wire-format.md` §4.4,
  §7.6; `infra-client-requirements.md` §4.4). This is how a member learns that a
  node they already reach has **moved** — the record is re-signed at a higher
  counter and travels on the attach replays. It covers every address change
  after the first, and covers nothing before it.
- **At the meeting itself.** The card handed over in D2 already carries the
  anchor and where it answers, so a member adopted *after* an instance exists
  learns of it as part of being adopted and needs none of the above. **The four
  bench phones met before this instance existed**, which is exactly why they
  need the bootstrap now.

**What must not be built**: an address typed by an operator into somebody else's
phone, and nothing that makes a member's client trust an address because a
message said so. An introduction is signed; a scanned code is confirmed by the
person holding both phones. Those are the two warrants, and a bare address in a
notice is neither.

---

## 5. A known node on the attach screen is selectable [author, 2026-11-03]

**Found on the bench**, re-entering the same sixty-four hex digits for the third
time: *"it's displaying on the screen as a known node, but it is not selectable
so a manual entry is again required."*

**The screen already knows.** `AttachActivity` reads `Kernel.instances()` on
every visit and draws `Attach.roll(...)` into a `TextView` — the node's keyhash
and where it answers, the two values the fields below want, **rendered as text
nobody can touch**. The roll's own empty-case line says *"Type where a node
answers below"*, which is the right instruction when nothing is known and the
wrong one the moment something is.

So this is the same shape as features 3 and 4 and should be built with them: the
values are held, and what is missing is that **reading them off a screen and
typing them back in is the operator's job today**. Each known node becomes a
control; tapping it fills both fields; the fields stay editable, because a node
this device has never heard of is still the case the form was written for.

### What the tap is for, which decides when there must be one

**Ruled** [author, 2026-11-03]: *"future state should have automatic re-attach on
any known current node for which the connection information has not changed. The
requirement for a human tap is to prevent stealth insertion of a node the user did
not choose."*

**So the consent is about the choice of node, not about the act of attaching.**
That is a sharper rule than the one this entry first carried — which had the tap
guarding the session itself, on the grounds that attaching publishes this device's
payload material — and it reverses the default in the common case:

| | what happens |
|---|---|
| a node this device already chose, connection information unchanged | **re-attach, with no tap**: nothing is being chosen that was not chosen before |
| a node this device has not attached to before | **a tap**, because this is the choice |
| a node already chosen whose **connection information has changed** | **a tap**, because a changed address is exactly the shape a substitution takes |

**And it is what the design already does one level along.** A client fails over to
a cached sibling without asking (`light-client-requirements.md` §4): that list was
pushed by the node the user chose, so following it is not an insertion of anybody.
The rule above is the same principle stated generally — a node reached on the
authority of a choice already made needs no second consent.

**Two consequences worth writing down now.**

- **An automatic re-attach must do what an attach does**, the replay included.
  Tonight's finding was that a reconnect pushes an endpoint record and a
  delegation and **no transactions**, so the teaching happens only on an explicit
  attach. Make re-attach automatic without fixing that and a node that drops out
  stops being taught, silently, with nobody tapping anything.
- **A newer record with a different address is the change, not a detail.** This
  entry first said the newer counter should simply win; under the rule above it
  wins the *display* and not the attach — it is precisely the case that wants the
  human, and a client that quietly followed a re-signed record to a new address
  would be doing the thing the tap exists to prevent. What makes it safe to show
  is that the record is signed; what makes it want a tap is that the operator is
  entitled to know their node moved.

**And the two ports are the trap again** (feature 4): an instance row holds the
administration surface's port and an attach wants the node's. Whatever fills these
fields takes the address apart and puts the right port back, or the operator waits
out a timeout for a reason the screen did not explain.

**And the roll should say which it is.** A node known from a signed record, a
node this device is attached to now, and a node typed in by hand once are three
different kinds of knowledge, and an operator choosing between two of them is
entitled to know which they are looking at.
