#!/usr/bin/env python3
"""Run only the two proxy-boundary probes; remove the temporary test afterwards."""
from pathlib import Path
import os
import subprocess

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[2]
ANDROID = ROOT / "crates/mobile/android"
dest = ANDROID / "app/src/test/kotlin/com/comptus/fueros/Review20261125Test.kt"
with dest.open("x") as out:
    out.write((HERE / dest.name).read_text())
env = dict(os.environ)
env.setdefault("JAVA_HOME", str(Path.home() / "opt/android-studio/jbr"))
try:
    result = subprocess.run([
        "nice", "-n", "19", "./gradlew", "--offline", "--no-daemon", "-q",
        "testReleasableDebugUnitTest", "--tests", "com.comptus.fueros.Review20261125Test",
    ], cwd=ANDROID, env=env)
finally:
    dest.unlink()
raise SystemExit(result.returncode)
