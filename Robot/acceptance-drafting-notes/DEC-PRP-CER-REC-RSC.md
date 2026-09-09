# Notes: the passes drafted in single-thread mode (2026-09-09)

DEC (16 entries, drafted before the interruption, no notes file survived), then
PRP/CUR/MET (36), CER (29), REC/PAY (29) and RSC/PRD (33), drafted directly.
Every quote verified verbatim by `rhtn/acceptance/tools/check.py`; 0 flags.
The interpretations recorded in the entries are the questions for the author;
the items below are what the drafting noticed beside them.

## Inconsistencies noticed (not fixed)

1. **design §12.6.5's inline `CurrencyAttestation` sketch is stale against
   `wire-format.md` §7.1.** The sketch has five fields and a signature "BY THE
   PATRON"; the wire has an issuer role (field 5: patron, sibling, grandpatron,
   down-line) and an issuer identity (field 6), and design §12.6.5.1's ladder
   depends on both. The design wins on facts, but here the wire is the fuller statement.
2. **Who chooses which sealed capture opens.** `light-client-requirements.md` §1.3
   says a verifier should "prefer the most recent eligible one when
   answering"; design §7.5.2.8 and `wire-format.md` §7.3 have the subject's
   grant name the capture, and a verifier can open only what it is granted.
   CER-08 and CER-09 read the preference as the subject's default when granting.
3. **Quotes that live in schema comments carry the comment's leading
   semicolons after the checker's normalisation** (RSC-04, RSC-22, the earlier
   RES-13). A prose restatement of those rules would remove the artefact; it is
   recorded here rather than worked around in the checker, whose job is to
   match the text as written.

## Questions only the author can answer

- **Rate-limited one-time keys** (PAY-05): is the excess request answered with
  the bundle and no field 3, or with failure code 1?
- **Exhaustion notice** (PAY-06): what carries "tell the subject when their
  one-time pool is exhausted"? No wire object does.
- **Payload demultiplexing** (PAY-14): open per design §22.2; the entry
  parameterises it.
- **Second profile in one ceremony** (CER-16): the verifier rejects; is the
  subject's client also required to refuse to countersign, or is that the
  capability design §7.4.2 describes and no more?
- **Form of client notices** (CER-21, CER-26): the split warning and the
  capture-time disclosure are asserted to occur; their content is not specified.
- **A second request on one resource stream** (RSC-11): the wire says one per
  stream and nothing about the consequence.
- **Fork resolution policy** (REC-12): "by which patron it trusts" is policy; the
  entry fixes it. Is a default expected of the reference client?
- **The pluggable-policy interface** (MET-04): what must be exposed so a
  substitute policy can run is not stated.
