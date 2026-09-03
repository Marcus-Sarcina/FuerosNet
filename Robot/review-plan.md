# Review Plan

**For:** Reconfigurable-Hierarchic Trust Network
**Artifacts under review:** `network-design.md`, `wire-format.md`, later an implementation
**Created:** 2026-08-12

---

> **Note on dates:** entries stamped `2026-08-12` in this file actually span
> 12–15 August 2026. The date was carried forward without re-checking. Ordering is
> correct; individual dates are not.

## How to read this

Reviews are grouped by **what they need as input**, which is what actually gates
them — not by importance. A review needing running code cannot be pulled
forward, and a review needing only documents should not be deferred.

Each entry gives: what it catches, what it needs, configuration notes, and a
prompt you can paste.

**One caveat before anything else.** Most of this document's structure came out
of a conversation with Claude. A review by Claude in a fresh context is still the
same model reviewing its own architectural instincts — fresh context removes
*anchoring*, not *shared blind spots*. Where a review matters, run it on at least
one other model family. This is the single highest-value configuration change in
the plan and it costs nothing.

---

## Reading a clean result

**0.5.2 (enforcement boundary) returned clean on its fourth run** — the first pass
in the programme to find nothing. That is a result about one reviewer, not a proof
that nothing exists.

**Spot-check a pass that finds nothing.** It is the one most likely to have looked
in the wrong place, and the cost of checking two or three of the mandates it
cleared is small against accepting a false all-clear. When 0.5.2 came back empty,
its reasoning on the verifier rule and the greasing obligation was verified against
the documents before the result was recorded.

**A second pass has now returned clean: 0.5.1 (rule fragility).** Its reviewer
checked seven plausible exceptions — departure signing, formation subtype, disavowal
reason field, witness nomination, `Attach` field 1, routing repair, abuse-report
delivery — rather than accepting the document's status claim, which is the right
way to read one.

**Spot-checking a clean pass has twice found something the pass was not looking
for.** Verifying 0.5.1's departure-signing case surfaced a malformed decision
marker, `[D (§6.2]`, and a sweep found seven more. The reference checker cannot see
those: they do not parse as references at all.

**A clean pass means the discipline has moved into the documents.** The earlier runs
corrected instances; this one found the corrections holding, including the
abuse-report rule the register keeps as its worked example. Expect a pass to go
clean once its lesson is written into design Appendix A rather than applied case by case — and
expect it to stop being clean the moment a large new mechanism lands.

## Prompt construction principles

These matter more than the specific wording below. Adapt freely, but preserve:

1. **Give a completion criterion, not an evaluation request.** "List every X"
   terminates and can be checked; "review this" produces shallow coverage of
   everything.
2. **Give the artifact, withhold the reasoning.** Never paste the conversation
   that produced a decision into a review of that decision — the reviewer
   inherits the frame and confirms it. If a decision cannot be reconstructed from
   the document alone, *that is the finding*.
3. **Explicitly permit an empty result.** A reviewer told to find problems will
   find problems. Every prompt below says some version of "if you find none, say
   so" — do not remove it.
4. **Demand location and severity**, so findings can be triaged rather than
   read.
5. **Forbid adjacent work.** "Do not suggest improvements" keeps a
   contradiction-finding pass from becoming a redesign pass.
6. **Run twice, compare.** Findings appearing in two independent runs are
   materially higher confidence than findings appearing in one.
7. **Track every enumerated finding to an explicit disposition.** Keep
   `Robot/review-tracking.md`: one row per finding, one of FIXED / FIXED-DIFFERENTLY /
   ACCEPTED / DEFERRED / REJECTED / NOT-A-FINDING, before an edit round is called
   complete. This exists because pass 0.5 was answered by engaging its framing
   and fixing seven of eighteen items, with the other eleven neither fixed nor
   recorded — a failure the verification pass caught and the author did not.
   Responses to long enumerated findings drift toward the interesting items and
   away from the tedious ones; a checklist is the only reliable defence.
8. **Always include a severity axis, even in extraction passes.** Learned the
   hard way: pass 0.3 was written as pure exhaustive extraction with evaluation
   explicitly forbidden, and returned 280 correct, undifferentiated findings.
   Roughly ten mattered. Exhaustiveness without severity produces a list nobody
   can triage, and the triage then falls back on the author — which is exactly
   the judgement the review was meant to supply. Ask for the completion criterion
   *and* a two-line severity rubric.

---

## What to supply, and what to withhold

**Strip the change log (`change-log.md`) from every purged pass.** It is the design
reasoning in condensed form — it says not only what changed but why, including
already-formed conclusions about privacy, threat models and tradeoffs. Feeding it
to a reviewer violates the "give the artifact, withhold the reasoning" rule more
directly than pasting conversation would, because it is pre-digested.

The second reason matters more: **the specification should stand without it.** If
a reviewer reading only the spec reaches a conclusion the change log would have
corrected, that is not noise — it means the specification fails to communicate its
own resolution, which is a finding you would otherwise never see.

The change log exists for *us*, to catch stale claims across edits. That is a real
job and it has caught several. It is not review input.

**Corrected 2026-08-12.** An earlier version of this section advised stripping
known-weakness sections for the adversarial pass. **That was wrong**, and it
confused two questions:

- *Threat model:* a real attacker has the specification. Kerckhoffs's principle —
  a weakness that is only survivable because the attacker does not know about it
  is not survivable. Reviewing a stripped document reviews an artifact no
  adversary will ever face.
- *Review methodology:* a reviewer handed a weakness list tends to elaborate it
  rather than search past it.

The second concern is real, but it is an **instruction** problem, not a
**withholding** problem — and withholding costs something specific.

**The decisive argument: only a reviewer who sees our reasoning can tell us the
reasoning is wrong.** design §18 accepts patron eclipse because identities are cheap
and there is no token to steal. design §19.7 accepts its privacy costs, each for a
stated reason. If any of those reasons is bad, that is among the most valuable
findings available — and stripping the section makes it structurally impossible
to find. It hides precisely the arguments most in need of adversarial reading.

**Second gain:** with full disclosure, *"nothing beyond your known list"* becomes a
meaningful result. From a maximum-effort reviewer with complete information, that
is evidence the list is reasonably complete. A stripped run that rediscovers a
known finding teaches nothing.

### Rule

**Supply everything to every pass, including `change-log.md` and the weakness registers.**
Handle anchoring in the prompt, by naming what a worthless finding looks like.

The one exception remains `Robot/review-tracking.md`, which is not specification but a
record of dispositions — supplying it would tell a reviewer not only what was
found but what was decided, which is process, not artifact.

**Supply `wire-format.md` alongside the design document for 0.8.** Encoding-level
attacks — canonicalisation, malleability, bounds, seed grinding — are invisible
from the design document alone, and the one attack of that class found so far
(design §8.1.2's grinding attack) came from an implementation attempt rather than from
adversarial review.

**On re-running a pass at higher effort:** if you hold results from a lower-effort
run, compare rather than discard. A high-effort run that finds a strict superset
is the expected result. One that finds *different* things means the pass is
high-variance, and variance is itself a finding — it implies a single run of 0.8
should never be treated as coverage.

## Context hygiene between passes

**Extraction passes always start from a purged context. Verification passes
deliberately do not.** These are different tasks and conflating them degrades
both.

| | Extraction | Verification |
|---|---|---|
| Question | "Find all X" | "Were findings 1–n addressed, and did any fix introduce a new problem?" |
| Inputs | Document only | **Prior findings + current document**, both pasted |
| Context | Purged session | Purged session — the findings are *supplied as input*, not carried in session state |
| Scope | Whole document | Only the listed findings and their edit sites |
| Finds new instances? | Yes, that is the point | **No** — explicitly forbidden; that is the extraction pass's job |
| Run when | Any time | Immediately after edits responding to a specific pass |

**Both start from a fresh session.** The difference is not session state but what
you paste. Supplying prior findings as input rather than continuing a live
session is reproducible, keeps you in control of exactly what the reviewer sees,
and lets you re-run the check later against a further-revised document.

Three reasons extraction passes purge, in order of how often they bite:

1. **The document has changed.** Editing in response to a pass means a retained
   context holds the old document, its own findings, and the new document —
   putting the reviewer in "check my corrections landed" mode instead of reading
   afresh.
2. **Adjacent frames contaminate worse than unrelated ones.** Two passes that
   both produce lists of normative statements — say, rules at the wrong
   abstraction level and rules that cannot be enforced — will pull up overlapping
   text, and a reviewer holding the first list tends to re-sort it rather than
   rescan. **Run near-neighbour passes furthest apart**, not back to back.
3. **Completion bias.** A reviewer that has just delivered a large accepted
   finding set carries an implicit sense that the document is worked over. Second
   passes in the same context come back thinner, and you cannot tell "thinner
   because cleaner" from "thinner because done."

**A rewrite is the likeliest moment to introduce a new inconsistency**, so a
verification pass earns its keep most after a pass whose findings were addressed
by rephrasing rather than deletion.

### Verification prompt

**Do not paste the original extraction prompt.** It re-primes the extraction
frame and pulls the reviewer toward hunting new instances. The findings
themselves define the concept better than the prompt that produced them. Also
avoid phrasing like "edited to resolve the issues" — it presupposes the verdict.

```
I am going to paste two things: (a) findings from an earlier review of a
protocol design document, and (b) the current version of that document,
revised in response to those findings.

For each numbered finding, report exactly one verdict:

- ADDRESSED — quote the current text that resolves it
- PARTIALLY ADDRESSED — quote the current text and say what remains
- NOT ADDRESSED — say where you looked
- ADDRESSED BUT INTRODUCED A NEW PROBLEM — quote it and describe the problem

A verdict of ADDRESSED requires a quotation from the document body. The
document contains a change log describing what was intended; that is not
evidence a change was made correctly. Verify against the actual rule text.

Then, as a separate section: examine only the passages that changed. Do any
of the edits contradict text elsewhere in the document, use a term
inconsistently with its definition, broaden or narrow a rule's scope beyond
what the finding called for, or cite a section that does not say what the
citing text claims?

Do NOT look for new instances of the original finding class — that is a
separate review. Confine yourself to the listed findings and the edits made
in response to them.

--- FINDINGS ---
[paste findings here]

--- CURRENT DOCUMENT ---
[paste document here]
```

**The change-log guard is load-bearing.** `change-log.md` records what each fix was
*intended* to do. A reviewer skimming it can mark findings ADDRESSED on the
strength of the author's own description rather than the rule text — which makes
the pass worthless in precisely the cases where a rewrite went wrong.

---

# Stage 0 — Runnable now (documents only)

## 0.1 Factual verification of external claims

**Catches:** wrong algorithm sizes, misremembered platform behaviour, stale
standards, invented defaults.
**Needs:** documents + web search.
**Config:** search enabled; fresh context; **low temperature**; a model family
other than the one that wrote the claims. Reasoning effort low-to-medium — this
is lookup, not inference.
**Why now:** none of it depends on outstanding decisions, and it is the category
you have least ability to check yourself.

```
You are verifying factual claims in a technical specification. I will paste a
document.

For every factual claim about the external world — encoding format properties,
cryptographic algorithm sizes and characteristics, mobile OS behaviour, protocol
defaults, published research findings, standards content, regulatory
requirements — do the following:

1. Quote the claim verbatim, with its section number.
2. Search for authoritative confirmation.
3. Mark it: CONFIRMED / CONTRADICTED / PARTIALLY CORRECT / UNVERIFIABLE.
4. Cite the source you relied on.
5. If not CONFIRMED, state what is actually true.

Do NOT evaluate design decisions, architectural choices, or reasoning. Only
claims that could be checked against an external authority. If something is a
design choice rather than a factual assertion, skip it silently.

Output a table sorted by status, CONTRADICTED first.
```

## 0.2 Internal coherence

**Catches:** sections contradicting each other after independent edits — the
failure mode this project has already hit more than once.
**Needs:** documents only.
**Config:** **fresh context, document only, no conversation history.** High
reasoning effort. Run per major section rather than whole-document; a 2,000-line
document reviewed in one pass gets shallow coverage.

```
You are reading a technical design document you have never seen before. You have
no access to the discussion that produced it and should not speculate about it.

Identify places where the document CONTRADICTS ITSELF: a rule stated one way in
one section and differently in another; a cross-reference pointing somewhere that
does not say what the citing text claims; a parameter given two values; a
mechanism described as required in one place and optional in another; a term used
with two different meanings.

For each finding:
- Quote both locations with section numbers
- State the contradiction in one sentence
- Say which reading you believe was intended, and why
- Severity: BLOCKING / SERIOUS / MINOR

Do NOT suggest improvements. Do NOT flag decisions you disagree with. Only
places where the document disagrees with itself.

If a section contains no contradictions, say so explicitly by name. I would
rather see "sections 4 through 6: none found" than have you manufacture findings.
```

## 0.3 Unjustified claims

**Catches:** assertions that survived because nobody challenged them; numbers
with no derivation; "obviously" doing load-bearing work.
**Needs:** documents only.
**Config:** fresh context, document only. Low temperature — completeness matters
more than insight.

```
I will paste a technical design document.

List EVERY claim that is asserted without stated justification. Categorise each:

A. QUANTITATIVE — a number, threshold, or size with no derivation or source
B. CAUSAL — "X will cause Y" with no mechanism given
C. COMPARATIVE — "X is better/cheaper/safer than Y" with no basis
D. FEASIBILITY — "X is possible/cheap/easy" with no demonstration
E. BEHAVIOURAL — a prediction about how people or organisations will act

For each: quote it, give the section, give the category, state what evidence
would be needed, and assign a severity:

- LOAD-BEARING — if this claim is false, a design decision in the document
  changes. Say which one.
- SUPPORTING — the claim matters but a design decision does not hinge on it.
- COLOUR — an intensifier or characterisation doing no structural work
  ("trivially", "catastrophically", "negligible").

Be exhaustive rather than selective. Do not skip claims that seem obviously true
— "obviously true" is exactly the category that hides errors. Do not evaluate
whether the claim is correct; only whether the document justifies it.

Sort output by severity, LOAD-BEARING first, and give the count in each band
before the list.
```

## 0.4 Unset and arbitrary parameters

**Catches:** placeholder values that became permanent by inertia.
**Needs:** documents only.
**Config:** fresh context; low temperature.

```
I will paste a technical design document containing many numeric parameters.

Produce a table of EVERY parameter, threshold, timeout, size limit, count, or
interval mentioned anywhere in the document. For each:

- Name and section
- Value (or "UNSET" if named but never given a value)
- Stated basis: DERIVED (from an argument in the document) / BORROWED (from
  another system, cited) / ASSERTED (no basis given) / UNSET
- If DERIVED, name the argument it depends on
- Whether the document says it should be tuned later

Flag separately any parameter whose value appears differently in two places.

Be exhaustive. Include parameters mentioned only in passing.
```

## 0.5 Rule fragility: types versus roles

**Catches:** safety rules expressed in terms of a message type or field name,
which silently evaporate when that type is renamed or removed. This project has
already had one near-miss of exactly this kind.
**Needs:** documents only.
**Config:** fresh context; medium reasoning effort. Mechanical enough to be
reliable.

```
I will paste a protocol design document.

Find every rule stated as a PROHIBITION or REQUIREMENT that is expressed in terms
of a specific message type, field name, or transaction name — rather than in
terms of a role, relationship, or capability.

Example of the fragile form: "type 2 messages must not be vetoed."
Example of the robust form: "a patron must not veto an action that ends their
own authority over a subordinate."

The fragile form breaks silently if the type is renamed, merged, or removed,
because nothing then looks wrong.

For each finding: quote the rule, give the section, state what security or
fairness property it protects, and rewrite it in role terms.

If you find none, say so.
```

**Run this extraction immediately after any pass that produced role-level
rewrites.** Generalising a rule widens its scope, and a wider rule can reach past
the enforcement boundary a narrower one respected — both findings in the
2026-08-15 run were introduced by role-generalisation earlier the same day.

Second extraction, same pass — **unenforceable mandates**:

```
The attached document states as a design principle (design §1.1) that a network can
only enforce where there is shared state, and that beyond that boundary the
correct move is to make a distinction VISIBLE in the evidence schema rather than
to require behaviour.

Find every place the document nonetheless states a requirement — "must", "shall",
"is required to", "may not" — that would need to bind a party the enforcing side
shares no state with.

For each: quote the requirement, give the section, name who would have to enforce
it against whom, and state whether shared state exists between them. Where it
does not, say what visible artifact would carry the same distinction instead.

Note that requirements binding a node to its own patron, or a client to its own
user, usually DO have shared state and are fine. The failures are cross-subnet
and cross-implementation mandates.

If you find none, say so.
```

## 0.6 Specification gaps via implementation attempt

**Catches:** assumptions living in conversation rather than in the document. The
most reliable gap-finder available before there is code, because it has an
objective stopping condition.
**Needs:** `wire-format.md` plus the relevant design sections.
**Config:** fresh context, **specification only**, no design rationale unless the
spec cites it. Medium-high reasoning effort. Repeat with 3–4 different targets.
**Phase 2 adds the vector fixtures — after the attempt, never before** (see below).

**Why this pass finds what reading does not.** Prose can leave a decision
unstated and still read as complete — *"signed by the issuer"* satisfied every
reviewer who read it, and stopped an implementer immediately, because you cannot
emit an object without knowing where the signature goes. **The pass converts
"seems fine" into "I cannot proceed."**

What it reliably surfaces: a field that is optional in the schema and mandatory in
practice; an array bound that cannot accommodate the protocol's own threshold; a
message with no framing; two rules that are individually correct and jointly
unsatisfiable; and any object described as carrying something it has no field for.

**Language: Rust.** The strict typing is the *reason*, not an obstacle. Every type
declaration asserts something about the specification — `Option<T>` versus `T`
asserts optionality, an enum asserts a closed variant set, an integer width
asserts a range, `Vec<T>` versus a fixed array asserts whether a count is
bounded. A dynamically typed language lets all of that stay vague; Rust forces a
commitment, and every commitment the spec does not support is a finding the model
cannot avoid producing.

Compilation does not matter — the deliverable was never working code. Say so in
the prompt, or the model will spend its effort on borrow-checker plumbing instead
of on the questions.

*(Target platform is a separate question. The current working assumption is
wasm32, carried over from prior work, but it is not settled — see the storage note
below.)*

**Suggested targets, in order:**

1. Encode, sign and verify an adoption transaction, including the optional recovery
   branch.
2. Resolve a locator to a serving infra node given an anchor table.
3. Validate a presence record, including the finalization threshold,
   verifier-selection seeding, and **a minimised record** — recompute the
   disclosure root from revealed disclosures plus withheld digests
   (`wire-format.md` §4.5.1).
4. Client attach with sibling failover.
5. Run a capture and a verifier query, including sealed-capture handling and
   capture-key release. *(This target once said "segment-key release"; the
   segment concept died with the keystream, 2026-08-31.)*
6. Register a resource and answer a catalog query for it.
7. Authorise a user to a hosted resource, from the incoming request through to the
   framing handed to the resource.
8. **Carry a resource request that is refused**, from the incoming request to the
   response the requester receives. Every prior target took a success path, and
   refusal is the normal outcome for most requesters rather than the exceptional
   one — the materialised table holds a row per member of the owner's Dunbar Org
   (design §11.4), so most askers have no row at all and the lookup misses.
9. **Flood a topology transaction and publish an endpoint record**
   (`wire-format.md` §10.1, §7.6): receive a `TopologyPush`, decide whether to
   store and forward it, suppress the duplicate that arrives back through a peering
   cycle, and build the child table a patron refers from. Exercises the body-kind
   tag, forward-if-stored, `txid` versus `(keyhash, seqno)` identity, and a record
   that supersedes rather than accumulates.
10. **Forward a rootward memo and detect a cycle** (`wire-format.md` §10.2): apply
    a memo to the optional `(patron, slot)` table ordered by field-4 timestamp,
    forward it rootward unchanged, and fire the identity check when one arrives
    naming you in field 1. Exercises ordering under out-of-order arrival, the
    at-or-after suppression rule, and the rule that a memo is a hint confirmed
    against the detector's own records — never fetched, never acted on alone.
    *(Rewritten 2026-09-02: the original wording predated the memo privacy
    correction — subject-seqno keying, a position-equality check, and a
    fetch-before-acting rule are all superseded; 0.6.10 correctly implemented
    the current text and recorded the staleness rather than following it.)*

**Coverage matters as much as novelty.** Targets 1–4 exercise identity and presence,
5 the capture path, 6–8 the resource layer, 9 and 10 propagation. **A layer nobody has built against
tends to produce a construction change rather than a list of encoding corrections**
— targets 5, 6 and 7 each did on their first run, and each needed a second.

**A target earns its place by being new or by having changed.** Every target here
has found blocking defects on a re-run, because the wire format changed underneath
it — usually as a result of the previous run. **A target run against unchanged text is worth
much less than one run against a mechanism written since the last pass** — so when
adding to this list, prefer whatever the last cycle could not have seen.

```
You are implementing part of a protocol from its specification. I will paste
the spec.

Task: [SPECIFIC NARROW TARGET]

Language: Rust. The code does not need to compile and will not be run — I will
not be checking it. Do not spend effort on borrow-checker or lifetime plumbing;
clone freely, use owned types, and leave `todo!()` where an implementation
detail is uninteresting. Suggested crates so you hit real ecosystem shapes
rather than inventing APIs: `ciborium` for CBOR, `coset` for COSE,
`ed25519-dalek` for classical signatures.

What I want is the decisions, not the code.

As you work, whenever you reach something the specification does not determine:

1. STOP and record the question precisely
2. Make an explicit assumption so you can continue
3. Mark it in a code comment as UNSPECIFIED
4. Continue

Deliver four things:

(a) The implementation.

(b) A numbered list of every UNSPECIFIED question: the question, the assumption
    you made, and whether a different reasonable implementer might have assumed
    something incompatible.

(c) A separate list of every TYPE DECISION the specification does not determine.
    In a strictly typed language each type declaration asserts something about
    the spec, and I want to know which assertions the spec actually supports.
    For each:
      - The declaration (field, signature, enum)
      - What the spec says, or that it is silent
      - What you chose and why
      - Whether a different implementer could reasonably choose incompatibly,
        and what breaks if two implementations differ
    Pay particular attention to: Option<T> vs T (is this field optional?), enum
    variants (is this list closed?), integer widths, whether counts are bounded,
    and owned vs borrowed data in anything that gets signed.

(d) A short note on CRATE MATURITY for anything the spec requires that the Rust
    ecosystem may not support production-ready today — post-quantum signatures
    (ML-DSA) and KEMs (ML-KEM) in particular, and whether they are available for
    a wasm32 target. If a required primitive has no mature implementation, say
    so plainly; that is an implementation constraint the specification needs to
    know about.

**Before adding a wire field to satisfy a requirement, classify the requirement.**
This pass is structurally biased toward adding fields, because fields are the shape
of its output — asked "what carries this?", the reflex is to add something when the
answer is "nothing". Ask instead whether the requirement governs **what the
protocol transmits**, **what an implementation does**, or **what a user is shown**.
Only the first needs a field. In the 2026-08-15 run, "attachment to a sibling must
be explicit" was misread as a wire requirement when it was a UI obligation, and a
field was added before the misreading was caught.

(b), (c) and (d) are the point of the exercise. Do not resolve ambiguities by
inference from what seems sensible — record them. If the spec is complete
enough that you made no assumptions in some category, say so explicitly.
```

### Phase 2 — vectors as acceptance data [added 2026-09-02]

**After (a)–(d) are delivered, in the same session, supply the fixture
documents** (`test-vectors/*.md`) **and never the tools directory** —
`generate.py` is a reference implementation in disguise, and handing it over
destroys the clean room: every under-determination in the prose can be silently
resolved by reading how the author resolved it. The fixtures arrive only after
the attempt for the same reason — available during implementation, they become
an oracle the reviewer iterates against, and the pass loses exactly the
findings it exists to produce.

**What phase 2 buys that phase 1 cannot.** The generator and the verification
harness are one author's reading of the specification; the 25 checks passing
proves they agree with *each other*, not with the documents. An independent
implementation hitting the same bytes is the second-implementation test, and
phase 2's third divergence class — a vector the specification contradicts — is
the only way that error class gets found before a real implementer trips on it.

```
Phase 2. I will now paste canonical test vectors covering the mechanism you
just implemented. They were generated by the specification's author from their
own reading of it, and they are DRAFT and UNVERIFIED: the specification is
authoritative, and where a vector and the specification disagree, the vector
is wrong until shown otherwise.

Check your implementation against them:

1. For each vector exercising your target, state what your implementation
   would produce for the same inputs — derived by hand-tracing your code, not
   by running it.
2. Classify every divergence as exactly one of:
   - IMPLEMENTATION BUG: the specification determines the vector's answer and
     your code took a wrong turn. Note it briefly and move on; these are not
     findings.
   - SPEC AMBIGUITY: one of your recorded assumptions and the vector resolve
     an under-determined point differently. The vector is not the resolution —
     it is evidence the point is under-determined. Cross-reference your
     UNSPECIFIED item, or add one if phase 1 missed it.
   - VECTOR DEFECT: the specification determines an answer and the vector
     carries a different one. Show the derivation from the specification text.
3. Do not modify your implementation to match a vector. The deliverable is the
   classified divergence list, not agreement.

If a vector exercises behaviour your implementation did not cover, say so
rather than extending the implementation to cover it.
```

**Reading the result.** IMPLEMENTATION BUG entries are noise. SPEC AMBIGUITY
entries merge into the phase-1 finding list — a vector "settling" a question is
not the spec settling it, and the fix belongs in the prose. VECTOR DEFECT
entries are findings against `test-vectors/` and the generator: verify the
derivation against the documents, then fix the generator and regenerate rather
than hand-editing fixtures.

**Open design item this target raises, not a review finding.** An earlier version
of this note said WASM has no filesystem access. **That is false in general** and
was a browser-specific claim stated as a property of the target:

- **WASI** (`wasip1`/`wasip2`) provides POSIX-like filesystem calls; a native WASM
  binary under Wasmtime, Wasmer, WasmEdge or WAMR has ordinary file access.
- **A WASM component embedded in a native application** gets whatever host
  bindings the host chooses to provide.
- **Even in a browser**, OPFS gives real random-access file handles with
  synchronous access in workers, which suits a photo archive far better than
  IndexedDB.

**What actually varies is durability, and that is the thing to care about.** Browser
storage is subject to eviction under storage pressure unless persistent storage is
granted; a native filesystem is not. Design document §10.2 makes the archive a **second
factor**, and A13 assumes users retain it — so silent eviction would cost portable
history and weaken a security property, not merely inconvenience the user. That
bears on retention enforcement (design §13.7.1's scan-on-import) and on recovery.

## 0.7 Privacy threat modelling (LINDDUN)

**Catches:** linkability, identifiability and detectability problems. More apt
here than STRIDE, given how much of this design turns on metadata and
correlation.
**Needs:** documents only.
**Config:** fresh context; high reasoning effort. Give the LINDDUN categories
explicitly rather than assuming familiarity.

```
Perform a LINDDUN privacy threat analysis on the system described in the attached
document. The LINDDUN categories are: Linking, Identifying, Non-repudiation,
Detecting, Data Disclosure, Unawareness/Unintervenability, Non-compliance.

For each data flow and each stored artifact in the design, work through all seven
categories and record any applicable threat. For each threat:

- Category
- The specific data flow or stored item
- The adversary who benefits (be concrete: the counterparty, the patron, a
  witness, an infra operator, a state actor, a platform vendor)
- What they learn or can do
- Whether the document acknowledges it
- Severity, and whether any stated mitigation is adequate

Pay particular attention to what becomes possible by CORRELATING two artifacts
that are individually acceptable.

Note explicitly where the design has accepted a privacy cost deliberately — those
are not findings, but confirm the acceptance is stated rather than accidental.
```

## 0.8 Adversarial design review

**Catches:** attacks the designers did not think of. Run last in Stage 0, after
the mechanical passes.
**Needs:** design document + `wire-format.md`.
**Config:** fresh context; **highest reasoning effort available**; different model
family. Slightly higher temperature than the extraction passes — unusual attacks
benefit from less predictable search. (Inference, not an established result.)

**Supply the full document set, including the weakness registers.** The prompt
tells the reviewer those are known, so its effort goes past them rather than into
them.

### Run ONE adversary per session

An attempt at all roles in a single run died silently mid-generation. Six full
attack constructions plus a classification pass over a 3,000-line document set
exceeds what one response can carry — this is a length problem, not a refusal;
a refusal would say so rather than stopping partway. Splitting also improves
depth, since each adversary gets the whole budget, and makes a failure cost one
role rather than all of them.

### Adversary roles

One per session. The prompt below takes exactly one of these.

1. A participant's own patron
2. A witness at a presence ceremony
3. A commercial operator seeking unearned standing at scale
4. A state actor with legal compulsion over one cloud provider
5. A malicious counterparty at a single in-person meeting
6. Someone who has stolen a device, including its keys and archive

### Prompt

*(Rewritten 2026-09-03 after run 0.9.3 tripped the reviewing provider's
cybersecurity classifier. The earlier form asked the reviewer to construct
and optimise attacks — steps, costs, quantities, a budgeted role — which the
provider's own feedback identified as adversary-optimisation rather than
audit. This form requests the same findings as a defensive review: property
violated, preconditions, impact, repair — never an operating plan. Role 3
lost its budget figure for the same reason. Ingest note: items 6 and 7 of
the deliverable are reviewer SUGGESTIONS and enter the process as
candidates for the author's ruling, never as decisions.)*

```
Treat the attached specifications as an authorized defensive architecture
review of a system I own and am designing. Your deliverable is findings and
repairs, not attack development.

The threat-model perspective for this session, and the only one to consider:

    [ONE ROLE FROM THE TABLE]

You have been given everything, deliberately — including the designers' own
register of known weaknesses (design §18 accepted risks, §19.4 findings
requiring action, §19.7 accepted costs, §19.8 correlations, §20.1 unsourced
assumptions, §20.2 load-bearing assumptions) and their change log. The
review must reach past what those registers already record.

Work by proving or falsifying the specification's own security claims as
they face this adversary: each stated defence, bound, and accepted-risk
justification either holds under your best scrutiny or fails for a reason
you can state. For each finding, provide only:

  1. The affected document sections.
  2. The security property violated or endangered.
  3. The minimal preconditions under which the weakness matters.
  4. The resulting impact, at a conceptual level.
  5. Whether the specification already acknowledges it, via the classes:
       RESTATES   — already in their registers, in substance
       EXTENDS    — a known weakness, but a worse consequence or a path
                    they did not describe
       NOVEL      — not in the registers at all
       REASONING  — an accepted risk whose stated JUSTIFICATION is wrong;
                    every entry in design §18 and §19.7 carries a reason,
                    and if a reason does not hold, show why
  6. A defensive design change or invariant that would mitigate it.
  7. A test, simulation, or property-check that could validate the
     mitigation.

Do not develop step-by-step exploitation procedures, optimise attacker cost
or scaling, design operational campaigns, or provide anything resembling an
operating plan. If a finding would ordinarily need operational detail to
explain, stop at the vulnerability category and affected sections and state
that the omitted details are unnecessary to remediation.

RESTATES findings are worthless — give the count and move on. REASONING
findings are the most valuable, being the only ones the designers cannot
reach by looking harder at their own list.

Rank findings by the severity of the endangered property. If a stated
defence holds under your best scrutiny, say so — a confirmed defence is a
useful result. If a maximum-effort review yields nothing beyond RESTATES,
say so plainly. That is a real result, not a failure.
```

## 0.9 Organisation and style

**Catches:** structural problems that make later review harder. Cheapest pass;
run it last so it does not churn against content changes.
**Config:** fresh context; low reasoning effort.

```
I will paste a long technical document. Assess its organisation only, not its
content.

Report: sections that would be hard to find by someone looking for a specific
decision; content that appears in a section its heading does not predict;
material duplicated across sections that should be consolidated or
cross-referenced; sections that have grown enough to warrant splitting;
inconsistent heading depth or numbering; terminology introduced before it is
defined.

Do not comment on technical content, correctness, or decisions.
```

---

## migration — the specification reordering

**Not a review.** The executable spec for the atomic renumber, settled from 0.9-before
[author, 2026-08-31]. Run it in one pass, verify, commit once.

### Method, and why it is one pass

**Two-phase renumber: every section to a unique placeholder, then placeholders to final
numbers.** A direct old→new sweep lets Topology-becomes-2 collide with
Scope-becomes-3 while both numbers are live. Placeholders make the intermediate state
unambiguous, so no reference resolves to the wrong target mid-sweep.

Roughly 420 section references across seven files move with it. **The reference checker
validates the result exactly**, resolving an unprefixed `§N` against its own document
first and `network-design.md` second — the rule that finally reproduced a clean run.

### `network-design.md` — chapter mapping

| New | Chapter | From | Reason |
|---:|---|---|---|
| — | Preface | Preface | — |
| 1 | Thesis | 1 | reorder |
| 2 | Vocabulary | 3 | terms were used before being defined; Appendix A leaves for the appendix |
| 3 | Topology | 4 | reorder |
| 4 | Scope | 2 | reorder, kept brief |
| 5 | Cryptography | 5 | — |
| 6 | Transactions | 6 | — |
| 7 | The presence ceremony | 7.1 | split; 7.1.1–7.1.8 become 7.1–7.8 |
| 8 | The presence record | 7.2–7.3 | split; become 8.1–8.2 |
| 9 | Key compromise and recovery | 7.4 | split; 7.4.0–7.4.4 become 9.0–9.4 |
| 10 | The archive | 8 | — |
| 11 | Resources | 9 | — |
| 12 | Addressing and resolution | 10.1–10.7 | become 12.1–12.7 |
| 13 | Subnet formation and lifecycle | 10.8 | promoted; unfindable under "Addressing" |
| 14 | Sessions and payload | 11 | — |
| 15 | Control plane / data plane | 12 | — |
| 16 | Trust model | 13 | — |
| 17 | Security analysis | 14.1–14.3 | split; become 17.1–17.3 |
| 18 | Accepted risks | 14.4 | split; a 238-line catalogue |
| 19 | Privacy analysis | 14.5 | split; 14.5.1–14.5.8 become 19.1–19.8 |
| 20 | Assumptions and evidence | 15 | — |
| 21 | Parameters | 16 | — |
| 22 | Open for v1 | 17 + 18.2 | restructured, see below |
| 23 | Deferred to a later version | 18 + 2's *Explicitly deferred* | restructured |
| 24 | Suggested build order | 19 | — |
| A | Document conventions | 0 | moved to appendix |
| B | Decision log | Appendix A | — |

**§7 splits three ways, not two** [author]. The ceremony is ~1,000 lines on its own and
is not further divisible without cutting a single argument; the record and the recovery
procedure separate cleanly from it and from each other.

### `wire-format.md` — chapter mapping

| New | Chapter | From | Reason |
|---:|---|---|---|
| 1–3 | Encoding, Primitives, Common envelope | 1–3 | — |
| 4 | Transaction types | 4.1–4.5, 4.8 | series reissue becomes 4.6 |
| 5 | Verifier selection | 4.6 | a seven-subsection mini-specification |
| 6 | Resource registration, catalog, abuse reports | 4.7 | the section says these are *not* transactions |
| 7 | Attestations and records | 5 | — |
| 8 | Session messages | 6 | — |
| 9 | Transport binding | 7.1–7.2 | §7 outgrew "QUIC binding" |
| 10 | Topology propagation | 7.2a, 7.2b | split; retires the alphanumerics |
| 11 | Resource requests | 7.3 | split; 7.3.1–7.3.2 become 11.1–11.2 |
| 12 | Size estimates | 8 | — |
| 13 | Open items | 9 | reduced to detail referenced from design §22 |

### `infra-client-requirements.md` — alphanumeric normalisation

`§4a` became **§5** and `§9.2a` became **§10.3**, with the sections after each shifted. Both are retrofit identifiers that break decimal sorting
and outline generation. `wire-format.md`'s `7.2a`/`7.2b` retire in the split above.

### Open work: two chapters, not six lists

**The distinction is release-scoped** [author]: what must be settled for the initial
release is separate from what is wanted in a later one. §23.2 already carries it as
three subheadings — *blocks a subsystem*, *decide during implementation*, *deferred by
decision* — so this promotes an existing classification rather than inventing one.

- **§22 Open for v1** — the single index. Former §22's live questions and §23.2's first
  two groups. This is the section that must be empty of blockers before release.
- **§23 Deferred to a later version** — §23.2's third group, §23.1 multi-device, §4's
  *Explicitly deferred*, §22's two standing deferrals, and §23.3's test-vector note.
  Nothing here blocks anything; it is the wishlist and should read as one.

**The other four lists stay where they are and are referenced, not absorbed.**
`wire-format.md` §13, `light-client-requirements.md` §Open, `infra-client-requirements.md`
§Open and the local block in the ceremony chapter each hold items belonging to their own
document's authority. §22 names them and says what each holds; moving their contents into
the design would break the authority split Appendix A sets up.

### Verification before commit

1. **Reference checker: zero unresolved** across the five root documents.
2. **Heading depth** matches numbering everywhere (`depth = components + 1`).
3. **No out-of-order headings** in any document.
4. **Word census** against the pre-migration revision: additions confined to new chapter
   headings, no words removed. This is what caught a 20-entry miss in the change-log
   restructure and is cheaper than reading 7,000 lines.
5. **Fence parity, no trapped headings, no unclosed table rows.**

---

# Stage 1 — Formal modelling (before or alongside implementation)

These need a formalisation, not a running system. They are the highest-value
reviews in the plan and the ones least replaceable by an LLM pass.

## 1.1 Symbolic protocol verification

**Tools:** Tamarin or ProVerif. Both are symbolic-model cryptographic protocol
verifiers; Tamarin was used on TLS 1.3, ProVerif on Signal's protocol.

**Targets, in priority order:**

| Target | Property to verify |
|---|---|
| Presence ceremony | An adversary not physically present cannot produce a record a third party accepts |
| Recovery adoption | A thief holding the key cannot produce a rotation that outweighs the legitimate holder's |
| Currency attestation + stapling | A revoked key cannot be made to appear current beyond the attestation lifetime |
| Session attach | A sibling cannot impersonate the patron for trust-bearing operations |

**Note honestly:** an LLM can help *write* the model and interpret the output,
but the verification result comes from the tool, not from the model. Do not
substitute an LLM's opinion about whether a property holds.

## 1.2 Distributed-systems modelling

**Tool:** TLA+ / PlusCal.

**Targets:** convergence after partition and merge; whether stranding can occur
in states the design does not anticipate; correctness of cycle detection under
concurrent adoptions; whether the currency-attestation escalation chain (patron →
sibling → grandpatron) can deadlock or produce two live answers.

The partition-and-merge behaviour is the one I would model first, because the
whole design rests on it and it has never been tested against anything but
argument.

## 1.3 Trust metric simulation

**Tool:** a graph simulator, not a model checker.

**Targets:** the flow-metric claims in design §16.2 and §17.1 — that a fake subtree's
claim is bounded by its cut regardless of size; that λ < 1/f is necessary; the
detection probability arithmetic for shared identities; hub formation around
infra operators. All of these are currently argued analytically and none has been
run.

---

# Stage 2 — Implementation review (needs code)

| Review | Notes |
|---|---|
| **Spec conformance** | Differential testing against the spec; best done by an implementer who did not write the spec |
| **Cryptographic implementation audit** | **Human expert, paid, not an LLM pass.** This is the one place where the "don't roll your own" rule extends to review — LLM review of crypto implementation is not a substitute and should not be presented as one |
| **Fuzzing the decoder** | The CBOR parser is the highest-value target: deterministic-encoding enforcement, duplicate keys, indefinite-length rejection, unknown-field preservation. Malformed input arriving from strangers is the design's largest untrusted surface |
| **Property-based testing** | Canonical encoding round-trips; signature verification invariants under field reordering; locator truncation at nibble boundaries |
| **Differential testing** | Two independent implementations disagreeing on canonical encoding is the failure that breaks every signature in the network |

---

# Stage 3 — Pre-deployment

| Review | Notes |
|---|---|
| **Legal review of biometric handling** | **Not optional and not deferrable.** GDPR Art. 9, Illinois BIPA, Texas CUBI. Every node operator storing photographs is a data controller. Get real counsel before any public beta |
| **Scale simulation** | The parameters explicitly deferred "to tune under load": cache placement, TTLs, anchor caching thresholds, heartbeat intervals |
| **Ceremony UX and accessibility review** | The presence ceremony assumes camera use, guided head movement, and several minutes of cooperation. Consider users with visual, motor, or cognitive impairments, and what the fallback is |
| **Red team against a running system** | Repeat 0.8 with the implementation available |

---

# Stage 4 — Post-deployment

- Parameter tuning against real traffic
- A published vulnerability disclosure process, which needs to exist *before*
  launch even though it operates after
- Periodic re-run of 0.1 (external facts go stale: platform behaviour changes,
  algorithms get deprecated, regulations move)

---

# What this plan cannot do

Stated plainly so it is not over-trusted:

- **It cannot validate the social claims.** That competent patrons will be
  selected for, that trust networks will stabilise, that friction will be
  tolerated — these are empirical predictions about human behaviour and no review
  resolves them. Only deployment does.
- **It cannot substitute for a cryptographic audit** by people who do that for a
  living.
- **LLM passes find inconsistency and omission far better than they find deep
  design flaws.** They are a filter, not a verdict.
- **A finding's absence is weak evidence.** Two independent runs agreeing that
  something is fine is worth something; one run finding nothing is worth little.

---

# Suggested order

**Working order as of 2026-08-12** (0.1–0.5.1 complete):

| # | Pass | Context | Notes |
|---|---|---|---|
| 1 | **0.5-verify** | Purge session; **paste the 18 findings + current doc** | Certify the seven rule rewrites while "what changed" is still unambiguous. After 0.6 that instruction stops being usable |
| 2 | **0.6** | Purge | Implementation gaps. Start with *encode, sign and verify an adoption transaction* — build-order step 2, and it exercises deterministic CBOR, COSE multi-signature, key material vs key hash, the locator, the PoP reference and archive-subset references. Expect the most edits from this pass |
| 3 | **0.5.2** | Purge | Unenforceable mandates. Deliberately separated from 0.5.1 — near-neighbour frames contaminate. Likely cheap now that design §1.1 states the principle |
| 4 | **0.7** | Purge | LINDDUN privacy |
| 5 | **0.8** | Purge, high effort, **different model family** | Adversarial. Last of the substantive passes — an adversarial reviewer distracted by inconsistencies produces worse attack analysis |
| 6 | **0.9-before** | Purge | Organisation, on the current structure. Fix local defects — heading levels, misfiled blocks, out-of-sequence subsections — **before** anything is moved, so the migration relocates sound material rather than carrying breakage into a new place where it is harder to attribute |
| 7 | **migration** | — | **DONE 2026-09-01**, in four stages. Not a review. Spec below, settled from 0.9-before: `network-design.md` reordered and three chapters split, `wire-format.md` split at §3 and §7, `infra-client-requirements.md`'s alphanumerics normalised, and the six open-work lists reduced to two release-scoped chapters plus references. **One atomic two-phase renumber** — sections to unique placeholders, then to final numbers — so Topology-becomes-2 cannot collide with Scope-becomes-3 mid-sweep. ~420 section references across seven files; the reference checker validates the result exactly |
| 8 | **0.9-after** | Purge | **DONE 2026-09-01.** Organisation again, on the migrated structure. **Confirmed nothing was orphaned or double-numbered in the move** — the pass's whole purpose, and the result was negative. What it found was local: three heading-level defects, three misfiled blocks, four chapters needing subsections, one real duplication and one that was not, and the change log's own heading collisions. Applied in four passes; dispositions in `review-tracking.md` |

## Second cycle (2026-09-01 →)

**The programme restarts from 0.1** [author, 2026-09-01]: the 0.8 rounds and the
nine test-vector review passes changed enough — absence-is-the-encoding, the
curated bundle, selection by recognition, the type-6 retirement, chain-wide
monotonicity, ten encoding rulings — that the first cycle's findings describe a
different design in places. A full consistency-and-coherence pass plus de-lint ran
2026-09-01 to produce the baseline set (change-log records the numbers); cycle 2
runs the same pass sequence against it, clean-room as before. First-cycle
dispositions in `review-tracking.md` remain as-of-filing.

**0.9 runs twice, before and after the migration** [author, 2026-08-28]. The
first pass is about defects in what exists; the second is about the move itself.
Splitting them keeps two failure modes apart: material that was already
misorganised, and material the migration displaced.

**Known inputs to 0.9-before**, found in passing and deliberately not fixed early:
51 heading-level anomalies (depth-3 sections written `###` in 51 places and
`####` in the resources section's subsections — two conventions, the smaller one
correct); and the open question of where Vocabulary sits in the new order, which
reads naturally before Topology since a reader meets *patron*, *subordinate* and
*Dunbar Org* there before the structure that uses them.

## De-linting: the released document is a design, not a record of drafting it

**Target** [author, 2026-08-28]: the initial specification should read as a
**discrete, self-contained design**. Alternatives are mentioned briefly, future-state
material is confined to one topic, and nothing narrates how the document was
written. Example given: §20.1's *"Flagged by the 2026-08-14 factual verification
pass as asserted without an external source"* — a true statement about the drafting
process and no part of the design as released.

**Survey (2026-08-28).** Smaller than it feels, and concentrated in four kinds:

| Kind | Count | Disposition |
|---|---|---|
| **Pass archaeology** — *flagged by the … pass*, *a review pass registered it*, *found by a self-check* | ~4 | Remove. The finding survives; the pass that found it does not |
| **Dated decision markers**, `[D — 2026-08-28]` against a bare `[D]` | 42 (8 design, 34 wire) | The *decision* is design; the *date* is change-log material. Dropping the dates leaves 241 bare `[D]` markers doing their job |
| **Bare dates in prose** — *Agreed 2026-08-16*, *Settled 2026-08-25*, *narrowed/reduced/restated 2026-08-28* | ~86 date tokens overall | Same class. Some carry a real qualifier (*narrowed*, *reduced*) whose **substance** should stay and whose date should not |
| **Drafting archaeology** — *an earlier reading gave…*, *previously stated the opposite*, *superseded* | ~8 | **Judge individually.** Some stop a reader re-deriving a rejected design and earn their place; some are pure history. Appendix A's *rejected alternatives* is the right home for the first kind |

**Not lint, despite looking like it**: the change-log pointers in the registers.
*Withdrawn numbers are recorded there so a citation resolves* is live
infrastructure — it is what makes the no-reuse rule work — and the document table
row naming `change-log.md` is orientation for a reader, not process narration.

**The cause, and why a cleanup alone will not hold it** [author, 2026-08-28]. A
dedicated pass was run against this material once already, with a different model,
for no purpose but to clear it — and it came back. Working through a large review
means holding per-finding state somewhere, and **this file's companion,
`review-tracking.md`, is the scratchpad that exists for exactly that.** Writing that
state into the specification instead is an attempt at the same thing which "doesn't
work very well and leaves a confusing document". Remove the instances without
removing the reason and they return with the next review round.

**It is also duplication.** `change-log.md` already records when each of these
changed and why, at length. A parenthetical date in the specification is a second
copy of a fact that is kept better elsewhere, and two records of one fact drift.

**So the rule, not just the cleanup**: per-finding iteration state goes in
`review-tracking.md`; reasoning goes in `change-log.md`; **the specification carries
the decision and nothing about when it was reached.** A bare `[D]` is the
convention and is sufficient. Where a qualifier carries real substance — *narrowed*,
*reduced*, *restated* — keep the substance and drop the date.

Sequence the sweep **after** the last substantive review, or it will be run twice.

**Known input to the migration**: the open-items section cites the scope section
four times for deferred material — IPv6, hard-fork forwarding, transaction types
beyond the seven, ranging mode. Folding *Explicitly deferred* into it turns those
into self-references, which want rewording rather than remapping, and a mechanical
sweep will pass straight over them.

Run a **verification pass after any pass whose findings were addressed by
rephrasing or by replacing a mechanism.** Rewriting a rule is a good moment to
change its scope accidentally; *replacing a mechanism* is worse, because the old
one leaves references scattered across both documents. The COSE conversion in
pass 0.6 left five stale signature descriptions in the design document while the
wire format had moved on — found by a self-check, but only because the change was
large enough to prompt one.

**Cheap habit that catches most of it:** after any mechanism change, grep both
documents for the name of the thing you replaced before declaring the edit round
done.

**Front matter goes stale silently.** Status lines, "last updated" dates and
scope summaries are written once and never re-read, because nobody looks at the
top of a document they already know. Both went stale here — the date was carried
forward unchecked for three days, and the status line claimed two things were
unspecified that had since been specified.

**Resolved by removal rather than by checking** (2026-08-29). The "last updated"
and "last revised" dates are gone from both documents: an edit timestamp is in the
filesystem and in git, so restating it by hand created a surface that could only
ever be wrong. **Scope summaries in headers remain on the grep list** — they carry
claims nothing else records, so they can still go stale and still have to be read.

**Original ordering rationale, for reference:**

**Now, in this order:** 0.1 (facts) → 0.4 (parameters) → 0.3 (unjustified
claims) → 0.5 (rule fragility) → 0.2 (coherence) → 0.6 (implementation gaps) →
0.7 (privacy) → 0.8 (adversarial) → 0.9 (organisation).

Mechanical extraction first: it is cheap, it surfaces concrete defects, and
fixing those before the interpretive passes means the expensive reviews are not
spent rediscovering typos. Organisation last so it does not churn against content
edits.

**Then**, once the open items in design §22 are closed, re-run 0.2 and 0.4 only —
those are the two that decay fastest as the document changes.

**Then** Stage 1, before writing much code. The formal models are cheapest to
build while the protocol is still malleable, and their findings are the most
expensive to act on after implementation.
