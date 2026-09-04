# Change log — Reconfigurable-Hierarchic Trust Network

**History for the document set.** The current design is in `network-design.md`;
alternatives considered and declined are in its Appendix A. **Nothing here is
needed to understand the design as it stands** — this records how it got there.

**Reverse chronological within each day; oldest day first.** The project began
**2026-08-12T03:36Z**.

**Dates reconstructed from per-turn transcript timestamps**, twice — the first
attempt used the wrong "today" and stamped 91 entries on a date nothing happened.

**What the evidence supports, and what it does not.** Two transcripts carry
timestamps: the first spans 2026-08-12 03:36 to 08-14 05:59, the second 08-14 05:57
to **08-17 00:30**. Entries for 12–13 August are dated directly from the first.
**Entries between 14 and 17 August are placed by matching each entry's distinctive
terms against the second transcript and smoothing the result** — so the *span* is
evidenced while an individual entry's day is approximate, and the boundaries are
soft. A cross-check by turn volume per day disagrees with the match-based placement
by enough to be worth stating: it would put more entries on 14 August and fewer on
the 16th.

**Between 08-17 00:30 and 08-22 16:15 no transcript exists.** The session appears to
have paused; nothing is dated in that window and nothing should be read into it.

Entries from **2026-08-22** are the current session and are dated directly.

An earlier version stamped every entry with the date of the first one, which put
three days of work on 12 August and a further day on 15 August, a date on which
nothing happened. Ordering was always correct; the dates were not.

| Entries | Date | Established by |
|---|---|---|
| Topology through addressing | **12 Aug** | Transcript, 03:36–18:35 |
| Currency through wire format | **13 Aug** | Transcript, 05:00–20:42 |
| Review passes 0.1–0.5 | **14 Aug** | Transcript, ends 05:59 |
| Everything since | **17 Aug** | After the 16 Aug compaction; **this session spans 16–17 August and its entries are not separately dated** |



**Structure.** Each day is a `##` heading and each entry a `###` beneath it, so a Markdown outline gives days that collapse to entries. Entry headings keep the full `date (topic)` label they have always carried, so citations of the form *"the 2026-08-24 entry"* still resolve.

## 2026-08-12

### 2026-08-12 (morning)

Network layer: topology, cryptography tiering,
addressing and resolution, control/data plane split, trust model, security
analysis. Frozen separately as
`network-design-checkpoint-2026-08-12.md`.

### 2026-08-12 (afternoon)

Proof of presence specified end to end (§7–6.7)
and key compromise/recovery added (§9). Corrections made in this pass:
latency bounds distance from *above*, not below (§7.6); radio-environment
co-presence cut (§7.6.1); a second veto keypair reversed to a single keypair
(§9.2); decay reframed as a deliberate mechanism (§16.5); retention's
recovery role narrowed to weak ties (§9.1).

### 2026-08-12 (consistency pass)

Scope, vocabulary, cryptography table,
§14 message classes, §16.3, rejected alternatives, parameters, and open
questions reconciled against the presence work.

### 2026-08-12 (later)

Oracle hardening strengthened (§7.4): dual rate
limits, subject-side enforcement of per-subject limits, and queries bound to a
witnessed ceremony. Rotation reconceived as adoption-shaped (§9.0), split
into attested and recovery variants, and its propagation resolved as a pattern
rather than a new message class (§14).

### 2026-08-12 (rotation collapse)

Rotation folded into adoption entirely
(§9.0). The floor requirement and explicit haircut proposed earlier were
removed as redundant with the flow metric. Verification reclassified as
evidence rather than authority. Identity forking documented as an accepted
property.

### 2026-08-12 (framing)

Teams paradigm added to §1; membership plurality
and key forking separated as distinct axes. Fork visibility (current-keys
assertion) and misconduct attribution norm added to §9.0.2.

### 2026-08-12 (pull correction)

Current-keys assertions corrected from push
to pull (§9.0.2); they are the pull endpoint of the existing "push near,
redirect far" pattern, not a standing broadcast. Divergence notification,
sibling fallback, and answer TTL added.

### 2026-08-12 (routing)

§11.6 added: self-routing addresses, repair in
transit, infra nodes carry no payload, and the security/performance TTL split.
Closes the routing open question.

## 2026-08-13

### 2026-08-13 (currency)

§12.6.5 added: key currency treated as the PKI
revocation problem, not a session-key one. Stapled short-lived currency
attestations, ~10 h lifetime, and failure mode graded by stakes rather than
uniformly soft-fail.

### 2026-08-13 (outage)

§12.6.5.1 added: patron outage cascades downward
through countersignatures. Handled by sibling then grandpatron issuance, with
the rule that attestations are issued fresh rather than extended stale.
Light-client patrons pre-delegate issuance.

### 2026-08-13 (disavowal)

§6.2.2 adds disavowal as the patron-side
counterpart to transfer. §11.7 covers rootless operation: roots derive currency
from below, anchor status requires size as well as rootlessness (correcting
§11.2), and peering gains a third justification as reachability insurance.

### 2026-08-13 (corrections)

§12.7.3 reverses an incorrect claim: the anchor
budget is a per-node cache limit, not a global cap, so the caching threshold is
per-node policy and must NOT be a protocol constant — a fixed threshold would
make roots invisible in the early network. §11.2's 60B figure reframed as a
stress test rather than a target. §12.7.1 added: Genesis identities need no
currency attestation because the question it answers does not arise.

### 2026-08-13 (peering optionality)

§12.7.5 records why peering stays
optional (infra-tier operation vs light-tier bootstrap; no ordering conflict)
and that its three dependencies degrade gracefully. Peerless infra nodes
flagged as a visible, policy-discountable deficiency. §6.2.5 notes cycle
prevention as unspecified.

### 2026-08-13 (subnet formation)

§13 added, closing the bootstrap open question. Bootstrap
reframed as recurring subnet formation rather than a one-time genesis: meeting
before adoption, formation records permanently typed as witnessless, one infra
instance required and two recommended, and ceremony evidence documented as
degrading by availability across three stages.

### 2026-08-13 (subnet plurality)

Major simplification. §3.1.1 reframes
multi-subnet membership as a power the protocol does not model rather than a
feature it represents; the DAG claim removed from the specification. Soft-fork
deleted as a transaction type. §21.8 dissolved rather than resolved. §13.6
notes rotation is per-subnet; §13.7 records that deferring multiple identities
forces correlation, and that the device is the single point of failure for a
cross-subnet identity.

### 2026-08-13 (graph precision)

§3.1 distinguishes the acyclic *authority*
relation from the deliberately cyclic *connectivity* graph. Peering creates
cycles by design (that is how it raises the cut); sibling replication and
forwarding records also add non-parent-child edges. "The structure is a tree"
refers to authority only.

### 2026-08-13 (client vs protocol)

§13.7 records that multiple keys are a
client concern with no protocol consequence beyond the rotation transaction,
and that exhaustive-disclosure investigation degrades as multi-key matures.
§13.7.1 added: local store is append-only and backs up with ordinary tooling,
but the blob aggregates every key and photo, and backups must be encrypted
separately and retention-aware or §7.5.1's commitment is fiction.

### 2026-08-13 (sessions & backup)

§12.1 added, closing session establishment:
no NAT traversal problem, QUIC, dial-out attach with sibling failover as a
declared degraded state, store-and-forward default with content-free push
opt-in, and the patron-as-mailbox queue policy left open. §13.7.1 gains the
envelope-encryption construction, an honest costing of Shamir/SLIP-39 KEK
splitting, and scan-on-import retention enforcement. §13.7 records that
rotation chains are walkable, so recovery and unlinkability are the same
choice seen from two sides.

### 2026-08-13 (wire format)

Companion file `wire-format.md` drafted: CBOR
deterministic encoding plus COSE signatures, the five transaction types then
defined (seven as of 2026-08-17), the
supporting attestations, session frames and QUIC binding. Fields marked [D]
where derived from a decision here and [P] where proposed. Design doc remains
authoritative on any disagreement.

### 2026-08-13 (transfer collapse)

Transfer removed as a transaction type;
it was adoption plus a drop, and dropping was never a network operation once
§3.1.1 landed. §6.2 restructured into departure (node-initiated) and disavowal
(patron-initiated): formed bilaterally, ended unilaterally by either party.
Lateral/vertical shifts documented as ordinary adoption with a derivable
trust-preserving property. Veto exemption restated as a role condition rather
than a type exclusion, since the type it named no longer exists.

## 2026-08-14

### 2026-08-14 (factual verification)

Review pass 0.1 run against an external
model with search. Corrections applied: **NFC is not anti-relay** (relay
attacks are a documented class; relay resistance now rests solely on UWB, and
the channel ranking says so); serving cell ID is Android-only; geohash cell
sizes were wrong (~156 km at precision 3, not ~78 km); APNs tokens are stable
but not permanent; OCSP soft-fail scoped to Firefox rather than "browsers";
"repeated sampling does not help" corrected (min-of-many-samples does suppress
queueing jitter — the systematic radio floor is the real obstacle); BIPA/CUBI
differences and controller-status caveats added; Kerberos working-day rationale
dropped as unsourced; ML-DSA ratios and presence record size given exact
figures and a parameter set. New §17.1 lists eight unsourced assumptions.
**One reviewer false positive:** the §7.6 latency bound was reported as
backwards; it is correct as written, and the reviewer's own explanation agrees
with it.

### 2026-08-14 (parameter inventory)

Review pass 0.4. Five conflicts found
and resolved; see new Appendix A.2. Presence record size aligned at ~35 KB, stale
geohash comment fixed, infra threshold corrected to 110 in §16.6, anchor
threshold S reframed as a guideline rather than a status boundary, and the
`n` notation collision resolved by renaming the query count to `q`. Heartbeat
interval and currency attestation lifetime given explicit status. One open
parameter choice surfaced: L=2 vs L=3 for the infra threshold.

### 2026-08-14 (infra yield)

The L=2 vs L=3 "open question" raised in the
parameter pass was a misreading and is withdrawn. 110 (non-infra ceiling),
1,110 (single infra node's span across three tiers) and 1,000 (asymptotic yield
once overhead infra nodes are counted) are three distinct correct quantities;
Appendix A.2 now derives the convergence, which lands on f^(L+1) exactly.

### 2026-08-14 (coherence)

Review pass 0.2 returned ten contradictions, all
genuine, no false positives. Eight were the same failure: a claim corrected in
one place and left stale elsewhere. Fixed: patron invariant scoped to non-root
nodes; **eleven residual references to the abolished transfer transaction**
swept; NFC removed as an "anti-relay" fallback in the ceremony step; formation
subtype added to the §8.1 schema to match the wire format; anchor redefined in
§2 and the "every node replicates" claim corrected to local caching policy;
the "any patron with subordinates is infra" claim scoped, with the consequence
that light-client patrons give no censorship signal; §7.4's infra-trust claim
rescoped to evidentiary reliability per §16.6; peering minimum marked
recommended rather than required; agent deferral split so the schema
requirement is in scope; §22 build step 1 no longer says "hostile networks".

### 2026-08-14 (unjustified claims)

Review pass 0.3 returned 280 items.
Triage: ~10 load-bearing (new Appendix A.2 register), ~15 already in §17.1, ~5 genuine
errors, ~30 parameter choices misread as derivations, ~220 rhetorical
intensifiers (new Appendix A's intensifier rule editorial rule). Errors fixed: roots do **not** have
large down-lines by definition — Genesis and small roots have neither a veto
nor an issuance path, now flagged open; sibling "independence" scoped to the
honesty axis; the pull-scaling claim reframed as an implementation constraint
rather than a prediction; the ~1/1000 infra figure identified as a packing
ratio rather than an adoption forecast; "hardest part of P2P networking"
softened. §20 now distinguishes chosen from derived parameters.

### 2026-08-14 (vignettes)

Convention established in a new Appendix A: numbered
informal passages illustrating intended human behaviour, set off as
blockquotes, restricted to sections where behaviour is assumed, and
cross-linked to the Appendix A.2 assumptions they dramatise. Rationale: the 0.3 review
found narrative and derived claims typographically indistinguishable — a
reader could not tell which sentences were computed and which were felt. Four
exemplars added (V1–V4), two by extracting analogies already present as formal
prose. A vignette/spec mismatch is a required conversation, not an errand;
neither wins automatically. New review pass 0.8b checks the agreement.

### 2026-08-14 (PoP and adoption)

A previously unstated assumption made
explicit: §6.1.1 adds an optional proof-of-presence reference to adoption,
expected on fresh adoptions, not enforced (no global enforcement point exists,
§3.1.1), weighted by policy. §7's trust-upgrade framing widened from peering
to any relationship a presence record attaches to, which is what makes the
unenforced version safe. §7.1's ceremony summary completed with the guided
capture and automatic verifier-query steps, both previously described only in
their own subsections. Vignette V5 added on the ceremony as social ritual, and
assumption A11 registered: that ordinary users tolerate the friction — the
honest-user mirror of A1.

### 2026-08-14 (visibility corollary)

New §1.1 states as a first-class
principle what had been rediscovered four times: enforcement is possible
exactly where shared state exists, so beyond that boundary visibility replaces
enforcement. Gives implementers a diagnostic rather than a slogan, and supplies
the missing reason why the evidence schema alone cannot be pluggable — a
distinction the schema cannot express is one no policy can act on. Review pass
0.5 gains a second extraction for unenforceable mandates.

### 2026-08-14 (rule fragility)

Review pass 0.5 returned 18 rules coupled to
identifiers rather than roles. Resolution recorded in Appendix A: invariants are stated
in role terms here, encodings in the wire format, and neither substitutes for
the other. Seven given role-level statements: the veto exemption (restated a
second time — the previous "fix" swapped one type name for two), trust-bearing
operations now defined rather than enumerated, formation-record distinctness,
verifier-selection seeding, currency TTL derivation, locator authentication,
and agent-grant capacity. Several flagged rules already had role statements
(witness cross-nomination, subject-side rate limiting) and were left alone.

### 2026-08-14 (0.5 verification)

Verification pass on the eighteen rule
fragility findings: 6 addressed, 2 addressed with new problems, 10 not
addressed. All ten now carry role-level invariants, and Appendix A records that the
convention was introduced in the same edit that left them — a stated discipline
with known exceptions being worse than none. Two new problems fixed: the
verifier-selection encoding was **grindable** (seeding from participant-chosen
`started_at` let a participant retry until the sample favoured it), now
requiring witness nonces revealed after participant commitment; and the
challenge-window paragraph still said departure was vetoable three lines above
the invariant saying it is not. Currency TTL now states what would derive it
(detection latency, unmeasured) and marks the value chosen rather than derived.

### 2026-08-14 (implementation attempt)

Review pass 0.6, encode/sign/verify
an adoption transaction in Rust. Two flat contradictions found: COSE algorithm
identifiers declared `uint` when the required algorithms are negative
(EdDSA −8, ML-DSA −48/−49/−50), and a recovery grammar requiring one-or-more
verifiers that made attested rotation unrepresentable. Larger finding: the spec
claimed COSE and then defined its own signature map — **now uses real RFC 9052
COSE**, which supplies domain separation via `Sig_structure` across the three
signing contexts and fixes two signature inputs that were undefined. New §5.1
specifies what hybrid identity means concretely (keyhash covers both
components; routine transactions classical, presence records and identity
changes both), which was previously a category rather than a construction.
§5.2 records that the PQ crates are unaudited. Full disposition of all findings
in `review-tracking.md`.

### 2026-08-14 (COSE residue)

Self-check after the 0.6 edits found five sites
in this document still describing signatures as `sig over H(body)` while the
wire format had moved to COSE. Corrected, and §8.1 now marks the wire format
authoritative on encoding so the duplicated schema is a reading aid rather than
a competing source of truth.

### 2026-08-14 (unenforceable mandates)

Review pass 0.5.2 found nine MUSTs
crossing §1.1's enforcement boundary, several written after §1.1 itself. Fixed
at the root: Appendix A now fixes a normative vocabulary — MUST only where a recipient
can check the rule from evidence it holds, otherwise "the reference client" or
"the reference policy". All nine sites reworked. Two became better mechanisms
rather than weaker prose: attestation delivery now MUST carry the requesting
evaluator's nonce, making unsolicited copies visibly unsolicited and locally
rejectable; and verifier responses MUST carry the subject's countersignature
over the query, so unauthorised disclosure cannot be laundered into the trust
graph.

### 2026-08-14 (LINDDUN)

Review pass 0.7, 23 analysis units. New §16.5 gives
privacy a home for the first time; it previously existed only as local argument
at each mechanism, which is why the central finding went unnoticed. **§19.1
states the composition invariant**: privacy must be assessed under composition
of everything an observer can obtain, never artifact by artifact. §7.2 had
said exactly this for biometrics and never generalised it. New finding P2: a
presence record leaks a sample of the subject's *prior* counterparties via the
verifier list, and neighbourhood structure via witnesses — §8.1's
"strictly dominates" claim about carrying verifier responses is corrected to
"a trade". §19.3 proposes Merkle-ised record bodies for selective disclosure.
Nine deliberately accepted privacy costs consolidated in §19.7.

### 2026-08-14 (V6)

Vignette on the composition tradeoff added to §19.1,
with two amendments to the proposed text: aggregation cost restored as part of
privacy (identical facts, different access cost and evidentiary weight — the
argument this document already makes about camera networks), and the exposure
baseline scoped to a median user rather than asserted universally. Registered
as assumption A12, marked known-false for part of the population. Appendix A notes that
V6 is persuasive rather than illustrative, a genre that must carry its own
counter-argument and a statement of what it may not be used for.

### 2026-08-14 (camera comparison withdrawn; P12 found)

The Ring/Flock
comparison was wrong twice over: those are designed aggregation services, not
examples of high-friction fragmented data, and the "zero access cost" claim was
already false for this design because attestation has been pull-only since §14.
New §19.1.1 records the error and states disaggregation as a positive
property — §16.5 had catalogued composition risk without crediting what limits
it. New §19.1.2 records **P12, Critical: end-to-end payload encryption is not
specified**, so both serving infra nodes currently see plaintext. The patron was
accepted as a metadata chokepoint, never a content one, and the distinction was
never written down.

### 2026-08-14 (resources)

New §10 defines the resource object: services and
data stores owned by a node, addressed relative to it, with access adjudicated
by the requester's position. Notable as the one place §1.1's diagnostic comes
out affirmative — owner and requester share topology and the owner runs the
service, so access control is genuinely enforceable rather than merely visible.
Resolves the agent question — now §23.2 — structurally: *borrows authority → resource; bears its own costs
→ node*, which makes an agent a resource with permission to act, and makes its
inability to accumulate independent standing structural rather than asserted.
Object in scope; permission vocabulary and interaction protocol deferred.

### 2026-08-14 (payload confidentiality)

New §12.2 sketches requirements for
end-to-end encryption to the *addressed endpoint*, naming the leaf-to-leaf,
leaf-to-patron and leaf-to-resource cases. Separates the patron's two roles:
ciphertext as relay, plaintext as endpoint — conflating them is how "the patron
sees everything" returns. Enforcement is affirmative under §1.1 (the client
withholds the key). Two forced consequences recorded: store-and-forward makes
asynchronous key agreement mandatory, so prekeys and their exhaustion,
last-resort and rotation problems arrive with it; and §5 specifies signing keys
only, saying nothing about payload KEM keys or their binding to identity.

### 2026-08-14 (adversarial: hostile client)

Four findings, all upheld.
**§16.4's patron-eclipse justification withdrawn**: "no asset to steal" is true
and irrelevant, because the asset is the victim's authentication view, which is
this network's primary product — and §11.4's no-cold-lookup property *amplifies*
eclipse rather than being neutral to it. Subnet plurality promoted from side
effect to structural mitigation. **Selective-abort grinding closed**: commit-
reveal prevented changing the nonce but not aborting the attempt; fixed by
seeding from the participant pair plus a time window and by revealing nonces
only after the physically expensive steps, a point previously unspecified.
**New §8.1.1 records a structural gap**: the evaluator learns the subject's
history size from the subject, so selective disclosure collapses the verifier
threshold and the anti-suppression property. Per-subnet presence chain proposed,
not decided. **P13 added**: retention promises are undetectable against a hostile
client, since illegally retained photos can be laundered as `personal_knowledge`.

### 2026-08-14 (chained archive; PoP countersignature removed)

Two changes
from the 0.8 findings. **Patrons no longer countersign PoP** (§6.4): the patron
was not present, PoP sits outside the subnet trust envelope, and a patron able
to veto it shapes what later patrons see. This also makes the eclipse attack
permeable, since witnessless ceremonies (§13.2) let an eclipsed user memorialise
any real meeting unsuppressably. **The archive becomes a hash chain** (§8.1.1):
each transaction carries a hash of the subject's previous one, so excision is
impossible and only truncation to a prefix remains — which is self-defeating,
because history is standing. Finding 3 dissolves rather than being mitigated.
Emergent and undesigned: **the archive is a second factor** — key without archive
yields no portable reputation.

### 2026-08-14 (self-review sweep)

Eleven propagation and staleness defects
found and fixed without external review. Notable: §1.1 still asserted the
retention-detectability claim withdrawn after 0.8; §14 omitted resources and
contradicted §12.2 on who holds payload; the build order omitted the chain,
which **cannot be retrofitted** since back-pointers must exist from the first
transaction; and **the chain had been applied only to presence records when it
covers every transaction type** — corrected by moving back-pointers to a common
body field, one per required signer. Chain genesis resolved as
SHA-256(signer keyhash). New assumption A13 (users retain their archives) and a
seed-window parameter added. Full list in `review-tracking.md`.

## 2026-08-15

### 2026-08-15 (unspecified areas)

New §22 lists features the design does not
address at all, kept separate from §21's open questions about specified
mechanisms. Four load-bearing: **negative attestation** (every edge carries
positive capacity; nothing can say "this identity defrauded me" peer-to-peer,
and the cure may be worse than the asymmetry), **succession** (no mechanism for
a permanently absent patron, so teams cannot outlive their members),
**multi-device** (one keypair per device is unviable and collides with the
archive-as-second-factor property), and **intra-subnet discovery** (the one
place a directory is compatible with §1.1, and urgent now that resources exist).
Five contained: group operations, state sync, resource abuse controls, time and
ordering, invitation flow.

### 2026-08-15 (feature gaps tested)

Nine candidate gaps from §22 tested
against the existing design; **five dissolved**. Negative attestation is
answered by disavowal plus cessation-and-payload, and by the scope statement now
in §1: *we do not produce trust, we capture and formalize it.* Succession is
graceful degradation — everything survives an absent patron except subnet-scoped
countersigning — with one new consequence recorded, that a dead anchor cannot
publish a forwarding record. Group operations become a standard-library
abstraction over pairwise messaging; resource abuse becomes resource policy plus
one report event; cross-identity ordering is unnecessary given per-identity
chains and witness-corroborated timestamps. **Two remain real**: multi-device,
where storage is solved but chain-fork semantics are not, and intra-subnet
discovery, where DNS-SD provides near-direct prior art.

### 2026-08-15 (chain forking)

Followed through on the multi-device fork risk.
**A deliberate self-fork reintroduces selective disclosure**: both branches
inherit the full pre-fork history and its standing while each omits the other's
post-fork transactions, which beats permitted identity plurality where each
identity starts from zero. §8.1.1's "truncation to a prefix is the only edit" is
corrected to **prefix plus one divergent tail per branch maintained**. §22.2
reframed from usability tradeoff to security decision — only the single-primary
shape prevents forking structurally. Partial mitigation added: publishing the
chain head with the currency attestation makes a fork detectable on contact.

### 2026-08-15 (header corrected)

Two stale values in the document header. The
date was stamped 2026-08-12 throughout, having been carried forward from the
first entry rather than checked. *(Superseded: all dates were reconstructed from
transcript timestamps later the same day — see this log's header.)* **The status line was also stale**, claiming wire formats and session
establishment were unspecified when `wire-format.md` exists and §12.1 closed
session establishment. Both are the same stale-value failure the review passes
kept finding in the design text — committed in the header and in the record of
finding it, which is a reminder that the discipline applies to process artifacts
and front matter, not only to the specification body.

### 2026-08-15 (fork finding retracted)

The author's claim that deliberate
multi-device forking reintroduces selective disclosure is **withdrawn**. §13.7
explicitly disclaims cross-subnet accountability, so presenting different views
to different subnets attacks no stated guarantee — the finding was generated
against an imagined requirement. §8.1.1's original claim is restored and scoped:
the guarantee is **per-evaluation**, and within one subnet's view truncation to a
prefix remains the only edit. §22.2 reframed around the real problem, which is
**accidental** forking from concurrent device use — silent loss of the user's own
history rather than an attack. Shape 3 (per-device keypairs with published
bindings) identified as the likely answer, since it removes the shared chain
entirely and preserves offline signing.

### 2026-08-15 (archive merges)

Forked archives can be **merged**: a
transaction following a fork carries back-pointers to both branch heads. This
**strengthens completeness** rather than merely repairing divergence, since the
merge commits to both branches and omitting one leaves a back-pointer
unsatisfied. The archive is properly a **Merkle DAG**, not a chain; cross-branch
ordering is deliberately not recovered, because the structure proves no
intermediate record is missing rather than ordering events (§22.4 makes the same
argument for cross-identity ordering). **Supersedes all three multi-device shapes
in §22.2** — no primary device, no head-check before signing so offline signing
works, and no device-binding declaration since the merge is self-describing. Wire
change: back-pointers become a list per signer.

### 2026-08-15 (resource layer specified)

Four items moved from resolved-in-
principle to specified. **§10.4 permission scopes**: access expressed as a
region relative to the owner, with the horizon bounding the vocabulary naturally
since relative position is not computable beyond it — only explicit grants reach
further. **§10.5 service catalog**: DNS-SD-shaped entries signed by the owner,
propagating as topology and filtered at the source by `discover_scope`, so
browsing shows what you can plausibly use. **§10.6 abuse reporting**: one event
type addressed to the owner, never broadcast, with response left entirely to
resource policy — which keeps it clear of §6.2.2's reasons for having no
peer-to-peer negative attestation. **§12.3 group operations**: a client library
over pairwise sends, with its non-guarantees stated so callers do not assume
atomicity or ordering the network cannot provide. Wire types 6 and 7 added.

## 2026-08-16

### 2026-08-16 (resource layer agreed)

§10.4 permission scopes, §10.5
service catalog, §10.6 abuse reporting and §12.3 group operations agreed as
written; PROPOSED markers removed and wire types 6 and 7 promoted from [P] to
[D]. Note these were agreed from the chat description rather than from a
document read-through, so §22's remaining entries and the open-question list
are the right places to check for anything that did not survive the transition
to written form.

### 2026-08-16 (consent countersignature; vignettes V7–V8)

**The subject now
countersigns every verification query about themselves**, replacing the standing
disclosure policy, which is removed from the presence record. This is stronger
than notification-plus-aggregation: a querier could send *different* fuzzed
profiles to different verifiers, one probe each, defeating a counting defence —
per-query countersignature lets the subject require identical profiles across a
ceremony, giving one probe point instead of *n*. Availability is free because the
subject is physically present at the ceremony the query is bound to. Retention
commitments remain, governing storage rather than disclosure. **V7 (jury
nullification) and V8 (postal model) extracted from §3.1.1**, where they had sat
as formal-voiced prose since before the vignette convention existed; Appendix A now says
to check for that case first, since extraction has produced three of eight
vignettes.

### 2026-08-16 (pre-review sweep)

Propagation check before re-running the
review plan. Fixed: §22.2 still told the reader to state a degradation
"whichever shape is chosen", though merges (§10.3) had superseded the three
shapes; §16.5 had no entry for the resource layer, specified after it was
written, so **P15** (catalog entries reveal what a node runs to its whole
horizon, and the set of them is a fingerprint) and **P16** (abuse reports
accumulate as signed complaints at the owner) were added. §16.5 now carries a
coverage caveat naming the four subsystems specified after pass 0.7, since
hand-added findings are not a substitute for a systematic re-run.

### 2026-08-16 (0.1 factual verification, second run)

Four contradicted, two
substantive. **COSE does not supply application-role domain separation**: the
`Sig_structure` context is `"Signature"`/`"Signature1"`, distinguishing COSE
structure types rather than roles — and this protocol had grown to **seven**
signing roles on the strength of a property it did not have. Wire §1.1 now
requires a distinct `external_aad` role tag per context, reconstructed by the
verifier from context rather than content. **UWB is not categorically
relay-resistant**: USENIX Security 2022 demonstrated physical-layer
distance-reduction against deployed 802.15.4z HRP parts including Apple U1,
without key knowledge. §7.6.3 downgraded — **no channel is categorically
relay-resistant** and all proximity evidence is probabilistic, which is
consistent with §7's existing "a cost, not a primitive" framing but changes the
confidence a policy should place in a record. Also: `alg` in the protected header
is a profile rule rather than a COSE guarantee; background networking is
constrained rather than impossible; DNS-SD puts the endpoint in **SRV, not TXT**,
so the catalog gains a separate metadata field; and the §7.1 liveness claim is
registered as unsourced.

### 2026-08-16 (0.2 coherence, second run)

26 contradictions, all fixed. **Two
blocking.** A light client was told to dial its patron while §3.3 permits
light-client patrons with no static address — a genuine architectural error, now
corrected to **dial the nearest serving infra node**, with serving and
countersigning named as separate roles. §11.5 still called the anchor table
globally replicated. Of the serious findings, three changed substance rather than
wording: the **eclipse-permeability claim is scoped to new joiners**, since a
witnessless ceremony is only valid where the finalization threshold is zero;
**patrons do see presence records** (they store them) and merely do not
countersign them; and **the archive provides ancestor reachability, not total
order**. Also: merges do not invalidate anchors, abuse reports get their own
storage class so they cannot become public accusations, and §6.3 now actually
states the ASN/prefix fields §3.4 had been citing it for.

### 2026-08-16 (direct payload path)

NAT traversal returns for payload only.
Infra nodes act as **STUN and TURN**, which adds no capability they lacked —
relaying payload is what a TURN server does — and makes the relay a **fallback**
rather than the default. §14.1.1's "there is no NAT traversal problem" is
withdrawn: it held only because payload was always relayed. Three consequences.
**Infra economics improve materially** — §16.6's ~$20/month was never checked
against relaying all payload for 1,110 subordinates, and bandwidth would have
dominated it. **The relay path stays first-class**, since symmetric NAT and CGNAT
defeat hole punching and two mobile peers is both the worst and the common case.
And **new finding P17**: a direct connection reveals each peer's IP to the other,
trading serving-node metadata for counterparty location signal, so
direct-versus-relay must be a stated choice rather than a silent optimisation.

### 2026-08-16 (direct path horizon-limited)

Direct payload connections are
confined to the ±2 tier horizon; everything outside relays. The horizon is the
right boundary because it is **already the set that holds your locator and
topology** (§11.1), so IP is incremental disclosure inside it and novel disclosure
outside — and the check needs no new state. P17 downgraded to Low–Medium
accordingly, with the residual noted: the horizon is bounded rather than chosen,
and includes cousins a user may never have met, so both defaults stay
overridable. **A2 gains a fifth dependent** and its failure would now collapse
§16.6's infra economics as well. §14.1 gains a table of the horizon's five jobs,
since *h* is now the most over-loaded parameter in the design.

### 2026-08-16 (0.3 unjustified claims, second run)

36 findings in three
severity bands, against 280 undifferentiated in the first run; the rubric added
after that run is what made the output actionable. **Structural finding: §17.1
and Appendix A.2 are orthogonal registers and were not linked.** Six claims were
unsourced *and* load-bearing while appearing only in §17.1 — matcher
reconstruction cost, TURN relay fraction, the PAD claim, face entropy, the
ageing regime, and infra costs. Added as **A14–A19**, and the orthogonality is
now stated. **A17 is singular**: the only assumption used to *reject* an
alternative rather than support a choice, closing off fuzzy commitments as a way
to keep verification without keeping biometrics. §16.3's "peering is the cheapest
route" superlative **withdrawn** — unattested adoption is plausibly cheaper —
though the mitigation stands, since it needs peering to be cheap and
endorsement-invisible rather than cheapest. Seven intensifiers replaced with
figures or mechanisms per Appendix A's intensifier rule.

### 2026-08-16 (trust ceiling framing withdrawn)

§16.3 rewritten. **There is no
trust ceiling**: standing is per-observer, and a peering edge is visible only
within the two peers' horizons, so it conveys nothing to anyone outside them. The
concern is therefore local and targeted — *gain standing with a particular victim
via a favour from their neighbour* — not a route to global standing. What
survives is that peering reads as a technical request rather than an endorsement,
which is exactly what the low default flow capacity addresses: **the mitigation
was right and the justification was wrong**. New **§16.3.1** states a principle
that was nowhere in the document: **the min-cut bound is observer-relative**.
Different observers hold different edge sets and compute different cuts, so
§16.3's Sybil bounds are per-observer, invisibility is conservative rather than
exploitable, and an attacker must work per-target rather than accumulate edges
globally.

### 2026-08-16 (0.4 parameter inventory, second run)

138 parameters
inventoried jointly across both documents, 30 unset. **Joint review found what
single-document review could not**: the presence-record size conflicted (~35 KB
design vs ~40 KB wire), the transaction-type count was stale at five against
seven, and the **envelope signer bound of 12 contradicted a witness bound of 16**
— a record one bound permitted the other rejected. The signer bound is now
**derived** from the per-role bounds rather than asserted, making the
incompatibility structurally impossible. Eleven arrays lacked the maximum that
wire §1 requires of every array; all now bounded. **"Current-keys assertion" and
"currency attestation" were one object under two names**, which had produced two
inventory entries with independent unset TTLs — unified. New **§20.1**
consolidates all unset parameters, grouped by whether settling each needs a
security argument, measurement, a policy decision, or an encoding decision.

### 2026-08-16 (0.5.1 rule fragility, second run)

Six residual
identifier-coupled rules restated in role terms; the reviewer confirmed six
previously-fixed instances are holding. **Generalising the signing-tier rule
exposed that §5's justification was invalidated by the archive chain.** §5 said
routine transactions could be classical because "signatures need only hold until
nobody relies on them" — but §8.1.1 later made the archive a hash chain, and
verifying a history means verifying its records' signatures, so reliance lasts as
long as the archive is presentable. Once classical signatures are forgeable,
fabricating a record needs only a forged counterparty signature, defeating
§8.1.1's position guarantee. **All archive-retained transactions are now
PQ-signed** — ~8 KB each against a photo store at ~100 MB. Note the shape: every
prior propagation failure ran forwards, this one ran **backwards**, a later
decision undermining an earlier justification while leaving its text looking
correct. Greping for changed terms cannot catch that class.

### 2026-08-16 (0.5.2 unenforceable mandates, second run)

Two findings against
nine in the first run; §1.1 is largely holding. **Both survivors were written the
same day, after §1.1 existed.** §15.2 stated the principle correctly and then
mandated a foreign implementation's λ seven lines later — now restated as a fact
about the arithmetic (a metric that does not decay steeply enough **diverges**),
with the policy descriptor — bad news only — as the visible artifact. §10.6's rule about
intermediaries not retaining abuse reports **was introduced by the Appendix A
role-generalisation earlier the same day**: the original delivery property was
enforceable at the sender and the generalisation reached into foreign storage.
Split into the enforceable part and a visible distinction. Appendix A now records that
**generalising a rule can break §1.1** and that the two conventions must be
applied in order — state the property, then ask who would enforce the restated
version against whom.

### 2026-08-16 (0.7 LINDDUN, second run)

Full re-run over 28 flows and stores,
discharging the coverage caveat. 31 threats and 10 correlation findings; the
individual threats mapped almost entirely onto P1–P17, and **the new material was
overwhelmingly compositional**. New **§19.8 correlation register** — the
structure §19.1's invariant required and the document lacked. Four new
individual findings, P18–P21. **C4 is the most instructive**: the same archive
means different things to different observers, so *moving* one changes which
entries are legible and **archive portability carries a destination-dependent
privacy cost** — a shape the composition invariant did not anticipate, since it
composes an artifact with the observer's own knowledge rather than with another
artifact. **C9 is the one to design against**: face archive plus transaction
archive on one device is a face-to-key-to-social-history database, which is what
the network architecture works to prevent any single party assembling. Also fixed
an implementation-blocking stale rule: the wire format still denied NAT traversal
after the design adopted ICE.

### 2026-08-16 (0.6 implementation attempt, second run)

Encode/sign/verify for
adoption **could not be completed**: §5.1's hybrid-signing requirement, adopted
earlier the same day, was incompatible with the wire format's exactly-two-signers
rule, and no encoding existed for a two-component identity. Both fixed —
`KeyMaterial` is now a two-entry COSE_KeySet in fixed order with ML-DSA-65
selected, and signer rules count **logical signers rather than entries**, each
contributing two `COSE_Signature` entries sharing a `kid` and distinguished by
`alg`. **Evidence embedded inside a signed body stays classical**, protected
transitively by the envelope signature — without that limit a recovery adoption
with 32 verifier responses would exceed 400 KB. Three internal contradictions
fixed: `uint .size 8` against shortest-form encoding, "recovery field 2 may be
empty" against "empty arrays MUST be omitted", and a back-pointer bound of 8
against "accept any length ≥ 1". `VerificationQuery` defined, so the embedded
signature payloads are now implementable.

### 2026-08-16 (WASM storage claim corrected)

An earlier claim that a WASM
target has no filesystem access was **browser-specific stated as general**, and
is withdrawn: WASI gives native WASM ordinary file access, an embedded component
gets host bindings, and even in a browser OPFS provides real file handles. **What
survives is durability**: browser storage is evictable under storage pressure
unless persistent storage is granted, and since §10.2 makes the archive a
second factor, silent eviction costs a security property rather than only
convenience. §22.2 now states this, and a browser-hosted client needs
§13.7.1's backup path to be mandatory rather than advisory.

### 2026-08-16 (0.6 target 2, resolve a locator)

**The resolution
request/response protocol did not exist.** The design described resolution
narratively across §11.3 and §12.6.1 and no wire message carried it, so the
network's primary operation was unimplementable. New `wire-format.md` §7.6
specifies it. Two further findings of substance: the **locator authentication
contradiction** — design §11.1 requires the node's signature, the wire format
claimed an enclosing envelope always supplies it — resolved by distinguishing
carriage, with a `SignedLocator` required standalone and a bare locator rejected;
and **descent is through infrastructure only**, terminating at the serving infra
node and returning the residual path suffix rather than traversing it, which is
what lets a path address a light client nothing can route to directly. That was
implicit and is now in §12.6.1. Anchor entries are now self-signed — they vouch
for nothing, but an unsigned entry lets a gossip peer inject addresses and
partition the network.

### 2026-08-16 (0.6 target 3, validate a presence record)

30 findings. **The
verifier-selection invariant could not be recomputed from the record**: §8.1.2
requires third-party recomputation so a missing verifier is visible, and the
witness nonce commitments and reveals it depends on were nowhere in the presence
record — the anti-suppression property was unverifiable. Now fully specified in
`wire-format.md` §5: nonce carriage in `Witness`, a domain-separated
identity-bound commitment, canonical seed bytes with participant order
canonicalised so it cannot become a grinding variable, **a 24-hour epoch-aligned
window** (filling the §20 UNSET), hash-rank sampling, and candidates as
**distinct counterparties**. The threshold gains a necessary third term —
`min(floor(n/2), 10, |candidates|)` — since *n* counts transactions while
candidates are people. `pending` and `unavailable` count toward finalization,
because the alternative lets an attacker block it by making verifiers
unreachable. Consent signs `query_id` rather than the query, keeping a 4 KB
profile out of the record while remaining verifiable. Several structural rules
were simply absent and are now stated; `duration_s` removed as derivable.

### 2026-08-16 (verifier recomputability scoped)

Two corrections. §8.1.2's
claim that selection is "recomputable by any third party" is **stronger than
achievable**: checking a subject's verifier set needs that subject's candidate
set, which comes from their archive, so the property is **per-subject and
holder-relative** and nobody validates both halves of a record without both
archives. That scope is correct — an evaluator assessing A cares whether *A*
suppressed verifiers — but was unstated. Second, a **binding invariant** added at
`wire-format.md` §5.2: a subject's selection must be recomputable from the
record plus that subject's own history and nothing else, so the seed may draw
only on record-carried values and the candidate set only on the subject's own
history. The current construction satisfies this by accident of design; adding
counterparty-derived entropy would keep every property §8.1.2 asks for while
making selection **silently unverifiable**, and would not show up in testing
because a developer holds both archives.

### 2026-08-16 (verifier recomputability, direction corrected)

**A selects
verifiers from B's history**, so the party performing a selection is never the
party it is about. The property is therefore stated from the **subject's** side:
B must be able to demonstrate B's verification was honestly conducted, from the
record plus B's own history. The earlier evaluator-centric framing pointed at the
wrong party and obscured the harm — if the seed is not reconstructable from B's
own holdings, **B can never clear themselves**, permanently, since the record is
immutable. **New ceremony step 7**: each party verifies the other's selection
before signing. If A selects B's verifiers off-seed and B signs anyway, B holds
an unprovable record forever. **This is the only ceremony check that protects a
signer against their counterparty** — every other guards against outsiders or
against the pair colluding, a threat direction the ceremony had not addressed.

### 2026-08-16 (0.6 target 4, client attach with sibling failover)

14
findings, two unsatisfiable. **A zero-sibling topology had no valid encoding**:
`AttachAck` required one-or-more siblings while §1 forbids encoding an empty
array, so an infra node with a single child could produce no conforming ack.
And **§14.1.2's requirement that degraded attachment "must be explicit" had no
wire representation** — `Attach` now names the client's intended serving node and
`AttachAck` echoes the receiver's determination, so a disagreement surfaces
rather than one side permitting operations the other considers unavailable.
**The PQ transport handshake was also unspecified**, now settled with a standard
rather than an invention: QUIC + TLS 1.3 with group `X25519MLKEM768`, peer
authentication by raw public key (RFC 7250) since identities are keyhashes and
there is no CA, and ML-KEM-768 as the profile set. Heartbeat units, counter
semantics, failure threshold, failback, queue-depth meaning, endpoint port and
the sibling-update frame were all undefined and now are.

### 2026-08-16 (degraded-attachment "explicit" corrected)

§14.1.2's requirement
that degraded attachment be explicit is a **user-interface obligation**, not a
wire one; the author had misread it and added an `Attach` field for the client to
assert its intended serving node, now removed. The supporting argument was also
wrong: a sibling can determine the mode from its own topology, since a client
attaching to it sits two hops away within its horizon. `AttachAck`'s mode field
is kept for a better reason — it reports the **server's** determination, because
a client with stale topology may not know which state it is in. §14.1.2 now states
the UI obligation as a reference-client requirement, of the same kind as
§19.6's disclosure rules.

### 2026-08-16 (0.7 LINDDUN, third run)

Independent walk over 42 flows and
stores; most findings mapped to the existing register. **Three new
correlations.** C11: a prekey fetch precedes any message, so a patron sees intent
separably from delivery and **communications that never happened become
visible**. C12: the verification-query log is a *defence* that becomes an
exposure — retained beside the archive it timelines ceremony attempts the network
never recorded. C13: enumerated disavowal reasons avoid a defamation surface, but
**context supplies the semantics the code omits** when joined with resource
history. **New P23**: §19.6's capture-time disclosure obligation covered
participants only — a verifier who answers permanently proves they met the
subject, and the consent machinery protects the subject while asking the verifier
nothing. **P22**: query logs have no retention rule. §19.7 gains an accepted
cost for the absence of erasure at the evidence layer, noted as a compliance
posture needing a documented lawful basis rather than a deletion mechanism.

### 2026-08-16 (resource layer propagated)

`resource-requirements.md` folded
into the documents. §10 rewritten around the execution model: **the infra node
is the front door and the resource sits behind it**, so a resource never reads
network state and a compromised one leaks only its own data. New §11.0.1:
**wider reach is federation, not wider scope** — a subnet-wide service is many
local instances, each administered by a patron, which makes §1.2's freedom
argument an architectural property rather than a claim. §10.2 states
**membership in the owner's Dunbar Org as the precondition for all access**, so
departure revokes uniformly and no access accumulates; it also separates
ownership from hosting. §10.4 adds package-declared roles, predicate binding,
and **rank-based rather than raw trust thresholds**, since a raw threshold is
denominated in units meaningful only within one metric family. §10.5 splits the
signed `CatalogEntry` from the personalised catalog page and notes role names
leak. New §10.7 on gateways re-concentrating what the architecture
disaggregates, with broker-rather-than-proxy as the default. New **P24** (gateway
operators see external traffic), **A21** (patrons will administer resources — the
resource-layer sibling of A11), **an open item since removed as out of scope** (external root-of-trust bootstrap is
unmapped), and a new accepted risk in §16.4: **a compromised infra node can forge
its subordinates' access**, though not their transactions. §1.2 gains the SSO
adoption observation; §14.1's horizon table gains a sixth job.

### 2026-08-16 (agent deferral withdrawn)

§4 still deferred delegated-agent
participation while requiring its schema constraints be settled immediately. Both
halves are stale: §10.3 makes a delegated agent **a resource**, and §10.2
already gives resources identity without consuming subordinate slots — so there
is nothing left to settle in the schema. §23.2 narrowed to the one question that
remains open, **what "presence" means for a persistent self-directed AI**, and §4
now notes that autonomous participation is deferred as a *decision* rather than
as a mechanism: such a participant is an ordinary node.

### 2026-08-16 (payload encryption: import, not invent)

§14.2.4 replaced.
**Directly applicable prior art exists, specified and formally verified**, so the
requirement is to adopt rather than design. **PQXDH** for asynchronous key
agreement — the exact situation of a recipient offline with published prekeys —
whose published second revision incorporates fixes from ProVerif and CryptoVerif
analysis (USENIX Security 2024). **The Triple Ratchet** (Double Ratchet plus
SPQR, released October 2025) for session forward secrecy and post-compromise
security, whose erasure-coded chunking already solves the ML-KEM key-size problem
this design would otherwise hit. Two narrowing observations: **only leaf-to-leaf
needs any of it**, since patron and resource endpoints are online by definition
and covered by the §14.1.1 transport handshake; and §14.2.2 had independently
reconstructed the X3DH prekey shape, so importing replaces a hand-rolled version
with the analysed original. One place to exceed the deployed profile: PQXDH's
published revision authenticates classically, and §5.1's hybrid identity permits
binding to the post-quantum component too. Also noted: PQXDH's **deniability** is
the property the data plane wants, precisely inverting the non-repudiation the
control plane requires — the split is correct rather than a compromise.

### 2026-08-16 (payload encryption propagated)

The §14.2.4 adoption carried
into the companion documents. **`wire-format.md` §7.7 adds prekey distribution**
— `PrekeyBundle`, `PrekeyRequest`, `PrekeyReply` — with the bundle **opaque to
this protocol**, since only the endpoints hold the state to interpret it and
carrying it as a blob means a PQXDH revision forces no wire change. Serving a
one-time prekey consumes it; falling back to the last-resort key is a declared
reduction in forward secrecy rather than a failure. Light-client obligations added
for publishing and replenishing bundles, and for not prefetching speculatively
since a fetch discloses intent to message. Infra-client obligations added for
holding and serving bundles, telling a subject when their pool is exhausted —
an attacker can drain it — and keeping no record of who requested whose bundle.
Prekey rotation cadence added to §20.1's unset list.

### 2026-08-16 (blanket prekey prefetch)

The intent-disclosure problem (C11) is
addressed by **prefetching reusable prekey material for the whole Dunbar Org
uniformly and by default**, rather than by discouraging prefetch. A uniform fetch
carries no information about intentions, and its timing is driven by peers'
rotation schedules — so it reveals **past** activity rather than **future**, a
strictly weaker disclosure. **One-time keys are excluded**, and the exclusion is
what makes it work: serving one consumes it, so blanket prefetch would drain every
pool in the org and **make exhaustion the normal state**, destroying its value as
a signal that someone is draining a pool deliberately. The wire bundle is split
accordingly — reusable material served freely, a one-time key requested only when
opening a session. Cost: a session opened from prefetched material alone lacks
one-time-key forward secrecy for its **first message**, with the ratchet covering
everything after. Prefetch scope is the horizon's **seventh** job (§14.1).

### 2026-08-16 (archive presentation: head txid)

The last blocker to finalising
an adoption transaction is closed. Adoption field 7 is **a single head txid**, not
a list, range or proof: §8.1.1's chain already carries the predecessors, so a
patron walks backward from the head and fetches in batches (`wire-format.md`
`wire-format.md` §7.8). Truncation needs no grammar — presenting less history means presenting an
earlier head, the only edit the chain permits. **The patron chooses its own
depth**, which is the right asymmetry: the presenter picks the head and cannot
control how far back the recipient looks. The three alternatives were rejected on
specific grounds — a **list** duplicates the chain while proving nothing, a
**range** is *reachable from B but not A*, which is precisely the excision §8.1.1
exists to prevent, and a **Merkle proof** optimises away a fetch the patron needs
anyway to recognise counterparties. New `wire-format.md` §7.8 specifies batched
fetch, with the requester verifying the chain itself since a holder cannot be
trusted to have walked it correctly.

### 2026-08-16 (disavowal reason codes)

`wire-format.md` §4.3 defines a
**64-value space banded so that prejudice is structural**: codes 0–31 without
prejudice, 32–63 with. A policy can evaluate an unfamiliar code correctly —
`code >= 32` means an adverse judgment — without a lookup table or a
specification update, which matters because most values are undefined in v1 and
will be assigned later. Ten codes assigned. **Codes 32+ are the vocabulary for
the negative attestation §6.2.2 describes.** Code 4, incompatible subnet
membership, sits without prejudice deliberately: the patron is asserting
something about *that subnet*, not about the subordinate. Also clarified: the *(since removed)*
external root-of-trust bootstrap is **not** the genesis procedure — §13 is
bootstrapping inside the network, while that item was about an outside consumer
deciding which issuers to recognise. §21 item 10 marked **deferred by decision**, not to be
implemented in this or any intervening version.

### 2026-08-16 (two items moved out of scope)

**Negative attestation and the
external root-of-trust bootstrap are no longer listed as gaps**, because neither
is a wanted future feature. The negative-attestation material moves into §6.2.2
where it belongs: **disavowal is the network's negative attestation and the only
one**, and no peer-to-peer equivalent is planned — per §1 the network captures
trust rather than producing it, so a warning about someone travels as payload
between people rather than as protocol state. §6.2.2 also now states that
**disavowal is positional**: it ends one patron-subordinate relationship, the
node stays in the tree, and for several reason codes re-adoption elsewhere in the
same subnet is the expected outcome. The reason code is therefore **information
for the next patron** rather than a broadcast. External root-of-trust is removed
entirely: which issuers an outside consumer recognises sits between the patron
and the resource, and is the responsibility of IdP plugin developers.

### 2026-08-16 (capability negotiation)

`wire-format.md` §8.1 replaces the
placeholder with a **QUIC-shaped parameter map**, sent in both directions, with
unknown parameters ignored rather than rejected — the same treatment §1 gives
unknown map keys, and deliberately unlike unknown enum values. Parameters carry
**values rather than flags**, since several natural capabilities are quantities
(largest archive batch, prekey construction, repair depth) that a bitfield could
not express. **Absence of a capability is never a connection failure**: parameters
set limits, so version skew costs features and not connectivity — the alternative
would make ordinary skew a connectivity problem for the party least able to route
around it. **Greasing is mandatory**: every implementation sends parameters it
knows to be unassigned and must tolerate receiving them, with ids of the form
`31*N + 27` permanently reserved. An extension mechanism nobody exercises decays
until it no longer works anywhere, so this keeps the ignore-unknown path
exercised in production rather than only in tests. §22.1's blocker list is down
to one: the resource interaction protocol.

### 2026-08-16 (node-resource trust boundary)

§10 settles a constraint the
interaction protocol must respect: **credentials are audience-bound and never
forwarded.** A credential names one resource and any other must reject it; a
resource never passes a user's credential to another resource. Resource-to-
resource trust is **configured rather than delegated**, carrying the requesting
resource's own authority. This removes the confused-deputy problem by
construction rather than guarding against it — a resource cannot be induced to
wield a user's authority elsewhere because it never holds any. **Accepted cost:**
a user cannot authorise one resource to act on their behalf at another, so such
composition is operator-configured and happens at organisational rather than
individual scale. The alternative is delegation with guards, which is what OAuth
is, and most of its complexity and vulnerability history is the cost of those
guards.

### 2026-08-16 (revocation scope)

§10.2's claim that departure revokes
everything uniformly is scoped: **membership gates session establishment;
continuation is the resource's business.** A hosted package terminates at the
node so revocation is immediate; an external service continues on its own terms,
**which is what choosing that service means.** The network provides the bridge
and administers who may cross it; it does not undertake to improve the products
a team decides to use. A manifest field declaring revocation support was
considered and rejected — unverifiable, and it framed a third-party property as a
shortfall here. The infra client shows which kind a resource is, since that
follows from where it runs.

### 2026-08-16 (session termination on role change)

The reference infra client
**drops hosted sessions when a principal's roles change**, rather than notifying
the resource. Local behaviour, not a protocol rule — the network cannot reach into
an operator's node — but it is what makes §10.2's membership gate mean what it
says for the resources where anything here can. **Termination is simpler than
notification and that is why it is correct**: notifying would need a protocol, an
acknowledgement, a latency budget and a rule for requests already in flight,
whereas dropping the session means **role changes are enforced by reconnection** —
no mid-session mutation, no partial-privilege state, no request that starts under
one role and finishes under another. This closes two of the interaction protocol's
open questions: no notification mechanism is owed, and the node need not reach
into a live session. What remains is the credential's contents.

### 2026-08-16 (pairwise principal identifiers)

A resource sees
`SHA-256("rhtn/1:pairwise" || resource_keyhash || user_keyhash)`, never the
user's keyhash. **Per-user functionality is unaffected** — the identifier is
deterministic and stable, which is what ownership, history and preferences need,
and most applications distinguish users holding identical roles. **Two vendors
cannot compare notes**, since different resources derive different identifiers
for the same person; handing out the keyhash would let any two services discover
they share a user, which is P3's cross-context linkage arriving through a door
the network opened rather than one it failed to close. **A hosted operator loses
nothing**: knowing the resource identity and their own org's keyhashes, they can
invert the mapping at one hash per member — the scheme withholds network identity
from parties who do not already know you (registered as **P25**). **Keyed on the
resource**, so a package moving between local hosting and a broker keeps its
identifiers; keying on the hosting model would have made every user look new the
day a resource moved. Per resource rather than per owner, since the coarser grain
cannot be recovered from the finer. This settles the credential contents
(`resource-requirements.md` §2); **C1, the request shape, is what remains of
the interaction protocol.**

### 2026-08-16 (resources assert nothing into the trust graph)

§11.0.3 closes
the last question with design content in it. **A resource reports to its host and
may message nodes peer-to-peer where configured; it emits nothing the trust
metric consumes.** The reason is that the alternative is unbounded — letting
resources assert would require **global message types for arbitrary service
needs**, so a fixed protocol would have to absorb an open-ended ecosystem's
vocabulary. It is also §10.3's line applied: a resource emitting trust signals
would be *borrowing authority in order to create trust*, the combination excluded
everywhere else. **Two consequences.** Resource-initiated contact needs nothing
new — a resource is a payload endpoint (§12.2), addressable via `CatalogEntry`.
And the request path carries no trust, which makes the interaction protocol a
**framing choice rather than a design question**: HTTP/3 over the existing QUIC
session, credential in request headers, is the expected answer and matches the
reverse-proxy pattern §10 already uses as its analogy.

### 2026-08-16 (interaction protocol closed)

The last blocker. **HTTP/3 over the
existing QUIC session**, credential in request headers
(`resource-requirements.md` §3) — no new transport, no new framing, and no
signing or canonical encoding, because the request path carries no trust
(§11.0.3). The **header-spoofing hazard** is stated on both sides, being the way
this pattern is most often misimplemented: the node must strip inbound `rhtn-*`
headers before inserting its own, and a resource must not trust them on any path
but the gateway's, since being behind a gateway is a deployment fact rather than
something a request can prove. **Owner movement settled** (§10.2): the old
upline loses access, the down-line is unaffected, and **the new upline gains
access silently** — nobody acts, the predicate simply matches different people,
so the client warns before such a move. The larger effect is hosting: a
light-client owner's resource runs on its serving infra node, so a move that
changes that node forces migration — an argument for resource owners to run
infrastructure. **§22.1's blocker list is now empty.**

### 2026-08-16 (provisionality of unset parameters)

New §21.1.1. *"Take a
provisional value and tune later"* is true of most of the eighteen and false of
six. **A parameter hardens when someone other than its chooser depends on it** —
an attacker choosing where to attack, a peer parsing a number, a consumer reading
a published figure. Local performance knobs stay soft indefinitely. The security
ones are the sharpest case: **heterogeneity does not average out, because an
attacker targets whichever subnet uses the weakest value**, so no individual node
can protect itself by choosing well. Capability parameter ids are noted as a
**registry rather than a value** — colliding assignments are unrecoverable, and
§6.1's greasing protects against unknown ids, not against two implementations
meaning different things by the same one. Also softened §10.2's owner-movement
note: accepting a patron is deliberate, and an upline gains access only where the
operator wrote a predicate reaching upward, so the client reminds rather than
warns.

### 2026-08-16 (capability ids derived, not assigned)

The registry problem is
removed rather than centralised. `capability_id = first 8 bytes of
SHA-256("rhtn/cap:" || name)`, with `name` namespaced by whoever mints it —
`rhtn/core:` for specification-defined ids, a domain for anyone else. **The name
is the registration**, so no authority assigns anything and no document is
consulted before shipping an experiment; collision needs two *different names*
hashing to the same 64 bits. It also **simplifies greasing**: the reserved
`31*N + 27` range existed because densely assigned small integers make a random
value likely to hit a real one, and in a sparse hash space **a random id is an
unassigned id**, so the reservation is dropped. Accepted cost: an unknown id is
opaque forever, where a registry would have allowed lookup — which matters only
for diagnosis, since unknown parameters are ignored regardless.

### 2026-08-16 (post-refactor sweep)

Full propagation and reference check
across all six documents. Fixed: light-client requirements had **no archive
obligations** — presenting a head on adoption, serving requests for one's own
archive, verifying a fetched chain oneself, and not reading a short reply as a
short archive; §6.2.5's heading still said "unspecified" though the interim rule
is stated; a wire disavowal field cited §6.2.1 when it meant §6.2.2; five wire
items were marked **[P]** though settled elsewhere, leaving two genuinely
proposed; §21 item 12 still listed the resource interaction protocol as open;
item 13 now defers with item 10 rather than standing alone; §8.1.1's rationale
was the last passage still written as history and is now present-tense. The
resource-interaction extraction document is **closed** with a table of where each
answer landed. All references in all six documents resolve.

### 2026-08-16 (dates reconstructed from the transcript)

Every date in every
document corrected against per-turn timestamps in the conversation transcript.
**The project began 2026-08-12T03:36Z and is five days old.** The previous
stamping put three days of work on 12 August and a further day on **15 August, a
date on which nothing happened** — an artefact of carrying the first entry's date
forward rather than checking. Corrected: topology through addressing to **12
Aug**, currency through wire format to **13 Aug**, review passes 0.1–0.5 to **14
Aug**, everything since the 16 August compaction to **17 Aug**, with this
session's entries not separately dated because it spans 16–17 August. Also
corrected a claim in Appendix A that §3.1.1 held two unextracted vignettes "for weeks",
which was **two days**. Body references to the first review passes now say 14
August rather than 12. Ordering was correct throughout; only the dates were not.

### 2026-08-16 (structure)

Format and organisation pass over the five
deliverable documents. **§6 held 1,949 lines — a third of this document — across
three unrelated subsystems**, and is split: §6 keeps transaction mechanics, §7
becomes **Proof of presence and recovery**, §10 becomes **Resources** (which was
never a transaction type), and everything after shifts by two. **`wire-format.md`
was out of sequence**: §3.6 sat after §5.5, and §5.6–5.8 after that, from
appending rather than inserting; now ordered. **`resource-requirements.md` is
confirmed as part of the deliverable set** — §4 and §2 are the only specification
of the credential and the request framing anywhere — and converted from
working-file form: the R-numbering, the propagation table and the
"what this implies" section are gone, replaced by plain sections and the same
no-protocol-facts rule the other requirements documents carry. Also: §19.8 sat
before §19.7; four headings still carried stale status markers
(*REQUIREMENTS SKETCH*, *DATA PLANE UNDESIGNED*, *NEW FINDING*, *Still deferred*).
All references in all six documents re-verified after the renumber.

### 2026-08-16 (0.1 factual verification, all five documents)

32 externally
checkable claims audited. **Two contradicted.** *"HTTP/3 rides QUIC without adding
a layer"* is false — it adds no new **transport connection** but does add its own
framing, control streams and SETTINGS (RFC 9114). And *"hole punching will fail
for symmetric NAT, CGNAT and two mobile peers"* overstates: address- and
port-dependent mapping defeats traversal and those conditions raise the odds, but
**neither CGNAT nor mobility guarantees failure** — success depends on mapping and
filtering behaviour, IPv6 availability and the candidate pairs ICE can form
(RFC 8445). **Citation error:** RFC 6763 specifies DNS-SD; mDNS is RFC 6762, and
"Bonjour" is the pair. **WebAuthn** binds to a relying-party ID with the origin
separately checked, not to an origin — so a gateway preserving the vendor origin
need not break it, while one substituting its own does. **The SaaS adoption claim
is narrowed**: standard federation support turns an integration project into a
configuration task, but vendors differ in claim mappings, provisioning and policy
hooks, so one adaptor does not work unchanged everywhere. Also scoped: Firefox's
OCSP behaviour stated without implying it is current practice (CRLite since
Firefox 142); iOS cell-ID and BSSID limits scoped to **documented public APIs**;
UWB accuracy stated as a measured implementation result rather than a standard's
guarantee; geohash dimensions marked equatorial maxima; and the Grover claim
restated as a query bound rather than "128-bit security".

### 2026-08-16 (0.2 coherence, all five documents)

18 contradictions, four
blocking, almost all stale summaries left by the day's rapid edits. **The four
blocking were in the wire format**: a §4.2 algorithm row still said routine
transactions are classical-only, contradicting §5.1's rule that every archived
transaction needs both components; the envelope said *one* `COSE_Signature` per
signer where §2.2 requires two; the signer bound counted **logical signers** where
§1 presented it as the array's own maximum, so a presence record's real ceiling is
68 entries rather than 34; and subject consent was specified as signing both the
full `VerificationQuery` and the `query_id`. **Also fixed:** `duration_s` survived
in §8.1's illustrative schema after the wire format removed it; §21 listed the
disavowal enumeration, version-mismatch behaviour and CBOR-versus-protobuf as
open when all three are settled, and pointed at a nonexistent `wire-format.md`
§12; §22.1 listed test vectors as blocking two lines above "none remain";
parameter counts disagreed (17 versus 23); §14 said four message classes over a
five-row table; and **five presence references had become §11.2 in the section
split** — pattern-indistinguishable from the legitimate anchor references, so the
earlier repair missed them. In the requirements documents: resource conformance
still claimed departure revokes everything uniformly after the design scoped it to
what the node controls; the resource definition contradicted its own external-SaaS
category; and the infra document listed the interaction protocol as open.

### 2026-08-16 (0.3 unjustified claims, all five documents)

106 findings across
three severity bands. **LB1–21 map onto the existing A1–A21 register**, and
LB27–43 are §20 parameters the document already labels chosen rather than
derived — so the actionable material was elsewhere. **Five new load-bearing
assumptions registered as A22–A26**: the transaction-rate ceiling under the
capacity argument, that an unattested adoption is *near-worthless* rather than
merely weaker (which is what makes proof of presence optional rather than a gap),
a few hundred archive records per decade, that ~400 KB is prohibitive (which is
what keeps embedded evidence classical), and that ~99% discrimination is the right
privacy/utility point for a fuzzed profile. **Five comparative claims added to
§17.1** — including §1.2's benchmark that physical-world profiling is expensive
per target, which the entire privacy target is set against and which no study
here supports. **§20 now states plainly that every parameter is a chosen
operating point**, and `wire-format.md` §1 that its maxima are conservative DoS
ceilings rather than capacity results — exceeding one means *malformed*, not
*overloaded*. **The SaaS adoption claim was narrowed in `resource-requirements.md`
during 0.1 and left unfixed in design §1.3** — the same claim in two places,
one corrected. Twelve intensifiers replaced with the mechanism or the figure.

### 2026-08-16 (0.4 parameter inventory, all five documents)

Nine findings.
**Two rested on an arithmetic misreading worth recording**: a hybrid signer costs
Ed25519 **plus** ML-DSA — 3,373 bytes — not twice the post-quantum figure, since
only one of its two entries is post-quantum. So ~35 KB for a ten-signer presence
record and ~8 KB for an adoption are both correct. Both documents now show the
arithmetic so the figures are checkable rather than assertable. **Three figures
were genuinely wrong.** The maximum presence record is **~112 KB**, not ~95 KB.
The hybrid-embedded-evidence counterfactual is **~211 KB**, not "past 400 KB" —
which matters because that number is the entire justification for keeping embedded
evidence classical, and is registered as **A25**. And the prekey bundle bound of
64 KB was a ~40× ceiling over a ~1.5 KB object; now 4 KB. **The envelope bound
derivation did not carry the doubling** — it is now stated as two steps, sum the
per-role bounds then double for entries. Counts corrected: **twenty-six**
load-bearing assumptions, **twenty-one** unset parameters. And the Dunbar Org's
"~121 nodes" is replaced with its derivation — 111 at or below at f = 10, more
once siblings and cousins are counted.

### 2026-08-16 (0.5.1 rule fragility, all five documents)

Thirteen rules
restated in terms of the property they protect rather than the identifiers they
name. The substantive ones: **the hybrid-signing tier** now turns on *authenticity
surviving as long as an object can be presented as evidence*, with "every
transaction" as the consequence rather than the rule; **proximity evidence** is
described as bounding an attacker's cost rather than as a claim about which
channels are relay-resistant, since resistance is a property of a deployed
implementation and the channel list will change; **the membership gate** is stated
as *the region the owner's policy can evaluate*, which is what the Dunbar Org
currently is; **pairwise identifiers** as a requirement that identifiers for one
subject at different parties be uncorrelatable; **finalization** as counting
structurally valid responses without inspecting content, so a new response type
needs no rule change; **prekey prefetch** as *a fetch revealing intent must be made
independent of intent, and consumable material never prefetched*; **session
termination** as triggered by any authorisation-state change rather than an
enumerated list of causes; and **§21.1.1's hardening rule** as a property with the
table as instances. Two generalise beyond their sections: *absence of data from a
holder is never evidence about that data's existence*, and *a component behind an
authenticating intermediary must not treat intermediary-asserted metadata as
authentic unless it can verify the path*.

### 2026-08-16 (0.5.2 unenforceable mandates, all five documents)

Eight
findings, and the four high-severity ones are a single finding: **the requirements
documents stated unverifiable obligations as MUSTs without saying they were
unverifiable.** Whether an implementation discards what it was told to discard,
warns before an irreversible action, or keeps a promise about local storage is
invisible to every other party — §1.1's diagnostic returns *no enforcement
available* almost everywhere in those documents. Fixed once per document rather
than at twenty statements: each now opens by saying these are **commitments, not
enforceable rules**, that a conforming label means the author asserts them rather
than that anyone checked, and that where a requirement does leave a visible
artifact it is noted in place. They are stated as requirements anyway because
omitting them would leave an implementer to reinvent each decision, usually worse.
Also: §20.1's *"cannot be tuned for convenience"* restated as its consequence —
an attacker selects which deployment to attack, so the cost of a convenient value
is borne by everyone rather than the chooser; the capture-time disclosure
obligation now says plainly that nobody can verify it and the people harmed are
those never told; and **greasing's two halves are separated**, since whether a
peer *sends* greased parameters is invisible while whether it **tolerates** them
is testable by anyone who greases — so the sending obligation is what enforces the
receiving one.

### 2026-08-16 (0.6 implementation attempt, adoption)

**Two contradictions,
both introduced by today's own edits.** `Recovery[3]` was declared `COSE_Sign` in
the grammar — correctly, since the old identity is hybrid — and called a
`COSE_Sign1` in prose a few lines below. And the `keyhash` primitive still said
SHA-256 of "the COSE_Key", singular, after `KeyMaterial` became a fixed-order
pair; the reviewer had to consult the design to resolve it. Both would have broken
interoperability outright. **One gap is identity-critical and now flagged as
open**: the exact `COSE_Key` encoding of the ML-DSA-65 component is unpinned, and
because the keyhash is taken over the encoded `KeyMaterial`, **two
representations of the same key are two different identities** — this must be
fixed against the COSE registration before a second implementation exists rather
than inherited from whichever library is convenient. Also settled: `kid` goes in
the **protected** header, since RFC 9052 offers both buckets and a verifier
accepting only one rejects valid signatures; `template_version` is **required**
when the basis involves a photograph rather than merely permitted; **node and
patron must differ**, which was implied by the signer-set rule but never stated
as the presence-record equivalent is; **duplicate map keys must be rejected
before the map is materialised**, since a decoder that parses into a map first has
already collapsed them; and **verification requires key state** — an adoption may
carry only keyhashes, so an API promising to verify from bytes alone cannot be
built.

### 2026-08-16 (0.6 implementation attempt, resolve a locator)

One finding
materially weakened a rule added the day before. **Anchor entries are self-signed,
but an entry carries the anchor's *keyhash*, not its key** — so nothing in the
entry lets a recipient check the signature on receipt, and the key only arrives
at contact time. The signature therefore gives **retroactive attribution, not
prior authentication**: a node that reaches the address and obtains the key can
confirm the entry was genuine and identify which gossip source forged it if not.
That makes address injection *attributable* rather than *prevented*, which is
weaker than the earlier claim. The consequence now stated: **an implementation
must be explicit about its ingestion boundary**, since one that treats "present
in the table" as "verified" while its ingestion path does not enforce it has a
partition vulnerability with no visible symptom. Also: endpoint selection and
retry remain local policy, but with a floor — **the list must be treated as
alternatives**, or a single unreachable first entry becomes a permanent outage.
And new §7.7.1 resolves an ambiguity between the design narrative and the wire
operation: **a light client sends its resolution request to its serving node**,
not to the anchor; the request is anchor-*relative* in how its path is read, which
is not a statement about who it is addressed to.

### 2026-08-16 (0.6 implementation attempt, validate a presence record)

**A
hard conflict, and checking it exposed a second error in the same arithmetic.**
The bound of 16 verifier responses per record is unsatisfiable: each subject's
threshold caps at 10, and two well-connected participants require 20. Raised to
32 — two subjects times the per-subject cap, with headroom. Checking that
revealed the envelope signer bound had been **counting verifiers as envelope
signers**, when their signatures are embedded evidence inside the body under
§5.1's rule. Corrected to 2 participants plus 16 witnesses — **18 logical signers,
36 entries** — which also brings the maximum record size down from ~112 KB to
~65 KB. **Six definitions the selection rule depended on and never carried**:
`window_ordinal` derives from `started_at` rather than `finalized_at`, or a
participant could steer the seed by choosing when to finalise; "two years" is 730
days rather than a calendar interval; the boundary is inclusive-near and
exclusive-far; the candidate horizon is that same window, since drawing *n* and
the candidate set from different intervals would let the threshold exceed the
pool; **the current counterparty is never a candidate for their own
verification**; and formation records count both toward *n* and as candidates,
since they record real meetings and their lack of corroboration is a weight
question rather than a structural one. Also: a normal record **must carry at
least one witness**, zero being the formation case and nothing else; and a
response occupies a slot only if its `query_id` was countersigned, its subject is
a participant, and its verifier was selected — otherwise a participant could pad
the record with responses nobody asked for.

### 2026-08-16 (0.6 implementation attempt, client attach)

Fifteen findings,
four blocking. **A stale sentence in design §14.1.2** still said `Attach` names the
intended serving node after the field was removed — an implementation written
from the design alone would have produced incompatible CBOR. **The most
consequential finding concerns transport authentication**: RFC 7250 carries one
SubjectPublicKeyInfo, while an identity here is a *pair*. Resolved by §5.1's own
rule — **a peer presents its classical component**, and the post-quantum
component authenticates nothing at the transport layer because a session's
authenticity expires with the session. Confidentiality remains post-quantum via
the key exchange group. **Authentication is now explicitly mutual**, and a serving
node MUST verify `Attach` field 1 against the connection-authenticated identity —
`Attach` is unsigned, so without that check any party could claim any keyhash and
collect another node's queued messages. **Stream 0 had no framing at all**: no
type tag, no length prefix, no discriminator, so two implementations could not
parse each other. Now length-delimited type-tagged frames with unknown types
skipped rather than rejected. **Capability id byte order was unstated** — now
big-endian, since little-endian yields a different id for every named capability.
Also settled: a zero heartbeat interval is malformed; the counter ends the session
rather than wrapping; a miss is one full interval without a `Heartbeat`, with
other traffic not counting and the timestamp advisory; and every `AttachAck`
replaces the cached sibling list, including one from a failover sibling.

### 2026-08-16 (post-review consistency scan)

Swept all five documents for
values superseded during the day's review passes. Six stragglers: the 400 KB
hybrid-evidence figure survived in `wire-format.md` after correction to ~211 KB
in the design; "~121 nodes" survived in two places after the Dunbar figure was
replaced by its derivation; and "two years" survived where 730 days was adopted.
Three **malformed section ranges** left by the §6 split — `§16.2–10.3`,
`§7–6.7` — where the first half of a range remapped and the second did not,
which the reference checker cannot see because each half resolves on its own.
All references in all five documents resolve; no superseded value remains in
any body text.

### 2026-08-16 (0.7 LINDDUN, all five documents)

Four new correlations and
three findings. **C15 narrows a guarantee written today**: pairwise identifiers
stop two *independent* vendors comparing IDs, and do nothing to stop **one vendor
correlating its own several resources** using account and network data it holds
anyway. §11.0.2's claim now says so — the scheme addresses cross-*operator*
linkage, not cross-*service* linkage within one operator. **C17 is the cheapest
serious fix**: a retained source photograph carrying EXIF or recognisable
background defeats the coarse-geohash design outright, bypassing rather than
weakening the argument §7.6 makes about disclosure precision — the light client
must now strip metadata rather than rely on camera-pipeline defaults, which vary
and are not privacy-motivated. **C16**: the abuse-report category enum is
deliberately small so an allegation carries no particulars, and the opaque
companion `detail` field was unbounded; now capped at 1 KB with a note that a
resource needing more should reference its own record. **C14**: greasing
randomises unknown capability parameters, but the known set remains a device
fingerprint across an identity fork — so "start fresh" is weaker against the one
operator best placed to use it. **P26**: a resolution request discloses intent to
reach someone before any contact, the same shape as C11's prekey fetch, and no
uniform-prefetch defence has been considered for it.

### 2026-08-16 (0.8 adversarial, six roles)

Run in one pass at high effort.
**The design is much stronger against cryptographic fabrication than against
control of endpoints, local infrastructure and human participation** — its worst
cases are where the attacker legitimately possesses the component the
architecture deliberately trusts. Three findings changed text. **§16.4's
pluggability tension is an attack surface, not a coordination inconvenience**: a
funded operator's best move is not building a giant fake region, which the
reference metric renders nearly worthless, but **shopping for evaluators whose
policy does not respect the flow bound** — the attacker chooses which policy to
face while no evaluator chooses which attacker they face. That reframes the
policy descriptor as a basis for **refusing to rely** on unsound evaluations
rather than merely understanding them. **The archive-as-second-factor property is
defeated by the premise it seemed to address**: it narrows *key-only* compromise,
and an adversary holding the device holds keys, archive and photo store together
— nothing separates the factors, since they live on one device by construction.
**Provider independence is adversarial, not only operational**: legal compulsion
over one provider reaches every node hosted there at once, so concentration bears
on metadata confidentiality and local-state integrity, where §6.3 had motivated
diversity as fault tolerance. New **P29**: a ceremony counterparty retains the
victim's raw capture indefinitely with no detection mechanism, making the
ceremony a collection event as much as an evidence event. **Confirmed as holding
under attack**: end-to-end encryption against relay patrons and providers,
subject-recomputable verifier selection against a malicious counterparty,
cross-nomination's reduction of a lone witness's power, and the flow bound against
amplification inside a fake region.

### 2026-08-16 (policy descriptor: bad news only)

A node's account of its own
trust policy is unverifiable, and the asymmetry that follows decides the whole
mechanism: **a claim that your policy is sound is worthless, a claim that it is
not is credible.** Nobody falsely declares a weak metric. So the descriptor now
carries **negative declarations only** — a node may declare its policy fails the
soundness condition, silence means nothing, and no positive claim about metric
family, parameters or resistance bound is defined. **Dropping it entirely was the
alternative** and is arguably cleaner: per-observer trust means no party ever
consumes another's trust *computation*, so positive claims would be machinery for
a use case this architecture does not have, and would invite the mental model the
design rejects — that trust is a global quantity somebody can certify. It is kept
because **a silent absence invites reinvention**: a future maintainer without this
reasoning would find nothing and reasonably propose a full descriptor, whereas a
mechanism deliberately shaped to carry only bad news documents its own reasoning
by existing. Also removed a mangled fragment left in §15.2 by an earlier edit.

### 2026-08-16 (0.1 factual verification, all five documents, second run)

**Three contradictions,
all one error, and it was a conflation rather than a fact.** HTTP/3 is selected by
ALPN token `h3` at QUIC connection establishment (RFC 9114), while the client's
session negotiates `rhtn/1` — so standard HTTP/3 cannot be layered onto it, and
"no new transport, no new framing" was false twice over. The fix separates two
legs I had merged: **client-to-node travels as an rhtn control frame** on the
existing session (`wire-format.md` §8.0, frame types 5 and 6), and
**node-to-resource is ordinary HTTP over whatever suits** — a local socket for a
hosted package, HTTPS for an external service. Nothing required those to be the
same transport, and requiring it was the whole error; a proxy speaking different
protocols on its two sides is what a proxy is. **Six claims scoped**: span of
control and Dunbar's number now read as quoted heuristics rather than established
constants; **a small RTT constrains the endpoint answering the measurement, not a
person** — a nearby proxy satisfies it equally, which matters because the ceremony
treats latency as evidence about participants; QUIC migration is contrasted with
*ordinary single-path* TCP; vendor-push metadata is unavoidable *where push is the
wake mechanism* rather than for background delivery generally; and the SaaS
federation claim allows that it holds for vendors supporting it well rather than
universally.

### 2026-08-16 (0.2 coherence, all five documents, second run)

Six contradictions, **both blocking
ones introduced within the previous two hours**. The HTTP/3 correction reached
`resource-requirements.md` §3 and left §2.2 asserting the superseded claim four
paragraphs later — the same fix-one-instance failure as the SaaS claim earlier
today. And design §8.1's illustrative schema said subject consent signs *the
query* where the wire format signs the **`query_id`**; implementations following
each would build different `Sig_structure` payloads and fail to verify one
another. **Serious**: §8.2's size arithmetic still counted *~3 verifiers* among a
presence record's signers, contradicting §5.1's rule that embedded evidence signs
classically and the wire format's exclusion of verifiers from the envelope signer
set — corrected to 2 participants and ~8 witnesses, with the per-signer cost
restated as 3,373 B rather than 3,309. **Minor**: the wire format said seven
signing roles above a table of eleven; two passages described design §8.1.2 as
requiring recomputation "by any third party" when that section expressly rejects
the phrase; and the infra document used "prekey fetch" both for the
intent-independent reusable prefetch and for the on-demand one-time request that
does carry intent.

### 2026-08-16 (0.3 unjustified claims, all five documents, second run)

84 claims, 58 load-bearing
against 26 registered. **Most of the gap is §20's parameters and the wire
format's array bounds**, which both documents already declare chosen and
conservative rather than derived — but the reviewer's methodology is right that
**labelling a value "chosen" is a disclosure, not a justification**. §17 now says
plainly that the register is **curated rather than exhaustive**: it lists
assumptions whose failure would change a *design decision* rather than a tuning
value, which is the useful cut for deciding what to test first, and is not a claim
that the remainder are supported. The strict count is recorded there. **Two new
assumptions**: **A27**, that a normal presence record carries ~10 logical signers
with ~8 witnesses — a social artifact underpinning the ~35 KB figure and the
storage arithmetic; and **A28**, that package authors' incentive runs toward
breadth in permission defaults, which is the entire motivation for requiring
templates be inspectable, and is an economic claim asserted rather than argued.
Three supporting claims added to §17.1 (carrier gateway aggregation, radio access
latency, and the comparative size of the extension attack surface), and seven
intensifiers replaced with the property they were standing in for.

### 2026-08-16 (0.4 parameter inventory, all five documents, second run)

**No unresolved value
contradiction across the five documents** — the first parameter pass to come back
clean. Five apparent mismatches were checked and are not: "two years" versus
"730 days" is a definition the wire format states; ~35 KB typical versus ~65 KB
maximum name different operating cases; a 10-per-subject threshold under a
32-response array bound reflects two subjects plus headroom; a heartbeat interval
that is UNSET while its floor is 1 second separates a value from its validity
range; and three superseded bounds appear only where the change log records them
as superseded. **One item fixed**: §3.4 said at least two cross-tree peers are
**required** for genuine redundancy while §20 records the same figure as a
**recommendation**. Same number, different normative force — and the design
intent is the weaker one, since peering is voluntary and zero peers is supported.
§3.4 now states it as what redundancy costs rather than as a condition of
participation.

### 2026-08-16 (0.5.1 rule fragility, all five documents, second run)

Eleven residual rules
restated in terms of the property they protect. The reviewer's exclusions are as
informative as its findings: it deliberately passed over veto delegation,
formation typing, verifier-selection fields, audience-bound credentials and scope
evaluability, on the grounds that **the defect is not that identifiers appear in
an encoding rule but that the property would disappear with the identifier** —
which is the right test and the one Appendix A states. Substantive rewrites: patron
non-countersignature becomes *a party with authority must not authenticate or gate
evidence of an event it did not observe and that occurred outside its authority*;
the departure warning becomes **any user-initiated change that ends or alters an
authority relationship**, departure being the common case rather than the only
one; header stripping becomes *an authenticating intermediary must remove every
caller-controlled value in the namespace it uses for trusted assertions*; the
`Attach` identity check becomes *bind the identity for which queued data is
requested to the identity the transport authenticated*; and liveness becomes
*only an explicit liveness signal resets the failure detector*, since incidental
traffic would otherwise conceal failure of the path the detector exists to test.
The reviewer also cautioned against reintroducing an intermediary-retention clause
into the abuse-report rule, which 0.5.2 removed as crossing the enforcement
boundary — a cross-pass consistency check neither pass could have made alone.

### 2026-08-16 (0.5.2 unenforceable mandates, all five documents, second run)

Three findings, all
upheld. **Two asked implementations to make a fetch "independent of intent" or to
avoid "speculative" consumption** — properties no observer can check, since the
fact of a fetch is shared and the motive is not. Replaced with visible structure:
a **`PrekeyBatchRequest` naming the population it sweeps** is distinguishable on
the wire from a targeted single-subject request, so a serving node sees which it
received rather than taking the client's word. **The reviewer's proposed fix for
the second — binding one-time key consumption to session-opening evidence — is
circular under PQXDH**, since the key is needed before the session exists; the
node instead **rate-limits issuance per requester per subject**, bounding the harm
the rule protected against without anyone proving motive. **Third: greasing's
MUST-send is withdrawn.** The document diagnosed its own failure — whether a peer
sends greased parameters is invisible — and then issued the requirement anyway.
**Tolerance remains a MUST and is checkable by anyone who greases; sending is what
the reference implementation does.** The honest mechanism is self-interest: an
implementation that never greases relies on others to keep the tolerance path
exercised for it, which works while most do and fails quietly when they stop.

### 2026-08-16 (0.6 implementation attempt, adoption, second run)

**One direct
contradiction and one schema gap.** §5.1 listed **old-key proofs** among embedded
evidence that stays classical, while the wire grammar made `Recovery[3]` a
`COSE_Sign` because the old identity is hybrid. The wire format is right and the
design's list was wrong: an old-key proof is the prior identity **authorising a
change to itself**, which is the operation §5.1's own rule names, not evidence
supporting someone else's decision. **The schema gap is more consequential.** A
`VerifierResponse` carries no field naming the prior identity, so in a recovery
the verifier signs that some face matches some subject **without ever signing
which old identity that continues** — the assertion the whole recovery rests on
was unsigned. Field 8 added, required inside a `Recovery` block and covered by
the verifier's signature. **A recovery now also requires at least one `match`**:
responses are structurally valid whatever they report, which is right for
presence finalization where the question is whether the query was honestly
*conducted*, but a recovery **asserts a fact** and a set containing no match
asserts nothing. Also pinned: Ed25519's `COSE_Key` encoding and the rule that no
extra parameter may appear, since any addition changes the keyhash and therefore
the identity; protected headers carry exactly `alg` and `kid` and reject
extras, since tolerating unknowns in a signed header lets implementations
disagree about what was signed; **signature entries sort by `kid` then classical
before post-quantum**, because deterministic CBOR does not order arrays and two
encoders would otherwise produce different `txid`s; carried `KeyMaterial` must
hash to the keyhash it accompanies; and **"verify" means structurally valid, not
effective** — whether an adoption takes effect depends on topology a validator may
not hold.

### 2026-08-16 (0.6 implementation attempt, resolve a locator, second run)

**Four blockers, one of them a plain omission.** `ResolveRequest` carried subject,
path and nonce and **no anchor**, while defining the path as anchor-relative — the
request was uninterpretable as specified. Anchor field added. **Two more were
first-contact problems.** A `ServingInfra` returned a keyhash and endpoints, but
TLS presents only the classical component while the keyhash covers the pair, so a
requester with nothing pinned could not authenticate what it reached; the reply
now carries optional `KeyMaterial`. And **resolution is now explicitly
requester-driven** — a node answers, repairs, or reports non-authority, and never
forwards on the requester's behalf — which removes the need for consumed-prefix
state the request had no field for, and removes an intermediary's ability to
misreport progress invisibly. **Canonicalisation settled**: all signatures are
**detached**, since an embedded payload gives one logical object two byte
encodings that can disagree; a signature covers **every retained field including
unknown extensions**, because reading "signs fields 1–2" as a closed list would
defeat §1's extension-preservation rule; and a packed path must be exactly
`ceil(nibbles/2)` bytes, since surplus trailing bytes would give one path
unboundedly many encodings. Also: the four failure codes now carry explicit
retry/re-resolve/terminal dispositions, an anchor **table** entry may only name a
contactable infra node, and a resolution nonce is reused across endpoint retries
so a reply to one attempt cannot answer another.

### 2026-08-16 (0.6 implementation attempt, validate a presence record, second run)

Eleven gaps. **One is a self-contradiction introduced the previous day**:
the 730-day window was given as a formula admitting the boundary instant and a
sentence excluding it, which two implementations resolve differently at exactly
that point. Now open at the far end, closed at the near. **The deepest finding is
that the anti-grinding property had no mechanism behind it.** §5.2 claims an
honest retry within the window reproduces the same verifier sample while an
aborting attacker gets one fresh sample per day — both depend on a witness nonce
being *stable across attempts*, and nothing said how. A witness generating fresh
randomness per attempt hands a grinding participant a new sample per abort, which
is the attack commit-reveal was introduced to close. Nonces are now derived by PRF
over the participant pair and window ordinal, and **the property is stated as
resting on witness honesty, which cross-nomination supplies** — grinding requires
the counterparty's nominees to collude, the same bar as forging the ceremony.
**Also settled**: witness identities must be distinct and must exclude both
participants, since duplicates leave the seed's ordering undefined and a
participant is not an independent witness; *n* counts distinct transactions
reachable in the archive **DAG**, each once, which a chain-era implementation
would double-count after a merge; an off-selection verifier response makes the
record **malformed** rather than merely unweighted, since the gap between the
32-entry bound and 20 legitimate responses is room to pad; and `formation` is an
evidence label rather than a history-dependent precondition, because checking
eligibility would require every validator to hold both participants' histories.

### 2026-08-16 (0.6 implementation attempt, client attach, second run)

Thirteen
gaps, four blocking, and **two are omissions of exactly the kind the resolution
target had just exposed.** A `CurrencyAttestation` said "signed by the issuer" and
carried **no signature field and no issuer field** — each implementer would have
invented a carriage and none would interoperate. And a `SiblingRef` carried a
keyhash and endpoints but no `KeyMaterial`, so **a client could not authenticate a
sibling it had never contacted**: the handshake presents the classical component
while the keyhash commits to the pair, and trust-on-first-use cannot check a
keyhash it cannot reconstruct. Both fixed the same way `ServingInfra` was an hour
earlier — which is the tell that the pattern was structural rather than a one-off.
**The empty-container rule was over-broad**: "empty maps MUST be omitted" collides
with `Capabilities`, a required field a peer advertising nothing must still send,
and with a `SiblingUpdate` clearing the list, which would have had no encoding at
all. Now scoped to optional fields. **Also settled**: a malformed heartbeat counts
as absence rather than a fault, since closing immediately would let one corrupted
frame impersonate a dead peer; **failover applies to a primary already dark at
attach time**, not only one that dies mid-session, or the cached list is unusable
in the case it exists for; there is no `AttachNack`, so a failed dial is a timeout
or a close; and the sibling list should survive restart.

### 2026-08-16 (§1.2.2: emergent properties made explicit)

Three properties the
design relies on and never stated. **Cheap fabrication is a privacy feature**: a
subnet with no real members is easy to build, so any package of correlated
evidence is consistent with being invented, and **Sybil attackers contribute to
everyone's deniability**. The boundary is stated so this does not read as
contradicting §16.3 — forging evidence about a *specific real person* needs their
signature and stays hard; fabricating a *whole graph* is easy, so **the deniability
is in the graph rather than in any signature**, and evaporates for an evaluator
who can reach the counterparties independently. **The subject's confidence in
their own evidence is better founded than an attacker's from identical bytes** —
which is why trust is per-observer rather than a global score, since the same
evidence genuinely supports different conclusions for differently-placed parties.
**Baseline exposure is the floor**: each attack should cost at least as much as
its conventional equivalent against a non-user, which is what makes §16.5's
register sortable — "an attacker with the device sees everything" is a defect only
if they see more than they would from any other phone. **The limit is stated with
it**: the floor holds for *acquisition cost*, not for *evidentiary weight*. A
camera roll is ambiguous and decays; an archive is durable, portable and
non-repudiable, so along that vector this design is worse than the baseline, as
§1.2.1 already concedes.

### 2026-08-16 (0.7 LINDDUN, all five documents, third run)

Three new findings. **N2 is C9
reproduced on the infrastructure side**: topology, liveness, queue state, prekey
requests, role decisions and resource traffic are individually minimised and all
visible to a single installed extension. The sandboxing note treated this as code
safety; **host bindings are a privacy boundary**, and should be granted
per-package at the narrowest declared scope. Registered as **C18** and **P31**.
**N1**: the design tells the *operator* whether a resource is hosted or brokered,
and tells the **accessing user** nothing — so someone leaving an organisation may
reasonably believe their access ended everywhere while a vendor session continues.
A user-facing disclosure is now a light-client obligation. **N3**: veto delegation
cannot be privacy-assessed until its carriage and revocation are specified —
recorded as a specification dependency rather than a leak, so the gap is visible
when that object is finished.

### 2026-08-16 (§1.2.2: surveillance classes; 0.8 second run)

§1.2.2's
evidentiary-weight limit is **narrowed by a three-class taxonomy**, and the
narrowing is substantial. Attestation only helps an attacker who needs their
conclusion to survive scrutiny. **Bulk collectors do not verify signatures.** An
immigration officer or employment screener refuses over unsigned photographs
exactly as readily as over signed ones, so against that class — where most
realistic fear sits — **a signature adds nothing and encryption is the whole
defence either way.** Only the discriminate class can spend the difference, and
the fabricability discount partly offsets it even there: a signature proves a key
signed, not that a person exists, so a **remote** examiner cannot tell a
synthesised subnet from a real one, and the first two classes are remote almost by
definition. §16.5 now directs readers to sort findings by which class can use
them. **From the adversarial re-run**, two structural points: **plurality must
already exist to defeat an eclipse cleanly** — establishing a second patron while
eclipsed means reaching one, which is what the eclipsing patron mediates, so the
escape costs a physical meeting and the client should encourage plurality early;
and **the state-actor ranking rests on §3.3's unsupported cloud-concentration
expectation**, so if deployment is provider-diverse it drops below endpoint theft.

### 2026-08-16 (0.8b vignette agreement)

Four contradictions between vignettes
and the sections they illustrate, all in the same direction: **the vignette
claimed more than the mechanism delivers.** V2 showed a first contact resolved
from a business card, where §11.3 says the locator travels with the key out of
band and a bare key is not resolvable at all — rewritten as reaching someone
already in her contacts. V4 said a connection means *"we stood in a room together
for four minutes"*, which §7.1 expressly forbids the record from claiming, since
witnesses cannot attest that two humans shared a room; now *"somebody spent four
minutes proving they were where I was"*. V4 also priced the attack as universal —
*"an attacker with a warehouse of handsets and no warehouse of people"* — where
§7 states plainly that **bilateral collusion is unpreventable**; the cost falls
on edges to people who did not agree, not on meetings inside territory the
attacker controls. And V8 described institutions binding *"its own name for you"*,
the multi-identity property §13.7 records as **deferred**. Worth noting the
pattern: vignettes drift optimistic, because the version that reads well is the
version that claims the mechanism works better than it does.

### 2026-08-16 (vignette voice)

The 0.8b corrections were accurate and
over-corrected: hedging folded into the narration produced prose that is
technically careful and teaches less. **A vignette now states the base case
plainly and appends its limits in italics**, rather than qualifying itself
mid-scene. V4 says *"a connection here says we met"* again, with the two real
limits noted after — witnesses attest that the protocol ran rather than what they
saw, and collusion between willing parties is unpreventable, so the cost lands on
edges to people who did not agree. V2 keeps the introduction requirement as a
deliberate limit rather than burying it in the scene. V8 notes that v1's single
identity per client makes the postal analogy partly aspirational. **Appendix A records the
rule**: the reader who needs the caveat is not the reader a vignette is for, and
writing for the most security-anxious reader misinforms the ordinary one — most
people leak far more than this network asks and are untroubled by it.

### 2026-08-16 (0.9 organisation, part two)

Two structural moves. **The archive
is now §10**, a first-class section rather than three levels down under "Presence
record format" — it applies to every transaction type, says so, and "how does the
archive work" is a question nobody would have answered by looking under presence
records. Resources become §11 and everything after shifts by one. **Resolved,
dissolved and drafted entries moved out of §22 and §23 into Appendix A.3**, so
"open questions" now means currently open: thirteen entries became five, and
§23's three closed areas — succession, intra-subnet discovery, contained gaps —
moved with them. Nine cross-references pointed at the moved material and were
repaired. Also fixed, for the third time in this pass: **bare `§N` references in
the companion files**, which the remapping scripts skip because they lack the
`design §` prefix that identifies them as cross-document.

### 2026-08-16 (status blocks reconciled)

Swept all six headers. The design's
*Specified* line **listed the archive twice** — once as promoted in the 0.9 pass
and once from its old wording — and its *Not specified* line **omitted the one
item that actually blocks interoperation**: the unpinned ML-DSA `COSE_Key`
encoding, where two representations of one key are two identities. That is now
first in the list, ahead of payload encryption, which is adopted rather than
missing. The wire format carried **two competing companion statements** stacked
from separate edits. The light-client header cited "nineteen requirements
scattered across a 5,000-line specification" — both numbers stale. The resource
document's remit line said operator-side material lives elsewhere while the
document itself covers packaging and sandboxing; it now states the actual split,
**obligations there, the resource's side of the same boundaries here**. Two files
had `network-design.md` **design Appendix A** from the preamble dedup. Reading order
gained §10.

## 2026-08-22

### 2026-08-22 (preface; §1 reordered; style pass §§1–4)

Author's preface added
ahead of the status block. **§1 now opens with what the network is for** rather
than what it refuses: anchoring trust in human attention and social relations.
The no-global-state requirement follows from a **commitment rather than a
mechanism** — respect for individual autonomy and communal self-determination
requires that people be free to form their own views and assemble their own
networks of trust, and a system deciding centrally who is credible has taken that
away whatever else it provides. The metering choice reinforces it rather than
causing it. §1.1's heading drops "Corollary", since visibility-replaces-
enforcement now follows from the value commitment rather than from the
architectural one. The previous opening stated the mechanism first and
reached the purpose obliquely, which left the preface setting an expectation the
next section did not meet. **Process material moved to
`authoring-conventions.md`** — the intensifier rule, the vignette convention and
invariant-authoring guidance, plus eight references to review passes and reviewers
in the body. Appendix A keeps only what governs the design itself. **The user's own
explanations now frame §1.2.2**: the three surveillance classes, fabrication as a
source of deniability, trust emanating from the user, and baseline exposure as the
security floor, all in the original framing rather than paraphrased. **Style pass
over §§1–4**: em-dashes reduced from 60 to 11 in those sections, keeping only
those marking a genuine aside.

### 2026-08-22 (keystream-encrypted captures)

New §7.5.2. **Each participant
streams keystream material to the other during a ceremony and encrypts its
captures under the keystream the *subject* supplied**, so A's images on B's device
are ciphertext A holds the key to. B regains access only when A hands over the
keystream again in a later ceremony, directly, bypassing the counterparty and
every witness. **Retention stops being a promise and becomes a consequence of
meeting cadence**: if A and B never meet again, B's copies are permanently
inaccessible without anyone deleting anything. **This is the only mitigation in
the set that protects the depicted person's data from the holder** rather than a
holder's data from third parties. **The claim is bounded and stated as such**: a
non-compliant client defeats it entirely and nothing detects the difference, so it
changes what a well-behaved holder is *capable of*, not what a hostile one is. Its
value over an ordinary retention promise is structural rather than intentional — a
compliant holder cannot decrypt afterwards even if they want to, because they
never held the seed. **Regulatory consequence**: a compliant client holds no
decryptable biometric data at rest, which lowers the Article 9 barrier to hosting
for the small institutions §1.2's argument depends on. **No collision with
verifier queries**, since §7.4 requires the subject's countersignature on every
query and the subject is therefore always present; the keystream travels in
parallel with the query. Side effect: **the subject learns which counterparties
were selected**, making P18's verifier-privacy asymmetry symmetric. **Seeds die
with the device**, consistent with rotation never carrying state forward from
beyond the local trust horizon — the scheme adds no dependency, it makes an
existing one visible. Propagated to P13, P29, C9, the §1.2.2 floor table, the
accepted-cost register, the ceremony summary, light-client obligations, and
`wire-format.md`, where seeds are recorded as **never appearing on the wire**.

### 2026-08-22 (keystream: legal position propagated)

§7.5.2 was propagated
to eleven sites and **missed §7.2, the biometric-custody warning**, which is
where an operator decides whether hosting is viable at all. Now carries the
changed position: a compliant client holds no decryptable biometric data at rest,
and **ciphertext a holder cannot open is arguably not data processed for the
purpose of uniquely identifying a natural person** — the qualifier Article 9 turns
on. Three cautions stated with it and given more space than the relief: the
position is unsettled and may turn on whether a key is *ever* obtainable rather
than held now; it protects the well-behaved operator only, since a non-compliant
client retains plaintext and its operator is in exactly the original position; and
the full analysis still applies to decryptable images, to unencrypted derived
templates, and to anyone who obtains a keystream and keeps it. **Also noted at
§7.5.1**: keystream encryption *weakens* the breach-surface argument for short
retention rather than strengthening it, since a store of unopenable ciphertext has
little breach surface to reduce.

### 2026-08-22 (retention becomes subject-enforced)

A consequence of §7.5.2
that reverses who holds the policy: **a subject enforces their own retention
horizon by declining to supply the keystream.** No detection, no cooperation,
nothing to audit — the holder's copy simply stays inert. §7.5.1's window was a
commitment the *holder* made about material it controlled; it is now a decision
the *subject* takes about material it holds the key to, so **two years becomes a
default rather than a rule** and the value of a common number is coordination
rather than constraint. Two consequences recorded rather than buried: withholding
is **indistinguishable from unavailability** to an evaluator, which is the right
treatment since the alternative is asking evaluators to infer motive from an
absence, but it means a refusal goes unrecorded; and **a subject who withholds
broadly degrades their own recoverability**, since each counterparty cut off is
one that can no longer answer `photo_match`, leaving the weaker
`personal_knowledge` basis. The client must surface that trade when the policy is
set. **The same reservation as everywhere in §7.5.2**: none of it binds a
non-compliant client, which is unaffected by a refusal. What changes is that a
*compliant* holder now has no way to defeat the subject's decision, where before
it had only an obligation not to. Also noted: this **strengthens** §7.5.1's
Schelling-point argument while weakening its breach-surface one.

### 2026-08-22 (template prefix; seeds; open questions; style)

**The derived
template is a fixed-length record at the head of the encrypted store**, images
after (§7.5.2). Because it sits at a known offset, **the subject controls which
retention tier a counterparty gets by choosing how much keystream to send**: full
length opens template and images, template length opens the template alone, none
opens nothing. §7.5.1's two-year and five-year windows stop being commitments a
holder makes and become **a graduated capability the subject grants**, degradable
remotely without the holder acting or knowing. Constraints recorded: the template
must be fixed-length or a partial keystream decrypts a partial record; **order is
normative**, since writing images first silently converts a template-only grant
into an image grant; and a decrypted template is still Article 9 data. **Seeds are
ordinary device state and belong in the backup** — losing a backup costs them
along with the keys, which is the same loss for the same reason as losing portable
standing. **The two remaining wire `[P]` items are now stated as questions** rather
than as gaps: for activity summaries, who may hold one and how they obtain it,
what window it covers, when it goes stale, and whether the counterparty count is
optional in principle; for veto delegation, how a delegation reaches the parties
who must honour it, how it is revoked with the same reach, what expiry means when
clocks are contested, and whether a delegate may re-delegate. **Style pass
completed across all sections**: em-dashes reduced from 680 to 332, applying the
patterns established in §§1–4 — colon after a bolded term, full stop between
independent clauses, comma before a subordinate one.

### 2026-08-22 (veto withdrawn; challenge window withdrawn)

Both fail §1.1's
test, and neither had been run through it. **A patron shares no state with a
subordinate's client**: nothing prevents that client forming, signing,
replicating or acting on a transaction, and nothing makes it wait. An adoption is
signed by node and patron, a peering by two infra nodes, so a third party
asserting a veto over either has no mechanism to exercise it — and a challenge
window that delays nothing the actor controls delays nothing at all. **Removed**:
the delegated-veto object (`wire-format.md` §7.5), its four open questions, its
expiry parameter, the down-line threshold veto, and the challenge window.
**N3's blocked privacy analysis is closed by withdrawal** rather than by being
answered. **What replaces it is what was always real**: a compliant client
notifies the patron of high-value actions; the patron refuses resource access,
which *is* enforceable because node and requester share the state the decision
turns on; and the patron disavows. **Rejection produces a fork, not a block** —
a refused sub-subordinate exists in both readings, which is §3.1.1's membership
plurality reached through a different door. §9.2's argument that one keypair
suffices is rewritten: not because anything can be blocked, but because a thief's
natural move destroys most of what they stole, and a second credential would add
a key to steal without adding a power anyone can exercise. **The invariant is
kept** in `wire-format.md` §7.5, now describing a power that does not exist rather
than one to be constrained, so that reintroducing any blocking mechanism has to
argue against it.

### 2026-08-22 (grandpatron subtree acknowledgement)

New §11.2.1 and
`wire-format.md` §7.5. **Adoption puts a node inside its grandpatron's Dunbar
Org automatically, so a patron could admit arbitrary strangers to their own
superior's resources without that superior agreeing.** A grandpatron now
countersigns before the new node reaches resources they host — enforceable for
the ordinary reason, since their node evaluates access and shares with the
requester the state the decision turns on. **It does not gate the adoption**,
which is complete and propagates on the node and patron signatures alone, so a
grandpatron who is offline delays a convenience rather than blocking a
membership. Carried as a separate `SubtreeAck` over the adoption's txid rather
than a third envelope signer, for that reason. **Nodes that already trust the
grandpatron may accept the one signature instead of evaluating a stranger
themselves** — the grandpatron's siblings and the great-grandpatron in practice —
and this is **evidence they choose to accept rather than an instruction**, since
nobody binds anyone's policy here. **It attests subtree membership, not
identity**: the adoption's own signatures carry that, and it **lapses without a
revocation object** when the relationship it describes ends, because the condition
it depends on is already visible in topology. The infra client must prompt its
operator rather than sign automatically, since auto-signing recreates the
situation the mechanism exists to correct.

### 2026-08-22 (subtree acknowledgement: default and enforceability)

Two
refinements. **The default on accepting a `SubtreeAck` is to allocate roles as to
any subordinate in that network position** — the new node is treated as its
position implies, nothing special and nothing withheld. **It reaches positional
grants only**: a resource whose roles are bound to named individuals is untouched,
since those were decisions about particular people and a node arriving in a
position was never one of them. So the default admits a new member to the team
datastore and not to whatever three people were explicitly listed for. And the
**do-not-auto-sign rule is named as unenforceable**, which it is: an auto-signed
acknowledgement is byte-identical to a considered one, so nothing detects the
difference and this is a commitment of the same kind as everything else in the
requirements documents.

### 2026-08-22 (open items consolidated)

Everything currently open is now at
**§23.2**, classified by what it blocks, so a reviewer need not reassemble it from
six registers. **One item blocks interoperation**: the unpinned ML-DSA
`COSE_Key` encoding, where two representations of one key are two identities.
Four block a subsystem rather than a first node. The rest are decided during
implementation or deferred by decision. Housekeeping alongside it: three entries
marked RESOLVED were still sitting in `wire-format.md`'s open list, which
overstated it at eight when four are live; the unset-parameter count was
**twenty-one against twenty rows** after the veto-expiry removal; and §22's item 5
still cross-referenced "item 10" from before that list was renumbered. §22 also
now says why two deferred-by-decision entries are kept there rather than in the
appendix — a standing choice that could be revisited reads differently from
settled history.

### 2026-08-22 (ML-DSA key encoding: not open after all)

The single item
recorded as blocking two implementations from interoperating **was never open**.
RFC 9964 standardised ML-DSA for JOSE and COSE in May 2026, defining an
**Algorithm Key Pair** key type that pins the representation completely: `kty` = 7,
`alg` REQUIRED, `pub` REQUIRED at label -1, and `priv` forbidden in a public key.
`wire-format.md` §2.2 now states those three parameters and the rule that no other
may appear, matching the classical component's treatment for the same reason —
**any additional entry changes the encoding and therefore the identity**. The flag
reflected a failure to check whether a specification existed rather than a gap in
the standards, and it survived several review passes because a reviewer reading
`[OPEN]` has no reason to question that something is open. **§23.2 now records
that nothing blocks interoperation.**

### 2026-08-22 (queue settled; activity summary withdrawn; gateway scoped)

**Queue policy decided**: messages queue **indefinitely at the direct patron**,
bounded by a per-subordinate storage cap. Retention and size collapse into one
policy — a space bound declines the trade a time window forces, so a message
survives an absence of any length and an over-accumulating subordinate hits a
ceiling rather than a clock. **Siblings hold no queue state**: failover covers
sessions, not mailboxes, and a client attached to a sibling still collects from
its own patron on return. That removes the metadata-spreading question entirely.
**Patron-signed activity summaries withdrawn.** A portable behavioural figure
asserted by one party and consumed by strangers **is a reputation signal**, which
§1 refuses and §16.1 replaces with per-observer evaluation — the reader cannot
verify the count and is trusting the patron transitively. The problem it addressed
does not need solving: an observer with no history has nothing to call anomalous,
and the correct response is to extend no credit rather than accept someone else's
number in place of the history they lack. Removed with it: the wire object, its
four questions, the window and staleness parameters, and **P7**. **Gateway
operator evaluation scoped rather than left open**: the user's protection is the
signed `CatalogEntry` binding the service to what the trusted owner published, and
beyond that they are where an employee is with their employer's SaaS vendors —
free to avoid a resource they do not trust. Asking the protocol for more would be
asking it to adjudicate a vendor relationship it is not party to.

### 2026-08-22 (§23.2 expanded, then refreshed)

The four subsystem blockers
were expanded with what is missing, what each blocks, and why it is hard —
and **three of the four were resolved in the same session**, leaving §23.2
describing a state two decisions out of date. Refreshed: the queue block now
records what was settled and lists only the residue (cap value and ceiling
behaviour, crash-recovery copies, operator logging); the activity-summary and
gateway blocks are gone; the count reads two rather than four. The **cycle
prevention** entry gains the reason the interim rule is adequate rather than
provisional — **an undetected cycle is a correctness problem in one subnet's
topology, while a false positive refuses a legitimate adoption and is
indistinguishable from censorship** — so the rule accepts the first to avoid the
second, and any general procedure must preserve that ordering.
`wire-format.md` §13's list renumbered after the activity-summary removal left it
starting at 2, and its queue entry rewritten to the residue.

### 2026-08-22 (decision audit)

Walked every decision made today against every
place it should appear. **Four defects, all in the direction of a change reaching
most sites and missing one.** §11.2.1 described the grandpatron countersignature
without ever naming the `SubtreeAck` that carries it, so a reader could not find
the object from the design. **N3 in the privacy register still recorded veto
delegation as blocked pending its audience** — the entry survived an edit that
reported success against a string that must have differed, which is the failure
mode of matching on remembered text rather than read text. The unset-parameter
count read twenty against nineteen rows after the activity-summary window and
staleness parameters were removed — **the third time today that count has gone
stale**, each time because removing a row does not touch the sentence that counts
them. And `wire-format.md` §13's list was renumbered. Confirmed clean: the
remaining mentions of `VetoDelegation`, `ActivitySummary` and P7 are inside
withdrawal notes naming what was removed, which is correct, and the "challenge
window" at §7.6.1 is a timing window in the proximity analysis, unrelated to the
withdrawn mechanism.

### 2026-08-22 (style regression; companion documents)

Two corrections to what
was reported. **The style pass had covered only `network-design.md`**; the four
companions were never touched, and carried 355 em-dashes between them. Now
reduced to 185. And the design had **risen from 332 back to 351** because the
day's new prose — keystream, subtree acknowledgement, queue policy — reintroduced
the habit the pass existed to remove. Now 281 across the design and 466 across the
set, from a combined 1,035. **The counts are stated rather than characterised as
finished**, since new text reintroduces them and no pass makes that permanent.
One substitution had to be reverted: converting *"— see §X"* into a parenthetical
broke on references spanning a line break, leaving eight unbalanced parentheses,
which were repaired by dropping the paren rather than by patching across lines.

### 2026-08-22 (protocol completeness stated precisely)

§23.2 and the status
block now say that **nothing on the protocol itself is unanswered, with two
qualifications**: the general cycle-prevention procedure for partial information
(§6.2.5) and the owner-movement rule for in-flight resource state (§11.2) are
protocol-shaped and unwritten. Neither blocks writing code or two implementations
agreeing. Everything else outstanding is a parameter value, a policy choice, or
product behaviour. **`authoring-conventions.md` gains a rule on cleanup passes**:
scope by document set rather than by file, report the count rather than a claim of
completion — since new prose reintroduces a style habit and the design rose from
332 back to 351 within a day — and for any mechanical substitution, verify the
applied count and read a sample, because regex on remembered text matches nothing
silently, line-spanning phrases mangle, and an opened parenthesis will not close
itself on the next line. All three have occurred.

### 2026-08-22 (0.6 gains a fifth target)

**Capture and verifier query,
including keystream handling**, added to the implementation-attempt list and
flagged as the highest-value target for the next cycle: §7.5.2 is the largest
mechanism in the design that nobody has tried to build. It exercises the keystream
exchange during a ceremony, seed storage, the template-as-fixed-length-prefix
layout, tier selection by keystream length, and the parallel channel carrying a
keystream to a selected verifier. The plan also now records **why the pass finds
what reading does not** — prose can leave a decision unstated and still read as
complete, and *"signed by the issuer"* satisfied every reviewer while stopping an
implementer immediately — and the rule that **a target earns its place by being
new or having changed**, since all four existing targets found blocking defects on
their second run only because the wire format had changed underneath them.

### 2026-08-22 (dates reconstructed, second attempt)

The first reconstruction
corrected 12–14 August from transcript timestamps and then **stamped everything
after with the wrong "today"**, putting 91 entries on 17 August. The second
transcript shows that work spans **14 to 17 August**, ending 08-17 00:30, and that
**no transcript covers 08-17 to 08-22** — the session appears to have paused.
Entries re-placed by matching each one's distinctive terms against the transcript
and smoothing to suppress false positives, since a term often appears in
discussion before the entry mentioning it was written. **The span is evidenced;
an individual entry's day is approximate**, and the header says so, including that
a cross-check by turn volume disagrees enough to be worth recording. In-document
`[D — 2026-08-17]` stamps moved to 08-16; document headers now read 08-22.

### 2026-08-22 (statutory detail removed)

§7.2 carried a survey of Illinois
BIPA's private right of action and its 2024 amendment, Texas CUBI's penalty
ceiling and enforcement route, and a controller/processor analysis under GDPR.
**All of it is out.** This is not a legal document; the rules change, they differ
by where an operator and their counterparties sit, and a summary here would be
stale before it was useful — while reading as advice the document is not qualified
to give. **What remains is the load-bearing sentence**: holding identifiable
biometric data creates legal exposure that varies by jurisdiction, and an operator
should get advice about their own. The keystream section keeps its claim in the
same register — **ciphertext a holder cannot open is a different thing from a
stored likeness**, and whether that matters anywhere in particular is not this
document's judgement. Every design reason for keeping biometrics out of network
state is untouched: irrevocability, failure of fuzzing under composition, and the
metadata-resistance inversion were never legal arguments. `review-plan.md`'s entry
requiring a real legal review before deployment is retained, since it is an
instruction rather than an analysis.

### 2026-08-22 (0.1 factual verification, third run)

**Four contradictions, two
of them in text written the same day.** `wire-format.md` §2.2 said RFC 9964 and
RFC 9053 pin the key encodings to exactly three parameters. They do not: each RFC
specifies which parameters a key of that type *requires*, while COSE keys may also
carry common parameters, and **RFC 9964's own example includes a `kid`**. The
three-parameter restriction is correct and necessary — the keyhash is taken over
the encoding, so any extra entry yields a different identity for the same key —
but it is **this profile's rule and is now attributed to this profile**. Also
contradicted: **BLE is faster than NFC, not slower** — its 2M PHY carries several
times NFC's 424 kbit/s, and the keystream sizing note said the opposite. The
BIPA/CUBI "usable form" claim was contradicted too and had already been removed
with the rest of the statutory material. **Three claims narrowed**: PQXDH composes
a classical Diffie-Hellman with a post-quantum KEM and is *parameterised* over
both rather than being X25519 plus ML-KEM by definition, which this profile
instantiates; it provides **a form of** cryptographic deniability subject to
stated limitations; and UWB is the strongest **of the channels considered here**
rather than of all available ones, which §20.1 already recorded as an
uncorroborated comparative.

### 2026-08-22 (0.2 coherence, third run)

33 contradictions, 5 blocking, and
**every blocking one was introduced by an edit made the same day.** `COSE_Key`
labels are negative while §1 required all map keys to be unsigned — now scoped to
protocol-defined maps, with standardised structures exempt. §1 said a signature
covers every retained field while §2 said body-only — now stated as *every
retained field of the object it signs*, which for a transaction is the body, so
adding a signer cannot invalidate existing signatures. The transaction table
listed verifiers among presence-envelope signers, contradicting the signer bound
and §3.5. `Recovery.responses` was required by the grammar and optional by the
prose, which made attested rotation unrepresentable. And `list([keyhash])` was
described as the scope valid **beyond** horizon while §11.2 says nothing reaches
outside the Dunbar Org — resolved in favour of §11.2, since the owner's node cannot
evaluate a requester whose topology it does not hold. **Serious**: canonical
signature ordering affects **envelope bytes**, not the `txid`, which excludes the
signature array; disavowal codes are an explicit exception to the unknown-enum
rejection rule, since the banding exists so an unfamiliar code can be acted on;
design §8.1's illustrative schema gives witnesses and corroborations their own
signatures where the wire format does not, now flagged in place; queue retention
and end-to-end encryption were each described as both settled and open. **Minor**:
eleven signing roles above a table of twelve, the horizon's job count given as
five, six and seven in three documents, and the requirements documents claiming to
contain no protocol facts while repeating several — reworded to *states no
protocol rules of its own*, which is what they mean.

### 2026-08-22 (0.3 unjustified claims, third run)

199 distinct unsupported
propositions, 104 load-bearing, against 30 registered. **§20's curation note is
updated to the new figure** — it already states that the register is a curated
subset listing assumptions whose failure changes a *decision*, and the gap remains
mostly §21's parameters and the wire format's array bounds, which both documents
declare chosen rather than derived. **Two new assumptions from today's
mechanisms.** **A29**: custody obligations are a real barrier to hosting for the
institutions §1.2 names — the product case for the keystream scheme, asserted with
no operator research behind it, and if false the mechanism still survives on its
privacy grounds while the product argument does not. **A30**: users and operators
prefer indefinite retention under a space cap to time-based expiry, which is the
premise the queue decision rests on, and some operators may well prefer a hard
time limit for exactly the privacy reason the space bound gives up. Eleven
intensifiers replaced with the figure or mechanism they stood in for.

### 2026-08-22 (queue rationale corrected; invented business cases removed)

**The stated reason for the queue policy was wrong.** It read as a claim about
what users prefer; the actual reasons are that light clients may connect rarely,
and — the stronger one — **an expired verification query damages its subject
rather than its sender.** §7.4 counts `unavailable` toward finalization, so a
verifier who never receives a query cannot answer and the absence counts against
the person being verified. Expiring a queued query therefore penalises a third
party for their verifier's connection habits, and falls hardest on light clients,
who are most likely both to be queued and to be verified. **A29 and A30 removed**
along with the keystream section's product argument: a feature contributive to a
justified system does not need its own adoption case, and writing one manufactures
an unsupported behavioural claim that then sits in the register looking
load-bearing. **`authoring-conventions.md` records the rule**, with the queue as
the example of why it matters — the real design argument was better than the
invented business one, and is checkable against another section.

### 2026-08-22 (convention: whose reasoning is in the document)

The rule against
inventing justifications is narrowed to the distinction that matters. **An
explanation may be supplied by the drafter; a load-bearing one may not.** Much of
the reasoning behind this design is deliberately unwritten, since spelling it out
would bury the payload — so filling a gap with a good explanation is useful work,
and several imputed rationales here improve on the original intuition. What may
not be supplied is a justification the design comes to **rest on**: a registered
assumption, a premise another section cites, a claim that would have to be tested.
That manufactures a dependency nobody chose. Two such were written and removed the
same day, and the queue's real argument was better than the invented one. Also
recorded: **where the author states reasoning, push back if it seems wrong** —
recording a stated rationale is not transcription. And **imputed explanations
already in place are not to be stripped programmatically**: one that has survived
a claims review and the author's own reading is doing its job.

### 2026-08-22 (activity-summary residue)

The withdrawal removed the wire
object, the register entries and the parameters, and left two references behind.
**§16.2's argument for keeping the policy descriptor cited activity summaries as
an example of a mechanism reporting facts rather than evaluations** — now
corrected to note that this was the one mechanism which *would* have published one
party's figures for another to rely on, and that it was withdrawn for the same
reason the descriptor carries bad news only. §22 item 2 still listed them as a new
thing patrons publish needing a propagation rule. A sweep confirms every other
mention of a withdrawn mechanism sits inside a withdrawal note or Appendix A's naming
example.

### 2026-08-22 (style-pass regressions repaired)

A regression check found three
classes of damage from the em-dash reduction, none of them from the edits that
prompted the check. **Ten sentences began lowercase**: the substitution turned
*"X — it is Y"* into *"X. it is Y"*, and the capitalisation pass that followed
required at least two following letters, so every sentence starting with *it* was
skipped. **Three status markers lost their punctuation** — *"RESOLVED see §7"*
where the dash had been removed and nothing replaced it. And one section range
became `§12–8` when only its first half remapped. All repaired, and the
capitalisation rule corrected to handle two-letter words. The `§§1–13` range and
the quote-spanning-line-break hits are legitimate and were left alone.

### 2026-08-22 (0.4 parameter inventory, third run)

**No numeric disagreement
among live protocol values.** The four conflicts found are one failure: **§21.1's
unset list had fallen behind three decisions.** It still listed the
challenge-window delay and the down-line revocation threshold, both withdrawn when
the veto mechanism was — a patron shares no state with a subordinate's client and
can block nothing; queue retention, settled as indefinite at the direct patron;
and peering replication units, which `wire-format.md` §4.4 defines as bytes. All
four removed, the queue entry narrowed to its actual residue, and the count
corrected from nineteen to sixteen. **§21.1.1's hardening table emptied as a
result**, since three of its four entries were the withdrawn parameters and the
fourth was already resolved — rewritten so the rule survives without stale
instances, and noting plainly that **no unset parameter is currently in that
class because the mechanisms were withdrawn, not because the hazard was
addressed.** Everything else the inventory flagged is already marked historical in
the documents: presence-record size, geohash dimensions, the infra threshold, the
anchor guideline, the signer bound, the verifier-response array, and timestamp
serialization.

### 2026-08-22 (§17 and §20 registers audited)

Prompted by the observation that
§17 reads as full of unfamiliar open questions. **The cause is mostly ordering**:
entries had been prepended as they were added, so the privacy register ran
P1–P12, P18, P19, P20, P26, P27, P30, P31, N3, P29, P28, P25… and the assumption
register was similarly scrambled. All three registers — privacy findings,
correlations and assumptions — are now in numeric order, which is how a reader
checks whether something is already known. **§19.3's selective-disclosure
proposal is marked as what it is**: proposed by the drafter, never put to the
author, and not adopted. It is retained because §19.1's composition argument
keeps arriving at it and because leaving the record all-or-nothing should be a
decision rather than an omission — but **nothing depends on it**, and P1 and P19
name it as an available direction rather than a planned one. **P4 corrected**: it
said the queue was open, where retention and sibling replication are settled and
the residual is queue metadata.

### 2026-08-22 (§§22–18 audited)

**§23.3 was a stale duplicate of §23.2.** Both
enumerated what is open; §23.3's version still said queue retention was
unresolved, claimed "two encoding decisions" while listing one, cited five
parameters needing a security argument where none remain, and used `###` headings
for its own subsections so they appeared as siblings of §23.3 rather than under
it. Replaced with what only it carried: **test vectors, and what a test suite
would add** that the implementation passes cannot, since those ask *can this be
written?* and stub the error paths while a suite asks *what happens when the input
is wrong?* **§22's five entries are all live.** Indentation normalised, and item 4
updated — the keystream scheme widens archive-recovery loss, since seeds are
device state too, so a lost device also loses the ability to unlock one's likeness
on every counterparty's machine.

### 2026-08-22 (P8 withdrawn)

*Topology deanonymisation by association* is not a
distinct vulnerability. **Identifying one member by real name yields their job,
not a label for any of their subtrees**: §3.1.1's membership plurality means a
member belongs to several, and nothing in the protocol says which is a workplace
rather than a bowling team or a congregation. **What labels a subtree is its
catalog**, which is P15 and is a designed feature working as intended — so the
mechanism runs the opposite way from P8's claim: you do not deanonymise a subtree
by identifying a member, you learn what a subtree is for by reading its catalog,
and its members inherit that. The attacker P8 described also **holds both the
topology and a real-name link, which makes them a horizon member**, and horizon
membership already carries a known package of disclosures (§1.2). Folded into P15.
**§19.4 now states the general test**: a finding must add something membership
does not already carry, and any finding whose precondition is *an attacker inside
the horizon* should be checked against that before being entered.

### 2026-08-22 (withdrawn findings get tombstones)

Removing P7 and P8 left two
silent gaps in the privacy register, which reads as an error to anyone auditing
it. Both now keep a struck-through row recording what they were and why they went.
**§19.4 states the rule: numbers are not reused.** Renumbering would be worse
than a gap — a dangling citation is visible, whereas a reused number makes an old
citation resolve to a *different* finding, silently. The assumption and
correlation registers were checked and have no gaps.

### 2026-08-22 (P9 withdrawn)

*Divergence-notification fan-out* is not a privacy
cost. **The exposure it named predates the recovery**: a thief holding the key
already reads everything addressed to it, so an inquirer's loss dates from the
theft rather than from the notification, and recovery only lets the legitimate
holder regain one subnet from a position of none. **The disclosure also runs in
the design's favour** — a third party who sees a rotation and doubts the
un-rotated binding in another subnet devalues the memberships the thief still
holds, which is the only mechanism here that damages a thief across subnets they
retain, and it works because the fork is visible. §9.0.2 now records that
reasoning where the mechanism is specified. **§19.4 gains a second test**: a
finding must name a loss the design *causes* — check when the harm occurs, not
only whether the mechanism touches it.

### 2026-08-22 (P10 closed, P14 accepted)

**P10 is closed by the policy
descriptor's redesign.** It described fingerprinting a descriptor's published
parameters; §16.2 now publishes none — bad news only, silence meaning nothing — so
there is nothing to fingerprint, and the proposed mitigation of bucketing raw
parameters refers to values that no longer exist. **P14 moves from open to
accepted.** Chain back-pointers reveal a subject's chain head, so a counterparty
meeting them twice sees how far it advanced — but that discloses nothing past
§7.4's finalization threshold, where *n* is the subject's presence count and
**the evaluator learns it from the subject by design**. Activity level is already
an input every evaluator receives; a chain head is a coarser view of the same fact
given to someone who has met them. **§19.4 gains a third test**: a finding whose
mechanism is withdrawn is closed rather than stale, since leaving it makes the
register describe a system nobody is building. The register now holds 27 live
findings and 4 tombstones.

### 2026-08-22 (C11 updated to its residual)

The entry still described the
**pre-fix** model: an on-demand prekey fetch announcing each intended
conversation. Blanket batch prefetch across the Dunbar Org addressed that — an
ordinary fetch now names a population rather than a person, and the two request
forms are structurally distinct on the wire so a node sees which it received
rather than inferring motive. **What remains is narrow**: the on-demand one-time
key request, made when a session is actually being opened, so a node sees a
request followed by traffic or by nothing and an abandoned contact still leaves a
trace. Severity drops from Medium–High to Low–Medium, and depletion is bounded by
rate limiting rather than by policing motive. **P26 sharpened in the same pass**:
it is the shape C11 had *before* the fix, and **no equivalent defence has been
considered for resolution requests** — which is the useful comparison, since one
problem was solved and its twin was not.

### 2026-08-22 (query log becomes an ephemeral lock)

**The anti-oracle defence
is rate limiting, and rate limiting needs a lock rather than a log.** §7.4 now
states that what a subject holds is a counter per requester and per ceremony
window, kept **only for the enforced ceremony duration** — a few minutes — and
that it exists to refuse the next query rather than to record that a previous one
happened. A durable queryable history was a separate feature nobody required, and
an expensive one: it is an auxiliary timeline of ceremony attempts **including
those abandoned before any record existed**, sitting on a device that can be
seized. **C12 and P22 both close.** C12's correlation needed a retained history to
join against the archive and there is none; P22 asked for a retention rule and the
answer is that nothing is retained. The parameter is dropped from §21.1, leaving
fifteen unset. As with every client-side rule, a non-conforming client may retain
more and nothing detects it.

### 2026-08-22 (fourth register test: ask whether the component is required)

§19.4 gains a test in the form of a question rather than a judgement: **when a
finding assumes a component, ask whether the component is required** — not whether
it can be made safe, which takes the component as given and generates work. C12
and P22 both reasoned about protecting a verification-query log that was not
needed, and **a finding can entrench the thing it is about** while the register
looks like it is doing its job. `authoring-conventions.md` records the
corresponding instruction: **the drafter is poorly placed to make this call**,
since whether a component is load-bearing depends on intent that is frequently
unwritten, so the judgement belongs with the author and the question costs one
exchange.

### 2026-08-22 (C13 withdrawn, C16 distinguished)

**C13 fails on two counts.**
The join needs one party holding both halves, and only the disavowing patron does
— resource-grant state is local to the owner's node, and that patron already knows
their own reasoning; an observer with the code alone gets the band, which is what
the enumeration was designed to give them. **The scenario is also unreachable**:
disavowal ends the relationship, so the membership gate revokes all access as a
consequence, and there is no separately-dated grant revocation to correlate
against. **C16 was checked against the same test and survives**, on a difference
worth recording: an abuse report is written by a **third-party reporter**, not by
its recipient, so the owner receives particulars they did not previously hold
about someone they may know nothing about. C13 needed a party to join two facts
they already had; C16 delivers a new one.

### 2026-08-22 (C16 withdrawn; P27 narrowed)

The distinction drawn between C16
and C13 an hour earlier **was wrong**. An abuse report goes from a resource to its
own owner, and the owner granted the role in the first place, so they are not
receiving particulars about a stranger. **The schema settles it more firmly**: an
`AbuseReport` carries `resource` and `reporter` and has **no subject field**, so a
detail describing someone's circumstances has nobody to describe — either the
resource reports to its owner, both being that party's own assets, or a user
reports about a resource and any personal particular is their own. **P27 survives
in narrowed form**: not a disclosure to the recipient, but §11.6 states that a
reporter may hand the signed object to anyone, and **a signature makes forwarded
particulars credible in a way an unsigned account would not be.** The wire
format's rationale for the 1 KB bound is corrected to that reason, since it
previously cited the withdrawn correlation.

### 2026-08-22 (C14 narrowed)

The capability vector is exchanged in `Attach`, so
the party who sees it is the **serving infra node**, which §14.1.2 says need not be
the patron — and that split removes most of the finding. **A patron or resource
owner is excluded**: they participated in both adoptions and hold the link
already, by the recovery record if it was a rotation and by having met the person
if it was a fresh Genesis identity, since adoption requires a ceremony. What
remains is a serving node that **serves without having adopted**, seeing attach
traffic from two identities on one device — and there **the capability vector is
the weakest of the three signals**, since the network point is the fingerprint.
This is ordinary network-level linkability rather than anything the capability
mechanism introduces, and greasing was never claimed to address it. Severity
Medium to Low. §13.7 now states the same thing where the fresh-identity path is
described: **a fresh identity is unlinkable in the record, not on the wire.**

### 2026-08-22 (C14 withdrawn)

A serving infra node is necessarily inside the
Dunbar org, and a rotation propagates as a topology-class message **pushed within
horizon** (§9.0.2) — so **any node positioned to see both attaches has already
received the record binding the two keys**, and a device fingerprint adds nothing.
The fresh-Genesis case fails from the other side: where unlinkability matters the
new identity appears in a *different* subnet under a different serving node that
sees only one, and where a single node could see both, the person was adopted by a
neighbour who met them, so the link exists socially whatever the transport shows.
**§13.7 restated as a result**: unlinkability is a property of **where you
reappear**, not of what you avoid signing — appearing fresh in the same
neighbourhood is not unlinkable at all, and the operation is meaningful only where
nobody knows the old identity, which is what §12.4's absence of cold lookup
protects.

### 2026-08-22 (C15 accepted)

Pairwise identifiers address cross-*operator*
linkage; one vendor running several resources correlates them from account,
device and network data it holds anyway, and no identifier scheme changes that.
**Recorded as an accepted cost rather than an open correlation** (§19.7 item
10): resources differ in what anonymity they offer, and **which ones a subnet
admits is part of how it sets its security posture** — a user more
security-conscious than their organisation may decline a service the organisation
accepts. That is the same remedy §11.7 gives for gateways: the network binds a
service to what its owner published, and beyond that the choice is the user's.
§11.0.2's scope note aligned with the same framing.

### 2026-08-22 (C17 and P28 narrowed)

Both are largely obviated by §7.5.2. A
compliant client holds captures as ciphertext under the *subject's* keystream, so
retained EXIF and background are unreadable, and **a non-compliant client keeping
plaintext is the baseline** — the same bad actor with an ordinary camera app gets
the same thing, which is the test §1.2.2 sets. Severity from High to Low. **The
residual keeps the stripping obligation alive**: a compliant holder decrypts
legitimately during a later verification and has the plaintext in hand for that
window, which is the one moment the keystream does not cover. Recorded in the
light-client requirement so the obligation does not read as redundant now that
encryption exists.

### 2026-08-22 (C18 withdrawn; sandboxing restated)

The correlation assumed a
hosted package could reach topology, liveness, queue state, prekey requests and
role-evaluation inputs. **The design offers no binding that exposes any of them**:
§11's rule that *the resource never reads network state* applies to a hosted
package as much as an external one, and `infra-client-requirements.md` §9.2 now
says the hooks **do not exist** rather than that they should be scoped narrowly.
Offering a binding and scoping it carefully would still be offering it, and the
narrow scope would become a policy an operator could widen. **What remains is an
implementation question, not a correlation**: whether an isolation mechanism
enforces the boundary against hostile code. §8.1 now says plainly that this is for
the sandboxing literature rather than a claim the document makes, and that an
implementer should treat WASM component isolation as an open engineering question.
P31 narrowed to the residual — installed code runs inside the boundary the threat
model draws around operator conduct.

### 2026-08-22 (0.5 rule fragility, fourth run)

Two residual rules, and the
reviewer explicitly excluded wire grammar, borderline cases whose invariant is
stated elsewhere, and all three requirements documents. **The disavowal reason
rule was stated as an encoding fact** — *field 5 is an enumerated code, never free
text* — with its rationale in prose but the invariant nowhere. Now stated at
role level in both documents: a party issuing a durable negative attestation must
express its basis only through a bounded, machine-interpretable category whose
adverse character is structurally visible, and must not attach arbitrary
accusation text. **The notification rule was the sharper finding.** "High-value
actions" reads as an abstraction while its operative definition was the list
*adoption, departure, peering* — so a future operation with the same security
consequence, or a merge of those three into a generic topology-change
transaction, would leave an implementer with no criterion. Restated as: when a
participant under another party's authority authorises an action that materially
changes an authority binding or establishes a cross-tree infrastructure
relationship, its client makes it visible to the party vouching for it, and **the
notification is observational only** — it must neither gate nor delay.

### 2026-08-22 (0.5.2 enforcement boundary, fourth run — CLEAN)

**No findings.**
The first pass in the programme to return none. Six apparent mandates were checked
and each survives on the §1.1 test: §7.4's verifier rule is enforced by the
verifier itself, which receives the proof in the query; §11.5's disclosure rule
rests on `discover_scope` travelling in signed catalog evidence the recipient can
read; §11.6's abuse-report rule is scoped to what the sender controls and refuses to
bind any holder; `wire-format.md` §4.2.1 governs a client's treatment of its own
user's move from topology it already holds; §6.1.1's greasing obligation is
checkable by any peer that greases; and `resource-requirements.md` §3.1's header
rules bind an intermediary over traffic it originates. **The reviewer also
confirmed the three requirements documents avoid claiming their local obligations
are protocol-enforceable**, which is the framing added in the second run.
Recorded as one reviewer finding nothing rather than as proof none exists — but
the corrections from earlier runs have held, including the abuse-report rule that
the register keeps as the worked example of the failure.

### 2026-08-22 (0.6 adoption, third run)

Ten encoding questions, and **two would
have caused silent divergence or worse.** `VerifierResponse` field 8 was required
and described as naming the prior identity, but **the equality relation to
`Recovery.prior_key` was never stated** — so evidence collected about old identity
X could be embedded under a Recovery claiming old identity Y with every signature
still verifying. That is a security hole rather than an ambiguity, and it is the
second time this field has needed work since it was added. And **Adoption field 5
did not say whose `KeyMaterial` it carries** — the body names both node and
patron, and there is no discriminator on the wire, so two implementations could
differ with nothing to detect it. Fixed to the adopted node's. **Also settled**:
nested COSE objects are **untagged**, since RFC 9052 permits either and libraries
expose both; the `alg`/`kid`-only header rule applies to nested objects and not
merely the envelope; ML-DSA signing is **deterministic**, which changes nothing on
the wire but makes a failing test vector diagnosable; three Recovery consistency
rules — response `subject` must equal the newly adopted node, duplicates from one
verifier are malformed, and an old-key proof and verifier responses may both
appear as independent evidence; and **structural verification does not dereference
`proof_of_presence`**, which is an evaluation step against the *valid versus
effective* boundary already drawn.

### 2026-08-22 (resolution: iterative with referrals)

A 0.6 implementation
attempt found **a direct contradiction between the two documents**: §12.6.1
described resolution descending through infra nodes, while `wire-format.md` §7.6
forbade forwarding outright — a rule added in an earlier 0.6 pass and never
checked against the design. **The iterative model as written also did not work**:
a node reporting "not authoritative" gave the requester nowhere to go, since no
referral existed. **Resolved as DNS-shaped iterative resolution**: a requester
queries a known anchor and receives the answer or **a referral naming the next hop
and its endpoints**, repeating and caching as it goes, with a node free to refer
past several indices where it knows its subtree. **The objection that forwarding
lets an intermediary misreport progress does not survive endpoint
authentication** — the requester authenticates each endpoint against the keyhash
it expects, so a wrong address is a handshake failure rather than a silent
misdirection, and **the lie is self-detecting at contact**. New `Referral` type,
reply code 3, and failure code 1 narrowed to *cannot refer*. **Node type stays out
of the locator**: a light client that gains subordinates becomes infra without
moving, so an address asserting terminal type would go silently wrong in every
cached copy — the residual path suffix carries the distinction instead, learned at
resolution time, and the client renders it. **Resolution cache TTL** added as
unset: infra nodes move and nothing currently bounds a stale entry.

### 2026-08-22 (referral model propagated)

Consistency sweep after the
resolution change. **Three places still described the superseded model.** §12.3
Case 1 had Bob hand his message to his patron, which forwards toward the anchor
and descends — message routing rather than resolution, and it contradicted
§12.6.3's rule that infra nodes carry no payload but their own clients' — now
rewritten as query, refer, cache, then contact the serving node directly.
**§12.6.2 was titled *repaired in transit***, which assumes messages travel
through nodes; repair now happens during resolution, and the requester's cache
absorbs the correction for later contacts. And a claim about invalidation cost
said repair degrades a bad case to *extra hops*, now *extra round trips*.
**`infra-client-requirements.md` had no resolution obligations at all** despite
being the party that answers and refers — new §3 covers answer, refer, repair,
report failure, the multi-index shortcut, answering from one's own children, and
the process-and-discard obligation on what a request discloses. Sections
renumbered and all inbound references updated.

### 2026-08-22 (infra resolution obligations, properly)

The §3 added in the
previous sweep covered *answering* and almost nothing about the state required to
answer, and it **contained a contradiction**: "answer from your own children"
against "you may refer past several indices where you know your own subtree."
Resolved by making deeper caching an **optimisation above the floor** — the
constant-state guarantee bounds what a node must keep, not what it may. §3 now
specifies the **child table**, forwarding records and their 90-day TTL, the
**anchor table's ingestion boundary** with the requirement to state which model is
implemented, and maintenance: remove on departure or disavowal, replace endpoints
on a strictly greater `seqno`, collapse forwarding chains at the source.
**A real gap surfaced**: nothing delivers an infra child's endpoints to its
patron. A light client's arrive at attach; an infra child serves itself and never
attaches — so **a patron cannot refer to a node whose address it does not hold**,
which blocks resolution past one hop. `SignedLocator` is the natural carrier and
no propagation rule for it exists. Recorded in §23.2 as a subsystem blocker rather
than an unset parameter, since what is missing is a mechanism.

### 2026-08-22 (consistency check after the referral change)

All references
resolve in every direction across the five documents; assumption, correlation and
privacy registers have no gaps; parameter count matches its claim; no `[P]` or
`[OPEN]` markers remain. **Two claims had gone stale.** §23.2 said *three items*
above four subheadings — correct, since the fourth is the resource residue which
blocks nothing, but it read as an error and now says so explicitly. And **the
protocol-completeness claim still said two qualifications** when the locator
propagation gap makes three — recorded in both §23.2 and the status block, with
the note that this one **does** block a subsystem, unlike cycle prevention and
owner movement.

### 2026-08-22 (0.1 factual verification, fourth run)

**One contradiction.**
§23.1 said a native WASM runtime under WASI "has ordinary filesystem access";
**WASI is capability-oriented and grants none** — a host chooses what to expose,
so persistence there is a property of the runtime's configuration and a client
cannot assume it. **Four narrowings that touch mechanisms.** The keystream sizing
note was wrong in the direction I had just corrected it: **2 MB over NFC is
roughly 40 seconds at raw line rate**, which is a floor rather than a realistic
transfer time, and BLE's advantage depends on negotiation and overhead — the
honest statement is that **neither proximity channel is a plausible carrier at
that size**, which strengthens rather than weakens the case for carrying the
keystream over the session. *"The ratchet covers everything after"* is too
categorical: protection improves as ratchet contributions are incorporated, and
how quickly depends on the message pattern and compromise model. Feldman and
Pedersen are **examples** of verifiable secret sharing rather than the only
options. And TLS 1.3's downgrade protection does not by itself prevent negotiation
below the named group — **that is this profile offering only one group**, and
attributing it to TLS misstates where the property comes from.

### 2026-08-22 (0.2 coherence, fourth run)

15 contradictions, 2 blocking, and
**both blocking ones came from fixes made earlier in this session.** The header
rule was generalised to *every COSE object carries `alg` and `kid`* while the
exception it contradicts — embedded `COSE_Sign1` omits `kid` — was left standing
four lines below. Resolved by stating the rule as a condition rather than a list:
**`kid` is present exactly when the surrounding structure does not already name
the signer**, so envelope entries carry it and embedded objects do not, since a
`VerifierResponse` names its verifier in a field and a second copy could disagree.
**Late verifier responses were said in three places to have a present encoding
and no such object existed** — now `LateResponse` (`wire-format.md` §7.4), a
standalone signed object referring to the record it supplements, which explicitly
**does not amend it**: the record is immutable, finalization already happened, and
a late response is evidence an evaluator may weigh rather than a change to what
was decided. **Thirteen serious**, most being today's decisions not reaching every
mention: prekey exhaustion **degrades forward secrecy rather than blocking
messaging**; the presence signer ceiling is 18 and the dual-role rule was implying
34; the disavowal notice period referenced the withdrawn challenge window; the
anchor-table sizing omitted the self-signature it now carries; and the security
analysis still said relays see plaintext.

### 2026-08-22 (0.3 unjustified claims, fourth run)

123 propositions, 79
load-bearing, against 31 registered. **The count has converged** — the previous run
found 104 load-bearing on a stricter split, and the gap is still mostly §21's
parameters and the wire format's array bounds, which both documents declare
chosen. **Three new assumptions, all from §1.2.2's emergent properties**, which
are load-bearing and were unregistered because they arrived as reasoning rather
than as decisions. **A29**: fabricating a whole fictitious graph is easy — if it
is harder than assumed, correlated evidence is *more* probative and the deniability
property shrinks. **A30**: a remote evaluator cannot distinguish a synthesised
subnet from a real one, untested against anyone applying structural analysis
rather than key-checking. **A31**: to an attacker accountable to no evidentiary
standard, attestation adds nothing — a claim about how such parties decide, and if
wrong the archive's non-repudiability costs more than recorded. Eight intensifiers
replaced with the quantity or mechanism they stood for, including *"catastrophically
exploitable"* → exploitable by a deep fake subtree, and *"chain verification is
cheap"* → one hash comparison per record.

### 2026-08-22 (0.4 parameter inventory, fourth run)

**No unresolved numeric
conflict**, the second parameter pass running to come back clean on values. The
reviewer also confirmed that the conflicts the documents record as *resolved* —
the presence-record size, the signer bound, the verifier-response array, the
geohash figures — read as history rather than as live disagreements. **One live
status inconsistency**, and both halves of it were in the same place: §21.1.1's
*freely tunable, forever* list still contained **query-log retention**, which has
no subject since there is no durable query log, and **queue retention**, which is
settled as indefinite and therefore not tunable at all. Removed rather than
reclassified, with the queue's **per-subordinate storage cap** left in its place
as the quantity a node does choose. Listing a settled value among the freely
tunable ones invites an operator to change it.

### 2026-08-22 (0.5.1 rule fragility, fifth run — CLEAN)

**No residual
violations.** The second pass in the programme to return none. Its reviewer
checked seven plausible exceptions rather than accepting the status claim —
departure signing, formation subtype, disavowal reason field, witness nomination,
`Attach` field 1, routing repair, abuse-report delivery — and found each carries
its invariant independently of the identifier that encodes it. **Spot-checking
found something the pass was not looking for**: the departure rule read
`[D (§6.2]`, a malformed decision marker, and a sweep found seven more in the wire
format. **The reference checker cannot see these** — they do not parse as
references — so they had survived every pass. Repaired, and the departure rule
restated at role level while there: a party with authority may never gate an
action whose sole effect is to end that authority.

### 2026-08-22 (0.5.2 enforcement boundary, fifth run)

Two findings, and the
reviewer's framing of the first is the useful part: **§11.6 had already made the
move §11.5 did not.** The catalog rule said a relaying party **must** disclose an
entry only to authorised recipients — and once a relay holds the entry, the owner
has no state showing whom it copied it to. Restated as an obligation on the
**publisher**: carry the intended discovery audience inside the signed object so
any recipient can tell whether it was meant for them. **The recipient can
recognise a leak; the owner can neither prevent nor learn of one**, and
`discover_scope` filtering is now stated as conforming-relay behaviour rather than
a property the owner enforces. **Second finding, same defect in declarative
dress**: §9.0.2's fork detection said an inquirer who sees divergent assertions
*notifies both patrons* — an arbitrary third party sharing state with neither. A
patron receiving no notice cannot distinguish *no divergence was observed* from
*an inquirer observed it and said nothing*. Now marked as conforming behaviour
with the gap stated, and **§22 records the durable alternative**: an
inquirer-signed divergence notice makes the *observation* an object that stands
alone, which is evidence rather than assumed behaviour.

### 2026-08-22 (0.6 adoption, fourth run)

Five questions, **three of them
consequences of the header rule rewritten two passes ago.** Saying *every COSE
object carries `alg` in its protected header* did not distinguish a signature
container from a signature entry: a `COSE_Sign` carries no signature of its own
and therefore has no algorithm to name. Now stated as **`alg` lives in the
protected header of whatever structure holds the signature**, with the outer
`COSE_Sign.protected` empty. And *nothing in the unprotected header* was
ambiguous between rejecting and ignoring — now **malformed, not ignored**, since
tolerating one gives a single logical object two byte encodings. The word
"ignored" appears elsewhere about *unknown map keys*, which are preserved and
re-serialised; that rule does not extend to COSE headers. **The serious finding is
the `match` threshold**: as written it applied to every recovery, which would make
**attested rotation unrepresentable whenever the verifier responses are
inconclusive** — inverting the intent, since an old-key proof is the prior identity
authorising a change to itself and is the *stronger* evidence. Scoped to the
branch where field 3 is absent. Also settled: **a verifier lacking a signer's key
has neither verified nor rejected** — a third outcome naming the missing identity,
since collapsing it into "invalid" reports a well-formed transaction as malformed;
and the two signatures **need not be produced together**, with nothing in the
envelope recording how they were gathered.

### 2026-08-22 (0.6 resolution, third run)

Twelve questions, **four of them one
defect**: `Referral.advances` was added without saying what it counts. Now
**incremental from the referring node's own position**, never an offset from the
anchor, and **MUST be at least 1** since a referral advancing nothing is a loop.
With the two rules that depend on it: **each node interprets the path from its own
anchor-relative position**, so requests carry the full unmodified path and no
consumed-prefix field — a field the requester computes and every node must trust
is state an intermediary could misreport. And the requester **checks accumulated
`advances` against path length minus residual on arrival**; a mismatch means a
node miscounted and the resolution is unsound rather than slow. **The reviewer's
premise about infra nodes not seeing their subordinates' subordinates was wrong,
and §14.1.2 already said so**: a light client attaches to the nearest
infrastructure on its patron chain, walking up past light-client patrons, so a
serving node holds its **entire light-client subtree** and resolves those paths
itself. Intermediate light-client patrons adopt and countersign but carry no
traffic. **Referral is therefore only ever between infra nodes** — now stated in
§12.6.1 and the infra requirements, where it was implied by the attach rule and
never drawn out. Also settled: first contact with an unpinned anchor is
trust-on-first-use over an **unauthenticated** peer, so it must disclose nothing
beyond the query; a locator whose anchor is absent locally is a caller-side
condition rather than a wire failure; and **equal `seqno` with different contents
is malformed**, not a tie to break.

## 2026-08-23

### 2026-08-23 (0.6 presence validation, third run)

Six questions, and **one is
a real attack the selection rule left open.** *n* was defined as transactions
reachable in the subject's archive DAG, without saying from where — and a
validator counting from the subject's **current head** can be shown a backfilled
archive: append records after the ceremony, present the enlarged history to a
later evaluator, and the threshold the record was supposed to meet comes out
different from what any witness saw. **Now rooted at the back-pointer the record
commits for that subject**, which key 0 fixes at signing time and the signature
covers, so it cannot move. Also settled: **validate the received bytes rather than
decoding and re-encoding**, since that erases the duplicate keys and unknown-key
encodings the check exists to find; the 730-day lower bound **saturates at zero**
for records near the epoch; and **validation is per-subject** — a holder with one
participant's history verifies that half and reports the other as *unverifiable*,
which is neither valid nor invalid, and collapsing them lets a caller overclaim
what it checked.

### 2026-08-23 (0.6 client attach, third run)

Fifteen questions, most of them
correctly local policy. **Three were wire-level gaps.** The `Capabilities` map had
**no bound at all** — §1 bounds every array and this is a map, so it escaped the
rule; now 64 entries and 1 KB per value, since an unbounded map in the first
message of a session is a denial-of-service surface. **`SiblingRef.key_material`
omission had no resolution path**: the infra requirements permit omitting it when
the client is known to hold the key, and nothing said what a client does when it
does not. Now **omissible only when the serving node itself supplied that key
earlier**, and a client lacking it treats the sibling as **unusable rather than
dialling it unauthenticated** — there is no fetch path, because the party that
would serve one is the node that is down. And **a sibling list naming the
receiving client, or containing duplicates, is malformed**: a client cannot fail
over to itself, and a duplicate is either an error or an attempt to weight one
endpoint in a list the client tries in order. Also settled: heartbeat counter 0 is
sent **after one interval, not immediately**, since an immediate beat makes the
first interval a half-interval on one side; the advertised interval is **fixed for
the session**, with no update message; and the reference greasing shape is a
random 64-bit id with 8 random bytes, so a greased parameter is not
distinguishable by its size.

### 2026-08-23 (0.6 ceremony and verifier query, first run — construction respecified)

27 questions, the most of any target, on the mechanism nobody had
built. **Three were security-critical and the fix improves the design.** The store
is now **two independently sealed AEAD segments with keys derived from a seed**,
not one keystream with a prefix. **Encryption is authenticated**: a raw keystream
is XOR, and a device thief holding ciphertext and knowing the plaintext could
produce a chosen plaintext that later decrypts legitimately to something the
subject never presented. **A released key binds `subject`, `holder` and `txid`**,
so a key given to one counterparty does not open another's copy of the same face
and cannot be replayed at a later ceremony — without that, releasing to one
counterparty silently released every copy in existence. **And the transfer problem
disappears**: the subject sends a 32-byte seed or a derived key, not megabytes of
expanded stream, so the NFC and BLE throughput analysis was chasing a problem that
existed only because the mechanism was described in terms of the expanded stream
rather than what generates it. Any channel carries 32 bytes. Also settled:
**truncated or unauthenticated ciphertext is a decryption failure, never a
no-match** — a corrupted store must not become adverse evidence about its subject.
Terminology converted across the design and light-client documents.

### 2026-08-23 (0.6 targets 6 and 7 added)

Two implementation targets covering
the resource layer, which **no attempt has touched** despite being roughly a third
of the design and the part §1.2 names as the network's purpose. **Target 6,
register a resource and propagate its catalog entry**: `CatalogEntry`
construction and owner signature, scope predicate encoding, propagation within
horizon, and conforming-relay filtering — with the pressure on the scope language,
since predicates are evaluated against the recipient's own topology and §11.5 now
requires the publisher to carry its intended audience, so the recipient's check
must be expressible from the entry alone. It also exercises the
subtree-acknowledgement gate, which changes what a node may serve and which the
catalog entry does not mention. **Target 7, authorise a user to a hosted resource
end to end**: predicate evaluation, pairwise principal derivation, credential
construction, the header contract and its stripping rule, and termination on
authorisation-state change — with the pressure on the two legs, since
client-to-node is an rhtn frame and node-to-resource is ordinary HTTP and an
implementer must build both. Target 5's description was also stale, still
describing the keystream model its own first run replaced.

### 2026-08-23 (review-plan references remapped)

Adding the resource targets
surfaced **ten stale section references in `review-plan.md`**, predating several
renumbers — it had never been included in the cross-document reference sweep,
because the sweep covers the five specification documents and the plan is not one
of them. Remapped. **`review-tracking.md` has 52 of the same, and they stay**: it
is a historical record of what each pass found and how it was disposed of, so its
references are correct as of filing, and rewriting them would falsify the record.
A note now says so.

### 2026-08-23 (0.6 ceremony, second run)

25 questions against the respecified
construction, and **one is a circularity introduced by this morning's fix.** Key
derivation bound `txid` — which hashes the finalized body **including verifier
responses**, while sealing happens at capture, before either exists. **Rebound to
the ceremony pre-commitment**: fixed before capture, countersigned by both parties
and the witnesses, unique per ceremony — everything the binding needed, available
when the binding is made. **The key grant had no message**, the same gap class as
the `CurrencyAttestation` signature: three implementations would have invented
three. Now `KeyGrant` (`wire-format.md` §7.3), carried as end-to-end encrypted
payload and **never retained in a record**, since a persisted grant defeats the
retention property the scheme exists for. It names the record to open — a verifier
met several times holds several sealed captures — and the query it answers, so an
unattached grant is malformed rather than an invitation. Also settled: **a ceremony
seals a new capture rather than replacing an earlier one**, each ageing
independently; and a decryption or authentication failure is **`inconclusive`,
never `no-match`** — reporting it as no-match would turn a corrupted store into
adverse evidence about its subject.

### 2026-08-23 (0.6 hosted resource authorisation, first run)

The target added
to cover the resource layer found what it was added to find. **The authorisation
rule is implementable** — membership gate, subtree acknowledgement, predicate
evaluation, pairwise derivation, credential construction all work — and **the
request cannot be carried**, because frame types 5 and 6 were named and neither
body was ever defined. **A direct contradiction alongside it**: §8.0 put resource
frames on stream 0 while §8.1 assigns request/response to bidirectional streams.
Resolved toward §8.1 — resource traffic is not session control — with frames 5 and
6 withdrawn from the control table and `ResourceRequest`/`ResourceResponse` drafted
at `wire-format.md` §11. Two rules fell out of writing them: **the node's status
codes are not the resource's**, so an application error returns inside a delivered
response rather than looking like a gateway refusal; and **`not authorised` does
not distinguish absent from refused**, because distinguishing them tells a
stranger about the owner's membership and policy. **A topology gap is recorded
open**: resource eligibility spans the owner's Dunbar Org while attachment is to
one's own serving node, so an eligible requester two tiers away has no specified
path to the host. **The completeness claims are corrected** — the identity,
presence, routing and messaging layers are specified and the resource layer is
not, where §23.2 had said nothing blocks interoperation.

### 2026-08-23 (refusal reasons split by horizon)

The uniform `not authorised`
answer dropped information that should reach anyone inside the owner's Dunbar Org.
**The membership gate already separates the two cases**, so a member lacking a
`SubtreeAck` or matching no role predicate now receives the specific reason (codes
5 and 6) — they can act on it, and they already hold the topology it describes,
so withholding it made the resource undiagnosable to precisely the people entitled
to use it. **A non-member receives `refused` and nothing else**, which is the case
the opacity was actually for. One consequence stated with it: **`no such resource`
is only ever sent to a member** — to a stranger every path returns the same code,
including a keyhash naming nothing, or the differing replies let them enumerate
what a host runs.

### 2026-08-23 (refusal evaluation order made normative)

Splitting refusal
reasons left the codes unordered, which conflated the two conditions the split was
for: **a member with no role and a member whose service is down**. Order is now
normative — membership, existence, acknowledgement, roles, **then availability** —
and the placement of availability last is what does the work. **A role-holder
learns the service is down; someone with no role never does**, so operational
information about the owner stays inside the set entitled to it. Running
availability earlier would answer *unavailable* to someone who has no role, which
is true and useless: they would retry indefinitely against a resource they could
never reach.

### 2026-08-23 (role assignment is a materialised table)

The predicate language
was recorded as a subsystem blocker needing an interoperable grammar. **It needs
none.** Role assignment is a **table** — one row per Dunbar Org member per
resource, bounded at a few hundred — and predicates are a **macro over it**,
evaluated when an operator configures roles and in a background pass when a node
enters or leaves the horizon. **Authorisation at request time is a lookup**, which
makes it deterministic, cheap, and readable by the operator who set it. **The
predicate never crosses the wire and no other party evaluates it**, so two
implementations may differ in expression power and remain conforming: what the
network sees is the table's result. That is the opposite of `Scope`, which travels
in a `CatalogEntry`'s `discover_scope` for a recipient to check, and is therefore a
closed vocabulary. **A table change is also what terminates a session**, rather
than a predicate change in the abstract.

### 2026-08-23 (role visibility corrected)

A first attempt said *what the
network sees is the table's result*. **It sees nothing of it.** Roles are internal
to the owner's node end to end: the node holds the table, the resource receives
roles over the hosting path inside the owner's own machine, and the user sees
theirs only as the resource's interface reflects them — as a menu, a permission,
an error, never as protocol. **What the network sees is which resources appear in
a user's catalog**, and that a connection was made. **One interface still needs
stability and it is not a network one**: a package must run on any conforming
node, so the node-to-resource contract must mean the same thing everywhere — that
is **portability, not interoperability**, binding a node to the packages it hosts
rather than one node to another.

### 2026-08-23 (portability scoped to legibility)

*A package must run on any
conforming node* was too strong. **Legible everywhere is not runnable
everywhere**: a package may need storage, compute, a hardware capability, a
persistent address, or something of the requesting client's device, and a node or
device that cannot meet those requirements **cannot host or use it while remaining
fully conforming**. Two distinct failures now stated: **not understanding the
credential contract is a conformance defect**; **not meeting a package's
requirements is ordinary capacity** and says nothing about either party. What must
hold is that the node-to-resource contract is **legible to any implementation of
either client type**, so a package reads the same credential wherever it does run.
A package should declare its requirements in its manifest so a mismatch is visible
before installation rather than at first request — packaging hygiene, not
something the protocol enforces.

### 2026-08-23 (0.6 catalog registration, first run)

**The deepest finding is
not a catalog problem.** §15 names *flood-within-horizon* and *push near, redirect
far*, and those patterns are relied on by rotation, catalog entries, peering and
the locator gap — while **no message carries any of them**. Nothing says what is
sent, on which stream, how a receiver decides to forward, or how a flood
terminates in a horizon that contains cycles once peering exists. Recorded at
`wire-format.md` §10.1 and as a §23.2 blocker, with the diagnosis: **§15 settles
the policy — which class reaches how far — and a policy is not a protocol.** No
target exercised propagation until this one. It blocks the catalog subsystem and
subsumes most of the locator gap; it blocks nothing two parties do directly.
**`CatalogEntry` had no lifecycle**: no sequence, no withdrawal, no rule for
competing registrations. Now `seqno` and an operation field — **an entry is
superseded, never edited**, and **a withdrawal is an entry rather than a
deletion**, because a deletion travelling as an absence cannot be distinguished
from a message that never arrived. A withdrawal does not expire, which stops an
old registration resurfacing from a node that was offline when it passed. **And a
stale rule was corrected**: the wire format still had a decoder *reject* scopes it
could not evaluate. Rejection makes structural validity depend on the reader's
topology, so a node would refuse an object its neighbour accepts — such a scope is
**valid and ineffective**: store it, forward it, grant nothing from it.

### 2026-08-23 (catalog is answered, not propagated)

The catalog is a **query
response**: a node asks the infra nodes in its horizon what they have, and each
returns the entries it owns and the asker may see. **That removes most of what the
previous entry added.** The `seqno` and withdraw operation are withdrawn — an
owner that has withdrawn a resource simply stops returning it, there are no copies
to invalidate, and freshness is inherent rather than maintained. **`discover_scope`
leaves the wire entirely**: it is the owner's local rule for which entries go to
which asker, evaluated where the answer is composed, and **receiving an entry is
what qualifying looks like** — a field carrying it would tell the asker how they
were selected. Same shape as the role table. `connect_scope` stays and is
**advisory**, letting a client present likely-usable entries differently while the
owner decides at request time regardless. **The topology-propagation blocker no
longer covers the catalog**, only rotation's push-within-horizon and the locator
gap. And §11.5's enforcement problem dissolves rather than being solved: with no
relay, **the owner is the only party that ever discloses an entry**.

### 2026-08-23 (0.6 catalog registration, second run)

Eighteen questions, and
the two predicted gaps were both real. **The query had no message** — the catalog
became answered-on-request and nothing defined the asking. Now `CatalogQuery` and
`CatalogReply`, with an optional service-type filter and **no pagination**: an
asker narrows by type instead, since a cursor would be state the answering node
holds for a party it owes nothing. Result order is explicitly insignificant, since
a node answering from a map has none to offer. **An entry is signed once at
registration and that signature is reused**, because no field varies per query and
a per-answer signature would make an entry's bytes differ between askers, defeating
attributability. **Re-registration replaces the current entry** — one per resource,
with the archive keeping both transactions: **the archive is history, the catalog
is state.** The infra client must append and replace together, since a crash
between them leaves either a transaction with no queryable entry or an entry whose
transaction is missing. **And the §11.1 topology gap is worse than recorded**: a
resource request needs a session with one non-attached node, a catalog query needs
one with every infra node in the asker's horizon.

### 2026-08-23 (catalog view is swept and cached)

The concurrent-session problem
dissolves. A client **builds its view by sweeping the horizon once** — on joining a
subnet, periodically, after a failed connection, and on demand — **holding one
session at a time and closing it**. Between sweeps it reads a cached view and opens
nothing, so browsing costs no sessions and the topology gap reduces to the
resource-request case. **Staleness is acceptable for a reason particular to this
network**: subnet members are people who have met, so a new resource arrives with a
conversation attached, and **a user going to look for the thing someone mentioned
is the expected path** rather than a convergence failure. A refresh control is a
better answer than keeping hundreds of views in sync for an event that is socially
announced anyway.

### 2026-08-23 (0.6 catalog registration, third run)

Nineteen questions, **two
of them contradictions from my own edits.** `resource-requirements.md` §6 still
said entries *propagate as topology* after the catalog became query-answered, and
`wire-format.md` §3.5 said a nonempty unprotected COSE header was both **malformed
and ignored** — five lines apart, in text written the same day. **A real dead end
in the truncation design**: an asker told to *narrow by service type* had no way to
learn which types exist, so a catalog over 64 entries was undrainable. The reply
now carries a **continuation type** the answering node picks from what it withheld
— a hint rather than a cursor, so the node keeps no state. **Custody and authorship
separated for hosted resources**: a light-client owner cannot answer queries, so
the **hosting node answers and the owner signs** — with two consequences stated,
that delegating hosting delegates the discovery filtering, and that a resource
whose host stops hosting becomes unfindable while remaining validly registered.
**Concurrent re-registration is settled by the answering node**, since nothing
timestamps two registrations authoritatively and nobody else holds a copy to
reconcile. Field bounds set, and **service type is matched byte-for-byte** — no
case folding, no Unicode normalisation — with instance name display-only.

### 2026-08-23 (host loss and resource failover)

Loss of a hosting node is **not
a silent failure**: there is no registry outside the host and the asker's cache, so
a client that cannot reach a node knows that node's contribution to its view is
stale, arriving as the same signal it uses for any outage. **Failover is a
per-resource question with three different answers and the protocol specifies
none.** Local services disappear with their owner's node and should — replication
to siblings buys little for the complexity. **Network-native services will be
designed for multiple ingress points**, and the observation that makes this cheap
is that a widely-used resource **needs an owner every few tiers along each branch
anyway** or it falls outside some members' horizons: reach and redundancy are the
same problem, so placing owners more densely gives alternate paths as a side
effect. Density is a deployment decision driven by use profile. **Gateway-fronted
external services mostly will not do that** — a second gateway may carry a licence
cost or the application may assume one instance — and that is the position a
corporate intranet deployment of the same product already occupies, times the
reliability of whoever hosts the gateway.

### 2026-08-23 (§8.2.1 withdrawn — not a gap)

The "topology gap" was a
misreading of what attachment is for. **Messaging already opens sessions to
non-attached nodes**: §12.6.3's relayed path runs client → own serving node →
**recipient's serving node** → recipient, and its direct path has a client reach a
peer outside its own subtree. Infra nodes hold static addresses and authenticate by
keyhash, so opening a session to one is the same operation wherever it sits.
**Attachment answers one question — where do my messages queue — and is singular
because a mailbox must have one address.** It says nothing about which nodes a
client may connect to, and §14.1.2 now says so explicitly. Both halves of the
supposed tension were true and never in conflict. §23.2's subsystem blockers drop
from five to four.

### 2026-08-23 (§8.2.1 closed, not answered)

Reaching an infra node you are not
attached to **was never a gap**. §12.6.3 already specifies direct connection to any
peer inside the horizon — bounded, and privacy-analysed under P17 — and **a hosting
node is a horizon peer**, so a resource request is that operation with an infra node
as the peer instead of a light client. A catalog sweep is the same, one node at a
time. **Attachment is about inbound service** — queue, currency, sibling list — and
never was a restriction on outbound connections; reading it as one is what made this
look unresolved. The privacy position is unchanged, since IP disclosure to a horizon
peer is incremental rather than novel, which is the argument §12.6.3 makes for
drawing the boundary there. §23.2's resource entry rewritten to separate what three
runs settled from what remains: the `connect`/`discover` relationship to roles, and
a second attempt against the frames as drafted.

### 2026-08-23 (reserved actions are node-consumed)

`discover` and `connect` are
listed in §11.4 as *actions* alongside application actions like `read` and `write`,
and the credential passes roles to the resource — so an implementer reading it
straight forwards them, and a resource ends up holding a role it cannot act on.
**Neither is ever forwarded.** Both are consumed by the owner's node when it
evaluates its role table, and a resource's role set carries **application actions
only**. The reason is that neither is actionable downstream: `discover` is
evaluated when composing a catalog answer, before any request exists, and **a
request arriving at the resource is what a `connect` grant looks like** — restating
it would tell the resource what the delivery already told it. Recorded in §11.4 and
in the resource requirements, where a package is told not to expect them and not to
read their absence as a missing grant. **§23.2's resource entry now has one item
left**: a second implementation attempt against the frames as drafted.

### 2026-08-23 (0.6 target list reduced to prompt text)

The seven targets carried
commentary on what each exercises and where the pressure lies, which was written
for a reader rather than for the prompt it is pasted into — and had gone stale
besides: target 6 still said *propagate its catalog entry* and named
`discover_scope`, both withdrawn. Each target is now a single sentence naming the
operation. The rationale that was embedded in them moves nowhere, since the passes
it described have run; the coverage note is corrected to say that **a layer nobody
has built against produces a construction change rather than encoding corrections**
— which targets 5, 6 and 7 each did on their first run, and each needed a second.

### 2026-08-23 (0.6 catalog registration, fourth run)

Eleven questions, and
**the propagation contradiction appeared for a third time**: §15's message-class
table listed resource registration under *topology*, and
`resource-requirements.md` §6's comparison table said *propagates as topology,
horizon-limited*. Two earlier passes each fixed the instance they found. Both
corrected. **The largest finding is not a catalog problem**: resolution, archive
fetch, prekey fetch, verifier queries, catalog queries and resource requests all
share one ALPN and one stream class, and **nothing told a receiver which it had** —
structural guessing across six schemas. Bidirectional streams now open with a
**request-type tag**, framed as stream 0 is, with unknown types **rejected** rather
than skipped: a control frame arrives on a shared stream where skipping preserves
the session, while a bidirectional stream *is* the request. **A termination bug in
my own truncation design**: an answering node choosing any 64 could return the same
64 and the same continuation hint indefinitely, so an asker following it would loop
rather than drain. Entries are now ordered by resource keyhash — arbitrary, but
stable. Also settled: an out-of-horizon asker is **refused before the application
reply**, since an empty reply is a true statement and the wrong one; the entry
signature covers the entry alone and not the enclosing transaction's key 0, because
the same bytes are returned in replies where no transaction exists; **two owners may
register the same resource keyhash**, since it is not a namespace anyone allocates;
and the unsigned query and reply **reject** unknown keys, because preservation
exists to protect signature verification and neither is signed.

### 2026-08-23 (catalog bounds completed)

Two bounds from the fourth run were
not applied with the rest. `CatalogQuery`'s service-type filter had no bound and
now carries the same one as the field it matches — **a filter longer than any legal
type cannot match and should not be allocated for**. And **unknown extension keys
had no bound anywhere**: §1 requires them to survive re-serialisation, which makes
them attacker-supplied storage on a signed object a node retains. Sixteen per map,
1 KB per value, with a total encoded bound on `CatalogEntry`. **Extension tolerance
is not unbounded tolerance**, and a rule admitting arbitrary bytes into retained
state is a denial-of-service surface however well-intentioned.

### 2026-08-23 (0.6 hosted resource authorisation, second run)

Eighteen
questions against the rebuilt layer. **One normative branch was impossible as
written**: the evaluation order put membership first, and membership is
**owner-relative** while an unknown resource keyhash **has no owner** — so step 1
could not run for the case step 2 was meant to catch. Reordered: existence on this
node first, returning **code 2 rather than code 1**, since code 1 to a non-member
is exactly what the split was for. **Code 1 is reachable only once the asker is
known to be a member**, and a member asking for a keyhash the host does not have
still gets code 2 — the host cannot tell whether it exists elsewhere. **The opaque
payload had no format**: now an HTTP/1.1 message in both directions, still opaque
to the network but **fixed**, because the node must hand the resource something it
can read. **And 64 KB could not carry an ordinary response** — field 1 = 0 may now
repeat on the stream, each frame a portion of one message, with no length declared
up front since a streaming resource does not know one. Also settled: authorisation
state is read **once per request** — four reads at four instants can describe a
state that never existed — and an in-flight request **completes under the state it
started with**, since terminating a session stops the next request rather than
reaching into one whose result the resource may already have committed.

### 2026-08-23 (multi-identity reframed as client scope)

*v1's single identity
key across subnets* was recorded as an architectural limitation and kept surfacing
in privacy sweeps as one. **It is not.** Nothing in the wire format or topology
binds a device to one key — an identity *is* a key, adoptions are per-identity, and
a client holding several is running several identities as far as the network can
tell. **What v1 defers is the client work**: key management and the interface for
choosing between them. §4 already listed it as a client-scope exclusion; §13.7
already said it was a client concern; and the registers had nonetheless inherited
the reading that a protocol change was needed. **P3 and C10 are now stated as
conditional on the client**: high for a single-identity client, absent for a
multi-identity one, **with no wire change separating the two cases** — so P3 no
longer ships as a known defect, because the design does not have the defect.

### 2026-08-23 (0.7 LINDDUN, fourth run)

**One live contradiction**: §7.4 says
the anti-oracle aggregate is a lock retained for the ceremony window, and thirteen
paragraphs later instructed a client to *log every query and make it visible to the
subject*. Removing the log left the instruction that produces one. Restated as
**notification, not a log** — the subject sees each query as it arrives, and what
persists is the counter and nothing else. Following the second reading would have
rebuilt the timeline C12 and P22 were closed by removing. **Four records had no
retention rule and two now do**: a `LateResponse` **follows the record it
supplements**, since saying no rule applies left it outliving the thing it
describes; a `SubtreeAck` is **discarded when it lapses**, because a retained
lapsed acknowledgement is a durable statement that two parties were once connected,
which is what departure is supposed to end. **P32** records the client-side caches
that still lack lifetimes — **a cache with no expiry is a retention decision made
by omission** — and **P33** records that multi-device replication is unspecified, so
retention and deletion commitments cannot be assessed at all: a deletion on one
device says nothing about the others.

### 2026-08-23 (policy descriptor removed entirely)

**Reverses an earlier
decision to keep it as a declared limitation.** The intermediate form carried *bad
news only* — a node may declare its policy fails the soundness condition, silence
meaning nothing — on the reasoning that a deliberately crippled mechanism documents
its own reasoning by existing. **It was still generating attack surface in review**,
with adversaries reasoning about shopping across published policies, for
documentation value an Appendix A entry provides at no cost. Removed. §16.2 now
states plainly that **nothing publishes a policy and nothing should**, since
per-observer trust has no consumer for one. **§16.4's shopping attack survives and
is restated**: an attacker cannot *read* a policy, but can **try** — present the
fake region and see who accepts it — which is slower and noisier than reading a
declaration, is the reason no declaration exists, and is not prevented. The
durable part is the asymmetry: the attacker chooses how many evaluators to
approach, and no evaluator chooses which attacker approaches them. P10's tombstone
now reads *closed by removal* rather than *by redesign*.

### 2026-08-23 (0.8 restored verbatim)

The section had been rewritten to run all
nine adversaries in one pass with a complexity-warning preamble. A later run in
that form was blocked by the provider's cybersecurity classifier. **The section is
restored exactly as it was**, including its section numbers, which predate several
renumbers and no longer resolve against the current documents. **That is
deliberate**: it is the text that ran, and it is preserved as such. Anything else
is a change, and changes to a known-good prompt belong in a separate decision.

### 2026-08-23 (0.9 organisation, second run)

**The reference checker had been
reporting clean while nineteen references were broken.** Three failure modes it
could not see: an exemption for §22.x, added when those citations pointed at
numbered list items, hid two that pointed at nothing after the list was renumbered;
**§20.1's assumption table carries addresses as bare numbers in a column**, not
§-prefixed, so thirteen stale ones — `6.5.1`, `6.5.4`, `7.6.5`, `7.9.1` and others
predating the §6 split — were never examined; and a duplicated heading,
`#### 7.2.2 Verifier selection#### 7.2.2 Verifier selection`, parsed as a valid
heading and so passed. All corrected, and the checker now runs with **no
exemptions**. **A structural fix alongside them**: the catalog, `CatalogEntry`,
query and reply, and `AbuseReport` — 1,856 words — sat between `## 4` and
`### 4.1` under no heading at all, while every reference to them pointed at §5.4,
which is the withdrawn activity summary. Now §3.7, with references repointed.
Citation labels normalised: `design design §`, `Design §` and
`` `infra-client-requirements.md` design § `` all appeared.

## 2026-08-24

### 2026-08-24 (0.8 hostile client implementer)

The pass ran after the reviewer
accepted it as a defensive review of the author's own specification, with sensitive
detail redacted at the author's request. **Thirteen findings beyond the registers,
and the first is critical.** The old-key recovery proof signed *the Recovery map
with field 3 omitted* — and when field 2 is absent, that payload is **a map
containing nothing**. The old key attested that a rotation occurred and **named no
successor**, so one observed proof authorised unlimited competing successors:
extract it, build a fresh adoption naming the same `prior_key` with an
attacker-controlled key and patron, insert it unchanged, and the envelope
signatures authenticate the assembly while the old key's statement contains nothing
to contradict it. **Replaced with a `SuccessorStatement`** binding `prior_key`,
`new_key` and `patron_key`, which a verifier must check against adoption fields 1
and 2. The circularity that motivated the original payload is avoided by naming the
fields directly rather than by signing an emptied map. **A REASONING finding
corrected §5.1's justification.** Embedded evidence stays classical because its
relevance expires before the post-quantum horizon — true, except **where the
decision it induces is permanent**. A patron's hybrid signature preserves *that the
patron decided*, not *that the evidence was true*, so a forged classical `match`
would sit sealed inside an authentic post-quantum record. **Verifier responses
inside a `Recovery` block are now hybrid**; everywhere else the rule stands,
because everywhere else a later reader can revisit the weight they give something.
Only an identity replacement cannot be undone.

### 2026-08-24 (0.8 findings 2–5 applied)

**Formation records no longer count
toward *n* or the candidate set**, reversing a decision from an earlier 0.6 run
that they count toward both because their lack of corroboration is a weight
question rather than a structural one. **Candidacy is structural.** A formation
record needs no witnesses and no verifiers, so a thousand cost one keypair and two
signatures each — and deterministic selection, which exists so a subject cannot
choose their own verifiers, then samples almost entirely from the attacker's set.
Typing them as weaker evidence governs weight, not eligibility: the defence and the
attack act on different quantities. **The node parses HTTP and re-serialises it**,
where the wire format had said it relays bytes opaquely *and* strips `rhtn-*`
headers — two different components, with the gap between them being the classic
multi-parser boundary hazard. New §11.2 requires parse, reject-don't-normalise,
and exactly one message per request. **`started_at` MUST be monotonic against the
committed back-pointer**: it determines the 730-day window and therefore *n*, so a
backdated ceremony yielded a required verifier count of **zero**, and varying the
claimed day yielded fresh samples without waiting. The back-pointer is inside the
signature and names a record with its own `finalized_at`, which bounds a subject
against their own history rather than against anyone's clock. **P34 registers what
is not closed**: a subject who forks their archive and keeps a branch dormant
presents the sparse branch and obtains a low threshold, because the evaluator
learns *n* from the subject by design. Both obvious fixes break something
load-bearing, so it is recorded as a design decision rather than a rule.

### 2026-08-24 (P34 withdrawn: partitioned identity is the design)

The
archive-fork finding treated *n* as a global reputation figure a subject could
understate. **It is not.** *n* is the history a subject has **in the domain they
are presenting to**, and a sparse branch yields a low threshold *and*
correspondingly low standing — the two move together, so understating gains
nothing. A subject appearing with no history is a stranger, which is what they are
there. **§16.1 now states the property rather than leaving it implicit**:
credibility is constructive, built by engaging in a domain, and what is at stake in
an interaction is what was built there. There is no universal permanent record and
no cross-domain enforcement. Forking an archive **divides what can be claimed
rather than hiding it**. Withdrawn from the register and from §23.2's blocker list.

### 2026-08-24 (formation flooding: the real bound, and a framing correction)

The candidate-exclusion fix applied an hour earlier is **withdrawn**; it addressed
an attack the design already prevents, at the cost of removing honest bootstrap
meetings from the history they belong to. **The actual bound was never written
down**: a key may appear in **at most one formation record, its first**, because a
formation record's back-pointers are absent and an established key's key 0 entry
names a predecessor. Without that stated, an identity with standing appeared able
to manufacture formation records against arbitrary fresh keys; **with it, an
attacker's thousand keys can only form with each other**, and mutually formed
strangers are candidates for nobody. Now stated in `wire-format.md` §4.5.
**The framing correction reaches further.** Both the finding and my fix treated
history as accumulated credibility that volume could inflate. **An archive answers
specific questions rather than supplying a total** — *"you say you were active in
the Portland chapter; prove it"* — and the valuable evaluation is discontinuous and
local, an evaluator looking for interactions with people they recognise. **Volume
proves nothing, so manufacturing volume gains nothing.** §16.1 now says so, and
distinguishes the protocol's sampling floor from the evaluation, which is the
adopter's and lies outside this specification.

### 2026-08-24 (horizons are scopes, not shared regions)

§15.1 described the ±2
tier horizon in terms that read as a shared region, and a reader — including a
reviewer — naturally infers that parties inside it share a view. **They do not.**
Each node's horizon is centred on itself, so no two nodes at different positions
have the same one, and *"inside the horizon"* always means *inside mine*.
**Siblings are the case that shows the distinction**: they occupy the same
position, so their scopes coincide exactly, **and their trust pictures still
differ** because each has its own history with users outside the subtree. Same
scope, different content. **A node's total trust picture is unique to it**, and
there is no tree-level trust state for a node's view to be a view *of*.

### 2026-08-24 (consistency pass across all files, working documents included)

Scanned all twelve files for the defect classes that have recurred: doubled
citation labels, malformed decision markers, malformed ranges, lowercase sentence
starts from the em-dash substitution, `STATUS see` without punctuation, unclosed
code fences. **Most hits were false positives** — schema comments, sentence-initial
*Design*, a malformed marker quoted inside a change-log entry describing its own
repair. **Four real defects**: a range reading `§17.2–12.3`, a doubled
`design design §11.2` in the resource requirements, and two sentence-initial
*Design §* that read as citation labels. **§23.2's trailing note miscounted again**
after P34's withdrawal removed a heading — the third time that sentence has drifted,
which is what the note about consolidating sections in `CLAUDE.md` is drawn from.
All references resolve in every direction; assumption, correlation and privacy
registers have no gaps at 31, 18 and 34 entries; the unset-parameter count matches
its claim. Trailing newlines normalised across the set.

### 2026-08-24 (history ingestion, from the original)

Recovered from the
author's fifth message in the project, and absent from every document since:
*"Upon joining a new tree, that archive can be scanned by the new patron and
compared to the new tree's local trust table. Transactions with unknown
counter-parties can be ignored, but those where the counter-party is known can
contribute to the users initial trust-state in the new tree."*

**It is the structural answer to fabricated history.** A manufactured counterparty
is unknown to the patron by construction, so records naming it contribute nothing
rather than contributing weakly — an attacker cannot add to the intersection
without compromising someone already trusted, at which point the fabrication is not
what bought the access. Volume is irrelevant: a thousand invented meetings and none
intersect. **It also answers how a joining member comes to have any standing** in a
subnet where nobody has met them.

**Stated permissively, as the original does.** *Can be ignored*, *can contribute* —
how a patron weighs an archive is local policy (§1.1), and writing it as a MUST
would claim an enforcement the design does not have. **A first restoration wrote it
as a rule** and also lost the framing it arrived in: **history portability as an
exit right.** Transactions are self-signed and countersigned, so a node's history
stays independently verifiable after it moves — leaving costs proximity, not
evidence — and the intersection rule is what makes a portable archive worth
carrying. §10.1 now says so.

### 2026-08-24 ("local trust table" withdrawn)

The phrase appeared in the
original statement of archive ingestion and the author has withdrawn it as
incorrect. **Trust is node-local, and describing it as a tree's table implies a
shared structure that does not exist** — a patron compares a presented archive
against the identities *it* already knows of, and no two nodes hold the same set.
§15.1 no longer describes a node's trust picture as its view of anything; there is
nothing for it to be a view of. Corrected in §15.1, §16.1, §10.1 and the infra
requirements. The change-log quotations of the original message are left as the
author wrote them.

## 2026-08-25

### 2026-08-25 (topology propagation, the rootward memo, and §23.2 rebuilt)

The
largest of the remaining §23.2 blockers closed together, because they turned out to
share a mechanism.

**Topology propagation acquired an encoding** (`wire-format.md` §10.1). Control frame
type 7 on stream 0, carrying the signed envelope byte-for-byte with no wrapper.
Stream 0 rather than a bidirectional stream because an unknown control frame is
*skipped* and the session survives, which is the right outcome for gossip, where an
unknown request type on a bidirectional stream is *rejected*. `SiblingUpdate` was
the precedent nobody had looked at: an unsolicited, server-initiated frame.
**Forwarding is "forward if and only if you stored it"** — reach is a consequence of
each node's own storage policy, with no hop count, TTL or reach field, because a
counter would encode the sender's horizon and impose it on receivers (§15.1) and
because nothing verifies that an intermediary decremented it. Duplicate suppression
is by `txid` against the store the node already keeps. **No acknowledgement and no
retry**: §2.1's back-pointers make a gap self-announcing at the receiver, §7.9
fetches what is missing, and periodic reconciliation with siblings and patron is a
replay of the same frames rather than a separate mechanism.

**The rootward memo is new** (design §15.2, `wire-format.md` §10.2). Frame type 8. A
minified record of every membership change — subject, patron's position, added or
removed, subject's `seqno` — travels up the patron chain to its subnet's root. This
is what §15's *ancestors* reach had always meant and never said. **Peering is
excluded from rootward travel, and the exclusion is load-bearing**: a peering record
carries both endpoints' network points plus ASN and prefix, which C8 maps to a
natural person, and a memo carrying no address is what makes a root's accumulated
view tolerable.

**Cycle prevention closed on the back of it** (§6.2.5). The general procedure looked
impossible because detection appeared to need topology beyond one's horizon. It does
not: a memo carries the path it travelled, so **a node that finds its own position on
that path is its own ancestor**. The check is exact, needs no stored state and works
at any depth. A memo is a *hint* — unsigned and derived — so action requires fetching
the underlying signed transaction, which preserves this section's ordering of failure
modes: an undetected cycle is a local correctness problem, a false positive is
indistinguishable from censorship. A detecting node disavows the direct subordinate
that forwarded the memo, under new reason code 5, without prejudice.

**`SignedLocator` does not bind identity to endpoints**, and three sections said it
did. A `Locator` is `{anchor, path, seqno}` — a position. The infra-child endpoint
gap was also narrower than stated, since a **peering** record already carries both
endpoints' addresses; what had no carrier was an infra node that neither peers nor
serves as an anchor, a supported degraded state. Closed by a **node endpoint record**
(`wire-format.md` §7.6) modelled on `AnchorEntry`, self-signed for the same reason
§5.3 gives — retroactive attribution of forged gossip, which is worth having for a
flooded object and not for a referral from the one party you are already talking to.

**Queue policy settled** (§14.1.6). At the ceiling: refuse the newest and tell the
sender, never drop the oldest — chosen on the adversarial case, since drop-oldest
lets anyone who can reach a queue flush what is in it. No copy outlives delivery;
crash recovery is the operator's backup problem and duplicating it in the protocol
buys durability against a smaller failure than the sibling decision already accepted.
Metadata bounded to ciphertext, recipient keyhash and arrival time. Only the cap
*value* remains, and §21.1.1 already classified that as freely tunable.

**Owner movement was already written, in the wrong document.**
`resource-requirements.md` §7.1 and §7.1 carried the general rule — access is a
predicate evaluated at request time, so there is no grant object for a move to
invalidate — while design §22 and §23.2 recorded it as unwritten. Relocated to §11.2
per Appendix A's rule that requirements documents carry no protocol facts.

**`CatalogEntry` gains an optional `data_practice` declaration** (§11.5), which is
§1.1's move where enforcement is unavailable: make the distinction visible and let
policy weight it, as client-integrity attributes already do. Its reach is bounded by
the catalog's — you learn a resource's posture if and when you already have access
and think to ask — which is stated rather than left to be discovered. **P20 is not
closed by it**; the field gives visibility, not a limit.

**Abuse reports are standalone**, not bound to a live session (§11.6). Binding would
make a report unfileable after the session ended, which is when most are filed.

**What closing all of that cost**, recorded because it is the part that gets lost:
two new privacy findings — **P35**, an ancestor accumulating a key→position index for
its subtree, and **P36**, `seqno` gaps disclosing out-of-subnet activity — one new
correlation entry **C19** joining them, an amended product property at **§12.4**
(*you cannot search for a person, except downward within your own subtree*), and a
withdrawal whose reasoning had to be restated: **P8** rested partly on *the attacker
is already a horizon member*, which the memo made false for ancestors. The conclusion
survives on its other leg — what labels a subtree is its catalog, and the catalog is
answered on request within horizon — and P15 carried the same faulty clause and was
corrected with it.

**Scope accepted by the author:** a subnet-bounded key→position index is acceptable
because *joining one subnet rather than another is a choice to be in some sense
visible to that subnet* — a company, a club, a party. The property defended is that
it never crosses a subnet boundary, which §3.1.1 guarantees by construction.

### 2026-08-25 (defects repaired during the same round)

Found by sweep rather than
by review. `§8.1.2 Verifier selection` was physically located inside §10.4, between the
archive's open items and §11, and is moved under §8.1 where it belongs; six references
had been landing readers in the wrong chapter. `wire-format.md`'s §8.2.2 sat after §11
and is moved back under §8.2. **P17's table row was truncated mid-word** with its tail
orphaned below the table as body text; rejoined. **§19.8's table header was
corrupted**, a stray footnote occupying the `#` cell; restored. §21.1's *needs an
encoding decision* group stood over an empty table and now says none remain. §15.1's
horizon-jobs table still listed *catalog propagation range* after the 2026-08-23
decision that the catalog never propagates; corrected to *query range*, **topology
forwarding reach** added, and the count corrected from seven to eight. §11.5's
"(§11.5 below)" pointed at its own section, and its claim that `discover_scope` is
"still required for entries that propagate" named a class that no longer exists.
§10.4's two remaining entries were both already answered — one contradicted P14
outright — and the section is retitled *Closed*. §24 cited `wire-format.md` §10,
which does not exist, and an unbalanced parenthesis in build step 12 is closed. The
parameter count corrected from sixteen to fourteen as the queue row collapsed.

### 2026-08-25 (retrospective material moved out of the human-facing documents)

At the author's direction. **The complaint:** an idea, once raised, was preserved
even after being decided closed — so a reader could not tell what state the document
was describing. The disavowal notice period was the example: proposed by the
assistant, never adopted, and closed in some mentions but not others, ending up with
a wire field and a parameter row while the design still called it an open question.

**The rule applied.** A sentence stays if it describes the current design, why it is
that way, why an expected alternative is absent, or an accepted risk. It moves to the
logs if its subject is a **past state of the documents**.

**What moved.** Thirty-two in-line retrospective notes — *"an earlier version
said…"*, *"this section previously…"* — now zero across all five human-facing
documents. **Sixteen were converted** to forward-facing statements because the
rationale existed only there; **ten were deleted** because the rule they trailed
already stood alone; the remainder disappeared inside larger restatements. Converted
example:
*"an earlier version had it sign the Recovery map with field 3 omitted, which is a
replay primitive"* became *"signing the Recovery map with field 3 omitted **would
be** a replay primitive, which is why it does not"* — same warning, no history.

**Sections removed as tombstone lists.** Design §10.4, whose three entries were all
resolved; §11.8's closed entries, leaving only the gateway-evaluation item; and the
"Closed in the 2026-08-25 round" table added to §23.2 earlier the same day, which was
a change-log entry in a specification. §9.4 was titled *Open* while stating an
answer and is retitled to what it says. `wire-format.md` §7.4 and §5.5 kept their
numbers — removing them would shift §5.6 — and shrank from 39 lines to 20, pointing
at design §9.2 for reasoning they had been duplicating.

**Twelve withdrawn register rows moved to `review-tracking.md`** — P7, P8, P9, P10,
P22, P34, N3, C12, C13, C14, C16, C18. **The tombstone rule is unchanged**: numbers
are never reused and a citation to a withdrawn one still resolves. It resolves in the
tracker rather than the design, because a withdrawn finding is not a current risk and
§19.4 is the design's statement of current risk.

**What was deliberately kept.** `wire-format.md`'s field- and type-number tombstones,
which are normative — an implementer must not reuse a withdrawn number — reduced to
one-line comments without their explanatory paragraphs. And every *why not* that a
reader might otherwise re-propose: no activity summary, no veto delegation, no notice
period, no cold lookup. Those are not history; they are the third thing the design is
for.

### 2026-08-25 (participant locator removed; selective disclosure adopted; abuse-report reporter removed)

Three changes from one question: what does a presence record
actually need to carry, to whom.

**`Participant.locator` is gone.** It recorded *position at meeting time*. A sweep of
the eleven exchanges that transmit or evaluate a presence record found **nothing that
reads it**, and it was stale by construction — any later locator supersedes it under
§4.3's strictly-greater rule and nothing resolves against an old one. It was also the
field that made §19.1's worked example work. **Deleting it is a better answer than
making it withholdable**, and costs nothing rather than 0.4%. What it removes is the
*historical* position series, so records no longer trace a trajectory; it does not
remove position inference, since the keyhashes remain and the witness set still
discloses a neighbourhood. The worked example is rebuilt on the witness set and says
so.

**Selective disclosure is specified**, at design §8.1.1 and `wire-format.md` §4.5.1,
after fourteen days as a proposal nobody had costed. **Scope came from the sweep, not
from preference**: location evidence, retention tiers, client integrity, capture
parameters, proximity channels, `started_at` and subtype leave the body and are
committed as salted digests; **ten of eleven exchanges read none of them.**
`wire-format.md` §4.5.2 and design §8.1.1 both state the per-exchange visibility, and
each consuming section now says what it sees.

**A flat digest list, not a Merkle tree.** §19.3 proposed a tree, following SD-JWT
loosely; SD-JWT's actual construction is a digest array, and at nine leaves a tree
buys nothing while adding odd-node handling and the duplicated-node second-preimage
class. Domain-separating prefixes are still required so a digest cannot be
reinterpreted as a root. **Salts are mandatory** — without them `subtype`, `liveness`
and a precision-3 geohash are brute-forced straight out of their digests.

**The envelope is untouched.** Disclosable fields travel *beside* the body rather
than inside it, and the body carries a 32-byte root, so `txid` and the COSE payload
keep their existing definitions. An earlier framing that made the body itself a tree
would have changed both for one transaction type.

**What it cannot do, recorded because the proposal implied otherwise.** `kid` sits in
the COSE protected header, outside the body, so **every participant and witness
keyhash is on the record however little the body discloses** — and §2 infers signer
role by comparing `kid` to body fields, so withholding those lists would break
inference outright. **P2 and C2 are untouched.** §19.3's claim to be "the only lever
that addresses composition directly" was wrong in both halves and is corrected.

**And the trade it actually offers.** §7.7's impossible-travel check and P21's
behavioural-location leak are **the same computation over the same series**. Field-level
disclosure cannot separate them; it separates audiences. The one recipient with a use
for location — a prospective patron reading an archive prefix — is the same party P19
and C4 flag as the dangerous holder. Everyone else stops receiving it. Stated plainly
in §8.1.1 rather than left for a reviewer.

**Registers.** P1 reduced to Medium, P21 mitigated, C1 reduced to Medium–High, P19
restated to say this does not help it. **Over-asking was raised as a hazard and
withdrawn**: §1.1 makes the evidence schema the one non-pluggable thing, so a
recipient weights what it receives and cannot make an interface carry what the
interface does not define.

**`AbuseReport.reporter` removed.** With the resource as sender it named the same
party as `resource`. The rule that keeps the object honest is that **the signing key's
keyhash MUST equal `resource`**, checkable from the object alone. There is no
user-signed abuse report: producing one would require a user's network client to
interoperate with arbitrary third-party applications, blurring the boundary the
resource layer exists to keep. An application naming which of its own users complained
puts that in `detail`, as application data. C16 stays withdrawn on firmer ground and
P27 reduces again.

### 2026-08-25 (aggressive minification of the specification documents)

At the
author's direction: review ingestion was drowning in descriptive residue, and the
chosen posture is *describe the minimum active feature set, repair any
erroneously-deleted justification in the author's own voice later*. Normative
content untouched; everything cut is recoverable from git and this log.

**Cut from `network-design.md`** (~6,600 → ~5,900 lines): all eight vignettes and
their cross-reference map; every date qualifier in decision markers (`[D — date]` →
`[D]`, 79 across both documents); Appendix A rewritten to the three conventions without
their origin stories; the preface's dated status paragraph and working-context
note; review-pass narration in §19, §21.1, §21.1.1 and §23 ("a re-run on
2026-08-16 found…", "pass 0.4 found 30…"); the locator-removal and
worked-example provenance notes from the previous round; **Appendix A.2 (parameter
conflicts) and A.3 (questions closed during design) deleted** — history, already
duplicated here — with A.1 kept, the orphaned trust-policy row folded into its
table, and the 110/1,110/1,000 disambiguation kept as the new A.2. Stale
references that had pointed at the old A.2/A.3 redirected to §20.2, §10.3 and this
file (several had been silently resolving to the wrong content since the
assumptions register moved into §20.2).

**Cut from `wire-format.md`** (~3,000 → ~2,880): "previously unspecified"
paragraphs; §11's resolved-items history; field tombstones shrunk to one-line
"key N unused, not reused" comments — **numbering itself unchanged**, since
renumbering is a normative change reserved for a publication pass.

**`authoring-conventions.md`**: the vignette convention removed with the vignettes.

**Kept deliberately**: §17's findings and correlation registers (current risk),
§20 (assumptions), A.1 (rejected alternatives — the anti-re-proposal register),
§22/§23 (open items), and every forward-stated "why not".

### 2026-08-25 (0.8's adversary roles restored; a verbatim prompt repaired after being edited)

Two corrections to the review plan, both about Appendix A.

**The six adversary roles were missing** and the author recovered them from an
early download: a participant's own patron; a witness at a ceremony; a funded
commercial operator at $100k/month; a state actor with legal compulsion over one
cloud provider; a malicious counterparty at a single meeting; a device thief
holding keys and archive. **They were absent before version control began**, so
the loss is not in the history — the likely moment is the 2026-08-23 restoration,
which recovered the section's prose and prompt but not the table the prompt
selects from. The section had been self-contradicting since: the prompt says
*[ONE ROLE FROM THE TABLE]* and the rubric assumes six constructions, with no
table in the file. All six map onto existing findings — §18's eclipse, §17.1's
fake-subtree cost, C8, P29, P5/C9 — which is what corroborated the recovery.

**And an edit to the prompt was reverted.** The 2026-08-25 reference sweep
remapped the weakness-register citations inside Appendix A's prompt to current
numbering. The 2026-08-23 entry states that those numbers are stale *deliberately*
— it is the text that ran, preserved as such — so the remap was a change to a
known-good prompt made incidentally rather than as a decision. Reverted; Appendix A now
diffs identical to the baseline commit. **The lesson for future sweeps: a
reference can be stale on purpose, and a mechanical repair cannot tell.** Appendix A's
*nine privacy costs* is wrong against §19.7 for the same reason and stays wrong.

### 2026-08-25 (P24 reduced; §11.8 closed; a reach claim corrected)

The author
corrected a mistake in the previous round's §11.5 text. `data_practice` was
described as reaching "a user who already has access", which conflated the two
scopes: **`discover_scope` gates catalog answers and `connect_scope` gates
sessions, and the first comes first.** A user must query the catalog to know a
resource exists at all, so the declaration is in hand at the moment the decision
to connect is made — not after it. The reach paragraph is rewritten and §11.7's
narrow-protection paragraph, which predates the field, now says the user evaluates
rather than discovers afterwards.

**P24 reduced from High to Medium** in consequence: the finding was that §11's
permission model gave a user *no way to evaluate the operator they route through*,
and the signed entry plus its declared posture, delivered pre-connection, is that
way. Residual: a declaration is a claim and not a guarantee (§1.1), and the
operator sees the traffic whatever they declared.

**§11.8 deleted** — the gateway-evaluation item was its only entry. Three
citations of §11.8 elsewhere in §11 turned out to mean **gateways**, which are §11.7,
and were stale from an earlier renumber; repointed. §23.2's citation of §11.8 for
the resource-interaction blocker was stale in the other direction — §11.8 stopped
carrying that status when it was reduced to one item — and is removed rather than
repointed, since §23.2 already states the blocker in full.

### 2026-08-25 (incident narration removed from §23.2; §19's intro repaired)

§23.2's resource-interaction entry told the story of an authoring error: a status
assigned before anything was built, an implementation attempt that could not carry
a request, and why nobody had noticed. **None of that is an open item.** The error
surfaced no structural problem and no decision about the design — only that a
mistake once existed — so the change log is the whole record it needs. The entry
now states the open item and what it blocks, and nothing else. **The general rule: a
lessons-learned belongs in the log even when it is a genuine lesson, and never in a
register of open items.**

**The suggested target moved to where targets live.** *Carry a resource request that
is refused* is now review plan 0.6 target 8, alongside the other seven, rather than
sitting in the design's open register — the design says what is unfinished, the
review plan says how to attack it. Its rationale travels with it: every prior target
took a success path, and `resource-requirements.md` §7.1 makes refusal the normal
outcome for most requesters, since access is a predicate evaluated at request time.

**§19's intro was damaged**, and the damage predates version control — identical
at the baseline commit. A sentence had lost its subject, leaving *"…under LINDDUN's
seven categories. it was argued locally at each mechanism and never assembled"*
followed by a claim about why an unnamed finding went unnoticed. Repaired to state
the surviving point — privacy is assessed under composition rather than mechanism
by mechanism — and to cite §19.1, which argues it in full.

### 2026-08-25 (0.8's prompt updated; 0.6 gains propagation targets)

The
2026-08-23 decision preserved Appendix A verbatim, stale section numbers included, on
the grounds that changes to a known-good prompt belong in a separate decision.
**This is that decision**, and it found more than stale numbers.

**The prompt's REASONING example had been refuted by the design itself.** It told
a reviewer that the designers "accept patron eclipse because *identities are cheap
and there is no token to steal*" and invited them to attack that reason. §18 now
states the opposite in terms: *the tempting justification — identities are cheap
and there is no asset to steal — does not hold*, because the asset is the victim's
authentication decision-making environment. **A reviewer given the old prompt
would have spent effort attacking a justification already retracted, and the
finding would have classified RESTATES** — the category the prompt calls
worthless. Replaced with two live justifications, from §18's Potemkin entry and
§19.7's locator-leakage entry.

Register citations updated to current numbering and named as well as numbered, so
a future renumber degrades to a still-usable prompt; **§19.8's correlations
added**, having been omitted from the original list. The count of privacy costs is
gone rather than corrected — §19.7 has held nine, then eleven, and a count in a
prompt is a thing that drifts silently.

**0.6 gains targets 9 and 10**, for topology propagation and the rootward memo.
Nothing in targets 1–8 emits or receives a `TopologyPush` or `TopologyMemo`, and
nothing publishes or ingests an `EndpointRecord` — adoption and resolution both
stop short of propagation. By this section's own observation that *a layer nobody
has built against tends to produce a construction change rather than encoding
corrections*, which held for targets 5, 6 and 7, propagation is the likeliest
source of the next construction change and had no way to be reached. **Target 3
also gains the minimised-record case**, since the presence body was restructured
around a disclosure root and an implementer following the old shape would fail
immediately.

## 2026-08-26

### 2026-08-26 (withdrawn register rows refiled; the design no longer cites the scratchpad)

The 2026-08-25 minification moved twelve withdrawn P/C rows out of
design §19.4 and §19.8 so those registers would state current risk rather than
the history of what was asked. **That part was right and the destination was
wrong**: they went to `review-tracking.md`, and the design was given two citations
pointing there for resolution of withdrawn numbers.

**`review-tracking.md` is an assistant scratchpad** — a queue for working through
review responses without losing the thread mid-evaluation. The author has never
read it. So the move put the content somewhere invisible to him, and made an
authoritative document depend on a scratchpad for its own citation integrity.

**Refiled here**, below. The design's two citations now point at this file, and
**no document in the six cites the scratchpad at all.** The rule that follows: a
scratchpad may cite the design; the design may never cite the scratchpad.

### Withdrawn and closed register entries

Rows that once sat in design §19.4
and §19.8, kept here so **a citation to a withdrawn number resolves to
*withdrawn*** rather than silently to a different finding. **Numbers are never
reused**, so the gaps in those registers' sequences are these. They live in the
change log rather than the design because a withdrawn finding is not a current
risk, and §19.4 is the design's statement of current risk.

| # | Finding | Severity | Disposition |
|---|---|---|---|
| ~~P7~~ | ~~Activity summaries export a behavioural baseline to strangers~~ | — | **WITHDRAWN** with the mechanism (§9.2). Numbers are never reused |
| ~~P8~~ | ~~Topology deanonymisation by association~~ | — | **WITHDRAWN**, folded into P15: identification yields a member's job, not a label for any of their subtrees. **The withdrawal originally rested on a second leg — *the attacker described is a horizon member already* — which stopped holding on 2026-08-25**, when §15.2's memo gave ancestors topology for parties far outside their horizon. The conclusion survives on the first leg alone: what labels a subtree is its catalog, and the catalog is answered on request within horizon (§11.5), so a distant ancestor cannot obtain the labels at all. Numbers are never reused |
| ~~P9~~ | ~~Divergence-notification fan-out~~ | — | **WITHDRAWN.** The exposure it named predates the recovery: a thief holding the key already reads everything addressed to it, so an inquirer's loss dates from the theft, not the notification. And the disclosure runs the **right** way — see §9.0.2. Numbers are never reused |
| ~~P10~~ | ~~Policy-descriptor fingerprinting~~ | — | **CLOSED by removal.** Nothing publishes a trust policy (§16.2), so there are no published parameters to fingerprint. Numbers are never reused |
| ~~P22~~ | ~~Verification-query logs have no retention rule~~ | — | **CLOSED** (§7.4). There is no log: the anti-oracle aggregate is a **lock**, a per-requester and per-window counter held only for the enforced ceremony duration. A durable queryable history was never needed for the defence and is not kept. Numbers are never reused |
| ~~P34~~ | ~~A subject controls their own verification threshold~~ | — | **NOT A FINDING.** It treats *n* as a score a subject understates. **An archive answers specific questions rather than supplying a total** (§16.1): an evaluator looking for interactions with people they know either finds them or does not, so **volume proves nothing and manufacturing volume gains nothing**. Partitioned identity is a designed property; forking divides what can be demonstrated rather than concealing a total. Numbers are never reused |
| ~~N3~~ | ~~Veto delegation cannot be privacy-assessed~~ **CLOSED by withdrawal.** The mechanism does not exist (§9.2), so there is no object to assess and no audience to determine | — | — |
| ~~C12~~ | ~~Verification-query log + the subject's own archive~~ | **CLOSED** (§7.4). The correlation required a retained query history to join against the archive; the aggregate is now an ephemeral lock expiring with the ceremony window, so there is nothing to join. Numbers are never reused | — |
| ~~C13~~ | ~~Disavowal reason code + resource access history~~ | **WITHDRAWN.** The join needs one party holding both halves, and only the disavowing patron does — resource-grant state is local to the owner's node, and that patron already knows their own reasoning. An observer with the code alone gets the band, which is what the enumeration was designed to give them; anyone holding the owner's role assignments already knows more than a reason code adds. **The scenario is also unreachable**: disavowal ends the relationship, so the membership gate (§11.2) revokes all access as a consequence, and there is no separately-dated grant revocation to correlate against. Numbers are never reused | — |
| ~~C14~~ | ~~Capability vector + network point across an identity fork~~ | **WITHDRAWN.** A serving node is necessarily inside the Dunbar org, and a rotation propagates as a topology-class message pushed within horizon (§9.0.2) — so **any node positioned to see both attaches has already received the record binding the two keys.** The fresh-Genesis case fails from the other side: where unlinkability matters, the new identity appears in a *different* subnet under a different serving node that sees only one; where one node could see both, the person was adopted by a neighbour who met them, so the link exists socially whatever the transport shows. Numbers are never reused | — |
| ~~C16~~ | ~~`AbuseReport.detail` + resource log~~ | **WITHDRAWN**, for the reason C13 was and one more. **The schema has no subject** (§11.6): a report carries `resource` and `reporter` and names no third party, so a detail field describing someone's circumstances has nobody to describe. And both ends are held by one party — either the resource reports to its own owner, or a user reports about a resource and any personal particular is their own. An earlier version distinguished this from C13 on the grounds that a third-party reporter delivers particulars the owner did not hold; **that reading was wrong**, since the owner granted the role and owns the resource. Numbers are never reused | — |
| ~~C18~~ | ~~Datasets joined within one infra process~~ | **WITHDRAWN as stated.** The correlation assumed a package could reach topology, liveness, queue state, prekey requests and role-evaluation inputs; **the design offers no binding that exposes any of them** — §11's rule that *the resource never reads network state* applies to a hosted package, and `infra-client-requirements.md` §9.2 states that the hooks do not exist rather than being narrowly scoped. What remains is not a correlation but an **implementation question**: whether the isolation mechanism enforces that boundary against hostile code, which is for the sandboxing literature rather than this document. Numbers are never reused | — |
| ~~P6~~ | ~~Forwarding records are a post-departure linkability window~~ | — | **CLOSED BY REMOVAL** 2026-08-26. Forwarding records are withdrawn (§4, deferred features), so no former patron holds a pointer to where a departed subordinate went and **departure severs immediately**. Numbers are never reused |
| ~~C3~~ | ~~Forwarding record + old locator~~ | **CLOSED BY REMOVAL** 2026-08-26. The join required a forwarding record, which no longer exists (§4). Numbers are never reused | — |

### 2026-08-26 (0.6.1 adoption implementation pass, clean room)

Seven
UNSPECIFIED items, **all seven confirmed against the text and all seven fixed**.
One was blocking.

**The blocker: `VerifierResponse` field 9 could not satisfy both rules.** §3.5
fixed it as `COSE_Sign1`, which carries exactly one signature, while §3.1 requires
verifier responses inside a `Recovery` to be hybrid, which needs two. Two rules
individually correct and jointly unsatisfiable — the class Appendix A exists to find.
Field 9 is now `COSE_Sign1` in a presence record and an untagged detached
`COSE_Sign` inside a Recovery, and the field's type is fixed by where the response
sits.

**Which signature hybridises is now stated rather than derivable from a size
figure.** Field 9 only; field 7 stays classical. The reviewer inferred this
correctly from *~34 KB per typical recovery* being one Ed25519+ML-DSA pair per
response rather than two, and §5.1's arithmetic agrees — but the load-bearing reason
is different and is now in the text: **a recovery response's `subject` MUST equal
the newly adopted node, so field 7 is signed by the very key an attacker mounting a
fraudulent recovery already controls.** Hybridising it protects nothing. Field 9
forges a *verifier's* attestation, which is the attack; field 7's protection is
anti-oracle and expires with the ceremony window.

**A transaction's timestamp is when it takes effect** [author] — not when it was
drafted, not when either signature was applied. Stated at §1's type definition so it
governs every transaction rather than adoption alone, with the constraint that
§2.2 fixes the body before anyone signs and permits any delay in gathering
signatures: the value is the parties' agreed effective time, not an observed moment,
and nothing checks it against a clock.

**Back-pointer lists have a signer order.** §2.1 said "the same order as the
transaction type's required signer set", and a set has no order. Now enumerated per
type — adoption is node then patron [author: no practical valence either way, but
there must be a rule] — with the hazard stated: **getting it wrong is silent**, since
every hash and signature still verifies while each predecessor attaches to the wrong
signer.

**Three smaller fixes.** `rhtn/1:recovery` deleted from the domain-separation table,
orphaned since field 3 stopped signing the Recovery map on 2026-08-24 — §3.1 defines
one old-key proof and it signs under `rhtn/1:successor`. Subject consent signs the
**raw 32 bytes** of `query_id`, not a CBOR bstr wrapping them; every other payload in
the document names its encoding and this one alone did not. The 1024-byte extension
bound measures the **complete encoded CBOR slice** for the value, which is measurable
for every value type and is what bounds parser work.

**One consequence found while applying**: with field 9's hybrid case explicit, a
recovery adoption is ~42 KB — the second-largest object in the protocol — and the
size table had no row for it.

### 2026-08-26 (root reserved for the design; process files moved to `Robot/`)

The root now holds exactly the six documents that constitute the design —
`network-design.md`, `wire-format.md`, the three requirements documents and this
log — plus `CLAUDE.md`. Moved to `Robot/`: `authoring-conventions.md`,
`review-plan.md`, `review-tracking.md`, `resource-interaction-requirements.md` and
`network-design-checkpoint-2026-08-12.md`.

**`CLAUDE.md` stays in the root because it must.** A root `CLAUDE.md` is loaded at
session start; a subdirectory one loads only when files in that directory are
touched. Every instruction in it is a start-of-session instruction — sweep by
search, do not invent justifications, ask whether the component is required — and
those are worth nothing arriving late.

**The invariant this encodes:** a root document may cite another root document;
**no root document may cite anything in `Robot/`.** The design must not depend on a
working file for its own integrity — the failure the 2026-08-25 register move
produced and this layout now makes structurally visible. Verified: zero citations
from the five root design documents into `Robot/`.

Live cross-references updated in `CLAUDE.md`, `Robot/authoring-conventions.md` and
`Robot/review-plan.md`. **The change log's eighteen mentions are left as written**,
being a historical record of what those files were called when the entries were
made. Moves were made with `git mv`, so history follows the files.

### 2026-08-26 (0.6.2 locator-resolution pass; forwarding withdrawn)

Twenty
UNSPECIFIED items. Fourteen were correctly identified as local policy and needed
nothing. Of the six structural claims, **two dissolved under the author's
questioning and four held.**

**Withdrawn: the anchor-bootstrap gap.** The reviewer found that §5.3's procedure —
*receives the peer's `KeyMaterial` in the resolution reply, checks its hash, pins* —
cannot execute when an anchor returns a referral, since the reply carries only the
next hop's or serving node's key. True, and it does not matter. **The author's
point: reaching the recipient proves the chain.** §12.6.1 already says nothing
polices referral honesty because nothing needs to; a hostile chain costs a failed
dial, since the session that matters authenticates against the subject's own key and
the payload is end-to-end encrypted. §5.3 corrected to describe what happens rather
than a procedure that cannot run.

**Withdrawn: the missing `ServingInfra.advances`.** §12.6.1 permits a node to return
the serving node directly; §7.7.3's arrival check rejects exactly that. The author
asked what the check breaks. **Nothing** — §7.7.3 says the running total exists
"purely to know when it has arrived… no node depends on it", and the reply's result
code already says so. Over-counting is caught by the overshoot bound; equality only
detected *under*-counting, which is the legitimate optimisation. **The check was
deleted rather than a field added.**

**Forwarding withdrawn entirely.** The reviewer found that a rotation repair cannot
be expressed. The sweep found worse: **`ForwardingRecord` had no delivery message at
all.** Departure (§3.2) carries no locator and cannot, since departure and adoption
are independent, so the record needed a separate post-adoption notice that was never
specified — while six design passages said a former patron *holds* one. The author
described its intent — a courtesy stub so traffic still arriving at an old patron is
forwarded — and withdrew it: it is unenforceable either way (§1.1), and it cost a
standing pointer to where a departed subordinate went.

**Removed with it**: `wire-format.md` §7.2 (number tombstoned), `ResolveReply`'s
repair variant and result code 1, the four-repair bound, the 90-day TTL parameter,
and the infra obligation to retain and return forwarding records. §12.6.2 is
retitled *Stale paths fail; they are not repaired.* **P6 and C3 are closed by
removal** and their rows moved here — departure now severs immediately.
Redirect-far is left to currency-attestation queries, which is the other mechanism
§15 already named. **Inside the horizon nothing is lost**: departure and adoption
propagate as topology, so a neighbourhood holds the new position; the cost falls on
distant contacts, who re-resolve from a higher ancestor or are re-introduced, which
is where §12.4 puts discovery anyway. Recorded in §4's deferred list with what
revisiting it would require.

### 2026-08-26 (§5.3 corrected: an unpinned anchor is never authenticated)

A
follow-on to the 0.6.2 pass, from the author asking whether the anchor's ID in the
routing table is the comparator for the key the handshake presents. **It is not,
and the reason is already written down elsewhere.** An identity is SHA-256 of the
*pair* — §1 says "never one of them" — while RFC 7250 carries one
SubjectPublicKeyInfo, so the classical component alone cannot reconstruct the hash.
§6.2 states exactly this for a sibling: *trust-on-first-use cannot check a keyhash
it cannot reconstruct*, and it concludes that a sibling without key material is
**UNUSABLE rather than dialling it unauthenticated.**

§5.3 claimed the opposite for anchors — trust-on-first-use, receive the key in the
reply, check the hash, pin — which is both impossible and unsupplied, since
`Referral.key_material` names the next hop and `ServingInfra.key_material` the
serving node, never the responder. **Two sections, one problem, opposite
conclusions.**

Corrected to say an unpinned anchor is **never authenticated**, that this is not
trust-on-first-use, and that it is tolerable because a *referrer's* identity is not
what protects the requester — design §12.6.1's point, which the author made
independently: reaching the recipient proves the chain. The retroactive-attribution
property is narrowed to a holder who has the key by other means; for everyone else a
forged entry costs one failed dial. **The divergence from the sibling rule is stated
explicitly** — a sibling is a destination whose identity is the point, an anchor
returning a referral is a referrer whose identity is incidental — so that nobody
"fixes" one case to match the other. The companion claim that "full keys are fetched
and verified at contact time" was corrected with it.

### 2026-08-26 (0.6.2 verification pass: three fixes claimed and not applied)

A
read-back of the 0.6.2 report against the current documents found the previous
entry's claim *"the check was deleted rather than a field added"* was **false at
the time it was written**: the arrival-consistency equation still stood in §7.7.3,
as did a "returns a repair" in its intro, and two fixes proposed as mechanical —
the truncation clarification and `NetworkPoint`'s key listing order — had never
been applied at all. All four are now actually in.

The truncation clarification landed stronger than proposed: **a signed locator
always carries the complete path because the signature covers it**, so truncated
prefixes exist only in unsigned aggregate state and §5.6's full-path rule needs no
completeness marker — the reviewer's `require_complete_path_provenance` seam
dissolves.

§7.7.3 now states that arrival is announced by the reply rather than computed:
per-referral checks (`advances` ≥ 1, no advancing past the path's end) remain, the
equality that rejected §12.6.1's direct-serving answer is gone.

**Still open from 0.6.2**: `routable prefix` has no defined encoding while §19
reasons about prefix-based independence — awaiting the author's ruling on whether
it is compared programmatically or is human-facing evidence.

### 2026-08-26 (v1 is IPv4-only; IPv6 and prefix-based reputation deferred with their design worked out)

Following the routable-prefix encoding question from
0.6.2, the author asked what else in the specification indicated IPv6 in the
initial design — and §17.3 had already decided the matter: *demand IPv4 for now
and take the security as a bonus.* Two sites contradicted the decision.

**`NetworkPoint` accepted 16-byte addresses**, wire surface no v1 implementation
could exercise — the ossification pattern §6.1's greasing argument warns about.
Field 1 is now `bstr .size 4` and a 16-byte address is malformed in v1. Widening
later is an ordinary versioned change; nothing is precluded.

**The routable-prefix field was IPv6 machinery in an IPv4 release.** It exists to
answer the counting problem IPv6 creates; under IPv4 the address itself is the
scarce unit and ASN carries concentration. Key 3 withdrawn, not reused — **and the
0.6.2 open question closes by removal**, the second time in two days a component
question dissolved because the component was not required.

**The deferral carries its design so revisiting imports rather than re-derives**:
/64 as the one-subscriber reputation unit (RFC 6177's assignment floor; Spamhaus
lists at /64, M3AAWG recommends it for rate-limiting), BGP NLRI encoding
(RFC 4271 §3.3) with zero trailing bits, self-asserted and weighed under §1.1,
checkable against public routing data.

**Swept**: peering records now carry ASN only (§3.4, §6.3, §3.4's redundancy
bullet), §19.7 item 6 and C8 narrowed to ASN, §17.3's IPv6 leg points at the
deferred entry. The author accepted the one cost named: two IPv4 peers in one ASN
but different data centres are no longer distinguishable by prefix — *"we aren't
relying on the independence measurement for trust"*, which the wire note now
states: independence is a visible signal, not a trust input.

### 2026-08-26 (0.6.3 presence-validation pass: eleven items; two disclosure fields return to the body)

All eleven verified; ten held, one (U-11) was the text
being confusing rather than contradictory.

**The central finding was self-inflicted.** §4.5.2 claimed structural verification
needs no disclosures while §2.2's structural rules consume three disclosable
fields — `started_at` (monotonicity, the 730-day window), `subtype` (formation
rules), `proximity` (the strongest-channel rule). A minimised record could not be
structurally validated or threshold-checked at all, and the reviewer's
UNVERIFIABLE third state was the only honest output.

**Checking what withholding protected settled it.** `started_at`: `finalized_at`
and the seed-window ordinal are body fields, so the ceremony's *day* was already
public in every presentation — withholding hid only time-of-day while breaking four
checks. `subtype`: a formation record's absent evidence arrays announce it, so the
label hid nothing. **Both return to the body** (keys 1 and 10, subtype's 0/1
assignment restored — closing U-5, which the move had orphaned). `proximity` stays
disclosable — genuinely private — and the strongest rule is the one structural rule
checked only on reveal, stated in §2.2, §4.5.1's decoder rules and both exchange
tables. The disclosure set is seven; salts ~112 B, 0.3%.

**A presentation now has a schema** (U-1, blocking): `PresentedRecord = [Envelope,
[7 DisclosureSlot]]`, slots ascending by label, each a full `Disclosure` or a bare
32-byte digest — **position supplies the label**, so withheld digests carry
nothing. Seven slots always; a revealed label differing from its slot is malformed.

**Value schemas restored** (U-6): each label's value is the CBOR its field carried
in the body, tabulated in §4.5.1 — the move had orphaned retention's `[uint, uint]`
entirely.

**Formation records carry the genesis value, not an absent back-pointer** (U-7):
key 0 is `[SHA-256(signer keyhash)]` per signer, as for every first record. "MUST
be absent" contradicted §2.1 and left three incompatible encodings open. U-11's
confusion resolved in the same block by separating the claims: a *conforming*
established key cannot produce one honestly; a *cheating* one can, undetectably to
a history-less validator, and the design accepts that as evidentiary weakness —
detection arrives with anyone holding the real chain.

**The monotonicity comparison uses the predecessor's effective time** (U-8) —
`finalized_at` for presence, the §1 transaction timestamp otherwise, every merge
predecessor checked.

***n* is scoped to participation** (U-9): a record the subject signed as a witness
is in their chain and is not their meeting — counting it would raise *n* without
adding a candidate. Candidates are the other participant of counted records.
**And only verified history counts** (U-10): canonical, content-addressed,
signature-checked; an unfetchable record leaves the chain incomplete, not smaller.

### 2026-08-26 (0.6.3 re-run: previous fixes held; twelve new items, all applied)

The re-run validated the seven-slot presentation, genesis back-pointers,
participation-scoped *n*, verified-history admission and effective-time rules
cleanly, including the holder-relative half-verified case. Twelve new items, all
confirmed real.

**Two were more than encoding gaps.** `query_id` was defined self-referentially —
SHA-256 of a map that contains the hash — and is now the hash of fields 1–4 with
field 5 absent. And the bidirectional frame bound inherited stream 0's 64 KB while
the size table's maximum presence record is ~65 KB: **the largest legal object was
untransmittable.** Bidirectional streams now bound at 256 KB; control stays 64 KB.

**The location-method registry was stranded as a comment under `Channel.3`**,
where the reviewer correctly refused to apply it. Relocated to
`Asserted`/`Corroboration` as a shared registry, 0–3, **deliberately open** like
the disavowal bands — evidence channels grow, and rejecting an unknown method
would make each a flag day. Geohash pinned to 3–4 bytes of canonical lowercase
base32, one cell one encoding.

**Rules stated rather than left to divergent inference**: threshold traversal may
prune a verified branch at the 730-day boundary (monotonic effective time makes
anything past it irrelevant — without this one missing genesis-era record blocks
every recomputation forever); response arrays have no canonical order and are
evaluated as (subject, verifier) sets; there is no aggregate verdict — malformed
is terminal, every other dimension reports independently, and collapsing them is
a policy act (§16.1); client-integrity evidence has no structural tie to
attested/scheme; channel resolution and binding are syntactically optional with
no presence condition; `nominated_by`'s wire comment now matches §2.2's checkable
rule rather than restating the uncheckable ceremony claim.

**The design/wire disagreement on location assertions resolved toward the wire**:
"at least two assertion methods are required" meant the *registry* defines two so
the ceremony survives one being unavailable, not that a record carries two — a
record may carry none, and sufficiency is the evaluator's policy.

**Ranging mode and receiver implementation are deferred** (§4): carrying them
would let policy weight a UWB pass by defeatability, but the fields fingerprint
hardware, need a registry nobody can populate, and disclosures are not extension
points. Until then a UWB pass is weighted as the weakest deployed mode. Bounds
added while there: integrity evidence ≤1024 B, channel binding ≤128 B.

## 2026-08-27

### 2026-08-27 (0.6.4 attach/failover pass: twenty-three items — nine wire clarifications, three small wire rules, a 0.6.2 leftover)

Fourteen of the
twenty-three were local policy, correctly identified and left local. The rest:

**The heartbeat counter is one independent sequence per sender.** "Per-session,
from 0" plus both-sides-send left the namespace ambiguous, and the reviewer rated
it blocking: a peer expecting one shared alternating sequence treats every valid
heartbeat as a gap. A shared sequence is unimplementable — neither side can know
the interleaving — and the schema now says so.

**Refusal has a carrier: QUIC application close code 1 (`refused`).** [author]
`light-client-requirements.md` required treating a policy refusal as the node's
answer rather than the endpoint's, while the wire said close, reset and timeout
are indistinguishable failures — a rule with no carrier, and the reviewer's
clean-close heuristic was honestly marked unreliable. One registered code closes
it; every other outcome stays an endpoint failure.

**The heartbeat interval is bounded 1–3600 s.** [author, after prior-art check]
Unbounded, it let a hostile server hold clients in never-failing sessions and made
QUIC idle timeouts unconfigurable. Prior art brackets the cap comfortably — MQTT
mobile practice 30–300 s with an 18-hour protocol ceiling, XMPP pings 60–300 s,
RFC 4787 UDP NAT expiries 30–120 s — and the NAT fact is noted in the schema: a
server advertising near the cap loses its push path long before liveness fails.

**A server MUST NOT process an `Attach` from 0-RTT early data.** [author] Early
data is replayable and a replayed `Attach` re-binds session state; stated
server-side because that is where it is checkable. Resumption keeps its latency
benefit for everything after the handshake.

**Nine clarifications**: 64 KB is exactly 65,536 bytes (and 256 KB is 262,144);
unknown frames may precede `AttachAck` but any known frame other than the ack
fails the attempt, and repeated `Attach`/`AttachAck` after it is a protocol
error; a receiver may authenticate a sibling with any validated pin matching the
keyhash, provenance being the sender's obligation; an invalid currency
attestation is treated as absent, never as an authentication failure — currency
gates trust operations, not connectivity; `queued_messages` counts the responding
node's queue only and a degraded client must not present it as global; a
malformed `SiblingUpdate` is ignored whole on the malformed-heartbeat posture;
misses are counted by elapsed full monotonic intervals, not delivered timer
callbacks; heartbeat timestamps are advisory; a greased id avoids locally-known
ids and a collision is accepted at its probability.

**A 0.6.2 leftover surfaced**: `light-client-requirements.md` still instructed
clients to check the arrival equation deleted from §7.7.3 — replaced with the
per-referral checks. And design §5.2 now states the browser-wasm constraint
precisely: the gap is the transport stack, not the primitives, and native and
browser conformance are separate targets.

### 2026-08-27 (0.6.5 capture and verifier query: twenty-six items — three jointly unsatisfiable, one security restatement, one four-day-old stale sentence)

Fourteen were local or implementation behaviour and stay local, consistent with
the author's earlier ruling that the sealed store's AEAD suite waits for
implementation.

**Three pairs of rules could not both be satisfied.** `KeyGrant.query_id` was 16
bytes while the query and response define the same identifier as 32-byte SHA-256
— now 32. Field 7's rule required a verifier to reject a query lacking subject
consent while request type 4's body was `VerificationQuery` alone, which cannot
carry it — the body is now `[ VerificationQuery, COSE_Sign1 ]`, consent beside
the query and never inside it, since it signs the hash of fields 1–4 and placing
it in the map it authorises recreates §5.6's circularity one level up. And
`basis` was required on every response while `unavailable` and `pending` assert
no evidence — no truthful value existed, photo bases demanding a version that may
not exist and personal_knowledge as a sentinel being structurally valid and
false. Basis is now required for results 0–2 and absent for 3–4.

**§7.4's probe-pricing encoding was restated to what actually holds.** It
claimed queries carry the ceremony pre-commitment "countersigned by the
witnesses" — an object that was never defined, and that would prove nothing: a
distant verifier cannot tell real witnesses from an attacker's keys, the same
impossibility as authenticating an unpinned anchor. **The proof that the
encounter is live is the subject's own consent over `query_id`**, minted by the
subject's client during the ceremony — each probe requires the subject's live
cooperation, checkable by any verifier from the identity the query itself names.

**Smaller**: a retention year is 365 fixed days, matching the 730-day window's
arithmetic; a grant may arrive before its query and is buffered unopened,
briefly, bound locally; §7.5.2's key-derivation prose bound `txid` in one
sentence four days after the correction to `ceremony_id` — flagged in review on
2026-08-25 and only now actually fixed. §23.2's template-length item broadened to
**the canonical biometric profile** — extractor, format, fuzzer, matcher,
registry — since cross-client verification depends on the whole set; §14.2.4's
open list gains payload-type demultiplexing, grants and late responses riding
the end-to-end channel with nothing to tell them from application payload.

**Left for the author**: the `pending` deadline. Design §16.6 has a patron
returning pending *with a deadline* and §21.1.1 classes the deadline as set by
one side and obeyed by the other, but `VerifierResponse` has no field, and a
deadline inside a permanently archived response would be stale noise. Options:
a transport-level notice outside the signed response, or reclassifying the
deadline as the querier's local patience.

### 2026-08-27 (pending deadline is the querier's patience; every tombstone removed; wire numbering compacted)

Three author directives in one round.

**The `pending` deadline is not transmitted.** §16.6 now says a queued query
resolves when the client reconnects and how long to wait is the querier's own
patience parameter; §21.1.1 reclassifies it as local policy, leaving the heartbeat
interval as the only set-by-one-side value. Nothing carries a deadline — a promise
inside a permanently archived response would be stale noise the moment it passed.

**No tombstones, no withdrawn-feature explanations, anywhere in the
specifications.** Twenty-odd prose asides — "an earlier version said…", "no such
object", "withdrawn 2026-08-26…" — deleted or converted to forward statements
where the why-not is load-bearing (no activity summaries, no veto, no notice
period, witness countersignatures deliberately not carried). §19's
register-method paragraphs, which taught discipline by citing withdrawn findings,
moved to `Robot/authoring-conventions.md`; the design keeps one functional line —
numbers are not reused, missing ones resolve here.

**Wire numbering compacted, feasible precisely because nothing is published**:
Disavowal reason code is field 4; AbuseReport is keys 1–5; NetworkPoint's port is
key 3; the presence body is contiguous keys 0–8 (participants 3, witnesses 4,
responses 5, subtype 6, ordinal 7, disclosure root 8); ResolveReply is fields 1–5
with result codes 0 serving / 1 failure / 2 referral; TopologyPush and
TopologyMemo are frame types 5 and 6; and wire §7 renumbers to a clean 5.1–5.9 —
anchor entry 5.2, segment grant 5.3, late response 5.4, subtree ack 5.5, endpoint
record 5.6, resolution 5.7 with subsections 5.7.1–3, prekeys 5.8, archive fetch
5.9. The lettered 5.3a–d sections and the three stub sections are gone. Every
cross-reference swept cluster by cluster and verified by the checker after each;
the P/C/A register numbers are deliberately untouched, being identifiers this log
cross-references rather than features.

**From here the rule is prospective**: a withdrawal edits the text to its final
state and this log carries the reasoning; nothing in a specification marks where
something used to be.

### 2026-08-27 (0.6.6 resource registration and catalog query: reply bound raised to 111, registration given request type 7, two broken code fences repaired)

The
clean-room pass reported one blocking defect, nine unspecified questions and a
classification conflict.

**The classification conflict was the serious one.** `wire-format.md` §4's table
gave type 6 the class `Topology` while design §15's Topology row says in bold that
resource registration is **not** in that class — so an implementer reading only the
wire table floods the catalog and undoes the discovery model. Type 6 previously
belonged to no class at all, having been excluded from the only one that named it.
§15 gained a **Catalog** row (five classes → six) and the wire cell now reads
`Catalog — never propagated`.

**The pagination defect was real and the author's repair was cheaper than the one
proposed.** The continuation names a service type, not a position, so 65 visible
entries of one type left the 65th unreachable and an asker following the hint
looping forever — the exact failure §6.4's "truncation must make progress"
paragraph claims to prevent, since ordering makes the answer *stable* and stability
is what causes the loop. A position-carrying cursor was proposed and dropped in
favour of **raising the reply bound to 111 = 1 + f + f²**, the Dunbar Org
population at or below, on the author's observation that the horizon is the
practical ceiling on same-type services. The limits table already authorised this:
its maxima are DoS ceilings, not capacity claims, and it says a ceiling a
legitimate use approaches is wrong and should move. Expressing the bound in terms
of `f` follows the sibling row's `9 (f − 1)`. **The frame bound caps the field at
127** — at 2 KB an entry, 128 no longer fit one 256 KB frame — and that is recorded
so nobody raises it again by reflex. The residual loop, reachable only where a node
hosts more than the bound of one type, is now detectable by the asker at no wire
cost: a full page plus a continuation naming a type already filtered on means
truncation, not another page.

**Registration was given carriage.** Type 6 was excluded from flooding and was not
among the bidirectional request tags, so nothing said how an owner hands a signed
entry to its host, who may submit one, or how `discover_scope` is configured —
leaving the one act that makes a resource discoverable to a proprietary management
interface, while `Attach` to that same host is fully specified. **Request type 7**
carries `ResourceRegistration` (the envelope, a requested `discover_scope`, a
nonce) and returns `ResourceRegistrationReply` (status, and the recorded `txid`).
**The authenticated peer MUST be the owner**: an owner-signed entry is relayable by
anyone, so accepting one from any peer accepts a replayed earlier envelope, which
silently reverts the current entry under apply-last. The echoed `txid` is a layout
check — a mismatch means the two implementations disagree about the body bytes.
The requested scope is a request the host may narrow, which is what "an owner
delegating hosting delegates that filtering" now means concretely. Withdrawal by a
non-answering owner needs no new message: re-register with a `discover_scope` of
self, which no asker satisfies.

**Two broken code fences, invisible to the reviewer and to a parity check.** §3's
`Scope` block was never closed, so `### 4.1 Adoption (type 1)` and its prose
rendered inside a code block; §5.2 had a stray opening fence putting five prose
paragraphs in a block while `AnchorEntry`'s own opener was missing. The file held
78 fences — an even count, which is why the check run after the compaction pass
reported clean. **Even parity is not pairing.** The check now walks the pairs and
looks for headings trapped in blocks; all 82 fences pair and none is.

**Four more the review did not reach.** §6.4 mandated the keyhash order, then said
which 64 are returned is the node's choice, then argued that requiring an order
would be "a sorting obligation with no consumer" — an argument against the rule the
section had already made. "Concurrent re-registration" was stated twice in two
wordings six lines apart. `infra-client-requirements.md` §11 told operators to
**sign every entry they return**, contradicting "the owner alone signs" and the
single owner signature `CatalogEntry` actually carries. And the sort key was not
total — §6.7 states two owners may register the same resource keyhash — so the
stability the truncation argument rests on did not hold; the key is now (resource,
owner).

**Applied from the register of unspecified questions**: the type-6 body layout is
now stated to be the `CatalogEntry` map itself with key 0 added, not a wrapper (the
wrapper reading changes body bytes, `txid` and envelope signatures); and the
explicit-scope keyhash list is ascending and distinct, matching every comparable
list in the document — it is signed, so two decoders disagreeing means one rejects
bytes the other accepts on an object neither may re-encode. **Not applied**:
apply-last needs no linearization point, which §6.5 already says outright; and the
crate-maturity findings restate design §5.2, which already records the unaudited
notices and the wasm transport gap.

**The consistency sweep that followed found six more.** Two stale ordinals: §9.0
and §15 both argued that "push near, redirect far" is *not a fifth message class*,
written when there were four and now wrong by one — restated by role, as *not a
further class* and *without a class of its own*, so the claim survives the next
row. **§21.1 lists eleven parameters and two places said thirteen**; the count had
already been corrected once, from sixteen to fourteen, and drifted again as rows
collapsed. Three sites still called `discover_scope` local-and-never-on-the-wire,
true of the entry and false once a registration can request one — the design's
illustrative schema listed it as a member of `CatalogEntry` annotated *never on the
wire*, and §11.5 said it is evaluated "at the owner's node" when for a hosted
resource the owner is not who answers. §23.2's blocking item said resource
interaction had **no** implementation attempt against it; the catalog half now has
one and the §8.2 request/response half does not, so the item is narrowed rather
than closed. And §7's prose list of what bidirectional streams carry had been
drifting from the tag table above it since before this pass — it omitted catalog
queries and prekeys — so it now points at the table instead of restating it.

**One asymmetry made explicit rather than repaired.** Type 7 has no request tag
either, and needs none: design §11.6 already says an abuse report is created and
consumed at the owner's node, signed because it is portable rather than because it
travels. Type 6 is the opposite case, which is why it gets a tag — an entry must
cross from the owner who signs it to the host that answers for it. §6.3 now says so,
because the question will be asked again.

## 2026-08-28

### 2026-08-28 (0.6.7 hosted-resource authorisation: response code 1 deleted and the codes compacted, hosting made unique per resource keyhash, three cross-document contradictions resolved, three broken tables repaired)

A clean-room pass at the
request path found eighteen unspecified questions, two source disagreements it
resolved by document hierarchy, and one code that cannot be sent.

**`no such resource` had no reachable state.** §8.2 declared code 1, put existence
first in the normative order and answered absence with code 2 there, then said code
1 was "only reachable at step 3 onward" — by which point the resource is known to
exist — and separately that a member asking for a keyhash the host lacks "still gets
code 2". Three statements that jointly leave no state in which the code can honestly
be sent. **The argument against it was already in the text**: a host cannot tell
whether a resource exists elsewhere, so *no such resource* asserts something it does
not know, and that is as true for a member as for a stranger. The code is gone and
the rest compacted: **0 delivered, 1 refused, 2 unavailable, 3 malformed, 4 no
subtree acknowledgement, 5 no matching role.**

**Malformed had no place in a normative order it was excluded from.** It now has
two: a body that does not decode is answered immediately, because nothing has been
addressed and nothing can be disclosed; a decoded body carrying malformed HTTP is
answered only after the opaque gates, so a stranger cannot probe a resource by
sending it rubbish and reading which complaint comes back. Availability stays last.

**One host may not serve two claims on one resource keyhash.** §6.7 permits two
owners to register the same keyhash and `ResourceRequest` names the resource and
nothing else — so a node holding both has nothing to choose with, and choosing
wrongly applies one owner's membership and roles to the other's backend. The host
refuses the second registration, which is a check it can make and the requester
cannot. The catalog rule is unchanged: two claims may exist on two hosts, which is
the case its reasoning was about.

**Three contradictions across documents, each resolved toward the design.** §8.2's
step 4 said *role predicates* run on the request path while §11.4 says predicates are
a macro over a materialised table and **neither of their two evaluations is on that
path** — the step is a row lookup, and the same wording was corrected in
`infra-client-requirements.md` §10.1. §11.3 said access is "a predicate evaluated at
request time", which is the same error inside the design itself; the point it was
making — no durable grant object — survives as a row the node re-derives when
topology changes. `infra-client-requirements.md` §10.1 gave the evaluation order as
*membership, existence* while citing §8.2, which gives existence first for a stated
reason: membership is owner-relative and a keyhash you do not host has no owner.
And `resource-requirements.md` §3 called the client-to-node carrier an rhtn control
frame citing §8.0, when it is a `ResourceRequest` on a new bidirectional stream.

**Three broken tables, none of which a reference check or a fence check can see.**
`resource-requirements.md` §3's credential table was cut in half by an interleaved
paragraph, orphaning `rhtn-audience` and `rhtn-session` — two of the four headers
the node-to-resource contract consists of, invisible to a renderer. `wire-format.md`
§1's limits table was split by a blank line, orphaning everything from `NetworkPoint
entries` down, including the `CatalogReply` bound edited the day before. And this
log's own withdrawn-register table had no header row at all, so fourteen rows
rendered as literal pipes; the note explaining that P rows carry four columns and C
rows three was itself stale, since all fourteen carry four. **The check now walks
every table run and requires a separator as its second line**, alongside the fence
pairing added yesterday.

**Answers where the documents had a determinate one and had not said it.**
`rhtn-roles` **may be empty** and that is not an error: `connect` is the gate and it
is spent delivering the request, so an empty list means an admitted caller granted
nothing further — the alternative reading would make two existing sentences
pointless. base64url is **RFC 4648 §5 without padding**, because a strict parser
given the other spelling rejects bytes that decode identically. Role order in the
header **carries no information**. The session identifier is **per resource**, on
the pairwise principal's own reasoning (§11.0.2): one shared across resources would
re-link the caller between them and undo the separation the principal was derived to
create. And §11.2 now rejects `CONNECT`, `Upgrade` and `Expect: 100-continue` —
each wants a tunnel, a protocol switch or an interim response, and the exchange
carries one answer — and fixes routing to the resource keyhash, since letting a
caller's `Host` or absolute-form target re-aim the request turns the proxy into
someone else's client.

**Two limits stated rather than closed.** "Canonically re-serialise" means *from
your parse, deterministically*, not identically to another node: nothing signs those
bytes and no second implementation compares them. And the opaque-refusal paths
return equal codes but are **not equalised in time** — the opacity is in what the
node says, not in how long it takes to say it, and an implementation that does not
equalise has a narrower property than §8.2 describes.

### 2026-08-28 (0.6.8 refused resource request: the 0-RTT rule generalised, stream sequencing and the failure boundary stated, response framing moved to the object it describes)

The first pass to find **no blocking defect**. It confirmed the
previous day's renumbering — codes 0–5, field 2 present iff code 0, and code 1
covering both an unhosted keyhash and a lapsed member — and reported fifteen
unspecified questions, of which four had a determinate answer the documents had
not given.

**The 0-RTT rule named one instance of a claim that covers five.** §6.2 forbade
processing `Attach` in TLS 1.3 early data because a replay re-binds session state.
The review noticed a `ResourceRequest` is worse — it carries an arbitrary
application request, and a replayed one repeats whatever that request did — but the
claim is broader than either: **a request that changes state or spends a budget
must not be processed in early data.** A replayed prekey fetch consumes a one-time
key (§7.8); a replayed verifier query spends an anti-oracle count (§3.6); a
replayed registration reverts the current entry (§6.5). The general rule now sits
in §8.1 and `Attach` is named as its other instance, on the other stream class.
**Read-only lookups are unaffected**, which is what keeps 0-RTT worth having.

**Two implementations could deadlock on framing nobody had written down.** Nothing
said whether a requester half-closes after its one frame or whether a responder
waits for that close before answering — and a responder that waits while a
requester waits for the answer first is a hang, not a disagreement about bytes.
One frame per stream, requester half-closes, **responder does not wait**. The
boundary between a malformed frame and an answerable one is now stated in the same
place: until the array header and `request_type` are readable there is nothing to
answer *in*, so the stream fails; once the type is known a defect in the body is
that type's business. **No application error code is assigned for the reset** and
none is needed — the reset is the whole message.

**The resource response's framing rules were filed under the catalog.** §6.4's
sentence about carrying a catalog query had three paragraphs about
`ResourceResponse` run onto it — that field 1 = 0 may repeat as a sequence, that no
length is declared up front, that a non-zero status ends the exchange. In §6.4 they
are not merely misplaced but wrong: a `CatalogReply`'s field 1 is a nonce, not a
status, so a reader following them would expect a repeating catalog reply with a
status code. Moved to §8.2, where the object they describe is defined. The
implementation found the rules and applied them correctly, which is how they were
noticed rather than missed.

**Existence is a binding, not a running process.** §8.2's step 1 said the resource
must exist on the node without saying what that means, so a stopped package could
be read as absent — answering code 1 where another host answers code 2 for the same
deployment, and collapsing a distinction the two codes exist to keep. Existence is
a binding from keyhash to owner and backend; running is step 6; a published
`CatalogEntry` is optional and irrelevant to either.

**Answered by rules already stated.** Retry after a refusal is covered by
`light-client-requirements.md`'s existing floor — *treat a policy refusal as that
node's answer rather than that endpoint's* — which was written for an attach
refusal and reaches this one because it is stated by role rather than by message.
Nothing was added.

### 2026-08-28 (0.6.9 topology flooding and endpoint publication: an infra node could not republish its own address; adjacency, subject and *stored* defined)

The propagation layer's first implementation attempt. It found a deadlock in the
one object §5.6 exists to carry, and three normative terms doing load-bearing work
without definitions.

**An infra node that changed address could not say so.** §5.6's `seqno` is §4.3's
per-node counter, which §4.3 said was "incremented on every position change".
§5.6 replaces on strictly greater. §10.1 and §7.7.3 make equal `seqno` with
different contents **malformed** — reasoning, correctly, that a subject advances
its own counter to express a change. **A node whose address changed but whose
position did not could therefore advance nothing**, so its republished record
collided with its own previous one and was malformed by rule. It was stuck
advertising a dead address until it happened to move. This is exactly the node
§5.6 was written for: the infra node that neither peers nor anchors, whose address
reaches its patron by no other route. **The counter now advances on a position
change or an endpoint change** — one counter, two triggers. Nothing else moves:
§4.3's freshness rule was already *strictly greater, not previous + 1*, because
gaps were always expected.

**Republishing an unchanged set replays the held record** rather than spending a
number. Reconciliation is a replay of the same frames, so a fresh number over
identical contents is churn that can also race a real position change.

**Three terms carrying normative weight without definitions.** *Adjacent* appeared
exactly once, in the forwarding rule, and nowhere else in any document — two
conforming nodes could store the same objects and deliver them to different
neighbourhoods, which surfaces as a permanent gap rather than as an error. It is
now the authenticated sessions a node already holds by topology relationship:
patron, subordinates, peers. **Node type does not enter it, and must not**: type is
not a function of position at all, so a rule phrased on type would go stale on an
act that changed no topology — which is why a locator does not encode type either.
*Subject* was singular while peering has two endpoints, and reading it as the
*issuer* would put a patron's adoption of a distant node in range of everyone near
the patron. And *stored* was never tied to verification, so a literal reading made
every node an amplifier for whatever an authenticated neighbour sent; storing now
requires verifying, with `EndpointRecord` the exception the design already states.

**"Who may push" read as a rule on every sender stops flooding at the first
relay.** A relay is not a party to what it forwards. The paragraph governs
origination; the distinction is safe because a relay changes nothing — the object
goes on byte-for-byte and the signature that made it worth trusting at the origin
is the one the next receiver checks.

**Precision fixes.** §5.6 claimed a recipient learns "which gossip source supplied
it": it learns the immediate authenticated hop, since nothing on the wire carries a
path and a relay is indistinguishable from a publisher. That is enough — the
neighbour is the party you can stop listening to. The endpoint list is in publisher
preference order with **distinct** entries, and **port zero is malformed**; a
signed object two decoders disagree about splits the network on bytes rather than
on meaning. And `infra-client-requirements.md` §4.4's "treat it as unverified
gossip" was read by the implementation as *do not refer from it*, which would make
a live child unreachable through its own patron: a referral is not a credential,
the requester authenticates the subject it meant to reach, and a wrong address
costs a failed dial.

**P36 narrowed.** Endpoint changes now advance the same counter, so a `seqno` gap
has an innocent local explanation as well — but only for infra nodes, and only
against an observer outside the horizon, since one inside it sees the endpoint
records that account for the jump.

**Correction, author 2026-08-28: how a node becomes infra.** §12.6.1 said "a light
client that acquires subordinates becomes infra without moving", and
`light-client-requirements.md` repeated it. **That is false, and §3.3 had it right
all along**: a node may hold two full tiers of subordinates — up to 110 — as an
ordinary light client. Nothing about the tree promotes it. **You become infra by
launching and signing an infra instance**, and the effect runs the other way: your
grand-subordinates thereby unlock the ability to take subordinates of their own.
The conclusion the false premise supported is untouched and now rests on a true
one — type is not a function of position, so a locator must not encode it. **Client
type says which kind of endpoint you are connected to and little about topology**:
the root of a million-node tree still runs a light client on a phone and takes every
user action through it. The stale claim was not corrected in the earlier entry that
records it; entries are what was decided then, and this is the correction.

**One assistant gloss removed.** The italicised paragraph closing §2's
four-addresses vignette explained the analogy by calling a change of address "a hard
fork (§6.2)" — a term §6.2 no longer defines, since hard-fork departure and
forwarding are deferred by decision. The vignette above it is the author's and
stands.

**Sweep: "light client" was used 52 times and defined nowhere.** §2's vocabulary
defined *infra node* and left its complement to inference, and the inference a
reader naturally makes — a tier, or a class of person — is wrong in both halves.
The term now has a row: the participant-facing application, and by extension a node
with no infrastructure of its own; **every user runs the application, infra
operators included**, because that is where user actions happen.

**The three requirements documents read as three populations.** They are three
software roles. An infra operator is an ordinary participant who also runs
infrastructure, so their presence ceremonies, catalog browsing and resource
requests happen in a participant client like anyone else's — **a complete operator
deployment satisfies `light-client-requirements.md` as well as
`infra-client-requirements.md`**, which neither document said and both now do.
Reaching your own instance from your own client is user interface, not protocol;
the network sees one node.

**Two consequent fixes.** §13's bootstrap ordering spoke of "the light-client
tier" — there is no such tier, only a node that does not yet run infrastructure.
And `light-client-requirements.md` §7 rendered the empty-residual distinction as
telling a user *what kind of party* they are contacting, which invites exactly the
status reading being corrected: it tells them **which endpoint answered**. The root
of a large tree takes their own actions through a client like everyone else.

**The remaining uses across the four other documents were checked and are sound** —
all of them the node sense (no static address, attaches to the nearest infra node
on its patron chain, cannot host), which the new vocabulary row now covers.

**Where an operator's key lives, settled 2026-08-28.** It lives on every device
they use, the infra instance among them — the relationship is a seed shared across
wallets, not a client and a server. §23.1 already permitted this ("devices may
share a key or hold their own; the choice is ordinary key management"), but framed
multi-device as a case for people who own several phones. **It is the ordinary
condition of everyone in §3.3's tier**, which the section now says. §7's
reliability distinction gains the mechanism it was missing: an infra participant
has no unavailability excuse because a device holding their key is up whether or
not they are.

**Correction, author 2026-08-28: nobody prompts a grandpatron.**
`infra-client-requirements.md` §10.1 required an operator to be prompted for every
`SubtreeAck` and forbade auto-signing, arguing that an automatic signature
"recreates the situation the mechanism exists to correct". **That was an assistant
elaboration and it is wrong.** §11.2.1 never asked a grandpatron *user* to
countersign anything: the deliberate human act in the sequence is the **patron's**
adoption, and in-horizon propagation of membership follows from it automatically.
The grandpatron's decision is a **policy**, set once and asynchronously to any
traffic it governs, which their node then applies without asking. The bullet is
replaced, §11.2.1 states the policy-not-prompt shape so the error cannot be
re-derived from it, and its "offline, slow or simply uninterested" — which imagined
a human declining to act — becomes a node that is offline or a policy that does not
reach the position.

**The automation principle, stated for the first time (author, 2026-08-28).** The
document set had no rule about when a client may spend a user's attention — three
scattered touchpoints across five documents, one of which was the prompt rule just
removed, and nothing an implementer could reason from. Appendix A now carries *What a
client does without asking*: **infra operation is automatic** — routing, queuing,
countersigning, acknowledging subtree membership, replication and issuing resource
credentials — and **a user wanting less sets policy in advance rather than being
interrupted**, which is §16.1's pluggable-policy shape applied to attention instead
of to trust.

**Hands-on authorisation covers two things.** A **live interaction in which you are
one of the people being present** — a presence ceremony, an adoption on either
side, and affirming in person that you recognise someone whose key is rotating; and
**an operator configuring their own node and resources**. Everything else runs from
what the user already decided.

**The criterion had to be sharpened, and the correction is the useful part.** The
obvious test — *does this sound like something a person does* — gets witnessing and
verifying backwards, and both are automatic. A witness's **client** observes the
ceremony, tests the evidence at each point, checks the timing and signs, while the
human who owns it is very likely unaware the ceremony happened or who was in it. A
verifier's client compares the presented profile against a picture it already
holds — the machine analogue of recognising a face — and its operator is neither
asked nor told; they know the person exists, having met them, and learn nothing
about this encounter. **The real test is whether your own presence is what is being
claimed.** A witness attests what its client saw, a verifier attests what its
client holds, and neither asserts that its operator was anywhere. Both facts are
now stated at §7.1 and §7.3, where an implementer would otherwise build a
prompt.

**The verifier notification runs to the subject, not to the answering operator**,
and §7.3 now says so. The asymmetry is deliberate: the subject is the only party
who can see probing spread across many verifiers (§7.4), while telling a
verifier's operator about each query would tell them only who is meeting whom.

**Both client documents point at it**, and every existing warn/confirm obligation
was checked against it — departure warnings, identity-path warnings, the
move-consequence reminder, the bootstrap disambiguation prompt and §14.1.5's
doorbell all sit on deliberate acts or are not authorisation prompts at all. None
needed changing.

**§7.4's per-subject probing limit was describing a superseded mechanism
(author, 2026-08-28).** It said verifiers notify the subject, the subject
aggregates, and *the subject withdraws disclosure permission* when probing is
detected — a reactive policy act, and the "(below)" it pointed at described no such
standing permission. **The real lever had already been built and the section never
mentioned it.** A verifier's captures of the subject are sealed under keys only the
subject can derive (§7.5.2), and `wire-format.md` §7.3's `KeyGrant` releases one
bound to a single `query_id` — so a verifier can evaluate **nothing** until the
subject's client sends the grant. The limit is **structural rather than vigilant**:
not a permission a subject must notice they should withdraw, but a key their client
declines to send. Fifty parallel probes need fifty grants. The notification stays,
now doing visibility rather than enforcement.

**The same correction settles the earlier question about a go-ahead round trip.**
There is one, and it was already specified: the verifier waits, not for permission
to answer, but for the key that makes answering possible.

**The L = 2 allowance does not compose (author, 2026-08-28), and L itself does not
change.** §21 already defined L as *non-infra subordinate levels* with the value 2
giving 110, which is what the author's §3.3 heading says. What was wrong sat on
top of it: Appendix A.2 gave each of an infra node's children its own two levels,
so the allowance compounded into three tiers and 1,110 as of right. **The levels
are counted from the infra node.** A node one level down may hold subordinates and
its subordinates may not, because a third level lies outside every infra node's
horizon and nothing can acknowledge it into a resource table.

**A third level may still exist, at the serving operator's option**, receiving
network services and not the resource function. This is not a restriction anyone
imposes: messaging, presence, adoption and trust need no infra node to *hold* the
requester, while resource access does, so "reachable but not a full user" is what
falls out of the horizon rather than a rule added to it. It needs **no new wire
type** — only a difference in what the last infra node in a chain caches and
forwards — and it is now an optional feature in `infra-client-requirements.md` §1,
with the obligation to say whether you serve it, since a user who cannot tell will
read absence as breakage.

**§3.3 carries the author's reasoning verbatim, as a titled blockquote in his own
voice** rather than a numbered subsection — set off from the specification voice
without being enrolled in the vignette machinery, since it states intent rather
than illustrating a mechanism: memberships that are not cheap to give away, a fanout too
small to build an influencer base on, saturated trees that distribute leadership
instead of concentrating it, and slow selective growth as the thing worth buying
entry to. None of that is derivable from the protocol, and without it the bound
reads as a capacity workaround.

**The Sybil symmetry breaks, in the attacker's favour, and §17.2 now says so.** It
claimed *attacker cost per fake identity equals honest cost per real identity*. An
attacker serves its own infrastructure and always enables the optional third level;
an honest operator may decline. So the attacker yields **f^(L+1) − 1 = 999** per
infra node against an honest **f^L − 1 = 99** — roughly a tenth the cost per
identity. This **strengthens** §17.2's conclusion rather than damaging it, since the
section exists to say topology cannot provide Sybil resistance and §17.3 carries
the actual defence. The closed form is `f^n − 1` for n levels, which is why both
figures land one short of a round power; the old text's "f^(L+1) exactly" was the
reach figure rounded.

**Numbers swept**: §17.1, §17.2's table (honest servers @1M rises tenfold: 125,000
/ 42,000 / 10,000 / 2,500), §16.6, §12.6.3's relay pricing, §21's L row and
Appendix A.2, rewritten and retitled — *110, 1,110, 99 and 999 are four different
quantities*. **Two sites deliberately untouched**: §15's `h = 3` process-and-discard
horizon and §21's `h_process` row both read ~1,110 from 1 + 10 + 100 + 1,000 and
have nothing to do with L. A find-and-replace would have corrupted both.

**Fake-subtree economics removed from the security analysis (author, 2026-08-28).**
It kept resurfacing as a security consideration because the document invited it to:
§17 opened with a section titled *Fake subtree cost*, priced an attacker's tree to
three significant figures, and only then said in §17.2 that topology cannot provide
Sybil resistance at all. A reader — or a reviewer, or an assistant — meets the
arithmetic first and treats it as a parameter. **The fix was structural, not
verbal.** §17.1 is now *Standing comes from edges, not from nodes*: standing comes
from the edges a given observer can see into an attacker's region (§16.2, §16.3.1),
a fake subtree buys appearance and deniability but never trust, and the cost of
building one is capacity arithmetic recorded in Appendix A.2 **and only there**,
because pricing an attacker's tree invites the reader to treat the price as a
defence. §17.2 keeps the symmetry argument and the `f` table with its
fake-identity columns removed — what f trades is depth, path length and sibling
factor.

**No register moved.** A10's subject survives — §17.2 still claims attacker
economics are worse than parity, now for two reasons — and A19 is repointed from
"§17.2–14.3's attack economics" to §16.6's operator pricing and §17.3's
static-addressing leg, which is where those figures actually live. Appendix A.2's
999 row loses its "§17.1 uses this one" gloss and is capacity like the other three.
§20.1's *lowering fanout for security* row now reads "at least the same factor"
rather than "exactly", the one place the old parity claim survived.

### 2026-08-28 (0.6.10 rootward memo and cycle detection: the cycle predicate fired on every normal hop; two operations still cannot be expressed)

The last of the
0.6 targets, and the propagation layer's second pass. Four defects, two fixed and
two needing a protocol decision.

**The cycle check was inverted.** §10.2 said *if field 2's path contains your own
position, you are your own ancestor*. A memo travels up patron edges, so on every
legitimate hop the receiver **is** an ancestor of the patron it names and its path
**is** a prefix of that patron's — containment therefore reports a cycle on ordinary
traffic, at every hop, forever. The coherent reading is **equality**: field 2's
anchor and path equal your own, meaning the memo left you going up and reached you
from below. The locator's `seqno` takes no part, since an endpoint change advances
it (§4.3) without moving the position and would hide a loop from a whole-locator
comparison.

**A removal was a deletion, and deletions forget.** The table is
`subject → (location, seqno)` with no removal state, so `added = false` had nothing
to store. Deleting the row discards the counter, and §4.3's *absence of prior state
is not a failure* then lets an older addition arriving late reinstate the mapping it
had already superseded. A removal now replaces the row with the former location and
the subject's counter — a tombstone, local, no wire field.

**Two receiver rules were stated as properties.** *A memo never leaves its subnet*
is only true if a receiver drops one whose anchor is not its own, and §10.2 said
nothing about what to do with it. Nor did it inherit §10.1's no-ack/no-retry
decision explicitly. Both now stated.

**The memo was rebuilt around the patron, not the subject (author, 2026-08-28).**
The disavowal problem looked blocking — field 4 wanted the subject's counter, a
disavowal is the patron's act alone and carries none — until the author asked why a
disavowal should carry anything from the disavowed node at all. **A subordinate
reaches the subtree on its patron's authority and ceases to exist from the
subtree's point of view when that authority is withdrawn.** The memo is now a
patron's statement about one of its own slots: `{patron, patron's position, slot,
timestamp, ? occupant}`, with an absent occupant meaning a departure or a
disavowal. **Every field is one the patron has authority over**, so nothing in the
object comes from a party that did not sign the transaction behind it, and the
question that started this does not arise.

**It closed three things at once.** The subject counter is gone, so the disavowal
ordering problem goes with it. The patron's *keyhash* is now present, which settles
a defect the review raised separately: the old memo carried the patron's locator
but not their identity, and siblings share a position, so a root holding two memos
about one node could not tell which sibling was the patron. And **the reason code
stays in-horizon** — a root accumulates that a membership changed, never why, which
is the same restraint that keeps peering out of this class.

**The cycle check became an identity comparison.** With field 1 naming the patron,
a node that receives a memo naming itself, from below, is its own ancestor. No path
arithmetic, no anchor comparison, and nothing that a counter moved for an endpoint
change can disturb.

**Ordering is by field 4, the underlying transaction's own timestamp, copied.** The
comparison is always within one patron and one slot, so **no clock is compared
across nodes** — two memos about one slot were written by the same patron from the
same clock, which is the one case where a timestamp orders reliably. Equal
timestamps break by arrival. Copying the transaction's value rather than reading a
clock at composition time also means a detector that fetches the record can check
it.

**C19 substantially reduced.** Its join was the memo table against the per-node
counter; a memo no longer carries the subject's counter, so an ancestor sees patron
counters, which say nothing about a subordinate's out-of-subnet activity. What
replaces it is smaller — field 4's timestamp, retained per current row rather than
as a history, so an ancestor holds when each slot last changed and not a series.

**The unformable confirmation fetch dissolved with the same change.** §10.2 required
a detection to be confirmed by fetching the underlying transaction via §7.9, and
§7.9 needs an archive head that reaches you only by having adopted the subject
(§3.1 field 7) — so the rule was unperformable for exactly the distant cycles the
memo exists to catch. **It is also unnecessary**: the cycle check fires only on the
party field 1 names, who signed the transaction and holds the slot, so confirmation
is a local read. A fabricated memo fails it at no traffic cost.

**Replay, and the rule that closes most of it (author, 2026-08-28).** A replayed
memo passes the fabrication check, since the change it describes really happened.
The author's observation was that it is recognisable anyway to anyone holding the
later state — the adoption carries the subject's counter and the disavowal carries
none, and a slot row carries the timestamp of its last write. So **a node holding
that slot at or after the memo's timestamp no longer forwards it**, which stops a
stale replay at the first table-holding hop above wherever it was injected instead
of letting it reach the party it names. The previous rule forwarded unconditionally
on the reasoning that an upstream table might not have seen the memo — true of
genuine out-of-order delivery, false of a replay by definition.

**What survives is registered as an accepted risk** (§18): a replay of the memo
describing the *current* slot state matches the detector's own row and cannot be
told from a genuine loop. Bounded on three sides — the injector must sit at or
below one of the detector's direct subordinates, the edge severed is the one that
handed the memo over and so lies on the injector's route, and the disavowal is
reason code 5 with re-adoption available. A freshness nonce would put a second
clock on an unsigned object to defend against an attack costing the attacker more
than the target.

**The 0.6 programme closes with nothing open in it.**

### 2026-08-28 (0.5.2 unenforceable mandates: two of five held, and the three that did not are evidence the Appendix A vocabulary is working)

The pass looked for rules
aimed at parties the issuer shares no state with.

**A sentence that read as two rules, one of them a wish.** `wire-format.md` §4.5.2
said *no exchange may demand a disclosable field as a condition of proceeding*,
which is true as a claim about the schema and unenforceable as an instruction to
recipients — a recipient may always refuse, and its acceptance policy is private.
The structural reading is the one the author has already stated (*policy is
pluggable, interfaces are not*), so the sentence now says it outright: **there is
nowhere in a fixed interface for "disclose this or we stop" to be expressed**, an
implementation that tried would be non-conforming at the wire, and what that buys
is that a refusal is the recipient's policy rather than the protocol's.

**One bare `must` where its own sibling site got it right.** §23.1's *backups must
honour declared retention* is the same obligation as §13.7.1's scan-on-import,
which is correctly written as *the reference client must* and followed by an
explicit honest-scope paragraph. §23.1 now matches.

**Three findings did not hold, and why is worth recording.** The retention scan,
the queue's crash-copy rule and §15's ban on background scoring were all flagged as
unenforceable mandates. They are unenforceable, and each is **already typed that
way**: two use Appendix A's sanctioned *the reference client…* form, and the third sits in
`infra-client-requirements.md`, whose header states that its contents are
commitments rather than enforceable rules. §13.7.1 goes further and says so in
place. **The vocabulary Appendix A introduced is doing its job** — a reviewer reading for
over-strong mandates found the form and read it as one, which is the failure mode
the form exists to prevent, one level up.

**The one real defect was a wire gap wearing a mandate's clothes.** §7.4 required
template formats to be canonical and versioned, but `VerificationQuery` carried the
fuzzed profile with **no indication of which scheme produced it**, while
`VerifierResponse` field 6 carries the version the *verifier* used. The version was
named on the way back and not on the way out, so a verifier running a different
extractor could not know it could not compare: it compared anyway and signed a
`no-match` indistinguishable from an identity mismatch, permanently portable.
**The version is now query field 5**, and `query_id` renumbers to field 6 and
hashes fields 1–5. No new result code was needed — a verifier that cannot compare
answers `3 unavailable` with no basis, which the schema already permits and which
§7.4's *a verifier who has not evaluated asserts no evidence basis* already
describes.

**Inside the hash, deliberately.** A version outside `query_id` could be altered
after signing, so the subject would have countersigned a comparison without
consenting to the terms of it. Placing it at field 5 rather than appending it keeps
the scheme within what consent covers.

### 2026-08-28 (consistency audit of the seven commits 4afe910..b80e257 against the whole set: nine mechanical completions applied, five items for the author)

Decided changes whose sweep missed a site, each verified against the current text
before edit.

**The counter's own definition missed the 0.6.9 trigger change.** Design §6.2.1 —
the authoritative sentence — still said the sequence number is signed "on every
position change", and `wire-format.md` §8.2's heartbeat comment still said a
locator seqno "changes only on position change". Both now carry the endpoint
trigger. The sweep at the time updated §4.3, §5.6, infra §3.4 and P36 and missed
the definition site itself.

**§10.2 still ordered the fetch it had abolished.** *What a detecting node does*
said the disavowal follows "once the transaction is fetched and confirmed", four
paragraphs after the fetch requirement was replaced by local confirmation. Aligned.

**§23.2's blocking item and the front matter were stale, exactly as CLAUDE.md
predicts of consolidating sections.** Both said §8.2's request/response path had no
implementation attempt; 0.6.7 and 0.6.8 were implementation attempts against
exactly that path. Nothing currently blocks a subsystem, and both sites now say so.

**Two "up to 110 subordinates" claims survived the L change** (§12.6.5.1's
censorship-signal and pre-delegation paragraphs) — the old composing reading, under
which a light client could hold two full tiers. Now f = 10. The pre-delegation
mechanism itself is untouched.

**The catalog truncation argument miscounted its own bound.** It said an answering
node "hosts at most f = 10 subordinates", but an answering node answers for its
whole served subtree — itself plus ≤110 full users (§11.5, §12.6.1) = 111 owners,
which is the bound. The corrected argument is exact where the old one was wrong in
the design's favour by accident.

**The owner tie-break's justification was falsified by a later rule in the same
section.** 0.6.6 justified sorting by (resource, owner) because two owners may
share a keyhash; 0.6.7 then made a host refuse a second claim on a keyhash it
serves, making keyhash alone total per host. The composite key stays — it costs
nothing and survives any future relaxation — and the justification now says that
rather than the falsified claim.

**Also: scope-self withdrawal returns the entry to the owner** (Scope 0 is the
owner, so "no asker satisfies" was wrong by exactly one asker — two sites); and
A10's consequence column claimed its failure restores exact 1:1 cost symmetry,
which the optional-third-level asymmetry now survives.

### 2026-08-28 (the third tier removed entirely; adjacency extended to the serving relationship; the downward memo's branch made trigger-relative; four stale citations remapped)

Author: *"Remove the third tier entirely. Don't make it
work, it is out of keeping with the design concept."* The audit had shown the
optional level could not be served without extending control adjacency to it, and
rather than confine the extension he removed the thing needing it.

**All traces gone**: design §3.3's second bullet, §12.6.1's reachable-not-full
paragraph, §12.6.3's and §16.6's 1,110-reach clauses, §17.2's second asymmetry (the
section is back to packing alone, and A10's consequence column reverts with it),
§21's L row qualifier, and `infra-client-requirements.md` §1's serving-option
bullet. Appendix A.2 is again two quantities — *110 and 99* — with the superseded
readings named in its closing note. **"Full users" reverts to "users"
everywhere**, the qualifier's contrast class having been removed with the tier.
§3.3's first bullet now states *why* no third level can exist — outside every
infra node's horizon, nothing can acknowledge, serve, or connect it — which is the
author's original L = −3 argument in its final home.

**Adjacency includes the serving relationship, both directions.** Removing the
tier did not dissolve the gap the audit found: a tier-2 node is a full user whose
patron holds no sessions, so its horizon view and its own departure flood still
had no path. §10.1's *adjacent* now names the serving node and attached clients
alongside patron, subordinates and peers — still only sessions the node holds
anyway — and states that the serving relationship is what carries control past
light-client patrons.

**Rootward memos collapse light-client patron hops through infrastructure.**
Where no session with the patron exists, the memo goes to the nearest infra node
on the patron chain; a skipped patron's optional table has gaps, which *no tier is
load-bearing* already prices. **A serving node runs the cycle check for its
attached clients as well as itself** — it holds its whole subtree, so it checks
field 1 against itself and every attached client, and a hit for a client is handed
to that client at contact: the confirming records and the disavowal are the
client's, not the serving node's.

**The downward memo goes down the branch the arriving memo did not name** (author).
Trigger-relative and clockless: the arriving memo names one slot, the table holds
the other, and the memo goes toward the patron who has not just spoken. This
replaces "toward the stale position", which the per-patron ordering could no longer
define — whether the edge is stale is the receiving patron's own records' business,
and the "stale-edge holder" language went with it.

**`VerifierResponse` field 6 needs no equality rule** (author): it records which
algorithm produced the verdict for evaluators who never see the query; nobody
tests it to validate the record.

**Four stale `design §7` citations in wire §2 remapped** — resolvable, so
invisible to the checker, and pointing at Proof of Presence since some old
renumbering. Truncatable paths, routing-information authentication and mailbox
substitution live in design §12.1; the monotonic counter in design §6.2.1.

### 2026-08-28 (0.7 LINDDUN privacy: the three findings the author answered — the resource front door, historical catalog resurrection, and the currency query)

A clean-room LINDDUN pass over the six documents. Its three items not already in
the P/C registers were put to the author; all three answers corrected text that
had overreached rather than adding mechanism.

**NEW-1, the resource front door.** §14.2's table said a leaf-to-resource message
is plaintext only at the resource, while §11.2 requires the hosting node to parse
and re-serialise the HTTP to insert the credential. **The node cannot both insert
an authenticated credential and be unable to read what it inserts into**, and the
table now says so — splitting the row into *through its host*, where the node is an
endpoint and reads the request, and *directly*, where a `CatalogEntry` endpoint is
reached point-to-point and no node is in the path. The visibility itself was
already stated at §11.2 and P24; only §14.2 denied it.

**The far leg is now required rather than assumed** (author): **HTTPS wherever it
crosses a network**, plain HTTP permitted only on a local socket to a package the
node hosts itself. `resource-requirements.md` §3 previously described this
("a local socket for a hosted package, ordinary HTTPS for an external service") as
deployment fact; it is a requirement. What it protects is everyone *else* — an
operator relaying to a resource elsewhere must not put an authenticated principal
and an application body on the open network.

**And the first correction of it overreached the other way (author,
same day).** Splitting §14.2's row into *through the host* and *direct* made
relaying the assumed shape, which inverts §11.7's stated default: **broker rather
than proxy** — a gateway that only authenticates and hands off leaves the user
connecting directly, and one that carries the traffic becomes a content
chokepoint. The author's point was broader than the catalog endpoint I had cited:
**a light client runs on an ordinary network-enabled device and makes its own
outbound connections**, and a third-party adaptor could make the node an
authenticator to a service the client attaches to directly — the SSO shape §1.3
already describes. §14.2 now has three rows, hosted / brokered / proxied, with
*brokered* marked as the default and carrying no application traffic across the
node at all; §2's far-leg row is scoped to *where the node carries the traffic*;
and the HTTPS requirement applies to that case rather than to every resource not
on the node. **What a resource does is deliberately not enumerated (§11.4), so how
it is reached must not be assumed either.**

**NEW-2, historical catalog resurrection — the finding dissolved on an archive
correction.** The reviewer found that superseded registrations survive in the
append-only archive and could be disclosed in a prefix presented to a new patron,
exposing services long after withdrawal. **The archive never included them.** §10's
definition is *adoption, departure, disavowal, peering and presence* — a resource
registration is none of those, and the retention claim came from `wire-format.md`
§6 and `infra-client-requirements.md` §11 over-generalising to "it keeps
everything". Author: *the archive is only for topology transactions and PoP; the
owner keeps a live table and generates responses from current state; there is no
requirement to keep records of past resources.* §10 now states the exclusions and
why they matter — anything travelling with an archive is presented to every future
patron along with it, which is the wrong audience for what a person runs and the
wrong duration for what they used to run. §15's Catalog class row, §6.5's
supersession and withdrawal paragraphs, and infra §12's bullet all follow.

**NEW-3, the currency query — specified.** §12.6.5's stapling design falls back to
"a lookup" that had no message, audience, or retention rule, and the fallback
undoes the exact property stapling exists for: it tells the subject's patron that
someone is being introduced to their subordinate. Now `wire-format.md` §7.1,
**bidirectional request type 8**: `CurrencyRequest {subject, nonce}` and
`CurrencyReply {nonce, result, ? attestation}`. Three bounds, each following from
something already decided. **The query names the subject and not the querier** — it
is the authenticated session peer — so a reply forwarded onward attributes the
question to nobody. **Nothing is retained**: liveness class (§15), answered and
discarded, never archived (§10). **And a caller with a stale staple asks its
introducer first**, who already knows it is talking to the subject, where the
patron would learn something new. `Code 1` and silence are the same answer and both
fail closed, which is §9.0.2's distinguishability requirement met — an answer is
an attestation, and one naming the key the caller holds *is* "no rotation".

**§11.0's subsections were out of order and its taxonomy misfiled (author approved
2026-08-28).** The file ran 9.0.2, 9.0.3, 9.0.4, then **9.0.1** — so a sequential
reader met *wider reach is federation* after *credentials do not cross the boundary
twice*. §11.0.1's **number was already right**, so moving its block ahead of §11.0.2
fixed order and numbering together and **changed no citation**. The *Three
categories, one mechanism* table — local application, gateway, external service —
sat at the tail of §11.0.4, whose subject is credential audience-binding; it is a
taxonomy of the whole section and now sits in §11's introduction, with the
implementation-requirements pointer that belongs beside it.

**A document-wide heading audit found 51 level anomalies, left for the
organisation pass.** Depth-3 sections are written as `###` in 51 places and
`####` in the §11.0.x group, so the file has two conventions and the smaller group
is the one a strict renderer agrees with. Ordering is otherwise clean across all
141 headings. This is exactly the churn the review plan defers to 0.9, so it is
recorded rather than fixed.

### 2026-08-28 (de-linting pass; the type-6 transaction retired)

Two operations,
the first mechanical and the second a simplification the author had been weighing.

**De-linting: the specification stopped narrating its own drafting.** 47 dated
decision markers became bare `[D]`, leaving 275 doing the job the convention gives
them. Date tokens across the five root documents fell from 86 to **2** — the two
front-matter revision lines, which are document metadata rather than process
narration. The pass-archaeology instances went with them: §20.1's *"flagged by the
2026-08-14 factual verification pass"* (the author's own example), §20.2's *"from
the 2026-08-14 unjustified-claims pass, which returned 280 items"*, §19.8's
*"the 2026-08-16 LINDDUN re-run found"*, and §9's *"a review pass registered it
as a privacy cost"*. In each case the finding survives and the pass that produced
it does not.

**Drafting archaeology was judged individually rather than swept**, since some of
it earns its place. *An earlier form of this check asked whether the path
contained…* becomes **"Containment is not the test and cannot be"** — the same
protective content, stated as a rule rather than as history, so a reader cannot
re-derive the bug it warns against. §6.1's transfer transaction and §7.5.2's
`txid` derivation are reframed the same way. The one instance left standing is in
Appendix A.2, where superseded readings are the section's subject.

**A line-wrap trap caught 15 markers.** The first regex required a space after the
em dash, and `[D —\n  2026-08-28]` does not have one — so 15 of 47 survived the
first pass and were only found by re-counting. Counting after a mechanical
substitution is not optional.

**The type-6 transaction is retired** (author: *"there is no resource registration
in shared network state; resource queries are point-to-point communication within
the trust horizon"*). It follows from the archive decision earlier the same day:
a registration advances no archive, reaches nobody else, and chains to nothing —
so an envelope around it would carry **a back-pointer nothing walks, a `txid`
nothing references, and a post-quantum signature justified only by an archive it
never enters** (§2.2 ties hybrid signing to archive retention). A registration is
now the signed `CatalogEntry` itself: `ResourceRegistration` carries the entry
rather than an `Envelope`, the reply loses its `txid` echo, and the wrapper-versus-
inline hazard that echo existed to catch dissolves with the wrapper. **Abuse report
moves from type 7 to type 6**, and the transaction types are six.

**Correction, author 2026-08-28: nothing ever reports that a key was rotated.** A
rotation **evaporates the old identity in the local subnet and instantiates a new
one**, and the inheritance is not carried. Within the subnet the upstream sees a
disavowal and an adoption in quick succession and **cannot prove they are the same
person** — the arriving node may equally have displaced another from a slot.
**Beyond the horizon nothing is exposed at all**: the old locator is simply an
address that stopped working, with no explanation offered and none available.

**This corrected pre-existing text as well as new.** §9.0.2 and §12.6.5 both
required *"no answer from patron or siblings"* to be distinguishable from *"answer
says no rotation"* — a sentence that has a patron announcing a rotation to a
distant caller. Both now read *"answer attests this identity is current"*, which is
the only thing an attestation ever claims. §9.0.2's opening said rotation
propagates as a topology-class message; it propagates as **the transactions it is
made of**, a disavowal and an adoption, within the horizon only. §15's *push near,
redirect far* bullet said callers *learn lazily*; they learn whether an identity is
currently attested and nothing about one that is not. §9.0 now states the model
outright, since §5.1 and §12.6.5 both depend on it.

**And the correction surfaced a stale mechanism.** §9.0.2's staleness bullet said
a cached attestation is *"corrected on next contact via the forwarding record
(§12.3 Case 2)"*. The forwarding record was removed with hard-fork departure, and
this was its last reference anywhere — invisible to the checker because §12.3
exists. Worse, §12.3 Case 2 says the **opposite** of what the citation claimed:
*"Resolution fails. Nothing redirects on Alice's behalf, and no party holds a
pointer to where she went."* The bullet now says what actually happens, which is
also what the author's model predicts: the holder re-resolves or re-establishes
socially, and learns nothing about why.

**The currency-query residual is accepted rather than registered** (author: *"this
is fine"*), and is §19.7 item 12. What remains after the bounds is narrow: a
patron learns that someone asked about their subordinate. The reply discloses
nothing further — an attestation says only that an identity is current in its
issuer's subnet, never reporting a rotation or naming what replaced anything — and
that residual is the price of being able to see a fork at all (§9.0.2). No
P-number allocated, so no register number was spent on a cost the design chooses.

**And the correction needed qualifying against the recovery case.** Transcribing
the author's model into §9.0 as written would have overstated it: a *plain*
rotation carries no link, but a **recovery** adoption publishes `prior_key` plus
verifier continuity attestations deliberately (`wire-format.md` §4.1) — which is
what §19.7 item 3 has always accepted as recovery destroying unlinkability. The
section now separates the two and says the publication **reaches the horizon and
stops there**; beyond it neither case exposes anything, which is the part of the
author's statement that holds universally.

### 2026-08-28 (0.8.1 adversarial: three findings, all confirmed against the construction)

A clean-room adversarial pass, discarding eight restatements and
returning one novel attack, one failed acceptance rationale, and one extension of a
known gap. All three verified before edit; all three held.

**A patron could recover its own victim, and the design's own text shows where the
model was thin.** Nothing required the verifier in a `Recovery` block to be
distinct from the adopting patron, and a recovery resting on verifier responses
needs **one** `match`. A patron is ordinarily a legitimate prior counterparty — it
met the subject at adoption — so it can assert `basis: personal_knowledge`, which
rests on the verifier's own memory and nothing checkable, without forging anything.
It mints a keypair it controls, signs the response, signs the adoption, and the
subject-consent signature is by the **new** key: §3.1 already says so outright —
*"field 7 is signed by the very key an attacker mounting a fraudulent recovery
already controls"*. The consistency rules defend against a **forged** verifier
attestation, which is why field 9 is hybrid; they never contemplated a **genuine
verifier that lies**, and the same party occupying both roles is what makes lying
free. **No response in field 2 may now come from the adopting patron** — checkable
by comparing field 1 against the adoption's patron, and it costs a legitimate
recovery nothing, since design §9.1's attestations are supposed to come from
counterparties trusted *independently of the adopter*.

**§18's escape hatch answers a patron that withholds, not one that acts.** The
entry says unilateral departure plus adoption elsewhere escapes censorship, and the
patron's cost is a fleeing down-line. Against **retaliatory disavowal** that cost is
**zero**: the subordinate is already leaving and the slot refills. Nor is there any
ordering to appeal to — a departure advances the departing node's counter, a
disavowal advances the patron's and carries no subject counter, and timestamps are
signer-controlled and never checked against a clock. §18 now carries the case
separately, and states what actually limits it: an observer holding a departure and
a with-prejudice disavowal minutes apart holds an evaluable pair, and a contested
exit reads differently from an ordinary disavowal. Visibility, not enforcement,
which is §1.1's usual answer — and the record stays durable regardless.

**§12.7.2 dismissed the leaf case with an assumption disavowal breaks.** *"A root
without [a down-line] is a lone identity whose currency nobody has occasion to
check"* is true of a Genesis user, who claims nothing. A **disavowed leaf** has
history, and §12.7.1's own rule makes a staple required precisely because it claims
prior standing — with no down-line to attest from below and no patron from above.
**Adoption is itself trust-bearing**, so the operation that would restore an issuer
is gated on already having one. What distinguishes this from the availability gap
Appendix A records is that **a patron can manufacture the state deliberately**: one
disavowal freezes a leaf once its last staple expires, and the only escapes are a
second binding established beforehand, a non-conforming evaluator, or returning as
Genesis and abandoning the history — denial of service converted into destruction
of portable standing. *"Re-adoption is available"* is therefore not a general answer
to patron abuse, and §18 no longer reads as offering one.

**Author, same day: there is no lost-key variant, and recovery is key *and*
presence.** The adversarial finding was closed by removing the case it exploited
rather than by constraining it. **A rotation now carries both halves and always
did in intent**: field 3, the old key's own signature, and field 2, at least one
`match` from a prior counterparty met in person. Neither substitutes for the
other — **a thief holds the key too and could rotate with it; a verifier can be
mistaken or lying and nothing checks it** — so an attacker needs the secret *and*
a person willing to assert a meeting that did not happen. A key you cannot sign
with is not recoverable: you make a new identity and are re-adopted by people who
know you, which in a network of close acquaintance is faster than any mechanism
here.

**The separation rule added an hour earlier is withdrawn**, and was wrong. Barring
the adopting patron from supplying a `match` would have blocked the most natural
legitimate path — in a network of close acquaintance your patron is very often
exactly the person who would recognise you. With both halves required the rule
buys nothing, because a patron who cannot sign with the victim's old key cannot
mount the attack at all.

**The residual is accepted and stated** (§9.0.1): a patron who steals your key
satisfies both halves in its own subtree, holding the key and asserting the
recognition. **That subtree is effectively its property** — if it wants a straw man
wearing your name, the answer is to go elsewhere and make new friends, which the
exit argument already assumes you can.

**And `personal_knowledge` was inverted, not weakened.** The attack read it as the
soft option because nothing checks it. It is **the ordinary basis for a rotation**:
a template match is a machine agreeing two images resemble each other, where
recognition is somebody who knows you saying so. §9.1 now leads with the human
judgement and treats the stored photo as support for it.

**Swept**: `Recovery`'s schema (both fields required, `[+ …]` grammar), the
threshold logic and the optionality discussion it forced, the two-variant table in
design §9.0.1, the replay-primitive argument's dependence on an absent field 2,
§9.1's procedure, §9.3's weak-tie boundary, and the retention-tier argument,
which now rests on there being nobody within reach to recognise you rather than on
a missing quorum.

**Two-tier capture retention removed entirely** (author). One window, one seed, one
key, one grant. The 5-year derived-template tier is gone, `k_template` and
`k_images` collapse to a single `k_capture`, `KeyGrant` loses its optional second
key, and `pN.retention` goes from `[photo_years, template_years]` to one `uint`.
**The tier had one consumer and lost it**: §9.1 called weak-tie recovery the only
argument supporting the longer window, and with rotation resting on recognition
rather than a stored template there is nothing on the other side of the cliff to
reach for. §7.5.1 now states the cliff as accepted rather than softened, and says
why: a second window buys detection by keeping biometric material for years after
the images are deleted, and doubles every declaration, key and grant.

**The consequential simplifications are larger than the parameter change.** The
normative segment *ordering* rule existed only because writing images first would
"silently convert a template-only grant into an image grant" — with one key there
is no such conversion and the rule goes. So do the graduated-capability framing,
the tier-release obligation in `light-client-requirements.md`, and *"allow
template-only storage for users who accept version lock-in"*.

**"Segment key" was itself an oblique trace** and is renamed to **capture key**
throughout — 16 sites across three documents, plus §5.3's title and the *"two
segments, two derived keys"* construction heading. Segments implied separately
keyed parts, which is the design being removed; a reader meeting the term would
have inferred a structure that no longer exists.

## 2026-08-29

### 2026-08-29 (witness adversarial review 0.8.2; cross-nomination refiled)

An
external pass took one witness at a presence ceremony as the adversary and returned
three findings. All three held on verification, and the root of all three was that
**the witness had never been modelled as a principal**: searching both documents,
every witness-related security statement treated witnesses as the *instrument* of a
participant's attack — "colluding witnesses", "friendly witnesses" — and none as a
party acting on its own behalf.

**Cross-nomination was doing the wrong job in the documents** (author). `wire-format.md`
§5.2.1 said the anti-grinding property "rests on the witnesses being honest, and
**cross-nomination is what supplies that** … the same bar as forging the ceremony
outright." That contradicted §17.2 — no topology rule can supply Sybil resistance,
and cross-nomination is a topology rule — and the "same bar" claim was false besides:
forging needs a participant's signature, grinding needs a nominee. The author's
correction is that cross-nomination was never for honesty. **It is for
representativeness of the sample.** Witnesses would ideally be drawn at random from
the whole userbase; no node can enumerate it, so each party draws from what it can
see in the counterparty's neighbourhood, and in a balanced set each party knows half
the witnesses are its own nominees and therefore uncurated by the counterparty.
§7.1 step 1 and §5.2.1's closing now say that, and say what it leaves open.

**A one-witness record is where the property vanishes, and it stays valid.**
`wire-format.md` §3.2 requires at least one witness and requires `nominated_by` to
name a participant; it does not require both participants to be represented. Where
one party nominated the whole set, the other has none of its own nominees present —
and since the seed's only input the participants do not fix is the witness nonces
(§5.3), that party's verifier sample derives entirely from its counterparty's
nominees. A sole witness can therefore commit to a nonce whose sample it has already
computed. **Left visible rather than forbidden** (author), on §6.1.1's precedent: a
balanced set is what a party should insist on, not what the encoding can require, and
a meeting where only one other node could serve is a real case a hard rule would
invalidate. `nominated_by` is in the record, so the split reads directly, and
`light-client-requirements.md` §1.0 now requires the client to surface it. **Two or
more independent witnesses close the seed attack with no further rule**: commitments
are published before capture and reveal nothing about their nonces.

**"Volunteering" was the wrong word and is gone** (author). §7.1 previously opened
"witnesses signal availability", which describes candidates bidding. What actually
happens is the reverse: the nominator selects — with a random element, and spread
across as many independent branches of the counterparty's graph as it can, since
catching a spread selection needs a correspondingly larger fake region — and *then*
probes, because many nodes worth nominating are inactive or light-client-only and
cannot serve at ceremony time. Availability is discovered by the nominator, not
advertised by the candidate. New **§7.1.1** carries the procedure and the residual.

**§18's Potemkin justification was falsified and is replaced.** The accepted risk
said a fake region "harms nobody who is not engaging with it." Witness eligibility is
not a function of standing, so a node placed by one boundary adoption — valid without
a proof of presence, §6.1.1 — sits in a neighbourhood *someone else's counterparty*
nominates from, and the C1/C2 disclosures and a withheld signature land on a party
that never engaged with it. The risk is still accepted; what bounds it is the branch
spread of §7.1.1, not the flow metric.

**The witness's selective abort is now registered** in §8.1.2, which previously
analysed only a participant aborting. A witness reveals after the capture is spent
and signs last, so it decides twice with a meeting already paid for by two other
people; a fresh nonce on retry violates §5.2.1 and is invisible in a completed
record. The design's usual answer is unavailable here and that is the sharp part: an
attempt that never finalises is never published, so there is no artifact to
attribute. It remains a denial of service against one pair, not a route to false
evidence — the three defences the pass confirmed (no forged record, no rewritten
verifier response, no capture from witnessing) all held.

**Also**: the "one check that protects you against the person in front of you" in
`light-client-requirements.md` §1.2 is now two, since the nomination-split check is
the same shape; §19.2's "an attacker choosing friendly witnesses" scoped to
*participant*; nine duplicated-word artefacts fixed (`(design design §…)` ×8 across
the two client documents, and §13.4's "a a choice"), all of them line-spanning and
invisible to a single-line grep.

**The `[D]` provenance markers are gone — 289 of them across the five root
documents** (author). The de-linting pass of 2026-08-28 stripped the dates and kept
the markers; this removes the markers on the same reasoning, stated by the author as
the rule the earlier pass should have followed: *if something is undecided it belongs
in the `Robot/` set, and if it is decided it is just part of the current design.
Everything was decided at some point, so there is nothing to flag.* Removed: 220 from
`wire-format.md`, 65 from `network-design.md`, 3 from `resource-requirements.md`, 1
from `infra-client-requirements.md`. `wire-format.md`'s front-matter **provenance
block went with them**, along with the companion `[P]` marker it defined for
proposals "not yet agreed" — a marker with **zero instances in use**, its only two
occurrences being inside its own definition. Its removal costs nothing and closes the
question the block invited.

**Where the marker was the sentence's object it was rewritten, not deleted.** §3's
*"Both are **[D].**"* went entirely, the paragraph ending at its §11.5 citation; §5.2's
*"never a protocol constant**\n[D]."* took the period back onto the sentence. Four
standalone marker lines were removed without disturbing the blank line a following
table or heading needs. Verified after: no marker or `[P]` residue, no trailing
whitespace, no doubled prose spaces, and the four documents that had two `\n\n\n`
sequences each still have exactly two.

**Two doubled words fell out of the check that found them**, both predating this
round: §1's *"each with with varying rules"* and `wire-format.md` §7.3's *"a capture
capture key"*. The earlier duplicated-word sweep this session missed them because its
word list was restricted to the terms it expected; the unrestricted form found them.

### 2026-08-29 (adversarial review 0.8.3; the candidate set and the claimed day)

Four findings against a funded fake-standing operator. Two applied here; the other
two are described below and left for the author.

**Manufactured volume is worthless to an evaluator and not worthless to selection.**
§16.1 carried both halves of this two paragraphs apart — *"what the protocol supplies
is the sampling floor"* and *"volume proves nothing, so manufacturing volume gains
nothing"* — without noticing they collide. Candidate eligibility is structural:
`wire-format.md` §5.4 admits a prior counterparty when the record is canonical and
its signatures verify, and nothing weighs it. So a subject who manufactures
counterparties owns the population their own verifiers are drawn from, and the
2026-08-24 formation bound does not reach it — that capped *formation* records at one
per key, while normal records among controlled identities are uncapped and each adds
a candidate.

**What answers it is that the party who selects is the party at risk** (author's
framing: fabricated verifiers hurt the counterparty, not a later evaluator). §8.1.2
has each participant select the *other's* verifiers, and §7.3 has C send the query
to B's prior counterparty — so C enumerates B's candidate set in order to sample it,
and sees the population before it sees any answer. A sample drawn wholly from
strangers returns `match` from strangers. The intersection argument governs the
sample too; it is not suspended because the protocol chose it. §16.1's sampling-floor
paragraph now carries this, extending `wire-format.md` §5.7's holder-relative
property from checkability to evidential value, and §7.3's detection arithmetic now
states the precondition it always had: `1 − (1/k)^q` assumes the sampled
counterparties are honest and independent of the subject, and against a fabricated
candidate set detection is **absent rather than reduced** — no q repairs it, since
the confederates sharing the key are exactly who would answer.

**The seed window is claimed, not elapsed, so the rate limit was never real.**
`window_ordinal` derives from `started_at` (`wire-format.md` §5.3.1), which the
proposer chooses; §2.2's monotonicity against the committed back-pointer bounds that
**only from below**. The documents already named the mechanism — *"the same field
derives `window_ordinal`, so varying the claimed day also yields fresh verifier and
witness samples"* — and then treated the back-pointer as closing it. It closes
backdating, not rerolling. **The real budget is the span between the signer's last
committed record and the day it is willing to claim**: an identity transacting weekly
has a handful of ordinals, a dormant one has as many as it has been dormant. Future
ordinals are structurally valid too, at the cost of the signer's own forward
timeline. And the same choice moves the 730-day candidate horizon and *n*, so a
claimed day steers the seed, the pool and the threshold together.

**The witness's clock is what binds it** (author). A witness is the only party to a
ceremony with an independent clock and no stake in the sample, and it is asked to
commit while the ceremony is happening; the reference client declines to commit
against a claimed day far from the time it observes. Not checkable by any later
validator — a completed record carries no evidence of what a witness's clock read —
so it is a commitment in Appendix A's sense. **Cross-nomination does a second job here**: a
grinding participant cannot pick lenient witnesses because its witnesses are
nominated by the counterparty, and since every witness nonce feeds the seed, one
refusal denies the attempt. New `light-client-requirements.md` §1.0.1 carries the
witness obligations; §1.2 gained the candidate-population check, making **three**
checks that protect a signer against the person in front of them rather than two.

**Three stale instances of the overstated rate were swept**, two of them definition
sites: §5.2.1's own restatement of the property, and the §20 parameter register's
rationale for the 24-hour window — which asserted *"one fresh sample per day per
participant pair"* as the reason for the value. Rule (1) in §8.1.2 now states the
intent and points at why it is not delivered rather than asserting the rate.

**Percentile role predicates: described rather than defended** (author). 0.8.3's
third finding was that `resource-requirements.md` §7.2 recommends rank predicates
over raw scores — correctly, since a raw threshold means something different after a
metric-family switch — and that both its worked examples, *"top 20% of my Dunbar
org"* and *"above the median of my direct clients"*, have a cutoff that is a fraction
of a population. An org of 100 with its line at the top 20 admits its 25th member
once 25 further members exist beneath them, and the trust metric is right throughout:
it rates the padding as worthless, which is why it lands at the bottom and lengthens
the queue. The predicate never asked how trusted anyone was.

**The author's disposition is that this needs describing, not fixing**, on two
grounds. A predicate is *sugar over individual role grants* — design §11.4 already
says role assignment is a materialised table and predicates are a macro over it, so
the operator binds a role by reading an expansion into names. And the horizon is
deliberately sized to people the operator has at least passing acquaintance with, so
moving a quantile enough to matter means unfamiliar names appearing in a list they
read. **Statistical manoeuvring is a weak attack against a population recognised
individually.** New `resource-requirements.md` §7.2.1 sets out the five classes —
structural, tenure, named, absolute rank, relative rank — and what each depends on
besides the member; only relative rank depends on the population.

**One implementation consequence was worth stating.** `infra-client-requirements.md`
§10.2 said to re-evaluate predicates when a node enters or leaves the horizon,
scoring the entrant and dropping the leaver's rows. That is correct for four of the
five classes and wrong for a quantile, whose line moves for **every** row when the
count changes — so an implementation treating them alike leaves rows stale in both
directions. §11.2 now separates the two, and recommends an absolute rank where the
operator's intent is absolute, since *"my ten most trusted"* survives a metric change
exactly as well as *"top 20%"*. `light-client-requirements.md` §8 gained the
operator-facing half: expand a predicate into names before it is bound and keep the
names primary, and for a quantile show the population it is a fraction of.

### 2026-08-29 (the Dunbar Org is a two-edge walk, and it is 221)

A stray phrase in
a reply — *"a few hundred once siblings and cousins"* — drew the correction that the
horizon had never actually been settled. §12.6.3 admitted as much in writing:
*"~113, or a few hundred once siblings and cousins are counted, **depending on how
the org is drawn**."* Three documents carried three answers.

**`sub(n)` is deleted from the scope language** (author: *"this was not my
requirement, this was a Claude annotation of a misunderstanding of my requirement"*).
§11.4 had defined `dunbar` as shorthand for `sub(2)`, *"the subtree rooted n levels
above the owner"* — which is the grandpatron's subtree and therefore contains
cousins by construction. **No subtree has the Dunbar Org's shape**, so the form was
describing something the design does not have. `sub(n)` appeared in exactly three
places and nothing else used it. Wire tag **3 is retired and must not be reused**;
4, 5 and 6 keep their values rather than sliding down.

**All trust is relative to the observer** (author), so the region is defined by what
a party is to *you* rather than by a shape in the tree — downline because they have
a trust chain back to you, patron and grandpatron because you chose them
recursively, sibling sets because where they run infra they are **automatically
replicators** and because treating a sibling set as an **atomic trust set** is what
lets responsibility for resource administration be shared across one. Stated that
way it took several attempts to enumerate correctly; stated as a walk, below, it
takes one line and cannot be enumerated wrongly.

**The region is every node within a two-edge walk**, over adoption and sibling
edges — **221** at f = 10: you, then patron/subordinates/siblings at one edge, then
grandpatron, patron's siblings, grand-subordinates and nephews at two. One sentence
replaced a five-row tier enumeration, and the enumeration was what had been
generating the errors. An intermediate answer of 140, built tier by tier earlier the
same day, left **eighteen members who could not reciprocate** — a node held its
patron's siblings while they did not hold it. That cannot happen under a walk:
graph distance is symmetric, so the up-rules and down-rules cannot drift apart.
**Cousins also stopped needing a rule** — they are three edges out, alongside
nephews' children and great-grandchildren, and nobody was tempted to write a rule
excluding those.

**Sibling edges are load-bearing in the definition, not incidental.** Over adoption
edges alone the same walk gives 122 and drops both the patron's siblings and the
nephews — the two groups the region exists to include. §3.3 already had those edges
as real and implicit, *"authorised implicitly by the patron's adoption transaction,
with no separate agreement."*

**Peering edges do not count, and the reason is authority rather than arithmetic**
(author): *"peering is an ungoverned edge, it is outside of the tree. It is
permissionless and so it does not carry the authority of the subnet."* §6.3 said
peering contributes to trust, §15.1 defined the region, and nothing connected them —
a reader could reasonably have taken a peer for a neighbour, and the walk would have
made that reading nearly forced. §6.3 now states that **contributing to trust and
conferring scope are different things**, and that this is the edge where they part.

**Why two edges** (author): a patron's direct subordinates are a team it built and
maintains, with specialisation in who does what. A node needs access to the
complementary functions its patron's siblings are responsible for, and reaches them
by dealing with the responsible node **at the level where its graph intersects
theirs** — not by addressing the staff underneath. Two edges is exactly the reach
that yields the responsible party and stops short of their people. Each generation
offers a capability set to the generation below it.

**`f = 10` keeps its justification, restated as a heuristic** (author). The old form
— ten *"keeps the neighbourhood beneath Dunbar's number"* — is false at 221. The
replacement: the name is an **anchor for order of magnitude, not a bound**, and 221
against the vernacular ~200 is close enough. It fixes the scale as a small village
or an elementary school, larger than a household and smaller than a town, where the
alternative is a number with no intuition attached to it at all.

**Ten sites carried a superseded composition** and are corrected: §2's vocabulary
row, §3.3, §11.2.1's departure table, §11.4's role-table sizing (*"a few hundred
nodes"*), §11.5's cache warming, §12.6.3, §15.1, §20's parameter register, the A2
assumption row, and `resource-requirements.md` §7.1.1. The **`±2 tier` phrasing is
retired** wherever it named the region: the walk spans tiers −2 to +2 without
containing all of them, so that framing invites exactly the inference — cousins
included — which the region excludes.

**§15.1's storage figure was wrong before this round and is now right.** It read
*"h = 2. Full topology storage (~110 nodes)"* while §11.2 defines the Dunbar Org as
*"the region a node holds topology for"* — one quantity, given as 110 in one place
and as the horizon in the other. It is the 221-node horizon, of which 110 are the
node's own downline.

**The `CatalogReply` bound of 111 is unaffected, and its stated reason was wrong.**
`wire-format.md` §1 justified it as *"the Dunbar Org population at or below"*; the
real argument, two sections later, is that **an answering node answers for itself
plus the ≤110 users it serves**. Both come to 111 — the same arithmetic read from
different ends — but only one survives the horizon growing past it. The bound is per
*answering node*, not per horizon, and both sites now say so.

**0.8.3 finding 4: the locator acceptance was right and its reason was borrowed.**
§19.7 item 2 accepted locator topology disclosure as *"intrinsic, because graph
position is the evidence."* A locator is **self-signed by the node** and §12.1 gives
its signature a different job — *"an intermediary cannot substitute itself as the
destination"* — so it is routing authentication, not attestation, and §17.1 is
titled *"standing comes from edges, not from nodes."* The disclosure also runs wider
than the evidence: an introduction hands a distant party a patron chain their own
horizon would never have carried.

**The author's replacement is stronger than the one recommended, and moots it.** The
recommendation had been to keep the acceptance and justify it from the signature
construction — §12.1 says paths are truncatable for routing, so it is the signature
covering the whole path that forces a signed locator to carry it complete.
**Concealment was never wanted**, which makes that a mechanical note rather than a
reason: *"there is no expectation that topology is secret. Indeed, the hierarchy
reflected in your ancestor line is your identity to the outside world as a member of
that subtree. If you supplied your address in a different subtree, you would
effectively be presenting a different conceptual person… Only Marcus the natural
person can bridge these identities and regulate between them, from which the
individual draws most of their power and autonomy."*

**Every piece of this was already in the design and item 2 was connected to none of
it.** §13.7 disclaims cross-subnet accountability outright — the archive informs a
new subnet on joining rather than holding anyone to account across them — and §13
already says presenting different views to different subnets *"is not an edit to a
history, it is two histories."* C10 registers the correlation that remains while a
participant holds one identity, and §19.3 records multiple-identity support as the
deferred piece that completes it. Item 2 now states the position and both residuals,
the second being the reconnaissance value of a complete path to an operator sizing
independent edges — which creates no standing but cheapens §17.3's expensive step.

**The same justification was in a second place.** §12.1 carried *"since graph
position is the trust signal, this is intrinsic rather than fixable — document it,
don't pretend otherwise"* under a **Known leak** heading. Corrected there too, and
the heading with it: this is disclosure, not leakage.

### 2026-08-29 (de-linting pass over the 0.8 rounds)

Seven detectors over the five
root documents returned **112 raw hits**, judged individually rather than swept.
**Sixteen were real**; the rest were the detectors being over-broad, which is worth
recording so the next pass does not re-litigate them.

**Two root documents cited `Robot/`, which the working rules forbid outright.** §20.2
called its assumptions *"the natural targets for the Stage 1 simulations in the
review plan"*, and §23.3 opened *"the implementation passes (review plan 0.6) ask can
this be written?"* Both name `Robot/review-plan.md` — a design document depending on
a working file for its own argument. Rewritten to say the same thing without the
citation: assumptions are targets for simulation, and implementing a mechanism asks
whether it can be written.

**Pass archaeology, six sites.** §18 attributed its risk ranking to *"an
adversarial review"*; §20.2's Basis column was qualified *"following the 0.3
review"*; §23.3 narrated which mechanisms took one implementation pass and which took
two; §23.2 was *"consolidated so a reviewer need not reassemble it"*; and both Appendix A and
§23.2 deferred canonical test vectors *"until a review cycle runs clean"* — a
specification conditioning its own completeness on a review programme. In each case
the finding survives and the pass that produced it does not.

**Drafting narration, eight sites**, including one that took two passes to clear.
*"Until that was written, selection could not be recomputed"* became the requirement
it was describing; *"a hole in §12.6.5 as originally written"* lost three words;
§19.4's P35 row dropped *"§12.4 amended, since it previously stated the opposite"*;
§19.5 lost *"§14.1.6 originally asked for"*; `wire-format.md` §5.4's *"conflating
them was left open"* became the rule it had been deferring. **§10.2's italicised
parenthetical** — *"a draft of this section briefly treated deliberate multi-device
forking as an attack"* — is now the positive statement it was hiding: deliberate
forking is not an attack on this, and the multi-device problem is accidental forking.

**"More load-bearing than when written" appeared twice**, in §10.2 and §22, and the
first fix did not find the second — the recurring failure this log keeps recording.
The detector caught it on the re-scan, which is the argument for re-running rather
than trusting a sweep.

**Ninety-six hits were the detectors being wrong**, and the categories are worth
naming. *"Stated because"* and *"worth stating"* (13) are the design explaining why a
rule is explicit — authorial voice, not an instruction to a drafter. *"Withdrawn"* and
*"superseded"* (20) are either domain vocabulary — a superseded identity, a withdrawn
authority — or the register tombstone convention that keeps numbers from being
reused. Most *"open question"* hits (7 of 8) are legitimate pointers into §22 and
§23; the eighth was a malformed `(§22.)` citation, now fixed. And most *"pass"* hits
were a UWB `pass`, a path passing through a root, or a credential not being passed.

**Both front-matter revision dates were stale** — 2026-08-25 and 2026-08-22 against
documents edited that day — **so they are gone rather than corrected** (author): an
edit timestamp lives in the filesystem and now in git, and restating it by hand is a
surface that can only ever be wrong. `network-design.md` loses its *"Last updated"*
line; `wire-format.md`'s status line keeps **Status: Draft** and drops *"Last revised"*.
The other three root documents never had either, so the set is now consistent.

**The root set carries no date tokens at all.** The 2026-08-28 pass took them from 86
to 2 and kept those two as document metadata; this removes the category. `Robot/`'s
review plan had already caught the failure mode — *"front matter goes stale silently…
nobody looks at the top of a document they already know"* — and prescribed adding the
header to the grep list. **Removing the surface beats checking it**, which is the
correction this project keeps arriving at. The plan is updated to say so, and to keep
header *scope summaries* on the grep list: those carry claims nothing else records,
so they can still go stale and still have to be read.

### 2026-08-29 (adversarial review 0.8.4; the provider is not the node)

Six
findings against a state actor compelling a cloud provider. All six held, and five
of them are one mistake: **the design repeatedly reasons from the infra node as the
compromise and observation boundary**, while a compelled provider sits beneath many
nodes, their storage, their authenticated transports and their signing capability at
once.

**§18's provider item asserted the opposite of §14.2.** It read *"end-to-end
payload encryption still holds here, which is what keeps the failure to metadata and
local state rather than content."* Payload is encrypted **to the addressed
endpoint**, and a provider hosts endpoints — §14.2's own table says a patron reads
leaf-to-patron traffic *"as the addressed party, not a relay"*, a hosting node reads
a hosted resource's requests because it parses and re-serialises them to insert the
credential, and a proxying node reads proxied ones. What encryption does protect is
leaf-to-leaf traffic in transit and brokered resources. The **queue** claim in
§14.1.6 was checked and is correct as written: queued traffic is leaf-to-leaf, so it
really is ciphertext to the holding node. Relay against endpoint is the distinction
the whole item now turns on.

**The exposure is not limited to reading.** Appendix A requires infra operation to be
unattended, so countersignatures, `SubtreeAck` decisions, currency attestations,
disavowals, peering records and topology propagation all proceed with no operator
present and the signing capability on the machine. One order reaches the **genuine
protocol authority** of every hosted instance at once, with no separate social
compromise per operator. The neighbouring register item names resource-access
forgery, which is one consequence among these rather than the boundary.

**Cross-identity linkage is folded in rather than given a register number**
(author). C14 was withdrawn on the reasoning that a fresh identity *"appears in a
different subnet under a different serving node that sees only one"* — sound against
a node, silent about what sits beneath several. Where both serving nodes are hosted
together, one observer sees both mutually-authenticated attaches (`wire-format.md`
§9.1). C14's tombstone stands as written; it was right about the adversary it
considered.

**Every adjusted claim now names the breaking scenario and links to §1.2.3**
(author's instruction). The design is explicit that *"the threat model of an
anonymity network does not apply"* and that this is not a platform for evading the
state; a compelled provider is §1.2.2's third class operating below the whole
architecture. Saying so at each site keeps the corrections from reading as newly
discovered weaknesses in a design that never claimed to cover this.

**The rootward-memo replay keeps its acceptance and loses its economics.** It was
accepted as an attack *"costing the attacker more than the target"*, which assumes
the injector owns the edge it loses. An actor operating a commandeered tenant's
instance pays with somebody else's relationship, and re-adoption makes the result
repeatable churn rather than a one-time price. The three structural bounds — the
injector must sit at or below a direct subordinate, the severed edge is the one that
handed the memo over, and the disavowal is reason code 5 with re-adoption available
— are unaffected and now carry the acceptance alone.

**What survives, stated in the item**: participants' own signatures remain
unforgeable, so the actor cannot manufacture a presence record for anyone it has not
separately compromised; §6.4's ungated proof of presence means an honest ceremony
cannot be suppressed; and sealed captures sit on light-client devices out of
infrastructure's reach (§7.5.2).

**A further bound, and a sharper one than the review found** (author): **a
compromised node is not on the direct path.** §12.6.3's table already gives who sees
a direct-path flow as *"nobody — the peers exchange addresses during setup and
connect"*, so an actor holding an infra node's key sees the connection setup and
nothing after it. It bites hardest for the node's own operator, whose light client
is a different device: traffic addressed to them **as a participant** never reaches
the instance the actor controls and cannot be read or even measured there. Forcing a
downgrade to the relayed path gains nothing, since a relay carries ciphertext. **The
actor's reach is prospective, not retrospective**, which is precisely what *"bulk
access"* overstates — for direct-path payload there is no accumulated content at the
node to seize.

**The residual is impersonation rather than interception.** §23.1 is explicit that an
operator's instance *"holds the same key as their phone — the relationship is a seed
shared across wallets, not a client and a server"*, so an actor with that key can
present as the operator in **new** exchanges and become the endpoint legitimately.
What it cannot do is reach a session it was never on the path for. This makes the
shared-key model cut both ways in the same item, which is worth having stated in one
place: it is what lets the actor impersonate, and it is not what lets it read.

**The economics of that residual are the point, and they close the item** (author):
impersonating an infra user toward people who know that user, directly or through
someone who does, **risks burning the subverted node**. It forces the actor out of
passive bulk collection and into social engineering — per-target, high-commitment,
and self-burning when detected. So the design's position on its top-ranked systemic
risk is not that the adversary is stopped but that **what the adversary must spend
changes**: compelling a provider is *for* scalable deniable collection, and the
direct path denies it that for content. **§1.1's principle reaching the one case
§1.2.3 says the design does not cover** — visible and expensive rather than
impossible, which is the answer the design gives everywhere else.

**And the capability devalues what it collects** (author). §1.2.1 already treats
cheap fabrication as a privacy property — *"Sybil attackers inadvertently contribute
to the deniability of every record"* — and an actor able to act as any hosted
operator **enlarges that discount rather than escaping it**: a surveilled record
naming an operator is deniable because the actor's own capability is the standing
alternative explanation. The asymmetry runs as §1.2.1's second property already
says — a person who has met you infers your identity cheaply from personal
knowledge, while a remote examiner pays for either an impersonation operation or an
evidence chain that survives due process. **Structurally the same acceptance as
Potemkin subnets**, four items below: an expensive fake that makes the surveilled
record less useful to whoever built it.

**§1.2.1's boundary is restated with it**, so this stops short of claiming nothing
can be proved. Forging evidence about a specific real person needs their
participation; a presence record needs a live counterparty, witnesses and verifiers
who were there. An actor holding an operator's key can sign as them and cannot put
them in a room — which is also why the shared-key model does not collapse the
presence layer along with the identity.

**The two remaining 0.8.4 corrections, applied on the same terms.**

**ASN is a concentration detector, not an independence proof.** §17.3 exposed it
*"so that concentration is observable"*, §3.4 said *"the resulting independence, or
lack of it, is visible"*, and §19.7 item 6 accepted the placement disclosure on
that basis — three sites treating ASN and region diversity as evidence of separate
control. **The signal only runs one way**: 1,000 nodes in one ASN really is evidence
of concentration, but ASN and region describe routing and geography rather than the
entity subject to one legal order, and one provider can present many of each.
`NetworkPoint` field 2 is `? uint` besides — optional and self-asserted, with no
IP-to-ASN validation anywhere. §3.4 already knew the shape of this, saying two nodes
*"in different subtrees but the same availability zone are not"* independent and
that most infra *"will live in a handful of clouds"*, and then drew the opposite
conclusion one sentence later. All three sites now say concentration is what the
check finds and independence is what it cannot prove.

**The currency query's body-level omission is not a privacy property.** §19.7 item
12 accepted the disclosure partly because *"the query carries the subject and not the
querier"* — but `wire-format.md` §9.1 states **"Authentication is mutual"** and
requires the serving node to bind the requested identity to the transport-authenticated
one, so the party answering knows exactly who asked and can join querier, subject and
time whatever the schema omits. What genuinely limits exposure is **frequency**, and
stapling and introducer-first do reduce it; those survive and now carry the
acceptance alone. *"Nothing is retained on either side"* is restated as a commitment
under §1.1's test rather than a rule, since it cannot reach a compelled provider's
logs.

Both name the breaking scenario and link §1.2.3, per the same instruction as the
provider item.

**The ASN claim was in five places, not three.** A sweep after the first three edits
found §3.4's *"an observer can see whether two peers are actually independent"* and
§6.3's *"so independence and concentration are observable rather than asserted"* —
the second being the schema's own description of why the field exists. Both now say
concentration. This is the same miss the log keeps recording: fixing the sites a
review cites, then finding the claim's own definition site on the sweep afterwards.
A further sweep across the whole set found a **sixth**, in the other document:
`wire-format.md` §4.4's *"**Independence** is a visible signal, not a trust input"*,
beside the `Peering` schema. Corrected to concentration, with the one-way asymmetry
and the optional self-asserted field stated where the field is defined.

### 2026-08-29 (adversarial review 0.8.5; the ceremony counterparty)

Four findings
from the perspective of a hostile person standing in front of you. All four held.

**An ordinary ceremony discloses the subject's whole 730-day presence history to
whoever they just met.** Selecting the other's verifiers means computing their
candidate set, and `wire-format.md` §5.4 counts a record only if its signatures
verify — so the selecting party needs the **records**, not a list of names, and a
list the subject asserted would let the subject curate its own sample. §8.1.1's
eleven-exchange sweep already recorded the fact, giving recomputation's reads as
*"nonces, seed, candidate set"*; **what was never priced is who receives it.** Every
prior counterparty, every witness and verifier those records name, every timestamp —
none withholdable, since they are body fields under signature. Registered as
**P37**, and distinguished there from P19 (archive presentation to a patron the user
*chose*) and from P2/C2 (the verifier set *carried in the record*, not the population
it was drawn from). §19.2 gains it as the same trade one step earlier, with the
same non-answer: hiding the candidate population from the selecting party also stops
them performing §8.1.2's before-signing check, which is one of the three that protect
a signer against the person in front of them.

**`finalized_at` had only a lower bound, and one signature could freeze every
co-signer's chain.** The rule was *"MUST be ≥ `started_at`"*, and §2.2 states that no
future tolerance applies; since every envelope signer's next record must clear a
predecessor's `finalized_at`, a body naming the year 2100 would freeze the victim and
its whole witness set — roughly eight archives per record, at the cost of one
disposable identity. The 0.8.3 witness clock check covers `started_at` only.
**Bounded structurally rather than by client obligation** (author): the difference is
now capped at 24 hours, which **a validator can check with no clock at all**, since
it constrains two fields in the record against each other rather than either against
the reader's time. That is why this one can be structural where §2.2's other
timestamp rules cannot. The figure reuses the seed window rather than introducing
another, and is far beyond any honest finalization: `pending` and `unavailable` count
toward the threshold, so a ceremony never waits on an absent verifier.

**The bound covers the gap and not `started_at`, which divides the work between two
checks.** A body claiming `started_at` in 2100 with `finalized_at` an hour later
satisfies the new rule and poisons chains just as well; what stands against that is
0.8.3's witness clock check, which no validator can verify. So the structural rule
did not supersede the client commitment — it closed the half a reader can check, and
§8.1.2 now says which mechanism covers which field and why.

**The second encounter of any identity has no continuity check**, because
`min(floor(n/2), 10, |candidates|)` is zero at *n* = 1 as well as at *n* = 0.
§6.4 asserted the opposite — *"with prior counterparties the finalization threshold
is non-zero"* — which is false for exactly one prior meeting. **Corrected in prose
rather than in the formula** (author): the threshold first bites at *n* = 2, the
second encounter rests on liveness and proximity alone, and those establish that
*someone* is present rather than that they are who the identity represents. Accepted,
since the attack needs both the key and the archive, and one clean false-continuity
edge bought before an identity has any history is worth less than the ceremony costs.

**§7.4's "an impostor gains nothing" is scoped to the honest counterparty it
assumed.** The argument is that the profile is *"generated by the honest counterparty
from what they captured, so signing it means signing a profile of oneself"* — sound
against an impostor subject, and silent about a malicious one. A client performing
the capture can substitute a synthetic or third-party profile, and countersigning
binds the bytes and template version rather than their provenance: the subject's
device captured the counterparty, not itself. The yield is small — the anti-oracle
rule caps it at one probe point per ceremony, and no record finalizes without the
subject's envelope signature, which a subject seeing `no_match` about themselves will
withhold — but the claim read more broadly than it holds. Registered rather than
closed: binding a profile to the live subject needs an attestation the capture device
does not have (§7.8).

### 2026-08-29 (adversarial review 0.8.6; the stolen device)

Five findings. Four
applied here; the terminal-`seqno` finding is open pending a decision on the series
proposal.

**§19.7 item 2's bridging claim contradicted P5, and item 2 was written yesterday.**
It said *"only the natural person can bridge their positions and regulate between
them"*, while P5 already registers the device as *"the global correlation point the
network architecture otherwise avoids"* and §13.7 already says the strongest attack
on a multi-subnet identity is device compromise. **The bridge is the device, not the
person.** Scoped to the network layer, which is the only layer the claim was ever
about: the autonomy is real and endpoint-bound, and whoever holds the hardware holds
the join. A statement about what the protocol declines to build, not a guarantee
about where the correlation lives.

**§7.4's anti-oracle binding is to the subject's key, not to a witnessed
encounter.** The requirement is stated as *"the requester proves it is currently
engaged in a witnessed encounter"*, and the proof offered is the subject's signature
over the `query_id` — which prices probing *"in ceremonies rather than in packets"*
only for an attacker who lacks that key. **A thief holds both halves of the price**:
fresh commitments, every signature, every seed release, and the notifications — every
subject-side limit in that section sits on the stolen device. What survives is
verifier-side, the per-requester and per-subject counters each verifier keeps
locally. **Witness countersignatures would not repair it**, for the reason §7.4
already gives: a distant verifier cannot tell real witnesses from an attacker's keys.
That reasoning was sound and its conclusion reached one step too far.

**Two compositions registered in §18** (author). A stolen device plus one colluding
counterparty manufactures presence evidence real witnesses will attest to — they
attest that the protocol ran, not that the queried profile came from the live
participant's face — and the thief signs whatever comes back, since all response
categories count structurally and the subject-side refusal §7.4 relies on is the
thief's to make. The colluder then supplies §9.1's recognition half. Bounds that
hold: honest verifier signatures stay unforgeable, so adverse results are visible,
and §16.2 caps the successor at what the colluding parties carry.

**And the biometric exposure is a seed release, not a grant** (author): *"there is no
general grant state that a PoP participant enters that opens their images on all
devices"* — release is per query and direct to each selected verifier. What theft
changes is that a previously compliant holder can later be **selected** and receive
the seed through ordinary automatic traffic, with the depicted person never asked.
**Bounded by a confluence**: the counterparty must run a new ceremony inside the
730-day window *and* the compromised identity must be sampled, roughly q/d per
ceremony. Records past the window are safe outright, since the subject will not
release their seed. Real and hard to target — no captured device releases its whole
archive this way, and most release none.

### 2026-08-29 (0.8.6 finding 1 closed: `seqno` becomes `{series, counter}`)

A
stolen key could pin an identity permanently. One record at the top of a `u64` leaves
no successor, §7.7.3 makes an equal `seqno` with different contents malformed rather
than a tie, and `wire-format.md` §2.3 **states the consequence while arguing for a
different rule**: *"a subject that cannot advance has no way to publish a change at
all."* Nothing in the registers mentioned exhaustion. §8.2's heartbeat counter
already carried an exhaustion rule — *"on reaching u64 max, end the session rather
than wrapping"* — so the case was reasoned about where running out is benign and not
where it is adversarial.

**The counter is now half of a pair, and the series is arbitrary** (author). Two
`seqno`s compare only when their series are equal; across series there is no order
and a reader must not invent one. **Ordering the series would have moved the attack
up a level** rather than closing it — an attacker would simply name the top series —
which is why arbitrariness is the defence: there is no top to name, nothing registers
a series globally, and poisoning is therefore per-reader. To block a target an
attacker must exhaust 2³² **and deliver every one of those series to every party it
wants to block.**

**The scope is the patron relationship, not the anchor, and this was a correction to
the assistant** (author): *"Roots are an ephemeral condition… Anchors are a cacheing
decision made on other nodes for convenience, there is no anchor in the network."*
§12.2 is sharper still — *"anchor status is not a protocol property a node possesses;
it is relative to whoever is resolving"* — so an anchor-scoped series would have been
undefined, differing per correspondent, and §12.2 had already rejected tier-based
definitions for the same instability under merge. The patron edge is stable under
upline churn, bilateral, and already countersigned.

**Series refresh is transaction type 7**, node plus patron. The countersignature is
the whole security property: a stolen key can exhaust a counter and cannot escape into
a fresh series, while the legitimate holder can because their patron signs for them
and not for a thief. **The proof is presentation, not propagation and not a history
scan**: the adoption's `Locator` already carries the `seqno` the patron countersigned
at that moment, so the chain is that adoption plus each refresh since. Chain length is
the order; two divergent chains are patron equivocation, attributable by that patron's
own signatures. **A refresh is not a move** — routing walks `anchor` and `path`, which
it does not touch, so cached locators keep working.

**Two rules follow from the construction rather than being chosen.** A node must never
refresh into a series it has occupied, or the abandoned line's high counters return —
checkable by anyone holding the chain, so a MUST rather than a hope. And a refresh is
an **archive checkpoint**, which is what makes pruning possible at all: today
truncation to a prefix is the only edit, because every record commits to its
predecessor and verification walks to genesis, so early history cannot be dropped. A
countersigned checkpoint asserts continuity across the boundary without carrying its
contents — the résumé shape, where blocks and dates survive and details go.

**Pruning is confined to beyond the 730-day window.** Verifier selection counts *n*
and draws candidates from what is reachable, so a checkpoint discarding recent history
would let a subject choose its own verification burden — down to the `n = 1` case that
requires **zero** verifiers, which 0.8.6 confirmed. The constraint costs nothing the
feature wants, since the storage saving and the elision are both about records the
candidate set has already aged out.

**The exhaustion move is also the defence** (author). A 32-bit counter advanced only
on position and endpoint changes will not run out in a lifetime, so the top of the
range is dead space the legitimate holder can spend: **on suspected compromise, set
the counter of every series you are leaving to its maximum, then refresh naming that
value.** Nothing the thief signs can supersede it afterwards — no strictly greater
counter exists, and an equal one with different contents is malformed. The property
that made the attack possible is what seals the abandoned line behind you.

**Order is forced by the schema.** Field 3 of the refresh records the counter the old
series reached, so the seal must precede the refresh or the chain names a departure
point that later records contradict. Seal, refresh, repeat per patron relationship.

**And it exposed a rule left implicit.** A party holding the §4.6.1 chain knows which
series were abandoned and **MUST reject records in them at any counter** — the seal is
not for them. It protects parties holding a cached locator and nothing else, and
against those it is a race the thief wins by arriving first, which is the argument for
acting on suspicion rather than on confirmation.

**Sealing is unilateral and the refresh is not** (author), which is what gives it
reach. It is a self-signed locator at the top of the counter — §4.3's standalone
carriage form — so a node can seal a line toward parties in subnets where it holds
no membership and could not refresh if it wanted to. Only creating new room needs
the patron.

**Which closes a gap in §9.0's own language.** Rotation is described as
*evaporating* the old identity, and that was true of propagation and not of
capability: the old key still exists and nothing stopped it publishing a fresh
locator to anyone holding a stale one — §12.3 Case 2 guarantees those holders have
no other source of truth. **Sealing the retired series forecloses that**, at the cost of one
self-signed locator, and the old key is already in hand at rotation because it signs
the `Recovery`.

**Scoped precisely, because it is not a revocation** (author): a seal reaches as far
as the locator carrying it and no further, so parties never contacted and subnets
never entered never see it and the key is not dead to them. **They are also not the
population at risk** — redirection needs a cached locator to redirect, and a thief
presenting the old key to someone holding none is attempting a *first contact*, which
§12.3 Case 0 makes an out-of-band act answered by the presence layer rather than the
routing layer. Affirmative closure for the parties reached, silence about the rest,
and the two face different attacks.

**The model, in the author's terms**: a seal puts *"an address that can't be updated
in the address book of those specific users who receive it"*, occupying the slot any
future locator for that `(node, series)` pair would take. Not a deletion — the entry
still resolves if the position behind it answers — but nothing can move it again, the
node included — **which binds the writer as much as anyone** (author). Writing the top
slot is **denial, not control**: a thief that seals a series can execute nothing
further in it either, so it burns a line rather than capturing one. **The `keyhash` is
untouched**, because the legitimate holder refreshes into a fresh series with the
patron's countersignature and the thief cannot obtain one — what was destroyed is a
line the holder was leaving anyway. The cost is re-contact for parties that took the
thief's seal: a chain for those that will take one, out-of-band re-introduction for
those that will not.

**And burning a key outright needs no mechanism** (author): rotate, then never
refresh. A series continues only by patron-countersigned refresh, so declining one
leaves the sealed line as that key's last word. Sealing and continuing is repair;
sealing and stopping is retirement; the difference is entirely in what the subject
does next, and no revocation object is needed to express either.

**Two registers move.** P36 is largely answered: a separate series per patron
relationship means a separate counter per binding, so an observer in one subnet sees
no gaps it cannot account for — the residual is a node that has not yet refreshed, and
the refresh count itself. P37 is bounded: what a ceremony counterparty receives is what
has accumulated since the last checkpoint rather than the subject's whole
participation, floored at the window.

## 2026-08-30

### 2026-08-30 (consistency pass over commits `c988712`..`99e1051`)

Diffed the five
commits spanning 0.8.2 through 0.8.6 and repaired what the feature changes stranded.

**A table row rendered broken.** The P37 register row (added in 0.8.6) carried literal
newlines inside its final cell, so Markdown split it across five physical lines and
the row did not render. Joined to one line. The structural suite gains an
**unclosed-table-row check** — a row that starts with `|` and whose line does not end
with `|` — which is what would have caught it; fence parity and headerless-table
checks both passed it.

**P36's detail column contradicted its own severity column.** The severity cell was
updated to *"largely answered by the `{series, counter}` split"* while the detail cell
still read *"a node bound into two subnets advances one counter in both"* and
*"registered rather than engineered away… a per-binding counter would break `seqno`'s
double duty"* — the exact objection the split overcomes by tagging each counter with
its series and forbidding cross-series comparison. Detail rewritten: the split *is* the
per-binding counter, made safe by the series tag, with the residuals (a not-yet-refreshed
binding, and the visible refresh count) kept.

**A drifted count.** §23.3 still deferred *"transaction types beyond the six"* after
0.8.6 added series refresh as type 7 — now *"beyond the seven"*. The overview at Appendix A was
left as written: it already lists five and omits the point-to-point abuse report, so it
is a deliberate sketch rather than an enumeration.

**One scalar-model residue.** The departure schema's `seqno ; incremented` predated the
`{series, counter}` split; tightened to *"counter incremented within the current
series"* to match §4.3.

**Left for the author, not repaired**: checkpoint-pruning (§10.2) is a second archive
edit and is in tension with §10's foundational guarantees — *"provably unbroken from
identity genesis to the last record shown"*, *"excision is impossible, only truncation
to a prefix remains"*, and *"activity gaps are visible"* — and with §16.7's
prefix-only presentation model. §10.2 hedges its own claim with *"today"*, but §10, §16.7
and `wire-format.md` §§3.1, 4.1 state the exclusivity flatly. Reconciling them is a
design decision (see the critique of 2026-08-30), so the cross-references were not
scatter-patched.

## 2026-08-31

### 2026-08-31 (§10.0: the kinds of history, and what each is for)

Five mechanisms
were routinely spoken of as one — transaction archive, PoP records, sequence numbers,
seqno series, and the refresh that branches a series. New **§10.0** tabulates them by
what each establishes, who governs it, and whether it is evidence at all. **The
sequence number is the one most often mistaken for the chain**: it corrects
asynchronously updated routing state and enforces nothing.

**PoP records are a distinct kind of history** (author). They are referenced by the
archive and carried in a sequence's chain, but are **not themselves subject to
sequencing**: self-signed, dated, about the natural person, and potent under any
sequence and in any subtree. So they **survive pruning of the chain that carried
them**, which §6.4 already required without anyone noticing it applied here — *"a
party with authority over a participant must not authenticate or gate evidence of an
event it did not observe"*, and *"PoP history must survive a node's tenure under any
patron."* 0.8.6 had made the patron-countersigned refresh the checkpoint governing
pruning, which put the *unit of retention and disclosure* for presence evidence under
patron control — a weaker form of the coupling §6.4 forbids. §10.0 states that a
checkpoint bounds what the **chain** must retain and does not reach the PoP records.

**The correlation between a series and an archive segment is elective, and now says
so** (author): *"not a necessary consequence, as they are two distinct forms of
history, it is a design decision made for simplicity and ease of administration."*
Recorded as a choice rather than a consequence so the two can be separated later
without disturbing either.

**Retention and disclosure are separate controls, and the direction of foreclosure
matters** (author's correction). Retention forecloses disclosure, permanently and in
one direction — not the reverse. A single combined control would make choosing to
*show* less also *destroy*, giving a presentation decision silent destructive side
effects. `light-client-requirements.md` §2 now requires them kept apart, and requires
that pruning the chain not delete presence records, sealed captures or capture seeds.

**§10's "exhaustive" list of what advances an archive predated type 7** and omitted
the series refresh; corrected. No other enumeration of archive membership exists.

**Nobody walks another party's archive, and P37 was wrong to imply otherwise**
(author): *"we never intended to let PoP counterparties walk the user's full archive
to discover their PoPs; you just have to trust the bundle of PoP records your
counterparty hands you."* §8.1.2 now states the bundle model: a counterparty computes
*n* and the candidate set from records the subject supplies, can check that they
verify and chain to the commitment, and **cannot check that the bundle is complete**.
P37 had conflated this with §16.7's fetch-and-walk — which is the *adoption*
disclosure a prospective patron drives — the very distinction the row claimed to be
drawing. **Reframed from a compelled disclosure to a pressure**, severity High to
Medium: the subject chooses the bundle, and the incentive runs one way, since a pool
holding nobody the selector recognises establishes nothing for them.

**Which scopes §8.1.2's non-influence rule rather than weakening it.** The pool is
the subject's and the sample within it is not: determinism stops a participant
steering the sample, not choosing what to put in front of the selector. **What
protects the selector is recognition, not completeness** (§16.1) — a curated bundle
costs its author credibility rather than buying a verdict, which is the intersection
test applied everywhere else here.

**§10.1's "one chain per identity, spanning subnets" is withdrawn and not replaced**
(author): seqno series postdate it, and no claim about archive scope across bindings
is made in either direction. Nothing turns on it, since no reader can enumerate a
subject's archive uninvited. The §10.0 PoP row states the same separation: a PoP **is**
a transaction and enters the archive, and the chain does not govern its *use* —
identifying validators for a later ceremony does not depend on the archive's
continuity.

**The real security of a PoP is the physical presence, and §7 now says so**
(author). The section framed the primitive only as *"a cost imposed on acquiring
edges into territory the attacker does not already control"* — the Sybil half. The
other half was missing: **the ceremony's first product is not the record.** Two
people met and can recognise each other afterwards, each having grown a graph they
trust on their own account, which is §1.2.1's second property — *"a participant knows
which meetings happened"* — stated where the ceremony is defined rather than only in
the deniability argument. **The record is the residue a ceremony leaves for people
who were not there**, and witnesses, verifiers and deterministic selection are all
spent on that residue.

**Which gives P37 a floor.** A thin ceremony is weaker *evidence*, not a weaker
meeting: fewer witnesses or an unrecognised candidate pool cost the record its weight
with third parties, and cost the counterparty its assurance about **continuity** —
whether the person present is the one this key's history belongs to — while touching
neither party's actual gain, a face they will know again. So disclosing narrowly is a
choice a subject can really make, per ceremony, rather than a ratchet. Severity left
at Medium; what changed is that the row now states what declining costs.

**Propagation sweep over the whole set.** The archive-integrity claims were the
outstanding tension from 2026-08-30 and are now reconciled rather than deferred,
because the design decision behind them is settled. **Excision remains impossible and
the reason is sharper than before**: a checkpoint takes everything before it or
nothing, so it **cannot be aimed at a record**. What changed is that truncation now
cuts at either end — an earlier head drops what is recent, a checkpoint drops what is
early — and a presented range is unbroken across itself, rooted at genesis *or* at a
checkpoint. §10.1's three guarantees, §10.2's per-evaluation claim, §16.7's
presentation model (*"prefix"* → *"contiguous run"*), and `wire-format.md` §3.1 and
§3.1 all say this now.

**One conflict the sweep caught between two same-day changes.** §10.2 said *"records
before it need not be retained or presented"* while §10.0 and
`light-client-requirements.md` §2 said presence records survive pruning. §10.2 now
releases **the chain, not the evidence**, and says so where the licence is granted.

**§5.4 defines the candidate set without granting access to it**, which is what
permitted the archive-walking reading in the first place. It now states that a
selecting counterparty computes over records it is **handed**, and that the traversal
rule is what makes a supplied bundle checkable — records must chain, so a bundle
missing one from the middle fails to connect.

**References: zero unresolved across the five root documents.** The prior baseline of
29 was never reproducible because the checker mistargeted bare `§N`; the convention is
that an unprefixed reference resolves against the containing document first and
`network-design.md` second. Under that rule the set is clean, and the only real
defects were **three compound citations** — `(`wire-format.md` §2.3, §5.6)` and two
like it — where the second reference silently inherits the prefix. Prefix now
repeated, matching the fix applied to the same shape in 0.8.3.

**Naming settled: the operation is a `series reissue`** (author). *"Rotation"* was
the word to avoid and never reached the text; `series branching` was considered and
dropped, and `series refresh` — the term 0.8.6 was drafted with — is renamed across
**34 edit sites in three documents**, yielding 38 occurrences of the new term,
including the type-7 table row and `wire-format.md` §4.6's heading.

**`branch` was the disqualifying collision, and it sits one subsection away.** §10.3
is *"Merges. The archive is a DAG, not a chain"*, and there a **branch** is a line
created by concurrent device use whose defining property is that it **merges back** —
*"branches can be merged, and merging strengthens completeness."* A series is the
opposite: it must never merge back, which is what keeps the ordering total and the
rollback closed. Two structurally similar things named `branch`, differing in exactly
the property that matters, introduced a subsection apart. `branching` survives as
**prose** in §10.0's row, where it does the explanatory work without being the term.

**`reissue` is apt rather than merely free.** The old line is sealed and dead, the
identity and its position continue, and the usual reason to ask is that the old one
was compromised — which is a card reissue in every particular. `refresh` implied
renewal of the same thing and undersold the finality of the seal.

**The rename also disambiguated a word carrying five senses.** `refresh` was being
used for the biometric reference image (§7.5), the catalog view (§11.5), the
currency attestation (§12.6.5), the prekey (§14.2.4) **and** the series. The first
four are each unambiguous inside their own section; the series was the newest and the
only one that ranged across all three documents. Removing it leaves the others alone,
and no surviving `refresh` refers to a series.

### 2026-08-31 (0.8b retired from the running order)

The suggested-order table
listed **0.8b — vignette/spec agreement** as pending. It is not: **the pass ran on
2026-08-16** and found four contradictions, all in the same direction — the vignette
claiming more than the mechanism delivers — with a follow-up entry the same day
correcting the over-hedged prose it produced. The row had been carried as an open
item ever since, which is how it reached this session's open list.

**Retired rather than rescheduled** (author): it does not belong under 0.8, whose
six adversary roles are attacks on the mechanism, and **0.2 already covers it** —
that pass looks for *"a rule stated one way in one section and differently in
another"*, and a vignette contradicting the section it illustrates is exactly that.
`Robot/authoring-conventions.md` supplies the rule 0.2 would apply: *"a mismatch
between vignette and specification is a defect in the vignette."*

**It never had a section body**, in any version of the plan at either path — checked
against every historical revision. So the row was a line in a summary table with
nothing behind it, which is the failure mode this log keeps recording for
consolidating sections, appearing here in a working file rather than the spec. Table
renumbered; the change log's three references to 0.8b resolve to the entries above,
which is where a reader tracing the name should land.

### 2026-08-31 (the 2026-08-30 critique closed; its proposals declined)

That pass
produced five protocol-level suggestions. **Four are declined and nothing is carried
forward** (author): they drew no comment beyond the corrections already applied, and
the pass was **methodologically weak by the plan's own terms** — 0.8 specifies a
*different model family*, and this critique reached the drafting model through
platform routing rather than by design. A critique of the drafting by the drafter
finds what the drafter already believes. It may be re-run elsewhere; nothing here
depends on it.

**Two entries it left dangling are closed here.** The 2026-08-30 entry cites *"the
critique of 2026-08-30"* — that was **session conversation, not a document**, and a
reader should not hunt for a file. The same entry recorded checkpoint-pruning's
tension with §10's guarantees as *"left for the author, not repaired"*; that was
**reconciled on 2026-08-31**, when excision was restated as impossible for a sharper
reason — a checkpoint takes everything before it or nothing, so it cannot be aimed at
a record. Both entries stand as written; this one supersedes them.

**Of the five, only the recovery-threshold suggestion survived contact**, and not in
the form proposed: scaling it as a validity rule would have failed Appendix A's own test,
since the threshold needs the *old* identity's archive and a subject recovering from
device loss does not have it. What survives is the observation that recovery's flat
*"at least one match"* sits oddly beside a ceremony threshold that scales with
history — recorded as something an evaluator may weigh, not as a rule.

### 2026-08-31 (de-lint over 0.8.3–0.8.6 and the consistency work)

Six detectors
over the five root documents. **Dates, `Robot/` citations, markers and drafting
narration all returned zero** — the categories that dominated the 2026-08-28 and
08-29 passes did not recur, so the rounds since were drafted clean of them. **Six
sites needed work**, all in the two categories a spec is likeliest to grow late:

**Self-narration, three sites.** §6.1.1's vignette gloss said a rule *"now states"*
its default; `infra-client-requirements.md` §10 said design §11.2 *"now carries"* the
general rule; §7.5.2 said a compliant holder *"now"* has no way to defeat the
subject's decision *"where previously it had only an obligation"*. Each described the
document changing rather than the design working. The third kept its teaching by
naming the mechanism as the agent — **what *sealing* changes is capability rather
than obligation**, which is §1.1's test applied to storage and does not depend on a
reader knowing what the design used to say.

**Process archaeology, two sites**, both about pass 0.6. Appendix A read *"the resource
layer's interaction protocol, previously the one item that did, has now had
clean-room implementation attempts on both its halves"*, and §23.2 opened with the
same claim. A specification stating that its own mechanisms survived an
implementation attempt is describing the review programme, not the protocol; both now
state the current fact — **nothing blocks a subsystem**, and **resource interaction is
specified on both halves**.

**One typo**, §14.1.2's *"The the sibling determines"*, found by the unrestricted
doubled-word check rather than the whitelisted one — the same lesson as 2026-08-29's
*"capture capture key"*.

**First-person prose was reviewed and left alone.** Sixty-eight hits, all legitimate:
the preface is the author's own voice by design, Appendix A's vocabulary table defines *"the
reference client"* as *"what our implementation does"*, §3.3 carries the author's
verbatim social-agenda blockquote, and the rest is quoted speech inside vignettes.

### 2026-08-31 (0.9-before, mechanical half)

An organisation-only pass returned six
classes of finding. The two that need no editorial judgement are applied here; the
rest are structural and wait for the author.

**Heading depth did not match the numbering, in 68 places.** The convention is
`depth = components + 1` — `## 1.` , `### 1.2`, `#### 1.2.1` — and 111 of
`network-design.md`'s 163 numbered headings already followed it. The other 52 did
not, nor did 15 in `wire-format.md` and one in `light-client-requirements.md`, so
**the numbering and the rendered outline described different structures**: `### 4.6`,
`### 4.6.1` and `### 4.6.2.1` all sat at one level. Every deviation was too shallow,
none needed shallowing, and the deepest result is `h5`. Re-levelled mechanically; no
heading text changed, and references still resolve at zero unresolved.

**One subsection was physically out of sequence.** `wire-format.md` §5.2.1 sat
between §3.6.3 and §3.6.3.1 — noticed during 0.8.3 and never fixed. Moved to precede
§3.6.3, which is what its number claims. §3.6 now reads 4.6.1, 4.6.2, 4.6.2.1, 4.6.3,
4.6.3.1, 4.6.4…, and no document has an out-of-order heading.

**`infra-client-requirements.md` §5 is correctly placed and awkwardly named**, so it
is left alone: it follows §3.5 as an insertion between §3 and §5, which is what `4a`
means. The same is true of `9.2a` there and `7.2a`/`7.2b` in `wire-format.md`.
Renumbering them is a naming decision with reference consequences, not a local defect.

### 2026-08-31 (the change log gains an outline)
0.9-before found this file the hardest to navigate: **5,800 lines carrying exactly one
heading**, with 262 dated entries formatted as bullets, so no outline existed and
nothing could be jumped to. Days are now `##` and entries `###`.

**Entry headings keep their full `date (topic)` label** rather than dropping the date
under a day heading, because entries are cited by date elsewhere in the set and a
bare topic would break those references for the sake of avoiding a repeated string.

**Verified by word census rather than by inspection.** Against the previous revision
the transformation adds 51 words — exactly the 17 day headings — and removes none.
Every dated entry sits under its matching day, and the days ascend.

**Twenty entries had labels that wrapped across two lines** and survived the first
mechanical pass untouched, because the pattern being matched assumed the bold label
and its em dash sat on one line. It is the same line-wrap trap this log has recorded
three times already, and the reason the count was checked against the 262 known
entries rather than trusted.

### 2026-08-31 (§17 retitled)
0.9-before found a status contradiction in a heading: **§17 was "Security analysis —
settled findings"** while containing **§19.4 "Findings requiring action"**. A reader
had to work out what *settled* meant before trusting either label. Retitled **Security
and privacy analysis**, which is neutral and also describes §19 accurately — the
privacy analysis had grown to be most of the chapter. No document cited the old title.

### 2026-08-31 (the migration spec, settled from 0.9-before)
The migration existed as one table cell reading *"the rest in current order"*. 0.9-before
returned reorderings that belong inside it rather than before it — every one renumbers
sections, and a renumber is what the two-phase placeholder method exists to make safe —
so they are folded into an executable spec in `Robot/review-plan.md` rather than applied
piecemeal. **Four decisions, all the author's.**

**Vocabulary moves to §4.** It was §2 while Appendix A and §1 already used its terms, and Appendix A is
leaving for the appendix. This changes the stated order — *preface → thesis → topology →
brief scope* — by inserting vocabulary second.

**§7 splits three ways**: the ceremony, the presence record, and key compromise and
recovery. At 1,818 lines it was 2.4× the next largest chapter; the ceremony is ~1,000 of
those on its own and is not further divisible without cutting one argument.

**Open work becomes two chapters distinguished by release**, not six parallel lists:
what must be settled for the initial release, and what is wanted in a later one. §23.2
already carried exactly that split as three subheadings — *blocks a subsystem*, *decide
during implementation*, *deferred by decision* — so the restructure promotes a
classification the document already had. **The four lists in other documents are
referenced rather than absorbed**: each holds items under its own document's authority,
and moving them into the design would break the split Appendix A establishes.

**`infra-client-requirements.md`'s `4a` and `9.2a` normalise to decimals.**
`wire-format.md`'s `7.2a`/`7.2b` retire in the §7 split without separate work.

Also specified: `wire-format.md` splits §3 — verifier selection is a seven-subsection
mini-specification and the catalog objects are, by that section's own words, not
transactions — and §7, which had outgrown "QUIC binding". The spec carries a five-item
verification list, including a **word census against the pre-migration revision**, which
is what caught the twenty missed entries in the change-log restructure.

## 2026-09-01

### 2026-09-01 (migration, stage 1: network-design reordered)
The chapter reordering and the three splits, executed as one pass. **Line count is
identical before and after** — 7,116 either side — because nothing was written or
deleted, only moved and renumbered.

**The new order** is preface → thesis → vocabulary → topology → scope, then the rest.
Vocabulary moves to §2 because §1 already used its terms while it sat at §3. Document
conventions become **Appendix A**; the decision log moves from Appendix A to
**Appendix B**, taking its A.1 and A.2 subsections to B.1 and B.2.

**Three chapters split.** Proof of presence at 1,818 lines becomes §7 the ceremony,
§8 the presence record, §9 key compromise and recovery. Addressing sheds subnet
formation, which was unfindable under it, to §13. Security analysis becomes §17
analysis, §18 accepted risks and §19 privacy — the last of which had grown to be most
of the chapter.

**2,421 references were remapped across seven files**, far more than the ~420 the plan
estimated, because that figure counted the design's own references and not the six
other files that cite it. **A single-pass regex with a mapping function replaced the
two-phase placeholder scheme**: the collision the placeholders guard against is an
artifact of sequential string replacement, and a single left-to-right pass that never
re-examines what it has written cannot produce it.

**`Robot/review-tracking.md` was deliberately excluded.** Its references are
as-of-filing by the working rules and are not remapped; every other file was.

**Verified**: zero unresolved references across the five root documents, no depth
errors, nothing out of order, fences paired, no unclosed table rows. The word census
shows small positive deltas — 2 to 38 words per file — accounted for entirely by
`§0` becoming the two-word `Appendix A`.

**Two things the mechanical pass could not fix, repaired by hand.** The front-matter
reading order pointed at *"§7 for proof of presence"* and *"§17 records what is known
to be weak"*, both of which now span three chapters; they read §7–9 and §17–19, and
the order now names §2 and Appendix A. `CLAUDE.md`'s instruction to read *"§§0–1
first"* pointed at a section that had become an appendix.

### 2026-09-01 (migration, stage 2: wire-format split at §4 and §7)
**§4 held three unlike things.** Transaction encodings, a seven-subsection
verifier-selection mini-specification, and a family of objects the section itself said
were **not** transactions. Verifier selection becomes **§5** and the registration,
catalog and abuse-report objects **§6**; series reissue stays a transaction and
renumbers from §4.8 to **§4.6**.

**§7 had outgrown "QUIC binding."** It began with the handshake and then carried
topology propagation, the rootward memo and resource request framing. It becomes **§9
Transport binding**, **§10 Topology propagation** and **§11 Resource requests** — and
the split retires the `7.2a`/`7.2b` alphanumerics, which existed only because there was
nowhere else to put those subsections.

**433 wire references remapped across seven files.** Deciding which bare `§N` inside
`wire-format.md` was a wire reference and which a design reference required the
*pre-migration* heading set, read from git rather than reconstructed.

**Verified by differencing word multisets against the previous commit**, ignoring
section numbers. Every file's removed and added counts match exactly, and **the only
non-reference word removed anywhere in the set is `QUIC`** — from the retitled chapter.
Zero unresolved references.

### 2026-09-01 (migration, stage 3: infra alphanumerics, and a defect the earlier stages left)
`infra-client-requirements.md`'s retrofit labels are gone: **§4a becomes §5** and
**§9.2a becomes §10.3**, with the sections after each shifted. No document in the set now
carries an alphanumeric section.

**Stage 1 damaged seven references, and the cause is worth recording.** Its reference
pattern was `\d+(\.\d+)*` with no trailing-letter guard, so `§7.2a` matched as `§7.2`
and was remapped to the design's new §8.1, leaving the nonsense `§8.1a`. **The
specifications escaped**: every `§7.2a` in them carries an explicit
`` `wire-format.md` `` prefix, which the resolver honoured. Only the change log and the
review plan wrote the reference bare, and only there did it break — 3 × `§8.1a` and
4 × `§8.1b`, now §10.1 and §10.2. The review plan's own migration spec was corrupted by
the same pass, its `§4a` rewritten to `§3a`.

**The lesson is that prefixed cross-references survived a defective sweep and bare ones
did not.** The convention that looked like verbosity is what made the corpus repairable.

**Whole-migration verification.** Zero unresolved references across the five root
documents; no heading-depth errors; nothing out of order; fences paired. Differencing
word multisets against the pre-migration revision `14292b9` shows **no prose removed
from any document** — every removed token is a section number, an old chapter title
replaced by the split ones, or `Appendix A.n` becoming `Appendix B.n`.

### 2026-09-01 (migration, stage 4: open work split by release)
Six parallel open-work lists become **§22 Open for v1** and **§23 Deferred to a later
version**, on the author's distinction between what must be settled to ship and what is
wanted afterwards.

**The classification already existed.** §18.2 carried *blocks a subsystem*, *decide
during implementation* and *deferred by decision* as three subheadings; the first two
are v1 and the third is the wishlist, so this promotes a distinction the document had
rather than imposing one. §22 gains the live open questions in detail — peering audit
calibration, replication distance, the divergence-notice object, archive recovery —
and §23 takes autonomous participation, the attention denominator, multi-device and
test vectors.

**Four lists stay where they are**, named by §22.4 rather than absorbed: `wire-format.md`
§13, both client-requirements `Open` sections, and the local block beside the
co-presence mechanism it qualifies. Each holds obligations under its own document's
authority, and moving them into the design would break the split Appendix A establishes.

**The deferral rationale was nearly lost and is restored.** The old §17 explained that
deferred items sit in the chapter rather than the appendix because *a standing choice
that could be revisited reads differently from settled history*. It survived the
restructure only because the word census flagged its vocabulary as removed.

**Whole migration, verified.** Zero unresolved references across the five root
documents; no depth errors; nothing out of order; fences paired; no unclosed rows.
Differencing word multisets against `14292b9` leaves 53 removed tokens in
`network-design.md` and one in `wire-format.md`, every one of them an old chapter title
replaced by a split one, `Appendix A.n` becoming `B.n`, or an intro paragraph rewritten
— and `QUIC`, from the retitled transport chapter.

### 2026-09-01 (0.9-after, pass 1: heading levels and three misfiled blocks)
The post-migration review's mechanical findings, applied as one pass. The review's
headline result was negative and reassuring — **no broad numbering collapse across the
set** — so what remains is local.

**Three heading-level defects.** Design §11's four `11.0.x` subsections had no `### 11.0`
parent, so the numbering entered at depth four; the umbrella now exists as *What the
boundary is, and what it is not*, which is the subject all four already shared. Design
§14.2.4 was followed by six unnumbered `####` headings at its own depth, reading as
siblings when they are its parts; wire §4.5.1 by five. Both sets demoted to `#####`.

**Wire §10 and §10.1 carried the same title.** A chapter and its first section calling
themselves *Topology propagation* tells a reader nothing about which to open. §10.1 is
the `TopologyPush` frame and the rule for forwarding it, and is now titled so.

**Three blocks sat in chapters that were not about them.** Two light-client bullets on
counting *n* and on reporting unverifiable records were stranded after §1.3's closing
rule; both are verification obligations and move to §1.2. A capacity paragraph in
resource §1 pointed at "the manifest (§7)" — §7 is Roles, and the manifest is defined
nowhere; it moves to §8 Packaging and names the manifest inline. Its operator-side twin
sat at the end of infra §10.2, which is about session identifiers, and moves to §9
Package hosting. The two now cite each other.

**Verified**: zero unresolved references across the five root documents, no depth
errors, fences paired. Word-multiset differencing against the prior commit shows the
only removed tokens are the retitled wire heading and the rewritten resource sentence —
no prose was lost in the moves.

### 2026-09-01 (0.9-after, pass 2a: wire §3.2 split, the review's headline finding)
The reviewer called this the most consequential organisational defect in the set:
**231 lines of validation rules filed under the heading *Genesis***, of which genesis
was the first three. Nothing else in the section was findable.

**Genesis moved to §3.1**, where chain back-pointers are defined and where §3.1 had
been forward-referencing it. A three-line section pointing back at the section that
points forward to it is friction with no reader on the other side.

**The remainder became five named subsections**: §3.2 structural rules for presence
records, §3.3 timestamps and monotonicity, §3.4 what structural verification decides
and what it does not, §3.5 the signer set, §3.6 canonicality and version. Each was
already a coherent run of prose; none was rewritten. **§3.5 existed in all but name** —
it opened with a bold **Signer set rules** lead, which is a heading that could not be
linked to.

**The chapter is retitled *Common envelope and structural verification*.** It was never
only the envelope, and the presence-record rules were the evidence: they are consumed by
the same validator pass as the timestamp and signer-set rules, so keeping the structural
contract in one chapter is worth more than a title that describes half of it. The
alternative — moving those rules to §4.5 — would have scattered the contract across two
chapters and renumbered twenty citations to buy it.

**Twenty-one citations retargeted** across `wire-format.md`, `network-design.md` and
this log, each to the subsection that now holds the rule it was reaching for. Two of
them were §3.2 citing itself. The design's structural-verification table row and
`wire-format.md`'s disclosure table row now cite **§3** rather than a subsection,
because what they describe spans all five.

**Verified**: zero unresolved references across the five root documents; word-multiset
differencing shows the only removed tokens are the rewritten genesis sentence and the
old headings.

**Noted, not fixed here**: extending the reference checker to `change-log.md` for the
first time reports **160 unresolved references** there, and 2 in `Robot/review-plan.md`.
**Ruled on the same day** — historical numbering is of no interest and the log should
point at the current design. The remapping is the last entry in this file.

### 2026-09-01 (0.9-after, pass 2b: wire §6 split, and a duplicated block removed)
327 lines with no subsection at all — the largest undivided run left in the set after
§3.2. **Eight subsections, and not one paragraph moved**: §6.1 the catalog entry,
§6.2 registering an entry, §6.3 abuse reports, §6.4 the query and its answer, §6.5 an
entry's lifecycle, §6.6 the scope fields, §6.7 two owners and one resource keyhash,
§6.8 a scope the evaluator cannot compute. The boundaries were already there in the
prose; only the headings were missing, and **§6.4 had a bold *The query and its answer*
lead standing in for one**, the same defect §3.5 had.

**Scope material lands in two places rather than one** — §6.6 for the fields, §6.8 for
the uncomputable case, with §6.7 between them. Making it contiguous would have meant
reordering paragraphs, and a short findable section is worth more than a tidy sequence
bought with a reordering that could strand a reference.

**Two paragraphs were deleted, not moved.** §6 ended with *the patron relationship is
formed bilaterally and ended unilaterally* and *there is no transfer transaction* —
both of them restatements of design §6.2, sitting in the chapter about resources. The
wire text was a strict subset of the design's, which also carries the §3.1.1 collapse
argument and the soft-fork case. **§4.2.1 is where an implementer looks for this**, and
it already said moving to a grandpatron warrants no type of its own; it now says the
same of moving between unrelated patrons, and cites design §6.2 rather than repeating
it. *If the author wants the fuller statement back in the wire format, it belongs
there, not at the end of the resource chapter.*

**Thirteen citations sharpened** from `§6` to the subsection that now holds what they
reach for, across `wire-format.md`, `network-design.md` and all three requirements
documents. The signer-order table's `§6` row was left alone: it spans §6.1 and §6.3.

**Verified**: zero unresolved references across the five root documents, fences paired.
Word-multiset differencing accounts for every removed token — the two deleted
paragraphs, the bold lead that became §6.4's heading, and one full stop that became a
comma in §4.2.1.

### 2026-09-01 (0.9-after, pass 2c: design §7.4 and §18 split, and four citations that pointed at the wrong section)
The last two of the review's four splits. **§18 had the same defect §6 did** — 238
lines, eleven risk bullets, no subsection at all, one bullet running to 91 lines. It
becomes §18.1 a compelled cloud provider, §18.2 a compromised infrastructure node,
§18.3 a stolen key and a stolen device, §18.4 eclipse and occupying a region, §18.5
what a patron can refuse.

**§7.4 was a different shape of the same problem**: seven bullets, of which the first
ran 85 lines with a five-item sublist inside it. Content nested that deep inside a list
item **cannot be cited at all**, which is why §7.4 was cited twenty-five times and
always as a whole chapter. It becomes §7.4.1 oracle leakage, §7.4.2 consent, §7.4.3
availability and what silence is worth, §7.4.4 matching, templates and local storage.

**Thirty-one citations sharpened** to the subsection that holds what they reach for.
**Two of them were §7.4 citing itself** from inside §7.4 — one now names the paragraph
it meant, the other names §7.4.1 across the new boundary.

**Four citations attributed the finalization threshold to §7.4, which states neither
half of it.** The threshold is `min(floor(n/2), 10, |candidates|)` and is defined in
§8.1; what counts toward it — `pending` and `unavailable` alike — is `wire-format.md`
§5.5. Two sites saying *"§7.4 counts `unavailable` toward finalization"* now cite
`wire-format.md` §5.5; the disclosure table's *Finalization threshold* row and P14's
argument now cite §8.1. **The split is what made this visible**: four wrong pointers to
a 191-line chapter all resolved, because the chapter was large enough to plausibly
contain anything.

**Nine headings had no blank line before them**, seven of them new. Two were
pre-existing — design §8.1 and wire §10.1 both sat directly under their chapter
heading. All fixed; the set now has none.

**Verified**: zero unresolved references across the five root documents, no depth
errors, fences paired, and **word-multiset differencing shows not one token removed
from any of the five** — every heading in this pass was added, nothing was rewritten.

### 2026-09-01 (0.9-after, pass 3: one duplication confirmed, one refuted, and four citations pointing at the wrong section)
The review's two duplication findings, verified against the text. **One holds and one
does not.**

**The federation passage was near-verbatim in two documents.** design §11.0.1 and
`resource-requirements.md` §4.1 shared a title, an opening sentence, three bullets and
a closing cost paragraph. The design keeps it — this is rationale, and the design is
authoritative on rationale — and **gains the one phrase only the resource copy had**:
*local accountable intermediaries are structurally required, not merely hoped for.*
Resource §4.1 keeps what a package author needs, which is the obligation rather than
the argument: build for one instance, let the application federate. **172 words of
duplicated rationale removed**; the two citations that depend on §4.1 still resolve,
because §4.1 still says every patron becomes an operator.

**The disclosure-table finding does not hold as stated.** wire §4.5.2 and design
§8.1.1 answer different questions over the same exchanges — the design asks what each
exchange *reads* and whether it needs location, which is the sweep that justifies the
scoping; wire asks what a recipient *sees*, which is what a holder needs before
presenting. Neither is redundant. **What is wrong is wire's claim that design §8.1.1
"carries the same table"**, because the row sets differ: the design counts the trust
metric, which reads adoptions and no presence record at all, and combines the two
`txid`-only exchanges that wire lists separately. Both arrive at eleven rows by
different arithmetic. The sentence now says what each table answers and where they
diverge.

**Four citations to §11.0.1 point at a section about federation while discussing
hosting.** Checked against `14292b9`: **pre-existing, not migration damage** — the
section was §9.0.1 before and carried the same title. One is fixed here, because its
target became definite this session: *"§11.0.1's manifest is where it belongs"* now
cites `resource-requirements.md` §8, where pass 1 put the manifest.

**Three are left for the author**, because fixing them means deciding what he meant,
not what the text says:

- design §11.4's who-sees-what list — *"the hosting path, which is inside the owner's
  own machine or a connection the owner controls (§11.0.1)"*. **Nothing else in the
  set states this**; the claim appears only at its own citation.
- design §11.5 — *"registered with, and answered by, the infra node hosting it
  (§11.0.1)"*. The sentence sits inside §11.5 and describes §11.5's own subject, so
  the citation is either self-referential or meant for `wire-format.md` §6.2.
- design §14.2's visibility table — *"parses and re-serialises the request to insert
  the credential (§11.0.1, `wire-format.md` §11.2)"*. The wire half is right; the
  design half is not obviously §11.0.4 either.

**§18.2's A21 row is the one correct §11.0.1 citation** — it names the federation
pattern, which is what the section is.

**Verified**: zero unresolved references across the five root documents, fences paired,
no heading without a blank line before it. The only tokens removed outside
`resource-requirements.md` are the two rewritten sentences.

### 2026-09-01 (0.9-after, pass 4: heading collisions in this file, and the front matter)
**Seven entry headings collided, and the reason is visible in the outline.**
2026-08-16 is a reconstructed date carrying two whole review programmes: an early one
whose passes are labelled *second run*, and a later one that labels its first round
*all five documents* and then runs its own *second run* series. The two series produced
identical strings. **The later of each pair now carries its own programme's label** —
*0.1 factual verification, all five documents, second run* — which is the vocabulary
that entry's own first round already used, not a distinction invented for the fix.

**Three days had two `##` headings each**: 2026-08-26, 2026-08-28 and 2026-08-31 each
opened, ran, and opened again. Merged. Nineteen day headings become sixteen, and the
outline has no duplicate at either level.

**The document set table was a subsection of the Preface.** design's front matter — the
six-document table, the rule that requirements documents restate no protocol facts, and
the pointer to Appendix B — sat at `###` inside a personal essay about the author's
history with distributed systems. **A reader who skips the Preface skips the map**, so
it is now `## Document set`, a sibling rather than a child.

**§1 uses `patron` and `Dunbar Org` before §2 defines them**, which the reading order
already anticipates — it says to read §2 next for the vocabulary. Rather than move
vocabulary or restructure §1, each term now cites §2 at its first use, which is the
convention the rest of the document already follows.

**The topic index the review asked for is declined, and the reason is in the working
rules.** *Any section that summarises state elsewhere is stale the moment something it
summarises changes* — a topic index over 275 entries would be the largest consolidating
section in the set and the one nothing forces anybody to update. The outline this file
gained on 2026-08-31 is navigable and derives from the headings themselves; a second
index would compete with it and decay. **If the author wants one, it should be
generated rather than written.**

### 2026-08-31 (one word, four jobs: the node software and the resource application get separate names)
Found while the author was correcting a bad finding of mine. **Three citations to
§11.0.1 were reported as pointing at a federation section while discussing hosting;
all three are correct and the misreading was mine** — §11.0.1 is what establishes that
a wide-scale service is *many local instances, each hosted by a patron*, so it is
exactly the authority for a sentence about what a node hosts. Author: *"even in the
case of a third-party service, the infra node runs the authentication package to
access that user-facing service, and so it's still a thing the infra node hosts."*

**What the misreading was detecting was real, and it was not a missing name.** The
distinction — the package on the server, versus the thing the user reaches — is
already stated, and stated well, in `resource-requirements.md` §4 as **Physically** /
**Logically**, with the three-category table covering all three shapes. What was wrong
is that **`application` was doing four jobs**, two of which collide: §2 said *every user
runs the application* (the node software) while §11.0.1 said *an application can be any
scale* (the wider system). A reader meeting §2 first carries the wrong sense into §11.

**The author's ruling settles it without new jargon**: *"2 is the client software and
11 is the resource application."*

**Seven sites now say `client software` or `server software`** — §2's Infra node and
Light client rows, §3.3's deeper-chain rule, §4's in-scope list, and
`light-client-requirements.md`'s opening line. §2's Light client row already said the
term *names software*; it now uses the word.

**`resource application` is anchored where it first appears** in both documents that
use it — *the wider system a resource is one instance of; the network hosts the
instance and knows nothing of the system*. A21 follows it.

**Verified**: the node-software sense appears nowhere in the five design documents.
What remains is the **Local application** category name, the conventional layer sense,
and one *act of applying*. Zero unresolved references; every removed token is an
`application` that was renamed. `change-log.md`'s own occurrence stays as written.

**§2's `Resource` row stays as written** [author]: *"Section 2 is introducing the
resource for the first time and explaining it in familiar terms."* *"A service, data
store or application"* is everyday English orienting a reader who has met none of this
yet; §11's three-category table is the taxonomy and arrives when it is wanted. **Only
the two colliding senses needed names**, and both now have one.

### 2026-08-31 (the change log is repointed at the current design)
**Author's ruling**: *"Historical numbering is of no interest in this case, the change
log should point to the areas of the current design affected."* **144 references named
sections that exist in no document; 26 remain, and every one of those is a sentence
whose subject is the number itself.**

**Git could not do this.** The repository begins 2026-08-25 and the log begins
2026-08-12, so more than half the stale references predate all available history.
Resolving by checking out the document as it stood resolved **7 of 144**. The rest were
resolved by reading what each entry says the section was *about* and finding where that
subject lives today.

**Most of it turned out to be structural, and verifiable by heading title.** Whole
clusters had shifted together: old §11.x is today's §12.x, old §11.8.x is §13.x — the
chapter split in two — old §12.x is §14.x, old §15.x is §16.x, old §16.5.x is §19.x.
Titles confirm rather than assume: old §11.6.5 *key currency: adopt the PKI revocation
playbook* is today's §12.6.5 under the same words, and §11.7.3, §11.7.5, §11.8.6,
§11.8.7.1, §12.1.1, §12.2.4, §15.3.1, §15.4, §16.5.1, §16.5.1.1, §16.5.3, §16.5.7,
§16.5.8 and §20.1.1 all land on an exact title match. In `wire-format.md` the same held:
old §3.5.x is §4.5.x, §3.6.x is §5.x, §5.6.x is §7.7.x, §6.0 is §8.0, §8.2.x is §11.x.

**Positional mapping alone would have been wrong, because a number is not a topic.**
Several resolved differently at different sites, and each site was read:

- **§11.8 is two unrelated sections.** At the 2026-08-13 and 08-16 entries it is subnet
  formation, today §13. At the 08-22 through 08-25 entries it is an open-items register
  inside Resources that was later deleted — its sites resolve to §11.2, §11.6 and §22
  by what each sentence is about.
- **`wire-format.md` §7.3a is `LateResponse` in one entry and `KeyGrant` in another**,
  written weeks apart under different numbering. They go to §7.4 and §7.3.
- **§5.8 is archive fetch in one entry and prekey distribution in another** — §7.9 and
  §7.8. **§5.6.2 is resolution messages in five places and the heartbeat counter in a
  sixth** — §7.7.3 and §8.2.
- **§3.7's eleven sites** spread across §6.3, §6.4, §6.5 and §6.7 by which catalog rule
  each one invokes.

**Four references named an open question rather than content, and a question that was
closed has no successor section.** *"Closes §21.7"* now closes *the routing open
question*; the external root-of-trust item, already annotated *(since removed as out of
scope)*, no longer pretends to a section number. **One collision was caught and
repaired**: §11.8 and §21.9 both fell to §13 on one line, briefly producing *"§13 added,
closing §13."*

**The 26 that remain are a different grammatical case and must not be remapped.** Their
sentences are *about* the old number: *"§4a becomes §5"*, *"§8.2.1 withdrawn — not a
gap"*, *"`§7.2a` matched as `§7.2`"*, *"§21.8 dissolved rather than resolved"*, *"§11.8
deleted"*. Rewriting these to current numbering would make every one of them false.
**A reference that cites a section was repointed; a reference that names one was left.**

**Verified**: only `change-log.md` changed; the five design documents are untouched and
still resolve at zero. No line now carries the same section number twice by accident.

### 2026-08-31 (three stale sites in the two chapters an implementer reads first)
Found while evaluating readiness for implementation, in §22 and §24 — the
consolidating chapters, decaying on schedule.

**§24 step 10 contradicted §22.1.** The build order still said *"the interaction
protocol is not [specified]"* — a statement that was true until 2026-08-16, when the
protocol closed as HTTP/3 over the existing session and §22.1's blocker list emptied.
§22.1 was updated that day; the build order was the missed propagation site, and its
citation had meanwhile migrated onto §11.7, which is Gateways.

**§22.1's request-path citation survived the migration by accident.** It read *"§8.2's
normative evaluation order"* — old `wire-format.md` §8.2, today's wire §11, where
`ResourceResponse` declares its evaluation order normative. The bare number escaped
every checker pass because the design has its own §8.2, presence transactions at the
network layer. A reference that resolves to the wrong section is invisible to a checker
that only tests existence. Now cites `wire-format.md` §11 explicitly.

**§24 step 7 carried a pre-migration range end**: *"(§7.2–6.5.3)"*, the old number for
verification-by-query. The checker missed it because a range's second half carries no
`§`. Now §7.2–7.3.

### 2026-08-31 (draft test vectors, and what drafting them found)
**`test-vectors/` now exists**: six documents and the generator that produces four of
them, added to the document guide. Everything computable is computed — nothing
hand-transcribed — and every generated value was re-verified with an independently
written decoder before filing: all five bodies parse clean with sorted unique keys,
every txid matches, and both Ed25519 envelope signatures verify against an
independently reconstructed `Sig_structure`.

**What the draft covers** is what the readiness evaluation recommended: build-order
steps 1–2 plus verifier selection. Deterministic CBOR atoms, seqno, path, Locator; a
complete end-to-end `SignedLocator` signature; bodies and txids for adoption,
departure, disavowal, series reissue and a formation-subtype presence record; the
adoption's full four-entry envelope; nonce commitments, the seed preimage,
`required()` arithmetic, hash-rank sampling, window boundaries; and 44 negative
vectors, each citing the rule it violates. **No ML-DSA-65 implementation was
available**, so post-quantum slots carry their exact signing input and a marker.

**Drafting is itself a review pass, and it found things**, as `wire-format.md` §13
predicted it would:

- **§3.1's signer-order table had no row for series reissue.** The table's own
  generative rule yields node-then-patron; the row now exists. The table claimed to
  enumerate what the rule generates and was one type short.
- **Seven places the specification under-determines the bytes**, now standing in the
  README as numbered interpretations for the external review to attack — chief among
  them the `SignedLocator` payload (*"canonical CBOR of fields 1–2"* admits a map, an
  array, or a concatenation reading) and the genesis back-pointer's hash input (raw
  keyhash bytes versus a CBOR wrapping).

**The vectors are deliberately pre-implementation.** §23.4 and `wire-format.md` §13
both argued vectors written from the spec alone encode the spec's own mistakes — so
the draft states every choice it made and goes to a different model family to have
those choices attacked, before an implementation exists to inherit them. Both
sections now record the changed status: drafts exist, canonical waits on an
implementation reproducing every value.

**Status sweep**: the document-set table gains a `test-vectors/` row; §23.4's
*absent by decision* and `wire-format.md` §13's *none exist* are updated;
`CLAUDE.md`'s root-contents sentence now names the folder. References across the
five documents and the six vector files resolve at zero.

### 2026-08-31 (test-vector review ingested; the vectors and the wire format both corrected)
The first clean-room review of `test-vectors/` came back from a different model
family. **It reran the generator byte-for-byte and independently confirmed the
adoption txid and the `SignedLocator` signature — no arithmetic error.** What it
found instead is fit, and three of its findings corrected `wire-format.md` itself:

**Three stale references the missing end-to-end vectors had concealed.** §5.4's
witness-only rule said *"names them in field 8, not field 4"* — pre-migration numbers
for what are now fields 4 and 3. §4.5.2's recomputation row read seed inputs from
*"body fields 4, 8, 11"* and the presence body has no key 11 — now 3, 4, 7. And §1.1's
domain-separation table, which calls itself the current enumeration, was missing
`rhtn/1:endpoints`; the row exists now, with a sentence separating the hash/PRF tag
family from the `external_aad` signing contexts.

**The vectors gained what the review showed they lacked.** Witness nonces are now
derived per §5.2.1 — HMAC-SHA-256 from synthetic secrets, re-derived independently in
verification — instead of arbitrary labels. A new adoption vector makes signer order
and kid order diverge, and the formation record's participant order now runs against
keyhash order, so an implementation conflating §3.1's ordering with §3.5's fails a
vector instead of passing by coincidence. A real two-head merge reunites the adoption
and formation branches of one chain. Two must-accept vectors — an unassigned in-range
disavowal code and a bounded unknown extension key — give over-strict decoders
something to fail. Every generated file now pins the SHA-256 of `wire-format.md` it
was generated against.

**The negative suite was rebuilt around the specification's own outcome vocabulary.**
Malformed, unverifiable, incomplete, ineffective, accepted — five classes, and
fixtures split into byte-level, context-dependent (bytes plus external state plus
expected outcome) and method requirements. Six rows the review faulted were wrong and
are fixed: E8's rationale inverted a banded exception, S9 misnamed the Recovery
proof's COSE type, R9 and R10 pretended context-dependent judgments were byte-level
rejections, T6 dropped *for one subject*, and T7 turned out to be **untestable as
specified** — unknown keys are preserved and nothing marks a seed — which goes to the
author as a testability gap rather than a fixture.

**Three blockers stand, by design of the draft**: no fully valid envelope until real
ML-DSA test keys exist (none reachable in this environment — and canonical promotion
was always going to be wholesale regeneration, since the synthetic pubs have no
private halves); no normal-subtype presence record; and selection tested as
arithmetic rather than derived by traversal. These, with the disclosure-root
construction and the boundary sweep, are now the README's **canonical bar** — the
review's own priority order.

**Dispositions in `Robot/review-tracking.md`. Five under-determinations stand as
numbered interpretations awaiting the author**: the `SignedLocator` payload form, the
genesis hash input, §5's raw-concatenation framing, §5.2.1's ordinal encoding, and
merge-list order — each a one-sentence specification fix if confirmed.

### 2026-08-31 (test-vector review, second run: the result model, and E8 corrected twice)
The second clean-room round reran the generator byte-for-byte against the pinned
specification and found no drift; its findings are fit and coverage, and **every one
verified**. The sharpest corrected the previous round's own correction: **E8's
replacement enum was wrong too** — `ClientIntegrity.scheme` is open (*"a validator
checks only the shapes"*), so the row now uses the presence `subtype`, and carries its
own history as a warning that the closed-enum default has exceptions enough that every
instantiation needs checking against its field.

**The single-outcome fixture model is gone.** The suite's own cases — *valid for one
subject, unverifiable for the other* — never fit one scalar, as the reviewer observed;
fixtures now constrain named dimensions of a structured result: structural,
signatures per signer, chain completeness, selection per subject, effectiveness,
evidentiary flags. Over-strictness is stated as non-conformance alongside
over-acceptance.

**Generated files now pin both documents.** The design wins on disagreement, so a
design-only semantic change stales vectors the wire pin alone would call green.

**Coverage added, every value independently re-verified**: a positive peering vector
(showing optional-field omission live); a complete classical `EndpointRecord` under
`rhtn/1:endpoints`, opening `records.md` toward one known-answer signature per
domain-separation context; a counter-jump `SignedLocator` pair and a reissue to a
numerically smaller series, the two must-accepts that catch contiguity and
generation-counter misreadings of §2.3; the unknown-extension adoption now fully
signed, with independent confirmation that mutating the unknown key breaks all
signatures; five COSE-profile negatives a default library would pass (embedded
payload, tagged nesting, wrong `external_aad`, extra protected parameter,
out-of-profile alg); and four Recovery cross-binding negatives, each checked against
a stated rule.

**Two of the reviewer's proposed Recovery negatives were declined**: self-adoption
and `prior_key` equal to the new key are stated nowhere, so they went to the author
as questions rather than into vectors asserting rules the text does not carry. The
author queue now holds five items; dispositions in `Robot/review-tracking.md`.

### 2026-08-31 (self-adoption is structurally malformed)
**Author's ruling.** An adoption whose node and patron are the same identity is
rejected from the bytes. The framing that makes it belong in `wire-format.md` §4.1:
a self-adoption is the **degenerate cycle** — the proposed patron *is* the node —
and it is the one cycle a validator can see from the record alone, where design
§6.2.5's rule otherwise requires topology state and rejects on positive knowledge
only. Negative vector T13 added; the vector set regenerated against the new
specification hash. **The sibling question — whether departure, disavowal, peering
and series reissue reject the same degenerate pair — is put to the author, not
assumed.**

### 2026-09-01 (eight rulings: the lost-archive path, and five spec sentences the vectors asked for)
The author ruled on the whole test-vector queue and one design question in a single
pass; everything below is applied.

**The retained-key, lost-archive case is an adoption, not a recovery** — and the
design already carried every mechanism it needs. The sequence, now stated where each
piece lives: **refetch first** — sibling replication means the patron holds the
history, and `ArchiveRequest` field 2 is now optional, absent meaning the holder's
newest, because the head to walk back from is the one thing a client that lost
everything cannot name. **Else adopt afresh** on a new series: §2.3 now says a new
adoption *establishes* its relationship's series, countersigned by the same party a
reissue would need — the previous "advanced only by §4.6" read literally would have
stranded exactly this user, since a reissue must name the counter the old series
reached. Adopting an established key that presents no history is the patron's
discretion, like every adoption. **Merge when the past resurfaces**: the two-head
back-pointer list reunites the branches, and the fork-and-merge machinery built for
two devices absorbs archive loss unchanged. Accordingly **`prior_key` MUST now
differ from field 1** — a same-key Recovery is vacuous evidence — and the vectors
gain T14.

**Trust, surfaced rather than hidden.** A headless archive fetch verifies its chain
internally while its *newestness* stays the holder's claim; §7.9 says so, and
`light-client-requirements.md` §2 gains the interface obligation: present the whole
lost-archive path — restore-on-trust, re-adoption on personal judgment — as what it
is, and never present a restored archive as verified-complete. One paragraph at §4.6
records that sealing needs the series number the archive took with it: benign in
loss, moot in theft, where rotation is the remedy.

**Self-adoption is malformed** — the degenerate cycle, and the one cycle a validator
sees from the record alone where design §6.2.5 otherwise needs topology state (T13).

**Two vector interpretations became specification sentences.** *"Canonical CBOR of
fields X–Y"* now means the **map** of exactly those fields, globally — one sentence
in §1 resolving eight signed objects, the author's reason being the practical one, a
map is debuggable where a concatenation is not. And §5.2.1's PRF is **normatively
HMAC-SHA-256**, ordinal 8 bytes big-endian — nothing interoperates on the value, so
naming it costs nothing and closes the honest-homebrew risk, the same move RFC 6979
and Ed25519 made. The seed sentence in §4.1 is restated as the writer commitment it
always was.

**The vectors regenerated against the new pins**; the nonce table is promoted from
reference example to client-conformance vector, and the independent re-derivation
still matches. Still open: the three remaining interpretations, the degenerate-pair
question for the other two-party types, and whether a root can reissue at all.

### 2026-09-01 (the degenerate pair is rejected everywhere; a mis-recorded ruling corrected same-day)
**The author's ruling, corrected by the author within the hour**: every two-party
type — adoption, departure, disavowal, peering, series reissue — rejects a
transaction naming the same identity on both sides, as presence records already
reject equal participants. The first reading ("adoption's alone") was briefly
applied and never pushed; the correction replaces it. The generalised rule sits at
`wire-format.md` §4.1, noting the two independent agreements: presence's §3.2 rule,
and — for the two-signer types — §3.5's no-duplicate-signers rule, which the
collapsed signer set collides with anyway. Negative vector T13 now spans the five
types.

**A consequence sharpens the open root question.** With self-reissue malformed, a
root — having no patron — cannot produce a type-7 at all, and §2.3 makes a series
claim *proved, not inferred*, by a chain a root cannot produce for a transition.
Advice on what a root's series therefore is, and what an unlinked series means to a
consumer, is with the author.

### 2026-09-01 (the last three interpretations become sentences; the root's rollup is an internal operation)
**The vector suite's interpretations register is empty.** The genesis value hashes
the raw 32 keyhash bytes, not a CBOR encoding (§3.1); §5 states its hash-input
convention once — raw concatenation, safe there because every component after the
domain tag is fixed-length, so the concatenation is injective, and unsafe as a
convention anywhere else; and a merge back-pointer list sorts ascending bytewise
(§3.1) — one logical merge, one encoding, one txid, where deterministic CBOR alone
would have allowed as many txids as the heads have permutations. Negative vector
E11 is the merge rule's complement.

**The root series question closed on the author's refinement of the draft answer.**
A root cannot produce a type-7 — the patron slot cannot name a distinct party and
an exception could not be checked — but the *rollup point* survives without the
transaction: the root continues its hashchain under a new series designator, an
internal operation with no countersigner because for a root the countersignature
never gated anything. The chain back-pointer is the real predecessor, so an
observer holding the chain sees ordinary continuation, distinguishable from a
genesis event; **presented as a history root it is logically equivalent to one**,
which is what a rollup point is (§4.6, with §2.3 pointing at it). §2.3's currency
rule stands unchanged: a series is proved by the chain into it, and one nobody can
link stays unprovable.

**With this, nothing about the test-vector suite awaits a decision.** What remains
is the canonical bar — work, not rulings.

### 2026-09-01 (third vector review: eleven applied, one for the author, and a duplicate the record must own)
The verification round reran the generator — five outputs byte-for-byte, both pins
matched — and found no arithmetic drift. Its findings are taxonomy, object model and
result model, and **eleven of twelve are applied**.

**The one that needs the author is the type-6 taxonomy.** §4's table assigns type 6
to the abuse report while §6.3 defines that object with no key 0, its own embedded
signature and no carriage — and §3.1's signer-order row still names *resource
registration*, which stopped being a transaction on 2026-08-28. That retirement's
rationale — *no archive advance, chains to nothing, an envelope nothing walks* —
reads on the abuse report verbatim, so the question is whether type 6 follows it
into retirement with a tombstone row, or a type-6 envelope gets key-0 semantics for
a signer that keeps no archive. The vectors' claim of "no open interpretations" did
not survive this finding and now says so.

**Applied**: `records.md` no longer implies signatures the specification does not
define — the capture key grant, late-response wrapper, resolution and archive-fetch
messages are unsigned encodings, verified one by one, and are now listed as such
apart from the signed contexts. D2/E10 claim what placeholders can prove — both
classical signatures, the PQ slots non-oracular. The result model gains `failed`
and a per-check dimension, unhooking the withheld-`strongest` outcome from the
selection dimension it was leaning on. **A departure envelope joins the adoption's**
— the single-signer shape, two entries, independently verified, carrying §4.2's
rule that a decoder MUST NOT expect the patron's signature. Fifteen negative rows
join the suite: the KeyMaterial family (P9–P14), the outer `COSE_Sign` headers
(S15/S16), formation inverses (R12/R13), the successor `patron_key` binding (T15),
the reissue counter-0 rule (T16) and series reuse as a context fixture (V4). The
boundary sweep now says *planned*, and its target list absorbs the peering-audit
and endpoint-list bounds that were mislabelled out-of-scope.

**And a correction owned in `review-tracking.md`**: round 2's claim that
self-adoption and prior-equals-new were *stated nowhere* was false — §4.1's
successor block carried both, in phrasings the verification grep did not try. The
author's rulings confirmed existing text, the additions duplicated it, and the
duplicates are now consolidated into the ruled statements. One phrasing is not a
sweep.

### 2026-09-01 (type 6 retired; the window ordinal becomes structural)
**Both third-review questions ruled.** Type 6 follows the registration into
retirement: the abuse report is §6.3's standalone signed object — it advances no
archive, reaches only its addressee, and chains to nothing, the same grounds as
2026-08-28 — and the §4 table keeps a tombstone row because numbers are never
reused. §3.1's signer-order row for "resource registration, abuse report", half
stranded since 08-28, goes with it.

**Key 7 MUST equal `floor(started_at / 86400)`.** Both values are body fields, so
the check is clockless — the `finalized_at` class — and a record whose ordinal
disagrees with its own `started_at` is lying about which window seeded its verifier
sample. Negative vector R14; the formation vector already satisfies the rule.

**The sweep found one more count adrift**: the design's §4 scope list still named
five transaction types, missing series reissue since type 7 arrived on 2026-08-30.
Completed. The vectors' interpretation register and author queue are empty again —
this time with the taxonomy ruled rather than silently assumed.

### 2026-09-01 (fourth vector review: the pin gate, the conflict pair, and two contradictions for the author)
Ten of thirteen findings applied; the two that remain are specification
contradictions only a ruling can resolve, and the vector exercise is what exposed
both.

**`pending` cannot be built as specified.** The design queues queries to an offline
light client at its patron and returns `pending`; the wire makes `pending` a
result inside a response the **verifier** signs — who is, by construction, offline
— and no patron-authenticated variant exists. §5.5 counts `pending` toward
finalization, so this is load-bearing, and the options are in the README: the
queue-holder attests delivery, or absence becomes the encoding and the counting
rule changes.

**§1.1 carried a thirteenth row for a context that does not sign.** A
`VerificationQuery` is hashed into `query_id`; what gets signed is the id, under
consent. Row removed — the table is twelve rows and the stated count of twelve is
true again — with the canonical-form pointer folded into the hash-family note, and
one observation left beside it: `query_id` is now the profile's only undomained
hash of a CBOR map.

**The window prose contradicts its formula** — *"closed at the near end"* against
`lower < finalized_at < started_at`, strict at both ends. The vector implements
the formula; the author picks the word or the boundary.

**The generator now refuses to stamp new hashes onto old assumptions.** A changed
specification hash stops generation until `--accept-spec-change` acknowledges an
audit — tested in both directions — and the two hand-authored files carry a
machine-managed pin line, so the negative suite can no longer go stale silently.
The equal-seqno conflict pair is generated rather than described: two
independently verified `EndpointRecord`s by one signer, same `seqno`, different
endpoints — individually valid, jointly the malformed condition no reader may
break as a tie. Eight negative rows and three canonical-bar items join the suite:
the wrong-identity signer (S17), the `VerifierResponse` conditional matrix
(T19–T23), endpoint duplicates and the u16 port edge, the unsigned message
families, per-type signer binding, and the finalization semantics with the
commitment-mismatch and committed-predecessor traps.

### 2026-09-01 (absence is the encoding: `pending` leaves the wire; the window is exclusive at both ends)
**The fourth review's two contradictions, ruled and applied.** The author: *"Late
arriving replies are private information for participants, not part of the
record."*

**`pending` is gone from the `VerifierResponse` enum** — it was never
constructible: the offline verifier could not sign it and its patron held no
authority to. An unreachable verifier now answers nothing, and **its slot is
absent from field 5**, visible to any evaluator recomputing the deterministic
selection. With that, **the threshold changes character: it sizes the sample and
no longer gates finalization.** Wire §5.5 is rewritten as *The selected slots,
and what the record carries*; design §8.1's invariant is renamed *The verifier
sample* and its blockquote now says *select and query* where it said *require …
to finalize*. The anti-DoS rationale inverts rather than weakens: an attacker who
can make verifiers unreachable no longer needs answering at all, because absence
blocks nothing — what suppression buys is a record that advertises its own
thinness. Late replies resolve privately to the participants and are never
retro-inserted; a responder wanting durability has `LateResponse`, unchanged.

The sweep touched fourteen sites across the two documents — ceremony summary,
schemas, withholding posture, the stolen-device analysis, the dropped-query
paragraph, both tables, P14 — renamed the unset parameter to *queued
verifier-reply patience*, and caught one stale citation riding along: the q-vs-n
aside pointed its threshold at §12.2, which is the anchor set. Vectors: T20
records that result 4 is now an unknown enum value; **V7** is the new must-accept
— a record with absent selected slots is valid, and a decoder demanding a full
slot set is non-conforming.

**The 730-day window is exclusive at both ends** — *previously completed
ceremonies only*. The word "closed" is gone from §5.3.1; the formula and the
vector were already strict, and the boundary table now states the rule in the
author's words. The vectors' author queue holds one item: whether `query_id`,
now the profile's only undomained hash, gets a tag.

### 2026-09-01 (the hash-disjointness invariant)
The last open vector question closes as a stated invariant rather than a new tag.
**§1.1 now records why the four untagged hashes are safe**: `txid`, `keyhash`,
`query_id` and the genesis value have pairwise structurally disjoint preimage
languages — a body's mandatory key 0, `KeyMaterial`'s two-element array shape, the
query's key-1-first five-map, and a fixed 32-byte input shorter than any other
preimage — so no digest can be reinterpreted across roles without a SHA-256
collision across disjoint languages. **And the disjointness is now a rule, not an
accident**: any future hashed object must either stay structurally disjoint from
every language above or carry its own `rhtn/1:` tag. Stating the invariant was
chosen over tagging `query_id` because the tag would have protected one hash and
left the other three resting on unstated luck — the drift §1.1 already records
happening once to the signing roles.

### 2026-09-01 (the vectors go cryptographically complete: real ML-DSA-65, cross-verified)
The last tooling blocker fell — the machine gained pip, PEP 668 routed the install
into a target directory — and canonical bar item 1 executed end to end.

**The keygen recipe came first, as the fourth review required**: `xi =
SHA-256("rhtn-test-vectors:<name>:ml-dsa-65-seed")`, keypair = FIPS 204
`ML-DSA-65.KeyGen_internal(xi)`, deterministic signing with empty context. **And it
was proved implementation-independent before anything regenerated**: a second,
independent ML-DSA implementation re-derived every public key from the same seeds
and verified the first implementation's deterministic signatures. A second
implementation deriving the same keys from a stated recipe is the opposite of
taking generator output as an oracle, which was the fourth review's exact worry.

**Then the wholesale regeneration** every reviewer said this day would bring: real
keypairs change every keyhash, so every body, txid, seed preimage, rank and
bytewise ordering recomputed — mechanically, because every value flows from the
generator. The placeholder machinery is deleted; every envelope is final; the
extension envelope's mutation property now breaks **all four** signatures rather
than the two classical ones. Independent verification of the regenerated set: 11
keyhashes re-derived from scratch, 11 bodies and txids, 4 envelopes with 14
signatures all verified under the second implementation, nonces and seed
re-derived.

**The suite is cryptographically complete.** What canonical status awaits is
unchanged in kind and now singular: an independent implementation reproducing the
whole suite — which is the gate `wire-format.md` §13 set on the day the vectors
were first drafted.

### 2026-09-01 (fifth vector review: two stale survivors of earlier rulings, and the suite's process hardened)
All ten findings verified and applied, and for the first time a round's
specification findings needed **no new ruling** — both were sentences the earlier
rulings' sweeps had missed. §3.2 still said an unmet threshold keeps a ceremony
unpublished, a survivor of the absence ruling whose fourteen-site sweep was one
site short (and it hid a stale `LateResponse (§7.3)` reference besides); and §4.5
still generalised *"both signatures here are classical-only"* over a field whose
own comment states the Recovery-hybrid exception. Both now say what the rulings
decided.

**One sentence and eleven fixtures close the round's gaps**: the type table states
that *the tombstone reserves the number; it does not readmit the bytes* — with S18
and S19 as the retired-type and unassigned-type fixtures; E12 separates an
arbitrary CBOR key from a uint extension key; S20–S22 give the standalone
`COSE_Sign1` path its own kid, unprotected-header and context-forbidden-algorithm
negatives; and a **generated** extension-carrying `EndpointRecord` (D8, with
mutation complement E13, both independently verified) proves §1's coverage rule on
the Sign1 reconstruction path, as D2/E10 prove it for envelopes.

**Process hardening from the review's two sharpest observations**: a missing
`spec-pins.json` is now fatal without a `--bootstrap-pins` flag distinct from
`--accept-spec-change` — deleting the pin file no longer converts unaudited
specifications into a silent baseline — and every generated write is explicit
UTF-8 with fixed newlines, with the producing generator's own SHA-256 recorded in
the pin file as provenance. The canonical bar tightened in two places: the merge
history must be a **diamond** (the fixture that fails chain code with no visited
set), and every conformance case must resolve to exact bytes or a deterministic
mutation of a named positive vector — a corpus, not a description.

### 2026-09-01 (sixth vector review: the monotonicity gap, and the harness joins the corpus)
The round's most consequential finding was its last-listed and first-priority one:
**§3.3 bound effective time for presence records only, while §5.4's pruning and
§3.2's chronology rationale rely on monotonicity along every verified chain** — as
written, a non-presence transaction could bridge backward through a chain and
break both. §3.3 now states the rule for every type: each record's effective time
must clear every committed predecessor's, merge heads included, and the reliance
is stated as the reason. T24 is the temporal-bridge negative.

**Two more stale survivors fell**: `LateResponse` still said the record "already
met its threshold" (with a mis-aimed §4.5 citation beside it), and the unsigned
unknown-key rule named five message families where its own rationale is fully
general — it now lives in §1 as a rule for every message outside a signature's
coverage. **And one drift was the vector suite's own**: R11 called a revealed
`strongest` violation a failed check when §3.2 calls the rule *structural* —
revealed-and-violated is malformed, §3.2 now says so in words, and the row
records its own correction.

**The suite gained its independent harness**: `tools/verify.py`, sharing no code
with the generator — its own decoder, its own Sig_structure reconstruction —
re-deriving all eleven identities, verifying all fourteen bodies and every
signature under two ML-DSA implementations, and proving the mutation properties.
Eighteen checks, exit-nonzero, part of the corpus so the cross-implementation
claims are reproducible rather than asserted. **The generator hardened around
it**: construction assertions on every primitive, both hand-typed tables now
generated from the functions they document, pins v2 gating the generator itself
and the newly pinned `light-client-requirements.md`, and every output's SHA-256
recorded.

**New vectors, all harness-verified**: a wrong-signer `SignedLocator` — bob's
cryptographically valid signature under alice's name, the binding the only defect
(S23); a nested unknown key inside the extension adoption's `Locator`, both
mutations breaking all four signatures; and three optionals-exercised bodies, so
no schema field exists that no positive vector decodes. Fixture rows P15–P18,
T24, V8, C3 and bar items 12–13 complete the round. Dispositions in
`Robot/review-tracking.md`.

### 2026-09-01 (seventh vector review: semantic coherence, and the harness schema decided)
The round re-hashed every pin independently — exact match — and found **no new
encoding ambiguity**: for the first time, a review states that every supplied byte
vector appears derivable from the present text. What it found instead was one
semantic incoherence and a set of harness-schema decisions being made silently.

**The optionals adoption's field 8 pointed from an alice–bob adoption at the
alice–carol formation record** — structurally legal, since dereference is an
evaluation step and not a structural one, but presented as a positive exercise of
the field while referencing a record that cannot support the adoption. The vector
now says exactly what it is and why: the suite's only presence record names the
wrong pair, and generating a supporting one today would put alice in two formation
records, violating §3.2 inside the positive universe. The reference swaps to a
genuine alice–bob record when the normal-subtype fixture lands, and V9/V9b turn
the mismatch into what it should have been from the start — the dereference
context fixtures, under a **newly decided home for reference evaluation**:
`checks[proof_of_presence] = fail | unverifiable(unfetchable)`, with `effective`
and `chain` explicitly not absorbing it.

**The hash-disjointness invariant now has a mechanical guard**: generation asserts
the four preimage-language classifications over every body and key, so a schema
change that broke disjointness would fail the generator rather than wait for a
reviewer. The harness's claim is narrowed to what it proves; bars 2 and 3 are
marked promotion-blocking in the reviewer's own framing — the arithmetic tables
test none of the specification's hardest derivation; and the canonical bar
absorbed cross-context signature substitution, unsigned-family result-code
matrices, machine-readable fixture identity, and a unit-fixture class for
requirements no natural wire input can instantiate. **Four harness-schema
assumptions are now recorded in the README as their own register**, distinct from
the retired encoding interpretations.

### 2026-09-01 (eighth vector review: the harness audited as hard as the vectors)
The round's sharpest findings were against the verification harness itself, and
all held. **`verify.py` was using decode-and-re-encode to judge canonicality — the
exact method the suite's own E9/C1 rows forbid a conforming decoder to use.** Its
parser already rejected duplicates and unsorted keys before materialising, which
contained the damage, but shortest-form checking rode on the re-encode. The parser
now rejects non-shortest forms at byte level and the re-encode survives only as a
cross-check of the harness's own encoder. **Its Sign1 helper inferred the
signature slot from key magnitude** — an unknown extension key below 10 would have
been misread as the signature — and that trap is now a generated vector: a
`SignedLocator` carrying unknown key 4, directly above its schema slot, with its
mutation complement, both verified. Text strings get a distinct representation so
the future catalog and disclosure vectors cannot be silently mis-encoded, and the
harness itself is now **pinned and gated** — a tool change alters what *all checks
pass* means, so it takes the same acknowledgment a generator change does.

**One finding goes to the author, and it is the round's real discovery**: the
generator asserts every path has at least one nibble, and §2.1 states no lower
bound — an unstated protocol assumption hiding in a constructor. The question has
protocol shape: may a locator's path be empty, a node that is its own anchor,
which is what a root would publish? Reopened in the interpretations register.

**Eleven fixtures round out the coverage**: the missing response cross-bindings
(T25, T26), the stateful series traces — a post-reissue record in the abandoned
series rejected at any counter, and two current-looking series that never rank
numerically (V10, V11) — the exact-boundary must-accepts (`finalized_at ==
started_at`, the full 24-hour gap, the idempotent `EndpointRecord` replay: D10–
D12), and the extension pair D9/E14. The unsigned inventory gains the currency
request/reply it had omitted — with the omission recorded in the inventory itself
as the standing argument for enumerating it mechanically.

### 2026-09-01 (roots legitimately self-anchor; the empty path is stated)
**Author's ruling on the eighth review's discovery.** A path may be empty — zero
nibbles, the empty byte string, count 0, `{1: h'', 2: 0}` its one encoding — and
the case is not an edge but a population: it is what **every root publishes**,
having no ancestor to name. Wire §2.1 states it; the design's Anchor vocabulary
row and §12.1's anchor bullet gain the root clause; the generator's silent ≥ 1
assertion — the unstated protocol assumption the review caught — is corrected;
and D13 is the generated must-accept, a root's complete self-anchored
`SignedLocator` verified end to end. A decoder asserting a minimum path length
rejects every root's locator, which is exactly the class of over-strictness the
must-accept suite exists to catch. The vectors' interpretations register is
empty again.

### 2026-09-01 (ninth vector review: the harness becomes schema-aware; a bound-table contradiction falls)
Two findings stood out. **The wire format's own bound table contradicted its
peering schema** — eight `NetworkPoint` entries per "peering endpoint" against
§4.4's strictly singular fields 3 and 4; the schema governs, and the row now says
so and why. And **the harness, for all its cryptographic rigor, was not checking
what the positives claimed to be**: it verified canonicality and signatures while
the generator remained the semantic oracle for schema shape. `verify.py` now
validates every envelope's body against a per-type schema and **derives the
required signer set from the body itself**, asserting the envelope's `kid` set
matches — the check §3.1 warns is silent when wrong, now independent.

**The result model gained its missing dimension**: `state_action` — install,
replace, replay, ignore_stale, conflict, incomparable — the vocabulary for what a
holder's store does with an individually valid freshness-bearing record, which
`effective` was never meant to absorb. The equal-`seqno` conflict now exists on
**both** decoding paths — a generated `SignedLocator` partner joins the
endpoint-record pair — and the must-accept suite gained the three valid role
overlaps a strict implementation plausibly rejects, including the
uniform-`nominated_by` record that is wire-valid while being exactly what the
client warns about. **The pin process closed its last silent path**: outputs are
now compared against the stored pins when specs and tools are unchanged, so a
crypto-dependency drift cannot baseline itself; dependency versions ride in the
pin file. Nine new fixture rows seed the schema-shape and parser-branch matrices.

**Two questions to the author**: may the default port be written explicitly —
recommended no, a field equal to its default MUST be omitted, the scalar analogue
of the optional-empty rule, and §7.6's distinctness needs the ruling either way —
and what an unknown extension value may be, with slice-opacity recommended over
type narrowing.

### 2026-09-01 (two §1 rules: defaults are omitted, extensions are opaque slices)
**Author's rulings on the ninth review's questions.** A field equal to its stated
default MUST be omitted — the scalar analogue of the optional-empty rule, for the
same reason: a written-out default gives one logical object two encodings, and
§7.6's distinct-entries rule would otherwise have to decide whether `{ip}` and
`{ip, port: 7431}` are one destination or two. Omission is the one spelling; E22
is the negative, and the generator's constructor now refuses the mistake.

**And unknown extension values are opaque encoded slices, preserved and never
interpreted** — *uninterpretable state kept for a reader that may understand it
later*. Any deterministically encoded CBOR item is admissible; the profile
governs the slice's framing and nobody else evaluates its meaning, which is why
preservation works by slice and never by reconstruction through a typed model.
D18 is the fixture that catches exactly that reconstruction. With these, every
question accumulated across nine review rounds is ruled and applied.

### 2026-09-01 (bar 3 reworded: the handed bundle, not an archive walk)
**Author's correction of the canonical bar's framing.** Item 3 described deriving
*n* "by DAG traversal" of the subject's history — wording that re-imported the
archive-walking reading deleted during 0.8 (P37). The design is the **bundle
model**, and `wire-format.md` §5.4 states it: a counterparty computes *n* and the
candidate set *over the records the subject hands it*, and the traversal rules
are what make the handed bundle **checkable** — records must chain, so a missing
middle record fails to connect. The fixture survives as what it always should
have been: a constructed bundle in which every processing rule has a case that
changes the answer, with completeness carrying no certainty and needing none —
overstatement impossible, understatement visible as a bundle that fails to
connect, and history elsewhere invisible by design.

### 2026-09-01 (the disclosure construction is real; the formation record becomes whole)
Campaign (a) of the canonical bar opened on the author's go-ahead, and its first
slice landed: **§4.5.1 is implemented for real**. Seven labelled, salted
disclosures per record — deterministic salts, stated as a vector convention —
digests prefixed `0x00`, the root prefixed `0x01` over the label-ordered digest
run. The formation record's synthetic root is gone: **field 8 is now the genuine
root**, the record gained its type-5 envelope, and three `PresentedRecord`s —
fully revealed, partial, minimal — verify against the same body root under the
same signatures. The harness recomputes all of it independently: every digest,
the root, and the three presentations; its schema table now validates type 5's
dynamic signer set alongside the five fixed-count types.

**And the bar's item 3 was reworded on the author's correction**: the fixture is
the **handed bundle** — nobody walks anyone's archive, and the phrasing that
suggested otherwise re-imported a reading deleted during 0.8. The traversal rules
are what make a supplied bundle checkable, which is §5.4 verbatim.

### 2026-09-01 (the bundle is curated, not chained: §5.4's traversal apparatus dissolves)
**Author's correction, and the session's largest simplification.** *"You can
cherry-pick whatever PoP transactions you wish from any of your series and do not
have to expose the intervening transactions."* Wire §5.4 had built a traversal:
*n* counted records **reachable from the committed back-pointer**, merge paths
deduplicated by visited set, a gap made the bundle *incomplete*, pruning stopped
at the window boundary, and the committed root closed a backfill argument. All of
it described a contiguity the design does not want — the same archive-walking
reading deleted during 0.8, rebuilt one layer up.

**The replacement is smaller and stronger.** A bundle is a set of individually
verifiable records from any of the subject's series; a record qualifies alone —
canonical, content-addressed, signed, subject a participant, inside the window;
duplicates count once by txid; a failing record contributes nothing rather than
making anything incomplete. Understatement is free and self-defeating — a smaller
*n*, a thinner sample, a record advertising less corroboration, the absent-slot
posture one layer up. Overstatement is impossible. And **the chain's job passes
to the record itself**: each party verifies the other's selection before signing,
so the responder slots in field 5 are the durable commitment — a selection
recomputed over any other bundle visibly fails to reproduce them. Design
§8.1.2's *"chain to the commitment"* clause is corrected, the light-client
count-*n* bullet now says verify-per-record and pin-by-selection, and the
canonical bar's fixture sheds its diamond, committed-predecessor and
bundle-minus-one cases — machinery for a model that no longer exists. What
remains to build is simpler: records that exist and verify, curated into bundles.

### 2026-09-01 (verifier selection redesigned: recognition replaces recomputation)
**The author's redesign, and the deepest simplification since the roles table.**
The PoP is a connection between individual users, outside any subnet boundary; what
it must deliver is the participants' own confidence in who they met, provable to
another person who has met the same counterparty. Against that purpose the
deterministic machinery was doubly wrong: a hash-rank sample over a bundle the
subject curates is the curator's pick with extra steps — *"Eeny, meeny, miny, moe
over a list populated by someone else is the same as just letting them select the
output"* — and the determinism only ever served a distant audience, one that could
not verify the pick without the complete history the bundle model and the privacy
posture both refuse to grant.

**Selection is now by recognition.** Each party still selects the other's
verifiers; the number sought is still `min(floor(n/2), 10, |candidates|)` — but
stated for what it always was, a reasonableness criterion over a number entirely
under the verified party's control. The selector picks people it can vouch for, in
four descending tiers: users it has met; users in any of its **trust horizons**
(the operative term for the two-edge walk — self-centred, no fixed set; *Dunbar
Org* remains the theory chapters' name for it); users its horizon-mates have met;
users in the horizons of users it has met. Two edges over the graph of meetings
and horizon-mates is the halting condition — a second two-edge walk, mirroring the
horizon itself. Where the bundles surface no common acquaintance, the parties **go
fishing** — proposing further candidates over the direct channel, carried by no
wire object — and fill the remainder at their discretion, **each response carrying
the selector's claim of its basis** (new field 10: known, reachable,
discretionary).

**What fell**: the witness nonce commitments and reveals (Witness fields 4–5), the
seed and its window ordinal (body key 7), hash-rank sampling, the anti-grinding
property, the recomputability invariant, §5.2.1's freshly-normative HMAC, three
hash-domain tags, a §21 parameter, and every fixture that tested them — all
retired with numbers and ids unreused. **What was rebuilt on the surviving legs**:
§7.4.1's oracle defence now rests on per-query grants, the counters, and bundle
curation rather than scatter; §7.3's shared-identity detection rests on the
selector's acquaintances being beyond the confederates' reach; §10.1's
tamper-evidence argument now distinguishes the two presentations — the curated
ceremony bundle, defended by recognition, and the chained standing presentation a
patron walks, defended by the chain. Consent, the verification process, and
cross-selection are unchanged. Twenty-five harness checks pass against the
regenerated vectors.

### 2026-09-01 (§7.4.1's concentration note sharpened: the reference side, never the schema)
The author challenged the redesigned oracle paragraph's wording — *"is this
claiming a hostile counterparty can probe the underlying biometric with multiple
schemas?"* — and the answer is no, with the machinery unchanged and now stated in
place: the pre-commitment pins one profile per ceremony, the subject countersigns
that profile or none, and every verifier rejects a profile differing from another
countersigned under the same pre-commitment, under any selection rule. What
selector choice permits is narrower and now said precisely: aiming **successive
ceremonies'** single probes at the same verifier's stored captures, where the
seed used to scatter them — a cleaner gradient against one reference set, still
priced at a full witnessed ceremony per probe. The paragraph also names the
tell: the same counterparty selecting the same verifier ceremony after ceremony
is exactly the pattern the query-surfacing rule exists to show the subject.

### 2026-09-01 (full consistency pass and de-lint; the set baselines for review cycle 2)
The author called the close of the first review era: enough changed in the 0.8
rounds and the nine vector passes that **the programme restarts from 0.1**, against
the cleanest set constructible. This pass produced it.

**Consistency findings, all repaired.** Seven residues of the retired selection
machinery had survived their own rulings: the ceremony's step 6 still said
*deterministically selected* two sentences before *selection is by recognition* —
and never introduced the bundle handover the selection reads from, which it now
does; `LateResponse` still required its verifier to have been *in the selected
set*, a set that no longer exists — consent is the gate; the offline-client bullet
and both withholding-visible passages still spoke in absent-slot language; §19.2's
composition tension still rested the visibility requirement on anti-suppression —
it now rests on recognition, and the aggregate-signature direction is priced
accordingly: an unnamed responder cannot be recognised, so that escape now costs
the record its value; and a pre-existing *"subnet trust envelope"* — the author's
own later-disowned word — became *outside any subnet boundary*. One dangling
argument in the light-client document still cited §7.3's retired detection
arithmetic.

**Counts, re-verified against their tables**: twelve signing roles, twelve rows;
eleven unset parameters, eleven rows; eleven exchanges claimed, eleven rows in
both disclosure tables; the light-client's *three checks* still count three; §21
still carries the 24-hour figure §3.2 leans on; seven disclosure labels. The
*ten-of-eleven* wording in both disclosure tables remains as previously recorded
— the author has seen it and left it. **Vignettes V1–V8 checked against their
changed sections** per the authoring conventions; all still illustrate what their
sections now say. Headings: zero depth, order, duplication or spacing defects
across the five documents; fences paired; no trailing whitespace, CRLF, doubled
words, or task markers anywhere in the set.

**The numbers, as the conventions require**: em-dashes 583 in the design, 400 in
the wire format, 31 / 37 / 44 in the three requirements documents — 1,095 across
the five-document set. References resolve at zero across the five documents and
seven vector files; the 25-check harness passes against the regenerated vectors.
`Robot/review-plan.md` opens the second cycle.

### 2026-09-02 (cycle 2, pass 0.1: external-claims audit applied)
The first pass of the second review cycle checked externally-checkable claims
against their authorities: 48 rows, 38 confirmed. Six corrections applied, each
verified against the source before the edit: RFC 6177 does not fix a /64
"assignment floor" — the /64 is the smallest assignment it contemplates; BGP
NLRI trailing bits are irrelevant per RFC 4271, so zero-padding is now owned as
this document's canonical-form rule (and the cite corrected to §4.3); the
heartbeat comment's prior art now carries RFC 4787's real figures — two-minute
minimum, five recommended — and MQTT 5.0's "application-specific, typically a
few minutes", which strengthens the argument they support; PQXDH first-message
exposure without a one-time key requires the peer's identity key, signed prekey
and PQ prekey together, not the signed prekey alone; iOS current-BSSID reads
need entitlement plus further conditions; and V7's "no statute, no judge"
became "almost nowhere", conceding the exceptions. The unverifiable block
required no edits — every row was already registered in §20.1, as the reviewer
acknowledged. One item queued for the author: A18's 24-month ageing figure,
which the register already advises recasting and the two-year retention tier
leans on.

Applying the RFC 4271 correction exposed that the reference checker had been
reading RFC-prefixed section cites as internal references — six such cites
existed, passing by collision with the set's own headings. All six were checked
against their RFCs: five held; wire §2.2's COSE_KeySet cite said RFC 9052 §9
where the definition lives in §7, now fixed. The checker classifies
RFC-prefixed cites as external and the internal count stands at zero.

A18's 24-month figure is kept by ruling: the two-year tier needs a threshold,
two years is a common one, and it is close enough on several axes, of which
face ageing is only one. §20.1's advisory row records the ruling; the
assumption row and the body text stand unchanged.

### 2026-09-02 (cycle 2, pass 0.1, second run applied)
A lower-effort re-run of the external-claims audit: ~50 rows, 31 confirmed —
among them the first run's own corrections. Four edits: `rhtn-roles` is RFC 9110
list syntax whose elements are HTTP `token`s, not itself a token (comma is not a
`tchar`); the deterministic-CBOR sentence now says float *representation* and
notes no schema in the wire format admits a float; the Dunbar anchor corrected
from "around 200" to the canonical ~150 with the original interval of roughly
100–230, inside which 221 still sits; and the browser-wasm transport gap is
dated "today". Five partially-correct rows and all ten unverifiable rows needed
no edit — each already registered in §20.1, already hedged, already ruled, or
the design's own labelled inference.

### 2026-09-02 (cycle 2, pass 0.2 applied: 25 internal contradictions, 25 held)
The high-effort contradiction pass found 25 and every one survived verification.
The blocking find was seed custody in §7.5.2: the opening sentence had
participants exchanging seeds while the construction, the KeyGrant and the
security claims all require the seed never to leave the subject — the text now
hands over a per-ceremony derived key at capture, discarded once the capture is
sealed. Around it, the retention window is now everywhere the subject's default
rather than a hard stop; seeds die with the device only absent a restored
backup; "never meet again" became "never releases the key again"; and the grant
defers to the subject's choice of eligible capture. Redesign residue in §10.1's
pruning argument (chain traversal) was rebuilt on bundle qualification. Mode
conflations were split: gateway as proxy-or-broker in both documents naming it,
the transport-coverage claim now names which transport, and §24's step 10 no
longer directs HTTP/3 onto a connection §3 of the resource document forbids it
on. The 110 rule reads in the right direction again; the §4 scope bullet
concedes the interaction protocol and owner-movement rule are specified; the
integration-decision count is five at all four sites with demultiplexing
restored to §22.2; the witness reads-row is None, which makes ten-of-eleven
arithmetically true in both documents; the forwarding TTL left §21; the
twenty-six became the thirty-one it always was; nine leaves became seven and
four proof hashes three; and §20.1's section column was remapped wholesale,
merging two duplicate rows found in the process. Sweeps for every replaced
phrase return nothing.

The author confirmed both 0.2 reconstructions. His custody rationale is now in
§7.5.2: the capture-time handover adds no new trust class — a compliant holder
is already trusted to discard a released key after answering as a verifier, and
discarding the sealing key is the same obligation at an earlier moment. The
non-compliance list names capture-time key retention among the defeats.

### 2026-09-02 (cycle 2, pass 0.2, second run: five of six applied)
The medium-effort contradiction run found six, disjoint from the high-effort
twenty-five. Five held: §16.1's "deterministic sample" residue (contradicting
its own section), §7.1.1's missing formation exception on the witness minimum,
wire §3.2's two nonce residues with the clock-decline defence re-cited to the
light client's §1.0.1, and the resource document's "No HTTP" cell now explaining
that the first leg embeds a serialized HTTP/1.1 message without speaking HTTP on
the wire. The sixth — predicate recalculation — was rejected: tuning is the
two-moments rule's first moment and "ceasing to match" happens at either moment,
so the cited texts cohere; the genuinely open point, whether evidence drift
between moments should re-evaluate the table, is queued as a question rather
than applied as a fix.

The staleness question is ruled: predicates now re-evaluate at four moments —
configuration, horizon membership, evidence arrival about a standing member
(the span is narrow enough that events are affordable), and a periodic
whole-table pass for time-dependent values, never continuously and never on
request. A table update is enforced at issuance and reconnection; where the
node itself holds the session the drop on authorisation change is mandatory,
and a non-intermediated connection cannot be relied on to drop. The periodic
cadence becomes the twelfth unset parameter.

### 2026-09-02 (cycle 2, pass 0.3: the census holds)
The unsupported-claims pass independently counted 123 total and 79 load-bearing
— the same figures §20's own census sentence has carried, so the register's
arithmetic is externally reproduced. Of the 79, all but eight were already
disclosed by the A-register, §21's chosen-basis table, the wire ceilings'
no-capacity-study note, or existing §20.1 rows; seven rows now cover those
eight (the two SSO claims share one). And one adjective became arithmetic: the
greased-id collision probability is k/2^64 per draw, derived from the id
space wire §8.1.1 already states.

Two elevation rulings closed 0.3: trust-emanates is a primordial design
parameter rather than an assumption — §1.2.1 says so and its register row is
withdrawn — and the compromised-resource confinement claim is load-bearing,
now A32. The duals are seven; §20.2 lists thirty-two.

### 2026-09-02 (cycle 2, pass 0.4: parameter census; two counts repaired)
The parameter census classified every operating value and found two numeric
conflicts, both stale counts: the Preface still said eleven unset parameters
(a sweep that caught "Eleven unset" missed "Eleven parameters remain unset")
and wire §12's minimised-presentation figure still said ~288 B — the nine-leaf
arithmetic — where seven 32-byte digests are 224 B. Both fixed; the ~112 B
salts figure beside it was already right. The census's derived/borrowed calls
match the documents' own basis labels throughout.

§21.1 now carries its scope rule: it lists the design's parameters, and
component-local policy bounds live with their mechanisms in the component
documents — two tables in sync is one more surface for error. The general
placement doctrine is recorded in the authoring conventions.

### 2026-09-02 (cycle 2, pass 0.5: seven role-invariant anchors)
The fragile-rules pass flagged eighteen identifier-bound rules; eleven already
carried their invariant in role form beside the encoding — the
role-rule-then-"Present encoding" idiom doing its job — and seven anchors were
added: departure's unblockability cites §6.2's formed-bilaterally-ended-
unilaterally rule (restoring the invariant that left with the veto), the
archive gains its transaction-hood criterion above the type list, the rootward
memo states it summarises only slots its chain governs, a verifier reports an
identity judgment only where it ran the supporting comparison, a client must be
able to authenticate every failover peer before it needs one, possession of a
signed catalog entry is not authority to install it, and a referral moves the
requester strictly forward.

### 2026-09-02 (cycle 2, pass 0.6.1 phase 1: adoption implementation attempt)
The clean-room implementation attempt surfaced two real conflicts and two
under-determinations, all fixed in the wire format: the Recovery-presence prose
no longer says every rotation carries the block — design §9's plain rotation
carries nothing and is an ordinary adoption on the wire; template_version is
bounded to uint16 per design §8.1; an adoption's locator now explicitly opens
its series at counter 0, the reissue rule, with all three adoption fixtures
regenerated from arbitrary counters and a harness check added; and both
verifier-response arrays sort ascending by verifier keyhash, the witness rule.
One genuine design gap is queued: recovery reuses the verifier-response schema
but defines no selector or querier role for its queries. KeyMaterial-omission
policy confirmed deliberately local.

Phase 2 of the implementation attempt returned no vector defects and one
implementation bug — the nested-extension fixture caught its intended failure
class in an independent implementation. Must-accept row D5 is retired: its
any-order rule predated the ascending-verifier-keyhash ordering and cited a
non-canonicalisation sentence the wire format never contained; T28 carries the
malformed case. The complete positive Recovery vector remains the queued
canonical-bar item, blocked on the recovery selector ruling.

### 2026-09-02 (the recovery selector ruled; canonical bar 4's Recovery half lands)
The open selector question resolved from §9.1's own text: recovery runs §7.3
in reverse, so the counterparty is its own querier — the subject stands in
front of the party answering — the meeting opens as a ceremony whose
pre-commitment binds the query, and selection_basis is 0, the only value a
Recovery block admits, since a recovery verifier is by definition a prior
counterparty. §9.1 states the mechanics; wire §4.1 carries both rules. The
complete positive recovery adoption now sits in the vectors: query, consent
over the raw query_id, hybrid verifier response, successor proof, and the
adoption presenting the old chain head — every signature real and verified by
the harness, which grows to 28 checks, all passing.

### 2026-09-02 (all open work closed; the canonical bar is met)
Under the new practice — close all open work before further review passes —
the vector suite's remaining thirteen bar items were built: the normal
presence record with sixteen witnesses and its 36-entry envelope, the
curated-bundle fixture, both verification authentication forms, the
disclosure negative family, the machine-readable corpus (167 entries under
stable ids in four classes with layered expects), the full boundary sweep,
every signed context with wrong-signer analogues and cross-context
substitution, the unsigned message families with session traces, the
finalization must-accepts, the enumeration matrix, six newly-covered
optionals, and the schema-shape matrix. The harness runs 64 checks, all
passing, every signature real. Nothing is open for the author; promotion
awaits only an independent implementation reproducing the suite.

### 2026-09-02 (cycle 2, 0.6 resolution target, phase 1)
The clean-room resolution attempt added no wire fields and validated the
no-arrival-equation design end to end. Two wire nits fixed: AnchorEntry's
endpoint list was unbounded in the schema against §1's ceiling of 8, and the
endpoint lists of Referral and AnchorEntry now state the
publisher's-preference semantic the other lists already carried. Referral
field 4's comment now says a requester CAN authenticate the next hop — an
option, not a requirement — naming the disclose-nothing rule and the
terminal-ServingInfra distinction, after the reviewer's implementation read
it as mandatory and would have stranded resolutions the design completes.
Everything else was deliberately local policy, correctly exercised.

Phase 2 of the resolution target: zero implementation bugs, zero vector
defects; the one classified ambiguity exposed a fixture-expectation defect —
the wrong-signer anchor's unconditional reject omitted the pinned-key
precondition that separates infra §4.1's two permitted ingestion models, and
now states it. The reviewer's coverage observation closed the last referral
gap: advances-zero as bytes, overshoot as a context fixture. 65 checks pass.

### 2026-09-02 (cycle 2, 0.6 presence-validation target, phase 1)
The clean-room presence validator reproduced the redesign wholesale and
surfaced four under-determinations, all now ruled: retired field numbers are
tombstones a decoder rejects (the type-6 and Scope-tag-3 rule, T29);
verifier-response ordering breaks ties by ascending subject keyhash — D15's
legal tie was the most wire-significant gap, two encoders producing two txids
for one response set; a proximity channel kind may repeat, a retried channel
being two measurements; and a corroboration must name a field-4 witness,
since its authority is its maker's envelope signature over the root. Fields
4/5 are now marked subtype-conditional and zero responses omit key 5 — one
logical record, one encoding — resolving the reviewer's reading in favour of
the shipped fixture. The curated bundle explicitly has no protocol ceiling;
truncation is a visible local act.

Phase 2 of the presence target ran under version skew (current vectors,
pre-ruling spec) and returned the morning's three rulings as its three
divergences — independent confirmation each was needed. One real repair
resulted: R14's tombstone note still carried the superseded
preserve-as-extension reading and now records the tombstone rule. All six
presentations, the formula and boundary arithmetic, and fourteen semantic
rows traced to agreement; the one implementation bug (silently skipped
missing-key embedded signatures) is the reviewer's, and demonstrates the
unverifiable(key) dimension.

### 2026-09-02 (cycle 2, 0.6 attach target, phase 1)
The attach attempt surfaced the two refusal-scope gaps and both are ruled the
way the reviewer assumed: a primary's application-close-1 refusal does not
open sibling failover — the triggers are unreachability and the three-miss
rule, and a refusal is an answer, not an outage — while a failover sibling's
refusal forecloses that sibling alone. Traces TR9 and TR10 carry both.
AttachAck's queue count is stated U64 RANGE. And the heartbeat comment now
says gap-detection is for information, never liveness — an
exact-expected-counter implementation ignores every beat after one loss and
fails over against a live server, which is TR7 read backwards.

### 2026-09-02 (the over-strictness stress family)
By direction, every case where a clean-room implementer read the
specification more strictly than written is now a named stress family in the
suite: ten entries accumulated from the 0.6 rounds, six newly built — dial
the unkeyed referral hop, accept the gapped heartbeat, accept the server's
mode, accept the early ServingInfra, reject the empty-array response
spelling while accepting the absent one, and accept the retried channel kind
— four already present and now listed. Where the prose could not prevent the
misreading, the corpus now catches it. Fourteen traces; 67 checks, all
passing.

Phase 2 of the attach target: the heartbeat-gap trace caught the reviewer's
exact-expected-counter bug — the over-strictness family earning its keep on
its first outing — and exposed one real vector defect: the machine-readable
TR2 required only the defer branch where the specification permits reject or
defer. TR2 now requires never-process-as-early-data with a one_of carrying
both conforming branches, a convention the corpus format now documents.

### 2026-09-02 (cycle 2, 0.6 capture/query target, phase 1)
The capture-and-query attempt surfaced the cycle's one wire-schema change:
request type 4 now carries the selector's selection_basis claim as its third
element — response field 10 sits inside the verifier's signature and no
specified request element carried it, so the specified response was
unconstructible from the specified bytes. It also settled a design/wire
contradiction by precedence — an absent capture key is unavailable, not
inconclusive, and a decryption failure reports basis 0 with the query's
template version — and pinned the capture-key derivation as HKDF-SHA-256 to
the byte, with a known-answer vector and the KeyGrant fixture carrying the
derived key. The querier field binds to the authenticated requester; the
reply is one VerifierResponse; malformed queries close the stream; a grant's
sender must be the subject, duplicates and rivals ignored. The capture-key
handover and the selected verifier's identity ride the ceremony's direct
channel, carried by no wire object.

### 2026-09-02 (capture/query re-run: pre-commitment construction ruled)
The re-run against the corrected specification confirmed every prior ruling
and found the custody fix's one unswept instance — the ceremony summary still
exchanged seeds, and now hands the derived key. The ceremony pre-commitment,
previously fixed-unique-countersigned but constructed by nobody, is now
contributory: SHA-256 of the tag and both participants' 16-byte contributions
in ascending keyhash order, so neither party can force a repeat of the value
that consents and capture keys bind to — with a known-answer vector the
harness recomputes. A grant naming a record the holder does not hold yields
unavailable, the absence posture.

Phase 2 of the capture/query target caught the suite's most instructive
defect yet: the KeyGrant chain used the record under assembly as its sealing
context — a circularity the design's own construction note forbids, and a
holder mismatch besides, since c1 held no capture of alice from any fixture
meeting. The prior alice–c1 record now exists, the derivation and grant
re-derive from its contributory pre-commitment, and the harness pins the
prior-record and current-query bindings. The end-to-end payloads moved to
their own messages.md section claiming no framing — the demultiplexing
decision stays open, and the replies caption no longer overreaches.

### 2026-09-02 (cycle 2, 0.6 catalog target, phase 1)
The catalog attempt ruled three small gaps: a first registration with no
requested scope defaults to self, the least-disclosing rule; "ascending
keyhash" is defined once at the keyhash definition as lexicographic raw
bytes; and registration failure signalling splits — malformed framing resets
the stream, well-formed failures are answered refused. The reviewer's
require-a-local-backend assumption was over-strict for brokered resources —
a registration is complete in itself, now stated, and the stress family
gains the row.

Phase 2 of the catalog target: ten of eleven vectors agree; the truncated
CatalogReply was unproducible as built — continuation present entails more
than 111 qualifying entries and exactly the first 111 returned — and is
rebuilt conformingly with 111 sorted entries and the withheld 112th's type
as the hint, harness-pinned.

### 2026-09-02 (the Rust conformance runner)
The suite gains an executable second validator: a self-contained Rust crate
with its own strict deterministic-CBOR parser, identities re-derived from
the stated seed recipe, and every signature in the corpus verified through
the Rust ecosystem's implementations — which makes the post-quantum half
cross-implementation for the first time, dilithium-py's signatures verifying
under RustCrypto's ml-dsa: envelopes, embedded consents and responses, the
recovery block's hybrid evidence, presentations and standalone records. 143
byte-entries pass with zero failures, and its first full run produced a real
finding — the unknown-extension boundary fixtures measured payload where the
ceiling bounds the encoded slice, now corrected in both fixtures and checked
by both harnesses.

### 2026-09-02 (cycle 2, 0.6 resource-authorization target, phase 1)

The seventh implementation-attempt round — authorize a user to a hosted
resource — produced no wire changes and eleven unspecified items, six of
which were rulings the documents now state. Every type-6 request is in
§9.2's 0-RTT-forbidden class: the gateway deliberately does not interpret
application semantics, so it cannot certify any method effect-free, and §9.2
now says a resource request is never read-only whatever its method. A backend
failing mid-handoff is answered `resource unavailable` and the gateway never
retries on its own — it cannot know whether the application committed an
effect. Role names in the resource-facing header form a set, and a principal
holds at most 64 application roles per resource, enforced where it can be
seen: a node refuses to materialise an over-wide row, so the operator hears
at configuration time and no request-time failure code exists. The session
dropped on an authorisation change is the resource-facing one — the node-held
identifier the resource correlates by — never the caller's transport session:
retire the identifier, reset that pair's in-flight requests, and the next
request arrives under a new identifier, which is how a resource observes the
change on a path that has no teardown message. A local-socket backend's
`Host` authority is whatever the installation recorded. The remaining five
items are local by design and recorded as such.

The round also exposed an interop surface: the pairwise principal is
computed by whichever node currently hosts the resource, so implementations
must agree byte-for-byte or a provider migration renames every user a
resource knows. A known-answer vector now pins the construction, recomputed
by the Python harness (79 checks). A reference sweep fixed two pre-existing
bare cross-document citations in wire §5.5 and light §2.

### 2026-09-02 (cycle 2, 0.6 resource-authorization target, phase 2)

The clean-room implementation and the draft vectors agree everywhere they
meet: zero implementation bugs, zero spec ambiguities, zero vector defects.
What phase 2 actually delivered is a coverage map — the resource-path vectors
exercise wire shapes and shared framing, not the authorization mechanism —
and three traces now close the compatibility-relevant part of that gap. TR15
pins the 0-RTT rule for opaque requests (never process, whatever the method).
TR16 pins termination granularity — the very reading the implementation
attempt got wrong before the spec ruled: a role-row change retires the
resource-facing identifier while the transport session and other resources'
hosted sessions survive. TR17 pins the evaluation order that keeps
member-specific refusal statuses inside the membership. The remaining
uncovered behaviours are operator conduct the suite's own remit excludes.
Seventeen traces; both harnesses green.

### 2026-09-02 (cycle 2, 0.6 refused-request target, phase 1)

The eighth implementation attempt required no specification changes at all —
the first round of the family to close without one. The evaluation order,
the step-0 malformed-body boundary, the unassigned reset code, the open
timing question and the user-presentation duty were each cited from the
text rather than rediscovered, including two rules written only six rounds
ago. One trace was added to pin the boundary that almost invites
generalisation: a malformed type-6 body is answered with status 3 while a
malformed type-4 body closes the stream, because each type answers defects
in its own terms. Eighteen traces; both harnesses green.

### 2026-09-02 (cycle 2, 0.6 refused-request target, phase 2)

The vectors and the implementation agree everywhere they meet, including
the two traces written from this target's own phase-1 rulings — satisfied
on first contact rather than after correction. No changes anywhere; the
target closes at zero divergences across both phases.

### 2026-09-02 (cycle 2, 0.6 topology-flood target, phase 1)

The ninth implementation attempt surfaced the most substantive gap of the
family: an EndpointRecord carries one seqno, but a sequence series is per
patron relationship, so a node bound under two patrons had no specified
carrier. The record is now one per relationship line, each with that line's
own seqno — forced three ways: a single record's series is unprovable in the
other relationship's subnet, a shared counter across subnets would disclose
exactly the cross-subnet activity P36 conceals, and each patron must be able
to refer. No relationship field was added; the reviewer rightly declined to
invent one. Alongside it: a malformed control frame of a known type is
discarded whole and the session survives, generalising the heartbeat and
SiblingUpdate instances; the series-reissue subject joins §10.1's
enumeration from that section's own principle; an unproved-series endpoint
record is neither stored nor forwarded until its series proves current, the
same posture as a transaction with a missing signer key; equal seqno with
different signed contents — extensions included — is malformed, aligning the
one statement that said "endpoints" to the three that said "contents"; and
the changed-endpoint rule now says "list", with reordering explicitly a
change. A fixture pins the port whose default spelling is omission
(B-port-7431-explicit), and TR19 pins the peering-cycle duplicate dying
against the store. Nineteen traces; both harnesses green, the Rust runner
at 146.

### 2026-09-02 (per-line endpoint records confirmed)

The author confirmed the per-relationship-line ruling and supplied its
intent: record and locator partitioning between different subtrees is the
anticipated use. §7.6 now says so.

### 2026-09-02 (cycle 2, 0.6 topology-flood target, phase 2)

Phase 2 caught its first planted quarry: the explicit-default-port fixture,
added one round earlier from reading the phase-1 type sketch, hand-traced to
exactly the implementation miss it anticipated. It also exposed the one gap
the equal-seqno rule had left: what happens to the record installed first.
Ruled — malformed names the pair, not the later arrival. A holder that kept
the earlier record would let arrival order split the network's view, and a
thief whose forgery lands first would pin every reader it reached; on
discovering the conflict a holder retains neither as current and re-resolves,
and the line is repaired only by its subject. Stated at §10.1, echoed for
the locator route at §7.7.3, and aligned across the V6, V11 and V12 vector
rows — V11 also gaining its legitimate other half, where chains proving both
series make holding both the correct end state under per-line records.

### 2026-09-02 (cycle 2, 0.6 rootward-memo target, phase 1)

The last implementation attempt of the plan met a task prompt that had gone
stale around it and followed the documents instead — the memo table keyed
by (patron, slot) with the patron's timestamp, the cycle test as field-1
identity, and confirmation against the detector's own records with no
fetch. Every claim verified; all seven unspecified items are local, one of
them answered by the text outright. Reading the attempt's decode path
exposed the one rule §8.0 stated without a consequence: a declared frame
length above 65,536 now ends the session — it is a violation of the
framing layer itself, beneath the discard-whole rule for malformed bodies,
because skipping it would stream an attacker-declared volume through the
ceiling that exists to bound the buffer. Two traces land: an unconfirmed
cycle hint severs nothing, and the documented containment trap — the
receiver's path prefixes every legitimate memo — joins the over-strictness
family. Design §5.2 notes aws-lc-rs as the production-oriented native
route that still leaves the browser gap standing. Twenty-one traces; both
harnesses green.

### 2026-09-02 (cycle 2, 0.6 rootward-memo target, phase 2 — the family closes)

The final phase 2 traced five memo vectors to zero divergences and
self-caught the oversize-frame bug the previous ingest had ruled on —
classifying it with the same layering argument the ruling used, from the
pre-ruling text alone. The confirmed-cycle complement of the hint-rejection
trace and the wrong-subnet drop join the trace set, closing the named
coverage gap. The implementation-attempt family ends at ten targets and
twenty rounds: spec ambiguities at zero for the last six targets, two
planted fixtures each catching their quarry one round after planting, and
the integration suite grown to twenty-three traces.

### 2026-09-02 (cycle 2, pass 0.7: LINDDUN privacy analysis)

An external LINDDUN pass audited thirty-nine data flows and twenty-nine
stored artifacts against the privacy registers and confirmed the registers'
posture: every accepted cost matched §19.7, every severity matched the
register's own, and the reviewer's closing judgment was that the C-register
already catches most of the dangerous joins. Four compositional gaps
survived verification and are now registered: a patron clustering a
fanout's tightly timed pairwise deliveries into latent group membership
(C20); the capability set as an implementation fingerprint that greasing
does not conceal (C21); the materialised role table as a pre-joined
classification matrix awaiting compromise or compulsion (C22); and a
filtered catalog query as a dated statement of service-class interest,
already blunted by the reference client's sweep-and-cache rule (P38).

### 2026-09-03 (cycle 2, adversarial pass 0.9.1: the patron)

Three patron findings ruled. Exit laundering: a nonconforming serving
patron can drop a subordinate's departure while propagating its own
disavowal — accepted at its true size, which verification shrank twice:
both objects flood the same neighbourhood (in-horizon, not per-subnet),
and no cross-tree channel carries a disavowal, so the adoption scan reads
an archive holding the departure and never the adverse half. The falsified
story reaches only the audience the victim left. Manufactured currency
fallbacks: the stapling rarity that justifies the accepted fallback leak is
the patron's own signature to withhold — repaired by what the schema
already permitted: attestations name no querier, so any fresh one is a
reusable staple and the subject may fetch its own, making the inversion
visible to its victim. Selective resolution censorship: "gains nothing by
lying" softened to the impersonation claim it always was; denial is what
lying retains, and the eclipse section now prices its cheap selective form.

### 2026-09-03 (cycle 2, adversarial pass 0.9.2: the witness)

Three witness findings ruled. The location corroboration's radius is now
witness-relative by statement — the witness bounds the participant within
R km of itself, no coordinate is carried, and interpretation is by
recognition of the witness, which turns a falsely tight radius into
ordinary attested-evidence lying weighed like any other witness bit. The
availability filter in witness nomination is acknowledged as not
uptime-neutral — it cannot be, since an absent witness cannot serve — with
the Potemkin item pricing the cheaper path a continuously online placement
buys, bounded as before by branch spread. And the witness floor now means
what its rationale always said: a normal record needs at least one witness
affirmatively attesting that the protocol ran and both parties were
responsive; witnesses attesting nothing remain representable but do not
corroborate, and the uncorroborated meeting stays where it is visible —
the formation subtype. T31 and a unit fixture pin the floor; both
harnesses enforce it.

### 2026-09-03 (cycle 2, adversarial pass 0.8.3: the commercial operator)

Four findings ruled under the rewritten defensive prompt. The deep one:
the min-cut argument proved a per-identity bound while the design's
at-scale claims rest on the aggregate — setwise conservation is now a
normative property of the reference metric, one shared-capacity
computation for any set of identities behind a cut, which is the Advogato
shape the text already cited. The per-target economics of §16.3.1 are
restated as coverage: one acquired edge helps every observer whose
horizon contains it. The selection basis is renumbered tier-aligned —
met, in-horizon, reachable, discretionary — because met and
merely-in-horizon are different security facts (A23) and the old
vocabulary folded them into one value, laundering cheap structural
placement into the look of acquaintance in the permanent record. And the
candidate pool now counts identities, never people: one person may hold
several, and the record claims no human independence the protocol cannot
prove. Fixtures and both harnesses follow the renumbering.

### 2026-09-03 (cycle 2, adversarial pass 0.8.4: the compelled provider)

Four findings ruled, all corrections of credited protections rather than
new attack surface. The impersonation residual is no longer called
self-burning: one shared key spans the operator's devices, concurrent
presence is the ordinary multi-device condition, and the direct-path
horizon includes strangers, so detection is contingent on acquaintance the
architecture does not guarantee — what stands is per-target, prospective,
and no bulk collection. The deniability acceptance now claims relief only
on the transfer ledger: fabrication capability discounts what a collected
record can prove onward, never what the collector knows. The sibling
fallback's honesty-axis independence is scoped to adversaries who can only
attack availability; one control domain hosting both signers is
correlated. And queue deletion is stated for what it is: the real
retention bound above the hypervisor, hygiene below it. A commissioned
analysis records where the operator-key-on-node assumption is
load-bearing: session authentication, the ten-hour currency cadence, and
the sibling issuance ladder.

### 2026-09-03 (cycle 2, adversarial: the malicious counterparty)

Five findings ruled, and the ceremony evidence layer gained the two legs
its defences had assumed. Verifier responses now reach the subject as well
as the querier — over the association the capture-key grant already
creates — so the finalization veto the substitution analysis always cited
is real: suppression requires both participants and buys only a visibly
thin record. The subject's consent is no longer bearer paper: the query
names its one addressed verifier in a new field inside the query_id hash,
so consent confines the profile to the verifier the selector actually
named, one consent per verifier; the fixtures' three queries now share one
profile, as the pinning rule always intended, and differ by addressee.
Three smaller rulings: a client refuses to sign a record attributing to it
a witness it did not nominate; a fishing proposal is a bundle augmentation
under the same curation, stopping at an adjustable count of responsive
candidates, with post-bundle cherry-picking visible to the party being
fished; and the verifier-signed selection basis is carriage, not
endorsement, with a locally refutable false "met" answered as unavailable.

### 2026-09-03 (cycle 2, adversarial: the stolen device — the pass completes)

Four findings ruled, closing the sixth and final adversary role. §18.3's
two surviving bounds are restated at their true width: unforgeability
prevents manufacture, not suppression — a colluding pair finalises thin,
and thinness is the evaluator's signal — and the standing cap belongs to
the reference metric, which §16.4's pluggability does not impose on every
evaluator. The stolen-device entry now carries the operator case: one
seed means a stolen phone is stolen infrastructure authority, priced with
its asymmetry — thief currency dies in hours, a thief disavowal is
durable. And the currency lifetime claim is cut to its real width: expiry
bounds what a stolen credential can spend; the new supersession rule —
fail-open is for ignorance, never knowledge — is what retires it from
use, at every party that has verified the succession.

### 2026-09-03 (pre-0.9 coherence and consistency pass)

A directed sweep before the organisation pass. Three half-swept claims
completed: §17.3's per-target preamble joins §16.3.1's coverage bound, the
staple-window derivation names trust-bearing capability, and wire §13's
vector item records the Rust runner's cross-implementation reproduction
with its same-author limit and the unchanged independent-party bar. The
infra Open item on gateway pre-evaluation aligned with P24's reduction.
And the reference checker runs at zero for the first time — the nine
standing manually-inspected flags were exemptions by another name, and
every cross-document reference now names its document.

### 2026-09-04 (cycle 2, pass 0.9: organisation)

The organisation pass assessed the post-migration structure. Everything
cheap was applied: wire §1's four topics each have their own section
(domain separation had annexed deterministic encoding, the global bounds
and content addressing for 165 lines), the light client's ceremony
chapter is conventionally numbered with witnessing as a sibling of
nomination, thirty-eight headings that carried decisions without numbers
now carry both, Appendix A's subsections are citable as A.1-A.4, the Open
chapters are numbered, the force-of-requirements paragraph lives once in
Appendix A.2, and both documents gained front-matter bridges — a notation
index at the wire's head, a four-term pointer at the thesis. The large
chapters were renamed in place rather than split: addressing carries its
reachability freight in its title, propagation surfaces the rootward
memo, and the role chapters name their lifecycle halves. A second
migration was declined for v1; the standing rulings from the first —
three-way ceremony split, §22 as the open-item index — were reported to
the reviewer's satisfaction rather than reopened.

### 2026-09-04 (Stage 1 formal models)

Built the review plan's Stage-1 formal models under `models/`, with all
dependencies installed user-locally. Eight artifacts, all passing: a
stdlib-only trust-metric simulation reproducing §16.2's four analytical
claims including the setwise-conservation saturation the adversarial cycle
made normative; three TLA+/TLC models (partition-and-merge convergence
restated for a no-shared-state system, the currency escalation ladder's
freedom from deadlock, and concurrent-adoption cycle resolution); and four
Tamarin theories (session attach, currency expiry, recovery, and the
presence ceremony), thirteen lemmas verified, with the physical co-presence
and face-recognition axioms stated explicitly rather than hidden. The
recovery model falsified twice before converging, each counterexample
confirming a real design rule — wire §5.3's distinctness requirement is
load-bearing for recovery, and §9.1's "neither factor alone" is the honest
provable form. Everything is textbook-commented and re-runnable via
`models/run-all.sh`.

### 2026-09-04 (cross-family review of the trust-metric simulation)

An external review of the flow-metric simulation validated its max-flow
core (700 random graphs, zero discrepancies) and found six modelling
defects, all verified against the design and corrected. The substantive
one: the simulation had collapsed three relations the design keeps
separate -- scope (adoption+sibling edges, the horizon), trust-capacity
(adoption+peering edges, the flow), and visibility (a peering edge seen
only inside both peers' horizons). The conflation understated an acquired
edge's coverage several-fold and let the setwise-conservation experiment
count identities the observer could not see. The rewrite separates the
three, makes the conservation experiment observer-visible, enumerates
every edge placement rather than sampling, and fixes the exact divergence
boundary (fλ=1 diverges). It also surfaced an open design question, now
flagged for a ruling: when scarce trust capacity must be allocated among
symmetric principals the max-flow value is unique but the allocation is
not, and whether the reference metric fixes a deterministic tie-break or
leaves it to policy is undecided.

### 2026-09-04 (scarce-capacity tie-break ruled)

The open allocation question from the trust-metric review is resolved. The
reference metric breaks a scarce-capacity tie deterministically -- the
earlier-considered candidate wins -- as reference policy rather than a
protocol invariant: per-observer trust means no node consumes another's
computation, so differing resolutions both conform and a node need only be
stable to itself. Stated at s16.4, mirroring the line already drawn there
for lambda, and tested in the simulation.

### 2026-09-04 (scarce-capacity allocation re-ruled; simulation corrected)

A second cross-family review of the trust-metric simulation found the
allocation rule unimplemented, and the finding prompted a better rule. The
reference metric now allocates scarce capacity in two limbs: the shorter
path dominates, because a longer path is less trustworthy by nature, and
consideration order breaks true ties only. §16.4 carries both, still as
reference policy rather than protocol invariant, with an implementation
note the review earned: a single multi-sink max-flow satisfies the first
limb for free and misses the second silently, deciding equal-length ties by
the order edges sit in the implementation's own adjacency rather than by
candidate order. The simulation ranks candidates explicitly and tests both
limbs. The same review rebuilt the edge-coverage experiment around genuine
two-ended peering — an earlier one-ended attacker had made "visible implies
influenced" true by construction — and separated the unit-demand admission
count from the general-demand conservation statement the design makes.

### 2026-09-04 (allocation: available flow ranks first)

The scarce-capacity allocation rule gains its primary pass. Available flow
ranks before path length, because the flow is what the metric measures and
preferring a nearer candidate over a better-supported one would substitute
the tie-break for the measurement; path length separates candidates the
flow ranks equally, and consideration order separates what path length also
ranks equally. §16.4 notes why the ordering is easy to miss: behind a
single saturated cut every candidate carries the same flow, so the first
pass decides nothing there, and the section is about pluggable policy in
general rather than the chokepoint case alone.

### 2026-09-04 (third simulation review: destination vs relay capacity)

A third cross-family review found the simulation charging every candidate
its own relay capacity when receiving: joint drains hung off the far side
of each candidate's node-capacity edge while individual standing stopped at
the near side, so a lone candidate with no contention could be told it
could use half its own standing. Node capacity models what a node may
relay, and a candidate under evaluation is the destination, so both joint
routines now drain from the near side and the two definitions agree. The
edge-coverage experiment lost its unjustified restriction to nodes with
subordinates — infra status does not depend on downline — and its
conclusion is downgraded from confirming the coverage economics to
demonstrating, in one toy topology, that amortisation exists. The report's
allocation summary, which still described two passes after the third was
added, now matches the rule.

### 2026-09-04 (the reference flow-graph construction rule)

The open question of what becomes an edge in the reference trust graph is
answered, in a new §16.2.1. Adoption and sibling edges are the routing and
authority hierarchy. Proof-of-presence and peering edges form an
acquaintance graph orthogonal to it: neither confers subnet scope or
resource access, and to any party other than the two an edge joins, both
carry trust by the same rules — a peering edge is an acquaintance edge from
outside the subnet, not a lesser kind of one, and what a peer uniquely holds
is an encrypted backup, durability rather than readable content. Archives
are neither: they are for the node's own reference and for adoption-time
review, where a prospective patron counts only transactions whose other
participant it already recognises and weights each by the flow its own graph
can push to that participant. Recognition decides legibility and the metric
decides worth, so a presented history can never enlarge the cut it is
presented across.

### 2026-09-04 (the trust landscape: the horizon is the origin)

The peering-capacity question is answered by a geometry rather than a
number. Hierarchical edges are unthrottled out to the two-edge horizon and
the metric throttles only beyond it, so a node's whole horizon sits at the
origin together and distance counts edges outward from there — a node's own
proof-of-presence counterparty and its patron's sibling's counterparty are
both at distance one. Beyond the horizon trust flows equally over the
hierarchical and the proof-of-presence/peering graphs: the metric
distinguishes inside from beyond, never edge kind from edge kind, and a
peer's special status exists only toward the node it peers with. Three
things followed: the assumption register's peering row lost the mitigation
it cited, the peering-capacity ratio dissolved as a parameter with no
ratio left to set, and the unset-parameter count fell to eleven in both
places that state it. What remains open is recorded in §16.3 rather than
inferred — with capacity no longer pricing the difference between a
technical favour and a physical meeting, nothing currently does.

### 2026-09-04 (landscape corrected: the horizon is distance 1)

The trust landscape is corrected before it could propagate: only the user
sits at the origin, the whole horizon is one step out, and anyone the user
met themselves is at that same step whether or not the horizon contains
them. A node outside the horizon and one edge from a horizon member or from
one of the user's own counterparties is at distance 2, and so on. The
correction supersedes its own first example — a patron's sibling's
counterparty is at distance 2, not 1 — and forces a distinction the
simulation now keeps explicitly: inside-or-beyond the horizon decides
whether a node is throttled, while distance decides how much.

### 2026-09-04 (why the horizon is both a boundary and a distance)

The last open question from the landscape exchange is answered, and the
answer explains the shape rather than merely settling it. Two mechanisms
answer two questions: the hierarchy governs resources, routing and some
countersigning, and the trust horizon is the reach of that effect, so
membership is a privileged position for hierarchy-specific operations. The
flow metric answers generic trust — whether a user exists, is a real
person, is who they claim — and for that question every horizon member sits
at distance one, no nearer than someone met in person. A node inside the
horizon is therefore unthrottled not because the metric rates it highly but
because the metric is not what is being asked there; past the horizon the
generic question is the only one left, which is why throttling begins
exactly at that boundary.

### 2026-09-04 (consistency sweep against the trust landscape)

A directed sweep of the documents and all eight models against the
landscape decision. One document conflict: the trust consequences of
disavowal were explained by a peering capacity the landscape had retired,
and the conclusion now rests on the boundary instead — losing a patron edge
drops a node out of the horizons it held through that patron, from the
unrationed side to the rationed one. One reconciliation: equal landscape
distance is not interchangeability, since attestation is a separate axis,
and a met counterparty and an unmet horizon member remain different
security facts wherever attestation is what is asked. Among the models, the
conservation experiment was found testing the wrong region — its fake
identities sat inside the horizon, where the metric does not ration — and
is rebuilt around identities reached only by peering from a horizon member:
visible, beyond the horizon, behind one cut, which is the setting the
conservation sentence is about.
