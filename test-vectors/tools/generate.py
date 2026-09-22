#!/usr/bin/env python3
"""Generate the draft canonical test vectors for the RHTN wire format.

Everything computable is computed; nothing is hand-transcribed. Re-running this
script regenerates keys.md, primitives.md, transactions.md, records.md,
messages.md and verifier-selection.md byte-for-byte, each pinned to the SHA-256 of both
`network-design.md` and `wire-format.md`. README.md and negative-vectors.md are
authored by hand.

Status: DRAFT, derived from the specifications and verified by no
independent RHTN implementation.

Requires: Python 3, `cryptography` (Ed25519) and `dilithium-py` (ML-DSA-65).
All signatures are real. The ML-DSA keygen recipe, so a second implementation
derives identical keys: xi = SHA-256("rhtn-test-vectors:<name>:ml-dsa-65-seed"),
keypair = FIPS 204 ML-DSA-65.KeyGen_internal(xi) (dilithium-py's key_derive),
signing deterministic with empty context. Cross-checked at generation time
against a second, independent ML-DSA implementation where available.
"""

import hashlib, hmac, os
from cryptography.hazmat.primitives.asymmetric.ed25519 import Ed25519PrivateKey
from cryptography.hazmat.primitives.serialization import Encoding, PublicFormat
try:
    from dilithium_py.ml_dsa import ML_DSA_65
except ImportError:
    raise SystemExit('dilithium-py is required for the ML-DSA-65 values: '
                     'pip install dilithium-py (any environment this '
                     'interpreter can import from)')

H = lambda b: hashlib.sha256(b).digest()

_ROOT = os.path.join(os.path.dirname(os.path.abspath(__file__)), '..', '..')
def _sha(name):
    return hashlib.sha256(open(os.path.join(_ROOT, name), 'rb').read()).hexdigest()
WIRE_SHA = _sha('wire-format.md')
DESIGN_SHA = _sha('network-design.md')
LIGHT_SHA = _sha('light-client-requirements.md')

# --- The pin gate. A changed specification hash means the generator's
# hard-coded constructions may encode stale semantics: refuse to stamp new
# hashes onto old assumptions unless the change is explicitly acknowledged
# with --accept-spec-change, which records the new hashes after (presumably)
# a human audited the constructions against the diff.
import json, sys
_PINS = os.path.join(os.path.dirname(os.path.abspath(__file__)), 'spec-pins.json')
_current = {'wire-format.md': WIRE_SHA, 'network-design.md': DESIGN_SHA,
            'light-client-requirements.md': LIGHT_SHA}
GEN_SHA = hashlib.sha256(open(os.path.abspath(__file__), 'rb').read()).hexdigest()
_VERIFY = os.path.join(os.path.dirname(os.path.abspath(__file__)), 'verify.py')
VER_SHA = (hashlib.sha256(open(_VERIFY, 'rb').read()).hexdigest()
           if os.path.exists(_VERIFY) else None)
if os.path.exists(_PINS):
    _stored = json.load(open(_PINS, encoding='utf-8'))
    if 'specs' in _stored:
        _stored_specs, _stored_gen = _stored['specs'], _stored.get('generator')
        _stored_ver = _stored.get('verifier')
    else:  # legacy flat format
        _stored_specs = {k: v for k, v in _stored.items() if k != 'tools/generate.py'}
        _stored_gen = _stored.get('tools/generate.py'); _stored_ver = None
    if _stored_specs != _current and '--accept-spec-change' not in sys.argv:
        sys.exit('spec-pins.json does not match the current specifications.\n'
                 'Accepting a specification change asserts BOTH audits: the '
                 'generator constructions AND the hand-authored fixture '
                 'semantics in README.md and negative-vectors.md, whose pin '
                 'lines this run will refresh. Rerun with --accept-spec-change '
                 'once both are done.')
    if (_stored_gen != GEN_SHA or (_stored_ver is not None and _stored_ver != VER_SHA)) \
            and '--accept-generator-change' not in sys.argv \
            and '--accept-spec-change' not in sys.argv:
        sys.exit('the generator or the verification harness has changed since '
                 'spec-pins.json was written. A tool-only change can alter every '
                 'vector, or alter what "all checks pass" means, with the spec '
                 'pins green: rerun with --accept-generator-change after '
                 'reviewing the tool diff.')
elif '--bootstrap-pins' not in sys.argv:
    sys.exit('spec-pins.json is missing. A missing pin file is not a clean '
             'slate — regenerating would silently baseline unaudited '
             'specifications. Restore it from version control, or rerun with '
             '--bootstrap-pins to deliberately establish a new baseline.')

def _dep_versions():
    from importlib import metadata
    out = {}
    for d in ('dilithium-py', 'cryptography'):
        try: out[d] = metadata.version(d)
        except Exception: out[d] = 'unknown'
    return out

def _write_pins(output_hashes):
    json.dump({'specs': _current, 'generator': GEN_SHA, 'verifier': VER_SHA,
               'deps': _dep_versions(), 'outputs': output_hashes},
              open(_PINS, 'w', encoding='utf-8', newline='\n'), indent=1)

_ACCEPTED = any(f in sys.argv for f in
                ('--accept-spec-change', '--accept-generator-change',
                 '--accept-output-change', '--bootstrap-pins'))
_STORED_OUTPUTS = (_stored.get('outputs', {}) if os.path.exists(_PINS) else {})

def _repin_hand_file(name):
    """The hand-authored files carry one machine-managed pin line so staleness
    is mechanically detectable there too."""
    path = os.path.join(os.path.dirname(os.path.abspath(__file__)), '..', name)
    text = open(path, encoding='utf-8').read()
    marker = '**Pinned**: '
    line = (marker + 'wire-format.md `' + WIRE_SHA + '` · network-design.md `'
            + DESIGN_SHA + '`')
    out = []
    hit = False
    for l in text.split('\n'):
        if l.startswith(marker):
            out.append(line); hit = True
        else:
            out.append(l)
    if not hit:
        sys.exit(name + ' has no "**Pinned**:" line to manage.')
    open(path, 'w', encoding='utf-8', newline='\n').write('\n'.join(out))
PIN = ("Generated against `wire-format.md` `" + WIRE_SHA[:16] + "…`, "
       "`network-design.md` `" + DESIGN_SHA[:16] + "…` and "
       "`light-client-requirements.md` `" + LIGHT_SHA[:16] + "…` (full hashes, "
       "producer and output hashes in `tools/spec-pins.json`). The design wins "
       "on any disagreement; a change to any pinned document stales these "
       "vectors.")

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
    # RFC 8949 §4.2.1 core deterministic ordering: bytewise lexicographic order
    # of the keys' deterministic encodings. Sorting (key, value) pairs is
    # equivalent because keys are unique, so the key alone decides. This is the
    # CURRENT rule, not RFC 7049's retired length-first order — and it holds
    # for arbitrary key types, though this suite only exercises uint keys and
    # the fixed negative labels of the COSE structures.
    keys = [k for k, _ in pairs]
    assert len(keys) == len(set(keys)), 'duplicate map key in a positive vector'
    return head(5, len(pairs)) + b''.join(k + v for k, v in sorted(pairs))

NULL = b'\xf6'

# ---------------------------------------------------------------- identities
# Test identities are synthetic, deterministic, and REAL for both components:
# Ed25519 from seed = SHA-256("rhtn-test-vectors:<name>:ed25519-seed"), and
# ML-DSA-65 from xi = SHA-256("rhtn-test-vectors:<name>:ml-dsa-65-seed") via
# FIPS 204 KeyGen_internal. Signing is the deterministic variant with empty
# context, per wire §2.2.

def ed25519_seed(name):
    return H(b'rhtn-test-vectors:' + name.encode() + b':ed25519-seed')

def mldsa_seed(name):
    return H(b'rhtn-test-vectors:' + name.encode() + b':ml-dsa-65-seed')

class Identity:
    def __init__(self, name):
        self.name = name
        self.seed = ed25519_seed(name)
        self.sk = Ed25519PrivateKey.from_private_bytes(self.seed)
        self.ed_pub = self.sk.public_key().public_bytes(Encoding.Raw, PublicFormat.Raw)
        self.pq_seed = mldsa_seed(name)
        self.pq_pub, self.pq_sk = ML_DSA_65.key_derive(self.pq_seed)
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
    def sign_pq(self, data):
        return ML_DSA_65.sign(self.pq_sk, data, deterministic=True)

IDS = {n: Identity(n) for n in
       ['alice', 'bob', 'carol', 'alice2'] + [f'w{i}' for i in range(1, 17)]
       + ['c1', 'c2', 'c3', 'c4', 'c5']}

class TransportKey:
    """A device's transport key (wire §8.2): a raw Ed25519 key with no keyhash,
    minted on the device and named by a delegation the identity signs.  It
    is also what names a device (wire §7.8).  Seed recipe as for the
    identities' classical halves, under its own label."""
    def __init__(self, name):
        self.name = name
        self.seed = H(f'rhtn-test-vectors:{name}:transport-seed'.encode())
        self.sk = Ed25519PrivateKey.from_private_bytes(self.seed)
        self.pub = self.sk.public_key().public_bytes(Encoding.Raw, PublicFormat.Raw)
    def sign(self, data):
        return self.sk.sign(data)

TK = {n: TransportKey(n) for n in ['bob-instance', 'carol-instance', 'alice-desktop']}

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

SIGNER_COUNT = {1: 2, 2: 1, 3: 1, 4: 2, 7: 2}  # per-type logical signers (§3.1)
# type 5's signer set is dynamic (participants + witnesses); callers assert it

def envelope(version, msg_type, body_bytes, signers):
    """signers: the type's logical signer set — asserted against §3.1's
    per-type count and for distinctness. Entries sort by kid, then classical
    before post-quantum (wire §3.5)."""
    if msg_type in SIGNER_COUNT:
        assert len(signers) == SIGNER_COUNT[msg_type], 'wrong signer count for type'
    assert len({i.keyhash for i in signers}) == len(signers), 'duplicate signer'
    entries = []
    for ident in sorted(signers, key=lambda i: i.keyhash):
        for alg in (-8, -49):
            prot = sig_protected(alg, ident.keyhash)
            tbs = sig_structure_sign(prot, AAD_ENVELOPE, body_bytes)
            sig = ident.sign(tbs) if alg == -8 else ident.sign_pq(tbs)
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
    assert 0 <= len(nibbles) <= 24 and all(0 <= n <= 9 for n in nibbles), \
        'path: nibbles 0-9, length 0-24 — empty is the self-anchor case (wire §2.1)'
    packed = bytearray()
    for i in range(0, len(nibbles) - 1, 2):
        packed.append(nibbles[i] << 4 | nibbles[i + 1])
    if len(nibbles) % 2:
        packed.append(nibbles[-1] << 4)  # unused low nibble MUST be zero (wire §2.1)
    return e_map([(e_uint(1), e_bstr(bytes(packed))), (e_uint(2), e_uint(len(nibbles)))])

def seqno(series, counter):
    assert 0 <= series < 2**32 and 0 <= counter < 2**32, 'seqno: u32 ranges (§2.3)'
    return e_arr([e_uint(series), e_uint(counter)])

def locator(anchor_kh, path_bytes, seqno_bytes):
    return e_map([(e_uint(1), e_bstr(anchor_kh)),
                  (e_uint(2), path_bytes),
                  (e_uint(3), seqno_bytes)])

def genesis(keyhash):
    """Over the raw 32 keyhash bytes, not a CBOR encoding of them (wire §3.1,
    stated since 2026-09-01)."""
    return H(keyhash)

def backptrs(*lists):
    assert lists, 'at least one signer list (§3.1)'
    for one in lists:
        assert 1 <= len(one) <= 8 and all(len(h) == 32 for h in one), \
            'back-pointers: 1-8 entries of 32 bytes per signer (§1, §3.1)'
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

alice, bob, carol, alice2 = IDS['alice'], IDS['bob'], IDS['carol'], IDS['alice2']

emit('keys.md', f"""# Test identities

{PIN}

**Draft. Spec-derived, unverified by an implementation.** Derivation rules and
status are in [README.md](README.md); regenerate with `tools/generate.py`.

Every identity is synthetic, deterministic, and **real for both components**:

- **Ed25519**: `seed = SHA-256("rhtn-test-vectors:<name>:ed25519-seed")`,
  public key per RFC 8032.
- **ML-DSA-65**: `xi = SHA-256("rhtn-test-vectors:<name>:ml-dsa-65-seed")`,
  keypair = **FIPS 204 `ML-DSA-65.KeyGen_internal(xi)`**. Signing is the
  deterministic variant with empty context (wire §2.2). The recipe is
  implementation-independent — at generation time a second, independent
  ML-DSA implementation reproduced the same public keys from the same seeds
  and verified the deterministic signatures.

Per `wire-format.md` §2.2, `KeyMaterial = [COSE_Key, COSE_Key]` in fixed order
classical-then-post-quantum, each key carrying exactly three labels, and
`keyhash = SHA-256(deterministic CBOR of KeyMaterial)` (§2).

The classical `COSE_Key` is `{{1: 1, -1: 6, -2: x}}`; deterministic map order
sorts by the bytewise order of the **encoded** keys (`0x01` < `0x20` < `0x21`),
so the entries appear as 1, −1, −2. The post-quantum `COSE_Key` is
`{{1: 7, 3: -49, -1: pub}}`, appearing as 1, 3, −1.

| Identity | Role in the vectors | Ed25519 public key | keyhash |
|---|---|---|---|""")

for name in (['alice', 'bob', 'carol', 'alice2'] + [f'w{i}' for i in range(1, 17)]
             + ['c1', 'c2', 'c3', 'c4', 'c5']):
    i = IDS[name]
    role = {'alice': 'node / subject', 'bob': 'patron', 'carol': 'counterparty',
            'alice2': "alice's replacement key (recovery adoption)"}.get(
        name, 'witness' if name.startswith('w') else 'verifier candidate')
    emit('keys.md', f"| {name} | {role} | `{hx(i.ed_pub)}` | `{hx(i.keyhash)}` |")

emit('keys.md', f"""
## Transport keys

A device that holds a delegation and no seed presents a raw Ed25519 key in
the handshake, named by a `Delegation` its identity signed (`wire-format.md`
§8.2), and that key is also what names the device (`wire-format.md` §7.8). It
has no keyhash and no post-quantum half. Seed recipe:
`seed = SHA-256("rhtn-test-vectors:<name>:transport-seed")`, public key per
RFC 8032.

| Key | Held by | Raw public key |
|---|---|---|
| bob-instance | bob's infra instance, delegated by bob | `{hx(TK['bob-instance'].pub)}` |
| carol-instance | carol's infra instance, delegated by carol | `{hx(TK['carol-instance'].pub)}` |
| alice-desktop | alice's desktop, a delegated device with prekeys of its own | `{hx(TK['alice-desktop'].pub)}` |

The seed-holding device presents its identity's classical key and needs no
delegation; as a device (`wire-format.md` §7.8) it is named by that key.

## Worked example: alice

Ed25519 seed (private key bytes):

```
{hx(alice.seed)}
```

ML-DSA-65 seed `xi`:

```
{hx(alice.pq_seed)}
```

Classical `COSE_Key`, deterministic CBOR ({len(alice.cose_ed)} bytes):

```
{hexblock(alice.cose_ed)}
```

Byte-level reading: `a3` map(3) · `01 01` kty: OKP · `20 06` crv: Ed25519 ·
`21 58 20 …` x: 32-byte public key.

Post-quantum `COSE_Key` ({len(alice.cose_pq)} bytes): `a3` map(3) · `01 07`
kty: AKP · `03 38 30` alg: −49 · `20 59 07 a0 …` pub: the real 1,952-byte
ML-DSA-65 public key derived from `xi` above, printed in the appendix.

`KeyMaterial` is the two-element array `82` followed by both keys
({len(alice.key_material)} bytes); its SHA-256 is the keyhash in the table.

## Appendix: alice's ML-DSA-65 `pub`

```
{hexblock(alice.pq_pub)}
```

Other identities' keys follow the same derivation and are regenerable from the
script — or from any FIPS 204 implementation exposing seed-based keygen.""")

# ================================================================ primitives.md

TS_2026 = 1767225600  # 2026-01-01T00:00:00Z

p_odd = path([3, 1, 4, 1, 5])
p_even = path([3, 1, 4, 1])
sq = seqno(5, 42)
sq_max = seqno(5, 0xFFFFFFFF)
loc = locator(bob.keyhash, p_odd, sq)
# An adoption's locator opens the relationship's series at counter 0 (§4.1);
# the standalone SignedLocator fixtures above keep sq's later counter values.
adopt_loc = locator(bob.keyhash, p_odd, seqno(5, 0))

# SignedLocator: COSE_Sign1 by the subject over canonical CBOR of fields 1-2,
# external_aad "rhtn/1:locator", classical only, no kid (embedded/named signer).
sl_payload = e_map([(e_uint(1), e_bstr(alice.keyhash)), (e_uint(2), loc)])
sl_protected = sig_protected(-8)
sl_tbs = sig_structure_sign1(sl_protected, AAD_LOCATOR, sl_payload)
sl_sig = alice.sign(sl_tbs)
sl_cose = e_arr([e_bstr(sl_protected), b'\xa0', NULL, e_bstr(sl_sig)])
signed_locator = e_map([(e_uint(1), e_bstr(alice.keyhash)), (e_uint(2), loc),
                        (e_uint(3), sl_cose)])

# --- Same-series counter jump: a second SignedLocator by alice, seqno [5,100].
# Strictly greater, deliberately not previous+1 (§2.3).
loc2 = locator(bob.keyhash, path([3, 1, 4, 1, 5]), seqno(5, 100))
sl2_payload = e_map([(e_uint(1), e_bstr(alice.keyhash)), (e_uint(2), loc2)])
sl2_tbs = sig_structure_sign1(sl_protected, AAD_LOCATOR, sl2_payload)
sl2_sig = alice.sign(sl2_tbs)
sl2 = e_map([(e_uint(1), e_bstr(alice.keyhash)), (e_uint(2), loc2),
             (e_uint(3), e_arr([e_bstr(sl_protected), b'\xa0', NULL, e_bstr(sl2_sig)]))])

# The SignedLocator equal-seqno conflict partner: same subject, same
# [series, counter] as sl2, DIFFERENT path — the locator-path analogue of the
# EndpointRecord conflict pair (V12).
loc2b = locator(bob.keyhash, path([2, 7]), seqno(5, 100))
sl2b_payload = e_map([(e_uint(1), e_bstr(alice.keyhash)), (e_uint(2), loc2b)])
sl2b_sig = alice.sign(sig_structure_sign1(sl_protected, AAD_LOCATOR, sl2b_payload))
sl2b = e_map([(e_uint(1), e_bstr(alice.keyhash)), (e_uint(2), loc2b),
              (e_uint(3), e_arr([e_bstr(sl_protected), b'\xa0', NULL,
                                 e_bstr(sl2b_sig)]))])

# The root's self-anchored locator: bob names himself, empty path (D13).
root_loc = locator(bob.keyhash, path([]), seqno(9, 3))
slr_payload = e_map([(e_uint(1), e_bstr(bob.keyhash)), (e_uint(2), root_loc)])
slr_tbs = sig_structure_sign1(sl_protected, AAD_LOCATOR, slr_payload)
slr_sig = bob.sign(slr_tbs)
sl_root = e_map([(e_uint(1), e_bstr(bob.keyhash)), (e_uint(2), root_loc),
                 (e_uint(3), e_arr([e_bstr(sl_protected), b'\xa0', NULL,
                                    e_bstr(slr_sig)]))])

# Extension-bearing SignedLocator: unknown key 4 — deliberately the nearest
# uint above the signature slot (3), the value that breaks any slot-inference
# heuristic. Payload = fields 1-2 plus the unknown key, per §1's global rule.
slx_payload = e_map([(e_uint(1), e_bstr(alice.keyhash)), (e_uint(2), loc),
                     (e_uint(4), e_bstr(bytes.fromhex('aa')))])
slx_tbs = sig_structure_sign1(sl_protected, AAD_LOCATOR, slx_payload)
slx_sig = alice.sign(slx_tbs)
sl_ext = e_map([(e_uint(1), e_bstr(alice.keyhash)), (e_uint(2), loc),
                (e_uint(3), e_arr([e_bstr(sl_protected), b'\xa0', NULL,
                                   e_bstr(slx_sig)])),
                (e_uint(4), e_bstr(bytes.fromhex('aa')))])

# Wrong-signer negative: bob signs a locator whose field 1 names alice.
sl_wrong_sig = bob.sign(sl_tbs)  # same payload, same tag, wrong key
sl_wrong = e_map([(e_uint(1), e_bstr(alice.keyhash)), (e_uint(2), loc),
                  (e_uint(3), e_arr([e_bstr(sl_protected), b'\xa0', NULL,
                                     e_bstr(sl_wrong_sig)]))])


emit('primitives.md', f"""# Primitives

{PIN}

**Draft. Spec-derived, unverified by an implementation.** See
[README.md](README.md).

## Deterministic CBOR atoms (`wire-format.md` §1)

Shortest-form unsigned integers at every width boundary (rows generated by
the same encoder every vector uses):

| Value | Encoding |
|---|---|
{chr(10).join(f'| {v} | `{e_uint(v).hex()}` |' for v in
              [0, 23, 24, 255, 256, 65535, 65536, 4294967295, 4294967296])}

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

Payload — the map of exactly fields 1–2, per §1's fields-X–Y rule:

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

## Same-series counter jump — MUST ACCEPT (§2.3)

A second `SignedLocator` by alice, same series, counter **42 → 100**. Within a
series a new counter must be **strictly greater** — deliberately not
previous+1, since a verifier may have missed intervening updates. An
implementation requiring contiguity rejects this valid supersession and is
non-conforming (negative suite, D6).

Payload ({len(sl2_payload)} bytes):

```
{hexblock(sl2_payload)}
```

Signature: `{hx(sl2_sig)}`

Complete object ({len(sl2)} bytes):

```
{hexblock(sl2)}
```

## The `SignedLocator` equal-seqno conflict partner (V12)

Same subject, same `[5, 100]`, **different path** than the counter-jump
locator above — each individually valid, holding both is the equal-`seqno`
conflict, on the **locator path** as V6 is on the endpoint-record path: two
separate decoding routes, each needing the rule. ({len(sl2b)} bytes):

```
{hexblock(sl2b)}
```

## A root's self-anchored `SignedLocator` — MUST ACCEPT (D13)

**Roots legitimately self-anchor** [author, 2026-09-01]: bob names himself as
anchor with the **empty path** — zero nibbles, empty byte string, count 0
(`a2 01 40 02 00`) — the case §2.1 now states. An implementation asserting a
minimum path length rejects every root's locator. ({len(sl_root)} bytes):

```
{hexblock(sl_root)}
```

## A `SignedLocator` carrying an unknown extension — MUST ACCEPT (D9)

Unknown key **4** — deliberately the nearest uint above the signature slot
(field 3), so any implementation that *infers* the signature slot from key
magnitude instead of the schema misreads the extension as the signature
(eighth review). The key is inside the signed payload per §1's global rule;
mutating it breaks the signature (E14). ({len(sl_ext)} bytes):

```
{hexblock(sl_ext)}
```

## A wrong-signer `SignedLocator` — MUST REJECT (S23)

Field 1 names **alice**; the signature is **bob's**, and it is a
cryptographically valid signature over exactly the right payload under the
right tag. The only defect is the binding: the signer is not the named subject.
An implementation that verifies the signature against "whatever key it
resolves" instead of the key field 1 names accepts this object
({len(sl_wrong)} bytes):

```
{hexblock(sl_wrong)}
```

## Genesis back-pointer (§3.1)

`SHA-256(the signer's keyhash)` — over the raw 32 bytes, not a CBOR encoding
(§3.1).

| Signer | Genesis value |
|---|---|
| alice | `{hx(genesis(alice.keyhash))}` |
| bob | `{hx(genesis(bob.keyhash))}` |
| carol | `{hx(genesis(carol.keyhash))}` |""")

# ================================================================ transactions.md


# --- Presence, formation subtype: alice and carol.
TS_START = TS_2026 + 12 * 3600
TS_FINAL = TS_START + 900
WINDOW_ORDINAL = TS_START // 86400
participant = lambda i: e_map([(e_uint(1), e_bstr(i.keyhash))])

# ---- Selective disclosure, §4.5.1: the REAL construction ----
LABELS = ['capture', 'location', 'p0.integrity', 'p0.retention',
          'p1.integrity', 'p1.retention', 'proximity']  # ascending byte order

def disclosure(salt, label, value_bytes):
    assert len(salt) == 16
    return e_arr([e_bstr(salt), e_tstr(label), value_bytes])

def disclosure_set(nickname, values):
    """values: {label: encoded CBOR}. Salts are deterministic for the vectors:
    first 16 bytes of SHA-256('rhtn-test-vectors:salt:<nickname>:<label>') —
    production salts are fresh randomness; vectors must reproduce."""
    slots = {}
    for lab in LABELS:
        salt = H(f'rhtn-test-vectors:salt:{nickname}:{lab}'.encode())[:16]
        D = disclosure(salt, lab, values[lab])
        slots[lab] = (D, H(b'\x00' + D))
    root = H(b'\x01' + b''.join(slots[lab][1] for lab in LABELS))
    return slots, root

def presented(envelope_bytes, slots, reveal):
    """PresentedRecord = [Envelope, [7 DisclosureSlot]] — revealed Disclosure
    or the withheld 32-byte digest, position supplying the label."""
    arr = [slots[lab][0] if lab in reveal else e_bstr(slots[lab][1])
           for lab in LABELS]
    return e_arr([envelope_bytes, e_arr(arr)])

# The formation ceremony's disclosable values — schema-valid throughout:
# capture {modality, count, liveness, version}; location with one asserted
# geohash and no corroborations (no witnesses to corroborate); per-participant
# retention (years) and integrity; proximity with optical+latency passing and
# strongest = optical (3) — no higher-ranked channel present, satisfying §3.2.
form_values = {
    'capture':      e_map([(e_uint(1), e_uint(0)), (e_uint(2), e_uint(3)),
                           (e_uint(3), e_uint(0)), (e_uint(4), e_uint(1))]),
    'location':     e_map([(e_uint(1), e_arr([e_map([(e_uint(1), e_uint(0)),
                                                     (e_uint(2), e_tstr('u4p'))])])),
                           (e_uint(2), e_arr([]))]),
    'p0.integrity': e_map([(e_uint(1), b'\xf5'), (e_uint(2), e_uint(1))]),
    'p0.retention': e_uint(2),
    'p1.integrity': e_map([(e_uint(1), b'\xf4'), (e_uint(2), e_uint(0))]),
    'p1.retention': e_uint(2),
    'proximity':    e_map([(e_uint(1), e_arr([
                            e_map([(e_uint(1), e_uint(3)), (e_uint(2), e_uint(0))]),
                            e_map([(e_uint(1), e_uint(4)), (e_uint(2), e_uint(0)),
                                   (e_uint(3), e_uint(50))])])),
                           (e_uint(2), e_uint(3))]),
}
form_slots, form_root = disclosure_set('formation', form_values)
# Field 3's order is DELIBERATELY the reverse of keyhash order, so that
# participant order (which fixes back-pointer list order, §3.1) and envelope
# kid order (which fixes signature entry order, §3.5) disagree — a decoder
# conflating the two fails this vector.
p_hi, p_lo = sorted([alice, carol], key=lambda i: i.keyhash, reverse=True)
formation_body = e_map([
    (e_uint(0), backptrs([genesis(p_hi.keyhash)], [genesis(p_lo.keyhash)])),
    (e_uint(1), e_uint(TS_START)),
    (e_uint(2), e_uint(TS_FINAL)),
    (e_uint(3), e_arr([participant(p_hi), participant(p_lo)])),
    (e_uint(6), e_uint(1)),
    (e_uint(8), e_bstr(form_root)),
])
formation_txid = H(formation_body)
formation_env, formation_entries = envelope(1, 5, formation_body, [p_hi, p_lo])
pres_full = presented(formation_env, form_slots, set(LABELS))
pres_min = presented(formation_env, form_slots, set())
pres_part = presented(formation_env, form_slots, {'proximity', 'p0.retention'})

# --- Presence, formation subtype: ALICE AND BOB.  design §6.1.1 requires every
# adoption to carry evidence, and §13.1 puts a ceremony before a formation
# adoption, so the alice-adopted-by-bob transaction below needs a record naming
# THOSE TWO.  The alice-carol record above names carol and cannot serve: the
# check is that the record exists and NAMES THESE TWO PARTIES (§4.1 field 8).
TS_AB_START = TS_START - 3 * 3600
TS_AB_FINAL = TS_AB_START + 600
ab_values = dict(form_values)
ab_slots, ab_root = disclosure_set('formation-ab', ab_values)
ab_hi, ab_lo = sorted([alice, bob], key=lambda i: i.keyhash, reverse=True)
ab_form_body = e_map([
    (e_uint(0), backptrs([genesis(ab_hi.keyhash)], [genesis(ab_lo.keyhash)])),
    (e_uint(1), e_uint(TS_AB_START)),
    (e_uint(2), e_uint(TS_AB_FINAL)),
    (e_uint(3), e_arr([participant(ab_hi), participant(ab_lo)])),
    (e_uint(6), e_uint(1)),
    (e_uint(8), e_bstr(ab_root)),
])
ab_form_txid = H(ab_form_body)
ab_form_env, ab_form_entries = envelope(1, 5, ab_form_body, [ab_hi, ab_lo])

# --- Adoption: alice adopted by bob, both at genesis.
TS_ADOPT = TS_2026 + 100 * 86400 + 3600
# Field 8 names the alice-bob formation record above: design §6.1.1 requires
# every adoption to carry evidence, and the check is that the record exists and
# NAMES THESE TWO PARTIES.  Both signers continue that record, which is their
# chain head by the time they sign this.
adopt_body = e_map([
    (e_uint(0), backptrs([ab_form_txid], [ab_form_txid])),
    (e_uint(1), e_bstr(alice.keyhash)),
    (e_uint(2), e_bstr(bob.keyhash)),
    (e_uint(3), adopt_loc),
    (e_uint(4), e_uint(TS_ADOPT)),
    (e_uint(8), e_bstr(ab_form_txid)),
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
dep_env, dep_entries = envelope(1, 2, depart_body, [alice])

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

alice_first, bob_first = sorted([alice, bob], key=lambda i: i.keyhash)
ordered_names = [alice_first.name, bob_first.name]

# --- Adoption where signer order and kid order DIVERGE: the node's keyhash
# sorts after the patron's, so back-pointer list 0 belongs to the signer whose
# envelope entries come second.
div_node, div_patron = sorted([alice, bob], key=lambda i: i.keyhash, reverse=True)[0], \
                       sorted([alice, bob], key=lambda i: i.keyhash)[0]
div_loc = locator(div_patron.keyhash, path([2, 7]), seqno(9, 0))
div_body = e_map([
    (e_uint(0), backptrs([genesis(div_node.keyhash)], [genesis(div_patron.keyhash)])),
    (e_uint(1), e_bstr(div_node.keyhash)),
    (e_uint(2), e_bstr(div_patron.keyhash)),
    (e_uint(3), div_loc),
    (e_uint(4), e_uint(TS_ADOPT + 7200)),
    (e_uint(8), e_bstr(ab_form_txid)),
])
div_txid = H(div_body)
div_env, div_entries = envelope(1, 1, div_body, [div_node, div_patron])

# --- Departure carrying a MERGE: alice's chain forked (the adoption and the
# formation record both continue her genesis) and reunites here.
# §3.1: a merge list is sorted ascending bytewise — one logical merge, one
# encoding.
merge_heads = sorted([adopt_txid, formation_txid])
merge_body = e_map([
    (e_uint(0), backptrs(merge_heads)),
    (e_uint(1), e_bstr(alice.keyhash)),
    (e_uint(2), e_bstr(bob.keyhash)),
    (e_uint(3), seqno(5, 44)),
    (e_uint(4), e_uint(TS_DEPART + 3600)),
])
merge_txid = H(merge_body)

# --- Disavowal with an UNASSIGNED in-range code: MUST be accepted and
# evaluated by band (§4.3's exception to the unknown-enum rule).
code40_body = e_map([
    (e_uint(0), backptrs([adopt_txid])),
    (e_uint(1), e_bstr(bob.keyhash)),
    (e_uint(2), e_bstr(alice.keyhash)),
    (e_uint(3), e_uint(TS_DEPART)),
    (e_uint(4), e_uint(40)),
])
code40_txid = H(code40_body)

# --- Adoption carrying a bounded unknown extension key: preserved,
# re-serialised, and covered by txid and signatures (§1).
ext_loc = e_map([(e_uint(1), e_bstr(bob.keyhash)),
                 (e_uint(2), path([3, 1, 4, 1, 5])),
                 (e_uint(3), seqno(5, 0)),
                 (e_uint(99), e_bstr(bytes.fromhex('beef')))])  # NESTED unknown key
ext_body = e_map([
    (e_uint(0), backptrs([genesis(alice.keyhash)], [genesis(bob.keyhash)])),
    (e_uint(1), e_bstr(alice.keyhash)),
    (e_uint(2), e_bstr(bob.keyhash)),
    (e_uint(3), ext_loc),
    (e_uint(4), e_uint(TS_ADOPT)),
    # Evidence, like every adoption (design §6.1.1): this one is between the
    # same two parties as the first, so it names the same record.  The vector
    # is about UNKNOWN KEY PRESERVATION, and an adoption that is malformed for
    # an unrelated reason cannot demonstrate that a decoder preserved anything.
    (e_uint(8), e_bstr(ab_form_txid)),
    (e_uint(99), e_bstr(bytes.fromhex('c0ffee'))),
])
ext_txid = H(ext_body)
ext_env, ext_entries = envelope(1, 1, ext_body, [alice, bob])

# --- Normal presence record (canonical bar 2): alice and bob, 16 witnesses,
# three embedded classical verifier responses covering the selection_basis
# matrix, a real disclosure set, and the 36-entry envelope. Participant order
# is reverse-keyhash so participant, witness and kid orders all differ.
AAD_CONSENT = b'rhtn/1:consent'
AAD_VERIFIER = b'rhtn/1:verifier'
AAD_SUCCESSOR = b'rhtn/1:successor'
AAD_TRANSFER = b'rhtn/1:transfer'

TS_C2 = TS_DEPART + 40 * 86400
TS_C2F = TS_C2 + 3600
W16 = sorted([IDS[f'w{i}'] for i in range(1, 17)], key=lambda i: i.keyhash)
n_hi, n_lo = sorted([alice, bob], key=lambda i: i.keyhash, reverse=True)

def witness_entry(w, nominated_by, bits):
    return e_map([(e_uint(1), e_bstr(w.keyhash)),
                  (e_uint(2), e_bstr(nominated_by.keyhash)),
                  (e_uint(3), e_uint(bits))])

# nominated_by alternates between the participants; attestation bits vary
# within the interpreted 0-2 range (7 = all three, 3 = ran+responsive, 5 =
# ran+latency).
npr_witnesses = [witness_entry(w, alice if i % 2 == 0 else bob, (7, 3, 5)[i % 3])
                 for i, w in enumerate(W16)]

def vquery(subject, querier, precommit, profile_seed, tplv, verifier):
    # field 7 (addressed verifier) is inside the hash: query_id covers the
    # map with field 6 absent, i.e. fields 1-5 and 7 (wire s5.6, 2026-09-03)
    prof = H(profile_seed)
    q157 = e_map([(e_uint(1), e_bstr(subject.keyhash)),
                  (e_uint(2), e_bstr(querier.keyhash)),
                  (e_uint(3), e_bstr(precommit)),
                  (e_uint(4), e_bstr(prof)),
                  (e_uint(5), e_uint(tplv)),
                  (e_uint(7), e_bstr(verifier.keyhash))])
    qid = H(q157)
    full = e_map([(e_uint(1), e_bstr(subject.keyhash)),
                  (e_uint(2), e_bstr(querier.keyhash)),
                  (e_uint(3), e_bstr(precommit)),
                  (e_uint(4), e_bstr(prof)),
                  (e_uint(5), e_uint(tplv)),
                  (e_uint(6), e_bstr(qid)),
                  (e_uint(7), e_bstr(verifier.keyhash))])
    return qid, full

def consent_over(qid, subject):
    prot = sig_protected(-8)
    tbs = sig_structure_sign1(prot, AAD_CONSENT, qid)
    return e_arr([e_bstr(prot), b'\xa0', NULL, e_bstr(subject.sign(tbs))])

def classical_response(verifier, subject, qid, consent, result,
                       basis=None, tplv=None, sb=0):
    """Presence-record form: field 9 is COSE_Sign1, classical, over the
    canonical CBOR of the present fields 1-8 and 10 (wire s4.5)."""
    pairs = [(e_uint(1), e_bstr(verifier.keyhash)),
             (e_uint(2), e_bstr(subject.keyhash)),
             (e_uint(3), e_bstr(qid)),
             (e_uint(4), e_uint(result))]
    if basis is not None:
        pairs.append((e_uint(5), e_uint(basis)))
    if tplv is not None:
        pairs.append((e_uint(6), e_uint(tplv)))
    pairs.append((e_uint(7), consent))
    payload = e_map(pairs + [(e_uint(10), e_uint(sb))])
    prot = sig_protected(-8)
    tbs = sig_structure_sign1(prot, AAD_VERIFIER, payload)
    vsig = e_arr([e_bstr(prot), b'\xa0', NULL, e_bstr(verifier.sign(tbs))])
    return e_map(pairs + [(e_uint(9), vsig), (e_uint(10), e_uint(sb))])

npr_precommit = H(b'rhtn-test-vectors:normal-ceremony-precommitment')
npr_verifiers = sorted([IDS['c1'], IDS['c2'], IDS['c3']], key=lambda i: i.keyhash)
# One profile per ceremony (the pre-commitment pins it); the three queries
# differ by their addressed verifier - field 7 - which is what makes their
# query_ids distinct (2026-09-03; previously three profiles stood in).
npr_q0, npr_query0 = vquery(alice, bob, npr_precommit, b'rhtn-test-vectors:npr-profile', 3, npr_verifiers[0])
npr_q1, _ = vquery(alice, bob, npr_precommit, b'rhtn-test-vectors:npr-profile', 3, npr_verifiers[1])
npr_q2, _ = vquery(alice, bob, npr_precommit, b'rhtn-test-vectors:npr-profile', 3, npr_verifiers[2])
npr_responses = [
    classical_response(npr_verifiers[0], alice, npr_q0, consent_over(npr_q0, alice),
                       0, basis=0, tplv=3, sb=0),   # match, photo, met
    classical_response(npr_verifiers[1], alice, npr_q1, consent_over(npr_q1, alice),
                       0, basis=1, sb=2),           # match, personal, reachable
    classical_response(npr_verifiers[2], alice, npr_q2, consent_over(npr_q2, alice),
                       3, sb=3),                    # unavailable, discretionary
]

npr_values = {
    'capture':      e_map([(e_uint(1), e_uint(0)), (e_uint(2), e_uint(4)),
                           (e_uint(3), e_uint(0)), (e_uint(4), e_uint(2))]),
    'location':     e_map([(e_uint(1), e_arr([e_map([(e_uint(1), e_uint(0)),
                                                     (e_uint(2), e_tstr('u4pr'))])])),
                           (e_uint(2), e_arr([e_map([(e_uint(1), e_bstr(W16[0].keyhash)),
                                                     (e_uint(2), e_uint(3)),
                                                     (e_uint(3), e_uint(5))])]))]),
    'p0.integrity': e_map([(e_uint(1), b'\xf5'), (e_uint(2), e_uint(1))]),
    'p0.retention': e_uint(2),
    'p1.integrity': e_map([(e_uint(1), b'\xf4'), (e_uint(2), e_uint(0))]),
    'p1.retention': e_uint(3),
    'proximity':    e_map([(e_uint(1), e_arr([
                            e_map([(e_uint(1), e_uint(2)), (e_uint(2), e_uint(0))]),
                            e_map([(e_uint(1), e_uint(3)), (e_uint(2), e_uint(0))])])),
                           (e_uint(2), e_uint(2))]),
}
npr_slots, npr_root = disclosure_set('normal', npr_values)

npr_signers = [n_hi, n_lo] + W16
def _npr_head(s):
    if s is alice: return [merge_txid]
    if s is bob: return [adopt_txid]
    return [genesis(s.keyhash)]
npr_body = e_map([
    (e_uint(0), backptrs(*[_npr_head(s) for s in npr_signers])),
    (e_uint(1), e_uint(TS_C2)),
    (e_uint(2), e_uint(TS_C2F)),
    (e_uint(3), e_arr([participant(n_hi), participant(n_lo)])),
    (e_uint(4), e_arr(npr_witnesses)),
    (e_uint(5), e_arr(npr_responses)),
    (e_uint(6), e_uint(0)),
    (e_uint(8), e_bstr(npr_root)),
])
npr_txid = H(npr_body)

# --- Presence, normal subtype: BOB AND CAROL.  design §6.3 requires a peering
# record to name a proof of presence between the two peers, and no record above
# names both: the formation is alice-carol, and this file's other normal record
# is alice-bob.  Peers are in different subtrees by construction, so there is no
# former-patron alternative (§6.1.1) — peering is always priced in a meeting.
# Minimal but structurally complete: one witness, whose attestation bits carry
# `protocol_ran` and `both_responsive` so §3.2's floor is met.
TS_BC = TS_C2 + 4 * 3600
TS_BCF = TS_BC + 720
bc_values = dict(npr_values)
bc_slots, bc_root = disclosure_set('normal-bc', bc_values)
bc_hi, bc_lo = sorted([bob, carol], key=lambda i: i.keyhash, reverse=True)
bc_signers = [bc_hi, bc_lo, W16[0]]
def _bc_head(s):
    if s is carol: return [formation_txid]
    return [npr_txid]          # bob and w1 both signed the normal record above
bc_body = e_map([
    (e_uint(0), backptrs(*[_bc_head(s) for s in bc_signers])),
    (e_uint(1), e_uint(TS_BC)),
    (e_uint(2), e_uint(TS_BCF)),
    (e_uint(3), e_arr([participant(bc_hi), participant(bc_lo)])),
    (e_uint(4), e_arr([witness_entry(W16[0], bc_hi, 3)])),
    (e_uint(6), e_uint(0)),
    (e_uint(8), e_bstr(bc_root)),
])
bc_txid = H(bc_body)
bc_env, bc_entries = envelope(1, 5, bc_body, bc_signers)
npr_env, npr_entries = envelope(1, 5, npr_body, npr_signers)
npr_full = presented(npr_env, npr_slots, set(LABELS))
npr_min = presented(npr_env, npr_slots, set())
npr_part = presented(npr_env, npr_slots, {'location', 'p1.retention'})

# --- Finalization must-accepts (canonical bar 11, V7): a record finalized on
# a lone no-match, and one finalized with NO responses at all (key 5 absent -
# the empty array is never encoded). Both fork alice's and bob's chains, which
# the archive-as-DAG permits.
fin_precommit_a = H(b'rhtn-test-vectors:fin-nomatch-precommitment')
fin_qa, _ = vquery(alice, bob, fin_precommit_a, b'rhtn-test-vectors:fin-nm-profile', 3, IDS['c4'])
fin_nm_resp = classical_response(IDS['c4'], alice, fin_qa, consent_over(fin_qa, alice),
                                 1, basis=0, tplv=3, sb=0)   # no-match
fin_nm_slots, fin_nm_root = disclosure_set('fin-nomatch', npr_values)
fin_nm_signers = [n_hi, n_lo, IDS['w1']]
fin_nm_body = e_map([
    (e_uint(0), backptrs(*[_npr_head(s) if s in (alice, bob) else [genesis(s.keyhash)]
                           for s in fin_nm_signers])),
    (e_uint(1), e_uint(TS_C2 + 86400)),
    (e_uint(2), e_uint(TS_C2 + 86400 + 1800)),
    (e_uint(3), e_arr([participant(n_hi), participant(n_lo)])),
    (e_uint(4), e_arr([witness_entry(IDS['w1'], alice, 7)])),
    (e_uint(5), e_arr([fin_nm_resp])),
    (e_uint(6), e_uint(0)),
    (e_uint(8), e_bstr(fin_nm_root)),
])
fin_nm_txid = H(fin_nm_body)
fin_nm_env, _ = envelope(1, 5, fin_nm_body, fin_nm_signers)

fin_ab_slots, fin_ab_root = disclosure_set('fin-absent', npr_values)
fin_ab_signers = [n_hi, n_lo, IDS['w2']]
fin_ab_body = e_map([
    (e_uint(0), backptrs(*[_npr_head(s) if s in (alice, bob) else [genesis(s.keyhash)]
                           for s in fin_ab_signers])),
    (e_uint(1), e_uint(TS_C2 + 2 * 86400)),
    (e_uint(2), e_uint(TS_C2 + 2 * 86400 + 1800)),
    (e_uint(3), e_arr([participant(n_hi), participant(n_lo)])),
    (e_uint(4), e_arr([witness_entry(IDS['w2'], bob, 7)])),
    (e_uint(6), e_uint(0)),
    (e_uint(8), e_bstr(fin_ab_root)),
])
fin_ab_txid = H(fin_ab_body)
fin_ab_env, _ = envelope(1, 5, fin_ab_body, fin_ab_signers)

# --- Two more alice-carol normal records (canonical bar 3's repeated
# counterparty), minimal shape: one witness, no responses. ac2 continues ac1
# on both chains; ac1 forks alice from her merge and carol from the formation.
ac1_slots, ac1_root = disclosure_set('ac1', npr_values)
ac1_signers = [s for s in sorted([alice, carol], key=lambda i: i.keyhash, reverse=True)] + [IDS['w3']]
ac1_body = e_map([
    (e_uint(0), backptrs(*[[merge_txid] if s is alice else [formation_txid]
                           if s is carol else [genesis(s.keyhash)] for s in ac1_signers])),
    (e_uint(1), e_uint(TS_C2 + 5 * 86400)),
    (e_uint(2), e_uint(TS_C2 + 5 * 86400 + 1200)),
    (e_uint(3), e_arr([participant(ac1_signers[0]), participant(ac1_signers[1])])),
    (e_uint(4), e_arr([witness_entry(IDS['w3'], carol, 7)])),
    (e_uint(6), e_uint(0)),
    (e_uint(8), e_bstr(ac1_root)),
])
ac1_txid = H(ac1_body)
ac1_env, _ = envelope(1, 5, ac1_body, ac1_signers)

ac2_slots, ac2_root = disclosure_set('ac2', npr_values)
ac2_signers = ac1_signers
ac2_body = e_map([
    (e_uint(0), backptrs(*[[ac1_txid] if s in (alice, carol) else [genesis(s.keyhash)]
                           for s in ac2_signers])),
    (e_uint(1), e_uint(TS_C2 + 6 * 86400)),
    (e_uint(2), e_uint(TS_C2 + 6 * 86400 + 1200)),
    (e_uint(3), e_arr([participant(ac2_signers[0]), participant(ac2_signers[1])])),
    (e_uint(4), e_arr([witness_entry(IDS['w3'], alice, 3)])),
    (e_uint(6), e_uint(0)),
    (e_uint(8), e_bstr(ac2_root)),
])
ac2_txid = H(ac2_body)
ac2_env, _ = envelope(1, 5, ac2_body, ac2_signers)

# The prior alice-c1 meeting (canonical bar: the capture context): c1 can hold
# a capture of alice only from a ceremony c1 participated in. Sealed at THIS
# ceremony's pre-commitment; the later KeyGrant names THIS record's txid -
# never the record under assembly, whose txid embeds the very responses the
# grant unlocks (s7.5.2's circularity note).
pc1_ca = H(b'rhtn-test-vectors:precommit-contribution:alice:c1-meeting')[:16]
pc1_cb = H(b'rhtn-test-vectors:precommit-contribution:c1')[:16]
_p1, _p2 = (pc1_ca, pc1_cb) if alice.keyhash < IDS['c1'].keyhash else (pc1_cb, pc1_ca)
pc1_precommit = H(b'rhtn/1:ceremony' + _p1 + _p2)
pc1_slots, pc1_root = disclosure_set('alice-c1', npr_values)
pc1_signers = [s for s in sorted([alice, IDS['c1']], key=lambda i: i.keyhash, reverse=True)] + [IDS['w2']]
pc1_body = e_map([
    (e_uint(0), backptrs(*[[merge_txid] if s is alice else [genesis(s.keyhash)]
                           for s in pc1_signers])),
    (e_uint(1), e_uint(TS_DEPART + 10 * 86400)),
    (e_uint(2), e_uint(TS_DEPART + 10 * 86400 + 1500)),
    (e_uint(3), e_arr([participant(pc1_signers[0]), participant(pc1_signers[1])])),
    (e_uint(4), e_arr([witness_entry(IDS['w2'], alice, 7)])),
    (e_uint(6), e_uint(0)),
    (e_uint(8), e_bstr(pc1_root)),
])
pc1_txid = H(pc1_body)
pc1_env, _ = envelope(1, 5, pc1_body, pc1_signers)

TS_EVAL = TS_C2 + 100 * 86400   # a hypothetical next alice-bob ceremony's started_at

# --- Peering (type 4): bob and carol as infra peers; alternative continuations
# of their existing chain heads (bob: the adoption; carol: the formation).
def network_point(ip, asn=None, port=None):
    assert port != 7431, 'the default port is never written out (§1, §4.4)'
    assert port is None or 1 <= port <= 65535, 'port: u16, never zero'
    pairs = [(e_uint(1), e_bstr(bytes(ip)))]
    if asn is not None: pairs.append((e_uint(2), e_uint(asn)))
    if port is not None: pairs.append((e_uint(3), e_uint(port)))
    return e_map(pairs)
peer_body = e_map([
    (e_uint(0), backptrs([adopt_txid], [formation_txid])),
    (e_uint(1), e_bstr(bob.keyhash)),
    (e_uint(2), e_bstr(carol.keyhash)),
    (e_uint(3), network_point([10, 0, 0, 1], asn=64511, port=7432)),
    (e_uint(4), network_point([192, 0, 2, 7])),
    (e_uint(5), e_uint(TS_DEPART + 7200)),
    (e_uint(8), e_bstr(bc_txid)),
])
peer_txid = H(peer_body)

# --- Node endpoint record (§7.6): classical-only COSE_Sign1, fully computable.
# Payload: the map of exactly fields 1-3, per §1's fields-X–Y rule.
AAD_ENDPOINTS = b'rhtn/1:endpoints'
er_fields = [
    (e_uint(1), e_bstr(bob.keyhash)),
    (e_uint(2), e_arr([network_point([10, 0, 0, 1], asn=64511, port=7432)])),
    (e_uint(3), seqno(9, 2)),
]
er_payload = e_map(er_fields)
er_protected = sig_protected(-8)
er_tbs = sig_structure_sign1(er_protected, AAD_ENDPOINTS, er_payload)
er_sig = bob.sign(er_tbs)
er_cose = e_arr([e_bstr(er_protected), b'\xa0', NULL, e_bstr(er_sig)])
endpoint_record = e_map(er_fields + [(e_uint(4), er_cose)])

# --- The equal-seqno conflict pair: a second record by bob, SAME seqno [9,2],
# DIFFERENT endpoint set. Individually valid; together malformed rather than a
# tie (§7.6, §7.7.3) — the disagreement no reader may break by preference.
er2_fields = [
    (e_uint(1), e_bstr(bob.keyhash)),
    (e_uint(2), e_arr([network_point([192, 0, 2, 7], port=7433)])),
    (e_uint(3), seqno(9, 2)),
]
er2_payload = e_map(er2_fields)
er2_tbs = sig_structure_sign1(er_protected, AAD_ENDPOINTS, er2_payload)
er2_sig = bob.sign(er2_tbs)
er2 = e_map(er2_fields + [(e_uint(4),
      e_arr([e_bstr(er_protected), b'\xa0', NULL, e_bstr(er2_sig)]))])

# --- Sign1 extension coverage: fields 1-3 plus an unknown key, all signed.
er3_fields = er_fields + [(e_uint(99), e_bstr(bytes.fromhex('c0ffee')))]
er3_payload = e_map(er3_fields)
er3_tbs = sig_structure_sign1(er_protected, AAD_ENDPOINTS, er3_payload)
er3_sig = bob.sign(er3_tbs)
er_ext = e_map(er3_fields + [(e_uint(4),
      e_arr([e_bstr(er_protected), b'\xa0', NULL, e_bstr(er3_sig)]))])


# --- Second reissue: to a NUMERICALLY SMALLER series. Series are arbitrary
# labels (§2.3); an implementation treating them as generations rejects this.
reissue2_body = e_map([
    (e_uint(0), backptrs([reissue_txid], [reissue_txid])),
    (e_uint(1), e_bstr(alice.keyhash)),
    (e_uint(2), e_bstr(bob.keyhash)),
    (e_uint(3), seqno(0xDEADBEEF, 17)),
    (e_uint(4), seqno(2, 0)),
    (e_uint(5), e_uint(TS_DEPART + 2 * 86400)),
])
reissue2_txid = H(reissue2_body)

# --- Optionals-exercised bodies (F9, sixth review) ---
adopt_full_body = e_map([
    (e_uint(0), backptrs([genesis(alice.keyhash)], [genesis(bob.keyhash)])),
    (e_uint(1), e_bstr(alice.keyhash)),
    (e_uint(2), e_bstr(bob.keyhash)),
    (e_uint(3), adopt_loc),
    (e_uint(4), e_uint(TS_ADOPT)),
    (e_uint(5), alice.key_material),
    (e_uint(7), e_bstr(depart_txid)),
    (e_uint(8), e_bstr(npr_txid)),
])
adopt_full_txid = H(adopt_full_body)
depart_r_body = e_map([
    (e_uint(0), backptrs([adopt_txid])),
    (e_uint(1), e_bstr(alice.keyhash)),
    (e_uint(2), e_bstr(bob.keyhash)),
    (e_uint(3), seqno(5, 45)),
    (e_uint(4), e_uint(TS_DEPART + 7200)),
    (e_uint(5), e_uint(2)),
])
depart_r_txid = H(depart_r_body)
audit = e_map([(e_uint(1), e_uint(TS_DEPART)), (e_uint(2), b'\xf5'),
               (e_uint(3), e_bstr(carol.keyhash))])
peer_full_body = e_map([
    (e_uint(0), backptrs([adopt_txid], [formation_txid])),
    (e_uint(1), e_bstr(bob.keyhash)),
    (e_uint(2), e_bstr(carol.keyhash)),
    (e_uint(3), network_point([10, 0, 0, 1], asn=64511, port=7432)),
    (e_uint(4), network_point([192, 0, 2, 7])),
    (e_uint(5), e_uint(TS_DEPART + 7200)),
    (e_uint(6), e_uint(1 << 30)),
    (e_uint(7), e_arr([audit])),
])
peer_full_txid = H(peer_full_body)

emit('transactions.md', f"""# Transaction bodies, txids, and one full envelope

{PIN}

**Draft. Spec-derived, unverified by an implementation.** See
[README.md](README.md).

`txid = SHA-256(deterministic CBOR of the body map)`, signature array excluded
(`wire-format.md` §1). Key 0 carries one back-pointer list per required signer
in signer order (§3.1). *Historical note: §3.1's table lacked its series-reissue
row when this set was first drafted; drafting found it and the row was added the
same day (README, Findings).*

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
    emit('transactions.md', f"""Signature:

```
{hexblock(sig)}
```
""")

emit('transactions.md', f"""#### Envelope bytes — final, all four signatures real

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

### The departure envelope — the single-signer shape

One logical signer, **two** `COSE_Signature` entries, and a decoder MUST NOT
expect the old patron's signature (§4.2) — an implementation deriving the
signer set from the identities the body names, rather than from the type's
required-signer rule, fails here. Ed25519 signature (protected
`{hx(dep_entries[0][2])}`):

```
{hexblock(dep_entries[0][4])}
```

Envelope bytes ({len(dep_env)} bytes — final, both signatures real):

```
{hexblock(dep_env)}
```

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

## Presence record (type 5), formation subtype — alice and bob

**The evidence the adoptions between these two rest on.** design §6.1.1
requires every adoption to carry evidence, and the check on field 8 is that the
record *exists and names these two parties* — so a corpus whose adoptions
reference a record it does not contain cannot demonstrate the rule. §13.1 puts
a ceremony before a formation adoption, which is what makes a formation record
the faithful evidence for an adoption of two parties at genesis.

Same shape as the alice–carol record below: keys 4 and 5 omitted, key 6 = 1,
key 0 the genesis value for both signers.

Body bytes ({len(ab_form_body)} bytes):

```
{hexblock(ab_form_body)}
```

txid: `{hx(ab_form_txid)}` — field 8 of the adoption, the divergent-order
adoption, and the extension-keys adoption

Envelope bytes ({len(ab_form_env)} bytes):

```
{hexblock(ab_form_env)}
```

## Presence record (type 5), normal subtype — bob and carol

**The evidence the peering rests on.** design §6.3: peering requires a meeting
between the two peers, and §6.1.1's former-patron alternative cannot apply,
peers sharing no prior relationship. Normal rather than formation — they are
not forming a subnet, they are meeting. One witness, attestation bits `3`:
`protocol_ran` and `both_responsive`, §3.2's floor.

Body bytes ({len(bc_body)} bytes):

```
{hexblock(bc_body)}
```

txid: `{hx(bc_txid)}` — field 8 of the peering record

Envelope bytes ({len(bc_env)} bytes, three signers — two participants and the
witness):

```
{hexblock(bc_env)}
```

## Presence record (type 5), formation subtype — alice and carol

Keys 4 and 5 are **omitted entirely** (§3.2: absence means empty); key 6 is 1;
key 0 carries the genesis value for both signers, which for a formation record
is also a structural rule (§3.2). *(Key 7, the seed-window ordinal, was
retired 2026-09-01 with deterministic selection; the number is not reused.)*
**Key 8 is the real §4.5.1 root** — the record is a fully integrated
known-answer object, and the three presentations below verify against it.

**Field 3's order is deliberately {p_hi.name} then {p_lo.name} — the reverse of
keyhash order.** Participant order fixes back-pointer list order (§3.1); kid
order fixes envelope entry order (§3.5); here they disagree, so list 0 is
{p_hi.name}'s genesis while an envelope's first entries would be {p_lo.name}'s.

Body ({len(formation_body)} bytes):

```
{hexblock(formation_body)}
```

txid: `{hx(formation_txid)}`

### The disclosure set (§4.5.1)

Salts are deterministic for the vectors — the first 16 bytes of
`SHA-256("rhtn-test-vectors:salt:formation:<label>")`; production salts are
fresh randomness. `digest = SHA-256(0x00 ‖ CBOR(Disclosure))`;
`root = SHA-256(0x01 ‖ digests ascending by label)` = body field 8. `p0` is
{p_hi.name}, `p1` is {p_lo.name} (field 3's order).

| Label | Salt | Value (CBOR) | Digest |
|---|---|---|---|
{chr(10).join(f"| `{lab}` | `{form_slots[lab][0][2:18].hex()}` | `{form_values[lab].hex()}` | `{form_slots[lab][1].hex()}` |" for lab in LABELS)}

root: `{hx(form_root)}`

### The formation envelope — type 5's dynamic signer set

Two participants, zero witnesses: two logical signers, four entries, sorted by
`kid` then classical-first as every envelope is ({len(formation_env)} bytes):

```
{hexblock(formation_env)}
```

### Three presentations of one record (§4.5.1)

`PresentedRecord = [Envelope, [7 DisclosureSlot]]`, position supplying the
label; a slot is the revealed `Disclosure` or the withheld 32-byte digest. All
three verify against the same body root under the same envelope signatures.

**Fully revealed** ({len(pres_full)} bytes):

```
{hexblock(pres_full)}
```

**Partial** — `proximity` and `p0.retention` revealed, five withheld
({len(pres_part)} bytes):

```
{hexblock(pres_part)}
```

**Minimal** — all seven withheld ({len(pres_min)} bytes):

```
{hexblock(pres_min)}
```

## Adoption where signer order and kid order diverge — {div_node.name} adopted by {div_patron.name}

Field 8 references the alice–carol formation record while this adoption is
between {div_node.name} and {div_patron.name} — the dereference-mismatch case
(context fixture V9a): structurally valid, `checks[proof_of_presence] = fail`.

The node's keyhash sorts **after** the patron's, so §3.1's signer order
({div_node.name}, {div_patron.name}) and §3.5's entry order
({div_patron.name}, {div_node.name}) disagree. **Back-pointer list 0 is
{div_node.name}'s; the envelope's first two entries are {div_patron.name}'s.**
An implementation deriving either order from the other passes the first
adoption vector and fails this one — the confusion §3.1 warns is silent.

Body ({len(div_body)} bytes):

```
{hexblock(div_body)}
```

txid: `{hx(div_txid)}`

Envelope entry order and Ed25519 signatures (same construction as the first
adoption; `Sig_structure`s omitted for brevity):
""")

for ident, alg, prot, tbs, sig in div_entries:
    algname = 'Ed25519' if alg == -8 else 'ML-DSA-65'
    emit('transactions.md',
         f"- {ident.name}, {algname}: protected `{hx(prot)}`, signature SHA-256"
         f" `{hx(H(sig))}` (full value in the envelope bytes below)")
emit('transactions.md', f"""
Envelope bytes ({len(div_env)} bytes — final):

```
{hexblock(div_env)}
```""")

emit('transactions.md', f"""
## Departure carrying a merge — alice reunites two branches

Alice's chain forked: the first adoption and the formation record both continue
her genesis. This departure closes the fork with a two-entry back-pointer list —
**a merge is an ordinary transaction with a longer list; no merge type exists**
(§3.1). The list is sorted ascending bytewise, as §3.1 requires — one logical
merge, one encoding, one txid.

Body ({len(merge_body)} bytes):

```
{hexblock(merge_body)}
```

txid: `{hx(merge_txid)}`

## Disavowal with an unassigned in-range code — MUST ACCEPT

Reason code **40**: unassigned in v1, inside the 0–63 space, with-prejudice
band (bit 5 set). §4.3 makes disavowal codes an exception to §1's unknown-enum
rule — **an unfamiliar in-range code is retained and evaluated by its band,
never rejected.** A decoder rejecting this body is over-strict and
non-conforming. (Code 64 is malformed — outside the code space, not
"unassigned"; see negative vector T8.)

Body ({len(code40_body)} bytes):

```
{hexblock(code40_body)}
```

txid: `{hx(code40_txid)}`

## Adoption carrying unknown extension keys, top-level AND nested — MUST ACCEPT AND PRESERVE

The first adoption's body plus unknown key `99: h'c0ffee'` at the top level
**and `99: h'beef'` nested inside the Locator** — §1's preservation rule reaches
every map, and a typed implementation with a struct per schema can preserve the
outer key while silently dropping the nested one. Both are covered by the txid
and by every signature: mutating **either** breaks all four (E10).

Body ({len(ext_body)} bytes):

```
{hexblock(ext_body)}
```

txid: `{hx(ext_txid)}`

**The envelope over this body is fully signed, and the unknown key is inside
the signed payload** — mutate `c0ffee` and **all four signatures fail**
(negative suite, E10). Ed25519 signatures ({ext_entries[0][0].name} then
{ext_entries[2][0].name}, by kid):

```
{hx([e for e in ext_entries if e[1] == -8][0][4])}
{hx([e for e in ext_entries if e[1] == -8][1][4])}
```

Envelope bytes ({len(ext_env)} bytes — final, all four signatures inside):

```
{hexblock(ext_env)}
```

## Peering (type 4) — bob and carol as infra peers

Signer order: endpoint A then endpoint B, as fields 1 and 2 (§3.1). The chains
continue bob's adoption and carol's formation record. Field 3's `NetworkPoint`
carries ASN and an explicit port; field 4's carries neither — the port absent
means the default 7431, and neither optional field is ever encoded empty.
Field 7 (audits) is omitted entirely per §1's optional-empty rule.

Body ({len(peer_body)} bytes):

```
{hexblock(peer_body)}
```

txid: `{hx(peer_txid)}`

## Series reissue to a numerically smaller series — MUST ACCEPT

Alice reissues again: the series she leaves is `0xDEADBEEF` at counter 17, the
new series is **2** — numerically far below it. `series` is an arbitrary label,
never ordered and never assumed to increment (§2.3): an implementation treating
it as a generation number rejects this valid reissue and is non-conforming
(negative suite, D7). Both chains continue the first reissue.

Body ({len(reissue2_body)} bytes):

```
{hexblock(reissue2_body)}
```

txid: `{hx(reissue2_txid)}`

## Optionals exercised — three positive bodies no minimal vector decodes

An implementation can pass every minimal vector above without ever decoding
`KeyMaterial`, a `Recovery`-free adoption's optional references, a departure
reason, or peering's commitment and audit history. These three close that.

**Adoption with fields 5, 7 and 8** — carried `KeyMaterial` (alice's, hashing
to field 1 per §4.1), an archive head, and a proof-of-presence reference.
**Field 8 references the normal alice–bob presence record** (canonical bar 2,
landed 2026-09-02): dereference evaluation (§3.4) finds a record naming exactly
these two parties — context fixture V9's *pass* case. The mismatch case lives
on the divergence adoption above, whose field 8 references the alice–carol
formation record (context fixture V9a). ({len(adopt_full_body)} bytes):

```
{hexblock(adopt_full_body)}
```

txid: `{hx(adopt_full_txid)}`

**Departure with a reason code** (2, same enumeration as §4.3;
{len(depart_r_body)} bytes):

```
{hexblock(depart_r_body)}
```

txid: `{hx(depart_r_txid)}`

**Peering with commitment and audit history** — field 6 and one `Audit`
({len(peer_full_body)} bytes):

```
{hexblock(peer_full_body)}
```

txid: `{hx(peer_full_txid)}`""")

emit('transactions.md', f"""
## Two further alice–carol records — the bundle's repeated counterparty

Minimal normal records (one witness, no responses — key 5 absent per §1) for
canonical bar 3: with the formation, alice holds **three** records naming
carol, which the curated-bundle fixture in `verifier-selection.md` counts as
**one candidate**. ac2 continues ac1 on both participants' chains; ac1 forks
alice from her merge and carol from the formation, which the archive-as-DAG
permits.

ac1 body ({len(ac1_body)} bytes):

```
{hexblock(ac1_body)}
```

txid: `{hx(ac1_txid)}`

ac1 envelope ({len(ac1_env)} bytes):

```
{hexblock(ac1_env)}
```

ac2 body ({len(ac2_body)} bytes):

```
{hexblock(ac2_body)}
```

txid: `{hx(ac2_txid)}`

ac2 envelope ({len(ac2_env)} bytes):

```
{hexblock(ac2_env)}
```""")

emit('transactions.md', f"""
## The prior alice–c1 record — the capture context

The normal record's first verifier, c1, answers by photo — which requires a
capture of alice sealed at a ceremony **c1 participated in**. This is that
meeting: one witness, no responses, finalized well before the normal record.
The `KeyGrant` in `messages.md` names **this** record's txid and derives its
key from **this** ceremony's contributory pre-commitment — never the record
under assembly, whose txid embeds the very responses the grant unlocks
(design §7.5.2).

Body ({len(pc1_body)} bytes):

```
{hexblock(pc1_body)}
```

txid: `{hx(pc1_txid)}`

Envelope ({len(pc1_env)} bytes):

```
{hexblock(pc1_env)}
```""")

emit('transactions.md', f"""
## Normal presence record — alice and bob, sixteen witnesses, 36-entry envelope

Canonical bar 2. Subtype 0; participant order is reverse-keyhash
({n_hi.name} first), witness order is ascending keyhash, and envelope entries
sort by kid — three orders, all different, so a decoder conflating any two
fails here. Sixteen witnesses is the §1 ceiling: with both participants that
is eighteen logical signers and **36 envelope entries**. `nominated_by`
alternates between the participants; attestation bits vary within the
interpreted 0–2 range. Field 5 carries three classical responses **sorted
ascending by verifier keyhash**, covering the `selection_basis` matrix — 0
(met) with a photo match, 2 (reachable) with personal knowledge (no
template version, per the field-6 presence rule), 3 (discretionary) with
`unavailable` (no basis, no template). Value 1 (in-horizon) is uncovered
here: three responses, four values, and the three retained carry the same
semantics they had before the tier-aligned renumbering (2026-09-03). Disclosure salts derive from nickname
`normal` (the formation's recipe); the location value carries a witness
corroboration. Back-pointers: alice continues her merge, bob his adoption,
every witness its genesis.

Worked verification query for the first response (querier is **bob**, the
counterparty — the ceremony form, against the recovery form where the
verifier is its own querier):

```
{hexblock(npr_query0)}
```

query_id: `{hx(npr_q0)}`

Body ({len(npr_body)} bytes):

```
{hexblock(npr_body)}
```

txid: `{hx(npr_txid)}`

Envelope bytes ({len(npr_env)} bytes — final, all 36 signatures real):

```
{hexblock(npr_env)}
```

### Presentations of the normal record

**Fully revealed** ({len(npr_full)} bytes):

```
{hexblock(npr_full)}
```

**Partial** — location and p1.retention revealed ({len(npr_part)} bytes):

```
{hexblock(npr_part)}
```

**Minimal** — all seven withheld ({len(npr_min)} bytes):

```
{hexblock(npr_min)}
```

## Finalization must-accepts (V7) — the threshold gates nothing

Two normal records an over-strict decoder wrongly rejects. Both fork alice's
and bob's chains, which the archive-as-DAG permits (§3.1).

**Finalized on a lone no-match** — one witness, one response, result 1. A
no-match is evidence, not a validity failure ({len(fin_nm_body)} bytes):

```
{hexblock(fin_nm_body)}
```

txid: `{hx(fin_nm_txid)}`

Envelope ({len(fin_nm_env)} bytes):

```
{hexblock(fin_nm_env)}
```

**Finalized with no responses at all** — key 5 absent (§1: the empty array is
never encoded). Absent slots are the encoding of unanswered queries; the
criterion sizes the sample and does not gate finalization
({len(fin_ab_body)} bytes):

```
{hexblock(fin_ab_body)}
```

txid: `{hx(fin_ab_txid)}`

Envelope ({len(fin_ab_env)} bytes):

```
{hexblock(fin_ab_env)}
```""")

# --- Recovery adoption (type 1): alice2 recovers alice's identity, adopted by
# bob; carol (a prior counterparty of alice: the formation ceremony) is the
# verifier. Per design s9.1 the verifier is its own querier; selection_basis is
# 0 (known) by rule; personal_knowledge is the ordinary basis, so response
# field 6 is absent. The recovery meeting's pre-commitment binds the query.
rec_precommit = H(b'rhtn-test-vectors:recovery-ceremony-precommitment')
rec_profile = H(b'rhtn-test-vectors:recovery-fuzzed-profile')
REC_TPL_V = 3
rq157 = e_map([(e_uint(1), e_bstr(alice2.keyhash)),
               (e_uint(2), e_bstr(carol.keyhash)),   # querier IS the verifier (design s9.1)
               (e_uint(3), e_bstr(rec_precommit)),
               (e_uint(4), e_bstr(rec_profile)),
               (e_uint(5), e_uint(REC_TPL_V)),
               (e_uint(7), e_bstr(carol.keyhash))])  # field 7 = field 2 in Recovery
rec_qid = H(rq157)
rec_query = e_map([(e_uint(1), e_bstr(alice2.keyhash)),
                   (e_uint(2), e_bstr(carol.keyhash)),
                   (e_uint(3), e_bstr(rec_precommit)),
                   (e_uint(4), e_bstr(rec_profile)),
                   (e_uint(5), e_uint(REC_TPL_V)),
                   (e_uint(6), e_bstr(rec_qid)),
                   (e_uint(7), e_bstr(carol.keyhash))])

# Consent: COSE_Sign1, Ed25519 only, no kid; payload = RAW 32 bytes of query_id.
con_prot = sig_protected(-8)
con_tbs = sig_structure_sign1(con_prot, AAD_CONSENT, rec_qid)
con_sig = alice2.sign(con_tbs)
rec_consent = e_arr([e_bstr(con_prot), b'\xa0', NULL, e_bstr(con_sig)])

# Response fields 1-8 and 10 (field 6 absent: basis 1), the field-9 payload.
resp_unsigned_pairs = [(e_uint(1), e_bstr(carol.keyhash)),
                       (e_uint(2), e_bstr(alice2.keyhash)),
                       (e_uint(3), e_bstr(rec_qid)),
                       (e_uint(4), e_uint(0)),          # match
                       (e_uint(5), e_uint(1)),          # personal_knowledge
                       (e_uint(7), rec_consent),
                       (e_uint(8), e_bstr(alice.keyhash)),
                       (e_uint(10), e_uint(0))]         # known - MUST be 0 (s4.1)
resp_payload = e_map(resp_unsigned_pairs)

# Field 9: hybrid COSE_Sign by carol, untagged, detached, no kid, empty outer.
vr_entries = []
for alg in (-8, -49):
    prot = sig_protected(alg)
    tbs = sig_structure_sign(prot, AAD_VERIFIER, resp_payload)
    sig = carol.sign(tbs) if alg == -8 else carol.sign_pq(tbs)
    vr_entries.append(cose_signature_entry(prot, sig))
rec_vr_sig = e_arr([e_bstr(b''), b'\xa0', NULL, e_arr(vr_entries)])
rec_response = e_map(resp_unsigned_pairs[:5]
                     + [(e_uint(7), rec_consent),
                        (e_uint(8), e_bstr(alice.keyhash)),
                        (e_uint(9), rec_vr_sig),
                        (e_uint(10), e_uint(0))])

# Old-key successor proof: hybrid COSE_Sign by alice's OLD key over the array.
succ_stmt = e_arr([e_bstr(alice.keyhash), e_bstr(alice2.keyhash), e_bstr(bob.keyhash)])
succ_entries = []
for alg in (-8, -49):
    prot = sig_protected(alg)
    tbs = sig_structure_sign(prot, AAD_SUCCESSOR, succ_stmt)
    sig = alice.sign(tbs) if alg == -8 else alice.sign_pq(tbs)
    succ_entries.append(cose_signature_entry(prot, sig))
rec_succ = e_arr([e_bstr(b''), b'\xa0', NULL, e_arr(succ_entries)])

rec_block = e_map([(e_uint(1), e_bstr(alice.keyhash)),
                   (e_uint(2), e_arr([rec_response])),
                   (e_uint(3), rec_succ)])

TS_RECOVER = TS_DEPART + 90 * 86400
rec_loc = locator(bob.keyhash, path([6]), seqno(11, 0))  # opens series: counter 0
rec_body = e_map([
    (e_uint(0), backptrs([genesis(alice2.keyhash)], [adopt_txid])),
    (e_uint(1), e_bstr(alice2.keyhash)),
    (e_uint(2), e_bstr(bob.keyhash)),
    (e_uint(3), rec_loc),
    (e_uint(4), e_uint(TS_RECOVER)),
    (e_uint(5), alice2.key_material),
    (e_uint(6), rec_block),
    (e_uint(7), e_bstr(depart_txid)),   # alice's old chain head: genesis -> adopt -> depart
])
rec_txid = H(rec_body)

# --- Adoption carrying a TRANSFER (§4.1 field 9): alice moves from bob to
# carol, with bob countersigning in place of a meeting.  design §6.1.1's second
# route, and the case §6.2.3's lateral and vertical shifts take.  Field 8 is
# ABSENT: the two are alternatives, and carrying both is malformed.
TS_TRANSFER = TS_RECOVER + 10 * 86400
xfer_stmt = e_arr([e_bstr(alice.keyhash),      # node_key      = field 1
                   e_bstr(bob.keyhash),        # former_patron = Transfer field 1
                   e_bstr(carol.keyhash)])     # new_patron    = field 2
xfer_entries = []
for alg in (-8, -49):
    prot = sig_protected(alg)
    tbs = sig_structure_sign(prot, AAD_TRANSFER, xfer_stmt)
    sig = bob.sign(tbs) if alg == -8 else bob.sign_pq(tbs)
    xfer_entries.append(cose_signature_entry(prot, sig))
xfer_sign = e_arr([e_bstr(b''), b'\xa0', NULL, e_arr(xfer_entries)])
xfer_block = e_map([(e_uint(1), e_bstr(bob.keyhash)), (e_uint(2), xfer_sign)])
xfer_loc = locator(carol.keyhash, path([4]), seqno(13, 0))   # opens at counter 0
xfer_body = e_map([
    (e_uint(0), backptrs([adopt_txid], [formation_txid])),
    (e_uint(1), e_bstr(alice.keyhash)),
    (e_uint(2), e_bstr(carol.keyhash)),
    (e_uint(3), xfer_loc),
    (e_uint(4), e_uint(TS_TRANSFER)),
    (e_uint(9), xfer_block),
])
xfer_txid = H(xfer_body)
xfer_env, xfer_entries_env = envelope(1, 1, xfer_body, [alice, carol])
rec_env, rec_entries = envelope(1, 1, rec_body, [alice2, bob])

emit('transactions.md', f"""
## Recovery adoption (type 1) — alice2 recovers alice's identity, adopted by bob

The complete positive `Recovery` object (canonical bar 4). Carol — a prior
counterparty of alice via the formation ceremony — meets the recovering subject
in person and recognises them (design §9.1). **The verifier is its own
querier**: the query's field 2 names carol, its pre-commitment is the recovery
meeting's own, and the subject's **new** key countersigns. Basis is
`personal_knowledge` (the ordinary case, design §9.1), so response field 6 is
absent; `selection_basis` is **0 (met)**, the only value a `Recovery` block
admits (§4.1). The old key signs the successor statement, never the Recovery
map. Field 7 presents alice's old chain head (the departure); the locator opens
series 11 at counter 0.

`VerificationQuery` ({len(rec_query)} bytes; field 6 is the SHA-256 of fields 1–5 and 7; field 7 = field 2, the querier being the verifier):

```
{hexblock(rec_query)}
```

query_id: `{hx(rec_qid)}`

Subject consent (COSE_Sign1, Ed25519 by alice2, external_aad `rhtn/1:consent`,
payload the RAW query_id bytes; {len(rec_consent)} bytes):

```
{hexblock(rec_consent)}
```

Verifier response ({len(rec_response)} bytes — field 9 is a hybrid `COSE_Sign`
by carol over canonical CBOR of fields 1–8 and 10, external_aad
`rhtn/1:verifier`):

```
{hexblock(rec_response)}
```

Successor statement (deterministic CBOR array `[prior, new, patron]`,
{len(succ_stmt)} bytes) and the old key's hybrid proof over it, external_aad
`rhtn/1:successor` ({len(rec_succ)} bytes):

```
{hexblock(succ_stmt)}
```

```
{hexblock(rec_succ)}
```

Body ({len(rec_body)} bytes):

```
{hexblock(rec_body)}
```

txid: `{hx(rec_txid)}`

Envelope bytes ({len(rec_env)} bytes — final; the envelope signers are the NEW
key and the patron, per §3.1 — the old key signs only the embedded proof):

```
{hexblock(rec_env)}
```""")

emit('transactions.md', f"""
## Adoption carrying a TRANSFER (type 1, field 9) — alice moves from bob to carol

design §6.1.1's second evidence route. Alice was adopted by bob; she now moves
to carol, and **bob countersigns in place of a meeting between alice and
carol**. This is the shape every lateral and vertical shift takes (§6.2.3).

**Field 8 is ABSENT and that is required, not incidental**: fields 8 and 9 are
alternatives, and an adoption carrying both is malformed (§4.1). A decoder that
demands a presence record on every adoption rejects this vector and with it
every ordinary move between patrons — negative-vectors.md D20.

The `Transfer` block is `Recovery`'s shape one field over: a keyhash naming the
former patron and a hybrid `COSE_Sign` by them. What it signs is **not** the
map but a `TransferStatement`, the deterministic CBOR of

```
[ {hx(alice.keyhash)}    ; node_key      = field 1
  {hx(bob.keyhash)}    ; former_patron = Transfer field 1
  {hx(carol.keyhash)} ]  ; new_patron    = field 2
```

with `external_aad = "rhtn/1:transfer"`. All three are bound for the reason the
successor statement binds three: a countersignature naming no destination would
authorise an unlimited number of moves.

TransferStatement bytes ({len(xfer_stmt)} bytes):

```
{hexblock(xfer_stmt)}
```

Body bytes ({len(xfer_body)} bytes):

```
{hexblock(xfer_body)}
```

txid: `{hx(xfer_txid)}`

Envelope bytes ({len(xfer_env)} bytes — signers are the node and the NEW
patron; the former patron signs only the embedded statement, exactly as a
recovery's old key does):

```
{hexblock(xfer_env)}
```""")

# ================================================================ records.md

emit('records.md', f"""# Standalone signed records (`wire-format.md` §7)

{PIN}

**Draft. Spec-derived, unverified by an implementation.** See
[README.md](README.md). Each **signed** §7 object is a standalone `COSE_Sign1`
under its own domain-separation tag (§1.1) — this file grows toward one
known-answer vector per signing context, and the unsigned §7 encodings are
listed apart at the end so nobody generates signatures the specification does
not define. Payload reading throughout: the deterministic CBOR
of the map of exactly the named fields (§1's fields-X–Y rule, which governs
all eight signed objects).

## Node endpoint record (§7.6) — complete, classical-only

Bob publishes one endpoint; `external_aad = "rhtn/1:endpoints"`. Fields 1–3
({len(er_payload)} bytes):

```
{hexblock(er_payload)}
```

Protected header `{{1: -8}}` → `{hx(er_protected)}`; no `kid` — field 1 names
the signer (§3.5).

`Sig_structure`:

```
{hexblock(er_tbs)}
```

Ed25519 signature by bob:

```
{hexblock(er_sig)}
```

Complete `EndpointRecord` ({len(endpoint_record)} bytes):

```
{hexblock(endpoint_record)}
```

## The equal-seqno conflict pair (§7.6, §7.7.3)

A second record by bob — **same seqno `[9, 2]`, different endpoint set**, its
signature equally valid ({len(er2)} bytes):

```
{hexblock(er2)}
```

Each record is individually well-formed; **holding both is the malformed
condition** — an equal `seqno` carrying different contents is a disagreement,
never a tie to break, and a reader MUST NOT prefer either (negative suite,
V6). A subject advances its own counter, so the pair can only mean equivocation
or a key in two hands. **The aftermath is ruled** (wire §10.1, 2026-09-02):
malformed names the pair — on discovery the holder retains neither as current,
forwards nothing further for that `(subject, seqno)`, and repairs by
re-resolution. First-wins would let arrival order split the network's view.

## An `EndpointRecord` carrying an unknown extension — MUST ACCEPT (D8)

§1's coverage rule is global: the signed payload is the map of the named fields
**plus any unknown extension keys**. This record carries `99: h'c0ffee'` inside
the signed payload of the standalone `COSE_Sign1` path — the same property
D2/E10 prove for the envelope path. Mutate the extension value and the
signature fails (E13). Complete record ({len(er_ext)} bytes):

```
{hexblock(er_ext)}
```

## The §7 object model — signed contexts versus unsigned encodings

**Not every §7 object is signed, and the target list must not imply otherwise**
(third review). One known-answer signature per *signing context* remains the
goal — but only for objects that have one:

- **Signed — complete 2026-09-02.** Every domain-separation context now has a
  known-answer signature: locator and endpoints (below), currency, catalog,
  abuse, anchor, subtree-ack and prekey (the bar-8 section at the end of this
  file, each with its wrong-signer analogue), and successor, verifier and
  consent (the recovery adoption and normal record in `transactions.md`).
  The envelope tag is every transaction's.
- **Unsigned message encodings — landed 2026-09-02 in `messages.md`**
  (canonical bar 9): every family in this inventory has a positive
  known-answer encoding there — session frames, topology push and memo,
  resolution, archive, prekey, query-plus-consent, catalog, resource
  request/response, registration and reply, currency, the capture key grant
  and the late-response wrapper — plus the session-trace table. *The
  inventory remains hand-maintained until the corpus format enumerates it
  mechanically from the schemas (bar 6).*""")

# ================================================================ verifier-selection.md

emit('verifier-selection.md', f"""# Verifier selection — the reasonableness criterion

{PIN}

**Draft. Spec-derived, unverified by an implementation.** See
[README.md](README.md). *The nonce-commitment, seed and hash-rank vectors that
lived here were retired 2026-09-01 with deterministic selection
(`wire-format.md` §5): selection is by recognition — the selector's own
judgment over the handed bundle — and produces nothing recomputable to vector.
What remains checkable is the arithmetic below.*

## The reasonableness criterion (§5.2)

`required(subject) = min(floor(n / 2), 10, |candidates|)` (rows generated from
the formula itself):

| n | candidates | required |
|---|---|---|
{chr(10).join(f'| {n} | {c} | {min(n // 2, 10, c)} |' for n, c in
              [(0, 0), (1, 1), (2, 1), (3, 2), (7, 5), (20, 1), (25, 12)])}

## Window boundaries (§5.3)

With `started_at = {TS_START}`, the 730-day window is
`{TS_START} − 63072000 = {TS_START - 63072000} < finalized_at < {TS_START}` —
**exclusive at both ends: previously completed ceremonies only** (§5.3):

| Prior record `finalized_at` | Counted? |
|---|---|
| {TS_START - 63072000} (exactly 730 days) | no — boundary instant is out |
| {TS_START - 63072000 + 1} | yes |
| {TS_START - 1} | yes |
| {TS_START} | no — not before `started_at` |

## The curated bundle (§5.4) — canonical bar 3

Alice hands bob a bundle at a hypothetical next ceremony,
`started_at = {TS_EVAL}` (window floor {TS_EVAL - 63072000}, exclusive both
ends; every record below finalized inside it). **Six entries handed, one a
duplicate and one non-verifying**:

| # | Handed | Qualifies? |
|---|---|---|
| 1 | formation record `{hx(formation_txid)[:16]}…` (alice–carol) | yes — formations count (§5.3) |
| 2 | ac1 `{hx(ac1_txid)[:16]}…` (alice–carol) | yes |
| 3 | ac2 `{hx(ac2_txid)[:16]}…` (alice–carol) | yes |
| 4 | ac1 again, byte-identical | counts **once** — duplicates dedupe by txid (§5.3) |
| 5 | normal record `{hx(npr_txid)[:16]}…` (alice–bob) | yes — but **bob is the current counterparty**, never a candidate for his own verification (§5.3) |
| 6 | the formation envelope with any signed-body byte mutated | **not in the pool** — a record that fails its checks contributes nothing; there is no "incomplete", it is simply absent (§5.4) |

The arithmetic, stated so a harness can recompute it:

```
n = 4          (distinct qualifying records: 1, 2, 3, 5)
candidates = 1 (distinct prior counterparties {{carol, bob}} minus the current counterparty bob)
required = min(floor(4 / 2), 10, 1) = 1
```

**Witness-only does not qualify** (§5.3): the same normal record handed by
**w1** as subject names w1 only in field 4 — a witnessed ceremony's
participants met each other, not the witness. For w1 that bundle yields
`n = 0, candidates = 0, required = 0`.

**Understatement is free and self-defeating** (§5.4): alice handing only
`[ac1, npr]` yields `n = 2, candidates = 1, required = min(1, 10, 1) = 1` —
a smaller claim, a thinner record, and both withheld records remain
individually valid wherever else she presents them.

**The reasonableness reading** (§5.2): an evaluator comparing a finalized
record's response count against `required` learns whether the ceremony was
diligent — never whether the subject's history is complete. The criterion
gates nothing; the finalization must-accepts in `transactions.md` are the
positive proof.""")

# --- Canonical bar 8: the remaining signed contexts, one known-answer
# signature per domain-separation tag, each with a wrong-signer analogue
# (bar 10) signed by carol's key while the object names its true signer.
def sign1_slot(pairs, sig_slot, aad, signer):
    """Build {pairs..., sig_slot: COSE_Sign1(payload = map without sig_slot)}."""
    payload = e_map(pairs)
    prot = sig_protected(-8)
    tbs = sig_structure_sign1(prot, aad, payload)
    cose = e_arr([e_bstr(prot), b'\xa0', NULL, e_bstr(signer.sign(tbs))])
    allp = sorted(pairs + [(e_uint(sig_slot), cose)], key=lambda p: p[0])
    return e_map(allp)

AAD_CURRENCY = b'rhtn/1:currency'
AAD_CATALOG = b'rhtn/1:catalog'
AAD_ABUSE = b'rhtn/1:abuse'
AAD_ANCHOR = b'rhtn/1:anchor'
AAD_SUBTREE = b'rhtn/1:subtree-ack'
AAD_PREKEY = b'rhtn/1:prekey'
AAD_DELEGATION = b'rhtn/1:delegation'
DELEGATION_WINDOW = 172_800   # exactly 48 hours (wire §8.2)

def sign1_slot_extra(pairs, sig_slot, aad, signer, extra):
    """sign1_slot, plus fields outside the signature (a currency
    attestation's field 8, the stapled delegation)."""
    payload = e_map(pairs)
    prot = sig_protected(-8)
    tbs = sig_structure_sign1(prot, aad, payload)
    cose = e_arr([e_bstr(prot), b'\xa0', NULL, e_bstr(signer.sign(tbs))])
    allp = sorted(pairs + [(e_uint(sig_slot), cose)] + extra, key=lambda p: p[0])
    return e_map(allp)

def delegation(named, tk, not_before, signer=None, window=DELEGATION_WINDOW, algs=(-8, -49)):
    """wire §8.2: the identity `named` signs, hybrid, over the transport key
    `tk` and a 48-hour window.  `signer` other than `named` is the
    wrong-signer analogue; `algs=(-8,)` the classical-only malformation."""
    signer = signer or named
    pairs = [(e_uint(1), e_bstr(tk.pub)),
             (e_uint(2), e_bstr(named.keyhash)),
             (e_uint(3), e_uint(not_before)),
             (e_uint(4), e_uint(not_before + window))]
    payload = e_map(pairs)
    entries = []
    for alg in algs:
        prot = sig_protected(alg)
        tbs = sig_structure_sign(prot, AAD_DELEGATION, payload)
        sig = signer.sign(tbs) if alg == -8 else signer.sign_pq(tbs)
        entries.append(cose_signature_entry(prot, sig))
    cose = e_arr([e_bstr(b''), b'\xa0', NULL, e_arr(entries)])
    return e_map(pairs + [(e_uint(5), cose)])

TS_REC = TS_C2 + 10 * 86400
res = IDS['c5']   # the resource's own keys; bob owns and hosts it

cur_pairs = [(e_uint(1), e_bstr(alice.keyhash)),
             (e_uint(2), e_bstr(alice.keyhash)),
             (e_uint(3), e_uint(TS_REC)),
             (e_uint(4), e_uint(TS_REC + 10 * 3600)),
             (e_uint(5), e_uint(0)),
             (e_uint(6), e_bstr(bob.keyhash))]
# bob's instance holds no seed: it signs under its delegated key and the
# staple carries bob's delegation for that key as field 8 (wire §7.1, §8.2)
deleg_bob = delegation(bob, TK['bob-instance'], TS_REC - 3600)
deleg_carol = delegation(carol, TK['carol-instance'], TS_ADOPT)
deleg_alice_desktop = delegation(alice, TK['alice-desktop'], TS_REC - 3600)
deleg_wrong = delegation(bob, TK['bob-instance'], TS_REC - 3600, signer=carol)
deleg_short = delegation(bob, TK['bob-instance'], TS_REC - 3600, window=DELEGATION_WINDOW - 1)
deleg_long = delegation(bob, TK['bob-instance'], TS_REC - 3600, window=DELEGATION_WINDOW + 1)
deleg_classical = delegation(bob, TK['bob-instance'], TS_REC - 3600, algs=(-8,))
currency = sign1_slot_extra(cur_pairs, 7, AAD_CURRENCY, TK['bob-instance'], [(e_uint(8), deleg_bob)])
# an issuer that holds its own seed signs under its identity and carries no field 8
currency_identity = sign1_slot(cur_pairs, 7, AAD_CURRENCY, bob)
currency_wrong = sign1_slot(cur_pairs, 7, AAD_CURRENCY, carol)
# field 8 names bob's instance; field 7 was made under another device's key
currency_deleg_mismatch = sign1_slot_extra(cur_pairs, 7, AAD_CURRENCY, TK['alice-desktop'], [(e_uint(8), deleg_bob)])
# field 2 differs from field 1: the honest rotation report, a must-accept
# (wire §7.1 [author, 2026-09-18]; CUR-20)
cur_succ_pairs = [cur_pairs[0], (e_uint(2), e_bstr(alice2.keyhash))] + cur_pairs[2:]
currency_successor = sign1_slot_extra(cur_succ_pairs, 7, AAD_CURRENCY, TK['bob-instance'], [(e_uint(8), deleg_bob)])

cat_pairs = [(e_uint(1), e_bstr(res.keyhash)),
             (e_uint(2), e_bstr(bob.keyhash)),
             (e_uint(3), e_tstr('rhtn-forum')),
             (e_uint(4), e_tstr('The Reading Room')),
             (e_uint(5), e_bstr(b'quic://198.51.100.7:4433')),
             (e_uint(7), e_bstr(b'v=1')),
             (e_uint(9), e_uint(1))]
catalog = sign1_slot(cat_pairs, 8, AAD_CATALOG, bob)
catalog_wrong = sign1_slot(cat_pairs, 8, AAD_CATALOG, carol)

abuse_pairs = [(e_uint(1), e_bstr(res.keyhash)),
               (e_uint(2), e_uint(TS_REC + 3600)),
               (e_uint(3), e_uint(2)),
               (e_uint(4), e_bstr(b'burst of 9k requests/min'))]
abuse = sign1_slot(abuse_pairs, 5, AAD_ABUSE, res)
abuse_wrong = sign1_slot(abuse_pairs, 5, AAD_ABUSE, carol)

np1 = e_map([(e_uint(1), e_bstr(bytes([198, 51, 100, 7]))),
             (e_uint(2), e_uint(64500))])   # port ABSENT: default 7431, written out is malformed
anchor_pairs = [(e_uint(1), e_bstr(bob.keyhash)),
                (e_uint(2), e_arr([np1])),
                (e_uint(3), e_uint(111)),
                (e_uint(4), seqno(1, 7))]
anchor = sign1_slot(anchor_pairs, 5, AAD_ANCHOR, bob)
anchor_wrong = sign1_slot(anchor_pairs, 5, AAD_ANCHOR, carol)

ack_pairs = [(e_uint(1), e_bstr(adopt_txid)),
             (e_uint(2), e_bstr(carol.keyhash)),
             (e_uint(3), e_bstr(alice.keyhash)),
             (e_uint(4), e_uint(TS_ADOPT + 600))]
# the grandpatron's NODE signs, under its delegated key (wire §7.5); a
# receiver checks it against carol's delegation held from the topology class
ack = sign1_slot(ack_pairs, 5, AAD_SUBTREE, TK['carol-instance'])
ack_wrong = sign1_slot(ack_pairs, 5, AAD_SUBTREE, bob)

pk_material = H(b'rhtn-test-vectors:prekey-material-1') + H(b'rhtn-test-vectors:prekey-material-2')
pk_pairs = [(e_uint(1), e_bstr(alice.keyhash)),
            (e_uint(2), e_uint(1)),
            (e_uint(3), e_bstr(pk_material)),
            (e_uint(4), e_uint(TS_REC)),
            (e_uint(5), e_bstr(alice.ed_pub))]   # the device: alice's phone, the seed-holding device
prekey = sign1_slot(pk_pairs, 6, AAD_PREKEY, alice)
prekey_wrong = sign1_slot(pk_pairs, 6, AAD_PREKEY, carol)
# alice's desktop: its own material, named by its transport key, signed by
# alice's identity on the phone (wire §7.8, design §23.3)
pk_desk_material = H(b'rhtn-test-vectors:prekey-material-desktop-1') + H(b'rhtn-test-vectors:prekey-material-desktop-2')
pk_desk_pairs = pk_pairs[:2] + [(e_uint(3), e_bstr(pk_desk_material)), pk_pairs[3], (e_uint(5), e_bstr(TK['alice-desktop'].pub))]
prekey_desktop = sign1_slot(pk_desk_pairs, 6, AAD_PREKEY, alice)

emit('records.md', f"""
## The remaining signed contexts (canonical bar 8)

One known-answer `COSE_Sign1` per domain-separation tag. Every payload is the
object's canonical map **without its signature slot**; protected header
`{{1: -8}}`, no kid (the object names its signer), empty unprotected, detached
payload. Each is followed by its **wrong-signer analogue** (canonical bar 10):
byte-identical fields, the signature cryptographically valid under a key the
object does **not** name — the binding, not the mathematics, is the defect
(S23's rule). And each signature is bound to its tag: verified under any other
context's `external_aad`, it MUST fail — the cross-context substitution family
(S24).

**Currency attestation** — subject alice, issuer bob (role 0, patron), ~10 h
expiry, signed under bob's instance's delegated key with bob's delegation
stapled as field 8, outside the signature (§7.1, §8.2) ({len(currency)} bytes;
wrong-signer: carol's identity, field 8 absent):

```
{hexblock(currency)}
```

```
{hexblock(currency_wrong)}
```

The same attestation from an issuer holding its own seed, signed under bob's
identity with no field 8 ({len(currency_identity)} bytes):

```
{hexblock(currency_identity)}
```

**Catalog entry** — resource c5, owner bob, `connect_scope` absent (no
prediction offered), `data_practice` 1; the endpoint is the SRV analogue
({len(catalog)} bytes; wrong-signer: carol):

```
{hexblock(catalog)}
```

```
{hexblock(catalog_wrong)}
```

**Abuse report** — the resource c5 reports excessive load to its own owner;
field 1 is both the resource and the signer ({len(abuse)} bytes;
wrong-signer: carol, violating the field-1-equals-signer binding):

```
{hexblock(abuse)}
```

```
{hexblock(abuse_wrong)}
```

**Anchor table entry** — bob, one `NetworkPoint` with the port **absent**
(default 7431; writing it out is malformed, §1's default-omission rule),
subtree size 111 ({len(anchor)} bytes; wrong-signer: carol):

```
{hexblock(anchor)}
```

```
{hexblock(anchor_wrong)}
```

**Subtree acknowledgement** — carol as grandpatron acknowledges alice's
adoption by bob (the tree above bob is asserted for the fixture, not built),
signed by carol's NODE under its delegated key `carol-instance` (§7.5), which a
receiver checks against carol's delegation held from the topology class
({len(ack)} bytes; wrong-signer: bob, who is the patron and exactly the party
that must not substitute for the grandpatron):

```
{hexblock(ack)}
```

```
{hexblock(ack_wrong)}
```

**Prekey bundle** — subject alice, construction 1 (PQXDH), 64 bytes of opaque
reusable material, field 5 naming the device (alice's phone, the seed-holding
device's classical key), the signature at field 6 over fields 1 to 5
({len(prekey)} bytes; wrong-signer: carol):

```
{hexblock(prekey)}
```

```
{hexblock(prekey_wrong)}
```

The bundle of alice's **desktop**, a delegated device: its own material, field
5 its transport key, signed by alice's identity ({len(prekey_desktop)} bytes):

```
{hexblock(prekey_desktop)}
```

**Transport delegation** (§8.2) — bob delegates to his instance's transport key
`bob-instance` for exactly 48 hours; field 5 is a **hybrid `COSE_Sign`** over
fields 1 to 4 under `rhtn/1:delegation`, the delegating identity being hybrid
({len(deleg_bob)} bytes; wrong-signer: carol, over the same fields naming bob):

```
{hexblock(deleg_bob)}
```

```
{hexblock(deleg_wrong)}
```""")

# --- Canonical bar 9: the unsigned message families. Every framed message
# family gets a positive known-answer encoding; session semantics get trace
# rows. frame = u32-be length || CBOR [type, body] on stream 0 (64 KiB) and
# bidirectional streams (256 KiB).
def frame(t, body):
    c = e_arr([e_uint(t), body])
    return len(c).to_bytes(4, 'big') + c

CAP_BATCH = int.from_bytes(H(b'rhtn/cap:max-archive-batch')[:8], 'big')
CAP_GREASE = int.from_bytes(H(b'rhtn-test-vectors:grease-id')[:8], 'big')
caps_client = e_map(sorted([(e_uint(CAP_BATCH), e_bstr(b'\x01\x00')),
                            (e_uint(CAP_GREASE), e_bstr(H(b'rhtn-test-vectors:grease-value')[:8]))],
                           key=lambda p: p[0]))
caps_server = e_map([(e_uint(CAP_BATCH), e_bstr(b'\x00\x40'))])

NONCE = lambda tag: H(b'rhtn-test-vectors:nonce:' + tag)[:16]

f_attach = frame(1, e_map([(e_uint(1), e_bstr(alice.keyhash)),
                           (e_uint(2), currency),
                           (e_uint(3), caps_client)]))
sib_ref = e_map([(e_uint(1), e_bstr(carol.keyhash)),
                 (e_uint(2), e_arr([np1])),
                 (e_uint(3), carol.key_material)])
f_ack = frame(2, e_map([(e_uint(1), e_uint(0)),
                        (e_uint(2), e_arr([sib_ref])),
                        (e_uint(3), e_uint(300)),
                        (e_uint(4), e_uint(0)),
                        (e_uint(5), caps_server)]))
f_hb = frame(3, e_map([(e_uint(1), e_uint(0)), (e_uint(2), e_uint(TS_REC))]))
f_sib = frame(4, e_map([(e_uint(1), e_arr([sib_ref]))]))
f_sib_none = frame(4, e_map([]))
f_push = frame(5, e_map([(e_uint(1), e_uint(0)), (e_uint(2), e_bstr(adopt_env))]))
f_memo = frame(6, e_map([(e_uint(1), e_bstr(bob.keyhash)),
                         (e_uint(2), root_loc),
                         (e_uint(3), e_uint(4)),
                         (e_uint(4), e_uint(TS_ADOPT)),
                         (e_uint(5), e_bstr(alice.keyhash))]))
f_memo_empty = frame(6, e_map([(e_uint(1), e_bstr(bob.keyhash)),
                               (e_uint(2), root_loc),
                               (e_uint(3), e_uint(4)),
                               (e_uint(4), e_uint(TS_DEPART))]))

f_resolve = frame(1, e_map([(e_uint(1), e_bstr(alice.keyhash)),
                            (e_uint(2), e_bstr(bob.keyhash)),
                            (e_uint(3), p_odd),
                            (e_uint(4), e_bstr(NONCE(b'resolve')))]))
serving = e_map([(e_uint(1), e_bstr(bob.keyhash)),
                 (e_uint(2), e_arr([np1])),
                 (e_uint(3), path([])),
                 (e_uint(4), bob.key_material)])
r_serving = e_map([(e_uint(1), e_bstr(NONCE(b'resolve'))),
                   (e_uint(2), e_uint(0)),
                   (e_uint(3), serving)])
r_referral = e_map([(e_uint(1), e_bstr(NONCE(b'resolve'))),
                    (e_uint(2), e_uint(2)),
                    (e_uint(5), e_map([(e_uint(1), e_bstr(carol.keyhash)),
                                       (e_uint(2), e_arr([np1])),
                                       (e_uint(3), e_uint(2))]))])
r_fail = e_map([(e_uint(1), e_bstr(NONCE(b'resolve'))),
                (e_uint(2), e_uint(1)),
                (e_uint(4), e_uint(0))])

f_archive = frame(2, e_map([(e_uint(1), e_bstr(alice.keyhash)),
                            (e_uint(3), e_uint(16)),
                            (e_uint(5), e_bstr(NONCE(b'archive')))]))
r_archive = e_map([(e_uint(1), e_bstr(NONCE(b'archive'))),
                   (e_uint(2), e_arr([adopt_env])),
                   (e_uint(3), b'\xf4')])
# a presence record arrives in the presented form (§7.9 ArchiveEntry): an
# array beside the envelope maps, no discriminator needed
r_archive_presented = e_map([(e_uint(1), e_bstr(NONCE(b'archive-presented'))),
                             (e_uint(2), e_arr([npr_full])),
                             (e_uint(3), b'\xf4')])
# a request walking back from a two-txid frontier, past a merge
f_archive_frontier2 = frame(2, e_map([(e_uint(1), e_bstr(alice.keyhash)),
                                      (e_uint(2), e_arr([e_bstr(adopt_txid), e_bstr(depart_txid)])),
                                      (e_uint(3), e_uint(16)),
                                      (e_uint(5), e_bstr(NONCE(b'archive-2')))]))

f_pk1 = frame(3, e_map([(e_uint(1), e_bstr(alice.keyhash)),
                        (e_uint(2), e_uint(1)),
                        (e_uint(3), e_bstr(NONCE(b'prekey'))),
                        (e_uint(4), e_bstr(alice.ed_pub))]))   # a one-time key is one device's
f_pk1_nodevice = frame(3, e_map([(e_uint(1), e_bstr(alice.keyhash)),
                                 (e_uint(2), e_uint(1)),
                                 (e_uint(3), e_bstr(NONCE(b'prekey')))]))
batch_khs = sorted([alice.keyhash, bob.keyhash])
f_pkb = frame(3, e_map([(e_uint(1), e_arr([e_bstr(k) for k in batch_khs])),
                        (e_uint(2), e_bstr(NONCE(b'prekey-batch')))]))
r_pk = e_map([(e_uint(1), e_bstr(NONCE(b'prekey'))),
              (e_uint(2), e_arr([prekey])),
              (e_uint(3), e_bstr(H(b'rhtn-test-vectors:one-time-prekey')))])
# every device's bundle, for a request naming none
r_pk_all = e_map([(e_uint(1), e_bstr(NONCE(b'prekey-all'))),
                  (e_uint(2), e_arr([prekey, prekey_desktop]))])
r_pk_8 = e_map([(e_uint(1), e_bstr(NONCE(b'prekey-8'))), (e_uint(2), e_arr([prekey] * 8))])
r_pk_9 = e_map([(e_uint(1), e_bstr(NONCE(b'prekey-9'))), (e_uint(2), e_arr([prekey] * 9))])
r_pk_fail = e_map([(e_uint(1), e_bstr(NONCE(b'prekey'))),
                   (e_uint(4), e_uint(0))])

f_query = frame(4, e_arr([npr_query0, consent_over(npr_q0, alice), e_uint(0)]))
f_catq = frame(5, e_map([(e_uint(1), e_tstr('rhtn-forum')),
                         (e_uint(2), e_bstr(NONCE(b'catalog')))]))
r_cat = e_map([(e_uint(1), e_bstr(NONCE(b'catalog'))),
               (e_uint(2), e_arr([catalog]))])

http_req = (b'GET /threads/42 HTTP/1.1\r\nhost: reading-room.internal\r\n'
            b'rhtn-principal: ' + hx(H(b'rhtn-test-vectors:pairwise-principal'))[:43].encode()
            + b'\r\n\r\n')
f_rr = frame(6, e_map([(e_uint(1), e_bstr(res.keyhash)),
                       (e_uint(2), e_bstr(http_req))]))
http_resp = b'HTTP/1.1 200 OK\r\ncontent-length: 2\r\n\r\nok'
r_rr = e_map([(e_uint(1), e_uint(0)), (e_uint(2), e_bstr(http_resp))])
r_rr_refused = e_map([(e_uint(1), e_uint(1))])

f_reg = frame(7, e_map([(e_uint(1), catalog),
                        (e_uint(3), e_bstr(NONCE(b'register')))]))
f_cur = frame(8, e_map([(e_uint(1), e_bstr(alice.keyhash)),
                        (e_uint(2), e_bstr(NONCE(b'currency')))]))
r_cur = e_map([(e_uint(1), e_bstr(NONCE(b'currency'))),
               (e_uint(2), e_uint(0)),
               (e_uint(3), currency)])
r_cur_no = e_map([(e_uint(1), e_bstr(NONCE(b'currency'))),
                  (e_uint(2), e_uint(1))])

# §7.10: the four a client hands the node serving it.  Every one is about
# the client that sends it, so none of them names a subject, an owner or a
# sender: the node takes that from the session.
f_pub = frame(9, e_map([(e_uint(1), prekey),
                        (e_uint(2), e_bstr(NONCE(b'publication')))]))
otks = [H(b'rhtn-test-vectors:one-time-deposit:' + bytes([i])) for i in range(3)]
f_dep = frame(10, e_map([(e_uint(1), e_arr([e_bstr(k) for k in otks])),
                         (e_uint(2), e_bstr(NONCE(b'deposit')))]))
f_relay = frame(11, e_map([(e_uint(1), e_bstr(carol.keyhash)),
                           (e_uint(2), e_bstr(H(b'rhtn-test-vectors:relayed-ciphertext'))),
                           (e_uint(3), e_bstr(NONCE(b'relay'))),
                           (e_uint(4), e_bstr(carol.ed_pub))]))   # carol's phone: the device the session is with
f_relay_nodevice = frame(11, e_map([(e_uint(1), e_bstr(carol.keyhash)),
                                    (e_uint(2), e_bstr(H(b'rhtn-test-vectors:relayed-ciphertext'))),
                                    (e_uint(3), e_bstr(NONCE(b'relay')))]))
f_wake = frame(12, e_map([(e_uint(1), e_bstr(NONCE(b'wake'))),
                          (e_uint(2), e_tstr('https://push.example/rhtn/a3f9')),
                          (e_uint(3), e_bstr(H(b'rhtn-test-vectors:wake-endpoint-key'))),
                          (e_uint(4), e_uint(1_800_090_000))]))
# a withdrawal: field 2 absent, and 3 and 4 absent with it
f_wake_off = frame(12, e_map([(e_uint(1), e_bstr(NONCE(b'wake-withdraw')))]))
r_sub = e_map([(e_uint(1), e_bstr(NONCE(b'publication'))), (e_uint(2), e_uint(0))])
r_sub_bound = e_map([(e_uint(1), e_bstr(NONCE(b'deposit'))), (e_uint(2), e_uint(2))])
# what the node then delivers for a relay submission: the submitter in
# front of the ciphertext, an array and not a map
relayed = e_arr([e_bstr(alice.keyhash), e_bstr(H(b'rhtn-test-vectors:relayed-ciphertext'))])

f_attach_deleg = frame(1, e_map([(e_uint(1), e_bstr(alice.keyhash)),
                                 (e_uint(2), currency),
                                 (e_uint(3), caps_client),
                                 (e_uint(4), deleg_alice_desktop)]))
f_ack_deleg = frame(2, e_map([(e_uint(1), e_uint(0)),
                              (e_uint(2), e_arr([sib_ref])),
                              (e_uint(3), e_uint(300)),
                              (e_uint(4), e_uint(0)),
                              (e_uint(5), caps_server),
                              (e_uint(6), deleg_bob)]))
f_deleg = frame(7, deleg_bob)

_msg_pairs = [
    ('Attach (frame 1) — carrying the currency attestation and two capability parameters, one greased', f_attach),
    ('AttachAck (frame 2) — primary mode, one sibling with full KeyMaterial, heartbeat 300 s', f_ack),
    ('Heartbeat (frame 3) — counter 0', f_hb),
    ('SiblingUpdate (frame 4) — replacing the list', f_sib),
    ('SiblingUpdate, no siblings — field 1 absent, the empty map', f_sib_none),
    ('TopologyPush (frame 5) — kind 0, carrying the first adoption envelope byte-for-byte', f_push),
    ('TopologyMemo (frame 6) — slot 4 occupied by alice', f_memo),
    ('TopologyMemo — the same slot EMPTIED (field 5 absent: departure or disavowal)', f_memo_empty),
    ('ResolveRequest (request 1)', f_resolve),
    ('ArchiveRequest (request 2) — head ABSENT: the recovery case, a must-accept', f_archive),
    ('PrekeyRequest (request 3) — reusable plus a one-time key', f_pk1),
    ('PrekeyBatchRequest (request 3) — two subjects, ascending', f_pkb),
    ('Query, consent, and the selector\'s selection_basis claim (request 4) — the normal record\'s worked query', f_query),
    ('CatalogQuery (request 5) — filtered to `rhtn-forum`', f_catq),
    ('ResourceRequest (request 6) — an HTTP/1.1 GET inside the frame', f_rr),
    ('ResourceRegistration (request 7) — the signed catalog entry, requested scope absent', f_reg),
    ('CurrencyRequest (request 8)', f_cur),
    ('PrekeyPublication (request 9) — the subject\'s own bundle, published by the subject', f_pub),
    ('OneTimeDeposit (request 10) — three keys, opaque to the node', f_dep),
    ('RelaySubmission (request 11) — ciphertext for carol, which this node cannot read', f_relay),
    ('WakeRegistration (request 12) — endpoint, its key, and when the client expects it to lapse', f_wake),
    ('WakeRegistration — the WITHDRAWAL: field 2 absent, and fields 3 and 4 absent with it', f_wake_off),
    # appended 2026-09-22, after the request families, so every earlier fixture keeps its id
    ('Attach — from a DELEGATED DEVICE, alice\'s desktop: field 4 carries alice\'s delegation naming the key the handshake presented (§8.2)', f_attach_deleg),
    ('AttachAck — from an INSTANCE: field 6 carries bob\'s delegation, the normal case (§8.2)', f_ack_deleg),
    ('Delegation (frame 7) — a delegated peer\'s first frame on a connection that opens no session (§8.0, §8.2)', f_deleg),
]
CONTROL_NAMES = ('Attach', 'Heartbeat', 'SiblingUpdate', 'TopologyPush', 'TopologyMemo', 'Delegation')
def hkdf_sha256(ikm, info, length=32):
    prk = hmac.new(b'\x00' * 32, ikm, hashlib.sha256).digest()  # empty salt
    out, t, i = b'', b'', 1
    while len(out) < length:
        t = hmac.new(prk, t + info + bytes([i]), hashlib.sha256).digest()
        out += t; i += 1
    return out[:length]

# k_capture for the capture c1 holds of alice, sealed at their PRIOR meeting's
# ceremony (pc1) - released against the CURRENT query. Field 1 names the prior
# record; the current record's txid cannot appear, since it embeds the very
# response this grant unlocks [0.6 phase 2, 2026-09-02].
demo_seed = H(b'rhtn-test-vectors:capture-seed:alice:c1')
demo_k = hkdf_sha256(demo_seed, b'rhtn/1:capture' + alice.keyhash
                     + IDS['c1'].keyhash + pc1_precommit)
kg = e_map([(e_uint(1), e_bstr(pc1_txid)),
            (e_uint(2), e_bstr(npr_q0)),
            (e_uint(3), e_bstr(demo_k))])
late = e_map([(e_uint(1), e_bstr(npr_txid)),
              (e_uint(2), e_bstr(alice.keyhash)),
              (e_uint(3), classical_response(IDS['c4'], alice, npr_q0,
                                             consent_over(npr_q0, alice),
                                             2, basis=1, sb=0))])
r_reg = e_map([(e_uint(1), e_bstr(NONCE(b'register'))), (e_uint(2), e_uint(0))])

_reply_pairs = [
    ('ResolveReply — serving (code 0, `ServingInfra` with KeyMaterial and empty residual path)', r_serving),
    ('ResolveReply — referral (code 2, advances 2)', r_referral),
    ('ResolveReply — failure (code 1, reason 0: no such child)', r_fail),
    ('ArchiveReply — one envelope, no more remaining', r_archive),
    ('PrekeyReply — bundle plus one-time key', r_pk),
    ('PrekeyReply — failure 0, unknown subject', r_pk_fail),
    ('CatalogReply — one entry, no truncation', r_cat),
    ('ResourceResponse — 0 delivered, HTTP/1.1 200 inside', r_rr),
    ('ResourceResponse — 1 refused, field 2 absent', r_rr_refused),
    ('CurrencyReply — attestation follows', r_cur),
    ('CurrencyReply — 1 cannot issue', r_cur_no),
    ('ResourceRegistrationReply — 0 recorded', r_reg),
    ('SubmissionReply — 0 accepted, echoing the publication\'s nonce', r_sub),
    ('SubmissionReply — 2 over a bound this node applies, echoing the deposit\'s nonce', r_sub_bound),
    # appended 2026-09-22
    ('PrekeyReply — every device\'s bundle, alice\'s phone and desktop, for a request naming no device (§7.8)', r_pk_all),
    ('ArchiveReply — one PRESENTED presence record, the holder\'s disclosure choice carried (§7.9)', r_archive_presented),
]
_e2e_pairs = [
    ('KeyGrant — the key sealing the capture c1 holds of alice from their PRIOR meeting (field 1 names that record), released against the normal record\'s first query', kg),
    ('LateResponse — the normal record supplemented by a late `inconclusive` from a fourth verifier; private information for the participants, never part of the record', late),
]
_ctrl_md, _req_md, _msg_md = [], [], []
for cap, by in _msg_pairs:
    block = f"""**{cap}** ({len(by)} bytes, length prefix included):

```
{hexblock(by)}
```"""
    (_ctrl_md if cap.startswith(CONTROL_NAMES) else _req_md).append(block)
for cap, by in _reply_pairs:
    _msg_md.append(f"""**{cap}** ({len(by)} bytes — replies carry no type tag and no length prefix here; on the wire the same u32-be prefix applies):

```
{hexblock(by)}
```""")
for cap, by in _e2e_pairs:
    _msg_md.append(f"""**{cap}** ({len(by)} bytes — an END-TO-END PAYLOAD, not a stream reply: the bytes are the object alone, and what frames or discriminates it on the encrypted channel is §14.2.4's open demultiplexing decision — no prefix is claimed here):

```
{hexblock(by)}
```""")

emit('messages.md', f"""# Unsigned message families (`wire-format.md` §§6–11)

{PIN}

**Draft. Spec-derived, unverified by an implementation.** Canonical bar 9:
one positive known-answer encoding per framed message family. Framing is
`u32-be length || deterministic CBOR of [type, body]`; stream 0 frames bound
at 64 KiB (65,536 B), bidirectional request streams at 256 KiB (262,144 B).
Unknown **control frames are skipped** and the session survives; unknown
**request types are rejected**. Signed objects embedded below (the currency
attestation, catalog entry, prekey bundle, adoption envelope, query consent)
are byte-identical to their fixtures in `records.md` and `transactions.md`.

## Control frames (stream 0)

{chr(10).join(_ctrl_md)}

## Requests (bidirectional streams)

{chr(10).join(_req_md)}

## What the node delivers for a relay submission

`wire-format.md` §7.10's `RelayedPayload`: the submitter's keyhash in front of
the ciphertext, composed by the node that took the submission and not by
either end. The name is a routing hint — it tells a recipient which peer's
material to try — and never an attribution, which the material the message
opens under decides.

**RelayedPayload — alice's submission as carol collects it** ({len(relayed)} bytes):

```
{hexblock(relayed)}
```

## Replies

{chr(10).join(_msg_md[:len(_reply_pairs)])}

## End-to-end payloads

Objects that ride the encrypted end-to-end channel (design §14.2.4), never a
request/reply stream. Their on-channel framing and type discrimination are the
open demultiplexing decision; the bytes below are the objects alone.

{chr(10).join(_msg_md[len(_reply_pairs):])}

## Session traces (canonical bar 9's trace class)

A static bytes-to-result fixture cannot express sequence rules; each trace is
a sequence of events with the required actions.

| Trace | Sequence | Required actions |
|---|---|---|
| TR1 | control frame with unknown `frame_type` 99 arrives mid-session | `skip_frame`, `session_survives` (§8.0) |
| TR2 | `Attach` arrives in TLS 1.3 0-RTT early data | `defer_until_handshake` or reject — never process (§8.2, §9.2) |
| TR3 | bidirectional stream opens with unknown `request_type` 99 | `close_stream`, `session_survives` (§9.2) |
| TR4 | malformed `SiblingUpdate` arrives | ignore whole, previous list stands, `session_survives` (§8.2) |
| TR5 | second `Attach` on an attached session | `fail_attach` — protocol error (§8.2) |
| TR6 | stream-0 frame with length prefix over 65,536 | protocol error — the stream 0 bound, distinct from §9.2's 262,144 (§8.0) |
| TR7 | heartbeat counter gap observed | liveness accounting only — 3 consecutive missed INTERVALS drive failover, not counter arithmetic (§8.2) |
| TR8 | `Attach` carries an attestation that fails validation | treat as ABSENT, session attaches — currency gates trust, never connectivity (§8.2) |
| TR9 | the primary serving node closes with application code 1 (`refused`) at attach | no sibling failover — a refusal is an answer, not an outage; the client is refused, not disconnected (§8.2) |
| TR10 | during failover, a sibling closes with code 1 | that sibling alone is foreclosed; the next cached candidate is tried in order (§8.2) |
| TR11 | a `Referral` arrives without `key_material` and the requester holds no pin for the next hop | **dial it** — unauthenticated, disclosing nothing beyond the query (§7.7.3): a referrer's identity is not what protects the requester. An implementation failing with missing-key-material strands resolutions the design completes |
| TR12 | heartbeats 0 and 2 arrive; beat 1 was lost | accept the gapped beat, reset liveness, no failover (§8.2) — an implementation accepting only the exact expected counter ignores every beat after one loss and fails over against a live server |
| TR13 | a failover attach returns `AttachAck` mode 0 (primary) | accept the server's determination (§8.2) — the server is authoritative; a client inferring degraded from having dialled a sibling reports wrong state on stale topology |
| TR14 | a `ServingInfra` reply arrives after two of five path indices consumed | resolution complete (§7.7.3) — arrival is announced by the reply; an implementation checking a consumed-equals-length equation rejects a deeper-caching node's valid answer |
| TR15 | a `ResourceRequest` (type 6, an HTTP GET inside) arrives in TLS 1.3 0-RTT early data | `defer_until_handshake` or reject — never process (§9.2, §11): the gateway does not interpret application semantics, so no method is certifiably effect-free, GET included |
| TR16 | one transport session carries hosted sessions to resources A and B; the caller's role row for A changes | retire A's resource-facing session identifier; the transport session and B's hosted session survive, and an in-flight A request completes under its starting snapshot (`infra-client-requirements.md` §10.1, §10.5) — an implementation closing the transport punishes B and the control plane for an authorisation change at A |
| TR17 | a non-member's request names a resource whose snapshot still holds a stale role row for them | status 1 `refused`, never 4 or 5 (§11) — membership is evaluated before acknowledgement and roles, and the specific statuses are answers only members receive; an implementation reaching the role row first hands a stranger member-only information |
| TR18 | a frame parses as `[6, body]` but the body is not a well-formed `ResourceRequest` | answer status 3, no stream reset (§9.2, §11) — once the type is known, a body defect is that type's business, answered in its own terms. The boundary does not generalise across types: a malformed type-4 body closes the stream, because that is *that* type's term |
| TR19 | a stored topology transaction arrives again through a peering cycle | drop the duplicate, forward nothing, session survives (§10.1) — the store is the seen-set; no dedicated suppression cache exists and none may be added |
| TR20 | a memo arrives naming the receiver in field 1, but its own records do not confirm the change | reject the hint: no disavowal, nothing severed (§10.2) — a memo is unsigned and never evidence; acting on it alone manufactures the false positive design §6.2.5 ranks as the worse failure |
| TR21 | an ordinary memo from below arrives; the receiver's position is a prefix of the field-2 path | forward rootward, no cycle (§10.2) — the cycle test is field-1 identity, never path containment: a memo reaches you *because* you are an ancestor, so your path is a prefix on every legitimate hop and a containment test fires on all of them |
| TR22 | a memo names the receiver in field 1, its own records confirm the change and the current slot state, and no live disambiguation is available | disavow the direct subordinate on the ingress branch, reason 5, without prejudice; the memo terminates here (§10.2, §4.3) — the confirmed complement of TR20: the detector cuts the one edge it has authority over, and the disavowal is an ordinary transaction, not a memo field |
| TR23 | a memo arrives whose field-2 anchor is not a subnet the receiver holds a line in | drop it — no table write, no forwarding (§10.2) — a memo never leaves its subnet, and the privacy property only holds if every receiver enforces it: forwarding would carry the memo across the boundary the argument rests on |
| TR24 | a type-4 request arrives whose query field 7 names a different verifier | close the stream: no response, no processing (§5.6) — the subject's consent confines the query to the one verifier field 7 names, and a verifier processing a query not addressed to it turns the consent back into bearer paper |
""")

# ================================================================ corpus.json
# Canonical bar 6: machine-readable fixture identity. Every entry carries a
# stable id, a class (bytes | trace | context | unit), an expect object, and
# for byte fixtures the exact hex. Negative families (bars 5, 7, 12, 14 and
# bar 10's transaction half) are enumerated here as full bytes wherever the
# identity set permits; bounds needing more identities than exist, or bodies
# over 100 KB, are class "unit" with a deterministic recipe instead.
import json as _json
REG = []
def reg(fid, cls, expect, by=None, note='', **extra):
    e = {'id': fid, 'class': cls, 'expect': expect}
    if by is not None:
        e['hex'] = by.hex()
    if note:
        e['note'] = note
    e.update(extra)
    REG.append(e)

def ACC(kind, note=''):
    d = {'outcome': 'accept', 'kind': kind}
    if note: d['reason'] = note
    return d
def REJ(kind, layer, why):
    return {'outcome': 'reject', 'kind': kind, 'layer': layer, 'reason': why}

# ---- positives (envelopes, bodies, standalone objects, presentations, frames)
for fid, by, kind in [
    ('P-adopt-min', adopt_env, 'envelope'), ('P-adopt-divergent', div_env, 'envelope'),
    ('P-adopt-extensions', ext_env, 'envelope'), ('P-adopt-optionals', adopt_full_body, 'body'),
    ('P-departure', depart_body, 'body'), ('P-departure-merge', merge_body, 'body'),
    ('P-disavowal-code40', code40_body, 'body'), ('P-peering', peer_body, 'body'),
    ('P-reissue', reissue_body, 'body'), ('P-formation', formation_env, 'envelope'),
    ('P-normal-record', npr_env, 'envelope'), ('P-fin-nomatch', fin_nm_env, 'envelope'),
    ('P-fin-absent', fin_ab_env, 'envelope'), ('P-ac1', ac1_env, 'envelope'),
    ('P-ac2', ac2_env, 'envelope'), ('P-recovery-adoption', rec_env, 'envelope'),
    ('P-transfer-adoption', xfer_env, 'envelope'),
    ('P-formation-ab', ab_form_env, 'envelope'),
    ('P-normal-bc', bc_env, 'envelope'),
    ('P-alice-c1-record', pc1_env, 'envelope'),
    ('P-presented-full', npr_full, 'presentation'), ('P-presented-partial', npr_part, 'presentation'),
    ('P-presented-minimal', npr_min, 'presentation'),
    ('P-signedlocator', signed_locator, 'SignedLocator'), ('P-signedlocator-jump', sl2, 'SignedLocator'),
    ('P-signedlocator-root', sl_root, 'SignedLocator'), ('P-endpointrecord', endpoint_record, 'EndpointRecord'),
    ('P-currency', currency, 'CurrencyAttestation'), ('P-catalog', catalog, 'CatalogEntry'),
    ('P-abuse', abuse, 'AbuseReport'), ('P-anchor', anchor, 'AnchorEntry'),
    ('P-subtree-ack', ack, 'SubtreeAck'), ('P-prekey', prekey, 'PrekeyBundle'),
    ('P-prekey-desktop', prekey_desktop, 'PrekeyBundle'),
    ('P-currency-identity', currency_identity, 'CurrencyAttestation'),
    ('P-currency-successor', currency_successor, 'CurrencyAttestation'),
    ('P-delegation', deleg_bob, 'Delegation'), ('P-delegation-carol', deleg_carol, 'Delegation'),
    ('P-delegation-alice-desktop', deleg_alice_desktop, 'Delegation'),
    ('P-verification-query', rec_query, 'VerificationQuery'),
    ('P-relayed', relayed, 'RelayedPayload'),
]:
    reg(fid, 'bytes', ACC(kind), by)
for i, (cap, by) in enumerate(_msg_pairs):
    reg(f'P-frame-{i+1:02d}', 'bytes', ACC('frame'), by, note=cap)
for i, (cap, by) in enumerate(_reply_pairs):
    # the reply family, so a consumer need not infer it from the fixture id
    reg(f'P-reply-{i+1:02d}', 'bytes', ACC('reply'), by, note=cap, family=cap.split(' ')[0])
for i, (cap, by) in enumerate(_e2e_pairs):
    reg(f'P-e2e-{i+1:02d}', 'bytes', ACC('e2e-payload'), by, note=cap)
for fid, by, why in [
    ('N-wrong-signer-catalog', catalog_wrong, 'CatalogEntry'),
    ('N-wrong-signer-abuse', abuse_wrong, 'AbuseReport'),
    ('N-wrong-signer-ack', ack_wrong, 'SubtreeAck'),
    ('N-wrong-signer-prekey', prekey_wrong, 'PrekeyBundle'),
]:
    reg(fid, 'bytes', REJ(why, 'semantic', 'signature valid under a key the object does not name'), by)
reg('N-wrong-signer-currency', 'bytes',
    REJ('CurrencyAttestation', 'semantic', 'signature valid under a key the object does not name'),
    currency_wrong,
    note='Inside an Attach the consequence is treat-as-ABSENT, not a rejected '
         'session (TR8): currency gates trust, never connectivity.')
reg('N-wrong-signer-anchor', 'bytes',
    REJ('AnchorEntry', 'semantic', 'signature valid under a key the entry does not name'),
    anchor_wrong,
    precondition='the named key is pinned',
    note='Conditional by design [0.6 phase 2, 2026-09-02]: infra s4.1 permits '
         'an unverified gossip cache, under which this defect is undetectable '
         'at ingestion and surfaces only at first authenticated contact (or '
         'never, the anchor exception). Only a verified-on-acceptance '
         'implementation, or one holding the pinned key, rejects at ingest.')

# ---- referral progress negatives [0.6 phase 2, 2026-09-02]: the corpus had a
# positive advances=2 referral and no malformed-progress counterpart.
reg('N-referral-advances-0', 'bytes',
    REJ('ResolveReply', 'schema', 'advances MUST be >= 1 - a referral that advances nothing is a loop'),
    e_map([(e_uint(1), e_bstr(NONCE(b'resolve'))),
           (e_uint(2), e_uint(2)),
           (e_uint(5), e_map([(e_uint(1), e_bstr(carol.keyhash)),
                              (e_uint(2), e_arr([np1])),
                              (e_uint(3), e_uint(0))]))]))
reg('CTX-referral-overshoot', 'context',
    {'outcome': 'reject', 'layer': 'semantic',
     'reason': 'a referral advancing past the path end is malformed'},
    inputs={'request': 'P-frame-09 (five path nibbles, none yet consumed)',
            'reply': 'the P-reply-02 referral with field 5.3 = 6'},
    note='Schema-valid bytes; the defect is relative to the request, so this is a context fixture.')

# ---- retired-number tombstones (T29, 2026-09-02) and a non-witness corroborator
reg('N-retired-body-key-7', 'bytes',
    REJ('body', 'schema', 'retired key 7 is a tombstone, not extension space (T29)'),
    e_map([(e_uint(0), backptrs(*[[genesis(x.keyhash)] for x in (n_hi, n_lo, IDS['w2'])])),
           (e_uint(1), e_uint(TS_C2)), (e_uint(2), e_uint(TS_C2F)),
           (e_uint(3), e_arr([participant(n_hi), participant(n_lo)])),
           (e_uint(4), e_arr([witness_entry(IDS['w2'], bob, 7)])),
           (e_uint(6), e_uint(0)), (e_uint(7), e_uint(5)),
           (e_uint(8), e_bstr(fin_ab_root))]))
reg('N-retired-witness-key-4', 'bytes',
    REJ('Witness', 'schema', 'retired key 4 is a tombstone, not extension space (T29)'),
    e_map([(e_uint(1), e_bstr(IDS['w1'].keyhash)),
           (e_uint(2), e_bstr(alice.keyhash)),
           (e_uint(3), e_uint(7)),
           (e_uint(4), e_bstr(H(b'rhtn-test-vectors:dead-nonce-commitment')))]))
reg('U-witness-no-affirmative', 'unit',
    REJ('body', 'schema',
        'a normal record none of whose witnesses set both protocol_ran and both_responsive fails the witness floor'),
    note='recipe: any normal record where no field-4 entry has attestation bits 0 and 1 both set; such entries are partial evidence, not corroboration (s3.2, T31, 2026-09-03)')

reg('U-corroboration-non-witness', 'unit',
    REJ('presentation', 'semantic',
        'a revealed location naming a corroborator absent from body field 4 attests nothing'),
    note='recipe: any normal record whose committed location value cites a keyhash outside field 4; checkable only when location is revealed (s4.5, 2026-09-02)')

# ---- bar 10, transaction half: envelope whose signer set mismatches body roles
ws_env, _ = envelope(1, 1, adopt_body, [alice, carol])
reg('N-envelope-wrong-signers', 'bytes',
    REJ('envelope', 'semantic', 'envelope kids are alice+carol; body roles are alice+bob'), ws_env)

# ---- bar 12: closed-enum unknown values (each otherwise valid)
reg('N-enum-result', 'bytes', REJ('VerifierResponse', 'schema', 'result 9 outside 0-3'),
    classical_response(IDS['c1'], alice, npr_q0, consent_over(npr_q0, alice), 9, basis=0, tplv=3, sb=0))
reg('N-enum-basis', 'bytes', REJ('VerifierResponse', 'schema', 'basis 9 outside 0-2'),
    classical_response(IDS['c1'], alice, npr_q0, consent_over(npr_q0, alice), 0, basis=9, tplv=3, sb=0))
reg('N-enum-selection-basis', 'bytes', REJ('VerifierResponse', 'schema', 'selection_basis 9 outside 0-3 (T27)'),
    classical_response(IDS['c1'], alice, npr_q0, consent_over(npr_q0, alice), 0, basis=0, tplv=3, sb=9))
reg('N-enum-disavowal-64', 'bytes', REJ('body', 'schema', 'code 64 outside the 0-63 space (T8) — contrast P-disavowal-code40'),
    e_map([(e_uint(0), backptrs([adopt_txid])), (e_uint(1), e_bstr(bob.keyhash)),
           (e_uint(2), e_bstr(alice.keyhash)), (e_uint(3), e_uint(TS_DEPART)), (e_uint(4), e_uint(64))]))
reg('N-enum-currency-role', 'bytes', REJ('CurrencyAttestation', 'schema', 'issuer role 9 outside 0-3'),
    sign1_slot([p if p[0] != e_uint(5) else (e_uint(5), e_uint(9)) for p in cur_pairs], 7, AAD_CURRENCY, bob))
reg('N-enum-resolve-code', 'bytes', REJ('ResolveReply', 'schema', 'result 9 outside 0-2'),
    e_map([(e_uint(1), e_bstr(NONCE(b'resolve'))), (e_uint(2), e_uint(9))]))
reg('N-enum-push-kind', 'bytes', REJ('frame', 'schema', 'TopologyPush kind 2 outside 0-1'),
    frame(5, e_map([(e_uint(1), e_uint(2)), (e_uint(2), e_bstr(adopt_env))])))
reg('N-enum-memo-slot', 'bytes', REJ('frame', 'schema', 'memo slot 10 outside the nibble range 0-9'),
    frame(6, e_map([(e_uint(1), e_bstr(bob.keyhash)), (e_uint(2), root_loc),
                    (e_uint(3), e_uint(10)), (e_uint(4), e_uint(TS_ADOPT))])))
reg('N-enum-resource-status', 'bytes', REJ('ResourceResponse', 'schema', 'status 9 outside 0-5'),
    e_map([(e_uint(1), e_uint(9))]))
reg('N-enum-attach-mode', 'bytes', REJ('frame', 'schema', 'AttachAck mode 9 outside 0-1'),
    frame(2, e_map([(e_uint(1), e_uint(9)), (e_uint(3), e_uint(300)), (e_uint(4), e_uint(0)),
                    (e_uint(5), caps_server)])))
reg('D-enum-location-method', 'bytes', ACC('LocationEvidence', 'method 9: the registry is deliberately open (D3)'),
    e_map([(e_uint(1), e_arr([e_map([(e_uint(1), e_uint(9)), (e_uint(2), e_tstr('u4p'))])])),
           (e_uint(2), e_arr([]))]))
reg('D-enum-witness-reserved-bits', 'bytes', ACC('Witness', 'bits 3+ retained, 0-2 interpreted (D4)'),
    witness_entry(IDS['w1'], alice, 0b1111))

# ---- bar 13b: consistency rules a validator checks from the object alone
#      (s3.2, s3.5, s4.1, s4.4) [conformance review F01-F03, 2026-09-11].  Each
#      is otherwise a valid object; what it gets wrong is whom or what the
#      evidence is about, or the container it sits in.
_adopt_bare = [(e_uint(0), backptrs([ab_form_txid], [ab_form_txid])),
               (e_uint(1), e_bstr(alice.keyhash)), (e_uint(2), e_bstr(bob.keyhash)),
               (e_uint(3), adopt_loc), (e_uint(4), e_uint(TS_ADOPT))]
reg('N-adopt-no-evidence', 'bytes',
    REJ('body', 'schema', 'none of fields 6, 8 and 9: an adoption carries exactly one evidence form (s4.1, design s6.1.1)'),
    e_map(_adopt_bare))
reg('N-adopt-two-evidence', 'bytes',
    REJ('body', 'schema', 'fields 8 and 9 together: the evidence forms are alternatives (s4.1)'),
    e_map(_adopt_bare + [(e_uint(8), e_bstr(ab_form_txid)), (e_uint(9), xfer_block)]))
reg('N-adopt-self-patron', 'bytes',
    REJ('body', 'schema', 'node and patron one identity: a node cannot hold authority over itself (s4.1)'),
    e_map([(e_uint(0), backptrs([ab_form_txid], [ab_form_txid])),
           (e_uint(1), e_bstr(alice.keyhash)), (e_uint(2), e_bstr(alice.keyhash)),
           (e_uint(3), adopt_loc), (e_uint(4), e_uint(TS_ADOPT)), (e_uint(8), e_bstr(ab_form_txid))]))
reg('N-peering-no-evidence', 'bytes',
    REJ('body', 'schema', 'field 8 absent: a peering carries its presence record, unconditionally (s4.4)'),
    e_map([(e_uint(0), backptrs([adopt_txid], [formation_txid])),
           (e_uint(1), e_bstr(bob.keyhash)), (e_uint(2), e_bstr(carol.keyhash)),
           (e_uint(3), network_point([10, 0, 0, 1], asn=64511, port=7432)),
           (e_uint(4), network_point([192, 0, 2, 7])),
           (e_uint(5), e_uint(TS_DEPART + 7200))]))
reg('N-formation-non-genesis', 'bytes',
    REJ('body', 'schema', 'key 0 lists are not the genesis value: a key appears in at most one formation record, its first (s3.2)'),
    e_map([(e_uint(0), backptrs([formation_txid], [formation_txid])),
           (e_uint(1), e_uint(TS_START)), (e_uint(2), e_uint(TS_FINAL)),
           (e_uint(3), e_arr([participant(p_hi), participant(p_lo)])),
           (e_uint(6), e_uint(1)), (e_uint(8), e_bstr(form_root))]))
def _rec_body_with(response):
    return e_map([(e_uint(0), backptrs([genesis(alice2.keyhash)], [adopt_txid])),
                  (e_uint(1), e_bstr(alice2.keyhash)), (e_uint(2), e_bstr(bob.keyhash)),
                  (e_uint(3), rec_loc), (e_uint(4), e_uint(TS_RECOVER)),
                  (e_uint(6), e_map([(e_uint(1), e_bstr(alice.keyhash)),
                                     (e_uint(2), e_arr([response])),
                                     (e_uint(3), rec_succ)]))])
def _resp_with(key, value):
    # the recovery response with one field swapped; its signatures go stale,
    # and the rule this exercises is structural
    return e_map([(k, value if k == e_uint(key) else v) for k, v in resp_unsigned_pairs[:5]]
                 + [(e_uint(7), rec_consent),
                    (e_uint(8), value if key == 8 else e_bstr(alice.keyhash)),
                    (e_uint(9), rec_vr_sig), (e_uint(10), e_uint(0))])
reg('N-recovery-subject-other', 'bytes',
    REJ('body', 'schema', "response subject is bob, not the adopted node alice2: every response's subject MUST equal field 1 (s4.1)"),
    _rec_body_with(_resp_with(2, e_bstr(bob.keyhash))))
reg('N-recovery-prior-other', 'bytes',
    REJ('body', 'schema', "response field 8 names bob, not the block's prior key alice: evidence about one old identity under a claim about another (s4.1)"),
    _rec_body_with(_resp_with(8, e_bstr(bob.keyhash))))
_empty_xfer_body = e_map([(e_uint(0), backptrs([adopt_txid], [formation_txid])),
                          (e_uint(1), e_bstr(alice.keyhash)), (e_uint(2), e_bstr(carol.keyhash)),
                          (e_uint(3), xfer_loc), (e_uint(4), e_uint(TS_TRANSFER)),
                          (e_uint(9), e_map([(e_uint(1), e_bstr(bob.keyhash)),
                                             (e_uint(2), e_arr([e_bstr(b''), b'\xa0', NULL, e_arr([])]))]))])
_empty_xfer_env, _ = envelope(1, 1, _empty_xfer_body, [alice, carol])
reg('N-transfer-empty-block', 'bytes',
    REJ('envelope', 'schema', "the former patron's COSE_Sign carries no entries: one hybrid signer contributes exactly two, and an empty block covers nothing (s3.5, s4.1)"),
    _empty_xfer_env)
_payload_env = e_map([(e_uint(1), e_uint(1)), (e_uint(2), e_uint(1)), (e_uint(3), adopt_body),
                      (e_uint(4), e_arr([e_bstr(b''), b'\xa0', e_bstr(b'\x00'),
                                         e_arr([cose_signature_entry(p, sg) for _, _, p, _, sg in adopt_entries])]))])
reg('N-envelope-payload-present', 'bytes',
    REJ('envelope', 'schema', "the COSE payload slot carries h'00' with every signature intact: signatures are detached and the slot is nil (s1, s3.5)"),
    _payload_env)

# ---- bar 14: schema-shape — missing-required and wrong-major-type per core schema
reg('N-shape-adoption-missing-locator', 'bytes', REJ('body', 'schema', 'field 3 required'),
    e_map([(e_uint(0), backptrs([genesis(alice.keyhash)], [genesis(bob.keyhash)])),
           (e_uint(1), e_bstr(alice.keyhash)), (e_uint(2), e_bstr(bob.keyhash)),
           (e_uint(4), e_uint(TS_ADOPT))]))
reg('N-shape-adoption-timestamp-tstr', 'bytes', REJ('body', 'schema', 'field 4 must be uint'),
    e_map([(e_uint(0), backptrs([genesis(alice.keyhash)], [genesis(bob.keyhash)])),
           (e_uint(1), e_bstr(alice.keyhash)), (e_uint(2), e_bstr(bob.keyhash)),
           (e_uint(3), adopt_loc), (e_uint(4), e_tstr('yesterday'))]))
reg('N-shape-presence-missing-root', 'bytes', REJ('body', 'schema', 'field 8 required'),
    e_map([(e_uint(0), backptrs([genesis(alice.keyhash)], [genesis(carol.keyhash)])),
           (e_uint(1), e_uint(TS_START)), (e_uint(2), e_uint(TS_FINAL)),
           (e_uint(3), e_arr([participant(alice), participant(carol)])),
           (e_uint(6), e_uint(1))]))
reg('N-shape-presence-subtype-tstr', 'bytes', REJ('body', 'schema', 'field 6 must be uint'),
    e_map([(e_uint(0), backptrs([genesis(alice.keyhash)], [genesis(carol.keyhash)])),
           (e_uint(1), e_uint(TS_START)), (e_uint(2), e_uint(TS_FINAL)),
           (e_uint(3), e_arr([participant(alice), participant(carol)])),
           (e_uint(6), e_tstr('normal')), (e_uint(8), e_bstr(form_root))]))
reg('N-shape-response-missing-consent', 'bytes', REJ('VerifierResponse', 'schema', 'field 7 required'),
    e_map([(e_uint(1), e_bstr(KH_c1 := IDS['c1'].keyhash)), (e_uint(2), e_bstr(alice.keyhash)),
           (e_uint(3), e_bstr(npr_q0)), (e_uint(4), e_uint(0)), (e_uint(5), e_uint(1)),
           (e_uint(10), e_uint(0))]))
reg('N-shape-currency-missing-issuer', 'bytes', REJ('CurrencyAttestation', 'schema', 'field 6 required'),
    sign1_slot([p for p in cur_pairs if p[0] != e_uint(6)], 7, AAD_CURRENCY, bob))
reg('N-shape-catalog-missing-endpoint', 'bytes', REJ('CatalogEntry', 'schema', 'field 5 required'),
    sign1_slot([p for p in cat_pairs if p[0] != e_uint(5)], 8, AAD_CATALOG, bob))
reg('N-shape-attach-missing-identity', 'bytes', REJ('frame', 'schema', 'Attach field 1 required'),
    frame(1, e_map([(e_uint(3), caps_client)])))
reg('N-shape-frame-arity', 'bytes', REJ('frame', 'schema', 'a frame is [type, body], not [type]'),
    (lambda c: len(c).to_bytes(4, 'big') + c)(e_arr([e_uint(3)])))
reg('N-shape-locator-missing-path', 'bytes', REJ('Locator', 'schema', 'field 2 required'),
    e_map([(e_uint(1), e_bstr(bob.keyhash)), (e_uint(3), seqno(5, 0))]))
reg('N-shape-keyhash-31', 'bytes', REJ('SignedLocator', 'schema', 'field 1 must be exactly 32 bytes'),
    e_map([(e_uint(1), e_bstr(alice.keyhash[:31])), (e_uint(2), loc), (e_uint(3), sl_cose)]))
reg('N-shape-keyhash-33', 'bytes', REJ('SignedLocator', 'schema', 'field 1 must be exactly 32 bytes'),
    e_map([(e_uint(1), e_bstr(alice.keyhash + b'\x00')), (e_uint(2), loc), (e_uint(3), sl_cose)]))

# ---- bar 7: the boundary sweep (declared-scope list, negative-vectors.md)
def _depart_with_heads(k):
    heads = sorted(H(b'rhtn-test-vectors:head:%d' % i) for i in range(k))
    key0 = e_arr([e_arr([e_bstr(h) for h in heads])])   # bypasses backptrs()'s own 1..8 guard
    return e_map([(e_uint(0), key0), (e_uint(1), e_bstr(alice.keyhash)),
                  (e_uint(2), e_bstr(bob.keyhash)), (e_uint(3), seqno(5, 50)),
                  (e_uint(4), e_uint(TS_DEPART + 7200))])
reg('B-backptrs-8', 'bytes', ACC('body', 'a merge of eight heads, the ceiling'), _depart_with_heads(8))
reg('B-backptrs-9', 'bytes', REJ('body', 'schema', 'nine heads exceed the 1..8 bound'), _depart_with_heads(9))
w17 = [witness_entry(w, alice, 7) for w in W16] + [witness_entry(IDS['c1'], bob, 7)]
reg('B-witnesses-17', 'bytes', REJ('body', 'schema', 'seventeen witnesses exceed the 16 ceiling'),
    e_map([(e_uint(0), backptrs(*([[genesis(alice.keyhash)], [genesis(bob.keyhash)]]
                                  + [[genesis(w.keyhash)] for w in W16] + [[genesis(IDS['c1'].keyhash)]]))),
           (e_uint(1), e_uint(TS_C2)), (e_uint(2), e_uint(TS_C2F)),
           (e_uint(3), e_arr([participant(n_hi), participant(n_lo)])),
           (e_uint(4), e_arr(w17)), (e_uint(6), e_uint(0)), (e_uint(8), e_bstr(npr_root))]))
def _pth(n):
    packed = bytearray()
    nb = [1] * n
    for i in range(0, n - 1, 2): packed.append(nb[i] << 4 | nb[i + 1])
    if n % 2: packed.append(nb[-1] << 4)
    return e_map([(e_uint(1), e_bstr(bytes(packed))), (e_uint(2), e_uint(n))])
reg('B-path-24', 'bytes', ACC('Locator', 'path at the 24-nibble ceiling'),
    e_map([(e_uint(1), e_bstr(bob.keyhash)), (e_uint(2), _pth(24)), (e_uint(3), seqno(5, 0))]))
reg('B-path-25', 'bytes', REJ('Locator', 'schema', '25 nibbles exceed the ceiling'),
    e_map([(e_uint(1), e_bstr(bob.keyhash)), (e_uint(2), _pth(25)), (e_uint(3), seqno(5, 0))]))
def _adopt_ext(nkeys, vlen):
    ext = [(e_uint(100 + i), e_bstr(b'x')) for i in range(nkeys)]
    if vlen:
        # vlen is the ENCODED slice target: s1's ceiling is 1024 bytes of
        # encoded CBOR, not payload [Rust runner finding, 2026-09-02] - a
        # bstr header for this size is 3 bytes (0x59 + u16 length)
        ext = [(e_uint(100), e_bstr(b'v' * (vlen - 3)))]
        assert len(ext[0][1]) == vlen
    # field 8 present: an adoption carries exactly one evidence form (s4.1),
    # and a boundary body is otherwise valid [conformance review, 2026-09-11]
    return e_map([(e_uint(0), backptrs([genesis(alice.keyhash)], [genesis(bob.keyhash)])),
                  (e_uint(1), e_bstr(alice.keyhash)), (e_uint(2), e_bstr(bob.keyhash)),
                  (e_uint(3), adopt_loc), (e_uint(4), e_uint(TS_ADOPT)),
                  (e_uint(8), e_bstr(ab_form_txid))] + ext)
reg('B-ext-keys-16', 'bytes', ACC('body', 'sixteen unknown keys, the ceiling'), _adopt_ext(16, 0))
reg('B-ext-keys-17', 'bytes', REJ('body', 'schema', 'seventeen unknown keys exceed the ceiling'), _adopt_ext(17, 0))
reg('B-ext-value-1024', 'bytes', ACC('body', 'an unknown value whose ENCODED slice is exactly the 1024-byte ceiling'), _adopt_ext(0, 1024))
reg('B-ext-value-1025', 'bytes', REJ('body', 'schema', '1025 ENCODED bytes exceed the ceiling'), _adopt_ext(0, 1025))
def _peer_with_audits(k):
    audits = e_arr([e_map([(e_uint(1), e_uint(TS_DEPART + i)), (e_uint(2), b'\xf5'),
                           (e_uint(3), e_bstr(carol.keyhash))]) for i in range(k)])
    return e_map([(e_uint(0), backptrs([adopt_txid], [formation_txid])),
                  (e_uint(1), e_bstr(bob.keyhash)), (e_uint(2), e_bstr(carol.keyhash)),
                  (e_uint(3), network_point([10, 0, 0, 1], asn=64511, port=7432)),
                  (e_uint(4), network_point([192, 0, 2, 7])),
                  (e_uint(5), e_uint(TS_DEPART + 7200)), (e_uint(7), audits),
                  (e_uint(8), e_bstr(bc_txid))])   # required, unconditionally (s4.4)
reg('B-audits-8', 'bytes', ACC('body', 'eight peering audits, the ceiling'), _peer_with_audits(8))
reg('B-audits-9', 'bytes', REJ('body', 'schema', 'nine audits exceed the ceiling'), _peer_with_audits(9))
def _er_np(k):
    nps = e_arr([network_point([10, 0, 0, i + 1]) for i in range(k)]) if k else e_arr([])
    return e_map(er_fields[:1] + [(e_uint(2), nps)] + er_fields[2:] + [(e_uint(4), er_cose)])
reg('B-endpoints-8', 'bytes', ACC('EndpointRecord', 'eight NetworkPoints, the ceiling (signature stale by design: shape fixture)'), _er_np(8))
reg('B-endpoints-9', 'bytes', REJ('EndpointRecord', 'schema', 'nine NetworkPoints exceed 1..8'), _er_np(9))
reg('B-endpoints-0', 'bytes', REJ('EndpointRecord', 'schema', 'field 2 is 1*8 — an empty list violates the minimum'), _er_np(0))
reg('B-port-65535', 'bytes', ACC('NetworkPoint', 'maximum non-default port'), network_point([10, 0, 0, 1], port=65535))
_np_raw = lambda p: e_map([(e_uint(1), e_bstr(bytes([10, 0, 0, 1]))), (e_uint(3), e_uint(p))])
reg('B-port-65536', 'bytes', REJ('NetworkPoint', 'schema', 'port exceeds u16'), _np_raw(65536))
reg('B-port-0', 'bytes', REJ('NetworkPoint', 'schema', 'zero is never a destination'), _np_raw(0))
reg('B-port-7431-explicit', 'bytes', REJ('NetworkPoint', 'schema', 'writing the default port out is malformed; omission is the one spelling (s1, s4.4)'), _np_raw(7431))
reg('B-prekey-4096', 'bytes', ACC('PrekeyBundle', 'blob at the 4 KB ceiling'),
    sign1_slot([(e_uint(1), e_bstr(alice.keyhash)), (e_uint(2), e_uint(1)),
                (e_uint(3), e_bstr(b'k' * 4096)), (e_uint(4), e_uint(TS_REC)),
                (e_uint(5), e_bstr(alice.ed_pub))], 6, AAD_PREKEY, alice))
reg('B-prekey-4097', 'bytes', REJ('PrekeyBundle', 'schema', 'blob exceeds the 4 KB ceiling'),
    sign1_slot([(e_uint(1), e_bstr(alice.keyhash)), (e_uint(2), e_uint(1)),
                (e_uint(3), e_bstr(b'k' * 4097)), (e_uint(4), e_uint(TS_REC)),
                (e_uint(5), e_bstr(alice.ed_pub))], 6, AAD_PREKEY, alice))
reg('B-archive-max-0', 'bytes', REJ('ArchiveRequest', 'schema', 'max_records below 1'),
    e_map([(e_uint(1), e_bstr(alice.keyhash)), (e_uint(3), e_uint(0)), (e_uint(5), e_bstr(NONCE(b'a0')))]))
reg('B-archive-max-256', 'bytes', ACC('ArchiveRequest', 'max_records at the ceiling'),
    e_map([(e_uint(1), e_bstr(alice.keyhash)), (e_uint(3), e_uint(256)), (e_uint(5), e_bstr(NONCE(b'a256')))]))
reg('B-archive-max-257', 'bytes', REJ('ArchiveRequest', 'schema', 'max_records above 256'),
    e_map([(e_uint(1), e_bstr(alice.keyhash)), (e_uint(3), e_uint(257)), (e_uint(5), e_bstr(NONCE(b'a257')))]))
for gh, exp in [('u4', REJ('LocationEvidence', 'schema', 'two geohash characters')),
                ('u4pqr', REJ('LocationEvidence', 'schema', 'five geohash characters')),
                ('U4P', REJ('LocationEvidence', 'schema', 'upper case is malformed'))]:
    reg(f'B-geohash-{gh}', 'bytes', exp,
        e_map([(e_uint(1), e_arr([e_map([(e_uint(1), e_uint(0)), (e_uint(2), e_tstr(gh))])])),
               (e_uint(2), e_arr([]))]))
def _prox(k):
    return e_map([(e_uint(1), e_arr([e_map([(e_uint(1), e_uint(3)), (e_uint(2), e_uint(0))])] * 1
                                    if k == 1 else
                                    [e_map([(e_uint(1), e_uint(1 + (i % 4))), (e_uint(2), e_uint(0)),
                                            (e_uint(3), e_uint(i + 1))]) for i in range(k)])),
                  (e_uint(2), e_uint(3))])
reg('B-channels-8', 'bytes', ACC('Proximity', 'eight channels, the ceiling'), _prox(8))
reg('B-channels-9', 'bytes', REJ('Proximity', 'schema', 'nine channels exceed the ceiling'), _prox(9))
def _loc_ev(nass, ncorr):
    return e_map([(e_uint(1), e_arr([e_map([(e_uint(1), e_uint(0)), (e_uint(2), e_tstr('u4p'))])
                                     for _ in range(nass)])),
                  (e_uint(2), e_arr([e_map([(e_uint(1), e_bstr(W16[i % 16].keyhash)),
                                            (e_uint(2), e_uint(3)), (e_uint(3), e_uint(5))])
                                     for i in range(ncorr)]))])
reg('B-asserted-4', 'bytes', ACC('LocationEvidence', 'four asserted locations, the ceiling'), _loc_ev(4, 1))
reg('B-asserted-5', 'bytes', REJ('LocationEvidence', 'schema', 'five asserted locations exceed the ceiling'), _loc_ev(5, 1))
reg('B-corroborations-16', 'bytes', ACC('LocationEvidence', 'sixteen corroborations, the ceiling'), _loc_ev(1, 16))
reg('B-corroborations-17', 'bytes', REJ('LocationEvidence', 'schema', 'seventeen corroborations exceed the ceiling'), _loc_ev(1, 17))
_gap = lambda g: e_map([(e_uint(0), backptrs(*[[genesis(x.keyhash)] for x in (n_hi, n_lo, IDS['w2'])])),
                        (e_uint(1), e_uint(TS_C2)), (e_uint(2), e_uint(TS_C2 + g)),
                        (e_uint(3), e_arr([participant(n_hi), participant(n_lo)])),
                        (e_uint(4), e_arr([witness_entry(IDS['w2'], bob, 7)])),
                        (e_uint(6), e_uint(0)), (e_uint(8), e_bstr(fin_ab_root))])
reg('B-finalization-gap-86400', 'bytes', ACC('body', 'gap at exactly 24 hours, the maximum'), _gap(86400))
reg('B-finalization-gap-86401', 'bytes', REJ('body', 'schema', 'gap exceeds the 24-hour bound'), _gap(86401))
def _scope_list(k, sortit=True, dupe=False):
    khs = sorted(H(b'rhtn-test-vectors:scope:%d' % i) for i in range(k))
    if not sortit: khs = list(reversed(khs))
    if dupe: khs[1] = khs[0]
    return e_arr([e_uint(6), e_arr([e_bstr(x) for x in khs])])
reg('B-scope-256', 'bytes', ACC('Scope', 'list at the 256 ceiling'), _scope_list(256))
reg('B-scope-257', 'bytes', REJ('Scope', 'schema', '257 keyhashes exceed the ceiling'), _scope_list(257))
reg('N-scope-unordered', 'bytes', REJ('Scope', 'schema', 'the list must ascend'), _scope_list(4, sortit=False))
reg('N-scope-duplicate', 'bytes', REJ('Scope', 'schema', 'duplicates are malformed'), _scope_list(4, dupe=True))
reg('P-scope-down2', 'bytes', ACC('Scope', 'down(2), the [tag, n] form'), e_arr([e_uint(1), e_uint(2)]))
reg('P-scope-self', 'bytes', ACC('Scope', 'the bare-uint form'), e_uint(0))
reg('B-caps-64', 'bytes', ACC('Capabilities', 'sixty-four parameters, the ceiling'),
    e_map(sorted([(e_uint(int.from_bytes(H(b'cap%d' % i)[:8], 'big')), e_bstr(b'v')) for i in range(64)],
                 key=lambda p: p[0])))
reg('B-caps-65', 'bytes', REJ('Capabilities', 'schema', 'sixty-five parameters exceed the ceiling'),
    e_map(sorted([(e_uint(int.from_bytes(H(b'cap%d' % i)[:8], 'big')), e_bstr(b'v')) for i in range(65)],
                 key=lambda p: p[0])))
reg('B-caps-value-1025', 'bytes', REJ('Capabilities', 'schema', 'a 1025-byte value exceeds the ceiling'),
    e_map([(e_uint(CAP_BATCH), e_bstr(b'v' * 1025))]))
reg('B-siblings-9', 'bytes', ACC('frame', 'AttachAck with nine SiblingRefs, the f-1 ceiling'),
    frame(2, e_map([(e_uint(1), e_uint(0)),
                    (e_uint(2), e_arr([e_map([(e_uint(1), e_bstr(W16[i].keyhash)),
                                              (e_uint(2), e_arr([network_point([10, 0, 1, i + 1])]))])
                                       for i in range(9)])),
                    (e_uint(3), e_uint(300)), (e_uint(4), e_uint(0)), (e_uint(5), caps_server)])))
reg('B-siblings-10', 'bytes', REJ('frame', 'schema', 'AttachAck with ten SiblingRefs exceeds f-1 = 9'),
    frame(2, e_map([(e_uint(1), e_uint(0)),
                    (e_uint(2), e_arr([e_map([(e_uint(1), e_bstr(W16[i].keyhash)),
                                              (e_uint(2), e_arr([network_point([10, 0, 1, i + 1])]))])
                                       for i in range(10)])),
                    (e_uint(3), e_uint(300)), (e_uint(4), e_uint(0)), (e_uint(5), caps_server)])))
def _catalog_padded(target):
    # Reach the exact total via a legal connect_scope keyhash list (coarse,
    # 33 bytes per entry) plus metadata padding (fine, 1 byte per byte) — the
    # metadata field's own 1024-byte bound cannot reach 2048 alone.
    for k in range(2, 40):
        scope = e_arr([e_uint(6), e_arr([e_bstr(H(b'rhtn-test-vectors:pad:%d' % i))
                                         for i in range(k)])])
        for pad in range(0, 700):
            pairs = sorted([p for p in cat_pairs if p[0] != e_uint(7)]
                           + [(e_uint(6), scope), (e_uint(7), e_bstr(b'v=1' + b'.' * pad))],
                           key=lambda p: p[0])
            out = sign1_slot(pairs, 8, AAD_CATALOG, bob)
            if len(out) == target:
                return out
    raise AssertionError('could not pad catalog entry to %d' % target)
reg('B-catalog-2048', 'bytes', ACC('CatalogEntry', 'total encoded size at the 2048 ceiling'), _catalog_padded(2048))
reg('B-catalog-2049', 'bytes', REJ('CatalogEntry', 'schema', '2049 encoded bytes exceed the ceiling'), _catalog_padded(2049))
reg('B-batch-1-subject', 'bytes', REJ('PrekeyBatchRequest', 'schema', 'a batch names at least two subjects'),
    e_map([(e_uint(1), e_arr([e_bstr(alice.keyhash)])), (e_uint(2), e_bstr(NONCE(b'b1')))]))
reg('U-responses-33', 'unit',
    REJ('body', 'schema', 'thirty-three verifier responses exceed the 32 ceiling'),
    note='recipe: any normal record with 33 distinct-verifier responses; the suite holds fewer distinct identities than the bound')
reg('U-recovery-responses-33', 'unit',
    REJ('Recovery', 'schema', 'thirty-three responses exceed the 32 ceiling'),
    note='recipe: a Recovery block with 33 distinct-verifier responses')
reg('U-frame-65537', 'unit',
    REJ('frame', 'session', 'a stream-0 frame over 65,536 bytes'),
    note='recipe: frame(3, heartbeat) with body padded by one unknown 65,000-byte capability value; length prefix 65,537')
reg('U-request-262145', 'unit',
    REJ('frame', 'session', 'a request frame over 262,144 bytes'),
    note='recipe: request 2 (ArchiveRequest) padded past the bidirectional bound')

# ---- bar 5: the disclosure negative family (from the normal record)
_np_slots = [npr_slots[lab] for lab in LABELS]
def _pres(slots_arr):
    return e_arr([npr_env, e_arr(slots_arr)])
full7 = [npr_slots[lab][0] for lab in LABELS]
reg('N-disclosure-six-slots', 'bytes', REJ('presentation', 'schema', 'six slots — exactly seven always'),
    _pres(full7[:6]))
reg('N-disclosure-eight-slots', 'bytes', REJ('presentation', 'schema', 'eight slots — exactly seven always'),
    _pres(full7 + [e_bstr(npr_slots['capture'][1])]))
reg('N-disclosure-label-mismatch', 'bytes', REJ('presentation', 'schema',
    "slot 0 carries the 'location' disclosure — a revealed label must match its slot position"),
    _pres([npr_slots['location'][0]] + full7[1:]))
reg('N-disclosure-salt-15', 'bytes', REJ('presentation', 'schema', 'a 15-byte salt is malformed'),
    _pres([e_arr([e_bstr(H(b's')[:15]), e_tstr('capture'), npr_values['capture']])] + full7[1:]))
reg('N-disclosure-root-mismatch', 'bytes', REJ('presentation', 'semantic',
    'a flipped withheld digest — the recomputed root no longer equals body field 8'),
    _pres([e_bstr(bytes([npr_slots['capture'][1][0] ^ 1]) + npr_slots['capture'][1][1:])] + full7[1:]))
reg('N-disclosure-value-mutated', 'bytes', REJ('presentation', 'semantic',
    'a revealed value mutated — its digest, and so the root, no longer match'),
    _pres([e_arr([e_bstr(H(f'rhtn-test-vectors:salt:normal:capture'.encode())[:16]),
                  e_tstr('capture'),
                  e_map([(e_uint(1), e_uint(1)), (e_uint(2), e_uint(4)),
                         (e_uint(3), e_uint(0)), (e_uint(4), e_uint(2))])])] + full7[1:]))

# ---- bar 13: optionals not otherwise exercised
reg('P-archive-request-bounded', 'bytes', ACC('ArchiveRequest', 'frontier and stop-timestamp both present'),
    e_map([(e_uint(1), e_bstr(alice.keyhash)), (e_uint(2), e_arr([e_bstr(depart_txid)])),
           (e_uint(3), e_uint(16)), (e_uint(4), e_uint(TS_ADOPT)), (e_uint(5), e_bstr(NONCE(b'ab')))]))
reg('P-archive-reply-continued', 'bytes', ACC('ArchiveReply', 'more remaining, the frontier present'),
    e_map([(e_uint(1), e_bstr(NONCE(b'ab'))), (e_uint(2), e_arr([adopt_env])),
           (e_uint(3), b'\xf5'), (e_uint(4), e_arr([e_bstr(adopt_txid)]))]))
# ---- 2026-09-22: the delegation (§8.2), the device (§7.8, §7.10), the frontier (§7.9)
reg('N-wrong-signer-delegation', 'bytes',
    REJ('Delegation', 'semantic', 'signature valid under a key the delegation does not name'), deleg_wrong)
reg('B-delegation-window-172799', 'bytes',
    REJ('Delegation', 'schema', 'the window is exactly 172,800 seconds'), deleg_short)
reg('B-delegation-window-172801', 'bytes',
    REJ('Delegation', 'schema', 'the window is exactly 172,800 seconds'), deleg_long)
reg('N-delegation-classical-only', 'bytes',
    REJ('Delegation', 'schema', 'hybrid: one entry per algorithm, a classical-only delegation is malformed'), deleg_classical)
reg('N-currency-delegation-mismatch', 'bytes',
    REJ('CurrencyAttestation', 'semantic', 'field 8 names a key other than the one that signed field 7'),
    currency_deleg_mismatch)
reg('P-frame-prekey-request-device', 'bytes', ACC('frame', 'PrekeyRequest naming the device, a one-time key asked for'), f_pk1)
reg('N-shape-prekey-request-onetime-without-device', 'bytes',
    REJ('frame', 'schema', 'field 4 required when field 2 = 1: a one-time key is one device\'s'), f_pk1_nodevice)
reg('N-shape-relay-missing-device', 'bytes',
    REJ('frame', 'schema', 'RelaySubmission field 4 required: a session is with a device'), f_relay_nodevice)
reg('B-prekey-reply-8', 'bytes', ACC('PrekeyReply', 'eight bundles, the ceiling'), r_pk_8)
reg('B-prekey-reply-9', 'bytes', REJ('PrekeyReply', 'schema', 'nine bundles exceed the ceiling of eight'), r_pk_9)
reg('P-archive-request-frontier-2', 'bytes', ACC('frame', 'a two-txid frontier, past a merge'), f_archive_frontier2)
reg('B-archive-frontier-0', 'bytes', REJ('frame', 'schema', 'an empty frontier: field 2 is [ + txid ]'),
    frame(2, e_map([(e_uint(1), e_bstr(alice.keyhash)), (e_uint(2), e_arr([])),
                    (e_uint(3), e_uint(16)), (e_uint(5), e_bstr(NONCE(b'archive-0')))])))
reg('N-shape-archive-reply-frontier-without-more', 'bytes',
    REJ('ArchiveReply', 'schema', 'field 4 present iff field 3 is true'),
    e_map([(e_uint(1), e_bstr(NONCE(b'ab'))), (e_uint(2), e_arr([adopt_env])),
           (e_uint(3), b'\xf4'), (e_uint(4), e_arr([e_bstr(adopt_txid)]))]))
reg('P-catalog-scoped', 'bytes', ACC('CatalogEntry', 'connect_scope present: dunbar'),
    sign1_slot(sorted(cat_pairs + [(e_uint(6), e_uint(5))], key=lambda p: p[0]), 8, AAD_CATALOG, bob))
# A conforming truncated reply [0.6 phase 2, 2026-09-02]: continuation present
# entails MORE than 111 qualifying entries, and the node returns the FIRST 111
# by (resource, owner) keyhash with the withheld 112th's type as the hint. The
# earlier one-entry-plus-continuation object was unproducible by a conforming
# host. 111 forum entries sort below the wiki entry's all-high resource id.
def _cat_variant(res_kh, stype):
    pairs = [(e_uint(1), e_bstr(res_kh)), (e_uint(2), e_bstr(bob.keyhash)),
             (e_uint(3), e_tstr(stype)), (e_uint(4), e_tstr('The Reading Room')),
             (e_uint(5), e_bstr(b'quic://198.51.100.7:4433'))]
    return sign1_slot(pairs, 8, AAD_CATALOG, bob)
_trunc_ids = sorted(H(b'rhtn-test-vectors:trunc-res:%d' % i) for i in range(111))
_trunc_entries = [_cat_variant(r, 'rhtn-forum') for r in _trunc_ids]
# the withheld 112th: resource id above every SHA-256 output here
_wiki_id = b'\xff' * 31 + b'\x01'
assert all(r < _wiki_id for r in _trunc_ids)
reg('P-catalog-reply-truncated', 'bytes',
    ACC('CatalogReply', 'a CONFORMING truncated reply: the first 111 entries by (resource, owner) keyhash, continuation naming the withheld 112th entry type'),
    e_map([(e_uint(1), e_bstr(NONCE(b'catalog'))),
           (e_uint(2), e_arr(_trunc_entries)),
           (e_uint(3), e_tstr('rhtn-wiki'))]))
reg('P-channel-bound', 'bytes', ACC('Channel', 'claimed resolution and session-key binding both present'),
    e_map([(e_uint(1), e_uint(1)), (e_uint(2), e_uint(0)), (e_uint(3), e_uint(1)),
           (e_uint(4), e_bstr(H(b'rhtn-test-vectors:uwb-session-key')[:32]))]))
reg('P-integrity-evidence', 'bytes', ACC('ClientIntegrity', 'evidence bstr present'),
    e_map([(e_uint(1), b'\xf5'), (e_uint(2), e_uint(2)), (e_uint(3), e_bstr(H(b'attest-quote')))]))

# ---- Over-strictness stress family [author, 2026-09-02]: each entry stresses
# a spot where a clean-room implementer in the 0.6 rounds adopted logic
# stricter than the specification - divergences the spec alone demonstrably
# did not prevent, so the suite must catch them.
os_empty_body = e_map([
    (e_uint(0), backptrs(*[[genesis(x.keyhash)] for x in (n_hi, n_lo, IDS['w1'])])),
    (e_uint(1), e_uint(TS_C2 + 3 * 86400)),
    (e_uint(2), e_uint(TS_C2 + 3 * 86400 + 1800)),
    (e_uint(3), e_arr([participant(n_hi), participant(n_lo)])),
    (e_uint(4), e_arr([witness_entry(IDS['w1'], alice, 7)])),
    (e_uint(5), e_arr([])),
    (e_uint(6), e_uint(0)),
    (e_uint(8), e_bstr(fin_nm_root)),
])
reg('N-responses-empty-array', 'bytes',
    REJ('body', 'schema', 'zero responses OMIT key 5 - the empty-array spelling is the second encoding of one logical record (s4.5, T30)'),
    os_empty_body,
    note='Stress: an implementer reading field 5 as required-with-empty-array emits and accepts this; P-fin-absent is the one valid spelling.')

retry_values = dict(npr_values)
retry_values['proximity'] = e_map([(e_uint(1), e_arr([
    e_map([(e_uint(1), e_uint(3)), (e_uint(2), e_uint(1))]),   # optical, FAIL
    e_map([(e_uint(1), e_uint(3)), (e_uint(2), e_uint(0))])])), # optical retried, PASS
    (e_uint(2), e_uint(3))])
cr_slots, cr_root = disclosure_set('channel-retry', retry_values)
cr_signers = [n_hi, n_lo, IDS['w3']]
cr_body = e_map([
    (e_uint(0), backptrs(*[[genesis(x.keyhash)] for x in cr_signers])),
    (e_uint(1), e_uint(TS_C2 + 4 * 86400)),
    (e_uint(2), e_uint(TS_C2 + 4 * 86400 + 1800)),
    (e_uint(3), e_arr([participant(n_hi), participant(n_lo)])),
    (e_uint(4), e_arr([witness_entry(IDS['w3'], bob, 7)])),
    (e_uint(6), e_uint(0)),
    (e_uint(8), e_bstr(cr_root)),
])
cr_txid = H(cr_body)
cr_env, _ = envelope(1, 5, cr_body, cr_signers)
cr_pres = presented(cr_env, cr_slots, set(LABELS))
reg('P-channel-retry', 'bytes', ACC('envelope', 'a repeated channel kind: optical failed, optical retried and passed (D19)'),
    cr_env,
    note='Stress: a validator imposing one-entry-per-kind rejects this valid record; a retried channel is two measurements (s4.5).')
reg('P-channel-retry-presentation', 'bytes', ACC('presentation', 'fully revealed; the proximity strongest-rule passes over the retried channel'),
    cr_pres)

# ---- traces and contexts (structured, no bytes)
TRACE_ONE_OF = {'TR2': ['defer_until_handshake', 'reject'],
                'TR15': ['defer_until_handshake', 'reject']}
for trid, seq, actions, cite in [
    ('TR1', 'control frame with unknown frame_type 99 arrives mid-session', ['skip_frame', 'session_survives'], '§8.0'),
    ('TR2', 'Attach arrives in TLS 1.3 0-RTT early data', ['never_process_as_early_data'], '§8.2, §9.2'),
    ('TR3', 'bidirectional stream opens with unknown request_type 99', ['close_stream', 'session_survives'], '§9.2'),
    ('TR4', 'malformed SiblingUpdate arrives', ['ignore_whole', 'previous_list_stands', 'session_survives'], '§8.2'),
    ('TR5', 'second Attach on an attached session', ['fail_attach'], '§8.2'),
    ('TR6', 'stream-0 frame with length prefix over 65,536', ['protocol_error'], '§8.0'),
    ('TR7', 'heartbeat counter gap observed', ['liveness_by_intervals_only'], '§8.2'),
    ('TR8', 'Attach carries an attestation failing validation', ['treat_attestation_absent', 'session_attaches'], '§8.2'),
    ('TR9', 'primary closes with application code 1 (refused) at attach', ['no_sibling_failover', 'attach_refused'], '§9.2'),
    ('TR10', 'a failover sibling closes with code 1', ['foreclose_that_sibling_only', 'try_next_candidate'], '§9.2'),
    ('TR11', 'a Referral arrives without key_material and the requester holds no pin for the next hop', ['dial_unauthenticated', 'disclose_nothing_beyond_query'], '§7.7.3'),
    ('TR12', 'heartbeats 0 and 2 arrive; beat 1 was lost in transit', ['accept_gapped_beat', 'reset_liveness', 'no_failover'], '§8.2'),
    ('TR13', 'a failover attach to a cached sibling returns AttachAck mode 0 (primary)', ['accept_server_mode', 'no_client_inference'], '§8.2'),
    ('TR14', 'a ServingInfra reply arrives after two of five path indices were consumed', ['resolution_complete', 'no_arrival_equation'], '§7.7.3'),
    ('TR15', 'a ResourceRequest (type 6, an HTTP GET inside) arrives in TLS 1.3 0-RTT early data', ['never_process_as_early_data'], '§9.2, §11'),
    ('TR16', 'one transport session carries hosted sessions to resources A and B; the caller role row for A changes', ['retire_A_resource_session_id', 'transport_survives', 'B_session_unaffected', 'in_flight_A_completes_under_starting_snapshot'], 'infra-client-requirements.md §10.1, §10.5'),
    ('TR17', 'a non-member request names a resource whose snapshot still holds a stale role row for the requester', ['status_refused', 'never_a_member_specific_status'], '§11'),
    ('TR18', 'a frame parses as [6, body] but the body is not a well-formed ResourceRequest', ['answer_status_3', 'no_stream_reset'], '§9.2, §11'),
    ('TR19', 'a stored topology transaction arrives again through a peering cycle', ['drop_duplicate', 'no_forward', 'session_survives'], '§10.1'),
    ('TR20', 'a memo arrives naming the receiver in field 1, but its own records do not confirm the change', ['reject_hint', 'no_disavowal', 'nothing_severed'], '§10.2'),
    ('TR21', 'an ordinary memo from below arrives; the receiver position is a prefix of the field-2 path', ['forward_rootward', 'no_cycle'], '§10.2'),
    ('TR22', 'a memo names the receiver in field 1, its own records confirm the change and the current slot state, and no live disambiguation is available', ['disavow_ingress_subordinate', 'reason_5_without_prejudice', 'memo_terminates'], '§10.2, §4.3'),
    ('TR23', 'a memo arrives whose field-2 anchor is not a subnet the receiver holds a line in', ['drop', 'no_table_write', 'no_forward'], '§10.2'),
    ('TR24', 'a type-4 request arrives whose query field 7 names a different verifier', ['close_stream', 'no_response', 'no_processing'], '§5.6'),
]:
    exp = {'actions': actions, 'cite': cite}
    if trid in TRACE_ONE_OF:
        exp['one_of'] = TRACE_ONE_OF[trid]
    reg(trid, 'trace', exp, note=seq)
for cid, inputs, expect in [
    ('V9', {'adoption': 'P-adopt-optionals', 'dereference': 'P-normal-record'},
     {'structural': 'valid', 'checks': {'proof_of_presence': 'pass'}}),
    ('V9a', {'adoption': 'P-adopt-divergent', 'dereference': 'P-formation'},
     {'structural': 'valid', 'checks': {'proof_of_presence': 'fail'}}),
    ('V9b', {'adoption': 'P-adopt-optionals', 'dereference': 'unfetchable'},
     {'structural': 'valid', 'checks': {'proof_of_presence': 'unverifiable(unfetchable)'}}),
    ('CTX-bundle', {'bundle': ['P-formation', 'P-ac1', 'P-ac2', 'P-ac1', 'P-normal-record', 'mutated P-formation'],
                    'evaluated_at': 'TS_EVAL', 'subject': 'alice', 'counterparty': 'bob'},
     {'n': 4, 'candidates': 1, 'required': 1}),
]:
    reg(cid, 'context', expect, inputs=inputs)

pc_a = H(b'rhtn-test-vectors:precommit-contribution:alice')[:16]
pc_b = H(b'rhtn-test-vectors:precommit-contribution:bob')[:16]
pc_first, pc_second = (pc_a, pc_b) if alice.keyhash < bob.keyhash else (pc_b, pc_a)
pc_demo = H(b'rhtn/1:ceremony' + pc_first + pc_second)

emit('records.md', f"""
## Ceremony pre-commitment construction (design §7.5.2) — known answer

Contributory: SHA-256 of the ASCII tag `rhtn/1:ceremony` followed by each
participant's 16 random bytes, in ascending participant-keyhash order
(here {'alice then bob' if alice.keyhash < bob.keyhash else 'bob then alice'}).

contributions (alice, bob):

```
{hx(pc_a)}
{hx(pc_b)}
```

pre-commitment:

```
{hx(pc_demo)}
```

*(The presence-record fixtures predate this construction and carry arbitrary
32-byte pre-commitments; construction is unobservable from a record, so they
remain valid inputs.)*

## Capture-key derivation (design §7.5.2) — known answer

HKDF-SHA-256, salt empty, IKM the seed, info the ASCII tag `rhtn/1:capture`
followed by the raw subject keyhash, holder keyhash and ceremony
pre-commitment, output 32 bytes. Subject alice, holder c1, ceremony the
**prior alice–c1 meeting's** contributory pre-commitment (`transactions.md`) —
the ceremony that sealed the capture, never the one under assembly.

seed:

```
{hx(demo_seed)}
```

info (14-byte tag + 3 × 32 bytes):

```
{hexblock(b'rhtn/1:capture' + alice.keyhash + IDS['c1'].keyhash + pc1_precommit)}
```

k_capture:

```
{hx(demo_k)}
```

The `KeyGrant` in `messages.md` carries exactly this key, bound to the normal
record's txid and its first worked query.

## Pairwise principal (design §11.0.2) — known answer

`SHA-256("rhtn/1:pairwise" || resource_keyhash || user_keyhash)` — computed by
whichever node currently hosts the resource, so two node implementations MUST
agree byte-for-byte or a provider migration renames every user the resource
knows. Resource c1, user alice (keyhashes in `keys.md`):

principal_id:

```
{hx(H(b'rhtn/1:pairwise' + IDS['c1'].keyhash + alice.keyhash))}
```
""")

# ---------------------------------------------------------------- write files

import os
os.chdir(os.path.join(os.path.dirname(os.path.abspath(__file__)), '..'))
with open('corpus.json', 'w', encoding='utf-8', newline='\n') as f:
    _json.dump({'format': 'rhtn-test-corpus/1',
                'pins': {'wire-format.md': WIRE_SHA, 'network-design.md': DESIGN_SHA,
                         'light-client-requirements.md': LIGHT_SHA},
                'classes': {'bytes': 'hex is the complete input',
                            'unit': 'no bytes: note carries a deterministic recipe',
                            'trace': 'a session event sequence with required actions; where the spec permits alternatives, one_of lists them and satisfying any one conforms',
                            'context': 'named fixture inputs with an expected evaluation'},
                'entries': REG}, f, indent=1, sort_keys=False)
    f.write('\n')
# ---- hash-language disjointness (wire §1.1's invariant, asserted) ----
# The four untagged hash preimage languages must stay pairwise disjoint. A
# schema change that broke this would otherwise pass generation silently.
def _assert_hash_languages(bodies):
    for b in bodies:  # txid preimages: maps whose first encoded key is 0
        assert b[0] >> 5 == 5 and b[1] == 0x00, 'body map must begin with key 0'
        assert len(b) > 32, 'body shorter than a genesis preimage'
    for i in IDS.values():  # keyhash preimages: arrays, first byte 0x82
        assert i.key_material[0] == 0x82 and len(i.key_material) > 32
    # query_id preimages: five-entry maps beginning key 1 (0xa5 0x01) — no
    # query is generated yet, but the language stays reserved and disjoint:
    # bodies begin a? 00, KeyMaterial begins 82, genesis input is exactly 32B.
_assert_hash_languages([adopt_body, depart_body, disavow_body, reissue_body,
                        formation_body, div_body, merge_body, code40_body,
                        ext_body, reissue2_body, adopt_full_body, depart_r_body,
                        peer_full_body, peer_body])

_outputs = {}
_pending = {}
for fname, parts in OUT.items():
    data = '\n'.join(parts) + '\n'
    _pending[fname] = data
    _outputs[fname] = hashlib.sha256(data.encode()).hexdigest()
# Output-drift gate: identical specs and tools must reproduce identical bytes.
# A divergence here means an untracked input changed — a crypto dependency,
# the platform — and silently baselining it would defeat the pins (ninth
# review). Any acceptance flag covers it, since those already assert an audit.
if _STORED_OUTPUTS and not _ACCEPTED:
    _drift = [f for f in _outputs
              if f in _STORED_OUTPUTS and _STORED_OUTPUTS[f] != _outputs[f]]
    if _drift:
        sys.exit('outputs changed with specs and tools unchanged: '
                 + ', '.join(_drift) + '\nAn untracked input (dependency, '
                 'platform) altered generation. Diagnose, then rerun with '
                 '--accept-output-change.')
for fname, data in _pending.items():
    with open(fname, 'w', encoding='utf-8', newline='\n') as f:
        f.write(data)
    print(f"wrote {fname}")
_repin_hand_file('README.md')
_repin_hand_file('negative-vectors.md')
for name in ('README.md', 'negative-vectors.md'):
    path = os.path.join(os.path.dirname(os.path.abspath(__file__)), '..', name)
    _outputs[name] = hashlib.sha256(open(path, 'rb').read()).hexdigest()
_write_pins(_outputs)
print("repinned README.md, negative-vectors.md; pins written")
