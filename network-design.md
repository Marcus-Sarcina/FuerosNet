# Reconfigurable-Hierarchic Trust Network — Design Document

## Preface

I have produced this document and its subsidiary files with the assistance of
generative AI, primarily Anthropic's Opus 5 model circa late Summer of 2026. While
the high level concept and most of the structural innovations it contains are mine,
I will not pretend that I would have been able to elaborate and validate a design of
this complexity by my own efforts alone. Utilizing Claude allowed me to benefit from
a far wider and deeper base of cryptography, networking, security architecture, and
software design knowledge than I could have obtained by my own efforts in a
reasonable timeframe.

This project describes one of several such platforms I have conceived in the years
since 2009 when my experience being hired at US Airways first exposed me to the
inefficiencies of corporate administration and most especially since 2012 when my
first encounter with Bitcoin launched me into a near-constant daydream about the
potential of cryptographic signatures to structure distributed institutions. The
root of this obsession may go back even farther, to my early exposure to role
playing games and later early MMOs, which inspired me to meditate on the
interactions between systems of rules or incentives and emergent social behavior.

A previous attempt at such a project can be seen in Sarcina (sarcina.co), my concept
for a distributed labor platform. Despite valiant effort by a very talented
co-founder (who may not wish to be named here), development of Sarcina stalled when
the volume of design and development effort it required out-scaled what unpaid,
part-time contributors could bring to bear. With that lesson in mind, my new hope is
that I can today realize some of these daydreams as operational systems with the aid
of generative AI.

Like many people today in 2026, I recognize that the advent of generative AI has
broken some crucial assumptions that our larger society and conventions of behavior
relied upon. You used to be able to trust that when a document or piece of media
consumed your attention, there was a human behind that content who had invested a
level of effort at least equal to your effort in consuming it. It was true that
broadcast media might "consume" millions of hours of attention for content that took
mere hours or days to produce, but since popularity was itself earned by people's
appreciation of a certain thing, it remained true on balance that the human attention
devoted to a work was tethered to its ability to deliver some positive value.

Today though, it is possible for a person lacking skill or insight or even interest
can, with a few sentences of prompt, generate a firehose of content costing an
audience more to dismiss than it cost to produce. That is why, of all the ideas for
distributed systems bouncing around in my head, each with varying rules and
dynamics, I chose to implement this one as my first AI-assisted project: This is the
system which most crucially anchors trust and attention in the ACTUAL scarce
resource: Human attention and human social relations.

~ A. Marcus Zuech,
Aug. 2026

---

**Status:** Design. No implementation yet.

**Specified.** Topology and transactions; the archive, its genesis rule and its
merge semantics; proof of presence, including selection by recognition; addressing, resolution and routing; session establishment, failover and the
transport handshake; the trust model; the resource layer — object, permission
scopes, catalog, abuse reporting, credential, and request framing; group
operations. Encoding for all of it is in
`wire-format.md`.

**Not specified.** Nothing currently blocks a subsystem (§22.1). End-to-end payload encryption is *adopt PQXDH and
the Triple Ratchet* (§14.2.4) rather than designed here, with four integration
decisions open. Multi-device beyond archive merge (§23.3). **Eleven parameters
remain unset** (§21.1), sorted by how provisional they actually are in §21.1.1, and
none of them currently hardens on first deployment. **Canonical test vectors are
deferred by decision** until the encoding stops moving.

**Everything currently open is consolidated at §22**, classified by what it
blocks.

**Reading order.** §1 for what this is and what privacy level it targets, then §2 for
the vocabulary everything after it assumes. §3 and §6 for the shape; §7–9 for proof of
presence, which is the scarcity mechanism everything else rests on; §10 for the
archive; §11 for resources, which is what the network is *for*. §17–19 record what is
known to be weak, and §20 what the design rests on that is not established here (read
that before trusting any quantitative claim). **Appendix A** carries the conventions
the rest of the document is written to — normative vocabulary, where invariants live,
and what a client does without asking.

## Document set

| Document | Authoritative on |
|---|---|
| **`network-design.md`** (this file) | **What must be true.** Protocol, topology, trust, security and privacy analysis. Wins over the others on any disagreement |
| `wire-format.md` | Encoding: bytes, schemas, canonical form |
| `light-client-requirements.md` | What a participant's client must do |
| `infra-client-requirements.md` | What an operator's software must do, including package hosting and sandboxing |
| `resource-requirements.md` | What a resource must do to conform, and the credential and request framing it receives |
| `change-log.md` | History. How the design reached its current form |
| `test-vectors/` | Draft canonical vectors: encodings, hashes, signatures, selection arithmetic. Spec-derived and unverified by an implementation — a disagreement with the specifications is a finding against one of them (§23.4, `wire-format.md` §13) |

**Requirements documents contain no protocol facts.** They cite sections here
rather than restating them, because a restated fact is one that will drift.

**Appendix B** records alternatives considered and declined; read it before
re-proposing anything. The chronological history is in **`change-log.md`**, and is
not needed to understand the design as it stands.

---

## 1. Thesis

**Anchor trust and attention in the resource that is actually scarce: human
attention and human social relations.**

Every system that allocates credibility has to meter it against something costly,
or it gets flooded. Proof-of-work meters against electricity. Platform reputation
meters against account age and engagement, both of which are cheap to manufacture.
This design meters against **time spent in another person's physical presence**,
which is the one input that does not scale with capital, automation or generated
content. A person has a fixed number of hours and can spend them in one place at a
time.

Respect for the autonomy of individuals and the self-determination of human
communities requires that they be free to form their own views and assemble their
own networks of trust. A system that decides centrally who is credible has taken
that away, whatever else it provides. So:

**Build a peer-to-peer network that requires no global trust and no globally shared
state**, so that multiple graphs of any size can coexist, partition and merge
arbitrarily over time.

The metering choice and the autonomy commitment reinforce each other. Because the
meter is a physical meeting, evidence is produced by the people who were there
rather than by an authority observing them, and there is no one standing above the
participants to appeal to even if someone wanted one.

Our security and privacy model aims to track that of physical-world affiliation:
people belong to subnets roughly as they belong to companies, clubs, churches and
families — public but not centrally registered. Someone determined enough can
profile your physical-world affiliations, and it is expensive, manual, per-target
work that no single breach short-circuits. That is the level being aimed at. Not
anonymity, which §4 rejects outright, and not the stronger guarantees a network for
evading a state would need.

We see this as a security win rather than a compromise. It lets people affiliate in
digitally legible ways, and so to get the benefits of modern computing, without a
central platform operator existing as both attack surface and potential attacker.
Moving your affiliations and services off commercial platforms makes you a
materially harder target even if the replacement is only as private as the physical
world, because the replacement has no honeypot.

We see this as a win for democracy and self-determination because centralized
platforms capture margin in most cases by displacing more local and accountable
intermediaries (small businesses, local institutions, families, friend groups,
third places, civil society) and retaining value that used to sit there. Rebuilding
intermediating institutions in a post-internet era requires a network whose
structure follows human relationships and physical proximity while supporting
services competitive with centralized alternatives.

The guiding idea: *strong interfaces can replace shared truth*. Any number of
incompatible interpretations of reality can coexist productively so long as the
data model through which they communicate is well-specified. Cooperation does
not require agreement on a universal ledger. It requires a well-specified boundary
object. (Cf. Star & Griesemer on boundary objects; Masnick,
"Protocols, Not Platforms"; the end-to-end argument.)

### 1.1 Visibility replaces enforcement

**A network can only enforce where there is shared state.** Enforcement needs a
party with authority over both the actor and the observer; that party is exactly
what a design refusing global state does not have. Giving disparate groups the freedom
to self-organise therefore means giving up the ability to bind them, and
**visibility becomes the answer to divergent policy.** You cannot make anyone
behave. You can make behaviour legible, and let every party decide independently
what to do about it.

**The diagnostic, for implementers:** *is there shared state between the party who
would enforce and the party who would be bound?*

- **Yes** → enforcement works. A patron (§2) countersigns, so a patron can refuse.
  Within a subnet there is enough shared state to make rules bite.
- **No** → stop trying. Put the distinction in the evidence schema so it is
  visible, and let policy weight it.

**This is why the evidence schema is the one thing that cannot be pluggable**
(§6). The schema is the interface; policy is the interpretation. A distinction
the schema cannot express is one no policy can act on, so "make the difference
visible" is a *schema* requirement rather than a documentation one.

The pattern has been the answer four times over, each arrived at separately
before the principle was stated:

| Cannot be enforced | Made visible instead |
|---|---|
| Peering, since a node may simply not peer | Peering transactions are public; peerless infra is observable and policy-discountable (§12.7.5) |
| Retention promises, since local storage is beyond reach | Retention is committed in the record, and §7.5.2 converts the commitment into a structural consequence for compliant clients, so an honest client's obligation is public. **Against a hostile client this is neither enforceable nor detectable.** See P13 and §13.7.1. An honest example of the limit of visibility-in-place-of-enforcement |
| Client integrity, since attestation would mean trusting a manufacturer | Carried as a record attribute for policies to weight, not a requirement (§7.8) |
| Proof of presence at adoption, since no global point can require it | An optional field, so an attested adoption is visibly a different object from an unattested one (§6.1.1) |



The network is intended to **formalize real-world relationships**, not replace
them. It assumes and depends on a significant face-to-face human component.

> **We do not produce trust. We capture and formalize it, so it can be leveraged
> as a computational input.**

That sentence settles a whole class of scope questions before they are asked. The
network is not a governance layer, a reputation bureau, or an adjudicator of
conduct. It memorialises and operationalises relationships **as they occur in the
real world**. Where a proposed feature would have the network *decide* something
about people rather than *record* something they did, it is out of scope by
construction. §6.2.2 shows how it disposes of an apparent gap.

A useful one-line framing for engineers: **a reconfigurable PKI**. X.509 already
builds hierarchical certification paths, each certificate's issuer being the
subject of the next (RFC 5280); the innovation here is that
subjects can unilaterally defect from their authority and reattach elsewhere,
and nobody is obligated to accept any given root.

A useful framing for everyone else: **each Dunbar Org is a team you have
joined** (§2), and membership in one is not exclusive of others.

> **Vignette V1 — The teams you're on.**
> You belong to a company, a bowling league, a family, and an open-source
> project. Each grants you local access and privileges. Each forms its own view
> of you, and being respected in one does not automatically make you respected in
> another; your bowling league has no opinion on your code review. But the
> memberships overlap in people, and that overlap is how reputation actually
> travels: someone from work who also bowls can vouch across the gap. Nobody
> maintains a global registry of who you are; each team knows what it knows.
>
> *Illustrative. The formal version is flow across shared vertices (§16.2).*

**Two distinct axes, easily conflated:**
- *Membership plurality*, meaning presence in several subnets at once. Ordinary,
  and not modelled by the protocol (§3.1.1). The teams analogy explains it.
- *Key forking*, meaning one archive with two live keys. Orthogonal to plurality,
  and
  closer to using the same password on two mutually non-interacting sites, then
  changing it on one. See §9.0.2.

### 1.2 Security Expectation Tiers

| Layer | Target |
|---|---|
| **Resources** (§11) | Where strong security belongs. An application knows what it is protecting and can implement whatever it needs |
| **Transactions visible within a subnet** | Moderately secure, protected principally by **disaggregation** rather than by cryptography |
| **The global network** | **No more secure than the internet itself.** Nothing here assumes the wire is private |

Security expectations follow physical-world affiliations as described in §1.

#### 1.2.1 Three properties the design relies on and does not otherwise state

Load-bearing and emergent rather than mechanisms: nothing enforces them, and
several arguments elsewhere assume them.

**Cheap fabrication is a privacy feature, not only a weakness.** Because fully
convincing fictitious subnets are possible, even a solidly documented package of
correlated evidence remains deniable — Sybil attackers inadvertently contribute to
the deniability of every record. The boundary that keeps this from contradicting
§17.3: **forging evidence about a specific real person is hard** — it needs their
signature, and the ceremony machinery exists to keep it hard — while **fabricating
a whole fictitious graph is easy.** The deniability therefore lies in the graph,
not in any signature: a package showing that someone met fifty people is
indistinguishable from one where forty-nine are fictitious, unless the evaluator
can reach those people independently. A distant evaluator cannot; a local one can —
the same asymmetry §16.1 builds trust on.

**Trust emanates from the user, so a subject's confidence in their own evidence is
better founded than an attacker's, even when both hold the same information.** A
participant knows which meetings happened; a holder of identical records knows only
what the records say, with no way to separate real from fabricated without
independent reach into the graph. This is why trust is per-observer (§16.1) rather
than a global score: the same evidence genuinely supports different conclusions for
differently-placed parties, and a global number would have to discard that
difference.

**A user's baseline exposure to surveillance is the floor of their overall
security.** The design does not aim at absolute confidentiality. It treats the
exposure a person already accepts as the level to beat, and attempts to be at least
marginally more secure along each vector of attack than a non-user's equivalent
activity:

| Attack | Conventional cost | Here |
|---|---|---|
| Read someone's photographs | Take their unlocked phone | Take their phone, defeat archive encryption, **and obtain a capture key from each person depicted** (§7.5.2) |
| Log someone's whereabouts | Follow them and photograph them in public | Gain enough trust to be admitted to their subnet, then attend a ceremony |
| Learn who someone associates with | Read their social platforms | Reach inside their horizon, or compromise a party who already holds it |

**This is what makes the privacy findings sortable.** "An attacker with the device
sees everything" reads as a defect against a hypothetical zero-disclosure system;
against the floor, the question is whether that attacker sees more than they would
from any other phone, and mostly they do not. §19's register should be read
against this floor.

#### 1.2.2 Whom the floor protects against, and where it does not

Surveillance is not one activity, and a design that treats it as one will defend
the wrong thing. Three classes are worth separating, because the network stands
differently against each and because the difference is not a matter of degree.

**Indiscriminate and untargeted.** Bulk collection for research, advertising,
sentiment analysis or market intelligence. Nobody here is looking for a particular
person; the value is in volume. This is the class the network resists most
naturally, because there is no store to collect from. The possibility of
synthesised subnets further devalues whatever is gathered, since the collector
cannot tell fabricated activity from real.

**Indiscriminate but targeted.** Intelligence services, secret police, immigration
officers, employment screeners. This class wants information about a **specific
subject** and is **accountable to no fixed evidentiary standard**. An officer will
refuse you entry because your device holds photographs of you somewhere
compromising, and will refuse you exactly as readily whether or not those
photographs are signed and witnessed. **To this class, cryptographic attestation
adds nothing**; on-device encryption is a real defence and attestation is not an
enhancement.

**Discriminate.** A police department or investigator building a case that must
survive examination under rules of evidence, with some standard of due process.
This is the only class for whom attestation adds value, and it is also the class
that suffers from the reduced epistemic value of network data, since the same
fabricability that devalues bulk collection is available to a defence.

**The design's exposure differs sharply across them:**

| Class | Who | What defends | Does attestation help them? |
|---|---|---|---|
| **Indiscriminate, untargeted** | Bulk collection for research, advertising, sentiment or market intelligence | **Disaggregation.** There is no store to walk (§19.1.1), and the fabricability discount further devalues whatever is collected | **No.** Bulk collectors do not verify signatures |
| **Indiscriminate, targeted** | Intelligence services, secret police, immigration officers, employment screeners — wanting a specific subject, accountable to no evidentiary standard | **On-device encryption**, and nothing else. The decision is not evidentiary | **No.** An officer refuses entry over unsigned photographs exactly as readily as over signed ones |
| **Discriminate** | Police, investigators, litigants — needing a conclusion that withstands examination under rules of evidence | Disaggregation, plus the **fabricability discount** (§1.2.1), which is a defence that operates *inside* their standard | **Yes, and only here** |

**So the evidentiary-weight cost is narrower than it first appears.** An archive is
durable and non-repudiable where a camera roll is not, and that is a real
difference, but one **only the third class can spend**. Against the
second, which is where most people's realistic fear lies, a signature adds nothing
an unsigned photograph did not already supply, and encryption is the whole defence
either way.

**The discount also applies to the classes least able to defeat it.** A signature
proves a key signed, not that a person exists, so a subject can say a subnet was
synthesised, and a **remote** examiner cannot distinguish that from a real one.
Classes one and two are remote from the subject's graph almost by definition. Class
three can reach counterparties and defeat the discount, and is also the only class
bound by rules that constrain what it may do with the result.

**Where the cost is real and unoffset**: an attacker in class three who obtains an
archive gets **signed assertions about who someone met**, which no camera roll
provides, and who can afford to check them. §1.2.4's concession stands for that
case and should not be read more broadly.

#### 1.2.3 Not a platform for evading the state

This is not built for crime or insurgency, so the threat model of an anonymity
network does not apply. Someone committing crimes on this network would not be
doing it at the layer this document specifies; it would be inside a resource,
which can implement whatever additional obfuscation its authors choose. **The
network layer's job is identity and trust, cheaply enough that resources can build
strong security on top of it.**

#### 1.2.4 Three honest limits on that target

**Where affiliation is itself the incriminating fact, this design offers no more
protection than the physical world — and no less.** The attacker who punishes
affiliation alone is §1.2.2's second class, indiscriminate-but-targeted, and to that
class cryptographic attestation adds nothing. What such a user's exposure turns on
is *discoverability*, and the network makes subnet membership discoverable roughly
as a church or club is — visible to members and neighbours, enumerable by no
stranger (§12.4). The regulator is therefore the same one the physical world
offers: circumspection about which subnets you visibly join. Users needing more
than circumspection need anonymity, which §4 rejects; this is not a cryptoanarchy
platform. The residual delta — signed records are durable and machine-readable once
obtained — is the third limit below, and it is spendable only by §1.2.2's third
class.

**Strong security lives at the resource layer.** Network- and resource-layer
security protects the interests of organisations that run resources, and is less
provable from the perspective of an individual who reaches the network and its
hosted services through someone else's infrastructure. The middle tier is the
weakest, and so a user's security posture is managed primarily by their own
behavior: which subnets they choose to join, and which activities they trust to
each subnet.

**Physical affiliation is discoverable but deniable; this is discoverable and
signed.** The security of a user's affiliations then reduces back to the security
of their key and their disclosure choices. The benchmark is therefore not quite
met, insofar as a disclosed record or a compromised key means the network
reproduces the discoverability of physical affiliation while removing its
deniability. Non-repudiation is accepted as a cost separately (§19.7), and
§1.2.2 narrows this: the cost lands on the classes least able to exploit it.

**Disaggregation defeats bulk collection, not targeted assembly.** Physical-world
profiling is expensive both in bulk and per target. This design is expensive in
bulk and comparatively cheap per target once an observer is inside the relevant
horizon, which is what C4 and C7 in §19.8 describe. The per-target cost is real;
the per-record processing cost is near zero. An adversary who solves acquisition
solves everything downstream — but the compromise extends only to the compromised
user's activity within the subnets the adversary can see into.

### 1.3 The adoption path is cheaper than it looks

Consuming subnet membership as a credential is **structurally identical to
enterprise single sign-on**: an identity provider asserts a user with roles or
groups, and a service consumes the assertion. **Any SaaS supporting enterprise SSO
is most of the way to being usable here**, with the infra node as an unusual IdP.

**The identity-provider package is written largely once rather than once per
vendor**, covering the standard federation surface. It does not reduce per-vendor
work to zero. Vendors differ in claim mappings, tenant configuration, provisioning
expectations and occasionally in which identity providers they will federate with
at all. The claim is that **standard federation support turns an integration
project into a configuration task**, not that one adaptor works unchanged
everywhere (`resource-requirements.md` §5).

The burden still falls mostly off the vendor, which is what makes this among the
cheapest cases to reach. *"Vendors must build for us"* and *"vendors need only the
SSO they already support"* are very different propositions, and the adoption
argument should not be made without saying which one applies.

### 1.4 Primary use case

Most traffic is expected to be **non-logged activity within a user's two-edge
"Dunbar Org"**, using the network as a **DNS and SSL replacement** for naming and
authentication grounded in social attestation rather than in certificate
authorities. Protocol-defined high-importance transactions should be far less
frequent than one per 100 seconds per user.

This framing matters for engineering: the metric to optimize is **resolution
latency for a stranger's key**, not bandwidth.

> **Vignette V2 — An ordinary week.**
> Priya's phone spends most of its week talking to about a dozen people: her
> five direct reports, her patron, a couple of siblings she coordinates with, her
> brother's household two tiers over. None of that leaves her serving infra
> node's neighbourhood; it never climbs toward a root and never touches an
> anchor. Twice this month she reached someone she had met weeks before, and both
> times the brief wait was the only moment she noticed the network existed at all.
>
> *Note what does not happen here: there is no lookup from a name or a key alone.
> Reaching someone new means having been introduced, since the locator travels
> with the
> key, out of band (§12.3). That is a deliberate limit, not an omission.*
>
> *Illustrates assumption **A2** (Appendix B.2), which is unvalidated. If Priya's week
> looks nothing like this, and most traffic is long-range, then the control /
> payload split (§12.6.3), the f=10 cap (§3.2), anchor lookup rarity (§12.2) and
> store-and-forward sufficiency (§14.1.4) all fail together.*

---

## 2. Vocabulary

| Term | Meaning |
|---|---|
| **Node** | A participant identified by a persistent keypair |
| **Patron** | A node's immediate upstream node |
| **Subordinate / down-line** | Nodes beneath a node in the hierarchy |
| **Sibling** | A node sharing the same direct patron |
| **Infra node** | A node running the server software on a statically routed device |
| **Light client** | The participant-facing client software, and by extension a node with no infrastructure of its own (§3.3). **Every user runs the client software, infra operators included** — it is where user actions happen — so the term names software, or a node's lack of a static device. It is never a tier and never a class of person |
| **Peer** | Cross-tree infrastructure partner (voluntary, see §6.3) |
| **Anchor** | An ancestor a node names in its locator so a recipient can route to it — or itself: **a root self-anchors**, with an empty path (`wire-format.md` §2.1). Not a status a node holds — relative to whoever is resolving, and subject to that party's caching policy (§12.2, §12.7.3) |
| **Trust horizon** | Every node within a **two-edge walk** over adoption and sibling edges, centred on the observer. At f = 10, **221** (§15.1). Not a subtree, not a tier band, and not a fixed set — every user's horizon is different and centred on themselves. **The operative term in mechanical text** [author, 2026-09-01]; *Dunbar Org* names the same set where the thesis and theory chapters speak of it as a team |
| **Dunbar Org** | The introductory and theory name for a trust horizon — the framing *a team you have joined* (§1.4). Same set, same walk |
| **Witness** | A node nominated by the *counterparty* to notarise a presence ceremony (§7.1) |
| **Verifier** | A prior counterparty queried to confirm a subject's identity (§7.3) |
| **Presence record** | The signed artifact of a face-to-face ceremony (§8.1) |
| **Node identity** | A keypair, and for hybrid identities the pair of components bound together by the keyhash (§5.1). Swapping a component produces a *different* node identity |
| **Lineage** | The history an archive carries across rotations. A lineage may be claimed by more than one node identity at once (§9.0.2), which is why "an identity has one current key" is false, the *lineage* may have several claimants, though each *node identity* is exactly one keypair |
| **Resource** | A service, data store or application owned by a node, addressed relative to it, with access adjudicated by the requester's position (§11) |
| **Agent** | A resource with permission to act with respect to users and other resources; borrows all standing from its owner (§11.3) |

---

## 3. Topology

### 3.1 Structure
- Persistent identities; **reconfigurable hierarchy**.
- **Within a subnet the authority relation is a tree.** Every **non-root** node
  has exactly one patron, and every node has at most **f = 10** subordinates.
  Roots have none, which is an ordinary state rather than an error (§12.7). Directedness and acyclicity are
  enforced within a subtree.
- **Acyclicity is a property of the patron relation, not of connectivity.** The
  operational graph is a tree *with elaboration around replication, caching and
  failover*, and several of those edges are not parent-child:
  - **Sibling replication** (§3.4): lateral, authorised implicitly by the
    patron's adoption transaction.
  - **Cross-tree peering** (§6.3), which deliberately creates cycles in the
    connectivity graph. That is the entire point: a subtree hanging off one
    vertex has cut 1, and peering is what raises it (§3.4, §16.3). "Acyclic" would
    be a defect here, not a guarantee.
  So: **acyclic authority, deliberately cyclic connectivity.** Any statement that
  the structure "is a tree" refers to the first.
- Root nodes are **emergent, not a special class**. Today's root may sign on to
  a larger tree tomorrow.

#### 3.1.1 Subnet plurality is a power, not a right

A user can be bound into several subnets at once, and neither the protocol nor
any patron can prevent it. **This is an accidental consequence of other design
choices** (network partitionability, permissionless infrastructure and free
exit) rather than an intended feature, and the protocol does not model it.

> **Vignette V7 — A power, not a right.**
> Juries can refuse to convict against the evidence. No statute grants this and no
> judge instructs a jury about it; it exists because juries are supreme on the
> facts and cannot be punished for their verdict. Lawyers therefore call
> nullification a *power* of jurors rather than a *right*, a consequence of two
> other rules, never a feature anyone wrote.
>
> Belonging to several subnets at once has the same shape. Nothing here grants it,
> nothing models it, and legislating it away would mean discarding the
> independence that produces it.
>
> *Illustrative. Implementers should build adoption; there is no "add second
> patron" command to build.*

**There is no DAG inside the network.** The graph is not globally continuous:
**the edge connecting a user's presence in two subnets passes outside the
network**, through local storage and the user's physical person. A DAG appears
only to an observer standing outside and looking at the human, a position no
node occupies.

**Addressing follows the postal model.**

> **Vignette V8 — Where the post office has no opinion.**
> You might have a home address, a work address, a place at the coast and a
> registered agent for your business. Each institution keeps its own address for
> you, under its own rules, with its own change-of-address form, and none of them
> arbitrates which is really yours.
> The post office has no view about which is really yours. It delivers what is
> posted to an address, to that address, and it will not redirect between them
> unless you file a change.
>
> You also do not hand someone all four when they ask where to send something. You
> give the one that fits.

**Each binding of a key to a subnet is an address**, its patron is privileged for
actions under that binding, and no node adjudicates which of a user's names and
addresses are legitimate.

**Consequences, all of them deletions:**
- There is no soft-fork transaction. Being adopted in another subnet is ordinary
  adoption (§6.1).
- Locators carry one `{anchor, path}` (§12.1); a protocol-defined transaction
  presents one name and one address. Mixing several would be a malformed
  introduction.
- No cross-subnet fanout accounting, no patron-privilege tiebreak, no down-line
  credit reconciliation.
- **Key rotation is per-subnet** (§13.6).

Applications may demand more. A security-clearance-style review might require
disclosure of every binding and past address within a look-back period. That is
**local application policy**, processed like any other application concern, never
adjudicated at the transport or network layer.

### 3.2 Why f = 10 (social, not technical)
Span of control in human organisations is often quoted near 7±2 for
interdependent work, though the management literature does not establish a
universal figure, and suitable spans range from 2–3 to 20+ with the nature of the
work. Treat ~8 as a **design heuristic, not an empirical constant**. 10 allows
an 8-person team with headroom so a new addition doesn't force a contemporaneous
exit. It also puts the two-edge neighbourhood at **Dunbar scale** — 221 at f = 10
(§15.1), against the vernacular estimate of around 200.

**The name is a heuristic anchor, not a bound**, and 221 overshooting 200 does not
weaken it. What it fixes is the **order of magnitude**: the population of a small
village, or the students in an elementary school. Bigger than a household or an
extended family, smaller than a town or a nation state. Of the anchors available for
a social graph at this scale, Dunbar's is close enough, and the alternative is a
number with no intuition attached to it at all. **The cap applies to infrastructure nodes as well**, preserving the
property that an infra node is a *person's* node rather than a capacity pool.

This is affordable only because the tree carries control and attestation, not
payload, so apex load scales with churn and introductions, not with usage.

### 3.3 The infrastructure tier
- **An infra node's subtree holds at most L = 2 levels of non-infra nodes** —
  10 + 100 = **110** users. The levels are counted from the
  infra node and **do not compose**: a node one level down may hold subordinates, and
  *its* subordinates may not, since a third level would sit outside every infra
  node's horizon — nothing could acknowledge it, serve it, or connect it to the
  network. To go deeper, a node in that chain must associate its key with at least
  one statically routed device running the server software. The reason
  for the bound is below.
- **Each server corresponds to a user.** Infra operators are people.
- Expected deployment: container or VM image, mostly in cloud datacentres, each
  instance requiring a unique static IP. **No centralised operator.**

> **Why there can be only 110 light client users under an infra node.**
> I will admit to having a particular social agenda with this project, and it involves
> scaffolding people to form meaningful working relationships they feel invested in.
> It's true that the ratio of paying to non-paying users implied by a 2 tier limit of
> non-infra subordinates will likely hurt the prospect for rapid uptake. But the social
> formation process I'm aiming for is actually helped by this in several ways:
>
> - More infra users means that memberships aren't cheap to give away. You have to
>   think about who your ten people ought to be and you have to consider whether those
>   people are likely to attract useful subs of their own. This creates a social
>   incentive to develop leadership and judgement and to reward it in others.
> - The smaller the fanout before someone has to pay real money discourages a
>   "celebrity" or "influencer" type culture. Even if you're good at earning attention
>   from the general public, you can't build a base of thousands of low-investment
>   consumers without building an organization of trusted subordinates and investing in
>   promoting those subordinates in turn. This rewards virtue and steadiness over
>   virality and genuine social relations over parasocial relations.
> - With fewer free memberships to give away, trees will tend to become more saturated
>   and opportunities to lead more distributed for those willing to invest in the
>   network. My hope is that this ultimately makes organizations who adopt this network
>   stronger as they'll not be able to grow on only a very few charismatic organizers
>   and so will be less likely to fall apart over any one departure.
> - A network that grows slowly but selectively and which filters for the steadiest and
>   most virtuous people will hopefully develop a reputation of exclusivity, making
>   people more willing to invest their best efforts and resources to obtain entry.

### 3.4 Replication
- **Siblings** replicate each other's traffic (up to f−1 = 9). Authorised
  implicitly by the patron's adoption transaction, with no separate agreement.
- Sibling replication provides **locality and latency, not fault independence**.
  Every replica set contained within a subtree has a vertex cut of 1 (the
  subtree root). Cousins share a grandparent; same problem one level up.
- Therefore **genuine redundancy requires at least 2 cross-tree peers** (§6.3):
  replicas reachable without traversing one's own ancestors. **This is a statement
  about what redundancy costs, not a participation requirement**: peering is
  voluntary (§6.3), zero peers is a supported configuration, and a node with none
  simply does not have the property. §21 records the same figure as a
  recommendation for that reason.
- **The peering record carries the ASN for both endpoints** (§6.3), so an
  observer can see whether two peers are *concentrated* — not whether they are
  independent, which ASN cannot show (§17.3). **Independence is
  adversarial, not only operational**: a party with legal compulsion over one
  provider reaches every node hosted there at once, so concentration bears on
  metadata confidentiality and local-state integrity as much as on fault
  tolerance. Two nodes in
  different subtrees but the same availability zone are not, and most infra will
  live in a handful of clouds. The *selection process* of a foreign operator
  cannot be compelled; the resulting *concentration*, where it exists, is visible.
  The reference client prefers peers differing in ASN and region, and surfaces it
  when they do not — **but a difference is not a demonstration.** ASN and region are
  routing and geography, so peers differing in both may still sit under one provider
  and one legal order (§17.3). The check finds concentration it can see; it does not
  establish independence.
- Replication depth (how far up/down metadata propagates) should be computed
  from expected traffic. Floor: every user's messages replicate on their nearest
  infra node and that node's siblings.

---

## 4. Scope

### In scope for v1
- Client software (light, end-user devices)
- Server software (always-up infrastructure nodes)
- Transaction types: **adoption**, which subsumes key rotation, recovery and
  transfer between patrons (§9.0, §6.2); **departure**, **disavowal**,
  peering, **proof of presence**, **series reissue** (§6.2.1,
  `wire-format.md` §4.6)
- **End-to-end payload encryption** to the addressed endpoint (§14.2). The
  endpoint may be another leaf, the patron, or a resource. Requirements sketched
  §14.2.4 adopts PQXDH and the Triple Ratchet; four integration decisions open
- **Resources** (§11): object, permission scopes, service catalog and abuse
  reporting specified; the interaction protocol and owner-movement rule deferred
- **Self-chained transaction archive** (§10). Each transaction carries a hash
  of the subject's previous one, making sequence position tamper-evident
- Addressing, resolution, and routing
- Reference trust metric (flow-based)

**See also §23**, which lists areas the design does not yet address at all:
distinct from §22's open questions about mechanisms that *are* specified.

### Explicitly deferred
- **Ranging mode and receiver implementation in proximity evidence.** §7.6.3's
  known UWB attacks turn on HRP-versus-alternative modes and specific receiver
  designs, so carrying them would let policy weight a `pass` by how defeatable it
  was. **Not carried in v1**: the fields would fingerprint hardware (P15's shape
  one layer down), the values need a registry nobody can populate yet, and
  `wire-format.md` §4.5.1's disclosures are not extension points — an ad-hoc addition is malformed.
  Until then a UWB pass is weighted as the weakest deployed mode. Revisiting means
  a `Channel` field with a mode registry, and deciding whether it joins the
  proximity disclosure or stays body-visible.
- **IPv6 endpoints, and prefix-based reputation with them.** v1 demands IPv4
  (§17.3): routable IPv4 is genuinely scarce and metered, which is the second leg
  of Sybil cost, and a 16-byte address in a `NetworkPoint` is malformed. IPv6
  dissolves per-address scarcity — a /64 holds 2^64 addresses — so admitting it
  means counting infrastructure by **delegated prefix** rather than address.
  **The package to adopt when revisiting**: the reputation unit is the /64, per RFC 6177's assignment floor
  and standing anti-abuse practice (Spamhaus lists IPv6 at /64; M3AAWG recommends
  it as the rate-limiting unit), with larger delegations a per-observer policy
  choice (§16.1); the encoding is BGP NLRI form (RFC 4271 §3.3) — `[length_bits,
  truncated address bytes]`, trailing bits zero, one logical prefix one encoding;
  the assertion is self-made and weighed, not verified (§1.1), checkable against
  public routing data (RouteViews, RIPE RIS).
- **Hard-fork departure and forwarding.** A departing node tells its former patron
  where it went, and the patron forwards traffic still arriving for it — a courtesy
  stub with a bounded lifetime, so contacts *outside* the departing node's horizon
  keep reaching it across a move instead of failing. **Not in v1.** Building it
  needs a delivery message that does not exist — departure (`wire-format.md` §4.2)
  carries no locator and cannot, since departure and adoption are independent
  (§6.2), so a separate post-adoption notice would be required. It is unenforceable
  either way — a former patron may simply not forward, and a departing node may
  never send it (§1.1) — and it costs a standing pointer to where a departed
  subordinate went, answerable to anyone who asks for the record's lifetime, which
  is the opposite of what departure is for.
  **Inside the horizon nothing is lost**: departure and adoption propagate as
  topology (§15), so a neighbourhood already holds the new position. The cost falls
  on distant contacts, who re-resolve from a higher ancestor (§12.3 Case 3) or are
  re-introduced — which is where §12.4 puts discovery anyway. *Revisit only with a
  delivery message and an accepted linkability window.*
- Transaction types beyond those listed above
- Multiple identities per client — **a v1 client-scope exclusion, not a protocol
  limit.** The protocol permits them already (§13.7); what v1 omits is the key
  management and interface work. A single-identity client correlates across
  subnets; a multi-identity one does not, with no wire difference between them
- **Autonomous-AI participation** (§23.2, *Autonomous participation*). Note this is
  deferred *participation*,
  not a deferred mechanism: an autonomous participant that bears its own costs and
  attends ceremonies is an ordinary node, so what is deferred is whether to admit
  one, not how (§11.3)

### Non-goals
- No tradable token. No asset to steal.
- No global ledger, no consensus, no canonical version of truth.
- No cold lookup of an identity by public key alone (§12.4; this is a
  deliberate property, not a gap).
- No anonymity. A user's real-world identity and physical presence is the token
  by which they buy trust. **This is not the cryptoanarchy platform it
  superficially resembles.** It shares the decentralisation and refuses the
  pseudonymity.
- Not a universal architecture. It deliberately favours in-person, long-running
  relationships, and serves weak-tie-only participants worse (§9.3).

---

## 5. Cryptography

Post-quantum from the start, tiered by how long authenticity must hold.

| Layer | Choice | Why |
|---|---|---|
| Transport (hop) | **QUIC + TLS 1.3, group `X25519MLKEM768`.** Peer authentication by raw public key (RFC 7250) | "Harvest now, decrypt later" is a real threat to confidentiality. The hybrid group survives either primitive failing. Identities here are keyhashes and there is no CA, so X.509 has nothing to validate against |
| Payload (end-to-end) | **PQXDH + Triple Ratchet (§14.2.4** | Adopted rather than designed here. Both have published specifications and formal verification |
| Identity keys | Hybrid classical + PQ (§5.1) | Long validity window justifies the cost |
| **Every transaction retained in the archive** | Post-quantum | §5.1 |
| Evidence embedded inside a signed body | Classical | §5.1 |
| Session-layer traffic, never retained | Classical | Nothing relies on these signatures once the session ends |

**Sizing.** ML-DSA signatures run 2,420 / 3,309 / 4,627 bytes for ML-DSA-44/65/87
against Ed25519's 64 — 38× / 52× / 72×. **A hybrid signer costs the sum, not
double the post-quantum figure**: 3,373 bytes at ML-DSA-65, since only one of its
two entries is post-quantum (§5.1). A presence record with ~10 signers is
**~35 KB at ML-DSA-65**, ranging roughly 26–48 KB across parameter sets. At a few
hundred records per user per decade that is under 10 MB lifetime, against local
stores already holding the photo archive (§7.5). A transaction envelope is
roughly 8 KB.

**Consequence for the anchor table (§12.2):** it stores *key hashes*, not full
public keys. PQ key sizes would exceed the 25 MB budget by ~18×.

### 5.1 Hybrid identity, and what must be signed with both components

An identity's keypair has **two components, classical and post-quantum**, and the
**keyhash covers both.** So neither can be swapped without producing a different
identity. Naming a category is not enough to implement: without this, two
implementations disagree about which key's hash *is* the identity.

**The rule:** any signed object whose authenticity must remain security-relevant on
the post-quantum timescale requires both components, as does any operation that
changes the identity being authenticated. Objects whose relevance expires before
that horizon may use the classical component alone.

**A signed object requires both components whenever its authenticity must survive
for as long as it can be presented as evidence.** In this design that is every
transaction, because the archive makes any of them presentable indefinitely. The reason is §10's chain: **verifying a history means verifying the
signatures on its records**, so an evaluator checking a five-year prefix relies on
five-year-old signatures. **Reliance does not expire while an archive is
presentable**, which is the identity's lifetime — signatures here are not
short-lived merely because the transaction was routine.

The property this protects: once classical signatures are forgeable, §10's
guarantee that a record's *position* cannot be altered fails, because fabricating a
record requires only forging its counterparty's signature. **A PQ-signed archive
head does not rescue it.** That proves the holder asserts a chain, not that the
counterparties signed its records.

**Evidence embedded inside a signed body stays classical**, and this is a
deliberate limit. Verifier responses and subject countersignatures sit *inside* a
body the envelope signature covers, so substituting either breaks that signature.

**An old-key proof is not in this class.** It is the prior identity
**authorising a change to itself.** The operation
§5.1's rule names explicitly — rather than evidence supporting someone else's
decision, so it carries both components like any other identity operation
(`wire-format.md` §4.1). Their authenticity is protected transitively and need hold
only until the receiving party decides; afterwards what an evaluator relies on is
*"the patron accepted this"*, attested by the patron's own hybrid signature.

**That justification fails where the decision is a permanent identity change.** A patron's hybrid signature preserves *that the patron
decided*, not *that the evidence was true*. Once the classical primitive is
forgeable, an attacker forges the one `match` a recovery structurally requires,
induces a patron to accept it, and **the forgery is sealed inside an authentic
post-quantum record that outlives the primitive that produced it.** The evidence
expired; the identity change it caused did not.

**So verifier responses inside a `Recovery` block are hybrid**, unlike every other
embedded signature. The cost falls where the argument below assumed it would not:
a recovery is rare, and ~10 responses at 3,373 B is ~34 KB rather than the ~211 KB
that figure was computed against for 32.

**Everywhere else the original rule stands**, because everywhere else the decision
an evaluator relies on can be revisited — a presence record's weight, a resource
grant, a currency attestation. **Only an identity replacement cannot be undone by a
later reader.**

Without that limit the cost is prohibitive rather than acceptable: a recovery
adoption may carry 32 verifier responses, each with two embedded signatures.
Hybridising all 64 costs 64 × (64 + 3,309) ≈ **211 KB** against ≈ **4 KB** as
classical, a fifty-fold increase on a single transaction.

**Degradation is bounded, and bounded where it matters.** Breaking the classical primitive lets an
attacker forge session-layer traffic and embedded evidence in flight, but not
presence records, key rotations, identity changes, or any archived transaction.

### 5.2 Implementation constraint: PQ crate maturity

RustCrypto provides `ml-dsa` (FIPS 204) and `ml-kem` (FIPS 203), both pure-Rust
and `no_std`, so a wasm32 target is viable **for the primitives**. **Both carry
explicit notices that they have not been independently audited**, and alternatives
(`fips203`/`fips204`) carry the same caveat.

**The browser gap is the transport, not the primitives.** Native Rust
plausibly covers the full profile — rustls/quinn expose `X25519MLKEM768`, RFC 7250
raw public keys and 0-RTT — but browser `wasm32-unknown-unknown` has no
production path for that stack: no UDP sockets for QUIC, no ML-KEM-capable pure-wasm
TLS provider, and entropy only via explicit `getrandom` `wasm_js` wiring. Native and
browser conformance are **separate implementation targets** until that closes;
compiling the arithmetic to wasm is not the constraint.

This constrains the PQ tier rather than defeating it, but it should be stated
plainly: **the design's post-quantum choices depend on unaudited implementations.**
Browser wasm additionally needs `getrandom` / Web Crypto integration for entropy,
which is target-specific work to be tested rather than assumed.

---

## 6. Transactions

Policy for evaluating evidence is pluggable. **The evidence schema is not.**
What a transaction contains, what each signature covers, and how history is
canonically serialised must be fixed, or nodes cannot evaluate each other's
evidence regardless of how compatible their policies are.

**Provenance must survive as typed categories.** A peering edge, a countersigned
transaction, and a face-to-face attestation are different kinds of evidence and
must remain distinguishable to any consumer of the schema, the protocol must
never pre-collapse them into a single scalar. This is a protocol requirement,
not a UI requirement: no interface can display provenance the data model has
already discarded, and the intended norm (trust flows from substantial
out-of-network history, not from having peered with a stranger) depends on the
distinction being queryable. What any given client *does* with that
distinction is entirely its own business, the published apps are reference
implementations and any number of alternative UIs should be expected.

### 6.1 Adoption
Patron accepts a new subordinate. Also implicitly authorises sibling peering
among that patron's children.

#### 6.1.1 Proof of presence and adoption

**A fresh adoption should carry a reference to a proof-of-presence record
between the two parties** — "fresh" meaning the patron has no prior PoP with the
node. The adoption transaction therefore has an optional field naming that
record's `txid`.

**Optional in the protocol, expected in the reference client, weighted by
policy.** Not a hard requirement, for a reason §3.1.1 already settles: there is
no global enforcement point. Subnet B cannot compel subnet A's adoptions to carry
a PoP; it can only decline to trust them. A "requirement" would be a
recommendation with extra steps. An adoption with no meeting is therefore not
*invalid* — it is *near-worthless*, which achieves the same result without a
hard rule breaking cases the design cannot anticipate (remote-only participants,
accessibility constraints, unusual bootstraps).

**Scope: fresh adoptions only.** Lateral and vertical shifts (§6.2.3) need no
PoP, the new patron is inside the old one's replication horizon and already
holds the history, which is the whole point of that case. Recovery adoptions have
their own evidence requirement (§9.1).

§13.1 requires a ceremony before the formation adoption, and §9.1's recovery
depends on prior ones.

### 6.2 Ending a patron relationship

**Formed bilaterally, ended unilaterally by either party.** Adoption (§6.1)
requires both signatures. Ending requires only one, from whichever side wants
out.

**There is no transfer transaction.** One would be node plus new patron with the
old patron not signing, and it collapses against §3.1.1: the protocol has no concept
of a node's *set* of patrons, so dropping an old patron was never a network
operation — and dropping is the only thing that would distinguish transfer from
adoption. Moving between patrons is **adopt at the destination,
depart the origin**, in either order, with no requirement to do both.

There is likewise **no soft-fork transaction**. Being adopted in another subnet
while remaining in this one is ordinary adoption in that other subnet, invisible
here (§3.1.1).

#### 6.2.1 Departure (node-initiated)
Single signature: the departing node. Required for a node to become a root **by
its own action.** Without it, a node that adopts elsewhere remains in the old
subtree's view indefinitely, since adoption says nothing about existing bindings.
(Disavowal, §6.2.2, is the patron-initiated route to the same state.)

The old patron does not sign. **This is the escape hatch that makes exit a real
right**, and no rule may condition it on the patron's cooperation.

Each node signs a **sequence number** on every position change — and, for an infra
node, on every endpoint change (`wire-format.md` §2.3, `wire-format.md` §7.6). Not consensus, a
freshness test, letting any observer order a node's own competing claims without a
clock and detect a stale record a revocation failed to reach.

**It is a pair, `{series, counter}`, and only the counter is monotone.** The series is
an arbitrary label that orders nothing; records in different series do not rank
against each other at all. **A counter can be exhausted, and that is why**: whoever
holds a node's key can sign one record at the top of the range, after which no
successor exists and the node can never publish a position or endpoint change again.
Advancing to a fresh series is what repairs it, and a **series reissue is
countersigned by the patron** (`wire-format.md` §4.6) — so the key alone cannot make
one, and a thief cannot follow the legitimate holder into a new series. Because the
series is arbitrary and nothing registers it globally, an attacker cannot pre-empt the
move either: it would have to exhaust a 2³² space and deliver every one of those
series to every party it wanted to block.

#### 6.2.2 Disavowal (patron-initiated)
Single signature: the patron. A patron whose reputation is charged for every
countersignature (§6.4) cannot be permanently liable for another party's
conduct. Without this the relationship would be voluntary on one side only.

**The disavowed node becomes a root**, with consequences in §12.7. **Disavowal is
positional**: it ends one patron-subordinate relationship and nothing else. The
node remains in the tree and may be adopted elsewhere within the same subnet —
which for several reason codes is the expected outcome rather than an exception.

**A party issuing a durable negative attestation must express its basis only
through a bounded, machine-interpretable category whose adverse character is
structurally visible, and must not attach arbitrary accusation text.** Free text
on a permanently signed record is a defamation surface with no recourse mechanism,
and a bounded vocabulary keeps the judgement evaluable by policy. Present encoding:
the reason is a code in a 64-value banded space (`wire-format.md` §4.3).

**Disavowal is the network's negative attestation, and the only one.** It is
public, carries a reason code half of whose space is explicitly *with prejudice*
(`wire-format.md` §4.3), and is **costly to the issuer.** A patron who disavows
loses a subordinate and stakes their own standing on the judgement, which makes
it structurally resistant to abuse: each disavowal costs the issuer a subordinate,
so the cost scales with the volume.

**No peer-to-peer equivalent exists, and none is planned.** Per §1, the network
does not produce trust; it captures and formalises it. When Bob defrauds Alice,
Alice stops dealing with him and tells the people she knows, on this network she
does exactly that: cessation, plus **payload** to her contacts. The warning travels
as a message between people, which is what it is. Making it protocol state would
mean the network adjudicating conduct, which is out of scope by construction.

**Loss of standing is already specified.** §16.5's inactivity decay means an
identity nobody transacts with loses derived standing, so cessation is a real loss
channel rather than merely an absence. It is slow and passive where fraud is fast —
but bounded rather than unbounded, since each victim was convinced by standing the
fraudster genuinely accumulated, and burning it is a one-time move.

**The reason code is information for the next patron**, not a broadcast to the
network. That is what determines whether re-adoption elsewhere is unremarkable.

**There is no notice period.** A disavowal takes effect when it is signed. A
notice period could not be enforced: a disavowal is the issuer's own signed
statement and nothing stops them signing it whenever they choose, so an effective-at
date would state the issuer's intentions rather than anything a recipient can check
(§1.1).

#### 6.2.3 Lateral and vertical shifts
Moving to a grandpatron or a patron's sibling is an **ordinary adoption whose
counterparty happens to be nearby.** No separate type. But when the new patron
lies inside the old patron's replication horizon it already holds the node's
history through sibling replication (§3.4), so **no archive presentation is needed and trust history is preserved**. A new patron
may still run a fresh ceremony if they want one — that is their choice, not a
requirement the move imposes — and the reference client does not prompt for it.

Derivable, not declared: an observer holding the relevant topology computes the
distance itself, and a self-asserted flag would only be something to lie about.

> **Vignette V3 — Onboarding a new hire.**
> A new engineer joins. One person on the team meets her in person, does the
> ceremony, and adopts her. That is the only face-to-face step anyone has to
> arrange. Over the next week the team shuffles her into the right place in the
> reporting structure: up a tier to sit under the team lead, or across to the
> group she'll actually work with. Because every one of those moves stays inside
> the replication horizon, her history comes with her and nobody has to meet her
> again to re-establish it. The network has no opinion about the org chart; the
> company does.
>
> *Illustrative. The trust-preserving property is derivable from topology, not
> declared (see below). The vignette assumes a fresh adoption carries a proof of
> presence, which §6.1.1 states as the expected default rather than leaving
> implicit.*

Expected uses are application-level — teams rebalancing, or onboarding by having
one member perform an adoption with proof of presence and then redistributing.
**The network is deliberately opinion-free about structure; applications built on
it will not be.** Reporting paths and org shape will matter to them even though
they matter to no protocol rule.

#### 6.2.4 Stranding
A departed or disavowed node cannot unilaterally reattach — adoption needs a
patron's signature, so someone must accept it. A node nobody will take is
stranded. Recorded rather than solved; same shape as §9.3 and the same answer.

#### 6.2.5 Cycle prevention
§3.1 requires a tree within each subnet. At bootstrap, mutual adoption is a
**likely accident** rather than an attack — two
friends setting up at the same time will each reasonably try to add the other,
making each simultaneously the other's patron and subordinate. Detection needs only local
state: check whether the proposed patron sits in one's own down-line, which is
inside the horizon (§15.1) for any realistic case and therefore already stored. The reference client must handle this as a normal
disambiguation prompt ("one of you must be the patron"), not as an error.

**Rejection is on positive knowledge only**: reject where local topology
establishes that the proposed patron is at or below the node, and never for absence
of knowledge. Beyond the horizon a node cannot know, and treating uncertainty as a
cycle would refuse legitimate adoptions.

**The partial-information case is answered by the rootward memo** (§15.2), and the
two mechanisms layer cheapest first. The check above catches the bootstrap accident
inside `h_store` at no traffic cost, and the disambiguation prompt resolves it
socially while both parties are present. The memo catches what neither reaches: a
cycle formed at a distance, where neither party holds the other in view.

**The memo makes the check local rather than global.** A general procedure looked
impossible because detecting a cycle appeared to need topology beyond one's horizon,
which is §1.1's *no* branch. It does not: a memo names the patron it speaks for, so a
node that **receives a memo naming itself, from below** is its own ancestor. The node
reads what it was handed rather than querying anyone.

**The failure-mode ordering is preserved, and it is why a memo is a hint rather than
evidence.** A memo is unsigned and derived, so acting on one directly would let a
fabricated memo sever a legitimate adoption — the false positive this section ranks
as the worse failure. **Detection is therefore confirmed against the detector's own
records**: the check fires only on the party the memo names, who signed the
transaction behind it and holds the slot it describes, so a fabricated memo fails
without asking anyone anything (`wire-format.md` §10.2).

**Residual, stated rather than hidden:** cycles are detected within a subnet and
nothing detects one spanning two, because §3.1.1 says nothing may. That is a
consequence of subnet plurality, not a gap in this mechanism.

### 6.3 Peering (cross-tree)
- **Voluntary and ad-hoc.** Unlike adoption, not required to participate.
- **The record carries each endpoint's network point.** Address, and where
  available the ASN, so **concentration** is observable rather than asserted
  (§3.4, §17.3). Independence is not: ASN is routing, not legal control.
- Two signatures, between infra nodes in different subtrees.
- Attests investment in the network and therefore contributes to trust, but at
  **lower flow capacity than hierarchical edges by default** (see §16.3).
- Carries a real cost: persisting the peer's data.
- **It confers no scope, and does not extend the horizon.** A peering edge is
  **ungoverned**: permissionless, outside the tree, requiring nobody's authority and
  therefore carrying none of the subnet's. A peer is not in your Dunbar Org by virtue
  of peering, no scope reaches them (§11.2), and the region in §15.1 is built from
  adoption and sibling edges only. **Contributing to trust and conferring scope are
  different things** — this is the edge where they separate, and it is the reason the
  two are described in different sections.

### 6.4 Countersigning
First-level patrons countersign their subordinates' **subnet-scoped**
transactions, except departures.

**A party with authority over a participant must not authenticate or gate evidence
of an event it did not observe and that occurred outside its authority**, where
doing so would let it suppress evidence the participant may later present under a
different authority.

Present encoding: **patrons do not countersign proof-of-presence records.**
Three reasons, any one sufficient:

- **The patron was not there.** A signature adds no evidence about a physical
  event the signer did not observe. §6.4's rationale — staking patron reputation
  and rate-limiting — is coherent for subnet-internal transactions and empty
  here. PoP is already rate-limited by human time (§7.1), a constraint a patron cannot impose
  than a patron could.
- **PoP happens outside the subnet trust envelope.** It memorialises a fact about
  the world, not an action within the patron's authority.
- **PoP history must survive a node's tenure under any patron** (§16.7). A patron
  able to gate PoP creation would shape what *later* patrons see, which is an
  attack surface on portability itself. This is why patrons do not countersign
  presence records (§6.4) rather than why they cannot block them: **no party can
  block another's client** (§9.2).

The case that decides it: Alice is patron to Bob, who is friends with Charlie.
Alice dislikes Charlie. **Bob being unable to memorialise a friendship he
actually has is not intended behaviour under any reading of this design.**

**This makes the eclipse attack permeable for the case that matters.** §18.4's
accepted risk is eclipse of a **new joiner**, whose view is controlled from day
one. A new joiner has no prior counterparties, so `min(floor(n/2), 10, |candidates|)`
is zero (§8.1)
and a **witnessless, verifierless ceremony is valid.** The formation path of
§13.2. They can therefore memorialise a meeting with anyone they physically
encounter, and the eclipsing patron cannot suppress it. **The false social
universe is permeable wherever the victim meets a human being.**

**This does not extend to an established user — from the third encounter.**
`min(floor(n/2), 10, |candidates|)` is zero at *n* = 0 **and at *n* = 1**, because
`floor(1/2)` is zero: a subject with exactly one prior meeting is verified by nobody,
and the threshold first bites at *n* = 2. **The second encounter of any identity
therefore has no continuity check**, and rests on liveness and proximity alone —
which establish that *someone* is present, not that they are who the existing
identity represents. Accepted: the case requires an attacker holding both the key and
the archive, and buying one clean false-continuity edge before the identity has a
history is worth less than the ceremony costs. From *n* = 2 the threshold is non-zero,
verifier responses are required, and reaching those verifiers may run through the very
patron doing the eclipsing. An
established user who becomes eclipsed has a harder escape, though they also have
existing standing and relationships that a new joiner does not, which is why
§18.4 scopes the accepted risk to new joiners in the first place. This makes a spammer's volume chargeable against their **patron's**
reputation, so patrons become natural rate limiters — self-enforcing rather than
protocol-enforced.

## 7. The presence ceremony

The face-to-face ceremony, the record it produces, how that record moves, and what
happens when a key is lost. This is the design's scarcity mechanism (§1) and its
largest subsystem.


Deliberately costly transactions attesting that two users met in person.

**A presence record upgrades any relationship it attaches to**, not only peering:

- **Peering** (§6.3), turns a pragmatic infrastructure arrangement into a
  trust-bearing one.
- **Adoption** (§6.1.1), distinguishes a patron who has met their subordinate
  from one who has not. Since PoP is not enforced at adoption, this distinction
  is what makes the unenforced version safe: an unattested adoption carries
  little weight, and observers can see which kind they are looking at.

**The ceremony's first product is not the record.** Two people met. Each can now
recognise the other, and each has grown their own graph by a party they trust on
their own account — §1.2.1's second property already says a participant knows which
meetings happened where a holder of identical records does not. **The record is the
residue a ceremony leaves for people who were not there**, and everything spent on
witnesses and verifiers is spent on that residue rather than
on the meeting.

**So a thin ceremony is weaker evidence, not a weaker meeting.** Fewer witnesses, or
a candidate pool in which the counterparty recognises nobody, cost the record its
weight with third parties and cost the counterparty its assurance about
**continuity** — whether the person present is the one this key's history belongs to.
Neither touches what both parties actually acquired: a face they will know again. A
subject may therefore disclose narrowly and accept a record that persuades fewer
people, and a counterparty may accept a thin one because the meeting is worth having
on its own terms.

**What this primitive is.** Bilateral collusion is unpreventable, two parties
who both want to fake a meeting hold both key sets and can simulate every step,
and no witness can distinguish that. So this is **not an unforgeable primitive**
and must not be designed as one. It is a **cost imposed on acquiring edges into
territory the attacker does not already control**, and it is complementary to
the flow metric (§16.2): presence proofs do not bound the size of a fake region,
the min-cut does; presence proofs make each honest-to-attacker edge expensive.
Neither works alone.

Trust does not emanate from these transactions. It is deduced outward by each
end node, starting from its own local records of people it has met.

### 7.1 Ceremony (summary)
1. Participants announce intent, and each participant **nominates witnesses from
   the other party's neighbourhood** — never from its own. A party choosing its
   own witnesses is the classic failure mode. What cross-nomination buys is
   **representativeness of the sample, not honesty**: in a balanced set, half the
   witnesses are each party's own nominees and are therefore uncurated by the
   counterparty. It needs no global randomness, and it is not a Sybil defence —
   no topology rule can be (§17.2). Procedure and residual: §7.1.1.
2. Route-latency plausibility check (§7.6), weak evidence, modest weight.
3. Optical channel: QR codes exchanged screen-to-camera. Carries key exchange
   and the transcript hash. Inherently short-range and line-of-sight.
4. **Proximity channel: UWB secure ranging (802.15.4z) where available, NFC tap
   as fallback.** UWB is the **strongest** channel but **not categorically
   relay-resistant.** Deployed 802.15.4z HRP implementations have been defeated by
   physical-layer distance-reduction attacks (§7.6.3). NFC provides
   physical-range friction and no distance-bounding guarantee. Bluetooth RSSI is
   NOT usable: received signal strength is attacker-controllable, since an
   amplifier raises it without changing distance. For the participants' own
   assurance; not remotely verifiable.
5. **Guided face capture**, locally on each device (§7.2, §7.5). Each party
   captures 3–5 images of the counterparty over 10–15 seconds under randomised
   prompts — turn slightly, change expression, which supplies both the angle and
   lighting *diversity* that defeats correlated within-session failure, and the
   motion and parallax that constitute the **liveness check** against a printed
   photo, a screen replay or generated video. **Each party also gives the other a
   32-byte seed, and seals its captures under keys derived from the seed the
   *subject* supplied** (§7.5.2), so a compliant client holds no decryptable
   likeness of anyone but itself. Images stay on the device **for the declared
   retention period** (§7.5.1) and are then deleted; nothing biometric enters the
   record at any point.
6. **Verifier queries**, run automatically by the client (§7.3). Each party's
   client sends a fuzzed profile of the person in front of it to a
   deterministically selected sample of that identity's prior counterparties.
   **The subject sends each selected verifier the capture key** for that verifier's
   stored images, directly and in parallel, so the verifier can decrypt what it
   holds (§7.5.2). The queries ask whether it matches what they hold.
   Responses — match, no-match, inconclusive, unavailable — are signed by the
   verifier and carried in the record; a verifier that does not answer within
   the ceremony simply does not appear, and the record's response count against
   the `min(floor(n/2), 10, |candidates|)` criterion is part of what any
   evaluator reads (§8.1). **Selection is by recognition** (§8.1.2): each party
   picks the other's verifiers from the handed bundle, preferring people they
   have met or share a trust horizon with, going fishing for common
   acquaintances where the bundles surface none, and filling the remainder at
   their own discretion — each response carrying the selector's claim of which
   it was.
7. **Each party reviews the other's selection before signing.** Because A selects
   B's verifiers and B selects A's, a participant who signs without looking may be
   left holding a record whose responders they cannot stand behind. And cannot
   repair it, since the record is immutable. This is the one ceremony check that
   protects a signer **against their counterparty**, rather than against outsiders
   or against the pair colluding.
8. Witnesses attest only to what they observed: that the protocol ran, both
   parties were live and responsive, and the sequence was well-formed. They
   **cannot** attest that two humans were in a room, and the record must not
   claim they did.

**A witness's operator is not involved and need not know it happened.** The witness *client* observes the procedure, tests the evidence at each
point, checks what it receives against the enforced timing, and signs — without
asking the person who owns it, who is very likely unaware of the ceremony or of who
was in it. **Witnessing resembles a human act and is not one** (Appendix A): what a witness
attests is what its client observed, and a client that stopped to ask would be
asking its operator about something the operator did not see.

Ceremony duration is deliberately minutes, not seconds. It meters human time,
which is the scarce resource the attack must consume; a per-identity cooldown
would not, since an attacker holds many identities.

> **Vignette V4 — Why it takes a few minutes.**
> At a conference you could collect two hundred LinkedIn connections in an
> afternoon without looking up from your badge. Here you'll manage perhaps a
> dozen: each one is a few minutes standing across from someone, phones facing
> each other, turning your head when prompted. That is annoying, and it is
> supposed to be. The friction is the product, a connection here says *we met*,
> and that is a claim nobody makes two hundred times in an afternoon. A warehouse
> of handsets buys nothing, because what the ceremony costs is somebody else's
> minutes.
>
> *Two limits on that, both real. The record does not prove two people shared a
> room — witnesses attest that the protocol ran and both parties were live, not
> what they saw (§7.1). And two people who both want to fake a meeting can:
> collusion between willing parties is unpreventable (§7). The cost lands on
> edges to people who did not agree, which is where it needs to.*
>
> *Illustrates assumptions **A1** and **A8** (Appendix B.2), both unvalidated. If human
> attention can be bought cheaply enough at scale, the friction meters a resource
> the attacker has, and the strongest leg of the Sybil defence (§17.3) weakens
> considerably.*

> **Vignette V5 — A new ritual.**
> In the late 2010s people all over the world started doing something they had
> never done before. Partway through talking to a customer service rep or filling
> in a form, they would be told to open their phone, check for a code, and read it
> back. Almost nobody outside the industry knew or cared why every institution
> adopted this at roughly the same moment. But the ritual arrived alongside the
> automation of things that used to involve a person, and alongside working from
> home, which had been rare. A few years later "Sign in with Google" joined the
> repertoire.
>
> If this protocol is broadly adopted, most users will never know most of what is
> in this document. What they will know is another new ritual: meeting someone
> face to face, talking for a few minutes until the screen goes green, then
> holding the phone out, not in front of your face, while it tells you to turn
> your head this way, then that way.
>
> It is a little strange, particularly the first few times. But where the other
> rituals the industry taught us arrived with more automation and more isolation,
> this one runs the other way. It is the point at which human faces, human
> voices, and knowing someone personally start to carry weight in your dealings
> with the wider world.
>
> *Illustrates assumption **A11** (Appendix B.2): that ordinary users will tolerate the
> friction rather than route around it. Unvalidated, and the whole Sybil defence
> depends on the mechanism actually being used. The claim that culture would
> coalesce around the ceremony is speculation, included because the ceremony is
> the most novel thing a user will encounter and is the natural anchor for
> interface and identity work.*

#### 7.1.1 Nominating witnesses

**What the nomination approximates.** The witness sample would ideally be drawn at
random from the whole userbase. No node can enumerate the userbase, so each party
draws from the nodes it can see in its counterparty's neighbourhood, and the two
draws together stand in for that sample.

**Two things the reference client optimises when nominating.** A **random element**,
so the counterparty cannot predict which of its neighbours will be asked; and
**spread across as many independent branches of the counterparty's graph as
possible**, because catching every one of a party's nominations then requires a
correspondingly larger fake neighbourhood rather than one well-placed node. Branch
spread — not the flow metric — is what raises the cost of the Potemkin case in
§18.4.

**Availability is discovered by the nominator, not advertised by the candidate.**
A large fraction of the nodes a party would like to nominate are inactive, or
light-client-only and not running at the time of the ceremony, so the client probes
its selections and keeps those able to serve. The filter comes *after* selection and
is a property the nominator finds; a node making itself conspicuously available to
be nominated would be self-selecting, which is the thing cross-nomination exists to
prevent.

**Balance is what the property depends on, and nothing enforces it.**
`wire-format.md` §3.2 requires at least one witness and requires each
`nominated_by` to name one of the participants; it does not require both
participants to be represented. Where one party nominated every witness — a
one-witness record being the smallest case — the other party has none of its own
nominees present and the "half the set is uncurated" property is simply absent, not
merely weakened. This is left **visible rather than mandatory**, for §6.1.1's
reason: a balanced set is what a party should insist on, not what the encoding can
require, and a meeting where only one other node could serve is a real case a hard
rule would invalidate. `nominated_by` is in the record, so the split reads directly.

**The split is a signing check before it is an evaluator's.** It bears on the
attestation's representativeness: a party whose counterparty nominated every
witness holds a record attested entirely by that counterparty's nominees.
§8.1.2's review-before-signing is the same posture for the same reason — it
protects the signer against their counterparty, not against an outsider.

### 7.2 Local face storage, no biometrics in network state

**Biometric templates never enter network state.** Reasons, ascending: they are
irrevocable (keys rotate, faces do not); fuzzing does not survive combination
with timestamp, location and graph position in the same record; and a global
replicated log of who physically met whom with biometrics attached inverts the
network's own metadata-resistance property.

**Holding identifiable biometric data also creates legal exposure**, and it varies
by jurisdiction in who may sue, how repeated collection is counted, and what
destruction obligations attach. **This document does not attempt to survey that**:
the rules change, they differ by where an operator sits and where their
counterparties sit, and a summary here would be stale before it was useful. **An
operator holding such data should get advice about their own jurisdiction and
their own role.**

*(This is the first statement of what §19.1 generalises into a design-wide
invariant: privacy properties must be assessed under composition of everything an
observer can obtain, never artifact by artifact.)*

##### Keystream encryption changes this position

**A compliant client holds no decryptable biometric data at rest** (§7.5.2).
Captures are sealed under keys derived from a seed the *depicted person* supplied, and cannot
be opened without that person handing it over again in a later ceremony.

**Ciphertext a holder cannot open is a different thing from a stored likeness**,
and the difference is likely to matter wherever the obligation attaches to holding
material in usable form. **Whether it matters in any particular jurisdiction is not
this document's judgement to make**, and the position is not settled anywhere.

**Two things it does not do.** A non-compliant client retains plaintext and its
operator is exactly where they were. And the relief, whatever it amounts to, does
not reach decryptable images, unencrypted derived templates, or anyone who obtains
a capture key and keeps it.

Instead:

- Each participant retains, **locally, and sealed under keys derived from the seed
  that counterparty supplied** (§7.5.2), a full photo record of the counterparty. The
  holder cannot open it unaided.
- Retaining the *photo* rather than only a derived template is deliberate: face
  recognition models improve, and templates are only comparable within a
  version. Holding the source image allows re-derivation under future template
  versions.
- The published record carries only a signed assertion of the form *"I observed
  a live human face; it matched / did not match / no prior record for this
  identity."*

### 7.3 Verification by query

Threat addressed: several physical humans sharing one keypair, attending events
in many cities to build an unnaturally rich meeting history, then exploiting the
resulting standing.

Protocol: C, having just met someone claiming to be B, sends a **fuzzed
biometric profile** point-to-point to A, a prior counterparty of B. A replies
`match` / `no-match` / `inconclusive` / `unavailable`. Full photos never transit
the network; fuzzed profiles travel point-to-point only, never broadcast.

**Invariant:** *whenever a participant asks others to rely on a newly asserted
face-to-face encounter as evidence of identity continuity, its client must
automatically ask a sample of that claimed identity's prior counterparties
whether the person now present matches the person they previously encountered.*

**Answering is automatic too, and the notification runs the other way.** A's client compares the presented profile against what it already holds
and replies — the machine analogue of recognising a face, and no more a human act
than the witness's signature above (Appendix A). **A's operator is not asked and is not
told**: they know B exists, having met them, and are not informed that a ceremony
involving B took place or that they were sampled for it. The notification obligation
below runs to **the subject's client**, which is the only party able to see probing
spread across verifiers; the verifier's operator has nothing to do with the answer,
and learning of each query would tell them only who is meeting whom.

**A verifier receives no part of the record.** The query carries a fuzzed profile
and a query id; the record does not exist yet, and none of §8.1.1's disclosable fields
reaches the verifier at any point.

**This is detection, not prevention, and only works if querying is routine** —
which is why it is a client obligation rather than a user action. Present
encoding: query by default on every presence transaction, against several prior
counterparties. If querying is exceptional,
the shared-identity attack succeeds most of the time.

**Sizing the query count.** Write **q** for the number of queries made — *not*
to be confused with **n**, the subject's prior-meeting count used in §8.1's
reasonableness criterion. Against k humans sharing one identity, detection
turns on querying counterparties who met a *different* confederate than the one
standing here — and with selection by recognition (§8.1.2), which
counterparties get queried is the **selector's** choice from its own
acquaintance, not the subject's to steer and not a random draw whose odds can
be quoted. The confederates can curate the bundle toward records involving
today's human, but they cannot curate the selector's acquaintances into it:
a bundle emptied of everyone the selector knows is exactly the pool of
strangers §16.1 prices at nothing.

**The arithmetic assumes the sampled counterparties are honest and independent of
the subject, and nothing structural makes them so.** Candidates are the subject's
*own* prior counterparties, drawn from the subject's own archive, and eligibility is
structural rather than weighed (§16.1). Against a subject whose candidate population
is itself fabricated, detection is not reduced but **absent**, and no q repairs it —
every query lands on the attacker. This is the same threat this section opens with,
since the confederates sharing the key are exactly who would answer.

**The check that survives is the one the querier makes while selecting.** C
enumerates B's candidates in order to sample them, so C sees the population before
it sees any answer. Recognising none of it is the signal; a `match` returned by
strangers establishes nothing for C, whatever q was. This protects C rather than any
later evaluator, which is the right party — C is the one being asked to accept the
person in front of them as continuous with a history.

### 7.4 Required hardening

#### 7.4.1 Oracle leakage

- **Oracle leakage.** Any match/no-match oracle leaks the template under
  repeated probing (hill-climbing / template reconstruction). Published attacks
  against binary-output matchers need thousands to tens of thousands of queries,
  so the defence is to make queries expensive rather than merely categorical.
  Required, in ascending order of strength:
  - **Categorical responses, never scores.** A score converges far faster.
  - **Rate limits per querier AND per subject.** No querier legitimately queries
    more often than the enforced ceremony duration, and no subject is legitimately
    queried about more often than that either. Both limits are needed: a
    per-querier limit alone is parallelisable through sock-puppet queriers, and a
    per-subject limit alone is parallelisable across many subjects.
  - **Per-subject limits must be enforced BY THE SUBJECT.** A verifier can only
    rate-limit the queries *it* receives. A subject with 50 prior counterparties
    can be probed 50-fold in parallel, one query each, with no verifier seeing an
    anomaly, and coordinating limits across verifiers would require global state,
    which this design refuses. The subject is the only party with both a complete
    view and the incentive to act on it — **and the only party holding what a
    verifier needs.** A verifier's captures of the subject are
    sealed under keys only the subject can derive (§7.5.2), so **it can evaluate
    nothing until the subject's client sends a `KeyGrant` bound to that query**
    (`wire-format.md` §7.3). The limit is therefore **structural rather than
    vigilant**: not a standing permission the subject must notice they should
    withdraw, but a key their client declines to send. Fifty parallel probes need
    fifty grants. **That is not global state; it is one node holding what everyone
    else needs.**

    **The aggregate is a lock, not a log.** What the subject holds
    is a counter per requester and per ceremony window, kept **only for the
    enforced ceremony duration** — a few minutes. It exists to refuse the next
    query, not to record that a previous one happened.

    **Retaining a queryable history would be a separate feature, and a costly
    one.** A durable record of who probed whom is an auxiliary timeline of ceremony
    attempts including ones abandoned before any record existed — events the
    network never learned about, sitting on a device that can be seized. The
    defence needs none of it: rate limiting works from a counter that expires with
    the window it enforces.

    As with every client-side rule here, a non-conforming client may retain more
    and nothing detects it (Appendix A).
  - **Bind every query to a witnessed encounter.** *A verifier must answer an
    identity-comparison request only when the requester proves it is currently
    engaged in a witnessed encounter with someone claiming to be that subject,
    and the proof must bind the request to that encounter.* A legitimate query
    exists for no other reason. **The proof is the subject's own consent**: every query carries the subject's signature over its `query_id`
    (`wire-format.md` §5.6), minted by the subject's client during the ceremony
    (§7.3's automatic querying), so each probe requires the subject's live
    cooperation **from whoever holds that key**. The ceremony pre-commitment travels
    with it and pins the fuzzed profile — one profile per ceremony, or the verifier
    rejects. Witness countersignatures are deliberately not
    carried: a distant verifier could not tell real witnesses from an attacker's
    keys, while the subject's signature is checkable from the identity the query
    itself names. This prices probing
    in *ceremonies* rather than in packets: at ~10 queries per ceremony and
    minutes per ceremony, a reconstruction attack costs weeks of continuously
    staged meetings under witness observation.

    **The binding is to the subject's key, not to a witnessed encounter**, and the two
    are the same thing only for an attacker who lacks that key. **A thief holding the
    subject's device holds both halves of the price**: it mints fresh commitments,
    signs every query, releases every seed, and receives the notifications below —
    every subject-side limit here sits on the stolen device. What survives is
    verifier-side: the per-requester and per-subject counters each verifier keeps
    locally, which no attacker can reach and which bound probing per counterparty
    rather than per ceremony. **Witness countersignatures would not repair it** for the
    reason already given — a distant verifier cannot tell real witnesses from an
    attacker's keys — so the price against a thief is set by the verifiers' own limits
    and by how many prior counterparties the victim has (§18.3's stolen-device entry).
  - **Surface each query to the subject's client as it arrives**, independent of the
    limits above, so probing is visible even when it stays under them.
    **Notification, not a log**: the client sees the query, and what persists
    afterwards is the ceremony-window counter and nothing else (the aggregate above). A retained
    history would rebuild the timeline the counter exists to avoid.

    **This is client-to-client and the subject is not interrupted**. A ceremony runs several verifiers per participant, with witnesses
    exchanging their own messages at the same time, while both humans stand facing
    each other with their screens turned away (Appendix A). **What reaches a person is the
    conclusion, not the traffic**: probing detected, grants refused. A design
    that popped up each step would be asking someone mid-ceremony to adjudicate
    dozens of exchanges they cannot evaluate and did not initiate.

  Selection by recognition (§8.1.2) means a hostile counterparty **does** choose
  which verifiers are queried, and could try to concentrate probes on one
  verifier's photographs across many ceremonies. Three controls remain, and
  they are the real ones: **the subject's per-query grant** — each capture key
  is released per selected verifier (§7.5.2), so the subject sees exactly who
  is being probed and refuses; the per-subject and per-requester counters
  above; and **bundle curation** — a subject can leave a verifier out of the
  bundle entirely, and nobody can be queried through a record the subject
  declines to show.

#### 7.4.2 Consent

- **Consent: the subject countersigns every query about themselves.** *No
  verifier may answer an identity-comparison request unless the subject of that
  request has signed it.* Checkable from the query itself, so an unauthorised
  disclosure cannot be laundered into the trust graph.

  **The subject is already present, which is what makes this workable.** The subject is
  **physically present at the ceremony** that the query is bound to (§7.4.1), so
  countersigning happens in real time at no cost. Almost no other consent
  mechanism has that property.

  **It also closes a hill-climbing vector that rate limiting does not.** A querier
  can send **different fuzzed profiles to different verifiers.** One probe each,
  no verifier seeing a pattern, and the subject seeing only a count. Because the
  subject signs each request, they see the profile in each and **can require that
  all queries from one ceremony carry the identical profile**. That turns
  detection-by-counting into prevention: one probe point per ceremony rather than
  *n*, and identical probes yield no gradient.

  **An impostor gains nothing, where the counterparty is honest.** The profile is
  generated by that counterparty from what they captured, so signing it means signing
  a profile of *oneself* — which is precisely what produces `no_match`. Refusing to
  sign leaves the record with fewer responses than its own claim suggests (§8.1),
  so refusal is legible.

  **The premise is the honest counterparty, and it excludes the malicious one.**
  A client performing the capture can send a **substituted** profile — synthetic, or
  taken from someone else — and the subject cannot tell: countersigning binds the
  bytes and the template version, not their provenance, and the subject's own device
  captured the counterparty rather than itself. What that buys is **one probe point
  per ceremony** against the subject's prior counterparties' stored captures, which
  the anti-oracle rule above already caps, and it produces no record: finalization
  needs the subject's envelope signature, and a subject seeing `no_match` returned
  about themselves will withhold it. Registered rather than closed — binding a profile
  to the live subject needs an attestation the capture device does not have (§7.8).

  **This supersedes the standing disclosure policy.** All queries are
  ceremony-bound and the subject is present at every ceremony about themselves, so
  per-query consent covers everything the policy field expressed, more precisely
  and without a stale-policy problem. The **retention commitment remains.** It
  governs storage rather than disclosure and has no per-query equivalent.

  **Off-protocol disclosure still cannot be prevented.** A holder who simply tells
  someone what they know is beyond any protocol rule. What the design ensures is
  that disclosure *presented as network evidence* carries its authorisation. B never
  consented to A becoming a biometric verification service merely by meeting, and
  the per-query countersignature above is what makes that consent explicit rather
  than presumed.

#### 7.4.3 Availability, and what silence is worth

- **"Unavailable" must not be free.** *A participant who undertakes to retain
  identity-verification material about another must publicly commit, at the time
  of collection, to how long that material will remain available for
  verification.* Otherwise attackers exploit a non-prejudicial "no record"
  response. Present encoding: the record **commits to a retention period at
  meeting time**, so a later "unavailable" is checkable against a declared
  policy rather than being an unfalsifiable excuse. A high rate of unavailable
  responses across an identity's history is itself a signal.
- **Light clients vs infrastructure nodes are judged differently on
  availability.** Light clients have legitimately unstable uptime, so silence
  from them carries little weight; infrastructure nodes have none of that
  excuse — **the operator's instance holds their key too** (§23.3), so the party
  being queried has a device that is up whether or not they are. A consequence is that a presence record with an infrastructure
  participant is **more reliable as evidence.** That party can be expected to
  answer verification queries years later. Note this is a statement about the
  *record's* evidentiary durability, **not** about the participant's
  trustworthiness. Reliability and social trust are **separately typed channels in
  the evidence schema** (§16.6), that separation is enforceable. Whether a given
  policy keeps them separate is not; the reference policy does.
  - **But silence should not be free for light clients either.** Queries to an
    offline light client **queue at its patron**, resolving when the client next
    connects — and a reply that misses the ceremony reaches the querier
    **privately**: late replies are the participants' information, not part of
    the record, whose slot simply stays absent (`wire-format.md` §5.5). **How
    long to keep listening is the querier's own patience parameter** (§21.1) —
    nothing transmits a deadline, and a promise inside a permanently archived
    response would be stale noise the moment it passed. The absent slot
    preserves the distinction without handing light clients an unfalsifiable
    excuse.
  - **Watch for hub concentration.** Weighting infra participation more heavily
    on the reliability axis creates a gradient toward meeting infra operators,
    which will make the presence graph hub-structured around infra operators. (Note
    the ~1/1000 figure elsewhere is the *packing ratio* f^(L+1), how many
    non-infra nodes one infra node can span — **not** a forecast of how many users
    will choose to run one. Actual operator density is unknown.) Those become high-capacity on the **reliability** axis only —
    §16.6 forbids the same weighting on the social-trust axis, and if an
    implementation collapses the two scalars this becomes a genuine
    re-privileging of a paying class. Check the metric against that.

#### 7.4.4 Matching, templates, and local storage

- **False rejection is the practical hazard, not false acceptance.** Cross-device
  matching under uncontrolled lighting, angle, ageing and occlusion produces a
  few percent false-reject rate; across q=5 queries the chance of at least one
  spurious `no-match` on an honest user is material. **A single negative must
  never be treated as damning by the reference policy.** The protocol's job is to
  expose the count and
  the `inconclusive` rate, and let policy set the threshold. A generous
  `inconclusive` band is also privacy-protective, since it blunts hill-climbing.
- **Template format must be canonical and versioned, and the version travels with
  the query.** Cross-client comparison requires a common extractor and fuzzing
  scheme. This is the one part of the presence layer that cannot be pluggable;
  everything above it can be. **The requirement is only checkable because the version
  is carried** (`wire-format.md` §4.5, query field 5): a verifier that cannot compare
  under it answers *unavailable* and asserts no basis, where a verifier left to guess
  would compare under its own scheme and sign a `no-match` indistinguishable from an
  identity mismatch. The subject's consent covers the version, so it is the terms of
  the comparison they countersigned and not merely the fact of one.
- **Local photo stores are themselves a hazard.** Device compromise or seizure
  exposes photos of everyone the user has met. Encrypt at rest under a key not
  held in normal working state; honour declared retention with automatic
  deletion.

### 7.5 Capture, fuzzing, and modality versioning

**Fuzzing target (query channel only).** Specific enough to discriminate from
~99% of humans; below the confidence level that would serve as court evidence.
This bound protects the *query channel* only. It provides little protection in
a record that also carries timestamp, location and graph position, which is why
the biometric stays local.

**Multiple images per capture.** Correct instinct for the false-negative
problem, but a burst of frames does not deliver it: failures within one session
are strongly correlated (same lighting, same angle, same glasses, same day), so
m near-duplicate frames give far less than the p^m improvement independence
would imply. The gain comes from **diversity, not count.** A short guided
capture with varied angle and expression. **m = 3–5 over 10–15 seconds** (chosen; §21).

**Bonus: the guided capture doubles as a liveness check.** Randomised prompts
producing motion and parallax are what distinguish a live face from a printed
photo, a screen replay, or a generated video. Presentation-attack resistance is
required regardless, so the capture should be designed for both purposes at
once.

**Storage is not the constraint.** A face crop at modest resolution runs roughly
30–80 KB; five images is ~250–400 KB per meeting, and 200 meetings over 730 days
is under 100 MB. Capture *time* is the binding cost, not disk.

**Modality versioning.** Retaining source photographs rather than templates
means any future still-image algorithm can be applied to old records. A break
occurs only at a generational change in capture modality — stereoscopic, depth,
LiDAR, which cannot be compared against a still. Policy for that turnover:
continue using still-image algorithms for verification against legacy records
while storing the new modality going forward. The
record therefore carries a **capture-modality field** now, so a future client knows
what comparison is possible without fetching the image itself.

**Default retention: 2 years**, declared in the record per §7.4.3. A *default*
rather than a limit: the subject enforces it by choosing whether to release the
capture key at all (§7.5.2).

#### 7.5.1 Retention rationale and the ageing/detection tension

**The retention window is a default the subject enforces, not a commitment the
holder makes.** Captures are sealed under the subject's keys (§7.5.2), so a
holder retains nothing it can open, and the window is enforced by declining to
release a capture key. Two years is the value both parties can expect without it
needing to bind anyone. *(Breach surface, once the leading argument for a short
window, mostly dissolved with the same change: a seized store is ciphertext.)*

Two justifications carry the value:

1. **A Schelling point for excusable non-retention.** With a universal default,
   "older than the retention window" is a coordinated and checkable excuse rather
   than an idiosyncratic claim — and deviation becomes conspicuous. An identity
   whose counterparties all declare unusually short windows is visibly anomalous.
   With no default, every policy is idiosyncratic and nothing stands out, which is
   precisely the cover a shared-identity scheme needs.
2. **Limiting damage from subject ageing.** Face-recognition accuracy degrades
   with time between enrolment and probe: modest for adults over a few years,
   severe for minors, whose faces change substantially in 24 months.

**These pull against each other.** Longer retention improves detection of
slow-burn shared-identity schemes; shorter retention reduces false rejection from
ageing, and false rejection is the more common failure (§7.4.4). Two years is a
**compromise, not an optimum on any axis**, and is recorded as chosen, not derived
(§21).

**A retention year is 365 days** — 365 × 86,400 seconds, the same fixed-day
arithmetic as verifier selection's 730-day window, and for the same reason:
calendar arithmetic differs across implementations and two clients would release
keys at different instants near expiry.

**A hard expiry is a cliff**: meetings older than the window become permanently
unverifiable, which is the regime a patient attacker would target. **The cliff is
accepted rather than softened.** A second, longer window for derived templates would
buy detection at the cost of keeping biometric material — smaller and less harmful on
breach, but still biometric and still irrevocable — for years after the images were
deleted, and it doubles every declaration, every key and every grant. **Recognition
is what a rotation rests on** (§9.1), not a stored template, so the longer window
had no second consumer to justify it.

**Re-enrolment defuses most of the ageing concern.** Parties who meet again
refresh each other's stored reference, so ageing degrades verification only for
identities met exactly once and never again — the low-value case. The reference
client refreshes on every subsequent meeting.

**Minors.** The ageing argument is strongest for young subjects and their
`inconclusive` rate will be higher under any policy. Rather than storing age —
more PII — rely on §7.4.3's subject-set retention policy and expect elevated
`inconclusive` responses in this population.

#### 7.5.2 Keystream-encrypted captures: the subject holds the key

**Each participant gives the other a 32-byte seed during the ceremony, and each
seals its captures of the other party under keys derived from the seed that party
supplied.**

So A's images sit on B's device sealed under keys **only A can derive**. B
stores ciphertext. A stores the seed, privately, in its own record of the
transaction. B can decrypt only when A releases a capture key and hands it over
directly, which happens during a subsequent ceremony involving A.

**Retention stops being a promise and becomes a consequence of meeting cadence.**
If A and B never meet again, B's copies are permanently inaccessible without anyone
deleting anything. Compare §7.5.1, where the two-year window is a commitment
nobody can verify.

**The subject holds the key to their own likeness on someone else's device.** Every
other mitigation here protects a holder's data from third parties; this is the only
one protecting the depicted person's data from the holder.

##### What it does not do

**A non-compliant client defeats it entirely**, and nothing detects the difference.
Such a client can decline to seal its captures, retain a released key after a
ceremony, or
keep plaintext alongside the ciphertext. §1.1's diagnostic returns *no enforcement
available*, as it does for every other client-side obligation.

**The honest claim is therefore narrower than it first appears: this changes what a
well-behaved holder is capable of, not what a hostile one is capable of.** What
makes it worth more than an ordinary retention promise is that compliance produces
a **structural** consequence rather than a continuing intention. A compliant holder
**cannot** decrypt afterwards even if they later want to, because they never held
the seed. The commitment moves from *"I will delete this"* to *"I run a client that
never gave me the ability"*, which is easier both to keep and to mean.

##### Why this matters for who can participate

§7.2 makes biometric custody a real cost of hosting: an operator holding
identifiable face data acquires custody obligations they may have no way to
discharge. **A compliant client holds no decryptable biometric data at
rest**, which is a materially different position from holding a stored likeness,
though what follows from it depends on a jurisdiction this document does not
attempt to assess (§7.2).

Data in memory during a ceremony authorised to display
it is not storage, and retention provisions do not reach it.

##### Retention becomes enforceable by the subject

**A enforces their own retention horizon by declining to release a capture key.**
No detection, no cooperation, nothing to audit: B's copy simply stays inert.

This inverts who holds the retention policy. §7.5.1's window was a commitment
**B** made about material B controlled; it is now a decision **A** takes about
material A holds the key to. A can enforce the declared period, enforce a shorter
one, or extend by continuing to supply.

**So the declared window is a default rather than a rule.** Two years is what a
compliant client does absent instruction from the subject, and the value of a
common number is coordination rather than constraint, an evaluator reading a
record knows what to expect without needing it to be binding.

**Two consequences a client should surface rather than bury.**

**Withholding is now a deliberate act, and it looks like unavailability.**
`wire-format.md` §5.5 admits an `unavailable` response as a filled slot, which was right when
unavailability meant a verifier was offline or had deleted in good faith. It now
also covers a subject who chose not to help, and an evaluator cannot tell the
difference. That is the correct treatment, the alternative is asking evaluators to
infer motive from an absence, but it means a refusal is invisible rather than
recorded.

**A subject who withholds broadly degrades their own recoverability.** Every
counterparty A cuts off is one that can no longer answer `photo_match` if A later
needs recovery (§9), leaving `personal_knowledge` as the basis. An aggressive
retention policy is therefore a trade against A's own ability to recover, and a
client should say so at the moment the user sets it.

**The same reservation applies as everywhere else in this section: none of it binds
a non-compliant client.** A client that kept a released key, kept plaintext, or
never encrypted is unaffected by A's refusal, and nothing detects the difference.
What sealing changes is that a **compliant** holder has no way to defeat the
subject's decision, rather than merely an obligation not to.

##### Interaction with verifier queries

**None, because the subject is always present.** §7.4.2 requires the subject's
countersignature on every verification query, so there is no case where B is asked
about A while A is absent. The sequence:

1. A and C perform a ceremony. C selects B as a verifier for A.
2. A signs C's query to B, carrying C's current template of A.
3. **In parallel, A sends B the capture key** for B's stored images of A, directly,
   bypassing C and every witness.
4. B decrypts, compares, and answers C.

**A side effect worth having: A learns which counterparties were selected.**
Verifier privacy is currently one-sided — P18 records that a verifier learns a
ceremony is under way while the subject learns nothing about who was asked. The
key-release channel makes it symmetric without a new message type.

##### Interaction with recovery

**Seeds die with the device, and that is consistent rather than a new fragility.**
A rotated or recovered key inherits only local standing (§9); rotation never
carries state forward from beyond the local trust horizon. So a participant who
loses their device loses the ability to unlock their likeness on every
counterparty's device, in the same way and for the same reason they lose portable
standing (§10.2).

The practical consequence is that a post-loss recovery ceremony rests on
`personal_knowledge` rather than `photo_match`, which §7.4.4 already treats as the
weaker basis. **The scheme adds no dependency here; it makes an existing one
visible.**

##### Construction: one sealed capture, one derived key

**The store is a sealed capture under a key derived from the subject's seed, not a
keystream the subject hands over in pieces.** Describing it as *N bytes of
keystream* left three things unspecified and one of them dangerous.

```
seed            32 random bytes, generated by the SUBJECT per ceremony
ceremony_id     the ceremony pre-commitment both parties countersign (§7.1)
k_capture   =   HKDF(seed, "rhtn/1:capture" || subject || holder || ceremony_id)
```

**Bound to the ceremony, not the transaction.** **Deriving it from `txid` would
be circular**: `txid` hashes the finalized
body **including verifier responses**, and sealing happens at capture, before either
exists. The pre-commitment is fixed before capture begins, is countersigned by both
parties and the witnesses, and is unique per ceremony — everything the binding
needed, available when the binding is made.

The capture is sealed under an AEAD. **The subject grants access by releasing the
key, and withholds it by not**: there is nothing partial to grant.

**Three problems this fixes.**

**Encryption is now authenticated.** A raw keystream is XOR, and XOR is malleable —
anyone holding the ciphertext and knowing the plaintext can produce a chosen
plaintext. A device thief could tamper with a stored capture and have it decrypt,
later and legitimately, to something the subject never presented. An AEAD refuses.

**A released key is useless to anyone but its holder.** The derivation binds
`subject`, `holder` and `ceremony_id`, so a key given to B does not open C's copy of the
same face, and cannot be replayed against a later ceremony. Without that binding a
subject releasing a key to one counterparty would silently release every copy in
existence.

**And the transfer problem disappears.** The subject sends a **32-byte seed, or a
derived key** — not megabytes of expanded keystream. §7.5.2's earlier note about
NFC and BLE throughput was chasing a problem that only existed because the
mechanism was described in terms of the expanded stream rather than what generates
it. **Any channel carries 32 bytes.**

##### Layout: the template comes first and has a fixed length

**B derives the template at capture time and stores it ahead of the source images**,
under the same seal. The template has a fixed length per modality version so a holder
knows what it is reading; nothing about decryption depends on the ordering, since one
key opens the whole capture.

§7.5.1 sets the retention window at two years and describes it as a commitment a
holder makes. **It is a capability the subject grants**, and the subject can close a
counterparty's access remotely without the counterparty acting or knowing.

**Constraints this imposes:**

- **The derived template is still biometric data** when decrypted, and is sealed
  under the same key as the images it came from. There is no lesser category here
  and no partial grant.
- **Truncated or unauthenticated ciphertext is a decryption failure**, reported as
  such. It is not evidence about the subject and must not be reported as a
  no-match.

##### One sealed capture per presence record

**A ceremony seals a new capture; it does not replace an earlier one.** Each
presence record has its own sealed store, its own seed and its own retention
windows, ageing independently. **A verifier who has met the subject several times
holds several**, and the subject's grant names which to open (`wire-format.md`
§7.3).

**Querying prefers the most recent eligible capture**, because a stale template
matches worse and says less. Nothing forbids a subject granting against an older
one — that is theirs to choose, and choosing an older capture discloses less recent
appearance.

##### Seeds are backed up with everything else

**A participant's seeds are ordinary device state and belong in the backup**
(§10.2). Losing a backup costs the seeds along with the keys and the archive, which
breaks the ability to unlock one's likeness on every counterparty's device.

**That is expected behaviour, not a failure mode to engineer around.** It is the
same loss, for the same reason, as losing portable standing: rotation and recovery
never carry state forward from beyond the local trust horizon (§9).

##### Open

- **Fixed template length per modality version**, so a holder knows what it is
  reading, and the AEAD parameters (cipher, nonce derivation, framing).
- **Whether a subject may re-derive and re-release a key after a device restore**,
  which turns on whether the seed survives backup intact — §9's rotation rule says
  a recovered key inherits only local standing, and the seed is not a key.

### 7.6 Route latency — what it can and cannot show

**Latency bounds distance from above, never from below.** The direction is easy to
invert and the inverted version is useless, so it is worth stating carefully.

A signal cannot travel faster than light, so a measured RTT *R* implies
distance ≤ (R/2) × v, with v ≈ 200 km/ms in fibre. Latency therefore gives an
**upper bound on distance, never a lower one**. Consequences:

- A **small** RTT constrains the location of the **endpoint answering the
  measurement.** Not necessarily a person, since traffic terminating at or
  tunnelling through a nearby proxy satisfies it equally. A **large** RTT
  **refutes nothing**: congestion, detours and queuing inflate it freely.
- Intersecting upper-bound disks from several witnesses yields a genuine coarse
  region. This is standard constraint-based geolocation (CBG).
- **Witness selection should therefore include nodes geographically near the
  claimed location**, since only a nearby witness can produce a tight bound.

**Resolution is worse than CBG's usual figures, because participants are on
mobile.** Radio access latency alone runs 20–80 ms, and 40 ms of baseline RTT
implies a bound of ~4000 km. Realistic outcome: **continental resolution** —
adequate for the gross impossible-travel checks this is meant to support
(Dayton and Tokyo three hours apart), useless for distinguishing Dayton from
Chicago. Do not design anything that needs city-level verification from this.

**Per-leg analysis.** Witnesses traceroute to participants and examine
individual hop latencies rather than only end-to-end time. This localises
*where* an anomaly sits and catches the characteristic signature of a relaying
proxy, a single hop with a large, otherwise unexplained jump. Caveats: many
routers rate-limit or deprioritise ICMP TTL-expired replies, so per-hop RTTs are
noisier than end-to-end minimums and frequently non-monotonic for reasons that
mean nothing; **some** MPLS configurations hide hops, though RFC 4950 extensions expose
label-stack information on others. Treat per-hop data as diagnostic,
not as added precision.

**Traceroute cannot reach the cell tower.** Mobile architecture does not expose
the radio access network to IP routing: the path terminates at the carrier's
packet gateway (PGW/UPF), and tower, backhaul and scheduling are all invisible
below it. Carriers aggregate to a small number of regional gateways, so egress
may be hundreds of km from the subscriber, and CGNAT often means the device IP
is not globally routable. The last visible hop yields carrier and coarse region
— not a tower. RAN latency also cannot be subtracted, because the
gateway-to-device segment bundles distance and radio scheduling with no way to
decompose them. Per-leg analysis therefore does **not** get below the continental
resolution stated above.

#### 7.6.1 Radio-environment co-presence is not used

Comparing local radio environments — Wi-Fi BSSIDs, BLE beacons — via private set
intersection is a natural-looking proximity check and **does not work here**, for
three compounding reasons:

1. **No timing binding, therefore forwardable.** A Wi-Fi scan takes seconds, so
   no challenge window tighter than a network round trip is achievable. A
   confederate standing in front of the honest party scans the room, ships the
   result to the remote key holder, and the remote party signs it. Overlap
   succeeds. Contrast UWB secure ranging, whose nanosecond budget is
   cryptographically bound to the session key and which a relay's milliseconds
   defeat. **Radio overlap provides no anti-relay property whatsoever.**
2. **No threat left in the middle.** Under bilateral collusion the parties can
   agree on anything anywhere. Under unilateral fraud the dishonest party is
   physically present by construction and therefore has legitimate access to the
   same radio environment. There is no threat model in between that this catches.
3. **Narrow residual, unavailable where it matters.** The one case it uniquely
   catches is a naive remote relay, two parties on a video call, one pointing a
   phone at the other's feed to capture QR codes, which the optical channel alone
   does not catch. But UWB catches it, and NFC's physical-range
   friction makes it awkward, so the value is additive only on devices lacking
   both, and iOS blocks general
   *nearby*-BSSID enumeration for ordinary apps in its public APIs (an entitled app can read the
   **current** network's BSSID, which is not enough for this purpose).

Against that: PSI machinery, capability negotiation, BLE MAC rotation, and an
uncalibrated overlap threshold that would silently penalise rural users where
there is simply nothing to scan.

#### 7.6.2 Location evidence: assertion plus corroboration

**No single channel is authoritative.** Location is *asserted* by participants
and *corroborated* by independent channels. The protocol defines **at least two
assertion methods** so the ceremony survives any one being unavailable — a
property of the method registry, not a per-record minimum: **a record may carry
any number of assertions, including none** (`wire-format.md` §4.5), and how much
location evidence a record needs is the evaluator's policy.

| Channel | Type | Resolution | Notes |
|---|---|---|---|
| GNSS | self-reported | metres | Forgeable; weight by client attestation (§7.8) |
| Serving cell ID | self-reported | ~1 km urban | Forgeable; **Android only among documented public APIs.** IOS CoreTelephony exposes carrier data, not a serving-cell identifier |
| Witness latency bounds | independent | continental | Upper-bound disks, §7.6 |
| Egress gateway IP geo | independent | carrier region | Coarse; CGNAT-affected |

**The record carries which corroborations succeeded and at what resolution**, so
a later evaluator knows whether a meeting was located to ten metres or four
thousand kilometres.

#### 7.6.3 Co-presence evidence, ranked

Strongest first:
1. **UWB secure ranging (802.15.4z).** The **strongest of the channels considered
   here** — §20.1 records that no comparison against every handset-available
   channel was made — proximity
   channel, and **not categorically relay-resistant**. Distance bounding at
   nanosecond resolution cryptographically bound to the session key requires an
   attacker to defeat the ranging protocol rather than relay a timing measurement bar
   very substantially, since a relay's milliseconds exceed the time budget by
   orders of magnitude. Decimetre-class accuracy where it works — *Ghost Peak* observed 10–20 cm normal error in the systems it tested, which is an implementation and environment result rather than a guarantee of the standard.

   **But deployed implementations have been broken.** USENIX Security 2022
   demonstrated practical over-the-air **distance-reduction attacks against
   802.15.4z HRP UWB**, including Apple U1, NXP and Qorvo parts — 12 m spoofed as
   0 m **without knowledge of the ranging key**, because the attack targets the
   physical-layer preamble rather than the cryptography above it. Resistance
   depends on ranging mode, receiver design, and implementation, not on the
   standard alone. Not universally available either.
2. **NFC tap.** A few cm, widely deployed on modern handsets though not a
   guaranteed platform capability, requires deliberate contact.
   **NOT anti-relay.** Relay attacks against NFC are a well-documented class;
   short physical range does not prevent a relay pair with a fast link. NFC
   supplies *physical-range friction*, not a distance-bounding guarantee.
3. **Optical (QR) channel.** Short-range and line-of-sight, but relayable
4. **Route latency.** Continental resolution only; plausibility filter

**Consequence: proximity evidence bounds an attacker's cost; it never establishes
co-presence.** Any channel can be defeated by an adversary able to manipulate the
physical layer it measures, and a channel's resistance is a property of its
deployed implementation rather than of the standard it follows. The ranking above
therefore orders cost imposed on an attacker, not guarantees — UWB sits at the top
and has been defeated in deployed form.

This is consistent with what §12.1 already says — proof of presence is **a cost, not
an unforgeable primitive**, and bilateral collusion defeats it regardless. The
correction moves UWB from "the one solid channel" to "the strongest of several
imperfect ones", which changes the confidence a policy should place in a record,
not the architecture.

**Practical consequences:**
- `Proximity` (§8.1) records which channel was achieved and **deliberately not the
  ranging mode or receiver implementation** — deferred (§4). The consequence for
  v1 evaluators: a UWB `pass` cannot be distinguished by mode, so policy must
  weight it as the *weakest* mode in deployment (HRP), since the known attacks
  turn on exactly that distinction.
- No implementation should treat any single channel as proof that a counterparty's
  key is physically present. Combining channels raises attacker cost; nothing here
  makes it prohibitive.

A reference client should obtain the strongest channel the hardware supports, and **record which
channel it got**, since a policy evaluating the record later needs to know the
strength of the proximity claim. None survives bilateral collusion, which is out
of scope by §12.1.

**Self-reported serving cell ID** is available on Android through
`TelephonyManager` under location permission, and **is not available on iOS** —
CoreTelephony publishes carrier and subscriber information but no serving-cell
identifier. It is forgeable by construction. It belongs in the same category as client integrity (§7.8) —
a trust-raising attribute weighted by attestation, never treated as a fact.

**Witnesses notarise; they do not verify proximity.** Under the corrected
physics of §7.6, the co-presence evidence that matters is generated *between
the two devices* and merely reported to witnesses. Witnesses timestamp, make the
event non-repudiable, and — being nominated from the counterparty's
neighbourhood — cannot be friendly notaries chosen by an attacker. That is a
narrower role than the original sketch implied, and the record format must not
imply otherwise.

**Privacy cost:** traceroute exposes each participant's network path — ISP,
corporate network, approximate region, to whichever nodes are witnessing. This
is directed probing at a known moment, correlated to an identity, by nodes the
counterparty's neighbourhood nominated, which differs from ambient exposure.
A disclosure item for the consent surface, not a design constraint.

**What persists.** Witnesses compare their bounds during the ceremony, but **raw
measurements are not written to network state**. What the record carries is the
**per-witness corroboration.** Witness identity, method and bound radius, each
signed (§12.2's `corroborated[]`), so a later evaluator sees which corroborations
succeeded and at what resolution (§7.6.2) rather than a single pre-digested
figure. **There is no single "imputed location" field**, deliberately: one number
would hide which corroborations succeeded and at what resolution, which is what an
evaluator actually needs.

**Do not attempt proximity by network timing.** A 0.00001 light-second budget is
3 km, i.e. 10 µs of light travel, against 20–80 ms of mobile radio latency with
tens of milliseconds of jitter, a noise floor roughly 3000× the signal.
Repeated sampling helps less than it appears: minimum-of-many-samples is the
standard constraint-based-geolocation technique for suppressing *queueing*
jitter and does work for that. What it cannot remove is the **systematic
radio-access floor**, which is the dominant term here and does not average out. Proximity
comes from the optical and ranging channels (§7.1), never from latency.

**Terminating vs relaying proxies.** A relay still traverses the attacker's true
position and loses the tight upper bound, so it is detectable. A *terminating*
proxy, a client genuinely running on a VPS in the claimed city — is
indistinguishable at the network layer. That case is what the optical and
ranging channels exist for.

### 7.7 Location field
Coarse **geohash at precision 3–4**, not precise coordinates. Standard geohash
cell dimensions at the equator are ~156 × 156 km at precision 3 and
~39 × 19.5 km at precision 4 — **maximum dimensions, at the equator**; east-west
width shrinks with latitude. **Precision 3 is the default.** It satisfies every
intended check below and identifies a region rather than a venue.
Sufficient for the intended checks — impossible travel by rigorous induction
(two places thousands of miles apart within hours) and statistical divergence
from a user's typical patterns, while removing venue-level identification.

### 7.8 Client integrity
Every step depends on a sensor reporting honestly, and a modified client can
fabricate the camera feed, the ranging result and the timing. Hardware
attestation (Play Integrity, App Attest) would reintroduce the manufacturer as a
global trusted party, the same tension as §23.2's autonomous-participation entry raises about attestation. Resolution: treat attested
client integrity as a **trust-raising attribute carried in the record, not a
requirement**, so policies can weight it and users on unattested or open-source
builds are not excluded. A user can only trust their own client's features,
never the presumed features of a counterparty's client.

---

## 8. The presence record

### 8.1 Presence record format

Field-level specification. **`wire-format.md` is authoritative on encoding**; the
shapes here are for reading alongside the rationale and will drift if edited
independently. All signature fields are **COSE structures** (RFC 9052), see

**A record carries no participant locator.** Nothing that transmits or evaluates
a presence record reads one (§8.1.1), and a historical position would be stale by
construction — `wire-format.md` §2.3's strictly-greater rule means any later locator
supersedes it. The absence removes the **historical** position series: records do not
trace where a participant sat over time. It does **not** remove position inference —
the keyhashes remain, an observer holding topology can look up where those keys sit
now, and the witness set still discloses a neighbourhood (§7.1).

**They have drifted already, and in a way worth naming**: the shapes below give
witnesses and corroborations their own `COSE_Sign1`, while the wire schema has
witnesses authenticate as **envelope signers** and gives corroborations no
signature of their own (`wire-format.md` §4.5). **Read the encoding document for
signature placement.**
§5.1 and the wire format's §1. `H()` denotes the canonical hash.

```
PresenceRecord {
  # --- header ---
  type              : "presence"
  subtype           : enum{normal, formation}    # body — structural. §13.2:
                                                 # PERMANENT, never rewritten; a
                                                 # formation record must never age
                                                 # into looking like a normal one
  schema_version    : uint16
  txid              : H(canonical body)          # content-addressed
  started_at        : timestamp                  # body — monotonicity and the
                                                 # 730-day window need it
  finalized_at      : timestamp                  # body — window checks need it
  disclosure_root   : bstr[32]                   # §8.1.1

  # --- participants (exactly 2) ---
  participants[2] {
    identity        : keyhash                    # permanent
    signature       : COSE_Signature within the envelope's COSE_Sign
    # retention and client_integrity are DISCLOSABLE (§8.1.1) and travel in the
    # disclosure set rather than the body. No locator: see below. Disclosure
    # consent is the per-query subject countersignature (§7.4.2)
  }

  # --- proximity evidence --- DISCLOSABLE
  proximity {
    channels[]      : { kind: enum{uwb, nfc, optical, latency},
                        result: enum{pass, fail, unavailable},
                        resolution_m: uint32,     # claimed strength
                        binding: opaque }         # session-key binding, if any
    strongest       : enum                        # highest-ranked channel passed
  }

  # --- capture evidence (NO biometric data) --- DISCLOSABLE
  capture {
    modality        : enum{still, stereo, depth}  # §7.5, forward-compat
    image_count     : uint8                       # 3-5, guided variation
    liveness        : enum{pass, fail, not_performed}
    liveness_version: uint16
  }

  # --- location --- DISCLOSABLE
  location {
    asserted[]      : { method: enum{gnss, cell_id},
                        geohash: string(3..4),    # ~156 km at p3, ~39x19.5 km at p4; §7.7
                        self_reported: true }
    corroborated[]  : { witness: keyhash,
                        method: enum{latency_bound, egress_geo},
                        bound_radius_km: uint32,
                        signature: COSE_Sign1 }
  }

  # --- witnesses ---
  witnesses[] {
    identity        : keyhash
    nominated_by    : keyhash          # MUST be the counterparty, §7.1
    attests         : { protocol_ran, both_responsive, latency_bound }
    signature       : COSE_Sign1
  }

  # --- verification queries (§7.3) ---
  verifier_responses[] {
    verifier        : keyhash          # a prior counterparty of the subject
    subject         : keyhash          # which participant was checked
    query_id        : H(query)
    result          : enum{match, no_match, inconclusive, unavailable}
                                       # no pending value: an unreachable
                                       # verifier simply does not appear
    selection_basis : enum{known, reachable, discretionary}
                                       # the SELECTOR's claim of why this
                                       # verifier was picked (§8.1.2)
    basis           : enum{photo_match, personal_knowledge, both}
                                       # absent when result is unavailable —
                                       # no evaluation, no basis
    template_version: uint16           # omitted when basis = personal_knowledge
                                       # or absent
    subject_consent : COSE_Sign1       # BY THE SUBJECT, over the query_id —
                                       # see `wire-format.md` §5.6 (§7.4.1)
    signature       : COSE_Sign1       # BY THE VERIFIER
  }
}
```

**Why `basis` is a separate field.** A verifier who personally knows the subject
does not need a stored photograph, and their memory outlasts any retention
window. But the two bases fail in opposite directions: **human memory is robust
to elapsed time and vulnerable to persuasion** — "it's me, I lost my phone" from
someone who looks roughly right is the classic social-engineering vector, and a
friend under social pressure signs what a matcher would not, while **template
matching is unpersuadable but degrades with time, lighting and ageing**. Neither
dominates, so the record distinguishes them and lets policy weight them
differently. A `personal_knowledge` response from a long-standing high-trust
contact may reasonably outweigh a `photo_match` from a one-time acquaintance.

Note also that a photo match is computed on the verifier's own device and
reported as an assertion — **no third party can check it was performed
honestly**, exactly as with a memory claim. The photograph protects the
*verifier* from being deceived; it adds nothing to third-party verifiability.
Retention therefore matters in proportion to how weakly the verifier knows the
subject: redundant for close friends, essential for someone met once years ago.

**Why verifier responses live in the record rather than going to witnesses.**
If C receives the verification result privately, a C colluding with a fake B
simply suppresses a negative and the record still looks clean, which destroys
the check's value to third parties, who are the people the record is for.
Carrying A's signature in the record means C can neither fabricate nor suppress
it, and the **absence of an expected verifier response is itself visible**. This
beats routing through witnesses on integrity, since the attestation persists
rather than living in witness memory.

**It is a trade, not a dominance.** Persisting verifier identities in the record
exposes a sample of the subject's *prior* counterparties to every future
evaluator see §19.2, which treats the resulting social-graph leakage as an
open finding.

**The verifier sample.** *Newly created encounter evidence should query a
sample of the subject's prior counterparties, sized by a reasonableness
criterion and selected by the counterparty's own recognition.* The sample sizes
the evidence; **it does not gate finalization** [author, 2026-09-01]: the record
finalises with whatever responses arrived, an evaluator reads the count against
the criterion, and late replies reach the participants privately, never
retro-inserted (`wire-format.md` §5.5). Present encoding:

> For a subject with *n* recorded presence transactions in the preceding 730 days,
> seek **min(floor(n/2), 10, |candidates|)** verifiers,
> where `|candidates|` is the count of **distinct** prior counterparties. The third
> term is necessary because *n* counts transactions while candidates are people:
> twenty meetings with one person would otherwise demand ten verifiers from a pool
> of one. Rounding is **floor**, stated explicitly because an unstated rule differs
> by one query at every odd *n*. ***n* is the subject's own claim** — it counts a
> bundle the subject curates — so this is a **reasonableness criterion**, never a
> security check (§8.1.2).

A brand-new identity has n=0 and queries none; an established one caps at ten, so
the check never scales with history. Non-responsive verifiers are noticed and
de-trusted **separately**, as a fact about the verifier, rather than blocking the
transaction.

#### 8.1.1 Selective disclosure, and what it reaches

**A holder can withhold the fields a given recipient has no use for.** Each disclosable field is committed as a salted digest and the body
commits to the list; the envelope signature covers the body, so a recipient verifies a
partial presentation against the same signature. Encoding: `wire-format.md` §4.5.1.

**Disclosable — seven fields:** location evidence, per-participant retention,
client integrity, capture parameters, and proximity channels. *`started_at` and
`subtype` are body fields rather than disclosable ones: the structural rules
consume them, and withholding them would conceal nothing — `finalized_at` already shows the
ceremony's day, and a formation record's
absent evidence arrays announce its subtype regardless.*

**This was scoped by a sweep, not by preference.** Eleven exchanges transmit or
evaluate a presence record, and what each actually reads decides what may be withheld:

| Exchange | Reads | Location? |
|---|---|---|
| Ceremony, at creation (§7.1) | everything | were there |
| Witness signing (§7.1) | its own corroboration | **contributes it** |
| Verification by query (§7.3) | a fuzzed profile and a query id — **not the record**, which does not exist yet | no |
| Verifier selection (§8.1.2) | the handed bundle — the selector's own act, replayed by nobody | no |
| Response accounting (§8.1) | verifier responses | no |
| Structural verification (`wire-format.md` §3) | the body: signatures, back-pointers, timestamps, subtype, participant distinctness — plus `proximity`'s strongest rule when revealed | no |
| Adoption's proof-of-presence reference (§6.1.1) | that the record exists and names these two parties | no |
| Archive presentation to a prospective patron (§16.7) | signatures, and counterparties the patron already knows (§10.1) | **benefits** |
| Trust metric (§16.2) | graph edges, which come from adoptions | no |
| Presence-based recovery (§9.1) | verifier responses | no |
| Late response, capture key grant (`wire-format.md` §7.3–b) | `txid` | no |

**Ten of eleven have no use for it.** The mechanism exists because a record currently
reaches all of them whole.

##### What it does not reach, and why

**Not the signer set.** `kid` sits in the COSE protected header, on the envelope,
outside the body — so **every participant and witness keyhash is on the record however
little the body discloses.** Worse, `wire-format.md` §3 infers signer role by comparing
each `kid` to the body's fields, so withholding the participant or witness lists would
break role inference: a correctness failure, not a privacy limit. That inference was
itself a deliberate choice, and a signer-role field was rejected as redundant and
forgeable.

**So P2 and C2 are untouched** — the verifier and witness graph is the High finding
this cannot help with. §19.2 records the only direction that would, threshold or
aggregate signatures, and what it costs: the legibility of who answered.

**Nor the record's evidential core.** Chain back-pointers, participant and
witness keyhashes, `finalized_at` and the verifier responses all stay in the
body: they are what an evaluator weighs by recognition (§16.1), and who took
part is the one thing a presence record exists to say.

##### The use and the harm are the same computation

**Location's one legitimate consumer needs a series, and so does the leak.** §7.7
gives the checks as impossible travel by rigorous induction and statistical divergence
from a user's typical patterns — both longitudinal, neither supported by a single
geohash. P21 says the harm is longitudinal for the same reason: a time series of coarse
cells reveals commuting, travel, employment and residence.

**So field-level disclosure cannot separate the use from the harm. It separates
audiences.** The party that legitimately runs impossible travel is a prospective patron
reading an archive prefix — the same party P19 flags for receiving an intelligible
history and C4 flags for reading it against local knowledge. **That is the trade this
mechanism offers and the whole of it**: the one recipient with a use for location is
also the most dangerous holder of it, and everyone else stops receiving it.

##### Withholding is visible

The digest count and labels always travel, so a recipient knows a field exists and was
withheld. Same posture as absent selected slots and `unavailable` responses: absence is
legible and a policy may weight it, rather than being silently indistinguishable from a
field that was never there.

**Over-asking is not available.** A recipient can weight what it receives however it
likes — §16.1 makes policy pluggable — but it cannot make an exchange carry a field the
exchange does not define. §1.1: the evidence schema is the one thing that is not
pluggable, because the schema is the interface and policy is only the interpretation.

**Cost: 16 bytes per disclosable field at rest**, about 112 bytes on a ~35 KB record,
**0.3%**. Presentation size does not otherwise fall — a presence record is ~96%
signatures — so this buys disclosure control and not bandwidth.

#### 8.1.2 Verifier selection

**Selection is by recognition, and determinism is retired** [author,
2026-09-01]. The old rule — a hash-rank sample over committed nonces, so the
parties could not influence which prior counterparties were selected — rested
on a pool the parties do not control. The pool is a bundle the subject curates
(`wire-format.md` §5.4), and a deterministic pick over a curated list is the
curator's pick. It also served the wrong audience: the pick's regularity was
provable only to someone holding the subject's full history as of the
ceremony, which the bundle model and §19's privacy posture both refuse to
grant anyone. What every evaluator actually does — here and everywhere else
(§16.1) — is weigh who answered by whether they recognise them.

**The PoP is a connection between individual users, not between subnets.** Its
most important property is that the participants themselves are confident in
who they met, and can prove that meeting to another person who has met the
same counterparty. Selection now serves exactly that property:

- **Each participant selects the other's verifiers** — the party who performs
  a selection is never the party it is about. Unchanged.
- **The selector picks people it can vouch for**, in descending
  trustworthiness: users it has met; users in any of its trust horizons;
  users someone in any of its trust horizons has met; users in the trust
  horizons of users it has met, where foreign topology is visible at all. Two
  edges over the graph of meetings and horizon-mates is the halting
  condition.
- **Where the bundles surface no common acquaintance, the parties go
  fishing**: either proposes further candidates from its own history for the
  other to test against its own knowledge — a conversation over the direct
  channel, recorded nowhere.
- **The remainder is discretionary fill**, and marked as such: each response
  carries the selector's claim of its basis (`wire-format.md` §5.5), because
  a later evaluator cannot reconstruct the selector's acquaintance graph and
  should not be invited to guess.

**The number sought is `min(floor(n/2), 10, |candidates|)` — a reasonableness
criterion, not a security check.** *n* counts a bundle the subject curates, so
it is entirely the subject's claim; the formula sizes diligence, and an
evaluator comparing a record's response count against it learns how thorough
the ceremony was, never how complete the history is.

**What protects each party is their own act, not a recomputation.** The
selector queried people *it* trusts about the counterparty's continuity; the
subject consented to every query about itself (§7.4.2) and released every
capture key (§7.5.2); and a later evaluator weighs the responders by its own
recognition. Nobody owes anyone a proof of how the picking was done, and no
distant audience could have checked one.

### 8.2 Presence transactions at the network layer

- **Propagation class: attestation → pull, not push** (§15). *Whenever evidence is
  delivered in response to an evaluator's request, the delivery MUST
  cryptographically bind that evaluator's challenge*, so solicited and unsolicited
  evidence are locally distinguishable. Present encoding: the delivery carries the
  requesting evaluator's nonce. The reference
  client never floods presence records; a foreign implementation that sends them
  unbidden cannot be prevented, only recognised. Stored by the two participants, their patrons, and the
  witnesses; fetched on demand by evaluators.
- **Signature tier: post-quantum.** This is the one transaction type where the
  §5 tiering argues *for* PQ signatures rather than against — presence records
  are low-volume and must remain verifiable for decades, which is exactly the
  long-validity case. With ~10 envelope signers — 2 participants and ~8 witnesses,
  since **verifier responses are embedded evidence signing classically rather than
  envelope signers** (§5.1), at ML-DSA-65, expect **~35 KB per record** (10 × 3,373 B —
  each signer contributes one classical and one PQ entry, §5.1 — plus body; 26–48
  KB across the three parameter sets). At 200 meetings over 730 days that is ~7 MB
  per user — negligible.
- **Immutable once finalized.** *Once every party required to authenticate a
  piece of historical evidence has finalized it, nobody may alter that evidence
  in place; later assertions must be separately signed and must cryptographically
  refer back to the evidence they supplement.* Present encoding: `LateResponse` (`wire-format.md` §7.4), signed by
  signed reference to `txid`.
- **Rate**: far below the 1-per-100s threshold in §1. Ceremony duration is
  deliberately minutes (§7.1), which caps throughput at the human.

## 9. Key compromise and recovery

**The threat decay cannot reach.** Inactivity decay (§16.5) is useless here: a
compromised account has *fresh* activity by definition. The danger is not the
acquaintance who disappears for two years and returns. It is an account that
legitimately built standing over years being taken over, with the attacker
cashing out that endowment in a burst of abuse.

### 9.0 Rotation is adoption

**Adoption and key rotation are one operation, not two.** Both instantiate a
keypair on the network with some initial attestation state, and the credibility
of both is bounded by the credibility of the vouching party. Archive ingestion
is already an optional part of adoption (§16.7), so adoption spans the
genuinely-new user *and* the arriving user carrying a decade of history —
rotation is simply the case where the presented archive belongs to a different
key.

**Seal the old line as the old key's last act.** "Evaporates" describes propagation,
not capability: the old key still exists, and nothing so far stops it publishing a
fresh locator to anyone holding a stale one — §12.3 Case 2 guarantees those holders
have no other source of truth, since nothing redirects on the subject's behalf.
**Setting the retired series to its maximum counter forecloses that**
(`wire-format.md` §4.6), at the cost of one self-signed locator, unilateral and
needing no patron. The old key is in hand at rotation — it signs the `Recovery` — so
this is the moment to spend it.

**It does not burn the key globally, and cannot.** A seal reaches exactly as far as
the locator carrying it, so parties you never contact and subnets you never enter
never see it. **They are also not the population at risk from it**: redirection needs
a cached locator to redirect, and someone holding none has nothing to be moved. A
thief presenting the old key to them is attempting a *first contact*, which §12.3
Case 0 makes an out-of-band act — a meeting or a referral from a mutual contact — and
which the presence layer answers rather than the routing layer. **Affirmative closure
for the parties you reach, and silence about the rest**, which is the right division
because the two face different attacks.

**Burning a key outright needs no mechanism of its own: rotate, and never reissue.**
A series is continued by a patron-countersigned reissue (`wire-format.md` §4.6), so
declining to take one leaves the sealed line as the last word that key will ever have.
Sealing and continuing is a repair; sealing and stopping is a retirement. **The
difference is entirely in what the subject does next**, and the protocol needs no
revocation object to express either.

**So a rotation evaporates the old identity and instantiates a new one, and whether
the inheritance is carried at all is the subject's choice.** A plain rotation
carries nothing: the upstream sees a disavowal and an adoption in quick succession
and **cannot prove they are the same person** — the arriving node may equally be one
that displaced another from a slot. A **recovery** adoption is the other case and
publishes the link deliberately, `prior_key` plus verifier continuity attestations
(`wire-format.md` §4.1), which is why §19.7 item 3 accepts that recovery destroys
unlinkability. **That publication reaches the horizon and stops there.**

**Beyond the horizon neither case exposes anything**: the old locator is simply an
address that stopped working, with no explanation offered and none available. This
is why no message anywhere reports that a key was rotated, and why a currency
attestation asserts only that an identity is current in its issuer's subnet
(§12.6.5).

In flow terms they are identical: each admits a vertex whose inbound capacity is
limited by its vouchers, so §16.2's min-cut metric handles both unmodified.

**No separate discount is needed**, and none should be added. A rotated key's
trust reaches an observer only through the attesters
who bound old key to new, so it is capped by *their* capacity, the original
identity had many independent paths accumulated over years, the rotated one has
however many verifiers vouched. The discount is arithmetic, not policy, and
specifying it separately would double-count.

*Caveat:* this is a property of the **reference metric, not the protocol**. A
node running a naive metric that tallies archive transactions directly would
grant the new key full credit. Same tension as §16.4.

**Verification is evidence, not authority.** A verifier supplies a signed
"this is the same person" fact, by photo match or personal knowledge (§12.2
`basis`), which is an *input* to an adoption performed by a patron. The patron
bears the responsibility, since their countersignature is what is on the line.
This is exactly the shape §7.3 already uses and needs no delegation primitive.

**Verification is bounded by trust reachability, not data visibility.** A
verifier anywhere holds their own presence record of the old key, since
attestations are stored by participants rather than scoped by horizon. What a
distant verifier lacks is *capacity*: under a flow metric there is little or no
path from the returning user's old neighbours to them, so the attestation is
valid and conveys nothing. The constraint is therefore soft, an unusually
well-connected distant verifier could still convey something, but in the
typical case a verifier outside the user's last active Dunbar Org cannot
substitute a new key for an old one without bringing the user into their own
neighbourhood, which is to say: without adopting them.

**Recovery cost scales with reach, automatically.**

| Recovering through | Topology change | Cost |
|---|---|---|
| Your own patron | None. They re-adopt you under a new key | Minimal; the common case |
| A nearby node in your Dunbar Org | Local | Modest |
| A distant friend who knows you personally | Transplant into their neighbourhood | Loss of old position and most standing |

No parameter tuning required. It also answers "what if my patron is the
attacker": recover elsewhere and accept the transplant.

#### 9.0.1 One procedure: key and presence together

**A rotation carries both the old key's signature and a prior counterparty's
in-person recognition.** Neither alone is enough, and there is **no lost-key
variant**.

| Half | What it proves | Why the other is still needed |
|---|---|---|
| **The old key signs** | Possession of the identity being rotated from | A thief holds that key too, and could rotate with it |
| **A prior counterparty recognises you, in person** | A human who knows you says you are you | A verifier can be mistaken or lying, and nothing checks it |

**A key you cannot sign with is not recoverable.** You make a new identity and are
re-adopted by people who know you. This is harsher than a recovery mechanism and
cheaper than one: subnet status is a vague thing, and in a network of close personal
acquaintance a team that recognises you reconstitutes your access quickly.

**The residual, accepted.** A patron who steals your key can satisfy both halves in
its own subtree — it holds the key and it can assert the recognition. **That subtree
is effectively its property**: if it wants to stand up a straw man and call it by
your name, the answer is to go elsewhere and make new friends, which the design's
whole exit argument already assumes you can (§6.2.1).

#### 9.0.2 Propagation, and identity forking

Rotation propagates as **the topology transactions it is made of** (§15) — a
disavowal and an adoption — pushed within the horizon and folded into aggregate
state beyond. **No message anywhere says "rotation"**, and the inheritance linking
the two is not carried (§9.0). Parties outside the horizon holding a cached key
binding are handled by a **currency-attestation query** addressed using the anchor
and path the introduction already carries (§12.6.5); what they learn on contact is
whether that identity is **currently attested**, never what became of one that is
not. So the gap
§15 appeared to have is **not a further message class**; it is a propagation
*pattern*, "push near, redirect far", available to topology-class messages.

**Forwarding-record authority** cannot come from the old key, which may be
precisely what was compromised. It comes from the adopting patron's signature
and the verifier attestations behind it.

**Identity can fork, and that is correct behaviour, not a defect.** Two
competing recovery claims become two adoptions by different patrons, and since
the two keys are distinct nodes each with their own patron, both can be
simultaneously valid. Observers route
to and trust whichever attester they trust; there is no mechanism to declare a
single heir to an archive, and two keys may carry lineage from the same history
indefinitely. §6.2's monotonic sequence number cannot help even in principle,
since a thief holding the key signs a higher counter than the legitimate owner.

The paradigm: **one secret authenticating to mutually non-interacting networks**.
A person's LinkedIn reputation and Twitter reputation are separately persisted
and the platforms neither talk to nor trust each other — yet nothing prevents
using the same password on both, or changing it on one and not the other. The
adversarial case maps too: a stolen password changed on one site loses you that
site and leaves the other intact, recovered through that site's own process.

Implementers assuming an identity has one current key will build something
subtly wrong.

**Boundaries are ragged, not clean.** Unlike the SSO picture, a fork can be
*intra-org*: a thief adopted by patron X and the legitimate holder by patron Y,
both inside the same Dunbar Org, leaves shared neighbours holding two claims and
choosing between X and Y.

Two consequences requiring mitigation:

- **A fork can be invisible to the forked-from.** With a password you notice when
  login fails; here, a thief adopted in a neighbourhood you do not reach leaves
  you operating normally and unaware.

  Mitigation: a node's patron holds a signed **currency attestation** (§12.6.5),
  and this is **pulled, not pushed**. When a node introduces itself to a distant part of
  the network, the anchor and path in its locator (§12.1) tell the recipient which
  patron to query. A thief's patron will answer with a competing assertion —
  which is the point; visible-but-unresolved beats invisible, and this
  architecture tolerates that state everywhere else.

  This is **not a new push obligation**: the currency-attestation query is the pull
  endpoint of the "push near, redirect far" pattern (§15) already specified above.
  The rotation *delta* is pushed within horizon, because the Dunbar Org needs it
  to keep routing; the currency *state* is queried by everyone else on
  contact. Volume therefore scales with introductions rather than with
  population, and the control plane is never flooded with standing assertions
  nobody is asking for.

  - **Divergence notification.** Pull makes a fork visible to *inquirers*, not to
    the forked-from — neither the legitimate holder nor their patron learns of a
    competing claim adopted elsewhere. **A conforming inquirer who receives divergent assertions notifies both patrons.**
    The inquirer is the only party holding both claims at once, is already in
    contact with at least one, and has an obvious interest in the answer, so
    discovery is a byproduct of inquiry rather than a broadcast obligation.

    **Nothing compels this and no patron can detect its absence.** A patron
    receiving no notice cannot distinguish *no divergence was observed* from *an
    inquirer observed it and said nothing* — the inquirer is an arbitrary third
    party sharing state with neither patron (§1.1). **What can be made durable is
    the observation, not the behaviour**: an inquirer-signed notice naming the two
    conflicting assertions would be evidence standing on its own. Unspecified; §22
    records it.

    **The disclosure this creates runs in the design's favour.** It reads at first
    as a privacy cost — a stranger's inquiry generating notifications that name
    parties to it — and that reading does not survive inspection. **The
    inquirer's exposure predates the recovery**: a thief holding the key already
    reads everything addressed to it, so whatever the inquirer lost was lost at the
    theft. Recovery only lets the legitimate holder regain one subnet, up from
    none.

    **And a third party who sees a rotation and doubts the un-rotated binding
    elsewhere is doing the right thing.** It devalues the memberships the thief
    still holds. This is the only mechanism here that damages a thief across
    subnets they retain, and it works precisely because the fork is visible.
  - **Patron unavailability.** A leaf node's patron may be a light client with
    unstable uptime. Fall back to the patron's **siblings**, who already
    replicate its data (§3.4). "No answer from patron or siblings" must be
    distinguishable from "answer attests this identity is current".
  - **Staleness.** A cached currency attestation goes stale if the subject's
    identity is superseded afterwards, the same shape as locator staleness and
    given the same treatment: a TTL on the answer, and **nothing that redirects**.
    §12.3 Case 2 is explicit that no party holds a pointer to where a superseded
    identity went — the holder re-resolves from a higher ancestor or re-establishes
    socially, and learns nothing about why.
- **Misconduct must not bleed across an unauthorised fork.** If a thief's key
  commits fraud, an observer might discount anything carrying that archive,
  staining the legitimate holder. Norm for the reference metric: *responsibility for an
  action attaches to the actor that authorised it and to the parties that vouched
  for that actor at the time; shared historical evidence must never transfer
  responsibility between competing successors that did not authorise one
  another.* Present encoding: **attribute misconduct to keys and to vouching
  patrons, never to archives.** Consistent
  with attribution everywhere else in the design, the patron's countersignature
  is what is on the line.

### 9.1 Presence-based recovery

**Physical presence is the half of a rotation a key thief cannot steal**, which is
why every rotation carries it (§9.0.1). Run §7.3 in reverse:

1. A user rotating a key meets, in person, someone they have met before.
2. **That counterparty recognises them** — from memory, as one acquaintance
   recognises another. A stored photo record can support the judgement, but the
   judgement is the person's.
3. On recognition, the counterparty co-signs the **key rotation** binding the old
   identity to a new keypair, alongside the old key's own signature.
4. Further such attestations, from counterparties the observer independently
   trusts, make the rotation more credible, with no global authority adjudicating
   and no consensus required.

**The recognition is a human act, and that is the point.** Recording it as
`basis: personal_knowledge` (§8.1) is the ordinary case rather than the degraded
one: a template match is a machine agreeing that two images resemble each other,
where recognition is somebody who knows you saying so.

**Retention does not gate rotation.** The presence *record* is permanent; only the
photograph expires. A counterparty who personally knows the subject still holds
signed proof of the original meeting, and recognises them without needing a stored
image at all.

**The residual is weak-tie-only participants.** A user whose network presence is
entirely conference contacts in distant cities, with no close friends on the
network, has nobody who would recognise them and nobody within reach to meet — so
they have no rotation path once captures expire, and their verifiers depended on the
stored comparison in the first place. This is the population §9.3 flags.

### 9.2 Supporting mechanisms

**Behavioural anomaly weighting.** A burst is anomalous relative to the
account's own history regardless of accumulated standing. A fiftyfold jump in
transaction rate is computable locally by any observer from data it already
holds, no protocol change, no shared state.

- *Limitation: no baseline for strangers.* Under pull-not-push an observer may
  have no history at all for a subject it is evaluating for the first time, and
  therefore nothing to call anomalous. **That is the correct answer rather than a
  gap**: per-observer trust (§16.1) says an observer who holds no history extends
  no credit. A shortcut around it would be a shortcut around the model.

**No patron publishes activity summaries for a subordinate.** A portable
behavioural figure asserted by one party and consumed by strangers **is a
reputation signal**, which §1 refuses and §16.1 replaces with per-observer
evaluation — the reader cannot verify a count and would be trusting the patron
transitively, the shape the design declines everywhere else. It would buy little
besides: the patron already vouches by countersigning, and **the honest answer to
"I have no history with this person" is that you extend no credit**, not that
someone else's number stands in for the history you lack.

**When a participant under another party's authority authorises an action that
materially changes an authority binding, or establishes a cross-tree infrastructure
relationship, its client makes that action visible to the party currently vouching
for it.** **The notification is observational only**: it must
neither gate nor delay the action.

That criterion, not a list of transaction names, is what determines whether a future
operation notifies. Present encoding: adoption, departure and peering all broadcast,
and a compliant client tells the patron.

**There is no veto, and no delay before an action takes effect.** A challenge
window during which a patron could block fails §1.1's test: **a patron shares no
state with a subordinate's client.**
Nothing stops that client forming a transaction, signing it, replicating it, or
acting on it, and nothing makes it wait. An adoption is signed by node and patron;
a peering by two infra nodes. A third party asserting a veto over either has no
mechanism to exercise it.

**What a patron actually holds is refusal and repudiation**, both of which are
real:

- **Refuse resource access.** The patron's node evaluates who may reach the
  resources it hosts (§11), so an unwanted sub-subordinate simply never gets in.
  This is genuine enforcement because the node and the requester share the state
  the decision turns on.
- **Disavow the subordinate** (§6.2.2). Public, costly to the issuer, and the
  network's only negative attestation.

**Rejection produces a fork rather than a block.** A patron who refuses to
recognise a subordinate's adoption does not undo it: the sub-subordinate exists in
both readings, still attached in the subordinate's own record and root of a new
one-node subnet in the patron's. That is §3.1.1's membership plurality reached
through a different door, and it needs no machinery.

**A single keypair per identity is sufficient**, and the reason is not that
anything can be blocked:

- **A thief's natural move destroys most of what they stole.** Relocating the
  identity under a patron they control costs the new patron's acceptance and leaves
  only what is verifiable from a one-sided history as their starting point. The
  attack is self-punishing.
- **Detection is the defence, and response follows detection.** Notification makes
  a compromise visible; disavowal and resource revocation are what happens next.
  A second credential would add a key to steal without adding a power anyone can
  exercise.
- **Departure is not blockable and does not need to be**, for the same reason.

What remains is a thief cashing out **in place**, which notification and
behavioural anomaly weighting surface rather than prevent, with presence-based
recovery (§9.1) as the backstop and disavowal as the response.

**Social revocation.** The patron and/or a threshold of the down-line co-signing a
revocation is native to the hierarchy: the parties who vouched can un-vouch. Note
this is **repudiation, not prevention.** A co-signed revocation says the vouchers
withdraw their support, and the repudiated identity's own client is unaffected. Needs specifying alongside the rotation format. Subject to the
same departure exemption above.

### 9.3 Scope boundary: weak-tie-only participants

Rotation requires the user to reach a prior counterparty **in person** (§9.0.1).
That works for someone embedded in a local group and fails for someone whose
meetings were all at conferences in other cities. Such users have real standing and
nobody within reach who would recognise them, and once photographs expire they also
lose template-based verification (§9.1).

**This is intended behaviour, not a gap.** The design is not a universal network
architecture; it deliberately favours in-person, long-running relationships, and
degraded service for participants who have none is the thesis working as
specified rather than a defect to engineer around.

Social revocation via the patron partly covers the case anyway, since a patron
is a relationship the user can probably reach, though that places weight on the
patron relationship being geographically local, an assumption the topology makes
nowhere else.

### 9.4 Rotation and history portability

A rotated identity must carry its archive forward, and the archive is signed under
the old key (§16.7).

*When a participant replaces the credential under which earlier parties
authenticated its history, the replacement evidence must cryptographically
connect predecessor to successor strongly enough that those earlier attestations
remain verifiable under the successor claim — without letting a thief perform the
same connection.* Present encoding: rotation records chain old key to new.

## 10. The archive

**Every transaction advances its signers' archives.** Adoption, departure,
disavowal, peering, series reissue and presence alike. The archive is what makes a history
presentable to a party who was not there, and what stops that history being edited
after the fact.

**That list is exhaustive, and the exclusions matter.** A resource
registration (§11.5) and an abuse report (§11.6) are signed, but neither advances an
archive: the first is **current state a host replaces**, the second is a private
complaint held by one owner. **An archive is topology and presence.** Anything that
travelled with it would be presented to every future patron along with the history
it was carried by — which is the wrong audience for what a person runs and the wrong
duration for what they used to run.

It is introduced here rather than under proof of presence because it is not a
presence mechanism: presence records are the largest thing it carries, not its
subject.

### 10.0 The kinds of history, and what each is for

**Five mechanisms are routinely spoken of as though they were one.** They differ in
what they establish, who governs them, and whether they are evidence at all.

| Kind | What it is | What it establishes | Governance and retention |
|---|---|---|---|
| **Transaction archive** | Each signer's own hash chain over the topology and presence transactions it signed | The history presented at an adoption. **Continuity is proven by the chain** | The node's own. Prunable at a checkpoint, floored at the 730-day window (§10.2) |
| **PoP records** | The signed artifact of a face-to-face ceremony (§8.1) | That two people met, and the **witnesses and verifiers available for a later ceremony with anyone**. Potent under any sequence and in any subtree | **Not patron-countersigned** (§6.4). A PoP **is** a transaction and enters the archive like any other — but the chain does not govern its *use*: identifying validators for a later ceremony does not depend on the archive's continuity, and a counterparty is **handed a bundle rather than walking the archive** (§8.1.2). They survive pruning of the chain that carried them |
| **Sequence number** (`counter`) | Freshness for asynchronously updated routing information (`wire-format.md` §2.3) | Which of two locators for one node is current | **Not history, and not an enforcement mechanism.** Neither the archive nor its chain |
| **Seqno series** | A grouping of sequence numbers, generally though not necessarily bound to one subnet or to a run of subnet memberships | That one user has several routing-table entries locating them in different subnets. The series follows the subnet, and address updates use the counter in that same series | Not history. Advanced only by a reissue |
| **Series reissue** (type 7) | Branching a new series off an existing one, patron-countersigned (`wire-format.md` §4.6) | That activity is partitionable by subnet — and, by election, a **checkpoint** for retention and archive look-back | Advances the archive, like any topology transaction |

**The first two answer questions about a person; the middle two answer questions
about an address.** Conflating them is the standing hazard here, and the sequence
number is the one most often mistaken for the chain: it corrects routing state and
enforces nothing.

**The correlation between a series and an archive segment is a design decision, not
a consequence.** A series is routing and an archive is evidence; nothing makes a
reissue a checkpoint except that this design elects it, for simplicity and ease of
administration. Saying so matters because the two can be separated later without
disturbing either mechanism — and because a patron countersigns the reissue, so any
rule that made presence evidence *depend* on that signature would put a patron
astride evidence of events it did not observe, which §6.4 forbids. **A checkpoint
bounds what the chain must retain; it does not reach the PoP records themselves.**

### 10.1 History completeness, the self-chained archive

**History portability is the point, and an explicit commitment rather than an
accident.** Transactions are self-signed and countersigned, so **a node's history
remains independently verifiable after it moves** — leaving costs proximity, not
evidence. That is the difference between an exit right and a real one.

**What makes a portable archive worth carrying** is §16.1's intersection rule: an
evaluator weighs the records naming identities it already knows of, and nothing
else. **The chaining below makes a presented history complete and tamper-evident;
it does not make it interesting.** A complete archive of meetings with strangers
tells an evaluator nothing, which is why fabricating one gains nothing.

**Why the archive must be tamper-evident — and which presentation it defends.**
Two presentations exist and they are different animals [author, 2026-09-01]. A
**ceremony bundle** is deliberately curated: the subject cherry-picks presence
records from any series, no chaining, no completeness — and what defends the
selector there is recognition, not the chain (§8.1.2, `wire-format.md` §5.4).
An **archive presentation for standing** — the history a prospective patron
walks at adoption (§16.7) — is the opposite claim: *this range is unbroken*,
and that claim is exactly what the chain makes checkable. Trimming a standing
presentation trims apparent standing in the same stroke, which is what keeps
the freedom to curate from being a freedom to launder.

**The archive is therefore a hash chain.**

Each transaction incorporates **a hash of the subject's previous transaction**,
used as the nonce in a challenge the counterparty signs. **This applies to every
transaction type.** Adoption, departure, disavowal, peering and presence all
advance their signers' archives, and each signer carries their own, so a
two-signer transaction carries a back-pointer set for each. A signer normally
supplies one back-pointer; supplying more is a **merge** (§10.3). Sequence position
therefore becomes as trustworthy as the record itself. A record may be entirely
fabricated, but **its position in the sequence cannot be altered after the fact**,
because altering it would require forging the next record's back-pointer, which
its counterparty already signed.

Consequences:

- **Any presented sequence is provably unbroken across the range shown**, even
  though the archive is built and held by the subject. That range is rooted at
  identity genesis, or at a checkpoint attesting that history existed before it
  (§10.2).
- **Excision is impossible.** Only truncation remains, and it cuts at the ends: an
  earlier head drops what is recent, a checkpoint drops what is early. Nothing
  removes a record from the middle and keeps its neighbours.
- **Activity gaps are visible.** You cannot maintain the appearance of a
  long-active user while stripping incriminating records and keeping their
  contemporaries — **a checkpoint takes everything before it or nothing**, so it
  cannot be aimed at a record.

**Why this largely dissolves the attack.** Truncation is **self-defeating**,
because history is precisely what confers standing. Trimming to lower the
threshold trims apparent standing in exact proportion, and trimming all the way is
economically identical to **starting a fresh identity.** An operation already
permitted, already costing everything, and requiring no attack.

**The guarantee is per-evaluation.** Within any one subnet's view of a subject,
truncation is the only available edit, at either end. Presenting different views to
different subnets is **not** an edit to a history. It is two histories, which
§13.7 explicitly permits: *the archive exists to inform new subnets on joining,
not to hold users accountable across subnets.* Cross-subnet accountability is
disclaimed by design and is not a property this chain is protecting.

**Deliberate multi-device forking is not an attack on this.** The "gain" of carrying
pre-fork standing into two subnets is the archive doing its job, and a reviewing
patron already ignores transactions with counterparties it cannot reach. **The
multi-device problem is *accidental* forking** (§22.2).

**A series reissue is also an archive checkpoint, and this is what makes pruning
possible at all.** Because every record commits to its predecessor, verification walks
backward: without a second root a subject can withhold what they did lately and cannot
drop their early life. A reissue countersigned by your patron (`wire-format.md` §4.6)
supplies that root, asserting that history existed across the boundary without carrying
what it contained, so **the chain before it need not be retained or presented** — the
blocks and the dates survive and the details go.

**What is released is the chain, not the evidence.** Presence records are a separate
kind of history (§10.0): a PoP is a transaction and sits in the archive, but its use —
identifying validators for a later ceremony, and standing as evidence that two people
met — does not depend on the chain's continuity. Pruning the chain therefore does not
license discarding presence records, sealed captures or capture seeds, and
`light-client-requirements.md` §2 requires they be kept.

**Except inside the 730-day window, where they are still required.** Verifier
selection counts *n* and draws the candidate set by traversing what is reachable from
the committed back-pointer (`wire-format.md` §5.4), so a checkpoint that discarded
recent history would shrink both — letting a subject choose its own verification
burden, down to the `n = 1` case that requires **zero** verifiers (§6.4). **Pruning is
therefore permitted only beyond the window**, which costs nothing anyone wants: the
storage saving and the elision of early history are both about records the candidate
set has already aged out.

**The archive's scope across bindings is left unstated**, and no claim is made
that one chain spans them. Seqno series (§10.0) postdate the question, and no reader
is given a way to enumerate a subject's archive uninvited in any case. Where a
presented history names counterparties a reader cannot reach, a reviewing patron
**ignores them** (§16.7): an unfamiliar counterparty is an opaque entry rather than
an intelligible one, contributing length and nothing else.

**Forking identities is expected and fine.** The archive exists to inform a new
subnet on joining, not to hold users accountable across subnets. Key rotation is
potent only within the subnet where it occurs (§13.6); viewed from anywhere else
a rotation simply creates a new user with a new history.

### 10.2 The archive is a second factor

Nobody designed this; it falls out of the chain.

**A thief holding the key but not the archive cannot exploit accumulated
reputation.** They can act in subnets where that key is current — resource access,
subnet-scoped transactions, but they can join *new* subnets only as a completely
fresh user with no history. And doing so carries risk: if any of the genuine
user's history later becomes visible in that subnet, the impostor is unmasked by
the mismatch.

**Key alone is not identity portability. Key plus archive is.** This narrows the
compromise problem of §12.4, but **only for key-only compromise, which is the
weaker premise.** An adversary holding the device holds both, so the property is
defeated by exactly the case it might have been expected to help with: a stolen
phone yields keys, archive, and often the photo store, and the thief can carry the
victim's history anywhere.

**Nothing here separates the two factors.** They live on one device by
construction. A genuinely separate authorisation factor for high-value operations —
non-exportable, held elsewhere — is what would change that adversary's position; it
is undesigned, and would cost ordinary usability, so it is a trade rather than an
oversight.

**The cost:** archive loss is history loss. Presence-based recovery (§9.1)
restores the *key*, not the archive, so a user who loses their device and recovers
their key returns with standing intact in their existing subnets and **nothing
portable to a new one**. This makes §13.7.1's backup requirements
load-bearing. *(Whether recovery should restore history is an open question — §22.3.)*

### 10.3 Merges. The archive is a DAG, not a chain

**Branches can be merged, and merging strengthens completeness.** A transaction
following a fork carries back-pointers to **both** branch heads. That commits to
both: omitting a branch afterwards leaves the merge's back-pointer unsatisfied and
is therefore detectable. Excision within a branch was already impossible; the
merge closes off dropping a whole branch as well.

The structure is properly a **Merkle DAG**, git-shaped, and for the same reason
git is: concurrent authors, no coordination available, reconcile afterwards rather
than prevent divergence.

**What is lost is cross-branch ordering, and it costs nothing.** The archive was
built to prove **no intermediate records are missing**, not to order events. Nothing in
this design consumes a total order, within or across identities. Completeness arguments need reachability;
impossible-travel detection needs comparable timestamps (§7.7); neither needs a
sequence.

**No merge transaction type is required.** The next ordinary transaction carries
two back-pointers instead of one, so the back-pointer field becomes a **list per
signer.** Length one in the common case, longer at a merge.

**The incentive aligns without enforcement.** Merging makes a history more
complete, and completeness is what confers standing, so users want to merge. There
is nothing to compel.

**Asymmetry against a key thief.** If a thief holds one device's branch
and the legitimate user merges theirs, the legitimate history is strictly more
complete, and the thief cannot merge without a branch they do not have. This
sharpens the second-factor property of §10.2 rather than weakening it.

---

## 11. Resources

**A resource is a package running on an infra node, exposing a set of roles, and
accepting that node's credential to access them.** It is the boundary layer
between this network and everything else.

This is the first job the hierarchy has taken on beyond trust evaluation, and it
is what the network is *for* once naming and authentication work: §1 frames the
product as letting people **coordinate** with their social graph rather than
merely communicate with it (V6), and resources are how coordination happens.

**The infra node is the front door; the resource sits behind it.** A reverse proxy
with an authenticating gateway, a conventional shape, which is a good sign: the
novelty stays confined to *how the gateway decides*, which is where it belongs.

**The resource never reads network state.** The node evaluates access and presents
the result as a credential; the resource sees an authenticated principal holding
roles, exactly as a SaaS product sees a user arriving through a corporate gateway.
**A compromised resource therefore leaks its own data, not the owner's archive.**

Three categories, one mechanism:

| Category | Example |
|---|---|
| **Local application** | Team datastore or tracker, using the node's own storage and compute |
| **Gateway** | Homeserver bridging to another distributed system, see §11.7 |
| **External service** | Commercial SaaS. **The vendor need not join the network**, or know it exists |

Implementation requirements — package format, sandboxing, supply chain, the
role-binding interface — are in `infra-client-requirements.md`. Conformance
requirements for a resource are in `resource-requirements.md`.

### 11.0 What the boundary is, and what it is not

Four properties of the resource boundary that the rest of this chapter assumes.

#### 11.0.1 Wider reach is federation, not wider scope

**A resource is neighbourhood-scale; a resource application can be any scale.** A
**resource application** is the wider system a resource is one instance of — the
network hosts the instance and knows nothing of the system. §11.2 bounds a resource
to its owner's Dunbar Org, and that is not a ceiling on what can be built: a
subnet-wide or cross-subnet service is **many local instances**, each hosted by a
patron, each administering its own team, with the resource application handling
instance-to-instance connection in its own architecture.

**The network never expresses wide-scale access control because it never sees a
wide-scale audience.**

Three properties follow, none of which was designed for:

- **Every user's access is administered by someone they have a real relationship
  with.** No distant operator decides whether you may use the subnet-wide service;
  your patron does. §1's freedom argument appears here as an **architectural
  property** rather than a claim — local accountable intermediaries are structurally
  required, not merely hoped for.
- **The gate holds at every hop.** A wider system cannot reach past a patron to
  that patron's members, and a patron may deny access the wider system would
  grant.
- **It is the same shape as a gateway** (§11.7) at a different scale. One
  mechanism, not two.

**The cost: every patron becomes an operator.** Holding a connection to the wider
system, hosting an instance, administering access, carrying availability. That
lands on exactly the people §3.3 already asks to run infrastructure, and whether
they will do it is **assumption A21** (§20.2).

#### 11.0.2 Principals are identified pairwise

**An identifier disclosed to a relying party must be derived so that identifiers
for the same subject at different parties cannot be correlated, while remaining
stable at each.** Present encoding: a resource sees a per-user identifier derived
from its own identity and the subject's, never the subject's keyhash.

```
principal_id = SHA-256("rhtn/1:pairwise" || resource_keyhash || user_keyhash)
```

**Per-user functionality is unaffected.** The identifier is deterministic and
stable, which is the property ownership, history and preferences depend on — the
same property a raw keyhash would have provided, which matters, because most applications distinguish users who hold
identical roles.

**Two *independent* vendors cannot compare notes.** Different resources derive
different identifiers for the same person, so an external service learns an
identifier meaningful only to itself.

**One vendor running several resources still can**, using ordinary account, device
and network data it holds anyway, the identifier scheme is not what stands between
them (C15). Pairwise derivation addresses cross-*operator* linkage, not
cross-*service* linkage within one operator.

**That is accepted rather than unaddressed** (§19.7 item 11). Resources differ in
what anonymity they offer, and which ones a subnet admits is part of how it sets its
security posture; a user may decline a service their organisation accepts. Handing out the network keyhash instead would let any
two services discover they share a user, which is **P3's cross-context linkage
arriving through a door the network opened** rather than one it merely failed to
close.

**A hosted operator loses nothing.** They know the resource identity and their own
org's keyhashes, so they can compute the mapping and invert it whenever they want
— one hash per member. The scheme withholds nothing from a party who already knows
you; it withholds the network identity from parties who do not.

**Keyed on the resource, so moving a resource between hosts changes nothing.**
Identifiers survive a package migrating from local hosting to a broker or back.
Keying on the hosting model instead would have made every user look new on the day
a resource moved.

**Per resource, not per owner.** An owner running three services gets three
identifier spaces, and may correlate across their own by computing the mapping. The
finer grain is the default because the coarser one cannot be recovered from it.

#### 11.0.3 A resource asserts nothing into the trust graph

**A resource can report to its host, and can message nodes peer-to-peer where
configured to. That is all.** It cannot emit anything the trust metric
consumes, no completion, no attestation, no reputation signal.

**The reason is that the alternative is unbounded.** Letting resources assert into
the trust graph would require **global message types for arbitrary service needs**,
so the protocol would have to grow a definition for every semantic any application
ever wanted. A fixed protocol cannot absorb an open-ended ecosystem's vocabulary.

**It is also the line §11.3 already draws.** *Bears its own costs → node; borrows
authority → resource.* A resource emitting trust signals would be **borrowing
authority in order to create trust**, which is the combination excluded everywhere
else in this design.

**Peer-to-peer messaging needs nothing new.** §14.2 already treats a resource as a
possible payload endpoint, and `CatalogEntry` (§11.5) exists to make one
addressable so it can initiate contact. Such traffic is payload — opaque to the
network, carrying no protocol meaning.

**Consequence: the request path carries no trust**, so how a request is framed is a
plumbing decision rather than a design one (§11.7).

#### 11.0.4 Credentials do not cross the boundary twice

**The node-resource interface is an impenetrable trust boundary, and credentials
never cross it twice.** A credential is **audience-bound.** It names one
resource, and any other MUST reject it. **A resource never passes a user's
credential to another resource.**

Resource-to-resource trust is **configured, not delegated**: if one resource needs
another, that is a relationship between them, carrying the first resource's own
authority rather than a borrowed user's. This removes the confused-deputy problem
by construction, a resource cannot be induced to wield a user's authority
elsewhere, because it never holds any.

**What this forecloses is accepted:** a user cannot authorise one resource to act
on their behalf at another. Such composition is **operator-configured**, so it
happens at organisational rather than individual scale — natural for team tools
(*"our tracker may post to our chat"*), and simply unavailable for user-scoped
delegation. The alternative is delegation with guards, which is what OAuth is, and
most of OAuth's complexity and vulnerability history is the cost of getting those
guards right.

### 11.1 Why this is where enforcement works

§1.1 establishes that the protocol can only compel where shared state exists, and
almost everywhere it does not — hence visibility in place of enforcement. **A
resource is the exception.** The owner and a requester inside the owner's subtree
share topology state, and the owner is *running the service*. So access control
here is genuinely enforceable rather than merely observable: a resource can simply
decline.

This is a proper application of §1.1's diagnostic, not another workaround.

### 11.2 Structural properties

**A participant may hold access only while it remains within the region the
owner's policy can evaluate.** That gate sits behind every scope and grant, and
**nothing reaches outside it**. Present encoding: membership in the owner's Dunbar
Org, which is the region a node holds topology for (§15.1). Consequences:

- **Departure revokes everything the node controls, uniformly.** No category of
  access survives leaving, but see the establishment/continuation distinction
  below, which bounds what "everything" means.
- **No access accumulates.** A grant to a named individual cannot outlive their
  membership, which is what would otherwise happen — positional access lapsing
  while named access persisted, backwards from what anyone expects.
- **It is enforceable**, evaluated from topology the node already holds.
- **Resources are neighbourhood-scale by construction.** At most the owner's
  two-edge neighbourhood, consistent with §11.4's vocabulary topping out at `dunbar`.

#### 11.2.1 Membership is necessary, not sufficient, above the patron

**A new subordinate reaches its patron's resources on adoption. It reaches
resources higher in the tree only once the grandpatron countersigns.** The countersignature is carried as a `SubtreeAck` (`wire-format.md` §7.5).

Adoption puts a node inside its grandpatron's Dunbar Org, so the §11.2 gate opens
automatically, and a patron can therefore admit arbitrary strangers to their own
superior's resources without that superior agreeing. **The countersignature is the
grandpatron's own decision about who they let in**, and it is enforceable for the
ordinary reason: the grandpatron's node evaluates access to the resources it hosts,
and shares with the requester exactly the state that decision turns on.

**The decision is a policy, not a prompt.** The grandpatron's node
issues the acknowledgement automatically under a rule its operator set in advance;
the deliberate human act in this sequence was the *patron's* adoption, and
membership propagates within the horizon as a consequence of it. Nobody is
interrupted per arrival, and an operator wanting to acknowledge nobody, or only
certain positions, writes that once.

**It does not gate the adoption.** The adoption is complete and valid with the node
and patron signatures (`wire-format.md` §4.1); it is signed and propagated whether
or not the grandpatron acknowledges it. What waits is access to *their* resources,
and nothing else. So a grandpatron whose node is offline, or whose standing policy
does not reach this position, delays a convenience rather than blocking a
membership.

##### One signature, several acceptors

**Nodes that already trust the grandpatron may accept that `SubtreeAck` instead of
evaluating the new node themselves.** In practice the grandpatron's siblings
and the great-grandpatron, who hold the new node in their horizon but have no
relationship with it.

**The default on receiving one is to allocate roles as to any subordinate in the
same network position.** The new node is treated as its position implies —
nothing special, nothing withheld, so an acknowledged node gets what a node at
that place in the tree gets.

**Two limits on that default, and both matter.**

**It reaches positional grants only.** A resource whose roles are bound to named
individuals (§11.4) is untouched: those were decisions about particular people, and
a node arriving in a position was never one of them. So the default admits a new
member to the team datastore and not to whatever three people were explicitly
listed for.

**It is a default, not an obligation.** The grandpatron cannot bind a sibling's
policy any more than anyone binds anyone's here (§1.1). What the countersignature
offers is **a party they already evaluate, vouching for a party they do not**,
which is cheaper than each forming an independent view of a stranger and is the
same substitution the trust model rests on throughout. An operator wanting a
stricter rule sets one.

##### What it attests, and what it does not

The grandpatron is **not** vouching for identity, the patron's signature on the
adoption does that, backed by whatever presence evidence it carries. The
countersignature says only: *I acknowledge this node as a member of my subtree and
will treat it as one.*

That distinction matters when a grandpatron later disavows the patron. The
countersignature was about the subtree, so it lapses with the relationship it
described, exactly as any positional grant does.

**Membership gates session establishment. Continuation is the resource's
business.**

Where the node hosts the package, it terminates the session and revocation is
immediate. Where the resource is an external service, the node authenticates and
steps out, so access persists on that service's terms — **which is what choosing
that service means.** A team signed up to a third-party product gets that product,
including its session behaviour. The network's job is the bridge: administering who
may reach it, by the teaming logic defined here.

**The reference infra client drops hosted sessions when a principal's roles
change**, so the access ends rather than persisting to the next reconnection
(see `infra-client-requirements.md`). Local behaviour rather than a protocol rule
— the network cannot reach into an operator's node, but it is what makes the
membership gate mean what §11.2 says it means, for the resources where anything
here can.

An operator needing prompt revocation should therefore prefer hosted packages. The
infra client shows which a resource is, since that follows from where it runs.

#### When an owner moves

**The general rule: there is no grant object for a move to invalidate.** Access
is **a row the node re-derives**, never a durable grant recorded at the moment of
authorisation (`resource-requirements.md` §7.1). A change of the owner's position
changes what the predicates match, so the rows change with it — and the next request
is answered from whatever the row says then. **The request itself is still a lookup**
(§11.4): re-derivation happens when the topology changes, not while a caller waits.
Nothing propagates, nothing is revoked, and no reconciliation runs.

**Three consequences, all of them absences:**

- **Access changes without anyone acting.** A node joining the subtree gains access;
  one departing loses it; one crossing a tenure boundary gains it silently. None of
  it produces an event anyone sees.
- **No stale-grant accumulation.** Individual grants are not exceptions to positional
  access. A departed node cannot hold named roles while its predicate access lapses.
- **In-flight state follows what the network can reach.** For a hosted package the
  node terminates the session, so access ends at once. For a brokered external
  service, membership gates *establishment* only — an existing session continues on
  that service's terms, which is what choosing that service means. §1.1 is why: the
  network can stop new establishment and cannot reach into a session running
  elsewhere.

**Permissions follow the owner's new position**, since predicates are relative to
the owner (§11.4):

| Who | Effect |
|---|---|
| **The old upline and lateral org.** Former patron and its generation, former siblings, and their subordinates | **Lose access.** They are no longer within the owner's Dunbar Org |
| **The owner's down-line** | **Unaffected.** Subordinates travel with the owner |
| **The new upline and lateral org** | **Gain access**, wherever a predicate matches |

**The third row follows from the operator's own policy.** Accepting a new patron
is a deliberate act, and an upline gains access only where the operator wrote a
predicate that reaches upward. **An operator who does not want their upline seeing
their resources simply does not write upline predicates** — `down(n)` and explicit
lists never match an upline, wherever the owner moves. The client should confirm
the consequence at the point of a move, as a reminder rather than a warning.

**Hosting is the larger consequence.** A light-client owner's resource runs on its
serving infra node (below), so a move that changes the serving node **means the
resource must migrate or become unreachable.** Package, state and data. An infra
owner hosts its own resources and takes them along.

That is a real asymmetry, and an argument for a resource owner to run
infrastructure rather than rely on a patron's.

**Ownership and hosting are separable.** A light client cannot host a service, so
its resource runs on its **serving infra node.** Whose operator may decline to
host it and sees its traffic regardless. Where a resource *executes* is a
deployment fact, not a property of ownership.

- **A resource has an identity but is not a node.** It needs a keypair to sign, but
  **must not consume its owner's f=10 subordinate slots.** The same rule already
  written for agent grants, now general.
- **Addressed relative to its owner**: the owner's locator plus a resource
  identifier. Resources therefore never enter the anchor table (§12.2) and do not
  affect its sizing.
- **A resource can never accumulate social trust directly**, because it cannot
  attend a presence ceremony (§7). Its standing is borrowed from its owner,
  permanently. The agent design had to *assert* this property; here it is
  structural.
- **Permission follows position, so departure revokes access automatically.**
  Elegant, and a hazard: a node that departs or is disavowed silently loses access
  to team resources. **The reference client must warn before a departure that would
  cut the user off from resources they use.**
- **Currency**: a resource's key currency derives from its owner's attestation
  (§12.6.5); it has no independent claim.

### 11.3 This resolves the agent question

Delegated and autonomous agents need no separate treatment. The resource object
supplies one structural line:

> **If it borrows authority, it is a resource. If it bears its own costs, it is a
> node.**

A delegated agent is **a resource with permission to act with respect to users and
other resources.** Owned by a principal, scoped to a region, acting on borrowed
permission, unable to accumulate standing of its own. That is precisely what
"borrows scarcity from a principal" was reaching for, now expressed as a thing the
network has rather than an exception to what it has.

An autonomous participant that pays for its own infrastructure and attends
ceremonies is a node like anyone else, with no special case required.

### 11.4 Permission scopes

**Access is expressed as a region relative to the owner.** A requester's
eligibility is then computable locally from topology the evaluator already holds.

**The horizon bounds the vocabulary, and does so naturally.** A node can only
compute its position relative to an owner within its horizon (§15.1). Beyond that,
relative position is unknown, so **scope-based permission does not extend past the
horizon** and distant access requires an explicit grant. This is a consequence of
the architecture rather than a rule added to it.

```
Scope =
    self                  ; the owner alone
  | down(n)               ; owner's subordinates to depth n
  | up(n)                 ; owner's patron chain to height n
  | siblings              ; the owner's siblings
  | dunbar                ; the owner's whole Dunbar Org (§15.1). A primitive,
                          ;   NOT a subtree — no subtree has this shape
  | list([keyhash])       ; named members, inside the Dunbar Org like every
                          ;   other scope (§11.2)
```

`dunbar` is called out because it is expected to be the common case: a team
resource available to the team (§1, V1).

**No scope reaches outside the owner's Dunbar Org**, `list` included. It escapes
the *positional* predicates, not the membership gate — the owner's node cannot
evaluate a requester whose topology it does not hold, so a named grant to a
stranger would be unenforceable rather than permissive (§11.2).

**Actions are resource-defined, with two reserved.** The protocol does not
enumerate what a resource can do. That is the application's business (§15). Two
are reserved because the catalog needs them:

- `discover` — the resource appears in the requester's catalog view
- `connect` — the requester may open a session with it

**Neither is ever forwarded to the resource.** Both are consumed by
the owner's node when it evaluates its role table, and a resource's role set carries
**application actions only**.

**They cannot be acted on downstream, which is why.** `discover` is evaluated when
composing a catalog answer, before any request exists — a resource is never present
for that decision. And **a request arriving at the resource is what a `connect` grant
looks like**; restating it in the role set would tell the resource something the
delivery already told it.

Everything beyond those is opaque to the network. A resource may define `read`,
`write`, `admin` or anything else; the protocol carries the strings and does not
interpret them.

**A permission is a (scope, action-set) pair**, and a resource's policy is a list
of them. Policy resides at the owner's node (§15.1), which is the party that can
genuinely enforce.

**Roles are declared by the package, bound by the operator.** The package manifest
says what roles exist; the operator binds predicates to them. A package cannot
invent a role after installation, and an operator cannot grant one the package does
not understand.

**Role assignment is a materialised table, and predicates are a macro over it.**
The node holds, per resource, a row for each member of the owner's
Dunbar Org and the roles that member has. **Authorisation at request time is a
lookup**, not an evaluation.

**The table is bounded, so materialising it costs nothing.** A Dunbar Org is 221
nodes at f = 10 (§15.1); one row each, per resource.

**Predicates are evaluated twice, and neither is on the request path:**

1. **When an operator configures roles**, interactively — the predicate expands to
   a set of assignments the operator can see and adjust.
2. **When a node enters or leaves the horizon**, in the background — a new member
   is scored against the standing predicates and given rows; a departing one has
   theirs removed.

**Nothing recalculates on request.** A resource request consults the table as it
stands, which makes authorisation deterministic, fast, and inspectable — an
operator can read who has what, rather than deriving it.

**So the predicate language needs no interoperable specification**, and neither does
the table. **The network never sees a role.** It is internal to the owner's node
throughout:

- **The node** holds the table and consults it.
- **The resource** receives the roles over the hosting path, which is inside the
  owner's own machine or a connection the owner controls (§11.0.1).
- **The user** sees their role only as the resource's own interface reflects it —
  as a menu, a permission, an error. Never as protocol.
- **The network** sees which resources appear in a user's catalog, and that a
  connection was made. Not what it authorises.

**What does travel is `Scope`** (§11.4 above), in a `CatalogEntry`'s
`connect_scope`, which is advisory and tells an asker whether a connection is
likely to be accepted. That
is a closed vocabulary for exactly that reason, and it is a different thing from a
role.

**One interface does need stability, and it is not a network one.** The
node-to-resource contract — principal, roles, audience, session — must be **legible
to any implementation of either client type**, so a package reads the same
credential wherever it runs. **That is portability, not interoperability**: it binds
a node to the packages it hosts, never one node to another.

**Legible everywhere is not runnable everywhere.** A package may need storage,
compute, a GPU, a persistent address, or a device capability the requesting client
lacks — and a node or a user device that cannot meet those requirements **cannot
host or use it, while remaining fully conforming.** Two different failures:

| | |
|---|---|
| **The contract is not understood** | A conformance defect. Must not happen on a conforming node |
| **The requirements are not met** | Ordinary capacity. Expected, and says nothing about either party's conformance |

**A package should declare what it needs** so the second failure is visible before
installation rather than at first request. Nothing in the protocol enforces that;
it is packaging hygiene, and `resource-requirements.md` §8's manifest is where it
belongs.

**Binding is by predicate over data the node already holds.** Topology and archive
within its horizon, so the predicate language needs no new state. It is a query
language over existing data, which bounds what it can express, and for a
security-critical component that is a feature. Named individuals are also bindable,
inside the membership gate (§11.2).

**Trust thresholds are expressed as rank or percentile, never as a raw score.**
A raw threshold is denominated in units meaningful only within one metric
family (§16.1), so switching families silently changes who has access, while the
operator's intent was to change the metric, not the access. Every metric produces an
ordering, so *"top 20% of my Dunbar org"* survives a change that *"score > 0.6"*
does not.

*(Tuning parameters **within** a metric and having access follow is the feature,
not a hazard: the operator changed it because they wanted that effect.)*

### 11.5 Service catalog

Prior art is **DNS-SD (RFC 6763)**, commonly paired with mDNS
(RFC 6762) as "Bonjour" — it maps almost field
for field.

```
CatalogEntry = {
  resource        : keyhash        ; the resource's own identity
  owner           : keyhash
  type            : tstr           ; service type, DNS-SD style
  instance        : tstr           ; human-readable instance name
  connection      : opaque         ; how to reach it; resource-defined
  discover_scope  : Scope          ; NOT PART OF THE ENTRY. The answering node's
                                   ;   rule for which askers it returns this entry
                                   ;   to — requested at registration, never sent
                                   ;   to an asker (§11.5)
  connect_scope   : Scope          ; who may open a session
  data_practice   : ? uint         ; declared logging and retention posture.
                                   ;   OPTIONAL; absent means undeclared, which
                                   ;   is itself informative. Values enumerated
                                   ;   in `wire-format.md` §6.1
  signature       : COSE_Sign1     ; by the OWNER, not the resource
}
```

**Signed by the owner**, because the owner is the party staking standing on the
resource's existence and behaviour.

**`data_practice` is a declaration, not a control**. The protocol
says nothing about what a resource may log — that is the resource's business (§11.0.3)
— which left a user with no way to weigh one resource against another and a deployment
hosting sensitive resources with no guidance (P20). §1.1's answer where enforcement is
unavailable is to make the distinction visible and let policy weight it, which is the
same move as client-integrity attributes (§7.8). Because the field sits inside the
owner's signature, a declaration is a claim the owner staked standing on.

**It arrives before any connection, because the catalog lookup is how a user learns
the resource exists at all.** An entry is served on request to askers the owner's
`discover_scope` admits, and a user must query the catalog to know there is anything
there to address — so the declared posture is in hand at the moment the decision to
connect is made, not after it. `discover_scope` gates the answer; `connect_scope`
gates the session; they are different gates and the first one comes first.

**Its reach is bounded by the catalog's**, which costs nothing: a user the owner does
not admit never sees the entry, and also cannot use the resource, so there is no
evaluation they were denied. It is not a discovery surface across organisations and
does not help anyone deciding whether to *join* one.

**A `CatalogEntry` is optional.** It is a network-layer *registration* that makes a
resource addressable — reachable point-to-point, or able to call out to nodes. A
locally-hosted application may need none, since it is reached by asking the node
directly. Publishing one is a deployment decision, not a property of being a
resource.

**The catalog page is a different object.** Served by the infra node over an
authenticated session, not signed, not propagated, and **personalised: it names the
roles the viewer holds.** For locally-hosted resources it is also the front door to
interaction. A node does not list what the viewer cannot use, which makes
`discover_scope` filtering redundant for the catalog page — but the field remains
load-bearing for the query path below, where the hosting node composes an answer from
entries it holds.

**Role names are visible strings.** A role called `clinical-records-write` says
something about the service and about whoever holds it — P15's shape one level
down, where the resource set fingerprints the owner and the role set fingerprints
the user within it. Not a reason to hide roles; a reason to choose names knowing
they are public to everyone who can see the resource.

**Answered on request, not propagated.** A node asks the infra nodes
within its horizon what they have; each returns the entries it owns and the asker
may see. **A node's catalog view is the union of those answers** — assembled by the
asker at query time rather than replicated toward it.

**"Services within range" therefore means "within horizon"**, and it means so for a
simple reason: those are the nodes you can ask. Composability is free, since the
union is computed where it is used.

**The hosting node answers, and the owner signs.** A light client
cannot answer queries — it is not always reachable and holds no static address — so
a resource owned by a light client is **registered with, and answered by, the infra
node hosting it** (§11.0.1). The entry still carries the owner's signature and the
owner's keyhash: **custody and authorship are separate**, which is the same split
the whole hosting model rests on.

**Consequence worth stating.** The hosting node decides the `discover_scope`
filtering in practice, because it composes the answer — so an owner delegating
hosting delegates that filtering too, and should be told so. The owner **requests** a
scope when it registers and cannot check that the request was honoured, which is the
ordinary shape here: the party that acts is the party that knows.

##### When a hosting node goes down

**Loss of a host is not a silent failure.** There is no registry outside the host
and the asker's cache, so a client that cannot reach a node **knows that node's
contribution to its catalog view is stale** — the same signal it uses for any
outage, arriving at the moment it matters.

**Whether to provide failover is a per-resource question with three different
answers**, and the protocol specifies none of them:

**Local services disappear with their owner's node, and should.** A resource whose
whole point is that it runs here has no meaningful existence elsewhere. Replicating
to siblings is possible and buys little for the complexity.

**Network-native services will be designed for multiple ingress points.** A resource
used widely across a large subnet **needs an owner every few tiers along each branch
anyway**, or it falls outside some members' horizons — reach and redundancy are the
same problem. Placing one every second tier rather than every fourth gives most
members an alternate path as a side effect. **How densely is a deployment decision**,
driven by a resource's actual use profile, and nothing here should fix it.

**Gateway-fronted external services will mostly not do that**, since a second
gateway may carry a marginal licence cost or the application may assume one instance.
**That is the position a corporate intranet deployment of the same product is
already in**, times the reliability of whoever hosts the gateway — no worse, and
familiar to anyone choosing to deploy that way.

**A client caches its view and refreshes it deliberately.** The
sweep runs on joining a subnet, periodically, after a failed connection, and when
the user asks. **Between sweeps the view is stale and that is fine.**

**Staleness costs little here because of who is on the network.** Subnet members
are people who have met (§7), so a new resource arrives with a conversation
attached — someone mentions it. **A user going to look for the thing Bob added is
expected behaviour**, not a convergence failure, and a refresh control is a better
answer than a protocol that keeps hundreds of views in sync for an event that is
socially announced anyway.

**This also bounds the session cost.** A sweep holds one session at a time and
closes it; browsing opens nothing (`wire-format.md` §11.1).

**The catalog is answered, not published.** A node asks an infra
node within its horizon what it has; the infra node returns the entries it **owns**
and that the asker may see. **There is no propagation, no relay and no cached
authority** — so the enforcement question does not arise at all. **The owner is the only party that ever discloses an entry**, and filtering
at the source is not a rule imposed on anyone else; it is the only thing that
happens.

**`discover_scope` is therefore local to whoever answers.** It decides which entries
that node returns to which asker, is evaluated where the answer is composed, and
**never reaches an asker** — the same shape as the role table (§11.4). An owner
requests one when it registers and cannot check that the request was honoured; an
asker does not check it either, because **receiving an entry is what qualifying
looks like.**

**Browsing shows what you can plausibly use rather than everything that exists**,
which is a property of who answers rather than of who forwards. This is capability-style cataloguing: §15's access
control becomes legible at discovery time rather than only at request time.

**Instructive failure to avoid: UDDI**, the SOAP-era universal registry, which
died of centralisation and of describing services nobody wanted to find. A catalog
is useful at neighbourhood scale and worthless at global scale, which is the
scoping this design already has for other reasons (§12.4).

### 11.6 Abuse reporting

**Abuse detection and response belong to the resource**, which
will typically be a full application with its own notion of misuse. The protocol
contributes exactly one thing: a way to tell the owner.

**The resource reports; the owner receives.** The owner may be the
host or the gateway to the host, and in either case **it is the owner that holds the
interface between the network and the resource**, so the owner's node is where such a
message is both created and consumed. A resource cannot emit one itself — it asserts
nothing into the trust graph (§11.0.3) and reaches the network only through the
gateway.

**Which makes a report closer to a log entry than to a message**, and that is the
right way to read it. It is defined as a transaction because it is signed, portable
and durable, not because it traverses the network in the ordinary case.

```
AbuseReport = {
  resource     : keyhash           ; and the signer; there is no separate reporter
  occurred_at  : timestamp
  category     : uint              ; small enumeration, below
  detail       : ? opaque          ; resource-defined, uninterpreted
  signature    : COSE_Sign1        ; by the resource. Its keyhash MUST equal
                                   ;   `resource`
}
```

Categories are deliberately few and structural rather than judgemental:
`0 unavailable`, `1 malfunction`, `2 excessive-load`, `3 unauthorised-access-attempt`,
`4 content` (resource-defined meaning), `5 other`.

**The protocol defines no response.** What happens next is determined by the
policy object at the owner's node (§15.1).

**There is no separate reporter field.** It named the same party as
`resource`, and the rule that keeps the object honest is that **the signing key's
keyhash MUST equal `resource`** — checkable from the object alone, so a genuine MUST
under §1.1. Without it, an object could attribute a complaint to a party that did not
make one.

**An application wanting to name which of its own users complained puts that in
`detail`, as application data.** There is no user-signed abuse report, because
producing one would require a user's network client to interoperate with arbitrary
third-party applications — a blurring of the network and application layers that the
resource boundary exists to prevent (§11.0.3). The network's contribution stays what
§11.6 says it is: a way for a resource to tell its owner.

**Enforceable at the sender:** *a party submitting a private complaint for action
by a responsible decision-maker MUST address it only to that decision-maker, and
MUST NOT broadcast it as reputation evidence.* The sender controls this completely. Nothing is being asked of
anyone else. Present encoding: the report is addressed to the resource owner, and the
signer is the resource itself.

**Not enforceable, and not attempted:** what an unrelated party does with a copy
it obtains anyway. The owner can hand the signed object to anyone, and no shared
state lets any rule govern a foreign implementation's storage or trust
calculation. §7.4.2 already states this limit for biometric disclosure — *a holder
who simply tells someone what they know is beyond any protocol rule* — and it
applies identically here.

**The visible distinction that substitutes for it:** the report names the
**resource**, hence its owner, so any recipient can determine *"I am not the
authority for this complaint."* An improperly obtained copy is therefore
recognisable as such, and treating it as reputation evidence is a visible choice
by that evaluator rather than something the protocol appears to sanction.

Because the report is never broadcast by a conforming sender, it creates no public
accusation, which keeps it clear of §6.2.2's reasons for having no peer-to-peer
negative attestation.

### 11.7 Gateways re-concentrate what the architecture disaggregates

A gateway relaying to another network **sees that traffic**. Its operator is
socially trusted, a neighbour, a subnet member, but **socially trusted is not
accountable**, and they become the obvious target for anyone wanting that subnet's
external activity. The design spends considerable effort ensuring no patron sees
content (§14.2); a gateway hands a subnet member exactly that, one layer up.

**Broker rather than proxy by default.** A gateway that only authenticates and
hands off leaves the user connecting directly to the external service; one that
carries the traffic becomes a content chokepoint. Proxying also breaks systems that
bind sessions to client credentials or fingerprints — under proxying every user of
a subnet arrives from one address, which reads as exactly the pattern anti-fraud
systems exist to flag, **and gets worse the more successful the subnet is**.

**§11's permission model runs owner-to-user throughout**, and the user's protection is
what the signed `CatalogEntry` carries (§11.5): that **the service they reach is the one
the trusted owner published**, and the owner's declared `data_practice`. Both arrive at
catalog lookup, before any connection, so the user evaluates rather than discovers
afterwards. That is an identity binding plus a staked claim, and it is what the network
owes them.

**Beyond it they are where an employee is with their employer's SaaS vendors.** The
gateway operator sees their external traffic, and the remedy is not to use the
resource. **A user who does not trust a resource in their subnet is free to avoid
it**, and asking the protocol for more would be asking it to adjudicate a vendor
relationship it is not party to (§1.1).

---

## 12. Addressing and resolution

### 12.1 Locator format
A locator is four fields, **signed by the node itself**:

```
{ anchor, path, sequence, signature }
```

- **anchor.** Key hash of the anchor whose subtree contains the node — a
  root names itself, with an empty path
- **path.** Position beneath the anchor; **truncatable** to a prefix
  sufficient to route to the right region (this is where aggregation savings
  come from). Truncation lives in distant nodes' unsigned aggregate state only —
  **a signed locator always carries the complete path**, since the signature
  covers it (`wire-format.md` §2.1)
- **sequence.** The `{series, counter}` pair from §6.2; the counter detects stale
  cache entries within a series, and series do not rank against each other
- **signature.** Non-optional. **Any routing information presented as
  specifying where a participant can be reached must be authenticated by that
  participant**, so an intermediary cannot substitute itself as the destination.
  Without it, any relay can silently become that node's mailbox.

Identity (public key) is permanent; locator is mutable. Standard
identity/locator split (see LISP and HIP for well-mapped potholes).

**A locator discloses the node's patron, depth and subtree to anyone it introduces
itself to, and this is disclosure rather than leakage.** An ancestor line is what a
participant *is* to that subtree — the identity being presented, not something
escaping alongside it. Introducing yourself from a different subtree introduces a
different conceptual person, and the design disclaims accountability across the two
(§13.7). §19.7 item 2 states the position and its two residuals: correlation
across subnets while a participant holds a single identity, and the reconnaissance
value of a complete path to someone sizing independent edges.

### 12.2 Anchor set
**Anchors are defined by subtree size, not by tier.**

Tier-based definition fails because depth is measured from the root, is not
locally knowable, and shifts under merge — when a root joins a larger tree,
every node beneath it drops a tier and every cached locator referencing the old
anchors breaks at once. Since merging is meant to be routine, that is
disqualifying.

Subtree size is **invariant under merge**: acquiring a patron does not change
your subtree.

- **S ≈ 500,000 subordinates is a guideline, not a status boundary.** Anchor
  status is not a protocol property a node possesses; it is relative to whoever
  is resolving. **Any ancestor may serve as a node's anchor**, and a node picks
  the one appropriate to its recipient, a nearer ancestor for a local contact, a
  higher one for a distant party — exactly as a postal address is given at the
  precision the sender needs (§3.1.1). S estimates which ancestors are likely to
  be *widely cached*; which are *actually* cached is per-node policy (§12.7.3).
  Being patronless is necessary but not sufficient for the widely-cached case.
- **Hysteresis:** promote at S, demote at S/2, so boundary nodes don't flap and
  force cache churn.
- Anchor count ≈ N/S. **Stress test** (not a design target): at a deliberately
  provocative 60 billion users — 10B human, 50B AI. The count is ~120,000,
  which at ~60 bytes an entry is ~7.2 MB. The figure exists to prove the mechanism does
  not break at far beyond any plausible population, not to describe an expected deployment. Real early
  networks will be thousands to tens of thousands of nodes, where see §12.7.3.
- **Entry format (key hash, not key):** 32B hash + 16B address + 4B subtree size +
  8B sequence ≈ 60 bytes for the index fields, **plus the anchor's own signature**
  (`wire-format.md` §7.2), which the sizing below omits. 120,000 × 60B ≈ **7.2 MB**
  of index, inside the 25 MB budget with room for signatures. **Full keys are
  fetched at contact time** — the table is an *index*, not a credential store, which
  is also why an entry's signature cannot be checked on receipt
  (`wire-format.md` §7.2).
- Nodes replicate anchor entries **according to local caching policy** (§12.7.3),
  not universally. In practice widely-cached anchors converge across nodes with
  similar policies, but **no node is guaranteed to hold any particular anchor**
  and there is no global addressability promise (§12.4).

### 12.3 Resolution sequence

**Case 0 — First contact (dominant case).** The locator travels with the key,
out of band: QR code at a meeting, a referral from a mutual contact. No lookup
occurs. Bob pins Alice's full public key on first contact and uses the hash
thereafter.

**Case 1 — Cached locator, still valid.**
1. Bob verifies Alice's signature over `{key, anchor, path, sequence}`.
2. Bob looks up `anchor` in his local table for an address to start from.
3. Bob queries that node. It answers with Alice's serving infra node, or **refers
   him onward** with the next hop's identity and endpoints (§12.6.1). He repeats
   until he has the serving node, caching what he learns.
4. **Bob then contacts that serving node directly.** The chain he walked to find
   it carries nothing afterwards — infra nodes do not relay payload except for
   their own attached clients (§12.6.3).
5. The serving node delivers, or holds for Alice if she is a light client
   currently offline (§14.1.6).

**Case 2 — Cached locator stale (Alice moved).**
6. **Resolution fails.** Nothing redirects on Alice's behalf, and no party holds a
   pointer to where she went.
7. Bob's recourse is Case 3 — re-resolve from a higher ancestor Alice can name — or
   re-establish socially (§12.4). **Inside Alice's horizon this does not arise**:
   her departure and adoption propagate as topology (§15), so her neighbourhood
   already holds her new position. The cost falls on contacts outside it, which is
   where the design puts discovery anyway.

**Case 3 — Alice's anchor is no longer usable to Bob.**
9. Either the anchor's subtree shrank below Bob's caching threshold, or Bob's
   policy changed, so Bob no longer holds it. He re-resolves using a higher
   ancestor Alice can name. **Merging does not cause this.** Anchor-relative
   paths are merge-stable (§12.6.2), which is exactly why §12.2 abandoned the
   tier-based definition.

**Case 4 — Public key only, no locator, no referral.**
**Not resolvable.** See below.

### 12.4 There is no cold lookup — deliberately

A global key→locator index is a DHT, which is rejected (Appendix B.1). Nor can it be
replicated: Bloom summaries of anchor subtrees run ~625 KB per anchor, or ~12.5
GB globally at target scale, three orders of magnitude over budget.

So keys do not circulate without provenance. You reach people you have a
referral or a meeting for. **You cannot search for a person.** Discovery is
social, which is the intended behaviour, but it should be stated as a product
property because it is a visible difference from DNS.

**One exception, and it is directional**. §15.2's rootward memo
lets an ancestor accumulate a key→position index for its own subtree, so a node
**can** look up any descendant, and a subnet's root can look up any member. The
rejection above was argued on *global* grounds — the DHT, and 12.5 GB of Bloom
summaries — and neither applies to a subtree-bounded table.

**What changes is access, not content.** §12.1 already has a locator disclosing your
patron, depth and subtree to anyone you introduce yourself to; the index does not
add a field. It removes the requirement of an introduction, for ancestors only.

**Stated as the product property it now is:** *you cannot search for a person, except
downward within your own subtree.* Joining a subnet is a choice to be structurally
visible to it, and the boundary that matters — that this never crosses into another
subnet — holds by construction (§3.1.1, §15.2).

### 12.5 Caching
- **Anchor table.** Cached per local policy and updated by gossip. **Not
  globally replicated**; no node is guaranteed to hold any particular anchor
  (§12.2, §12.7.3).
- **Dunbar Org (two-edge walk, §15.1).** Pre-fetched and kept warm; this is where most
  traffic goes.
- **Contact locators** — `{key → locator, sequence, TTL, last-verified}`.
- Negative results cached briefly to avoid retry storms.

---

### 12.6 Routing and aggregation

#### 12.6.1 Self-routing

**Descent is through infrastructure only.** A path is not walked node by node:
intermediate nodes may be light clients, which are neither always online nor
independently reachable (§14.1.1). Resolution descends through **infra nodes** and
terminates at the target's **serving infra node** — its nearest infrastructure
ancestor, the node it attaches to (§14.1.2).

**Resolution is iterative with referrals, in the shape of DNS.**
A requester holds out-of-band knowledge of at least one anchor's address, queries
it, and receives either the answer or **a referral naming the next hop and its
address**. It repeats until it reaches the serving node, caching what it learns.

**A node may answer for more than one step, at its own cost.** Nothing requires a
referral to advance a single index — a node caching deeper than its own children
can refer further, or return the serving node directly. **That is an optimisation
above the floor**, not a requirement: the constant-state guarantee below is a
minimum a node must be able to operate on, not a ceiling on what it may keep.

**A referral cannot be usefully falsified, which is why no machinery polices it.**
The requester knows the keyhash it intends to reach and authenticates the endpoint
against it (§14.1.3). A node that returns a wrong address produces a handshake
failure, not a silent misdirection — **the lie is self-detecting at contact**, so
an intermediary gains nothing by lying and the protocol needs no traversal state to
prevent it.

**A serving node holds its whole light-client subtree, not just its children.**
§14.1.2 has a light client attach to the **nearest infrastructure node on its
patron chain**, walking up past any light-client patrons. So every light client
beneath an infra node — at any depth, until another infra node intervenes —
attaches to that same node, and it therefore holds them all.

**Intermediate light-client patrons carry no traffic.** They adopt, countersign and
vouch; they do not serve sessions and nothing routes through them. A node admitted
to the subnet talks to the infrastructure directly from then on. **So the residual
path is resolved by the serving node itself** rather than walked — it identifies
which of its attached clients the suffix names.

**Referral is therefore only ever between infra nodes.** A node refers when the
path leads into another infra node's subtree, and answers when it does not.

**The reply returns the residual path suffix rather than traversing it.** The
serving node uses that suffix to identify which of its attached clients is meant —
and **the residual is also what tells the requester which kind of party it has
reached**: empty means the addressed party is the node itself, non-empty means the
last hop forwards to an attached client. That distinction is therefore learned at
resolution time from a party positioned to know it, rather than asserted by an
address that recorded it earlier. **Node type is deliberately not encoded in a
locator**: type is not a function of position. A node becomes infra by **launching
and signing an infra instance** (§3.3), which changes nothing about where it sits — so an address asserting terminal type would go wrong the moment its operator
stood up infrastructure, while every cached copy kept asserting it.

**Caching, and its cost.** A requester caches intermediate addresses and prunes
toward stable entries — a node with infra-grade subordinates two levels down is a
reasonable anchor to retain, which is the criterion §12.2 already uses. **Pruning
is local policy and needs no agreement.** What does need a decision is **how long a
cached address stays valid**: infra nodes move, and nothing currently bounds a
stale entry. Anchor `seqno` orders competing entries but does not expire an
uncontested one. Recorded as unset (§21.1).

**State per node remains constant at the floor** — parent plus ≤f children —
regardless of network size, since a node can answer from its own children and refer
onward rather than holding a map of who lives beneath it. This is the aggregation
argument behind §3.2 and §12.2, and it bounds what a node *must* keep rather than
what it *may*.

**An infra node publishes its endpoints as a signed record** (`wire-format.md`
§7.6), carried in the topology class and therefore reaching its patron. A light client's address arrives when it attaches (§14.1.2); an infra
child serves itself and never attaches, so without this nothing delivered its address
to its patron — and **without that the patron cannot refer**.

**`SignedLocator` is not the carrier**, and is easily mistaken for one: a `Locator`
is `{anchor, path, seqno}`, a topological position with no address in it
(`wire-format.md` §2.3). Endpoints live in `NetworkPoint`. **A peering record already
carries both endpoints' network points** (§6.3), so the record above matters most for
an infra node that neither peers nor serves as an anchor — a supported state
(§12.7.5).

**Resolution is not delivery.** Infra nodes do not relay payload except for their
own attached clients (§12.6.3). Once resolved, the requester contacts the serving
node directly; the chain it walked to find that node carries nothing afterwards.

#### 12.6.2 Stale paths fail; they are not repaired
A resolution walking a stale path fails at the point of divergence. **No node
redirects on the subject's behalf**, because none holds a pointer to where they went
(§4, deferred features).

Invalidation cost is low in expectation: most nodes are leaves, and mean subtree
size in an f-ary tree is f/(f−1) ≈ 1.11, so an average move invalidates about one
cached locator. The tail is worse — a low-tier move invalidates its whole subtree,
and every distant holder of one of those locators must re-resolve from a higher
anchor or be re-introduced. **Inside the horizon the move propagates as topology
(§15) and nothing is invalidated at all**, so the cost falls entirely on contacts
outside it.

**Merges do not invalidate paths at all.** Paths are expressed relative to the
anchor, so when an anchor acquires a patron its descendants' relative paths are
unchanged. Anchor-relative addressing is merge-stable in the same way anchor
identity is (§12.2).

#### 12.6.3 Infra nodes do not carry payload

**Only the infra tier closest to a light-client endpoint relays that client's
traffic.** The exception is control-plane transactions, which must be visible to
the local network and therefore travel the tree.

**Payload takes the direct path where it can, and the relayed path when it
cannot.**

| Path | When | Who sees the flow |
|---|---|---|
| **Direct** | Peer is **inside the horizon** (§15.1), both online, traversal succeeds | Nobody. The peers exchange addresses during setup and connect |
| **Relayed** | Peer is **outside the horizon**, *or* traversal fails — symmetric NAT, CGNAT, two mobile peers | Both serving infra nodes, as TURN relays carrying ciphertext |
| **Queued** | Recipient offline | The recipient's serving node holds ciphertext until reconnect (§14.1.4) |

**Direct connection is limited to the horizon**, because a direct path reveals
each peer's IP to the other (P17). Three reasons this is the right boundary:

- **The horizon already sees your structural position.** A locator discloses your
  patron, depth and subtree to anyone you introduce yourself to (§12.1), and nodes
  inside your horizon hold your topology regardless. IP is *incremental*
  disclosure to that set and *novel* disclosure outside it.
- **It is evaluable from state a node already holds.** "Is this peer in my h=2
  topology store?" is the entire check.
- **It should cover most traffic** (A2), so the bandwidth argument below survives.

**But the horizon is a bounded set, not a trusted one.** At f = 10 and h = 2 it is
221 nodes (§15.1), and it includes a user's nephews and their patron's siblings —
people they may never have met. A patron already sees its subordinates'
traffic metadata and gains nothing; a sibling or a patron's sibling would gain an IP
they do not have today. **The rule bounds exposure rather than restricting
it to chosen parties**, and both defaults must therefore be overridable —
relay-with-a-patron's-sibling and direct-with-a-distant-trusted-party are both
reasonable user choices (P17).

The relayed path is the federation shape (email, XMPP, Matrix). **In every case
the hierarchy carries no payload**, which is what makes the f=10 cap affordable
(§3.2).

**Direct-first materially changes infra economics.** §16.6 prices an infra node at
roughly $20/month, a figure never checked against relaying *all* payload for its
whole subtree of up to 110 users. Bandwidth would plausibly have dominated it.
Making relay the exception removes the dominant term.

**Consequence: the two planes route differently, and resolution returns an
address, not a path.**

| | Route | Uses |
|---|---|---|
| **Control plane** | Self-routed along the tree, visible locally | Adoption, departure, disavowal, peering, rotation deltas |
| **Payload** | Point-to-point between serving infra nodes after resolution | Application data |

**Serving infra nodes are a metadata chokepoint, not a content one.** Payload
**MUST** be encrypted end to end to its addressed endpoint, so a node acting as a
relay carries ciphertext it cannot read (§14.2, §15). **The requirement is
settled; the cryptographic construction is not.** Key agreement, prekeys and
forward secrecy follow from §14.2.4's adopted constructions, with integration
decisions open. An implementation that shipped hop
encryption alone would leak payload to both serving nodes, which is why the
requirement is stated normatively before the construction exists.

**Anchors resolve; they never carry.** Anchor load therefore scales with
out-of-horizon first contact, not with traffic, and most traffic is inside the
Dunbar Org, where no anchor is involved at all.

**The static IP requirement earns a second justification here.** Introduced for
statically routable infrastructure and retained as the strongest Sybil cost
(§17.3), it is also what makes infra-to-infra direct connection work without NAT
traversal, which is what makes this payload story viable at all.

#### 12.6.4 TTLs: separate security from performance

Cache TTLs are mostly performance knobs to be tuned under load. **One is not.**

| TTL | Kind | Guidance |
|---|---|---|
| Locator / resolution cache | Performance | **Not generous.** A stale locator costs delivery failure and a re-resolve from a higher anchor (§12.6.2), so the TTL trades staleness against re-resolution load |
| **Currency attestation** | **Security** | **Derive from the maximum acceptable exposure window after credential compromise, never from performance or cache-efficiency considerations.** This is how long a compromised key keeps working for parties who cached before rotation — hours, not days. See §12.6.5. |

#### 12.6.5 Key currency: adopt the PKI revocation playbook

This is **not** a session-key problem. It is the certificate revocation
problem, and the web PKI's failures there are directly transferable.

**The cautionary tale: OCSP soft-fail.** Where online revocation checking
soft-fails, the conventional OCSP behaviour, an unreachable responder
means the client proceeds anyway, so an attacker who can block the check has
defeated revocation entirely. (Not all browsers: Chrome largely abandoned online
revocation checking in favour of CRLSets, which **reinforces** the lesson rather
than weakening it, the industry response to soft-fail's uselessness was to stop
relying on the mechanism.) **Revocation that fails open under adversarial
conditions is decorative.** This is why §9.0.2 requires that "no answer from
patron or siblings" be distinguishable from "answer attests currency": if
unreachable silently means proceed, a thief who can disrupt the legitimate
patron's reachability keeps the stolen key working indefinitely.

**Adopt stapling.** Rather than the recipient querying, the introducing node
**staples a recent patron-signed currency attestation to its introduction**:

```
CurrencyAttestation {
  identity   : keyhash        # the subject
  current_key: keyhash        # which key is live
  issued_at  : timestamp
  expires_at : timestamp      # ~10 h default (§21); hours, not days
  signature  : COSE_Sign1     # BY THE PATRON
}
```

The recipient verifies locally, queries nobody, and falls back to a lookup
(§9.0.2) only if the staple is absent or expired. This removes the round trip
before first contact, removes the dependence on patron reachability at the
moment of contact, and stops the patron learning who its subordinate is talking
to.

**The fallback undoes that last property, which is why it is a fallback.** Querying tells the subject's patron that someone is evaluating or being
introduced to their subordinate, and repeated queries would map relationship
formation. Three things bound it, and the encoding is `wire-format.md` §7.1's
request type 8. **The query names the subject and not the querier**, so an answer
forwarded onward attributes the question to nobody. **Nothing is retained** — it is
liveness class (§15), answered and discarded, and never archived (§10), so no
durable record of who asked about whom accumulates at a compliant responder.
**And a caller with a stale staple should ask its introducer first**: that party
already knows the caller is talking to the subject, so a fresher staple from them
discloses nothing new, where the patron learns something it did not know.

**Prefer short lifetime over revocation machinery.** The industry direction is
away from long-lived credentials plus revocation infrastructure and toward
credentials short enough that expiry does revocation's work. **This is far
cheaper here than on the web**: the issuer is the node's own patron, a party it
is in continuous contact with by construction, so refresh is a local operation
between neighbours rather than a round trip to a remote CA.

**What would derive it:** the lifetime should be at most the expected time for a
legitimate holder to notice a compromise and begin recovery, since that is
exactly the window in which a thief holding a captured staple retains full
capability. **That detection latency is unmeasured**, so the current value is
*chosen*, not derived (§21).

Concrete anchor for the choice: MIT Kerberos documents a ten-hour default ticket
lifetime on many systems — right order of magnitude, and a sanity check rather
than a derivation. **It is not a working-day approximation.** That reading is
tempting and unsourced.

**Grade the failure mode by stakes — something the web PKI cannot do.**
Soft-fail was chosen for availability because a browser cannot distinguish a
bank from a blog. This design can:

**An operation is trust-bearing** if it would create, transfer or spend social
standing, establish a new trust relationship, or produce evidence that other
parties may later rely upon. That definition governs; the examples below
illustrate it and are not the rule.

| Operation | On expired or missing staple |
|---|---|
| Receiving messages, routine payload — nothing relied upon by others | **Fail open.** Proceed |
| Any trust-bearing operation per the definition above (presently: adoption, peering, presence ceremony) | **Fail closed.** Current control of the acting credential must be established first |

This also handles the intermittently-connected light client: a user offline for
a day arrives with a stale staple, can still receive messages, and cannot spend
accumulated standing until they refresh.

##### 12.6.5.1 Long-duration patron outage

**The cascade.** A node whose patron is unreachable cannot refresh, and after
the attestation lifetime its countersignatures (§6.4) are degraded — which
degrades its subordinates' transactions, and theirs. **A single infra node
outage propagates downward through its whole subtree**, reintroducing exactly
the tree fragility §3.4 was written to eliminate. This must be handled, not
tolerated.

**Escalation path.** Siblings already replicate the patron's data (§3.4),
including its adoption records, so:

| Outage duration | Response |
|---|---|
| < attestation lifetime | Nothing; staple still valid |
| Hours–days | **Sibling issuance** from replicated state, marked secondhand and weighted slightly lower |
| Days | **Grandpatron issuance.** It holds the adoption record anyway, being inside the h=2 storage horizon |
| Extended / permanent | **Adopt at a new patron** (§6.2). The designed remedy for a bad patron works identically for a dead one; the standing cost is proportionate at this timescale |

**Issue fresh; never extend stale.** The tempting fix is a grace period —
extend the last attestation while the patron is verifiably down. **Do not.** A
thief who steals a key and then DoSes the patron would be *buying* extended
validity for the stolen key's staple: OCSP soft-fail in a different hat, with
the attacker controlling the trigger. Sibling issuance has no such property —
siblings are independent **on the honesty axis** — DoSing the patron does not
make them lie — even though §3.4 warns they are *not* independent on the
availability axis, sharing a patron and often a provider. Issuance survives what
replication does not. A
sibling issues a **fresh** attestation reflecting whatever rotation reached the
replicated state. Stale extension preserves a thief's claim; fresh issuance can
supersede it.

**Outage is publicly verifiable, for infra patrons.** A patron above the §3.3
threshold is an infra node at a static IP, so the network can distinguish "down
for everyone" from "will not sign for this subordinate specifically", which is
censorship. Most PKIs cannot tell these apart.

**This does not extend to light-client patrons**, who may hold up to f = 10
subordinates (§3.3) with no static address and no uptime commitment. Their
unreachability is indistinguishable from refusal, so subordinates of a
light-client patron get no censorship signal, one more reason for the issuance
pre-delegation below.

**Structural tension: light-client patrons.** A light client may hold up to f = 10
subordinates (§3.3), who would then depend for currency attestations on a patron
with no uptime commitment. Resolution: a light-client patron **pre-delegates
issuance to its own patron at adoption time**. Preferred over restricting
patronage to infra nodes, which would break the property that anyone can be a
patron.

**Honest limit.** If the patron *and* its siblings are all unreachable — a
whole-neighbourhood outage — **meaning the §12.6.5.1 escalation is exhausted, not
merely that the patron is down** — no issuance path exists, and affected nodes are
frozen for trust-bearing operations until recovery or re-adoption elsewhere. Not fixable
without weakening what makes fail-closed worth having.

---

### 12.7 Rootless and disavowed nodes

A node may be patronless by disavowal (§6.2.2), by patron loss, or because it
never had one (§3.1 makes roots emergent and ordinary, so this is a normal
state rather than an error.

#### 12.7.1 Genesis identities: currency is vacuous, not missing

A **Genesis user.** No patron, no subordinates, no transaction history — has no
constituency to attest from below and no patron to attest from above. This is
not a gap in the mechanism; the question the mechanism answers does not arise.

Currency attestation exists to resolve **which of several possible keys
represents this accumulated history**. With no history, no competing claim is
constructible, so **self-attestation is complete**. If the key is lost, creating
a new identity is cheaper than recovery — recovery only becomes worth having
once there is something to recover.

This is consistent with the stake-graded failure table (§12.6.5): a Genesis user
cannot perform trust-bearing operations because there is no trust to bear, so
failing closed on them costs nothing.

**Except one.** The first presence ceremony *is* trust-bearing, and requiring a
fresh staple would make it impossible to ever perform. The rule must therefore
be: **a staple is required when a record claims prior standing, not when an
identity claims nothing.** The counterparty in a Genesis user's first ceremony is
relying entirely on the in-person meeting, which is precisely the bootstrap
problem of §13 (bootstrap). The asymmetry is sound — absence of a claim is absence of
standing, and nobody is asked to prove a negative.

#### 12.7.2 Roots derive currency from below
Currency attestations come from the patron (§12.6.5), so a patronless node cannot
staple and would be permanently frozen for trust-bearing operations. **This is a
hole in §12.6.5, independent of disavowal.**

Resolution inverts a mechanism already present: §9.2's social revocation uses a
threshold of the down-line co-signing, and the same threshold attests **"our
root's current key is K"**. A root with subordinates has a constituency to vouch
for it.

**A root without one is not always a lone identity, and that is the residual.**
The dismissal holds for a Genesis user, who claims nothing and whose currency
nobody has occasion to check (§12.7.1). It fails for a **disavowed leaf**: it has
history, §12.7.1's rule makes a staple required precisely because it *claims prior
standing*, and it has no down-line to attest from below and no patron to attest from
above. **Adoption is itself trust-bearing** (§12.6.5's table), so the operation that
would restore an issuer is gated on already having one.

**A patron can therefore manufacture this state deliberately**, which is what
distinguishes it from the availability gap Appendix B records: one disavowal freezes
a leaf's trust-bearing operations once its last staple expires. The escapes are a
second binding established beforehand, an evaluator willing to proceed without
currency, or returning as a Genesis identity and **abandoning the accumulated
history** — which converts denial of service into destruction of portable standing.
**"Re-adoption is available" is therefore not a general answer to patron abuse**, and
§18.5 should not be read as offering one.

#### 12.7.3 Anchor caching is a per-node policy, not a protocol constant

Two quantities look like one and are not:

- **How many patronless identities exist.** Unbounded, and fine.
- **How many any given node caches.** A local policy decision.

Conflating them produces an apparent anchor-budget crisis and a spurious fixed
size threshold. There is no budget to blow, because no node is obliged to cache
anything.

The anchor table is *local state*. Nothing in this design promises global
addressability (§12.4 already rules out cold lookup), so a node that cannot hold
every root simply holds fewer, and disjoint non-communicating partitions are an
explicitly supported outcome (§1) rather than a failure.

**Therefore: each node sets its own threshold for which roots it caches.** For
example, ignoring roots with fewer than *n* subordinates, with *n* a prudential
choice based on current network characteristics.

**This must not be a protocol constant.** In the early network — thousands of
nodes, roots with a dozen to a few hundred subordinates, the anchor concept is
essentially inert, and a fixed threshold of 500,000 would render every root
invisible in exactly the phase where visibility matters most. Expect *n* near
zero at that stage, rising as the network grows. The anchor mechanism exists to
provide *more entry points than one* into very large subtrees; it has no work to
do until subtrees get large.

Consequence: reachability is policy-dependent and may be asymmetric — node A can
reach a root that node B does not cache. Consistent with the rest of the design,
where reachability and trust are already local and policy-determined.

#### 12.7.4 Outcomes by node size

| Node | Outcome |
|---|---|
| **Light client, no subtree** | Unattached identity retaining its transaction history; cached only by nodes whose policy is permissive enough (§12.7.3); seeks a new patron |
| **Small subtree, below threshold** | Subtree stays internally functional; externally reachable only via peering edges (§6.3) |
| **Infra node, subtree above threshold** | Large enough that other nodes' caching policies will generally retain it (§12.7.3), so it continues operating as a root with the subordinate network reachable. Note this is *other nodes deciding to cache it*, not a status it acquires (§12.2) |

#### 12.7.5 Peering as reachability insurance, and why it stays optional
Cross-tree peering (§6.3) gives a patronless infra node routes into the wider
network with no patron at all. This is a **third justification for peering**,
after fault independence (§3.4) and trust attestation (§16.3), and it creates a
sensible incentive for infra operators to maintain peering edges they might
otherwise skip.

**Peering must nevertheless remain optional.** *Creating an identity or
establishing an ordinary authority relationship must not depend on first obtaining
cooperation from an unrelated infrastructure operator; relationships whose purpose
is additional cross-tree redundancy or connectivity remain optional.* Concretely:
an identity must be able to exist disconnected, and a disconnected node must be
able to adopt or be adopted before it has any peers. The ordering resolves this structurally. Peering is an
**infra-tier** operation (§6.3) while bootstrap happens before a node has
infrastructure at all, so the ordinary growth sequence is identity → adoption → grow
to infra → peering, and a new node could not peer even if peering were mandatory.
**Subnet formation is the exception** (§13.1). *When participants create a new
disconnected authority domain rather than joining an existing one, they must
establish at least one externally reachable serving participant before creating
the domain's first authority relationship.* Concretely: identity → infra → PoP →
adoption. Both orderings are correct for their case.

**Three dependencies, three graceful degradations.** Peering being load-bearing
for three subsystems is not a hidden dependency, because absence degrades rather
than breaks:

| Without peering | Result |
|---|---|
| Fault independence (§3.4) | Replication falls back to siblings only — cut-1, single point of failure at the patron, but functional |
| Trust attestation (§16.3) | Simply less trust surface; nothing breaks |
| Reachability insurance (this section) | A disavowed node is unreachable until it reattaches |

**But peerlessness in an *infra* node should be visible.** §12.6.5.1 established
that an infra outage cascades downward through countersignatures across its whole
subtree, so an infra node with no cross-tree peers exposes its subordinates to a
cascade with no independent replication path. That is not the operator's private
risk. It is their down-line's. Peering transactions are public, so this is
observable: **trust policies should discount peerless infra nodes, and the
reference client should tell an operator plainly what their subordinates are
exposed to.** Policy-weighted, not mandated.

#### 12.7.6 Trust consequences
No special-casing needed. A disavowed node keeps everything flowing from below,
since those edges are untouched, and loses most external standing, since
observers now reach it only through peering edges whose flow capacity §16.3
deliberately sets low. Internal cohesion retained, external standing reduced.

#### 12.7.7 Re-rooting cost
The disavowed node's subtree had anchor-relative paths (§12.6.2). Those paths
re-root on the node itself, so every locator beneath it changes. **Within the
horizon the disavowal propagates as topology** (§15) and neighbours re-resolve from
what they already hold. Beyond it, cached locators into that subtree fail and their
holders re-resolve from a higher anchor (§12.3 Case 3). No flood, and no pointer
left behind.

---

## 13. Subnet formation and lifecycle

**Not a genesis procedure.** This runs every time a disconnected group starts,
so it will execute thousands of times and must be built as a first-class path,
not as bootstrap scaffolding discarded after launch. Expect many roots and many
disjoint subnets as the normal early state (§1).

### 13.1 Procedure
1. Two users create identities (Genesis identities per §12.7.1 — self-attested,
   no standing).
2. **At least one** launches and signs an infra instance. Without a static IP the
   pair has no serving infra node (§12.6.3) and is unreachable to anyone outside,
   including someone who wants to join them.
3. They perform a **proof-of-presence ceremony with no witnesses and no
   verifiers.** There are no neighbours to witness and no prior counterparties
   to query.
4. One adopts the other and becomes the subnet's first root.

**The meeting is the primitive, not the adoption.** The relationship is
established first and the hierarchy follows from it. This also dissolves the
cycle hazard of §6.2.5 structurally: the ceremony is symmetric, and the adoption
that follows is a single deliberate asymmetric choice.

### 13.2 Formation records are permanently distinct
A record with empty `witnesses[]` and empty `verifier_responses[]` carries no
third-party attestation, and still carries none in twenty years. **Evidence
created when the participants had no independent witnesses and no prior
counterparties must remain permanently distinguishable from evidence created with
independent corroboration.** Regardless of its age or of how much the network
has grown since. The record's subtype field is the present encoding of that
invariant, not the invariant itself.

Two colluding parties can mint these at will, so the honest position is that a
**formation record is evidence only to its two participants**. It bootstraps
their mutual trust and confers nothing on anyone else.

This needs no special rule under the flow metric (§16.2): a fresh pair has no
edges into anyone else's territory, so nothing flows to them regardless of what
their record asserts. The only requirement is *labelling*, so no policy can
mistake self-attestation for independent attestation.

### 13.3 Infra: require one, recommend two
Requiring both to run infra doubles the entry cost for what may be two people in
a family or a club, and §3.3 does not demand it, a root with one subordinate is
far below the two-level threshold.

But two is materially safer, for the reason established in §12.6.5.1: if only the
root runs infra and it vanishes, the other node has no sibling, no peer and no
grandpatron, the whole-neighbourhood outage case with n=1. **Require one,
recommend two, and have the client explain why.**

### 13.4 Which one becomes root is a choice with no lasting consequence, since either party can depart
Worth stating so no ceremony accretes around it. Departure is unilateral (§6.2)
and subnet plurality is unpreventable (§3.1.1), so the subordinate can leave or
join elsewhere at will. Whoever runs infra is the natural pick; beyond that it does not
matter.

### 13.5 The ceremony degrades by availability
Generalises past bootstrap:

| Stage | Witnesses | Verifiers |
|---|---|---|
| Subnet formation | None available | None available |
| Young subnet | Available | None, no prior counterparties yet |
| Mature | Available | Available |

Same principle §8.1 applies to proximity evidence: **the record states what was
available and at what strength**, rather than pretending to a uniform standard.
A young subnet's records are legitimately weaker than a mature one's and should
say so, rather than being padded or rejected.

### 13.6 Key rotation is per-subnet

Each binding has its own patron and its own currency attestation (§12.6.5), so
rotating in one subnet does nothing in another, the "each institution has its
own name change form" property of §3.1.1.

**Hazard:** a compromised user must rotate separately everywhere, and an attacker
continues operating in any subnet where they have not. **Mitigation is
application-level**, consistent with §3.1.1: the client already tracks all
bindings locally in order to present the right name and address to the right
person, so it can prompt rotation across all of them on compromise.

### 13.7 Two consequences of the postal model

**Multiple identities is a client capability the protocol already permits.** Nothing in the wire format or the topology binds a device to
one key: an identity *is* a key, adoptions are per-identity, and a client holding
several is running several identities as far as the network can tell. **What is
deferred is the client work** — key management and the interface for choosing
between them — and §4 lists it as a v1 client-scope exclusion, not an architectural
limit.

**The consequence is therefore conditional, and belongs to the client.** §3.1.1's
analogies presume different names in different domains, and a **single-identity
client** makes subnet plurality correlating: anyone present in two of that user's
subnets can link them. A **multi-identity client** does not have that property. **No
wire change separates the two cases**, which is why this is not a protocol
limitation and should not be recorded as one.

**It is nevertheless a client concern, not a protocol one.** Multiple keys are
local data plus the interface for working with them; the network sees only
unrelated identities. The only plausible reason the *protocol* would need to know
two keys belong to one person is to propagate a compromise warning between
subnets, which requires exactly the linkage this design refuses. So it stays
client-local by necessity rather than convenience: the client can warn its user
across all bindings, and the network never learns why. The one protocol-level
requirement that remains is the key rotation/recovery transaction, which is
needed regardless (§9, §13.6).

**Exhaustive-disclosure investigation degrades as the client matures, but only
partly.** An application demanding a full accounting of identities (§3.1.1) could
extend itself to query untrusted subnets for matching keys and confront the user
with what it finds. Against v1 this works, because a shared key *is* a matchable
identifier. Against the deferred multi-key version it does not reach keys that
were **independent from birth.** But it *will* turn up any key rotation
involving a known key as either the pre- or post-rotation key, since the rotation
record publicly binds the two and the chain can be walked in both directions.

**Therefore: recovery and unlinkability are the same choice seen from two sides.**
Recovery works precisely by publishing the link. Unlinkability survives only for
separate Genesis identities (§12.7.1), never for rotations. This makes "abandon
and start fresh" a **privacy operation distinct from recovery**, and the client
should present it as such rather than treating rotation as the default answer to
"I want out of this subnet". The capability is also bounded by §12.4: with no cold
lookup, an investigator needs standing in each subnet before key-matching is even
possible, which mirrors the real-world constraint that the capability is social
access rather than cryptographic power. Neither the protocol nor the reference
apps should implement any of it.

**Unlinkability is a property of where you reappear, not of what you avoid
signing.** A separate Genesis identity defeats key-matching (§12.7.1); it does not
defeat anyone who was in the room. **Appearing fresh in the same neighbourhood is
not unlinkable at all** — someone there adopted you, which required meeting you.
The operation is meaningful when the new identity appears in a subnet where nobody
knows the old one, and §12.4's absence of cold lookup is what makes that hold: an
investigator needs standing in both subnets before key-matching is even possible.

**The device is the single point of failure for the cross-subnet identity.** The
linkage between bindings lives exactly where §3.1.1 says the edge passes —
local storage and the person. The network is partitioned by design and has no
global chokepoint; a phone is not partitioned and is a complete one. **The
strongest attack on a multi-subnet identity is device compromise, not anything
at the network layer.** This relocates the risk rather than eliminating it.

#### 13.7.1 Backup: append-only, but not with default tooling

The local store is **not dynamic.** The transaction archive (§16.7) and photo
records (§7.2) are append-only, and cached locators and anchor tables are
re-derivable rather than needing preservation. So ordinary file backup is
sufficient in mechanism, and no continuous sync is required.

**But the default mechanisms are wrong for this payload**, for two reasons:

- **The backup blob is the aggregation the design avoids everywhere else.**
  Every subnet key plus photographs of everyone the user has met, in one file.
  Handing that to consumer cloud storage rebuilds precisely the central point
  §7.2 worked to prevent. **Encrypt under a key not stored alongside it.**

  Standard construction, no invention required: **envelope encryption.** A
  random data key encrypts the blob, and a key-encryption key (KEK) derived from
  a passphrase encrypts the data key. Argon2id for the KDF (memory-hard; **raises the cost
  of** GPU and specialised-hardware attack rather than preventing it, and only in
  proportion to the memory and time parameters chosen), XChaCha20-Poly1305 or AES-256-GCM for the payload, KEK never
  stored with the ciphertext. Broadly what 1Password, Bitwarden, KeePass and
  Signal backups do.

  **The format leaves a seam.** How the KEK is protected is a pluggable layer
  above the backup format: passphrase in v1, hardware token later, split shares
  later still — none of which changes the format. So the harder options need not
  be decided now.

  **On splitting the KEK among trusted counterparties** (the §9.1 principle
  applied to backup): attractive, but cost it honestly. The "don't roll your own
  crypto" rule is about *primitives*, not composition — **SLIP-39** already
  standardises Shamir backup splitting and has reference implementations, so
  adopting it is composition. The real cost is elsewhere: plain Shamir is **not verifiable**, and
detecting a bad share needs a verifiable scheme — Feldman and Pedersen VSS are the
classic examples rather than the only options. Social recovery also has a poor practical
  record; the usual failure is holders being lost track of or unreachable when
  needed. **If adopted, share holders should be disjoint from the presence-recovery
  quorum (§9.1).** Otherwise compromising one group of *k* people yields both
  the backup key and the identity attestation.
- **A backup that outlives declared retention makes the retention commitment
  false.** §7.5.1 has the record committing to a photo-retention period at
  meeting time, and counterparties rely on it when weighing a later
  "unavailable" response. Backups that silently preserve photos past two years
  mean the commitment was never real.

  **The fix is scan-on-import, not cron-style aging.** Age-based flushing acts on
  live data, but a restored backup reintroduces files that aged while offline —
  import is exactly where the leak occurs. The reference client must **scan any
  bulk or side-loaded data for past-retention files and discard them**, covering
  device migration and manual copies as well as restore.

  **Honest scope:** this is a statement about *client behaviour*, not a
  cryptographic guarantee. No system short of rooted, boot-locked, fully
  encrypted hardware can control a user's handling of device-local data.

  **Over-retention is neither enforceable nor detectable against a hostile
  client**, though §7.5.2 makes a *compliant* client structurally incapable of it. It looks detectable by its own use — answering a query about a
  five-year-old meeting under a two-year promise appears to expose the violation —
  but `basis` is
  client-asserted (§8.1), and the document already concedes that no third party
  can tell whether a photo match was actually performed. A client that retained
  photographs illegally simply runs the match, returns the result, and sets
  `basis = personal_knowledge`. Nothing distinguishes that from genuine human
  recognition.

  **There is no protocol fix.** Nothing can prove a human used memory rather
  than local storage. Weighting `personal_knowledge` below `photo_match` would
  penalise the honest memory-based verification the design deliberately supports
  (§8.1). So: retention promises bind honest clients and are **unenforceable and
  undetectable against hostile ones**, and the document should not suggest
  otherwise.

Note that backup and presence-based recovery (§9.1) are complementary rather
than redundant: backup covers key *loss*, recovery covers key *compromise*, and
a compromised backup is itself a compromise path.

---

## 14. Sessions and payload

How a client attaches, stays attached, and moves data, and how that data is
protected end to end. Addressing (§12) answers *where*; this answers *how*.

### 14.1 Session establishment and failover

#### 14.1.1 NAT traversal — required for the direct payload path

**Control traffic needs none.** Clients dial **outward** to their serving infra
node at a static IP; infra nodes dial each other directly, static IP to static IP.
Session establishment, attestation pulls, resolution and queue operations are all
client-server at the edge and server-server in the core, and none needs hole
punching.

**Payload takes a direct path where it can** (§12.6.3), meaning *inside the
horizon*, since a direct connection reveals each peer's IP (P17), and that does
need traversal. **There is no way to avoid it**: an always-relay model would make
traversal unnecessary at the cost of routing every payload byte through
infrastructure, which §12.6.3 declines for both cost and privacy reasons.

**Infra nodes act as STUN and TURN.** This adds no capability they did not already
have — relaying payload *is* what a TURN server does. What changes is that the
relay becomes the **fallback**: ICE attempts the direct path first and falls back
to the serving node when it fails.

**Feature capabilities are separate from the handshake and deliberately
non-fatal** (`wire-format.md` §8.1): parameters set limits rather than gating a
session, so version skew costs features and never connectivity, which matters
most for a light client, whose only alternatives are its serving node's siblings.
**Tolerating unknown parameters is mandatory and checkable; sending greased ones is
self-interested rather than obligatory.** An implementation that never greases
relies on others to keep the tolerance path exercised for it, and an extension
mechanism nobody exercises ossifies until it no longer works.

**Expect the fallback to be used.** Address- and port-dependent mapping defeats
hole punching, and carrier-grade NAT and mobile networks raise the odds of
encountering it, though **neither guarantees failure**. Success depends on each
NAT's mapping and filtering behaviour, on IPv6 availability, and on which candidate
pairs ICE can form (RFC 8445).
Comparable deployments commonly relay a substantial minority of connections. The
relay path is not vestigial and must be maintained as a first-class route.

#### 14.1.2 Attach and heartbeat

**Attachment answers one question: where do my messages queue.** It is singular
because a mailbox must have one address. **It is not a restriction on which nodes a
client may open a session with** — messaging already has a client reach a
recipient's serving node (§12.6.3), and resource requests and catalog queries do the
same (`wire-format.md` §11.1).

1. Light client dials out to its **serving infra node.** The nearest
   infrastructure node on its patron chain, which **is not necessarily its
   patron** (§12.6.3). A light client whose patron is itself a light client (§3.3
   permits this) walks up until it reaches infrastructure. Serving and
   countersigning are separate roles: a light-client patron still countersigns
   subnet-scoped transactions, it simply does not serve sessions.
2. Periodic heartbeat keeps the connection alive. **Interval unset.** A
   performance parameter to tune under load, traded off between battery cost and
   failover detection latency. QUIC 0-RTT resumption (§14.1.3) makes reattachment
   cheap enough to favour a lazy interval.
3. **Client heartbeat fails (server side):** the patron **marks the client
   unreachable and begins queuing.** It does not simply do nothing, because
   §7.4.3's queue mechanism depends on the patron distinguishing "offline"
   from "no record". **This state replicates to siblings**, or a sibling
   answering during failover has no idea of the client's status.
4. **Server heartbeat fails (client side):** after **3 consecutive missed
   intervals**, the client attaches to another party in the same replication set
   as its serving node, and **the resulting session is degraded.** That party
   holds the replicated state but not the authority to countersign. Present
   encoding: a sibling of the serving infra node (§3.4) —
   siblings of the serving node, not of the patron, where those differ. The
   sibling determines from its own topology that this client is not in its
   subtree, and therefore that it is providing failover rather than primary
   service; `AttachAck` reports that determination so both ends agree the session
   is degraded (`wire-format.md` §8). **No automatic failback**: the client stays on
   the sibling until that session ends, and the next fresh attach tries its actual
   serving node first.

**The sibling list must be pushed to clients during normal operation.** A client
cannot discover failover targets after its patron is already dark. Cheap at
session establishment; impossible to add afterwards.

**Attachment to a sibling is a degraded state, and the reference client must make
that explicit to the user.** A sibling is not the patron and cannot countersign
(§6.4). Payload flows; trust-bearing operations do not, the same shape as the
secondhand currency attestations of §12.6.5.1.

**"Explicit" here means a product obligation, not a wire field**, of the same kind
as §19.6's disclosure requirements. A user whose trust-bearing operations
silently stop working will read it as the application being broken, and will not
know that it resolves on reconnection. The serving sibling determines the mode
from its own topology, a client not in its subtree is in failover, and reports
it in `AttachAck` (`wire-format.md` §8), because a client with stale topology may
not know which state it is in.

**Asymmetry:** client-detects-server matters more than server-detects-client,
because only the client can act on it.

#### 14.1.3 Transport: QUIC
Mobile clients change IP constantly, and QUIC's connection migration survives
that natively where ordinary single-path TCP does not, a TCP connection is bound
to its endpoint addresses and ports. Multipath TCP is a different matter and is not
what clients run. 0-RTT resumption makes frequent reattachment
cheap, which permits a lazy heartbeat and saves battery.

#### 14.1.4 Mobile OS policy is the binding constraint
iOS suspends apps shortly after backgrounding and Android Doze suspends ordinary
network access, so **an arbitrary always-on socket is not reliable** for a
normally backgrounded app. Both platforms offer constrained alternatives —
background `URLSession` transfers, approved background modes, Doze maintenance
windows, foreground services, WorkManager, so background networking is
**constrained rather than categorically impossible**, and **APNs and FCM are the
common and most dependable wake mechanisms rather than the only ones**. Relying on
them introduces **Apple and Google as central parties** into a design that refuses
central parties everywhere else.

*Unreliable is sufficient for the conclusion below: a design cannot depend on a
persistent connection it cannot count on.*

**Connection model options:**

| | Real-time | Central dependency | Complexity |
|---|---|---|---|
| A. Persistent + push | Yes | Apple/Google | High |
| B. Foreground + store-and-forward | No | None | Low |
| **C. B default, push opt-in** | Optional | Only if opted in | Medium |

**Decision: C, with B as the default.** §1's traffic profile is deliberate,
occasional and foreground — presence ceremonies are synchronous by nature and
the DNS/SSL-replacement case is request-driven, so store-and-forward at the
patron plus fetch-on-foreground covers v1. Push is a later feature paid for in
dependency, and **enabling it should be treated as a declared degradation of the
trust model**, plausibly an attribute in the record the way client integrity is
(§7.8).

#### 14.1.5 Push is a doorbell, not a mailbox
The patron sends **content-free** pushes through OS channels that only prompt
the user to open the app and re-establish a session; all payload moves over the
network's own channel and notification text is rendered locally. Signal uses
this pattern.

**What the OS vendors still learn:** that a push went to this device at this
time, from this app, and the frequency pattern that implies. Real, but a
different order of magnitude from payload, and unavoidable where vendor push is
the wake mechanism, the dependable one, though the platforms offer constrained
alternatives (§14.1.4), for background
delivery on mobile.

**Caveats.** iOS throttles silent (`content-available`) pushes as explicitly
best-effort, so they cannot be relied on for anything time-sensitive; a
user-visible notification delivers more reliably at the cost of an interruption.
The push token is a routable identifier the patron holds, a small additional
linkage on top of what §12.6.3 already grants. Note it is **stable while valid,
not permanent**: Apple documents that device tokens change periodically and must
not be cached as immutable identifiers.

#### 14.1.6 The patron as mailbox

**Undelivered messages queue indefinitely at the direct patron, bounded by a
per-subordinate storage cap.** No time limit; a space limit.

**Two reasons, and the second is the stronger one.**

**Light clients may connect rarely**, and a time-based window would drop the
messages of anyone whose absence exceeded it. A space bound survives an absence of
any length; a subordinate who accumulates more than their share hits a ceiling
rather than a clock.

**A dropped verification query damages its subject, not its sender.** A verifier
who never receives a query cannot answer, so its slot stays absent
(`wire-format.md` §5.5) — and the absence weighs against the person being verified.
**Expiring a queued query therefore penalises a third party for their verifier's
connection habits** — and it falls hardest on light clients, who are the most likely
both to be queued and to be verified. A queue that expires messages makes
verification quietly less reliable for exactly the users the design exists to
include.

**Siblings do not hold queue state.** Failover covers *sessions*, not
mailboxes: a client attached to a sibling still collects from its own patron once
that patron returns. **The message waits; it is not lost.** This removes the
metadata-spreading question entirely. Who has mail waiting, from whom, and for how
long is known to one node rather than to a replica set.

**What the patron still learns** is the chokepoint §12.6.3 already accepts,
deepened: not only who talks to whom, but what is queued and for how long.
End-to-end encryption (§14.2.4) means the queue holds ciphertext, so the residual
is **queue metadata**, which encryption does not touch and which C5 joins with
heartbeat data to reveal when someone came online to collect a particular message.

**What remains open is the cap's *value* alone**, which §21.1.1 classifies as
freely tunable per node and which nothing here fixes.

**At the ceiling: refuse the newest message and tell the sender. Never drop the
oldest.** Two reasons, and the second is the decisive one.

Dropping the oldest destroys a message the sender believes was accepted, which is an
invisible failure; refusing the newest produces an error the sender receives and can
act on — retry, direct path within horizon (§12.6.3), or tell the user. §1.1's whole
posture is converting invisible failures into visible ones.

**And drop-oldest is an attack.** Anyone who can send to a queue could flush earlier
messages by flooding it, with the victim never learning anything was there.
Refuse-newest bounds an attacker to denying *new* delivery, which the sender sees.

*The net behaviour of either discipline is the same for the honest case — a user who
does not connect often enough to keep their queue drained appears unreliable, and
that is the correct signal. The discipline is chosen on the adversarial case, not the
honest one.*

**No copy outlives delivery.** Deletion on delivery is immediate and leaves
nothing recoverable — no journal, no tombstone. Sibling replication was already
declined above, so a patron failure already loses queued messages; a crash-recovery
copy buys durability against a strictly smaller failure than the one already accepted
and pays for it with the retention window C5 joins against heartbeat state. **Durable
storage for crash recovery is the operator's own backup problem** and duplicating it
in the protocol adds complexity for a guarantee the design does not make.

**Queue metadata is the minimum: ciphertext, recipient keyhash, arrival time.**
Anything richer is C5's ingredient list.

**Operator logging is a commitment, not a rule.** §1.1 returns *no enforcement
available*, so a MUST would be a wish. §15's process-and-discard extends to queue
events as an obligation in `infra-client-requirements.md`, alongside a declared
retention posture.

### 14.2 Payload confidentiality

**All messages are end-to-end encrypted to their addressed endpoint.** The
endpoint varies, and naming the cases is what keeps the rule from collapsing.

**A node that carries application traffic is an endpoint, not a relay, and the table
says so.** It cannot both insert an authenticated credential and be
unable to read what it inserts into.

**But it does not always carry it, and nothing here assumes it does.** §11.7's
default is **broker rather than proxy**: the node authenticates and hands off, and
the user connects to the service themselves. A light client runs on an ordinary
network-enabled device and makes its own outbound connections — an adaptor package
that makes the node a third-party authenticator to a service the client reaches
directly is the same SSO shape §1.3 already describes, with the infra node as an
unusual identity provider. **What a resource does is deliberately not enumerated**
(§11.4), so how it is reached must not be assumed either.

**Where the node does carry traffic across a network it uses HTTPS**
(`resource-requirements.md` §3), so the exposure is the operator's and not also
everyone's along the path. A package on the node itself is reached over a local
socket, where ordinary HTTP is sufficient because nothing is in between.

| Case | Endpoint | Who sees plaintext |
|---|---|---|
| **Leaf → leaf** | The recipient node | Recipient only. On the direct path nobody relays; on the fallback path both serving infra nodes carry ciphertext (§12.6.3) |
| **Leaf → patron** | The patron | The patron, legitimately. It is the addressed party, not a relay. Attach, heartbeat, queue operations, currency requests |
| **Leaf → resource**, hosted on the node | The resource (§11) | The resource **and its hosting node**, which parses and re-serialises the request to insert the credential (§11.0.1, `wire-format.md` §11.2). An endpoint, not a relay, and the same visibility §11.2 gives it over role and membership |
| **Leaf → resource**, brokered | The resource | **The resource alone**, and this is the default (§11.7): the node authenticates and hands off, the client connects to the service itself, and no application traffic crosses the node |
| **Leaf → resource**, proxied | The resource | The resource and the proxying node. §11.7 keeps this off the default path precisely because it makes the node a content chokepoint |

#### 14.2.1 The patron's two roles

**As a relay, the patron sees ciphertext. As an endpoint, it reads plaintext.**
Conflating these is exactly how "the patron sees everything" returns after being
designed out, so the roles are named separately and a message is one or the other.

§12.6.3 accepted the patron as a **metadata** chokepoint. §19.1.2 establishes it
must not be a **content** chokepoint. This section is where those two statements
are reconciled.

**Enforcement works here because node and requester share the state the decision turns on.** The client does not give the relay
the key. Nothing to observe, weight or attest (§1.1's diagnostic comes out
affirmative, as it does for resource access control (§11.1).

#### 14.2.2 Store-and-forward forces asynchronous key agreement

§14.1.4 makes store-and-forward the default, because mobile clients are usually
backgrounded. **You cannot run an interactive handshake with a recipient who is
not there.** Therefore:

- Each node **publishes prekeys**, which its patron serves on request, the X3DH
  shape. Carriage is in `wire-format.md` §7.8; the bundle itself is opaque to this
  protocol, since only the endpoints hold the state to interpret it.
- **Prekey exhaustion degrades forward secrecy rather than blocking messaging**: an
  attacker drains a target's one-time prekeys, after which sessions open from
  reusable or last-resort material and the first message loses one-time-key forward
  secrecy (§14.2.4). Delivery continues. Standard mitigation is a reusable
  **last-resort prekey** at reduced forward secrecy, which must be an explicit
  accepted degradation rather than an accident.
- **Rotation policy** for prekeys is open (see §14.2.4).

**New metadata event:** fetching a prekey tells the patron you are about to
message someone, *before* you do. For leaf-to-leaf traffic the patron already sees
the routing, so this is early warning rather than new information, but it is new
for any traffic that would otherwise not have traversed them. Belongs in §19's
composition analysis once designed.

#### 14.2.3 Signing keys are not encryption keys

**§5.1 specifies signature tiering only.** Identity keys sign; encryption requires
KEM keys, which are distinct and must be bound to the identity. §5's table assigns
a PQ KEM to *transport* and says nothing about payload encryption keys, prekeys,
or how any of them relate to the hybrid identity construction.

This is a gap in §5, not merely in this section.

#### 14.2.4 Construction — import rather than invent

**There is directly applicable prior art, specified and formally verified.** The
requirement below is to adopt it, not to design a replacement.

##### Only leaf-to-leaf needs the asynchronous machinery

| Case | What it needs |
|---|---|
| **Leaf → patron** | Nothing new. The endpoint is online by definition and the §14.1.1 transport handshake already gives an authenticated, PQ-hybrid channel |
| **Leaf → resource** | Nothing new. The resource is reached through an infra node that is online by definition (§11) |
| **Leaf → leaf** | **The full construction below.** The recipient may be offline, which is the whole difficulty |

That narrows the import considerably: two of the three endpoint cases are already
covered by transport.

##### Key agreement: PQXDH

**Signal's PQXDH** is designed for exactly this situation, one party offline
having published prekeys, the other wanting to send immediately and establish a
session. **It composes a classical Diffie-Hellman with a post-quantum KEM** and is
parameterised over both rather than fixed to particular primitives; this profile
instantiates it with X25519 and ML-KEM-768, matching the transport (§14.1.3).

It has a **published specification and formal verification**: ProVerif and
CryptoVerif analysis (Bhargavan et al., USENIX Security 2024) found flaws in the
first version, and the resulting fixes were incorporated into the published second
revision with a security proof. Adopting a construction that has been through that
is strictly better than a hand-rolled equivalent.

**§14.2.2 had already reached the X3DH shape independently.** Prekeys published by
each node, served by its patron, with exhaustion as a denial-of-service and a
reusable last-resort key as the mitigation. Importing PQXDH replaces a
reconstruction of that with the analysed original.

**One decision this design must make differently.** PQXDH's published revision
relies on the discrete-log problem for **mutual authentication**, while providing
post-quantum *confidentiality*. §5.1's identity keys are hybrid, so authentication
here can bind to the post-quantum component as well. That is a place where this
design should exceed the deployed profile rather than inherit its limit.

##### Session: the Triple Ratchet

**Signal's Triple Ratchet.** The Double Ratchet running alongside the **Sparse
Post-Quantum Ratchet (SPQR)**, with their outputs mixed — provides forward secrecy
and post-compromise security against a quantum adversary. Released October 2025,
specified publicly, and formally analysed.

**SPQR solves a problem this design would otherwise hit head-on**: ML-KEM keys are
too large to put in every message header, so it transmits them as erasure-coded
chunks across successive headers. This design is unusually sensitive to object
size (§5), so a construction that already addresses key-size inflation is worth
more here than elsewhere.

##### A property that falls out, and is wanted

PQXDH provides **a form of cryptographic deniability** for the session, subject to
the limitations Signal's specification states about what each party holds and
publishes. That is the opposite
of what the control plane wants. Attestations are deliberately non-repudiable
(§19), and exactly what the data plane wants: **you should be able to prove you
met someone and not be able to prove what you said to them.**

The split is not a compromise between the two goals. It is the correct arrangement
of them, and it arrives free with the imported construction.

##### Blanket prefetch defeats the intent signal

**A prekey fetch discloses intent to message, before any message exists** (C11).
Fetching on demand therefore announces each intended conversation to whoever serves
the bundle.

**The two fetch classes are structurally distinct on the wire, so intent need not
be inferred.** A batch request names the population it sweeps; a single-subject
request is visibly targeted (`wire-format.md` §7.8). **Asking that fetches be
*made independent of intent* would state a property no observer could check**, since
the fact of a fetch is shared and the motive is not (§1.1).

**Speculative depletion of consumable material is bounded, not forbidden.** A
serving node cannot know whether a requester is really opening a session, and
binding consumption to session-opening evidence is circular: under PQXDH the key is
needed before the session exists. The node therefore rate-limits one-time key
issuance per requester per subject, which bounds the harm — draining a pool to
force someone onto their last-resort key — without anyone having to prove motive.

Present encoding: reusable prekey material is prefetched across the Dunbar Org as a
batch; one-time keys are requested singly and rate-limited.

Two properties follow:

- **A uniform fetch carries no information about intentions.** Everyone is fetched
  whether or not they will be messaged, the same shape as defeating access-pattern
  leakage by reading every record.
- **The timing inverts.** A refresh is driven by *peers' rotation schedules*, so it
  reveals **past** activity. That someone rotated — rather than **future** activity.
  That is a strictly weaker disclosure.

**One-time keys are excluded from the prefetch**, and the exclusion is what makes
it work. Serving a one-time key consumes it, so blanket prefetch across an org of
that size
would drain every pool continuously — and, worse, **make exhaustion the normal
state.** Exhaustion is currently a usable signal that someone is draining a pool
deliberately; under blanket prefetch of one-time keys an attacker would be
indistinguishable from ordinary traffic.

So: **reusable material prefetched for the whole org; a one-time key requested only
when actually opening a session.** The cost is that a session opened from prefetched
material alone lacks one-time-key forward secrecy **for its first message**: a later
compromise of that peer's signed prekey exposes it. **Protection improves as ratchet
contributions are incorporated**, not immediately — how quickly depends on the
message pattern and the compromise model. Against announcing each intended
conversation, that is still the better trade.

**This is the horizon's seventh job** (§15.1): prefetch scope. Outside the org the
intent signal returns, since blanket prefetch does not scale past a bounded set —
but given A2 that is the rarer case, and such traffic is relayed anyway (§12.6.3).

##### Still to settle

- **Prekey rotation and last-resort policy.** Cadence, and the accepted reduction
  in forward secrecy when the last-resort key is used.
- **Binding the session to §5.1's hybrid identity**, per the authentication point
  above.
- **Whether prekeys are served only by the patron** or also by siblings. Serving
  a **one-time key** is a metadata event (§14.2.2, C11), and spreading it changes
  who sees it. Serving reusable material is not.
- **Payload-type demultiplexing.** Protocol objects ride the end-to-end channel —
  capture key grants (`wire-format.md` §7.3), late verifier responses
  (`wire-format.md` §7.4) —
  beside application payload, and nothing says how a recipient tells them apart.
- **Crate maturity.** §5.2's audit caveat applies here too.

---

### 14.3 Group operations, a client convention, not a protocol feature

**Group operations are an abstraction over pairwise operations**,
provided by a standard library rather than the protocol.

```
fanout_down(depth, message)   ; deliver to subordinates to the given depth
fanout_up(height, message)    ; deliver along the patron chain to the given height
```

Each expands to **independent pairwise deliveries** (§14.2), each end-to-end
encrypted to its own recipient. Nothing is atomic, nothing is ordered, and no
recipient learns who else received it unless the message says so.

**Why not a protocol-level primitive.** Group semantics at the protocol layer
would mean attempting ACID-shaped updates across a network built around extensive
caching, store-and-forward and horizon-limited propagation. It cannot support
that, and pretending otherwise would produce a primitive that fails in ways
callers cannot reason about. A library that visibly loops over pairwise sends is
honest about what the network is.

**Guarantees, stated so callers do not assume more:**

- **Best-effort delivery.** Offline recipients receive on reconnect (§14.1.4); the
  caller is not told when, or whether.
- **No atomicity.** Partial delivery is normal, not an error.
- **No ordering** between recipients, and none is recoverable (§10.3).
- **No implicit group identity.** A fanout is a set of messages, not a channel.
  Recipients cannot reply to the group.

**Depth is bounded by the horizon** (§15.1) for the same reason permission scopes
are (§11.4): beyond it, the sender does not hold the topology to enumerate
recipients.

§9.2's threshold revocation remains the one genuinely multi-party primitive, and
it is a **counting** operation over independently produced signatures rather than
a coordinated one, which is why it works here and a coordinated one would not.

---

## 15. Control plane / data plane

Six message classes with different reach and different persistence — point-to-point
attestation is separated from ordinary attestation because its audience is one party,
not the usual participants-patrons-witnesses set:

| Class | Contents | Reach | Persistence |
|---|---|---|---|
| **Topology** | adoption (incl. Rotation/recovery and former transfer, §9.0), departure, disavowal, peering, node endpoint records (`wire-format.md` §7.6). **Resource registration is NOT in this class**, and is not a transaction at all — a signed entry goes to the hosting node and is answered on request, never propagated (§11.5) | **horizon** as the full transaction; **ancestors** as a memo only (§15.2) | stored within horizon, folded into aggregate state beyond |
| **Catalog** | resource registrations (§11.5) | **the hosting node alone**, answered on request | **current state only, and not archived** (§10): the host holds what it serves now, replaces it on re-registration, and keeps no history. Never propagated, and no copy exists to reconcile |
| **Attestation** | presence records (§8.1), verifier responses, other trust-bearing transactions | **pull, not push** | stored by participants, their patrons, and witnesses; fetched on demand by evaluators |
| **Attestation, point-to-point** | abuse reports (§11.6) | delivered to the addressed party only, never broadcast | stored by the resource owner alone. **Not** by patrons or witnesses, an abuse report is a private complaint, and giving it the generic attestation audience would make it the public accusation §6.2.2 declines to build |
| **Liveness / routing state** | heartbeats, route updates | horizon | **process and discard.** Keep the table, not the update history (as BGP keeps the RIB) |
| **Payload** | application data | point-to-point | **endpoints only.** Relaying infra nodes carry ciphertext they cannot read (§14.2). A patron sees plaintext only when it is itself the addressed endpoint |

**Propagation patterns are orthogonal to classes.** A message class says *what
reach* is appropriate; a pattern says *how* that reach is achieved. Two patterns
are in use:
- **Flood-within-horizon.** The default for topology. Encoded at
  `wire-format.md` §10.1: forward if and only if you stored it, duplicate-suppressed
  by `txid`, with no hop count and no acknowledgement.
- **Rootward memo.** A minified record of a membership change travels up the patron
  chain to its subnet's root (§15.2). This is what *ancestors* means in the table
  above, and it is the only thing that leaves the horizon.
- **Push near, redirect far.** The *delta* is pushed within horizon, where
  neighbours need it to keep routing; beyond the horizon the *state* is pulled on
  contact, via a currency-attestation query addressed using the anchor and path the
  introduction already carries. **Callers learn whether an identity is currently
  attested, and nothing about one that is not.** Used by key rotation
  (§9.0.2); a locator change beyond the horizon is **not** redirected and the
  stale holder re-resolves or is re-introduced (§12.3 Case 2). This is what lets
  rotation propagate like control despite being attestation-derived, without a
  class of its own, and it keeps standing assertions off the control plane entirely.

**Attestation is pull, and solicitation is made visible rather than mandated.**

You cannot stop a stranger from sending packets. What the protocol can do is make
unsolicited delivery *recognisable*: **an attestation delivery MUST carry the
request nonce generated by the evaluator that asked for it.** That is checkable
from the evidence, a recipient can see whether a delivery answers a request it
made, so it is a genuine MUST. Deliveries without a matching nonce are visibly
unsolicited and may be dropped, stored unweighted, or counted against the sender,
all at local discretion.

The reference client pulls, and does not push attestations.

This is the single biggest scaling decision in the design. Flooding makes
backbone traffic grow with total network activity; request-bound delivery makes
it grow with evaluation demand, which is expected to be occasional. The class
table below names attestation's present contents rather than defining it.

**That expectation is a constraint on implementations, not a prediction about
users.** A client that background-scores its neighbourhood, prefetches trust for
contacts it might meet, or runs periodic audits would invert the property and
make attestation traffic grow with population rather than with dealings. The
reference client must not do this, and a network of clients that do would need a
different propagation design.

### 15.1 Horizon parameters

**A horizon is a scope, not a shared region.** Each node's is centred on itself,
so **no two nodes with different positions have the same one** — my walk differs from
my subordinates' and from my patron's. There is no shared trust state anywhere in
this design, and *"inside the horizon"* means *inside mine*.

**Siblings are the case that shows the distinction.** They occupy the same position,
so their scopes coincide exactly — **and their trust pictures still differ**, because
each has its own history with users outside the subtree. Same scope, different
content. **A node's total trust picture is unique to it** — there is no tree-level
trust state for it to be a view of.

**A horizon is every node within a two-edge walk of you**, over adoption and sibling
edges. That is the whole definition; everything below is consequence.

| Distance | Who | At f = 10 |
|---|---|---|
| 0 | you | 1 |
| 1 | your patron, your subordinates, your siblings | 20 |
| 2 | your grandpatron, your patron's siblings, your grand-subordinates, your nephews | 200 |
| | | **221** |

**Sibling edges are what make this the right walk**, and they are not an addition to
the tree: siblings replicate each other, authorised implicitly by the patron's
adoption transaction (§3.3). Over adoption edges alone the same walk yields 122 and
drops both your patron's siblings and your nephews — the two groups the region exists
to include. **Peering edges do not count** (§6.3): a peering edge is ungoverned and
carries none of the subnet's authority, so it contributes flow without conferring
scope.

**Why two edges, and not one or three.** A patron's direct subordinates are a team it
built and maintains, with specialisation and balance in who does what. You need some
access to the complementary functions your patron's siblings are responsible for, and
you reach that part of the operation by dealing with **the node responsible for it at
the level where its graph intersects yours** — not by addressing the people working
under them. Two edges is exactly the reach that gives you the responsible party and
stops short of their staff. **Each generation offers a capability set to the
generation below it**: your patron's generation collectively serves you and your
siblings, and you and your siblings collectively serve the hundred nodes below you.

**Cousins are not excluded by a rule.** They are three edges away, as are nephews'
children and great-grandchildren, and nobody was tempted to write a rule excluding
those. Reaching cousins would make **the choice of patron meaningless** in shaping
what you see and who you deal with, which is the thing a third edge would cost.

**Membership is mutual, because graph distance is.** If you are in my horizon I am in
yours, with no rule required and none possible to get wrong — which is the practical
argument for defining the region by a walk rather than tier by tier. An enumeration
has to state the up-rules and the down-rules separately, and nothing forces them to
mirror; a walk cannot fail to. Every node you can see can therefore grant to you, and
§11.2's gate — access only within *"the region the owner's policy can evaluate"* —
never has to arbitrate a one-sided case.

- **h = 2.** Full topology storage — the 221-node horizon above, of which 110 are
  the node's own downline
- **h = 3.** Process-and-discard (~1,110 nodes of downline)

Control gossip volume scales as f^h, so fanout and horizon are coupled: any
increase in one must be paid for in the other.

**The horizon carries eight jobs**, and they move together if *h* changes:

| Job | Section |
|---|---|
| Topology storage and gossip volume | §15.1 |
| **Topology forwarding reach** | `wire-format.md` §10.1 — a node forwards what it stores, so `h_store` *is* the flood boundary |
| Permission scope evaluability | §11.4 |
| Fanout depth for group operations | §14.3 |
| Catalog **query** range | §11.5 — the nodes a client asks. The catalog is answered on request and never propagates, so this is a range of *asking*, not of replication |
| **Direct payload path eligibility** | §12.6.3 |
| **Resource access eligibility** | §11.2 — membership in the owner's Dunbar Org gates all resource access |
| **Prekey prefetch scope** | §14.2.4 — reusable material is prefetched org-wide so that fetching carries no intent signal |

Each was adopted independently and for its own reason, which is some evidence the
horizon is the right primitive, a boundary that keeps turning out to be the
natural one usually is. **But it also means *h* is now the most over-loaded
parameter in the design**, and a change to it for one reason silently moves seven
other things. Anyone proposing to tune *h* should be shown this table.


### 15.2 The rootward memo

**Full transactions flood within the horizon; a minified memo of every membership
change travels up the patron chain to its subnet's root.** That is
what *ancestors* means in §15's class table, and the memo is the only topology
object that leaves the horizon. Encoding: `wire-format.md` §10.2.

**A memo is a patron's statement about one of its own subordinate slots**: it names
the patron, the patron's position, which slot, when, and who is in it — with an empty
slot meaning a departure or a disavowal. It carries no address, **and no reason**: why
a patron disavowed someone is in-horizon state and stays there.

**Every field is one the patron has authority over.** Nothing in a memo comes from a
party that did not sign the transaction behind it, which is what lets a disavowal
produce a memo like any other: the subordinate neither signs it nor contributes to
it, because it reached the subtree on the patron's authority and ceases to exist from
the subtree's point of view when that authority is withdrawn.

**Restricted to membership operations, and the restriction is load-bearing.**
Adoption, departure and disavowal travel rootward; **peering does not.** A peering
record carries each endpoint's network point plus ASN (§6.3), and
C8 maps that composition to a natural person. A memo carries keys and positions, so
an ancestor accumulating them holds structure and no routable or identifying
information — and adding peering for symmetry would silently remove that property.
**It is a constraint on what may travel rootward, not a scoping convenience.**

**A memo never leaves its subnet.** Its anchor names the subnet and rootward travel
terminates at that subnet's root. That is what keeps the mechanism clear of §3.1.1:
nothing compares a node's binding in one subnet against its binding in another, and
no node adjudicates between them.

**What it buys.** Cycle prevention for the partial-information case (§6.2.5), and
detection of a node held at two positions within one subnet. Both are *detection*;
neither adjudicates. A node that detects a conflict fetches the underlying signed
transaction before acting, and what it then does is bounded by what it has authority
over — a patron may disavow its own subordinate and nothing more (§1.1, §6.2.2).

#### 15.2.1 The memo table, and what an ancestor comes to hold

**A node may accumulate the memos passing through it into a table of slot →
occupant.** Every memo from below traverses it, so the table's coverage is the node's
**whole subtree** rather than its horizon — and read the other way, by occupant, it
answers whether a node is held in two places at once.

**It is a RIB.** §15's liveness class already sets the rule — keep the table, not the
update history — so no new retention question arises.

**The table is optional and detection degrades gracefully.** §12.6.1 fixes required
state at parent plus ≤f children and calls anything beyond it an optimisation above
the floor. A tier keeping no table loses latency, not detection: the memo continues
upward and a tier that does keep one catches the conflict, at worst the root. **No
tier is load-bearing**, which is what makes it safe to specify without requiring.

**Two consequences worth stating rather than discovering.**

**First, a root comes to hold a key→position index for its subnet**, which is the
object §12.4 rejects, bounded to a subtree. §12.4 amends accordingly. The
disclosure *content* is unchanged — §12.1 already has a locator disclosing patron,
depth and subtree to anyone you introduce yourself to. What changes is that an
ancestor **stops needing the introduction**.

**That scope is accepted, and the reason is social rather than technical.** Trust is
bounded by the two-edge horizon, but **joining one subnet rather than another is a choice
to be visible to that subnet** — a company, a club, a party. Structural visibility to
the thing you joined is what joining means. The property that is defended is that
this stops at the subnet boundary, which §3.1.1 guarantees by construction.

**Second, the required state and the incentivised state diverge.** At the root of a
large subnet the table *is* a map of the subnet, which is exactly what §12.6.1 says a
node must never be *required* to hold. Permitted-but-incentivised is how such floors
erode, and an implementer reading §12.6.1 alone will not see it coming.

**The capability is directional, and worth naming as such.** Only ancestors
accumulate a table, so upper tiers hold a lookup lower tiers structurally cannot.
That is not authority — nothing in a table lets a root compel anything, and §9.2
withdrew the mechanism that would have — but it is asymmetric visibility in a design
whose §1 rests on there being *no one standing above the participants to appeal to*.
Registered as P35.

---

## 16. Trust model

### 16.1 Principle
Each node computes its own trust for every known node from the transactions it
observes, using the reference algorithm or a variant tuned to its application or
group norms. Trust fans out from a user's closest and best-attested connections;
a distant high-volume cluster is weighted only to the degree that it transacts
with users well-attested from *that user's* point of view.

**On joining a new tree, the archive can be scanned by the new patron and compared
against the identities that patron already knows of** — its own, not the tree's
(§15.1). **Transactions with unknown counterparties can be
ignored; those where the counterparty is known can contribute to the user's initial
trust state.**

**Stated permissively because nothing enforces it.** How a patron weighs a presented
archive is local policy (§1.1), and this is the shape the design expects rather than
a rule it imposes. **What it describes is how a joining member comes to have any
standing at all** in a subnet where nobody has met them.

**A patron following it weighs the intersection and nothing else** — records naming
unknown identities are not weighed less, they are not weighed. That is what makes
fabricated history worthless rather than merely discounted.

**This is the structural answer to fabricated history.** A manufactured
counterparty is unknown to the evaluator by construction, so records naming it fall
outside the intersection and contribute nothing. **An attacker cannot add to the
intersection without compromising someone the evaluator already trusts** — at which
point the fabrication is not what bought the access. Volume is irrelevant: a
thousand invented meetings and none intersect.

**So an archive answers specific questions rather than supplying a score.** *"You
say you were active in the Portland chapter; prove it"* is answered by records with
Portland people, and by nothing else. **The valuable evaluation is discontinuous and
local**, and how an adopter weighs what it finds is theirs — outside this
specification.

**What the protocol supplies is the sampling floor.** Verifier selection asks a
deterministic sample of prior counterparties to confirm a subject is who they say.
That is an anti-impersonation check, not the evaluation.

**The floor is holder-relative in what it proves, not only in what it checks.**
Candidate eligibility is structural — `wire-format.md` §5.4 admits a prior
counterparty when the record is canonical and its signatures verify, and nothing
weighs it — so volume worthless to an evaluator is **not** worthless to selection. A
subject who manufactures counterparties owns the population their own verifiers are
drawn from. What answers this is that **the party who selects is the party at risk**:
§8.1.2 has each participant select the other's verifiers, so the counterparty
enumerates that candidate set in order to choose from it and sees who is in it. A
sample drawn wholly from identities the selector has never heard of returns `match`
from strangers, which is worth what any fabricated history is worth to them —
nothing. The intersection argument governs the sample too — with selection by
recognition it *is* the selection rule (§8.1.2), and `wire-format.md` §5.7 states
the same property at evaluation time: weight comes from recognising responders,
so it is holder-relative all the way down.

**Credibility is constructive and partitioned by domain.** You start at zero in
every subnet and build by engaging there. There is no universal permanent record and
no cross-domain enforcement, so a subject appearing in a new subnet with no history
is a stranger rather than someone concealing one.

**This is why presenting a sparse archive is not an attack.** There is no quantity
being understated — an evaluator with a specific question either finds records
answering it or does not. **Volume proves nothing to an evaluator, so manufacturing
volume gains no standing**, and a subject who forks their archive has divided what
they can demonstrate rather than concealed a total. What manufactured volume *does*
buy is the candidate population above — and it buys it from the one party placed to
notice.

Trust rules are deliberately **not inherent to the network structure.** This is
an intended locus of adaptation and evolutionary pressure.

### 16.2 The reference metric must be flow-based

**Finding:** distance-decay metrics are exploitable and the naive tuning fails
catastrophically.

For weight = λ^distance, the total weight of an attacker's fake subtree is
λ^D · Σ(fλ)^k, which **diverges unless fλ < 1**. At a plausible-sounding
λ = 0.5 with f = 10 and a depth-6 fake subtree, the attacker's mass sums to
~19,500× the weight of one honest node at the same distance, the attacker
saturates the metric, and deeper is always better for them. At λ = 0.05 the
series converges to ~2 and the whole million-node subtree is worth at most twice
a single node.

**λ < 1/f is a soundness condition, not a protocol mandate.** Trust policy is
expressly local and pluggable (§16.1), so the spec cannot compel a foreign
implementation's choice of λ.

**Nothing publishes a policy, and nothing should.** A node's account of its own
policy is unverifiable, so a positive claim is exactly what an attacker would assert
and a mechanism carrying one would invite the mental model this design rejects —
that trust is a global quantity somebody can certify. **Per-observer trust means no
party ever consumes another's trust computation** (§16.1): a resource consumes its
own owner's access decision, and evidence is pulled and evaluated locally. There is
no consumer for a published policy.

**The soundness condition, stated as a fact rather than a rule:** a distance-decay
metric whose per-hop decay is not steeper than the reciprocal of the fanout
**diverges**, and is therefore exploitable by a deep fake subtree. That is a
property of the arithmetic, true whether or not anyone is instructed to avoid it.

**Stating it as a requirement would be a category error.** A MUST aimed at a
foreign implementation's private policy choice, which §16.1 puts beyond reach.

Note also that if fanout ever becomes variable, every deployed decay-based metric
silently becomes unsound — another reason the flow metric, which is independent of
*f*, is the reference choice.

**Better: max-flow / min-cut.** Every path into a subtree passes through its
root, so a subtree has vertex connectivity 1 to the rest of the graph. Under a
capacity-limited flow metric the entire subtree inherits at most what flows
through that one vertex, regardless of how many nodes it contains. Attack cost
becomes a function of the number of edges from honest nodes into the attacker's
region, not the size of the region. See **Raph Levien's Advogato attack-resistant
trust metrics.** It maps onto this structure closely. It is
also **independent of f**, which frees fanout to be chosen on social and
plumbing grounds.

### 16.3 Peering edges and trust capacity

**There is no trust ceiling to raise.** Standing is per-observer, not global.
Nobody anywhere is obliged to weight you more heavily because you have many peers
or many subordinates, each observer computes trust in the graph it can see, under
its own policy (§16.1).

A peering edge raises a region's cut, which is what bounds a subtree's trust claim
(§16.2), but **only in the view of an observer who can see that edge.** Peering is
an owner-signed record returned on request within horizon (§11.5), so it is visible
inside the two peers' horizons and **nowhere else**. An observer outside them
never learns the edge exists, and it conveys nothing to them.

**The concern is therefore local and targeted, and narrower than it first
appears.** It is not *"raise my standing"*. It is *"gain standing with a
particular victim by obtaining a technical favour from someone in that victim's
neighbourhood"* — which requires the attacker to already be near their target, at
which point the target can evaluate them by other means. What remains is that
*"want to back each other up?"* **reads as a routine technical request rather than
an endorsement**, so a peer may extend credit they did not intend to extend.

**Two framings to avoid.** Peering is not *the cheapest* route to standing — an
unattested adoption (§6.1.1) needs no meeting and no storage commitment. And there
is no *ceiling* to raise: that framing mistakes per-observer standing for a global
quantity.

**Mitigation, unchanged and still justified:** peering edges carry a distinct, low
default flow capacity, separate from hierarchical edges. The reason is the
endorsement-misreading above, which survives the correction. Raising the capacity
is a policy decision, upgraded by face-to-face attestation (§7) between the
peers.

#### 16.3.1 The min-cut bound is observer-relative

Not stated anywhere before, and it generalises the above.

§16.2 and §17.3 describe a region's cut as though it were a property of the graph.
**It is a property of the graph the observer can see.** Horizon-limited
propagation (§15.1) means different observers hold different edge sets, so they
compute different cuts for the same region, and an edge invisible to an observer
cannot raise the cut in that observer's view no matter what it does elsewhere.

Consequences:

- **The Sybil bound of §17.3 is per-observer**, like everything else here. An
  attacker's region is bounded by the edges *that observer* can see into it, not
  by every edge that exists.
- **Distant observers see less and therefore bound harder.** Invisibility is
  conservative: unseen edges cannot inflate a claim, only fail to support one.
  This is the safe direction for the error to run.
- **An attacker must therefore work per-target**, acquiring visible edges inside
  each intended victim's horizon rather than accumulating edges globally. That is
  more expensive than the global reading suggests, and it is another
  instance of §1.1: what an observer cannot see cannot bind them.

### 16.4 The pluggability tension

**An adversarial reading sharpens this into a concrete attack.** A well-funded
operator's best move is **not** to build a giant fake region — against the reference
metric that region is nearly worthless regardless of size. The move is to **find
evaluators whose policy does not respect the flow bound**, and present the fake
region to those.

**Nothing publishes a policy, so an attacker cannot read one** (§16.2). What they
can do is **try**: present the region and see who accepts it. That is slower and
noisier than reading a declaration, and it is the reason no declaration exists —
but it is not prevented, and **an attacker who tries widely enough learns the same
thing.**

**The asymmetry is the durable part.** The attacker chooses how many evaluators to
approach; no individual evaluator chooses which attacker approaches them.

**Whether protocol conformance should imply anything about Sybil resistance is a
question this design answers "no"**, deliberately rather than as a side effect of
pluggability. **The only lever is the reference implementation** (below): what most
deployments run is what most attackers face.
If security lives in the metric rather than in the structural caps, then the
*protocol* cannot be secure — only individual policies can be. A node that picks
a plausible-looking distance decay is exploitable for a few hundred dollars a
month while believing itself protected by the fanout rule.

**Proposed mitigation:** ship a flow-based reference implementation as the
default, publish the λ < 1/f criterion prominently, and provide a conformance
test that reports a policy's resistance bound so anyone tuning their own can see
what they have given up.

### 16.5 Decay on inactivity

Expiry of old standing is a **deliberate decay function**, not an unfortunate
side effect of retention limits. It buys two things:

- **Zombie keys lose value**, so a retrieved or stolen key ages out of
  profitability rather than conferring standing indefinitely.
- **Parked alternate identities cost upkeep**, so isolating undesirable
  behaviour in a dormant identity across sub-networks stops being free.

**Critical distinction: facts do not decay, standing does.** This is a *schema*
rule and therefore enforceable: a presence proof between two people who genuinely
met is permanent and immutable evidence (§8.2), and its age and activity context
are visible. What may decay is *derived* standing — down-line credit, reliability
weighting, aggregate scores.

**The reference policy** does not let a low-activity user's real relationships
evaporate, decaying derived standing while leaving the underlying evidence at
full weight. Another policy may choose differently; decay is computed locally
(below) and cannot be mandated.

This unifies with retention (§7.5.1): a presence proof whose verification
substrate has expired is naturally unverifiable, so the retention window is itself
a decay floor. Same mechanism from two angles. Decay is a
policy parameter computed locally, so it requires no protocol change.

### 16.6 Reliability weight is not social trust

Infrastructure status is open to anyone, no restriction on running a node. An
infra node is required to exceed **110** users (f = 10, L = 2, §3.3). But any user
may launch a
server instance and sign it with their key; having one does not cause or require
a hundred people to follow them. Cost is a low-spec VM plus a static IP, roughly
$20/month at retail, a figure that only holds because payload takes the direct
path where it can (§12.6.3); an infra node relaying all traffic for its whole
subtree would be dominated by bandwidth — comparable to Discord Nitro or an X blue check,
and increasingly launchable by an agent that encapsulates cloud provisioning
behind a single recurring-payment authorisation.

This defuses the hub-concentration concern, but carries a caution drawn from the
same analogy: **X's blue check destroyed its own signal value the moment it
became purchasable**, because a marker meaning "verified person" came to mean
"paid twenty dollars".

**Therefore, keep the channels strictly separate in the evidence schema:**
- **Infra status raises reliability weighting.** Expected to answer queries,
  hold replicas, remain reachable.
- **The two are separately typed in the evidence schema, and that separation is
  the enforceable part.** Money buys uptime. It does not buy the presumption of a
  trustworthy human, and the reference policy never converts one into the other —
  but no protocol rule can stop a foreign policy from doing so. What the protocol
  guarantees is that the distinction remains *visible*, so a policy that collapses
  the two has done so deliberately rather than by accident, and others can decline
  to rely on its evaluations.

Note also that attackers do not pay retail: running many instances per VM, the
marginal cost is a static IP plus a compute slice, perhaps $5–7. Honest
operators pay retail, attackers pay wholesale, the same asymmetry as §17.2,
smaller in magnitude.

### 16.7 History portability
Every node, including light clients, keeps an archive of its own transactions
plus past counterparties' signatures. Because transactions are self-signed and
countersigned, history remains independently verifiable after a move — only
proximity is lost, not evidence. This is what makes exit a real right rather
than a formal one, and should be an explicit design commitment.

**Portability has a destination-dependent privacy cost** (§19.8 C4): entries
opaque in one subnet may name known people in another, so *where* an archive is
presented changes what it discloses.

**This is the one exchange with a use for location** (§8.1.1), and therefore the one
where the presenter has a choice to make. A prospective patron running §7.7's
impossible-travel check needs the geohash series; a presenter withholding it is
declining to offer that evidence rather than concealing a defect, and the withholding
is visible either way. Everything else disclosable — retention, client integrity,
capture parameters, proximity — has no consumer here.

**Presentation is a single head txid** (`wire-format.md` §4.1 field 7), from which
the patron walks the chain backward and fetches what it wants
(`wire-format.md` §7.9, archive fetch). **The patron chooses its own depth.** The presenter picks the head and
cannot control how far back the recipient looks, so the party extending credit
decides how much evidence it wants. On joining a new tree, the node presents a
**contiguous run** of its archive to the new patron rather than the whole thing —
**not an arbitrary subset**, since §10's chain makes excision impossible and truncation
the only available edit (§10.1); the patron verifies signatures on what it is
shown and compares it against the identities that patron already knows of.
Transactions with unknown counterparties can be ignored; those where the
counterparty is known can contribute to the user's initial trust state (§16.1).

Open tension: an archive presented *lacking* previously-seen transactions may
itself be read as evidence of deviousness. This probably pushes users wanting to
participate in rival networks toward creating separate identities — acceptable
for v1, given multiple-identity support is deferred.

---

## 17. Security analysis

### 17.1 Standing comes from edges, not from nodes

**Counting identities prices nothing an attacker wants.** Standing
comes from the edges a given observer can see into an attacker's region (§16.2,
§16.3.1) — not from how many identities that region holds. A fake subtree is cheap
or dear depending on how it is built, and either way it buys **appearance and
deniability, never trust**: nobody extends anything to a node on the strength of its
subordinate count.

**So the cost of building one is capacity arithmetic, not a security parameter**, and
it belongs with the other capacity figures in Appendix B.2. It is recorded there
because operators sizing infrastructure need it, and it is recorded *only* there
because pricing an attacker's tree invites the reader to treat the price as a
defence. §17.3 has the defences.

### 17.2 Topology cannot provide Sybil resistance
**No topology rule can, because the protocol cannot tell the two populations apart.**
Any rule that makes an attacker's infrastructure expensive makes an honest operator's
expensive by at least as much, so tightening one tightens the other and nothing is
gained.

**It is worse than parity.** The attacker builds a perfectly packed tree while real
social graphs are sparse and lopsided, so the attacker reaches the theoretical floor
and honest operators never do (A10).

What f actually trades is shape:

| f | Depth @1M | Max path | Sibling factor |
|---|---|---|---|
| 3 | 13 | 26 hops | ×3 |
| 5 | 9 | 18 hops | ×5 |
| **10** | **6** | **12 hops** | **×10** |
| 20 | 5 | 10 hops | ×20 |

**Conclusion: f is a plumbing and social parameter, not a security parameter**
(§3.2, §3.3).

### 17.3 What actually provides Sybil resistance
Three independent mechanisms, none relying on topology rules:

**All three bounds below are per-observer** (§16.3.1): an attacker's region is
bounded by the edges *a given observer* can see into it, not by every edge that
exists, so an attacker must work per-target rather than accumulate standing
globally.

1. **Face-to-face attestation** makes *identities* expensive. Proof of presence
   is the one resource an attacker cannot parallelise. This is the strongest
   leg; specified in §7. Note it is a *cost*, not an unforgeable
   primitive — bilateral collusion defeats it, and the flow metric is what
   bounds the resulting damage (§7).
2. **Static routable addressing** makes *infrastructure* expensive and visible.
   Routable IPv4 is genuinely scarce and metered — AWS charges $0.005/hour per
   public IPv4 address, roughly $3.65/month, indicative of major cloud pricing
   rather than a universal market rate; 1,000 addresses is a real bill, and
   unlike RAM it cannot be optimised away.
   - **IPv6 dissolves this.** A /64 holds 2^64 addresses, and whether one costs
     anything is **provider policy, not a property of IPv6** — commonly free, but
     not guaranteed and not generalisable. The answer, when IPv6 is admitted, is
     to count by **routable prefix** rather than address; the unit, encoding and
     prior art are recorded with the deferral (§4).
   - **Expose the ASN** as an attribute of the infra attestation.
     Concentration (1,000 nodes in one ASN) is observable and is a signal
     policies can weight — a visible signal, not a trust input.
     **The signal runs one way.** Concentration in one ASN is evidence of
     concentration; ASN or region *diversity* is not evidence of independence.
     Both describe routing and geography rather than the entity subject to one
     legal order, and one provider can present many of each. The field is also
     optional and self-asserted (`wire-format.md` §4.4) with no IP-to-ASN
     validation specified. Read it as a concentration detector and never as an
     independence proof — the difference is what §18.1's compelled provider
     turns on (§1.2.3).
   - Decision: demand IPv4 for now and take the security as a bonus, while
     making no engineering decision that precludes IPv6 later.
3. **Flow-limited trust** bounds what any single-entry region can claim
   regardless of its size (§16.2).

## 18. Accepted risks

### 18.1 A compelled cloud provider

- **Provider concentration is the highest-ranked systemic risk, and rests on an
  unsupported premise.** A state actor compelling one cloud provider ranks above
  endpoint theft, because §3.3's expectation that
  infrastructure concentrates in a handful of clouds turns architectural
  disaggregation back into bulk access. **That expectation is itself unsupported**
  (§20.1), so the ranking inherits its uncertainty: if deployment is genuinely
  provider-diverse, this risk drops below endpoint compromise.

  **This is the adversary the design does not defend against, and §1.2.3 says so
  directly**: the threat model of an anonymity network does not apply here. A state
  compelling a provider is §1.2.2's third class operating below the whole
  architecture at once. What follows describes the shape of that failure; none of it
  is a claim that the failure is survivable.

  **End-to-end encryption does not confine it to metadata.** Payload is encrypted
  **to the addressed endpoint**, and a provider hosts endpoints — so §14.2's own
  table gives the plaintext away: a patron reads leaf-to-patron traffic *as the
  addressed party, not a relay*; a hosting node reads a hosted resource's requests,
  which it parses and re-serialises to insert the credential; a proxying node reads
  proxied ones. What encryption does protect here is **leaf-to-leaf traffic in
  transit and brokered resources**, where the node authenticates and hands off
  (§11.7). Everything terminating at the provider is readable there.

  **Nor is the exposure limited to reading.** An infra node signs **unattended**,
  because Appendix A requires it to: countersignatures, `SubtreeAck` decisions, currency
  attestations, disavowals, peering records and topology propagation all proceed
  without an operator present, so the signing capability is on the machine. One order
  therefore reaches the **genuine protocol authority** of every hosted instance at
  once, with no separate social compromise per operator. The item below names
  resource-access forgery; that is one consequence among these rather than the
  boundary.

  **And the observation boundary is the provider, not the node.** Where a participant
  holds separate identities in two subnets whose serving nodes are hosted together,
  one observer sees both mutually-authenticated attaches (`wire-format.md` §9.1) and
  can join them on device and network signals. C14's withdrawal does not cover this:
  that reasoning turned on the new identity appearing under *a different serving node
  that sees only one*, which is sound against a node and says nothing about what sits
  beneath several.

  **What survives.** Participants' own signatures remain unforgeable, so the actor
  cannot manufacture a presence record for anyone it has not separately compromised,
  and §6.4's ungated proof of presence means it cannot suppress an honest ceremony
  either. Sealed captures sit on light-client devices and are not reachable from
  infrastructure (§7.5.2).

  **And a compromised node is not on the direct path.** Payload between two clients
  inside the horizon goes point-to-point, and §12.6.3's table gives who sees that flow
  as **nobody** — so an actor holding an infra node's key sees the connection *setup*
  and nothing after it. This bites hardest for the node's **own operator**, whose
  light client is a different device (§23.3): traffic addressed to them as a
  participant never reaches the instance the actor controls, and cannot be read or
  even measured there. Forcing the relayed path gains nothing either, since a relay
  carries ciphertext (§14.2). **The actor's reach is prospective rather than
  retrospective**, which is the part *"bulk access"* overstates.

  **The residual there is impersonation, not interception.** An instance holds the
  same key as its operator's other devices (§23.3), so an actor with it can present as
  that operator in **new** exchanges and become the endpoint legitimately. What it
  cannot do is reach a session it was never on the path for.

  **And impersonation is a different class of operation from collection.** Reading
  direct-path content means *acting as* the operator toward people who know that
  operator — directly, or through someone who does. That is social engineering against
  a graph built out of face-to-face acquaintance, and it **risks the subverted node
  the moment anyone notices**. So the position is not that this adversary is stopped;
  it is that **what the adversary has to spend changes**. Passive bulk collection is
  what compelling a provider is *for* — scalable, deniable, invisible to the people it
  collects from — and the direct path denies it that for content, leaving impersonation
  instead: per-target, high-commitment, and self-burning when detected. **This is
  §1.1's principle reaching the case the design does not claim to cover** (§1.2.3):
  the attack is made visible and expensive rather than impossible, which is the same
  answer the design gives everywhere else.

  **The capability also devalues what it collects.** §1.2.1 already treats cheap
  fabrication as a privacy property — *"Sybil attackers inadvertently contribute to
  the deniability of every record"* — and an actor able to act as any hosted operator
  **enlarges that discount rather than escaping it**. A surveilled record naming an
  operator becomes deniable in the way a synthesised subnet is, because the actor's own
  capability is the standing alternative explanation. The asymmetry then runs as
  §1.2.1's second property says: someone who has met you infers your identity cheaply
  from personal knowledge, while a remote examiner must pay for either an impersonation
  operation or an evidence chain that survives due process. **This is the Potemkin
  acceptance below in a different costume** — an expensive fake that makes the
  surveilled record less useful to whoever built it.

  **§1.2.1's boundary still holds**, which is what keeps this short of a claim that
  nothing can be proved. Forging evidence about a *specific real person* needs their
  participation, and a presence record needs a live counterparty, witnesses and
  verifiers who were there (§7.1). An actor holding an operator's key can sign as
  them; it cannot put them in a room.

### 18.2 A compromised infrastructure node

- **A compromised infra node can forge its subordinates' resource access.** A
  resource trusts the node's assertion of who holds which role (§11), so a
  compromised node can mint any principal with any role. This is the trust a
  corporate gateway holds and is expected, but it is a **new concentration**, and
  it should be named against the design's careful claim that a patron cannot forge
  its subordinates' *transactions*. It can forge their **access**.
- **A replayed rootward memo can cost one edge, without prejudice.** A memo
  describing a patron's *current* slot state, captured below and re-injected upward,
  matches that patron's own row and is indistinguishable from a memo that came back
  around a cycle (`wire-format.md` §10.2). Stale replays are stopped by the slot's
  timestamp at the first table-holding hop; this one is not. **Bounded on three
  sides**: the injector must sit at or below one of the detector's direct
  subordinates, the edge severed is the one that handed the memo over, and the
  disavowal is reason code 5 with re-adoption available. **Accepted rather than
  closed**: a freshness nonce would put a second clock on an unsigned object, and the
  three bounds hold without one.

  **The economic argument is withdrawn.** This was accepted as an attack *costing the
  attacker more than the target*, which assumes the injector owns the edge it loses.
  A state actor operating a commandeered tenant's instance pays with somebody else's
  relationship, so the cost is externalised, and re-adoption turns the result into
  repeatable churn rather than a one-time price. The structural bounds are
  unaffected. The reason for accepting is the item above — §1.2.3, this is the
  adversary the design does not defend against.

### 18.3 A stolen key, and a stolen device

- **A stolen key can exhaust a sequence counter, and a patron is what undoes it.**
  One record at the top of the range leaves no successor, and an equal `seqno` carrying
  different contents is malformed rather than a tie — so the identity could otherwise
  never publish a position or endpoint change again, which §6.2.1 names as fatal on its
  own terms. **Closed rather than accepted**: the counter is half of a `{series,
  counter}` pair, series are arbitrary and unordered, and advancing to a new one is
  countersigned by the patron (`wire-format.md` §4.6). A thief holding only the subject
  key cannot make that transaction, and cannot pre-poison the space either — with no
  global registration of series, it would have to exhaust 2³² **and reach every party
  it wanted to block with every one of them.** Residual: the patron must not countersign
  a reissue for a thief, which is the same social check adoption and recovery rest on.
- **A stolen device composes into false presence evidence and fraudulent recovery.**
  Theft grants the key, the archive and the capture seeds at once, and the pieces
  compose further than any of them registers alone. With a colluding counterparty, the
  thief runs a real ceremony that real witnesses observe — witnesses attest that the
  protocol ran and that two humans responded, not that the queried profile came from
  the live participant's face (§7.1). The colluder's client substitutes a profile,
  the stolen key countersigns it, and **the thief signs the record whatever comes
  back**, since the record finalises with whatever responses arrived and absence
  blocks nothing (`wire-format.md` §5.5) — and the subject-side refusal §7.4.1 relies on is the
  thief's to make. The colluder is then a prior counterparty, positioned to supply the
  recognition half of §9.1's recovery. **Two bounds hold**: honest verifiers'
  signatures are unforgeable, so adverse results are visible to anyone who weighs
  them, and §16.2's flow metric caps the successor's standing at what the colluding
  parties can carry rather than inheriting the victim's. Accepted — the answer to a
  stolen key is rotation, and what this describes is the cost of the window before it.
- **A stolen device becomes a biometric collector when a counterparty next meets
  someone.** Its sealed captures of past counterparties stay ciphertext, and §7.5.2's
  seed release is per query and direct to each selected verifier — there is no standing
  grant, and no state a subject enters that opens their likeness on every device that
  ever held it. What theft changes is that a **previously compliant holder** can be
  selected as a verifier later and receive that seed through ordinary automatic
  traffic (Appendix A), with the depicted person never asked. **The exposure needs a
  confluence**: the counterparty must run a new ceremony inside the 730-day window,
  *and* the compromised identity must be among the selected verifiers — roughly q/d
  per ceremony. Records older than the window are safe outright, because the subject
  will not release the seed for them. Real, and hard to target: no captured device
  releases all of its archive this way, and most release none.

### 18.4 Eclipse, and occupying a region

- **Patron eclipse of a new joiner.** An attacker who volunteers to be someone's
  patron controls their view from day one.

  **The tempting justification — *identities are cheap and there is no asset to
  steal* — does not hold.** "No tradable token" is true and irrelevant. **The
  asset is the victim's authentication decision-making environment.** Who they believe
  exists, which key represents a counterparty, which introductions reach them,
  which social evidence is reachable at all. This network's stated product is a
  DNS-and-SSL replacement (§1); corrupting a user's naming and authentication
  view *is* theft of the primary asset, and an attacker need not take anything
  from the victim to profit from it.

  **§12.4 amplifies it.** No cold lookup means the victim has no independent
  resolution path against which to notice that people or whole regions are
  missing from the view their patron supplies. A property adopted for privacy
  makes eclipse strictly worse than it would be in a network with global
  resolution. That interaction was never noted.

  **Face-to-face grounding does not repair it from day one**, which is precisely
  the accepted case. It helps only once the victim has an independent physical
  relationship, and the attacker controls which introductions they receive
  before then.

  **The actual mitigation is subnet plurality** (§3.1.1), previously framed only
  as an unavoidable side effect of partitionability and free exit.

  **But plurality must already exist to defeat an eclipse cleanly.** Establishing a
  second patron *while eclipsed* means reaching one, and reachability is what the
  eclipsing patron mediates. The escape is real — proof of presence is ungated
  (§6.4), so any physical meeting produces evidence the patron cannot suppress —
  **but it costs a physical meeting rather than a message**, paid at the moment the
  user is least placed to recognise the need. The reference client should encourage
  a second independent adoption early, while it is cheap. **It is also
  the structural answer to eclipse**: a user adopted in two independent subnets
  has no single party controlling their view. The reference client should
  therefore treat "every contact you have came through one patron" as a
  condition worth surfacing, and encourage a second independent adoption early.

  Residual risk after that: a user who has only ever had one patron is eclipsable
  and the protocol cannot prevent it. Accepted, but for this reason rather than
  the old one.
- **Potemkin networks / one operator occupying a region.** Accepted: the value
  of the network is the activity it facilitates, and a fake region cannot
  manufacture standing across an honest cut (§16.2). Arguably *strengthens* the
  design's purpose, since surveilled metadata becomes less useful when
  face-to-face interaction is what distinguishes true from false participation.

  **The reach is not zero, and the earlier reason — that a fake region "harms
  nobody who is not engaging with it" — was wrong.** Witness eligibility is
  not a function of standing, so a node placed by a single boundary adoption,
  valid without a proof of presence (§6.1.1), sits in a neighbourhood *someone
  else's counterparty* will nominate from. The harm then lands on a party that
  never engaged with it: the C1 and C2 disclosures of §19.8, and a withheld
  signature (§8.1.2). What bounds this is the nomination heuristic of §7.1.1
  and not the flow metric — the operator must occupy enough independent branches
  to catch a spread selection, which is a far larger region than one adoption.
- **Collusion rings between real distant nodes** mutually transacting to
  manufacture the cross-distance history a distance metric rewards. **No
  structural signature.** Which is the substantive problem; the cost of
  recruiting and running such a ring is unmodelled. Countered by corroboration (weight a counterparty by
  whether *your* neighbours also transact with them) and by §7.

### 18.5 What a patron can refuse

- **Patron censorship.** A patron can refuse to countersign. Unilateral departure
  plus adoption elsewhere is the escape hatch. Consequence: patron reputation can evaporate in a day
  when a down-line flees. Treated as intended evolutionary pressure.
- **Retaliatory disavowal, which the escape hatch does not answer.** The item
  above addresses a patron that *withholds*; it does not address one that **acts**.
  A patron seeing a departure can sign a disavowal carrying a with-prejudice reason
  code (`wire-format.md` §4.3) and propagate it first, and **nothing orders the
  two**: a departure advances the departing node's counter, a disavowal advances the
  patron's and carries no subject counter, and timestamps are signer-controlled and
  never checked against a clock (`wire-format.md` §2). The stated cost — losing a subordinate — is
  **zero in exactly this case**, because the subordinate is already leaving and the
  slot refills.

  **What limits it is visibility, not enforcement**, which is §1.1's usual answer. An
  observer holding both objects holds a departure and a with-prejudice disavowal
  minutes apart, and **that pair is itself evaluable**: a contested exit reads
  differently from an ordinary disavowal, and the reference policy weights it as
  evidence about the relationship rather than about the departing node. Nothing
  compels any evaluator to read it that way, and the record remains durable
  regardless.

---

## 19. Privacy analysis

Privacy is analysed here under LINDDUN's seven categories, and **under composition
rather than mechanism by mechanism** — §19.1 states why.

**Numbers are not reused.** Withdrawn and closed findings are recorded in
`change-log.md`, so a citation to a missing number resolves there rather than
silently to a different finding.

**Read this register against §1.2.1's baseline-exposure floor, and against its
three surveillance classes.** A finding's severity depends on which class can use
it: most of what an endpoint attacker learns is unusable to anyone needing their
conclusion to survive examination, and most of what survives examination is
available to that class by other means.

**Read this register against §1.2.1's baseline-exposure floor.** A finding that an
attacker holding a device sees the local archive is not by itself a defect — the
question is whether it costs more than reading an ordinary phone. Findings that
clear the floor are recorded because they are real, not because they are failures.

### 19.1 The composition invariant

**Privacy properties must be evaluated under composition of every artifact
available to the same observer, never artifact by artifact.**

This design minimises consistently and well at the level of individual objects:
coarse geohashes rather than coordinates, fuzzed profiles rather than templates,
biometrics kept off the network entirely, attestations pulled rather than
flooded, locators disclosed only on introduction. **Each of those is sound and
none of them compose.** A legitimate evaluator, patron, witness, compromised
endpoint or state actor can often acquire several minimised artifacts whose
intersection recreates the sensitive fact each minimisation was meant to prevent.

§7.2 states exactly this for biometrics. Fuzzing does not survive combination
with timestamp, location and graph position, and then applies it nowhere else.
It is a general rule.

**Worked example.** Presence record A says key K met X near region R, witnessed by
two members of P's subtree. Record B says K met Y two hours later, witnessed by two
more. P's subtree corresponds to a known organisation. None of the three identifies
anyone; the intersection gives identity, affiliation, movement and social
relationships.

**Practical consequence for future work:** any proposal to add a field, expose an
attribute, or publish an attestation must be assessed against what an observer
holding it *plus everything else they can legitimately obtain* would learn. A
per-artifact privacy argument is not an argument.

> **Vignette V6 — What you are already giving away.**
> This protocol exists to structure and empower real-world social relations, and
> it accepts leakage of things about you that are already obtainable elsewhere,
> where accepting that buys something the network needs.
>
> Every data point here is obscured and secured as far as the mechanism allows.
> Enough of them together defeat those measures, and we say so rather than
> pretending otherwise. But consider what someone who knows enough to ask can
> already find: where you work, who your friends are, what you look like. Much of
> it you published yourself. More of it sits in timestamped photographs on devices
> you do not control. Anyone who knows that Alice works with Bob, that Bob is
> married to Claire, and that Claire is friends with Derrick can reconstruct most
> of what a composition attack on this network would yield, using a few social
> platforms and a public directory.
>
> What those platforms do not offer in exchange is a way to bring modern computing
> to bear on what Alice, Bob, Claire and Derrick are actually trying to do
> together. **You can already communicate with your social network. This is a
> proposal to help you coordinate and interoperate with it.**
>
> **Two honest limits on that argument.**
>
> *Authentication and durability still differ, even where the facts match.* A
> LinkedIn claim is unverified and deletable; platforms lose data and people edit
> it. A presence record is cryptographically authenticated and immutable for
> decades. An attacker who obtains records holds better evidence than one who
> scrapes profiles, a real difference, though a narrower one than "the facts are
> already public" suggests, and independent of how hard the records were to get.
>
> *Access cost, by contrast, runs the other way here* (see §19.1.1). This network
> is disaggregated by construction; the platforms it is being compared against are
> aggregation services by design.
>
> *The baseline is not universal.* It describes a median user in a country with
> mature social platforms. It is false for someone who deliberately minimised their
> footprint, and dangerously false for an activist, a dissident, someone under a
> hostile government, or a person hiding from a former partner, for whom
> composition risk is a **new** exposure rather than a matched one. §4 already
> declines to be a universal architecture; this is that limitation, stated where
> it bites.
>
> *Illustrates assumption **A12** (Appendix B.2), unvalidated. **This vignette must not be
> used to dismiss a composition finding.** The trade it describes is a reason to
> accept a known cost with eyes open, never a reason to stop counting.*

#### 19.1.1 What limits composition: disaggregation by construction

§19.1 catalogues composition risk. It would misread the design to stop there,
because the architecture already resists composition in ways the comparison
classes usually invoked do not.

**There is no store to walk.** Attestation is pull-only and presence records are
never flooded (§15, §8.2): they are held by the two participants, their patrons and
the witnesses, and fetched on demand. Topology is horizon-limited (§15.1). The one
globally cached structure, the anchor table, carries key hash, address, subtree
size and sequence — **no social content whatsoever** (§12.2). An attacker seeking
composition must penetrate neighbourhoods **individually**, and the network-layer
data they find is sparse.

**Transmission to aggregators is never a default behaviour.** No component pushes
anywhere central. That is a structural property, not a policy promise.

**The comparison classes usually reached for are the opposite of this.** Ring's
police-partnership programme and Flock's business model are *designed* aggregation
and push to law-enforcement; the fragmentation is at the camera and the
aggregation is the product. Real-name platforms are built for maximum PII
collection and distribution and are near-mandatory for ordinary life in wealthy
countries. **Neither is an example of high-friction fragmented data**, and citing
them as one inverts the comparison this design should be measured against.

**What remains true:** some nodes and subnets will be bad actors, and a
sufficiently resourced attacker can penetrate several neighbourhoods. Composition
is a real risk to mitigate wherever possible (§19.1). It is a risk against an
adversary who works for it, not a property of the system at rest.

#### 19.1.2 The network layer is scaffolding

**What this network exists to carry is point-to-point communication between people
who have established connection, identity and trust through the network layer.**
The control and attestation records are the scaffolding that makes that possible;
they are sparse and deliberately so.

Which is why the following mattered: **end-to-end payload encryption was, for a
time, unspecified.** It is now adopted rather than designed here — PQXDH and the
Triple Ratchet, §14.2.4 — with integration decisions open and **no implementation
yet**, which is the residual risk P12 records. The analysis below describes the
exposure that remains until one exists.

§12.6.3 routes payload client → own serving infra → recipient's serving infra →
recipient, with PQ KEM protecting each *hop*. Hop encryption terminates at each
infra node, so **before §14.2.4 was adopted, both serving infra nodes saw payload
plaintext**. §14.1.6 lists "end-to-end encryption of queued payload" among things
the queue policy must settle, which confirms it is settled nowhere.

§12.6.3 already accepts the patron as a *metadata* chokepoint. It was never
intended to be a *content* chokepoint, and the distinction has not been written
down anywhere.

**Required:** payload MUST be encrypted end-to-end to its addressed endpoint,
independently of hop transport, so nodes acting as *relays* carry ciphertext they
cannot read. The endpoint is not always another person see **§14.2**, which
separates the leaf-to-leaf, leaf-to-patron and leaf-to-resource cases and the
patron's two roles. This is enforced at the source by simply not sharing the key,
and therefore a genuine MUST (Appendix A). It also disposes of most of §19.5's queue
question, a queue holding ciphertext is a much smaller problem than one holding
messages.

**Construction: §14.2.4**, which adopts PQXDH for asynchronous key agreement and
the Triple Ratchet for session secrecy rather than designing either here. Note the
endpoint may be a **patron** or a **resource** rather than a person, so the
requirement is "encrypted to the endpoint", whoever the endpoint is, and both of
those cases are already covered by the §14.1.1 transport handshake, since such an
endpoint is online by definition.

### 19.2 The verifier and witness social graph

A presence record discloses more than the encounter it records. **It names a
sample of the subject's *prior* counterparties** (the verifiers, who can only be
verifiers because they met the subject before) **and a set of witnesses drawn
from both participants' neighbourhoods.** Correlating verifier lists across
several records reconstructs the subject's historical meeting graph; correlating
witness sets maps neighbourhoods.

Both properties are load-bearing and were adopted for good reasons — verifier
responses sit in the record so a colluding participant cannot suppress a negative
(§8.1), and cross-nomination stops a *participant* choosing friendly witnesses
(§7.1). **Neither was assessed for what it leaks.** The anti-suppression
requirement in particular was described as strictly dominating the alternative;
under composition it is a trade, not a dominance.

**This is a genuine tension with no clean resolution in the current design.**
Anti-suppression requires the responses be visible; visibility exposes the
historical relationships. The direction if it needs solving is **aggregate or
threshold signatures.** Proving that *k* of the deterministically selected
verifiers answered `match` without naming them, but that sacrifices the
"absence of an expected verifier is visible" property (§8.1), which is doing real
work. **Open.**

**The selection input reaches a different party than the responses do.** To select
the other's verifiers a participant needs a candidate set, and the subject supplies it
as records rather than names, since `wire-format.md` §5.4 counts only what verifies.
§8.1.1's sweep already records the fact — the selection row reads the handed
bundle; what was never priced is who receives it —
the person in front of you rather than a later evaluator.

**The disclosure is elective, and the pressure is what makes it wide.** Nobody walks
another party's archive (§8.1.2), so a subject hands over what it chooses. But the
incentive runs one way: a bundle holding nobody the selector recognises establishes
nothing for them (§16.1), so being believed means showing counterparties in common —
and each record shown carries its witnesses, verifiers and times along with it, none
of them withholdable, since they are body fields under signature. Registered as
**P37**, and priced as a pressure rather than a compelled disclosure.

**It is the same trade as the paragraph above, one step earlier**, and has the same
non-answer. Any scheme that hid the candidate population from the selecting party
would also stop them checking the selection — the check §8.1.2 requires *before
signing*, and one of the few protecting them against the person in front of them.

### 19.3 Selective disclosure — adopted, and what it does not cover

**Specified at §8.1.1 and `wire-format.md` §4.5.1.** A presence
record's disclosable fields are committed as salted digests, so a holder can present
the record to a recipient without the fields that recipient has no use for, and the
recipient still verifies against the same signature.

**What it covers:** location evidence, retention, client integrity, capture
parameters, proximity channels. Ten of the eleven exchanges that
transmit or evaluate a record read none of them (§8.1.1).

**What it does not cover, and this is the part to keep in view.** It cannot hide the
signer set, because `kid` is on the envelope rather than in the body, and because
signer role is inferred by comparing `kid` against body fields. **So P2 and C2 stand
untouched** — the verifier and witness graph is the disclosure §19.1's composition
argument keeps arriving at, and this is not the lever for it. §19.2 records the only
direction that would be, and what it costs.

**The construction is a flat list of salted digests, not a Merkle tree** — at nine
leaves a tree buys nothing and adds odd-node handling and the duplicated-node
second-preimage class (`wire-format.md` §4.5.1).

**Not to be confused with record-level disclosure, which is constrained.** §10's chain
makes *which records* you show a matter of prefix only — arbitrary subsets of an
archive are not constructible, precisely because that was an attack. This concerns
*which fields within a record* you show. The two are independent and point in opposite
directions: records constrained, fields liberalised.

### 19.4 Findings requiring action

**Current findings only.** Withdrawn, closed and not-a-finding entries are in
`change-log.md`, so this register states what is open or accepted rather than the
history of what was asked. **Numbers are never reused**, so the gaps below are real
and a citation to a missing number resolves there.

| # | Finding | Severity | Status |
|---|---|---|---|
| P1 | Presence-record composition — durable correlatable tuple of identity, time, social graph and geography | **Medium, reduced** | **Graph position is gone**: the participant locator is removed (§8.1), so records no longer trace a trajectory. **Geography is withholdable** from ten of eleven exchanges (§8.1.1). **Identity, time and the social graph remain by construction** — `kid` is on the envelope and signer role is inferred from body fields, so no field-level measure reaches them. See P2 and C2 |
| P2 | Verifier/witness graph leakage | High | §19.2 — open, genuine tension |
| P3 | **A single-identity client** correlates across subnets: anyone present in two of a user's subnets links them | High for such a client, **absent for a multi-identity one** | **Not a protocol limitation.** The protocol permits multiple identities already (§13.7); v1 clients omit the key management and interface work, so this ships as a **client** scope decision rather than a design defect. No wire change separates the two cases |
| P4 | Patron metadata plus mailbox queue | High | **Queue policy settled** (§14.1.6): indefinite retention at the direct patron, no sibling replication, ceiling refuses the newest, no copy outlives delivery, metadata bounded to ciphertext, recipient keyhash and arrival time. The residual is queue *metadata* held while a message waits, which encryption does not touch, and operator logging, which §1.1 cannot reach |
| P5 | Endpoint and backup aggregation | Critical on compromise | Acknowledged (§13.7, §13.7.1). The device is the global correlation point the network architecture otherwise avoids |
| P11 | Heartbeat patterns reveal sleep, work and travel routines | Medium | Process-and-discard (§15) materially helps; the residual risk is implementations that log what the protocol discards |
| **P12** | **End-to-end payload encryption is specified but not yet implemented.** An implementation shipping hop encryption alone leaks payload to both serving nodes | **Critical until built** | §14.2.4 adopts PQXDH and the Triple Ratchet; four integration decisions remain. The patron was accepted as a metadata chokepoint, never a content one |
| **P13** | **Retention promises are undetectable against hostile clients**, though §7.5.2's keystream encryption makes a *compliant* client structurally unable to retain, a client that retains photographs past its declared window runs the match and reports `basis = personal_knowledge` | Medium | §13.7.1. No protocol fix exists, and the mechanism is not detectable by its own use |
| P14 | **Chain back-pointers leak activity level to counterparties.** Signing over a predecessor hash reveals the subject's chain head, so a counterparty meeting the same person twice sees how far it advanced | Low | **Accepted, not open.** It discloses nothing past §8.1's selection threshold, which is `min(floor(n/2), 10, |candidates|)` where *n* is the subject's presence count and **the evaluator learns *n* from the subject by design**. Activity level is already an input every evaluator receives; a chain head is a coarser view of the same fact, given to a party who has met them |
| **P15** | **Service catalog entries reveal what a node runs, to anyone in its horizon who asks.** Resource type, instance name and connection info are served on request (§11.5) | Medium | `discover_scope` filtering at the source limits the audience to those who could use the resource, which is a genuine mitigation. Residual: running a resource at all is visible to everyone in scope, and the *set* of resources a node runs is a fingerprint. Unassessed under §19.1 **Subsumes the former P8** (*topology deanonymisation by association*), withdrawn: identifying one member by real name yields their job, not a label for any of their subtrees — §3.1.1's membership plurality means a member belongs to several, and nothing in the protocol says which is a workplace rather than a bowling team. **What labels a subtree is its catalog**, which is this finding — and the catalog is answered on request within horizon, so a party holding topology from further away cannot obtain the labels at all. An attacker who holds both the topology and a real-name link within their horizon gets the disclosures membership carries (§1.2) |
| P16 | **The resource owner accumulates signed reports about its own resources** (§11.6) | Low | Never broadcast, so no public accusation is created — that was deliberate (§6.2.2). Because the resource reports and the owner receives (§11.6), these are records of the owner's own operation rather than of who complained about whom. **The residual is what a report describes**, not who filed it |
| **P17** | **A direct payload connection reveals each peer's IP address to the other** (§14.1.1) | Low–Medium | New with direct-first payload, and **bounded by limiting direct connection to the horizon** (§12.6.3), the set that already holds your locator and topology, so IP is incremental rather than novel there. Residual: the horizon is up to 111 nodes at or below plus siblings and cousins a user may never have met, so exposure is *bounded* rather than *chosen*. §7.6 establishes IP gives coarse location, so an in-horizon party gains an ongoing location signal. **Both defaults must be overridable**, and the reference client must say what each option discloses |
| **P18** | **A verification query tells a prior counterparty that the subject is *right now* in a witnessed ceremony with someone** (§7.3) | Medium | Intrinsic to verification, and never analysed as a cost, the oracle protections address what a verifier *learns about the biometric*, not what they learn about the subject's current activity. Repeated queries reveal activity cadence. Bounded by the selector querying only a handful of the subject's counterparties per event — and, since 2026-09-01, by the subject's curation: nobody is queryable through a record the subject declines to bundle |
| **P19** | **Archive presentation hands a new patron an intelligible history of prior relationships** (§16.7) | High | The prefix is chosen by the user, but the chain forbids arbitrary omission, so the choice is coarse. Selective disclosure within records (§19.3) is now specified, and does not help here: what a prospective patron reads is counterparty identity, which no field-level measure withholds (§8.1.1). See also C4 in §19.8, which is worse |
| **P20** | **Resource access logs, where an implementation creates them, bind network identity to application actions** (§11.7) | High for sensitive resources | Flagged as unfinished but never analysed. A resource already authenticates by network identity and topological scope, so a log connects *who* to *what they did, when, and under which organisational relationship* |
| **P21** | **Coarse location becomes behavioural location under temporal correlation** (§7.7) | **Medium, mitigated**; High for at-risk users | A single precision-3 geohash is ~156 km. A *time series* of them plus counterparties reveals commuting, travel, conference attendance, employment and residence, and coarsening reduces precision rather than longitudinal inference. **Location is now withholdable** (§8.1.1) and ten of eleven exchanges never receive it. **The residual is structural**: §7.7's impossible-travel check and this leak are the same computation over the same series, so the one recipient with a legitimate use — a prospective patron — is also the dangerous holder. Graph position no longer compounds it (§8.1) |
| **P23** | **Witnesses and verifiers get no disclosure of what their participation creates** (§19.6) | High | The consent machinery protects the *subject* of a query; the verifier, whose own prior relationship is what the response exposes, is asked nothing. Corrected as a reference-client obligation, not yet a demonstrated one |
| **P24** | **A gateway operator sees their subnet's external traffic** (§11.7) | **Medium, reduced** | Socially trusted is not accountable, and the design ensures no patron sees payload content (§14.2) while a gateway hands a subnet member exactly that one layer up. **The user can now evaluate before routing**: the signed `CatalogEntry` binds the service to what the owner published and carries a declared `data_practice`, and it reaches them at catalog lookup — which precedes any connection (§11.5). **The residual is that a declaration is a claim, not a guarantee** (§1.1), and the operator sees the traffic whatever they declared. Beyond that the remedy is not to use the resource |
| **P25** | **A hosted operator can invert a pairwise principal identifier** (§11.0.2) | Low | They know the resource identity and their own org's keyhashes, so one hash per member recovers the mapping. Deliberate, the scheme withholds network identity from parties who do not already know you, not from the operator you chose |
| **P26** | **A resolution request reveals intent to reach someone before any contact** (`wire-format.md` §7.7) | Medium | The same shape C11's prekey fetch had **before** batch prefetch addressed it, and **no equivalent defence has been considered here**: the serving node learns who a client wants to find, whether or not anything follows. Unlike the prekey case, no uniform-prefetch defence has been considered |
| P27 | **`AbuseReport.detail` puts arbitrary particulars into a signed, portable object** (§11.6) | **Low, reduced** | Not a disclosure to the recipient, who is the resource owner and already holds the context (see the withdrawn C16). **The residual was portability by a third-party reporter**, and there is no such party: the signer is the owner's own resource, so the object does not leave the owner's control unless the owner releases it. **That choice remains unreachable by any rule** (§1.1), which is why this is reduced rather than closed — a signature still makes forwarded particulars credible in a way an unsigned account would not. **`detail` may carry an application's own record of which of its users complained**, which is where any real personal particular now sits. Bounded at 1 KB (`wire-format.md` §6.3), which limits volume rather than kind |
| P28 | **Source photographs may carry EXIF and contextual background** (§7.5) | Low | See C17. Encryption under the subject's keystream (§7.5.2) means a compliant client holds nothing readable; stripping still applies because a legitimate decryption during verification puts plaintext in the holder's hands |
| **P29** | **A ceremony counterparty holds the victim's raw capture indefinitely** (§7.5) — **substantially answered by §7.5.2** for compliant clients, where the capture is ciphertext the subject holds the key to | Medium–High | Retention is a client commitment with no detection mechanism (P13). An adversary attending one meeting acquires a biometric sample joined to a record naming time, coarse place and social position. The ceremony is a collection event as much as an evidence event, and the victim consents to the second |
| P30 | **A brokered external session may outlive a user's membership** (§11.2) | Medium | **Addressed at the client**: `light-client-requirements.md` §6 requires telling the user, at first use of a brokered resource, that ending their membership will not end that vendor's session. The network can stop new establishment; it cannot reach into a session running on someone else's terms |
| P31 | **A hostile infra extension is inside the trust boundary** (`infra-client-requirements.md` §9.2) | Medium | Narrowed from a claim about joining separated datasets, which the design does not permit — no binding exposes network primitives to a package. The residual is that **installed code runs inside the boundary the threat model draws around operator conduct**, and whether an isolation mechanism holds against a hostile module is an engineering question this document does not settle |
| P32 | **Client-side caches have no stated lifetimes** — resolved locators, catalog answers, session and capability history, currency queries (§12.6.1, §11.5) | Medium | Each is a record of who a user looked for and when, held on a device that can be seized. **A cache with no expiry is a retention decision made by omission**, and the endpoint-aggregation problem (C9) is what it feeds. Client obligation added; the values are unset |
| P33 | **Multi-device replication semantics are unspecified** (§23.3) | Undetermined | Which devices hold archives, seeds, sealed captures, caches and deletion state is open, so **retention and deletion commitments cannot be assessed at all** — a deletion on one device says nothing about the others. A specification dependency rather than evidence of a leak |
| **P35** | **An ancestor accumulates a key→position index for its whole subtree** (§15.2.1), so a subnet's root can look up any member without an introduction | Medium | **Accepted, with the boundary stated.** The disclosure content is unchanged — §12.1 already has a locator disclosing patron, depth and subtree to anyone you introduce yourself to — and what changes is that an ancestor stops needing the introduction. **Joining a subnet is a choice to be structurally visible to it**; the property defended is that this never crosses a subnet boundary, which §3.1.1 guarantees by construction. **The memo carries no address**, and that depends on peering being excluded from rootward travel (§15.2) |
| **P36** | **`seqno` gaps disclose out-of-subnet activity** (`wire-format.md` §2.3) | Low–Medium, **largely answered** by the `{series, counter}` split | The threat was that a node sharing **one** counter across two bindings advances it in both, so an observer in one subnet sees jumps it cannot account for and learns the node is active elsewhere. **Distinct from P3**, which needs an observer present in both subnets; this works from inside one. **The `{series, counter}` split answers it** by giving each patron relationship its own series and so its own counter (`wire-format.md` §2.3, `wire-format.md` §4.6): a per-binding counter, which was previously rejected as breaking `seqno`'s double duty as freshness test and stale-cache detector — until the series tag made within-series the only comparison and cross-series unrankable, which is what removes the breakage. **Residuals**: a node that has not yet reissued since binding elsewhere still shares a line, and the *number* of reissues it has taken is itself visible in the chain. See C19 for what sharpens the pre-split case |
| **P37** | **A ceremony counterparty is handed a bundle of the subject's presence records, and credibility pushes that bundle wide** (§8.1.2, §19.2) | Medium | Selecting the other's verifiers needs a candidate set, and the subject supplies it as records rather than names, since `wire-format.md` §5.4 counts only what verifies. **The disclosure is elective, not compelled** — nobody walks another party's archive — but the incentive runs one way: a bundle holding nobody the selector recognises is worth nothing to them (§16.1), so being believed means showing counterparties in common, and each record shows its witnesses, verifiers and time. **Distinct from P19**, which is the *adoption* disclosure a prospective patron drives by fetching and walking; this one the subject hands over. **Distinct from P2/C2**, which price the verifier set carried *in the record* rather than the pool it was drawn from. Bounded by what the subject retains (§10.2) and by what they elect to include — and **the floor is a real choice**, since what a ceremony gives its participants is a face they will know again (§7), which no bundle affects. Disclosing narrowly costs third-party weight and the counterparty's continuity assurance, not the relationship |

### 19.5 Queue policy had to settle more than size

A queue needs a retention rule as well as a size bound, and **a size bound is not a
privacy measure**:

| Question | Where |
|---|---|
| End-to-end encryption of queued payload | §14.2.4 — the queue holds ciphertext |
| Whether patron siblings receive queue state | §14.1.6 — they do not |
| Deletion semantics after delivery | §14.1.6 — immediate, nothing recoverable |
| Crash-recovery copies beyond deletion | §14.1.6 — none |
| Queue metadata minimisation | §14.1.6 — ciphertext, recipient keyhash, arrival time |
| What infra operators may log | §14.1.6 — a commitment, not a rule (§1.1) |

**The point the section was making stands and is why it is kept**: every one of these
was invisible behind "how big is the queue", and a specification that settled size
alone would have read as complete.

### 19.6 Unawareness is a product obligation, not a schema one

**The obligation extends to witnesses and verifiers, not only participants.**

A **verifier** who answers a query permanently proves they previously met the
subject; the response is signed and lives in someone else's record for decades
(§8.1). A **witness** who attests permanently proves neighbourhood involvement.
Both become durable nodes in another person's evidence graph, disclosed to
audiences they never chose, and **the design's consent machinery does not address
them**: the subject countersigns the query (§7.4.2), which protects the *subject*,
while the verifier — whose own relationship is what gets exposed — is asked
nothing.

Agreeing to answer one question is not informed consent to years of graph
disclosure. **The reference client must disclose to a verifier or witness, at the
moment they are asked, that their identity and relationship will be permanently
recorded in a third party's evidence and readable by future evaluators.**

**No party can verify this happened.** A client that skips it produces records
indistinguishable from one that does not, and the people harmed are those who were
never told. It is a commitment by the implementer and the only lever available.

Several units score badly on LINDDUN's Unawareness/Unintervenability axis for the
same reason: a participant understands "prove we met" without appreciating that
the record durably names witnesses and verifiers and will be fetched by evaluators for
years. The schema cannot fix this. **The
reference client must disclose at capture time what the record will contain and
who will be able to read it.** Not in a policy document, at the moment of the
ceremony.

### 19.7 Accepted costs

1. **No anonymity or pseudonymity.** Real identity and physical presence are the
   trust mechanism (§4).
2. **Locator topology is disclosed on introduction, and is not meant to be secret.**
   Patron, depth and subtree travel with the address (§12.1). **An ancestor line is a
   participant's identity to the outside world as a member of that subtree** — the
   thing being presented, not metadata escaping alongside it. Supplying an address in
   a different subtree presents a *different conceptual person*, which is why §13.7
   disclaims cross-subnet accountability outright: the archive exists to inform a new
   subnet on joining, not to hold anyone to account across them, and presenting
   different views to different subnets is two histories rather than one edited one.
   **Only the natural person can bridge their positions and regulate between them**,
   and that is where most of a participant's power and autonomy comes from — **at the
   network layer, which is the only layer this claim is about.** The bridge is not the
   person, it is the **device**: P5 already registers it as *"the global correlation
   point the network architecture otherwise avoids"*, and §13.7 already says the
   strongest attack on a multi-subnet identity is device compromise rather than
   anything the network does. So the autonomy is real and it is **endpoint-bound**,
   and whoever holds the hardware holds the join. That is a statement about what the
   protocol declines to build, not a guarantee about where the correlation lives.

   Two residuals, both already registered. With a **single** identity across several
   subnets the bridging is observable to anyone who correlates (C10, §19.8);
   multiple-identity support is the deferred piece that completes this (§19.3). And
   a complete path has **reconnaissance value** to an operator buying edges into a
   region — it shows which candidate relationships collapse onto one cut and which are
   genuinely independent branches. That lowers the cost of the expensive step in
   §17.3's attack without creating any standing, and it is priced here rather than
   concealed.
3. **Recovery destroys unlinkability.** Rotation publishes the predecessor link;
   a fresh Genesis identity is the privacy-preserving alternative (§13.7).
4. **Source-photo retention.** Full photographs rather than templates alone, for
   algorithm migration — sealed under the subject's keystream (§7.5.2), so the
   accepted residual is the release window and the non-compliant holder (P13),
   not storage breach.
5. **Persistent encounter evidence.** Presence facts are immutable; only derived
   standing decays (§16.5).
6. **Visible infrastructure placement** — the ASN is exposed *so that*
   concentration is observable (§17.3). What that buys is a concentration detector
   and not an independence proof: diversity in ASN or region does not establish
   separate legal control, which is the boundary §18.1's compelled provider turns
   on (§1.2.3).
7. **Optional platform-vendor metadata.** Push is opt-in and declared a
   degradation of the trust model (§14.1.5).
8. **Patron as communications-metadata chokepoint** (§12.6.3).
9. **No erasure at the evidence layer.** Once distributed among participants,
   patrons and witnesses, a signed record cannot be withdrawn anywhere. Only
   *derived standing* decays (§16.5). This is architecturally intentional and
   inseparable from what presence evidence is for, but it is a **compliance
   posture, not merely a privacy cost**, and in jurisdictions with strong erasure
   or purpose-limitation requirements it likely needs a documented lawful basis
   for immutable evidence rather than a technical deletion mechanism. Less
   developed than the biometric-retention analysis beside it (§7.2).
10. **No cold lookup.** Discovery is social rather than searchable (§12.4).
11. **A single vendor correlates its own resources.** Pairwise identifiers
   (§11.0.2) stop two *independent* vendors comparing identifiers for the same
   person. They do nothing about one vendor running several services, which
   correlates them from account, device and network data it already holds — the
   identifier scheme is not what stands between them.

   **This is expected rather than unaddressed.** Resources vary in what anonymity
   they offer, and **choosing which to admit is part of how a subnet sets its
   security posture** (§11.4). A user more security-conscious than their
   organisation may decline a service the organisation accepts, which is the same
   remedy §11.7 gives for gateways: the network binds a service to what its owner
   published, and beyond that the choice is the user's.

12. **A currency query tells a patron that someone is asking about their
   subordinate.** The fallback for an absent or expired staple (§12.6.5) discloses
   interest — that a party is evaluating, or being introduced to, that subordinate —
   which is precisely what stapling exists to avoid, and repeated queries would map
   relationship formation.

   **Accepted because the reply discloses nothing further and the query is already
   the exception.** An attestation says only that an identity is current in its
   issuer's subnet; it never reports a rotation and never names what replaced
   anything (§9.0). Stapling makes the query rare, and a caller with a stale staple
   asks its introducer first — a party that already knows. What remains is that a
   patron learns someone asked, which is the price of being able to see a fork at
   all (§9.0.2).

   **The body omits a querier field, and that hides nothing from the responder.**
   Transport authentication is mutual (`wire-format.md` §9.1), so the party
   answering knows which identity asked and can join querier, subject and time
   whatever the schema leaves out. **The omission is not a privacy property against
   the issuer**; what limits exposure is frequency, which stapling and
   introducer-first genuinely reduce. *"Nothing is retained on either side"* is a
   commitment under §1.1's test rather than a checkable rule, and it does not reach
   a compelled provider's logging at all (§18.1, §1.2.3).

### 19.8 Correlation register

**§19.1 says privacy must be assessed under composition. This is where that
assessment lives.** Assessment under composition found that the individual
threats were largely already registered as P1–P21, and that **the new material was
almost entirely compositional.** Pairs and triples whose ingredients are each
acknowledged and whose join is not.

**Current entries only**, on the same rule as §19.4: withdrawn compositions are in
`change-log.md` and their numbers are not reused.

| # | Composition | What it yields | Severity |
|---|---|---|---|
| **C1** | Witness set + presence timestamp + coarse location | *Who, where, when, and with which organisation* — where a subtree maps to a recognisable employer or group. §19.1's worked example. **Reduced**: the participant locator is removed (§8.1), so the affiliation half now comes from the witness set — a sample rather than a statement of position — and coarse location is withholdable from ten of eleven exchanges (§8.1.1) | **Medium–High** |
| **C2** | Witness set + verifier set, in **one** record | A miniature **temporal social graph**: the subject's present neighbourhood (witnesses) *and* a sample of their past counterparties (verifiers). Repeated records turn samples into a map | High |
| **C4** | Archive prefix + the **new** patron's local topology | **The same archive means different things to different observers.** A counterparty opaque in the old subnet may be a known person in the new one, so *moving* an archive changes which entries are legible and can disclose relationships that were effectively private at origin. **Archive portability therefore has a privacy cost that depends on the destination.** Which nothing in the design said | High |
| **C5** | Queue metadata + heartbeat state | *When someone came online to retrieve a particular message.* Distinguishes daily routine from exceptional activity, and both ingredients sit with the same party | High for a patron keeping logs |
| **C6** | Push timestamps + patron queue state | The patron knows what is queued and sends the push; the platform vendor sees the push event. A party obtaining both **aligns network identity with a platform device account** more confidently than either dataset allows alone | High under legal process or platform compromise |
| **C7** | Catalog entry + resource access log + topology | *Role and occupation inference.* A clinical service, code-review tool, family datastore or specialist tracker makes an otherwise pseudonymous subtree **semantically identifiable** | High |
| **C8** | Peering ASN + ordinary provider records | Maps a pseudonymous infra key to a **natural person.** Sharpened by §3.3's rule that each server corresponds to a user. The placement disclosure is accepted (§19.7); this identification consequence was not worked through | Medium–High |
| **C9** | Local face archive + transaction archive, on one device | **§7.5.2 breaks this for compliant clients.** A seized device yields an archive and ciphertext, and the join requires per-counterparty cooperation from the depicted person. **A face-to-key-to-social-history database.** Either store alone is far less sensitive: photos identify faces without network history, the archive identifies keys without biometrics. On a compromised device they coexist and join. **This is the central consequence of P5**, not a separate risk | Critical |
| **C10** | Stable key + catalog + multi-subnet membership | The same person's **services** followed across socially independent contexts — P3's linkage plus P15's fingerprint. **Conditional on a single-identity client**, since the linkage half disappears when a user presents different identities in different subnets | High for such a client |
| C11 | **One-time key request** + queue and routing metadata | **Largely addressed** (§14.2.4). Reusable prekey material is prefetched across the Dunbar Org as a **batch**, so an ordinary fetch names a population rather than a person and carries no intent signal — and the two request forms are structurally distinct on the wire, so a serving node sees which it received rather than inferring motive. **What remains** is the on-demand one-time key request, which is made when a session is actually being opened and therefore precedes its message by a short interval: a node sees a request followed by traffic or by nothing, so an abandoned contact still leaves a trace. Depletion is bounded by per-requester rate limiting rather than by policing motive | Low–Medium |
| C15 | Pairwise principal + vendor account data + several resource ids | **Accepted, not open** (§19.7 item 11). Pairwise identifiers address cross-*operator* linkage; one vendor running several resources correlates them from account, device and network data it holds anyway, and no identifier scheme changes that. **Which resources a subnet offers is part of how it sets its security posture**, and a user may decline one their organisation accepts | — |
| **C19** | **Memo table + per-node `seqno`** | A durable, subtree-wide index of **out-of-subnet activity** for every member. Each ingredient is registered — the table at P35, the counter at P36 — and the join is what turns an incidental gap into a longitudinal series an ancestor holds for everyone below it. **Bounded by what a memo carries**: positions and keys, never addresses (§15.2). **Substantially reduced**: a memo no longer carries the subject's own counter, so the join that produced the series is gone — an ancestor sees patron counters, which say nothing about a subordinate's out-of-subnet activity. What replaces it is smaller: field 4's timestamp, retained per current row rather than as a history, so an ancestor holds when each slot last changed and not a series of when it changed before | Medium |
| C17 | Source photograph + archive or locator | **Largely obviated by §7.5.2.** A compliant client holds captures as ciphertext under the *subject's* keystream, so retained EXIF and background are unreadable. **A non-compliant client keeps plaintext — and that is the baseline**: the same bad actor with an ordinary camera app obtains the same thing, which is the test §1.2.1 sets. **Residual**: a compliant holder decrypts legitimately during a later verification, and has the plaintext in hand for that window, which is why stripping remains a client obligation | Low |

**C4 is the most instructive.** Every other entry composes artifacts held by one
observer. C4 composes an artifact with *the observer's own knowledge*, so the
disclosure depends on **who is receiving** rather than on what is sent. That is a
shape the composition invariant did not anticipate, and it means archive
portability, a feature §16.7 treats as unambiguously good for the user — carries a
destination-dependent cost.

**C9 was the one to design against, and §7.5.2 addresses it for compliant clients.** The architecture works hard to prevent any
network party from joining faces to keys to history; a single compromised device
does all three at once.

## 20. Assumptions and evidence

**This register is curated, not exhaustive, and the difference should be stated.**
A strict reading, one that counts every claim lacking a derivation, mechanism or
source — finds **79 load-bearing unsupported claims** across the document set and 123
in total, against the 31 listed in §20.2. The gap is not concealment: most of it
is §21's parameters and `wire-format.md` §1's array bounds, which both documents
declare as chosen operating points and conservative ceilings rather than derived
values.

**But "chosen" is a disclosure, not a justification.** Labelling a value chosen
says only that nobody claims it was derived. Every parameter in §21 is therefore
an unsupported quantitative claim in the sense that matters for validation, and
§21's preamble says so.

**What §20.2 lists is narrower**: assumptions whose failure would change a
**design decision** rather than a tuning value. That is the useful cut for deciding
what to test first, and it is why the register is worth reading. It is not a claim
that the rest are supported.

What the design rests on that is not established here. §20.1 records claims
made without a source; §20.2 records claims whose failure would change a design
decision. **The two are orthogonal.** A claim can be both, and the six that are
(A14–A19) are the highest-priority items in the document.

### 20.1 Unsourced assumptions

**Asserted without an external source.** None is a fact; each is an input this
design currently relies on, and each needs either a citation, a measurement, or
restatement as a parameter to be determined empirically. Listed so they are not
mistaken for established results.

| § | Assumption | Status |
|---|---|---|
| 7.1.4 | Hill-climbing against binary-output matchers needs "thousands to tens of thousands" of queries | Query counts depend on modality, matcher, and information exposed. **Needs a specific cited attack** if used as a security-cost input |
| 7.1.4 | Cross-device face matching gives "a few percent" false-reject rate | NIST evaluations show error rates vary strongly with algorithm, image quality, pose and threshold. **No externally valid figure exists** until matcher, dataset, threshold and capture conditions are specified |
| 1 | Physical-world affiliation profiling is "expensive, manual, per-target work that no single breach short-circuits" | The benchmark the whole privacy target is set against (§1). No comparative investigation-cost study supports it |
| 1 | Moving affiliations off commercial platforms "makes you a materially harder target" | The security argument for the design. Plausible, and no adversary-cost comparison establishes it |
| 1 | Centralized platforms "capture margin in most cases by displacing more local and accountable intermediaries" | The freedom argument. An economic claim about mechanism, not merely outcome, and unsupported here |
| 1.2.4 | Subnet membership is discoverable "roughly as a church or club is" — parity with physical-world discovery cost | The claim the affiliation limit now rests on. The deniability delta is argued and narrowed (spendable only by §1.2.2's third class); the discovery-cost parity has no comparative study behind it |
| 7.1.6.3 | UWB is "the strongest available proximity channel" | The channel ranking (§7.6.3). Comparative claim with no comparison against the other handset-available channels under a stated attacker |
| 10.6.5 | Carriers aggregate traffic through a small number of regional gateways | SUPPORTING. Drives the "continental resolution" conclusion for latency; carrier topology varies and is not published |
| 7.1.6 | Radio access latency runs 20–80 ms | SUPPORTING. Feeds the same conclusion; varies by radio generation, load and core placement |
| 5 | A hostile installed extension is a larger attack surface than operator conduct | SUPPORTING. Comparative claim about two surfaces neither of which is measured |
| 4.3, 4.4 | Infra will deploy "mostly in cloud datacentres", concentrated in "a handful of clouds" | SUPPORTING. Strengthens the case for making ASN/region visible; the concentration signal stands without the prediction |
| 6.2.5 | Bootstrap mutual adoption is a "likely accident" | SUPPORTING. Cycle prevention must work regardless of whether cycles are accidental or malicious |
| 7.2 | "Real early networks will be thousands to tens of thousands of nodes" | SUPPORTING. Characterises expected early operation; the anchor mechanism is stress-tested independently |
| 10.6.5 | "The industry direction is away from long-lived credentials" toward short-lived ones | SUPPORTING. Persuasive background; the local renewal mechanism does not depend on the trend |
| 7.4 | "Creating a new identity is cheaper than recovery" | SUPPORTING. The stronger structural reason is that a Genesis identity has no accumulated history to recover |
| 11.1.3 | 0-RTT "saves battery" and permits a lazy heartbeat | SUPPORTING. Heartbeat interval is unset; this affects tuning, not architecture |
| 11.1.1 | "A substantial minority" of connections will fall back to TURN relay | Drawn from general familiarity with WebRTC-style deployments, not measured for this topology. **Two mobile peers behind CGNAT is the worst case and the common case here**, so the true fraction may be much higher, which would put the infra economics of §16.6 back in question |
| 7.1.1 | Randomised motion prompts "constitute the liveness check" against print, replay and generated video | Presentation-attack detection is method- and attack-dependent. Motion and parallax are **inputs** to a PAD system, not a defence in themselves. **Needs evaluation of a named algorithm against a specified attack suite**, particularly for generated video |
| 7.1.5 | A face crop runs 30–80 KB | Depends entirely on dimensions, codec and quality. **State the assumptions and derive** |
| 7.1.5.1 | Ageing is modest for adults, severe for minors, "substantial in 24 months" | Degradation with age and particular difficulty with children are supported; the sharp threshold and the 24-month figure are not. **Recast qualitatively** unless a longitudinal study is cited |
| A.1 | Face entropy makes fuzzy commitments' security margins weak | Depends on representation, entropy estimate and helper-data construction. **A design concern, not a settled result** |
| 7.1.6 | Radio access latency runs 20–80 ms | Varies by generation, radio state, operator and load, and the term is ambiguous between one-way, RTT and access procedure. **Tie to a specific technology and measurement** |
| 7.1.6 | Carriers aggregate to a small number of regional gateways | A documented deployment pattern, not a universal property of cellular networks |
| 9.6 | Infra costs ~$20/month retail, ~$5–7 marginal to an attacker | Budgeting assumptions. Cloud pricing varies by provider, region and commitment. **Specify configuration and date if used as threat-model inputs** |

The 2-year retention parameter (§7.5.1) partly rests on the ageing assumption
above, so its basis is weaker than the surrounding argument implies.

---

### 20.2 Load-bearing assumptions

**Distilled from the claims this design makes without establishing them.**
Most are rhetorical intensifiers or parameter choices. The twenty-six below are
different:
**each supports a design decision that would change if the assumption is false.**
None is currently validated. They are the list to attack first, and the natural
targets for simulation.

| # | Assumption | What rests on it | If false |
|---|---|---|---|
| **A1** | **Human time is the binding resource for a presence-proof attack** | The entire Sybil defence (§17.3). Ceremony duration is priced in minutes precisely to meter it | If attackers can hire humans at scale cheaply, presence proofs are far weaker than §17.3 claims and the three-legged defence becomes two-legged |
| **A2** | **Most traffic stays within the two-edge Dunbar Org** | The control/payload split (§12.6.3), f=10 affordability (§3.2), rarity of anchor lookups (§12.2), store-and-forward sufficiency (§14.1.4), **and now the direct payload path, which is horizon-limited (§12.6.3)** | Apex load, anchor load and push requirements change together — **and relay load becomes (out-of-horizon traffic) + (in-horizon traffic where traversal fails)**, so if this is wrong the infra economics of §16.6 collapse as well. The single most load-bearing behavioural claim in the document |
| **A3** | **Apex load scales with churn, not usage** | The f=10 cap surviving at scale (§3.2) | The fanout cap becomes a throughput ceiling and the social rationale collides with the plumbing again |
| **A4** | **Evaluation demand is much smaller than total activity** | Pull-not-push attestation (§15), the biggest scaling decision here | Backbone traffic grows with population. Note this is now framed as an implementation constraint (§15), which is enforceable — unlike the others |
| **A5** | **Presence ceremonies are rare per user** | PQ signatures on presence records (§5), the ~35 KB budget, storage estimates | Record size becomes a real cost and the PQ exception needs revisiting |
| **A6** | **Verification querying will be routine** | The shared-identity defence (§7.3) | The attack succeeds most of the time. Also enforceable via client default rather than assumed |
| **A7** | **Infra operators persist for years with their records** | Reliability weighting (§16.6), verification answerability (§7.4.3), the whole "infra silence is not excusable" argument | Infra participation stops being more evidentially durable and §7.4.3's distinction collapses |
| **A8** | **Attackers cannot parallelise physical presence** | A1's teeth (§17.3) | Paid participants, simultaneous ceremonies and colluding witnesses would defeat it |
| **A9** | **Cross-device face matching FRR is a few percent** | The aggregation rule and threshold design (§7.4.4); also §20.1 | A materially higher rate makes false accusation common; a much lower one makes single-negative policies safe |
| **A10** | **Real social graphs are sparse, so honest operators never achieve theoretical packing** | The claim in §17.2 that attacker economics are *worse* than 1:1 for defenders | The cost symmetry becomes exactly 1:1, weakening §17.2's conclusion |
| **A11** | **Ordinary users will tolerate ceremony friction rather than route around it** | The whole Sybil defence, which only works if the mechanism is used (§7.1) | Users adopt without meeting, unattested adoptions become the norm, and the face-to-face grounding is decorative. Note this is the honest-user mirror of A1: both must hold |
| **A12** | **The facts a composition attack would yield are already obtainable about the target user from existing sources** | The acceptance of composition risk in §19.1 as a *matched* rather than *new* exposure | For any user whose baseline exposure is lower — deliberate minimisers, activists, dissidents, people hiding from someone. The trade is not the one described, and the network creates exposure rather than matching it. **Known false for part of the population**; the question is whether it holds for the intended one |
| **A13** | **Users successfully retain and back up their transaction archive** | The second-factor property (§10.2), portability of history to a new subnet (§16.7), and the value of the chain at all | If archives are routinely lost, users arrive at every new subnet as fresh identities and accumulated standing becomes non-portable in practice. The security property survives; the usability does not, and §13.7.1's backup design becomes the whole story |
| **A14** | Reconstruction against a binary-output matcher needs **thousands to tens of thousands of queries** | §7.4.1's whole oracle-hardening argument — ceremony binding turns that count into weeks of staged meetings | Also §20.1. If the true count is orders lower, ceremony binding and rate limits do not price the attack out |
| **A15** | **"A substantial minority"** of connections fall back to TURN relay | §16.6's infra economics via §12.6.3's direct path | Also §20.1. Two mobile peers behind CGNAT is worst case *and* common case here; a high fraction collapses the cost model |
| **A16** | Randomised motion prompts **constitute** a liveness check | Presentation-attack resistance for the whole ceremony (§7.1) | Also §20.1. They are *inputs* to a PAD system; if no algorithm delivers the property, print and replay attacks pass |
| **A17** | **Face entropy is low enough** that fuzzy commitments have weak margins | Used to *reject* a mechanism that would retain verification capability without retaining biometrics (Appendix B.1) | Also §20.1. If false, the whole retention design could change. This is the only assumption used to close off an alternative rather than support a choice |
| **A18** | Ageing is modest for adults, severe for minors, **substantial in 24 months** | The two-year capture retention tier (§7.5.1) | Also §20.1 |
| **A19** | Infra costs **~$20/month retail, ~$5–7 marginal to an attacker** | §16.6's operator pricing and §17.3's static-addressing leg | Also §20.1 |
| **A20** | A peer may read *"want to back each other up?"* as a **routine technical request rather than an endorsement**, and extend credit they did not intend | §16.3's low default flow capacity for peering edges | Both the superlative ("the cheapest route") and the "trust ceiling" framing are withdrawn. Standing is per-observer and peering is visible only within the two peers' horizons, so the concern is a local misreading rather than a route to global standing |
| **A21** | **Patrons will administer resources.** Hold a connection to a wider system, host an instance, bind roles, carry availability | §11.0.1's federation pattern, and through it every resource application larger than one neighbourhood | The resource-layer sibling of A11: A11 says users tolerate ceremony friction, this says operators tolerate administration. If false, applications stay local or route around the network, and if they route around it, §1.2's product argument goes with them |
| **A22** | Protocol-defined high-importance transactions occur **far less often than once per 100 seconds per user** | The capacity argument under the control-plane topology (§1) | If ordinary use is transaction-heavier than assumed, apex load ceases to be dominated by churn and A3 fails with it |
| **A23** | An adoption without a meeting is **near-worthless** rather than merely weaker | Keeping proof of presence optional (§6.1.1) instead of mandatory where it could be enforced | If unattested edges carry meaningful standing under plausible policies, optionality becomes a gap rather than a graceful degradation |
| **A24** | A user accumulates **a few hundred archive records per decade** | The claim that post-quantum archive size is operationally insignificant (§5) | An order of magnitude more makes the archive a storage and bandwidth problem, not a rounding error |
| **A25** | An envelope past **~400 KB** is prohibitive rather than merely large | Keeping embedded evidence classical instead of hybrid (§5.1) | If that size is tolerable, the simpler uniform rule — everything hybrid — becomes available |
| **A26** | A fuzzed profile discriminating **~99% of humans** is the right privacy/utility point | The biometric query representation (§7.5) | Too specific and it approaches court-grade evidence; too vague and verification stops working |
| **A27** | A normal presence record carries **~10 logical signers, around 8 of them witnesses** | The ~35 KB record figure and the storage arithmetic that follows from it (§8.2) | Witness counts are a social artifact of how ceremonies actually run. Materially more witnesses and the size estimate moves proportionally; materially fewer and the corroboration the record claims is thinner than modelled |
| **A28** | **Package authors' incentive runs toward breadth** in permission defaults | The requirement that templates be inspectable in the predicate language (`resource-requirements.md` §7.3) | An economic claim about a party's motivation, asserted rather than argued. If authors default conservatively instead, the inspectability requirement addresses a problem that does not arise |
| **A29** | **Fabricating a whole fictitious graph is easy**, and convincing synthetic histories are within reach of a motivated party | §1.2.1's deniability property — if fabrication is harder than assumed, correlated evidence is *more* probative and the deniability shrinks | Asserted from the observation that a fabricator holds every key. Nobody has built one, and a graph that survives an evaluator with local reach may be considerably harder than one that survives a distant reader |
| **A30** | **A remote evaluator cannot distinguish a synthesised subnet from a real one** | The same property, and §1.2.2's claim that the discount falls hardest on the classes least able to defeat it | Follows from A29 plus the absence of cold lookup. Untested against an evaluator applying statistical structure analysis rather than key-checking |
| **A31** | **To an attacker accountable to no evidentiary standard, cryptographic attestation adds nothing** | §1.2.2's three-class taxonomy, and the conclusion that on-device encryption is the whole defence against that class | A claim about how such parties actually decide, asserted rather than observed. If signed evidence does shift their behaviour, the archive's non-repudiability costs more than recorded |

**§20.1 and §20.2 are orthogonal registers.**
§20.1 records what is **unsourced**; §20.2 records what is **load-bearing**. A
claim can be both, and **A14–A19 are.** Which makes them the highest-priority
items in the document, since they are simultaneously unvalidated and structural.
Neither register subsumes the other.

**Vignettes citing these assumptions**: A1 → V4 · A2 → V2 · A8 → V4 · A11 → V5 ·
A12 → V6. Changing an assumption obliges a check of every vignette citing it.

**A4 and A6 are different in kind from the rest.** They are things the reference
client can be *built* to satisfy rather than facts about the world. Converting an
assumption into a constraint is generally the cheapest way to discharge it, and
worth checking for the others.

---

## 21. Parameters

**Almost every value here is a chosen operating point rather than a derived one.** The exception is the soundness condition λ < 1/f, which follows from the arithmetic of §16.2 rather than from a choice — what is chosen is any particular λ satisfying it. Seventeen of
them are unjustified quantitative claims in the strict sense: the document explains *why each parameter exists* and *what it trades
off*, and for none of them does it show that the number is right.

**That is the honest position for a design with no deployment**, but it should not
be mistaken for validation. Each of these is a hypothesis about an operating point,
and §20.2's assumptions are what would have to hold for the choices to be sound.
The parameters most exposed are those an attacker can probe (§21.1.1 sorts the
unset ones by that criterion, and the same reasoning applies to the set ones.

**Read the Basis column carefully.** Values are marked
**derived** only where the document supplies a calculation that produces them.
Most are **chosen.** A judgement with stated rationale, which is not the same
thing. `L`, `S`, `h_store`, `h_process`, the retention window, `min(n/2,10)`, the
image count, the geohash default, the forwarding TTL and the currency lifetime
are all **chosen**, not derived.

| Symbol | Meaning | Value | Basis |
|---|---|---|---|
| f | Max subordinates per node | 10 | Span of control ~8 + headroom; Dunbar scale at a two-edge walk (§3.3) |
| L | Non-infra subordinate levels beneath an infra node | 2 | 110 users before infrastructure is required |
| S | Anchor **guideline** (subtree size) | ~500,000 | Not a status boundary, any ancestor may serve as anchor; caching is per-node policy (§12.2, §12.7.3). Yields ~120k widely-cached anchors at the 60B stress scale |
| h_store | Topology storage horizon | 2 | ~110 nodes |
| h_process | Process-and-discard horizon | 3 | ~1,110 nodes |
| — | Cross-tree peers per infra node | ≥2 **recommended** | For fault independence — hierarchical replication has cut 1. **Not required**: peering is voluntary and zero peers is a supported, degraded state (§6.3, §12.7.5) |
| λ | Trust decay per hop (if decay metric used) | < 1/f | Convergence requirement (§16.2) |
| — | Verifiers sought per subject | min(floor(n/2), 10, \|candidates\|) | A reasonableness criterion — n is the subject's own claim (§8.1.2); candidates = distinct prior counterparties (§8.1) |
| — | Capture retention | 2 years | Schelling point; §7.5.1 |
| — | Images per capture | 3–5 | Guided variation, not burst; doubles as liveness (§7.5) |
| — | Location precision | geohash 3 (default) | ~156 × 156 km; precision 4 is ~39 × 19.5 km (§7.7) |
| — | Presence record size | ~35 KB | 10 ML-DSA-65 signatures + body; 26–48 KB across parameter sets (§5, §8.2) |
| — | Currency attestation lifetime | ~10 h | Security parameter, not a cache knob; Kerberos-anchored (§12.6.5) |
| — | Ceremony duration | minutes, not seconds | Meters human time, the scarce resource (§7.1) |
| — | Heartbeat liveness threshold | 3 consecutive missed intervals | Below this a client does not fail over (§14.1.2) |
| — | Default transport port | 7431/udp | Overridable per `NetworkPoint` (`wire-format.md` §9.2) |
| — | Maximum `finalized_at` − `started_at` | **24 hours** | Bounds chronology poisoning: every envelope signer's chain must clear a record's `finalized_at`, so an unbounded one freezes the victim and every witness. Structural, since it compares two fields in the record rather than either against a clock (`wire-format.md` §3.2) |

### 21.1 Unset parameters, the implementation checklist

Grouped by what settling each requires.

**Needs a security argument.** Each of the following bounds an attacker, so a value chosen
for convenience buys convenience at the cost of the bound, and because an attacker
selects which deployment to attack (§21.1.1), the cost is borne by everyone rather
than by the chooser:

| Parameter | Section |
|---|---|
| Peering flow capacity relative to a hierarchical edge | §16.3 |

**Needs measurement under load.** Performance parameters, safe to tune:

| Parameter | Section |
|---|---|
| Heartbeat interval (the value within 1–3600 s; **units are seconds**, the range and the 3-miss failure threshold are set) | §14.1.2 |
| Queue **cap value** only. *(Retention, sibling replication, ceiling behaviour, crash-recovery copies and operator logging are all settled, §14.1.6)* | §14.1.6 |
| Replication depth beyond the floor | §3.4 |
| Contact-locator and negative cache TTLs | §12.5 |
| Prekey rotation cadence and last-resort policy | §14.2.4 |
| Resolution cache TTL, how long a cached intermediate address stays valid | §12.6.1 |
| Anchor caching threshold per node | §12.7.3 |

**Needs a policy decision.** Local, and legitimately different per node:

| Parameter | Section |
|---|---|
| Inactivity-decay function | §16.5 |
| Negative-evidence threshold and inconclusive band | §7.4.4 |
| Queued verifier-reply patience | §7.4.3 |

**Needs an encoding decision.** Wire-format work: **none remain.** The last was
capability parameter ids, resolved by deriving ids from namespaced names so that
nobody assigns them (`wire-format.md` §8.1). The heading is kept because the category
is real and the next unset parameter may fall into it.

#### 21.1.1 How provisional a provisional value is

**"Take a provisional value and tune later" is true of most of these and false of
some.** The distinction is whether a later change costs one operator a
reconfiguration or costs the network a flag day.

**Freely tunable, forever.** Each node chooses independently and may change its
mind; nothing coordinates. Anchor caching threshold, inactivity-decay function,
negative-evidence threshold, replication depth beyond the floor, cache TTLs, and
the **queue's per-subordinate storage cap**. These are §16.1's pluggable-policy
territory and were never going to be fixed.

**Not on that list, deliberately.** There is no durable query log to set retention
for — only an anti-oracle counter expiring with the ceremony window (§7.4.1) — and
**queue retention is not tunable at all**: it is indefinite by decision (§14.1.6),
and what a node chooses is the storage cap, a different quantity.

**Set by one side, obeyed by the other.** Heartbeat interval: the serving node
states it in `AttachAck`, changeable per relationship at the cost of a
reconnection. The queued verifier-reply patience is **not** in this class — it
is the querier's own patience, local policy, transmitted to nobody.

**Effectively fixed by first deployment.** A parameter hardens when **a party other
than its chooser depends on its value**, and hardens hardest when that party is an
adversary who can select which deployment to attack — because then the weakest value
deployed anywhere is the one that matters, and choosing well locally buys nothing.

**No unset parameter is currently in that class** — a current fact, not a permanent
property. The test for the next parameter added: **does someone other than its
chooser depend on it** — an attacker choosing where to attack, a peer parsing a
number, a consumer reading a published figure? Local performance knobs stay soft
indefinitely.

---

## 22. Open for v1

**Everything that must be settled before an initial release, gathered here so it need
not be reassembled from six registers.** What is wanted in a *later* release is §23;
nothing in that chapter blocks anything in this one.


**Consolidated so it need not be reassembled from six registers.**

**The identity, presence, routing and messaging layers are specified. The resource
layer is partly specified.** Every transaction, record, signature rule and encoding is
specified. What remains is parameter values, policy tuning, product behaviour, and one
implementation attempt.

**Nothing here blocks writing code.** The classification below says what each item
does block.

---

### 22.1 Blocks a subsystem — none

**Resource interaction is specified on both halves.** The catalog path —
registration, query and reply, entry lifecycle as local state (`wire-format.md`
§6) — and the request/response path — `wire-format.md` §11's normative evaluation
order and refusal behaviour, with the role row consulted as a lookup.

---

### 22.2 Decide during implementation

- **Eleven unset parameters** (§21.1), sorted in §21.1.1 by how provisional they
  actually are. **None is currently in the class that hardens on first deployment**,
  which is a change worth noting rather than a permanent property.
- **`§14.2.4`'s remaining integration decisions**: binding the session to §5.1's hybrid
  identity; whether prekeys are served only by the patron or also by siblings; prekey
  rotation cadence and last-resort policy; and §5.2's crate-maturity caveat.
- **The canonical biometric profile** (§7.5): extractor, template format and
  fixed length per modality version, fuzzing algorithm, matcher and version
  registry, plus the sealed store's AEAD parameters — cipher, nonce derivation,
  capture framing (§7.5.2). **Cross-client verification depends on the whole
  set**: a fuzzed profile one engine produces must be comparable by another's
  matcher, or verifier queries only work between clients sharing an
  implementation. *The channel and size question is closed: any channel carries
  32 bytes.*
- **Whether a subject may re-derive and re-release a capture key after a device
  restore** (§7.5.2).
- **Peering audit calibration** and **replication distance** (§22.3), both tuning problems
  over working mechanisms.
- **Divergence-notice object** (§22.3), which would make fork detection a durable artifact
  rather than assumed behaviour.
- **Archive recovery after device loss** (§22.3), a gap in user experience rather than in
  the protocol.
- **Multi-device beyond archive merge** (§23.3), which also blocks assessing retention
  and deletion commitments at all (P33).
- **Queue cap value** (§14.1.6), freely tunable per node.

---

### 22.3 The open questions in detail

**Currently open only.** Resolved and dissolved questions are recorded in
`change-log.md`; an entry appearing here means the question is live.

1. **Peering audit calibration.** Latency-bounded challenge-response is
   adversary-influenced: a peer being audited controls the timing of anything
   routed through it, so it can inflate the calibration baseline. Calibrate
   against paths not traversing the auditee, and use randomised, unannounced
   challenges indistinguishable from routine traffic. Note also that other
   replica holders can serve the challenged block, so the audit proves
   availability rather than storage.
2. **Replication distance rules.** Balance of resilience against resource use;
   currently only a floor is defined.
3. **Divergence-notice object.** §9.0.2's fork detection depends on a
   conforming inquirer notifying both patrons, which nothing compels and no patron
   can detect the absence of. **An inquirer-signed notice** naming the two
   conflicting currency assertions would make the observation durable and
   forwardable — evidence rather than assumed behaviour. Patron acknowledgements
   would additionally make successful notification visible. Neither compels an
   inquirer to speak; both replace an unenforceable expectation with a checkable
   artifact.

4. **Archive recovery after device loss** (§10.2). Presence-based recovery
   restores the key, not the archive, so a user who loses their device returns
   with standing in existing subnets and nothing portable to a new one. **The
   keystream scheme widens this**: seeds are device state too (§7.5.2), so a lost
   device also loses the ability to unlock one's likeness on every counterparty's
   machine. Whether
   recovery should restore history, and how, without handing an attacker the
   same path — is undecided. §13.7.1's backup requirements are load-bearing
   because of it.

### 22.4 Open items held in other documents

**Not absorbed, because each belongs to its own document's authority** (Appendix A).
This chapter names them; the obligations stay where the party bound by them will look.

- **`wire-format.md` §13** — encoding items, chiefly the canonical test vectors deferred at §23.4.
- **`light-client-requirements.md` §Open** — participant-client behaviour still to settle.
- **`infra-client-requirements.md` §Open** — operator-side behaviour still to settle.
- **§7.6's local block** — the co-presence questions that belong beside the mechanism they qualify.

---

## 23. Deferred to a later version

**Nothing here blocks anything.** These are areas the design does not address at all,
or addresses only to record that it will not — distinct from §22, which is open
questions about specified mechanisms that must be settled to ship.

**Deferred by decision is not the same as unanswered**, and the deferrals are kept
here rather than in Appendix B because a standing choice that could be revisited reads
differently from settled history. An appendix entry says *this was decided against*; a
line here says *this is not being built yet*.

### 23.1 Deferred by decision

- **Autonomous participation** and the attention question depending on it (§22). Not to
  be implemented in this or any intervening version.
- **Canonical test vectors** (`wire-format.md` §13), until the encoding stops moving
  and someone other than the author writes them.
- **Transaction types beyond the seven**, and **multiple identities per client** (§4) —
  a v1 client-scope exclusion, not a protocol limit.
- **IPv6 endpoints and prefix-based reputation** (§4). v1 demands IPv4; the
  /64 unit, NLRI encoding and prior art are recorded with the deferral.
- **Hard-fork departure and forwarding** (§4). Not in v1: it needs a delivery
  notice that was never specified, is unenforceable, and costs a post-departure
  pointer. Revisiting it means specifying the notice and accepting the linkability
  window.

---

### 23.2 Autonomous participation, and attention as its denominator

1. **Autonomous participation. DEFERRED BY DECISION, not to be implemented in
   this or any intervening version.** To be revisited only if autonomous agents
   become a practical reality. The *delegated* case is closed regardless: §11.3
   makes a delegated agent a resource, so it consumes no subordinate slots,
   confers no down-line credit, and cannot accumulate standing of its own — not
   by rule, but because a resource cannot attend a ceremony. The open question, if
   it ever matters, is **what "presence" means for a persistent self-directed AI**
   bearing its own costs.
   One candidate is that persistent occupancy of specific physical hardware is
   the analogue of embodiment, which would make such a participant infra-tier by
   construction (§3.3). But hardware attestation depends on trusting a
   manufacturer, reintroducing exactly the global trusted party the design
   rejects. Unresolved.

2. **Attention as the denominator of agent trust. DEFERRED with autonomous
   participation above.** It is
   a question about autonomous participation, which is not to be implemented in
   this or any intervening version. Retained because the hazard is worth
   remembering if that ever changes. An agent that persuades its
   principal to physically meet someone demonstrates real substance, since the
   principal/agent relationship normally runs the other way (agent expends work
   to save human time). This makes agent standing denominated in human
   attention, the scarcest resource in the system, and therefore a good
   unit. **The hazard:** it rewards agents for spending their principal's time,
   which is adversarial to the principal's interest. Any such metric needs a
   counterweight, or agents optimise for pushing humans into meetings.


### 23.3 Multi-device

**Storage is solved; durability is a host property.** §13.7.1's envelope
encryption makes the archive safe in untrusted storage, so password managers and
consumer cloud sync carry it as an opaque blob — building private replication would
add new attack surface to duplicate a solved problem. Durability varies by host:
native storage persists until deleted; a WASM runtime persists only as the host
configured it, since WASI confers no ambient filesystem access; browser storage
(OPFS, IndexedDB) is evictable under pressure unless persistence is granted.
Because the archive is a second factor (§10.2), a client that can be silently
evicted must treat §13.7.1's backup as mandatory rather than advisory, and tell
the user so. Practical limits: photo stores reach hundreds of MB, awkward for
attachment quotas, and **the reference client honours declared retention across
backup and restore** (§13.7.1's scan-on-import) — client behaviour, not a
guarantee anyone can check.

**Concurrent devices fork the user's own chain, and the fork is repaired by merge
rather than prevented.** §10 makes the archive a hash chain, so two devices
appending concurrently fork it silently — a semantic conflict no generic sync tool
can resolve, since back-pointers are countersigned and cannot be rewritten into one
sequence. The hazard is unintentional loss — a ceremony performed on one device
absent from the branch later presented — not deliberate partition, which §13.7
permits. §10.3's merge resolves it: a transaction following a fork carries
back-pointers to both heads, so concurrency is legal, **offline signing works**,
and the merge is self-describing — an evaluator learns the structure from the
record itself. Devices may share a key or hold their own; the choice is ordinary
key management, not an archive constraint.

**An infra operator is a multi-device user by construction**, and their instance is
one of the devices. It holds the same key as their phone — the relationship is a
seed shared across wallets, not a client and a server — and runs different software
in a different network role. **So this section is not an edge case for people who
own two phones**: it is the ordinary condition of everyone in §3.3's tier. *(Three prevention shapes — a single
primary device, a head-check before signing, published per-device key bindings —
are superseded; Appendix B.1.)*

**Residual.** A user who never merges leaves branches outstanding, and an evaluator
seeing one branch sees a valid truncation — permitted and self-defeating (§10), so
no rule is needed; the reference client merges automatically on noticing
divergence, since the user has no reason to want otherwise. Replication weakens the
second factor arithmetically, and merging does not change that: **a user who syncs
their archive to three devices has three places to lose it from** (§10.2). What
remains open is which devices hold seeds, sealed captures and deletion state (P33),
tracked at §22.2.

### 23.4 Test vectors, and what a test suite would add

**Canonical test vectors are deferred by decision; draft vectors exist**
(`test-vectors/`, `wire-format.md` §13). Vectors do not block *building*; they
block **demonstrating** that two implementations agree, which is a later and
different thing. Vectors written against a design still in motion become a second
artefact to keep in sync, and cross-artefact drift is this project's dominant
failure mode. They are also better produced by someone other than the designer,
for the same reason review is: **tests written by the author encode the author's
misunderstandings** — which is why the draft set states every interpretation it
had to take and exists to be attacked before an implementation exists to confirm
it. Canonical status waits on an independent implementation reproducing every
computed value.

**A test suite is the natural successor to §22.** Implementing a mechanism asks
*can this be written?* and stubs the error paths; a test suite asks *what should happen when the input is wrong?*, which is exactly where those
stubs were. Expect a different class of defect — boundary values, error paths, and
rules that conflict only on malformed input.

---

## 24. Suggested build order

1. Two nodes exchanging one authenticated message: transport and PQ KEM
   handshake, client dialling outward to a statically reachable infra node.
   **No NAT traversal needed for this step** (§14.1.1), control traffic is always
   client-to-infra. Everything else assumes this.
2. Adoption, departure and disavowal transactions + the local topology table.
   **Build the archive chain from the first transaction** (§10), back-pointers
   cannot be retrofitted onto records whose counterparties have already signed.
3. Anchor set gossip and the resolution sequence (§12.3).
4. Sibling replication, then cross-tree peering.
5. Reference flow-based trust metric + conformance test (§16.2, §16.4).
6. Proof-of-presence ceremony and record (§7–7.3). Sequence the co-presence
   channels by availability: optical first since it needs no special hardware,
   then NFC, then UWB.
7. Verification-by-query and the local face store (§7.2–7.3).
8. Recovery adoption (§9), depends on 7, since the local photo archives are
   the recovery substrate.
9. End-to-end payload encryption (§14.2) — **integrate PQXDH and the Triple
   Ratchet** (§14.2.4) rather than designing key agreement. Independent of 1–8 and
   can proceed in parallel once transport exists. **Leaf-to-leaf only**: patron and
   resource endpoints are covered by step 1's transport handshake.
9b. **ICE for the direct payload path** (§14.1.1), with the serving infra node as
    STUN and TURN. The relayed path must work first: direct is an optimisation
    over it, and a substantial minority of connections will never get it.
10. Resources (§11): object, catalog, abuse reporting and the request path are
    all specified — HTTP/3 over the existing session (`resource-requirements.md`
    §3, `wire-format.md` §11).

Existing stacks (libp2p, Iroh) can absorb step 1 if the novelty is elsewhere —
and it is. Prior art worth reading rather than rediscovering: **Secure
Scuttlebutt** (closest philosophical relative — gossip, local trust, no global
consensus), **Freenet's darknet mode** (friend-to-friend small-world routing),
**Advogato** (attack-resistant trust metrics), and **BGP** (internet-scale
routing on purely local policy, including its failure modes).

---

---

## Appendix A. Document conventions

### Where invariants live

**A security or fairness invariant is stated in role terms in this document; the
wire format states its encoding.** Both are needed, and they are not
interchangeable. A rule expressed only in terms of a message type or field name
dies silently when the identifier is renamed or the taxonomy changes. The test:
**would the rule still be true and still findable if every identifier in the
document were renamed?** The usual shape is *invariant in italics, then "Present
encoding: …"*.

**Generalising a rule can break §1.1.** Abstraction widens scope, and a wider rule
may reach past the enforcement boundary the narrower one respected. So apply the two
conventions in order: state the property, then **ask who would enforce the restated
version against whom** (§1.1). If the answer has changed, split the rule — keep the
enforceable part as a MUST and represent the rest as a visible distinction.

### The force of client requirements

The three requirements documents state obligations that **no party can check**
(§1.1's diagnostic returns *no enforcement available* almost everywhere in them).
**They are commitments, not enforceable rules**: a conforming label means the author
asserts them, not that anyone verified them. Where a requirement does leave a
visible artifact — an absent verifier response, a malformed record, a failing
signature — that is noted in place, and those are the only ones a counterparty can
act on. They are stated as requirements anyway, because a specification that omitted
them would leave an implementer to reinvent each decision, usually worse.

**They describe three software roles, not three populations.** An infra operator is
an ordinary participant who also runs infrastructure: their presence ceremonies,
catalog browsing and resource requests happen in a participant client like anyone
else's, so a complete operator deployment satisfies `light-client-requirements.md`
**as well as** `infra-client-requirements.md`. Reaching your own infra instance from
your own client is a user-interface matter and not a protocol one — the network sees
one node.

### What a client does without asking

**Infra operation is automatic.** Routing, queuing, countersigning
adoptions, acknowledging subtree membership (§11.2.1), replication and issuing
resource credentials all proceed without user intervention. **Witnessing and
answering a verifier query are automatic too, despite resembling human acts**: the
witness's client observes a ceremony, tests the evidence and signs without its
operator knowing the ceremony occurred, and a verifier's client compares a profile
against what it already holds without asking anyone (§7.1, §7.3). **A user
wanting less sets policy in advance rather than being interrupted** — the switch is
theirs, set asynchronously to any traffic it governs, which is §16.1's
pluggable-policy shape applied to attention instead of to trust.

**Hands-on authorisation is for two things only.** The first is **a live interaction
in which you are one of the people being present**: a presence ceremony, an adoption
on either side, and affirming in person that you recognise someone whose key is
rotating. The second is **an operator configuring their own node and resources** —
launching a resource, writing the predicates, setting the policies above.

**The test is whether your own presence is what is being claimed**, not whether the
act sounds like something a person does. A witness attests what its client saw; a
verifier attests what its client holds; neither is asserting that its operator was
anywhere. **A design that meters trust against human attention cannot spend that
attention on bookkeeping** (§1) — and a user prompted routinely stops reading the
prompts.

### Normative vocabulary

§1.1 says the protocol can only compel where shared state exists, so:

| Term | Meaning | Test |
|---|---|---|
| **MUST / MUST NOT** | Enforceable from shared evidence. A violating implementation produces output a recipient can detect as invalid | Can a receiver *check* it from what is on the wire? |
| **The reference client…** | What our implementation does. Others may differ and remain conforming | Local behaviour, not checkable remotely |
| **The reference policy…** | What our trust metric does. Others may differ and remain conforming | Evaluation weighting, expressly pluggable (§16.1) |

**If a rule cannot be checked by a recipient from the evidence it holds, it is
not a MUST.** Write what the reference implementation does, and put the
distinction the rule was protecting into the schema so others can act on it.

## Appendix B. Decision log

Alternatives considered and declined, and conflicts resolved. Standing record —
read before re-proposing anything here.

### B.1 Rejected alternatives

| Rejected | Why |
|---|---|
| **Global DHT for routing** | Exposes every user device's route and interest graph to arbitrary strangers, violating the premise that distant/disjoint trees may be untrustworthy |
| **Routing solely along the tree** | Too fragile; a strict tree has minimum connectivity and any node failure severs its subtree |
| **Tier-based anchor definition** | Not merge-stable; every merge becomes a network-wide re-addressing event |
| **Lowering fanout for security** | Raises honest cost by at least the same factor (§17.2) |
| **Distance-decay as the default trust metric** | Diverges unless λ < 1/f; naive tunings are exploitable by a deep fake subtree |
| **Flooding attestations** | Backbone traffic would grow with total network activity |
| **Tradable token** | Introduces an asset to steal and an incentive gradient toward extraction |
| **Radio-environment co-presence (Wi-Fi/BLE set overlap)** | No timing binding, so scan data is freely forwardable and it provides no anti-relay property; the fraudulent party is physically present by construction anyway (§7.6.1) |
| **Network timing as a proximity proof** | 3 km is 10 µs of light travel against 20–80 ms of mobile radio latency, a noise floor ~3000× the signal (§7.6) |
| **Transfer as a distinct transaction** | The protocol has no concept of a node's set of patrons, so dropping an old one was never a network operation. Moving is adopt-then-depart, in either order (§6.2) |
| **A lateral/vertical shift transaction type** | An ordinary adoption whose counterparty is nearby. The trust-preserving property is derivable from topology, and a self-asserted flag would only be something to lie about (`wire-format.md` §4.2.1) |
| **Rotation as a distinct operation from adoption** | Archive ingestion is already optional in adoption, so rotation is just adoption where the presented archive belongs to another key (§9.0) |
| **An explicit haircut on inherited standing** | Redundant, a rotated key's trust is capped by its attesters' capacity, which is far narrower than the original's accumulated paths (§9.0) |
| **Delegated verification authority** | Verification is evidence, not authority; a signed "same person" fact is an input to an adoption someone else signs (§9.0) |
| **A second veto keypair per identity** | Superseded: there is no veto at all (§9.2). Retained because the original reasoning, a second key adds something to steal without adding a power anyone can exercise — survives the mechanism it was arguing about. Down-line threshold covers roots **that have a sufficient down-line**, with small and Genesis roots an acknowledged open gap (§9.2); departure is self-punishing for a thief |
| **Biometrics in network state** | Irrevocable, fuzzing does not survive combination with timestamp and location, and it would invert the design's own metadata-resistance property (§7.2) |
| **A published trust-policy descriptor** | A node's account of its own policy is unverifiable, so a positive claim is what an attacker asserts; a bad-news-only variant generated attack surface (policy shopping) for documentation value this entry provides instead. **Per-observer trust has no consumer for a published policy** — a resource consumes its own owner's decision, evidence is pulled and evaluated locally |
| **Fuzzy commitments / secure sketches for retained verification capability** | The theoretically correct tool for verifying without retaining the biometric, and face entropy is low enough that their security margins are weak. Would have served §7.5.1's retention problem |
| **Multi-device fork prevention** (single primary device; head-check before signing; published per-device key bindings) | Superseded by archive merge (§10.3): divergence need not be prevented, only reconcilable. Each also broke a property the design needs — a primary can't do phone ceremonies, head-checks forbid offline signing, published bindings need maintenance and revocation |

---

### B.2 110 and 99 are two different quantities

These look like competing answers to one question and are not. **L = 2 stands**; what
differs is a single node's span against a large network's average.

| Figure | Meaning |
|---|---|
| **110** | Users a single infra node holds — two levels of non-infra, 10 + 100. Beyond it a node in that chain must run infrastructure (§3.3) |
| **99** | **Asymptotic** users per infra node in a large network — f^L − 1 |

**Why the yield falls below the span.** In a large network, infra nodes must also
serve as patrons to other infra nodes, and those overhead nodes carry no subtree of
their own. If the infra nodes form a tree of fanout f and depth D, only its leaves
carry subtrees:

- Leaf infra nodes: 10^D, each spanning 110
- Total infra nodes: (10^(D+1) − 1)/9 ≈ 1.111 × 10^D
- Ratio: 110 × 10^D ÷ (1.111 × 10^D) = **99.0**

Which is **f^L − 1** exactly, and is why it lands one short of a round power.
Convergence is fast and from above: **110 → 100 at D=1 → 99 by D=2.** Small networks
are therefore slightly *more* infrastructure-efficient per user than large ones.

**The levels do not compose**. An earlier reading gave each of an
infra node's children its own two levels, making three tiers available as of right
and 1,110 the ordinary span; a later variant kept a third level as an operator
option. **Both are gone**: the levels are counted from the infra node, there is no
third, and a node two levels down that wants subordinates runs infrastructure
(§3.3).
