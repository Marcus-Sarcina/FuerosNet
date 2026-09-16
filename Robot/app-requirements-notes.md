# Application requirements — working notes

**Working file for mapping the application tier before the next review cycle.**
The five client variants are being specified from the top down, and the point of
doing it here first is that a protocol change found late costs a full review
programme to re-establish. So: surface everything that touches the protocol
*before* the surgery starts, not one item at a time.

Same standing as `review-tracking.md`. **Nothing in the root may cite this
file.** Answers that survive move into the design or the requirements documents
and are cited from there; this file keeps the working state.

---

## 1. The five variants

| Variant | PoP | Notes |
|---|---|---|
| Infrastructure node | n/a | `rhtnd` exists; milestone 11 met. Operated as a tab in the light client, not as a separate system |
| Android light client | yes | First to build. Retires CER-38/39 and most of PRD |
| iOS light client | yes | Second. Re-uses the generator and the shell design |
| Desktop graphical light client | **no** | A second device for an identity established elsewhere. CER-39 already says a desktop instrument keeps its seal in memory |
| CLI light client | no | Largely banked in `rhtn` and `rhtnp`. Claims no product entries — a command read from standard input is not a person |

Owed at the catalogue: **PAY-13** (payload library, awaiting a licence decision)
and **PRD-01 … PRD-09**, all milestone `manual`. Eight of the nine are
light-client product behaviour; **PRD-06** is the operator's. CER-37/38/39 are
implemented but deferred for want of real hardware.

---

## 2. Protocol changes surfaced

The expensive list. Each of these would move a base requirement.

### 2.1 A short-lived transport credential

**So an operator's seed need not sit on rented infrastructure.**

*Why it is forced.* The node's TLS identity **is** the operator's Ed25519
signing key, loaded directly as the TLS private key and authenticated as a raw
public key against the keyhash the peer expects. Every inbound connection needs
it live and hot — from up to 110 subordinates plus siblings, peers and attached
clients. The only other things the node signs are endpoint records and the
anchor entry; both are cached and re-signed only when the endpoint set or the
subtree size changes, so both are rare and could be batched to the operator's
client. **The transport is the sole reason the seed must be on the box.**

*Shape.* The node holds an ephemeral keypair. The operator's client signs a
delegation binding that key to the operator's keyhash for a window. Peers verify
the delegation against a keyhash they already hold.

*Form — decided* [author, 2026-09-16]. **Model it on OpenSSH certificates**: a
key signs a credential carrying a public key and a validity window, with no
revocation infrastructure and short lifetimes in its place. The "authority" is
just a key, which is what an operator's client is. **Keep raw public key at TLS
and let the presented key be the node's ephemeral one**; carry the delegation as
a `COSE_Sign1` record in the attach handshake, under a fourteenth
domain-separation tag beside the thirteen already in the codec. No ASN.1, no new
signature algorithm, no certificate authority, no revocation list.

Design §12.6.5 already adopts the PKI revocation playbook and has done the
analysis this inherits — including why online revocation checking that fails
open is decorative.

*Prior art considered and set aside.* **Signal's sender certificate** is the
same idea with an even smaller structure and would serve; **SPIFFE/SPIRE SVIDs**
are purpose-built but drag X.509 and ASN.1 in for no other benefit here;
**macaroons and Biscuit** bring bearer semantics and a caveat language that is
more than this needs.

*Lifetime — decided* [author, 2026-09-16]. **A longer window combined with a
forward-dated batch, reaching a total of about three months.** The author's
reasoning: the high-impact actions stay gated by the light client and the true
key, so the delegation only permits routine connections. The split between
window length and batch depth is still to set, but note that it buys nothing
against seizure — a node holds its whole batch in advance, or it cannot survive
its operator being offline, so **the exposure horizon is the batch total
regardless of how the individual windows are cut.**

*What three months measures.* Not "a seized node keeps its powers for a
quarter". Design §12.6.5 settles this: supersession inside the horizon, expiry
only beyond it. An operator who re-provisions and reissues the endpoint record
supersedes the seized node for every party that holds the new binding, and a
party holding that knowledge serves nothing under the old one. The three months
is the ceiling for parties **outside** the operator's horizon, where the new
binding never arrives — and for them a stale credential attests nothing anyway.
Inside the horizon it ends as fast as the endpoint record propagates.

*And the initiation cycle should push it* [author, 2026-09-16]. Because topology
is recoverable, **propagating the new binding to every node it knows belongs in
an instance's initiation**, not left to ordinary drift. "Every node it knows" is
the horizon and the far endpoints of visible peering records — there is no wider
list to push to — which is precisely the set §12.6.5 hands supersession. What
the ninety days then covers is the residue: stale contacts, long-idle operators,
and operators who never re-provision after a seizure.

*Residual exposure to state in the surgery.* For that window a seized node is
still the serving node for its subtree: it sees what a serving node sees and
holds what a serving node queues. That is a metadata and availability exposure,
not a trust one — it cannot sign an adoption, a disavowal or a recovery — but it
should be written down rather than left implied.

*Propagation — decided* [author, 2026-09-16]: **there is none, because the
delegation is a handshake artefact rather than a record.** The question of who
must see it dissolves: the delegation is *how* a party authenticates the node on
a connection, so exactly the parties on its connections verify it and nobody
else ever needs it. It is not stored, not forwarded, not replicated, and adds
nothing to the topology. The node's endpoint records and anchor entries are
still signed by the identity key and verify as they always did; the ephemeral
key signs nothing any third party reads.

The consequence to accept is that it rides **every** handshake. A verifier
should cache a delegation it has checked, keyed by the ephemeral public key, so
the cost is once per credential per peer rather than once per connection.

*Signature class — decided* [author, 2026-09-16]: **hybrid**, a logical signer
at ~3,373 B (`wire-format.md` §4.1) rather than classical Ed25519's 64. Forging
a delegation means impersonating a node at the transport layer for the
credential's window, and since the keyhash is over both components and the rest
of the identity is hybrid, a classical-only delegation would have been the one
non-hybrid link in the chain. Verifier-side caching is what makes the size
affordable, so the caching is not an optimisation to defer — it is what the
choice assumes.

*0-RTT — no new exposure, one rule needed.* The replay worry was misdirected:
early data travels client to node, and what rides it is the client's `Attach`,
which is already deferred and never processed before the handshake completes
(design §8.2, §9.2). A delegation travels the other way, in the node's handshake
flight, and is never in replayable early data.

The real interaction is **resumption outliving the delegation**. A resumed 0-RTT
connection authenticates by pre-shared key and re-presents nothing, so a ticket
issued under a live delegation still works after that delegation expires — or
after the node is seized. This is §12.6.5's stale-credential failure wearing a
different hat. **Decided** [author, 2026-09-16]: **never issue a resumption
ticket whose lifetime exceeds the delegation's remaining validity.** Standard
practice, one clamp, no new machinery, and it costs design §14.1.3's battery
argument nothing — tickets stay long for all but the tail of each window.

---

**§2.1 is fully specified.** What the surgery moves, for scoping the review:

| Where | What |
|---|---|
| `wire-format.md` | The delegation's encoding, and a fourteenth domain-separation tag beside the thirteen the codec carries |
| design §14.1.1, §14.1.3 | A node presents an ephemeral key with a delegation rather than its identity key; the resumption-ticket clamp |
| design §12.6.5 | Already carries the playbook this inherits; check whether it needs to name the delegation as a thing supersession answers |
| `infra-client-requirements.md` §7 | What an operator's node holds, presents and rotates |
| `infra-client-requirements.md` §8.2 | Cites design §23.3's same-key sentence, which §2.1 changes |
| `infra-client-requirements.md` §4.3, §4.4 | Endpoint and anchor records signed by the operator's client rather than by the node |
| `light-client-requirements.md` §4.1 | What a client verifies on attach, and that it caches a checked delegation |
| §2.2 below | P33's answer follows from this one |

### 2.2 P33 — which devices hold seeds, sealed captures and deletion state

Design §23.3 leaves it open and §22.2 tracks it; the register entry says
retention and deletion commitments cannot be assessed at all while it stands.
**The premium-tab product makes it blocking rather than background**, because
one-click provisioning places a seed on hardware the operator does not control.
§2.1 is the mitigation if it lands; if it does not, the answer to P33 has to
account for a compelled provider (design §18.1).

**P33's shape** [author, 2026-09-16]. The register entry allocates five things;
they are three decisions, not one.

- **Seeds — the PoP-capable device alone.** Everything else holds a delegation:
  the rented instance, the desktop, the CLI. What a light client signs with its
  identity key is three contexts (`ENVELOPE`, `CONSENT`, `VERIFIER`), every call
  site of them in the ceremony or the verifier-selection query, so a client that
  cannot originate a relationship has almost no occasion to use the key. The
  residue is departure and disavowal, which bounce to the PoP device — where
  they belong anyway, being what a shared desktop should least be able to do.
- **Deletion state follows the seed allocation and needs no separate answer.**
  Design §7.5.2 makes a capture decryptable only when the subject releases a key
  derived from a seed only they hold, so "which device holds deletion state" is
  "which device holds the per-ceremony seed". The register's worst line — that a
  deletion on one device says nothing about the others — dissolves once seeds
  sit in one place.
- **Archives, sealed captures and caches go where the storage is, and that is
  the desktop.** A phone is lost, stolen and broken; the full transaction record
  and the biometric captures should not go with it, and they are too voluminous
  to push to every patron, sibling and peer even encrypted — design §23.3
  already calls photo stores reaching hundreds of MB awkward for attachment
  quotas. A personal computer is not storage-constrained.

**The cold store resolves CER-39 rather than inheriting it.** That entry defers
because a desktop has no platform key storage to hold a live seal under. A
§13.7.1 envelope needs none: a random data key encrypts the blob, an Argon2id
KEK derived from a passphrase encrypts the data key, and the KEK is never
stored. A desktop holding that envelope holds ciphertext, and what protects it
is a passphrase rather than an enclave. **Cold store, not live client** is the
distinction to keep.

*The trade to state plainly in the surgery.* Design §23.3: a user who syncs
their archive to three devices has three places to lose it from. This buys
durability and recovery-to-a-new-phone at the cost of the archive's strength as
a second factor, and the design says so already.

---

## 3. Requirements text to revisit

Not protocol changes, but places where the application decisions and the
existing text do not obviously agree.

- ~~**`infra-client-requirements.md` §8.1 says an operator's interface "reads and
  never speaks for the node".**~~ **Withdrawn — a misreading** [author,
  2026-09-16]. §8.1 draws design §14.1.0's kernel line: the operator's view is
  not a second protocol implementation, which is why what crosses to it is what
  to draw and why an interface handed frames would be a second parser. It says
  nothing about the operator's authority to command their own node. A button
  asking the kernel to act composes no frame, and nothing in §8.1 forbids the
  product.

- **Administration does not go on the wire at all** [author, 2026-09-16], which
  is the answer to where an operator's command rides and removes the item from
  the surgery entirely. The reasoning: a participant's client and an infra node
  are distinct *network roles*, and the protocol governs what passes between
  them. An operator configuring the machine they own is not that. The infra tab
  in a client package is logically an extension of the infra client — the same
  software this document specifies — bundled into one product because a person
  should not juggle two, not because it is a participant's client doing
  participant things.

  So: no new request type, no `wire-format.md` change, no host binding, and no
  sandboxed component administering its own sandbox. The cost is a channel the
  product must carry and the document set does not specify. **`infra-client-
  requirements.md` §8.2 was written the other way and has been corrected**; the
  earlier text argued from authentication — the node can verify the operator's
  key, so why a second credential — which was answering the wrong question.
  Layering, not authentication, is what decides this. The premise is gone as well
  as the argument: under §2.1 the instance no longer holds that key at all.

- ~~**The node serves its own administration pages.**~~ **Closed** [author,
  2026-09-16] and landed at `infra-client-requirements.md` §8.3. Both halves went
  there rather than splitting across documents, because the operator's tab is
  infra-client software wearing a client's chrome, which is the ruling that
  produced it.

---

## 4. Landed already

- **`light-client-requirements.md` §6** — order a provisioning choice by how
  concentrated each provider is inside the operator's own horizon, mark the
  crowded ones rather than hiding them, and say what the ordering does not cover.
- **`infra-client-requirements.md` §8.2** — an operator reaches their instance
  over the session their own key already authenticates; the host's operating
  system and the provider's control plane stay out of band.
- **`infra-client-requirements.md` §8.3** — a node develops and serves its own
  administration pages, a client ships only provisioning, and the frame is a
  sandbox isolated from the presenting client's keys, archive and captures.
- **`network-design.md` §23.3, §18.1** — an instance carries a delegated
  credential rather than its operator's seed, and §18.1's residual narrows with
  it from impersonating the operator to continuing as the node.
- **`implementation-plan.md` section 7** — PRD-06 is a terminal *and* a page in
  the light client; the binding generator is `uniffi`.

---

## 5. Open application questions

To work through. Each gets checked for whether it touches the protocol before it
gets an answer.

- ~~**Provisioning.**~~ **Closed as a UX note, not a requirement** [author,
  2026-09-16]. What an operator is taking on — the recurring commitment being
  with the provider rather than the network, the capacity design §3.3's two-level
  bound buys, and the exposure the configuration creates — belongs in a
  selectable explainer about what infra operation means. Not warnings and
  click-throughs crowding the screen. Nothing lands in the requirements: LCR §6's
  ordering rule is the whole of what is owed there, and payment never touches the
  network.
- ~~**Re-provisioning after loss.**~~ **Decided** [author, 2026-09-16]: with the
  seed off the box, re-provisioning is routine housekeeping. Launch a new
  instance through the same sign-up screen, initialise it with the operator's
  key, and recover topology from siblings and peers — which is what sibling
  replication (design §3.4) exists for. A seizure costs the user data held on
  that instance and the services it hosted locally; everything else is a
  re-launch. **Only if the seed was on the box does the same event also require
  a key succession** (design §9), which is the heavier path and the one a
  non-technical operator is least able to drive. That asymmetry is the argument
  for §2.1.
- **The administration page.** What state does it read, at what rate, and over
  which stream? §8.1 bounds it to what to draw — a count, a keyhash, a time.
- ~~**Custody on the phone.**~~ **Decided** [author, 2026-09-16]: **the
  control-plane credential is persisted inside design §13.7.1's backup
  envelope.** Encrypted under a passphrase and held on a controlled device it is
  as secure as it needs to be, and it makes the desktop cold store a fully potent
  restore source — a user who remembers their passphrase recovers the ability to
  administer, not only to participate. A lost or compromised store is the one
  failure recoverable outside this network, since providers carry their own
  account recovery and can withdraw and reissue. Landed at
  `light-client-requirements.md` §2.

  *The precision that matters:* the envelope, not the archive. Siblings replicate
  the chain (design §3.4), and a provider credential is nobody else's.
- ~~**Wake endpoints and push.**~~ **Closed** — settled already and further than
  it looked. Design §14.1.4 decides the connection model (store-and-forward
  default, push opt-in, the dependency the client's and not the network's) and
  `light-client-requirements.md` §4.1 carries six obligations covering the
  mechanics. What remains is shell behaviour under iOS suspension and Android
  Doze, which §14.1.4 already scopes.

  *A disclosure requirement was proposed and declined* [author, 2026-09-16].
  The proposal was that LCR §5 should make a client state what opting into a
  doorbell reveals, on the pattern of its direct-versus-relayed bullet. Declined:
  push is an OS feature behind an OS opt-in, and a phone comes active on the
  network for every app at the same times, so a doorbell discloses nothing a
  user's other notifications do not. **This is not a darknet protocol and does
  not owe users pro-active remediation of leaks through other products.**
  Recorded so it is not raised again.
- ~~**Backup and restore**~~ **Closed** [author, 2026-09-16]. PRD-07 carries the
  mechanics and needs no change. The distinction worth holding is that **a
  self-restore is not a recovery from a holder** — and both places that govern it
  already say so: PRD-09 fires only when a reviewer "restores from a holder", and
  `light-client-requirements.md` §2 opens its bullet with restoring *from a
  holder* being an act of trust. Restoring your own envelope from your own cold
  store has no holder and is outside both. Recorded because the nearest mistake
  is over-applying PRD-09 until its warning means nothing, and because a reviewer
  may otherwise read the gap as missing rather than scoped.
- **The desktop variant.** Settled in §2.2, and the custody ruling above makes
  its cold store a complete restore source rather than a partial one. What
  remains is product: what it shows about its own limits, and whether a user
  without a desktop is told what they are not getting — design §23.3 says they
  are not stranded, envelope encryption being what lets a password manager or
  consumer cloud sync carry the same blob.
- **Multi-device in the ordinary case.** Phone, node and possibly desktop under
  one key. Forking and merge are specified (design §10.3); what the user is told
  about it is not.
