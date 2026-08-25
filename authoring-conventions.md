# Authoring and review conventions

**Working notes for whoever drafts and reviews this document set**, human or
otherwise. None of it is part of the design; it is kept out of
`network-design.md` so that a reader looking for the protocol is not reading
instructions about how the protocol was written.

---

### Rhetorical intensifiers

The 0.3 pass flagged roughly 220 adjectives doing unearned work: "trivially",
"catastrophically", "comfortably", "negligible", "cheap", "far cheaper",
"materially safer", "self-punishing". Individually harmless; collectively they
make a document of assumptions read like a document of results.

**Standing editorial rule:** where a quantity is known, give it. Where it is not,
say "assumed" or "unmeasured" rather than reaching for an adjective. An
intensifier is not a substitute for a number, and a reader cannot tell which
claims were computed and which were felt.

---

---

### Vignettes

Sections where the design **anticipates or scaffolds human behaviour** carry a
numbered vignette: a short passage in an informal voice giving a physical-world
analogy or a worked use case. They are set off as blockquotes and prefixed
`**Vignette Vn**` so they can be found mechanically.

**What they are:** illustrations of intent. They communicate the gestalt of how
the system is meant to be used, which a dry specification conveys badly, and they
are the right input for later work that needs the feel of the thing — user
documentation, interface design, art direction.

**What they are not:** evidence. A vivid story is not a workload study. Where a
vignette dramatises one of the load-bearing assumptions in Appendix A.2, it says so, and
that cross-reference means the assumption remains unvalidated no matter how
plausible the scene reads.

**A vignette states the base case plainly, then notes its limits separately.**
Hedging inside the narration produces prose that is technically careful and
rhetorically dead — *"somebody spent four minutes proving they were where I was"*
teaches a reader less than *"we met"*, and the reader who needs the caveat is not
the reader the vignette is for.

So: **write the optimistic case in the vignette's own voice, then append the
reservations in italics with their section references.** The limits belong in the
document and belong visible; they do not belong woven through a scene until it
argues with itself. Most people leak far more than this network asks them to and
are untroubled by it — writing for the most security-anxious reader misinforms the
ordinary one, who is who these are for.

**One vignette is persuasive rather than illustrative.** V6 argues for accepting
a known cost rather than describing intended use. That genre carries a risk illustration does not: it can be quoted to wave away a
future finding. Any persuasive vignette must
carry its own counter-argument in the body and an explicit statement of what it
may not be used for — V6 does both. **Prefer illustration; use persuasion
sparingly and mark it.**

**Extraction produced three of the eight vignettes here.** V1, V7 and V8 were already in the document as
formal-voiced prose doing informal-voiced work — the teams analogy, jury
nullification, the postal model. Finding those is usually more productive than
writing new ones, and §4.1.1 held two of them for two days after this convention
was written. **When adding a vignette, first check whether the section already
contains one in the wrong register.**

**Three rules:**

1. **Vignettes belong only where behaviour is being assumed.** Never beside
   arithmetic, wire formats, or derivations — putting a story next to the min-cut
   argument blurs exactly the line this convention exists to draw.
2. **A mismatch between vignette and specification is a required conversation,
   not an errand.** Neither wins automatically. This document's postal and
   jury-nullification analogies each *corrected* the spec rather than
   illustrating it, and that is a normal outcome.
3. **Any change to a section obliges a check of its vignette**, and any change to
   a Appendix A.2 assumption obliges a check of every vignette citing it.

---

---

## Where invariants live

*(The design document keeps its own statement of this rule, because it governs how
requirements are written there. Repeated here as authoring guidance.)*


**A security or fairness invariant is stated in role terms in this document; the
wire format states its encoding.** Both are needed, and they are not
interchangeable.

A rule expressed in terms of a message type, field name, or transaction name is
correct *in the wire format* — that is where identifiers live. It becomes a
hazard when it is the **only** statement of the rule, because a rename, merge, or
taxonomy change then removes the invariant and nothing looks wrong afterwards.

> The veto exemption in §7.4.2 has been restated twice for exactly this reason.
> "Transfer is exempt from patron veto" evaporated when transfer collapsed into
> adoption. Its replacement named departure and adoption — one type name swapped
> for two, no more durable. Only *"a party with authority over another may never
> veto an action whose sole effect is to end that authority"* survives a
> refactor.

**Generalising a rule can break §1.1, and has.** Abstraction widens scope, and a
wider rule may reach past the enforcement boundary that the narrower one respected.
§9.6's abuse-report rule began as *"addressed to the owner and not broadcast"* —
a delivery property the sender controls completely. Its role-level rewrite added
*"intermediaries must neither receive nor retain it as reputation evidence"*, which
governs foreign storage and is unenforceable. **The generalisation was more elegant
and less true.**

So the two conventions must be applied together, in order: state the property, then
**ask who would enforce the restated version against whom** (§1.1). If the answer
has changed, split the rule — keep the enforceable part as a MUST and represent the
rest as a visible distinction.

The test when writing a rule: **would this still be true and still findable if
every identifier in the document were renamed?** If not, state the property, then
give the encoding as its present implementation. The usual shape is *invariant in
italics, then "Present encoding: …"*.

**Status.** All eighteen rules identified by review pass 0.5 now carry role-level
statements. This convention was itself introduced in an edit that left ten of
those eighteen unfixed — a stated discipline with known exceptions is worse than
none, and the verification pass caught it. Any future rule added without its role
statement is a defect, not a deferral.

---

## Review programme

The staged review plan, per-finding dispositions and pass history live in
`review-plan.md` and `review-tracking.md`. **Findings and their reasoning belong
there, not in the design.** Where a review changed a decision, the design states
the decision as it now stands and `change-log.md` records that it changed.

---

## Cleanup passes finish, or they do not count

**A mechanical pass over the document set covers every document in it.** A pass
that covers one file leaves the others in a different state, and the resulting
inconsistency is worse than the thing being cleaned: a reader moving between two
files sees a change in register and has no way to know it is an artefact rather
than a signal.

**This has failed once already.** The em-dash reduction ran across
`network-design.md` and was reported as complete; the four companion documents were
untouched and held 355 between them. "All sections" meant all sections of one file.

**Two rules follow.**

**Scope by document set, not by file.** Before starting, list every file the pass
applies to. Report progress against that list. A pass is complete when the last
file on it is done, and not before.

**Report the number, not the state.** Say *"281 in the design, 466 across the
set"*, not *"the style pass is complete"*. A style property is not durable: new
prose reintroduces the habit, and the design rose from 332 back to 351 within a day
of the first pass because everything written that day used em-dashes freely. **A
count is checkable later and a claim of completion is not.**

**Corollary for any mechanical substitution: verify the applied count, and read a
sample of the output.** Regex on remembered text silently matches nothing;
line-spanning phrases mangle; and a substitution that opens a parenthesis on one
line will not close it on the next. All three have happened.

---

## Whose reasoning is in the document

**An explanation may be supplied by the drafter. A load-bearing one may not.**

Much of the reasoning behind this design is not written down, because spelling all
of it out would be cumbersome and would bury the payload: a workable system
specification and a concise argument for the system as a whole. Gaps are therefore
normal, and filling one with a good explanation is useful work — several imputed
rationales here are better than the original intuition behind the decision.

**The line is load-bearing versus explanatory.**

- **Explanatory is fine.** A sentence saying why a mechanism exists, so a reader
  does not take it for pointless elaboration, is editorial work and may be
  supplied. Behavioural content earns its place exactly here: where a design
  element would otherwise look like elaboration for its own sake.
- **Load-bearing is not.** A justification that becomes something the design
  *rests on* — a registered assumption, a premise another section cites, a claim
  that would have to be tested — must come from the author. Supplying one
  manufactures a dependency nobody chose, and it then sits in the register looking
  as though something was built on it.

**Two were written and removed the same day.** The keystream scheme was justified by
*"custody obligations are otherwise a real barrier to hosting"* and the queue policy
by *"a time window forces a choice nobody wants to make"*. Both entered the
assumptions register. Neither was the author's reasoning, and the queue's real
argument was better: an expired verification query cannot be answered, and §7.1.4
counts a missing answer against **the subject**, so expiry penalises a third party
for their verifier's connection habits.

**Where the author states reasoning, push back if it seems wrong.** Recording a
stated rationale is not transcription. If the reasoning does not hold, or rests on
something the design contradicts elsewhere, say so before writing it down — that is
the point of stating it.

**Do not go back and strip imputed explanations programmatically.** One that has
survived an unsupported-claims review and the author's own reading is doing its job.
The rule governs what is introduced, not a purge of what is there.

---

## When a finding assumes a component, ask

A privacy or security finding usually arrives shaped as *"X leaks, how do we
protect X?"* **That framing takes X as given**, and answering it produces a
mitigation, a parameter, and often a new obligation — all of which then need
maintaining.

**Ask the author whether X is required before reasoning about how to protect it.**

Two findings were resolved this way by deletion. A verification-query log had a
retention question (P22) and a correlation against the archive (C12); the author's
response was that no log is needed, since the anti-oracle defence is rate limiting
and a rate limit wants a **lock that expires**, not a history that persists. Both
findings closed, a parameter disappeared, and a client obligation got smaller.

**The drafter is poorly placed to make this call.** Whether a component is load-
bearing depends on intent that is frequently unwritten, so the judgement belongs
with the author and the question costs one exchange. Reasoning about how to protect
something nobody needs is the expensive alternative, and it is invisible while it
is happening — the register looks like it is working.
