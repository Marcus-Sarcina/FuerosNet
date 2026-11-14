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

**On the force of these requirements**: commitments, not enforceable rules —
`network-design.md` Appendix A.2 states why, once, for all three requirements
documents. Where a requirement leaves a visible artifact, that is noted in place.

---

## 1. Ceremony

**Either party may begin.** Offer both ways in: a control that shows an invite
for the counterparty to read, and a control that reads one. Which the user chose
has no bearing on the ceremony's outcome or on who adopts whom if an adoption
follows (design §7.1); the pair takes a start time from whoever began, within
clock tolerance, and the one who began proposes the body for signature
(`wire-format.md` §14.3.2, §7.10.1). The two intents say which side began
and must disagree; refuse one that claims your own side. **The first code
you read is who you are meeting**: it carries their full key, which you pin
at this first contact (design §12.3; `wire-format.md` §14.3.1), and a later
code naming anyone else is refused. Nothing you were told beforehand names
the counterparty; the code does.

**Provisional, until the second shell exists** [author, 2026-10-08]: **two
shells cross the optical channel only if they share one framing.**
`wire-format.md` §14.3.1 fixes the bytes and pins no symbology, so how those
bytes are cut into codes, what each code's header carries and how a receiver
shows what it already holds are the shell's own — and two shells that chose
differently cannot read each other's screens at all. The reference shell's
framing — the object in parts under a short header, the parts held drawn as
rows beneath the code — is what a second shell is built against, and this
paragraph is restated as a requirement, compatible with both, once the iOS
shell exists.

### 1.1 Nomination

- **Nominate witnesses from your own contacts, up to eight, before the
  counterparty has accepted anything** [author, 2026-10-19]. The pool is the
  acting subnet's trust horizon — held as contacts you reach directly (§4.2) —
  and the parties you have met in person anywhere; your horizons in other
  subnets, and any node you have ever learned of, widen it at the person's
  choice, the client saying what feeding a wider sample discloses (design
  §7.1.1, §19.2). Engage the nominees with the intent: they are observers from
  before the ceremony-id exists, and that is their value — a witness shown
  only finished records adds no friction to a replay (design §7.1).
- **Spread nominations across as many independent branches of the pool as you
  can, and include a random element.** A selection concentrated on one branch
  hands the counterparty's prune nothing to keep but that branch, and a
  predictable selection is one a well-placed node can sit in wait for (design
  §7.1.1).
- **Probe your selections for availability; do not nominate from a set of
  nodes that advertised themselves.** Many nodes worth nominating are inactive
  or not running at ceremony time, so availability is something you discover
  after selecting. A node that solicits nomination is self-selecting, and the
  counterparty's prune is what it now has to get past (design §7.1.1).
  **Engaging is where you discover it, and it is retried, not decided once**
  [author, 2026-10-19]: a nominee you hold no session material for is asked
  for on the first attempt and reached on a later one, so a transient
  absence is not an unavailable witness. **The set you offer the prune is
  the nominees the engagement reached** — one still unreached when the
  ceremony-id fixes was unavailable, and is a witness that does not appear
  (design §7.1), never an entry the counterparty is asked to keep blind.
- **Stream the opening to your nominees as it happens** [author, 2026-10-19]:
  the intent; then the optical exchange's shape — the number of blocks you
  have to transmit, the number the counterparty declares, the index of each
  inbound block as it lands and of each outbound block as the counterparty's
  display acknowledges it — and **never its content**: not a contribution,
  which is half of what keys the local session (`wire-format.md` §14.3.2), and
  not the bundle. When the transcript confirmation fixes the ceremony-id, send
  it to each nominee against the engagement's handle, so what they watched
  binds to the record they may later sign.
- **Exchange witness sets by locator over the local channel, then prune the
  counterparty's to three** [author, 2026-10-19] — `min(3, offered)` — keeping
  spread rather than convenience, since their locators are in your hand
  exactly so your choice can be blind to their loyalties and sighted on their
  diversity. Relay survival or dismissal to your own nominees yourself: they
  are your contacts, reached by your routes. From here, send every leg to the
  surviving witnesses of both sides.
- **Aim for two of your witnesses signing at the end; the third is margin.**
  All who remain live sign — nothing promotes an alternate, and a role would
  need machinery the margin does not. Check the `nominated_by` split before
  you sign, and tell the user when your own nominees are absent or
  outnumbered: a record attested only by the counterparty's survivors is a
  record your side of the story did not witness, which design §6.1.1 leaves
  visible rather than mandatory.
- **Refuse to sign a record attributing to you a witness you did not nominate**
  [author, 2026-09-03]. Your client knows exactly which witnesses it nominated
  and which survived the counterparty's prune; an entry claiming
  `nominated_by = you` outside that set is the counterparty dressing its own
  curation in your name. The wire cannot check this (`nominated_by` is a
  recorded claim); you can, and you are the only party who can.

- **Send each message of the ceremony's conversation to your counterparty and
  to every surviving witness either of you engaged** (`wire-format.md`
  §7.10.1): the consent each query needs, the fishing proposals, the witness
  request and answer, the responses you gathered, your back-pointers, the
  proposed body, your signature entry and the finalised record — **as each
  happens, never batched**, because the arrival pattern on the witnesses'
  clocks is the evidence (design §7.1). The consent request alone goes to its
  subject only: it carries the fuzzed profile, which reaches the one verifier
  the query names and nobody else (`wire-format.md` §5.6).

### 1.2 Acting as a witness

- **Decline to witness a ceremony whose claimed `started_at` is far from the
  time you observe.** Your clock is the only independent one at the ceremony,
  and the chronology bound (`wire-format.md` §3.3) is worth nothing unless
  witnesses apply it. No later validator can check that you did: set the
  tolerance you can defend and refuse outside it.
- **Attest from what reached you, and from when it reached you** [author,
  2026-10-19]. The ceremony's conversation is sent to you as it runs
  (`wire-format.md` §7.10.1): the engagement before the counterparty accepted,
  the optical exchange's block trace at screen-and-camera pace, the
  ceremony-id when it is fixed, the request, the channel outcomes, the queries
  and responses, the proposed body and the finalised record. What you sign is
  that the protocol ran and both parties were responsive as the sequence you
  received shows (design §7.1) — the arrival pattern on your own clock being
  the one thing a replay cannot cheaply counterfeit — without asking your
  operator, who saw none of it (design Appendix A.3). A dismissal at the prune
  ends your part: keep nothing you were sent except what your own records
  policy keeps of anything.
- **Do not sign before four minutes of ceremony have passed on your own
  clock** [author, 2026-10-06]. Four minutes, whatever the participants, the
  other witnesses or the verifiers say, and however early the conversation
  reaches you complete: the ceremony is priced in human time (design §7.1) and
  your clock is one of the few at a ceremony the parties do not hold. An eager
  witness is the one an automated pair would want, and the only thing that
  stops them having it is that neither of them chose you (design §7.1.1).
  **Nobody can check that you waited**, as nobody can check the tolerance
  above, so this is a commitment and not something a reader of the record
  enforces. **Measure from your own receipt of the request, against your own
  clock, with no fudge factor** [author, 2026-10-06]: the arrival is the only
  instant of the ceremony your clock witnessed, where `started_at` is a claim
  the parties made. The participants will have been at it longer than four
  minutes by then — the engagement and the codes precede your request, while
  the channels, the capture and the verification run inside the interval you
  are timing [author, 2026-10-19] — and that is the intended shape, so do not
  discount your four minutes by what they claim to have already spent. **And withhold your
  reply rather than refuse** [author, 2026-10-08]: your answer to the request
  acknowledged the ceremony; your signature, once your clock has matured and
  you have seen the by-products of the steps you were sent, is your readiness.
  No reply names waiting (`wire-format.md` §7.10.2), and the proposer waits
  on an unanswered signer.
- **Hold only a few ceremonies as a witness at once.** The bound is yours; the
  reference client holds eight, and past it the request with the oldest claimed
  start goes. A request for a ceremony you already hold gets the answer you
  gave. **Having declined, take nothing further of that ceremony.**

### 1.3 Capture

- **Hold the counterparty present for the whole enforced duration, with the
  camera** [author, 2026-10-06]. The check that a face is in front of the lens
  rather than a blank wall is one the capture needs anyway (design §7.5); over
  the four minutes of design §7.1 it is also how this device establishes that
  it met one person continuously, and **it is the user's own interest it
  enforces** — nothing about it enters the record, no counterparty or witness
  can tell that it ran, and a client that skips it weakens the standing of
  nobody but its own user. So:
    - **Probe for a valid face throughout**, not only at the capture.
    - **Sample the frames where one is present**, at a frequency you state.
    - **Compare the samples across the meeting**, so what is established is
      one person's continuous presence and not a sequence of strangers.
    - **Abandon the meeting** where no face is visible for longer than the
      maximum gap; where a different face appears while the one being held is
      absent; or where the gaps sum past the allowed ratio by the end of the
      four minutes.
    - **Take the captures to be archived from across the whole window**,
      rather than in one burst: the prompts fall at random through it and the
      retained images are the ones taken at them [author, 2026-10-06], so one
      mechanism spreads the archive and makes it a liveness check.
    - **Hold nothing of the rest once the ceremony closes.** A continuity
      sample is compared and discarded; design §7.5.2's commitment — that a
      compliant client holds no decryptable likeness of another person — binds
      the samples as much as the archive, so they are compared in memory,
      never written unsealed, and gone when the meeting is.
    - **Retaining every intermediate sample is not required.** A lookback
      longer than the maximum gap is enough to establish continuity. **But a
      client that prunes must choose its keepers as it goes**, or every
      archived capture comes from the last minutes of the meeting, which is
      the one stretch a sample of the whole was supposed not to be.
  The reference parameters are **four minutes, a ninety per cent presence
  ratio and a twenty-second maximum gap**, and they are the client's to adjust
  [author, 2026-10-06]. **A client that sets no duration holds the window the
  capture itself takes** [author, 2026-10-18], which is the setting to use
  while a bench is run often: the loop and its three rules are unchanged, and
  the only difference is the length of what they run over. Note what follows
  from it rather than meeting it by surprise — against a window of ten to
  fifteen seconds a twenty-second maximum gap cannot be exceeded, so the
  presence ratio is what catches a long absence there and the gap rule bites
  only once the window is longer than it. **Use the profile and the matcher verification already
  needs** (design §7.3, §7.5) [author, 2026-10-06]: comparing two of your own
  samples taken seconds apart is the same question a verifier answers, at its
  easiest — one camera, one light, one minute — against the cross-device
  matching across months that the protocol already rests on. So this needs no
  second algorithm and nothing further pinned; what the platform must supply
  beyond it is the presence detection that says a face is there at all.
- **Refresh the reference image on every subsequent ceremony** with the same
  counterparty, so the stored image tracks the person rather than the first
  meeting (design §7.5).
- **Run the capture loop for the whole capture portion, and retain 3–5 of what
  it takes** (design §7.5) [author, 2026-10-06]. **The randomised prompts are
  the requirement; the ten-to-fifteen-second window was not** — turn, smile,
  frown, whatever the interface comes to show the person in front of the
  camera — so spread the prompts through the capture and **sync the retained
  images to them**, which is what makes each retained image carry the motion
  and parallax a printed photograph cannot. The loop between prompts is the
  continuity check below, and most of what it takes is never retained.
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
  revealing them only on the user's instruction. Nine of the eleven exchanges that
  receive a record need none of them, so the default is the correct one and the
  reverse would make the mechanism decorative.
- **Tell the user what revealing location means** at the one exchange where it has a
  use — presenting an archive to a prospective patron. Revealing offers the geohash
  series that supports an impossible-travel check; withholding is visible and declines
  to offer that evidence. **Both are legitimate**, and the same series is what P21
  turns into a behavioural record, so the user is choosing between being checkable and
  being trackable rather than between honest and evasive.
- **Keep one sealed capture per presence record**, ageing each independently. A
  grant names which one opens (`wire-format.md` §7.3); when you are the subject
  releasing a key, the most recent eligible capture is the default (design
  §7.5.2).
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
  answer `photo_match` if the user later needs a rotation, leaving the weaker
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

### 1.4 Verification

- **Hold the anti-oracle aggregate as a lock, not a log.** A counter per requester
  and per ceremony window, discarded when the window closes (design §7.4.1). It
  exists to refuse the next query; retaining a queryable history of who probed you
  builds a timeline of ceremony attempts on a device that can be seized.
- **Query verifiers automatically.** A client treating a face-to-face encounter as
  evidence of identity continuity must issue the queries without asking; a client
  that skips them silently produces evidence weaker than it appears (design
  §7.3).
- **Put each query to its verifier on the end-to-end path, and take the answer
  there** (`wire-format.md` §5.6): direct where you hold a path to the
  verifier, through the serving nodes otherwise, as any payload goes (design
  §12.6.3). A verifier that is your own serving node is asked on the session.
- **Select your counterparty's verifiers by your own recognition, and say which
  was which** (design §8.1.2): people you have met, people in any of your trust
  horizons, then one further edge, and go fishing for common acquaintances
  before filling the remainder at your discretion, marking each response's
  basis honestly (`wire-format.md` §5.5). **Review who was selected for you
  before signing**: the record will carry those responders forever, and you
  cannot repair it.
- **The bundle you hand is introductions to prior counterparties you choose
  to name, each with the most recent locator you hold for them**
  (`wire-format.md` §5.4) [author, 2026-10-11] — a handful meant to
  suffice, never your meeting history. **Fish one candidate at a time, in
  turns, and only where the bundles left the criterion unmet**: each
  proposal is one disclosure decision through the same interface that
  curates the bundle, and once you have exposed as many counterparties as
  you care to, accept the next proposal and move on. Show the user whether
  the candidates they offer are genuinely unavailable or are being
  declined: after the bundle, the working assumption is that **every
  mutually reachable candidate is accepted if available** — quality
  filtering stops with the initial bundle, so a counterparty declining
  available candidates to pull more names out of you is visible as exactly
  that.
- **Withhold your envelope signature from a proposed body omitting a verifier
  response you hold** [author, 2026-09-03]. Verifiers deliver a copy of each
  response about you to you (`wire-format.md` §5.6); a proposed final record
  missing one is the counterparty editing your evidence, and your signature is
  the veto (design §7.4.2). A response you never received is the thin-record
  case and signs normally.
- **Read each response copy against the retention its verifier declared**
  [author, 2026-09-30]. You chose the grant, so you know which prior record
  that verifier should still hold and the retention it committed to there
  (design §7.5.1); an `unavailable` inside that window is your private
  signal about that verifier's caching compliance. It is yours alone to
  attribute — a response names no prior record, by decision (design §7.4.3),
  since the counterparty could not read one without the whole prior record —
  and nothing of it enters the record.
- **Compute *n* over the bundle handed to you, and treat it as your
  counterparty's claim** (`wire-format.md` §5.2 and `wire-format.md` §5.4). The bundle is theirs to
  curate — records from any of their series, no chaining, no completeness, so
  verify each record alone, count qualifying ones once by txid, and read the
  result as sizing your diligence, never as a fact about their history. A
  bundle larger than one carriage arrives in numbered continuations
  (`wire-format.md` §14.3.2); how many you read is your resource policy, and
  *n* is what you hold.
- **Report a record as unverifiable, not invalid, when you lack a participant's
  history.** Those are different answers and a caller may act on the difference.
- **Look at the candidate population you are sampling, not only at the answers it
  returns.** You enumerate the counterparty's prior counterparties in order to select
  from them, so you can see whether you recognise any of them before you see a single
  reply. **Tell the user when you recognise none of it**: a `match` from strangers
  establishes nothing, however many you queried, and a candidate set holding
  nobody you know defeats the shared-identity check outright rather than merely
  weakening it (design §7.3). **And ask anyway** [author, 2026-10-30]: recognition
  is how you rank the offered, never a reason to query nobody. Ask at least one
  prior counterparty wherever one is offered, never more than the criterion
  allows (design §8.1), those you recognise first and strangers after, marked as
  your discretion; a subject with one prior counterparty is asked of that one,
  and the record stands with or without the answer.

  With the `nominated_by` check in §1.1, **these are the three checks that protect
  you against the person in front of you** rather than against an outsider.

### 1.5 Disclosure at capture time

**The schema cannot fix comprehension** (design §19.6). These are the client's
job.

- **Tell participants what the record will contain and who will be able to read
  it**, at the moment of capture, not in a policy document.
- **The number a device reports for itself crosses at a meeting, both
  ways, and nobody is asked for it** [author, 2026-11-10]. Read it off the
  handset at meeting time and carry it to the counterparty **on the local
  bearer, beside the intent** (`wire-format.md` §14.3.2), so that an
  invitation to a node a patron stands up later can arrive without waiting
  for the next meeting (design §12.6.3's code is the alternative). **No
  control collects it and no screen explains it**: a number is plumbing,
  and the people meeting are doing something else.
  - **Nullable, and that is the ordinary case.** A number is
    carrier-provisioned rather than device-known, so a handset without a
    subscription, a carrier that never provisioned one, or a platform that
    will not answer all send nothing — and a meeting where neither side
    sends one is unchanged in every other respect. **Never prompt for what
    the platform would not give.**
  - **It goes in no transaction and to no witness or verifier.** What
    crosses is the local session between the two devices present, and the
    record carries nothing of it.
  - **Keep it with the rest of what is held of that party**, so it is
    preserved by what already preserves a contact: this client's store and
    the backup a peer holds (design §6.3). A later meeting replaces it; a
    meeting that carries none leaves what is held alone, a device whose
    platform stopped answering not having withdrawn what it gave before.
  - **It is not handed on with the contact.** A card that introduces a
    party carries its key material and its locator (`wire-format.md`
    §2.3, §2.4) and nothing else: the number was given to the device that
    was present, not to its acquaintances.
  - **Plaintext, and nothing structural keeps it.** A likeness is sealed
    under its subject's keys, so a holder retains nothing it can open
    (design §7.5.2); a number is text in another person's phone. Design
    §19.8's C25 registers what holding many of them composes into.


---

## 2. Archive

- **Never take a series reissue into a series you have occupied before.** The abandoned line's
  high-counter records would come back into comparison against the new one
  (`wire-format.md` §4.6). You hold your own chain, so this is yours to check, and a
  counterparty holding that chain will reject a reissue that repeats a series in it.
- **Keep the chain that proves your current series**: your adoption and every reissue
  since, and present it when a counterparty needs to rank two of your records
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
- **The identity's key material comes in and goes out as bytes, from a source
  the application names** [author, 2026-10-17]. A new identity is made the way
  it always was, and a second signing device comes to hold it because its
  holder put the material there and not because a device sent it (design
  §23.3). **Everything below the application takes it as bytes** — handed in at
  start (§9), or read from a file — and nothing in the kernel knows where a
  person kept it. What a restore of it restores is the key and every key the
  identity derives, the capture keys that unlock your likeness on other
  people's devices among them (design §7.5.2.6); what it does not restore is
  the archive, which comes from the backup or not at all (design §10.2; §6).
  Say both.

- **A seed phrase, if you offer one, is a feature of its own and nothing
  depends on it** [author, 2026-10-17]. It encodes the identity's key material
  as words for a person to store, and reads those words back into the same
  bytes; **that is the whole of its contract**. It lives above the kernel, has
  no surface inside it, and **deleting it must leave the client working** with
  the material given as a file instead — which is the test of whether it was
  built as a feature or wired into the identity. Making an identity may offer
  it, and doing so is a display of the material and not a different way of
  making one. Show it once, when the identity is made, and never again on
  demand: **a phrase a screen will redisplay is a phrase anyone holding the
  unlocked device can take**. A client that offers none is conforming.
- **Pruning the chain must not delete presence records or sealed captures.** A
  checkpoint bounds what the *chain* must retain; presence evidence is a
  separate kind of history that stays usable under any sequence and in any subtree
  (design §10.0). **There are no capture seeds to keep**: a capture key is
  derived from your own identity and the ceremony's id when you want it (design
  §7.5.2.6), so what a pruned record must still yield is the record and its
  ceremony-id, not a secret stored beside it. Deleting it with the chain would discard the evidence a later
  adoption and a later rotation both depend on, and would put a patron's willingness
  to countersign a series reissue astride evidence design §6.4 says it must not gate.
- **Keep retention and disclosure as separate controls.** Choosing how far back to
  *disclose* must never delete anything: disclosure is reversible and can be widened
  later, up to what is retained. Deleting is the one-way door, and it caps every
  future disclosure, so it belongs behind its own deliberate control, never as a
  side effect of choosing what to show today. **Say that it takes the window's
  capture keys with it** [author, 2026-10-17]: those derive from ceremony
  pre-commitments you keep beside the records and nothing serves one back
  (design §7.5.2.6), so deleting a window makes every capture sealed in it
  permanently inert on every counterparty's device. That is the one way to put
  old captures beyond your own reach, it is all-or-nothing across the window,
  and it is the same price as putting any other history beyond reach.
- **Treat a series reissue as deliberate, not routine.** It costs nothing in routing, since
  the path is unchanged, but it is the boundary beyond which you may prune, and
  pruning is irreversible. **Never prune inside the 730-day window**: those records are
  what verifier selection counts, and discarding them lowers your own verification
  threshold, which is a thing an evaluator can see you did (design §10.1).


- **Offer a way to take an identity off the device, as its own control**
  [author, 2026-11-07]. Starting a new identity and removing the one held
  are both necessary and they are **two functions, not one**: fused, the
  only way to start fresh would be a destruction and the only way to
  destroy would be a fresh start, and a person confirming one control
  could not say which they had agreed to. So removal clears the key, the
  archive, the store and every capture sealed to that identity, leaves the
  device holding no identity, and the doors for starting or restoring one
  take it from there. **Say what it cannot give back**: the seed phrase
  and a backup are the only way to that identity again, and a device
  without both is the last place it existed. **It is not a rotation**
  (design §9.0): a rotation replaces the key a subnet knows you by and
  keeps you in that subnet, while this keeps nothing and tells nobody.
  **And what comes after is a genuinely-new user, vouched for afresh** —
  not a person who lost a status they hold, which is a different claim and
  a false one (design §9.0, PRD-03). **Do not assume one identity per
  device** [author, 2026-11-07]: supporting key rotation means supporting
  more than one, so removal is of *an* identity and adding one is another
  way into the same door that starts a new one.
- **There are two doors, and each says which it is** [author, 2026-11-11].
  A device holding several identities can be asked for either act, and a
  control that said *this identity* while taking the device would have a
  person confirm the wrong one:
  - **Take one identity off, leaving the others.** Its key, its archive,
    its store and the captures sealed to it go; the other identities are
    untouched, and the device keeps acting as one of them. **Refuse where
    it cannot be honoured rather than half-doing it**: the only identity
    held (the other door's act, which also clears the wrapping key), and
    an identity whose storage is the device's own rather than its own
    directory, where singling it out would mean inventing a list of files
    to delete. **Refuse while that identity owes a departure from a
    rotation**, or the key it replaced is left standing in a subnet with
    nothing on the device able to say so. **Name the list before the
    storage goes**, so a process that dies between leaves storage nothing
    opens rather than a list naming storage that is not there.
  - **Take the device**, which is every identity on it and the wrapping
    key with them, leaving a device holding none. The words say *every
    identity* and *their keys*, since one of them is not what goes.
- **Hold several identities at once, and divide what they may do**
  [author, 2026-11-07]. A key rotation replaces the key one subnet knows
  and leaves every other subnet untouched (design §9.0, §13.6), so a
  device that has rotated holds a key that is still live elsewhere: the
  rotated-away key **stays usable to join new subnets and to act in any
  subnet that was not rotated**, and only on the subnet it rotated does
  it lose its reachability and the standing that depended on it.
  - **Concurrent for viewing and for acting within subnets.** Every
    identity the device holds is usable at once for what happens inside
    the subnets it belongs to.
  - **A conversation continues in the identity it started in.** Which key
    a direct exchange belongs to is settled when it begins and is not a
    choice made again later.
  - **One identity at a time for a meeting or an operator action**, so
    neither has to ask which key it belongs to. **Chosen by press and
    hold on the nameplate at the foot of the home screen**, which opens a
    scrollable list of the identities held. **The list carries the door to
    another identity as its last row** [author, 2026-11-09], so it always
    has something to offer and the only way to add one is not a control of
    its own somewhere else.
  - **An identity is one name, and each subnet it holds a position in is
    an address of that name** [author, 2026-11-11]: *"Each ID:Subnet is
    effectively a unique FN address. Work, school, home."* An address has
    its own node — the one serving the identity in that subnet — its own
    path and its own mail. A party you met in one subnet holds your
    locator for that subnet alone (§4.3, one per anchor), and nothing
    merges your addresses into one path, which would tell them which
    other subnets you are in.
  - **A session per address.** Attach to the node serving each of your
    addresses, publish your bundle to each so sessions open there, and
    receive at each; never route one address's mail through another's
    node. The serving node of §4, and of the wire format throughout, is
    one such session described alone: the wire level speaks of one client
    and one node, and this is the dimension it does not carry (design
    §12.9).
  - **The key is a name, and identities are the unit of unlinkability**
    [author, 2026-11-11]. The same bundle reaches every address's node, so
    two nodes comparing notes can link an identity's addresses by its
    key: intended, and often the point of using one name. Where audiences
    must not be linked, that is a second identity, never a second key.
  - **In focus, one identity opens new tabs.** Joining a subnet, attaching
    to a node and starting a thread are the focused identity's acts;
    receiving is every identity's, at every address, in or out of focus —
    as a browser out of focus keeps its tabs open and its notifications
    arriving.
  - **Each identity has a colour, and what the focus changes wears it**
    [author, 2026-11-12]. The nameplate, the door to the person's options
    and the door to a meeting take the colour of the identity in focus;
    each subnet's door takes the colour of the identity that holds it;
    each thread in the list of direct messages is marked with the colour
    of the identity it is held as. The colour is a choice in that
    identity's options, and a new identity takes one no identity on the
    device uses.
  - **A row a screen shows carries the identity that holds it, and acting
    on that row acts through that identity** [2026-11-07]. The merged
    lists are the point of the first rule, and a screen that acted on a
    merged row through whichever identity happens to be active would act
    as the wrong key — an act nothing would refuse, since the key it used
    is one the device holds. Say the wrong thing and a reader can correct
    it; sign the wrong thing and they cannot.
  The reason for the division is the author's and is not restated here.
- **A rotation makes a new identity beside the old one** [author,
  2026-11-07]: *"just start a new archive for the new identity … The old
  archive should show the old ID leaving the subnet that rotated, but
  should otherwise remain unchanged."*  So the new key gets storage and a
  history of its own rather than the old one's, and from the person's own
  standpoint it is a new identity that arrived already holding the roles
  and state the old one had in that subnet — which the patron's adoption
  is what carries, not a history copied beneath it. **The old key leaves
  that subnet once the new one is adopted into its place, and not
  before**, so the person is never briefly outside the subnet they are
  rotating inside; the leaving is the old key's own act in its own archive
  and it is owed until it is signed. **And the new key takes the address
  the old one held** (design §9.1), so a rotation inside a full subnet is
  possible: it adds no member.
- **Merge automatically on noticing divergence.** A user has no reason to want
  outstanding branches, and an unmerged fork means their history is incomplete
  wherever they present it (design §10.3).
- **Maintain backups, and make the consequence visible to the user.** Archive loss
  is loss of portable history, since the archive is a second factor: key without
  archive yields no transferable standing (design §10.2). Nobody can check that
  a client backs up; what a user *can* check is whether they were told what losing
  the device costs. On a host with evictable storage, saying so is the substance of
  the obligation.
- **Prompt for a backup until one exists.** The bullet above makes the
  consequence visible; this is for the user who has read it and still has no
  backup, which is the case it was written for. A desktop client is one
  destination and a password manager or consumer cloud sync another — design
  §13.7.1's envelope is what makes any of them safe to use.
- **Scan any imported backup for expired retention** and delete what is past its
  window. Import is exactly where an over-retention leak occurs (design §13.7.1).
- **Hold a peer's backup as ciphertext, and give a peer only what a peer holds**
  [author, 2026-09-29]: the contacts and the transaction records, under design
  §13.7.1's envelope. Not the sealed captures — those go to a desktop or
  third-party backup with the archive (design §23.3) — and never your identity
  seed, which leaves the device that performs ceremonies for nowhere (design
  §23.3). Peering is priced in a
  meeting and memorialised in a record (design §6.3); the backup is what the
  record entitles the two of you to, and nothing else does.
- **Keep your peers' endpoints current, and tell them yours** [author,
  2026-09-29]. A peering record's endpoint is a snapshot; send your peers your
  newest signed locator when your position changes and, when you add an
  instance, its endpoint record (`wire-format.md` §4.4, §7.6), each on the
  end-to-end channel and verified by them under the key the peering already
  names. Once both of you run instances, route to each other directly.
  **Hold a peer's locators per subnet**: one per anchor, as your own address
  table does — a locator for an anchor you already hold for that peer replaces
  it under `wire-format.md` §2.3's freshness rule; one for an anchor you do
  not inserts a new entry (design §3.1.1: a user is bound into several subnets
  at once, and holds one position in each).
- **An operator's backup carries their provider credential too.** It belongs in
  design §13.7.1's envelope with everything else and **not in the archive**,
  which siblings replicate (design §3.4) and which is the wrong place for
  something nobody else has any business holding. Encrypted under the
  identity's own key (design §13.7.1), it is as protected as the records
  beside it, and it makes a restore a whole one: a replacement device regains
  the ability to administer, not only to participate. It is also the piece whose
  loss is recoverable outside this network, a provider having its own account
  recovery of its own and its own facility to withdraw and reissue.
- **Restoring your own archive from a holder is an act of trust, and the
  interface must say so.** A client that lost its archive does not know its own
  head, so it fetches from the newest record the holder claims
  (`wire-format.md` §7.9) — the chain verifies internally, its completeness does
  not, and the same reduced footing extends through the whole lost-archive path:
  re-adoption on a fresh series presents no history and rests on the patron's
  personal judgment. Surface both as what they are: a restore on trust rather
  than on verification, and never present a restored archive as
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
  **Serving is owed in the initial release; reviewing what was served is not**
  [author, 2026-11-09]. A patron forming an initial trust state by reading a
  candidate's presence records is deferred past it (design §16.7, §23.1) —
  *"it demands a level of platform sophistication which an initial adopting
  public won't have"* — so a client serves, and a client that fetches verifies
  the structure of what came back, and neither is obliged to weigh it yet. What
  an evaluator reads meanwhile is its own copy, asking nobody (design §16.2).
- **When fetching someone else's archive, verify the structure yourself.** Every
  returned record must be named by the frontier you requested or by another
  returned record's back-pointers, and every back-pointer naming nothing in the
  batch must appear in the reply's frontier (`wire-format.md` §7.9); order is no
  part of it. **A holder cannot be trusted to have walked
  correctly**, and the check is hash comparison over records already being parsed.
- **Absence of data from a holder is never evidence about that data's existence.**
  A holder may refuse, rate-limit, or return less than asked for any reason; none
  of it says anything about what exists. Concretely: a short archive reply is not
  a short archive.
- **Hold what awaits a prerequisite under a bound, and let the reconstructible go
  first** (`wire-format.md` §10): a reissue or an endpoint record returns through
  gossip or a later locator distribution; a disavowal or departure held for the
  adoption it precedes does not, and outlasts them at the ceiling, oldest first
  among themselves.

---

## 3. Payload encryption

design §14.2.4 adopts **PQXDH** for key agreement and the **Triple Ratchet** for
session secrecy. The client implements them; it does not reinvent them.

- **Wipe key material rather than leaving it in freed memory** [author,
  2026-09-26]. Root keys, chain keys, message keys and prekey seeds are
  wiped when they go, including the copies a ratchet makes of its own state
  while deciding whether a message opens. This is not observable to a peer
  and §1.1 cannot reach it; it is stated because **no expert review is
  coming** and an obligation nobody wrote down is one nobody implements.

**The Double Ratchet is the floor** (design §14.2.4.3) [author, 2026-09-26]:
run at least it over a PQXDH session, and run the Triple Ratchet where an
implementation of the post-quantum half is available. A client running the
floor alone has the post-quantum session establishment PQXDH gives and lacks
post-compromise security against a quantum adversary. **Say which one is
running** where the user can reach it: no peer can tell from the wire, so the
only party who can tell them is their own client.

- **Publish a prekey bundle and keep it stocked.** A subject with no one-time
  prekeys left falls back to the last-resort key, which is a **declared reduction
  in forward secrecy.** Not a state to remain in. Replenish
  well before exhaustion (`wire-format.md` §7.8).
- **Rotate the signed prekey on a policy interval**, and never reuse a one-time
  prekey.
- **Every identity is a leaf.** A patron is a person on devices that may be
  offline, and a patron who runs no node is a light client like any other
  [author, 2026-11-14]: a message to one takes this construction, through
  the node that holds the device's bundle, and never rides a session to a
  node as material for the node. Only a resource is reached without it: a
  session to a resource terminates at an endpoint that is online by
  definition and already runs over an authenticated transport — the
  `rhtn/1` session, or the service's own TLS on a brokered connection
  (design §14.2.4, `resource-requirements.md` §3).
- **Prefetch reusable prekey material for the whole trust horizon as a batch request**
  (`wire-format.md` §7.8), which is structurally distinct from a targeted fetch —
  so the serving node sees a sweep rather than having to take your word for it. A fetch driven by peers' rotation schedules reveals *past* activity —
  someone rotated, rather than intent to message. Fetching on demand instead
  announces each intended conversation to whoever serves the bundle (design
  §19.8, C11).
- **Put a targeted fetch to the node that serves the subject** [author,
  2026-10-05]. A node answers a prekey request from what was published to it
  (`wire-format.md` §7.8), so the party holding a subject's bundle is the node
  that subject attached to, and your own serving node holds it only where it
  serves them too. The intent a targeted fetch discloses then reaches the party
  that is about to be handed the message rather than your patron (design §19.4,
  P26). Your own node is where it goes when that one cannot be reached.
- **Never prefetch one-time keys.** Serving one consumes it, so blanket prefetch
  would drain every pool in the org and make exhaustion the normal state —
  destroying its value as a signal that someone is draining a pool deliberately.
  Request a one-time key only when actually opening a session.
- **Accept the cost knowingly:** a session opened from prefetched material alone
  lacks one-time-key forward secrecy for its **first message**. Protection improves
  as ratchet contributions are incorporated, not immediately.
- **Write your sessions after every message you receive or send, not only
  after what you show the person.** A ratchet moves on every message, and a
  verifier's whole service — the session a grant opened, the query it
  answered — may show nothing. A process that ends holding sessions it never
  wrote cannot read what its peers go on sending, and has no session to say
  so on.
- **When a query or a grant has gone unanswered for half the patience you
  give a verifier, send it again on a fresh session, once.** A verifier that
  lost its sessions refuses everything sent on the old ones and cannot say
  so; only the sender can open another. Set the sessions you hold with it
  aside for what it may still send on them, open anew as for a first message,
  and send the same bytes. The verifier's crossing rule takes the open
  (`wire-format.md` §7.10). Once: after that the person decides whether to go
  on without the answer (§1.4).

## 4. Session

**Said of one session.** Everything here is one address's session with the
node serving it; an identity holds one such session per subnet it has a
position in (§2), each dialled, kept and failed over on its own, and
nothing below reaches across them.

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
- **Tell the user when attachment is degraded.** On a sibling, the subnet-scoped
  transactions the patron countersigns wait for the patron (design §6.4); a user
  who is not told will read this as the application being broken and will not know
  that reconnecting resolves it (design §14.1.2).
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
  (`wire-format.md` §7.7); it would reject the direct-serving answer a
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
- **Ask it of any infra node whose address you hold**, not only of your serving
  node (`wire-format.md` §7.7.1) [author, 2026-10-05]. The addresses above are
  what that choice is made from, and a client whose serving node will not answer
  has somewhere else to ask. What a resolution came to is a cache like the
  others: give it a lifetime and say what it is.

---

### 4.1 What you hand your serving node

- **Publish your bundle and stock your pool as part of attaching**
  (`wire-format.md` §7.10 and `wire-format.md` §7.8). A node that holds neither cannot answer for
  you, and a peer that cannot fetch your material cannot open a session with
  you at all. Each of your devices does this for itself, on its own session:
  the bundle it publishes carries material it generated and a signature your
  ceremony device made over it, and the pool it stocks is its own
  (design §23.3) [author, 2026-09-22]. **And a pool per address** [author,
  2026-11-11]: each node you hold a session with (§2) is stocked
  separately, with keys of its own, since a key in two pools could be
  spent twice.
- **Attach again after a loss, without the person's hand** [author,
  2026-11-04: *"all horizon members should re-attach every time they become
  detached"*]. A session lost to the node serving one of your addresses,
  or an attach at start that got no answer, is tried again with backoff
  (seconds growing to minutes, then steady) until it holds; a restart is
  not the way back. Each address is tried on its own.
- **Keep your sessions while you are not on the screen** [author,
  2026-11-14]. A mobile platform may refuse network to an application the
  person is not looking at; the client takes whatever standing the platform
  offers a process that must keep a connection (on Android, a foreground
  service and the notification it costs), so that mail arrives while the
  phone is in a pocket. **And works without it.** The platform, a vendor's
  build of it or a utility the person installed may refuse that standing
  [author: *"it seems like the kind of thing a mobile OS vendor or
  performance utility might choose to block in the future"*]: a client so
  refused runs the same, its sessions re-attached the moment it is visible
  again, and nothing is lost but the time it was away. Both states are
  tested, not the first alone.
- **Open a session with each of a recipient's devices, and send to each.** A
  fetch returns a bundle per device (`wire-format.md` §7.8), a session is with
  a device, and a message sent to one of them reaches one of them; the
  submission names the device (`wire-format.md` §7.10).
- **Hand payload to your node rather than waiting for the recipient.** The
  answer says your node took it, not that it arrived; a recipient may be away
  for days, and that is what the mailbox is for (design §14.1.6).
- **Verify the delegation your node presents, and check that it names the key the
  handshake gave you** (`wire-format.md` §8.2). An instance carries a delegated
  credential rather than its operator's seed (design §23.3), so the key it
  presents is not the keyhash you pinned and the delegation is what joins the
  two. One that names a different key is a replay, and a session built on it is a
  session with whoever replayed it. The same check binds any peer that presents
  an unpinned key on any connection, a node you resolve through or fetch from
  included: its delegation is one you already hold from the topology class or
  the first thing it sends you (`wire-format.md` §9.1), and until one of those
  holds you send it nothing.
- **Cache a delegation you have verified, against the key it names.** It is
  hybrid, it arrives on every handshake, and its window runs to months, so
  checking it per connection pays that cost repeatedly for an answer that cannot
  have changed in between.
- **Check a stapled attestation's delegation the same way** (`wire-format.md`
  §7.1 field 8). The issuer's node signed under a delegated key, and the
  delegation the staple carries is what joins that key to the issuer whose
  material you hold; a staple whose delegation names a different key is
  malformed, not merely stale.
- **A wake endpoint is the user's choice and nobody else's** (design §14.1.5).
  Obtain it from a service they picked, hand it to your node with the key its
  body is encrypted to, and refresh it: an endpoint is stable while valid, not
  permanent.
- **Withdraw it as readily as you registered it.** A registration with no
  endpoint withdraws, and a user turning doorbells off should not have to wait
  for one to lapse.
- **Render the notice locally.** A doorbell carries nothing, so whatever the
  person reads on the lock screen is written by this client from what it finds
  when it reconnects, never from what was posted.
- **Do not treat a doorbell as delivery.** It says there may be something, and
  what there is comes over the network's own channel or not at all.

---

### 4.2 What you keep of your horizon, and what you hand your own other devices

- **Bring another device of your own identity up to date whenever you are in
  contact with it** [author, 2026-10-16]. A device that can see another is
  behind is in contact with it, so it does not wait to be asked and it does not
  ask the person: it hands over what the other lacks — design §10.3's merge
  where the two have forked, a plain append where one is simply short — over
  the archive fetch any reader uses (`wire-format.md` §7.9), or across the
  local interface where the two are side by side (`wire-format.md` §14.3.3).
  **The subject is reading its own archive**, so no new permission and no new
  object arises. What remains for a screen is the residual alone: what one
  device signed while the other was away (design §23.3).

- **Hand over everything you hold of it, not the archive alone** [author,
  2026-10-16]. The other device is brought to your whole state: the archive,
  the view you keep beside it — your contacts and the aliases you have fetched
  for them (§4.3), the locators you hold, the profiles you publish (§4.4),
  your portion of the resource catalogue —
  and your sealed captures, which cross as the ciphertext they are. **The identity seed does not cross**:
  a new signing device comes to hold the identity because its holder put the
  seed there (§2), never because another device sent it (design §23.3).
  Assume the
  other device is the one that will be powered up next and yours the one that
  will be away, because that is the ordinary multi-device shape (design
  §23.3) — anything you keep back is something its holder will find missing
  with no way to ask for it.

- **Persist enough to resolve every node in your trust horizon directly**
  (design §15.1.1), for routing around a patron that is not answering and for
  calculating trust distance. The first is what settles it: the party you would
  ask for the shape of your neighbourhood is the party that is down.
- **Your patron's propagated version is the system of record.** What you hold
  is a copy of what your patron infrastructure gave you; where the two differ,
  the propagated version wins and yours is rewritten. Never present yours as
  authority, to a user or to a counterparty.
- **Materialise it, so a wake does not replay your transaction history**
  (design §15.1.1). Store the derived shape as derived, with a watermark saying
  which transactions produced it, and on waking fold in only what arrived
  since.
- **Evaluate a transaction you are no party to, and keep what the evaluation
  came to rather than the act** [author, 2026-09-23], as a node does
  (`infra-client-requirements.md` §4.3): the shape, the parties you can now
  reach, what a patron's determination moved. The transaction is the archive's
  of everyone party to it, and not yours to retain.
- **Discard the copy whole when its watermark cannot account for what you
  hold.** A cache that cannot prove it is current is one you have to earn
  again, and keeping a partial one to save the work is how a client comes to
  route on a relationship that ended. **Earning it again is repair, not a
  replay**: with the acts gone there is nothing to fold, so what you hold
  comes from your serving node's propagation and from the archives of the
  parties themselves (`wire-format.md` §7.9).
- **Keep it bounded by the horizon** (design §15.1). Nodes leave it, and a copy
  that only ever grew would hold topology about parties you no longer have a
  reason to know anything about.
- **Hold the delegations of the parties in it, as a node does**
  [author, 2026-09-21]. A `Delegation` pushed through your serving node whose
  delegating keyhash falls within your horizon is stored, the newest by
  `not_before` per keyhash and the rest dropped (`wire-format.md` §10.1.1). It
  is what binds a delegated peer on a direct path without waiting for its
  frame, a desktop under an identity you know being the ordinary case, and it
  is current state like the rest of the view, never a history of keys.

- **Candidates may be offered and taken on the ceremony's local interfaces**
  [author, 2026-09-25], where the two parties are in front of each other
  (§1.3, design §12.6.3). A route obtained this way was carried by no patron,
  which is what it is for: it makes reaching a counterparty cost proximity
  rather than a patron's cooperation, and so survives one that is
  uncooperative or eclipsing. Offer it as the disclosure it is — a route
  handed over is an IP the counterparty keeps (design §19.4, P40), whoever
  hands it — and open nothing from candidates that did not come from the
  party they name. **A candidate taken this way opens under the same rule as
  any other** [author, 2026-10-24; the horizon override of 2026-09-25 is
  subsumed]: the counterparty is a contact from the exchange on, and a party
  held as a contact may be dialled (design §12.6.3). One that arrived any
  other way opens only for a party held as a contact. What it discloses is
  design §19.4's P40, and §5's rule covers saying so.
- **Recommended, and not required** [author, 2026-09-25]: when a local
  connection is being established, prefer the transport that exposes the least
  of the address behind it — a link-local duplex channel such as Bluetooth
  first, a shared wireless network second, and the cellular interface last. The
  geography is not what this protects: a counterparty at a ceremony was in the
  room and knows where the user is (design §19.8, C24). **It is the carrier or
  ISP behind the address**, which physical presence does not imply and which
  travels with the user. Stated as a recommendation because the available
  transports are the platform's and the ranking cannot be made a conformance
  condition; an implementation with only one of them is not in breach.

### 4.3 Contacts

**[author, 2026-10-11]** A client keeps **contacts**: for every party it
has learned a locator for, the most recent `SignedLocator` it holds for
them and the key material that verifies it — the two halves of an
introduction (`wire-format.md` §2.3) — with when it last dealt with
them. This is the "local table" design §12.3 Case 1 has you look an
anchor up in, and it is what you hand onward in a bundle (§1.1,
`wire-format.md` §5.4).

- **Hold where a node answers, beside the locator.** A contact for a node
  keeps that node's `EndpointRecord` (`wire-format.md` §7.6) and its
  `AnchorEntry` where one came, the signed records and not a copy of what
  they say. A locator names an anchor and a path and never an address, so
  this is what makes design §12.3 Case 1's "address to start from"
  available at all — and your horizon is not the place for it, since a
  horizon holds endpoints for its own neighbourhood and refuses a record
  for a node further off, which is its purpose rather than a limitation.
  Order two records of one node by seqno as you order locators.
- **Take an introduction, and take it with its anchor.** A person must be
  able to bring in a party they have never met, from a code or a referral:
  that is an `AnchoredIntroduction` (`wire-format.md` §2.4) — the
  introduction with the anchor its locator names, that anchor's key
  material, and where it answers — and a bare introduction or a bare
  endpoint record is taken too, the kind decided by parsing rather than by
  asking the person what they were handed. **Verify before keeping any of
  it**: the material against the hash it must equal, each record against
  the material it must be signed by.
- **Offer a code of your own.** It is the same object about yourself: your
  introduction, the anchor of your own locator, and the record of where
  that anchor answers. Offer none where you hold neither a position nor
  that record, because a code nobody could act on is worse than no code.
- **Capture a locator from every party that becomes visible, until the
  store is full.** A counterparty's, from its intent; every party a
  bundle introduces, whether or not you selected it; every nominee that
  answered, attesting or declining; every verifier that answered; the
  infrastructure you went through to reach any of them — **including every
  node a resolution walk went through**, the node that referred and the
  serving node the terminal answer named (`wire-format.md` §7.7), which
  are the upstreams the pruning below needs and which nothing else shows
  you; and a candidate proposed in fishing and declined. **A referral's
  addresses are the referrer's assertion**: keep them as that, apart from
  a record the node signed, and never hand them on in a card
  (`wire-format.md` §2.4). **A hop whose referral carried no key material
  is kept by keyhash alone** [author, 2026-10-14] — a name and a place to
  try, never verified by, never dialled bound (`wire-format.md` §9.1) and
  never introduced. **The session does not supply what the referral
  omitted**: reaching the hop presents its classical key, where key
  material is the classical and post-quantum pair whose whole encoding the
  keyhash is taken over (`wire-format.md` §2.2), and one member cannot be
  had from the other. `wire-format.md` §7.7.3 leaves that field optional
  for its own reason, so this is the state to hold rather than a gap to
  close. Capturing is cheap, each is a party
  you could later be asked about or need to reach, and a store that holds
  only the parties a ceremony happened to need is one that cannot route
  the next ceremony's witness.
- **Replace by the subject's own word first, then by sequence.** A
  locator the subject handed you itself (its intent, its own message)
  replaces whatever you held; one handed by a third party replaces only a
  held locator of the same series with a lower counter, since series do
  not rank against each other (`wire-format.md` §2.3).
- **Bound the store, and prune toward what still resolves** [author,
  2026-10-11]. The storage limit is yours. As it is approached: rank the
  contacts you have had least to do with — some combination of when you
  last dealt with each and how often you have — and for each, read its
  path for the infrastructure above it that you also hold; **remove the
  contacts that remain discoverable from an upstream you keep**, since
  their locator can be resolved again from there (design §12.3 Case 1);
  then loosen the rank threshold and go again until the space you wanted
  is free. What survives longest is what resolves — anchors and the
  infrastructure beneath them that your remaining contacts sit under — and
  a contact with no upstream among your holdings goes last, because
  pruning it is losing it: that party you will need introduced again,
  which is the ordinary way of meeting anybody (design §12.3 Case 0).
- **Take a cold call** [author, 2026-10-30]: anyone holding your locator can
  write to you first, and their first message carries their introduction
  (`wire-format.md` §7.10.1). Take it where the material hashes to the
  keyhash the channel names — it is what a scanned code carries, presented
  by its own holder — and hold them as a contact introduced, so the
  message that follows is attributable. **Whether to accept what they then
  say is your policy, and the default is yes**; a query the subject of a
  ceremony consented to is accepted whatever that policy says of other cold
  messages, since the consent names the one asking and the subject is a
  prior counterparty of yours. Design §12.4's rule stands: there is no cold
  *lookup* — the locator had to reach them some other way.
- **Show a contact by the alias and image it published for the subnet its
  locator is in, and by keyhash where none has reached you** [author,
  2026-10-15]. A locator names a subnet (`wire-format.md` §2.3), and a
  profile is published per subnet and hosted where the locator leads
  (`wire-format.md` §7.11), so holding a locator is what lets you fetch one:
  fetch it when you take the locator and again when the party's counter
  moves, keep it beside the contact, and never show a name you have not
  received. The alias is display only: nothing you do is keyed by it, and
  two contacts may carry the same one.
- **Persist contacts as you persist records.** They are state, not a
  cache: a client that forgot its contacts between starts could not name
  a witness, select a verifier it had met, or introduce anyone, and
  would behave differently depending on what this session happened to
  do first. The same rule covers everything a ceremony leaves behind: a
  ceremony ends with the live state as it was, plus the contacts it
  taught you, the record, and the meeting's outcome, all persisted — and
  nothing of the ceremony in a live variable that a later one could read.

### 4.4 Your profile

- **Publish an alias and, if you choose, an image, for each subnet you hold a
  position in** (`wire-format.md` §7.11; design §12.8) — to your serving node
  in that subnet, as you publish your prekeys (§4.1), at attach and whenever
  it changes, each one at a higher counter than the last. Both are the
  person's to pick, and neither is expected to be anything in particular: a
  picture of the face is not asked for, and how much to show is the person's
  own decision, made separately for each subnet, as the postal model has
  each institution holding its own name for you (design §3.1.1).
- **It goes in no transaction.** A profile is a statement about now, replaced
  when the person changes their mind; a record is forever. Nothing you sign
  into an archive carries an alias or an image, and a counterparty that
  wants your name fetches it.
- **Where you are the root, publish the subnet's profile too**: its alias
  and, if you choose, a logo, as the subnet's own (`wire-format.md` §7.11
  kind 1). **By convention the root alone sets or changes it.** Nothing
  enforces that, since any party can sign a profile naming the subnet
  (design §1.1), and the client says so plainly rather than pretending the
  convention is a rule; where two reach you, prefer the one signed by the
  root, whose key you hold.

---

## 5. Path selection

- **Direct is preferred and the relay is the fallback, and neither is a user
  choice** [author, 2026-09-27]. The direct path opens between contacts
  where one can (design §12.6.3) — dial any party you hold a contact for, and
  open the path to one that holds you — and payload falls back to the relay
  where none is held; the client offers no override and raises no
  per-connection warning. The trade each path carries — an IP to a contact,
  a communication graph to the serving node — is recorded where trades are
  (design §19.4, P17), not put to the person.

- **The relayed path's first term is the recipient's own node** [author,
  2026-10-05]. Where no direct path is held, hand the payload to the node that
  serves the recipient — dialled at an address you hold, or one resolved for it —
  because that is the party which queues for its own client while the client is
  away and gives it over on return (design §§12.6.3, 14.1.4). **A light client
  does not need its patron or infra node to address a distant node for which it
  holds a locator**; your own serving node is the fallback, for a client that
  cannot open an outbound connection at all and needs it to traverse a proxy. A
  node that already serves you is reached on the session that is up rather than
  dialled a second time.
- **Bound what you keep open to nodes that do not serve you, and close an idle
  one.** An open connection to a stranger's node is a standing statement about
  who you are talking to (design §19.4), and the bound is yours to choose and
  state, like every other cache here.

---

## 6. Warnings before irreversible or surprising actions

- **Say that a rotation restores standing and not history** [author,
  2026-10-16]. A presence-anchored key rotation returns the key; the archive comes
  from the person's own backup or not at all, because a protocol path that
  restored it would hand the same history to whoever completes the same
  rotation (design §10.2, §22). So a client that has rotated an identity
  says what came back and what did not, and a restored archive is shown as
  restored on trust rather than as verified-complete (PRD-09).

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

- **Before an operator commits an instance to a provider and zone, order the
  choices by how concentrated they already are inside that operator's own
  horizon, least concentrated first, and mark the crowded ones rather than
  hiding them.** The peering record carries each endpoint's ASN and the zone
  that provider calls the place (design §3.4), and **concentration is keyed by
  the pair**: the ASN disperses the company, the zone disperses the place, and
  a zone label is a provider's own word and means nothing apart from it. What
  an observer may use is its horizon plus the far endpoints of the peering
  records it can see (design §16.2.1) — 221 nodes at f = 10 (design
  §15.1). Design §3.4 already asks the reference client to prefer peers
  differing in ASN and region and to surface it when they do not; this is that
  preference one screen earlier, while the choice is still free, and design
  §18.1 is what turns on it. Dana's patron and four of her nine siblings
  already run inside one provider's ASN, three of them in the same zone of it:
  that provider sorts last, that zone sorts last within it, both are marked,
  and Dana may still choose either.
- **Say what that ordering does not cover.** It speaks to one operator's own
  redundancy and not to the network's, because visibility does not compose: a
  provider uncrowded inside a horizon may be crowded one edge out, and design
  §16.3.1's bound is why an observer may not go looking.

---

## 7. Cycle handling

- **The kind is the inviter's, and the side reading the code carries no
  direction of its own** [author, 2026-11-03]. Two participants cannot propose
  authority in opposite directions within one meeting: the invitation states
  what the meeting is, and the other person accepts or refuses it at the brief
  (§1.5). So there is no opposition to resolve here, nothing deferred past the
  record, and no question to ask. **A choice offered where it cannot act is
  worse than none** — a client that lets the reading side pick a direction
  will discard it wherever the invitation asked for no adoption, and the
  person is told nothing.
- **The mutual adoption that does arise is built across two meetings**, each
  party adopting the other in turn, and that is a cycle in the topology rather
  than a collision of intentions. It is detected from local state — whether the
  proposed patron sits in one's own down-line — and handled as design §6.2.5
  requires, on positive knowledge only.
- **Check an adoption body before you sign it.** The patron builds the body and
  you are shown it, and design §6.1.1 has an adoption carry both signatures — so
  what your signature says is that *this* patron, *this* series and *this*
  address are yours to accept. Refuse a body that names another party as the
  node; one whose evidence is not a presence record you hold between the two of
  you; an address that is not one hop below the patron's own position, in the
  patron's own anchor, as the `SignedLocator` their intent carried states it
  (`wire-format.md` §§2.1, 2.3, 14.3.2); a series you have occupied or a counter
  that is not zero (§2 above; `wire-format.md` §2.3); a node back-pointer list
  that is not your archive's own next (`wire-format.md` §3.1); and a timestamp
  before the meeting's effective time, or further ahead of your clock than you
  tolerate. **Every one of these you can check**, which is §1.1's test answered:
  the patron's claim about where it sits came from the patron, signed, and
  everything else is in your own archive.
- **Say which series you can enter, because your patron cannot know.** A series
  is yours never to reuse (§2) and your patron holds no record of the ones you
  have occupied, so an adoption it proposes into one of them is a body you must
  refuse. Offer a free series rather than making them guess.
- **A refusal here is an answer and not a fault.** Tell the patron why in words
  it can show its own person, and leave the meeting alone: the presence record
  the adoption rested on is complete without it, and either side may propose
  again.
- **Ask for a peering at the invitation, as a choice beside the plain
  meeting, and never as the adoption's alternative** [author, 2026-10-19]. A
  peering joins two parties where neither is over the other (design §6.3), so
  there is no direction for the two to settle and nothing to ask beyond
  whether to make one — which makes it a box to tick on the regular meeting
  rather than a question of its own. **The choice is the inviter's and the
  other person reads it**: it travels with the kind, the brief states it
  before anything is signed (§1.5), and the peering itself is made after the
  record it rests on. A meeting carries a peering, an adoption, both or
  neither. **What the box must say is what it does**: ticking it makes a
  transaction, and the backup either party may then hold is what the peering
  entitles them to (design §3.4) rather than a thing the box arranges by
  itself. **Somebody has to build the body**,
  since it carries one timestamp, and nothing in the protocol makes either
  side the builder — so order the pair as the record itself does
  (`wire-format.md` §4.4, by keyhash) and let the lower build, which needs no
  role and no new agreement. The other side sends what its archive chains to,
  which is the one thing the builder cannot work out for itself. **Say nothing
  of a replication commitment here**: §4.4's field is optional and what it
  promises is bytes held for a peer (design §3.4), which is an operator's
  offer and not this meeting's.
- **A rotation adoption and a transfer are assembled by their own subjects**
  (design §9.0, §6.2), who hold the material that makes them. A body carrying
  somebody else's `Recovery` block or transfer statement is not one you were shown
  to sign.
- **Ask which subnet before you invite, and say which subnet before you
  accept** [author, 2026-10-15]. A patron holding positions in several subnets
  adopts into one of them (design §3.1.1), so an invitation to be adopted asks
  the inviter which, by the label the client gives each: *the patron's alias
  in that subnet, and the subnet's own* — *Dana's Bowling League*. The party
  invited is shown that label in the brief it accepts or refuses, from the
  claim the invitation carried until the signed profile replaces it
  (`wire-format.md` §7.11), and a regular meeting shows none.
- **Ask which scope to submit as, before you are adopted** [author,
  2026-10-15]. A party rooting a scope may bring it under the patron, every
  position in it coming to show the patron's subnet, or join as an
  individual with its scopes standing as they were (design §13.8); the
  choice is the party's, made before the body is proposed, carried to the
  patron as its claim (`wire-format.md` §4.1 field 10), and checked against
  what it holds before it signs: a body naming a scope it does not root is
  refused. Where it roots none there is nothing to ask. **Two screens, one
  each way**: *adopt into which scope?* for the would-be patron and *submit
  as which scope?* for the would-be subordinate, shown before the invitation
  is offered or, for the party that read it, before the acceptance.

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
- **For a relative rank predicate (a percentile, a median, any quantile), show the
  population it is a fraction of, and say that the line moves when the org does.**
  These are the predicates whose *line* moves when others join — absolute-rank
  predicates depend on others too (`resource-requirements.md`
  §7.2.1), but their cutoffs do not shift with the population — and an operator reading
  *"top 20%"* has no reason to expect that admitting members at the bottom promotes
  someone at the cutoff. An absolute *top-k* does not behave this way, and the
  client should offer it as the plainer alternative when the operator's intent looks
  absolute.
- **The network can stop new establishment; it cannot reach into a session already
  running on someone else's terms** (design §11.2). **Say nothing of this to the
  member** [author, 2026-11-05]: ending a session protects the resource from the
  person, not the person from anything, so a member who keeps access they were not
  cut off from has lost nothing and has nothing to act on. What the limit bears on
  is an owner's belief that revoking a membership was complete, and that belongs
  where an owner reads it (`infra-client-requirements.md` §10.5, design §11.2) and
  not on the member's first use.
- **Confirm a brokered service matches the signed `CatalogEntry`** before routing
  traffic to it, and show the user which owner published it (design §11.5). That
  binding is the whole of what the network offers about a gateway; beyond it the
  user's remedy is to avoid the resource.
- **Show which roles the user holds on each resource** they can reach. Without it,
  a user cannot distinguish *denied by policy* from *broken*, and will not connect
  losing access to having departed a patron (design §11.5).
- **Do not present resources the user cannot use.** The serving node filters its
  catalog page by what the viewer holds; the client should not re-expand it.
- **Access is gated by current membership in the resource owner's trust horizon**
  (design §11.2), so it changes without any action by the user, on joining,
  departing, or crossing a tenure boundary. The departure warning in §6 above is
  the case worth warning about, since the user is acting deliberately and the
  consequence is elsewhere.

**What a resource answered, and what may be shown of it.** `wire-format.md` §11
delivers the application's reply as one HTTP/1.1 message, so a client that means
to show a resource's page has to read one.

- **Read it once, below whatever boundary your interface layer talks across, and
  reject rather than normalise anything ambiguous.** `wire-format.md` §11.2 names the hazard the
  node's own parse exists to close — two parties disagreeing about where one
  message ends and the next begins — and that hazard is about a boundary, not a
  direction: an interface layer parsing what the kernel handed it raw is the
  second parser it forbids on the node's side of the same exchange. That
  list reads the same from this end: a line not terminated by CRLF, a folded
  header, a duplicated length, a length beside a transfer-encoding, an encoding
  other than chunked, bytes after the declared body. **A node that sent you one
  of those did not do its half** — `wire-format.md` §11 has it parse and
  re-serialise the response
  too — so name the refusal rather than guess, and show none of the message
  rather than some of it.
- **The declared media type decides what is rendered, and nothing else does.**
  Render the types you have decided to render; name every other one to the user
  with its declared type and its size; treat an absent declaration as declaring
  nothing. **Never inspect a body to find out what it is.** Sniffing delivers a
  page as something the user did not agree to look at, and *could not display*
  tells them nothing where *this is a PDF of 40 kB* tells them what happened.
- **Frame a resource's page so that it reaches nothing of the user's**: no
  script, no bridge into the client, no file or content access, no storage
  outliving the screen, no fetch, no navigation, and no origin — the reply came
  through the kernel and has none, so the page is same-origin with nothing. This
  is `infra-client-requirements.md` §8.3's argument for framing a node's own
  pages, one step further: a resource is an application its owner packaged,
  hosted by a node that owner chose, and **the party whose isolation it is cannot
  be the party asserting it**. The cost to an application is nil — its every
  exchange goes through the kernel in any case, so a form is carried by the
  client whether or not a page could run code.
- **Style beneath the page rather than over it, and do not rewrite its markup.**
  What the resource has not styled for itself the client may style to match its
  own surface, with rules the page's own override. Editing somebody else's
  document to make it safe would assert that the client understood the document;
  the frame's refusals do not depend on understanding it at all.

### 8.1 What a resource may ask of the client

**A page that reaches nothing cannot take anything, so what needs the
device is asked for** [author, 2026-10-30: wanted, for the utilities named
below]. A scan, a photograph with its provenance, a place, a tag read, a
signature over a document: each needs a capability of the device under the
person's consent, and the frame above has no bridge a page could reach one
through. **The resource asks instead, in one typed ask; the kernel reads
it, puts it to the person, and composes the answer itself.** Nothing of
what a page may do changes, and nothing the resource layer may see widens:
what goes back is user-submitted application data, exactly as a form the
client carried would be.

- **An ask is a resource's answer declared `application/vnd.rhtn.utility+json`**,
  and only that declaration makes it one — the rule above holds, and a
  body under any other type or none is never inspected to find out whether
  it might be a question. The body is one JSON object: `utility`, one of
  the names in the table; `ask`, what it is for, in the resource's words;
  `retention`, how long the resource keeps what it gets, in its words;
  `return`, a rooted path on the same resource the answer goes to; `token`,
  the resource's own name for the ask, `[A-Za-z0-9._-]` and at most 64
  bytes, echoed back; and what the utility needs — `payload` as hex for a
  signature, `precision` for a place, `media` for a scan, a capture or an
  import, `channel` for a tag or a check-in, `seconds` for a live session.
  **Read once, below the boundary, and refused rather than guessed at**:
  the ask is bounded at 16 KiB, a sentence shown to a person at 280 bytes,
  and a field absent, ill-formed or out of bounds is the whole ask refused
  by that field's name — a question half-read is a question half-put. An
  ask the kernel could not read is shown to the person as unreadable and
  **nothing is sent back**.

| Utility | What the person is asked to give |
|---|---|
| `scan` | a document, scanned with the camera, as a PDF or an image |
| `capture` | a photograph or a video taken now, with the time it was taken and a claim that this device took it |
| `location` | a place, as a geohash at the precision asked, sent once |
| `check-in` | presence at a place, read from a tag on a proximity channel |
| `tag` | a tag read: the code on a thing |
| `sign` | a signature over the exact payload the ask carried, authorised by the person — the platform's own unlock where it has one |
| `live` | a live session: capture pushed to a service the resource names, for the seconds asked, with its own consent, duration and indicator |
| `notify` | standing permission to be shown that resource's notifications |
| `export` | a copy of what the resource holds, handed to the person as a file |
| `import` | a file the person picks, handed to the resource |

- **Put to the person whole.** The purpose and the retention are shown in
  the resource's words and marked as the resource's: §1.1's test applies —
  the client shares no state with the resource and cannot hold it to
  either — so they are shown as what it said, never as what the client
  promises. Every result is the person's act at the moment of asking:
  **no utility reads a sensor ambiently**, a place is never bundled into
  another action and never accumulates into a track, and the client keeps
  no location history for any resource to ask after. **What would be sent
  is shown before it is sent** — the photograph taken, the place as the
  cell it would go in, the exact bytes to be signed — and a capability
  this device has not got, or has not been allowed, is a refusal carrying
  that as its reason rather than a silence. A live session is the
  one stated exception to *results, not streams*, and is a distinct ask
  with its own display rather than a quiet relaxation of the rule.
- **Declining is an answer the resource sees**, with a reason — the
  person said no, or this device has no such utility — never a silence it
  has to interpret or an error loop. An ask is spent by its answer, whatever
  the answer; the kernel holds at most eight unanswered, the oldest let go,
  and nothing of one outlives the process.
- **The kernel composes the answer**: one `POST` to the ask's return path,
  on the resource that asked and through the node that served it, as
  `multipart/form-data` — the parts `resource-requirements.md` §3.4 lists,
  each once. The application supplies the capture the person chose to give
  and nothing else; a result is checked against the utility's shape (a
  geohash at the precision asked, bytes under a declared type, text, or an
  approval) and against `wire-format.md` §11's message bound of 16 MiB —
  a request larger than one frame travels as a sequence of them — so a
  capture larger than that is the client's to shrink before it is
  offered.
- **The audience is the resource that asked**, never one a resource names.
  Where the utility signs — a capture's provenance, a signature — the
  answer carries a claim under **the application key for that one
  audience**: an Ed25519 key expanded from a secret the device mints at
  first need and the resource's keyhash, so each resource sees a key of its
  own and no two can be matched. **It is never the network identity**, and
  the identity's keys sign nothing here — publishing that key to a
  resource would undo the resource layer's pairwise separation. The secret
  is kept with the device's own state (§9) and **never in the backup**
  [author, 2026-10-31]: a device restored from its phrase signs under a
  new key, which is the rotation policy — an application key is not the
  identity and comes back with nothing. Deriving it from the identity root
  instead would bring it back with the phrase and tie every claim a
  resource holds to the one recoverable secret, which is the opposite of
  what a key per audience is for. **The media type above and this array
  are fixed** [author, 2026-10-31], being what a resource is written
  against: the claim is a signature over the CBOR array `["rhtn/utility@1",
  audience, utility, token, digest, at]` — the resource's keyhash, the
  utility's name, the ask's token, SHA-256 of the result (of the payload,
  for a signature) and the time in milliseconds — so a claim given to one
  resource says nothing shown to another.
- **Three things a signature must not conflate**: a *local unlock*, by
  which the platform vouches the holder to the device; an *application
  signature*, by which the device vouches a payload to a stated audience;
  and a *ceremony claim*, the presence machinery of design §7, untouched by
  any of this. `sign` is the second, authorised by the first where the
  device offers one, and is never the third — the key says so.
- **What crosses the interface** (§9): the ask crosses outward as the
  question, the person's capture crosses inward as their act, and the
  answer is the kernel's. The application never composes, signs or
  addresses it.

---

## 9. The interface between the kernel and the application

design §14.1.0 puts identity, keys, the archive, the topology table, sessions
and every byte on the wire inside a kernel, and makes what a person looks at a
client of it. What that interface offers is stated here, since none of it is
on the wire.

- **Every session is the kernel's.** The application does not dial, does not
  hold a connection and never sees a frame. Attaching, publishing, stocking,
  sweeping, relaying and registering a wake endpoint all happen inside, and
  what comes back out is the result, not the exchange.
- **What crosses outward is ordinarily what to draw**: a verdict, a distance, a
  decrypted message, a mode. Returning bytes for the application to route as a
  matter of course would be the second parser design §14.1.0 refuses.
- **A cryptographic payload may cross where the kernel must pass one through**
  (design §14.1.0), at a named interaction and not as a general channel. The
  ceremony's optical exchange is the case that arises here: the screen and the
  camera are the transport, so the kernel hands over what to display and takes
  back what was read. **The application relays it and does not read it** —
  which is what keeps the two layers separable, and is the whole of the
  obligation.
- **Payload is decrypted before it crosses**, and ciphertext never does. The
  application receives what was written and the party the material attributes
  it to.
- **The hardware crosses inward** (design §14.1.0, §1.3 above): the camera, the
  clock, the random source and the person. The kernel
  asks; the application answers; the application does not answer on the
  person's behalf. Consent, in particular, is what they said and never what the
  application assumed.
- **A wake endpoint comes from outside and goes straight through.** The
  application obtains it from the service the user picked (§4.1) and hands it
  in; the kernel holds no relationship with that service and obtains nothing
  itself.
- **The kernel seals what it persists; the application keeps the key**
  [author, 2026-09-26]. What is persisted is the archive, the payload
  sessions and the ratchet state — **key material among it** — so it is
  encrypted before it reaches the platform's storage, in the kernel, under a
  storage key the kernel mints. The kernel stays platform-agnostic
  (design §14.1.0) through the key's **provenance**: it never chooses where
  the key lives, it calls a named source the application supplies, and
  providing that source from platform-local affordances is the application's
  whole obligation — a platform key store wrapping it on a phone,
  hardware-backed where the device has one, so the wrapping key never enters
  the process; a key derived from a passphrase where a person is present at
  start; the operating environment's own
  convention where the platform declares itself unsealed. **Unsealed is a
  declared configuration, never a fallback**: a source that cannot keep the
  key refuses the start rather than quietly writing plain. The storage key
  sits in the kernel's working memory while it runs, beside the session keys
  it protects, and is wiped like them (§3); what custody buys is the state at
  rest. Sealed state that fails to open refuses the start — beginning fresh
  over unreadable state would mint new material and silently lose every
  message queued for the old.
- **And seals for the application what the application keeps** [author,
  2026-11-14]. The kernel offers the application, and nothing else, a
  service that encrypts and decrypts bytes it is handed under keys that
  never leave the kernel, so the application defines its own files and
  conventions and they are sealed at rest all the same. The service is
  general; the application's conversation store is the first use of it,
  and its shape is the one to follow: each message's ciphertext appended
  to a file as it accumulates rather than the history sealed and unsealed
  whole on every change. What is in memory need not be sealed. The
  reference client reads that file lazily: a thread is decrypted only when
  it is opened, and only its most recent messages unless the person scrolls
  further back, which keeps the least cleartext in memory and spares the
  decryption of every history at start.
- **A resource's ask crosses outward as a question, and its answer is the
  kernel's** (§8.1). The kernel reads the ask and hands up what was asked,
  in whose words; the application shows it, and hands in the capture the
  person chose to give, or their refusal; the kernel checks it, signs it
  where the utility signs, composes the one request and sends it. The
  application composes nothing of the answer and holds no key it is
  signed under — the shell keeps the sensor, the kernel keeps the policy,
  which is the ceremony's own shape.
- **Refusals are values that carry their reason.** A bad length, an absent
  session, an endpoint the node would not take: each is an answer with words in
  it rather than a silence the application has to interpret, which is design
  §1.1's posture at this boundary.

### 9.1 The nouns the kernel offers

**What crosses outward is one vocabulary** [author, 2026-10-29]: the kernel
holds each of the following as one record, the application reads it as that
record and shows what it chooses of it, and a second client — a desktop, an
operator's console — reads the same. **Identities cross whole**: a keyhash is
its thirty-two bytes, never a prefix of them; what is shortened for a screen
is shortened by the screen. **Kinds cross as the wire numbers them**: how a
contact came to be held, why a resource would be lost, what bounds a cost,
each is the enumeration and not a sentence about it. Nothing in a record is
a display decision.

| Noun | What it is | What it carries |
|---|---|---|
| **Contact** | A party this client holds (§4.3) | Its keyhash; the alias it published for the subnet its locator names, or none, since a name not received is never shown; how it came to be held — met, introduced, nominated, queried, serving, declined, provisioned; whether a locator of it is held; whether an address of it is held; how many times it has been met or heard from; when last |
| **Subnet** | A subnet this client holds a position in (design §13.8) | Its id; its alias where its profile is held; this client's own alias there, where it published one; its rank there — the root, an operator, a patron and of how many direct lines, or none of these; where this client sits — the anchor, the path and how much of it is this client's own |
| **Line** | A patron relationship this key holds (§6), as the subordinate holds it | The patron; the series it runs on; the anchor and depth it puts this key at |
| **Instance** | An infra node whose endpoint record this client holds (§4; §10 where this operator runs it) | Its keyhash; the addresses it answers at |
| **Resource** | An entry as a catalog served it (§8; `wire-format.md` §6.4) | The node that served the entry, which is the node asked for the resource; the resource; its owner; its service type and instance name; its endpoint; its data practice where stated; whether the portion it came in was truncated, and whether it was stale |
| **Profile** | What a party or a subnet published about itself (§4.4; `wire-format.md` §7.11) | The subject; the subnet; whether it is the subject's own or the subnet's; the alias; the image where one is carried; when it was published |
| **Cost** | What ending a line costs (§6), computed before the act and never retained | What goes, each with why — the node serving it leaves the horizon, or its owner does; how many stay; who leaves the horizon; and every reason the list is a floor rather than a total — a predicate no client holds, a node whose portion is stale, one whose portion was truncated, one never swept, several lines to the one patron, this key's own adoption not yet in its table |
| **Connection** | Where each of this client's addresses stands with the node serving it (§4; one per subnet, §2) | Detached; attached, to whom, and whether as a member of that node's subtree; reconnecting, from whom; lost, from whom and why |
| **Meeting** | A presence ceremony this client took part in, as its outcome (design §7.1, §8.1) | The presence record; the counterparty; when it finalised; every witness that signed it; the adoption or the peering that rested on it, where this client holds one. **The ceremony's own state is not a noun**: the step, the optical exchange, the window and the nominees' answers are the client's in-flight state, bound to a camera and a window of minutes, and nothing of them outlives the process or should |

**And the kernel says which of these changed.** After each exchange it names
the nouns it altered, once each, so an application keeping a copy refreshes
those and no others, and never learns of a change by polling the status
line. A copy is the application's convenience; the kernel's record is the
one that is true.

---

## 10. The instances you run

- **Administer every instance you run, and assume nothing from running one**
  [author, 2026-10-15]. An instance serves one subnet (design §3.3), so an
  operator bound in several subnets who serves more than one runs an instance
  in each, and this client is where all of them are administered
  (`infra-client-requirements.md` §8.2): a list of instances, each with the
  subnet it serves, its address and the run of delegations it holds. Holding a
  position in a subnet says nothing about running infrastructure there — an
  operator in one subnet is an ordinary light client in the next — so the
  client never infers an instance from a position, never offers to administer
  one it was not told about, and never addresses its own traffic in a subnet
  to an instance it runs in another.
- **Show each instance under the subnet's label** (§4.4, §7), since an
  operator with two instances needs to know which is which by something other
  than an address.
- **Launch an instance from this client, at a provider the operator
  chose, with a credential the operator holds** [author, 2026-10-25].
  The provisioning pages (`infra-client-requirements.md` §8.3) are where
  an instance comes from as well as where it is enrolled: the client mints
  the enrolment token, composes the configuration the instance cannot
  supply for itself, hands both to a fresh machine at the provider as the
  script it runs once, and enrols it at the address that comes back. **No
  broker stands between the client and the provider**: the client speaks
  to the provider's own interface with a credential the operator obtained
  there, and that credential is identity state — carried in design
  §13.7.1's envelope under the identity, beside the archive it protects,
  and never in the archive (§2) — so a replacement device launches as the
  same operator. The operator's recourse for a credential that leaked is
  the provider's own console, where it is cancelled; this client offers no
  other. **The credential is a token the operator makes at the provider's
  own console and pastes** [author, 2026-10-25]: the client opens the
  provider's token page and shows the steps there — what to name it, what
  expiry to give it, which scopes the launch needs and no more — and takes
  the token pasted back; the person signs up or in and works through the
  page at whatever pace, a payment method, a second factor, a confirmation
  mail between. No application is registered with any provider on the
  project's behalf, so the client ships nothing that names the project to a
  provider. A token the operator gave an expiry is replaced the same way
  when it lapses. **The image is pulled from the reference registry unless the
  operator names another**, which is how a custom or third-party node is
  launched by the same pages. Order the providers and zones offered as §6
  says, by concentration inside this operator's own horizon. **And record
  where the machine was put** [author, 2026-11-06]: the provider's ASN and
  that provider's own zone label ride in the endpoint record and the anchor
  entry this client signs at enrolment (`wire-format.md` §4.4, design
  §3.4), because the ordering above is only as good as what records carry
  — an operator ordering by concentration is reading what other operators'
  launches wrote down. A machine enrolled by hand is in a place only its
  operator can state, so the pair is theirs to give or to omit, and a zone
  is never written without its ASN.
- **Install a resource on your own instance from here, not from a shell**
  [author, 2026-11-02]. An operator's only window on their machine may be
  this client, so installing a package is an act on the administration
  surface (`infra-client-requirements.md` §8.2) rather than a command
  somebody types. A package that runs as a service of its own has to be
  *started* by something with control of the machine, and the instance has
  none: so **the instance asks its own host, and the authority arrives
  with the request**. Derive a second ssh key from the signing seed under
  its own label, hand its public half to the machine at launch beside your
  own, and give the private half to the instance **with the request and
  for that act**. **The key it hands over is never the one that opens a
  shell**: the host binds the second key to one program, whose vocabulary
  is what an instance may ask for, so an instance already seized keeps
  something worth little and the key that opens a shell is never on the
  machine at all. Keep neither key: both come back from the phrase
  wherever the identity does.
- **What the install act carries is the operator's choices and the
  package's own declaration** [author, 2026-11-02]. The resource, its
  owner, the authority it answers under, the image and the port are the
  operator's, taken on this screen. **The manifest is the package's, and
  this client relays it**: `infra-client-requirements.md` §9.1 puts the
  capability declaration in the manifest that ships with the package, so
  carry that manifest to the instance rather than letting either of you
  compose one — a node or a client that wrote role names of its own would
  be declaring them on the package's behalf. Show the operator what the
  manifest declares before the act, since what they are admitting is what
  it says. **Then show them that nobody may reach it yet** and offer the
  grant as the next step (`infra-client-requirements.md` §7, §9.4): a
  package's roles start bound to nobody, and an operator who has just
  installed something will otherwise look for why it refuses them.
- **Carry the adopted node's key material in the adoption you compose**
  [author, 2026-11-03]. `wire-format.md` §4.1's field 5 is what it is for, and a
  patron composing one has **just met that node in person**, so it already holds
  their material. Fill it.
  - **Because a record is unverifiable until every signer's identity is known**,
    and this one is signed by the node it introduces. A party given it afterwards
    — a node being taught the subnet it was launched into
    (`infra-client-requirements.md` §4.4) — has **no other way to obtain that
    identity**: the material does not travel in the topology class, and the party
    who could serve it is reached through the node that does not yet work.
    Carried, the record introduces its own subject and verifies anywhere.
  - **It is the adopted node's and never the patron's**, which §4.1 fixes: the
    patron is already known to anyone who holds the record above it.
  - **A subnet whose records were made without it stays unverifiable to a new
    node**, since the records are signed and cannot be amended. That is a reason
    to fill the field from now on and not a reason to invent a second way to
    distribute material.
- **A launch that does not come up says why, as far as it can establish, and
  says where the rest of the truth is** [author, 2026-11-02]. A machine is
  created, an engine installed and an image pulled before any node answers, and
  every one of those can fail on a machine that is otherwise healthy. **A client
  that reports only that the surface did not answer has told its operator
  nothing they can act on** — and for the operator this path is for, whose only
  window is this client, there is nowhere else to look.
  - **Say what the wait is for while it waits.** A first boot installs a
    container engine and pulls the node's image; an operator watching a blank
    screen cannot tell that from a failure.
  - **Open the machine's other door to tell the two apart.** A machine that
    answers ssh and serves no node is a machine that is up and whose **node did
    not start** — which is a different fact from a machine that is not answering
    at all, and calls for a different act. **Stop waiting once that is known**:
    the engine and the pull have had their chance by then.
  - **Name the causes without claiming which it was.** The image could not be
    pulled, the configuration was refused, the container started and exited —
    in the order they happen. A client that asserted one of them would be
    reporting something it did not look at.
  - **Say where the truth is, and that it is reachable.** It is in the machine's
    own log, which wants a shell this client does not have and the operator's
    own derived key does open (§10's key above). Give them the command.
  - **Keep the verdict with the instance**, not only in the line that was on
    screen: an operator who looked away should still find out what became of a
    machine they are paying for, and what is theirs to destroy.
- **Draw the acts the instance declares, and make them yourself** [author,
  2026-11-02]. `infra-client-requirements.md` §8.3 divides the surface: the node's
  page shows what it knows, framed and read-only, and **what may be asked arrives
  as a declaration you draw** — a name, a label, a sentence, and parameters from a
  closed set. Ship no screen of your own for an act: a native screen per node
  version, and per third-party package, is the thing `infra-client-requirements.md`
  §8.3 exists to prevent, and a
  declaration is how the same acts reach you without one.
  - **Read the version first.** Where it is one you do not know, draw what you
    understand and **say plainly that the rest wants a newer client** rather than
    guessing at a field whose meaning may have moved.
  - **Supply what the declaration says you must bring**, and nothing it does not
    ask for. Today that is the restricted host key of §10's bullet above, derived
    for the one act. **Ask the person before you derive it**: an act that reaches
    their machine is one they should be looking at.
  - **The page's control is a bounce, not an act.** A node's page carries a link
    in a scheme of this client's; take it, and treat **every value in it as text
    that fills a field the person confirms** — never as an act to perform. The page
    came from the node, a seized node is planned for (design §18.1, §23.3), and a
    page that could make an act happen could grant itself a role.
  - **Read a refusal back to them.** An act is one proved request and its answer
    is the instance's own words; show them rather than a spinner that stops.
- **Take a package offering from a link or a code** [author, 2026-11-02]. A
  distributor publishes the image, a suggested authority and port, and **the
  package's own manifest** (`infra-client-requirements.md` §9.1, §9.4) as one
  string; register for it at the device's own level so "open with this client" is
  how an operator starts, and read one from a code where there is nothing to tap.
  - **It is untrusted content and it authorises nothing.** Every field fills a
    screen the operator confirms. The resource identity you mint, the owner is
    their identity, the instance is their choice, and the key is yours to derive —
    **none of those is in the string**, and one that names them is read without
    them.
  - **Show what the package declares before they agree**, which is what they are
    admitting: §9.1's supply chain is trusting the package author's judgment about
    access, and `resource-requirements.md` §7.3 has that author's incentive run
    toward breadth. A name in the offering is a label its distributor chose and
    nothing you have checked — present it as one.
  - **Bound every field on the way in**, and refuse an offering longer than you
    will read. A label that is not one line, an image reference that reads as a
    flag, a manifest that is not text: each is refused where the person is
    standing rather than carried to the instance to be refused there.
- **Teach the node you attach to what you hold** [author, 2026-11-01].
  Attaching is two replays, not one: the node replays its store to you
  (`wire-format.md` §10.1.3), and **you replay the records you were party
  to** on the same edge, as the topology pushes `wire-format.md` §10.1
  defines. A node is
  not always older than the subnet it serves — an operator launches one
  into a subnet that already exists, moves hosting, or replaces a machine
  — and a node that has never heard of a member can resolve no position
  for them and materialise no role row, so the subnet would be invisible
  on its own infrastructure. **Your own acts and nothing else**: what you
  keep of a transaction you were no party to is that you took it and the
  shape it produced (`infra-client-requirements.md` §4.3), and the party
  that was there replays the body when it attaches. Replay at every
  attach, including a failover to a sibling: what the node drops as
  outside its store is offered again next time, by which point it may be
  inside.
- **Install a way in to every machine you launch, derived from the
  signing secret** [author, 2026-10-31]. An operator needs a shell on a
  machine they rent — installing a package is theirs to do
  (`infra-client-requirements.md` §9), and so is taking the machine apart
  — and a key the provider holds or a device keeps is a key that dies with
  that device. So the client derives an Ed25519 **ssh** key from the
  identity's own signing seed under one fixed label, hands the public line
  to the machine in the script it runs once, and keeps nothing: **the
  phrase that restores the identity restores control of the machines it
  launched**, which is the only restore story a rented node can have
  once the launching device is gone. **It is one key for this identity,
  not one per machine**, since the machines are already named together by
  their endpoint records and a key per machine would want a label the
  restore path has to carry. **It is not the identity's key and signs
  nothing for it**: a derived key cannot be walked back to the seed, it is
  no part of the protocol, and ordinary tools read it as the ordinary ssh
  key it is. **Show it to the operator rather than storing it**: the
  public line is on the machines already, and the private half is derived
  again wherever the identity is, so a copy is never the only one and a
  screen that shows it says what holding it means.
- **Renew a run before it lapses, from where it ends** [author,
  2026-10-16]. An instance holds 45 credentials of 48 hours and signs none
  of them: it tells its operator before the last one
  (`infra-client-requirements.md` §7) and **this client is the only thing
  that can answer**, since the key that signs a credential stays on the
  ceremony device (design §23.3). The next run opens exactly where the
  held run ends, which the instance reports, so the run stays contiguous
  (`wire-format.md` §8.2) and nothing guesses from a clock; the proof over
  the instance's transport key is checked again first, a renewal being no
  more an occasion to sign for a stranger's key than an enrolment was; and
  neither record of `infra-client-requirements.md` §4.4 is re-signed, a
  renewal being about the credential alone.

---
