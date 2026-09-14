# Wire Format Specification

**Status:** Draft.

**Companion to `network-design.md`.** That document holds rationale, this one
holds encoding. Where they disagree the design is authoritative and this file is
stale.

---

**Notation, and where each name is defined.** Global rules in §1 refer to
structures defined later; this table is the bridge, not a schema.

| Name | Defined | Name | Defined |
|---|---|---|---|
| `keyhash`, `KeyMaterial` | §2.2 | `txid` | §1.4 |
| `path` | §2.1 | `Locator`, `seqno` | §2.3 |
| envelope, `Sig_structure` | §3 | timestamps | §3.3 |
| control frames (`Attach`, `Heartbeat`, …) | §8 | request types | §9.2 |

## 1. Encoding

**CBOR (RFC 8949) with deterministic encoding, RFC 8949 §4.2.**

Rationale: every archive-retained object in this protocol is signed, and **protobuf serialization
is explicitly not canonical** — Google's own documentation says so, citing
unspecified field ordering and unknown-field handling, so the same logical
message can serialize differently and signatures fail to verify. CBOR's deterministic profile fixes
map key ordering, integer encoding, and float representation; the residual
float semantics RFC 8949 leaves to protocols are moot here — no schema in this
document admits a float.

**Signatures use real COSE (RFC 9052).** — *not* a custom map that merely
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
  separation for free**, and this protocol has **thirteen** signing roles, and will acquire more. See
  §1.1-§1.4 for the required profile rules.
- **A place to put the algorithm identifier.** RFC 9052 requires `alg` to be
  authenticated but permits it either in the protected header **or as externally
  supplied data**, so "self-describing" is a profile choice rather than a COSE
  guarantee. **This profile requires `alg` in the protected header** of every
  signature. The earlier custom `VerifierResponse` signature carried no algorithm
  at all and was not independently implementable.
- **A defined signing input**, which the custom scheme lacked for two of its
  three contexts.

**Nested COSE objects are UNTAGGED.** RFC 9052 permits either, and libraries
expose both serialisations, so a profile that does not choose gets two byte
encodings for one object — which breaks canonicality before it breaks
interoperability.

**All signatures in this profile are DETACHED.** The COSE payload slot is
`nil`, and a verifier reconstructs the payload from the object it is checking.
Carrying it embedded would give one logical object two byte encodings, the object
itself plus a copy inside its own signature, and the second copy could disagree
with the first.

**A signature covers every retained field of the object it signs, except the
signature field itself**, including unknown extension keys. **For a transaction that
object is the body, never the envelope** (§3): version and type sit outside
coverage, so adding a signer does not invalidate existing signatures. The per-object descriptions below name the
fields defined *today*; they are not a closed list. §1's rule that unknown map keys
survive re-serialisation exists precisely so an extension is covered rather than
silently dropped, and reading "signs fields 1–2" as exhaustive would defeat it.

**Wherever a signature is described as covering "fields X–Y", the signed payload
is the deterministic CBOR of the *map* containing exactly those fields under
their original keys — plus any unknown extension keys the object carries — and
never the signature field itself.** Stated once because the phrase names eight
objects, and it admits an array and a concatenation reading that produce
different bytes and interoperate with nothing. The map keeps each field under
its own key, which also makes a failing signature diagnosable — a concatenation
offers a debugger nothing to label.

### 1.1 Domain separation — REQUIRED PROFILE RULE

COSE does not separate application roles (above), so this profile does.

**Every distinct signing context MUST carry its own separation tag, and a verifier
MUST reconstruct that tag from the context in which it is checking — never from the
message.** A context is distinct whenever the same key could be asked to sign in
more than one role; adding a role therefore means adding a tag, and the table below
is the current enumeration rather than a closed set.

Present encoding: `external_aad` holds the ASCII role tag. *A `VerificationQuery` is hashed, never signed — `query_id` is
SHA-256 of its canonical form (§4.5), and what gets signed is the resulting id,
under `rhtn/1:consent`. The `rhtn/1:nonce-commit`, `rhtn/1:verifier-seed` and
`rhtn/1:wnonce` hash tags were retired 2026-09-01 with deterministic selection
and are not reused.*

**Four hashes carry no tag, and their safety is structural rather than tagged**:
`txid` (a body map, whose first key is always 0), `keyhash` (the two-element
`KeyMaterial` array), `query_id` (a six-entry map whose first key is 1), and
the genesis value (exactly 32 raw bytes, shorter than any other preimage here).
Their preimage languages are pairwise disjoint from the first bytes, so no
digest can be reinterpreted across roles without a SHA-256 collision. **The
disjointness is an invariant, not an accident: any future hashed object MUST
either be structurally disjoint from every language above, or carry its own
`rhtn/1:` tag** [author, 2026-09-01].

| Role | `external_aad` |
|---|---|
| Transaction envelope | `rhtn/1:envelope` |
| Verifier response (§4.5) | `rhtn/1:verifier` |
| Subject consent to a query (§4.5) | `rhtn/1:consent` |
| Currency attestation (§7.1) | `rhtn/1:currency` |
| Catalog entry (§4) | `rhtn/1:catalog` |
| Abuse report (§4) | `rhtn/1:abuse` |
| Standalone locator (§2.3) | `rhtn/1:locator` |
| Anchor entry (§7.2) | `rhtn/1:anchor` |
| Node endpoint record (§7.6) | `rhtn/1:endpoints` |
| Prekey bundle (§7.8) | `rhtn/1:prekey` |
| Subtree acknowledgement (§7.5) | `rhtn/1:subtree-ack` |
| Old-key successor statement (§4.1) | `rhtn/1:successor` |
| Former-patron transfer statement (§4.1) | `rhtn/1:transfer` |

**Why it matters more than it did.** Exploiting cross-context confusion requires a
byte string valid in two roles, which the differing CBOR structures argue against without ruling out
— and unproven non-confusability is precisely what domain separation exists to
replace. **Thirteen roles carry a tag**, and the risk domain separation
answers grows with every one of them: each new signed context is another
chance for a byte string to be valid in two places at once. A verifier that derives the tag from
context rather than content also makes the check free.

### 1.2 Deterministic encoding — REQUIRED PROFILE RULE

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
  verification, and MUST NOT be silently dropped. **Their values are opaque
  encoded slices, preserved and never interpreted** [author, 2026-09-01]:
  uninterpretable state kept for a reader that may understand it later. Any
  deterministically encoded CBOR item is admissible — the profile's encoding
  rules govern the slice's framing, and its meaning is nobody else's to
  evaluate, which is why preservation works by slice and never by
  reconstruction through a typed model.
- **Validate the bytes as received; do not decode and re-encode to compare.**
  A decode-then-re-encode check erases the evidence it is meant to find — duplicate
  keys collapse, and an unknown key's original encoding is lost. Keep the received
  bytes and derive typed views from them.
- **Duplicate map keys MUST be rejected before the map is materialised.** A
  decoder that parses into a map type first has already collapsed them, and
  deterministic encoding is then satisfied by a body the sender never sent.
- **Unsigned messages reject unknown map keys — every message outside a
  signature's coverage, in every family.** The preserve-unknown-keys rule below
  exists so an extension survives re-serialisation for signature verification;
  an unsigned message is never re-serialised for one, so there is nothing for
  preservation to protect — and accepting unknown keys on an unsigned message is
  accepting unbounded input from an unauthenticated peer.
- **An OPTIONAL field whose value equals its stated default MUST be omitted,
  never written out** [author, 2026-09-01] — the scalar analogue of the rule
  below: a written-out default gives one logical object two encodings, and
  every distinctness rule would then have to answer whether `{ip}` and
  `{ip, port: default}` are one destination or two. Omission is the one
  spelling.
- **An OPTIONAL field whose value would be an empty array or map MUST be omitted,
  never encoded empty.** Absent and present-but-empty produce different bytes
  and therefore different signatures and txids; without this the same logical object
  has two valid encodings.

  **This does not apply to required fields.** A required map with no entries is
  encoded as an empty map — `Capabilities` in `Attach` and `AttachAck` is the case
  in point, since a peer advertising nothing must still send the field. Nor does it
  apply to a top-level object: a `SiblingUpdate` clearing the list encodes as an
  empty map, and reading the rule literally would leave that state with no encoding
  at all.
- **Unknown *values* in a known enumerated field MUST be rejected** — distinct
  from unknown map *keys*, which are preserved. A decoder that cannot interpret a
  result code cannot evaluate the object, and silently ignoring it would mean
  treating an unevaluated field as absent.
### 1.3 Global structural bounds

**Every array is bounded.** Arrays arriving from strangers are a resource
attack surface, so each is given an explicit maximum below; exceeding it is
malformed, not merely unusual.

| Array | Maximum |
|---|---|
| Envelope **logical signers** | **Derived, not a constant.** The sum of the transaction type's per-role bounds. For a presence record: 2 participants + 16 witnesses = **18**. **Verifiers are not envelope signers.** Their signatures are embedded evidence inside the body (§3.5, design §5.1) |
| Envelope `COSE_Signature` **entries** | **Twice the logical-signer bound**, since each contributes one classical and one PQ entry (§3.5). For a presence record, **36** |
| Archive subset references | 256 |
| Verifier responses per recovery | 32 |
| Witnesses per presence record | 16 |
| Path length | 24 nibbles (depth 24 at f=10 exceeds any plausible network) |
| Prekey bundle blob | 4 KB. A PQXDH bundle is an ML-KEM-768 encapsulation key (1,184 B) plus signed prekeys and their signatures — roughly 1.5–2 KB, so this is a DoS ceiling with headroom rather than a capacity figure |
| Merge back-pointers per signer | 8 |
| Verifier responses per presence record | 32 — **two subjects × a per-subject threshold capped at 10**, with headroom. Sixteen does not fit: two well-connected participants require ten each |
| Asserted locations per record | 4 |
| Corroborations per record | 16 (one per witness) |
| Proximity channels per record | 8 |
| Explicit-scope keyhash list | 256 |
| NetworkPoint entries per anchor entry or endpoint record (§7.2, §7.6) | 8 — this row is the anchor entry's and the endpoint record's alone; **peering carries exactly one `NetworkPoint` per endpoint** (§4.4) |
| `CatalogEntry`, total encoded bytes | 2048 |
| `CatalogReply` entries | 111 — an answering node answers for **itself plus the ≤110 users it serves** (§6.4, design §11.5). Not the trust horizon population, which is larger (design §15.1) and irrelevant here: the bound is per *answering node*, not per horizon. The frame bound caps this at 127 |
| Unknown extension keys per map | 16 |
| Unknown extension value | **1024 bytes of encoded CBOR** — the complete encoded slice for the value, which is measurable for every value type and is what bounds parser work. Not the aggregate of contained byte/text content |
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
logical signer contributes one classical and one post-quantum entry (§3.5).

**An independently asserted envelope bound can contradict the per-role bounds**, so
that a record one bound permits the other rejects. Deriving it makes that
structurally impossible rather than something to notice.

**Signature arithmetic, so the figures above are checkable.** Ed25519 is 64 bytes,
ML-DSA-65 is 3,309. **One logical signer costs 3,373 bytes**, not 6,618 — only one
of its two entries is post-quantum (§3.5). An adoption's two signers are therefore
≈ 6.6 KB, and a ten-signer presence record ≈ 33 KB, both before the body.

**Typical is far below maximum.** A presence record is expected to carry ~10
logical signers — two participants and ~8 witnesses, at 10 × (64 + 3,309) ≈ **33
KB**, or ~35 KB with the body. The bound accommodates 18 envelope signers plus 32
embedded verifier responses, ≈ **65 KB**. Bounds exist to
stop a stranger exhausting memory, not to describe normal operation.

### 1.4 Content addressing

**Content addressing.** `txid = SHA-256(deterministic CBOR of the body map)`,
excluding the signature array. SHA-256 is adequate post-quantum: Grover
reduces an ideal 256-bit preimage search from ~2^256 to ~2^128 *queries*. Calling
that "128-bit security" compresses quantum circuit cost, parallelisation limits and
hardware realities into one number — all of which push the practical margin up
rather than down.

---

## 2. Primitives

```
keyhash   = bstr .size 32          ; SHA-256 of the deterministic CBOR encoding
                                   ; "ASCENDING keyhash" anywhere in this
                                   ;   document is LEXICOGRAPHIC order of the
                                   ;   raw 32 bytes [2026-09-02] — identical to
                                   ;   big-endian numeric order; stated so no
                                   ;   little-endian reading survives
                                   ; of KeyMaterial (§2.2), the fixed-order
                                   ; PAIR of COSE_Keys, never one of them
timestamp = uint                   ; seconds since Unix epoch; u64 RANGE.
                                   ; **A transaction's timestamp is when it TAKES
                                   ; EFFECT** — not when it was drafted and not
                                   ; when either signature was applied. The body
                                   ; is fixed before anyone signs and signatures
                                   ; may be gathered with any delay (§3.4), so
                                   ; the value is the effective time the parties
                                   ; agreed at assembly, not an observed moment.
                                   ; Never checked against a local clock at
                                   ; structural verification (§3.3); a recipient
                                   ; reads it as the parties' claim about when
                                   ; the relationship or event began
seqno     = [series, counter]      ; §2.3. NOT a single integer, and NOT
                                   ;   comparable as one
series    = uint                   ; U32 RANGE. ARBITRARY, never ordered
counter   = uint                   ; U32 RANGE. Monotone WITHIN one series

; NOTE: no `.size 8` here. It would read as a fixed eight-byte
; serialisation, which contradicts the shortest-form requirement in §1, a 2026
; timestamp encodes in five bytes, not nine. These are range constraints only;
; encoding is always shortest-form deterministic CBOR.
```

**Identities are referenced by hash, never by key** (design §5.1). An **ML-DSA-44**
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

With f = 10 (design §3.2) each hop index needs values 0–9, so 4 bits suffice. A
depth-11 path — sufficient for 6×10¹⁰ nodes — occupies 6 bytes.

**A path may be empty** [author, 2026-09-01]: zero nibbles, the empty byte
string, count 0 — `{1: h'', 2: 0}` is its one encoding. It is the
**self-anchor** case: a root has no ancestor to name, so it names itself, and
the path from itself to itself has no hops.

**The packed byte string MUST be exactly `ceil(nibble_count / 2)` bytes.** Trailing
surplus bytes are malformed, not ignorable — otherwise one logical path has
unboundedly many encodings.

**Nibble values MUST be 0–9**; 10–15 are malformed. **On an odd nibble count the
unused low nibble of the final byte MUST be zero.** Without that rule one
logical path has sixteen valid byte encodings, and deterministic CBOR does not
fix semantic malleability inside a byte string. The explicit
nibble length is required because **paths are truncatable** (design §12.1): a distant
node receives only the prefix needed to route to the right region, and truncation
must be expressible at nibble granularity rather than byte granularity.

**A signed locator always carries the subject's complete path.**
The signature covers the path — standalone via `SignedLocator`, in a transaction via
the envelope — so no intermediary can truncate one without breaking it. Truncated
prefixes exist only in **unsigned aggregate routing state** held by distant nodes,
and are never what a resolution starts from: §7.7's *full, unmodified path* is
guaranteed by the input being signed, not by a completeness marker.

### 2.2 Key material

Identity keys are a **COSE_KeySet** (RFC 9052 §7) of exactly two entries in fixed
order — classical first, post-quantum second.

```
KeyMaterial = [ COSE_Key, COSE_Key ]   ; [classical, post-quantum]
                                       ; fixed order; a COSE_KeySet
```

design §5.1 binds an identity to **both** components, so a single
`COSE_Key` cannot represent one. The order is fixed because the keyhash is taken
over the encoding: reordering would produce a different identity for the same
keypair.

**Profile: the post-quantum component is ML-DSA-65** (`alg = -49`).

**Signing is deterministic** (the ML-DSA variant without added randomness).
Both variants verify identically, so this affects nothing on the wire — but test
vectors must pick one, and reproducible signing makes a failing vector diagnosable
rather than merely repeatable.

**Ed25519's parameters come from RFC 9053**: `kty` = OKP (1), `crv` = Ed25519 (6),
public key in `x` (-2).

**That only three labels may appear is this profile's rule, not the RFC's.**
RFC 9053 says which parameters an OKP key requires; COSE keys may also carry common
parameters such as `kid`, `alg` and `key_ops`, and the standard does not forbid
them. **`KeyMaterial` forbids them** — no `kid`, no `key_ops`, nothing beyond the
three above — because the keyhash is taken over the encoding, so **any additional
parameter yields a different identity for the same key.**

**The ML-DSA component uses RFC 9964's AKP key type.** The RFC supplies the
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

**Algorithm identifiers are encoded as COSE `int`.** The COSE registry is not
restricted to negative values, but **the algorithms this design requires happen to
be negative** — EdDSA is −8, ML-DSA-44/65/87 are −48/−49/−50, so a `uint` field
cannot encode them, and a `uint` field would make the mandated algorithms
unrepresentable. `int` is a profile decision that accommodates the
registry; it is not a claim that COSE identifiers are always negative.

Algorithm profile (design §5):

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
  3: seqno        ; the node's own {series, counter} pair
}
```

The `seqno` here **is** design §6.2.1's sequence number, not a separate field
(design §6.2). Its **counter** is incremented on every position change **and on every
endpoint change** (§7.6), serving both as freshness test and stale-cache detector; its
**series** identifies which line those counts belong to. Within a relationship it
is advanced only by §4.6's patron-countersigned reissue; **a new adoption
establishes its relationship's series**, countersigned by the same party a
reissue would need. Whether to adopt an established key that presents no
history — a fresh series on a visibly non-fresh key — is the patron's
discretion, like every adoption (design §16.7). A root's own line has neither
counterparty nor gate: a root changes series by continuing its chain under a new
designator — the uncountersigned rollup §4.6 closes with. **One series per patron relationship** is the expected
shape — a node bound under two patrons keeps two, which is what stops its counts in one
subnet disclosing its activity in another (design §19.4, P36).

**Endpoints advance it because the malformed-on-equal rule requires them to.** §7.7.3 treats equal `seqno` with different contents as malformed rather
than a tie to break, on the ground that a subject advances its own counter — so **a
subject that cannot advance has no way to publish a change at all.** An infra node
that changes address without changing position would otherwise be stuck advertising
a dead one until it happened to move, which is precisely the node §7.6 exists for.
One counter, two triggers; a reader learns that something it must re-fetch has
changed, which is all the counter ever said.

**Verification rule**: two `seqno`s are comparable **only when their `series` are
equal.** Within one series a new `counter` must be **strictly greater** than the last
one the verifier holds — **not** exactly previous+1, since a verifier may
legitimately have missed intervening transactions and requiring contiguity would
reject valid updates. Absence of prior state is not a failure.

**Across series there is no order, and a reader MUST NOT invent one.** `series` is an
**arbitrary** 32-bit label, not a generation number: it is never compared, never
assumed to increment, and two records in different series cannot be ranked by any
party holding only those records. Which series a node is currently on is **proved,
not inferred** — by the chain of §4.6, presented on request.

**The arbitrariness is the defence, and ordering would remove it.** A counter can be
exhausted: one record at the top of the range leaves no successor, and §7.7.3 makes an
equal `seqno` carrying different contents malformed rather than a tie — so a party
holding the node's key could otherwise pin it permanently, and §2.3's own reasoning
says why that is fatal (*"a subject that cannot advance has no way to publish a change
at all"*). Were `series` ordered, the same record could simply name the top series and
the attack would survive one level up. Unordered, there is no top to name: an attacker
must exhaust a 2³² space **and deliver every one of those series to every reader it
wants to block**, because nothing registers a series globally and poisoning is
therefore per-reader.
**Two carriage forms**. design §12.1 requires that routing information be
authenticated by the participant it describes — true via the envelope inside a
transaction, and via its own signature at introduction, where a locator is handed
over alone.

```
SignedLocator = {
  1: keyhash,        ; subject — whose position this describes
  2: Locator,
  3: COSE_Sign1      ; BY THE SUBJECT over canonical CBOR of fields 1-2;
                     ; external_aad = "rhtn/1:locator".
                     ; Classical only: a locator's relevance expires when the
                     ; node next moves, so design §5.1's horizon does
                     ; not apply
}
```

- **Inside a transaction body**, a bare `Locator` suffices, the enclosing
  envelope signature authenticates it.
- **Standalone**, as at introduction, a `SignedLocator`
  is required. A bare `Locator` presented alone MUST be rejected.

A bare locator carries no
signature of its own, because an unsigned locator lets any relay substitute
itself as the node's mailbox (design §12.1).

---

## 3. Common envelope and structural verification

```
Envelope = {
  1: uint,              ; schema version — 1 is current
  2: uint,              ; message type (see §4)
  3: { * uint => any }, ; body — type-specific
  4: COSE_Sign          ; RFC 9052; TWO COSE_Signature entries per required
                        ; logical signer — one classical, one PQ (§3.5)
}
```

**Signature coverage is the body map only.** Never the envelope, or signatures
would not survive the addition of a signer. The COSE payload is therefore the
deterministic CBOR of field 3, and the signed input is the `Sig_structure` COSE
builds around it.

### 3.1 Common body field: chain back-pointers

**Every transaction body carries key 0**, reserved across all types:

```
0: [ + [ + bstr .size 32 ] ]  ; chain back-pointers, one LIST per required signer,
                     ; in SIGNER ORDER as defined below.
                     ; Each list holds SHA-256 of that signer's previous
                     ; transaction(s), length 1 normally, longer at a merge.
```

**Signer order is the order the type's own schema introduces its required signers**,
and it must be stated because *"the required signer set"* names an unordered thing.

| Type | Signer order |
|---|---|
| Adoption (§4.1) | node (field 1), then patron (field 2) |
| Departure (§4.2) | the departing node |
| Disavowal (§4.3) | the issuing patron |
| Peering (§4.4) | endpoint A, then endpoint B, as fields 1 and 2 |
| Presence (§4.5) | the two participants in field 3's order, then witnesses in field 4's order |
| Series reissue (§4.6) | node (field 1), then patron (field 2) |

**Getting this wrong is silent.** Every hash and signature still verifies while each
predecessor is attributed to the wrong signer, so a mismatch surfaces only when
someone walks a chain and finds it does not reach back.

Each participant's archive is a hash chain, so sequence
position is as trustworthy as the record itself. Because the back-pointer sits in
the **signed body**, altering a record's position requires forging the *next*
record's back-pointer, which its counterparty already signed. Excision is
therefore impossible; only truncation remains, and it cuts at the ends rather than
the middle — an earlier head drops what is recent, a checkpoint drops what is early
(design §10.1, §10.2).

**This applies to every transaction type, not only presence records.** Adoption,
departure, disavowal and peering all advance their signers' chains.

**A signer with no prior transaction uses the genesis value**, `SHA-256(the
signer's keyhash)` — over the 32 keyhash bytes themselves, not over a CBOR
encoding of them — derivable by any verifier, so a claimed first transaction is
checkable rather than assertable.

**One back-pointer LIST per required signer**, since each signer has their own
independent archive. An adoption advances both the node's archive and the patron's.

**Length > 1 is a merge**. A signer whose archive has forked
— typically from concurrent use of two devices — reunites it by supplying **both**
branch heads in the next ordinary transaction. **No merge transaction type
exists**; a merge is an ordinary transaction with a longer back-pointer list.
**A list of length greater than one is sorted ascending bytewise** — one logical
merge, one encoding, one txid. Deterministic CBOR orders map keys and not array
elements, so without this rule the same merge would have as many valid txids as
its heads have permutations.

A merge **commits to both branches**: omitting one afterwards leaves a
back-pointer unsatisfied and is detectable. The archive is therefore properly a
**Merkle DAG**, not a chain, and cross-branch ordering is deliberately not
recovered, the structure proves that no intermediate record is missing, which
requires reachability rather than sequence.

A decoder MUST accept lists of **any length from 1 to 8** — the bound §1.3 sets —
and MUST verify every back-pointer present, not merely the first.

### 3.2 Structural rules for presence records

Rules a validator checks from the record alone. All were previously unstated.

- **The two participant identities MUST differ.**
- **`finalized_at` MUST be ≥ `started_at`, and MUST NOT exceed it by more than 24
  hours.** The lower bound alone lets a body name any future instant, and every
  envelope signer's next record must clear it (§3.3's monotonicity) — so one
  disposable identity could freeze the chains of a victim and its whole witness set
  until a date it chose. **This is checkable with no clock**: both values are in the
  record, and the rule constrains their difference rather than either one against the
  reader's time, which is why it can be structural where §3.3's timestamp rules
  cannot. 24 hours is far beyond any honest finalization — an unanswered slot is simply
  absent from the record (§5.5), so a ceremony never waits on an absent
  verifier — and 24 hours is a figure the design already carries rather than a new
  one (design §21).

  **This bounds the gap, not `started_at` itself**, and the two need different
  mechanisms. A body claiming `started_at` in 2100 with `finalized_at` an hour later
  satisfies this rule and poisons chains just as effectively; what stands against
  *that* is a witness declining to sign a ceremony whose claimed day its own
  clock contradicts (`light-client-requirements.md` §1.2), which no validator
  can check. **Structural here,
  client-side there** — the difference is that this rule compares two values the
  record already carries, and the other needs a clock the reader does not have.
- **A presence record on the wire is always final.** A ceremony the participants
  abandon is local state and is not published; `finalized_at` records when
  assembly closed, and **the verifier sample does not gate it** — an unanswered
  query simply yields no response (§5.5). Late responses arrive as
  `LateResponse` objects (§7.4) and never alter the original record's validity.
- **`strongest` MUST appear among the channels with `result = pass`, and no
  higher-ranked channel may appear with `pass`.** Ranking is UWB > NFC > optical >
  latency (design §7.6.3). Without the second half the field is not
  deterministic. *Proximity is disclosable (§4.5.1), so this is the one structural
  rule checked only when its field is revealed — withheld, it reports
  unverifiable, never valid and never malformed. **Revealed and violated, the
  record is malformed**, like any structural failure.*
- **A key may appear in at most one formation record: its first.** A formation record's key 0 list for each signer MUST be exactly
  **the genesis value, `[ SHA-256(signer keyhash) ]`** — the same encoding every
  first transaction carries (§3.1).

  **Two distinct claims, easy to conflate.** *Structurally*: a validator checks the
  subtype, the absent evidence arrays, and the genesis back-pointers — all local to
  the record. *Honestly*: a conforming established key cannot produce one, since its
  true chain has a predecessor. **A cheating established key can** — it simply omits
  its history and signs — and a validator holding no history cannot tell. The design
  accepts that: the forgery's weakness is evidentiary (design §13.2), a formation
  record attests nothing anyway, and detection arrives with anyone who holds the
  key's real chain, against which the fork is visible.

  **The structural rule is what bounds the population.** An identity with standing
  minting formation records against fresh keys gets a set of mutually formed
  strangers with no standing with anyone — a candidate for nobody real.
- **Formation records omit fields 4 and 5 entirely** rather than encoding empty
  arrays, per §1's rule. Absence means empty.
- **Witness identities MUST be distinct**, and **MUST NOT include either
  participant.** Duplicates would leave the ascending-keyhash ordering undefined
  between equal keys; a
  participant witnessing their own ceremony is not an independent witness, which is
  the entire role.
- **A normal record MUST carry at least one witness whose attestation sets both
  `protocol_ran` and `both_responsive`** (bits 0 and 1) [author, 2026-09-03].
  Zero witnesses is the
  formation case and nothing else, a normal record with none is malformed rather
  than merely weak, since the subtype is what separates an uncorroborated bootstrap
  from an uncorroborated ordinary meeting (design §13.2) — **and a witness
  attesting nothing is not corroboration**: an entry with neither bit set is
  representable as partial or negative evidence, but it does not satisfy this
  floor, or the count rule would let a formally normal record carry no
  affirmative witness statement at all. `latency_bound` remains genuinely
  optional. The uncorroborated meeting stays expressible — as a formation
  record, where it is visible as exactly what it is.
- **One identity is one logical signer regardless of how many roles it holds.** A
  party that is both a witness and a verifier signs once **in each capacity it
  signs in** — as a witness it is an envelope signer, as a verifier it signs an
  embedded response, and those are different objects. **The envelope bound is 2 + 16
  = 18** (§1); verifier responses are embedded evidence and do not count toward it.
- **A witness's `nominated_by` MUST be one of the two participants.** Whether that
  witness actually belongs to the nominator's counterparty's neighbourhood is not
  checkable from the record and requires topology state, the schema records the
  claim, evaluation is the reader's.

### 3.3 Timestamps and monotonicity

**Timestamps are not checked against a local clock at structural verification.**
No maximum age or future tolerance applies; a decoder has no authoritative clock
to check against and inventing one would make validity depend on the reader.

**But time is not merely evidentiary, and MUST be monotonic against the
committed back-pointers — for every transaction type.** A record has two times
and the rule reads one of each:

- Its **effective time**, which is what it presents to a successor, is
  `finalized_at` for a presence record and the transaction `timestamp` (§1) for
  every other type.
- Its **own floor**, which is what the rule holds it to, is `started_at` for a
  presence record and the same `timestamp` otherwise.

For each signer, the current record's own floor MUST be greater than or equal
to the **effective time** of every record its key 0 list names, every
predecessor in a merge list checked; and a presence record's `finalized_at`
MUST be greater than or equal to its `started_at`. **A record violating any of
these is malformed.**

**The asymmetry is the point, for presence records.** A record must not have
*begun* before its predecessor *finished*, which is what comparing `started_at`
against `finalized_at` says. Comparing finish against finish would admit a
ceremony that opened while the one it names was still running.

**Stated for all types** because §5.4's pruning and §3.2's chronology bound rely
on time being monotonic along **every** verified chain — a rule binding presence
records alone would let a non-presence transaction bridge backward through the
chain and break both.

**Without that bound, backdating collapses verification.** `started_at` determines
the 730-day window, so *n* — and therefore the selection threshold — is computed
relative to a value the proposer chooses. **Claiming a `started_at` earlier than
one's own history yields n = 0 and a required verifier count of zero** — and
shifts the qualification window itself (§5.3), so the claimed day decides which
records may be counted at all.

**The back-pointer is what makes this checkable.** It is inside the signature and
names a record with its own `finalized_at`, so a subject cannot claim a ceremony
earlier than their own last one. **The bound is against the signer's own history,
not against anyone's clock** — which is the only monotonicity available in a system
with no global time.

**It is a floor, and not a rate limit.** Everything at or after the predecessor's
effective time is admissible, including days in the future, so the bound closes
backdating without licensing the claimed day: what limits an implausible
`started_at` is a witness declining to attest a ceremony dated far from its own
clock (`light-client-requirements.md` §1.2) — not anything checkable here. *(With deterministic
selection retired, there is no sample to reroll; the claimed day now decides only
the qualification window, §5.3.)*

### 3.4 What structural verification decides, and what it does not

**"Verify" means structurally valid, not effective.** A verifier confirms
encoding, signatures, and the structural rules stated here. Whether the adoption
*takes effect* — the patron has capacity, no cycle results, the node's prior
binding ended — depends on topology state a validator may not hold, and is a
separate question answered by §7.7's resolution and the receiver's own view. An API
returning one boolean for both is answering a question nobody asked.

**A back-pointer whose record is unavailable is not a failure.** Verifying a chain
means each *presented* record's back-pointers match the record following it; an
unfetchable predecessor means the chain is **incomplete**, which is a fact for the
caller to weigh, not a malformed transaction (design §10).

**If `KeyMaterial` is carried, its hash MUST equal the keyhash of the party it
describes** — in an adoption, field 1, the adopted node (field 5).
Otherwise a sender could present one identity's keyhash alongside another's
key material, and a recipient pinning from the transaction would pin the wrong key.

**A dereferenced proof-of-presence record needs none of §4.5.1's disclosable fields.**
The check is that the record exists and names these two parties, both of which are
body fields. A fully withheld record satisfies it.

**Structural verification does not dereference `proof_of_presence`.** The field
names a presence record by txid; confirming that record exists, and that it names
this node and this patron, requires fetching it. **That is an evaluation step, not a
structural one** — a validator holding only the transaction can check the reference
is well-formed and nothing more, which is the same boundary §4.1's *valid versus
effective* note draws.

**A verifier lacking one signer's key material has neither verified nor rejected
the object.** That is a third outcome, not a variant of failure: the encoding is
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

**The two signatures need not be produced together.** The body is fixed before
either party signs — `txid` covers it — so signatures may be collected in any order,
over any channel, with any delay. **Nothing in the envelope records how they were
gathered**, and a verifier cannot tell a co-present exchange from one assembled over
days. Only the finished envelope is specified.

### 3.5 The signer set

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
- **A nonempty unprotected header is malformed, not ignored.** Tolerating one
  would give a single logical object two byte encodings; "ignored" appears elsewhere
  in this document about *unknown map keys*, which are preserved and re-serialised,
  and that rule does not extend to COSE headers.
- **`kid` is present exactly when the surrounding structure does not already name
  the signer.** Envelope `COSE_Signature` entries carry it, since a transaction
  body names identities by role and the entries must be matched to them. **Embedded
  `COSE_Sign1` objects omit it**: a `VerifierResponse` names its verifier in a
  field, a `Recovery` proof its prior key, so a `kid` would be a second copy that
  could disagree with the first.
- **Nothing else appears in either header.** Any additional entry changes the
  protected bytes and therefore the signature, so an unrecognised header entry is
  a malformed object rather than a tolerable extension.
  Additional headers are rejected rather than ignored: the protected header is
  covered by the signature, so tolerating unknown entries there would let two
  implementations disagree about what was signed. **A nonempty unprotected header
  is malformed**, per the rule above — not ignored.
- **Both envelope entries carry the same `kid`, in the PROTECTED header: the
  32-byte identity keyhash.** RFC 9052 offers protected and unprotected buckets and a
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
tooling could verify it. Two standard entries stay within RFC 9052 and let each
component be verified independently — a verifier may check the classical
component for a fast path and the post-quantum one when the decision warrants
it.

### 3.6 Canonicality and version

**Canonicality applies to the whole envelope**, not only the signed body. A
non-canonical envelope is malformed and rejected even if its body verifies —
otherwise two encodings of the same message both validate.

**Version mismatch**: schema version 1 is current. **An unknown version MUST
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
| 6 | — | — | **Retired 2026-09-01.** The abuse report is §6.3's standalone signed object, not a transaction: it advances no archive, reaches only its addressee, and chains to nothing — the same grounds on which the registration transaction was retired 2026-08-28. Numbers are never reused |
| 7 | Series reissue | node + patron | Topology |

**A decoder MUST reject a retired or unassigned type value.** Retirement removes
the schema — there is nothing a type-6 envelope could be validated *as* — and an
unassigned value is §1's unknown-enum case. The tombstone reserves the number; it
does not readmit the bytes.

**There is no registration transaction.** A resource registration is **not
shared network state**: it is a signed `CatalogEntry` handed to one hosting node and
answered from that node's current table (§6.5, design §11.5). It advances no archive
(design §10), reaches nobody else, and has nothing to chain to — so an envelope
around it would carry a back-pointer nothing walks, a `txid` nothing references, and
a post-quantum signature justified only by an archive it never enters.

**A `CatalogEntry` is optional.** It makes a resource *addressable* — reachable
point-to-point, or able to call out to nodes. A locally-hosted application needs
none, being reached by asking its host directly. **The personalised catalog page a
node serves over a session is a different object entirely**, is not signed, does
not propagate, and names the roles the viewer holds (design §11.5).

```
Scope = uint / [uint, uint] / [uint, [ + keyhash ]]
      ; 0 self | 1 down(n) | 2 up(n) | 4 siblings
      ; 5 dunbar | 6 list([keyhash])
      ; 3 is RETIRED and MUST NOT be reused. It encoded sub(n), "the subtree
      ;   rooted n levels above the owner", which described a shape a trust
      ;   horizon does not have (design §15.1); a decoder meeting tag 3 rejects
      ; forms taking a depth encode as [tag, n]; list encodes as [6, [...]]
      ; the list is in ASCENDING KEYHASH ORDER WITH NO DUPLICATES, and
      ;   violating either is malformed. The list is signed, so a decoder
      ;   that accepted an unordered or repeating one would accept bytes
      ;   another decoder rejects, on an object neither may re-encode
```

### 4.1 Adoption (type 1)

Subsumes key rotation and recovery.

```
{
  1: keyhash,          ; node being adopted
  2: keyhash,          ; patron
  3: Locator,          ; node's resulting position. Its seqno opens the
                       ;   relationship's series at counter 0 — the same rule
                       ;   a series reissue states for its new series (§4.6)
  4: timestamp,
  5: ? KeyMaterial,    ; the ADOPTED NODE's key material, never the patron's
                       ;   — there is no discriminator on the wire, so the profile
                       ;   fixes it: the arriving node is the identity being
                       ;   introduced and a recipient pins from it.
                       ;   ALWAYS structurally optional, never a
                       ; validity condition, since whether the recipient already
                       ; holds the key is recipient state and a signed object
                       ; cannot depend on it. Senders include it when they believe
                       ; the recipient may lack the key
  6: ? Recovery,       ; present iff this is a recovery adoption. It is also
                       ;   that adoption's EVIDENCE (design §6.1.1): a
                       ;   recovery carries its presence half inside itself,
                       ;   so fields 8 and 9 are absent
  7: ? txid,           ; archive HEAD presented (design §16.7), one hash, not
                       ; a list. The chain gives the rest
  8: ? txid,           ; proof-of-presence record between these two parties.
                       ;   EXACTLY ONE of fields 6, 8 and 9 is present (design
                       ;   §6.1.1) — a recovery's evidence is its own block;
                       ;   none of the three, or more than one, is malformed
  9: ? Transfer        ; present iff this adoption is a transfer — the former
                       ;   patron vouching in place of a meeting
}

Transfer = {
  1: keyhash,          ; the FORMER patron, whose signature stands where a
                       ;   proof of presence otherwise would (design §6.1.1)
  2: COSE_Sign         ; by that former patron. COSE_Sign rather than
                       ;   COSE_Sign1 for the same reason field 3 of Recovery
                       ;   is: the signer is hybrid and one logical signer
                       ;   contributes two entries (§3.5)
}

Recovery = {
  1: keyhash,          ; prior key whose history is claimed
  2: [ + VerifierResponse ],     ; at least one, and at least one `match`.
                       ;   The PRESENCE half: a prior counterparty who met the
                       ;   subject again and recognised them.
                       ;   Sorted ascending by verifier keyhash — the witness
                       ;   rule, one set one encoding
  3: COSE_Sign         ; by the OLD key. The KEY half. COSE_Sign rather than
                       ; COSE_Sign1 because the old identity is hybrid and one
                       ; logical signer contributes two entries (§3.5)
}
```

**The node and the patron MUST differ — a node cannot hold authority over
itself — and the rule holds for every two-party type** [author, 2026-09-01]:
departure, disavowal, peering and series reissue reject the degenerate pair
identically, as presence records already reject equal participants (§3.2). For adoption it is also the degenerate cycle — the proposed
patron *is* the node — and the one cycle a validator can see from the record
alone, where design §6.2.5's rule otherwise needs topology state. For the
two-signer types the envelope layer agrees independently: a required signer set
collapsing to one identity collides with §3.5's no-duplicate-signers rule.

`Recovery` is present iff the adoption claims a predecessor's history — the
recovery adoption of design §9 — and absent otherwise. A **plain rotation
carries nothing** (design §9): on the wire it is an ordinary adoption,
indistinguishable from a fresh one, and no `Recovery` appears. **There are no
variants of the block itself**: every claim of predecessor history carries both
halves, the old key's signature and a prior counterparty's `match` (below).

**Keystream seeds are local, private and never on the wire.** A participant's
seed for a counterparty's captures lives in that participant's own record of the
transaction (design §7.5.2) and is exchanged only over the direct channel during
a ceremony. **It is not a field here, and no conforming client writes one anywhere in a
record — extension keys included, where no validator could recognise one** (§1
preserves unknown keys; Appendix A states the force of client rules). Nothing in
the evidence a third party evaluates depends on it, and placing it in a signed
object would hand every reader the key.

**Consistency rules for a Recovery block.** Each closes a case where every
signature verifies and the assembly still means something other than it claims:

- **A verifier's authentication inside a `Recovery` block is HYBRID**, not
classical — **field 9 only.** Every other embedded signature is classical because
its relevance expires; **a recovery's does not** — it induces a permanent identity
change that a later evaluator cannot revisit, so a forged classical `match` would sit
inside an authentic post-quantum record indefinitely (design §5.1). Field 9 becomes an
untagged detached `COSE_Sign` carrying one Ed25519 and one ML-DSA-65 entry (§4.5).

  **Field 7 stays classical**, and the reason is not cost. A recovery response's
  `subject` MUST equal the newly adopted node, so **field 7 is signed by the very key
  an attacker mounting a fraudulent recovery already controls** — hybridising it
  protects nothing. Field 9 forges a *verifier's* attestation, which is the attack.
  Field 7's protection is anti-oracle and expires with the ceremony window; field 9's
  is permanent. Cost follows: ~10 responses at 3,373 B is ~34 KB, one pair each.

**`prior_key` MUST differ from field 1** — identical keys represent no rotation
  at all [author, 2026-09-01]. A holder who kept
  their key and lost their archive needs no `Recovery`: refetch from a holder
  (§7.9), or adopt afresh on a new series and merge the old branch back when its
  head resurfaces (§3.1, design §10.3). A same-key `Recovery` would be vacuous
  evidence — the old key's signature and the continuity attestation each prove a
  key the subject already holds — and vacuous evidence in a permanent identity
  object is surface with no use.

**Every response's field 8 MUST equal `prior_key`** — otherwise evidence about
  one old identity is embeddable under a claim about another.
- **Every response's `subject` MUST equal the newly adopted node** (field 1). The
  verifier is attesting continuity *to* the new key; a response naming a third
  party attests something else.
- **Every response's verifier MUST differ from its subject** — field 1 from
  field 2 [author, 2026-09-08]. A successor cannot supply its own continuity
  attestation: §7.3 already holds that the party being established "is never a
  candidate for their own verification", and design §9.1 has the subject meet
  *someone they have met before*, which a key generated for this rotation is
  not. Stated here because it is the half of that principle a **patron can
  check from the block alone** — prior-counterparty status generally is not
  validator-visible, but this inequality is, and without it an attacker holding
  the old key satisfies both of recovery's factors by controlling one new
  identity.
- **Duplicate responses from one verifier are malformed**, as they are in a
  presence record (§4.5) and for the same reason: one verifier occupies one slot.
- **The querier is the verifier** (design §9.1): recovery runs §7.3 in reverse,
  the subject standing in front of the party answering, so each response's
  `query_id` derives from a `VerificationQuery` whose field 2 names the
  responding verifier — a mismatch is malformed where the query is held. The
  pre-commitment in that query is the recovery meeting's own: the meeting opens
  as a ceremony (design §7.5.2) and yields this block instead of a presence
  record.
- **`selection_basis` MUST be 0 (met) in every response here** [2026-09-02]: a
  recovery verifier is by definition a prior counterparty who recognises the
  subject (design §9.1). Values 1 and 2 are malformed inside a `Recovery`
  block.
- **An old-key proof and verifier responses both appear, always.** They are
  independent evidence of the same continuity — one cryptographic, one human — and
  **neither substitutes for the other**, which is why both are required rather than
  weighed against a threshold.

**A recovery requires both, always: the old key and a present human.** Field 3
carries the prior identity's own signature and field 2 carries **at least one
`match`**, and neither substitutes for the other. **Key alone is not recovery**,
because a thief holds the key too and could rotate with it; **presence alone is not
recovery**, because a verifier can be mistaken or lying and nothing checks it.
Together they cost an attacker both the secret and a person willing to assert a
meeting that did not happen.

**There is no lost-key variant** (design §9.0.1). A key you cannot sign with is
not recoverable through this protocol: you make a new identity and are re-adopted by
people who know you, which in a network of close acquaintance reconstitutes access
faster than any mechanism here would.

**Each response in field 2 MUST carry field 8**, naming the prior identity, and the
verifier's signature MUST cover it. Otherwise the verifier has attested that some
face matches some subject, without ever signing which old identity that continues.

**Structural rule**: both fields are **required**, and field 2 carries **at
least one** response. `[+ …]` is the grammar, so an absent field 2 and a
present-but-empty array are both malformed — the second by §1's rule that empty
arrays are never encoded. **Neither half is optional**: there is one procedure, and it needs both.

**Field 3 is a `COSE_Sign`**, not a `COSE_Sign1` — the old identity is hybrid, so it
contributes two entries like any other logical signer (§3.5).

**It signs a successor statement, not the Recovery map.** The payload is the deterministic CBOR of:

```
SuccessorStatement = [
  prior_key,           ; keyhash — the identity being rotated FROM
  new_key,             ; keyhash — adoption field 1, the ONLY successor authorised
  patron_key           ; keyhash — adoption field 2
]
```

with `external_aad = "rhtn/1:successor"`.

**Signing the Recovery map with field 3 omitted would be a replay primitive**, which
is why it does not. That payload would say only *"a rotation happened"* and name no
successor, so **one observed proof would authorise an unlimited number of competing
successors**: extract it, build a fresh adoption naming the same `prior_key` with an
attacker-controlled new key and patron, insert the proof unchanged, and the envelope
signatures authenticate the assembly while the old key's statement contains nothing
to contradict it.

**A verifier MUST check `new_key` and `patron_key` against adoption fields 1 and 2
and reject on mismatch.** An unchecked binding is the same as no binding.

**It still cannot sign `txid`**, which hashes the body containing it. The successor
statement names the fields it needs directly, which avoids the circularity without
leaving the payload empty.

**Transfer field 2 signs a transfer statement, and for the same reason.** The
payload is the deterministic CBOR of:

```
TransferStatement = [
  node_key,            ; keyhash — adoption field 1, the node being transferred
  former_patron_key,   ; keyhash — Transfer field 1, this signer
  new_patron_key       ; keyhash — adoption field 2, the ONLY destination authorised
]
```

with `external_aad = "rhtn/1:transfer"`.

**Omitting the destination would be the same replay primitive one field over.**
A countersignature saying only *"I vouch for this node moving"* names no
destination, so one observed signature would authorise an unlimited number of
moves: extract it, build a fresh adoption naming the same node with an
attacker-controlled patron, insert the countersignature unchanged, and every
signature still verifies while the former patron's statement contains nothing
to contradict it. Naming all three parties is what makes the vouching specific
to *this* move.

**A transfer discloses no meeting, because it references none.** Field 8 names
a presence record, and §4.5.2 sets out what each exchange handling one can see;
field 9 names three keyhashes and a signature over them. So the two evidence
routes differ in privacy as well as in what they attest — the transfer route
reveals that a former patron vouched and nothing about any ceremony, which is
one reason a node with a usable former patron may prefer it.

**`former_patron_key` MUST differ from adoption field 2** — identical keys
represent no transfer, the same rule and the same reason as `prior_key` MUST
differ from field 1 in a `Recovery` — **and MUST differ from field 1**, since a
node does not vouch for its own move.

**A verifier MUST check all three against the enclosing adoption** — `node_key`
against field 1, `former_patron_key` against the `Transfer` map's own field 1,
`new_patron_key` against field 2 — **and reject on mismatch.** The same
sentence as above, and for the same reason: an unchecked binding is the same as
no binding.

**The evidence fields — 6, 8 and 9 — and exactly one is present.** design
§6.1.1: an adoption rests on a proof of presence between the two parties
(field 8, that record's `txid`), on the former patron's countersignature
(field 9, a `Transfer` block), or on a recovery's own evidence (field 6, a
`Recovery` block, whose responses are the presence half). **An adoption
carrying none of the three is malformed**, and one carrying more than one is
malformed too — they are alternatives, and an object offering two answers to
the same question invites a validator to pick.

**A recovery takes no separate reference and MUST NOT carry one.** Its
presence half is embedded rather than named: `Recovery` field 2's responses
come from a prior counterparty who met the subject again. A recovery adoption
carrying field 8 or field 9 beside field 6 is malformed on the same rule as
any other double answer.

**This is checkable without consulting anything outside the object**, which is
what lets it be a structural rule at all. A decoder does not ask whether the
meeting happened, whether the countersigner really was the patron, or where
either party sits in its own topology; it asks which field is present and
whether the signature in it verifies. §6.8's rule holds: no node rejects an
object its neighbour accepts.

**Field 8 is for a *fresh* adoption** — the patron holds no prior PoP with this
node. **Field 9 is for a node moving from a patron it already had**, which
covers lateral and vertical shifts (design §6.2.3): the former patron vouches
in place of a meeting, and where it will not or cannot, the move is an ordinary
adoption on a proof of presence like any other.

**What each is worth is not a structural question** (design §16.1). An observer
holding the topology can confirm that field 9's signer really was this node's
patron; one that cannot, weighs the edge lower for that reason. The decoder
takes no view.

**Field 7 is a single txid: the head of the archive prefix being presented.**
Not a list, not a range, not a proof.

**The chain already encodes everything else.** design §10 makes each transaction carry
its predecessors, so a patron given a head walks backward through the
back-pointers, fetching as it goes (§7.9) and stopping when it has seen enough or
stops recognising counterparties. Merges need no special handling, a record with
two back-pointers means both branches are reachable and both get walked.

**Truncation needs no grammar.** A subject presenting less history presents an
*earlier* head, and this field expresses that and nothing else. The chain's other
edit — dropping what is early — is carried by the checkpoint that licenses it
(§4.6, design §10.2) rather than by anything here, so the field still cannot express
what the archive model does not already allow.

**The patron chooses its own depth**, which is the right asymmetry: the presenter
picks the head and cannot control how far back the recipient looks, so the party
extending credit decides how much evidence it wants.

**Why not the alternatives:**

| Option | Rejected because |
|---|---|
| **List of txids** | Up to 8 KB of signed body that proves nothing, the patron must still fetch the records and follow back-pointers to confirm the list is a prefix rather than a selection. Duplicates the chain |
| **Range (from A to B)** | In a DAG a range is *reachable from B but not from A*, which is **precisely excision**. design §10 exists to make that impossible; this would return it as a protocol feature |
| **Merkle proof** | Largest body, highest complexity, and needs a tree imposed on a native DAG. It buys verification without fetching, but the patron **needs the contents anyway** to recognise counterparties, so it optimises a cost this use case does not have |

References rather than contents remains right, and is consistent with attestation
being pull (§12): the patron fetches what it wants to verify rather than receiving a
bulk push.

### 4.2 Departure (type 2)

```
{
  1: keyhash,          ; departing node
  2: keyhash,          ; patron being left
  3: seqno,            ; counter incremented within the current series (§2.3)
  4: timestamp,
  5: ? uint            ; reason code — same enumeration as §4.3
}
```

Single signature: the departing node.

**Required for a node to become a root.** Without it, a node that adopts
elsewhere remains in the old subtree's view indefinitely, since adoption says
nothing about existing bindings.

**A party with authority over another may never gate an action whose sole effect
is to end that authority relationship** (design §6.2). Present encoding: **the old
patron does not sign a departure, and a decoder MUST NOT expect a second
signature.** This is the escape hatch that makes exit a real right.

#### 4.2.1 Lateral and vertical shifts are not a separate type

Moving to a grandpatron, or to a patron's sibling, is **an ordinary adoption
whose counterparty happens to be nearby**. It warrants no type of its own, and
neither does moving between unrelated patrons: **there is no transfer transaction**,
because dropping the old patron was never a network operation (design §6.2).

But it has a property worth stating: when the new patron lies **inside the old
patron's replication horizon**, it already holds the node's history through
sibling replication (design §3.4), so **no archive presentation or re-verification is
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
  4: ? uint            ; reason code, 0..63 see below
}
```

Single signature.

**Field 3 orders the disavowal within the patron's own slot** (design §6.2.2):
the adoption that filled the slot and this record were written by the same
party from one clock. It orders nothing another party signed.

**There is no notice period.** A disavowal takes effect when signed — an
effective-at date could not work, since a disavowal is the issuer's own signed
statement and nothing stops them signing it whenever they choose: the date would be
a claim about the issuer's intentions rather than anything a recipient can check
(design §1.1).

**A party issuing a durable protocol-level negative attestation about another party
must express its basis only through a bounded, machine-interpretable category whose
adverse character is structurally visible, and must not attach arbitrary public
accusation text.** Present encoding: **field 4 is an enumerated code, never free
text.** Trust policies may
reasonably weight the stated reason, but free-form text on a permanently published
record is a defamation surface with no recourse mechanism. Enumeration also keeps
it machine-evaluable.

**Disavowal reason codes are an exception to §1's unknown-enum rule.** An
unfamiliar code in 0–63 is **retained and evaluated by its band**, not rejected —
the banding exists precisely so a policy can act correctly on a code it does not
recognise. Rejecting the transaction would make every future code a flag day.

**The code space is 64 values, banded so that prejudice is structural.** Bit 5 carries the distinction: **codes 0–31 are without prejudice,
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
| 5 | Cycle repair — the patron relation formed a cycle and this edge was cut to break it (§10.2) |
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

**The banding makes severity legible without making particulars so.** A code
says which band a patron placed an ending in and nothing about the reasoning
behind it. design §19.8 tested the composition that would undo that — a code
read beside a known resource and a revocation date — and withdrew it: the only
party holding both halves is the disavowing patron, who already knows them.

### 4.4 Peering (type 4)

```
{
  1: keyhash,          ; infra node A
  2: keyhash,          ; infra node B
  3: NetworkPoint,     ; A
  4: NetworkPoint,     ; B
  5: timestamp,
  6: ? uint,           ; replication commitment, bytes
  7: ? [ * Audit ],    ; most recent few only (design §6.3)
  8: txid              ; proof-of-presence record between the two peers.
                       ;   REQUIRED, and unconditionally so (design §6.3):
                       ;   §4.1's field-9 alternative is for a node moving
                       ;   between patrons it already had, and peers share no
                       ;   prior relationship to draw on. A peering record
                       ;   without it is MALFORMED
}

NetworkPoint = {
  1: bstr .size 4,     ; IPv4 address. v1 demands IPv4 (design §17.3);
                       ;   IPv6 endpoints are deferred by decision (design §4),
                       ;   and a 16-byte address here is malformed in v1
  2: ? uint,           ; ASN. U32 RANGE per RFC 6793 4-byte ASNs
  3: ? uint            ; UDP port, u16 range. Absent means the default 7431,
                       ;   and WRITING 7431 OUT IS MALFORMED (§1's
                       ;   default-omission rule) — omission is the one
                       ;   spelling, so §7.6's distinct-entries rule never
                       ;   meets the same destination twice.
                       ;   ZERO IS MALFORMED — it is never a destination
}

Audit = {
  1: timestamp,
  2: bool,             ; passed
  3: keyhash           ; challenger
}
```

ASN is exposed deliberately so policies can weight network diversity, and so
concentration (many nodes in one ASN) is observable. **Concentration is a visible
signal, not a trust input** — nothing in the metric consumes it. **The signal runs
one way**: many nodes in one ASN is evidence of concentration, while differing ASNs
are not evidence of independence, since ASN is routing and not the entity subject to
one legal order (design §17.3). The field is optional and self-asserted besides —
no IP-to-ASN validation is specified anywhere.
The audit list is pruned to the most recent few; peering is a status
rather than a trust-bearing history.

### 4.5 Presence record (type 5)

Field-for-field per design §8.1.

```
{
  1: timestamp,        ; started_at. In the body, not disclosable: §3.3's
                       ;   monotonicity and §5.3's 730-day window need the
                       ;   exact value, and finalized_at and key 7 already show
                       ;   the ceremony's day in every presentation
  2: timestamp,        ; finalized_at
  3: [ Participant, Participant ],
  4: ? [ + Witness ],  ; PRESENT on a normal record (1..=16), ABSENT on a
                       ;   formation — subtype-conditional, never free choice
  5: ? [ + VerifierResponse ],
                       ;   sorted ascending by verifier keyhash, ties broken by
                       ;   ascending subject keyhash [2026-09-02] — the tie is
                       ;   D15's one-verifier-answering-for-both-participants
                       ;   case; one set, one encoding. PRESENT iff at least
                       ;   one response arrived: ZERO RESPONSES OMIT THE KEY —
                       ;   §1's empty rule extended to this subtype-conditional
                       ;   field, so one logical record has one encoding, and a
                       ;   normal record with none is announced by key 4 and
                       ;   subtype regardless. ABSENT always on a formation
  6: uint,             ; record subtype: 0 = normal, 1 = formation
                       ;   (design §13.2). In the body, not disclosable: structurally
                       ;   load-bearing, and a formation record's absent keys 4
                       ;   and 5 announce it regardless
  ; key 7 (the seed-window ordinal) was RETIRED 2026-09-01 with deterministic
  ; selection; the number is not reused — and a decoder meeting it REJECTS
  ; [2026-09-02]: retired numbers are tombstones, not extension space, the
  ; same rule as §4's type 6 and Scope's tag 3. An unknown key is one the
  ; schema never assigned; a retired key is one it remembers
  8: bstr .size 32     ; disclosure root (§4.5.1) — commits to the seven
                       ;   disclosable fields (§4.5.1's label set): proximity,
                       ;   capture, location, and each Participant's retention
                       ;   and client integrity

  ; key 0 (chain back-pointers) is common to all bodies (see §3.1).
  ; NOTE: no patron signature appears in a presence record. Patrons do not
  ; countersign proof of presence (design §6.4)
}

Participant = {
  1: keyhash           ; retention and client integrity are DISCLOSABLE and
                       ;   travel in the disclosure set (§4.5.1)
}

ClientIntegrity = { 1: bool, 2: uint, 3: ? bstr .size (1..1024) }
                     ; attested, scheme, evidence. No structural relation ties
                     ; field 3's presence to fields 1-2: an
                     ; unattested client may still carry scheme evidence and an
                     ; attested one may omit it. Policy reads the combination
                     ; (design §7.8); a validator checks only the shapes

Proximity = {
  1: [ + Channel ],
  2: uint              ; strongest channel that passed
}
Channel = {
  1: uint,             ; 1 uwb, 2 nfc, 3 optical, 4 latency. A KIND MAY
                       ;   REPEAT [2026-09-02]: a channel retried is two
                       ;   measurements, both evidence — validators impose no
                       ;   uniqueness rule here
  2: uint,             ; 0 pass, 1 fail, 2 unavailable
  3: ? uint,           ; claimed resolution, metres. Syntactically optional with
                       ;   no presence condition: whether a kind carries one is
                       ;   the client's claim, and policy weighs it
  4: ? bstr .size (1..128)
                       ; session-key binding, where the channel provides one.
                       ;   Optionality is syntactic only, as field 3
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
Asserted      = { 1: uint, 2: tstr }              ; method, geohash
Corroboration = { 1: keyhash, 2: uint, 3: uint }  ; witness, method, radius_km.
                     ; Field 1 MUST name an entry of body field 4 [2026-09-02]:
                     ; a corroboration's authority is its maker's envelope
                     ; signature over the disclosure root, so one naming a
                     ; non-witness attests nothing. Checkable only where the
                     ; location disclosure is revealed.
                     ; radius_km is WITNESS-RELATIVE [author, 2026-09-03]:
                     ; the witness bounds the participant within radius_km of
                     ; ITSELF, and no coordinate is carried — interpretation
                     ; is by recognition of the witness (design §7.6.2, design §16.1)

; LOCATION METHOD REGISTRY, shared by Asserted.1 and Corroboration.2:
;   0 GNSS · 1 serving-cell · 2 network egress · 3 latency bound.
; DELIBERATELY OPEN, like §4.3's disavowal codes and unlike §1's default:
; evidence channels will grow, and rejecting an unknown method would make every
; new channel a flag day. Unassigned values are retained and left uninterpreted.

; GEOHASH: 3 or 4 ASCII bytes from the canonical lowercase geohash base32
; alphabet 0123456789bcdefghjkmnpqrstuvwxyz. Upper case, other
; text, or other lengths are malformed — one logical cell, one encoding, the
; same rule as path-nibble padding.

Witness = {
  1: keyhash,
  2: keyhash,          ; nominated_by — MUST be one of the two participants
                       ;   (§3.2). That it is the witness's counterparty is the
                       ;   ceremony's cross-nomination claim (design §7.1),
                       ;   not checkable from the record; evaluation is the
                       ;   reader's
  3: uint              ; attestation bitfield:
                       ;   bit 0 protocol_ran
                       ;   bit 1 both_responsive
                       ;   bit 2 latency_bound
                       ;   bits 3+ reserved; a decoder retains them and
                       ;   interprets only 0-2.
                       ;   A NORMAL record's witness floor counts only entries
                       ;   with bits 0 AND 1 set (§3.2) [author, 2026-09-03];
                       ;   entries without them are partial evidence, present
                       ;   but not corroborating
  ; fields 4-5 (nonce commitment and reveal) were RETIRED 2026-09-01 with
  ; deterministic selection; the numbers are not reused — and a decoder
  ; meeting either REJECTS [2026-09-02]: tombstones, not extension space
  ; (§4's type-6 rule, Scope's tag 3)
}

VerificationQuery = {
  1: keyhash,          ; subject
  2: keyhash,          ; querier
  3: bstr .size 32,    ; ceremony pre-commitment (design §7.4.1; contributory
                       ;   construction, design §7.5.2 [2026-09-02])
  4: bstr .size (1..4096),   ; fuzzed profile
  5: uint,             ; TEMPLATE VERSION the profile in field 4 was produced
                       ;   under (design §7.4.4). 0..=65535 — design §8.1's
                       ;   uint16, the same bound field 6 echoes [2026-09-02]. A verifier that cannot compare
                       ;   under this version answers `3 unavailable` with no
                       ;   basis — it has not evaluated. Without it a mismatched
                       ;   engine compares anyway and signs a `no-match`
                       ;   indistinguishable from an identity mismatch
  6: bstr .size 32,    ; query_id — SHA-256 of the canonical CBOR of THIS MAP
                       ;   WITH FIELD 6 ABSENT (fields 1-5 and 7), then stored
                       ;   here: a map cannot contain its own hash
  7: keyhash           ; the verifier this query is addressed to
                       ;   [author, 2026-09-03]. REQUIRED. A verifier MUST
                       ;   reject, before any processing, a query whose field
                       ;   7 is not its own keyhash: the subject's consent
                       ;   signs query_id, field 7 is inside it, and the
                       ;   consent therefore confines the profile and the
                       ;   ceremony metadata to the one verifier the selector
                       ;   named — without it the consent is bearer paper,
                       ;   authenticating the same query to any prior
                       ;   counterparty the selector cares to reach.
                       ;   Inside a Recovery block, field 7 MUST equal field 2:
                       ;   the querier is the verifier (design §9.1)
}

VerifierResponse = {
  1: keyhash,          ; verifier
  2: keyhash,          ; subject
  3: bstr .size 32,    ; query_id — matches VerificationQuery field 6
  4: uint,             ; 0 match, 1 no-match, 2 inconclusive, 3 unavailable.
                       ; THERE IS NO pending VALUE: an unreachable verifier
                       ; answers nothing, and its slot is ABSENT (§5.5) —
                       ; nobody holds authority to sign on its behalf
  5: ? uint,           ; basis: 0 photo_match, 1 personal_knowledge, 2 both.
                       ; REQUIRED when result is 0-2; MUST be absent when result
                       ; is 3 (unavailable) — a
                       ; verifier who has not evaluated asserts no evidence
                       ; basis, and no truthful value existed: photo bases
                       ; demand a template version that may not exist, and
                       ; personal_knowledge as a sentinel is structurally valid
                       ; and false
  6: ? uint,           ; template version, 0..=65535 — design §8.1's schema
                       ;   says uint16, and a larger value is malformed.
                       ;   REQUIRED when basis is 0 or 2;
                       ; MUST be absent when basis = 1 or absent. The optional
                       ; marker is syntax; result and basis determine presence
  7: COSE_Sign1,       ; SUBJECT's countersignature. Payload: the RAW 32 BYTES of
                       ; query_id (field 3) — not a CBOR bstr wrapping them, and
                       ; not the query itself (see §5.6).
                       ; external_aad = "rhtn/1:consent".
                       ; ALWAYS CLASSICAL, including inside a Recovery (§4.1).
                       ; A verifier MUST reject a query lacking it, and MUST reject
                       ; one whose fuzzed profile differs from another countersigned
                       ; under the same ceremony pre-commitment
  10: uint,            ; selection basis, the SELECTOR's claim (§5.5),
                       ;   carried to the verifier as the third element of the
                       ;   type-4 request body (§5.6) [2026-09-02].
                       ;   TIER-ALIGNED (§5.1) [author, 2026-09-03]:
                       ;   0 met (a PoP of the selector's own - tier 1)
                       ;   1 in a trust horizon of the selector (tier 2)
                       ;   2 reachable (one further edge, tiers 3-4)
                       ;   3 discretionary fill
                       ;   Renumbered from the three-value vocabulary that
                       ;   folded tiers 1-2 into one value: met and
                       ;   merely-in-horizon are different security facts
                       ;   (design §16.2.1) and the record retains the
                       ;   difference
  8: ? keyhash,        ; PRIOR identity being matched against. REQUIRED when this
                       ; response appears inside a Recovery block (§4.1), absent
                       ; otherwise. **MUST equal that Recovery's `prior_key`** —
                       ; without the equality check, evidence collected about old
                       ; identity X can be embedded under a Recovery claiming old
                       ; identity Y, and every signature still verifies. Without it the verifier's signature never
                       ; names the old identity it is attesting continuity with,
                       ; and the assertion the recovery rests on is unsigned
  9: COSE_Sign1 / COSE_Sign
                       ; BY THE VERIFIER. Payload: canonical CBOR of fields 1-8
                       ; and 10 of THIS map. External_aad = "rhtn/1:verifier".
                       ; **COSE_Sign1, classical, in a presence record.**
                       ; **COSE_Sign, hybrid, inside a Recovery block** (§4.1) —
                       ; untagged, detached, one Ed25519 and one ML-DSA-65 entry.
                       ; The field's type is fixed by where the response sits
}

**In an ordinary presence record both signatures here are `COSE_Sign1` and
classical-only**, unlike envelope signatures: they are evidence embedded inside a
hybrid-signed body, so substituting them breaks the envelope signature and their
authenticity is protected transitively (see design §5.1). Hybridising all 64
signatures would cost ≈ 211 KB against ≈ 4 KB classical. **The consent signature
(field 7) is classical everywhere. Verifier authentication (field 9) is the one
exception: inside a `Recovery` it is a hybrid `COSE_Sign`** (§4.1) — a recovery
induces a permanent identity change, so that one signature's reliance never
expires.
```

**Field 6 is load-bearing**: a formation record has empty witness
and verifier arrays permanently, and **must never age into looking like a normal
record**. Typing it explicitly means no policy can mistake self-attestation for
independent attestation.

**Deliberately absent** (design §8.1): biometric templates, photographs, raw
latency samples, precise coordinates. Everything identifying stays on the
participants' devices.

---

#### 4.5.1 Selective disclosure

**A holder can present a presence record without the fields a given recipient has no
use for.** Scoped deliberately: this hides **location, retention, client
integrity, capture parameters and proximity channels**, and hides **nothing else**.
See design §8.1.1 for what it does not reach and why. *`started_at` and `subtype`
are body fields, not disclosable — the structural rules consume them, and
withholding them concealed nothing the body does not already show (§4.5).*

##### 4.5.1.1 The construction is a digest list, not a tree

**Each disclosable field becomes a salted digest; the body commits to the sorted
list of digests.**

```
Disclosure   = [ bstr .size 16, tstr .size (1..32), any ]
                 ; salt, label, value
digest(D)    = SHA-256( 0x00 || deterministic CBOR of D )
root         = SHA-256( 0x01 || concatenation of all digests, ascending by label )
```

`root` is body field 8. **Labels are the field's path**, so a decoder knows what it
is looking at without a table. **Exactly seven, in ascending byte order**:
`capture`, `location`, `p0.integrity`, `p0.retention`, `p1.integrity`,
`p1.retention`, `proximity`. A label outside this set is malformed.

**Each label's value is the CBOR the field carried when it lived in the body**, stated because the move otherwise orphans the schemas:

| Label | Value |
|---|---|
| `proximity` | the `Proximity` map (§4.5) |
| `capture` | the `Capture` map (§4.5) |
| `location` | the `LocationEvidence` map (§4.5) |
| `pN.retention` | `uint` — years the capture is retained (design §7.5.1) |
| `pN.integrity` | `{ 1: bool, 2: uint, 3: ? bstr }` — attested, scheme, evidence (design §7.8) |

**Why not a Merkle tree.** design §19.3 proposed one, following SD-JWT loosely. At
the leaf count here — seven — a tree buys nothing: inclusion
proofs would cost three hashes each where sending every digest costs seven, and a tree
adds real hazards a flat list does not have, **odd-node handling and the
duplicated-node second-preimage class**. SD-JWT's own construction is a digest array
for the same reason. **The 0x00 / 0x01 prefixes are still required**, so that a
digest can never be reinterpreted as a root or the reverse.

##### 4.5.1.2 Salts are mandatory

**Every disclosure carries a fresh 16-byte salt.** Without one, an undisclosed
field is recovered by brute force from its digest: `subtype` has two values,
`liveness` three, a precision-3 geohash about 32,000, and a timestamp is guessable
within the ceremony window. **A salted digest is the only thing that makes withholding
mean anything here.**

**Salts are agreed during the ceremony**, because both participants sign one body and
must therefore compute one root. They are ordinary record state afterwards, held by
both participants and by anyone given a full record.

##### 4.5.1.3 What travels: the presentation

**A presented record is the envelope plus exactly seven disclosure slots, in
ascending label order.** Stated as a schema because prose alone left
encoders free to invent containers that could not interoperate:

```
PresentedRecord = [
  Envelope,                  ; the signed transaction (§3), untouched
  [ 7*7 DisclosureSlot ]     ; one per label, ascending by label byte order
]
DisclosureSlot = Disclosure / bstr .size 32   ; revealed, or the withheld digest
```

**Position supplies the label.** Slot *i* belongs to label *i* of §4.5.1's sorted
set, so a withheld digest needs no label of its own — a revealed `Disclosure` whose
embedded label differs from its slot's is malformed. Exactly seven slots always: a
shorter or longer array is malformed, and a fully-withheld presentation is seven
digests.

**A recipient verifies by recomputing `root`** from what it holds — revealed
disclosures hashed, withheld digests taken as given — and checking it equals body
field 8, which the envelope signature covers. A mismatch means the presentation is
malformed, not that a field is missing.

**Withholding is visible, and that is deliberate.** All seven slots are always
present, so a recipient always knows a field exists and was withheld. This is the
same posture as §5.5's unanswered queries and `unavailable` responses: absence is
legible rather than silent, and a policy may weight it.

##### 4.5.1.4 Cost

**+16 bytes per disclosable field at rest** — about 112 bytes on a ~35 KB record,
**0.3%**. A minimised presentation carries 32 bytes per withheld field, at most 224
bytes. **Presentation size does not otherwise fall**: a presence record is ~96%
signatures and the envelope requires exactly the required signer set, so a minimised
record is still ~34 KB. This is a disclosure measure, not a bandwidth one.

##### 4.5.1.5 What a decoder MUST do

- **Reject a record whose recomputed root does not equal body field 8.**
- **Reject a slot count other than seven**, a revealed label differing from its
  slot's position, and a label outside the set above.
- **Reject a `Disclosure` whose salt is not exactly 16 bytes.**
- **Reject a revealed value that does not match its label's schema** (the table
  above) — a disclosure is not an extension point.
- **The `strongest`-channel rule (§3.2) is checked when `proximity` is revealed**,
  and reported as unverifiable — not valid, not malformed — when it is withheld.
  It is the one structural rule living in a disclosable field.
- **Accept any subset of disclosures, including none.** A minimised record is
  well-formed; only a root mismatch is malformed.
- **Never treat a withheld field as a default value.** Withheld is not zero, not
  absent, and not `unavailable` — it is unknown, and §4.5.2 says which recipients may
  require it.
- **There is no aggregate verdict.** *Malformed* is terminal;
  everything else — missing keys, unavailable history, a withheld field, each
  subject's half — is an independent dimension a validator reports separately.
  **Collapsing them into one boolean is a policy act** (design §16.1), not a
  validation result: one evaluator may treat any unverifiable dimension as
  disqualifying while another accepts a verified half, and both read the same bytes.

#### 4.5.2 Which exchanges see the disclosable fields

**Stated per exchange, because a holder needs to know what a given recipient will be
able to read.** design §8.1.1 sweeps the same exchanges to establish *why* each field
is withholdable — what each one **reads**, and whether it has any use for location.
This table answers the holder's question instead: what a given recipient **sees**.
**The row sets are close but not identical**: the design counts the trust metric,
which reads only *that* an edge exists and none of a record's disclosable fields
(design §16.2.1), and combines the two `txid`-only exchanges that appear
separately here.

| Exchange | Disclosable fields |
|---|---|
| Ceremony, at creation (design §7.1) | **All.** Both parties construct the body |
| Witness signing (design §7.1) | **None.** The witness contributes its own corroboration (design §7.6.2); it receives no disclosed field |
| Verification by query (§5.6, design §7.3) | **None.** A verifier receives a fuzzed profile and a query id, never the record |
| Verifier selection (§5) | **None.** Selection is the selector's judgment over the handed bundle; nothing in the record replays it |
| Response accounting (§5.5) | **None.** Counts and reads field 5 |
| Structural verification (§3) | **None**, with one stated exception: the `strongest`-channel rule lives in `proximity` and is checked only when revealed. Everything else — signatures, back-pointers, timestamps, subtype rules, participant distinctness — reads the body |
| Adoption's proof-of-presence reference (§4.1 field 8) | **None.** Confirms the record exists and names these two parties |
| Archive fetch by a prospective patron (§7.9) | **Holder's choice.** The only exchange with a use for location |
| Presence-based recovery (§4.1 `Recovery`) | **None.** Reads field 9 |
| Late verifier response (§7.4) | **None.** References `txid` |
| Capture key grant (§7.3) | **None.** References `txid` |

**Nine of eleven exchanges need none of it**, which is what makes the mechanism worth
its 0.3%. **A conforming client withholds by default and reveals on the holder's
instruction**, rather than the reverse.

**No exchange has a slot in which to demand a disclosable field.** The interface for each exchange above is fixed, and design §1.1 makes the
evidence schema the one thing that is not pluggable, so there is nowhere for
*disclose this or we stop* to be expressed: an implementation that tried would be
non-conforming at the wire, which is checkable.

**This is a statement about the schema, not an instruction to recipients.** A
recipient may refuse for any reason, and nothing here compels it to proceed — that
would be a rule aimed at a party whose acceptance policy is private, which design §1.1 calls
a wish. What the fixed interface buys is that a refusal is **the recipient's policy
rather than the protocol's**, and that every disclosure subset stays structurally
valid.

### 4.6 Series reissue (type 7)

**Starts a new `seqno` series for the node and does nothing else.** Two signers, the
node and its patron.

```
{
  1: keyhash,          ; the node
  2: keyhash,          ; the patron, who countersigns
  3: seqno,            ; the series being LEFT, at the counter it reached
  4: seqno,            ; the new series. Its counter is 0
  5: timestamp
}
```

**The patron's countersignature is the whole of the security property.** A `series`
cannot be advanced by the node's key alone, so a party holding a stolen key can
exhaust the current counter and cannot escape into a fresh series — while the
legitimate holder can, because their patron will sign for them and not for a thief.
This is the same social check adoption and recovery already rest on, applied to the
one operation that repairs an exhausted or poisoned counter.

**Seal the old series before leaving it, and field 3 is what records that.** A
32-bit counter advanced only on position and endpoint changes will not exhaust in a
lifetime, so **the top of the range is dead space the legitimate holder can spend.**
A node that believes its key is compromised sets the counter it is leaving to the
**maximum**, then reissues naming that value — after which nothing the thief signs
can supersede it, since a strictly greater counter does not exist and an equal one
carrying different contents is malformed (§7.7.3). **The exhaustion that made the
attack possible is the same move that closes the abandoned line behind you.**

**Order matters, and the schema enforces it.** Field 3 records the counter the old
series reached, so sealing must happen *before* the reissue or the chain will name a
departure point later records contradict. Seal, then reissue, then repeat for every
other patron relationship whose line is worth securing.

**Sealing is unilateral, and that is the point.** It is an ordinary self-signed
locator at the top of the counter (§2.3's standalone carriage form) — no patron, no
transaction, no countersignature. Only the *reissue* needs the patron, because only
the reissue creates new room. So a node can seal a line toward parties it has no
standing relationship with, in subnets where it holds no membership and could not
reissue even if it wanted to. What it cannot do is undo it: a sealed line is spent,
and reopening means a reissue and the countersignature that requires.

**Sealing requires naming the series, so a holder who lost their archive cannot
seal lines whose numbers went with it.** Benign where the device is lost rather
than stolen — nobody else holds the key, so the unsealed lines are dead space.
Where it is stolen, the thief holds the key and could out-sign a seal anyway;
the remedy is rotation (§4.1, design §18.3), not sealing.

**A root cannot produce a reissue, and does not need the transaction — for a
root its content is empty.** Type 7 requires a patron distinct from the node
(§4.1), and a root has none; an exception could not be checked, since nothing in
a record shows its signer is a root. What a root may still want is the **rollup
point** — a Tx0 on a new series, electable as a checkpoint for retention and
look-back exactly as a countersigned reissue is (design §10.0) — and that is an
**internal operation**: the root simply continues its hashchain under a new
series designator. The chain back-pointer is the real predecessor, not the
genesis value, so to an observer holding the chain it is an ordinary
continuation, distinguishable from a genesis event; **presented as a history
root, it is logically equivalent to one**, which is what a rollup point is. What
the internal transition lacks is what the countersignature supplied — a gate
against a thief — and for a root that gate never existed: owner and thief are
indistinguishable at the series layer, and root compromise is answered at the
subnet level (design §12.7, §13). §2.3's currency rule is unchanged: a series
claim is proved by showing the chain into it, and a series nobody can link stays
unprovable.

**A chain-holder does not need the seal.** Anyone holding §4.6.1's chain knows which
series was abandoned and **MUST reject records in it** whatever their counter. The
seal protects the parties who hold no chain — a cached locator and nothing else — and
against them it is a race the thief can win by reaching a reader first. That is the
argument for acting on suspicion rather than on confirmation.

**What a recipient ends up holding is an address that can never be updated.** The
seal occupies the top of the `(node, series)` sequence — the slot any future locator
for that pair would have to take — so the entry in that party's address book is final.
Not deleted: they still hold an address, and it still resolves if the position behind
it still answers. What is gone is anyone's ability to *move* it, the node's own
ability included.

**Writing the top slot is denial, not control, and it binds the writer too.** No
record supersedes a maximum counter, so a party that seals a series — the node or a
thief holding its key — can execute nothing further in that series either. Sealing
confers no continuing ability; it removes the series from use by everyone.

**So a thief who seals first burns a line rather than capturing one.** The parties it
reached first hold a frozen entry pointing where that record pointed, and the thief
cannot develop it: no further transaction in that series will be recognised by anyone
holding the seal. **The `keyhash` is untouched** — the legitimate holder reissues into
a fresh series with the patron's countersignature, which the thief cannot obtain
(§4.6), and continues. What the thief destroyed is one line the holder was leaving
anyway. The cost is re-contact for the parties that took the thief's seal: a chain
(§4.6.1) for those that will take one, out-of-band re-introduction for those that will
not.

**A seal reaches as far as it is carried and no further.** It is not a revocation and
there is nowhere to publish one: parties never contacted and subnets never entered
never see it, and the key is not dead to them. Nor does it need to be — **redirection
requires a cached locator to redirect**, so a party holding none is not exposed to
this attack at all, and a thief presenting the old key to them is attempting a first
contact, which design §12.3 Case 0 makes an out-of-band act.

**The new series MUST NOT be one the node has previously occupied.** Reusing one
brings its abandoned high-counter records back into comparison against the new line.
The node knows its own history, so this is local bookkeeping — and it is **checkable
by anyone holding the chain**, who MUST reject a reissue naming a series already in it.

#### 4.6.1 Proving which series is current

**Presentation, not propagation, and not a history scan.** A node proves its current
series by presenting:

- its **adoption** (§4.1), whose `Locator` in field 3 carries the `seqno` — and so the
  series — that the patron countersigned at that moment; and
- **each series reissue since**, each naming the series left and the series entered.

Every object in that chain is countersigned by the patron named in the node's own
path, so a recipient already knows the key that must have signed it. The chain
discloses the age of the patron relationship and the number of reissues taken, and
nothing else about what the node did.

**Chain length is the order.** Where two presented chains share an adoption and one
extends the other, the longer is current. Two chains that **diverge** are two
successors countersigned by the same patron — patron equivocation, attributable to
that patron by its own signatures, and handled the way this design handles
equivocation everywhere: made visible rather than prevented.

**A reissue is not a move.** Routing walks `anchor` and `path` (design §12.3), neither
of which a reissue touches, so cached locators keep working and no correspondent has to
be told. What changes is only which records rank against which.

## 5. Verifier selection — recognition, not recomputation

**Selection is by recognition, and the deterministic machinery is retired**
[author, 2026-09-01]. Two facts killed it. The candidate pool is a bundle the
subject curates (§5.4), and a deterministic pick over an adversary-populated
list is the adversary's pick with extra steps. And the determinism never served
the parties present — it existed to prove the pick's regularity to a distant
audience, who could only have checked it against the subject's full history as
of the ceremony, which the bundle model and the privacy posture both withhold.
The evaluation principle that was already true everywhere else (design §16.1)
now governs selection too: **what protects a party is recognising who answered,
not auditing how they were chosen.**

### 5.1 Who selects, and how

**Each participant selects the other's verifiers** — the party who performs a
selection is never the party it is about — from the counterparty's handed
bundle (§5.4), by the selector's own knowledge, in descending order of
trustworthiness:

1. **users the selector has met** — a PoP of their own with the candidate;
2. **users in any of the selector's trust horizons** — the two-edge walk of
   any subnet the selector belongs to (design §2);
3. **users someone in any of the selector's trust horizons has met**;
4. **users in the trust horizons of users the selector has met** — to the
   extent foreign topology is visible at all, which it usually is not.

Tiers 3 and 4 are one further edge over the graph of meetings and
horizon-mates: **a two-edge walk is the halting condition**, mirroring the
trust horizon itself. Reachability is the **selector's own judgment over their
own knowledge** — computed locally, provable to nobody, owed to nobody
(design §1.1).

**Where the initial bundles surface no common acquaintance, the parties go
fishing**: either may propose further candidates from their own history so the
other can test them against tiers 1–4 — an exchange over the ceremony's direct
channel, carried by no wire object and recorded nowhere. **A fishing proposal
is a bundle augmentation** [author, 2026-09-03]: proposing a candidate from
your own history is the same disclosure decision the bundle was, made
explicitly, and a client stops revealing once a locally adjustable number of
responsive candidates is found. **After the bundle, quality filtering stops**:
every mutually reachable candidate is accepted if available, so a party
declining available candidates to draw more names out is visible as exactly
that — the proposing side can see whether its offers are unavailable or
refused. **After common
acquaintances are exhausted, the selector fills the remaining slots at its own
discretion** from the counterparty's pool; responses from strangers are weak
evidence and are marked as such (§5.5).

### 5.2 The reasonableness criterion

```
required(subject) = min( floor(n / 2), 10, |candidates| )
```

`floor`, explicitly — an unstated rounding differs by one at every odd *n*. The
`|candidates|` term is necessary: without it a subject with twenty meetings
against one counterparty needs ten verifiers from a pool of one.

***n* is entirely the subject's claim** — it counts a bundle the subject
curates — so the formula is a **reasonableness criterion, not a security
check**: it sizes how many verifiers the selector should seek, and an evaluator
comparing a record's response count against it learns whether the ceremony was
diligent, never whether the subject's history is complete. Understating is
free and self-defeating (§5.4); no credence beyond that is warranted or
intended.

### 5.3 Qualification and the window

A record **qualifies** for *n* and the candidate pool when it verifies alone —
canonical, its content address checks, its signatures verify — names the
subject as one of the two participants, and its `finalized_at` falls inside
the window. Precisely:

- **"Two years" is 730 days**, measured back from the record's `started_at`.
  Not a calendar interval — calendar arithmetic differs across implementations
  and timezones for no benefit here.
- **The window is exclusive at both ends** [author, 2026-09-01]: a prior record
  counts if `started_at - 730d < its finalized_at < started_at` — **previously
  completed ceremonies only**, so a record finalising at this ceremony's own
  start instant is out, as is one finalising exactly 730 days earlier.
- **The lower bound saturates at zero.** Timestamps are unsigned; for a record
  less than 730 days after the epoch the subtraction would underflow.
- **A record the subject signed only as a witness does not qualify** — it names
  them in field 4, not field 3, and a witnessed ceremony's participants met
  each other, not the witness.
- **Formation records count**, both toward *n* and as candidates. They record
  real meetings; what they lack is corroboration, which is a weight question
  for policy (design §16.1), not a structural one.
- **The current counterparty is never a candidate for their own verification.**
  They are the party being established; asking them is not evidence.
- **Duplicates count once, by `txid`; candidates are distinct prior
  counterparties, deduplicated by keyhash.** The count counts transactions,
  the pool counts **identities**, and the two must not be conflated — and
  identities are the most the protocol can count [author, 2026-09-03]: one
  person may hold several (design §13.7), so slot uniqueness is keyhash
  uniqueness, and the record claims no human independence the protocol
  cannot prove.

### 5.4 The candidate pool is the curated bundle

**The bundle is the subject's to curate** [author, 2026-09-01]: any presence
records they choose, **from any of their series, with no intervening
transactions exposed and no chaining between them**. A bundle is a set of
individually verifiable records, not a stretch of archive — nobody walks
another party's archive (design §8.1.2), and nothing requires the handed
records to connect. A record that fails its checks contributes nothing; with
no completeness to protect, it is simply not in the pool.

**The bundle has no protocol ceiling** [2026-09-02]. It rides the ceremony
channel and is retained nowhere, so what a client will hold is local resource
policy — and truncation is a local act with a visible price, since it changes
*n* and the candidate pool.

**Understatement is free, and self-defeating rather than dangerous.** A subject
who hands fewer records gets a smaller *n*, a smaller sample, and a record that
advertises thinner corroboration. Overstatement is impossible, since every
record must verify. What protects the selector is **recognition, not
completeness** (design §8.1.2): a pool holding nobody they know is worth what
unrecognised history is worth — and a curated pool of strangers announces
itself to the one party it is aimed at.

### 5.5 What the record carries

**The record carries the responses gathered, and nothing defines which slots
"should" exist.** An evaluator weighs the response count against §5.2's
criterion — knowing *n* is the subject's claim — and weighs each responder by
their own recognition of them, which is the same act design §16.1 asks of every
evaluator everywhere.

**Structural rules that remain, all checkable from the record and its
queries**:

- **A response is structurally valid with respect to its query when its
  `query_id` matches one the subject countersigned and its `subject` names one
  of the two participants.** There is no selected-set membership to check —
  no set exists apart from the selector's judgment.
- **Duplicate responses from one verifier for one subject are malformed**, so a
  single verifier cannot occupy multiple slots. One identity answering once
  for each participant is two slots and valid.
- **Each response carries the selector's claim of its basis** — field 10,
  tier-aligned [author, 2026-09-03]: `0 met` (tier 1), `1 in-horizon`
  (tier 2), `2 reachable` (tier 3 or 4), `3 discretionary fill`. Met and
  merely-in-horizon are separately encoded because they are different
  security facts — sharing a position with somebody is not having met them
  (design §16.2.1), and a record folding the two would launder structural
  proximity into the look of acquaintance. The claim is the
  selector's, recorded because an evaluator reading
  the record cannot reconstruct the selector's acquaintance graph; like
  `nominated_by`, the schema records the claim and evaluation is the
  reader's.
- **Responses that are present count content-blind** — match, no-match,
  inconclusive, unavailable each fill a slot; content is evidence weighed by
  policy (design §16.1), never an input to structural validity.

**There is no `pending` on the wire.** A reachable verifier that cannot
evaluate answers `unavailable`, under its own signature; an unreachable one
answers nothing — nobody signs on an absent verifier's behalf. A reply that
misses the ceremony resolves **privately to the participants** and is never
retro-inserted: the record is immutable, and a responder who wants its late
answer durable has §7.4's `LateResponse`.

**`inconclusive` covers a failure to decrypt, and `no-match` never does.** A
truncated or unauthenticated sealed capture tells the verifier nothing about
the subject. **Reporting it as `no-match` would turn a corrupted store into
adverse evidence**, which is the one outcome a storage fault must not produce.
**An absent capture key is `unavailable`, not `inconclusive`** [2026-09-02,
design §7.5.2's precedence]: withholding is the subject's deliberate act and
is deliberately indistinguishable from a verifier with no capture at all —
`unavailable` is for evidence that never reached the comparison;
`inconclusive` is for a capture in hand that could not be read, and it carries
**basis 0 and the query's template version** — the attempted mechanism, not an
assertion that comparison ran [2026-09-02].

### 5.6 Consent is signed over the query id

`query_id = SHA-256(canonical CBOR of the VerificationQuery with field 6 absent)` —
the map of fields 1–5 and 7, hashed, then stored as field 6. Hashing a map that contains
the hash is unconstructible.

**The addressed verifier is inside the hash, deliberately** [author,
2026-09-03]: field 7 names the one verifier this query may reach, the consent
signs the id that names it, and a receiving verifier rejects a query not
addressed to it before any processing. One consent per verifier follows —
`query_id` differs per addressee — which is what makes the consent an
authorization instead of bearer paper. Witnesses must observe all
participants; verifiers are drawn from a wider pool and need not, which is
why the confinement sits here and not on the witness path.

**The template version is inside the hash, deliberately**. The
subject consents to a comparison, and *under which scheme* is part of what they are
consenting to: a version outside the id could be altered after signing, so the
subject would have countersigned a comparison and not the terms of it.

The subject's `COSE_Sign1` (field 7 of `VerifierResponse`) signs **`query_id`**,
not the query itself. The query, which carries a fuzzed profile up to 4 KB — is
therefore not carried in the record, while the signature remains verifiable from
it, because the id commits to the query's full contents including the profile.

Carrying the query itself would add up to 64 KB to a record with sixteen responses,
for no verification benefit.

**How consent reaches the verifier**: request type 4's body is the
array `[ VerificationQuery, COSE_Sign1, uint ]` — the query, the subject's
consent beside it, never inside it, and the selector's `selection_basis` claim.
Consent cannot be a query field: it signs
`query_id`, which hashes fields 1–5 and 7, so placing it in the map it authorises would be
the §5.6 circularity again one level up. The verifier checks the consent against
the query's own `query_id` before anything else; **a query arriving without consent
is rejected**, which is what field 7's rule already required and the wire could not
previously carry.

**How `selection_basis` reaches the verifier** [2026-09-02]: the third element,
a bare uint 0–3, the same closed enumeration field 10 carries and the same one
design §8.1 names. Response field 10 is the selector's claim inside the
verifier's signature, and no earlier element carried it — the specified
response was unconstructible from the specified request. It travels outside
every signature because the transport already authenticates the requester, who
is the selector making the claim; the verifier echoes it into field 10 and
signs the echo. The recovery form is untouched: there the verifier is its own
querier (§4.1) and no type-4 request travels.

**Field 2 MUST name the authenticated requester** [2026-09-02]: on a type-4
stream the querier is the transport-authenticated peer, and a mismatch is
rejected — otherwise field 2 chooses its own rate-limit bucket.

**The successful reply body is a single `VerifierResponse`** [2026-09-02],
framed as every reply is (§9.2). A malformed query — consent absent or
failing, recomputed `query_id` mismatching, field 7 naming a different
verifier — is answered by **closing the
stream**: no error schema exists, and no signed response is fabricated for
input that is not evidence.

**A response about a subject is also delivered to that subject** [author,
2026-09-03]: the verifier sends a copy of its signed `VerifierResponse` to the
subject over the association the capture-key grant already establishes
(§7.3, design §7.5.2). The subject consented to the query, so the copy
disclosed nothing new — and it is what makes the finalization veto real: a
subject holding a response withholds its envelope signature from a proposed
body that omits it (design §7.4.2). Suppression then requires both
participants, and buys a visibly thin record (§5.2).

**A claimed `met` the verifier's own records refute is answered `unavailable`**
[author, 2026-09-03]: `met` names the selector's relationship to this
verifier, which the verifier can check locally — a false claim voids the
selection premise, and a false `met` is the same as not available. No other
basis value is verifier-checkable, and none is endorsed by answering: see
field 10's carriage rule below.

**Field 10 in the signed payload is carriage, not endorsement** [author,
2026-09-03]: the verifier's signature binds the selector's claim against later
alteration — the `nominated_by` pattern — and policy MUST NOT weight the claim
more heavily for sitting inside a verifier signature. Evaluation of the claim
is the reader's, by their own recognition of the selector.

### 5.7 Who can verify what, the property is holder-relative

**Validation is per-subject, and an evaluator can weigh only what it can
recognise.** A record's two halves are independent: an evaluator who knows
some of subject A's responders and none of B's has learned something about A
and nothing about B, and reports the two separately rather than collapsing
them (§4.1's valid-versus-unverifiable discipline).

**A stranger holding only the record can verify signatures, structure, consent
and the bindings of §5.5 — and can weigh nothing**, because weight comes from
recognising responders (design §16.1), which no distant audience can do. That
is not a shortfall; it is the design's statement of who presence evidence is
*for*: the people connected enough to recognise the people in it.

## 6. Resource registration, the catalog, and abuse reports

### 6.1 The catalog entry

**A registration is the entry itself, signed.** There is no envelope and no
transaction around it: field 8's signature covers fields 1–7 and 9, and those are
the same bytes a query reply returns. **One object, one signature, wherever it
appears** — which is what lets a host store what it was handed and serve it
unchanged.

```
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
                     ;   request time regardless (§11)
  7: ? bstr .size (1..1024),  ; additional service metadata, the TXT analogue.
                     ;   SHOULD NOT duplicate field 5 — an uninterpreted byte
                     ;   string cannot be checked for semantic duplication, so
                     ;   this is guidance to a publisher and not a decoder rule
  9: ? uint,         ; data_practice — the owner's DECLARED logging and retention
                     ;   posture (design §11.5). Small enumeration, below.
                     ;   OPTIONAL: absent means undeclared, which is itself
                     ;   informative and is NOT equivalent to any declared value
  8: COSE_Sign1      ; by the OWNER over fields 1-7 AND 9 of THIS map only.
                     ;   Classical component alone — an entry's relevance ends
                     ;   when the owner stops returning it (§7.1). It does NOT
                     ;   cover the enclosing transaction's key 0: the same entry
                     ;   is returned in query replies where no transaction exists
                     ;   around it, and a signature covering the body would be
                     ;   unverifiable there
}
```

**`data_practice` values**. Deliberately few and structural rather
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

**Four values, deliberately coarse.** A resource may be
anything from a shared drive to a social network to a persistent agent, so its security
posture is particular to it in a way no enumeration can track. **A granular or
exhaustive taxonomy would be counterproductive** — it would multiply values nobody
maps consistently and give a reader false confidence that the categories mean the same
thing across two resources. Four coarse bands a publisher can be held to socially are
worth more than twenty nobody applies the same way.

**Nothing checks it.** The protocol has no view of what a resource logs (design
§11.0.3) and cannot acquire one. The field makes an owner's claim signed and portable so
a policy can weight it, which is design §1.1's move where enforcement is unavailable —
the same as client-integrity attributes. **A false declaration is undetectable** and is
a matter between the owner and whoever relied on it.

### 6.2 Registering an entry

**Submitting one: bidirectional request type 7.** A light client cannot serve
its own catalog (design §11.5), so it hands the signed transaction to the node
hosting it. Without a message for that, the single act which makes a resource
discoverable would be the one thing an attached client cannot say to the node it is
already attached to, and every implementation would invent its own.

```
ResourceRegistration = {
  1: CatalogEntry,   ; the entry, signed by its owner (field 8 above)
  2: ? Scope,        ; the REQUESTED discover_scope. A request and not an
                     ;   instruction: the host composes the answer and may
                     ;   narrow or ignore this, and the owner cannot check
                     ;   (design §11.5). Absent leaves the host's existing rule
                     ;   for this resource in place — and on a FIRST
                     ;   registration, with no existing rule, the default is
                     ;   `self`, the least-disclosing scope [2026-09-02]:
                     ;   broader visibility is the owner's to request
  3: bstr .size 16   ; nonce, echoed in the reply
}

ResourceRegistrationReply = {
  1: bstr .size 16,  ; echoes the request nonce
  2: uint            ; 0 recorded | 1 refused
}
```

**Failure signalling** [2026-09-02]: malformed framing or non-canonical CBOR
resets the stream, §9.2's generic rule; a well-framed registration failing the
owner-session binding, the entry's signature, or the one-owner-per-keyhash
check is answered `refused` — the reply exists to carry exactly that answer.

**A registration is complete in itself** [2026-09-02]: nothing structural
requires a local backend — a brokered resource's endpoint belongs to the
external service, and any admission requirement beyond the checks here is host
policy. A host refusing every entry that lacks a locally installed package
refuses valid brokered registrations.

**The authenticated peer MUST be the owner named in the entry.** An entry is
owner-signed and so may be relayed by anyone, which means a host accepting one from
any peer would also accept a **replayed earlier envelope** — and since the node keeps
whichever it applied last, a stale registration would silently supersede the current
one. Binding submission to the owner's own session is the check that costs nothing:
the owner is attached already and the host has its identity from the handshake.

**Refusal names no reason.** A host declines for capacity, for policy, or for
reasons of its own, and saying which would describe the host's state to an owner who
can do nothing differently with it. Ask again, or ask elsewhere.

### 6.3 Abuse reports

```
AbuseReport = {
  1: keyhash,        ; resource — and the signer. A resource reports; its owner
                     ;   receives (design §11.6)
  2: timestamp,
  3: uint,           ; 0 unavailable | 1 malfunction | 2 excessive-load
                     ; 3 unauthorised-access-attempt | 4 content | 5 other
  4: ? bstr .size (1..1024),   ; detail, resource-defined and uninterpreted by
                     ; the network. **Bounded, and deliberately small.** The
                     ; report goes to the resource's own owner, who already holds
                     ; the context, so the bound is not about what the recipient
                     ; learns — it is that a signed object is **portable** and the
                     ; owner may hand it to anyone (design §11.6, P27). A
                     ; resource needing more should reference its own record
                     ; rather than inline it.
                     ; An application wanting to name which of ITS users
                     ; complained puts that in here, as its own schema. It is
                     ; application data: the network has no user-signed report,
                     ; because that would require a user's network client to
                     ; interoperate with arbitrary third-party applications
  5: COSE_Sign1      ; BY THE RESOURCE named in field 1. The signing key's
                     ;   keyhash MUST equal field 1 — otherwise the object
                     ;   attributes a complaint to a party that did not make one
}
```

**An abuse report has no carriage, and needs none.** It is created and consumed
at the owner's node (design §11.6) — signed because it is portable and durable, not
because it traverses the network. **A registration is the opposite case** and that is
why it has a request tag: an entry must cross from the owner who signs it to the host
that answers for it.

### 6.4 The query and its answer

```
CatalogQuery = {
  1: ? tstr .size (1..64),  ; service type filter, matched byte-for-byte
                       ;   against §6.1's field 3. Absent means everything the
                       ;   asker may see. Same bound as the field it matches —
                       ;   a filter longer than any legal type cannot match and
                       ;   should not be allocated for
  2: bstr .size 16     ; nonce, echoed in the reply
}

CatalogReply = {
  1: bstr .size 16,    ; echoes the query nonce
  2: [ * CatalogEntry ],  ; bounded at 111; an answering node with more than
                       ;   111 visible entries for one asker returns 111 and
                       ;   sets field 3
  3: ? tstr .size (1..64)  ; TRUNCATION CONTINUATION: present iff entries were
                       ;   omitted, carrying a service type to ask for next.
                       ;   Lets an asker drain the catalog without knowing the
                       ;   types in advance. A hint, not a cursor: the node keeps
                       ;   no state
}
```

**Truncation must make progress, so selection is not free.** An answering node
**orders qualifying entries by resource keyhash, then by owner keyhash, and returns
the first 111**, and the continuation names the type of the first entry it withheld.
Without an order, two queries could return the same 111 and the same hint forever.
The order is arbitrary and that is fine; it only has to be *total*. **The owner key
is defensive**: on any one host, resource keyhash is already total, because a host
refuses a second claim on a keyhash it serves (below) — the composite key costs
nothing and keeps the order total even if that rule is ever relaxed.

**A filtered query is answered from the same order**, so a type filter narrows the
qualifying set and the asker makes progress within it.

**The bound sits above the population, so truncation is the exception and not the
mechanism.** A single answering node answers for at most **itself plus the ≤110
users it serves** (design §11.5, design §12.6.1), so a reply that truncates comes from a
node holding more entries of one service type than it has owners to own them. The
bound is per *answering node* and not per horizon — a trust horizon is larger (design
§15.1), and no single node answers for all of it. **The continuation is a hint for an
unusual case, not the normal path through a catalog.** It is also near its own
ceiling: at 2 KB an entry, 128 maximum-sized entries no longer fit one 256 KB frame
(§9), so 127 is the highest this field can go without moving the frame bound.

**Where a node does exceed the bound, the asker can detect it and MUST NOT loop.**
The continuation names a *type*, not a position, so an asker that has already
filtered on that type and received a full page will receive the same page again.
**That is the signal: a full page plus a continuation naming a type already
exhausted means the node holds more than the bound**, and the asker's view of that
node is truncated. Treating it as truncated is right; following the hint again is a
loop. Nothing on the wire reports the condition, and nothing needs to — the asker
holds both facts already.

**Carried on a bidirectional stream** (§9.2) tagged request type 5, a query per
stream, with the reply closing it.

**Inclusion in a truncated set is not a ranking.** The order below is by resource
keyhash — arbitrary, chosen because it is stable rather than because it is
meaningful — so an asker must not read the first 111 as the most relevant 111.

**Preserved unknown keys are bounded like anything else.** §1 requires unknown
map keys to survive re-serialisation, which makes them attacker-supplied storage on
a signed object a node retains — 16 per map and 1 KB per value. **Extension
tolerance is not unbounded tolerance**, and a rule that admits arbitrary bytes into
retained state is a denial-of-service surface however well-intentioned.

**Unsigned session messages reject unknown map keys** (§1's global rule) —
`CatalogQuery`, `CatalogReply`, `ResourceRequest`, `ResourceResponse` and the
control frames of §8.0 alike, and equally every unsigned family this chapter does
not name: resolution, prekey and archive requests, grants and wrappers.

**An asker outside the answering node's horizon is refused before the application
reply.** Close the stream; do not return an empty `CatalogReply`. **An empty
reply is a true statement — nothing is visible to you — and it is the wrong one**,
because it is indistinguishable from a node that hosts nothing, and it invites an
asker to conclude the catalog was answered.

### 6.5 An entry's lifecycle

**An entry is signed once, at registration, and that signature is reused for every
answer.** No field varies per query, so re-signing buys nothing — and a
per-answer signature would make an entry's bytes differ between askers, which
defeats the attributability that signing is for.

**A catalog entry is a query answer, not a propagated record.** A
node asks an infra node within its horizon what it has; the infra node replies with
the entries it **owns** and that the asker may see. **Nothing floods, nothing is
cached authoritatively, and nothing needs invalidating.**

**Concurrent re-registration is settled by the answering node, not by ordering.**
Two registrations for one resource keyhash have no authoritative total order —
nothing timestamps them. **The node holding the entry keeps whichever it applied
last**, and since nobody else holds a copy there is nothing to reconcile; an owner
who cares which won can query and see.

**Re-registering a resource replaces the current entry.** An owner holds **one
current entry per resource**; a new registration for the same resource keyhash
supersedes the previous one locally. **A registration is not archived**: design §10's archive advances on adoption, departure, disavowal, peering
and presence, and a resource registration is none of those. The owner keeps a live
table of what it currently offers and answers from that; **nobody is required to
keep a record of a resource they no longer run.** Nothing on the wire needs to
express the replacement, because nobody else holds a copy to reconcile.

**So there is no propagation lifecycle to specify, and no `seqno` or withdraw
operation.** Those would exist to handle supersession, stale copies and competing
registrations, and **none of those conditions arise**: the node holding the entry
stops returning it, and the next query gets the truth.

**An owner who is not the answering node still has to ask, and needs no new
message.** It re-registers the resource with a requested `discover_scope` of
self, which no asker but the owner satisfies, so the entry stops being returned to
anyone else. What
remains is the host's own state to keep or drop, and no asker can distinguish an
entry withdrawn this way from one that was never registered. **Nothing preserves it
elsewhere**: a superseded registration is not archived, so a withdrawn resource
leaves no durable record to be presented later.

**Freshness is inherent rather than maintained.** Each answer is computed when
asked, by the party that knows.

### 6.6 The scope fields

**`discover_scope` is no part of the entry.** It decides which entries an answering
node returns to which asker — a filtering rule evaluated where the answer is
composed, never read by the recipient, since receiving an entry is what qualifying
looks like. Like the role table (design §11.4), it is local state, and carrying it *in
an entry* would be telling the asker how they were selected. **It travels in one
direction only**: an owner requests one at registration, above.

**`connect_scope` remains and is advisory.** It lets a client show whether a
connection is likely to succeed rather than presenting every entry identically. The
owner decides at request time regardless (§11), so a client that ignores the field
is wrong about presentation and never about access.

**A requester may cache what it was told**, and holds an answer that was true when
given. That is ordinary staleness with no protocol consequence: the next query
corrects it, and nothing grants access on the strength of a cached entry — access is
decided by the owner at request time (§11).

### 6.7 Two owners, one resource keyhash

**Nothing prevents two owners registering the same resource keyhash**, and nothing
needs to. An entry is a claim by its owner, verifiable as theirs; a reader
holding two such claims holds two claims, and **the resource keyhash is not a
namespace anyone allocates.** Which one a reader acts on follows from whose catalog
answered — and each answer came from a node the reader chose to ask.

**One host may not serve two, and that is where the rule bites.** A
`ResourceRequest` names the resource and nothing else (§11), so a node holding two
claims for one keyhash has nothing to choose between them with — and choosing wrongly
applies one owner's membership and roles to the other owner's backend. **A host
refuses a registration for a keyhash it already serves under a different owner**,
which is a check it can make and the requester cannot. Two claims may still exist on
two hosts, which is the case the paragraph above is about: the reader's choice of
whom to ask is what separates them.

**The owner alone signs**, so a cached or forwarded entry remains attributable. The
resource proves nothing and supplies no key material: the owner is asserting the
resource's identity, which is the only assertion a catalog carries.

### 6.8 A scope the evaluator cannot compute

**A scope the evaluator cannot compute is structurally valid and ineffective.** A decoder MUST NOT reject it.

**Rejection is wrong because validity is not local.** The same entry is valid for a
node that can compute the position and not for one that cannot — so rejection would
make structural validity depend on the reader's topology, and a node would reject an
object its neighbour accepts. **Store it, forward it if the forwarding rule says to,
and grant nothing from it.**

**No scope reaches outside the owner's trust horizon**, `list` included (design §11.4).
A scope naming a position outside it is not an error; it simply matches nobody the
evaluator can see.

---

## 7. Attestations and records: currency, anchors, key grants, acknowledgements, endpoints, resolution, prekeys, archive fetch

### 7.1 Currency attestation

```
CurrencyAttestation = {
  1: keyhash,          ; subject identity
  2: keyhash,          ; current key
  3: timestamp,        ; issued_at
  4: timestamp,        ; expires_at — ~10 h default; hours, not days (design §12.6.5)
  5: uint,             ; issuer role: 0 patron, 1 sibling (secondhand),
                       ;   2 grandpatron, 3 down-line threshold (root):
                       ;   optional, an anchor-caching input (design §12.7.2)
  6: keyhash,          ; ISSUER identity, the signature below is by this party,
                       ; who is not otherwise named
  7: COSE_Sign1        ; BY THE ISSUER over canonical CBOR of fields 1-6;
                       ; external_aad = "rhtn/1:currency". Classical only: an
                       ; attestation's relevance expires with it (~10 h), well
                       ; inside design §5.1's post-quantum horizon.
}
```

Signed by the issuer.

**Field 5 exists because issuance escalates** during patron outage, and a
consumer must be able to weight a secondhand attestation lower. Note the rule it
encodes: attestations are **issued fresh, never extended stale.** There is no
"extend" operation and no field for one, deliberately.

**Stapling**: an introduction carries the attestation inline, so a
recipient verifies locally rather than querying. Genesis identities omit it
entirely — currency is vacuous with no history.

**The fallback query, for a staple that is absent or expired**.
Bidirectional request type 8, addressed using the anchor and path the introduction
already carries (design §9.0.2).

```
CurrencyRequest = {
  1: keyhash,          ; the SUBJECT asked about. The QUERIER is not named:
                       ;   it is the authenticated session peer, so a reply
                       ;   forwarded onward attributes the question to nobody
  2: bstr .size 16     ; nonce, echoed in the reply
}

CurrencyReply = {
  1: bstr .size 16,    ; echoes the nonce
  2: uint,             ; 0 attestation follows | 1 cannot issue
  3: ? CurrencyAttestation   ; present iff field 2 is 0
}
```

**An attestation asserts only that this identity is current in the issuer's
subnet.** It never says a key was rotated, because **no party outside the
horizon is ever told that** (design §9.0): a rotation evaporates the old identity
locally and instantiates a new one, and the inheritance is not exposed and could not
be proved even upstream. To a distant caller a superseded locator is simply an
address that stopped working, **with no explanation offered**.

**So the answers are: an attestation, or nothing.** Code 1 says the responder
cannot issue rather than inventing one, silence says the responder was not reached,
and a caller receiving neither **concludes nothing** — silence is not attestation,
which is design §9.0.2's requirement that the two be distinguishable. No operation
waits on the answer (design §12.6.5): the staple decides which key a caller
addresses, never what it may do.
**What a caller can learn is a fork**: two patrons attesting competing claims is
visible-but-unresolved, and that is the point (design §9.0.2).

**Nothing is retained on either side.** The request is liveness class (design
§15): answer it and discard it. **It is not archived** — design §10's archive is
topology and presence — and a responder that logged these would hold a record of who
was being introduced to whom, which is the exposure stapling exists to prevent.
Rate-limit per requester as with any other query.

### 7.2 Anchor table entry

**An entry may only name a contactable infrastructure node.** Any ancestor may
be *named* as an anchor in a locator (design §12.2), but an anchor **table** entry
requires routable endpoints, so a locator naming a light-client anchor cannot enter
this protocol — its holder must present one naming a reachable ancestor instead.

**An unpinned anchor is never authenticated, and this is not trust-on-first-use.**
The requester has a keyhash from the table and no key material —
`AnchorEntry` carries neither — and **a keyhash cannot be checked against what the
handshake presents.** An identity is SHA-256 of the *pair* (§1), while RFC 7250
carries one SubjectPublicKeyInfo, so the classical component alone cannot reconstruct
the hash. This is the same impossibility §8.2 states for a sibling arriving without
`KeyMaterial`. **Nor does the reply supply it**: `Referral.key_material` names the
next hop and `ServingInfra.key_material` the serving node, never the responder
itself, so an anchor that refers you onward is never pinned at all.

**It does not need to be, because a referrer's identity is not what protects you.**
design §12.6.1: a referral cannot be falsified for impersonation, since the requester
authenticates the *subject* it intended to reach and a wrong address produces a
handshake failure rather than a silent misdirection. A hostile chain costs a failed
dial — denial, not misdirection, and design §18.4 prices the selective form. **Disclose nothing beyond the query itself** to a party you cannot
authenticate.

> **Why this differs from the sibling rule, which an implementer will notice.** §8.2
> makes a sibling with no key material **UNUSABLE** rather than dialling it
> unauthenticated. The cases are genuinely different: a sibling is a **destination**,
> and its identity is the whole point of contacting it; an anchor returning a referral
> is a **referrer**, whose identity is incidental to an answer validated by reaching
> someone else. Do not "fix" the anchor case to match the sibling one.

**Anchor entries are self-signed, and the signature is verifiable only by a party
that already holds the anchor's key** — from having attached to it, or from a
transaction naming it. An entry carries the *keyhash*, not the key, so **nothing in
the entry lets a recipient check the signature on receipt, and resolution never
supplies what would.**

**So the signature gives retroactive attribution to a holder who has the key by other
means, and nothing to anyone else.** Where it can be checked, a forged entry is
**attributable** rather than **prevented** — the gossip source that supplied it is
identifiable. Where it cannot, a forged entry costs one failed dial.

**The ingestion boundary must therefore be explicit.** An implementation that
treats "present in the table" as "verified" while its ingestion path does not
enforce that has a partition vulnerability with no visible symptom. State which it
is: either entries are verified on acceptance — possible only where the key is
already pinned — or the table holds unverified gossip and verification happens on
contact.

Freshness is by `seqno`, strictly greater `counter` to replace **within a series**; entries in different series do not rank (§2.3).

```
AnchorEntry = {
  1: keyhash,          ; 32
  2: [ 1*8 NetworkPoint ],
                       ; §1's ceiling, stated here too — the earlier `+` left
                       ;   this schema unbounded against the table. Publisher's
                       ;   preference order, as everywhere endpoints are listed
  3: uint,             ; subtree size. U64 RANGE, the design's ~4-byte sizing
                       ; arithmetic is a storage estimate, not a validity bound
  4: seqno,
  5: COSE_Sign1        ; BY THE ANCHOR over canonical CBOR of fields 1-4;
                       ; external_aad = "rhtn/1:anchor". Classical only
}
```

**Key hashes, not keys** (design §5). Full PQ keys would blow the table by ~18×.
The table is an index, not a credential store. **Full keys are not fetched during
resolution** — they arrive from attaching, from a transaction naming the node, or not
at all (above).

Which anchors a node caches is **per-node policy, never a protocol constant**.

### 7.3 Capture key grant

**The message by which a subject releases a capture key to a holder**
(design §7.5.2). It exists because nothing else in this profile carries one, and
three implementations would otherwise invent three.

```
KeyGrant = {
  1: txid,             ; the presence record whose capture is being unsealed —
                       ;   the holder may hold several for this subject
  2: bstr .size 32,    ; query_id this grant answers, binding it to one query.
                       ;   the full SHA-256 value from VerificationQuery field 6
  3: bstr .size 32     ; k_capture — the one key sealing this record's capture
}
```

**Carried as payload, never as a record.** It travels over the end-to-end
encrypted path (design §14.2.4), which authenticates the sender to the recipient
and hides it from the transport. **It is never retained in the presence record and
never propagates**: a grant is a momentary release, and an object that persisted
would defeat the retention property the whole scheme exists for.

**Field 1 disambiguates which capture.** A verifier who has met the subject several
times holds several sealed captures; **the grant names the one to open** — ordinarily the latest finalized eligible
meeting in the committed history (§4.5), though which eligible capture to grant
against is the subject's choice (design §7.5.2). Without field 1 the holder
would guess.

**Field 2 binds the grant to a query.** A grant arriving unattached to a query the
subject countersigned is an unsolicited key release, and a holder should treat it as
malformed rather than as an invitation to open its store. **Arrival order is not
guaranteed**: the grant and the query travel different paths, so a
holder may buffer an unopened grant briefly awaiting its query — bounded and brief,
since an indefinite buffer defeats the momentary-release property. The bound is
local policy. **The authenticated sender MUST be the subject** [2026-09-02]:
the seed never leaves the subject, so a grant arriving from anyone else is
replay or fabrication, and the holder rejects it. **Duplicates are ignored, and
so is a differing second grant for a query already satisfied** — the first
authenticated grant stands: one release, one answer. **A grant naming a record
the holder does not hold yields `unavailable`** [2026-09-02]: there is nothing
to compare, and nothing about the subject is thereby evidenced — the same
posture as every other absence.

### 7.4 Late verifier response

**A verifier response that arrives after finalization is carried as a standalone
signed object referring to the record it supplements.** It is not an
amendment — the record it names is immutable.

```
LateResponse = {
  1: txid,                 ; the presence record supplemented
  2: keyhash,              ; subject — one of that record's two participants
  3: VerifierResponse      ; the response itself, signed by the verifier as in §4.5
}
```

**It does not amend the record it names.** A presence record is immutable and its
`txid` fixes its content; this object sits beside it. **The record is unaffected**
— it finalised with the responses that had arrived, its unanswered slots absent
and visible (§5.5, §3.2). A late response is additional evidence an evaluator may
weigh, never a change to what was decided — and never retro-inserted.

**The response must carry a `query_id` the subject countersigned for that
ceremony** — consent is the gate, there being no selected set to test against
(§5.5). Otherwise anyone could attach unsolicited assertions to a record naming
someone else.

**Retention follows the record it supplements.** A `LateResponse`
is evidence about one presence record; a holder keeps it while it keeps that record
and discards it with them. **Saying no rule applies left it outliving the thing it
describes**, which is how an attestation becomes an orphaned fact about a person.
Nothing obliges anyone to serve it.

### 7.5 Subtree acknowledgement

**A grandpatron's countersignature over an adoption, admitting the new node to the
resources it hosts** (design §11.2.1). Carried separately rather than as a third
envelope signer, because the adoption must not wait on a party who may be offline.

**Issued by the grandpatron's node, not by its operator.** It says
*I am aware of this member and have added them to my tables*, and a node emits it
under standing policy without interrupting anyone (design Appendix A). Nothing in it requires
a human at the moment it is written; the deliberate act it acknowledges was the
patron's.

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
                       ; describes, well inside design §5.1's horizon
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
individuals (design §11.2.1).

**Discard it when it lapses**, rather than holding a record of a relationship that
has ended. The condition is visible in topology, so a holder can tell — and a
retained lapsed acknowledgement is a durable statement that two parties were once
connected, which is exactly what departure is supposed to end.

**It lapses when the acknowledged relationship ends.** If the grandpatron disavows
the patron, or the patron departs, the acknowledgement describes a subtree the node
is no longer in and confers nothing. **No revocation object is needed**: the
condition it depends on is already visible in topology.

### 7.6 Node endpoint record

**How an infra node's address reaches the parties that must refer to it.** Modelled on `AnchorEntry` (§7.2), which is the anchor-table
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
(design §14.1.2) and it holds no static address; an infra node serves itself and
never attaches, so nothing otherwise carries its address to its patron — and without
it **the patron cannot refer** (design §12.6.1).

**Carried in the topology class**, by §10.1's forwarding rule, so it reaches the
node's horizon and its patron with it. **It does not travel rootward** (§10.2).

**Self-signed, and the signature's value is the same one §7.2 states**: a recipient
holding no key material cannot check it on receipt, so it gives **retroactive
attribution rather than prior authentication** — a node that reaches the address and
obtains the key can confirm the record was genuine, and if it was not, knows **which
neighbour handed it over**. That is the immediate authenticated hop and not the
origin: nothing on the wire carries a path, and a relay is indistinguishable from a
publisher. It is enough, because the neighbour is the party you can stop listening
to. That is worth having *here* and not in a referral: a
flooded object passes through parties the recipient did not choose, where a
`Referral` (§7.7.3) comes from the single party the requester is already talking to.
This is why §7.7.3's replies are unsigned and this record is not.

**Freshness by `seqno`, strictly greater `counter` to replace within a series**, under §2.3's rule; across series they do not rank.
**Publishing a changed endpoint list advances the counter** — and the list is
ordered, so reordering alone is a change [2026-09-02]: field 2 carries
preference, and any field-2 difference at an equal `seqno` would be exactly the
equal-`seqno` disagreement the advance exists to prevent. That is what makes the
new record replace the old rather than collide with it. **Republishing an unchanged list replays the record already held**
rather than consuming a number: reconciliation is a replay of the same frames
(§10.1), and a fresh number over identical contents is freshness churn with nothing
behind it.

**One record per patron relationship** [2026-09-02]. The `seqno` is the
relationship line's (§2.3: one series per patron relationship, two patrons two
series — design §19.4, P36), so a node bound under two patrons publishes one
record per line, each carrying that line's own current `seqno`. A single
record could not serve: its series is unprovable in the other relationship's
subnet, so it would never enter storage there — and a shared counter across
subnets would disclose exactly the cross-subnet activity P36 exists to
conceal. The lists may agree; nothing requires it, and each record is that
line's own address claim — **partitioning records and locators between
subtrees is the anticipated use** [author, 2026-09-02], not tolerated slack.
A holder proves each record against its own series
chain, and holding one per proved series is the correct end state, not a
conflict.

**The list is in the publisher's preference order, and its entries are distinct.**
A repeated `NetworkPoint` is malformed. The record is signed, so a decoder
accepting a repetition another rejects splits the network on bytes rather than on
meaning — and a repetition expresses nothing the order does not already say.

**A peering record already carries this for peered nodes** (§4.4, `NetworkPoint` for
both endpoints). The gap this record closes is the infra node that **neither peers
nor serves as an anchor** — a supported, degraded state (design §6.3, design §12.7.5), and
until now one with no carrier for its address at all.

**Volume.** Only infra nodes publish, and design §3.3 sets the infra threshold at 110
subordinates, so an `h=2` ball of ~110 nodes contains on the order of one. The
constant-state floor of design §12.6.1 is untouched.

### 7.7 Resolution

Encodes design §12.3's cases and design §12.6.1's self-routing.

#### 7.7.1 Who sends a resolution request

**A light client sends `ResolveRequest` to its serving infra node, not to the
anchor.** Control traffic is always client-to-serving-node (design §14.1.1); a
light client has no reason to hold a socket to an arbitrary anchor and often could
not reach one. The serving node resolves on the client's behalf and returns the
result.

**Infra nodes exchange resolution requests directly**, static address to static
address.

The request is anchor-*relative* — the path in it is interpreted from the anchor
named in the locator, which is a statement about how the path is read, not about
who the request is addressed to.

#### 7.7.2 Descent is through infrastructure only

**A path is not walked node by node.** Intermediate nodes may be light clients,
which are neither always online nor independently reachable (design §12.6.1).
Resolution therefore descends **only through infra nodes**, and terminates at the
**serving infra node** for the target, the nearest infrastructure ancestor, which
is the node the target attaches to (design §14.1.2).

**The remaining path suffix is returned rather than traversed.** The serving node
uses it to identify which of its attached clients is meant. This is what lets a
path address a light client that nothing can route to directly.

#### 7.7.3 Messages

```
ResolveRequest = {
  1: keyhash,           ; subject being resolved
  2: keyhash,           ; ANCHOR the path is relative to — without it the
                        ; anchor-relative path is uninterpretable
  3: path,              ; anchor-relative, from the locator
  4: bstr .size 16      ; nonce, chosen by the requester. Cryptographically
                        ; random, fresh per logical resolution, and REUSED
                        ; across endpoint retries for that same resolution —
                        ; a fresh nonce per endpoint would let a reply for one
                        ; attempt be accepted as an answer to another
}

ResolveReply = {
  1: bstr .size 16,     ; echoes the request nonce
  2: uint,              ; 0 serving | 1 failure | 2 referral
  3: ? ServingInfra,    ; present iff field 2 = 0
  4: ? uint,            ; present iff field 2 = 1; see codes below
  5: ? Referral         ; present iff field 2 = 2
}

Referral = {
  1: keyhash,                ; the next hop to query
  2: [ 1*8 NetworkPoint ],   ; its endpoints, in the publisher's preference
                             ;   order — the same semantic as `ServingInfra`
                             ;   and §7.6; selection and retry are local policy
  3: uint,                   ; path indices this referral advances past, counted
                             ;   INCREMENTALLY from the referring node's own
                             ;   position, not as an offset from the anchor.
                             ;   MUST be ≥ 1: a referral that advances nothing
                             ;   is a loop, and a node with nothing to add
                             ;   reports failure instead
  4: ? KeyMaterial           ; so a requester with nothing pinned CAN authenticate
                             ;   the next hop (§9.1) — an option, not a
                             ;   requirement [2026-09-02]: a hop without it is
                             ;   still dialled, under the disclose-nothing rule
                             ;   below, because a referrer's identity is not what
                             ;   protects the requester. Only a terminal
                             ;   `ServingInfra` needs authenticatable material to
                             ;   proceed past resolution. Same field, same
                             ;   reason, as `ServingInfra` carries
}

ServingInfra = {
  1: keyhash,                 ; the serving infra node's own identity
  4: ? KeyMaterial,           ; full key material, so a requester with no pinned
                              ; entry can verify the keyhash and then authenticate
                              ; the TLS peer (§9.1). Without this a first contact
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

**Resolution is iterative with referrals** (design §12.6.1). A node answers
authoritatively, **refers the requester onward**, or reports failure. It never carries the request itself, so no progress or
consumed-prefix state exists: each request carries its own anchor and full path, and
every reply is interpretable without knowing what came before.

**Each node interprets the path from its own position.** A request carries the
anchor and the **full, unmodified path** every time; it holds no consumed-prefix
field, and none is added. A node knows its own anchor-relative position, so it knows
which portion of the path is still ahead of it. **Arrival is announced by the reply, not computed by the requester.** A `ServingInfra` answer says the resolution is done; no running total
of `advances` needs to be kept, and **no arrival-consistency equation is checked**
— such an equation would reject the direct-serving answer design §12.6.1 permits,
for no gain, since no node depends on any requester-side total. What remains
checkable per referral: a referrer moves the requester strictly forward along
the path and never claims progress past its end — `advances` MUST be ≥ 1, and a
referral advancing past the path's end is malformed.

**Why not carry the consumed prefix:** a field the requester computes and every node
must trust is state an intermediary could misreport. Deriving position from a node's
own place in the tree removes the question.

**A referral names the next hop and gives its endpoints**, so the requester
continues from there. A node may refer past several indices at once where it knows
its own subtree.

**Nothing polices referral content, because impersonation is self-detecting.** The requester
authenticates each endpoint against the keyhash it expects (§9.1), so a wrong
address produces a handshake failure rather than a silent misdirection — and an
intermediary misreporting progress buys only the same failed dial. What a false
answer retains is denial (design §12.6.1, design §18.4).

**The reply is not signed.** It conveys where to try next, and the requester
authenticates the endpoint it reaches by ordinary means at contact time (design
§12.2, the anchor table is an index, not a credential store). A wrong or hostile
reply causes a failed connection, not a false identity.

**Failure codes** for field 4, with the disposition each implies — stated
because "distinguishes retry from re-resolve" without a mapping lets two resolvers
disagree about the same reply:

| Code | Meaning | Disposition |
|---|---|---|
| 0 | No such child at some index | **Re-resolve.** The locator is wrong or stale; another endpoint will say the same |
| 1 | This node is not authoritative and cannot refer | **Re-resolve.** Distinct from a referral (field 2 = 2), which *can* point onward — this code means the node knows of no next hop, so the locator is stale or names a subtree it has no relationship with |
| 2 | Temporarily unavailable | **Retry**, here or at another endpoint for the same node |
| 3 | Refused by policy | **Terminal** for this requester. Retrying elsewhere may succeed, but not by repetition |

**A locator whose anchor is absent from the local table is not resolvable by this
node.** That is a caller-side condition, not a wire failure: no request is sent,
because there is nowhere to send it. The caller needs an address for that anchor
from some other source — an introduction, a cached entry, a peer's referral.

**A stale locator is not repaired in transit; it fails.** A
resolution against a position the subject has left returns failure code 0 or 1, and
the requester re-resolves from a higher anchor (design §12.3) or re-establishes
socially (design §12.4). **No party redirects a resolution to a different position on
the subject's behalf** — that is the redirection mechanism this rule existed to
prevent, and removing forwarding removed the thing that needed the guard.

**`seqno` comparison still governs locator freshness** wherever two locators for one
subject are compared, **within a series**: strictly greater `counter` to replace, and
equal `seqno` with different contents malformed rather than a tie to break, since a
subject advances its own counter (§2.3). The aftermath is §10.1's [2026-09-02]:
malformed names the pair, the holder retains neither as current and
re-resolves — kept-because-it-arrived-first is not a freshness rule. **Two locators in different series do not
rank**, and a reader holding both has learned nothing about which is current — it
either holds a §4.6 chain that says, or it re-resolves (design §12.3). **A series
reissue does not disturb routing**: the path is unchanged, so a cached locator still
reaches the subject, and the comparison only matters once the position moves as
well — at which point the cached path is stale on its own account.

### 7.8 Prekey distribution

**The network distributes prekeys; it does not define them.** design §14.2.4 adopts
PQXDH, whose bundle contents are specified by that protocol. This section carries
them.

**The bundle is opaque to this protocol**. A node that serves
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
                       ; §7.1's horizon does not apply
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
reusable material for its whole trust horizon (design §14.2.4) consumes nothing
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

**Speculative depletion is bounded rather than forbidden.** A serving node
cannot know whether a requester is truly opening a session, and binding consumption
to session-opening evidence is circular — under PQXDH the key is needed *before*
the session exists. So the node **rate-limits one-time key issuance per requester
per subject** rather than policing motive. That bounds the harm the rule was
protecting against — draining a victim's pool to force them onto the last-resort
key — without requiring anyone to prove why they asked.

### 7.9 Archive fetch

**A presence record arrives in whatever form its holder chose** (§4.5.1). The
disclosable fields may be revealed or withheld, withholding is visible in the digest
list, and the record verifies either way. **This is the only fetch path with a use for
location**, so it is the only one where the choice carries information (§4.5.2).

**A patron given an archive head (§4.1 field 7) walks the chain backward.** Doing
that one record per round trip would be prohibitive, so fetching is batched.

```
ArchiveRequest = {
  1: keyhash,          ; subject whose archive is wanted
  2: ? txid,           ; head to walk back from. ABSENT: the holder's newest
                       ;   record for this subject — the recovery case, where
                       ;   the requester lost the one thing this field asks for
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
requested head. **Where field 2 was absent, there is no requested head to match**:
the chain still verifies internally, but its *newestness* is the holder's claim
and nothing the requester holds can check it — a requester restoring its own
archive is trusting the holder not to serve a truncated history
(`light-client-requirements.md` §2 states the interface obligation this creates). **A holder cannot be trusted to have walked correctly**, and the
verification is one hash comparison per record, over records the requester is
already parsing.

**Bounded by the same array limits as everything else** (§1): 256 records per
batch, matching the archive-reference bound it replaces.

### 7.10 What a client hands its serving node

**Four things a light client cannot do for itself.** It cannot publish its own
prekey bundle, stock its own one-time pool, hold its own mail, or be woken by a
service it is not connected to. Each is an act the node it is already attached
to performs on its behalf, and each needs a message. §6.2 gives the fifth member
of this family, a catalog registration, its own request type and says why:
without a message, the one act that matters would be the one thing an attached
client cannot say, and every implementation would invent its own.

**Four request types, not one submission carrying a kind** [2026-09-13]. The
request table is explicit for every other message and a kind tag inside a body
is the sender-supplied discriminator §10.1 refuses for pushes, for the same
reason: it is state every receiver must trust before it knows what it is
reading.

**None of them is read-only** (§9.2). Each changes state at the node or spends
something, so none may be processed in early data.

```
PrekeyPublication = {          ; type 9
  1: PrekeyBundle,             ; §7.8's bundle, signed by its subject
  2: bstr .size 16,            ; nonce, echoed in the reply
}

OneTimeDeposit = {             ; type 10
  1: [ 1*256 bstr ],           ; one-time keys, opaque (§7.8)
  2: bstr .size 16,            ; nonce
}

RelaySubmission = {            ; type 11
  1: keyhash,                  ; the recipient
  2: bstr,                     ; the ciphertext, which this node cannot read
  3: bstr .size 16,            ; nonce
}

WakeRegistration = {           ; type 12
  1: bstr .size 16,            ; nonce
  2: ? tstr .size (1..2048),   ; the endpoint to post a doorbell to (design
                               ;   §14.1.5); ABSENT WITHDRAWS the registration
  3: ? bstr .size (1..256),    ; the key the posted body is encrypted to
  4: ? timestamp,              ; when the client expects the endpoint to lapse
}

SubmissionReply = {            ; the answer to any of the four
  1: bstr .size 16,            ; echoes the request nonce
  2: uint                      ; 0 accepted | 1 refused | 2 over a bound
                               ;   this node applies
}
```

**A withdrawal is field 2 absent, and fields 3 and 4 absent with it.** An
endpoint's key and lapse describe an endpoint; carrying either without one
would be a shape with no meaning, and a decoder rejects it. **An endpoint
arrives with its key**, for the same reason in the other direction: the body
is encrypted to that key before it is posted, so an endpoint without one is an
endpoint nothing can be sent to.

**The subject of a publication is the sender, and the node checks it.** A
bundle names its subject and is signed by it (§7.8); a client publishing
another party's bundle would be choosing the material its peers open sessions
against. The node refuses a publication whose subject is not the authenticated
requester.

**A deposit is bounded by the array limits like everything else** (§1.3), and a
node applies its own storage bound on top: the pool is space it lends.

**A relay submission is answered before the message is delivered, not after.**
The answer says the node took it, which is the same promise design §14.1.6's queue
makes; delivery is the node's problem from that moment, and a sender that
waited for delivery would be waiting on a party that may be offline for days.
**The node refuses one for a recipient it holds no record of**, which is the
distinction design §7.4.3 draws between offline and unknown.

**A wake registration with no endpoint withdraws.** Opting out must be as
sayable as opting in, and a client that has stopped wanting a doorbell should
not have to wait for the endpoint to lapse. **A node keeps at most one per
relationship**, replaces it on re-registration, and forgets it when the
relationship ends.

**The reply carries the nonce and a code and nothing further.** A client learns
that its node took what it said; the node discloses nothing about the other
clients it serves, and a refusal names no reason, since every reason it could
give is about capacity or about somebody else.

**What the node hands the recipient carries the submitter in front of the
ciphertext.**

```
RelayedPayload = [             ; what a node delivers for a submission
  keyhash,                     ; the submitter, as this node authenticated it
  bstr,                        ; the ciphertext, unchanged
]
```

A recipient holds material for many peers and must choose which to try before
it can read anything, and only the node that took the submission knows who
handed it over. **The name is a routing hint and not an attribution.** It is
the node's assertion, not the sender's, and a recipient that treated it as
authorship would be letting its own node say who wrote to it; what a message
is attributed to is decided by the material it opens under (§7.8, design
§14.2.4).

## 8. Session messages

### 8.0 Control frame framing

**Stream 0 carries length-delimited, type-tagged frames.** Without this two
implementations cannot parse each other at all.

```
frame = u32-be length || deterministic CBOR of [ uint frame_type, body ]
```

`length` counts the CBOR bytes that follow it, and is bounded at **64 KB — exactly
65,536 bytes**, stated because "64 KB" reads as 64,000 to a
decimal implementer and the boundary frames would divide the two.

**A declared length above the bound is a protocol error, and it ends the
session** [2026-09-02] — this sits *below* the malformed-frame rule, not under
it. A malformed body inside a bounded frame is cheap to discard and the
session survives; a length out of contract is a violation of the framing layer
itself, and skipping it would mean streaming an attacker-declared volume
through the very ceiling that exists to bound the receiver's buffer.

| `frame_type` | Body |
|---|---|
| 1 | `Attach` |
| 2 | `AttachAck` |
| 3 | `Heartbeat` |
| 4 | `SiblingUpdate` |
| 5 | `TopologyPush` (§10.1) |
| 6 | `TopologyMemo` (§10.2) |

**Topology framing belongs on stream 0 and not on a bidirectional stream.** Both are unsolicited pushes with no reply, so neither is a request; and
the extension posture decides it. An unknown control frame is **skipped** and the
session survives, which is the correct outcome for gossip carrying a class the
receiver does not implement. An unknown request type on a bidirectional stream is
**rejected** (§9.2), which would turn every future topology class into a flag day.
`SiblingUpdate` is the precedent: a server-initiated frame that arrives with nobody
having asked.

**Unknown frame types MUST be skipped, not rejected.** The length prefix exists so
a receiver can skip one it does not understand; tearing down the connection instead
would make every future frame type a flag day. This is the same extension posture
as unknown capability parameters (§8.1).

**A malformed control frame of a known type is discarded whole, and the session
survives** [2026-09-02]. A control frame is unsolicited information with no
reply channel, so there is nothing to answer and nothing to abort; a single
corrupted frame is indistinguishable from loss, and closing an authenticated
session over what loss would have cost nothing is a self-inflicted outage.
§8.2 states the two instances this generalises — a malformed heartbeat counts
as absence, a malformed `SiblingUpdate` is ignored whole with the previous
list standing — and the rule holds for every control frame, `TopologyPush`
included: discard, hold what you held, let reconciliation or the next frame
repair it.

### 8.1 Capabilities

**Modelled on QUIC transport parameters** (RFC 9000), which are already the
transport underneath this.

```
Capabilities = { * uint => bstr }   ; parameter id => opaque value
```

**Parameter ids are derived from names, not assigned by anyone.**

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
batch a peer will serve (§7.9), which prekey construction it implements (§7.8),
what it will accept as a resolution referral depth (§7.7). A bitfield could express
none of these, and would cap the space at 64.

**Absence of a capability is never a connection failure.** Parameters set limits;
they do not gate the session. Two peers operate within what both support, and a
client whose serving node is older stays attached with fewer features. Failing the
attach instead would turn ordinary version skew into a **connectivity** problem —
and the party harmed would be the light client, which has no alternative serving
node except its serving node's siblings.

#### 8.1.1 Greasing — required, not decorative

*A greased id is drawn avoiding ids the sender knows; collision with an id known
only to the peer is accepted at its probability — k/2^64 per draw against a peer
holding k unpublished capabilities, negligible at any real k — and the peer
treats the value as an ordinary unknown parameter.*

**Every implementation MUST tolerate receiving parameters whose ids it does not
recognise.** That is checkable by anyone: send one and observe whether the session
continues. An implementation that rejects it has failed observably, so this
obligation is enforced by every peer that greases.

**The reference implementation sends one greased parameter per session: a random
64-bit id and a value of 8 random bytes.** Any length within the value bound
works; a fixed shape is given so the reference behaviour is reproducible and so a
greased parameter is not itself distinguishable by its size.

**Sending is not a MUST, because nothing could check it.** An unassigned id is
indistinguishable from one the receiver simply does not know, so "did you grease?"
has no answer from the wire. **The reference implementation sends one
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

### 8.2 Frames

Not signed transactions; these are transport-layer control frames.

```
Attach = {
  1: keyhash,          ; client identity
  2: ? CurrencyAttestation,
                       ; an attestation that fails validation — bad signature,
                       ; wrong subject, expired — is treated as ABSENT, not as
                       ; an authentication failure. Currency decides
                       ; which key a caller addresses (§7.1), never
                       ; connectivity; the transport authenticated the client
                       ; already, and refusing the session would strand a
                       ; client whose patron is the party that can refresh it
  3: Capabilities      ; §8.1
}

AttachAck = {
  1: uint,             ; mode: 0 primary, 1 degraded/failover. The SERVER's
                       ; determination, from its own topology, a client is in
                       ; failover iff it is not in this node's subtree. Sent
                       ; because a client with stale topology may not know
  2: ? [ + SiblingRef ],   ; OPTIONAL — absent means the serving node has no
                           ; siblings, which is a legal topology
  3: uint,             ; heartbeat interval, SECONDS, 1..=3600 — a value over
                       ;   3600 is malformed: three-miss
                       ;   detection over more than an hour detects nothing, and
                       ;   an unbounded interval lets a hostile server hold
                       ;   clients in never-failing sessions and makes QUIC idle
                       ;   timeouts unconfigurable. Prior art brackets this:
                       ;   MQTT 5.0 calls keep-alive application-specific,
                       ;   typically a few minutes, and RFC 4787 requires UDP
                       ;   NAT mapping timers of only two minutes minimum, five
                       ;   recommended, so a server advertising near the cap
                       ;   loses its push path through the client's NAT long
                       ;   before liveness fails. Zero is malformed for its own reason:
                       ;   every session instantly overdue, failover permanent.
                       ;   Fixed for the session's lifetime — there is no update
                       ;   message, and changing it requires a fresh attach
  4: uint,             ; messages queued for this client AT THE RESPONDING NODE;
                       ;   U64 RANGE [2026-09-02] — stated so no implementation
                       ;   narrows it by inference. Advisory. In a degraded session this cannot include
                       ;   the dark patron's mailbox — siblings hold no queue
                       ;   state (design §14.1.6) — so a failover value is
                       ;   normally 0 and a client MUST NOT present it as a
                       ;   global count
  5: Capabilities      ; §8.1, the serving node's own
}

SiblingRef = {
  1: keyhash,
  2: [ 1*8 NetworkPoint ],
  3: ? KeyMaterial     ; OMISSIBLE ONLY when the serving node has itself
                       ;   supplied that sibling's key material to this client
                       ;   in an earlier `AttachAck` or `SiblingUpdate` on a
                       ;   session it served. That is the SENDER's
                       ;   obligation; the RECEIVER's rule is simpler: any validated pin whose canonical hash
                       ;   equals the named keyhash is usable, whatever supplied
                       ;   it — a keyhash is a global identity and provenance
                       ;   adds nothing to the check. A client that finds the
                       ;   field absent and holds no pinned entry treats the
                       ;   sibling as UNUSABLE rather than dialling it
                       ;   unauthenticated — there is no fetch path, since the
                       ;   party that would serve one is the node that is down.
                       ; full key material, so a client can authenticate a
                       ; sibling it has never contacted. Without it first
                       ; failover cannot complete: TLS presents only the
                       ; classical component while the keyhash commits to the
                       ; pair, and trust-on-first-use cannot check a keyhash it
                       ; cannot reconstruct. Same reason `ServingInfra` carries
                       ; it (§7.7.3)
}

Heartbeat = {
  1: uint,             ; heartbeat counter — ONE INDEPENDENT SEQUENCE PER SENDER,
                       ; from 0, +1 each beat. Both sides send
                       ; (§9.2), and a shared sequence is unimplementable:
                       ; neither peer can know the interleaving order. Each side
                       ; gap-detects the other's sequence alone — FOR
                       ;   INFORMATION, NEVER FOR LIVENESS [2026-09-02]: any
                       ;   valid beat carrying a counter not yet seen resets
                       ;   the liveness clock, and failover is driven by three
                       ;   missed INTERVALS, not by counter arithmetic. An
                       ;   implementation accepting only the exact expected
                       ;   counter ignores every beat after one loss and
                       ;   fails over against a live server.
                       ; On reaching u64 max, end the session rather than
                       ; wrapping, a wrap would silently reset gap detection.
                       ; NOT the node's locator seqno, a heartbeat needs gap
                       ; detection, and a locator seqno changes only on a
                       ; position or endpoint change (§2.3)
  2: timestamp         ; advisory; liveness uses local monotonic receipt time
}

SiblingUpdate = {
  1: ? [ + SiblingRef ]    ; replaces the client's cached list ENTIRELY.
                           ; Absent means the node now has no siblings.
                           ; A MALFORMED update is ignored whole:
                           ; the previous list stands and the session survives —
                           ; same posture as a malformed heartbeat, and for the
                           ; same reason: tearing down a live session cannot
                           ; repair a bad frame, and a partial list must never
                           ; replace a good one
}

TopologyPush = {
  1: uint,             ; body kind: 0 = signed transaction envelope,
                       ;            1 = EndpointRecord (§7.6)
  2: bstr              ; the object, byte-for-byte as received. NOT re-encoded:
                       ;   it is already canonical (§1) and re-serialising risks
                       ;   changing bytes a signature covers
}

TopologyMemo = {
  1: keyhash,          ; the PATRON — the party whose subtree changed, and the
                       ;   only party this object speaks for
  2: Locator,          ; the patron's OWN position
  3: uint,             ; the subordinate SLOT: one nibble, 0-9, the child index
                       ;   under field 2's path (§2.1)
  4: timestamp,        ; the underlying transaction's own timestamp, copied —
                       ;   not a fresh clock reading. Orders two statements by
                       ;   THIS patron about THIS slot, and nothing else
  5: ? keyhash         ; the node now occupying that slot. ABSENT means the slot
                       ;   is empty: a departure or a disavowal
}
```

**A sibling list naming the receiving client is malformed**, as is one containing
duplicate keyhashes. A client cannot fail over to itself, and a duplicate entry
is either an error or an attempt to weight one endpoint in a list the client is
expected to try in order.

**Every successful `AttachAck` replaces the cached sibling list**, including one
from a failover sibling — its list is the correct one for the node the client is
now attached to. Retaining the primary's list through a degraded session would
send the next failover to peers of a node the client is not talking to.

```
```

`AttachAck` carries the sibling list **when there is one**: a
client cannot discover failover targets after its serving node is already dark.

**Field 2 is optional because it must be.** §1 forbids encoding an empty
array, and an infra node with a single child — a perfectly legal topology — has no
siblings to list. Absence means no siblings.

**The serving node determines the mode; the client is told.** A client
attaching to a sibling sits two hops away in topology the sibling holds within
its horizon, so *"is this client in my subtree?"* answers the question without
anything on the wire. `AttachAck` field 1 exists because **the client may not
know**: one whose serving node changed — its patron grew into infra, say — can
believe it is reaching its primary when it is not. The determination belongs with
the node that has authority over its own subtree.

**`Attach` carries no client-asserted serving node.** design §14.1.2's *"must be
explicit"* is a **user-interface** obligation, below, not a wire one.

**Heartbeat units are seconds** and the counter is per-session. Both were bare
`uint` with no stated meaning; a peer reading milliseconds where the other wrote
seconds fails liveness in exactly the way that looks like a network fault.

**Liveness rule**: a peer is considered failed after **3 consecutive missed
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
- **Misses are counted by elapsed full intervals on a local monotonic clock**, not
  by delivered timer callbacks. A mobile runtime suspended for
  three intervals has missed three on waking — callback-counting would keep a dead
  session apparently alive for as long as the OS withheld the timer. Input already
  readable at a deadline is processed before the deadline fires; there is no grace
  period.
- **The timestamp in a heartbeat is advisory.** Liveness is decided by local
  monotonic receipt time; a clock-skew test would let two peers disagree about
  liveness while exchanging identical frames. Below that a client does not fail over; at or above it, it dials a
sibling of its serving node.

**Failover applies to a primary that is already dark, not only to one that dies
mid-session.** The three-missed-intervals rule presupposes an established
session; a client whose serving node is unreachable at attach time uses its cached
sibling list immediately. Otherwise the list — pushed precisely because it cannot
be discovered once the node is dark — would be unusable in the case it exists for.

**There is no `AttachNack`, and refusal is carried as a QUIC close code.** A node refusing an attach by policy closes the connection with
**application close code 1 (`refused`)** in CONNECTION_CLOSE. A client seeing it
MUST treat the refusal as **that node's answer, not that endpoint's** — its other
addresses will say the same — which gives `light-client-requirements.md`'s existing
retry floor the carrier it lacked. Every other outcome — any other close code,
reset, timeout, no valid `AttachAck` — is an **endpoint** failure: the client tries
the remaining candidates in the order received, and when all fail it is disconnected
and a later fresh attach begins with the primary again. **Two scope rules for
the refusal itself** [2026-09-02]: a primary's refusal does **not** open
sibling failover — the failover triggers are unreachability at attach and the
three-missed-intervals rule, and a refusal is an answer, not an outage, so
siblings are not a channel for overriding it. And during failover, one
sibling's refusal forecloses **that sibling alone**: the remaining candidates
are tried in the order received. Timeout and backoff are
local policy. *Code 0 is not used for refusal, since it is the conventional
no-error close.*

**Ordering before the ack.** Unknown control frames may precede
`AttachAck` and are skipped as always; **any known frame other than `AttachAck`
arriving first fails the attach attempt.** After the ack, a repeated `Attach` or
`AttachAck` on the session is a protocol error.

**A server MUST NOT process an `Attach` received in TLS 1.3 0-RTT early data** — reject it or defer it until handshake completion. Early data is
replayable, and a replayed `Attach` re-binds session state; 0-RTT resumption still
serves reattachment latency because everything after the handshake keeps its
benefit. The rule sits on the server because that is where it is checkable. **It is
one instance of §9.2's general rule** — anything replayable that changes state waits
for the handshake — and `Attach` is named here because this is where it arrives.

**The cached sibling list should survive restart.** It is replaced by any
`AttachAck` or `SiblingUpdate` and otherwise persists; an in-memory-only client
loses failover exactly when a crash coincides with its serving node being down.

**No automatic failback.** A client on a sibling stays there until that session
ends; the next fresh attach tries its actual serving node first. Probing the
primary from a degraded session adds traffic for a state the client will leave
anyway at the next natural reattachment.

Queuing is continuous — there is no "begin queuing" signal, and
reachability is only a hint about when to attempt delivery.

---

## 9. Transport binding

### 9.1 Handshake and hop authentication

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

**Authentication is mutual.** The serving node authenticates the client the
same way, and **MUST bind the identity for which session state and queued data are
requested to the identity the transport authenticated, rejecting any mismatch.**
Present encoding: `Attach` field 1 must equal the connection-authenticated
identity. `Attach` is unsigned, so without that check any party
could claim any keyhash and receive another node's queued messages.

That is the whole authentication step, and it is exactly what §7.7's unsigned resolution
replies rely on, a wrong address produces a failed handshake rather than a false
identity.

**Downgrade protection** is TLS 1.3's own. **That nothing can be negotiated below
the named group is this profile's doing, not TLS's** — TLS 1.3 negotiates among
whatever groups are configured, and offering only `X25519MLKEM768` is what leaves
nothing weaker to fall back to.

### 9.2 Binding

- ALPN: `rhtn/1`
- **Default port 7431/udp**, overridable per endpoint. `NetworkPoint` carried no
  port at all, so a client could not turn a sibling reference into a socket
  address
- Stream 0: the control frames of §8.0 — session control (attach, heartbeat,
  sibling updates) and topology propagation (push, memo)
**Every bidirectional stream opens with a request-type tag.**
Resolution, archive fetch, prekey fetch, resource requests, catalog queries and
verifier queries all share the same ALPN and the same stream class, and **nothing
told a receiver which it had.** Structural guessing across six schemas is not a
protocol.

```
frame = u32-be length || deterministic CBOR of [ uint request_type, body ]
```

Same framing as stream 0 (§8.0), same rule — **unknown request types are rejected
on a bidirectional stream**, unlike unknown control frames which are skipped — but
**a larger bound: 256 KB — exactly 262,144 bytes.** Stream 0's 64 KB cannot carry what
bidirectional streams exist to fetch: a maximum-signer presence record is ~65 KB
before its presentation slots, and a recovery adoption ~42 KB. Control frames stay at
64 KB — nothing on stream 0 approaches it. A control frame arrives on a shared stream where skipping preserves the
session; a bidirectional stream *is* the request, so a type nobody understands has
no continuation to preserve.

| `request_type` | Body |
|---|---|
| 1 | `ResolveRequest` (§7.7) |
| 2 | `ArchiveRequest` (§7.9) |
| 3 | `PrekeyRequest` / `PrekeyBatchRequest` (§7.8) |
| 4 | `[ VerificationQuery, COSE_Sign1, uint ]` — the query, the subject's consent, and the selector's `selection_basis` claim (§5) |
| 5 | `CatalogQuery` (§6.4) |
| 6 | `ResourceRequest` (§11) |
| 7 | `ResourceRegistration` (§6.2) |
| 8 | `CurrencyRequest` (§7.1) |
| 9 | `PrekeyPublication` (§7.10) |
| 10 | `OneTimeDeposit` (§7.10) |
| 11 | `RelaySubmission` (§7.10) |
| 12 | `WakeRegistration` (§7.10) |

**The reply carries no type tag and is framed identically otherwise** — the same
`u32-be` length prefix and CBOR body. It answers a request whose type the requester
chose, on a stream it opened, so a tag would restate what the requester already
knows.

**One request frame per stream, and the requester half-closes after it.** The
stream *is* the request, so a second frame has nothing to be: a responder processes
the first and treats anything after it as a stream failure. **A responder does not
wait for the half-close before answering** — a requester waiting for the answer
before closing and a responder waiting for the close before answering would
deadlock, and nothing else in the framing chooses between them.

**Where a malformed frame stops being answerable.** Until the array header and
the `request_type` are readable, the receiver does not know what it is holding, so
there is nothing to answer in and the stream fails. **Once the type is known, a
defect in the body is that type's business** and is answered in that type's own
terms — §11's status 3, for instance. **No application error code is assigned for
the reset**, and none is needed: the reset is the whole message, and a requester
waiting to be told why would be waiting for something no responder owes it.

**A request that changes state or spends a budget MUST NOT be processed in TLS 1.3
0-RTT early data** — reject it or defer it until the handshake
completes. Early data is replayable, so a replay repeats whatever the request did:
an application effect the requester never asked for twice (§11), a consumed
one-time prekey (§7.8), a spent anti-oracle count (§5), a replayed registration
that reverts an owner's current entry (§6.5). **Read-only lookups are unaffected** — resolution,
archive fetch and catalog queries answer the same way however often they are
replayed, which is what makes 0-RTT still worth having. A resource request is
never in the read-only class, whatever the HTTP method inside (§11), and
neither is anything a client hands its serving node (§7.10): each changes state
there or spends something. §8.2's rule for `Attach` is
this rule's other instance, on the other stream class.

- Bidirectional streams: request/response — the request types tabled above
- Unidirectional streams: payload delivery, queue drain
- Connection migration relied upon for mobile IP change; 0-RTT resumption for
  reattachment, which permits a lazy heartbeat and saves battery

**NAT traversal for the payload path only**. Control traffic
needs none: clients dial outward to their serving infra node at a static IP, and
infra nodes dial each other directly.

**Payload attempts a direct path inside the horizon**, which does require
traversal. Infra nodes act as **STUN and TURN**, and ICE falls back to relaying
through the serving node when hole punching fails. Address- and port-dependent
mapping defeats it, and carrier-grade NAT and mobile networks raise the odds of
meeting that, but **neither guarantees failure** (RFC 8445).

---

## 10. Topology propagation

### 10.1 The push frame and the forwarding rule

**design §15 defines the classes and the patterns; this section says what carries
them.** *Flood-within-horizon* and *push near, redirect far* are
named and reasoned about across rotation (design §9.0.2), peering (design §6.3)
and the endpoint records of §7.6. Until now none of them said what was sent, on
what stream, or how a receiver decided whether to forward. **A policy is not a
protocol.**

**Frame:** `TopologyPush`, control frame type 5 on stream 0 (§8.0), carrying the
object byte-for-byte with a **body-kind tag and nothing else.** The class is not
uniform — most topology objects are signed transactions with an envelope, and an
`EndpointRecord` (§7.6) is a standalone signed map with neither — so a receiver needs
one discriminator to know which parser to use. **That is the only wrapper field
permitted.** Anything further would be sender-supplied state every receiver must
trust, which §7.7.3 rejected for resolution and rejects here for the same reason.

**Who may push.** A node pushes topology it is a party to — its own adoptions and
departures, disavowals of its own subordinates, its own peerings, its own endpoint
record. **Within the horizon this is a trusted push**, in the narrow sense that the
pusher is the party whose position the object describes and the object is signed by
that party. It is not an assertion about anyone else's topology.

**This governs origination, not relaying.** A node forwarding a
stored object under the rule below is ordinarily *not* a party to it, and reading
the paragraph above as a rule on every sender would stop each flood at its first
relay. The distinction costs nothing, because **a relay changes nothing**: it
forwards the object byte-for-byte, and the signature that made it worth trusting at
the origin is the one the next receiver checks.

#### 10.1.1 The forwarding rule: forward if and only if you stored it

**A node stores a topology-class transaction when its subject falls within that
node's own `h_store`; a node forwards a stored transaction to every adjacent node
except the one it arrived from.** There is no hop count, no TTL and no reach
field.

**Adjacent means the authenticated sessions you already hold by virtue of a topology
relationship**: your patron and your subordinates where sessions with them exist,
your peers (design §6.3), **your serving node, and the clients attached to you**
(design §14.1.2). It is not a set to maintain — it is the
sessions the node has anyway, which is the same move the duplicate rule makes with
the store. **The serving relationship is what carries control past light-client
patrons**, who hold no sessions and relay nothing (design §12.6.3): a client's
floods enter and leave the network through the node that serves it, which is how a
node two levels below its infra ancestor keeps its own horizon view and publishes
its own departure. **Node type does not
enter it**, and must not: type is not a function of position — a node becomes infra
by launching and signing an infra instance, without moving (design §12.6.1) — so a
rule phrased on type would go stale on an act that changed no topology at all. It is
the same reason a locator does not encode type. Leaving adjacency unstated would let two conforming nodes store the same
objects and deliver them to different neighbourhoods, which shows up as a permanent
gap rather than as an error.

**The subject is the node whose position the transaction changes**: the adopted or
departing node, the disavowed subordinate, the node whose series a reissue
starts (§4.6, field 1 — the patron countersigns but it is the node's line that
changes) [2026-09-02], and — for a peering, which has two —
either endpoint, so the transaction is in range if either is. Reading the
*issuer* as the subject would put a patron's adoption of a distant node in range of
everyone near the patron, which is not whose neighbourhood changed.

**Stored means verified.** A node forwards what it stored, so storing an object
it could not verify would make it an amplifier for whatever an authenticated
neighbour cared to send. A transaction whose signer's key material the node lacks is
§3.4's *neither verified nor rejected*: hold it, fetch the key, and let it enter
storage and propagation when it verifies. **An `EndpointRecord` is the exception the
design already states** — self-signed by a party the receiver may hold no key for,
and accepted as gossip precisely so that reaching the address is what confirms it
(§7.6).

**Reach is a consequence of storage policy, not a separate mechanism.** A hop
counter would encode the *sender's* horizon and impose it on every receiver, and
design §15.1 is explicit that a horizon "is a scope, not a shared region… no two
nodes with different positions have the same one." A counter is also a value an
intermediary can fail to decrement, so a rule resting on it asks each node to trust
arithmetic it cannot check. Deriving the decision from the receiver's own store
removes both problems, and introduces no concept the node did not already have.

**Each receiver evaluates on receipt against its current view.** Local topology
changes while a message is in flight, and a receiver that evaluates against what it
holds *now* is correct by construction where one applying a sender's precomputed
reach is not.

**It also preserves the constant-state floor.** design §12.6.1 bounds what a node
*must* keep at parent plus ≤f children. A storage-derived rule inherits that bound;
a TTL-derived one would let a sender push traffic into nodes that had decided not to
hold it.

#### 10.1.2 Duplicate and loop suppression: by `txid`, against the store

**A node that already holds an object does not store it again and does not forward
it.** The store the node keeps anyway is the seen-set. A horizon contains cycles
once peering exists (design §6.3); the second arrival is a duplicate and dies there.

**Identity differs by body kind, and both are already defined:**

| Kind | Identity | Duplicate when |
|---|---|---|
| Signed transaction | `txid` (§1) | the `txid` is already held |
| `EndpointRecord` | `(subject keyhash, seqno)` | the held `seqno` is greater than or equal (§2.3) |

**An `EndpointRecord` therefore supersedes rather than accumulating**, which is what a
current-address record should do, and §2.3's strictly-greater rule already governs it.
**Equal `seqno` with different signed contents is malformed**, per §7.7.3 —
endpoints and extensions alike, since §2.3's rationale is that a subject
advances its own counter for *any* new signed content [2026-09-02].
**Malformed names the pair, not the later arrival** [2026-09-02]: two signed
contents at one number can only mean equivocation or a key in two hands, and
which arrived first is an accident of the path — a holder that kept the
earlier one would let arrival order split the network's view, the thing this
section's own repetition rule refuses on bytes. On discovering the conflict
a holder retains **neither** as current and forwards nothing further for that
`(subject, seqno)`; it repairs by re-resolving (§7.7), which descends the
authenticated path and lands on whatever the subject's line actually says.
The line itself is repaired only by its subject: a greater counter, or §4.6's
reissue. Whether the holder keeps the pair as evidence is local. **Records in
different series are not comparable** and neither supersedes the other (§2.3); a
holder keeps the one whose series it has been shown a chain for (§4.6) and re-resolves
if it holds none. **A record in a series the receiver cannot prove current is
neither stored nor forwarded** [2026-09-02] — the same posture as the
transaction whose signer key is missing: it enters storage and propagation when
its prerequisite does. Whether the receiver holds it meanwhile or drops it and
lets reconciliation replay it is local; the wire-visible rule is only that an
unproved series never floods onward, since a forwarding node vouches with its
storage decision and this one it could not make.

**No dedicated suppression cache exists**, and none should be added. It would be a
second copy of a fact the store already holds, with its own expiry parameter to leave
unset.

#### 10.1.3 No acknowledgement, no retry

**Deliberately.** Neither is defined, and their absence is a decision rather
than an omission.

**Loss is detected at the receiver, which is the party that can act.** §3.1 puts a
back-pointer in every signed body, so a transaction whose predecessor the receiver
does not hold announces the gap, names exactly what is missing, and §7.9 fetches it.
That is stronger than acknowledgement, which detects loss at the sender — who can
only resend into the same hole.

**Second repair path: periodic reconciliation.** A node that missed something it
ought to hold reconciles with its siblings and its patron, and reconciliation is a
**replay of the same frames** rather than a distinct mechanism. There is no repair
protocol to specify beyond what propagation already defines.

**And acknowledgement aimed at a party you share no state with is design §1.1's
unenforceable direction.** You cannot make a peer forward. Acks and retries would
also make gossip traffic scale with population, inverting design §15's stated
scaling property.

### 10.2 Rootward topology memo

**A minified record of every membership change travels rootward to its subnet's
root.** `TopologyMemo`, control frame type 6 on stream 0 (§8.0).
This is what design §15's *ancestors* reach means: **full transactions flood within
the horizon; only memos travel further up.**

**Restricted to membership operations — adoption, departure and disavowal — and the
restriction is load-bearing.** Peering is topology class and is **excluded from
rootward travel**: a peering record carries `NetworkPoint` for both endpoints plus
ASN (§4.4), and design §19.8 C8 maps that composition to a
natural person. A memo carries keys and positions and no address, which is what
makes a root's accumulated view tolerable; adding peering for symmetry would
silently remove that property.

**Forwarding.** A receiving node forwards the memo to its own patron, unchanged
except that nothing is added — the memo already carries the position it describes.
A root has no patron and forwarding stops there.

**Where no session with the patron exists, the memo goes to the nearest
infrastructure node on the patron chain instead** (design §14.1.2). A light-client patron holds no sessions (design §12.6.3), so hops
through one are collapsed through the infrastructure that serves it; a skipped
patron's optional table simply has gaps, which *no tier is load-bearing* already
prices. **A serving node runs the cycle check for its attached clients as well as
for itself** — it holds its whole light-client subtree (design §12.6.1), so it
checks field 1 against itself and every attached client, and a hit for an attached
client is handed to that client at contact: the records that confirm it and the
disavowal that answers it are that client's (design §6.2.2), not the serving
node's.

**A node holding that slot at or after the memo's timestamp does not forward it.**
It has already passed on what the memo says, or something later, so
no upstream table can need it. This costs a little latency on a genuinely late
out-of-order memo — repaired by the next one or by reconciliation, the failure this
section already accepts — and it stops a **replayed** memo at the first table-holding
hop above wherever it was injected, rather than letting it travel to the party it
names. **A memo never leaves its subnet**,
because its anchor names the subnet and rootward travel terminates at that subnet's
root. This is what keeps the mechanism clear of design §3.1.1: nothing compares a
node's binding in one subnet against its binding in another, and nothing adjudicates
between them.

**So a memo whose anchor is not your subnet's is dropped, not forwarded** — neither applied to a table nor passed on. The property above is only a
property if a receiver enforces it; a node that forwarded such a memo would carry it
across the boundary the privacy argument rests on.

**No acknowledgement and no retry**, on §10.1's reasoning: one best-effort send to
your patron, and a memo that does not arrive is repaired by the next one or by
reconciliation. A table mutation already made is kept — the memo was true when it
passed.

#### 10.2.1 Two checks, with different requirements

| Check | Needs | Reach |
|---|---|---|
| **Is this me?** — cycle detection | the memo alone | any depth |
| **Do I already hold this node elsewhere?** — re-parenting | a memo table | wherever a table exists |

**The cycle check is an identity comparison.** A memo travels up
patron edges and field 1 names the patron it speaks for. **If field 1 is you, a memo
you originated has come back to you from below** — you are your own ancestor, and
that is a cycle. It fires at any depth, on the memo alone, and terminates the memo
there.

**No path arithmetic is involved, deliberately.** **Containment is not the test
and cannot be**: asking whether field 2's path *contains* the receiver's own
position fails because a memo reaches you precisely because you are an ancestor of
the patron, so your path is a prefix of theirs on **every legitimate hop** and
containment would report a cycle on ordinary traffic. Comparing positions for equality would work, but comparing
identities is exact, cheaper, and unaffected by anchors, padding or a counter that
moved for an endpoint change (§2.3).

**The re-parenting check needs a table**, described below.

#### 10.2.2 The memo table

**A node MAY maintain a table of `(patron, slot) → occupant` built from the memos
that pass through it.** Every memo from below traverses it, so the table's
coverage is that node's **whole subtree** rather than its horizon. Read the other
way — by occupant — it answers the re-parenting question: a node appearing in two
slots is held in two places.

**It is a RIB.** design §15's liveness class already sets the retention rule: *keep
the table, not the update history, as BGP keeps the RIB.* Write the slot, retain the
current mapping, discard the memo. No separate retention parameter is needed and
none is defined.

**An empty slot is a row, not a deletion.** Field 5 absent writes *nobody* into
that slot; it does not remove the row. The row carries the timestamp that emptied it,
which is what stops a late-arriving earlier memo from reinstating an occupant the
patron has already removed.

**Ordering is by field 4, and the comparison is always within one patron and one
slot.** Later timestamp replaces; equal timestamps break by arrival
order. **No clock is compared across nodes**: two memos about one slot were written
by the same patron from the same clock, which is the one case where a timestamp
orders reliably. This is why the field is the underlying transaction's own timestamp
(§4.1, §4.2, §4.3) rather than a reading taken when the memo is composed — it is
copied from a signed record and a detector that fetches that record can check it.

**Every field is a statement the patron has authority to make.** It names
itself, its own position, its own slot, and who is in it. **Nothing in the object
comes from a party that did not sign the transaction behind it**, which is why a
disavowal — the patron's act alone (§4.3), which the subordinate neither signs nor
contributes to — produces a memo like any other. A subordinate reaches the subtree on
its patron's authority and ceases to exist from the subtree's point of view when that
authority is withdrawn.

**And the reason code stays where it was decided.** A disavowal's reason (§4.3)
is in-horizon state and does not travel rootward. A root accumulates *that* a
membership changed, never *why* — the same restraint that keeps peering out of this
class.

**The table is optional, and detection degrades gracefully rather than failing.**
design §12.6.1 fixes the required state at parent plus ≤f children and calls
anything beyond it "an optimisation above the floor." A tier that keeps no table
loses latency, not detection: the memo continues upward and a tier that does keep one
catches the conflict, at worst the root. **No tier is load-bearing.**

> **Pressure worth naming.** At the root of a large subnet the table *is* a map of
> the subnet, which is exactly the state design §12.6.1 says a node must never be
> *required* to hold. Permitted-but-incentivised is how such floors erode. An
> implementer reading design §12.6.1 alone will not see this coming, which is why it is
> stated here.

#### 10.2.3 A memo is a hint, never evidence

**No node acts on a memo alone.** A memo is a derived summary and is not signed;
an intermediary can fabricate one. Acting directly would manufacture the **false
positive** design §6.2.5 ranks as the worse failure — a refused or severed legitimate
adoption, indistinguishable from censorship.

**So a detection is confirmed against the detector's own records, which is always
possible.** The cycle check fires only on the party field 1 names,
and that party signed the transaction behind the memo and holds the slot it
describes: it asks whether it really made that change, and whether its row for that
slot is still the one the memo asserts. **A fabricated memo fails both**, at no
traffic cost. This is what lets the memo stay small and unsigned.

**Not a fetch.** A `§7.9` archive request needs a head, and a head reaches you only
by having adopted the subject (§4.1 field 7) — so a rule requiring a fetch would be
unperformable for exactly the distant cycles the memo exists to catch. It is also
unnecessary: the detector is never a stranger to the transaction.

**What survives is a replay of the memo describing the *current* slot state**, which
matches the detector's row and is therefore indistinguishable from a genuine loop.
**Registered and accepted** (design §18.2): the injector must sit at or below one of
the detector's own subordinates, the edge severed is on the injector's route, and
the disavowal carries no prejudice.

#### 10.2.4 What a detecting node does

**It disavows the direct subordinate that forwarded the memo to it**, once the
memo is confirmed against its own records (above). Any edge breaks a cycle, and that is the
one the detector has authority over (design §6.2.2). It requires no agreement with
the other party, no tie-break rule and no clock.

**Reason code 5, without prejudice** (§4.3). Nothing adverse is alleged: a cycle is
a structural accident, and design §6.2.5 already calls the bootstrap case "a likely
accident rather than an attack."

**Where both parties are present, the prompt is better and comes first.** design
§6.2.5's disambiguation — *one of you must be the patron* — resolves the bootstrap
case socially. Automatic disavowal is the fallback for cycles formed at a distance,
which is the case the memo exists for.

**The downward memo.** A node whose table shows the same occupant already held in
another slot in its subtree sends a memo down the branch holding **the slot the
arriving memo did not name**. The choice is trigger-relative and
needs no clock — the arriving memo names one slot, the table holds the other, and
the memo goes toward the patron who has not just spoken. Whether that edge is in
fact stale is that patron's own records' business: the memo is a hint there as
everywhere. Same frame; direction is implied by the receiver's position relative to
the sender rather than by a field.

**It walks the tree rather than being addressed to the old patron**, and that is the
point: the intervening nodes are updated on the way past. Descent is by anchor and
path (§7.7.2), the same mechanism as resolution.

**Nothing compels the receiving patron to act.** Its own records say whether it
still holds that subordinate, and design §1.1 is why nothing further is said. **A node
registered at two positions is not known to harm the network**: addressing is by
anchor and path, never by lookup against a higher tier's table, so a stale entry
misroutes nobody. What the memo achieves even where a patron declines to disavow is
that **the rest of the subnet's view reflects the most recent adoption**, to the
extent its members run compliant clients. That is a convergence property of compliant
behaviour, not a rule anyone enforces.

## 11. Resource requests

**A resource request rides a bidirectional stream on a session the requester holds
with the *hosting* node**, tagged request type 6 (§9.2), one request per stream.
**Every type-6 request is in §9.2's 0-RTT-forbidden class** [2026-09-02]: the
gateway deliberately does not interpret application semantics, so it cannot
certify any HTTP method effect-free — a `ResourceRequest` stream is never
processed in early data, whatever the method inside.

**A backend failing during the handoff is answered `resource unavailable`, and
the gateway NEVER retries on its own** [2026-09-02]: it cannot know whether
the application received enough of a side-effecting request to commit, so an
automatic retry could duplicate an effect. Retry is the requester's decision —
the requester knows its request's semantics; the gateway does not.

```
ResourceRequest = {
  1: keyhash,          ; the resource being addressed. A hosting node runs several,
                       ;   and nothing else in the frame names which
  2: bstr             ; the application request as an HTTP/1.1 message: request
                      ;   line, headers, blank line, body (RFC 9112). The node
                      ;   PARSES and RE-SERIALISES it (§11.2) — it does not
                      ;   relay the bytes. Opaque means the node does not
                      ;   interpret the application semantics, not that it
                      ;   forwards unexamined input
}

ResourceResponse = {
  1: uint,             ; 0 delivered | 1 refused | 2 resource unavailable
                       ;   | 3 malformed request | 4 no subtree acknowledgement
                       ;   | 5 no matching role
                       ; EVALUATION ORDER IS NORMATIVE, see below
  2: ? bstr            ; the application response as an HTTP/1.1 message, present
                       ;   iff field 1 = 0. Same reasoning as the request
}
```

**A response larger than the frame bound is sent as a sequence.** Field 1 = 0
may repeat on the same stream, each carrying the next portion of one HTTP message,
with the stream closing when it ends. **No length is declared up front**: a resource
streaming a response does not know one, and requiring it would forbid exactly the
case the sequence exists for.

**A non-zero status ends the exchange** and may not be followed by further frames.
There is one answer and the stream carries it.

**Evaluation order is normative**, because it decides which of two true things a
requester is told.

0. **The body decodes at all** → **code 3** if it does not. Nothing has been
   addressed yet, so answering directly discloses nothing about any resource. A
   malformed *outer frame* is a stream failure, not a response (§9.2).
1. **Resource exists on this node** → **code 1** if it does not. An unknown resource
   keyhash has **no owner**, and membership is owner-relative (design §11.2), so
   there is no membership question to ask first. **Existing means the node holds a
   binding from that keyhash to an owner and a backend** — not that the package is
   running, which is step 6, and not that a `CatalogEntry` was ever published, which
   is optional (design §11.5). Collapsing a stopped package into absence would answer
   code 1 where another host answers code 2 for the same deployment.
2. **Membership** in that resource owner's trust horizon → code 1. Nothing further is
   evaluated or disclosed.
3. **Subtree acknowledgement** → code 4 if absent.
4. **The role row** → code 5 if it grants no `connect` (design §11.4). **A lookup,
   never a predicate evaluation** — predicates are a macro over the materialised
   table and neither of their two evaluations is on this path.
5. **The carried HTTP message is well-formed** → code 3 if it is not. It sits here
   rather than at step 0 because by now the asker has passed the opaque gates;
   answering earlier would let a stranger probe a resource by sending it rubbish
   and reading which complaint came back.
6. **Availability** → code 2 if the package is not running.

**Absence is refusal, to everyone.** A member asking for a keyhash this host
does not have gets code 1, exactly as a stranger does — **the host cannot tell
whether that resource exists elsewhere**, and answering *no such resource* would
assert something it does not know. There is no code for it because there is no state
in which the host could honestly send one.

**Availability is checked last, so a role-holder learns the service is down and a
non-role-holder never does.** Putting availability before the role check would tell
anyone in the horizon when a resource is offline, which is operational information
about the owner. And running availability first would answer *unavailable* to
someone who has no role,
which is true and useless — they would retry forever against a resource they could
never reach.

**Each code answers exactly one question**, so a member is never left choosing
between two explanations: *no role* and *service down* are different answers and a
member entitled to one should not receive the other.

**The non-zero codes are the node's answers, not the resource's.** A resource
that returns an application-level error returns it inside field 2 with code 0 —
**the network delivered it**. Conflating the two would let an application error
look like a gateway refusal.

**A requester inside the owner's trust horizon gets a specific reason; one outside
gets `refused` and nothing more.** The membership gate (design §11.2) runs first,
so the node already knows which it is talking to.

**Inside the horizon, withholding the reason helps nobody.** A member who lacks a
`SubtreeAck` (code 4) or holds no role row granting `connect` (code 5) can act on
that — ask
the grandpatron to acknowledge them, ask the owner for a role — and a bare refusal
leaves the resource undiagnosable to precisely the people entitled to use it. They
already hold the topology those answers describe.

**Outside it, `refused` (code 1) covers everything**: not a member, membership
lapsed, resource does not exist for you. A stranger learns nothing about the owner's
membership or policy, which is the case the opacity was for — and **a keyhash naming
nothing returns the same code**, otherwise a stranger enumerates the host's resources
by watching which lookups differ.

**The codes are equal; the timing is not specified.** A host that looks up a resource
it has and one it does not may take measurably different time, and nothing here sets
an envelope for that. **The opacity is in what the node says, not in how long it
takes to say it** — an implementation that cares should equalise the two paths, and
one that does not has a narrower property than this section describes.

### 11.1 Reaching a node you are not attached to

**This is ordinary session establishment, not a gap.** A resource
request goes to the hosting node and a catalog query to each horizon node in turn;
neither is the asker's serving node, and **neither needs a mechanism the design
lacks.**

**Messaging already works this way.** design §12.6.3's relayed path runs client →
own serving node → **recipient's serving node** → recipient, and its direct path has
a client reach a peer outside its own subtree entirely. Infra nodes hold static
addresses (§7.2) and authenticate by keyhash (§9.1), so opening a session to one is
the same operation wherever it sits.

**Attachment is not exclusivity.** design §14.1.2 makes attachment the answer to
*where do my messages queue* — one node, because a mailbox must have one address. It
says nothing about which nodes a client may open a session with, and reading it as a
restriction was a misreading of what the singular is for.

---

### 11.2 The node parses; it does not relay

**A node that strips `rhtn-*` headers is parsing HTTP, and must be specified as
doing so.** Saying it relays bytes opaquely *and* replaces
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
- **Reject anything asking for an exchange other than one request and one
  response**: `CONNECT`, `Upgrade`, `Expect: 100-continue`. Each needs a tunnel, a
  protocol switch or an interim response, and `ResourceResponse` carries one answer
  (§11). They are ordinary HTTP and they do not fit here.
- **Route by the resource keyhash, never by anything the caller wrote.** Convert an
  absolute-form target to origin form and replace `Host` with the backend you
  selected — for a backend with no network authority (a local socket), with
  whatever authority your installation recorded for it: binding state, not
  protocol [2026-09-02]. Field 1 already named the resource; letting a header re-aim the request
  is how a proxy is turned into someone else's client.
- **Insert your headers after stripping, into the re-serialised message.**

**"Canonically" means *from your parse, deterministically* — not identically to
another node.** Nothing signs these bytes and no second implementation compares
them, so there is no interoperable serialisation to agree on. The property that
matters is that what you emit is what you parsed, entirely, and that the same input
twice gives the same output. Ordinary HTTP tolerance covers the rest.

**This is a category with published prior art rather than a novel hazard**, and it
is why the requirement is *parse and re-serialise* rather than a list of patterns
to filter. A filter enumerates what its author thought of.

---

## 12. Size estimates

**All archive-retained transactions are post-quantum** (design §5.1), so the
classical column applies only to session-layer traffic.

| Object | Classical | Post-quantum |
|---|---|---|
| Adoption | — | **~8 KB** |
| Adoption carrying a recovery (~10 responses) | — | **~42 KB** — ~8 KB plus ~34 KB of hybrid verifier authentications (§4.1, §4.5). The second-largest object in the protocol, and rare by construction |
| Adoption carrying a transfer | — | **~11 KB** — ~8 KB plus one hybrid logical signer at 3,373 B for the former patron's countersignature (§4.1). The `txid` alternative in field 8 costs 32 B, so a transfer is the more expensive of the two evidence routes |
| Departure | — | **~4 KB** |
| Disavowal | — | **~4 KB** |
| Peering | — | **~8 KB** |
| Presence record (typical, ~10 signers) | ~2 KB | **~35 KB** at ML-DSA-65. Includes ~112 B of disclosure salts, **0.3%** (§4.5.1) |
| Presence record (maximum signers) | ~5 KB | **~65 KB** — 18 logical signers × (64 + 3,309), plus 32 verifier responses at 2 classical signatures each |
| Currency attestation | ~150 B | ~2.6 KB |
| Anchor entry | ~60 B | ~60 B (hashes only) |

**Selective disclosure does not reduce a presentation.** A minimised record replaces
each withheld field with a 32-byte digest, at most 224 B, against ~34 KB of signatures
that cannot be omitted — the envelope requires exactly the required signer set. It buys
disclosure control, not bandwidth (§4.5.1).

Presence records are the deliberate PQ exception: rare, and they must remain
verifiable for decades. A few hundred per user per decade is under 10 MB lifetime.

---

## 13. Open items

1. **Queue cap value.** A per-node policy value; design §21.1.1 classifies it
   *freely tunable, forever*, and nothing here fixes one.
2. **Canonical test vectors.** A draft set exists at `test-vectors/` —
   spec-derived and generated, with every computed value now reproduced by a
   second harness in a different language over independent cryptographic
   implementations (`test-vectors/runner-rs`, RustCrypto ML-DSA against the
   generator's dilithium-py) [2026-09-03]. That is cross-language and
   cross-crypto validation, not independence: both sides share an author, so
   an interpretation both encode would pass both. The draft's README states
   every interpretation taken so that each is a review target rather than a
   silent choice; ten clean-room implementation reviews have since traced the
   fixtures against independently written code. **Canonical status still waits
   on an independent party's implementation reproducing every computed
   value.**
