#!/usr/bin/env python3
"""What the Kotlin shell still owes in documentation.

The Rust side has `#![warn(missing_docs)]` and the gate's `-D warnings` to
hold it; Kotlin has no equivalent this project can turn on, so the count is
taken here instead.  Run from the repository root:

    python3 Robot/tools/kdoccheck.py            # the counts per file
    python3 Robot/tools/kdoccheck.py --list     # every owed declaration

What is counted: every declaration a reader outside the file can reach —
`class`, `object`, `interface`, `fun`, `val`, `var`, `enum class` — that
carries no `/** */` or `//` above it.  `private` and `internal`
declarations are not counted: they are the file's own business, and the
convention asks for a comment where it says something, not on every line.

**A local is not a declaration a reader reaches.**  Kotlin puts `val`
inside a function body with the same keyword it puts one on a class, so the
indentation decides: anything more deeply indented than the `fun` it sits
in is that function's own working and is skipped.

An override carries its contract from what it overrides, so `override` is
not counted either.
"""
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
SRC = ROOT / "crates/mobile/android/app/src/main/kotlin"

DECL = re.compile(
    r'^(?P<indent>\s*)'
    r'(?P<mods>(?:@\w+(?:\([^)]*\))?\s+|public\s+|private\s+|internal\s+|protected\s+|override\s+|abstract\s+|open\s+|sealed\s+|data\s+|final\s+|companion\s+|inner\s+|const\s+|lateinit\s+|external\s+|suspend\s+|inline\s+|operator\s+|annotation\s+|value\s+)*)'
    r'(?P<kind>class|object|interface|enum class|fun|val|var)\s+'
    # a generic's type parameters sit between the keyword and the name
    r'(?:<[^>]*>\s*)?'
    r'(?P<name>[A-Za-z_][\w]*)'
)
SKIP = re.compile(r'\b(private|internal|override)\b')


TYPE_KINDS = ('class', 'object', 'interface', 'enum class')


def opens_body(m, line):
    """Whether this declaration opens a scope that is its own working.

    A function body is one.  So is a property's block initialiser — `val x
    = run { ... }` puts locals under a `val`, and they are no more public
    than a function's.  A type's body is not: its members are the point.
    """
    if m.group('kind') == 'fun':
        return True
    return m.group('kind') in ('val', 'var') and line.rstrip().endswith('{')


def documented(lines, i):
    """Whether line `i` carries a comment above it, over any annotations."""
    at = i
    while at > 0 and (lines[at - 1].lstrip().startswith('@')
                      or lines[at - 1].strip() == ''):
        at -= 1
    return at > 0 and lines[at - 1].strip().startswith(('*/', '/**', '//', '*'))


def owed(path: Path):
    """Every declaration in `path` a reader outside it reaches undocumented.

    Each is `(line, kind, name, text)`, in file order.
    """
    lines = path.read_text().split('\n')
    out = []
    # the scopes open at this line, outermost first: ('body', indent) for a
    # function or initialiser, ('type', indent, public) for a class's own
    # body.  Nothing inside a body is public, and nothing inside a
    # `private` type is either, however it is declared.
    stack = []
    # an unclosed parameter list: counted by parenthesis rather than by
    # looking for the body, because a default value may hold both a `=`
    # and a lambda's braces
    sig = 0
    for i, l in enumerate(lines):
        if sig:
            sig += l.count('(') - l.count(')')
            continue
        if not l.strip():
            continue
        indent = len(l) - len(l.lstrip())
        while stack and indent <= stack[-1][1]:
            stack.pop()
        if any(e[0] == 'body' for e in stack):
            continue
        m = DECL.match(l)
        if m is None:
            continue
        public = not (SKIP.search(m.group('mods') or '')
                      or SKIP.search(l[:m.start('kind')]))
        public = public and all(e[2] for e in stack if e[0] == 'type')
        if public and not documented(lines, i):
            out.append((i + 1, m.group('kind'), m.group('name'),
                        l.strip()[:60]))
        if opens_body(m, l):
            stack.append(('body', indent, False))
            sig = max(0, l.count('(') - l.count(')'))
        elif m.group('kind') in TYPE_KINDS:
            stack.append(('type', indent, public))
    return out


def main():
    show = '--list' in sys.argv
    total = 0
    for p in sorted(SRC.rglob('*.kt')):
        items = owed(p)
        total += len(items)
        if items:
            print(f"{len(items):4}  {p.relative_to(SRC)}")
            if show:
                for n, kind, name, text in items:
                    print(f"        {p.name}:{n} | {kind} {name}")
    print(f"\n{total} owed across {len(list(SRC.rglob('*.kt')))} files")


if __name__ == '__main__':
    main()
