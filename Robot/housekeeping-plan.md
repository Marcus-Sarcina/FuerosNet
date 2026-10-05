# Housekeeping: documentation, organization, optimization (2026-10-04)

Three passes the author asked for, in order, each depending on the one
before it [author, 2026-10-04]:

1. a sweep of every source file for thorough and consistent commenting and
   documentation;
2. with each file's and function's purpose settled, an inspection of the
   whole for improper organization, overlapping concerns, object ownership
   and duplicated effort;
3. a high-effort pass to make each function efficient and reliable, and to
   find and remove incorrect corner-case behaviour.

This is the plan, not the work. **The author ruled on all five open
questions on 2026-10-04**; each is recorded where it was asked, and the
plan below is the ruled version.

---

## What is actually there

Measured 2026-10-04, at `bc08f98`.

| | Files | Lines |
|---|---|---|
| Rust, `src/` | 134 | 52,407 |
| Rust, `tests/` | 108 | 43,866 |
| Rust, total | 242 | 96,273 |
| Kotlin, the shell | 27 | 6,634 |

Documentation, counting public items against those carrying a `///`:

| Crate | Public items | Documented | |
|---|---|---|---|
| `codec` | 122 | 55 | 45% |
| `participant` | 40 | 23 | 57% |
| `archive` | 304 | 178 | 58% |
| `transport` | 201 | 124 | 61% |
| `sim` | 84 | 53 | 63% |
| `adaptors` | 56 | 36 | 64% |
| `policy` | 121 | 79 | 65% |
| `crypto` | 65 | 45 | 69% |
| `node` | 375 | 268 | 71% |
| `client` | 519 | 411 | 79% |
| `cli` | 42 | 34 | 80% |
| `resources` | 12 | 10 | 83% |
| `daemon` | 47 | 41 | 87% |
| `ffi` | 162 | 142 | 87% |
| **Total** | **2,150** | **1,499** | **70%** |

**The base is better than the number suggests.** Every one of the
source files that declares a module but one carries a header, and the headers are the good
part of this codebase's documentation: they say what the file is for, cite
the document that required it, and record the decision behind it. The gap
is in the items beneath them — about 650 public functions, types and
constants with no comment at all — and in consistency, since no standard
has ever been written down and nothing enforces one.

**Nothing enforces anything.** There is no `missing_docs` lint in any
crate, so the 30% can grow without the gate noticing.

---

## Phase 1: the documentation sweep

### 1.0 Settle the standard first

The house style exists but is tacit. It should be written down before 650
items are written to it, or the sweep will produce 650 items in whatever
style each hour of the day produced. The model is already in the tree
(`daemon/src/diag.rs`, `client/src/sequence.rs`, `archive/src/chain.rs`):

- **A module header says what the file is for, what it owns, and which
  document required it.** It cites by document and section, never a bare
  `§N` (`refcheck` enforces that rule for the root documents; code is
  outside its reach today — see 1.4).
- **An item's comment says what it is for and why it is as it is**, not
  what the code plainly does. A bound says where the number came from. A
  refusal says which rule it enforces.
- **Where the author decided something, the comment names the decision and
  dates it**, in the `[author, YYYY-MM-DD]` form the documents use. Where
  he decided something and gave no reason, the comment records the decision
  and does not invent one — the standing failure mode in `CLAUDE.md`.
- **A `//` comment inside a function marks a hazard or a choice**, not a
  narration of the next line.

**[ruled, author, 2026-10-04]** A new section of
`Robot/authoring-conventions.md`. **Written**: *The source, which is
documentation too*, with the five rules above stated at length and the
cleanup-pass rule carried over to crates. The sweep is written to it.

### 1.1 Order of work: bottom-up by dependency

The crate graph is a clean layering with four inversions (see 2.1). Working
up it means each crate is documented by someone who has just read what it
rests on:

```
codec → crypto → archive → policy → transport → node
      → client → adaptors → ffi → {daemon, cli, sim, participant, resources}
```

This order also happens to put the worst-documented crate first: `codec` at
45% is both the bottom of the stack and the weakest, and it is the one
whose correctness every other crate inherits.

### 1.2 Batches

Ten batches, sized by items rather than files, each ending with one gate
run and one commit:

| Batch | Crates | Items owed | Notes |
|---|---|---|---|
| 1 | `codec` | ~67 | the schema is 2,700 lines and the single densest file |
| 2 | `crypto`, `policy` | ~62 | |
| 3 | `archive` | ~126 | second densest; `topology.rs` is 1,736 lines |
| 4 | `transport` | ~77 | `session.rs` is 2,551 lines |
| 5 | `node` | ~107 | |
| 6–7 | `client` | ~108 | `ceremony.rs` is 3,744 lines, the largest file in the tree |
| 8 | `adaptors`, `ffi` | ~40 | the FFI's comments are the binding's documentation |
| 9 | `daemon`, `cli`, `sim`, `participant`, `resources` | ~50 | |
| 10 | the Kotlin shell | — | 27 files; no coverage measured yet, `Meet.kt` is 970 lines |

The lesson from milestone 1 of the diagnostics work applies: run the full
gate once per batch at its end, not before and after each file. The cost
there was half avoidable and it was avoidable in exactly this way.

### 1.3 What the sweep will produce besides comments

Three by-products, and the second is the valuable one:

- **A queue of questions for the author.** Every place where a bound, a
  refusal or an ordering exists for a reason nobody wrote down. These are
  not to be guessed at. Expect this queue to be long; the documents were
  written before much of the code, and the code made choices the documents
  do not reach.
- **A list of phase-2 candidates**, gathered as they are met rather than
  looked for: a function in the wrong module, two functions that do one
  thing, a type owned by nobody in particular.
- **Dead code**, which is cheapest to find while reading every line.

### 1.4 Enforcement, so it cannot regress

Two mechanisms, both added as each crate is finished rather than at the
end:

- **`#![warn(missing_docs)]` at each crate root.** The gate already runs
  `clippy -D warnings`, so a crate marked this way cannot acquire an
  undocumented public item afterwards. This is the whole of the
  machine-checkable part.
- **`Robot/doccheck.py`**, for what the compiler cannot see: a module
  header on every file, citations in code resolving to real headings in the
  real documents (the `refcheck` rule extended past the root), and no
  citation of `Robot/` from code, which is the same rule the root documents
  keep.

**[ruled, author, 2026-10-04]**, on the question of who pays for it. **The
drafter pays, and the author is never asked.** `missing_docs` fires on one
condition: a public item with no `///` above it. It is answered by writing
the comment, which the drafter was going to write; it asks nothing, judges
nothing, and cannot be satisfied by a decision only the author can make.
Where a comment would need a rationale nobody wrote down, the question goes
to him **because the convention says so, not because the lint said
anything** — the lint is as content with *"the bound is 256; why is not
recorded"* as with a full account.

Three further reasons it is cheap here, and one cost:

- It is added **per crate, as that crate is swept**, so it never fires on a
  backlog. A crate that passes the sweep passes the lint by construction.
- It catches the drafter's omission at the gate rather than at the next
  reader, which is the cheapest place.
- Recent work already meets it: everything written in the last fortnight
  carries comments, which is why `ffi` and `daemon` sit at 87%.
- **The cost**: a public item added in haste fails the gate until it is
  documented, which is a half-minute and is the point.

**The alternative he offered** — a pre-release sweep instead of a
continuous one — is what produced the present 30% gap, and he named its
cost himself: the sources are less useful as documentation in the
meantime. A repeated sweep also costs more in total than a ratchet, since
each one re-reads what the last one read.

### 1.5 Risk

Low. No behaviour changes, so the gate is a full check of the work. The two
real risks are a sweep that invents rationales, answered by the question
queue above, and a sweep that goes stale against a moving codebase, which
argues for doing it now while the field-test work is paused.

---

## Phase 2: organization, concerns and ownership

### 2.1 A correction, and the graph as it actually is

**The four dependency findings in the first draft of this plan were all
wrong, and they were wrong in one way.** The measurement read each
`Cargo.toml` whole, so `[dev-dependencies]` counted as dependencies. Every
one of the supposed inversions is a test-only edge:

| Claimed | Actually |
|---|---|
| `daemon` depends on `cli` and `sim` | both dev-dependencies |
| `resources` depends on `sim` | dev-dependency |
| `sim` depends on `ffi` | dev-dependency |
| `ffi` and `daemon` list dependencies twice | once in each section, which is ordinary |
| `client` depends on `node` | dev-dependency, used by three test files |

**The production graph is a clean acyclic layering with no inversions:**

```
0  codec                         5  adaptors  cli  resources  sim
1  crypto                        6  daemon  ffi
2  archive                       7  conformance  participant
3  policy  transport
4  client  node
```

This is the project's own named failure mode — *asserting a sweep is
complete without running it* — committed in a planning document, where it
would have sent phase 2 after five defects that do not exist. The
measurement is now in the plan rather than the claim.

**What survives** is the author's ruling on the one real question
underneath it: where a type or function is genuinely shared between two
targets, it belongs in a shared crate and not in either **[author,
2026-10-04]**. Nothing in the production graph violates that today. The
three client test files reaching into `node` are the place to check it
again once phase 1 has said what those items are for.

### 2.2 The five files that are their own organization problem

| File | Lines |
|---|---|
| `client/src/ceremony.rs` | 3,744 |
| `codec/src/schema.rs` | 2,700 |
| `transport/src/session.rs` | 2,551 |
| `ffi/src/client.rs` | 2,105 |
| `archive/src/topology.rs` | 1,736 |

Size is not a defect by itself. But a 3,700-line file holds more than one
concern or it would not need 3,700 lines, and phase 1 will have said in
writing what each of its parts is for, which is the evidence a split needs.

### 2.3 Method: analyse, change one thing, gate, re-analyse

**[ruled, author, 2026-10-04]** The public FFI surface may change where
there is reason, **as a discrete action and never on the fly**. An omnibus
refactor that finds and fixes as it goes is not safe here. The loop:

1. **Analyse.** Enumerate every change the organization needs, as a list,
   with each item's consumers named. Nothing is changed during the
   analysis.
2. **Take one item.** Work it through as a coherent refactoring: the item,
   every consumer inspected, and those that need it modified in the same
   change.
3. **Run the gate.** Including the Kotlin round trip, which is what proves
   the generated binding still holds.
4. **Redo the analysis on what remains.** The first item's change may have
   dissolved later items, created new ones, or changed their consumers.

So the enumeration is rebuilt each cycle rather than trusted to the end.
One item per cycle, one gate run, one commit.

**Before the loop**, two inputs it needs:

- **An ownership map** built from phase 1's headers: for each piece of
  durable state and each invariant, the one module that owns it. A working
  document, not comments.
- **A duplication pass**: by reading, and with a mechanical scan for
  similar function bodies across crates.

### 2.4 Risk, which is real here

High, and in three ways:

- **The FFI surface is generated into the Kotlin binding.** Moving a public
  item across a module boundary can change it. The gate's Kotlin
  round-trip step catches breakage, but a change that compiles and alters
  the binding's shape is a change the shell must follow.
- **Reviewer2's line references go stale.** Every finding filed against a
  file and line stops resolving. This work belongs between review rounds,
  not during one.
- **It is the phase with no external check.** A documentation error is
  visible; a reorganization that makes the system subtly harder to reason
  about passes every test.

The procedure in 2.3 is the answer to this risk and the author's own
prescription for it: one change, its consumers, the gate, then look again.

---

## Phase 3: reliability and corner cases

**[ruled, author, 2026-10-04]** Efficiency is out of this process: *"we
will profile the working app as the live testing continues"*. The phase is
reliability and corner-case correctness, with two additions he made to it.
The earlier draft argued the point about measurement more stridently than
it was worth; the substance was that there is no benchmark harness in the
tree and no profile to optimize against, and the ruling settles it by
putting the profiling where the load actually is.

### 3.1 The corner-case pass

Five sources, in the order they are likely to pay:

- **A systematic pass for the classes the review rounds keep finding**:
  unbounded structures a peer can grow, arithmetic that can wrap or divide
  by zero, `unwrap` on a parsed value, an error path that drops a message
  silently, an ordering assumed but not enforced. The fifth round found
  four of these in one new file; they are not rare.
- **Every refusal in the specification, checked against the code that
  should raise it.** The acceptance catalogue does this for the rules it
  carries; the gap is the rules it does not.
- **The bounds the documents state**, each tested at its edge and one past
  it. Some are; most are not.
- **Fuzz targets for the parsers that have none.** Five exist; the codec
  has more entry points than five.
- **What the field runs turn up**, which is now a live source: the third
  run alone produced two kernel defects and a node defect.

### 3.2 Hard-coded values, interrogated

**[author, 2026-10-04]** Every function is asked of each constant in it:
**should this be a parameter?** Three outcomes, and each is recorded:

- **It belongs to the specification.** It stays, and its comment names the
  document and section that fixes it. A reader must not be able to mistake
  a protocol constant for a tuning knob.
- **It is a local engineering choice with one sensible value.** It stays as
  a named constant with the reason beside it, not as a literal in a
  expression.
- **It is a policy a caller should set.** It becomes a parameter or a
  configuration field. The candidates already visible are the shell's
  timings, which were chosen at a desk and have now met hardware: the
  sixty-second verifier patience, the thirty-second link wait, the
  twenty-second camera-stall restart, the 256-byte optical part, the
  five-second write acknowledgement.

The third outcome is where the care goes. A constant made configurable
acquires a range that must be valid, a default that must be right, and a
test for both.

### 3.3 Closing: the unit tests, condition by condition

**[author, 2026-10-04]** The phase closes with an inspection of each
function's unit tests, to confirm they exercise **every allowable positive
and negative condition** the function admits. Not coverage by line, which
is satisfied by a test that never asserts; coverage by condition:

- every branch of every refusal, each reached by the input that causes it;
- every bound at its edge and one past it, both sides;
- every `Option` and `Result` arm a caller can observe;
- for a function with an ordering or a state machine, each transition the
  states admit, and a rejection for each they do not.

**What it produces** is a list of conditions with no test, which become
test rows, and conditions a test claims to cover but does not assert on,
which are the worse of the two and are invisible to any coverage tool.
Specification matters among them become catalogue entries, as the review
rounds' findings do.

**This is the largest of the three phases** and the one that cannot be
sized before phase 1 has read the code.

---

## Sequencing against everything else

- **The witness build** is queued behind this at the author's word. It is
  the thing that unblocks a second ceremony between the same two phones,
  so the longer housekeeping runs the longer the field tests stay at one
  record.
- **Reviewer2's cycle** continues. Phase 1 is safe to interleave with it;
  phase 2 is not, and wants a gap between rounds.
- **The field-test bench** is unaffected by phase 1 and will need
  re-verifying after phase 2.

**[ruled, author, 2026-10-04]** **Phase 1, then a reassessment.** The
witness build waits. `bc08f98` is the known-good build through the basic
meeting and the fallback if anything here goes wrong, and the author wants
as much hygiene done as is prudent **and the meeting re-tested** before the
ceremony is extended further.

**So phase 1 closes with a field run**, not with the tenth batch: the same
two phones through the same meeting to a record, on the swept tree, against
`bc08f98` as the comparison. A documentation pass should not be able to
change behaviour, and the run is what says so rather than the claim.

---

## What I would recommend

Run phase 1 to completion, in the ten batches above, now that the standard
is written. Let it produce its question queue and its phase-2 candidate
list. Then reassess: phase 2's scope is unknowable until the ownership map
exists, and phase 3's is unknowable until the code has been read end to
end, which phase 1 is the act of doing.

**Phase 1 is also the cheapest insurance against the kind of error this
plan made in its first draft.** Four phase-2 findings came from a
measurement nobody had read the output of. Reading every file is what
stops that, and it is the one phase whose value does not depend on what
the others find.
