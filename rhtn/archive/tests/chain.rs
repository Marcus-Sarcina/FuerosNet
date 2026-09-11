//! Archive entries (ARC): chain construction, seqno rules, merges, serving
//! and fetching, backward verification, checkpoints and pruning.

mod common;

use common::World;
use rhtn_archive::chain::{ArchiveRequest, Evidence as Kept};
use rhtn_archive::record::Record;
use rhtn_archive::tx::*;
use rhtn_archive::walk::{self, BatchEnd, Fetch, Root};
use rhtn_archive::{Txid, WINDOW_SECONDS, genesis};
use std::collections::BTreeMap;

/// A world where alice and bob have formed, and alice adopted bob: both
/// hold chains of two records.
fn formed() -> (World, Txid) {
    let mut w = World::new(&["alice", "bob", "carol", "w1", "w2"]);
    let f = w.meet("alice", "bob");
    w.adopt("bob", "alice", f.txid, 5);
    (w, f.txid)
}

fn key0_lists(rec: &Record) -> Vec<Vec<Txid>> {
    rec.back.clone()
}

// acceptance: ARC-01
#[test]
fn a_first_transaction_carries_the_genesis_value() {
    let mut w = World::new(&["alice", "bob", "carol"]);
    // P (alice) has an existing chain; K (carol) has none; a record between them
    let f = w.meet("alice", "bob");
    w.adopt("bob", "alice", f.txid, 5);
    let pop = w.meet("alice", "carol"); // carol's first transaction
    // K's first transaction THROUGH the component: an adoption of K under P
    let carol = w.kh("carol");
    let mut fresh = World::new(&["alice", "carol"]);
    fresh.store = w.store.clone();
    fresh.clock = w.clock;
    *fresh.archive_mut("alice") = w.archive("alice").clone();
    let adoption = fresh.adopt("carol", "alice", pop.txid, 9);
    let lists = key0_lists(&adoption);
    assert_eq!(lists[0], vec![genesis(&carol)], "K's list is exactly the genesis value");
    assert_eq!(genesis(&carol), rhtn_codec::cose::sha256(&carol), "SHA-256 over the 32 keyhash bytes");
    // a verifier recomputing it reports the chain rooted at genesis at this record
    let wk = walk::walk(&carol, &adoption.txid, &fresh, &fresh.lookup(), 64);
    assert_eq!(wk.root, Root::Genesis);
    assert_eq!(wk.txids(), vec![adoption.txid]);
}

// acceptance: ARC-02
#[test]
fn an_adoption_advances_both_chains_in_signer_order() {
    let (mut w, _) = formed();
    let pop = w.meet("alice", "carol");
    w.adopt("carol", "alice", pop.txid, 7);
    let h_n = w.head("bob");
    let h_p = w.head("alice");
    // a fresh adoption of N (bob) under P (alice): re-adoption on new evidence
    let f = w.meet("alice", "bob");
    let h_n2 = w.head("bob");
    let h_p2 = w.head("alice");
    // a normal record between established keys: the participants' lists in
    // field 3 order (alice, bob), then the witness's
    let lists = key0_lists(&f);
    assert_eq!(lists.len(), 3, "two participants and a witness");
    assert_eq!(lists[..2], [vec![h_p], vec![h_n]], "presence: participants in field 3 order (alice, bob)");
    let adoption = w.adopt("bob", "alice", f.txid, 6);
    assert_eq!(key0_lists(&adoption), vec![vec![h_n2], vec![h_p2]], "node then patron");
    let dep = w.depart("bob", "alice", Seqno { series: 6, counter: 1 });
    assert_eq!(key0_lists(&dep), vec![vec![adoption.txid]]);
    let dis = w.disavow("alice", "carol", Some(0));
    assert_eq!(key0_lists(&dis), vec![vec![adoption.txid]]);
}

// acceptance: ARC-03
#[test]
fn swapped_lists_verify_and_do_not_reach_back() {
    let (mut w, _) = formed();
    w.meet("alice", "carol"); // P's head is a record N never signed
    let h_n = w.head("bob");
    let h_p = w.head("alice");
    let pop = w.meet("alice", "bob");
    let t = w.tick();
    let a = Adoption {
        node: w.kh("bob"),
        patron: w.kh("alice"),
        locator: Locator { anchor: w.kh("alice"), path: vec![0x10], nibbles: 2, seqno: Seqno { series: 6, counter: 0 } },
        timestamp: t,
        key_material: None,
        evidence: Evidence::Presence(pop.txid),
        presented_head: None,
        back: [&[h_p], &[h_n]], // swapped
    };
    let body = adoption_body(&a);
    let env = envelope(TYPE_ADOPTION, &body, &[w.id("bob"), w.id("alice")]);
    let rec = Record::parse(&env).expect("structurally valid");
    assert_eq!(rec.check_signatures(&w.lookup()), rhtn_archive::record::SigStatus::Verified);
    w.store.insert(rec.txid, env);
    let bob = w.kh("bob");
    let wk = walk::walk(&bob, &rec.txid, &w, &w.lookup(), 64);
    assert_eq!(wk.root, Root::DoesNotReachBack { at: rec.txid, named: h_p });
    assert_eq!(wk.txids(), vec![rec.txid], "no complete prefix for N from that head");
}

// acceptance: ARC-04
#[test]
fn a_series_opens_at_zero_and_a_departure_advances_it() {
    let (mut w, _) = formed();
    let adoption = w.archive("bob").get(&w.head("bob")).unwrap().clone();
    assert_eq!(adoption.seqno(), Some(Seqno { series: 5, counter: 0 }));
    let dep = w.depart("bob", "alice", Seqno { series: 5, counter: 3 });
    let s = dep.seqno().unwrap();
    assert_eq!(s.series, 5);
    assert!(s.counter > 0);
    assert_eq!(compare(adoption.seqno().unwrap(), s), Order::Newer);
    // the same {s, c} again with different contents is not newer
    let again = w.loose_departure("bob", "alice", &[dep.txid], Seqno { series: 5, counter: 3 }, w.clock + 1);
    assert_ne!(again.txid, dep.txid);
    assert_eq!(compare(s, again.seqno().unwrap()), Order::Same);
}

// acceptance: ARC-05
#[test]
fn counter_gaps_and_unknown_series_are_accepted() {
    let (mut w, _) = formed();
    let adoption = w.archive("bob").get(&w.head("bob")).unwrap().clone();
    let held = adoption.seqno().unwrap();
    let d1 = w.depart("bob", "alice", Seqno { series: 5, counter: 7 });
    assert_eq!(compare(held, d1.seqno().unwrap()), Order::Newer, "0 to 7 is not an error");
    let d2 = w.loose_departure("bob", "alice", &[d1.txid], Seqno { series: 77, counter: 5 }, w.clock + 1);
    assert_eq!(compare(held, d2.seqno().unwrap()), Order::Incomparable, "no prior state in s2 is not a failure, and no rank against s");
    assert!(Record::parse(&w.store[&d2.txid]).is_ok());
}

// acceptance: ARC-06
#[test]
fn a_timestamp_at_the_floor_or_far_ahead_is_admissible() {
    let (mut w, _) = formed();
    let head = w.head("bob");
    let t0 = w.archive("bob").get(&head).unwrap().effective;
    let t_now = w.clock;
    let at_floor = w.loose_departure("bob", "alice", &[head], Seqno { series: 5, counter: 1 }, t0);
    let far = w.loose_departure("bob", "alice", &[head], Seqno { series: 5, counter: 1 }, t_now + 30 * 86_400);
    let bob = w.kh("bob");
    for r in [&at_floor, &far] {
        let wk = walk::walk(&bob, &r.txid, &w, &w.lookup(), 64);
        assert!(wk.is_unbroken(), "{:?}", wk.root);
        assert!(!matches!(wk.root, Root::Malformed { .. }));
    }
    // and below the floor is malformed
    let early = w.loose_departure("bob", "alice", &[head], Seqno { series: 5, counter: 1 }, t0 - 1);
    let wk = walk::walk(&bob, &early.txid, &w, &w.lookup(), 64);
    assert!(matches!(wk.root, Root::Malformed { .. }));
}

// acceptance: ARC-07
#[test]
fn two_heads_are_merged_by_the_next_ordinary_transaction() {
    let (mut w, _) = formed();
    let base = w.head("bob");
    let t = w.clock;
    // two devices each sign a record after the same base
    let a = w.loose_departure("bob", "alice", &[base], Seqno { series: 5, counter: 1 }, t + 1);
    let b = w.loose_departure("bob", "alice", &[base], Seqno { series: 5, counter: 2 }, t + 2);
    w.archive_mut("bob").append(a.clone()).unwrap();
    w.archive_mut("bob").append(b.clone()).unwrap();
    assert!(w.archive("bob").is_forked());
    let mut expect = vec![a.txid, b.txid];
    expect.sort();
    assert_eq!(w.archive("bob").next_back_pointers(), expect);
    let merge = w.depart("bob", "alice", Seqno { series: 5, counter: 3 });
    assert_eq!(merge.back[0], expect, "both heads, sorted ascending bytewise");
    assert_eq!(merge.tx_type, TYPE_DEPARTURE, "no separate merge object");
    assert!(!w.archive("bob").is_forked());
    let bob = w.kh("bob");
    let wk = walk::walk(&bob, &merge.txid, &w, &w.lookup(), 64);
    assert_eq!(wk.root, Root::Genesis, "both branches reached, nothing unsatisfied");
    assert_eq!(wk.records.len(), 5);
}

/// A subject with `n` records rooted at genesis: formation, adoption, then
/// departures with rising counters.
fn long_chain(n: usize) -> World {
    let mut w = World::new(&["alice", "bob"]);
    let f = w.meet("alice", "bob");
    w.adopt("bob", "alice", f.txid, 5);
    for c in 1..=(n - 2) as u32 {
        w.depart("bob", "alice", Seqno { series: 5, counter: c });
    }
    assert_eq!(w.archive("bob").len(), n);
    w
}

// acceptance: ARC-08
#[test]
fn serving_paginates_head_first_and_continues_from_the_oldest() {
    let w = long_chain(11);
    let bob = w.kh("bob");
    let head = w.head("bob");
    let m = 4;
    let mut all: Vec<Txid> = Vec::new();
    let mut next = Some(head);
    let mut replies = 0;
    loop {
        let nonce = [replies as u8; 16];
        let req = ArchiveRequest { subject: bob, head: next, max_records: m, stop_before: None, nonce };
        let reply = w.archive("bob").serve(&req);
        assert_eq!(reply.nonce, nonce);
        let recs: Vec<Record> = reply.records.iter().map(|b| Record::parse(b).unwrap()).collect();
        if replies == 0 {
            assert_eq!(recs[0].txid, head, "first envelope is the requested head");
        }
        for pair in recs.windows(2) {
            assert_eq!(pair[0].back_pointers_of(&bob).unwrap(), &[pair[1].txid], "each record names the one that follows");
        }
        for r in &recs {
            if !all.contains(&r.txid) {
                all.push(r.txid);
            }
        }
        replies += 1;
        if reply.more {
            assert_eq!(reply.continue_from, Some(recs.last().unwrap().txid), "continue from the oldest returned");
            next = reply.continue_from;
        } else {
            assert_eq!(reply.continue_from, None);
            break;
        }
    }
    assert_eq!(all.len(), 11, "the batches concatenated are the whole chain");
    assert!(replies > 1);
    let wk = walk::walk(&bob, &head, &w, &w.lookup(), 64);
    assert_eq!(wk.txids(), all);
}

// acceptance: ARC-09
#[test]
fn a_batch_that_does_not_chain_fails_at_the_first_mismatch() {
    let w = long_chain(6);
    let bob = w.kh("bob");
    let head = w.head("bob");
    let req = ArchiveRequest { subject: bob, head: Some(head), max_records: 6, stop_before: None, nonce: [1; 16] };
    let honest = w.archive("bob").serve(&req);
    assert_eq!(honest.records.len(), 6);
    // run 1: the middle record replaced by another record of S the preceding one does not name
    let mut bad = honest.clone();
    bad.records[2] = honest.records[4].clone();
    let v = walk::verify_batch(&bob, Some(&head), &bad, &w.lookup());
    assert!(matches!(v.end, BatchEnd::Mismatch { index: 2, .. }), "{:?}", v.end);
    assert_eq!(v.verified.len(), 2, "records from the mismatch on are not a verified prefix");
    // run 2: the first record is not the requested head
    let mut bad2 = honest.clone();
    bad2.records.remove(0);
    let v2 = walk::verify_batch(&bob, Some(&head), &bad2, &w.lookup());
    assert!(matches!(v2.end, BatchEnd::Mismatch { index: 0, .. }));
    assert!(v2.verified.is_empty());
    // and the honest batch verifies to genesis
    let ok = walk::verify_batch(&bob, Some(&head), &honest, &w.lookup());
    assert_eq!(ok.end, BatchEnd::Genesis);
}

// acceptance: ARC-10
#[test]
fn a_short_reply_is_not_a_short_archive() {
    let w = long_chain(8);
    let bob = w.kh("bob");
    let head = w.head("bob");
    let req = ArchiveRequest { subject: bob, head: Some(head), max_records: 8, stop_before: None, nonce: [2; 16] };
    let full = w.archive("bob").serve(&req);
    for more in [true, false] {
        let mut short = full.clone();
        short.records.truncate(3); // k < m, oldest names a record not in the batch
        short.more = more;
        short.continue_from = None;
        let v = walk::verify_batch(&bob, Some(&head), &short, &w.lookup());
        let oldest = Record::parse(&short.records[2]).unwrap().txid;
        match &v.end {
            BatchEnd::Unfetched { continue_from, missing } => {
                assert_eq!(*continue_from, oldest, "the next request names the oldest returned record");
                assert_eq!(missing.len(), 1);
            }
            other => panic!("short reply read as {other:?}"),
        }
        assert_ne!(v.end, BatchEnd::Genesis, "not recorded as rooted at genesis");
        // continuing fetches the rest and only then roots at genesis
        let mut calls = 0;
        let out = walk::fetch_chain(&bob, Some(head), 3, &w.lookup(), |r| {
            calls += 1;
            w.archive("bob").serve(r)
        });
        assert_eq!(out.end, BatchEnd::Genesis);
        assert_eq!(out.records.len(), 8);
        assert!(calls > 1);
    }
}

// acceptance: ARC-11
#[test]
fn a_restore_without_a_head_is_internally_verified_not_complete() {
    let w = long_chain(7);
    let bob = w.kh("bob");
    let holders_newest = w.head("bob");
    let out = walk::fetch_chain(&bob, None, 3, &w.lookup(), |r| w.archive("bob").serve(r));
    assert_eq!(out.end, BatchEnd::Genesis, "every back-pointer matched the following record");
    assert_eq!(out.records.len(), 7);
    assert!(!out.verified_complete, "a restored archive is never marked verified-complete");
    assert!(out.newest_is_holders_claim);
    assert_eq!(out.newest, Some(holders_newest));
    // the head-verified fetch is distinguishable
    let anchored = walk::fetch_chain(&bob, Some(holders_newest), 3, &w.lookup(), |r| w.archive("bob").serve(r));
    assert!(anchored.verified_complete);
    assert!(!anchored.newest_is_holders_claim);
}

// acceptance: ARC-12
#[test]
fn an_earlier_head_presents_an_unbroken_prefix() {
    let w = long_chain(9);
    let bob = w.kh("bob");
    let head = w.head("bob");
    let all = walk::walk(&bob, &head, &w, &w.lookup(), 64).txids();
    let r_j = all[4]; // an earlier head
    let wk = walk::walk(&bob, &r_j, &w, &w.lookup(), 64);
    assert_eq!(wk.root, Root::Genesis);
    assert_eq!(wk.txids(), all[4..].to_vec(), "r_j back to genesis, unbroken");
    for later in &all[..4] {
        assert!(!wk.txids().contains(later), "records after r_j are neither fetched nor reported");
    }
}

// acceptance: ARC-13
#[test]
fn a_walk_stops_at_a_reissue_as_a_checkpoint() {
    let mut w = long_chain(4);
    let bob = w.kh("bob");
    let before = w.head("bob");
    let r_c = w.reissue("bob", "alice", Seqno { series: 5, counter: 2 }, 6);
    w.depart("bob", "alice", Seqno { series: 6, counter: 1 });
    let head = w.head("bob");
    // S no longer serves what lies before r_c
    let mut served: BTreeMap<Txid, Vec<u8>> = BTreeMap::new();
    let mut t = head;
    loop {
        served.insert(t, w.store[&t].clone());
        if t == r_c.txid {
            break;
        }
        t = Record::parse(&w.store[&t]).unwrap().back_pointers_of(&bob).unwrap()[0];
    }
    let wk = walk::walk(&bob, &head, &served, &w.lookup(), 64);
    assert_eq!(wk.root, Root::Checkpoint { at: r_c.txid, beyond: vec![before] });
    assert_eq!(wk.txids(), vec![head, r_c.txid]);
    assert!(wk.is_unbroken());
    // an ordinary unfetchable record is a different result
    served.remove(&r_c.txid);
    let wk2 = walk::walk(&bob, &head, &served, &w.lookup(), 64);
    assert_eq!(wk2.root, Root::Unfetched { at: head, missing: r_c.txid });
}

// acceptance: ARC-16
#[test]
fn pruning_at_a_checkpoint_keeps_the_evidence() {
    let mut w = World::new(&["alice", "bob"]);
    let f1 = w.meet("alice", "bob");
    w.adopt("bob", "alice", f1.txid, 5);
    let f2 = w.meet("alice", "bob");
    for (t, name) in [(f1.txid, "f1"), (f2.txid, "f2")] {
        let ev = Kept { record: w.store[&t].clone(), sealed_capture: format!("capture:{name}").into_bytes(), seed: format!("seed:{name}").into_bytes() };
        w.archive_mut("bob").keep_evidence(t, ev);
    }
    let r_c = w.reissue("bob", "alice", Seqno { series: 5, counter: 0 }, 6);
    let after = w.depart("bob", "alice", Seqno { series: 6, counter: 1 });
    let bob = w.kh("bob");
    let now = r_c.effective + WINDOW_SECONDS + 1;
    assert!(w.archive_mut("bob").prune(&r_c.txid, r_c.effective + 10).is_err(), "never inside the window");
    let released = w.archive_mut("bob").prune(&r_c.txid, now).unwrap();
    assert_eq!(released, 3);
    let wk = walk::walk(&bob, &after.txid, w.archive("bob"), &w.lookup(), 64);
    assert_eq!(wk.root, Root::Checkpoint { at: r_c.txid, beyond: vec![f2.txid] });
    assert_eq!(w.archive("bob").fetch(&f1.txid), None, "chain records before r_c are no longer served");
    for (t, name) in [(f1.txid, "f1"), (f2.txid, "f2")] {
        let ev = w.archive("bob").evidence(&t).expect("still present by txid");
        assert_eq!(ev.record, w.store[&t]);
        assert_eq!(ev.sealed_capture, format!("capture:{name}").into_bytes());
        assert_eq!(ev.seed, format!("seed:{name}").into_bytes());
    }
}

// acceptance: ARC-17
#[test]
fn one_chain_per_key_across_bindings_partitioned_by_series() {
    let mut w = World::new(&["alice", "bob", "carol"]);
    let f1 = w.meet("alice", "bob");
    let a1 = w.adopt("bob", "alice", f1.txid, 11);
    let p1 = w.meet("bob", "alice");
    let f2 = w.meet("carol", "bob");
    let a2 = w.adopt("bob", "carol", f2.txid, 22);
    let p2 = w.meet("bob", "carol");
    let bob = w.kh("bob");
    let out = walk::fetch_chain(&bob, None, 256, &w.lookup(), |r| w.archive("bob").serve(r));
    assert_eq!(out.end, BatchEnd::Genesis);
    assert_eq!(out.records, vec![p2.txid, a2.txid, f2.txid, p1.txid, a1.txid, f1.txid], "one chain, back-pointing across both relationships in signing order");
    assert_eq!(a1.seqno(), Some(Seqno { series: 11, counter: 0 }));
    assert_eq!(a2.seqno(), Some(Seqno { series: 22, counter: 0 }));
    assert_eq!(w.archive("bob").series_occupied().len(), 2);
}
