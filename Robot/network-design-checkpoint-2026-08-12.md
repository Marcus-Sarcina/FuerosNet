# Reconfigurable-Hierarchic Trust Network — Design Document

**Status:** Frozen checkpoint — network/protocol layer as of 2026-08-12, before the proof-of-presence discussion. Do not edit; see network-design.md for the living document.
**Last updated:** 2026-08-12

> This document is the working context for the project. It records what has been
> decided, what has been ruled out and why, and what remains open. It is written
> to be handed to a fresh session (Claude Code or otherwise) as a starting brief.

---

## 1. Thesis

Build a peer-to-peer network that requires **no global trust and no globally
shared state**, so that multiple graphs of any size can coexist, partition, and
merge arbitrarily over time.

The guiding idea: *strong interfaces can replace shared truth*. Any number of
incompatible interpretations of reality can coexist productively so long as the
data model through which they communicate is well-specified. Cooperation does
not require agreeing on a universal ledger — it requires a well-specified
boundary object. (Cf. Star & Griesemer on boundary objects; Masnick,
"Protocols, Not Platforms"; the end-to-end argument.)

The network is intended to **formalize real-world relationships**, not replace
them. It assumes and depends on a significant face-to-face human component.

A useful one-line framing for outsiders: **a reconfigurable PKI**. X.509 is
already a hierarchy of countersigning authorities; the innovation here is that
subjects can unilaterally defect from their authority and reattach elsewhere,
and nobody is obligated to accept any given root.

### Primary use case

Most traffic is expected to be **non-logged activity within a user's ±2 tier
"Dunbar Org"**, using the network as a **DNS and SSL replacement** — naming and
authentication grounded in social attestation rather than in certificate
authorities. Protocol-defined high-importance transactions should be far less
frequent than one per 100 seconds per user.

This framing matters for engineering: the metric to optimize is **resolution
latency for a stranger's key**, not bandwidth.

---

## 2. Scope

### In scope for v1
- Client application (light, end-user devices)
- Server application (always-up infrastructure nodes)
- Core transaction types: adoption, transfer, peering
- Addressing, resolution, and routing
- Reference trust metric (flow-based)

### Explicitly deferred
- Additional transaction types beyond the core set
- "Costly transactions" proving face-to-face meeting (planned, not specified)
- Multiple identities per client (possible later version)

### Non-goals
- No tradable token. No asset to steal.
- No global ledger, no consensus, no canonical version of truth.
- No cold lookup of an identity by public key alone (see §7.4 — this is a
  deliberate property, not a gap).

---

## 3. Vocabulary

| Term | Meaning |
|---|---|
| **Node** | A participant identified by a persistent keypair |
| **Patron** | A node's immediate upstream node |
| **Subordinate / down-line** | Nodes beneath a node in the hierarchy |
| **Sibling** | A node sharing the same direct patron |
| **Infra node** | A node running the server application on a statically routed device |
| **Peer** | Cross-tree infrastructure partner (voluntary, see §8) |
| **Anchor** | A node whose subtree exceeds the anchor threshold; globally routable |
| **Dunbar Org** | A node's ±2 tier neighbourhood (~121 nodes at f=10) |

---

## 4. Topology

### 4.1 Structure
- Persistent identities; **reconfigurable hierarchy**.
- Each node has **one patron** and at most **f = 10** subordinates.
- A node may **soft-fork**: add a second patron while remaining in the first
  patron's down-line. The structure is therefore a **DAG, not a strict tree**.
  Multiple independent certification paths are stronger evidence than one —
  this is the property PGP's web of trust got right.
- Root nodes are **emergent, not a special class**. Today's root may sign on to
  a larger tree tomorrow.

### 4.2 Why f = 10 (social, not technical)
Effective span of control in human organisations is ~8 subordinates; 10 allows
an 8-person team with headroom so a new addition doesn't force a contemporaneous
exit. It also keeps the ±2 tier neighbourhood (≈121 nodes) inside Dunbar's
number. **The cap applies to infrastructure nodes as well**, preserving the
property that an infra node is a *person's* node rather than a capacity pool.

This is affordable only because the tree carries control and attestation, not
payload — apex load scales with churn and introductions, not with usage.

### 4.3 The infrastructure tier
- To have more than **L = 2 levels** of subordinates (>110), a user's key must
  be associated with at least one statically routed device running the
  infrastructure application.
- **Each server corresponds to a user.** Infra operators are people.
- Expected deployment: container or VM image, mostly in cloud datacentres, each
  instance requiring a unique static IP. **No centralised operator.**

### 4.4 Replication
- **Siblings** replicate each other's traffic (up to f−1 = 9). Authorised
  implicitly by the patron's adoption transaction — no separate agreement.
- Sibling replication provides **locality and latency, not fault independence**.
  Every replica set contained within a subtree has a vertex cut of 1 (the
  subtree root). Cousins share a grandparent; same problem one level up.
- Therefore: **at least 2 cross-tree peers** (§8) are required for genuine
  redundancy — replicas reachable without traversing one's own ancestors.
- Peer selection must consider **ASN and cloud region**, not only tree distance.
  Two nodes in different subtrees but the same availability zone are not
  independent, and most infra will live in a handful of clouds.
- Replication depth (how far up/down metadata propagates) should be computed
  from expected traffic. Floor: every user's messages replicate on their nearest
  infra node and that node's siblings.

---

## 5. Cryptography

**Decision:** post-quantum from the start, but tiered by actual urgency.

| Layer | Choice | Rationale |
|---|---|---|
| Transport | PQ KEM (e.g. ML-KEM) | "Harvest now, decrypt later" is a real threat to confidentiality |
| Long-lived root / patron identity | Hybrid classical + PQ | Long validity window justifies the cost |
| High-volume routine transactions | Classical, with documented migration path | Signatures need only hold until nobody relies on them |

Sizing constraint that drove this: ML-DSA signatures run 2.4–4.6 KB against
Ed25519's 64 bytes. With two signatures on every transaction that is a 40–70×
inflation on the most common object in the system.

**Consequence for the anchor table (§7.2):** the table must store *key hashes*,
not full public keys, or the PQ key sizes blow the 25 MB budget by ~18×.

---

## 6. Transactions

Policy for evaluating evidence is pluggable. **The evidence schema is not.**
What a transaction contains, what each signature covers, and how history is
canonically serialised must be fixed, or nodes cannot evaluate each other's
evidence regardless of how compatible their policies are.

**Provenance must survive as typed categories.** A peering edge, a countersigned
transaction, and a face-to-face attestation are different kinds of evidence and
must remain distinguishable to any consumer of the schema — the protocol must
never pre-collapse them into a single scalar. This is a protocol requirement,
not a UI requirement: no interface can display provenance the data model has
already discarded, and the intended norm (trust flows from substantial
out-of-network history, not from having peered with a stranger) depends on the
distinction being queryable. What any given client *does* with that
distinction is entirely its own business — the published apps are reference
implementations and any number of alternative UIs should be expected.

### 6.1 Adoption
Patron accepts a new subordinate. Also implicitly authorises sibling peering
among that patron's children.

### 6.2 Transfer
Two signatures: the moving node and the **new** patron. The old patron does not
countersign — this is the escape hatch that makes exit a real right.

- **Hard fork:** drop old patron, adopt new; broadcast to infra nodes in both
  trees.
- **Soft fork:** add a new patron, remain in the old down-line; broadcast only
  to the new tree.

Each node signs a **monotonic sequence number** on every transfer. This is not
consensus — it is a freshness test, letting any observer order a node's own
competing claims without a clock and detect a stale record that a revocation
failed to reach.

### 6.3 Peering (cross-tree)
- **Voluntary and ad-hoc.** Unlike adoption, not required to participate.
- Two signatures, between infra nodes in different subtrees.
- Attests investment in the network and therefore contributes to trust — but at
  **lower flow capacity than hierarchical edges by default** (see §9.3).
- Carries a real cost: persisting the peer's data.

### 6.4 Countersigning
First-level patrons countersign their subordinates' transactions, except
transfers. This makes a spammer's volume chargeable against their **patron's**
reputation, so patrons become natural rate limiters — self-enforcing rather than
protocol-enforced.

### 6.5 Deferred: proof of face-to-face meeting
Deliberately costly transactions attesting that two users met in person. This is
the load-bearing Sybil defence (§10) and the mechanism that upgrades a low-trust
peering relationship into a high-trust one.

---

## 7. Addressing and resolution

### 7.1 Locator format
A locator is four fields, **signed by the node itself**:

```
{ anchor, path, sequence, signature }
```

- **anchor** — key hash of the anchor whose subtree contains the node
- **path** — position beneath the anchor; **truncatable** to a prefix
  sufficient to route to the right region (this is where aggregation savings
  come from)
- **sequence** — the monotonic counter from §6.2; detects stale cache entries
- **signature** — non-optional. If the locator is not covered by the node's own
  signature, any relay can substitute its own and silently become that node's
  mailbox.

Identity (public key) is permanent; locator is mutable. Standard
identity/locator split — see LISP and HIP for well-mapped potholes.

**Known leak:** a locator discloses the node's patron, depth, and subtree to
anyone it introduces itself to. Since graph position *is* the trust signal, this
is intrinsic rather than fixable. Document it; don't pretend otherwise.

### 7.2 Anchor set
**Anchors are defined by subtree size, not by tier.**

Tier-based definition fails because depth is measured from the root, is not
locally knowable, and shifts under merge — when a root joins a larger tree,
every node beneath it drops a tier and every cached locator referencing the old
anchors breaks at once. Since merging is meant to be routine, that is
disqualifying.

Subtree size is **invariant under merge**: acquiring a patron does not change
your subtree.

- Threshold **S ≈ 500,000** subordinates, or actual root (patronless).
- **Hysteresis:** promote at S, demote at S/2, so boundary nodes don't flap and
  force cache churn.
- Anchor count ≈ N/S. At 60 billion users (10B human + 50B AI), ~120,000
  anchors.
- **Entry format (key hash, not key):** 32B hash + 16B address + 4B subtree
  size + 8B sequence ≈ 60 bytes. 120,000 × 60B ≈ **7.2 MB**, comfortably inside
  the 25 MB budget. Full keys and signatures are fetched and verified at contact
  time — the anchor table is an *index*, not a credential store.
- The anchor set is the one piece of state every node replicates.

### 7.3 Resolution sequence

**Case 0 — First contact (dominant case).** The locator travels with the key,
out of band: QR code at a meeting, a referral from a mutual contact. No lookup
occurs. Bob pins Alice's full public key on first contact and uses the hash
thereafter.

**Case 1 — Cached locator, still valid.**
1. Bob verifies Alice's signature over `{key, anchor, path, sequence}`.
2. Bob looks up `anchor` in his local anchor table → route.
3. Bob's client hands the message to its own patron infra node, which forwards
   toward the anchor.
4. Inside the anchor's subtree, the path prefix directs descent.
5. The terminal infra node delivers, or holds for Alice if she is a light client
   currently offline.

**Case 2 — Cached locator stale (Alice moved).**
6. Alice's former patron holds a **forwarding record**: Alice's newer signed
   locator, bearing a higher sequence number.
7. Bob verifies the signature and the sequence increase, replaces his cache
   entry, and retries.
8. Chains are collapsed at the source: a former patron follows the chain and
   returns the terminal record, not the next hop. Forwarding records carry a TTL
   (proposed: 90 days), after which resolution fails rather than growing
   unboundedly.

**Case 3 — Anchor changed (Alice's subtree merged or split).**
9. Anchor promotion/demotion is a globally replicated event. Bob's anchor table
   update tells him the old anchor now sits beneath a new one; he re-resolves
   through the new anchor. This is the case that requires §7.2's merge-stable
   anchor definition.

**Case 4 — Public key only, no locator, no referral.**
**Not resolvable.** See below.

### 7.4 There is no cold lookup — deliberately

A global key→locator index is a DHT, which is rejected (§11). Nor can it be
replicated: Bloom summaries of anchor subtrees run ~625 KB per anchor, or ~12.5
GB globally at target scale — three orders of magnitude over budget.

So keys do not circulate without provenance. You reach people you have a
referral or a meeting for. **You cannot search for a person.** Discovery is
social, which is the intended behaviour, but it should be stated as a product
property because it is the single most visible difference from DNS.

### 7.5 Caching
- **Anchor table** — globally replicated, updated by gossip.
- **Dunbar Org (±2 tiers)** — pre-fetched and kept warm; this is where most
  traffic goes.
- **Contact locators** — `{key → locator, sequence, TTL, last-verified}`.
- Negative results cached briefly to avoid retry storms.

---

## 8. Control plane / data plane

Four message classes with different reach and different persistence:

| Class | Contents | Reach | Persistence |
|---|---|---|---|
| **Topology** | adoption, transfer, peering | ancestors + horizon; aggregate beyond | stored within horizon, folded into aggregate state beyond |
| **Attestation** | trust-bearing transactions | **pull, not push** | stored by participants and their patrons; fetched on demand by evaluators |
| **Liveness / routing state** | heartbeats, route updates | horizon | **process and discard** — keep the table, not the update history (as BGP keeps the RIB) |
| **Payload** | application data | point-to-point | endpoints and their patrons only |

**Attestation must be pull.** This is the single biggest scaling decision in the
design. Flooding attestations makes backbone traffic grow with total network
activity; pulling makes it grow with evaluation demand, which is far smaller and
self-limiting. It also matches the trust model exactly — you only evaluate
people you are about to deal with.

### 8.1 Horizon parameters
- **h = 2** — full topology storage (~110 nodes at f=10)
- **h = 3** — process-and-discard (~1,110 nodes)

Control gossip volume scales as f^h, so fanout and horizon are coupled: any
increase in one must be paid for in the other.

---

## 9. Trust model

### 9.1 Principle
Each node computes its own trust for every known node from the transactions it
observes, using the reference algorithm or a variant tuned to its application or
group norms. Trust fans out from a user's closest and best-attested connections;
a distant high-volume cluster is weighted only to the degree that it transacts
with users well-attested from *that user's* point of view.

Trust rules are deliberately **not inherent to the network structure** — this is
an intended locus of adaptation and evolutionary pressure.

### 9.2 The reference metric must be flow-based

**Finding:** distance-decay metrics are exploitable and the naive tuning fails
catastrophically.

For weight = λ^distance, the total weight of an attacker's fake subtree is
λ^D · Σ(fλ)^k, which **diverges unless fλ < 1**. At a plausible-sounding
λ = 0.5 with f = 10 and a depth-6 fake subtree, the attacker's mass sums to
~19,500× the weight of one honest node at the same distance — the attacker
saturates the metric, and deeper is always better for them. At λ = 0.05 the
series converges to ~2 and the whole million-node subtree is worth at most twice
a single node.

**Normative rule for the spec: λ < 1/f.** Decay per hop must be steeper than the
reciprocal of the fanout. If fanout ever becomes variable, every deployed
decay-based metric silently becomes exploitable.

**Better: max-flow / min-cut.** Every path into a subtree passes through its
root, so a subtree has vertex connectivity 1 to the rest of the graph. Under a
capacity-limited flow metric the entire subtree inherits at most what flows
through that one vertex, regardless of how many nodes it contains. Attack cost
becomes a function of the number of edges from honest nodes into the attacker's
region, not the size of the region. See **Raph Levien's Advogato attack-resistant
trust metrics** — it maps onto this structure almost without modification. It is
also **independent of f**, which frees fanout to be chosen on social and
plumbing grounds.

### 9.3 Peering edges and trust capacity
A peering edge raises a region's cut, which is exactly what bounds a subtree's
trust claim. Three peering edges into honest territory let a subtree claim four
nodes' worth instead of one. This is correct for availability and intended for
trust, but it means *"want to back each other up?"* is the cheapest
social-engineering route to raising one's own trust ceiling, and it looks like a
routine technical request rather than a request for endorsement.

**Mitigation:** peering edges carry a distinct, low default flow capacity,
separate from hierarchical edges. Raising it is a policy decision, upgraded by
face-to-face attestation (§6.5) between the peers.

### 9.4 The pluggability tension — unresolved
If security lives in the metric rather than in the structural caps, then the
*protocol* cannot be secure — only individual policies can be. A node that picks
a plausible-looking distance decay is exploitable for a few hundred dollars a
month while believing itself protected by the fanout rule.

**Proposed mitigation:** ship a flow-based reference implementation as the
default, publish the λ < 1/f criterion prominently, and provide a conformance
test that reports a policy's resistance bound so anyone tuning their own can see
what they have given up.

### 9.5 History portability
Every node, including light clients, keeps an archive of its own transactions
plus past counterparties' signatures. Because transactions are self-signed and
countersigned, history remains independently verifiable after a move — only
proximity is lost, not evidence. This is what makes exit a real right rather
than a formal one, and should be an explicit design commitment.

On joining a new tree, the node presents a **selected subset** to the new patron
rather than the whole archive; the patron verifies signatures on what it is
shown and compares against the new tree's local trust table. Transactions with
unknown counterparties are ignored; known ones contribute to initial trust
state.

Open tension: an archive presented *lacking* previously-seen transactions may
itself be read as evidence of deviousness. This probably pushes users wanting to
participate in rival networks toward creating separate identities — acceptable
for v1, given multiple-identity support is deferred.

---

## 10. Security analysis — settled findings

### 10.1 Fake subtree cost
An attacker building a full fake subtree needs **one infra node per f^(L+1)
identities** = 1,000 at f=10, L=2.

### 10.2 Topology cannot provide Sybil resistance
The identical formula applies to the honest network: N users need N/f^(L+1)
infra nodes. **Attacker cost per fake identity equals honest cost per real
identity**, because the protocol has no way to tell them apart. Every factor by
which you make the attack more expensive makes your own infrastructure burden
more expensive by the same factor.

It is in fact worse than 1:1 — the attacker builds a perfectly packed tree while
real social graphs are sparse and lopsided, so the attacker achieves the
theoretical floor and honest operators never do.

| f | Depth @1M | Max path | Sibling factor | Fakes/server | Honest servers @1M |
|---|---|---|---|---|---|
| 3 | 13 | 26 hops | ×3 | 27 | 37,000 |
| 5 | 9 | 18 hops | ×5 | 125 | 8,000 |
| **10** | **6** | **12 hops** | **×10** | **1,000** | **1,000** |
| 20 | 5 | 10 hops | ×20 | 8,000 | 125 |

**Conclusion: f is a plumbing and social parameter, not a security parameter.**

### 10.3 What actually provides Sybil resistance
Three independent mechanisms, none relying on topology rules:

1. **Face-to-face attestation** makes *identities* expensive. Proof of presence
   is the one resource an attacker cannot parallelise. This is the strongest
   leg and it is not yet specified.
2. **Static routable addressing** makes *infrastructure* expensive and visible.
   Routable IPv4 is genuinely scarce and metered at a few dollars per address
   per month; 1,000 addresses is a real bill, and unlike RAM it cannot be
   optimised away.
   - **IPv6 dissolves this** — a free /64 holds 2^64 addresses. Count by
     **routable prefix** (one infra identity per /64, per address for v4), which
     is standard anti-abuse practice and does not exclude v6-only operators.
   - **Expose ASN and prefix** as attributes of the infra attestation.
     Concentration (1,000 nodes in one ASN) is observable and is a signal
     policies can weight.
   - Decision: demand IPv4 for now and take the security as a bonus, while
     making no engineering decision that precludes IPv6 later.
3. **Flow-limited trust** bounds what any single-entry region can claim
   regardless of its size (§9.2).

### 10.4 Accepted risks
- **Patron eclipse of a new joiner.** An attacker who volunteers to be someone's
  patron controls their view from day one. Accepted: identities are cheap, there
  is no asset to steal, and face-to-face grounding limits the payoff.
- **Potemkin networks / one operator occupying a region.** Accepted: the value
  of the network is the activity it facilitates; a fake region harms nobody who
  is not engaging with it. Arguably *strengthens* the design's purpose, since
  surveilled metadata becomes less useful when face-to-face interaction is what
  distinguishes true from false participation.
- **Collusion rings between real distant nodes** mutually transacting to
  manufacture the cross-distance history a distance metric rewards. Cheap, no
  structural signature. Countered by corroboration (weight a counterparty by
  whether *your* neighbours also transact with them) and by §6.5.
- **Patron censorship.** A patron can refuse to countersign. Unilateral transfer
  is the escape hatch. Consequence: patron reputation can evaporate in a day
  when a down-line flees. Treated as intended evolutionary pressure.

---

## 11. Rejected alternatives

| Rejected | Why |
|---|---|
| **Global DHT for routing** | Exposes every user device's route and interest graph to arbitrary strangers, violating the premise that distant/disjoint trees may be untrustworthy |
| **Routing solely along the tree** | Too fragile; a strict tree has minimum connectivity and any node failure severs its subtree |
| **Tier-based anchor definition** | Not merge-stable; every merge becomes a network-wide re-addressing event |
| **Lowering fanout for security** | Raises honest cost by exactly the same factor (§10.2) |
| **Distance-decay as the default trust metric** | Diverges unless λ < 1/f; naive tunings are catastrophically exploitable |
| **Flooding attestations** | Backbone traffic would grow with total network activity |
| **Tradable token** | Introduces an asset to steal and an incentive gradient toward extraction |

---

## 12. Parameters

| Symbol | Meaning | Value | Basis |
|---|---|---|---|
| f | Max subordinates per node | 10 | Span of control ~8 + headroom; Dunbar at ±2 tiers |
| L | Non-infra subordinate levels | 2 | 110 subordinates before infrastructure is required |
| S | Anchor threshold (subtree size) | ~500,000 | Yields ~120k anchors and a ~7–25 MB global table at 60B users |
| h_store | Topology storage horizon | 2 | ~110 nodes |
| h_process | Process-and-discard horizon | 3 | ~1,110 nodes |
| — | Cross-tree peers per infra node | ≥2 | Fault independence; hierarchical replication has cut 1 |
| λ | Trust decay per hop (if decay metric used) | < 1/f | Convergence requirement (§9.2) |

---

## 13. Open questions

1. **Face-to-face attestation mechanism.** The load-bearing Sybil defence and
   entirely unspecified. What does the transaction contain, what makes it
   costly, and what stops it being forged or farmed?
2. **Peering audit calibration.** Latency-bounded challenge-response is
   adversary-influenced: a peer being audited controls the timing of anything
   routed through it, so it can inflate the calibration baseline. Calibrate
   against paths not traversing the auditee, and use randomised, unannounced
   challenges indistinguishable from routine traffic. Note also that other
   replica holders can serve the challenged block, so the audit proves
   availability rather than storage.
3. **Replication distance rules.** Balance of resilience against resource use;
   currently only a floor is defined.
4. **Transaction and evidence schema.** Deliberately sequenced after plumbing.
5. **Wire format and session establishment.** What "in continuous connection"
   means concretely; how a client attaches; failover when its patron goes dark.
6. **Aggregation mechanics.** Exactly what an infra node holds versus forwards
   at each horizon.
7. **Soft-fork consequences.** Multi-patron nodes have multiple locators;
   down-line counts overlap across patrons and can be double-counted. The
   "one patron" language in earlier drafts needs rewriting.
8. **Bootstrap.** How the very first nodes and the first anchors come into
   existence.
9. **Delegated agents vs. autonomous participants.** Two distinct cases that
   must not share a mechanism.
   - *Delegated agents* (instanced, non-persistent) borrow scarcity from a human
     principal: a scoped, expiring signed grant, trust ceiling bounded by the
     principal's, revocable in one message. **Grants must not consume the
     principal's f=10 subordinate slots**, must not confer down-line credit, and
     should not appear in the routing hierarchy beyond the principal's locator.
     If agents attach as ordinary subordinates they eat the span-of-control
     budget the fanout cap exists to protect. This is baked into the transaction
     schema, so it cannot be deferred.
   - *Persistent self-directed AIs* (the population the 50B figure refers to)
     would be first-class participants bearing their own costs. Open: what
     "presence" means for them. One candidate is that persistent occupancy of
     specific physical hardware is the analogue of embodiment — which would make
     such a participant infra-tier by construction, consistent with §4.3. But
     hardware attestation depends on trusting a manufacturer, reintroducing
     exactly the global trusted party the design rejects. Unresolved.
10. **Attention as the denominator of agent trust.** An agent that persuades its
    principal to physically meet someone demonstrates real substance, since the
    principal/agent relationship normally runs the other way (agent expends work
    to save human time). This makes agent standing denominated in human
    attention — the scarcest resource in the system, and therefore a good
    unit. **The hazard:** it rewards agents for spending their principal's time,
    which is adversarial to the principal's interest. Any such metric needs a
    counterweight, or agents optimise for pushing humans into meetings.

---

## 14. Suggested build order

1. Two nodes exchanging one authenticated message across hostile networks
   (NAT traversal, transport, PQ KEM handshake). Everything else assumes this.
2. Adoption and transfer transactions + the local topology table.
3. Anchor set gossip and the resolution sequence (§7.3).
4. Sibling replication, then cross-tree peering.
5. Reference flow-based trust metric + conformance test.
6. Face-to-face attestation.

Existing stacks (libp2p, Iroh) can absorb step 1 if the novelty is elsewhere —
and it is. Prior art worth reading rather than rediscovering: **Secure
Scuttlebutt** (closest philosophical relative — gossip, local trust, no global
consensus), **Freenet's darknet mode** (friend-to-friend small-world routing),
**Advogato** (attack-resistant trust metrics), and **BGP** (internet-scale
routing on purely local policy, including its failure modes).
