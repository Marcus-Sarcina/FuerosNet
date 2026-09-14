//! Two participant processes, a daemon process, and a payload between them.
//!
//! **Nothing here is in process.** Both clients are `rhtnp` started from
//! an identity file; the node is `rhtnd` started from its own
//! configuration; every claim is made from what a process printed or from
//! what arrived at one. That is the whole point of the instrument: the
//! same chain has been tested inside one process for weeks, and what had
//! never been shown is that it survives being three of them.

use rhtn_sim::daemons::{Daemons, hex};
use rhtn_sim::participants::Participants;
use std::path::PathBuf;

const CAST: [&str; 3] = ["bob", "alice", "carol"];

fn kh(name: &str) -> [u8; 32] {
    rhtn_crypto::identity::testkit::test_identity(name).public.keyhash
}

/// The daemon beside us.  Cargo tells a test where its own package's
/// binaries are and nothing about another's; every workspace binary lands
/// in the same directory, so the one it does say is where to look.
fn rhtnd() -> PathBuf {
    let p = PathBuf::from(env!("CARGO_BIN_EXE_rhtnp"));
    p.parent().expect("a target directory").join("rhtnd")
}

// acceptance: PRT-01
#[test]
fn two_participant_processes_exchange_payload_through_a_daemon() {
    let mut nodes = Daemons::new(rhtnd(), "participants");
    let addr = nodes.start("bob", &CAST, None);

    let mut set = Participants::new(env!("CARGO_BIN_EXE_rhtnp"), "payload");
    set.start("alice", &CAST);
    set.start("carol", &CAST);

    // nothing is attached before an attach, and the node is named by
    // keyhash and reached at an address the harness was told, exactly as
    // a person would have been told one
    assert_eq!(set.get("alice").must("attached"), ["attached false"]);

    // **the order is the protocol's, not the harness's.**  An attach
    // publishes this client's bundle and sweeps the parties it names, so
    // carol must publish before alice can sweep it — a payload needs the
    // recipient's one-time material and there is nowhere else to get it —
    // and carol must then sweep alice in turn, because a recipient
    // attributes an initial message only under a binding it already holds
    // (`wire-format.md` §7.8, design §14.2.4).
    let attached = format!("attached serving={} primary=true queued=0", hex(&kh("bob")));
    let node = hex(&kh("bob"));
    assert_eq!(set.get("carol").must(&format!("attach {node} {addr}")), vec![attached.clone()]);
    assert_eq!(set.get("alice").must(&format!("attach {node} {addr} {}", hex(&kh("carol")))), vec![attached.clone()]);
    assert_eq!(set.get("carol").must(&format!("attach {node} {addr} {}", hex(&kh("alice")))), [attached]);
    assert_eq!(set.get("alice").must("attached"), ["attached true"]);

    // and what alice sends comes out of carol's events, decrypted, having
    // crossed two sockets and a process that never held the plaintext
    let body = b"the chain, end to end";
    assert_eq!(set.get("alice").must(&format!("send {} 0 {}", hex(&kh("carol")), hex(body))), ["sent"]);
    let want = format!("payload from={} bytes={}", hex(&kh("alice")), hex(body));
    let mut got = Vec::new();
    for _ in 0..50 {
        got = set.get("carol").must("events 200");
        if got.iter().any(|l| l.starts_with("payload ")) {
            break;
        }
    }
    assert!(got.contains(&want), "carol reads what alice sent: {got:?}");
}

// acceptance: PRT-02
#[test]
fn an_instrument_reports_what_it_was_told_the_hardware_did_and_nothing_more() {
    let mut set = Participants::new(env!("CARGO_BIN_EXE_rhtnp"), "channels");
    let p = set.start("alice", &CAST);

    // **nothing is supported until something says so.**  A machine with
    // no radio and no camera pointed at anybody has no proximity channel,
    // and `light-client-requirements.md` §1.3 forbids presenting a weaker
    // one as a stronger one — so the instrument starts by claiming none.
    assert!(p.must("channel").is_empty(), "an undeclared instrument claims no channel");

    assert_eq!(p.must("channel latency pass 30"), ["channel latency pass"]);
    assert_eq!(p.must("channel optical fail"), ["channel optical fail"]);
    assert_eq!(p.must("channel"), ["channel optical fail", "channel latency pass 30"], "strongest first, and neither promoted");
    assert_eq!(p.must("channel optical none"), ["channel optical none"]);
    assert_eq!(p.must("channel"), ["channel latency pass 30"], "a cleared channel is unsupported again");

    // the person's standing answer is declining, because every question a
    // client puts is whether to release something
    assert_eq!(p.must("answer"), ["answer no"]);
    assert_eq!(p.must("answer yes"), ["answer yes"]);
    assert_eq!(p.must("answer"), ["answer yes"]);

    // and a command it does not know is refused rather than ignored
    assert!(p.tell("channel sonar pass")[0].contains("not uwb, nfc, optical or latency"));
    assert!(p.tell("nonsense")[0].contains("is not a command"));
    assert!(p.tell("send zz 0 00")[0].contains("not hex"));
    assert!(p.tell("attach 00 1.2.3.4:1")[0].contains("and an identity is 32"));
}

#[test]
fn a_participant_reads_its_identity_and_never_mints_one() {
    let mut set = Participants::new(env!("CARGO_BIN_EXE_rhtnp"), "identity");
    let p = set.start("alice", &CAST);
    let dir = p.dir.clone();
    assert_eq!(p.must("me"), [format!("me {}", hex(&kh("alice")))]);

    let run = |identity: &std::path::Path| {
        std::process::Command::new(env!("CARGO_BIN_EXE_rhtnp")).arg(identity).stdin(std::process::Stdio::null()).output().expect("runs")
    };
    // absent: a participant that generated one would run under an identity
    // nobody has met, and the person would not know
    let out = run(&dir.join("nothing.key"));
    assert!(!out.status.success());
    assert!(String::from_utf8_lossy(&out.stderr).contains("No such file"), "{:?}", String::from_utf8_lossy(&out.stderr));

    // the wrong length, and readable beyond its owner: the same two rules
    // the daemon holds its own key to, for the same reason
    let short = dir.join("short.key");
    std::fs::write(&short, [0u8; 32]).unwrap();
    assert!(String::from_utf8_lossy(&run(&short).stderr).contains("32 bytes, not 64"));

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let open = dir.join("open.key");
        std::fs::write(&open, [0u8; 64]).unwrap();
        std::fs::set_permissions(&open, std::fs::Permissions::from_mode(0o644)).unwrap();
        assert!(String::from_utf8_lossy(&run(&open).stderr).contains("readable beyond its owner"));
    }
}
