# Light client — requirements

**What a participant's client application must do.** Companion to
`network-design.md`, which is authoritative on protocol; this document is
authoritative on client behaviour.

**These are conformance requirements for the reference client, not protocol
rules.** Another implementation may differ and remain conforming (design §0). They
are collected here because they were otherwise scattered through the design
document, where an implementer had to find them by search.

**This document states no protocol rules of its own.** Where it repeats one for context it cites the section that owns it, and the cited document governs on any disagreement. Where a requirement depends on how
something works, it cites the design section rather than restating it, a restated
fact is one that will drift.

**On the force of these requirements.** Most of what follows cannot be checked by
anyone (see `network-design.md` §0, *The force of client requirements*). These are
**commitments, not enforceable rules**: a conforming label means the author asserts
them, not that anyone verified them. Where a requirement leaves a visible artifact,
that is noted in place.

---

## 1. Ceremony

### 1.1 Capture

- **Obtain the strongest proximity channel the hardware supports**, and record
  which was achieved. Never present a weaker channel as a stronger one (design
  design §7.1.6.3).
- **Refresh the reference image on every subsequent ceremony** with the same
  counterparty, so the stored image tracks the person rather than the first
  meeting (design §7.1.5).
- **Run the guided capture sequence.** Randomised prompts, 3–5 images over 10–15
  seconds (design §7.1.5).
- **Seal captures under keys derived from the seed the subject supplied, and never
  retain a released key after the ceremony** (design §7.1.5.2). A compliant client holds
  no decryptable likeness of another person; it regains access only when that person
  releases a segment key again in a later ceremony.
- **Derive the template at capture time and store it as a fixed-length record at
  the head of the encrypted store, images after** (design §7.1.5.2). Order is
  normative: writing images first silently converts a template-only grant into an
  image grant.
- **Release keys by tier.** Both while the image window is open, `k_template` only
  between the image and template windows, neither after. That is
  how the retention tiers are enforced.
- **Back up seeds with the rest of device state.** Losing them costs the ability to
  unlock your likeness everywhere, in the same way and for the same reason as losing
  portable standing (design §8.2).
- **Send a segment key as a `KeyGrant` naming both the record and the query it
  answers** (`wire-format.md` §5.3). A grant arriving unattached to a query the
  subject countersigned is an unsolicited key release; treat one as malformed rather
  than opening your store.
- **Withhold a presence record's disclosable fields by default** (design §7.2.1),
  revealing them only on the user's instruction. Ten of the eleven exchanges that
  receive a record need none of them, so the default is the correct one and the
  reverse would make the mechanism decorative.
- **Tell the user what revealing location means** at the one exchange where it has a
  use — presenting an archive to a prospective patron. Revealing offers the geohash
  series that supports an impossible-travel check; withholding is visible and declines
  to offer that evidence. **Both are legitimate**, and the same series is what P21
  turns into a behavioural record, so the user is choosing between being checkable and
  being trackable rather than between honest and evasive.
- **Keep one sealed capture per presence record**, ageing each independently, and
  prefer the most recent eligible one when answering (design §7.1.5.2).
- **Report a decryption failure as `inconclusive`, never as `no-match`.** Truncated or
  unauthenticated ciphertext says nothing about the subject, and reporting it as
  evidence would let a corrupted store become an adverse result (design §7.1.5.2).
- **Store your own seeds privately**, in your record of the transaction. They are
  what lets you unlock your likeness on a counterparty's device later, and they die
  with the device exactly as portable standing does (design §8.2).
- **Send the segment key directly to a selected verifier**, bypassing the
  counterparty running the ceremony and every witness (design §7.1.5.2).
- **Let the user set their own retention horizon, and tell them what it costs.**
  Declining to release a key enforces retention unilaterally against a
  compliant holder, but every counterparty cut off is one that can no longer
  answer `photo_match` if the user later needs recovery, leaving the weaker
  `personal_knowledge` basis (design §7.4). Say so when the policy is set, not
  afterwards.
- **Do not present withholding as private.** It is indistinguishable from
  unavailability to an evaluator, which is deliberate, but the user should not
  infer that a refusal goes unnoticed by the counterparty who asked.
- **Strip metadata from captured images before storing them, and do not rely on
  camera-pipeline defaults.** **This still applies under sealed-capture
  encryption** (design §7.1.5.2): the ciphertext protects the image at rest, and a
  legitimate decryption during a later verification puts the plaintext, EXIF
  included, in the holder's hands. EXIF location, timestamps and device identifiers
  defeat the coarse-geohash design outright, the record argues carefully about
  what precision to disclose, and an unstripped photograph beside it settles the
  question differently (design §14.5.8, C17).
- **Capture is not a photograph of a place.** Where framing can be influenced, a
  tight crop reduces recognisable background; where it cannot, the residual is
  real and belongs in what the user is told at capture time.

### 1.2 Verification

- **Hold the anti-oracle aggregate as a lock, not a log.** A counter per requester
  and per ceremony window, discarded when the window closes (design §7.1.4). It
  exists to refuse the next query; retaining a queryable history of who probed you
  builds a timeline of ceremony attempts on a device that can be seized.
- **Query verifiers automatically.** A client treating a face-to-face encounter as
  evidence of identity continuity must issue the queries without asking; a client
  that skips them silently produces evidence weaker than it appears (design
  design §7.1.3).
- **Verify the counterparty's verifier selection before signing.** If they
  selected off-seed and you sign anyway, you hold a record that fails
  recomputation permanently and cannot be repaired (design §7.2.2). **This is the
  one check that protects you against the person in front of you.**

### 1.3 Disclosure at capture time

**The schema cannot fix comprehension** (design §14.5.6). These are the client's
job.

- **Tell participants what the record will contain and who will be able to read
  it**, at the moment of capture, not in a policy document.
- **Tell witnesses and verifiers the same.** A verifier who answers permanently
  proves they previously met the subject; a witness proves neighbourhood
  involvement. Both become durable nodes in someone else's evidence graph, and the
  protocol's consent machinery does not cover them (design §14.5.6).

---

- **Count *n* from the back-pointer the record commits**, never from a subject's
  current head (`wire-format.md` §4.5). Counting from a current head lets a subject
  backfill after the ceremony and present a later evaluator a different threshold
  than any witness saw.
- **Report a record as unverifiable, not invalid, when you lack a participant's
  history.** Those are different answers and a caller may act on the difference.

---

## 2. Archive

- **Merge automatically on noticing divergence.** A user has no reason to want
  outstanding branches, and an unmerged fork means their history is incomplete
  wherever they present it (design §8.3).
- **Maintain backups, and make the consequence visible to the user.** Archive loss
  is loss of portable history, since the archive is a second factor — key without
  archive yields no transferable standing (design §8.2). Nobody can check that
  a client backs up; what a user *can* check is whether they were told what losing
  the device costs. On a host with evictable storage, saying so is the substance of
  the obligation.
- **Scan any imported backup for expired retention** and delete what is past its
  window. Import is exactly where an over-retention leak occurs (design §10.8.7.1).

**Presenting and serving history:**

- **When asking a new authority counterparty to rely on prior history, identify one
  contiguous archive prefix by its head.** Never an arbitrary selection of records.
  Presenting less history means presenting an *earlier* head; there is no other way
  to truncate, and none is needed. Present encoding: a single head txid on adoption
  (`wire-format.md` §4.1).
- **Serve archive requests for your own archive** (`wire-format.md` §5.9). The
  subject holds their archive, so a patron evaluating you fetches from you — this
  is peer-to-peer payload, not something an infra node serves on your behalf.
- **When fetching someone else's archive, verify the chain yourself.** Each
  returned record's back-pointers must match the record following it, and the first
  must match the head requested. **A holder cannot be trusted to have walked
  correctly**, and the check is hash comparison over records already being parsed.
- **Absence of data from a holder is never evidence about that data's existence.**
  A holder may refuse, rate-limit, or return less than asked for any reason; none
  of it says anything about what exists. Concretely: a short archive reply is not
  a short archive.

---

## 3. Payload encryption

design §11.2.4 adopts **PQXDH** for key agreement and the **Triple Ratchet** for
session secrecy. The client implements them; it does not reinvent them.

- **Publish a prekey bundle and keep it stocked.** A subject with no one-time
  prekeys left falls back to the last-resort key, which is a **declared reduction
  in forward secrecy.** Not a state to remain in. Replenish
  well before exhaustion (`wire-format.md` §5.8).
- **Rotate the signed prekey on a policy interval**, and never reuse a one-time
  prekey.
- **Only leaf-to-leaf needs this.** Sessions to a patron or a resource terminate at
  an endpoint that is online by definition and are already covered by the transport
  handshake (design §11.2.4).
- **Prefetch reusable prekey material for the whole Dunbar Org as a batch request**
  (`wire-format.md` §5.8), which is structurally distinct from a targeted fetch —
  so the serving node sees a sweep rather than having to take your word for it. A fetch driven by peers' rotation schedules reveals *past* activity —
  someone rotated — rather than intent to message. Fetching on demand instead
  announces each intended conversation to whoever serves the bundle (design
  design §14.5.8, C11).
- **Never prefetch one-time keys.** Serving one consumes it, so blanket prefetch
  would drain every pool in the org and make exhaustion the normal state —
  destroying its value as a signal that someone is draining a pool deliberately.
  Request a one-time key only when actually opening a session.
- **Accept the cost knowingly:** a session opened from prefetched material alone
  lacks one-time-key forward secrecy for its **first message**. Protection improves
  as ratchet contributions are incorporated, not immediately.

## 4. Session

- **Dial the serving infra node**, which is the nearest infrastructure node on the
  patron chain and not necessarily the patron (design §11.1.2).
- **Treat a sibling whose `KeyMaterial` you lack as unusable**, not as one to dial
  unauthenticated (`wire-format.md` §6). There is no fetch path: the party that
  would serve one is the node that is down.
- **A failed or partial cache load means no cached siblings**, not a partial list.
  Failing over on uncertain data is worse than reporting disconnection, because the
  user can act on the second.
- **Persist the cached sibling list across restarts.** It is pushed at attach
  precisely because it cannot be discovered once the serving node is dark, and an
  in-memory-only client loses failover exactly when a crash coincides with that
  outage (`wire-format.md` §6).
- **Use the cached list when the serving node is unreachable at attach time**, not
  only when a session dies mid-flight. The three-missed-intervals rule presupposes
  an established session; a client that waits for one can never fail over from a
  cold start.
- **Tell the user when attachment is degraded.** On a sibling, trust-bearing
  operations are unavailable; a user who is not told will read this as the
  application being broken and will not know that reconnecting resolves it (design
  design §11.1.2).
- **Disclose trust evidence only in response to another party's actual evaluation
  need**, never proactively for speculative or unsolicited evaluation (design §12).
  Present encoding: attestations are pulled, not pushed.
- **Show whether an address reaches a node's operator or a person behind it.**
  Resolution returns a residual path suffix: empty means the addressed party is the
  node itself, non-empty means the last hop forwards to an attached client (design
  §10.6.1). Rendering that distinction — `.0` for the node's own operator, in the
  military "actual" sense — tells a user what kind of party they are contacting.
  **It is not encoded in the locator**, because a light client that gains
  subordinates becomes infra without moving, and an address asserting terminal type
  would then be silently wrong in every cached copy.
- **Check each referral, not an arrival total.** A referral's `advances` must be at
  least 1 and must not advance past the path's end; arrival is announced by the
  `ServingInfra` reply itself, and no arrival-consistency equation is checked
  (`wire-format.md` §5.7) — it would reject the direct-serving answer a
  deeper-cached node is permitted to give.
- **Retry and endpoint-selection policy is yours**, with two floors: treat a node's
  endpoint list as alternatives rather than stopping at the first failure, and treat
  a policy refusal as that node's answer rather than that endpoint's — trying its
  other addresses will not change it.
- **Give every cache a lifetime, and say what it is.** Resolved locators, catalog
  answers, session and capability history, currency queries: each is a record of who
  you looked for and when, and **a cache with no expiry is a retention decision made
  by omission** (design §14.5.4).
- **Cache resolved intermediate addresses and prune toward stable entries.** Local
  policy; a node with infra-grade subordinates two levels down is a reasonable one
  to keep (design §10.6.1).
- **A resolution request discloses intent to reach someone**, before any contact
  exists, to whoever serves it (design §14.5.4, P26). Do not resolve
  speculatively, the same reasoning that forbids prefetching one-time prekeys.

---

## 5. Privacy choices the user must be able to make

- **Direct versus relayed payload must be overridable**, in both directions, and
  the client must state what each discloses. A direct connection reveals the
  user's IP to a peer inside the horizon; relaying reveals the communication graph
  to the serving node (design §14.5.4, P17).

---

## 6. Warnings before irreversible or surprising actions

- **Encourage a second independent adoption early**, while the user can still reach
  one freely. Subnet plurality is what defeats patron eclipse (design §14.4), and
  establishing it *after* an eclipse costs a physical meeting rather than a message
  — at the moment the user is least placed to recognise the need.

- **Before any user-initiated change that ends or alters an authority relationship,
  warn if the resulting topology change will revoke access to resources the user
  currently uses.** Access follows the org chart, which is correct and will surprise
  people (design §9.2). Departure is the common case, not the only one.
- **When accepting a new patron, list any resources whose policy reaches upward.**
  Predicates are relative to the owner, so an upline predicate matches the new
  neighbourhood after a move. The move is deliberate and the policy is the
  operator's own, so this is a reminder rather than a warning, but the consequence
  is easy to forget at the moment of accepting (design §9.2).
- **Before a user chooses an identity path that does not cryptographically link
  the new credential to prior history, warn that prior standing will be abandoned;
  where a path does publish such a link, make the corresponding loss of
  unlinkability clear.** The two are one choice seen from opposite sides (design
  design §10.8.7).

---

## 7. Cycle handling

- **When two participants simultaneously propose authority relationships in
  opposite directions — each becoming the other's superior — ask them to choose one
  direction rather than reporting a protocol failure.** Two people setting up
  together will each reasonably try to adopt the other (design §6.2.5).

---

## 8. Resources

- **Build your catalog view by sweeping the horizon, and cache it** (design §9.5).
  Refresh on joining a subnet, periodically, after a failed connection, and when the
  user asks. **Do not query per UI interaction.**
- **Treat an unreachable node as staleness in its portion of the view**, not as a
  general catalog failure. The other nodes' answers are unaffected, and telling the
  user which part is unknown is more useful than declaring the whole view suspect.
- **Give the user a refresh control** and let a stale view be stale. A new resource
  is normally mentioned by the person who added it, so deliberate lookup is the
  expected path rather than a fallback.
- **Hold one session at a time during a sweep** and close it. Browsing a cached view
  opens nothing.

- **Tell the user, when they first use a brokered resource, that ending their
  membership will not end that vendor's session.** The network can stop new
  establishment; it cannot reach into a session already running on someone else's
  terms (design §9.2). A user who leaves an organisation and assumes their access
  ended everywhere is wrong in a way only the client can correct.
- **Confirm a brokered service matches the signed `CatalogEntry`** before routing
  traffic to it, and show the user which owner published it (design §9.5). That
  binding is the whole of what the network offers about a gateway; beyond it the
  user's remedy is to avoid the resource.
- **Show which roles the user holds on each resource** they can reach. Without it,
  a user cannot distinguish *denied by policy* from *broken*, and will not connect
  losing access to having departed a patron (design §9.5).
- **Do not present resources the user cannot use.** The serving node filters its
  catalog page by what the viewer holds; the client should not re-expand it.
- **Access is gated by current membership in the resource owner's Dunbar Org**
  (design §9.2), so it changes without any action by the user, on joining,
  departing, or crossing a tenure boundary. The departure warning in §6 above is
  the case worth warning about, since the user is acting deliberately and the
  consequence is elsewhere.

---

## Open

- Whether recovery should restore archive history, and how, without handing an
  attacker the same path (design §17).
- Multi-device behaviour beyond merge: which device holds what, and how a user
  understands their archive spanning several (design §18.1).
