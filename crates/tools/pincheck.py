#!/usr/bin/env python3
"""The freshness gate: the specifications the code last passed against, and
the specifications, tools and outputs the test vectors were generated from,
compared with what the tree holds now.  Non-mutating: it reports and fails,
and never rewrites a pin.

Why it exists.  On 2026-09-21 every checker in the tree reported clean while
the vector pin was stale on all three of its documents and the codec agreed
with a corpus that encoded a superseded wire section.  The gate checked
quotes, stubs, lint, tests and fuzzing and compared no specification hash, so
a specification-only change was invisible to it (Robot/outstanding-work-
2026-09-21.md, section 6).  `implementation-plan.md` section 3 had promised a
code pin in the shape of the vector pin; this is it.

Two pins, two acknowledgements.
  * The CODE pin, `crates/spec-pins.json`: the SHA-256 of every root document
    the code answers to.  `--accept` rewrites it, which asserts that the gate
    passed against the documents as they now stand.
  * The VECTOR pin, `test-vectors/tools/spec-pins.json`, is written only by
    `generate.py` under its own flags; this script only reads it, and reports
    a stale specification, tool or output.

Exit 0 only when every hash matches.  Report the numbers, not a state.
"""
import hashlib, json, os, sys, datetime

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.abspath(os.path.join(HERE, '..', '..'))
CODE_PIN = os.path.join(HERE, '..', 'spec-pins.json')
VECTOR_PIN = os.path.join(ROOT, 'test-vectors', 'tools', 'spec-pins.json')
DOCS = ['network-design.md', 'wire-format.md', 'light-client-requirements.md',
        'infra-client-requirements.md', 'resource-requirements.md', 'functional_tests.md']

def sha(path):
    with open(path, 'rb') as f:
        return hashlib.sha256(f.read()).hexdigest()

stale, checked = [], 0
current = {d: sha(os.path.join(ROOT, d)) for d in DOCS}

# ---- the code pin
if '--accept' in sys.argv:
    json.dump({'specs': current, 'accepted': datetime.date.today().isoformat(),
               'note': 'the documents crates/check.sh last passed against; rewritten only by tools/pincheck.py --accept'},
              open(CODE_PIN, 'w', encoding='utf-8', newline='\n'), indent=1)
    print(f'  code pin written for {len(DOCS)} documents')
if not os.path.exists(CODE_PIN):
    stale.append('crates/spec-pins.json is missing: run tools/pincheck.py --accept after the gate passes')
else:
    stored = json.load(open(CODE_PIN, encoding='utf-8'))['specs']
    for d in DOCS:
        checked += 1
        if stored.get(d) != current[d]:
            stale.append(f'{d} changed since the code pin ({(stored.get(d) or "absent")[:12]} -> {current[d][:12]})')

# ---- the vector pin, read only
if not os.path.exists(VECTOR_PIN):
    stale.append('test-vectors/tools/spec-pins.json is missing')
else:
    vp = json.load(open(VECTOR_PIN, encoding='utf-8'))
    for name, h in vp.get('specs', {}).items():
        checked += 1
        if sha(os.path.join(ROOT, name)) != h:
            stale.append(f'vectors: {name} changed since generation; regenerate with --accept-spec-change after the audit')
    tools = {'generator': 'generate.py', 'verifier': 'verify.py'}
    for key, fname in tools.items():
        checked += 1
        if vp.get(key) and sha(os.path.join(ROOT, 'test-vectors', 'tools', fname)) != vp[key]:
            stale.append(f'vectors: tools/{fname} changed since generation; regenerate with --accept-generator-change')
    for name, h in vp.get('outputs', {}).items():
        checked += 1
        p = os.path.join(ROOT, 'test-vectors', name)
        if not os.path.exists(p) or sha(p) != h:
            stale.append(f'vectors: {name} differs from the pinned output; regenerate')

for s in stale:
    print('  STALE: ' + s)
print(f'  {checked} pins checked ({len(DOCS)} documents against the code pin; the vector pin\'s specifications, tools and outputs); {len(stale)} stale')
sys.exit(1 if stale else 0)
