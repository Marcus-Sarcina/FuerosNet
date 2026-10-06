"""Say whether any model result is stale.

`models/results/` holds each tool's verdict. Nothing recorded which source
a verdict was *about*, so a result that outlived an edit to its theory read
exactly like a current one, and "the results match the sources" was a claim
checkable only by reading them (`CLAUDE.md`: *report numbers, not states*).

`models/run-all.sh` now stamps every run into `results/sources.json`: the
result file, each source it proved, and that source's hash at the moment it
was proved. This compares those hashes against the files as they are now.

Three kinds of flag, and each means a different thing:

  STALE    -- the source changed after the result was written. The verdict
              is about bytes that no longer exist; re-run.
  UNSTAMPED-- a result file with no entry. Either it predates the manifest
              or a run site does not stamp, and neither is evidence.
  MISSING  -- a stamped source that is gone. The result is about something
              the tree no longer has.
"""
import hashlib
import json
import sys
from pathlib import Path

MODELS = Path(__file__).resolve().parent.parent / 'models'
RESULTS = MODELS / 'results'
MANIFEST = RESULTS / 'sources.json'


def main():
    if not MANIFEST.exists():
        print('models/results/sources.json absent: run models/run-all.sh, '
              'which writes it')
        return 1
    stamped = json.loads(MANIFEST.read_text())['results']
    flags, checked = [], 0
    for out, sources in sorted(stamped.items()):
        if not (RESULTS / out).exists():
            flags.append(f'MISSING result {out}, stamped but not present')
        for src, want in sorted(sources.items()):
            p = MODELS / src
            if not p.exists():
                flags.append(f'MISSING source {src}, proved by {out}')
                continue
            checked += 1
            got = hashlib.sha256(p.read_bytes()).hexdigest()
            if got != want:
                flags.append(f'STALE {out}: {src} changed after it was '
                             f'proved ({want[:12]}… → {got[:12]}…)')
    # A result nobody stamped is not evidence either -- and an ORPHAN is the
    # case that matters: a run renamed or retired leaves its old output
    # sitting beside the new one, reading exactly like a current result.
    # Only the generated theory files are excused, never a verdict.
    for p in sorted(RESULTS.glob('*.txt')):
        if p.name not in stamped:
            flags.append(f'UNSTAMPED {p.name}: no source recorded for it, '
                         f'so it is either orphaned by a renamed run or a '
                         f'site that does not stamp')
    # **And the other direction, which the stamped set cannot see**
    # [reviewer, 2026-10-06]: a theory added to the tree and never run is
    # named by no result, so every check above passes while the file has
    # never been proved.  The sources are globbed rather than listed for
    # the same reason `refcheck` has no exemptions: a list is a place for
    # something to go missing from.  `results/` holds the generated
    # mutants and the bounded copies, which are a run's output and not a
    # source anybody writes.
    proved = {s for srcs in stamped.values() for s in srcs}
    for p in sorted(MODELS.rglob('*.spthy')) + sorted(MODELS.rglob('*.tla')):
        rel = p.relative_to(MODELS).as_posix()
        if rel.startswith('results/'):
            continue
        if rel not in proved:
            flags.append(f'UNPROVED {rel}: a theory in the tree that no '
                         f'stamped result names, so nothing has run it')
    print(f'{len(stamped)} stamped results over {checked} sources; '
          f'{len(flags)} flags')
    for f in flags:
        print('  FLAG', f)
    return 1 if flags else 0


if __name__ == '__main__':
    sys.exit(main())
