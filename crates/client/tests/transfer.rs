//! **A former patron's countersignature, asked and answered on the
//! payload path** (`wire-format.md` §7.10.4; design §6.1.1, §6.2.3,
//! Appendix A.3): the subordinate asks, the ask waits at the patron's node
//! while its client is away, and the client signs the statement when it
//! next attaches, with nobody asked.

mod common;

use common::net::*;
use common::*;
use rhtn_client::ceremony::Abort;
use rhtn_client::payload::{KIND_TRANSFER_GRANT, KIND_TRANSFER_REQUEST};
use rhtn_client::store::ClientStore;
use rhtn_codec::cose::aad;
use rhtn_codec::encode::{emit_bstr, emit_map_head, emit_uint};
use rhtn_crypto::verify;

/// A `TransferRequest` naming `new_patron`, as a subordinate sends it.
fn request(new_patron: &str) -> Vec<u8> {
    let mut b = Vec::new();
    emit_map_head(&mut b, 1);
    emit_uint(&mut b, 1);
    emit_bstr(&mut b, &kh(new_patron));
    b
}

// functional: TX-031
#[test]
fn a_former_patron_vouches_for_a_move_when_it_next_attaches_and_the_subject_holds_it() {
    let mut n = net(&["w1"], &[("alice", "w1"), ("bob", "w1"), ("carol", "w1")]);
    n.s.adopted("bob", "alice", "w2", 1);
    n.attach("alice");
    n.attach("bob");

    // alice is away, and no direct path joins the two: bob asks her to
    // vouch for his move under carol
    n.s.handles["alice"].reach.0.set(false);
    n.s.handles["bob"].reach.0.set(false);
    n.away.insert(kh("alice"));
    let msgs =
        n.s.client("bob")
            .request_transfer(kh("alice"), kh("carol"))
            .expect("bob was adopted by alice");
    n.carry(kh("bob"), msgs);
    assert!(
        n.s.client("bob").transfer_for(&kh("carol")).is_none(),
        "nothing is held before she answers"
    );
    assert_eq!(n.queued("alice").len(), 1, "the ask waits at her node");

    // **she comes back, and her client signs with nobody asked**
    let asked = n.s.prompts_asked("alice");
    n.away.remove(&kh("alice"));
    let got = n.collect("alice");
    assert_eq!(got.len(), 1);
    assert!(got[0].starts_with("Transfer(vouched for"), "{got:?}");
    assert_eq!(
        n.s.prompts_asked("alice"),
        asked,
        "her person was asked nothing"
    );

    // bob holds the statement, and it is the one §4.1 field 9 carries
    let (former, block) =
        n.s.client("bob")
            .transfer_for(&kh("carol"))
            .expect("held once it came");
    assert_eq!(former, kh("alice"));
    verify::transfer_statement(&ids()[..], &kh("bob"), &kh("alice"), &kh("carol"), &block)
        .expect("over bob, alice and carol");
    for (node, new_patron) in [("bob", "w1"), ("w2", "carol")] {
        assert!(
            verify::transfer_statement(
                &ids()[..],
                &kh(node),
                &kh("alice"),
                &kh(new_patron),
                &block
            )
            .is_err(),
            "and for no other move: {node} under {new_patron}"
        );
    }

    // kept across a restart and in a backup
    let st = n.s.client("bob").store.clone();
    let back = ClientStore::decode(&st.encode()).expect("the store reads back");
    assert_eq!(back.transfers, st.transfers);
}

// functional: TX-031
#[test]
fn a_patron_vouches_only_for_a_node_it_adopted_and_has_not_disavowed() {
    let mut n = net(&["w1"], &[("alice", "w1"), ("bob", "w1"), ("carol", "w1")]);
    let adoption = n.s.adopted("bob", "alice", "w2", 1);
    for c in ["alice", "bob", "carol"] {
        n.attach(c);
    }

    // carol was never under alice: her client will not ask, and an ask
    // sent anyway is answered with nothing
    assert!(matches!(
        n.s.client("carol").request_transfer(kh("alice"), kh("w1")),
        Err(Abort::NoRelationship(_))
    ));
    let msgs =
        n.s.client("carol")
            .send_payload(kh("alice"), KIND_TRANSFER_REQUEST, &request("w1"))
            .unwrap();
    n.carry(kh("carol"), msgs);
    assert!(
        n.delivered
            .iter()
            .any(|(f, t, w)| *f == kh("carol") && *t == kh("alice") && w.contains("never adopted")),
        "{:?}",
        n.delivered
    );
    assert!(
        !n.delivered.iter().any(|(f, _, _)| *f == kh("alice")),
        "and alice sent nothing back"
    );

    // a move under the node itself, or under its patron, names nothing
    for bad in ["bob", "alice"] {
        assert!(matches!(
            n.s.client("bob").request_transfer(kh("alice"), kh(bad)),
            Err(Abort::PatronRefused(_))
        ));
    }

    // a statement bob did not ask for is not held
    let block = rhtn_archive::tx::transfer_block(&id("alice"), &kh("bob"), &kh("w1"));
    let mut grant = Vec::new();
    emit_map_head(&mut grant, 2);
    emit_uint(&mut grant, 1);
    emit_bstr(&mut grant, &kh("w1"));
    emit_uint(&mut grant, 2);
    emit_map_head(&mut grant, 2);
    emit_uint(&mut grant, 1);
    emit_bstr(&mut grant, &kh("alice"));
    emit_uint(&mut grant, 2);
    grant.extend_from_slice(&block);
    let msgs =
        n.s.client("alice")
            .send_payload(kh("bob"), KIND_TRANSFER_GRANT, &grant)
            .unwrap();
    n.carry(kh("alice"), msgs);
    assert!(n.s.client("bob").transfer_for(&kh("w1")).is_none());

    // **once alice has disavowed bob, she vouches for no move of his**:
    // the disavowal reaching her view of the relationship as the topology
    // brings it, minted where her seed is (design §23.3)
    let rec = n.s.client("alice").store.records[&adoption].clone();
    let alice = n.s.client("alice");
    assert!(matches!(
        alice.horizon.ingest(&rec, &ids()[..]),
        rhtn_client::horizon::Took::Applied | rhtn_client::horizon::Took::Duplicate
    ));
    let t = n.now();
    let bp = n.s.client("alice").archive.next_back_pointers();
    let body = rhtn_archive::tx::disavowal_body(&bp, &kh("alice"), &kh("bob"), t, Some(1));
    let entries = id("alice").sign_entries(aad::ENVELOPE, &body);
    let env = rhtn_archive::tx::envelope_from_entries(
        rhtn_archive::tx::TYPE_DISAVOWAL,
        &body,
        &[(kh("alice"), entries)],
    );
    let took = n.s.client("alice").horizon.ingest(&env, &ids()[..]);
    assert_eq!(took, rhtn_client::horizon::Took::Applied);
    let before = n.delivered.len();
    let msgs =
        n.s.client("bob")
            .request_transfer(kh("alice"), kh("carol"))
            .expect("bob still holds the adoption");
    n.carry(kh("bob"), msgs);
    assert!(
        n.delivered[before..]
            .iter()
            .any(|(_, t, w)| *t == kh("alice") && w.contains("disavowed")),
        "{:?}",
        &n.delivered[before..]
    );
    assert!(n.s.client("bob").transfer_for(&kh("carol")).is_none());
}
