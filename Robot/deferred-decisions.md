# Decisions the author has explicitly deferred

**This file holds only what he has deferred on purpose** [author,
2026-10-16]. A decision that is his gets asked in the output when it
arises — at the moment if it blocks the current task, in the same output
if it does not — and asked again in every output until he answers.
Nothing arrives here because it was inconvenient to ask; the rule is in
`CLAUDE.md`.

Each entry is written to be decided **cold**, months later:

- **What prompted it** — the work that ran into it.
- **The options**, as narrowly as they can be put.
- **What was done meanwhile**, so nothing is blocked on the answer.
- **What changes if it goes the other way** — the cost of having waited.
- **When it arose**, and when it was deferred.

---

## The presence figures of design §7.5, under the four-minute window

**Deferred** [author, 2026-10-24]: "there's no sense trying to tune the
presence thresholds until we move to the 4 minute timer, as what feels
reasonable from a user standpoint will undoubtedly shift at that point."

- **What prompted it.** A6 on the bench (2026-10-24): the capture loses
  1.6–3.7 s of a 15 s window to the camera's first frame while the
  detector samples frames that do not exist yet; the NFC latch is 4 s; a
  user has to move from tap position to filming position inside those
  windows; the continuity check is off on the field-test build
  (`device.rs`, window 0 [author, 2026-10-18]) because the 90 % figure is
  design §7.5's ratio for four minutes and over ten seconds is a different rule.
  The author's reading: "a handful of spot checks over the course of
  minutes with a forgiving rule as to presence; just enough that it wasn't
  safe to do the initial exchange and walk away."
- **The options.** (a) Restate design §7.5's figures now, against the 30 s
  field-test window; (b) restate them once the witness window is the
  design's 240 s and the post-optical regime is one concurrent state, from
  what runs on the bench then; (c) leave design §7.5 as written and carry the
  field-test relaxation as a build difference indefinitely.
- **What was done meanwhile.** (b) in effect: design §7.5 stands as written; the
  field-test build keeps continuity off and the 30 s window; the regime
  restructuring proceeds first (the author's third ruling of the same day).
- **What changes if it goes the other way.** (a) would tune thresholds to
  a window the author has already said will not be the one users feel, and
  would be redone; (c) leaves the releasable rule untested on hardware.
- **Arose** 2026-10-24, during A6; **deferred** 2026-10-24.
