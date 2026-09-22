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
    rhtn_crypto::identity::testkit::test_identity(name)
        .public
        .keyhash
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
    assert_eq!(
        set.get("carol").must(&format!("attach {node} {addr}")),
        vec![attached.clone()]
    );
    assert_eq!(
        set.get("alice")
            .must(&format!("attach {node} {addr} {}", hex(&kh("carol")))),
        vec![attached.clone()]
    );
    assert_eq!(
        set.get("carol")
            .must(&format!("attach {node} {addr} {}", hex(&kh("alice")))),
        [attached]
    );
    assert_eq!(set.get("alice").must("attached"), ["attached true"]);

    // and what alice sends comes out of carol's events, decrypted, having
    // crossed two sockets and a process that never held the plaintext
    let body = b"the chain, end to end";
    assert_eq!(
        set.get("alice")
            .must(&format!("send {} 0 {}", hex(&kh("carol")), hex(body))),
        ["sent"]
    );
    let want = format!("payload from={} bytes={}", hex(&kh("alice")), hex(body));
    let mut got = Vec::new();
    for _ in 0..300 {
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
    assert!(
        p.must("channel").is_empty(),
        "an undeclared instrument claims no channel"
    );

    assert_eq!(p.must("channel latency pass 30"), ["channel latency pass"]);
    assert_eq!(p.must("channel optical fail"), ["channel optical fail"]);
    assert_eq!(
        p.must("channel"),
        ["channel optical fail", "channel latency pass 30"],
        "strongest first, and neither promoted"
    );
    assert_eq!(p.must("channel optical none"), ["channel optical none"]);
    assert_eq!(
        p.must("channel"),
        ["channel latency pass 30"],
        "a cleared channel is unsupported again"
    );

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
        std::process::Command::new(env!("CARGO_BIN_EXE_rhtnp"))
            .arg(identity)
            .stdin(std::process::Stdio::null())
            .output()
            .expect("runs")
    };
    // absent: a participant that generated one would run under an identity
    // nobody has met, and the person would not know
    let out = run(&dir.join("nothing.key"));
    assert!(!out.status.success());
    assert!(
        String::from_utf8_lossy(&out.stderr).contains("No such file"),
        "{:?}",
        String::from_utf8_lossy(&out.stderr)
    );

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
    // every claim is inside `ceremony`, which is the one implementation
    // of the steps: both devices name it the same, each nominee answers
    // its own nominator, the participants sign over the disclosures and
    // the witnesses without them, and all four name one record.
    let txid = ceremony(&mut set, &cast, "alice", "bob", "carol", "w1");
    assert_eq!(
        txid.len(),
        64,
        "a record is named by a 32-byte txid: {txid}"
    );
}

/// The one line beginning `prefix`, without it.
fn line(lines: Vec<String>, prefix: &str) -> String {
    lines
        .iter()
        .find_map(|l| l.strip_prefix(prefix))
        .unwrap_or_else(|| panic!("no `{}` in {lines:?}", prefix.trim()))
        .to_string()
}

// acceptance: PRT-05
#[test]
fn a_newly_minted_root_adopts_on_the_record_it_just_made() {
    let cast = ["alice", "bob", "carol", "w1"];
    let mut set = Participants::new(env!("CARGO_BIN_EXE_rhtnp"), "adoption");
    for n in cast {
        set.start(n, &cast);
    }
    let (a, b) = (hex(&kh("alice")), hex(&kh("bob")));

    // **alice has been configured as nothing and adopted by nobody.** Its
    // position is the self-anchor, which is a position like any other, and
    // is the whole of what it needs to be a patron.
    assert_eq!(
        set.get("alice").must("where"),
        [format!("anchor {a}")],
        "one subnet, its own"
    );
    assert!(
        set.get("alice").must(&format!("where {a}"))[0].starts_with("position "),
        "and it knows where it sits in it"
    );
    assert_eq!(
        set.get("alice").must(&format!("where {b}")),
        ["position none"],
        "it sits in nobody else's"
    );

    let pop = ceremony(&mut set, &cast, "alice", "bob", "carol", "w1");

    // the patron proposes under its own anchor, both sign the same body,
    // and each takes the record it signed
    let back = line(set.get("bob").must("back-pointers"), "back-pointers ");
    let body = line(
        set.get("alice")
            .must(&format!("adopt {a} {b} {pop} 1 {back}")),
        "adoption ",
    );
    let sig_b = line(set.get("bob").must(&format!("sign {body}")), "signed ");
    let sig_a = line(set.get("alice").must(&format!("sign {body}")), "signed ");
    let envelope = line(
        set.get("alice")
            .must(&format!("adoption-envelope {body} {b}:{sig_b},{a}:{sig_a}")),
        "envelope ",
    );
    let t_a = line(
        set.get("alice").must(&format!("take-adoption {envelope}")),
        "adopted ",
    );
    let t_b = line(
        set.get("bob").must(&format!("take-adoption {envelope}")),
        "adopted ",
    );
    assert_eq!(t_a, t_b, "one adoption, and both name it the same");

    // **and taking it is what tells the subordinate where it now sits.**
    // Nothing was configured on either side and nothing was granted: the
    // record says it.
    let anchors = set.get("bob").must("where");
    assert!(
        anchors.contains(&format!("anchor {a}")),
        "bob is in alice's subnet now: {anchors:?}"
    );
    assert!(
        anchors.contains(&format!("anchor {b}")),
        "and still in its own"
    );
    assert!(set.get("bob").must(&format!("where {a}"))[0].starts_with("position "));
}

/// One ceremony between alice and bob, witnessed by carol and w1, to the
/// record both hold.  The steps are PRT-04's; what this returns is what an
/// adoption is evidence of.
fn ceremony(
    set: &mut Participants,
    cast: &[&str],
    one: &str,
    two: &str,
    wa: &str,
    wb: &str,
) -> String {
    let (a, b) = (hex(&kh(one)), hex(&kh(two)));
    for n in [one, two] {
        set.get(n).must("channel latency pass 30");
    }
    for n in cast {
        set.get(n).must("answer yes");
    }
    let ia = line(
        set.get(one)
            .must(&format!("begin {b} {} initiator", hex(&kh(wa)))),
        "intent ",
    );
    let ib = line(
        set.get(two).must(&format!("begin {a} {}", hex(&kh(wb)))),
        "intent ",
    );
    let ca = line(set.get(two).must(&format!("intent {a} {ia}")), "ceremony ");
    let cb = line(set.get(one).must(&format!("intent {b} {ib}")), "ceremony ");
    assert_eq!(ca, cb, "both devices name the ceremony the same thing");
    let ch = line(set.get(one).must("proximity"), "channels ");
    set.get(two).must(&format!("take-channels {ch}"));
    let ka = line(set.get(one).must("capture-key"), "capture-key ");
    let kb = line(set.get(two).must("capture-key"), "capture-key ");
    set.get(two).must(&format!("capture {ka}"));
    set.get(one).must(&format!("capture {kb}"));
    let ask_a = line(set.get(one).must("witness-ask"), "witness-ask ");
    let ask_b = line(set.get(two).must("witness-ask"), "witness-ask ");
    let f_a = line(
        set.get(wa).must(&format!("take-witness-ask {ask_a}")),
        "witnessing ",
    );
    let f_b = line(
        set.get(wb).must(&format!("take-witness-ask {ask_b}")),
        "witnessing ",
    );
    let witnesses = format!("{}:{a}:{f_a},{}:{b}:{f_b}", hex(&kh(wa)), hex(&kh(wb)));
    let theirs = line(set.get(two).must("gathered"), "gathered ");
    let made = set.get(one).must(&format!("propose {theirs} {witnesses}"));
    let proposal = line(made.clone(), "proposed ");
    let disclosures = line(made, "disclosures ");
    let signers: Vec<String> = line(
        set.get(one).must(&format!("signers {proposal}")),
        "signers ",
    )
    .split(',')
    .map(str::to_string)
    .collect();
    let named: Vec<&str> = signers
        .iter()
        .map(|s| {
            cast.iter()
                .find(|n| hex(&kh(n)) == *s)
                .copied()
                .expect("a signer in the cast")
        })
        .collect();
    let back: Vec<String> = named
        .iter()
        .map(|n| line(set.get(n).must("back-pointers"), "back-pointers "))
        .collect();
    let back = back.join(";");
    let body = line(
        set.get(one).must(&format!("body {proposal} {back}")),
        "body ",
    );
    let mut entries = Vec::new();
    for (n, k) in named.iter().zip(&signers) {
        let signed = if *n == one || *n == two {
            line(
                set.get(n)
                    .must(&format!("review-and-sign {proposal} {disclosures} {back}")),
                "signed ",
            )
        } else {
            line(
                set.get(n).must(&format!("witness-sign {proposal} {back}")),
                "signed ",
            )
        };
        entries.push(format!("{k}:{signed}"));
    }
    let envelope = line(
        set.get(one)
            .must(&format!("envelope {body} {}", entries.join(","))),
        "envelope ",
    );
    let mut txids = Vec::new();
    for n in &named {
        let command = if *n == one || *n == two {
            format!("finalize {envelope} {disclosures}")
        } else {
            format!("finalize {envelope}")
        };
        txids.push(line(set.get(n).must(&command), "finalized "));
    }
    assert!(
        txids.windows(2).all(|w| w[0] == w[1]),
        "one record, and every signer names it the same: {txids:?}"
    );
    assert_eq!(
        txids.len(),
        4,
        "two participants and two witnesses signed it"
    );
    txids.remove(0)
}

// acceptance: PRT-06
#[test]
fn a_record_a_client_makes_reaches_the_node_that_serves_it() {
    // everybody pins everybody, the node included: a client cannot attach
    // to a node whose key it does not hold (`wire-format.md` §9.1)
    let all = ["bob", "alice", "carol", "w1", "w2"];
    let cast = ["alice", "carol", "w1", "w2"];
    let mut nodes = Daemons::new(rhtnd(), "originate");
    let addr = nodes.start("bob", &all, None);

    let mut set = Participants::new(env!("CARGO_BIN_EXE_rhtnp"), "originate");
    for n in cast {
        set.start(n, &all);
    }
    // the two parties are attached; the witnesses are not, and do not need
    // to be — a ceremony is between devices in each other's presence
    let node = hex(&kh("bob"));
    for n in ["alice", "carol"] {
        set.get(n).must(&format!("attach {node} {addr}"));
    }

    let pop = ceremony(&mut set, &cast, "alice", "carol", "w1", "w2");

    // **the adoption is what travels.** design §15 puts presence records
    // in the attestation class — *pull, not push*, stored by participants,
    // their patrons and witnesses — and the topology class is adoptions,
    // departures, disavowals, peerings and reissues (`wire-format.md`
    // §10.1). So the ceremony's own record stays with its signers and the
    // adoption made on it is what the node is offered.
    let (a, c) = (hex(&kh("alice")), hex(&kh("carol")));
    let back = line(set.get("carol").must("back-pointers"), "back-pointers ");
    let body = line(
        set.get("alice")
            .must(&format!("adopt {a} {c} {pop} 1 {back}")),
        "adoption ",
    );
    let sig_c = line(set.get("carol").must(&format!("sign {body}")), "signed ");
    let sig_a = line(set.get("alice").must(&format!("sign {body}")), "signed ");
    let envelope = line(
        set.get("alice")
            .must(&format!("adoption-envelope {body} {c}:{sig_c},{a}:{sig_a}")),
        "envelope ",
    );
    let txid = line(
        set.get("alice").must(&format!("take-adoption {envelope}")),
        "adopted ",
    );

    // the polls below are bounded at thirty seconds for the reason the
    // daemon scenarios' dials are: the gate fences every long job at
    // `nice -n 19`, and a process spawned from a test inherits it
    //
    // **it went up because the party that made it offered it, and came
    // back down because the node flooded it.** A client cannot flood; the
    // one node it is attached to can, and `wire-format.md` §10.1.1 already
    // counts an attached client an adjacency — which is the same edge read
    // in each direction. Carol learns of its own adoption from the flood,
    // not from having signed it.
    let mut landed = false;
    for _ in 0..300 {
        if line(set.get("carol").must(&format!("holds {txid}")), "holds ") == "true" {
            landed = true;
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
    assert!(landed, "the adoption came back down to the other client");
    assert_eq!(
        line(set.get("carol").must(&format!("distance {a}")), "distance "),
        "1",
        "and carol places its patron one edge away"
    );

    // and it is in what the process wrote when it stopped, not in a view
    nodes.stop("bob");
    let held = nodes.get("bob").topology().join("tx");
    let names: Vec<String> = std::fs::read_dir(&held)
        .expect("a topology store")
        .flatten()
        .map(|e| e.file_name().to_string_lossy().to_string())
        .collect();
    assert!(
        names.contains(&txid),
        "the node holds the transaction its client made: {names:?}"
    );
}
