# Review Finding Tracker

**Purpose:** every enumerated finding from every review pass gets an explicit
disposition here before an edit round is called complete. Introduced after review
pass 0.5, where seven of eighteen findings were addressed and the remaining
eleven were neither fixed nor recorded — the response engaged the framing and
dropped the list.

**Section references here are as-of-filing and are not remapped.** A finding
recorded against §10.5.4 was filed when that section existed; the design has been
renumbered several times since. **Chasing them would falsify the record** — the
point of this file is what was found and what was decided, not where the text sits
today. Use the change log to follow a section through its moves.

**Dispositions:** FIXED · FIXED-DIFFERENTLY · ACCEPTED (assumption adopted as
spec) · DEFERRED (with owner//reason) · REJECTED (with reason) · NOT-A-FINDING

---

> **Note on dates:** entries stamped `2026-08-12` in this file actually span
> 12–15 August 2026. The date was carried forward without re-checking. Ordering is
> correct; individual dates are not.

## Pass 0.6 — implementation attempt (adoption encode/sign/verify)

### Contradictions found

| # | Finding | Disposition |
|---|---|---|
| C1 | COSE `alg` declared `uint`, but EdDSA is −8 and ML-DSA are −48/−49/−50 | **FIXED** — changed to `int` throughout |
| C2 | `Recovery.2` is `[+ VerifierResponse]` but attested rotation needs none | **FIXED** — changed to `[* VerifierResponse]`, with the structural rule stated |

### (b) UNSPECIFIED questions

| # | Question | Disposition |
|---|---|---|
| U1 | Is CBOR actually the v1 encoding? (`[P]`) | **ACCEPTED** — promoted from [P] to [D]. CBOR + deterministic encoding is now a decision, not a proposal |
| U2 | Is txid SHA-256 of deterministic body CBOR? (`[P]`) | **ACCEPTED** — promoted to [D] |
| U3 | Type of COSE algorithm identifier | **FIXED** — see C1 |
| U4 | "COSE signatures" claimed but a custom map defined | **FIXED** — adopt real RFC 9052 COSE. The custom `Signature` map is removed. Resolves U4, U9, U10 together, and buys domain separation via `Sig_structure` |
| U5 | What exactly does keyhash hash? | **FIXED** — SHA-256 of deterministic CBOR of the **COSE_Key** (RFC 9052 §7). Also resolves U18 |
| U6 | Must the whole envelope be canonical, or only the body? | **FIXED** — whole envelope; non-canonical is malformed |
| U7 | Exactly two signatures? Extras? Order? | **FIXED** — exactly the required signer set, no extras, no duplicates, order-independent |
| U8 | Empty verifier list on attested rotation | **FIXED** — see C2 |
| U9 | What does the old-key recovery signature sign? | **FIXED** — COSE_Sign1 over the canonical Recovery map with the signature field omitted |
| U10 | What does the verifier-response signature sign, with what algorithm? | **FIXED** — COSE_Sign1 over fields 1–6; algorithm carried in the COSE protected header, which removes the missing-alg problem |
| U11 | How many verifier attestations make a recovery structurally valid? | **FIXED** — structurally ≥1 when no old-key signature; anything beyond that is policy weight, not validity |
| U12 | Schema version and mismatch behaviour | **FIXED** — v1 is current; unknown version is unverifiable and MUST be rejected |
| U13 | Unused low nibble of an odd-length path | **FIXED** — MUST be zero. Otherwise one logical path has sixteen encodings |
| U14 | May `prior_key` equal the new key? | **FIXED** — rejected; it represents no rotation |
| U15 | Exact cycle-prevention procedure | **DEFERRED** — design §6.2.5 already marks it open. The reviewer's rule (reject on positive local knowledge, never on absence) is adopted as the *interim* rule and recorded |
| U16 | Precise monotonic sequence rule | **FIXED** — strictly greater than last known; **not** previous+1, since a verifier may not have seen intervening transactions |
| U17 | How routine classical signatures bind to hybrid long-lived identities | **FIXED** — real design decision, see §5. Identity keypair is hybrid; routine transactions signed by the classical component; presence records and identity changes by both; keyhash covers both |
| U18 | Public-key byte encoding in KeyMaterial | **FIXED** — subsumed by U5's COSE_Key decision |

### (c) TYPE DECISIONS

| Item | Disposition |
|---|---|
| `Option<Vec<TxId>>` — absent vs present-empty are cryptographically distinct | **FIXED** — empty arrays MUST be omitted, never encoded. Removes the malleability |
| Closed enums vs `Unknown(u64)` for result / basis / message type | **FIXED** — unknown enum *values* in known fields MUST be rejected in v1. Distinct from unknown *map keys*, which MUST be preserved |
| Unbounded `Vec` for archive subset, verifiers, path length | **FIXED** — explicit bounds added; unbounded arrays from strangers are a resource attack |
| Integer widths (timestamp, seqno, version) | **FIXED** — bounded to 64-bit explicitly |
| Signer-role enum (not on the wire) | **NOT-A-FINDING** — role is correctly inferred by comparing signer to body fields; adding a role field would be redundant and forgeable |
| Owned vs borrowed signed data | **NOT-A-FINDING** — local implementation concern. The reviewer's note that signing must operate on an immutable snapshot is sound engineering guidance, not a spec gap |
| Unknown map field preservation | **ALREADY SPECIFIED** — the wire rules already require it; the reviewer confirms it is load-bearing |

### (d) CRATE MATURITY

| Item | Disposition |
|---|---|
| `ml-dsa` 0.1.1 and `ml-kem` 0.3.2 exist, pure-Rust, `no_std`, wasm-capable, **unaudited** | **ACCEPTED** — recorded in design §5 as an implementation constraint |
| Browser wasm needs `getrandom`/Web Crypto integration | **ACCEPTED** — recorded |
| "Hybrid" names a category, not a construction — having both crates does not solve it | **FIXED** — see U17 |
| `coset` unusable until the COSE shape is settled | **RESOLVED BY** U4 |

### Self-check after edits (2026-08-12)

Ran a residue check on the COSE conversion rather than waiting for a review pass.
**Found five sites in `network-design.md` still describing signatures the old way**
(`sig over H(full body)`) while the wire format had moved to COSE — the same
one-file-updated drift this project has hit three times. All five corrected, and
§6.6 now states that the wire format is authoritative on encoding, so the
duplicated schema is marked as a reading aid rather than a second source of truth.

**Implication:** a mechanism *replacement* leaves more residue than a rephrasing.
The review plan's rule ("verify after passes answered by rephrasing") is too
narrow — replacements need it more, not less.

### Remaining after this round

- **U15** — cycle prevention, interim rule recorded, procedure still open (design §6.2.5)
- **Test vectors** — the reviewer's tenth minimum fix. No canonical positive/negative
  vectors exist. **DEFERRED** to first implementation; nothing can be interoperably
  tested until then, and vectors written before an implementation tend to encode
  the spec's own mistakes.


---

## Pass 0.5.2 — unenforceable mandates

Seven patterns, nine textual occurrences. **All valid.** Several were written
*after* §1.1 stated the principle they violate.

| # | Finding | Disposition |
|---|---|---|
| ROOT | Nine MUSTs across the enforcement boundary, including some added after §1.1 | **FIXED at the root** — §0 now fixes a normative vocabulary: MUST only where a recipient can check it from evidence; otherwise "the reference client…" or "the reference policy…" |
| 1 | §4.4 "peer selection must consider ASN and cloud region" | **FIXED** — the peering record carries ASN and prefix so independence is *visible*; the reference client prefers diverse peers and surfaces when they are not |
| 2 | §6.2.3 "implementations must not force re-verification" | **FIXED** — reference client does not; others may and remain conforming. The condition is derivable from topology either way |
| 3 | §6.5.4 disclosure must respect the more restrictive policy | **FIXED** — a verifier response MUST cite the disclosure policy version and the requester's authorisation basis, which *is* checkable. Off-protocol disclosure is acknowledged as unpreventable, but cannot be laundered into the trust graph |
| 4 | §6.5.4 "a single negative must never be treated as damning" | **FIXED** — protocol exposes the count and inconclusive rate; the reference *policy* does not treat one negative as damning |
| 5a/5b | §6.7 and §8 "attestation must be pull / never proactively distributed" | **FIXED, and improved** — an attestation delivery MUST carry the requesting evaluator's nonce. That is checkable, so it is a genuine MUST, and unsolicited copies become visibly unsolicited and locally rejectable. Converts an unenforceable mandate into a mechanism |
| 6 | §9.2 "normative rule for the spec: λ < 1/f" | **FIXED** — reframed as a soundness condition for anyone using distance decay, with a **policy descriptor** publishing metric family, parameters and computed resistance bound (the §9.4 conformance test) so the choice is visible |
| 7 | §9.5 "must not watch real relationships evaporate" | **FIXED** — the schema rule (facts permanent, standing derived) is enforceable and stated as such; the non-evaporation is reference-policy behaviour |
| — | §6.5.4 and §9.6 "must never leak into social trust weighting" (repeated) | **FIXED** — the *separate typing* of reliability and social evidence is enforceable and is what the protocol guarantees; whether a policy keeps them separate is not, so the reference policy does and others are visibly free not to |

### Note on reviewer independence

The user reports that context purges may have been ineffective, as the reviewing
model retained a cache across sessions. **A larger issue applies regardless:** the
document now contains §0's conventions, §1.1, the §11.x registers and a change log
describing every prior pass, so *any* reviewer knows what previous passes
concluded. True naivety is no longer recoverable by clearing context.

Mostly benign — it is why this pass sensibly declined to re-flag the departure
veto. **But for pass 0.8 (adversarial), supply a version with §11.x and §15
stripped.** An attacker should not be handed the defender's own list of known
weaknesses.


---

## Pass 0.7 — LINDDUN privacy analysis

23 analysis units, 7 categories each. The reviewer's central conclusion is
adopted as a design principle.

| Finding | Disposition |
|---|---|
| **META: privacy weakness is composition, not transmission** — every artifact is well minimised and none of them compose | **FIXED at the root** — new §10.5.1 states the composition invariant. §6.5.2 had said exactly this for biometrics and never generalised it |
| Presence-record composition (High) | **ACCEPTED as finding P1.** §10.5.3 proposes Merkle-ised record bodies for selective disclosure — the only lever that addresses composition rather than individual fields |
| **Verifier/witness social-graph leakage (High)** — a record names a sample of the subject's *prior* counterparties and neighbourhood witnesses | **ACCEPTED as finding P2, open.** Genuine tension: anti-suppression needs visibility, visibility leaks history. Threshold/aggregate signatures are the direction but sacrifice the visible-absence property. §6.6's "strictly dominates" claim corrected to "a trade" |
| v1 cross-subnet correlation (High) | **ACCEPTED as P3** — ships as a known defect; the privacy property depends on a deferred feature |
| Patron metadata + mailbox (High) | **ACCEPTED as P4.** §10.5.5 sharpens the open item: a size bound is not a privacy measure. Queued-payload encryption, deletion semantics, crash-recovery copies, operator logging, sibling queue state |
| Endpoint/backup aggregation (Critical on compromise) | **ACCEPTED as P5** — already acknowledged in §7.8.7 and §7.8.7.1 |
| Forwarding records as a post-departure linkability window | **FIXED** — the 90-day TTL is now documented as a privacy parameter as well as an operational one. Departure does not fully sever for 90 days |
| Activity summaries export a behavioural baseline to strangers | **FIXED** — privacy cost recorded in both documents; summary should be coarse and the subject should know it exists |
| Topology deanonymisation by association | **ACCEPTED as P8** — not previously analysed as an attack |
| Divergence-notification fan-out | **ACCEPTED as P9** |
| Policy-descriptor fingerprinting | **ACCEPTED as P10** — use bucketed rather than raw parameters |
| Heartbeat patterns reveal routines | **ACCEPTED as P11** — process-and-discard helps; logging is the residual risk |
| Unawareness across many units | **FIXED** — §10.5.6 makes capture-time disclosure a reference-client obligation. The schema cannot fix comprehension |
| Nine deliberately accepted costs | **NOT-A-FINDING**, but **consolidated into §10.5.7** — previously scattered, so a reader could not distinguish an accepted cost from an unexamined one |

### Note

Privacy had no home in this document before this pass. It was argued locally at
each mechanism and never assembled, which is precisely why the composition
finding went unnoticed for so long. §10.5 now exists.


---

## Vignette V6 — composition tradeoff (user-proposed, 2026-08-12)

**Adopted with two amendments.** The comparative-baseline argument is legitimate:
privacy analysis routinely measures against a hypothetical zero-disclosure world
rather than the one users inhabit, and the closing distinction — you can already
*communicate* with your social network, this helps you *coordinate* with it — is
the clearest statement of the trade in the document.

Amendments:

1. **Aggregation cost restored.** The draft implied that identical facts mean
   identical exposure. This document argues the opposite about camera networks
   (§6.5.2's Ring/Flock comparison) and cannot abandon it here: scattered,
   unauthenticated, judgement-requiring facts are not the same object as an
   authenticated, machine-readable, pre-correlated record set.
2. **Baseline scoped.** It describes a median user with mature social platforms.
   It is false for deliberate minimisers and dangerously false for activists,
   dissidents and people hiding from a former partner — for whom composition is a
   **new** exposure. Registered as assumption **A12**, noted as *known false for
   part of the population*.

**Genre risk flagged in §0.** V6 is persuasive rather than illustrative, which
makes it quotable to dismiss future findings. It carries its own counter-argument
and an explicit statement that it may not be used to wave away a composition
finding. §0 now says to prefer illustration and mark persuasion.


---

## Correction — the camera-network comparison (user, 2026-08-12)

**The author was wrong, twice, and the error had propagated.**

1. **Wrong comparison class.** Ring and Flock were cited as examples of
   *fragmented, high-friction* data whose frictions constitute privacy. They are
   the opposite: Ring's police-partnership programme and Flock's business model
   are designed aggregation and push to law enforcement. The fragmentation is at
   the camera; the aggregation is the product. The document compared this network
   unfavourably to something structurally worse.
2. **The access-cost claim was already false for this design.** Attestation had
   been pull-only since §8. Presence records are never flooded and never globally
   replicated; there is no store to walk. Composition requires penetrating
   neighbourhoods individually — the very friction being credited to camera
   networks and denied to this one.

**What survives:** authentication and durability. A presence record is signed and
immutable for decades where a platform claim is unverified and deletable. Real,
but narrower than claimed, and independent of access cost.

**Actions:**
- V6's aggregation-cost paragraph rewritten; the camera comparison removed
- New **§10.5.1.1** states disaggregation as a positive architectural property —
  §10.5 had catalogued composition risk without crediting what limits it, making
  the privacy posture read worse than it is
- The error is recorded in §10.5.1.1 rather than silently deleted

## NEW FINDING P12 — no end-to-end payload encryption (Critical)

Surfaced by the user's framing that the network layer is scaffolding and the real
payload is point-to-point communication. If that is what the system exists to
carry, its confidentiality matters most — and **it is not specified**.

§7.6.3 protects each *hop* with PQ KEM. Hop encryption terminates at each infra
node, so **both serving infra nodes currently see payload plaintext**. §7.9.6
listed "end-to-end encryption of queued payload" as an open queue question, which
confirmed it was settled nowhere.

The patron was accepted as a **metadata** chokepoint (§7.6.3). It was never
intended to be a **content** chokepoint, and nobody had written the distinction
down. New §10.5.1.2 states the requirement; key agreement between endpoints, its
relation to §5.1 identity keys, and forward secrecy are all undesigned.

## Meta

The user notes he did not press the original camera-network point at the time, to
avoid discouraging the author from valuing privacy highly. Understandable, and
the wrong trade: deference left a false claim in the document for weeks and it
propagated into a vignette. **Correction is more useful than encouragement.**


---

## Resource object (user proposal, 2026-08-12)

**Adopted, object only; protocol deferred.**

The proposal: participants launch services, data stores and applications that
piggyback on the network's identity and trust, with access adjudicated by the
requester's position relative to the resource owner. Agents are resources with
permission to act with respect to users and other resources.

**Strongest argument, not made in the proposal:** this is where enforcement
actually works. §1.1 says the protocol can only compel where shared state exists,
and almost nowhere does — but a resource owner and a requester inside the owner's
subtree share topology, and the owner runs the service. Access control is
genuinely enforceable here rather than merely observable. First proper application
of §1.1's diagnostic in the affirmative.

**Resolves §13.10.** The delegated/autonomous agent split was awkward; the
resource object supplies a structural line — *borrows authority → resource; bears
its own costs → node.* Several properties the agent design had to assert now hold
structurally, notably that an agent can never accumulate social trust of its own
(it cannot attend a ceremony).

**Deferred:** permission vocabulary, interaction protocol, owner-movement
semantics, resource-access privacy.

**Scope note recorded in §6.9.4.** Recent passes contracted the design; this
expands it. It earns its place by simplifying the agent question conceptually,
but the asymmetry is worth naming — expansions deserve more resistance than
contractions.

**Correction accepted:** P12 (no E2E payload encryption) was framed as a "serious
omission". It is an unwritten section of an undesigned area — work to date has
been almost entirely control plane. Severity Critical for what ships is right;
the framing was not.


---

## P12 refined — endpoints vary (user, 2026-08-12)

"End-to-end" means encrypted to the **addressed endpoint**, which is not always
another person: leaf-to-leaf, leaf-to-patron, and leaf-to-resource are three
distinct cases. New **§7.10** names them.

**Key consequence: the patron has two roles.** As a *relay* it must see ciphertext
only; as an *endpoint* — attach, heartbeat, queue operations, currency requests —
it legitimately reads plaintext because it is the addressed party. Conflating
these is precisely how "the patron sees everything" returns after being designed
out. §7.6.3's metadata-chokepoint acceptance and §10.5.1.2's content-chokepoint
prohibition are reconciled here.

**Enforcement is affirmative under §1.1** for the second time in one session (the
first being resource access control, §6.9.1): the client simply does not give the
relay the key. Nothing to observe or weight.

**Two consequences that are forced, not optional:**

1. **Store-and-forward requires asynchronous key agreement.** §7.9.4 makes
   store-and-forward the default because clients are usually backgrounded, so no
   interactive handshake is possible. Prekeys published by each node and served by
   its patron, X3DH-shaped. Brings prekey exhaustion DoS, a reusable last-resort
   key at reduced forward secrecy, and rotation policy. Also a **new metadata
   event**: fetching a prekey reveals intent to message before the message exists.
2. **§5.1 specifies signing keys only.** Encryption needs KEM keys, distinct and
   bound to the identity. The §5 table assigns a PQ KEM to *hop transport* and
   says nothing about payload keys. Recorded as a gap in §5, not only in §7.10.


---

## Pass 0.8 — adversarial, role: hostile client implementer

**Run notes.** Two attempts at maximum effort died silently — once with all nine
roles, once with a single role plus `wire-format.md`. The third attempt, at
**medium** effort, single role, **without** the wire format, succeeded and
produced the best findings of any pass. Conclusion: the constraint is context and
output volume, not reasoning depth, and **0.8 does not require maximum effort**.
Recorded in the review plan.

Reviewer discarded 10 candidates as RESTATES. Four findings kept.

| # | Finding | Class | Disposition |
|---|---|---|---|
| 1 | **Patron eclipse has a valuable asset after all** — the victim's authentication decision-making environment. "No tradable token" is true and irrelevant | REASONING | **FIXED.** §10.4's justification withdrawn and rewritten. Also records that §7.4 (no cold lookup) *amplifies* eclipse — a privacy property making a security property worse, never previously noted — and that **subnet plurality is the structural mitigation**, upgrading it from side effect to defence |
| 2 | **Selective-abort grinding** — commit-reveal stops changing the nonce, not aborting the attempt and restarting for a new one | EXTENDS | **FIXED.** Two rules, both required: seed from participant pair plus coarse time window (restarting within it gains nothing), and reveal nonces only *after* the physically expensive steps. **The revelation point was previously unspecified**, and it decides whether abort-grinding is cheap or ruinous |
| 3 | **Selective history presentation defeats recomputable selection** — the evaluator learns *n* from the subject, and §9.7 permits disclosing a subset | NOVEL | **ACCEPTED as a structural gap, direction proposed.** New §6.6.1. Per-subnet presence chain with the head carried in the patron's currency attestation. Not decided — it changes the record, the attestation and the archive model |
| 4 | **Retention laundering via `personal_knowledge`** — a hostile client uses illegally retained photos and reports memory | NOVEL | **FIXED.** The "detectable by its own use" claim is withdrawn; no protocol fix exists. Registered as **P13** |

### Assessment of finding 3

The sharpest observation in the review is the general one: *the design authenticates
individual pieces of evidence very well, and is much weaker at proving a client has
shown you all the pieces whose existence affects a security calculation.* Verifier
selection, its threshold, and the anti-suppression property all depend on **negative
knowledge** — knowing which records *should* exist. A collection of individually
immutable signed objects does not supply that.

### Confirmed defences

Signed locators hold. NFC is not overclaimed — the spec already says it is not
relay-resistant. Sensor forgery is correctly scoped by §6.5.8. Formation records
cannot be laundered into looking independently corroborated.


---

## User proposals following pass 0.8 (2026-08-12)

### 1. Remove patron countersignature from PoP — ADOPTED

Resolves a pre-existing inconsistency (§6.4's blanket countersigning rule vs
§6.6's schema, which had no patron signature field) and removes a real attack
surface: a patron able to veto PoP shapes what *later* patrons see, attacking
portability itself.

**Also punctures Finding 1's eclipse attack**, and better than subnet plurality
does. §7.8.2 permits witnessless ceremonies, so an eclipsed user can always form a
formation-typed PoP with anyone they physically meet. **The false social universe
is permeable wherever the victim meets a human**, requiring nothing unusual of
them.

*Side effect:* invalidates the author's proposed fix for Finding 3, which put the
presence-chain head in the patron's currency attestation. Moot — proposal 2
supersedes it.

### 2. Self-chained archive — ADOPTED, dissolves Finding 3

Each transaction carries a hash of the subject's previous one, signed by the
counterparty. Sequence position becomes as trustworthy as the record. Excision is
impossible; only truncation to a prefix remains.

**The economic argument the proposal left implicit:** truncation is
self-defeating. History *is* standing, so trimming to lower the `min(n/2,10)`
threshold trims apparent standing proportionally, and trimming fully is identical
to starting a fresh identity — already permitted, already costly, requiring no
attack. Finding 3 does not get mitigated; it **dissolves**.

**Emergent property, undesigned: the archive is a second factor.** A thief with
the key but not the archive can act in subnets where the key is current, but joins
new subnets only as a fresh user, and risks unmasking if genuine history surfaces
there. Key alone is not portability; key plus archive is.

**Scope decided: one chain per identity**, spanning subnets. Cross-subnet leak is
bounded by the existing rule that a reviewing patron ignores transactions with
unreachable counterparties — an unfamiliar entry contributes chain length and
nothing else.

**New open item:** archive loss is history loss, and presence-based recovery
restores keys only. §7.8.7.1's backup requirements are now much more load-bearing.
**New P14:** back-pointers leak activity level to counterparties.


---

## Self-review sweep (2026-08-12, no context clear)

Run after the resource, payload-encryption, chained-archive and PoP-countersignature
changes. Scoped to the failure classes earlier passes actually caught: stale
claims, incomplete propagation, broken references. **Not a substitute for external
review** — anchoring from the design conversation is intact and Claude-family
blind spots are unaddressed.

**Method:** explicit list of what changed, then grep each term across both files,
rather than reading impressionistically. Same discipline the 0.5 failure lacked.

| # | Finding | Class | Fix |
|---|---|---|---|
| R1 | §1.1's visibility table still asserted over-retention is "detectable by its own use" — the claim withdrawn in §7.8.7.1 after 0.8 | Stale claim | Rewritten to say the opposite, and now serves as an honest example of the *limit* of visibility-in-place-of-enforcement |
| R2 | "Patrons already countersign every subordinate transaction" — false since PoP was excluded | Incomplete propagation | Scoped to subnet transactions; notes a patron-signed activity summary cannot speak to meeting history |
| R3 | §8's message classes omitted resources entirely | Incomplete propagation | Resource registration added to Topology |
| R4 | §8 said payload is stored by "endpoints and their patrons" — contradicts §7.10, where relays hold ciphertext | Contradiction | Corrected to endpoints only |
| R5 | §10.5.3's Merkle proposal could be read as reinstating the record-level subsetting that §6.6.1 just forbade | Ambiguity | Distinguished explicitly: records constrained, fields liberalised, opposite directions |
| R6 | Build order omitted the chain, E2E payload and resources. **The chain cannot be retrofitted** — back-pointers must exist from the first transaction | Incomplete propagation | Added, with the retrofit warning |
| R7 | Archive retention became load-bearing (second factor, portability) with no assumption registered | Missing register entry | **A13** added |
| R8 | The abort-grinding fix introduced a seed-window parameter absent from §12 | Missing parameter | Added as UNSET, with a note that it sets the grinding rate |
| **R9** | **The chain was applied only to presence records.** Every transaction advances its signers' chains — adoption, departure, disavowal, peering | **Author's own incomplete propagation, same session** | Back-pointers moved to a **common body field (key 0)** across all types, one per required signer |
| R10 | Presence-specific back-pointer field superseded by R9's common field | Consequence of R9 | Replaced; note added that no patron signature appears |
| R11 | No wire transaction type for resource registration | Incomplete propagation | Type 6 reserved, marked unspecified |

**R9 is the instructive one.** I wrote the chained-archive section and the wire
format change in the same session, and still put the back-pointer only in the
presence record — while the design text said "user transactions" generally.
Propagation failures are not a function of elapsed time or forgotten context; they
happen within a single edit.

**Genesis value specified** as a side effect: `SHA-256(signer keyhash)`, derivable
by any verifier, so a claimed first transaction is checkable rather than
assertable. Previously listed as open in §6.6.1.2.


---

## §13B feature-gap review (2026-08-12)

Nine candidate gaps proposed by the author, tested against the existing design by
the user. **Five dissolved, two real, two product.**

| Gap | Outcome |
|---|---|
| Negative attestation | **DISSOLVED.** Disavowal *is* negative attestation, costly to the issuer and unspammable. The peer-to-peer case is answered by scope — *we do not produce trust, we capture and formalize it* — and by cessation plus payload, which is what happens in life. §9.5's decay is an already-specified loss channel the author had missed |
| Succession | **DISSOLVED.** Everything survives a permanently absent patron except subnet-scoped countersigning: currency via sibling issuance, routing via sibling failover, adoption and PoP ungated. Degradation, not failure. One consequence newly recorded — a dead *anchor* cannot publish a forwarding record, so cached locators into its subtree fail until gossip carries the new anchors |
| Group operations | **DISSOLVED.** An abstraction over pairwise operations in a standard library, not a protocol primitive. Protocol-level group semantics would mean ACID-shaped updates over a caching, forwarding network |
| Resource abuse | **DISSOLVED.** Resource policy, plus one protocol abuse-report event type addressed to the owner. Consistent with §6.9.1, where the owner is the one party who can genuinely enforce |
| Time / cross-identity ordering | **DISSOLVED.** Impossible without global identity, and not required: the chain gives per-identity total order, witness-corroborated timestamps give what impossible-travel detection needs |
| **Multi-device** | **REAL.** Storage is solved — §7.8.7.1's envelope encryption means existing password managers and cloud sync carry the blob, and building private replication would duplicate a solved problem. **The problem is chain semantics**: two devices appending fork the user's own chain, which no generic sync tool resolves. Three shapes recorded, none chosen |
| **Intra-subnet discovery** | **REAL.** Service catalog, subsettable and composable. **Prior art: DNS-SD / mDNS (RFC 6763)** maps almost directly — type, instance, TXT metadata, per-domain browse. Two adaptations: propagation follows the horizon rather than a broadcast domain, and entries carry their permission requirement so browsing shows what you can use. Instructive failure to avoid: UDDI |
| State synchronisation | Product-adjacent; bounded by horizon, protocol unspecified |
| Invitation flow | Product rather than protocol, but the path every user takes exactly once and currently nobody's |

### Method note

**Five of nine dissolved.** A list of apparent gaps written by one party is
substantially a list of things that party has not connected. Testing each against
the existing design is far cheaper than building, and should precede any feature
work.

### Scope statement adopted into §1

> *We do not produce trust. We capture and formalize it, so it can be leveraged
> as a computational input.*

The clearest scope statement produced in this project. It disposes of an entire
class of why-doesn't-it-do-X questions: where a proposed feature would have the
network **decide** something about people rather than **record** something they
did, it is out of scope by construction.


---

## Chain forking (author, following up an acknowledged risk, 2026-08-12)

The user flagged multi-device chain forking as a risk without a response. Followed
through: **it is worse than first stated, and it is a security problem rather than
a usability one.**

**A deliberate self-fork reintroduces selective disclosure.** Accumulate standing
to record N, then have two devices extend separate branches. Present branch A to
one subnet, branch B to another. Both inherit the full pre-fork history and its
standing; each omits the other's post-fork transactions. This is **strictly better
for an attacker than §4.1.1's permitted identity plurality**, where each identity
starts from zero — a fork is plurality *with inherited reputation*.

Detection requires an observer holding both branches. Partitioning is what this
architecture provides, so in the case that matters, nobody does.

**Corrections made:**

- §6.6.1's claim that the chain leaves "truncation to a prefix" as the only edit
  is **overstated**. Accurate statement: **a prefix plus one divergent tail per
  branch maintained.** Excision within a branch remains impossible and *k* branches
  cost *k* times the transactions, so the improvement over arbitrary subsets is
  real and large — but not total.
- §13B.3 reframed from usability tradeoff to security decision. **Only shape 1
  (single primary device) prevents forking structurally.**
- Noted that shape 3 (per-device keypairs, declared) makes forking *legitimate and
  visible* rather than covert, which may be the honest resolution.
- **Partial mitigation added, available under any shape:** publish the current
  chain head alongside the currency attestation. Two branches then disagree about
  the head as well as sharing a predecessor, so anyone encountering both detects
  it. Converts a covert fork into one detectable on contact — the §1.1 move.

**Method note:** this surfaced by following through on an acknowledged uncertainty
rather than leaving it as noted-and-parked. Worth doing when the person raising it
says "no immediate response" — that is precisely when nobody is going to look
again.


---

## Chain forking — author's finding RETRACTED (2026-08-15)

The author claimed a deliberate multi-device fork "reintroduces selective
disclosure" by letting both branches inherit pre-fork standing while each conceals
the other's tail. **The user retracted it and was right.**

**The error: inventing a property, then finding it violated.** §7.8.7 states
plainly that *users can and will fork their identities, and this is fine — the
archive informs new subnets on joining, not to hold users accountable across
subnets.* Cross-subnet accountability is **disclaimed by design**. Presenting
different views to different subnets is not an attack on a guarantee; it is the
guarantee behaving as written.

The claimed "gain" also evaporates: presenting the same history to two subnets is
the archive's intended function, and a reviewing patron already ignores
transactions with counterparties it cannot reach, so the concealed entries were
opaque regardless.

**Corrections:**

- §6.6.1's original claim restored. The guarantee is **per-evaluation**: within any
  one subnet's view, truncation to a prefix is the only available edit. The
  author's "prefix plus one divergent tail per branch" correction was itself the
  error.
- §13B.3 reframed again — from *security problem* back to **integrity and
  usability problem**, and specifically the **accidental** case: two devices
  appending concurrently fork silently, so a ceremony performed on one device is
  simply absent from the other's branch. Silent loss of the user's own history.
- **Shape 3 identified as the likely answer**, for concurrency reasons rather than
  security ones: per-device keypairs with published bindings mean there is no
  shared chain to fork, no coordination required, and offline signing works.
  Shape 2 cannot sign offline, which defeats the mobile case.

### Failure mode worth naming

**Self-review can generate findings against imagined requirements.** An external
reviewer works from the document and is anchored to what it claims; the author
carries an internal model that may include properties the document explicitly
disclaims. This produced a confident, detailed, entirely spurious security finding
— and then a "correction" to a correct section on the strength of it.

Cheap check before accepting any self-generated finding: **quote the document
sentence stating the property being violated.** Here, no such sentence existed; the
nearest one said the opposite.


---

## Archive merges (user, 2026-08-15) — resolves multi-device

The user proposed that forked chains be **merged**: a post-fork transaction takes
both branch heads as back-pointers, with the observation that *the chain was built
to prove no intermediate messages are missing, not to sequence events*, so losing
cross-branch order costs nothing.

**Adopted, and it does more than reunite branches — it strengthens completeness.**
A merge **commits to both branches**, so omitting one afterwards leaves a
back-pointer unsatisfied and is detectable. Excision within a branch was already
impossible; the merge closes off dropping a whole branch.

The structure is properly a **Merkle DAG**, git-shaped, for the same reason git
is: concurrent authors, no coordination, reconcile afterwards rather than prevent
divergence.

**Consequences:**

- **No merge transaction type needed.** The next ordinary transaction carries two
  back-pointers. Wire change: back-pointers become a **list per signer**, length
  one in the common case.
- **Supersedes all three multi-device shapes** (§13B.3). No primary device, no
  head-check before signing (**so offline signing works**), and no published
  device-binding declaration — **the merge is self-describing**, naming both heads,
  so an evaluator learns the structure from the record rather than from a prior
  declaration needing maintenance and revocation.
- **Incentive aligns without enforcement.** Merging makes a history more complete,
  completeness confers standing, so users want to merge.
- **Asymmetry against a key thief:** the legitimate holder who merges has a
  strictly more complete history, and the thief cannot merge without a branch they
  do not hold. Sharpens §6.6.1.1's second-factor property rather than weakening it.

**Note on the sequence of errors here.** The author (a) framed deliberate forking
as an attack, which was spurious; (b) "corrected" a correct section on that basis;
(c) after retraction, reframed the real problem as needing *prevention* via one of
three awkward shapes. The user's proposal shows prevention was the wrong frame
entirely — divergence needs to be **reconcilable**, not impossible. Two rounds of
analysis were spent on the wrong question.


---

## Pass 0.1 second run — factual verification (2026-08-15)

Run against both documents after the resource layer, payload confidentiality,
merges and per-query consent landed. **Four CONTRADICTED, two of them
substantive.**

| # | Finding | Disposition |
|---|---|---|
| **C1** | **COSE does not supply application-role domain separation.** `Sig_structure`'s context is `"Signature"`/`"Signature1"` — COSE structure types, not roles | **FIXED, security-relevant.** The author claimed it as free and then added three more signing roles on that basis, reaching **seven with no separation**. Wire §1.1 now requires a distinct `external_aad` role tag per context, reconstructed by the verifier from context rather than message content |
| **C3** | **UWB is not categorically relay-resistant.** USENIX Security 2022: physical-layer distance-reduction against deployed 802.15.4z HRP parts including Apple U1, 12 m spoofed as 0 m **without key knowledge** | **FIXED.** §6.5.6.3 downgraded from "the only relay-resistant channel" to "the strongest available, and not categorical". **No channel is categorically relay-resistant**; all proximity evidence is probabilistic. Consistent with §6.5's existing "a cost, not a primitive" framing, but it changes the confidence a policy should place in a record. `Proximity` should additionally record ranging mode and implementation, since that is what the known attacks turn on |
| C2 | `alg` in the protected header is permitted-or-external under RFC 9052, not required | **FIXED** — stated as a profile rule this protocol imposes |
| C4 | Background networking is constrained, not categorically impossible; APNs/FCM are common rather than only | **FIXED** — softened; the §7.9.4 conclusion is unchanged, since unreliable suffices |

**Partially correct, all corrected:** protobuf varint claim narrowed to what
Google's documentation actually says; `alg` as `int` restated as a profile
decision rather than a property of COSE; "~1.3 KB PQ key" scoped to ML-DSA-44;
**DNS-SD puts the endpoint in SRV, not TXT** — the catalog gains a separate
metadata field and the mapping table is corrected; MPLS does not invariably hide
hops; NFC "near-universal" softened to a deployment observation; Argon2id
"resists" softened to "raises the cost of"; IPv6 /64 cost identified as provider
policy.

**New unsourced assumption registered:** §6.5.1's claim that motion and parallax
*constitute* a liveness check. They are inputs to a PAD system, not a defence in
themselves, and the claim needs evaluation of a named algorithm against a
specified attack suite — particularly for generated video.

### Note

The reviewer correctly observed that several UNVERIFIABLE items are already
registered in §11.1 but are **still stated factually in the body**. That is a real
inconsistency: registering an assumption does not license asserting it elsewhere.
Worth a targeted sweep.


---

## Pass 0.2 second run — internal coherence (2026-08-15)

**26 findings, all valid, all fixed.** Two BLOCKING, twelve SERIOUS, twelve MINOR.

### The blocking pair

| # | Finding | Disposition |
|---|---|---|
| **14** | **A light client was told to dial its patron, but §4.3 permits light-client patrons with no static address.** Not stale text — a genuine architectural error | **FIXED.** A light client dials its **serving infra node**: the nearest infrastructure node on its patron chain, which is not necessarily its patron. **Serving and countersigning are separate roles** — a light-client patron still countersigns subnet-scoped transactions, it simply does not serve sessions. Failover follows the serving node's siblings, not the patron's |
| **11** | §7.5 called the anchor table "globally replicated" after §7.2 and §7.7.3 had made caching per-node policy | **FIXED** — stale from before the §7.7.3 correction |

### Serious findings of substance

- **7 — the eclipse-permeability claim overreached.** A witnessless, verifierless
  ceremony is valid only where `min(n/2,10)` is zero, i.e. for a subject with no
  prior counterparties. That covers §10.4's accepted risk exactly (**eclipse of a
  new joiner**), and the claim is now scoped to it. An established user who becomes
  eclipsed has a harder escape, which is noted.
- **5 — patrons do see presence records.** They store them (§6.7, §8); what they no
  longer do is countersign them. The author had conflated the two when removing
  PoP countersignature, and had drawn a false limitation on activity summaries from it.
- **21 — the archive does not provide total order.** §13B.5 claimed per-identity
  total order; the DAG provides **ancestor reachability**, which is what
  completeness arguments actually need. Corrected in both places.
- **10 — "identity" meant two things.** A keypair-bound *node identity* (§5.1) and a
  *lineage* that may have several claimants (§6.8.0.2). Both now in the vocabulary.
- **13 — merges do not invalidate anchors.** §7.3 Case 3 listed merges as a cause
  of re-resolution, contradicting §7.6.2's merge-stability, which is the entire
  reason §7.2 abandoned tier-based anchors. Case 3 rewritten around cache policy.
- **16 — payload plaintext.** §7.6.3 described current behaviour, §8 and §7.10 the
  requirement. Now consistently: **the requirement is settled, the construction is
  not**, and an implementation shipping hop encryption alone would leak.
- **17 — abuse reports** were in the generic attestation row, whose storage audience
  is participants, patrons and witnesses. Given that audience an abuse report
  becomes the public accusation §13B.1 declines to build. Now its own row: stored
  by the resource owner alone.
- **20** — §13B.3 still said branches "cannot be merged after the fact", written
  before §6.6.1.3 existed. What is impossible is merging *by editing existing
  records*; a protocol-level merge is exactly the resolution.

### Minor

Front matter listed resource permissions and discovery as unspecified hours after
they were specified (1, 2); departure described as categorically required to
become a root, ignoring disavowal (3); `n` used for query count after `q` was
reserved for it (8); three parameters "proposed" in their sections and "chosen" in
§12 (18); four stale `§6.2.1` cross-references pointing at Departure when they
meant Disavowal or Cycle prevention (23); §14's resource step carrying a sentence
about recovery adoption (24); §4.4 citing §6.3 for ASN/prefix fields §6.3 never
stated — **fixed by adding them to §6.3**, since the requirement was real and only
its home was missing (25); §11.3 saying "ten" over a table of thirteen (26).

### Observation

**Five findings (1, 2, 4, 5, 20) were introduced in the last two working days**,
several within the same session as the change that made them stale. Propagation
failure is not a function of elapsed time — it is a function of edit volume, and
these were high-volume days.


---

## Direct payload path via ICE (user, 2026-08-15)

Proposal: bring NAT traversal back so point-to-point payload connects directly
after resolution, with infra nodes as STUN and TURN. **Adopted.**

**The reframing that makes it clearly right:** this is not adding a relay, it is
adding a **bypass**. The infra node already relays payload — that is precisely
what a TURN server does. ICE makes the relay a fallback rather than the default,
so the architecture is unchanged and gains a fast path.

**Benefit the proposal did not name, and the strongest one:** §9.6 prices infra at
~$20/month, a figure never checked against relaying *all* payload for up to 1,110
subordinates. Bandwidth would plausibly have dominated it. Direct-first removes
the dominant term and makes the estimate defensible. §9.6 now states the
dependency.

**Costs recorded:**

1. **§7.9.1's "there is no NAT traversal problem" is withdrawn.** It was presented
   as a design advantage and was true only because payload was always relayed.
2. **The relay path stays first-class.** Symmetric NAT and CGNAT defeat hole
   punching, and two mobile peers is the worst case *and* the common case here.
   The relay fraction is registered in §11.1 as unsourced — if it is high, §9.6's
   economics come back into question.
3. **New privacy finding P17: a direct connection reveals each peer's IP to the
   other.** This trades "the serving node sees my communication graph" for "my
   counterparty sees my address whenever we talk", and §6.5.6 establishes IP gives
   coarse location — so a hostile counterparty who completed a ceremony gains an
   **ongoing location signal** they lack under relaying. Direct-versus-relay must
   therefore be a choice with a stated consequence; the reference client must
   permit forcing relay.

**Build order:** ICE is step 9b, explicitly after the relayed path works. Direct
is an optimisation over relay, not a replacement for it.


---

## Direct path limited to the horizon (user, 2026-08-15)

Proposal: confine direct connections to the ±2 tier horizon, relaying everything
outside it, so IPs leak only within the assumed high-trust range. **Adopted.**

**Supporting argument the proposal did not make:** the horizon is *already* the
set that holds your locator (§7.1: patron, depth, subtree) and your topology. IP
is therefore **incremental** disclosure inside the horizon and **novel** disclosure
outside it — which is precisely where the rule places the relay. It is also
trivially evaluable: "is this peer in my h=2 topology store?" needs no new state.

**Honest limits recorded:**

- **The horizon is bounded, not trusted.** ~121 nodes including cousins a user may
  never have met. A patron already sees its subordinates' metadata and gains
  nothing; siblings and cousins gain an IP they did not have. Exposure is
  *bounded*, not *chosen*, so both defaults remain overridable.
- **A2 becomes more load-bearing, not less.** Relay load is now
  *(out-of-horizon traffic) + (in-horizon traffic where traversal fails)*, so if
  most traffic is not intra-Dunbar the relay carries nearly everything and §9.6's
  economics collapse. A2 acquired its fifth dependent.

**Structural observation added to §8.1: the horizon now carries five jobs** —
topology storage, permission scope evaluability, fanout depth, catalog
propagation, and now direct-path eligibility. Each was adopted independently,
which is some evidence it is the right primitive. It also makes *h* the most
over-loaded parameter in the design, and a table now warns anyone proposing to
tune it.


---

## Pass 0.3 second run — unjustified claims (2026-08-15)

**36 findings across three severity bands** — 20 load-bearing, 9 supporting, 7
colour — against 280 undifferentiated last time. **The severity rubric added after
the first run is the reason**, and it made the output actionable rather than
merely correct.

### Structural finding: §11.1 and §11.3 are orthogonal, and were not linked

§11.1 records what is **unsourced**. §11.3 records what is **load-bearing**. A
claim can be both — and **six were**, sitting only in §11.1: the matcher
reconstruction query count, the TURN relay fraction, the PAD/liveness claim, face
entropy, the ageing regime, and infra costs.

Those six are the highest-priority items in the document, being simultaneously
unvalidated *and* structural, and nothing marked them as such. Added to §11.3 as
**A14–A19**, each cross-referencing §11.1, with the orthogonality now stated
explicitly so neither register is read as subsuming the other.

**A17 deserves separate mention:** it is the only assumption used to *reject* an
alternative rather than support a choice. "Face entropy is low enough that fuzzy
commitments have weak margins" closes off a mechanism that would retain
verification capability without retaining biometrics. If it is false, the whole
retention design could change.

### New load-bearing claim, and it may be false

**§9.3 called peering "the cheapest social-engineering route to raising one's own
trust ceiling."** Checked: an **unattested adoption** (§6.1.1) requires no physical
meeting and no storage commitment, so it is plausibly cheaper. **The superlative is
withdrawn.** The mitigation is unaffected — low default flow capacity is justified
by peering being cheap *and not looking like an endorsement*, which does not
require it to be cheapest. Registered as **A20**.

### Supporting and colour

Six supporting-tier claims added to §11.1 with their severity noted: cloud
deployment concentration, mutual-adoption likelihood, early network size, the
short-lived-credential industry trend, new-identity-versus-recovery cost, and
0-RTT battery savings.

All seven colour findings fixed by applying §11.4's standing rule — replacing the
adjective with the figure or the mechanism. "Negligible" became "under 10 MB
lifetime"; "comfortably cacheable" became "~7.2 MB"; "trivially spoofed" became
"received signal strength is attacker-controllable"; "availability is free" became
"the subject is already present"; and collusion rings are now described by their
actual property — **no structural signature** — with the cost noted as unmodelled.


---

## Peering / trust ceiling — framing withdrawn (user, 2026-08-15)

The user rejected not just the superlative but **the whole "raising one's trust
ceiling" framing**: trust is local, a peer trusts you only to whatever their own
policy implies, nobody elsewhere is compelled to follow, and peering history is
not visible outside the peers' local subnets.

**Correct, and it collapses the concern.** Tracing the flow metric: a peering edge
raises the attacker's capacity toward observer O only if O **can see the edge**,
has capacity to the peer, and weights peering non-zero. Peering is topology-class
and propagates within horizon (§8), so it is visible in the two peers' horizons and
nowhere else.

So the attack was never "raise my standing". It is *"gain standing with a
particular victim via a technical favour from someone in that victim's
neighbourhood"* — which requires the attacker to already be local to the target, at
which point the target can evaluate them by other means.

**What survives** is that *"want to back each other up?"* reads as a routine
technical request rather than an endorsement, so a peer may extend credit they did
not intend. That is a local misreading, and the low default flow capacity addresses
exactly it. **The mitigation was right; the justification was wrong.**

### New principle: §9.3.1 — the min-cut bound is observer-relative

Falls out of the above and was stated nowhere.

§9.2 and §10.3 described a region's cut as a property of the graph. **It is a
property of the graph the observer can see.** Horizon-limited propagation means
different observers hold different edge sets and compute different cuts for the
same region.

- The **Sybil bound of §10.3 is per-observer**, like everything else here.
- **Invisibility is conservative**: an unseen edge cannot inflate a claim, only
  fail to support one — the safe direction for the error to run.
- **An attacker must work per-target**, acquiring visible edges inside each
  intended victim's horizon rather than accumulating them globally. Strictly more
  expensive than the global reading implied.

§10.3 now states this before its three Sybil mechanisms, since all three inherit
it. Another instance of §1.1: what an observer cannot see cannot bind them.


---

## Pass 0.4 second run — parameter inventory (2026-08-15)

**138 numeric parameters across both documents, 30 UNSET.** Run jointly over the
design and wire format, which is what surfaced the cross-document conflicts.

| Finding | Disposition |
|---|---|
| **Presence record size**: design ~35 KB at ML-DSA-65, wire ~40 KB | **FIXED** in the wire format. Now carries both a typical figure (~10 signers, ~35 KB) and a maximum (~34 signers, ~95 KB), which the single number was conflating |
| **"All five transaction types"** in the design vs seven in the wire format | **FIXED.** Resource registration and abuse report were added 2026-08-15 and the count was never updated |
| **Bound incompatibility**: envelope signers capped at 12, witnesses alone permitted 16 | **FIXED structurally.** The envelope signer bound is now **derived** from the sum of per-role bounds rather than asserted independently — 2 participants + 16 witnesses + 16 verifiers = 34. A record the witness bound allowed was previously rejected by the signer bound |
| **"Every array is bounded"** stated in wire §1, but eleven arrays had no maximum | **FIXED.** Bounds added for merge back-pointers, verifier responses, asserted locations, corroborations, proximity channels, explicit-scope lists, veto-delegation type lists, NetworkPoint lists, SiblingRefs and peering audit history |
| **30 UNSET parameters scattered across sixteen sections** | **CONSOLIDATED** into new §12.1, grouped by what settling each requires: a security argument, measurement under load, a policy decision, or an encoding decision |
| **"Current-keys assertion" and "currency attestation" are one object** under two names — the inventory listed their TTLs separately | **FIXED.** Names unified; the duplication had made a single object look like two with independent lifetimes |

### Observations

**Joint review found what single-document review could not.** Both conflicts and
the bound incompatibility are cross-document or cross-section; neither file alone
is inconsistent. §0.4 should be run over both files together from now on.

**The naming duplication is the subtler find.** Two names for one object did not
merely read badly — it produced two separate entries in a parameter inventory,
each with its own unset TTL, which is how a single object acquires two divergent
lifetimes in an implementation.

**§12.1's grouping is the useful output**, more than the list. "Needs a security
argument" and "needs measurement under load" are different kinds of work with
different owners, and five parameters in the first group must not be tuned for
convenience — the distinction §7.6.4 already makes for the currency lifetime,
now applied to every open value.


---

## Pass 0.5.1 second run — rule fragility (2026-08-15)

**Six residual identifier-coupled rules, all fixed.** The reviewer confirmed six
previously-fixed instances now carry proper role-level statements — formation
subtype, `nominated_by`, verifier-selection seeding, the departure veto, the
locator signature, and trust-bearing operations — so the convention introduced in
§0 is holding where it was applied.

| # | Rule | Fix |
|---|---|---|
| 1 | §5.1 signing tier stated as a list of transaction names | Restated: *any signed object whose authenticity must remain security-relevant on the post-quantum timescale, and any operation changing the identity being authenticated, requires both components.* **See below — the rewrite exposed an error** |
| 2 | §6.7 "delivery of a presence record MUST carry the nonce" | Generalised to any evidence delivered in response to a request; §8 already had the general form |
| 3 | §6.9.5 `discover_scope` filtering | Restated as a disclosure rule for any party relaying discovery metadata |
| 4 | §6.9.6 abuse report "addressed to the owner" | Restated: a signed complaint is disclosed only to the party with authority to act on it, and intermediaries must not retain it as reputation evidence |
| 5 | §7.7.5 "peering must remain optional" | Restated: creating an identity or an ordinary authority relationship must not depend on cooperation from an unrelated infrastructure operator |
| 6 | §7.8.1 bootstrap ordering | Restated in terms of creating a disconnected authority domain rather than in terms of named steps |

### The consequential finding: §5's tiering was invalidated by the archive chain

Generalising rule 1 exposed that **§5's justification for classical signatures on
routine transactions no longer holds.** It read: *"signatures need only hold until
nobody relies on them."* §6.6.1 subsequently made the archive a hash chain, and
**verifying a history means verifying the signatures on its records** — so an
evaluator checking a five-year prefix relies on five-year-old signatures.
**Reliance does not expire while the archive is presentable.**

Consequence: once classical signatures are forgeable, §6.6.1's guarantee that a
record's *position* cannot be altered fails, because fabricating a record requires
only forging its counterparty's signature. **A PQ-signed archive head does not
rescue it** — that proves the holder asserts a chain, not that counterparties
signed its records.

**Resolution: every archive-retained transaction is PQ-signed**, which is all of
them. Cost is ~8 KB per transaction against a photo store already at ~100 MB. The
tiering simplifies: archive-retained objects are PQ, session-layer traffic is
classical.

**Shape worth noting.** Every prior propagation failure ran forwards — a decision
made, then not carried into dependent sections. This one runs **backwards**: a
later decision silently invalidated the *justification* for an earlier one, while
leaving the earlier text looking correct. Nothing about §5 read as stale, because
its words were unchanged; only its reasoning had been undermined. **Greping for
changed terms cannot catch this class.**


---

## Pass 0.5.2 second run — unenforceable mandates (2026-08-15)

**Two findings**, against nine in the first run. §1.1 was added between runs and
is largely holding — the reviewer explicitly cleared eight categories that look
suspicious but stay inside the boundary, including the verifier-authorisation
rule, the request nonce, the departure veto, peering optionality, and end-to-end
encryption (source-enforced by withholding the key).

**Both survivors were written today, after §1.1 existed.**

| # | Finding | Disposition |
|---|---|---|
| **§9.2** | States §1.1 correctly — *"the spec cannot compel a foreign implementation's choice of λ"* — then **seven lines later** mandates *"decay per hop must be steeper than the reciprocal of the fanout"* | **FIXED.** The soundness condition is restated as **a fact about the arithmetic**: a metric that does not decay steeply enough **diverges**, true whether or not anyone is instructed. The policy descriptor is the §1.1 artifact — publish λ and the resistance bound, let others decline to rely on unsound policies |
| **§6.9.6** | *"Intermediaries and unrelated trust participants must neither receive nor retain it as reputation evidence"* — governs foreign storage | **FIXED by splitting.** Enforceable at the sender: addressed to the authority, never broadcast. Not enforceable and not attempted: what a party does with a copy obtained anyway. Visible distinction: the report names the resource, so any recipient can determine it is not the intended authority |

### The conventions can pull against each other

**§6.9.6's violation was introduced by the §0 role-generalisation earlier the same
day.** The original — *"addressed to the owner and not broadcast"* — was a delivery
property the sender controls completely. Generalising it produced a rule about
foreign storage. **The generalisation was more elegant and less true.**

§0 now says the two conventions must be applied in order: state the property, then
**ask who would enforce the restated version against whom**. If the answer has
changed, split the rule — enforceable part as a MUST, remainder as a visible
distinction. The review plan now recommends running 0.5.2 immediately after any
pass that produced role-level rewrites.

Note the document already contained the correct limit: §6.5.4 says off-protocol
disclosure of biometric information cannot be prevented. The abuse-report rule
imposed the opposite three sections away.


---

## Pass 0.7 second run — LINDDUN (2026-08-15)

Full re-run over **28 flows and stored artifacts**, discharging the coverage
caveat left when the resource layer, payload confidentiality, archive merges and
per-query consent were specified after the first pass. Returned **31 threats and
10 correlation findings**.

**The individual threats mapped almost entirely onto P1–P17.** Four were new and
are registered as P18–P21: verification queries disclosing a *current* encounter to
prior counterparties; archive presentation handing a new patron an intelligible
relationship history; resource access logs binding identity to application
actions; and coarse location becoming behavioural location under temporal
correlation.

**The new material was overwhelmingly compositional**, which is the design's own
predicted failure mode, and is now recorded in a new **§10.5.8 correlation
register** — the place §10.5.1's invariant demanded and the document did not have.

### C4 is the most instructive finding of the pass

**Archive prefix + the receiving patron's local topology.** The same archive means
different things to different observers: a counterparty opaque in the old subnet
may be a known person in the new one. So **moving an archive changes which entries
are legible**, and archive portability — which §9.7 treats as unambiguously good
for the user — carries a **destination-dependent** privacy cost.

Every other correlation entry composes artifacts held by one observer. **C4
composes an artifact with the observer's own knowledge**, so the disclosure depends
on *who is receiving* rather than on what is sent. That shape was not anticipated
by the composition invariant as written.

### C9 is the one to design against

Local face archive + transaction archive on one device yields a
**face-to-key-to-social-history database**. Either store alone is far less
sensitive. The architecture works hard to stop any *network* party joining those
three; a single compromised device does all three at once. This is the central
consequence of P5 rather than a separate risk.

### Implementation-blocking stale rule, fixed

The wire format still said **"No NAT traversal … No STUN, TURN, hole punching or
relay fallback"** after the design adopted ICE the previous day. An implementer
following it literally would have built a different metadata topology and would not
have implemented P17's exposure boundary. Fixed, with the old text's scope recorded.

### Reviewer self-correction, accepted

The reviewer initially described activists, dissidents and people hiding from a
former partner as the design's *intended userbase*, then corrected it: the document
presents them as populations for whom composition risk is a **new** exposure rather
than a matched one, not as the target. That matches §10.5.1's V6 footer exactly. No
document change needed.


---

## Pass 0.6 second run — implementation attempt, adoption (2026-08-15)

**Result: a fully conforming adoption signer could not be written.** Sixteen
UNSPECIFIED questions, two BLOCKING, and **three internal contradictions where two
normative statements cannot both be satisfied.**

### The blocking pair — introduced the same day

**§5.1's "both components sign every archive-retained transaction" is incompatible
with the wire format's exactly-two-signers rule for adoption.** The design was
changed in pass 0.5.1 earlier the same day and the wire format was never checked
for whether it could express the result. Two normative statements, mutually
unsatisfiable — the worst thing an implementation attempt can find and precisely
what it exists to find.

| # | Finding | Disposition |
|---|---|---|
| 1 | `KeyMaterial = COSE_Key` (singular) cannot represent a hybrid identity whose keyhash covers both components | **FIXED.** `KeyMaterial = [COSE_Key, COSE_Key]`, a COSE_KeySet in fixed order — classical first. Order is fixed because reordering changes the keyhash and therefore the identity. **ML-DSA-65 selected** as the profile parameter set; naming three without choosing one is not implementable |
| 2 | No encoding exists for "one logical signer signs with two component keys" | **FIXED.** Signer rules now count **logical signers, not entries**: each contributes two `COSE_Signature` entries sharing a `kid` (the identity keyhash) and distinguished by `alg`. Adoption therefore carries two logical signers and **four entries**. Rejected the composite-algorithm alternative — it needs an algorithm the COSE registry does not define, no standard tooling could verify it, and independent component verification supports §5's staged migration |

### The decision that keeps this affordable

**Evidence embedded inside a signed body stays classical.** Verifier responses,
subject countersignatures and old-key proofs sit *inside* a body the envelope
signature covers, so substituting any of them breaks that signature. Their
authenticity is protected transitively and needs to hold only until the receiving
party decides; afterwards what a future evaluator relies on is *"the patron
accepted this"*, attested by the patron's own hybrid signature.

Without this limit a recovery adoption carrying 32 verifier responses would exceed
**400 KB**. With it, the envelope is ~8 KB.

### Three internal contradictions, all fixed

| Contradiction | Resolution |
|---|---|
| `uint .size 8` reads as fixed eight-byte serialisation, contradicting §1's shortest-form rule — a 2026 timestamp encodes in five bytes | `.size 8` removed; these are **range** constraints, encoding is always shortest-form |
| Recovery field 2 "may be empty" vs §1's "empty arrays MUST be omitted" | Field 2 is **optional; absence means empty**; present-but-empty is malformed. Otherwise one implementation emits `[]` and the other rejects it as non-canonical, each rejecting the other's valid encoding |
| Back-pointer bound of 8 vs "decoder MUST accept lists of any length ≥ 1" — **the author's own contradiction**, written when merges were added | "Any length from 1 to 8" |

### Also fixed

Signer `kid` profiled as the 32-byte identity keyhash; **`VerificationQuery`
defined** so the two embedded signature payloads are specified (previously
unimplementable — a verifier could not know what to verify); old-key proof changed
from `COSE_Sign1` to `COSE_Sign` since the old identity is also hybrid; query_id
and back-pointers fixed at 32 bytes; timestamps explicitly not checked against a
local clock, since differing skew windows would make one object valid for one party
and invalid for another; field 5 "on first contact" clarified as never a validity
condition, because recipient state cannot determine a signed object's validity.

### Still blocking

**Adoption field 7 (archive references) remains `[P]`.** It is part of the adoption
body, so the transaction cannot be finalised while its encoding is unagreed — the
choice between references, a head txid, a range, and a Merkle proof is
wire-incompatible in every direction.

### Crate maturity

Confirms the design's §5.2 note. `ml-dsa` 0.1.1 and `ml-kem` 0.3.2 implement final
FIPS 204/203, are pure Rust and `no_std`, and **both explicitly disclaim
independent audit**. Alternatives (`fips203`/`fips204`) target WASM but carry the
same caveat. wasm32 is technically viable; the entropy path needs testing on the
actual target rather than inferring from `no_std`.


---

## WASM storage claim corrected (user, 2026-08-15)

The author said a WASM target "has no direct filesystem access" and used it to
question the target choice. **That is a browser-specific claim stated as a property
of WebAssembly, and it is false in general.**

- **WASI** (`wasip1`/`wasip2`) provides POSIX-like filesystem calls; a native WASM
  binary under Wasmtime, Wasmer, WasmEdge or WAMR has ordinary file access.
- **A WASM component inside a native application** gets whatever host bindings the
  host provides.
- **Even in a browser**, OPFS gives real random-access file handles with
  synchronous access in workers — better suited to a photo archive than IndexedDB.

**What survives the correction, and is the point worth keeping: durability differs
by host.** Browser storage is evictable under storage pressure unless persistent
storage is granted; a native filesystem is not. §6.6.1.1 makes the archive a
**second factor** and A13 assumes users retain it, so **silent eviction costs a
security property**, not merely convenience. A browser-hosted client therefore
needs §7.8.7.1's backup path to be mandatory rather than advisory.

Corrected in both the review plan and §13B.3. The original claim had been carried
forward unexamined since the 0.6 planning discussion.


---

## Pass 0.6 target 2 — resolve a locator (2026-08-15)

Run against a slightly earlier document version; **all findings verified still
applicable**, since nothing in the intervening WASM correction touches resolution.

### The headline is larger than target 1's

**The resolution request/response protocol did not exist.** The design describes
resolution narratively — §7.3's five cases, §7.6.1's self-routing — and the wire
format had no message for it. **The network's primary operation was
unimplementable**, and no reading pass had noticed, because the narrative reads as
complete.

Target 1 found two normative statements that could not both hold. Target 2 found
an operation with no wire representation at all. Different failure, same cause:
prose that describes behaviour convincingly enough that nobody checks whether a
message exists to carry it.

| Finding | Disposition |
|---|---|
| **No resolution protocol** | **FIXED.** New wire §5.6: `ResolveRequest`, `ResolveReply` with serving/forwarded/failure variants, `ServingInfra`, failure codes, a 4-repair bound against hostile chains, and the rule that a forwarded reply must name the same subject with a strictly greater seqno |
| **Locator authentication contradiction** — design §7.1 says the locator is signed by the node; wire §2.3 said it is "always transmitted inside a signed envelope" | **FIXED.** Both are right for different carriage. Inside a transaction a bare `Locator` suffices; **standalone — at introduction or in a repair — a `SignedLocator` is required**, and a bare locator presented alone MUST be rejected. Classical signature, since a locator's relevance expires when the node moves |
| **How does descent traverse light-client tiers?** (#14 — architectural) | **RESOLVED, and it was implicit.** Descent is **through infra only**, terminating at the serving infra node, **returning the residual path suffix rather than traversing it**. The serving node uses the suffix to identify which attached client is meant. This is what lets a path address a light client nothing can route to directly. Now in design §7.6.1 as well |
| **Anchor entries are unauthenticated** (#17) | **FIXED.** Self-signed by the anchor. The table is an index rather than a credential store, so an entry vouches for nothing — but an unsigned entry lets any gossip peer inject addresses for a node it does not control, which **partitions rather than impersonates**. Cheap to prevent |
| `subtree_size` range (#6) | **FIXED** — u64. The design's ~4-byte sizing arithmetic is a storage estimate, not a validity bound; a `u32` implementation would reject values another accepts |
| ASN range (#7) | **FIXED** — u32 per RFC 6793 |
| Resolution failure vocabulary (#9) | **FIXED** — four codes, so a resolver can distinguish retry from re-resolve |
| Repair bound (#10) | **FIXED** at 4. §7.3 requires the source to return the *terminal* forwarding record, so a well-behaved chain never exceeds one; the bound is against a hostile peer |
| Forwarding subject check (#12) | **FIXED** — a forwarded reply MUST name the same subject, else the repair is a redirection attack |

### Not fixed

`routable_prefix` byte format (#5) remains opaque; multi-`NetworkPoint` selection
policy (#11) is deliberately local; truncated-path region semantics (#13) still
need the design's Case 4 spelled out against the new protocol.


---

## Pass 0.6 target 3 — validate a presence record (2026-08-15)

**30 UNSPECIFIED questions.** The finalization arithmetic was implementable; the
**verifier-selection invariant was not.**

### The headline: selection could not be recomputed from the record

§6.6.2 requires that selection be recomputable by any third party **so that a
missing verifier is visible** — the anti-suppression property the entire verifier
design exists to provide. But the witness nonce commitments and reveals it depends
on **were nowhere in the presence record**. A validator holding a complete, valid
record could not check which verifiers should have been asked.

The design had specified the *security properties* of the seed — depends on the
participant pair, a coarse window, witness contributions; must not be determinable
from participant-controlled values — and none of the *mechanism*.

**Now specified in `wire-format.md` §4.6:**

| Component | Decision |
|---|---|
| Nonce carriage | `Witness` gains commitment and revealed nonce, both 32 bytes |
| Commitment | `SHA-256("rhtn/1:nonce-commit" \|\| witness \|\| nonce)` — domain-separated and identity-bound so it cannot be replayed by another witness |
| Seed | Domain tag, canonically ordered participants, window ordinal big-endian, then witness/nonce pairs in keyhash order. **Participant order canonicalised** so which party is listed first cannot become a grinding variable |
| Window | **24 hours, epoch-aligned** — fills the §12 UNSET. Long is safe: honest retry inside the window reproduces the *same* sample, while an aborting attacker gets one fresh sample per day per pair |
| Sampling | Hash rank `SHA-256(seed \|\| subject \|\| candidate)`, ascending, ties by keyhash |
| Candidates | **Distinct counterparties, deduplicated.** The threshold counts transactions, the candidate set counts people |
| Threshold | `min(floor(n/2), 10, \|candidates\|)`. **The third term was missing and is necessary** — twenty meetings with one person would otherwise demand ten verifiers from a pool of one. Floor stated explicitly; unstated rounding differs by one at every odd *n* |

### Two judgement calls worth flagging

**`pending` and `unavailable` count toward finalization.** Finalization is
structural; response *content* is evidence for policy. The alternative hands an
attacker who can make verifiers unreachable a way to block finalization
indefinitely, and a record finalized on ten `pending` responses is visibly weak to
any evaluator — which is the right place for that weakness to live.

**Consent signs `query_id`, not the query.** `query_id = SHA-256(canonical query)`,
so the signature remains verifiable from the record while the query — carrying a
fuzzed profile up to 4 KB — is not carried. Embedding it would add up to 64 KB to a
sixteen-response record for no verification benefit, since the id already commits
to the profile.

### Structural rules that were simply absent

Participants must differ; `finalized_at` ≥ `started_at`; **a record on the wire is
always final**, so an unmet threshold is local state and never published;
`strongest` must pass **and no higher-ranked channel may pass**, without which the
field is non-deterministic; formation records omit rather than empty their witness
and verifier arrays; **one identity is one logical signer regardless of roles**, so
the 2+16+16 bound counts distinct identities rather than role slots.

`duration_s` **removed** — it duplicated `finalized_at - started_at` and could
disagree with it.


---

## Verifier recomputability — direction corrected (user, 2026-08-15)

The user corrected their own prior statement, and the correction changes who the
property protects.

**A selects verifiers from B's history**, because A is establishing that B is B.
The party who *performs* a selection is never the party it is *about*.

So the recomputation that matters is **B's**: B must be able to demonstrate that
B's own verification was honestly conducted, from the record plus B's own history.
The author had framed this evaluator-centrically — *"an evaluator holding A's
history can check A's half"* — which points at the wrong party.

**Why the subject-centric framing is better: it names the concrete harm.** If the
seed is not reconstructable from B's own holdings, **B can never clear themselves**.
Not "an evaluator is inconvenienced" — B is permanently unable to prove something
about B, and the record is immutable, so nothing repairs it later.

### New ceremony step, previously unstated

**Each party MUST verify the other's selection before signing.** If A selects B's
verifiers off-seed and B signs regardless, B is left holding a record that fails
recomputation forever.

**This is the only check in the ceremony that protects a signer against their
counterparty.** Every other check guards against outsiders, or against the pair
colluding against third parties. This one guards a participant against the person
in front of them — a threat direction the ceremony design had not addressed at all,
because the whole mechanism is built around what A and B jointly assert to others.

Added as ceremony step 7 (§6.5.1) and as a normative rule in `wire-format.md`
§4.6.2.


---

## Pass 0.6 target 4 — client attach with sibling failover (2026-08-15)

14 findings. Two were unsatisfiable rather than merely undefined.

| # | Finding | Disposition |
|---|---|---|
| **13** | **A zero-sibling topology had no valid encoding.** `AttachAck` required `[ + SiblingRef ]` while §1 forbids encoding an empty array, and the field was not optional — so an infra node with one child, a legal topology, could not produce a conforming `AttachAck` | **FIXED** — field optional, absence means no siblings |
| **1** | **"Attachment to a sibling is a degraded state and must be explicit"** (design §7.9.2) — and nothing on the wire carried it | **FIXED.** `Attach` field 4 names the client's **intended serving node**; the receiver compares it to its own identity. `AttachAck` field 1 echoes the determination, so a disagreement surfaces immediately rather than one side permitting trust-bearing operations the other considers unavailable |
| **14** | **The PQ transport handshake was not specified**, and no ML-KEM parameter set was chosen | **FIXED, with a standard rather than an invention.** QUIC + TLS 1.3 with group **`X25519MLKEM768`** — the deployed industry hybrid, surviving either primitive's failure. **Peer auth by raw public key (RFC 7250)**, since identities are keyhashes and there is no CA to issue chains. ML-KEM-768 as the profile set, matching ML-DSA-65's level |
| 2 | No port in `NetworkPoint`, so a sibling reference could not become a socket address | **FIXED** — optional port field, default 7431/udp |
| 4 | Heartbeat interval had no unit — "catastrophic for liveness" if one side reads milliseconds | **FIXED** — seconds |
| 6 | `Heartbeat.seqno` ambiguous between locator seqno and heartbeat counter | **FIXED** — a **per-session counter from 0**. A heartbeat needs gap detection; a locator seqno changes only on position change and would be constant across beats |
| 5 | "Queue depth" undefined | **FIXED** — messages currently queued for this client, advisory |
| 7 | No `NetworkPoint` bound inside `SiblingRef` | **FIXED** — `1*8`, matching anchors and peering |
| 10 | No heartbeat failure threshold | **FIXED** — 3 consecutive missed intervals |
| 11 | Failback behaviour undefined | **FIXED** — none automatic; the client stays on the sibling until that session ends. Probing the primary from a degraded session adds traffic for a state the client leaves at the next natural reattachment anyway |
| 12 | Stream 0 carries "sibling updates" with no frame defined | **FIXED** — `SiblingUpdate`, replacing the cached list entirely |
| 8, 9 | Sibling and endpoint ordering | **ACCEPTED as behavioural** — listed order is preference; not a wire incompatibility |
| 3 | Capability set still a placeholder | **DEFERRED** — unchanged |

### Observation

The transport finding is the one worth noting for its shape: unlike most gaps in
these passes, **it had a correct answer already available externally**. Hybrid
X25519MLKEM768 is standardised and deployed, and RFC 7250 raw public keys fit a
design whose identities are keyhashes rather than certificate subjects. The gap
existed because nobody had asked the question, not because the answer was hard.


---

## Degraded-attachment "explicit" — misread corrected (user, 2026-08-15)

The author read design §7.9.2's *"attachment to a sibling is a degraded state and
must be explicit"* as a **wire** requirement and added an `Attach` field for the
client to assert its intended serving node. **It was a UI requirement** — the user
must be told they are degraded — from the earlier session-establishment discussion.

**The supporting argument was also wrong.** The author claimed a sibling might
believe it was providing primary service and permit trust-bearing operations the
client considered unavailable. It cannot: a client attaching to a sibling sits two
hops away in topology the sibling holds within its horizon, so *"is this client in
my subtree?"* settles it locally.

**Kept, for a better reason:** `AttachAck` field 1 reports the **server's**
determination, because **the client may not know**. A client whose serving node
changed — its patron grew into infra — can believe it is reaching its primary when
it is not. The determination belongs with the node that has authority over its own
subtree; the client is informed rather than asserting.

`Attach` field 4 removed. §7.9.2 now states the UI obligation explicitly, as a
reference-client requirement of the same kind as §10.5.6's.

### Methodological note on pass 0.6

**The implementation pass is biased toward adding wire fields**, because that is
the shape of its output. It asks *"what carries this requirement?"*, and when the
answer is *"nothing"*, the reflex is to add a field — even when the correct answer
is that the requirement is not a wire requirement at all.

Guard for future runs: before adding a field to satisfy a stated requirement, ask
whether the requirement is about **what the protocol transmits**, **what an
implementation does**, or **what a user is shown**. Only the first needs a field.
Added to the review plan.


---

## Pass 0.7 third run — LINDDUN (2026-08-15)

Independent walk over 42 flows and stores. **Most findings mapped to the existing
register**, which is the expected result for a re-run and evidence the register is
holding. Four items were new.

| Finding | Disposition |
|---|---|
| **C11 — prekey fetch + queue/routing metadata** | **ADDED.** A prekey is fetched before any message exists, so the patron sees the fetch and then either sees traffic or does not. **Communications that never happened become visible** — a relationship that produced nothing still leaves a trace, and aborted or reconsidered contact is precisely what people assume is private. The prekey leak was registered; this join was not |
| **C12 — verification-query log + the subject's own archive** | **ADDED.** The query log exists as a *defence*, letting a subject see who is probing them — but retained beside the archive it is an auxiliary timeline of ceremony attempts including ones abandoned before a record existed. **A privacy mechanism becomes an attack surface under endpoint compromise**, and it records events the network never learned about |
| **C13 — disavowal reason code + resource history** | **ADDED.** §6.2.2 enumerates reasons rather than allowing free text precisely to avoid a defamation surface. But `policy_violation` beside a revoked clinical-resource grant on a known date reconstructs the specifics the enumeration withheld. **Context supplies the semantics the code omits**, partially defeating the mitigation |
| **U2/NC4 — witnesses and verifiers get no disclosure** | **ACCEPTED as P23**, and it is a scope error rather than an omission. §10.5.6's capture-time obligation covered *participants*. A verifier who answers permanently proves they previously met the subject; a witness proves neighbourhood involvement. **The consent machinery protects the subject of a query while asking nothing of the verifier, whose own relationship is what the response exposes.** Extended as a reference-client obligation |
| Query-log retention unspecified | **ACCEPTED as P22**, added to §12.1's unset list |
| NC2 — immutability vs erasure rights | **ACCEPTED** — added to §10.5.7 as accepted cost 8b, noting it is a **compliance posture** likely needing a documented lawful basis for immutable evidence rather than a technical deletion mechanism, and that it is less developed than the biometric analysis beside it |

### Observations

**C12 is the shape worth remembering: a defence that becomes an exposure.** The
query log was added to make oracle probing visible to its target. Under endpoint
compromise it hands an attacker a record of ceremony attempts the network never
saw. Nothing was wrong with adding it; the composition was never checked.

**The reviewer's summary of the structural tension is accurate and worth keeping in
view:** this network makes signed, durable, socially meaningful evidence its
principal security primitive, and those properties score badly under Linking,
Non-repudiation and Unintervenability **by construction**. The architecture can
only stay privacy-preserving through audience restriction, minimisation and
selective disclosure — not by making evidence unlinkable after sharing. The three
places where the stated model and a deployable one diverge most: selective
disclosure for presence records (§10.5.3, proposed not adopted), queue lifecycle
(§7.9.6, open), and the payload encryption construction (§7.10.4, undesigned).


---

## Author working session — §18.2 dispositions (2026-08-25)

**Not a review pass.** The author took §18.2's open list item by item and asked for a
proposed decision on each, argued from consistency with the rest of the set. Recorded
here because the dispositions are decisions, and because three items changed shape
once checked against the documents rather than against §18.2's summary of them.

**References here are as-of-filing**, per this file's standing rule.

### Dispositions

| # | Item | Disposition |
|---|---|---|
| 1 | Topology-class propagation has no message | **SPECIFIED** — `wire-format.md` §7.2a. Stream-0 frame type 7; forward-if-stored; `txid` dedup; no ack or retry |
| 2 | Rootward reach of the topology class (§12's *ancestors*) | **DECIDED by the author** — a minified memo of membership changes only, to the subnet root. Design §12.2, `wire-format.md` §7.2b |
| 3 | Cycle prevention beyond the interim rule | **CLOSED** — the memo's in-path check answers the partial-information case. Interim rule kept as the cheaper first line |
| 4 | An infra child's endpoints have no path to its patron | **CLOSED** — `wire-format.md` §5.3d node endpoint record |
| 5 | Queue ceiling behaviour | **DECIDED** — refuse the newest, tell the sender |
| 6 | Queue crash-recovery copies | **DECIDED** — none outlive delivery |
| 7 | Queue operator logging | **DECIDED** — commitment in `infra-client-requirements.md`, not a protocol rule |
| 8 | Owner movement | **NOT-A-FINDING as stated** — the general rule already existed in `resource-requirements.md` §7.1/§7.1.1. Relocated to design §9.2 |
| 9 | What a resource may log (P20) | **PARTIALLY ADDRESSED** — optional `data_practice` declaration on `CatalogEntry`. **P20 stays open**: visibility is not a limit |
| 10 | Whether an abuse report relates to a live session | **CLOSED — no.** Standalone transaction |
| 11 | Gateway operator evaluation (P24) | **STILL OPEN** — `data_practice` helps a user who already has access, not one deciding whether to acquire it |
| 12 | Keystream channel and size | **NOT-A-FINDING** — closed at §7.1.5.2 since 2026-08-23 and carried in §18.2 as open |

### Three items changed shape on checking

**`SignedLocator` does not bind identity to endpoints.** Three sections said it did —
design §18.2, §10.6.1 and `infra-client-requirements.md` §4.4. A `Locator` is
`{anchor, path, seqno}`. The object nominated as the natural carrier could not carry
the thing, and the gap was narrower than stated because peering records already carry
both endpoints' addresses.

**Owner movement was specified, in the document class §0 says carries no protocol
facts.** It read as unwritten for nine days for that reason alone.

**The keystream channel question was closed and still listed.** §7.1.5.2 records it as
a problem that existed only while the mechanism was described in terms of the expanded
stream.

### What closing them cost

New findings **P35** and **P36**, new correlation entry **C19**, an amended product
property at design §10.4, and one withdrawal whose reasoning had to be restated —
**P8** rested partly on *the attacker is already a horizon member*, which stopped being
true for ancestors. **P15 carried the same clause** and was corrected with it, which is
the sweep-by-search rule working as intended.

### Observation on the method

**Three of twelve items dissolved rather than resolving**, and all three dissolved
against the *source* sections rather than against §18.2's summary of them. §18.2 is the
section this project's own notes predict will decay fastest, and it had drifted from
four of the six registers it consolidates. **The consolidating section is not a
substitute for the registers when deciding anything** — it is a reader's index, and it
was being used as a work list.


---
## Author working session — presence record scope (2026-08-25)

**References as-of-filing**, per this file's standing rule.

| # | Item | Disposition |
|---|---|---|
| 1 | `Participant.locator` | **REMOVED.** Nothing reads it; sweep of eleven consuming exchanges recorded at design §7.2.1 |
| 2 | Selective disclosure (§14.5.3, proposed 14 days) | **ADOPTED, rescoped.** Salted digest list, not a Merkle tree. Design §7.2.1, `wire-format.md` §4.5.1–2 |
| 3 | Merkle tree vs digest list | **FIXED-DIFFERENTLY.** At nine leaves a tree adds hazards and saves nothing |
| 4 | "Only lever that addresses composition directly" | **NOT-A-FINDING → corrected.** Wrong in both halves; P2 and C2 are untouched by any field-level measure |
| 5 | Over-asking hazard (drafter-raised) | **WITHDRAWN by the author.** Policy is pluggable; interfaces are not (§1.1). Coercing a degraded client is out-of-protocol |
| 6 | `AbuseReport.reporter` | **REMOVED.** Synonymous with `resource`; signer-binding MUST added in its place |

### The finding worth carrying forward

**The legitimate use of location and the leak are the same computation.** §7.1.7's
impossible-travel check and P21 both require a *series* of coarse geohashes; neither
works on one. So no field-level mechanism can separate them — it can only choose who
receives the series. That was not visible until the consuming exchanges were enumerated,
and it is the reason §7.2.1 states the trade rather than claiming a mitigation.

### Method note

**The sweep changed the answer twice.** Asked to cost a mechanism, enumerating its
consumers found (a) a field with no consumer at all, better deleted than made optional,
and (b) that the mechanism could not do the thing it was proposed for. **Costing a
proposal by enumerating who consumes the data is cheaper than building it**, and it is
the second time in two days that a §18.2 item dissolved against its source sections
rather than against the summary.


---

## Pass 0.6.1 — implementation attempt, adoption (clean room, 2026-08-26)

**Seven UNSPECIFIED items, all seven confirmed against the text.** No false
positives — the first pass of this size with none.

| # | Finding | Disposition |
|---|---|---|
| U1 | Adoption field 4 `timestamp` has a range but no stated event semantics | **FIXED** — author's rule: a transaction's timestamp is **when it takes effect**. Stated at the type definition in §1 so it governs every transaction type, not adoption alone |
| U2 | Back-pointer lists ordered "as the required signer set", which is unordered | **FIXED** — signer order is the order each type's schema introduces its required signers, now enumerated per type in §3.1. Author: no practical valence either way, but there must be a rule |
| U3 | `rhtn/1:recovery` orphaned in the domain-separation table | **FIXED** — row deleted. Residue from when field 3 signed the Recovery map, corrected 2026-08-24 |
| U4 | Field 9 is `COSE_Sign1` and must be hybrid — jointly unsatisfiable | **FIXED** — field 9 is `COSE_Sign1` in a presence record and hybrid `COSE_Sign` inside a Recovery. **Was blocking for recovery interoperability** |
| U5 | Which signature hybridises — field 7, field 9, or both | **FIXED** — field 9 only, stated explicitly rather than derivable from a KB figure |
| U6 | "Signs `query_id`" — raw bytes or CBOR bstr | **FIXED** — raw 32 bytes. Every other payload in the document names its encoding; this one alone did not |
| U7 | The 1024-byte extension bound does not say what it measures | **FIXED** — complete encoded CBOR slice for the value |

### What the reviewer supplied and what was added

**U5's cost arithmetic was right and is not the best argument.** The reviewer inferred
field-9-only from *~34 KB for a typical recovery* equalling one Ed25519+ML-DSA pair
per response rather than two. Correct — and design §5.1's own arithmetic says the same.
The stronger reason now in the text: a recovery response's `subject` MUST equal the
newly adopted node, so **field 7 is signed by the very key an attacker mounting a
fraudulent recovery already controls.** Hybridising it protects nothing. Field 9 forges
a *verifier's* attestation, which is the attack.

**U2 is broader than reported.** The reviewer raised it for adoption; the rule was
missing for every type, and a presence record has 2 participants plus 16 witnesses
with no stated order at all.

**One consequence found while applying.** With field 9's hybrid case explicit, a
recovery adoption is ~42 KB — the second-largest object in the protocol — and the size
table had no row for it. Added.

### Method note

**Nothing in part (d) changed the design.** The crate-maturity findings restate §5.2's
existing position; the `getrandom` `wasm_js` requirement was already recorded. That is
the expected result for a re-run and is not a criticism of the pass.

---

## 0.9-after — organisation on the migrated structure (2026-09-01)

**The headline result is negative and is the one that mattered.** The reviewer found
**no broad numbering collapse across the document set**, which is what the pass existed
to check: the migration relocated ~2,900 references and nothing was orphaned or
double-numbered. Everything below is local.

### Pass 1 — mechanical

| # | Finding | Disposition |
|---|---|---|
| M1 | design §11's `11.0.x` block has no `### 11.0` parent | **FIXED** — added *What the boundary is, and what it is not*, titled from the subject all four subsections already shared |
| M2 | design §14.2.4 followed by six same-depth `####` | **FIXED** — demoted to `#####` |
| M3 | wire §4.5.1 followed by five same-depth `####` | **FIXED** — demoted to `#####` |
| M4 | wire §10 and §10.1 carry identical titles | **FIXED** — §10.1 is *The push frame and the forwarding rule*, which is what it covers |
| M5 | light-client: two bullets stranded after §1.3 | **FIXED** — both are verification obligations; moved to §1.2 |
| M6 | resource §1's capacity block points at "the manifest (§7)", which is Roles | **FIXED** — moved to §8 Packaging, names the manifest inline. The manifest had no definition anywhere |
| M7 | infra §10.2 ends with a package-hosting paragraph | **FIXED** — moved to §9; it and M6 now cite each other |

### Pass 2 — splits

| # | Finding | Disposition |
|---|---|---|
| S1 | wire §3.2: 231 lines under the heading *Genesis* | **FIXED** — genesis (3 lines) folds into §3.1 where back-pointers live; the rest becomes §3.2 presence rules, §3.3 timestamps, §3.4 what structural verification decides, §3.5 the signer set, §3.6 canonicality. Chapter retitled *Common envelope and structural verification*. **21 citations retargeted**, two of which were §3.2 citing itself |
| S2 | wire §6: 327 lines, no subsection | **FIXED** — eight subsections, no paragraph moved. §6.4 had a bold lead standing in for a heading, the same defect as §3.5 |
| S3 | design §7.4: seven bullets, the first 85 lines with a nested sublist | **FIXED** — four subsections. Content nested that deep **could not be cited at all**, which is why all 25 citations named the whole chapter |
| S4 | design §18: 238 lines, eleven risk bullets, no subsection | **FIXED** — five subsections |

### Pass 3 — duplication

| # | Finding | Disposition |
|---|---|---|
| D1 | design §11.0.1 and resource §4.1 both carry the federation passage | **FIXED** — the design keeps the argument and gains the one phrase only the resource copy had; resource §4.1 keeps the obligation. 172 words removed |
| D2 | wire §4.5.2 duplicates design §8.1.1's disclosure table | **NOT UPHELD** — they answer different questions over the same exchanges: the design asks what each exchange *reads*, wire what a recipient *sees*. **What was wrong is wire's claim they are the same table**; the row sets differ. Sentence corrected |

### Pass 4 — the change log and the front matter

| # | Finding | Disposition |
|---|---|---|
| C1 | Seven duplicate `###` entry headings | **FIXED** — 2026-08-16 carries two review programmes; the later of each pair takes its own programme's *all five documents* label |
| C2 | *(found while applying)* Three days carry two `##` headings each | **FIXED** — merged; nineteen become sixteen |
| C3 | A topic index for the change log | **DECLINED** — *a section that summarises state elsewhere is stale the moment something it summarises changes.* An index over 275 entries would be the largest consolidating section in the set and the one nothing forces anybody to update. If wanted, it should be generated |
| C4 | design's front matter buried under the Preface | **FIXED** — the document-set table is `## Document set`, a sibling of the Preface rather than a child of a personal essay |
| C5 | §1 uses `patron` and `Dunbar Org` before §2 defines them | **FIXED** — each cites §2 at first use, which is the document's own convention. Vocabulary stays at §2 by the author's migration decision |

### Found while applying, and not in the review

**Four citations to §7.4 attributed the finalization threshold to it, and §7.4 states
neither half.** §8.1 defines `min(floor(n/2), 10, |candidates|)`; `wire-format.md` §5.5
says what counts toward it. Two sites now cite wire §5.5, two cite §8.1. **The split is
what exposed this** — four wrong pointers to a 191-line chapter all resolved, because
the chapter was large enough to plausibly contain anything.

**Nine headings had no blank line before them**, two of them pre-existing (design §8.1
and wire §10.1, both sitting directly under their chapter heading).

**Both disclosure tables say "ten of eleven" and each lists three rows that need
something.** Under one reading — *use* meaning *reads a disclosable field from a
presented record*, so a party that constructs or contributes has no use — ten is right
and archive presentation is the one. Under the plain reading it is eight.
**Unchanged, and for the author**: the counts are consistent with each other and the
ambiguity is in the word *use*, not in either table.

### Open for the author

**~~Three citations to §11.0.1 discuss hosting while §11.0.1 is about
federation.~~ WITHDRAWN** [author, 2026-08-31]. **All three are correct and the
misreading was mine.** §11.0.1 is what establishes that a wide-scale service is *many
local instances, each hosted by a patron* — so it is exactly the authority for a
sentence about what a node hosts. Author: *"The infra node is a server and the resource
is a package that interacts with one or more networks... even in the case of a
third-party service, the infra node runs the authentication package to access that
user-facing service, and so it's still a thing the infra node hosts."* The fourth,
*"§11.0.1's manifest"*, was genuinely wrong and is fixed — the manifest's home is
`resource-requirements.md` §8.

**What the misreading was actually detecting** is recorded below as V1: not a wrong
citation, but one word carrying four jobs.

**~~`change-log.md` holds 160 unresolved section references.~~ RESOLVED** [author,
2026-08-31]: *"Historical numbering is of no interest in this case, the change log
should point to the areas of the current design affected."* **144 references named
sections existing in no document; 26 remain.**

Git resolved only 7 — the repository starts 2026-08-25 and the log starts 08-12, so
most stale references predate all available history. The rest were resolved by reading
what each entry says its section was *about*. Whole clusters had shifted together and
most confirmed by **exact heading-title match**; several numbers meant **different
things at different sites** and were read one at a time — §11.8 is both subnet
formation and a deleted open-items register, `wire-format.md` §7.3a is `LateResponse`
in one entry and `KeyGrant` in another.

**The 26 remaining are a different grammatical case**: their sentences are *about* the
old number — *"§4a becomes §5"*, *"§8.2.1 withdrawn"*, *"§21.8 dissolved rather than
resolved"*. Remapping them would make every one false. **A reference that cites a
section was repointed; a reference that names one was left.** `Robot/review-plan.md`'s
two are the same case and stay.

**This file keeps its as-of-filing exemption** — the working rules grant it explicitly,
and the ruling was about the change log.

### V1 — "application" carries four jobs, and two of them collide

**Open for the author.** Found while checking the withdrawn §11.0.1 finding above. Not
a missing name: **the distinction the author described is already in the documents, and
already well put.** `resource-requirements.md` §4 states it as **Physically** / **Logically** —

> **Physically:** a package running on the same infra node as the network services. For
> an external service, **the conforming component is the local adaptor or gateway.** The
> vendor's own system sits behind it and conforms to nothing here.
> **Logically:** a black box exposing a set of **roles**, accepting the infra node's
> credential to access those roles.

— and the three-category table in both design §11 and resource §4 covers all three
shapes the author named: **Local application** (local to the node), **External service**
(third party), **Gateway** (another distributed system). *"The sandbox constrains the
broker, not the service behind"* (resource §4) already names the local half of a
third-party case.

**So no new jargon is needed.** What is wrong is that one word does four jobs:

| Sense | Sites | Example |
|---|---|---|
| **Node software** | **7** — design §2 (×3), §3.3, §4 (×2); light-client §0 | *"A node running the server application"*, *"the participant-facing application"*, *"Every user runs the application"* |
| **The wider thing many resources instantiate** | **3** — design §11.0.1, §20.2's A21; resource §4.1 | *"A resource is neighbourhood-scale; **an application** can be any scale"* |
| **A thing a resource can be** | 1 — design §2's Resource row | *"A service, data store or application owned by a node"* |
| **The layer** (`application data`, `application-level`, `application semantics`) | the majority | Conventional, unambiguous, leave alone |

**The first two are the collision.** §2 says *every user runs the application*; §11.0.1
says *an application can be any scale*. A reader who meets §2 first carries the wrong
sense into §11.

**The third is what made §2's Resource row unhelpful** — it enumerates what a resource
can front instead of saying what a resource *is*, which is §11's three-category table's
job and is done better there.

**Cheapest fix, using only words already in the set — about eight edits:**

- The 7 node-software sites become **client** / **node software**. Design §2 already
  defines *Light client* as *naming software*, so *"the participant-facing application"*
  → *"the participant-facing client"* is the definition using its own term.
- §2's **Resource** row says what a resource is and points at §11 for the shapes, rather
  than listing *"a service, data store or application"*.

**APPLIED** [author, 2026-08-31]: *"2 is the client software and 11 is the resource
application."* Each sense now carries its own name.

- **The 7 node-software sites say `client software` or `server software`.** design §2's
  two rows, §3.3's deeper-chain rule, §4's in-scope list (both lines), and
  `light-client-requirements.md`'s opening line. **No new term** — §2 already said the
  row *names software*.
- **`resource application` is the author's term and is anchored at both first uses**:
  *the wider system a resource is one instance of — the network hosts the instance and
  knows nothing of the system* (design §11.0.1), and the package-author phrasing in
  resource §4.1. A21 follows.
- **`change-log.md`'s one occurrence is left as written**, per the standing rule that
  the log records what the text said at the time.

**Verified**: the node-software sense appears nowhere in the five design documents.
What remains of the word is the **Local application** category name, the conventional
layer sense (`application data`, `application actions`, `application layer`), and one
site of the word meaning *act of applying*.

**§2's `Resource` row is deliberate and stays** [author, 2026-08-31]: *"Section 2 is
introducing the resource for the first time and explaining it in familiar terms."* The
row is orienting a reader who has met none of this yet, so *"a service, data store or
application"* is doing the work everyday words do — §11's three-category table is the
taxonomy, and it arrives when the reader needs one. **The fourth sense was never a
collision**; it reads as the ordinary English word, which is the point.

---

## Test-vector review, first run (2026-08-31, different model family)

**The reviewer reran the generator byte-for-byte and independently confirmed the
adoption txid and the `SignedLocator` signature — no arithmetic error found.** The
findings are about fit: what the vectors fail to exercise, and outcome semantics.

### Blockers — confirmed, queued as the canonical bar (vectors README)

| # | Finding | Disposition |
|---|---|---|
| B1 | No fully valid envelope; synthetic ML-DSA pubs have no private halves, so filling the slots later is impossible — **canonical promotion is a wholesale regeneration with real keypairs** | **CONFIRMED, blocked on tooling** (no ML-DSA implementation reachable; pip absent). Consequence recorded in README; mechanical when tooling exists, since every value flows from the generator |
| B2 | Verifier selection tested as arithmetic, not recomputation — *n* and candidates supplied, never derived by DAG traversal | **CONFIRMED** — canonical bar item 3 |
| B3 | No normal-subtype presence record connecting nonces, selection, consent, responses, threshold and signer set | **CONFIRMED** — canonical bar item 2 |

### High — applied this pass

| # | Finding | Disposition |
|---|---|---|
| H1 | Accept/reject is too coarse for the spec's outcome vocabulary | **APPLIED** — `negative-vectors.md` restructured: five outcome classes (malformed, unverifiable, incomplete, ineffective, accepted), three fixture kinds (byte-level, context + external state, method) |
| H2 | E8's rationale wrong: disavowal codes 0–63 are a banded exception, retained and evaluated | **CONFIRMED, FIXED** — E8 re-pointed at a non-excepted enumeration; new T8 (code 64 malformed as out-of-space); new must-accept D1 with a generated code-40 body |
| H3 | S9 misnames the Recovery proof as `COSE_Sign1` | **CONFIRMED, FIXED** — names the exact `COSE_Sign1` contexts; states Recovery field 3 is hybrid `COSE_Sign` and why the kid-omission rule still reaches its entries |
| H4 | R9 cannot be an unconditional reject (a cheating established key mints one; no-history validators cannot tell) | **CONFIRMED, FIXED** — split R9/R9b context fixtures: fork-detected with the real chain, accepted without |
| H5 | R10 conflates malformed with incomplete | **CONFIRMED, FIXED** — R10/R10b: predecessor present → malformed; unfetchable → incomplete |
| H6 | T6 too broad — the rule is per `(subject, verifier)` slot | **CONFIRMED, FIXED**, with the one-verifier-two-subjects positive stated |
| H7 | Signer order vs kid order never diverge, so a conflating implementation passes | **APPLIED** — new adoption vector where the node's keyhash sorts after the patron's (list 0 is the node's, first entries the patron's); the formation record's field-3 order reversed to disagree with keyhash order |
| H8 | Witness nonces arbitrary; §5.2.1's derivation untested | **APPLIED** — nonces now derived (HMAC-SHA-256 per §5.2.1) from synthetic witness secrets; seed consumes them; independently re-derived in verification. New interpretation: the PRF's ordinal encoding is unstated (8-byte BE used, per §5.3's layout) |
| H9 | Synthetic disclosure root — formation txid belongs to no valid presentation | **CONFIRMED** — canonical bar item 5 |
| H10 | Negative vectors mostly prose | **PARTIAL** — outcome classes and the `bytes + context + state + outcome` fixture format adopted; machine-readable files are canonical bar item 6 |

### Medium — applied or queued

| # | Finding | Disposition |
|---|---|---|
| M1 | DAG semantics barely covered | **PARTIAL** — a real two-head merge added (the adoption and formation branches of one chain, reunited in a departure); 8/9 boundary queued (bar 7). **New under-determination found**: §3.1 states no order for a merge list — ascending bytewise used, flagged, since without a rule one logical merge has several txids |
| M2 | Boundary/exception semantics under-covered | **QUEUED** — bar 7; four must-accept rows (D1–D5) added now |
| M3 | README over-classifies determined choices as interpretations | **APPLIED** — old #4–#7 demoted to *Determined by the profile*; interpretations now the three genuine ones plus two new (PRF ordinal encoding, merge-list order) |
| M4 | Stale generator commentary; no spec pin | **CONFIRMED, FIXED** — commentary updated; every generated file now embeds the SHA-256 of `wire-format.md` it was generated against |

### Specification findings — all verified against the text, fixed

| # | Finding | Fix |
|---|---|---|
| SP1 | §5.4's witness-only sentence: "names them in field 8, not field 4" — pre-migration numbers | Now "field 4, not field 3" |
| SP2 | §4.5.2's recomputation row: "body fields 4, 8, 11" — no key 11 exists | Now "fields 3, 4, 7" |
| SP3 | §1.1's context table missing `rhtn/1:endpoints`; hash/PRF tags conflatable with signing contexts | Row added; one-sentence family distinction added |

### Open for the author

- **T7 / the seed sentence** (§4.1): *"a record carrying one would be malformed"* is
  untestable — unknown keys are preserved and nothing marks a seed. Reserve a key
  range, or restate as a writer commitment. §1.1's own enforceability test.
- **Five under-determinations** now standing as vector interpretations, each a
  one-sentence spec clarification if confirmed: `SignedLocator` payload form,
  genesis hash input, §5's raw-concatenation framing, §5.2.1's ordinal encoding,
  merge-list order.

---

## Test-vector review, second run (2026-08-31, different model family)

**Reran the generator byte-for-byte against the pinned spec; no transcription
drift.** All findings are fit and coverage. Every one was verified against the text;
all held, including one that corrected the previous round's correction.

| # | Finding | Disposition |
|---|---|---|
| 1 | **E8 wrong again**: `ClientIntegrity.scheme` is an open namespace — *"a validator checks only the shapes"* | **CONFIRMED, FIXED** — E8 now uses presence `subtype = 2`, a genuinely closed load-bearing enum. The row's two wrong instantiations are recorded in it: the closed-enum default has enough exceptions that every instantiation needs checking |
| 2 | The nonce table cannot be a conformance vector — §5.2.1 admits any 32-byte PRF | **CONFIRMED, RELABELLED** — now an HMAC-SHA-256 reference example; what is conformance-testable (commitment equation, seed) is stated. Whether to harden HMAC as normative → **author queue** |
| 3 | Pinning `wire-format.md` alone misses the authority model — the design wins on disagreement | **CONFIRMED, FIXED** — every generated file pins both documents' SHA-256 |
| 4 | A single-enum expected outcome recreates the boolean collapse the prose warns against | **CONFIRMED, FIXED** — structured result model: structural / signatures(per signer) / chain / selection(per subject) / effective / evidentiary |
| 5 | Peering and §7 standalone objects absent and unacknowledged | **CONFIRMED, PART-FIXED** — positive peering vector (optional-field omission shown live) and a complete classical `EndpointRecord` under `rhtn/1:endpoints` (new `records.md`, one known-answer per signing context as the growth path); the rest scoped explicitly |
| 6 | COSE profile under-tested for what a default library accepts | **CONFIRMED, FIXED** — S10 embedded payload, S11 tagged nested COSE, S12 wrong/empty `external_aad`, S13 extra protected parameter, S14 out-of-profile alg; method rule C2, tag from context never content |
| 7 | seqno vectors do not force strictly-greater or series-arbitrariness | **CONFIRMED, FIXED** — counter-jump `SignedLocator` pair [5,42]→[5,100] (D6) and a reissue to a numerically smaller series, 0xDEADBEEF→2 (D7), both fully signed where applicable |
| 8 | Recovery cross-object bindings untested | **PART-FIXED** — T9 empty response set, T10 no `match`, T11 non-hybrid old-key proof, T12 successor `new_key` ≠ adoption field 1: all four verified as stated rules. **Two of the reviewer's cases are stated nowhere** — self-adoption (field 1 = field 2) and `prior_key` = new key — and went to the author instead of into vectors. A complete Recovery adoption is canonical bar 4 |
| 9 | D2 does not prove signature coverage of unknown keys | **CONFIRMED, FIXED** — the extended adoption now carries its full envelope; independent verification confirms both Ed25519 signatures verify and mutating `c0ffee`→`c0ffef` breaks both (E10) |
| 10 | Verifier-selection backlog missing the verified-only and pruning-boundary traps | **CONFIRMED** — added to canonical bar 3 |
| 11 | Selective-disclosure needs its negative family | **CONFIRMED** — added to canonical bar 5 |
| 12 | "Every explicit bound" overclaims | **CONFIRMED, FIXED** — scoped to bounds within the declared suite scope, remainder enumerated as joining with their objects |
| 13 | Real ML-DSA needs an independent deterministic keygen recipe, not generator-as-oracle | **CONFIRMED** — canonical bar 1 now requires the recipe first |
| 14 | Interpretation 1 recurs across eight signed objects | **CONFIRMED, GENERALISED** — one global sentence proposed; `EndpointRecord`'s vector instantiates the same map reading |

**Author queue after this round** (also in the vectors README): the §4.1 seed
sentence's testability; the five interpretations (one sentence each, the
canonical-CBOR one resolving eight objects); HMAC-SHA-256 normative or reference;
whether self-adoption is structurally malformed; whether `prior_key` may equal the
new key.

---

## Author rulings on the test-vector queue (2026-08-31 / 2026-09-01)

| Question | Ruling | Applied |
|---|---|---|
| The §4.1 seed sentence | **Writer commitment** — the security rests on the seed never needing to leave the device, and the writer mostly injures themselves | §4.1 reworded; the negative suite's gap section records the resolution; no fixture by design |
| "Canonical CBOR of fields X–Y" | **The map**, globally — *"map encoding seems easier to debug than concatenation"* | One sentence in §1 beside the coverage rule, resolving all eight signed objects; vector interpretation 1 retired |
| §5.2.1's PRF | **Normative as a client commitment: HMAC-SHA-256**, ordinal 8 bytes big-endian | §5.2.1 rewritten with the RFC 6979/Ed25519 precedent; the nonce table is now a client-conformance vector; interpretation 4 retired |
| Self-adoption | **Malformed** | §4.1 rule (the degenerate cycle — the one a validator sees from the record alone); T13 |
| `prior_key` = new key | **Forbidden** — the retained-key, lost-archive case is an adoption, not a recovery | §4.1 rule with the fetch/adopt/merge path cited; T14 |
| §2.3 "advanced only by §4.6" | **Qualified** — within a relationship; a new adoption establishes its relationship's series. *"It is at the Patron's discretion to relax the history walk to adopt a user on a fictive seqno in this situation"* | §2.3 carries the qualifier and the discretion sentence |
| `ArchiveRequest` field 2 | **Optional** — absent means the holder's newest; **the light client must surface the trust this involves** | Field made optional (§7.9) with the newestness-is-the-holder's-claim paragraph; `light-client-requirements.md` §2 gains the restore-on-trust interface obligation covering the whole lost-archive path |
| Unsealable lost series | **Say so** | One paragraph at §4.6: benign in loss, moot in theft where rotation is the remedy |

**The lost-archive sequence is now fully stated in the documents it touches**:
refetch first (§7.9, head optional), else ordinary adoption on a fresh
patron-countersigned series (§2.3), merge the old branch back when its head
resurfaces (§3.1, design §10.3) — and Recovery stays what it was, a key-change
mechanism.

**Remaining open**: interpretations 1–3 in the vectors README (genesis hash input,
§5 raw concatenation, merge-list order); whether departure, disavowal, peering and
reissue reject the degenerate equal-pair; whether a root can reissue at all.

**Correction, same day**: the degenerate-pair ruling above ("No") was the author
misreading the question and is reversed — **all two-party types reject field 1 =
field 2**. Applied at `wire-format.md` §4.1; T13 widened to the five types. The
root-reissue consequence (a root cannot produce a type-7 at all) is with the author
as an advice question.

**Final rulings (2026-09-01), closing the vector queue entirely**: the three encoding
interpretations became specification sentences — genesis hashes the raw 32 keyhash
bytes (§3.1); §5 states its raw-concatenation convention once, with the
fixed-length-injectivity argument; a merge back-pointer list sorts ascending
bytewise (§3.1), E11 added as the negative complement. **The root series question is
resolved by the author's own refinement**: a root cannot produce a type-7 and does
not need to — the rollup point is an *internal* operation, the chain simply
continuing under a new series designator; the back-pointer distinguishes it from a
genesis event, and presented as a history root it is logically equivalent to one
(§4.6 closing paragraph; §2.3 carries the pointer). **The vectors' interpretations
register is empty** — every byte in the suite now follows from the text.

---

## Test-vector review, third run (2026-09-01, verification round)

**Reran the generator; all five outputs byte-for-byte, both pins matched.** Twelve
findings, every one verified against the text; eleven applied, one to the author.

| # | Finding | Disposition |
|---|---|---|
| 1 | Type-6/abuse-report taxonomy contradictory; the suite silently chose the archive-six reading | **CONFIRMED — AUTHOR RULING NEEDED.** §4's table assigns type 6 to `AbuseReport`; §6.3 defines it with no key 0, an embedded signature and no carriage; §3.1's signer row still names *resource registration*, retired 2026-08-28 — a missed propagation from that retirement, whose rationale applies verbatim to the abuse report. The suite's reading is now declared an open interpretation; no abuse vector until ruled |
| 2 | `records.md` queued unsigned objects for known-answer signatures | **CONFIRMED, FIXED** — KeyGrant is transient E2E payload, the late-response wrapper adds no signature, resolution and archive-fetch messages are unsigned: all verified. `records.md` now separates signed contexts from unsigned encodings |
| 3 | D2/E10 claim four failing signatures; placeholders can prove two | **CONFIRMED, FIXED** — constrained to both classical signatures, PQ slots named non-oracular until canonical bar 1, in the fixture rows and the generated text |
| 4 | Result model cannot express `failed` signatures or the withheld-`strongest` outcome | **CONFIRMED, FIXED** — `signatures[signer, alg]` gains `failed`; new `checks[name]` dimension (pass/fail/unverifiable(withheld)); R11 re-expressed and unhooked from `selection` |
| 5 | Boundary-sweep wording claims coverage that does not exist; audit/endpoint bounds mislabelled out-of-scope | **CONFIRMED, FIXED** — "planned, not present"; peering-audit and NetworkPoint-list bounds moved into the target list, since their objects are covered |
| 6 | `patron_key` binding untested | **CONFIRMED, FIXED** — §4.1 requires checking both successor bindings; T15 |
| 7 | Reissue counter-0 and series-reuse negatives missing | **CONFIRMED, FIXED** — both rules verified as stated (§4.6); T16 byte-level, V4 context |
| 8 | "All six types" overstates: bodies, not envelopes | **CONFIRMED, FIXED** — restated as positive body vectors; **a departure envelope added and independently verified** — the single-signer shape, two entries, with §4.2's decoder-MUST-NOT-expect-the-patron rule stated |
| 9 | KeyMaterial negatives thin against the identity profile | **CONFIRMED, FIXED** — P9–P14: cardinality, missing labels, wrong kty/crv/alg, forbidden `priv`, wrong key widths |
| 10 | Outer `COSE_Sign` headers not explicitly pinned | **CONFIRMED, FIXED** — S15/S16 |
| 11 | Formation inverses missing | **CONFIRMED, FIXED** — R12/R13 (witnesses or responses present on a formation record). The `window_ordinal` consistency rule is implicit in the spec; **proposed to the author** as one sentence, same class as the `finalized_at` bounds |
| 12 | Generator docstring stale | **CONFIRMED, FIXED** — five files, both pins |

### A correction to this file's own record

**Review round 2's "stated nowhere" was wrong, and the round-2 verification here
missed it**: `wire-format.md` §4.1's successor block already carried both rules —
*"a node cannot hold authority over itself"* and *"identical keys represent no
rotation at all"*. The author's rulings were therefore confirmations of existing
text, and the round-2 additions created duplicates, now consolidated: the
generalised two-party rule at the §4.1 schema and the prior≠new bullet in the
consistency list each absorbed the pre-existing phrasing, and the successor-block
duplicates are removed. The grep that failed searched for "self-adoption"-family
phrasings and missed "authority over itself". Sweep by more than one phrasing.

**Rulings on the third-review questions (2026-09-01)**: **type 6 is retired** — the
abuse report is §6.3's standalone signed object; the §4 table carries a tombstone
row on the registration-retirement grounds (*advances no archive, reaches only its
addressee, chains to nothing*), and §3.1's stranded registration/abuse signer row is
gone. **Key 7 MUST equal `floor(started_at / 86400)`** — structural, clockless, the
`finalized_at` class; R14 added and the formation vector confirmed clean against it.
Found in the sweep: the design's §4 scope list had omitted **series reissue** since
type 7's introduction on 2026-08-30 — completed. The vectors' interpretation
register and author queue are both empty again, this time with the taxonomy ruled
rather than assumed.

---

## Test-vector review, fourth run (2026-09-01, verification pass)

**Five outputs byte-for-byte, pins matched.** Thirteen findings; ten applied, two
specification contradictions to the author, one observation alongside them.

| # | Finding | Disposition |
|---|---|---|
| 1 | `pending` responses are not constructible: the offline verifier cannot sign what the schema requires, and no patron-authenticated variant exists — while §5.5 counts `pending` toward finalization | **CONFIRMED — AUTHOR.** Options presented in the vectors README: a patron-authenticated pending (the queue-holder attests delivery); absence-as-encoding, which changes §5.5's counting; or other |
| 2 | §1.1 carried a phantom thirteenth row — `VerificationQuery` is hashed, never signed | **CONFIRMED, FIXED** — row removed, the table is twelve and the stated count true again; the canonical-form pointer moved into the hash-family note. *Observation raised alongside*: `query_id` is the profile's only undomained hash of a CBOR map |
| 3 | §5.3.1's "closed at the near end" contradicts its strict formula | **CONFIRMED — AUTHOR.** The vector implements the formula; the word or the formula must move |
| 4 | The canonical bar omitted the unsigned message families | **CONFIRMED, FIXED** — bar item 9: positive encodings plus characteristic malformed/must-accept per family, naming the two divergences a generic implementation misses (unknown-type behaviour, 64 KB vs 256 KB) |
| 5 | Signer-to-role binding untested — right shape, wrong identity passes | **CONFIRMED, FIXED** — S17 now; bar item 10 generalises it per type |
| 6 | The commitment-recompute MUST has no negative | **CONFIRMED, FIXED** — V5, riding the planned normal record; named in bar item 11 |
| 7 | Finalization must-accepts missing (threshold met by non-`match` results; omitted selected slot) | **CONFIRMED, FIXED** — bar item 11 |
| 8 | `VerifierResponse` conditional-field matrix untested | **CONFIRMED, FIXED** — T19–T23, from the schema's own REQUIRED/absent rules |
| 9 | The committed-predecessor trap unnamed in the history fixture | **CONFIRMED, FIXED** — bar item 11: a backfilled post-ceremony head that would change *n*, expected selection unchanged |
| 10 | Equal-seqno/different-content pair untested | **CONFIRMED, FIXED** — the pair is now *generated*: two independently verified `EndpointRecord`s by one signer, same seqno, different endpoints; V6 states the malformed-condition-not-a-tie rule |
| 11 | `EndpointRecord` duplicate entries and the u16 port edge unlisted | **CONFIRMED, FIXED** — T17, T18; the boundary list gains 65535/65536 |
| 12 | Pins are provenance, not a gate; the hand files were unpinned | **CONFIRMED, FIXED** — `tools/spec-pins.json` gates generation: a changed spec hash refuses to run without `--accept-spec-change` (tested both ways); the two hand files carry a machine-managed pin line the generator rewrites, so their staleness is mechanically visible |
| 13 | `records.md` intro said every §7 object is signed | **CONFIRMED, FIXED** — in the generator, as the reviewer specified |

**Rulings on the fourth-review contradictions (2026-09-01)**:

**Absence is the encoding.** *"Late arriving replies are private information for
participants, not part of the record."* `pending` leaves the `VerifierResponse`
enum entirely — an unreachable verifier answers nothing, nobody signs on its
behalf, and its slot is simply absent from field 5, visible against the
recomputed selection. **The threshold now sizes the sample and does not gate
finalization**: wire §5.5 is rewritten (*The selected slots, and what the record
carries*), design §8.1's invariant block is renamed *The verifier sample* and its
blockquote now reads *select and query* rather than *require … to finalize*. The
DoS rationale inverts cleanly: suppression cannot stall a ceremony, it can only
produce a record that advertises its own thinness. Late replies resolve privately;
`LateResponse` remains the responder's opt-in durability. Swept: the ceremony
summary, the §7.4.3 offline-client bullet, both schemas, the withholding-posture
sentences, the stolen-device analysis, the dropped-query paragraph, the reads
table, P14, the parameter table (row renamed *Verifiers selected and queried per
subject*), and the unset parameter renamed *queued verifier-reply patience*. The
q-vs-n aside's stale "§12.2's finalization threshold" citation was caught and
fixed in the same sweep. Vectors: T20 notes result 4 is gone, V7 is the
absent-slot must-accept.

**The window is exclusive at both ends** — *"previously completed ceremonies
only."* §5.3.1's "closed at the near end" is gone; formula and vector were already
strict, and the generator's boundary table now says so in the ruling's words.

**Final item closed (2026-09-01)**: the `query_id` observation resolved as option
(b) — §1.1 now states the **hash-disjointness invariant**: the four untagged hashes
(`txid`, `keyhash`, `query_id`, genesis) have pairwise structurally disjoint
preimage languages (mandatory key 0; the `KeyMaterial` array; first key 1; exactly
32 bytes), and any future hashed object MUST stay disjoint or carry an `rhtn/1:`
tag. Chosen over tagging `query_id` alone because the invariant is what does the
work — a tag would have left `txid` and `keyhash` resting on unstated luck. **The
test-vector author queue is empty, for the first time with every closure a ruling.**

---

## Canonical bar item 1 complete (2026-09-01): real ML-DSA-65 throughout

The tooling blocker dissolved when the author enabled pip on the machine (PEP 668
routed the install into a target directory). `dilithium-py` 1.4.0 supplies FIPS 204
seed-based keygen and deterministic signing; **pyca `cryptography` 50.0.1 serves as
the independent second implementation** in the verification harness.

**The keygen recipe, now stated in the suite**: `xi =
SHA-256("rhtn-test-vectors:<name>:ml-dsa-65-seed")`, keypair =
`ML-DSA-65.KeyGen_internal(xi)`, deterministic signing, empty context.
**Cross-implementation agreement proved before regeneration**: pyca re-derives every
public key from the same seeds and verifies dilithium-py's deterministic signatures
— the recipe is implementation-independent, not an oracle over one generator.

**Wholesale regeneration executed** — every keyhash changed, so every body, txid,
seed, rank and ordering recomputed; the generator's placeholder machinery is gone.
Independent verification after: 11 keyhashes re-derived from scratch with pyca's
keygen; 11 bodies parse canonically with matching txids; **4 envelopes, 14
signatures, every one verified under the independent implementation**; the E10
mutation now breaks **all four** signatures of the extension envelope; nonces and
seed re-derived. D2/E10 restored to full strength; README's canonical bar item 1
marked done. The suite is **cryptographically complete** — canonical status still
awaits what it always did, an independent implementation reproducing the whole
suite.

---

## Test-vector review, fifth run (2026-09-01)

**Reproduced the snapshot byte-for-byte, pins matched.** Ten findings; all ten
verified and applied — the two specification contradictions were both stale
survivors of already-made rulings, so no new author decision was needed.

| # | Finding | Disposition |
|---|---|---|
| 1 | §3.2 still said an unmet threshold blocks publication — contradicting §5.5 and the design after the absence ruling | **CONFIRMED, FIXED** — the bullet now says the sample does not gate finalization; a ceremony the participants abandon is what stays local. The sweep that missed it also hid a stale `LateResponse (§7.3)` reference, now §7.4 |
| 2 | "Both signatures here are `COSE_Sign1` and classical-only" contradicts field 9's own Recovery-hybrid rule | **CONFIRMED, FIXED** — qualified to the ordinary presence record; consent classical everywhere; field 9 hybrid inside `Recovery`, with the permanence rationale |
| 3 | Most negatives are descriptions, not bytes — reopening the derivation problem for harness builders | **CONFIRMED** — canonical bar 6 rewritten: every A/B/D case must resolve to exact bytes or a deterministic mutation of a named positive vector, plus machine-readable expected dimensions |
| 4 | Retired type 6 and unassigned type values had no stated handling and no fixtures | **CONFIRMED, FIXED** — one sentence at the type table (*the tombstone reserves the number; it does not readmit the bytes*); S18 (type 6) and S19 (type 9) |
| 5 | No negative for a non-uint protocol map key | **CONFIRMED, FIXED** — E12: `-1` or a text key is malformed, not an "unknown extension key", which is always a uint |
| 6 | Bar 3's "a merge" can be satisfied without catching double-counting | **CONFIRMED, FIXED** — bar 3 now requires a **diamond**: an in-window qualifying record reachable through both merge heads, asserted to contribute one to *n* |
| 7 | Endpoint-list boundary missed the lower bound | **CONFIRMED, FIXED** — 0/1 joins 8/9 |
| 8 | The standalone `COSE_Sign1` path under-tested: no kid/unprotected/alg negatives, no extension-coverage fixture | **CONFIRMED, FIXED** — S20–S22; and a **generated** extension `EndpointRecord` (D8) with its mutation complement (E13), independently verified both ways |
| 9 | The pin gate silently bootstraps when the pin file is missing | **CONFIRMED, FIXED** — a missing file is now fatal without `--bootstrap-pins`, a flag deliberately distinct from `--accept-spec-change`; tested |
| 10 | Text-mode I/O assumptions; no generator provenance | **CONFIRMED, FIXED** — every write is explicit UTF-8 with `\n` newlines; `spec-pins.json` now records the producing generator's SHA-256 (provenance, not gated) |

---

## Test-vector review, sixth run (2026-09-01)

Fifteen findings; **fourteen verified and applied, one applied as a determinate
completion the reviewer read as an open choice**. The round's centre of gravity was
the reviewer's closing observation: the specification relied on an invariant §3.3
did not impose.

| # | Finding | Disposition |
|---|---|---|
| 1 | **Effective-time monotonicity stated for presence records only**, while §5.4's pruning and §3.2's chronology bound rely on it chain-wide — a non-presence transaction could bridge backward | **CONFIRMED, FIXED** — §3.3 now binds **every** type: each record's effective time (`started_at` or `timestamp`) ≥ every committed predecessor's, merge heads included, with the reliance stated as the reason. T24 is the temporal-bridge negative, with the one-head-violates merge variant |
| 2 | `LateResponse` still said "the record already met its threshold" | **CONFIRMED, FIXED** — stale survivor of the absence ruling (with a stale §4.5-for-§3.2 citation beside it); now speaks in absent-slot terms |
| 3 | R11's revealed-violation verdict was an unstated choice | **CONFIRMED — but determinate, and the drift was this file's, not the spec's**: §3.2 calls it a *structural* rule, so revealed-and-violated is malformed. §3.2 now says so explicitly; R11 corrected and its own history noted in the row |
| 4 | The unsigned unknown-key rule named five families and left the rest ambiguous | **CONFIRMED, FIXED** — the rule is global in §1 (every message outside a signature's coverage), the §6 instance now cites it |
| 5 | Chain-binding arity untested (list counts, 31/33-byte hashes, seqno arity) | **CONFIRMED, FIXED** — P15–P18 |
| 6 | No fixture isolates the content-address check from signature verification | **CONFIRMED, FIXED** — V8: a valid signed envelope answering the wrong txid |
| 7 | Signer-role binding missing for standalone objects | **CONFIRMED, FIXED** — a **generated** wrong-signer `SignedLocator` (field 1 alice, bob's valid signature; S23), harness-checked to fail under the named subject and verify under bob; bar 10 broadened |
| 8 | Extension coverage only top-level | **CONFIRMED, FIXED** — the extension adoption now carries a **nested** unknown key inside its `Locator`; the harness proves both mutations break all four signatures |
| 9 | Six-type positive coverage never decodes the optional fields | **CONFIRMED, FIXED** — three optionals-exercised bodies: adoption with `KeyMaterial`/head/PoP-ref, departure with reason, peering with commitment and audit; bar 13 makes the principle standing |
| 10 | Session semantics need trace fixtures, not byte fixtures | **CONFIRMED** — bar 9 amended with the action vocabulary |
| 11 | Two generator tables were hand-typed literals | **CONFIRMED, FIXED** — the shortest-form and `required()` tables are now generated from the same encoder and formula the vectors use |
| 12 | Positive construction relied on author discipline | **CONFIRMED, FIXED** — assertions in `e_map` (duplicate keys), `path`, `seqno`, `backptrs`, and `envelope` (per-type signer counts, distinctness) |
| 13 | Generator changes were recorded but not gated; outputs unhashed | **CONFIRMED, FIXED** — pins v2: three specs gated (`light-client-requirements.md` joins, per finding 15), generator gated behind `--accept-generator-change`, every output's SHA-256 recorded |
| 14 | The cross-check claims were externally asserted, not in the corpus | **CONFIRMED, FIXED** — `tools/verify.py` is now part of the suite: an independent decoder and verifier sharing no code with the generator, 18 checks, exit-nonzero |
| 15 | The nonce table's client-conformance status was outside the stated scope and pin model | **CONFIRMED, FIXED** — scope restated as wire interoperability *plus named client-conformance vectors*; the light-client document is pinned |

Plus the two closing suggestions: the enumeration/extension **matrix** is bar 12
(E8's two wrong instantiations being the argument for mechanical enumeration), and
the rank tie-break is C3 — a comparator requirement the real-hash suite cannot
instantiate without a SHA-256 collision.

---

## Test-vector review, seventh run (2026-09-01)

**Independently re-hashed every pin — specs, generator, all seven outputs — exact
match; no arithmetic discrepancy in anything recomputable.** The round found no new
encoding ambiguity ("the supplied byte vectors appear derivable from the present
text") and concentrated on semantic coherence and harness schema. Twelve findings
plus four assumptions to record; all applied.

| # | Finding | Disposition |
|---|---|---|
| 2 | **The optionals adoption's field 8 references the alice–carol formation from an alice–bob adoption** — structurally legal, semantically non-supporting, presented as a positive exercise | **CONFIRMED, LABELLED** — the vector now states the mismatch is deliberate and why the clean fix must wait: the suite's only presence record names the wrong pair, and a second formation naming alice would violate §3.2's one-formation-per-key rule inside the positive universe. Swaps to the genuine alice–bob record at bar 2; V9/V9b are the dereference context fixtures |
| 3 | No stated home for failed/unavailable reference evaluation in the result model | **CONFIRMED, DECIDED** — `checks[...]` extends to dereference evaluation (`fail` / `unverifiable(unfetchable)`); `effective` and `chain` explicitly do not absorb it. Recorded assumption 1 |
| 4 | README overclaimed the harness ("every mutation and arithmetic claim") | **CONFIRMED, FIXED** — "every *generated* mutation and arithmetic claim it currently reaches" |
| 5 | The harness discovers fixtures by parsing Markdown conventions | **CONFIRMED** — folded into bar 6: machine-readable fixture *identity*, not only expected results; Markdown is presentation, never an interface. Recorded assumption 2 |
| 6 | The selection-derivation fixture deserves promotion-blocking status | **CONFIRMED** — bars 2 and 3 so marked, with the reviewer's framing kept: the arithmetic tables test none of the specification's hardest derivation |
| 7 | The synthetic disclosure root makes the formation vector non-compositional | **ACKNOWLEDGED, now stated in the vector itself**: a root-recomputing validator cannot use it as an integrated known-answer object |
| 8 | The enum matrix must reach unsigned-family result codes and their evaluation-order traces | **CONFIRMED** — bar 12 amended (`ResourceResponse`'s six statuses named) |
| 9 | Cross-context signature substitution beats the artificial empty-AAD case | **CONFIRMED** — bar 8 amended: valid context-X signature presented as context Y must fail under Y's reconstructed tag |
| 10 | The hash-disjointness invariant had living witnesses but no mechanical guard | **CONFIRMED, FIXED** — the generator now asserts the language classifications over every generated body, `KeyMaterial`, and the reserved query language; a schema change breaking disjointness fails generation |
| 11 | `e_map`'s generality is an assumption | **CONFIRMED, DOCUMENTED** — it implements RFC 8949 §4.2.1's bytewise-encoded-key order via unique-key pair sorting (the current rule, not RFC 7049's length-first); exercised only over the suite's key domains. Recorded assumption 3 |
| 12 | Rank ties define a fixture class, not just an exception | **CONFIRMED** — the unit-fixture class joins bar 6's schema. Recorded assumption 4 |

Findings 1 (declared gaps are real) and the execution note (the reviewer could not
run `verify.py` without `dilithium_py` — environment, not suite) required no
change. **The four recorded assumptions now live in the README as their own
section, distinct from encoding interpretations.**

---

## Test-vector review, eighth run (2026-09-01)

**All pins independently recomputed, exact match.** Ten findings; nine applied, one
to the author — the round's genuine discovery, an unstated protocol assumption
inside the generator itself.

| # | Finding | Disposition |
|---|---|---|
| 1 | Declared gaps real, promotion-blocking | No change needed — status accurate |
| 2 | **The generator asserts `path` ≥ 1 nibble; §2.1 states no lower bound** | **CONFIRMED — AUTHOR.** The question has protocol shape: may a locator's path be empty — a node that is its own anchor, i.e. a root — or is one nibble the minimum? Reopened in the README's interpretations register; the assertion stands flagged until ruled |
| 3 | `verify.py` used decode→re-encode for canonicality — the method E9/C1 forbid | **CONFIRMED, FIXED** — the parser now rejects non-shortest forms at byte level (duplicates/unsorted/indefinite were already parse-time); re-encode survives only as a cross-check of the harness's own encoder, never the verdict |
| 4 | The harness collapses tstr/bstr and covers a restricted type domain | **CONFIRMED, FIXED** — text strings get a distinct representation; the docstring states the domain; richer extension-value must-accepts queued with their families |
| 5 | `sign1_object` inferred the signature slot from key magnitude — an unknown key ≤ 10 would be misread | **CONFIRMED, FIXED** — slots are schema-fixed arguments; and the trap is now a **generated vector**: a `SignedLocator` carrying unknown key **4**, directly above its slot (D9), with mutation complement E14, both harness-verified |
| 6 | The unsigned inventory omitted the currency request/reply | **CONFIRMED, FIXED** — inventory completed (registration/reply, catalog, resource request/response also now named); the hand-list's own failure is recorded in it as the argument for bar 9's mechanical enumeration |
| 7 | Response cross-binding fixtures missing two of three bindings | **CONFIRMED, FIXED** — T25 (subject names a non-participant), T26 (transplanted query_id) join T23 |
| 8 | Series semantics need stateful complements | **CONFIRMED, FIXED** — V10 (post-reissue high-counter record in the abandoned series: reject at any counter, §4.6.1's MUST verified at the text) and V11 (two current-looking series never rank numerically; the presented chain decides) |
| 9 | Boundary must-accepts missing: exact 24 h gap, `finalized_at == started_at`, idempotent replay | **CONFIRMED, FIXED** — D10, D11, D12; the sweep list gains the 0/86,400/86,401 gap boundary and fixed-width keyhash fields |
| 10 | `verify.py` unpinned; the spec-accept flag's audit obligation understated | **CONFIRMED, FIXED** — the harness hash is pinned and gated with the generator flag; the accept-spec-change refusal message now states both audit halves: generator constructions **and** hand-authored fixture semantics |

The reviewer's execution note — could not run the ML-DSA path without
`dilithium-py` — is the environment limitation already recorded; the harness's
source made its coverage inspectable, which was the point of including it.

**Ruling (2026-09-01): roots legitimately self-anchor.** The empty path — zero
nibbles, empty byte string, count 0, `{1: h'', 2: 0}` — is valid and is the
self-anchor case. Stated at wire §2.1; the design's Anchor vocabulary row and
§12.1's anchor bullet carry the root clause. The generator's ≥ 1 assertion is
corrected to ≥ 0, and **D13 is the generated must-accept**: bob's complete
self-anchored `SignedLocator`, harness-verified — anchor equals subject, path
empty, signature valid. The interpretations register is empty again.

---

## Test-vector review, ninth run (2026-09-01)

Ten findings; eight applied, two to the author. One was a genuine **specification
inconsistency** — the second the vector programme has caught in the wire format's
own bound table.

| # | Finding | Disposition |
|---|---|---|
| 1 | The harness verified crypto and canonicality but not schema conformance — the generator was the semantic oracle for positives | **CONFIRMED, FIXED** — `verify.py` now validates every envelope's body against a per-type schema (required fields, shapes, back-pointer list counts, merge sort) and **derives the required signer set from the body**, asserting the envelope's kid set equals it. Presence stays bespoke until bar 2 |
| 2 | No result dimension for freshness/reconciliation outcomes | **CONFIRMED, FIXED** — `state_action = install · replace · replay · ignore_stale · conflict · incomparable`; existing rows (D6, D12, V6, V11) labelled, D17 added |
| 3 | Explicit default port undetermined — and §7.6 distinctness needs the answer | **CONFIRMED — AUTHOR**, with the recommendation that a field equal to its stated default MUST be omitted (the scalar analogue of the optional-empty rule) |
| 4 | **The bound table said eight `NetworkPoint`s per "peering endpoint"; §4.4's schema is singular** | **CONFIRMED, FIXED** — the schema governs; "peering endpoint" struck from the row, with the correction noted in it |
| 5 | Extension-value domain wider than the harness's type model | **CONFIRMED — AUTHOR** — recommended reading: slices preserved opaquely, never interpreted; one sentence either way |
| 6 | No systematic missing-field / wrong-type matrix | **CONFIRMED, FIXED** — E15–E18 seed it; bar 14 makes it standing, with the reason: the schema-aware harness checks *our* positives, not an implementation under test |
| 7 | Valid role overlaps lacked must-accepts | **CONFIRMED, FIXED** — D14 (witness∩verifier), D15 (one verifier, both subjects), D16 (uniform `nominated_by` is wire-valid; the split is client policy, verified at `light-client-requirements.md` §1.0) |
| 8 | The equal-seqno rule tested on one of two decoding paths | **CONFIRMED, FIXED** — a **generated** `SignedLocator` conflict partner (same subject, same `[5,100]`, different path), harness-verified; V12, plus D17's replace/ignore_stale reading of the existing pair |
| 9 | Output hashes recorded but not gated; dependencies unrecorded; the ML-DSA independence env-dependent | **CONFIRMED, FIXED** — the generator now compares regenerated outputs against the stored pins when specs and tools are unchanged and refuses drift without `--accept-output-change` (tested: clean rerun reproduces byte-for-byte); dependency versions recorded in the pins; the harness already names which ML-DSA path ran in its output |
| 10 | Generic CBOR negatives exercised two parser branches | **CONFIRMED, FIXED** — E19 (non-shortest length headers), E20 (indefinite across major types), E21 (invalid UTF-8), S24 (non-canonical map inside the protected-header bstr) |

**Rulings on the ninth-review questions (2026-09-01)**: **a field equal to its
stated default MUST be omitted** — §1 carries the rule as the scalar analogue of
the optional-empty rule, the `NetworkPoint` comment names its instance, writing
7431 out is malformed (E22), and the positive constructor asserts it — and
**unknown extension values are opaque encoded slices, preserved and never
interpreted**, in the author's words *uninterpretable state kept for a reader
that may understand it later*; any deterministic CBOR item is admissible, and
D18 is the fixture that fails typed-model reconstruction. The vectors' author
queue is empty across all nine rounds.

---

## Campaign (a) opens: the §4.5.1 construction is real (2026-09-01)

Author cleared item 2 to proceed and corrected item 3's framing to the **bundle
model** — the fixture is the records one participant hands the other, §5.4's own
words; the archive-walking phrasing re-imported a reading deleted during 0.8, and
the bar now says bundle. First build slice delivered:

- **The real §4.5.1 digest-list construction** in the generator: seven labelled
  salted disclosures, `digest = SHA-256(0x00‖D)`, `root = SHA-256(0x01‖digests)`,
  deterministic vector salts stated as such.
- **The formation record is now a fully integrated known-answer object**: real
  root in field 8 (the synthetic-root caveat is gone), its type-5 envelope (two
  participants, four entries — the harness's schema table now handles type 5's
  dynamic signer set), and **three verified `PresentedRecord`s** — full, partial,
  minimal — all recomputing to the same root under the same signatures.
- The harness independently recomputes every digest, the root, field 8, and all
  three presentations; 25 checks, all passing (5 envelopes, 18 signatures).

Next slices: the history bootstrap (records with `required = 0` growing *n*), the
handed-bundle fixture, and the normal-subtype target record with divergent
orderings and real responses.

**Author correction (2026-09-01): the bundle is curated, not chained.** *"You can
cherry-pick whatever PoP transactions you wish from any of your series and do not
have to expose the intervening transactions."* The bar-3 rewording of the same day
had kept the chaining requirement ("records must chain; a missing middle record
fails to connect") — which was wire §5.4's own text, and it was wrong: the
traversal apparatus (reachability from the committed back-pointer, DAG dedup
across merge paths, incomplete-on-gap, boundary pruning, the backfill argument)
described machinery the design does not want. **§5.4 is rewritten**: a bundle is a
set of individually verifiable records from any series; qualification is
per-record (verifies alone, subject a participant, in-window); duplicates count
once by txid; understatement is free and self-defeating (§5.5's absent-slot
posture one layer up); overstatement impossible; and **the record's responder
slots are what pin the bundle** — selection recomputed over any other bundle
visibly fails. Design §8.1.2's "chain to the commitment" clause is corrected, the
light-client count-*n* bullet rewritten around verify-before-signing, and the
fixture plan sheds its diamond, committed-predecessor and bundle-minus-one cases,
which tested the dissolved model. This also simplifies the upcoming build: no
chain bootstrap is needed for bundle purposes — records need only exist and
verify.

---

## The verifier-selection redesign: recognition replaces recomputation (2026-09-01)

**Author's redesign, applied in full after a four-question clarification round.**
The PoP is a connection between individual users, outside subnet boundaries; its
central property is that the participants are confident in who they met and can
prove the meeting to someone who has met the same counterparty. Deterministic
selection died twice over: a hash-rank pick over a curated bundle is the
curator's pick, and the determinism only ever served a distant audience that
could not check it without the full history the design withholds.

**The rulings**: *trust horizon* is the operative term for the two-edge walk
(self-centred, not a fixed set; *Dunbar Org* stays for thesis and theory);
reachability is a second two-edge walk over meetings and horizon-mates, in four
descending tiers, judged locally and provable to nobody; "Go Fish" candidate
proposal is client conversation, carried by no wire object; Witness fields 4–5
and body key 7 are retired (numbers not reused); `VerifierResponse` gains
field 10 `selection_basis` (0 known / 1 reachable / 2 discretionary), the
selector's claim, covered by field 9.

**Applied**: wire §5 rewritten end to end (recognition ladder, reasonableness
criterion, qualification and window, curated pool, what the record carries;
§5.6 consent unchanged; §5.7 reframed holder-relative-by-recognition); design
§8.1.2 rewritten; the ceremony steps, §7.4.1's oracle defence (per-query grants,
counters, bundle curation replace the dead scatter argument), §7.3's
shared-identity analysis (recognition replaces the q/k randomness math), §10.1's
tamper-evidence argument (re-grounded on the standing presentation, with the
two presentations explicitly distinguished), the nominated_by rationale, both
reads-table rows, the §21 parameter rows (seed window deleted; "verifiers
sought" as reasonableness), P18's bound, and the front matter. Light-client:
witness duties lose the nonce bullets and keep the clock check; the selection
duty becomes pick-by-recognition-and-say-which; the bundle bullet reads *n* as
the counterparty's claim. §1.1's hash-tag family note records the three retired
tags; §3.3's reroll rationale notes there is no sample left to reroll.

**Vectors**: the nonce/commitment/seed/rank vectors retired;
verifier-selection.md is now the reasonableness criterion and window
boundaries; formation record rebuilt without key 7; V1, V5, T23, C3 and R14
tombstoned (ids not reused); T27 (selection_basis closed enum) and the
`weight[subject]` result dimension added; V7 reframed; bars 2/3/4/11 rewritten.
25 harness checks pass; references at zero.

## Cycle 2, pass 0.1 — external-claims audit (2026-09-02, different model family, high effort)

Reviewer consolidated the five documents, checked externally-checkable claims
against authorities, and self-declared the audit **partial**: main standards,
crypto, mobile-platform, networking, biometric and expressly-unsourced blocks
complete; no final line-by-line pass for incidental assertions. 48 rows:
38 CONFIRMED (no action), 4 CONTRADICTED, 6 PARTIALLY CORRECT, ~10 UNVERIFIABLE.

**CONTRADICTED — all verified against text and applied:**
- **RFC 6177 "/64 assignment floor"** (design §4 deferral package). Verified:
  RFC 6177 makes no formal size recommendation; /64 is the least it contemplates.
  Reworded: "the smallest end-site assignment RFC 6177 contemplates (it declines
  to fix a formal size and expects most sites to receive more)".
- **BGP NLRI "trailing bits zero"** (same passage). Verified: RFC 4271 calls the
  trailing bits irrelevant. Zero-padding now owned as this document's
  canonical-form rule; the section cite corrected §3.3 → §4.3 (UPDATE format),
  a second defect the reviewer did not flag.
- **RFC 4787 "30–120 s"** (wire §8.2 heartbeat comment). Verified: RFC 4787 REQ-5
  is ≥2 min minimum, 5 min recommended default. Comment now carries the real
  figures; the argument *strengthens* — a 3600 s heartbeat outlives even the
  recommended binding. The MQTT clause (PARTIALLY CORRECT row) fixed in the same
  block: "MQTT 5.0 calls keep-alive application-specific, typically a few
  minutes".
- **PQXDH signed-prekey compromise** (design §14.2.4). Verified against Signal's
  PQXDH spec: without a one-time key, retrospective exposure needs IK_B, SPK_B
  and PQSPK_B together, not the signed prekey alone. Sentence now reads
  "long-lived private keys — identity key, signed prekey and PQ prekey together".
  The prefetch trade-off argument survives; its stated cost was overstated.

**PARTIALLY CORRECT — two more applied, three closed without change:**
- **iOS current-BSSID** (design §7.6.1): applied — entitlement alone is not
  enough; now "an entitled app meeting further conditions — precise-location
  authorization among them". Strengthens the passage's own argument.
- **Jury nullification "no statute / no judge"** (V7, §3.1.1): applied — the
  categorical was false (New Hampshire et al.); now "Almost nowhere does a
  statute grant this or a judge instruct a jury about it". The power-not-right
  frame the vignette rests on is confirmed by the reviewer.
- **7±2 / Dunbar** (§3.2): no change — the text already carries the exact hedge
  requested ("does not establish a universal figure", "design heuristic, not an
  empirical constant", "vernacular estimate").
- **Face ageing "24 months"** (§7.5.1): no change without ruling — already
  registered as **A18** with the §20.1 advisory "recast qualitatively unless a
  longitudinal study is cited"; the reviewer's finding confirms the register row.
  Recasting touches the two-year retention tier it motivates. **Queued for the
  author.**
- **Biometric legal exposure** (§7.2): no change — reviewer confirms the general
  proposition; the text claims nothing jurisdiction-specific.

**UNVERIFIABLE (~10 rows)** — no change: every one is already a §20.1 register
row (rows 1, 7.1.4, 7.1.5, 7.1.5.1, 7.1.6, 7.1.6.3, 9.6, 11.1.1; assumptions A9,
A14, A15, A18, A19, A26), which the reviewer acknowledges ("appropriately
self-identified by the document as unsourced"). The register is the mechanism
working as designed; elevating any row needs a named measurement, not wording.

**Carried forward**: the reviewer's own caveat — the audit is not exhaustive for
incidental external assertions. A completion run of 0.1 remains available before
0.2.

**Follow-up found by the pass itself**: applying the RFC 4271 §4.3 correction
exposed a checker blind spot — the reference checker read RFC-prefixed section
cites as internal references, so all six such cites in the set had been passing
by collision with our own headings or not at all. The checker now classifies a
§N preceded by "RFC nnnn" as an external citation, and all six were verified
against their RFCs by hand: five correct; **one wrong — wire §2.2 cited
COSE_KeySet to RFC 9052 §9, which is "CBOR Encoding Restrictions"; the
definition is §7 "Key Objects"** (verified against the RFC text). Fixed. This is
a classification correction, not an exemption: the six cites are now positively
checked rather than skipped.

**A18 ruled (2026-09-02): kept.** The author: it is rhetoric, not proof — the
reasoning is heuristic, approximate, and his. A threshold is needed, two years
is a common one, and it is close enough on a number of axes, of which face
ageing is only one. §20.1's advisory row now records the ruling in place of its
"recast qualitatively" advice; A18 remains a declared assumption unchanged, and
§7.5.1's body already states the same frame ("a compromise, not an optimum on
any axis... chosen, not derived"). The 0.1 queue is empty. The author re-runs
0.1 at low-moderate effort next.

## Cycle 2, pass 0.1 — second run (2026-09-02, low-moderate effort)

The author re-ran 0.1 at lower effort. ~50 rows: 31 CONFIRMED — including both
corrections from the first run, which now pass audit — 1 CONTRADICTED, 9
PARTIALLY CORRECT, 10 UNVERIFIABLE. Every actionable row verified against the
text before edit.

**CONTRADICTED, applied**: resource-requirements §3 called all four HTTP header
values `token`/`base64url`; RFC 9110's `token` excludes comma, so a
comma-separated list of tokens is not itself a token. Reworded: `rhtn-roles` is
RFC 9110 list syntax, each element an HTTP `token`, the comma the list
delimiter. The no-escaping property survives; the misclassification does not.

**PARTIALLY CORRECT, applied (3)**:
- wire §1's "fixes ... float handling": RFC 8949 §4.2 fixes float
  *representation* and leaves residual float semantics to protocols — now stated,
  with the observation that no schema in the document admits a float, so the
  residue is moot here.
- design §3.2's "vernacular estimate of around 200": the canonical Dunbar figure
  is ~150 (original 95% interval roughly 100–230). Corrected; 221 sits inside
  the original interval, which the anchor paragraph now says — the
  order-of-magnitude argument is unchanged and slightly strengthened.
- design §5.2's "no production path" for browser wasm: hedged to "today" — the
  passage already carried its own evidence and "until that closes".

**PARTIALLY CORRECT, no change (5)**: UWB-strongest (registered 7.1.6.3);
liveness motion/parallax (registered 7.1.1 — the reviewer's critique is the
register row verbatim); regional gateways (registered 10.6.5 and 7.1.6);
24 months (ruled 2026-09-02, kept); WebAuthn — the standards half is confirmed
and the broker inference is the design's own, labelled "in practice".

**UNVERIFIABLE (10), no change**: all registered — §20.1 rows and A9, A14, A15,
A17 (fuzzy commitments), A19, A24 (records per decade); "mostly in cloud
datacentres" is self-labelled *Expected deployment* and registered (4.3/4.4);
0-RTT "saves battery" registered (11.1.3). The reviewer's own observation
stands: the overprecision class is exactly the §20.1 register's contents.

**Note carried**: the reviewer credits the profile-rule/RFC-rule distinction
with preventing several false findings. Both 0.1 runs are now ingested; the
high-effort run found four contradictions to this run's one, and the two runs'
contradiction sets are disjoint.

## Cycle 2, pass 0.2 — internal contradictions (2026-09-02, high effort)

25 findings: 1 BLOCKING, 16 SERIOUS, 8 MINOR. **All 25 verified against the text
and all 25 held** — none died on inspection. All applied this session.

**The seed-custody cluster (1–5), all in the §7.5.2 complex:**
1. BLOCKING — "gives the other a 32-byte seed" contradicted "never held the
   seed" and the construction block. Resolved on the construction's own terms:
   the subject derives a per-ceremony capture key from a seed only they hold,
   hands the *key* at capture time, and the holder discards it once sealed.
   The KeyGrant (wire §7.3, k_capture only), every security claim, and the
   light client's never-retain duty all pointed the same way. **Flagged for
   author confirmation as the one BLOCKING item.**
2. Light §1.1's "and not after" forbade the extension design §7.5.2 expressly
   permits; the bullet now carries the default-not-rule model.
3. "Seeds die with the device" vs "back up seeds": both design and light now say
   *absent a restored backup*; the §7.5.2 Open question (re-release after
   restore) is untouched.
4. "If A and B never meet again" ignored the key release during A's ceremony
   with C four paragraphs later; now "if A never releases the key again".
5. Wire §7.3's "names the latest" now defers to the design's
   subject's-choice rule, latest as the ordinary case.

**Redesign residue and mode conflation (6–13):**
6. §10.1's pruning argument still traversed "from the committed back-pointer";
   rebuilt on the current model — only in-window records qualify for a bundle,
   so the pruning boundary aligns with qualification. The dead n=1 clause went.
   **Rationale reconstructed from the redesign's own rules; author should
   glance.**
7. Infra §1's "knows the client has it" loosened wire §8.2's sender-history
   test; now states the only omission the wire permits.
8. Infra §11's "entries you own" excluded third-party registrations the same
   section requires holding; now "eligible entries you hold", ownership deciding
   who signs.
9. Appendix A's "without its operator knowing" contradicted §19.6's mandatory
   disclosure; now "without pausing for approval — §19.6's disclosure still
   reaches the operator, as notice rather than a question".
10. rr §1's categorical "reverse proxy" (and design §11.5's) now: gateway —
    proxy where the node carries traffic, broker where it hands off. A
    duplicated sentence in rr §1 died in the same edit.
11. Light §3's "covered by the transport handshake" now names which transport:
    rhtn/1, or the service's own TLS on a brokered connection.
12. §24 step 10's "HTTP/3 over the existing session" contradicted rr §3's
    you-cannot-layer-HTTP/3 rule; now ResourceRequest on rhtn/1, HTTP on the
    outbound leg.
13. rr §9's "sees everything the node sees" now "run outside a sandbox, would
    see" — the baseline that motivates infra §9.2's no-hooks model.
14. §16.6's "an infra node is required to exceed 110 users" inverted §3.3's
    rule; now "infrastructure becomes required only once a branch would exceed
    110 users under two light levels".
15./18. §4's scope bullet said interaction protocol and owner-movement rule
    deferred; both are specified (§22.1, §11.2). Bullet updated.
16. Wire §4.5.2's witness reads-row said "Location only"; design §8.1.1 makes
    the patron the sole recipient and design's own table says the witness
    *contributes* corroboration. Row now **None**. This makes the ten-of-eleven
    sentence (both documents) arithmetically true — the previously-left tension
    dissolves.
17. §14.2.4's Still-to-settle has five items; the preface (twice), P12 and
    §22.2 said four and omitted payload-type demultiplexing. All four sites now
    say five and §22.2 carries the item.

**Stale numbers and cross-references (19–25):**
19. §21's chosen-enumeration listed a forwarding TTL wire §10.1 explicitly
    eliminates; dropped.
20. Light Open's multi-device pointer §23.1 → §23.3.
21. §20.1's section column remapped throughout (7.1.x → current 7.x, 10.6.5 →
    7.6/12.6.5, 11.1.x → 14.1.x, 9.6 → 16.6, 7.2 → 12.2, 7.4 → 12.7.1, 4.3/4.4
    → 3.3, A.1 → B.1, the extension row → `resource-requirements.md` §9;
    6.2.5 and the §1 rows were still correct). **Two duplicate rows found
    beyond the finding** — radio latency and regional gateways each appeared
    twice from separate additions — merged, statuses combined.
22. §23.1's "beyond the seven" → "beyond the current six" with the tombstone
    noted.
23. Wire §4.5.1's "nine for a typical record" → seven, and the proof cost
    four → three hashes (ceil(log2 7) = 3), consistent with "sending every
    digest costs seven" in the same sentence.
24. §20.2's "twenty-six below" → thirty-one; the table runs A1–A31 and §20's
    intro already said 31.
25. Light §1.0's "your verifier sample derives entirely from their nominees" —
    stale selection-era wording — now "the attestation rests entirely on their
    nominees".

Checks after: §21.1 counts eleven parameter rows across its three tables as
§22.2 claims; the wire reads-table counts eleven exchanges with the patron the
sole field recipient; §23.2 clean; residual sweeps for every replaced phrase
return nothing.

**0.2 findings 1 and 6 confirmed by the author (2026-09-02).** On custody: "The
holder has to be trusted to discard the capture key when they act as verifier,
so this is a minimal extension of that trust" — recorded into §7.5.2's
non-compliance paragraph, which now also names retaining the capture-time key
among the defeats. The §10.1 qualification-alignment rationale stands as
rebuilt. The 0.2 queue is empty.

## Cycle 2, pass 0.2 — second run (2026-09-02, medium effort)

Six findings, again disjoint from the high-effort run's 25. **Five held and were
applied; one rejected on verification.**

**Applied:**
1. §16.1 still said "deterministic sample" fourteen lines before its own
   "recognition *is* the selection rule"; now recognition-selected, citing
   §8.1.2.
2. §7.1.1's "requires at least one witness" now carries the formation
   exception wire §3.2 states ("zero witnesses is the formation case and
   nothing else").
3. Wire §3.2's two nonce residues: the future-`started_at` defence is now "a
   witness declining to sign a ceremony whose claimed day its own clock
   contradicts", and the duplicate-witness rationale stands on undefined
   ordering plus independence, the dead seed-input clause removed.
4. The same sentence's cross-reference moved from design §8.1.2 (verifier
   selection; says nothing about clocks) to `light-client-requirements.md`
   §1.0.1, which states the behaviour.
5. rr §3 now says only the second leg "speaks HTTP on the wire", and the
   client→node cell explains the frame embeds a serialized HTTP/1.1 message
   (wire §11) — the schema and the ALPN argument both preserved.

**Rejected — finding 6 (predicate recalculation).** The claimed contradiction
dissolves on reading: rr §7.2's "tuning and having access follow" is design
§11.4's moment one (an operator configuring — §11.4 carries the identical
parenthetical itself), and infra §10.5's "a predicate ceasing to match" is what
happens *at* either sanctioned moment, upon which §10.5 terminates sessions.
Both cited behaviours live inside the two-moments rule; no text says access
follows anything between moments. **Residual question queued for the author,
distinct from the finding**: a score drifting on new *evidence* between the two
moments (no retune, no membership change) leaves the table deliberately stale
until the next moment — nothing says whether that staleness is intended or
whether evidence arrival should be a third trigger. The documents are silent
rather than contradictory.

**The evidence-drift question ruled (2026-09-02).** The author: the span is
narrow enough that recalculation is fine on events, and time-dependent values
recalculate periodically for the whole table rather than continuously; a table
update won't upgrade or close a connection already open — but where the node
itself holds the session, dropping on an authorisation change is mandatory
("if we are already dropping sessions, then make that mandatory"), while a
non-intermediated connection can't be relied on to drop. Applied: design §11.4
now evaluates predicates at four moments (config, membership, evidence arrival,
periodic time pass) with the issuance-and-reconnection enforcement paragraph;
infra §10.2 mirrors the four moments; infra §10.5's drop is stated as mandatory
for node-held sessions and its brokered paragraph carries the can't-rely-on-drop
statement; rr §7's departure bullet already stated the brokered half and stands.
The periodic cadence joins §21.1's needs-measurement table and §21.1.1's
freely-tunable list; §22.2's count moves to twelve. The 0.2 queue is empty
again.

## Cycle 2, pass 0.3 — unsupported-claims census (2026-09-02)

**The reviewer's independent count reproduces the register's own arithmetic
exactly**: 123 unsupported claims, 79 load-bearing, against §20's stated "79
load-bearing... and 123 in total, against the 31 listed in §20.2". The A–E
categories and the SUPPORTING/COLOUR split are the reviewer's overlay. The
reviewer accepts §20's curation frame and its chosen-is-disclosure distinction,
and excludes the derived numbers (18 envelope signers, 36 COSE entries, λ<1/f)
on the same boundary the documents draw.

**Structure of the 79**: 31 = the A-register (all registered); 18 = §21's chosen
operating points (self-declared); 17 = wire §1's ceilings (self-declared "no
capacity study"); 13 = other structural, of which five were already §20.1 rows.
**The true delta was eight claims registered nowhere** (census rows 67, 73–79).
Seven §20.1 rows added covering them (the two §1.3 SSO claims share a row):
cheap-to-manufacture, trust-emanates, disaggregation-defeats-bulk, the SSO
adoption path, nominee inactivity (§7.1.1), the compromised-resource confinement
conclusion (§11 + rr §1), and rr §4.1's any-scale federation claim. §20.2's
A-table is untouched at A1–A31; the census sentence needs no change since it
counts claims, not rows.

**One derivation applied**: wire §8.1.1's greased-id collision "negligible
probability" is now stated as k/2^64 per draw against a peer holding k
unpublished capabilities — computed from the section's own 64-bit id space, in
the style of its existing 2^32 birthday bound.

**Left alone deliberately**: the 32 SUPPORTING and 12 COLOUR rows. The colour
class is the author's rhetoric (the A18 precedent: rhetoric, not proof, and
his); the supporting class repeats claims already registered or self-evidently
labelled. Row 91's ten-of-eleven "enumeration needed" is satisfied by the
eleven-row reads table itself, verified during 0.2.

**0.3 elevation rulings (2026-09-02).** Trust-emanates is not an assumption:
"a primordial design parameter — the network concept was created to reify a
model of observer-relative trust." The §20.1 row added during 0.3 ingestion is
removed as a mischaracterisation; §1.2.1 now carries the author's framing in
place of any register entry. Compromised-resource confinement "is load
bearing": elevated to **A32** (numbers never reused; A-table now A1–A32), dual
with its §20.1 row, so the duals sentence reads seven (A14–A19, A32) and the
census sentence reads 32. The 123/79 strict counts are unaffected — they count
claims, not rows. The 0.3 queue is empty.

## Cycle 2, pass 0.4 — parameter census (2026-09-02)

Full taxonomy over every operating parameter, bound, width and knob
(ASSERTED / DERIVED / BORROWED / UNSET — the classifications are the reviewer's
overlay and required no document changes; the derived and borrowed calls match
the documents' own basis labels, λ<1/f and the Kerberos anchor included).

**Two numeric conflicts found; both verified and fixed:**
1. Preface said "Eleven parameters remain unset" against §22.2's twelve and
   §21.1's twelve rows. **An incomplete sweep of ours**: when the periodic
   re-score cadence became the twelfth parameter, the §22.2 count was updated
   via a search for "Eleven unset" — the Preface's different phrasing ("remain
   unset") escaped it. One phrasing is not a sweep, again. Fixed to twelve.
2. Wire §12's minimised-presentation figure said "at most ~288 B" — nine-leaf
   residue (9 × 32) surviving the 0.2 nine→seven fix, which corrected §4.5.1
   (224 B) but not the size-estimate prose. Fixed to 224 B. The adjacent
   ~112 B salts figure was already correct (7 × 16).

**Verified non-conflicts, agreeing with the reviewer**: CatalogReply's 111
against the 127 frame ceiling (chosen bound vs calculated maximum), and the
4 KB prekey global ceiling against the unannotated local bstr.

**Flagged for the author, not applied**: §21.1 is not exhaustive of unset
quantities — the witness clock tolerance (lc §1.0.1), the one-time-key issuance
rate limit (infra §6 / wire §7.8), the KeyGrant buffer bound (wire §7.3),
general client cache lifetimes (lc §4) and attach timeout/backoff (wire §8.2)
are named-but-unset in the subsidiary documents and absent from the checklist.
All five are client-local policy; whether §21.1 should enumerate them or its
scope statement should disclaim them is a curation call.

**0.4's curation question ruled (2026-09-02): scope sentence, not enumeration.**
§21.1 now states it lists the design's parameters and that component-local
bounds live with their mechanisms — "keeping two tables in sync is just one
more surface for error." The author stated the general doctrine: what is
optional or variable for a software component favours inclusion in its own
document, with network-design saying what is always true of the whole, except
where an implementation-variable characteristic impacts the security model or
similar and must be weighed in the design too. Recorded in
authoring-conventions "Where invariants live". The 0.4 queue is empty.

## Cycle 2, pass 0.5 — fragile rules (2026-09-02)

18 rules flagged as identifier-bound. Verified each against the "Where
invariants live" test: does a role-form statement of the invariant exist
anywhere in the set?

**Eleven needed no change — the invariant already precedes the encoding**, in
most cases in the document's own role-rule-then-"Present encoding" idiom:
countersigning (#1, design §6.4 states the authority-must-not-gate-unobserved-
evidence rule verbatim), disavowal codes (#12, wire states the bounded-adverse-
attestation rule verbatim), Attach identity binding (#18, wire §9.1's mutual-
authentication paragraph), recovery binding (#13, design §9.4's italic
invariant), 0-RTT Attach (#17, explicitly "one instance of §9.2's general
rule"), the KeyGrant/late-response query bindings (#14, #15 — "a query the
subject countersigned" is already role language), series reissue (#7 — "series"
is a schema concept whose rename would sweep the set regardless), pruning (#8 —
lc §2 grounds it in §10.0's kinds-of-history), tenant isolation (#11 — the
bullet's rationale is role-stated), and resource-registration classing (#4 —
covered by the archive criterion added under #3).

**Seven anchors added:**
- #2: §9.2's "departure is not blockable" now cites §6.2's standing rule —
  formed bilaterally, ended unilaterally; the holder gets no gate over the act
  that ends the authority. (The old veto-exemption formulation left the
  document when the veto did; this restores the surviving invariant.)
- #3: §10 now states the archive criterion above its list — transaction-hood is
  earned by durably changing topology or constituting portable evidence of a
  meeting; the list remains as current state.
- #5: §15.2's memo restriction now states its criterion — only membership slots
  this chain governs; outside relationships are not its business to summarise.
- #6: §7.5.2's decryption-failure constraint now carries "a verifier reports an
  identity judgment only where it ran the comparison that supports one".
- #9: infra §1's KeyMaterial bullet now opens with the invariant — a client
  must be able to authenticate every failover peer before it needs one.
- #10: infra §11's registration bullet — possession of a signed entry is not
  authority to install it; only the owner's live authenticated channel is.
- #16: wire §7.7's referral check now leads with strict-forward-progress before
  the `advances` encoding.

**0.6 gains phase 2 (2026-09-02).** Vectors join the pass as acceptance data
delivered after the clean-room attempt, never before, and never the tools
directory — generate.py is a reference implementation in disguise. Divergences
classify as implementation bug (noise), spec ambiguity (merges into phase-1
findings; the vector is evidence of under-determination, not its resolution),
or vector defect (a finding against the generator: verify, fix, regenerate).
Prompt recorded in review-plan.md §0.6.

## Cycle 2, pass 0.6.1 — implementation attempt, adoption + recovery (phase 1, 2026-09-02)

Clean-room Rust attempt. Two claimed conflicts and four UNSPECIFIED questions;
every claim verified against text.

**Conflict 1 — real, wire fixed.** Wire §4.1's prose said "`Recovery` is
present on a rotation and absent otherwise... every rotation carries both
halves", against its own schema comment ("present iff this is a recovery
adoption") and design §9's explicit case split: "A plain rotation carries
nothing... A recovery adoption is the other case and publishes the link
deliberately." The word *rotation* was doing double duty. The prose now reads:
present iff the adoption claims a predecessor's history; a plain rotation is an
ordinary adoption on the wire and carries no Recovery; the no-variants rule now
binds the block itself. The reviewer's precedence resolution (Option<Recovery>)
was correct.

**Conflict 2 — real, wire fixed.** Design §8.1's schema says `template_version:
uint16`; wire's VerifierResponse field 6 said bare `uint`. Now bounded
0..=65535 with values above malformed, citing design §8.1.

**UNSPECIFIED #1 — applied as a rule.** Initial counter for an
adoption-established series was unstated; a reissue explicitly opens at 0.
Wire §4.1's locator comment now states: the adoption's seqno opens the
relationship's series at counter 0, the reissue rule. All three adoption
fixtures carried arbitrary counters (42, 1, 42) and were regenerated at 0 —
which also makes the fixture narrative coherent (later locators at 42/100/max
all advance from the opening). Harness gains the check. **Author should
confirm the rule**; interop requires one and the reissue precedent selects it.

**UNSPECIFIED #2 — applied as a rule.** Neither response array (presence field
5, Recovery field 2) had an ordering rule, against a unanimous house precedent:
witnesses, merge heads, disclosure labels and catalog populations all sort
ascending. Both arrays now sort ascending by verifier keyhash ("the witness
rule, one set one encoding"). Fixtures unaffected (no multi-response array
exists yet). **Author should confirm.**

**UNSPECIFIED #3 — genuine design gap, queued for the author.** Recovery
responses reuse VerifierResponse: field 10 is "the selector's claim" and
query_id derives from a VerificationQuery carrying a querier — but recovery
defines no selector or querier role. In a ceremony the counterparty selects; in
recovery, is the querier the adopting patron, or the recovering subject
approaching prior counterparties directly — and what does selection_basis mean
there? Two implementations independently producing recovery evidence would
populate incompatible signed bytes. Not invented; the reviewer likewise
stopped at the module boundary.

**UNSPECIFIED #4 — no change.** KeyMaterial omission is deliberately local
policy; an archived object faces many future recipients, so the sibling rule's
sender-history test cannot apply. Both presence and absence are conforming and
the missing-key third verification state absorbs the failure mode.

**(c) type decisions** — no document changes: the constrained ones are
supported as the reviewer found; the rest are legitimately local. **(d) crate
maturity** — matches design §5.2's existing unaudited-crates caveat; no change.
Phase 2 (vectors as acceptance data) not yet run for this target.

## Cycle 2, pass 0.6.1 — phase 2, vectors as acceptance data (2026-09-02)

Run against the pre-0.6.1 documents. **No vector defects in the examined
targets; no new spec ambiguities; one implementation bug** — the reviewer's
typed Locator dropped the nested unknown extension, exactly the failure class
the top-level-AND-nested extension vector was built to catch. First independent
confirmation that a fixture catches its intended bug in a real second
implementation. All four adoption vectors and eighteen negative/must-accept
rows hand-traced to agreement; V9/V9b correctly outside the structural
operation; the crypto octets not independently reproduced (same-preimage
equivalence argued instead), as the phase-2 protocol permits.

**One reconciliation ours**: must-accept row D5 ("verifier responses in any
array order") predated the ordering rule this pass's phase 1 produced, and its
rationale cell cited a §5.5 non-canonicalisation sentence that never existed.
D5 retired (id not reused); T28 added — unsorted responses are malformed under
the ascending-verifier-keyhash rule.

**Coverage gap confirmed and carried**: no complete positive Recovery adoption
exists in the corpus — query → consent → hybrid response → Recovery block
remains the queued canonical-bar work, and it is exactly what would exercise
the open selector/querier question. Blocked on that ruling; unblocking it
builds the vector next.

## The recovery selector ruled by derivation; the Recovery vector lands (2026-09-02)

On the author's go-ahead, UNSPECIFIED #3 was resolved from design §9.1's own
steps rather than by invention: "run §7.3 in reverse" — the subject meets a
prior counterparty in person, who recognises them. **The counterparty is its
own querier** (the subject stands in front of the party answering; no third
role exists), the meeting opens as a ceremony (§7.5.2 already calls a
post-loss recovery exactly that), whose countersigned pre-commitment binds the
query, and **selection_basis MUST be 0 (known)** — a recovery verifier is by
definition a prior counterparty. Design §9.1 carries the mechanical paragraph;
wire §4.1 carries both rules as consistency bullets. **Derived, not
author-worded — flagged for his review.**

**Canonical bar 4's Recovery half is built.** transactions.md now carries the
complete positive recovery adoption: alice2 recovers alice, adopted by bob,
carol (prior counterparty via the formation ceremony) verifying. The chain:
VerificationQuery (querier = carol) → query_id → Ed25519 consent by the NEW
key over the raw query_id → hybrid COSE_Sign verifier response over fields 1–8
and 10 → [prior, new, patron] successor proof by the OLD key → Recovery block
→ adoption whose locator opens series 11 at counter 0, presenting alice's old
chain head, envelope-signed by the new key and the patron only. alice2 joins
the identity set (12 identities). The harness gains seven checks — query_id
recomputation, all bindings, querier-is-verifier, and real signature
verification of consent, both response entries and both successor entries —
**28 checks, ALL PASS**. One generator bug caught during the build (an
unwrapped txid in body field 7 leaving 21 stray bytes) — caught by the
harness's own canonicality gate, which is the division of labour working.
records.md's queued list and README bar 4 updated; still open there: the
presence/classical form and the selection_basis matrix, which need bar 2's
normal record.

**The querier derivation confirmed by the author (2026-09-02): "Yes."** §9.1's
counterparty-is-its-own-querier stands as ruled. **New standing practice, same
date**: close all open work before proceeding with further review passes — the
author hopes to make cycle 2 the final cycle. Open work therefore precedes 0.6.2+.

## All open work closed (2026-09-02): the canonical bar is met

Per the author's ruling ("close all open work before proceeding with the
reviews"), the remaining canonical-bar items were built in sequence:

- **Bar 2** — the normal presence record: alice–bob, sixteen witnesses (the
  ceiling), 36-entry envelope, three orders deliberately different, three
  classical responses covering the selection_basis matrix, real disclosure set,
  its own presentations. The optionals adoption's field 8 swapped to it (V9
  pass; mismatch moved to the divergence adoption, V9a).
- **Bar 3** — the curated-bundle fixture over real records (two new
  alice–carol records), harness-recomputed arithmetic.
- **Bar 4** — both authentication forms complete with the recovery adoption
  (prior) and the normal record.
- **Bar 5** — presentation sets for both records plus the six-member negative
  disclosure family in the corpus.
- **Bar 6** — `corpus.json`, format `rhtn-test-corpus/1`: 167 entries, four
  classes (bytes / unit / trace / context), structured expects with rejection
  layers; the harness executes the encoding layer of every byte entry.
- **Bar 7** — the boundary sweep: every declared bound at and past it as
  bytes; two over-identity-set counts and two 100 KB frame bounds as unit
  recipes.
- **Bar 8** — every domain-separation context signed, with the cross-context
  substitution family (S24).
- **Bar 9** — `messages.md`: 17 frames, 14 replies/payloads, traces TR1–TR8.
- **Bar 10** — wrong-signer analogues for every standalone object plus the
  wrong-signer envelope.
- **Bar 11** — finalization must-accepts (lone no-match; key 5 absent).
- **Bar 12** — the enum/extension matrix: ten closed-enum rejects, the open
  namespaces' must-accepts.
- **Bar 13** — six unexercised optionals found and covered.
- **Bar 14** — the schema-shape matrix: missing-required and wrong-type
  across the core schemas, plus keyhash width edges.

**The harness stands at 64 checks, all passing**, every signature in the suite
real and verified. Identities grew to 21 (alice2, w4–w16). One §1 scope point
clarified along the way: the empty-array prohibition is §1's OPTIONAL-field
rule — a required `[ * … ]` field may legitimately encode empty (the
formation's corroborations), while `[ + … ]` and `1*8` minima make emptiness a
schema violation. "Open for the author" remains **nothing**. What canonical
status still awaits is unchanged in kind: an independent implementation
reproducing the whole suite — and 0.6.1's phase 2 has begun exactly that.

## Cycle 2, pass 0.6 — implementation attempt, resolution (phase 1, 2026-09-02)

Clean-room Rust attempt on target 2 (resolve a locator against an anchor
table). **Structurally clean: no wire fields added** — no consumed-prefix, no
target-type bit, no arrival equation; progress is local state and arrival is
the `ServingInfra` reply, exactly the §7.7 design. Eight behavioural
questions, thirteen type decisions; every claim verified against text.

**No change (deliberately local or already specified)**: U1 — the
anchor-ingestion model is the implementer's stated choice by design (infra
§4.1 "state plainly which model you implement"; the reviewer chose the
unverified gossip cache and said so); U3 retry policy; U5
malformed-endpoint handling; U6 higher-anchor re-resolution is the caller's
(design §12.3); U7 cache TTL (§21.1); U8 pin-timing (local, and the
provisional-until-handshake choice is sound). T1–T13 all legitimately local;
T4's warning about reconstructed KeyMaterial bytes is exactly why keys.md
carries the worked canonical-encoding example.

**One reviewer divergence, no spec change**: U4's "never dial a post-anchor
hop unauthenticated" is stricter than the design. Wire §7.7.3 states the
posture explicitly — a referrer's identity is not what protects the requester,
a hostile chain costs a failed dial, disclose nothing beyond the query to a
party you cannot authenticate — and its sibling-difference callout
anticipates precisely this confusion. The implementation's MissingKeyMaterial
failure at referral hops would strand resolutions the design completes.

**Two wire nits found via U2/U4, fixed**: `AnchorEntry` field 2 said bare
`[ + NetworkPoint ]` against §1's ceiling of 8 — now `1*8` with the ceiling
noted; `Referral` field 2 and `AnchorEntry` field 2 now state the
publisher's-preference-order semantic the other endpoint lists already
carried; and `Referral` field 4's comment — the likely source of the
divergence — now says CAN, an option not a requirement, with the
disclose-nothing rule and the terminal-ServingInfra distinction inline.

**Crate maturity**: consistent with design §5.2's caveats; the reviewer adds
that rustls's AWS-LC provider prefers X25519MLKEM768 by default (the profile
must restrict rather than accept defaults — rr/wire already require
offering only that group), that AWS-LC's WASM support is experimental
(emscripten, not browser wasm32), and that browser QUIC remains
relay-experimental — all inside §5.2's "today" hedge. No change. Phase 2
(the resolution messages now sit in messages.md and the corpus) not yet run.

## Cycle 2, pass 0.6 — resolution target, phase 2 (2026-09-02)

Run against the corpus and vectors as promoted. **Zero implementation bugs,
zero vector defects; one SPEC AMBIGUITY, correctly cross-referenced to phase
1's U1** — and on verification the defect was the fixture's expectation, not
the spec or the implementation: `N-wrong-signer-anchor` gave an unconditional
`reject` while infra §4.1 expressly permits the unverified-gossip-cache model,
under which a wrong-signer entry is undetectable at ingestion and surfaces at
first authenticated contact. **The entry now carries its precondition** ("the
named key is pinned") and a note naming both permitted models.
`N-wrong-signer-currency` gained the treat-as-absent note (TR8's posture:
currency gates trust, never connectivity).

**Coverage observation accepted and closed**: the suite had the positive
advances=2 referral and no malformed-progress counterpart. Added
`N-referral-advances-0` (bytes; MUST be ≥ 1) and `CTX-referral-overshoot`
(context — schema-valid bytes whose defect is relative to the request's
remaining path). U2/U3/U5–U8 are local policy and correctly not fixtures.

The NOT-COVERED classifications (EndpointRecord maintenance, raw-byte
decoding, COSE-shape checks) reflect the phase-1 implementation's deliberate
todo!() boundary, honestly reported rather than credited — the phase-2
protocol working as written. Harness: 65 checks, all pass.

## Cycle 2, pass 0.6 — implementation attempt, presence validation (phase 1, 2026-09-02)

Clean-room Rust attempt on target 3. **The reviewer independently reproduced
the whole redesign**: no seed to restore, the formula as reasonableness never
a gate, absence as the encoding, the disclosure construction hashed over
received bytes, and a multi-dimensional report instead of a boolean. Their
monotonicity reading (current started_at against predecessors' effective
times) matches §3.3 exactly. Six UNSPECIFIED questions; four produced rules,
two were already local.

**Ruled and applied (each from existing precedent or determinism):**
1. **Retired numbers are tombstones** — a decoder meeting body key 7 or
   Witness keys 4/5 REJECTS. Derived from the two standing precedents (§4's
   type 6, Scope's tag 3): an unknown key is one the schema never assigned; a
   retired key is one it remembers. T29 added; two byte fixtures in the
   corpus.
2. **Response ordering ties break by ascending subject keyhash** — D15's
   one-verifier-both-participants case was a legal tie with no rule, the most
   wire-significant gap found (same set, different txids). §4.5, T28 and D15
   updated; harness checks the pair sort.
3. **A Channel kind may repeat** — a retried channel is two measurements,
   both evidence; no uniqueness rule, now stated rather than inferable.
4. **A Corroboration MUST name a field-4 witness** — its authority is its
   maker's envelope signature over the disclosure root, so a non-witness
   corroborator attests nothing; checkable only where location is revealed.
   Unit-recipe fixture registered.
5. **Bundle carriage and size (#5, #6)**: correctly local — the bundle rides
   the ceremony channel; §5.4 now states it has no protocol ceiling and that
   truncation is a visible local act (it changes n).

**One divergence resolved in the fixture's favour**: the reviewer read normal
field 5 as required-with-empty-array for zero responses; the shipped
finalization fixture omits the key. §4.5 now states the rule — fields 4/5 are
subtype-conditional (`?`/`+`), and zero responses OMIT key 5, §1's one-
encoding principle extended to a subtype-conditional field.

**Type decisions (25)**: all legitimately local; the closed/open enum split
matches the spec's registry declarations throughout, including the
deliberately-open location-method registry. **Crate maturity**: consistent
with §5.2; the additions (RustCrypto's fixed rc-series advisories, the
rustls-WASM-provider issue closed as not-planned) sit inside the existing
caveats. ML-KEM correctly identified as off this code path.

## Cycle 2, pass 0.6 — presence target, phase 2 (2026-09-02, version skew)

The author purged the first phase-1 response early; phase 1 was re-run against
the same (pre-ruling) documents and phase 2 then saw the CURRENT vectors
against the OLD spec. The skew is itself informative: all three spec-level
divergences are this morning's rulings read from the other side, and each
lands as independent confirmation that the ruling was needed.

- **V1 (vector defect: absent key 5)** — correct against the old schema,
  where field 5 was unmarked-required and §1's empty rule explicitly excludes
  required fields (the reviewer's quote verified). The morning ruling made
  fields 4/5 subtype-conditional with omission mandated for zero responses, so
  under the current spec the fixtures are well-formed and the bundle
  arithmetic (n = 4) stands. Their corrected figures apply only to the
  superseded reading.
- **A1 (retired keys)** — their R14-vs-T29 catch was real: R14's tombstone
  note, written 2026-09-01 under the preserve-as-extension reading,
  contradicted 2026-09-02's tombstone ruling. R14's note now records the
  supersession. The wire carries the reject rule; the suite is internally
  consistent again.
- **A2 (ordering tie)** — the ruling the morning pass produced; T28 was not
  "inventing" the secondary key but encoding it. Current wire §4.5 states it.
- **I1 (implementation bug: silently skipped missing-key embedded
  signatures)** — the reviewer's own, correctly classified, and a live
  demonstration of V3's unverifiable(key) dimension earning its place.

Everything else — the disclosure roots across all six presentations, the
formula table, the exclusive window and 24-hour boundaries, the seeding
retirement, fourteen R/T/D semantic rows — hand-traced to agreement.

## Cycle 2, pass 0.6 — implementation attempt, attach + failover (phase 1, 2026-09-02)

Clean-room Rust attempt on target 4. The attach path held up well: no fields
added, the UI/wire split honoured (degraded visibility stays a product
obligation), the 0-RTT rule, sibling-pin safety, whole-list replacement, and
the requirement-classification table all match. Five UNSPECIFIED questions.

**No change (expressly local)**: #1 timeout/backoff; #2 endpoint-selection
strategy (the lc floors — alternatives-not-first-failure and
refusal-binds-the-node — correctly identified as the only constraints).

**Ruled and applied:**
- **#3 — a primary's refusal does not open sibling failover.** Derived from
  the enumerated triggers (unreachable at attach; three missed intervals): a
  refusal is an answer, not an outage, and siblings are not a channel for
  overriding it. The reviewer's assumption agreed, and flagged it as their
  most important open question — now wire §9's text. Trace TR9.
- **#4 — one sibling's refusal forecloses that sibling alone**; the sweep
  continues in order. The reviewer's assumption agreed. Wire §9; trace TR10.
- **#5 — AttachAck field 4 is U64 RANGE**, stated so no implementation
  narrows it by inference (the reviewer's exact worry: a u32 reading rejects
  what a u64 reading accepts).

**One latent implementation divergence anchored in prose**: their heartbeat
sketch accepts only the exact expected counter, so after a single lost beat
every subsequent one is ignored and a live server fails over — TR7's rule
read backwards. The Heartbeat comment now states gap-detection is for
information, never liveness: any not-yet-seen counter resets the clock, and
failover counts intervals. Phase 2 would have caught it against TR7; the
prose now says what the trace encoded.

**Type decisions**: all sound, including the deliberately-open integer-first
frame-type decode (a closed deserializing enum would tear down sessions on
future frames). **Crate maturity**: transport assessment consistent with
§5.2; the aws-lc-rs platform-table detail (emscripten tested, browser wasm
not) sits inside the existing hedge.

## The over-strictness stress family (author-directed, 2026-09-02)

The author: accumulate the cases where the review-implementer adopted
over-strict logic and add tests specifically stressing those areas — where
the spec cannot prevent a compatibility-breaking misunderstanding, the
integration test must catch it. Inventory across the four 0.6 rounds
produced ten entries (README's new table). Six needed new fixtures:
- TR11 (dial an unkeyed referral hop, disclose-nothing) — resolution U4's
  never-dial reading would strand resolutions;
- TR12 (a gapped heartbeat resets liveness) — the exact-expected-counter
  sketch fails over against a live server;
- TR13 (the server's AttachAck mode is authoritative) — client inference
  reports wrong state on stale topology;
- TR14 (early ServingInfra completes resolution) — the arrival-equation trap;
- N-responses-empty-array + T30 (the empty-array spelling rejects — the
  complement of P-fin-absent, so both directions of the field-5 reading are
  pinned);
- P-channel-retry + D19 (a repeated channel kind, failed-then-passed, with a
  fully revealed presentation whose root and strongest-rule the harness
  recomputes).
Four were already stressed (D6, TR1, the open-registry D-rows, V7's
finalization fixtures) and are now listed under the family so the register is
one place. Traces stand at fourteen; harness at 67 checks, all passing.

## Cycle 2, pass 0.6 — attach target, phase 2 (2026-09-02)

Run on the pre-stress-family vector set (the pin gate flagged the staleness to
the reviewer exactly as designed — they noted the wire hash mismatch and
classified against the current spec). Tally: **one implementation bug, one
vector defect, zero spec ambiguities.**

- **Implementation bug — TR7's heartbeat gap**: their exact-expected-counter
  guard permanently poisons the sequence after one lost beat and fails over
  against a live server — precisely the over-strictness the trace was built
  for, caught before any interop. (The prose anchor and TR12's concrete
  sequence landed the same day, independently.)
- **Vector defect — TR2's machine encoding, fixed**: the spec permits
  reject-or-defer for an Attach in 0-RTT; messages.md's table said so, but
  corpus.json required only defer_until_handshake — a conforming rejecting
  server would fail it. TR2 now requires never_process_as_early_data with a
  one_of carrying both branches, and the trace class documents the one_of
  convention. Harness checks the disjunction. Swept the other traces: TR2 was
  the only alternative-permitting rule.
- **TR9/TR10 epistemics**: the reviewer correctly notes vectors do not
  retroactively disambiguate prose — and against their snapshot the prose was
  indeed silent. The wire §9 scope rules landed in the same commit as the
  traces; under the current spec the sentences exist. Timeline noted.

Everything else agreed at decision level or fell honestly outside the
target's implemented boundary. Harness: 68 checks, all passing.

## Cycle 2, pass 0.6 — capture + verifier query (phase 1, 2026-09-02)

The deepest round: 18 UNSPECIFIED items, one design/wire contradiction, one
real carriage gap. Dispositions:

**Design-precedence contradiction, fixed**: wire §5.5 classed an absent
capture key as `inconclusive`; design §7.5.2 makes withholding deliberately
look like unavailability. Wire now: absent key is `unavailable`; a capture in
hand that cannot be read is `inconclusive` — carrying **basis 0 and the
query's template version** (U14's rule: the attempted mechanism, never an
assertion comparison ran).

**The carriage gap (U4), fixed with the cycle's one field addition**:
response field 10 is the selector's claim under the verifier's signature, and
no specified request element carried it — the specified response was
unconstructible from the specified bytes. Request type 4's body is now
`[VerificationQuery, COSE_Sign1, uint]`: the claim rides third, outside every
signature, because the transport authenticates the requester who is the
selector; the verifier echoes it into field 10 and signs the echo. Recovery
untouched (its verifier self-queries; no type-4 request travels). **Flagged
for the author as the round's substantive protocol change.**

**Ruled and applied**: U1 — HKDF-SHA-256 stated to the byte (salt empty, IKM
seed, info tag‖subject‖holder‖ceremony, L=32; divergence is catastrophic) with
a known-answer vector the harness recomputes and the KeyGrant fixture now
carrying the derived key; U2/U3 — the capture-key handover and the
selected-verifier identity ride the ceremony's direct channel, carried by no
wire object (§8.1.2's rule), now stated in §7.5.2; U8 — query field 2 MUST
name the authenticated requester; U9 — the reply body is a single
VerifierResponse; U10 — a malformed query closes the stream, no signed
fabrication; U12 — a KeyGrant's authenticated sender MUST be the subject;
U13 — duplicates and differing second grants are ignored, the first stands;
query field 5 bounded 0..=65535 (the reviewer's u64/u16 asymmetry).

**Already open or local, no change**: U5 (canonical biometric profile,
§22.2), U6 (AEAD/framing, §22.2), U7 (anti-oracle values, §21.1-class local),
U11 (payload demultiplexing, §14.2.4's standing fifth decision), U15
(querier patience, §21.1), U16 (restore re-release, the §7.5.2 Open bullet),
U17/U18 (local). The segment-key concept confirmed dead; review-plan target
5's stale wording fixed. Type decisions sound throughout, including the
affirmative no-segment-types and no-wire-demux-enum decisions.

## Cycle 2, pass 0.6 — capture/query re-run against the corrected spec (2026-09-02)

The re-run validates every ruling from the first run — the request-4 triple,
the unavailable/inconclusive precedence with the basis rule, the exact HKDF
(their code implements it byte-identically), the KeyGrant sender and
first-stands rules (their GrantInbox is a direct implementation), and the
ceremony-channel carriage. Three findings:

1. **A custody sweep failure of ours, fixed**: the §7.1 ceremony summary still
   said "each party also gives the other a 32-byte seed" — the phrasing the
   §7.5.2 custody fix replaced at its own site but not here. The summary now
   hands the derived per-ceremony key, seed retained by the subject. One
   phrasing is not a sweep, once again.
2. **The pre-commitment construction, ruled (new mechanism — flagged)**: the
   32 bytes were specified as fixed-before-capture, unique, countersigned —
   and constructed by nobody. Now contributory: SHA-256 of `rhtn/1:ceremony`
   followed by each participant's 16 random bytes in ascending
   participant-keyhash order. Either party's honest randomness forces
   uniqueness, and a forced repeat is the one thing worth forcing, since
   consents and capture keys bind to the value. Known-answer vector added and
   harness-recomputed; the presence fixtures' arbitrary pre-commitments remain
   valid inputs (construction is unobservable from a record) and are noted as
   predating the rule.
3. **Unknown-txid grants yield `unavailable`** (their U12): nothing to
   compare, nothing evidenced — the absence posture, now stated in wire §7.3.

The remaining unspecified items (U4–U11, U13, U14) are the standing §22.2 /
§21.1 / Open-bullet set, correctly mapped. Type decisions sound throughout —
the Zeroizing seed/key ownership, the no-Unknown-variant enums, the raw-slice
extension preservation, and the affirmative refusal to give ReleasePolicy a
transmitted reason code (withholding must stay indistinguishable). Crate
assessment matches §5.2. Harness: 72 checks, all passing.

## Cycle 2, pass 0.6 — capture/query phase 2 (2026-09-02)

**Two real vector defects, both ours, both the same root error, both fixed**:
the KeyGrant chain treated the record under assembly as the sealing context.
The fixture's field 1 named the current alice–bob record's txid — circular,
since that txid embeds the very responses the grant unlocks — and the
derivation used the current ceremony's pre-commitment for a capture c1 could
only hold from a meeting c1 participated in. The fixture universe did not even
contain such a meeting. Built now: **the prior alice–c1 record**
(`P-alice-c1-record`), with its own contributory pre-commitment; the
derivation and the KeyGrant re-derive from it; the harness pins both the
prior-record binding and the current-query binding, so the circularity class
cannot recur silently. Wire §7.3's field-1 comment already said "ordinarily
the latest finalized eligible meeting" — the generator simply violated it, and
the reviewer's hand-trace of their own release path produced the correct shape
the fixture now has.

**One caption defect fixed with structure**: the replies-group caption claimed
the u32-be prefix applies on the wire — true for stream replies, an
overreach for the two end-to-end payloads, whose framing is §14.2.4's open
demultiplexing decision. messages.md now has a separate **End-to-end
payloads** section claiming no framing; KeyGrant and LateResponse re-register
as `P-e2e-01/02` (their U15 refinement of U10, honestly recorded as open).

**The pre-commitment epistemics**: same timeline note as prior rounds — their
snapshot's spec lacked the contributory construction; it landed with the
vector in c368542. Under the current spec the construction is normative.

NOT-COVERED classifications (personal-knowledge evaluation, recovery
response, LateResponse assembly, COSE-shape internals, selection arithmetic)
all honest boundary reports. No covered-path implementation bug. Harness: 74
checks, all passing.

## Cycle 2, pass 0.6 — catalog registration and query (phase 1, 2026-09-02)

A clean round: no fields added, the registration/entry/roles/UI split exactly
right, the reply construction matching the specified
resource-then-owner-keyhash ordering and continuation rule verbatim, and the
raw-splice reply encoder honouring the serve-unchanged rule. Seven questions:

**Ruled and applied:**
- U2 — a FIRST registration with field 2 absent defaults to `self`, the
  least-disclosing scope (the withhold-by-default posture); broader
  visibility is the owner's to request.
- U4 — "ascending keyhash" is stated once, at the keyhash definition:
  lexicographic over the raw 32 bytes, identical to big-endian numeric, so no
  little-endian reading survives (their truncation-page divergence case).
- U6 — failure signalling split: malformed framing resets the stream;
  well-framed registrations failing the owner/signature/conflict checks get
  `refused`, the reply existing to carry exactly that answer.
- U3 — the reviewer's local-binding requirement was OVER-STRICT for brokered
  resources: a registration is complete in itself, nothing structural
  requires a local backend, and the stress family gains the row (existing
  fixtures P-frame-16/P-catalog already exercise a brokered endpoint).

**Already local (U1, U5, U7)**: scope-choice algorithm, admission policy, and
the 0-RTT reject-or-defer disjunction (TR2's one_of). Type decisions all
sound — notably the OPEN DataPractice newtype where a closed enum would
violate the extension rule, and the Arc-raw-plus-view entry representation.
Crate assessment adds cbor2's RawValue as a fitting strict-codec substrate;
consistent with §5.2 otherwise.

## Cycle 2, pass 0.6 — catalog target, phase 2 (2026-09-02)

Ten of eleven target vectors hand-traced to agreement; zero implementation
bugs; zero spec ambiguities (the reviewer correctly noted that P-frame-16's
absent field 2 does not settle U2 — the vector shows the encoding, not the
host's installed rule, and U2's `self` default was ruled in phase 1's
ingestion regardless).

**One vector defect, real and fixed**: P-catalog-reply-truncated carried one
entry plus a continuation — unproducible by a conforming host, whose
continuation-present state entails MORE than 111 qualifying entries and
exactly the first 111 returned. Rebuilt conformingly: 111 owner-signed
entries sorted by resource keyhash, continuation naming the withheld 112th's
type (its all-high resource id provably sorts last). The harness pins the
111-count, the sort, and the continuation's coexistence. The reviewer's
boundary note stands: receiver-side tolerance of a NONCONFORMING host's
short-plus-continuation reply is untested and unspecified — a client cannot
verify the producer's qualifying count, so the object is receiver-valid;
nothing added, recorded as observed.

## The Rust conformance runner (2026-09-02, author-directed high-effort window)

Built under the expiring-credit directive: the highest-value artifact
available without an external party is an EXECUTABLE second validation of the
suite. `test-vectors/runner-rs` is a self-contained crate — its own
byte-level deterministic-CBOR parser, all 25 identities re-derived from the
stated seed recipe (RustCrypto ml-dsa `SigningKey::from_seed` = FIPS 204
KeyGen_internal), and full signature verification through the Rust ecosystem:
every envelope (signer sets derived per type, both algorithms, the normal
record's 36 entries), the embedded evidence layer (consents over raw
query-ids, classical field-9s over map-minus-9, the recovery block's HYBRID
responses and successor proof), all presentations' roots, and the standalone
records with wrong-signer analogues failing as required.

**Result: 143 byte-entries pass, 0 fail** (39 skipped: traces/contexts/units
are structured, plus rejects of kinds the runner does not yet validate).
**The post-quantum half is now cross-implementation**: signatures produced by
dilithium-py verify under RustCrypto's ml-dsa.

**One real finding on the first full run**: B-ext-value-1024/1025 measured
payload bytes where §1's ceiling bounds the ENCODED slice ("1024 bytes of
encoded CBOR — the complete encoded slice for the value"). Both fixtures were
over the ceiling as built; corrected to encoded-slice targets, and the bound
taught to the Python harness, which had never checked it. The runner also
found four of its own bugs en route (nested-offset parsing, a mis-firing
disavowal classifier, missing shape rules) — each fixed against the spec, not
the fixtures. Honesty note recorded in its README: same author, so this is
cross-language/cross-crypto validation, not the independent-party
reproduction promotion ultimately wants — but it is the artifact such a
party starts from.

**Runner deepened (same session)**: query_id recomputation for the standalone
query and the request-4 frame (with its consent verified under alice and the
selection_basis triple checked), the TopologyPush inner envelope fully
verified as an envelope, and cross-entry bindings — push payload ==
adoption bytes; KeyGrant == (recomputed prior-record txid, recomputed current
query_id). **145 pass, 0 fail.**

## Cycle 2, pass 0.6 — resource authorization (phase 1, 2026-09-02)

Target 7: authorize a user to a hosted resource, incoming request through to
the framing handed to the resource. The reviewer added no wire fields (all
authorization inputs are local snapshot state; only the four `rhtn-*` headers
cross), ordered the checks exists→member→ack→connect→HTTP-validity→availability
with availability last per the operational-privacy rule, and computed the
pairwise principal with the exact specified formula — verified against
`resource-requirements.md` §2 and design §11.0.2, byte-identical construction.
Zero implementation bugs claimed and none found. Eleven UNSPECIFIED items;
the reviewer's own priority order was U9, U8, then U6.

| # | Item | Disposition |
|---|---|---|
| U1 | Session-id byte width/generation | LOCAL BY DESIGN — rr §3 already forbids resources assuming a width; no change |
| U2 | Local-socket ABI (fd passing, socket kind) | LOCAL — packaging, below the protocol; no change |
| U3 | `Host` authority for a local-socket backend | APPLIED — wire §11: a backend with no network authority gets whatever authority the installation recorded; binding state, not protocol |
| U4 | What "availability" measures | LOCAL — deliberately operator-defined; no change |
| U5 | Canonical HTTP reserialization bytes | LOCAL BY DESIGN — wire §11 already says "from your parse, deterministically", explicitly not byte-interoperable; no change |
| U6 | 0-RTT for opaque application requests | APPLIED — wire §11: every type-6 request is in §9.2's forbidden class, since the gateway deliberately does not interpret application semantics and so cannot certify any method effect-free; §9.2 gets the mirror sentence (a resource request is never read-only, whatever the method). The reviewer's conservative reading is now the specified one |
| U7 | Duplicate role names in the header | APPLIED — rr §3: role names form a set; duplicates carry no meaning and a conforming node does not emit them |
| U8 | No bound on role count / header size | APPLIED — rr §3: at most 64 application roles per principal per resource (header stays near 2 KB, inside common HTTP stack limits). Enforced where visible: a node refuses to MATERIALISE an over-wide row (infra §10.2 carries the actor side), so the failure is configuration-time and no request-time failure code exists because none can occur. Component bound, so it lives in the component docs per §21.1's placement rule |
| U9 | Hosted-session termination granularity | APPLIED (derived, flag for author sign-off) — infra §10.5: the session dropped is the RESOURCE-FACING one: the node-held identifier under which the resource sees the principal's requests (rr §2), not the caller's rhtn/1 transport session. Retire the identifier, reset that pair's in-flight requests; the next request is evaluated afresh and arrives under a new identifier, which is how the resource observes the change — rr §3 already states there is no teardown message and sessions are observed ending by requests ceasing. The transport session is untouched: an authorisation change at one resource is not a connectivity event. Derivation, not invention: rr's no-teardown observation model and design §11.4's end-not-mutate rule jointly determine it; the whole-transport reading would end sessions with unaffected resources and the control plane |
| U10 | Backend dies mid-handoff | APPLIED — wire §11: answered `resource unavailable` (status 2) and the gateway NEVER retries on its own; it cannot know whether the application committed an effect, so retry is the requester's decision |
| U11 | Timing equalization for refusals | ALREADY OPEN — acknowledged in the spec as such; no change |

**Interop surface the round exposed**: the pairwise principal is computed by
whichever node currently hosts the resource, so two node implementations must
agree byte-for-byte or a provider migration renames every user a resource
knows. Known-answer added: `records.md` gets a pairwise-principal KAT
(resource c1, user alice), `verify.py` recomputes it (79 checks now, was 77).
The corpus schema has no derivation class; prose KATs recomputed by the
harness are the standing convention (HKDF, pre-commitment), and this follows
it.

**Reference sweep after the edits** found two pre-existing bare cross-doc
refs and fixed both: wire §5.5's "§16.1" and light §2's "§6.4" each meant the
design and now say so (wire has no §16; light has its own §6, so that one was
locally ambiguous). Eight other flags were checker false-negatives (RFC
sections, capitalized "Design §", list-continuation refs) — each inspected,
each resolves.

**Both harnesses green after regeneration**: Python 79/79, Rust runner
145 pass 0 fail.

## Cycle 2, pass 0.6 — resource-authorization phase 2 (2026-09-02)

**Zero divergences in all three classes**, and the reviewer's six-vector hand
trace verified accurate on every checkable claim: P-frame-15 is 163 bytes
carrying c5's resource keyhash and the spoofed `rhtn-principal` inside the
HTTP bytes; P-reply-08/09 are 46 and 3 bytes as read; N-enum-resource-status
is `a10109`; N-shape-frame-arity is `[3]`; TR3 is the type-99 trace. The
side observation (c5 serves as both catalog resource and this fixture's
resource) is correct and intended — identities are fixtures, not roles.

**The payload is the coverage report**: the vectors reach the wire shapes and
shared framing but none of the authorization mechanism. Disposition of the
eighteen uncovered behaviours:

- **Vectored now (stress directive, author 2026-09-02)** — three traces:
  TR15 (type-6 in 0-RTT early data: defer-or-reject, never process — pins
  U6's ruling; the phase-1 reviewer got this right but the spec should not
  require rediscovery), TR16 (role-row change retires the resource-facing
  identifier; transport and other resources' sessions survive, in-flight
  completes under its starting snapshot — pins U9 against the whole-transport
  reading the phase-1 attempt itself adopted; README stress row added),
  TR17 (a non-member with a stale role row in the snapshot gets status 1,
  never 4 or 5 — the evaluation order is what keeps member-specific statuses
  inside the membership).
- **Already closed between their phases**: the pairwise-principal KAT landed
  in the phase-1 ingest; their vector set predated it.
- **Deliberately out of remit, no change**: exact node→resource
  reserialization bytes (explicitly non-interoperable), local-socket
  authority, availability meaning, session-id representation, audience/
  header emission and stripping conduct, backend-death races, timing
  equalization — behavioural client/operator commitments the README already
  says this suite does not certify. Recorded, not vectored.

Trace count 14 → 17; the two harness checks pinning fourteen updated
(the counts-drift rule, again). Python harness 79/79; Rust runner 145 pass,
0 fail, the new traces in its structured-skip set. Corpus at 185 entries.

## Cycle 2, pass 0.6 — refused resource request (phase 1, 2026-09-02)

Target 8. **The cleanest round of the family: every claim verified against
the text, zero spec changes required.** The reviewer's implementation
reproduced wire §11's normative evaluation order exactly, split malformed
input at the step-0 boundary the way §9.2 states it ("once the type is known,
a defect in the body is that type's business"), kept status 1 opaque to
strangers, contacted no resource on any refusal, and correctly named the two
deliberately absent types (no requester field in the request, no reason
payload on a refusal). The 0.6.7 rulings returned as quoted specification
this round — the type-6 0-RTT class and the never-retry rule were cited, not
rediscovered.

Seven unspecified items, all verified as local or specified-open:

| # | Item | Disposition |
|---|---|---|
| U1 | Snapshot atomicity mechanism | LOCAL — infra §10.1 mandates the observable property (one coherent read per request), not the storage mechanism |
| U2 | QUIC error code for the stream-failure reset | SPECIFIED AS UNASSIGNED — §9.2: "No application error code is assigned for the reset, and none is needed: the reset is the whole message." The reviewer's disposition (local code, requester attaches no meaning) is the specified reading |
| U3 | Timing equalisation across code-1 paths | SPECIFIED OPEN — §11's closing paragraph says the opacity is in what is said, not how long it takes; equalising is optional hardening |
| U4 | Which parser performs the byte-level checks | LOCAL — the accepted byte language is normative (wire §1), the library is not |
| U5 | Machine width for type/status | LOCAL — CBOR uint, shortest-form; any width encodes identically |
| U6 | Owned vs borrowed byte strings | LOCAL — nothing on this path is signed, so no preimage-lifetime question exists |
| U7 | Requester-side API shape | LOCAL — light §8's duty (show roles so a user distinguishes policy denial from breakage) verified as UI conduct, not a wire field |

Crate-maturity check: design §5.2 verified still accurate — it makes no
coset claim (so coset 0.4.2's new ML-DSA identifiers stale nothing), and the
January 2026 ml-dsa timing advisory predates the 0.1.1 release the Rust
runner pins (verification-only use regardless); the advisory reinforces
§5.2's unaudited caveat rather than contradicting it.

**One trace added under the stress directive — TR18**: a frame parsing as
`[6, body]` whose body is not a well-formed ResourceRequest is ANSWERED with
status 3, never reset — and the boundary does not generalise across types,
since a malformed type-4 body closes the stream (§5.6: "no error schema
exists"). Each type answers in its own terms; an implementer generalising
either rule to the other type diverges visibly. Trace count 17 → 18, both
count checks updated. Python 79/79; Rust 145/0.

## Cycle 2, pass 0.6 — refused-request phase 2 (2026-09-02)

**Zero divergences, zero changes.** Five fixtures hand-traced to agreement
(P-frame-15, P-reply-09, N-enum-resource-status, TR17, TR18) — TR17 and
TR18, written one round earlier from this target's own rulings, were
satisfied on first contact. The apparent prefix discrepancy the reviewer
raised and resolved themselves is the documented convention: every reply
fixture in messages.md states "replies carry no type tag and no length
prefix here; on the wire the same u32-be prefix applies" — their reading
verified verbatim. The TR9/TR10 distinction (attach-time `refused` close
code versus ResourceResponse status 1) was correctly kept apart — the
role-not-identifier discipline holding up in a reader.

Scope reports, accurate and requiring nothing: TR15 not covered (phase 1
has no transport-state input to test early-data handling against) and
TR16 partially covered (the snapshot half — in-flight completes under its
starting state — is implemented and matches; the session-manager half is
outside a refusal-path implementation). Both are honest phase-boundary
reports, not gaps in the vectors or the spec.

Target 8 closes with both phases at zero divergences in all three classes.

## Cycle 2, pass 0.6 — topology flood + endpoint record (phase 1, 2026-09-02)

Target 9. The reviewer's two structural decisions both verified as specified:
the store IS the seen-set (§10.1: "no dedicated suppression cache exists, and
none should be added") and no wrapper field beyond the body-kind tag ("that is
the only wrapper field permitted"). Their field classification added nothing
to the wire. Twelve unspecified items — the most substantive haul since the
family started, and the reviewer's own priority (#6) is the one that mattered.

| # | Item | Disposition |
|---|---|---|
| 1 | Task prompt cites wire §5.6 for endpoint records | STALE PROMPT REFERENCE — review-plan.md target 9 corrected to §7.6; the documents were never wrong |
| 2 | Session consequence of a malformed known control frame | APPLIED — §8.0: discarded whole, session survives, generalising the two stated instances (heartbeat counts as absence, SiblingUpdate ignored whole); a corrupted frame is indistinguishable from loss and there is no reply channel to answer on |
| 3 | Pending-verification quarantine bounds | LOCAL — deliberately unset, same posture as every other resource ceiling; reconciliation repairs an eviction |
| 4 | Which sender is "the arrival edge" for a multi-supplied pending object | LOCAL — any supplier already holds the object and suppresses the echo by txid; traffic pattern only |
| 5 | Subject of a type-7 SeriesReissue | APPLIED — §10.1's enumeration now names it: field 1, the node whose line changes, from the section's own general principle ("the node whose position the transaction changes"); the patron countersigns but it is not the patron's neighbourhood that changed |
| 6 | EndpointRecord has one seqno; series are per patron relationship | APPLIED (derived, FLAG FOR AUTHOR — the reviewer's "most consequential") — §7.6: one record per relationship line, each carrying that line's own seqno; infra §4.4 echoes it. Three independent forcings: a single record's series is unprovable in the other subnet so it never enters storage there; a shared counter across subnets discloses exactly what P36 (design §19.4) conceals; and the patron in each subnet must be able to refer. Contents usually agree; nothing requires it. No patron field added — the reviewer was right not to invent one |
| 7 | First-publication counter; unchanged list after a position change | LOCAL — the record carries the current seqno; the counter advanced when the position or the list changed, so publication itself never spends. Receivers do not require contiguity, so the alternative is not even observable |
| 8 | Is reorder-only a change? | APPLIED — §7.6 and infra §4.4 now say "list", with reordering explicitly a change: field 2 is preference-ordered, and any field-2 difference at equal seqno is the collision the advance exists to prevent. Forced by the malformed-on-equal rule, as the reviewer argued |
| 9 | Unproved-series EndpointRecord: store/flood? | APPLIED (derived) — §10.1: neither stored nor forwarded until the series proves current, the same posture as the transaction with a missing signer key; hold-versus-drop meanwhile is local, and the wire-visible rule is only that an unproved series never floods onward |
| 10 | Equal seqno, equal endpoints, different extensions | APPLIED — §10.1 aligned to §2.3's own wording: equal seqno with different SIGNED CONTENTS is malformed, endpoints and extensions alike; §2.3's rationale (a subject advances for any new signed content) already decided it, and three of four statements already said "contents" |
| 11 | State repair after retroactive verification fails | LOCAL — remove and rebuild; no negative record exists on the wire and absence is the encoding. Their no-synthesis instinct is the design's |
| 12 | Receiver-verifiable infra status for the child table | LOCAL, CONFIRMED CORRECT — possession of an EndpointRecord is the discriminator (only infra nodes publish); a false one costs a failed dial (infra §4.4's refer-from-gossip requirement). No is_infra bit exists and none may be added |

Their NetworkPoint claims all verified (4-byte IPv4, optional u32 ASN, port
u16 with zero malformed and 7431 never written). One latent trap noticed in
their type sketch: `Option<NonZeroU16>` can represent an explicit 7431, which
is malformed to WRITE (§1's default-omission) — the corpus had fixtures for
ports 0 and 65536 but not this one. **B-port-7431-explicit added** (the Rust
runner's schema check already rejected it — the fixture now proves that).
**TR19 added**: the peering-cycle duplicate dies against the store, no
forward, session survives.

Crate table consistent with design §5.2 again; their "authoritative thing
signed is a byte-preserving representation, not the serde struct" closing is
§1's rule restated — the profile's design intent surviving a clean-room read.

Nineteen traces, 187 entries; Python 79/79, Rust 146/0. Reference check
clean at 1851 references (six known false-negatives).

## Author ruling on 0.6.9 #6 (2026-09-02)

Confirmed: one EndpointRecord per relationship line, each with that line's
own seqno. Author rationale, now in §7.6: **record and locator partitioning
between different subtrees is the anticipated use** — the lists differing
per line is the point, not tolerated slack. Flag closed.

## Cycle 2, pass 0.6 — topology-flood phase 2 (2026-09-02)

One implementation bug, two spec ambiguities claimed; verified as one bug,
one already-ruled item, one genuinely new gap now ruled.

**The bug is the stress mechanism working end-to-end.** B-port-7431-explicit
was added during the phase-1 ingest, from noticing that the reviewer's
`Option<NonZeroU16>` could represent an explicit default port; phase 2 hand-
traced exactly that miss. The fixture existed for one round before catching
the divergence it was built for — the author's compatibility-breaking-
misunderstandings directive doing precisely its job.

**V11 (#9): already ruled between their snapshots.** Their phase 2 correctly
reports the pre-ruling text as silent on storage posture; §10.1 has since
said an unproved-series record is neither stored nor forwarded. Their
quarantine implementation conforms (hold-versus-drop is local). The V11 row
now cites the ruling and adds the legitimate other half: with chains proving
BOTH series — two patron relationships, one record per line — holding both
is the correct end state, not a conflict.

**V6 (#13): new gap, ruled (FLAG FOR AUTHOR).** The spec said equal seqno
with different contents is malformed but never said what happens to the
record installed FIRST. Their implementation chose first-wins; the vector
suite (ninth review) had already asserted "a reader MUST NOT prefer either."
Ruled at wire §10.1, echoed at §7.7.3 for the locator route: **malformed
names the pair, not the later arrival** — which arrived first is an accident
of the path, and first-wins lets arrival order split the network's view
(§10.1's own repetition-rule refusal, and §2.3's pinning attack: a thief
whose record lands first would pin every reader it reached). On discovery
the holder retains neither as current, forwards nothing further for that
(subject, seqno), and repairs by re-resolution, which descends the
authenticated path; the line is repaired only by its subject (greater
counter, or §4.6 reissue). Keeping the pair as evidence is local. V6/V12
rows updated; records.md conflict-pair prose extended.

Also verified: their P-reply-02 non-divergence claim is right (advances=1
floor conforms; multi-index referrals are optional depth), TR19 and D12
matched exactly, and the frame-level not-covered notes are honest scope.

Python 79/79, Rust 146/0.

## Author ruling on 0.6.9 phase-2 #13 (2026-09-02)

Confirmed: malformed names the pair, not the later arrival. The §10.1/§7.7.3
text stands as written. Flag closed.

## Cycle 2, pass 0.6 — rootward memo + cycle detection (phase 1, 2026-09-02)

Target 10, the last of the plan. The reviewer implemented the CURRENT text
against a task prompt that had gone stale, and did exactly the right thing:
recorded the staleness, followed the documents. Verified: the prompt's
"keyed on the subject's own seqno" predates the memo privacy correction
(C19: the memo no longer carries the subject's counter; the table is
(patron, slot) ordered by field-4 timestamp), its "naming your own position"
predates the identity-comparison rule (§10.2 states position equality would
work and is deliberately not the test), and — a third staleness they acted
on correctly without flagging — its "underlying transaction fetched before
acting" contradicts §10.2's "Not a fetch" paragraph. The plan's target-10
wording is rewritten with a note crediting the round.

Every implementation claim verified against the text: schema fields 1-5
with absent-occupant-means-empty, (patron, slot) keying, within-one-slot
timestamp ordering, equal-timestamps-by-arrival-order, at-or-after
forwarding suppression, empty-slot-is-a-row, wrong-subnet memos dropped
unapplied, serving-node checks for attached clients with the hit handed
over at contact, hint-never-evidence with the two-question confirmation,
reason 5 without prejudice, the banded reason exception to the closed-enum
rule, body key 0 back-pointers, and the full path validation (24 nibbles,
0-9, exact packed length, zero-filled odd nibble — all four rules already
in the text; their validate() matches).

Seven unspecified items, all verified local; #2 is answered by the text:

| # | Item | Disposition |
|---|---|---|
| 1 | Table write on a cycle-hit arrival | LOCAL — the table is optional in its entirety, so any mutation discipline is local by construction; the detector's own slot state is authoritative regardless |
| 2 | Confirmation predicate depth for field 2 | ANSWERED BY TEXT — §10.2 states the two questions (did I make that change; is my row still what the memo asserts) and the cycle paragraph already notes identity comparison is unaffected by "a counter that moved for an endpoint change"; field 2 is context, not part of the predicate. Their no-byte-equality choice is the specified reading |
| 3 | Ingress-branch API under collapsed hops | LOCAL — the wire adds nothing; how the session layer surfaces the logical branch is host architecture |
| 4 | Table durability across restart | LOCAL — a RIB, optional, rebuilt by traffic; no tier is load-bearing |
| 5 | Machine test for "both parties present" | LOCAL/UI — no both_present field, correctly |
| 6 | Clock source for the auto-disavowal timestamp | LOCAL — §4.3 has no notice period and the timestamp is the signing act's; any local clock with the stated semantics conforms |
| 7 | Pending-client-cycle queue representation | LOCAL — no new message exists; delivery-at-contact is the specified behaviour |

**One gap found by reading their decode path, ruled**: §8.0 stated the
65,536 bound but not its consequence, and their code discarded an oversized
frame with the session surviving — while TR6 has always expected
protocol_error. Ruled at §8.0: a declared length above the bound ends the
session; it sits BELOW the malformed-frame rule, not under it — a malformed
body inside a bounded frame is cheap to discard, but an out-of-contract
length would have the receiver stream an attacker-declared volume through
the very ceiling that bounds its buffer. Their handling is now a
specified-answer divergence for phase 2 to catch.

**Two traces added**: TR20 (unconfirmed cycle hint → reject, no disavowal,
nothing severed — the false-positive rule §6.2.5 ranks worst) and TR21
(ordinary memo whose field-2 path has the receiver as prefix → forward, no
cycle; the containment trap §10.2 documents joins the README over-strictness
family). Twenty-one traces.

Crate assessment: their ciborium-is-not-the-validator conclusion is wire
§1's rule rediscovered clean-room; design §5.2 gains one sentence —
aws-lc-rs now exposes ML-KEM and ML-DSA as a production-oriented native
route with no browser-wasm target, sharpening the stated split. Their
libsignal PQXDH note is ecosystem context; the design cites PQXDH as
protocol, claims no crate path, nothing stale.

Python 79/79; Rust 146/0, 46 structured skips.

## Cycle 2, pass 0.6 — rootward-memo phase 2 (2026-09-02); the family closes

Zero divergences on the five direct memo vectors (P-frame-07/08,
N-enum-memo-slot, TR20, TR21) — TR20 and TR21, written from this target's
own phase-1 findings, satisfied on first contact, TR21's containment trap
included. The reviewer's P-frame-07→08 sequence reading (later timestamp
writes the empty ROW) is the specified semantics, correctly composed
unprompted.

**They self-caught the TR6 divergence phase 1 planted.** Their snapshot
predates the §8.0 ruling, yet they classified their own oversize handling
as an implementation bug with exactly the ruling's layering argument: at
length-rejection time no frame type is readable, so the known-frame discard
rule cannot apply. The pre-ruling text plus TR6 sufficed — the ruling and
the clean-room read agree independently, which is the best evidence the
ruling carved at a real joint.

**Coverage response**: the confirmed-cycle complement of TR20 was the
named gap — spec-determined and untested. TR22 added (records confirm, no
live disambiguation → disavow the ingress subordinate, reason 5, without
prejudice, memo terminates) and TR23 with it (wrong-subnet memo → drop, no
table write, no forward — the privacy boundary §10.2 says holds only if
every receiver enforces it). The table temporal behaviours (at-or-after
suppression, equal-timestamp arrival order) are deliberately NOT traced:
the table is optional, a no-table node forwards everything conformingly,
so no universal expectation exists to pin — the optionality is the reason,
recorded here so the gap is not re-flagged as an oversight.

**The 0.6 family closes**: ten targets, twenty rounds. Divergence trend
across the family: spec ambiguities zero for the last six targets;
implementation bugs caught twice by fixtures planted one round earlier
(B-port-7431-explicit, TR6 via the §8.0 ruling); the stress family grew
from nothing to thirteen README rows and TR11-TR23. Twenty-three traces,
188 corpus entries; Python 79/79, Rust 146/0.

## Author ruling on 0.6.10 §8.0 (2026-09-02)

Confirmed: a declared stream-0 length above 65,536 is a protocol error that
ends the session — below the malformed-frame discard rule, not under it.
The §8.0 text stands as written. Flag closed; no open work remains ahead of
pass 0.7.

## Cycle 2, pass 0.7 — LINDDUN privacy analysis (2026-09-02)

The register survived an external LINDDUN audit. The reviewer's own closing:
the design "does not generally make the classic mistake of arguing that each
artifact is safe in isolation. Its own C-register already catches most of
the dangerous joins." Thirty-nine data flows and twenty-nine stored
artifacts audited; every ACCEPTED marking matched §19.7; every severity
they assigned to an existing finding matched the register's own (P12
Critical-until-built, P5/C9 Critical-on-compromise, P2/C2, P19/C4, P3/C10,
P20/C7, P4/C5, P23, C6 all High). Their non-elevation of resolution nonce
reuse (an amplifier of P26, not a new capability) matches the earlier
disposition. Their warning that process-and-discard must never be described
as protection from a malicious operator is the register's own residual
language.

Four proposed additions, all verified as genuinely unregistered, all
applied:

| New | Content | Verification |
|---|---|---|
| C20 (Medium) | Pairwise fanout deliveries + patron queue metadata → latent group membership and cadence | §14.3 analyses fanout only from the endpoint ("recipients cannot reply to the group"); no text prices the patron seeing the burst. Both ingredients priced (§14.3, P4); the join was not. Registered with "no traffic-shaping mitigation is specified, and none is promised" stated plainly |
| C21 (Low–Medium) | Capability set + attachment continuity → implementation/device fingerprint at the serving node | §8.1.1's greasing rationale is anti-ossification and its own text confirms a greased parameter hides nothing about the real set; P32 prices retaining the history, not what the history identifies |
| C22 (Low–Medium) | Materialised role table + catalog store at one host → operator-readable person/role/service matrix | Aggregation-accessibility, exactly as the reviewer framed it: no new disclosure crosses any interface, the cost of extraction drops. C7's yield pre-joined at one party |
| P38 (Low–Medium) | A filtered catalog query reveals the asker's service-class interest to the answering node | P26's shape at the catalog; unregistered. The reference client's sweep-and-cache (light §8) already blunts it — registered with that as the standing mitigation and targeted queries as the residual |

Register extents now P38 / C22; numbering append-only as required. The
reviewer's category-level conclusions (Linking dominant; Non-repudiation a
product, not an accident; the data plane's deniability separation "a strong
design choice") align with the design's stated posture and need no text.
Their four NC-category items are all already registered (immutable
evidence/erasure = accepted cost 9 with the compliance-posture note;
biometric custody = P13/P29; unenforceable declarations = P20/P24 and
§1.1; multi-device = P33 undetermined). Reference check clean at 1,860.

## Cycle 2, pass 0.9.1 — adversarial: the patron (2026-09-02/03)

Three non-restating findings under the patron adversary; zero novel, three
verified, all ruled by the author and applied. Fourteen candidate lines
correctly self-discarded as restatements.

**1. Exit laundering (REASONING → accepted at true size, §18.5 extended).**
Verified: §18.5's limiter is "an observer holding both objects" verbatim,
and wire §10.1 makes the serving node a light client's sole flood ingress
with no ack, no retry, receiver-side-only gap detection. Two corrections
found during verification, one to the assistant's own analysis: both
objects name the same subject and therefore flood the same ball — "the old
subnet's propagated view" (reviewer) and "floods in the new subnet"
(assistant) were both wrong scope; transactions are in-horizon, only the
memo travels rootward. Author's ruling reduced the finding: the network
builds no cross-tree reputation except the adoption history scan, which
reads the subject's own archive — carrying the departure and never the
patron's single-signer disavowal. Suppression therefore buys a falsified
exit story told only to the neighbourhood the victim left; every evaluator
who matters to the victim's future sees the exculpating half. §18.5 now
says so [author, 2026-09-03]. The reviewer's High was scoped to a
reputation surface the design does not have.

**2. Manufactured currency fallbacks (REASONING → repaired, §12.6.5 +
§19.7 item 12).** Verified fully: stapling's stated purpose, the fallback
undoing it, frequency as the stated limiter, and the inversion — the
limiter is the patron's own signature. Repair applied (author: "seems
fine"): an attestation never names its querier, so any fresh one is a
reusable staple, and the subject may request its own directly (the wire
already permits it: the requester is the authenticated peer, nothing
requires peer and subject to differ). A patron sustaining the inversion
must refuse its own subordinate while answering strangers about them, which
the subordinate sees — covert only until used, answered by §18.5's exit.

**3. Selective resolution censorship (EXTENDS → claim softened, §18.4
extended).** Verified: design §12.6.1 said "an intermediary gains nothing
by lying" — true for impersonation, false for a censor whose goal is the
failed handshake. Author: the phrasing was not his; soften or remove.
§12.6.1 now claims impersonation-impossibility only and names denial as
what lying retains; §18.4 gains the cheap selective form (chosen targets
unreachable inside a healthy view, one false reply per retry, P26 telling
the censor whom the victim keeps trying to reach); wire §7.7.3's two echoes
aligned ("nothing polices referral content, because impersonation is
self-detecting").

Both harnesses green after repin; no wire semantics touched.

**0.9.1 finding 2, author addendum (2026-09-03)**: no single patron's path
is load-bearing for identification — an attestation vouches for the key,
and a multi-subnet user has an issuer per line, so the inversion surveils
only the line its issuer controls. Added to §12.6.5 and §19.7 item 12.

## Cycle 2, pass 0.9.2 — adversarial: the witness (2026-09-03)

Two NOVEL, one EXTENDS, zero REASONING; eight lines self-discarded as
restatements, and the reviewer correctly declined to revive the retired
nonce/grinding findings and the already-repaired Potemkin rationale. All
three findings verified against the text and ruled by the author.

**1. Radius without centre (NOVEL → ruled: witness-relative, social
interpretation).** Verified in full: §7.6's disks are witness-centred, a
record may carry zero assertions, and Corroboration = {witness, method,
radius_km} names no reference point — §7.6.2's evaluator claim had no
defined computation behind it. Author ruling: the radius is
witness-relative ("W bounds the participant within R km of W"), no
coordinate is carried, and interpretation is by recognition of the witness
— §16.1's evidentiary model applied to geometry. A falsely tight radius is
ordinary attested-evidence lying, weighed like a false protocol_ran bit.
Applied to §7.6.2 prose and the wire schema comment. No new fields, no
privacy cost — the centre-field repair was rejected for exactly the C1/P2
cost the reviewer flagged.

**2. Uptime weighting (EXTENDS → accepted and stated).** Verified: the
probe-after-draw rationale removes advertisement, not availability, and
the inactive-fraction figure the effect size depends on is already
registered unsourced (§20.2). Applied: §7.1.1 now states the filter is not
uptime-neutral and cannot be (a witness that is not there cannot be
nominated); §18.4's Potemkin item prices the cheaper path in — a
continuously online placement survives every draw it enters — with the
bounds unchanged: branch spread, and no control over which ceremonies draw
it.

**3. The attests=0 witness (NOVEL → floor strengthened, author: "zero
witnesses is still available, so force it to be visible").** Verified: the
bitfield carried no floor semantics and the ≥1-witness rule's own rationale
was defeated by a witness attesting nothing. Applied: wire §3.2 — a normal
record MUST carry at least one witness with protocol_ran AND
both_responsive set; latency_bound stays optional; zero-bit and
latency-only entries remain representable as partial evidence that does
not satisfy the floor; the uncorroborated meeting stays expressible as a
formation record, visible as what it is. Bitfield comment and design
§7.1.1's balance passage aligned. T31 row + U-witness-no-affirmative unit
entry added; both harnesses taught the floor (Python 80 checks — the
normal record's own witnesses now checked; Rust runner enforces it on
subtype-0 bodies, 146/0). All existing positives satisfy it: single-witness
fixtures carry bits 7 or 3, and bits-3 fixtures pin latency_bound as
optional.

Defences the reviewer confirmed surviving: no record forgery without
participants, no verifier-response rewriting, no capture access by
witnessing, and cross-nomination improving representativeness rather than
honesty.

## Pass 0.9.3 blocked; the adversarial prompt rewritten (2026-09-03)

Run 0.9.3 (third adversary role) tripped the reviewing provider's
cybersecurity classifier. The provider-side feedback located the cause in
the review's operational shape — the run had moved from auditing the design
into optimising an adversary (cheapest paths, paid participants, scaling)
— and supplied a narrower defensive-deliverable format. The plan's 0.8
prompt is rewritten accordingly: authorized-defensive-review framing, a
prove-or-falsify-the-claims method, a seven-point per-finding deliverable
(sections, property, minimal preconditions, conceptual impact,
register-class, mitigation, validation check), an explicit stop-at-category
rule, and no attacker cost/scaling optimisation. Role 3 lost its budget
figure — the one role with attacker economics in its definition, and most
likely the blocked run. The four-class classification, one-role-per-session,
and registers-supplied conventions are unchanged, so ingest stays the same
— with one addition: deliverable items 6 and 7 (proposed mitigations and
validation checks) arrive as reviewer suggestions and are candidates for
author ruling, never decisions. Author is also replacing the external
safety disclaimer with the provider-suggested wording; output shape may
drift accordingly.

## Cycle 2, pass 0.8.3 — adversarial: the commercial operator (2026-09-03)

The re-run of the blocked 0.9.3, first round under the rewritten defensive
prompt — which worked: findings arrived as property/preconditions/impact/
repair with no operating plan, and the classifier passed it. One REASONING,
one NOVEL, two EXTENDS; fifteen RESTATES self-discarded. All four verified
and ruled by the author.

**1. Setwise conservation (REASONING, Critical → ruled normative).** The
strongest catch of the cycle. §16.2's "entire subtree inherits at most what
flows through that one vertex" is an aggregate claim; Advogato's cited
semantics deliver it via one shared-capacity computation, but the text
never required joint computation, §16.1 reads naturally as per-node scores,
and rr §7.2.1 materialises per-member rows — independent per-identity
max-flows would reuse the same cut capacity once per identity, growing
aggregate entitlement with population while every per-identity bound holds.
§18.4's Potemkin acceptance rests on the aggregate reading. Ruled: setwise
conservation is normative for the reference metric — for any set of
identities behind a cut, simultaneously usable standing totals at most the
cut's capacity; one computation, shared capacity; per-principal decisions
(§11.4) draw from one conserved computation. Stated at §16.2. Validation
is a property of the eventual implementation (the metric remains an
implementation item), recorded here rather than vectored.

**2. Coverage economics (NOVEL, High → bullet restated).** §16.3.1's
"work per-target, more expensive than the global reading suggests"
ignored horizon overlap: one acquired edge sits inside every horizon that
contains it. Restated as coverage of acquired edges over the victim
population; the conservative per-observer half (an invisible edge cannot
help that observer) survives and is kept. The superseded phrasing is
quoted in place per the register style.

**3. selection_basis tier alignment (EXTENDS, High → renumbered, author
chose option A).** The wire's own comment said basis 0 = "met, or in a
trust horizon" — §5.1's tiers 1 and 2 folded, the laundering path A23
depends on keeping visible (cheap structural placement acquiring the look
of acquaintance in the permanent record). Renumbered tier-aligned:
0 met, 1 in-horizon, 2 reachable (tiers 3-4), 3 discretionary. Every
"MUST be 0" recovery rule survives unchanged (recovery verifiers are met
by definition; "known" reworded to "met" at wire §4.1, design §9.x prose,
and the vector docs). Fixtures renumbered semantics-preserving (old
0/1/2 = new 0/2/3; value 1 uncovered by the three-response record and
said so); T27 updated to 0-3; verify.py matrix check now {0,2,3}; the
Rust runner's field-10 range widened to 3 — with a caught near-miss: the
first edit widened field 5 (result basis, still 0-2) instead of field 10,
the two checks being adjacent. Corrected before any run.

**4. "Counts identities" (EXTENDS, Medium → applied).** Wire §5.3 said
the pool "counts people" where dedup is by keyhash and design §13.7
permits one person several identities. Now: counts identities, one person
may hold several, and the record claims no human independence the
protocol cannot prove.

Confirmed defences recorded: the single-observer cut bound, recognition
against fabricated volume, uptime-not-social-trust, agents borrowing
standing, and §19.7 item 2's reconnaissance rationale — all matching the
registers. Python 80/80, Rust 146/0; references clean at 1,883 (eight
list-continuation false-negatives, two of them from today's own edits,
each inspected).

## Cycle 2, pass 0.8.4 — adversarial: the compelled provider (2026-09-03)

Two REASONING, two EXTENDS, zero NOVEL; eighteen RESTATES discarded — the
reviewer's own summary: the residual issues are places where the text
credits protections that do not follow, not missing attack paths. All four
verified and ruled.

**1. "Self-burning" (CRITICAL, REASONING → acceptance corrected, option
B).** §18.1 credited impersonation detection as architectural; verified
false three ways: one shared key across devices (§23.3's "seed shared
across wallets"), concurrent multi-device presence is normal not
anomalous, and the direct-path horizon includes never-met parties (P17).
Rewritten: per-target/prospective/no-bulk-collection stand as the
architectural residual; detection is contingent on actual acquaintance,
which the architecture does not guarantee; the superseded phrasing is
quoted. Role-separated credentials (the reviewer's repair) noted as
adjacent to Appendix B.1's superseded shapes; author deferred the
mechanism and commissioned a usability-impact analysis instead (below).

**2. Collector vs court (HIGH, REASONING → paragraph rewritten).** "Makes
the surveilled record less useful to whoever built it" conflated
transferability with collector knowledge. Now split: fabrication
capability discounts onward transfer and proof; it cannot reduce what the
collector knows about genuine observations — the actor knows which records
it fabricated. The acceptance claims relief only on the transfer ledger.

**3. Honesty axis scoped (HIGH, EXTENDS → §12.6.5.1).** "Independent on
the honesty axis" now scoped to adversaries who can only attack
availability; against a compelled provider hosting both, primary and
fallback are one control domain — correlated, not independent — with
§3.4's adversarial-independence rule carrying the load.

**4. Deletion bounds the node (HIGH, EXTENDS → §14.1.6, infra §2, P4, C5).**
"Nothing recoverable" is the guest's truth; a provider snapshotting below
the guest keeps the queue tuple after the VM forgets it — §18.1's
observation boundary applied to the one place that hadn't carried it.
Stated at all four sites: hygiene against the provider, the real bound
against everyone above the hypervisor.

Confirmed defences recorded: direct-path content outside the instance
(contingent on P12's E2E), no participant-key reach from infrastructure
alone, sealed captures off-infrastructure, and the corrected ASN asymmetry.

### Commissioned analysis: usability impact of the operator key leaving the infra node

Author directive: analyse impacts only, no redesign planning. The
always-up-signer assumption is structural in three places and incidental
in several more.

**Tier 0 — session authentication.** Wire §9.1's mutual transport
authentication runs under the node identity. Without the key (or a
delegated transport credential — B.1 territory), the instance cannot
accept an attach, answer a resolution, receive a flood, or be dialed.
Everything below sits on this.

**Tier 1 — continuous-cadence signing the design is built around.**
(a) Currency staples: ~10-hour lifetime, refreshed per subordinate,
issued to fallback callers (§12.6.5). The lifetime was derived against
compromise-detection latency on the implicit premise that refresh is free
because the signer never sleeps. Key on the operator's device makes staple
freshness a function of human device availability; one night offline
approaches the lifetime, and §12.6.5.1's degradation cascade becomes a
consequence of sleep schedules rather than outages. (b) Sibling fallback
issuance: the ladder exists precisely for when the patron is DOWN — with
off-node keys it would need other operators' devices present during
someone else's outage, converting an infrastructure property into a
social-availability property. (c) §6.4 countersignatures and SubtreeAck:
unattended per Appendix A; latency becomes device-cadence-bounded and
flows into subordinates' transaction weight.

**Tier 2 — event-driven signing that tolerates latency worse than it
looks.** EndpointRecord signing on address change — the record exists to
END unreachability, and an address change during operator absence extends
the outage until the operator signs. The §10.2 automatic cycle-repair
disavowal would defer to the device. Heartbeats, acks, queue service and
forwarding are unsigned but live inside Tier-0 sessions.

**Tier 3 — genuinely human acts, unaffected.** Attested adoptions,
recovery, deliberate disavowals, peering: operator-present or
latency-tolerant by nature.

**Summary**: a key split forces either a delegated credential covering
Tiers 0-1 (reopening B.1), or accepting that currency freshness and
fallback issuance become functions of human availability. The 10-hour
staple lifetime and the sibling ladder are the two mechanisms DESIGNED
around continuous signing; under a split they need rederivation, not
tolerance. The author's suspicion is confirmed: the assumption is
load-bearing and pervasive.

## Cycle 2, adversarial pass 0.6.5 (role 5: the malicious counterparty, 2026-09-03)

Author's round label 0.6.5; the fifth adversary role under the defensive
prompt. Three EXTENDS, two NOVEL, zero REASONING; eleven RESTATES
discarded. All five verified and ruled — this round changed the ceremony
evidence layer more than any since the family began.

**1. Adverse-response suppression (High → dual delivery, author ruling).**
Verified exactly: §7.4.2's substitution defence read "a subject seeing
no_match returned about themselves will withhold [signature]" — but
responses returned only to the querier, who in the substitution scenario is
the attacker. §2360's "neither fabricate nor suppress" lost its suppress
half when deterministic selection retired. Ruled (author: verifiers already
hold a point-to-point association with the subject for the capture-key
grant, so returning the verdict adds almost nothing): a verifier answering
a query about S delivers a copy of its signed response to S; S's client
withholds the envelope signature from a body omitting a response S holds
(light §1.2). Suppression now requires both participants, and a colluding
pair buys a visibly thin record §5.2 discounts. §2360 rewritten with the
superseded claims quoted; wire §5.6 carries the delivery rule.

**2. Consent was bearer paper (Med-High → field 7, author ruling).**
query_id hashed fields 1-5 — identical for every verifier in a ceremony —
so one consent authenticated the query to any prior counterparty. Ruled:
VerificationQuery field 7 names the addressed verifier, sits inside the
hash (the field-6-absent rule needed only its parenthetical updated), and
a verifier rejects before processing any query not naming it. One consent
per verifier follows. Recovery: field 7 MUST equal field 2. Author
rationale recorded: witnesses must observe all participants; verifiers are
drawn from a wider pool and need not. P18 updated — curation now backed by
cryptographic confinement. Fixture layer rebuilt: vquery takes the
addressee; the three npr queries now share ONE profile (as the
one-profile-per-ceremony rule always intended — the old fixtures varied
profiles to get distinct qids) and differ by field 7; recovery echoes
field 2; KeyGrant/worked-query bindings recompute; both harnesses updated
(the runner's generic map-without-field-6 hash absorbed field 7
automatically; verify.py's three explicit field tuples now include 7, and
its request-4 basis bound was found still reading 0-2 from the
renumbering — fixed to 0-3). TR24 added: wrong-addressee type-4 → close
stream. Twenty-four traces.

**3. nominated_by self-check (Medium → light §1.0 rule; author: "weird
this wasn't stated as a rule previously").** The split check counted
halves; nothing compared entries claiming your nomination against your
local nomination set. Rule added: refuse to sign a record attributing to
you a witness you did not nominate — the wire records the claim, you are
the one party who can check it.

**4. Fishing is bundle augmentation (Medium → §8.1.2, wire §5.1, light
§1.2, P37; author-designed rule).** The fishing path disclosed
outside-bundle history "recorded nowhere". Author's design applied: each
proposal is a bundle augmentation through the same curation; clients stop
revealing at a locally adjustable count of responsive candidates; after
the bundle, every mutually reachable candidate is accepted if available —
quality filtering ends with the initial bundle, so declining available
candidates to extract more names is visible to the party being fished.

**5. Field 10 carriage semantics (Low-Med → wire §5.6; author: "false met
is the same as not available").** The verifier's signature binds the
selector's claim against alteration — the nominated_by pattern — and
policy MUST NOT weight it as verifier-attested. A claimed `met` the
verifier's own records refute is answered `unavailable`: the false claim
voids the selection premise.

Confirmed defences recorded: no fabrication of positive responses, the
capture-key gate, recognition as the selector's local defence, and
cross-nomination as representativeness. Python 80/80 (three query_id
sites now hash fields 1-5+7), Rust 146/0 with field-7 required in its
query schema. References: nine known list-continuation flags of 1,903.

## Cycle 2, adversarial pass 0.8.6 (role 6: the stolen device, 2026-09-03) — the pass completes

Two REASONING, two EXTENDS, zero NOVEL; five RESTATES discarded. All four
ruled "yes to all" by the author. This closes the sixth and final adversary
role.

**1. Stolen operator phone = stolen infra authority (Critical, EXTENDS →
§18.3 bullet added).** The reverse of 0.8.4's finding: §23.3's one seed
means the phone satisfies every unattended signing context the instance
does, with no host compromise. Same answer (rotation via the
patron-countersigned reissue), with the window's asymmetry stated:
thief-issued currency dies with the staple lifetime; a thief-signed
disavowal is durable. The role-separation mechanism stays parked with the
commissioned usability analysis; the entry prices the shared-seed model.

**2. "Adverse results remain visible" (REASONING → §18.3 bound 1
rewritten).** The paragraph refuted itself — "the thief signs the record
whatever comes back" two sentences before claiming visibility.
Unforgeability is not completeness; in the both-adversarial case both
custody legs of §5.6's dual delivery are the colluders', so adverse
becomes absent and the record finalises thin. The bound now stated:
nothing positive can be manufactured; the evaluator's instrument is
weight — thin set against claimed n, responders by recognition, the
colluder's genuine match theirs to answer for. The reviewer's
roster-commitment repair declined in the text itself: it would
reintroduce the selected-set machinery retired 2026-09-01.

**3. Flow cap narrowed to the reference metric (REASONING → §18.3 bound 2
rewritten).** §16.2's setwise conservation is the reference policy's
property and §16.4's pluggability deliberately permits archive-tallying
evaluators the bound does not reach — the text now says exactly that.
The reviewer's conformance-profile alternative noted, not adopted:
soundness-as-fact is the §16.2 pattern.

**4. TTL bounds spend, supersession bounds use (EXTENDS → §21 parameter
row, §12.6.5 rule, infra §2).** "How long a compromised key keeps working"
overclaimed: routine payload fails open by design and rotation is not
global. Reworded to trust-bearing authority, and the new rule lands:
fail-open is for ignorance, never for knowledge — a party holding
authenticated supersession evidence (verified reissue chain, validated
recovery) terminates the superseded binding's sessions and delivers
nothing further to it, queues and capture-key grants included. One
self-caught reference error en route: the rule first cited bare §4.6
inside the design (a wire section); fixed before commit.

Confirmed defences recorded: the reissue/seal construction in scope, the
two-part recovery proof against key-alone theft, and sealed captures
staying ciphertext at seizure with the per-query residual correctly
scoped.

**The adversarial pass closes at six roles**: patron (3 findings), witness
(3), commercial operator (4), compelled provider (4), malicious
counterparty (5), stolen device (4) — twenty-three findings ruled, zero
left open, four superseded-claim corrections quoting their earlier drafts
in place, two new wire mechanisms (dual delivery, addressed consent), one
schema renumbering (tier-aligned basis), and the registers extended
throughout. Every round under the rewritten defensive prompt cleared the
classifier.

## Pre-0.9 coherence, consistency and de-linting pass (2026-09-03)

Author-directed sweep before the style pass. Findings and dispositions:

**Claims left half-swept by the cycle's rulings, now completed:**
- §17.3's preamble still said "an attacker must work per-target rather than
  accumulate standing globally" — the uncorrected sibling of §16.3.1's
  amortisation overclaim. Now states the coverage bound. (The §1
  benchmark-language per-target mentions are the PRICED claim — A-register
  rows 1 and 1.2.4 — and stand.)
- §12.6.5's lifetime-derivation sentence said a captured staple "retains
  full capability" — pre-0.8.6 shape; now "the key's full trust-bearing
  capability", pointing at the supersession rule.
- Wire §13 item 2 said the vectors were "verified by no implementation" —
  stale since the Rust runner; now states the cross-language/cross-crypto
  reproduction, its same-author limit, the ten clean-room traces, and the
  unchanged bar (independent-party reproduction).
- Infra's Open item on gateway pre-evaluation contradicted P24's reduction;
  aligned (catalog lookup precedes connection; residuals are §1.1's
  claim-not-guarantee and out-of-scope viewers).

**Reference checker at true zero for the first time**: the nine standing
"known false negatives" were manual exemptions by another name — every
list-continuation cross-ref now carries its doc prefix, the RFC 4271 cite
reworded to "section 4.3", and the checker reports 0 flags of 1,917 with
no inspection residue.

**Verified clean**: no trailing whitespace, tabs, or double spaces; no
duplicate section numbers in any document; §21.1's twelve parameters
count against §22.2's sentence (1+8+3); §14.2.4's five integration
decisions enumerate; §22/§23 consolidators current (build-order item 5
already points at §16.2/§16.4 where setwise conservation now lives);
light-doc Open items both still genuinely open; the superseded-claim
sweep finds only the four deliberate earlier-draft quotes.

**Flow repairs from the heavy-edit regions**: §18.1's orphaned "So" after
the removed sentence; nothing else mangled on read-through of §12.6.5,
§18.1, §18.3, §16.2-16.3.1, wire §5.5-5.6.

Both harnesses green (Python 80/80, Rust 146/0). Ready for 0.9.

## Cycle 2, pass 0.9 — organisation (2026-09-04)

The reviewer assessed the post-migration structure clean-room; findings
sorted three ways and the author ruled on each class.

**Already adjudicated (reported, not reopened):** further splitting design
§7 (author's standing ruling: three ways, not four — the ceremony is not
divisible without cutting one argument); a master open-item index (§22 IS
the index by ruling; the four local lists are referenced, not absorbed);
vocabulary order (moved to §2 in the migration; residual handled below).

**New, cheap, applied (author: "do all of these"):**
- Wire §1.1 split into §1.1 domain separation / §1.2 deterministic
  encoding / §1.3 global structural bounds / §1.4 content addressing —
  165 lines were under one heading. No cross-document citations existed;
  the front-matter pointer now reads §1.1-§1.4. One live misresolution
  found and fixed en route: a bare "§1.1" in wire §5 meant DESIGN §1.1
  and resolved silently against wire's own — the class of error the
  checker cannot catch.
- Light §1 renumbered: 1.0→1.1, 1.0.1 promoted to 1.2 (Acting as a
  witness is a sibling activity, not a child of Nomination), 1.1-1.3 →
  1.3-1.5. Two citations updated (one internal, wire §3.2's).
- Thirty-six unnumbered H4/H5 headings numbered mechanically across wire
  and design (4.5.1.1-.5, 10.1.1-.3, 10.2.1-.4, the §7.5.2 family, and
  the rest); two mis-leveled H5s found sitting directly under H3s and
  corrected to numbered H4s (7.2.1, 11.5.1). Depth invariant verified:
  components+1 everywhere.
- Appendix A subsections lettered A.1-A.4 to match B.1/B.2; the three
  name-form citations now cite A.2/A.3.
- The Open chapters numbered (light §9, infra §12); design §22.4's
  §Open citations updated.
- Force-of-requirements boilerplate deduplicated: one sentence + Appendix
  A.2 citation in each requirements document.
- A notation bridge added at the wire front (name → defining section) and
  a four-term italic pointer at design §1's head.

**Structural splits (author: "rename chapters in place"):** the
rename-in-place middle path adopted over a second migration — design §12
is now "Addressing, resolution, and reachability under change", §15
"Propagation, horizons, and the rootward memo", wire §7 enumerates its
families in the title, infra §10 is "Role assignment and hosted-session
lifecycle", resource §7 "Roles, and what the accessing user sees". No
numbers moved; the discoverability half of the size complaints is
answered, and the splits themselves are declined for v1.

Not adopted: cross-layer rationale dedup (the reviewer's item 7) beyond
the boilerplate — the "repeat the contract sentence; cross-reference the
explanation" rule is already the authoring convention, and a
prose-consolidation sweep is deferred with the splits.

Checker at 0 flags of 1,931; depth invariant clean; harness green after
repin.

## Stage 1 formal models built (2026-09-04)

Author directive: build all Stage-1 models, install all dependencies,
textbook-commented for line-by-line review. Delivered under `models/`,
outside the design set (the reference checker is unaffected: 0 flags of
1931). All eight artifacts pass; `models/run-all.sh` is the regression gate.

**Tooling, user-local, no root** (~/tools): Temurin JRE 21 + tla2tools for
TLC; Tamarin 1.12.0 + Maude 3.5.1; Python 3 stdlib. python3-venv was
unavailable and unneeded — the simulator imports nothing outside stdlib.

**1. Trust-metric simulation** (`simulation/flow_metric.py`, own max-flow
implementation, no deps): E1 reproduces §16.2's divergence figures
(~19,500x / ~2x), E2 the individual cut bound to 341 identities, E3 the
setwise-conservation saturation the 0.8.3 review made normative (independent
sum grows, conserving joint flat at the ceiling), E4 the §16.3.1 coverage
bound (changed observers == horizon-holders, exactly).

**2. TLA+ / TLC** (3 models): PartitionMerge (convergence restated for
no-shared-state: heal-then-agree; 15,080 states, SelfTruth/NoInvention/
Convergence all hold), CurrencyEscalation (ladder no-deadlock + issue-fresh;
no error), CycleDetection (concurrent-adoption cycles always resolve; no
error). All run -deadlock (they terminate; quiescence is not a bug), noted
in each cfg and run-all.sh.

**3. Tamarin** (4 theories, 13 lemmas, all verified): attach (server auth,
no sibling impersonation, no replay), currency (expired key can't appear
current, expiry as event order), recovery (neither factor alone — the two
mirror impossibilities), ceremony (record implies co-presence; roster
binding; the co-presence and face-recognition AXIOMS stated explicitly per
the plan's honesty instruction).

**What the models found** (the point of the exercise): the recovery model
falsified twice before converging. (a) First version let the verifier sign
the NEW KEY and omitted subject!=verifier; Tamarin found a self-recognition
trace where a stolen key signs both factors -- fixed by matching the design
(verifier recognises the PERSON; new key only from the old-key proof; wire
s5.3 distinctness enforced), which CONFIRMED wire s5.3 is load-bearing for
recovery. (b) A combined both-factors lemma was falsified by a thief
producing factor a via adversary signing (honest rule never fires),
surfacing the model boundary -- stolen key + identity recognition succeeds
because symbolic models have no face check (design s18.3's residual). The
provable form is the two mirror "neither alone suffices" lemmas, exactly
s9.1's phrasing. Recorded in models/README.md, not silently patched.

Author will cross-review the models against the other artifacts and read
line by line.

## Cross-family review: trust-metric simulation (2026-09-04)

An external clean-room review of `models/simulation/flow_metric.py`
brute-force-validated max_flow against 700 random graphs (zero
discrepancies) and found six issues in the MODELLING around it, all
verified against the design and all correct:

- **HIGH (E4 horizon semantics)**: the sim used one adjacency for three
  distinct design concepts. §6.3 and §15.1 separate SCOPE (adoption+sibling
  edges → horizon) from TRUST-CAPACITY (adoption+peering → flow), and
  §16.3.1 gives peering-edge VISIBILITY (visible inside both peers'
  horizons). The old code made the acquired peering edge itself confer
  scope, understating coverage ~4x (the reviewer's corrected seed-1 figures:
  13/19/18 vs the old 3/6/5). FIXED: scope_adjacency (adoption+sibling),
  visible_flow_subgraph (peering visible iff a peer is in the observer's
  scope horizon, conferring no scope). Corrected coverage now min/median/max
  7/16/23 at seed 1, matching the reviewer's independent range.
- **MEDIUM (E4 assertion)**: old code asserted `changed <= holds_edge` while
  printing "influences EVERY observer... CONFIRMED". FIXED: asserts
  `changed == holds_edge` per placement (a theorem given the construction:
  any node in the scope 2-ball is flow-reachable via adoption edges within
  the horizon).
- **MEDIUM (E3 visibility)**: old E3 ran the conserving computation over the
  GLOBAL graph, using fakes 3+ hops away that the observer could not see.
  FIXED: E3 is now observer-visible -- a wide fake fan within the horizon
  saturates at the visible ceiling, and a deep fake subtree is shown to add
  zero visible identities (§16.3.1's conservative direction).
- **MEDIUM/UNSPECIFIED (allocation rule)**: max-flow VALUE is unique but the
  ALLOCATION among symmetric principals is augmenting-path-order dependent.
  accepted_count reframed as a conservation BOUND, not an allocation; the
  design question flagged in the sim output AND here for author ruling ---
  **does the reference metric need a deterministic scarce-capacity
  tie-break, or is that left to 16.4 policy pluggability?** Not decided.
- **LOW (E1 boundary)**: old code classified fλ=1 as convergent; the design
  says diverges unless fλ<1, so fλ=1 diverges (linearly). FIXED: three
  regimes, diverges when fλ>=1, boundary case asserted to grow unbounded.
- **LOW (E4 statistics)**: old code sampled 5 placements. FIXED: enumerates
  every interior placement, reports min/median/max and worst-case coverage.

E1's underlying divergence and E2's cut bound were confirmed valid by the
reviewer; those needed only the boundary fix. The reviewer correctly noted
E1's weakness is the design's own §16.2 (already documented), not a new
finding. All ten seeds pass after the rewrite; the other seven models
(3 TLA+, 4 Tamarin) are untouched and still green.

**OPEN FOR AUTHOR**: the allocation/tie-break UNSPECIFIED above.

## Author ruling: scarce-capacity tie-break (2026-09-04)

The allocation UNSPECIFIED from the trust-metric review is closed. Ruling:
the reference metric resolves a scarce-capacity tie deterministically --
the earlier-considered candidate wins -- but this is REFERENCE POLICY, not
a network-wide invariant. Per-observer trust (s16.1) means no party
consumes another's trust computation, so two nodes may resolve the same tie
differently and both conform; a node's own decisions are stable, which is
all that is needed. Individual standing (each principal's own max-flow) and
the aggregate bound remain unique values every flow policy agrees on; only
the allocation among equal-standing principals is policy. Applied to design
s16.4 (alongside the lambda "reference implementation, not protocol rule"
line it mirrors), and to the simulation (accepted_count documents the rule;
experiment_setwise tests earlier-wins on the minimal symmetric case).
Reference check clean at 1934; both vector harnesses and all eight models
green. No open items remain.

## Second cross-family review of the trust-metric simulation (2026-09-04)

Three findings, all verified by execution before applying, all correct.
The reviewer independently cross-checked max_flow against exhaustive
min-cuts on 500 random graphs (clean), so again the defects were in the
modelling, not the algorithm.

**F1 (Medium, MODEL GAP -> author re-ruled the design rule).** The
2026-09-04 tie-break was NOT implemented: I had claimed passing candidates
in order to a single multi-sink max-flow realises "earlier-considered
wins". It does not -- BFS augments the SHORTEST path first, so a nearer
later candidate takes the scarce unit. Reviewer's counterexample
reproduced exactly (obs-1->bot-1->x-1->A, bot-1->B, order [A,B] admits B).
**The author then re-ruled the design rule itself**: the shorter path
SHOULD dominate -- "a longer path is less trustworthy by nature" -- with
first insertion breaking TRUE TIES only. s16.4 rewritten into two limbs
with that justification (the author's, not invented), plus an implementer
note: one multi-sink max-flow delivers limb 1 free and misses limb 2
SILENTLY, because at equal path length it follows the GRAPH CONSTRUCTION
order rather than candidate order -- measured here: two equidistant
candidates passed as [B,A] still admitted A. The simulation now ranks by
(path length, consideration index) explicitly and tests both limbs
including construction-order independence.

**F2 (Medium, MODEL GAP).** E4's `changed == holds_edge` was an artifact of
a one-ended synthetic attacker with no scope position: a node with no prior
path gains one from every observer that can see its only edge, so the
equality held by construction. Verified the reviewer's disproof (two
disjoint trees, real two-ended peering: 9 see, 6 change, 3 unchanged
because they already held standing to the peer through their own tree).
E4 rebuilt: two disjoint trees, peering between interior nodes, a FIXED
beneficiary so every observer answers the same question, all 80 cross-tree
placements enumerated, three quantities reported separately. Now 14-28 see
vs 8-12 influenced. The equality is NOT asserted; what is asserted is the
design's actual economic claim (one edge influences many) plus the
conservative direction (no invisible edge influences anyone). Also fixed
the noted `>= 1` vs `> 1` weakness.

**F3 (Medium, UNSOUND ABSTRACTION).** accepted_count's unit drains count
ADMITTED IDENTITIES, not "simultaneously usable standing" as s16.2 words
it. Verified (obs-10->t: individual standing 10, accepted_count 1). Added
deliverable_flow() for arbitrary per-target demands and a general test
where every candidate demands its own individual standing (sum 160,
deliverable 10, cut 10) -- the strongest reading of the sentence. Both
properties now tested and distinguished in the prose.

**UNSPECIFIED 2/3 (node-capacity schedule 32/16/8/4; peering:hierarchical
10:2).** Both verified as MODELLER CHOICES not in the design -- s21.1 lists
peering flow capacity as an unset parameter. node_capacity() now says so
explicitly and notes the experiments test relative behaviour under one
fixed schedule.

All six seeds pass; all eight models green; references clean at 1935.

## Author correction: available flow ranks before path length (2026-09-04)

Ruling refined during the second simulation review. The scarce-capacity
allocation rule has THREE passes, not two: **available flow first** -- the
total capacity reaching a candidate, which is what the metric measures --
then shorter path, then consideration order for true ties. Author's
reasoning: number of edges / total available flow is always the first pass;
preferring a nearer candidate over a better-supported one would substitute
the tie-break for the measurement. Applied to s16.4 with the note that this
pass is invisible in the single-chokepoint case (every candidate behind one
saturated cut carries identical flow), which is why the section states it
explicitly -- s16.4 is about pluggable policy generally, not only about
candidates sharing a bottleneck.

Simulation updated: admit_reference_order ranks by
(-available_flow, path_length, consideration_index), all three measured on
the pre-allocation graph. New discriminating case added -- a nearer/thinner
candidate B against a further/wider A, where flow-first admits A and a
distance-first ranking would admit B. **Mutation-tested**: deleting the flow
key makes the new assertion fail, confirming the test is not vacuous; the
key was then restored and the full run passes. (The deliberate failure was
mid-turn verification, not a regression.)

Six seeds pass; all eight models green; references clean at 1936.

## Third cross-family review of the trust-metric simulation (2026-09-04)

Three findings, all verified by execution, all correct. max_flow again
cross-checked clean (600 graphs incl. antiparallel capacities) -- three
independent reviews now, zero discrepancies; every defect found has been in
the modelling.

**F1 (Medium, MODEL GAP -- the real one).** Joint drains attached to
("out", candidate) while score_independent terminates at ("in", target), so
the joint computation charged every candidate its OWN node (relay)
capacity. Reproduced exactly: obs-10-x-10-y-10-A gives individual standing
8 but deliverable_flow({A:8}) = 4, with no contention at all. Node capacity
models what a node may RELAY; a candidate here is the DESTINATION. Fixed:
both admit_reference_order and deliverable_flow now drain from
("in", candidate); individual and joint now agree (8 = 8). This mattered
specifically for the general-demand E3 claim -- unit demands were mostly
insulated because node capacity bottoms out at 1.

**F2 (Medium, VACUOUS PROPERTY + placement restriction).** E4's assertions
(max(sees)>1, max(infl)>1) establish that amortisation EXISTS, not the
population-coverage economics s16.3.1 argues; the terminal "CONFIRMED"
overstated them. Also E4 restricted peer endpoints to nodes with children,
but infra status is independent of downline -- wire s10.1: "a node becomes
infra by launching and signing an infra instance, without moving" -- so
low-degree placements were silently excluded. Fixed: every node is a valid
endpoint (48 placements over 14 observers, all enumerated), and the label
is now "DEMONSTRATED IN THIS TOPOLOGY (not confirmed as economics)" with an
explicit list of what is NOT measured (edges per target fraction, cover-set
overlap, variation across topology families).

**F3 (Low, stale summary).** The final report block still described two
limbs and omitted the available-flow pass added hours earlier -- precisely
the consolidating-section decay the project's own conventions warn about.
Fixed; the implementer note now distinguishes all three passes (a plain
multi-sink max-flow gets the PATH-LENGTH pass free, misses the TIE pass
silently, and cannot deliver the FLOW pass at all).

**U1 flagged for the author, NOT decided**: what constitutes an EDGE in the
reference flow graph? The simulation uses current adoption topology plus
visible peering. s16.1 computes trust from observed transactions and s16.7
makes an archive portable, so an implementer could reasonably treat
verified historical adoptions as trust-graph edges -- which would change
which cut binds. The reviewer's working assumption was that archive
evidence seeds or weights trust without creating live capacity edges
unless the reference metric materialises one. **This is a specification
question about the reference flow-graph construction rule (which live
topology, historical adoptions, portable archive evidence, departures and
peering records become or cease to be capacity edges), not a demonstrated
flaw.** E2/E3 remain valid arithmetic over the graph they are given.

Six seeds pass; all eight models green.

## Author ruling: what constitutes an edge in the reference flow graph (2026-09-04)

U1 from the third simulation review is closed. Two parts, both applied.

**Peering.** Not a first-order trust edge: it does not expand the horizon
in which transaction records flood and resources are shared (already §6.3).
NEW: what a peer persists is an **encrypted** backup, so the entrustment is
durability, not readable content -- the one thing a peer gets that an
ordinary acquaintance does not. A peer is a persistent trusted acquaintance
from OUTSIDE the subnet and gains no status beyond the node it peers with;
toward subnet state and resource access its position is a PoP counterparty's
(none by virtue of the relationship). **The peering graph is orthogonal to
the routing/authority hierarchy exactly as the PoP graph is, and to any
third party both kinds of edge carry trust by the same rules.** Applied at
§6.3 and §16.2.1.

**Archives.** Portable, but used only for the node's internal reference and
for **adoption-time review**. The reference implementation reviews the
adoptee's archive counting ONLY transactions whose other participant the
adopting patron already recognises (§16.7 already said this) -- NEW: and
**weights each by the flow-implied veracity**, i.e. by the flow the
patron's own graph can push to that recognised participant. So an archive's
value is bounded twice by the reviewer's own graph: which transactions are
legible, and how much each weighs. **Review, not edge creation**: an
archive informs the initial trust state and installs no standing capacity.
Applied at §16.7 and §16.2.1.

**New subsection §16.2.1, "What counts as an edge in the reference graph"**,
states all three sources (hierarchy edges; PoP+peering acquaintance edges;
archive-as-review) in one place, with the reason the ordering matters: if
presented history created edges, a region could enlarge its own cut by
presenting history, and the cut would stop being a property of the
evaluator's graph.

**FLAGGED FOR THE AUTHOR, not resolved by me**: §16.3 still says peering
carries "lower flow capacity than hierarchical edges by default", while the
ruling says trust flows over PoP and peering edges "equally" for third
parties. I read these as compatible -- same KIND of edge (this ruling),
default VALUE still a policy parameter (§21.1 lists the peering ratio as
unset) -- and wrote §16.2.1 that way. If the intent was that peering and PoP
edges also carry the same default CAPACITY, §16.3's mitigation paragraph
needs revising and I have not touched it.

References clean at 1945; harnesses and all eight models green.

## Author ruling: the trust landscape (2026-09-04)

The peering-capacity question I flagged is answered by a geometry that
resolves it from an unexpected direction, and the answer reaches further
than the question.

**The ruling.** Hierarchical edges are UNTHROTTLED out to the two-edge
patron/sibling horizon; beyond it is where the flow metric throttles. So
**everyone in a node's trust horizon sits at the ORIGIN together** -- the
horizon is a point, not a graded region -- and distance counts edges outward
from it: a node's own PoP counterparty and its patron's sibling's PoP
counterparty are BOTH at distance 1. **Beyond the horizon, trust flows
equally over the hierarchical and the PoP/peering graphs**: the metric
distinguishes inside from beyond, never edge kind from edge kind. A peer has
special status only to the node it peers with (the encrypted backup and its
cost); to everyone else it is a PoPmate.

**Applied**: new landscape paragraphs in §16.2.1; §6.3's "lower flow
capacity than hierarchical edges" bullet rewritten (the comparison is a
landscape question, not an edge-kind one); §16.3's mitigation marked
SUPERSEDED with the reasoning in both regions.

**Three consequences swept, each of which would otherwise have rotted:**
1. **A20's stated mitigation is retired.** The assumption "a peer may read a
   technical request as an endorsement" was answered by §16.3's low default
   peering capacity, which no longer exists. Row updated to say so and
   point at §16.3's open note.
2. **§21.1's "peering flow capacity relative to a hierarchical edge" is
   DISSOLVED** -- there is no ratio to set when the metric does not
   distinguish edge kinds. Row struck with the reason.
3. **The parameter count therefore drops twelve -> eleven**, corrected in
   BOTH places that state it (the Preface and §22.2). This is the
   counts-drift trap the conventions warn about; caught only because the
   dissolution was applied deliberately rather than by leaving the row.

**Open and recorded, not inferred** (§16.3's block quote): if peering and
PoP edges sit at the same distance for third parties, the
endorsement-misreading concern has lost its answer and the face-to-face
upgrade path has nothing to upgrade. The asymmetry is real -- a peering
costs a technical favour, a PoP costs a physical meeting -- so what should
price that difference, if not capacity, is OPEN.

**Simulation rebuilt to the landscape.** node_capacity(0) is now
UNTHROTTLED; a new landscape_distance() computes horizon-as-origin with an
outward walk that treats adoption and peering edges alike; split_graph and
its four consumers take scope and peer edges so the RHTN-shaped experiments
use the landscape (the bare allocation unit-tests keep graph hops, noted in
place). PEER_CAP raised from 2 to equal HIER_CAP, since capacity no longer
varies by kind. Conclusions are unchanged -- edge capacities were the
binding constraint, not node capacities -- which is reassuring rather than
convenient: the earlier distance error was not load-bearing for E1-E4.

Six seeds pass; all eight models green; references clean at 1953.

## Author correction: the horizon is distance 1, not the origin (2026-09-04)

Corrected within the hour, before the earlier version could propagate.
**Only the user sits at the origin.** Distance 1 is the WHOLE trust horizon
collapsed into one step, TOGETHER WITH anyone the user met themselves (PoP
counterparty or peer), in or out of the horizon. Distance 2 is a node
outside the horizon one edge from a horizon member or from one of the
user's own counterparties, and so on outward.

**This supersedes the author's own earlier example**, which had "my PoPmate
and my Patron's Sibling's PoPmate both at distance 1": under the correction
the patron's sibling is at 1 (a horizon member) and THEIR PoPmate is at 2
(outside the horizon, one edge from someone at 1). §16.2.1 now carries the
corrected example and the phrase "the horizon flattens; the world past it
does not."

**A distinction the correction forces, and which the simulation now keeps**:
DISTANCE and THROTTLING are different functions. The design says
hierarchical edges are unthrottled out to the horizon and the metric
throttles beyond it -- so INSIDE-OR-BEYOND decides *whether* a node is
rationed while DISTANCE decides *how much*. Distance 1 contains both kinds:
horizon members (inside, unthrottled) and the observer's own counterparties
who are not horizon members (outside, throttled at distance 1).
node_capacity() now takes both arguments.

**FLAGGED FOR THE AUTHOR, not decided**: is that split right? A
counterparty you met *yourself* sits at distance 1 but outside the horizon,
so on this reading it is throttled while a horizon member you have never
met is not. The alternative reading -- everything at distance 1 is
unthrottled, since the metric's job is reaching past your own acquaintance
-- is equally consistent with the sentences given, and changes what a
direct PoP is worth. I implemented the first because "beyond that horizon
is where trust becomes throttled" is the more literal reading of the text.

Six seeds pass; all eight models green; the E1-E4 conclusions are again
unchanged, edge capacities still being the binding constraint.

**Author confirmation (2026-09-04)**: the corrected landscape example is
right, and the author noted *why* it mattered -- "spelling it out like that
is how I realized it wasn't what I intended." Recorded as an authoring
convention (`authoring-conventions.md`, *Worked instances for structural
rules*): a structural rule carries a worked instance naming specific
parties, because an abstract rule can be read as what you meant while an
instance either matches or does not. This one caught the author's own first
example contradicting his own rule.

**CLOSED (2026-09-04)**: whether a counterparty the observer met ITSELF --
distance 1, outside the horizon -- is throttled. **Author: yes**, the
literal reading the simulation implements. The ruling came with the reason,
now in §16.2.1: **two mechanisms answer two questions.** The hierarchy
governs resources, routing and some countersigning, and the horizon is the
REACH of that effect -- membership is a privileged position for
hierarchy-specific operations. The flow metric answers GENERIC trust: does
this user exist, are they a real person, are they who they say they are.
For that question every horizon member is at distance 1, no nearer than
someone you met yourself. **A node inside the horizon is unthrottled not
because the metric rates it highly but because the metric is not what is
being asked there** -- you hold topology and direct evidence, and the
privileges are decided by position. Past the horizon the generic question
is the only one left, which is why throttling begins exactly there.

No open items remain from the landscape exchange.

## Consistency sweep of documents and models against the landscape (2026-09-04)

Author-directed. Documents first, then all eight models.

### Documents -- one real conflict, one reconciliation, references clean

**§12.7.6 (trust consequences of disavowal) contradicted the ruling.** It
explained a disavowed node's loss of external standing by "peering edges
whose flow capacity §16.3 deliberately sets low" -- reasoning the landscape
retired. The CONCLUSION survives for a better reason, now stated: losing
the patron edge drops the node out of the horizons it occupied through that
patron, so for those observers it moves from distance 1 (unrationed,
hierarchy privileges) to distance 2+ (where the metric is the only thing
answering and does ration), and the cut shrinks by the removed edge at the
same time.

**Reconciliation added at §16.2.1**: equal distance is NOT
interchangeability. §16.1 weighs a connection by *closest AND best
attested*, and attestation is a separate axis the landscape does not carry
-- a counterparty you MET and a horizon member you have NOT are both at
distance 1 for generic trust while remaining different security facts
wherever attestation is asked. Verifier selection ranks met first (§8.1.2)
and `selection_basis` encodes them separately (A23). Without this note a
reader could collapse the distinction A23 depends on.

**Checked and clean**: §4247 ("the horizon is a bounded set, not a trusted
one") reinforces rather than conflicts; §17.1, §17.2, §17.3, §16.5, §16.6,
§16.1 all consistent; the λ rows are about the REJECTED decay metric; the
resource doc's flow-metric mention concerns external consumers outside the
graph. No residual edge-kind capacity claims anywhere. References 0 of 1960.

### Models -- one stale comment, one substantive rebuild

**TLA+ / Tamarin (7 models): no landscape dependence.** Two horizon claims
verified against the design (CurrencyEscalation's grandpatron-inside-h=2
against §12.6.5.1; recovery.spthy's pointer to the flow simulator).
PartitionMerge's comment said "at this model's scale (four nodes)" after
the config was reduced to three -- fixed.

**flow_metric.py E3 was testing the wrong region.** After the landscape
correction, E3's fake fan sat INSIDE the observer's horizon -- where the
design says the metric does not ration at all, so the experiment tested
conservation in the one place conservation is not the operative constraint.
(It got there honestly: an earlier reviewer objected that the fakes were
invisible, and moving them inside the horizon fixed visibility at the cost
of region.) **Rebuilt faithfully**: the attacker now obtains peering edges
from ONE horizon member to a fan of identities holding no position in the
subnet -- visible (the peer's record is visible inside the horizon
member's horizon), beyond the horizon (peering confers no scope, so
distance 2, throttled), and behind one cut. That is exactly §16.2's
setting. Results: independent-sum 40/80/160 against a conserving joint of
4/8/10 at a cut of 10; general demands 160 asked, 10 deliverable; and the
same fan peered to a node OUTSIDE the horizon is 0-visible, the
conservative direction.

**Omissions now documented rather than silent**: the simulation has no
separate PoP edge type (§16.2.1 puts PoP and peering in one class, so
peering stands in for both -- but a model pricing ATTESTATION would need
them distinct), and no archive evidence (faithful: §16.2.1 makes archives
review, not standing edges).

Eight seeds pass; all eight models green.

**Author confirmation and rationale (2026-09-04)**: the equal-distance /
attestation reconciliation is correct, with the underlying reason -- *"you
trust the integrity of the local subnet you participate in, outsiders do
not."* Added to §16.2.1: the horizon's exemption rests on PARTICIPATION,
not proximity. Horizon members are covered by the structure that covers
you -- countersignatures you can check, topology that floods to you,
acknowledgements your position lets you verify -- and you rely on it
because you are inside it. **That reliance is first-person and does not
travel**: to an observer outside your subnet your horizon members carry no
exemption at all. Which is §16.1's per-observer rule appearing exactly
where it should, and it closes the frame: the metric's inside/beyond
boundary is not a statement about the world, it is a statement about where
the evaluator is standing.

---

## Cross-family review 4 of the trust-metric simulation (2026-09-04)

Reviewed `flow_metric.py` at SHA-256 `e9f2fb96...5ce24b` (the committed
file; hash matched). Reviewer ran seeds 1-20 and independently
brute-force-validated `max_flow` against exhaustive minimum cuts on 600
random directed graphs, 2-7 vertices, antiparallel edges included: zero
discrepancies. That is the third independent cross-validation of the
max-flow core (700/600/600 graphs, three reviewers, no discrepancies).

Three findings, all verified as stated. None was a defect in the flow idea;
all three were defects in the graph fed to it, or in the wording that
describes that graph.

**F1 -- sibling edges absent from the capacity graph. Verified as a
reading, and the DOCUMENT was wrong, not the model.** §16.2.1's first
bullet said *"adoption and sibling edges ... These carry trust"*, and the
model puts siblings only in `scope_adjacency` ("carries no capacity").
Reproduced the reviewer's differential exactly -- seed 5, cut 10 -> 30 on
materialising sibling edges at `HIER_CAP`, conservation holding on both
graphs. **Author ruling**: *"Sibling edges are an abstraction of
graph-distance in the patronage hierarchy, not in the trust graph."* So the
model was right and §16.2.1's wording generated the finding. Bullet
rewritten: adoption edges carry trust; siblings are a distance abstraction
authorised implicitly by the patron's adoption transaction and add no
capacity -- which would otherwise mint f(f-1) capacity edges out of f
adoptions.

**F2 -- allocation tie-break used raw split-graph hops. Verified, and
dissolved by the F4 ruling without touching the allocation code.**
Reproduced exactly: two candidates at landscape distance 2 with equal flow
measured 3 and 5 in the node-split graph, and the shallower won under both
consideration orders. The reviewer read this as `admit_reference_order`
using the wrong key. It was the wrong *graph*: hops in a COLLAPSED graph
are the landscape distance, and the model had not collapsed. After the
collapse both measure 3, tie, and consideration order decides -- verified
under both orders. §16.4's "fewer hops" now names the graph; the two
readings coincide once §16.2.1 is built as written, so this names rather
than adds.

**F3 -- PoP substituted by peering, and the graph stopped one shell out.
Verified, and sharper than filed.** The two are alike in distance and
capacity and differ in VISIBILITY, which is what the model needed: §5358's
propagation class makes attestation records *pull, not push* -- *"fetched
on demand by evaluators"*, no horizon bound -- while §16.3 confines a
peering record to *"the two peers' horizons and nowhere else"*. Collapsing
them imported the narrower regime, so `visible_flow_subgraph` could not
express a region behind an acquired edge at all (measured: `visible=0` at
every width). **Author ruling**: how far outward a graph reaches *"depends
what relationships you are aware of ... you can discern some of a foreign
subtree's structure from locator data"*; calculating it is not a
requirement, and the rule should be general so later implementations can
use what they have *"while keeping the same proven flow metric"*. Added to
§16.2.1 and implemented as `reach`, a modelling parameter the metric is
identical at every value of.

**F4 (not filed by the reviewer; found while verifying F1) -- E3 had been
passing for the wrong reason.** F1's differential put the cut under a
microscope, and E3's chokepoint turned out not to be an acquired edge but
**the observer's own in-horizon hierarchical edge**, throttled at
`HIER_CAP` by an uncollapsed graph. §16.2.1 says hierarchical edges are
unthrottled out to the horizon. Unthrottling them and rerunning: E2 and E4
pass bit-identically, and **E3 trips its own "test is vacuous unless demand
exceeds the cut" guard** -- deliverable rises to equal the independent sum,
so no conservation claim remained. An attacker who buys N edges should get
N edges' worth; there was never anything to demonstrate in that shape.
F1's 10 -> 30 also vanishes under the collapse, being a symptom of the same
defect.

**Author ruling on the collapse**: *"In-horizon edges are collapsed to a
single edge (distance 0 -> distance 1) ... nodes within each other's
horizons are always aware of each other, capable of point-to-point
communications. Even for a secondary patronage relation (two edges on the
hierarchy graph), there is a direct relationship that justifies collapsing
this distance. In short, the distance from a node to the edge of its trust
horizon is 1."* Recorded in §16.2.1 as graph construction rather than as
capacity, with both measured consequences of getting it wrong.

**E3 rebuilt** around the only shape setwise conservation speaks to: a
region of 4/8/16/32 identities behind ONE acquired peering edge, bought
once. Independent-sum 32/64/128/256 against a conserving joint of 4/8/8/8
at a cut of 8; general demands 256 asked, 8 deliverable; the same region
gated behind a node outside the horizon is 0-visible. The cut is the gate's
relay capacity at distance 2 (`node_capacity(2) = 8`), not the peering
edge's nominal 10 -- what a region inherits is what its gate can pass, and
the gate is throttled by how far away it is.

Seeds 1-40 pass. All eight models green. References: 1,967 checked across
the five specification documents, 0 flags (checker saved as
`Robot/refcheck.py`, no exemptions). Test vectors: ALL CHECKS PASS.

**Noted, not acted on**: `change-log.md` cites `Robot/` at 20 lines, which
the root-document rule forbids. Pre-existing and in a historical record;
flagged for the author rather than swept.

**Drafting-history sweep (2026-09-04)**, from the author's note on the
§16.2.1 provenance style: *"I don't love this style. We don't need to refer
to earlier drafts or review rounds. The spec has not been published yet, so
there is no backward compatibility to maintain."* Swept by search across the
five specification documents, not by section. Eight instances, all in
`network-design.md`: §8.1.2's carriage claim (rewritten to state that a
verifier response's absence is not visible, and why), §12.7.6's parenthetical
(rewritten to name the horizon boundary as what does the work), §16.2.1's
sibling bullet and §16.3.1's coverage paragraph (both mine, rewritten),
§18.1's self-burning note and §18.1's deniability-ledger note (deleted --
each restated an author ruling three lines above), §18.3's adverse-result
note (rewritten), and §21's level-composition paragraph (rewritten to state
the rule without the two rejected variants). Zero remaining. Register
tombstones, protocol supersession and rejected-alternative rows left alone;
recorded as a convention in `authoring-conventions.md`.

**Raised, not acted on**: §16.3 still states a mitigation it then retracts
in the next paragraph ("peering edges carry a distinct, low default flow
capacity" / "Superseded by §16.2.1's landscape"). Under the new convention
the section would state the current position -- no edge-kind discount, and
what should price the peering/`PoP` asymmetry is open -- and drop the
retracted mitigation. Not done unilaterally because A20 cites §16.3 as the
record of what is now unpriced, so the edit moves a risk-register
dependency. Author's call.

**§16.3 restructured and A20 withdrawn (2026-09-04)**, on the author's
ruling: *"Peering edges are equivalent to PoP edges. There is an off-protocol
premium implied for the peer nodes themselves, but this is not part of the
view of other nodes. The claim, the statement that it is superceded, and A20
should all just go."* Removed: the low-default-capacity mitigation, the
supersession paragraph, and the open block quote asking what should price the
peering/`PoP` asymmetry -- the question dissolves, since the asymmetry is not
in any third party's view to be priced. Replaced with the two positive
statements the ruling makes. The dangling *"what remains is that a peer may
extend credit they did not intend"* sentence went with A20, being A20's own
text. Register 32 -> 31 entries, §20's count sentence corrected, withdrawal
recorded in `change-log.md` per §19.4's numbers-are-not-reused rule.

Also caught in the same pass: §16.2.1's bullet 2 cited *"§21.1's unset
ratio"*, which §21.1 no longer contains -- the peering ratio was dissolved
earlier the same day. The reference checker passes it because §21.1 is a
valid heading; only reading the target catches it. Rewritten to state that
there is no ratio between the two kinds, set or unset. `flow_metric.py`'s
capacity comment pointed at the same removed default and is corrected.

References 1,962 across the five specification documents, 0 flags.
Simulation passes.

**Change-log path citations and bytecode tracking (2026-09-05)**. Both were
flagged by the assistant during the §16.3 round; the first flag was wrong as
filed. The 2026-08-26 entry scoped the no-citation invariant to the five
design documents and deliberately exempted the log — *"being a historical
record of what those files were called when the entries were made"* — so
there was no breach, and CLAUDE.md's six-document framing is what produced
the misreading. Put to the author with the prior ruling quoted; he elected to
reverse the carve-out and take the rephrasing rather than the exemption.

Applied: twenty mentions rephrased to name the file without the path, the
directory itself named as `Robot` rather than as a path fragment. Every
changed line was read individually rather than trusted to the substitution.
The 2026-08-26 entry's own sentence — *"the change log's eighteen mentions
are left as written"* — is left standing as the record of what was decided
then, with today's entry recording the reversal.

`refcheck.py` extended: the no-path rule now runs over all six root
documents rather than the five, and is mutation-tested (one seeded citation
=> 1 flag, exit 1; restored => 0 flags, exit 0). Bytecode untracked and
`.gitignore` added.

Final state: 1,962 references across the five specification documents, 0
flags; Robot/ citations across all six, 0 flags.

**Consistency, coherence and de-linting pass (2026-09-05)**, author-directed,
over the six root documents and then the models.

Six defects, all found by sweeping rather than by re-reading what had just
changed — which is the point of the instruction. §16.4 announced "two limbs"
and added the governing pass in a later paragraph, so the bullets stated the
wrong rule; restructured into three passes in one list. §8.1.1's
selective-disclosure sweep said the metric reads edges "which come from
adoptions", stale since §16.2.1 put acquaintance edges in the same graph;
`wire-format.md` §4.5.2 inherited it. Two sections numbered §11.2.1 (all
seven citations meant the first; the second renumbered to §11.2.2). Two
"origin" usages left from before the landscape correction. A sentence
describing §16.2.1's first two bullets as being about scope, contradicting
bullet 1's "these carry trust". §22 opening with a duplicated sentence.

Counts re-verified against what they count: eleven unset parameters (11 live
rows plus 1 dissolved tombstone), eight horizon jobs (8 data rows),
thirty-one assumptions after A20's withdrawal. Register gaps P{6,7,8,9,10,22,
34}, C{3,12,13,14,16,18}, A{20} — every one resolves in `change-log.md`, so
the numbers-are-not-reused rule holds.

Models checked as a set, not assumed: 98 section references across 13 files,
all resolving; reason code 5 confirmed as cycle repair (`wire-format.md`
§4.3), the "current counterparty is never a candidate" rule confirmed at
`wire-format.md` §5.3, the patron/sibling/grandpatron ladder confirmed, f=10
and h=2 confirmed. Two stale claims in `flow_metric.py`'s commentary: the
horizon described as sitting at the origin (corrected -- unthrottled and
equidistant-from-origin are different statements, and only the first holds),
and the module docstring asserting nothing turned on telling PoP from
peering, which the visibility difference disproved -- now states the limit
and points at `reach`. Terminology aligned to the design's "pass".

De-lint: 3 blank-line runs collapsed. Remaining lint hits are 10, all
verified false positives -- 6 doubled words and 1 unbalanced bold that are
`change-log.md` quoting the artefacts it recorded fixing, and 3 table-column
counts that are escaped pipes inside formulas.

Final: references 1,963 across the five specification documents and Robot/
citations across all six, 0 flags. All eight models pass. Test vectors: ALL
CHECKS PASS. Seeds 1-25 pass on the simulation.

---

## Cross-family review 5 of the trust-metric simulation (2026-09-05)

Reviewed at SHA-256 `44dcaba7...e6b506`; hash matched the committed file.
Reviewer ran seeds 1, 2, 7, 42 and cross-validated `max_flow` against
exhaustive minimum cuts on 1,800 random directed graphs -- zero
discrepancies. That is the fourth independent validation of the max-flow core
(700/600/600/1,800 graphs, four reviewers, no discrepancies). Both findings
verified by execution before any change.

**F1 -- E2 omits the entry adoption from scope. Verified, High, holds.**
`attach_fake_region` has `boundary` adopt `FAKE` and puts the edge in the
capacity graph; `experiment_cut_bound` then built scope from the honest
`children` table, dropping it. With `boundary = nodes[1]` (a direct child),
restoring that one edge puts `FAKE` two scope edges from the observer --
inside the horizon -- and `score_independent` returns UNTHROTTLED instead of
10, at all three populations (reproduced exactly: 10/10/10 -> 1e9/1e9/1e9).
The comment defending the omission covers the fake region's INTERNAL
adoptions, which is fair, but not the entry edge, which this observer is
precisely one of the parties that holds.

Fixed as the reviewer proposed and then some: boundary moved to scope
distance 2 so `FAKE` lands at 3, entry adoption carried in scope, and an
assertion that `FAKE` is outside the horizon *before* measuring. Added the
inside-horizon placement as a positive second case asserting UNTHROTTLED --
the claim's edge stated as a measurement rather than avoided.

**Design consequence, applied**: §17.3's third leg said flow-limited trust
bounds "any single-entry region ... regardless of its size", which is
broader than §16.2.1 permits. Qualified to *where the entry lies beyond the
observer's horizon*, with the reason (participation, and disavowal as the
in-subnet remedy) and an explicit note that legs 1 and 2 carry no such
condition. This applies the author's existing landscape ruling to a sentence
that predates it rather than deciding anything new, but it NARROWS A
SECURITY CLAIM and is flagged as such.

**F2 -- `reach` and `landscape_distance` admit invisible peering edges.
Verified, High, holds. Introduced by this assistant earlier the same day.**
Two independent manifestations, both reproduced:

  (a) `visible_flow_subgraph`'s outward walk added every peering edge
      incident to a discovered node without re-applying §16.3.1's
      endpoint-in-horizon rule. Counterexample (4 nodes): O--H adoption,
      H--G peering visible, G--X peering invisible. reach=0 -> X standing 0;
      reach=1 -> X standing 8.
  (b) `landscape_distance` folded the caller's raw `peer_edges` into its
      outward adjacency, so an edge absent from the capacity graph could
      shorten a distance and raise a relay's node capacity. Reproduced at
      2 -> 4 (deepest relay moved from distance 4/cap 2 to distance 2/cap 8).

Fixed from the author's own words for what `reach` models -- *"discern some
of a foreign subtree's structure from LOCATOR DATA ... in that foreign
PATRONAGE graph"* -- so the walk follows hierarchical edges only and peering
enters by the endpoint rule alone; and `landscape_distance` now takes its
adjacency from the observer's graph, using `peer_edges` only for
observer-incident edges, which cannot be invisible to a party to them.

**Why the existing tests missed it**, which the reviewer diagnosed correctly:
E3's invisibility case uses one ISOLATED invisible edge, so no visible edge
ever pulls an endpoint into the frontier and the faulty expansion never
fires. Both leaks now have regression tests -- reach 0..3 on the composing
case, and the ghost-edge distance case.

**Mutation-tested**, four of five discriminating: restoring the peering
expansion fails; restoring the raw-`peer_edges` leak fails; dropping the
entry adoption from the inside-horizon case fails; moving that boundary back
outside fails. The one that does NOT discriminate is stated rather than
papered over -- dropping the entry adoption from the corrected OUTSIDE
placement changes nothing, since `FAKE` is beyond the horizon with or without
it. The inside case is what carries that finding.

Seeds 1-30 pass. All eight models green. References 1,965 across the five
specification documents, 0 flags; 112 across 13 model files, 0 flags.

**Audit of the TLA+ and Tamarin models against the current specification
(2026-09-05)**, author-directed. The earlier pass had checked that section
references RESOLVE; this one checks that each model says what the cited text
says. Seven models read in full against their sections.

**Five misattributed quotations**, all the same shape -- `wire-format.md`
text cited as the design. Caught by a checker that pairs each `<doc> Section
N ... "quote"` against the document the citation names, rather than against
the set: `recovery.spthy` quoted *"Key alone is not recovery"* and *"presence
alone is not recovery"* as design §9.1 (both are `wire-format.md` §4.1);
`CycleDetection.tla` quoted *"If field 1 is you..."* and *"No node acts on a
memo alone"* as design §15.2 (both `wire-format.md` §10.2);
`PartitionMerge.tla` quoted *"a replay of the same frames"* as design §15
(`wire-format.md` §10.1.3). The substance was right in every case -- §9.1
does require the counterparty's co-signature alongside the old key's -- so
these are citation errors, not invented claims. All corrected; 5 of 5
attributed quotations now resolve to the cited document.

**One misattributed claim** the quote-checker could not see:
`CycleDetection.tla` cited §3.1.1 for "a node may not hold two patrons in one
subnet". §3.1.1 is *subnet plurality across* subnets -- close to the opposite
concern. The rule is §3's "within a subnet the authority relation is a tree".
Corrected.

**One real coverage gap.** `PartitionMerge`'s `Adopt` required
`patron[c] = None`, so a node could only be adopted while patronless. §6.2 is
explicit that there is no transfer transaction and that moving between
patrons is *"adopt at the destination, depart the origin, IN EITHER ORDER"* --
so the model excluded the adopt-first order, which is precisely the one that
puts two adoptions for a single subject in flight across a partition.
Relaxed to `patron[c] # p` and re-run: 15,080 -> 21,032 distinct states,
`SelfTruth`, `NoInvention` and `Convergence` all still hold. **The
restriction was costing coverage, not hiding a defect**, and supersession by
ordinal does the work the design gives to chain order. README's state count
corrected.

**One undocumented scope limit.** `CurrencyEscalation` models three rungs of
a four-rung table and omits the light-client pre-delegation path, without
saying so. Both omissions are defensible -- they are escapes from the frozen
state rather than rungs of the issuing ladder -- but they make the liveness
claim strictly weaker than the design's, which the file now states.

**Two vacuous definitions removed.** `SingleLiveAnswer == TRUE` and
`RepairOnlyCuts == TRUE` were honestly commented and absent from both `.cfg`
files, so nothing false was being claimed -- but a definition equal to TRUE
reads as a property and would pass silently if a later hand added it to a
config. Replaced by comments saying why no operator exists.

**Design finding**: §12.6.5.1's *Honest limit* said "if the patron *and* its
siblings are all unreachable", omitting the grandpatron -- rung 3 of the
table directly above it -- while its own parenthetical says the escalation is
exhausted. The model had it right. Corrected, and its self-citation to
§12.6.5.1 from inside §12.6.5.1 replaced with "the escalation above".

**Cleanup**: four bare `0.8.6` review-round identifiers in model files
replaced with the substance they stood for. These files are meant to be read
line by line without background, and a round number is unreadable outside
this directory.

Verified against everything checkable: reason code 5 = cycle repair; the
memo's identity check; wire §5.3's distinctness rule; the
patron/sibling/grandpatron ladder and its "issue fresh, never extend stale"
rule; `current_key`; wire §3.2's witness distinctness; f=10 and h=2. All
eight models pass. References 1,964 across the five specification documents,
0 flags; 118 across 13 model files, 0 flags.

---

## Cross-family Tamarin review (2026-09-05)

Six findings, four High and two Medium. The reviewer had no Tamarin
installed and worked statically, so nothing was proved or falsified on their
side; every finding was reproduced or refuted with the prover here. Five hold
as filed, one is half right.

**TAM-01 currency has no current key. HOLDS, machine-confirmed.**
`!SubjKey` and `!Epoch` are persistent, so `Issue_Currency` can mint a fresh
attestation for any key ever held, at any epoch. Probe verified in 5 steps: a
subject holds k_old then k_new, an epoch starts after k_new exists, and
k_old is accepted as current on it with no compromise. The model's comment
claimed "a rotated-away key ... cannot be made to appear current", which is
not established. Worse than a gap: §12.6.5's author ruling separates expiry
from supersession and warns that conflating them reads as licence to serve a
binding known dead -- and the model has exactly that conflation.
**Not fixed; stated in the file as open work.** A first rebuild attempt
(linear `CurrentKey` consumed by `Rotate_Key`) closes the reviewer's probe --
it no longer finds a trace -- but the supersession lemma needs rotate-back
forbidden (wire §4.1: `prior_key` MUST differ) and proof hints before it is
tractable; a 4-minute run did not terminate. Landing half a rebuild is worse
than stating the limit.

**TAM-02 ceremony. Two halves; the first HOLDS and is serious, the second
does not.** `Meet` minted a `CoPresent` token for the WITNESS and
`Witness_Sign` consumed it, making the notary physically present -- against
§7.6's *"Witnesses notarise; they do not verify proximity ... the co-presence
evidence that matters is generated between the two devices and merely
reported to witnesses"*, which adds *"the record format must not imply
otherwise."* Fixed; all four lemmas verify without the token, so the theorem
is now about the PARTICIPANTS' co-presence.
The second half -- that the model cannot express a malicious-but-
uncompromised owner, and so overclaims against bilateral collusion -- **does
not hold**. Probe verified in 11 steps: a record accepted with NO meeting at
all, via `Compromise_Key`. In the symbolic model "the adversary holds A's
key" IS how a willing colluder is expressed; there is no other way to sign.
The real defect was the commentary, which framed that rule as theft only.
Reworded to say the carve-out covers §7.6's bilateral collusion as well as
§18.3's theft.

**TAM-03 attach cannot express sibling-vs-patron authority. HOLDS.** The
theory has `$C` and `$S` and nothing else -- no patron/sibling role, session
mode, countersignature or trust-bearing operation -- so it cannot reach the
failover case §12.6.5.1 turns on, where a client knowingly attaches to a
sibling that authenticates correctly as itself and must still be denied
patron authority. What it proves (endpoint authentication) is correct and
useful. The header claimed the broader property; narrowed, with the missing
vocabulary named.

**TAM-04 recovery omits the successor binding. HOLDS, machine-confirmed.**
`Recognise` signed `<'recognise', S>` -- the person, not the key -- so one
recognition could be assembled beside competing successors. Probe verified in
16 steps: two accepted recoveries for different new keys off one honest
recognition, no compromise. wire §4.1 describes this exact primitive and
forbids it. Fixed: the recognition now names the successor, and a new lemma
`recognition_binds_the_successor` checks it. Mutation-tested -- remove the
binding and the new lemma falsifies while the two original ones still verify,
which is why they never caught it.

**TAM-05 `no_replay` is weaker than its name. HOLDS, demonstrated.** Deleting
the `Eq` signature check from `Client_Finish`: `server_authentication`
falsifies (so it does real work), `no_replay` still verifies. It follows from
`St_Client_1` being linear plus `nc` fresh. Renamed
`client_commit_is_injective`; the project's real replay concern (wire §11's
0-RTT ban on Attach) recorded as out of scope.

**TAM-06 compromise carve-outs unbounded in time. HOLDS, fix was free.** Six
carve-outs of the form `Ex #k. Compromised(A) @ #k` with no ordering, so a
later compromise could discharge an earlier authentication. All six now carry
`& #k < #i`; all fourteen lemmas still verify.

Also fixed: `"neither alone is recovery."` was attributed in quotation marks
to design §9.1 and appears nowhere in the documents -- a paraphrase presented
as a quote, missed by yesterday's attribution sweep because that checker only
reported quotes found in the OTHER document. Replaced with wire §4.1's actual
words.

Lemma count 13 -> 14. All eight models pass. Model references 128, 0 flags.

**Supersession semantics, author-ruled (2026-09-05).** Arising from the
Tamarin review's TAM-01: asked which binding "superseded" names, the author
ruled *"The rotation should cause all nodes within the horizon to remove the
old key from their records and overwrite it with the new key. The rotation
should not be transmitted beyond the horizon, so any node outside of the
horizon should continue seeing old and new keys as separate entities."*

Checking it against the text found the ruling **already half-written and half
contradicted**. §9.0 distinguishes a PLAIN rotation (carries nothing; the
subject's privacy choice, §19.7 item 3) from a RECOVERY adoption, which
"publishes the link deliberately, `prior_key` plus verifier continuity
attestations" and whose "publication reaches the horizon and stops there" --
which IS the ruling. But §9.0.2 then said "**No message anywhere says
'rotation'**, and the inheritance linking the two is not carried", which is
false of the recovery case: `wire-format.md` §4.1 field 6 is a `Recovery`
block "present iff this is a recovery adoption", carrying `prior_key` and the
successor statement, inside a topology-class object forwarded byte-for-byte
(§10.1). A horizon member therefore receives the link in one signed object.
Had that sentence stood, the overwrite would have been unimplementable.

Applied: §9.0.2's blanket sentence split into the two cases; the overwrite
recorded with the author's words; §12.6.5's supersession paragraph given the
consequence -- inside the horizon the rule is enforced by REPLACEMENT rather
than by a check, so it bites on running sessions and queues, and outside there
is no supersession to know, expiry being the only bound. The two halves never
overlap.

Two inferences, flagged as such rather than ruled: (a) the fork case is
reconciled as one-current-key-per-observer with a CHOICE between competing
claims, not two current keys -- the author confirmed the premise ("a horizon
member does hold one current key per identity ... what has no single current
key is the identity globally") and this follows; (b) the overwrite applies to
RECOVERY adoptions, since a plain rotation carries nothing to overwrite with.
(b) is forced by §9.0's two cases rather than chosen.

**The currency model is now specified but not built.** The property to prove
is the relying party's, not the patron's: a party that received the recovery
adoption must not accept the old key thereafter, expiry notwithstanding. The
patron-side no-fresh-issuance property falls out of the same overwrite, the
patron being a horizon member itself. Awaiting the author's go, the earlier
"say the word" not having been answered with one.

**Follow-up, same day.** The author's note on the corrected sentence: *"Sometimes
I make these pronouncements contextually, I think we're talking about the
rootward memo process at the time."* Checked, and he was right in that context
-- so the correction above had over-reached by removing the claim entirely
rather than re-scoping it. `TopologyMemo` (`wire-format.md` §10.2) has exactly
five fields: patron, patron's position, slot, timestamp, and the key now
occupying that slot. **No prior-key field and no recovery discriminator**, so
rootward the inheritance is not carried and no message says "rotation" --
structurally, not by convention. Restored to §9.0.2 scoped to rootward travel,
with the mechanism given: past the horizon there is no object capable of
carrying the link, which is stronger than saying none is sent.

---

## Cross-family Tamarin review 2 (2026-09-05)

Nine findings, again filed statically without a prover; all checked here with
one. Seven hold, two restate limitations the files already declare. **Two of
the seven were introduced by this assistant in the previous round**, and the
process finding behind them matters more than either.

**A-2 free timepoint variable -- MINE, and the gate passed it.** Last round's
time-bounding of the compromise carve-outs was applied by regex; it put
`#k < #i` into `client_commit_is_injective`, which quantifies `#i1` and `#i2`.
`#i` is free. Tamarin reports this as a WELLFORMEDNESS WARNING, **exits 0, and
still prints "verified"** -- so `run-all.sh`, grepping only verified/falsified,
called it a pass. Fixed to `#i1`; five other carve-out sites checked
individually and correctly bound.

**`run-all.sh` now fails on wellformedness failures**, and the fix needed two
attempts, both instructive. The first pattern (`'wellformedness check'`) also
matched the SUCCESS line "All wellformedness checks were successful", failing
all four theories -- caught only by running it. Narrowed to
`'wellformedness check(s)? failed'` and tested BOTH ways: reintroduce the free
variable, exit 1; clean files, pass. The gate then immediately surfaced two
**pre-existing** unbound variables no review had reported: `cid` in
`Accept_Record` and `sk` in `Accept_Currency`. Both now bound from the
received record, which is also more faithful -- a relying party evaluates what
it was handed.

**R-1 old-key proof does not bind the patron. HOLDS, High,
machine-confirmed.** wire §4.1's `SuccessorStatement` is `[prior_key,
new_key, patron_key]` and a verifier MUST check the last two against the
adoption. The model signed `<'rotate', S, newkey>`. Probe verified in 13
steps: two different patrons accepting one proof, no compromise. Fixed;
`successor_statement_binds_the_patron` added and mutation-tested (removing the
third element falsifies only that lemma).
**How the fix went is worth recording.** The first attempt updated the `In`
pattern to carry `$P` but the `.replace` for the `Eq(verify(...))` line
matched nothing and I did not assert the count -- so the rule CARRIED the
patron and never CHECKED it, which is precisely wire §4.1's "an unchecked
binding is the same as no binding", reproduced by accident. Two lemmas
falsified; three rounds of speculation got nowhere and reading the actual
counterexample found it in one look. **Third instance this session of
mechanical substitution without verifying the applied count.**

**C-2 formation ceremonies absent. HOLDS, Medium.** `wire-format.md` §4.5
field 6: a formation record "has empty witness and verifier arrays
permanently". `Accept_Record` required a witness signature, so the co-presence
theorems said nothing about a structurally legal record. Added `Meet_Formation`,
`Formation_Sign`, `Accept_Formation`, plus `formation_is_executable` and
`formation_requires_copresence`. What a formation record is WORTH is left out
deliberately -- §13.2 makes it evidence only to its two participants, a weight
rule (§16.1), not a structural one.

**R-2 `key_alone_insufficient` did not exercise the thief. HOLDS, Medium.**
The antecedent required the honest `OldKeyProof` action, which a thief signing
with a stolen key never fires -- excluding the case the lemma is named for.
Dropping the conjunct makes it strictly stronger; still verifies.

**X-1 one name, several keys. HOLDS, Medium.** `Register` could fire twice for
one public label. A `OneKeyPerName` restriction is added to all four theories;
all lemmas still verify. `currency`'s `Subject_Key` is deliberately exempt --
a subject holding several keys over time is rotation, that theory's subject.

**K-2 expiry is imposed by a restriction. HOLDS, Medium, documented rather
than repaired.** A restriction discards traces; it does not show the protocol
prevents them. The currency theorem holds *relative to* a relying party that
enforces expiry, which is an environment axiom of the same kind as ceremony's
co-presence. Listed in the README's axioms section, where it belongs.

**C-1 and K-1 restate declared limitations.** The co-presence axiom and
currency's missing current-key state are both already stated in the files;
K-1 is the open work recorded last round.

**A-1** was narrowed last round already (attach proves endpoint
authentication, not authorisation); the reviewer's reading of the header
predates that edit.

17 lemmas, all verifying, all wellformedness clean. All eight models pass.
Model references 132, 0 flags.

---

## Currency current-key rebuild -- ATTEMPTED, NOT LANDED (2026-09-05)

Authorised by the author after a plan was agreed. **The rebuild is
structurally complete and does not converge; it is reverted.** The suite is
back to three currency lemmas with the gap stated, exactly as before, and the
attempt is preserved as `Robot/currency-rebuild-attempt.spthy` so the next go
does not start from scratch.

**What was built, and it is right.** Per-party `View(A, S, k)` -- A's own
record, never a global current key, which §16.1's per-observer rule and the
no-shared-state rule both forbid. `Receive_Adoption` consumes and replaces a
party's view; whether it fires for a given party is the adversary's choice,
which is simultaneously the right Dolev-Yao shape and the right RHTN one --
a party that never receives it keeps its old view, and that is what being
outside the horizon looks like without inventing a horizon predicate.
`Issue_Currency` reads the patron's own view, the patron being a horizon
member itself, which is how the author's ruling collapsed two properties
into one mechanism. Successors are drawn `Fr()`, per §9.1's "new keypair"
and `wire-format.md` §4.1's "prior_key MUST differ", which also makes the
successor relation acyclic.

**What would not prove.** `no_issuance_after_overwrite` and
`superseded_acceptance_predates_the_rotation`. Tried, in order: `use_induction`;
`[reuse]` helper invariants; a bounded instance (one rotation per subject,
which is how every model here works -- TLC runs three nodes); and a
reformulation splitting view-CHANGE from view-RESTORATION events. Whole-file
proof ran past 25 minutes; even a negative probe timed out at 300s.

**The cause is identified.** `Issue_Currency` consumes and restores the
patron's View without stamping it, so a backward search chains through
unboundedly many restorations to find where the view was set. Stamping each
restoration proves that lemma and breaks `view_never_returns` instead: the
two want different event vocabularies. Reconciling them is the open work.

**Two traps caught, worth recording because both would have shipped a false
result.** `--prove=<name>` ASSUMES prior `[reuse]` lemmas rather than proving
them, so `no_issuance_after_overwrite` "verified in 3 steps" while the helper
it rests on timed out, and `view_never_returns` "verified in 26 steps" resting
on a helper that never proved either. Neither was a result. **Always run the
whole file.**

**The other half was never this model's to prove.** "A party holding
authenticated supersession evidence MUST NOT continue serving" is a client
obligation, self-enforced, and already lives at
`infra-client-requirements.md`: *"Stop serving a binding you have verified
superseded"*. §1.1's test applies -- a rule aimed at a party you share no
state with is a wish -- and a symbolic model can assume such a rule or ignore
it, never prove it. That is not a gap in the model; it is the correct
division.

**Kept from the attempt**: `run-all.sh` now bounds every Tamarin call
(`TAMARIN_TIMEOUT`, default 600s) and fails on a timeout. A non-converging
theory would otherwise hang the gate forever, which is exactly what happened
here.

**And then the author named it, which closed it without any of that
(2026-09-05).** Asked what the gap was, his answer: *"It is unenforceable
client behavior."* Checked, and he is right on both halves, which is why no
amount of Tamarin was going to help:

  * the ISSUER'S half is unenforceable because a `CurrencyAttestation` names
    `{identity, current_key, issued_at, expires_at, signature}` and carries
    nothing distinguishing an issuance made before a rotation from one made
    after;
  * the RELYING PARTY'S half is unenforceable for the ordinary reason -- it
    governs what a party does with its own records.

**And the two never meet**, which is the part that settles it. A party able to
detect a stale issuance is one holding the recovery adoption -- and that party
has already overwritten its own record (§9.0.2), so it rejects on that and
never reads the staple. A party that would read the staple is outside the
horizon, where old and new keys are separate entities by design and there is
nothing to detect. There is no third-party-checkable property, so there was
never a lemma.

**The one real gap was in a requirements document.** The relying party's
obligation was written (`infra-client-requirements.md` §2, "stop serving a
binding you have verified superseded"); the issuer's was not. Added to §3
alongside "issue fresh, never extend stale", stated as a commitment with the
reason it cannot be checked. That is the whole repair.

Actions: `currency.spthy`'s "open work" note replaced with why the property is
not the protocol's to have; `models/README.md`'s "Open" section replaced with
the same; `Robot/currency-rebuild-attempt.spthy` DELETED -- preserving it
implied a next attempt, and there should not be one. The reference above is
left as the record of what was tried.

**The lesson is one CLAUDE.md already records and this assistant did not
apply**: *when a finding assumes a component, ask whether the component is
required* -- not whether it can be built. Three reviewers filed this as a
model gap and I accepted the framing, planned a rebuild, got authorisation,
and spent it on machinery for a property the protocol never had. The question
"is this enforceable at all?" costs one message and would have closed it
before the first line of Tamarin.

---

## Reverse alignment: the specification against the corrected models (2026-09-05)

Author-directed, and the opposite direction to the audit above. That one asked
whether the models say what the design says; this asks whether the design
carries what the models established, and whether it claims anything they
contradict. Four findings, two of them substantive.

**1. Peering visibility does not compose -- NOT STATED, and security-relevant.**
§16.3 says a peering record is visible "inside the two peers' horizons and
nowhere else", which IMPLIES non-composition. §16.2.1's reach paragraph says
reach grows with "what relationships you are aware of" and does not exclude
peering records. That is the exact ambiguity that produced the defect in
`flow_metric.py`: reading reach as walking every edge kind, which a
cross-family review then broke in four nodes (standing 0 -> 8). The model was
corrected on the author's own words -- locator data exposes PATRONAGE
structure -- but the words were in a review answer, not in the document, so a
second implementer would make the same choice. §16.2.1 now states it, with the
four-node instance as a worked example rather than as review history.

**2. A trustworthy clock was an unregistered load-bearing assumption.**
Surfaced by the currency model's declared axioms. §12.6.5's fail-closed table
turns on whether a staple has EXPIRED, which is a local comparison made for a
SECURITY decision. Registered as **A33**, with the asymmetry named: a fast
clock costs availability, a slow one accepts dead credentials, and the
dangerous direction is the one an attacker prefers. §12.6.5 gains the same
note. Register 31 -> 32; BOTH count sentences updated.

**The first draft of this got its justification wrong** and the author caught
it. I framed expiry as an EXCEPTION to `wire-format.md` §3.3, reading "not
checked against a local clock" and "not against anyone's clock" as a
prohibition on local timing. It is not: *"There is no globally enforced
sequencing of events ... there is no limitation on using internal timing
within a node"* [author, 2026-09-05]. §3.3 withholds global sequencing and
reader-dependent validity, and §3.3's OWN closing paragraph relies on a node's
clock -- "a witness declining to attest a ceremony dated far from its own
clock". So there was never a tension, and A33 does not rest on one. The
assumption stands on its own terms: what singles this use out is not that it
consults a clock but that a security decision turns on the answer. Corrected
in A33, in §12.6.5's note, and in `models/README.md`.

**3. §20.2's count was stale, and this one is mine.** It read "The thirty-two
below" after A20's withdrawal took the register to 31. I missed it when
withdrawing A20 because I grepped for digits and the count is spelled out.
Corrected -- and then correctly returned to thirty-two by A33, which is why
both count sentences were re-checked rather than assumed.

**4. The fork discussion did not reflect the successor binding.** §9.0.2 says
two competing recovery claims become two adoptions, without saying that each
needs its own old-key statement naming that successor and that patron
(`wire-format.md` §4.1) -- the property the recovery model now proves as
`successor_statement_binds_the_patron`. Without it a reader could take one
leaked proof to spawn unlimited heirs, which is precisely what §4.1 forbids.
Stated: a fork costs the old key a second signature.

**Checked and clean**: no root document claims anything has been formally
verified (the "formally verified" hits are external prior art, §14's MLS/PQXDH
citations). All three model axioms now have a home in the design --
co-presence at §7.6, face recognition at §9.1/§18.3, the clock at A33.

**One self-correction during the pass.** The first draft of the §16.2.1
addition cited "the reference simulation" and "a cross-family review" -- review
history in a root document, against the convention the author gave on this
same day and which I had recorded in `authoring-conventions.md`. Rewritten to
carry the worked instance without the provenance.

References 1,983 across the five specification documents, 0 flags. All eight
models pass. Test vectors pass.

---

## Cross-family Tamarin review 3 (2026-09-05)

Six findings, static-only again; all four file hashes matched the committed
versions. Three are real model defects, two are claim/prose mismatches, one is
a lemma-strength point. All checked with the prover.

**T-01 recovery collapses past acquaintance into present recognition. HOLDS,
High, machine-confirmed, and the sharpest finding of the three rounds.**
design §9.1 step 1 asks the subject to *"meet, IN PERSON, someone they have
met BEFORE"* -- two facts. The model had only the past one: a persistent
`!Met(V,S)` that `Recognise` read forever. Probe verified in 9 steps: a thief
holding the stolen key completes a recovery with **verifier and patron both
honest and no meeting anywhere**. The file called that §18.3's
colluding/deceived residual; it is not, since nobody in the trace colludes or
is deceived -- the honest rule simply fires. Fixed with `Recovery_Meeting`
minting a linear `AtMeeting` token that `Recognise` consumes, a second
physical axiom of the same kind as ceremony's co-presence, plus
`recovery_requires_a_meeting` stating what it buys. Mutation-tested: revert to
history-only and ONLY the new lemma falsifies, which is exactly why the four
existing ones never caught it. Deliberately not claimed: WHO turned up. A
thief taken for the subject satisfies the token too; that residual stays
§18.3's and is stated in the file.

**T-03 the witness held a truth oracle. HOLDS, High.** `Witness_Sign` was
premised on `!Ceremony`, which only the physical `Meet` creates, so an honest
witness could notarise only a genuine meeting -- against §7.6, which says
colluding parties can simulate the whole exchange and NO WITNESS CAN TELL. The
witness now takes its roster from the network. **Removing the oracle revealed
the model had been leaning on it for something else**: `DistinctParties` was
enforced at `Meet` only and reached acceptance through that premise, so with
it gone the degenerate roster P1 = P2 = W became assemblable from a single
signature satisfying all three checks. A validator checks role distinctness
itself (`wire-format.md` §3.2), so `Accept_Record` now does. All six lemmas
verify and the theorem rests on the PARTICIPANTS' tokens alone.

**T-06 `recognition_binds_the_successor`'s escapes were loose. HOLDS,
Low-Medium.** Compromise disjuncts untimed, and the verifier disjunct named
ANY verifier rather than the relied-on one. Tied to `ReliedOn(P,V,S)` and
time-bounded, matching `successor_statement_binds_the_patron` beside it.

**T-02 and T-05 are claim mismatches, and each file contradicted itself.**
`currency`'s introduction said that once the patron rotates and the lifetime
passes, no attestation makes the old key current -- while the note above its
security lemma, added last round, says there is no rotation state. `attach`'s
said it WAS the Stage 1.1 sibling-authority target, while the paragraph
directly above explains it cannot reach authorization. Both introductions
rewritten to state what is proved and name what is not.

**T-04 holds and is already disclosed.** Expiry is an event plus a
restriction; "beyond the attestation lifetime" has no meaning in the model.
The reviewer's framing is better than what was there and is adopted into the
currency introduction: what is proved is "an epoch DECLARED expired is not
subsequently accepted", never "a lifetime elapsed".

**T-02's second argument was engaged, not dismissed.** The reviewer allows
that "issue only for the current key" is unenforceable externally but argues
Tamarin routinely models honest-party local state anyway. True in general;
here the rebuild was attempted on 2026-09-05, does not converge, and the
author has ruled the property is client behaviour on both sides. The
disclosure is the answer, and the introduction now carries it rather than
contradicting it.

18 lemmas, all verifying, all wellformedness clean. All eight models pass.
Model references 141, 0 flags.

**Checked on the author's challenge (2026-09-05): "Did I decide that meeting
was not required for recovery?"** No -- searched `review-tracking.md` and
`change-log.md` for any ruling to that effect and the only hits are today's own
entries. The requirement is in both documents, and the ENCODING states it more
sharply than the design does: `wire-format.md` §4.1's Recovery field 2 --
*"The PRESENCE half: a prior counterparty who **met the subject again** and
recognised them."* "Again" is the fresh meeting, on the wire. §9.1 step 1 says
the same in prose. So the model was weaker than both documents and the T-01
fix aligns it rather than adding a requirement; `recovery.spthy` now cites
field 2, which is the better citation.

**Process note against myself.** The T-01 change was made on the reviewer's
reading plus §9.1, without first checking the record for a contrary ruling or
pulling the encoding's wording -- the strongest evidence, and found only
because the author asked. The answer came out the same, which is luck rather
than method: CLAUDE.md's rule is verify before applying, and "verify" includes
checking whether the author has already ruled the other way.

**Parallel-edge collapse, author-ruled (2026-09-05).** The open question from
the layered pass -- peering now requires a PoP, so both edges join one pair --
is answered and generalised: *"Parallel edges need to collapse because
otherwise you would add multiple edges with successive PoPs between the same
parties, which is probably exploitable. Every patron/sub is also a PoPmate,
etc."*

Scope is wider than the question asked. §6.1.1's requirement makes EVERY
adopted pair a PoP pair, so under summing every hierarchical edge would have
doubled -- a systematic distortion introduced by layer 1 and caught by the
author, not by the layered consistency passes, which checked prose consistency
and not the arithmetic the prose implies.

Applied in order. §16.2.1 states the rule with the farming reason. §16.3's "a
peering edge raises a region's cut" is qualified: it raises it only for an
observer holding no other edge for that pair, which after §6.3's meeting
requirement means an observer who sees the peering record and not the presence
record -- per-observer, like everything else. §6.3's "contributes to trust"
bullet now says what it contributes is the meeting it required. Wire and the
vectors are unaffected: they encode transactions, and multiplicity is a
property of the graph an evaluator builds.

`FlowGraph.add_edge` summed, with the comment "so parallel logical edges
combine"; it now keeps the larger. Measured before the fix: an adoption plus
its required meeting gave 20 where the rule says 10. No experiment's numbers
changed, because every fixture pair is joined once -- which is exactly why a
regression case was needed rather than assumed. It asserts a pair joined by an
adoption and two later meetings carries one edge's capacity; restoring the sum
fails it.

Where two constructions offer different capacities for one pair the larger is
kept. Recorded rather than relied on: §16.2.1 makes an edge worth what an edge
at its distance is worth whatever produced it, so they are equal in every
construction here and the choice decides nothing today.

All eight models pass, 19 lemmas. References 2,029 across the specification
documents and 160 across the model files, 0 flags. Vectors: ALL CHECKS PASS.

---

## Second full consistency pass across all four layers (2026-09-05)

Author-directed after the parallel-edge ruling. **Two real defects, both
introduced by this assistant during the layered pass, both invisible to the
first round of consistency checks** -- because that round compared prose to
prose, and these are places where prose and bytes disagree.

**1. §6.1.1 made every recovery adoption malformed.** The rewritten rule said
an adoption carries field 8 or field 9 and "carrying neither is malformed". A
recovery adoption carries NEITHER: its presence half is embedded in the
`Recovery` block, whose field 2 is "a prior counterparty who met the subject
again". The old §6.1.1 had the carve-out -- *"Recovery adoptions have their own
evidence requirement (§9.1)"* -- and the rewrite dropped it. Propagated to wire
§4.1 in layer 2 and to negative vector T25 in layer 3, so all three layers
agreed on a rule that outlaws a mechanism the design requires.
**Fixed in layer order**: three evidence forms, exactly one present -- field 8,
field 9, or field 6. Wire annotates field 6 as evidence and forbids doubling.
T25 qualified to non-recovery adoptions, T26 widened to "more than one of 6, 8,
9", and D21 added: the recovery adoption is a MUST-ACCEPT carrying neither 8
nor 9.

**2. The corpus referenced records it did not contain.** The alice-bob
formation and bob-carol normal records were built in the generator and their
txids used as field 8, but never emitted as vectors. `README.md` claimed "the
fixtures build the records they reference", which was true of the generator and
false of the published corpus. Both are now published sections, and both
envelopes joined the `P-*` sweep so the Rust runner verifies their signatures
independently: 147 -> 149 entries, 15 envelopes.

**What found them, and what would not have.** Neither turned up in the
reference checker, the linter, the count checker, or any prose sweep. Both
turned up in **two new checks in `verify.py` that test a design rule against
the actual bytes**:

  * *every adoption carries exactly one evidence form* -- swept over every
    adoption section, not asserted on one vector. Found the extension-keys
    adoption carrying no evidence at all on its first run, and the recovery
    conflict followed from thinking about what the sweep should accept.
  * *every field-8 reference resolves to a record naming both parties* --
    design §6.1.1 and §8.1.1 both state that check. It failed 0/4 immediately,
    which is how the unpublished-records gap surfaced.

Adding the first check also exposed that two pre-existing checks located "the
formation record" **positionally** -- `## Presence record.*?` matching whatever
came first. With three presence records in the corpus they silently recomputed
the alice-carol root against the alice-bob body. Both now name their section.

**The lesson for the next pass**: a consistency pass that only reads is a
consistency pass that only finds what reading finds. Every rule stated in the
design that a lower layer can be made to check against its own artefacts should
be, and the check belongs in the harness rather than in a session.

Final: references 2,032 across the five specification documents, 0 flags; 160
across the model files, 0 flags; 7 counted claims all matching; all eight
models (19 lemmas); vectors ALL CHECKS PASS; Rust runner 149 pass, 0 fail.

---

## Cross-family Tamarin review 4 (2026-09-06)

Seven findings, static again; all four hashes matched. Five hold, two are
claim-width points that hold as stated. **F1 is the most serious defect any
round has produced, and it had survived three previous reviews and every run
of the gate.**

**F1 currency could not execute its honest path. HOLDS, HIGH,
machine-confirmed both ways.** The attested field was the subject's fresh
SECRET `~sk`, and `Issue_Currency` emitted only the signature -- so nothing
ever output `~sk` and no relying party could assemble the tuple
`Accept_Currency` demands. `wire-format.md` §7.1 carries `current_key` as a
public KEYHASH; the model had made a public field secret.
**What hid it was that the lemma passed.** `currency_is_usable` verified in 7
steps, and dumping the trace shows `case Compromise_Patron`: the adversary
stealing the patron key and forging an attestation. So the anti-vacuity guard
was satisfied by a compromised trace, and `currency_requires_unexpired_issuance`
was satisfied by its own compromise disjunct. Three green lemmas, none about
the honest protocol.
Fixed: issuance signs and emits the whole staple over `pk(sk)`. Added
`currency_is_usable_honestly` -- an acceptance with NO compromise anywhere --
which verifies now (10 steps) and, against the old modelling, is **falsified,
no trace found**. That is the guard the file never had.

**F3 `key_alone_insufficient` excluded the stolen key it names. HOLDS, HIGH,
by quantifier scope alone.** `not(Ex V #kv. Compromised(V) @ #kv)` with V
unbound reads "no identity anywhere was compromised"; instantiate V = S and the
thief is excluded. The lemma established only that recovery needs a recognition
on traces where nothing was stolen. Rewritten to PERMIT `Compromised(S)` and
exclude only the patron and the relied-on verifier. The stronger form still
verifies -- the property held, it was not being tested.
`recovery_requires_a_meeting` had the same loose `Ex V. Compromised(V)`
carve-out and is now tied to `ReliedOn(P,V,S)`. **Both lemmas are mine**, and
the second I had already tied to `ReliedOn` in `recognition_binds_the_successor`
one round earlier without checking its neighbours.

**F5 `Accept_Formation` omitted participant distinctness. HOLDS, MEDIUM.**
`Accept_Record` checks it; the formation path did not, and enforcing it at
`Meet_Formation` is not enough because acceptance takes body and signatures
from the network. Added.

**F2, F4, F6, F7 are claim-width and scope, all held, all narrowed where they
were overclaimed.** `attach` called its missing direction "symmetric" when what
is absent is `wire-format.md` §9.1's binding of the Attach-claimed identity to
the transport-authenticated one -- an application-to-channel binding, not a
direction, and the theory has one client variable so it cannot state it.
`recovery` quoted a Stage 1.1 target whose "outweighs" half is comparative,
belongs to per-observer weight (§16.1) and the flow metric, and is not this
theory's. `ceremony`'s `no_remote_forgery` read as a claim against any
adversary when its antecedent means CONFORMING participants -- §7.6's bilateral
collusion appears only as the compromise carve-out. `Met` is now labelled the
environmental assumption it is: no premises, so the theory is conditional on
being handed a truthful prior-counterparty relation.

**The lesson, and it is not the reviewer's finding but what it exposes about
the gate.** `run-all.sh` checks that lemmas verify and that wellformedness
passes. Neither catches a theory whose honest path is unreachable, because
every lemma still goes green. An exists-trace guard is only a guard if it
excludes compromise, and three of the four theories' executability lemmas do
not. Widening those is the obvious next hardening and is not done here.

20 lemmas, all verifying, wellformedness clean. All eight models pass.
References 2,032 across the specification documents and 160 across the model
files, 0 flags.

---

## Cross-family Tamarin review 5 (2026-09-06)

Four findings, static again. All four hold. **TAM-01 is the defect the previous
round predicted and did not act on**, in a different theory.

**TAM-01 no honest ceremony could reach acceptance. HOLDS, HIGH,
machine-confirmed.** `Meet` minted a fresh `~cid` and never published it, while
`Witness_Sign` and `Accept_Record` take the record body from the NETWORK -- both
changes this assistant made two rounds earlier, to remove the witness's truth
oracle and to bind an unbound `cid`. Neither change published the id, so the
network could not construct a body containing it and the honest path died.
Probe: an honest completion with no compromise is **falsified, no trace found**,
for the normal path (5 steps) and the formation path (4 steps). All six lemmas
verified anyway, through traces where participant keys were stolen and the
adversary chose its own id.
Fixed: `Meet` and `Meet_Formation` publish the id, which is faithful -- a
presence record is published, witnesses are handed it and evaluators fetch it
(§15's attestation class), and nothing treats the id as secret. Added
`honest_ceremony_completes` and `honest_formation_completes`.

**And the class is now closed rather than the instance.** All four theories
carry an honest-path guard excluding compromise. `attach` and `recovery` turn
out to be honestly reachable, but nothing had been checking, and that is the
same gap that let currency's defect survive three rounds and ceremony's survive
two. **This is the hardening the previous entry named as obvious and left
undone**, which is why the review found it.

**TAM-04 the witness carve-out was unnecessary and wrong. HOLDS, MEDIUM.**
`presence_requires_copresence` discharged on `Compromised(W)`, but design §7.6
gives a witness `protocol_ran` and `both_responsive` and explicitly NOT a claim
that two humans shared a room. Removing the disjunct: verifies unchanged, in
the same 10 steps. Carrying it would have hidden a future regression in which
one compromised witness manufactured presence.

**TAM-03 "issue fresh, never extend stale" was asserted, not enforced. HOLDS,
MEDIUM.** `!Epoch` is persistent and `Expire_Epoch` does not consume it, so a
patron could keep stamping an epoch already expired -- which is what extending
stale state looks like -- while the file's comment claimed every issuance draws
a current epoch. The acceptance restriction does not cover it: that blocks
accepting an expired attestation, not minting one. Added
`restriction IssueFreshNotStale`.

**TAM-02 is a SPECIFICATION question and is NOT resolved here.** Put to the
author rather than guessed: `Recognise` takes the successor key from `In(newkey)`,
so the model lets a network adversary choose which key an honest verifier's
genuine recognition attaches to, with neither verifier nor patron compromised.
Whether that is an over-approximation or a real gap depends on whether the
design binds `new_key` to the device physically present at the recovery
meeting. §9.1 says the new key countersigns the query and that "the counterparty
is its own querier", which reads as a binding -- and in the same paragraph says
continuity "rests on the response's prior-key binding and the old key's proof,
**never on the consent**", which reads as explicitly NOT relying on it. Those
two readings give different models and different security claims.

24 lemmas, all verifying, wellformedness clean. All eight models pass. Model
references 171, 0 flags; specification references 2,032, 0 flags.

**TAM-02 resolved from the documents, not by ruling (2026-09-06).** The author
asked where the "network adversary" entered and what "bound to the device in
the room" meant -- fair, since the finding had been relayed in the reviewer's
framing rather than grounded in the protocol. Run down:

The adversary entered **only in the model**. `Recognise` took the successor
from `In(newkey)`, a Dolev-Yao network input, so the term was whatever the
adversary supplied. In the protocol there is no such step: design §7.1's
channel 3 is optical, *"QR codes exchanged screen-to-camera"*, and it
**"carries key exchange"**; §9.1 makes a recovery meeting a ceremony. The
verifier's client reads the successor off the screen in front of it.

So substituting the key requires a device in the room, which is §7.6's
co-presence residual -- already accepted and priced -- and not a network
attack. **No specification change is needed and none was made.** The model
over-approximated, and is corrected: `Recovery_Meeting` mints the successor and
`AtMeeting` carries it, with `recognition_names_the_meeting_key` checking that
a recognition names the key the meeting exchanged. Mutation-tested: restore
`In(newkey)` and only that lemma falsifies.

**The honest-path guard added earlier this session paid for itself
immediately.** The first version of this fix left `Old_Key_Proof` minting its
own `Fr(~newkey)`, so the two factors could never name one successor and no
honest recovery could complete. `recovery_completes_honestly` reported
*falsified, no trace found* on the first run. Under the old guards -- which
permit compromise -- every other lemma still verified and the break would have
shipped.

Worth recording as the general shape: this is the third time a change made the
honest path unreachable while the universal lemmas stayed green, and the first
time it was caught in the same minute it was introduced.

25 lemmas, all verifying. All eight models pass.

---

## Cross-family Tamarin review 6 (2026-09-06)

Static-only again: the reviewer had no prover and said so, and declined to
count the models' own comments about past Tamarin runs as reproduced evidence.
Seven findings. **The character of the round changed.** Five of the seven are
the reviewer confirming a limitation the file already declares, each closing
with some form of *"verdict on defense: holds"*; one was already retracted in
the header it cites; one is new and small. No finding proposed a change to the
design, and the reviewer states plainly that they *"would not reject any
project design rule on the basis of these models."*

That is the first round to find no defect in behaviour. Recording it as a
signal about the state of the suite rather than as a list of dispositions.

**F1, F2 (attach: no client authentication, no 0-RTT replay state), F3
(ceremony co-presence is an axiom), F5 (expiry holds by restriction), F6
(`Met` is assumed, not derived).** Confirmed as disclosed; no change. Each is
scoped in the file and again in `models/README.md`'s assumptions section. F3 is
worth restating because it is the one people will most want to over-read: the
ceremony theory shows that **cryptographic assembly cannot detach an accepted
record from an already-assumed co-presence event**. It is not evidence of relay
resistance, and §7.5 already says the physical signals are participant
assurance rather than remotely verifiable proof.

**F4 (currency has no current-key state).** The header retraction the reviewer
asks for is already there. What is live is their U1, put to the author below.

**F7 (`RecoveryAccepted` overreads).** *Applied.* The finding is right and it is
the naming class that has cost time three times this session. A recovery
adoption is a two-signer transaction (§6.1); the theory models a patron's
evidence test and nothing else -- no successor envelope signature, no Adoption
body, no signer set. Renamed to `PatronAcceptedRecoveryEvidence` /
`PatronAcceptedTransferEvidence` (8 + 2 uses, count asserted before replacing
-- the first assertion failed on a miscount of 9, which is the point of it),
with a scope note at the emission site saying what a trace reaching it does not
mean. The patron sits in the first argument because there is no
observer-independent "accepted" to name -- the no-shared-state rule again.

**Hygiene, all three confirmed and applied.** The recovery comment was worse
than filed: it did not merely go stale, it **asserted the opposite of the rule
twenty lines below it**, telling a reader the recognition signs the subject and
not the successor -- the exact binding `recognition_binds_the_successor`
proves. It also carried drafting history, which does not belong in the corpus.
Rewritten to hold both halves together: the judgement is about a face, the
signature is about which key that judgement licenses. Currency's "the CURRENT
epoch" overstated a model with several coexisting unexpired epochs; corrected.
Attach called the exchange mutual authentication before qualifying it 17 lines
later; the qualification moved up to the first mention.

**Found while checking the reviewer's F3/F5 point about how results are
reported: three stale lemma counts in `models/README.md`** -- attach and
currency each said "Three lemmas" against four, ceremony "Four" against eight.
The counts drifted when the honest-path witnesses were added and the catalogue
was not swept. Corrected, with each guard now named at its count so the number
and its reason travel together. The review-history entries keep their
as-of-filing counts.

25 lemmas, all verifying after the rename. All eight models pass.

---

## The two Tamarin trees (2026-09-06)

Author's ruling, in answer to whether the suite verifies wire-enforceable
properties or the honest client's own commitments: *"Duplicate the models and
we'll verify both versions, as they prove different things, both of which are
of interest. Create distinctly named folders for the compliant and wire only
versions."*

`models/tamarin/wire-only/` holds the four existing theories unchanged.
`models/tamarin/compliant/` holds four new ones. **They are not duplicates.**
Wire-only proves the cryptography; compliant proves the state machine and takes
the cryptography as given. Nothing is stated twice, so the two cannot drift
apart -- which duplication would have guaranteed within a fortnight.

**Each compliant theory models an obligation the design says is unenforceable,
and the design's own words are why the theory exists.** The four were chosen by
reading the two client-requirements documents for commitments, not by pattern:

| theory | the written obligation |
|---|---|
| `currency` | infra-client: *"Issue only for the key you currently record ... nobody can check this for you"* |
| `attach` | design 12.6.5 supersession; light-client: trust-bearing stops on a sibling |
| `ceremony` | light-client: *"Refuse to sign a record attributing to you a witness you did not nominate ... the wire cannot check this -- you can, and you are the only party"* |
| `recovery` | light-client: *"On suspected key compromise, seal before you take a series reissue"* |

**Every compliant theory carries a lemma asserting the non-compliant trace is
STILL REACHABLE.** Without that, the tree would drift into proving that the
wire enforces what the design says it cannot, and a green result would mean the
opposite of what it appeared to. Four such witnesses: the stale sibling issuing
for a rotation that has not reached it, misattribution succeeding when the
nomination check is skipped, the thief winning against a chainless
counterparty, and a second node serving under a credential the first superseded.

**WHAT THE SPLIT FOUND.** `compliant/currency.spthy`'s obligation is FALSE read
alone. Both the initial key and every issued one reach the adversary, so it can
hand an issuer back a key that issuer already superseded; the issuer then
issues for it in full compliance, because it is once again the key it records.
What forbids the walk-back is in another document, binding another party:
light-client's *"never take a series reissue into a series you have occupied
before"*, which a counterparty holding the chain can enforce. The infra node's
rule is coherent only in company. No wire-only theory could have found it -- an
attestation issued after the walk-back is well formed and no relying party can
see the history. The dependency is now a labelled restriction in the file with
the mutation test recorded beside it.

**THREE OBLIGATIONS ARE STATED AND NOT DISCHARGED**, and they are the central
ones: currency's `no_issuance_for_a_key_this_issuer_superseded`, and attach's
`nothing_served_under_a_credential_this_node_superseded` and
`nothing_delivered_after_supersession`. They sit in their files commented out
under a block giving the claim, the cause, and what was tried. **None was
falsified** -- there is no counterexample, only no proof.

One cause, in both theories: the issuing and serving rules consume a linear
capability and restore it, which is faithful -- issuing does not change what a
node records, serving does not end a binding -- so the backward search for that
fact's origin regresses through unboundedly many prior operations. Source
invariants (`a_key_enters_a_record_once`, `a_record_key_entered_by_one_of_two_
doors`, `service_follows_an_attach`) closed the regress for the neighbouring
properties in both theories and did NOT close it for these three. Tried and
rejected: `use_induction` on each goal, the negated form, removing the `Out`
facts, removing each restriction, removing the non-compliant rule, and a
420-second budget. A longer wall clock is not the answer -- `run-all.sh` is
right that a proof needing more time needs a hint, and an unbounded regress
does not terminate at any budget. What would close it is a different encoding
of a revocable capability's lifetime, which is a modelling decision.

**Two modelling errors caught by lemmas rather than by reading**, both the same
mistake and both worth recording because the second was made after fixing the
first. In `attach`, minting `Fr(~k)` per session meant two nodes could never
serve the SAME credential, which made the ignorance witness unstatable. In
`currency`, minting a fresh key per issuer meant two issuers could never record
the same key -- and `stale_sibling_issuance_is_reachable` FALSIFIED, which is
how it was found. The key belongs to the subject in both cases. A per-issuer
key would have made the model prove a stronger property than the design claims,
silently, with every other lemma green.

Also: `attach`'s first shape gave each session its own fact, and all three
supersession lemmas falsified because supersession ended one session while
another kept serving. That is not a design defect -- design 12.6.5 says
"SESSIONS ... terminate", plural, and the duty is to the binding. The serving
capability is now one fact per (node, client, credential).

12 models, 43 lemmas verifying: 25 wire-only, 18 compliant. 3 obligations open.

---

## The three open obligations, bounded (2026-09-06)

Author asked whether the undischarged lemmas were an open issue with the model
or whether the models reach their target confidence. Both halves matter, so
both were established rather than asserted.

**It is the model, not the design.** No counterexample was ever produced --
Tamarin did not falsify, it failed to terminate. And the cause is local to the
encoding: the issuing and serving rules consume a linear capability and restore
it, so the backward search for that fact's origin regresses through unboundedly
many prior operations.

**But "believed true" was too weak to leave standing**, so the claim was
converted into a checked result. Bounding the loop -- at most one supersession,
at most two issuances or services -- makes the space finite, and all three
obligations VERIFY: 33 steps for currency's, 84 and 46 for attach's. Two things
this is not: it is not the unbounded claim, and it is not `--bound`, which
merely truncates the search and reports *analysis incomplete* (tried first, at
depths 6 and 10; no verdict either way).

**The bound is applied by appending a fragment to the real theory at gate
time**, not by keeping a second copy. Tamarin has no `#include`, so a bounded
companion would otherwise be a hand-maintained duplicate of the rules -- the
exact drift this session split the two trees to avoid. `run-all.sh` strips the
theory's final `end`, appends `<theory>.bounded`, and proves that. The rules
have one source and the bounded result follows any change to them.

Keeping the bound OUT of `attach.spthy` also preserves
`no_trust_bearing_operation_on_a_sibling` as an unbounded result. Carrying the
restrictions in the file itself would have silently demoted a target property
that currently holds in full -- the same class of quiet weakening as the
per-issuer key.

12 models, 43 lemmas unbounded + 13 bounded. Nothing open without a number
against it.

---

## Closing the three obligations: the right tool (2026-09-06)

Author: *"Go ahead and attempt."*

**The diagnosis came first, and it is the part worth keeping.** Rather than
reshape the theories on a hunch, the pattern was reduced to a three-rule
theory carrying nothing else:

```
rule Set:  [ Fr(~k) ]              --[ Set($I, ~k) ]->             [ St($I, ~k) ]
rule Read: [ St($I, k) ]           --[ Read($I, k) ]->             [ St($I, k) ]
rule Kill: [ St($I, k), Fr(~k2) ]  --[ Kill($I, k), Set($I, ~k2) ]-> [ St($I, ~k2) ]
```

`no_read_after_kill` does not terminate here, with or without induction.
Delete the restore from `Read` and it **verifies in four steps**. So the cause
is the consume-restore loop alone, it is fundamental to Tamarin's backward
search, and nothing in the RHTN modelling caused it. Everything tried before
that diagnosis -- five invariant shapes, the loop-aware heuristics, removing
the adversary -- was guessing, and the minimal theory settled it in one run.

**Which means the obligations were in the wrong tool.** They are safety
properties of a mutable local state machine: a record that is written, read
many times, and overwritten. That is what TLC enumerates for a living, and
this repository already had a TLA+ layer. `tla/SupersessionDiscipline.tla`
checks all three exhaustively over two nodes and three key generations -- 100
distinct states, complete graph depth 7.

**Two modelling traps caught while writing it**, both of which would have made
the check pass while meaning nothing:

- `Superseded(n)` was first DERIVED as `everHeld[n] \ {record[n]}`. That makes
  `RecordIsNeverSuperseded` a tautology -- the current generation is excluded
  by definition, so a walk-back onto a superseded key satisfies it silently.
  Supersession is history and never un-happens, so it is now accumulated state.
- `Issue` and `Serve` are deliberately UNGUARDED. Had they required the key to
  be current, the invariants would restate their own preconditions. Instead
  they act on whatever the state holds and a flag records whether that was
  superseded, so the invariant tests the machine's shape. That is precisely
  design 12.6.5's "enforced by replacement rather than by a check", and the
  claim is now tested rather than assumed.

**The cross-document dependency is now an executable gate.** Setting
`SeriesCheck = FALSE` removes light-client's "never take a series reissue into
a series you have occupied before" -- another document, another party -- and
TLC violates `NeverIssuedForASupersededKey`. `run-all.sh` FAILS if that
violation stops occurring, because a clean mutation run would mean the
invariant had quietly stopped depending on the rule that holds it up.

The Tamarin `.bounded` fragments are kept. They are strictly weaker than TLC's
result and earn their place by checking the TAMARIN rules -- the ones the other
lemmas in those theories rest on -- rather than a separate TLA+ encoding.

13 artifacts. 43 Tamarin lemmas unbounded + 13 bounded, 4 TLA+ models, 1
mutation that must fail. Nothing open.

---

## Cross-family Tamarin review 7: the partition was wrong (2026-09-06)

Static-only again. The reviewer's scope was the WIRE component, and the finding
is the sharpest of the series: **`wire-only/` contained lemmas that are not
wire properties.** Verified and applied.

**The claim.** `ceremony`'s `Participant_Sign` consumed a `CoPresent` token,
so a legitimate participant could not use its own key without having met.
`recovery`'s `Recognise` consumed `AtMeeting` the same way. Nothing in the
protocol gates the use of a party's own key, and nothing could. The design says
so twice: design 7, "bilateral collusion is unpreventable"; design 15.4,
presence "is not an unforgeable primitive, and bilateral collusion defeats it
regardless". `wire-format.md` 4.1 says a recovery verifier "can be mistaken or
lying and nothing checks it".

**The partition was settled by the prover, not by argument.** A rule was added
letting a legitimate principal sign a body off the network with its own
uncompromised key -- no theft, no `CoPresent`. Seven lemmas falsified and named
themselves: four in ceremony (`presence_requires_copresence`,
`no_remote_forgery`, `copresence_binds_one_roster`,
`formation_requires_copresence`) and three in recovery
(`key_alone_insufficient`, `recognition_names_the_meeting_key`,
`recovery_requires_a_meeting`). All seven moved to `compliant/`, reframed with
a `SkippedTheMeeting` carve-out. Both trees now carry a reachable witness that
the collusion trace survives.

**The first probe was wrong and green, which is the lesson.** It required
`!Ceremony`, a fact only `Meet` produces, so it still needed a meeting and
every lemma verified -- the partition looked confirmed when nothing had been
tested. A second probe on recovery emitted `Recognised` where the rule emits
`RecognisedFor`, and mislabelled two lemmas in the opposite direction. Both
were caught by reading the rules rather than the verdict. **A green result from
a probe proves nothing until the probe is shown to reach the thing it probes.**

**KEY COMPROMISE IS NOT DISHONESTY**, and that conflation was the root error.
`Compromised(P)` means P's key was stolen. It was doing double duty for "P used
its own key contrary to client policy", which requires no theft -- so the
theorems read stronger than they were while the carve-out looked principled.

**What wire-only now proves instead is ATTRIBUTABILITY**: an accepted record
was really signed, over that exact body, by each party it names, or that
party's key was stolen. That survives collusion, and it is what makes design
15.4's "attack cost rather than an absolute primitive" bite -- a fabricated
record is a signed lie by named parties. Recovery keeps its four bindings for
the same reason: a lying verifier signs a real recognition, so acceptance stays
attributable to it.

**F4, verified and half-fixable.** `Distinct($P, $S)` was commented as the
wire's "node and patron MUST differ" rule. It is not: `$S` is the PRIOR
identity, and `wire-format.md` 4.1's field 1 is the NEW key. `Distinct(pkOld,
newkey)` -- "prior_key MUST differ from field 1" -- was missing entirely and is
now added. The node/patron rule CANNOT be stated in this abstraction, which
carries identity as a stable name while RHTN identifies a node by its keyhash;
the comment now says so instead of implying the check is made.

**F5 recorded, not closed.** Any registered patron may issue currency for any
subject -- no `PatronOf` relation, no issuer role. The lemmas establish "the P
whose key signed this issued it", not "P was authorised by one of the ladder's
paths". Noted in the file. F3 (client authentication) and F6 (0-RTT) remain as
already documented.

**The honest-path guard fired once more**, on the first version of
`compliant/recovery`'s meeting: the meeting minted the successor but never
published it, so the two factors could never name one key.
`a_conforming_recovery_completes` reported *falsified, no trace found*
immediately. Fourth occurrence; second caught in the same minute.

13 artifacts. 46 Tamarin lemmas unbounded (20 wire-only, 26 compliant) + 13
bounded, 4 TLA+ models, 1 mutation that must fail.

---

## F3 closed, and per-folder capability READMEs (2026-09-06)

**F3.** `wire-format.md` 9.1: "Authentication is mutual ... [the serving node]
MUST bind the identity for which session state and queued data are requested to
the identity the transport authenticated, rejecting any mismatch ... [otherwise
a party] could claim any keyhash and receive another node's queued messages."
The theory carried only the client-authenticates-server half.

Modelled by keeping the two identities as SEPARATE TERMS -- `$A`, established
by verifying a signature over the server's own challenge, and `$Claim`, what
the Attach asked for. That separation is the whole requirement: a model
carrying them as one variable cannot express the attack, which is why the
property was previously unstatable rather than merely unproven. `$Claim` is a
free variable in the client rule, so nothing stops a client naming someone
else's keyhash; the defence is the server's check.

Three lemmas added: `client_authentication`,
`queued_data_reaches_only_the_authenticated_peer`, `a_delivery_is_reachable`.
Mutation-tested: delete `Eq($A, $Claim)` and the mailbox lemma falsifies with
the trace 9.1 describes, while `client_authentication` stays verified -- the
mutation hits exactly its own lemma.

**One correction found by a lemma rather than by reading.** The first version
signed the client's challenge response without covering the claim, so an
adversary could re-pair one client's authentication with a different Attach.
`client_authentication` falsified on exactly that. The fix is faithful rather
than cosmetic: wire 9.1 carries `Attach` INSIDE the mutually authenticated
transport, so the claim is not a separable token, and the signature now covers
it.

**Per-folder READMEs** added to `simulation/`, `tla/`, `tamarin/wire-only/` and
`tamarin/compliant/` at the author's instruction: two bulleted lists each, what
the model demonstrates and what it cannot, and nothing else -- no framing, no
history, no open items.

13 artifacts. 49 Tamarin lemmas unbounded (23 wire-only, 26 compliant) + 13
bounded, 4 TLA+ models, 1 mutation that must fail.

---

## F5 and F6 closed (2026-09-06)

**F5 -- issuer identity and role.** `wire-format.md`'s CurrencyAttestation
carries field 5 (issuer role: patron, sibling, grandpatron, down-line
threshold) and field 6 (issuer identity), both under the signature. The theory
had neither: any registered patron could issue for any subject.

Two things added, and they answer different halves. `an_attestation_binds_its_
issuer_and_role` is the wire property -- an attestation issued as a sibling
cannot be re-presented as a patron's, nor attributed to another issuer.
`acceptance_names_an_issuer_this_party_authorised` is the check that makes the
role mean anything.

**The authorisation relation is indexed by the RELYING PARTY**, not global.
`!Authorised($R, $I, $S, role)` says *R records I as authorised for S in this
role* -- per-observer (design 16.1), no shared topology. That placement is the
finding, not a detail: a global relation would have asserted a fact no node
can hold. A party inside the horizon has the adoption records; one outside
does not and accepts nothing on this ground. The lemma therefore says the
accepting party consulted its OWN records rather than the attestation's
say-so, which is all a relying party can do and all the wire can support.

Issuance itself is unrestricted, because field 5 is a CLAIM: any keyholder can
sign an attestation calling itself anyone's patron, and the theory says so
rather than modelling an authorisation the wire cannot enforce.

Mutation-tested both: drop `role` from the signed tuple and the binding lemma
falsifies; drop the `!Authorised` premise and the authorisation lemma
falsifies. Each hits its own lemma only.

**F6 -- 0-RTT, and it went in `compliant/`.** `wire-format.md` 9.1: "A server
MUST NOT process an `Attach` received in TLS 1.3 0-RTT early data ... THE RULE
SITS ON THE SERVER BECAUSE THAT IS WHERE IT IS CHECKABLE." That last clause
places it: checkable by the server about its own processing, and by nobody
else. So it is a conformance obligation, and putting it in `wire-only/` would
have repeated the mistake review 7 found.

**Replayability is modelled as persistence.** `!EarlyData` is a persistent
fact, consumable any number of times, which is exactly what "early data is
replayable" means and the only property of 0-RTT the rule needs. A conforming
server consumes a LINEAR `Handshaken` token instead, so one completed
handshake admits one binding. The harm is kept reachable: a server that acts
on early data binds the same `Attach` twice with no second handshake.

13 artifacts. 54 Tamarin lemmas unbounded (25 wire-only, 29 compliant) + 16
bounded, 4 TLA+ models, 1 mutation that must fail. Every finding from reviews
6 and 7 is now applied or closed.

---

## Cross-family wire-only review 8 (2026-09-06)

Static-only. Five findings, all verified, all applied. **F2 is a regression I
introduced in review 7** and is the most important thing in this entry.

**F2 -- the anti-vacuity guards stopped guarding.** Adding
`Participant_Sign_Without_Meeting` to repartition the trees left
`honest_ceremony_completes` satisfiable without the honest rule firing at all:
a genuine `Meet`, two without-meeting signatures, a witness, an accepted
record, nothing compromised. Verified by deleting `Participant_Sign` outright
-- **the guard still verified.** Recovery was worse: its guard asked only for
SOME meeting before SOME acceptance, binding neither the successor, the
verifier relied upon, nor an honest recognition.

Fixed by emitting a distinct action from the co-presence-consuming branch
(`HonestParticipantSigned`, `HonestFormationSigned`, `HonestlyRecognised`) and
requiring it from both parties, with the recovery guard now naming the same
`(V, S, newkey)` across `MeetingKey`, `HonestlyRecognised`, `ReliedOn` and the
acceptance. Both re-tested by deletion: each guard now reports *falsified, no
trace found*.

**This is the fifth time this session a change made an honest path unreachable
or unrequired while every universal lemma stayed green, and the first time an
outside reviewer caught it rather than the guard.** The guard was the thing
that failed. Worth stating plainly: adding a rule that BYPASSES an honest
branch silently weakens every exists-trace lemma that does not name that
branch, and the branch must be named by an action, not implied by the shape of
the trace.

**F1 -- attribution overstated.** The lemma quantified the two participants
only, while the README claimed "every party it names". Extending it to the
witness FALSIFIED, and the trace was instructive: `Participant_Sign_Without_
Meeting` instantiates at `W` too, producing the same body under W's key, so
`WitnessSigned` never fires. The honest statement is authorship -- *this key
signed this body* -- so all three signing rules now emit a common `SignedBody`
and the lemma is stated over that. The projection limit (the signed term omits
timestamps, verifier responses, witness metadata, disclosure commitments) and
the single-witness signer set against the wire's sixteen are now stated in the
file and in both READMEs.

**F3 -- the threat model was not uniform.** Ceremony and recovery let a
legitimate principal sign anything; attach and currency did not, so their
authentication lemmas concluded that an honest TRANSITION had been taken when a
signature can only show that A KEY SIGNED A TERM. Added
`Server_Signs_Without_Responding`, `Client_Signs_Without_Attaching` and
`Issue_Currency_Arbitrary`, and restated `server_authentication` and
`client_authentication` over `AttachResponseSigned` / `ClientAuthSigned`. The
honest guards in both theories now also exclude `SignedWithoutTransition`, or
F2 would have reappeared immediately in the theories just repaired.

**F4 -- README wording.** "An accepted recovery" became "a patron that accepts
recovery evidence", matching the scope note the file already carried.

**F5 -- verified and applied.** `client_commit_is_injective`'s
server-compromise disjunct was unnecessary: removed, and the unconditional
lemma verifies in 12 steps.

**Stale header removed.** `ceremony.spthy`'s introduction still claimed an
accepted record implies co-presence and that binding signatures can only come
from a `CoPresent` token -- both false of this theory since review 7.

**Three mutation attempts were no-ops before they were caught**, all in this
session's pattern: a slice taken in the wrong direction because
`Recognise_Without_Meeting` precedes `Recognise` in the file, and an assertion
whose expected count ignored that the action name also appears in a comment.
Each was found by asserting the mutation landed rather than reading its
verdict.

13 artifacts. 54 Tamarin lemmas unbounded (25 wire-only, 29 compliant) + 16
bounded, 4 TLA+ models, 1 mutation that must fail.

---

## Cross-family wire-only review 9 (2026-09-07)

Static-only. Five findings, all verified against the text, all applied, each
mutation-tested. Two were mine from review 8; three are refinement gaps at the
boundary between the symbolic abstraction and the wire.

**W1 (HIGH) -- the identity/key binding was assumed, not checked.**
`Register_Node` produced `!Pk($A, pk(~sk))` and `Client_Finish` looked the key
up BY IDENTITY, so the symbolic name was born bound to exactly the key that
authenticates it. But that binding is a normative wire check under review:
`infra-client-requirements.md` says "the handshake presents the CLASSICAL
COMPONENT while the keyhash COMMITS TO THE PAIR", so reaching an intended
identity takes two checks -- `h(KeyMaterial)` equals the intended keyhash, and
the presented key equals that pair's classical half. An implementation omitting
either had NO REPRESENTATION: there was no trace in which the wrong key could
be reached under the right name.

`KeyMaterial` now arrives over the network ("transmitted on first contact and
pinned thereafter") and both checks are explicit, in both directions -- 9.1 has
the serving node "authenticate the client the same way". Mutation-tested
separately: deleting either check falsifies `server_authentication`.

**W4 (MEDIUM) -- hybrid signatures were one key with one compromise event.**
The wire "requires both identity components to sign an archive-retained
transaction". One symbolic key could express only whole-signer-intact or
whole-signer-gone, so the state hybrid EXISTS TO SURVIVE -- one component
broken, the other sound -- was not a state, and a validator checking only the
classical entry was indistinguishable from one checking both.

Split into `!LtkC`/`!LtkP` with separate compromise events, and acceptance now
verifies both entries per logical signer. Two new lemmas state what the second
component buys: `classical_compromise_alone_does_not_forge` (ceremony) and
`forging_a_recognition_needs_both_verifier_halves` (recovery). Both
mutation-tested by dropping a post-quantum check.

**The asymmetry was modelled rather than smoothed over.** `wire-format.md`
hybridises field 9 (a verifier's `match` -- "field 9 forges a VERIFIER'S
attestation, which is the attack", protection "permanent") and field 3 (the old
identity's successor statement), but leaves **field 7 classical on purpose**:
"signed by the very key an attacker mounting a fraudulent recovery already
controls -- hybridising it protects nothing." Hybridising uniformly would have
been easier and would have misrepresented a deliberate decision.

**W2 (MEDIUM) -- authorisation was permanently historical.** `!Authorised` was
persistent and nothing withdrew it, so the lemma meant "recorded at some
point", not "authorises now" -- while the roles name CURRENT relationships that
design 12.6.5.1's ladder moves among. Making the record revocable in Tamarin
produced the consume-and-restore regress again (attempted; no verdict in two
minutes), so the lifecycle went to `tla/IssuerAuthorisation.tla`, checked
exhaustively, with a mutation: the STALE reading -- which is precisely what the
persistent Tamarin fact embodies -- violates the invariant. The Tamarin lemma
is renamed `..._recorded_as_authorised` for what it actually proves.

**W3 (MEDIUM) -- mine.** `ClientAuthSigned` carried no `ns`, so the lemma had
no challenge to compare and an edit removing the challenge from the signed
payload would not have falsified it. Propagated into the action, `Bound`, and
the lemma; mutation-tested.

**W5 (LOW) -- mine, and two parts.** `IssueFreshNotStale` quantified over
`Issued`, which BOTH issuance rules emit, so it made a dishonest keyholder
cryptographically unable to sign a stale epoch -- an inability no wire format
confers. Now bound to `IssuedFresh`, emitted only by the conforming
transition. And `Transfer_Countersign` minted its subject with `Fr`, so a
former patron could only countersign for a node nobody had heard of; the
subject now comes off the network.

14 artifacts. 56 Tamarin lemmas unbounded (27 wire-only, 29 compliant) + 16
bounded, 5 TLA+ models, 2 mutations that must fail.

---

## Cross-family wire-only review 10 (2026-09-07)

Static-only. Four findings, all verified, all applied, each mutation-tested.
**Two were false claims I had written**, and those are the ones worth reading.

**F2 (HIGH) -- the hybrid guard guarded one signer out of three, and a comment
in the file said otherwise.** `Accept_Record` checks six signature components,
and the comment beside them claimed dropping "any one of the six" falsifies
`classical_compromise_alone_does_not_forge`. It does not: that lemma quantified
only over `P1`. Verified by deleting P2's post-quantum check -- **every lemma
stayed green.**

The deeper fault was the carve-out. `an_accepted_record_is_attributable`
excused a signer on `Compromised(A)`, and BOTH `Compromise_Classical` and
`Compromise_PostQuantum` emit that event -- so compromising one half exempted
that signer from attribution entirely, which is the exact opposite of what
hybrid is for. Every carve-out in ceremony and recovery now requires BOTH
halves. That single change guards all six checks, so
`classical_compromise_alone_does_not_forge` was deleted as subsumed rather than
repaired. Re-tested by deleting each of P1's, P2's and W's PQ check in turn:
all three now falsify.

**F1 (HIGH) -- Transfer was classical, and that was my error.** Last round I
read the Recovery block's field-by-field hybrid decisions and generalised
"field 7 stays classical" into leaving the transfer countersignature classical
too. `wire-format.md` is explicit: Transfer field 2 is a `COSE_Sign` "for the
same reason field 3 of Recovery" is one. The model admitted a transfer evidence
gate after CLASSICAL-ONLY compromise of the former patron. Now hybrid;
mutation-tested.

**F3 (MEDIUM) -- the rotation check compared unlike terms.** `Distinct(pkOldC,
newkey)` compared the prior identity's classical PUBLIC KEY with the successor,
while `wire-format.md`'s rule -- "`prior_key` MUST differ from field 1" -- is
between two KEYHASHES, an identity being the hash over its `KeyMaterial` pair.
The check caught a successor equal to the old classical key, which is not the
wire's case, and nothing in the theory stood for the old identity itself. `!Kh`
now does. New lemma `a_recovery_never_installs_the_prior_identity`, falsified
when the check is removed.

**F4 (MEDIUM) -- the verifier response had no result, and the README claimed a
field that is not modelled at all.** `wire-format.md` gives a response four
results and requires "at least one actual `match`"; all four are signed by the
same verifier over the same fields, so a validator taking any authentic
response would accept a signed `no-match`. Now modelled, with
`acceptance_requires_a_signed_match`. The README said "Recovery's field 7 is
modelled classical" -- field 7 is not modelled, and the line is removed. What
remains abstracted (`query_id`, field 7 consent, selection basis) is now stated
as an assumption in both the file and the README, because no refinement
argument establishes it.

**A pattern worth naming.** Both false claims were about MUTATIONS AND
COVERAGE, not about the protocol: a comment asserting a mutation I had not run,
and a README sentence generalising from one signer to all. The lemmas were
sound; the prose around them was not. Neither would have been caught by
re-running the suite, because the suite was green in both cases.

14 artifacts. 57 Tamarin lemmas unbounded (28 wire-only, 29 compliant) + 16
bounded, 5 TLA+ models, 2 mutations that must fail.

---

## Cross-family wire-only review 11 (2026-09-07)

Static-only. Three findings, all verified, all applied, each mutation-tested.

**F1 (HIGH) -- three theories used the abstraction attach's own comments say is
inadequate.** `attach.spthy` was corrected in review 9 to carry candidate
`KeyMaterial` and check `h(km)` against the intended keyhash before using any
key from it. Ceremony, currency and recovery still looked verification keys up
BY SYMBOLIC NAME, which makes the identity-to-key relation true before
validation: a validator that resolved the wrong key material under the right
identity had no trace, and the attribution lemmas stayed green for exactly that
error. The reviewer found this by reading attach's own rationale against the
other three -- an internal-consistency argument, which is the strongest kind.

Fixed in all three: acceptance now takes candidate `KeyMaterial` off the
network and binds it before use. Mutation-tested in each.

Note what this closes that the hybrid work did not. Review 10's carve-outs
require both halves of a NAMED signer to be compromised -- but under a missing
keyhash check, neither half need be: the attacker binds its OWN pair to the
victim's identity. The two findings look similar and are not.

**F2 (MEDIUM) -- domain separation held by symbolic message shape.** The role
tag sat inside the signed tuple, so `<'rotate', ...>` and `<'recognise', ...>`
were simply different terms. `wire-format.md` declines to rely on that:
"exploiting cross-context confusion requires a byte string valid in two roles,
which the differing CBOR structures argue against WITHOUT RULING OUT -- and
unproven non-confusability is precisely what domain separation exists to
replace." Signed terms are now `<aad, payload>` with the aad supplied by the
verifying rule.

**The collision is real in recovery**, which is what makes the new lemma more
than decoration: the successor statement's payload is `<S, newkey, P>` and a
verifier response's is `<S, newkey, result>`. Set `result = P` and they are the
SAME TERM. `a_recognition_is_not_accepted_as_an_old_key_proof` falsifies when
the verifier checks the payload alone.

**F3 (MEDIUM) -- a recovery block is a collection.** The wire allows 32
responses and its rules are per-collection: every response names the new node,
duplicate verifier responses are malformed, and the block needs at least one
`match`. The model had one response, so a validator that checked the first and
stopped satisfied it. Two responses now, with all three rules; dropping the
second response's checks falsifies
`forging_a_recognition_needs_both_verifier_halves`.

**One lemma was wrong and its falsification said so.** `acceptance_requires_a_
signed_match` demanded a match from EVERY verifier relied on, and broke the
moment a second response existed -- correctly, because the wire requires at
least one, not all. Restated.

**A gate change, not an exemption.** Recovery's derivation checks began timing
out, which the gate reports as a wellformedness failure. The fix is
`--derivcheck-timeout=60` so the check COMPLETES, rather than a pattern that
would have let a real failure through.

14 artifacts. 58 Tamarin lemmas unbounded (29 wire-only, 29 compliant) + 16
bounded, 5 TLA+ models, 2 mutations that must fail.

---

## Cross-family wire-only review 12 (2026-09-07)

Static-only. Five findings, all verified, all applied. **F1 is a bug I
introduced in review 11, and it is the most instructive entry in this file.**

**F1 (HIGH) -- the honest recovery path signed a term acceptance could not
verify.** Review 11's context separation substituted on the pattern
`<'recognise', $S, newkey, result>`. The honest `Recognise` rule signs the
LITERAL `'match'`, not the variable, so the pattern missed it and that rule was
left signing the old flat term. Its signature could never satisfy
`Patron_Accept_Recovery`.

The gate did not catch it because `recovery_completes_honestly` named only
ACTIONS -- a meeting, an honest recognition, an acceptance -- and
`Recognise_Without_Meeting` supplied the signature acceptance actually
consumed. The trace contained an honest recognition whose SIGNATURE WAS NEVER
USED. That is the sixth occurrence this session of an honest path being
unreachable or unrequired while the universal lemmas stayed green, and the
second where the guard itself was the thing at fault.

Two things were wrong and both are fixed: the guard now excludes
`RecognisedWithoutMeeting` outright, so it witnesses the conforming path
reaching acceptance; and the substitution that caused it is recorded as what it
was -- **a replacement reporting "3 roles separated" that was never checked
site by site.** The CLAUDE.md rule about mechanical substitution says exactly
this, and the count assertion I did write (`count > 0`) was too weak to catch a
missed site.

**Three role tags were invented.** `rhtn/1:recognise`, `rhtn/1:presence` and
`rhtn/1:formation` do not appear in `wire-format.md`'s role table. The wire has
`rhtn/1:verifier` for verifier responses and `rhtn/1:envelope` for transaction
envelopes -- both presence subtypes use the latter. Corrected. The two I got
right, `rhtn/1:successor` and `rhtn/1:transfer`, were the two I copied from the
table rather than named from the rule.

**F3 (MEDIUM) -- the invented contexts also hid the real mechanism.** Normal
and formation are separated on the wire by SIGNED BODY FIELD 6, not by
context. Giving them different contexts added a cryptographic separation the
protocol does not have and made the one it does have untestable. The subtype is
now the first element of a shared payload shape, with the formation witness
slot carrying `'none'` rather than being absent.

**F2 (MEDIUM) -- cardinality and match position.** Acceptance required exactly
two responses with the match hard-wired to the first slot; the wire's array is
nonempty, and the match requirement is existential. A one-response acceptance
is added and the match is now a disjunctive restriction over the block. This
was not cosmetic: with two responses mandatory and one honest meeting
modelled, the conforming path could not reach acceptance at all, which is why
F1's corrected guard falsified until this was fixed. The two findings were one
problem seen from two sides.

**F4 (MEDIUM) -- one epoch token stood for two signed timestamps.** Splitting
them was not enough on its own: acceptance took `!Epoch(iat, exp)` as a PAIR,
which re-bound the two to something the environment had minted together, so the
binding of field 4 was enforced by the model rather than by the signature. A
validator consults no such registry. With the premise removed, omitting
`expires_at` from the signed payload falsifies
`currency_requires_unexpired_issuance` -- three mutation attempts were needed
to find a form that tested the right thing.

**F5 (LOW) -- the hybrid lemma was one-directional.** Added the mirror.

**One mutation is not yet resolved.** Dropping the signed subtype from
`ceremony.spthy` should let a formation signature satisfy a normal record's
check; the proof search on the mutated theory has not returned a verdict within
900 seconds, which is unsurprising because removing the subtype is exactly what
makes the two payload languages collide. **The subtype's load-bearingness is
therefore asserted from the term structure and NOT established by mutation.**
Recorded rather than claimed.

14 artifacts. 59 Tamarin lemmas unbounded (30 wire-only, 29 compliant) + 16
bounded, 5 TLA+ models, 2 mutations that must fail.

**Correction to the entry above, same day.** It records the subtype mutation as
"asserted from the term structure and NOT established by mutation". That is no
longer true and the correction is worth more than the original note.

The general lemma could not be mutated -- 900 seconds, no verdict, killed
during source saturation -- because removing the subtype is precisely what
makes the presence and formation payload languages collide, and that collision
is what explodes the search. The fix was not a longer budget but a NARROWER
PROPERTY: `a_formation_signature_is_not_accepted_as_a_normal_record` pins the
witness slot to the value a formation carries there and drops the compromise
disjunctions. It verifies in seconds, and falsifies in seconds when the subtype
is removed from all 20 signing and verifying sites.

Worth keeping as a technique. When a mutation will not terminate against a
general property, the answer is a property narrow enough to isolate the thing
being mutated -- not more wall clock, and not a claim asserted from reading the
terms.

Also worth noting: the assertion in that mutation script fired on its first
run, catching sites that had NOT been replaced. They turned out to be inside a
comment quoting the terms, so the assertion was over-strict rather than the
substitution being wrong -- but that is the failure this session has hit
repeatedly in the other direction, and the strict assertion cost one round trip
where a weak one has repeatedly cost a false claim.

31 wire-only lemmas. 60 Tamarin lemmas unbounded, 14 artifacts, all pass.

---

## Cross-family wire-only review 13 (2026-09-07)

Static-only. Five findings, all verified, all applied. One is a real defect;
**three are overclaims of mine, and two of those are a worse kind than this
session has seen before.**

**F1 (HIGH) -- a per-slot check written into one slot.** The two-response
recovery rule carried `Distinct($V, $S)` and no `Distinct($V2, $S)`, while
emitting `ReliedOn` for both verifiers. A subject could sign its own
recognition into slot 2 and every lemma stayed green -- because the
self-recognition emits genuine `RecognisedAs` and `RecognisedFor` facts and the
old-key proof is real. Nothing in the suite looked at WHICH identities had been
excluded.

Fixed, and guarded by `no_relied_on_verifier_is_the_subject`, stated over the
RELIANCE ACTION rather than over a slot. That is the point: a property
quantified over "every verifier relied on" cannot be satisfied by adding the
next check to only one of them. Mutation-tested.

**F2 and F3 -- I manufactured the collisions I then cited as evidence.**

`a_recognition_is_not_accepted_as_an_old_key_proof` falsified when the verifier
dropped its `external_aad`, and that was reported as a concrete cross-context
replay the tag prevents. It was not. The falsification depended on the
successor statement and a verifier response sharing one symbolic three-tuple,
so that `result = P` made them the same term. On the wire a `SuccessorStatement`
is a CBOR ARRAY of three keyhashes and a verifier signature covers a MAP of
fields 1-8 and 10; the top-level structures differ before any field is read.

`a_formation_signature_is_not_accepted_as_a_normal_record` was the same
mistake. It needed a formation to carry the sentinel `'none'` in the witness
slot a normal record uses -- but `wire-format.md` says keys 4 and 5 are "ABSENT
always on a formation" and that their absence "announce[s] it regardless". The
shared shape was mine.

Both models are now faithful -- `succ_stmt/3` and `vresp/3` are distinct
constructors, a formation payload is a shorter tuple -- and **both lemmas are
deleted rather than repaired.** Domain separation and the signed subtype remain
normative and are still modelled; what is gone is the claim that this suite
demonstrates why they are needed.

The distinction worth keeping: an abstraction that is too COARSE loses attacks,
which is the familiar failure. These two were too coarse in a way that
INVENTED one, and the mutation then "confirmed" it. A green mutation is only
evidence if the collision it exploits exists in the thing being modelled.

**F4 -- a claim about my own file that was false.** The comment said splitting
the timestamps made "omitting or substituting either mutation-testable". Only
`exp` appeared in `Issued` and `Accepted`, so no property had an `iat` to
compare and an implementation consistently leaving `issued_at` unsigned
contradicted nothing. `the_accepted_issue_time_is_the_signed_one` added and
mutation-tested.

**F5 -- a disclosed limitation that was narrower than the truth.** The
identity/keyhash split also suppresses the transfer's `former_patron != node`,
not only the adoption's node/patron inequality. Recorded in the README.

14 artifacts. 60 Tamarin lemmas unbounded (31 wire-only, 29 compliant) + 16
bounded, 5 TLA+ models, 2 mutations that must fail.

---

## Cross-family wire-only review 14 (2026-09-07)

Static-only. Two findings, both verified, both applied, both mutation-tested.
They are one class: **the rule holds the provenance and the lemma throws it
away.**

**F1 -- recovery lemmas discharged on evidence the acceptance never
consumed.** `acceptance_requires_a_signed_match` asked only that SOME
`RecognisedAs(V, S, newkey, 'match')` existed anywhere in the trace, and its
compromise branch quantified over ANY verifier -- one that had never taken part
in the recovery satisfied it. `recognition_binds_the_successor` was the same
shape. So a regression admitting a bad response in either slot could leave both
lemmas satisfied by an unrelated verifier's good one.

`ReliedOnResponse(P, V, S, newkey, result)` is now emitted once per response
actually accepted, in both the one- and two-response rules. The match lemma
requires the match to be among them; the successor lemma is UNIVERSAL over
them, with each compromise exception tied to that verifier rather than to any.
Removing the second response's signature checks now falsifies it -- the exact
regression the old form could not see. The stronger property costs 1398 proof
steps against the old handful, which is about what the extra content is worth.

**F2 -- currency's role binding was split across two uncorrelated facts.**
`Issued(I,S,key,iat,exp)` and `IssuedAs(I,S,role)` shared no attestation, so
the pair of lemmas could be discharged by TWO DIFFERENT attestations: the
accepted key's timestamps from one, the accepted role from another the same
issuer had made earlier for the same subject. Neither lemma noticed that the
accepted role had never been signed over the accepted key.
`IssuedAttestation(I,S,key,iat,exp,role)` carries the whole signed tuple --
wire fields 1 to 6 -- and both lemmas now depend on it. Dropping `role` from
the signature falsifies.

**The class is worth naming, because it is the third distinct way this suite
has been green while proving less than it read as.** Earlier rounds: a lemma
whose carve-out fired on one compromise half; a guard naming actions rather
than the signature consumed; and now an existential satisfiable by a coincident
trace fact. In all three the RULES were right. What failed each time was the
edge between the rule and the property -- and none would have been caught by
re-running the suite.

14 artifacts. 60 Tamarin lemmas unbounded (31 wire-only, 29 compliant) + 16
bounded, 5 TLA+ models, 2 mutations that must fail.

---

## Cross-family wire-only review 15 (2026-09-07)

Static-only. One finding, verified and applied, and it is the same class as the
last round's two: **the rule holds the provenance and the lemma throws it
away.** Third round running.

**T-01 -- the queue theorem lost the session.** `Server_Bind` checked the wire's
binding correctly and then produced `Session($S, $Claim)`, dropping the
challenge `~ns` the session was established under. `Deliver` could not name the
session that authorised it, and the lemma asked only whether S had
authenticated R SOMEWHERE EARLIER IN THE TRACE. A stale authentication from a
finished session discharged it.

`Server_Bind` is the only producer of `Session`, so the present rules could not
exhibit the defect -- which is exactly why it was worth fixing. The property
existed to catch a future session-creation path (resumption, restore, failover)
that skipped the bind, and it would not have.

**Demonstrated rather than argued, on the second attempt.** The first mutation
-- a restore rule producing `Session` out of nothing -- falsified BOTH the old
and new forms, because the trace it found contained no authentication at all
and so failed even the weak version. It discriminated nothing. The faithful
construction is the reviewer's own scenario: a restore path reachable only
AFTER a legitimate authentication for that pair. Against that,

    new (per-session) form:  falsified  (8 steps)
    old (trace-wide)  form:  verified   (4 steps)

which is the finding, exhibited. Worth recording because a mutation that
falsifies is not automatically a mutation that DISCRIMINATES, and the first one
looked like success.

**One consequence handled.** Threading the token meant `Deliver` briefly
restored `Session`, which is the consume-and-restore shape whose backward
search does not terminate here -- the same regress that moved three obligations
to `tla/`. `Deliver` no longer restores it: one delivery per session loses
nothing for this property, since the question is whether ANY delivery happens
off an unauthenticated session.

14 artifacts. 60 Tamarin lemmas unbounded (31 wire-only, 29 compliant) + 16
bounded, 5 TLA+ models, 2 mutations that must fail.

---

## Cross-family wire-only review 16 (2026-09-07)

Static-only. Three findings, all verified, all applied; the two HIGH ones
mutation-tested.

**F1 (HIGH) -- I swapped one empty provenance for another.** Review 14 replaced
a trace-wide existential in `acceptance_requires_a_signed_match` with
`ReliedOnResponse`, an action emitted BY THE ACCEPTANCE RULE at the same
instant. So the lemma concluded that acceptance had recorded a match -- true by
construction, and silent about what any verifier signed. A response whose
result was outside signature coverage could be upgraded from `no-match` to
`match` in transit and nothing would fail.

Two rounds, two directions, same hole: first a property that could be
discharged by an unrelated trace fact, then one that could be discharged by the
rule's own emission. The link that was missing both times is between the
acceptance edge and the SIGNING event, carrying the same `result`.
`every_relied_on_response_was_signed_as_such` supplies it; removing `result`
from the verifier's signed payload falsifies it.

**F2 (HIGH) -- the wire's prior-key equality was true by representation.**
`vresp($S, newkey, result)` used the same symbol for the response's prior
identity and the recovery's, so there were never two values to compare and the
mandatory check could not be stated. The wire is explicit that a correctly
signed response about identity X can otherwise sit under a recovery claiming Y
with every signature valid. The response now carries its own prior-key field
and both acceptance rules compare it; removing the comparison falsifies.

**A correction found while fixing F2, by reading a counterexample.** Giving the
lying verifier its prior identity from `In` let its emitted `RecognisedFor`
name one identity while its signature covered another --
`recognition_binds_the_successor` falsified and the trace showed why. The rule
now takes `!Kh($S, priorKh)`, so a dishonest verifier still chooses WHICH
identity to lie about, but the action it emits and the payload it signs agree.
That is the right shape: dishonesty is in the content, not in a mismatch
between the model's bookkeeping and its cryptography.

**F3 (LOW) -- and a comment of mine that was simply wrong.** The attach theory
called `~ns` "an unforgeable session token". It is fresh but PUBLIC -- sent in
the clear -- so any principal seeing it can sign the client-auth term over it.
The two authentication lemmas are each side's own fact and do not compose into
an agreement theorem that both completed the same TLS connection. Corrected in
the file and registered in the README; a real fix needs a connection handle
carried through both endpoint facts, which is recorded and not done.

**Documentation hygiene, which is not cosmetic in this suite.** A paragraph in
`attach.spthy` still said the theory "cannot state" the authenticated-versus-
claimed identity binding -- written when that was true, never updated when
`Server_Bind` was changed to do exactly that. A `ceremony.spthy` comment still
described a formation's witness slot as "empty" after the model moved to
absence. Both corrected. A suite that leans this heavily on commentary to
delimit what each lemma means cannot afford stale commentary.

14 artifacts. 61 Tamarin lemmas unbounded (32 wire-only, 29 compliant) + 16
bounded, 5 TLA+ models, 2 mutations that must fail.

---

## Cross-family wire-only review 17 (2026-09-08)

Static-only. Two findings, both verified, both applied, both mutation-tested.
**One produced a specification change** -- the first from this review series to
do so, and it turned on a distinction the model had been unable to draw.

**F1 (HIGH) -- the self-verifier lemma compared the wrong pair, and the wire
was missing the check.** `no_relied_on_verifier_is_the_subject` compares the
model names `$V` and `$S`, where `$S` is the identity being recovered FROM.
`wire-format.md`'s `VerifierResponse` field 2 is the NEWLY ADOPTED node. Two
different pairs; the lemma's name read as though it covered both, and the
README repeated that reading.

Stated over the successor it FALSIFIED: nothing compared the verifier's
keyhash with `newkey`, so a recovery could rely on a response whose verifier
WAS the new node -- a successor attesting to its own continuity, which
collapses recovery's second factor into the first. An attacker holding the old
key and controlling one new identity satisfies both.

**The design excluded it; 4.1 did not enforce it.** design 9.1 has the subject
meet "someone they have met before", which a key generated for this rotation is
not, and `wire-format.md` 7.3 that the party being established "is never a
candidate for their own verification". But 4.1's enumerated Recovery
consistency rules -- subject equals the newly adopted node, field 8 equals
`prior_key`, no duplicate verifier, querier is the verifier -- carried no
inequality between response fields 1 and 2.

Put to the author rather than assumed, because adding a MUST to a normative
document is his call and not a modelling decision. **Ruling: add it as a
structural rule** [author, 2026-09-08]. 4.1 now carries "every response's
verifier MUST differ from its subject", stated with the reason it belongs
there rather than in 7.3's selection rules: it is the half of that principle a
patron can check from the block alone. Both acceptance rules gained
`Distinct(khV, newkey)`; the lemma verifies and falsifies when either site is
removed.

**F2 (MEDIUM) -- the signed projection had no verifier identity.**
`vresp(priorKh, newkey, result)` collapsed the verifier a response NAMES (wire
field 1, covered by the signature) into the identity whose key validates it.
An implementation binding the signature to one and its duplicate-slot
accounting to the other had no representation. `vresp/4` now carries field 1,
signers set it, and acceptance compares it against the verifying identity.
`the_named_verifier_is_the_signing_verifier` falsifies when that comparison
goes.

**A shape error found while doing it.** The signer's wrapper and the
acceptance's input pattern had drifted to different arities, so an honest
response could only reach acceptance by adversary reassembly of its published
signatures. Both sides now carry the same six fields. Worth noting because the
exists-trace guards did not catch it: reassembly is a legitimate adversary
capability, so the honest path "completed" through it.

**Not done, and flagged.** `change-log.md`'s last dated entry is 2026-09-01;
everything since has gone here. A new MUST in a root document is the kind of
decision that file exists to trace, but restarting it after a week's gap is a
convention change and not mine to make.

14 artifacts. 63 Tamarin lemmas unbounded (34 wire-only, 29 compliant) + 16
bounded, 5 TLA+ models, 2 mutations that must fail. References 2034 / 0 flags.

---

## Coherence pass across the base documents, and models against them (2026-09-08)

Run at the author's request after review 17, alongside a change-log entry
covering the model review series.

**Base documents.** References 2034 / 0 flags. The role table carries 13 rows
against §1.2's "Thirteen roles carry a tag" -- consistent. §4.1's Recovery
consistency list gained a row this session and has no sentence counting its
members, so nothing drifted. §23.2 summarises autonomous participation and is
untouched by any of this work. The verifier-selection chain holds in one
direction only, which is right: `light-client-requirements.md` cites
`wire-format.md` §7.3 for selection, §7.3 states the candidate rule, and §4.1
now carries the structural check -- one normative home each, no third
statement added.

**One citation error in the change-log entry, found by its own convention.**
`change-log.md` has no numbered sections and its bare §N mean
`network-design.md` -- which is why refcheck.py excludes it, a documented scope
boundary rather than an exemption. A bare "§7.3" written there would have
resolved to design §7.3, "Verification by query", when the sentence quoted is
`wire-format.md` §7.3. Qualified.

**Models against the documents: two new checks, because neither existed.**

`Robot/modelrefcheck.py` checks every section citation in `models/` against
actual headings. refcheck.py covers the root and Robot/ and never covered
models/, which cite the specification 176 times. **First run: 1 flag.** `design
15.4` does not exist; the passage is design §7's preamble. It appeared in two
comment blocks in `wire-only/ceremony.spthy` and again in `models/README.md`.
Now 176 / 0 flags, and the checker is permanent.

**A quotation check, run once.** 55 quotations attributed to a document were
compared against the documents' own text. It found:

- **"an attack cost rather than an absolute primitive"** -- FABRICATED. That
  string is in no base document. design §7 says proof of presence is "a cost,
  not an unforgeable primitive" and "a cost imposed on acquiring edges into
  territory the attacker does not already control". Three sites, all corrected.
  The first fix caught one of the two occurrences in ceremony.spthy and the
  checker caught the second -- fixing the instance rather than the claim, in
  the same pass as writing the check that catches it.
- **"and NOT A FAILURE"** for "and not a violation of this rule" -- the sense
  survived, the words did not (compliant/attach).
- **"knows the series was abandoned"** for "knows which series was abandoned"
  (compliant/recovery).
- **"every node's view is its own"** attributed to the design -- that is
  `CLAUDE.md`'s phrasing, a working file. Replaced with design §16.1's own
  words (PartitionMerge.tla).

Nine flags remain and all nine are understood: one is a scare-quoted paraphrase
of mine, eight are the checker's line-flattener mangling TLA+ `\*` and `(* *)`
comment markers. Six of those eight were verified present by hand. The checker
is left in the session scratchpad rather than `Robot/` -- a gate that reports
nine known artifacts every run is the kind of noise exemptions get bought to
silence.

14 artifacts pass. 63 Tamarin lemmas unbounded, 16 bounded, 5 TLA+ models,
2 mutations that must fail. References 2034 / 0 flags; model citations 176 / 0.

---

## Flow-metric review (2026-09-08)

**The first reviewer with a working interpreter**, and it shows: 1,800
exhaustive min-cut comparisons against `max_flow()` over 2-7 vertex graphs with
antiparallel arcs, plus E3/E4 over seeds 1-100. Zero discrepancies, zero
assertion failures. Those negative results are worth more than most of the
Tamarin rounds' positives, which were all static.

Three findings. **Two applied, one NOT REPRODUCED.**

**F3 (LOW) -- applied.** E4's header claimed one peering edge "helps every
observer whose horizon contains a peer". The experiment refutes it: an
observer can see the edge and already have standing to the beneficiary, so
visibility and influence are counted separately -- seed 1 reports up to 13
observers seeing an edge and at most 7 changing. Both header sites now say
visible-to and may-influence, with a note recording what the old wording
claimed.

**F1 (MEDIUM) -- applied as a regression.** design 16.2's bound is normative
for a set's *simultaneously usable* standing, "one computation, shared
capacity", and says a policy materialising per-principal decisions draws them
"from one conserved computation, not from one computation per principal". E3
demonstrated that for a set submitted together; nothing tested what happens
across two calls, and `admit_reference_order` builds a fresh residual per call
-- correct as a pure function, and the exact shape that would let an
incremental policy spend one cut twice.

`regression_conservation_is_per_computation` exhibits it: two disjoint halves
of one region admitted separately draw 8 + 8 against a cut of 8; the same
identities admitted together draw 8. Stable across seeds 1, 7, 23, 99, 500. No
specification change -- the rule is already stated, and "simultaneously usable"
covers the cross-time case. What was missing was the demonstration that the
calling convention carries it.

**F2 (MEDIUM) -- NOT REPRODUCED, and I tried hard.** The claim: 16.3.1's
conservative-evidence property ("unseen edges cannot inflate a claim") needs the
distance-to-capacity schedule to be monotone, because a richer graph can
SHORTEN a node's landscape distance and a non-monotone schedule could then
lower its capacity.

The mechanism is real in principle and the file does say its halving schedule
is "MODELLER'S CHOICE OF SCHEDULE, NOT FROM THE DESIGN". But:

- Building the reviewer's own described graph (capacity 1 at distance 2, 100 at
  distance 3) gave the SAME standing either way, not 2 versus 1.
- A random search over 6-node graphs first appeared to confirm it -- 3 hits
  under a non-monotone schedule. **Those were artifacts.** The targets were at
  distance 1, inside the horizon, where scores are saturated sentinels around
  1e9 and where 16.2 says "there is nothing for the metric to do". The same
  search then produced 35 apparent hits under the REFERENCE schedule, which is
  what exposed the artifact: a result that damning against the reference metric
  was likelier to be my harness than the design.
- Restricted to targets OUTSIDE the horizon, where the claim applies: **5,851
  pairs, 0 counterexamples, under both the reference and a deliberately
  non-monotone schedule.**

A likely reason it resists construction: distance along a shortest path
increases by one per hop, so the low-capacity bracket is traversed in the
poorer graph too. Not a proof that no counterexample exists -- an explanation
of why my search found none.

**No specification change on F2.** A monotonicity requirement is a normative
addition, and adding one on an unreproduced finding is worse than missing it.
Referred with the numbers.

Flow metric: all assertions pass on seeds 1, 7, 23, 99, 500.

---

## Flow-metric review 2 (2026-09-08)

Executed review again: 900 exhaustive min-cut comparisons against `max_flow()`,
seeds 0-9 clean, and the reviewer was explicit that an attempted 100-seed sweep
was cut short and should not count as a passed test. Two findings, both applied,
**one producing a specification change**.

**F1 (MEDIUM) -- `f` is not the branching bound of the graph a decay policy
runs on.** design 16.2 derives the divergence of an attacker's fake subtree as
`λ^D · Σ(fλ)^k` and calls `λ < 1/f` a soundness condition; 21's table gives it
as the convergence requirement; 16.4 proposes publishing it prominently. But
`f` is "max subordinates per node", while 16.2.1 puts proof-of-presence and
peering edges in the same trust graph, where "to any party other than the two
the edge joins, both carry trust by the same rules". **Nothing bounds
acquaintance degree, and nothing should** -- meeting widely is what the network
is for. Checked the parameter table: it bounds f, L, the horizons, verifiers
per subject, and a floor on cross-tree peers; there is no cap on meetings.

So the criterion can be satisfied while the series diverges. At the configured
f = 10, λ = 0.095 satisfies λ < 1/f = 0.1 and diverges at branching 11.

Put to the author because it is a normative claim about what a published
criterion guarantees. **Ruling: say plainly that no f-only guarantee exists**
[author, 2026-09-08]. Applied at four sites -- the soundness statement in 16.2,
16.4's publishing proposal, 21's parameter-table basis, and 21's
chosen-versus-derived note. Appendix B's rejected-alternatives row says
"diverges unless λ < 1/f", which is a necessity claim and remains accurate;
history left alone.

E1 is now labelled adoption-tree arithmetic, and
`regression_f_is_not_the_branching_bound` exhibits the gap: branching 10
converges to 20.00, branching 11 reaches 154560.83 over 200 levels, on the same
λ. Those figures match the reviewer's to two decimal places.

**F2 (LOW) -- E4 counted the two endpoints.** The influence loop ran over every
node, so `pb` was asked to evaluate its OWN standing -- zero before the edge
existed, positive after, an artefact of self-evaluation -- and `pa` is the other
end of a relationship that requires a meeting, so it already holds the pair's
presence edge and 16.2.1 collapses parallel sources to one pair edge. Both
excluded, and the denominators corrected from 16 to the 14 third parties
actually evaluated. Counts fell from min/median/max 7/8/12 to 5/6/10 and the
best placement from 12 to 10 -- exactly the reviewer's predicted figures. The
qualitative result survives: one edge still moves 10 of 14 third-party
observers.

**One error of mine, caught by an assertion.** The new regression first asserted
the converging series stays under 2. It converges to 1/(1 - fλ) = 20. The
threshold was wrong, not the model.

Flow metric: all assertions pass on seeds 1, 7, 23, 99, 500. 14 artifacts pass.
References 2038 / 0 flags; model citations 176 / 0.

---

## Flow-metric review 3 (2026-09-08)

Executed review: 480 exhaustive min-cut comparisons against `max_flow()`,
`admit_reference_order()` against a one-shot super-sink flow on 250 graphs,
whole-script seeds 1-8, E4-only seeds 1-50, and an explicit disclaimer that a
larger sweep hit the execution limit and does not count. Two findings, both
verified, both applied, **one producing a specification change by ruling.**

**F1 (MEDIUM-HIGH) -- the lifecycle gap, sharpened into a specification
claim.** Last round's disclosed limitation was that conservation holds per
computation and the model cannot speak to retained entitlements. This reviewer
followed the documents' own pointer: 16.2 named 11.4 as the consumer of
"one conserved computation" -- but 11.4's scopes never leave the Dunbar Org,
and 16.2.1 says the metric is "a mechanism for reaching past the horizon rather
than for grading inside it". So the only named consumer sits where the metric
does not ration, and nothing defines a lifecycle for one that does. Four
premises verified against the text; a fifth found while checking --
`resource-requirements.md` 7.2.1's *Absolute rank* predicate consumes standing,
so infra 413's "changed member only" guidance would matter if such a consumer
existed.

The reviewer's remedy was to define an allocation lifecycle -- population,
epoch, lease, release. That is the shape the author's corrections have removed
before, so it went to him as the question CLAUDE.md prescribes: is the
component required? **Ruling: standing is not a persistent entitlement**
[author, 2026-09-08]. An observer computes it on demand from its own graph and
nothing is retained between evaluations, so there is no lifecycle to specify.
16.2 now says so in its own words and the 11.4 pointer is gone (references
2038 to 2037, exactly that one). The model's regression stands unchanged in
substance -- it exhibits why retention would break the bound -- and its
docstring, its report text, and the simulation README were updated because
they quoted the sentence that no longer exists; the quote checker is what makes
that a requirement rather than a nicety.

**F2 (MEDIUM) -- E4 measured one evidence state.** design 16.3 says it
outright: "What a peering record adds to such an observer's graph is nothing;
what it adds to an observer who can see the peering record and not the
presence record is one edge." E4's baseline had no pair edge before peering,
so it measured only the second state.

Reproduced with a harness that first self-validated against E4's own
per-placement figures (5/6/10, exact): 610 visible observer-placement cases,
390 changed with the presence record absent, **0 changed with it held**. The
reviewer reported 140/80/0 -- a different denominator I could not reconstruct
-- but the conclusion is identical and independently established. E4 now
computes both states per case, asserts zero change per placement (the collapse
rule exercised on the actual visible subgraph, not in isolation), and its
report says amortisation exists in the state where the observer lacks the
presence record.

**Two discipline notes.** A sweep's `||` was bound to `cut` rather than `grep`,
so its silence proved nothing; re-run correctly, the one remaining carrier of
the old clause was generated gate output, since regenerated. And the harness
was built to self-validate before its new numbers were trusted, which is what
let the reviewer's differing denominator be reported as a denominator
difference rather than a disagreement.

14 artifacts pass; flow metric clean on seeds 1, 7, 23. References 2037 / 0
flags; model citations 177 / 0.

## Flow-metric review 4 (2026-09-08)

Executed review: 600 exhaustive min-cut comparisons against `max_flow()`,
`admit_reference_order()` against a joint super-sink flow on 1,215 graphs,
whole-script seeds 0-34, and the same disclaimer as last round that a 0-199
sweep hit the execution limit and does not count. Two findings, both verified,
both applied. **No specification change.** The reviewer reports the
conservation result survives their own corrected construction.

**F-01 (MEDIUM) -- E3's region was not a construction the evidence rules
permit.** `visible_flow_subgraph`'s outward shells walk the adoption
FlowGraph, and the docstring says why: the ruling has an evaluator discerning
"some of a foreign subtree's structure from locator data", and locators expose
patronage. So the adoption graph is already the typed container -- peering
lives in `peer_edges` -- and whatever is put in it is, to reach, patronage
structure. `attach_region_behind` put a 32-way star and a clique at capacity
100 there and called it generosity. design 3.1: "Within a subnet the
authority relation is a tree. Every non-root node has exactly one patron, and
every node has at most f = 10 subordinates" -- so at widths 16 and 32 the gate
had more subordinates than the design permits, at ten times HIER_CAP, and E3
was proving max-flow arithmetic over an injected region rather than
conservation over what an observer's evidence can hold. All premises verified
against the file and the design; the file hashes the reviewer reports match
the committed ones.

The reviewer's first remedy was an edge-type tag on the FlowGraph. Not
required: the type split exists structurally, and what was missing was the
construction respecting it. So no component. The region is now a patronage
tree -- breadth-first, round-robin, at most FANOUT children per node, HIER_CAP
edges -- and returns its depth, which is the `reach` an observer's evidence
must cover; E3 also checks §3.1's bound on the adoption graph itself,
independently of the builder's bookkeeping. **Mutation:** the old clique
builder run through the new E3 trips that check ("the visible region is not a
patronage structure §3.1 permits"). The generosity argument survives where it
belongs: the attacker's interior acquaintance edges are presence records the
observer holds none of (16.3.1's conservative direction), and E2 already shows
interior wiring does not move a cut. E2's own region was checked for the same
pattern: a tree at fanout 2-4 at capacity 100, not walked by reach, a bound
test whose over-approximation favours the attacker -- the case the reviewer
explicitly allows -- and left as is.

Figures: independent sums 32/64/104/168 (was 32/64/128/256), joint 4/8/8/8,
cut 8, general demands 8 of 168 (was 8 of 256), regression unchanged at 8 + 8
against 8. The reviewer's 160 is 8 children by 8 plus 24 grandchildren by 4;
mine is 10 by 8 plus 22 by 4 -- the same arithmetic on a different fill, so
the harnesses agree. Both READMEs now say the region is a patronage tree at
§3.1's fanout, and the models README carries the new figures.

**F-02 (LOW) -- two comments described a program that no longer existed.**
Verified: the `expanded` comment said `add_edge` "ACCUMULATES capacity" while
`add_edge` keeps `max`; the E4 preamble said `changed == holds_edge` is
asserted while the docstring and the code say it is not, because it is false.
Both rewritten to say what is asserted. The `expanded` set is what makes
`reach` count shells outward from visible peering endpoints, and that is now
its stated purpose.

**Not acted on.** The reviewer's U-01 (vertex-capacity schedule) and U-02
(how much foreign patronage an evaluator knows) are the modelling parameters
the file already labels as such; U-03 (population split between E4's two
evidence states) is the simulation README's third limitation. Nothing new to
record.

Flow metric clean on seeds 0-34 and 7, 23. Model citations 179 / 0 flags
(two new, both §3.1); references 2037 / 0. Gate: 14 artifacts pass (two
mutations violated as expected, two bounded companions verified), ALL MODELS
PASS; the old clique construction trips E3's new check by mutation.

## TLA+ review (2026-09-08)

Clean-room review of the five TLA+ models. No TLC in the reviewer's
environment; they built enumerators from the transition relations and
reproduced PartitionMerge's 21,032 states exactly, then argued the rest from
the text. Six findings. All six verified here with TLC before anything was
changed, all six applied, **no specification change**, and one thing found
while repairing that the reviewer had not reported.

**TLA-01 (HIGH) -- cycle repair could cut the wrong subordinate.**
`DetectAndRepair` chose any child whose patron is the detector "and which
reaches the detector going up" -- but a direct child reaches its patron in one
hop, so the second conjunct excluded nobody. `Forward` kept no previous
holder. On three nodes each cycle node has one child, so the choice never
existed. Reproduced: four nodes, four adoptions, TLC finds a counterexample to
`CyclesResolve` in 66 s -- both cycle-detecting memos spent on innocent
off-cycle children, the loop standing, the remaining memos circling it
forever under a schedule that satisfies weak fairness. The documents say
which edge: wire 10.2.4, "it disavows the direct subordinate that forwarded
the memo to it"; design 18.2, "the edge severed is the one that handed the
memo over". The model now carries `from` on each memo, `Forward` sets it to
the holder it left, and repair cuts `from` and nothing else. Fairness is
stated per memo id rather than per record (the record set would have grown
to |Nodes|^4 conjuncts). `CycleDetection_FourNodes.cfg` is the regression:
330,301 states, no error, 31 s. `CycleDetection_Mutation.cfg` sets
`CutAnyChild = TRUE` there and TLC reports the temporal violation in 30 s,
the reviewer's shape exactly.

**Found while repairing, not in the review.** A first attempt added a safety
property, "an edge is cut only from a node on a cycle", and TLC refuted it
under the rule in two seconds: a memo that had arrived and not yet fired
outlived its loop -- another memo's repair broke it, the detector was
re-adopted elsewhere, a new loop formed -- and the stale memo's arrival
branch was by then an innocent child. Two conclusions, both applied. The
model carried the memo's stated patron (`pat`) and never compared it with the
detector's row, though wire 10.2 has the detector act "once the memo is
confirmed against its own records"; the guard now requires `m.pat =
patron[m.about]`, and a memo naming a patron the detector no longer has
stops. And the property itself is not the design's: 18.2 accepts that a
replay matching the current row can sever an edge and bounds the damage
rather than preventing it, so no operator asserts it, and the header says
so. The mutation was retargeted to `CyclesResolve`, which is the claim.

**TLA-02 (HIGH) -- the clock stopped and liveness followed.** Verified by
reading and by TLC: a state with `clock = MaxClock` holding an attestation
stamped at `MaxClock` is reachable in eight steps, `Tick` is then disabled,
and that attestation can never expire, so `<>[]Current` held because time had
ended. The reviewer's second point also holds: for unbounded time the
property is false anyway, since weak fairness lets the patron issue only after
each expiry. Remodelled as the reviewer proposed -- the held attestation's
AGE, saturating at LIFETIME, so the space is finite and cyclic (256 states) --
and the property restated as recovery: every expiry is followed by renewal
while a rung is the operative one from some point on, one property per rung
(`LadderMakesProgress`, `SiblingRungServes`, `GrandpatronRungServes`). Per
sibling rather than "some sibling", because two siblings flapping in
alternation keep either from being continuously enabled, which is a real
behaviour the property does not claim.

**TLA-03 (MEDIUM) -- `FreshOnly` checked nothing.** `att.issued <= clock`
held in every state for free; the README credited it with "issue fresh; never
extend stale". The reviewer's remedy was attestation serials. Not required:
the design names the rejected alternative itself -- 12.6.5.1, "the tempting
fix is a grace period -- extend the last attestation while the patron is
verifiably down. Do not." -- so the model now has that action, switched by
`GraceOK`, and `FreshOnly` is the step property that validity is renewed only
by an issuer reachable under its rung in that step. The mutation violates it
on the first extension of a patron-issued attestation. What it cannot
distinguish is said in the header and the README: with no key rotation in the
model, a live issuer re-signing and its old attestation being stretched are
one state.

**TLA-04 (MEDIUM) -- the two-live-answers question was assumed away.**
Verified: one attestation slot, and `rotated` initialised and never written.
The single slot was presented as answering the question. The question is now
stated as not asked, `rotated` is deleted, and the README lists two current
attestations among what the model cannot demonstrate. The top-level README
had never claimed it.

**TLA-05 (MEDIUM) -- the series rule was checked against the wrong
history.** Reproduced in three states: a node records 0, then 2, then takes
1 as current, having never recorded 1, and every invariant stays green because
"superseded" meant "what this node moved off". The documents say what a node
holds: wire 4.6.1, a node proves its series "by presenting its adoption ...
and each series reissue since", and "chain length is the order"; wire 4.6,
the series rule is "checkable by anyone holding the chain, who MUST reject a
reissue naming a series already in it". The model now has the subject's
chain, each node holding some prefix of it as of its last arrival, a node's
record being the tip, and `superseded` accumulating every series a held chain
shows was left. Two constants, two mutations: `SeriesCheck` (the rule, at the
subject's reissue and at the chain holder) and `OrderCheck` (wire 4.6.1's
ordering). Compliant: 108 states, six invariants including that every held
chain is a prefix. `SupersessionDiscipline_Mutation.cfg` (no series rule):
chain 0-1-0 arrives, the record walks back, violated. The new
`SupersessionDiscipline_OrderMutation.cfg` (no ordering): a node holding 0-1
takes 0, and issues for a series only the chain said was left -- the
reviewer's stale-unseen class, now covered and shown load-bearing.

**TLA-06 (LOW).** `NoInvention` checked only the latest transaction per
subject; the stronger form over every held transaction was checked first on
the old model (21,032 states, holds) and is now the invariant. The
`IssuerAuthorisation_Mutation.cfg` comment called `StaleOK = TRUE` the current
reading; corrected.

**Gate.** Section 2 now runs further instances of a module
(`CycleDetection_FourNodes`), and section 2b's entries carry module,
configuration and what TLC must report -- an invariant, an action property or
a temporal violation -- five mutations in all. Both READMEs updated;
`models/README.md` carries the new counts.

**Not acted on.** The reviewer's "recurring-age currency instead of
MaxClock" was adopted; their attestation-serial suggestion was not needed
once the design's own rejected alternative was the mutation.

Gate: 15 artifacts pass (the four-node instance is new), five mutations
violated as expected, two bounded companions verified, ALL MODELS PASS.
Flow metric unchanged. Model citations 189 / 0 flags (ten new); references
2037 / 0.

## Flow-metric review 5 (2026-09-08)

Executed review: seeds 1-10, 1,200 exhaustive min-cut comparisons against
`max_flow()`, and seed 1 under five `PYTHONHASHSEED` values with byte-identical
reports. One material finding, against the design's prose rather than the
model, verified and applied. One informational packaging note, applied.

**F1 (MEDIUM analytical) -- the design claimed that visibility is influence.**
16.3.1: one acquired edge "sits inside every horizon that contains it and
helps each of those observers at once"; 17.3: "one visible edge helps every
observer whose horizon contains it, never only one". 16.3 itself says a
peering record adds nothing to an observer already holding the pair's
presence record, and E4 -- corrected two rounds ago on exactly this point --
reports visible and moved separately: seed 1, visible 9-14 (median 10),
moved 5-10 (median 6), and zero moved wherever the presence record is held.
So the categorical sentence was false in the document that the model was
built to check, and the model had absorbed the correction while the prose
had not. Swept all five root documents and the model READMEs for the same
claim: the two sites only.

Both narrowed to what holds, with a dated tag. 16.3.1 now says visibility
BOUNDS coverage and is not it -- an edge helps only the observers whose
evaluation adding it changes, and one already holding standing to the far
peer, or the presence record, sees it and is not moved -- and "every horizon
containing it" became "several"; 17.3's sentence says the edge is visible to
every such observer and helps those whose evaluation it changes, several at
once rather than one per acquisition. The author's ruling of 2026-09-03
stands in substance: amortisation exists and a per-target framing overstates
the cost. What went was the word "every". The reviewer notes, and the
tracking agrees, that the error ran in the conservative direction for the
security argument -- an attack priced from raw visibility needs more edges,
not fewer -- and would have contaminated later cost modelling, which is why
it is worth a sentence rather than a shrug.

**Informational.** `flow_metric.py` said the committed run was `results.txt`;
it is `models/results/flow_metric.txt`, and the docstring now says so and
names the gate that writes it.

**Not acted on.** The reviewer's three unspecified questions -- what
"coverage" means (now answered by F1), the capacity policy, and how much
foreign hierarchy an evaluator knows -- are, for the latter two, the
modelling parameters the file labels as such.

Flow metric clean on seed 1. References 2038 / 0 flags (one new, the 16.3
citation); model citations 189 / 0.

## Flow-metric review 6 (2026-09-08) -- PASS

Executed review of the committed version (hashes match): seeds 1-30, 1,080
exhaustive min-cut comparisons against `max_flow()`, 12,600 random allocation
cases against the one-shot super-sink flow, ten `PYTHONHASHSEED` values with
byte-identical reports. **No new material finding**, no vacuous headline
experiment. E1-E4 and the 16.4 ordering each confirmed for the property
actually claimed, with the per-computation boundary of E3 read as the
declared limitation it is. The author is treating the family as passed and
moving the other models on against the same version of the documents.

**Two informational notes.** `add_edge` collapsed a pair's repeated
capacities with `max`, which the docstring said decided nothing; the reviewer
asked for an assertion so that nothing later could come to rely on it.
Applied: a pair offered two different capacities now fails, after checking
that `split_graph` drops in-horizon adoption edges before adding the
unthrottled ones, so no pair is ever offered two. Seeds 1-30 clean. The
`UNTHROTTLED` sentinel (10^9) is not semantic infinity; at every scale
exercised it dominates, and deriving a per-graph bound would be machinery
for a stress model that does not exist. Not acted on, and the file already
says what the sentinel stands for.

**Not acted on.** The three unspecified questions are the same three as the
previous two rounds: capacity schedule, evaluator reach, and the E4 evidence
split, all labelled as modelling parameters.

Model citations 189 / 0 flags.

## TLA+ review 2 (2026-09-08)

Clean-room review of the five models. No TLC available to the reviewer; they
rebuilt the transition relations as enumerators and reported state counts.
Those counts agree with this session's TLC runs wherever both exist --
PartitionMerge 21,032, CurrencyEscalation 256, SupersessionDiscipline 108,
CycleDetection 1,621 at three nodes and 330,301 at four -- which is good
evidence they enumerated the same machines. Four findings, **all four
verified with TLC before anything changed, all four applied, no
specification change.** No new defect in CurrencyEscalation,
SupersessionDiscipline or IssuerAuthorisation.

**F1 (HIGH) -- CycleDetection detected from the wrong end of the loop.** The
memo's principal is the patron. design 15.2: "A memo is a patron's statement
about one of its own subordinate slots: it names the patron, the patron's
position, which slot, when, and who is in it." wire 10.2.1: "field 1 names
the patron it speaks for. If field 1 is you, a memo you originated has come
back to you from below." design 18.2 says the same of the replay case, a
captured memo "matches that patron's own row". The model named the OCCUPANT,
fired when the memo reached the adopted child, and confirmed by asking
whether that child's own patron pointer still agreed. Both constructions cut
an edge of the loop, from opposite ends, which is why the two-node cycle in
the old three-node instance never showed it. Rebuilt: the memo carries `pat`
(field 1) and `occ`, originates at the adopter's own patron -- "a receiving
node forwards the memo to its own patron", and a rootless adopter therefore
originates no travelling memo, which costs nothing because the adoption that
CLOSES a cycle always has a patron chain leading back -- and fires where
field 1 comes home.

**Found while fixing, not in the review.** The guard also required
`OnCycle`, that the loop genuinely exists. No node in a loop can see the
loop; that is why the memo exists. It is a check 1.1 puts beyond the party
asked to make it, and it was doing real work in the model -- suppressing
exactly design 18.2's accepted case, "a replayed rootward memo can cost one
edge, without prejudice". Dropped, leaving the row check the documents
actually specify. The accepted case is now reachable rather than assumed
away, and CyclesResolve still holds.

**F2 (HIGH) -- PartitionMerge scalarised the binding, and 6.2.1 says so.**
"a node that adopts elsewhere remains in the old subtree's view
indefinitely, since ADOPTION SAYS NOTHING ABOUT EXISTING BINDINGS", and
moving is "adopt at the destination, depart the origin, in either order,
with no requirement to do both" (6.2). The model overwrote one scalar per
adoption, so after adopting at the destination there was no origin left to
depart -- the very order the file's own comment claimed to have been relaxed
to cover. Rebuilt around a SET of bindings per node, transactions naming one
relationship, departures naming the relationship they end, and ordering
within a relationship only, which wire 2.3 requires: "one series per patron
relationship ... a node bound under two patrons keeps two", cross-series
unrankable per P36. No action names the set, because 6.2 says the protocol
has no concept of one. 21,032 to 21,416 states, all three properties hold.

**F3 (MEDIUM) -- the convergence antecedent was half unstated.** `<>[](healed)
=> <>[]Agreed` does not hold for an indefinitely active topology: each update
propagates under fair gossip while the next is signed before the last has
arrived, so there need never be a point after which all views agree forever.
MaxEvents supplied the missing half in silence. The property now conjoins
`<>[][events' = events]_vars` -- topology changes eventually stop -- which
TLC accepts and which is true in every execution of this instance. That is
the point: what is checked is convergence UNDER quiescence, and the cfg now
says the budget is what produces it.

**F4 (MEDIUM) -- the disavowal blacklist outlived the protocol's.** `Adopt`
refused any pair ever cut, in both orientations, forever. Reason 5 is
"without prejudice" (wire 10.2.4) and design 18.2 bounds the replay attack
partly ON re-adoption remaining available. Removed; `disavowed` is now a
record of what was cut and gates nothing.

**Anti-vacuity, because three of these changes shrink or reshape the space.**
CycleDetection's three-node instance fell 1,621 to 370 states -- fewer memos
travel, since a rootless adopter originates none -- so cycle reachability was
checked directly rather than inferred: cycles form at three nodes and at
four, and repairs fire at four. In PartitionMerge, two simultaneous bindings
are reachable, the reviewer's exact transfer trace runs (b under a, then b
under a and c, then b departs a leaving c), and disagreement is reachable, so
`<>[]Agreed` is a real requirement.

**Hardening note, not acted on as code.** Every unpartitioned pair in
PartitionMerge can exchange frames directly, so no relay or chokepoint
reconciliation failure is reachable. Recorded as a declared limitation in
`models/tla/README.md` rather than modelled, along with the patron-row /
child-binding divergence CycleDetection cannot show and the memo timestamp
rule it does not carry.

**Not modelled, deliberately.** The reviewer asked for the full memo tuple.
Position stays out because the cycle check is an identity comparison with
"no path arithmetic ... deliberately" (wire 10.2.1); the timestamp stays out
because it serves the forwarding rule that stops replays, which this model
does not carry and now says so.

Figures: CycleDetection 370 (3 nodes), 35,777 (4 nodes), mutation violates
CyclesResolve at 50,897 states. PartitionMerge 21,416. Model citations
192 / 0 flags (three new); references 2038 / 0.

## Tamarin wire-only review (2026-09-08)

Clean-room review of the four wire-only theories. No Tamarin available to the
reviewer; static source-to-spec comparison only, with file hashes reported --
all four match the committed files. Two findings, both HIGH, both in
`recovery`, both the same class: **a value the wire requires a validator to
COMPARE was carried in one variable on both sides, so the comparison held by
unification and no mutation could remove it.** Both verified against the wire
text, both applied, **no specification change.** No new defect in `attach`,
`ceremony` or `currency`.

**The class matters more than the two instances.** This suite has fixed it
four times now -- `claimKh`, `priorKh`, the response result, the second
response's subject -- and each time the fix was local. The reviewer's
contribution is noticing the same shape in the fields that bind a recovery or
transfer to its ENCLOSING ADOPTION, which is where the wire is most explicit
about it.

**F1 (HIGH) -- assembly bindings were unification.** wire 4.1 on Recovery
field 3: "A verifier MUST check `new_key` and `patron_key` against adoption
fields 1 and 2 and reject on mismatch. An unchecked binding is the same as no
binding." On Transfer: "A verifier MUST check all three against the enclosing
adoption -- `node_key` against field 1, `former_patron_key` against the
`Transfer` map's own field 1, `new_patron_key` against field 2 -- and reject
on mismatch. The same sentence as above, and for the same reason." The model
verified the successor statement over a body REBUILT from the accepting
patron's own name and the adoption's key, and the transfer statement likewise.
That is a sound validator -- one of the conforming implementations -- but it
is not the one the wire's separate MUST exists to catch, which verifies what
the object says and then compares. The other implementation's error was not
representable.

Rebuilt: the adoption's field 1 enters both recovery rules as its own term
(`adoptNew`), the successor statement carries `stmtNew`/`stmtPat`, the
transfer object carries the map's field 1 and the statement's three values
separately, signatures are verified over what the object says, and acceptance
then compares. The transfer's map field 1 is also tied to the identity whose
key the validator fetched -- the fetch step, not a fourth MUST, and commented
as such.

**F2 (HIGH) -- the first response's subject was assumed.** wire 4.1: "Every
response's `subject` MUST equal the newly adopted node" (field 1). The
two-response rule got this right for response 2 -- independent `newkey2`,
explicit `Eq` -- and parsed response 1 straight into the enclosing successor.
The one-response rule, which the wire's nonempty array makes a real shape, did
the same. So a correctly signed response about successor K1 could not be
transplanted under a recovery for K2, not because the model rejected it but
because the model could not write it down. The reviewer's point about the
action is exact: `ReliedOnResponse` was emitted after unification, so it
recorded the desired conclusion. Both responses now carry their own subject
and both are compared against `adoptNew`, which is also what the action now
records.

**Mutations, which is what makes this different from a comment.** The gate
grew a section 3c: each entry replaces ONE comparison with a tautology of the
same shape, leaving every signature valid, and the named lemma must falsify.
Replacing rather than deleting the line is deliberate -- an earlier mutation
script left a dangling comma, which Tamarin rejects, and a rejected theory
reads from outside exactly like a mutation that worked. The substitution is
literal and the gate fails if the pattern is absent, for the same reason.
Three mutations: the patron comparison, the first response's subject, the
transfer destination.

Recovery after the rebuild: 14 lemmas verified, wellformedness clean.
`recognition_binds_the_successor` now takes 1,458 proof steps, having been
nearly free when the binding was syntactic. Each mutation falsifies its lemma
against a WELL-FORMED mutant -- checked, because a theory Tamarin rejects
reports much like one a mutation broke: the patron comparison in 26 steps, the
first response's subject in 19, the transfer destination in 9.

**Not acted on.** The reviewer's UNSPECIFIED question -- whether an upstream
parser is assumed to have reconciled the object with its adoption -- is
answered no, and the wire-only README now says so rather than leaving it to
inference. The already-declared limitations they list (multi-witness, body
projection, query_id and consent, elapsed time, TLS agreement, current issuer
authorisation) are unchanged.

## Tamarin compliant review (2026-09-08)

Clean-room review of the four compliant-tree theories. No Tamarin available to
the reviewer; static comparison against the requirements and the design. Four
findings -- two HIGH, two MEDIUM -- **all four verified on the text, all four
applied, no specification change.** No new defect in `currency`. The reviewer
also noted they were not sent `run-all.sh` and so could not see how the
`.bounded` fragments are spliced; the compliant README now says.

**F1 (HIGH) -- 0-RTT deferral was not bound to the connection.** wire 9.1
says defer the `Attach` "until handshake completion", and the handshake meant
is the one on the connection carrying the early data -- that is what makes
deferral a replay defence, since a replayed first flight opens a new
connection its replayer cannot complete. The model's `Handshaken(N, C)` token
carried no connection and was minted by a rule with no premises, so a
conforming server could bind early data off any completed handshake with that
client, one from before the data existed included. The linear token limited
it to one binding per handshake: one per EVENT, where the rule is one per
CONNECTION. Now a connection is a fresh `~conn` the client opens; early data
travels on it; the handshake completes on it or never; the conforming server
binds only with that connection's own token; and a `Replay` rule puts the
same early data on a connection nobody opened, so no handshake ever completes
there. Two harm witnesses stay reachable for the non-conforming server: the
double bind, and a bind off a connection that never completes. The carve-out
is scoped to N (F3).

**F2 (HIGH) -- the queue had lost the credential it was queued for.** design
14.1.6: "Queue metadata is the minimum: ciphertext, recipient keyhash,
arrival time" -- the recipient keyhash is one of the three things a queued
item IS. infra-client: "deliver nothing further to it, its queue included",
and "it" is the credential. The model's `Queued(N, C, m)` carried no
credential, and `Deliver` combined an entry with whichever credential was
serving that client: queued under k1, k1 superseded, k2 attached, delivered
labelled k2 -- which the supersession lemma, asking about k1, never saw. The
queue now carries the credential and `Deliver` requires the match. A new
unbounded lemma, delivered under k means queued for k (3 steps -- `Queued`
has one origin and no restore, so no regress), and a bounded regression
naming the reviewer's exact retag (47 steps).

**F3 (MEDIUM) -- "conforming" was universal over the trace.** The carve-outs
read `not(Ex P #s. SkippedTheMeeting(P) @ #s)`: nobody anywhere ever skipped.
So an unrelated party fabricating a record elsewhere made the theorem say
nothing about P1 and P2's record, and one lying verifier in an unrelated
recovery silenced the theorem about this one. That proves "if every actor
conforms then ...", where the requirements are commitments each client makes
for itself and the README already said "among participants who keep" and "a
server that defers". Scoped to the principals each property names --
ceremony's two participants (four, for the roster lemma), the verifier the
patron relied on via `ReliedOn`, the server N -- which is the STRONGER
statement, and every one of them still verifies. The nomination lemma had this
shape from the start, as the reviewer noticed.

**F4 (MEDIUM) -- the thief regression was vacuous.** `thief_wins_against_a_
chainless_counterparty` asked only that some seal and some chainless
acceptance both occurred -- no shared line, no order, and the accepted record
was any `<'record', S, c>` off the network. Verified the reviewer's claim
directly: with `Thief_Signs` deleted the old form would still be satisfiable.
Now the thief's record is a fact with the thief's provenance, the chainless
counterparty has two rules (the subject's record or the thief's, whichever
reaches it), and the regression says: sealed first, thief signed in that line
afterwards, chainless party took the thief's. **Negative control run**: delete
`Thief_Signs` and the lemma reports no trace found, the only wellformedness
warning being that `!ThiefRecord` then has no producer, which is the point.

**Mutations.** Two more in the gate's section 3c, now "theory mutations"
rather than wire-only: the conforming server binds off any of C's handshakes
(`connT`), and delivery ignores the queued credential (`kq`). Both falsify
against well-formed mutants, in 5 and 6 steps.

Figures: ceremony 8/8, recovery 8/8, attach 10/10 unbounded, bounded attach
13/13 with the two original supersession lemmas at 84 and 46 steps. The
thief regression verifies in 5 steps; deleting its producer, no trace.
Gate: 15 artifacts pass, 5 TLA+ mutations violate, 5 theory mutations
falsify, ALL MODELS PASS. Model citations 193 / 0 flags (one new);
references 2038 / 0.

## Flow-metric review 7 (2026-09-08) -- regression run

Executed review: seeds 1-10, all assertions passing, the 8 + 8 regression
reproduced. One finding, HIGH, the lifecycle question of review 3 returning
with a named consumer: the resource role table materialises rows, re-scores a
changed member alone, and retains the result, which the reviewer read as the
consumer that spends one cut repeatedly and as a contradiction of the 16.2
ruling that nothing is a retained entitlement. **Verified against the
documents; does not hold against the consumer named. Disposed by ruling,
with one sentence added to the design.**

**Why it does not hold.** The table is "one row per Dunbar Org member per
resource" (resource-requirements 7.2.1, infra 10.2); resource 7.1.1 makes
membership "the gate every other predicate sits behind. No grant of any kind
reaches outside it"; design 11.4, "No scope reaches outside the owner's
Dunbar Org". The Dunbar Org is the horizon, and 16.2.1 says inside it "there
is nothing for a flow bound to ration". So the reviewer's step 1 -- a region
behind a cut of 8, scored by this evaluator -- is excluded three times over:
an identity behind a cut is outside the horizon and has no row. The retained
rows are real and the shape is the 8 + 8 shape, and there is no cut for them
to spend. Nor does the table consume the bounded quantity at all: the metric
"reaches past the horizon rather than grading inside it", so "my ten most
trusted" ranks by 16.1's per-node computation. Swept the rest for any retained
consequence of standing scored past a cut: membership is the only thing that
turns outside into inside, and 6.1.1 prices it in presence per identity, which
17.3 calls the one resource an attacker cannot parallelise. 16.4 already names
a consumer whose policy ignores the bound as the attack surface, deliberately.

**Ruling: state the scope** [author, 2026-09-08], the reviewer's own third
option made precise. 16.2 now says the bound is inherited only by a consumer
that scores identities past a cut in one computation; that 11.4's table scores
Org members inside the horizon and neither inherits nor needs it; that
membership is priced in presence, not flow; and that a consumer retaining
standing scored past a cut would be 16.4's attack surface, not this rule's.
The simulation README's lifecycle limitation now points at that sentence, so
the next reader of the regression does not take the role table for the policy
it exhibits. The reviewer's other two options -- whole-table conserving
refresh, persistent residual accounting -- were put to the author and not
taken: both build a mechanism for a population the metric does not ration.

**Why review 3's ruling stands.** It said standing is computed on demand and
nothing is retained between evaluations. The table retains authorisation
decisions, not standing, and inside the horizon those decisions do not draw on
the bounded quantity. The reviewer's point about the 16.2 sentence was fair as
read -- it did not say which consumers it spoke of, and after review 3
removed the 11.4 pointer nothing connected the two -- and that is what the
added sentence does.

Not a register entry: not a weakness. Model unchanged; gate not re-run.
References 2044 / 0 flags (six new, all in the sentence); model citations
193 / 0.

## Tamarin wire-only review 2 (2026-09-08)

Static review of the four wire-only theories against the wire document alone;
no Tamarin in the reviewer's environment. Two findings, MEDIUM and LOW, both
MODEL GAP, both verified on the wire, **both disposed in the claim rather than
the model**, and one conditional question answered on the wire's own words. No
system flaw.

**F1 (MEDIUM) -- "every response is validated" read wider than the theory.**
`result` is a free term in `vresp`; the wire's field 4 is a four-value
enumeration and §1 makes an unknown value in a known enumeration malformed.
Fields 5 and 6 (`basis`, `template_version`) are required or forbidden by the
result and basis values and are absent from the projection. So a block whose
second response carries `'bogus'` is admitted here and rejected by a decoder,
and the README's sentence claimed the decoder's checks. Verified. **Not
modelled, deliberately**: a free result gives the attacker more traces than a
decoder would, which strengthens every all-traces lemma rather than weakening
one, and a restriction pinning the enumeration would be a decoder's
precondition restated as an axiom -- the shape this suite removes. The README
now says each response receives "the checks this theory represents", lists
them, and lists what a decoder rejects and the theory does not; the theory's
collection comment carries the same list.

**F2 (LOW) -- response ordering is not represented.** The Recovery block sorts
responses by verifier keyhash, ties by subject keyhash, "one set, one
encoding". Two symbolic slots carry no order. Verified; added to the same
boundary statement. Modelling lexicographic SHA-256 order symbolically would
be machinery for a claim this theory does not make.

**The currency question.** `!Epoch` is persistent, so a current (iat, exp)
pair can be stamped twice; the reviewer asked whether "issue fresh" forbids
that and did not count it. The wire answers: "attestations are issued fresh,
never extended stale. There is no 'extend' operation and no field for one."
Two attestations over one window are equivalent to one and extend nothing.
Recorded in the theory's comment so the next reviewer need not ask.

**Discipline note.** I first cited the wire's "no extend operation" sentence
as §4.7; §4 ends at 4.6. Both new citations were re-derived from the headings
that actually contain the cited lines before the checker ran.

Model unchanged in substance; both edited theories parse well-formed. Gate not
re-run. Model citations 194 / 0 flags (one new); references unchanged.

## Tamarin compliant review 2 (2026-09-08, closed 2026-09-09)

Static review of the four compliant-tree theories; no Tamarin in the reviewer's
environment. Six findings -- one HIGH, four MEDIUM, one LOW-MEDIUM -- **all six
verified on the theories and the design, all six applied, no specification
change**, plus three unspecified questions, two answered on the documents and
one recorded as not established. No system flaw. `currency` unchanged in
substance; one of its comments corrected.

**F1 (HIGH) -- one attach per binding, for ever.** `OneServingStatePerBinding`
said two `Opened(N, C, k)` events are the same event: not one serving state
at a time but one attach in the trace's whole history. design 14.1.2 has a
client stay "on the sibling until that session ends, and the next fresh
attach tries its actual" patron, so reconnection is the ordinary lifecycle
and the model forbade it outright. Worse, it made the property under test
true by fiat: a node that ended every session and then FORGOT the credential
was superseded would serve a reconnect, and no trace could show it because no
reconnect was allowed. Rebuilt with two facts, because the design has two
things: the node's RECORD of the binding, entered once and retired by
supersession, and the SESSIONS under it, as many as the client opens, each
with its own role and its own end. Service needs both; supersession retires
the record, so every session is inert at once and no attach can follow. The
record is entered once by restriction, and that is not the property restated:
`currency` proves the same of its record as a lemma, and what keeps a
superseded binding from being served is the linear record being gone, which
the mutation shows -- supersession that keeps the record is the forgetful
node, and `no_attach_after_supersession` falsifies against it in 7 steps.
The role gate is now per session, the reconnection is a reachable witness,
and the two prior mutations (0-RTT `connT`, queue `kq`) still apply to the
rebuilt rules.

**Found while fixing.** `no_attach_after_supersession` does not discharge
unbounded: an attach consumes the record and restores it, so the search
regresses through prior attaches exactly as the two supersession obligations
regress through services, and `[use_induction]` did not close it in 300 s.
It lives in `attach.bounded` beside them, and the fragment now bounds attaches
at two -- the shape of the failure guarded against, attach, supersede, attach.
Diagnosed per lemma after the whole-theory run timed out at 900 s with nothing
reported: the source lemma verifies in 15 steps and the reconnection witness
in 8, so the hang was isolated before anything was moved.

**F2 (MEDIUM) -- the directional nomination rule was an axiom.** `Nominate`
checked only the counterparty's neighbourhood and asserted `NotInOwn`; the
restriction forbade the own-neighbourhood case anywhere in the trace; the
lemma restated the restriction. Verified: with the restriction removed, a
three-step counterexample. Non-membership cannot be a premise, so the check
is now the semantics of the conforming rule's action -- no neighbour record
EXISTED AT THE TIME, time-scoped, which answers U3 -- with a
`Nominate_Without_Checking` twin, the lemma carved out by name like the
others in the tree, and an exists-trace showing the own-neighbour nominee
reachable without the check. The obligation IS a check and nothing deeper,
and the file now says so rather than dressing the check as a theorem.

**F3 (MEDIUM) -- the replay witnesses did not require the replay.** Both
0-RTT harm lemmas were satisfiable by binding the persistent early data twice
on the ORIGINAL connection, so losing the `Replay` transition would have left
them green. Both now name `SentEarly` on one connection and `Replayed` on
another, distinct, and bind on the replayed one. 8 and 5 steps.

**F4 (MEDIUM) -- "reissue first, seal later" had no later.** `Reissue_Then_
Seal` consumed the open line and left nothing to seal, so the witness showed
reissue-then-never-seal, weaker than the requirement's "before", which is
about a race. The line now stays open after an out-of-order reissue,
`Seal_After_Reissue` closes it at the maximum, and the witness names all
three in order: reissue, thief supersedes, seal arrives too late. 6 steps.

**F5 (MEDIUM) -- one witness cannot show partial roster validation.** The
record carried one witness attributed to the signer, so a signer that
validated one element and authorised the collection was indistinguishable
from one that validated all. Two witnesses now, distinct by restriction,
both attributed to the signer; the conforming rule checks both; the gate
mutation checks the first twice and the lemma falsifies for the second in
6 steps. The record remains a projection: the counterparty's own
attributions are not carried.

**F6 (LOW-MEDIUM) -- negative controls without the wire's inequalities.** The
collusion witness allowed one party in both chairs; the lying-verifier
witness allowed the subject to verify itself. Both now carry the wire's
distinctness (participants differ; verifier and patron differ from the
subject), so each witness is a record or recovery the design would accept.

**U1 -- key freshness was justified by the wrong rule.** `currency` mints the
successor fresh and its comment leaned on the series rule, which is about
series identifiers and leaves the keyhash untouched (wire 4.6). The ground
is design 9.1 step 3, the rotation binding the old identity "to a new
keypair"; the restriction is renamed for what it is and the README lists it
as a labelled assumption. **U2** -- whether a node must refuse to enqueue for
a known-superseded binding -- is not stated in the requirements and is now
listed as not established. **U3** answered by F2's time-scoping.

Figures: attach 11/11 unbounded; ceremony 9/9 (one new witness); recovery
8/8; currency 5/5. Bounded attach 15/15, the four obligations at 4,316,
2,573, 1,696 and 2,574 steps -- up from double digits, since attaches are
now bounded rather than forbidden and sessions are their own facts.
Mutations: memory 7 steps, roster 6 steps, both against well-formed
mutants. Model citations 195 / 0 flags (two new).

## Gate run and prover fence (2026-09-09)

The gate launched at 00:08 on 2026-09-09 after compliant review 2 never
completed: it was killed two seconds in, when the session that launched it
was closed, and left `results/PartitionMerge.txt` truncated mid-run. The
several-minute lock-up that preceded the restart was not the gate. Between
23:44 and 00:00 two non-terminating searches ran side by side -- the
unbounded attach theory and its bounded companion, before
`no_attach_after_supersession` was moved into the fragment -- each on the
prover's default of every core with no heap ceiling. The journal shows
memory pressure at 23:54 and journald killed at 00:00:07; both searches
reached their 900 s timeouts afterwards. A runaway search fills memory
faster than it spends wall clock, so the timeout was no protection.

`run-all.sh` now fences every prover call: `+RTS -N8 -M64g -RTS` under
`nice -n 19`, overridable through `TAMARIN_RTS`. Measured before applying:
8 cores cost nothing (bounded attach 76.5 s against 76.6 s on 32; peak
footprint 4.9 GB), and a 300 MB cap turned the same run into a clean
"Heap exhausted" exit in 19 s, which the no-verdict branches report.

Gate re-run clean at 00:27, 1,299 s end to end, rc 0: 5 TLC models and
the four-node instance, 5 TLC mutations violated, 8 Tamarin theories
(wire-only 7/7/14/6, compliant 5/11/9/8), bounded currency 6, bounded
attach 15, 7 theory mutations falsified. Every wire-only time fell on 8
cores against the 32-core figures committed (recovery 290 s from 307,
ceremony 160 s from 186, the two slow recovery mutants 371 s and 293 s
from 409 and 329). The compliant tree is 90 s of the total; the bounded
attach companion is 73 s of that and stays in the gate.

## Acceptance suite (2026-09-09)

`rhtn/` is now in the tree as the workspace `Robot/implementation-plan.md`
lays out, with one member: the acceptance catalogue, its checker and the
stubs generated from it. `rhtn/acceptance/acceptance.json` holds 253 entries
across the sixteen gap rows of the plan's section 8.2 (decoder 16, transport
16, session 15, queue 16, archive 16, topology 16, resolution 16,
replication-peering 15, propagation 15, currency 13, metric 8, ceremony 29,
recovery 14, payload 15, resources 24, product 9). Every citation names a
heading and every rule quote is found verbatim in its section; 51 entries
carry an interpretation, each a reading the specification does not state
and therefore a question for the author. `rhtn/check.sh` passes: catalogue
0 flags, stubs in sync, 253 ignored stubs compiled. One duplicate dropped at
assembly (the session fragment's replay entry, asserted by TRN-16).

Drafting was five parallel passes, then single-thread after two usage-limit
cutoffs. Each pass's list of thin bases, inconsistencies and author
questions is in `Robot/acceptance-drafting-notes/`; the items below are the
ones that touch the specification's text and want a disposition.

**Inconsistencies found while drafting, none applied:**

1. `wire-format.md` §8.1.1's heading says greasing is required; its body and
   design §14.1.1 say sending is not a MUST. The same section says "one" and
   then "at least one" greased parameter.
2. `wire-format.md` §9.2's stream-0 list omits the topology frames §8.0 puts
   there.
3. design §14.1.2 "marks the client unreachable and begins queuing" against
   `wire-format.md` §8.2 "Queuing is continuous — there is no 'begin queuing'
   signal".
4. The mailbox is "the direct patron" in design §14.1.6 and infra §2, and the
   serving node in design §14.1.2 item 1 and item 3; they differ when the
   patron is a light client.
5. design §14.1.2 says the heartbeat interval is unset without the bounds
   §21.1 and `wire-format.md` §8.2 fix.
6. infra §2 and design §12.6.3 cite §14.1.4 for queuing rules that live in
   §14.1.6.
7. design §10.0 and `light-client-requirements.md` §2 cite §10.2 for the
   730-day pruning floor, which is stated in §10.1. The heading exists, so
   `refcheck.py` cannot see it.
8. design §10.1 leaves archive scope across bindings unstated while
   `wire-format.md` §7.9 addresses one chain per key.
9. A disavowal carries no subject counter, so its order against later records
   in the same relationship is decided nowhere.
10. `wire-format.md` §3.1: "The bound in §1 — and MUST verify every
    back-pointer present" has lost its verb.
11. design §3.4's replication floor says messages replicate to siblings;
    design §14.1.6 and infra §2 say siblings hold no queue state.
12. infra §4.3 "Replace an endpoint set when you receive a locator": a locator
    carries no endpoints (`wire-format.md` §2.3, §7.6).
13. infra §4.3 "Collapse forwarding chains" survives the withdrawal of
    forwarding (infra §4.2, `wire-format.md` §7.7.3, design §12.3).
14. `wire-format.md` §7.7.3 says "Failure codes for field 5" where the schema
    puts them in field 4.
15. design §12.6.5's inline attestation sketch lacks the issuer role and
    issuer identity `wire-format.md` §7.1 carries and §12.6.5.1 depends on.
16. `light-client-requirements.md` §1.3 has the verifier prefer the most recent
    capture "when answering"; design §7.5.2.8 and `wire-format.md` §7.3 have
    the subject's grant choose.
17. Cosmetic: design §12.3's steps run 6, 7, 9; design §12.7 opens a
    parenthesis it does not close; `wire-format.md` §7.7.3 spells one bound
    ">= 1" and "≥ 1".

**Questions the entries carry as interpretations, by area:** decoder, a
nesting-depth ceiling and whether unknown keys count per map or per object;
transport, what a field-1 mismatch and an over-bound frame look like on the
wire, and whether "until handshake completion" names the carrying
connection's handshake; session and queue, whether a successor collects its
predecessor's queued mail, whether marking unreachable ends the session,
whether offline-versus-no-record reaches the sender, the cap's unit, and what
carries reachability state between siblings; archive and topology, one chain
per key or per binding, over-capacity adoptions in the table, and what a
failed presence-reference evaluation does to a binding; resolution, the code
after child removal and the handling of an unranked different-series
locator; payload, the shape of a rate-limited reply, the exhaustion notice's
carrier and the demultiplexing scheme; ceremony, the subject-side profile
refusal and the form of the client's notices; resources, a second request on
one stream. The drafting-notes files carry each with its entry id.

**Dispositions (2026-09-09).** Author: greasing is expected client behaviour
and not an enforceable part of the wire (1, heading kept, the two sentences
now agree on one parameter); add the frames to §9.2 (2, applied); clear up
the rest where the documents settle it. Applied from the documents: 3 and 4
(the serving node holds the queue, per design §14.1.2's opening and design
§12.6.3; marking unreachable starts nothing, per `wire-format.md` §8.2), 5,
6 (infra §2 cites both sections), 7, 10, 12, 13, 14, 15, 16, 17. The
generator's TR9 and TR10 citations are corrected in `generate.py`; the
vectors are not regenerated — the pins were already stale at HEAD for the
design and the wire, and the generator's dependency is absent on this
machine. **Still the author's:** 8 (one chain per key or per binding), 9
(ordering a disavowal within a relationship), 11 (design §3.4's floor says
messages replicate to siblings; design §14.1.6 and infra §2 say siblings hold
no queue state). Nine catalogue quotes were re-pointed at the edited text and
verify; the whole set is at 0 flags. `change-log.md` carries the reasoning.

**Dispositions, second round (2026-09-09, author).** 8: design §10.1 had not
been updated for seqno series and reissues; it now states one chain per key
spanning every binding, partitioned by one series per patron relationship,
with the reissue as checkpoint. 9: the patron orders its own slot by its own
clock and keeps its own record of the relationship's status; stated in design
§6.2.2 with the encoding note in `wire-format.md` §4.3, and §18.5's
cross-signer case is unchanged. 11: siblings replicate topology and
trust-bearing transaction history and never queue state; design §3.4's bullet
and floor now say so, and §14.1.6 stands. Three catalogue entries added
(ARC-17, TOP-17, REP-16); two quotes re-pointed; 256 entries, 0 flags.

**Upline queuing** (2026-09-09, author): not v1. Recorded at design §23.1 with
the intended shape — probe siblings under the same grandpatron or let the
grandpatron and above queue, behind a second queue partition for non-local
traffic — and the reason a mail queue is rejected. Implementation proceeds;
the author expects classes of issue that only building will show.


## Milestone 1: codec and crypto (2026-09-09)

`rhtn-codec` and `rhtn-crypto` are in the workspace, seeded from the
conformance runner. The corpus test agrees on 153 bytes-class fixtures and
the two cross-entry bindings, against the runner's 149: frames and replies
are now validated by family, with key tables extracted from the wire's own
schema blocks, rather than skipped. All sixteen decoder entries are
implemented and marked; the catalogue reports 16 of 256 implemented, 0
flags, and the code gate passes. DEC-05's 18-signer record and DEC-06's three
at-ceiling objects are constructed with real hybrid signatures from the
test-vector recipe, so the derived 36-entry ceiling and the two 32-response
bounds are exercised on both sides.

**Design choices worth knowing.** The parser is iterative with an explicit
stack, so nesting is bounded by input length and never by the machine stack;
a 1024-deep extension value is accepted and a hostile 64 KB of nesting
cannot end the process. Every declared length is checked before use and no
allocation follows a declared size, which is how DEC-03 returns at once. The
envelope map admits no unknown keys, since it sits outside every signature
(DEC-09's reading, applied). Extension bounds are counted per map for the
body and an adoption's Locator; other nested signed maps are not yet
counted.

**Not done, deliberately.** The EndpointRecord and
SignedLocator fixtures carry signatures the corpus itself marks stale by
design, so those kinds are checked structurally in the corpus test; the
verification path for them exists in `rhtn-crypto` and is unexercised by a
fixture. Entry order within a `COSE_Sign` array is not checked. The
per-fixture reply families are assigned by fixture id in the test driver,
because the corpus does not say which family a `reply` entry belongs to.

**Coverage-guided fuzzing added (2026-09-09, author's go-ahead).** A minimal
nightly toolchain and `cargo-fuzz` 0.13.2 are installed; `codec/fuzz/` carries
five libFuzzer targets (CBOR, body, envelope, control frame, request frame),
each asserting the DEC-02 property that anything accepted re-encodes to
itself, seeded from the 171 accepted fixture spans. The code gate runs each
target for a bounded time under a 512 MB memory cap, which is the memory
budget the decoder entries parameterise, observed rather than assumed; the
step reports itself skipped where the toolchain is absent. The seeded runs
stay in the gate as the entries specify.


**Fifty-five-minute seeded run (2026-09-09).** Seed 0x18d3bbcee178a3a3,
budget 3,300 s: 6,159,172 inputs, 660,751 reported valid and every one of
those round-tripped, no panic, none over the one-second budget.

## Milestone 2: transport (2026-09-09, in progress)

`rhtn-transport` is in the workspace on quinn 0.11.11 and rustls 0.23.44
with the aws-lc-rs provider. The three third-party facts design §5.2 rests
on hold in these releases and are now exercised by tests: rustls offers
`X25519MLKEM768` as a configurable sole group; RFC 7250 raw public keys
work in both directions through `AlwaysResolvesServerRawPublicKeys`,
`AlwaysResolvesClientRawPublicKeys` and `verify_tls13_signature_with_raw_key`;
quinn exposes 0-RTT (`into_0rtt`), address rebinding and the peer's raw key.
TRN-01, TRN-02, TRN-03 and TRN-05 are implemented over two loopback
endpoints: the profile's client completes nothing against a classical-only
server and the profile's server nothing with a classical-only client, a
valid Ed25519 key that is not the pinned classical member fails the dial,
and `rhtn/1` negotiates with mutual raw-key authentication. TRN-01 and
TRN-02 are observed from handshake outcomes rather than from a captured
ClientHello, which the test file says; the inference is exact because a
single-group peer completes a handshake only with a peer offering that
group.

**The fuzzer's first finding (2026-09-09).** The coverage-guided smoke on
the raw-CBOR target caught a stack overflow on a 56 KB input of nested
one-element arrays. The parser is iterative, but the derived destructor for
the item tree recursed once per nesting level, and the frame layer cloned
the body item recursively as well; DEC-12's 1024-deep case was too shallow
to show it and the seeded mutator never built the shape. Fixed with an
iterative destructor that drains children into a worklist, a frame parser
that moves the body out rather than cloning it, and a regression test at
100,000 levels through both the parser and the frame layer. The crash
artifact replays clean. The commit that added the session layer went out
with the gate red because its commit chain tested the wrong exit status;
this commit's chain tests the gate's verdict.

**Milestone 2 closed (2026-09-09).** The stream-0 session is implemented in
`rhtn-transport` and 22 of the milestone's 23 entries pass over loopback:
TRN-01 to TRN-15 and SES-01 to SES-05, SES-13 and SES-15; 38 of 256 in all.
SES-15 shows a reattach whose Attach rode 0-RTT early data, accepted by the
server, acknowledged only after that connection's handshake completed; the
node reads nothing before the handshake, so the deferral holds by
construction. TRN-15 shows a client rebinding to a new socket mid-session
with heartbeats continuing both ways, an item enqueued after the move
delivered on the same connection, and no second Attach. TRN-16 stays owed:
its replay half needs early-data packets captured and replayed onto a
second connection whose handshake never completes, which the loopback
harness cannot do; the deferral half is SES-15. Two readings the code
takes, both recorded in it: an Attach whose field 1 is not the
authenticated identity is closed with the refusal code 1 and no frame,
since the wire assigns no other code (the open question from the transport
notes); and a known control frame before Attach on the node side is
answered the same way. The corpus derives its named capability id from the
bare name `max-archive-batch` where `wire-format.md` §8.1's example is
namespaced; the id rule is what is normative, and the generator's name is
noted here rather than changed.

**TRN-16 and the replay harness (2026-09-09, author agreed).** The entry's
replay half moves to milestone 4, where `rhtn-sim` gets a datagram-level
path harness for loss, delay, replay and blackholing; that harness captures a
client's 0-RTT first flight and replays it as a second connection, closes
TRN-16, and replaces the frame filters the session tests currently stand on
the path with. Recorded as a third-party fact the design now leans on:
rustls 0.23.44's TLS 1.3 resumption calls `take` on the server session
store, whose contract is that a taken ticket is deleted, so tickets are
single-use and a replayed first flight presents a consumed ticket, fails
the PSK lookup and falls back to a full handshake with its early data never
decrypted. The session layer's deferral (SES-15) is the second line behind
that, and the only line if a deployment switches to stateless tickets. The
plan's milestone 4 and its risk list now say so.


**Milestone 3 closed (2026-09-09).** `rhtn-archive` is new: transaction
builders with their back-pointers, one key's archive as a Merkle DAG with
heads, merges, serving in batches and pruning at a checkpoint, the backward
walk with its five verdicts, batch verification, the local topology table,
subtree acknowledgements, recovery replacement, the patron's archive
evaluation and the inquirer's fork report. The queue lives in
`rhtn-transport`'s node for now (`queue.rs`: an in-memory store and a
directory store, a per-subordinate byte cap, supersession), not in the
`rhtn-node` crate the plan's table names; the split is deferred until
there is a second consumer. All 46 milestone-3 entries pass: ARC-01 to
ARC-17, TOP-01 to TOP-17, QUE-01 to QUE-09 and QUE-14 to QUE-16; 84 of 256
in all, in 15 chain tests, 18 topology tests and 13 queue tests. Two codec
defects surfaced: the body rules counted `wire-format.md` §4.1's field 9 as
an extension, so a transfer adoption failed the 1024-byte extension bound,
and the archive's first draft fed the body check envelope bytes rather than
body bytes, which made the recovery block fail the same bound; both fixed,
the corpus unchanged at 153 agreements. `rhtn-crypto` gained kid-less
signing variants for embedded and standalone signatures, which
`wire-format.md` §3.5 requires and the fixtures already show, and
`verify::envelope` now checks the former patron's transfer statement.

Readings the milestone's code takes, for the author to confirm or reverse:

- **Batch continuation overlaps by one record.** `wire-format.md` §7.9's
  field 4 names the oldest record returned and the next request names it in
  field 2, so the next reply begins with that record again; the repeat is
  the continuity check ARC-08 and ARC-10 both describe. "Each returned
  record's back-pointers must match the record that follows it" is read as
  reachability — every record after the first must be one an earlier
  record's back-pointers name — since a merge has two followers
  (`wire-format.md` §3.1). Pointers unsatisfied at the batch's end are the
  continuation, or a checkpoint when the naming record is a series reissue.
- **A reissue is a checkpoint in a walk** when its predecessor is not
  served (ARC-13's interpretation); an ordinary unserved predecessor is
  reported as unfetched, distinctly. Pruning takes a reissue beyond the
  730-day window and releases the chain before it; the evidence store is
  keyed by txid and untouched (design §10.0).
- **Supersession evidence is a value** (`Supersession`) built from a
  verified recovery adoption (prior key to node) or a verified reissue (the
  same key). The node ends the credential's session — close code 1 with
  reason `superseded`, since no code is assigned — refuses its attach with
  no AttachAck, and **drops what was queued for a key whose successor is a
  different key**: nothing further may be delivered to it and its successor
  collects nothing of it (the model's reading in QUE-16), so holding the
  ciphertext would be retention without a recipient. On a reissue the key
  stays, so only the session ends and the queue is kept.
- **QUE-04's restart is in-process**: the node and its endpoint are dropped
  and a new node starts from the queue directory. The directory store keeps
  the recipient and arrival time in the path and the ciphertext as the
  content; delivery unlinks the file before the item goes out on its stream.
- **A recovery ends every open binding the prior key holds in the
  observer's table**, not only the one under the adopting patron, on design
  §9.0.2's "remove it from their records and overwrite it with the new
  one"; competing recoveries resolve by the table's patron preference and
  the loser's successor stays a node under its own patron. Rotation being
  per-subnet (design §13.6) may argue for ending only the binding in the
  subnet the recovery names; the code takes the broader reading and the
  author should say.
- **A failed evidence evaluation keeps an adoption out of the table**
  (TOP-12's interpretation), and a disavowal that arrives before the
  adoption it ends is held and settled when that adoption arrives, so the
  slot reads the same in every arrival order (TOP-17).
- **Open for a decoder pass**: `verify_sign_block` still accepts an
  embedded signature that carries a `kid` when it matches the enclosing
  structure's signer; `wire-format.md` §3.5's "nothing else appears in
  either header" makes that arguably malformed. The builders no longer emit
  one; the decoder's tolerance is recorded rather than tightened here.

**Milestone 4 closed (2026-09-10).** Two crates are new. `rhtn-node` carries
what a node decides above the session: the topology store and the
forwarding rule, the rootward memo, the anchor table and resolution,
currency issuance with its escalation ladder, peering records and the
direct payload path. `rhtn-sim` carries a datagram-level path — a UDP relay
below QUIC that can drop, delay, blackhole, capture and replay — and a mesh
of node views with severable links. 71 of the milestone's 72 entries pass,
plus TRN-16 from milestone 2: PRP-01 to PRP-15, RES-01 to RES-16, CUR-01 to
CUR-13, REP-01 to REP-06 and REP-08 to REP-16, SES-06 to SES-12, SES-14,
QUE-10 to QUE-13. 156 of 256 in all, in 15 propagation tests, 14 resolution
tests, 2 contact tests, 13 currency tests, 12 peering tests, 13 session
tests, 3 partition-merge tests and 2 replay tests.

**The exit criterion is met.** `models/tla/PartitionMerge`'s two safety
invariants and its convergence property are restated over the running code
in `rhtn-sim`'s mesh: three nodes seeded, partitioned so one side signs a
departure the other cannot see, then healed and reconciled, with both
invariants checked at every step and every pair agreeing about every
subject afterwards. A companion asserts the negative — a partition that
never heals does not converge — because a convergence test that never sees
divergence proves nothing. TRN-16 passes with its premise asserted rather
than assumed: a separate test shows the second dial really does carry its
Attach as 0-RTT early data and the server really does accept it, so the
replay test is exercising early data and not an ordinary first flight.

**REP-07 is deferred to milestone 5**, where the reference metric exists.
The entry asks that two graphs differing only in peering ASN score
identically, which needs `rhtn-policy`. Nothing else in the milestone is
owed.

Readings the milestone's code takes, for the author to confirm or reverse:

- **The storage rule reads the position a transaction establishes, not the
  subject's current one.** `wire-format.md` §10.1.1 says a node stores a
  transaction when its subject falls in that node's own `h_store`, and a
  new member's adoption would never flood under a literal reading, since
  before the adoption nobody holds the subject at all. The code stores when
  the subject is within h, **or** the counterparty the transaction names is
  within h-1, which is the same ball read forward. A peering is in range if
  either endpoint is, as the section says.
- **PRP-11 as drafted names the receiving node in field 1**, which also
  fires §10.2.1's cycle check. The test asserts what the entry asserts —
  nothing forwarded to the patron — and covers the timestamp rule
  separately, against a memo about another patron's slot the node's memo
  table already holds at a later timestamp.
- **A memo naming an attached client is neither acted on nor forwarded by
  the serving node**: §10.2 says the hit is handed to that client at
  contact and that the records answering it are the client's. The code
  reports it and stops; handing it over is client work and belongs with
  `rhtn-client`.
- **A currency attestation's field 1 is the identity as the querier named
  it and field 2 is the key the issuer currently records.** After a
  recovery a distant caller still holds the old keyhash, so a query naming
  it is answered with the successor in field 2 and never with the old key,
  which is what `infra-client-requirements.md` §3 forbids. CUR-03 as
  drafted reads as though both fields move.
- **CUR-07's evidence is a validated recovery, not a reissue chain.** The
  entry says a reissue chain supersedes "S's key k1", and a reissue changes
  the series and keeps the key; the Attach frame carries no series, so a
  node cannot refuse an attach on a reissue without refusing the legitimate
  holder's reattachment too. §12.6.5 names both forms as authenticated
  supersession evidence and the test uses the one the wire can act on.
- **A client marked unreachable is queued for, not delivered to**, even
  while a session object survives: design §14.1.2 says material queues as
  it always did and delivery waits for its return, and a connection whose
  packets are being blackholed outlives the detector's verdict by the idle
  timeout. The detector decides.
- **Reachability replication has no wire object.** design §14.1.2 requires
  the state to reach the siblings and specifies nothing that carries it.
  The node calls a configured hook; the tests observe the sibling's state
  and check neither the carrier nor the latency, which is what SES-14's own
  interpretation allows.
- **The client's dial timeout is the client's own number.** `wire-format.md`
  §7.7.3 makes selection and retry local policy, so a dial that does not
  answer is abandoned after a configured interval and the next endpoint
  tried. Without one a blackholed path holds a fresh attach for the whole
  idle timeout.
- **SES-07 needs the session to be running before the suspension.** A task
  spawned and never polled takes its monotonic start when it first runs, so
  a test that blocks the runtime before the control loop has started is
  measuring the wrong interval. The test exchanges heartbeats first, which
  is what "is in a session" means.
- **PRP/RES/CUR/REP run over an in-process fabric rather than loopback
  QUIC.** The frames are the wire's own bytes, encoded and decoded by the
  codec; what the fabric stands in for is the session, not the encoding.
  The entries that turn on real packets — the failover detector, the
  mailbox across an absence, a wrong address, a dead first endpoint, the
  replay — run over loopback with the path harness.

**Milestone 4 review pass (2026-09-10).** The milestone was built on a
lower-spec model by accident and reviewed in full afterwards: every new and
changed source and test file read, the readings checked against the wire and
design text, and the tests checked against the entries they claim. The
review found three definite bugs, two design-level gaps, one security gap,
about ten tests that asserted what they set up, and one missing layer: the
node's decisions were never bound to live sessions, so over real QUIC none
of the propagation, resolution or currency behaviour ran. Seven commits
disposed of it; what each decided is below, and the readings that need the
author's word are marked.

- **Bound to sessions.** The transport gives each session an outbound
  channel, a control-frame hook and a request-stream hook, and a client
  session exposes the frames its serving node delivers and a request call.
  `rhtn-node`'s `LiveNode` installs the hooks, so a topology push arriving
  on any session goes through the forwarding rule and out on the node's
  adjacency, a client's resolve request is answered from the node's tables
  or run on its behalf hop by hop over real sessions, and a currency
  request is answered by the ladder. Six end-to-end tests over loopback
  QUIC: a push crossing two real sessions byte for byte and never back up,
  a resolution answered with the residual suffix, a resolution proxied
  through a real referral chain into another subnet, a currency request
  answered by the patron, a running node whose replication payload holds
  its store and never its mailbox (REP-16 lives there now), and a
  requester past its allowance. Binding it found two more defects: the
  transport's classifier handed handlers the whole frame payload rather
  than the body, and a node with no published endpoint record answered a
  resolution with a reply the schema rejects. Both fixed; a running node
  now publishes its own endpoint record at start, and a node with none
  reports itself unavailable.
- **The relay's table follows the flood** [reading, for the author]. The
  storage rule measures distance from the table, and the table refused any
  adoption whose presence record it could not dereference, which for a
  relay is every adoption, so the flood stalled one hop past any relay.
  The archive table now takes an evaluation mode: a patron relying on an
  adoption evaluates first, as TOP-12 has it; a relay binds on structural
  verification and records the evidence as unevaluated, upgrading it when
  the record arrives. `wire-format.md` §3.4 separates valid from effective,
  and this reads a relay's position table as the former. The author should
  say whether an unevaluated binding is a position at all.
- **Slots follow adoptions.** The child index a memo names is assigned from
  the adoption's locator when the patron applies it, emptied by a departure
  or disavowal, and every change to one of the node's own slots travels
  rootward as a memo. A cycle-check disavowal therefore produces a memo
  like any other (§10.2.2), and PRP-11 and PRP-12 now distinguish the
  received memo, which is not forwarded, from the node's own, which is.
- **Origination stores.** An originated object enters the originator's own
  store and table before it goes out, so an echo dies as a duplicate.
- **An attached client's own memo travels.** The serving node's cycle check
  for a client fires only on a memo that arrived from below that client.
- **Re-resolution starts from the subject's locator**, held in a locator
  store the view now carries, with a random nonce from the transport's
  crypto provider, and sends nothing when no locator is held.
- **Memos route by subnet.** A node bound under several patrons carries a
  position per subnet, sends each memo to the patron of that subnet, and
  answers a resolution for each position it holds. The binding records the
  anchor its adoption's locator named.
- **The relying party places the issuer** [security gap closed]. A staple
  is current only when its issuer stands on the rung it claims for the
  subject — the patron, a sibling of the patron, or the grandpatron — as
  the table or the introduction's locator has it. A stranger's signature or
  a real party on the wrong rung is not current, and an issuer the node
  cannot place at all is not current either; both fail closed. The variant
  that checked the subject is now named for what it checks.
- **The table is the currency record.** The parallel map of recorded keys
  is gone; issuance names what the table records as current for the
  identity asked about, following the recovery lineage, so an identity a
  querier still names by its old key is answered with the successor and
  never the old key. The fallback query is a state machine: introducer
  first, patron only on the introducer's code 1 or absent session, and an
  exhausted ask fails closed. Establishing current control before a
  trust-bearing operation is the node's own act.
- **Proposals are bodies.** An adoption proposal takes the subject's own
  back-pointers, which are the subject's to supply, and assigns the lowest
  free slot under the node's position; a peering proposal names the
  counterparty's own point and a real presence record. The helpers that
  forced genesis and fabricated endpoints are gone.
- **A second endpoint line waits on a chain.** The global toggle is gone: a
  subject's first line is taken as gossip, a second line for a subject the
  node already holds is held until a series chain proves it, and two
  unproved lines rank nobody rather than by series number. Tested.
- **Smaller items.** One `NetworkPoint`, the transport's; the session's
  reachability handle dropped at session end and the settled mark kept;
  queued payload waits at the recipient's serving node; the adoption
  evaluation dereferences through the store rather than reparsing it; dead
  helpers removed; the mesh's dead adjacency removed.
- **Tests that fail when their rule is broken.** RES-14's slot is emptied
  by the node; RES-01 records every identity the requester asks for and
  finds none; RES-07 is proxied by the serving node through a real referral
  chain and returned; RES-03 goes through the node's entry point and shows
  no frame; REP-08 shows the peering stored and forwarded and no memo;
  REP-15 derives the replication set from the serving node's own table with
  the light-client patron's sibling present and excluded; SES-10 submits a
  body naming the patron and shows the sibling's signature over it is no
  transaction at all; CUR-05 and CUR-11 are driven by the node's ask
  machine; CUR-07 reads the node's own supersession; TRN-16 asserts its
  premise, cuts the captured flight at the instant the handshake completed
  and requires the replayed Attach never to have been read; NoInvention is
  the model's formulation asked of the running views, and a record handed
  to one node alone is caught. The tests that count seconds run one at a
  time.
- **Persistence and rate limiting.** The topology store writes itself to a
  directory and reads back as the seen-set, so a restarted node replays no
  forwarding wave; a running node allows each requester so many requests
  per window and fails the stream past that.
- **Lint.** clippy is installed on the stable toolchain and the gate runs
  it over every crate and every target with warnings as errors: 76
  findings on the first run, 0 after.

Still open after this pass, for the record: down-line issuance for roots
(design §12.7.2) is a multi-signer object not built; the reachability
detector and the currency ladder's `unreachable` set are joined by hand
rather than by the transport; REP-07 waits on milestone 5; the memo table
is kept unconditionally; a cycle-check memo about an emptied slot is not
treated as confirmed.

**Milestone 5 closed (2026-09-10).** One crate is new. `rhtn-policy` carries
the reference flow metric over the graph an evaluator builds, the policy
interface a node consults, and the conformance test that reports what a
substitute policy gives up. 9 entries pass: MET-01 to MET-08, and REP-07,
deferred from milestone 4. 165 of 256 in all, in 13 metric tests, 1 archive
test, 1 node metric test, and REP-07 among the peering tests, which are 13
now.

**The exit criterion is met**, and by more than it asks. The plan names four
regression cases; six are carried from `models/simulation/flow_metric.py`
and each fails when its rule is broken: λ < 1/f necessary and not
sufficient, at the file's own numbers (branching 10 converges to 20,
branching 11 passes 1,000); setwise conservation per computation and not
per lifetime (8 + 8 separately against a cut of 8, 8 together); parallel
edges collapsing to one pair (a pair joined by an adoption and two meetings
carries 10, and the summed graph 30); visibility not composing (a peering
edge one hop past a visible one invisible at reach 0 to 3, standing 0);
hops in the collapsed graph being the landscape distance (every placed node
in the E3 graph, 2d − 1 split hops for distance d); and the three-pass
allocation independent of construction order, in the simulation's own three
cases. The fixed-graph fixture ranks five candidates by flow, then hops,
then consideration order, against a cut of 4 that admits four, and reports
each candidate's flow. The simulation's committed figures are reproduced
exactly and read back from `models/results/flow_metric.txt` by the test
itself: E2's best individual score 10 at 7, 40 and 341 identities; E3's
independent sums 32, 64, 104 and 168 against a joint of 4, 8, 8 and 8 at a
cut of 8. `models/simulation/results.txt` was stale — 128 and 256 at 16
and 32, from the clique construction the region lost on 2026-09-09 — and is
regenerated; it is identical to the gate's file now.

Readings the milestone's code takes, for the author to confirm or reverse:

- **The policy interface is one call.** A policy is handed the evidence the
  node holds and a candidate set, and returns each candidate's standing, the
  admitted set and the joint, from one computation. Nothing on the node's
  decision path consults it: MET-04 runs the same pushes through a node
  under the reference metric and under a policy that scores every known
  node alike, and shows the same decisions, the same forwarded frames and
  the same stored objects, with only the standings differing.
- **A node's evidence is its table and its store.** Adoption pairs come from
  the table's open bindings; acquaintance pairs from the presence records
  kept as evidence and the peering records stored. A relay keeps no
  presence record it did not dereference itself, so a running node's
  acquaintance graph is its peerings plus what it evaluated — design
  §16.2.1's floor, "what §15.1 stores and §16.3 makes visible".
- **Edge capacity is uniform at 10 and node throughput is the simulation's
  schedule**: unthrottled for the observer and its whole horizon, then 16,
  8, 4, 2, 1 by landscape distance. Both are the reference policy's numbers
  and neither is the design's (design §16.2, §16.4). Every pair carries the
  same edge whatever relationship produced it, which is design §16.2.1's
  "no ratio between the two kinds".
- **Standing is a float at the interface** so that a decay policy's λ^d and
  the flow metric's integers share one type; the reference metric's own
  calls are integer, and the tests on it compare integers.
- **The conformance verdict is about growth for a policy that is not
  flow-based.** A decay policy's joint standing is a sum of λ^d and compares
  with nothing in flow units, so the report says it grows with population.
  For the reference the report states the region's cut and that neither the
  best individual standing nor the joint exceeded it at any size, and that
  the joint stopped growing. The branching section grows two regions to
  depth 3 at f = 10 and classifies by the ratio of successive increments:
  0.95 for the hierarchy alone, 1.045 then 1.14 with an acquaintance degree
  growing with depth — the same verdicts as the 200-level arithmetic, which
  the test also carries.
- **The conformance test's honest tree is not the simulation's tree.** The
  simulation draws from Python's generator; the crate draws from its own,
  seeded the same way. No figure above depends on the honest tree's shape,
  only on a node existing at the horizon's edge and on the first child
  being the peer, which both have.
- **`rhtn-policy` builds at opt-level 2 in dev profiles** (an override in
  the workspace manifest): the conformance report scores regions of about
  1,900 identities with one max-flow each, which an unoptimised build made
  a matter of minutes.
- **The gate's verdict was `tail`'s, not cargo's.** `check.sh` ran
  `cargo clippy … | tail -20` and `cargo test … | tail -20` and took the
  pipeline's status, which is the last command's, so a lint error or a
  failing test printed its text and the step still reported clean. Found
  today when a lint in this milestone's own test compiled as an error and
  the gate printed "cargo clippy: clean" beneath it. `set -o pipefail` is
  added; the script has read this way since it was written on 2026-09-09,
  so every verdict between then and now rested on the step's printed text
  rather than its exit code. The text was read each time, and the
  milestone 4 lint count came from a direct clippy run; this milestone's
  run is the first whose exit code means what it says.

Still open after this milestone: down-line issuance for roots (design
§12.7.2); the reachability detector and the currency ladder's
`unreachable` set joined by hand; the memo table kept unconditionally; a
cycle-check memo about an emptied slot not treated as confirmed. Nothing
from milestone 5 is owed.

**Root currency reframed (2026-09-10).** Asked about the milestone 5 open item
"down-line issuance for roots", the author did not recognise the direction and
found it inadequate, a root with no subordinates having no process under it.
Traced: design §12.7.2 and `wire-format.md` §7.1's role 3 entered on
2026-08-13 with §12.6.5 and §12.6.5.1, attributed to nobody, before the
repository's baseline; the one later touch was a reviewer's leaf-case finding.
Ruling, verbatim: *"As long as roots with zero subs can still propagate, leave
the mechanism for subordinate-attested roots in place and document it as an
optional input for alternative anchor cacheing rules."* The condition holds in
the design and in the code: §15.1's storage rule and the flood in
`rhtn-node` consult no staple, and the currency gate's callers are the
adoption proposal, the peering proposal and the ask machine only.

Applied: design §12.7.2's heading and first two paragraphs rewritten, the
residual paragraphs kept with a new lead; one sentence in §12.7.3; §19's veto
row; the role-3 comment in `wire-format.md` §7.1; a change-log entry; the
`ROLE_DOWNLINE` comment in `rhtn-node`. The milestone 5 entry's open item
"down-line issuance for roots" is withdrawn as a currency rung: nothing issues
or consults role 3, and nothing now says anything should.

One code gap surfaced by the check, for the record and not yet built: the
node's currency gate refuses a trust-bearing operation on any absent staple,
with no path for an identity that claims nothing. design §12.7.1 says a staple
is owed when a record claims prior standing and not when an identity claims
nothing, so a Genesis identity's first adoption through `propose_adoption` is
refused where the design says it proceeds. CUR-05's fixture is an identity
with a patron and an expired staple, not a Genesis one, so no entry catches
it. What "claims nothing" is in a form the code can test — no back-pointer
beyond genesis, no presented head — is a reading the author should confirm
before it is built.

**Currency gates nothing (2026-09-10).** Asked what trust-bearing operations
the node refused for a naked root, the author ruled, verbatim: *"Neither of
these cases should depend on currency. You need to be adopted to establish
currency in the first place, and peering, PoP, and adoptions are explicitly
declared to be independent of any possible patron restriction."* Verified:
design §6.4 says no party can block another's client and calls proof of
presence ungated; §6.3 calls peering ungoverned, requiring nobody's authority;
§18.5 names departure plus adoption elsewhere as the escape hatch. Asked
whether anything remained fail closed on an absent or expired staple: *"No,
stapling supports routing operations only, not any of the trust transactions
or user operations."* And to the soft-fail argument being answered by
supersession inside the horizon and by expiry beyond it: *"Yes, go ahead."*
The gate and its examples entered on 2026-08-13 with §12.6.5, before the
baseline, attributed to nobody.

Applied, 20 places. Design: §12.6.5's soft-fail paragraph gains the author's
answer; the lifetime derivation, the stakes table and its light-client
sentence, the knowledge rule's two references to the table, and the closing
spend-versus-use sentence are rewritten; §12.6.5.1's cascade and honest
limit; §12.7.1's third and fourth paragraphs; §12.7.2's first paragraph and
residual; A33's dependency cell. Wire: §7.1's "fails closed" and §8's Attach
comment. Catalogue: CUR-05 and CUR-10 withdrawn as tombstones — a new kind
`withdrawn` keeps the number, cites nothing, carries its reason, generates no
stub and counts as nothing owed; CUR-06's two quotes and CUR-13's outcome
reworded; CUR-14 (an adoption countersigned with no staple and with an
expired one, nothing asked first) and CUR-15 (a peering proposed with every
issuer dark, and again) added. Code: the operation enum and the staple cases
of the gate removed, the one refusal being supersession knowledge; the
adoption and peering proposals take no staple; the ask machine settles on
proceed where nobody can be asked; four tests rewritten and two replaced.
Counts: 258 entries, 2 withdrawn, 165 of 256 implemented, 0 flags;
references 2074 / 0 flags.

Consequences drawn rather than ruled, for the author to confirm:

- **The cascade is gone.** §12.6.5.1 said a dark patron degraded its
  subordinates' countersignatures and so their subordinates' transactions,
  down the subtree. Countersignatures do not depend on staples, and a
  subordinate refreshes from its own patron, so what a dark patron silences
  is the identities it issues for and nothing below them. The ladder's
  motivation is narrower than it was and still real; the sentence now says
  so.
- **The disavowed leaf is not frozen.** The residual in §12.7.2, which came
  from a reviewer's leaf-case finding, rested on adoption being gated; it now
  says a disavowal costs a leaf its issuer and nothing else, and that §18.5's
  escape hatch is as open to it as to anyone. §18.5 itself was unchanged.
- **What "routing" covers is stated as which key to address**, on the
  author's words; the table lists addressing the named key, falling back to
  the query, concluding nothing from silence, and serving nothing under a
  known-superseded key. The Attach comment follows.

Not touched, and now describing the withdrawn gate: `models/README.md` says
`wire-only/currency` establishes that a trust-bearing acceptance of a key as
current requires an unexpired staple, and `CurrencyEscalation.tla`'s comments
call re-adoption an escape from a frozen state. Both belong to the next model
pass; the theorems themselves state issuance and the ladder, which stand.
Still open from milestone 5: the detector and the ladder joined by hand; the
memo table kept unconditionally and never read by occupant; the emptied-slot
cycle memo unconfirmed.

**The genesis of a root, tested (2026-09-10).** On the author's instruction
the models README no longer describes `wire-only/currency` as proving a
trust-bearing acceptance; it says the relying party takes a key as current on
an unexpired issuance, which key to address being all a staple decides. Four
tests exercise a root's genesis, propagation and reachability. Three over the
fabric: a new root has nothing and needs nothing — self-anchored, empty path,
a chain at genesis, no staple, nobody to ask, and the currency call settles on
proceed with no frame sent; its peering with a node of an existing subnet
floods that subnet up and down and never back, its address travels in the
peering record, it has standing of 10 at the subnet's root and at a node two
hops off, and its endpoint record is out of everyone's store, reaching a
stranger being the anchor table's business; an existing node whose anchor
policy admits a root of nobody resolves it with an empty path and the root
answers for itself, while a policy of one subordinate does not cache it and
an absent entry sends nothing. One over loopback QUIC: the root starts,
attaches to the existing node, originates its peering, which reaches the
subnet's root and its client over real sessions; the client resolves the new
root through the node's anchor table, is answered with the root's real
endpoint, and attaches to the root directly, which answers a currency request
with code 1 and a resolution for itself. None carries a catalogue id: no
entry asks for genesis, and the tests are the author's own addition.

**The sweep after the currency ruling (2026-09-10).** On the author's
instruction, every material read for conformance to "nothing waits on a
staple", design first, then models, implementation and tests, with every
automated stage run.

Design and requirements: 5 documents swept for trust-bearing, fail closed,
frozen, current control and spend, and for every mention of a staple or of
currency — 7 hits outside §12.6.5 read in context. Two needed changing.
§14.1.2 and `light-client-requirements.md` §4 used "trust-bearing
operations" for what a degraded session withholds, a phrase whose definition
went with the table; both now say the subnet-scoped transactions the patron
countersigns wait for the patron (design §6.4), and the comparison to
secondhand attestations is dropped. The rest stand as they are: §3.4's
"trust-bearing transaction history" names a replication class; §11.2's
"a resource's key currency derives from its owner's attestation" and §9.x's
fork visibility describe addressing; the compelled-provider entry's
"thief-issued currency dies with the staple lifetime" was already about
addressing. One reading left for the author: §11.4 lists currency among the
time-dependent values a role table recalculates. A staple decides which key to
address and is not a trust input, so what a role table recalculates from it
is unclear; nothing was changed there.

Models: `wire-only/currency`'s comments described a trust-bearing acceptance
that fails closed in four places; they now describe taking a key as current
for addressing, with nothing waiting on it, and the lemma names are kept.
`CurrencyEscalation.tla`'s header and honest-limit comments said frozen in
three places and now say unattested, with the sentence that nothing the node
does waits on it. The compliant README describes
`no_trust_bearing_operation_on_a_sibling` as countersigning nothing, the
name kept from when the design called countersigned transactions
trust-bearing. Appendix B holds no currency text. Model citations 197 / 0
flags.

Implementation and tests: the transport's degraded-session comment and the
node's ask-machine doc followed the wording; SES-10's quote follows §14.1.2
and CUR-11's trigger is addressing a subject on a stale staple. No source or
test carries the withdrawn vocabulary now except the two tombstones' titles,
which are the record of it. Catalogue 258 entries, 2 withdrawn, 165 of 256
implemented, 0 flags. References 2074 / 0 flags.

Stages run: refcheck, modelrefcheck and the catalogue checker above; the
test-vector verifier, with dilithium-py on its target path — ALL CHECKS PASS;
the code gate — CODE GATE PASSES, clippy clean, every test green, the fuzz
smoke crash-free; the model runner — ALL MODELS PASS: the simulation's
assertions, 6 TLC instances with no error, 5 TLC mutations violated as
expected, 8 Tamarin theories verified (67 lemmas), 2 bounded companions, and
7 theory mutations falsified as expected. The results files are regenerated
by the run and differ from the committed ones in their timings only.

**§11.4's currency taken out (2026-09-10).** Asked about the one reading the
sweep left, the author: *"Take it out."* §11.4's fourth trigger now names
decay alone as what moves with time; the change-log records it. No entry
quoted the sentence. References 2074 / 0 flags.

**The readings accepted, provisionally (2026-09-10).** Shown the readings the
code takes at milestones 2 to 5 and the sweep's two consequences, the author:
*"I don't necessarily have a full understanding, but these all seem
reasonable to me. Accept these without recording them as decisions, if
something comes up later which conflicts this assessment can be overridden."*
So recorded: none is written into the design as a ruling, the code keeps
each reading, and any of them yields to a later finding that conflicts.

**Three of the six open items built (2026-09-10).** On the author's "Build all
and retest":

- **The detector feeds the ladder, by outage duration.** The currency state
  now holds who is dark and since when on the node's own clock, and how
  long an outage must have lasted before each rung opens: the sibling's
  interval defaults to the attestation lifetime, the grandpatron's to two
  days, both the operator's numbers standing in for the table's "hours to
  days" and "days". A running node feeds it from both sides: the sessions
  it serves, through the transport's replication hook, and the session it
  holds upstream, through a new client-side reachability hook on
  `ClientConfig`. CUR-08 and CUR-09 now show a rung closed within the
  interval and open past it; CUR-16, over loopback with one-second
  heartbeats, shows a grandpatron's state hold a patron and its sibling dark
  since its own clock, a subordinate's state hold its patron dark, code 1
  within the interval and a grandpatron attestation two days on. What the
  join cannot supply: a sibling that holds no session with its sibling
  learns nothing from the detector, since the transport has no sibling
  sessions.
- **The memo table is read by occupant, and the downward memo walks the
  tree.** A memo from below whose occupant the table already holds in
  another slot goes down the branch toward the patron who has not just
  spoken, by that patron's position as the table last saw it, and up as
  before; a memo from above is a downward memo, written on the way past
  and walked on, and it stops at the patron whose slot it did not name,
  who reports it and keeps its row. A node keeping no table forwards every
  memo and detects nothing, which is the graceful degradation §10.2.2
  describes. PRP-16 and PRP-17.
- **A cycle memo about an emptied slot is confirmed by the empty row**, an
  empty slot being a row and not a deletion. PRP-18.

Four entries added, 262 in all, 169 of 260 implemented, 0 flags; the gate
passes. The other three items — the queue's move, the decoder's tolerances,
the corpus's stale fixtures and reply families — follow.

**The other three open items built (2026-09-10).**

- **The decoder's tolerances.** An envelope's signature entries must sit in
  the canonical order, by kid and classical before post-quantum, a
  nonempty unprotected header is malformed, and a protected header carries
  alg and kid and nothing else; an embedded or standalone signature, whose
  enclosing structure names the signer, carries alg alone, so a kid there —
  matching or not — is rejected where it used to be tolerated. The
  extension bounds are counted in every nested signed map — a peering's
  network points, an adoption's Recovery block and its responses and
  Transfer, a presence record's participants, witnesses and responses, and
  the network points and locator inside a standalone record — each on its
  own, as `wire-format.md` §1.3 counts per map. DEC-17 and DEC-18. The
  signed-decoder tests' own response builder emitted kids in embedded
  signatures, which the tolerance had hidden; it now builds as the archive's
  builders do, and the kid-bearing variant is the negative case.
- **The queue's move.** The directory store a restarting node keeps is
  `rhtn-node`'s (`queue.rs`), with QUE-04 beside it; the store contract, the
  memory store and the supersession discipline stay in the transport, whose
  session layer delivers from them. The plan's milestone 3 note says so.
- **The corpus.** Every reply entry now states its family on the entry, and
  the two harnesses read it there rather than inferring it from the id.
  EndpointRecord and SignedLocator fixtures are signature-verified like every
  other signed record; the one shape fixture the corpus marks "signature
  stale by design" is checked structurally, per fixture rather than per
  kind. The vectors were regenerated with the spec-change acknowledgement.
  What the acknowledgement covers: the documents changed since the pins
  were last written on 2026-09-08, and the diff to `wire-format.md` was read
  in full — a prose correction naming field 4 rather than 5 for the
  resolution failure codes, the ≥ sign, "sends one", the currency comments
  of §7.1 and §8 rewritten today, and the successor-continuity note of
  2026-09-08 — none of which changes an encoding the generator constructs;
  `network-design.md`'s changes are today's currency ruling and the
  down-line reframing, which encode nothing. The regenerated set differs
  from the committed one in the twelve `family` fields and the pin lines
  only; the verifier reports ALL CHECKS PASS over it.

Two entries added, 264 in all, 171 of 262 implemented, 0 flags.

**Conformance review of `rhtn/` at ea4d5aa, remediated (2026-09-10 to
2026-09-11).** The review (`conformance-review/`, untracked, the reviewer's
own directory with a harness of twelve assertions) reported twelve findings
against the implemented components, eight P1 and four P2, and a coverage
section on the unbuilt subsystems, which the author set aside. All twelve
held on verification: the harness reproduced eleven here exactly as
recorded, and the three source-traced findings (F08, F10, F12) were
confirmed by reading the paths cited. Nothing rested on a misreading of the
root documents. One ruling was needed and given: a light client answers no
request stream, so a request goes only up a session the node holds as the
client, and a party attached below cannot be asked (2026-09-11). Applied,
one gate-green commit per finding or pair, in review order within three
groups:

- **F01, F03** (2321e99): the COSE container checked in the four places a
  COSE array is read; an embedded block by one hybrid signer carries exactly
  two entries in canonical order, so an empty block no longer verifies.
  DEC-19.
- **F02** (c6a3b2e): the body checked against the type its envelope names,
  never guessed from shape; exactly one evidence form; the Recovery block's
  consistency rules; two distinct parties in every two-party type; a
  peering's evidence and a reissue's counter; a formation genesis-rooted
  with no evidence arrays, a normal record witnessed, participants distinct.
  DEC-20 to DEC-22. Wider than the review's three reproductions, since the
  decoder had none of §4.1's Recovery rules and the corpus no negative for
  any of them: three boundary fixtures that omitted their evidence were
  repaired in the generator and nine negatives added, the envelope-class
  ones now run through the verifier. The test worlds meet established keys
  on a witnessed normal record, a key forming only once (`wire-format.md`
  §3.2), which every harness had been violating.
- **F07** (e29271e): the packed-path invariant at the schema, the resolver's
  decoder and the archive's locator decoder; hop extraction total. DEC-23.
- **F04, F09** (02d2ed8): a departure held and settled against the series it
  names in any arrival order; a re-adoption's new series untouched.
  TOP-18. The mailbox on disk resumes its sequence and creates exclusively.
  QUE-17.
- **F05, F12** (a929b8b): every stored recovery or reissue carried into the
  serving state on both paths, and seeded from the store at start. QUE-18.
  Own endpoints published per relationship line from what is held: replay
  unchanged, next counter for a change, the position advancing with it.
  RES-17.
- **F11, F10** (9574398): a clock source read at every decision, the running
  node's installed at start, the simulation harness pinning each node at
  its scenario's epoch. CUR-17. Session recording a test facility, off by
  default on both sides. SES-16.
- **F08** (2276ef8): every accepted message enters the store; a drain per
  recipient delivers oldest first and removes each message once the peer
  has acknowledged every byte; the immediate path is the same path. QUE-19.
  Taken means transport-acknowledged, so a message whose acknowledgement a
  failure swallowed may go twice: the no-loss failure mode.
- **F06** (5ff6f18): the adjacency gains a request operation on a
  bidirectional stream; the currency ask and a conflict's re-resolution use
  it and are held by nonce for replies, which come back through a reply
  path into the view; the test fabric records requests apart from control
  frames. CUR-18. The ruling above is the documented limit; what a node does
  with a repair's answer beyond following referrals is not specified and
  not built.

Numbers: Thirteen entries added, 277 in all, 184 of 275 implemented,
0 flags; each commit's gate green. The review harness rerun at the end:
12 of 12 pass on a copy in the scratchpad, after two adaptations to its own code, the session-adjacency literal
gaining the two fields the request path added and the view's clock set
through its setter; the reviewer's directory itself is untouched and
untracked.

**Second conformance review of `rhtn/`, at 85fcf08, remediated
(2026-09-11).** The reviewer's second pass (`conformance-review/`, again
untracked and untouched) found the twelve original assertions passing
after adapting its own harness to the clock and adjacency interfaces, and
reported eleven current findings, five P1 and six P2: two carried forward
as partially resolved (F02, F04) and nine new (N01 to N09). All eleven held
on verification: the harness's thirteen new assertions reproduced here
exactly, and every cited path read as described. Three were wider than
their reproductions and were treated so: N01's collapse of a verification
error into "no key" was the anchor entry's too; N05's validator belongs in
the codec, since an adoption's field 5 carries the same encoding; N08's
unchecked algorithm value applied to every classical Sign1. Applied, one
gate-green commit per pair, in three groups:

- **F02, N08** (c783edb): the adoption's locator and the peering's network
  points checked in a signed body, a signed map's unknown keys kept as the
  extensions it bounds; a witness nominated by a participant; the crypto
  verifier running the typed body rules. DEC-24. A signature's declared
  algorithm held to its profile's. DEC-25. Found on the way: the test
  worlds wrote the default port out, which §4.4 makes malformed, so the
  network point normalises it to absent.
- **N03, N01** (34f6900): the verifier's failure typed, a missing key of any
  signer, envelope or embedded, reported as unverifiable naming it, so the
  store holds a transfer for the former patron's key. DEC-26. A self-signed
  record with no slot, or failing under a held key, malformed and never
  gossip; every standalone signed kind carries its slot. RES-18.
- **F04, N09** (2ff4d57): a routing slot derived from the settled binding,
  so a departure held before its adoption leaves the row empty. PRP-19.
  Dereferenced evidence counted only once its signatures verify: a failing
  record never evidence, an unverifiable one unevaluated where deferred and
  refused naming the key where required. TOP-19. That reading of "an
  evaluation step, not a structural one" is the assistant's, open to
  reversal.
- **N02, N04** (05a055c): the store's save rewrites the endpoint records
  whole and its load drops what the conflict markers retire. PRP-20. An
  archive appends a held record idempotently. ARC-18.
- **N06** (1628413): a frame reader that keeps its buffer across the loop's
  selects, reading through the cancellation-safe chunk read, so a frame
  half received when a timer or an outbound frame wins is finished on the
  next turn. SES-17.
- **N07, N05** (500b9e6): the cap read and the push under one gate. QUE-20.
  Key material validated to §2.2's shape before a pin. TRN-17.

Numbers: Eleven entries added, 288 in all, 195 of 286 implemented, 0 flags;
each commit's gate green. The reviewer's harness rerun at the end:
24 of its 25 assertions pass on an unadapted copy in the
scratchpad, N07 excluded and run alone, where it waits past a one-minute
timeout. Its N07 reproduction cannot pass against a serialised
submission path: it makes the store block inside the cap read until a
second submission reaches the same point, which the gate that closes the
race now prevents, so that one assertion waits on its own barrier rather
than failing or passing.

**Milestone 6, the ceremony, built (2026-09-11).** `rhtn-client` added in
three gate-green commits: 8560119 (the pre-commitment and the capture key
against the corpus's known answers, the sealed capture store), bf0ed03 (the
query objects, selection by recognition, the verifier's and the subject's
sides, record assembly and the presentation), e142182 (the device interface,
the ceremony driver and its in-process harness). All 29 CER entries carry a
marker; 224 of 286 entries implemented, 0 flags; the ceremony stubs file is
gone. Three readings are the assistant's and open to reversal:

- **The seal binds the ceremony, not the record.** Design §7.5.2.6 says
  sealing happens at capture, before the record exists, and argues from
  that against deriving the key from `txid`; the store's associated data
  follows the same argument and carries subject, holder, pre-commitment,
  modality and template version, with the record only as the index the
  capture is filed under at finalization. §7.5.2.10 leaves the AEAD
  parameters open; AES-256-GCM, a random 96-bit nonce, and the template
  ahead of length-prefixed frames are the implementation's.
- **The engine is a stand-in.** Design §22.2 is undecided; the client's
  template is a hash of a frame's leading bytes, the profile is the
  template, and a comparison is byte equality, all behind one trait.
- **The verifier's operator.** Design §7.3 says a verifier's operator is
  not told they were sampled; design §19.6 and `light-client-requirements.md`
  §1.5 say a verifier is told, at the moment they are asked, that answering
  records them in someone else's evidence. Both are stated as obligations
  and they cannot both hold of the same moment. The client raises §19.6's
  disclosure as a notice naming no ceremony, subject or querier, and asks
  nothing; CER-17 records that reading in its interpretation and CER-26
  asserts the notice. Which the author intends is a question for him, and
  the answer changes one line in the verifier's query path.

**Milestone 7, recovery, built (2026-09-11).** Two gate-green commits:
adcfcda (the chain-holder's side: the series chain, abandoned lines, the
signed locator and the seal moved to the archive, the patron's check of an
adoption's evidence before it countersigns, a plain rotation's memo) and
afa41bf (the subject's side: sealing on rotation as the old key's last act,
seal-then-reissue on suspicion into a series never held, the chain
presented on request; and the recovery meeting on the ceremony harness,
the verifier its own querier, the adoption on both halves). All 14 REC
entries carry a marker; 238 of 286 entries implemented, 0 flags; the
recovery stubs file is gone. Three readings are the assistant's and open to
reversal:

- **Recovery recognises by personal knowledge only.** Design §9.1 has the
  recognition as the person's judgment, a stored photo supporting it; the
  reference client asks the verifier's operator the one question of the
  meeting and issues `personal_knowledge`, `met`. A photo-match recovery
  would need the verifier to open a capture sealed for the *prior* key
  against a query about the *new* key, which the grant and the response
  schema bind to one subject; not built, and whether it should be is the
  author's.
- **The recovery verifier's operator is asked.** Unlike an ordinary query
  (design §7.3, Appendix A.3), the recovery meeting asks the person whether
  they recognise the subject, since design §9.1 makes that judgment theirs.
  CER-17's reading of the ordinary case stands beside it.
- **How the chain is asked for.** `wire-format.md` §4.6.1 says the chain is
  presented on request; §9.2's request table carries no request type for
  it, and the archive fetch (§7.9) walks by back-pointers rather than by
  countersignature. The client and the node present the chain as records;
  on which stream the ask arrives is unspecified and left to the author.
  REC-10 asserts what is specified: the adoption and the reissues are
  presented, and nothing is pushed.

Found on the way: a signing identity carries its expanded ML-DSA key
inline, 66 KB, and a client that held one by value, plus a rotation
holding another, overflowed a debug test thread's stack once the recovery
path added depth; both are boxed now.

**Milestone 8, payload encryption, built (2026-09-11).** One gate-green
commit, 9360cd1: PQXDH in `rhtn-crypto` on RustCrypto's `ml-kem` and
`x25519-dalek`, both permissively licensed; the prekey objects in
`rhtn-archive`; the prekey service in `rhtn-node`, dispatched from the
runtime's request handler; the client's material, sweep, one-time request,
session, ratchet, routing and dispatch in `rhtn-client`. 14 of the 15 PAY
entries carry a marker; 252 of 286 entries implemented, 0 flags. Open for
the author, each recorded where it bites:

- **PAY-13 is not marked, and the library decision is what blocks it.**
  Design §14.2.4.3 adopts the Triple Ratchet, the Double Ratchet beside the
  Sparse Post-Quantum Ratchet with their outputs mixed. The Double Ratchet
  is built to Signal's specification; the post-quantum ratchet is
  libsignal's alone, AGPL (`Robot/implementation-plan.md` sections 3 and
  7), and writing one would be designing what §14.2.4 says to import. The
  entry also asks for agreement with the adopted library's published
  vectors; none are published for PQXDH or either ratchet, so the oracle
  is libsignal's own tests or nothing. Until the library is chosen, the
  session runs on PQXDH and the Double Ratchet, which is post-quantum in
  its agreement and classical in its ratchet.
- **The identity binding is classical.** Design §14.2.4.2 wants
  authentication to bind to the post-quantum identity component, past the
  deployed profile. `wire-format.md` §7.8 signs the bundle classically and
  bounds its blob at 4 KB; an ML-DSA-65 signature is 3.3 KB and an ML-KEM
  key 1.2 KB, so the hybrid signature does not fit inside the blob either.
  The bundle's classical signature binds the material to the identity; the
  design's wish and the wire's bound disagree, and one of them moves.
- **A sweep names at least two.** `wire-format.md` §7.8's batch is
  `2*256` keyhashes, so a client whose org has one other member asks for
  that one singly, reusable material only — the case PAY-07 cannot reach.
- **A batch is answered with an array of replies.** §7.8 defines the
  single `PrekeyReply` and no batch reply; the node answers a sweep with
  one `PrekeyReply` per subject named, in order, on the one stream.
- **What travels client to serving node is unspecified.** §7.8 says the
  patron serves bundles and one-time keys and tells the subject of
  exhaustion; no message publishes a bundle, uploads keys, or carries the
  notice. The harness moves them as values; the session will carry them
  as whatever the author specifies. A one-time request for a subject
  served elsewhere goes to that subject's serving node, which is how the
  harness routes it and what §7.8's "the patron serves" implies.
- **Randomness for the ratchet's keys comes from the device**, as
  everything else the client draws; the KEM's encapsulation randomness
  too, so a session is deterministic in its seeds and a harness can
  replay one.

**Milestone 9, the direct payload path, built (2026-09-11).** One
gate-green commit, 951e97c. The catalogue gained an area, `traversal`, with
six entries quoting design §14.1.1 and §12.6.3, `wire-format.md` §9.2 and
`infra-client-requirements.md` §7, since design §24 step 9b had none; all
six carry a marker; 258 of 292 entries implemented, 0 flags. STUN Binding
is RFC 8489's to the byte, checked against RFC 5769's sample
XOR-MAPPED-ADDRESS; ICE is RFC 8445's shape without its message set: host
and server-reflexive candidates, every pair tried at once, the first QUIC
handshake under the pinned key kept, and the relay on failure. Two readings
are the assistant's and open to reversal:

- **TURN is the relay the node already is.** Design §14.1.1 says the
  infra node acts as STUN and TURN and that relaying payload is what a TURN
  server does; the reference runs no RFC 8656 allocation protocol, and the
  relayed path is the serving node's delivery and mailbox (PAY-15, QUE).
  A TURN server proper would add a second relay of the same bytes under a
  second protocol; whether the author wants one for interoperability with
  ICE agents outside this implementation is his.
- **Candidates travel on the end-to-end channel, through the relay.** No
  wire object carries an ICE offer or answer; the design says the peers
  exchange addresses during setup, and the relayed path must work first.
  The harness carries them as a payload delivery; on a live client they
  would ride the ratchet session over the relay as a payload kind of their
  own, which is a demultiplexing decision already recorded as open under
  milestone 8.

Found on the way: the NAT emulator lives on loopback, where every address
shares one IP, so address-dependent filtering cannot be told from
endpoint-independent there; the tests use address-and-port-dependent
filtering for the hard case. And a serving node's STUN answers and a
leaf's outward dial share the leaf's NAT mapping only because both cross
the same emulated NAT; the leaf's two sockets — the one it serves and
dials peers on, the one it dials upstream on — are distinct, as they
would be on a phone.

**Milestone 10, resources, built (2026-09-11).** One gate-green commit,
738b535: the resource objects in `rhtn-archive`, the catalog service, the
gateway with its strict HTTP parse, the role table and the package host in
`rhtn-node`, the early-data proof in `rhtn-transport`, and the catalog
view in `rhtn-client`. All 24 RSC entries carry a marker; 282 of 292
entries implemented, 0 flags. Design §24's order is complete. Two readings
are the assistant's and open to reversal:

- **The package host is a binding table, not a runtime.**
  `infra-client-requirements.md` §9.2 expects a component-model sandbox
  and calls its effectiveness an open engineering question; the plan
  names `wasmtime` for when `rhtn-resources` arrives. The reference
  encodes the contract the sandbox must honour — two exports, the
  credentialled request and its response, and no binding for topology,
  liveness, the queue, prekeys or role inputs — as the host's export list,
  and refuses a manifest importing anything else. A backend is a trait a
  test stands in for. Whether a wasm runtime is adopted now is his.
- **A request to a brokered resource through the node is unavailable.**
  Design §11.7 and `resource-requirements.md` §3 have the client reach a
  brokered service itself; the node holds no backend to carry a
  `ResourceRequest` to, so one arriving is answered code 2 after the
  gates. No document says which code such a request gets, since none
  expects it.

Found on the way: the HTTP parser is the reference's own rather than a
crate's, because `wire-format.md` §11.2 asks for rejection of exactly the
tolerances a general parser exists to provide; and `ClientConfig::tls_for`
is public now, so a test can dial a resumed session and put a request in
early data by hand.

**Review of the sources after milestone 10 (2026-09-11).** A read of every
crate for gaps and dangling stubs, one gate-green commit, 42c4b56. No
`todo!`, `unimplemented!`, `#[ignore]` or unmarked catalogue entry beyond
the ten named last below. Found and closed:

- **Archive fetch was unserved.** `Archive::serve` existed since milestone
  3 and no runtime arm dispatched request type 2, so a prospective patron
  could not fetch from a live node. Dispatched to the node's own archive
  (`wire-format.md` §7.9; `light-client-requirements.md` §2); a subject
  the node is not gets an empty batch. A live test in `rhtn-sim` covers
  both.
- **The direct path's candidates had no payload kind.** Design §12.6.3
  exchanges them over the relayed channel; the client dispatched every
  unknown kind to the application. Kind 3 is theirs now, and the client
  hands them out as their own outcome.
- **The plan's crate table still promised `rhtn-resources` the catalog
  and the gateway**, which landed in `rhtn-node` and `rhtn-archive` at
  milestone 10. The row now says what that crate still owes: the sandbox.

Open after the review, each a decision or a milestone of its own:

- **Request type 4 to a light client.** `wire-format.md` §7.7.2 has
  resolution stop at the verifier's serving node, which identifies the
  attached client from the path suffix, and §9.2 has a bidirectional
  stream carry a request. How the serving node carries the query to the
  client it attaches, and the response back, is written in neither the
  documents nor the reference. `rhtn-client`
  verifies and answers a query the ceremony driver hands it; no runtime
  arm dispatches type 4.
- **The client's direct path is a device trait.** `rhtn-transport`
  gathers, punches and dials; `rhtn-client` asks a `DirectPath` object
  whether a peer is reachable. The join, where a client's candidates go
  out as kind 3 and the transport's socket answers the trait, is not
  built.
- **`rhtnd` and `rhtn`**, the daemon and the CLI, are unstarted.
- **PAY-13** stays open on the library decision, libsignal being AGPL;
  PRD-01 to PRD-09 are manual.

**Adaptors for both kinds of client (2026-09-11).** The author's answer
to the review's two open seams: "at least create the adaptors for local
resources for both types of client." One gate-green commit, 8922a72, a
new crate `rhtn-adaptors`: `rhtn-client` bound to what is local to its
process, for a light client beside its serving node and for a node that
is a participant alike. Two catalogue entries, CER-30 and TRV-07, carry
markers; 284 of 294 implemented, 0 flags.

- **The client on a thread of its own.** `Client` reaches its device
  through `Rc` and never moves; a handle carries closures across, from
  the transport's tasks and the node's request handlers.
- **A hosted verifier answers on the node's request stream.**
  `rhtn-node`'s `LiveNode` gains a slot for the verifiers its process
  hosts, and request type 4 goes there; nothing hosted, and the stream
  fails, as before. The adaptor registers each hosted client under its
  keyhash: a node that is a participant for its own key, a light client
  beside it for its own. A query awaiting its grant holds the stream up
  to the buffer bound; the grant arriving on the payload channel
  completes it early, and the bound passing lets the client's own expiry
  answer `unavailable`. The subject's copy travels as a payload kind of
  the reference's own, kind 4, beside kinds 0 to 3.
- **The direct path over the transport's socket.** The client's
  interface stays a yes-or-no per peer; the transport side sets it. A
  light client binds a socket of its own: it gathers on it, dials the
  peer's candidates from it, and accepts what a pinned peer opens toward
  it. A node that is a participant uses the node's own socket, which
  already gathers inside the horizon, dials and holds the path per peer.
  Candidates go out as kind 3 from the client's own send, over whatever
  path exists; a peer receiving them dials, and offers its own back.
- **The serving node beside the client** answers what the client asks of
  its serving node in-process: material published and stocked, a peer's
  fetched, payload relayed. A node reaches a neighbour for a subject it
  does not hold or a recipient it does not serve, and only for those, so
  two nodes each beyond the other never bounce a fetch between them.

Two things the wire does not say, kept as seams and recorded here for
the author:

- **A client's hand-off of payload to relay.** `wire-format.md` §9.2 has
  unidirectional streams carry payload delivery and queue drain, and the
  served session loop accepts none from an attached client; no frame
  submits material for relay, and nothing names the sender to the
  recipient, who decrypts under it. The adaptors frame relayed bytes as
  `[sender, bytes]` between themselves. An adaptor convention, not the
  wire's.
- **A serving node's leg to a client attached over the wire**, for a
  verifier query, stays as the review left it.

Found on the way: the light-client test gates the direct path on the
node's table, since a light client keeps none; whether a light client
should carry a horizon of its own is a question for
`light-client-requirements.md`.

**The application tier, stubbed (2026-09-12).** The author asked for the
file structure of the components the repository-structure assessment
named, and for the process extrapolated into the plan. One gate-green
commit, 4b19623. Three crates and two shells exist as scaffolds; no
catalogue entry changed, so 284 of 294 stands, 0 flags.

- **`rhtn-daemon`**, binary `rhtnd`: `config` carries the fields an
  operator must supply that a node cannot derive, with real types and no
  `Default`, since a default listen address or queue cap is a policy
  choice made by omission; `service` and `operator` are doc-only, each
  stating what it owes and against which section. `main` reports that the
  lifecycle is not assembled and exits non-zero.
- **`rhtn-cli`**, binary `rhtn`: `inspect`, `keys` and `probe`, doc-only.
  It answers to no obligation document, which is written into the crate
  doc so nothing later reads a subcommand as a requirement.
- **`rhtn-ffi`**: `client`, `device` and `types`, doc-only. One crate so
  the shells track one surface and a later split has one seam.
- **`mobile/android`, `mobile/ios`**: READMEs only, not Cargo members.

Package names are `rhtn-daemon` and `rhtn-cli` against the plan's table,
which named the crates `rhtnd` and `rhtn`. The binaries keep those names,
which is what the author specified; the packages follow the workspace's
`rhtn-*` convention. Reversible by renaming two directories.

Found on the way, and closed:

- **The plan's section 2.2 check did not exist.** It requires
  `Robot/modelrefcheck.py` to scan `rhtn/` so a renumbered section fails
  rather than leaving a stale citation. The checker walked `models/`
  only. Measured before changing anything: 637 code citations, 0 would
  flag, so the discipline had been kept by hand and turning the check on
  was free. It now reads `.rs`, `.py`, `.md`, `.toml`, `.kt` and
  `.swift`, skipping build output and the generated stubs. It caught the
  first citation written after it: `wire-format.md` §2.4 in the FFI
  crate, a section that does not exist. No document fixes a display form
  for a keyhash, so the claim was wrong rather than the number; the doc
  comment now says rendering is the shell's and that the two shells must
  agree.

Left as it stands, and worth the author's eye:

- **`Robot/refcheck.py` does not validate section citations inside
  `Robot/` files.** It checks citations within the five specifications
  and the rule that no root document cites `Robot/`; the `Robot/` list
  only supplies headings for the "resolves in" hint. The plan now carries
  112 qualified citations, checked by hand here at 0 flags. Extending a
  checker over `Robot/` would flag `review-tracking.md`'s as-of-filing
  references, which are deliberately not remapped, so the scope is the
  author's call and not the assistant's.

**The 2026-09-12 conformance review, applied (2026-09-12).** Twelve
findings against commit edb5cb3: F02 and F04 still open from the previous
round, R01 to R07 still open, and R08 to R10 newly identified. **All
twelve held on verification**: every cited path read as described and
every rule they cite says what the reviewer says it says. Applied in seven
gate-green commits, one per coherent area:

- **bdcedbf, the traversal assertion.** Not a finding but the reviewer's
  workspace failure: `mappings()[0]` indexed a `HashMap`'s values while the
  fixture held two sockets behind that NAT. `Nat::mappings_for` filters by
  inside address, and the assertion names the socket it is about. Fixed
  first so the gates behind it were worth reading.
- **35d78bd, R02 and R03** (client payload). The initial key came from the
  message's identity key and the ratchet was stored under the name the
  channel gave, unchecked against each other. The binding is the prekey
  bundle, which its subject signs. A one-time pair is now used and spent
  after, not before. PAY-16, PAY-17.
- **9f9a5ee, R01** (the HTTP boundary). A header value could carry a bare
  LF and be copied verbatim into the forwarded message, writing a header
  line of the caller's own. A chunk length was added to an offset
  unchecked. RSC-25, RSC-26.
- **bf4ae83, R04 and R10** (the adaptors, both written the day before).
  `gather` asked the local gate and `open` did not; the accept did not
  either. The verifier's reply channel was registered before the requester
  was checked, so a replayed body cancelled a legitimate request. TRV-08,
  CER-31.
- **267feab, R07, R08 and F04** (the node). Directional scopes escaped the
  owner's Dunbar Org; the boundary is now taken once, before the scope is
  read. `connect` and `discover` could be declared, assigned and emitted.
  An ending cleared a slot by identity without asking which binding closed.
  RSC-27, RSC-28, PRP-21.
- **fa36b72, F02, R09 and R06** (validation). The transfer's three keys
  were never compared; a presented chain was never held to §3.3's
  chronology although the archive's append path was; a late response was
  tested against consent flattened across ceremonies. DEC-27, ARC-19,
  CER-32.
- **2c09b52, R05** (frame and dispatch). The generic decoder applied the
  body schema before dispatch, so the gateway's own "code 3" branch was
  unreachable and a malformed resource body reset the stream. `parse_outer`
  separates the outer frame from the body's schema, and the fallback is
  scoped to request type 6, since no other family has an answer defined for
  a body that does not decode. RSC-29.

Three readings are the assistant's, each open to reversal:

- **A recipient holding no binding for a sender refuses its first
  message.** R02 admits a pending path or a refusal; refusal was chosen
  and the author approved it. The cost is real: a peer who published after
  this client's last sweep loses its first message. The client records the
  peer as wanted and routine maintenance asks for the binding, so the
  second attempt is attributable. A pending-and-retry path would recover
  the first, at the cost of holding ciphertext it cannot attribute.
- **`Client::sweep` no longer drops the caller's serving node.** A node
  can be a participant and send payload of its own, which is exactly the
  hosted verifier, and the population is the caller's to choose.
- **A reserved role is refused where the operator writes it**, not
  filtered where it is emitted, so a mistake is visible rather than silent.

The author's ruling this round: **refuse but record**, for a late response
whose record is held without its seed. Recorded as the verifier's keyhash
against that record, not the bytes, since retention follows the record; and
only where the response verified, a forgery being no signal.

Numbers: twelve entries added, 309 in all, 297 of 307 implemented, 0 flags;
each commit's gate green. Ten entries remain unimplemented: PAY-13 on the
library decision and the nine manual product entries.

**The reviewer's harness rerun (2026-09-12).** All 38 of its assertions
pass on a copy in the scratchpad, its own directory untouched. Two calls
were adapted, both because a repair changed the surface the reviewer's
correction asked to change:

- **R06's** call passed `all_consented()`, the flattened set. The fix
  replaced it with consent by ceremony, which is what that finding's
  correction asked for; the adapted call passes
  `consented_by_ceremony()` and still fails to attach, as it should.
- **R03's** fixture received on a `Sessions::default()` holding no
  bundles, so R02's repair refuses it before the one-time key is reached.
  The adapted fixture prefetches the sender's bundle; the corrupted copy
  then fails and the original still opens, which is what R03 asserts.
  Neither adaptation weakens an assertion, and both defects have workspace
  regressions of their own in PAY-16 and CER-32.

**Milestone 11, the daemon (2026-09-12).** Six commits, the last gate-green,
c6f3b1b. `rhtnd` starts from a configuration file and serves; the exit
criterion's restart half is proved against the real binary. Two catalogue
entries, DMN-01 and DMN-02, under a new `daemon` area; 299 of 309, 0 flags.

- **The configuration fixes no dependency.** Section 7 lists the choice of
  a format as the author's, so the placeholder is line-oriented
  `key = value` parsed in a few dozen lines. It is strict for the reason
  the HTTP boundary is: an unknown key, a repeated key, a missing one or a
  value out of the range the wire fixes is an error naming its line.
  Swapping in TOML or JSON replaces `Config::parse` and nothing else.
- **The identity is read and never minted**, and on Unix one readable
  beyond its owner is refused. The file holds the two seeds an identity is
  derived from, sixty-four bytes; no document fixes that format and it is
  the reference's, open to reversal.
- **Consumable state is read before the transport can accept a session.**
  A node serving before it has loaded its one-time pools reissues a key it
  already served; one serving before its topology store replays a
  forwarding wave. Both are written back on a signal and on a tick, so a
  kill that never reaches the handler loses at most one interval.
- **The operator's three views** are data plus a plain-text rendering. The
  role view cannot leak a predicate by accident: a materialised row does
  not record what produced it. The daemon prints the exposure at startup.

Two gaps in the crates beneath, closed here because nothing worked without
them:

- **`NodeConfig` had no listen address.** `LiveNode` bound
  `127.0.0.1:0` unconditionally, so an operator's chosen address could not
  reach it. Absent still binds ephemeral loopback, which every existing
  test relies on. The outward dial socket now takes an ephemeral port on
  the same interface, so a host with several does not dial out of one it
  was not given.
- **An `Identity` could not be built from `KeyMaterial`.**
  `Identity::from_key_material` is the inverse of `key_material` and the
  way a holder turns material it was handed into keys it verifies with. A
  keyhash alone cannot be pinned and there is no fetch path for material a
  node lacks, which is why peers are configuration.

Left owed, each recorded rather than invented:

- **Telling a subject its one-time pool ran dry.**
  `infra-client-requirements.md` §6 obliges it and no wire object carries
  it; §7.8 says the session does, without saying how. The daemon drains
  the list and reports it locally. A frame for it is the author's to
  specify.
- **Re-evaluating predicates at the four moments** §10.2 names. There is
  no scheduler and no horizon-change pass; that is `rhtn-resources`'s, not
  the daemon's lifecycle.
- **The exit criterion's other half**: rerunning `rhtn-sim`'s whole
  scenario set against daemon processes needs a harness that spawns and
  addresses several, which is its own piece of work.
- **PRD-06** stays open. It is manual, and where the binding and role
  views are surfaced is not settled.

Three workers ran on this: the assistant on the lifecycle, one agent
surveying every operator obligation against what `LiveNode` already does,
and one writing the operator views. The survey found the listen-address
gap before any code was written, which is what made it worth spending a
worker on.

**The 2026-09-12 review, second round, applied (2026-09-12).** Five
findings against ad1ae25: F04 and R04 reported partially resolved, and
D01 to D03 newly identified in the daemon written that day. **All five
held on verification.** Ten of the previous round's twelve stayed closed
and the workspace suite came back green, the traversal assertion included.

- **d88d5a7, F04 and R04.** Both earlier commits fixed the branch their
  reproduction exercised and left the same defect on a sibling branch.
  F04's adoption branch read only the binding of the record in hand, so an
  adoption already ended arriving last cleared a slot a live binding held;
  the question now lives in one place, `still_bound`, and every branch
  asks it. R04's node path never asked the local decision at all:
  `LiveNode::open_direct` asks it now, being the entry every caller
  reaches, and `NodeConfig` gained `accepts_direct` so a connection this
  node would not have dialled is closed before anything on it is read.
  PRP-22, TRV-09.
- **06e0ccf, D01 and D03.** The daemon loaded the topology store and
  assigned it without deriving anything from it, so a restart held
  relationships it could not route on and resubmitting the records could
  not repair it, the store answering `Duplicate` before the derivation.
  `NodeView::rebuild_from_store` folds the stored records oldest first
  through the same path `take_object` uses after it has decided to store,
  forwarding nothing, and takes this node's own position from the binding
  its patron countersigned rather than assuming a root. The daemon's own
  public key is in the verification lookup and not because a peers file
  listed it: a node countersigns its subordinates' adoptions, and asking
  an operator to list themselves makes a working configuration depend on
  remembering to. DMN-03.
- **6cc2078, D02.** One-time keys were consumed in memory and written on
  a tick, so a stop between snapshots returned a key already served. The
  pool is now kept rather than snapshotted: each key is written as it is
  stocked and unlinked as it is served, before the reply goes out, and a
  key whose file cannot be removed is not served at all. Bundles are
  written through for the same reason, a pool whose bundle is missing
  being a subject a restart drops. `save` no longer wipes the tree, which
  would have put served keys back. DMN-04.

Recorded rather than fixed: **rate-limit counters stay snapshot-based**, so
a stop can reset a requester's window. The window bounds it and a disk
write per request does not earn its cost. Reversible if that trade is
wrong.

Found on the way: the two daemon commits were gated together rather than
one each. Batch three's edits were made while batch two's gate was still
running, which left that verdict ambiguous, so the tree was gated once
more as a whole and both commits taken from that run.

The reviewer's harness rerun at the end: **43 of 43 pass on an unmodified
copy in the scratchpad**, the five new assertions included. No adaptation
was needed this round, the repairs having changed no surface the harness
calls.

**Milestone 12, the command line (2026-09-12).** Three commits, the last
gate-green, 69df096. Both halves of the exit criterion are met: every
byte-class corpus entry decodes and prints from the binary, and a probe
resolves, fetches an archive and queries a catalog against a running node
over a real session. DMN-05 and DMN-06; 305 of 315, 0 flags.

- **`inspect` decodes with the parser a node uses and no other.** It
  prints the shape in diagnostic notation, an envelope's derived txid,
  type and signers, and the verdict under whatever kind the caller says
  the object is. A refusal is a line of output rather than an error,
  because the reason the strict decoder gives is what the reader came for.
- **`keys`** mints an identity readable by its owner alone, refuses to
  replace one in place, never prints a private half, and reproduces the
  seeds the vectors derive so a mismatch shows from one command.
- **`probe`** attaches and sends one of the three request types §9.2 calls
  read-only. Nothing else is reachable from it, and the ask is checked
  before anything is read from disk or dialled.

The argument parser is hand-rolled, as the daemon's configuration format
is and for the same reason: section 7 lists the choice of one as the
author's, and taking none leaves it open. About sixty lines, reaching only
what three subcommands need.

Two defects of the assistant's own, found by the corpus test rather than
by reading:

- **The envelope lines read the wrong fields.** They guessed at map keys
  and printed one record's txid for another and "adoption" for every type.
  The envelope's own parser gives both, and is used now.
- **Every signed object came back "refused".** The doc comment claimed
  §3.4's distinction between unverifiable and failing while the code
  folded a missing key into a refusal, for envelopes and presentations
  though not for standalone records. The presentation verifier reports in
  prose rather than a typed failure, so the envelope inside it is asked
  first and its typed answer is the one printed.

Found on the way: `verify::presentation` returns a `String` where
`verify::envelope` and `verify::record` return a typed `Failure`. The
command line works around it by asking the inner envelope. Making the
three agree is a small change nobody has needed until now, and is left
for the author to want.

**The three verifiers report the same way (2026-09-13).** One gate-green
commit, 1c90b9c, on the author's ruling: make them consistent.
`verify::presentation` returned a `String` where `verify::envelope` and
`verify::record` return a typed `Failure`, so a key the holder lacked
reached a caller as prose. §3.4's distinction between an object that
cannot be verified here and one that is wrong survives only if the type
carries it; folded into a string, every caller matches on wording.

The change was the return type and one line. Every error path inside the
function already converted through the `From<&str>` and `From<String>`
impls the failure type carries, so nothing else moved. Three callers: the
two corpus runners flatten it to prose where they compare prose and say
so, and `rhtn-cli` asks it directly, the workaround that reached past it
into the inner envelope now gone.

A test under DEC-26 holds all three to it: a presentation verified with no
keys held names the same missing signer the envelope inside it does, and
one whose root does not match is invalid rather than unverifiable. 305 of
315, 0 flags.

Left as it stands, and worth the author's eye: **`verify::record` returns
`Ok(false)` for a signature that fails** under a key the holder does have,
where `verify::envelope` returns an error for the same thing. The error
types agree now; the success types still disagree about what a failing
signature is. Seventeen call sites read that bool, so it is a wider change
than this one and nobody has needed it yet.

**The 2026-09-13 review, applied (2026-09-13).** Four findings against
b2901b3: D01 half open, R11 a regression this assistant introduced, and
R12 and C01 new. **All four held on verification.** Everything else from
the previous round stayed closed and the workspace suite came back green.
Four gate-green commits, one per finding, in the order the report proposed.

- **9873eba, R11**, and the reason given for causing it was wrong. D02's
  repair said wiping the prekey directory before writing "would have put
  served keys back". It would not: a rewrite from memory cannot restore a
  key memory no longer holds. The real hazard of a wipe is the window in
  which the directory is empty, which is a different argument, and what
  shipped instead added missing files and removed none. `save` now has one
  rule whichever way the service is kept: make the directory match what is
  held. DMN-07.
- **491c0de, R12.** The topology table had arms for adoption, departure
  and disavowal and none for a series reissue, so a reissue fell through
  doing nothing, the binding kept the adoption's series, and a departure
  naming the proven current series matched no binding and waited for ever.
  The series left identifies the relationship a reissue advances, not the
  larger number, so a re-adoption opening a higher series is untouched;
  a reissue arriving before its adoption is held as a departure is. TOP-20.
- **2e8a455, D01.** The rebuild left `view.archive` empty, so a restarted
  node derived its next back-pointer from genesis and the next record it
  signed would have opened a second chain beside its published one, every
  record still on disk. And the own position took the adoption's series
  rather than the binding's, so startup could publish an endpoint record
  under a series the patron had retired. DMN-08.
- **897ad33, C01.** The archive probe printed each record's identifier and
  returned success without comparing one record's back-pointers with the
  next. §7.9 is explicit that the requester verifies the chain and that a
  holder cannot be trusted to have walked correctly. The batch's links are
  checked now, and the reply says that newestness is the holder's claim
  where no head was requested, which is the one thing no requester can
  check. DMN-09.

Three of the four tests were confirmed to fail with their repair backed
out before being kept. The fourth, R11's, is a persistence sequence the
old code could not express.

Recorded rather than fixed: **the own archive is rebuilt only as far as
the store still holds each record's predecessors.** A record whose
predecessor was never stored, its subject having fallen outside the
horizon, is left out rather than appended over the gap, and the daemon
counts and reports those. Persisting the archive separately from the
topology store would close it and is a larger change than this one.

The reviewer's harness rerun at the end: **49 of 49 pass on an unmodified
copy in the scratchpad**, the five new assertions included. No adaptation
was needed.

Found on the way, a process slip of the assistant's: C01's first gate ran
against a tree whose clippy had already failed. The lint check and the
gate were launched in one backgrounded chain, so its verdict was never
read and the `&&` carried on past it. Clippy is checked and read before
the gate is launched, not beside it.

**A key's own archive is its own state, and milestone 13's boundary
(2026-09-13).** Two gate-green commits before this one and fd3efdb with
it; 310 of 320, 0 flags.

**The archive ruling** (a05b361). The assistant had the daemon derive its
archive from the topology store on restart, and recorded the resulting
gap as a limit. The author's ruling is that the premise was wrong: **a
key's own archive is not its topology store and is not derived from one.**
The store is a seen-set of what a node accepted about others and its
horizon bounds it; the archive is the key's own signed history from its
first transaction, which nothing prunes. `Archive` gained `save` and
`load` of its own, one file per record named by its txid, heads recomputed
on load rather than written. The configuration names an archive directory,
startup reads it and shutdown writes it. The rebuild from the store is now
only what is genuinely derived: the table, the slots and the own position.
DMN-08 was revised to the ruling rather than left describing the reading
it replaced, and the gap it recorded is gone, nothing being derived.

**Milestone 13, the boundary** (92035ba, fd3efdb). The facade carries
value types with no borrow and no generic, the platform's six objects
inward, and the client's operations outward. The one piece of real work is
that a shell hands over objects usable from any thread while the client
reaches its device through `Rc` and never leaves the thread it runs on, so
each object is wrapped once inside that thread. Three rules are kept at
the crossing rather than assumed: pixels come inward and nothing the
pipeline attached comes with them, the platform's clock is the clock, and
randomness of short measure is refused rather than padded. DMN-10.

Found on the way, and fixed: **`Handle::spawn` panicked when the build
failed**, so a platform that failed on the client's thread reached the
caller as a dead channel. Across a language boundary that is a process
that vanished rather than an answer, so the spawn reports it and the
facade turns it into a refusal carrying its reason. The two other callers
were unaffected.

Two readings are the assistant's, open to reversal:

- **An intent crosses as fields, not as bytes.** What two present devices
  tell each other when a ceremony opens has no encoding any document
  fixes, so the shell carries it however the two manage and rebuilds it on
  the other side. Giving it a wire encoding would be inventing one.
- **The biometric engine is not the shell's to supply.** Design §22.2
  leaves the real one open and the reference recognises nobody, so the
  platform object list has six members and not seven.

**The exit criterion's other half waits on the author.** No binding is
generated because no generator is adopted; section 7 lists the choice as
his. The facade is shaped for one whichever is chosen.

**What a client hands its serving node** (4122220). §7.10 and request types
9 to 12 went in end to end: the objects in `rhtn-archive`, the four frame
types and the wake registration's shape in the codec, the node's handlers
and its wake register, and `AttachedNode` on the client side. `Serving`
became asynchronous, because one of its two implementations now waits for
a node to answer.

Two things the work produced that were not asked for:

- **`RelayedPayload` had to be written.** A node taking a submission
  composes what the recipient later collects, so the shape stopped being
  one implementation's private business the moment the submission became a
  message. §7.10 carries it, and says the name in front is a routing hint
  and not an attribution.
- **A deposit's keys were being stored with their CBOR headers.** The
  decoder walked item ranges where it wanted contents, so every one-time
  key served would have been six bytes of item rather than the key. The
  first end-to-end test caught it, which is the argument for the test
  running against a real node over the wire rather than against the
  service in memory.

**Owed: the corpus is not regenerated.** The generator gained
constructions for the four frames, the withdrawal, two submission replies
and the relayed payload, but `test-vectors/tools/spec-pins.json` names
`dilithium-py 1.4.0` and `cryptography 50.0.1`, and this machine has
neither the first nor that version of the second. Regenerating under
different dependencies is the substitution the pins exist to catch, so the
run is owed on a machine that has them, with `--accept-spec-change` after
the constructions are audited against the specification diff.

**The derived view is stored as derived.** design §15.1.1 states the three
rulings; `infra-client-requirements.md` §4.3 and
`light-client-requirements.md` §4.2 carry the two sides. The node side is
implemented: `Table::materialise`/`from_materialised`, `Snapshot` with a
watermark, and `NodeView::restore_materialised`, which folds only what
sorts above the watermark and replays the whole store whenever the counts
do not add up. The daemon writes it into the topology directory after the
store, so it is never the newer of the two. DMN-11 and DMN-12.

**Owed: the light client's side of §15.1.1.** The client crate holds no
topology at all — nothing reads `Session.frames` on the client side, so a
light client currently learns nothing its patron propagates. That is the
next piece: a horizon in `rhtn-client` fed by the propagation, materialised
the same way, answering `locator` and `distance` without asking anyone.

**The client's side of §15.1.1.** `rhtn-client` now has a `horizon` module and
`Client` holds one. It keeps the records its serving node floods, folds them
into a table of its own, and answers `place` and `distance` from that table
with nobody asked. `Table` gained `distance`, which is the walk that defines
the horizon returning the round it found a party in, so membership and distance
cannot disagree. `topology::unfolded` is the watermark test, shared by the node
and the client rather than written twice.

One reading, open to reversal: **where a node sits is a place and not a
locator.** A resolution needs the anchor and the path (`wire-format.md`
§7.7.3); the series and counter belong to the record that carried them. The
anchor of a subtree has no adoption of its own in the client's records, so it
is placed at the empty path under itself and claims no series. Claiming one
would be asserting something nobody propagated. Both are kept: `place` for
every node, `locator` only where a record carried one.

**Not yet wired: nothing in production calls `attached::follow`.** The function
carries stream 0's pushes into the horizon and TOP-24 exercises it over a
channel, but no attach path in `rhtn-adaptors` invokes it, because there is no
attach path for a light client there at all — the tests attach by hand. That is
the same gap the FFI defect names, and both close together when `Participant`
owns its transport.

**Still owed:**

1. **`verify::record` returns `Ok(false)` where `verify::envelope` errors**, 17
   call sites, recorded and unfixed.
2. **The corpus**, as above: owed on a machine with the pinned dependencies.

**The FFI kernel, done.** `Participant` owns a runtime, an endpoint, the
session, the courier and the two readers; `attach`, `maintain`, `send`, `wake`
and `next_event` are the surface, and none of them carries an encoding.
`attached::follow` and the new `attached::collect` are called from there, which
closes the other half of the gap the client's horizon left open.
`light-client-requirements.md` §9 and `infra-client-requirements.md` §8.1 state
the interface, which design §14.1.0 puts in the client documents. DMN-13.

Two readings, open to reversal:

- **The runtime is shut down in the background on drop.** A tokio runtime
  dropped inside an asynchronous context panics, and a shell cannot know
  whether the thread it releases a participant on is inside one. The cost is
  that what was in flight is abandoned, which is what releasing a client means.
- **No direct path from the facade.** design §14.1.1 gathers candidates on a
  socket, and this endpoint dials and is never dialled, so the relay carries
  everything. Adding it means giving the facade a listening socket, which is a
  decision rather than an omission.

**A defect found on the way, and it is the largest thing in this stretch.**
`TraversalSocket::poll_recv` flattened the kernel's receive offload: several
arrivals in one buffer became one datagram with the stride rewritten to the
whole length, so the first packet of each batch was delivered and the rest were
lost. Nothing was ever wrong, only slow, which is why no test had caught it —
every existing message fits in one datagram. Measured against a node on
loopback:

| bytes | before | after |
|---|---|---|
| 1,000 | 1.8 ms | 1.8 ms |
| 6,000 | 363 ms | 2.4 ms |
| 25,000 | 35.3 s | 3.9 ms |

The fix passes an untouched batch through with the stride it came with, and
takes a batch apart only where a STUN datagram or the harness's own wrapping
means it has to. TRV-10 is the regression test, and **it was checked against
the old code**: it fails there at the 25,000-byte case and passes on the fix.
The first version of that test did not fail against the old code, because the
way it reintroduced the bug missed the branch that carried it.

**A recursive consistency pass, 2026-09-13.** Two audits ran against the tree,
one over the six documents and one comparing every CDDL block to the schema and
the encoder. Both were verified finding by finding before anything was changed;
what follows is what held.

**The documents.** Nine counts had drifted, each checkable against the thing it
counted. The load-bearing one was `wire-format.md` §3.3, which defined
*effective time* twice, differently, inside a MUST — and the implementation had
always read it the way the second clause meant, so the fix was to name the two
roles rather than to change either. A locator was described as four fields in
§12.1 and three everywhere else. Two consolidating passages asserted the
opposite of what they summarise. Fifteen citations resolved to a real but wrong
section; the witness-clock rule alone was misattributed to design §8.1.2 in four
places. And thirty-four instances of *Dunbar Org* stood in mechanical text
against §2's ruling, swept across all five documents at once as
`Robot/authoring-conventions.md` requires.

**The implementation.** One outright interoperation failure and one silent
value corruption, both invisible to any round trip:

- `PrekeyPublication` field 1 wrapped the bundle object in a byte string where
  §7.10 declares the object. A decoder wrong the same way in both directions
  agrees with itself, which is why nothing caught it and why the test that pins
  it now asserts the *shape* rather than the round trip.
- `Locator::decode` narrowed a u32-range `seqno` with `as u32`.

The rest was under-enforcement: bounds the documents state, and in several
cases that `negative-vectors.md` already described as required, which the
schema did not apply. Presence-record responses are the clearest case — the
recovery form checked sorting and duplicates and the presence form did not, an
asymmetry rather than a decision, and field 5 sits inside the signed body so an
unsorted encoding gave one logical record two txids.

**Two things found while fixing, not in either audit:**

- `reply_family` existed twice, in `crypto/tests/corpus.rs` and in
  `crypto/tests/common/mod.rs`. Adding a family to one and not the other is
  exactly what happened, and two tests panicked on a corpus entry the third
  accepted. The corpus runner now uses the shared table.
- `verify.py` resolved `corpus.json` against the working directory while every
  other read resolved against the script, so it only ran from one place.

**The test vectors were regenerated** under the dependency versions
`spec-pins.json` names, installed into an isolated directory rather than over
the system's. `verify.py` now reads the message fixtures **by section rather
than by absolute index**, so adding a family to one section cannot silently
renumber another's, which is what the five new frames and two new replies would
otherwise have done. All checks pass.

**The models were re-run and all pass**, unchanged in verdict.

**Owed, and the list is now short:** nothing. The `verify::record` error type is
fixed, and the corpus debt is paid.

**The 2026-09-13 review, second round, applied (2026-09-13).** Eight findings
against the tree after the consistency pass: S01-S05, H01, H02 and B01, with
a Rust harness that reproduces each. **Seven held on verification and one did
not.** Four gate-green commits, in the order the author approved.

- **74f35e6, B01.** The reviewer read design §14.1.0's "never envelopes,
  never signatures, never key material" as forbidding `Participant::consent`
  to return an encoded COSE signature across the FFI boundary. Put to the
  author, who ruled the sentence **was not his wording** and overstated the
  rule: where the kernel must pass a cryptographic payload through the UI it
  may, and he named the cases — the ceremony's QR display and optical return,
  seed material from a device entropy source, a user-input secret at
  initialisation, and ingesting a contact from a QR or a contact card. What
  the encapsulation is for is keeping the network stack and device-specific
  services out of each other. §14.1.0 was rewritten in those terms, and
  `light-client-requirements.md` §9 and `infra-client-requirements.md` §8.1
  with it. **B01 dissolves as a conformance failure**; what survives is a
  shaping question — whether the facade should offer a named interaction
  rather than a general byte pipe — which is not a defect and is not urgent.
- **2c1edaa, S04 and S01.** Issuance kept a per-requester counter file naming
  the subject asked for, which `infra-client-requirements.md` §6 forbids
  keeping at all; the counters are gone rather than expired, since a record
  that expires is a record. And `stock` reported success when a key failed to
  write, so a node acknowledged an accepted deposit it had not taken. It is
  now `#[must_use]`, rolls back the keys it did write, and the submission
  path answers `SUBMISSION_REFUSED`.
- **5cbd1d8, S02, S05 and S03.** A relationship ending now forgets the wake
  endpoint with it, in the one place both the unbinding and the departure
  paths pass through. `save`'s cleanup walked its own directory and tried to
  descend into the files in it. And the FFI `send` reported success for a
  submission the serving node refused: the courier now carries back what was
  refused and what was left, and the shell is told.
- **dd22e70, H01 and H02.** A materialised snapshot was accepted on a count
  and a high-water mark, which two different record sets can share, and from
  any identity at all. It now carries a fold over the txids it was taken from
  and is refused if it names another node. And a client's horizon placed and
  resolved without bound, so replaying retained records resurrected a place
  the horizon no longer holds; `place`, `locator` and `resolvable` are now
  bounded at distance two.

**The reviewer's own suite was rerun** rather than only the workspace's: 57 of
its 58 tests pass, the one failure being B01's, which asserts the sentence the
author withdrew.

**Milestone 11's exit criterion, met (2026-09-13).** One gate-green commit,
a2ab981. `rhtn-sim` gained `Daemons`, which spawns several `rhtnd` processes,
writes each an identity at 0600, a peers file excluding itself and a
configuration naming its upstream, reads the address back off the process's
first line of output, and can stop and restart any of them by name. Two
scenarios run against it and neither reads a view: every claim is made over a
session or from what a process wrote. DMN-17 and DMN-18. Catalogue 340 of 350,
0 flags.

- **The finding was in the library, not the daemon.** `NodeView.attached` was
  populated by no production code — only by tests reaching into the view — so
  a running daemon forwarded the flood to none of its clients whatever its
  configuration said. `wire-format.md` §10.1.1 counts "the clients attached to
  you" among the adjacencies a stored transaction goes to, so this was silent
  under-delivery to the parties a serving node exists for, and no in-process
  scenario could have shown it because every one of them set the field itself.
  The transport calls an `on_attach` hook as a session is inserted and as it is
  removed; the runtime installs one. PRP-23.
- **What DMN-18 leaves open, and why it is not an omission.** `mark_infra` is
  called in one place, by `NodeView::new` for the node itself. Nothing in a
  running node marks another node as infrastructure, so the nearest infra
  ancestor a table can see is always the node itself and it answers `Serving`
  for itself with the residual that identifies the target. `wire-format.md`
  §7.6 says only infra nodes publish endpoint records, which is the only
  documented signal, but reading it as *the* marking rule is a protocol
  decision and the plan's section 2 puts those outside this tier. **Put to the
  author.** The scenario asserts the reply's shape — the nonce it was asked
  with, a residual and endpoints on a serving answer, a positive advance and
  endpoints on a referral, a stated failure for a path held no record of — and
  the catalogue entry carries the same in its interpretation.
- **Two documents corrected on the way.** `wire-format.md` §7.7.3 repeated the
  word across a wrapped citation, reading "(design design §12.2". The
  reference checkers pass either way, since they resolve the section and not
  the prose. Committed on its own; **that commit's message misnames the
  section as §7.7.2**, and is left uncorrected rather than rewriting pushed
  history.

**Still owed above the library.** Milestone 14, the mobile shells, is blocked
on two decisions from section 7 — the mobile framework for the ceremony track
and the binding generator, with `uniffi` the candidate — and on a toolchain:
no gradle, swiftc, adb or xcodebuild exists on this machine. PAY-13 waits on
the payload-library licence decision. B01's residue is a shaping question, not
a defect: whether the facade should offer a named consent interaction rather
than a general byte pipe.

**`rhtn-resources`, the sandbox, and the daemon's hosting (2026-09-13).** Two
gate-green commits, 966c553 and 4b2da5e. This is the last thing the library
owed that was not waiting on a decision; what remains of that list is PAY-13
and the licence. Catalogue 349 of 359, 0 flags.

- **What the crate enforces is an absence.**
  `infra-client-requirements.md` §9.2 does not ask for narrow scopes
  carefully granted — *the hooks do not exist* — so `Sandbox::admit` is
  mostly a list of what a package may not ask for. The check on the
  functions **inside** the offered instance turned out to be the
  load-bearing half: without it a package asking this host for `topology`
  is admitted and fails later at link time, which is a worse answer to
  give an operator than a refusal naming the hook.
- **Every check was verified by breaking the thing it tests.** Removing
  the instance check, removing the hook check and disabling fuel each fail
  the test that claims them, and the memory ceiling was confirmed to track
  the number it is given at three different values rather than stopping
  somewhere of its own. One probe was inconclusive rather than failing —
  a hundred fuel units is enough for the echo package, because the
  canonical ABI's copying is the host's work and not the guest's — and the
  loop test is what actually pins fuel.
- **`Gateway::bind` had no production caller**, so `view.resources` was
  empty at every node and every resource request answered refused. The
  same shape as the attach hook earlier in the day, and found the same
  way: by running the thing end to end against a process rather than
  against a view a test had filled in.
- **A `host` line names a manifest rather than a component**, because §9.1
  puts the capability declaration in the manifest that ships with the
  package. An operator writing role names into their own configuration
  would be declaring them on the package's behalf, and `Gateway::set_row`
  already refuses a role the binding does not declare — which is how the
  first cut failed, and the failure was the design telling me where the
  declaration belongs. The manifest and the component are now checked
  against each other in both directions.
- **The supply chain is not implemented and is not claimed.** §9.1 names
  signing, provenance and an update channel and calls them a distribution
  problem rather than a protocol one. A manifest that agrees with its
  component is not a manifest anybody vouched for, and the plan says so
  where it records the crate as built.
- **`wasmtime` 48 is the dependency**, which section 3 had already chosen;
  it is Apache-2.0 WITH LLVM-exception, so no licence question arises.
  Compile cost: a cold build of the crate is about 40 seconds on this
  machine and the gate's wall clock is visibly longer than before.

**Endpoint records mark a node infra, on the author's ruling (2026-09-14).**
One gate-green commit, f6069e1. The question DMN-18 was left open on is
closed: `wire-format.md` §7.6's *published by infra nodes only* is the
marking rule, and holding a record is what tells a node. Catalogue 350 of
360, 0 flags.

- **The mark follows storage, not the wire.** `accept_endpoint` already
  refuses a record for a node more than two edges out, so the horizon
  bounds the set without a second rule. Derived rather than kept: a rebuild
  from the store reaches it again, and the rebuild restates the node's own
  mark because replacing the table is the one operation that could lose it.
  No unmarking — §7.6 gives a record a successor and no retraction.
- **Two more missing callers under it, and this is now three of the same
  shape in two days.** `replay_to` had no production caller, so §10.1.3's
  reconciliation never ran: an endpoint record published before any session
  existed reached nobody, and two parties that connected after their records
  were made never exchanged them. A session coming up now replays both ways.
  The pattern in all three — `Gateway::bind`, `NodeView.attached`,
  `replay_to` — is a library function that is correct, tested, and called
  only by tests. **Worth a sweep of its own**: what else in `rhtn-node` and
  below is public, exercised, and reached by nothing that runs.
- **The periodic half of §10.1.3 is not implemented.** It asks for a
  periodic reconciliation with siblings and the patron; the interval is an
  operator's number and no document states one.
- **DMN-18 now starts N after N has been adopted**, which is the order a
  deployment has. It failed the other way round for a real reason rather
  than a timing one: a record for a node the patron has never heard of is
  outside its store reach, is dropped, and nothing re-offers it. Writing the
  scenario in the deployment's order was the fix, not a sleep.
- **One assertion was weakened and re-verified.** `sim`'s PRP-01 asserted P
  received zero topology frames; it now measures from what N reconciled at
  attach. Forwarding back to the arrival peer still fails it, which is the
  claim it was written for.

**The operator's two files moved to TOML (2026-09-14).** One gate-green
commit, 8bb6d4d, on the author's decision. Catalogue 350 of 360, 0 flags.

- **The two files stay two, on a different argument.** The repeated-key
  reason for splitting them is gone with the format; what survives is that
  packages and grants are what an operator regenerates, and doing so should
  not mean rewriting the identity and the paths beside them.
- **A grant nests inside its package**, which removes an error rather than
  catching one: a grant naming a resource nothing hosts is not expressible.
- **Hand validation is only what a document fixes**: the heartbeat's range,
  an allowance of zero, a keyhash's form, the ingestion boundary, a package
  budget of zero. Those fields read through `toml::Spanned`, so a refusal
  still names a line. Deserialisation's own messages are better than what
  they replace — an unknown key now comes with the list of keys expected —
  and only the missing-field wording is rewritten.
- **The manifest is untouched.** It is the package's file and what §9.1's
  signing will sign; a signed object wants a canonical encoding and TOML
  has none. If signing lands, the choice is canonical CBOR or a custom
  section inside the component, so there is one artefact to sign.
- **One timeout was raised, and it took some care to be sure that was
  honest.** The daemon scenarios failed on a dial after the TOML change,
  which is exactly when a regression would look like this. It reproduced
  under twelve busy cores and did not reproduce for the scenario alone,
  which is what said it was the machine: the three run beside each other,
  each starting processes, one compiling a WebAssembly component before it
  serves, and five seconds of post-quantum handshake is not enough on a
  loaded box. Thirty seconds now, re-confirmed under twenty-four busy
  cores. Nothing in those scenarios is a claim about handshake latency.

**The instrument, `rhtnp` (2026-09-14).** Two gate-green commits, 5fa788a
and 78e62ae, on the author's word. The client had been finished and tested
for weeks in a process nothing outside a test ever started; this is the
process. Catalogue 353 of 363, 0 flags.

- **What it claims and what it refuses to claim.** PRT-01 runs a payload
  between two `rhtnp` processes through an `rhtnd` process. PRT-04 runs a
  ceremony between four, to a record every signer names the same. It marks
  **none** of PRD-01 to PRD-09: those are obligations about what a user is
  shown and when they are asked, and a command read from standard input is
  not a person.
- **Every proximity channel is unavailable until it is told otherwise**,
  and that was the design decision worth making carefully.
  `light-client-requirements.md` §1.3 forbids presenting a weaker channel
  as a stronger one, so a machine with no radio and no camera pointed at
  anybody supports none, and the instrument reports what its operator
  declares. A declaration is evidence about a scenario rather than about
  hardware, which is the whole difference between an instrument and a
  client. The alternative — quietly passing a latency channel — would have
  been the exact failure §1.3 names.
- **Two more things nothing outside a test had ever needed.** A
  participant's own key was not in its own lookup, so it could not verify
  a record it had just signed: `wire-format.md` §3.4's rule, which the
  daemon holds its own identity to and which nothing had asked of a client.
  And `rhtn-ffi` carried six of the ceremony's operations and not the rest,
  so no shell could have run one; the facade now carries them as values, in
  the shape `Intent` already had.
- **The ceremony's encoding is the instrument's own and says so.** design
  §7 has the conversation cross whatever channel the two devices have and
  fixes none, which is why the boundary carries fields. A harness copying a
  token between two processes is the analogue of a screen and a camera.
- **The adoption leg is open and is a decision, not code.** A patron
  proposes under its own position and a client that has never been adopted
  holds none. Either `rhtnd` gains an operator action to adopt, which waits
  on section 7's terminal-or-page question, or a participant stands as a
  root, which is a genesis fact an instrument should not mint for itself.

**The position question, and what it dissolved (2026-09-14).** One
gate-green commit, ffb9cb0. Asked why a position was an input to adoption
at all, given that adopting as a newly minted root is ordinary and adoption
into several trees concurrently is expected. Catalogue 356 of 366, 0 flags.

- **The answer was that it should not have been an input.** A patron does
  need its own path, because a subordinate's locator is that path with a
  nibble added. What was wrong was requiring it to be supplied from
  outside and defaulting to none.
- **`Client.position` was written by nothing but a test**: one reader,
  `propose_adoption`, and one writer, a line in `client/tests/recovery.rs`.
  No client could ever adopt anybody. **Fourth of these in two days** —
  `Gateway::bind`, `NodeView.attached`, `replay_to`, and now this — which
  makes the sweep already noted overdue rather than merely worth doing.
- **The framing that had to be withdrawn was this assistant's.** Standing
  as a root was called a genesis fact an instrument should not mint. It is
  not one: `wire-format.md` §2.1 names the empty path the self-anchor case,
  design §2's vocabulary says a root self-anchors, `Locator::root` is a
  constructor, and `rhtnd` mints exactly this for itself at every start.
  The mistake was reading *genesis formation of a transaction* — which the
  conformance harness does test — as covering a party's own anchor.
- **The second half was invisible until it was asked for.** `wire-format.md`
  §2.3 keeps one series per patron relationship and §7.6 one endpoint
  record per line, and `NodeView` has held positions per subnet all along;
  the client had one, so a second adoption would have overwritten the
  first. Nothing tested it because nothing could adopt at all.
- **Closed the same day**, bd42d45. `Horizon` keyed the places and
  locators it holds *about other parties* by party alone; they are keyed
  by party and subnet now, and `place`/`locator` gave way to `places_of`
  and `place_in`, `locators_of` and `locator_in` — a caller taking one of
  several without saying which asserts something the records do not.
  **The stored shape did not change, and the prediction that it would was
  wrong**: the row already carried the party and the anchor as separate
  fields, because a place is a path relative to one, so keying by both
  puts two rows where there was one. A snapshot written before it reads
  back the same. TOP-27.

**What the horizon fix also showed (2026-09-14).** Nothing in production
reads `Horizon` beyond `ingest`: `place`, `locator`, `resolvable`,
`distance`, `prune`, `materialise` and `wake` are called by tests alone.
That is not the same defect as the four — the surface is correct and the
tests exercise it — but it is the same shape, and it means
`light-client-requirements.md` §4.2's whole point, that a client routes
around an unanswering patron without asking anyone, has no caller. **What
would close it is a reader**: the client's own resolution path, and the
direct-versus-relayed decision design §12.6.3 makes from whether a peer is
inside the horizon. Worth scoping as work rather than folding into a fix.

**The four applications of the horizon, built (2026-09-14).** The author
named them: propagating new adoption, recovery and PoP transactions; an
alternate route around an unavailable infra node; populating resource
permissions tables; and identifying distance-1 nodes for flow. Three
gate-green commits. Catalogue 362 of 372, 0 flags. **None of the four was
working, and each was broken in the same shape**: something correct with
nothing computing its input or reading its output.

- **Propagation.** `Msg::Record` fell through the courier into `left` — what
  the adaptors do not carry at all — so a ceremony ended in a record only
  its signers held and an adoption on it went the same way. `Serving` gains
  `propagate`; `Client` keeps an outbox; the boundary carries it as soon as
  the record exists. PRT-06.
- **What actually travels, and the test that got it wrong first.** The
  first PRT-06 asserted the *presence record* reaching the node. design §15
  puts presence in the attestation class — pull, not push — and
  `wire-format.md` §10.1's topology class is adoptions, departures,
  disavowals, peerings and reissues. The adoption is what travels.
- **Failover needed three fixes, any one of which alone left it dead.**
  `NodeConfig.siblings` was set by nothing, so every ack carried an empty
  list into a cache the transport does read. The ack's endpoints were
  ignored in favour of an address book §4 says a client would not have. And
  a client dropped endpoint records off the flood, holding a shape with no
  addresses in it. `EndpointRecord` moved to `rhtn-archive`, both sides
  reading one. SES-25, TOP-28.
- **A sibling with no address is not named**, because §8.2 gives a
  `SiblingRef` one to eight points and no way to say none. The first draft
  named them and the ack would not decode — the wire agreeing that a
  failover target nobody can reach is not one.
- **And it surfaced a bug of this assistant's from the morning**: the
  reconciliation replay ran inside the attach hook, on the accept path, so
  a node whose store was non-empty raced the ack and the client refused the
  attach. Spawned now.
- **Role tables** follow the owner's horizon through a standing grant,
  re-expanded when a stored transaction moves the table — §10.2's moments
  are none of them a request. RSC-36.
- **Trust distance** is the client's own fold now, two sources where a node
  has three. MET-09.
- **Still open, and named rather than papered over.** Nothing pulls a
  presence record to a node, so `keep_presence` has no production caller
  and `evidence()`'s acquaintance edges are empty at a node — the metric
  there runs on adoptions alone. The pull path design §15 describes is the
  work that closes it.

## Author rulings on the reviewer's functional test document (2026-09-14)

The document is `functional_tests.md` in the root, untracked, reviewer-authored
from the five design documents. Review found it mechanically sound: five
fingerprints matching, 441 rows across 26 prefixes matching its own totals table,
297 distinct section citations all resolving, 75 source paths all present, and
every boundary value checked against its source correct. Six items went to the
author; his rulings, and what each cost:

1. **Slot uniqueness.** Ruled: a conforming patron performs the check, and other
   nodes enforce it passively through a strict limitation of their own storage —
   spell it out only where a section is ambiguous. It was: §3.1 gave f=10,
   `wire-format.md` §2.1 gave the nibble range, and nothing joined them. Both
   now do, and `Binding` carries its slot so the fold refuses a second occupant.
   TOP-34, TOP-35.
2. **Endpoint records.** Ruled: no rejection. A receiver cannot tell an infra
   node from a light client that signed one anyway, and the IP-gossip machinery
   would not normally exist on a light client. **This confirms current
   behaviour** — `ingest_endpoint` checks the signature and the horizon and
   nothing else, and TOP-28 already asserts that holding the record is what
   marks the publisher. A draft test for it was written and deleted as a
   duplicate rather than kept.
3. **Contested exit.** Ruled: in-horizon nodes default to the remaining patron's
   determination; out-of-horizon nodes do not see the disavowal. **Open**: which
   patron "remaining" names is ambiguous where the departing node has been
   adopted elsewhere, and it decides whose judgement an observer inherits. Asked
   rather than guessed.
4. **Departure.** The author's question — who retains, and what kind of
   participant — is answered in the report: three kinds hold a `Table`, and
   "left as a root in the table" was true of the fold and false of the horizon
   and of the address book. His proposal implemented: an ending re-anchors the
   departed party on itself and shortens every path beneath it by the prefix
   that reached it. TOP-36, TOP-37.
5. **Key sizes.** Ruled: state them in the wire format, keeping the standard
   each is derived from. Done in §2.2, with §1.3's signature figures given
   theirs.
6. **Nonce echo.** Ruled: add tests wherever relevant. The registration reply's
   echo was already asserted on every registration the tests make. The survey
   found the real gap elsewhere: `Sweep::take` read entries out of any
   `CatalogReply` handed to it without checking the nonce, where resolution,
   currency and the archive walk all check theirs. `Step::WrongNonce` added,
   following those three. RSC-37.

**The defect class recurred twice more.** `departure_body` has no production
caller — only tests — so no client can mint a departure, and `Sweep` itself is
reached from tests alone. Both recorded, neither in scope here.

**Three test harnesses were placing every subordinate at slot 0** and one client
fixture had two different parties at one index, so the rule bit the moment it
existed. That is the check working, not the check being wrong.

## Conformance review of 2026-09-15 at `4edfe52` — all eight closed

Eight findings, all verified against the code and the cited text before any
change, all reproduced by the reviewer's own `tests/september15.rs`, all now
passing. The review suite finishes 67 passed, 0 failed, where it finished 59/8.
**No finding was wrong and none needed an author decision**: each cites a
passage that says plainly what should happen.

| ID | What it was | Disposition |
|---|---|---|
| P01 | The reference metric filtered denied candidates out of the scores and the admitted set but returned the allocation's unfiltered total as `joint`; the allocation had already given the denied party capacity | Denial moved ahead of §16.4's three passes. MET-10 extended |
| T01 | The slot bound lived in the fold, which runs after the store accepts and the adjacencies are written; the node flooded what it then refused | `Table::admits_slot`, asked by the storage decision and the fold. PRP-25 |
| E01 | The client kept the first arrival of an equivocating pair | §10.1.2's decision shared. TOP-39 |
| E02 | The client admitted an unproved second series | The same, with the client's table as its series evidence. TOP-39 |
| E03 | A restored horizon held positions and no addresses | The snapshot carries the lines and the retirements. TOP-40 |
| G01 | A gateway could not tell a derived row from an operator's, so narrowing a grant preserved its permissions | Rows carry provenance. RSC-40 |
| S02 | A recognised recovery left the superseded key's wake endpoint | The cleanup reads the fold's outcome. SUB-10 |
| S04 | The legacy requester/subject log survived an upgrade | Removed by name on load. SUB-11 |

**Two were the previous day's work**, and both landed in the wrong layer: P01
filtered after the allocation instead of before it, and T01 put a storage rule
in the fold. Neither is a rule that was wrong; both are a rule applied where it
could not bind.

**E01, E02 and E03 were one defect.** §10.1.2 was implemented in full at the
node and in one line at the participant — a rule written twice, which is the
shape everything else this week has had. The remediation shares the decision
rather than fixing three symptoms, which is the only version of this fix that
stops the fourth divergence.

**Every finding was already required by `functional_tests.md`.** TOP-016 had the
equivocation rule, TOP-013 the forwarding rule, MAIL-019 the wake cleanup,
MAIL-006 the prohibition on a request history, GAT-006/009 the recomputation.
The document was right and the implementation was not, which is what a test
specification is for. Four rows were added for what the *remediation* exposed
and nothing required: that a storage decision admit only what the holder's own
state can hold (TOP-033), that a rule kept by two holders be exercised at both
(TOP-034), that grant provenance be distinguishable (GAT-037), and that an
upgrade remove durable state the current rules forbid rather than merely
stopping the write (OPS-016).

**Not fixed, and not in scope here**: `counter-advances-strictly` on type 2
still has no negative — nothing refuses a departure whose counter does not
advance. Recorded in the matrix.

## The non-blocked backlog (2026-09-15)

Closed today, in the order they were listed:

**The presence pull path** turned out to be misdiagnosed on my part. I had it as
"nothing pulls"; it is "nobody answered". design §15 makes attestation pull and
names the evaluator as the party that fetches; `light-client-requirements.md` §2
names the subject as the party that serves — *the subject holds their archive,
so a patron evaluating you fetches from you* — and no client served. The node's
handler answered for the node's own archive, which is right and beside the
point, since every participant's archive lives in a client. Built as two payload
kinds, with the walk verified at the fetcher and the nonce tying a reply to its
request. ARC-20, ARC-21. **`keep_presence` still has no production caller**: a
node comes to hold a presence record by fetching one as an evaluator, and
nothing yet drives a node to evaluate.

**The five conditions held on neither side** are closed, and two of them were
not test gaps. The decoder did not enforce §4.5's classical/hybrid split, so a
recovery's hybrid verifier signature passed inside a presence record and the
presence record's classical one passed inside a recovery — with every signature
verifying either way, which is why nothing else could have caught it. DEC-33,
DEC-34, MET-11. **`functional_tests.md` had required this** at SIG-009 and
SIG-010: the specification was right and the implementation was not, for the
third time this week.

**§10.1.3's periodic replay** is running, with the interval read from the
daemon's configuration — it is an operator's number and no document states one,
so the default is a default. DMN-22.

**`Table::is_root`, `Table::is_node` and `NodeView::is_root` are not the defect
class I filed them under.** They are queries tests use to interrogate state, and
the rule one of their comments named — rootward forwarding stopping at a root —
is applied by `send_memo` finding no patron. The comment implied otherwise and
now does not. Nothing deleted, and the earlier note overstated it.

**Still open, and none of it blocked:**
- Resource predicates stop at named rows and standing horizon membership. The
  structural/rank/quantile/tenure/date evaluator and scheduled recomputation are
  unbuilt, and O-009's absolute-rank displacement is an open decision inside that
  work.
- Participant durable storage: in-memory archive and capture sealing only, no
  export/import, no Argon2id-wrapped backup. The daemon persists; the client does
  not.
- 33 conditions held on one side. The eighteen with no positive are the
  dangerous direction: nothing asserts that a well-formed object passes the
  check rather than passing for some other reason.

---

## Cycle 3, pass 0.1 — factual verification of external claims (2026-09-16)

**Eight contradicted, seven applied.** Corrections are in `ea0533c`. The wire
format's note on CDDL `.size` was wrong about the language: `.size` bounds a
uint's value, not its encoded length (RFC 8610 §3.8.1). Three biometric claims
were categorical where the literature is not — templates across versions, depth
capture against a still, and Ghost Peak, which attacks the receiver's handling of
the scrambled timestamp sequence rather than the preamble. §12.4 equated a global
key→locator index with a DHT when it has two shapes and the design refuses each
for a different reason. V5's chronology ran backwards. And §7.5.2.2's flat "data
in memory is not storage" had a second problem the reviewer did not raise: it
contradicted §7.2.1 two subsections away, which declines to judge any
jurisdiction.

**§1's privacy benchmark: declined** [author, 2026-09-16]. The reviewer offered
FTC enforcement against location-data brokers as contradicting *"expensive,
manual, per-target work"*. The ruling: the benchmark names **the commonly
understood and accepted level of exposure**, not a claim about what surveillance
can currently do. Speculative or newly emerging programmes that make the world
outside the network less private do not change the privacy the network affords.
§20.1's row stands as filed. **Expect this finding again** — the text does not say
what the benchmark is for, so each cycle's 0.1 can be expected to re-derive it;
closing that would mean a clause in §1, which has not been written.

**Three findings were already registered** at §20.1 before the pass ran: §7.4.1's
hill-climbing query counts, §7.4.4's cross-device false-reject rate, and §1's
benchmark. The register is doing its job, and a 0.1 reviewer re-deriving what the
document already admits is a pass working correctly rather than a defect.

**The unverifiable set was mostly already registered.** Of the eighteen claims
pass 0.1 could not verify, **thirteen already had a §20.1 row**: the 20–80 ms
radio latency, the 30–80 KB face crop, regional-gateway aggregation, the UWB
channel ranking, the $20/month infra figure, "capture margin in most cases",
0-RTT's battery saving, the substantial-minority relay rate, face entropy against
fuzzy commitments, ageing degradation, randomised motion prompts as the liveness
check, hill-climbing query counts, and account age being cheap to manufacture. A
0.1 reviewer re-deriving those is the register working, not a defect. **Three
were genuinely unregistered** and now have rows: §3.2's 7±2 span of control,
which is an input to f = 10; §7.5's claim that capture failures within one session
are strongly correlated, which feeds the multi-frame argument; and §13.7.1's
"poor practical record" for social recovery, which argues against adopting it.
§20.1 stands at 31 rows.

**Registered rather than removed** [author, 2026-09-17]. The figures are
load-bearing for sizing and security arguments, so striking them would cost the
arguments; the register exists to hold a number that is relied on and not
established.

**Sixteen hedges applied**, each a universal the source did not carry: UDP
sockets outside Chrome's Isolated Web Apps, `getrandom`'s documented route rather
than its only one, templates rooted in something unreissuable rather than
themselves irrevocable, OCSP soft-fail as client convention rather than protocol
requirement, four password products that do not share one key derivation, native
storage excepting reclaimable caches, ICE checking candidate pairs in priority
order rather than strictly direct-then-relay, NAT mapping timers as minimums,
push vendors as best-supported rather than most dependable, jurisdictions
surveyed rather than anywhere, a Wi-Fi scan as not instantaneous, HTTP/3's ALPN
selection absent another mechanism, one characteristic failure rather than the
most common one, federation attributes with no shared vocabulary, hole punching
that can be defeated rather than is, and V7's two legal absolutes.

**Nothing in the confirmed set needed action**: eighty-odd claims verified as
written, including every figure in the post-quantum size tables, the COSE and
CDDL encoding facts, the UWB attack results, and the platform background-execution
constraints.

## Cycle 3, pass 0.2 — internal coherence (2026-09-16 to 09-17)

**Twelve contradictions, all real, both sides verified before any change.** Nine
closed in `e5c966e`, the archive pair in `7e625d2`, and the twelfth by deletion.
Two were sharper than filed: §10.1 does not merely disagree with §10.5, it *cites*
§10.5 as what stops the next request while §10.5 reset the current one; and the
rotation pair is settled by the schema rather than the prose, `Recovery` being
field 6 and present iff this is a recovery adoption. One was narrower: §12.2 and
wire §7.2 agree on substance and collided on the verb "fetched".

**The archive pair carried a third defect the reviewer did not raise.** §7.9's
verification rule required each record's back-pointers to match *"the record that
follows it in the batch"* — a sequence check over a structure §3.1 says has no
sequence. Repaired with the frontier: reachability, not order.

**Finding 11 was withdrawn entirely, not relocated** [author, 2026-09-17]. The
reviewer read §7.3 against §19.6/A.3 and proposed §19.6 was intended. Two author
corrections changed the answer. First: **a witness is not present and is not asked
anything** — witnessing and verification are both client background tasks, the
witness watching message flow for conformance and timing the ceremony against its
own clock, none of it a user action. That made §19.6's *"at the moment they are
asked"* wrong for both roles rather than one, and made §19.6 self-contradictory,
since it says the verifier "is asked nothing" two lines above. Second, and
decisive: **the paragraph is a drafter gloss, not a requirement.** Consent to
perform protocol actions is given by choosing to use the network; there is no
per-request warning. The obligation is deleted from §19.6, the matching bullet
from `light-client-requirements.md` §1.5, and the clause from Appendix A.3. P23
moves from *corrected* to *accepted*, with the reason. What survives in §19.6 is
the capture-time disclosure to the **participant**, which is the one point in the
mechanism where a person is present and acting.

**The privacy fact stays even though the obligation goes**: a witness and a
verifier do become durable nodes in another person's evidence graph, and §19.6
still says so. Recording the exposure is not the same as owing a warning for it.

## Cycle 3, pass 0.2 second run — internal coherence (2026-09-17)

**Four live contradictions, eight stale cross-references, and the split is the
result.** The design set itself now yields four; the other eight were
`functional_tests.md` §9 describing contradictions the first run closed.

**Four applied.** Wire format said *thirteen* signing roles in §1 and *fourteen*
in §1.1 — the consistency pass corrected the sentence above the table and left the
one above that, which is fixing the instance rather than the claim. §9.0 had the
old key signing a `Recovery` at every rotation while defining a plain rotation as
carrying nothing; scoped to the linked case. §11.4 said a table update never
closes a session and then made dropping one mandatory in the same paragraph; it
now separates the three things the infra document already separated — a running
request completes, the resource-facing session is dropped, the transport is
untouched. And the parameter table gave `h_store` = 2 as ~110 nodes where §15.1
gives that horizon as 221; not a wrong number but an unlabelled measure, since
§15.1 calls `h_process`'s 1,110 *"of downline"* and the table dropped the
qualifier from both cells.

**Ten O-items closed, not the eight flagged.** The reviewer did not count O-003
or O-007, but both describe contradictions the first run closed: the rotation
wording, and §12.2's key acquisition against wire §7.2. O-007 keeps a genuine gap
— the authentication profile for an unpinned intermediate or anchor — which is not
a contradiction and stays open. Six remain open (O-011 to O-016), all gaps rather
than disagreements, as the reviewer said.

**Seven dependent rows moved with them**, which the reviewer did not enumerate and
which would have been the expensive half to miss: UX-002 tested the *withdrawn*
witness-and-verifier disclosure and now tests the capture-time participant one
plus the absence of any per-query notice; GAT-014 was marked open pending O-001
and is now a three-part assertion; ARC-010 and SCH-018 described linear pagination
and a single head; and TOP-026, REC-009 and GAT-009 each carried a trailing
"tracked in O-00N" clause pointing at a closed item.

**This is the second run's real yield.** Eight of its twelve findings exist
because closing the first run's twelve left a test document describing the
documents as they were. A pass that reviews a set including its own test
specification will produce that every cycle unless the specification is updated in
the same commit as the sections it cites.

## Cycle 3, pass 0.2 third run — internal coherence (2026-09-17)

**Five findings, all real, and the echo is receding**: three in the root documents
against two in the test specification, where the second run ran eight to four the
other way.

**Two were mine.** Wire §1.1's domain table pointed the catalog entry and the
abuse report at §4, which is *Transaction types* and which itself says an abuse
report is not one — the schemas are at §6.1 and §6.3. I added the delegation row
to that table and did not audit the rows beside it. `refcheck` passed both,
because **§4 exists**: the checker validates that a reference resolves, never that
it resolves to the right place. That is the second time this cycle a resolving
citation has been wrong — the other was §4.1 cited for a signature size that lives
at §1.3. Audited the whole table this time; the remaining twelve point where the
object is defined.

**The design's status line said "No implementation yet."** Fifteen crates, a green
gate and 403 of 413 acceptance entries say otherwise. Now: *Design, with a
reference implementation under way.* No path is named, the root citing nothing
under `Robot/` or the workspace.

**Multi-device was settled in one document and open in two.** §23.3 answered P33
on 2026-09-16 and the register marked it answered; `light-client-requirements.md`
§10 and O-015 still listed device allocation as open. Both now say which half is
closed — which device holds what — and which is not: restoration, deletion
propagation across devices, and how a person is shown an archive spanning several.
That is the reviewer's own reading and it is right: answering P33 did not answer
restoration.

**The two test rows were ordinary staleness**: SIG-005 enumerated thirteen signing
domains against the wire's fourteen, and INT-019 flagged the in-flight outcome as
disputed after O-001 had been marked resolved in the same document.

**Sweep note.** O-015 survived my first sweep because its wording puts *"do not
settle"* before *"multi-device"* and my pattern expected the reverse. Grepping for
a claim is only as good as the phrasings guessed for it, which is an argument for
grepping the subject rather than the assertion.

## Cycle 3, pass 0.3 — unjustified claims, two audits (2026-09-17)

**Two independent audits, 123 and 138 claims, and both reproduce §20.** The
first decomposes to 79 load-bearing exactly as §20's preamble predicts — §20.2's
32, §21's operating points, wire §1.3's tabled bounds, §20.1's load-bearing rows.
The second reaches 103 by counting every byte-string length, frame size and
threshold the wire format states inline. Neither found a load-bearing claim the
register did not already hold or a section did not already declare chosen. That is
the register being read back accurately, twice.

**The second audit's sharpest point was about the register itself.** §20 stated
*"79 load-bearing and 123 in total"* without showing the enumeration, and the two
reviewers' 79 and 103 show the figure is rubric-dependent. It also drifted the
moment I added three rows to §21. The preamble now states the load-bearing set as
**a sum over four enumerated sets rather than an asserted figure**, says it moves
when a row is added, and says a stricter rubric reaches higher. No fixed number.

**Three operating points were missing from §21's table**, and one was mine:
the delegated credential's run of 45 × 48 h, which I had placed with its
mechanism under §21.1's local-policy rule when it is a protocol constant like the
currency attestation lifetime beside it; anchor hysteresis at S and S/2; and the
25 MB anchor index budget. All three now have rows with the basis stated as chosen.

**Wire §1.3's declaration now covers every bound, not only its tabled arrays.**
The second audit counted twenty-one inline byte-string and frame bounds beyond the
table, and the sentence that declares bounds as chosen ceilings only reached the
table. It now reaches all of them.

**Nine intensifiers taken under the standing rule** across the two audits — six
from the first, three from the second — each given its number where one existed
("catastrophically" had ~19,500:1 in the next paragraph; "negligible at any real
k" is 64/2⁶⁴) or restated as assumed where none does.

**Declined, with reasons.** The second audit's #3 (QUIC migration versus TCP) and
#4 (an always-on socket is unreliable when backgrounded) were both **confirmed by
pass 0.1** against RFC 9000 §9, RFC 9293 and the Apple and Android background
execution documentation; they are sourced, not unsupported. Its S3, "nine of the
eleven exchanges", is derived from wire §4.5.2's table, which enumerates eleven
exchanges of which nine read no disclosable field — checked by counting the rows.
S1, that the workspace has fifteen crates, is a fact about the workspace and not a
claim the design makes.

**Both items carried from the first audit are closed** [author, 2026-09-17].
§20.2 gained its fifth column, *evidence that would settle it*, on all 32 rows,
the reviewer's per-assumption text mapped by A-number; and §20.1's threshold
stands, the register being curated by design and the author reading the current
cut as matching his judgment. Neither is to be reopened by a later 0.3.

## Cycle 3, pass 0.5 — fragile rules (2026-09-17)

15 rules flagged as identifier-bound, the reviewer having already excluded
encodings whose role statement immediately precedes them. Verified each against
the "Where invariants live" test: does a role-form statement exist anywhere in
the set?

**Thirteen needed no change — the invariant is already stated in role terms**,
and the wire format is where identifiers live:
- Four repeat cycle-2 dispositions on the same text: decryption failure (#1, cycle
  2's #6 anchor at design §7.5.2: *a verifier reports an identity judgment only
  where it ran the comparison that supports one*), KeyGrant binding (#2, cycle 2's
  #14: *a query the subject countersigned* is role language, and the bullet names
  what the release must name rather than any field), pruning (#3, cycle 2's #8:
  design §10.1 *what is released is the chain, not the evidence*), late response
  (#13, cycle 2's #15: *consent is the gate*).
- Two quote an encoding whose role statement is the sentence before it, which the
  reviewer's own criterion excludes: failover key material (#4, infra §1 opens
  *a client must be able to authenticate every failover peer before it needs one*,
  cycle 2's #9 anchor) and referral progress (#14, wire §7.7 leads with
  *strictly forward along the path and never past its end*, cycle 2's #16 anchor).
- Three have the design's explicit invariant-then-encoding idiom: the witness
  floor (#6, design §13.2 *the record's subtype field is the present encoding of
  that invariant, not the invariant itself*, and design §7.1 states the floor in
  words), recovery's selection basis (#8, design §9.1 *a recovery verifier is by
  definition a prior counterparty recognising the subject; the encoding fixes
  it*), adoption evidence (#11, design §6.1.1 *exactly one of three forms*, then
  the three fields).
- Four carry the property in the design and in the wire's own rationale: recovery
  binding (#7, design §9.4's italic invariant, cycle 2's #13), the successor
  statement (#9, design §9.4 *without letting a thief perform the same
  connection*; wire §4.1 *one observed proof would authorise an unlimited number
  of competing successors*), the transfer statement (#10, design §6.1.1 *vouches
  again, to a named successor*, §6.2 *to that patron*; wire §4.1 *naming all three
  parties is what makes the vouching specific to this move*), archive closure
  (#15, wire §7.9 *the requester verifies the structure itself... the check is
  reachability... a holder cannot be trusted to have walked correctly*; design
  §10.1 verification walks backward through commitments).

**Two reordered so the role statement leads**, the cycle-2 shape:
- #5: infra §7's delegation bullet opened *send it in every AttachAck*; it now
  opens *send the delegation with every attach you acknowledge, and refuse a
  session whose delegation does not name the key the handshake presented*, with
  `AttachAck` field 6 as the present encoding. This bullet was new this cycle and
  had never been through a 0.5.
- #12: wire §5.6's querier rule opened *field 2 MUST name the authenticated
  requester*; it now opens *the querier a verifier limits and attributes is the
  party the transport authenticated, never one the request names*, then the
  field-2 encoding. "Type-4 stream" became *the stream carrying the query*.

**Verifying #5 found wire §9.1 stale against the delegation surgery.** The
surgery's table (`app-requirements-notes.md` §2.1) listed design §14.1, §12.6.5,
§23.3, infra §7 and lc §4.1, and not the wire's authentication step. §9.1 still
said the dialling party checks the presented key *is the classical member of the
KeyMaterial it has pinned* and that *Attach field 1 must equal the
connection-authenticated identity*, both false for a delegated key, which is the
normal case for every instance. Now: a peer presents one raw key, its own
classical component or a delegated one; either check binds it, the pinned member
or the attach carrying that keyhash's delegation naming it; a session on which
neither holds is refused. The mutual-authentication invariant binds the claimed
identity *to the identity the transport-authenticated key speaks as*, and its
present encoding names both forms of `Attach`.
- Consequence swept: *a wrong address produces a handshake failure* was stated
  four times (design §12.6.1, wire §7.6, §7.7, §9.1) and is now *a refused
  session*, at the handshake or at the attach that cannot bind the key. The
  property the four sites exist for, self-detection at contact, is unchanged.
- Design §12.6.1 cited §14.1.3 (QUIC) for endpoint authentication, a citation that
  resolved to the wrong section since the baseline (§11.1.3 then, also QUIC); now
  §5 and `wire-format.md` §9.1. Delegation field 1 cited §9.2 for the handshake;
  now §9.1.

**One garble repaired.** Design §8.2's immutability bullet read *Present encoding:
`LateResponse`, signed by signed reference to `txid`* since the baseline; now *a
separately signed response naming the record's `txid`*.

**Catalogue.** Five entries requoted (RES-12, TRN-03, TRN-04, TRN-17, CER-31).
Three scenarios adjusted rather than only requoted, because the text they
asserted is now forbidden: RES-12 and TRN-03 refuse the session at the attach
instead of aborting the handshake on a non-pinned key, and TRN-04's Attach carries
no field 4. Their tests in `crates/transport` and `crates/node` still exercise
the pre-delegation model, which is the recorded documents-ahead-of-code gap; the
catalogue holds no delegation entries yet. check.py 0 flags, 403 of 413.

## Cycle 3, pass 0.7 — LINDDUN privacy threat analysis (2026-09-18)

31 data flows and 30 stored artifacts through seven categories, ~370 ledger
rows. **The ledger reproduces §19**: every row's acknowledgement column cites a
P or C entry, an accepted cost, or a section, and the reviewer's table of
thirteen deliberately accepted costs matches §19.7 item for item, including item
7's tombstone. The correlation table restates C1–C22 with severities that agree.
That is the register read back accurately, as both 0.3 audits did for §20.

**Five additional findings, verified:**
- **N1, archive-request interest leakage — does not hold.** The finding assumes
  a third-party holder who learns a requester is investigating a subject. lc §2:
  *the subject holds their archive, so a patron evaluating you fetches from you;
  this is peer-to-peer payload, not something an infra node serves on your
  behalf.* The request reaches the party presenting the history, for an adoption
  they are party to; what a relaying node sees is a session, the chokepoint
  §19.7 item 8 accepts.
- **N2, prekey publication and deposit cadence — folded into N4.** The serving
  node serves the one-time fetches itself (C11), so depletion is something it
  already sees rather than infers; what the finding rightly notices is that the
  no-record obligation named fetches and doorbells and not publications and
  deposits. The cross-class rule below covers them.
- **N3, role-table history — applied.** Infra §10.2 described the table as
  current state (a departing member's rows removed) without saying no history of
  rows is kept, and the house pattern says so everywhere else (memo table, C19;
  registrations, §6.5; caches, P32). One sentence added: a row replaced is gone,
  and what a member's roles used to be is the time axis C22's matrix would
  otherwise gain. **Derived from the pattern; signed off as derived [author,
  2026-09-18].**
- **N4, interest telemetry as one class — applied, as a consolidation.** Infra §1
  stated process-and-discard for liveness; §2, §4.5, §6 and §6.1 each said their
  class "falls under §1's obligation"; catalog queries (P38) and currency
  fallbacks (§19.7 item 12's *nothing is retained*) had no infra statement at
  all. §19.1 says the composition is what matters, and one node answers every
  class. Infra §1 now states the rule once, for every request class including
  one the document does not name, and the per-class statements stand as its
  instances. Design §19.8 gains **C23**, the composition itself: histories of
  resolution, one-time key, catalog, currency and publication requests at one
  node join into an interest graph no single class yields; High for a node
  keeping histories, and the obligation is what keeps it low. **Derived from
  §19.1 and the existing per-class rules; signed off as derived [author,
  2026-09-18].**
- **N5, role names at the resource endpoint — already carried.** rr §7.4: *role
  names are visible strings and they leak... a reason for operators to choose
  names knowing they are public to those who can see the resource.* The resource
  receives names its own manifest declared (rr §7), which is no disclosure to it.

**Rows marked new or implicit elsewhere in the ledger, declined:**
- Backup blob size and update timing at a storage provider (DF27/SA27): §23.3's
  answer to P33 puts the cold store on a personal computer and §13.7.1 rejects
  consumer cloud defaults; a user who chooses a cloud location chooses that
  observer. Below the register's threshold, which the author has said is
  calibrated.
- Group-fanout residual not disclosed to users (DF26): C20 registers it; the
  standing ruling is no per-event warnings, consent being given by using the
  network.
- A person described in an abuse report's `detail` gets no notice (SA26): P27
  bounds and localises the report; the person is the application's data subject,
  outside the protocol's boundary (rr §1).
- Resource-side session retention after a session ends (SA23): P20 and P24 price
  resource logging as outside enforcement, and a per-resource session id retained
  links nothing across resources.
- discover_scope reveals the intended audience to the host (DF20): the host
  filters at answer time (infra §11), so it must know; it is the owner's own
  chosen host.
- Peering visibility broader than the two parties, wanting UI disclosure (DF29):
  §19.7 item 6 accepts visible placement; infra §8's disclosure obligation is
  general.
- Anchor-table history (SA13): the table is an index replaced by counter; infra
  §1's rule now says tables, not histories, for every class.
- Per-introduction and per-resolution consent moments (DF01, DF07): the same
  standing ruling as DF26.
- The 48 h × 45 run "reviewed against the threat model" (SA02): §12.6.5 gives the
  90-day horizon as the chosen consequence [author, 2026-09-16].

**P12 was stale, found while verifying the reviewer's item 3.** The register said
*specified but not yet implemented*, *Critical until built*, and §19.1.2 said *no
implementation yet*; the status line was corrected on 09-17 and these two
instances were not, the same claim in three places. The root documents do not
track implementation state, so both now say what is true regardless of it: an
implementation shipping hop encryption alone leaks payload to both serving
nodes, Critical for any implementation without it, five integration decisions
remaining (§22.1).

**Test spec** (untracked): two retention-checklist rows extended, prekeys to
publication and deposit history, role rows to no row history.

## Cycle 3, pass 0.8.1 — adversarial: the patron (2026-09-18)

Six findings, 14 self-discarded restatements. Verified each against the text and,
for the currency and memo findings, against `crates/`. One finding is blocked on
an author reconciliation (F1); the rest are ruled.

**F1 — currency attestation as a second, patron-only key selector (REASONING vs
§12.1). VERIFIED; BLOCKED pending author input.** The premise holds. §12.1's
participant-authentication invariant governs the locator; `CurrencyAttestation`
field 2 (`current_key`) is a second selector of the addressed key, issuer-signed,
and the beyond-horizon path treats it as authoritative
(`crates/archive/src/currency.rs::assess` → `Attested(a)` → address `a.current`).
When field 2 ≠ field 1 the object asserts a rotation on the patron's signature
alone; the participant authorization a real rotation carries (recovery adoption's
successor statement + verifier responses, wire §4.1) does not travel with it, and
nothing requires field 2 to be backed by one. The fork machinery (§9.0.2, ARC-15)
catches this only when a *competing* attestation exists; a lone compelled-patron
attestation naming `K_attacker` produces no fork. Author ruled **constrain field 2
== field 1** with the caveat "check whether any flow needs field 2 ≠ field 1."
**It does**: the fork/divergence notification (§9.0.2, ARC-15) is built on field 2
carrying a successor that differs from the queried key; a literal field2==field1
removes fork-detection from the currency path. **Resolved [author, 2026-09-18].** The author's ruling: the key binding is the
user's (authored by the recovery adoption's successor statement and the signed
locator); the patron-signed staple only vouches currency and cannot select a key.
Verifying against `crates/` showed the implementation **already conforms**:
`require_currency` addresses the queried subject and uses the staple only to
confirm that key current; `assess` reads field 2 solely to count divergent claims
for fork detection, and no consumer addresses `Attested(a).current`. So the gap was
prose, not mechanism: wire §7.1, the §12.6.5 staple table, and §12.1 could be read
as licensing a redirect to field 2. Fixed by stating the invariant in all three:
field 2 is the issuer's account of what is live, not an authority to select a key;
a relying party addresses a key authored by the participant (SignedLocator §2.3 or
recovery adoption §4.1), and a differing field 2 is a rotation-or-fork signal, never
a redirect. Fork detection (ARC-15) is unaffected: it turns on divergent field 2s
across issuers, which the invariant keeps as a signal. No code or acceptance change
needed; the literal field2==field1 first proposed was set aside because it would
have removed fork detection from the currency path.

**Backed by executable tests [author, 2026-09-18].** The author asked that the
clarification be demanded by a test a redirecting client would fail, not left as
prose. Two catalogue entries, both implemented as live tests in
`crates/node/tests/currency.rs`:
- **CUR-20** (must-accept): a valid attestation whose current key differs from its
  subject is accepted, not rejected -- the honest rotation-report/fork-signal form;
  a client requiring field2==field1 fails it.
- **CUR-19** (negative): a lone current attestation naming a differing successor
  does not move the addressed key; the relying party proceeds on the queried key;
  a redirect-on-field-2 client fails it.
check.py 405/415 implemented, 0 flags; gate green. `functional_tests.md` gains
CUR-011/CUR-012 (untracked). A canonical wire vector (`P-currency-successor`,
field 2 differing from field 1, accept) was prepared in
`test-vectors/tools/generate.py` but **not landed**: the vector corpus is gated
behind `spec-pins.json`, which this cycle's spec edits have left stale, so
regenerating asserts a re-audit of the hand-authored fixtures across the whole
spec delta -- a separate pass, not part of this finding. The executable
acceptance tests are the binding demand.

**F2 — a positional predicate delegates ACL membership to the topology authority
(NOVEL). NOT A FINDING [author, 2026-09-18].** Accurate framing but already the
design: a resource owner marks per role whether it is topology-granted, and may
grant sensitive roles manually to named individuals within the horizon disregarding
topology (rr §7, "assignable to named individual nodes"). The auto-grant ceiling
the reviewer asks for is the owner's existing per-role choice. No change.

**F3 — retaliatory disavowal prejudices the cheap re-homing path (REASONING).
VERIFIED; ruling kept, justification repaired [author, 2026-09-18].** Both premises
hold: §18.5 says the neighbourhood defaults to the patron's determination [author,
2026-09-14], and §6.2.3's cheap re-homing (grandpatron/patron-sibling) sits in
exactly the neighbourhood the disavowal floods. §18.5's own two claims were in
tension — "bounded to spite / any evaluator who matters sees the exculpating half"
did not neutralise the adverse default for that local audience. Author keeps the
adverse default (tree sovereignty; relationships are meant to be weighty and a
little painful to exit, not tyrannical). §18.5 rewritten to remove the tension: the
adverse default is friction *inside the neighbourhood being left* and does not
travel; an evaluator in a different context reads the subject's own archive (the
departure, never the disavowal); the cheap local move carrying the former patron's
account is the shape of a with-prejudice exit, and a clean slate stays available by
presenting into a context holding no history. MET-10's quoted sentence preserved
verbatim.

**F4 — memo ordering's "one patron clock" premise is imprecise for departures
(REASONING). VERIFIED; precision fix applied [author, 2026-09-18].** wire §10.2.2
said competing memos for one slot share one patron's clock, while the same
paragraph sources field 4 from the underlying signed transaction — and a departure
(§4.2) is participant-signed, so its vacancy memo carries the participant's clock.
Impact is nil: the only cross-clock pair pits an emptying against a fill, and a skew
can only leave a slot stale-empty (the emptied-timestamp rule already blocks
resurrection), which is the safe direction. §10.2.2 now says so. The suppression
half (patron omits a vacancy memo) reduces to a stale entry in the ancestor's
private RIB, bounded by "hint, never evidence" and by stale routes failing and
re-resolving — no change.

**F5 — a dishonest "custody accepted" prolongs censorship past exit (EXTENDS).
COVERED, no change [author, 2026-09-18].** Relay "accepted" is explicitly custody,
not delivery (lc §2), so a correspondent never had delivery confirmation; reaching
a lying old node requires it to also answer resolution falsely, which is the §18.4
censorship-by-serving-node primitive already priced, and infra §6.1 makes a node
with no record refuse (a visible failure). The serving-lease mechanism is declined
(a new temporary keyset, unnecessary); §23.3's delegation window already bounds an
instance's serving authority.

**F6 — patron countersignature on series reissue gates the participant's pruning
(EXTENDS). VERIFIED; rejected as intended, registered [author, 2026-09-18].**
Confirmed: pruning-with-continuity needs a reissue checkpoint (§10.1) and a reissue
is patron-countersigned (§6.2.1, wire §4.6). The reviewer's "design says the
coupling is elective" is not in the text; §10.1 frames it as load-bearing. Author
rejects the owner-only checkpoint: a pruned continuous history has dropped the
hashchain that proves it whole, so it must anchor to a social context (the
patron's countersignature, queryable out of band); an owner-only rollup would
allow costless multiple sets of books, which is the friction the archive exists to
impose. Parallel histories are fine but each must be socially anchored; the
clean-slate alternative (present with no history) remains. §10.1 now states this;
the refusing-patron cost is one instance of the §18.5 cost.

**Standing note from the author [2026-09-18].** The drafter and reviewer, trained
on trustless and centrally governed systems, recurrently misperceive the authority
expected of patrons and other user classes. These relationships are meant to be
weighty and a little painful to exit, but not tyrannical: exit and parallel
membership remain possible. A user should care about the patron's opinion and a
patron about the sub's satisfaction. Findings framed as "a patron has power X over
its sub" are to be weighed against this, not automatically against a least-authority
default.

**Confirmed defenses** (reviewer's, agreed): unilateral exit (§6.2.1), locator
authentication in isolation, the memo hint-rule (wire §10.2.3), the horizon outer
gate.

## The signing-authority rulings, applied (2026-09-21)

Out of `Robot/outstanding-work-2026-09-21.md` section 8. The survey found the
delegation surgery unspecified for what an instance signs unattended; the author
ruled it in four exchanges the same day and the whole was applied in one pass.

| Ruling [author, 2026-09-21] | Where it landed |
|---|---|
| Acknowledgement and attestation signed by the instance's delegated key; the memo is the session's; currency needs no change beyond that | wire §7.5, §7.1, §8.2 field 1; design §23.3; infra §7 |
| Topology persists as updated topology, never as history; reason data stays within the horizon; a third party's flood enters no bystander's archive | infra §5's seen-set bullet; the store keeps txids and the fold (code owed) |
| A cycle repair is not a disavowal: a removal, the vacancy memo, no transaction, no reason code; §10.2.4 reworded; *disavow* not used for it | wire §10.2.4, §4.3 code 5 tombstone; design §18's cycle-injection bound |
| The delegation is a topology-class object, pushed as each credential comes into force, held as current state by `not_before` | wire §8.2, §10.1, §10.1.1 |
| The staple carries the delegation | wire §7.1 field 8; lc §4.1; wire §12 size row |

**Verified before applying**: every code site the survey and its review cited
(`currency.rs` 345, `topology.rs` 565, `propagation.rs` 716, `prekey.rs` 38,
`catalog.rs` 183, `resolution.rs` 641–682) signs as described; `TopologyMemo` carries
no signature; the ack's chief verifier is the issuing node's own gateway
(`resources.rs` 269); design §10's list is unchanged once a cycle repair is not a
transaction.

**Catalogue.** PRP-12 requoted and re-derived; ARC-08, ARC-09, ARC-11, DMN-09
re-derived to the frontier (their quotes had moved on 09-17, their expectations had
not); CER-26 and CER-17 re-derived to design §19.6's withdrawal. Six added: TOP-41,
TOP-42, CUR-21, CUR-22, PRP-26, PRP-27. Five live tests unmarked, each holding the
superseded rule (`chain.rs` ARC-08/09/11, `probe.rs` DMN-09, `propagation.rs`
PRP-12); 405 → 400 of 415 → 421.

**Code.** The reference client no longer notifies a witness or verifier
(`ceremony.rs` three sites; `notice::Role` keeps `Participant` alone; the FFI and
the terminal follow); CER-17's and CER-26's tests assert the absence.
client, ffi, participant and acceptance: 104 passed, 0 failed, 24 ignored.

**Models.** `wire-only/currency.spthy` and `models/README.md` restated: a staple
confirms currency and selects no key.

**Robot.** `app-requirements-notes.md` §2.1's *no propagation* decision of 09-16
struck and superseded; its three-context count corrected to six; §2.1's *only other
things the node signs* corrected. `implementation-plan.md` line 588's *one item* and
section 7's PRD-06 bullet aligned with infra §8.2 and §8.3.

**Counts.** refcheck 2,266 / 0; modelrefcheck 1,194 / 0; stalecheck 1 + 10 + 3;
check.py 400 of 421, 0 flags; matrixcheck 87: 51 / 33 / 0 / 3, 0 flags; `crates/check.sh`
CODE GATE PASSES, five fuzz targets with no crash.

**The connection bind, ruled the same evening [author, 2026-09-21].** Of the
two shapes offered for a dialler outside the instance's horizon, the delegation
presented first on every connection (control frame type 7 where no session
opens) over the referral carrying it. Applied: wire §8.0's table gains row 7,
§8.2 the rule and its two encodings, §9.1 the three-way bind; infra §7 and lc
§4.1 follow. RES-12, TRN-03 and TRN-04 requoted with their expectations
unchanged; TRN-18, TRN-19 and TRN-20 added; 400 of 424, 0 flags, stubs in sync,
clippy clean, the stub crate builds. Two quotes hit the 40-word limit on the
first attempt and were trimmed to the binding clause; three new quotes carried
line breaks into the generated stubs and were flattened, `gen_stubs.py` emitting
them verbatim. The wire-only attach model now owes one rule for every
connection mode rather than a per-path exclusion.

**Three more, ruled and applied the same evening [author, 2026-09-21].** The
window: a decoder enforces exactly 48 hours (172,800 seconds, epoch times),
credentials finish-to-start, and the receiver's clock check carries a
configurable leeway defaulting to 10 seconds, with a refused connection retried
(wire §8.2; TRN-21, TRN-22). The keypair: minted on the instance, only the public
half sent to be signed, one key for the run, the OpenSSH shape (wire §8.2, infra
§7; DMN-23). The light client is a holder of delegations as §10.1.1 has a node be
(lc §4.2; TOP-43). Two quotes for TRN-21 fell inside CDDL comment lines and
`check.py` does not strip the comment markers, so the entry quotes the
single-line fragments. 400 of 428, 0 flags, stubs in sync.

**The prekey question, ruled 2026-09-22 [author]: own prekeys, the phone signs
their public halves.** The reason given: even without direct point-to-point
contact, duplex resource interaction is the desktop's main anticipated use, and
a team intranet will carry resource types used chiefly from desktops. The three
options were put as what each loses; the cold store loses messaging and the
verifier role, shared keys part the ratchet at the first message, per-device
keys cost a device dimension on the wire. Landed as principle in design §23.3
and §14.2.4: a session is with a device, an identity has a bundle per device,
an initiator opens one session per device and sends to each. **Not yet landed
on the wire**, since the encoding turns on one choice the author has not made,
the device's identifier: `PrekeyBundle`, `PrekeyReply`, `OneTimeDeposit`,
`RelaySubmission` and `WakeRegistration` gain a device dimension, the queue of
design §14.1.6 becomes per device, and an attach speaks for a device. The
count of a subject's devices becomes visible to anyone who fetches its prekeys,
which is a register item for §19.

**The identifier, ruled and applied [author, 2026-09-22]: the device's transport
key**, *necessary to allow multi-device on root nodes*. `wire-format.md` §7.8
defines `device` as the 32-byte raw key a device presents in a handshake, the
delegated key or the seed-holding device's classical component; `PrekeyBundle`
carries it as field 5 under the signature, whose field is now 6; `PrekeyRequest`
names one in field 4, required when a one-time key is asked for; `PrekeyReply`
field 2 is an array of at most eight bundles. §7.10: `RelaySubmission` field 4
names the recipient device; a publication whose device is not the session's key
is refused; deposit and wake registration belong to the session's device. §8.2:
an attach speaks for a device and a subject's devices attach as several sessions.
Design §14.1.6: the queue is per device and the metadata names it. lc §4.1 and
infra §6 carry the obligations. Six entries re-derived (PAY-01, PAY-04, PAY-06,
PAY-12, SUB-02, QUE-05), five of their live tests unmarked (`payload.rs`,
`prekeys.rs`, `submissions.rs`, `queue.rs`), four added (SUB-12, PAY-20, QUE-21,
SES-26); 395 of 432, 0 flags. `gen_stubs.py` wrote a multi-line quote into a
doc comment verbatim, which a demoted entry's old quote exposed; it now collapses
whitespace. **Two numbers are the assistant's, pending the author**: eight as the
reply's ceiling, chosen because design §23.3 holds the count near three, and the
severity of the new register row, the device count a prekey fetch reveals, which
§7.8 states and §19 does not yet price. The test-vector pins are staler by this
pass and advance once, for the whole delta, per the note's section 3.

**The five items off the wire, ruled 2026-09-22 [author].** Candidate order:
*defer to the RFC*; design §14.1.1 says RFC 8445's order binds the
implementation, TRV-11 holds it, `connect_direct` does not yet. The register's
seven open rows: the reading offered stands, O-007 and O-013 before the API,
O-015 during the first shell, the rest before release with O-012's optional
paths and O-014's peer backup optional. `runner-rs`: retired and purged, its
mentions in wire §13, the vectors' README, the corpus test and the plan
rewritten; the workspace and the crypto tests build without it, so nothing
depended on it. `functional_tests.md`: a root design document, the seventh,
named in `CLAUDE.md` with its provenance and staged; `Robot/` is scratch and
context and no part of the deliverable; `conformance-review/`'s placement was
not reached and is asked. The first shell: payload, ceremony, provisioning,
resource, in that order because each needs the one before, any part taken up as
it comes up, the shared primitives written whole-system-aware. 395 of 433, 0
flags.

**`conformance-review/`, ruled the same day [author, 2026-09-22]: tooling, in
`crates/`.** The harness moved to `crates/conformance/` as a workspace member,
path dependencies one level up, its standalone lockfile dropped; the reviewer's
reports and inventories at `4edfe52` stay out of the tree, being a report at a
commit and not a tool. The one fixture that no longer compiled, `tests/daemon.rs`
line 22's `Config`, gained `reconcile_secs: 900`, the daemon's own default, and
nothing it asserts changed; **the rule, recorded in the crate's manifest and
`crates/README.md`: we repair what stops a reviewer's test compiling and never
what it asserts, an assertion changing only when the reviewer changes it or the
author withdraws it.** 67 of 67 pass in the workspace; `modelrefcheck.py` sees
1,217 citations across 259 files, 0 flags, the reviewer's `D §` and `W §` forms
being ones it does not read as citations. `crates/README.md`'s duplicated `cli/`
and `daemon/` rows removed. The first gate run over the moved harness failed on
lint alone: the gate denies every clippy warning and the reviewer's code was
written outside it, one site tripping `field_reassign_with_default`. Allowed in
the crate's manifest with the reason, under the same rule, rather than repaired
in the reviewer's test; a new lint is added there, never fixed here.

## The delegated bind, modelled (2026-09-22)

Milestone A step 1 of `Robot/outstanding-work-2026-09-21.md`. The wire-only
attach theory modelled one bind, the pinned classical half (CHECK 2); every
instance's normal case since 09-16 had no rule and no lemma. Added: `Delegate`
(the identity's key over a fresh transport key, public), `Compromise_Instance`
(design §18.1's residual as a carve-out: a seized instance answers as the node
and can never mint a delegation), `Client_Holds_Delegation` (the topology-class
bind, the delegation checked under the pinned material before it is held),
`Server_Respond_Delegated` and `Client_Finish_Delegated` (presented first),
`Client_Finish_Held`, `Server_Bind_Delegated` and `Delegated_Client_Answers`
(the mirror, a delegated desktop or instrument), and
`Instance_Signs_Without_Responding`. `server_authentication` and
`client_authentication` gain the seized-instance disjunct. New all-traces lemma
`a_delegated_bind_names_a_key_the_identity_delegated`: whoever bound a
presented key by a delegation bound a key the identity's own key delegated,
seizure being no carve-out. Three exists-trace guards.

**Verified before wiring the gate**: eleven lemmas verify, wellformedness
clean; each of the three candidate mutations falsifies the lemma when run by
hand (13, 15 and 15 steps). Two of the three patterns matched more than once
because the theory's own commentary quotes them; anchored to surrounding
syntax, and the gate's substitution, which replaces every occurrence, is
unaffected either way. The gate names a mutant's transcript by lemma, so the
three mutations of one lemma share a transcript, each verdict checked at its
own run.

`models/run-all.sh`: **ALL MODELS PASS** — five TLA+ models and one further
instance, five TLA+ mutations violated as expected, eight Tamarin theories
(wire-only 11 + 7 + 14 + 6 = 38 lemmas, compliant 29), two bounded companions,
ten theory mutations falsified. `modelrefcheck.py` 1,233 citations across 259
files, 0 flags. The READMEs carry the new bullet, the exclusions, and the
count. The currency premise had been restated the day before.

**What the theory states it does not reach**, in its header and the folder's
README: the window and the receiver's leeway; the distinction between an
attach and a request-only contact; the flood as such; one key per run and the
run.
