#!/usr/bin/env python3
"""Independent verification harness for the RHTN test vectors.

Deliberately separate from generate.py: its own CBOR decoder and encoder, its
own Sig_structure reconstruction, no shared code — so agreement between the two
is evidence, not tautology. Reads the generated markdown, re-derives and checks
every claim it can reach:

  - all eleven identities' keyhashes, from the stated seeds (ML-DSA public keys
    re-derived via dilithium-py, and cross-checked via pyca `cryptography`'s
    independent ML-DSA implementation where importable);
  - every transaction body: canonical parse (sorted unique keys, definite
    lengths, shortest forms via re-encode comparison) and txid;
  - every envelope: entry order, header profile, and every signature under
    both algorithms — ML-DSA verified with pyca where importable, else
    dilithium-py;
  - the standalone COSE_Sign1 objects (SignedLocator ×2, EndpointRecord ×3)
    and the wrong-signer negative (must NOT verify under the named subject);
  - the extension-coverage mutations: E10 (top-level and nested) and E13 must
    break their signatures;
  - the reasonableness arithmetic: required() table rows against the formula;
  - the formation record's structural claims (R8 genesis form, R14 ordinal).

Exit status 0 only if every check passes.
Requires: cryptography (Ed25519), dilithium-py; pyca cryptography >= 45
optionally strengthens the ML-DSA check to a second implementation.
"""

import hashlib, hmac, os, re, sys

HERE = os.path.join(os.path.dirname(os.path.abspath(__file__)), '..')
def read(name):
    return open(os.path.join(HERE, name), encoding='utf-8').read()

from cryptography.hazmat.primitives.asymmetric.ed25519 import Ed25519PrivateKey, Ed25519PublicKey
from cryptography.hazmat.primitives.serialization import Encoding, PublicFormat
from dilithium_py.ml_dsa import ML_DSA_65
try:
    from cryptography.hazmat.primitives.asymmetric.mldsa import (
        MLDSA65PublicKey, MLDSA65PrivateKey)
    PYCA_MLDSA = True
except ImportError:
    PYCA_MLDSA = False

H = lambda b: hashlib.sha256(b).digest()
FAILURES = []
def check(cond, what):
    if cond:
        print('  ok  ' + what)
    else:
        print('FAIL  ' + what)
        FAILURES.append(what)

# ---------------------------------------------------------------- CBOR
def parse(b, off=0):
    ib = b[off]; mt = ib >> 5; ai = ib & 0x1f; off += 1
    if ai < 24: n = ai
    elif ai == 24:
        n = b[off]; off += 1
        if n < 24: raise ValueError('non-shortest form')
    elif ai == 25:
        n = int.from_bytes(b[off:off+2], 'big'); off += 2
        if n < 0x100: raise ValueError('non-shortest form')
    elif ai == 26:
        n = int.from_bytes(b[off:off+4], 'big'); off += 4
        if n < 0x10000: raise ValueError('non-shortest form')
    elif ai == 27:
        n = int.from_bytes(b[off:off+8], 'big'); off += 8
        if n < 0x100000000: raise ValueError('non-shortest form')
    else: raise ValueError('indefinite length')
    if mt == 0: return n, off
    if mt == 1: return -1 - n, off
    if mt == 2: return b[off:off+n].hex(), off + n
    if mt == 3: return ('tstr', b[off:off+n].decode()), off + n
    if mt == 4:
        out = []
        for _ in range(n):
            v, off = parse(b, off); out.append(v)
        return out, off
    if mt == 5:
        out = {}; prev = None
        for _ in range(n):
            ks = off; k, off = parse(b, off); kb = b[ks:off]
            if prev is not None and kb <= prev:
                raise ValueError('map keys unsorted or duplicate')
            prev = kb; v, off = parse(b, off); out[k] = v
        return out, off
    if mt == 7 and ai == 20: return False, off
    if mt == 7 and ai == 21: return True, off
    if mt == 7 and ai == 22: return None, off
    raise ValueError((mt, ai))

def hd(m, n):
    if n < 24: return bytes([m << 5 | n])
    if n < 0x100: return bytes([m << 5 | 24, n])
    if n < 0x10000: return bytes([m << 5 | 25]) + n.to_bytes(2, 'big')
    if n < 0x100000000: return bytes([m << 5 | 26]) + n.to_bytes(4, 'big')
    return bytes([m << 5 | 27]) + n.to_bytes(8, 'big')
bs = lambda x: hd(2, len(x)) + x
ts = lambda x: hd(3, len(x.encode())) + x.encode()
def enc(o):
    if isinstance(o, bool): return b'\xf5' if o else b'\xf4'
    if isinstance(o, int): return hd(0, o) if o >= 0 else hd(1, -1 - o)
    if isinstance(o, tuple) and len(o) == 2 and o[0] == 'tstr': return ts(o[1])
    if isinstance(o, str): return bs(bytes.fromhex(o))
    if isinstance(o, list): return hd(4, len(o)) + b''.join(enc(x) for x in o)
    if isinstance(o, dict):
        pr = sorted((enc(k), enc(v)) for k, v in o.items())
        return hd(5, len(pr)) + b''.join(a + b for a, b in pr)
    if o is None: return b'\xf6'
    raise TypeError(o)

def canonical(b):
    """Byte-level canonical validation: the parser itself rejects duplicate or
    unsorted map keys before materialising, indefinite lengths, and
    non-shortest integer forms — the E9/C1 method, not decode->re-encode
    equality. The re-encode comparison remains as a redundant cross-check of
    this harness's own encoder, never the verdict."""
    try:
        obj, end = parse(b)
    except ValueError:
        return None
    if end != len(b):
        return None
    assert enc(obj) == b, 'harness encoder disagrees with its parser'
    return obj

def sig_sign(prot, payload):
    return hd(4, 5) + ts('Signature') + bs(b'') + bs(prot) + bs(b'rhtn/1:envelope') + bs(payload)
def sig_sign1(prot, aad, payload):
    return hd(4, 4) + ts('Signature1') + bs(prot) + bs(aad) + bs(payload)

# ---------------------------------------------------------------- identities
NAMES = (['alice', 'bob', 'carol', 'alice2'] + [f'w{i}' for i in range(1, 17)]
         + ['c1', 'c2', 'c3', 'c4', 'c5'])
ED, PQ, KH = {}, {}, {}
for n in NAMES:
    ed = Ed25519PrivateKey.from_private_bytes(H(f'rhtn-test-vectors:{n}:ed25519-seed'.encode()))
    ED[n] = ed.public_key().public_bytes(Encoding.Raw, PublicFormat.Raw)
    xi = H(f'rhtn-test-vectors:{n}:ml-dsa-65-seed'.encode())
    pk, _ = ML_DSA_65.key_derive(xi)
    if PYCA_MLDSA:
        assert MLDSA65PrivateKey.from_seed_bytes(xi).public_key().public_bytes_raw() == pk
    PQ[n] = pk
    km = hd(4, 2) \
        + (hd(5, 3) + b'\x01\x01\x20\x06\x21' + bs(ED[n])) \
        + (hd(5, 3) + b'\x01\x07\x03\x38\x30\x20' + bs(pk))
    KH[n] = H(km).hex()
keys_md = read('keys.md')
stated = dict(re.findall(r'\| (\w+) \| [^|]+ \| `[0-9a-f]{64}` \| `([0-9a-f]{64})` \|', keys_md))
check(all(stated.get(n) == KH[n] for n in NAMES),
      f'keyhashes: all {len(NAMES)} re-derived from seeds'
      + (' (ML-DSA cross-checked against pyca)' if PYCA_MLDSA else ' (dilithium-py only)'))
BY = {KH[n]: n for n in NAMES}
# transport keys (wire §8.2): raw Ed25519, no keyhash, re-derived from the seed recipe
TKNAMES = ['bob-instance', 'carol-instance', 'alice-desktop']
TKPUB = {}
for n in TKNAMES:
    TKPUB[n] = Ed25519PrivateKey.from_private_bytes(H(f'rhtn-test-vectors:{n}:transport-seed'.encode())) \
        .public_key().public_bytes(Encoding.Raw, PublicFormat.Raw)
stated_tk = dict(re.findall(r'\| ([a-z]+-[a-z]+) \| [^|]+ \| `([0-9a-f]{64})` \|', keys_md))
check(all(stated_tk.get(n) == TKPUB[n].hex() for n in TKNAMES),
      f'transport keys: all {len(TKNAMES)} re-derived from seeds')

def verify_raw(pub, sig, tbs):
    try:
        Ed25519PublicKey.from_public_bytes(pub).verify(sig, tbs); return True
    except Exception:
        return False

def verify_sig(name, alg, sig, tbs):
    if alg == -8:
        try:
            Ed25519PublicKey.from_public_bytes(ED[name]).verify(sig, tbs); return True
        except Exception:
            return False
    if PYCA_MLDSA:
        try:
            MLDSA65PublicKey.from_public_bytes(PQ[name]).verify(sig, tbs); return True
        except Exception:
            return False
    return ML_DSA_65.verify(PQ[name], tbs, sig)

# ------------------------------------------------- schema-aware validation
def is_h32(v): return isinstance(v, str) and len(v) == 64
def is_seq(v): return (isinstance(v, list) and len(v) == 2
                       and all(isinstance(x, int) and 0 <= x < 2**32 for x in v))
def is_loc(v): return (isinstance(v, dict) and is_h32(v.get(1))
                       and isinstance(v.get(2), dict) and is_seq(v.get(3)))
SCHEMAS = {  # type: (required {field: predicate}, signer-role fields)
    1: ({1: is_h32, 2: is_h32, 3: is_loc, 4: lambda v: isinstance(v, int)}, (1, 2)),
    2: ({1: is_h32, 2: is_h32, 3: is_seq, 4: lambda v: isinstance(v, int)}, (1,)),
    3: ({1: is_h32, 2: is_h32, 3: lambda v: isinstance(v, int)}, (1,)),
    4: ({1: is_h32, 2: is_h32, 3: lambda v: isinstance(v, dict),
         4: lambda v: isinstance(v, dict), 5: lambda v: isinstance(v, int)}, (1, 2)),
    7: ({1: is_h32, 2: is_h32, 3: is_seq, 4: is_seq,
         5: lambda v: isinstance(v, int)}, (1, 2)),
    5: ({1: lambda v: isinstance(v, int), 2: lambda v: isinstance(v, int),
         3: lambda v: isinstance(v, list) and len(v) == 2,
         6: lambda v: v in (0, 1),
         8: is_h32}, None),  # signer set is dynamic: participants + witnesses
}
def validate_body(t, obj):
    req, roles = SCHEMAS[t]
    if 0 not in obj or not isinstance(obj[0], list): return 'missing key 0'
    for lst in obj[0]:
        if not (isinstance(lst, list) and 1 <= len(lst) <= 8
                and all(is_h32(h) for h in lst)): return 'bad back-pointer list'
        if len(lst) > 1 and lst != sorted(lst): return 'merge list unsorted'
    expected_lists = (len(obj[3]) + len(obj.get(4, [])) if roles is None
                      else len(roles))
    if len(obj[0]) != expected_lists: return 'wrong signer-list count'
    for f, pred in req.items():
        if f not in obj: return f'missing field {f}'
        if not pred(obj[f]): return f'field {f} wrong shape'
    return None

# ---------------------------------------------------------------- bodies + envelopes
tx = read('transactions.md')
bodies = re.findall(r'```\n([0-9a-f\n]+?)```\n\ntxid: `([0-9a-f]{64})`', tx)
ok = 0
for hexs, txid in bodies:
    b = bytes.fromhex(hexs.replace('\n', ''))
    obj = canonical(b)
    if obj is not None and hashlib.sha256(b).hexdigest() == txid: ok += 1
check(ok == len(bodies), f'bodies: {ok}/{len(bodies)} canonical with matching txid')

envs = 0; sigs = 0; sig_ok = 0; ext_env = None
for m in re.finditer(r'```\n(a4[0-9a-f\n]+?)```', tx):
    b = bytes.fromhex(m.group(1).replace('\n', ''))
    try:
        obj = canonical(b)
    except Exception:
        continue
    if not isinstance(obj, dict) or set(obj) != {1, 2, 3, 4}: continue
    envs += 1
    body = enc(obj[3])
    t = obj[2]
    err = validate_body(t, obj[3])
    assert err is None, f'type-{t} body schema: {err}'
    if t == 1:
        assert obj[3][3][3][1] == 0, 'adoption locator must open its series at counter 0 (s4.1)'
    if SCHEMAS[t][1] is None:  # presence: participants + witnesses
        derived = {pp[1] for pp in obj[3][3]}
        derived |= {w[1] for w in obj[3].get(4, [])}
    else:
        derived = {obj[3][f] for f in SCHEMAS[t][1]}
    kids = {parse(bytes.fromhex(e[0]))[0][4] for e in obj[4][3]}
    assert kids == derived, f'type-{t} envelope signers != body roles'
    if isinstance(obj[3], dict) and 99 in obj[3]: ext_env = (obj, body)
    prev = None
    for entry in obj[4][3]:
        prot = bytes.fromhex(entry[0]); pobj, _ = parse(prot)
        name = BY[pobj[4]]; sigs += 1
        if verify_sig(name, pobj[1], bytes.fromhex(entry[2]), sig_sign(prot, body)):
            sig_ok += 1
        key = (pobj[4], 0 if pobj[1] == -8 else 1)
        assert prev is None or key > prev, 'entry order violated'
        prev = key
check(sig_ok == sigs, f'envelopes: {envs} found, {sig_ok}/{sigs} signatures verify (both algorithms)')

# E10: both extension mutations break all four signatures
obj, body = ext_env
muts = [body.replace(bytes.fromhex('c0ffee'), bytes.fromhex('c0ffef')),
        body.replace(hd(0, 99) + bs(bytes.fromhex('beef')),
                     hd(0, 99) + bs(bytes.fromhex('beee')))]
broken = 0
for bad in muts:
    assert bad != body
    for entry in obj[4][3]:
        prot = bytes.fromhex(entry[0]); pobj, _ = parse(prot)
        if not verify_sig(BY[pobj[4]], pobj[1], bytes.fromhex(entry[2]), sig_sign(prot, bad)):
            broken += 1
check(broken == 8, 'E10: top-level and nested mutations each break all four signatures')

# ---------------------------------------------------------------- standalone Sign1
pr = read('primitives.md'); rc = read('records.md')
def sign1_object(hexs, aad, sig_slot):
    """sig_slot is SCHEMA-FIXED (SignedLocator: 3; EndpointRecord: 4) — never
    inferred from key magnitude, or an unknown extension key below the slot
    would be misread as the signature (eighth review)."""
    b = bytes.fromhex(hexs.replace('\n', ''))
    obj = canonical(b); assert obj is not None
    payload = enc({k: v for k, v in obj.items() if k != sig_slot})
    prot = bytes.fromhex(obj[sig_slot][0]); sig = bytes.fromhex(obj[sig_slot][3])
    return obj, prot, sig, sig_sign1(prot, aad, payload)

m = re.search(r'## SignedLocator.*?Complete `SignedLocator`.*?```\n([0-9a-f\n]+?)```', pr, re.S)
obj, prot, sig, tbs = sign1_object(m.group(1), b'rhtn/1:locator', 3)
check(verify_sig(BY[obj[1]], -8, sig, tbs), 'SignedLocator: signature by the named subject')
m = re.search(r'## Same-series counter jump.*?Complete object.*?```\n([0-9a-f\n]+?)```', pr, re.S)
obj, prot, sig, tbs = sign1_object(m.group(1), b'rhtn/1:locator', 3)
check(verify_sig(BY[obj[1]], -8, sig, tbs) and obj[2][3] == [5, 100],
      'counter-jump SignedLocator: verifies, seqno [5,100]')
m = re.search(r'## The `SignedLocator` equal-seqno conflict partner.*?```\n([0-9a-f\n]+?)```', pr, re.S)
objb, protb, sigb, tbsb = sign1_object(m.group(1), b'rhtn/1:locator', 3)
check(verify_sig(BY[objb[1]], -8, sigb, tbsb) and objb[2][3] == [5, 100],
      'V12 partner: same subject and seqno as the counter-jump locator, different path, valid')
m = re.search(r"## A root's self-anchored `SignedLocator`.*?```\n([0-9a-f\n]+?)```", pr, re.S)
obj, prot, sig, tbs = sign1_object(m.group(1), b'rhtn/1:locator', 3)
check(verify_sig(BY[obj[1]], -8, sig, tbs) and obj[2][2] == {1: '', 2: 0}
      and obj[1] == obj[2][1],
      'D13: root self-anchors — empty path {1: h\'\', 2: 0}, anchor = subject, verifies')
m = re.search(r'## A `SignedLocator` carrying an unknown extension.*?```\n([0-9a-f\n]+?)```', pr, re.S)
obj, prot, sig, tbs = sign1_object(m.group(1), b'rhtn/1:locator', 3)
check(verify_sig(BY[obj[1]], -8, sig, tbs) and 4 in obj,
      'D9: extension SignedLocator (unknown key 4) verifies with the key in the payload')
bad = enc({k: (v if k != 4 else 'ab') for k, v in obj.items() if k != 3})
check(not verify_sig(BY[obj[1]], -8, sig, sig_sign1(prot, b'rhtn/1:locator', bad)),
      'E14: mutating the SignedLocator extension breaks the signature')
m = re.search(r'## A wrong-signer `SignedLocator`.*?```\n([0-9a-f\n]+?)```', pr, re.S)
obj, prot, sig, tbs = sign1_object(m.group(1), b'rhtn/1:locator', 3)
check(not verify_sig(BY[obj[1]], -8, sig, tbs) and verify_sig('bob', -8, sig, tbs),
      'S23 wrong-signer: rejected under field 1, valid under bob — binding is the only defect')
found = []
for m in re.finditer(r'```\n(a[45][0-9a-f\n]+?)```', rc):
    b = bytes.fromhex(m.group(1).replace('\n', ''))
    try:
        obj = canonical(b)
    except Exception:
        continue
    if (isinstance(obj, dict) and 4 in obj and isinstance(obj[4], list)
            and len(obj[4]) == 4 and isinstance(obj[4][0], str)):
        payload = enc({k: v for k, v in obj.items() if k != 4})
        prot = bytes.fromhex(obj[4][0]); sig = bytes.fromhex(obj[4][3])
        good = verify_sig(BY[obj[1]], -8, sig, sig_sign1(prot, b'rhtn/1:endpoints', payload))
        found.append((obj, sig, prot, payload, good))
check(len(found) == 3 and all(g for *_, g in found),
      f'EndpointRecords: {len(found)} found, all verify under the named node')
pair = [o for o, *_ in found if 99 not in o]
check(len(pair) == 2 and pair[0][3] == pair[1][3] and pair[0][2] != pair[1][2],
      'conflict pair: same seqno, different endpoints, both individually valid')
extr = [(o, s, p, pl) for o, s, p, pl, _ in found if 99 in o][0]
bad = enc({k: (v if k != 99 else 'c0ffef') for k, v in extr[0].items() if k != 4})
check(not verify_sig(BY[extr[0][1]], -8, extr[1], sig_sign1(extr[2], b'rhtn/1:endpoints', bad)),
      'E13: mutating the Sign1-path extension breaks the signature')

# ------------------------------------------------- selective disclosure (§4.5.1)
LABELS = ['capture', 'location', 'p0.integrity', 'p0.retention',
          'p1.integrity', 'p1.retention', 'proximity']
dtab = re.findall(r'\| `([a-z01.]+)` \| `([0-9a-f]{32})` \| `([0-9a-f]+)` \| `([0-9a-f]{64})` \|', tx)
assert [r[0] for r in dtab] == LABELS, 'disclosure table labels'
digests = {}
for lab, salt, val, dig in dtab:
    D = hd(4, 3) + bs(bytes.fromhex(salt)) + ts(lab) + bytes.fromhex(val)
    assert H(b'\x00' + D).hex() == dig, f'digest {lab}'
    digests[lab] = bytes.fromhex(dig)
root = H(b'\x01' + b''.join(digests[l] for l in LABELS))
froot = re.search(r'root: `([0-9a-f]{64})`', tx).group(1)
check(root.hex() == froot, 'disclosure root recomputed from salts, labels and values')
m = re.search(r'## Presence record \(type 5\), formation subtype — alice and carol'
              r'.*?```\n([0-9a-f\n]+?)```', tx, re.S)
fbody = canonical(bytes.fromhex(m.group(1).replace('\n', '')))
check(fbody[8] == froot, 'formation body field 8 equals the recomputed root')
pres_ok = 0
for m in re.finditer(r'\*\*(Fully revealed|Partial|Minimal)\*\*.*?```\n([0-9a-f\n]+?)```', tx, re.S):
    pr_obj = canonical(bytes.fromhex(m.group(2).replace('\n', '')))
    env, slots = pr_obj
    assert len(slots) == 7
    ds = []
    for i, slot in enumerate(slots):
        if isinstance(slot, str):          # withheld digest
            ds.append(bytes.fromhex(slot))
        else:                              # revealed Disclosure [salt, label, value]
            assert slot[1] == ('tstr', LABELS[i]), 'label != slot position'
            D = enc(slot)
            ds.append(H(b'\x00' + D))
    if H(b'\x01' + b''.join(ds)).hex() == env[3][8]:
        pres_ok += 1
check(pres_ok == 6, 'all six presentations (formation and normal record) recompute their roots')
v = read('verifier-selection.md')
good = True
for n, c, r in re.findall(r'\| (\d+) \| (\d+) \| (\d+) \|', v):
    good &= (min(int(n) // 2, 10, int(c)) == int(r))
check(good, 'required() table rows match the formula')

# ---------------------------------------------------------------- curated bundle (bar 3)
bsect = v[v.index('## The curated bundle'):]
arith = re.search(r'n = (\d+).*?candidates = (\d+).*?required = min\(floor\((\d+) / 2\), 10, (\d+)\) = (\d+)', bsect, re.S)
bn, bc, fn, fc, br = map(int, arith.groups())
check(bn == fn and bc == fc and min(fn // 2, 10, fc) == br,
      'bundle arithmetic: required recomputes from the stated n and candidates')
tx_txids = {t for _, t in bodies}
named = re.findall(r'`([0-9a-f]{16})…`', bsect)
check(len(named) == 4 and all(any(t.startswith(p) for t in tx_txids) for p in named),
      'bundle: all four qualifying txids resolve to transactions.md fixtures')
for n_, c_, r_ in re.findall(r'n = (\d+), candidates = (\d+), required = (?:min\([^)]*\) = )?(\d+)', bsect):
    check(min(int(n_) // 2, 10, int(c_)) == int(r_),
          f'bundle case n={n_}: required matches the formula')

# ---------------------------------------------------------------- formation structural
m = re.search(r'## Presence record \(type 5\), formation subtype — alice and carol'
              r'.*?```\n([0-9a-f\n]+?)```', tx, re.S)
obj = canonical(bytes.fromhex(m.group(1).replace('\n', '')))
p0, p1 = obj[3][0][1], obj[3][1][1]
check(4 not in obj and 5 not in obj and obj[6] == 1, 'formation: keys 4/5 absent, subtype 1')
check(obj[0] == [[H(bytes.fromhex(p0)).hex()], [H(bytes.fromhex(p1)).hex()]],
      'formation R8: key 0 is exactly the genesis value per signer')
check(7 not in obj, 'formation: retired key 7 absent')

# ---------------------------------------------------------------- recovery adoption
sect = tx[tx.index('## Recovery adoption'):]
blocks = re.findall(r'```\n([0-9a-f\n]+?)```', sect)
r_query = canonical(bytes.fromhex(blocks[0].replace('\n', '')))
r_body_hex = re.search(r'```\n([0-9a-f\n]+?)```\n\ntxid:', sect).group(1)
r_body = canonical(bytes.fromhex(r_body_hex.replace('\n', '')))
rq157 = enc({k: r_query[k] for k in (1, 2, 3, 4, 5, 7)})
check(H(rq157).hex() == r_query[6], 'recovery query_id = SHA-256 of fields 1-5 and 7')
check(r_query[7] == r_query[2], 'recovery query field 7 equals field 2: the querier is the verifier')
rec = r_body[6]; resp = rec[2][0]
check(len(rec[2]) >= 1 and resp[4] == 0, 'recovery carries at least one match')
check(resp[3] == r_query[6] and resp[2] == r_body[1] and resp[8] == rec[1]
      and rec[1] != r_body[1] and resp[10] == 0 and 6 not in resp and resp[5] == 1,
      'recovery response bindings: query_id, subject, prior_key, basis, selection_basis 0')
check(r_query[1] == r_body[1] and r_query[2] == resp[1],
      'recovery querier IS the verifier; query subject is the new key (design s9.1)')
con_prot, con_sig = bytes.fromhex(resp[7][0]), bytes.fromhex(resp[7][3])
check(verify_sig('alice2', -8, con_sig,
                 sig_sign1(con_prot, b'rhtn/1:consent', bytes.fromhex(r_query[6]))),
      'recovery consent verifies: Ed25519 by the NEW key over raw query_id')
vr_payload = enc({k: resp[k] for k in (1, 2, 3, 4, 5, 7, 8, 10)})
def sig_sign(prot, aad, payload):
    return hd(4, 5) + ts('Signature') + bs(b'') + bs(prot) + bs(aad) + bs(payload)
ok9 = 0
for e in resp[9][3]:
    prot = bytes.fromhex(e[0]); alg = canonical(prot)[1]
    ok9 += verify_sig('carol', alg, bytes.fromhex(e[2]), sig_sign(prot, b'rhtn/1:verifier', vr_payload))
check(ok9 == 2 and len(resp[9][3]) == 2, 'recovery field 9: hybrid COSE_Sign by the verifier, both entries verify')
succ = enc([rec[1], r_body[1], r_body[2]])
oks = 0
for e in rec[3][3]:
    prot = bytes.fromhex(e[0]); alg = canonical(prot)[1]
    oks += verify_sig('alice', alg, bytes.fromhex(e[2]), sig_sign(prot, b'rhtn/1:successor', succ))
check(oks == 2 and len(rec[3][3]) == 2, 'recovery successor proof: hybrid by the OLD key over [prior, new, patron]')

# ---------------------------------------- EVERY adoption carries exactly one evidence form
# design §6.1.1 / wire §4.1: an adoption rests on field 8 (a presence record),
# field 9 (a former patron's countersignature) or field 6 (a recovery's own
# evidence) -- exactly one.  Swept over every adoption section in the corpus
# rather than checked on one vector, because a rule the fixtures do not all
# obey is a rule the fixtures disprove.
adopt_sections = re.findall(r'\n## ([^\n]*[Aa]doption[^\n]*)\n(.*?)(?=\n## |\Z)', tx, re.S)
ev_ok, ev_seen = 0, 0
for title, sect_txt in adopt_sections:
    m = re.search(r'```\n([0-9a-f\n]+?)```\n\ntxid:', sect_txt)
    if not m: continue
    body = canonical(bytes.fromhex(m.group(1).replace('\n', '')))
    if body is None or 1 not in body or 2 not in body: continue
    ev_seen += 1
    present = [k for k in (6, 8, 9) if k in body]
    if len(present) == 1: ev_ok += 1
    else: print(f'      !! {title.strip()[:60]}: evidence fields {present}')
check(ev_seen >= 4 and ev_ok == ev_seen,
      f'every adoption carries exactly one evidence form: {ev_ok}/{ev_seen} '
      f'(fields 6, 8, 9 — design §6.1.1)')

# ------------------------- a PoP reference names BOTH parties (design §6.1.1, §8.1.1)
# "The check is that the record exists and names these two parties."  Swept,
# because an earlier corpus referenced the alice-carol formation from an
# alice-bob adoption -- a reference no validator applying that check accepts,
# and one nothing in the harness would have caught.
by_txid = {}
for hexs, txid in bodies:
    o = canonical(bytes.fromhex(hexs.replace('\n', '')))
    if o is not None: by_txid[txid] = o
def parties_of(rec):
    return {p[1] for p in rec.get(3, [])} if isinstance(rec.get(3), list) else set()
ref_ok, ref_seen = 0, 0
for title, sect_txt in adopt_sections + re.findall(
        r'\n## ([^\n]*Peering[^\n]*)\n(.*?)(?=\n## |\Z)', tx, re.S):
    m = re.search(r'```\n([0-9a-f\n]+?)```\n\ntxid:', sect_txt)
    if not m: continue
    body = canonical(bytes.fromhex(m.group(1).replace('\n', '')))
    if body is None or 8 not in body or not isinstance(body[8], str): continue
    ref_seen += 1
    rec = by_txid.get(body[8])
    named = parties_of(rec) if rec else set()
    if rec is not None and {body[1], body[2]} <= named: ref_ok += 1
    else: print(f'      !! {title.strip()[:58]}: field 8 -> '
                f'{"unresolvable" if rec is None else "names " + str(sorted(named))}')
check(ref_seen >= 3 and ref_ok == ref_seen,
      f'every field-8 reference resolves to a record naming both parties: '
      f'{ref_ok}/{ref_seen}')

# ------------------------------------------------------- transfer adoption (§4.1 field 9)
xsect = tx[tx.index('## Adoption carrying a TRANSFER'):]
x_body_hex = re.search(r'```\n([0-9a-f\n]+?)```\n\ntxid:', xsect).group(1)
x_body = canonical(bytes.fromhex(x_body_hex.replace('\n', '')))
check(9 in x_body and 8 not in x_body,
      'transfer adoption: field 9 present, field 8 ABSENT — they are alternatives (§4.1)')
xfer = x_body[9]
check(xfer[1] != x_body[2] and xfer[1] != x_body[1],
      'transfer: former_patron differs from both the new patron and the node')
# The statement binds all three parties, and the former patron signed it.
xstmt = enc([x_body[1], xfer[1], x_body[2]])   # [node, former_patron, new_patron]
okx = 0
for e in xfer[2][3]:
    prot = bytes.fromhex(e[0]); alg = canonical(prot)[1]
    okx += verify_sig(BY[xfer[1]], alg, bytes.fromhex(e[2]),
                      sig_sign(prot, b'rhtn/1:transfer', xstmt))
check(okx == 2 and len(xfer[2][3]) == 2,
      'transfer statement: hybrid COSE_Sign by the FORMER patron over [node, former, new]')

# ------------------------------------------- remaining signed contexts (bars 8/10)
CTX = [  # (caption, sig_slot, aad, signer-field or fixed name, wrong-signer name, delegated key or None)
    ('Currency attestation', 7, b'rhtn/1:currency', 6, 'carol', 'bob-instance'),
    ('Catalog entry', 8, b'rhtn/1:catalog', 2, 'carol', None),
    ('Abuse report', 5, b'rhtn/1:abuse', 1, 'carol', None),
    ('Anchor table entry', 5, b'rhtn/1:anchor', 1, 'carol', None),
    ('Subtree acknowledgement', 5, b'rhtn/1:subtree-ack', 2, 'bob', 'carol-instance'),
    ('Prekey bundle', 6, b'rhtn/1:prekey', 1, 'carol', None),
]
csect = rc[rc.index('## The remaining signed contexts'):]
ok_pos = ok_wrong = ok_cross = 0
tags = [c[2] for c in CTX]
def payload_without(obj, slot, extra=()):
    return enc({k: v for k, v in obj.items() if k != slot and k not in extra})
for i, (cap, slot, aad, sfield, wrong, dkey) in enumerate(CTX):
    seg = csect[csect.index('**' + cap + '**'):]
    hexes = re.findall(r'```\n([0-9a-f\n]+?)```', seg)[:2]
    # a currency attestation's field 8 sits outside the signature (§7.1)
    extra = (8,) if cap == 'Currency attestation' else ()
    b = bytes.fromhex(hexes[0].replace('\n', ''))
    obj = canonical(b); assert obj is not None
    prot = bytes.fromhex(obj[slot][0]); sig = bytes.fromhex(obj[slot][3])
    tbs = sig_sign1(prot, aad, payload_without(obj, slot, extra))
    name = BY[obj[sfield]]
    if dkey is None:
        ok_pos += verify_sig(name, -8, sig, tbs)
    else:
        # signed under the node's DELEGATED key (§7.5, §7.1): the identity's own
        # key must NOT verify it, the delegated key must
        ok_pos += (not verify_sig(name, -8, sig, tbs)) and verify_raw(TKPUB[dkey], sig, tbs)
    # cross-context: the same signature under the NEXT context's tag must fail
    payload = payload_without(obj, slot, extra)
    other = tags[(i + 1) % len(tags)]
    ok_cross += not (verify_sig(name, -8, sig, sig_sign1(prot, other, payload))
                     or (dkey and verify_raw(TKPUB[dkey], sig, sig_sign1(prot, other, payload))))
    wb = bytes.fromhex(hexes[1].replace('\n', ''))
    wobj = canonical(wb); wprot = bytes.fromhex(wobj[slot][0]); wsig = bytes.fromhex(wobj[slot][3])
    wtbs = sig_sign1(wprot, aad, payload_without(wobj, slot, extra))
    ok_wrong += (not verify_sig(BY[wobj[sfield]], -8, wsig, wtbs)) and \
                (dkey is None or not verify_raw(TKPUB[dkey], wsig, wtbs)) and \
                verify_sig(wrong, -8, wsig, wtbs)
# the stapled delegation (§7.1 field 8) verifies under the issuer and names the signing key
cur_seg = csect[csect.index('**Currency attestation**'):]
cur_obj = canonical(bytes.fromhex(re.findall(r'```\n([0-9a-f\n]+?)```', cur_seg)[0].replace('\n', '')))
def verify_delegation(d, expect_signer):
    """§8.2: hybrid COSE_Sign over fields 1-4 under rhtn/1:delegation; window exactly 48 h."""
    payload = enc({k: v for k, v in d.items() if k != 5})
    entries = d[5][3]
    ok = len(entries) == 2 and d[4] - d[3] == 172_800
    for ent in entries:
        prot = bytes.fromhex(ent[0]); sig = bytes.fromhex(ent[2])
        alg = canonical(prot)[1]
        tbs = hd(4, 5) + ts('Signature') + bs(b'') + bs(prot) + bs(b'rhtn/1:delegation') + bs(payload)
        ok = ok and verify_sig(expect_signer, alg, sig, tbs)
    return ok
check(verify_delegation(cur_obj[8], 'bob') and cur_obj[8][2] == KH['bob'] and cur_obj[8][1] == TKPUB['bob-instance'].hex(),
      'currency: the stapled delegation is bob\'s, names bob-instance, and verifies hybrid')
cur_id = canonical(bytes.fromhex(re.findall(r'```\n([0-9a-f\n]+?)```', cur_seg)[2].replace('\n', '')))
check(8 not in cur_id and verify_sig('bob', -8, bytes.fromhex(cur_id[7][3]),
                                     sig_sign1(bytes.fromhex(cur_id[7][0]), b'rhtn/1:currency', payload_without(cur_id, 7))),
      'currency: the identity-signed form carries no field 8 and verifies under bob')
# the transport delegation known answer and its wrong-signer analogue
dseg = csect[csect.index('**Transport delegation**'):]
dhex = re.findall(r'```\n([0-9a-f\n]+?)```', dseg)[:2]
dobj = canonical(bytes.fromhex(dhex[0].replace('\n', '')))
dwrong = canonical(bytes.fromhex(dhex[1].replace('\n', '')))
check(verify_delegation(dobj, 'bob') and dobj[2] == KH['bob'] and dobj[1] == TKPUB['bob-instance'].hex(),
      'delegation: hybrid, both entries verify under bob, naming bob-instance, window 172,800 s')
check(not verify_delegation(dwrong, 'bob') and verify_delegation(dwrong, 'carol') and dwrong[2] == KH['bob'],
      'delegation wrong-signer: valid under carol, never under the bob it names')
dpay = enc({k: v for k, v in dobj.items() if k != 5})
dent = dobj[5][3][0]
check(not verify_sig('bob', -8, bytes.fromhex(dent[2]),
                     hd(4, 5) + ts('Signature') + bs(b'') + bs(bytes.fromhex(dent[0])) + bs(b'rhtn/1:currency') + bs(dpay)),
      'delegation cross-context: the classical entry fails under another tag')
check(ok_pos == 6, 'signed contexts: all six known-answer signatures verify under the named signer')
check(ok_wrong == 6, 'wrong-signer analogues: valid under the wrong key, never under the named one')
check(ok_cross == 6, 'cross-context substitution: every signature fails under another context tag')
anch = canonical(bytes.fromhex(re.findall(r'```\n([0-9a-f\n]+?)```',
        csect[csect.index('**Anchor table entry**'):])[0].replace('\n', '')))
check(3 not in anch[2][0], 'anchor NetworkPoint: default port is ABSENT, never written out')
ab = canonical(bytes.fromhex(re.findall(r'```\n([0-9a-f\n]+?)```',
        csect[csect.index('**Abuse report**'):])[0].replace('\n', '')))
check(ab[1] == KH['c5'], 'abuse report: field 1 is both the resource and the signer')

# ---------------------------------------------------------------- messages (bar 9)
ms = read('messages.md')
blocks = re.findall(r'```\n([0-9a-f\n]+?)```', ms)

# The blocks are read by SECTION rather than by absolute index, so adding a
# family to one section does not silently renumber another's fixtures.
def section_blocks(name):
    start = ms.index('## ' + name)
    rest = ms[start + 3:]
    nxt = rest.index('\n## ')
    return re.findall(r'```\n([0-9a-f\n]+?)```', rest[:nxt])

control = section_blocks('Control frames (stream 0)')
requests = section_blocks('Requests (bidirectional streams)')
relayed_blocks = section_blocks('What the node delivers for a relay submission')
replies = section_blocks('Replies')
e2e = section_blocks('End-to-end payloads')
check(len(blocks) == len(control) + len(requests) + len(relayed_blocks) + len(replies) + len(e2e),
      'messages: every fixture block belongs to a named section')

EXPECT_FRAMES = [1, 2, 3, 4, 4, 5, 6, 6, 1, 2, 7, 1, 2, 3, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 12]
framed = control + requests
check(len(framed) == len(EXPECT_FRAMES),
      f'messages: {len(EXPECT_FRAMES)} framed fixtures, one per family and variant')
frames_ok = 0
frame_objs = []
for i, hexs in enumerate(framed):
    b = bytes.fromhex(hexs.replace('\n', ''))
    n = int.from_bytes(b[:4], 'big')
    obj = canonical(b[4:])
    good = (n == len(b) - 4 and isinstance(obj, list) and len(obj) == 2
            and obj[0] == EXPECT_FRAMES[i])
    frames_ok += good
    frame_objs.append(obj)
check(frames_ok == len(EXPECT_FRAMES),
      f'messages: all {len(EXPECT_FRAMES)} frames length-prefixed, canonical, correctly typed')
unframed = replies + e2e
replies_ok = sum(canonical(bytes.fromhex(h.replace('\n', ''))) is not None
                 for h in unframed)
check(replies_ok == len(unframed),
      f'messages: all {len(replies)} replies and {len(e2e)} end-to-end payloads canonical')

# `RelayedPayload` (§7.10): the submitter in front of the ciphertext, an
# array and not a map, and the submitter is a keyhash
rp = canonical(bytes.fromhex(relayed_blocks[0].replace('\n', '')))
check(isinstance(rp, list) and len(rp) == 2 and len(bytes.fromhex(rp[0])) == 32,
      'RelayedPayload: a two-element array whose first member is a keyhash')

# the four submissions carry the shapes §7.10 fixes
pub = frame_objs[EXPECT_FRAMES.index(9)][1]
check(isinstance(pub[1], dict), 'PrekeyPublication: field 1 is the bundle object, not a byte string')
dep = frame_objs[EXPECT_FRAMES.index(10)][1]
check(isinstance(dep[1], list) and 1 <= len(dep[1]) <= 256,
      'OneTimeDeposit: field 1 is one to 256 keys')
wake_on = frame_objs[EXPECT_FRAMES.index(12)][1]
wake_off = frame_objs[len(EXPECT_FRAMES) - 1][1]
check(2 in wake_on and 3 in wake_on, 'WakeRegistration: an endpoint arrives with its key')
check(set(wake_off) == {1}, 'WakeRegistration: a withdrawal is the nonce alone')

late_obj = canonical(bytes.fromhex(e2e[-1].replace('\n', '')))
lr = late_obj[3]
lp = enc({k: lr[k] for k in (1, 2, 3, 4, 5, 6, 7, 10) if k in lr})
check(verify_sig(BY[lr[1]], -8, bytes.fromhex(lr[9][3]),
                 sig_sign1(bytes.fromhex(lr[9][0]), b'rhtn/1:verifier', lp)),
      'LateResponse: the embedded verifier signature verifies')
# positions by type, since control frames were appended after the request families (2026-09-22)
QUERY_AT = len(control) + [i for i, f in enumerate(EXPECT_FRAMES[len(control):]) if f == 4][0]
push_body = frame_objs[EXPECT_FRAMES.index(5)][1]
adopt_env_hex = re.search(r'#### Envelope bytes — final, all four signatures real.*?```\n([0-9a-f\n]+?)```', tx, re.S)
check(push_body[2] == adopt_env_hex.group(1).replace('\n', ''),
      'TopologyPush carries the first adoption envelope byte-for-byte')
srv = canonical(bytes.fromhex(replies[0].replace('\n', '')))
check(H(bytes.fromhex(srv[3][4])).hex() == srv[3][1]
      if isinstance(srv[3][4], str) else H(enc(srv[3][4])).hex() == srv[3][1],
      'ServingInfra: KeyMaterial hashes to the named keyhash')
check(len(re.findall(r'\| TR\d+ ', ms)) == 24, 'messages: twenty-four session traces')
qc = frame_objs[QUERY_AT][1]
check(H(enc({k: qc[0][k] for k in (1, 2, 3, 4, 5, 7)})).hex() == qc[0][6],
      'request-4 query frame: embedded query_id recomputes (fields 1-5 and 7)')
check(len(qc) == 3 and qc[2] in (0, 1, 2, 3),
      'request-4 body carries the selection_basis claim as its third element')

# ---------------------------------------------------------------- HKDF (s7.5.2)
import hmac as _hmac
def _hkdf(ikm, info, length=32):
    prk = _hmac.new(b'\x00' * 32, ikm, hashlib.sha256).digest()
    out, t, i = b'', b'', 1
    while len(out) < length:
        t = _hmac.new(prk, t + info + bytes([i]), hashlib.sha256).digest()
        out += t; i += 1
    return out[:length]
hk = re.search(r'## Capture-key derivation.*?seed:\n\n```\n([0-9a-f\n]+?)```.*?info[^`]*```\n([0-9a-f\n]+?)```.*?k_capture:\n\n```\n([0-9a-f\n]+?)```', rc, re.S)
_seed = bytes.fromhex(hk.group(1).replace('\n', ''))
_info = bytes.fromhex(hk.group(2).replace('\n', ''))
_k = hk.group(3).replace('\n', '')
check(_hkdf(_seed, _info).hex() == _k, 'capture-key HKDF-SHA-256 known answer recomputes')

pp = re.search(r'## Pairwise principal.*?principal_id:\n\n```\n([0-9a-f\n]+?)```', rc, re.S)
check(pp is not None, 'pairwise-principal known answer present')
if pp:
    _pp = pp.group(1).replace('\n', '')
    _want = hashlib.sha256(b'rhtn/1:pairwise' + bytes.fromhex(KH['c1']) + bytes.fromhex(KH['alice'])).hexdigest()
    check(_pp == _want, 'pairwise principal recomputes from c1 and alice keyhashes')
pcm = re.search(r'## Ceremony pre-commitment construction.*?```\n([0-9a-f]{32})\n([0-9a-f]{32})\n```.*?pre-commitment:\n\n```\n([0-9a-f\n]+?)```', rc, re.S)
_c1, _c2 = bytes.fromhex(pcm.group(1)), bytes.fromhex(pcm.group(2))
_pair = (_c1, _c2) if KH['alice'] < KH['bob'] else (_c2, _c1)
check(H(b'rhtn/1:ceremony' + _pair[0] + _pair[1]).hex() == pcm.group(3).replace('\n', ''),
      'contributory pre-commitment known answer recomputes')

# ---------------------------------------------------------------- corpus.json (bar 6)
import json as _json
corpus = _json.load(open(os.path.join(HERE, 'corpus.json'), encoding='utf-8'))
check(corpus['format'] == 'rhtn-test-corpus/1' and len(corpus['entries']) >= 160,
      f"corpus: format tag present, {len(corpus['entries'])} entries")
ids = [e['id'] for e in corpus['entries']]
check(len(ids) == len(set(ids)), 'corpus: fixture ids are unique')
acc_ok = rej_cbor_ok = rej_up_ok = 0
n_acc = n_cbor = n_up = 0
for e in corpus['entries']:
    if e['class'] != 'bytes':
        continue
    b = bytes.fromhex(e['hex'])
    if e['expect'].get('kind') == 'frame':
        if len(b) < 4 or int.from_bytes(b[:4], 'big') != len(b) - 4:
            b = None
        else:
            b = b[4:]
    parsed = canonical(b) if b is not None else None
    if e['expect']['outcome'] == 'accept':
        n_acc += 1; acc_ok += parsed is not None
    elif e['expect'].get('layer') == 'cbor':
        n_cbor += 1; rej_cbor_ok += parsed is None
    else:
        n_up += 1; rej_up_ok += parsed is not None
check(acc_ok == n_acc, f'corpus: all {n_acc} accept-class byte fixtures parse canonically')
check(rej_up_ok == n_up,
      f'corpus: all {n_up} above-encoding rejects parse canonically (the defect is theirs to find)')
check(n_cbor == rej_cbor_ok, f'corpus: all {n_cbor} cbor-layer rejects fail the byte-level scanner')
byid = {e['id']: e for e in corpus['entries']}
_npr_hexes = re.findall(r'```\n([0-9a-f\n]+?)```', tx[tx.index('## Normal presence record'):])
check(byid['P-normal-record']['hex'] ==
      [h for h in _npr_hexes if len(h) > 60000][0].replace('\n', ''),
      'corpus: P-normal-record is byte-identical to the transactions.md envelope')
check(len([e for e in corpus['entries'] if e['class'] == 'trace']) == 24
      and len([e for e in corpus['entries'] if e['class'] == 'context']) == 5,
      'corpus: twenty-four traces and five contexts')
_tr = canonical(bytes.fromhex(byid['P-catalog-reply-truncated']['hex']))
_res_ids = [e[1] for e in _tr[2]]
check(len(_tr[2]) == 111 and 3 in _tr and _res_ids == sorted(_res_ids),
      'truncated CatalogReply: exactly 111 entries, sorted, continuation present')
_kg = canonical(bytes.fromhex(byid['P-e2e-01']['hex']))
check(_kg[3] == _k, 'the KeyGrant carries the derived capture key')
_pc1 = canonical(bytes.fromhex(byid['P-alice-c1-record']['hex']))
check(_kg[1] == H(enc(_pc1[3])).hex(), 'the KeyGrant names the PRIOR alice-c1 record, never the record under assembly')
check(_kg[2] == frame_objs[QUERY_AT][1][0][6], 'the KeyGrant binds the prior capture to the CURRENT query')
tr2 = byid['TR2']['expect']
check(tr2['actions'] == ['never_process_as_early_data']
      and sorted(tr2.get('one_of', [])) == ['defer_until_handshake', 'reject'],
      'corpus: TR2 carries the reject-or-defer disjunction, not one branch')
cr = canonical(bytes.fromhex(byid['P-channel-retry-presentation']['hex']))
cr_env_o, cr_slots_o = cr
cr_ds = []
for i, slot in enumerate(cr_slots_o):
    D_ = enc(slot)
    cr_ds.append(H(b'\x00' + D_))
check(H(b'\x01' + b''.join(cr_ds)).hex() == cr_env_o[3][8],
      'channel-retry presentation recomputes its root')
cr_prox = cr_slots_o[6][2]
check([c[1] for c in cr_prox[1]] == [3, 3] and [c[2] for c in cr_prox[1]] == [1, 0]
      and cr_prox[2] == 3,
      'channel-retry: the repeated kind carries fail-then-pass and strongest is the retried channel')
check(byid['N-wrong-signer-anchor'].get('precondition') == 'the named key is pinned',
      'corpus: the anchor wrong-signer expect carries its pinned-key precondition')
b17 = canonical(bytes.fromhex(byid['B-witnesses-17']['hex']))
check(len(b17[4]) == 17, 'corpus: the 17-witness boundary fixture carries 17 witnesses')
check(len(canonical(bytes.fromhex(byid['B-backptrs-9']['hex']))[0][0]) == 9,
      'corpus: the 9-head boundary fixture carries 9 heads')
_ev = bytes.fromhex(byid['B-ext-value-1024']['hex'])
# encoded-extension-slice bound (Rust runner finding, 2026-09-02):
def _ext_slices(bh):
    p, out = 0, []
    def head(b, p):
        ai = b[p] & 0x1f
        n = ai; adv = 1
        if ai == 24: n = b[p+1]; adv = 2
        elif ai == 25: n = int.from_bytes(b[p+1:p+3], 'big'); adv = 3
        elif ai == 26: n = int.from_bytes(b[p+1:p+5], 'big'); adv = 5
        return b[p] >> 5, n, adv
    def skip(b, p):
        mt, n, adv = head(b, p)
        q = p + adv
        if mt in (0, 1): return q
        if mt in (2, 3): return q + n
        if mt == 4:
            for _ in range(n): q = skip(b, q)
            return q
        if mt == 5:
            for _ in range(n): q = skip(b, q); q = skip(b, q)
            return q
        return q  # simple
    mt, n, adv = head(bh, 0)
    q = adv
    for _ in range(n):
        kmt, kn, kadv = head(bh, q)
        kend = q + kadv
        vend = skip(bh, kend)
        if kmt == 0 and kn > 8:
            out.append(vend - kend)
        q = vend
    return out
check(max(_ext_slices(_ev)) == 1024, 'B-ext-value-1024: encoded slice exactly at the ceiling')
check(max(_ext_slices(bytes.fromhex(byid['B-ext-value-1025']['hex']))) == 1025,
      'B-ext-value-1025: encoded slice one over')

# ---------------------------------------------------------------- normal record (bar 2)
BYNAME = {KH[n]: n for n in NAMES}
nsect = tx[tx.index('## Normal presence record'):tx.index('## Finalization must-accepts')]
n_body = canonical(bytes.fromhex(re.search(r'```\n([0-9a-f\n]+?)```\n\ntxid:', nsect)
                                 .group(1).replace('\n', '')))
n_query = canonical(bytes.fromhex(re.findall(r'```\n([0-9a-f\n]+?)```', nsect)[0]
                                  .replace('\n', '')))
check(H(enc({k: n_query[k] for k in (1, 2, 3, 4, 5, 7)})).hex() == n_query[6],
      'normal record: worked query_id recomputes from fields 1-5 and 7')
check(BY.get(n_query[7]) == 'c1',
      'normal record: worked query field 7 addresses c1, the capture-holding verifier')
check(n_query[2] != n_query[1]
      and n_query[2] in (n_body[3][0][1], n_body[3][1][1])
      and n_query[1] in (n_body[3][0][1], n_body[3][1][1]),
      'normal record: querier and subject are the two participants (ceremony form)')
wit = n_body[4]
check(len(wit) == 16 and wit == sorted(wit, key=lambda w: w[1])
      and len({w[1] for w in wit}) == 16,
      'normal record: sixteen witnesses, ascending, distinct')
pset = {n_body[3][0][1], n_body[3][1][1]}
check(all(w[2] in pset for w in wit), 'normal record: every nominated_by names a participant')
check(any(w[3] & 3 == 3 for w in wit),
      'normal record: witness floor - at least one entry attests protocol_ran and both_responsive (T31)')
resps = n_body[5]
check(resps == sorted(resps, key=lambda r: (r[1], r[2])) and len(resps) == 3,
      'normal record: responses sorted by (verifier, subject) keyhash')
check({r[10] for r in resps} == {0, 2, 3},
      'normal record: selection_basis matrix covered (tier-aligned: met, reachable, discretionary)')
legal = [(0, 0, True), (0, 1, False), (3, None, False)]
seen = sorted(((r[4], r.get(5), 6 in r) for r in resps))
check(seen == sorted(legal), 'normal record: result/basis/template combinations are the legal set')
ok_c = ok_v = 0
for r in resps:
    cp, cs = bytes.fromhex(r[7][0]), bytes.fromhex(r[7][3])
    ok_c += verify_sig(BYNAME[r[2]], -8, cs, sig_sign1(cp, b'rhtn/1:consent', bytes.fromhex(r[3])))
    vp, vs = bytes.fromhex(r[9][0]), bytes.fromhex(r[9][3])
    payload = enc({k: r[k] for k in (1, 2, 3, 4, 5, 6, 7, 10) if k in r})
    ok_v += verify_sig(BYNAME[r[1]], -8, vs, sig_sign1(vp, b'rhtn/1:verifier', payload))
check(ok_c == 3 and ok_v == 3,
      'normal record: every consent and verifier signature verifies (classical form)')
n_env_hex = re.findall(r'```\n([0-9a-f\n]+?)```', nsect)
n_env = canonical(bytes.fromhex([h for h in n_env_hex if len(h) > 60000][0].replace('\n', '')))
check(len(n_env[4][3]) == 36, 'normal record: 36 envelope entries')

fsect = tx[tx.index('## Finalization must-accepts'):tx.index('## Recovery adoption')]
f_bodies = [canonical(bytes.fromhex(h.replace('\n', '')))
            for h, _ in re.findall(r'```\n([0-9a-f\n]+?)```\n\ntxid: `([0-9a-f]{64})`', fsect)]
check(len(f_bodies) == 2 and f_bodies[0][5][0][4] == 1,
      'finalization: the lone no-match record is a positive vector')
check(5 not in f_bodies[1] and f_bodies[1][6] == 0 and 4 in f_bodies[1],
      'finalization: zero responses encode as an ABSENT key 5 on a normal record')

print()
if FAILURES:
    print(f'{len(FAILURES)} FAILURE(S)'); sys.exit(1)
print('ALL CHECKS PASS')
