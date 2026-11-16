#!/usr/bin/env python3
"""The host key a bench identity derives, written where ssh can read it.

    tools/bench-host-key.py <entry> <out> [--runner]

`<entry>` names a line of `tools/phones-recovery.conf` (an adb serial, or
`<serial>:<id8>` for a phone's second identity). The phrase there is turned
into the identity the phone holds exactly as the shell turns it
(`Identity.kt`: BIP-39's 24 words to 32 bytes of entropy, HKDF-SHA256 with a
zero salt under `fueros identity seeds v1` to the kernel's 64 bytes), and
`rhtn keys ssh` derives the host control key from that -- or, with
`--runner`, the restricted runner key -- and writes its private half to
`<out>` at 0600.

**Bench identities only** [author, 2026-10-29: the phrases in that file are
test identities]: the identity file lives in a private temporary directory
for the length of the call and is removed; nothing of the phrase, the seeds
or the key is printed. What is printed is the public line and the keyhash,
so a caller can tell which identity it holds.

[author, 2026-11-16: "I expect you to handle the key management for the test
runs."]
"""
import hashlib
import hmac
import os
import subprocess
import sys
import tempfile

HERE = os.path.dirname(os.path.abspath(__file__))
CRATES = os.path.dirname(HERE)
CONF = os.path.join(HERE, "phones-recovery.conf")
WORDLIST = os.path.join(CRATES, "mobile/android/app/src/main/assets/bip39/english.txt")
SEEDS_INFO = b"fueros identity seeds v1"


def die(why):
    print(f"bench-host-key: {why}", file=sys.stderr)
    sys.exit(2)


def phrase_for(entry):
    with open(CONF) as f:
        for line in f:
            line = line.strip()
            if not line or line.startswith("#") or "=" not in line:
                continue
            key, value = line.split("=", 1)
            if key.strip() == entry:
                return value.split()
    die(f"{entry} is not an entry of {CONF}")


def entropy(words):
    with open(WORDLIST) as f:
        listed = [w.strip() for w in f if w.strip()]
    if len(listed) != 2048:
        die("the wordlist is not 2048 words")
    index = {w: i for i, w in enumerate(listed)}
    given = [w.lower() for w in words]
    if len(given) != 24:
        die(f"a phrase is 24 words; that entry has {len(given)}")
    if any(w not in index for w in given):
        die("a word in that entry is not on the list")
    bits = "".join(format(index[w], "011b") for w in given)
    raw = bytes(int(bits[i * 8:i * 8 + 8], 2) for i in range(32))
    if format(hashlib.sha256(raw).digest()[0], "08b") != bits[256:264]:
        die("that entry's words do not check")
    return raw


def seeds(master):
    prk = hmac.new(bytes(32), master, hashlib.sha256).digest()
    t1 = hmac.new(prk, SEEDS_INFO + b"\x01", hashlib.sha256).digest()
    t2 = hmac.new(prk, t1 + SEEDS_INFO + b"\x02", hashlib.sha256).digest()
    return t1 + t2


def main(argv):
    if len(argv) not in (2, 3) or (len(argv) == 3 and argv[2] != "--runner"):
        die("usage: bench-host-key.py <entry> <out> [--runner]")
    entry, out = argv[0], os.path.abspath(argv[1])
    runner = len(argv) == 3
    identity = seeds(entropy(phrase_for(entry)))
    with tempfile.TemporaryDirectory(prefix="rhtn-bench-") as d:
        os.chmod(d, 0o700)
        path = os.path.join(d, "identity")
        fd = os.open(path, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600)
        with os.fdopen(fd, "wb") as f:
            f.write(identity)
        shown = subprocess.run(
            ["cargo", "run", "-q", "-p", "rhtn-cli", "--", "keys", "show", path],
            cwd=CRATES, capture_output=True, text=True,
        )
        args = ["keys", "ssh", path] + (["--runner"] if runner else []) + ["--private", out]
        made = subprocess.run(
            ["cargo", "run", "-q", "-p", "rhtn-cli", "--"] + args,
            cwd=CRATES, capture_output=True, text=True,
        )
    if made.returncode != 0:
        die(f"rhtn keys ssh refused: {made.stderr.strip() or made.stdout.strip()}")
    keyhash = next((l for l in shown.stdout.splitlines() if "keyhash" in l), "").strip()
    print(keyhash)
    print(made.stdout.strip())


if __name__ == "__main__":
    main(sys.argv[1:])
