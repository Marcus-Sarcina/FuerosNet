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
models (invariants + temporal properties), 4 Tamarin theories (13 lemmas).

---

## What each model checks, and against what

### 1. `simulation/flow_metric.py` — the trust metric (design §16.2, §16.3.1, §17.3)

Pure-Python (stdlib only) graph calculation of the reference max-flow /
min-cut trust metric. Runs the four claims §16.2 argues on paper:

| Experiment | Design claim | Result |
|---|---|---|
| E1 | distance-decay diverges unless λ < 1/f | the 0.5/0.05 figures reproduce (~19,500× vs ~2×) |
| E2 | a region is bounded by its cut regardless of population | best individual score stays ≤ the boundary capacity as the fake region grows to 341 identities |
| E3 | **setwise conservation** (normative, 2026-09-03) | the independent-per-target sum grows with population; the one-conserving-computation joint saturates at the region ceiling and stays flat — the exact property the 0.8.3 review made normative |
| E4 | edge-influence is coverage, not per-target (§16.3.1) | one acquired edge changes exactly the observers whose horizon contains it, and no others |

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
  every subject (`Convergence`). 15,080 states, no error.

- **`CurrencyEscalation`** — the patron→sibling→grandpatron ladder (design
  §12.6.5.1) under outage. Checks the plan's two questions: the ladder never
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
  any party without the server's key) cannot impersonate the server**, and
  no replay. Three lemmas.

- **`currency`** (design §12.6.5) — currency attestation + stapling. A
  trust-bearing acceptance of a key as current requires an unexpired patron
  issuance for that exact key; **a rotated-away key whose attestation has
  expired cannot be made to look current**, expiry modelled as event order.
  Three lemmas.

- **`recovery`** (design §9.1) — recovery adoption. **Neither factor alone is
  recovery**: a key proof with no honest recognition never recovers
  (`key_alone_insufficient` — the stolen-key case), and a recognition with no
  key authorisation never recovers (`recognition_alone_insufficient`). Three
  lemmas. *See "What the models found," below — this one earned its keep.*

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

- **Co-presence** (`ceremony`): the `Meet` rule mints the co-presence tokens a
  ceremony's binding signatures require. The theorem proves the protocol
  admits a record *only when a meeting occurred or a key was stolen*; it does
  not prove physical co-presence is unforgeable, which is a real-world matter.

- **Face recognition** (`recovery`): an honest verifier recognises the true
  person, encoded as `!Met(V,S)` gating honest recognition. The theorem proves
  recovery *structurally* requires both a key and a human recognition; it
  cannot prove a look-alike is impossible — that residual is design §18.3's
  colluding/deceived-counterparty case, carved out as a compromised verifier.

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
