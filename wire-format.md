# Wire Format Specification

**Status:** Draft. Last revised 2026-08-22.

**Companion to `network-design.md`.** That document holds rationale, this one
holds encoding. Where they disagree the design is authoritative and this file is
stale.

> **Provenance marking.** Fields are marked **[D]** where derived directly from a
> decision in the design document, and **[P]** where proposed here and not yet
> agreed. Every [P] is a decision still owed.

---

## 1. Encoding

**CBOR (RFC 8949) with deterministic encoding, RFC 8949 §4.2.** [D]

Rationale: every archive-retained object in this protocol is signed, and **protobuf serialization
is explicitly not canonical** — Google's own documentation says so, citing
unspecified field ordering and unknown-field handling, so the same logical
message can serialize differently and signatures fail to verify. CBOR's deterministic profile fixes
map key ordering, integer encoding, and float handling.

**Signatures use real COSE (RFC 9052).** [D] — *not* a custom map that merely
resembles it.

**A custom map that resembles COSE is not COSE**, and the divergence is invisible
until someone tries to implement against a real library. **Use `COSE_Sign` for
multi-signer objects and
`COSE_Sign1` for single-signer ones**, with signed input being the RFC 9052
`Sig_structure`.

Three things this buys beyond interoperability:

- **A defined structure for domain separation.** But **not domain separation
  itself**, which this profile must supply. RFC 9052's `Sig_structure` context
  string is `"Signature"` or `"Signature1"`, distinguishing *COSE structure types*,
  not application roles. **COSE does not give role
  separation for free**, and this protocol has **twelve** signing roles, and will acquire more. See
  §1.1 for the required profile rule.
- **A place to put the algorithm identifier.** RFC 9052 requires `alg` to be
  authenticated but permits it either in the protected header **or as externally
  supplied data**, so "self-describing" is a profile choice rather than a COSE
  guarantee. **This profile requires `alg` in the protected header** of every
  signature. The earlier custom `VerifierResponse` signature carried no algorithm
  at all and was not independently implementable.
- **A defined signing input**, which the custom scheme lacked for two of its
  three contexts.

**Nested COSE objects are UNTAGGED.** [D] RFC 9052 permits either, and libraries
expose both serialisations, so a profile that does not choose gets two byte
encodings for one object — which breaks canonicality before it breaks
interoperability.

**All signatures in this profile are DETACHED.** [D] The COSE payload slot is
`nil`, and a verifier reconstructs the payload from the object it is checking.
Carrying it embedded would give one logical object two byte encodings, the object
itself plus a copy inside its own signature, and the second copy could disagree
with the first.

**A signature covers every retained field of the object it signs, except the
signature field itself**, including unknown extension keys. **For a transaction that
object is the body, never the envelope** (§3): version and type sit outside
coverage, so adding a signer does not invalidate existing signatures. [D] The per-object descriptions below name the
fields defined *today*; they are not a closed list. §1's rule that unknown map keys
survive re-serialisation exists precisely so an extension is covered rather than
silently dropped, and reading "signs fields 1–2" as exhaustive would defeat it.

### 1.1 Domain separation — REQUIRED PROFILE RULE

COSE does not separate application roles (above), so this profile does.

**Every distinct signing context MUST carry its own separation tag, and a verifier
MUST reconstruct that tag from the context in which it is checking — never from the
message.** A context is distinct whenever the same key could be asked to sign in
more than one role; adding a role therefore means adding a tag, and the table below
is the current enumeration rather than a closed set.

Present encoding: `external_aad` holds the ASCII role tag.

| Role | `external_aad` |
|---|---|
| Transaction envelope | `rhtn/1:envelope` |
| Old-key recovery proof (§4.1) | `rhtn/1:recovery` |
| Verifier response (§4.5) | `rhtn/1:verifier` |
| Subject consent to a query (§4.5) | `rhtn/1:consent` |
| VerificationQuery, canonical form | see §4.5 |
| Currency attestation (§5.1) | `rhtn/1:currency` |
| Catalog entry (§4) | `rhtn/1:catalog` |
| Abuse report (§4) | `rhtn/1:abuse` |
| Standalone locator (§2.3) | `rhtn/1:locator` |
| Anchor entry (§5.3) | `rhtn/1:anchor` |
| Prekey bundle (§5.7) | `rhtn/1:prekey` |
| Subtree acknowledgement (§5.3c) | `rhtn/1:subtree-ack` |
| Old-key successor statement (§4.1) | `rhtn/1:successor` |

**Why it matters more than it did.** Exploiting cross-context confusion requires a
byte string valid in two roles, which the differing CBOR structures argue against without ruling out
— and unproven non-confusability is precisely what domain separation exists to
replace, and the number of roles grew from three to seven while the protocol
believed it had separation it did not have. A verifier that derives the tag from
context rather than content also makes the check free.

**Deterministic rules, normative:**
- Map keys in **protocol-defined** maps are unsigned integers, sorted ascending.
  **Standardised structures are exempt**: a `COSE_Key` carries negative labels by
  definition (§2.2) and its encoding is fixed by its own RFC. Integer keys keep
  records
  small and avoid string-ordering ambiguity.
- Definite-length encoding only. No indefinite-length arrays, maps or strings.
- Shortest-form integers.
- No duplicate map keys; a decoder MUST reject them rather than take the last.
- Unknown map keys MUST be preserved when re-serializing for signature
  verification, and MUST NOT be silently dropped.
- **Validate the bytes as received; do not decode and re-encode to compare.** [D]
  A decode-then-re-encode check erases the evidence it is meant to find — duplicate
  keys collapse, and an unknown key's original encoding is lost. Keep the received
  bytes and derive typed views from them.
- **Duplicate map keys MUST be rejected before the map is materialised.** A
  decoder that parses into a map type first has already collapsed them, and
  deterministic encoding is then satisfied by a body the sender never sent.
- **An OPTIONAL field whose value would be an empty array or map MUST be omitted,
  never encoded empty.** [D] Absent and present-but-empty produce different bytes
  and therefore different signatures and txids; without this the same logical object
  has two valid encodings.

  **This does not apply to required fields.** A required map with no entries is
  encoded as an empty map — `Capabilities` in `Attach` and `AttachAck` is the case
  in point, since a peer advertising nothing must still send the field. Nor does it
  apply to a top-level object: a `SiblingUpdate` clearing the list encodes as an
  empty map, and reading the rule literally would leave that state with no encoding
  at all.
- **Unknown *values* in a known enumerated field MUST be rejected** [D] — distinct
  from unknown map *keys*, which are preserved. A decoder that cannot interpret a
  result code cannot evaluate the object, and silently ignoring it would mean
  treating an unevaluated field as absent.
- **Every array is bounded.** [D] Arrays arriving from strangers are a resource
  attack surface, so each is given an explicit maximum below; exceeding it is
  malformed, not merely unusual.

| Array | Maximum |
|---|---|
| Envelope **logical signers** | **Derived, not a constant.** The sum of the transaction type's per-role bounds. For a presence record: 2 participants + 16 witnesses = **18**. **Verifiers are not envelope signers.** Their signatures are embedded evidence inside the body (§3.2, design §5.1) |
| Envelope `COSE_Signature` **entries** | **Twice the logical-signer bound**, since each contributes one classical and one PQ entry (§3.2). For a presence record, **36** |
| Archive subset references | 256 |
| Verifier responses per recovery | 32 |
| Witnesses per presence record | 16 |
| Path length | 24 nibbles (depth 24 at f=10 exceeds any plausible network) |
| Prekey bundle blob | 4 KB. A PQXDH bundle is an ML-KEM-768 encapsulation key (1,184 B) plus signed prekeys and their signatures — roughly 1.5–2 KB, so this is a DoS ceiling with headroom rather than a capacity figure |
| Merge back-pointers per signer | 8 |
| Verifier responses per presence record | 32 — **two subjects × a per-subject threshold capped at 10**, with headroom. A bound of 16 was unsatisfiable: two well-connected participants each require 10 |
| Asserted locations per record | 4 |
| Corroborations per record | 16 (one per witness) |
| Proximity channels per record | 8 |
| Explicit-scope keyhash list | 256 |

| NetworkPoint entries per anchor, peering endpoint or endpoint record (§5.3d) | 8 |
| `CatalogEntry`, total encoded bytes | 2048 |
| `CatalogReply` entries | 64 |
| Unknown extension keys per map | 16 |
| Unknown extension value, bytes | 1024 |
| `Capabilities` map entries | 64 |
| `Capabilities` value, bytes | 1024 |
| SiblingRef entries in `AttachAck` **or `SiblingUpdate`** | 9 (f − 1), the update replaces the same logical list |
| Peering audit history | 8 |

**None of these maxima is derived from a capacity study.** They are conservative
ceilings chosen to bound a decoder's exposure to a hostile peer, set well above any
use anyone has articulated. That is the right basis for a DoS bound and the wrong
basis for a capacity claim: **exceeding one of these means malformed, not
overloaded.** If a legitimate use ever approaches a ceiling, the ceiling is wrong
and should move, the numbers carry no evidence that they are correct, only that
they are safe.

**The envelope bound MUST be derived, never asserted independently, and derived in
two steps.** Sum the transaction type's per-role bounds to get the **logical
signer** ceiling, then **double it** for the `COSE_Signature` array, since each
logical signer contributes one classical and one post-quantum entry (§3.2).

**An independently asserted envelope bound can contradict the per-role bounds**, so
that a record one bound permits the other rejects. Deriving it makes that
structurally impossible rather than something to notice.

**Signature arithmetic, so the figures above are checkable.** Ed25519 is 64 bytes,
ML-DSA-65 is 3,309. **One logical signer costs 3,373 bytes**, not 6,618 — only one
of its two entries is post-quantum (§3.2). An adoption's two signers are therefore
≈ 6.6 KB, and a ten-signer presence record ≈ 33 KB, both before the body.

**Typical is far below maximum.** A presence record is expected to carry ~10
logical signers — two participants and ~8 witnesses, at 10 × (64 + 3,309) ≈ **33
KB**, or ~35 KB with the body. The bound accommodates 18 envelope signers plus 32
embedded verifier responses, ≈ **65 KB**. Bounds exist to
stop a stranger exhausting memory, not to describe normal operation.

**Content addressing.** `txid = SHA-256(deterministic CBOR of the body map)`,
excluding the signature array. [D] SHA-256 is adequate post-quantum: Grover
reduces an ideal 256-bit preimage search from ~2^256 to ~2^128 *queries*. Calling
that "128-bit security" compresses quantum circuit cost, parallelisation limits and
hardware realities into one number — all of which push the practical margin up
rather than down.

---

## 2. Primitives

```
keyhash   = bstr .size 32          ; SHA-256 of the deterministic CBOR encoding
                                   ; of KeyMaterial (§2.2), the fixed-order
                                   ; PAIR of COSE_Keys, never one of them
timestamp = uint                   ; seconds since Unix epoch; u64 RANGE
seqno     = uint                   ; per-node monotonic counter; u64 RANGE

; NOTE: `.size 8` was removed 2026-08-16. It read as a fixed eight-byte
; serialisation, which contradicts the shortest-form requirement in §1, a 2026
; timestamp encodes in five bytes, not nine. These are range constraints only;
; encoding is always shortest-form deterministic CBOR.
```

**Identities are referenced by hash, never by key.** [D, design §7.2] An **ML-DSA-44**
public key is ~1.3 KB and the larger parameter sets are bigger still; a hash is 32
bytes. Full key material appears only in `KeyMaterial`
(§2.2), transmitted on first contact and pinned thereafter.

### 2.1 Path encoding

```
path = {
  1: bstr,        ; 4-bit nibbles, one per hop, high nibble first
  2: uint         ; length in nibbles (NOT bytes)
}
```

[D] With f = 10 (design §4.2) each hop index needs values 0–9, so 4 bits suffice. A
depth-11 path — sufficient for 6×10¹⁰ nodes — occupies 6 bytes.

**The packed byte string MUST be exactly `ceil(nibble_count / 2)` bytes.** Trailing
surplus bytes are malformed, not ignorable — otherwise one logical path has
unboundedly many encodings.

**Nibble values MUST be 0–9**; 10–15 are malformed. **On an odd nibble count the
unused low nibble of the final byte MUST be zero.** [D] Without that rule one
logical path has sixteen valid byte encodings, and deterministic CBOR does not
fix semantic malleability inside a byte string. The explicit
nibble length is required because **paths are truncatable** [D, design §7.1]: a distant
node receives only the prefix needed to route to the right region, and truncation
must be expressible at nibble granularity rather than byte granularity.

### 2.2 Key material

Identity keys are a **COSE_KeySet** (RFC 9052 §7) of exactly two entries in fixed
order — classical first, post-quantum second.

```
KeyMaterial = [ COSE_Key, COSE_Key ]   ; [classical, post-quantum]
                                       ; fixed order; a COSE_KeySet
```

[D] design §5.1 binds an identity to **both** components, so a single
`COSE_Key` cannot represent one. The order is fixed because the keyhash is taken
over the encoding: reordering would produce a different identity for the same
keypair.

**Profile: the post-quantum component is ML-DSA-65** (`alg = -49`).

**Signing is deterministic** (the ML-DSA variant without added randomness). [D]
Both variants verify identically, so this affects nothing on the wire — but test
vectors must pick one, and reproducible signing makes a failing vector diagnosable
rather than merely repeatable.

**Ed25519's parameters come from RFC 9053**: `kty` = OKP (1), `crv` = Ed25519 (6),
public key in `x` (-2).

**That only three labels may appear is this profile's rule, not the RFC's.** [D]
RFC 9053 says which parameters an OKP key requires; COSE keys may also carry common
parameters such as `kid`, `alg` and `key_ops`, and the standard does not forbid
them. **`KeyMaterial` forbids them** — no `kid`, no `key_ops`, nothing beyond the
three above — because the keyhash is taken over the encoding, so **any additional
parameter yields a different identity for the same key.**

**The ML-DSA component uses RFC 9964's AKP key type.** [D] The RFC supplies the
parameters; **the restriction to exactly these three is this profile's rule**, for
the same determinism reason as above. RFC 9964 requires `alg` and `pub` and forbids
`priv` in a public key, and its own example carries a `kid`, which this profile does
not permit inside `KeyMaterial`:

| Label | Parameter | Value |
|---|---|---|
| 1 | `kty` | **7** (AKP, *Algorithm Key Pair*) |
| 3 | `alg` | **-49** (ML-DSA-65). REQUIRED for all AKP keys |
| -1 | `pub` | `bstr`, the raw public key. REQUIRED |

**`priv` (label -2) MUST NOT appear** — that one is RFC 9964's rule for a public
key. **`kid` and every other optional or common parameter are excluded by this
profile**, because any additional entry changes the encoding and therefore the
identity.

*(This was previously flagged as unpinned and identity-critical. It was neither: RFC
9964 standardised the representation in May 2026, and the flag reflected a failure
to check rather than a gap in the standards.)*

**Algorithm identifiers are encoded as COSE `int`.** [D] The COSE registry is not
restricted to negative values, but **the algorithms this design requires happen to
be negative** — EdDSA is −8, ML-DSA-44/65/87 are −48/−49/−50, so a `uint` field
cannot encode them, and a `uint` field would make the mandated algorithms
unrepresentable. `int` is a profile decision that accommodates the
registry; it is not a claim that COSE identifiers are always negative.

Algorithm profile [D, design §5]:

| Use | Algorithm |
|---|---|
| Transport KEM | ML-KEM |
| Long-lived root / patron identity | Hybrid: **two components, and the keyhash covers both** (design §5.1) |
| Any object whose authenticity must survive as long as it can be presented as evidence — **every transaction here** | Post-quantum. The archive makes any transaction presentable indefinitely, so reliance never expires (design §5.1) |
| Session-layer traffic, never retained | Classical (Ed25519) |
| Presence records | Post-quantum (ML-DSA) |

### 2.3 Locator

```
Locator = {
  1: keyhash,     ; anchor
  2: path,
  3: seqno        ; the node's own monotonic counter
}
```

[D, design §7.1] The `seqno` here **is** the monotonic counter of §6.2, not a separate
field: one counter per node, incremented on every position change, serving both
as freshness test and stale-cache detector.

**Verification rule** [D]: a new locator's seqno must be **strictly greater** than
the last one the verifier holds for that node — **not** exactly previous+1. A
verifier may legitimately have missed intervening transactions, so requiring
contiguity would reject valid updates. Absence of prior state is not a failure.

**Two carriage forms** [D]. design §7.1 requires that routing
information be authenticated by the participant it describes; an earlier version of
this section said a locator is "always transmitted inside a signed envelope", which
is true inside a transaction and false at introduction, where a locator is handed
over on its own.

```
SignedLocator = {
  1: keyhash,        ; subject — whose position this describes
  2: Locator,
  3: COSE_Sign1      ; BY THE SUBJECT over canonical CBOR of fields 1-2;
                     ; external_aad = "rhtn/1:locator".
                     ; Classical only: a locator's relevance expires when the
                     ; node next moves, so §5.1's post-quantum horizon does
                     ; not apply
}
```

- **Inside a transaction body**, a bare `Locator` suffices, the enclosing
  envelope signature authenticates it.
- **Standalone**, as at introduction or in a forwarding repair, a `SignedLocator`
  is required. A bare `Locator` presented alone MUST be rejected.

A bare locator carries no
signature of its own, because an unsigned locator lets any relay substitute
itself as the node's mailbox [D, design §7.1].

---

## 3. Common envelope

```
Envelope = {
  1: uint,              ; schema version — 1 is current
  2: uint,              ; message type (see §4)
  3: { * uint => any }, ; body — type-specific
  4: COSE_Sign          ; RFC 9052; TWO COSE_Signature entries per required
                        ; logical signer — one classical, one PQ (§3.2)
}
```

**Signature coverage is the body map only.** Never the envelope, or signatures
would not survive the addition of a signer. The COSE payload is therefore the
deterministic CBOR of field 3, and the signed input is the `Sig_structure` COSE
builds around it. [D]

### 3.1 Common body field: chain back-pointers

**Every transaction body carries key 0**, reserved across all types:

```
0: [ + [ + bstr .size 32 ] ]  ; chain back-pointers, one LIST per signer, in the same
                     ; order as the transaction type's required signer set.
                     ; Each list holds SHA-256 of that signer's previous
                     ; transaction(s), length 1 normally, longer at a merge.
```

[D] Each participant's archive is a hash chain, so sequence
position is as trustworthy as the record itself. Because the back-pointer sits in
the **signed body**, altering a record's position requires forging the *next*
record's back-pointer, which its counterparty already signed. Excision is
therefore impossible; only truncation to a prefix remains.

**This applies to every transaction type, not only presence records.** Adoption,
departure, disavowal and peering all advance their signers' chains. A signer with
no prior transaction uses the genesis value (§3.2).

**One back-pointer LIST per required signer**, since each signer has their own
independent archive. An adoption advances both the node's archive and the patron's.

**Length > 1 is a merge** [D]. A signer whose archive has forked
— typically from concurrent use of two devices — reunites it by supplying **both**
branch heads in the next ordinary transaction. **No merge transaction type
exists**; a merge is an ordinary transaction with a longer back-pointer list.

A merge **commits to both branches**: omitting one afterwards leaves a
back-pointer unsatisfied and is detectable. The archive is therefore properly a
**Merkle DAG**, not a chain, and cross-branch ordering is deliberately not
recovered, the structure proves that no intermediate record is missing, which
requires reachability rather than sequence.

A decoder MUST accept lists of **any length from 1 to 8.** The bound in §1 — and
MUST verify every back-pointer present, not merely the first.

### 3.2 Genesis

A signer's first transaction has no predecessor. Its back-pointer is
`SHA-256(the signer's keyhash)` [D] — a value derivable by any verifier, so a
claimed first transaction is checkable rather than assertable.

**Presence record structural rules** [D], all previously unstated:

- **The two participant identities MUST differ.**
- **`finalized_at` MUST be ≥ `started_at`.**
- **A presence record on the wire is always final.** A ceremony whose threshold is
  unmet is local state and is not published; `finalized_at` therefore always
  records a threshold that was met. Late responses arrive as `LateResponse` objects (§5.3a) and
  never alter the original record's validity.
- **`strongest` MUST appear among the channels with `result = pass`, and no
  higher-ranked channel may appear with `pass`.** Ranking is UWB > NFC > optical >
  latency (design §7.1.6.3). Without the second half the field is not
  deterministic.
- **A key may appear in at most one formation record: its first.** [D]
  A formation record's back-pointer for each signer MUST be absent, which is only
  true of a signer's first record. **An established key cannot sign one**, because
  its key 0 entry names a predecessor.

  **This is what bounds the population.** Without it, an identity with standing could
  manufacture formation records against arbitrary fresh keys — cheap, since formation
  needs no witnesses and no verifiers — and flood its own candidate pool. **With it,
  the attacker's thousand keys can only form with each other**, and a set of mutually
  formed strangers has no standing with anyone and is a candidate for nobody real.

- **Beyond that, `formation` is an evidence label rather than a further
  history-dependent precondition.** A validator checks the subtype, the absent
  evidence arrays and the absent back-pointers; it does not evaluate what the
  participants did afterwards. Established
  identities can therefore mint formation records, which the design accepts —
  their weakness is evidentiary (design §10.8.2), and making eligibility a
  structural rule would require every validator to hold both participants'
  histories.
- **Formation records omit fields 8 and 9 entirely** rather than encoding empty
  arrays, per §1's rule. Absence means empty.
- **Witness identities MUST be distinct**, and **MUST NOT include either
  participant.** Duplicates would give one identity several nonce inputs to the
  seed and leave the ascending-keyhash ordering undefined between equal keys; a
  participant witnessing their own ceremony is not an independent witness, which is
  the entire role.
- **A normal record MUST carry at least one witness.** Zero witnesses is the
  formation case and nothing else, a normal record with none is malformed rather
  than merely weak, since the subtype is what separates an uncorroborated bootstrap
  from an uncorroborated ordinary meeting (design §10.8.2).
- **One identity is one logical signer regardless of how many roles it holds.** A
  party that is both a witness and a verifier signs once **in each capacity it
  signs in** — as a witness it is an envelope signer, as a verifier it signs an
  embedded response, and those are different objects. **The envelope bound is 2 + 16
  = 18** (§1); verifier responses are embedded evidence and do not count toward it.
- **A witness's `nominated_by` MUST be one of the two participants.** Whether that
  witness actually belongs to the nominator's counterparty's neighbourhood is not
  checkable from the record and requires topology state, the schema records the
  claim, evaluation is the reader's.

**Timestamps are not checked against a local clock at structural verification.**
[D] No maximum age or future tolerance applies; a decoder has no authoritative clock
to check against and inventing one would make validity depend on the reader.

**But `started_at` is not merely evidentiary, and MUST be monotonic against the
committed back-pointer.** [D] For each signer, `started_at` MUST be
greater than or equal to the `finalized_at` of the record its key 0 back-pointer
names, and `finalized_at` MUST be greater than or equal to `started_at`. **A record
violating either is malformed.**

**Without that bound, backdating collapses verification.** `started_at` determines
the 730-day window, so *n* — and therefore the finalization threshold — is computed
relative to a value the proposer chooses. **Claiming a `started_at` earlier than
one's own history yields n = 0 and a required verifier count of zero.** The same
field derives `window_ordinal`, so varying the claimed day also yields fresh
verifier and witness samples without waiting for one.

**The back-pointer is what makes this checkable.** It is inside the signature and
names a record with its own `finalized_at`, so a subject cannot claim a ceremony
earlier than their own last one. **The bound is against the signer's own history,
not against anyone's clock** — which is the only monotonicity available in a system
with no global time.

**"Verify" means structurally valid, not effective.** [D] A verifier confirms
encoding, signatures, and the structural rules stated here. Whether the adoption
*takes effect* — the patron has capacity, no cycle results, the node's prior
binding ended — depends on topology state a validator may not hold, and is a
separate question answered by §5.6's resolution and the receiver's own view. An API
returning one boolean for both is answering a question nobody asked.

**A back-pointer whose record is unavailable is not a failure.** Verifying a chain
means each *presented* record's back-pointers match the record following it; an
unfetchable predecessor means the chain is **incomplete**, which is a fact for the
caller to weigh, not a malformed transaction (design §8).

**If `KeyMaterial` is carried, its hash MUST equal the keyhash of the party it
describes** — in an adoption, field 1, the adopted node (field 5).
[D] Otherwise a sender could present one identity's keyhash alongside another's
key material, and a recipient pinning from the transaction would pin the wrong key.

**A dereferenced proof-of-presence record needs none of §4.5.1's disclosable fields.**
[D] The check is that the record exists and names these two parties, both of which are
body fields. A fully withheld record satisfies it.

**Structural verification does not dereference `proof_of_presence`.** [D] The field
names a presence record by txid; confirming that record exists, and that it names
this node and this patron, requires fetching it. **That is an evaluation step, not a
structural one** — a validator holding only the transaction can check the reference
is well-formed and nothing more, which is the same boundary §4.1's *valid versus
effective* note draws.

**A verifier lacking one signer's key material has neither verified nor rejected
the object.** [D] That is a third outcome, not a variant of failure: the encoding is
sound, the structural rules pass, and one signature could not be checked. **An API
collapsing it into "invalid" reports a well-formed transaction as malformed** and
invites a caller to discard something it should have fetched a key for. Name the
identity whose key is missing.

**Verification requires key state.** A conforming adoption may carry only
keyhashes — full `KeyMaterial` appears on first contact and is pinned thereafter
(§4.1), so a verifier needs previously pinned keys or a resolver. **An API
promising to verify an arbitrary transaction from bytes alone cannot be
implemented**, and offering one hides the key-management requirement rather than
removing it.

**The two signatures need not be produced together.** [D] The body is fixed before
either party signs — `txid` covers it — so signatures may be collected in any order,
over any channel, with any delay. **Nothing in the envelope records how they were
gathered**, and a verifier cannot tell a co-present exchange from one assembled over
days. Only the finished envelope is specified.

**Signer set rules** [D]:

The rules count **logical signers**, not `COSE_Signature` entries. design §5.1
requires both identity components to sign an archive-retained transaction, so one
logical signer contributes **two entries**.

- **Each logical signer contributes exactly two `COSE_Signature` entries**: one
  under its classical component (`alg = -8`) and one under its post-quantum
  component (`alg = -49`). Both cover the same payload and the same
  `external_aad`.
- **`alg` lives in the protected header of whatever structure holds the
  signature.** For a `COSE_Sign1` that is the object's own protected header; for a
  `COSE_Sign` it is **each contained `COSE_Signature`**, and **the outer
  `COSE_Sign.protected` is empty** — a `COSE_Sign` carries no signature of its own,
  so it has no algorithm to name. That covers nested objects (an old-key `Recovery`
  proof, an embedded `VerifierResponse`) as well as the envelope.
- **A nonempty unprotected header is malformed, not ignored.** [D] Tolerating one
  would give a single logical object two byte encodings; "ignored" appears elsewhere
  in this document about *unknown map keys*, which are preserved and re-serialised,
  and that rule does not extend to COSE headers.
- **`kid` is present exactly when the surrounding structure does not already name
  the signer.** [D] Envelope `COSE_Signature` entries carry it, since a transaction
  body names identities by role and the entries must be matched to them. **Embedded
  `COSE_Sign1` objects omit it**: a `VerifierResponse` names its verifier in a
  field, a `Recovery` proof its prior key, so a `kid` would be a second copy that
  could disagree with the first.
- **Nothing else appears in either header.** Any additional entry changes the
  protected bytes and therefore the signature, so an unrecognised header entry is
  a malformed object rather than a tolerable extension. [D]
  Additional headers are rejected rather than ignored: the protected header is
  covered by the signature, so tolerating unknown entries there would let two
  implementations disagree about what was signed. **A nonempty unprotected header
  is malformed**, per the rule above — not ignored.
- **Both envelope entries carry the same `kid`, in the PROTECTED header: the
  32-byte identity keyhash.** [D] RFC 9052 offers protected and unprotected buckets and a
  verifier that accepts only one placement rejects otherwise valid signatures;
  protected is chosen so the signer binding is covered by the signature itself. Entries are
  grouped into logical signers by `kid`, and distinguished within a group by
  `alg`. A group with fewer than two entries, two entries of the same `alg`, or
  any `alg` outside the profile is malformed.
- **Exactly the logical signers the transaction type requires.** So an adoption
  carries two logical signers and therefore **four entries**. No extra signers, no
  duplicate signers.
- **Order is not significant**, and a canonical order is nonetheless required so
  two encoders produce identical bytes: **entries sort by `kid`, then classical
  before post-quantum.** Deterministic CBOR fixes map ordering but says nothing
  about array element order, and the signature covers the body rather than the
  array, so without this rule two conforming encoders of the same transaction
  produce different **envelope bytes**. The `txid` is unaffected — it hashes the body
  map, which excludes the signature array (§1) — but envelope byte-identity matters
  wherever an envelope is compared, cached or deduplicated.
- Signer role is inferred by comparing each `kid` to the body's fields. There is
  deliberately no role field: redundant where inference works, forgeable where it
  does not.

**Why not a composite algorithm identifier.** Concatenating both signatures under
one private-use `alg` would keep the entry count at one per signer, but it would
require inventing an algorithm the COSE registry does not define, and no standard
tooling could verify it. Two standard entries stay within RFC 9052, let each
component be verified independently, and support §5's staged migration — a
verifier may check the classical component for a fast path and the post-quantum
one when the decision warrants it.

**Canonicality applies to the whole envelope**, not only the signed body. [D] A
non-canonical envelope is malformed and rejected even if its body verifies —
otherwise two encodings of the same message both validate.

**Version mismatch** [D]: schema version 1 is current. **An unknown version MUST
be rejected**, not skipped or best-efforted. A verifier that cannot interpret the
structure cannot establish what a signature covers, so accepting it would mean
accepting an unverified object.

---

## 4. Transaction types

| Type | Name | Signers | Class |
|---|---|---|---|
| 1 | Adoption | node + patron | Topology |
| 2 | Departure | node only | Topology |
| 3 | Disavowal | patron only | Topology |
| 4 | Peering | both infra nodes | Topology |
| 5 | Presence record | participants + witnesses. **Verifiers are not envelope signers** — their responses are embedded evidence signed inside the body (§4.5) | Attestation |
| 6 | Resource registration | owner only | Topology |
| 7 | Abuse report | the reporting resource only | Attestation (point-to-point, never broadcast) |

Type 6 carries a `CatalogEntry` (design §9.5) and registers a resource; type 7
carries an `AbuseReport` (design §9.6) addressed to a resource owner.

**A `CatalogEntry` is optional.** It makes a resource *addressable* — reachable
point-to-point, or able to call out to nodes. A locally-hosted application needs
none, being reached by asking its host directly. **The personalised catalog page a
node serves over a session is a different object entirely**, is not signed, does
not propagate, and names the roles the viewer holds (design §9.5). Both are
**[D].** Agreed 2026-08-16.

```
Scope = uint / [uint, uint] / [uint, [ + keyhash ]]
      ; 0 self | 1 down(n) | 2 up(n) | 3 sub(n) | 4 siblings
      ; 5 dunbar | 6 list([keyhash])
      ; forms taking a depth encode as [tag, n]; list encodes as [6, [...]]

### 4.1 Adoption (type 1)

Subsumes key rotation and recovery [D].

```
{
  1: keyhash,          ; node being adopted
  2: keyhash,          ; patron
  3: Locator,          ; node's resulting position
  4: timestamp,
  5: ? KeyMaterial,    ; the ADOPTED NODE's key material, never the patron's [D]
                       ;   — there is no discriminator on the wire, so the profile
                       ;   fixes it: the arriving node is the identity being
                       ;   introduced and a recipient pins from it.
                       ;   ALWAYS structurally optional, never a
                       ; validity condition, since whether the recipient already
                       ; holds the key is recipient state and a signed object
                       ; cannot depend on it. Senders include it when they believe
                       ; the recipient may lack the key
  6: ? Recovery,       ; present iff this is a recovery adoption
  7: ? txid,           ; archive HEAD presented (design §13.7), one hash, not
                       ; a list. The chain gives the rest
  8: ? txid            ; proof-of-presence record between these two parties
}

Recovery = {
  1: keyhash,          ; prior key whose history is claimed
  2: ? [ + VerifierResponse ],   ; OPTIONAL; absent means empty. An encoded
                       ;   empty array is malformed (§1)
  3: ? COSE_Sign       ; by the OLD key, iff available. COSE_Sign rather than
                       ; COSE_Sign1 because the old identity is hybrid and one
                       ; logical signer contributes two entries (§3.2)
}
```

[D] Presence of field 3 in `Recovery` distinguishes the two variants:
**attested rotation** (old key controlled, cheap, no verifiers needed) from
**recovery adoption** (old key lost or compromised, verifier attestations
required).

**Keystream seeds are local, private and never on the wire.** [D] A participant's
seed for a counterparty's captures lives in that participant's own record of the
transaction (design §7.1.5.2) and is exchanged only over the direct channel during
a ceremony. **It is not a field here**, and a record carrying one would be
malformed: nothing in the evidence a third party evaluates depends on it, and
placing it in a signed object would hand every reader the key.

**Consistency rules for a Recovery block.** [D] Each closes a case where every
signature verifies and the assembly still means something other than it claims:

- **Verifier responses inside a `Recovery` block are HYBRID**, not classical. [D] Every other embedded signature is classical because its relevance
expires; **a recovery's does not** — it induces a permanent identity change that a
later evaluator cannot revisit, so a forged classical `match` would sit inside an
authentic post-quantum record indefinitely (design §5.1). Cost is ~34 KB for a
typical recovery.

**Every response's field 8 MUST equal `prior_key`** — otherwise evidence about
  one old identity is embeddable under a claim about another.
- **Every response's `subject` MUST equal the newly adopted node** (field 1). The
  verifier is attesting continuity *to* the new key; a response naming a third
  party attests something else.
- **Duplicate responses from one verifier are malformed**, as they are in a
  presence record (§4.5) and for the same reason: one verifier occupies one slot.
- **An old-key proof and verifier responses may both appear.** They are independent
  evidence of the same continuity, and the strongest recovery carries both. Neither
  substitutes for the other's absence, which is why the structural minimum is
  stated over their union (§4.1).

**A recovery that rests on verifier responses requires at least one `match`.** [D] Field 2's responses are structurally valid whatever they report,
which is right for presence finalization where the question is whether the query was
honestly *conducted*. **A recovery asserts a fact**, and where verifier evidence is
the only support, a set containing no `match` asserts nothing.

**The rule applies only when field 3 is absent.** An attested rotation carries an
old-key proof, which is the prior identity authorising a change to itself — evidence
that stands without any verifier saying anything. **Requiring a `match` there would
make attested rotation unrepresentable whenever the responses happen to be
inconclusive**, which inverts the intent: the old-key proof is the *stronger*
evidence, not something needing corroboration.

So: field 3 present, no threshold on field 2. Field 3 absent, at least one `match`
among the responses.

**Each response in field 2 MUST carry field 8**, naming the prior identity, and the
verifier's signature MUST cover it. Otherwise the verifier has attested that some
face matches some subject, without ever signing which old identity that continues.

**Structural rule** [D]: field 2 is **optional**, and its
**absence means empty**. It may be absent only when field 3 is present, and must
carry **at least one** response when field 3 is absent. **A present-but-empty
array is malformed**, per §1's rule that empty arrays are never encoded.

**"May be empty" and "at least one" are both wrong here, in opposite directions.**
The first collides with §1: one implementation emits `[]` and another rejects it as
non-canonical, while the second omits the key and the first rejects it as missing.
The second — writing the field as `[+ …]` — makes the attested-rotation variant
unrepresentable, a grammar that contradicts the design.

**Field 3 is a `COSE_Sign`**, not a `COSE_Sign1` — the old identity is hybrid, so it
contributes two entries like any other logical signer (§3.2).

**It signs a successor statement, not the Recovery map.** [D] The payload is the deterministic CBOR of:

```
SuccessorStatement = [
  prior_key,           ; keyhash — the identity being rotated FROM
  new_key,             ; keyhash — adoption field 1, the ONLY successor authorised
  patron_key           ; keyhash — adoption field 2
]
```

with `external_aad = "rhtn/1:successor"`.

**Signing the Recovery map with field 3 omitted would be a replay primitive**, which
is why it does not. When field 2 is absent — an attested rotation carrying no
verifier responses — that payload is a map with nothing in it. The old key signed
*"a rotation happened"* and named no successor, so **one observed proof authorised
an unlimited number of competing successors**: extract it, build a fresh adoption
naming the same `prior_key` with an attacker-controlled new key and patron, insert
the proof unchanged, and the envelope signatures authenticate the assembly while the
old key's statement contains nothing to contradict it.

**A verifier MUST check `new_key` and `patron_key` against adoption fields 1 and 2
and reject on mismatch.** An unchecked binding is the same as no binding.

**It still cannot sign `txid`**, which hashes the body containing it. The successor
statement names the fields it needs directly, which avoids the circularity without
leaving the payload empty.

**The node and patron identities MUST differ** [D] — adoption creates an authority
relationship, and a node cannot hold authority over itself. This mirrors the rule
that a presence record's two participants must differ (§4.5), and was previously
implied only by the signer-set requirement rather than stated.

**`prior_key` MUST differ from the adopted node's key** [D] — identical keys
represent no rotation at all.

**Field 8 — proof of presence.** [D] Expected on a *fresh*
adoption, meaning the patron holds no prior PoP with this node. **Optional in the
protocol, not enforced**: there is no global enforcement point (design §4.1.1),
so a mandate would be a recommendation with extra steps. An adoption without it
is well-formed and near-worthless — policy weights it, the decoder does not
reject it. Not expected on lateral or vertical shifts (design §6.2.3), where the
new patron already holds the history.

**Field 7 is a single txid: the head of the archive prefix being presented.**
[D] Not a list, not a range, not a proof.

**The chain already encodes everything else.** design §8 makes each transaction carry
its predecessors, so a patron given a head walks backward through the
back-pointers, fetching as it goes (§5.8) and stopping when it has seen enough or
stops recognising counterparties. Merges need no special handling, a record with
two back-pointers means both branches are reachable and both get walked.

**Truncation needs no grammar.** A subject presenting less history presents an
*earlier* head. That is the only edit the chain permits (design §8), so the
field cannot express anything the archive model does not already allow.

**The patron chooses its own depth**, which is the right asymmetry: the presenter
picks the head and cannot control how far back the recipient looks, so the party
extending credit decides how much evidence it wants.

**Why not the alternatives:**

| Option | Rejected because |
|---|---|
| **List of txids** | Up to 8 KB of signed body that proves nothing, the patron must still fetch the records and follow back-pointers to confirm the list is a prefix rather than a selection. Duplicates the chain |
| **Range (from A to B)** | In a DAG a range is *reachable from B but not from A*, which is **precisely excision**. design §8 exists to make that impossible; this would return it as a protocol feature |
| **Merkle proof** | Largest body, highest complexity, and needs a tree imposed on a native DAG. It buys verification without fetching, but the patron **needs the contents anyway** to recognise counterparties, so it optimises a cost this use case does not have |

References rather than contents remains right, and is consistent with attestation
being pull (§8): the patron fetches what it wants to verify rather than receiving a
bulk push.

### 4.2 Departure (type 2)

```
{
  1: keyhash,          ; departing node
  2: keyhash,          ; patron being left
  3: seqno,            ; incremented
  4: timestamp,
  5: ? uint            ; reason code — same enumeration as §4.3
}
```

Single signature: the departing node. [D]

**Required for a node to become a root.** Without it, a node that adopts
elsewhere remains in the old subtree's view indefinitely, since adoption says
nothing about existing bindings.

**A party with authority over another may never gate an action whose sole effect
is to end that authority relationship** (design §6.2). Present encoding: **the old
patron does not sign a departure, and a decoder MUST NOT expect a second
signature.** [D] This is the escape hatch that makes exit a real right.

### 4.2.1 Lateral and vertical shifts are not a separate type

Moving to a grandpatron, or to a patron's sibling, is **an ordinary adoption
whose counterparty happens to be nearby**. It warrants no type of its own.

But it has a property worth stating: when the new patron lies **inside the old
patron's replication horizon**, it already holds the node's history through
sibling replication (§4.4), so **no archive presentation or re-verification is
needed and trust history is preserved**. Implementations MUST NOT force
re-verification in this case.

This is **derivable, not declared.** An observer holding the relevant topology
computes the distance between old and new patron itself. A self-asserted flag
would merely be something to lie about.

Expected uses are application-level: teams rebalancing, or onboarding by having
one member perform an adoption with proof of presence and then redistributing.
**The network is deliberately opinion-free about structure; applications built on
it will not be.** Reporting paths, org shape and delegation will matter to them
even though they matter to no protocol rule.

### 4.3 Disavowal (type 3)

```
{
  1: keyhash,          ; patron issuing
  2: keyhash,          ; subordinate being disavowed
  3: timestamp,
  5: ? uint            ; reason code, 0..63 see below. Key 4 unused, not reused
}
```

[D] Single signature.

**There is no notice period, and field 4 is withdrawn.** [D] The field
was added against a question design §6.2.2 recorded as *undecided*, so the schema
carried a mechanism while the design said none was waiting on a value. It also could
not have worked: a disavowal is the issuer's own signed statement and nothing stops
them signing it whenever they choose, so an effective-at date is a claim about the
issuer's intentions rather than anything a recipient can check (design §1.1).

**A party issuing a durable protocol-level negative attestation about another party
must express its basis only through a bounded, machine-interpretable category whose
adverse character is structurally visible, and must not attach arbitrary public
accusation text.** [D] Present encoding: **field 5 is an enumerated code, never free
text.** [D] Trust policies may
reasonably weight the stated reason, but free-form text on a permanently published
record is a defamation surface with no recourse mechanism. Enumeration also keeps
it machine-evaluable.

**Disavowal reason codes are an exception to §1's unknown-enum rule.** [D] An
unfamiliar code in 0–63 is **retained and evaluated by its band**, not rejected —
the banding exists precisely so a policy can act correctly on a code it does not
recognise. Rejecting the transaction would make every future code a flag day.

**The code space is 64 values, banded so that prejudice is structural.** [D] Bit 5 carries the distinction: **codes 0–31 are without prejudice,
32–63 are with prejudice.** A policy can therefore evaluate an unfamiliar code
correctly — `code >= 32` means the patron made an adverse judgment about the
subordinate — without a lookup table and without a specification update. Most
values are undefined in v1 and will be assigned into the band where they belong.

**Without prejudice (0–31).** The relationship ended, and nothing is being alleged
about the node:

| Code | Meaning |
|---|---|
| 0 | Inactivity |
| 1 | Voluntary withdrawal, the node asked, or has gone and the patron is tidying up |
| 2 | Pruned for space — inadequate room at the node's current position |
| 3 | Merge trim, the node held membership in both of two merging subnets and must keep one |
| 4 | Incompatible subnet membership, the node belongs to a subnet the patron deems incompatible |
| 5 | Cycle repair — the patron relation formed a cycle and this edge was cut to break it (§7.2b) |
| 6–30 | Unassigned |
| 31 | Other, without prejudice |

**With prejudice (32–63).** The patron is making an adverse judgment, and this is
**the network's negative attestation, and the only one** (design §6.2.2):

| Code | Meaning |
|---|---|
| 32 | Abuse of resources |
| 33 | In-network conduct |
| 34 | Out-of-network conduct |
| 35–62 | Unassigned |
| 63 | Other, with prejudice |

**Note code 4 sits without prejudice deliberately.** A patron judging another
subnet incompatible is asserting something about *that subnet*, not about the
subordinate, who may have joined it entirely reasonably.

**Codes carry inference risk beyond their text.** `32` beside a known resource and
a revocation date reconstructs specifics the enumeration was meant to withhold
(design §14.5.8, C13). The banding does not change this; it makes the *severity*
legible without making the *particulars* so.

### 4.4 Peering (type 4)

```
{
  1: keyhash,          ; infra node A
  2: keyhash,          ; infra node B
  3: NetworkPoint,     ; A
  4: NetworkPoint,     ; B
  5: timestamp,
  6: ? uint,           ; replication commitment, bytes
  7: ? [ * Audit ]     ; most recent few only (design §6.3)
}

NetworkPoint = {
  1: bstr,             ; IP address, 4 or 16 bytes
  2: ? uint,           ; ASN. U32 RANGE per RFC 6793 4-byte ASNs
  4: ? uint,           ; UDP port, u16 range. Absent means the default 7431
  3: ? bstr            ; routable prefix
}

Audit = {
  1: timestamp,
  2: bool,             ; passed
  3: keyhash           ; challenger
}
```

[D] ASN and prefix are exposed deliberately so policies can weight
network diversity, and so concentration (many nodes in one ASN) is observable.
[D] The audit list is pruned to the most recent few; peering is a status
rather than a trust-bearing history.

### 4.5 Presence record (type 5)

Field-for-field per design §7.2.

```
{
  2: timestamp,        ; finalized_at
                       ; key 1 (started_at) is DISCLOSABLE — see §4.5.1.
                       ; key 3 unused, not reused
  4: [ Participant, Participant ],
  8: [ * Witness ],
  9: [ * VerifierResponse ],
  11: uint,            ; seed window ordinal = floor(unix_seconds / 86400)
  12: bstr .size 32    ; disclosure root (§4.5.1) — commits to every disclosable
                       ;   field. Fields 1, 5, 6, 7 and 10, and Participant
                       ;   fields 4 and 5, moved out of the body and are
                       ;   committed here instead

  ; key 0 (chain back-pointers) is common to all bodies (see §3.1).
  ; NOTE: no patron signature appears in a presence record. Patrons do not
  ; countersign proof of presence (design §6.4)
}

Participant = {
  1: keyhash
                       ; keys 2 and 3 unused, not reused. Retention and client
                       ;   integrity are DISCLOSABLE and travel in the
                       ;   disclosure set (§4.5.1)
}

ClientIntegrity = { 1: bool, 2: uint, 3: ? bstr }   ; attested, scheme, evidence

Proximity = {
  1: [ + Channel ],
  2: uint              ; strongest channel that passed
}
Channel = {
  1: uint,             ; 1 uwb, 2 nfc, 3 optical, 4 latency
  2: uint,             ; 0 pass, 1 fail, 2 unavailable
  3: ? uint,           ; claimed resolution, metres
                       ; method values: 0 GNSS, 1 serving-cell, 2 network egress,
                       ; 3 latency bound. Unassigned values are retained and left
                       ; uninterpreted, the list is open (§1)
  4: ? bstr            ; session-key binding, where the channel provides one
}

Capture = {
  1: uint,             ; modality: 0 still, 1 stereo, 2 depth
  2: uint,             ; image count (3–5)
  3: uint,             ; liveness: 0 pass, 1 fail, 2 not-performed
  4: uint              ; liveness algorithm version
}

LocationEvidence = {
  1: [ * Asserted ],
  2: [ * Corroboration ]
}
Asserted      = { 1: uint, 2: tstr }              ; method, geohash (3–4 chars)
Corroboration = { 1: keyhash, 2: uint, 3: uint }  ; witness, method, radius_km

Witness = {
  1: keyhash,
  2: keyhash,          ; nominated_by — MUST be the counterparty (design §7.1.1)
  3: uint,             ; attestation bitfield:
                       ;   bit 0 protocol_ran
                       ;   bit 1 both_responsive
                       ;   bit 2 latency_bound
                       ;   bits 3+ reserved; a decoder retains them and
                       ;   interprets only 0-2
  4: bstr .size 32,    ; nonce commitment, published BEFORE capture
  5: bstr .size 32     ; revealed nonce, published AFTER capture (design §7.2.2)
}

VerificationQuery = {
  1: keyhash,          ; subject
  2: keyhash,          ; querier
  3: bstr .size 32,    ; ceremony pre-commitment (design §7.1.4)
  4: bstr .size (1..4096),   ; fuzzed profile
  5: bstr .size 32     ; query_id
}

VerifierResponse = {
  1: keyhash,          ; verifier
  2: keyhash,          ; subject
  3: bstr .size 32,    ; query_id — matches VerificationQuery field 5
  4: uint,             ; 0 match, 1 no-match, 2 inconclusive,
                       ; 3 unavailable, 4 pending
  5: uint,             ; basis: 0 photo_match, 1 personal_knowledge, 2 both
  6: ? uint,           ; template version. REQUIRED when basis is 0 or 2;
                       ; MUST be absent when basis = 1. The optional marker is
                       ; syntax; the basis determines presence
  7: COSE_Sign1,       ; SUBJECT's countersignature. Payload: query_id (field 3),
                       ; NOT the query itself (see §4.6.6).
                       ; external_aad = "rhtn/1:consent".
                       ; A verifier MUST reject a query lacking it, and MUST reject
                       ; one whose fuzzed profile differs from another countersigned
                       ; under the same ceremony pre-commitment
  8: ? keyhash,        ; PRIOR identity being matched against. REQUIRED when this
                       ; response appears inside a Recovery block (§4.1), absent
                       ; otherwise. **MUST equal that Recovery's `prior_key`** —
                       ; without the equality check, evidence collected about old
                       ; identity X can be embedded under a Recovery claiming old
                       ; identity Y, and every signature still verifies. Without it the verifier's signature never
                       ; names the old identity it is attesting continuity with,
                       ; and the assertion the recovery rests on is unsigned
  9: COSE_Sign1        ; BY THE VERIFIER. Payload: canonical CBOR of fields 1-8 of
                       ; THIS map. External_aad = "rhtn/1:verifier"
}

**Both signatures here are `COSE_Sign1` and classical-only**, unlike envelope
signatures. They are evidence embedded inside a hybrid-signed body, so
substituting them breaks the envelope signature and their authenticity is
protected transitively (see design §5.1). Hybridising all 64 signatures costs ≈ 211 KB against ≈ 4 KB classical.
```

**Field 10 is load-bearing** [D]: a formation record has empty witness
and verifier arrays permanently, and **must never age into looking like a normal
record**. Typing it explicitly means no policy can mistake self-attestation for
independent attestation.

**Deliberately absent** [D, design §7.2]: biometric templates, photographs, raw
latency samples, precise coordinates. Everything identifying stays on the
participants' devices.

---

### 4.5.1 Selective disclosure

**A holder can present a presence record without the fields a given recipient has no
use for.** [D] Scoped deliberately: this hides **location, position,
retention, client integrity, capture parameters, proximity channels, start time and
subtype**, and hides **nothing else**. See design §7.2.1 for what it does not reach
and why.

#### The construction is a digest list, not a tree

**Each disclosable field becomes a salted digest; the body commits to the sorted
list of digests.** [D]

```
Disclosure   = [ bstr .size 16, tstr .size (1..32), any ]
                 ; salt, label, value
digest(D)    = SHA-256( 0x00 || deterministic CBOR of D )
root         = SHA-256( 0x01 || concatenation of all digests, ascending by label )
```

`root` is body field 12. **Labels are the field's path**, so a decoder knows what it
is looking at without a table: `started_at`, `proximity`, `capture`, `location`,
`subtype`, `p0.retention`, `p0.integrity`, `p1.retention`, `p1.integrity`.

**Why not a Merkle tree.** design §14.5.3 proposed one, following SD-JWT loosely. At
the leaf count here — nine for a typical record — a tree buys nothing: inclusion
proofs would cost four hashes each where sending every digest costs nine, and a tree
adds real hazards a flat list does not have, **odd-node handling and the
duplicated-node second-preimage class**. SD-JWT's own construction is a digest array
for the same reason. **The 0x00 / 0x01 prefixes are still required**, so that a
digest can never be reinterpreted as a root or the reverse.

#### Salts are mandatory

**Every disclosure carries a fresh 16-byte salt.** [D] Without one, an undisclosed
field is recovered by brute force from its digest: `subtype` has two values,
`liveness` three, a precision-3 geohash about 32,000, and a timestamp is guessable
within the ceremony window. **A salted digest is the only thing that makes withholding
mean anything here.**

**Salts are agreed during the ceremony**, because both participants sign one body and
must therefore compute one root. They are ordinary record state afterwards, held by
both participants and by anyone given a full record.

#### What travels

| Form | Carries |
|---|---|
| **Full record** | body + every `Disclosure` |
| **Minimised** | body + the revealed `Disclosure`s + the **digests** of the withheld ones |
| **Fully withheld** | body + all nine digests |

**A recipient verifies by recomputing `root`** from what it holds — revealed
disclosures hashed, withheld digests taken as given — and checking it equals field 12,
which the envelope signature covers. A mismatch means the presentation is malformed,
not that a field is missing.

**Withholding is visible, and that is deliberate.** The digest count and the labels
are always present, so a recipient always knows a field exists and was withheld. This
is the same posture as §4.6.5's `pending` and `unavailable` verifier responses:
absence is legible rather than silent, and a policy may weight it.

#### Cost

**+16 bytes per disclosable field at rest** — about 144 bytes on a ~35 KB record,
**0.4%**. A minimised presentation carries 32 bytes per withheld field, at most 288
bytes. **Presentation size does not otherwise fall**: a presence record is ~96%
signatures and the envelope requires exactly the required signer set, so a minimised
record is still ~34 KB. This is a disclosure measure, not a bandwidth one.

#### What a decoder MUST do

- **Reject a record whose recomputed root does not equal field 12.**
- **Reject duplicate labels**, and a label outside the set above.
- **Reject a `Disclosure` whose salt is not exactly 16 bytes.**
- **Accept any subset of disclosures, including none.** A minimised record is
  well-formed; only a root mismatch is malformed.
- **Never treat a withheld field as a default value.** Withheld is not zero, not
  absent, and not `unavailable` — it is unknown, and §4.5.2 says which recipients may
  require it.

### 4.5.2 Which exchanges see the disclosable fields

**Stated per exchange, because a holder needs to know what a given recipient will be
able to read.** design §7.2.1 carries the same table with the reasoning.

| Exchange | Disclosable fields |
|---|---|
| Ceremony, at creation (design §7.1.1) | **All.** Both parties construct the body |
| Witness signing (design §7.1.1) | **Location only**, which the witness corroborates |
| Verification by query (§4.6.6, design §7.1.3) | **None.** A verifier receives a fuzzed profile and a query id, never the record |
| Verifier-selection recomputation (§4.6) | **None.** Seed inputs are body fields 4, 8, 11 |
| Finalization threshold (§4.6.5) | **None.** Counts field 9 |
| Structural verification (§3.2) | **None.** Signatures, back-pointers, timestamps, participant distinctness |
| Adoption's proof-of-presence reference (§4.1 field 8) | **None.** Confirms the record exists and names these two parties |
| Archive fetch by a prospective patron (§5.8) | **Holder's choice.** The only exchange with a use for location |
| Presence-based recovery (§4.1 `Recovery`) | **None.** Reads field 9 |
| Late verifier response (§5.3b) | **None.** References `txid` |
| Segment key grant (§5.3a) | **None.** References `txid` |

**Ten of eleven exchanges need none of it**, which is what makes the mechanism worth
its 0.4%. **A conforming client withholds by default and reveals on the holder's
instruction**, rather than the reverse.

**No exchange may demand a disclosable field as a condition of proceeding.** [D] The
interface for each exchange above is fixed, and design §1.1 makes the evidence schema
the one thing that is not pluggable — a recipient weights what it receives, and cannot
make an interface carry what the interface does not define.

### 4.6 Verifier selection — recomputation

**Previously unspecified, and the record did not carry the inputs.** design §7.2.2
requires that selection be recomputable by any party holding the subject's history,
so a missing verifier is
visible, but the nonce commitments and reveals it depends on were nowhere in the
presence record, so the anti-suppression property could not be checked at all.
[D]

### 4.6.1 Nonce commitment

```
commitment = SHA-256("rhtn/1:nonce-commit" || witness_keyhash || nonce)
```

The nonce is exactly 32 bytes. Domain separation and the witness identity are
included so a commitment cannot be replayed by a different witness.

A validator MUST recompute every commitment from the revealed nonce and reject the
record on any mismatch.

### 4.6.2 Recomputability invariant — binding on any future change to seeding

**Each participant selects the other's verifiers.** A queries B's prior
counterparties to establish that B is B, and B queries A's. So the party who
*performs* a selection is never the party the selection is *about*.

**The subject of a selection MUST be able to recompute it from the presence record
plus their own history, and from nothing else.** State it from the subject's side,
because the subject is the party with the interest: **B must be able to demonstrate
that B's verification was honestly conducted.** If B cannot recompute the selection
A performed, B can never clear themselves, the record is immutable, and no later
action fixes it.

Two consequences, both normative:

- **The seed MUST draw only on values carried in the record.** Not on the
  counterparty's archive, not on either party's chain head, not on network state,
  device state, or anything else about the moment of the ceremony that a validator
  years later cannot reproduce.
- **The candidate set MUST come only from the subject's own history**, never from
  the counterparty's.

**Each party MUST verify the other's selection before signing.** [D] If A selects
B's verifiers off-seed and B signs regardless, **B is left holding a record that
fails recomputation permanently**. B cannot repair it afterwards. This check
protects the signer *against their counterparty*, which is the opposite direction
from the rest of the ceremony's checks — every other one guards against outsiders
or against the pair colluding. A client that signs without performing it exposes
its own user.

**Why this needs stating rather than assuming.** The construction below satisfies
it, but nothing about the construction announces the requirement. Adding, say, a
hash of the counterparty's chain head to the seed for extra entropy would leave it
deterministic, unpredictable and ungrindable — every property design §7.2.2 asks for —
while making selection **silently unverifiable** by anyone who does not hold the
counterparty's archive. The failure would be invisible in testing, because a
developer with both archives sees everything work.

### 4.6.3 Seed

```
seed = SHA-256(
    "rhtn/1:verifier-seed"
 || min(participant_a, participant_b)      ; 32 bytes, bytewise comparison
 || max(participant_a, participant_b)      ; 32 bytes
 || window_ordinal                          ; 8 bytes, big-endian
 || for each witness in ascending keyhash order:
        witness_keyhash || revealed_nonce   ; 64 bytes each
)
```

**Participant order is canonicalised** so that which party is listed first cannot
become another grinding variable.

**The window ordinal is `floor(unix_seconds / 86400)`.** A 24-hour window, epoch
aligned. [D] This fills the parameter design §15 marked UNSET. Long is safe here: an
honest retry inside the window reproduces the *same* sample, which is exactly what
retry should do, while an attacker aborting to reroll gets **one fresh sample per
day per participant pair** (design §7.2.2).

### 4.6.2.1 Witness nonce derivation — required for the anti-grinding property

**A witness derives its nonce deterministically from the participant pair and the
window**, not freshly per attempt: [D]

```
nonce = PRF(witness_secret, "rhtn/1:wnonce" || min(a,b) || max(a,b) || window_ordinal)
```

`witness_secret` is the witness's own long-lived secret and never leaves it. Any
PRF with a 32-byte output is acceptable; HMAC-SHA-256 is the expected choice.

**Without this the anti-grinding property does not exist.** §4.6.2 claims an honest
retry within the window reproduces the same verifier sample while an aborting
attacker gets one fresh sample per day per pair. Both depend on the nonce being
stable across attempts, a witness generating fresh randomness each time hands a
grinding participant a new sample per abort, which is the attack the commit-reveal
was introduced to close.

**A validator cannot check this from a record**, and does not need to. The property
rests on the witnesses being honest, and **cross-nomination is what supplies that**
(design §7.1.1): a participant does not choose their own witnesses, so grinding
requires the *counterparty's* nominees to collude, the same bar as forging the
ceremony outright.

### 4.6.3.1 The window, the horizon, and who is eligible

Six definitions the selection rule depends on and did not carry. [D]

- **`window_ordinal` is derived from `started_at`**, not `finalized_at`. The seed
  must be fixed before capture completes, or a participant could steer it by
  controlling when the record finalises.
- **"Two years" is 730 days**, measured back from the record's `started_at`. Not a
  calendar interval — calendar arithmetic differs across implementations and
  timezones for no benefit here.
- **The lower bound saturates at zero.** Timestamps are unsigned; for a record
  less than 730 days after the epoch the subtraction would underflow, and the
  window simply starts at zero. Irrelevant in practice, and cheaper to state than
  to leave each implementation to discover.
- **The window is open at the far end and closed at the near end**: a prior record
  counts if `started_at - 730d < its finalized_at < started_at`. **A record
  finalising exactly 730 days earlier is out.** The prose and the formula must
  agree at the boundary instant, or two implementations resolve it differently.
- **The candidate horizon is the same window.** A counterparty met more than 730
  days ago is not a candidate, even though the meeting remains in the archive
  permanently — *n* and the candidate set are drawn from the same interval, and
  drawing them from different ones would let the threshold exceed the pool.
- **The current counterparty is never a candidate for their own verification.**
  They are the party being established; asking them is not evidence.
- **Formation records count**, both toward *n* and as candidates. They record real
  meetings; what they lack is corroboration, which is a weight question for policy
  (design §13.1), not a structural one.

  **A briefly-adopted exclusion is withdrawn.** It was introduced to stop candidate-
  pool flooding, and the flooding is prevented by the one-formation-per-key rule
  above rather than by excluding honest bootstrap meetings from the history they
  belong to.


### 4.6.4 Candidate set and sampling

***n* counts distinct presence transactions reachable from the back-pointer this
record commits for that subject**, each once. [D] The archive is
a Merkle DAG after a merge (design §8.3), so a transaction reachable by several
merge paths is still one transaction — traverse the reachable predecessor set and
count each `txid` once. An implementation written when the archive was a chain would
double-count across merged branches and derive a different threshold.

**The root of that traversal is the committed back-pointer, not the subject's
current head.** [D] The record's key 0 fixes each signer's predecessor at signing
time, and that is the only history the record itself attests to. **Counting from a
current head lets a subject backfill**: append records after the ceremony, present
the enlarged archive to a later evaluator, and *n* — and therefore the threshold the
record was supposed to meet — comes out different from what any witness saw. The
committed back-pointer is inside the signature and cannot move.

**Candidates are distinct prior counterparties, deduplicated by keyhash.** A
subject who met the same person twenty times has one candidate, not twenty — the
threshold counts *transactions*, the candidate set counts *people*, and conflating
them was left open.

```
required(subject) = min( floor(n / 2), 10, |candidates| )
```

`floor`, explicitly, an unstated rounding differs by one required response at
every odd *n*. **The `|candidates|` term is necessary**: without it a subject with
twenty meetings against one counterparty needs ten verifiers from a pool of one.

Sampling is by hash rank:

```
rank(c) = SHA-256(seed || subject || c)
```

Candidates are ordered by ascending `rank`, ties broken by ascending keyhash, and
the first `required(subject)` are selected. **Exactly that many are queried.** The
selected set and the finalization threshold are the same size, which is what makes
"a missing verifier is visible" precise.

### 4.6.5 What counts toward finalization

**Finalization counts structurally valid responses and never inspects their
content.** [D] Whether a response reports a match, a failure, or an inability to
answer, it occupies its slot — content is evidence weighed by policy (design
design §12.1), not an input to structural validity. Present encoding: `pending` and
`unavailable` count alongside `match` and `no-match`.

The alternative — counting only completed comparisons — hands an attacker who can
make verifiers unreachable a way to block finalization indefinitely. A record
finalized on ten `pending` responses is visibly weak to any evaluator, which is the
correct place for that weakness to be handled.

**Duplicate responses from one verifier for one subject are malformed**, so a
single verifier cannot occupy multiple slots.

**Validation is per-subject, and a validator may hold one history and not the
other.** [D] Structural and cryptographic checks cover the whole record; the
selection and threshold checks are computed **per participant** against that
participant's history. A holder with one subject's archive can verify that half and
must report the other as **unverifiable** — which is neither valid nor invalid, and
collapsing the two lets a caller overclaim what it checked (§4.1).

**`inconclusive` covers a failure to decrypt, and `no-match` never does.** [D] A
truncated or unauthenticated sealed capture, or an absent segment key, tells the
verifier nothing about the subject. **Reporting it as `no-match` would turn a
corrupted store into adverse evidence**, which is the one outcome a storage fault
must not produce. `unavailable` is for having no capture at all; `inconclusive` is
for having one it could not read.

**A response is structurally valid with respect to its query when its `query_id`
matches one the subject countersigned, its `subject` names one of the two
participants, and its `verifier` is in the selected set for that subject.**

**A response failing any of those makes the record malformed** where the validator
holds enough history to determine the selected set, not merely unweighted. [D] The
same treatment as duplicates, and for the same reason: "present but ignored" leaves
room between the 32-entry array bound and the 20 legitimate responses for a
participant to pad the record with material nobody asked for. A validator lacking
the subject's history cannot make this determination and treats the responses it
cannot place as unverifiable rather than invalid.

### 4.6.6 Consent is signed over the query id

`query_id = SHA-256(canonical CBOR of the VerificationQuery)`.

The subject's `COSE_Sign1` (field 7 of `VerifierResponse`) signs **`query_id`**,
not the query itself. The query, which carries a fuzzed profile up to 4 KB — is
therefore not carried in the record, while the signature remains verifiable from
it, because the id commits to the query's full contents including the profile.

Carrying the query itself would add up to 64 KB to a record with sixteen responses,
for no verification benefit.

### 4.6.7 Who can verify what, the property is holder-relative

**Anti-suppression is per-subject, and an evaluator can check only the subjects
whose history it holds.**

To check that subject A's verifier set is complete, an evaluator needs A's
candidate set, which comes from A's archive. An evaluator holding A's history but
not B's can verify A's half of a record and **not** B's.

**This is the right scope, not a shortfall.** An evaluator assessing A cares
whether *A* suppressed verifiers; B's verification is evidence about B. A record's
two halves are independently checkable by different parties, and **nobody verifies
both unless they hold both archives.** Which, given that archives are presented
selectively to parties one is dealing with (design §13.7), is uncommon.

**design §7.2.2 states the same thing.** Recomputation is available to any party
**holding that subject's history**, and it notes that *"recomputable by any third
party"* would be stronger than achievable. A stranger holding only the record can
verify signatures, structure and the seed, but cannot determine which verifiers
*should* have appeared.

### 4.7 Resource registration (type 6), the catalog, and abuse reports

CatalogEntry = {
  1: keyhash,        ; resource identity
  2: keyhash,        ; owner
  3: tstr .size (1..64),   ; service type. DNS-SD style, and MATCHED
                     ;   BYTE-FOR-BYTE: no case folding, no Unicode
                     ;   normalisation, no subtype grammar. A registry of
                     ;   conventional names may grow socially; the protocol
                     ;   compares bytes
  4: tstr .size (1..128),  ; instance name, for display only. Never matched
                     ;   against and never unique
  5: bstr .size (1..256),  ; connection ENDPOINT, the SRV-equivalent (host/port or
                     ; protocol-specific address). RFC 6763 keeps endpoint data
                     ; out of TXT; this field is the SRV analogue
  6: ? Scope,        ; connect_scope. ABSENT means no prediction is offered,
                     ;   neither allow-all nor deny-all. ADVISORY: tells the asker whether a
                     ;   connection is likely to be accepted, so a client can
                     ;   present the entry accordingly. The owner decides at
                     ;   request time regardless (§7.3)
  7: ? bstr .size (1..1024),  ; additional service metadata, the TXT analogue.
                     ;   SHOULD NOT duplicate field 5 — an uninterpreted byte
                     ;   string cannot be checked for semantic duplication, so
                     ;   this is guidance to a publisher and not a decoder rule
  9: ? uint,         ; data_practice — the owner's DECLARED logging and retention
                     ;   posture (design §9.5). Small enumeration, below.
                     ;   OPTIONAL: absent means undeclared, which is itself
                     ;   informative and is NOT equivalent to any declared value
  8: COSE_Sign1      ; by the OWNER over fields 1-7 AND 9 of THIS map only.
                     ;   Classical component alone — an entry's relevance ends
                     ;   when the owner stops returning it (§5.1). It does NOT
                     ;   cover the enclosing transaction's key 0: the same entry
                     ;   is returned in query replies where no transaction exists
                     ;   around it, and a signature covering the body would be
                     ;   unverifiable there
}
```

**`data_practice` values** [D]. Deliberately few and structural rather
than reassuring, on the same reasoning as §4.3's disavowal codes: a declaration a
publisher can make specific enough to be checked socially, and no finer.

| Value | Declared |
|---|---|
| 0 | No access log is kept |
| 1 | Access logged, retained for a stated period the owner publishes out of band |
| 2 | Access logged, retention unstated |
| 3 | Access logged and shared with a third party |

**Unknown values are retained and surfaced, not rejected**, on §4.3's reasoning:
rejecting an unfamiliar declaration would make every future value a flag day, and a
client that cannot interpret one should show the user that a declaration exists and is
unrecognised — which is more informative than absence.

**Four values, deliberately coarse.** [D] A resource may be
anything from a shared drive to a social network to a persistent agent, so its security
posture is particular to it in a way no enumeration can track. **A granular or
exhaustive taxonomy would be counterproductive** — it would multiply values nobody
maps consistently and give a reader false confidence that the categories mean the same
thing across two resources. Four coarse bands a publisher can be held to socially are
worth more than twenty nobody applies the same way.

**Nothing checks it.** The protocol has no view of what a resource logs (design
§9.0.3) and cannot acquire one. The field makes an owner's claim signed and portable so
a policy can weight it, which is design §1.1's move where enforcement is unavailable —
the same as client-integrity attributes. **A false declaration is undetectable** and is
a matter between the owner and whoever relied on it.

```

AbuseReport = {
  1: keyhash,        ; resource — and the signer. A resource reports; its owner
                     ;   receives (design §9.6). Key 2 unused, not reused
  3: timestamp,
  4: uint,           ; 0 unavailable | 1 malfunction | 2 excessive-load
                     ; 3 unauthorised-access-attempt | 4 content | 5 other
  5: ? bstr .size (1..1024),   ; detail, resource-defined and uninterpreted by
                     ; the network. **Bounded, and deliberately small.** The
                     ; report goes to the resource's own owner, who already holds
                     ; the context, so the bound is not about what the recipient
                     ; learns — it is that a signed object is **portable** and the
                     ; owner may hand it to anyone (design §9.6, P27). A
                     ; resource needing more should reference its own record
                     ; rather than inline it.
                     ; An application wanting to name which of ITS users
                     ; complained puts that in here, as its own schema. It is
                     ; application data: the network has no user-signed report,
                     ; because that would require a user's network client to
                     ; interoperate with arbitrary third-party applications
  6: COSE_Sign1      ; BY THE RESOURCE named in field 1. The signing key's
                     ;   keyhash MUST equal field 1 — otherwise the object
                     ;   attributes a complaint to a party that did not make one
}
```

**The query and its answer.** [D]

```
CatalogQuery = {
  1: ? tstr .size (1..64),  ; service type filter, matched byte-for-byte
                       ;   against §4.7's field 3. Absent means everything the
                       ;   asker may see. Same bound as the field it matches —
                       ;   a filter longer than any legal type cannot match and
                       ;   should not be allocated for
  2: bstr .size 16     ; nonce, echoed in the reply
}

CatalogReply = {
  1: bstr .size 16,    ; echoes the query nonce
  2: [ * CatalogEntry ],  ; bounded at 64; an owner with more than 64 visible
                       ;   entries for one asker returns 64 and sets field 3
  3: ? tstr .size (1..64)  ; TRUNCATION CONTINUATION: present iff entries were
                       ;   omitted, carrying a service type to ask for next.
                       ;   Lets an asker drain the catalog without knowing the
                       ;   types in advance. A hint, not a cursor: the node keeps
                       ;   no state
}
```

**Truncation must make progress, so selection is not free.** [D] An answering node
**orders qualifying entries by resource keyhash and returns the first 64**, and the
continuation names the type of the first entry it withheld. Without an order, two
queries could return the same 64 and the same hint forever — **an asker following
the continuation would loop rather than drain.** The order is arbitrary and that is
fine; it only has to be *stable*.

**A filtered query is answered from the same order**, so a type filter narrows the
qualifying set and the asker makes progress within it.

**Carried on a bidirectional stream** (§7.2) tagged request type 5, a query per
stream, with the reply closing it. **A response larger than the frame bound is sent as a sequence.** [D] Field 1 = 0
may repeat on the same stream, each carrying the next portion of one HTTP message,
with the stream closing when it ends. **No length is declared up front**: a resource
streaming a response does not know one, and requiring it would forbid exactly the
case the sequence exists for.

**A non-zero status ends the exchange** and may not be followed by further frames.

**Nothing distinguishes a truncated set by rank**: when more than 64
qualify, which 64 are returned is the answering node's choice, and an asker must not
infer priority from inclusion.

**Preserved unknown keys are bounded like anything else.** [D] §1 requires unknown
map keys to survive re-serialisation, which makes them attacker-supplied storage on
a signed object a node retains — 16 per map and 1 KB per value. **Extension
tolerance is not unbounded tolerance**, and a rule that admits arbitrary bytes into
retained state is a denial-of-service surface however well-intentioned.

**Unsigned session messages reject unknown map keys** — `CatalogQuery`,
`CatalogReply`, `ResourceRequest`, `ResourceResponse` and the control frames of
§6.0 alike. [D] The
preserve-unknown-keys rule (§1) exists so an extension survives re-serialisation for
signature verification. **Neither of these is signed and neither is re-serialised**,
so there is nothing for preservation to protect — and accepting unknown keys on an
unsigned message is accepting unbounded input from an unauthenticated peer.

**An asker outside the answering node's horizon is refused before the application
reply.** [D] Close the stream; do not return an empty `CatalogReply`. **An empty
reply is a true statement — nothing is visible to you — and it is the wrong one**,
because it is indistinguishable from a node that hosts nothing, and it invites an
asker to conclude the catalog was answered. A node
answering from a map has no natural order to offer, and requiring one would be a
sorting obligation with no consumer.

**An entry is signed once, at registration, and that signature is reused for every
answer.** [D] No field varies per query, so re-signing buys nothing — and a
per-answer signature would make an entry's bytes differ between askers, which
defeats the attributability that signing is for.

**A catalog entry is a query answer, not a propagated record.** [D] A
node asks an infra node within its horizon what it has; the infra node replies with
the entries it **owns** and that the asker may see. **Nothing floods, nothing is
cached authoritatively, and nothing needs invalidating.**

**Concurrent re-registration is resolved by the answering node, not by ordering.**
[D] Two registrations for one resource keyhash arriving close together have no total
order to appeal to — nothing timestamps them authoritatively. **The node holding the
entry keeps whichever it applied last and answers with that**, and since nobody else
holds a copy there is nothing to reconcile. An owner who cares about which won can
query and see.

**Concurrent re-registration is settled by the answering node, not by ordering.**
[D] Two registrations for one resource keyhash have no authoritative total order —
nothing timestamps them. **The node holding the entry keeps whichever it applied
last**, and since nobody else holds a copy there is nothing to reconcile; an owner
who cares which won can query and see.

**Re-registering a resource replaces the current entry.** [D] An owner holds **one
current entry per resource**; a new type-6 transaction for the same resource keyhash
supersedes the previous one locally, and the archive keeps both because it keeps
everything. **The archive is history; the catalog is state.** Nothing on the wire
needs to express the replacement, because nobody else holds a copy to reconcile.

**So there is no propagation lifecycle to specify, and no `seqno` or withdraw
operation.** Those would exist to handle supersession, stale copies and competing
registrations, and **none of those conditions arise**: an owner that has withdrawn a
resource simply stops returning it, and the next query gets the truth.

**Freshness is inherent rather than maintained.** Each answer is computed when
asked, by the party that knows.

**`discover_scope` is gone from the schema.** It decided which entries an owner
returns to which asker — a filtering rule evaluated **at the answering node**, never
read by the recipient, since receiving an entry is what qualifying looks like. Like
the role table (design §9.4), it is local state, and a field carrying it would be
telling the asker how they were selected.

**`connect_scope` remains and is advisory.** It lets a client show whether a
connection is likely to succeed rather than presenting every entry identically. The
owner decides at request time regardless (§7.3), so a client that ignores the field
is wrong about presentation and never about access.

**A requester may cache what it was told**, and holds an answer that was true when
given. That is ordinary staleness with no protocol consequence: the next query
corrects it, and nothing grants access on the strength of a cached entry — access is
decided by the owner at request time (§7.3).

**Nothing prevents two owners registering the same resource keyhash**, and nothing
needs to. [D] An entry is a claim by its owner, verifiable as theirs; a reader
holding two such claims holds two claims, and **the resource keyhash is not a
namespace anyone allocates.** Which one a reader acts on follows from whose catalog
answered — and each answer came from a node the reader chose to ask.

**The owner alone signs**, so a cached or forwarded entry remains attributable. The
resource proves nothing and supplies no key material: the owner is asserting the
resource's identity, which is the only assertion a catalog carries.

**A scope the evaluator cannot compute is structurally valid and ineffective.** [D
— 2026-08-23] A decoder MUST NOT reject it.

**Rejection is wrong because validity is not local.** The same entry is valid for a
node that can compute the position and not for one that cannot — so rejection would
make structural validity depend on the reader's topology, and a node would reject an
object its neighbour accepts. **Store it, forward it if the forwarding rule says to,
and grant nothing from it.**

**No scope reaches outside the owner's Dunbar Org**, `list` included (design §9.4).
A scope naming a position outside it is not an error; it simply matches nobody the
evaluator can see.

**The patron relationship is formed bilaterally and ended unilaterally by either
party.** Every non-root node has exactly one patron; roots have none, which is an
ordinary state (design §10.7). Adoption requires both signatures; departure and disavowal are the two
one-sided ends of the same relationship.

**There is no transfer transaction.** [D] Since the protocol has no
concept of "a node's set of patrons," dropping an old patron was never a network
operation — what distinguished transfer was the dropping, which is now either a
departure (node-initiated) or a disavowal (patron-initiated). Moving between
patrons is: adopt at the destination, then depart the origin, in either order and
with no requirement to do both.

---

## 5. Attestations and records

### 5.1 Currency attestation

```
CurrencyAttestation = {
  1: keyhash,          ; subject identity
  2: keyhash,          ; current key
  3: timestamp,        ; issued_at
  4: timestamp,        ; expires_at — ~10 h default; hours, not days (design §10.6.5)
  5: uint,             ; issuer role: 0 patron, 1 sibling (secondhand),
                       ;   2 grandpatron, 3 down-line threshold (root)
  6: keyhash,          ; ISSUER identity, the signature below is by this party,
                       ; who is not otherwise named
  7: COSE_Sign1        ; BY THE ISSUER over canonical CBOR of fields 1-6;
                       ; external_aad = "rhtn/1:currency". Classical only: an
                       ; attestation's relevance expires with it (~10 h), well
                       ; inside §5.1's post-quantum horizon.
}
```

Signed by the issuer. [D]

**Field 5 exists because issuance escalates** during patron outage, and a
consumer must be able to weight a secondhand attestation lower. Note the rule it
encodes: attestations are **issued fresh, never extended stale.** There is no
"extend" operation and no field for one, deliberately.

**Stapling** [D]: an introduction carries the attestation inline, so a
recipient verifies locally rather than querying. Genesis identities omit it
entirely — currency is vacuous with no history [D].

### 5.2 Forwarding record

```
ForwardingRecord = {
  1: keyhash,          ; subject
  2: Locator,          ; terminal position, chains already collapsed
  3: ? keyhash,        ; new key, when the move included a rotation
  4: timestamp         ; expiry — 90 day TTL (design §10.3)
}
```

[D] Held at the subject's former position; **repairs in transit**
rather than returning an error, and only at the point of divergence.

Authority comes from the adopting patron and its verifier attestations, **not
from the old key.** The old key may be precisely what was compromised
[D].

### 5.3 Anchor table entry

```
**An entry may only name a contactable infrastructure node.** [D] Any ancestor may
be *named* as an anchor in a locator (design §10.2), but an anchor **table** entry
requires routable endpoints, so a locator naming a light-client anchor cannot enter
this protocol — its holder must present one naming a reachable ancestor instead.

**First contact with an unpinned anchor is trust-on-first-use over an
unauthenticated identity.** [D] The requester has a keyhash from the table and no
key material — `AnchorEntry` carries neither. It dials, receives the peer's
`KeyMaterial` in the resolution reply (§5.6.2), checks its hash equals the intended
keyhash, and pins. **Until that check the peer is unauthenticated**, so a first
contact must disclose nothing beyond the query itself.

**Anchor entries are self-signed, and the signature is verifiable only once the
anchor's key is known** [D]. An entry carries the anchor's *keyhash*,
not its key, so **nothing in the entry lets a recipient check the signature on
receipt.** The key arrives at contact time (design §10.2, the table is an index,
not a credential store).

**So the signature gives retroactive attribution, not prior authentication.** A
node that reaches an address and obtains the key can then confirm the entry was
genuine, and if it was not, knows which gossip source supplied a forged one. That
makes injection **attributable** rather than **prevented**, which is a weaker claim
than an earlier draft made.

**The ingestion boundary must therefore be explicit.** An implementation that
treats "present in the table" as "verified" while its ingestion path does not
enforce that has a partition vulnerability with no visible symptom. State which it
is: either entries are verified on acceptance — possible only where the key is
already pinned — or the table holds unverified gossip and verification happens on
contact.

Freshness is by `seqno`, strictly greater to replace.

AnchorEntry = {
  1: keyhash,          ; 32
  2: [ + NetworkPoint ],
  3: uint,             ; subtree size. U64 RANGE, the design's ~4-byte sizing
                       ; arithmetic is a storage estimate, not a validity bound
  4: seqno,
  5: COSE_Sign1        ; BY THE ANCHOR over canonical CBOR of fields 1-4;
                       ; external_aad = "rhtn/1:anchor". Classical only
}
```

[D, design §7.2] **Key hashes, not keys.** Full PQ keys would blow the table by ~18×.
The table is an index, not a credential store; full keys are fetched and
verified at contact time.

Which anchors a node caches is **per-node policy, never a protocol constant**
[D].

### 5.3a Segment key grant

**The message by which a subject releases a capture segment key to a holder**
(design §7.1.5.2). It exists because nothing else in this profile carries one, and
three implementations would otherwise invent three.

```
KeyGrant = {
  1: txid,             ; the presence record whose capture is being unsealed —
                       ;   the holder may hold several for this subject
  2: bstr .size 16,    ; query_id this grant answers, binding it to one query
  3: bstr .size 32,    ; k_template
  4: ? bstr .size 32   ; k_images, absent for a template-only grant
}
```

**Carried as payload, never as a record.** [D] It travels over the end-to-end
encrypted path (design §11.2.4), which authenticates the sender to the recipient
and hides it from the transport. **It is never retained in the presence record and
never propagates**: a grant is a momentary release, and an object that persisted
would defeat the retention property the whole scheme exists for.

**Field 1 disambiguates which capture.** A verifier who has met the subject several
times holds several sealed captures; **the grant names the one to open**, and the
subject names the latest finalized eligible meeting in the committed history
(§4.5). Without field 1 the holder would guess.

**Field 2 binds the grant to a query.** A grant arriving unattached to a query the
subject countersigned is an unsolicited key release, and a holder should treat it as
malformed rather than as an invitation to open its store.

### 5.3b Late verifier response

**A verifier response that arrives after finalization is carried as a standalone
signed object referring to the record it supplements.** [D] Three
places said late responses "arrive as amendments" and no such object was ever
defined.

```
LateResponse = {
  1: txid,                 ; the presence record supplemented
  2: keyhash,              ; subject — one of that record's two participants
  3: VerifierResponse      ; the response itself, signed by the verifier as in §4.5
}
```

**It does not amend the record it names.** A presence record is immutable and its
`txid` fixes its content; this object sits beside it. **Finalization is unaffected**
— the record already met its threshold, and §4.5's structural rules governed what
counted at that moment. A late response is additional evidence an evaluator may
weigh, never a change to what was decided.

**The verifier must have been in the selected set** for that subject, and the
response must carry the same `query_id` the subject countersigned. Otherwise
anyone could attach unsolicited assertions to a record naming someone else.

**Retention follows the record it supplements.** [D] A `LateResponse`
is evidence about one presence record; a holder keeps it while it keeps that record
and discards it with them. **Saying no rule applies left it outliving the thing it
describes**, which is how an attestation becomes an orphaned fact about a person.
Nothing obliges anyone to serve it.

### 5.3c Subtree acknowledgement

**A grandpatron's countersignature over an adoption, admitting the new node to the
resources it hosts** (design §9.2.1). Carried separately rather than as a third
envelope signer, because the adoption must not wait on a party who may be offline.

```
SubtreeAck = {
  1: txid,             ; the adoption being acknowledged
  2: keyhash,          ; the acknowledging grandpatron
  3: keyhash,          ; the node admitted — redundant against field 1, and
                       ; present so a holder can index without fetching the
                       ; adoption
  4: timestamp,
  5: COSE_Sign1        ; BY THE GRANDPATRON over fields 1-4;
                       ; external_aad = "rhtn/1:subtree-ack".
                       ; Classical only: it lapses with the relationship it
                       ; describes, well inside §5.1's post-quantum horizon
}
```

**It attests membership of a subtree, not identity.** The adoption's own signatures
carry identity. A verifier that treats a `SubtreeAck` as evidence about who someone
*is* has misread it.

**Acceptance by other nodes is policy, not obligation.** A grandpatron's siblings
and the great-grandpatron may accept this in place of evaluating the node
themselves, which is the point of it existing, and nothing here requires them to.
Where accepted, the default is to allocate roles as to any subordinate in that
network position — **positional grants only**, never roles bound to named
individuals (design §9.2.1).

**Discard it when it lapses**, rather than holding a record of a relationship that
has ended. [D] The condition is visible in topology, so a holder can tell — and a
retained lapsed acknowledgement is a durable statement that two parties were once
connected, which is exactly what departure is supposed to end.

**It lapses when the acknowledged relationship ends.** If the grandpatron disavows
the patron, or the patron departs, the acknowledgement describes a subtree the node
is no longer in and confers nothing. **No revocation object is needed**: the
condition it depends on is already visible in topology.

### 5.3d Node endpoint record

**How an infra node's address reaches the parties that must refer to it.** [D] Modelled on `AnchorEntry` (§5.3), which is the anchor-table
specialisation of the same object.

```
EndpointRecord = {
  1: keyhash,          ; the node
  2: [ 1*8 NetworkPoint ],
  3: seqno,            ; the node's own counter (§2.3)
  4: COSE_Sign1        ; BY THE NODE over canonical CBOR of fields 1-3;
                       ; external_aad = "rhtn/1:endpoints". Classical only,
                       ; for §2.3's reason: relevance expires when the node moves
}
```

**Published by infra nodes only.** A light client's endpoints arrive when it attaches
(design §11.1.2) and it holds no static address; an infra node serves itself and
never attaches, so nothing otherwise carries its address to its patron — and without
it **the patron cannot refer** (design §10.6.1).

**Carried in the topology class**, by §7.2a's forwarding rule, so it reaches the
node's horizon and its patron with it. **It does not travel rootward** (§7.2b).

**Self-signed, and the signature's value is the same one §5.3 states**: a recipient
holding no key material cannot check it on receipt, so it gives **retroactive
attribution rather than prior authentication** — a node that reaches the address and
obtains the key can confirm the record was genuine, and if it was not, knows which
gossip source supplied it. That is worth having *here* and not in a referral: a
flooded object passes through parties the recipient did not choose, where a
`Referral` (§5.6.2) comes from the single party the requester is already talking to.
This is why §5.6.2's replies are unsigned and this record is not.

**Freshness by `seqno`, strictly greater to replace**, under §2.3's rule.

**A peering record already carries this for peered nodes** (§4.4, `NetworkPoint` for
both endpoints). The gap this record closes is the infra node that **neither peers
nor serves as an anchor** — a supported, degraded state (design §6.3, §10.7.5), and
until now one with no carrier for its address at all.

**Volume.** Only infra nodes publish, and design §4.3 sets the infra threshold at 110
subordinates, so an `h=2` ball of ~110 nodes contains on the order of one. The
constant-state floor of design §10.6.1 is untouched.

### 5.4 No activity-summary object

**None exists.** [D] A patron publishing transaction counts for a
subordinate, consumed by strangers as a behavioural baseline, is a reputation signal —
which design §1 refuses and §13.1 replaces with per-observer evaluation. Reasoning at
design §7.4.2.

### 5.5 No veto-delegation object

**None exists, and no mechanism it could serve.** [D] A patron shares no
state with a subordinate's client, so a delegation authorising a party to block an
action they cannot reach authorises nothing. What stands in its place — notification,
resource refusal, disavowal — needs no wire object. Reasoning at design §7.4.2.

**The invariant this number carried is retained in the design**, because it was always
the durable half: *a party with authority over another may never block an action whose
sole effect is to end that authority relationship.* It describes a power that does not
exist rather than one that must be constrained, and reintroducing any blocking
mechanism has to argue against it (design §0).

### 5.6 Resolution

Encodes design §10.3's cases and design §10.6.1's self-routing. [D]

### 5.6.0 Who sends a resolution request

**A light client sends `ResolveRequest` to its serving infra node, not to the
anchor.** [D] Control traffic is always client-to-serving-node (design §11.1.1); a
light client has no reason to hold a socket to an arbitrary anchor and often could
not reach one. The serving node resolves on the client's behalf and returns the
result.

**Infra nodes exchange resolution requests directly**, static address to static
address.

The request is anchor-*relative* — the path in it is interpreted from the anchor
named in the locator, which is a statement about how the path is read, not about
who the request is addressed to. An earlier reading conflated the two.

### 5.6.1 Descent is through infrastructure only

**A path is not walked node by node.** Intermediate nodes may be light clients,
which are neither always online nor independently reachable (§4.3, design §11.1.1).
Resolution therefore descends **only through infra nodes**, and terminates at the
**serving infra node** for the target, the nearest infrastructure ancestor, which
is the node the target attaches to (design §11.1.2).

**The remaining path suffix is returned rather than traversed.** The serving node
uses it to identify which of its attached clients is meant. This is what lets a
path address a light client that nothing can route to directly.

### 5.6.2 Messages

```
ResolveRequest = {
  1: keyhash,           ; subject being resolved
  2: keyhash,           ; ANCHOR the path is relative to. Without it the request
                        ; is uninterpretable, an earlier version omitted it
                        ; while defining the path as anchor-relative
  3: path,              ; anchor-relative, from the locator
  4: bstr .size 16      ; nonce, chosen by the requester. Cryptographically
                        ; random, fresh per logical resolution, and REUSED
                        ; across endpoint retries for that same resolution —
                        ; a fresh nonce per endpoint would let a reply for one
                        ; attempt be accepted as an answer to another
}

ResolveReply = {
  1: bstr .size 16,     ; echoes the request nonce
  2: uint,              ; 0 serving | 1 repair | 2 failure | 3 referral
  3: ? ServingInfra,    ; present iff field 2 = 0
  4: ? SignedLocator,   ; present iff field 2 = 1 (design §10.3 case 2 repair)
  5: ? uint,            ; present iff field 2 = 2; see codes below
  6: ? Referral         ; present iff field 2 = 3
}

Referral = {
  1: keyhash,                ; the next hop to query
  2: [ 1*8 NetworkPoint ],   ; its endpoints
  3: uint,                   ; path indices this referral advances past, counted
                             ;   INCREMENTALLY from the referring node's own
                             ;   position, not as an offset from the anchor.
                             ;   MUST be >= 1: a referral that advances nothing
                             ;   is a loop, and a node with nothing to add
                             ;   reports failure instead
  4: ? KeyMaterial           ; so a requester with nothing pinned can authenticate
                             ;   the next hop (§7.1). Same reason `ServingInfra`
                             ;   carries it
}

ServingInfra = {
  1: keyhash,                 ; the serving infra node's own identity
  4: ? KeyMaterial,           ; full key material, so a requester with no pinned
                              ; entry can verify the keyhash and then authenticate
                              ; the TLS peer (§7.1). Without this a first contact
                              ; cannot complete: the handshake presents only the
                              ; classical component, and the keyhash covers the
                              ; pair
  2: [ 1*8 NetworkPoint ],   ; ordering is the publisher's preference, not
                             ; binding. Selection and retry are local policy —
                             ; but an implementation MUST treat the list as
                             ; alternatives and try others on failure, or a
                             ; single unreachable first entry becomes a
                             ; permanent outage for that peer
  3: path                     ; RESIDUAL suffix identifying the client to
                              ; that node; empty when the target IS the
                              ; serving node
}
```

**Resolution is iterative with referrals** (design §10.6.1). [D] A node answers authoritatively, **refers the requester onward**, returns
a repair, or reports failure. It never carries the request itself, so no progress or
consumed-prefix state exists: each request carries its own anchor and full path, and
every reply is interpretable without knowing what came before.

**Each node interprets the path from its own position.** [D] A request carries the
anchor and the **full, unmodified path** every time; it holds no consumed-prefix
field, and none is added. A node knows its own anchor-relative position, so it knows
which portion of the path is still ahead of it. **The requester accumulates progress
locally** by summing `advances`, purely to know when it has arrived — that running
total is never transmitted and no node depends on it.

**Why not carry the consumed prefix:** a field the requester computes and every node
must trust is state an intermediary could misreport. Deriving position from a node's
own place in the tree removes the question.

**Consistency the requester checks on arrival:** the accumulated `advances` must
equal the full path length minus the returned `residual_path` length. A mismatch
means some node miscounted, and the resolution is unsound rather than merely slow.

**A referral names the next hop and gives its endpoints**, so the requester
continues from there. A node may refer past several indices at once where it knows
its own subtree.

**Nothing polices referral honesty, because nothing needs to.** The requester
authenticates each endpoint against the keyhash it expects (§7.1), so a wrong
address produces a handshake failure rather than a silent misdirection. **An earlier
version forbade forwarding outright**, reasoning that an intermediary could
misreport progress invisibly. That objection does not survive endpoint
authentication, and the prohibition contradicted design §10.6.1.

**The reply is not signed.** It conveys where to try next, and the requester
authenticates the endpoint it reaches by ordinary means at contact time (design
§7.2, the anchor table is an index, not a credential store). A wrong or hostile
reply causes a failed connection, not a false identity.

**Failure codes** for field 5, with the disposition each implies [D] — stated
because "distinguishes retry from re-resolve" without a mapping lets two resolvers
disagree about the same reply:

| Code | Meaning | Disposition |
|---|---|---|
| 0 | No such child at some index | **Re-resolve.** The locator is wrong or stale; another endpoint will say the same |
| 1 | This node is not authoritative and cannot refer | **Re-resolve.** Distinct from a referral (field 2 = 3), which *can* point onward — this code means the node knows of no next hop, so the locator is stale or names a subtree it has no relationship with |
| 2 | Temporarily unavailable | **Retry**, here or at another endpoint for the same node |
| 3 | Refused by policy | **Terminal** for this requester. Retrying elsewhere may succeed, but not by repetition |

**A locator whose anchor is absent from the local table is not resolvable by this
node.** [D] That is a caller-side condition, not a wire failure: no request is sent,
because there is nowhere to send it. The caller needs an address for that anchor
from some other source — an introduction, a cached entry, a peer's referral.

**Repair responses are bounded.** A resolver MUST NOT follow more than **4**
`forwarded` replies for one resolution. design §10.3 requires the source to return
the *terminal* forwarding record rather than the next hop, so a well-behaved chain
never exceeds one; the bound exists against a hostile or broken peer.

**A party supplying replacement routing information for a stale claim MUST NOT
change the identity being resolved, and the replacement MUST be demonstrably newer
than the claim it supersedes** — strictly greater `seqno`, never equal. **Equal
`seqno` with different contents is malformed**, not a tie to break: a subject
advances its own counter, so two distinct locators at one value means one of them
was not produced by the subject. Otherwise repair becomes a redirection mechanism,
or stale information displaces fresh. Present encoding: a forwarded reply names the
same subject, with `seqno` strictly greater than the locator being repaired
(§2.3).

### 5.7 Prekey distribution

**The network distributes prekeys; it does not define them.** design §11.2.4 adopts
PQXDH, whose bundle contents are specified by that protocol. This section carries
them.

**The bundle is opaque to this protocol** [D]. A node that serves
prekeys cannot validate their contents and does not need to — only the endpoints
share the state required to interpret them. Carrying the bundle as a blob also
means a PQXDH revision does not force a wire change here.

**The bundle is split, because its two halves have opposite properties.**

```
PrekeyBundle = {
  1: keyhash,          ; subject
  2: uint,             ; construction identifier — 1 = PQXDH
  3: bstr,             ; opaque REUSABLE material: signed prekey and PQ signed
                       ; prekey, per the named construction. Reusable by design
  4: timestamp,        ; published_at
  5: COSE_Sign1        ; BY THE SUBJECT over fields 1-4;
                       ; external_aad = "rhtn/1:prekey".
                       ; Classical only — relevance expires on replacement, so
                       ; §5.1's horizon does not apply
}

PrekeyRequest = {
  1: keyhash,          ; subject whose material is wanted
  2: uint,             ; 0 = reusable only | 1 = reusable plus a one-time key
  3: bstr .size 16     ; nonce
}

PrekeyBatchRequest = {
  1: [ 2*256 keyhash ],   ; the population swept, in ascending keyhash order
  2: bstr .size 16        ; nonce
}

PrekeyReply = {
  1: bstr .size 16,    ; echoes the request nonce
  2: ? PrekeyBundle,
  3: ? bstr,           ; a one-time prekey, iff requested and available
  4: ? uint            ; failure code when field 2 absent:
                       ;   0 unknown subject | 1 refused
}
```

**Only the one-time key is consumed on serving.** The reusable material may be
returned any number of times to anyone. A one-time key is returned once and
discarded; when none remain, field 3 is simply absent and the session proceeds on
reusable material alone, a **declared reduction in forward secrecy for the first
message**, not a failure. Everything after it is covered by the ratchet.

**The split exists so that blanket prefetch is affordable.** A client prefetching
reusable material for its whole Dunbar Org (design §11.2.4) consumes nothing
scarce, and **leaves one-time-key depletion meaningful as a signal.** Under
blanket prefetch of one-time keys, exhaustion would be the normal state and an
attacker draining a pool would be indistinguishable from ordinary traffic.

**The two request forms are structurally distinct, so a serving node need not infer
intent.** A `PrekeyBatchRequest` naming a population is visibly a sweep; a
`PrekeyRequest` naming one subject is visibly targeted. **Asking implementations to make fetches "independent of
intent" would state a property no observer could check**: the fact of a fetch is
shared, the motive behind it is not (design §1.1).
Making the shapes differ replaces an unverifiable behavioural claim with a visible
one.

**Requesting a one-time key discloses intent to message that subject.** Requesting
reusable material does not, when done as a batch.

**Speculative depletion is bounded rather than forbidden.** [D] A serving node
cannot know whether a requester is truly opening a session, and binding consumption
to session-opening evidence is circular — under PQXDH the key is needed *before*
the session exists. So the node **rate-limits one-time key issuance per requester
per subject** rather than policing motive. That bounds the harm the rule was
protecting against — draining a victim's pool to force them onto the last-resort
key — without requiring anyone to prove why they asked.

### 5.8 Archive fetch

**A presence record arrives in whatever form its holder chose** (§4.5.1). The
disclosable fields may be revealed or withheld, withholding is visible in the digest
list, and the record verifies either way. **This is the only fetch path with a use for
location**, so it is the only one where the choice carries information (§4.5.2).


**A patron given an archive head (§4.1 field 7) walks the chain backward.** Doing
that one record per round trip would be prohibitive, so fetching is batched.

```
ArchiveRequest = {
  1: keyhash,          ; subject whose archive is wanted
  2: txid,             ; head to walk back from
  3: uint,             ; max_records, 1..256
  4: ? timestamp,      ; stop at records older than this
  5: bstr .size 16     ; nonce
}

ArchiveReply = {
  1: bstr .size 16,    ; echoes the request nonce
  2: [ * Envelope ],   ; records in reverse chain order, head first
  3: bool,             ; true if more remain beyond this batch
  4: ? txid            ; continue from here, the oldest record returned
}
```

**Paginate by re-requesting with field 2 set to the previous reply's field 4.**
A holder may refuse a request or return fewer records than asked for; **a short
reply is not evidence of a short archive**, and a patron must not treat it as
truncation.

**The requester verifies the chain itself.** Each returned record's back-pointers
must match the record that follows it in the batch, and the first must match the
requested head. **A holder cannot be trusted to have walked correctly**, and the
verification is one hash comparison per record, over records the requester is
already parsing.

**Bounded by the same array limits as everything else** (§1): 256 records per
batch, matching the archive-reference bound it replaces.

## 6. Session messages

### 6.0 Control frame framing

**Stream 0 carries length-delimited, type-tagged frames.** [D] Without this two
implementations cannot parse each other at all.

```
frame = u32-be length || deterministic CBOR of [ uint frame_type, body ]
```

`length` counts the CBOR bytes that follow it, and is bounded at **64 KB**.

| `frame_type` | Body |
|---|---|
| 1 | `Attach` |
| 2 | `AttachAck` |
| 3 | `Heartbeat` |
| 4 | `SiblingUpdate` |
| 7 | `TopologyPush` (§7.2a) |
| 8 | `TopologyMemo` (§7.2b) |

**Frame types 5 and 6 are unused and not reused.** Resource traffic is
request/response and belongs on a bidirectional stream (§7.3), not on the
session-control stream.

**Topology framing belongs on stream 0 and not on a bidirectional stream.** [D] Both are unsolicited pushes with no reply, so neither is a request; and
the extension posture decides it. An unknown control frame is **skipped** and the
session survives, which is the correct outcome for gossip carrying a class the
receiver does not implement. An unknown request type on a bidirectional stream is
**rejected** (§7.2), which would turn every future topology class into a flag day.
`SiblingUpdate` is the precedent: a server-initiated frame that arrives with nobody
having asked.

**Unknown frame types MUST be skipped, not rejected.** The length prefix exists so
a receiver can skip one it does not understand; tearing down the connection instead
would make every future frame type a flag day. This is the same extension posture
as unknown capability parameters (§6.1).

### 6.1 Capabilities

**Modelled on QUIC transport parameters** (RFC 9000), which are already the
transport underneath this. [D]

```
Capabilities = { * uint => bstr }   ; parameter id => opaque value
```

**Parameter ids are derived from names, not assigned by anyone.**
[D]

```
capability_id = first 8 bytes of SHA-256("rhtn/cap:" || name),
                interpreted as a big-endian u64
```

**Byte order is normative.** Little-endian yields a different id for every named
capability, so two implementations would agree on names and disagree on every id.

`name` is namespaced by whoever mints it — `rhtn/core:max-archive-batch` for ids
this specification defines, `example.com/fast-sync` for anyone else. **The name is
the registration**, and the id is a function of it.

**No authority assigns anything and no document is consulted before shipping.** A
central registry would be the obvious alternative and is refused for the reason
central anything is refused here: it makes an experiment wait on someone else, and
it is a coordination point the design otherwise does without. Collision requires two
*different names* hashing to the same 64 bits, a birthday bound around 2^32
distinct capabilities, which this ecosystem will not approach.

**Cost, accepted: an unknown id is opaque forever.** With a registry it could be
looked up. Here a debugging session facing an unfamiliar capability learns only
that it exists, which is what an implementation does with it anyway, since unknown
parameters are ignored. An implementation wanting to be diagnosable should publish
its capability names.

**Both directions.** A client sends its capabilities in `Attach`; the serving node
sends its own in `AttachAck`. Neither is a request for the other's permission.

**Unknown parameters MUST be ignored, never rejected.** This is the same treatment
§1 gives unknown *map keys* — preserved and passed over, and deliberately
different from unknown *enum values*, which are rejected. A capability is an
extension point; an enumerated field is a closed vocabulary.

**Values, not flags.** A parameter carries an opaque value rather than mere
presence, because several natural capabilities are quantities: the largest archive
batch a peer will serve (§5.8), which prekey construction it implements (§5.7),
what it will accept as a resolution repair depth (§5.6). A bitfield could express
none of these, and would cap the space at 64.

**Absence of a capability is never a connection failure.** Parameters set limits;
they do not gate the session. Two peers operate within what both support, and a
client whose serving node is older stays attached with fewer features. Failing the
attach instead would turn ordinary version skew into a **connectivity** problem —
and the party harmed would be the light client, which has no alternative serving
node except its serving node's siblings.

#### 6.1.1 Greasing — required, not decorative

**Every implementation MUST tolerate receiving parameters whose ids it does not
recognise.** That is checkable by anyone: send one and observe whether the session
continues. An implementation that rejects it has failed observably, so this
obligation is enforced by every peer that greases.

**The reference implementation sends one greased parameter per session: a random
64-bit id and a value of 8 random bytes.** [D] Any length within the value bound
works; a fixed shape is given so the reference behaviour is reproducible and so a
greased parameter is not itself distinguishable by its size.

**Sending is not a MUST, because nothing could check it.** An unassigned id is
indistinguishable from one the receiver simply does not know, so "did you grease?"
has no answer from the wire. **The reference implementation sends at least one
randomly-chosen 64-bit id with a random value per session**, and a session either
visibly contains such a parameter or does not — presence is the evidence, not a
declaration.

**The incentive is what makes this work, not the wording.** An implementation that
never greases is relying on other implementations to keep the tolerance path
exercised on its behalf. That works while most do it and fails quietly when they
stop, at which point the extension mechanism this exists to protect is already
dead. **Greasing is self-interested rather than obligatory**, and stating it as a
duty would not change who does it.

**No reservation is needed**, which is a consequence of deriving ids by hash. A
reserved range exists in protocols with densely assigned small integers, where a
random value would probably collide with a real one. In a sparse 64-bit hash space
**a random id is an unassigned id** with overwhelming probability, so greasing is
simply sending one.

**The reason is ossification.** An extension mechanism nobody exercises decays:
implementations quietly stop tolerating unknown parameters, intermediaries begin
rejecting them, and by the time a capability is genuinely needed the mechanism no
longer works anywhere. QUIC addresses this by reserving values that must be
tolerated and by having endpoints set them unpredictably, the point being to keep
the ignore-unknown path exercised **in production**, not only in a test suite that
nobody runs against a five-year-old peer.

**For a protocol expecting to add capabilities over years, this is the difference
between an extension point and a comment claiming one exists.** It costs a few
bytes per session and is the only mechanism here whose value is entirely in the
future.

### 6.2 Frames

Not signed transactions; these are transport-layer control frames. [D]

```
Attach = {
  1: keyhash,          ; client identity
  2: ? CurrencyAttestation,
  3: Capabilities      ; §6.1
}

AttachAck = {
  1: uint,             ; mode: 0 primary, 1 degraded/failover. The SERVER's
                       ; determination, from its own topology, a client is in
                       ; failover iff it is not in this node's subtree. Sent
                       ; because a client with stale topology may not know
  2: ? [ + SiblingRef ],   ; OPTIONAL — absent means the serving node has no
                           ; siblings, which is a legal topology
  3: uint,             ; heartbeat interval, SECONDS, fixed for the session's
                       ;   lifetime — there is no update message, and changing
                       ;   it requires a fresh attach. MUST be >= 1 — zero is
                       ; malformed, since it would make every session
                       ; instantly overdue and failover permanent
  4: uint,             ; messages currently queued for this client; advisory
  5: Capabilities      ; §6.1, the serving node's own
}

SiblingRef = {
  1: keyhash,
  2: [ 1*8 NetworkPoint ],
  3: ? KeyMaterial     ; OMISSIBLE ONLY when the serving node has itself
                       ;   supplied that sibling's key material to this client
                       ;   in an earlier `AttachAck` or `SiblingUpdate` on a
                       ;   session it served. [D] A client that finds it absent
                       ;   and holds no pinned entry treats the sibling as
                       ;   UNUSABLE rather than dialling it unauthenticated —
                       ;   there is no fetch path, since the party that would
                       ;   serve one is the node that is down.
                       ; full key material, so a client can authenticate a
                       ; sibling it has never contacted. Without it first
                       ; failover cannot complete: TLS presents only the
                       ; classical component while the keyhash commits to the
                       ; pair, and trust-on-first-use cannot check a keyhash it
                       ; cannot reconstruct. Same reason `ServingInfra` carries
                       ; it (§5.6.2)
}

Heartbeat = {
  1: uint,             ; per-session heartbeat counter, from 0, +1 each beat.
                       ; On reaching u64 max, end the session rather than
                       ; wrapping, a wrap would silently reset gap detection.
                       ; NOT the node's locator seqno, a heartbeat needs gap
                       ; detection, and a locator seqno changes only on
                       ; position change
  2: timestamp
}

SiblingUpdate = {
  1: ? [ + SiblingRef ]    ; replaces the client's cached list ENTIRELY.
                           ; Absent means the node now has no siblings
}

TopologyPush = {
  1: uint,             ; body kind: 0 = signed transaction envelope,
                       ;            1 = EndpointRecord (§5.3d)
  2: bstr              ; the object, byte-for-byte as received. NOT re-encoded:
                       ;   it is already canonical (§1) and re-serialising risks
                       ;   changing bytes a signature covers
}

TopologyMemo = {
  1: keyhash,          ; SUBJECT — the node added or removed
  2: Locator,          ; the PATRON's position: anchor, path, patron's seqno
  3: bool,             ; true = added, false = removed
  4: seqno             ; the SUBJECT's own counter at this position change,
                       ;   NOT the patron's. Field 2's seqno orders the patron's
                       ;   moves; this one orders the subject's, which is what a
                       ;   memo table is keyed on (§2.3)
}
```

**A sibling list naming the receiving client is malformed**, as is one containing
duplicate keyhashes. [D] A client cannot fail over to itself, and a duplicate entry
is either an error or an attempt to weight one endpoint in a list the client is
expected to try in order.

**Every successful `AttachAck` replaces the cached sibling list**, including one
from a failover sibling — its list is the correct one for the node the client is
now attached to. Retaining the primary's list through a degraded session would
send the next failover to peers of a node the client is not talking to.

```
```

[D] `AttachAck` carries the sibling list **when there is one**: a
client cannot discover failover targets after its serving node is already dark.

**Field 2 is optional, and this was a correction.** [D] An earlier
version required `[ + SiblingRef ]` while §1 forbids encoding an empty array —
so an infra node with a single child, a perfectly legal topology, **had no valid
`AttachAck` encoding at all**. Absence now means no siblings.

**The serving node determines the mode; the client is told.** A client
attaching to a sibling sits two hops away in topology the sibling holds within
its horizon, so *"is this client in my subtree?"* answers the question without
anything on the wire. `AttachAck` field 1 exists because **the client may not
know**: one whose serving node changed — its patron grew into infra, say — can
believe it is reaching its primary when it is not. The determination belongs with
the node that has authority over its own subtree.

**`Attach` carries no client-asserted serving node.** design §11.1.2's *"must be
explicit"* is a **user-interface** obligation, below, not a wire one.

**Heartbeat units are seconds** and the counter is per-session. Both were bare
`uint` with no stated meaning; a peer reading milliseconds where the other wrote
seconds fails liveness in exactly the way that looks like a network fault.

**Liveness rule** [D]: a peer is considered failed after **3 consecutive missed
intervals**, where:

- **Both sides send one `Heartbeat` per advertised interval**, so liveness is
  bidirectional and the interval is the serving node's to set.
- **Only an explicit liveness signal — one whose purpose is to demonstrate the
  peer's control path is responsive — resets the failure detector.** Unrelated
  payload or application traffic must not, since it would let incidental activity
  conceal failure of the very path the detector tests. Present encoding: only a
  `Heartbeat` resets the counter.
- **One full interval elapsing without a valid `Heartbeat` is one miss.** The first
  deadline runs from `AttachAck`, and **counter 0 is sent after one interval, not
  immediately** — an immediate beat would make the first interval a half-interval on
  one side and invite disagreement about whether a miss had occurred.
- **A malformed heartbeat counts as absence, not as a fault.** It does not reset
  the detector and does not itself close the session; three missed intervals still
  govern. Closing immediately would make a single corrupted frame indistinguishable
  from a dead peer, and give a path-level attacker a cheap disconnect.
- **The timestamp in a heartbeat is advisory.** Liveness is decided by local
  receipt time; a clock-skew test would let two peers disagree about liveness while
  exchanging identical frames. Below that a client does not fail over; at or above it, it dials a
sibling of its serving node.

**Failover applies to a primary that is already dark, not only to one that dies
mid-session.** [D] The three-missed-intervals rule presupposes an established
session; a client whose serving node is unreachable at attach time uses its cached
sibling list immediately. Otherwise the list — pushed precisely because it cannot
be discovered once the node is dark — would be unusable in the case it exists for.

**There is no `AttachNack`.** A dial that closes, resets, or produces no valid
`AttachAck` within a local timeout is a failed attach. The client tries the
remaining candidates in the order received; when all fail it is disconnected and a
later fresh attach begins with the primary again. Timeout and backoff are local
policy.

**The cached sibling list should survive restart.** It is replaced by any
`AttachAck` or `SiblingUpdate` and otherwise persists; an in-memory-only client
loses failover exactly when a crash coincides with its serving node being down.

**No automatic failback.** A client on a sibling stays there until that session
ends; the next fresh attach tries its actual serving node first. Probing the
primary from a degraded session adds traffic for a state the client will leave
anyway at the next natural reattachment.

[D] Queuing is continuous — there is no "begin queuing" signal, and
reachability is only a hint about when to attempt delivery.

---

## 7. QUIC binding

[D]

### 7.1 Handshake and hop authentication

**QUIC with TLS 1.3, key exchange group `X25519MLKEM768`.** This is the deployed
industry-standard hybrid rather than anything invented here: classical X25519
composed with ML-KEM-768, so the connection survives either primitive failing.
**ML-KEM-768 is the profile parameter set**, matching ML-DSA-65's security level;
naming the family without choosing a set is not implementable.

**Peer authentication uses raw public keys (RFC 7250), not X.509.** Identities here
are keyhashes, not certificate subjects, and there is no certificate authority to
issue or validate chains.

**A peer presents its classical component**, since RFC 7250 carries one
SubjectPublicKeyInfo and an identity here is a *pair* (§2.2). The dialling party
checks that key is the classical member of the `KeyMaterial` it has pinned for the
keyhash it intended to reach. **The post-quantum component authenticates nothing at
the transport layer, and does not need to.** design §5.1's rule is that an object
may use the classical component alone where its relevance expires before the
post-quantum horizon, and a session's authenticity expires with the session.
Confidentiality is separately post-quantum via the key exchange group.

**Authentication is mutual.** [D] The serving node authenticates the client the
same way, and **MUST bind the identity for which session state and queued data are
requested to the identity the transport authenticated, rejecting any mismatch.**
Present encoding: `Attach` field 1 must equal the connection-authenticated
identity. `Attach` is unsigned, so without that check any party
could claim any keyhash and receive another node's queued messages.

That is the whole authentication step, and it is exactly what §5.6's unsigned resolution
replies rely on, a wrong address produces a failed handshake rather than a false
identity.

**Downgrade protection** is TLS 1.3's own. **That nothing can be negotiated below
the named group is this profile's doing, not TLS's** — TLS 1.3 negotiates among
whatever groups are configured, and offering only `X25519MLKEM768` is what leaves
nothing weaker to fall back to.

### 7.2 Binding

- ALPN: `rhtn/1` [D]
- **Default port 7431/udp**, overridable per endpoint. `NetworkPoint` carried no
  port at all, so a client could not turn a sibling reference into a socket
  address
- Stream 0: session control (attach, heartbeat, sibling updates)
**Every bidirectional stream opens with a request-type tag.** [D]
Resolution, archive fetch, prekey fetch, resource requests, catalog queries and
verifier queries all share the same ALPN and the same stream class, and **nothing
told a receiver which it had.** Structural guessing across six schemas is not a
protocol.

```
frame = u32-be length || deterministic CBOR of [ uint request_type, body ]
```

Same framing as stream 0 (§6.0), same bound, same rule: **unknown request types are
rejected on a bidirectional stream**, unlike unknown control frames which are
skipped. A control frame arrives on a shared stream where skipping preserves the
session; a bidirectional stream *is* the request, so a type nobody understands has
no continuation to preserve.

| `request_type` | Body |
|---|---|
| 1 | `ResolveRequest` (§5.6) |
| 2 | `ArchiveRequest` (§5.8) |
| 3 | `PrekeyRequest` / `PrekeyBatchRequest` (§5.7) |
| 4 | `VerificationQuery` (§4.6) |
| 5 | `CatalogQuery` (§4.7) |
| 6 | `ResourceRequest` (§7.3) |

**The reply carries no type tag and is framed identically otherwise** — the same
`u32-be` length prefix and CBOR body. It answers a request whose type the requester
chose, on a stream it opened, so a tag would restate what the requester already
knows.

- Bidirectional streams: request/response — resolution, attestation pull, resource
  requests (§7.3),
  verifier queries
- Unidirectional streams: payload delivery, queue drain
- Connection migration relied upon for mobile IP change; 0-RTT resumption for
  reattachment, which permits a lazy heartbeat and saves battery

**NAT traversal for the payload path only** [D]. Control traffic
needs none: clients dial outward to their serving infra node at a static IP, and
infra nodes dial each other directly.

**Payload attempts a direct path inside the horizon**, which does require
traversal. Infra nodes act as **STUN and TURN**, and ICE falls back to relaying
through the serving node when hole punching fails. Address- and port-dependent
mapping defeats it, and carrier-grade NAT and mobile networks raise the odds of
meeting that, but **neither guarantees failure** (RFC 8445).

---

### 7.2a Topology propagation

**design §12 defines the classes and the patterns; this section says what carries
them.** [D] *Flood-within-horizon* and *push near, redirect far* are
named and reasoned about across rotation (design §7.4.0.2), peering (design §6.3)
and the endpoint records of §5.3d. Until now none of them said what was sent, on
what stream, or how a receiver decided whether to forward. **A policy is not a
protocol.**

**Frame:** `TopologyPush`, control frame type 7 on stream 0 (§6.0), carrying the
object byte-for-byte with a **body-kind tag and nothing else.** The class is not
uniform — most topology objects are signed transactions with an envelope, and an
`EndpointRecord` (§5.3d) is a standalone signed map with neither — so a receiver needs
one discriminator to know which parser to use. **That is the only wrapper field
permitted.** Anything further would be sender-supplied state every receiver must
trust, which §5.6.2 rejected for resolution and rejects here for the same reason.

**Who may push.** A node pushes topology it is a party to — its own adoptions and
departures, disavowals of its own subordinates, its own peerings, its own endpoint
record. **Within the horizon this is a trusted push**, in the narrow sense that the
pusher is the party whose position the object describes and the object is signed by
that party. It is not an assertion about anyone else's topology.

#### The forwarding rule: forward if and only if you stored it

**A node stores a topology-class transaction when its subject falls within that
node's own `h_store`; a node forwards a stored transaction to every adjacent node
except the one it arrived from.** [D] There is no hop count, no TTL and no reach
field.

**Reach is a consequence of storage policy, not a separate mechanism.** A hop
counter would encode the *sender's* horizon and impose it on every receiver, and
design §12.1 is explicit that a horizon "is a scope, not a shared region… no two
nodes with different positions have the same one." A counter is also a value an
intermediary can fail to decrement, so a rule resting on it asks each node to trust
arithmetic it cannot check. Deriving the decision from the receiver's own store
removes both problems, and introduces no concept the node did not already have.

**Each receiver evaluates on receipt against its current view.** [D] Local topology
changes while a message is in flight, and a receiver that evaluates against what it
holds *now* is correct by construction where one applying a sender's precomputed
reach is not.

**It also preserves the constant-state floor.** design §10.6.1 bounds what a node
*must* keep at parent plus ≤f children. A storage-derived rule inherits that bound;
a TTL-derived one would let a sender push traffic into nodes that had decided not to
hold it.

#### Duplicate and loop suppression: by `txid`, against the store

**A node that already holds an object does not store it again and does not forward
it.** [D] The store the node keeps anyway is the seen-set. A horizon contains cycles
once peering exists (design §6.3); the second arrival is a duplicate and dies there.

**Identity differs by body kind, and both are already defined:**

| Kind | Identity | Duplicate when |
|---|---|---|
| Signed transaction | `txid` (§1) | the `txid` is already held |
| `EndpointRecord` | `(subject keyhash, seqno)` | the held `seqno` is greater than or equal (§2.3) |

**An `EndpointRecord` therefore supersedes rather than accumulating**, which is what a
current-address record should do, and §2.3's strictly-greater rule already governs it.
**Equal `seqno` with different endpoints is malformed**, per §5.6.2.

**No dedicated suppression cache exists**, and none should be added. It would be a
second copy of a fact the store already holds, with its own expiry parameter to leave
unset.

#### No acknowledgement, no retry

**Deliberately.** [D] Neither is defined, and their absence is a decision rather
than an omission.

**Loss is detected at the receiver, which is the party that can act.** §3.1 puts a
back-pointer in every signed body, so a transaction whose predecessor the receiver
does not hold announces the gap, names exactly what is missing, and §5.8 fetches it.
That is stronger than acknowledgement, which detects loss at the sender — who can
only resend into the same hole.

**Second repair path: periodic reconciliation.** [D] A node that missed something it
ought to hold reconciles with its siblings and its patron, and reconciliation is a
**replay of the same frames** rather than a distinct mechanism. There is no repair
protocol to specify beyond what propagation already defines.

**And acknowledgement aimed at a party you share no state with is design §1.1's
unenforceable direction.** You cannot make a peer forward. Acks and retries would
also make gossip traffic scale with population, inverting design §12's stated
scaling property.

### 7.2b Rootward topology memo

**A minified record of every membership change travels rootward to its subnet's
root.** [D] `TopologyMemo`, control frame type 8 on stream 0 (§6.0).
This is what design §12's *ancestors* reach means: **full transactions flood within
the horizon; only memos travel further up.**

**Restricted to membership operations — adoption, departure and disavowal — and the
restriction is load-bearing.** [D] Peering is topology class and is **excluded from
rootward travel**: a peering record carries `NetworkPoint` for both endpoints plus
ASN and routable prefix (§4.4), and design §14.5.8 C8 maps that composition to a
natural person. A memo carries keys and positions and no address, which is what
makes a root's accumulated view tolerable; adding peering for symmetry would
silently remove that property.

**Forwarding.** A receiving node forwards the memo to its own patron, unchanged
except that nothing is added — the memo already carries the position it describes.
A root has no patron and forwarding stops there. **A memo never leaves its subnet**,
because its anchor names the subnet and rootward travel terminates at that subnet's
root. This is what keeps the mechanism clear of design §4.1.1: nothing compares a
node's binding in one subnet against its binding in another, and nothing adjudicates
between them.

#### Two checks, with different requirements

| Check | Needs | Reach |
|---|---|---|
| **Am I on this path?** — cycle detection | the memo alone | any depth |
| **Do I already hold this subject elsewhere?** — re-parenting | a memo table | wherever a table exists |

**The cycle check is exact and needs no stored state.** [D] A memo reached you by
travelling up patron edges. If field 2's path contains your own position, you are
your own ancestor, which is a cycle. It fires at any depth, on the memo alone, and
terminates the memo there.

**The re-parenting check needs a table**, described below.

#### The memo table

**A node MAY maintain a table of `subject → (location, seqno)` built from the memos
that pass through it.** [D] Every memo from below traverses it, so the table's
coverage is that node's **whole subtree** rather than its horizon.

**It is a RIB.** design §12's liveness class already sets the retention rule: *keep
the table, not the update history, as BGP keeps the RIB.* Apply the operation,
retain the current mapping, discard the memo. No separate retention parameter is
needed and none is defined.

**Ordering is by the subject's own `seqno`** (field 4), under §2.3's existing rule:
strictly greater to replace, and absence of prior state is not a failure. Memos
arriving out of order therefore resolve without a clock. **Equal `seqno` naming a
different location is malformed**, per §5.6.2 — a subject advances its own counter,
so two distinct positions at one value means one was not produced by the subject.

**The table is optional, and detection degrades gracefully rather than failing.**
[D] design §10.6.1 fixes the required state at parent plus ≤f children and calls
anything beyond it "an optimisation above the floor." A tier that keeps no table
loses latency, not detection: the memo continues upward and a tier that does keep one
catches the conflict, at worst the root. **No tier is load-bearing.**

> **Pressure worth naming.** At the root of a large subnet the table *is* a map of
> the subnet, which is exactly the state design §10.6.1 says a node must never be
> *required* to hold. Permitted-but-incentivised is how such floors erode. An
> implementer reading §10.6.1 alone will not see this coming, which is why it is
> stated here.

#### A memo is a hint, never evidence

**No node acts on a memo alone.** [D] A memo is a derived summary and is not signed;
an intermediary can fabricate one. Acting directly would manufacture the **false
positive** design §6.2.5 ranks as the worse failure — a refused or severed legitimate
adoption, indistinguishable from censorship.

**So a detection triggers a fetch**, of the underlying signed transaction via §5.8,
and any action follows from the transaction. This is what lets the memo stay small
and unsigned.

#### What a detecting node does

**It disavows the direct subordinate that forwarded the memo to it**, once the
transaction is fetched and confirmed. [D] Any edge breaks a cycle, and that is the
one the detector has authority over (design §6.2.2). It requires no agreement with
the other party, no tie-break rule and no clock.

**Reason code 5, without prejudice** (§4.3). Nothing adverse is alleged: a cycle is
a structural accident, and design §6.2.5 already calls the bootstrap case "a likely
accident rather than an attack."

**Where both parties are present, the prompt is better and comes first.** design
§6.2.5's disambiguation — *one of you must be the patron* — resolves the bootstrap
case socially. Automatic disavowal is the fallback for cycles formed at a distance,
which is the case the memo exists for.

**The downward memo.** A node whose table shows the subject already held elsewhere in
its subtree sends a memo down the other branch, toward the stale position. Same frame;
direction is implied by the receiver's position relative to the sender rather than by
a field.

**It walks the tree rather than being addressed to the old patron**, and that is the
point: the intervening nodes are updated on the way past. Descent is by anchor and
path (§5.6.1), the same mechanism as resolution.

**Nothing compels the stale-edge holder to act.** [D] It holds an edge its own records
now show superseded, and design §1.1 is why nothing further is said. **A node
registered at two positions is not known to harm the network**: addressing is by
anchor and path, never by lookup against a higher tier's table, so a stale entry
misroutes nobody. What the memo achieves even where a patron declines to disavow is
that **the rest of the subnet's view reflects the most recent adoption**, to the
extent its members run compliant clients. That is a convergence property of compliant
behaviour, not a rule anyone enforces.

### 7.3 Resource requests

**A resource request rides a bidirectional stream on a session the requester holds
with the *hosting* node.** [D]

```
ResourceRequest = {
  1: keyhash,          ; the resource being addressed. A hosting node runs several,
                       ;   and nothing else in the frame names which
  2: bstr             ; the application request as an HTTP/1.1 message: request
                      ;   line, headers, blank line, body (RFC 9112). The node
                      ;   PARSES and RE-SERIALISES it (§7.3.2) — it does not
                      ;   relay the bytes. Opaque means the node does not
                      ;   interpret the application semantics, not that it
                      ;   forwards unexamined input
}

ResourceResponse = {
  1: uint,             ; 0 delivered | 1 no such resource | 2 refused
                       ;   | 3 resource unavailable | 4 malformed request
                       ;   | 5 no subtree acknowledgement | 6 no matching role
                       ; EVALUATION ORDER IS NORMATIVE, see below
  2: ? bstr            ; the application response as an HTTP/1.1 message, present
                       ;   iff field 1 = 0. Same reasoning as the request
}
```

**Evaluation order is normative**, because it decides which of two true things a
requester is told. [D]

1. **Resource exists on this node** → **code 2**, not code 1, if it does not. An
   unknown resource keyhash has **no owner**, and membership is owner-relative
   (design §9.2), so there is no membership question to ask first. **Returning code
   1 here would answer a non-member**, which is exactly what the split was for.
2. **Membership** in that resource owner's Dunbar Org → code 2. Nothing further is
   evaluated or disclosed.
3. **Subtree acknowledgement** → code 5 if absent.
4. **Role predicates** → code 6 if none match.
5. **Availability** → code 3 if the package is not running.

**Code 1 is therefore only reachable at step 3 onward**, once the asker is known to
be a member of that resource's owner's org. A member asking for a keyhash the host
does not have still gets code 2 — the host cannot tell whether that resource exists
elsewhere, and answering *no such resource* would assert something it does not know.

**Availability is checked last, so a role-holder learns the service is down and a
non-role-holder never does.** Reversing 4 and 5 would tell anyone in the horizon
when a resource is offline, which is operational information about the owner. And
running availability first would answer *unavailable* to someone who has no role,
which is true and useless — they would retry forever against a resource they could
never reach.

**Each code answers exactly one question**, so a member is never left choosing
between two explanations: *no role* and *service down* are different answers and a
member entitled to one should not receive the other.

**The non-zero codes are the node's answers, not the resource's.** A resource
that returns an application-level error returns it inside field 2 with code 0 —
**the network delivered it**. Conflating the two would let an application error
look like a gateway refusal.

**A requester inside the owner's Dunbar Org gets a specific reason; one outside
gets `refused` and nothing more.** [D] The membership gate (design §9.2) runs first,
so the node already knows which it is talking to.

**Inside the horizon, withholding the reason helps nobody.** A member who lacks a
`SubtreeAck` (code 5) or satisfies no role predicate (code 6) can act on that — ask
the grandpatron to acknowledge them, ask the owner for a role — and a bare refusal
leaves the resource undiagnosable to precisely the people entitled to use it. They
already hold the topology those answers describe.

**Outside it, `refused` (code 2) covers everything**: not a member, membership
lapsed, resource does not exist for you. A stranger learns nothing about the owner's
membership or policy, which is the case the opacity was for.

**Code 1, `no such resource`, is therefore only ever sent to a member.** To a
non-member every path returns code 2, including a keyhash naming nothing — otherwise
a stranger enumerates the host's resources by watching which lookups differ.

#### 7.3.1 Reaching a node you are not attached to

**This is ordinary session establishment, not a gap.** [D] A resource
request goes to the hosting node and a catalog query to each horizon node in turn;
neither is the asker's serving node, and **neither needs a mechanism the design
lacks.**

**Messaging already works this way.** design §10.6.3's relayed path runs client →
own serving node → **recipient's serving node** → recipient, and its direct path has
a client reach a peer outside its own subtree entirely. Infra nodes hold static
addresses (§5.3) and authenticate by keyhash (§7.1), so opening a session to one is
the same operation wherever it sits.

**Attachment is not exclusivity.** design §11.1.2 makes attachment the answer to
*where do my messages queue* — one node, because a mailbox must have one address. It
says nothing about which nodes a client may open a session with, and reading it as a
restriction was a misreading of what the singular is for.

---

#### 7.3.2 The node parses; it does not relay

**A node that strips `rhtn-*` headers is parsing HTTP, and must be specified as
doing so.** [D] Saying it relays bytes opaquely *and* replaces
caller-controlled headers describes two different components, and the gap between
them is the classic multi-parser hazard: **two parties disagreeing about where one
message ends and the next begins.** If the node's view of a message boundary differs
from the resource's, a caller can place content that one treats as a body and the
other as a fresh request — with the node's authenticated headers attached to it.

**Requirements on the node**, each closing part of that gap:

- **Parse the message fully and re-serialise it canonically.** Forward what your
  parser produced, never the bytes you received. A byte a resource sees that your
  parser did not examine is a byte outside your security model.
- **Reject rather than normalise anything ambiguous**: conflicting or duplicated
  length and transfer-encoding headers, malformed chunking, header names differing
  only by case or whitespace, obsolete line folding, a request line your parser
  cannot fully determine.
- **Emit exactly one message per `ResourceRequest`.** If the parse yields more than
  one, that is the attack; reject.
- **Insert your headers after stripping, into the re-serialised message.**

**This is a category with published prior art rather than a novel hazard**, and it
is why the requirement is *parse and re-serialise* rather than a list of patterns
to filter. A filter enumerates what its author thought of.

---

## 8. Size estimates

**All archive-retained transactions are post-quantum** (design §5.1), so the
classical column applies only to session-layer traffic.

| Object | Classical | Post-quantum |
|---|---|---|
| Adoption | — | **~8 KB** |
| Departure | — | **~4 KB** |
| Disavowal | — | **~4 KB** |
| Peering | — | **~8 KB** |
| Presence record (typical, ~10 signers) | ~2 KB | **~35 KB** at ML-DSA-65 [D]. Includes ~144 B of disclosure salts, **0.4%** (§4.5.1) |
| Presence record (maximum signers) | ~5 KB | **~65 KB** — 18 logical signers × (64 + 3,309), plus 32 verifier responses at 2 classical signatures each |
| Currency attestation | ~150 B | ~2.6 KB |
| Anchor entry | ~60 B | ~60 B (hashes only) |

**Selective disclosure does not reduce a presentation.** A minimised record replaces
each withheld field with a 32-byte digest, at most ~288 B, against ~34 KB of signatures
that cannot be omitted — the envelope requires exactly the required signer set. It buys
disclosure control, not bandwidth (§4.5.1).

Presence records are the deliberate PQ exception: rare, and they must remain
verifiable for decades. A few hundred per user per decade is under 10 MB lifetime.

---

## 9. Open items

1. **Queue cap value.** A per-node policy value; design §16.1.1 classifies it
   *freely tunable, forever*, and nothing here fixes one.
2. **Canonical test vectors.** None exist. Positive and negative vectors are
   required before two implementations can be shown to interoperate, but are best
   written alongside a first implementation rather than ahead of it, since
   vectors written from the spec alone encode the spec's own mistakes.
