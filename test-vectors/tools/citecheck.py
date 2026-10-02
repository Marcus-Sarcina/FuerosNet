#!/usr/bin/env python3
"""Check the negative-vector id citations, and the fixture-name ones.

`negative-vectors.md` numbers its rows T1.., S1.., R1.., D1.., and the
other vector documents, the generator and the harness cite those numbers
in prose and in pinned reason strings.  A renumbering that misses a
citation leaves it resolving to a *different rule* rather than to
nothing, which no reference checker catches and which the corpus then
pins.  The documents also name corpus fixtures in backticks
(`P-catalog`, `N-serving-infra-km-mismatch`), and a fixture renamed or
renumbered leaves the same kind of phantom.

Three checks, and only the first two are mechanical enough to gate:

  EXISTENCE -- every cited id names a row.  Exact, and a flag is a bug.
  FIXTURES  -- every backticked fixture name in a vector document is an
               id in corpus.json.  Exact, and a flag is a bug.
  OVERLAP   -- the citing sentence and the row share a distinctive word.
               A heuristic sweep, reported with `--overlap`, for a human
               to read.  Not gated: its false positives are the kind of
               noise that buys exemptions (`CLAUDE.md`).

The first version of this checker stripped a whole table row before
scanning it, meaning to skip the row's own id column, and so never read a
citation inside `negative-vectors.md` at all -- which is where three stale
ones lived [2026-10-01].  Now only the id column is skipped.
"""
import json
import re
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent.parent
TABLE = HERE / 'negative-vectors.md'
CORPUS = HERE / 'corpus.json'
# Where citations live.  `negative-vectors.md` itself is included: a row
# may cite another row.
SITES = ['README.md', 'records.md', 'transactions.md', 'messages.md',
         'primitives.md', 'keys.md', 'verifier-selection.md',
         'local-interfaces.md', 'negative-vectors.md',
         'tools/generate.py', 'tools/verify.py']
# Fixture names are checked in the documents alone: the generator and the
# harness define them, so a name there is a definition, not a citation.
FIXTURE_SITES = [s for s in SITES if not s.startswith('tools/')]

ROW = re.compile(r'^\| ([TSRD]\d+) \| (.*?) \| (.*?) \|\s*$')
# a table's own id column, which is the definition and not a citation
ID_COLUMN = re.compile(r'^\| [TSRD]\d+ \|')
# A citation is the id in parentheses, after a comma in one, or bare in a
# table's own id column.  Word boundaries both sides: `D1` must not match
# inside `D14`, and `T5` must not match `T52`.
CITE = re.compile(r'(?<![A-Za-z0-9])([TSRD]\d+)(?![0-9A-Za-z])')
# A fixture name: the corpus's P-/N-/B- prefix, in backticks.
FIXTURE = re.compile(r'`([PNB]-[A-Za-z0-9][A-Za-z0-9-]*)`')

STOP = set('''a an and are as at be because been before being between both but by
can cannot carries carry does for from has have how in into is it its more
never no nor not of on once one only or other over own rather same says set
so than that the their them then there these they this those to under until
up was what when where which while who whose with without would'''.split())


def rows():
    out = {}
    for line in TABLE.read_text().splitlines():
        m = ROW.match(line)
        if m:
            out[m.group(1)] = (m.group(2) + ' ' + m.group(3))
    return out


def words(s):
    return {w for w in re.findall(r'[a-z_]{4,}', s.lower()) if w not in STOP}


def distinctive(table):
    """Words that name few rows.  `record` and `field` appear in dozens and
    carry no information about *which* row is meant; `protocol_ran` names
    two.  Overlap on a common word is how a stale citation reads as
    resolving -- the T31/T52 confusion this checker was written for."""
    from collections import Counter
    n = Counter()
    for text in table.values():
        for w in words(text):
            n[w] += 1
    return {w for w, c in n.items() if c <= 5}


def fixture_ids():
    return {e['id'] for e in json.load(CORPUS.open())['entries']}


def main():
    table = rows()
    if not table:
        sys.exit('no rows parsed from negative-vectors.md')
    rare = distinctive(table)
    missing, thin, cites = [], [], 0
    for rel in SITES:
        p = HERE / rel
        if not p.exists():
            continue
        lines = p.read_text().splitlines()
        for n, line in enumerate(lines, 1):
            # a table's own id column is the definition, not a citation --
            # the rest of the row is prose and is read
            body = ID_COLUMN.sub('|', line) if rel == 'negative-vectors.md' else line
            for cid in CITE.findall(body):
                cites += 1
                if cid not in table:
                    missing.append((rel, n, cid, line.strip()[:90]))
                    continue
                ctx = ' '.join(lines[max(0, n - 2):n + 1])
                if not (words(ctx) & words(table[cid]) & rare):
                    thin.append((rel, n, cid, line.strip()[:90]))
    print(f'{cites} citations checked against {len(table)} rows '
          f'in {len(SITES)} documents and tools; {len(missing)} flags')
    for rel, n, cid, line in missing:
        print(f'  FLAG {rel}:{n} cites {cid}, which names no row: {line}')
    ids = fixture_ids()
    named, phantom = 0, []
    for rel in FIXTURE_SITES:
        p = HERE / rel
        if not p.exists():
            continue
        for n, line in enumerate(p.read_text().splitlines(), 1):
            for fid in FIXTURE.findall(line):
                named += 1
                if fid not in ids:
                    phantom.append((rel, n, fid, line.strip()[:90]))
    print(f'{named} fixture names checked against {len(ids)} corpus ids '
          f'in {len(FIXTURE_SITES)} documents; {len(phantom)} flags')
    for rel, n, fid, line in phantom:
        print(f'  FLAG {rel}:{n} names {fid}, which is no fixture: {line}')
    missing = missing + phantom
    if '--overlap' in sys.argv:
        print(f'\n{len(thin)} citations share no distinctive word with the '
              f'row they name ({len(rare)} of the table\'s words are '
              f'distinctive enough to count):')
        for rel, n, cid, line in thin:
            print(f'  {rel}:{n} -> {cid}: {line}')
            print(f'      row: {table[cid][:110]}')
    return 1 if missing else 0


if __name__ == '__main__':
    sys.exit(main())
