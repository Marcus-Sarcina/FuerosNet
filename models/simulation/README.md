# `simulation/`

## 1. What this model demonstrates

- E1: trust divergence under distance decay follows §16.2's arithmetic — for the **adoption tree**, whose branching is `f`.
- That `λ < 1/f` is **necessary and not sufficient** for the graph a decay policy actually evaluates: at the configured `f = 10`, `λ = 0.095` satisfies the criterion and still diverges at branching 11, since §16.2.1's acquaintance edges carry trust by the same rules and have no bound.
- E2: an observer's trust in a target is bounded by the min-cut between them, and the bound holds at a scope boundary outside the horizon.
- E3: setwise conservation holds over the identities an observer can actually see, the region being a patronage tree at §3.1's fanout and ordinary edge capacity, which is the structure locator-derived reach exposes; the construction is checked against §3.1's bound on the adoption graph itself.
- That the conservation bound rests on the whole set entering **one** computation: two disjoint halves of a region admitted separately draw 8 + 8 against a cut of 8, where the same identities admitted together draw 8.
- E4: the influence of a single edge on the metric, under §16.2.1's horizon semantics, counted over **third-party** observers — the two endpoints of the peering relationship are excluded, since one already holds the pair's presence edge and the other would be evaluating its own standing — and under **both evidence states**: where the observer lacks the pair's presence record, and where it already holds it, in which case the change is zero in every placement, as §16.3 says.
- Parallel edges collapse: a pair joined by both an adoption and repeated meetings carries one capacity, not a sum.
- Reach walks hierarchical edges only; visibility does not compose across observers.
- Landscape distance is computed from flow adjacency alone.
- §16.4's three-pass ranking — available flow, then path length, then consideration order — resolves allocation ties where the max-flow value is unique but the allocation is not.

## 2. What it cannot demonstrate

- Anything cryptographic. There are no keys, signatures or messages.
- That any implementation computes the metric this way. This is the reference calculation, not a conformance test.
- A general marginal influence for acquiring a peering relationship. The count is conditional on the observer's evidence state — substantial where it lacks the pair's presence record, zero where it holds it — and which state a given observer is in is per-observer and not modelled as a population.
- How the metric behaves across topology families. Results are per generated tree and per RNG seed.
- How many introductions reach a target fraction of a population, or how successive cover sets overlap.
- Anything about time, liveness, or message delivery.
- That an implementation retains no standing between evaluations. The metric is a pure function of a graph, and §16.2 now says outright that *"nothing here is a retained entitlement"* — the bound holds for the set an evaluation scores. A policy that keeps prior results and scores the next batch afresh is outside that rule, which the conservation regression exhibits (8 + 8 separately against a cut of 8) and cannot prevent.
- That the parameter values used are the right ones. They are inputs, not findings.
