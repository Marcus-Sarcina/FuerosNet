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

**It is one page of text and tables.** No script, no form, no control: §8.1 has
an operator's interface read and never speak for the node, and a page offering
a button would be offering one. What changes a node is an object its operator
signed, which goes to the routes above.

**The frame is the client's to sandbox** (§8.3), isolated from its keys,
archive and sealed captures, because a seized node serving a hostile page must
reach nothing on the device that still holds the seed (design §18.1, §23.3).
Nothing the node serves can assert that isolation; it is PRD-12's and is not
built.

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
