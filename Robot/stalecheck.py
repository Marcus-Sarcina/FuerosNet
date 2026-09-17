#!/usr/bin/env python3
"""Staleness sweep: the classes a selective reader cannot scan for.

Three checks, each reporting rather than gating, because each is a heuristic:

  A. STATE CLAIMS.  Sentences asserting the state of the world -- "no
     implementation yet", "none remain", "not yet built".  These are true when
     written and decay silently.  Reported for a human to confirm.

  B. MISDIRECTED CITATIONS.  `refcheck` proves a reference RESOLVES; it cannot
     prove it resolves to the right place.  For a table row of the form
     `| Some Object (§N) | ...`, the heading at §N should share a word with the
     object it claims to define.  "Catalog entry (§4)" where §4 is "Transaction
     types" shares nothing, and that was a real defect refcheck passed.

  C. COUNTS BESIDE TABLES.  A spelled-out number within a few lines of a
     markdown table, printed beside that table's row count so the two can be
     compared.  "Thirteen roles carry a tag" above a fourteen-row table.

Report the numbers, not a verdict (`Robot/authoring-conventions.md`).  **The
baseline on clean documents is 1 + 10 + 3 = 14**, all benign and each a shape the
checks cannot separate from a real defect without going blind to the real one:

  * A's one is 21.1's *"Wire-format work: none remain"*, kept deliberately, with
    the sentence below it saying why.
  * B's ten are citations to a PARENT: a verifier response is defined inside the
    presence record, a successor statement inside the adoption.  Testing the
    cited section's body instead of its heading removes all ten and removes the
    real catches too -- §4's body does mention catalog entries, which is exactly
    why citing §4 looked right.
  * C's three are number words in prose that happen to sit beside a table.

**A change in the count is the signal.**  Do not add a suppression list: the
reference checker's exemptions were permanent blind spots bought to silence
temporary false positives, and this would go the same way.

Validated against three defects real reviewers found and `refcheck` passed:
"Catalog entry (§4)" and "Abuse report (§4)", whose schemas are §6.1 and §6.3;
and "Thirteen roles carry a tag" above a fourteen-row table.  It missed all three
on its first two builds -- once because the body test silenced them, once because
`## 4. Transaction types` carries a period that the heading regex did not accept,
so no top-level section parsed at all and the check reported clean while blind to
half the document.  Hence the PARSE line: a checker that cannot see what it claims
to check reports clean for the wrong reason.
"""
import os, re, sys, collections

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
DOCS = ["network-design.md", "wire-format.md", "light-client-requirements.md",
        "infra-client-requirements.md", "resource-requirements.md"]
if "--with-tests" in sys.argv: DOCS.append("functional_tests.md")

STATE = re.compile(r"""(?ix)
    \b(?: no\ implementation
        | not\ yet\ (?:built|implemented|written|specified|decided)
        | none\ (?:remain|exist|are\ built)
        | nothing\ (?:is\ )?(?:implemented|built)
        | unbuilt | unimplemented
        | is\ not\ implemented
        | remains?\ unwritten
        | (?:currently|at\ present|so\ far|to\ date)\ (?:no|none|nothing|unset)
    )\b""")
NUMWORD = re.compile(r"(?i)\b(two|three|four|five|six|seven|eight|nine|ten|eleven|twelve|"
                     r"thirteen|fourteen|fifteen|sixteen|seventeen|eighteen|nineteen|twenty)\b")
WORDNUM = {w: i for i, w in enumerate(
    "zero one two three four five six seven eight nine ten eleven twelve thirteen fourteen "
    "fifteen sixteen seventeen eighteen nineteen twenty".split())}
ROWCITE = re.compile(r"^\|\s*\*{0,2}([A-Za-z][A-Za-z '`\-]{2,44}?)\*{0,2}\s*\(§([0-9]+(?:\.[0-9]+)*)\)")
STOP = {"the","a","an","of","and","or","to","for","in","on","at","its","it","this","that",
        "entry","record","statement","response","report","is","are","by","with","own"}

def headings(path):
    out = {}
    for l in open(path, encoding="utf-8"):
        m = re.match(r"^#{2,6}\s+(?:Appendix\s+)?([0-9]+(?:\.[0-9]+)*|[A-Z](?:\.[0-9]+)*)\.?\s+(.*)", l)
        if m: out[m.group(1)] = m.group(2).strip()
    return out

def bodies(path):
    """section number -> its own text, up to the next heading of any depth."""
    out, cur, buf = {}, None, []
    for l in open(path, encoding="utf-8"):
        m = re.match(r"^#{2,6}\s+(?:Appendix\s+)?([0-9]+(?:\.[0-9]+)*|[A-Z](?:\.[0-9]+)*)\.?\s+(.*)", l)
        if m:
            if cur: out[cur] = " ".join(buf)
            cur, buf = m.group(1), [m.group(2)]
        elif cur: buf.append(l)
    if cur: out[cur] = " ".join(buf)
    return out

def tables(lines):
    """(header_line_index, row_count) for each markdown table."""
    out, i = [], 0
    while i < len(lines):
        if lines[i].startswith("|") and i + 1 < len(lines) and re.match(r"^\|[\s:|-]+\|\s*$", lines[i+1]):
            j = i + 2
            while j < len(lines) and lines[j].startswith("|"): j += 1
            out.append((i, j - i - 2)); i = j
        else: i += 1
    return out

state_hits, cite_hits, count_hits = [], [], []
for d in DOCS:
    p = os.path.join(ROOT, d)
    if not os.path.exists(p): continue
    text = open(p, encoding="utf-8").read()
    lines, H, B, fence = text.split("\n"), headings(p), bodies(p), False
    for n, l in enumerate(lines, 1):
        if l.startswith("```"): fence = not fence; continue
        if fence: continue
        if STATE.search(l) and not re.search(r"(?i)\b(should|must|when|if|unless|until|while)\b", l):
            state_hits.append((d, n, l.strip()[:96]))
        m = ROWCITE.match(l)
        if m:
            name, sec = m.group(1).strip(), m.group(2)
            head = H.get(sec, "")
            nt = {w for w in re.findall(r"[a-z]+", name.lower()) if w not in STOP and len(w) > 2}
            ht = {w for w in re.findall(r"[a-z]+", head.lower()) if w not in STOP and len(w) > 2}
            if nt and ht and not (nt & ht):
                cite_hits.append((d, n, name, sec, head))
    for hdr, rows in tables(lines):
        span = list(range(max(0, hdr - 6), hdr)) + list(range(hdr + 2 + rows, min(len(lines), hdr + 8 + rows)))
        for k in span:
            if lines[k].startswith("|") or lines[k].startswith("```"): continue
            for w in re.findall(NUMWORD.pattern + r"\s+(?:\w+\s+){0,1}(?:rows|entries|roles|types|kinds|classes|items|cases|columns|parameters|domains|tags|fields|categories|values)\b", lines[k], re.I):
                if WORDNUM[w.lower()] != rows:
                    count_hits.append((d, k + 1, w.lower(), WORDNUM[w.lower()], rows, lines[k].strip()[:60]))

print("PARSE  heading lines vs headings parsed, per document:")
for d in DOCS:
    q = os.path.join(ROOT, d)
    if not os.path.exists(q): continue
    raw = sum(1 for l in open(q, encoding="utf-8")
              if re.match(r"^#{2,6}\s+(?:Appendix\s+)?(?:[0-9]+[.\s]|[A-Z]\.)", l))
    got = len(headings(q))
    print(f"     {d}: {got} of {raw}" + ("" if got == raw else "   <-- A NUMBERED HEADING IS UNPARSED; THE CHECK IS BLIND TO IT"))
print()
print(f"A. state claims: {len(state_hits)}")
for d, n, l in state_hits: print(f"     {d}:{n}: {l}")
print(f"B. citations resolving to an unrelated heading: {len(cite_hits)}")
for d, n, name, sec, head in cite_hits:
    print(f"     {d}:{n}: '{name}' cites §{sec} = '{head}'")
print(f"C. spelled counts within 6 lines above a table, differing from its row count: {len(count_hits)}")
for d, n, w, v, rows, l in count_hits:
    print(f"     {d}:{n}: '{w}' ({v}) vs {rows} rows — {l}")
print(f"\n{len(DOCS)} documents; {len(state_hits)} + {len(cite_hits)} + {len(count_hits)} reports")
