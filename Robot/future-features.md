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
