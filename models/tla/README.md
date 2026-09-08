# `tla/`

## 1. What this model demonstrates

- `PartitionMerge`: every node's view is its own (`SelfTruth`); no node invents state it was never told (`NoInvention`); views converge once messages flow again **and topology changes stop** (`Convergence`), over bindings held per patron relationship, so a node may hold two at once and depart the one it names.
- `CurrencyEscalation`: no state exists in which a node's attestation is expired, an issuer up the ladder is reachable, and no rung is enabled (`SomeIssuerCanAct`); an attestation's validity is renewed only by an issuer that is reachable under its rung's conditions in that step (`FreshOnly`); every expiry is followed by renewal while the patron, a given sibling, or the grandpatron is the operative rung from some point on (`LadderMakesProgress`, `SiblingRungServes`, `GrandpatronRungServes`).
- `CycleDetection`: authority cycles resolve (`CyclesResolve`), on three nodes and on four, where a cycle node has a child that is not on the cycle. The memo is the patron's statement about its own subordinate slot, it fires where field 1 comes home, the detector confirms it against its own row and cuts the subordinate that handed it over, and a repaired pair may be adopted again.
- `SupersessionDiscipline`: a node never issues for a series the subject's chain shows was left, never serves under one, and neither its record nor its session is ever such a series — over every reachable state of two nodes, three series and chains of length three, each node holding some prefix of the subject's chain and ordering by chain length.
- `IssuerAuthorisation`: a relying party never accepts a currency attestation on an issuer role it has withdrawn or replaced (`NeverAcceptedOnALapsedAuthorisation`).
- That the results are load-bearing on the rules that hold them up: removing the series rule violates `NeverIssuedForASupersededKey`, and so does removing chain-length ordering (a shorter chain then replaces a longer one, and a series the node never itself recorded is issued for); treating authorisation as permanent rather than current violates `NeverAcceptedOnALapsedAuthorisation`; allowing the grace period design §12.6.5.1 rejects violates `FreshOnly`; letting repair cut any subordinate rather than the one that handed the memo over violates `CyclesResolve` on four nodes.

## 2. What it cannot demonstrate

- Anything cryptographic. Signatures, keys and message authenticity are outside these models.
- That results for the finite instances checked generalise to arbitrary numbers of nodes, keys or generations.
- Real elapsed time. An attestation's age advances only by explicit steps and lifetimes are counted in ticks.
- Convergence under a topology that keeps changing. `PartitionMerge` bounds topology events, which is what makes them stop; the property is conditional on that and says nothing without it.
- A relay or chokepoint. Every unpartitioned pair of `PartitionMerge` nodes exchanges frames directly, so no reconciliation failure that depends on forwarding through a third party is reachable.
- A patron's slot row disagreeing with its subordinate's own binding. `CycleDetection` has one relation standing for the relationship and models no departure, so the two cannot diverge there; nor is the timestamp rule that stops a replayed memo at the first table-holding hop modelled.
- Two simultaneously current attestations from different issuers. The subject holds one attestation, so that state is not representable here.
- Key rotation, so a fresh attestation from a reachable issuer and an extension of that same issuer's earlier attestation are one state; `FreshOnly` checks only that renewal needs a live issuer under its rung.
- That every edge a cycle repair cuts lies on a live cycle. A memo matching the detector's current row can still describe a route that has since changed, which design §18.2 accepts and bounds rather than prevents.
- Patron equivocation. The subject in `SupersessionDiscipline` has one chain and every node holds a prefix of it.
- Re-adoption at a new patron, or a light client's pre-delegated issuance — neither rung is modelled.
- That any implementation follows these state machines.
- Partial or Byzantine failure. Nodes here are either reachable or not.
