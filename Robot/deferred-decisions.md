# Decisions the author has explicitly deferred

**This file holds only what he has deferred on purpose** [author,
2026-10-16]. A decision that is his gets asked in the output when it
arises — at the moment if it blocks the current task, in the same output
if it does not — and asked again in every output until he answers.
Nothing arrives here because it was inconvenient to ask; the rule is in
`CLAUDE.md`.

Each entry is written to be decided **cold**, months later:

- **What prompted it** — the work that ran into it.
- **The options**, as narrowly as they can be put.
- **What was done meanwhile**, so nothing is blocked on the answer.
- **What changes if it goes the other way** — the cost of having waited.
- **When it arose**, and when it was deferred.

---

## The presence figures of design §7.5, under the four-minute window

**Deferred** [author, 2026-10-24]: "there's no sense trying to tune the
presence thresholds until we move to the 4 minute timer, as what feels
reasonable from a user standpoint will undoubtedly shift at that point."

- **What prompted it.** A6 on the bench (2026-10-24): the capture loses
  1.6–3.7 s of a 15 s window to the camera's first frame while the
  detector samples frames that do not exist yet; the NFC latch is 4 s; a
  user has to move from tap position to filming position inside those
  windows; the continuity check is off on the field-test build
  (`device.rs`, window 0 [author, 2026-10-18]) because the 90 % figure is
  design §7.5's ratio for four minutes and over ten seconds is a different rule.
  The author's reading: "a handful of spot checks over the course of
  minutes with a forgiving rule as to presence; just enough that it wasn't
  safe to do the initial exchange and walk away."
- **The options.** (a) Restate design §7.5's figures now, against the 30 s
  field-test window; (b) restate them once the witness window is the
  design's 240 s and the post-optical regime is one concurrent state, from
  what runs on the bench then; (c) leave design §7.5 as written and carry the
  field-test relaxation as a build difference indefinitely.
- **What was done meanwhile.** (b) in effect: design §7.5 stands as written; the
  field-test build keeps continuity off and the 30 s window; the regime
  restructuring proceeds first (the author's third ruling of the same day).
- **What changes if it goes the other way.** (a) would tune thresholds to
  a window the author has already said will not be the one users feel, and
  would be redone; (c) leaves the releasable rule untested on hardware.
- **Arose** 2026-10-24, during A6; **deferred** 2026-10-24.

---

## A party's refusal to open a direct path

**Deferred** [author, 2026-10-24]: the direct path is between contacts — "a
client may dial anyone for whom they hold a contact, the recipient will
upgrade to a direct connection if they hold the sender as a contact modulo
any blacklisting process we wish to implement in the future."

- **What prompted it.** Moving the direct-path bound from the horizon to
  contacts (design §12.6.3), after run `58a6883-a5-2` showed every witness
  leg relayed through the node and cut off by its request allowance.
- **The options.** (a) No refusal beyond the contact rule: a contact may
  always open the path; (b) a per-party refusal the person sets — a
  blacklist — read by the recipient's gate; (c) a refusal that also drops
  the contact, so the party is no longer dialled either.
- **What was done meanwhile.** The gate is the contact rule alone; the
  design says a party may decline and that no mechanism yet specifies how
  (§12.6.3, P17). The transport already honours a local refusal
  (`adaptors/tests/direct.rs`, TRV-08), so (b) is a table and a screen, not
  a protocol change.
- **What changes if it goes the other way.** Nothing on the wire; the
  recipient's gate reads one more table.
- **Arose** 2026-10-24; **deferred** 2026-10-24.

## The production value of the peer contact interval

**Deferred** [author, 2026-10-25]: "It is my intention to de-rate the
peer recovery timing when we approach release. Peering should not be a
bandwidth intensive feature and recovery of contacts is not very time
sensitive. I am leaving the contact interval at 1 minute for now for the
same reason the witness window is at 30 seconds; individual test cycles
would be prohibitively slow if these parameters were at their production
values. Likewise, it is fine if contact recovery from peers dribbles in
over the course of a day or so after loss of on-device storage."

- **What prompted it.** The peer backup exchange (design §6.3,
  `wire-format.md` §7.10.3) landed 2026-10-24 with
  `PEER_CONTACT_EVERY` at a minute in the field-test flavour and fifteen
  minutes shipping, the shipping figure chosen by the assistant as a
  placeholder.
- **The options.** A figure in hours, with the reconnect trigger kept
  (the network coming back still contacts at once); a figure that scales
  with the number of peers held, so a user with many peers costs the
  network no more than one with few; or a randomised interval around a
  figure, so a population's rounds do not align.
- **What was done meanwhile.** Fifteen minutes stands in the shipping
  constant and is marked a placeholder in the code and in design §6.3; the
  field-test build keeps a minute.
- **What changes if it goes the other way.** Nothing structural: the
  constant and one sentence of design §6.3. The author's expectation to carry
  into the choice: active users will hold several peers, which multiplies
  the rounds and multiplies the recovery rate alike.
- **When it arose.** 2026-10-24 (the build); deferred 2026-10-25.

---

## What a member is told when a package's request-rate budget is spent

**Deferred** [author, 2026-10-31]: the three assumptions of the utility
build were accepted as they stood, this among them — the budget is
recorded and not enforced until there is an answer to give.

- **What prompted it.** The host surface's manifest (`infra-client-requirements.md`
  §9.3, `resource-requirements.md` §8) names five budgets, and four are
  enforced: the block's bytes, the log's bytes, jobs in flight and job
  seconds a day. `requests_per_minute` is recorded and not enforced,
  because enforcing it is the one budget whose refusal a *member* sees
  rather than the operator, and `wire-format.md` §11's six statuses were
  written before the budget existed. A bound on fetches a minute does not
  exist in `infra-client-requirements.md` §9.3 at all and would raise the
  same question.
- **The options.** (a) Reuse status 2, *resource unavailable*, which is
  true and tells the member nothing to act on. (b) A seventh status for a
  rate refusal, which is a wire change and tells a member precisely what
  a stranger should not learn about a resource's traffic — though that
  section's own rule is that a member inside the owner's horizon is owed the
  specific reason. (c) The host composes an HTTP 429 inside status 0, so
  the refusal is the application's shape and the network says *delivered*,
  which is what it did. (d) Leave it to the package, whose own accounting
  sits above the host's ceiling, which is what `infra-client-requirements.md` §9.3's "treat the rest as
  the package's own accounting" already permits.
- **What was done meanwhile.** Nothing enforces it; the field is read,
  validated and carried. `infra-client-requirements.md` §9.3's "you bound
  what you have built, name the
  one exceeded on your operator's surface" accommodates this without a
  change, so no document says *not yet*. The audit's owed list points
  here.
- **What changes if it goes the other way.** (a) and (d) are a few lines
  in the node's gateway. (c) is the same plus a composed response, and
  sets a precedent for the host answering as the application. (b) is
  `wire-format.md` §11's status list, its normative evaluation order, the
  client's reading of it, the Kotlin binding and the vectors — the only
  option that is not cheap, and the reason to decide before anything
  starts counting.
- **When it arose.** 2026-10-30 (the budgets landed); deferred 2026-10-31.

---

## How a node is taught a subnet older than itself [raised 2026-11-03, open]

**The bench established it with the field-test node's own log.** GXV attached and
replayed four transactions; the node judged each and stored none:

| bytes | verdict | `rhtn_node::store::Decision` |
|---|---|---|
| 7134 | `OutOfStore` | *the subject lies outside this node's own `h_store`* (`wire-format.md` §10.1.1) |
| 7170 | `Held(transaction)` | *held for a prerequisite; nothing is forwarded meanwhile* |
| 7205 | `Held(transaction)` | the same |
| 7134 | `OutOfStore` | outside `h_store` |

Afterwards: `attached 1`, `subordinates 0`, `holding 1 topology object(s), 0
record(s)` — the one object being the operator-signed endpoint record from
enrolment, not anything taught.

**The deadlock, stated.** The two held records wait for a prerequisite, and the
records that would supply it are among the two just refused for being out of
store. So the edges that would bring a subject inside the horizon are judged by a
horizon that does not yet contain them. **An empty node cannot be taught by a
replay evaluated record by record**, which is what the teaching of 2026-11-01
does.

**And it is why the three subordinates cannot attach at all.** With no records,
the node can bind only the identity it pins from `operator.material`; every other
member is `Refusal::Unbound` — *"presented key bound by nothing"* — so the attach
that would teach it is refused, and the one client it does admit teaches it
nothing that survives the store rule.

**The decision**: what admits a replay from a client the node has already bound.
The store rule exists so a node does not hoard the world (`wire-format.md` §10.1.1), and a bounded
replay from an attaching member is a different case from a flood.

| | What it changes | What it costs |
|---|---|---|
| **a. Admit the replay as a unit** — evaluate what the attaching client sent for internal consistency, apply what hangs together, judge reach afterwards | the teaching works for any member, which is what makes a subnet survive a change of hosting | a second admission path beside the per-object one, and a bound on how much one attach may teach |
| **b. Privilege the operator's own replay** — the client whose `operator.material` this node holds is the authority it serves under, so its records skip the reach test | smallest change; the operator can always repair their own node | only the operator can teach, so a subnet is taught only what its operator holds — and a member two steps down is invisible until its own patron attaches, which needs the same fix one level along |
| **c. Order the replay topologically** and let each record extend the horizon before the next is judged | possibly nothing else needed, if order is the whole of it | the log shows two records refused on **reach** and not on order, so this looks necessary and not sufficient |
| **d. Provision the node with the subnet** at launch | no protocol change | an operator's client would hand over records the node could have been taught, and a node that moves hosting still cannot be taught by the members it serves |
| **e. The horizon is a table the operator maintains** — CRUD on the administration surface, one row per member, no justification carried | the replay stops being how a node learns at all; teaching works for any member, two devices converge, and node-to-node merges are the same operation | the node holds no evidence for what it was told, so it can neither forward that topology nor prove a membership to a third party; and a row needs a version, or two devices resurrect each other's deletes |

**(e) is the author's stated direction** [author, 2026-11-04]: *"I want the
trust horizon to function more like a database table on both the node and the
light client… the light client posts an insertion for each held horizon
member, then later may post an update or a delete, all without needing to
express to the node the reason or justification for the change."* The entry
stays open for the four questions below it raises, not for the direction.

**Why it fits rather than fights the design.** The node runs on
`operator.material` under a delegated credential — verified on the bench
2026-11-04, its `operator.material` hashing to GXV's own keyhash — so from
the network's point of view it *is* its operator (author, on this run: "the
operator's node is identical to the operator"). The per-record reach test
treats it as a stranger to be convinced by evidence, when it is an agent
acting under the authority whose table it is being handed. Authority comes
from the channel here, as it already does for the three signature-carrying
routes; the payload needs to carry no reason.

**Most of it exists.** `rhtn_archive::topology::Table` is the table, held in
`NodeView` as `table` beside `store`, and `Table::clone_for(me)` already
copies one holder's bindings into another's, reprojected — used by
`node/runtime.rs`, `daemon/administration.rs` and four tests, and documented
as "what a test does when both learned the same transactions". What is
missing is a wire form, a route, and a merge rule, not a subsystem.

**A row is `Binding` plus key material.** `Binding` carries `node`, `patron`,
`series`, the `adoption` txid, `from` (the patron's clock), `end`, `anchor`,
`slot` and `evidence: EvidenceStatus` — already enough to merge on
`(node, patron, series)` without any replay order, and `EvidenceStatus`
already admits a binding known without its proof. **Material is the
addition, and it is not optional**: a keyhash is taken over the classical and
post-quantum keys together, the transport presents only the 32-byte classical
key, and `Pins::keyhash_for_key` resolves it by `classical_member(km) ==
Some(key)` over `(keyhash, material)` pairs. So a keyhash alone cannot
authenticate a presented key, which is what `Refusal::Unbound` was on this
run. **This is what field 5 is for**: an adoption carrying the adopted node's
key material is how a row acquires the one thing the node must have.

**Chunking is answered** [author, 2026-11-04]: one row per post, ~2 KB, so
`wire-format.md` §9.2's 256 KiB request-frame bound is never approached and
this does not wait on A3b's question.

**What (e) leaves open, and none of it is rhetorical:**

1. **Propagation and proof.** A node told a row holds nothing to forward and
   cannot show a sceptic why a member is a member. Is an operator's node a
   propagation participant for topology at all, or does the evidence live
   with the members' own clients while the node only serves them? This
   decides whether the table is the node's whole topology or a projection
   beside the store.
2. **Acknowledgements.** `NodeView::ack_policy` has the node acknowledge
   adoptions of its subordinates under its delegated credential — an
   attestation to others. May it sign one over a binding it was told about
   and holds no evidence for? It is vouching on its operator's word, which
   may be exactly right, but it is an attestation leaving the node.
3. **A row's version.** CRUD with no justification has no ordering of its
   own. Two devices, which the author names as a motivating case, will
   resurrect each other's deletes unless each row carries a monotonic
   version the operator sets. Needed even single-device if posts can arrive
   out of order.
4. **Divergence from the records.** A delete removes access; a departure or
   disavowal is a signed transaction. The operator may delete a row the
   network's records still support, or hold one they do not. Every node's
   view being its own, this is probably allowed — but it should be allowed
   on purpose.

**Not yet in any root document**, deliberately: this would be an
administration-surface act (`infra-client-requirements.md` §8.2) and touches
`light-client-requirements.md` §10, and the four questions above change what
those would say.

**What was done meanwhile**: nothing. The node is enrolled and serving with GXV
attached, the three subordinates refused, and the field-test image in place so
the verdicts are readable.

**What would make the next step quick**: an instrument that lists what a client's
archive holds — tx type, subject, counterparty, predecessors — so *which* four
records these are is a command rather than an inference. The sizes (7134, 7170,
7205, 7134) and the verdicts are all the bench has today.

### What the fix proved, and the second gate behind it [2026-11-03]

**Built, deployed to the node and the phones, and the live node settles the
question.** A push from the node's own identity is now judged against its own
state, and the held set drains until a pass stores nothing
(`infra-client-requirements.md` §4.4, OPS-026, PRP-30). Both halves were
necessary and the verdicts prove **neither was sufficient**: GXV replayed, and
the node's log reads exactly as before.

**Because two of the four were never reach refusals.** `TYPE_PRESENCE` is 5 and
`is_topology_class` takes only adoption, departure, disavowal, peering and
reissue — so a presence record returns `OutOfStore` from the *type* test at
`store.rs:564` and not from `stores()`, and no horizon can change that. The
topology store is for topology objects; it is right to refuse them.

**So the chain is:**

| | |
|---|---|
| 2 records of 7134 bytes | presence records — refused on **type**, correctly, and never stored |
| 2 records of 7170 and 7205 bytes | adoptions or peerings — **`Held` for a signer's key material** |
| the key material | carried by the presence records the store will not take |

**The node's `ids` set is the thing that is empty**, not its store.
`release_pending` frees a held object when `ids.identity(k).is_some()`, and a
node learns identities from `operator.material`, from delegations and from the
topology objects it stores. **A member's key material arrives in the ceremony
record, which is not a topology object** — so the adoptions naming them stay
held for ever.

**Ruled** [author, 2026-11-03]: *"learn the key material from the operator's
replay."* Built the same day — `Record::carried_identity` reads field 5 and
accepts it only where it hashes to the subject the record names, the push
handler harvests it from a push whose sender bound as the node's own keyhash,
and the held set settles afterwards because a key learned is an arrival
(`infra-client-requirements.md` §4.4, OPS-027, PRP-31). **This question is
closed**; what follows is the record of what was weighed.

**The decision this left**, and it was narrower than the one above:

| | What it does | What it costs |
|---|---|---|
| **a. Learn key material from the operator's replay whatever the object's class** — a presence record pushed by this node's own identity yields its parties' `KeyMaterial` into `known` and is then discarded, never stored | smallest change that completes the path; nothing enters the topology store that does not belong there | the node holds identities it has no topology object for, which is a set to bound and to expire |
| **b. Carry the material on the adoption** — `Adoption.key_material` exists and is `None` on the bench's records | the topology object becomes self-sufficient, which is what a node needs it to be | a wire-level choice about what an adoption carries, and it does nothing for subnets already recorded |
| **c. Let the node fetch it** — `wire-format.md` §7.9 has a participant's archive serve a record on request | no new object anywhere | the party to fetch from is reached through the node that is down, which is `wire-format.md` §10.1.1's own objection to fetching |

**What is not in question**: the reach fix and the drain both stay. They were
real defects, they are fixed, and this one sits behind them rather than instead
of them.

### Asked alongside it [author, 2026-11-03]

*"Can the light client maintain a view of its current trust horizon with the
information about each member needed to allow connection and provision the
current version of this data whenever it launches a node?"*

**Yes, and it already maintains the view.** `rhtn_client::horizon` keeps the
horizon so every node in it can be resolved directly.

**Corrected** [author, 2026-11-03]: *"the operator's own node is not a
counterparty. The operator's node is identical to the operator from the network's
point of view."* This entry first reached for `horizon.rs`'s rule — *"a client
never presents it as authority, to a user or to a counterparty"* — and called the
node a counterparty. It is not one. An instance runs on a credential its
operator's client signed and holds no seed (design §23.3); it speaks as its
operator, and the network sees one identity. **So this is not two parties sharing
state; it is one identity's state on two devices**, and nothing in the no-shared-
state rule touches it.

**What travels is therefore the view and the records it was folded from.** The
view because the node may simply have it, the records because they are what
carries each member's `KeyMaterial` — which is why the four on the bench are
7.1 KB each, and why a node given them can bind the members they name. The three
refused subordinates would attach.

**Corrected too** [author, 2026-11-03]: *"a node that moves hosting is a newly
launched node with some preloaded data. The provisioning and auth process will
have to be re-run."* So the bootstrap/repair distinction this entry drew
collapses: **there is no rehosting case separate from a launch**, and
provisioning is not a stopgap in front of the attach path but the ordinary way a
node comes to know its subnet.

**Which narrows what the deadlock above is still about.** Not the operator's own
node — that one is provisioned. It is about being taught by a **member**, and
even that narrows: an operator's horizon reaches two steps, so what a member
holds that their patron does not is mostly outside the node's own `h_store`
anyway. **The teaching at attach is a top-up, and provisioning is the mechanism**
— the inverse of the order this entry first assumed.

**But the deadlock still has to be answered**, because a provisioned bundle
applied record by record meets it unchanged: the prerequisites and the
out-of-store subjects do not care which door they came through. What provisioning
needs is exactly what *a* describes — the willingness to take a bounded bundle
from the operator's own client and judge reach after applying it, not before.

**Where it travels.** Not the launch script: cloud-init user data is bounded at
tens of kilobytes and four records already fill a quarter of it. The
administration surface is the place — the same channel that already takes the run
and the two signed records at enrolment — so the bundle arrives after the machine
is up, bounded by what the surface takes, and a node whose operator provisions
nothing is unchanged.
