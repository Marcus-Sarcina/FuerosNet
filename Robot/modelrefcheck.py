#!/usr/bin/env python3
"""Section citations in models/ against actual headings in the specification.

refcheck.py covers the root documents and Robot/ citations; nothing covered
models/, and the models cite the specification constantly -- 176 times at the
time of writing.  The first run found `design 15.4`, a section that does not
exist, carried in two comment blocks and repeated in models/README.md.

No exemptions, by the same rule refcheck.py follows.
"""
import os, re

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
TARGETS = {"design": "network-design.md", "wire": "wire-format.md",
           "light": "light-client-requirements.md",
           "infra": "infra-client-requirements.md",
           "resource": "resource-requirements.md"}

def headings(path):
    got = set()
    for line in open(os.path.join(ROOT, path), encoding="utf-8"):
        m = re.match(r"^#{1,6}\s+(?:Appendix\s+)?([0-9A-Z][0-9A-Za-z.]*)[\s.]", line)
        if m:
            got.add(m.group(1).rstrip("."))
    return got

H = {k: headings(v) for k, v in TARGETS.items()}

# How a citation names its target.  Longest first so `wire-format.md` is not
# matched as bare `wire`.
QUAL = [(r"network-design\.md", "design"), (r"design", "design"),
        (r"wire-format\.md", "wire"), (r"wire", "wire"),
        (r"light-client-requirements\.md", "light"),
        (r"infra-client-requirements\.md", "infra"),
        (r"resource-requirements\.md", "resource")]
PAT = re.compile(r"(network-design\.md|design|wire-format\.md|wire|"
                 r"light-client-requirements\.md|infra-client-requirements\.md|"
                 r"resource-requirements\.md)"
                 r"[`'\s]*(?:Section|§)\s*([0-9]+(?:\.[0-9]+)*)")

files = []
for d, _, fs in os.walk(os.path.join(ROOT, "models")):
    if "results" in d:
        continue                      # generated prover output, not authored
    for f in fs:
        if f.endswith((".spthy", ".tla", ".py", ".md", ".cfg", ".bounded")):
            files.append(os.path.join(d, f))

total, flags = 0, []
for path in sorted(files):
    txt = open(path, encoding="utf-8", errors="replace").read()
    for m in PAT.finditer(txt):
        qual, sec = m.group(1), m.group(2)
        tgt = next(t for q, t in QUAL if re.fullmatch(q, qual))
        total += 1
        if sec not in H[tgt]:
            flags.append((os.path.relpath(path, ROOT), qual, sec, TARGETS[tgt]))

print(f"{total} model citations checked across {len(files)} files; {len(flags)} flags")
for path, qual, sec, doc in flags:
    print(f"  FLAG {path}: '{qual} §{sec}' has no such heading in {doc}")
raise SystemExit(1 if flags else 0)
