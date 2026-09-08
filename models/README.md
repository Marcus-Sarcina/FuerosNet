# RHTN formal models

Stage 1 of `Robot/review-plan.md` — the formal-modelling reviews the plan
calls "the highest-value reviews in the plan and the ones least replaceable
by an LLM pass." Three tool families, fourteen artifacts, each checking claims
the design documents make analytically and had never run.

**Every result here is the tool's, not the author's.** As the plan states:
"an LLM can help *write* the model and interpret the output, but the
verification result comes from the tool." Re-run everything with
`./run-all.sh`; it exits non-zero if any check regresses.

```bash
./run-all.sh          # builds and checks all fourteen; writes results/
```

Current status: 3 Python assertion families, 5 TLA+ models (invariants +
temporal properties), and **8 Tamarin theories in two trees** — `wire-only/`
(34 lemmas) and `compliant/` (29 lemmas).

**Three obligations verify only over a bounded model**, all in `compliant/`:
`no_issuance_for_a_key_this_issuer_superseded` in `currency.spthy`, and
`nothing_served_under_a_credential_this_node_superseded` /
`nothing_delivered_after_supersession` in `attach.spthy`. They share one cause:
the serving and issuing rules consume a linear capability and restore it, so
the backward search for its origin regresses through unboundedly many prior
operations. Source invariants closed that regress for the neighbouring
properties in both theories and not for these three.

**All three are discharged in TLA+ instead**, by
`tla/SupersessionDiscipline.tla`, which checks them exhaustively over every
reachable state of a finite instance. They are safety properties of a mutable
local state machine, which is what TLC enumerates for a living; Tamarin's
difficulty was isolated to a three-rule theory carrying nothing but the
pattern — delete the restore and the same lemma verifies in four steps, keep it
and no budget or heuristic terminates. That is a fact about the backward
search, not about the claim.

They are also verified in Tamarin under a bound (33, 84 and 46 steps), by
`.bounded` fragments that `run-all.sh` **appends to the real theory** so the
rules keep one source. That check is weaker than TLC's, and it earns its place
by checking the *Tamarin* rules — the ones the other lemmas in those theories
rely on — rather than a separate TLA+ encoding of them.

---

## Two Tamarin trees, and why

`tamarin/` holds two directories that prove different things. Keeping them
apart was a decision [author, 2026-09-06] after a review asked which kind of
property the suite was for; the answer was both, separately.

**`wire-only/`** — properties a third party can check from bytes on the wire.
A relying party receives a record, a staple, an attach response, and these
theories say what it can conclude from the cryptography alone. This is the
tree whose results transfer to an adversary who wrote their own client,
because nothing in it depends on anyone keeping a promise.

**`compliant/`** — that an honest node's **own stated obligations are mutually
coherent**. `light-client-requirements.md` and `infra-client-requirements.md`
are lists of commitments, several of which the design says plainly that nobody
can verify: *"Nobody can check this for you, and this is a commitment rather
than an enforceable rule"*; *"The wire cannot check this — you can, and you are
the only party [who can]."* A `compliant/` lemma is **not** a claim that
compliance is checkable. It says that a node doing what it promised cannot
reach a state its own commitments forbid — which is worth knowing, because the
commitments were written separately, in two documents, over months, and had
never been read against one another by anything but a person.

The distinction matters when quoting a result. A green `wire-only/` lemma
survives a hostile counterparty. A green `compliant/` lemma does not: it
describes a node that kept its word, and says nothing whatever about one that
did not. Every `compliant/` theory carries at least one lemma asserting that
the non-compliant trace is **still reachable**, so the tree cannot quietly
start proving that the wire enforces what the design says it cannot.

**How the boundary is decided, and it is not by argument.** Each `wire-only/`
theory carries a rule letting a **legitimate principal misbehave with its own
uncompromised key** — signing a presence body it never met for, or a
recognition it never earned. Add that rule and any lemma resting on client
conformity falsifies immediately, naming itself. That is how seven lemmas were
moved out of `wire-only/` [2026-09-06]: four from `ceremony`
(`presence_requires_copresence`, `no_remote_forgery`,
`copresence_binds_one_roster`, `formation_requires_copresence`) and three from
`recovery` (`key_alone_insufficient`, `recognition_names_the_meeting_key`,
`recovery_requires_a_meeting`). None was wrong; each was in the wrong tree.

**Key compromise is not dishonesty**, and conflating them was the error
underneath. `Compromised(P)` means someone stole P's key. It cannot stand for
"P used P's own key contrary to client policy" — that needs no theft. Carving
the residual out under the compromise name made those theorems read stronger
than they were. In `compliant/` the carve-out is `SkippedTheMeeting`, named for
what it is.

**What the split has already produced.** `compliant/currency.spthy`'s main
obligation — *"issue only for the key you currently record"* — is **false read
on its own**. The adversary can hand an issuer back a key it already
superseded, and the issuer then issues for it in full compliance, because it is
once again the key it records. What forbids that is written in a different
document and binds a different party: the light client's *"never take a series
reissue into a series you have occupied before"*, which a counterparty holding
the chain can enforce. The infra node's rule is coherent only in company. No
`wire-only/` theory could have found this — an attestation issued after the
walk-back is perfectly well formed, and no relying party can see the history.

---

## What each model checks, and against what

### 1. `simulation/flow_metric.py` — the trust metric (design §16.2, §16.3.1, §17.3)

Pure-Python (stdlib only) graph calculation of the reference max-flow /
min-cut trust metric. Runs the four claims §16.2 argues on paper:

| Experiment | Design claim | Result |
|---|---|---|
| E1 | distance-decay diverges unless λ < 1/f | the 0.5/0.05 figures reproduce (~19,500× vs ~2×), and the exact boundary fλ=1 is classified as divergent (it grows linearly) |
| E2 | a region is bounded by its cut regardless of population, **where its entry lies beyond the horizon** | best individual score stays ≤ the boundary capacity as the fake region grows to 341 identities. The same region entered from *inside* the horizon is unthrottled instead — measured, and reported, because that is the bound's edge rather than a violation of it (§16.2.1) |
| E3 | **setwise conservation** (normative, 2026-09-03), **beyond the horizon** | a region of 4→32 identities behind **one acquired peering edge** from a horizon member — a patronage tree at §3.1's fanout and ordinary edge capacity, since locator-derived reach exposes patronage structure and nothing else; visible, outside the horizon, sharing one cut the attacker bought once. The independent-per-target sum grows with population (32→168) while the conserving joint saturates at the cut (4→8→8→8) and general demands deliver 8 of 168 asked; the same region gated behind a node *outside* the observer's horizon is invisible and cannot inflate anything |
| E4 | edge-influence amortises (§16.3.1) | with horizons over adoption+sibling scope only, one visible peering edge influences several observers and no observer that cannot see it — every cross-tree placement enumerated. **Demonstrated in one toy topology, not measured as economics**: it shows amortisation exists, not how many edges reach a target fraction of a population |

**Parallel edges collapse.** design §16.2.1: *"capacity belongs to the pair,
not to the count of relationships between them"*. `FlowGraph.add_edge` keeps
the larger of two capacities for one pair rather than summing them — which
stopped being a nicety once §6.1.1 required a proof of presence alongside every
adoption, since every patron and subordinate then became PoP counterparties
too and every hierarchical edge in the network would have carried double. The
regression case asserts a pair joined by an adoption and two later meetings
carries one edge's capacity; restore summing and it fails.

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

### 2d. `tla/SupersessionDiscipline.tla` — the supersession discipline (design §12.6.5)

Where three obligations live that Tamarin could not discharge: currency's
*"issue only for the key you currently record"* and both halves of §12.6.5's
rule on sessions and queues. They are safety properties of a **mutable local
state machine**, and TLC enumerates every reachable state of two nodes and
three key generations — 100 distinct states, exhaustive.

**What makes the invariants non-vacuous.** `Issue` and `Serve` are *unguarded*:
a node issues for whatever its record holds and serves whatever its session
holds, and a flag records whether that key had already been superseded. The
invariant is therefore a claim about the state machine's shape, not a
restatement of an action's precondition — which is exactly what §12.6.5 means
by *"enforced by replacement rather than by a check"*.

`superseded` is accumulated history, not `everHeld \ {record}`. Derived, the
current generation would be excluded **by definition** and a walk-back onto a
superseded key would satisfy the invariant silently.

**And the mutation must fail.** `SeriesCheck = FALSE` drops the light client's
*"never take a series reissue into a series you have occupied before"* — a rule
in another document binding another party — and TLC then violates
`NeverIssuedForASupersededKey`. `run-all.sh` fails if that violation stops
happening, because a clean run would mean the invariant had quietly stopped
depending on the cross-document rule.

### 3. `tamarin/` — cryptographic properties (Tamarin, symbolic model)

Symbolic (Dolev-Yao) protocol analysis: perfect cryptography, the network is
the adversary. Each `.spthy` is commented for a first-time Tamarin reader and
opens with the reading conventions. Every theory begins with an
`exists-trace` executability lemma — the guard against a model that cannot run
the honest protocol and so proves every security lemma vacuously.

- **`wire-only/attach`** (design §14, wire §9.1) — session attach.
  **Authentication is mutual**: a client that completes an attach
  authenticated the server it intended, and a serving node's bound session
  names a client that proved control of its key over that server's own
  challenge. **Queued material is released only to the transport-authenticated
  peer** (`queued_data_reaches_only_the_authenticated_peer`) — wire §9.1
  requires `Attach` field 1 to equal the connection-authenticated keyhash,
  *"[otherwise a party] could claim any keyhash and receive another node's
  queued messages"*. The authenticated identity and the claimed identity are
  separate terms in the model, which is what lets the attack be expressed at
  all; delete the equality check and that lemma falsifies while
  `client_authentication` stays verified. Seven lemmas.

  `client_commit_is_injective` is named that rather than `no_replay`, because
  it follows from linear-fact consumption rather than from the signature —
  measured by deleting the signature check, which falsifies
  `server_authentication` and leaves this one verifying.

- **`wire-only/currency`** (design §12.6.5) — currency attestation + stapling.
  A trust-bearing acceptance of a key as current requires an unexpired
  issuance for that exact key, expiry modelled as event order. The attestation
  **binds its issuer identity and claimed role**, so one issued as a sibling
  cannot be re-presented as a patron's; and **acceptance names an issuer the
  relying party's own records authorise** for that subject in that role.
  `!Authorised` is indexed by the relying party, because trust is per-observer
  (§16.1) and no topology is shared — a party outside the horizon holds no
  such record and accepts nothing on this ground. Six lemmas; both new ones
  mutation-tested.

  **Known limitation, stated in the file:** there is no notion of a *current*
  key here, so an issuer can mint a fresh attestation for a key the subject
  rotated away from. §12.6.5 keeps expiry and supersession apart; this model
  has the first. The second is `compliant/currency` and
  `tla/SupersessionDiscipline`.

- **`wire-only/recovery`** (design §9.1) — what a **patron** concludes from
  recovery evidence: **the bindings**. The target is that patron's evidence
  gate, not a complete accepted transaction — the successor's envelope
  signature, the Adoption body and the remaining structural checks sit outside
  it, which the file states at the acceptance rule. The old-key proof names *this* patron
  (`successor_statement_binds_the_patron`) — drop the third element of the
  successor statement and one proof is accepted by two patrons. Acceptance
  implies a recognition naming *that* successor
  (`recognition_binds_the_successor`) — without which "one observed proof
  would authorise an unlimited number of competing successors". A recognition
  with no key authorisation never recovers
  (`recognition_alone_insufficient`). Six lemmas.

  It also models the **other** evidence route an adoption may carry (design
  §6.1.1): a former patron's countersignature, with
  `transfer_statement_binds_the_destination` checking that the signed
  `TransferStatement` names *this* destination. Drop the destination and only
  that lemma falsifies — the same replay primitive the successor statement
  closes, one field over.

  **What it deliberately does not claim** is that a recovery happened at a
  meeting. `wire-format.md` §4.1 says a verifier *"can be mistaken or lying
  and nothing checks it"*, so that is a conformance property and lives in
  `compliant/recovery`.

- **`wire-only/ceremony`** (design §7–8) — the presence ceremony.
  **Attributability**: a record a third party accepts was really signed, over
  the body as modelled, by each party the model names — both participants and
  the witness — or that party's key was stolen
  (`an_accepted_record_is_attributable`, and the formation counterpart). Six
  lemmas. The signed term is a **projection** of the wire body and the signer
  set is **one witness** against the wire's sixteen; both limits are stated in
  the file, and no refinement argument connects the projection to arbitrary
  permitted bodies.

  **Not co-presence.** design §7 says *"bilateral collusion is unpreventable"*
  and §7 that proof of presence is *"a cost, not an unforgeable primitive"*,
  which *"bilateral collusion defeats … regardless"*. The theory therefore
  carries `Participant_Sign_Without_Meeting` — a legitimate principal signing a
  body it never met for, its own key, no compromise. Attributability is what
  survives that, and it is what makes presence *"a cost imposed on acquiring
  edges into territory the attacker does not already control"* bite: a
  fabricated record is a signed lie by named parties.

---

## The axioms these proofs rest on (stated, not hidden)

Two properties are physical facts no symbolic model can represent, so they
enter the models as **axioms** — restricted rules the theorems are proved
*relative to*. This is deliberate and follows the plan's instruction to model
the co-presence channel "as one the adversary can only use when a co-location
fact holds ... the target property becomes provable relative to that axiom,
which is honest."

- **Co-presence** (`ceremony`): the `Meet` rule mints co-presence tokens **for
  the two participants only**, and nothing else in the theory reads ground
  truth — a witness signs whatever roster it is handed, because §7.6 says two
  colluding parties can simulate the exchange and no witness can tell. The witness signs as a remote notary and
  consumes none — design §7.6: *"Witnesses notarise; they do not verify
  proximity … the record format must not imply otherwise."* The theorem
  proves the protocol admits a record *only when a meeting occurred or
  someone held a party's key*; it does not prove physical co-presence is
  unforgeable, which is a real-world matter. **And "held a party's key"
  covers a willing owner as well as a thief** — in the symbolic model there
  is no other way to sign, so the carve-out is where §7.6's bilateral
  collusion lives, not merely §18.3's theft.

- **Face recognition** (`recovery`): an honest verifier recognises the true
  person. Encoded as **two** facts kept apart — persistent `!Met(V,S)` for past
  acquaintance, and a linear `AtMeeting(V,S)` minted by `Recovery_Meeting` for
  the present encounter, which honest recognition consumes. The theorem proves
  recovery *structurally* requires both a key and a human recognition; it
  cannot prove a look-alike is impossible — that residual is design §18.3's
  colluding/deceived-counterparty case, carved out as a compromised verifier.

- **A trustworthy clock** (`currency`, design **A33**): the
  `NotExpiredBeforeAccept` restriction discards traces where an acceptance
  follows its epoch's expiry. A restriction *removes* traces; it does not show
  the protocol prevents them, so the theorem holds **relative to** a relying
  party that enforces expiry. The design registers the same dependency: a
  node's internal timing is unconstrained and other mechanisms use it too, so
  what singles this one out is that a *security* decision turns on it.
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

- **A fifth review found the same defect in `ceremony`, and it was predicted.**
  The previous round ended by noting that the gate catches neither an
  unreachable honest path nor an executability lemma that permits compromise,
  and that widening those guards was the obvious next hardening. It was not
  done, and the next review found exactly that in `ceremony`.

  `Meet` minted a fresh `~cid` and never published it, while `Witness_Sign`
  and `Accept_Record` take the record body **from the network** — a change made
  two rounds earlier to remove the witness's truth oracle and to bind `cid`.
  So no honest ceremony could reach acceptance: nobody could construct a body
  containing a value nobody had published. All six lemmas still verified,
  through traces where participant keys were stolen and the adversary chose
  its own id. Machine-confirmed: an honest-completion probe was **falsified,
  no trace found**, for both the normal and formation paths. A presence record
  is published — witnesses are handed it, evaluators fetch it — so `Meet` now
  outputs the id, and `honest_ceremony_completes` / `honest_formation_completes`
  are the guards that were missing.

  **Every theory now carries an honest-path guard** that excludes compromise,
  which closes the class rather than the instance: `attach` and `recovery`
  turned out to be reachable honestly, but nothing had been checking.

  **The fourth finding was a question, and the documents answer it.** The model
  took the successor key from `In(newkey)` — a network input — so an adversary
  chose which key an honest verifier's recognition attached to. §7.1's channel
  list settles it: channel 3 is optical, *"QR codes exchanged screen-to-camera"*,
  and it *"carries key exchange"*; a recovery meeting is a ceremony (§9.1). The
  verifier reads the successor off the screen in front of it, so there is no
  network step to occupy — substituting the key means putting a device in the
  room, which is §7.6's co-presence residual rather than a network attack. The
  model over-approximated; the meeting now carries the key, and
  `recognition_names_the_meeting_key` checks it. Restoring the network input
  falsifies only that lemma.

  Two further findings held. `presence_requires_copresence` carved out a
  compromised **witness**, which §7.6 says attests that the protocol ran and
  *not* that two humans shared a room — so a stolen witness key should not buy
  up the participant claim, and the stronger form verifies unchanged. And
  "issue fresh, never extend stale" was asserted in `currency`'s comments while
  nothing enforced it: `!Epoch` is persistent, so a patron could keep stamping
  an epoch that had already expired. It is now a restriction.

- **A fourth Tamarin review: three green currency lemmas, none about the
  honest protocol.** Seven findings, static again; the first is the most
  serious defect any round has produced.

  **`currency` could not execute its honest path at all.** The attested field
  was modelled as the subject's fresh *secret* `~sk`, and issuance emitted only
  the signature — so nothing ever output `~sk` and no relying party could
  assemble the tuple `Accept_Currency` demands. The wire object carries a
  **public keyhash** (`current_key`), not key material. The lemma still
  "verified", which is why three review rounds missed it: the trace ran through
  `Compromise_Patron`, the adversary forging an attestation with a stolen key.
  So the anti-vacuity guard was satisfied by a compromised trace and the
  security lemma by its own compromise disjunct. Issuance now emits the whole
  staple over `pk(sk)`, and `currency_is_usable_honestly` — an acceptance with
  **no compromise anywhere** — is the guard that regression cannot pass.
  Machine-confirmed both ways: it verifies now, and against the old modelling
  it is *falsified, no trace found*.

  **`key_alone_insufficient` excluded the stolen key it is named for.** Its
  antecedent carried `not(Ex V. Compromised(V))` with `V` unbound — "no
  identity anywhere was compromised" — so instantiating `V = S` excluded the
  thief. It now permits `Compromised(S)` and excludes only the patron and the
  verifier actually relied on. The stronger form still verifies, so the
  property held; it simply was not being tested. `recovery_requires_a_meeting`
  had the same loose quantifier and is tied to `ReliedOn`.

  **`Accept_Formation` did not check participant distinctness**, which
  `Accept_Record` does. Enforcing it only at `Meet_Formation` is not enough —
  acceptance takes its body and signatures from the network, so `P1 = P2` was
  acceptable and one signature satisfied both slots.

  Three findings were about claim width rather than rules, and each is
  narrowed at the point it was overclaimed: `attach` calling its missing
  direction "symmetric" when what is absent is the application-to-channel
  binding wire §9.1 requires; `recovery` quoting a Stage 1.1 target whose
  "outweighs" half is comparative and belongs to the flow metric; and
  `ceremony`'s no-forgery lemma reading as a claim about any adversary when
  its antecedent means *conforming participants*. `Met` is labelled the
  environmental assumption it is.

- **A third Tamarin review: past acquaintance is not present recognition.**
  Six findings, static again, three of them real model defects.

  **`recovery` let a thief draw a recognition out of an honest verifier.**
  §9.1 asks the subject to *"meet, in person, someone they have met before"* —
  two facts — and the model had only the past one, a persistent `!Met(V,S)`
  that `Recognise` read forever. So a thief holding the stolen key completed a
  recovery with **verifier and patron both honest and no meeting at all**:
  machine-confirmed in 9 steps. The file called that §18.3's
  colluding/deceived residual, which it was not — nobody colluded or was
  deceived, the rule simply fired. Now `Recovery_Meeting` mints a linear
  `AtMeeting` token that recognition consumes, and
  `recovery_requires_a_meeting` states what that buys. Mutation-tested:
  reverting to history-only falsifies **only** the new lemma, which is why the
  other four never caught it. What it deliberately does not claim is *who*
  turned up — a thief taken for the subject satisfies it too, and that residual
  stays §18.3's.

  **`ceremony` gave the witness a truth oracle.** `Witness_Sign` was premised
  on `!Ceremony`, which only the physical `Meet` creates, so an honest witness
  could notarise only a meeting that really happened — while §7.6 says
  colluding parties can simulate the exchange and *no witness can tell*. The
  witness now signs whatever roster it is handed. Removing the oracle exposed
  that the model had been leaning on it for something else: `DistinctParties`
  was enforced only at `Meet` and reached acceptance transitively, so the
  degenerate roster P1=P2=W became assemblable from one signature. A real
  validator checks role distinctness itself (wire §3.2), so `Accept_Record`
  now does. All six lemmas verify, and the co-presence theorem now rests on
  the **participants'** tokens alone, which is what the design claims.

  **Two theories claimed targets they do not close**, each contradicted by its
  own file lower down. `currency`'s introduction said that once the patron
  rotates and the lifetime passes no attestation makes the old key current —
  but there is no rotation state and no time, as the note above its security
  lemma already said. `attach`'s said it *was* the Stage 1.1 sibling-authority
  target, while the paragraph above it explains it cannot reach authorization.
  Both introductions now say what is proved and name what is not.

  The remaining finding tightened `recognition_binds_the_successor`, whose
  compromise escapes were untimed and named any verifier rather than the
  relied-on one.

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

## Not open as a wire property: currency supersession is client behaviour

**Superseded [author, 2026-09-06] in one respect only.** Everything below
remains true -- neither half is enforceable, and no `wire-only/` theory can
carry it. What changed is that unenforceable-by-a-third-party stopped meaning
unmodellable: `compliant/currency.spthy` and `compliant/attach.spthy` now model
the two halves as the commitments they are, and ask whether a node keeping them
can still reach a state they forbid. That is a different question from the one
answered here, and this section's answer to *its* question stands.


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
