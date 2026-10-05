#!/usr/bin/env python3
"""Emit a `docput` plan skeleton from the compiler's own `missing_docs`
warnings, so line numbers are never transcribed by hand.

    cargo build -p <crate> 2>&1 | Robot/tools/docplan.py > plan.txt

Each block carries the file, the current line, a fingerprint taken from
that line, and the line itself as a comment to write against.  Fill in the
text under each `>` marker and feed the file to `docput.py`.
"""
import sys, re, pathlib

seen, out = set(), []
lines = sys.stdin.read().split('\n')
for i, l in enumerate(lines):
    m = re.match(r'\s*--> (.+?):(\d+):\d+\s*$', l)
    if not m:
        continue
    f, n = m.group(1), int(m.group(2))
    if (f, n) in seen:
        continue
    seen.add((f, n))
    try:
        src = pathlib.Path(f).read_text().split('\n')[n-1]
    except (OSError, IndexError):
        continue
    text = src.strip()
    if not text or text.startswith('#!['):
        continue
    # a fingerprint distinctive enough to catch a shift, short enough to
    # survive a reformat: the line's own text, trimmed
    fp = text[:48]
    out.append(f"{f}:{n} | {fp}\n> {text}\n")
print('\n'.join(out))
print(f"# {len(out)} items", file=sys.stderr)
