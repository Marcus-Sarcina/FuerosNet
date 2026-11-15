//! **The provisioned fabric runs as `rhtnd` processes** [author,
//! 2026-11-15]: the state directories `Fabric::provision` writes are
//! what the daemon an operator runs starts from, and a client descends
//! the far subnet through three of them.  This is the local half of the
//! WAN harness (`crates/sim/tools/wan.py`): the same directories, copied
//! to rented machines in five regions, are the other half, and a claim
//! that fails here fails there for a cheaper reason.
//!
//! What the in-process fabric cannot show and this does: that the daemon
//! restores the table, the slots and its own position from a topology
//! store it never folded itself; that the referrals it makes come from
//! endpoint records it was given rather than sockets it holds; and that
//! the `anchors` directory seeds a serving node's table with another
//! subnet's entry, so it resolves on its client's behalf
//! (`wire-format.md` §7.7.1).

use rhtn_adaptors::beyond::{Beyond, Beyonder};
use rhtn_archive::submission::unrelayed;
use rhtn_client::horizon::{Askable, Reach, Upstream};
use rhtn_sim::subnets::{Fabric, kh, material, wan_plans};
use rhtn_transport::session::NetworkPoint;
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
        |n| addrs[n].to_string(),
    );
    // the anchors first, then their children, as a deployment would:
    // a child's upstream attach is reported and not fatal either way
    let said = Arc::new(Mutex::new(Vec::new()));
    let mut running = Running {
        children: Vec::new(),
    };
    for n in ["alice2", "c1", "bob", "c2", "c3"] {
        let (addr, child) = start(n, &p.dirs[n], &said);
        assert_eq!(addr, addrs[n], "{n} serves where its record says");
        running.children.push((n.to_string(), child));
    }
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
