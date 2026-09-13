//! What a client keeps of its own horizon (design §15.1.1;
//! `light-client-requirements.md` §4.2): what it can place without asking,
//! how far away a party is, what a wake costs, and whose copy wins.

mod common;

use common::*;
use rhtn_archive::record::Record;
use rhtn_archive::topology::Snapshot;
use rhtn_archive::tx::*;
use rhtn_client::horizon::{Horizon, Took, Woke};

/// `node` adopted under `patron` at `path` below `anchor`, into `series`.
fn adopt_at(w: &mut World, node: &str, patron: &str, anchor: &str, path: Vec<u8>, nibbles: u64, series: u32) -> Record {
    let pop = w.meet(node, patron);
    let t = w.tick();
    let (bn, bp) = (w.back(node), w.back(patron));
    let a = Adoption {
        node: kh(node),
        patron: kh(patron),
        locator: Locator { anchor: kh(anchor), path, nibbles, seqno: Seqno { series, counter: 0 } },
        timestamp: t,
        key_material: None,
        evidence: Evidence::Presence(pop.txid),
        presented_head: None,
        back: [&bn, &bp],
    };
    w.commit(TYPE_ADOPTION, &adoption_body(&a), &[node, patron])
}

fn adopt(w: &mut World, node: &str, patron: &str, path: Vec<u8>, nibbles: u64) -> Record {
    adopt_at(w, node, patron, "bob", path, nibbles, 1)
}

/// A client for `me` that has taken every record in `recs`, presence
/// records included, as its serving node propagated them.
fn fed(me: &str, recs: &[Record]) -> Horizon {
    let mut h = Horizon::new(kh(me));
    for r in recs {
        h.ingest(&r.bytes, &ids());
    }
    h
}

// acceptance: TOP-21
#[test]
fn a_client_places_every_node_in_its_horizon_and_says_how_far_away_it_is() {
    // bob roots a subtree: alice and carol under bob, w1 under alice, and
    // w3 under w2 under carol, which puts w3 three edges from alice
    let mut w = World::new();
    let recs = vec![
        adopt(&mut w, "alice", "bob", vec![0x10], 1),
        adopt(&mut w, "carol", "bob", vec![0x20], 1),
        adopt(&mut w, "w1", "alice", vec![0x11], 2),
        adopt(&mut w, "w2", "carol", vec![0x21], 2),
        adopt(&mut w, "w3", "w2", vec![0x21, 0x10], 3),
    ];
    let h = fed("alice", &recs);

    // every node within two edges is placed from what this client holds,
    // with nobody asked: bob the patron, w1 the subordinate, carol the
    // sibling, w2 the nephew
    for n in ["bob", "carol", "w1", "w2"] {
        assert!(h.place(&kh(n)).is_some(), "{n} is placed");
    }
    assert_eq!(h.place(&kh("w1")).map(|p| (p.anchor, p.nibbles)), Some((kh("bob"), 2)), "a subordinate, from its own record");
    assert_eq!(h.place(&kh("bob")).map(|p| (p.anchor, p.nibbles)), Some((kh("bob"), 0)), "the anchor, at the empty path");
    assert!(h.locator(&kh("bob")).is_none(), "and with no locator, since no record carried one for it");

    // and the distance is the number of adoption or sibling edges
    assert_eq!(h.distance(&kh("alice")), Some(0), "itself");
    assert_eq!(h.distance(&kh("bob")), Some(1), "the patron");
    assert_eq!(h.distance(&kh("w1")), Some(1), "a subordinate");
    assert_eq!(h.distance(&kh("carol")), Some(1), "a sibling under the same patron");
    assert_eq!(h.distance(&kh("w2")), Some(2), "a nephew");
    assert_eq!(h.distance(&kh("w3")), None, "three edges: outside the horizon");
    assert_eq!(h.distance(&kh("w4")), None, "a party in no record at all");

    // membership and distance are the same walk, so they cannot disagree
    let inside = h.table.horizon(&kh("alice"), 2);
    for n in ["alice", "bob", "carol", "w1", "w2", "w3", "w4"] {
        assert_eq!(h.distance(&kh(n)).is_some(), inside.contains(&kh(n)), "{n}");
    }

    // w3 sits three edges out and its record placed it, so the copy holds
    // a party the horizon does not: a prune is what drops it
    let mut h = h;
    assert!(h.place(&kh("w3")).is_some(), "placed by its own record, horizon or not");
    let dropped = h.prune();
    assert_eq!(dropped, 1, "one party forgotten");
    assert!(h.place(&kh("w3")).is_none(), "and it is w3");
    assert!(h.locator(&kh("w3")).is_none(), "its locator with it");
    for n in ["alice", "bob", "carol", "w1", "w2"] {
        assert!(h.place(&kh(n)).is_some(), "{n} is inside and stays");
    }
    assert_eq!(h.prune(), 0, "and a second prune has nothing to do");
}

// acceptance: TOP-22
#[test]
fn a_wake_folds_what_arrived_since_and_discards_a_copy_it_cannot_account_for() {
    let mut w = World::new();
    let first = vec![adopt(&mut w, "alice", "bob", vec![0x10], 1), adopt(&mut w, "carol", "bob", vec![0x20], 1)];
    let mut h = fed("alice", &first);
    let held = h.records();
    let snap = h.materialise();
    assert_eq!(snap.records as usize, held, "the watermark counts what went in");

    // more arrives, and a wake with the earlier copy folds in only that
    let later = adopt(&mut w, "w1", "alice", vec![0x11], 2);
    assert_eq!(h.ingest(&later.bytes, &ids()), Took::Applied);
    let after_ingest: Vec<_> = h.resolvable();
    let folded = h.records() - held;
    let mut waking = fed("alice", &[]);
    for (_, b) in h.stored() {
        waking.restore_record(b.clone());
    }
    assert_eq!(waking.wake(Some(&snap), &ids()), Woke::Extended { folded }, "only what arrived since");
    assert_eq!(waking.resolvable(), after_ingest, "and it lands where the ingest did");

    // the copy that is current folds nothing
    let current = h.materialise();
    let mut idle = fed("alice", &[]);
    for (_, b) in h.stored() {
        idle.restore_record(b.clone());
    }
    assert_eq!(idle.wake(Some(&current), &ids()), Woke::Current);
    assert_eq!(idle.resolvable(), after_ingest);

    // and every copy that cannot account for what is held is discarded
    // whole, landing on what a replay from nothing gives
    let all = h.records();
    let damaged = Snapshot { table: b"not a table".to_vec(), ..current.clone() };
    let overclaiming = Snapshot { records: 99, ..current.clone() };
    let ahead = Snapshot { records: 0, high: Some((u64::MAX, [0xff; 32])), ..current.clone() };
    let someone_elses = fed("carol", &first).materialise();
    for (label, s) in [("damaged", Some(damaged)), ("overclaiming", Some(overclaiming)), ("ahead", Some(ahead)), ("another client's", Some(someone_elses)), ("none", None)] {
        let mut v = fed("alice", &[]);
        for (_, b) in h.stored() {
            v.restore_record(b.clone());
        }
        assert_eq!(v.wake(s.as_ref(), &ids()), Woke::Replayed { replayed: all }, "{label}");
        assert_eq!(v.resolvable(), after_ingest, "{label}: the same answer a replay gives");
    }
}

// acceptance: TOP-23
#[test]
fn what_the_patron_propagates_replaces_what_the_client_held() {
    let mut w = World::new();
    let mut recs = vec![adopt(&mut w, "alice", "bob", vec![0x10], 1), adopt(&mut w, "carol", "bob", vec![0x20], 1)];
    recs.push(adopt(&mut w, "w1", "carol", vec![0x21], 2));
    let mut h = fed("alice", &recs);
    let first = h.locator(&kh("w1")).cloned().expect("placed");
    assert_eq!((first.nibbles, first.path.clone()), (2, vec![0x21]));

    // w1 leaves carol and is adopted by alice, deeper and on a new series:
    // the later propagation is what the client answers with
    let departure = {
        let t = w.tick();
        let bn = w.back("w1");
        let body = departure_body(&bn, &kh("w1"), &kh("carol"), Seqno { series: 1, counter: 0 }, t, None);
        w.commit(TYPE_DEPARTURE, &body, &["w1"])
    };
    assert_eq!(h.ingest(&departure.bytes, &ids()), Took::Applied);
    assert!(!h.table.subordinates(&kh("carol")).contains(&kh("w1")), "the ended binding is gone");

    let moved = adopt_at(&mut w, "w1", "alice", "bob", vec![0x11], 2, 2);
    assert_eq!(h.ingest(&moved.bytes, &ids()), Took::Applied);
    let now = h.locator(&kh("w1")).cloned().expect("still placed");
    assert_eq!((now.nibbles, now.path.clone(), now.seqno.series), (2, vec![0x11], 2), "the later locator, not the earlier");
    assert_ne!((now.path, now.seqno.series), (first.path, first.seqno.series));
    assert!(h.table.subordinates(&kh("alice")).contains(&kh("w1")), "and the binding the propagation made");

    // propagation repeats, and a repeat is not news
    assert_eq!(h.ingest(&moved.bytes, &ids()), Took::Duplicate);
    // what is not a record, or not signed by whom it names, changes nothing
    assert_eq!(h.ingest(b"not a record", &ids()), Took::Refused);
}
