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

Sections whose behaviour depends on human judgment may carry a numbered vignette:
a short passage in an informal voice giving a physical-world picture of the
mechanism. Headed `**Vignette Vn**` so they can be found mechanically. They are
illustrative, never normative — a mismatch between vignette and specification is a
required conversation, not a bug in either. Write the optimistic case in the
vignette's own voice, then append its limits separately. Vignettes belong only
where behaviour is being assumed, never beside pure encoding. Any change to a
section obliges a check of its vignette.

### Worked instances for structural rules

**A rule about structure gets a worked instance naming specific parties**
[author, 2026-09-04]. Not a vignette — no informal voice, no human
judgment — just the rule applied to named positions: *your patron's sibling
is at distance 1, their counterparty at distance 2.*

**The reason is evidential, not pedagogical.** The trust-landscape rule was
stated abstractly, agreed, written up, and only when the assistant spelled
the instance out did the author see it was not what he intended — the
author's own first example contradicted his own rule. An abstract rule can
be read as what you meant; an instance either matches or does not, and the
mismatch is visible to both parties at once. *"Spelling it out like that is
how I realized it wasn't what I intended."*

**So the instance is a check on intent**, which is why it belongs in the
document rather than in review correspondence: the next reader gets the
same check the author got.

### The documents carry the design, not its history

**No passage may explain itself by reference to an earlier draft or a review
round** [author, 2026-09-04]: *"We don't need to refer to earlier drafts or
review rounds. The spec has not been published yet, so there is no backward
compatibility to maintain."*

**The tell is a sentence whose subject is the document.** *"An earlier draft
said X, which conflated Y"*, *"a cross-family review found Z"*, *"this
acceptance now claims relief only on the second."* Each pins a live claim to
a dead one, and the reader has to hold both to extract the one that counts.

**Rewrite by keeping the claim and dropping the correction.** *"An earlier
draft said adverse results are visible to anyone who weighs them, which this
paragraph's premise contradicts"* becomes *"an adverse result is not visible
to anyone who weighs it, having become an absence."* Where the surrounding
text already carries the point, delete rather than rephrase — four of the
eight instances swept on 2026-09-04 were restating a ruling made three lines
above them.

**Three things this does not touch**, all of which stay:

- **Register tombstones.** Retired type numbers, withdrawn findings, closed
  assumptions. A citation must resolve to *withdrawn* rather than silently
  to a different entry, which is why numbers are never reused.
- **Protocol supersession.** Superseded credentials, locators and
  registrations are mechanisms, not drafting history.
- **Rejected alternatives.** Appendix rows recording what was considered and
  why it lost are forward-looking: they answer the implementer who is about
  to propose it again.

**The history lives in `change-log.md`**, which is what it is for, and in
this directory. A root document that needs its own history to be understood
has a drafting problem, not a documentation gap.

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

> The veto exemption in design §7.4.2 has been restated twice for exactly this reason.
> "Transfer is exempt from patron veto" evaporated when transfer collapsed into
> adoption. Its replacement named departure and adoption — one type name swapped
> for two, no more durable. Only *"a party with authority over another may never
> veto an action whose sole effect is to end that authority"* survives a
> refactor.

**What is component-variable lives in the component's document** [author,
2026-09-02]. Anything optional or variable for a software component — local
policy bounds, tunable knobs, per-client behaviour — favours inclusion in that
component's requirements document, with `network-design.md` saying what is
always true of the system as a whole. The design does not mirror component
parameter lists: keeping two tables in sync is one more surface for error
(§21.1 states this for its own table). The exception runs the other way:
an implementation-variable characteristic whose setting impacts the system
argument — the security model, the privacy posture — must additionally be
named in the design where that impact is weighed.

**Generalising a rule can break design §1.1, and has.** Abstraction widens scope, and a
wider rule may reach past the enforcement boundary that the narrower one respected.
design §9.6's abuse-report rule began as *"addressed to the owner and not broadcast"* —
a delivery property the sender controls completely. Its role-level rewrite added
*"intermediaries must neither receive nor retain it as reputation evidence"*, which
governs foreign storage and is unenforceable. **The generalisation was more elegant
and less true.**

So the two conventions must be applied together, in order: state the property, then
**ask who would enforce the restated version against whom** (design §1.1). If the answer
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
`Robot/review-plan.md` and `Robot/review-tracking.md`. **Findings and their reasoning belong
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
argument was better: an expired verification query cannot be answered, and design §7.1.4
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

## Register discipline for privacy findings

Moved from design §14.5 (2026-08-27): method rules for the findings register,
illustrated by its own history. The design keeps only the registers themselves.

**When a finding assumes a component, ask whether the component is required.**
Not whether it can be made safe — that question takes the component as given and
generates work. The former C12 and P22 both reasoned about protecting a
verification-query log that turned out not to be needed at all: the anti-oracle
defence is rate limiting, which wants a lock that expires, not a history that
persists. **A finding can entrench the thing it is about**, and the entrenchment is
invisible because the register looks like it is doing its job.

**A finding whose mechanism is withdrawn is closed, not merely stale.** The former
P10 described fingerprinting a policy descriptor's published parameters, and design §13.2
now publishes nothing at all. **When a mechanism is removed, its findings go with it** —
leaving them makes the register describe a system nobody is building.

**A finding must name a loss the design causes.** Two entries were withdrawn for
failing this: the former P8 reached only what horizon membership already discloses,
and the former P9 named an exposure that **predates the mechanism it blamed** — a
thief already reads what is addressed to the key they hold, so a later notification
adds nothing to the loss. **Check when the harm occurs, not only whether the
mechanism touches it.**

**A finding must add something membership does not already carry.** An attacker
holding a subnet's topology is a horizon member, and membership discloses a known
package: who is in the org, its shape, and what its members run (design §1.2's middle tier). **A
mechanism that reaches only that package is not a separate vulnerability**, however
it is framed — the former P8 was withdrawn on exactly this ground, and any finding
whose precondition is *"an attacker inside the horizon"* should be checked against
it before it is entered here.

