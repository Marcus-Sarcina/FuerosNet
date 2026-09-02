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
