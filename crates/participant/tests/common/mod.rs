//! What the live scenarios share: the names, the daemon beside the
//! instrument, and a ceremony driven to its record across processes.
#![allow(dead_code)]

use rhtn_sim::daemons::hex;
use rhtn_sim::participants::Participants;
use std::path::PathBuf;

pub fn kh(name: &str) -> [u8; 32] {
    rhtn_crypto::identity::testkit::test_identity(name)
        .public
        .keyhash
}

/// The daemon beside us.  Cargo tells a test where its own package's
/// binaries are and nothing about another's; every workspace binary lands
/// in the same directory, so the one it does say is where to look.
pub fn rhtnd() -> PathBuf {
    let p = PathBuf::from(env!("CARGO_BIN_EXE_rhtnp"));
    p.parent().expect("a target directory").join("rhtnd")
}

/// The one line beginning `prefix`, without it.
pub fn line(lines: Vec<String>, prefix: &str) -> String {
    lines
        .iter()
        .find_map(|l| l.strip_prefix(prefix))
        .unwrap_or_else(|| panic!("no `{}` in {lines:?}", prefix.trim()))
        .to_string()
}

/// The value of the first `key=value` word in a line.
pub fn field(line: &str, key: &str) -> String {
    line.split_whitespace()
        .find_map(|w| w.strip_prefix(key))
        .unwrap_or_else(|| panic!("no `{key}` in {line:?}"))
        .to_string()
}

/// One ceremony between alice and bob, witnessed by carol and w1, to the
/// record both hold.  The steps are PRT-04's; what this returns is what an
/// adoption is evidence of.
pub fn ceremony(
    set: &mut Participants,
    cast: &[&str],
    one: &str,
    two: &str,
    wa: &str,
    wb: &str,
) -> String {
    let (a, b) = (hex(&kh(one)), hex(&kh(two)));
    for n in cast {
        set.get(n).must("answer yes");
        // **the witness's window off for a process test** [author,
        // 2026-10-21]: these are separate processes against a real clock,
        // and a witness that waits its window signs nothing inside one.
        // What the window does is asserted where a clock can be moved
        // (`rhtn-client`'s CER-52); a bench build carries thirty seconds
        set.get(n).must("window 0");
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
    let ka = line(set.get(one).must("capture-key"), "capture-key ");
    let kb = line(set.get(two).must("capture-key"), "capture-key ");
    set.get(two).must(&format!("capture {ka}"));
    set.get(one).must(&format!("capture {kb}"));
    // the engagements go with the intent [author, 2026-10-19]: each
    // nominee holds the opening before the ask that closes it arrives
    let ea = line(set.get(one).must("engagement"), "engagement ");
    let eb = line(set.get(two).must("engagement"), "engagement ");
    assert_eq!(
        set.get(wa).must(&format!("take-engagement {a} {ea}")),
        vec!["engaged".to_string()]
    );
    assert_eq!(
        set.get(wb).must(&format!("take-engagement {b} {eb}")),
        vec!["engaged".to_string()]
    );
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
pub fn sign_and_finalize(
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
