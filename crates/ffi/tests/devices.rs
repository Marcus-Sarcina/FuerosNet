//! **Another device of the identity, through the boundary** (`wire-format.md`
//! §14.3.3; APP-014, APP-017): the contributions cross as opaque bytes,
//! the pairing's id names the link, the sealed dataset crosses once each
//! holder has said yes, and nothing crosses on a no.

use rhtn_ffi::client::Participant;
use rhtn_ffi::device::*;
use rhtn_ffi::types::*;
use std::sync::{Arc, Mutex};

/// A device's platform: what its holder answers, and what was asked.
struct Shell {
    questions: Mutex<Vec<String>>,
    says_yes: bool,
    store: Mutex<std::collections::BTreeMap<String, Vec<u8>>>,
}

impl Shell {
    fn new(says_yes: bool) -> Arc<Shell> {
        Arc::new(Shell {
            questions: Mutex::new(Vec::new()),
            says_yes,
            store: Mutex::new(Default::default()),
        })
    }
}

impl Custody for Shell {
    fn key(&self) -> Option<Vec<u8>> {
        None
    }
    fn keep(&self, _key: Vec<u8>) -> bool {
        false
    }
    fn unsealed(&self) -> bool {
        true
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
impl Camera for Shell {
    fn capture(&self, _ask: Ask) -> Vec<u8> {
        vec![7u8; 64]
    }
    fn probe(&self) -> Vec<u8> {
        vec![7u8; 64]
    }
}
impl Clock for Shell {
    fn now_ms(&self) -> u64 {
        1_800_000_000_000
    }
    fn wait_ms(&self, _ms: u64) {}
}
impl Random for Shell {
    fn fill(&self, n: u32) -> Vec<u8> {
        let mut out = Vec::with_capacity(n as usize);
        while out.len() < n as usize {
            out.extend_from_slice(&rhtn_transport::tls::random_bytes::<64>());
        }
        out.truncate(n as usize);
        out
    }
}
impl Operator for Shell {
    fn ask(&self, question: String) -> bool {
        self.questions.lock().unwrap().push(question);
        self.says_yes
    }
}
impl Notices for Shell {
    fn told(&self, _: Told) {}
}

fn platform_of(shell: Arc<Shell>) -> Arc<Platform> {
    Arc::new(Platform {
        camera: shell.clone(),
        clock: shell.clone(),
        random: shell.clone(),
        operator: shell.clone(),
        notices: shell.clone(),
        storage: shell.clone(),
        custody: shell,
        diagnostics: Arc::new(Silent),
        engine: None,
    })
}

fn seeds(name: &str) -> Vec<u8> {
    let mut b =
        rhtn_codec::cose::sha256(format!("rhtn-test-vectors:{name}:ed25519-seed").as_bytes())
            .to_vec();
    b.extend_from_slice(&rhtn_codec::cose::sha256(
        format!("rhtn-test-vectors:{name}:ml-dsa-65-seed").as_bytes(),
    ));
    b
}

fn material(name: &str) -> Vec<u8> {
    rhtn_crypto::identity::testkit::test_identity(name)
        .public
        .key_material()
}

fn device(name: &str, says_yes: bool) -> (Participant, Arc<Shell>) {
    let shell = Shell::new(says_yes);
    let known = ["alice", "bob"].iter().map(|n| material(n)).collect();
    let p = Participant::start(seeds(name), known, platform_of(shell.clone())).unwrap();
    (p, shell)
}

// functional: APP-017
#[test]
fn two_devices_of_one_identity_pair_by_contribution_and_the_dataset_crosses_on_two_yeses() {
    let (phone, phone_shell) = device("alice", true);
    let (desk, desk_shell) = device("alice", true);
    let (bob, _) = device("bob", true);

    // the contributions, as each camera reads them: opaque here
    let mine = phone.dataset_contribution().unwrap();
    let theirs = desk.dataset_contribution().unwrap();
    assert!(
        desk.dataset_pairing_id().is_err(),
        "no pairing before the other device's contribution is in"
    );
    assert!(
        desk.take_dataset_contribution(bob.dataset_contribution().unwrap())
            .unwrap_err()
            .reason()
            .contains("NotMine"),
        "a device of another identity does not pair"
    );
    assert_eq!(
        desk.take_dataset_contribution(mine).unwrap().len(),
        0,
        "the phone's heads: none yet"
    );
    assert_eq!(phone.take_dataset_contribution(theirs).unwrap().len(), 0);
    let id = phone.dataset_pairing_id().unwrap();
    assert_eq!(id.len(), 32);
    assert_eq!(
        desk.dataset_pairing_id().unwrap(),
        id,
        "one pairing, named the same on both"
    );

    // the phone serves from a node, and the desk from another at an
    // address the phone has since moved from
    let (node, other) = (vec![0x11u8; 32], vec![0x22u8; 32]);
    phone
        .agree_to(node.clone(), vec!["203.0.113.7:7431".into()])
        .unwrap();
    desk.agree_to(other.clone(), vec!["198.51.100.2:7431".into()])
        .unwrap();
    desk.agree_to(node.clone(), vec!["198.51.100.9:7431".into()])
        .unwrap();

    // the hand-over, each holder asked on their own device
    let sealed = phone.dataset_carriage().unwrap();
    assert_eq!(
        phone_shell.questions.lock().unwrap().len(),
        1,
        "the phone's holder was asked"
    );
    let mut tampered = sealed.clone();
    let last = tampered.len() - 1;
    tampered[last] ^= 1;
    assert!(
        desk.take_dataset_carriage(tampered)
            .unwrap_err()
            .reason()
            .contains("Unsealable"),
        "a carriage that does not open under the pairing is refused"
    );
    // emptied so that what the take writes is seen: the sibling list is
    // written by every save, so its return is the save's
    desk_shell.store.lock().unwrap().clear();
    let took = desk.take_dataset_carriage(sealed).unwrap();
    assert!(
        desk_shell.store.lock().unwrap().contains_key("siblings"),
        "what was taken is written before the take returns, so a restart keeps it"
    );
    assert_eq!(
        desk_shell.questions.lock().unwrap().len(),
        2,
        "the desk's holder was asked each time"
    );
    let agreed: Vec<(Vec<u8>, Vec<String>)> = desk
        .agreed()
        .into_iter()
        .map(|a| (a.serving, a.addresses))
        .collect();
    assert_eq!(
        agreed,
        vec![
            (node, vec!["203.0.113.7:7431".to_string()]),
            (other, vec!["198.51.100.2:7431".to_string()]),
        ],
        "the phone's agreement taken over the desk's for its node, and the desk's own kept"
    );
    assert_eq!(took.refused, 0);
    assert_eq!(
        took.records, 0,
        "two fresh devices: nothing to append, and the take succeeded"
    );
}

// functional: APP-017
#[test]
fn a_holder_who_says_no_on_either_device_moves_nothing() {
    let (phone, _) = device("alice", true);
    let (desk, desk_shell) = device("alice", false);
    let mine = phone.dataset_contribution().unwrap();
    let theirs = desk.dataset_contribution().unwrap();
    desk.take_dataset_contribution(mine).unwrap();
    phone.take_dataset_contribution(theirs).unwrap();
    let sealed = phone.dataset_carriage().unwrap();
    let refused = desk.take_dataset_carriage(sealed).unwrap_err();
    assert!(
        refused.reason().contains("Declined"),
        "{}",
        refused.reason()
    );
    assert_eq!(desk_shell.questions.lock().unwrap().len(), 1);
    // and the other way: the holder of the handing device declines
    let (no_phone, no_shell) = device("alice", false);
    let (taker, _) = device("alice", true);
    let a = no_phone.dataset_contribution().unwrap();
    let b = taker.dataset_contribution().unwrap();
    no_phone.take_dataset_contribution(b).unwrap();
    taker.take_dataset_contribution(a).unwrap();
    assert!(
        no_phone
            .dataset_carriage()
            .unwrap_err()
            .reason()
            .contains("Declined")
    );
    assert_eq!(no_shell.questions.lock().unwrap().len(), 1);
}
