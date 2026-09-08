# `simulation/`

## 1. What this model demonstrates

- E1: trust divergence under distance decay follows §16.2's arithmetic — for the **adoption tree**, whose branching is `f`.
- That `λ < 1/f` is **necessary and not sufficient** for the graph a decay policy actually evaluates: at the configured `f = 10`, `λ = 0.095` satisfies the criterion and still diverges at branching 11, since §16.2.1's acquaintance edges carry trust by the same rules and have no bound.
- E2: an observer's trust in a target is bounded by the min-cut between them, and the bound holds at a scope boundary outside the horizon.
- E3: setwise conservation holds over the identities an observer can actually see.
- That the conservation bound rests on the whole set entering **one** computation: two disjoint halves of a region admitted separately draw 8 + 8 against a cut of 8, where the same identities admitted together draw 8.
- E4: the influence of a single edge on the metric, under §16.2.1's horizon semantics, counted over **third-party** observers — the two endpoints of the peering relationship are excluded, since one already holds the pair's presence edge and the other would be evaluating its own standing.
- Parallel edges collapse: a pair joined by both an adoption and repeated meetings carries one capacity, not a sum.
- Reach walks hierarchical edges only; visibility does not compose across observers.
- Landscape distance is computed from flow adjacency alone.
- §16.4's three-pass ranking — available flow, then path length, then consideration order — resolves allocation ties where the max-flow value is unique but the allocation is not.

## 2. What it cannot demonstrate

- Anything cryptographic. There are no keys, signatures or messages.
- That any implementation computes the metric this way. This is the reference calculation, not a conformance test.
- How the metric behaves across topology families. Results are per generated tree and per RNG seed.
- How many introductions reach a target fraction of a population, or how successive cover sets overlap.
- Anything about time, liveness, or message delivery.
- That an implementation's **materialisation lifetime** respects the conservation bound. The metric is a pure function of a graph; whether a policy that scores principals at discrete events retires or retains prior allocations is outside it, and the design's rule — *"one conserved computation, not one computation per principal"* — is what governs there.
- That the parameter values used are the right ones. They are inputs, not findings.
