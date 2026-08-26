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


- **2026-08-12 (morning)** — Network layer: topology, cryptography tiering,
  addressing and resolution, control/data plane split, trust model, security
  analysis. Frozen separately as
  `network-design-checkpoint-2026-08-12.md`.
- **2026-08-12 (afternoon)** — Proof of presence specified end to end (§7.1–6.7)
  and key compromise/recovery added (§7.4). Corrections made in this pass:
  latency bounds distance from *above*, not below (§7.1.6); radio-environment
  co-presence cut (§7.1.6.1); a second veto keypair reversed to a single keypair
  (§7.4.2); decay reframed as a deliberate mechanism (§12.5); retention's
  recovery role narrowed to weak ties (§7.4.1).
- **2026-08-12 (consistency pass)** — Scope, vocabulary, cryptography table,
  §11 message classes, §13.3, rejected alternatives, parameters, and open
  questions reconciled against the presence work.
- **2026-08-12 (later)** — Oracle hardening strengthened (§7.1.4): dual rate
  limits, subject-side enforcement of per-subject limits, and queries bound to a
  witnessed ceremony. Rotation reconceived as adoption-shaped (§7.4.0), split
  into attested and recovery variants, and its propagation resolved as a pattern
  rather than a new message class (§11).
- **2026-08-12 (rotation collapse)** — Rotation folded into adoption entirely
  (§7.4.0). The floor requirement and explicit haircut proposed earlier were
  removed as redundant with the flow metric. Verification reclassified as
  evidence rather than authority. Identity forking documented as an accepted
  property.
- **2026-08-12 (framing)** — Teams paradigm added to §1; membership plurality
  and key forking separated as distinct axes. Fork visibility (current-keys
  assertion) and misconduct attribution norm added to §7.4.0.2.
- **2026-08-12 (pull correction)** — Current-keys assertions corrected from push
  to pull (§7.4.0.2); they are the pull endpoint of the existing "push near,
  redirect far" pattern, not a standing broadcast. Divergence notification,
  sibling fallback, and answer TTL added.
- **2026-08-12 (routing)** — §9.6 added: self-routing addresses, repair in
  transit, infra nodes carry no payload, and the security/performance TTL split.
  Closes §16.7.
- **2026-08-13 (currency)** — §9.6.5 added: key currency treated as the PKI
  revocation problem, not a session-key one. Stapled short-lived currency
  attestations, ~10 h lifetime, and failure mode graded by stakes rather than
  uniformly soft-fail.
- **2026-08-13 (outage)** — §9.6.5.1 added: patron outage cascades downward
  through countersignatures. Handled by sibling then grandpatron issuance, with
  the rule that attestations are issued fresh rather than extended stale.
  Light-client patrons pre-delegate issuance.
- **2026-08-13 (disavowal)** — §6.2.2 adds disavowal as the patron-side
  counterpart to transfer. §9.7 covers rootless operation: roots derive currency
  from below, anchor status requires size as well as rootlessness (correcting
  §9.2), and peering gains a third justification as reachability insurance.
- **2026-08-13 (corrections)** — §9.7.3 reverses an incorrect claim: the anchor
  budget is a per-node cache limit, not a global cap, so the caching threshold is
  per-node policy and must NOT be a protocol constant — a fixed threshold would
  make roots invisible in the early network. §9.2's 60B figure reframed as a
  stress test rather than a target. §9.7.1 added: Genesis identities need no
  currency attestation because the question it answers does not arise.
- **2026-08-13 (peering optionality)** — §9.7.5 records why peering stays
  optional (infra-tier operation vs light-tier bootstrap; no ordering conflict)
  and that its three dependencies degrade gracefully. Peerless infra nodes
  flagged as a visible, policy-discountable deficiency. §6.2.5 notes cycle
  prevention as unspecified.
- **2026-08-13 (subnet formation)** — §9.8 added, closing §16.9. Bootstrap
  reframed as recurring subnet formation rather than a one-time genesis: meeting
  before adoption, formation records permanently typed as witnessless, one infra
  instance required and two recommended, and ceremony evidence documented as
  degrading by availability across three stages.
- **2026-08-13 (subnet plurality)** — Major simplification. §4.1.1 reframes
  multi-subnet membership as a power the protocol does not model rather than a
  feature it represents; the DAG claim removed from the specification. Soft-fork
  deleted as a transaction type. §16.8 dissolved rather than resolved. §9.8.6
  notes rotation is per-subnet; §9.8.7 records that deferring multiple identities
  forces correlation, and that the device is the single point of failure for a
  cross-subnet identity.
- **2026-08-13 (graph precision)** — §4.1 distinguishes the acyclic *authority*
  relation from the deliberately cyclic *connectivity* graph. Peering creates
  cycles by design (that is how it raises the cut); sibling replication and
  forwarding records also add non-parent-child edges. "The structure is a tree"
  refers to authority only.
- **2026-08-13 (client vs protocol)** — §9.8.7 records that multiple keys are a
  client concern with no protocol consequence beyond the rotation transaction,
  and that exhaustive-disclosure investigation degrades as multi-key matures.
  §9.8.7.1 added: local store is append-only and backs up with ordinary tooling,
  but the blob aggregates every key and photo, and backups must be encrypted
  separately and retention-aware or §7.1.5.1's commitment is fiction.
- **2026-08-13 (sessions & backup)** — §10.1 added, closing session establishment:
  no NAT traversal problem, QUIC, dial-out attach with sibling failover as a
  declared degraded state, store-and-forward default with content-free push
  opt-in, and the patron-as-mailbox queue policy left open. §9.8.7.1 gains the
  envelope-encryption construction, an honest costing of Shamir/SLIP-39 KEK
  splitting, and scan-on-import retention enforcement. §9.8.7 records that
  rotation chains are walkable, so recovery and unlinkability are the same
  choice seen from two sides.
- **2026-08-13 (wire format)** — Companion file `wire-format.md` drafted: CBOR
  deterministic encoding plus COSE signatures, the five transaction types then
  defined (seven as of 2026-08-17), the
  supporting attestations, session frames and QUIC binding. Fields marked [D]
  where derived from a decision here and [P] where proposed. Design doc remains
  authoritative on any disagreement.
- **2026-08-13 (transfer collapse)** — Transfer removed as a transaction type;
  it was adoption plus a drop, and dropping was never a network operation once
  §4.1.1 landed. §6.2 restructured into departure (node-initiated) and disavowal
  (patron-initiated): formed bilaterally, ended unilaterally by either party.
  Lateral/vertical shifts documented as ordinary adoption with a derivable
  trust-preserving property. Veto exemption restated as a role condition rather
  than a type exclusion, since the type it named no longer exists.
- **2026-08-14 (factual verification)** — Review pass 0.1 run against an external
  model with search. Corrections applied: **NFC is not anti-relay** (relay
  attacks are a documented class; relay resistance now rests solely on UWB, and
  the channel ranking says so); serving cell ID is Android-only; geohash cell
  sizes were wrong (~156 km at precision 3, not ~78 km); APNs tokens are stable
  but not permanent; OCSP soft-fail scoped to Firefox rather than "browsers";
  "repeated sampling does not help" corrected (min-of-many-samples does suppress
  queueing jitter — the systematic radio floor is the real obstacle); BIPA/CUBI
  differences and controller-status caveats added; Kerberos working-day rationale
  dropped as unsourced; ML-DSA ratios and presence record size given exact
  figures and a parameter set. New §14.1 lists eight unsourced assumptions.
  **One reviewer false positive:** the §7.1.6 latency bound was reported as
  backwards; it is correct as written, and the reviewer's own explanation agrees
  with it.
- **2026-08-14 (parameter inventory)** — Review pass 0.4. Five conflicts found
  and resolved; see new Appendix A.2. Presence record size aligned at ~35 KB, stale
  geohash comment fixed, infra threshold corrected to 110 in §12.6, anchor
  threshold S reframed as a guideline rather than a status boundary, and the
  `n` notation collision resolved by renaming the query count to `q`. Heartbeat
  interval and currency attestation lifetime given explicit status. One open
  parameter choice surfaced: L=2 vs L=3 for the infra threshold.
- **2026-08-14 (infra yield)** — The L=2 vs L=3 "open question" raised in the
  parameter pass was a misreading and is withdrawn. 110 (non-infra ceiling),
  1,110 (single infra node's span across three tiers) and 1,000 (asymptotic yield
  once overhead infra nodes are counted) are three distinct correct quantities;
  Appendix A.2 now derives the convergence, which lands on f^(L+1) exactly.
- **2026-08-14 (coherence)** — Review pass 0.2 returned ten contradictions, all
  genuine, no false positives. Eight were the same failure: a claim corrected in
  one place and left stale elsewhere. Fixed: patron invariant scoped to non-root
  nodes; **eleven residual references to the abolished transfer transaction**
  swept; NFC removed as an "anti-relay" fallback in the ceremony step; formation
  subtype added to the §7.2 schema to match the wire format; anchor redefined in
  §3 and the "every node replicates" claim corrected to local caching policy;
  the "any patron with subordinates is infra" claim scoped, with the consequence
  that light-client patrons give no censorship signal; §7.1.4's infra-trust claim
  rescoped to evidentiary reliability per §12.6; peering minimum marked
  recommended rather than required; agent deferral split so the schema
  requirement is in scope; §17 build step 1 no longer says "hostile networks".
- **2026-08-14 (unjustified claims)** — Review pass 0.3 returned 280 items.
  Triage: ~10 load-bearing (new Appendix A.2 register), ~15 already in §14.1, ~5 genuine
  errors, ~30 parameter choices misread as derivations, ~220 rhetorical
  intensifiers (new §0's intensifier rule editorial rule). Errors fixed: roots do **not** have
  large down-lines by definition — Genesis and small roots have neither a veto
  nor an issuance path, now flagged open; sibling "independence" scoped to the
  honesty axis; the pull-scaling claim reframed as an implementation constraint
  rather than a prediction; the ~1/1000 infra figure identified as a packing
  ratio rather than an adoption forecast; "hardest part of P2P networking"
  softened. §15 now distinguishes chosen from derived parameters.
- **2026-08-14 (vignettes)** — Convention established in a new §0: numbered
  informal passages illustrating intended human behaviour, set off as
  blockquotes, restricted to sections where behaviour is assumed, and
  cross-linked to the Appendix A.2 assumptions they dramatise. Rationale: the 0.3 review
  found narrative and derived claims typographically indistinguishable — a
  reader could not tell which sentences were computed and which were felt. Four
  exemplars added (V1–V4), two by extracting analogies already present as formal
  prose. A vignette/spec mismatch is a required conversation, not an errand;
  neither wins automatically. New review pass 0.8b checks the agreement.
- **2026-08-14 (PoP and adoption)** — A previously unstated assumption made
  explicit: §6.1.1 adds an optional proof-of-presence reference to adoption,
  expected on fresh adoptions, not enforced (no global enforcement point exists,
  §4.1.1), weighted by policy. §7.1's trust-upgrade framing widened from peering
  to any relationship a presence record attaches to, which is what makes the
  unenforced version safe. §7.1.1's ceremony summary completed with the guided
  capture and automatic verifier-query steps, both previously described only in
  their own subsections. Vignette V5 added on the ceremony as social ritual, and
  assumption A11 registered: that ordinary users tolerate the friction — the
  honest-user mirror of A1.
- **2026-08-14 (visibility corollary)** — New §1.1 states as a first-class
  principle what had been rediscovered four times: enforcement is possible
  exactly where shared state exists, so beyond that boundary visibility replaces
  enforcement. Gives implementers a diagnostic rather than a slogan, and supplies
  the missing reason why the evidence schema alone cannot be pluggable — a
  distinction the schema cannot express is one no policy can act on. Review pass
  0.5 gains a second extraction for unenforceable mandates.
- **2026-08-14 (rule fragility)** — Review pass 0.5 returned 18 rules coupled to
  identifiers rather than roles. Resolution recorded in §0: invariants are stated
  in role terms here, encodings in the wire format, and neither substitutes for
  the other. Seven given role-level statements: the veto exemption (restated a
  second time — the previous "fix" swapped one type name for two), trust-bearing
  operations now defined rather than enumerated, formation-record distinctness,
  verifier-selection seeding, currency TTL derivation, locator authentication,
  and agent-grant capacity. Several flagged rules already had role statements
  (witness cross-nomination, subject-side rate limiting) and were left alone.
- **2026-08-14 (0.5 verification)** — Verification pass on the eighteen rule
  fragility findings: 6 addressed, 2 addressed with new problems, 10 not
  addressed. All ten now carry role-level invariants, and §0 records that the
  convention was introduced in the same edit that left them — a stated discipline
  with known exceptions being worse than none. Two new problems fixed: the
  verifier-selection encoding was **grindable** (seeding from participant-chosen
  `started_at` let a participant retry until the sample favoured it), now
  requiring witness nonces revealed after participant commitment; and the
  challenge-window paragraph still said departure was vetoable three lines above
  the invariant saying it is not. Currency TTL now states what would derive it
  (detection latency, unmeasured) and marks the value chosen rather than derived.
- **2026-08-14 (implementation attempt)** — Review pass 0.6, encode/sign/verify
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
- **2026-08-14 (COSE residue)** — Self-check after the 0.6 edits found five sites
  in this document still describing signatures as `sig over H(body)` while the
  wire format had moved to COSE. Corrected, and §7.2 now marks the wire format
  authoritative on encoding so the duplicated schema is a reading aid rather than
  a competing source of truth.
- **2026-08-14 (unenforceable mandates)** — Review pass 0.5.2 found nine MUSTs
  crossing §1.1's enforcement boundary, several written after §1.1 itself. Fixed
  at the root: §0 now fixes a normative vocabulary — MUST only where a recipient
  can check the rule from evidence it holds, otherwise "the reference client" or
  "the reference policy". All nine sites reworked. Two became better mechanisms
  rather than weaker prose: attestation delivery now MUST carry the requesting
  evaluator's nonce, making unsolicited copies visibly unsolicited and locally
  rejectable; and verifier responses MUST carry the subject's countersignature
  over the query, so unauthorised disclosure cannot be laundered into the trust
  graph.
- **2026-08-14 (LINDDUN)** — Review pass 0.7, 23 analysis units. New §13.5 gives
  privacy a home for the first time; it previously existed only as local argument
  at each mechanism, which is why the central finding went unnoticed. **§13.5.1
  states the composition invariant**: privacy must be assessed under composition
  of everything an observer can obtain, never artifact by artifact. §7.1.2 had
  said exactly this for biometrics and never generalised it. New finding P2: a
  presence record leaks a sample of the subject's *prior* counterparties via the
  verifier list, and neighbourhood structure via witnesses — §7.2's
  "strictly dominates" claim about carrying verifier responses is corrected to
  "a trade". §13.5.3 proposes Merkle-ised record bodies for selective disclosure.
  Nine deliberately accepted privacy costs consolidated in §13.5.7.
- **2026-08-14 (V6)** — Vignette on the composition tradeoff added to §13.5.1,
  with two amendments to the proposed text: aggregation cost restored as part of
  privacy (identical facts, different access cost and evidentiary weight — the
  argument this document already makes about camera networks), and the exposure
  baseline scoped to a median user rather than asserted universally. Registered
  as assumption A12, marked known-false for part of the population. §0 notes that
  V6 is persuasive rather than illustrative, a genre that must carry its own
  counter-argument and a statement of what it may not be used for.
- **2026-08-14 (camera comparison withdrawn; P12 found)** — The Ring/Flock
  comparison was wrong twice over: those are designed aggregation services, not
  examples of high-friction fragmented data, and the "zero access cost" claim was
  already false for this design because attestation has been pull-only since §11.
  New §13.5.1.1 records the error and states disaggregation as a positive
  property — §13.5 had catalogued composition risk without crediting what limits
  it. New §13.5.1.2 records **P12, Critical: end-to-end payload encryption is not
  specified**, so both serving infra nodes currently see plaintext. The patron was
  accepted as a metadata chokepoint, never a content one, and the distinction was
  never written down.
- **2026-08-14 (resources)** — New §8 defines the resource object: services and
  data stores owned by a node, addressed relative to it, with access adjudicated
  by the requester's position. Notable as the one place §1.1's diagnostic comes
  out affirmative — owner and requester share topology and the owner runs the
  service, so access control is genuinely enforceable rather than merely visible.
  Resolves §16.10 structurally: *borrows authority → resource; bears its own costs
  → node*, which makes an agent a resource with permission to act, and makes its
  inability to accumulate independent standing structural rather than asserted.
  Object in scope; permission vocabulary and interaction protocol deferred.
- **2026-08-14 (payload confidentiality)** — New §10.2 sketches requirements for
  end-to-end encryption to the *addressed endpoint*, naming the leaf-to-leaf,
  leaf-to-patron and leaf-to-resource cases. Separates the patron's two roles:
  ciphertext as relay, plaintext as endpoint — conflating them is how "the patron
  sees everything" returns. Enforcement is affirmative under §1.1 (the client
  withholds the key). Two forced consequences recorded: store-and-forward makes
  asynchronous key agreement mandatory, so prekeys and their exhaustion,
  last-resort and rotation problems arrive with it; and §5 specifies signing keys
  only, saying nothing about payload KEM keys or their binding to identity.
- **2026-08-14 (adversarial: hostile client)** — Four findings, all upheld.
  **§13.4's patron-eclipse justification withdrawn**: "no asset to steal" is true
  and irrelevant, because the asset is the victim's authentication view, which is
  this network's primary product — and §9.4's no-cold-lookup property *amplifies*
  eclipse rather than being neutral to it. Subnet plurality promoted from side
  effect to structural mitigation. **Selective-abort grinding closed**: commit-
  reveal prevented changing the nonce but not aborting the attempt; fixed by
  seeding from the participant pair plus a time window and by revealing nonces
  only after the physically expensive steps, a point previously unspecified.
  **New §7.2.1 records a structural gap**: the evaluator learns the subject's
  history size from the subject, so selective disclosure collapses the verifier
  threshold and the anti-suppression property. Per-subnet presence chain proposed,
  not decided. **P13 added**: retention promises are undetectable against a hostile
  client, since illegally retained photos can be laundered as `personal_knowledge`.
- **2026-08-14 (chained archive; PoP countersignature removed)** — Two changes
  from the 0.8 findings. **Patrons no longer countersign PoP** (§6.4): the patron
  was not present, PoP sits outside the subnet trust envelope, and a patron able
  to veto it shapes what later patrons see. This also makes the eclipse attack
  permeable, since witnessless ceremonies (§9.8.2) let an eclipsed user memorialise
  any real meeting unsuppressably. **The archive becomes a hash chain** (§7.2.1):
  each transaction carries a hash of the subject's previous one, so excision is
  impossible and only truncation to a prefix remains — which is self-defeating,
  because history is standing. Finding 3 dissolves rather than being mitigated.
  Emergent and undesigned: **the archive is a second factor** — key without archive
  yields no portable reputation.
- **2026-08-14 (self-review sweep)** — Eleven propagation and staleness defects
  found and fixed without external review. Notable: §1.1 still asserted the
  retention-detectability claim withdrawn after 0.8; §11 omitted resources and
  contradicted §10.2 on who holds payload; the build order omitted the chain,
  which **cannot be retrofitted** since back-pointers must exist from the first
  transaction; and **the chain had been applied only to presence records when it
  covers every transaction type** — corrected by moving back-pointers to a common
  body field, one per required signer. Chain genesis resolved as
  SHA-256(signer keyhash). New assumption A13 (users retain their archives) and a
  seed-window parameter added. Full list in `review-tracking.md`.
- **2026-08-15 (unspecified areas)** — New §17 lists features the design does not
  address at all, kept separate from §16's open questions about specified
  mechanisms. Four load-bearing: **negative attestation** (every edge carries
  positive capacity; nothing can say "this identity defrauded me" peer-to-peer,
  and the cure may be worse than the asymmetry), **succession** (no mechanism for
  a permanently absent patron, so teams cannot outlive their members),
  **multi-device** (one keypair per device is unviable and collides with the
  archive-as-second-factor property), and **intra-subnet discovery** (the one
  place a directory is compatible with §1.1, and urgent now that resources exist).
  Five contained: group operations, state sync, resource abuse controls, time and
  ordering, invitation flow.
- **2026-08-15 (feature gaps tested)** — Nine candidate gaps from §17 tested
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
- **2026-08-15 (chain forking)** — Followed through on the multi-device fork risk.
  **A deliberate self-fork reintroduces selective disclosure**: both branches
  inherit the full pre-fork history and its standing while each omits the other's
  post-fork transactions, which beats permitted identity plurality where each
  identity starts from zero. §7.2.1's "truncation to a prefix is the only edit" is
  corrected to **prefix plus one divergent tail per branch maintained**. §17.2
  reframed from usability tradeoff to security decision — only the single-primary
  shape prevents forking structurally. Partial mitigation added: publishing the
  chain head with the currency attestation makes a fork detectable on contact.

- **2026-08-15 (header corrected)** — Two stale values in the document header. The
  date was stamped 2026-08-12 throughout, having been carried forward from the
  first entry rather than checked. *(Superseded: all dates were reconstructed from
  transcript timestamps later the same day — see this log's header.)* **The status line was also stale**, claiming wire formats and session
  establishment were unspecified when `wire-format.md` exists and §10.1 closed
  session establishment. Both are the same stale-value failure the review passes
  kept finding in the design text — committed in the header and in the record of
  finding it, which is a reminder that the discipline applies to process artifacts
  and front matter, not only to the specification body.
- **2026-08-15 (fork finding retracted)** — The author's claim that deliberate
  multi-device forking reintroduces selective disclosure is **withdrawn**. §9.8.7
  explicitly disclaims cross-subnet accountability, so presenting different views
  to different subnets attacks no stated guarantee — the finding was generated
  against an imagined requirement. §7.2.1's original claim is restored and scoped:
  the guarantee is **per-evaluation**, and within one subnet's view truncation to a
  prefix remains the only edit. §17.2 reframed around the real problem, which is
  **accidental** forking from concurrent device use — silent loss of the user's own
  history rather than an attack. Shape 3 (per-device keypairs with published
  bindings) identified as the likely answer, since it removes the shared chain
  entirely and preserves offline signing.
- **2026-08-15 (archive merges)** — Forked archives can be **merged**: a
  transaction following a fork carries back-pointers to both branch heads. This
  **strengthens completeness** rather than merely repairing divergence, since the
  merge commits to both branches and omitting one leaves a back-pointer
  unsatisfied. The archive is properly a **Merkle DAG**, not a chain; cross-branch
  ordering is deliberately not recovered, because the structure proves no
  intermediate record is missing rather than ordering events (§17.4 makes the same
  argument for cross-identity ordering). **Supersedes all three multi-device shapes
  in §17.2** — no primary device, no head-check before signing so offline signing
  works, and no device-binding declaration since the merge is self-describing. Wire
  change: back-pointers become a list per signer.
- **2026-08-15 (resource layer specified)** — Four items moved from resolved-in-
  principle to specified. **§8.4 permission scopes**: access expressed as a
  region relative to the owner, with the horizon bounding the vocabulary naturally
  since relative position is not computable beyond it — only explicit grants reach
  further. **§8.5 service catalog**: DNS-SD-shaped entries signed by the owner,
  propagating as topology and filtered at the source by `discover_scope`, so
  browsing shows what you can plausibly use. **§8.6 abuse reporting**: one event
  type addressed to the owner, never broadcast, with response left entirely to
  resource policy — which keeps it clear of §6.2.2's reasons for having no
  peer-to-peer negative attestation. **§10.3 group operations**: a client library
  over pairwise sends, with its non-guarantees stated so callers do not assume
  atomicity or ordering the network cannot provide. Wire types 6 and 7 added.
- **2026-08-16 (resource layer agreed)** — §8.4 permission scopes, §8.5
  service catalog, §8.6 abuse reporting and §10.3 group operations agreed as
  written; PROPOSED markers removed and wire types 6 and 7 promoted from [P] to
  [D]. Note these were agreed from the chat description rather than from a
  document read-through, so §17's remaining entries and the open-question list
  are the right places to check for anything that did not survive the transition
  to written form.
- **2026-08-16 (consent countersignature; vignettes V7–V8)** — **The subject now
  countersigns every verification query about themselves**, replacing the standing
  disclosure policy, which is removed from the presence record. This is stronger
  than notification-plus-aggregation: a querier could send *different* fuzzed
  profiles to different verifiers, one probe each, defeating a counting defence —
  per-query countersignature lets the subject require identical profiles across a
  ceremony, giving one probe point instead of *n*. Availability is free because the
  subject is physically present at the ceremony the query is bound to. Retention
  commitments remain, governing storage rather than disclosure. **V7 (jury
  nullification) and V8 (postal model) extracted from §4.1.1**, where they had sat
  as formal-voiced prose since before the vignette convention existed; §0 now says
  to check for that case first, since extraction has produced three of eight
  vignettes.
- **2026-08-16 (pre-review sweep)** — Propagation check before re-running the
  review plan. Fixed: §17.2 still told the reader to state a degradation
  "whichever shape is chosen", though merges (§7.2.1.3) had superseded the three
  shapes; §13.5 had no entry for the resource layer, specified after it was
  written, so **P15** (catalog entries reveal what a node runs to its whole
  horizon, and the set of them is a fingerprint) and **P16** (abuse reports
  accumulate as signed complaints at the owner) were added. §13.5 now carries a
  coverage caveat naming the four subsystems specified after pass 0.7, since
  hand-added findings are not a substitute for a systematic re-run.
- **2026-08-16 (0.1 factual verification, second run)** — Four contradicted, two
  substantive. **COSE does not supply application-role domain separation**: the
  `Sig_structure` context is `"Signature"`/`"Signature1"`, distinguishing COSE
  structure types rather than roles — and this protocol had grown to **seven**
  signing roles on the strength of a property it did not have. Wire §1.1 now
  requires a distinct `external_aad` role tag per context, reconstructed by the
  verifier from context rather than content. **UWB is not categorically
  relay-resistant**: USENIX Security 2022 demonstrated physical-layer
  distance-reduction against deployed 802.15.4z HRP parts including Apple U1,
  without key knowledge. §7.1.6.3 downgraded — **no channel is categorically
  relay-resistant** and all proximity evidence is probabilistic, which is
  consistent with §7.1's existing "a cost, not a primitive" framing but changes the
  confidence a policy should place in a record. Also: `alg` in the protected header
  is a profile rule rather than a COSE guarantee; background networking is
  constrained rather than impossible; DNS-SD puts the endpoint in **SRV, not TXT**,
  so the catalog gains a separate metadata field; and the §7.1.1 liveness claim is
  registered as unsourced.
- **2026-08-16 (0.2 coherence, second run)** — 26 contradictions, all fixed. **Two
  blocking.** A light client was told to dial its patron while §4.3 permits
  light-client patrons with no static address — a genuine architectural error, now
  corrected to **dial the nearest serving infra node**, with serving and
  countersigning named as separate roles. §9.5 still called the anchor table
  globally replicated. Of the serious findings, three changed substance rather than
  wording: the **eclipse-permeability claim is scoped to new joiners**, since a
  witnessless ceremony is only valid where the finalization threshold is zero;
  **patrons do see presence records** (they store them) and merely do not
  countersign them; and **the archive provides ancestor reachability, not total
  order**. Also: merges do not invalidate anchors, abuse reports get their own
  storage class so they cannot become public accusations, and §6.3 now actually
  states the ASN/prefix fields §4.4 had been citing it for.
- **2026-08-16 (direct payload path)** — NAT traversal returns for payload only.
  Infra nodes act as **STUN and TURN**, which adds no capability they lacked —
  relaying payload is what a TURN server does — and makes the relay a **fallback**
  rather than the default. §10.1.1's "there is no NAT traversal problem" is
  withdrawn: it held only because payload was always relayed. Three consequences.
  **Infra economics improve materially** — §12.6's ~$20/month was never checked
  against relaying all payload for 1,110 subordinates, and bandwidth would have
  dominated it. **The relay path stays first-class**, since symmetric NAT and CGNAT
  defeat hole punching and two mobile peers is both the worst and the common case.
  And **new finding P17**: a direct connection reveals each peer's IP to the other,
  trading serving-node metadata for counterparty location signal, so
  direct-versus-relay must be a stated choice rather than a silent optimisation.
- **2026-08-16 (direct path horizon-limited)** — Direct payload connections are
  confined to the ±2 tier horizon; everything outside relays. The horizon is the
  right boundary because it is **already the set that holds your locator and
  topology** (§9.1), so IP is incremental disclosure inside it and novel disclosure
  outside — and the check needs no new state. P17 downgraded to Low–Medium
  accordingly, with the residual noted: the horizon is bounded rather than chosen,
  and includes cousins a user may never have met, so both defaults stay
  overridable. **A2 gains a fifth dependent** and its failure would now collapse
  §12.6's infra economics as well. §11.1 gains a table of the horizon's five jobs,
  since *h* is now the most over-loaded parameter in the design.
- **2026-08-16 (0.3 unjustified claims, second run)** — 36 findings in three
  severity bands, against 280 undifferentiated in the first run; the rubric added
  after that run is what made the output actionable. **Structural finding: §14.1
  and Appendix A.2 are orthogonal registers and were not linked.** Six claims were
  unsourced *and* load-bearing while appearing only in §14.1 — matcher
  reconstruction cost, TURN relay fraction, the PAD claim, face entropy, the
  ageing regime, and infra costs. Added as **A14–A19**, and the orthogonality is
  now stated. **A17 is singular**: the only assumption used to *reject* an
  alternative rather than support a choice, closing off fuzzy commitments as a way
  to keep verification without keeping biometrics. §12.3's "peering is the cheapest
  route" superlative **withdrawn** — unattested adoption is plausibly cheaper —
  though the mitigation stands, since it needs peering to be cheap and
  endorsement-invisible rather than cheapest. Seven intensifiers replaced with
  figures or mechanisms per §0's intensifier rule.
- **2026-08-16 (trust ceiling framing withdrawn)** — §12.3 rewritten. **There is no
  trust ceiling**: standing is per-observer, and a peering edge is visible only
  within the two peers' horizons, so it conveys nothing to anyone outside them. The
  concern is therefore local and targeted — *gain standing with a particular victim
  via a favour from their neighbour* — not a route to global standing. What
  survives is that peering reads as a technical request rather than an endorsement,
  which is exactly what the low default flow capacity addresses: **the mitigation
  was right and the justification was wrong**. New **§12.3.1** states a principle
  that was nowhere in the document: **the min-cut bound is observer-relative**.
  Different observers hold different edge sets and compute different cuts, so
  §13.3's Sybil bounds are per-observer, invisibility is conservative rather than
  exploitable, and an attacker must work per-target rather than accumulate edges
  globally.
- **2026-08-16 (0.4 parameter inventory, second run)** — 138 parameters
  inventoried jointly across both documents, 30 unset. **Joint review found what
  single-document review could not**: the presence-record size conflicted (~35 KB
  design vs ~40 KB wire), the transaction-type count was stale at five against
  seven, and the **envelope signer bound of 12 contradicted a witness bound of 16**
  — a record one bound permitted the other rejected. The signer bound is now
  **derived** from the per-role bounds rather than asserted, making the
  incompatibility structurally impossible. Eleven arrays lacked the maximum that
  wire §1 requires of every array; all now bounded. **"Current-keys assertion" and
  "currency attestation" were one object under two names**, which had produced two
  inventory entries with independent unset TTLs — unified. New **§15.1**
  consolidates all unset parameters, grouped by whether settling each needs a
  security argument, measurement, a policy decision, or an encoding decision.
- **2026-08-16 (0.5.1 rule fragility, second run)** — Six residual
  identifier-coupled rules restated in role terms; the reviewer confirmed six
  previously-fixed instances are holding. **Generalising the signing-tier rule
  exposed that §5's justification was invalidated by the archive chain.** §5 said
  routine transactions could be classical because "signatures need only hold until
  nobody relies on them" — but §7.2.1 later made the archive a hash chain, and
  verifying a history means verifying its records' signatures, so reliance lasts as
  long as the archive is presentable. Once classical signatures are forgeable,
  fabricating a record needs only a forged counterparty signature, defeating
  §7.2.1's position guarantee. **All archive-retained transactions are now
  PQ-signed** — ~8 KB each against a photo store at ~100 MB. Note the shape: every
  prior propagation failure ran forwards, this one ran **backwards**, a later
  decision undermining an earlier justification while leaving its text looking
  correct. Greping for changed terms cannot catch that class.
- **2026-08-16 (0.5.2 unenforceable mandates, second run)** — Two findings against
  nine in the first run; §1.1 is largely holding. **Both survivors were written the
  same day, after §1.1 existed.** §12.2 stated the principle correctly and then
  mandated a foreign implementation's λ seven lines later — now restated as a fact
  about the arithmetic (a metric that does not decay steeply enough **diverges**),
  with the policy descriptor — bad news only — as the visible artifact. §8.6's rule about
  intermediaries not retaining abuse reports **was introduced by the §0
  role-generalisation earlier the same day**: the original delivery property was
  enforceable at the sender and the generalisation reached into foreign storage.
  Split into the enforceable part and a visible distinction. §0 now records that
  **generalising a rule can break §1.1** and that the two conventions must be
  applied in order — state the property, then ask who would enforce the restated
  version against whom.
- **2026-08-16 (0.7 LINDDUN, second run)** — Full re-run over 28 flows and stores,
  discharging the coverage caveat. 31 threats and 10 correlation findings; the
  individual threats mapped almost entirely onto P1–P17, and **the new material was
  overwhelmingly compositional**. New **§13.5.8 correlation register** — the
  structure §13.5.1's invariant required and the document lacked. Four new
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
- **2026-08-16 (0.6 implementation attempt, second run)** — Encode/sign/verify for
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
- **2026-08-16 (WASM storage claim corrected)** — An earlier claim that a WASM
  target has no filesystem access was **browser-specific stated as general**, and
  is withdrawn: WASI gives native WASM ordinary file access, an embedded component
  gets host bindings, and even in a browser OPFS provides real file handles. **What
  survives is durability**: browser storage is evictable under storage pressure
  unless persistent storage is granted, and since §7.2.1.1 makes the archive a
  second factor, silent eviction costs a security property rather than only
  convenience. §17.2 now states this, and a browser-hosted client needs
  §9.8.7.1's backup path to be mandatory rather than advisory.
- **2026-08-16 (0.6 target 2, resolve a locator)** — **The resolution
  request/response protocol did not exist.** The design described resolution
  narratively across §9.3 and §9.6.1 and no wire message carried it, so the
  network's primary operation was unimplementable. New `wire-format.md` §5.6
  specifies it. Two further findings of substance: the **locator authentication
  contradiction** — design §9.1 requires the node's signature, the wire format
  claimed an enclosing envelope always supplies it — resolved by distinguishing
  carriage, with a `SignedLocator` required standalone and a bare locator rejected;
  and **descent is through infrastructure only**, terminating at the serving infra
  node and returning the residual path suffix rather than traversing it, which is
  what lets a path address a light client nothing can route to directly. That was
  implicit and is now in §9.6.1. Anchor entries are now self-signed — they vouch
  for nothing, but an unsigned entry lets a gossip peer inject addresses and
  partition the network.
- **2026-08-16 (0.6 target 3, validate a presence record)** — 30 findings. **The
  verifier-selection invariant could not be recomputed from the record**: §7.2.2
  requires third-party recomputation so a missing verifier is visible, and the
  witness nonce commitments and reveals it depends on were nowhere in the presence
  record — the anti-suppression property was unverifiable. Now fully specified in
  `wire-format.md` §4.6: nonce carriage in `Witness`, a domain-separated
  identity-bound commitment, canonical seed bytes with participant order
  canonicalised so it cannot become a grinding variable, **a 24-hour epoch-aligned
  window** (filling the §15 UNSET), hash-rank sampling, and candidates as
  **distinct counterparties**. The threshold gains a necessary third term —
  `min(floor(n/2), 10, |candidates|)` — since *n* counts transactions while
  candidates are people. `pending` and `unavailable` count toward finalization,
  because the alternative lets an attacker block it by making verifiers
  unreachable. Consent signs `query_id` rather than the query, keeping a 4 KB
  profile out of the record while remaining verifiable. Several structural rules
  were simply absent and are now stated; `duration_s` removed as derivable.
- **2026-08-16 (verifier recomputability scoped)** — Two corrections. §7.2.2's
  claim that selection is "recomputable by any third party" is **stronger than
  achievable**: checking a subject's verifier set needs that subject's candidate
  set, which comes from their archive, so the property is **per-subject and
  holder-relative** and nobody validates both halves of a record without both
  archives. That scope is correct — an evaluator assessing A cares whether *A*
  suppressed verifiers — but was unstated. Second, a **binding invariant** added at
  `wire-format.md` §4.6.2: a subject's selection must be recomputable from the
  record plus that subject's own history and nothing else, so the seed may draw
  only on record-carried values and the candidate set only on the subject's own
  history. The current construction satisfies this by accident of design; adding
  counterparty-derived entropy would keep every property §7.2.2 asks for while
  making selection **silently unverifiable**, and would not show up in testing
  because a developer holds both archives.
- **2026-08-16 (verifier recomputability, direction corrected)** — **A selects
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
- **2026-08-16 (0.6 target 4, client attach with sibling failover)** — 14
  findings, two unsatisfiable. **A zero-sibling topology had no valid encoding**:
  `AttachAck` required one-or-more siblings while §1 forbids encoding an empty
  array, so an infra node with a single child could produce no conforming ack.
  And **§10.1.2's requirement that degraded attachment "must be explicit" had no
  wire representation** — `Attach` now names the client's intended serving node and
  `AttachAck` echoes the receiver's determination, so a disagreement surfaces
  rather than one side permitting operations the other considers unavailable.
  **The PQ transport handshake was also unspecified**, now settled with a standard
  rather than an invention: QUIC + TLS 1.3 with group `X25519MLKEM768`, peer
  authentication by raw public key (RFC 7250) since identities are keyhashes and
  there is no CA, and ML-KEM-768 as the profile set. Heartbeat units, counter
  semantics, failure threshold, failback, queue-depth meaning, endpoint port and
  the sibling-update frame were all undefined and now are.
- **2026-08-16 (degraded-attachment "explicit" corrected)** — §10.1.2's requirement
  that degraded attachment be explicit is a **user-interface obligation**, not a
  wire one; the author had misread it and added an `Attach` field for the client to
  assert its intended serving node, now removed. The supporting argument was also
  wrong: a sibling can determine the mode from its own topology, since a client
  attaching to it sits two hops away within its horizon. `AttachAck`'s mode field
  is kept for a better reason — it reports the **server's** determination, because
  a client with stale topology may not know which state it is in. §10.1.2 now states
  the UI obligation as a reference-client requirement, of the same kind as
  §13.5.6's disclosure rules.
- **2026-08-16 (0.7 LINDDUN, third run)** — Independent walk over 42 flows and
  stores; most findings mapped to the existing register. **Three new
  correlations.** C11: a prekey fetch precedes any message, so a patron sees intent
  separably from delivery and **communications that never happened become
  visible**. C12: the verification-query log is a *defence* that becomes an
  exposure — retained beside the archive it timelines ceremony attempts the network
  never recorded. C13: enumerated disavowal reasons avoid a defamation surface, but
  **context supplies the semantics the code omits** when joined with resource
  history. **New P23**: §13.5.6's capture-time disclosure obligation covered
  participants only — a verifier who answers permanently proves they met the
  subject, and the consent machinery protects the subject while asking the verifier
  nothing. **P22**: query logs have no retention rule. §13.5.7 gains an accepted
  cost for the absence of erasure at the evidence layer, noted as a compliance
  posture needing a documented lawful basis rather than a deletion mechanism.
- **2026-08-16 (resource layer propagated)** — `resource-requirements.md` folded
  into the documents. §8 rewritten around the execution model: **the infra node
  is the front door and the resource sits behind it**, so a resource never reads
  network state and a compromised one leaks only its own data. New §8.0.1:
  **wider reach is federation, not wider scope** — a subnet-wide service is many
  local instances, each administered by a patron, which makes §1.2's freedom
  argument an architectural property rather than a claim. §8.2 states
  **membership in the owner's Dunbar Org as the precondition for all access**, so
  departure revokes uniformly and no access accumulates; it also separates
  ownership from hosting. §8.4 adds package-declared roles, predicate binding,
  and **rank-based rather than raw trust thresholds**, since a raw threshold is
  denominated in units meaningful only within one metric family. §8.5 splits the
  signed `CatalogEntry` from the personalised catalog page and notes role names
  leak. New §8.7 on gateways re-concentrating what the architecture
  disaggregates, with broker-rather-than-proxy as the default. New **P24** (gateway
  operators see external traffic), **A21** (patrons will administer resources — the
  resource-layer sibling of A11), **§17.6** *(since removed as out of scope)* (external root-of-trust bootstrap is
  unmapped), and a new accepted risk in §13.4: **a compromised infra node can forge
  its subordinates' access**, though not their transactions. §1.2 gains the SSO
  adoption observation; §11.1's horizon table gains a sixth job.
- **2026-08-16 (agent deferral withdrawn)** — §2 still deferred delegated-agent
  participation while requiring its schema constraints be settled immediately. Both
  halves are stale: §8.3 makes a delegated agent **a resource**, and §8.2
  already gives resources identity without consuming subordinate slots — so there
  is nothing left to settle in the schema. §16.10 narrowed to the one question that
  remains open, **what "presence" means for a persistent self-directed AI**, and §2
  now notes that autonomous participation is deferred as a *decision* rather than
  as a mechanism: such a participant is an ordinary node.
- **2026-08-16 (payload encryption: import, not invent)** — §10.2.4 replaced.
  **Directly applicable prior art exists, specified and formally verified**, so the
  requirement is to adopt rather than design. **PQXDH** for asynchronous key
  agreement — the exact situation of a recipient offline with published prekeys —
  whose published second revision incorporates fixes from ProVerif and CryptoVerif
  analysis (USENIX Security 2024). **The Triple Ratchet** (Double Ratchet plus
  SPQR, released October 2025) for session forward secrecy and post-compromise
  security, whose erasure-coded chunking already solves the ML-KEM key-size problem
  this design would otherwise hit. Two narrowing observations: **only leaf-to-leaf
  needs any of it**, since patron and resource endpoints are online by definition
  and covered by the §10.1.1 transport handshake; and §10.2.2 had independently
  reconstructed the X3DH prekey shape, so importing replaces a hand-rolled version
  with the analysed original. One place to exceed the deployed profile: PQXDH's
  published revision authenticates classically, and §5.1's hybrid identity permits
  binding to the post-quantum component too. Also noted: PQXDH's **deniability** is
  the property the data plane wants, precisely inverting the non-repudiation the
  control plane requires — the split is correct rather than a compromise.
- **2026-08-16 (payload encryption propagated)** — The §10.2.4 adoption carried
  into the companion documents. **`wire-format.md` §5.7 adds prekey distribution**
  — `PrekeyBundle`, `PrekeyRequest`, `PrekeyReply` — with the bundle **opaque to
  this protocol**, since only the endpoints hold the state to interpret it and
  carrying it as a blob means a PQXDH revision forces no wire change. Serving a
  one-time prekey consumes it; falling back to the last-resort key is a declared
  reduction in forward secrecy rather than a failure. Light-client obligations added
  for publishing and replenishing bundles, and for not prefetching speculatively
  since a fetch discloses intent to message. Infra-client obligations added for
  holding and serving bundles, telling a subject when their pool is exhausted —
  an attacker can drain it — and keeping no record of who requested whose bundle.
  Prekey rotation cadence added to §15.1's unset list.
- **2026-08-16 (blanket prekey prefetch)** — The intent-disclosure problem (C11) is
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
  everything after. Prefetch scope is the horizon's **seventh** job (§11.1).
- **2026-08-16 (archive presentation: head txid)** — The last blocker to finalising
  an adoption transaction is closed. Adoption field 7 is **a single head txid**, not
  a list, range or proof: §7.2.1's chain already carries the predecessors, so a
  patron walks backward from the head and fetches in batches (`wire-format.md`
  `wire-format.md` §5.8). Truncation needs no grammar — presenting less history means presenting an
  earlier head, the only edit the chain permits. **The patron chooses its own
  depth**, which is the right asymmetry: the presenter picks the head and cannot
  control how far back the recipient looks. The three alternatives were rejected on
  specific grounds — a **list** duplicates the chain while proving nothing, a
  **range** is *reachable from B but not A*, which is precisely the excision §7.2.1
  exists to prevent, and a **Merkle proof** optimises away a fetch the patron needs
  anyway to recognise counterparties. New `wire-format.md` §5.8 specifies batched
  fetch, with the requester verifying the chain itself since a holder cannot be
  trusted to have walked it correctly.
- **2026-08-16 (disavowal reason codes)** — `wire-format.md` §4.3 defines a
  **64-value space banded so that prejudice is structural**: codes 0–31 without
  prejudice, 32–63 with. A policy can evaluate an unfamiliar code correctly —
  `code >= 32` means an adverse judgment — without a lookup table or a
  specification update, which matters because most values are undefined in v1 and
  will be assigned later. Ten codes assigned. **Codes 32+ are the vocabulary for
  the negative attestation §6.2.2 describes.** Code 4, incompatible subnet
  membership, sits without prejudice deliberately: the patron is asserting
  something about *that subnet*, not about the subordinate. Also clarified: §17.6's *(since removed)*
  external root-of-trust bootstrap is **not** the genesis procedure — §9.8 is
  bootstrapping inside the network, §17.6 is an outside consumer deciding which
  issuers to recognise. §16 item 10 marked **deferred by decision**, not to be
  implemented in this or any intervening version.
- **2026-08-16 (two items moved out of scope)** — **Negative attestation and the
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
- **2026-08-16 (capability negotiation)** — `wire-format.md` §6.1 replaces the
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
  exercised in production rather than only in tests. §17.5's blocker list is down
  to one: the resource interaction protocol.
- **2026-08-16 (node-resource trust boundary)** — §8 settles a constraint the
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
- **2026-08-16 (revocation scope)** — §8.2's claim that departure revokes
  everything uniformly is scoped: **membership gates session establishment;
  continuation is the resource's business.** A hosted package terminates at the
  node so revocation is immediate; an external service continues on its own terms,
  **which is what choosing that service means.** The network provides the bridge
  and administers who may cross it; it does not undertake to improve the products
  a team decides to use. A manifest field declaring revocation support was
  considered and rejected — unverifiable, and it framed a third-party property as a
  shortfall here. The infra client shows which kind a resource is, since that
  follows from where it runs.
- **2026-08-16 (session termination on role change)** — The reference infra client
  **drops hosted sessions when a principal's roles change**, rather than notifying
  the resource. Local behaviour, not a protocol rule — the network cannot reach into
  an operator's node — but it is what makes §8.2's membership gate mean what it
  says for the resources where anything here can. **Termination is simpler than
  notification and that is why it is correct**: notifying would need a protocol, an
  acknowledgement, a latency budget and a rule for requests already in flight,
  whereas dropping the session means **role changes are enforced by reconnection** —
  no mid-session mutation, no partial-privilege state, no request that starts under
  one role and finishes under another. This closes two of the interaction protocol's
  open questions: no notification mechanism is owed, and the node need not reach
  into a live session. What remains is the credential's contents.
- **2026-08-16 (pairwise principal identifiers)** — A resource sees
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
- **2026-08-16 (resources assert nothing into the trust graph)** — §8.0.3 closes
  the last question with design content in it. **A resource reports to its host and
  may message nodes peer-to-peer where configured; it emits nothing the trust
  metric consumes.** The reason is that the alternative is unbounded — letting
  resources assert would require **global message types for arbitrary service
  needs**, so a fixed protocol would have to absorb an open-ended ecosystem's
  vocabulary. It is also §8.3's line applied: a resource emitting trust signals
  would be *borrowing authority in order to create trust*, the combination excluded
  everywhere else. **Two consequences.** Resource-initiated contact needs nothing
  new — a resource is a payload endpoint (§10.2), addressable via `CatalogEntry`.
  And the request path carries no trust, which makes the interaction protocol a
  **framing choice rather than a design question**: HTTP/3 over the existing QUIC
  session, credential in request headers, is the expected answer and matches the
  reverse-proxy pattern §8 already uses as its analogy.
- **2026-08-16 (interaction protocol closed)** — The last blocker. **HTTP/3 over the
  existing QUIC session**, credential in request headers
  (`resource-requirements.md` §3) — no new transport, no new framing, and no
  signing or canonical encoding, because the request path carries no trust
  (§8.0.3). The **header-spoofing hazard** is stated on both sides, being the way
  this pattern is most often misimplemented: the node must strip inbound `rhtn-*`
  headers before inserting its own, and a resource must not trust them on any path
  but the gateway's, since being behind a gateway is a deployment fact rather than
  something a request can prove. **Owner movement settled** (§8.2): the old
  upline loses access, the down-line is unaffected, and **the new upline gains
  access silently** — nobody acts, the predicate simply matches different people,
  so the client warns before such a move. The larger effect is hosting: a
  light-client owner's resource runs on its serving infra node, so a move that
  changes that node forces migration — an argument for resource owners to run
  infrastructure. **§17.5's blocker list is now empty.**
- **2026-08-16 (provisionality of unset parameters)** — New §15.1.1. *"Take a
  provisional value and tune later"* is true of most of the eighteen and false of
  six. **A parameter hardens when someone other than its chooser depends on it** —
  an attacker choosing where to attack, a peer parsing a number, a consumer reading
  a published figure. Local performance knobs stay soft indefinitely. The security
  ones are the sharpest case: **heterogeneity does not average out, because an
  attacker targets whichever subnet uses the weakest value**, so no individual node
  can protect itself by choosing well. Capability parameter ids are noted as a
  **registry rather than a value** — colliding assignments are unrecoverable, and
  §6.1's greasing protects against unknown ids, not against two implementations
  meaning different things by the same one. Also softened §8.2's owner-movement
  note: accepting a patron is deliberate, and an upline gains access only where the
  operator wrote a predicate reaching upward, so the client reminds rather than
  warns.
- **2026-08-16 (capability ids derived, not assigned)** — The registry problem is
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
- **2026-08-16 (post-refactor sweep)** — Full propagation and reference check
  across all six documents. Fixed: light-client requirements had **no archive
  obligations** — presenting a head on adoption, serving requests for one's own
  archive, verifying a fetched chain oneself, and not reading a short reply as a
  short archive; §6.2.5's heading still said "unspecified" though the interim rule
  is stated; a wire disavowal field cited §6.2.1 when it meant §6.2.2; five wire
  items were marked **[P]** though settled elsewhere, leaving two genuinely
  proposed; §16 item 12 still listed the resource interaction protocol as open;
  item 13 now defers with item 10 rather than standing alone; §7.2.1's rationale
  was the last passage still written as history and is now present-tense. The
  resource-interaction extraction document is **closed** with a table of where each
  answer landed. All references in all six documents resolve.
- **2026-08-16 (dates reconstructed from the transcript)** — Every date in every
  document corrected against per-turn timestamps in the conversation transcript.
  **The project began 2026-08-12T03:36Z and is five days old.** The previous
  stamping put three days of work on 12 August and a further day on **15 August, a
  date on which nothing happened** — an artefact of carrying the first entry's date
  forward rather than checking. Corrected: topology through addressing to **12
  Aug**, currency through wire format to **13 Aug**, review passes 0.1–0.5 to **14
  Aug**, everything since the 16 August compaction to **17 Aug**, with this
  session's entries not separately dated because it spans 16–17 August. Also
  corrected a claim in §0 that §4.1.1 held two unextracted vignettes "for weeks",
  which was **two days**. Body references to the first review passes now say 14
  August rather than 12. Ordering was correct throughout; only the dates were not.
- **2026-08-16 (structure)** — Format and organisation pass over the five
  deliverable documents. **§6 held 1,949 lines — a third of this document — across
  three unrelated subsystems**, and is split: §6 keeps transaction mechanics, §7
  becomes **Proof of presence and recovery**, §8 becomes **Resources** (which was
  never a transaction type), and everything after shifts by two. **`wire-format.md`
  was out of sequence**: §4.6 sat after §5.5, and §5.6–5.8 after that, from
  appending rather than inserting; now ordered. **`resource-requirements.md` is
  confirmed as part of the deliverable set** — §2 and §3 are the only specification
  of the credential and the request framing anywhere — and converted from
  working-file form: the R-numbering, the propagation table and the
  "what this implies" section are gone, replaced by plain sections and the same
  no-protocol-facts rule the other requirements documents carry. Also: §13.5.8 sat
  before §13.5.7; four headings still carried stale status markers
  (*REQUIREMENTS SKETCH*, *DATA PLANE UNDESIGNED*, *NEW FINDING*, *Still deferred*).
  All references in all six documents re-verified after the renumber.
- **2026-08-16 (0.1 factual verification, all five documents)** — 32 externally
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
- **2026-08-16 (0.2 coherence, all five documents)** — 18 contradictions, four
  blocking, almost all stale summaries left by the day's rapid edits. **The four
  blocking were in the wire format**: a §2.2 algorithm row still said routine
  transactions are classical-only, contradicting §5.1's rule that every archived
  transaction needs both components; the envelope said *one* `COSE_Signature` per
  signer where §3.2 requires two; the signer bound counted **logical signers** where
  §1 presented it as the array's own maximum, so a presence record's real ceiling is
  68 entries rather than 34; and subject consent was specified as signing both the
  full `VerificationQuery` and the `query_id`. **Also fixed:** `duration_s` survived
  in §7.2's illustrative schema after the wire format removed it; §16 listed the
  disavowal enumeration, version-mismatch behaviour and CBOR-versus-protobuf as
  open when all three are settled, and pointed at a nonexistent `wire-format.md`
  §12; §17.5 listed test vectors as blocking two lines above "none remain";
  parameter counts disagreed (17 versus 23); §11 said four message classes over a
  five-row table; and **five presence references had become §9.2 in the section
  split** — pattern-indistinguishable from the legitimate anchor references, so the
  earlier repair missed them. In the requirements documents: resource conformance
  still claimed departure revokes everything uniformly after the design scoped it to
  what the node controls; the resource definition contradicted its own external-SaaS
  category; and the infra document listed the interaction protocol as open.
- **2026-08-16 (0.3 unjustified claims, all five documents)** — 106 findings across
  three severity bands. **LB1–21 map onto the existing A1–A21 register**, and
  LB27–43 are §15 parameters the document already labels chosen rather than
  derived — so the actionable material was elsewhere. **Five new load-bearing
  assumptions registered as A22–A26**: the transaction-rate ceiling under the
  capacity argument, that an unattested adoption is *near-worthless* rather than
  merely weaker (which is what makes proof of presence optional rather than a gap),
  a few hundred archive records per decade, that ~400 KB is prohibitive (which is
  what keeps embedded evidence classical), and that ~99% discrimination is the right
  privacy/utility point for a fuzzed profile. **Five comparative claims added to
  §14.1** — including §1.2's benchmark that physical-world profiling is expensive
  per target, which the entire privacy target is set against and which no study
  here supports. **§15 now states plainly that every parameter is a chosen
  operating point**, and `wire-format.md` §1 that its maxima are conservative DoS
  ceilings rather than capacity results — exceeding one means *malformed*, not
  *overloaded*. **The SaaS adoption claim was narrowed in `resource-requirements.md`
  during 0.1 and left unfixed in design §1.2.1a** — the same claim in two places,
  one corrected. Twelve intensifiers replaced with the mechanism or the figure.
- **2026-08-16 (0.4 parameter inventory, all five documents)** — Nine findings.
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
- **2026-08-16 (0.5.1 rule fragility, all five documents)** — Thirteen rules
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
  enumerated list of causes; and **§15.1.1's hardening rule** as a property with the
  table as instances. Two generalise beyond their sections: *absence of data from a
  holder is never evidence about that data's existence*, and *a component behind an
  authenticating intermediary must not treat intermediary-asserted metadata as
  authentic unless it can verify the path*.
- **2026-08-16 (0.5.2 unenforceable mandates, all five documents)** — Eight
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
  Also: §15.1's *"cannot be tuned for convenience"* restated as its consequence —
  an attacker selects which deployment to attack, so the cost of a convenient value
  is borne by everyone rather than the chooser; the capture-time disclosure
  obligation now says plainly that nobody can verify it and the people harmed are
  those never told; and **greasing's two halves are separated**, since whether a
  peer *sends* greased parameters is invisible while whether it **tolerates** them
  is testable by anyone who greases — so the sending obligation is what enforces the
  receiving one.
- **2026-08-16 (0.6 implementation attempt, adoption)** — **Two contradictions,
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
- **2026-08-16 (0.6 implementation attempt, resolve a locator)** — One finding
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
  And new §5.6.0 resolves an ambiguity between the design narrative and the wire
  operation: **a light client sends its resolution request to its serving node**,
  not to the anchor; the request is anchor-*relative* in how its path is read, which
  is not a statement about who it is addressed to.
- **2026-08-16 (0.6 implementation attempt, validate a presence record)** — **A
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
- **2026-08-16 (0.6 implementation attempt, client attach)** — Fifteen findings,
  four blocking. **A stale sentence in design §10.1.2** still said `Attach` names the
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
- **2026-08-16 (post-review consistency scan)** — Swept all five documents for
  values superseded during the day's review passes. Six stragglers: the 400 KB
  hybrid-evidence figure survived in `wire-format.md` after correction to ~211 KB
  in the design; "~121 nodes" survived in two places after the Dunbar figure was
  replaced by its derivation; and "two years" survived where 730 days was adopted.
  Three **malformed section ranges** left by the §6 split — `§13.2–10.3`,
  `§7.1–6.7` — where the first half of a range remapped and the second did not,
  which the reference checker cannot see because each half resolves on its own.
  All references in all five documents resolve; no superseded value remains in
  any body text.
- **2026-08-16 (0.7 LINDDUN, all five documents)** — Four new correlations and
  three findings. **C15 narrows a guarantee written today**: pairwise identifiers
  stop two *independent* vendors comparing IDs, and do nothing to stop **one vendor
  correlating its own several resources** using account and network data it holds
  anyway. §8.0.2's claim now says so — the scheme addresses cross-*operator*
  linkage, not cross-*service* linkage within one operator. **C17 is the cheapest
  serious fix**: a retained source photograph carrying EXIF or recognisable
  background defeats the coarse-geohash design outright, bypassing rather than
  weakening the argument §7.1.6 makes about disclosure precision — the light client
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
- **2026-08-16 (0.8 adversarial, six roles)** — Run in one pass at high effort.
  **The design is much stronger against cryptographic fabrication than against
  control of endpoints, local infrastructure and human participation** — its worst
  cases are where the attacker legitimately possesses the component the
  architecture deliberately trusts. Three findings changed text. **§12.4's
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
- **2026-08-16 (policy descriptor: bad news only)** — A node's account of its own
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
  by existing. Also removed a mangled fragment left in §12.2 by an earlier edit.
- **2026-08-16 (0.1 factual verification, second run)** — **Three contradictions,
  all one error, and it was a conflation rather than a fact.** HTTP/3 is selected by
  ALPN token `h3` at QUIC connection establishment (RFC 9114), while the client's
  session negotiates `rhtn/1` — so standard HTTP/3 cannot be layered onto it, and
  "no new transport, no new framing" was false twice over. The fix separates two
  legs I had merged: **client-to-node travels as an rhtn control frame** on the
  existing session (`wire-format.md` §6.0, frame types 5 and 6), and
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
- **2026-08-16 (0.2 coherence, second run)** — Six contradictions, **both blocking
  ones introduced within the previous two hours**. The HTTP/3 correction reached
  `resource-requirements.md` §3 and left §3.2 asserting the superseded claim four
  paragraphs later — the same fix-one-instance failure as the SaaS claim earlier
  today. And design §7.2's illustrative schema said subject consent signs *the
  query* where the wire format signs the **`query_id`**; implementations following
  each would build different `Sig_structure` payloads and fail to verify one
  another. **Serious**: §7.3's size arithmetic still counted *~3 verifiers* among a
  presence record's signers, contradicting §5.1's rule that embedded evidence signs
  classically and the wire format's exclusion of verifiers from the envelope signer
  set — corrected to 2 participants and ~8 witnesses, with the per-signer cost
  restated as 3,373 B rather than 3,309. **Minor**: the wire format said seven
  signing roles above a table of eleven; two passages described design §7.2.2 as
  requiring recomputation "by any third party" when that section expressly rejects
  the phrase; and the infra document used "prekey fetch" both for the
  intent-independent reusable prefetch and for the on-demand one-time request that
  does carry intent.
- **2026-08-16 (0.3 unjustified claims, second run)** — 84 claims, 58 load-bearing
  against 26 registered. **Most of the gap is §15's parameters and the wire
  format's array bounds**, which both documents already declare chosen and
  conservative rather than derived — but the reviewer's methodology is right that
  **labelling a value "chosen" is a disclosure, not a justification**. §14 now says
  plainly that the register is **curated rather than exhaustive**: it lists
  assumptions whose failure would change a *design decision* rather than a tuning
  value, which is the useful cut for deciding what to test first, and is not a claim
  that the remainder are supported. The strict count is recorded there. **Two new
  assumptions**: **A27**, that a normal presence record carries ~10 logical signers
  with ~8 witnesses — a social artifact underpinning the ~35 KB figure and the
  storage arithmetic; and **A28**, that package authors' incentive runs toward
  breadth in permission defaults, which is the entire motivation for requiring
  templates be inspectable, and is an economic claim asserted rather than argued.
  Three supporting claims added to §14.1 (carrier gateway aggregation, radio access
  latency, and the comparative size of the extension attack surface), and seven
  intensifiers replaced with the property they were standing in for.
- **2026-08-16 (0.4 parameter inventory, second run)** — **No unresolved value
  contradiction across the five documents** — the first parameter pass to come back
  clean. Five apparent mismatches were checked and are not: "two years" versus
  "730 days" is a definition the wire format states; ~35 KB typical versus ~65 KB
  maximum name different operating cases; a 10-per-subject threshold under a
  32-response array bound reflects two subjects plus headroom; a heartbeat interval
  that is UNSET while its floor is 1 second separates a value from its validity
  range; and three superseded bounds appear only where the change log records them
  as superseded. **One item fixed**: §4.4 said at least two cross-tree peers are
  **required** for genuine redundancy while §15 records the same figure as a
  **recommendation**. Same number, different normative force — and the design
  intent is the weaker one, since peering is voluntary and zero peers is supported.
  §4.4 now states it as what redundancy costs rather than as a condition of
  participation.
- **2026-08-16 (0.5.1 rule fragility, second run)** — Eleven residual rules
  restated in terms of the property they protect. The reviewer's exclusions are as
  informative as its findings: it deliberately passed over veto delegation,
  formation typing, verifier-selection fields, audience-bound credentials and scope
  evaluability, on the grounds that **the defect is not that identifiers appear in
  an encoding rule but that the property would disappear with the identifier** —
  which is the right test and the one §0 states. Substantive rewrites: patron
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
- **2026-08-16 (0.5.2 unenforceable mandates, second run)** — Three findings, all
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
- **2026-08-16 (0.6 implementation attempt, adoption, second run)** — **One direct
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
- **2026-08-16 (0.6 implementation attempt, resolve a locator, second run)** —
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
- **2026-08-16 (0.6 implementation attempt, validate a presence record, second
  run)** — Eleven gaps. **One is a self-contradiction introduced the previous day**:
  the 730-day window was given as a formula admitting the boundary instant and a
  sentence excluding it, which two implementations resolve differently at exactly
  that point. Now open at the far end, closed at the near. **The deepest finding is
  that the anti-grinding property had no mechanism behind it.** §4.6.2 claims an
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
- **2026-08-16 (0.6 implementation attempt, client attach, second run)** — Thirteen
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
- **2026-08-16 (§1.2.2: emergent properties made explicit)** — Three properties the
  design relies on and never stated. **Cheap fabrication is a privacy feature**: a
  subnet with no real members is easy to build, so any package of correlated
  evidence is consistent with being invented, and **Sybil attackers contribute to
  everyone's deniability**. The boundary is stated so this does not read as
  contradicting §13.3 — forging evidence about a *specific real person* needs their
  signature and stays hard; fabricating a *whole graph* is easy, so **the deniability
  is in the graph rather than in any signature**, and evaporates for an evaluator
  who can reach the counterparties independently. **The subject's confidence in
  their own evidence is better founded than an attacker's from identical bytes** —
  which is why trust is per-observer rather than a global score, since the same
  evidence genuinely supports different conclusions for differently-placed parties.
  **Baseline exposure is the floor**: each attack should cost at least as much as
  its conventional equivalent against a non-user, which is what makes §13.5's
  register sortable — "an attacker with the device sees everything" is a defect only
  if they see more than they would from any other phone. **The limit is stated with
  it**: the floor holds for *acquisition cost*, not for *evidentiary weight*. A
  camera roll is ambiguous and decays; an archive is durable, portable and
  non-repudiable, so along that vector this design is worse than the baseline, as
  §1.2.1 already concedes.
- **2026-08-16 (0.7 LINDDUN, third run)** — Three new findings. **N2 is C9
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
- **2026-08-16 (§1.2.2: surveillance classes; 0.8 second run)** — §1.2.2's
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
  definition. §13.5 now directs readers to sort findings by which class can use
  them. **From the adversarial re-run**, two structural points: **plurality must
  already exist to defeat an eclipse cleanly** — establishing a second patron while
  eclipsed means reaching one, which is what the eclipsing patron mediates, so the
  escape costs a physical meeting and the client should encourage plurality early;
  and **the state-actor ranking rests on §4.3's unsupported cloud-concentration
  expectation**, so if deployment is provider-diverse it drops below endpoint theft.
- **2026-08-16 (0.8b vignette agreement)** — Four contradictions between vignettes
  and the sections they illustrate, all in the same direction: **the vignette
  claimed more than the mechanism delivers.** V2 showed a first contact resolved
  from a business card, where §9.3 says the locator travels with the key out of
  band and a bare key is not resolvable at all — rewritten as reaching someone
  already in her contacts. V4 said a connection means *"we stood in a room together
  for four minutes"*, which §7.1.1 expressly forbids the record from claiming, since
  witnesses cannot attest that two humans shared a room; now *"somebody spent four
  minutes proving they were where I was"*. V4 also priced the attack as universal —
  *"an attacker with a warehouse of handsets and no warehouse of people"* — where
  §7.1 states plainly that **bilateral collusion is unpreventable**; the cost falls
  on edges to people who did not agree, not on meetings inside territory the
  attacker controls. And V8 described institutions binding *"its own name for you"*,
  the multi-identity property §9.8.7 records as **deferred**. Worth noting the
  pattern: vignettes drift optimistic, because the version that reads well is the
  version that claims the mechanism works better than it does.
- **2026-08-16 (vignette voice)** — The 0.8b corrections were accurate and
  over-corrected: hedging folded into the narration produced prose that is
  technically careful and teaches less. **A vignette now states the base case
  plainly and appends its limits in italics**, rather than qualifying itself
  mid-scene. V4 says *"a connection here says we met"* again, with the two real
  limits noted after — witnesses attest that the protocol ran rather than what they
  saw, and collusion between willing parties is unpreventable, so the cost lands on
  edges to people who did not agree. V2 keeps the introduction requirement as a
  deliberate limit rather than burying it in the scene. V8 notes that v1's single
  identity per client makes the postal analogy partly aspirational. **§0 records the
  rule**: the reader who needs the caveat is not the reader a vignette is for, and
  writing for the most security-anxious reader misinforms the ordinary one — most
  people leak far more than this network asks and are untroubled by it.

- **2026-08-16 (0.9 organisation, part two)** — Two structural moves. **The archive
  is now §8**, a first-class section rather than three levels down under "Presence
  record format" — it applies to every transaction type, says so, and "how does the
  archive work" is a question nobody would have answered by looking under presence
  records. Resources become §9 and everything after shifts by one. **Resolved,
  dissolved and drafted entries moved out of §17 and §18 into Appendix A.3**, so
  "open questions" now means currently open: thirteen entries became five, and
  §18's three closed areas — succession, intra-subnet discovery, contained gaps —
  moved with them. Nine cross-references pointed at the moved material and were
  repaired. Also fixed, for the third time in this pass: **bare `§N` references in
  the companion files**, which the remapping scripts skip because they lack the
  `design §` prefix that identifies them as cross-document.

- **2026-08-16 (status blocks reconciled)** — Swept all six headers. The design's
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
  had `network-design.md` **design §0** from the preamble dedup. Reading order
  gained §8.

- **2026-08-22 (preface; §1 reordered; style pass §§1–4)** — Author's preface added
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
  in the body. §0 keeps only what governs the design itself. **The user's own
  explanations now frame §1.2.2**: the three surveillance classes, fabrication as a
  source of deniability, trust emanating from the user, and baseline exposure as the
  security floor, all in the original framing rather than paraphrased. **Style pass
  over §§1–4**: em-dashes reduced from 60 to 11 in those sections, keeping only
  those marking a genuine aside.

- **2026-08-22 (keystream-encrypted captures)** — New §7.1.5.2. **Each participant
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
  verifier queries**, since §7.1.4 requires the subject's countersignature on every
  query and the subject is therefore always present; the keystream travels in
  parallel with the query. Side effect: **the subject learns which counterparties
  were selected**, making P18's verifier-privacy asymmetry symmetric. **Seeds die
  with the device**, consistent with rotation never carrying state forward from
  beyond the local trust horizon — the scheme adds no dependency, it makes an
  existing one visible. Propagated to P13, P29, C9, the §1.2.2 floor table, the
  accepted-cost register, the ceremony summary, light-client obligations, and
  `wire-format.md`, where seeds are recorded as **never appearing on the wire**.

- **2026-08-22 (keystream: legal position propagated)** — §7.1.5.2 was propagated
  to eleven sites and **missed §7.1.2, the biometric-custody warning**, which is
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
  §7.1.5.1**: keystream encryption *weakens* the breach-surface argument for short
  retention rather than strengthening it, since a store of unopenable ciphertext has
  little breach surface to reduce.

- **2026-08-22 (retention becomes subject-enforced)** — A consequence of §7.1.5.2
  that reverses who holds the policy: **a subject enforces their own retention
  horizon by declining to supply the keystream.** No detection, no cooperation,
  nothing to audit — the holder's copy simply stays inert. §7.1.5.1's window was a
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
  set. **The same reservation as everywhere in §7.1.5.2**: none of it binds a
  non-compliant client, which is unaffected by a refusal. What changes is that a
  *compliant* holder now has no way to defeat the subject's decision, where before
  it had only an obligation not to. Also noted: this **strengthens** §7.1.5.1's
  Schelling-point argument while weakening its breach-surface one.

- **2026-08-22 (template prefix; seeds; open questions; style)** — **The derived
  template is a fixed-length record at the head of the encrypted store**, images
  after (§7.1.5.2). Because it sits at a known offset, **the subject controls which
  retention tier a counterparty gets by choosing how much keystream to send**: full
  length opens template and images, template length opens the template alone, none
  opens nothing. §7.1.5.1's two-year and five-year windows stop being commitments a
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

- **2026-08-22 (veto withdrawn; challenge window withdrawn)** — Both fail §1.1's
  test, and neither had been run through it. **A patron shares no state with a
  subordinate's client**: nothing prevents that client forming, signing,
  replicating or acting on a transaction, and nothing makes it wait. An adoption is
  signed by node and patron, a peering by two infra nodes, so a third party
  asserting a veto over either has no mechanism to exercise it — and a challenge
  window that delays nothing the actor controls delays nothing at all. **Removed**:
  the delegated-veto object (`wire-format.md` §5.5), its four open questions, its
  expiry parameter, the down-line threshold veto, and the challenge window.
  **N3's blocked privacy analysis is closed by withdrawal** rather than by being
  answered. **What replaces it is what was always real**: a compliant client
  notifies the patron of high-value actions; the patron refuses resource access,
  which *is* enforceable because node and requester share the state the decision
  turns on; and the patron disavows. **Rejection produces a fork, not a block** —
  a refused sub-subordinate exists in both readings, which is §4.1.1's membership
  plurality reached through a different door. §7.4.2's argument that one keypair
  suffices is rewritten: not because anything can be blocked, but because a thief's
  natural move destroys most of what they stole, and a second credential would add
  a key to steal without adding a power anyone can exercise. **The invariant is
  kept** in `wire-format.md` §5.5, now describing a power that does not exist rather
  than one to be constrained, so that reintroducing any blocking mechanism has to
  argue against it.

- **2026-08-22 (grandpatron subtree acknowledgement)** — New §9.2.1 and
  `wire-format.md` §5.4a. **Adoption puts a node inside its grandpatron's Dunbar
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

- **2026-08-22 (subtree acknowledgement: default and enforceability)** — Two
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

- **2026-08-22 (open items consolidated)** — Everything currently open is now at
  **§18.2**, classified by what it blocks, so a reviewer need not reassemble it from
  six registers. **One item blocks interoperation**: the unpinned ML-DSA
  `COSE_Key` encoding, where two representations of one key are two identities.
  Four block a subsystem rather than a first node. The rest are decided during
  implementation or deferred by decision. Housekeeping alongside it: three entries
  marked RESOLVED were still sitting in `wire-format.md`'s open list, which
  overstated it at eight when four are live; the unset-parameter count was
  **twenty-one against twenty rows** after the veto-expiry removal; and §17's item 5
  still cross-referenced "item 10" from before that list was renumbered. §17 also
  now says why two deferred-by-decision entries are kept there rather than in the
  appendix — a standing choice that could be revisited reads differently from
  settled history.

- **2026-08-22 (ML-DSA key encoding: not open after all)** — The single item
  recorded as blocking two implementations from interoperating **was never open**.
  RFC 9964 standardised ML-DSA for JOSE and COSE in May 2026, defining an
  **Algorithm Key Pair** key type that pins the representation completely: `kty` = 7,
  `alg` REQUIRED, `pub` REQUIRED at label -1, and `priv` forbidden in a public key.
  `wire-format.md` §2.2 now states those three parameters and the rule that no other
  may appear, matching the classical component's treatment for the same reason —
  **any additional entry changes the encoding and therefore the identity**. The flag
  reflected a failure to check whether a specification existed rather than a gap in
  the standards, and it survived several review passes because a reviewer reading
  `[OPEN]` has no reason to question that something is open. **§18.2 now records
  that nothing blocks interoperation.**

- **2026-08-22 (queue settled; activity summary withdrawn; gateway scoped)** —
  **Queue policy decided**: messages queue **indefinitely at the direct patron**,
  bounded by a per-subordinate storage cap. Retention and size collapse into one
  policy — a space bound declines the trade a time window forces, so a message
  survives an absence of any length and an over-accumulating subordinate hits a
  ceiling rather than a clock. **Siblings hold no queue state**: failover covers
  sessions, not mailboxes, and a client attached to a sibling still collects from
  its own patron on return. That removes the metadata-spreading question entirely.
  **Patron-signed activity summaries withdrawn.** A portable behavioural figure
  asserted by one party and consumed by strangers **is a reputation signal**, which
  §1 refuses and §13.1 replaces with per-observer evaluation — the reader cannot
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

- **2026-08-22 (§18.2 expanded, then refreshed)** — The four subsystem blockers
  were expanded with what is missing, what each blocks, and why it is hard —
  and **three of the four were resolved in the same session**, leaving §18.2
  describing a state two decisions out of date. Refreshed: the queue block now
  records what was settled and lists only the residue (cap value and ceiling
  behaviour, crash-recovery copies, operator logging); the activity-summary and
  gateway blocks are gone; the count reads two rather than four. The **cycle
  prevention** entry gains the reason the interim rule is adequate rather than
  provisional — **an undetected cycle is a correctness problem in one subnet's
  topology, while a false positive refuses a legitimate adoption and is
  indistinguishable from censorship** — so the rule accepts the first to avoid the
  second, and any general procedure must preserve that ordering.
  `wire-format.md` §9's list renumbered after the activity-summary removal left it
  starting at 2, and its queue entry rewritten to the residue.

- **2026-08-22 (decision audit)** — Walked every decision made today against every
  place it should appear. **Four defects, all in the direction of a change reaching
  most sites and missing one.** §9.2.1 described the grandpatron countersignature
  without ever naming the `SubtreeAck` that carries it, so a reader could not find
  the object from the design. **N3 in the privacy register still recorded veto
  delegation as blocked pending its audience** — the entry survived an edit that
  reported success against a string that must have differed, which is the failure
  mode of matching on remembered text rather than read text. The unset-parameter
  count read twenty against nineteen rows after the activity-summary window and
  staleness parameters were removed — **the third time today that count has gone
  stale**, each time because removing a row does not touch the sentence that counts
  them. And `wire-format.md` §9's list was renumbered. Confirmed clean: the
  remaining mentions of `VetoDelegation`, `ActivitySummary` and P7 are inside
  withdrawal notes naming what was removed, which is correct, and the "challenge
  window" at §7.1.6.1 is a timing window in the proximity analysis, unrelated to the
  withdrawn mechanism.

- **2026-08-22 (style regression; companion documents)** — Two corrections to what
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

- **2026-08-22 (protocol completeness stated precisely)** — §18.2 and the status
  block now say that **nothing on the protocol itself is unanswered, with two
  qualifications**: the general cycle-prevention procedure for partial information
  (§6.2.5) and the owner-movement rule for in-flight resource state (§9.8) are
  protocol-shaped and unwritten. Neither blocks writing code or two implementations
  agreeing. Everything else outstanding is a parameter value, a policy choice, or
  product behaviour. **`authoring-conventions.md` gains a rule on cleanup passes**:
  scope by document set rather than by file, report the count rather than a claim of
  completion — since new prose reintroduces a style habit and the design rose from
  332 back to 351 within a day — and for any mechanical substitution, verify the
  applied count and read a sample, because regex on remembered text matches nothing
  silently, line-spanning phrases mangle, and an opened parenthesis will not close
  itself on the next line. All three have occurred.

- **2026-08-22 (0.6 gains a fifth target)** — **Capture and verifier query,
  including keystream handling**, added to the implementation-attempt list and
  flagged as the highest-value target for the next cycle: §7.1.5.2 is the largest
  mechanism in the design that nobody has tried to build. It exercises the keystream
  exchange during a ceremony, seed storage, the template-as-fixed-length-prefix
  layout, tier selection by keystream length, and the parallel channel carrying a
  keystream to a selected verifier. The plan also now records **why the pass finds
  what reading does not** — prose can leave a decision unstated and still read as
  complete, and *"signed by the issuer"* satisfied every reviewer while stopping an
  implementer immediately — and the rule that **a target earns its place by being
  new or having changed**, since all four existing targets found blocking defects on
  their second run only because the wire format had changed underneath them.

- **2026-08-22 (dates reconstructed, second attempt)** — The first reconstruction
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

- **2026-08-22 (statutory detail removed)** — §7.1.2 carried a survey of Illinois
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

- **2026-08-22 (0.1 factual verification, third run)** — **Four contradictions, two
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
  rather than of all available ones, which §15.1 already recorded as an
  uncorroborated comparative.

- **2026-08-22 (0.2 coherence, third run)** — 33 contradictions, 5 blocking, and
  **every blocking one was introduced by an edit made the same day.** `COSE_Key`
  labels are negative while §1 required all map keys to be unsigned — now scoped to
  protocol-defined maps, with standardised structures exempt. §1 said a signature
  covers every retained field while §3 said body-only — now stated as *every
  retained field of the object it signs*, which for a transaction is the body, so
  adding a signer cannot invalidate existing signatures. The transaction table
  listed verifiers among presence-envelope signers, contradicting the signer bound
  and §4.5. `Recovery.responses` was required by the grammar and optional by the
  prose, which made attested rotation unrepresentable. And `list([keyhash])` was
  described as the scope valid **beyond** horizon while §9.2 says nothing reaches
  outside the Dunbar Org — resolved in favour of §9.2, since the owner's node cannot
  evaluate a requester whose topology it does not hold. **Serious**: canonical
  signature ordering affects **envelope bytes**, not the `txid`, which excludes the
  signature array; disavowal codes are an explicit exception to the unknown-enum
  rejection rule, since the banding exists so an unfamiliar code can be acted on;
  design §7.2's illustrative schema gives witnesses and corroborations their own
  signatures where the wire format does not, now flagged in place; queue retention
  and end-to-end encryption were each described as both settled and open. **Minor**:
  eleven signing roles above a table of twelve, the horizon's job count given as
  five, six and seven in three documents, and the requirements documents claiming to
  contain no protocol facts while repeating several — reworded to *states no
  protocol rules of its own*, which is what they mean.

- **2026-08-22 (0.3 unjustified claims, third run)** — 199 distinct unsupported
  propositions, 104 load-bearing, against 30 registered. **§15's curation note is
  updated to the new figure** — it already states that the register is a curated
  subset listing assumptions whose failure changes a *decision*, and the gap remains
  mostly §16's parameters and the wire format's array bounds, which both documents
  declare chosen rather than derived. **Two new assumptions from today's
  mechanisms.** **A29**: custody obligations are a real barrier to hosting for the
  institutions §1.2 names — the product case for the keystream scheme, asserted with
  no operator research behind it, and if false the mechanism still survives on its
  privacy grounds while the product argument does not. **A30**: users and operators
  prefer indefinite retention under a space cap to time-based expiry, which is the
  premise the queue decision rests on, and some operators may well prefer a hard
  time limit for exactly the privacy reason the space bound gives up. Eleven
  intensifiers replaced with the figure or mechanism they stood in for.

- **2026-08-22 (queue rationale corrected; invented business cases removed)** —
  **The stated reason for the queue policy was wrong.** It read as a claim about
  what users prefer; the actual reasons are that light clients may connect rarely,
  and — the stronger one — **an expired verification query damages its subject
  rather than its sender.** §7.1.4 counts `unavailable` toward finalization, so a
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

- **2026-08-22 (convention: whose reasoning is in the document)** — The rule against
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

- **2026-08-22 (activity-summary residue)** — The withdrawal removed the wire
  object, the register entries and the parameters, and left two references behind.
  **§13.2's argument for keeping the policy descriptor cited activity summaries as
  an example of a mechanism reporting facts rather than evaluations** — now
  corrected to note that this was the one mechanism which *would* have published one
  party's figures for another to rely on, and that it was withdrawn for the same
  reason the descriptor carries bad news only. §17 item 2 still listed them as a new
  thing patrons publish needing a propagation rule. A sweep confirms every other
  mention of a withdrawn mechanism sits inside a withdrawal note or §0's naming
  example.

- **2026-08-22 (style-pass regressions repaired)** — A regression check found three
  classes of damage from the em-dash reduction, none of them from the edits that
  prompted the check. **Ten sentences began lowercase**: the substitution turned
  *"X — it is Y"* into *"X. it is Y"*, and the capitalisation pass that followed
  required at least two following letters, so every sentence starting with *it* was
  skipped. **Three status markers lost their punctuation** — *"RESOLVED see §7"*
  where the dash had been removed and nothing replaced it. And one section range
  became `§10–8` when only its first half remapped. All repaired, and the
  capitalisation rule corrected to handle two-letter words. The `§§1–13` range and
  the quote-spanning-line-break hits are legitimate and were left alone.

- **2026-08-22 (0.4 parameter inventory, third run)** — **No numeric disagreement
  among live protocol values.** The four conflicts found are one failure: **§16.1's
  unset list had fallen behind three decisions.** It still listed the
  challenge-window delay and the down-line revocation threshold, both withdrawn when
  the veto mechanism was — a patron shares no state with a subordinate's client and
  can block nothing; queue retention, settled as indefinite at the direct patron;
  and peering replication units, which `wire-format.md` §4.4 defines as bytes. All
  four removed, the queue entry narrowed to its actual residue, and the count
  corrected from nineteen to sixteen. **§16.1.1's hardening table emptied as a
  result**, since three of its four entries were the withdrawn parameters and the
  fourth was already resolved — rewritten so the rule survives without stale
  instances, and noting plainly that **no unset parameter is currently in that
  class because the mechanisms were withdrawn, not because the hazard was
  addressed.** Everything else the inventory flagged is already marked historical in
  the documents: presence-record size, geohash dimensions, the infra threshold, the
  anchor guideline, the signer bound, the verifier-response array, and timestamp
  serialization.

- **2026-08-22 (§14 and §15 registers audited)** — Prompted by the observation that
  §14 reads as full of unfamiliar open questions. **The cause is mostly ordering**:
  entries had been prepended as they were added, so the privacy register ran
  P1–P12, P18, P19, P20, P26, P27, P30, P31, N3, P29, P28, P25… and the assumption
  register was similarly scrambled. All three registers — privacy findings,
  correlations and assumptions — are now in numeric order, which is how a reader
  checks whether something is already known. **§14.5.3's selective-disclosure
  proposal is marked as what it is**: proposed by the drafter, never put to the
  author, and not adopted. It is retained because §14.5.1's composition argument
  keeps arriving at it and because leaving the record all-or-nothing should be a
  decision rather than an omission — but **nothing depends on it**, and P1 and P19
  name it as an available direction rather than a planned one. **P4 corrected**: it
  said the queue was open, where retention and sibling replication are settled and
  the residual is queue metadata.

- **2026-08-22 (§§17–18 audited)** — **§18.3 was a stale duplicate of §18.2.** Both
  enumerated what is open; §18.3's version still said queue retention was
  unresolved, claimed "two encoding decisions" while listing one, cited five
  parameters needing a security argument where none remain, and used `###` headings
  for its own subsections so they appeared as siblings of §18.3 rather than under
  it. Replaced with what only it carried: **test vectors, and what a test suite
  would add** that the implementation passes cannot, since those ask *can this be
  written?* and stub the error paths while a suite asks *what happens when the input
  is wrong?* **§17's five entries are all live.** Indentation normalised, and item 4
  updated — the keystream scheme widens archive-recovery loss, since seeds are
  device state too, so a lost device also loses the ability to unlock one's likeness
  on every counterparty's machine.

- **2026-08-22 (P8 withdrawn)** — *Topology deanonymisation by association* is not a
  distinct vulnerability. **Identifying one member by real name yields their job,
  not a label for any of their subtrees**: §4.1.1's membership plurality means a
  member belongs to several, and nothing in the protocol says which is a workplace
  rather than a bowling team or a congregation. **What labels a subtree is its
  catalog**, which is P15 and is a designed feature working as intended — so the
  mechanism runs the opposite way from P8's claim: you do not deanonymise a subtree
  by identifying a member, you learn what a subtree is for by reading its catalog,
  and its members inherit that. The attacker P8 described also **holds both the
  topology and a real-name link, which makes them a horizon member**, and horizon
  membership already carries a known package of disclosures (§1.2). Folded into P15.
  **§14.5.4 now states the general test**: a finding must add something membership
  does not already carry, and any finding whose precondition is *an attacker inside
  the horizon* should be checked against that before being entered.

- **2026-08-22 (withdrawn findings get tombstones)** — Removing P7 and P8 left two
  silent gaps in the privacy register, which reads as an error to anyone auditing
  it. Both now keep a struck-through row recording what they were and why they went.
  **§14.5.4 states the rule: numbers are not reused.** Renumbering would be worse
  than a gap — a dangling citation is visible, whereas a reused number makes an old
  citation resolve to a *different* finding, silently. The assumption and
  correlation registers were checked and have no gaps.

- **2026-08-22 (P9 withdrawn)** — *Divergence-notification fan-out* is not a privacy
  cost. **The exposure it named predates the recovery**: a thief holding the key
  already reads everything addressed to it, so an inquirer's loss dates from the
  theft rather than from the notification, and recovery only lets the legitimate
  holder regain one subnet from a position of none. **The disclosure also runs in
  the design's favour** — a third party who sees a rotation and doubts the
  un-rotated binding in another subnet devalues the memberships the thief still
  holds, which is the only mechanism here that damages a thief across subnets they
  retain, and it works because the fork is visible. §7.4.0.2 now records that
  reasoning where the mechanism is specified. **§14.5.4 gains a second test**: a
  finding must name a loss the design *causes* — check when the harm occurs, not
  only whether the mechanism touches it.

- **2026-08-22 (P10 closed, P14 accepted)** — **P10 is closed by the policy
  descriptor's redesign.** It described fingerprinting a descriptor's published
  parameters; §13.2 now publishes none — bad news only, silence meaning nothing — so
  there is nothing to fingerprint, and the proposed mitigation of bucketing raw
  parameters refers to values that no longer exist. **P14 moves from open to
  accepted.** Chain back-pointers reveal a subject's chain head, so a counterparty
  meeting them twice sees how far it advanced — but that discloses nothing past
  §7.1.4's finalization threshold, where *n* is the subject's presence count and
  **the evaluator learns it from the subject by design**. Activity level is already
  an input every evaluator receives; a chain head is a coarser view of the same fact
  given to someone who has met them. **§14.5.4 gains a third test**: a finding whose
  mechanism is withdrawn is closed rather than stale, since leaving it makes the
  register describe a system nobody is building. The register now holds 27 live
  findings and 4 tombstones.

- **2026-08-22 (C11 updated to its residual)** — The entry still described the
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

- **2026-08-22 (query log becomes an ephemeral lock)** — **The anti-oracle defence
  is rate limiting, and rate limiting needs a lock rather than a log.** §7.1.4 now
  states that what a subject holds is a counter per requester and per ceremony
  window, kept **only for the enforced ceremony duration** — a few minutes — and
  that it exists to refuse the next query rather than to record that a previous one
  happened. A durable queryable history was a separate feature nobody required, and
  an expensive one: it is an auxiliary timeline of ceremony attempts **including
  those abandoned before any record existed**, sitting on a device that can be
  seized. **C12 and P22 both close.** C12's correlation needed a retained history to
  join against the archive and there is none; P22 asked for a retention rule and the
  answer is that nothing is retained. The parameter is dropped from §16.1, leaving
  fifteen unset. As with every client-side rule, a non-conforming client may retain
  more and nothing detects it.

- **2026-08-22 (fourth register test: ask whether the component is required)** —
  §14.5.4 gains a test in the form of a question rather than a judgement: **when a
  finding assumes a component, ask whether the component is required** — not whether
  it can be made safe, which takes the component as given and generates work. C12
  and P22 both reasoned about protecting a verification-query log that was not
  needed, and **a finding can entrench the thing it is about** while the register
  looks like it is doing its job. `authoring-conventions.md` records the
  corresponding instruction: **the drafter is poorly placed to make this call**,
  since whether a component is load-bearing depends on intent that is frequently
  unwritten, so the judgement belongs with the author and the question costs one
  exchange.

- **2026-08-22 (C13 withdrawn, C16 distinguished)** — **C13 fails on two counts.**
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

- **2026-08-22 (C16 withdrawn; P27 narrowed)** — The distinction drawn between C16
  and C13 an hour earlier **was wrong**. An abuse report goes from a resource to its
  own owner, and the owner granted the role in the first place, so they are not
  receiving particulars about a stranger. **The schema settles it more firmly**: an
  `AbuseReport` carries `resource` and `reporter` and has **no subject field**, so a
  detail describing someone's circumstances has nobody to describe — either the
  resource reports to its owner, both being that party's own assets, or a user
  reports about a resource and any personal particular is their own. **P27 survives
  in narrowed form**: not a disclosure to the recipient, but §9.6 states that a
  reporter may hand the signed object to anyone, and **a signature makes forwarded
  particulars credible in a way an unsigned account would not be.** The wire
  format's rationale for the 1 KB bound is corrected to that reason, since it
  previously cited the withdrawn correlation.

- **2026-08-22 (C14 narrowed)** — The capability vector is exchanged in `Attach`, so
  the party who sees it is the **serving infra node**, which §11.1.2 says need not be
  the patron — and that split removes most of the finding. **A patron or resource
  owner is excluded**: they participated in both adoptions and hold the link
  already, by the recovery record if it was a rotation and by having met the person
  if it was a fresh Genesis identity, since adoption requires a ceremony. What
  remains is a serving node that **serves without having adopted**, seeing attach
  traffic from two identities on one device — and there **the capability vector is
  the weakest of the three signals**, since the network point is the fingerprint.
  This is ordinary network-level linkability rather than anything the capability
  mechanism introduces, and greasing was never claimed to address it. Severity
  Medium to Low. §10.8.7 now states the same thing where the fresh-identity path is
  described: **a fresh identity is unlinkable in the record, not on the wire.**

- **2026-08-22 (C14 withdrawn)** — A serving infra node is necessarily inside the
  Dunbar org, and a rotation propagates as a topology-class message **pushed within
  horizon** (§7.4.0.2) — so **any node positioned to see both attaches has already
  received the record binding the two keys**, and a device fingerprint adds nothing.
  The fresh-Genesis case fails from the other side: where unlinkability matters the
  new identity appears in a *different* subnet under a different serving node that
  sees only one, and where a single node could see both, the person was adopted by a
  neighbour who met them, so the link exists socially whatever the transport shows.
  **§10.8.7 restated as a result**: unlinkability is a property of **where you
  reappear**, not of what you avoid signing — appearing fresh in the same
  neighbourhood is not unlinkable at all, and the operation is meaningful only where
  nobody knows the old identity, which is what §10.4's absence of cold lookup
  protects.

- **2026-08-22 (C15 accepted)** — Pairwise identifiers address cross-*operator*
  linkage; one vendor running several resources correlates them from account,
  device and network data it holds anyway, and no identifier scheme changes that.
  **Recorded as an accepted cost rather than an open correlation** (§14.5.7 item
  10): resources differ in what anonymity they offer, and **which ones a subnet
  admits is part of how it sets its security posture** — a user more
  security-conscious than their organisation may decline a service the organisation
  accepts. That is the same remedy §9.7 gives for gateways: the network binds a
  service to what its owner published, and beyond that the choice is the user's.
  §9.0.2's scope note aligned with the same framing.

- **2026-08-22 (C17 and P28 narrowed)** — Both are largely obviated by §7.1.5.2. A
  compliant client holds captures as ciphertext under the *subject's* keystream, so
  retained EXIF and background are unreadable, and **a non-compliant client keeping
  plaintext is the baseline** — the same bad actor with an ordinary camera app gets
  the same thing, which is the test §1.2.2 sets. Severity from High to Low. **The
  residual keeps the stripping obligation alive**: a compliant holder decrypts
  legitimately during a later verification and has the plaintext in hand for that
  window, which is the one moment the keystream does not cover. Recorded in the
  light-client requirement so the obligation does not read as redundant now that
  encryption exists.

- **2026-08-22 (C18 withdrawn; sandboxing restated)** — The correlation assumed a
  hosted package could reach topology, liveness, queue state, prekey requests and
  role-evaluation inputs. **The design offers no binding that exposes any of them**:
  §9's rule that *the resource never reads network state* applies to a hosted
  package as much as an external one, and `infra-client-requirements.md` §8.2 now
  says the hooks **do not exist** rather than that they should be scoped narrowly.
  Offering a binding and scoping it carefully would still be offering it, and the
  narrow scope would become a policy an operator could widen. **What remains is an
  implementation question, not a correlation**: whether an isolation mechanism
  enforces the boundary against hostile code. §7.2 now says plainly that this is for
  the sandboxing literature rather than a claim the document makes, and that an
  implementer should treat WASM component isolation as an open engineering question.
  P31 narrowed to the residual — installed code runs inside the boundary the threat
  model draws around operator conduct.

- **2026-08-22 (0.5 rule fragility, fourth run)** — Two residual rules, and the
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

- **2026-08-22 (0.5.2 enforcement boundary, fourth run — CLEAN)** — **No findings.**
  The first pass in the programme to return none. Six apparent mandates were checked
  and each survives on the §1.1 test: §7.1.4's verifier rule is enforced by the
  verifier itself, which receives the proof in the query; §9.5's disclosure rule
  rests on `discover_scope` travelling in signed catalog evidence the recipient can
  read; §9.6's abuse-report rule is scoped to what the sender controls and refuses to
  bind any holder; `wire-format.md` §4.2.1 governs a client's treatment of its own
  user's move from topology it already holds; §6.1.1's greasing obligation is
  checkable by any peer that greases; and `resource-requirements.md` §3.1's header
  rules bind an intermediary over traffic it originates. **The reviewer also
  confirmed the three requirements documents avoid claiming their local obligations
  are protocol-enforceable**, which is the framing added in the second run.
  Recorded as one reviewer finding nothing rather than as proof none exists — but
  the corrections from earlier runs have held, including the abuse-report rule that
  the register keeps as the worked example of the failure.

- **2026-08-22 (0.6 adoption, third run)** — Ten encoding questions, and **two would
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

- **2026-08-22 (resolution: iterative with referrals)** — A 0.6 implementation
  attempt found **a direct contradiction between the two documents**: §10.6.1
  described resolution descending through infra nodes, while `wire-format.md` §5.6
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

- **2026-08-22 (referral model propagated)** — Consistency sweep after the
  resolution change. **Three places still described the superseded model.** §10.3
  Case 1 had Bob hand his message to his patron, which forwards toward the anchor
  and descends — message routing rather than resolution, and it contradicted
  §10.6.3's rule that infra nodes carry no payload but their own clients' — now
  rewritten as query, refer, cache, then contact the serving node directly.
  **§10.6.2 was titled *repaired in transit***, which assumes messages travel
  through nodes; repair now happens during resolution, and the requester's cache
  absorbs the correction for later contacts. And a claim about invalidation cost
  said repair degrades a bad case to *extra hops*, now *extra round trips*.
  **`infra-client-requirements.md` had no resolution obligations at all** despite
  being the party that answers and refers — new §4 covers answer, refer, repair,
  report failure, the multi-index shortcut, answering from one's own children, and
  the process-and-discard obligation on what a request discloses. Sections
  renumbered and all inbound references updated.

- **2026-08-22 (infra resolution obligations, properly)** — The §4 added in the
  previous sweep covered *answering* and almost nothing about the state required to
  answer, and it **contained a contradiction**: "answer from your own children"
  against "you may refer past several indices where you know your own subtree."
  Resolved by making deeper caching an **optimisation above the floor** — the
  constant-state guarantee bounds what a node must keep, not what it may. §4 now
  specifies the **child table**, forwarding records and their 90-day TTL, the
  **anchor table's ingestion boundary** with the requirement to state which model is
  implemented, and maintenance: remove on departure or disavowal, replace endpoints
  on a strictly greater `seqno`, collapse forwarding chains at the source.
  **A real gap surfaced**: nothing delivers an infra child's endpoints to its
  patron. A light client's arrive at attach; an infra child serves itself and never
  attaches — so **a patron cannot refer to a node whose address it does not hold**,
  which blocks resolution past one hop. `SignedLocator` is the natural carrier and
  no propagation rule for it exists. Recorded in §18.2 as a subsystem blocker rather
  than an unset parameter, since what is missing is a mechanism.

- **2026-08-22 (consistency check after the referral change)** — All references
  resolve in every direction across the five documents; assumption, correlation and
  privacy registers have no gaps; parameter count matches its claim; no `[P]` or
  `[OPEN]` markers remain. **Two claims had gone stale.** §18.2 said *three items*
  above four subheadings — correct, since the fourth is the resource residue which
  blocks nothing, but it read as an error and now says so explicitly. And **the
  protocol-completeness claim still said two qualifications** when the locator
  propagation gap makes three — recorded in both §18.2 and the status block, with
  the note that this one **does** block a subsystem, unlike cycle prevention and
  owner movement.

- **2026-08-22 (0.1 factual verification, fourth run)** — **One contradiction.**
  §18.1 said a native WASM runtime under WASI "has ordinary filesystem access";
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

- **2026-08-22 (0.2 coherence, fourth run)** — 15 contradictions, 2 blocking, and
  **both blocking ones came from fixes made earlier in this session.** The header
  rule was generalised to *every COSE object carries `alg` and `kid`* while the
  exception it contradicts — embedded `COSE_Sign1` omits `kid` — was left standing
  four lines below. Resolved by stating the rule as a condition rather than a list:
  **`kid` is present exactly when the surrounding structure does not already name
  the signer**, so envelope entries carry it and embedded objects do not, since a
  `VerifierResponse` names its verifier in a field and a second copy could disagree.
  **Late verifier responses were said in three places to have a present encoding
  and no such object existed** — now `LateResponse` (`wire-format.md` §5.3a), a
  standalone signed object referring to the record it supplements, which explicitly
  **does not amend it**: the record is immutable, finalization already happened, and
  a late response is evidence an evaluator may weigh rather than a change to what
  was decided. **Thirteen serious**, most being today's decisions not reaching every
  mention: prekey exhaustion **degrades forward secrecy rather than blocking
  messaging**; the presence signer ceiling is 18 and the dual-role rule was implying
  34; the disavowal notice period referenced the withdrawn challenge window; the
  anchor-table sizing omitted the self-signature it now carries; and the security
  analysis still said relays see plaintext.

- **2026-08-22 (0.3 unjustified claims, fourth run)** — 123 propositions, 79
  load-bearing, against 31 registered. **The count has converged** — the previous run
  found 104 load-bearing on a stricter split, and the gap is still mostly §16's
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

- **2026-08-22 (0.4 parameter inventory, fourth run)** — **No unresolved numeric
  conflict**, the second parameter pass running to come back clean on values. The
  reviewer also confirmed that the conflicts the documents record as *resolved* —
  the presence-record size, the signer bound, the verifier-response array, the
  geohash figures — read as history rather than as live disagreements. **One live
  status inconsistency**, and both halves of it were in the same place: §16.1.1's
  *freely tunable, forever* list still contained **query-log retention**, which has
  no subject since there is no durable query log, and **queue retention**, which is
  settled as indefinite and therefore not tunable at all. Removed rather than
  reclassified, with the queue's **per-subordinate storage cap** left in its place
  as the quantity a node does choose. Listing a settled value among the freely
  tunable ones invites an operator to change it.

- **2026-08-22 (0.5.1 rule fragility, fifth run — CLEAN)** — **No residual
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

- **2026-08-22 (0.5.2 enforcement boundary, fifth run)** — Two findings, and the
  reviewer's framing of the first is the useful part: **§9.6 had already made the
  move §9.5 did not.** The catalog rule said a relaying party **must** disclose an
  entry only to authorised recipients — and once a relay holds the entry, the owner
  has no state showing whom it copied it to. Restated as an obligation on the
  **publisher**: carry the intended discovery audience inside the signed object so
  any recipient can tell whether it was meant for them. **The recipient can
  recognise a leak; the owner can neither prevent nor learn of one**, and
  `discover_scope` filtering is now stated as conforming-relay behaviour rather than
  a property the owner enforces. **Second finding, same defect in declarative
  dress**: §7.4.0.2's fork detection said an inquirer who sees divergent assertions
  *notifies both patrons* — an arbitrary third party sharing state with neither. A
  patron receiving no notice cannot distinguish *no divergence was observed* from
  *an inquirer observed it and said nothing*. Now marked as conforming behaviour
  with the gap stated, and **§17 records the durable alternative**: an
  inquirer-signed divergence notice makes the *observation* an object that stands
  alone, which is evidence rather than assumed behaviour.

- **2026-08-22 (0.6 adoption, fourth run)** — Five questions, **three of them
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

- **2026-08-22 (0.6 resolution, third run)** — Twelve questions, **four of them one
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
  and §11.1.2 already said so**: a light client attaches to the nearest
  infrastructure on its patron chain, walking up past light-client patrons, so a
  serving node holds its **entire light-client subtree** and resolves those paths
  itself. Intermediate light-client patrons adopt and countersign but carry no
  traffic. **Referral is therefore only ever between infra nodes** — now stated in
  §10.6.1 and the infra requirements, where it was implied by the attach rule and
  never drawn out. Also settled: first contact with an unpinned anchor is
  trust-on-first-use over an **unauthenticated** peer, so it must disclose nothing
  beyond the query; a locator whose anchor is absent locally is a caller-side
  condition rather than a wire failure; and **equal `seqno` with different contents
  is malformed**, not a tie to break.

- **2026-08-23 (0.6 presence validation, third run)** — Six questions, and **one is
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

- **2026-08-23 (0.6 client attach, third run)** — Fifteen questions, most of them
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

- **2026-08-23 (0.6 ceremony and verifier query, first run — construction
  respecified)** — 27 questions, the most of any target, on the mechanism nobody had
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

- **2026-08-23 (0.6 targets 6 and 7 added)** — Two implementation targets covering
  the resource layer, which **no attempt has touched** despite being roughly a third
  of the design and the part §1.2 names as the network's purpose. **Target 6,
  register a resource and propagate its catalog entry**: `CatalogEntry`
  construction and owner signature, scope predicate encoding, propagation within
  horizon, and conforming-relay filtering — with the pressure on the scope language,
  since predicates are evaluated against the recipient's own topology and §9.5 now
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

- **2026-08-23 (review-plan references remapped)** — Adding the resource targets
  surfaced **ten stale section references in `review-plan.md`**, predating several
  renumbers — it had never been included in the cross-document reference sweep,
  because the sweep covers the five specification documents and the plan is not one
  of them. Remapped. **`review-tracking.md` has 52 of the same, and they stay**: it
  is a historical record of what each pass found and how it was disposed of, so its
  references are correct as of filing, and rewriting them would falsify the record.
  A note now says so.

- **2026-08-23 (0.6 ceremony, second run)** — 25 questions against the respecified
  construction, and **one is a circularity introduced by this morning's fix.** Key
  derivation bound `txid` — which hashes the finalized body **including verifier
  responses**, while sealing happens at capture, before either exists. **Rebound to
  the ceremony pre-commitment**: fixed before capture, countersigned by both parties
  and the witnesses, unique per ceremony — everything the binding needed, available
  when the binding is made. **The key grant had no message**, the same gap class as
  the `CurrencyAttestation` signature: three implementations would have invented
  three. Now `KeyGrant` (`wire-format.md` §5.3a), carried as end-to-end encrypted
  payload and **never retained in a record**, since a persisted grant defeats the
  retention property the scheme exists for. It names the record to open — a verifier
  met several times holds several sealed captures — and the query it answers, so an
  unattached grant is malformed rather than an invitation. Also settled: **a ceremony
  seals a new capture rather than replacing an earlier one**, each ageing
  independently; and a decryption or authentication failure is **`inconclusive`,
  never `no-match`** — reporting it as no-match would turn a corrupted store into
  adverse evidence about its subject.

- **2026-08-23 (0.6 hosted resource authorisation, first run)** — The target added
  to cover the resource layer found what it was added to find. **The authorisation
  rule is implementable** — membership gate, subtree acknowledgement, predicate
  evaluation, pairwise derivation, credential construction all work — and **the
  request cannot be carried**, because frame types 5 and 6 were named and neither
  body was ever defined. **A direct contradiction alongside it**: §6.0 put resource
  frames on stream 0 while §7.2 assigns request/response to bidirectional streams.
  Resolved toward §7.2 — resource traffic is not session control — with frames 5 and
  6 withdrawn from the control table and `ResourceRequest`/`ResourceResponse` drafted
  at `wire-format.md` §7.3. Two rules fell out of writing them: **the node's status
  codes are not the resource's**, so an application error returns inside a delivered
  response rather than looking like a gateway refusal; and **`not authorised` does
  not distinguish absent from refused**, because distinguishing them tells a
  stranger about the owner's membership and policy. **A topology gap is recorded
  open**: resource eligibility spans the owner's Dunbar Org while attachment is to
  one's own serving node, so an eligible requester two tiers away has no specified
  path to the host. **The completeness claims are corrected** — the identity,
  presence, routing and messaging layers are specified and the resource layer is
  not, where §18.2 had said nothing blocks interoperation.

- **2026-08-23 (refusal reasons split by horizon)** — The uniform `not authorised`
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

- **2026-08-23 (refusal evaluation order made normative)** — Splitting refusal
  reasons left the codes unordered, which conflated the two conditions the split was
  for: **a member with no role and a member whose service is down**. Order is now
  normative — membership, existence, acknowledgement, roles, **then availability** —
  and the placement of availability last is what does the work. **A role-holder
  learns the service is down; someone with no role never does**, so operational
  information about the owner stays inside the set entitled to it. Running
  availability earlier would answer *unavailable* to someone who has no role, which
  is true and useless: they would retry indefinitely against a resource they could
  never reach.

- **2026-08-23 (role assignment is a materialised table)** — The predicate language
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

- **2026-08-23 (role visibility corrected)** — A first attempt said *what the
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

- **2026-08-23 (portability scoped to legibility)** — *A package must run on any
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

- **2026-08-23 (0.6 catalog registration, first run)** — **The deepest finding is
  not a catalog problem.** §12 names *flood-within-horizon* and *push near, redirect
  far*, and those patterns are relied on by rotation, catalog entries, peering and
  the locator gap — while **no message carries any of them**. Nothing says what is
  sent, on which stream, how a receiver decides to forward, or how a flood
  terminates in a horizon that contains cycles once peering exists. Recorded at
  `wire-format.md` §7.2a and as a §18.2 blocker, with the diagnosis: **§12 settles
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

- **2026-08-23 (catalog is answered, not propagated)** — The catalog is a **query
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
  gap. And §9.5's enforcement problem dissolves rather than being solved: with no
  relay, **the owner is the only party that ever discloses an entry**.

- **2026-08-23 (0.6 catalog registration, second run)** — Eighteen questions, and
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
  transaction is missing. **And the §7.3.1 topology gap is worse than recorded**: a
  resource request needs a session with one non-attached node, a catalog query needs
  one with every infra node in the asker's horizon.

- **2026-08-23 (catalog view is swept and cached)** — The concurrent-session problem
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

- **2026-08-23 (0.6 catalog registration, third run)** — Nineteen questions, **two
  of them contradictions from my own edits.** `resource-requirements.md` §6 still
  said entries *propagate as topology* after the catalog became query-answered, and
  `wire-format.md` §3.2 said a nonempty unprotected COSE header was both **malformed
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

- **2026-08-23 (host loss and resource failover)** — Loss of a hosting node is **not
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

- **2026-08-23 (§7.3.1 withdrawn — not a gap)** — The "topology gap" was a
  misreading of what attachment is for. **Messaging already opens sessions to
  non-attached nodes**: §10.6.3's relayed path runs client → own serving node →
  **recipient's serving node** → recipient, and its direct path has a client reach a
  peer outside its own subtree. Infra nodes hold static addresses and authenticate by
  keyhash, so opening a session to one is the same operation wherever it sits.
  **Attachment answers one question — where do my messages queue — and is singular
  because a mailbox must have one address.** It says nothing about which nodes a
  client may connect to, and §11.1.2 now says so explicitly. Both halves of the
  supposed tension were true and never in conflict. §18.2's subsystem blockers drop
  from five to four.

- **2026-08-23 (§7.3.1 closed, not answered)** — Reaching an infra node you are not
  attached to **was never a gap**. §10.6.3 already specifies direct connection to any
  peer inside the horizon — bounded, and privacy-analysed under P17 — and **a hosting
  node is a horizon peer**, so a resource request is that operation with an infra node
  as the peer instead of a light client. A catalog sweep is the same, one node at a
  time. **Attachment is about inbound service** — queue, currency, sibling list — and
  never was a restriction on outbound connections; reading it as one is what made this
  look unresolved. The privacy position is unchanged, since IP disclosure to a horizon
  peer is incremental rather than novel, which is the argument §10.6.3 makes for
  drawing the boundary there. §18.2's resource entry rewritten to separate what three
  runs settled from what remains: the `connect`/`discover` relationship to roles, and
  a second attempt against the frames as drafted.

- **2026-08-23 (reserved actions are node-consumed)** — `discover` and `connect` are
  listed in §9.4 as *actions* alongside application actions like `read` and `write`,
  and the credential passes roles to the resource — so an implementer reading it
  straight forwards them, and a resource ends up holding a role it cannot act on.
  **Neither is ever forwarded.** Both are consumed by the owner's node when it
  evaluates its role table, and a resource's role set carries **application actions
  only**. The reason is that neither is actionable downstream: `discover` is
  evaluated when composing a catalog answer, before any request exists, and **a
  request arriving at the resource is what a `connect` grant looks like** — restating
  it would tell the resource what the delivery already told it. Recorded in §9.4 and
  in the resource requirements, where a package is told not to expect them and not to
  read their absence as a missing grant. **§18.2's resource entry now has one item
  left**: a second implementation attempt against the frames as drafted.

- **2026-08-23 (0.6 target list reduced to prompt text)** — The seven targets carried
  commentary on what each exercises and where the pressure lies, which was written
  for a reader rather than for the prompt it is pasted into — and had gone stale
  besides: target 6 still said *propagate its catalog entry* and named
  `discover_scope`, both withdrawn. Each target is now a single sentence naming the
  operation. The rationale that was embedded in them moves nowhere, since the passes
  it described have run; the coverage note is corrected to say that **a layer nobody
  has built against produces a construction change rather than encoding corrections**
  — which targets 5, 6 and 7 each did on their first run, and each needed a second.

- **2026-08-23 (0.6 catalog registration, fourth run)** — Eleven questions, and
  **the propagation contradiction appeared for a third time**: §12's message-class
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

- **2026-08-23 (catalog bounds completed)** — Two bounds from the fourth run were
  not applied with the rest. `CatalogQuery`'s service-type filter had no bound and
  now carries the same one as the field it matches — **a filter longer than any legal
  type cannot match and should not be allocated for**. And **unknown extension keys
  had no bound anywhere**: §1 requires them to survive re-serialisation, which makes
  them attacker-supplied storage on a signed object a node retains. Sixteen per map,
  1 KB per value, with a total encoded bound on `CatalogEntry`. **Extension tolerance
  is not unbounded tolerance**, and a rule admitting arbitrary bytes into retained
  state is a denial-of-service surface however well-intentioned.

- **2026-08-23 (0.6 hosted resource authorisation, second run)** — Eighteen
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

- **2026-08-23 (multi-identity reframed as client scope)** — *v1's single identity
  key across subnets* was recorded as an architectural limitation and kept surfacing
  in privacy sweeps as one. **It is not.** Nothing in the wire format or topology
  binds a device to one key — an identity *is* a key, adoptions are per-identity, and
  a client holding several is running several identities as far as the network can
  tell. **What v1 defers is the client work**: key management and the interface for
  choosing between them. §2 already listed it as a client-scope exclusion; §10.8.7
  already said it was a client concern; and the registers had nonetheless inherited
  the reading that a protocol change was needed. **P3 and C10 are now stated as
  conditional on the client**: high for a single-identity client, absent for a
  multi-identity one, **with no wire change separating the two cases** — so P3 no
  longer ships as a known defect, because the design does not have the defect.

- **2026-08-23 (0.7 LINDDUN, fourth run)** — **One live contradiction**: §7.1.4 says
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

- **2026-08-23 (policy descriptor removed entirely)** — **Reverses an earlier
  decision to keep it as a declared limitation.** The intermediate form carried *bad
  news only* — a node may declare its policy fails the soundness condition, silence
  meaning nothing — on the reasoning that a deliberately crippled mechanism documents
  its own reasoning by existing. **It was still generating attack surface in review**,
  with adversaries reasoning about shopping across published policies, for
  documentation value an Appendix A entry provides at no cost. Removed. §13.2 now
  states plainly that **nothing publishes a policy and nothing should**, since
  per-observer trust has no consumer for one. **§13.4's shopping attack survives and
  is restated**: an attacker cannot *read* a policy, but can **try** — present the
  fake region and see who accepts it — which is slower and noisier than reading a
  declaration, is the reason no declaration exists, and is not prevented. The
  durable part is the asymmetry: the attacker chooses how many evaluators to
  approach, and no evaluator chooses which attacker approaches them. P10's tombstone
  now reads *closed by removal* rather than *by redesign*.

- **2026-08-23 (0.8 restored verbatim)** — The section had been rewritten to run all
  nine adversaries in one pass with a complexity-warning preamble. A later run in
  that form was blocked by the provider's cybersecurity classifier. **The section is
  restored exactly as it was**, including its section numbers, which predate several
  renumbers and no longer resolve against the current documents. **That is
  deliberate**: it is the text that ran, and it is preserved as such. Anything else
  is a change, and changes to a known-good prompt belong in a separate decision.

- **2026-08-23 (0.9 organisation, second run)** — **The reference checker had been
  reporting clean while nineteen references were broken.** Three failure modes it
  could not see: an exemption for §17.x, added when those citations pointed at
  numbered list items, hid two that pointed at nothing after the list was renumbered;
  **§15.1's assumption table carries addresses as bare numbers in a column**, not
  §-prefixed, so thirteen stale ones — `6.5.1`, `6.5.4`, `7.6.5`, `7.9.1` and others
  predating the §6 split — were never examined; and a duplicated heading,
  `#### 7.2.2 Verifier selection#### 7.2.2 Verifier selection`, parsed as a valid
  heading and so passed. All corrected, and the checker now runs with **no
  exemptions**. **A structural fix alongside them**: the catalog, `CatalogEntry`,
  query and reply, and `AbuseReport` — 1,856 words — sat between `## 4` and
  `### 4.1` under no heading at all, while every reference to them pointed at §5.4,
  which is the withdrawn activity summary. Now §4.7, with references repointed.
  Citation labels normalised: `design design §`, `Design §` and
  `` `infra-client-requirements.md` design § `` all appeared.

- **2026-08-24 (0.8 hostile client implementer)** — The pass ran after the reviewer
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

- **2026-08-24 (0.8 findings 2–5 applied)** — **Formation records no longer count
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
  multi-parser boundary hazard. New §7.3.2 requires parse, reject-don't-normalise,
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

- **2026-08-24 (P34 withdrawn: partitioned identity is the design)** — The
  archive-fork finding treated *n* as a global reputation figure a subject could
  understate. **It is not.** *n* is the history a subject has **in the domain they
  are presenting to**, and a sparse branch yields a low threshold *and*
  correspondingly low standing — the two move together, so understating gains
  nothing. A subject appearing with no history is a stranger, which is what they are
  there. **§13.1 now states the property rather than leaving it implicit**:
  credibility is constructive, built by engaging in a domain, and what is at stake in
  an interaction is what was built there. There is no universal permanent record and
  no cross-domain enforcement. Forking an archive **divides what can be claimed
  rather than hiding it**. Withdrawn from the register and from §18.2's blocker list.

- **2026-08-24 (formation flooding: the real bound, and a framing correction)** —
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
  proves nothing, so manufacturing volume gains nothing.** §13.1 now says so, and
  distinguishes the protocol's sampling floor from the evaluation, which is the
  adopter's and lies outside this specification.

- **2026-08-24 (horizons are scopes, not shared regions)** — §12.1 described the ±2
  tier horizon in terms that read as a shared region, and a reader — including a
  reviewer — naturally infers that parties inside it share a view. **They do not.**
  Each node's horizon is centred on itself, so no two nodes at different positions
  have the same one, and *"inside the horizon"* always means *inside mine*.
  **Siblings are the case that shows the distinction**: they occupy the same
  position, so their scopes coincide exactly, **and their trust pictures still
  differ** because each has its own history with users outside the subtree. Same
  scope, different content. **A node's total trust picture is unique to it**, and
  there is no tree-level trust state for a node's view to be a view *of*.

- **2026-08-24 (consistency pass across all files, working documents included)** —
  Scanned all twelve files for the defect classes that have recurred: doubled
  citation labels, malformed decision markers, malformed ranges, lowercase sentence
  starts from the em-dash substitution, `STATUS see` without punctuation, unclosed
  code fences. **Most hits were false positives** — schema comments, sentence-initial
  *Design*, a malformed marker quoted inside a change-log entry describing its own
  repair. **Four real defects**: a range reading `§14.2–12.3`, a doubled
  `design design §9.2` in the resource requirements, and two sentence-initial
  *Design §* that read as citation labels. **§18.2's trailing note miscounted again**
  after P34's withdrawal removed a heading — the third time that sentence has drifted,
  which is what the note about consolidating sections in `CLAUDE.md` is drawn from.
  All references resolve in every direction; assumption, correlation and privacy
  registers have no gaps at 31, 18 and 34 entries; the unset-parameter count matches
  its claim. Trailing newlines normalised across the set.
- **2026-08-24 (history ingestion, from the original)** — Recovered from the
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
  carrying. §8.1 now says so.
- **2026-08-24 ("local trust table" withdrawn)** — The phrase appeared in the
  original statement of archive ingestion and the author has withdrawn it as
  incorrect. **Trust is node-local, and describing it as a tree's table implies a
  shared structure that does not exist** — a patron compares a presented archive
  against the identities *it* already knows of, and no two nodes hold the same set.
  §12.1 no longer describes a node's trust picture as its view of anything; there is
  nothing for it to be a view of. Corrected in §12.1, §13.1, §8.1 and the infra
  requirements. The change-log quotations of the original message are left as the
  author wrote them.
- **2026-08-25 (topology propagation, the rootward memo, and §18.2 rebuilt)** — The
  largest of the remaining §18.2 blockers closed together, because they turned out to
  share a mechanism.

  **Topology propagation acquired an encoding** (`wire-format.md` §7.2a). Control frame
  type 7 on stream 0, carrying the signed envelope byte-for-byte with no wrapper.
  Stream 0 rather than a bidirectional stream because an unknown control frame is
  *skipped* and the session survives, which is the right outcome for gossip, where an
  unknown request type on a bidirectional stream is *rejected*. `SiblingUpdate` was
  the precedent nobody had looked at: an unsolicited, server-initiated frame.
  **Forwarding is "forward if and only if you stored it"** — reach is a consequence of
  each node's own storage policy, with no hop count, TTL or reach field, because a
  counter would encode the sender's horizon and impose it on receivers (§12.1) and
  because nothing verifies that an intermediary decremented it. Duplicate suppression
  is by `txid` against the store the node already keeps. **No acknowledgement and no
  retry**: §3.1's back-pointers make a gap self-announcing at the receiver, §5.8
  fetches what is missing, and periodic reconciliation with siblings and patron is a
  replay of the same frames rather than a separate mechanism.

  **The rootward memo is new** (design §12.2, `wire-format.md` §7.2b). Frame type 8. A
  minified record of every membership change — subject, patron's position, added or
  removed, subject's `seqno` — travels up the patron chain to its subnet's root. This
  is what §12's *ancestors* reach had always meant and never said. **Peering is
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
  (`wire-format.md` §5.3d) modelled on `AnchorEntry`, self-signed for the same reason
  §5.3 gives — retroactive attribution of forged gossip, which is worth having for a
  flooded object and not for a referral from the one party you are already talking to.

  **Queue policy settled** (§11.1.6). At the ceiling: refuse the newest and tell the
  sender, never drop the oldest — chosen on the adversarial case, since drop-oldest
  lets anyone who can reach a queue flush what is in it. No copy outlives delivery;
  crash recovery is the operator's backup problem and duplicating it in the protocol
  buys durability against a smaller failure than the sibling decision already accepted.
  Metadata bounded to ciphertext, recipient keyhash and arrival time. Only the cap
  *value* remains, and §16.1.1 already classified that as freely tunable.

  **Owner movement was already written, in the wrong document.**
  `resource-requirements.md` §7.1 and §7.1.1 carried the general rule — access is a
  predicate evaluated at request time, so there is no grant object for a move to
  invalidate — while design §9.8 and §18.2 recorded it as unwritten. Relocated to §9.2
  per §0's rule that requirements documents carry no protocol facts.

  **`CatalogEntry` gains an optional `data_practice` declaration** (§9.5), which is
  §1.1's move where enforcement is unavailable: make the distinction visible and let
  policy weight it, as client-integrity attributes already do. Its reach is bounded by
  the catalog's — you learn a resource's posture if and when you already have access
  and think to ask — which is stated rather than left to be discovered. **P20 is not
  closed by it**; the field gives visibility, not a limit.

  **Abuse reports are standalone**, not bound to a live session (§9.8). Binding would
  make a report unfileable after the session ended, which is when most are filed.

  **What closing all of that cost**, recorded because it is the part that gets lost:
  two new privacy findings — **P35**, an ancestor accumulating a key→position index for
  its subtree, and **P36**, `seqno` gaps disclosing out-of-subnet activity — one new
  correlation entry **C19** joining them, an amended product property at **§10.4**
  (*you cannot search for a person, except downward within your own subtree*), and a
  withdrawal whose reasoning had to be restated: **P8** rested partly on *the attacker
  is already a horizon member*, which the memo made false for ancestors. The conclusion
  survives on its other leg — what labels a subtree is its catalog, and the catalog is
  answered on request within horizon — and P15 carried the same faulty clause and was
  corrected with it.

  **Scope accepted by the author:** a subnet-bounded key→position index is acceptable
  because *joining one subnet rather than another is a choice to be in some sense
  visible to that subnet* — a company, a club, a party. The property defended is that
  it never crosses a subnet boundary, which §4.1.1 guarantees by construction.

- **2026-08-25 (defects repaired during the same round)** — Found by sweep rather than
  by review. `§7.2.2 Verifier selection` was physically located inside §8.4, between the
  archive's open items and §9, and is moved under §7.2 where it belongs; six references
  had been landing readers in the wrong chapter. `wire-format.md`'s §7.3.2 sat after §9
  and is moved back under §7.3. **P17's table row was truncated mid-word** with its tail
  orphaned below the table as body text; rejoined. **§14.5.8's table header was
  corrupted**, a stray footnote occupying the `#` cell; restored. §16.1's *needs an
  encoding decision* group stood over an empty table and now says none remain. §12.1's
  horizon-jobs table still listed *catalog propagation range* after the 2026-08-23
  decision that the catalog never propagates; corrected to *query range*, **topology
  forwarding reach** added, and the count corrected from seven to eight. §9.5's
  "(§9.5 below)" pointed at its own section, and its claim that `discover_scope` is
  "still required for entries that propagate" named a class that no longer exists.
  §8.4's two remaining entries were both already answered — one contradicted P14
  outright — and the section is retitled *Closed*. §19 cited `wire-format.md` §10,
  which does not exist, and an unbalanced parenthesis in build step 12 is closed. The
  parameter count corrected from sixteen to fourteen as the queue row collapsed.
- **2026-08-25 (retrospective material moved out of the human-facing documents)** —
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

  **Sections removed as tombstone lists.** Design §8.4, whose three entries were all
  resolved; §9.8's closed entries, leaving only the gateway-evaluation item; and the
  "Closed in the 2026-08-25 round" table added to §18.2 earlier the same day, which was
  a change-log entry in a specification. §7.4.4 was titled *Open* while stating an
  answer and is retitled to what it says. `wire-format.md` §5.4 and §5.5 kept their
  numbers — removing them would shift §5.6 — and shrank from 39 lines to 20, pointing
  at design §7.4.2 for reasoning they had been duplicating.

  **Twelve withdrawn register rows moved to `review-tracking.md`** — P7, P8, P9, P10,
  P22, P34, N3, C12, C13, C14, C16, C18. **The tombstone rule is unchanged**: numbers
  are never reused and a citation to a withdrawn one still resolves. It resolves in the
  tracker rather than the design, because a withdrawn finding is not a current risk and
  §14.5.4 is the design's statement of current risk.

  **What was deliberately kept.** `wire-format.md`'s field- and type-number tombstones,
  which are normative — an implementer must not reuse a withdrawn number — reduced to
  one-line comments without their explanatory paragraphs. And every *why not* that a
  reader might otherwise re-propose: no activity summary, no veto delegation, no notice
  period, no cold lookup. Those are not history; they are the third thing the design is
  for.
- **2026-08-25 (participant locator removed; selective disclosure adopted; abuse-report
  reporter removed)** — Three changes from one question: what does a presence record
  actually need to carry, to whom.

  **`Participant.locator` is gone.** It recorded *position at meeting time*. A sweep of
  the eleven exchanges that transmit or evaluate a presence record found **nothing that
  reads it**, and it was stale by construction — any later locator supersedes it under
  §2.3's strictly-greater rule and nothing resolves against an old one. It was also the
  field that made §14.5.1's worked example work. **Deleting it is a better answer than
  making it withholdable**, and costs nothing rather than 0.4%. What it removes is the
  *historical* position series, so records no longer trace a trajectory; it does not
  remove position inference, since the keyhashes remain and the witness set still
  discloses a neighbourhood. The worked example is rebuilt on the witness set and says
  so.

  **Selective disclosure is specified**, at design §7.2.1 and `wire-format.md` §4.5.1,
  after fourteen days as a proposal nobody had costed. **Scope came from the sweep, not
  from preference**: location evidence, retention tiers, client integrity, capture
  parameters, proximity channels, `started_at` and subtype leave the body and are
  committed as salted digests; **ten of eleven exchanges read none of them.**
  `wire-format.md` §4.5.2 and design §7.2.1 both state the per-exchange visibility, and
  each consuming section now says what it sees.

  **A flat digest list, not a Merkle tree.** §14.5.3 proposed a tree, following SD-JWT
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
  keyhash is on the record however little the body discloses** — and §3 infers signer
  role by comparing `kid` to body fields, so withholding those lists would break
  inference outright. **P2 and C2 are untouched.** §14.5.3's claim to be "the only lever
  that addresses composition directly" was wrong in both halves and is corrected.

  **And the trade it actually offers.** §7.1.7's impossible-travel check and P21's
  behavioural-location leak are **the same computation over the same series**. Field-level
  disclosure cannot separate them; it separates audiences. The one recipient with a use
  for location — a prospective patron reading an archive prefix — is the same party P19
  and C4 flag as the dangerous holder. Everyone else stops receiving it. Stated plainly
  in §7.2.1 rather than left for a reviewer.

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
- **2026-08-25 (aggressive minification of the specification documents)** — At the
  author's direction: review ingestion was drowning in descriptive residue, and the
  chosen posture is *describe the minimum active feature set, repair any
  erroneously-deleted justification in the author's own voice later*. Normative
  content untouched; everything cut is recoverable from git and this log.

  **Cut from `network-design.md`** (~6,600 → ~5,900 lines): all eight vignettes and
  their cross-reference map; every date qualifier in decision markers (`[D — date]` →
  `[D]`, 79 across both documents); §0 rewritten to the three conventions without
  their origin stories; the preface's dated status paragraph and working-context
  note; review-pass narration in §14.5, §16.1, §16.1.1 and §18 ("a re-run on
  2026-08-16 found…", "pass 0.4 found 30…"); the locator-removal and
  worked-example provenance notes from the previous round; **Appendix A.2 (parameter
  conflicts) and A.3 (questions closed during design) deleted** — history, already
  duplicated here — with A.1 kept, the orphaned trust-policy row folded into its
  table, and the 110/1,110/1,000 disambiguation kept as the new A.2. Stale
  references that had pointed at the old A.2/A.3 redirected to §15.2, §8.3 and this
  file (several had been silently resolving to the wrong content since the
  assumptions register moved into §15.2).

  **Cut from `wire-format.md`** (~3,000 → ~2,880): "previously unspecified"
  paragraphs; §9's resolved-items history; field tombstones shrunk to one-line
  "key N unused, not reused" comments — **numbering itself unchanged**, since
  renumbering is a normative change reserved for a publication pass.

  **`authoring-conventions.md`**: the vignette convention removed with the vignettes.

  **Kept deliberately**: §14's findings and correlation registers (current risk),
  §15 (assumptions), A.1 (rejected alternatives — the anti-re-proposal register),
  §17/§18 (open items), and every forward-stated "why not".
- **2026-08-25 (0.8's adversary roles restored; a verbatim prompt repaired after
  being edited)** — Two corrections to the review plan, both about §0.8.

  **The six adversary roles were missing** and the author recovered them from an
  early download: a participant's own patron; a witness at a ceremony; a funded
  commercial operator at $100k/month; a state actor with legal compulsion over one
  cloud provider; a malicious counterparty at a single meeting; a device thief
  holding keys and archive. **They were absent before version control began**, so
  the loss is not in the history — the likely moment is the 2026-08-23 restoration,
  which recovered the section's prose and prompt but not the table the prompt
  selects from. The section had been self-contradicting since: the prompt says
  *[ONE ROLE FROM THE TABLE]* and the rubric assumes six constructions, with no
  table in the file. All six map onto existing findings — §14.4's eclipse, §14.1's
  fake-subtree cost, C8, P29, P5/C9 — which is what corroborated the recovery.

  **And an edit to the prompt was reverted.** The 2026-08-25 reference sweep
  remapped the weakness-register citations inside §0.8's prompt to current
  numbering. The 2026-08-23 entry states that those numbers are stale *deliberately*
  — it is the text that ran, preserved as such — so the remap was a change to a
  known-good prompt made incidentally rather than as a decision. Reverted; §0.8 now
  diffs identical to the baseline commit. **The lesson for future sweeps: a
  reference can be stale on purpose, and a mechanical repair cannot tell.** §0.8's
  *nine privacy costs* is wrong against §14.5.7 for the same reason and stays wrong.
- **2026-08-25 (P24 reduced; §9.8 closed; a reach claim corrected)** — The author
  corrected a mistake in the previous round's §9.5 text. `data_practice` was
  described as reaching "a user who already has access", which conflated the two
  scopes: **`discover_scope` gates catalog answers and `connect_scope` gates
  sessions, and the first comes first.** A user must query the catalog to know a
  resource exists at all, so the declaration is in hand at the moment the decision
  to connect is made — not after it. The reach paragraph is rewritten and §9.7's
  narrow-protection paragraph, which predates the field, now says the user evaluates
  rather than discovers afterwards.

  **P24 reduced from High to Medium** in consequence: the finding was that §9's
  permission model gave a user *no way to evaluate the operator they route through*,
  and the signed entry plus its declared posture, delivered pre-connection, is that
  way. Residual: a declaration is a claim and not a guarantee (§1.1), and the
  operator sees the traffic whatever they declared.

  **§9.8 deleted** — the gateway-evaluation item was its only entry. Three
  citations of §9.8 elsewhere in §9 turned out to mean **gateways**, which are §9.7,
  and were stale from an earlier renumber; repointed. §18.2's citation of §9.8 for
  the resource-interaction blocker was stale in the other direction — §9.8 stopped
  carrying that status when it was reduced to one item — and is removed rather than
  repointed, since §18.2 already states the blocker in full.
- **2026-08-25 (incident narration removed from §18.2; §14.5's intro repaired)** —
  §18.2's resource-interaction entry told the story of an authoring error: a status
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

  **§14.5's intro was damaged**, and the damage predates version control — identical
  at the baseline commit. A sentence had lost its subject, leaving *"…under LINDDUN's
  seven categories. it was argued locally at each mechanism and never assembled"*
  followed by a claim about why an unnamed finding went unnoticed. Repaired to state
  the surviving point — privacy is assessed under composition rather than mechanism
  by mechanism — and to cite §14.5.1, which argues it in full.