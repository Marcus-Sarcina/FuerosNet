# Notes: session (SES) and queue (QUE) acceptance entries

Draft of 2026-09-09. 16 SES entries (milestone 2: SES-01..05, 13, 15, 16; milestone 4:
SES-06..12, 14) and 16 QUE entries (milestone 3: QUE-01..09, 14..16; milestone 4:
QUE-10..13). 11 entries carry an interpretation. Validated against the specification
files with the repository's own `catalogue.py` helpers (sections, normalise,
cite_target), read-only: 0 flags. Nothing in the repository was modified.

## 1. Assigned functions and their specification basis

Every assigned function has a rule sentence behind it. What is missing in several
cases is an **observable**: the design states the behaviour and no document encodes
how it shows on the wire. Each such gap is declared in the entry's `interpretation`.

- **"Tell the sender" at the cap** (design §14.1.6, infra §2). No wire encoding of the
  sender-facing refusal, and none of the message submission itself: payload delivery
  and queue drain are named as unidirectional streams (`wire-format.md` §9.2) with no
  body format, and design §14.2.4's integration decisions are open. QUE-01, QUE-06
  and QUE-07 say so; every QUE entry observes the queue through `AttachAck` field 4
  and the drain streams for that reason.
- **"Offline" versus "no record"** (design §14.1.2, §7.4.3). The distinction is
  asserted as something the patron must make; no document says what a sender is
  answered for a keyhash the node has no record of, or whether the distinction is
  meant to be visible to the sender at all rather than internal to §7.4.3's
  verifier-query queue. QUE-11 checks only that the two answers differ.
- **"This state replicates to siblings"** (design §14.1.2). No object carrying a
  client's reachability state between siblings exists in `wire-format.md`; §3.4
  replicates "traffic" and §10 propagates topology. SES-14 observes the sibling's
  state and not what carried it.
- **No countersigning in a degraded session** (design §14.1.2). Countersignatures
  live inside transactions (`wire-format.md` §4.1, §4.6); there is no session-level
  request frame, so SES-10 observes only the absence of a countersignature by the
  sibling.
- **"Begins queuing"** (design §14.1.2 item 3) has no queue-side observable distinct
  from the client being unattached, because `wire-format.md` §8.2 makes queuing
  continuous (see §2 below). QUE-10 tests the combined scenario: marked unreachable,
  message arrives, delivered at the fresh attach.
- **Termination on supersession** (design §12.6.5, infra §2). No close code is
  assigned for ending a session under a superseded credential nor for refusing an
  attach under one; QUE-14 and QUE-15 check closure and absence of delivery only.
- **Queued material and the successor credential.** The specification says nothing
  further is delivered *to the superseded credential*; it does not say whether the
  successor collects what was queued for the predecessor keyhash. QUE-16 encodes the
  Tamarin model's reading (`delivery_is_to_the_credential_it_was_queued_for`) and says
  so. `models/tamarin/compliant/README.md` §2 already records that refusing to
  *enqueue* after supersession is not claimed; no entry tests it.
- **Crash copies.** The only rule is that no crash-recovery copy outlives a delivery
  (QUE-04). Loss of *undelivered* mail on a crash is accepted by design §14.1.6, so no
  entry asserts durability before delivery.

## 2. Inconsistencies noticed (not fixed)

1. **"Begins queuing" versus "queuing is continuous".** design §14.1.2 item 3: the
   patron "marks the client unreachable and begins queuing". `wire-format.md` §8.2's
   closing line: "Queuing is continuous — there is no 'begin queuing' signal, and
   reachability is only a hint about when to attempt delivery." The design wins on
   any disagreement, but the wire's reading is the one under which the marking has
   no queue-side effect a test can see.
2. **Where the mailbox is: "direct patron" or "serving node".** design §14.1.6 and
   infra §2 put the queue at "the direct patron" with a "per-subordinate" cap;
   design §14.1.2 opens with "Attachment answers one question: where do my messages
   queue" and item 1 says the serving node "is not necessarily its patron" and that a
   light-client patron "does not serve sessions". When the patron is a light client
   the two readings name different nodes. The entries use N as both patron and
   serving node so as not to depend on the answer.
3. **"Interval unset" stated without its bounds.** design §14.1.2 item 2 says the
   heartbeat interval is unset with no qualification; design §21.1 and
   `wire-format.md` §8.2 fix the units (seconds), the range (1..3600) and the 3-miss
   threshold, leaving only the value open. Not a contradiction, a summarising
   sentence that has drifted from the table.
4. **Citation of §14.1.4 for queuing.** infra §2 ("Hold ciphertext for offline clients
   (design §14.1.4)") and design §12.6.3's payload table ("Queued ... until reconnect
   (§14.1.4)") cite the mobile-OS-policy section; the mailbox rules are in §14.1.6.
   §14.1.4 does carry the store-and-forward decision, so this may be intended.

## 3. Questions only the author can answer

1. Does a successor credential collect material queued for its predecessor's keyhash,
   or does that material go nowhere (the Tamarin reading)? QUE-16 depends on it.
2. When a node marks a client unreachable, does it end the session, or leave it open
   for the client's own detector to act on? QUE-10 avoids the question.
3. Is the "offline" versus "no record" distinction meant to reach the sender, and if
   so in what form? Or is it internal, serving only §7.4.3's queued verifier queries?
4. Is the mailbox the direct patron or the serving node when those differ (item 2
   above)?
5. Is the per-subordinate cap measured in storage (bytes) or in messages? "Storage
   cap" suggests bytes; the entries size the cap so that k messages fit and do not
   choose.
6. Should supersession-triggered termination, and an attach refused on supersession,
   carry application close code 1 (`refused`), or a code of their own?
7. What carries a client's reachability state between siblings, and is it within
   §3.4's replication floor ("every user's messages replicate on their nearest infra
   node and that node's siblings")?
8. Is SES-07's reading right that failover begins on waking, with the dial itself
   subject only to local timeout policy?
