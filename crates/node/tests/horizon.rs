//! **The horizon an attached client asserts** (`wire-format.md` §7.12):
//! taken from a sender speaking for its own downline, refused from one
//! speaking past it, and never bound without the material that lets the
//! member be recognised.

mod common;

use common::*;
use rhtn_archive::submission::{
    HorizonAssertion, SUBMISSION_ACCEPTED, SUBMISSION_REFUSED, SubmissionReply,
};
use rhtn_archive::topology::{Asserted, Table};
use rhtn_transport::tls::Pins;

/// A row of `node` under `patron`, with `node`'s real material.
fn row(node: &str, patron: &str, revision: u64, retracted: bool) -> HorizonAssertion {
    let m = id(node).public;
    HorizonAssertion {
        node: m.keyhash,
        patron: kh(patron),
        series: 1,
        revision,
        retracted,
        adoption: (!retracted).then_some([0x33; 32]),
        from: (!retracted).then_some(1_700),
        anchor: (!retracted).then(|| kh(patron)),
        slot: (!retracted).then_some(1),
        material: (!retracted).then(|| m.key_material()),
        nonce: [revision as u8; 16],
    }
}

fn offer(v: &mut rhtn_node::view::NodeView, pins: &Pins, peer: &str, a: &HorizonAssertion) -> u64 {
    let mut ids = Vec::new();
    offer_with(v, pins, &mut ids, peer, a)
}

/// The same, keeping the identities the node came to know.
fn offer_with(
    v: &mut rhtn_node::view::NodeView,
    pins: &Pins,
    ids: &mut Vec<rhtn_crypto::identity::Identity>,
    peer: &str,
    a: &HorizonAssertion,
) -> u64 {
    let reply = rhtn_node::submissions::horizon(v, Some(pins), ids, &kh(peer), &a.encode())
        .expect("a reply");
    SubmissionReply::decode(&reply)
        .expect("a submission reply")
        .code
}

/// alice roots it, bob is under alice: the table a node would hold once
/// its operator had told it.
fn served() -> (rhtn_node::view::NodeView, Pins) {
    served_in(Some(std::env::temp_dir().join(format!(
        "rhtn-peer-{}-{:?}",
        std::process::id(),
        std::thread::current().id()
    ))))
}

/// The same, with nowhere to keep rows where `dir` is `None`.
fn served_in(dir: Option<std::path::PathBuf>) -> (rhtn_node::view::NodeView, Pins) {
    let mut t = Table::with_me(kh("alice"));
    t.assert(&Asserted {
        node: kh("bob"),
        patron: kh("alice"),
        series: 1,
        adoption: [0x11; 32],
        from: 1_000,
        anchor: Some(kh("alice")),
        slot: Some(0),
        revision: 1,
    });
    let mut v = view("alice", t, "alice", &[]);
    if let Some(d) = &dir {
        let _ = std::fs::remove_dir_all(d);
    }
    v.horizon_dir = dir;
    (v, Pins::new())
}

#[test]
fn a_sender_speaking_for_its_own_subordinate_is_taken() {
    let (mut v, pins) = served();
    // bob asserts carol under itself: bob is the patron, so bob may
    assert_eq!(
        offer(&mut v, &pins, "bob", &row("carol", "bob", 2, false)),
        SUBMISSION_ACCEPTED
    );
    assert!(
        v.table
            .subordinates(&kh("bob"))
            .contains(&id("carol").public.keyhash)
    );
    // **and the material is pinned with it**, or the row names a member
    // this node could never recognise at a handshake
    let material = id("carol").public.key_material();
    let classical =
        rhtn_transport::tls::classical_member(&material).expect("a classical key in it");
    assert_eq!(
        pins.keyhash_for_key(&classical),
        Some(id("carol").public.keyhash),
        "the node can now resolve carol's presented key"
    );
}

#[test]
fn a_sender_speaking_past_its_own_downline_is_refused() {
    // **the whole of what bounds an assertion** [author, 2026-11-04]:
    // authority follows the topology, so carol — which this table does not
    // place at all — speaks for nothing, and bob may not assert a row
    // whose patron is alice above it.
    let (mut v, pins) = served();
    assert_eq!(
        offer(&mut v, &pins, "carol", &row("dave", "carol", 2, false)),
        SUBMISSION_REFUSED,
        "a sender the table does not place speaks for nothing"
    );
    assert_eq!(
        offer(&mut v, &pins, "bob", &row("dave", "alice", 2, false)),
        SUBMISSION_REFUSED,
        "and a subordinate may not assert its own patron's edges"
    );
}

#[test]
fn a_row_without_material_is_refused_rather_than_bound() {
    let (mut v, pins) = served();
    let mut a = row("carol", "bob", 2, false);
    a.material = None;
    assert_eq!(
        offer(&mut v, &pins, "bob", &a),
        SUBMISSION_REFUSED,
        "a row this node cannot pin would bind a member it never recognises"
    );
    assert!(v.table.subordinates(&kh("bob")).is_empty());
}

#[test]
fn a_retraction_from_the_same_sender_removes_it_and_a_stale_one_does_not() {
    let (mut v, pins) = served();
    assert_eq!(
        offer(&mut v, &pins, "bob", &row("carol", "bob", 2, false)),
        SUBMISSION_ACCEPTED
    );
    assert_eq!(
        offer(&mut v, &pins, "bob", &row("carol", "bob", 3, true)),
        SUBMISSION_ACCEPTED
    );
    assert!(v.table.subordinates(&kh("bob")).is_empty());
    // the counter outlives the row: an assertion still in flight from the
    // sender's other device must not bring the member back
    assert_eq!(
        offer(&mut v, &pins, "bob", &row("carol", "bob", 2, false)),
        SUBMISSION_REFUSED
    );
    assert!(v.table.subordinates(&kh("bob")).is_empty());
}

#[test]
fn a_node_not_yet_serving_cannot_pin_and_so_takes_nothing() {
    let (mut v, _) = served();
    let a = row("carol", "bob", 2, false);
    let mut ids = Vec::new();
    let reply = rhtn_node::submissions::horizon(&mut v, None, &mut ids, &kh("bob"), &a.encode())
        .expect("a reply");
    assert_eq!(
        SubmissionReply::decode(&reply).unwrap().code,
        SUBMISSION_REFUSED
    );
}

#[test]
fn a_row_taken_is_written_before_it_is_bound() {
    // **the discipline the operator's route already held to** [2026-11-04]:
    // the first cut of this bound in memory and wrote nothing, so a row an
    // attached client asserted was gone at the next start — on a node that
    // is restarted routinely.
    let (mut v, pins) = served();
    let dir = v.horizon_dir.clone().expect("a directory");
    assert_eq!(
        offer(&mut v, &pins, "bob", &row("carol", "bob", 2, false)),
        SUBMISSION_ACCEPTED
    );
    let kept = rhtn_node::horizon::read_all(&dir).0;
    assert_eq!(kept.len(), 1, "the row is on disk");
    assert_eq!(kept[0].row.node, id("carol").public.keyhash);
    assert!(
        kept[0].material.is_some(),
        "with the material to pin it again"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_node_with_nowhere_to_keep_a_row_refuses_it() {
    let (mut v, pins) = served_in(None);
    assert_eq!(
        offer(&mut v, &pins, "bob", &row("carol", "bob", 2, false)),
        SUBMISSION_REFUSED,
        "taking it into a table that would forget it is worse than refusing"
    );
    assert!(v.table.subordinates(&kh("bob")).is_empty());
}

#[test]
fn a_member_taken_becomes_one_this_node_can_verify_as_well_as_recognise() {
    // **the bug the bench named** [2026-11-04]: the pin let the node
    // resolve the key a member presented, and `ids` is what lets it verify
    // what that member *signs* — so a member bound without it had its
    // bundle and its profile refused the instant it attached, and a single
    // refusal detaches. `pay.refused ["PublishBundle", "PublishProfile"]`
    // was the whole of what the client could say until it was named.
    let (mut v, pins) = served();
    let mut ids = Vec::new();
    assert_eq!(
        offer_with(
            &mut v,
            &pins,
            &mut ids,
            "bob",
            &row("carol", "bob", 2, false)
        ),
        SUBMISSION_ACCEPTED
    );
    assert!(
        ids.iter().any(|i| i.keyhash == id("carol").public.keyhash),
        "carol is now one this node can verify a signature from"
    );
    let dir = v.horizon_dir.clone().expect("a directory");
    let _ = std::fs::remove_dir_all(&dir);
}
