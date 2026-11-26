#!/usr/bin/env python3
"""Run review counterexamples against existing fixtures, leaving the tree's tests intact.

The assertions express the required behaviour; failures reproduce the findings.
Run from anywhere, with no code gate running concurrently. Temporary test copies
are removed even when Cargo fails. No existing source or expectation is edited.
"""
from pathlib import Path
import os
import subprocess

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[2]
env = dict(os.environ, CARGO_BUILD_JOBS="8")
failed = False
for crate, source, fragment in [
    ("ffi", "boundary.rs", "boundary.rs"),
    ("node", "node_keys.rs", "node_keys.rs"),
    ("node", "resources.rs", "resources.rs"),
]:
    dest = ROOT / "crates" / crate / "tests" / f"review_20261125_{Path(source).stem}.rs"
    fixture = dest.parent / source
    with dest.open("x") as out:
        out.write(fixture.read_text() + (HERE / fragment).read_text())
    try:
        result = subprocess.run([
            "nice", "-n", "19", "cargo", "test", "--offline", "-j", "8",
            "-p", f"rhtn-{crate}", "--test", dest.stem,
            "review_20261125", "--", "--nocapture", "--test-threads=1",
        ], cwd=ROOT / "crates", env=env)
        failed |= result.returncode != 0
    finally:
        dest.unlink()
raise SystemExit(1 if failed else 0)
