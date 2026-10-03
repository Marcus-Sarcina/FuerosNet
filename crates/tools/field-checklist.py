#!/usr/bin/env python3
"""The tester's checklist for one field run (`Robot/field-test-diagnostics.md`,
section 3.7): one block per catalogue entry a person has to observe.

Rows are every `kind: manual` entry of `crates/acceptance/acceptance.json`
(the product rows a screen has to show), plus the three entries that need two
devices in the same room and so cannot run on the gate: TRV-12, TRV-13 and
MET-12.  Each block carries the id, the title, the `when` (what the tester
does) and the `then` (what the tester looks for), three boxes to mark one of,
and a notes line.  A withdrawn entry is a tombstone and is not a row.

    field-checklist.py [--catalogue PATH] [--run NAME] [-o FILE]

Runs from anywhere: the catalogue is found relative to this file.
`crates/tools/field-run.sh` writes the result into each run directory.
"""
import argparse
import json
import os
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
CATALOGUE = os.path.join(os.path.dirname(HERE), "acceptance", "acceptance.json")

# Not manual in the catalogue, but their `then` needs two devices face to face.
TWO_DEVICE = ["TRV-12", "TRV-13", "MET-12"]

MARKS = "| observed | not observed | not reached |\n|---|---|---|\n| [ ] | [ ] | [ ] |"


def rows(entries):
    """The entries that make a row, manual ones first in id order, then the
    two-device three in the order named.  A named id absent from the
    catalogue is an error: the list above would be lying about coverage."""
    by_id = {e["id"]: e for e in entries}
    manual = sorted(
        (e for e in entries if e.get("kind") == "manual"), key=lambda e: e["id"]
    )
    out = list(manual)
    for i in TWO_DEVICE:
        if i not in by_id:
            raise KeyError(f"{i} is not in the catalogue")
        e = by_id[i]
        if e.get("kind") == "withdrawn":
            raise KeyError(f"{i} is withdrawn")
        if e not in out:
            out.append(e)
    return out


def render(entries, run=None):
    """The checklist as Markdown."""
    chosen = rows(entries)
    head = ["# Field-test checklist" + (f": {run}" if run else "")]
    head.append("")
    head.append(
        "Run: " + (f"`{run}`" if run else "________")
        + "    Tester: ________    Date: ________    Counterparty run: ________"
    )
    head.append("")
    head.append(
        "One mark per row. *Not reached* means the run ended before the step "
        "this row is about; *not observed* means the step ran and the screen "
        "did not show what the row expects. Write what you saw instead on the "
        "notes line, in your own words."
    )
    head.append("")
    head.append(f"{len(chosen)} rows.")
    head.append("")
    body = []
    for e in chosen:
        body.append(f"## {e['id']}  {e['title']}")
        body.append("")
        body.append(f"Do: {e['when']}")
        body.append("")
        body.append(f"Look for: {e['then']}")
        body.append("")
        body.append(MARKS)
        body.append("")
        body.append("Notes: ______________________________________________")
        body.append("")
    return "\n".join(head + body)


def main(argv=None):
    p = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    p.add_argument("--catalogue", default=CATALOGUE)
    p.add_argument("--run", default=None, help="the run's name, for the header")
    p.add_argument("-o", "--output", default=None, help="write here instead of stdout")
    a = p.parse_args(argv)
    with open(a.catalogue, encoding="utf-8") as f:
        entries = json.load(f)
    text = render(entries, a.run)
    if a.output:
        with open(a.output, "w", encoding="utf-8") as f:
            f.write(text)
    else:
        sys.stdout.write(text)
    return 0


if __name__ == "__main__":
    sys.exit(main())
