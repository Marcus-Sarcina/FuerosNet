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
