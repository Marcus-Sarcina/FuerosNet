#!/usr/bin/env python3
"""Check the acceptance catalogue.  Exit 1 on any flag.

  * every entry has every field, a well-formed unique id whose prefix matches
    its area, and valid milestone, kind and oracle values;
  * every citation is qualified and names a heading that exists;
  * every rule quote names a cited section, is at most 40 words, and appears
    verbatim (after markdown normalisation) inside that section's text -- an
    expectation that cannot quote its sentence is an interpretation, and the
    catalogue must say so;
  * every gap row of the plan's section 8.2 has at least one entry;
  * a withdrawn entry is a tombstone: it keeps its number, cites nothing,
    carries its reason, and is neither owed nor implemented.

Numbers are reported, not states: entries per area, per milestone, per kind,
interpretations, implemented against owed.
"""
import collections, sys
from catalogue import *   # noqa

entries = load()
flags = []
SEC = {d: sections(d) for d in DOCS}
ids = collections.Counter(e.get("id") for e in entries)
for e in entries:
    eid = e.get("id", "?")
    for f in FIELDS:
        if f not in e:
            flags.append(f"{eid}: missing field {f!r}")
    m = ID_RE.match(eid)
    if not m:
        flags.append(f"{eid}: malformed id"); continue
    if ids[eid] > 1:
        flags.append(f"{eid}: duplicate id")
    if PREFIX.get(m.group(1)) != e.get("area"):
        flags.append(f"{eid}: prefix {m.group(1)} does not match area {e.get('area')!r}")
    if e.get("gap") != e.get("area"):
        flags.append(f"{eid}: gap {e.get('gap')!r} differs from area {e.get('area')!r}")
    if e.get("milestone") not in MILESTONES:
        flags.append(f"{eid}: milestone {e.get('milestone')!r}")
    if e.get("kind") not in KINDS:
        flags.append(f"{eid}: kind {e.get('kind')!r}")
    if e.get("oracle") not in ORACLES:
        flags.append(f"{eid}: oracle {e.get('oracle')!r}")
    if e.get("kind") == "withdrawn":
        # a tombstone: the number is never reused, a citation resolves to
        # withdrawn, and the interpretation says why and what replaced it
        if not str(e.get("interpretation") or "").strip():
            flags.append(f"{eid}: withdrawn without a reason")
        if e.get("id") in implemented_ids():
            flags.append(f"{eid}: withdrawn but marked implemented")
        continue
    cited = set()
    for c in e.get("spec", []):
        t = cite_target(c)
        if not t:
            flags.append(f"{eid}: unqualified or malformed citation {c!r}"); continue
        if t[1] not in SEC[t[0]]:
            flags.append(f"{eid}: {c!r} names no heading")
        cited.add(c)
    if not cited:
        flags.append(f"{eid}: no citations")
    rules = e.get("rule", [])
    if not rules:
        flags.append(f"{eid}: no rule quote")
    for r in rules:
        w, text = r.get("where", ""), r.get("text", "")
        t = cite_target(w)
        if not t or t[1] not in SEC[t[0]]:
            flags.append(f"{eid}: rule cites {w!r}, which names no heading"); continue
        if w not in cited:
            flags.append(f"{eid}: rule cites {w!r}, absent from spec list")
        if len(text.split()) > 40:
            flags.append(f"{eid}: rule quote is {len(text.split())} words")
        if normalise(text) not in normalise(SEC[t[0]][t[1]][3]):
            flags.append(f"{eid}: rule quote not found verbatim in {w}: {text[:70]!r}")
    for f in ("given", "when", "then", "title"):
        if not str(e.get(f, "")).strip():
            flags.append(f"{eid}: empty {f}")

by_gap = collections.Counter(e.get("gap") for e in entries)
for g in GAPS:
    if by_gap[g] == 0:
        flags.append(f"gap {g!r}: no entries")

impl = implemented_ids()
for i in impl:
    if i not in ids:
        flags.append(f"marker {i} in {impl[i]} names no catalogue entry")

def table(counter, title):
    print(f"{title}:")
    for k, v in sorted(counter.items(), key=lambda kv: str(kv[0])):
        print(f"  {str(k):20} {v:4}")

print(f"entries: {len(entries)}")
table(by_gap, "per gap")
table(collections.Counter(str(e.get("milestone")) for e in entries), "per milestone")
table(collections.Counter(e.get("kind") for e in entries), "per kind")
print(f"interpretations: {sum(1 for e in entries if e.get('interpretation'))}")
withdrawn = sum(1 for e in entries if e.get("kind") == "withdrawn")
print(f"withdrawn: {withdrawn}")
print(f"implemented: {sum(1 for e in entries if e.get('id') in impl)} of {len(entries) - withdrawn}")
print(f"flags: {len(flags)}")
for f in flags:
    print("  " + f)
sys.exit(1 if flags else 0)
