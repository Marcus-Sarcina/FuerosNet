//! The boundary as a shell uses it (`light-client-requirements.md` §1.3):
//! the platform's objects handed over once, the client running on a thread
//! of its own behind them, and every answer a value.

use rhtn_ffi::client::{Intent, Participant};
use rhtn_ffi::device::*;
use rhtn_ffi::types::*;
use std::sync::{Arc, Mutex};

/// A shell's hardware: what it reports and what it was asked.
#[derive(Default)]
struct Shell {
    has: Vec<Channel>,
    asked: Mutex<Vec<Ask>>,
    told: Mutex<Vec<Told>>,
    questions: Mutex<Vec<String>>,
    /// Bytes the platform's randomness returns for each call, in order.
    short: bool,
}

impl Proximity for Shell {
    fn supported(&self) -> Vec<Channel> {
        self.has.clone()
    }
    fn run(&self, channel: Channel, _peer: Vec<u8>) -> ChannelOutcome {
        match channel {
            Channel::Nfc => ChannelOutcome::Pass,
            _ => ChannelOutcome::Fail,
        }
    }
    fn resolution_m(&self, channel: Channel) -> Option<u64> {
        match channel {
            Channel::Uwb => Some(2),
            _ => None,
        }
    }
}

impl Camera for Shell {
    fn capture(&self, ask: Ask) -> Vec<u8> {
        self.asked.lock().unwrap().push(ask);
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
        // a platform that returns short measure is refused, not padded
        vec![9u8; if self.short { (n as usize).saturating_sub(1) } else { n as usize }]
    }
}

impl Operator for Shell {
    fn ask(&self, question: String) -> bool {
        self.questions.lock().unwrap().push(question);
        true
    }
}

impl Notices for Shell {
    fn told(&self, notice: Told) {
        self.told.lock().unwrap().push(notice);
    }
}

fn platform_of(shell: Arc<Shell>) -> Platform {
    Platform {
        proximity: shell.clone(),
        camera: shell.clone(),
        clock: shell.clone(),
        random: shell.clone(),
        operator: shell.clone(),
        notices: shell,
    }
}

fn seeds(name: &str) -> Vec<u8> {
    let mut b = rhtn_codec::cose::sha256(format!("rhtn-test-vectors:{name}:ed25519-seed").as_bytes()).to_vec();
    b.extend_from_slice(&rhtn_codec::cose::sha256(format!("rhtn-test-vectors:{name}:ml-dsa-65-seed").as_bytes()));
    b
}

fn material(name: &str) -> Vec<u8> {
    rhtn_crypto::identity::testkit::test_identity(name).public.key_material()
}

fn id(name: &str) -> Vec<u8> {
    rhtn_crypto::identity::testkit::test_identity(name).public.keyhash.to_vec()
}

// acceptance: DMN-10
#[test]
fn a_shell_drives_the_client_through_the_boundary_and_gets_values_back() {
    let shell = Arc::new(Shell { has: vec![Channel::Nfc, Channel::Uwb], ..Default::default() });
    let known: Vec<Vec<u8>> = ["alice", "bob", "carol"].iter().map(|n| material(n)).collect();
    let p = Participant::start(seeds("alice"), known, platform_of(shell.clone())).expect("starts");
    assert_eq!(p.me(), id("alice"), "the identity it was given, derived on its own thread");

    // the ceremony opens: the intent crosses as fields, since no document
    // fixes an encoding for what two present devices tell each other
    let i: Intent = p.begin(id("bob"), vec![id("carol")], true).expect("begins");
    assert_eq!(i.contribution.len(), 16);
    assert_eq!(i.nominees, vec![id("carol")]);
    assert!(i.initiator);
    assert_eq!(i.started_at, 1_800_000_000, "the platform's clock, in seconds");

    // the hardware is asked through the boundary, strongest first, and
    // nothing is promoted: the channel that failed is reported failed
    let bob = Participant::start(seeds("bob"), ["alice", "bob", "carol"].iter().map(|n| material(n)).collect(), platform_of(Arc::new(Shell { has: vec![Channel::Nfc, Channel::Uwb], ..Default::default() }))).unwrap();
    let theirs = bob.begin(id("alice"), vec![id("carol")], false).expect("begins");
    p.take_intent(id("bob"), theirs).expect("takes the counterparty's intent");
    let achieved = p.proximity().expect("runs the channels");
    assert_eq!(achieved.iter().map(|a| a.channel).collect::<Vec<_>>(), vec![Channel::Uwb, Channel::Nfc], "strongest first");
    assert_eq!(achieved[0].outcome, ChannelOutcome::Fail);
    assert_eq!(achieved[0].resolution_m, Some(2), "what the hardware measured");
    assert_eq!(achieved[1].outcome, ChannelOutcome::Pass);

    // a refusal is a value carrying its reason, not a failure the shell
    // has to guess at
    let e = p.begin(vec![1, 2, 3], vec![], true).unwrap_err();
    assert!(e.reason.contains("32 bytes"), "{e}");
    let Err(e) = Participant::start(vec![0; 10], vec![], platform_of(shell)) else { panic!("ten bytes is not an identity") };
    assert!(e.reason.contains("64 bytes of seed"), "{e}");
}

// acceptance: DMN-10
#[test]
fn randomness_of_short_measure_is_refused_and_the_shell_is_told() {
    let shell = Arc::new(Shell { has: vec![Channel::Nfc], short: true, ..Default::default() });
    let known: Vec<Vec<u8>> = ["alice"].iter().map(|n| material(n)).collect();
    // the client seeds its payload material as it starts, so a platform
    // giving short measure is caught there: a seed completed with zeroes
    // is not random, and padding one silently is worse than refusing
    let Err(e) = Participant::start(seeds("alice"), known, platform_of(shell)) else {
        panic!("short measure was accepted")
    };
    // and it reaches the shell as a value, not as a process that vanished
    assert!(e.reason.contains("could not be built"), "{e}");
}
