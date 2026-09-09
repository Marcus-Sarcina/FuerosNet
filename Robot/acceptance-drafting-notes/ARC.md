# Notes: archive (ARC) and topology (TOP) acceptance entries

32 entries in `ARC.json`: 16 archive, 16 topology, all milestone 3. Kinds: 20
positive, 5 negative, 7 must-accept. 6 entries carry an interpretation (ARC-03,
ARC-13, ARC-15, TOP-10, TOP-12, TOP-16). Validated with the repository checker's
own helpers (`rhtn/acceptance/tools/catalogue.py`, imported read-only): 0 flags,
every citation names a heading, every quote found verbatim after normalisation,
every quote at most 40 words, ids consecutive per prefix.

## Functions assigned, and where the specification basis was thin or absent

- **Ordinals.** The word appears in no specification document; only
  `Robot/implementation-plan.md` and `models/tla/PartitionMerge.tla` use it. Read
  here as the seqno counter within a relationship's series: `wire-format.md` §2.3
  (strictly greater, not previous+1, absence of prior state not a failure), §4.1
  field 3 (adoption opens the series at counter 0) and §4.2 field 3 (departure
  increments within the current series). ARC-04 and ARC-05 test that reading.
- **Prefix verification of a presented history.** No document names a result
  vocabulary for a chain walk. `wire-format.md` §3.4 gives *incomplete* and
  *malformed*; design §10.1 gives *rooted at identity genesis, or at a
  checkpoint*; `wire-format.md` §3.1 gives the symptom *does not reach back*.
  ARC-03 (a predecessor the named signer did not sign) and ARC-13 (a stop at a
  countersigned reissue, distinct from a stop at an ordinary unfetchable record)
  carry interpretations for this reason.
- **Fork detection at the inquirer.** The notification's object is unspecified;
  design §22.3 item 3 records it as open. ARC-15 observes only that a message is
  sent to each patron and checks neither content nor encoding. The test also
  needs currency attestations, which the plan's own matrix places in milestone 4
  (the currency row); the assignment fixes milestone 3 and the entry follows it.
- **What a light client keeps and backs up.** Backup is a product-level
  commitment: the plan's last row (*retention and backup*) is milestone
  *manual*, and `light-client-requirements.md` §2 says nobody can check that a
  client backs up. No executable entry was written for it. ARC-16 covers keeping
  presence evidence under pruning; the companion obligation *never prune inside
  the 730-day window* has no entry, for budget, and is worth one.
- **Effect of adoption, departure and disavowal on the table.** No document
  describes the table's interface. The TOP entries observe answers to *who are
  N's patrons*, *who are P's subordinates*, *who are C's siblings*, which is the
  shape of `PartitionMerge`'s `ViewPatrons` rather than of any sentence in the
  specification. The quotes justify the effect; the query shape is a reading.
- **Subnet lifecycle beyond formation.** Design §13.3 to §13.6 are a
  recommendation (require one infra, recommend two, and have the client explain
  why), a statement that the root choice has no lasting consequence, a table of
  ceremony strength by availability, and per-subnet rotation. Only formation
  (TOP-11) and the departure-to-root path it relies on (TOP-04) are testable as
  table behaviour; the rest is product or is covered by TOP-02 and TOP-03.
- **The light-client patron case.** The serving relationship is session
  behaviour; the plan's session row is milestones 2 and 4 and the SES area will
  hold attach tests. TOP-13 sits at milestone 3 as instructed and overlaps them.

## Established already, therefore not restated

Dropped after checking `test-vectors/negative-vectors.md` and `corpus.json`:
the formation-record genesis rule (R8), monotonicity against committed
back-pointers including the merge variant where only one head violates the bound
(T24), merge-list ascending order (E11), an unassigned in-range disavowal code
accepted (D1, corpus `P-disavowal-code40`) and code 64 rejected (T8). The
positive merge encoding (`P-departure-merge`) and the 8/9 back-pointer bound
(`B-backptrs-8`, `B-backptrs-9`) are likewise in the corpus; ARC-07 tests the
client producing a merge on divergence, not the encoding.

## Inconsistencies noticed between documents (not fixed)

1. **A stale cross-reference for the 730-day pruning floor.** Design §10.0's
   table says the archive is *floored at the 730-day window (§10.2)*, and
   `light-client-requirements.md` §2 cites *(design §10.2)* for *never prune
   inside the 730-day window*. Design §10.2 (*The archive is a second factor*)
   contains no mention of 730 days; the statement lives in §10.1 (*Except inside
   the 730-day window, where they are still required* / *Pruning is therefore
   permitted only beyond the window*). Checked with `grep -n 730` over §10.2's
   line range: no hit.
2. **Archive scope across bindings.** Design §10.1 says *the archive's scope
   across bindings is left unstated, and no claim is made that one chain spans
   them*. `wire-format.md` §7.9's `ArchiveRequest` names a subject keyhash and
   nothing else, and §3.1 says *each participant's archive is a hash chain*: the
   fetch path can only address one chain per key. The entries assume one chain
   per key with per-relationship series as checkpoints on it; if the author
   intends one chain per binding, ARC-08 to ARC-13 need a relationship selector
   the wire format does not carry.
3. **Ordering a disavowal within a relationship.** Design §6.2.1 has each node
   sign a sequence number on every position change; a disavowal (`wire-format.md`
   §4.3) carries no seqno for the subject, which design §18.5 states outright
   (*carries no subject counter*). `PartitionMerge` orders the latest record per
   relationship by one ordinal and models only adopt and depart. How a table
   orders P's disavowal of N against N's later re-adoption under P (a new series
   at counter 0) or against N's own departure is decided by no sentence;
   timestamps are signer-controlled and never checked against a clock. TOP-05
   and TOP-06 avoid the question by applying a disavowal to a live binding only.
4. **Evaluation of a presented archive is explicitly not a conformance
   requirement** (`infra-client-requirements.md` §5: *the rest of this section is
   what the design expects of a patron who follows it, not a conformance
   requirement*), while the plan's §8.2 archive row lists it as owed. ARC-14
   binds the reference policy only and says so in `given`.
5. Editorial: `wire-format.md` §3.1 has the fragment *A decoder MUST accept lists
   of any length from 1 to 8. The bound in §1 — and MUST verify every
   back-pointer present*; the clause after the dash has lost its verb.
6. For the checker rather than the documents: design §3.4 writes *f−1* with
   U+2212 (MINUS SIGN); `catalogue.normalise` folds em, en and non-breaking
   hyphens but not U+2212, so a quote of that sentence typed with a hyphen fails
   verbatim matching. The entries avoid quoting it.

## Questions only the author can answer

1. An observer's table can see an adoption that would give a patron an eleventh
   subordinate (f = 10, design §3.1), or a third non-infra level (L = 2, design
   §3.3). `wire-format.md` §3.4 makes *takes effect* a separate question from
   *valid* and answers it by *the receiver's own view*. What does the table do
   with such an adoption: apply it, apply and mark it, or hold it? No entry was
   written for either bound.
2. One archive chain per key, or one per binding (inconsistency 2)?
3. How is a disavowal ordered against later records in the same relationship
   (inconsistency 3)?
4. Design §9.0.2: *members holding the old key remove it from their records and
   overwrite it with the new one*. When the recovery adoption is under a
   different patron P′ inside the horizon, what does P's subordinate slot show
   afterwards — the old key removed and nothing in its place, or the old binding
   left until P disavows? TOP-15 uses the same patron to stay inside the quoted
   sentence.
5. Should a chain walk surface distinct outcomes for *rooted at genesis*,
   *rooted at a checkpoint*, *incomplete at an unfetchable record* and *does not
   reach back* (a fetched predecessor the signer did not sign)? ARC-03 and
   ARC-13 assume yes.
6. Is the inquirer's fork notification to be tested at all before §22.3 item 3
   is settled, and if so what is the minimum observable: a message to each
   patron, or a signed object?
7. Whether a failed evaluation of an adoption's presence reference keeps the
   binding out of the table (TOP-12's interpretation), or the table applies the
   binding and records the evaluation result beside it.
