# RHTN formal models

Stage 1 of `Robot/review-plan.md` — the formal-modelling reviews the plan
calls "the highest-value reviews in the plan and the ones least replaceable
by an LLM pass." Three tool families, eight artifacts, each checking claims
the design documents make analytically and had never run.

**Every result here is the tool's, not the author's.** As the plan states:
"an LLM can help *write* the model and interpret the output, but the
verification result comes from the tool." Re-run everything with
`./run-all.sh`; it exits non-zero if any check regresses.

```bash
./run-all.sh          # builds and checks all eight; writes results/
```

Current status: **all models pass** — 3 Python assertion families, 3 TLA+
models (invariants + temporal properties), 4 Tamarin theories (17 lemmas).

---

## What each model checks, and against what

### 1. `simulation/flow_metric.py` — the trust metric (design §16.2, §16.3.1, §17.3)

Pure-Python (stdlib only) graph calculation of the reference max-flow /
min-cut trust metric. Runs the four claims §16.2 argues on paper:

| Experiment | Design claim | Result |
|---|---|---|
| E1 | distance-decay diverges unless λ < 1/f | the 0.5/0.05 figures reproduce (~19,500× vs ~2×), and the exact boundary fλ=1 is classified as divergent (it grows linearly) |
| E2 | a region is bounded by its cut regardless of population, **where its entry lies beyond the horizon** | best individual score stays ≤ the boundary capacity as the fake region grows to 341 identities. The same region entered from *inside* the horizon is unthrottled instead — measured, and reported, because that is the bound's edge rather than a violation of it (§16.2.1) |
| E3 | **setwise conservation** (normative, 2026-09-03), **beyond the horizon** | a region of 4→32 identities behind **one acquired peering edge** from a horizon member — visible, outside the horizon, sharing one cut the attacker bought once. The independent-per-target sum grows with population (32→256) while the conserving joint saturates at the cut (4→8→8→8) and general demands deliver 8 of 256 asked; the same region gated behind a node *outside* the observer's horizon is invisible and cannot inflate anything |
| E4 | edge-influence amortises (§16.3.1) | with horizons over adoption+sibling scope only, one visible peering edge influences several observers and no observer that cannot see it — every cross-tree placement enumerated. **Demonstrated in one toy topology, not measured as economics**: it shows amortisation exists, not how many edges reach a target fraction of a population |

**Three concepts kept separate.** The design is emphatic (§6.3:
"Contributing to trust and conferring scope are different things") that
*scope* topology (adoption + sibling edges → the horizon, §15.1), *trust-
capacity* topology (adoption + peering edges → the flow), and *visibility*
(a peering edge "is visible inside the two peers' horizons and nowhere
else", §16.3.1) are three different relations. The simulation represents
them separately — `scope_adjacency` / `horizon`, `FlowGraph` capacities, and
`visible_flow_subgraph`. An external cross-family review caught an earlier
version collapsing all three into one adjacency, which understated an
acquired edge's coverage several-fold and let E3 count identities the
observer could not see; the current version is the correction. That review
also brute-force-validated the `max_flow` implementation against exhaustive
minimum cuts on 700 random graphs with zero discrepancies, so the core
arithmetic is not where a defect would hide.

A later review read §16.2.1 as putting *sibling* edges in the trust-capacity
graph too; the author ruled otherwise — a sibling edge abstracts
graph-distance in the patronage hierarchy and carries no capacity — so the
split above stands and the design's wording was corrected to match. Since
that round **visibility is a parameter, not a constant**:
`visible_flow_subgraph(..., reach=n)` extends the observer's evidence *n*
shells past the peering shell, because how much graph an evaluator can
populate is available evidence rather than a protocol quantity. The metric
is the same computation at every value of `reach`; a poorer graph bounds
harder, which is §16.3.1's conservative direction.

`simulation/results.txt` is the committed run.

### 2. `tla/` — distributed behaviour (TLA+ / TLC)

Finite-state models checked exhaustively by TLC. Each `.tla` is heavily
commented for a first-time TLA+ reader; the `.cfg` beside it fixes the finite
constants and lists the invariants and temporal properties.

- **`PartitionMerge`** — the plan's first target ("the whole design rests on
  it and it has never been tested against anything but argument"). Adoptions,
  unilateral departures, best-effort gossip (loss = a frame never sent),
  network partition and heal. Checks: no node is ever behind its own store
  (`SelfTruth`), views never invent transactions (`NoInvention`), and — the
  headline — **convergence restated for a no-shared-state system**: once the
  network heals and stays healed, every pair of nodes eventually agrees about
  every subject (`Convergence`). 21,032 distinct states, no error. Adoption
  does **not** require the subject to be patronless: design §6.2 says moving
  between patrons is *"adopt at the destination, depart the origin, in either
  order"*, and the adopt-first order is the one that puts two adoptions for a
  single subject in flight across a partition at once. An earlier version
  required patronlessness and so never explored it — 15,080 states rather than
  21,032; the properties hold either way, so the restriction was costing
  coverage rather than hiding a defect.

- **`CurrencyEscalation`** — the patron→sibling→grandpatron ladder (design
  §12.6.5.1) under outage. The table's **fourth** rung (re-adopt at a new
  patron) and the light-client pre-delegation path are *not* modelled, which
  the file now states: both are escapes from the frozen state rather than
  rungs of the issuing ladder, so the liveness claim proved here is strictly
  weaker than the design's. Checks the plan's two questions: the ladder never
  deadlocks while a rung can serve (`SomeIssuerCanAct`, `LadderMakesProgress`)
  and never extends a stale attestation (`FreshOnly`, the "issue fresh, never
  extend stale" rule). Time is modelled as event order, not a clock.

- **`CycleDetection`** — rootward-memo cycle detection (design §15.2) under
  *concurrent* adoptions that each look legal against a stale local view.
  Checks that a patron cycle never persists forever (`CyclesResolve`): the
  memo reaches the node it names as its own ancestor and a reason-5 disavowal
  breaks the loop, even with message loss repaired only by replay.

All three run with `-deadlock` (checking off) because the models legitimately
terminate — quiescence is a valid end state, not an error; the properties
that matter are the invariants and temporal properties, checked regardless.

### 3. `tamarin/` — cryptographic properties (Tamarin, symbolic model)

Symbolic (Dolev-Yao) protocol analysis: perfect cryptography, the network is
the adversary. Each `.spthy` is commented for a first-time Tamarin reader and
opens with the reading conventions. Every theory begins with an
`exists-trace` executability lemma — the guard against a model that cannot run
the honest protocol and so proves every security lemma vacuously.

- **`attach`** (design §14, wire §9.1) — session attach. A client that
  completes an attach authenticated the server it intended; **a sibling (or
  any party without the server's key) cannot impersonate the server**. Three
  lemmas. **This is endpoint authentication, not authorisation**: the theory
  has no patron/sibling role, session mode or trust-bearing operation, so it
  cannot express the failover case where a client knowingly attaches to a
  sibling and trust-bearing operations must stop anyway. The third lemma is
  named `client_commit_is_injective` rather than `no_replay`, because it
  follows from linear-fact consumption rather than from the signature —
  measured by deleting the signature check, which falsifies
  `server_authentication` and leaves this one verifying.

- **`currency`** (design §12.6.5) — currency attestation + stapling. A
  trust-bearing acceptance of a key as current requires an unexpired patron
  issuance for that exact key, expiry modelled as event order. Three lemmas.
  **Known limitation, now stated in the file:** there is no notion of a
  *current* key, so a patron can mint a fresh attestation for a key the
  subject rotated away from — Tamarin confirms the trace. §12.6.5 keeps
  expiry and supersession apart and warns against conflating them; this
  model has the first and not the second. See "What the models found".

- **`recovery`** (design §9.1) — recovery adoption. **Neither factor alone is
  recovery**: a key proof with no honest recognition never recovers
  (`key_alone_insufficient` — the stolen-key case), and a recognition with no
  key authorisation never recovers (`recognition_alone_insufficient`). A
  fourth lemma, `recognition_binds_the_successor`, checks wire §4.1's rule
  that a verifier response names the key it attests continuity to — without
  which "one observed proof would authorise an unlimited number of competing
  successors". *See "What the models found," below — this one earned its
  keep twice.*

- **`ceremony`** (design §7–8) — the presence ceremony, the plan's
  highest-value target. **A record a third party accepts implies the named
  participants were co-present** (`presence_requires_copresence`,
  `no_remote_forgery`), and one meeting's signatures cannot be transplanted
  onto a different roster (`copresence_binds_one_roster`). Four lemmas.

---

## The axioms these proofs rest on (stated, not hidden)

Two properties are physical facts no symbolic model can represent, so they
enter the models as **axioms** — restricted rules the theorems are proved
*relative to*. This is deliberate and follows the plan's instruction to model
the co-presence channel "as one the adversary can only use when a co-location
fact holds ... the target property becomes provable relative to that axiom,
which is honest."

- **Co-presence** (`ceremony`): the `Meet` rule mints co-presence tokens **for
  the two participants only**. The witness signs as a remote notary and
  consumes none — design §7.6: *"Witnesses notarise; they do not verify
  proximity … the record format must not imply otherwise."* The theorem
  proves the protocol admits a record *only when a meeting occurred or
  someone held a party's key*; it does not prove physical co-presence is
  unforgeable, which is a real-world matter. **And "held a party's key"
  covers a willing owner as well as a thief** — in the symbolic model there
  is no other way to sign, so the carve-out is where §7.6's bilateral
  collusion lives, not merely §18.3's theft.

- **Face recognition** (`recovery`): an honest verifier recognises the true
  person, encoded as `!Met(V,S)` gating honest recognition. The theorem proves
  recovery *structurally* requires both a key and a human recognition; it
  cannot prove a look-alike is impossible — that residual is design §18.3's
  colluding/deceived-counterparty case, carved out as a compromised verifier.

- **A trustworthy clock** (`currency`, design **A33**): the
  `NotExpiredBeforeAccept` restriction discards traces where an acceptance
  follows its epoch's expiry. The design registers the same dependency —
  §12.6.5's expiry is decided against the relying party's own clock, the one
  place a clock is load-bearing, and wire §3.3 checks timestamps against none.
  A restriction *removes* traces; it does not show the protocol prevents
  them. So the currency theorem holds **relative to a relying party that
  enforces expiry**, and `expiry_is_reachable` shows only that an expiry can
  occur — not that every attestation eventually expires.

Key theft is modelled explicitly (a `Compromise` rule) and appears as a named
carve-out in every security lemma, so the boundary of each guarantee is
visible in its statement — a stolen key defeats it, which is the design's own
residual, not a flaw in the mechanism.

---

## What the models found

The exercise is worth more than a row of green checks; two models pushed back.

- **`recovery` (a modelling gap that confirmed a load-bearing design rule).**
  The first version let a verifier sign a recognition over the *new key* and
  omitted the "subject is never its own verifier" rule. Tamarin falsified the
  security lemma with a trace where a thief holding the subject's key signed
  *both* factors — self-recognition. The fix was to match the design: the
  verifier recognises the *person* (the old identity), the new key comes only
  from the old-key proof, and wire §5.3's distinctness rule is enforced. The
  counterexample confirmed wire §5.3 is load-bearing for recovery specifically,
  not only for ordinary presence.

- **`recovery` (a lemma restatement forced by the stolen-key case).** A
  combined "both factors" lemma was falsified by a thief producing the key
  proof via adversary signing (the honest rule never fires). That surfaced the
  model's true boundary — with the key stolen *and* a verifier recognising the
  identity, recovery succeeds, because the symbolic model cannot represent a
  face check. The honest, provable form is the two mirror impossibilities
  (neither factor alone suffices), which is exactly how design §9.1 states it.

Both are recorded here rather than silently fixed, because the counterexample-
then-diagnose loop *is* the value of the exercise.

- **A second Tamarin review, and the gate that let a malformed lemma pass.**
  Nine findings, again filed without a prover. Two were mine from the round
  above, and one of those is the important one.

  **A free timepoint variable, introduced by my own regex and passed by the
  regression gate.** Time-bounding the compromise carve-outs put `#k < #i`
  into a lemma quantifying `#i1` and `#i2`. Tamarin reports this as a
  *wellformedness warning*, **exits 0, and still prints "verified"** — so
  `run-all.sh`, which grepped only for verified/falsified, called it a pass.
  The gate now fails on wellformedness failures and is tested both ways
  (reintroduce the free variable → exit 1; clean files → pass). It
  immediately found two **pre-existing** unbound variables nobody had
  reported: `cid` in `Accept_Record` and `sk` in `Accept_Currency`, both now
  bound from the received record, which is what a relying party actually has.

  **The old-key proof did not bind the patron.** wire §4.1 fixes the payload
  as `SuccessorStatement = [prior_key, new_key, patron_key]` and requires a
  verifier to check the last two against the adoption, *"an unchecked binding
  being the same as no binding"*. The model signed `<'rotate', S, newkey>`,
  so one proof assembled a recovery under **any** patron — machine-confirmed,
  two patrons accepting one proof with no compromise. Fixed, with
  `successor_statement_binds_the_patron` to check it. Worth recording how the
  fix went: my first attempt updated the message pattern but **silently failed
  to update the signature check**, so the rule carried the patron and never
  verified it — the exact defect wire §4.1 warns about, reproduced by
  accident. Two lemmas falsified, and reading the counterexample rather than
  guessing was what found it.

  **Formation ceremonies were outside the model.** wire §4.5 field 6: a
  formation record *"has empty witness and verifier arrays permanently"*, so
  requiring a witness signature left the co-presence theorems silent about a
  structurally legal record. Added as its own branch with
  `formation_requires_copresence`.

  **`key_alone_insufficient` never exercised the thief.** Its antecedent
  demanded the *honest* `OldKeyProof` action, which a thief signing with a
  stolen key never fires — excluding the case the lemma is named for.
  Dropping the conjunct makes it strictly stronger and it still verifies.

  **One symbolic name could hold several keys.** `Register` could fire twice
  for one label, so "the party I meant to reach" pinned nothing. A
  one-registration-per-name restriction is added to all four theories;
  `currency`'s `Subject_Key` is deliberately exempt, a subject holding several
  keys over time being that theory's subject.

  Two findings restate limitations the files already declare (the co-presence
  axiom, and currency's missing current-key state); a third, expiry-by-
  restriction, is now listed with the other axioms rather than read as a
  result.

- **A Tamarin review found four models claiming more than they proved.** The
  reviewer could not run the prover and worked statically; every finding
  below was then reproduced or refuted *with* Tamarin here.

  **`currency` has no notion of a current key** — `!SubjKey` and `!Epoch` are
  both persistent, so a patron can mint a brand-new attestation for a key the
  subject rotated away from. Machine-confirmed: a trace exists where an epoch
  begun *after* the successor appeared carries an acceptance of the older key,
  with no compromise. §12.6.5 keeps expiry and supersession apart in an
  author ruling — *"Expiry bounds what a stolen credential can spend;
  supersession, once known, is what retires it from use"* — and warns that
  conflating them reads as licence to keep serving a binding the server knows
  is dead. The model had exactly that conflation. Closing it needs a linear
  current-key binding that rotation consumes plus a relying party that can
  hold supersession evidence; a first attempt showed it also needs
  rotate-back forbidden (wire §4.1: *"prior_key MUST differ"*) before the
  property is tractable. **Open work, stated in the file rather than
  silently carried.**

  **`ceremony` made the witness physically present.** `Meet` minted a
  co-presence token for the witness and `Witness_Sign` consumed it, so the
  notary had to attend — against §7.6's *"Witnesses notarise; they do not
  verify proximity"*, a section that goes on to say *"the record format must
  not imply otherwise."* Corrected; all four lemmas verify without it, so the
  theorem is now about the participants' co-presence, which is what the
  design claims. The reviewer also read the compromise carve-out as unable to
  express a malicious-but-uncompromised owner; that half does not hold — a
  probe confirms a record accepted with **no meeting at all** via that rule,
  which is how Dolev-Yao expresses a willing colluder. The defect there was
  the commentary calling the rule theft only, and it is reworded.

  **`recovery` omitted the successor binding.** `Recognise` signed
  `<'recognise', S>` — the person, not the key — so one honest recognition
  could be assembled beside competing successors. Machine-confirmed: two
  accepted recoveries for different new keys off one recognition, no
  compromise. wire §4.1 forbids exactly this (*"one observed proof would
  authorise an unlimited number of competing successors"*) and requires the
  response to name the new subject and the prior key. The binding is added
  and `recognition_binds_the_successor` now checks it — falsified when the
  binding is removed, which is how we know it is load-bearing.

  **`attach`'s `no_replay` proved linear-fact consumption**, not replay
  resistance: deleting the signature check falsifies `server_authentication`
  and leaves `no_replay` verifying. Renamed to `client_commit_is_injective`,
  with the project's real replay concern — wire §11's ban on Attach in 0-RTT
  — recorded as out of scope. The theory's header also claimed to answer
  whether a sibling may act with patron authority, which needs role facts it
  does not have; the claim is narrowed to endpoint authentication.

  **All six compromise carve-outs were unbounded in time**, so a compromise
  *after* an authentication could discharge it. Every one is now
  `& #k < #i`, and all fourteen lemmas still verify — the properties were
  strengthened at no cost.

- **`flow_metric` (a cross-family review found three conflated concepts).**
  The first version used one adjacency relation for scope, trust-capacity and
  visibility. An external review showed this made E4 understate an acquired
  edge's coverage several-fold (the design's §16.3.1 economics) and let E3's
  saturation run over identities the observer could not see. The rewrite
  separates the three relations the design keeps in three different sections,
  and E4 now enumerates every interior placement rather than sampling. It also
  surfaced a design question the author has since **ruled** on (§16.4): the
  max-flow *value* is unique, but the *allocation* under scarce capacity is
  not. The reference metric decides in three passes — **available
  flow ranks first**, it being the metric itself; then **the shorter path
  dominates** (a longer path is less trustworthy by nature); then
  consideration order breaks **true ties only** — as *reference policy, not a
  network invariant*, since per-observer trust (§16.1) means no party
  consumes another's computation.

- **`flow_metric` (a fourth review found E2 passing on a deleted edge, and
  `reach` leaking invisible edges).** Two High findings, both confirmed by
  execution before anything was changed.

  **E2 had been confirming its bound by omitting an adoption it said had
  happened.** The experiment has an honest boundary node adopt the fake root
  and puts that edge in the *capacity* graph, then builds the *scope* graph
  from the honest tree alone — so flow knew about the adoption and scope
  pretended it had not occurred. Restore the edge and the fake root sits two
  scope edges from the observer, inside its horizon, where §16.2.1 says the
  metric does not ration: the score goes from 10 to unthrottled at every
  population. The bound was produced by the omission. E2 now places the
  boundary at the horizon's edge, carries the entry adoption in scope, and
  asserts the fake root is outside the horizon before measuring — **and runs
  the inside-horizon placement as a second case**, because "the metric does
  not bound here" is the claim's shape rather than a failure of it. §17.3's
  third leg is qualified to match; legs 1 and 2 carry no such condition.

  **`reach` made peering visibility transitive, which is the one thing it
  must not be.** §16.3 confines a peering record to "the two peers' horizons
  and nowhere else", and §16.3.1 turns that into the security property: an
  edge an observer cannot see cannot raise that observer's cut. The outward
  expansion added every peering edge incident to a newly discovered node
  without re-applying the visibility rule. Minimal counterexample, four
  nodes: `O—H` adoption, `H—G` peering visible, `G—X` peering invisible;
  at reach=0 X's standing is 0, at reach=1 it is 8. The fix follows the
  author's own words for what `reach` models — *"you can discern some of a
  foreign subtree's structure from **locator data** ... in that foreign
  **patronage** graph"* — so expansion walks hierarchical edges only, and a
  peering edge enters by the endpoint-in-horizon rule or not at all.

  **The same leak existed one layer down**, in `landscape_distance`, which
  folded the caller's raw `peer_edges` into its outward adjacency. An edge
  absent from the observer's capacity graph could still shorten a landscape
  distance, and since node capacity falls with distance, that raised a
  relay's throughput: measured at 2 → 4 on a chain whose deepest relay moved
  from distance 4 to distance 2. Distance is now computed from the same
  graph the flow is.

  Both leaks are regression-tested and **all four fixes are mutation-tested**
  — restoring each defect makes a named assertion fail — except one, stated
  because it does not: E2's entry adoption is unobservable at the corrected
  boundary placement, since the fake root is outside the horizon with or
  without it. It is the inside-horizon case that discriminates, and that one
  does.

- **`flow_metric` (a third review found the horizon was not collapsed, and
  E3 had been passing for the wrong reason).** A cross-family review reported
  that sibling relationships were missing from the capacity graph, citing
  §16.2.1's *"adoption and sibling edges ... carry trust"*. Reproducing it
  (seed 5: cut 10→30 on materialising them) put the cut under a microscope
  and showed E3's chokepoint was not an acquired edge at all but **the
  observer's own in-horizon hierarchical edge**, throttled at `HIER_CAP`
  because the graph had not collapsed the horizon. Unthrottling it made E3
  trip its own `"test is vacuous unless demand exceeds the cut"` guard:
  deliverable rose to equal the independent sum, so there was no
  conservation left to demonstrate. E2 and E4 were unaffected, bit for bit.

  The author ruled on all of it. **The horizon collapses to a single edge**
  (distance 0→1), because nodes within each other's horizons are directly
  aware of each other — so the horizon's internal topology is not in the
  flow graph at all. **Sibling edges are an abstraction of graph-distance in
  the patronage hierarchy, not edges in the trust graph**, so the reported
  finding was a defect in §16.2.1's wording rather than in the model, and
  the design was corrected instead. **How far outward a graph reaches is
  available evidence, not a protocol quantity** — hence `reach` on
  `visible_flow_subgraph`, a modelling parameter the metric is identical at
  every value of. E3 was rebuilt around the only shape conservation speaks
  to: a region behind one edge bought once.

  The collapse also **dissolved a second finding from the same review**
  without a line of allocation code changing. Ranking candidates by hops in
  the *uncollapsed* graph re-graded the inside of the horizon: two
  candidates at landscape distance 2 measured 3 and 5, and the deeper one
  lost under either consideration order. In the collapsed graph they measure
  3 and 3, tie, and consideration order decides — because **hops in the
  collapsed graph are the landscape distance**. §16.4 now names the graph.

- **`flow_metric` (a second review caught the allocation rule unimplemented).**
  A follow-up cross-family review showed the first pass was satisfied only by
  accident and the second not at all: a plain multi-sink max-flow gets
  shortest-path-first free from Edmonds-Karp, but decides equal-length ties by
  the order edges sit in the *graph's own adjacency* — an artifact of how the
  topology was built, not of candidate order. Measured directly: two
  equidistant candidates passed in the order [B, A] still admitted A. The
  simulation now ranks candidates explicitly and tests all three passes,
  including that construction order does not leak in; §16.4 carries the
  implementation note. The author's follow-up correction added the first
  pass: available flow must rank before path length, since the flow *is* the
  measurement and a chokepoint example cannot reveal the omission (every
  candidate behind one saturated cut carries the same flow). That pass is
  mutation-tested — removing the key makes the new case fail — because a test
  that passes with and without the thing it tests proves nothing. The same review also
  rebuilt E4 around genuine two-ended cross-tree peering (an earlier
  one-ended attacker made "visible ⇒ influenced" true by construction; with
  both endpoints real, 14–28 observers see an edge while only 8–12 are
  influenced), and separated the unit-demand admission count from the
  general-demand conservation statement the design actually makes.

---

## Not open: currency supersession is client behaviour

Three reviews in a row filed `currency.spthy`'s missing "current key" as a
**model gap**, and a rebuild was attempted and reverted before the author
named what it actually is: **both halves of §12.6.5's supersession rule are
unenforceable client behaviour, so there is no protocol property here for a
symbolic model to prove.**

- The **issuer's** half — *issue only for the key you currently record* — is
  unenforceable because a currency attestation names a subject, a key and an
  epoch, and carries nothing that distinguishes one issued before a rotation
  from one issued after.
- The **relying party's** half — *stop serving a binding you have verified
  superseded* — is unenforceable for the ordinary reason that it governs what
  a party does with its own records.

**And the two never meet.** A party that could detect a stale issuance is one
holding the recovery adoption — and such a party has already overwritten its
own record (§9.0.2), so it rejects on that and never consults the staple. A
party that would consult the staple is outside the horizon, where old and new
keys are separate entities by design and there is nothing to detect.

So the model proves what the protocol enforces: the signature and the epoch.
The obligations live where obligations live — `infra-client-requirements.md`
§3 for the issuer, §2 for the relying party — and §1.1's test is the whole of
the answer: *a rule aimed at a party you share no state with is a wish.*

**The lesson is the one the working notes already record**: when a finding
assumes a component, ask whether the component is required. Three reviewers
and this assistant took the component as given and generated work from it; the
repair was a sentence in a requirements document.

---

## Environment

Built and checked 2026-09-04 with user-local installs (no root required):

| Tool | Version | Source |
|---|---|---|
| TLC (TLA+) | tla2tools (latest) | github.com/tlaplus/tlaplus |
| Tamarin | 1.12.0 | github.com/tamarin-prover |
| Maude (Tamarin backend) | 3.5.1 | github.com/maude-lang/Maude |
| Java (for TLC) | Temurin JRE 21 | adoptium.net |
| Python | 3 (stdlib only) | system |

`run-all.sh` reads tool paths from `JAVA`, `TLA_JAR`, `TAMARIN`, `MAUDE_DIR`
env vars, defaulting to `~/tools/`. GraphViz (`dot`) is optional — only for
rendering Tamarin counterexample graphs, not for proving.

---

## What this does not do (from the plan, restated so it is not over-trusted)

- It does not validate the **social** claims (that competent patrons get
  selected for, that trust networks stabilise) — only deployment does.
- It does not substitute for a **cryptographic implementation audit** by
  people who do that for a living. These are *protocol-level* symbolic proofs;
  they say nothing about a concrete implementation's constant-time behaviour,
  side channels, or library correctness.
- **A finding's absence is weak evidence.** These models cover the four
  protocol cores the plan names; they are a filter, not a verdict, and the
  small finite instances (3 nodes, a handful of events) could in principle
  miss a defect that needs a larger configuration to appear.
