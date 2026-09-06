# `simulation/`

## 1. What this model demonstrates

- E1: trust divergence under distance decay follows §16.2's arithmetic.
- E2: an observer's trust in a target is bounded by the min-cut between them, and the bound holds at a scope boundary outside the horizon.
- E3: setwise conservation holds over the identities an observer can actually see.
- E4: the influence of a single edge on the metric, under §16.2.1's horizon semantics.
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
- That the parameter values used are the right ones. They are inputs, not findings.
