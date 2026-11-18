#!/usr/bin/env python3
"""The fabric on rented machines: one `rhtnd` per infra node, each in its
own DigitalOcean region, resolving one another over the public internet.

    wan.py up         make the machines (one per infra node) and wait for
                      their addresses; writes <out>/addresses.json
    wan.py provision  sign the fabric against those addresses (the Rust
                      provisioner), copy each node's state up, start it
    wan.py status     what each machine says of its node
    wan.py logs NAME  the node's log on that machine
    wan.py down       destroy every machine tagged rhtn-wan

The in-process fabric (`rhtn_sim::subnets::Fabric::build`) is the
automated test; this is the same plan on real machines, so that whatever
the in-process nodes let a test get away with -- one clock, one host, no
latency, a table reached into -- shows up here.  The tests read the
manifest `provision` writes through `RHTN_WAN_MANIFEST`:

    RHTN_WAN_MANIFEST=<out>/state/manifest.json \\
        cargo test -p rhtn-sim --test ceremony --test subnets -- --nocapture

and the sibling admission, which wants a freshly provisioned fabric
because it adopts a member no node has met:

    RHTN_WAN_MANIFEST=<out>/state/manifest.json \\
        cargo test -p rhtn-daemon --test fabric a_sibling -- --nocapture

Credential: a DigitalOcean token the operator made at the provider's own
console, in a file named by --token-file or $RHTN_DO_TOKEN_FILE.  Nothing
here is registered to the project (no shipped provider secrets).

The output directory defaults to ../FuerosNet-output/wan beside the
repository: keys, known hosts, addresses, state and logs, none of it for
the repository.
"""
import argparse
import json
import os
import subprocess
import sys
import time
import urllib.error
import urllib.request

HERE = os.path.dirname(os.path.abspath(__file__))
CRATES = os.path.abspath(os.path.join(HERE, "..", ".."))
REPO = os.path.abspath(os.path.join(CRATES, ".."))
OUT_DEFAULT = os.path.join(REPO, "..", "FuerosNet-output", "wan")
API = "https://api.digitalocean.com"
TAG = "rhtn-wan"
QUIC = 7447
ADMIN = 7449
IMAGE_DEFAULT = "docker.io/sarcina/rhtnd:latest"

# the infra nodes of `rhtn_sim::subnets::wan_plans()`, each in the first
# region of its list that is up and offers the size (a size is not offered
# everywhere: fra1 has none of the small ones), and no two in one region,
# so every referral and every payload leg crosses a WAN
REGIONS = {
    "alice2": ["nyc1", "nyc3", "tor1"],
    "bob": ["sfo3", "sfo2"],
    "c1": ["ams3", "lon1"],
    "c2": ["sgp1", "blr1", "syd1"],
    "c3": ["fra1", "lon1", "blr1", "tor1"],
    "c6": ["tor1", "blr1", "syd1", "nyc3"],
}


def say(*a):
    print("wan.py:", *a, file=sys.stderr, flush=True)


class Provider:
    def __init__(self, token):
        self.token = token

    def ask(self, method, path, body=None):
        data = json.dumps(body).encode() if body is not None else None
        req = urllib.request.Request(API + path, data=data, method=method)
        req.add_header("Authorization", "Bearer " + self.token)
        req.add_header("Content-Type", "application/json")
        try:
            with urllib.request.urlopen(req, timeout=60) as r:
                raw = r.read()
                return json.loads(raw) if raw else {}
        except urllib.error.HTTPError as e:
            detail = e.read().decode(errors="replace")
            raise SystemExit(f"wan.py: {method} {path}: {e.code} {detail[:400]}")

    def newest_debian(self):
        v = self.ask("GET", "/v2/images?type=distribution&per_page=200")
        slugs = [
            i["slug"]
            for i in v.get("images", [])
            if i.get("distribution") == "Debian" and str(i.get("slug", "")).endswith("-x64")
        ]
        if not slugs:
            raise SystemExit("wan.py: the token lists no Debian image (it needs image: read)")
        return sorted(slugs)[-1]

    def ssh_key_id(self, public_line):
        fingerprint = subprocess.run(
            ["ssh-keygen", "-l", "-E", "md5", "-f", "/dev/stdin"],
            input=public_line.encode(),
            capture_output=True,
            check=True,
        ).stdout.decode().split()[1].removeprefix("MD5:")
        for k in self.ask("GET", "/v2/account/keys?per_page=200").get("ssh_keys", []):
            if k.get("fingerprint") == fingerprint:
                return k["id"]
        made = self.ask("POST", "/v2/account/keys", {"name": TAG, "public_key": public_line})
        return made["ssh_key"]["id"]

    def droplets(self):
        return self.ask("GET", f"/v2/droplets?tag_name={TAG}&per_page=200").get("droplets", [])

    def regions_offering(self, size):
        live = {
            r["slug"]
            for r in self.ask("GET", "/v2/regions?per_page=200").get("regions", [])
            if r.get("available")
        }
        for s in self.ask("GET", "/v2/sizes?per_page=200").get("sizes", []):
            if s["slug"] == size:
                return live & set(s.get("regions", []))
        raise SystemExit(f"wan.py: the provider lists no size {size}")


def ssh_key(out):
    priv = os.path.join(out, "id_wan")
    if not os.path.exists(priv):
        subprocess.run(
            ["ssh-keygen", "-t", "ed25519", "-N", "", "-C", TAG, "-f", priv],
            check=True,
            capture_output=True,
        )
        say("made an ssh key for the machines at", priv)
    with open(priv + ".pub") as f:
        return priv, f.read().strip()


def ssh_opts(out):
    return [
        "-i", os.path.join(out, "id_wan"),
        "-o", "StrictHostKeyChecking=accept-new",
        "-o", "UserKnownHostsFile=" + os.path.join(out, "known_hosts"),
        "-o", "ConnectTimeout=15",
        "-o", "BatchMode=yes",
    ]


def ssh(out, ip, *cmd, check=True, capture=False):
    return subprocess.run(
        ["ssh", *ssh_opts(out), f"root@{ip}", *cmd],
        check=check,
        capture_output=capture,
        text=capture,
    )


def addresses_path(out):
    return os.path.join(out, "addresses.json")


def read_addresses(out):
    p = addresses_path(out)
    if not os.path.exists(p):
        raise SystemExit(f"wan.py: no {p}: run `up` first")
    with open(p) as f:
        return json.load(f)


USER_DATA = """#cloud-config
package_update: true
packages:
  - podman
ssh_pwauth: false
runcmd:
  - mkdir -p /var/lib/rhtn
"""


def cmd_up(a, p):
    os.makedirs(a.out, exist_ok=True)
    priv, pub = ssh_key(a.out)
    key_id = p.ssh_key_id(pub)
    image = p.newest_debian()
    have = {d["name"]: d for d in p.droplets()}
    offering = p.regions_offering(a.size)
    regions, taken = {}, set()
    for name, prefs in REGIONS.items():
        machine = f"{TAG}-{name}"
        if machine in have:
            regions[name] = have[machine]["region"]["slug"]
        else:
            regions[name] = next((r for r in prefs if r in offering and r not in taken), None)
            if regions[name] is None:
                raise SystemExit(f"wan.py: none of {prefs} is up and offers {a.size}: {sorted(offering)}")
        taken.add(regions[name])
    ids = {}
    for name, region in regions.items():
        machine = f"{TAG}-{name}"
        if machine in have:
            say(f"{machine} exists already ({have[machine]['id']}, {region})")
            ids[name] = have[machine]["id"]
            continue
        body = {
            "name": machine,
            "region": region,
            "size": a.size,
            "image": image,
            "ssh_keys": [key_id],
            "ipv6": False,
            "monitoring": False,
            "tags": [TAG],
            "user_data": USER_DATA,
        }
        made = p.ask("POST", "/v2/droplets", body)
        ids[name] = made["droplet"]["id"]
        say(f"{machine}: droplet {ids[name]} in {region} ({a.size}, {image})")
    addresses = {}
    deadline = time.time() + 600
    while len(addresses) < len(ids) and time.time() < deadline:
        for name, did in ids.items():
            if name in addresses:
                continue
            d = p.ask("GET", f"/v2/droplets/{did}")["droplet"]
            if d.get("status") != "active":
                continue
            v4 = [n["ip_address"] for n in d["networks"]["v4"] if n["type"] == "public"]
            if v4:
                addresses[name] = v4[0]
                say(f"{TAG}-{name}: {v4[0]}")
        if len(addresses) < len(ids):
            time.sleep(5)
    if len(addresses) < len(ids):
        raise SystemExit("wan.py: not every machine came up within ten minutes")
    with open(addresses_path(a.out), "w") as f:
        json.dump({"droplets": ids, "addresses": addresses, "regions": regions}, f, indent=2)
    say("waiting for cloud-init on each machine (podman installs)")
    for name, ip in addresses.items():
        for attempt in range(40):
            r = ssh(a.out, ip, "cloud-init", "status", "--wait", check=False, capture=True)
            if r.returncode == 0:
                say(f"{name}: ready")
                break
            time.sleep(10)
        else:
            raise SystemExit(f"wan.py: {name} at {ip} never answered ssh")
    say("wrote", addresses_path(a.out))


def cmd_provision(a, p):
    held = read_addresses(a.out)
    state = os.path.join(a.out, "state")
    if os.path.isdir(state):
        import shutil

        shutil.rmtree(state)
    pairs = [f"{n}={ip}:{QUIC}" for n, ip in held["addresses"].items()]
    say("signing the fabric against", ", ".join(pairs))
    # --host-network: the node sees the machine's interfaces and binds the
    # address its record names; otherwise it binds the wildcard inside its
    # container and the ports are mapped, as a deployment would
    bind = ["--listen-own"] if a.host_network else []
    net = ["--network=host"] if a.host_network else [f"-p {QUIC}:{QUIC}/udp -p {ADMIN}:{ADMIN}/tcp"]
    subprocess.run(
        [
            "cargo", "run", "-q", "-p", "rhtn-sim", "--example", "provision", "--",
            "--out", state, "--root", "/var/lib/rhtn", *bind, *pairs,
        ],
        cwd=CRATES,
        check=True,
    )
    for name, ip in held["addresses"].items():
        src = os.path.join(state, name)
        say(f"{name}: copying state to {ip}")
        ssh(a.out, ip, "podman", "rm", "-f", "rhtnd", check=False, capture=True)
        ssh(a.out, ip, "rm", "-rf", "/var/lib/rhtn", "&&", "mkdir", "-p", "/var/lib/rhtn")
        subprocess.run(
            ["scp", "-q", "-r", *ssh_opts(a.out), src + "/.", f"root@{ip}:/var/lib/rhtn/"],
            check=True,
        )
        ssh(
            a.out, ip,
            "chown -R 10001:10001 /var/lib/rhtn && chmod 0700 /var/lib/rhtn &&",
            "podman run -d --name rhtnd --restart=always",
            *net,
            "-v /var/lib/rhtn:/var/lib/rhtn:Z", a.image,
        )
    say("started; `status` shows what each node says")
    say("manifest:", os.path.join(state, "manifest.json"))


def cmd_status(a, p):
    held = read_addresses(a.out)
    for name, ip in held["addresses"].items():
        r = ssh(a.out, ip, "podman", "logs", "--tail", "3", "rhtnd", check=False, capture=True)
        print(f"== {name} {ip} ({held['regions'].get(name, '?')})")
        print((r.stdout + r.stderr).strip() or f"(exit {r.returncode})")


def cmd_logs(a, p):
    held = read_addresses(a.out)
    ip = held["addresses"].get(a.name)
    if not ip:
        raise SystemExit(f"wan.py: no machine for {a.name}")
    ssh(a.out, ip, "podman", "logs", "rhtnd", check=False)


def cmd_down(a, p):
    ds = p.droplets()
    if not ds:
        say("nothing tagged", TAG)
        return
    for d in ds:
        say(f"destroying {d['name']} ({d['id']})")
    p.ask("DELETE", f"/v2/droplets?tag_name={TAG}")
    try:
        os.remove(addresses_path(a.out))
    except FileNotFoundError:
        pass


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--token-file", default=os.environ.get("RHTN_DO_TOKEN_FILE"))
    ap.add_argument("--out", default=os.path.abspath(OUT_DEFAULT))
    sub = ap.add_subparsers(dest="cmd", required=True)
    up = sub.add_parser("up")
    up.add_argument("--size", default="s-1vcpu-1gb")
    up.set_defaults(run=cmd_up)
    pr = sub.add_parser("provision")
    pr.add_argument("--image", default=IMAGE_DEFAULT)
    pr.add_argument("--host-network", action="store_true")
    pr.set_defaults(run=cmd_provision)
    sub.add_parser("status").set_defaults(run=cmd_status)
    lg = sub.add_parser("logs")
    lg.add_argument("name")
    lg.set_defaults(run=cmd_logs)
    sub.add_parser("down").set_defaults(run=cmd_down)
    a = ap.parse_args()
    if not a.token_file:
        raise SystemExit("wan.py: --token-file or $RHTN_DO_TOKEN_FILE names the provider token")
    with open(a.token_file) as f:
        token = f.read().strip()
    a.run(a, Provider(token))


if __name__ == "__main__":
    main()
