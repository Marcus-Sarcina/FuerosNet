# Reviews received

External review passes over this repository's documents and its
implementation, as they arrived: the reviewer's words, unedited.

**Not re-cited and not corrected.** A review's section references are the
reviewer's, as-of-filing, and a received artefact is not ours to rewrite
to suit a checker. That is why these sit in a subdirectory:
`Robot/refcheck.py` reads the top level of `Robot/` and not below it, so
nothing here is held to this project's citation discipline and nothing
here buys an exemption from it. The rule in `CLAUDE.md` — every `§N` in
every document, in every direction, with no exemptions — covers the
documents this project writes, which is what it was written to protect.

**What happens to a review is recorded elsewhere.** Findings are verified
one at a time against the actual text, and the disposition of each goes in
[`../review-tracking.md`](../review-tracking.md); what the review changed
goes in the root documents and in `../../change-log.md`. A file here is
the input, never the record of what was done about it.

| Filed | What it reviewed | Where its findings stand |
|---|---|---|
| `directive-and-implementation-review-2026-10-26.md` | The directive surface and its implementation, as built on 2026-10-26 (the node's federation module, the three reserved exchanges, the envelope's record rules, the node-to-node carriage) | **Nine findings, not yet ingested here.** They were adopted into the application thread's `implementation-plan.md` on the same date; what any of them asks of this repository has not been verified or applied |
| `directive-review-probes-2026-10-26.rs`, `run-directive-review-probes.py` | The probes written for that review, which exercise the surface directly | Beside the review they belong to |

The older review artefacts in this workspace predate this directory and
stayed where they are: `client-review-2026-10-11.md` at the top level, and
the per-pass history inside `../review-tracking.md`.
