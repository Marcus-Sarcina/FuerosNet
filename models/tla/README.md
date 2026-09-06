# `tla/`

## 1. What this model demonstrates

- `PartitionMerge`: every node's view is its own (`SelfTruth`); no node invents state it was never told (`NoInvention`); views converge once messages flow again (`Convergence`).
- `CurrencyEscalation`: no state exists in which a node's attestation is expired, an issuer up the ladder is reachable, and no rung is enabled (`SomeIssuerCanAct`); no action extends a stale attestation instead of issuing a fresh one (`FreshOnly`); the ladder makes progress while the patron is eventually reachable (`LadderMakesProgress`).
- `CycleDetection`: authority cycles resolve (`CyclesResolve`).
- `SupersessionDiscipline`: a node never issues for a key generation it has superseded, never serves under one, and neither its record nor its session is ever a superseded generation — over every reachable state of two nodes and three generations.
- That the supersession result is load-bearing on the subject's series rule: removing it violates `NeverIssuedForASupersededKey`.

## 2. What it cannot demonstrate

- Anything cryptographic. Signatures, keys and message authenticity are outside these models.
- That results for the finite instances checked generalise to arbitrary numbers of nodes, keys or generations.
- Real elapsed time. Clocks advance only by explicit steps and lifetimes are counted in ticks.
- Re-adoption at a new patron, or a light client's pre-delegated issuance — neither rung is modelled.
- That any implementation follows these state machines.
- Partial or Byzantine failure. Nodes here are either reachable or not.
