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
clean once its lesson is written into design §0 rather than applied case by case — and
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
   `review-tracking.md`: one row per finding, one of FIXED / FIXED-DIFFERENTLY /
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
reasoning is wrong.** design §14.4 accepts patron eclipse because identities are cheap
and there is no token to steal. design §14.5.7 accepts its privacy costs, each for a
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

The one exception remains `review-tracking.md`, which is not specification but a
record of dispositions — supplying it would tell a reviewer not only what was
found but what was decided, which is process, not artifact.

**Supply `wire-format.md` alongside the design document for 0.8.** Encoding-level
attacks — canonicalisation, malleability, bounds, seed grinding — are invisible
from the design document alone, and the one attack of that class found so far
(design §7.2.2's grinding attack) came from an implementation attempt rather than from
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
3. Validate a presence record, including the finalization threshold and
   verifier-selection seeding.
4. Client attach with sibling failover.
5. Run a capture and a verifier query, including sealed-capture handling and
   segment-key release.
6. Register a resource and answer a catalog query for it.
7. Authorise a user to a hosted resource, from the incoming request through to the
   framing handed to the resource.
8. **Carry a resource request that is refused**, from the incoming request to the
   response the requester receives. Every prior target took a success path, and
   `resource-requirements.md` §7.1 makes refusal the normal outcome for most
   requesters rather than the exceptional one — access is a predicate evaluated at
   request time, so most evaluations return nothing.

**Coverage matters as much as novelty.** Targets 1–4 exercise identity and presence,
5 the capture path, 6–8 the resource layer. **A layer nobody has built against
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
granted; a native filesystem is not. Design document §8.2 makes the archive a **second
factor**, and A13 assumes users retain it — so silent eviction would cost portable
history and weaken a security property, not merely inconvenience the user. That
bears on retention enforcement (design §10.8.7.1's scan-on-import) and on recovery.

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
3. A well-funded commercial operator wanting fake standing at scale (budget:
   $100k/month)
4. A state actor with legal compulsion over one cloud provider
5. A malicious counterparty at a single in-person meeting
6. Someone who has stolen a device, including its keys and archive

### Prompt

```
You are a security researcher asked to break the system described in the
attached specification. You have no stake in it succeeding.

Your adversary role for this session, and the only one to consider:

    [ONE ROLE FROM THE TABLE]

You have been given everything, deliberately — including the designers' own
register of known weaknesses (§10.4, §10.5.4, §10.5.7, §11.1, §11.3) and their
change log. A real attacker would have all of this, so you do too.

Describe the best attacks you can construct in that role. For each: the
preconditions, the steps, what you gain, what defence the design states, and
whether that defence actually holds.

Classify every finding into exactly one of:

  RESTATES   — already in their registers, in substance
  EXTENDS    — a known weakness, but a worse consequence or an attack path
               they did not describe
  NOVEL      — not in the registers at all
  REASONING  — an accepted risk whose stated JUSTIFICATION is wrong. They
               accept patron eclipse because "identities are cheap and there
               is no token to steal", and accept nine privacy costs each for
               a stated reason. If a reason does not hold, show why.

RESTATES findings are worthless — give the count and move on. REASONING
findings are the most valuable, being the only ones the designers cannot
reach by looking harder at their own list.

Rank your findings by expected damage. Be specific about costs and
quantities. If a stated defence holds under your best effort, say so — a
confirmed defence is a useful result.

If a maximum-effort attempt yields nothing beyond RESTATES, say so plainly.
That is a real result, not a failure.
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

**Targets:** the flow-metric claims in design §13.2 and §14.1 — that a fake subtree's
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
| 6 | **0.8b** | Purge | Vignette/spec agreement. Late, once content has stabilised |
| 7 | **0.9** | Purge | Organisation. Last, so it does not churn against content edits |

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
unspecified that had since been specified. **Add the header to the grep list**: it
is the part of a document a new reader reads first and the authors read last.

**Original ordering rationale, for reference:**

**Now, in this order:** 0.1 (facts) → 0.4 (parameters) → 0.3 (unjustified
claims) → 0.5 (rule fragility) → 0.2 (coherence) → 0.6 (implementation gaps) →
0.7 (privacy) → 0.8 (adversarial) → 0.9 (organisation).

Mechanical extraction first: it is cheap, it surfaces concrete defects, and
fixing those before the interpretive passes means the expensive reviews are not
spent rediscovering typos. Organisation last so it does not churn against content
edits.

**Then**, once the open items in design §17 are closed, re-run 0.2 and 0.4 only —
those are the two that decay fastest as the document changes.

**Then** Stage 1, before writing much code. The formal models are cheapest to
build while the protocol is still malleable, and their findings are the most
expensive to act on after implementation.
