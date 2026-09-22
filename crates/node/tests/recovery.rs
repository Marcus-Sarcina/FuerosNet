//! Recovery entries at the patron and the observer (design §9; `wire-format.md`
//! §4.1): what a patron countersigns and what it refuses, and what a plain
//! rotation carries.

mod common;

use common::*;
use rhtn_archive::record::Record;
use rhtn_archive::tx::{self, *};
use rhtn_node::propagation::{FRAME_TOPOLOGY_MEMO, Memo};
use rhtn_node::store::KIND_TRANSACTION;

/// Patron alice, under w1, holding bob as a subordinate; alice's view with a
/// session up to w1.
fn patron() -> (World, rhtn_node::view::NodeView, std::sync::Arc<Fabric>) {
    let mut w = World::new();
    let (ua, _) = w.adopt("alice", "w1", 1);
    let (ab, pop) = w.adopt("bob", "alice", 2);
    let t = table_with(kh("alice"), &w, &[&ua, &pop, &ab], &["w1", "alice"]);
    let v = view("alice", t, "w1", &[0]);
    let fab = Fabric::with(&[kh("w1"), kh("bob")]);
    (w, v, fab)
}

fn recovery_block_for(old: &str, new: &str, patron: &str, verifier: &str) -> Vec<u8> {
    let qid = rhtn_codec::cose::sha256(format!("q:{old}:{new}").as_bytes());
    let resp = tx::recovery_response(&id(verifier), &id(new), &qid, &kh(old));
    tx::recovery_block(&id(old), &kh(new), &kh(patron), vec![resp])
}

// acceptance: REC-02
#[test]
fn a_recovery_whose_successor_statement_names_another_successor_or_patron_is_not_countersigned() {
    let (w, mut p, _fab) = patron();
    let back = [rhtn_archive::genesis(&kh("carol"))];
    let bindings = p.table.bindings().len();
    // the statement lifted from a recovery of another new key
    let lifted = recovery_block_for("bob", "alice2", "alice", "w2");
    let body = p
        .propose_adoption(&kh("carol"), Evidence::Recovery(lifted), 3, &back)
        .expect("proposed");
    assert!(
        p.countersign_adoption(&body, &id("carol"), &ids())
            .is_none(),
        "names a different successor"
    );
    // the statement lifted from a recovery under another patron
    let lifted = recovery_block_for("bob", "carol", "w2", "w2");
    let body = p
        .propose_adoption(&kh("carol"), Evidence::Recovery(lifted), 3, &back)
        .expect("proposed");
    assert!(
        p.countersign_adoption(&body, &id("carol"), &ids())
            .is_none(),
        "names a different patron"
    );
    // arriving signed by whoever, neither binds anything either
    let rec = Record::parse(&tx::envelope(
        TYPE_ADOPTION,
        &body,
        &[&id("carol"), &id("alice")],
    ))
    .unwrap();
    assert!(p.table.apply(&rec, &ids(), &w.store, None).is_err());
    assert_eq!(p.table.bindings().len(), bindings, "no binding for either");
    assert_eq!(p.table.current_key(&kh("bob")), kh("bob"));
    // the genuine one is countersigned and binds
    let good = recovery_block_for("bob", "carol", "alice", "w2");
    let body = p
        .propose_adoption(&kh("carol"), Evidence::Recovery(good), 3, &back)
        .expect("proposed");
    let rec = p
        .countersign_adoption(&body, &id("carol"), &ids())
        .expect("countersigned");
    p.table.apply(&rec, &ids(), &w.store, None).expect("binds");
    assert_eq!(p.table.current_key(&kh("bob")), kh("carol"));
}

// acceptance: REC-14
#[test]
fn a_transfer_whose_statement_disagrees_with_the_enclosing_adoption_is_not_countersigned() {
    // alice moves from patron bob to patron carol; carol is the patron evaluating
    let mut w = World::new();
    let (uc, _) = w.adopt("carol", "w1", 1);
    let (ab, pop) = w.adopt("alice", "bob", 2);
    let t = table_with(kh("carol"), &w, &[&uc, &pop, &ab], &["w1", "carol", "bob"]);
    let mut p = view("carol", t, "w1", &[0]);
    let back = w.back("alice");
    let transfer = |former_named: &str, block: Vec<u8>| Evidence::Transfer {
        former: kh(former_named),
        block,
    };
    for (label, evidence) in [
        (
            "a different node",
            transfer(
                "bob",
                tx::transfer_block(&id("bob"), &kh("w2"), &kh("carol")),
            ),
        ),
        (
            "a different new patron",
            transfer(
                "bob",
                tx::transfer_block(&id("bob"), &kh("alice"), &kh("w2")),
            ),
        ),
        (
            "a former patron other than the Transfer's field 1",
            transfer(
                "w3",
                tx::transfer_block(&id("bob"), &kh("alice"), &kh("carol")),
            ),
        ),
    ] {
        let body = p
            .propose_adoption(&kh("alice"), evidence, 5, &back)
            .expect("proposed");
        assert!(
            p.countersign_adoption(&body, &id("alice"), &ids())
                .is_none(),
            "{label}"
        );
    }
    let body = p
        .propose_adoption(
            &kh("alice"),
            transfer(
                "bob",
                tx::transfer_block(&id("bob"), &kh("alice"), &kh("carol")),
            ),
            5,
            &back,
        )
        .unwrap();
    assert!(
        p.countersign_adoption(&body, &id("alice"), &ids())
            .is_some(),
        "the consistent one"
    );
}

// acceptance: REC-13
#[test]
fn a_plain_rotation_carries_nothing_and_its_memo_names_no_prior_key() {
    let (mut w, mut p, fab) = patron();
    // S rotates from k1 (bob) to k2 (carol) without claiming k1's history:
    // carol meets alice and is adopted on a meeting, like anyone
    let pop = w.meet("carol", "alice");
    let back = w.back("carol");
    let body = p
        .propose_adoption(&kh("carol"), Evidence::Presence(pop.txid), 3, &back)
        .unwrap();
    let rec = p.countersign_adoption(&body, &id("carol"), &ids()).unwrap();
    let k1 = kh("bob");
    assert!(
        rhtn_codec::cbor::value_slice(&rec.bytes[rec.body.clone()], 6).is_none(),
        "no field 6"
    );
    assert!(rec.prior_key().is_none());
    assert!(
        !rec.bytes.windows(32).any(|x| x == k1),
        "no field naming k1 anywhere"
    );
    // the memo for the adoption travels rootward, to w1
    p.store.keep_presence(pop.txid, pop.bytes.clone());
    let d = p.take_object(&*fab, &kh("bob"), KIND_TRANSACTION, &rec.bytes, &ids());
    assert_eq!(d, rhtn_node::store::Decision::Stored);
    let memos = fab.to(&kh("w1"), FRAME_TOPOLOGY_MEMO);
    assert_eq!(memos.len(), 1, "one memo rootward");
    let m = Memo::decode(&memos[0]).unwrap();
    assert_eq!(m.occupant, Some(kh("carol")));
    assert!(
        !memos[0].windows(32).any(|x| x == k1),
        "the memo carries no prior key"
    );
    let memo_item = rhtn_codec::cbor::parse_all(&memos[0]).unwrap();
    let rhtn_codec::cbor::Item::Map(fields) = &memo_item else {
        panic!("memo not a map")
    };
    assert_eq!(fields.len(), 5, "five fields and no sixth for a prior key");
}

// acceptance: REC-10
#[test]
fn the_chain_is_presented_when_asked_and_never_propagated() {
    let mut w = World::new();
    let (ua, _) = w.adopt("alice", "bob", 1);
    let r = w.reissue(
        "alice",
        "bob",
        Seqno {
            series: 1,
            counter: 2,
        },
        5,
    );
    let t = table_with(kh("alice"), &w, &[&ua], &["bob"]);
    let mut s = view("alice", t, "bob", &[0]);
    s.archive = w.archives[&kh("alice")].clone();
    let fab = Fabric::with(&[kh("bob"), kh("carol")]);
    // a counterparty asks S to prove its series: the adoption and the reissue
    let chain = s.present_chain(&kh("bob"));
    assert_eq!(chain, vec![ua.bytes.clone(), r.bytes.clone()]);
    let read = rhtn_archive::series::SeriesChain::from_records(&chain, &ids()).unwrap();
    assert_eq!(read.series(), vec![1, 5]);
    let mut holder = rhtn_node::resolution::LocatorStore::new();
    holder.take_chain(&read);
    assert!(holder.abandoned(&kh("alice"), 1));
    // nothing leaves S on any session in response, then or later
    s.set_now(s.now() + 7 * 86_400);
    assert_eq!(
        fab.count(rhtn_node::propagation::FRAME_TOPOLOGY_PUSH),
        0,
        "no TopologyPush carrying either"
    );
    assert!(fab.frames().is_empty());
    // no chain under a patron S was never adopted by
    assert!(s.present_chain(&kh("carol")).is_empty());
}

/// **A recognised recovery ends the superseded key's relationship**
/// (`infra-client-requirements.md` §6.1; design §9.0.2): the old key is
/// not a party this node serves any more, so what it registered to be
/// woken at is not this node's to keep.
// acceptance: SUB-10
#[test]
fn a_recovery_forgets_what_the_superseded_key_registered_to_be_woken_at() {
    let (_w, mut p, fab) = patron();
    // bob, alice's subordinate, is served here and has somewhere to be
    // woken; so has an unrelated party, which this must not touch
    p.wake.register(
        kh("bob"),
        Some("https://push.example/bob".into()),
        Some(vec![7; 32]),
        None,
    );
    p.wake.register(
        kh("w1"),
        Some("https://push.example/w1".into()),
        Some(vec![9; 32]),
        None,
    );
    assert!(p.wake.get(&kh("bob")).is_some());
    let slot = p.slot_of(&kh("bob"));

    // carol recovers bob's identity under the same patron
    let good = recovery_block_for("bob", "carol", "alice", "w2");
    let back = [rhtn_archive::genesis(&kh("carol"))];
    let body = p
        .propose_adoption(&kh("carol"), Evidence::Recovery(good), 3, &back)
        .expect("proposed");
    let rec = p
        .countersign_adoption(&body, &id("carol"), &ids())
        .expect("countersigned");
    p.store.keep_presence(rec.txid, rec.bytes.clone());
    assert_eq!(
        p.take_object(&*fab, &kh("bob"), KIND_TRANSACTION, &rec.bytes, &ids()),
        rhtn_node::store::Decision::Stored
    );

    // controls: the supersession happened and the successor is bound
    assert_eq!(
        p.table.current_key(&kh("bob")),
        kh("carol"),
        "the old key is superseded"
    );
    assert!(
        p.table.subordinates(&kh("alice")).contains(&kh("carol")),
        "and the successor is a subordinate"
    );

    assert!(
        p.wake.get(&kh("bob")).is_none(),
        "the superseded key's registration is forgotten"
    );
    assert!(
        p.wake.get(&kh("w1")).is_some(),
        "and an unrelated party's is not"
    );
    if let Some(s) = slot {
        assert_ne!(
            p.slots.get(&s).and_then(|r| r.occupant),
            Some(kh("bob")),
            "nor does the old key still hold its row"
        );
    }
    // the successor's own registration is its own to make and is not
    // pre-empted by the old key's going
    assert!(p.wake.get(&kh("carol")).is_none());
}
