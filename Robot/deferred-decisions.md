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

## Where the image a launch pulls from lives [raised 2026-11-02, open]

**What prompted it.** Two nodes launched from GXV and neither served. The cause,
read off the second machine's own `cloud-init-output.log`:

```
Trying to pull ggcr.io/mzuech/fuerosnet/rhtnd:latest...
Error: initializing source docker://…/rhtnd:latest:
  Requesting bearer token: invalid status code from registry 403 (Forbidden)
```

**The chain, established on the machine.** The registry answers `401` with
`www-authenticate: Bearer realm="https://gitgud.io/jwt/auth"`, which is the
ordinary anonymous-pull handshake. The engine then asks that realm for a token
and **Cloudflare answers the challenge page** — `Just a moment…`, 403, 5.7 KB of
HTML — which no container engine can pass. A browser user-agent is challenged
too; **a request carrying any credentials is let through**, GitLab then answering
a clean `401 application/json` for wrong ones. So the wall is on anonymous
requests to the token endpoint and nowhere else.

**It is not the image, the tag or the configuration.** `latest` was
`0a94bba6` at the second launch, well past the `host-shell` key — the earlier
suspect — and nothing was pulled at all, so no container existed to refuse
anything.

**And it is not that the registry cannot serve an anonymous pull: it did,
hours earlier the same day** [author, 2026-11-02, correcting the first reading
of this]. The 2026-11-02 entry on the launch's own door records a node launched
from the phone and its *own administration page* opened in the app, which only a
running node serves — so that machine pulled this image anonymously and
succeeded. The first filing of this question reasoned from one observation to a
structural conclusion and skipped the obvious test.

**Tested afterwards, ten times.** Anonymous requests to the token endpoint are
challenged from the laptop five times out of five and from the droplet five out
of five, and a pull of a second tag fails the same way. So it is **consistent
right now and was absent earlier**: not intermittent within this hour, not
particular to one address, and not a permission on the repository — something
changed in gitgud's own protection between the two launches.

**Which is the thing to decide about.** The question is not whether that
registry can serve a pull but whether a launch may *depend* on it: what stands
between an operator and a working node is then a third party's bot-protection
setting, changed without notice, invisible until a machine comes up with no node
on it.

**The decision**: a launch has to pull from somewhere an engine can pull from
without a person, and the options differ in what they ask of an operator who is
not the author.

| | What it costs | What it asks of a third-party operator |
|---|---|---|
| **a. Mirror the released image to a registry that allows anonymous pulls** (Docker Hub, GHCR, Quay), `REGISTRY_DEFAULT` pointing there; gitgud keeps the per-commit builds | an account and a CI push target | nothing: they launch and it works |
| **d. Wait, and see whether the challenge lifts** | nothing | a launch that works or does not by the hour, with no way to tell which beforehand |
| **b. Authenticate the pull** with a registry-scoped read-only token | the token travels in the machine's user data, readable from the provider's metadata service by anything on the machine — the hazard rotation was built for — and the project must not *ship* one (`no-shipped-provider-secrets`, author 2026-10-25) | **paste a registry credential as well as a provider one**, for an image that is meant to be published |
| **c. Carry the image to the machine over ssh** | ~170 MB per launch, and a launch that needs the operator's laptop | a laptop and the key, which the phone-only operator does not have |

**What was done meanwhile**: the image is built from the tree and side-loaded over
ssh so the bench run can proceed — (c), by hand, for one machine. Nothing in the
client changed.

**What changes if it goes the other way**: `REGISTRY_DEFAULT` and the CI publish
step for (a); the launch gaining a pull credential, a field for it, and a
walkthrough like the provider token's for (b). The launch screen already lets an
operator name another registry, so (a) does not take that choice away from
anybody.

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

**What was done meanwhile**: nothing. The node is enrolled and serving with GXV
attached, the three subordinates refused, and the field-test image in place so
the verdicts are readable.

**What would make the next step quick**: an instrument that lists what a client's
archive holds — tx type, subject, counterparty, predecessors — so *which* four
records these are is a command rather than an inference. The sizes (7134, 7170,
7205, 7134) and the verdicts are all the bench has today.

### Asked alongside it [author, 2026-11-03]

*"Can the light client maintain a view of its current trust horizon with the
information about each member needed to allow connection and provision the
current version of this data whenever it launches a node?"*

**It already maintains one, and what it may hand over is records and not the
view.** `rhtn_client::horizon` keeps the horizon so every node in it can be
resolved directly, and its own heading fixes the limit: *"the patron's propagation
is the system of record… a client never presents it as authority, to a user or to
a counterparty."* A node is a counterparty. Shipping the derived view would be
shipping one party's reading of the topology as another party's state, which is
the shared-state reading this design refuses.

**Shipping the records is a different thing and is sound.** Their authority is
their own signers', the node verifies each as it verifies anything, and its view
stays its own — which is also why **binding falls out for free**: a member's
`KeyMaterial` travels inside the records, which is why the four on the bench are
7.1 KB each. A node given them can bind the members they name, and the three
refused subordinates would attach.

**Two things it does not settle.**

- **It does not avoid the decision above.** If a provisioned bundle is applied
  record by record it meets the same deadlock: the two prerequisites and the two
  out-of-store subjects do not care which door they came through. So
  provisioning needs *a* or *c* anyway, and chiefly the willingness to take a
  bounded bundle that cannot be reach-tested yet.
- **It is a bootstrap and not a repair.** It serves the node the operator is
  launching now. A node that moves hosting, a member adopted later, a subnet
  taught by somebody two steps down — each still wants the attach path to work,
  which is the same decision again.

**Where it would travel.** Not the launch script: cloud-init user data is
bounded at tens of kilobytes and four records already fill a quarter of it. The
administration surface is the place — the same channel that already takes the run
and the two signed records at enrolment — so the bundle arrives after the machine
is up, bounded by what the surface takes, and a node whose operator provisions
nothing is unchanged.
