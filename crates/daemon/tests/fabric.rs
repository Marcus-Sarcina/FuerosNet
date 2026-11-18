//! **The provisioned fabric runs as `rhtnd` processes** [author,
//! 2026-11-15]: the state directories `Fabric::provision` writes are
//! what the daemon an operator runs starts from, and a client descends
//! the far subnet through three of them.  This is the local half of the
//! WAN harness (`crates/sim/tools/wan.py`): the same directories, copied
//! to rented machines in six regions, are the other half, and a claim
//! that fails here fails there for a cheaper reason.
//!
//! What the in-process fabric cannot show and this does: that the daemon
//! restores the table, the slots and its own position from a topology
//! store it never folded itself; that the referrals it makes come from
//! endpoint records it was given rather than sockets it holds; and that
//! the `anchors` directory seeds a serving node's table with another
//! subnet's entry, so it resolves on its client's behalf
//! (`wire-format.md` §7.7.1); and that a node bound to the wildcard, as a
//! containerised one is, refers with the address its operator-signed
//! record names and never with the bind (`infra-client-requirements.md`
//! §4.4), which the rented machines found first.

use rhtn_adaptors::beyond::{Beyond, Beyonder};
use rhtn_archive::submission::unrelayed;
use rhtn_client::horizon::{Askable, Reach, Upstream};
use rhtn_sim::subnets::{Fabric, kh, material, wan_plans};
use rhtn_transport::session::{AttachOutcome, NetworkPoint, Session};
use std::collections::BTreeMap;
use std::io::{BufRead, BufReader};
use std::net::SocketAddr;
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// A free loopback port, by binding and releasing it: the daemon binds
/// the address its configuration names, and the endpoint records were
/// signed for it before the daemon existed.
fn free_port() -> u16 {
    std::net::UdpSocket::bind("127.0.0.1:0")
        .expect("a socket")
        .local_addr()
        .expect("an address")
        .port()
}

struct Running {
    children: Vec<(String, Child)>,
}

impl Drop for Running {
    fn drop(&mut self) {
        for (_, c) in &mut self.children {
            let _ = c.kill();
            let _ = c.wait();
        }
    }
}

/// Start one daemon from its provisioned directory and wait for its
/// first line, which names where it serves.
fn start(name: &str, dir: &std::path::Path, said: &Arc<Mutex<Vec<String>>>) -> (SocketAddr, Child) {
    let mut child = Command::new(env!("CARGO_BIN_EXE_rhtnd"))
        .arg(dir.join("rhtnd.conf"))
        .arg(dir.join("peers"))
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("rhtnd starts");
    let mut out = BufReader::new(child.stdout.take().expect("stdout"));
    let err = child.stderr.take().expect("stderr");
    let (sink, who) = (said.clone(), name.to_string());
    std::thread::spawn(move || {
        for line in BufReader::new(err).lines().map_while(Result::ok) {
            sink.lock().unwrap().push(format!("{who}: {line}"));
        }
    });
    let mut line = String::new();
    out.read_line(&mut line).expect("a first line");
    if line.trim().is_empty() {
        let _ = child.wait();
        std::thread::sleep(Duration::from_millis(200));
        panic!("{name} did not start:\n{}", said.lock().unwrap().join("\n"));
    }
    std::thread::spawn(move || for _ in out.lines().map_while(Result::ok) {});
    let addr = line
        .trim()
        .rsplit_once(' ')
        .expect("an address at the end of the first line")
        .1
        .parse()
        .expect("an address");
    (addr, child)
}

fn beyond(f: &Fabric, name: &str) -> Arc<Beyond> {
    let counter = Arc::new(Mutex::new(0u8));
    let nonces = Arc::new(move || {
        let mut c = counter.lock().unwrap();
        *c += 1;
        [*c; 16]
    });
    Beyond::new(
        Beyonder::new(f.client_endpoint(), f.client_cfg(name)),
        nonces,
    )
}

fn reach_by_anchor(f: &Fabric, to: &str, ask: Vec<Askable>) -> Reach {
    let (anchor, path, nibbles) = f.subnet_of(to).position(to);
    Reach {
        upstream: Upstream::Position {
            anchor,
            path,
            nibbles,
        },
        ask,
    }
}

fn anchor_of(f: &Fabric, to: &str) -> Askable {
    let (node, endpoints, key_material) = f.subnet_of(to).askable();
    Askable {
        node,
        endpoints,
        key_material,
    }
}

// functional: RES-016
// functional: RES-017
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn the_provisioned_fabric_runs_as_daemons_and_a_client_descends_it() {
    let out = std::env::temp_dir().join(format!("rhtn-fabric-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&out);
    std::fs::create_dir_all(&out).expect("a directory");
    let plans = wan_plans();
    let infra: Vec<&'static str> = plans
        .iter()
        .flat_map(|p| {
            std::iter::once(p.anchor).chain(p.members.iter().filter(|m| m.infra).map(|m| m.name))
        })
        .collect();
    let addrs: BTreeMap<&'static str, SocketAddr> = infra
        .iter()
        .map(|n| (*n, SocketAddr::from(([127, 0, 0, 1], free_port()))))
        .collect();
    let p = Fabric::provision(
        plans,
        &addrs,
        &[("bob", 1), ("c3", 0)],
        &out,
        |n| out.join(n).display().to_string(),
        // the wildcard bind of a deployment: the record names where the
        // node is reached, the bind does not
        |n| format!("0.0.0.0:{}", addrs[n].port()),
    );
    // **a supplied file that does not read is named, not fatal**: the
    // table is a cache, and bob starts with c1's entry beside the stray
    std::fs::write(p.dirs["bob"].join("anchors").join("stray"), b"not an entry")
        .expect("a stray file");
    // the anchors first, then their children, as a deployment would:
    // a child's upstream attach is reported and not fatal either way
    let said = Arc::new(Mutex::new(Vec::new()));
    let mut running = Running {
        children: Vec::new(),
    };
    for n in ["alice2", "c1", "bob", "c2", "c3"] {
        let (addr, child) = start(n, &p.dirs[n], &said);
        assert!(
            addr.ip().is_unspecified() && addr.port() == addrs[n].port(),
            "{n} serves on the wildcard at its record's port: {addr}"
        );
        running.children.push((n.to_string(), child));
    }
    let named = |said: &Mutex<Vec<String>>| {
        said.lock()
            .unwrap()
            .iter()
            .any(|l| l.starts_with("bob: ") && l.contains("not an anchor entry this node takes"))
    };
    for _ in 0..40 {
        if named(&said) {
            break;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    assert!(
        named(&said),
        "bob named the stray file and started regardless:\n{}",
        said.lock().unwrap().join("\n")
    );
    let f = Fabric::remote(wan_plans(), &p.manifest);
    assert!(f.is_remote());
    assert_eq!(f.subnet_of("c4").addr_of("c3"), addrs["c3"]);

    // **the client drives the descent**: w1 holds nothing of the far
    // subnet but c1's entry, and c1, c2 and c3 answer from tables they
    // restored from disk
    let w1 = beyond(&f, "w1");
    assert!(
        w1.carry(
            kh("c4"),
            [0; 32],
            b"across three daemons".to_vec(),
            reach_by_anchor(&f, "c4", vec![anchor_of(&f, "c4")]),
            Some(kh("bob")),
        )
        .await,
        "c3 took it:\n{}",
        said.lock().unwrap().join("\n")
    );
    // **the serving node resolves on its client's behalf** from the
    // entry its `anchors` directory seeded: bob is asked, and the payload
    // still lands at c3
    let bob = Askable {
        node: kh("bob"),
        endpoints: vec![
            NetworkPoint::from_socket(addrs["bob"])
                .unwrap()
                .encode_bytes(),
        ],
        key_material: Some(material("bob")),
    };
    assert!(
        w1.carry(
            kh("c5"),
            [0; 32],
            b"by way of bob".to_vec(),
            reach_by_anchor(&f, "c5", vec![bob]),
            Some(kh("bob")),
        )
        .await,
        "bob resolved c5 for w1:\n{}",
        said.lock().unwrap().join("\n")
    );
    // both collected where c3 queued them
    for (who, expect) in [
        ("c4", b"across three daemons".to_vec()),
        ("c5", b"by way of bob".to_vec()),
    ] {
        let mut s = f.attach(who).await;
        let d = tokio::time::timeout(Duration::from_secs(5), s.deliveries.recv())
            .await
            .unwrap_or_else(|_| panic!("delivered to {who}"))
            .expect("bytes");
        let (from, what, _) = unrelayed(&d).expect("a relayed payload");
        assert_eq!((from, what), (kh("w1"), expect), "{who}'s delivery");
    }
    drop(running);
    let _ = std::fs::remove_dir_all(&out);
}

/// Attach `name` at `node` until it is admitted, for up to `secs`: the
/// session and how long the admission took to arrive.
async fn admitted(f: &Fabric, name: &str, node: &str, secs: u64) -> (Session, Duration) {
    let began = std::time::Instant::now();
    let mut last = None;
    while began.elapsed() < Duration::from_secs(secs) {
        match f.attach_at(name, node).await {
            AttachOutcome::Attached(s) => return (s, began.elapsed()),
            other => last = Some(other),
        }
        tokio::time::sleep(Duration::from_millis(500)).await;
    }
    panic!("{node} did not admit {name} within {secs}s: {last:?}");
}

// functional: SES-018
/// **A sibling admits a member adopted after it started** [author,
/// 2026-11-18]: no node of the far subnet is provisioned knowing `c7`.
/// `c3`'s operator pushes `c7`'s presence and adoption to its own node,
/// which carries `c7`'s key; `c3` passes them to its patron `c2`, and `c2`
/// to `c6`, `c3`'s sibling, which learns the key from a push within its
/// horizon and admits `c7` in failover (design §14.1.2 step 4,
/// `infra-client-requirements.md` §4.4).  Here as `rhtnd` processes, or on
/// the rented machines a manifest in `RHTN_WAN_MANIFEST` names, which must
/// be freshly provisioned: a used fabric knows `c7` already.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_sibling_admits_a_member_adopted_after_it_started() {
    let said = Arc::new(Mutex::new(Vec::new()));
    let mut running = Running {
        children: Vec::new(),
    };
    let out = std::env::temp_dir().join(format!("rhtn-sibling-{}", std::process::id()));
    let f = match std::env::var_os("RHTN_WAN_MANIFEST") {
        Some(m) => Fabric::remote(wan_plans(), std::path::Path::new(&m)),
        None => {
            let _ = std::fs::remove_dir_all(&out);
            std::fs::create_dir_all(&out).expect("a directory");
            let plans = wan_plans();
            let infra: Vec<&'static str> = plans
                .iter()
                .flat_map(|p| {
                    std::iter::once(p.anchor)
                        .chain(p.members.iter().filter(|m| m.infra).map(|m| m.name))
                })
                .collect();
            let addrs: BTreeMap<&'static str, SocketAddr> = infra
                .iter()
                .map(|n| (*n, SocketAddr::from(([127, 0, 0, 1], free_port()))))
                .collect();
            let p = Fabric::provision(
                plans,
                &addrs,
                &[],
                &out,
                |n| out.join(n).display().to_string(),
                |n| addrs[n].to_string(),
            );
            // the far subnet alone, patrons first
            for n in ["c1", "c2", "c3", "c6"] {
                let (_, child) = start(n, &p.dirs[n], &said);
                running.children.push((n.to_string(), child));
            }
            Fabric::remote(wan_plans(), &p.manifest)
        }
    };
    let log = || said.lock().unwrap().join("\n");

    // nothing knows c7 yet: its sibling-to-be refuses it
    let before = f.attach_at("c7", "c6").await;
    assert!(
        !matches!(before, AttachOutcome::Attached(_)),
        "c6 admitted c7 before any node was told of it: is the fabric fresh?"
    );

    let _operator = f.adopt_late("c7").await;
    let (own, took) = admitted(&f, "c7", "c3", 60).await;
    println!("c3 admitted c7 {took:?} after the push");
    assert_eq!(own.ack.mode, 0, "c3 serves c7 as its own");
    assert!(
        own.ack.siblings.iter().any(|s| s.keyhash == kh("c6")),
        "c3's ack names c6 as a sibling: {:?}\n{}",
        own.ack
            .siblings
            .iter()
            .map(|s| {
                s.keyhash[..4]
                    .iter()
                    .map(|b| format!("{b:02x}"))
                    .collect::<String>()
            })
            .collect::<Vec<_>>(),
        log()
    );
    let (sib, took) = admitted(&f, "c7", "c6", 60).await;
    println!("c6 admitted c7 {took:?} after c3 did");
    assert_eq!(
        sib.ack.mode, 1,
        "c6 admits its sibling's client in failover"
    );
    drop(running);
    let _ = std::fs::remove_dir_all(&out);
}
