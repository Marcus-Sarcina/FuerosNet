# Transaction rules, and the tests that hold them

**What this is for.** The acceptance catalogue grew finding by finding: a
review reported something, a test was written, an entry recorded it. That
produces good coverage of what somebody thought to look at and says nothing
about what nobody did. Four defects found on 2026-09-14 were all of one
shape — a rule correctly implemented in one component, with nothing
computing its input or reading its output — and none of them was a rule
anybody had got *wrong*. A catalogue organised by finding cannot surface
that. One organised by transaction can.

**The method.** For every transaction the wire defines, enumerate the
conditions that govern what a node does with it, from the documents rather
than from the code.

**Coverage is read from the tests, not from the citations.** The first pass
built these tables by asking which entries cite `wire-format.md` §4.x, and
understated §4.5 badly: CER-18 holds five of that section's rules and cites
`light-client-requirements.md` §1.3 and design §8.1.1.3, because those are
where the *obligation* is stated and the wire is where the *encoding* is.
A survey by citation finds the tests somebody filed under a section, which
is not the same question. For each condition, name the component that enforces
it, and require **two** tests: one that the condition is honoured when it
holds, and one that it is enforced when it does not. A condition with only
one polarity is a gap, and is listed as one.

**Where a condition cannot be reached yet** — the proximity channels and
the guided capture need radios and a camera, and milestone 14's shells wait
on two decisions and a toolchain — the test is written and marked
`deferred`, with what it waits for. A deferred test is an `#[ignore]`d test
carrying its reason, not an absent one: it is counted, it is read, and it
runs the day the thing it waits for exists.

**This file is a working record and nothing in the root may cite it.** The
catalogue is the artefact; this is the reasoning that produced it, and the
`transaction` and `condition` fields on an entry are what a checker reads.

---

## Reading the tables

| Column | Meaning |
|---|---|
| **Condition** | The slug the catalogue carries in its `condition` field |
| **Rule** | What the documents say, and where |
| **Enforced in** | The component whose job it is. `decoder` is `rhtn-codec`'s schema plus `rhtn-archive`'s parsers; `verifier` is `rhtn-crypto`'s; `table` is `rhtn-archive`'s topology fold; `store` is `rhtn-node`'s storage and forwarding rule; `client` is `rhtn-client` |
| **+** / **−** | The catalogue entries holding the condition honoured and enforced |

A cell reading **gap** is a condition with no test on that side. A cell
reading **deferred** is one whose test exists and is ignored.

---

## Every transaction: the conditions that hold whatever the type

These are checked before the type is looked at, so a gap here is a gap in
all six.

| Condition | Rule | Enforced in | + | − |
|---|---|---|---|---|
| `envelope-shape` | The envelope is the four-key map §3.2 fixes, and an unknown key in it is refused | decoder | DEC-05 | DEC-09 |
| `locator-nibble-in-range` | A path nibble is 0–9; 10–15 are malformed, and the packed length and pad nibble are fixed | decoder | DEC-23, CER-40 | DEC-23 |
| `cose-profile` | Entries in canonical order, one hybrid pair, no carried payload, no header beyond alg and kid | decoder | — | DEC-17, DEC-19 |
| `declared-algorithm` | A signature's declared algorithm is held to the profile before it is verified | verifier | — | DEC-25 |
| `body-matches-type` | The body is checked against the type its envelope names | decoder | — | DEC-22 |
| `two-parties-differ` | The two parties of a two-party type differ; a node cannot hold authority over itself (§4.1) | decoder | — | DEC-22 |
| `nested-structures` | Structures nested in a signed body are checked, and the public verifier refuses what the record parser refuses | decoder | — | DEC-24 |
| `missing-signer-key` | A signer whose key is absent is a third outcome — held and fetched for — not a rejection | verifier | DEC-26 | — |
| `back-pointers` | One back-pointer list per signer in signer order, reaching the signer's own last record | table | ARC-02 | ARC-03 |
| `genesis-back-pointer` | A key's first transaction carries the genesis value | table | ARC-01 | — |
| `fork-heads` | After a fork both branch heads are carried in the next ordinary transaction | table | ARC-07 | — |
| `effective-time` | A record timed at its predecessor's effective time is accepted; one before it is not | table | ARC-06 | ARC-19 |
| `storage-reach` | Stored when the subject is within h=2 or the counterparty within h=1, and not otherwise | store | PRP-01 | PRP-02 |
| `forward-if-stored` | Forwarded to every adjacency except the arrival one, if and only if it was stored | store | PRP-01, PRP-23 | PRP-02 |
| `duplicate-is-not-news` | An object already held is neither stored again nor re-forwarded | store | — | PRP-03 |
| `originates-from-party` | A client offers its own records to its serving node, which is the only party that can flood them | client, store | PRT-06 | — |

**Gaps here.** `cose-profile`, `declared-algorithm`, `body-matches-type`,
`two-parties-differ` and `nested-structures` have negatives and no
positives: nothing asserts that a *well-formed* object passes each of those
checks rather than passing for some other reason. `missing-signer-key`,
`genesis-back-pointer` and `fork-heads` have the reverse.
`originates-from-party` has no negative: nothing asserts that a node
refuses to flood what a client had no standing to offer.
`duplicate-is-not-news` has no positive, which is the same rule read the
other way: nothing asserts that an object *not* already held is taken.

---

## Type 1 — adoption (`wire-format.md` §4.1)

The type that makes a binding, and the one with the most conditions,
because its evidence field is where three different histories arrive.

| Condition | Rule | Enforced in | + | − |
|---|---|---|---|---|
| `exactly-one-evidence` | Exactly one of fields 6, 8 and 9 is present; none is malformed and two is malformed | decoder | REC-01 | DEC-20 |
| `locator-under-patron` | The locator is the patron's path in the subnet being adopted into, with one nibble added | client | CER-33, CER-34 | gap |
| `self-anchor-suffices` | A party with no ancestor names itself, so a newly minted root can adopt | client | CER-33, PRT-05 | gap |
| `locator-index-is-free` | The index a patron issues is one no subordinate of its already holds, in that subnet | client | CER-40 | CER-41 |
| `fanout-cap` | A patron has at most ten subordinates, which is the ten values a nibble carries | client | CER-41 | CER-41 |
| `slot-unique` | A patron's ten slots hold one open subordinate each; a record claiming a filled one is not stored | table | TOP-34 | TOP-35 |
| `fanout-ten` | The eleventh subordinate has no slot to name, because the nibble carries ten values | table | TOP-34 | TOP-35 |
| `series-opens-at-zero` | A relationship's series opens at counter 0 | table | ARC-04 | DEC-22 |
| `presence-is-between-these-two` | An adoption's named presence record must be between the two parties named | table | gap | TOP-12 |
| `evidence-verified-before-counted` | Dereferenced evidence counts only once its own signatures verify | table | gap | TOP-19 |
| `recovery-both-halves` | A recovery carries an old-key proof and at least one matching verifier response; neither substitutes for the other | decoder | REC-01 | REC-02 |
| `recovery-successor-bound` | The successor statement names the same successor and patron as the enclosing adoption | verifier | REC-01 | REC-02 |
| `recovery-response-shape` | Every response's field 8 equals `prior_key`, its subject equals the adopted node, its verifier differs from its subject, `selection_basis` is 0, and duplicates from one verifier are malformed | decoder | REC-03 | DEC-20 |
| `transfer-parties-differ` | The former patron differs from both the adopted node and the new patron | decoder | — | DEC-27, REC-14 |
| `plain-rotation-carries-nothing` | A plain rotation carries no recovery block | client | — | REC-13 |
| `second-binding-stands` | Adopting elsewhere leaves the old binding in view | table | TOP-02 | gap |
| `key-replaced-on-recovery` | A recovery adoption inside the horizon replaces the old key with the successor | table | TOP-15 | gap |
| `presented-prefix-unbroken` | A history presented from an earlier head verifies as an unbroken prefix | table | ARC-12 | ARC-19 |

**Two conditions added on the author's reading** [2026-09-14], and the
second was a live defect: `propose_adoption` appended nibble `0x00`
unconditionally, so **every subordinate a patron adopted got index 0** —
ten parties at one address, and the routing slot beneath it holding one
occupant. A patron now issues the lowest index it has not already used in
that subnet, read from its own archive, and refuses an eleventh. It is
enforced at the issuer because that is the party that can: a holder
elsewhere may not have the other ten, which is design §1.1's test.

**And passively at every holder** [author, 2026-09-14]: the ruling was that
a duplicate index is caught by a strict limitation of the holder's own
storage rather than by anyone policing the patron. A binding now carries
the slot its locator names, and a table with an open occupant in that slot
has nowhere to put a second — so `slot-unique` and `fanout-ten` are one
bound counted from the other end, and they do not depend on the holder
having seen the other nine. Nothing there adjudicates which of two signed
adoptions the patron meant; the incumbent stays because it arrived, not
because it won.

**Gaps.** `self-anchor-suffices` and `locator-under-patron` have no
negative: nothing asserts that a patron with no position *in the subnet it
names* is refused — the check exists and only the positive path is held.
`presence-is-between-these-two` and `evidence-verified-before-counted` have
no positive, which is the more dangerous way round: nothing asserts that
evidence which *is* good is counted. `second-binding-stands` and
`key-replaced-on-recovery` have no negatives. `transfer-parties-differ` and
`plain-rotation-carries-nothing` have no positives.

---

## Type 2 — departure (`wire-format.md` §4.2)

| Condition | Rule | Enforced in | + | − |
|---|---|---|---|---|
| `one-signature-only` | The old patron does not sign a departure, and a decoder must not expect a second signature | decoder | TOP-03 | gap |
| `ends-only-the-named` | A departure ends the relationship its series names and nothing else | table | TOP-03, TOP-18 | PRP-21, PRP-22 |
| `counter-advances-strictly` | The relationship's counter advances strictly on departure | table | ARC-04 | gap |
| `departed-becomes-root` | A node with no other binding reads as a root in the holder's own table, and is out of the horizon where it was | table | TOP-04 | TOP-33 |
| `order-independent` | The binding a departure names ends whichever of the two arrives first | table | TOP-18 | PRP-22 |
| `reissue-advances-relationship` | A departure in the proven current series ends the relationship a reissue moved | table | TOP-20 | REC-09 |
| `move-inside-horizon` | A move within the replication horizon is accepted without archive presentation | table | REP-04 | gap |

**Gaps.** Three conditions have no negative. `one-signature-only` is the
sharpest: nothing asserts that a departure carrying a second signature is
refused, which is exactly the shape §4.2 calls out.

**On `departed-becomes-root`, and which table** [author, 2026-09-14]. Two
facts, and the first was being stated in a way that invited the second to
be forgotten. In **the holder's own fold** the departed party stays a node
it knows about — it still has the records naming it — and with no patron
left it reads as a root, which is §4.2's own word: a departure is *required
for a node to become a root*. But the **horizon walks open bindings**, so
the party is gone from the place it left: neither the old patron nor a
sibling adopted afterwards can reach it there. §4.2 states that
consequence the other way round — without a departure a node that adopts
elsewhere *remains in the old subtree's view indefinitely* — and TOP-33
asserts it.

---

## Type 3 — disavowal (`wire-format.md` §4.3)

The least covered of the six: three entries, none of them negative.

| Condition | Rule | Enforced in | + | − |
|---|---|---|---|---|
| `ends-only-the-named` | A disavowal ends one relationship and nothing else | table | TOP-05 | TOP-29 |
| `applies-regardless-of-time` | It is applied on verification whatever its timestamp | table | TOP-06 | TOP-29 |
| `ordered-in-patrons-slot` | Field 3 orders it within the patron's own slot, by the patron's timestamp | table | TOP-17 | TOP-30 |
| `reason-is-enumerated` | Field 4 is an enumerated code and never free text | decoder | TOP-31 | DEC-31 |
| `reason-band-is-structural` | Codes 0–31 are without prejudice, 32–63 with; the band is retained and evaluated | table | TOP-31 | TOP-32 |
| `code-space-is-64` | The code space is 64 values, and a code outside it is malformed | decoder | TOP-31 | DEC-31 |

**Closed** (2026-09-14). All six hold on both sides. Writing them found one
real gap: the reason code was carried and round-tripped and **nothing read
the band**, so no policy could act on an unfamiliar code the way §4.3
describes — which is the whole reason the space is banded. `End` answers
`with_prejudice` now.

---

## Type 4 — peering (`wire-format.md` §4.4)

| Condition | Rule | Enforced in | + | − |
|---|---|---|---|---|
| `evidence-present` | A peering carries its evidence; one without is malformed | decoder | gap | DEC-22 |
| `both-parties-exposed` | Both peers' addresses and ASNs are readable from the stored record | store | REP-06 | gap |
| `asn-is-self-asserted` | A self-asserted ASN is accepted and never validated against its address | store | REP-05 | gap |
| `asn-does-not-move-standing` | ASN concentration leaves the metric's output unchanged | metric | — | REP-07 |
| `peering-is-acquaintance` | A peering joins two parties in the acquaintance graph and confers no scope | metric | gap | gap |

**Gaps.** `peering-is-acquaintance` has neither side: nothing asserts that
a stored peering becomes an edge the metric sees, which is the same class
of oversight as the presence records a node never pulls.

---

## Type 5 — presence record (`wire-format.md` §4.5)

**The least covered type and the one carrying the deferrals.** One
catalogue entry cites §4.5. The ceremony that produces a record is tested
end to end (CER-\*, PRT-04) but the record's own rules — the disclosure
construction, what a decoder must do with a presentation, what a recipient
may and may not infer — are almost untested, and the proximity and capture
conditions cannot be reached on a machine with no radio and no camera.

| Condition | Rule | Enforced in | + | − |
|---|---|---|---|---|
| `participants-differ` | The two participants differ | decoder | gap | DEC-21 |
| `formation-is-genesis-rooted` | A formation is genesis-rooted and carries no evidence arrays | decoder | TOP-11 | DEC-21 |
| `normal-record-is-witnessed` | A normal record carries witnesses | decoder | gap | DEC-21 |
| `witness-nominated-by-a-participant` | `nominated_by` is one of the two participants | decoder | gap | gap |
| `witness-names-a-body-entry` | A witness's field 1 names an entry of body field 4 | decoder | gap | gap |
| `signatures-are-classical` | Both signatures in an ordinary record are `COSE_Sign1` and classical only | decoder | gap | gap |
| `consent-is-classical` | The consent signature is classical everywhere | decoder | gap | gap |
| `verifier-auth-hybrid-in-recovery` | Verifier authentication is hybrid only inside a `Recovery` block | decoder | REC-03 | gap |
| `disclosure-root-recomputes` | A recipient verifies by recomputing `root`, and a record whose recomputed root differs from body field 8 is rejected | decoder | CER-18 | CER-18, CER-35 |
| `seven-slots-exactly` | A presented record has exactly seven disclosure slots, in ascending label order; any other count is rejected | decoder | CER-18 | CER-35 |
| `salt-is-sixteen-bytes` | A `Disclosure` whose salt is not exactly sixteen bytes is rejected | decoder | CER-18 | CER-35 |
| `revealed-value-matches-schema` | A revealed value that does not match its label's schema is rejected | decoder | CER-18 | CER-35 |
| `any-subset-accepted` | Any subset of disclosures is accepted, including none | decoder | CER-18 | CER-36 |
| `withheld-is-not-a-default` | A withheld field is never treated as a default value | decoder | CER-36 | CER-36 |
| `no-aggregate-verdict` | There is no aggregate verdict; collapsing the responses into one boolean is a policy act | client | CER-36 | CER-36 |
| `strongest-channel-checked-when-revealed` | §3.2's strongest-channel rule is checked when `proximity` is revealed | decoder | CER-18 | CER-18 |
| `strongest-is-what-passed` | The record says the strongest channel that passed, and nothing is promoted | client | CER-01 | CER-02 |
| `channel-achieved-on-hardware` | The channel recorded is the strongest the hardware actually supports | client, platform | **deferred** CER-37 | **deferred** CER-37 |
| `guided-capture-on-a-camera` | Three to five images over ten to fifteen seconds under prompts that vary, from a real camera | client, platform | **deferred** CER-38 | **deferred** CER-38 |
| `capture-sealed-under-subjects-key` | The capture is sealed under the key the subject supplied and the key discarded | client | CER-04 | gap |
| `sealed-store-on-platform-keys` | The sealed store is held under the platform's key storage | client, platform | **deferred** CER-39 | **deferred** CER-39 |

**Deferred, and what each waits for.** The three marked `deferred` need a
device: a proximity radio for UWB and NFC, a camera pointed at a person for
the guided capture, and the platform key storage a sealed store belongs in.
Milestone 14's shells wait on the mobile framework decision, the binding
generator decision, and a toolchain this machine does not have. The
instrument reaches the `latency` rung and reports what it was told, which
is honest and is not the same claim.

**The presentation is closed** (2026-09-14): every §4.5.1.5 rule holds on
both sides, and the three hardware conditions are written and ignored with
what each waits for. What remains on this type is the record's *own* shape
rules — the witness fields, and which signatures are classical — which the
ceremony tests never reach because a ceremony builds well-formed records.

**And one thing the catalogue had to learn.** A rule whose enforcement *is*
its observation — *never treat a withheld field as a default* is the same
assertion read twice — was being split into two entries that said nothing
new. An entry may now name the side each condition it holds is on, so one
test can hold the positive of one and the negative of another without
ceremony.

---

## Type 7 — series reissue (`wire-format.md` §4.6)

| Condition | Rule | Enforced in | + | − |
|---|---|---|---|---|
| `new-series-at-counter-zero` | A reissue's new series opens at counter 0 | decoder | REC-06 | DEC-22 |
| `never-reissue-into-an-occupied-series` | A series this key has occupied is never reissued into | client | — | REC-07 |
| `reject-an-occupied-series` | A reissue naming a series already in the chain held is rejected | table | — | REC-08 |
| `abandoned-series-is-dead` | Records in an abandoned series are rejected whatever their counter | table | — | REC-09 |
| `seal-before-reissue` | Every old line is sealed as the old key's last act, before the reissue | client | REC-05, REC-06 | gap |
| `chain-is-presented-not-propagated` | The chain is presented when asked to rank and propagated to nobody | client | REC-10 | gap |
| `chain-reissue-not-before-its-record` | A presented chain whose reissue is timed before the record it follows is rejected | table | ARC-13 | ARC-19 |
| `seal-freezes-a-chainless-holder` | A thief's seal freezes a chainless holder's entry, and never moves it afterwards | table | — | REC-11 |
| `reissue-advances-the-relationship` | A reissue advances the relationship it names | table | TOP-20 | gap |

**Gaps.** Four conditions have no positive and four no negative.

---

## What this survey found

**Counted rather than characterised**, and counted by
`Robot/matrixcheck.py` rather than by hand — the first draft of this
paragraph said sixty-two conditions and twenty on both sides, and the
second said twenty-one after three rows had been written with the same
entry on both sides of a condition. An entry has one kind and cannot be
both; the checker found all three. Six transaction types, 82 conditions drawn from the documents,
and of them:

- 38 hold on both sides
- 36 have one polarity only
- 5 have neither
- 3 are deferred on hardware

**The shape of the gaps is not random.** They cluster where a rule is
enforced in a component nobody wrote a scenario against: the disclosure
construction in the decoder, the disavowal's reason bands in the table, the
acquaintance edges in the metric. That is the same shape as the four found
on 2026-09-14 — a rule correctly implemented with nothing exercising it —
and it is what a catalogue organised by finding cannot show.

**Closing them is the work, and the counts above move as it goes.** The
catalogue carries the pairing itself — an entry holding a transaction rule
names the `[type, condition]` pairs it holds, and `acceptance/tools/check.py`
flags a condition with one polarity — so a gap closed here is a gap the
gate stops complaining about, and a gap opened by a new rule is one it
starts complaining about.
