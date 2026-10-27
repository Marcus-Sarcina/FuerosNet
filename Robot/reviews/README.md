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
| ~~`directive-and-implementation-review-2026-10-26.md`~~ and its probes | The directive surface and its implementation as built on 2026-10-26 | **Ingested and deleted** [author, 2026-10-27]: all nine dispositions are in [`../review-tracking.md`](../review-tracking.md) under R01–R09, and the ten counterexample probes — every one of which reproduced before the fixes — became ordinary regression tests. The files are in `e34bcbb` if they are ever wanted |

**A review is deleted once its findings are recorded**, which is what this
directory is for: a received artefact is an input, and the record of what was
done about one is `../review-tracking.md`. Keeping the input afterwards would
keep two accounts of one pass, and the account with the dispositions is the one
that stays true.

The older review artefacts in this workspace predate this directory and
stayed where they are: `client-review-2026-10-11.md` at the top level, and
the per-pass history inside `../review-tracking.md`.
