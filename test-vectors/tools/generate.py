#!/usr/bin/env python3
"""Generate the draft canonical test vectors for the RHTN wire format.

Everything computable is computed; nothing is hand-transcribed. Re-running this
script regenerates keys.md, primitives.md, transactions.md and
verifier-selection.md byte-for-byte. README.md and negative-vectors.md are
authored by hand.

Status: DRAFT, derived from `wire-format.md` alone and verified by no
implementation. Interpretations this script had to take where the specification
under-determines the bytes are marked INTERPRETATION here and collected in
README.md.

Requires: Python 3, `cryptography` (for Ed25519). ML-DSA-65 values are not
computed — no implementation was available — so post-quantum signature slots
carry their exact signing input (`Sig_structure`) and a placeholder marker.
"""

import hashlib
from cryptography.hazmat.primitives.asymmetric.ed25519 import Ed25519PrivateKey
from cryptography.hazmat.primitives.serialization import Encoding, PublicFormat

H = lambda b: hashlib.sha256(b).digest()

# ---------------------------------------------------------------- CBOR encoder
# Deterministic encoding per RFC 8949 §4.2 as profiled by wire-format.md §1:
# shortest-form integers, definite lengths only, map keys sorted by the
# bytewise lexicographic order of their encoded form.

def head(major, n):
    if n < 24:
        return bytes([major << 5 | n])
    if n < 0x100:
        return bytes([major << 5 | 24, n])
    if n < 0x10000:
        return bytes([major << 5 | 25]) + n.to_bytes(2, 'big')
    if n < 0x100000000:
        return bytes([major << 5 | 26]) + n.to_bytes(4, 'big')
    return bytes([major << 5 | 27]) + n.to_bytes(8, 'big')

def e_uint(n):  return head(0, n)
def e_int(n):   return head(0, n) if n >= 0 else head(1, -1 - n)
def e_bstr(b):  return head(2, len(b)) + b
def e_tstr(s):  b = s.encode(); return head(3, len(b)) + b
def e_arr(items):  # items are pre-encoded byte strings
    return head(4, len(items)) + b''.join(items)
def e_map(pairs):  # pairs of (encoded_key, encoded_value)
    return head(5, len(pairs)) + b''.join(k + v for k, v in sorted(pairs))

NULL = b'\xf6'

# ---------------------------------------------------------------- identities
# Test identities are synthetic and deterministic. Ed25519 keys are real
# (seed = SHA-256 of the labelled string); ML-DSA-65 public keys are
# structurally valid byte strings of the correct length (1,952 bytes) expanded
# from a labelled hash stream, and are NOT valid lattice keys. Encoding and
# hashing vectors do not depend on key validity; signature vectors that would
# need a real ML-DSA key are marked as requiring an implementation.

def ed25519_seed(name):
    return H(b'rhtn-test-vectors:' + name.encode() + b':ed25519-seed')

def mldsa_pub(name):
    out = b''
    i = 0
    while len(out) < 1952:
        out += H(b'rhtn-test-vectors:' + name.encode() + b':ml-dsa-65-pub:' + str(i).encode())
        i += 1
    return out[:1952]

class Identity:
    def __init__(self, name):
        self.name = name
        self.seed = ed25519_seed(name)
        self.sk = Ed25519PrivateKey.from_private_bytes(self.seed)
        self.ed_pub = self.sk.public_key().public_bytes(Encoding.Raw, PublicFormat.Raw)
        self.pq_pub = mldsa_pub(name)
        # COSE_Key, classical: {1: 1 (kty OKP), -1: 6 (crv Ed25519), -2: x}
        # exactly three labels (wire §2.2)
        self.cose_ed = e_map([(e_int(1), e_uint(1)),
                              (e_int(-1), e_uint(6)),
                              (e_int(-2), e_bstr(self.ed_pub))])
        # COSE_Key, post-quantum: {1: 7 (kty AKP), 3: -49 (alg), -1: pub}
        self.cose_pq = e_map([(e_int(1), e_uint(7)),
                              (e_int(3), e_int(-49)),
                              (e_int(-1), e_bstr(self.pq_pub))])
        # KeyMaterial = [classical, post-quantum], fixed order (wire §2.2)
        self.key_material = e_arr([self.cose_ed, self.cose_pq])
        self.keyhash = H(self.key_material)
    def sign(self, data):
        return self.sk.sign(data)

IDS = {n: Identity(n) for n in
       ['alice', 'bob', 'carol', 'w1', 'w2', 'w3', 'c1', 'c2', 'c3', 'c4', 'c5']}

# ---------------------------------------------------------------- COSE pieces

AAD_ENVELOPE = b'rhtn/1:envelope'
AAD_LOCATOR = b'rhtn/1:locator'

def sig_protected(alg, kid=None):
    """Protected header of one COSE_Signature: {1: alg} plus {4: kid} on
    envelope entries (wire §3.5)."""
    pairs = [(e_int(1), e_int(alg))]
    if kid is not None:
        pairs.append((e_int(4), e_bstr(kid)))
    return e_map(pairs)

def sig_structure_sign(sign_protected_bytes, external_aad, payload):
    """RFC 9052 Sig_structure for a COSE_Sign entry. The outer COSE_Sign
    protected header is empty (wire §3.5), so body_protected is the empty
    byte string."""
    return e_arr([e_tstr('Signature'),
                  e_bstr(b''),
                  e_bstr(sign_protected_bytes),
                  e_bstr(external_aad),
                  e_bstr(payload)])

def sig_structure_sign1(protected_bytes, external_aad, payload):
    return e_arr([e_tstr('Signature1'),
                  e_bstr(protected_bytes),
                  e_bstr(external_aad),
                  e_bstr(payload)])

def cose_signature_entry(protected_bytes, signature):
    """[protected: bstr, unprotected: {} (empty map — nonempty is malformed,
    wire §3.5), signature: bstr]"""
    return e_arr([e_bstr(protected_bytes), b'\xa0', e_bstr(signature)])

MLDSA_PLACEHOLDER = b'\x00' * 3309  # marks the slot; a real value needs an implementation

def envelope(version, msg_type, body_bytes, signers):
    """signers: list of Identity, already the type's logical signer set.
    Entries sort by kid, then classical before post-quantum (wire §3.5)."""
    entries = []
    for ident in sorted(signers, key=lambda i: i.keyhash):
        for alg in (-8, -49):
            prot = sig_protected(alg, ident.keyhash)
            tbs = sig_structure_sign(prot, AAD_ENVELOPE, body_bytes)
            sig = ident.sign(tbs) if alg == -8 else MLDSA_PLACEHOLDER
            entries.append((ident, alg, prot, tbs, sig))
    cose_sign = e_arr([e_bstr(b''), b'\xa0', NULL,
                       e_arr([cose_signature_entry(p, s) for _, _, p, _, s in entries])])
    env = e_map([(e_uint(1), e_uint(version)),
                 (e_uint(2), e_uint(msg_type)),
                 (e_uint(3), body_bytes),
                 (e_uint(4), cose_sign)])
    return env, entries

# ---------------------------------------------------------------- primitives

def path(nibbles):
    packed = bytearray()
    for i in range(0, len(nibbles) - 1, 2):
        packed.append(nibbles[i] << 4 | nibbles[i + 1])
    if len(nibbles) % 2:
        packed.append(nibbles[-1] << 4)  # unused low nibble MUST be zero (wire §2.1)
    return e_map([(e_uint(1), e_bstr(bytes(packed))), (e_uint(2), e_uint(len(nibbles)))])

def seqno(series, counter):
    return e_arr([e_uint(series), e_uint(counter)])

def locator(anchor_kh, path_bytes, seqno_bytes):
    return e_map([(e_uint(1), e_bstr(anchor_kh)),
                  (e_uint(2), path_bytes),
                  (e_uint(3), seqno_bytes)])

def genesis(keyhash):
    """INTERPRETATION: SHA-256 over the raw 32 keyhash bytes, not over a CBOR
    encoding of them (wire §3.1 says only 'SHA-256(the signer's keyhash)')."""
    return H(keyhash)

def backptrs(*lists):
    return e_arr([e_arr([e_bstr(h) for h in one]) for one in lists])

# ---------------------------------------------------------------- formatting

def hexblock(b, width=64):
    h = b.hex()
    return '\n'.join(h[i:i + width] for i in range(0, len(h), width))

def hx(b):
    return b.hex()

OUT = {}
def emit(fname, text):
    OUT.setdefault(fname, []).append(text)

# ================================================================ keys.md

alice, bob, carol = IDS['alice'], IDS['bob'], IDS['carol']

emit('keys.md', f"""# Test identities

**Draft. Spec-derived, unverified by an implementation.** Derivation rules and
status are in [README.md](README.md); regenerate with `tools/generate.py`.

Every identity is synthetic and deterministic:

- **Ed25519**: real keys. `seed = SHA-256("rhtn-test-vectors:<name>:ed25519-seed")`,
  public key derived per RFC 8032.
- **ML-DSA-65**: `pub` is 1,952 structurally valid bytes expanded from
  `SHA-256("rhtn-test-vectors:<name>:ml-dsa-65-pub:<i>")` for i = 0, 1, … —
  **not a valid lattice key**. Encoding and hashing vectors do not depend on key
  validity; anything needing a real ML-DSA signature is marked.

Per `wire-format.md` §2.2, `KeyMaterial = [COSE_Key, COSE_Key]` in fixed order
classical-then-post-quantum, each key carrying exactly three labels, and
`keyhash = SHA-256(deterministic CBOR of KeyMaterial)` (§2).

The classical `COSE_Key` is `{{1: 1, -1: 6, -2: x}}`; deterministic map order
sorts by the bytewise order of the **encoded** keys (`0x01` < `0x20` < `0x21`),
so the entries appear as 1, −1, −2. The post-quantum `COSE_Key` is
`{{1: 7, 3: -49, -1: pub}}`, appearing as 1, 3, −1.

| Identity | Role in the vectors | Ed25519 public key | keyhash |
|---|---|---|---|""")

for name in ['alice', 'bob', 'carol', 'w1', 'w2', 'w3', 'c1', 'c2', 'c3', 'c4', 'c5']:
    i = IDS[name]
    role = {'alice': 'node / subject', 'bob': 'patron', 'carol': 'counterparty',
            'w1': 'witness', 'w2': 'witness', 'w3': 'witness'}.get(name, 'verifier candidate')
    emit('keys.md', f"| {name} | {role} | `{hx(i.ed_pub)}` | `{hx(i.keyhash)}` |")

emit('keys.md', f"""
## Worked example: alice

Ed25519 seed (private key bytes):

```
{hx(alice.seed)}
```

Classical `COSE_Key`, deterministic CBOR ({len(alice.cose_ed)} bytes):

```
{hexblock(alice.cose_ed)}
```

Byte-level reading: `a3` map(3) · `01 01` kty: OKP · `20 06` crv: Ed25519 ·
`21 58 20 …` x: 32-byte public key.

Post-quantum `COSE_Key` ({len(alice.cose_pq)} bytes): `a3` map(3) · `01 07`
kty: AKP · `03 38 30` alg: −49 · `20 59 07 a0 …` pub: 1,952 bytes. The `pub`
bytes are in the appendix.

`KeyMaterial` is the two-element array `82` followed by both keys
({len(alice.key_material)} bytes); its SHA-256 is the keyhash in the table.

## Appendix: alice's synthetic ML-DSA-65 `pub`

```
{hexblock(alice.pq_pub)}
```

Other identities' `pub` bytes follow the same derivation and are regenerable
from the script.""")

# ================================================================ primitives.md

TS_2026 = 1767225600  # 2026-01-01T00:00:00Z

p_odd = path([3, 1, 4, 1, 5])
p_even = path([3, 1, 4, 1])
sq = seqno(5, 42)
sq_max = seqno(5, 0xFFFFFFFF)
loc = locator(bob.keyhash, p_odd, sq)

# SignedLocator: COSE_Sign1 by the subject over canonical CBOR of fields 1-2,
# external_aad "rhtn/1:locator", classical only, no kid (embedded/named signer).
sl_payload = e_map([(e_uint(1), e_bstr(alice.keyhash)), (e_uint(2), loc)])
sl_protected = sig_protected(-8)
sl_tbs = sig_structure_sign1(sl_protected, AAD_LOCATOR, sl_payload)
sl_sig = alice.sign(sl_tbs)
sl_cose = e_arr([e_bstr(sl_protected), b'\xa0', NULL, e_bstr(sl_sig)])
signed_locator = e_map([(e_uint(1), e_bstr(alice.keyhash)), (e_uint(2), loc),
                        (e_uint(3), sl_cose)])

emit('primitives.md', f"""# Primitives

**Draft. Spec-derived, unverified by an implementation.** See
[README.md](README.md).

## Deterministic CBOR atoms (`wire-format.md` §1)

Shortest-form unsigned integers at every width boundary:

| Value | Encoding |
|---|---|
| 0 | `00` |
| 23 | `17` |
| 24 | `1818` |
| 255 | `18ff` |
| 256 | `190100` |
| 65535 | `19ffff` |
| 65536 | `1a00010000` |
| 4294967295 | `1affffffff` |
| 4294967296 | `1b0000000100000000` |

A 2026 timestamp encodes in five bytes, as §2 claims:
`{TS_2026}` (2026-01-01T00:00:00Z) → `{hx(e_uint(TS_2026))}`.

Map keys sort ascending (§1): `{{1: 0, 2: 0, 10: 0}}` →
`{hx(e_map([(e_uint(1), e_uint(0)), (e_uint(2), e_uint(0)), (e_uint(10), e_uint(0))]))}`.

## seqno (§2, §2.3)

`[series, counter]`, two shortest-form uints — never a single integer.

| seqno | Encoding |
|---|---|
| `[5, 42]` | `{hx(sq)}` |
| `[5, 4294967295]` (sealed: counter at u32 max) | `{hx(sq_max)}` |

## Path (§2.1)

Nibbles pack high-first; the byte string is exactly `ceil(n/2)` bytes; on an
odd count the final low nibble is zero.

| Path | Encoding |
|---|---|
| nibbles `3,1,4,1,5` (odd) | `{hx(p_odd)}` |
| nibbles `3,1,4,1` (even) | `{hx(p_even)}` |

Reading the odd case: `a2` map(2) · `01 43 314150` packed bytes (`50`: nibble 5
then the mandatory zero) · `02 05` length **in nibbles**.

## Locator (§2.3)

`{{1: anchor keyhash, 2: path, 3: seqno}}` — anchor is bob, path is the odd
example, seqno `[5, 42]` ({len(loc)} bytes):

```
{hexblock(loc)}
```

## SignedLocator (§2.3) — a complete classical signature, end to end

The one fully computable signature vector in this draft: classical-only by
profile, so no ML-DSA slot.

Payload — deterministic CBOR of fields 1–2 (INTERPRETATION 1, README):

```
{hexblock(sl_payload)}
```

Protected header `{{1: -8}}` → `{hx(sl_protected)}` (no `kid`: the surrounding
structure names the signer, wire §3.5).

`Sig_structure` (`["Signature1", protected, external_aad = "rhtn/1:locator",
payload]`):

```
{hexblock(sl_tbs)}
```

Ed25519 signature by alice over those bytes:

```
{hexblock(sl_sig)}
```

Complete `SignedLocator` ({len(signed_locator)} bytes) — field 3 is the untagged
`COSE_Sign1` `[protected, {{}}, null, signature]` with detached payload:

```
{hexblock(signed_locator)}
```

## Genesis back-pointer (§3.1)

`SHA-256(the signer's keyhash)` — over the raw 32 bytes (INTERPRETATION 2,
README).

| Signer | Genesis value |
|---|---|
| alice | `{hx(genesis(alice.keyhash))}` |
| bob | `{hx(genesis(bob.keyhash))}` |
| carol | `{hx(genesis(carol.keyhash))}` |""")

# ================================================================ transactions.md

# --- Adoption: alice adopted by bob, both at genesis.
TS_ADOPT = TS_2026 + 100 * 86400 + 3600
adopt_body = e_map([
    (e_uint(0), backptrs([genesis(alice.keyhash)], [genesis(bob.keyhash)])),
    (e_uint(1), e_bstr(alice.keyhash)),
    (e_uint(2), e_bstr(bob.keyhash)),
    (e_uint(3), loc),
    (e_uint(4), e_uint(TS_ADOPT)),
])
adopt_txid = H(adopt_body)
adopt_env, adopt_entries = envelope(1, 1, adopt_body, [alice, bob])

# --- Departure: alice leaves bob.
TS_DEPART = TS_ADOPT + 30 * 86400
depart_prev = adopt_txid  # alice's chain: genesis -> adoption -> departure
depart_body = e_map([
    (e_uint(0), backptrs([depart_prev])),
    (e_uint(1), e_bstr(alice.keyhash)),
    (e_uint(2), e_bstr(bob.keyhash)),
    (e_uint(3), seqno(5, 43)),
    (e_uint(4), e_uint(TS_DEPART)),
])
depart_txid = H(depart_body)

# --- Disavowal: bob disavows alice, reason 4.
disavow_body = e_map([
    (e_uint(0), backptrs([adopt_txid])),
    (e_uint(1), e_bstr(bob.keyhash)),
    (e_uint(2), e_bstr(alice.keyhash)),
    (e_uint(3), e_uint(TS_DEPART)),
    (e_uint(4), e_uint(4)),
])
disavow_txid = H(disavow_body)

# --- Series reissue: alice seals series 5 at max, opens series 3735928559.
# Alternative continuation of the adoption (not of the departure): both
# signers' chain heads are the adoption itself.
reissue_body = e_map([
    (e_uint(0), backptrs([adopt_txid], [adopt_txid])),
    (e_uint(1), e_bstr(alice.keyhash)),
    (e_uint(2), e_bstr(bob.keyhash)),
    (e_uint(3), seqno(5, 0xFFFFFFFF)),
    (e_uint(4), seqno(0xDEADBEEF, 0)),
    (e_uint(5), e_uint(TS_DEPART + 86400)),
])
reissue_txid = H(reissue_body)

# --- Presence, formation subtype: alice and carol.
TS_START = TS_2026 + 12 * 3600
TS_FINAL = TS_START + 900
WINDOW_ORDINAL = TS_START // 86400
synthetic_root = H(b'rhtn-test-vectors:synthetic-disclosure-root:formation')
participant = lambda i: e_map([(e_uint(1), e_bstr(i.keyhash))])
formation_body = e_map([
    (e_uint(0), backptrs([genesis(alice.keyhash)], [genesis(carol.keyhash)])),
    (e_uint(1), e_uint(TS_START)),
    (e_uint(2), e_uint(TS_FINAL)),
    (e_uint(3), e_arr([participant(alice), participant(carol)])),
    (e_uint(6), e_uint(1)),
    (e_uint(7), e_uint(WINDOW_ORDINAL)),
    (e_uint(8), e_bstr(synthetic_root)),
])
formation_txid = H(formation_body)

alice_first, bob_first = sorted([alice, bob], key=lambda i: i.keyhash)
ordered_names = [alice_first.name, bob_first.name]

emit('transactions.md', f"""# Transaction bodies, txids, and one full envelope

**Draft. Spec-derived, unverified by an implementation.** See
[README.md](README.md).

`txid = SHA-256(deterministic CBOR of the body map)`, signature array excluded
(`wire-format.md` §1). Key 0 carries one back-pointer list per required signer
in signer order (§3.1). **Series reissue is absent from §3.1's signer-order
table** — the order used here, node then patron, follows the table's own
generative rule (*the order the type's schema introduces its required
signers*); the missing row is FINDING 1 in README.

Timestamps: adoption `{TS_ADOPT}`, departure `{TS_DEPART}`, ceremony
`started_at {TS_START}` / `finalized_at {TS_FINAL}`.

**The departure, disavowal and series reissue are three alternative
continuations of the adoption, not one chain** — each names the adoption as its
signer's predecessor, so the same worked chain state serves all three.

## Adoption (type 1) — alice adopted by bob, both chains at genesis

Body — `{{0: [[g(alice)], [g(bob)]], 1: alice, 2: bob, 3: Locator, 4: ts}}`,
minimal: no `KeyMaterial`, no `Recovery`, no archive head, no
proof-of-presence reference ({len(adopt_body)} bytes):

```
{hexblock(adopt_body)}
```

txid: `{hx(adopt_txid)}`

### The full envelope

`{{1: 1, 2: 1, 3: body, 4: COSE_Sign}}`. The `COSE_Sign` is untagged with empty
outer protected header (`40`), empty unprotected map (`a0`), detached payload
(`f6`), and four `COSE_Signature` entries — two logical signers × (classical +
post-quantum), sorted by `kid` then classical first (§3.5). Bytewise,
{ordered_names[0]}'s keyhash sorts before {ordered_names[1]}'s, so the entry
order is {ordered_names[0]}/−8, {ordered_names[0]}/−49, {ordered_names[1]}/−8,
{ordered_names[1]}/−49.

Each entry's protected header is `{{1: alg, 4: kid}}`; each entry's
`Sig_structure` is `["Signature", h'', protected, "rhtn/1:envelope", body]`.
""")

for ident, alg, prot, tbs, sig in adopt_entries:
    algname = 'Ed25519 (−8)' if alg == -8 else 'ML-DSA-65 (−49)'
    emit('transactions.md', f"""#### Entry: {ident.name}, {algname}

Protected header: `{hx(prot)}`

`Sig_structure` (SHA-256 `{hx(H(tbs))}`, full bytes {len(tbs)}):

```
{hexblock(tbs)}
```
""")
    if alg == -8:
        emit('transactions.md', f"""Signature:

```
{hexblock(sig)}
```
""")
    else:
        emit('transactions.md',
             "Signature: **requires an ML-DSA-65 implementation** (deterministic"
             " variant, §2.2). The 3,309-byte slot is zero-filled in the envelope"
             " bytes below, which are therefore STRUCTURAL, not final.\n")

emit('transactions.md', f"""#### Envelope bytes (structural — PQ slots zero-filled)

{len(adopt_env)} bytes:

```
{hexblock(adopt_env)}
```

## Departure (type 2) — alice leaves bob

Single signer. Alice's back-pointer is the adoption's txid.
Body ({len(depart_body)} bytes):

```
{hexblock(depart_body)}
```

txid: `{hx(depart_txid)}`

## Disavowal (type 3) — bob disavows alice, reason code 4

Single signer; bob's back-pointer is the adoption's txid. Body
({len(disavow_body)} bytes):

```
{hexblock(disavow_body)}
```

txid: `{hx(disavow_txid)}`

## Series reissue (type 7) — seal series 5, open series 3735928559

Field 3 records the sealed series at the counter it reached (u32 max — the
seal-then-reissue pattern of §4.6); field 4 opens the new series at counter 0.
The new series value is arbitrary by design (§2.3) — `0xDEADBEEF` here, chosen
to look arbitrary. Body ({len(reissue_body)} bytes):

```
{hexblock(reissue_body)}
```

txid: `{hx(reissue_txid)}`

## Presence record (type 5), formation subtype — alice and carol

Keys 4 and 5 are **omitted entirely** (§3.2: absence means empty); key 6 is 1;
key 0 carries the genesis value for both signers, which for a formation record
is also a structural rule (§3.2). `window_ordinal = floor({TS_START} / 86400) =
{WINDOW_ORDINAL}`. Key 8's disclosure root is **synthetic**
(`SHA-256("rhtn-test-vectors:synthetic-disclosure-root:formation")`) — the
§4.5.1 digest-list construction is not yet covered by this draft (README,
*Not yet covered*).

Body ({len(formation_body)} bytes):

```
{hexblock(formation_body)}
```

txid: `{hx(formation_txid)}`""")

# ================================================================ verifier-selection.md

def commitment(w, nonce):
    return H(b'rhtn/1:nonce-commit' + w.keyhash + nonce)

nonces = {n: H(b'rhtn-test-vectors:nonce:' + n.encode()) for n in ['w1', 'w2', 'w3']}
witnesses_sorted = sorted(['w1', 'w2', 'w3'], key=lambda n: IDS[n].keyhash)
pa, pb = sorted([alice.keyhash, carol.keyhash])
seed_input = (b'rhtn/1:verifier-seed' + pa + pb + WINDOW_ORDINAL.to_bytes(8, 'big')
              + b''.join(IDS[n].keyhash + nonces[n] for n in witnesses_sorted))
seed = H(seed_input)

cands = ['c1', 'c2', 'c3', 'c4', 'c5']
ranks = {c: H(seed + alice.keyhash + IDS[c].keyhash) for c in cands}
ranked = sorted(cands, key=lambda c: (ranks[c], IDS[c].keyhash))
n_example = 7
required = min(n_example // 2, 10, len(cands))
selected = ranked[:required]

emit('verifier-selection.md', f"""# Verifier selection — recomputation

**Draft. Spec-derived, unverified by an implementation.** See
[README.md](README.md). All inputs are raw byte concatenations hashed with
SHA-256 (`wire-format.md` §5) — no CBOR wrapping anywhere in this file
(INTERPRETATION 3, README).

## Nonce commitment (§5.1)

`commitment = SHA-256("rhtn/1:nonce-commit" || witness_keyhash || nonce)`,
nonce exactly 32 bytes. Test nonces are
`SHA-256("rhtn-test-vectors:nonce:<name>")`.

| Witness | Nonce | Commitment |
|---|---|---|""")
for n in ['w1', 'w2', 'w3']:
    emit('verifier-selection.md',
         f"| {n} | `{hx(nonces[n])}` | `{hx(commitment(IDS[n], nonces[n]))}` |")

emit('verifier-selection.md', f"""
## Seed (§5.3)

Ceremony: participants alice and carol, `started_at = {TS_START}`, so
`window_ordinal = floor({TS_START} / 86400) = {WINDOW_ORDINAL}`, encoded as 8
bytes big-endian: `{hx(WINDOW_ORDINAL.to_bytes(8, 'big'))}`.

Participants canonicalise bytewise: min is
{'alice' if pa == alice.keyhash else 'carol'}, max is
{'carol' if pa == alice.keyhash else 'alice'}. Witnesses in ascending keyhash
order: {', '.join(witnesses_sorted)}.

Seed preimage ({len(seed_input)} bytes = 20-byte tag + 32 + 32 + 8 + 3 × 64):

```
{hexblock(seed_input)}
```

seed: `{hx(seed)}`

## Threshold (§5.4)

`required(subject) = min(floor(n / 2), 10, |candidates|)`:

| n | candidates | required |
|---|---|---|
| 0 | 0 | 0 |
| 1 | 1 | 0 |
| 2 | 1 | 1 |
| 3 | 2 | 1 |
| 7 | 5 | 3 |
| 20 | 1 | 1 |
| 25 | 12 | 10 |

## Sampling (§5.4)

`rank(c) = SHA-256(seed || subject || c)`, subject alice, candidates c1–c5;
ascending rank, ties by ascending keyhash.

| Candidate | rank |
|---|---|""")
for c in cands:
    emit('verifier-selection.md', f"| {c} | `{hx(ranks[c])}` |")
emit('verifier-selection.md', f"""
Rank order: {' < '.join(ranked)}.

With n = {n_example} and these five candidates, `required = {required}`;
**selected: {', '.join(selected)}**. Exactly that many are queried (§5.4).

## Window boundaries (§5.3.1)

With `started_at = {TS_START}`, the 730-day window is
`{TS_START} − 63072000 = {TS_START - 63072000} < finalized_at < {TS_START}` —
open at the far end, closed at the near end:

| Prior record `finalized_at` | Counted? |
|---|---|
| {TS_START - 63072000} (exactly 730 days) | no — boundary instant is out |
| {TS_START - 63072000 + 1} | yes |
| {TS_START - 1} | yes |
| {TS_START} | no — not before `started_at` |""")

# ---------------------------------------------------------------- write files

import os
os.chdir(os.path.join(os.path.dirname(os.path.abspath(__file__)), '..'))
for fname, parts in OUT.items():
    with open(fname, 'w') as f:
        f.write('\n'.join(parts) + '\n')
    print(f"wrote {fname}")
