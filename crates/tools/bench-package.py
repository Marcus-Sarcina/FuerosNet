#!/usr/bin/env python3
"""Install, grant and watch a package on a bench node, as its operator.

    tools/bench-package.py <verb> [--node <address>] [--as <entry>] [rhtn node options]
    tools/bench-package.py mint <file>

`<verb>` is one of `rhtn node`'s package acts -- `install`, `remove`,
`grant`, `grant-template`, `grants`, `grant-narrow`, `grant-drop`,
`relate`, `unrelate`, `packages`, `running`, `logs`, `log`, `acts` -- and every
option after it is handed to `rhtn node <verb>` as it is
(`--resource`, `--image`, `--port`, `--manifest`, `--authority`, `--roles`,
`--of`, `--relation`, `--instance`, ...). What this adds is the operator, so a caller holds no key:

  - **the identity** is the bench entry's (`--as`, default GXV's
    `RFCRC1JDGXV`), turned from its phrase in `phones-recovery.conf` exactly
    as `bench-host-key.py` turns it, into a private temporary directory that
    is removed when the act is done;
  - **the way in** is an ssh tunnel to the node's administration surface on
    its own loopback, opened with that identity's host control key: the node
    runs on the host's network, so an act arriving there needs no act token;
  - **`--owner`** for an install is that identity's keyhash where none is
    given, since a bench package is owned by the node's operator.

`--image` names a digest, never a tag (`docker.io/sarcina/<name>@sha256:<64
hex>`, the `digest:` line `docker push` ends with): the node refuses a tag.

A second `install` of the same `--resource` with a new `--image` is how a
build is deployed again: the node replaces the package under the name it
derives from the resource, and keeps its grants, its relationships, its
block, its log and its jobs.

`mint <file>` makes a resource's identity at `<file>` and prints its
keyhash, which is what `--resource` names; keep the file wherever the
package's own repository keeps its bench state, and reuse the keyhash.

`--node` defaults to `$RHTN_BENCH_NODE`, else the node recorded last in
`phones-recovery.conf`.

**Bench identities only** [author, 2026-10-29: the phrases in that file are
test identities; 2026-11-16: "I expect you to handle the key management for
the test runs."]. Nothing of the phrase, the seeds or a key is printed.
"""

import os
import re
import socket
import subprocess
import sys
import tempfile
import time

HERE = os.path.dirname(os.path.abspath(__file__))
CRATES = os.path.dirname(HERE)
sys.path.insert(0, HERE)
key_script = __import__("bench-host-key")

VERBS = {"install", "remove", "grant", "grant-template", "grants", "grant-narrow", "grant-drop",
         "relate", "unrelate",
         "packages", "running", "logs", "log", "acts"}
SURFACE = 7449


def die(why):
    print(f"bench-package: {why}", file=sys.stderr)
    sys.exit(2)


def rhtn(args, **kw):
    return subprocess.run(
        ["cargo", "run", "-q", "-p", "rhtn-cli", "--"] + args, cwd=CRATES, **kw
    )


def recorded_node():
    """The address on the last `# Node:` line that names one."""
    found = None
    with open(key_script.CONF) as f:
        for line in f:
            if line.startswith("# Node:"):
                m = re.search(r"\bat (\d+\.\d+\.\d+\.\d+)", line)
                if m:
                    found = m.group(1)
    return found


def free_port():
    with socket.socket() as s:
        s.bind(("127.0.0.1", 0))
        return s.getsockname()[1]


def tunnel(host_key, node, port):
    """An ssh -L to the node's surface, returned once it answers."""
    p = subprocess.Popen(
        [
            "ssh", "-i", host_key, "-o", "IdentitiesOnly=yes", "-o", "ConnectTimeout=15",
            "-o", "ExitOnForwardFailure=yes", "-o", "StrictHostKeyChecking=accept-new",
            "-N", "-L", f"{port}:127.0.0.1:{SURFACE}", f"root@{node}",
        ],
        stdin=subprocess.DEVNULL,
    )
    for _ in range(60):
        if p.poll() is not None:
            die(f"the tunnel to {node} closed: ssh said {p.returncode}")
        try:
            socket.create_connection(("127.0.0.1", port), timeout=1).close()
            return p
        except OSError:
            time.sleep(0.25)
    p.kill()
    die(f"the tunnel to {node} did not open")


def main(argv):
    if len(argv) == 2 and argv[0] == "mint":
        made = rhtn(["keys", "mint", argv[1]], capture_output=True, text=True)
        if made.returncode != 0:
            die(made.stderr.strip() or made.stdout.strip())
        shown = rhtn(["keys", "show", argv[1]], capture_output=True, text=True)
        print(next((l for l in shown.stdout.splitlines() if "keyhash" in l), "").strip())
        return 0
    if not argv or argv[0] not in VERBS:
        die("usage: bench-package.py <install|remove|grant|grant-template|grants|grant-narrow|"
            "grant-drop|relate|unrelate|packages|running|logs|log|acts> "
            "[--node <address>] [--as <entry>] [rhtn node options] | mint <file>")
    verb, rest = argv[0], argv[1:]
    node = os.environ.get("RHTN_BENCH_NODE")
    entry = "RFCRC1JDGXV"
    passed = []
    i = 0
    while i < len(rest):
        if rest[i] in ("--node", "--as") and i + 1 < len(rest):
            if rest[i] == "--node":
                node = rest[i + 1]
            else:
                entry = rest[i + 1]
            i += 2
            continue
        if rest[i] in ("--at", "--identity", "--token"):
            die(f"{rest[i]} is this script's to give")
        passed.append(rest[i])
        i += 1
    node = node or recorded_node()
    if not node:
        die("no --node, no $RHTN_BENCH_NODE, and no node recorded in phones-recovery.conf")

    identity = key_script.seeds(key_script.entropy(key_script.phrase_for(entry)))
    with tempfile.TemporaryDirectory(prefix="rhtn-bench-") as d:
        os.chmod(d, 0o700)
        path = os.path.join(d, "identity")
        fd = os.open(path, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600)
        with os.fdopen(fd, "wb") as f:
            f.write(identity)
        host_key = os.path.join(d, "host.key")
        made = rhtn(["keys", "ssh", path, "--private", host_key], capture_output=True, text=True)
        if made.returncode != 0:
            die(f"rhtn keys ssh refused: {made.stderr.strip() or made.stdout.strip()}")
        if verb == "install" and "--owner" not in passed:
            shown = rhtn(["keys", "show", path], capture_output=True, text=True)
            line = next((l for l in shown.stdout.splitlines() if "keyhash" in l), "")
            passed += ["--owner", line.split()[-1]]
        port = free_port()
        t = tunnel(host_key, node, port)
        try:
            done = rhtn(["node", verb, "--at", f"127.0.0.1:{port}", "--identity", path] + passed)
        finally:
            # **by its own PID**: a pattern would match this script's shell
            t.terminate()
            t.wait(timeout=10)
    return done.returncode


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
