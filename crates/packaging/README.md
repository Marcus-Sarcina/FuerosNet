# The node as an image

`infra-client-requirements.md` §9 opens by saying a node is **distributed as a
container or VM image**. This directory is that container: a two-stage build of
`rhtnd` and `rhtn`, the state it keeps, and what its operator must supply.

```
packaging/build.sh [--flavour releasable|fieldtest] [--tag <tag>]
packaging/smoke.sh [<image>]
```

`build.sh` tags with the commit, so an image on a rented host traces back to the
tree that made it. `smoke.sh` boots the image in both shapes a node takes and
asserts five things; it needs a container engine and nothing else.

Neither is in `crates/check.sh`. A build of this size does not belong in a gate
that runs on every commit, and the image's contents are already covered there by
the daemon's own tests.

## The two shapes

**A node holding its seed.** `identity` names the key it signs as. This is a
root, or a node whose operator runs it on hardware they control.

**An instance** (design §23.3). No seed: `operator` names the identity it speaks
as, `transport-key` the keypair it minted for itself, and `delegations` the runs
its operator's client signed over that key. **This is the shape a rented host
takes**, and §23.3 calls it "the ordinary condition of everyone in §3.3's tier"
— the key that signs as the operator stays on the device that performs
ceremonies, and a seized instance never holds it (design §18.1).

## The state volume

Mounted at `/var/lib/rhtn`, owned by the image's uid (10001 by default; the
build takes `--build-arg UID=`). Everything in `infra-client-requirements.md` §2
that must survive a restart lives here.

**What the operator supplies:**

| | |
|---|---|
| `rhtnd.conf` | the configuration. The entrypoint **will not invent one**: `Config` has no `Default` because "a default listen address or queue cap would be a policy choice made by omission" (§1), and the pages that compose it ship with the client (§8.3) |
| `operator.material` | the operator's `KeyMaterial`, hex — an instance only |
| `delegations/` | one signed run per file, read at start and on every tick |
| the endpoint record, the anchor entry | operator-signed, named by `endpoint-record` and `anchor-entry`. **An instance cannot mint these**: the signature on both is its operator's (§4.4), so a change of address is something the client signs |
| `peers` | the identities this node authenticates for its own countersigning. The entrypoint creates it **empty** when absent, which is a true statement about a node that has adopted nobody — an attaching client is authenticated by the key it presents (`wire-format.md` §9.1) and learned there |

**What the node writes for itself:** `grants/`, one file per resource,
holding who may reach it. The operator may write one by hand before the
first start, but an act on the surface rewrites it — it is the node's file,
not theirs ([who may reach a resource](#who-may-reach-a-resource)).

**What the node mints for itself:** the transport keypair, at `transport-key`,
0600 and never wider. The public half goes beside it with the extension
**replaced** — `transport.key` yields **`transport.pub`** — and that file is what
the operator's client signs a run over.

## The operator's surface, and enrolment as its first phase

1. The instance boots, finds no transport key, **mints one**, writes
   `transport.pub`, and says so on stderr: `transport key <hex>; no credential
   in force: waiting for the run in <dir>`.
2. It **serves nothing** meanwhile, and polls the delegations directory every
   second (`Service::start`'s provisioning loop; §7).
3. The operator's client gets that public half, signs over it, and hands back
   the run and the two records an instance cannot sign for itself (§4.4).
4. The node takes the run on its next look and serves. `mind_the_run` re-reads
   the directory on every tick thereafter, so a renewed run needs no restart,
   and the operator is told while seven credentials or fewer remain.

**Step 3 needs a channel, and §8.2 declines to specify one** — "administration
is out of band, with everything else about the host". So the reference node
offers one and the protocol knows nothing about it: an `[administration]` table
in the configuration opens a surface of its own, on a port of its own, speaking
HTTP/1.1 through the strict parser the gateway already uses.

```toml
[administration]
listen = "0.0.0.0:7449"
token  = "<64 hex digits>"
```

**7449/TCP is the convention**, beside the node's own 7447/UDP so an operator
opening one thinks of the other, and TCP where that is UDP. It is a convention
and not a default: the configuration names the address, because `Config` takes
no policy by omission. Open it in the provider's firewall alongside the QUIC
port.

```
GET /node?nonce=<16 bytes hex>   -> transport <hex>
                                    proof <hex>
                                    phase enrolling|serving
                                    credentials <n>
                                    endpoint-record held|wanted|unconfigured
                                    anchor-entry   held|wanted|unconfigured
PUT /node/run                    (body: one delegation's bytes)
PUT /node/endpoint-record        (body: the operator-signed record)
PUT /node/anchor-entry           (body: the operator-signed entry)
```

**One fetch says what is still wanted**, so a provisioning page drives the
exchange from the answer rather than from a list of steps kept elsewhere.

**The run is the commit point, so push everything else first.**
`Credential::add` puts a run in force as it verifies it, which ends the wait —
and `start` then reads the two records from the paths the configuration names,
refusing one it cannot read. A page that pushes the run first has the node look
for a file it has not sent yet.

### It outlives enrolment

**The surface stays up while the node runs**, because what it is for does not
happen once: §4.4 has an endpoint record re-signed whenever the address set
changes and an anchor entry whenever the subtree size does, §7 has the run
renewed before it lapses, and §8.3 has the node serve its own administration.
An instance whose address moves and has no way to be handed a new record
"publishes nothing until" its operator is reachable, which is the consequence
§4.4 says to plan for.

**While serving, a record taken is published at once** rather than kept for the
next start: an endpoint record is originated, an anchor entry offered to the
table. **Ordering is the node's question and it already answers it** — the
store's supersession discipline for the record, `AnchorTable::offer`'s "no
newer than the one held" for the entry — so a record is written to disk only
where the node took it, and a replay changes neither the node's state nor the
bytes a restart would read. Re-sending the record already held is a no-op that
says so, so a client unsure whether its push landed may simply push again.

### The administration page

`GET /` is the page the node serves its own operator (§8.3: "a node develops
and serves its own administration pages; a client provides the frame they are
presented in"). Before the run arrives it says what it is waiting for and
carries the transport key to sign over; afterwards it is the node's state —
what it is, what it holds, who is attached, and §8's exposure disclosure.

**What it shows and what may be asked are different halves**
(`infra-client-requirements.md` §8.3). The page is the node's state, read
only. The acts are declared beside it at `GET /node/acts`, in the same
closed vocabulary a package declares its operations in, and a client draws
them and proves each invocation (`rhtn_daemon::acts`): acknowledge, reload,
stop, install, running, upgrade, move, runner-refresh, the four grant acts,
logs and remove. `rhtn node acts` prints the declaration. What can never be
an act here is one needing the operator's *seed*: the endpoint record and
the anchor entry carry their signature, not the delegation's (§4.4), and the
instance does not hold that key (design §23.3), so for those the node can
only ask the client.

**The frame is the client's to sandbox** (§8.3), isolated from its keys,
archive and sealed captures, because a seized node serving a hostile page must
reach nothing on the device that still holds the seed (design §18.1, §23.3).
Nothing the node serves can assert that isolation; it is PRD-12's and is not
built.

### What a package declares

A manifest is TOML, read strictly — an unknown key or a missing one is an
error, because a manifest the host quietly repairs declares something the
package did not. Alongside its roles and its imports it declares the
operations its host's operator may ask of it:

```toml
component = "records.wasm"
roles = ["reader", "writer"]
imports = ["rhtn/1:request", "rhtn/1:response"]

[[admin]]
name = "retention"
label = "How long records are kept"
help = "Older records are discarded on the next sweep."

  [[admin.parameter]]
  name = "days"
  label = "Days"
  type = "number"
  low = 1
  high = 3650

  [[admin.parameter]]
  name = "nickname"
  label = "A name for this instance"
  type = "text"
  max = 64
```

**This is the surface the application presents to its host, not the
application itself.** A hosted resource is an independent program and serves
its own users directly, on ports it claims, with the node there to
authenticate them and gate their reach; none of that appears here. An
operation covers only what the operator hosting the instance may ask of it —
so the parameter types are a closed set (`flag`, `number` with `low` and
`high`, `text` with `max`, `choice` with `of`, `keyhash`), structured options
with free text where only free text will do.

The node draws every one of them the same way, which is §8.3's line: a node
"develops and serves its own administration pages". Nothing declared is
markup, layout or script, so an operator learns one idiom rather than one per
package. **Access is not declared here** — §7.3 requires an access template in
the predicate language rather than as opaque configuration, so who may reach a
resource stays in the grant section.

Every bound is checked at admission, not at the screen: at most 32 operations,
16 parameters each, 32 choices, 4096 characters of text, 80 of label, 240 of
help. A manifest naming a field the node could not draw is one it declines to
host, which §9 makes ordinary capacity rather than a fault on either side.

### Who may reach a resource

A package declares its roles; the operator decides who holds them
(`resource-requirements.md` §7). The deciding is written as **grants** in
the predicate language `infra-client-requirements.md` §10.3 fixes.

**Grants live in the node's own table, not in the hosting file.** The
configuration names a `grants` directory and the node keeps one file per
resource in it, named by the resource's keyhash:

```toml
# <grants>/0909….toml — written by the node
[[grant]]
roles = ["reader"]          # everyone in the owner's horizon

[[grant]]
roles = ["reader", "writer"]
  [[grant.where]]
  of = "clients"            # the owner's own clients, as well
```

**Why not the hosting file.** A grant is a table update, so an act on the
operator's surface rewrites it — and an act that did not survive a restart
would not be an act, because a revocation that came back on the next start
is worse than one that never happened. The hosting file is the operator's
and is never rewritten; changing it needs a restart or a re-read. Two
owners, two files. A hosting file that still carries `[[host.grant]]` is
refused with a pointer to where grants went.

A grant with no `where` admits every member of the owner's trust horizon,
which is §10.1's outer gate and nothing further. The clauses are §10.3's
list and no more:

| `of` | with | means |
|---|---|---|
| `clients` | — | the owner's direct clients |
| `grandclients` | — | those, and theirs |
| `distance` | `edges` | that many edges from the owner, by the walk that defines the horizon |
| `most-trusted` | `n` | the owner's `n` most trusted, an absolute rank |
| `top-fraction` | `percent` | the most trusted fraction, a quantile |
| `joined-before` | `when` | adopted before that time |
| `named` | `who` | one member, by keyhash |

Several clauses in one grant are an **and**; several grants are the **or**,
and a member matched by two holds both their roles. There is no negation —
no affordance asks for one, and a grant that says *everyone except* changes
meaning when somebody else joins. A grant never writes `connect`: reaching
the resource is what a grant is for (§3: "`connect` is the gate and it is
spent getting the request to you").

**A grant to one named party is a predicate like any other**, which is why
there is one shape here rather than one key for everyone and another for
somebody.

**Nothing is evaluated when a request arrives.** §10.2 holds a role table
and treats a predicate as a macro over it: the grants are expanded into
rows when the operator configures them, when membership moves, and when a
rank's inputs change, and a request is a single lookup. A member a
predicate stops matching loses the row and the hosted session with it
(§10.5).

**A package may ship grants ready-made**, in the same vocabulary, and the
node prints them back rather than applying them:

```toml
[[template]]
name = "org-read"
label = "Everyone in my org may read"
roles = ["reader"]

  [[template.where]]
  of = "grandclients"
```

§10.4 is why they are written in the predicate language and not as
configuration: an operator must see what a one-click choice grants "before
the click, in the vocabulary they use elsewhere". Taking one is their act;
installing a package grants nobody anything.

### Editing grants from the page

The resource's tab on the operator's page carries the controls, which is
OPS-012's "explicit management acts" over an operator's predicates:

| Control | Route | What it does |
|---|---|---|
| **Grant** | `POST /node/grant` | roles, and one clause or *every member of my trust horizon* |
| **Narrow** | `POST /node/grant/narrow` | adds a clause to a grant, which is the **and** |
| **Withdraw** | `POST /node/grant/drop` | removes it, and §10.5 ends the sessions its rows carried |
| **Take it** | `POST /node/grant/template` | takes a grant the package offered |

They are plain forms — §8.3 has the client provide the frame, and nothing
the node serves should need more of it than a browser already does. The
clause menu is built from the language itself, so the page cannot offer a
clause the node would not read, or call one by a word the file does not
use. Roles come as a checkbox each, with the ones nobody holds marked.

**Either both or neither.** The act changes the table first, because the
table is the authority on what a grant may say; if the file cannot be
written the table is put back and the act fails, so the two never disagree
about who may reach a resource. The rows are then re-expanded — §10.2's
first moment is "when an operator is configuring roles" — and the reply
says how many were written and dropped.

### Where a resource runs

Three shapes, and the file says which by what it names — §10.6 has the
hosting model follow from where the resource runs rather than from a
declaration:

| In `[[host]]` | In the manifest | What the node does |
|---|---|---|
| — | `component` | runs the package in its own sandbox |
| `address` | — | proxies to the resource on its own port |
| — | — | brokers: it authenticates, and the traffic goes elsewhere |

An `address` and a `component` together is refused rather than resolved:
that describes two places. So is the same address on two resources —
nothing partitions ports across package authors, so the file is the only
place a clash can be seen, and a node dialling one socket for two
resources would have one of them answering for both.

**A resource has one address, and nothing of it touches the network.**
Callers arrive on the node's own listen port and are routed by resource
keyhash inside the frame, so the loopback number is private to the host.
Two services means two resources — two keyhashes, two bindings, two
authorities — rather than one resource with two ports.

**A resource with its own port is an ordinary program.** It serves its own
users whatever it serves them — a web interface, a data service, an
application — and what arrives through the node is a request already
authenticated and authorised, carrying §2's credential as request headers:
`rhtn-principal`, `rhtn-roles`, `rhtn-audience`, `rhtn-session`. The node
strips every inbound `rhtn-*` header before inserting its own, because "a
client that sets `rhtn-roles: admin` and has it forwarded has defeated the
entire gateway" (§3.1). The resource's own answer is relayed unread; only a
chunked body is re-framed, since the framing would otherwise describe
something the caller never receives.

**Plain HTTP on that leg is only for a local socket.** §3 requires HTTPS
wherever the leg crosses a network: the node already reads the request, so
what the far leg protects is everyone else. The relay speaks plain HTTP, so
an `address` that is not a loopback address is **refused** rather than
carried — relaying an authenticated principal and an application body over
the open network is the thing that requirement exists to stop. A resource
elsewhere waits on TLS for that leg.

### A node on this machine

```bash
packaging/local.sh up      # build, run, enrol, and print the page's URL
packaging/local.sh down
```

It mints an operator identity, writes a configuration, runs the container, and
drives the surface with `rhtn node enrol` — the provisioning page's work as an
instrument. **The operator's key is kept outside the mounted volume**
(`runs/local-node/operator/`, against `runs/local-node/instance/`), which is
the whole of design §23.3: an instance that could read its operator's seed
would be the arrangement this design exists to avoid.

### Why only one leg carries a secret

A run is signed by the operator and `Credential::add` checks it is the
operator's, over *this* transport key, and a delegation; the two records carry
the operator's signature over this node's own keyhash, checked here exactly as
`Service::start` checks them. So **every inbound route is self-authenticating
and none is guarded by a bearer secret** — which is what keeps a long-lived
surface from being a long-lived credential.

The `GET` is the leg that needs authority: an on-path answer carrying an
attacker's key would have the client sign a run over it, and the attacker would
then serve as the operator's node. The only thing a client can share with a
host that does not exist yet is what it wrote into the configuration, so the
surface carries a **one-time token** there — **used as a MAC key and never
sent**. The client picks a nonce; the answer carries `HMAC-SHA256(token,
"rhtn/1:enrolment-proof" || nonce || key)` beside the key. The token is not the
transport key: the private half never leaves the instance, which is what design
§23.3 is for.

## Hosting a package: the procedure

What a package author does to have their resource served by a node, from
an image to a person opening it. Every step is a verb of `rhtn`, driving
the same routes and the same proof a client's screens do.

1. **Build an image** that keeps [the container contract](#the-container-contract)
   below, and **push it, naming it by digest** (`registry/name@sha256:…`)
   when it is installed, so what the host pulls is what was tested and a
   moved tag changes nothing already running.
2. **Mint the resource's identity**: `rhtn keys mint resource.key`, and
   `rhtn keys show resource.key` for its keyhash. It is the resource's,
   not the operator's.
3. **Install it** on the operator's instance:

   ```
   rhtn node install --at <machine>:7449 --token <token> \
     --identity operator.key --resource <resource keyhash> \
     --owner <operator keyhash> --manifest package.toml \
     --image registry/name@sha256:… --authority shop.internal --port 7460
   ```

   The token is the one `node launch` printed; an act arriving on the
   instance's own loopback needs none. The node writes the hosting entry,
   relays the manifest, reads the icon out of the image, and asks its host
   to pull and start it. A refusal changes nothing and says which part was
   missing or refused. `--offering <link>` in place of the manifest, image,
   authority and port takes what a distributor published (`rhtn node
   offer` composes one).
4. **Grant who may reach it**: nobody does until a grant is made.
   `rhtn node grant … --roles reader` is every member of the owner's
   horizon; `--of <clause> --with <value>` narrows it at once.
   `rhtn node grants` lists them numbered, and `grant-narrow` and
   `grant-drop` name one by that number. `grant-template --name` takes one
   the package offers.
5. **Open it**: from a launcher on a member's phone or desktop, or as a
   check from here, `rhtn resource <identity> <node keyhash> <address>
   <resource keyhash> /`.
6. **Read what it said**: `rhtn node logs` is the container's own output
   as its engine kept it (needs `--identity`); `rhtn node log` is what the
   instance wrote through its host's log binding.
   `rhtn node packages` and `rhtn node running` say what is installed and
   what is up.

**Installing again replaces it**: the container's name is derived from the
resource (`rhtn-pkg-` and its keyhash's first sixteen hex digits), so a
second install of one resource is an upgrade, keeping its block and its
secret. `rhtn node remove` stops it and leaves the block, which only its
operator destroys.

**A node from `local.sh` has no shell on this machine to start a package
with**, so steps 3 and 6's `node logs` are refused there and say which line
is missing. `testapp.sh install` is the route on this machine meanwhile: it
writes the hosting entry itself and restarts the node.

### The container contract

How the host starts a socket-backed package (`package-runner.sh`'s `start`),
and so what an image must keep:

| | |
|---|---|
| **Arguments** | `--listen 127.0.0.1:<port> --resource <64 hex> --secret <64 hex>`, after the image, in that order. Listen where you are told; answer as that resource |
| **Network** | the host's own (`--network=host`): your port is on the loopback the node shares, and the node is how anyone reaches it |
| **The secret** | read the node's `rhtn-*` headers only on a request presenting it as `rhtn-secret`, and present it on yours to the host's surface (`resource-requirements.md` §3.1, §3.3). It is the same across restarts and reinstalls |
| **Filesystem** | the root is read-only; `/tmp` is a tmpfs, lost on every restart; `/rhtn-block` is yours to write where your manifest asked for a block (`[budget] block_bytes`, or importing `rhtn/1:block`, 64 MiB unasked) and the host could bound one by its filesystem. Where it could not, your block is a surface of the host's instead, and the relationships exchange says which (§3.3) |
| **User** | the image's own `USER`. No capabilities (`--cap-drop=all`) and no privilege gained (`no-new-privileges`), so running as root buys nothing; run as a user of your own |
| **Lifetime** | `--restart=always`, with the arguments it was started with |
| **Output** | standard output and error are kept by the engine and read with `node logs`; what your operator should read as your log goes through the host's log binding instead |
| **Icon** | `/rhtn/icon.png`, `.jpg` or `.webp` in the image, read out without starting it (`resource-requirements.md` §8) |

`crates/testapp` is a package keeping all of this, and
`Containerfile.testapp` its image.

## The three credentials, and why the distinction is in the document

§8.2 names them and says why an operator must be told them apart: the node's
configuration, the host's operating system, and the provider's control plane.
**"A credential for the last two can destroy the instance and bill its owner,
and an operator who is never told the difference between the three cannot judge
what they are handing over."** A client that launches instances holds the third
kind, which is new for it; whatever it holds, it is not what signs as the
operator, and the volume above never contains a seed.

## Two things the image deliberately does not carry

**No certificate authorities.** The transport authenticates raw public keys
(RFC 7250, `wire-format.md` §9.1) and pins by keyhash, so a certificate store
would be a trust root this protocol does not have. The runtime base ships
without one and nothing asks for it.

**No privilege.** The node runs as its own uid, reads keys at 0600 and writes a
queue; nothing it does wants root.
