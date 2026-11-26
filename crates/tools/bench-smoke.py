#!/usr/bin/env python3
"""Boot a node image on the hosted test node, before `latest` names it.

    tools/bench-smoke.py <image> [--node <address>] [--as <entry>]

`packaging/smoke.sh` run on the bench node's own machine against `<image>`
(`docker.io/sarcina/rhtnd:<commit>`, which CI pushes for every commit on
master): the image pulled there, booted in each shape a node takes, its
surface's proof checked, and every container it made removed. **Where it
runs** [author, 2026-11-26]: "We'll run it on the hosted test node. For
now, that can be GXV's node." The node serving there is not touched: the
test's containers keep namespaces of their own, and the one surface it
publishes is moved off 7449 to 7459.

The way in is the bench entry's host control key (`--as`, default GXV's
`RFCRC1JDGXV`), derived as `bench-package.py` derives it into a private
temporary directory removed when the run is done; `--node` defaults as
there. Exits with the test's own status.
"""
import os
import subprocess
import sys
import tempfile

HERE = os.path.dirname(os.path.abspath(__file__))
CRATES = os.path.dirname(HERE)
sys.path.insert(0, HERE)
key_script = __import__("bench-host-key")
bench = __import__("bench-package")

SMOKE = os.path.join(CRATES, "packaging", "smoke.sh")
PORT = 7459


def main(argv):
    image, node, entry = None, os.environ.get("RHTN_BENCH_NODE"), "RFCRC1JDGXV"
    i = 0
    while i < len(argv):
        if argv[i] in ("--node", "--as") and i + 1 < len(argv):
            if argv[i] == "--node":
                node = argv[i + 1]
            else:
                entry = argv[i + 1]
            i += 2
        elif image is None and not argv[i].startswith("-"):
            image = argv[i]
            i += 1
        else:
            bench.die(f"usage: bench-smoke.py <image> [--node <address>] [--as <entry>]; not {argv[i]}")
    if not image:
        bench.die("usage: bench-smoke.py <image> [--node <address>] [--as <entry>]")
    # the image rides the remote command line, so it is held to the
    # characters an image reference has, as the runner holds one
    if image.startswith("-") or not all(c.isalnum() or c in "./:@_-" for c in image):
        bench.die(f"`{image}` is not an image reference")
    node = node or bench.recorded_node()
    if not node:
        bench.die("no --node, no $RHTN_BENCH_NODE, and no node recorded in phones-recovery.conf")

    identity = key_script.seeds(key_script.entropy(key_script.phrase_for(entry)))
    with tempfile.TemporaryDirectory(prefix="rhtn-bench-") as d:
        os.chmod(d, 0o700)
        path = os.path.join(d, "identity")
        fd = os.open(path, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600)
        with os.fdopen(fd, "wb") as f:
            f.write(identity)
        host_key = os.path.join(d, "host.key")
        made = bench.rhtn(["keys", "ssh", path, "--private", host_key],
                          capture_output=True, text=True)
        if made.returncode != 0:
            bench.die(f"rhtn keys ssh refused: {made.stderr.strip() or made.stdout.strip()}")
        # the script on standard input and the image as its argument, so
        # nothing of either is interpolated into the remote line
        remote = (
            'd="$(mktemp -d)"; cat > "$d/smoke.sh"; '
            f'ENGINE=podman RHTN_SMOKE_PORT={PORT} bash "$d/smoke.sh" "$1"; '
            'rc=$?; rm -rf "$d"; exit $rc'
        )
        with open(SMOKE, "rb") as script:
            done = subprocess.run(
                ["ssh", "-i", host_key, "-o", "IdentitiesOnly=yes", "-o", "ConnectTimeout=15",
                 "-o", "BatchMode=yes", "-o", "StrictHostKeyChecking=accept-new",
                 f"root@{node}", f"bash -c '{remote}' smoke {image}"],
                stdin=script,
            )
    print(f"bench-smoke: {image} on {node}: {'passed' if done.returncode == 0 else 'FAILED'}")
    return done.returncode


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
