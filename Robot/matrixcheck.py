"""Check `transaction-rules.md`: every id it cites is a catalogue entry,
and the counts it states are the counts its own tables carry.

Run from `crates/`.  A number in prose is a claim; this is what makes it one
somebody can check rather than one somebody remembered."""
import json, re, sys
d = {e['id']: e for e in json.load(open('acceptance/acceptance.json'))}
rows, flags = [], []
for line in open('../Robot/transaction-rules.md'):
    m = re.match(r'\|\s*`([a-z0-9-]+)`\s*\|(.*)\|(.*)\|(.*)\|(.*)\|\s*$', line)
    if not m:
        continue
    cond, _rule, _where, pos, neg = (x.strip() for x in m.groups())
    def ids(cell):
        return [i for i in re.findall(r'\b[A-Z]{3}-\d{2}\b', cell)]
    def state(cell):
        if '**deferred**' in cell: return 'deferred'
        if cell in ('gap', '—', '-'): return 'none'
        return 'held' if ids(cell) else 'none'
    for cell in (pos, neg):
        for i in ids(cell):
            if i not in d:
                flags.append(f'{cond}: {i} is not a catalogue entry')
    rows.append((cond, state(pos), state(neg)))

both = sum(1 for _, p, n in rows if p == 'held' and n == 'held')
one = sum(1 for _, p, n in rows if (p == 'held') != (n == 'held') and 'deferred' not in (p, n))
neither = sum(1 for _, p, n in rows if p == 'none' and n == 'none')
deferred = sum(1 for _, p, n in rows if 'deferred' in (p, n))
stated = re.search(r'- (\d+) hold on both sides\n- (\d+) have one polarity only\n- (\d+) (?:have|has) neither\n- (\d+) are deferred', open('../Robot/transaction-rules.md').read())
print(f'conditions {len(rows)}: both {both}, one {one}, neither {neither}, deferred {deferred}')
if stated:
    said = tuple(int(x) for x in stated.groups())
    if said != (both, one, neither, deferred):
        flags.append(f'the file says {said} and its tables say {(both, one, neither, deferred)}')
else:
    flags.append('the file states no counts to check')
for f in flags:
    print('  FLAG', f)
print(f'flags {len(flags)}')
sys.exit(1 if flags else 0)
