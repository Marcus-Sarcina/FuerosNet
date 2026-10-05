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

Documentation, counted as `missing_docs` counts it: every public function,
type, constant, module **and every enum variant and public struct field**.
Measured after batch 1.

| Crate | Items | Documented | | Owed |
|---|---|---|---|---|
| `archive` | 527 | 243 | 46% | 284 |
| `crypto` | 89 | 46 | 51% | 43 |
| `sim` | 112 | 63 | 56% | 49 |
| `transport` | 332 | 193 | 58% | 139 |
| `participant` | 42 | 25 | 59% | 17 |
| `adaptors` | 63 | 38 | 60% | 25 |
| `policy` | 177 | 107 | 60% | 70 |
| `client` | 932 | 565 | 60% | 367 |
| `node` | 618 | 391 | 63% | 227 |
| `ffi` | 297 | 194 | 65% | 103 |
| `cli` | 60 | 46 | 76% | 14 |
| `daemon` | 98 | 76 | 77% | 22 |
| `resources` | 22 | 20 | 90% | 2 |
| `codec` | 208 | 208 | **100%** | **0** |
| **Total** | **3,577** | **2,215** | **61%** | **1,362** |

**The first draft of this plan undercounted by half.** It said 2,150 items
and 650 owed, because its regex matched declarations and not the variants
and fields inside them, which is two thirds of what the lint actually
requires. The table above is the lint's own count. `codec` was 33% by this
measure, not 45%.

**The base is still better than the number suggests.** Every source file
that declares a module but one carries a header, and the headers are the
good part of this codebase's documentation: they say what the file is for,
cite the document that required it, and record the decision behind it.

**Nothing enforces anything.** There is no `missing_docs` lint in any
crate, so the 30% can grow without the gate noticing.

### 1.1b What phase 1 came to

**Every Rust crate is at zero owed with `#![warn(missing_docs)]` on, in
every feature flavour, and the Kotlin shell is at zero by its own counter**
[2026-10-04].

**"In every feature flavour" is there because the first claim left it
out.** The sweep built each crate with its default features and reported
zero, and five items behind feature flags were untouched: the two JSON-line
layers' constructors in `client/src/diag.rs` and `daemon/src/diag.rs`,
which only exist under `fieldtest`, and three items in
`ffi/src/harness.rs`, which only exist under `harness`. The gate found the
harness three and printed them; nobody found the other two until the
flavours were built one at a time. **This is `CLAUDE.md`'s own failure
mode** — asserting a sweep is complete without running it — and it cost a
whole gate run.

**The ratchet had the same hole, and the gate now closes it**
[author, 2026-10-05]. Stage 3b ran `cargo clippy --workspace --all-targets
-- -D warnings`, which is every target of the default features and not
every flavour. `--all-features` is not the fix: `releasable` and
`fieldtest` are two flavours of one build and never both
(`crates/README.md`), so enabling both enables neither honestly. What was
added instead:

- **Stage 3b2, one lint pass per flavour**: `fieldtest` across the four
  crates that have flavours in a single invocation, and `harness,cli` for
  `rhtn-ffi`. **Linted, not built and not tested** [author, 2026-10-05]:
  *"the fieldtest variant gets rebuilt repeatedly as a consequence of
  testing at the same time field test features are being added, so we
  don't owe that extra effort to each non-fieldtest commit."* Stage 3 and
  the test suite stay on the releasable flavour alone.

**Stage 4b was left as it is, and no longer needs changing.** It printed
the three `harness.rs` warnings and still reported `ok`, which is how they
survived; but the code it compiles under the `harness` feature is now
linted by 3b2, which reports the same items in the form the rest of the
gate reports them. Making the Kotlin stage fail on warnings in its own
output would also make it fail on warnings from the generated binding,
which is nobody's source.

### 1.1c What the Rust stage did on a missing comment, and what it does now

**The question** [author, 2026-10-05]: *"Throwing a warning without
halting seems fine for missing documentation; running the gate is time
consuming, so it's probably a good idea to finish it and then go back and
add all missing docs than have the thing halt at the first missing doc
instance it finds and then have to re-run the gate to reveal the next
one."*

**It never halted at the first instance.** `missing_docs` is a `warn`-level
lint, and rustc reports every instance in a crate in one pass before it
stops. The old stage would have listed all of a crate's missing comments
together.

**It did something worse, one level up.** The gate escalates with `-D
warnings`, so each missing comment became an *error*, and a crate with an
error **does not compile** — which means every crate above it in the
dependency graph is never linted in that run. The graph is eight layers
deep with `codec` at the bottom, so **one missing comment in `codec` would
leave the other fourteen crates unlinted**, and a real lint anywhere in
them would be invisible until the comment was written and the gate run
again. That is the author's objection, with a sharper edge than the
question gave it: not the first instance of one kind of finding, but every
instance of every other kind.

Confirmed rather than reasoned: a `len() == 0` planted in `codec/src/bounds.rs`
produced `could not compile rhtn-codec (lib)` and no diagnostics from
anything that depends on it.

**A third thing, smaller.** The old stage piped clippy through `tail -20`,
so even when it did collect a whole list, the output showed the last twenty
lines of it.

**What it does now.** One pass, `-D warnings --force-warn missing-docs`:

| Finding | Verdict | Where it appears |
|---|---|---|
| any real lint | halts the gate | rendered as clippy renders it |
| a missing comment | non-halting | listed `file:line`, with a count |

`--force-warn` is the only flag that can exempt a single lint from `-D
warnings`, because a source-level attribute overrides `-A` and `-W` from
the command line — `#![warn(missing_docs)]` in the crate root beats both,
which is why neither works here. It costs one thing: it also overrides the
suppression rustc applies to code from a macro expansion, and
`#[uniffi::export]` generates a struct, a field and a function per exported
trait, so the FFI's device module alone reports 27 items that exist in no
source file. `Robot/tools/lintreport.py` drops those — a diagnostic whose
primary span is inside an expansion — and so restores the rule the plain
attribute applies.

**Both behaviours were proved on this workspace, not assumed.** A planted
undocumented constant reported and did not halt; a planted `len() == 0`
halted and was rendered in full.

**The count is read from the tool and not from its output** [author,
2026-10-05]. The first version grepped the report for `| missing
documentation`, and the report carries diagnostics rendered by clippy,
whose context lines reproduce arbitrary source — so the pattern was one
any source file could contain. A function whose own line ends in a comment
spelling it made the count say one item was owed where none was, and three
where two were. `lintreport.py --count-file` writes the number, and
nothing parses the prose.

**What the gate caught that nothing else did.** Two failures on the first
run, both artefacts of turning the lint on rather than of any comment:

- `cargo fmt`, a double blank line where the lint went into
  `client/src/lib.rs`; and
- `cargo clippy`, `empty_line_after_doc_comments` in `ffi/src/lib.rs`,
  where a blank line sat between a module's comment and the module. **That
  one mattered**: a detached doc comment documents nothing, and the
  `missing_docs` count had read it as attached.

A documentation pass cannot change behaviour, which is the argument for
running the gate once at the end. Both of these say the argument is for
running it, not for trusting it unrun.

**Three checkers the gate did not run at all, now stage 1d**
[author, 2026-10-05]. `refcheck.py`, `stalecheck.py` and `matrixcheck.py`
were no part of `crates/check.sh`, whose document stages were `pincheck`,
the acceptance catalogue, `countcheck`, `citecheck` and `modelpincheck`.
So the rule `CLAUDE.md` states first — *"Reference checking runs with no
exemptions. Every `§N` in every document, in every direction, against
actual headings"* — was enforced by nothing on a commit, and its failure
mode, a reference that resolves silently to the wrong document, is
invisible on reading.

Run by hand after this phase's edits it flagged five, every one a bare
`§N` this plan's own new sections used to point at their neighbours. **A
working file has no default document by design** (`refcheck.py`'s own
note: no default fits all of them), so a bare marker there is always a
flag, and this file's existing form drops the `§` entirely — *"see 1.4"*.
The five read that way now and refcheck is at 0 flags over 4,563
references.

All three are **non-halting** in the gate [author, 2026-10-05]: each
reports a drift in the prose rather than a defect in the code.
`refcheck` and `matrixcheck` exit 1 on a flag, which stage 1d counts and
does not block on; `stalecheck` never exits non-zero, so its summary line
is the whole of its result.

The present state is checked by hand and recorded here:

| Crate | Flavours checked |
|---|---|
| `client`, `daemon`, `participant` | `releasable`, `fieldtest` |
| `ffi` | `releasable`, `fieldtest`, `cli`, `harness` |
| the other eleven | no features to vary |

The final run of the three crates the batch table had at the end found more
than the table said, in one direction and for one reason worth recording:
**`client` owed 420 and not 367, because the lint had never been turned on
there.** The table's figures came from a regex, and the regex and the lint
disagree. Where they disagreed, the lint was right. The counts that matter
now are the two the tools produce: `cargo build` with the lint on, and
`Robot/tools/kdoccheck.py`.

| | Owed at the plan's measurement | Owed now |
|---|---|---|
| Rust, 15 crates, default features | 1,362 | 0 |
| Rust, the feature flavours | not counted | 0 |
| Kotlin, 27 files | 105 | 0 |

**The Kotlin side has no lint, so it has a counter instead.**
`Robot/tools/kdoccheck.py` reports what the shell owes: every public
declaration a reader outside the file reaches, with no comment above it.
It is not in the gate — nothing stops the number growing — and the honest
thing to say is that the Kotlin side is held by re-running the counter and
not by the build. It started at 105 and is at 0.

Writing the counter took four corrections, each a case where Kotlin spells
a local the way it spells a public member: a `val` in a function body, a
`val` under a multi-line parameter list, a default parameter value holding
both a `=` and a lambda, and a `private class`'s own fields. Each one
inflated the count. The first number it printed, 479, was wrong by a factor
of four.

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

| Batch | Crates | Owed | Notes |
|---|---|---|---|
| 1 ✅ | `codec` | 138 | **done 2026-10-04**: 265 lines, no questions raised, lint on |
| 2 ✅ | `crypto`, `policy` | 113 | **done 2026-10-04**: no questions raised, lint on |
| 3–4 ✅ | `archive` | 284 | **done 2026-10-04**: lint on |
| 5 ✅ | `transport` | 139 | **done 2026-10-04**: lint on |
| 6–7 ✅ | `node` | 227 | **done 2026-10-04**: lint on |
| 8–10 ✅ | `client` | 420 | **done 2026-10-04**: 420 and not 367; the lint found what the regex missed |
| 11 ✅ | `adaptors`, `ffi` | 128 | **done 2026-10-04**: lint on |
| 12 ✅ | `cli`, `daemon`, `sim`, `participant`, `resources` | 104 | **done 2026-10-04**: lint on in all five, and in `acceptance` |
| 13 ✅ | the Kotlin shell | 105 | **done 2026-10-04**: counted by `Robot/tools/kdoccheck.py`, 122 unit tests pass |

**Calibrated on batch 1**: 138 items and 265 lines of comment in one
sitting, with the crate's tests and lint clean at the end. Twelve batches
remain at that rate.

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

### 1.3b The phase-2 candidates the sweep actually met

Gathered as they came up, in the form 2.3 below wants them: one concept,
how
many owners, where they are.

**One concept with eight owners: the short form of a key.** Every
diagnostic line in the tree names a party by the first eight hex characters
of its keyhash, and eight files define that function independently, under
three names:

| Name | Where |
|---|---|
| `hex8` | `adaptors/src/courier.rs`, `client/src/ceremony.rs`, `client/src/sequence.rs`, `transport/src/session.rs`, `daemon/src/service.rs` |
| `id8` | `client/src/diag.rs`, `node/src/diag.rs` |
| `id_of` | `ffi/src/types.rs` (the FFI's own, which crosses the boundary) |

The FFI's is a boundary translation and belongs where it is. The other
seven are one function. **A candidate for the shared library**, which is
where the author put this class of thing when it came up over the `client`
and `node` edge: *"Seems like whatever this is belongs in the shared
libraries instead of the specific target implementations"* [author,
2026-10-04].

**Hex encoding itself, spelled out in 26 files across 11 crates.** The
loop `b.iter().map(|x| format!("{x:02x}")).collect()` and its inverse
appear that often. `cli/src/inspect.rs` and `participant/src/terminal.rs`
hold byte-identical public copies.

**Two observer interfaces in the shell, with one method each.**
`Front.Ui` and `Meet.Ui` are both `interface { fun render() }`, bound and
unbound by the same four methods on two classes. Whether that is one
concern or two is a question for phase 2 rather than an answer.

**The question queue came out empty, and that is a weaker claim than it
sounds.** No batch raised a question for the author. The reason 1.3
expected the opposite and got this is that almost every bound and refusal
already carried a comment naming the document that set it or marking itself
a chosen value — the convention working, rather than nothing having been
chosen. But the sweep read each declaration to describe it, which is not
the same as interrogating each value for a missing rationale; **that is
phase 3's 3.2 below and it has not been done.** An empty queue here means
nothing stopped the sweep, not that there is nothing to ask.

**Dead code: not looked for, and the gate does not cover it.** 1.3
listed
it as a by-product and it is not one. `clippy -D warnings` fires on an
unused *private* item, so the gate covers those; nothing in the gate or in
this sweep would notice a `pub` function no caller anywhere reaches, which
is the kind this tree could accumulate. **Finding it needs a pass of its
own**, and the honest state is that none has run.

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

### 1.4b What batch 1 found

**`codec`, done 2026-10-04.** 138 items, 265 lines of comment, no
behaviour changed, tests and `clippy -D warnings` clean, the lint on.

- **No question for the author.** Every bound traced to `wire-format.md`
  §1.3, which carries its own reasoning for a third of its rows; every
  role tag to `wire-format.md` §1.1's table; both algorithm identifiers to
  `wire-format.md` §2.2; the delegation window to `wire-format.md` §8.2
  and design §12.6.5. The crate was
  documentable from the documents alone. **This will not hold higher up**, where the code made
  choices the documents do not reach.
- **A convention the crate already had, now kept.** `lib.rs` declares
  *"section references in this crate are to `wire-format.md` unless
  prefixed `design`"*, and the files then cite bare `§N`. That is better
  than repeating the filename several hundred times, and the conventions
  section was written to permit it: a crate that declares its default in
  the header may use the short form beneath it.
- **The lint found what the counter missed**, which is how the
  undercounting above came to light: 64 variants and fields the regex had
  never seen. The measurement now uses the lint's definition.

### 1.4c A tool the sweep acquired

**`Robot/tools/docput.py`**, after string-matching proved too fragile at
this volume: it takes `file:line` and the comment's text, inserts above
the item's attributes rather than between them and the item, matches the
indentation, works from the bottom of a file up so earlier line numbers
stay valid, and **refuses a line already documented**, so a stale plan is
caught rather than applied twice.

Two mistakes it was built in answer to, both made and both caught:

- A batch of twenty-one replacements aborted on its fifth and wrote
  nothing, because one pattern had a typo. All-or-nothing on a string
  match is the wrong failure mode for a sweep.
- A comment landed **between** a `#[derive]` and its item, which compiles
  and reads wrongly. The tool now walks back over attributes; the one
  instance was fixed by hand.

**`Robot/tools/docplan.py`**, built after a stale plan put comments inside
two function bodies: it generates the plan's skeleton from `cargo build`'s
own `missing_docs` output, so a line number is never transcribed by hand.

**`Robot/tools/kdoccheck.py`**, the Kotlin counter, since no lint there can
do it. 1.1b above records what writing it cost.

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

**Status, 2026-10-05.** The thirteen batches are done and the gate passes:
`fmt` clean, `clippy -D warnings` clean, `deny` clean, `cargo test` ok, the
five fuzz targets no crash over 1.49M runs, the Kotlin round trip ok, 122
Android unit tests, 0 failed. `refcheck` run by hand: 0 flags. **The field
run is the one thing outstanding, and it needs the phones.**

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
