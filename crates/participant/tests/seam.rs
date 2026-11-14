//! **The seam scenario** (`Robot/restructure-plan.md`, step 0): the
//! bench run of 2026-11-12/14 as a test. Two nodes, each speaking as the
//! identity that launched it; one handset's two identities, a member in
//! both subnets; an invitation taken as a tab beside a held session;
//! messages both ways on each node; and a restart that attaches again
//! from what the kernel kept. Every process is a real one — `rhtnp` per
//! identity, `rhtnd` per node — and every claim is read off what a
//! process printed, so the calls are a shell's and nothing reaches into
//! a client.
//!
//! What it fails for when reverted: an agreement the shell kept rather
//! than the kernel (nothing survives the restart), a payload to the
//! patron handed to the node as its own material (the reply to the
//! operator never arrives), a tab replacing the session instead of
//! joining it (one attachment where two are held).

mod common;

use common::*;
use rhtn_sim::daemons::{Daemons, hex};
use rhtn_sim::participants::Participants;

const CAST: [&str; 5] = ["gxv", "gxv2", "mrk", "w1", "w2"];

/// The patron adopts the child on the record of their meeting, both sign
/// the one body, and each takes the record it signed; the txid names
/// the subnet.
fn adopt(set: &mut Participants, patron: &str, child: &str, presence: &str) -> String {
    let (p, c) = (hex(&kh(patron)), hex(&kh(child)));
    let back = line(set.get(child).must("back-pointers"), "back-pointers ");
    let body = line(
        set.get(patron)
            .must(&format!("adopt - {c} {presence} 1 {back}")),
        "adoption ",
    );
    let sig_c = line(set.get(child).must(&format!("sign {body}")), "signed ");
    let sig_p = line(set.get(patron).must(&format!("sign {body}")), "signed ");
    let envelope = line(
        set.get(patron)
            .must(&format!("adoption-envelope {body} {c}:{sig_c},{p}:{sig_p}")),
        "envelope ",
    );
    let t_p = line(
        set.get(patron).must(&format!("take-adoption {envelope}")),
        "adopted ",
    );
    let t_c = line(
        set.get(child).must(&format!("take-adoption {envelope}")),
        "adopted ",
    );
    assert_eq!(t_p, t_c, "one adoption, named the same by both");
    t_p
}

fn send(set: &mut Participants, from: &str, to: &str, body: &[u8]) {
    assert_eq!(
        set.get(from)
            .must(&format!("send {} 0 {}", hex(&kh(to)), hex(body))),
        ["sent"],
        "{from} sends to {to}"
    );
}

/// `at` reads what `from` sent, within a minute of polling.
fn arrives(set: &mut Participants, at: &str, from: &str, body: &[u8]) {
    let want = format!("payload from={} kind=0 bytes={}", hex(&kh(from)), hex(body));
    let mut seen = Vec::new();
    for _ in 0..300 {
        seen.extend(set.get(at).must("events 200"));
        if seen.contains(&want) {
            return;
        }
    }
    panic!("{at} never read what {from} sent; it saw {seen:?}");
}

fn attached_to(
    set: &mut Participants,
    who: &str,
    node: &str,
    addr: &str,
    sweep: &[&str],
) -> String {
    let pop: Vec<String> = sweep.iter().map(|n| hex(&kh(n))).collect();
    let said = set
        .get(who)
        .must(&format!("attach {} {addr} {}", hex(&kh(node)), pop.join(" ")).trim_end());
    let l = line(said, "attached ");
    assert_eq!(
        field(&l, "serving="),
        hex(&kh(node)),
        "{who} is served by {node}: {l}"
    );
    l
}

// acceptance: PRT-11
#[test]
fn two_subnets_on_two_nodes_an_invitation_taken_as_a_tab_and_a_restart() {
    let mut nodes = Daemons::new(rhtnd(), "seam");
    // each node speaks as the identity that launched it, which is the
    // shape a node launched from the Operator screen has
    let n1 = nodes.start("gxv", &CAST, None).to_string();
    let n2 = nodes.start("gxv2", &CAST, None).to_string();
    let (gxv, gxv2, mrk) = (hex(&kh("gxv")), hex(&kh("gxv2")), hex(&kh("mrk")));

    let mut set = Participants::new(env!("CARGO_BIN_EXE_rhtnp"), "seam");
    for n in CAST {
        set.start(n, &CAST);
    }

    // **the first subnet**: gxv adopts mrk on the record of their meeting,
    // serves from the node it launched (agree, then attach, as the
    // Operator screen does), and mrk attaches there as its first node
    let pop1 = ceremony(&mut set, &CAST, "gxv", "mrk", "w1", "w2");
    let s1 = adopt(&mut set, "gxv", "mrk", &pop1);
    set.get("gxv").must(&format!("agree {gxv} {n1}"));
    attached_to(&mut set, "gxv", "gxv", &n1, &[]);
    set.get("mrk").must(&format!("agree {gxv} {n1}"));
    attached_to(&mut set, "mrk", "gxv", &n1, &["gxv"]);
    // the order is the protocol's: gxv sweeps mrk once mrk has published
    attached_to(&mut set, "gxv", "gxv", &n1, &["mrk"]);

    // **the second subnet**, anchored at the handset's other identity:
    // gxv2 adopts mrk and serves from its own node
    let pop2 = ceremony(&mut set, &CAST, "gxv2", "mrk", "w1", "w2");
    let s2 = adopt(&mut set, "gxv2", "mrk", &pop2);
    assert_ne!(s1, s2, "two subnets");
    set.get("gxv2").must(&format!("agree {gxv2} {n2}"));
    attached_to(&mut set, "gxv2", "gxv2", &n2, &[]);

    // **the invitation**, composed as the Operator screen composes it and
    // carried as the bench carried it; mrk places it against its own
    // records — the patron whose node this is — agrees, and attaches
    // while the first node is held: a tab, not a replacement
    let link = format!("fueros://attach?v=1&at={n2}&patron={gxv2}");
    assert_eq!(
        set.get("mrk").must(&format!("invitation {link}")),
        [format!("invited at={n2} node={gxv2} choose=- why=-")]
    );
    set.get("mrk").must(&format!("agree {gxv2} {n2}"));
    // (`primary=` in the answer is the node's admission mode, primary or
    // failover in its subtree, and says nothing about which session this
    // is; the attachments list below is what says a tab was added)
    attached_to(&mut set, "mrk", "gxv2", &n2, &["gxv2"]);
    attached_to(&mut set, "gxv2", "gxv2", &n2, &["mrk"]);
    let held = line(set.get("mrk").must("attachments"), "attachments ");
    assert!(
        held.contains(&gxv) && held.contains(&gxv2),
        "mrk holds a session per node: {held}"
    );
    let agreed = set.get("mrk").must("agreed");
    assert!(
        agreed.contains(&format!("agreed {gxv} at={n1}"))
            && agreed.contains(&format!("agreed {gxv2} at={n2}")),
        "the kernel keeps both agreements: {agreed:?}"
    );

    // **messages both ways on each node.** Delivery is the proof of the
    // route: gxv2 holds a session with its own node and no other, so
    // mrk's reply reaches it through that node — the tab it came by —
    // or not at all; and gxv's node speaks as gxv, so mrk's reply to the
    // *person* gxv arrives only if a patron is a leaf like any other
    send(&mut set, "gxv2", "mrk", b"from the second identity");
    arrives(&mut set, "mrk", "gxv2", b"from the second identity");
    send(&mut set, "mrk", "gxv2", b"back by the tab it came by");
    arrives(&mut set, "gxv2", "mrk", b"back by the tab it came by");
    send(&mut set, "gxv", "mrk", b"from the first identity");
    arrives(&mut set, "mrk", "gxv", b"from the first identity");
    send(
        &mut set,
        "mrk",
        "gxv",
        b"back to the patron, who is a person",
    );
    arrives(
        &mut set,
        "gxv",
        "mrk",
        b"back to the patron, who is a person",
    );

    // **the restart.** The agreements are the kernel's and survive it;
    // the shell's bring-up attaches each, and both sessions are held
    // again without anybody re-inviting anybody
    // the shell saves after every change; the driver saves before the
    // restart, which is the last change
    assert_eq!(set.get("mrk").must("save"), ["saved"]);
    set.restart("mrk");
    assert_eq!(set.get("mrk").must("attached"), ["attached false"]);
    let agreed = set.get("mrk").must("agreed");
    assert!(
        agreed.contains(&format!("agreed {gxv} at={n1}"))
            && agreed.contains(&format!("agreed {gxv2} at={n2}")),
        "the agreements survive the restart: {agreed:?}"
    );
    attached_to(&mut set, "mrk", "gxv", &n1, &[]);
    attached_to(&mut set, "mrk", "gxv2", &n2, &[]);
    let held = line(set.get("mrk").must("attachments"), "attachments ");
    assert!(
        held.contains(&gxv) && held.contains(&gxv2),
        "both held again: {held}"
    );
    send(&mut set, "gxv2", "mrk", b"after the restart");
    arrives(&mut set, "mrk", "gxv2", b"after the restart");
    send(&mut set, "mrk", "gxv", b"and back on the other node");
    arrives(&mut set, "gxv", "mrk", b"and back on the other node");
    let _ = mrk;
}
