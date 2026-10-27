#!/usr/bin/env python3
"""Run the 2026-10-26 counterexamples; passing means the defect reproduced.

Builds the existing node library offline, without changing core source files.
"""
import json
import os
from pathlib import Path
import subprocess
import tempfile

root = Path(__file__).resolve().parent.parent
env = dict(os.environ, CARGO_BUILD_JOBS="8")
build = subprocess.run(
    ["nice", "-n", "10", "cargo", "build", "-p", "rhtn-node", "--lib",
     "--offline", "--message-format=json"],
    cwd=root / "crates", env=env, text=True, stdout=subprocess.PIPE, check=True,
)
libraries = {}
for line in build.stdout.splitlines():
    item = json.loads(line)
    if item.get("reason") == "compiler-artifact":
        name = item["target"]["name"]
        if name in {"rhtn_node", "rhtn_archive"}:
            libraries[name] = next(p for p in item["filenames"] if p.endswith(".rlib"))
with tempfile.TemporaryDirectory(prefix="fn-directive-review-") as directory:
    binary = str(Path(directory) / "probes")
    command = ["nice", "-n", "10", "rustc", "--edition=2024", "--test",
               str(root / "resource-design/directive-review-probes-2026-10-26.rs")]
    for name, path in libraries.items():
        command += ["--extern", f"{name}={path}"]
    command += ["-L", f"dependency={Path(libraries['rhtn_archive']).parent}", "-o", binary]
    subprocess.run(command, check=True)
    subprocess.run([binary, "--nocapture"], check=True)
