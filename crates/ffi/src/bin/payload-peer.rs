//! A workstation peer for driving the Android payload screen, behind the
//! `harness` feature a shell never builds: a serving node reachable off
//! the machine, a participant that echoes every payload it is sent, and
//! on stdout the provisioning the phone needs until the ceremony exists
//! to replace it.
//!
//!     cargo run -p rhtn-ffi --features harness --bin payload-peer
//!
//! The printed provision names the node at `10.0.2.2`, which is how the
//! Android emulator reaches its host; pass a different host with the
//! first argument for a phone on the same network.

use rhtn_ffi::client::Participant;
use rhtn_ffi::device::{Camera, Clock, Notices, Operator, Platform, Proximity, Random, Storage};
use rhtn_ffi::harness::{TestNode, test_identity};
use rhtn_ffi::types::{Ask, Channel, ChannelOutcome, Told};
use std::sync::{Arc, Mutex};

struct Shell {
    store: Mutex<std::collections::BTreeMap<String, Vec<u8>>>,
}

impl Storage for Shell {
    fn read(&self, name: String) -> Option<Vec<u8>> {
        self.store.lock().unwrap().get(&name).cloned()
    }
    fn write(&self, name: String, bytes: Vec<u8>) -> bool {
        self.store.lock().unwrap().insert(name, bytes);
        true
    }
}
impl Proximity for Shell {
    fn supported(&self) -> Vec<Channel> {
        vec![]
    }
    fn run(&self, _channel: Channel, _peer: Vec<u8>) -> ChannelOutcome {
        ChannelOutcome::Unavailable
    }
    fn resolution_m(&self, _channel: Channel) -> Option<u64> {
        None
    }
}
impl Camera for Shell {
    fn capture(&self, _ask: Ask) -> Vec<u8> {
        vec![]
    }
}
impl Clock for Shell {
    fn now_ms(&self) -> u64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64
    }
    fn wait_ms(&self, ms: u64) {
        std::thread::sleep(std::time::Duration::from_millis(ms));
    }
}
impl Random for Shell {
    fn fill(&self, n: u32) -> Vec<u8> {
        let mut out = Vec::with_capacity(n as usize);
        while out.len() < n as usize {
            out.extend_from_slice(&rhtn_transport::tls::random_bytes::<32>());
        }
        out.truncate(n as usize);
        out
    }
}
impl Operator for Shell {
    fn ask(&self, _question: String) -> bool {
        false
    }
}
impl Notices for Shell {
    fn told(&self, notice: Told) {
        eprintln!("notice: {notice:?}");
    }
}

fn hex(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect()
}

fn main() {
    let host = std::env::args().nth(1).unwrap_or("10.0.2.2".into());
    let node = TestNode::start_on(
        "bob".into(),
        vec!["alice".into(), "carol".into()],
        "0.0.0.0:0".into(),
    );
    let port = node.address().rsplit(':').next().unwrap().to_string();
    let (alice, bob, carol) = (
        test_identity("alice".into()),
        test_identity("bob".into()),
        test_identity("carol".into()),
    );
    let known = vec![
        alice.material.clone(),
        bob.material.clone(),
        carol.material.clone(),
    ];

    let s = Arc::new(Shell {
        store: Mutex::default(),
    });
    let platform = Arc::new(Platform {
        proximity: s.clone(),
        camera: s.clone(),
        clock: s.clone(),
        random: s.clone(),
        operator: s.clone(),
        notices: s.clone(),
        storage: s,
    });
    let me =
        Participant::start(carol.seeds.clone(), known.clone(), platform).expect("carol starts");
    me.attach(
        bob.id.clone(),
        vec![format!("127.0.0.1:{port}")],
        vec![alice.id.clone()],
    )
    .expect("carol attaches");

    // What the phone needs, one JSON object on one line.
    let provision = format!(
        r#"{{"seeds":"{}","known":["{}","{}","{}"],"node":"{}","addr":"{host}:{port}","peer":"{}","peer_name":"carol"}}"#,
        hex(&alice.seeds),
        hex(&alice.material),
        hex(&bob.material),
        hex(&carol.material),
        hex(&bob.id),
        hex(&carol.id),
    );
    println!("PROVISION {provision}");
    eprintln!("node bob serves on 0.0.0.0:{port}; carol is attached and echoes; ctrl-c ends it");

    loop {
        match me.next_event(1000) {
            Some(rhtn_ffi::net::Event::Payload { from, bytes }) => {
                let text = String::from_utf8_lossy(&bytes).to_string();
                eprintln!("payload from {}: {text}", hex(&from));
                let mut reply = b"echo: ".to_vec();
                reply.extend_from_slice(&bytes);
                if let Err(e) = me.send(from, rhtn_ffi::types::kind_application(), reply) {
                    eprintln!("echo refused: {e:?}");
                }
            }
            Some(e) => eprintln!("event: {e:?}"),
            None => {}
        }
    }
}
