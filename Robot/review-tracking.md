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
