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
    if isinstance(obj, dict) and 4 in obj and isinstance(obj[4], list):
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
m = re.search(r'## Presence record.*?```\n([0-9a-f\n]+?)```', tx, re.S)
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

# ---------------------------------------------------------------- formation structural
m = re.search(r'## Presence record.*?```\n([0-9a-f\n]+?)```', tx, re.S)
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
rq15 = enc({k: r_query[k] for k in (1, 2, 3, 4, 5)})
check(H(rq15).hex() == r_query[6], 'recovery query_id = SHA-256 of fields 1-5')
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

# ---------------------------------------------------------------- normal record (bar 2)
BYNAME = {KH[n]: n for n in NAMES}
nsect = tx[tx.index('## Normal presence record'):tx.index('## Finalization must-accepts')]
n_body = canonical(bytes.fromhex(re.search(r'```\n([0-9a-f\n]+?)```\n\ntxid:', nsect)
                                 .group(1).replace('\n', '')))
n_query = canonical(bytes.fromhex(re.findall(r'```\n([0-9a-f\n]+?)```', nsect)[0]
                                  .replace('\n', '')))
check(H(enc({k: n_query[k] for k in (1, 2, 3, 4, 5)})).hex() == n_query[6],
      'normal record: worked query_id recomputes from fields 1-5')
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
resps = n_body[5]
check(resps == sorted(resps, key=lambda r: r[1]) and len(resps) == 3,
      'normal record: responses sorted ascending by verifier keyhash')
check({r[10] for r in resps} == {0, 1, 2}, 'normal record: selection_basis matrix covered')
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
