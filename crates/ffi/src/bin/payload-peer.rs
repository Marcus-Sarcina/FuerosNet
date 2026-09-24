//! A workstation peer for driving the Android payload screen, behind the
//! `harness` feature a shell never builds: a serving node reachable off
//! the machine, a participant that echoes every payload it is sent, and
//! on stdout the provisioning the phone needs until the ceremony exists
//! to replace it.
//!
//!     cargo run -p rhtn-ffi --features harness --bin payload-peer -- <material-hex>
//!
//! **No seed crosses this boundary.** The phone mints its own identity and
//! shows its public half; that half is this driver's argument, the node
//! admits it, and the provision sent back names only public things -- the
//! node, where it serves, the peer, and the key material of each. A blob
//! carrying seeds would drive the screen just as well, and would be a
//! shape worth nobody's trouble to imitate.
//!
//! The node binds loopback by default, which an Android emulator reaches
//! at `10.0.2.2`; pass a second argument to bind an interface a separate
//! machine can route to.

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

fn unhex(s: &str) -> Option<Vec<u8>> {
    if !s.len().is_multiple_of(2) {
        return None;
    }
    (0..s.len() / 2)
        .map(|i| u8::from_str_radix(&s[2 * i..2 * i + 2], 16).ok())
        .collect()
}

fn main() {
    let Some(arg) = std::env::args().nth(1) else {
        eprintln!(
            "usage: payload-peer <phone-key-material-hex> [bind-addr]\n\n\
             An unprovisioned phone prints its key material to logcat under\n\
             the `fueros` tag:\n\n    \
             adb logcat -d -s fueros | grep material\n"
        );
        std::process::exit(2);
    };
    let Some(phone) = unhex(&arg) else {
        eprintln!("the first argument is the phone's key material in hex");
        std::process::exit(2);
    };
    let Some(phone_id) = rhtn_crypto::Identity::from_key_material(&phone) else {
        eprintln!("that hex is not a KeyMaterial array");
        std::process::exit(2);
    };
    // Loopback by default: an emulator reaches its host's loopback at
    // 10.0.2.2, so nothing need listen past this machine for the ordinary
    // case.  A second machine needs an interface it can route to, and has
    // to say so.
    let bind = std::env::args().nth(2).unwrap_or("127.0.0.1:0".into());

    let node = TestNode::start_on(
        "bob".into(),
        vec!["carol".into()],
        vec![phone.clone()],
        bind.clone(),
    );
    let port = node.address().rsplit(':').next().unwrap().to_string();
    let host = match bind.split(':').next() {
        Some("127.0.0.1" | "localhost" | "0.0.0.0") | None => "10.0.2.2".to_string(),
        Some(ip) => ip.to_string(),
    };
    let (bob, carol) = (test_identity("bob".into()), test_identity("carol".into()));

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
    let known = vec![bob.material.clone(), carol.material.clone(), phone.clone()];
    let me = Participant::start(carol.seeds.clone(), known, platform).expect("carol starts");
    me.attach(
        bob.id.clone(),
        vec![format!("127.0.0.1:{port}")],
        vec![phone_id.keyhash.to_vec()],
    )
    .expect("carol attaches");

    // Public in every field: two keyhashes, three key materials and an
    // address.  The phone keeps the only copy of its seeds.
    let provision = format!(
        r#"{{"known":["{}","{}","{}"],"node":"{}","addr":"{host}:{port}","peer":"{}","peer_name":"carol"}}"#,
        hex(&bob.material),
        hex(&carol.material),
        hex(&phone),
        hex(&bob.id),
        hex(&carol.id),
    );
    println!("PROVISION {provision}");
    eprintln!("node bob serves on {bind} (port {port}); carol echoes; ctrl-c ends it");

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
