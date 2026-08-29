# Infrastructure client — requirements

**What an infra node operator's software must do.** Companion to
`network-design.md`, which is authoritative on protocol; this document is
authoritative on operator-side behaviour. Resource conformance is in
`resource-requirements.md`.

**It is not the whole of what an operator runs.** An operator is an ordinary
participant who also runs infrastructure, so their presence ceremonies, catalog
browsing and resource requests happen in a participant client and are governed by
`light-client-requirements.md` (design §0). This document covers the server side
only.

**Your node runs unattended.** Design §0's *What a client does without asking* is
the rule: serve, queue, countersign, acknowledge, replicate and issue credentials
from standing policy, and interrupt your operator only for a live two-person act or
for configuring their own node.

**The infra node is the front door.** It serves sessions, holds queues, issues
currency, hosts resources, and decides who may use them. It is the one component
holding both topology and trust state, which is why role assignment belongs to it
and to nothing else.

**This document states no protocol rules of its own.** It cites design sections rather
than restating them.

**On the force of these requirements.** Most of what follows cannot be checked by
anyone (see `network-design.md` §0, *The force of client requirements*). These are
**commitments, not enforceable rules**: a conforming label means the author asserts
them, not that anyone verified them. Where a requirement leaves a visible artifact,
that is noted in place.

---

## 1. Serving

- **Accept attachment from any node whose nearest infrastructure ancestor is this
  node**, and from siblings' clients in failover (design §11.1.2).
- **Determine the attachment mode from local topology.** A client not in this
  node's subtree is in failover, and report it. The client may hold stale
  topology and not know which state it is in.
- **Include each sibling's full `KeyMaterial` in the list** wherever the client may not already hold it — the field is optional on the wire (`wire-format.md` §6) so a node that knows the client has it may omit it, but omitting it from a client that does not is what makes first failover fail
  (`wire-format.md` §6). A client that has never contacted a sibling cannot
  authenticate it otherwise, the handshake presents the classical component while
  the keyhash commits to the pair.
- **Push the sibling list at attach.** A client cannot discover failover targets
  after its serving node is already dark (design §11.1.2).
- **Process liveness updates and discard them.** Store the resulting reachability
  state, not the update history (design §12). **The protocol cannot prevent an
  operator logging what it says to discard; not doing so is the requirement.**

## 2. Queue

- **Hold ciphertext for offline clients** (design §11.1.4).
- **Queue indefinitely, bounded by a per-subordinate storage cap** (design
  §11.1.6). No time limit: a message survives an absence of any length.
- **Do not replicate queue state to siblings.** Failover covers sessions, not
  mailboxes, a client attached to a sibling collects from its own patron when that
  patron returns.
- **At the cap, refuse the newest message and tell the sender** (design §11.1.6).
  Never drop the oldest: it destroys a message the sender believes was accepted, and
  it lets anyone who can reach the queue flush what is already in it.
- **Delete on delivery, immediately, leaving nothing recoverable.** No journal, no
  tombstone, no crash-recovery copy that outlives the delete. Durable storage for your
  own crash recovery is your backup problem and must not extend a message's life.
- **Hold the minimum while a message waits**: ciphertext, recipient keyhash, arrival
  time. Nothing further.
- **Do not log queue events**, and **state what your deployment actually does** —
  §1's process-and-discard obligation extends here, the protocol cannot reach it, and
  a deployment cannot state its data practices otherwise.
- **The cap's value is yours.** design §16.1.1 classifies it as freely tunable per
  node; nothing coordinates it.

## 3. Currency

- **Issue currency attestations for subordinates**, and accept the sibling,
  grandpatron and down-line issuance paths when the patron is unavailable (design
  design §10.6.5.1).
- **Issue fresh, never extend stale.**

## 4. Resolution

You are the party that answers resolution queries. This is the network's primary
operation (design §10.6.1, `wire-format.md` §5.7).

### 4.1 What you must hold

**Your attached light clients, by path.** Every light client beneath you attaches
to you — not only your direct subordinates, since design §11.1.2 has a client walk up past
light-client patrons to the nearest infrastructure. **You resolve their paths
yourself**; nothing routes through an intermediate light-client patron.

**A child table for your infra children: index → keyhash, endpoints.** These are the
only parties you refer to. Bounded at f, and holding only this keeps per-node state
constant regardless of network size — the aggregation argument design §10.6.1 rests on.

**An anchor table**, for starting points you did not learn out of band. Entries are
gossiped and **their signatures cannot be checked on receipt** — an entry carries
the anchor's keyhash, not its key (`wire-format.md` §5.2). **State plainly which
model you implement**: entries verified on acceptance, possible only where the key
is already pinned, or an unverified cache checked at contact. Treating gossip as
verified gives you a partition vulnerability with no symptom.

### 4.2 How to answer

- **Answer authoritatively** when the path terminates at a client you serve:
  return `ServingInfra` with the residual path suffix.
- **Refer** when it does not: the next hop's keyhash, its endpoints, and its
  `KeyMaterial` where the requester may lack it.
- **Keep nothing for a node that has left your subtree**, and do not redirect on its
  behalf. A resolution against a position it no longer occupies is a failure
  (design §10.3 Case 2); the requester re-resolves from a higher ancestor or is
  re-introduced. **Retaining a pointer to where a departed subordinate went would
  make departure fail to sever**, which is what withdrawing forwarding was for
  (design §2).
- **Report failure only when you can neither answer nor refer.** A referral and a
  failure are different replies; conflating them leaves a requester with nowhere
  to go.

**Referring past several indices is optional, and needs state above the floor.**
§4.1's child table supports one-index referrals only. A node that chooses to cache
deeper can refer further and save the requester round trips — **that is an
optimisation, not a requirement**, and it does not weaken the constant-state
guarantee, which is a floor rather than a ceiling.

### 4.3 Maintenance

- **Remove a child on departure or disavowal.** A stale entry refers requesters to
  a node that will not answer for that path.
- **Replace an endpoint set when you receive a locator with a strictly greater
  `seqno`** for a node you hold (`wire-format.md` §2.3).
- **Collapse forwarding chains at the source**: follow the chain and return the
  terminal record, not the next hop (design §10.3).
- **Keep the topology store across a restart — it is your seen-set.** Forwarding is
  *forward if and only if you stored it* (`wire-format.md` §7.2a), so duplicate
  suppression is a property of the store rather than of a separate cache. A node
  that forgets what it held replays a forwarding wave into every cycle in its
  horizon, which is correct behaviour and a cost your neighbours pay for you.

### 4.4 How you learn an infra child's endpoints

**An infra node publishes a signed endpoint record** (`wire-format.md` §5.6), carried
in the topology class, so its address reaches its patron and the rest of its horizon.
A light client's endpoints arrive when it attaches (§1); an infra child does not
attach to you — it serves itself — so without this record nothing delivered its
address, and **without that you cannot refer**.

**You accept it on the same terms as an anchor entry.** The signature is verifiable
only once you hold the node's key material, so it gives attribution after contact
rather than authentication before it. Treat the record as unverified gossip until you
reach the address and confirm the keyhash.

**Unverified does not mean unusable: refer from it.** A referral you give is not a
credential — the requester authenticates the *subject it meant to reach*, so a wrong
address costs it a failed dial rather than misdirecting it silently (design §10.6.1).
**Withholding referrals until you have confirmed a child yourself would make a live
child unreachable through you** for as long as you had not happened to contact it,
which is the failure this record exists to prevent.

**Publishing your own: a changed address advances your counter** (`wire-format.md`
§2.3), the same counter a position change advances. That is what lets the new record
replace the old instead of colliding with it, and a node that reuses its current
number is publishing an equal-`seqno` disagreement with itself, which is malformed.
**Republish an unchanged set by replaying the record you hold**, not by taking a new
number — reconciliation is a replay of the same frames, and a number spent on
identical contents buys nothing.


### 4.5 What a query discloses

**A resolution request tells you who wants to reach whom**, before any contact
exists (design §14.5.4, P26). It falls under §1's process-and-discard obligation:
answer the request, keep no record of who asked about whom.

---

## 4a. Evaluating a presented archive

**Compare a presented archive against the identities you already know of** (design
§13.1). **That set is yours alone** — there is no tree-level trust state, and your
siblings share your position without sharing your knowledge, because each of you has
different history with users outside the subtree.

**Transactions with unknown counterparties can be ignored; those where the
counterparty is known can contribute to the joining user's initial trust state.**
How you weigh an archive is your policy — the rest of this section is what the
design expects of a patron who follows it, not a conformance requirement.

**Records naming identities you have never heard of are not weak evidence; they are
no evidence**, and treating them as weak is what makes fabricated history worth
manufacturing.

**Do not compute a total.** Archive length, transaction count and counterparty
count are not inputs to an adoption decision. What you are looking for is whether
specific claims are supported by people you can ask.

---

## 5. Prekey service

design §11.2.4 adopts PQXDH, whose asynchronous property depends on someone
holding a subject's prekeys while that subject is offline. That someone is the
serving node.

- **Hold and serve prekey bundles for attached clients** (`wire-format.md` §5.8).
- **Serve reusable material freely.** It is returned any number of times to anyone
  and consumes nothing — clients prefetch it org-wide by default so that fetching
  carries no intent signal (design §11.2.4).
- **Consume one-time keys on serving them**, and only when one is requested.
- **Rate-limit one-time key issuance per requester per subject.** You cannot tell
  whether a requester is really opening a session, and binding consumption to
  session evidence is circular, the key is needed before the session exists. Rate
  limiting bounds the harm instead: an attacker draining a victim's pool to force
  them onto the last-resort key is slowed without anyone having to prove motive.
- **Tell the subject when their one-time pool is exhausted.** An attacker can drain
  it deliberately, and a subject who is not told cannot replenish. **This signal
  only works because one-time keys are excluded from blanket prefetch.** If they
  were prefetched, exhaustion would be the normal state and draining would be
  invisible.
- **The bundle is opaque.** Do not attempt to validate its contents — only the
  endpoints hold the state to interpret them, and treating the blob as inspectable
  couples this node to a PQXDH revision.
- **A one-time key request is a metadata event this node observes.** It learns
  that one party intends to message another before any message exists (design
  design §14.5.8, C11). **A reusable-material fetch is not**, which is the point of
  prefetching it org-wide: only the on-demand request carries intent. It falls under §1's process-and-discard obligation: serve the request,
  keep no record of who asked for whose bundle.

## 6. Transport

- **Act as STUN and TURN** for clients attempting direct payload paths, and carry
  the relayed path as a first-class route rather than a fallback afterthought — a
  substantial minority of connections will never get a direct path (design
  design §11.1.1).

## 7. Operator disclosure

- **Tell an operator plainly what their subordinates are exposed to** by their
  configuration and conduct (design §10.8).

---

## 8. Package hosting

**Distributed as a container or VM image**, so the client must be set up at the OS
level to load additional packages (`resource-requirements.md` §8).

**Accepting extensions is not accepting arbitrary programs.** A package must
declare its roles, tolerate the sandbox, and accept the node's credential
(`resource-requirements.md` §5). A package that cannot declare roles gives
predicates nothing to bind to; one that cannot be confined cannot be contained.

### 8.1 Supply chain

**This is where the project acquires a software supply chain**, and it is a
distribution problem rather than a protocol one:

- Package signing and provenance
- Update channel and its trust model
- Capability declaration in the manifest

**It also creates a trust relationship the trust model does not represent:** a
subnet's members trust their patron's judgment about which packages to run.
Nothing in design §13 expresses that, and it is not obviously reducible to the
existing metric.

### 8.2 Sandboxing

**No host binding exposes network transaction primitives to a package.** [D] This
is not a matter of granting narrow scopes carefully: **the hooks do not exist.**
design §9 already states the rule — *the resource never reads network state* — and
a hosted package is a resource. It receives its credential (`resource-requirements.md`
§2) and its own request traffic, and nothing else: no topology, no liveness state,
no queue contents, no prekey requests, no role-evaluation inputs.

**Why this matters more than ordinary containment.** Those datasets are minimised
individually and separated deliberately, and a single process holding all of them
reproduces the endpoint-aggregation problem (design §14.5.8, C9) on the
infrastructure side. **Offering a binding and scoping it narrowly would still be
offering it**, and the narrow scope would then be a policy decision an operator
could widen.

**A plugin runs inside the trust boundary regardless.** The design works to ensure
a patron sees metadata and not content (design §11.2); code the operator installed
sits inside that boundary, and **the threat model has no account of it** — the
operator selects that code with less information than they have about their own
conduct.

**Whether an isolation mechanism actually enforces the boundary is an
implementation question with its own literature**, not a claim this document makes.
A WASM component target is the expected approach because the component model is
capability-based, so a module reaches only what it is handed. Its effectiveness
against a hostile module is for that literature to establish, and an implementer
should treat it as an open engineering question rather than a settled one.

---

## 9. Role assignment

**The node evaluates access and presents the result as a credential**
(`resource-requirements.md` §1). The resource never reads network state.

### 9.1 Membership is the outer gate

**Read authorisation state once per request and decide from that snapshot.** A
membership check, an acknowledgement, a role row and an availability flag read at
four different instants can describe a state that never existed — and the request
either was authorised or was not.

**An in-flight request completes under the state it started with.** A row that
changes mid-request terminates the *session* (§9.4), which stops the next request;
it does not reach into one already running, and a node that tried would be tearing
down work whose result the resource may already have committed.

**Evaluate in the order `wire-format.md` §7.3 gives**: existence, membership,
acknowledgement, the role row, the carried request's own well-formedness, then
availability. **Existence comes first because membership is owner-relative** — a
keyhash you do not host has no owner, so there is no membership question to ask
about it — and it is answered with the same `refused` either way. **Availability
last is the load-bearing part**: a role-holder learns the service is down, and
someone with no role never does, which keeps operational information about the
owner inside the set entitled to it.

**Answer a member specifically and a stranger opaquely.** The gate has already told
you which you are talking to, so use it: a member lacking a `SubtreeAck`, or whose
role row grants no `connect`, gets the reason (`wire-format.md` §7.3), because they
can act on it and already hold the topology it describes. **You answer from the row,
not by evaluating a predicate** (§9.2). **A non-member gets `refused` for
everything**, including a resource keyhash that names nothing — otherwise a stranger
enumerates what you host by watching which lookups differ.

**Above the patron level, membership alone does not admit.** A node adopted into
your subtree lands inside your Dunbar Org automatically, which means **a subordinate
can put strangers inside your gate without asking you**. Require your own
`SubtreeAck` (`wire-format.md` §5.5) before granting such a node access to
resources you host (design §9.2.1).

- **Issue it automatically, under a policy the operator set beforehand.** The
  deliberate human act was the *patron's* adoption; propagation of membership within
  the horizon follows from it (design §9.2.1), and your node acknowledges a new
  member of your subtree without asking you. **The operator's decision is the
  policy**, made once and asynchronously to any traffic it governs — an operator who
  wants to acknowledge nobody, or only some positions, sets that and their node
  applies it. **Do not prompt per adoption.** A node that interrupts its operator
  once per arrival trains them to dismiss the interruption, which is a worse gate
  than the policy they would have written.
- **On accepting another node's `SubtreeAck`, allocate roles as to any subordinate
  in that network position.** That is the default, and it reaches **positional
  grants only.** A resource whose roles are bound to named individuals (`resource-requirements.md` §7.1.2) is
  untouched, since those were decisions about particular people.
- **Accepting one at all is your choice.** A grandpatron's siblings and the
  great-grandpatron may take one in place of evaluating a stranger themselves,
  which is the saving it offers. Nothing obliges you, and an operator wanting a
  stricter policy sets one.
- **Treat it as lapsed when the relationship it describes ends**, without waiting
  for a revocation that does not exist. If the acknowledged node's patron departs
  or is disavowed, the acknowledgement described a subtree the node is no longer
  in.
- **It says nothing about identity.** The adoption's signatures carry that. Do not
  let a `SubtreeAck` stand in for evidence about who someone is.


**Current membership in the owner's Dunbar Org is a precondition for all resource
access.** Every other predicate sits behind it, and no grant of any kind reaches
outside it (`resource-requirements.md` §7.1.1). Departure therefore revokes
everything, uniformly.

### 9.2 Hold a role table; treat predicates as a macro over it

**Materialise role assignments per resource, one row per Dunbar Org member**
(design §9.4). **Authorisation at request time is a lookup**, never a predicate
evaluation — that makes it deterministic, cheap, and readable by the operator who
configured it.

**Re-evaluate predicates at exactly two moments**: when an operator is configuring
roles, and in a background pass when a node enters or leaves the horizon. A new
member is scored against standing predicates and given rows; a departing one has
theirs removed.

**A table change is what terminates sessions** (§9.4 below), not a predicate change
in the abstract. The row moved or it did not.

**Your predicate language is yours, and so is the table.** Neither crosses the wire
and no other party evaluates either — **the network never sees a role**. The user
sees theirs only as the resource's interface reflects it.

**The one contract that must be stable is node-to-resource**: principal, roles,
audience, session (`resource-requirements.md` §2). It must be **legible to any
implementation**, so a package reads the same credential wherever it runs — that is
portability, not interoperability, and it binds you to the packages you host rather
than to another node.

**Mint the session identifier per resource, not per connection.** One caller
reaching three resources over one session gets three identifiers. A single identifier
shared across them would re-link that caller between resources and undo what the
pairwise principal was derived to separate (design §9.0.2) —
`resource-requirements.md` §3 states the resource-facing half.

**You may decline to host a package you cannot support.** Storage, compute, a
persistent address, a hardware capability: **not meeting a package's requirements is
ordinary capacity, not a conformance failure**, and it says nothing about you or the
package. Failing to understand the credential contract would be.

### 9.2a Predicates

Assignment is by predicate over topology and archive data the node already holds.
Affordances the UI must offer:

- All direct clients
- All clients and grand-clients
- Nodes at a given relative tier
- Nodes above a trust rank
- Nodes joined before a date
- **Named individuals**, inside the membership gate

**Trust thresholds are expressed as rank or percentile, never raw score.** A raw
threshold is denominated in units meaningful only within one metric family, so
switching families silently changes who has access, which is not what the
operator intended when they changed metrics (`resource-requirements.md` §7.2).

### 9.3 Templates

**Because every patron becomes an operator, administration must be nearly
automatic** or the federation pattern does not happen
(`resource-requirements.md` §4.1).

- **Templates ship with the package.** The author knows what "typical" means for
  their application.
- **Templates are expressed in the predicate language**, not as opaque
  configuration, so an operator can see what a one-click choice grants **before
  the click, in the vocabulary they use elsewhere.**
- **A permission default is a security decision**, and the package author's
  incentive runs toward breadth. Shrink-wrapping means trusting the author's
  judgment about access, not merely their code.

### 9.4 Terminate hosted sessions when a principal's roles change

**When a principal's authorisation state changes for any reason, end the affected
sessions rather than notifying the relying party.** Departure, disavowal, a
predicate ceasing to match, an operator revoking a grant, the cause does not
matter, and enumerating causes would mean missing one. Present encoding: drop that
principal's sessions to hosted resources. They reconnect and are re-evaluated; if the roles are gone, so is the
access.

**This is local behaviour, not a network promise.** No protocol rule compels it and
none could: the network cannot reach into an operator's node. It is nonetheless the
correct behaviour, and this client does it.

**Termination requires no protocol, no acknowledgement, no latency budget and no
rule for in-flight requests, which is why it is preferred here.** Telling a resource that a principal's roles changed would require a
notification protocol, an acknowledgement, a latency budget, and a definition of
what happens to a request already in flight. Dropping the session needs none of
that: **role changes are enforced by reconnection**, so there is no mid-session
mutation, no partial-privilege state, and no request that begins under one role and
ends under another.

The resource's only obligation is to tolerate a session ending at any time, which
any network service must regardless.

**Brokered resources are outside this** (design §9.2). The node can drop its own
leg; what the external service does with its session is that service's business.

### 9.5 Show the hosting model when binding a resource

**Display whether a resource is hosted here or brokered to an external service.**
It follows from where the resource runs, so nothing needs declaring.

It matters because revocation differs: a hosted package's session terminates at
this node, while an external service continues on its own terms. **That is a
property of the service the operator chose, not a shortfall here.** The operator
should simply know which they are getting.

### 9.6 What accessing users see

- **The roles they hold on each resource.** Not the predicates that granted them,
  which are the operator's business.
- Without this, a user cannot distinguish *denied by policy* from *broken*, and
  will not know that departing a patron cost them access.

---

## 10. Catalog

**Answer catalog queries; do not propagate entries** (design §9.5). A node in your
horizon asks what you have; you return the entries you **own** and that asker may
see. Nothing floods, nothing is replicated, and nothing needs invalidating.

- **Accept registrations only from the owner's own session** (`wire-format.md`
  §4.7, request type 7). An entry is owner-signed and therefore relayable by
  anyone, so accepting one from any peer means accepting a **replayed earlier
  envelope** — and because you keep whichever you applied last, that silently
  reverts the owner's current entry. The check is free: the owner is attached to
  you and the handshake already named it.
- **The requested `discover_scope` is a request.** You compose the answer, so you
  may narrow it or ignore it, and the owner cannot check. An owner who delegates
  hosting delegates this, and should be told so rather than discovering it.
- **Filter at answer time.** `discover_scope` is your local rule for which entries
  go to which asker. **It never leaves your node** — receiving an entry is what
  qualifying looks like, and a field carrying it would tell the asker how they were
  selected.
- **Hold one current entry per resource, whoever owns it.** A `ResourceRequest`
  names the resource and nothing else (`wire-format.md` §7.3), so if you served two
  claims for one keyhash you would have nothing to pick between them with — and
  picking wrong applies one owner's membership and roles to another owner's backend.
  **Refuse a registration for a keyhash you already serve under a different owner.**
- **Re-registering by the same owner replaces the entry, and the old one is gone.**
  **A registration is not archived** (design §8, `wire-format.md` §4.7): the archive
  advances on adoption, departure, disavowal, peering and presence, and this is none
  of them. Keep a live table of what you currently serve and answer from it. **Do
  not retain superseded registrations** — nobody needs a record of a resource its
  owner no longer runs, and keeping one turns a withdrawal into something a later
  reader can still find.
- **Withdrawal is not a propagated message.** Stop returning the entry and the next
  query gets the truth; there are no copies to invalidate, because you never sent
  any that claimed to be authoritative. **An owner who is not you still has to ask**,
  and does so by re-registering the resource with a requested `discover_scope` of
  self — which no asker but the owner satisfies. What is left is your state to keep
  or drop.
- **A cached answer is the asker's business.** It was true when given, and nothing
  grants access on the strength of it — access is decided by the owner at request
  time (§9.1).
- **Return the owner's signature unchanged, and add none of your own.** An entry is
  signed once at registration and that signature is reused for every answer
  (`wire-format.md` §4.7). It keeps the entry attributable **to its owner** wherever
  it travels; a per-answer signature from you would make one entry's bytes differ
  between askers and would attribute the claim to the wrong party.

---

## Open

- Whether a user can evaluate a gateway operator before routing external traffic
  through them (`resource-requirements.md` §11, design P24). A resource owner may
  declare a data-practice posture in its catalog entry (design §9.5), which helps a
  user who already has access and not one deciding whether to acquire it.

**Closed:** queue lifecycle (design §11.1.6 settles ceiling behaviour,
crash copies, metadata and logging; only the cap value is yours to choose) and
owner-movement semantics (design §9.2 now carries the general rule).
