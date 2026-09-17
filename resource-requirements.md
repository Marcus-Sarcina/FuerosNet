# Resource — requirements

**What a package must do to run on an infra node and be reachable through it.**
Companion to `network-design.md`, which is authoritative on protocol; this document
is authoritative on what a resource must implement.

**Split of responsibility.** Operator-side *obligations* — what a node must do to
host, confine, and bind roles — are in `infra-client-requirements.md`. This document
states the **resource's side of the same boundaries**: what a package must declare,
tolerate and accept. Where both documents discuss packaging, sandboxing or roles,
they are describing the two halves of one interface, not repeating each other.

Design rationale is in design §11.

**This document states no protocol rules of its own.** Where it repeats one for context it cites the section that owns it, and the cited document governs on any disagreement. It cites design sections rather than
restating them, because a restated fact is one that will drift.

**On the force of these requirements**: commitments, not enforceable rules —
`network-design.md` Appendix A.2 states why, once, for all three requirements
documents. Where a requirement leaves a visible artifact, that is noted in place.

---

## 1. How a resource is reached

**The infra node is the front door; the resource is behind it.**

This is an authenticating gateway — a reverse proxy where the node carries the
resource's traffic, a broker where it authenticates and hands off (§3).
Implementers have priors for that arrangement, which is the whole benefit claimed
for it, and the novelty stays confined to *how the gateway decides*, which is
where the novelty belongs.

**The resource never reads network state.** The infra node evaluates access and
presents the result as a credential; the resource sees an authenticated principal
holding roles, exactly as a SaaS product sees a user arriving through a corporate
gateway. A compromised resource therefore leaks its own data, not the owner's
archive.

This dissolves the read-surface question rather than answering it: a resource
never reads owner state directly, so no read-scoping interface — a
security-sensitive surface — needs to exist at all.


## 2. What the gateway presents

The credential a resource receives, and the whole of what it knows about a caller:

| Field | Contents |
|---|---|
| **Principal** | A **pairwise identifier**, `SHA-256("rhtn/1:pairwise" \|\| resource_keyhash \|\| user_keyhash)` — stable per user per resource, and not the user's network keyhash (design §11.0.2) |
| **Roles** | The role names this principal currently holds, from those the package declared (§7) |
| **Audience** | The relying party it was minted for. **An authorisation assertion is valid only there; one naming a different party MUST be rejected** (design §11.0.4) |
| **Validity** | The session. Role changes end the session rather than mutating it (design §11.4) |

**A resource sees nothing else.** No topology, no archive, no keyhash, no
membership predicate, and it cannot ask. The node evaluated access; the resource
receives the result.

**Per-user functionality works normally**: the principal identifier is
deterministic, so ownership, history and preferences behave as with any stable user
id. Most applications need to distinguish users holding identical roles, and this
does.

---

## 3. How a request arrives

**There are two legs, and only the second speaks HTTP on the wire**, and conflating them produces a
claim that cannot hold: HTTP/3 is selected by ALPN token `h3`
at QUIC connection establishment absent some other selection mechanism
(RFC 9114), while the client's session already
negotiated `rhtn/1`. You cannot layer standard HTTP/3 onto that connection.

| Leg | Transport | Framing |
|---|---|---|
| **Client → node** | The existing `rhtn/1` QUIC session | A `ResourceRequest` on a **new bidirectional stream**, request type 6 (`wire-format.md` §11), not a stream-0 control frame. No HTTP transport — the application request rides inside the frame as a serialized HTTP/1.1 message (`wire-format.md` §11) |
| **Node → resource**, *where the node carries the traffic* | A local socket for a package hosted on the node; **HTTPS, required**, where it crosses a network | **Ordinary HTTP**, carrying the headers below |

**The second leg often does not exist at all.** design §11.7's
default is **broker rather than proxy**: the node authenticates and hands off, and
the user's client connects to the service itself. **Do not assume a client's
resource traffic passes through its node** — a light client runs on an
ordinary network-enabled device and can make its own outbound connections, and an
adaptor that makes the node a third-party authenticator to a directly-attached
service is exactly the single sign-on shape §10 describes.

**Where the node does carry the traffic, HTTPS is required wherever that leg crosses
a network**, and plain HTTP is permitted only on a local socket to a package the
node hosts itself. The node already reads the request — it inserts the credential —
so what the far leg protects is everyone *else*: an operator relaying to a resource
elsewhere must not put an authenticated principal and an application body on the
open network.

**The node is a reverse proxy that authenticates; the resource is an origin server
behind it.** And the proxy's two sides speak different protocols, which is what a
proxy is for. Nothing requires the client's transport and the resource's transport
to be the same, and requiring it was the error.

**The credential travels as request headers**, inserted by the node:

| Header | Contents |
|---|---|
| `rhtn-principal` | Pairwise identifier, base64url (§2) |
| `rhtn-roles` | Role names held, comma-separated. **May be empty** |
| `rhtn-audience` | This resource's keyhash, a request naming another MUST be rejected |
| `rhtn-session` | Session identifier, so a resource can correlate requests and notice a session ending |

**The roles you receive are application actions.** `discover` and `connect` are
reserved for the node's own evaluation (design §11.4) and never appear here — a
request reaching you is what a `connect` grant looks like, and you are not present
for a discovery decision. **Do not expect them, and do not treat their absence as a
missing grant.**

**So `rhtn-roles` may be empty, and that is not an error.** `connect` is the
gate and it is spent getting the request to you; the application roles are what you
were given on top of it. **An empty list means a caller the node admitted and to whom
the operator has granted nothing further**: decide for yourself what that principal
may do, exactly as you would for any role set you do not recognise. It does not mean
the node failed to populate the header.

**Header encoding.** No value carries characters needing escaping:
`rhtn-principal` and `rhtn-audience` are base64url keyhashes, `rhtn-session` is
a base64url opaque identifier, and `rhtn-roles` is RFC 9110 list syntax — a
comma-separated list of role names, each an HTTP `token`, the comma being the
list delimiter rather than part of any name. **base64url here is RFC 4648 §5 without padding**: no trailing
`=`, since a strict parser given the other spelling rejects bytes that decode
identically, and one of the two had to be named. **Role names are `[a-z0-9_-]`, 1–32 bytes, matched
byte-for-byte** — no case folding, no escaping, and no comma admitted. **They
form a set** [2026-09-02]: duplicates carry no meaning and a conforming node
does not emit them. **A principal holds at most 64 application roles per
resource** [2026-09-02] — bounding the header near 2 KB, inside common HTTP
stack limits, and the bound is enforced where it can be seen: a node refuses
to *materialise* a row it could not encode, so the operator hears about an
over-wide role assignment at configuration time and no request-time failure
code exists, because none can occur. **Their
order is not significant and carries no information**, so do not key a cache on the
header's bytes; match names, not the string.

**A session identifier is opaque to you and unique within the issuing node.** Do not
parse it, do not assume a width, and do not treat it as globally unique — it
identifies one node's session and nothing else.

**It is also per resource.** One caller's session with a node yields a
*different* identifier for each resource it reaches, on the same reasoning as the
pairwise principal (design §11.0.2): an identifier common to two resources would
re-link the same caller across them and undo the separation the principal was
derived to create. Two resources comparing notes learn nothing from it.

**You are not notified when a session ends.** There is no teardown message on the
hosting path; you observe it as requests ceasing to arrive under that identifier
(design §11.4). **Do not hold state that requires a close signal.**


**Responses are ordinary HTTP.** The node relays them; it does not interpret them.

### 3.1 The header-spoofing hazard

This pattern has one characteristic failure, so both halves are
stated as requirements:

- **The node parses the HTTP message and re-serialises it; it does not forward your
  caller's bytes** (`wire-format.md` §11.2). Anything ambiguous is rejected rather
  than normalised, and exactly one message is emitted per request. **You receive what
  the node's parser produced**, which is what makes the header replacement below
  meaningful.
- **An authenticating intermediary MUST remove every caller-controlled value in the
  namespace it uses for trusted identity or authorisation assertions, before
  inserting its own.** Present encoding: the node strips inbound `rhtn-*` headers.
  A client that sets `rhtn-roles: admin` and has it forwarded has defeated the
  entire gateway.
- **A component behind an authenticating intermediary MUST NOT treat
  intermediary-asserted metadata as authentic unless it can verify the path the
  request arrived by.** Present encoding: do not trust `rhtn-*` headers on any path
  other than the gateway's.
  If a resource is reachable directly — bound to a port, on a shared host. It must
  either refuse such connections or ignore the headers on them. **Being behind a
  gateway is a deployment fact, not a property the resource can verify from the
  request.**

**Where a resource cannot guarantee it is unreachable except through the node**, it
should require a shared secret established at package installation, so
gateway-inserted headers are distinguishable from client-supplied ones.

### 3.2 Why HTTP rather than something purpose-built

- **HTTP is what the resource end already speaks.** The node-to-resource leg is a
  fresh connection regardless, to a local socket or an external vendor, so HTTP
  there costs nothing a bespoke protocol would save, and buys every existing
  server, library and debugging tool.
- **It is what resources already speak.** SaaS, local services and the IdP plugin
  case (§5) all expect HTTP, and the plugin needs it regardless for SAML or OIDC.
- **The request path carries no trust** (design §11.0.3), so nothing here needs
  signing, canonical encoding or domain separation. A purpose-built framing would
  buy properties this path does not require.

---

## 4. What a resource is

**Physically:** a package running on the same infra node as the network services.
For an external service, **the conforming component is the local adaptor or
gateway.** The vendor's own system sits behind it and conforms to nothing here.

**Logically:** a black box exposing a set of **roles**, accepting the infra node's
credential to access those roles.

**Three categories, all the same mechanism:**

| Category | Example | Notes |
|---|---|---|
| **Local application** | Team datastore, tracker | Uses the infra node's own storage and compute |
| **Gateway** | Homeserver bridging to another distributed system | See §11 — re-concentrates what the architecture disaggregates |
| **External service** | Commercial SaaS | Sees the inbound connection as it would a user arriving via a corporate intranet gateway. **The vendor need not join the network** |

The third case is the one a commercial vendor can act on without joining anything: a vendor
consumes subnet membership as a credential without participating. That is a far
easier adoption story than asking vendors to join, and probably deserves its own
vignette.

### 4.1 Wider reach is federation, not wider scope

**A resource is neighbourhood-scale; a resource application can be any scale.** A
**resource application** is the wider system your package is one instance of (design
§11.0.1). §7.1.1 bounds a resource to its owner's trust horizon. **That is not a ceiling
on what can be built**: a subnet-wide or cross-subnet service is **many local
instances**, each hosted by a patron, each administering its own team, with the
resource application handling instance-to-instance connection in its own
architecture. It is the same shape as
§4's gateway category at a different scale — one mechanism, not two.

**What this means for a package author.** Do not design for a single wide-scale
deployment and expect the network to express its access control; it never sees a
wide-scale audience. Design for one instance, and let the application federate.

**design §11.0.1 carries the argument**: the three properties that follow from
this, and the cost, which is that every patron running an instance becomes an
operator.

> **Assumption — resource-layer sibling of A11.** *Patrons will actually do this
> work.* A11 says ordinary users tolerate ceremony friction rather than routing
> around it; this is the same claim one layer up, about operators rather than
> participants. If patrons will not administer resources, the federation pattern
> does not happen and applications either stay local or route around the network —
> and if they route around it, the network's whole product argument (design §1) goes
> with them. **Belongs in design §20.2 when this propagates.**

## 5. Conformance: extension support is not arbitrary-program support

**Distributing a server that accepts extensions does not mean it accepts any
program.** A resource must be built or adapted to this network's requirements:
tolerate the sandbox, expose legible role definitions, accept the infra node's
credential. **There is development overhead to make a given service work here, and
it should be stated plainly rather than discovered.**

**The constraint is what makes the model work, not a concession.** A package that
cannot declare its roles gives predicates nothing to bind to (§7); a package that
cannot tolerate capability-based confinement cannot be contained (§9). Accepting
arbitrary binaries would forfeit both properties, and they are the ones the whole
design rests on.

**Four adaptation paths, with very different costs:**

| Starting point | Path | Cost |
|---|---|---|
| **New software** | Built to the package contract directly | Lowest, the contract is the target |
| **Existing local software** | Port: package format, role declaration, sandbox tolerance | Moderate. The sandbox is the part most likely to require changes, since it constrains filesystem and network access the software may assume |
| **Existing SaaS** | Adaptor brokering the gateway credential to the vendor's own auth | **Potentially very low; see below** |
| **Existing distributed system** | Homeserver or bridge (§4's gateway category) | Highest, and inherits §11's operator asymmetry |

**The SaaS case is much cheaper per vendor than a bespoke integration, and this
matters for adoption.**
The shape is identical to enterprise single sign-on: an identity provider asserts
a user with attributes the service reads as roles or groups, there being no claim
vocabulary every service shares. **The vendor
needs only the standard protocols it already supports.** The infra node is simply
an unusual IdP, and role assignment (§7) maps onto group claims the vendor already
understands.

**Someone must write the identity-provider plugin for the infra server, and that is
largely a one-time cost rather than a per-vendor one.** One conforming IdP package
covers the standard federation surface for any SaaS supporting SAML or OIDC.

**It does not reduce per-vendor work to zero.** Vendors differ in claim mappings,
tenant configuration, provisioning expectations (SCIM or otherwise), proprietary
policy hooks, and occasionally commercial restrictions on which identity providers
they will federate with. **Standard federation support turns much of an integration
project into configuration** where a vendor supports it well. It does not make one adaptor work unchanged
everywhere.

The burden still falls mostly off the vendor, which is what makes the commercially
legible case (§4) the cheapest of the three to reach.

### 5.1 What cannot be ported: software that must own the user's device

The hard limit is **not** server-side. A service needing raw hardware, kernel
modules or unrestricted host access simply is not a candidate for hosting inside
the infra node, and that is an ordinary packaging constraint.

**The unportable class is software requiring privileged access on the *user's*
device.** The exemplar is **an online game with a kernel-level anti-cheat
component.** The network can broker identity; it cannot broker *"install a rootkit
on your machine."* That request sits entirely outside what this network mediates,
and no adaptor bridges it.

**The incompatibility is philosophical before it is technical**, which is why no
amount of engineering closes it. Anti-cheat exists because **the game does not
trust the player** and needs adversarial inspection of their machine to function.
This network's whole premise is that trust flows from real relationships between
people who have met. A system whose security model requires treating the user as
the adversary is **running the design's central assumption in reverse**, and would
not become compatible even if the sandbox permitted it.

Same category, for scoping expectations: DRM requiring hardware attestation,
anything depending on remote attestation of the client platform, software requiring
exclusive control of a secure element.

### 5.2 The middle band: systems that resist brokered login

Between the SSO case and §5.1's rootkit case, both above, sits a
band of systems whose **session-creation** resists brokering. These are not
philosophical incompatibilities; they are concrete mechanisms that assume a direct
vendor-to-user relationship.

**First, a distinction §5's "broker" category blurs — auth path versus data
path:**

| | **Broker (auth path only)** | **Proxy (data path)** |
|---|---|---|
| What the gateway does | Authenticates, then hands off | Carries all traffic |
| Vendor sees | The user's own connection | **The infra node** |
| Privacy from the vendor | Low, the vendor sees the user directly | High — user IPs hidden |
| §11 exposure | Low | **High, the node becomes a content chokepoint** |

**Default to brokering.** Proxying hides users from the vendor, which some
operators will want, but it makes the infra node a data chokepoint — precisely what design §14.2
works to prevent for payload, and it breaks the systems below.

**What resists brokering specifically:**

- **Vendor-issued client credentials.** MTLS where the service is its own CA,
  device-bound tokens, per-client API keys. **The gateway has nothing to present**,
  because the secret was issued to the user by the vendor and the gateway is not in
  that relationship.
- **RP-bound credentials** — WebAuthn and passkeys are scoped to a **relying-party
  ID**, with the client-data origin authenticated and checked separately. A gateway
  that transparently preserves the vendor's origin does not inherently break this;
  **one that substitutes its own RP ID or origin does**, which is the entire point
  of the binding. In practice a broker that terminates the vendor's origin cannot
  proxy the ceremony.
- **Client-fingerprint or IP-bound anti-fraud.** Under proxying, every user of a
  subnet arrives from one address and looks like one machine, which reads as
  exactly the pattern such systems exist to flag. **Brokering avoids this; proxying
  makes it worse the more successful the subnet is.**
- **Proprietary or undocumented auth.** Not impossible, just bespoke per vendor,
  which forfeits §5's one-plugin-serves-everyone economics.
- **Stateful sessions with server affinity**, where a session cannot be resumed
  through a different path.

**The useful test is narrower than §5.1's.** That one asks *does this require
adversarial access to the user's machine?* This one asks: **does session creation
assume a credential or property the gateway does not and cannot hold?** If yes, the
service is reachable only by the user connecting directly, which the network can
still help with, by carrying identity and discovery while staying out of the
session itself.

*(design §7.8 does define optional client-integrity attestation, but deliberately as
**evidence a party may weigh**, never as a gate. A resource could ask for it and
weigh its absence; it cannot compel it, and that is the same distinction design §1.1 draws
everywhere else.)*

**Two conformance levels worth distinguishing**, because they have different trust
properties:

- **Hosted package.** Runs inside the infra node's sandbox, uses its storage and
  compute. Fully contained by §9.
- **Broker.** A thin adaptor that authenticates a principal and hands off to an
  external service. **The sandbox constrains the broker, not the service behind
  it**, so §11's asymmetry applies: the user's data goes somewhere the infra node
  does not control and cannot vouch for.

---

## 6. Registration versus the catalog page

| | `CatalogEntry` (design §11.5) | Catalog page |
|---|---|---|
| **What** | Network-layer registration | Session-time view |
| **Signed** | Yes, by the owner | No — served over an authenticated session |
| **Propagates** | **Not at all** — held by the hosting node and returned on request (design §11.5) | Not at all |
| **Uniform** | Yes, same for everyone | **No — personalised, names the viewer's roles** |
| **Purpose** | Makes a resource addressable: reachable point-to-point, or able to call out to a node | Discovery, and the front door for locally-hosted interaction |
| **Required** | **No** | For any resource a user interacts with |

**A locally-hosted application may need no `CatalogEntry` at all.** You reach it
by asking your infra node. A gateway, or a resource that initiates contact with
nodes, needs the signed registration because something must route to it.
**Publishing an entry is a deployment decision, not a property of being a
resource.**

**Consequence:** `discover_scope` is a filtering rule the node applies when
answering, and is **no part of the entry**. An owner may *request* one when it
registers (`wire-format.md` §6.2), and the host may narrow or ignore it; nothing
carries it to an asker, because receiving an entry is what qualifying looks like.
**Nothing propagates** — the node returns what an asker may see and omits the rest
(design §11.5), so there is no second path needing the scope on the wire.

---

## 7. Roles, and what the accessing user sees

**The infra node determines which connected nodes hold which roles**, from its
pluggable local trust algorithm plus per-resource configuration.

**Roles are declared by the package manifest.** The package says what roles exist;
the operator binds predicates to them; the client evaluates. So a package cannot
invent a role after installation, and an operator cannot grant a role the package
does not understand. It also gives the UI something concrete: a role list with
bound/unbound state, so *"you installed this and have not decided who may use it"*
is visible rather than a silent default.

### 7.1 Predicates, not membership lists

Group taxonomies are **per-resource, local data, not network objects**. No shared
record type is needed.

Assignment is by **predicate over data the infra node already holds.** Topology
and archive within its horizon. The predicate language therefore needs no new
state; it is a query language over existing data, and that bounds what it can
express, which for a security-critical component is a feature.

Affordances the UI should offer:

- All my direct clients
- All my clients and grand-clients
- Every node at [relative tier] with [trust above threshold]
- Every node in my trust horizon that joined before [date]
- **Named individual nodes.** See §7.1.2

**Access changes without anyone acting.** A node joining the subtree gains access;
one departing loses it; one crossing a tenure boundary gains it silently. Under a
*relative* rank predicate a membership change also moves the line for everyone else
(§7.2.1), which is the one case where a join changes somebody other than the joiner. **This is
correct.** Access follows the org chart, and it extends design §11.2's departure
warning to a wider surface. None of it produces an event anyone sees.

**design §11.2 states the general rule**, including what happens to in-flight state
when an owner moves. It is a protocol fact and belongs there; this section describes
what an implementation does with it.

#### 7.1.1 Membership is the outer gate

**Above the patron level, admission is a decision separate from membership.** The
node may present a principal whose adoption is valid and whose membership is
current, and still refuse it because the host has not acknowledged that node's
subtree (design §11.2.1). **A resource never sees this**: it receives a principal
and roles, or it receives nothing. The distinction matters only for understanding
why a structurally valid member may be absent.


**Current membership in the resource owner's trust horizon is a precondition for all
resource access.** It is not one predicate among others; it is the gate every
other predicate sits behind. **No grant of any kind reaches outside it.**

Consequences, all simplifications:

- **Departure revokes everything the node controls, uniformly.** For a hosted
  package the node terminates the session, so access ends at once. For a brokered
  external service, membership gates *establishment* only, an existing session
  continues on that service's terms, which is what choosing that service means
  (design §11.2).
- **No stale-grant accumulation.** Individual grants are not exceptions to
  positional access: a departed node cannot hold named roles while its predicate
  access lapses, which would be backwards from what either party expects.
  Membership as the outer gate removes the case entirely.
- **It is enforceable rather than advisory.** The infra node evaluates membership
  from topology it already holds, before consulting any predicate or grant.
- **It bounds the resource layer to neighbourhood scale.** A resource serves at
  most its owner's two-edge neighbourhood — 221 nodes at f = 10 (design §15.1). This is consistent with design §11.4's
  scope vocabulary topping out at `dunbar`, and it means **resources are
  neighbourhood-scale by construction**, not subnet-scale or network-scale.

**Resource access eligibility is one of the horizon's jobs**, and design §15.1's
table carries it as one. That table is also the argument against tuning *h* for
this reason alone: it is the most over-loaded parameter in the design, and a
change made here moves seven other things.

#### 7.1.2 Individual assignment within the org

**Roles must also be assignable to named individual nodes.** Inside the
membership gate, never outside it. Predicates cover the general case; individuals
cover the contractor who fits no group, the one person needing write access, the
exception that would otherwise force a predicate to be contorted around a single
node.

Because membership bounds them, individual grants raise only ordinary permission
hygiene: a node still in the org may hold a role it no longer needs. Worth showing
grants separately from predicates with the date each was made, so an operator can
review them, but this is ordinary permission hygiene rather than a structural hole, and expiry should
stay optional since some named grants are permanent by intent.

**External consumers are a different direction and unaffected.** A SaaS vendor
accepting a credential (§4, §13) is not *accessing a resource*; it is consuming an
assertion made about an org member by that member's own infra node. The vendor
need not be in the org, or in the network at all.

### 7.2 Trust thresholds as rank, not raw score

**Tuning a metric's parameters and having access follow is the feature**, not a
hazard: you re-tuned because you wanted that effect.

**Switching metric *families* is different.** design §16.1 makes the algorithm pluggable,
and a threshold of 0.6 under a decay metric has no defined counterpart under a flow
metric. The predicate evaluates without error against a number that now means
something else, and the operator's intent was "be more Sybil-resistant", not
"change who may use the datastore".

**Fix: express thresholds as ranks or percentiles.** *"Top 20% of my Dunbar org"*,
*"above the median of my direct clients"*. Every metric produces an ordering, so
rank-based predicates survive a metric change intact where raw scores do not. No
friction, no confirmation dialog, and it matches what operators actually mean,
since they are rarely thinking *0.6* and usually thinking *the people I trust most*.

#### 7.2.1 Predicate classes, and what each one depends on

**A predicate is a macro, not the mechanism.** What authorises a request is a
materialised table — one row per resource per member of the trust horizon, consulted as a
lookup (design §11.4). The predicate is how an operator writes that table quickly; it
expands at configuration time into assignments the operator can see and adjust. The
classes below differ in **what an assignment depends on besides the member**, which
is the whole of their security difference.

- **Structural** — *all my direct clients*, *clients and grand-clients*, *every node
  at a given relative tier*. Depends on the member's position relative to you.
  Nobody else's arrival or departure changes the answer for a given member.
- **Tenure** — *joined before [date]*. Depends on the member's own history. Same
  property: population-independent.
- **Named** — *these individuals* (§7.1.2). Depends on nothing but your choice, and
  is the form every other class is sugar for.
- **Absolute rank** — *my ten most trusted*. Depends on the member and on the nine
  above them. A node added below the line cannot change who is above it; a node
  added above can displace someone, which is the predicate working.
- **Relative rank** — *top 20%*, *above the median*, any quantile. **Depends on the
  size of the population.** This is the one class whose result a third party can move
  without out-ranking anybody.

**Why relative rank is different.** The cutoff is a fraction of a count, so anyone
who can enlarge the population *below* the line raises everyone above it. An org of
100 with the line at the top 20 admits its 25th member once 25 further members exist
beneath them, and the trust metric is not wrong at any point: it rates the added
members as worthless, which is exactly why they land at the bottom and lengthen the
queue. **The predicate never asked how trusted anyone was; it asked where they stood
in a line.** So a relative predicate's soundness rests on the acknowledgement policy
that decides who enters the horizon (design §11.2.1) rather than on the trust metric,
and those two settings are one decision rather than two.

**A relative predicate also has to be re-scored across the table, not per member.**
design §11.4 re-evaluates when a node enters or leaves the horizon; for the classes
above that means scoring the entrant and dropping the leaver's rows, but a quantile
moves for *everyone* when the count changes. An implementation that scores only the
changed member leaves rows that no longer follow the predicate: stale in both
directions, granting where the line has risen and withholding where it has fallen.

**What contains this is the size and character of the org, not a rule.** The table
is bounded and small (§3's vocabulary), and its members are people the operator has
at least passing acquaintance with; that is what the horizon is sized for. Moving a
quantile enough to matter means unfamiliar names appearing in a list the operator
reads while binding the role. **Statistical manoeuvring is a weak attack against a
population you recognise individually**, which is the reason the predicate language
is allowed to stay this simple.

### 7.3 Shrink-wrapping is a security requirement

**Because §4.1 makes every patron an operator, the administration must be
close to automatic** or the assumption above fails. Strong defaults, comprehensible
templates, *"let my org access this app in the typical way"* as a single click.

**But a permission default is a security decision**, and defaults are where most
real-world permission failures live. Three constraints follow:

- **Templates ship with the package**, because the package author knows what
  "typical" means for their application. Nobody else can write a sensible default
  for a service they did not build.
- **The package author's incentive runs toward breadth.** Their application works
  better with more users and fewer refusals, so an unexamined shipped default will
  tend to be more permissive than an operator would choose. This compounds §8's
  supply-chain problem: shrink-wrapping means **trusting the package author's
  judgment about access**, not merely their code.
- **Templates must therefore be expressed in the predicate language** (§7.1), not
  as opaque configuration. Then they are inspectable, editable, and diffable — an
  operator can see that *"typical"* means `dunbar` rather than discovering it
  later, and the client can render what a one-click choice actually grants **before
  the click, in the same vocabulary the operator uses elsewhere.**

The goal is one click for the common case with the grant legible in that same
click, not one click that hides the grant.

### 7.4 What the accessing user sees

**The roles they hold, not the predicates that granted them.**

The predicate is the operator's business and may encode judgments they would rather
not publish. But a user who cannot distinguish *denied by policy* from *broken*
will report bugs that are not bugs, and will have no idea that departing a patron
cost them access.

**Role names are visible strings and they leak.** A role called
`clinical-records-write` says something about the service and about whoever else
holds it. This is P15's shape one level down, the resource set fingerprints the
owner, the role set fingerprints the *user within it*. Not a reason to withhold
roles; a reason for operators to choose names knowing they are public to those who
can see the resource.

---

## 8. Packaging

Distributed as a container and/or VM image, so it must be set up at the OS level to
load additional packages.

**This puts software supply chain on the security-critical path for the first
time.** Signing, provenance, update channel and capability declaration are all now
questions this project has, and they are questions about *distribution* rather than
protocol.

**It also creates a trust relationship the trust model does not represent:** a
subnet's members trust their patron's judgment about which packages to run. Nothing
in design §16 expresses that, and it is not obviously reducible to the existing
metric.

**Declare what you need in the manifest.** Storage, compute, hardware capabilities, a
persistent address, anything a requesting client must supply. **A node that cannot meet
those requirements cannot host you, and remains conforming**, so does a user device
that cannot use you. That is ordinary capacity rather than a defect on either side.
Declaring it here makes the mismatch visible before installation instead of at first
request, which is packaging hygiene rather than something the protocol enforces. The
operator's side of the same fact is `infra-client-requirements.md` §9.

---

## 9. Sandboxing

**A plugin inside the infra client is inside the trust boundary.** The design works
hard to ensure a patron sees metadata and not content (design §14.2); run outside
a sandbox, a hostile or compromised extension would see everything the node sees.

The current threat model assumes the **operator's conduct** is the risk. It has no
model for **code the operator installed**, which is a different and probably larger
surface.

**This is a second, independent argument for a WASM target.** The first was
portability. This one is that the component model is **capability-based.** A
plugin receives exactly the host bindings it is granted and nothing else, which is
the shape needed for untrusted extensions running beside privileged state. This
argument survives the correction to the storage claim that undermined the first
one.

---

## 10. The node is an identity provider

**A resource trusting the node's assertion means a compromised node can mint any
principal with any role.**

This is the same trust a corporate gateway holds, and it is expected. But it is a
**new** concentration and it should be named: previously a compromised patron could
eclipse and observe (design §11.4). It can now also **impersonate its subordinates to
every resource they use.**

The design states carefully that a patron cannot forge its subordinates'
transactions. This is a place where it can effectively forge their **access**.

---

## 11. Gateways

A homeserver relaying to another network **sees that traffic**. Its operator is
socially trusted, a neighbour, a subnet member, but **socially trusted is not
accountable**, and they become the obvious target for anyone wanting that subnet's
external activity.

**design §11's permission model runs the wrong direction for this.** It is entirely about
the *owner* controlling who may access. Nothing helps a *user* evaluate an operator
they are about to route external traffic through. For team tools that asymmetry is
fine. **For gateways the user takes the larger risk and has the least support.**

---

## 12. Ownership and hosting are separable

A light client cannot host a service. It is not reachable. Its resource therefore
runs on its **serving infra node**, whose operator can decline to host it and sees
its traffic regardless.

design §11.2 states where they execute, a light-client owner's resource runs on
its serving infra node, and treats that as a **deployment fact separable from
ownership**. What follows from it is that a move changing the serving node forces
the resource to migrate.

---

## 13. External consumers

**Deliberately out of scope**, on design §1.1 grounds: the network has no shared state
with an external consumer, so it cannot define what a credential means to them. A
marketplace deciding what *"member in good standing"* is worth is doing **policy**,
and policy is theirs. Building a credential schema into the protocol would be the
same overreach as mandating a trust metric.

**Nothing new is needed.** A credential granter is a resource that reads its
owner's state — via the gateway, per §1, and emits something signed. The external
consumer's root-of-trust decision (*which resource keys do I accept?*) stays where
it belongs.

**Out of scope for the network.** An external consumer is not in the graph and
cannot compute a flow metric, so it needs its own root-of-trust decision, which
issuers do I recognise? That sits **between the patron and the resource**, and is
the responsibility of whoever writes the IdP plugin (§5), not of this protocol.
Consistent with per-observer trust rather than a violation of it.

---
