#!/usr/bin/env python3
"""What the gate shows a developer from one clippy run.

Reads `--message-format=json` on stdin and prints two things: every real
diagnostic, rendered exactly as clippy would have rendered it, and the list
of public items that owe a comment. Used by `crates/check.sh` stage 3b and
the per-flavour lint passes beside it, which run:

    cargo clippy ... --message-format=json -- -D warnings --force-warn missing-docs

**Why the JSON, when clippy prints perfectly good text.** The two kinds of
finding need different verdicts, and only the structured form tells them
apart reliably. A real lint must halt the gate. A missing comment must not
[author, 2026-10-05]: a gate run is expensive, so the whole list is wanted
in one pass rather than the first item followed by another run.

**Why `--force-warn`, when the lint is already `warn` in every crate
root.** Because the gate denies warnings, which turns a missing comment
into an error — and a crate that fails to compile is a crate whose
dependents go unlinted in that run. One missing comment could hide a real
lint in nine crates. `--force-warn` exempts this one lint from `-D
warnings` and nothing else from it.

**What `--force-warn` costs, and what this script gives back.** It also
overrides the suppression rustc applies to code from a macro expansion,
which the plain attribute respects. `#[uniffi::export]` generates a struct,
a field and a function per exported trait, so the FFI's device module alone
reports 27 items that exist in no source file. **Those are not owed and are
not counted**: a diagnostic whose primary span is inside an expansion is
dropped, restoring the rule the attribute itself applies.

Exit is always 0. The stage's verdict is clippy's own exit status, which
this script neither sees nor second-guesses.
"""

import json
import sys

DOC_LINT = "missing_docs"


def messages(stream):
    """Every compiler diagnostic in a cargo JSON stream, in order."""
    for raw in stream:
        raw = raw.strip()
        if not raw.startswith("{"):
            continue
        try:
            msg = json.loads(raw)
        except json.JSONDecodeError:
            # cargo interleaves its own lines; a partial one is not a
            # diagnostic and is not an error in this script's input
            continue
        if msg.get("reason") == "compiler-message" and msg.get("message"):
            yield msg["message"]


def split(diagnostics):
    """The diagnostics, as `(real, owed)`.

    `real` is every diagnostic that is not the documentation lint, each as
    the text clippy rendered, deduplicated — cargo reports a crate's
    diagnostics once per target, so a lib and its test build repeat them.

    `owed` is every documentation-lint diagnostic that names a place
    somebody can write a comment, as `(file, line, message)`, sorted and
    deduplicated the same way.
    """
    real, owed = {}, {}
    for d in diagnostics:
        code = (d.get("code") or {}).get("code")
        primary = next((s for s in d.get("spans") or [] if s.get("is_primary")), None)
        if code == DOC_LINT:
            # a span that came out of a macro is not a place to write a
            # comment: the attribute's own behaviour is to say nothing
            # about it, and --force-warn is what made it speak
            if primary is None or primary.get("expansion"):
                continue
            key = (primary["file_name"], primary["line_start"], primary["column_start"])
            owed.setdefault(key, d.get("message", ""))
            continue
        if d.get("level") not in ("error", "warning"):
            continue
        rendered = d.get("rendered") or d.get("message", "")
        real.setdefault(rendered.rstrip(), None)
    return list(real), [(f, ln, m) for (f, ln, _c), m in sorted(owed.items())]


def main():
    real, owed = split(messages(sys.stdin))
    for text in real:
        print(text)
    for f, ln, m in owed:
        print(f"  {f}:{ln} | {m}")
    if owed:
        n = len(owed)
        item = "item" if n == 1 else "items"
        print(f"  documentation: {n} public {item} owe a comment")
    return 0


if __name__ == "__main__":
    sys.exit(main())
