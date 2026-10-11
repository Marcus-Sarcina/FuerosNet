# The bundle is a bundle of locators — working plan

**[ruled, author, 2026-10-11]** verbatim, over three messages:

> "The verifiers need to be shared directly. The 'bundles' mentioned in
> previous discussions are meant to be bundles of locators. The locator is
> the primitive for communicating about any identity."

> "Not necessarily the locator at the time of the prior meeting, the most
> recent locator the participant has for that particular prior
> counterparty."

> "The idea was that each party would blast the other with a 'bundle' of
> prior counterparties which would usually be sufficient to cover the
> verifier requirement (e.g. the bundle contains 5 options, some might be
> rejected, some might be unavailable, if two were acceptable and
> available from each side, that would suffice). If and only if the
> initial bundles fail to yield an acceptable number of verifiers, the
> parties initiate the 'go fish' procedure where they propose one verifier
> at a time in turns. This is a mechanism to avoid handing over your whole
> meeting history in the initial bundle or having it milked out of you in
> successive bundles. IF the number of exposed counterparties climbs too
> high, you can just accept the next proposal and move on."

Found by B1 with-node (`82ffd1b-a4-node-3`, `4c9d99b7`): the handed
records were unverifiable for a co-signer the receiver had never held, so
no verifier qualified. Robot/ is the assistant's workspace; this file is
pruned when the amendment lands.

## The shape (assistant's, under the ruling; the author corrects)

- **`Introduction`** (`wire-format.md` §2.3, beside `SignedLocator`): what
  one party hands another about a third — `[ 1, KeyMaterial, SignedLocator ]`.
  The key travels with the locator (design §12.3 Case 0) because a
  stranger's locator verifies under nothing else; the receiver checks
  the material's hash is the locator's subject, verifies the signature
  under it, and pins the key by hash (`known`).
- **The bundle** (`wire-format.md` §5.4) is introductions, one per prior counterparty the
  subject chooses to name, each carrying **the most recent `SignedLocator`
  the subject holds for that party**. *n* is the number of introductions
  that verify, distinct by subject; candidates are those whose subject is
  not the current counterparty. **The 730-day window goes**: a locator
  has no time, and recency is the subject's curation.
- **The sender's own locator** rides the `IntentExchange` as a ninth
  field (the receiver pinned the sender's key optically, so a bare
  `SignedLocator` suffices). The receiver keeps it as the most recent it
  holds for the counterparty. A held locator is replaced by one of the
  same series with a greater counter; a different series from the
  subject's own hand (its intent) replaces outright, since series do not
  rank (design §12.1) and the subject's own statement is the authority.
- **`BundleContinuation`** carries introductions. **`FishingProposal`
  (kind 11) is exactly one introduction**: one candidate at a time, in
  turns, only after the bundles fail to yield enough; a party may accept
  the next proposal and stop revealing.
- `ArchiveEntry` stays for what else uses it (the archive pull) and
  leaves the bundle.

## Sweep (every place, by search)

**wire-format.md**: `wire-format.md` §2.3 (add `Introduction`); `wire-format.md` §3 bounds rows 214–218
(entries per `IntentExchange`/`BundleContinuation` are introductions;
`FishingProposal` carries one); `wire-format.md` §5.2 (*n* counts introductions); `wire-format.md` §5.3
(qualification rewritten, window removed, witness-only and formation
bullets removed, duplicates by subject); `wire-format.md` §5.4 (bundle of introductions,
most recent locator held); `wire-format.md` §7.9 (fishing one at a time in turns, the
milking rationale verbatim); `wire-format.md` §7.10.2 `FishingProposal` CDDL + kind table
row; `wire-format.md` §14.3.2 CDDL (field 3, new field 8) and the "reused not reinvented"
paragraph; `wire-format.md` §7.9's `ArchiveEntry` "a list of them is also a fishing
proposal" sentence.

**network-design.md**: design §7.1 step 6 narrative (1411); design §8.1.2 bullets
(2886–2895: fishing one at a time); design §8.1's n criterion note (2769, 2902);
design §13 retention (3530–3535: the window no longer gates the bundle — pruning
boundary needs the author: what now bounds pruning?); 1714–1716, 1833,
3447 ("curated" language — check each reads true of locators).

**light-client-requirements.md** 267–276; **functional_tests.md** CER-011,
012, 013, 017, 018, 020, NET-017, NET-019 (+ new rows: introduction
qualification; the sender's locator in the intent; one-at-a-time fishing;
the most-recent rule).

**Code**: codec `schema.rs` (`Introduction`, `IntentExchange` arity 9,
`BundleContinuation`, `FishingProposal`), `bounds.rs`; archive
`locator.rs` (an `Introduction` parse/verify; `signed_locator` reuse);
client `local.rs` (intent field, entries), `selection.rs` (`pool` over
introductions; `in_window`/`WINDOW_SECONDS` removed), `store.rs`
(`locators: BTreeMap<Keyhash, Vec<u8>>`), `ceremony.rs` (`begin` builds
the bundle from held locators; `take_intent` keeps the counterparty's;
own locator = `position_in(anchor())` with the client's seqno; `known`
grows from verified introductions; `conversation.rs` `FishingProposal`;
`met()` can read the held locators), tests (`selection.rs`, `ceremony.rs`,
`position.rs`, `carriage.rs`, `local.rs`, `diag.rs`, harness), `ffi`
(bundle counts only), vectors (`generate.py`: `Introduction`,
`P-intent-exchange*`, `P-bundle-continuation`, `P-fishing-proposal`,
REJ rows; regenerate `corpus.json` and `local-interfaces.md`).

**Owed from this, for the author**: the pruning boundary in design §13
once the window no longer gates the bundle; `reach_for` could route to a
handed locator (today it routes by horizon position only) — the thing
that made nodeless witnesses and verifiers unreachable.

## Status (2026-10-11)

Applied: the five documents, codec, archive, client, vectors (regenerated
with `--accept-spec-change`, `verify.py` clean), the whole workspace suite
and clippy (two tests restated, `adaptors/tests/conversation.rs` and
`participant/tests/live.rs`, both of which had made *n* = 2 by meeting
one party twice — one party is one introduction now). Open here: the APK
rebuild and B on the bench. Owed to the author: the design §13 pruning
number; `reach_for` by handed locator; the fishing act (turn-taking) in
the kernel.

**The bench after this build.** The phones hold records but no locators:
a locator is held from the next intent each receives. So B needs, in
order: GXV × MRK (GXV holds MRK), GXV × VGY (GXV holds VGY), then the
cross pair **GXV × 8XD**: GXV's bundle introduces MRK and VGY, *n* = 2,
two candidates, one verifier required; 8XD has met VGY and selects it by
recognition (`bases:[Met]`), asks it about GXV, and VGY holds GXV's
capture from their meeting — a response in the record. The reverse
direction wants 8XD to re-meet VGY first as well.

## Order

1. Documents (the design is authoritative; code follows).
2. Codec + archive + vectors; gate on vectors.
3. Client kernel + tests; `cargo test -p rhtn-client`.
4. ffi/shell untouched unless a count changes; Android suite.
5. Bench: B1 again on `82ffd1b-a4-node-3`'s state (phones keep records;
   locators appear from the next intent each sends).
