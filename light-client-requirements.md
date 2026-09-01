# Light client — requirements

**What a participant's client software must do.** Companion to
`network-design.md`, which is authoritative on protocol; this document is
authoritative on client behaviour.

**This document applies to infra operators too.** An operator is an ordinary
participant who also runs infrastructure, and their own user actions happen in a
participant client, so a complete operator deployment satisfies this document as well
as `infra-client-requirements.md` (design Appendix A). Reaching your own instance from your
own client is a matter of user interface, not of protocol.

**Ask the user only where design Appendix A says to.** *What a client does without asking*
fixes the line: a live interaction where two people must both act, or the user
configuring their own things. Everything else runs from policy they set earlier.

**These are conformance requirements for the reference client, not protocol
rules.** Another implementation may differ and remain conforming (design Appendix A). They
are collected here because they were otherwise scattered through the design
document, where an implementer had to find them by search.

**This document states no protocol rules of its own.** Where it repeats one for context it cites the section that owns it, and the cited document governs on any disagreement. Where a requirement depends on how
something works, it cites the design section rather than restating it, a restated
fact is one that will drift.

**On the force of these requirements.** Most of what follows cannot be checked by
anyone (see `network-design.md` Appendix A, *The force of client requirements*). These are
**commitments, not enforceable rules**: a conforming label means the author asserts
them, not that anyone verified them. Where a requirement leaves a visible artifact,
that is noted in place.

---

## 1. Ceremony

### 1.0 Nomination

- **Nominate witnesses only from the counterparty's neighbourhood, never from your
  own** (design §7.1). Nominating your own is the failure mode the rule exists to
  prevent, and it destroys the only property the witness set gives you.
- **Spread nominations across as many independent branches of that neighbourhood as
  you can, and include a random element.** A selection concentrated on one branch is
  caught by a correspondingly smaller fake region; a spread one forces a much larger
  occupation (design §7.1.1).
- **Probe your selections for availability; do not nominate from a set of nodes that
  advertised themselves.** Many nodes worth nominating are inactive or not running
  at ceremony time, so availability is something you discover after selecting. A
  node that solicits nomination is self-selecting (design §7.1.1).
- **Check the `nominated_by` split before you sign, and tell the user when your own
  nominees are absent or outnumbered.** The witness set is only representative to
  the extent you nominated half of it; where the counterparty nominated all of it,
  the attestation rests entirely on their nominees (design §7.1.1). This
  is not a validity condition — such a record is well-formed — which is why the
  client has to surface it.

#### 1.0.1 Acting as a witness

- **Decline to witness a ceremony whose claimed `started_at` is far from the
  time you observe.** Your clock is the only independent one at the ceremony,
  and the chronology bound (`wire-format.md` §3.3) is worth nothing unless
  witnesses apply it. No later validator can check that you did — set the
  tolerance you can defend and refuse outside it.

### 1.1 Capture

- **Obtain the strongest proximity channel the hardware supports**, and record
  which was achieved. Never present a weaker channel as a stronger one (design
  §7.6.3).
- **Refresh the reference image on every subsequent ceremony** with the same
  counterparty, so the stored image tracks the person rather than the first
  meeting (design §7.5).
- **Run the guided capture sequence.** Randomised prompts, 3–5 images over 10–15
  seconds (design §7.5).
- **Seal captures under the capture key the subject supplied, discard it once the
  capture is sealed, and never retain a key released later** (design §7.5.2). A
  compliant client holds no decryptable likeness of another person; it regains
  access only when that person releases the capture key again.
- **Release the capture key on the user's policy — the declared window is their
  default, not a rule** (design §7.5.2): they may enforce it, shorten it, or
  extend it by continuing to supply. It is the subject's key, and declining to
  send it is the whole mechanism.
- **Back up seeds with the rest of device state.** Losing them costs the ability to
  unlock your likeness everywhere, in the same way and for the same reason as losing
  portable standing (design §10.2).
- **Send a capture key as a `KeyGrant` naming both the record and the query it
  answers** (`wire-format.md` §7.3). A grant arriving unattached to a query the
  subject countersigned is an unsolicited key release; treat one as malformed rather
  than opening your store.
- **Withhold a presence record's disclosable fields by default** (design §8.1.1),
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
  prefer the most recent eligible one when answering (design §7.5.2).
- **Report a decryption failure as `inconclusive`, never as `no-match`.** Truncated or
  unauthenticated ciphertext says nothing about the subject, and reporting it as
  evidence would let a corrupted store become an adverse result (design §7.5.2).
- **Store your own seeds privately**, in your record of the transaction. They are
  what lets you unlock your likeness on a counterparty's device later, and absent
  a restored backup they die with the device exactly as portable standing does
  (design §10.2).
- **Send the capture key directly to a selected verifier**, bypassing the
  counterparty running the ceremony and every witness (design §7.5.2).
- **Let the user set their own retention horizon, and tell them what it costs.**
  Declining to release a key enforces retention unilaterally against a
  compliant holder, but every counterparty cut off is one that can no longer
  answer `photo_match` if the user later needs recovery, leaving the weaker
  `personal_knowledge` basis (design §9). Say so when the policy is set, not
  afterwards.
- **Do not present withholding as private.** It is indistinguishable from
  unavailability to an evaluator, which is deliberate, but the user should not
  infer that a refusal goes unnoticed by the counterparty who asked.
- **Strip metadata from captured images before storing them, and do not rely on
  camera-pipeline defaults.** **This still applies under sealed-capture
  encryption** (design §7.5.2): the ciphertext protects the image at rest, and a
  legitimate decryption during a later verification puts the plaintext, EXIF
  included, in the holder's hands. EXIF location, timestamps and device identifiers
  defeat the coarse-geohash design outright, the record argues carefully about
  what precision to disclose, and an unstripped photograph beside it settles the
  question differently (design §19.8, C17).
- **Capture is not a photograph of a place.** Where framing can be influenced, a
  tight crop reduces recognisable background; where it cannot, the residual is
  real and belongs in what the user is told at capture time.

### 1.2 Verification

- **Hold the anti-oracle aggregate as a lock, not a log.** A counter per requester
  and per ceremony window, discarded when the window closes (design §7.4.1). It
  exists to refuse the next query; retaining a queryable history of who probed you
  builds a timeline of ceremony attempts on a device that can be seized.
- **Query verifiers automatically.** A client treating a face-to-face encounter as
  evidence of identity continuity must issue the queries without asking; a client
  that skips them silently produces evidence weaker than it appears (design
  §7.3).
- **Select your counterparty's verifiers by your own recognition, and say which
  was which** (design §8.1.2): people you have met, people in any of your trust
  horizons, then one further edge — and go fishing for common acquaintances
  before filling the remainder at your discretion, marking each response's
  basis honestly (`wire-format.md` §5.5). **Review who was selected for you
  before signing**: the record will carry those responders forever, and you
  cannot repair it.
- **Compute *n* over the bundle handed to you, and treat it as your
  counterparty's claim** (`wire-format.md` §5.2 and `wire-format.md` §5.4). The bundle is theirs to
  curate — records from any of their series, no chaining, no completeness — so
  verify each record alone, count qualifying ones once by txid, and read the
  result as sizing your diligence, never as a fact about their history.
- **Report a record as unverifiable, not invalid, when you lack a participant's
  history.** Those are different answers and a caller may act on the difference.
- **Look at the candidate population you are sampling, not only at the answers it
  returns.** You enumerate the counterparty's prior counterparties in order to select
  from them, so you can see whether you recognise any of them before you see a single
  reply. **Tell the user when you recognise none of it**: a `match` from strangers
  establishes nothing, however many you queried, and a candidate set holding
  nobody you know defeats the shared-identity check outright rather than merely
  weakening it (design §7.3).

  With the `nominated_by` check in §1.0, **these are the three checks that protect
  you against the person in front of you** rather than against an outsider.

### 1.3 Disclosure at capture time

**The schema cannot fix comprehension** (design §19.6). These are the client's
job.

- **Tell participants what the record will contain and who will be able to read
  it**, at the moment of capture, not in a policy document.
- **Tell witnesses and verifiers the same.** A verifier who answers permanently
  proves they previously met the subject; a witness proves neighbourhood
  involvement. Both become durable nodes in someone else's evidence graph, and the
  protocol's consent machinery does not cover them (design §19.6).


---

## 2. Archive

- **Never take a series reissue into a series you have occupied before.** The abandoned line's
  high-counter records would come back into comparison against the new one
  (`wire-format.md` §4.6). You hold your own chain, so this is yours to check, and a
  counterparty holding that chain will reject a reissue that repeats a series in it.
- **Keep the chain that proves your current series** — your adoption and every reissue
  since — and present it when a counterparty needs to rank two of your records
  (`wire-format.md` §4.6.1). It is presented, never propagated, and it discloses the age
  of your patron relationship and how many reissues you have taken.
- **On suspected key compromise, seal before you take a series reissue.** Set the counter of the
  series you are leaving to its maximum, then take the reissue naming that value, and
  do it for every patron relationship you hold (`wire-format.md` §4.6). Nothing the
  holder of your old key signs can then supersede your last record in that line.
  **Act on suspicion**: against a counterparty that holds no chain this is a race, and
  the thief wins it by reaching them first.
- **Seal the old line when you rotate**, as the last thing the old key does (design
  §9.0). Otherwise it keeps the ability to redirect anyone still holding a stale
  locator, indefinitely and with no other source of truth available to them. One
  self-signed locator at the maximum counter; no patron needed.
- **Pruning the chain must not delete presence records, sealed captures or capture
  seeds.** A checkpoint bounds what the *chain* must retain; presence evidence is a
  separate kind of history that stays usable under any sequence and in any subtree
  (design §10.0). Deleting it with the chain would discard the evidence a later
  adoption and a later recovery both depend on, and would put a patron's willingness
  to countersign a series reissue astride evidence §6.4 says it must not gate.
- **Keep retention and disclosure as separate controls.** Choosing how far back to
  *disclose* must never delete anything: disclosure is reversible and can be widened
  later, up to what is retained. Deleting is the one-way door, and it caps every
  future disclosure — so it belongs behind its own deliberate control, never as a
  side effect of choosing what to show today.
- **Treat a series reissue as deliberate, not routine.** It costs nothing in routing, since
  the path is unchanged, but it is the boundary beyond which you may prune — and
  pruning is irreversible. **Never prune inside the 730-day window**: those records are
  what verifier selection counts, and discarding them lowers your own verification
  threshold, which is a thing an evaluator can see you did (design §10.2).


- **Merge automatically on noticing divergence.** A user has no reason to want
  outstanding branches, and an unmerged fork means their history is incomplete
  wherever they present it (design §10.3).
- **Maintain backups, and make the consequence visible to the user.** Archive loss
  is loss of portable history, since the archive is a second factor — key without
  archive yields no transferable standing (design §10.2). Nobody can check that
  a client backs up; what a user *can* check is whether they were told what losing
  the device costs. On a host with evictable storage, saying so is the substance of
  the obligation.
- **Scan any imported backup for expired retention** and delete what is past its
  window. Import is exactly where an over-retention leak occurs (design §13.7.1).
- **Restoring your own archive from a holder is an act of trust, and the
  interface must say so.** A client that lost its archive does not know its own
  head, so it fetches from the newest record the holder claims
  (`wire-format.md` §7.9) — the chain verifies internally, its completeness does
  not, and the same reduced footing extends through the whole lost-archive path:
  re-adoption on a fresh series presents no history and rests on the patron's
  personal judgment. Surface both as what they are — recovery on trust rather
  than on verification — and never present a restored archive as
  verified-complete.

**Presenting and serving history:**

- **When asking a new authority counterparty to rely on prior history, identify one
  contiguous archive prefix by its head.** Never an arbitrary selection of records.
  Presenting less history means presenting an *earlier* head; there is no other way
  to truncate, and none is needed. Present encoding: a single head txid on adoption
  (`wire-format.md` §4.1).
- **Serve archive requests for your own archive** (`wire-format.md` §7.9). The
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

design §14.2.4 adopts **PQXDH** for key agreement and the **Triple Ratchet** for
session secrecy. The client implements them; it does not reinvent them.

- **Publish a prekey bundle and keep it stocked.** A subject with no one-time
  prekeys left falls back to the last-resort key, which is a **declared reduction
  in forward secrecy.** Not a state to remain in. Replenish
  well before exhaustion (`wire-format.md` §7.8).
- **Rotate the signed prekey on a policy interval**, and never reuse a one-time
  prekey.
- **Only leaf-to-leaf needs this.** Sessions to a patron or a resource terminate
  at an endpoint that is online by definition and already run over an
  authenticated transport — the `rhtn/1` session, or the service's own TLS on a
  brokered connection (design §14.2.4, `resource-requirements.md` §3).
- **Prefetch reusable prekey material for the whole Dunbar Org as a batch request**
  (`wire-format.md` §7.8), which is structurally distinct from a targeted fetch —
  so the serving node sees a sweep rather than having to take your word for it. A fetch driven by peers' rotation schedules reveals *past* activity —
  someone rotated — rather than intent to message. Fetching on demand instead
  announces each intended conversation to whoever serves the bundle (design
  §19.8, C11).
- **Never prefetch one-time keys.** Serving one consumes it, so blanket prefetch
  would drain every pool in the org and make exhaustion the normal state —
  destroying its value as a signal that someone is draining a pool deliberately.
  Request a one-time key only when actually opening a session.
- **Accept the cost knowingly:** a session opened from prefetched material alone
  lacks one-time-key forward secrecy for its **first message**. Protection improves
  as ratchet contributions are incorporated, not immediately.

## 4. Session

- **Dial the serving infra node**, which is the nearest infrastructure node on the
  patron chain and not necessarily the patron (design §14.1.2).
- **Treat a sibling whose `KeyMaterial` you lack as unusable**, not as one to dial
  unauthenticated (`wire-format.md` §8). There is no fetch path: the party that
  would serve one is the node that is down.
- **A failed or partial cache load means no cached siblings**, not a partial list.
  Failing over on uncertain data is worse than reporting disconnection, because the
  user can act on the second.
- **Persist the cached sibling list across restarts.** It is pushed at attach
  precisely because it cannot be discovered once the serving node is dark, and an
  in-memory-only client loses failover exactly when a crash coincides with that
  outage (`wire-format.md` §8).
- **Use the cached list when the serving node is unreachable at attach time**, not
  only when a session dies mid-flight. The three-missed-intervals rule presupposes
  an established session; a client that waits for one can never fail over from a
  cold start.
- **Tell the user when attachment is degraded.** On a sibling, trust-bearing
  operations are unavailable; a user who is not told will read this as the
  application being broken and will not know that reconnecting resolves it (design
  §14.1.2).
- **Disclose trust evidence only in response to another party's actual evaluation
  need**, never proactively for speculative or unsolicited evaluation (design §15).
  Present encoding: attestations are pulled, not pushed.
- **Show whether an address reaches a node's operator or a person behind it.**
  Resolution returns a residual path suffix: empty means the addressed party is the
  node itself, non-empty means the last hop forwards to an attached client (design
  §12.6.1). Rendering that distinction — `.0` for the node's own operator, in the
  military "actual" sense — tells a user **which endpoint answered**, not what kind
  of participant is behind it: an operator is an ordinary user who also runs
  infrastructure, and the root of a large tree takes their own actions through a
  client like everyone else.
  **It is not encoded in the locator**, because type is not a function of position:
  a node becomes infra by launching and signing an infra instance, without moving
  (design §12.6.1), and an address asserting terminal type would then be silently
  wrong in every cached copy.
- **Check each referral, not an arrival total.** A referral's `advances` must be at
  least 1 and must not advance past the path's end; arrival is announced by the
  `ServingInfra` reply itself, and no arrival-consistency equation is checked
  (`wire-format.md` §7.7) — it would reject the direct-serving answer a
  deeper-cached node is permitted to give.
- **Retry and endpoint-selection policy is yours**, with two floors: treat a node's
  endpoint list as alternatives rather than stopping at the first failure, and treat
  a policy refusal as that node's answer rather than that endpoint's — trying its
  other addresses will not change it.
- **Give every cache a lifetime, and say what it is.** Resolved locators, catalog
  answers, session and capability history, currency queries: each is a record of who
  you looked for and when, and **a cache with no expiry is a retention decision made
  by omission** (design §19.4).
- **Cache resolved intermediate addresses and prune toward stable entries.** Local
  policy; a node with infra-grade subordinates two levels down is a reasonable one
  to keep (design §12.6.1).
- **A resolution request discloses intent to reach someone**, before any contact
  exists, to whoever serves it (design §19.4, P26). Do not resolve
  speculatively, the same reasoning that forbids prefetching one-time prekeys.

---

## 5. Privacy choices the user must be able to make

- **Direct versus relayed payload must be overridable**, in both directions, and
  the client must state what each discloses. A direct connection reveals the
  user's IP to a peer inside the horizon; relaying reveals the communication graph
  to the serving node (design §19.4, P17).

---

## 6. Warnings before irreversible or surprising actions

- **Encourage a second independent adoption early**, while the user can still reach
  one freely. Subnet plurality is what defeats patron eclipse (design §18.4), and
  establishing it *after* an eclipse costs a physical meeting rather than a message
  — at the moment the user is least placed to recognise the need.

- **Before any user-initiated change that ends or alters an authority relationship,
  warn if the resulting topology change will revoke access to resources the user
  currently uses.** Access follows the org chart, which is correct and will surprise
  people (design §11.2). Departure is the common case, not the only one.
- **When accepting a new patron, list any resources whose policy reaches upward.**
  Predicates are relative to the owner, so an upline predicate matches the new
  neighbourhood after a move. The move is deliberate and the policy is the
  operator's own, so this is a reminder rather than a warning, but the consequence
  is easy to forget at the moment of accepting (design §11.2).
- **Before a user chooses an identity path that does not cryptographically link
  the new credential to prior history, warn that prior standing will be abandoned;
  where a path does publish such a link, make the corresponding loss of
  unlinkability clear.** The two are one choice seen from opposite sides (design
  §13.7).

---

## 7. Cycle handling

- **When two participants simultaneously propose authority relationships in
  opposite directions — each becoming the other's superior — ask them to choose one
  direction rather than reporting a protocol failure.** Two people setting up
  together will each reasonably try to adopt the other (design §6.2.5).

---

## 8. Resources

- **Register a resource with your serving node, not with the network**
  (`wire-format.md` §6.2, request type 7). You sign the entry, your host answers for
  it, and the `discover_scope` you send is a request the host may narrow — an
  owner delegating hosting delegates that filtering. **What you send is the signed
  entry itself**, not a transaction: it enters no archive, chains to nothing, and the
  host answers from it directly.
- **Build your catalog view by sweeping the horizon, and cache it** (design §11.5).
  Refresh on joining a subnet, periodically, after a failed connection, and when the
  user asks. **Do not query per UI interaction.**
- **Treat an unreachable node as staleness in its portion of the view**, not as a
  general catalog failure. The other nodes' answers are unaffected, and telling the
  user which part is unknown is more useful than declaring the whole view suspect.
- **A repeated continuation means truncation, not another page.** If a reply fills
  the entry bound and its continuation names a service type you have already
  filtered on, the answering node holds more entries of that type than the bound
  and asking again returns the same page (`wire-format.md` §6.4). Record that
  node's portion as truncated, as you would an unreachable one, and do not follow
  the hint again.
- **Give the user a refresh control** and let a stale view be stale. A new resource
  is normally mentioned by the person who added it, so deliberate lookup is the
  expected path rather than a fallback.
- **Hold one session at a time during a sweep** and close it. Browsing a cached view
  opens nothing.

- **Expand a role predicate into names before the operator binds it, and keep the
  names the primary view.** A predicate is a macro over a table of individual grants
  (design §11.4); what the operator is deciding is who gets in, and a list of people
  they recognise is the form in which a wrong answer is obvious. Show the count
  beside it, never instead of it.
- **For a relative rank predicate — a percentile, a median, any quantile — show the
  population it is a fraction of, and say that the line moves when the org does.**
  These are the only predicates whose result depends on somebody other than the
  member being granted (`resource-requirements.md` §7.2.1), and an operator reading
  *"top 20%"* has no reason to expect that admitting members at the bottom promotes
  someone at the cutoff. An absolute *top-k* does not behave this way, and the
  client should offer it as the plainer alternative when the operator's intent looks
  absolute.
- **Tell the user, when they first use a brokered resource, that ending their
  membership will not end that vendor's session.** The network can stop new
  establishment; it cannot reach into a session already running on someone else's
  terms (design §11.2). A user who leaves an organisation and assumes their access
  ended everywhere is wrong in a way only the client can correct.
- **Confirm a brokered service matches the signed `CatalogEntry`** before routing
  traffic to it, and show the user which owner published it (design §11.5). That
  binding is the whole of what the network offers about a gateway; beyond it the
  user's remedy is to avoid the resource.
- **Show which roles the user holds on each resource** they can reach. Without it,
  a user cannot distinguish *denied by policy* from *broken*, and will not connect
  losing access to having departed a patron (design §11.5).
- **Do not present resources the user cannot use.** The serving node filters its
  catalog page by what the viewer holds; the client should not re-expand it.
- **Access is gated by current membership in the resource owner's Dunbar Org**
  (design §11.2), so it changes without any action by the user, on joining,
  departing, or crossing a tenure boundary. The departure warning in §6 above is
  the case worth warning about, since the user is acting deliberately and the
  consequence is elsewhere.

---

## Open

- Whether recovery should restore archive history, and how, without handing an
  attacker the same path (design §22).
- Multi-device behaviour beyond merge: which device holds what, and how a user
  understands their archive spanning several (design §23.3).
