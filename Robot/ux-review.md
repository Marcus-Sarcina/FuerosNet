# UX review: planned, before launch

**Planned by the author** [2026-11-17]: *"These and much of the other
interface text will have to be re-worded, in most places I will provide
the messaging verbatim as I have done in a few cases so far. The screens
will need some reorganization as well. I have this sort of UX review
planned for after the underlying functionality is debugged."*

It is a pre-launch activity: after the functionality is debugged, and
before anything ships. It covers rewording and reorganising the screens.
Until then, interface text is either the author's copy (flagged below) or
mine, written to work, and awaiting his words.

## The rule for his copy

*"Language I have provided is not part of the requirements set, so if a
future change invalidates my wording in the UI, let me know and I will
change it. My UI content can give you clues to my intentions, but should
not be treated as anchors to design around."*

- **Flag:** every string he gave verbatim carries a `// AUTHOR COPY
  [author, date]` comment in the shell. `grep -rn "AUTHOR COPY"
  crates/mobile/android` lists them. Unflagged text is mine.
- **Never engineer around it** [author, 2026-11-17]: *"You don't re-write
  it, but more importantly, you don't engineer around it. Raise the issue
  to me if there's a conflict."* A mechanism is never shaped to keep his
  sentence true. Where the design and his copy conflict, decide the design
  on its merits and raise the conflict with him in the output. Do not
  rewrite the copy either.
- **Not in the requirements:** root documents and `functional_tests.md`
  rows describe what text must convey and never quote his copy as the
  requirement. UX-033 was restated this way on 2026-11-17, and it was the
  only row that quoted copy.

## His copy, as of 2026-11-17 (7 flags)

| Where | What | Given |
|---|---|---|
| `Disclosure.kt`, every `Says.Here` | the sentences for adoption upward, unlinked path, linked path, second patron, backup (the Home prompt), restore, provisioning (the Operator screen's provider paragraph) | 2026-11-05 |
| `Relocate.kt` `question` | the move question and the first-invitation question | 2026-11-16, 2026-11-17 |
| `Relocate.kt` `ANSWERS` | Accept / Do not connect | 2026-11-16 |
| `Identities.kt` `ANOTHER` | `<New Identity>` | 2026-11-09 |
| `MeetActivity.kt` | "Adopt into which scope?" heading | 2026-10-15 |
| `MeetActivity.kt` | "Submit as which scope?" heading | 2026-10-15 |
| `Meet.kt` `Claim.label` | the "[patron alias]'s [subnet alias]" pattern | 2026-10-15 |

The flags were found by matching every shell string against quotes
attributed to him in comments, `change-log.md` and the tracking file,
then reading each candidate's provenance. The first-run buttons, "Renew
its run", the sizing and region notes and "An adoption moves authority…"
trace to no quote of his, so they are mine and are not flagged.

## Already marked "developer wording until the UX pass" [author, 2026-10-24]

`Meet.kt` (~1277), `Ceremony.kt` (~1042), `MeetActivity.kt` (~910).

## Queued for the review (observations, nothing changed)

- The Operator screen's launch section carries three paragraphs before
  its controls. The provider one is his copy; the sizing and region notes
  are mine.
- The region and size pickers draw no selected text (dark on dark). That
  is a bug rather than wording, but the screen is in the review anyway.
- His rule against verbose security text [2026-11-17]: invitations are now
  a counter on the orb plus one question. Other screens that explain security at length
  are candidates.
