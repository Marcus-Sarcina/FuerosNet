#!/usr/bin/env python3
"""Insert doc comments above named items, by line number.

Reads a plan from stdin: blank-line-separated blocks, each

    <file>:<line>[ | <fingerprint>]
    text of the comment, one or more lines, no `///` prefix

The line number is where the item is **now**; insertions are applied from
the bottom up so earlier numbers stay valid within one run.  The item's own
indentation is matched.  Refuses a line that already carries a `///` above
it, so running twice is safe.

**Across runs the numbers go stale**, and a plan computed before an edit
will otherwise insert into the middle of a function body, which compiles
and is wrong [2026-10-04, in the node crate].  So the plan may carry a
fingerprint after `|`: a substring the target line must contain, checked
before anything is written.  Without one the tool checks that the line
looks like an item at all — a `pub` declaration, an enum variant or a
field — and refuses it otherwise.
"""
import sys, re, collections

blocks, cur = [], []
for raw in sys.stdin.read().split('\n'):
    if raw.strip() == '':
        if cur: blocks.append(cur); cur = []
    else:
        cur.append(raw)
if cur: blocks.append(cur)

by_file = collections.defaultdict(list)
for b in blocks:
    loc, *text = b
    fp = None
    if '|' in loc:
        loc, fp = loc.split('|', 1)
        loc, fp = loc.strip(), fp.strip()
    f, n = loc.rsplit(':', 1)
    by_file[f].append((int(n), text, fp))

# a line that could carry a doc comment: a declaration, a variant or a field
ITEM = re.compile(r'^\s*(pub(\(crate\))? (fn|struct|enum|trait|const|type|mod|[a-z_]+:)'
                  r'|[A-Z]\w*\s*(\(|\{|,|=|$)'
                  r'|[a-z_]\w*:\s*\S'
                  r'|fn \w+)')

put = skipped = 0
for f, items in by_file.items():
    lines = open(f).read().split('\n')
    for n, text, fp in sorted(items, reverse=True, key=lambda x: x[0]):
        i = n - 1
        if i < 0 or i >= len(lines):
            print(f"  ?? {f}:{n} out of range", file=sys.stderr); skipped += 1; continue
        if fp is not None and fp not in lines[i]:
            print(f"  !! {f}:{n} is {lines[i].strip()[:48]!r}, not {fp!r}; stale plan",
                  file=sys.stderr); skipped += 1; continue
        if fp is None and not ITEM.match(lines[i]):
            print(f"  !! {f}:{n} does not look like an item: {lines[i].strip()[:48]!r}",
                  file=sys.stderr); skipped += 1; continue
        indent = re.match(r'\s*', lines[i]).group(0)
        # a doc comment goes above the item's attributes, not between them
        # and the item: walk back over `#[...]` lines first
        at = i
        while at > 0 and lines[at-1].strip().startswith('#['):
            at -= 1
        prev = lines[at-1].strip() if at > 0 else ''
        if prev.startswith('///'):
            print(f"  == {f}:{n} already documented", file=sys.stderr); skipped += 1; continue
        lines[at:at] = [f"{indent}/// {t}".rstrip() for t in text]
        put += 1
    open(f, 'w').write('\n'.join(lines))
print(f"  {put} inserted, {skipped} skipped")
