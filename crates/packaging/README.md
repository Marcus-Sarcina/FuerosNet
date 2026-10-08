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

## Enrolment, as it stands

The node half is already complete, and the image changes nothing about it:

1. The instance boots, finds no transport key, **mints one**, writes
   `transport.pub`, and says so on stderr: `transport key <hex>; no credential
   in force: waiting for the run in <dir>`.
2. It **serves nothing** meanwhile, and polls the delegations directory every
   second (`Service::start`'s provisioning loop; §7).
3. The operator's client reads that public half, signs a run over it — and the
   endpoint record and anchor entry beside it — and puts them in the volume.
4. The node takes the run on its next look and serves. `mind_the_run` re-reads
   the directory on every tick thereafter, so a renewed run needs no restart,
   and the operator is told while seven credentials or fewer remain.

**What is not here is the channel** by which step 3's files reach the volume.
`infra-client-requirements.md` §8.2 declines to specify it — "administration is
out of band, with everything else about the host" — so for now it is whatever
the provider gives: an attached volume, user-data at first boot, a shell. That
choice is the next thing to settle, and it is the author's.

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
