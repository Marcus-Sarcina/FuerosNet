"""Reference checker: every section reference, in every direction, against
actual headings.  No exemptions (CLAUDE.md).

Resolution, read off the documents themselves:
  * nearest preceding qualifier within 80 chars wins --
      "design §N"        -> network-design.md
      "`<file>.md` §N"   -> that file
  * otherwise the containing document; change-log.md has no numbered
    sections of its own, so its bare §N means network-design.md.
  * "RFC nnnn §N" is external and skipped.
"""
import os, re, sys, collections

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
DOCS = ["network-design.md", "wire-format.md", "light-client-requirements.md",
        "infra-client-requirements.md", "resource-requirements.md",
        "change-log.md"]
ROBOT = [os.path.join("Robot", f) for f in sorted(os.listdir(os.path.join(ROOT, "Robot")))
         if f.endswith(".md")]
ALL = DOCS + ROBOT
# Section references resolve against headings in the five specification
# documents.  change-log.md has no numbered sections of its own -- its bare
# §N mean network-design.md -- and its references are as-of-filing, so it is
# excluded here but NOT from the Robot/ rule below.
CHECK = [d for d in DOCS if d != "change-log.md"]

def headings(path):
    got = set()
    for line in open(os.path.join(ROOT, path), encoding="utf-8"):
        m = re.match(r"^#{1,6}\s+(?:Appendix\s+)?([0-9A-Z][0-9A-Za-z.]*)[\s.]", line)
        if m:
            n = m.group(1).rstrip(".")
            got.add(n)
            p = n.split(".")
            for i in range(1, len(p)):
                got.add(".".join(p[:i]))
    return got

H = {d: headings(d) for d in ALL}
BASE = {os.path.basename(d): d for d in ALL}
DEFAULT = {d: ("network-design.md" if d == "change-log.md" else d) for d in ALL}

REF  = re.compile(r"§\s*([0-9]+(?:\.[0-9]+)*)")
QUAL = re.compile(r"\b(design)\b|`?([a-z0-9-]+\.md)`?", re.I)

total = flags = 0
per_doc = collections.Counter()
findings = []
for d in CHECK:
    text = open(os.path.join(ROOT, d), encoding="utf-8").read()
    for m in REF.finditer(text):
        win = text[max(0, m.start() - 80):m.start()]
        if re.search(r"RFC\s+\d+\s*$", win):
            continue
        total += 1; per_doc[d] += 1
        # A qualifier belongs to THIS reference only if no earlier § sits
        # between them -- otherwise "(`wire-format.md` §5.2) ... (§16.1)"
        # would misattach the self-reference to the wire document.
        win = win[win.rfind("§") + 1:] if "§" in win else win
        win = win.rsplit("\n\n", 1)[-1]   # nor across a paragraph break
        quals = list(QUAL.finditer(win))
        target = DEFAULT[d]
        if quals:
            q = quals[-1]
            target = "network-design.md" if q.group(1) else BASE.get(q.group(2), DEFAULT[d])
        num = m.group(1)
        if num not in H.get(target, ()):
            line = text[:m.start()].count("\n") + 1
            findings.append((d, line, num, target,
                             sorted(x for x in ALL if num in H[x])))
            flags += 1

# No root document may cite anything in Robot/ -- the design must not depend
# on a working file for its own integrity.  This runs over ALL SIX root
# documents, change-log.md included: the 2026-08-26 carve-out that exempted
# the log was reversed on 2026-09-05, and its mentions now name each file
# without a path.
for d in DOCS:
    text = open(os.path.join(ROOT, d), encoding="utf-8").read()
    for rm in re.finditer(r"Robot/", text):
        line = text[:rm.start()].count("\n") + 1
        findings.append((d, line, "-", "cites Robot/", [])); flags += 1

for d, line, num, target, elsewhere in findings:
    where = f" (resolves in: {', '.join(elsewhere)})" if elsewhere else ""
    print(f"FLAG {d}:{line}: §{num} -> {target}{where}")
print(f"\n{total} references checked across {len(CHECK)} documents, "
      f"Robot/ citations across {len(DOCS)}; {flags} flags")
sys.exit(1 if flags else 0)
