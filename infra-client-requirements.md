# Infrastructure client — requirements

**What an infra node operator's software must do.** Companion to
`network-design.md`, which is authoritative on protocol; this document is
authoritative on operator-side behaviour. Resource conformance is in
`resource-requirements.md`.

**It is not the whole of what an operator runs.** An operator is an ordinary
participant who also runs infrastructure, so their presence ceremonies, catalog
browsing and resource requests happen in a participant client and are governed by
`light-client-requirements.md` (design Appendix A). This document covers the server side
only.

**Your node runs unattended.** Design Appendix A.3 (*What a client does without asking*) is
the rule: serve, queue, countersign, acknowledge, replicate and issue credentials
from standing policy, and interrupt your operator only for a live two-person act or
for configuring their own node.

**The infra node is the front door.** It serves sessions, holds queues, issues
currency, hosts resources, and decides who may use them. It is the one component
holding both topology and trust state, which is why role assignment belongs to it
and to nothing else.

**This document states no protocol rules of its own.** It cites design sections rather
than restating them.

**On the force of these requirements**: commitments, not enforceable rules —
`network-design.md` Appendix A.2 states why, once, for all three requirements
documents. Where a requirement leaves a visible artifact, that is noted in place.

---

## 1. Serving

- **An instance serves one subnet** [author, 2026-10-15]. Its position is in
  one (`wire-format.md` §2.3 field 4), and so is everything it serves, holds
  and forwards for its subordinates; an operator who serves two subnets runs
  two instances (design §3.3). Nothing here is partitioned by subnet because
  nothing here spans two.

- **Accept attachment from any node whose nearest infrastructure ancestor is this
  node**, and from siblings' clients in failover (design §14.1.2).
- **Determine the attachment mode from local topology.** A client not in this
  node's subtree is in failover, and report it. The client may hold stale
  topology and not know which state it is in.
- **A client must be able to authenticate every failover peer before it needs one.** Include each sibling's full `KeyMaterial` in the list unless this node itself already supplied that sibling's key material to this client in an earlier `AttachAck` or `SiblingUpdate` — the only omission the wire permits (`wire-format.md` §8.2); omitting it from a client that lacks it is what makes first failover fail
  (`wire-format.md` §8). A client that has never contacted a sibling cannot
  authenticate it otherwise, the handshake presents the classical component while
  the keyhash commits to the pair.
- **Push the sibling list at attach.** A client cannot discover failover targets
  after its serving node is already dark (design §14.1.2).
- **Keep the state that serving needs and no history of the requests that
  produced it, whatever the request class** [author, 2026-09-18]. A resolution says whom a client
  wants to reach, a one-time key request that a session is opening, a filtered
  catalog query what kind of service it seeks, a currency fallback whom it is
  evaluating, a publication or deposit that it is making itself reachable. Each
  is priced alone in design §19, and one node answers all of them, so histories
  of them would join into an interest graph no single class yields (design
  §19.8, C23). §2, §4.5, §6 and §6.1 say what this means for the queue,
  resolution, prekeys and doorbells; the rule is the same for a class this
  document does not name.
- **Process liveness updates and discard them.** Store the resulting reachability
  state, not the update history (design §15). **The protocol cannot prevent an
  operator logging what it says to discard; not doing so is the requirement.**

## 2. Queue

- **Hold ciphertext for offline clients** (design §14.1.4, design §14.1.6).
- **Queue indefinitely, bounded by a per-subordinate storage cap** (design
  §14.1.6). No time limit: a message survives an absence of any length.
- **Do not replicate queue state to siblings.** Failover covers sessions, not
  mailboxes, a client attached to a sibling collects from its own serving node when
  that node returns.
- **At the cap, refuse the newest message and tell the sender** (design §14.1.6).
  Never drop the oldest: it destroys a message the sender believes was accepted, and
  it lets anyone who can reach the queue flush what is already in it.
- **Delete on delivery, immediately, leaving nothing recoverable.** No journal, no
  tombstone, no crash-recovery copy that outlives the delete. Durable storage for your
  own crash recovery is your backup problem and must not extend a message's life.
  **This bounds what your node retains, not what your hosting provider can**
  (design §14.1.6, design §18.1) [2026-09-03]: a provider snapshotting below your guest
  keeps what you deleted, and your deletion is hygiene against that party — the
  real bound against everyone above the hypervisor.
- **Hold the minimum while a message waits**: ciphertext, recipient keyhash and
  device, arrival time. Nothing further.
- **Stop serving a binding you have verified superseded** (design §12.6.5)
  [2026-09-03]: authenticated supersession evidence — a verified reissue chain,
  a validated recovery — ends the old credential's service. Terminate its
  sessions and deliver nothing further to it, its queue included. Fail-open is
  for staleness you cannot resolve, never for knowledge you hold.
- **Do not log queue events**, and **state what your deployment actually does** —
  §1's process-and-discard obligation extends here, the protocol cannot reach it, and
  a deployment cannot state its data practices otherwise.
- **The cap's value is yours.** design §21.1.1 classifies it as freely tunable per
  node; nothing coordinates it.

## 3. Currency

- **Issue currency attestations for subordinates**, and accept the sibling,
  grandpatron and down-line issuance paths when the patron is unavailable (design
  §12.6.5.1).
- **Issue fresh, never extend stale.**
- **Issue only for the key you currently record.** A recovery adoption you
  have received overwrites your record of that subject (design §9.0.2), and
  after it you have nothing to issue for the old key and MUST NOT.
  **Nobody can check this for you, and this is a commitment rather than an
  enforceable rule** (design §1.1): an attestation names a subject, a key and
  an epoch, and carries nothing that distinguishes one issued before a
  rotation from one issued after. A party that could tell already holds the
  adoption, and is therefore relying on its own record rather than on your
  attestation. Both halves of design §12.6.5's supersession rule are of this
  kind — yours when issuing, the relying party's when serving (§2's *"stop
  serving a binding you have verified superseded"*).

## 4. Resolution

You are the party that answers resolution queries. This is the network's primary
operation (design §12.6.1, `wire-format.md` §7.7).

### 4.1 What you must hold

**Your attached light clients, by path.** Every light client beneath you attaches
to you, not only your direct subordinates, since design §14.1.2 has a client walk up past
light-client patrons to the nearest infrastructure. **You resolve their paths
yourself**; nothing routes through an intermediate light-client patron.

**A child table for your infra children: index → keyhash, endpoints.** These are the
only parties you refer to. Bounded at f, and holding only this keeps per-node state
constant regardless of network size — the aggregation argument design §12.6.1 rests on.

**An anchor table**, for starting points you did not learn out of band. Entries are
gossiped and **their signatures cannot be checked on receipt** — an entry carries
the anchor's keyhash, not its key (`wire-format.md` §7.2). **State plainly which
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
  (design §12.3 Case 2); the requester re-resolves from a higher ancestor or is
  re-introduced. **Retaining a pointer to where a departed subordinate went would
  make departure fail to sever**, which is what withdrawing forwarding was for
  (design §4).
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
- **Hold what awaits a prerequisite under a bound, and let the reconstructible go
  first** (`wire-format.md` §10). A reissue, an endpoint record or a delegation
  returns through gossip or a later locator distribution; a disavowal or departure
  held for the adoption it precedes does not, and outlasts them at the ceiling.
  Among the latter the oldest goes first.
- **Replace an endpoint set when you receive an endpoint record with a strictly
  greater `seqno`** for a node you hold (`wire-format.md` §7.6, ordered as
  `wire-format.md` §2.3 orders every series).
- **Keep the topology store across a restart; it is your seen-set.** Forwarding is
  *forward if and only if you stored it* (`wire-format.md` §10.1), so duplicate
  suppression is a property of the store rather than of a separate cache. What
  it holds of another party's transaction is that you stored it and the table
  it produced, not the transaction as a history: topology persists as the
  updated topology, and only a signer's own archive keeps the act
  [author, 2026-09-21]. A node
  that forgets what it held replays a forwarding wave into every cycle in its
  horizon, which is correct behaviour and a cost your neighbours pay for you.
- **Evaluate a transaction you are no party to, and keep what the evaluation
  came to rather than the act** [author, 2026-09-23]. Its effect on the
  topology, the counterparties it adds to what you can reach, and what it
  moves in your own representation of their trust: those you keep. The
  transaction you do not. Where a key is later rotated or recovered, what
  connects to the new key is those accumulated representations and not a
  history of the acts behind them.
- **A copy of a transaction sits in the archive of every node party to it,
  and none is authoritative over the others** [author, 2026-09-23]. So file
  what you co-signed in your own archive, and read your own position from
  there; and where you need an act you were no party to, fetch it from one
  of them (`wire-format.md` §7.9) rather than expecting a neighbour to have
  kept it for you.
- **Replay your current state and your own acts, and nothing else**
  [author, 2026-09-23]. `wire-format.md` §10.1.3 makes reconciliation a
  replay of the same frames; what you have to replay is the current-state
  objects you hold — endpoint records, delegations, acknowledgements — and
  the transactions you signed. A party that attaches after a flood has
  passed is not handed it again from here.
- **Keep the derived view too, and keep it beside the store** (design §15.1.1).
  Your table, your routing slots and your own position are a fold over the
  records you hold, and folding them again is work paid on every start. Store
  the fold's result with a watermark that says which records produced it.
- **Discard a derived view that cannot account for your store, and replay.** It
  is a cache of a pure function, so the only safe response to a mismatch is the
  whole fold — never a partial repair, and never a view kept because it is
  probably close. Write it after the store it claims to be a fold of, so it is
  never the newer of the two.

### 4.4 How you learn an infra child's endpoints

**An infra node publishes a signed endpoint record** (`wire-format.md` §7.6), carried
in the topology class, so its address reaches its patron and the rest of its horizon.
A light client's endpoints arrive when it attaches (§1); an infra child does not
attach to you — it serves itself, so without this record nothing delivered its
address, and **without that you cannot refer**.

**An instance cannot mint its own.** The signature on an endpoint record is its
operator's, not its delegation's (`wire-format.md` §7.6), and the anchor entry is
the same (`wire-format.md` §7.2), so a change of address is something an
operator's client signs. Both are rare by construction, an endpoint record being re-signed
when the address set changes and an anchor entry when the subtree size does. The
consequence to plan for is that an instance whose address moves while its
operator is unreachable publishes nothing until they are. **Both are signed at
enrolment** [author, 2026-10-25]: the endpoint record naming the address the
instance serves on, and the anchor entry stating the subtree beneath it, which
at enrolment is nothing. The size is a fact about the node, not a choice its
operator is asked to make, so the client states it and re-signs the entry as
the subtree grows.

**So the instance states the count and its client signs it** [author,
2026-10-26]. **Count what you hold of your own subtree** — the nodes at or below
you over open bindings, the clients attached to you, and the occupants your memo
table has accumulated (design §15.2.1), each once and not yourself — **and state it on
the fetch your operator's client already makes**, beside what your standing
entry claims, the sequence number it carries and the addresses it names. **It is
a floor and should be read as one**: you store topology for your two-edge ball
and the memo table is optional, so the number is at most your subtree and never
more, which understates your reach rather than overstating it — and design §12.2
wants it for an estimate with hysteresis either side. A client finding the count
and the claim different signs a new entry over the same addresses at the next
counter **in the same series**, since a sequence number orders two entries only
within one (`wire-format.md` §2.3). **Say on your administration surface (§8.3)
that one is owed**, with both numbers, as you say the records are owed at
enrolment. The renewal of §7 is where a client does this, being the visit its
operator already makes; an instance whose subtree moves between renewals carries
a stale claim until the next one, which costs an estimate's accuracy and nothing
else.

**You accept it on the same terms as an anchor entry.** The signature is verifiable
only once you hold the node's key material, so it gives attribution after contact
rather than authentication before it. Treat the record as unverified gossip until you
reach the address and confirm the keyhash.

**Unverified does not mean unusable: refer from it.** A referral you give is not a
credential — the requester authenticates the *subject it meant to reach*, so a wrong
address costs it a failed dial rather than misdirecting it silently (design §12.6.1).
**Withholding referrals until you have confirmed a child yourself would make a live
child unreachable through you** for as long as you had not happened to contact it,
which is the failure this record exists to prevent.

**Publishing your own: a changed endpoint list advances your counter** (`wire-format.md`
§2.3) — reordering included, since the list is preference-ordered — the same counter a position change advances. That is what lets the new record
replace the old instead of colliding with it, and a node that reuses its current
number is publishing an equal-`seqno` disagreement with itself, which is malformed.
**Republish an unchanged list by replaying the record you hold**, not by taking a new
number — reconciliation is a replay of the same frames, and a number spent on
identical contents buys nothing. **Bound under two patrons, publish one record
per relationship line** (`wire-format.md` §7.6), each carrying that line's own
`seqno` [2026-09-02].


### 4.5 What a query discloses

**A resolution request tells you who wants to reach whom**, before any contact
exists (design §19.4, P26). It falls under §1's process-and-discard obligation:
answer the request, keep no record of who asked about whom.

---

## 5. Evaluating a presented archive

**Compare a presented archive against the identities you already know of** (design
§16.1). **That set is yours alone**; there is no tree-level trust state, and your
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

## 6. Prekey service

design §14.2.4 adopts PQXDH, whose asynchronous property depends on someone
holding a subject's prekeys while that subject is offline. That someone is the
serving node.

- **Hold and serve prekey bundles for attached clients** (`wire-format.md` §7.8).
- **A bundle, a pool and a queue per device, and a reply carrying every device's
  bundle** [author, 2026-09-22]. A subject's devices attach as separate
  sessions, each named by the key it presented (`wire-format.md` §8.2); a
  publication, a deposit and a wake registration belong to the device on the
  session they arrive on, and a publication naming another device is refused
  (`wire-format.md` §7.10); a relay submission names the device its ciphertext
  is for and is queued for that device (design §14.1.6).
- **Serve reusable material freely.** It is returned any number of times to anyone
  and consumes nothing — clients prefetch it org-wide by default so that fetching
  carries no intent signal (design §14.2.4).
- **Serve a one-time key only for a subject you serve** [author, 2026-10-16].
  Serving one is a metadata event (below; design §14.2.2, C11), so the party
  that learns it is the one already serving that subject: a sibling holding a
  copy would widen who sees it for nothing. **Reusable material is different
  and may be served by a sibling** — it is published to be fetched and carries
  no intent — so a cache of it costs the subject no disclosure. Asked for a
  one-time key for a subject you do not serve, answer that you hold none,
  which is what you would answer anyway.
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
  §19.8, C11). **A reusable-material fetch is not**, which is the point of
  prefetching it org-wide: only the on-demand request carries intent. It falls under §1's process-and-discard obligation: serve the request,
  keep no record of who asked for whose bundle.

### 6.1 What your clients hand you

- **Take a prekey publication only from the subject it names**
  (`wire-format.md` §7.10). A client publishing another party's bundle would be
  choosing the material that party's peers open sessions against.
- **Bound a deposit and say so.** The pool is space you lend, and a client that
  asks for more than you lend is refused with the code rather than silently
  trimmed.
- **Answer a relay submission when you have taken it, not when you have
  delivered it.** Taking it is the promise the queue already makes (design
  §14.1.6); a sender made to wait for delivery would be waiting on a party who
  may be away for days. Refuse one for a keyhash you hold no record of, which
  is a different answer from offline (design §7.4.3).
- **Hold at most one wake endpoint per relationship, and post nothing but a
  doorbell to it** (design §14.1.5). The body carries no payload, no sender and
  no count: it says only that there is something to come back for.
- **Forget an endpoint when the relationship ends**, and when a client
  withdraws it. It is a routable identifier for a person, and keeping one past
  the reason you were given it is a retention decision made by omission
  (design §19.4).
- **Do not log doorbells.** When you rang is when that client had mail, and a
  log of it is the queue event log §2 already refuses, one step removed.

### 6.2 Profiles

- **Hold the latest profile per (subject, subnet, kind) and serve it to anyone
  who asks** (`wire-format.md` §7.11). It is published to be read: a fetch is
  the weight of a reusable-material fetch and is not logged (§1's
  process-and-discard).
- **Take a profile only from the subject it names**, as a prekey publication
  (`wire-format.md` §7.10), and replace the one you hold only by one
  published later (`wire-format.md` §7.11). A subnet's profile is
  taken from whoever signed it: that the signer is the root is a convention
  (design §12.8), and this node checks the signature and not the convention.
- **Bound what you hold.** An image is at most 64 KiB and an alias 64
  characters (`wire-format.md` §1.3); refuse over the bound with the code
  rather than trimming, as a deposit is refused (§6.1).
- **The profile is opaque to what you do with it.** You serve bytes a subject
  signed; you do not render, match, or index by the alias, and you do not read
  the image. Anything a profile says is between the subject and whoever
  fetches it.

---

## 7. Transport

- **Act as STUN and TURN** for clients attempting direct payload paths, and carry
  the relayed path as a first-class route rather than a fallback afterthought — a
  substantial minority of connections will never get a direct path (design
  §14.1.1).
- **Present a delegated transport key rather than your operator's identity key.**
  An instance mostly runs where its operator cannot stand over it (design §23.3),
  so what you hold is a credential their client signed for a window
  (`wire-format.md` §8.2) and what you present in a handshake is the key that
  credential names. The key that signs as your operator stays on the device that
  performs ceremonies.
- **At rest, your store's protection is your operating environment's**
  [author, 2026-09-26]. A daemon that restarts unattended cannot hold a
  secret its own disk does not, so this document issues no sealing rule it
  could not enforce (design §1.1): disk encryption and file permissions on
  an operator's box are the operator's own arrangements, stated here so the
  choice is a choice. What bounds the exposure instead is what the box holds
  — an instance runs on a delegated transport key and a run of credentials,
  never the operator's seed (design §23.3), so a seized disk yields a
  credential that expires, not an identity. An operator who runs with a seed
  on the box, or with a light client's store beside the node, is choosing
  that exposure, and the client store's own sealing is the client's
  obligation wherever it runs (`light-client-requirements.md` §9).
- **Sign what you emit unattended under that same key, and push the credential
  so your horizon can check it** [author, 2026-09-21]. A subtree acknowledgement
  (`wire-format.md` §7.5) and a currency attestation (`wire-format.md` §7.1)
  are topology state and yours to issue while your operator sleeps, so the
  delegated key signs them; push each credential as it comes into force
  (`wire-format.md` §10.1), and staple the current one to every attestation
  (`wire-format.md` §7.1 field 8), whose relying party sits outside your
  horizon. What advances an archive, a disavowal or a countersignature, is
  your operator's act and never yours.
- **Present your delegation before anything else on every connection you
  accept, and refuse a session whose delegation does not name the key the
  handshake presented.** A delegation is public and travels on every
  connection; that one check is the whole of what stops a captured one being
  replayed onto another. A party that reaches you by referral from outside
  your horizon holds your key material and not your delegation, so the
  delegation is the first thing it hears from you. Present encoding:
  `AttachAck` field 6 on a session, control frame type 7 first on stream 0 on
  a connection that opens none (`wire-format.md` §8.2) [author, 2026-09-21].
- **Mint your own transport keypair, and send only its public half to be
  signed** [author, 2026-09-21]. The private half never leaves you; your
  operator's client signs every credential of the run over the one key, and
  the key changes only when you are re-provisioned (`wire-format.md` §8.2).
- **Hold the run you were given, and say so while it is still long.** A run is
  **45 credentials of 48 hours each** [author, 2026-09-16], 90 days end to end,
  contiguous: each credential's `not_before` is the previous one's `not_after`,
  so there is no moment the run does not cover and no moment two credentials
  are both current beyond a receiver's leeway.
  The party that signs your next credential is a device with a battery and an
  owner who sleeps, so an instance that waited for expiry to ask would go dark for reasons
  none of its subordinates can see. Holding forward-dated credentials is the
  point of them; telling your operator before the last one is the obligation.
- **Never issue a resumption ticket that outlives your delegation** (design
  §14.1.3). A resumed session re-presents nothing, so a ticket is the one way a
  lapsed credential goes on working.

## 8. Operator disclosure

- **Tell an operator plainly what their subordinates are exposed to** by their
  configuration and conduct (design §13).

### 8.1 What an operator's interface may ask of the node

design §14.1.0 puts sessions and every byte on the wire inside a kernel and
makes what a person looks at a client of it. A node's operator has one too, and
it is bound by the same line.

- **An operator's interface reads and never speaks for the node.** What it is
  shown is state, who is attached, what is queued, what is held, and it does
  not compose, sign or send anything on the wire.
- **What crosses to it is what to draw.** A count, a keyhash, a time. An
  interface handed frames would be a second parser in the place the first one
  already works, and an operator's view has no ceremony and no device, so
  design §14.1.0's allowance for a payload that must cross does not arise
  here.

### 8.2 How an operator reaches their own instance

**An operator administering their node is not one network role addressing
another.** A participant's client and an infra node are distinct roles and the
protocol governs what passes between them; a person configuring the machine
they own sits beside that rather than inside it. No request type carries a
command (`wire-format.md` §9.2), and none is owed one.

- **Administration is out of band, with everything else about the host.** The
  node's own configuration, the host's operating system, and the provider's
  control plane are reached the way any other server is reached, and this
  document set does not specify the channel. A credential for the last two can
  destroy the instance and bill its owner, and an operator who is never told
  the difference between the three cannot judge what they are handing over.
- **One package, not two systems.** design §23.3 makes an operator's instance
  one of their own devices, so Dana should not have to think of the node she
  runs as somewhere she logs in: her client carries the operator's interface
  beside everything else it carries. But that interface is an extension of
  *this* document's software rather than a participant's client acting as one.
  What she is spared is juggling sessions and devices, not the distinction
  itself — §8.1's line holds whatever the channel, and an interface handed
  frames is still a second parser.

### 8.3 The administration surface ships with the node

**A node develops and serves its own administration pages; a client provides the
frame they are presented in.** Third-party implementations of both roles are
expected, and an administration surface agreed between them would have to be
defined universally — constraining what either may build, and fixing behaviour
that has no reason to be common to begin with. Serving its own, each
implementation administers itself with a surface that matches its software and
stays in step with it as that software changes.

- **A client ships the provisioning pages and no others.** Those have to work
  before any node exists, which is the reason they cannot come from one. Once
  Dana's instance is running, what she administers it with arrives from the
  instance.
- **The frame is a sandbox, isolated from the presenting client's keys, archive
  and sealed captures.** What it draws arrives from a node, and a seized node is
  a case this design plans for (design §18.1): a frame with reach into the device
  presenting it would hand a seized instance a path to the device that still
  holds the seed, which is exactly what design §23.3's arrangement is there to
  prevent.

---

## 9. Package hosting

**Distributed as a container or VM image**, so the client must be set up at the OS
level to load additional packages (`resource-requirements.md` §8).

**Accepting extensions is not accepting arbitrary programs.** A package must
declare its roles, tolerate the sandbox, and accept the node's credential
(`resource-requirements.md` §5). A package that cannot declare roles gives
predicates nothing to bind to; one that cannot be confined cannot be contained.

**You may decline to host a package you cannot support.** Storage, compute, a
persistent address, a hardware capability: **not meeting a package's requirements is
ordinary capacity, not a conformance failure**, and it says nothing about you or the
package. Failing to understand the credential contract would be. The package's side of
the same fact is `resource-requirements.md` §8.

### 9.1 Supply chain

**This is where the project acquires a software supply chain**, and it is a
distribution problem rather than a protocol one:

- Package signing and provenance
- Update channel and its trust model
- Capability declaration in the manifest

**It also creates a trust relationship the trust model does not represent:** a
subnet's members trust their patron's judgment about which packages to run.
Nothing in design §16 expresses that, and it is not obviously reducible to the
existing metric.

### 9.2 Sandboxing

**No host binding exposes network transaction primitives to a package.** This
is not a matter of granting narrow scopes carefully: **the hooks do not exist.**
design §11 already states the rule — *the resource never reads network state*, and
a hosted package is a resource. It receives its credential (`resource-requirements.md`
§2) and its own request traffic, and nothing else: no topology, no liveness state,
no queue contents, no prekey requests, no role-evaluation inputs.

**Why this matters more than ordinary containment.** Those datasets are minimised
individually and separated deliberately, and a single process holding all of them
reproduces the endpoint-aggregation problem (design §19.8, C9) on the
infrastructure side. **Offering a binding and scoping it narrowly would still be
offering it**, and the narrow scope would then be a policy decision an operator
could widen.

**A plugin runs inside the trust boundary regardless.** The design works to ensure
a patron sees metadata and not content (design §14.2); code the operator installed
sits inside that boundary, and **the threat model has no account of it** — the
operator selects that code with less information than they have about their own
conduct.

**Whether an isolation mechanism actually enforces the boundary is an
implementation question with its own literature**, not a claim this document makes.
A WASM component target is the expected approach because the component model is
capability-based, so a module reaches only what it is handed. Its effectiveness
against a hostile module is for that literature to establish, and an implementer
should treat it as an open engineering question rather than a settled one.

**What you hand a package besides its callers' traffic goes through the same door,
on paths that are yours** [author, 2026-10-26]. `resource-requirements.md` §3.3
reserves `/.rhtn/` for the node: refuse a member's request naming such a path
before it reaches any backend, so that a resource seeing one is seeing you. On
those paths you carry the hosting side of application federation for the instances
you host, and nothing of the network with it: tell each instance the relationships
its operator configured for it (a patron's instance, subordinates' instances, named
kin, each a resource keyhash, written in your hosting configuration beside the
resource); hand it each envelope a related instance sent, marked with the origin and
the relation; ask it for what it emits; and carry what it emits to the related
instances you host yourself. **Carry within the configured relationships and no
further**: an envelope whose origin is not a relationship of the receiving instance
is refused, and one addressed to an instance that is not a relationship of the
sender is held, visible, and not forced through.

**Authenticate the sender before you touch the record state** [2026-10-27]. An
envelope says which instance it comes from; a handshake establishes which *node* is
speaking, and the two are different claims. Bind them: **the key the caller
presented must be the key your operator configured for the node hosting the origin
it claims**, and where no node is configured for that instance, refuse — nothing
authenticates the claim, and your operator's own surface is where such an envelope
is handed over by hand (§8.2). **Refuse before the plane is touched, not after**,
because taking a record moves its revision watermark: an envelope accepted on a
caller's word alone can claim the largest revision there is and leave every
legitimate revision after it classified as older and never delivered, which a
signature the application would have rejected does not undo. **Authenticating the
node is not authenticating the instance**, and this is the narrower of the two
guarantees.

**It is also the guarantee this version offers** [author, 2026-10-27]. A node
speaks for the instances it hosts, which is no concession: it runs their code and
holds their store, so a dishonest one could make its own instance say anything
whatever a signature said. **So the envelope's signature is carried and not
verified here**, and the narrower claim is the one to make — not the wider one
worded loosely. What the narrower claim does not cover is anything that outlives
the connection carrying it: a record verifiable at a node it did not come from, a
receipt an operator can hold up afterwards, an envelope recovered from a backup,
and a provenance that survives a transport key being rolled. Each of those wants
the instance's own signature checked before the record state moves, and that waits
on a key contract — introduction, proof of possession, algorithm, rotation,
restoration, and a signature bound that follows the scheme rather than choosing
it. Custody is where it would go: verify, admit, acknowledge.

**And so this version carries one hop, by construction rather than by rule.**
Ingress requires the caller to be the origin's *own* node, so no third party can
deliver on another instance's behalf; an envelope therefore cannot arrive
forwarded, and an instance may only emit envelopes it originated. Carrying a
record further than one hop needs the signature above, because a receiver two hops
out is otherwise trusting the forwarder's word about who wrote it.

**Keep the record bookkeeping
`resource-requirements.md` §3.3 states before anything is delivered** — conflict before duplicate, newer
supersedes, older discarded, a tombstone that replay does not undo — and keep a
conflict for the operator rather than resolving it. **Show the plane on your
administration surface** (§8.3), per instance: the relationships, what is held and
whether it stands, what waits, what is pending for an instance elsewhere, and any
conflict, and the work you stopped carrying with the reason — so that what was
pushed to an instance and from whom is legible to its operator, and so is what was
cancelled.

**Deliver at least once, in order, and say so** [2026-10-27]. Retry what an
instance did not take; **supersede a queued older snapshot** rather than delivering
it after the newer one that replaced it, and discard one in flight when a newer is
taken; stop a delivery retried to exhaustion, visibly, and queue it afresh if the
identical envelope is offered again, so neither side has to invent a revision to
retry one. **Mark the record stopped whatever stopped it** — exhaustion, a
relationship withdrawn, a configuration that moved under the answer — because a
record left waiting with nothing queued takes the next offer of it for a duplicate
and suppresses it, and nobody is then able to retry. **A relationship removed revokes the work it authorised**, ingress and
egress alike, and the relation an envelope arrives under is read when it is handed
over rather than when it arrived. **Re-announce the relationships when a backend is
rebound**, a new backend having been told nothing, and **forget the plane of a
resource you no longer host**.

**Never call a resource while your own state is locked** [2026-10-27]. A hosted
package or a socket service may take as long as it likes to answer, and a pass that
called them under the lock guarding your topology, your queue and your
administration surface stopped all of it for that long — for every hosted instance
at once, where an arriving envelope triggered a pass. Decide what to ask under the
lock, ask holding nothing, commit under it again, and check on commit that the
configuration has not moved under the answer. Bound what you accept while you are
there: one delivery an instance a round, so a slow instance costs its own round
rather than everyone's; a cap on the exchanges in flight; a deadline on the whole
of an exchange rather than on each read of it; a bound on a body before it is
allocated; and checked arithmetic on any length a caller sent you.

**Keep what you have taken where a restart finds it** [2026-10-27]. An answer
that admits an envelope tells the sender it may forget its copy, so **write it
down and flush it before you answer**, not on a later tick: the rest of your state
can accept losing an interval, and this cannot. Keep, per hosted instance, the
envelopes you have admitted and not yet had applied, the emissions you have
admitted and not yet carried, and **the watermark of every record** — revision,
digest and state — which is the replay protection and without which a restart
reads a withdrawn record's older live copy as new. Give custody up when the
resource answers 2xx, and not before; give it up for an emission when the far side
has it, and not before. **Tell the application which of its items you hold**, on
the one exchange that can carry it, so that it knows when an item has stopped
being its responsibility; treat an item it offers again while you hold it as the
same item. **Say on your operator's page whether you are keeping custody at all**,
and do not federate without it: every answer such a node gives is a promise it
cannot keep across a restart.

**What that does and does not buy, said plainly.** A 2xx from an instance means
that instance took responsibility, and nothing weaker should be read as custody. A
redelivery is ordinary — an acknowledgement can be lost on either hop — so
**application idempotence is required and is not a remedy for loss**: it makes a
redelivery harmless and cannot recreate a message that no surviving component
knows to resend. What is still owed is the atomicity the application needs where
it shares a store: its own record change, its deduplication and its outbox entry
committed together. **An envelope for a related instance another node hosts is carried to
that node over TLS, node to node** [author, 2026-10-26]: where you opened a
federation listener it presents the raw public key your transport presents, and
where your operator wrote the far node's listener and key beside a related
instance you deliver there, presenting your own key and refusing a listener that
presents any key but the one written. The applications see nothing of the carriage
— instance to instance in meaning, node to node on the wire — so the bookkeeping
and the relationship check above are applied once, at the receiving node, whatever
carried the envelope. What this gives up is delivery to a node that is down: a
pending envelope is retried each tick until the far node takes it, and where no
location is written for an instance it stays pending, readable from the surface
and deliverable at the other node's surface by hand.

---

### 9.3 What a host provides a package

**The host surface is versioned, and this is `rhtn/host@1`** [author,
2026-10-30]. A package's manifest names the versions it tolerates
(`resource-requirements.md` §8); you offer the ones you implement; **a
package whose tolerances meet none of yours is refused at installation
with both lists named**, which is ordinary capacity (§9) said before any
request. Bindings are only ever added within a version, and a removal is a
new one.

**Six bindings, and the list is closed.** Each is answered here against
§9.2's test — *does this give the package anything of the network?* — and
a binding that cannot answer it is not in the surface.

| Binding | What it is | The §9.2 answer |
|---|---|---|
| **request and response** | the gateway's door as §3 and `resource-requirements.md` §3 have it, and the exchanges on your own paths (`resource-requirements.md` §3.3) | unchanged: the credential-bearing request traffic and nothing else |
| **the storage block** | a durable region of your own storage, provisioned per instance when you bind it and destroyed only by an operator's act, never by a reload; flat files under names of the package's choosing; a **byte ceiling the package declared and you enforce on every write**, the refusal naming the ceiling; `sync` as the whole of the durability discipline — when it returns, the bytes are on stable storage | instance-local data the package wrote; you read none of it, keep no persistence logic for it, and share it with no other instance |
| **the log** | lines the package writes for its operator, each stamped by you, kept per instance within the bytes the package declared, the oldest going first, shown on your administration surface (§8.3) | outward only: the package writes, the operator reads; it is for operators and never carries member content, which is the package's rule to keep |
| **the clock** | your time, milliseconds since the epoch | advisory by nature: wire timestamps are signer-controlled (`wire-format.md` §3.3) and nothing here treats yours as proof of anything |
| **fetch** | an outbound request to one of the destinations the package's manifest declared — `https://host` or `https://host:port`, exactly, no wildcard, never your own machine or network — the package handing you one HTTP/1.1 request as bytes and taking one response as bytes, as the gateway hands it a request and takes its answer; the destination is the request's `host`, you send with your own `host` and `connection: close` over TLS to the roots your operating system trusts, bound what you read back, honour a redirect only to a declared destination and only a few times, and refuse a declared name that resolves to your own machine or network | the package's own traffic to parties its operator read and accepted at installation; you are its egress and learn nothing of the network by carrying it |

| **jobs** | durable scheduled work: the package schedules a task for a time with a budget and gets a number back, may cancel it or list what stands, and you run each job that is due on your tick by calling the package's `job` entry with the number and the task — **under the instance's authority as of now, never under any caller's session**, which ends without notice; one file per job in a directory of yours, so a restart finds every job scheduled and not yet run; a job runs once and is not retried by you; bounded by the manifest's `jobs` in flight, the refusal naming the bound, and by `job_seconds_per_day`, the day's spend kept beside the jobs and the rest waiting when it is spent, said in the log | instance-local work the package gave itself; you carry the clock and the table and nothing of the network |

A package on a socket of its own has its own timers and schedules nothing
through you.

**Where the surface lives.** A package in your sandbox reaches the block,
the log, the clock, fetch and jobs as `rhtn/1:block`, `rhtn/1:log`,
`rhtn/1:clock`, `rhtn/1:fetch` and `rhtn/1:jobs`, imported the way it
imports `rhtn/1:request` and `rhtn/1:response`, and reaches nothing it did
not declare; a package importing fetch that names no destination, or jobs
without a `job` entry, is refused at installation. A package on a socket of its own has its own network, and the
list it declared is its operator's to read. A package on a socket of its own has
your filesystem already; **you tell it where its block and its log are, and
what it may fill them to, on the relationships exchange**
(`resource-requirements.md` §3.3): `rhtn-host` with the versions you offer,
`rhtn-block` and `rhtn-block-bytes`, `rhtn-log` and `rhtn-log-bytes`,
repeated with the relationships until the instance takes them, and again
when any of it moves. **Keep the blocks and the logs in a directory of your
own**, one block and one log per instance, named to it; a node that keeps
no such directory cannot host a package that needs one, and says so at
installation.

**Budgets are the package's claims and your enforcement.** The manifest
names the block's bytes, the log's bytes, jobs in flight, requests a minute
and job compute a day; you bound what you have built, name the one exceeded
on your operator's surface, and treat the rest as the package's own
accounting above your ceiling. Their first values are the package author's,
unmeasured, and the field is where they become measured.

**An instance's key** — the one its envelopes are signed under
(`resource-requirements.md` §3.3) — is provisioned per instance at its
creation, written beside the resource in your hosting file as its public
half, shown on your surface, and never present in a package. You carry the
signature it makes and do not verify it in this version.

## 10. Role assignment and hosted-session lifecycle

**The node evaluates access and presents the result as a credential**
(`resource-requirements.md` §1). The resource never reads network state.

### 10.1 Membership is the outer gate

**Read authorisation state once per request and decide from that snapshot.** A
membership check, an acknowledgement, a role row and an availability flag read at
four different instants can describe a state that never existed, and the request
either was authorised or was not.

**An in-flight request completes under the state it started with.** A row that
changes mid-request terminates the *session* (§10.5), which stops the next request;
it does not reach into one already running, and a node that tried would be tearing
down work whose result the resource may already have committed.

**Evaluate in the order `wire-format.md` §11 gives**: existence, membership,
acknowledgement, the role row, the carried request's own well-formedness, then
availability. **Existence comes first because membership is owner-relative** — a
keyhash you do not host has no owner, so there is no membership question to ask
about it, and it is answered with the same `refused` either way. **Availability
last is the load-bearing part**: a role-holder learns the service is down, and
someone with no role never does, which keeps operational information about the
owner inside the set entitled to it.

**Answer a member specifically and a stranger opaquely.** The gate has already told
you which you are talking to, so use it: a member lacking a `SubtreeAck`, or whose
role row grants no `connect`, gets the reason (`wire-format.md` §11), because they
can act on it and already hold the topology it describes. **You answer from the row,
not by evaluating a predicate** (§10.2). **A non-member gets `refused` for
everything**, including a resource keyhash that names nothing: otherwise a stranger
enumerates what you host by watching which lookups differ.

**Above the patron level, membership alone does not admit.** A node adopted into
your subtree lands inside your trust horizon automatically, which means **a subordinate
can put strangers inside your gate without asking you**. Require your own
`SubtreeAck` (`wire-format.md` §7.5) before granting such a node access to
resources you host (design §11.2.1).

- **Issue it automatically, under a policy the operator set beforehand.** The
  deliberate human act was the *patron's* adoption; propagation of membership within
  the horizon follows from it (design §11.2.1), and your node acknowledges a new
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
- **Accepting one at all is your choice.** The patron's siblings, who hold it, may take one — a
  grandpatron's sibling or the great-grandpatron never holds it (design
  §11.2.1.1) — in place of evaluating a stranger themselves,
  which is the saving it offers. Nothing obliges you, and an operator wanting a
  stricter policy sets one.
- **Treat it as lapsed when the relationship it describes ends**, without waiting
  for a revocation that does not exist. If the acknowledged node's patron departs
  or is disavowed, the acknowledgement described a subtree the node is no longer
  in.
- **It says nothing about identity.** The adoption's signatures carry that. Do not
  let a `SubtreeAck` stand in for evidence about who someone is.


**Current membership in the owner's trust horizon is a precondition for all resource
access.** Every other predicate sits behind it, and no grant of any kind reaches
outside it (`resource-requirements.md` §7.1.1). Departure therefore revokes
everything, uniformly.

### 10.2 Hold a role table; treat predicates as a macro over it

**Materialise role assignments per resource, one row per member of the trust horizon**
(design §11.4). **Authorisation at request time is a lookup**, never a predicate
evaluation; that makes it deterministic, a single table read, and readable by the operator who
configured it. **A row carries at most 64 roles**
(`resource-requirements.md` §3); refuse to materialise one wider [2026-09-02] —
the operator hears about it at configuration time, never a requester at request
time.

**Re-evaluate predicates at four moments, none of them a request**: when an
operator is configuring roles; in a background pass when a node enters or leaves
the horizon; when evidence about a standing member arrives, since the span is
narrow enough that re-scoring on events is affordable (design §11.4); and
periodically for time-dependent predicates, whose values move with time alone —
a scheduled whole-table pass, not a continuous recomputation. A new member is
scored against standing predicates and given rows; a departing one has theirs
removed. **The table is current state, and no history of its rows is kept**
[author, 2026-09-18]: a row replaced is gone, and what a member's roles used to be is a classification
history no lookup needs, the time axis C22's matrix would otherwise gain
(design §19.8).

**Scoring the changed member alone is correct for every predicate class but two,
and both are rank predicates.** A *relative* rank predicate (a percentile, a
median, any quantile) has a cutoff that is a fraction of the population, so a
membership change moves it for **every** row rather than for the entrant. An
*absolute* rank predicate — *my ten most trusted* — has a fixed cutoff and a
contested last place, so an entrant scoring above the line **displaces whoever
held it**, and that member's row changes without that member changing
(`resource-requirements.md` §7.2.1). Re-score the whole table for a relative rank
and at least the displaced row for an absolute one; the changed member alone is
enough for the rest. An implementation that treats them alike
leaves rows stale in both directions: granting where the line has since risen, and
withholding where it has fallen.

**A table change is what terminates sessions** (§10.5 below), not a predicate change
in the abstract. The row moved or it did not.

**Your predicate language is yours, and so is the table.** Neither crosses the wire
and no other party evaluates either — **the network never sees a role**. The user
sees theirs only as the resource's interface reflects it.

**The one contract that must be stable is node-to-resource**: principal, roles,
audience, session (`resource-requirements.md` §2). It must be **legible to any
implementation**, so a package reads the same credential wherever it runs; that is
portability, not interoperability, and it binds you to the packages you host rather
than to another node.

**Mint the session identifier per resource, not per connection.** One caller
reaching three resources over one session gets three identifiers. A single identifier
shared across them would re-link that caller between resources and undo what the
pairwise principal was derived to separate (design §11.0.2) —
`resource-requirements.md` §3 states the resource-facing half.


### 10.3 Predicates

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
**Prefer an absolute rank to a quantile** where the operator's intent is absolute:
*"my ten most trusted"* survives a metric change exactly as well as *"top 20%"* and
depends on nobody but the member and those above them, whereas a quantile's line
moves with the size of the org (`resource-requirements.md` §7.2.1).

### 10.4 Templates

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

### 10.5 Terminate hosted sessions when a principal's roles change

**When a principal's authorisation state changes for any reason, end the affected
sessions rather than notifying the relying party.** Departure, disavowal, a
predicate ceasing to match, an operator revoking a grant, the cause does not
matter, and enumerating causes would mean missing one. Present encoding: drop that
principal's sessions to hosted resources, and **the session dropped is the
resource-facing one** [2026-09-02]. A hosted session is the node-held identifier
under which the resource sees a principal's requests
(`resource-requirements.md` §2), not the caller's rhtn/1 transport session,
which may be carrying sessions to other resources and control traffic besides.
Ending it means retiring the identifier, so the caller's next request to that
resource is evaluated afresh and, where it still passes, arrives under a new
identifier —
which is how the resource observes the change, since there is no teardown
message on the hosting path. The transport session is untouched: an
authorisation change at one resource is not a connectivity event. If the roles
are gone, so is the access. **A request already in flight is not reached into**
(§10.1): the identifier is retired for what comes next rather than withdrawn from
underneath work the resource may already have committed.

**This is local behaviour, not a network promise.** No protocol rule compels it and
none could: the network cannot reach into an operator's node. **For sessions this
node holds it is nonetheless mandatory conduct**: the node is already the party
holding and dropping them, and nothing is gained by making the drop optional
[author, 2026-09-02].

**Termination requires no protocol, no acknowledgement, no latency budget and no
rule for in-flight requests, which is why it is preferred here.** Telling a resource that a principal's roles changed would require a
notification protocol, an acknowledgement, a latency budget, and a definition of
what happens to a request already in flight. Dropping the session needs none of
that: **role changes are enforced by reconnection**, so there is no mid-session
mutation, no partial-privilege state, and no request that begins under one role and
ends under another.

The resource's only obligation is to tolerate a session ending at any time, which
any network service must regardless.

**Brokered resources are outside this** (design §11.2). A non-intermediated
connection cannot be relied on to drop: the node can drop its own leg, but the
client-to-service session is one it never held, so enforcement there is at
establishment, and an existing session continues on the service's terms.

### 10.6 Show the hosting model when binding a resource

**Display whether a resource is hosted here or brokered to an external service.**
It follows from where the resource runs, so nothing needs declaring.

It matters because revocation differs: a hosted package's session terminates at
this node, while an external service continues on its own terms. **That is a
property of the service the operator chose, not a shortfall here.** The operator
should simply know which they are getting.

### 10.7 What accessing users see

- **The roles they hold on each resource.** Not the predicates that granted them,
  which are the operator's business.
- Without this, a user cannot distinguish *denied by policy* from *broken*, and
  will not know that departing a patron cost them access.

---

## 11. Catalog

**Answer catalog queries; do not propagate entries** (design §11.5). A node in your
horizon asks what you have; you return the eligible entries you **hold** — your
own and those registered with you, since ownership decides who signs an entry,
not who answers for it (design §11.5), and that asker may see. Nothing floods, nothing is replicated, and nothing needs invalidating.

- **Accept registrations only from the owner's own session** (`wire-format.md`
  §6, request type 7). An entry is owner-signed and therefore relayable by
  anyone, so accepting one from any peer means accepting a **replayed earlier
  envelope**, and because you keep whichever you applied last, that silently
  reverts the owner's current entry. Possession of a signed entry is not
  authority to install it — only the owner's live authenticated channel is. The
  check is free: the owner is attached to you and the handshake already named it.
- **The requested `discover_scope` is a request.** You compose the answer, so you
  may narrow it or ignore it, and the owner cannot check. An owner who delegates
  hosting delegates this, and should be told so rather than discovering it.
- **Filter at answer time.** `discover_scope` is your local rule for which entries
  go to which asker. **It never leaves your node** — receiving an entry is what
  qualifying looks like, and a field carrying it would tell the asker how they were
  selected.
- **Hold one current entry per resource, whoever owns it.** A `ResourceRequest`
  names the resource and nothing else (`wire-format.md` §11), so if you served two
  claims for one keyhash you would have nothing to pick between them with, and
  picking wrong applies one owner's membership and roles to another owner's backend.
  **Refuse a registration for a keyhash you already serve under a different owner.**
- **Re-registering by the same owner replaces the entry, and the old one is gone.**
  **A registration is not archived** (design §10, `wire-format.md` §6.5): the archive
  advances on adoption, departure, disavowal, peering and presence, and this is none
  of them. Keep a live table of what you currently serve and answer from it. **Do
  not retain superseded registrations**; nobody needs a record of a resource its
  owner no longer runs, and keeping one turns a withdrawal into something a later
  reader can still find.
- **Withdrawal is not a propagated message.** Stop returning the entry and the next
  query gets the truth; there are no copies to invalidate, because you never sent
  any that claimed to be authoritative. **An owner who is not you still has to ask**,
  and does so by re-registering the resource with a requested `discover_scope` of
  self, which no asker but the owner satisfies. What is left is your state to keep
  or drop.
- **A cached answer is the asker's business.** It was true when given, and nothing
  grants access on the strength of it — access is decided by the owner at request
  time (§10.1).
- **Return the owner's signature unchanged, and add none of your own.** An entry is
  signed once at registration and that signature is reused for every answer
  (`wire-format.md` §6.7). It keeps the entry attributable **to its owner** wherever
  it travels; a per-answer signature from you would make one entry's bytes differ
  between askers and would attribute the claim to the wrong party.

---

## 12. What a user can learn about a gateway before using one

- **The declaration reaches them before any connection, and it is a claim**
  (design §11.5, P24). A signed catalog entry carries the owner's declared
  `data_practice` and an in-scope user reads it at catalog lookup, which
  precedes connecting; what the declaration is worth is what design §1.1
  prices — nothing here can enforce it. **A user outside the entry's
  `discover_scope` sees no entry at all**, so the question of evaluating that
  operator does not arise for them: they cannot reach the resource either.
