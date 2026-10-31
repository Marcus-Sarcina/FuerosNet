# Decisions the author has explicitly deferred

**This file holds only what he has deferred on purpose** [author,
2026-10-16]. A decision that is his gets asked in the output when it
arises — at the moment if it blocks the current task, in the same output
if it does not — and asked again in every output until he answers.
Nothing arrives here because it was inconvenient to ask; the rule is in
`CLAUDE.md`.

Each entry is written to be decided **cold**, months later:

- **What prompted it** — the work that ran into it.
- **The options**, as narrowly as they can be put.
- **What was done meanwhile**, so nothing is blocked on the answer.
- **What changes if it goes the other way** — the cost of having waited.
- **When it arose**, and when it was deferred.

---

## The presence figures of design §7.5, under the four-minute window

**Deferred** [author, 2026-10-24]: "there's no sense trying to tune the
presence thresholds until we move to the 4 minute timer, as what feels
reasonable from a user standpoint will undoubtedly shift at that point."

- **What prompted it.** A6 on the bench (2026-10-24): the capture loses
  1.6–3.7 s of a 15 s window to the camera's first frame while the
  detector samples frames that do not exist yet; the NFC latch is 4 s; a
  user has to move from tap position to filming position inside those
  windows; the continuity check is off on the field-test build
  (`device.rs`, window 0 [author, 2026-10-18]) because the 90 % figure is
  design §7.5's ratio for four minutes and over ten seconds is a different rule.
  The author's reading: "a handful of spot checks over the course of
  minutes with a forgiving rule as to presence; just enough that it wasn't
  safe to do the initial exchange and walk away."
- **The options.** (a) Restate design §7.5's figures now, against the 30 s
  field-test window; (b) restate them once the witness window is the
  design's 240 s and the post-optical regime is one concurrent state, from
  what runs on the bench then; (c) leave design §7.5 as written and carry the
  field-test relaxation as a build difference indefinitely.
- **What was done meanwhile.** (b) in effect: design §7.5 stands as written; the
  field-test build keeps continuity off and the 30 s window; the regime
  restructuring proceeds first (the author's third ruling of the same day).
- **What changes if it goes the other way.** (a) would tune thresholds to
  a window the author has already said will not be the one users feel, and
  would be redone; (c) leaves the releasable rule untested on hardware.
- **Arose** 2026-10-24, during A6; **deferred** 2026-10-24.

---

## A party's refusal to open a direct path

**Deferred** [author, 2026-10-24]: the direct path is between contacts — "a
client may dial anyone for whom they hold a contact, the recipient will
upgrade to a direct connection if they hold the sender as a contact modulo
any blacklisting process we wish to implement in the future."

- **What prompted it.** Moving the direct-path bound from the horizon to
  contacts (design §12.6.3), after run `58a6883-a5-2` showed every witness
  leg relayed through the node and cut off by its request allowance.
- **The options.** (a) No refusal beyond the contact rule: a contact may
  always open the path; (b) a per-party refusal the person sets — a
  blacklist — read by the recipient's gate; (c) a refusal that also drops
  the contact, so the party is no longer dialled either.
- **What was done meanwhile.** The gate is the contact rule alone; the
  design says a party may decline and that no mechanism yet specifies how
  (§12.6.3, P17). The transport already honours a local refusal
  (`adaptors/tests/direct.rs`, TRV-08), so (b) is a table and a screen, not
  a protocol change.
- **What changes if it goes the other way.** Nothing on the wire; the
  recipient's gate reads one more table.
- **Arose** 2026-10-24; **deferred** 2026-10-24.

## The production value of the peer contact interval

**Deferred** [author, 2026-10-25]: "It is my intention to de-rate the
peer recovery timing when we approach release. Peering should not be a
bandwidth intensive feature and recovery of contacts is not very time
sensitive. I am leaving the contact interval at 1 minute for now for the
same reason the witness window is at 30 seconds; individual test cycles
would be prohibitively slow if these parameters were at their production
values. Likewise, it is fine if contact recovery from peers dribbles in
over the course of a day or so after loss of on-device storage."

- **What prompted it.** The peer backup exchange (design §6.3,
  `wire-format.md` §7.10.3) landed 2026-10-24 with
  `PEER_CONTACT_EVERY` at a minute in the field-test flavour and fifteen
  minutes shipping, the shipping figure chosen by the assistant as a
  placeholder.
- **The options.** A figure in hours, with the reconnect trigger kept
  (the network coming back still contacts at once); a figure that scales
  with the number of peers held, so a user with many peers costs the
  network no more than one with few; or a randomised interval around a
  figure, so a population's rounds do not align.
- **What was done meanwhile.** Fifteen minutes stands in the shipping
  constant and is marked a placeholder in the code and in design §6.3; the
  field-test build keeps a minute.
- **What changes if it goes the other way.** Nothing structural: the
  constant and one sentence of design §6.3. The author's expectation to carry
  into the choice: active users will hold several peers, which multiplies
  the rounds and multiplies the recovery rate alike.
- **When it arose.** 2026-10-24 (the build); deferred 2026-10-25.

---

## What a member is told when a package's request-rate budget is spent

**Deferred** [author, 2026-10-31]: the three assumptions of the utility
build were accepted as they stood, this among them — the budget is
recorded and not enforced until there is an answer to give.

- **What prompted it.** The host surface's manifest (`infra-client-requirements.md`
  §9.3, `resource-requirements.md` §8) names five budgets, and four are
  enforced: the block's bytes, the log's bytes, jobs in flight and job
  seconds a day. `requests_per_minute` is recorded and not enforced,
  because enforcing it is the one budget whose refusal a *member* sees
  rather than the operator, and `wire-format.md` §11's six statuses were
  written before the budget existed. A bound on fetches a minute does not
  exist in `infra-client-requirements.md` §9.3 at all and would raise the
  same question.
- **The options.** (a) Reuse status 2, *resource unavailable*, which is
  true and tells the member nothing to act on. (b) A seventh status for a
  rate refusal, which is a wire change and tells a member precisely what
  a stranger should not learn about a resource's traffic — though that
  section's own rule is that a member inside the owner's horizon is owed the
  specific reason. (c) The host composes an HTTP 429 inside status 0, so
  the refusal is the application's shape and the network says *delivered*,
  which is what it did. (d) Leave it to the package, whose own accounting
  sits above the host's ceiling, which is what `infra-client-requirements.md` §9.3's "treat the rest as
  the package's own accounting" already permits.
- **What was done meanwhile.** Nothing enforces it; the field is read,
  validated and carried. `infra-client-requirements.md` §9.3's "you bound
  what you have built, name the
  one exceeded on your operator's surface" accommodates this without a
  change, so no document says *not yet*. The audit's owed list points
  here.
- **What changes if it goes the other way.** (a) and (d) are a few lines
  in the node's gateway. (c) is the same plus a composed response, and
  sets a precedent for the host answering as the application. (b) is
  `wire-format.md` §11's status list, its normative evaluation order, the
  client's reading of it, the Kotlin binding and the vectors — the only
  option that is not cheap, and the reason to decide before anything
  starts counting.
- **When it arose.** 2026-10-30 (the budgets landed); deferred 2026-10-31.
