//! Topology entries (TOP), the patron's archive evaluation (ARC-14) and the
//! inquirer's fork report (ARC-15), over a table fed verified transactions
//! directly: propagation is milestone 4's.

mod common;

use common::World;
use rhtn_archive::currency::{self, CurrencyView};
use rhtn_archive::record::{Record, SigStatus};
use rhtn_archive::topology::*;
use rhtn_archive::tx::*;
use rhtn_archive::walk::Fetch;
use rhtn_archive::{Keyhash, Txid};
use rhtn_crypto::verify;
use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

fn set(v: &[Keyhash]) -> BTreeSet<Keyhash> {
    v.iter().copied().collect()
}

fn apply(t: &mut Table, w: &World, rec: &Record) -> Outcome {
    t.apply(rec, &w.lookup(), w, None).unwrap_or_else(|e| panic!("{:?} refused: {e:?}", rec.tx_type))
}

/// The same, for a test that expects a refusal.
fn offer(t: &mut Table, w: &World, rec: &Record) -> Result<Outcome, String> {
    t.apply(rec, &w.lookup(), w, None).map_err(|e| format!("{e:?}"))
}

/// A world with patron alice over carol, and a table holding it.
fn patron_with_one() -> (World, Table) {
    let mut w = World::new(&["alice", "bob", "carol", "alice2", "w1", "w2", "w3", "w4", "w5"]);
    let f = w.meet("alice", "carol");
    let a = w.adopt("carol", "alice", f.txid, 1);
    let mut t = Table::new();
    apply(&mut t, &w, &f);
    apply(&mut t, &w, &a);
    (w, t)
}

// acceptance: TOP-01
#[test]
fn a_verified_adoption_adds_the_subordinate_and_its_siblings() {
    let (mut w, mut t) = patron_with_one();
    let pop = w.meet("alice", "bob");
    let adoption = w.adopt("bob", "alice", pop.txid, 2);
    apply(&mut t, &w, &pop);
    let out = apply(&mut t, &w, &adoption);
    assert_eq!(out.applied, Applied::Adopted);
    let (p, c, n) = (w.kh("alice"), w.kh("carol"), w.kh("bob"));
    assert!(t.patrons(&n).contains(&p));
    assert_eq!(t.subordinates(&p), set(&[c, n]));
    assert!(t.siblings(&c).contains(&n));
    assert!(t.siblings(&n).contains(&c));
}

/// N (bob) under P1 (alice) and P2 (carol), as TOP-02 leaves it.
fn two_patrons() -> (World, Table) {
    let mut w = World::new(&["alice", "bob", "carol", "w1", "w2"]);
    let f = w.meet("alice", "bob");
    let a1 = w.adopt("bob", "alice", f.txid, 1);
    let fw = w.meet("alice", "w1");
    let aw = w.adopt("w1", "alice", fw.txid, 3);
    let (bob, carol) = (w.kh("bob"), w.kh("carol"));
    let block = transfer_block(w.id("alice"), &bob, &carol);
    let a2 = w.adopt_with("bob", "carol", Evidence::Transfer { former: w.kh("alice"), block }, 2, None);
    let mut t = Table::new();
    for r in [&f, &a1, &fw, &aw] {
        apply(&mut t, &w, r);
    }
    let out = apply(&mut t, &w, &a2);
    assert_eq!(out.applied, Applied::Adopted);
    (w, t)
}

// acceptance: TOP-02
#[test]
fn adopting_elsewhere_leaves_the_old_binding_in_view() {
    let (w, t) = two_patrons();
    let (n, p1, p2) = (w.kh("bob"), w.kh("alice"), w.kh("carol"));
    assert_eq!(t.patrons(&n), set(&[p1, p2]), "both bindings, neither replacing the other");
    assert!(t.subordinates(&p1).contains(&n));
    assert!(t.subordinates(&p2).contains(&n));
}

// acceptance: TOP-03
#[test]
fn a_departure_ends_only_the_named_relationship() {
    let (mut w, mut t) = two_patrons();
    let (n, p1, p2, w1) = (w.kh("bob"), w.kh("alice"), w.kh("carol"), w.kh("w1"));
    let dep = w.depart("bob", "alice", Seqno { series: 1, counter: 1 });
    assert_eq!(dep.signers, vec![n], "single signature: the departing node");
    assert_eq!(apply(&mut t, &w, &dep).applied, Applied::Ended);
    assert_eq!(t.patrons(&n), set(&[p2]));
    assert!(!t.subordinates(&p1).contains(&n));
    assert!(!t.siblings(&w1).contains(&n), "no longer a sibling of P1's other children");
    assert!(t.subordinates(&p2).contains(&n), "the binding under P2 is unchanged");
}

// acceptance: TOP-04
#[test]
fn a_departed_node_with_no_other_binding_is_a_root() {
    let (mut w, mut t) = patron_with_one();
    let (n, p) = (w.kh("carol"), w.kh("alice"));
    let dep = w.depart("carol", "alice", Seqno { series: 1, counter: 1 });
    let out = t.apply(&dep, &w.lookup(), &w, None);
    assert!(out.is_ok(), "no error for a patronless node");
    assert_eq!(t.patrons(&n), set(&[]));
    assert!(t.is_node(&n) && t.is_root(&n));
    assert!(!t.subordinates(&p).contains(&n));
}

// acceptance: TOP-05
#[test]
fn a_disavowal_ends_one_relationship_and_nothing_else() {
    let (mut w, mut t) = two_patrons();
    let fm = w.meet("bob", "w2");
    let am = w.adopt("w2", "bob", fm.txid, 9);
    apply(&mut t, &w, &fm);
    apply(&mut t, &w, &am);
    let (n, p, q, m) = (w.kh("bob"), w.kh("alice"), w.kh("carol"), w.kh("w2"));
    let dis = w.disavow("alice", "bob", Some(0));
    assert_eq!(dis.signers, vec![p]);
    assert_eq!(apply(&mut t, &w, &dis).applied, Applied::Ended);
    assert_eq!(t.patrons(&n), set(&[q]));
    assert!(!t.subordinates(&p).contains(&n));
    assert!(t.patrons(&m).contains(&n), "M's binding under N is untouched");
    assert!(t.is_node(&n));
}

// acceptance: TOP-06
#[test]
fn a_disavowal_applies_on_verification_whatever_its_timestamp() {
    let (mut w, mut t) = patron_with_one();
    let (n, p) = (w.kh("carol"), w.kh("alice"));
    let t_now = w.clock;
    let dis = w.disavow_at("alice", "carol", Some(1), t_now + 7 * 86_400);
    let out = t.apply(&dis, &w.lookup(), &w, None);
    assert!(out.is_ok(), "not rejected for its timestamp");
    assert!(!t.subordinates(&p).contains(&n), "removed on verification, not held open until the timestamp");
    assert_eq!(t.status_at(&p, &n, t_now), Some(1), "by the patron's own clock the slot was still filled at t_now");
}

/// G (alice, infra) over P (bob); a proof of presence between P and N (carol).
fn grandpatron_world() -> (World, Record, Record, Record) {
    let mut w = World::new(&["alice", "bob", "carol", "w1"]);
    let f = w.meet("alice", "bob");
    let a = w.adopt("bob", "alice", f.txid, 1);
    let pop = w.meet("bob", "carol");
    (w, f, a, pop)
}

// acceptance: TOP-07
#[test]
fn a_grandpatron_acknowledges_under_standing_policy() {
    let (mut w, f, a, pop) = grandpatron_world();
    let g = w.kh("alice");
    let mut t = Table::with_me(g);
    t.mark_infra(g);
    for r in [&f, &a, &pop] {
        apply(&mut t, &w, r);
    }
    let issuer = AckIssuer { identity: Arc::new(rhtn_crypto::identity::testkit::test_identity("alice")), policy: Arc::new(|_patron, _node| true), now: w.clock + 1 };
    let adoption = w.adopt("carol", "bob", pop.txid, 2);
    // verification and emission in one call: nothing asks anyone anything
    let out = t.apply(&adoption, &w.lookup(), &w, Some(&issuer)).unwrap();
    assert_eq!(out.acks.len(), 1);
    let ack = &out.acks[0];
    assert_eq!(verify::record(&w.lookup(), "SubtreeAck", ack), Ok(()), "G's signature over fields 1-4 with rhtn/1:subtree-ack");
    let item = rhtn_codec::cbor::parse_all(ack).unwrap();
    let rhtn_codec::cbor::Item::Map(m) = &item else { panic!() };
    let field = |k| match rhtn_codec::cbor::map_get(m, k) { Some(rhtn_codec::cbor::Item::Bytes(r)) => ack[r.clone()].to_vec(), _ => panic!() };
    assert_eq!(field(1), adoption.txid.to_vec());
    assert_eq!(field(2), g.to_vec());
    assert_eq!(field(3), w.kh("carol").to_vec());
    assert_eq!(t.acks().len(), 1);
}

// acceptance: TOP-08
#[test]
fn an_adoption_enters_the_table_without_an_acknowledgement() {
    let (mut w, f, a, pop) = grandpatron_world();
    let adoption = w.adopt("carol", "bob", pop.txid, 2);
    let (n, p) = (w.kh("carol"), w.kh("bob"));
    for me in ["bob", "w1"] {
        let mut t = Table::with_me(w.kh(me));
        for r in [&f, &a, &pop] {
            apply(&mut t, &w, r);
        }
        let out = t.apply(&adoption, &w.lookup(), &w, None).unwrap();
        assert_eq!(out.applied, Applied::Adopted, "neither pending nor rejected");
        assert_eq!(t.patrons(&n), set(&[p]));
        assert!(t.subordinates(&p).contains(&n));
        assert!(t.acks().is_empty());
    }
}

// acceptance: TOP-09
#[test]
fn an_acknowledgement_lapses_with_the_relationship_it_describes() {
    for run in 0..2 {
        let (mut w, f, a, pop) = grandpatron_world();
        let adoption = w.adopt("carol", "bob", pop.txid, 2);
        let ack = subtree_ack(w.id("alice"), &adoption.txid, &w.kh("carol"), w.clock);
        let mut h = Table::with_me(w.kh("w1"));
        for r in [&f, &a, &pop, &adoption] {
            apply(&mut h, &w, r);
        }
        assert_eq!(h.take_ack(&w.lookup(), &ack), Ok(true));
        assert_eq!(h.acks().len(), 1);
        let ending = if run == 0 { w.depart("bob", "alice", Seqno { series: 1, counter: 1 }) } else { w.disavow("alice", "bob", Some(2)) };
        apply(&mut h, &w, &ending);
        assert!(h.acks().is_empty(), "run {run}: discarded with no revocation object");
        assert!(h.patrons(&w.kh("carol")).contains(&w.kh("bob")), "N's binding under P is still held");
    }
}

// acceptance: TOP-10
#[test]
fn an_acknowledgement_alone_creates_no_binding() {
    let (mut w, f, a, _pop) = grandpatron_world();
    let mut h = Table::with_me(w.kh("w1"));
    apply(&mut h, &w, &f);
    apply(&mut h, &w, &a);
    let unknown_adoption: Txid = rhtn_codec::cose::sha256(b"an adoption H never saw");
    let now = w.tick();
    let ack = subtree_ack(w.id("alice"), &unknown_adoption, &w.kh("carol"), now);
    assert_eq!(verify::record(&w.lookup(), "SubtreeAck", &ack), Ok(()), "the signature verifies");
    assert_eq!(h.take_ack(&w.lookup(), &ack), Ok(false));
    assert_eq!(h.patrons(&w.kh("carol")), set(&[]));
    assert!(!h.subordinates(&w.kh("bob")).contains(&w.kh("carol")));
    assert!(h.acks().is_empty());
}

// acceptance: TOP-11
#[test]
fn two_genesis_identities_form_a_subnet() {
    let mut w = World::new(&["alice", "bob"]);
    let (a, b) = (w.kh("alice"), w.kh("bob"));
    assert!(w.archive("alice").is_empty() && w.archive("bob").is_empty());
    let f = w.meet("alice", "bob");
    assert_eq!(f.field_uint(6), Some(1), "subtype 1");
    assert_eq!(f.back, vec![vec![rhtn_archive::genesis(&a)], vec![rhtn_archive::genesis(&b)]]);
    let adoption = w.adopt("bob", "alice", f.txid, 1);
    for me in [a, b] {
        let mut t = Table::with_me(me);
        apply(&mut t, &w, &f);
        apply(&mut t, &w, &adoption);
        assert_eq!(t.patrons(&a), set(&[]));
        assert_eq!(t.subordinates(&a), set(&[b]));
        assert_eq!(t.patrons(&b), set(&[a]));
        assert!(t.is_root(&a));
    }
    for who in ["alice", "bob"] {
        assert_eq!(w.archive(who).len(), 2);
        assert_eq!(w.archive(who).heads(), vec![adoption.txid]);
    }
    assert_eq!(adoption.back, vec![vec![f.txid], vec![f.txid]], "the adoption's back-pointer for each signer names the formation record");
}

// acceptance: TOP-12
#[test]
fn a_presence_record_between_other_parties_does_not_satisfy_an_adoption() {
    let mut w = World::new(&["alice", "bob", "carol"]);
    let pz = w.meet("alice", "carol"); // P and Z
    let adoption = w.adopt("bob", "alice", pz.txid, 1); // N under P naming it
    assert_eq!(adoption.check_signatures(&w.lookup()), SigStatus::Verified, "structurally valid");
    let mut t = Table::new();
    apply(&mut t, &w, &pz);
    let out = t.apply(&adoption, &w.lookup(), &w, None);
    assert!(matches!(out, Err(Refusal::Evidence(_))), "{out:?}");
    assert!(!t.subordinates(&w.kh("alice")).contains(&w.kh("bob")));
}

// acceptance: TOP-14
#[test]
fn a_proposed_patron_is_refused_only_on_positive_knowledge() {
    let mut w = World::new(&["alice", "bob", "carol", "w1"]);
    let n = w.kh("alice");
    let f1 = w.meet("alice", "bob");
    let a1 = w.adopt("bob", "alice", f1.txid, 1); // M under N
    let f2 = w.meet("bob", "carol");
    let a2 = w.adopt("carol", "bob", f2.txid, 2); // M2 under M
    let mut t = Table::with_me(n);
    for r in [&f1, &a1, &f2, &a2] {
        apply(&mut t, &w, r);
    }
    let m2 = w.kh("carol");
    assert_eq!(t.propose_patron(&n, &m2), Err(Refusal::Cycle { below: m2 }), "M2 lies below N");
    let x = w.kh("w1");
    assert!(!t.is_node(&x));
    assert_eq!(t.propose_patron(&n, &x), Ok(()), "nothing known about X: not refused");
    let pop = w.meet("alice", "w1");
    let adoption = w.adopt("alice", "w1", pop.txid, 3);
    assert_eq!(adoption.signers.len(), 2);
}

/// A recovery adoption of `new` under `patron` claiming `old`'s history:
/// the old key's successor statement and one verifier's match.
fn recover(w: &mut World, old: &str, new: &str, patron: &str, verifier: &str, series: u32) -> Record {
    let qid = rhtn_codec::cose::sha256(format!("query:{old}:{new}:{patron}").as_bytes());
    let resp = recovery_response(w.id(verifier), w.id(new), &qid, &w.kh(old));
    let block = recovery_block(w.id(old), &w.kh(new), &w.kh(patron), vec![resp]);
    w.adopt_with(new, patron, Evidence::Recovery(block), series, None)
}

// acceptance: TOP-15
#[test]
fn a_recovery_replaces_the_old_key_inside_the_horizon() {
    let mut w = World::new(&["alice", "bob", "carol", "w1"]);
    let f = w.meet("alice", "bob");
    let a = w.adopt("bob", "alice", f.txid, 1);
    let mut t = Table::new();
    apply(&mut t, &w, &f);
    apply(&mut t, &w, &a);
    let (p, k_old, k_new) = (w.kh("alice"), w.kh("bob"), w.kh("carol"));
    let rec = recover(&mut w, "bob", "carol", "alice", "w1", 2);
    assert_eq!(rec.check_signatures(&w.lookup()), SigStatus::Verified, "passes the evidence gate");
    let out = apply(&mut t, &w, &rec);
    assert_eq!(out.applied, Applied::Replaced { prior: k_old, successor: k_new });
    assert_eq!(t.patrons(&k_new), set(&[p]));
    assert!(!t.subordinates(&p).contains(&k_old));
    assert_eq!(t.current_key(&k_old), k_new);
    assert_eq!(t.current_key(&k_new), k_new, "exactly one current key for the identity");
}

// acceptance: TOP-16
// acceptance: REC-12
#[test]
fn competing_recoveries_resolve_to_one_current_key_by_patron_trust() {
    let names = ["alice", "bob", "carol", "alice2", "w1", "w2", "w3"];
    for preferred in ["w1", "w2"] {
        let mut w = World::new(&names);
        let f = w.meet("alice", "bob");
        let a = w.adopt("bob", "alice", f.txid, 1);
        let ra = recover(&mut w, "bob", "carol", "w1", "w3", 2); // K_a under P_a
        let rb = recover(&mut w, "bob", "alice2", "w2", "w3", 3); // K_b under P_b
        let (k_old, k_a, k_b, p_a, p_b) = (w.kh("bob"), w.kh("carol"), w.kh("alice2"), w.kh("w1"), w.kh("w2"));
        let want = if preferred == "w1" { k_a } else { k_b };
        let pref = w.kh(preferred);
        for order in [[&ra, &rb], [&rb, &ra]] {
            let mut t = Table::new();
            t.prefer = Some(Arc::new(move |x: &Keyhash, y: &Keyhash| if *x == pref && *y != pref { Ordering::Greater } else if *y == pref && *x != pref { Ordering::Less } else { Ordering::Equal }));
            apply(&mut t, &w, &f);
            apply(&mut t, &w, &a);
            for r in order {
                apply(&mut t, &w, r);
            }
            assert_eq!(t.current_key(&k_old), want, "preferring {preferred}");
            assert_eq!(t.patrons(&k_a), set(&[p_a]));
            assert_eq!(t.patrons(&k_b), set(&[p_b]));
            assert!(t.patrons(&k_old).is_empty());
            // the other successor is a valid node, and not the continuation
            let other = if want == k_a { k_b } else { k_a };
            assert!(t.is_node(&other));
            assert_eq!(t.current_key(&other), other, "its own key, bound under its own patron");
            assert_ne!(t.current_key(&k_old), other);
        }
    }
}

// acceptance: TOP-17
#[test]
fn a_disavowal_is_ordered_within_the_slot_by_the_patrons_clock() {
    let mut w = World::new(&["alice", "bob"]);
    let (p, n) = (w.kh("alice"), w.kh("bob"));
    let f = w.meet("alice", "bob");
    let t1 = w.clock + 1000;
    let a1 = w.adopt_at("bob", "alice", f.txid, 1, t1);
    let t2 = t1 + 1000;
    let d = w.disavow_at("alice", "bob", Some(0), t2);
    let t3 = t2 + 1000;
    let f2 = w.meet("alice", "bob");
    let a2 = w.adopt_at("bob", "alice", f2.txid, 2, t3.max(f2.effective));
    let t3 = a2.time;
    let records = [&a1, &d, &a2];
    let orders = [[0, 1, 2], [0, 2, 1], [1, 0, 2], [1, 2, 0], [2, 0, 1], [2, 1, 0]];
    for order in orders {
        let mut t = Table::new();
        apply(&mut t, &w, &f);
        apply(&mut t, &w, &f2);
        for i in order {
            t.apply(records[i], &w.lookup(), &w, None).unwrap();
        }
        assert_eq!(t.slot_history(&p, &n), vec![(t1, Some(1)), (t2, None), (t3, Some(2))], "arrival order {order:?}");
        assert_eq!(t.status_at(&p, &n, t1), Some(1));
        assert_eq!(t.status_at(&p, &n, t2), None);
        assert_eq!(t.status_at(&p, &n, t3), Some(2));
        assert_eq!(t.patrons(&n), set(&[p]));
    }
}

// acceptance: ARC-14
#[test]
fn a_presented_archive_is_weighed_by_known_counterparties_only() {
    let mut w = World::new(&["alice", "bob", "carol", "w1", "w2", "w3", "w4", "w5"]);
    let f = w.meet("alice", "bob");
    let a = w.adopt("bob", "alice", f.txid, 1);
    let fc = w.meet("bob", "carol");
    let chain_a: Vec<Record> = vec![fc.clone(), a.clone(), f.clone()];
    let mut chain_b = chain_a.clone();
    for s in ["w1", "w2", "w3", "w4", "w5"] {
        chain_b.insert(0, w.meet("bob", s));
        chain_b.insert(0, w.meet("bob", s));
    }
    let known: BTreeSet<Keyhash> = set(&[w.kh("alice"), w.kh("carol")]);
    let bob = w.kh("bob");
    let ta = initial_trust(&known, &bob, &chain_a);
    let tb = initial_trust(&known, &bob, &chain_b);
    assert_eq!(ta, tb, "the strangers' records are no evidence");
    assert_eq!(ta.by_counterparty.keys().copied().collect::<BTreeSet<_>>(), known);
    assert_eq!(ta.by_counterparty[&w.kh("alice")].len(), 2);
    assert_eq!(ta.by_counterparty[&w.kh("carol")], vec![fc.txid]);
    let InitialTrust { by_counterparty: _ } = ta; // no other field exists to total
}

// acceptance: ARC-15
#[test]
fn divergent_currency_assertions_are_a_fork_and_both_patrons_are_told() {
    let w = World::new(&["alice", "bob", "carol", "alice2"]);
    let x = w.kh("bob");
    let a1 = currency_attestation(w.id("alice"), &x, &x, w.clock, w.clock + 36_000, 0);
    let a2 = currency_attestation(w.id("carol"), &x, &w.kh("alice2"), w.clock, w.clock + 36_000, 0);
    let ids = w.lookup();
    let p1 = currency::parse_attestation(&ids, &a1).unwrap();
    let p2 = currency::parse_attestation(&ids, &a2).unwrap();
    let view = currency::assess(vec![p1.clone(), p2.clone()]);
    let CurrencyView::Fork(kept) = &view else { panic!("{view:?}") };
    assert_eq!(kept.len(), 2, "both retained, neither chosen");
    let mut told: Vec<Keyhash> = Vec::new();
    assert_eq!(currency::notify_fork(&view, |p| told.push(*p)), 2);
    assert_eq!(set(&told), set(&[w.kh("alice"), w.kh("carol")]));
    // agreement is not a fork
    let a3 = currency_attestation(w.id("carol"), &x, &x, w.clock, w.clock + 36_000, 0);
    let p3 = currency::parse_attestation(&ids, &a3).unwrap();
    assert!(matches!(currency::assess(vec![p1, p3]), CurrencyView::Attested(_)));
    let _: BTreeMap<Txid, Vec<u8>> = BTreeMap::new();
    let _ = <World as Fetch>::fetch;
}

// acceptance: TOP-18
#[test]
fn a_departure_ends_its_series_in_whatever_order_it_arrives() {
    let mut w = World::new(&["alice", "bob"]);
    let (p, n) = (w.kh("alice"), w.kh("bob"));
    let f = w.meet("alice", "bob");
    let a1 = w.adopt("bob", "alice", f.txid, 1);
    let d = w.depart("bob", "alice", Seqno { series: 1, counter: 1 });
    // the reviewer's case: the departure before its own adoption
    let mut t = Table::with_me(p);
    apply(&mut t, &w, &f);
    apply(&mut t, &w, &d);
    assert_eq!(apply(&mut t, &w, &a1).applied, Applied::Adopted);
    assert!(!t.patrons(&n).contains(&p), "the held departure settles against the adoption it names");
    // a re-adoption on a new series, in every arrival order: series 1 ends
    // and series 2 stands
    let f2 = w.meet("alice", "bob");
    let a2 = w.adopt("bob", "alice", f2.txid, 2);
    let records = [&a1, &d, &a2];
    let orders = [[0, 1, 2], [0, 2, 1], [1, 0, 2], [1, 2, 0], [2, 0, 1], [2, 1, 0]];
    for order in orders {
        let mut t = Table::with_me(p);
        apply(&mut t, &w, &f);
        apply(&mut t, &w, &f2);
        for i in order {
            apply(&mut t, &w, records[i]);
        }
        let open: Vec<u32> = t.bindings().iter().filter(|b| b.node == n && b.patron == p && b.open()).map(|b| b.series).collect();
        assert_eq!(open, vec![2], "arrival order {order:?}: series 1 ended, series 2 open");
        assert_eq!(t.patrons(&n), set(&[p]), "arrival order {order:?}");
    }
    // the departure applied twice ends nothing twice and holds nothing new
    let mut t = Table::with_me(p);
    apply(&mut t, &w, &f);
    apply(&mut t, &w, &a1);
    assert_eq!(apply(&mut t, &w, &d).applied, Applied::Ended);
    assert_eq!(apply(&mut t, &w, &d).applied, Applied::Nothing);
}

/// A fetch that answers every txid with one record's bytes.
struct One(Vec<u8>);
impl Fetch for One {
    fn fetch(&self, _: &Txid) -> Option<Vec<u8>> {
        Some(self.0.clone())
    }
}

// acceptance: TOP-19
#[test]
fn dereferenced_evidence_counts_only_once_its_signatures_verify() {
    let mut w = World::new(&["alice", "bob", "carol"]);
    let f = w.meet("alice", "bob");
    let a = w.adopt("bob", "alice", f.txid, 1);
    // the record's body and txid untouched, one signature byte flipped
    let mut forged = f.bytes.clone();
    *forged.last_mut().unwrap() ^= 1;
    assert!(matches!(Record::parse(&forged).unwrap().check_signatures(&w.lookup()), SigStatus::Invalid(_)));
    let mut t = Table::with_me(w.kh("alice"));
    assert!(t.apply(&a, &w.lookup(), &One(forged.clone()), None).is_err(), "required evaluation refuses");
    assert!(t.apply_with(&a, &w.lookup(), &One(forged), None, Evaluation::Deferred).is_err(), "and a failing record is never evidence, deferred or not");
    assert!(t.patrons(&w.kh("bob")).is_empty());
    // a witnessed record whose witness key this holder lacks: unevaluated
    // where deferred, refused naming the key where required
    let f2 = w.meet("alice", "carol");
    let a2 = w.adopt("carol", "alice", f2.txid, 2);
    let without_witness: Vec<_> = w.lookup().into_iter().filter(|i| i.keyhash != w.kh("witness")).collect();
    let mut t = Table::with_me(w.kh("alice"));
    let e = t.apply(&a2, &without_witness, &w, None).unwrap_err();
    assert!(format!("{e:?}").contains("unverifiable"), "{e:?}");
    let mut t = Table::with_me(w.kh("alice"));
    t.apply_with(&a2, &without_witness, &w, None, Evaluation::Deferred).unwrap();
    let b = t.bindings().iter().find(|b| b.adoption == a2.txid).unwrap().clone();
    assert_eq!(b.evidence, EvidenceStatus::Unevaluated);
    // with every key, the same record satisfies
    let mut t = Table::with_me(w.kh("alice"));
    t.apply(&a2, &w.lookup(), &w, None).unwrap();
    let b = t.bindings().iter().find(|b| b.adoption == a2.txid).unwrap().clone();
    assert_eq!(b.evidence, EvidenceStatus::Satisfied);
}

// acceptance: TOP-20
#[test]
fn a_reissue_advances_its_relationship_and_a_departure_in_that_series_ends_it() {
    let mut w = World::new(&["alice", "bob", "carol"]);
    let f = w.meet("alice", "bob");
    let a = w.adopt("bob", "alice", f.txid, 1);
    let r = w.reissue("bob", "alice", Seqno { series: 1, counter: 5 }, 2);
    let d = w.depart("bob", "alice", Seqno { series: 2, counter: 1 });
    let apply = |t: &mut Table, rec: &Record, w: &World| t.apply(rec, &w.lookup(), &w.store, None);

    // in order: the reissue moves the relationship and leaves it open
    let mut t = Table::with_me(w.kh("alice"));
    apply(&mut t, &a, &w).expect("adoption");
    apply(&mut t, &r, &w).expect("reissue");
    assert!(t.subordinates(&w.kh("alice")).contains(&w.kh("bob")), "a reissue does not end a relationship");
    assert_eq!(t.bindings().iter().find(|b| b.open()).map(|b| b.series), Some(2), "it is in the series it entered");
    // and the departure naming that series ends it
    apply(&mut t, &d, &w).expect("departure");
    assert!(t.subordinates(&w.kh("alice")).is_empty(), "the departure ends the relationship the reissue advanced");

    // a departure naming the series left ends nothing
    let mut t = Table::with_me(w.kh("alice"));
    apply(&mut t, &a, &w).expect("adoption");
    apply(&mut t, &r, &w).expect("reissue");
    let stale = w.depart("bob", "alice", Seqno { series: 1, counter: 9 });
    apply(&mut t, &stale, &w).expect("applies");
    assert!(t.subordinates(&w.kh("alice")).contains(&w.kh("bob")), "the series it names is not the one open");

    // the reissue arriving before its adoption is held, and settles
    let mut t = Table::with_me(w.kh("alice"));
    apply(&mut t, &r, &w).expect("reissue");
    assert!(t.subordinates(&w.kh("alice")).is_empty(), "nothing to advance yet");
    apply(&mut t, &a, &w).expect("adoption");
    assert_eq!(t.bindings().iter().find(|b| b.open()).map(|b| b.series), Some(2), "settled when its adoption arrived");
    apply(&mut t, &d, &w).expect("departure");
    assert!(t.subordinates(&w.kh("alice")).is_empty());

    // a re-adoption opening its own series is a different binding, which
    // the earlier reissue never touched
    let mut t = Table::with_me(w.kh("alice"));
    apply(&mut t, &a, &w).expect("adoption");
    apply(&mut t, &r, &w).expect("reissue");
    apply(&mut t, &d, &w).expect("departure");
    let f2 = w.meet("alice", "bob");
    let again = w.adopt("bob", "alice", f2.txid, 7);
    apply(&mut t, &again, &w).expect("re-adoption");
    assert_eq!(t.bindings().iter().filter(|b| b.open()).map(|b| b.series).collect::<Vec<_>>(), vec![7]);
}

// acceptance: TOP-29
#[test]
fn a_disavowal_that_does_not_verify_ends_nothing_however_it_is_timed() {
    let (mut w, mut t) = two_patrons();
    let (n, p) = (w.kh("bob"), w.kh("alice"));
    assert!(t.patrons(&n).contains(&p), "the binding stands before any of this");

    // **the negative of TOP-06.** A disavowal takes effect when signed and
    // its timestamp orders it and nothing else; what makes it take effect
    // is that it verifies. One that does not is not a late disavowal, it
    // is not a disavowal.
    let mut forged = w.disavow("alice", "bob", Some(0));
    let last = forged.bytes.len() - 1;
    forged.bytes[last] ^= 0xff;
    let forged = Record::parse(&forged.bytes).expect("still parses; it is the signature that is wrong");
    assert!(offer(&mut t, &w, &forged).is_err(), "a broken signature is refused");
    assert!(t.patrons(&n).contains(&p), "and the relationship it named is untouched");

    // nor does one naming a relationship that does not exist end anything
    let stranger = w.disavow("alice", "w2", Some(0));
    let before = t.subordinates(&p);
    let _ = offer(&mut t, &w, &stranger);
    assert_eq!(t.subordinates(&p), before, "a pair with no binding has none to end");
}

// acceptance: TOP-30
#[test]
fn a_disavowal_orders_nothing_another_party_signed() {
    let (mut w, mut t) = two_patrons();
    let (n, q) = (w.kh("bob"), w.kh("carol"));

    // carol adopted bob after alice did; alice now disavows bob at a
    // timestamp of its own choosing, years past anything carol signed
    assert!(t.patrons(&n).contains(&q), "the second binding stands");
    let far = w.disavow_at("alice", "bob", Some(1), 4_000_000_000);
    assert_eq!(apply(&mut t, &w, &far).applied, Applied::Ended);

    // **field 3 orders the disavowal within the patron's own slot**
    // (§4.3): the adoption that filled the slot and this record came from
    // one party's clock. A patron can stamp its own record whenever it
    // likes — §4.3 has no notice period for exactly that reason — and the
    // stamp reaches nothing another party signed.
    assert!(t.patrons(&n).contains(&q), "carol's binding is untouched by alice's clock");
    assert!(!t.subordinates(&w.kh("alice")).contains(&n), "and alice's own is ended");
    assert!(t.is_node(&n), "bob is still a node, with one patron instead of two");
}

// acceptance: TOP-31
#[test]
fn a_disavowals_band_is_readable_without_a_lookup_table() {
    let (mut w, mut t) = two_patrons();
    let n = w.kh("bob");

    // **codes 0-31 are without prejudice, 32-63 with** (`wire-format.md`
    // §4.3), and bit 5 carries the distinction so a policy can act on a
    // code it has never seen without a lookup table and without a
    // specification update
    let dis = w.disavow("alice", "bob", Some(4));
    assert_eq!(apply(&mut t, &w, &dis).applied, Applied::Ended);
    assert_eq!(band(&t, &n, &w.kh("alice")), Some(false), "code 4 is incompatible subnet membership, and alleges nothing");

    let (mut w2, mut t2) = two_patrons();
    let adverse = w2.disavow("alice", "bob", Some(32));
    assert_eq!(apply(&mut t2, &w2, &adverse).applied, Applied::Ended);
    assert_eq!(band(&t2, &w2.kh("bob"), &w2.kh("alice")), Some(true), "32 opens the upper band");
}

// acceptance: TOP-32
#[test]
fn an_unfamiliar_disavowal_code_is_banded_rather_than_refused_and_none_bands_as_nothing() {
    // **an exception to the unknown-enum rule** (`wire-format.md` §4.3):
    // rejecting an unfamiliar code would make every future assignment a
    // flag day, which is the thing the banding exists to prevent
    let (mut w, mut t) = two_patrons();
    let unassigned = w.disavow("alice", "bob", Some(47));
    assert_eq!(apply(&mut t, &w, &unassigned).applied, Applied::Ended, "an unassigned code is not a reason to refuse");
    assert_eq!(band(&t, &w.kh("bob"), &w.kh("alice")), Some(true), "47 sits in the upper band, so the patron judged");

    // and a disavowal stating no reason bands as nothing rather than as
    // the benign half: saying nothing is not saying without prejudice
    let (mut w2, mut t2) = two_patrons();
    let silent = w2.disavow("alice", "bob", None);
    assert_eq!(apply(&mut t2, &w2, &silent).applied, Applied::Ended);
    assert_eq!(band(&t2, &w2.kh("bob"), &w2.kh("alice")), None, "no code is no judgment either way");
}

/// Whether the patron judged, from the ending the table recorded.
fn band(t: &Table, node: &[u8; 32], patron: &[u8; 32]) -> Option<bool> {
    t.bindings().iter().find(|b| b.node == *node && b.patron == *patron).and_then(|b| b.end.clone()).expect("an ending").2.with_prejudice()
}
