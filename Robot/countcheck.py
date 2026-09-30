"""Recount `functional_tests.md`'s own tables against its own rows.

Run from anywhere.  §10 states a headline count of families and a
per-prefix table; the rows are the truth and both summaries drift from
them the moment a row lands.  Three divergent numbers survived two review
passes before this existed, so the count is now a claim somebody checks
rather than one somebody remembered (`CLAUDE.md`: *counts drift*, *report
numbers, not states*).

The posture the file keeps, and that this enforces:

  * a **live** family is a row whose second column is not `—`;
  * a **withdrawn** row keeps its number as a tombstone and is counted
    nowhere, so a citation resolves to *withdrawn* rather than to some
    later family;
  * the prefix table and the headline both count live families only;
  * `O-` rows are the open-question register, not families.
"""
import re
import sys
from pathlib import Path

DOC = Path(__file__).resolve().parent.parent / 'functional_tests.md'


def main():
    s = DOC.read_text()
    table = {k: int(v) for k, v in re.findall(r'^\| ([A-Z]+) \| (\d+) \|$', s, re.M)}
    live, withdrawn = {}, {}
    for prefix, _n, second in re.findall(r'^\| ([A-Z]+)-(\d+) \|([^|]*)\|', s, re.M):
        if prefix == 'O':
            continue
        into = withdrawn if second.strip() == '—' else live
        into[prefix] = into.get(prefix, 0) + 1
    flags = []
    for prefix in sorted(set(table) | set(live)):
        said, counted = table.get(prefix, 0), live.get(prefix, 0)
        if said != counted:
            flags.append(f'the {prefix} row says {said}; {counted} live '
                         f'{prefix} rows are in the document')
    total = sum(live.values())
    head = re.search(r'contains \*\*(\d+) numbered requirement/test families\*\* '
                     r'across \*\*(\d+) ID prefixes\*\*', s)
    if not head:
        flags.append('§10 states no headline count to check')
    else:
        if int(head.group(1)) != total:
            flags.append(f'the headline says {head.group(1)} families; '
                         f'{total} live rows are in the document')
        if int(head.group(2)) != len(table):
            flags.append(f'the headline says {head.group(2)} prefixes; '
                         f'the table lists {len(table)}')
    print(f'{total} live families across {len(table)} prefixes, '
          f'{sum(withdrawn.values())} withdrawn tombstones '
          f'({", ".join(f"{k} {v}" for k, v in sorted(withdrawn.items()))}); '
          f'{len(flags)} flags')
    for f in flags:
        print('  FLAG', f)
    return 1 if flags else 0


if __name__ == '__main__':
    sys.exit(main())
