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

*Cheapest form.* Keep raw public key at TLS and let the presented key be the
node's ephemeral one; carry the delegation as a `COSE_Sign1` record in the attach
handshake, under a new domain-separation tag beside the thirteen already in the
codec. No ASN.1, no new signature algorithm, no certificate authority, no
revocation list.

*Prior art to evaluate.* The constraint is simple and bulletproof, no new design
burden:

- **OpenSSH certificates.** Closest conceptual fit. A key signs a certificate
  carrying a public key, a validity window and principals. No revocation
  infrastructure — short lifetimes instead. Deployed at scale for two decades.
  The "authority" is just a key, which is exactly what an operator's phone is.
- **Signal's sender certificate.** A short-lived signed assertion in a system
  with no authority. Minimal structure, close to what a `COSE_Sign1` record
  would carry anyway.
- **SPIFFE / SPIRE SVIDs.** Purpose-built for workload identity with short-lived
  credentials, but X.509 and ASN.1 bring surface the design has no other use for.
- **Macaroons, Biscuit.** Bearer semantics and a caveat language. More than is
  needed here.

*The hard open question: what happens when the operator's phone is offline.* A
credential measured in hours means a node goes dark when its operator's battery
does. Candidate answers, none chosen:

- A longer window, trading exposure for availability directly.
- The client pre-signs a forward-dated batch at provisioning time. Good
  availability; a seizure then yields the whole batch, so the exposure window is
  the batch horizon rather than the credential lifetime.
- A degraded mode where an expired credential still serves some traffic. Needs a
  rule for which, and that rule is a protocol fact.

*Also open.* Whether recovery (design §9) revokes a live delegation or waits it
out; whether every peer must see the delegation or only a serving node's direct
counterparties; and how the delegation interacts with 0-RTT deferral.

### 2.2 P33 — which devices hold seeds, sealed captures and deletion state

Design §23.3 leaves it open and §22.2 tracks it; the register entry says
retention and deletion commitments cannot be assessed at all while it stands.
**The premium-tab product makes it blocking rather than background**, because
one-click provisioning places a seed on hardware the operator does not control.
§2.1 is the mitigation if it lands; if it does not, the answer to P33 has to
account for a compelled provider (design §18.1).

The desktop variant needs the same answer from the other end — CER-39 already
says a desktop instrument keeps its seal in memory.

---

## 3. Requirements text to revisit

Not protocol changes, but places where the application decisions and the
existing text do not obviously agree.

- **`infra-client-requirements.md` §8.1 says an operator's interface "reads and
  never speaks for the node"** and does not compose, sign or send anything on the
  wire. Push-button administration is compatible only if the button is a request
  the kernel acts on rather than an interface composing a frame. The text should
  say which, because the natural reading of "never speaks" forbids the product.

---

## 4. Landed already

- **`light-client-requirements.md` §6** — order a provisioning choice by how
  concentrated each provider is inside the operator's own horizon, mark the
  crowded ones rather than hiding them, and say what the ordering does not cover.
- **`infra-client-requirements.md` §8.2** — an operator reaches their instance
  over the session their own key already authenticates; the host's operating
  system and the provider's control plane stay out of band.
- **`implementation-plan.md` section 7** — PRD-06 is a terminal *and* a page in
  the light client; the binding generator is `uniffi`.

---

## 5. Open application questions

To work through. Each gets checked for whether it touches the protocol before it
gets an answer.

- **Provisioning.** Provider and zone selection, routing to the provider's
  payment gateway, recurring-payment authorisation. Design §16 already
  anticipates an agent that encapsulates exactly this; what does the client owe
  beyond §4's ordering rule?
- **Re-provisioning after loss.** Launch a new instance, initialise it with the
  operator's key, recover topology from siblings and peers. Sibling replication
  (design §3.4) is what makes this work. Does it need anything the protocol does
  not already have, and is it one flow or two depending on whether the seed was
  exposed?
- **The administration page.** What state does it read, at what rate, and over
  which stream? §8.1 bounds it to what to draw — a count, a keyhash, a time.
- **Custody on the phone.** The operator's client ends up holding the RHTN
  identity, the provider's control-plane token, and possibly the node's
  delegation key. That concentration wants a deliberate decision.
- **Wake endpoints and push.** Registered per client; what a mobile shell owes
  around backgrounding and suspension.
- **Backup and restore** (PRD-07, design §13.7.1). Scan-on-import is specified;
  the product surface is not.
- **The desktop variant.** A second device that cannot originate a relationship.
  What it holds, what it may sign, and what it shows about its own limits.
- **Multi-device in the ordinary case.** Phone, node and possibly desktop under
  one key. Forking and merge are specified (design §10.3); what the user is told
  about it is not.
