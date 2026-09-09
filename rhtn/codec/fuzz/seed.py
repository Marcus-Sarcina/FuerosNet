#!/usr/bin/env python3
"""Seed each fuzz target's corpus from the accepted test-vector fixtures, so
libFuzzer starts from valid shapes.  Writes fuzz/corpus/<target>/; run from
anywhere.  The corpus directories are generated and not committed."""
import json, os
here = os.path.dirname(os.path.abspath(__file__))
root = os.path.abspath(os.path.join(here, "..", "..", ".."))
corpus = json.load(open(os.path.join(root, "test-vectors", "corpus.json")))
targets = {t: {} for t in ["parse_all", "body", "envelope", "frame_control", "frame_request"]}

def head(b, p):
    ib = b[p]; mt, ai = ib >> 5, ib & 31
    if ai < 24: return mt, ai, 1
    if ai == 24: return mt, b[p + 1], 2
    if ai == 25: return mt, int.from_bytes(b[p + 1:p + 3], "big"), 3
    if ai == 26: return mt, int.from_bytes(b[p + 1:p + 5], "big"), 5
    return mt, int.from_bytes(b[p + 1:p + 9], "big"), 9

def skip(b, p):
    ib = b[p]
    if ib in (0xf4, 0xf5, 0xf6): return p + 1
    mt, n, adv = head(b, p); q = p + adv
    if mt in (0, 1): return q
    if mt in (2, 3): return q + n
    if mt == 4:
        for _ in range(n): q = skip(b, q)
        return q
    for _ in range(n): q = skip(b, skip(b, q))
    return q

def body_of(env):
    _, n, adv = head(env, 0); q = adv
    for _ in range(n):
        ks = q; q = skip(env, q); key = env[ks]
        vs = q; q = skip(env, q)
        if key == 3: return env[vs:q]
    return None

for e in corpus["entries"]:
    if e["class"] != "bytes" or e["expect"].get("outcome") != "accept":
        continue
    b = bytes.fromhex(e["hex"]); kind = e["expect"].get("kind", ""); i = e["id"]
    if kind == "frame":
        targets["frame_control"][i] = b; targets["frame_request"][i] = b; targets["parse_all"][i] = b[4:]
    else:
        targets["parse_all"][i] = b
        if kind == "envelope":
            targets["envelope"][i] = b
            bd = body_of(b)
            if bd: targets["body"][i] = bd
        elif kind == "body":
            targets["body"][i] = b
n = 0
for t, files in targets.items():
    d = os.path.join(here, "corpus", t); os.makedirs(d, exist_ok=True)
    for i, b in files.items():
        open(os.path.join(d, i), "wb").write(b); n += 1
print(f"seeded {n} inputs across {len(targets)} targets")
