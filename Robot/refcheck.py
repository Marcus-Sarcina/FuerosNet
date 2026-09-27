"""Reference checker: every section reference, in every direction, against
actual headings.  No exemptions (CLAUDE.md).

A reference is a `§` or `§§` marker and the whole list of numbers it
introduces -- `§§1, 1.2, 3.6` is three references, not one, and every member
is checked, as is a list that repeats the marker after a comma (`D §12.6.3,
§10.1`).  A range is checked at both endpoints and not through its interior:
`§7–7.3` changes depth partway, so the interior is not a set this can
enumerate.

Resolution, read off the documents themselves:
  * the nearest preceding qualifier wins, searched back from the marker to
      the end of the previous marker (so a `§§` list cannot eat its own
      qualifier), within 80 characters and not across a paragraph break --
      "design §N"        -> network-design.md
      "`<file>.md` §N"   -> that file
  * in functional_tests.md only, its own §1 abbreviation table is also a
      qualifier -- "D §N", "W §N", "L §N", "I §N", "R §N" -- and the window
      additionally stops at the start of the line, because every reference
      there lives in a table cell and no cell spans a line.
  * a §N inside a verbatim quotation is the QUOTED document's reference,
      reproduced: it cannot be qualified without falsifying the quote, so it
      resolves against the document the quotation is attributed to.
  * otherwise the containing document.  change-log.md has no numbered
      sections of its own, so its bare §N mean network-design.md.
      functional_tests.md DOES have its own §1-§10, and a bare §N there
      means them: its §1 states the rule ("References such as `W §4.1` point
      to numbered sections in these files"), so a reference to a design
      document always carries its letter and an unqualified one is a
      self-reference.  That convention can pass a wrong reference silently
      wherever a design section number coincides with §1-§10 or §3.1-§3.7,
      which is why the comma-repeated marker above is joined rather than left
      to fall through, and why --self lists what it resolved that way.
  * a §N inside a code span is a LITERAL, not a reference.  That is how this
      corpus shows notation as text: a malformed marker quoted as an example
      (`[D (§6.2]`), a retired alphanumeric identifier (`§4a`), the citation
      format itself (`design §12.6.5`).  The count is printed rather than
      passed over in silence.
  * "RFC nnnn §N" is external and skipped.

The working files are checked too, and there a bare §N is a FLAG rather than
a default: no default fits.  Their bare references were surveyed and each
file disagreed with the next -- transaction-rules.md's meant wire-format,
app-requirements-notes.md's meant itself, outstanding-work's meant both --
so a per-file default would have passed the minority wrongly wherever a
number happened to exist in the wrong document.  A working file writes
`design §N` or "`<file>.md` §N", and refers to its OWN sections as "section
N", which is what most of them already did.  Two exceptions come from the
documents themselves and are counted, not silent:
  * a file declaring "Section references {here,in this file} are as-of-filing"
      is not section-checked at all: its references point at numbering that
      has since moved, by declaration, and cannot be checked against current
      headings.
  * a file declaring "**Status:** Frozen checkpoint" carries the numbering of
      the document it froze, so its bare §N are self-references.

`--self` prints every reference that resolved to its own document, and
`--counts` the per-document totals.  Neither changes the verdict.
"""
import os, re, sys, collections

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
DOCS = ["network-design.md", "wire-format.md", "light-client-requirements.md",
        "infra-client-requirements.md", "resource-requirements.md",
        "functional_tests.md", "change-log.md"]
ROBOT = [os.path.join("Robot", f) for f in sorted(os.listdir(os.path.join(ROOT, "Robot")))
         if f.endswith(".md")]
ALL = DOCS + ROBOT


def declares(path, pattern):
    text = open(os.path.join(ROOT, path), encoding="utf-8").read()
    return bool(re.search(pattern, text))


# Read off the documents rather than listed here, so that a file becoming
# frozen or ceasing to be needs no edit to this checker.
AS_OF = [r for r in ROBOT
         if declares(r, r"Section references (?:here|in this file) are as-of-filing")]
FROZEN = [r for r in ROBOT if declares(r, r"\*\*Status:\*\* Frozen checkpoint")]
# change-log.md's references are as-of-filing, so its section references are
# excluded here but NOT from the Robot/ rule below.
CHECK = [d for d in DOCS if d != "change-log.md"] + [r for r in ROBOT if r not in AS_OF]
# functional_tests.md's §1 abbreviation table, which is a qualifier there and
# nowhere else: a standalone "I" or "D" in the design documents is prose.
ABBREV = {"D": "network-design.md", "W": "wire-format.md",
          "L": "light-client-requirements.md", "I": "infra-client-requirements.md",
          "R": "resource-requirements.md"}
ABBREV_IN = "functional_tests.md"


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
# A working file has no default: a bare §N there is a flag, because no default
# fits all of them (see the module docstring).  A frozen checkpoint is the
# exception it declares itself to be.
UNQUALIFIED = "unqualified -- name the document"
for r in ROBOT:
    DEFAULT[r] = r if r in FROZEN else UNQUALIFIED

NUM = r"[0-9]+(?:\.[0-9]+)*"
# One marker and the list it introduces: members separated by commas, ranges
# by an en dash.  A comma may repeat the marker -- "D §12.6.3, §10.1" is one
# qualified list, not a qualified reference beside a bare one -- and joining
# it is not cosmetic: "L §4, §1.3" would otherwise leave §4 attributed to L
# while §1.3 resolved against THIS document's own §1.3, which is the kind of
# quiet pass that makes a checker worse than none.
MARKER = re.compile(rf"§{{1,2}}\s*{NUM}(?:\s*(?:,|–)\s*§{{0,2}}\s*{NUM})*")
QUAL = re.compile(r"\b(design)\b|`?([a-z0-9_-]+\.md)`?", re.I)
LETTER = re.compile(r"(?<![0-9A-Za-z])([DWLIR])(?![0-9A-Za-z])")
# A verbatim quotation of another document reproduces that document's own
# self-reference.  A §N inside one is therefore not this document's reference
# and cannot be qualified without falsifying the quote, so it resolves against
# the document the quotation is attributed to -- which is the document the
# sentence introducing the quote was already talking about, i.e. what the
# preceding reference resolved to.  Inheritance is confined to quotations for
# a reason: as a general rule it would let "D §6.4 and W §4.5 agree ... §6.4's
# threshold language" resolve §6.4 against W, where it also exists, and pass.
QUOTE = re.compile(r'\*"[^"]*"\*')
# A code span shows notation as text, not a reference to follow.
CODE = re.compile(r"`[^`\n]*`")

total = flags = literal = 0
per_doc = collections.Counter()
findings = []
selfref = []
for d in CHECK:
    text = open(os.path.join(ROOT, d), encoding="utf-8").read()
    quotes = [(q.start(), q.end()) for q in QUOTE.finditer(text)]
    spans = [(c.start(), c.end()) for c in CODE.finditer(text)]
    marks = [m for m in MARKER.finditer(text)
             if not any(a < m.start() < b for a, b in spans)]
    literal += len(list(MARKER.finditer(text))) - len(marks)
    ends = [m.end() for m in marks]
    targets = []
    for n, mk in enumerate(marks):
        quoted = next((a for a, b in quotes if a < mk.start() < b), None)
        anchor = mk.start() if quoted is None else quoted
        prev_end = max((e for e in ends[:n] if e <= anchor), default=0)
        win = text[max(prev_end, anchor - 80):anchor]
        if re.search(r"RFC\s+\d+\s*$", win):
            targets.append(None)
            continue
        win = win.rsplit("\n\n", 1)[-1]          # never across a paragraph
        if d == ABBREV_IN:
            win = win.rsplit("\n", 1)[-1]        # nor across a table row
        target = DEFAULT[d]
        quals = [(q.start(), "network-design.md" if q.group(1)
                  else BASE.get(q.group(2))) for q in QUAL.finditer(win)]
        if d == ABBREV_IN:
            quals += [(q.start(), ABBREV[q.group(1).upper()])
                      for q in LETTER.finditer(win)]
        quals = [(at, t) for at, t in quals if t]
        if quals:
            target = max(quals)[1]               # the nearest one wins
        elif quoted is not None and n and "\n" not in win:
            target = targets[n - 1] or target    # the quote's attribution
        targets.append(target)
        line = text[:mk.start()].count("\n") + 1
        if target == d == ABBREV_IN:
            selfref.append((d, line, mk.group(0)))
        for num in re.findall(NUM, mk.group(0)):
            total += 1
            per_doc[d] += 1
            if num not in H.get(target, ()):
                findings.append((d, line, num, target,
                                 sorted(x for x in ALL if num in H[x])))
                flags += 1

# No root document may cite anything in Robot/ -- the design must not depend
# on a working file for its own integrity.  This runs over ALL SEVEN root
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
if "--self" in sys.argv:
    # Every bare §N that functional_tests.md resolved against itself.  A
    # self-reference is the ordinary case in the design documents and needs no
    # audit; there it is a convention, and a wrong one passes silently where
    # the number happens to be one of its own, so the set is listed rather
    # than trusted.  Read each and check it means this document.
    print(f"\n{ABBREV_IN} references resolved to itself: {len(selfref)}")
    for d, line, mk in selfref:
        print(f"  {d}:{line}: {mk}")
if "--counts" in sys.argv:
    print()
    for d in CHECK:
        print(f"  {per_doc[d]:>5} {d}")
print(f"\n{total} references checked across {len(CHECK)} documents "
      f"({len(CHECK) - len(DOCS) + 1} of them working files), "
      f"Robot/ citations across {len(DOCS)}; {flags} flags")
print(f"  not references: {literal} inside code spans")
print(f"  not section-checked, by their own declaration: "
      f"{', '.join(os.path.basename(a) for a in AS_OF)}")
sys.exit(1 if flags else 0)
