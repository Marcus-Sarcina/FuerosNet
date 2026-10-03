//! A workstation harness for driving the Android payload screen, behind
//! the `harness` feature a shell never builds: a serving node reachable
//! off the machine, and on stdout the provisioning each phone needs until
//! the ceremony exists to replace it.
//!
//! **One phone**: the node plus a participant, carol, who echoes every
//! payload she is sent -- the phone talks to the workstation.
//!
//!     cargo run -p rhtn-ffi --features harness --bin payload-peer -- <material-hex>
//!
//! **Two phones**: the node alone, each phone provisioned with the other
//! as its peer -- payload between two phones, each a device of its own,
//! which is the milestone's own words. `PROVISION-A` goes to the phone
//! whose material came first, `PROVISION-B` to the other.
//!
//!     cargo run -p rhtn-ffi --features harness --bin payload-peer -- <material-a> <material-b>
//!
//! **No seed crosses this boundary.** A phone mints its own identity and
//! shows its public half; those halves are this driver's arguments, the
//! node admits them, and each provision sent back names only public
//! things -- the node, where it serves, the peer, and the key material of
//! each. A blob carrying seeds would drive the screen just as well, and
//! would be a shape worth nobody's trouble to imitate.
//!
//! The node binds loopback by default, which an Android emulator reaches
//! at `10.0.2.2`; pass a trailing bind address for an interface a
//! separate machine can route to.

use rhtn_ffi::client::Participant;
use rhtn_ffi::device::{
    Camera, Clock, Custody, Notices, Operator, Platform, Proximity, Random, Silent, Storage,
};
use rhtn_ffi::harness::{TestNode, test_identity};
use rhtn_ffi::types::{Ask, Channel, ChannelOutcome, Told};
use std::sync::{Arc, Mutex};

struct Shell {
    store: Mutex<std::collections::BTreeMap<String, Vec<u8>>>,
    custody: Mutex<Option<Vec<u8>>>,
}

// the key lives beside the store it opens because this peer is ephemeral
// by design; a shell with anything to lose keeps it apart
impl Custody for Shell {
    fn key(&self) -> Option<Vec<u8>> {
        self.custody.lock().unwrap().clone()
    }
    fn keep(&self, key: Vec<u8>) -> bool {
        *self.custody.lock().unwrap() = Some(key);
        true
    }
    fn unsealed(&self) -> bool {
        false
    }
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

/// A phone's key material, read off an argument: long hex that parses as
/// a `KeyMaterial` array. Anything else is taken for the bind address.
fn phone_of(arg: &str) -> Option<Vec<u8>> {
    let km = unhex(arg)?;
    rhtn_crypto::Identity::from_key_material(&km)?;
    Some(km)
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let phones: Vec<Vec<u8>> = args.iter().map_while(|a| phone_of(a)).collect();
    if phones.is_empty() || phones.len() > 2 || args.len() > phones.len() + 1 {
        eprintln!(
            "usage: payload-peer <phone-material-hex> [phone-b-material-hex] [bind-addr]\n\n\
             An unprovisioned phone prints its key material to logcat under\n\
             the `fueros` tag:\n\n    \
             adb logcat -d -s fueros | grep material\n\n\
             One material: the node plus carol, who echoes.  Two: the node\n\
             alone, each phone provisioned with the other as its peer.\n"
        );
        std::process::exit(2);
    }
    // Loopback by default: an emulator reaches its host's loopback at
    // 10.0.2.2, so nothing need listen past this machine for the ordinary
    // case.  A second machine needs an interface it can route to, and has
    // to say so.
    let bind = args
        .get(phones.len())
        .cloned()
        .unwrap_or("127.0.0.1:0".into());

    // carol exists only where there is one phone and nobody for it to
    // talk to; two phones are each other's peer
    let echoes = phones.len() == 1;
    let node = TestNode::start_on(
        "bob".into(),
        if echoes { vec!["carol".into()] } else { vec![] },
        phones.clone(),
        bind.clone(),
    );
    let port = node.address().rsplit(':').next().unwrap().to_string();
    let host = match bind.split(':').next() {
        Some("127.0.0.1" | "localhost" | "0.0.0.0") | None => "10.0.2.2".to_string(),
        Some(ip) => ip.to_string(),
    };
    let bob = test_identity("bob".into());

    if let [a, b] = phones.as_slice() {
        // both identities parsed in phone_of; the ids name the peers
        let (ida, idb) = (
            rhtn_crypto::Identity::from_key_material(a).expect("parsed once already"),
            rhtn_crypto::Identity::from_key_material(b).expect("parsed once already"),
        );
        for (label, own, other, other_id, other_name) in [
            ("PROVISION-A", a, b, &idb, "phone-b"),
            ("PROVISION-B", b, a, &ida, "phone-a"),
        ] {
            // public in every field, as below: what each phone needs is
            // the node, where it serves, and who its peer is
            let provision = format!(
                r#"{{"known":["{}","{}","{}"],"node":"{}","addr":"{host}:{port}","peer":"{}","peer_name":"{other_name}"}}"#,
                hex(&bob.material),
                hex(other),
                hex(own),
                hex(&bob.id),
                hex(&other_id.keyhash),
            );
            println!("{label} {provision}");
        }
        eprintln!(
            "node bob serves on {bind} (port {port}); two phones, each a \
             device of its own; ctrl-c ends it"
        );
        loop {
            std::thread::sleep(std::time::Duration::from_secs(3600));
        }
    }

    let phone = phones[0].clone();
    let phone_id = rhtn_crypto::Identity::from_key_material(&phone).expect("parsed once already");
    let carol = test_identity("carol".into());

    let s = Arc::new(Shell {
        store: Mutex::default(),
        custody: Mutex::default(),
    });
    let platform = Arc::new(Platform {
        proximity: s.clone(),
        camera: s.clone(),
        clock: s.clone(),
        random: s.clone(),
        operator: s.clone(),
        notices: s.clone(),
        storage: s.clone(),
        custody: s,
        diagnostics: Arc::new(Silent),
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
