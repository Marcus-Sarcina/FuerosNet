//! A subject's own lines (design §9.0; `wire-format.md` §4.6;
//! `light-client-requirements.md` §2): sealed on rotation as the old key's
//! last act, sealed and then reissued on suspicion, never reissued into a
//! series held before.

mod common;

use common::*;
use rhtn_archive::chain::Archive;
use rhtn_archive::locator::{COUNTER_MAX, SignedLocator};
use rhtn_archive::record::Record;
use rhtn_archive::tx::*;
use rhtn_client::rotation::*;
use rhtn_codec::cbor::*;

/// Adopt `node` under `patron` on a meeting, into `series`.
fn adopt(w: &mut World, node: &str, patron: &str, series: u32) -> Record {
    let pop = w.meet(node, patron);
    let t = w.tick();
    let (bn, bp) = (w.back(node), w.back(patron));
    let a = Adoption {
        node: kh(node),
        patron: kh(patron),
        locator: Locator { anchor: kh(patron), path: vec![0x10], nibbles: 2, seqno: Seqno { series, counter: 0 } },
        timestamp: t,
        key_material: None,
        evidence: Evidence::Presence(pop.txid),
        presented_head: None,
        back: [&bn, &bp],
    };
    let body = adoption_body(&a);
    w.commit(TYPE_ADOPTION, &body, &[node, patron])
}

/// alice under bob in series 1 and under carol in series 2.
fn two_relationships() -> (World, Archive) {
    let mut w = World::new();
    adopt(&mut w, "alice", "bob", 1);
    adopt(&mut w, "alice", "carol", 2);
    let a = w.archives[&kh("alice")].clone();
    (w, a)
}

fn seqno_of(seal: &[u8]) -> (Seqno, Locator) {
    let sl = SignedLocator::parse(seal).unwrap();
    assert_eq!(sl.subject, kh("alice"));
    assert_eq!(sl.verify(&ids()), Ok(true), "self-signed by alice");
    (sl.locator.seqno, sl.locator)
}

// acceptance: REC-05
#[test]
fn a_rotation_seals_every_old_line_as_the_old_keys_last_act() {
    let (_w, archive) = two_relationships();
    let rels = relationships(&archive);
    assert_eq!(rels.iter().map(|r| (r.patron, r.series())).collect::<Vec<_>>().len(), 2);
    let mut rot = Rotation::begin(Box::new(id("alice")), archive);
    // the old key is in hand for the successor statement
    assert!(rot.old_key().is_some());
    assert!(!rot.sealed());
    // then the seals, before any adoption under k2 goes anywhere
    let seals = rot.seal().to_vec();
    assert_eq!(seals.len(), 2, "one per series");
    for (patron, seal) in &seals {
        let (sq, loc) = seqno_of(seal);
        let rel = rels.iter().find(|r| r.patron == *patron).unwrap();
        assert_eq!(sq, Seqno { series: rel.series(), counter: COUNTER_MAX }, "at the maximum counter");
        assert_eq!((loc.anchor, loc.path, loc.nibbles), (rel.position.anchor, rel.position.path.clone(), rel.position.nibbles), "the position unchanged");
    }
    // and nothing signed by k1 follows them: the key is gone from the client
    assert!(rot.sealed());
    assert!(rot.old_key().is_none());
    assert_eq!(rot.seal().len(), 2, "sealing again produces nothing new");
}

// acceptance: REC-06
#[test]
fn on_suspicion_each_line_is_sealed_then_reissued_naming_the_sealed_counter() {
    let (w, mut archive) = two_relationships();
    let t = w.clock + 60;
    let repairs = suspect_compromise(&id("alice"), &archive, t, &|p| w.archives[p].next_back_pointers(), &[9, 10, 11]).unwrap();
    assert_eq!(repairs.len(), 2);
    let mut seen = std::collections::BTreeSet::new();
    for r in &repairs {
        let name = NAMES.iter().find(|n| kh(n) == r.patron).unwrap();
        let old_series = relationships(&archive).into_iter().find(|x| x.patron == r.patron).unwrap().series();
        // the seal first, at the maximum
        let (sq, _) = seqno_of(&r.seal);
        assert_eq!(sq, Seqno { series: old_series, counter: COUNTER_MAX });
        // then the type-7 reissue: field 3 the sealed maximum, field 4 a fresh series at 0
        let item = parse_all(&r.reissue).unwrap();
        let Item::Map(m) = &item else { panic!() };
        let sq_of = |k: u64| {
            let Some(Item::Array(a)) = map_get(m, k) else { panic!("field {k}") };
            Seqno { series: as_uint(&a[0]).unwrap() as u32, counter: as_uint(&a[1]).unwrap() as u32 }
        };
        assert_eq!(sq_of(3), Seqno { series: old_series, counter: COUNTER_MAX });
        assert_eq!(sq_of(4), Seqno { series: r.new_series, counter: 0 });
        assert!(!archive.series_occupied().contains(&r.new_series), "fresh");
        assert!(seen.insert(r.new_series), "distinct per relationship");
        // countersigned by that patron, it extends the chain
        let env = envelope(TYPE_REISSUE, &r.reissue, &[&id("alice"), &id(name)]);
        take_reissue(&mut archive, &r.reissue, &env, &ids()).unwrap();
        assert_eq!(archive.chain_for(&r.patron).unwrap().series(), vec![old_series, r.new_series]);
    }
    // a record countersigned by someone other than the patron is not the proposal
    let r = &repairs[0];
    let wrong = envelope(TYPE_REISSUE, &r.reissue, &[&id("alice"), &id("w1")]);
    assert_eq!(take_reissue(&mut archive, &r.reissue, &wrong, &ids()), Err(Refusal::NotTheProposal));
}

// acceptance: REC-07
#[test]
fn a_reissue_never_enters_a_series_the_key_has_occupied() {
    let (w, mut archive) = two_relationships();
    let t = w.clock + 60;
    let bob_back = w.archives[&kh("bob")].next_back_pointers();
    // s1 and s2 are occupied; move bob's line to s3
    let body = propose_reissue(&archive, &kh("bob"), &bob_back, 4, 3, t).unwrap();
    take_reissue(&mut archive, &body, &envelope(TYPE_REISSUE, &body, &[&id("alice"), &id("bob")]), &ids()).unwrap();
    assert_eq!(archive.series_occupied().into_iter().collect::<Vec<_>>(), vec![1, 2, 3]);
    // asked to reissue back into s1 or s2: no such reissue is produced
    for used in [1, 2, 3] {
        assert_eq!(propose_reissue(&archive, &kh("bob"), &bob_back, 5, used, t), Err(Refusal::SeriesOccupied(used)));
    }
    assert_eq!(fresh_series(&archive, [1, 2, 3, 4, 5]), Some(4));
    // one into an unused series is
    assert!(propose_reissue(&archive, &kh("bob"), &bob_back, 5, 4, t).is_ok());
    // and no relationship, no reissue
    assert_eq!(propose_reissue(&archive, &kh("w1"), &[], 5, 4, t), Err(Refusal::NoRelationship));
}
