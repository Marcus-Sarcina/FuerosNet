# The minimum tests for the disclosure rows

**Asked for** [author, 2026-11-05]: *"Enumerate the minimum set of tests
assuming the narrative content is fixed by transaction type and replace the
current tests."*

**The assumption that does the work** [author, 2026-11-05]: *"The disclosure
page is parameterized boilerplate by transaction type… what a meeting,
peering, patron relationship etc implies is fixed across all occurrences."*
If the narrative is fixed per type, then nothing needs testing per
*occurrence*. One test says every type has its text; one says the right text
is chosen for the type. Everything else a row asks for is not narrative at
all, and that residue is the real test list.

**And one correction to what I had proposed.** I claimed each screen needed a
content function extracted before any of this was testable. It does not:
`InsetSweepTest` states the project's own answer — *"A JVM test cannot lay out
a view, so this reads the sources, as `ManifestTest` does"* — so text already
in an Activity is assertable today. What the assumption above *does* require
is that the narrative exist as a **table keyed by type**, because that is what
"fixed by type" means in code, and T1/T2 below are tests of a table. That is
also the shape the author's own plan needs: friendlier language behind
expandable headings, fixed per relationship type.

---

## Each row's `then`, split

A row's requirement is one or both of: **narrative** (words fixed by type),
and **conduct** (an order, a count, a selection, a lifecycle, a fact about
what ships). Only the conduct half needs a test of its own.

| Row | Narrative half | Conduct half, which is what remains |
|---|---|---|
| PRD-02 | the departure warning; the adoption listing's own words | the **named resources** that will be lost, computed, and shown *before* confirmation; the predicates that reach upward |
| PRD-03 | both warnings, one per kind: plain rotation, recovery | selection by kind, and nothing else |
| PRD-04 | the prompt's words | that it comes **before any eclipse condition**, and unobtrusively |
| PRD-06 | the operator view's exposure statement | hosted shown against brokered; a member's view showing **roles and not predicates** |
| PRD-07 | what device loss costs | the backup **is** under a passphrase-derived key not stored beside it; the import **deletes** the expired captures |
| PRD-08 | the vendor-session notice | **once**, and **before traffic is routed** |
| PRD-09 | all of it | — |
| PRD-10 | — | the whole lifecycle: until, absent while, returns after, **no count and no memory** |
| PRD-11 | that the ordering speaks to this operator's horizon | the **ordering** by concentration, and crowded **marked rather than hidden** |
| PRD-12 | — | frame isolation |
| PRD-13 | — | **which pages ship**, which is a fact about the sources |
| PRD-14 | — | alias-and-image against keyhash **selection**; own alias published per subnet at attach and on change, and in no record |
| PRD-17 | how many are watching | the **sequence**: engaged before the code shows, retried while a binding is fetched, one message per event and never content |
| PRD-19 | the three cases' words, one per case | the **count** and the `nominated_by` **split**; a witnessless record said plainly |
| RSC-58 | the "frame did not go" message | the isolation behaviours |
| CER-55 | — | two shells, one framing |

---

## The minimum set

**Two tests for all the narrative, together.**

- **T1 — the table is total.** Every transaction type and every case within a
  disclosure has its text, and none is empty. This is the whole narrative
  requirement of PRD-02, 03, 04, 06, 07, 08, 09, 11, 17, 19 and RSC-58's
  refusal message: eleven rows, one test, because the words are fixed and the
  only question is whether each type has them.
- **T2 — the table is selected by type.** Rotation against recovery (PRD-03),
  departure against adoption (PRD-02), hosted against brokered (PRD-06), the
  three witness cases (PRD-19): one parameterized test over the type → text
  mapping, not four tests.

**Eleven tests for the conduct, one per row that has any.**

| | Row | What it asserts |
|---|---|---|
| T3 | PRD-02 | the resources named are the ones access is lost to, and the warning precedes confirmation |
| T4 | PRD-04 | the prompt arrives before any eclipse condition holds |
| T5 | PRD-06 | hosted against brokered; a member sees roles and not predicates |
| T6 | PRD-07 | the key is not stored beside the blob; an import deletes what is past its window — **check first whether `client/tests/backup.rs` already discharges this**, in which case T6 is a marker and not a test |
| T7 | PRD-08 | shown once, and before any traffic is routed |
| T8 | PRD-10 | the prompt's lifecycle, and that nothing counts or remembers |
| T9 | PRD-11 | the ordering, and that the crowded are marked and not hidden |
| T10 | PRD-13 | the pages that ship, read from the sources as `ManifestTest` reads them |
| T11 | PRD-14 | which of the two renderings is chosen, and that the alias is published per subnet and enters no record |
| T12 | PRD-17 | the engagement order and the one-message-per-event rule |
| T13 | PRD-19 | the counts and the split, per case |

**Three rows stay manual, and should say why rather than be converted.**
PRD-12 and RSC-58 are frame isolation — a real WebView and a hostile page,
which no JVM test reaches and which belongs in an instrumentation run.
CER-55 needs a second conforming shell and is already marked provisional
until the iOS one exists.

**So: 13 tests replace 16 manual rows, 11 of those rows closing on two
tests between them.**

---

## What has to exist first

**T1 and T2 test a table that does not exist yet.** Today the narrative is
inline `para("…")` in each Activity, except `Meet.brief()` and
`Meet.presign()`, which already return `List<Item>` keyed by the row they
satisfy — the shape everything else needs.

**And some of the narrative is not written at all.** PRD-09's sentence —
a restored archive is complete only as far as its holder claims — appears
nowhere in the shell; the row is owed because the screen does not say it,
not because nothing asserts it. Checked by search, 2026-11-05.

**Which splits the work in two, and the second half is the author's.**
Collecting existing text into a table keyed by type is mechanical. Writing
the sentences that are missing is writing the very text the author has said
will be rewritten into friendlier language behind expandable headings — so
those should wait for that pass rather than be composed twice. T1 fails
loudly for a type with no text, which is the right way for the gap to be
visible meanwhile.

---

## The conduct audit, clause by clause [2026-11-05]

**Most of the conduct is already tested.** The rows are owed in large part
because nobody mapped existing tests to their clauses, not because the
behaviour is unproven. Checked by search, test by test:

| Clause | Covered by | Verdict |
|---|---|---|
| PRD-02, the resources named and the gaps in them | `LeavingTest`: `the_standing_condition_is_said_every_time…`, `what_cannot_be_computed_is_said_before_anything_else`, `a_stale_a_truncated_and_an_unswept_node_are_each_named_as_a_gap` | **covered** |
| PRD-02, warned before confirmation | the two-press arm in `LeavingActivity` | **gap** — no test holds the arming |
| PRD-07, the key kept apart | `backup.rs`: `a_backup_is_enveloped_under_a_key_kept_apart_and_comes_back_whole` | **covered** |
| PRD-07, the import deletes and says so | `backup.rs`: `a_scan_on_import_discards_what_is_past_its_window_and_keeps_the_history`, `a_client_scans_what_it_imports_and_reports_what_it_dropped` | **covered** |
| PRD-14, alias or keyhash and never a guess | `ChromeTest`: `an_orbs_mark_is_the_published_name_or_the_keyhash_and_never_a_guess` | **covered** |
| PRD-14, published per subnet at attach and on change | `profiles.rs`: `a_held_profile_is_re_signed_at_a_higher_counter_at_each_attach` | **covered** |
| PRD-14, and in no record | — | **gap**, a negative nobody asserts |
| PRD-19, the attested set and the `nominated_by` split | `MeetTest`: `the_attested_set_is_the_survivors_that_signed_split_by_side` | **covered** |
| PRD-19, a witnessless record said plainly | `MeetTest`: `no_witness_is_surfaced_to_the_person_and_the_record_will_say_so`, `no_witnesses_is_the_formation_case_and_said_so` | **covered** |
| PRD-17, engagement before the code shows | `MeetTest`'s recorder holds `calls` including `engage`, so the harness reaches it | **gap**, and a cheap one |
| PRD-11, ordering by concentration | `Participant::providers_by_concentration` exists and **nothing tests it** | **gap** |
| PRD-10, the prompt's lifecycle | — | **gap**, nothing about a backup prompt is tested anywhere |
| PRD-13, which pages ship | — | **gap**, a source assertion like `ManifestTest`'s |
| PRD-06, hosted against brokered, roles not predicates | — | **gap** |
| PRD-08, once and before traffic | — | **gap** |
| PRD-04, before any eclipse condition | — | **gap** |

**So the conduct work is seven tests, not eleven**, and four rows
(PRD-07, PRD-14, PRD-19, PRD-02) are already satisfied in conduct and wait
only on a sentence.

**No row is marked implemented on the strength of a covered clause
alone.** PRD-07's conduct is proven and its narrative is owed, so marking
it would close a row whose screen still says nothing. The marker goes on
when the last clause of a row is met, not the first.

### Where the remaining gaps actually live [2026-11-05]

- **PRD-11 — closed in conduct.** The ordering now has a test. The
  existing `candidate_providers_are_ordered_by_what_the_horizon_already_holds`
  promised it in its name and asserted only `asns_seen`, its input; the
  function it is about was never called. Least first, ties in the caller's
  order, every candidate returned and the crowded marked by their count.
- **PRD-17 — not unit-reachable as written.** The engagement order, the
  retry while a binding is fetched and the "N of M are engaged and
  watching" line are all inside `Kernel.engageNominees`, which is private
  and needs a live `Participant`. Either an instrumented run, or the
  policy becomes pure the way `Chrome` and `Leaving` already are — the
  latter is the project's own pattern and would make it a unit test, but
  it is a refactor of live code and not a test to be written.
- **PRD-02's arming, PRD-14's "in no record", PRD-10, PRD-13, PRD-06,
  PRD-08** remain as the audit has them.
