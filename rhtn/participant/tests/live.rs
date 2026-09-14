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

// acceptance: PRT-04
#[test]
fn a_ceremony_runs_to_a_record_between_four_processes() {
    let cast = ["alice", "bob", "carol", "w1"];
    let mut set = Participants::new(env!("CARGO_BIN_EXE_rhtnp"), "ceremony");
    for n in cast {
        set.start(n, &cast);
    }
    let (a, b) = (hex(&kh("alice")), hex(&kh("bob")));

    // **the hardware is declared on both sides and they must agree.**  A
    // channel one device passed and the other did not is a disagreement
    // the record has to settle, so each side weighs the same pair of lists
    for n in ["alice", "bob"] {
        set.get(n).must("channel latency pass 30");
    }
    // and the person consents, which they have to be asked for and which
    // defaults to declining: every question a client puts is whether to
    // release something
    for n in cast {
        set.get(n).must("answer yes");
    }

    // 1. intents cross.  Each nominates a witness from the other's
    // neighbourhood; the harness copying a token between two processes is
    // what a screen and a camera would otherwise be
    let ia = one(set.get("alice").must(&format!("begin {b} {} initiator", hex(&kh("carol")))), "intent ");
    let ib = one(set.get("bob").must(&format!("begin {a} {}", hex(&kh("w1")))), "intent ");
    let ca = one(set.get("bob").must(&format!("intent {a} {ia}")), "ceremony ");
    let cb = one(set.get("alice").must(&format!("intent {b} {ib}")), "ceremony ");
    assert_eq!(ca, cb, "both devices name the ceremony the same thing");

    // 2. proximity, and the counterparty weighs what was achieved
    let ch = one(set.get("alice").must("proximity"), "channels ");
    set.get("bob").must(&format!("take-channels {ch}"));

    // 3. capture keys cross, then each captures the other under theirs.
    // A client holds no decryptable likeness: the key is discarded once
    // the capture is sealed (design §7.5.2).
    let ka = one(set.get("alice").must("capture-key"), "capture-key ");
    let kb = one(set.get("bob").must("capture-key"), "capture-key ");
    set.get("bob").must(&format!("capture {ka}"));
    set.get("alice").must(&format!("capture {kb}"));

    // 4. witnesses: each nominator asks its own nominee, and an acceptance
    // is what puts that witness on the record
    let ask_a = one(set.get("alice").must("witness-ask"), "witness-ask ");
    let ask_b = one(set.get("bob").must("witness-ask"), "witness-ask ");
    let f_a = one(set.get("carol").must(&format!("take-witness-ask {ask_a}")), "witnessing ");
    let f_b = one(set.get("w1").must(&format!("take-witness-ask {ask_b}")), "witnessing ");
    let witnesses = format!("{}:{a}:{f_a},{}:{b}:{f_b}", hex(&kh("carol")), hex(&kh("w1")));

    // 5. the proposal, from the counterparty's responses and the witnesses
    let theirs = one(set.get("bob").must("gathered"), "gathered ");
    let made = set.get("alice").must(&format!("propose {theirs} {witnesses}"));
    let proposal = one(made.clone(), "proposed ");
    let disclosures = one(made, "disclosures ");

    // 6. every signer's back-pointers, in signer order, then the body they
    // all sign over
    let signers: Vec<String> = one(set.get("alice").must(&format!("signers {proposal}")), "signers ").split(',').map(str::to_string).collect();
    let named: Vec<&str> = signers.iter().map(|s| cast.iter().find(|n| hex(&kh(n)) == *s).copied().expect("a signer in the cast")).collect();
    let back: Vec<String> = named.iter().map(|n| one(set.get(n).must("back-pointers"), "back-pointers ")).collect();
    let back = back.join(";");
    let body = one(set.get("alice").must(&format!("body {proposal} {back}")), "body ");

    // 7. each signs what it is: a participant reviews the disclosures it
    // is committing to, a witness never sees them
    let mut entries = Vec::new();
    for (n, k) in named.iter().zip(&signers) {
        let signed = if *n == "alice" || *n == "bob" {
            one(set.get(n).must(&format!("review-and-sign {proposal} {disclosures} {back}")), "signed ")
        } else {
            one(set.get(n).must(&format!("witness-sign {proposal} {back}")), "signed ")
        };
        entries.push(format!("{k}:{signed}"));
    }

    // 8. and every signer holds the finished record, the participants with
    // the disclosures and the witnesses without
    let envelope = one(set.get("alice").must(&format!("envelope {body} {}", entries.join(","))), "envelope ");
    let mut txids = Vec::new();
    for n in &named {
        let command = if *n == "alice" || *n == "bob" { format!("finalize {envelope} {disclosures}") } else { format!("finalize {envelope}") };
        txids.push(one(set.get(n).must(&command), "finalized "));
    }
    assert!(txids.windows(2).all(|w| w[0] == w[1]), "one record, and every signer names it the same: {txids:?}");
    assert_eq!(txids.len(), 4, "two participants and two witnesses signed it");
}

/// The one line beginning `prefix`, without it.
fn one(lines: Vec<String>, prefix: &str) -> String {
    lines
        .iter()
        .find_map(|l| l.strip_prefix(prefix))
        .unwrap_or_else(|| panic!("no `{}` in {lines:?}", prefix.trim()))
        .to_string()
}
