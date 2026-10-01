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
    // **degraded, by the node's own determination** (`wire-format.md`
    // §8.2): the daemon holds no adoption placing either client in its
    // subtree, so it names them in failover; payload flows all the same
    let attached = format!(
        "attached serving={} primary=false queued=0",
        hex(&kh("bob"))
    );
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

/// The value of the first `key=value` word in a line.
fn field(line: &str, key: &str) -> String {
    line.split_whitespace()
        .find_map(|w| w.strip_prefix(key))
        .unwrap_or_else(|| panic!("no `{key}` in {line:?}"))
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
    set.get(one)
        .must(&format!("begin {b} {} initiator", hex(&kh(wa))));
    set.get(two).must(&format!("begin {a} {}", hex(&kh(wb))));
    // THE OPTICAL EXCHANGE, as two people holding up two screens
    // (`wire-format.md` §14.3.1): each copies the other's QR across, and
    // nothing in this driver reads one.
    let oa = line(set.get(one).must("optical"), "optical ");
    let ob = line(set.get(two).must("optical"), "optical ");
    assert_eq!(
        line(
            set.get(one).must(&format!("take-optical {ob}")),
            "showed-by "
        ),
        b.to_string(),
        "the QR names who showed it"
    );
    set.get(two).must(&format!("take-optical {oa}"));
    let ta = line(set.get(one).must("transcript"), "transcript ");
    let tb = line(set.get(two).must("transcript"), "transcript ");
    let ca = line(
        set.get(one).must(&format!("take-transcript {tb}")),
        "ceremony ",
    );
    let cb = line(
        set.get(two).must(&format!("take-transcript {ta}")),
        "ceremony ",
    );
    assert_eq!(ca, cb, "both devices name the ceremony the same thing");
    // then the bearer's own load, which this driver carries as one line
    let ia = line(set.get(one).must("intent"), "intent ");
    let ib = line(set.get(two).must("intent"), "intent ");
    set.get(two).must(&format!("intent {a} {ia}"));
    set.get(one).must(&format!("intent {b} {ib}"));
    let pa = line(set.get(one).must("proximity"), "proximity ");
    set.get(two).must(&format!("take-proximity {pa}"));
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
    let (signers, txid) = sign_and_finalize(set, cast, one, two, &theirs, &witnesses);
    assert_eq!(
        signers.len(),
        4,
        "two participants and two witnesses signed it"
    );
    txid
}

/// From `one`'s proposal over the responses `two` gathered and the
/// witnesses' answers, drive the record to finalization: the participants
/// sign over the disclosures, the witnesses without them, and every
/// signer takes the finished record.  Returns the signers and the one
/// txid they all named.
fn sign_and_finalize(
    set: &mut Participants,
    cast: &[&str],
    one: &str,
    two: &str,
    theirs: &str,
    witnesses: &str,
) -> (Vec<String>, String) {
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
    (signers, txids.remove(0))
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

    // and it is in what the process wrote when it stopped, not in a view.
    // **By identifier** (`infra-client-requirements.md` §4.3
    // [author, 2026-09-23]): what the node wrote of a transaction it was no
    // party to is that it stored it and the table it produced
    nodes.stop("bob");
    let held = nodes.get("bob").topology().join("seen");
    let names: Vec<String> = std::fs::read_to_string(&held)
        .expect("a topology store")
        .lines()
        .filter_map(|l| l.split(' ').next().map(|s| s.to_string()))
        .filter(|s| !s.is_empty())
        .collect();
    assert!(
        names.contains(&txid),
        "the node holds the transaction its client made: {names:?}"
    );
}

// acceptance: PRT-07
#[test]
fn a_verifier_in_a_third_process_answers_and_the_record_carries_it() {
    let cast = ["alice", "bob", "carol", "w1", "w2"];
    let mut set = Participants::new(env!("CARGO_BIN_EXE_rhtnp"), "verifier");
    for n in cast {
        set.start(n, &cast);
    }
    // **bob can answer for alice the only way anybody comes to**: he has
    // met her, and holds her sealed captures to show for it.  Twice,
    // because the criterion is floor(n/2) of the records she hands over
    // (`wire-format.md` §5.2) — a bundle of one obliges no verifier and
    // the selection never runs.
    ceremony(&mut set, &cast, "alice", "bob", "w1", "w2");
    ceremony(&mut set, &cast, "alice", "bob", "w1", "w2");
    // the window is exclusive at the next ceremony's own start
    // (`wire-format.md` §5.3) and these clocks are seconds: the second
    // record must have finalized strictly before alice begins again
    std::thread::sleep(std::time::Duration::from_secs(2));

    // alice meets carol; intent through capture are PRT-04's steps.
    // carol's hardware, like everybody's, is what was declared: nothing,
    // until here
    set.get("carol").must("channel latency pass 30");
    let (a, b, c) = (hex(&kh("alice")), hex(&kh("bob")), hex(&kh("carol")));
    set.get("alice")
        .must(&format!("begin {c} {} initiator", hex(&kh("w1"))));
    set.get("carol")
        .must(&format!("begin {a} {}", hex(&kh("w2"))));
    let oa = line(set.get("alice").must("optical"), "optical ");
    let oc = line(set.get("carol").must("optical"), "optical ");
    set.get("alice").must(&format!("take-optical {oc}"));
    set.get("carol").must(&format!("take-optical {oa}"));
    let ta = line(set.get("alice").must("transcript"), "transcript ");
    let tc = line(set.get("carol").must("transcript"), "transcript ");
    assert_eq!(
        line(
            set.get("alice").must(&format!("take-transcript {tc}")),
            "ceremony "
        ),
        line(
            set.get("carol").must(&format!("take-transcript {ta}")),
            "ceremony "
        ),
    );
    let ia = line(set.get("alice").must("intent"), "intent ");
    let ic = line(set.get("carol").must("intent"), "intent ");
    set.get("carol").must(&format!("intent {a} {ia}"));
    set.get("alice").must(&format!("intent {c} {ic}"));
    let pa = line(set.get("alice").must("proximity"), "proximity ");
    set.get("carol").must(&format!("take-proximity {pa}"));
    let ka = line(set.get("alice").must("capture-key"), "capture-key ");
    let kc = line(set.get("carol").must("capture-key"), "capture-key ");
    set.get("carol").must(&format!("capture {ka}"));
    set.get("alice").must(&format!("capture {kc}"));

    // **the pool is what alice handed over, not configuration**
    // (`wire-format.md` §5.4): two records, one distinct prior
    // counterparty, so carol must seek one verifier and the one candidate
    // is bob — a stranger to her, taken at her discretion and marked so,
    // and her person is told nobody in the pool is known to her
    assert_eq!(
        set.get("carol").must("verifiers"),
        [
            format!("verifier {b} basis=3"),
            "told no-candidate-recognised".to_string(),
        ],
    );
    // carol, meanwhile, hands over nothing, and obliges alice to nothing
    assert!(
        set.get("alice").must("verifiers").is_empty(),
        "no bundle, no verifier owed"
    );

    // the query about the person in front of carol; alice's consent over
    // its id, and with the consent the one grant, minted for bob and
    // carried to him and to nobody else (design §7.5.2)
    let q = line(set.get("carol").must(&format!("query {b}")), "query ");
    let consented = set.get("alice").must(&format!("consent {q}"));
    let consent = line(consented.clone(), "consent ");
    let grant = line(consented, "grant ");
    let req = line(
        set.get("carol").must(&format!("request {q} {consent} 3")),
        "request ",
    );

    // **the request reaches bob before its key does, and bob fabricates
    // nothing for it**: no signed response exists until the grant arrives
    assert_eq!(
        set.get("bob").must(&format!("take-query {c} {req}")),
        ["awaiting-grant"]
    );
    // the grant opens bob's sealed capture of alice, and the buffered
    // query is answered on the spot.  What the match attests is the key
    // release and the carried legs, not a face: the reference engine
    // compares hashes and recognises nobody (design §22.2 leaves the
    // engine open).
    let answered = set.get("bob").must(&format!("take-grant {a} {grant}"));
    let response = field(&answered[0], "querier=");
    assert!(
        field(&answered[0], "subject=").starts_with(&format!("{a}:")),
        "the copy is addressed to the subject herself (design §7.4.2)"
    );

    // carol holds bob's answer under his signature, for the body
    set.get("carol").must(&format!("take-response {response}"));
    assert_eq!(
        set.get("carol").must("responses"),
        [format!("response verifier={b} subject={a} answer=Match")],
    );

    // witnesses, proposal, signatures, finalization: PRT-04's close, with
    // the one difference that is the point — what carol gathered is no
    // longer empty, and the record is proposed over it
    let ask_a = line(set.get("alice").must("witness-ask"), "witness-ask ");
    let ask_c = line(set.get("carol").must("witness-ask"), "witness-ask ");
    let f_a = line(
        set.get("w1").must(&format!("take-witness-ask {ask_a}")),
        "witnessing ",
    );
    let f_c = line(
        set.get("w2").must(&format!("take-witness-ask {ask_c}")),
        "witnessing ",
    );
    let witnesses = format!("{}:{a}:{f_a},{}:{c}:{f_c}", hex(&kh("w1")), hex(&kh("w2")));
    let theirs = line(set.get("carol").must("gathered"), "gathered ");
    assert_ne!(theirs, "-", "the response is in what carol carries over");
    let (signers, txid) = sign_and_finalize(&mut set, &cast, "alice", "carol", &theirs, &witnesses);
    assert_eq!(txid.len(), 64, "a record is named by a 32-byte txid");
    // the verifier answered and signs nothing: bob is no party to this
    // meeting, and the record he served is not his to hold
    assert!(!signers.contains(&b), "the verifier is not a signer");
    assert_eq!(signers.len(), 4, "two participants and two witnesses");
}
