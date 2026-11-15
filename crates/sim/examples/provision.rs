//! **Sign the WAN fabric against real addresses** [author, 2026-11-15]:
//! the state directory each infra node's `rhtnd` starts from, and the
//! manifest the tests read back.
//!
//!     provision --out DIR [--root /var/lib/rhtn] [--listen 0.0.0.0:7447] NAME=IP:PORT ...
//!
//! One `NAME=IP:PORT` per infra node of `rhtn_sim::subnets::wan_plans()`
//! (`alice2`, `bob`, `c1`, `c2`, `c3`), the address the node is reached
//! at from outside.  `--root` is where the directory will sit on the
//! machine that runs it, which the configuration's paths are written
//! against; `--listen` the bind.  `crates/sim/tools/wan.py provision`
//! drives this and copies the result up.

use rhtn_sim::subnets::{Fabric, wan_plans};
use std::collections::BTreeMap;
use std::net::SocketAddr;
use std::path::PathBuf;

fn main() {
    let mut args = std::env::args().skip(1);
    let mut out: Option<PathBuf> = None;
    let mut root = "/var/lib/rhtn".to_string();
    let mut listen = "0.0.0.0:7447".to_string();
    let mut addrs: BTreeMap<&'static str, SocketAddr> = BTreeMap::new();
    let plans = wan_plans();
    let names: Vec<&'static str> = plans
        .iter()
        .flat_map(|p| {
            std::iter::once(p.anchor).chain(p.members.iter().filter(|m| m.infra).map(|m| m.name))
        })
        .collect();
    while let Some(a) = args.next() {
        match a.as_str() {
            "--out" => out = args.next().map(PathBuf::from),
            "--root" => root = args.next().unwrap_or(root),
            "--listen" => listen = args.next().unwrap_or(listen),
            pair => {
                let Some((n, addr)) = pair.split_once('=') else {
                    usage(&format!("not NAME=IP:PORT: {pair}"));
                };
                let Some(name) = names.iter().find(|k| **k == n) else {
                    usage(&format!("{n} is not an infra node of the plan: {names:?}"));
                };
                let addr = addr
                    .parse()
                    .unwrap_or_else(|e| usage(&format!("{addr}: {e}")));
                addrs.insert(name, addr);
            }
        }
    }
    let Some(out) = out else {
        usage("--out is required")
    };
    for n in &names {
        if !addrs.contains_key(n) {
            usage(&format!("no address for {n}"));
        }
    }
    let p = Fabric::provision(
        plans,
        &addrs,
        // each serving node holds the other subnet's entry, as the
        // in-process tests introduce them
        &[("bob", 1), ("c3", 0)],
        &out,
        |_| root.clone(),
        |_| listen.clone(),
    );
    for (n, d) in &p.dirs {
        println!("{n}: {}", d.display());
    }
    println!("manifest: {}", p.manifest.display());
}

fn usage(why: &str) -> ! {
    eprintln!("provision: {why}");
    eprintln!(
        "usage: provision --out DIR [--root /var/lib/rhtn] [--listen 0.0.0.0:7447] NAME=IP:PORT ..."
    );
    std::process::exit(2)
}
