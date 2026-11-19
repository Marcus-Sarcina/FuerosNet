//! What the daemon tests that start a real `rhtnd` on a named port share.
#![allow(dead_code)]

use std::collections::BTreeSet;
use std::hash::{BuildHasher, Hasher};
use std::net::{TcpListener, UdpSocket};
use std::sync::Mutex;

/// The ports this process has handed out, so a test that names two never
/// names one twice.
static GIVEN: Mutex<BTreeSet<u16>> = Mutex::new(BTreeSet::new());

/// **A port for one daemon to bind, where nothing else in the suite is
/// handed it.**
///
/// A test that delivers an endpoint record has to name the port before the
/// daemon binds it: the record is signed over the port, so port 0 is not
/// an option, and a daemon being enrolled binds only once its run arrives,
/// so the port lies unclaimed through the whole of the enrolment. The
/// tests used to name fixed ports inside the kernel's ephemeral range, and
/// in a 16-way run whatever else bound a socket on port 0 — a QUIC client,
/// another daemon's surface — could be handed one in that window; the
/// daemon then panicked at its bind (`AddrInUse`, exit 101) and the test
/// read only that it never served [2026-11-18]. **Below the range the
/// kernel hands out nothing of its own accord**, so only a test that names
/// a port can take it; the port is picked from a random start and checked
/// free over both UDP (QUIC) and TCP (the federation listener), so that
/// two tests, or two gates on one machine, do not name the same one.
pub fn unclaimed_port() -> u16 {
    let floor = ephemeral_floor();
    let lo: u16 = 10_000;
    let span = u64::from(floor.saturating_sub(lo).max(1));
    let start = std::collections::hash_map::RandomState::new()
        .build_hasher()
        .finish();
    let mut given = GIVEN.lock().unwrap();
    for i in 0..span {
        let port = lo + ((start.wrapping_add(i)) % span) as u16;
        if given.contains(&port) {
            continue;
        }
        let free = UdpSocket::bind(("0.0.0.0", port)).is_ok()
            && TcpListener::bind(("0.0.0.0", port)).is_ok();
        if free {
            given.insert(port);
            return port;
        }
    }
    panic!("no port free between {lo} and the ephemeral range at {floor}");
}

/// Where the kernel's ephemeral range starts, read rather than assumed;
/// Linux's default where it cannot be read.
fn ephemeral_floor() -> u16 {
    std::fs::read_to_string("/proc/sys/net/ipv4/ip_local_port_range")
        .ok()
        .and_then(|s| s.split_whitespace().next()?.parse().ok())
        .unwrap_or(32_768)
}
