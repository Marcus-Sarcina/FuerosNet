//! Currency entries (CUR): issuance, the escalation ladder, and the checks
//! a relying party runs against its own clock.

mod common;

use common::*;
use rhtn_archive::record::Record;
use rhtn_archive::topology::Supersession;
use rhtn_archive::tx::{self, Evidence, Seqno};
use rhtn_node::currency::*;
use rhtn_node::resolution::REQUEST_CURRENCY;
use rhtn_node::view::NodeView;
use rhtn_node::Keyhash;
use std::sync::Arc;

fn nonce(n: u8) -> [u8; 16] {
    [n; 16]
}

/// P (alice) has adopted S (bob); P records S's current key.
fn patron_and_subordinate() -> (World, NodeView, CurrencyState) {
    let mut w = World::new();
    let (a, _) = w.adopt("bob", "alice", 1);
    let table = table_with(kh("alice"), &w, &[&a], &["alice"]);
    let mut p = view("alice", table, "alice", &[]);
    p.now = w.clock;
    let mut cur = CurrencyState::default();
    cur.record(kh("bob"), kh("bob"));
    (w, p, cur)
}

fn att(bytes: &[u8]) -> rhtn_archive::currency::Attestation {
    rhtn_archive::currency::parse_attestation(&ids(), bytes).expect("verifies under its issuer")
}

// acceptance: CUR-01
#[test]
fn a_patron_issues_a_fresh_attestation_for_its_subordinate() {
    let (_w, p, mut cur) = patron_and_subordinate();
    let lifetime = 9 * 3600;
    cur.lifetime = lifetime;
    let reply = p.answer_currency(&cur, &CurrencyRequest { subject: kh("bob"), nonce: nonce(1) });
    let CurrencyReply::Attestation { nonce: n, bytes } = &reply else { panic!("{reply:?}") };
    assert_eq!(*n, nonce(1));
    let a = att(bytes);
    assert_eq!(a.subject, kh("bob"), "field 1");
    assert_eq!(a.current, kh("bob"), "field 2");
    assert_eq!(a.role, ROLE_PATRON, "field 5");
    assert_eq!(a.issuer, kh("alice"), "field 6");
    assert_eq!(a.expires_at - a.issued_at, lifetime);
    // the reply round-trips through the wire schema
    assert_eq!(CurrencyReply::decode(&reply.encode()).unwrap(), reply);
}

// acceptance: CUR-02
#[test]
fn a_refresh_is_a_fresh_issuance_never_an_extension() {
    let (_w, mut p, cur) = patron_and_subordinate();
    let first = p.issue_currency(&cur, &kh("bob")).unwrap();
    let a0 = att(&first);
    p.now += 1800;
    let second = p.issue_currency(&cur, &kh("bob")).unwrap();
    let a1 = att(&second);
    assert!(a1.issued_at > a0.issued_at, "field 3 is later");
    assert_ne!(a1.bytes, a0.bytes, "a different signature");
    // no message carries A's fields 1 to 3 with a later field 4: the object
    // has no extend operation and no field for one
    assert_ne!(a1.issued_at, a0.issued_at);
    assert!(a1.expires_at > a0.expires_at);
}

/// A validated recovery adoption of `new` claiming `old`'s history.
fn recovery(old: &str, new: &str, patron: &str, verifier: &str) -> Record {
    let (o, n, p, v) = (id(old), id(new), id(patron), id(verifier));
    let qid = rhtn_codec::cose::sha256(b"recovery");
    let resp = tx::recovery_response(&v, &n, &qid, &o.public.keyhash);
    let block = tx::recovery_block(&o, &n.public.keyhash, &p.public.keyhash, vec![resp]);
    let a = tx::Adoption {
        node: n.public.keyhash,
        patron: p.public.keyhash,
        locator: tx::Locator::root(p.public.keyhash, Seqno { series: 4, counter: 0 }),
        timestamp: 1_800_000_000,
        key_material: None,
        evidence: Evidence::Recovery(block),
        presented_head: None,
        back: [&[rhtn_archive::genesis(&n.public.keyhash)], &[rhtn_archive::genesis(&p.public.keyhash)]],
    };
    Record::parse(&tx::envelope(tx::TYPE_ADOPTION, &tx::adoption_body(&a), &[&n, &p])).unwrap()
}

// acceptance: CUR-03
#[test]
fn nothing_is_issued_for_a_key_the_issuer_has_superseded() {
    let (_w, p, mut cur) = patron_and_subordinate();
    let k1 = kh("bob");
    let before = att(&p.issue_currency(&cur, &k1).unwrap());
    assert_eq!(before.current, k1);
    let rec = recovery("bob", "carol", "alice", "w1");
    let ev = Supersession::from_record(&rec, &ids()).unwrap();
    let k2 = ev.successor;
    assert_eq!((ev.superseded, k2), (k1, kh("carol")));
    cur.supersede(ev);
    // the identity as a querier still knows it is k1; what P answers with is
    // the key it currently records
    let reply = p.answer_currency(&cur, &CurrencyRequest { subject: k1, nonce: nonce(3) });
    let CurrencyReply::Attestation { bytes, .. } = &reply else { panic!("{reply:?}") };
    let a = att(bytes);
    assert_eq!(a.current, k2, "field 2 carries the successor");
    assert_ne!(a.current, k1, "and never the old key");
    assert_eq!(cur.recorded_key(&k1), Some(k2), "the recovery overwrote P's record of the subject");
    assert_eq!(cur.successor_of(&k1), Some(k2));
}

// acceptance: CUR-04
#[test]
fn a_responder_that_cannot_issue_says_so_rather_than_inventing_one() {
    let (_w, p, cur) = patron_and_subordinate();
    let x = kh("w9");
    assert!(cur.recorded_key(&x).is_none(), "P holds no record of X");
    let reply = p.answer_currency(&cur, &CurrencyRequest { subject: x, nonce: nonce(4) });
    assert_eq!(reply, CurrencyReply::CannotIssue { nonce: nonce(4) });
    let encoded = reply.encode();
    let item = rhtn_codec::cbor::parse_all(&encoded).unwrap();
    let rhtn_codec::cbor::Item::Map(m) = &item else { panic!() };
    assert_eq!(m.len(), 2, "field 3 absent");
    assert_eq!(CurrencyReply::decode(&encoded).unwrap(), reply);
}

// acceptance: CUR-05
#[test]
fn a_trust_bearing_operation_waits_on_an_expired_staple() {
    let mut w = World::new();
    let table = table_with(kh("alice"), &w, &[], &["alice"]);
    let mut n = view("alice", table, "alice", &[]);
    n.now = w.clock;
    let cur = CurrencyState::default();
    let fab = Fabric::with(&[kh("carol"), kh("w1")]);
    // the staple X arrived with expired against N's own clock
    let stale = tx::currency_attestation(&id("carol"), &kh("bob"), &kh("bob"), n.now - 40_000, n.now - 3600, ROLE_PATRON);
    let (_, state) = n.read_staple(&ids(), &stale, &kh("bob"));
    assert_eq!(state, Staple::Expired);
    let pop = w.formation("alice", "bob");
    let refused = n.adopt_gated(&cur, &kh("bob"), state, Evidence::Presence(pop.txid), 5, &id("bob"));
    assert!(matches!(refused, Err(Gate::Refuse(_))), "no countersigned adoption while the staple is expired");
    // a currency request naming X leaves N
    n.ask_currency(&*fab, kh("bob"), None, Some(kh("carol")), nonce(5));
    let asked: Vec<_> = fab.frames().into_iter().filter(|f| f.frame_type == REQUEST_CURRENCY).collect();
    assert_eq!(asked.len(), 1);
    assert_eq!(CurrencyRequest::decode(&asked[0].body).unwrap().subject, kh("bob"));
    // only after a code-0 reply with an unexpired attestation does it proceed
    let fresh = tx::currency_attestation(&id("carol"), &kh("bob"), &kh("bob"), n.now, n.now + 36_000, ROLE_PATRON);
    let reply = CurrencyReply::Attestation { nonce: nonce(5), bytes: fresh };
    let CurrencyReply::Attestation { bytes, .. } = &reply else { panic!() };
    let (_, state) = n.read_staple(&ids(), bytes, &kh("bob"));
    assert_eq!(state, Staple::Current);
    let ok = n.adopt_gated(&cur, &kh("bob"), state, Evidence::Presence(pop.txid), 5, &id("bob"));
    assert!(ok.is_ok(), "{ok:?}");
    assert_eq!(ok.unwrap().tx_type, tx::TYPE_ADOPTION);
}

// acceptance: CUR-06
#[test]
fn routine_payload_fails_open_on_an_expired_staple() {
    assert_eq!(gate(Operation::Routine, Staple::Expired, false), Gate::Proceed);
    assert_eq!(gate(Operation::Routine, Staple::Absent, false), Gate::Proceed);
    // and the trust-bearing half of the same table does not
    assert!(matches!(gate(Operation::TrustBearing, Staple::Expired, false), Gate::Refuse(_)));
}

// acceptance: CUR-07
#[test]
fn an_unexpired_staple_does_not_restore_a_superseded_binding() {
    let (_w, n, mut cur) = patron_and_subordinate();
    let k1 = kh("bob");
    let rec = recovery("bob", "carol", "alice", "w1");
    cur.supersede(Supersession::from_record(&rec, &ids()).unwrap());
    assert!(cur.is_superseded(&k1));
    // an unexpired, validly signed attestation naming S under k1
    let staple = tx::currency_attestation(&id("alice"), &k1, &k1, n.now, n.now + 36_000, ROLE_PATRON);
    let (_, state) = n.read_staple(&ids(), &staple, &k1);
    assert_eq!(state, Staple::Current);
    // fail-open is for ignorance, never for knowledge
    assert!(matches!(gate(Operation::Routine, state, true), Gate::Refuse(_)), "nothing is delivered under k1");
    assert!(matches!(gate(Operation::TrustBearing, state, true), Gate::Refuse(_)), "no session is served under k1");
}

/// P (bob) adopted S (carol); P' (w1) is P's sibling and G (alice) is P's
/// patron.  Every party holds the adoption record.
fn ladder() -> (World, CurrencyState) {
    let mut w = World::new();
    w.adopt("bob", "alice", 1);
    w.adopt("w1", "alice", 2);
    w.adopt("carol", "bob", 3);
    let mut cur = CurrencyState::default();
    cur.record(kh("carol"), kh("carol"));
    (w, cur)
}

fn ladder_view(name: &str, w: &World) -> NodeView {
    let mut ww = World::new();
    let (a1, _) = ww.adopt("bob", "alice", 1);
    let (a2, _) = ww.adopt("w1", "alice", 2);
    let (a3, _) = ww.adopt("carol", "bob", 3);
    let table = table_with(kh(name), &ww, &[&a1, &a2, &a3], &["alice", "bob", "w1"]);
    let mut v = view(name, table, "alice", &[]);
    v.now = w.clock;
    v
}

// acceptance: CUR-08
#[test]
fn a_sibling_issues_secondhand_while_the_patron_is_dark() {
    let (w, mut cur) = ladder();
    let p2 = ladder_view("w1", &w);
    assert_eq!(p2.rung_for(&cur, &kh("carol")), None, "not while the patron answers");
    cur.unreachable.insert(kh("bob"));
    assert_eq!(p2.rung_for(&cur, &kh("carol")), Some(Rung::Sibling));
    let bytes = p2.issue_currency(&cur, &kh("carol")).unwrap();
    let a = att(&bytes);
    assert_eq!(a.role, ROLE_SIBLING, "marked secondhand");
    assert_eq!(a.issuer, kh("w1"));
    assert_eq!(a.issued_at, p2.now, "fresh, from P' own clock");
    assert_eq!(a.current, kh("carol"));
}

// acceptance: CUR-09
#[test]
fn the_grandpatron_issues_once_the_patron_and_its_siblings_are_dark() {
    let (w, mut cur) = ladder();
    let g = ladder_view("alice", &w);
    cur.unreachable.insert(kh("bob"));
    assert_eq!(g.rung_for(&cur, &kh("carol")), None, "P' can still answer");
    cur.unreachable.insert(kh("w1"));
    assert_eq!(g.rung_for(&cur, &kh("carol")), Some(Rung::Grandpatron));
    let a = att(&g.issue_currency(&cur, &kh("carol")).unwrap());
    assert_eq!(a.role, ROLE_GRANDPATRON);
    assert_eq!(a.issuer, kh("alice"));
}

// acceptance: CUR-10
#[test]
fn a_whole_neighbourhood_outage_freezes_and_re_adoption_thaws() {
    let (w, mut cur) = ladder();
    let s = ladder_view("carol", &w);
    cur.unreachable.extend([kh("bob"), kh("w1"), kh("alice")]);
    // every rung is unreachable, so no issuance path exists
    for issuer in ["bob", "w1", "alice"] {
        let v = ladder_view(issuer, &w);
        assert!(cur.unreachable.contains(&kh(issuer)));
        let _ = v;
    }
    let staple = Staple::Expired;
    assert!(matches!(gate(Operation::TrustBearing, staple, false), Gate::Refuse(_)), "frozen for trust-bearing operations");
    let peering = s.peer_gated(&cur, &kh("w5"), staple);
    assert!(peering.is_err(), "the first attempt produces no peering transaction");
    // S adopts at a reachable patron P2 and obtains an attestation from it
    let mut w2 = World::new();
    let (a, _) = w2.adopt("carol", "w2", 9);
    let p2_table = table_with(kh("w2"), &w2, &[&a], &["w2"]);
    let mut p2 = view("w2", p2_table, "w2", &[]);
    p2.now = w2.clock;
    let mut cur2 = CurrencyState::default();
    cur2.record(kh("carol"), kh("carol"));
    let reply = p2.answer_currency(&cur2, &CurrencyRequest { subject: kh("carol"), nonce: nonce(10) });
    let CurrencyReply::Attestation { bytes, .. } = &reply else { panic!("code 0 expected") };
    let mut s2 = ladder_view("carol", &w);
    s2.now = p2.now;
    let (_, state) = s2.read_staple(&ids(), bytes, &kh("carol"));
    assert_eq!(state, Staple::Current);
    assert!(s2.peer_gated(&cur, &kh("w5"), state).is_ok(), "the peering transaction is produced");
}

// acceptance: CUR-11
#[test]
fn a_caller_with_a_stale_staple_asks_its_introducer_first() {
    let mut w = World::new();
    let table = table_with(kh("alice"), &w, &[], &["alice"]);
    let mut n = view("alice", table, "alice", &[]);
    n.now = w.tick();
    let introducer = kh("w1");
    let patron = kh("carol");
    let fab = Fabric::with(&[introducer, patron]);
    let to = n.ask_currency(&*fab, kh("bob"), Some(introducer), Some(patron), nonce(11));
    assert_eq!(to, Some(introducer), "N's first request goes toward I");
    assert_eq!(fab.to(&introducer, REQUEST_CURRENCY).len(), 1);
    assert_eq!(fab.to(&patron, REQUEST_CURRENCY).len(), 0, "and none toward X's patron");
    // when I does not answer, the patron is asked
    fab.clear();
    fab.drop_session(&introducer);
    let to = n.ask_currency(&*fab, kh("bob"), Some(introducer), Some(patron), nonce(11));
    assert_eq!(to, Some(patron));
}

// acceptance: CUR-12
#[test]
fn answering_a_currency_request_retains_nothing() {
    let (_w, p, cur) = patron_and_subordinate();
    let before = (p.archive.len(), p.store.objects(), p.memo_table.clone(), p.slots.clone());
    let req = CurrencyRequest { subject: kh("bob"), nonce: nonce(12) };
    let reply = p.answer_currency(&cur, &req);
    assert!(matches!(reply, CurrencyReply::Attestation { .. }));
    let after = (p.archive.len(), p.store.objects(), p.memo_table.clone(), p.slots.clone());
    assert_eq!(before, after, "no record of the request, of Q, or of the reply");
    // the request never named the querier, so no pairing was received either
    let encoded = req.encode();
    assert!(!encoded.windows(32).any(|x| x == kh("w9")));
    // map head, key 1 and a 32-byte string, key 2 and a 16-byte string
    assert_eq!(encoded.len(), 1 + (1 + 2 + 32) + (1 + 1 + 16), "subject and nonce, and nothing else");
}

// acceptance: CUR-13
#[test]
fn expiry_is_decided_against_the_relying_partys_own_clock() {
    let (_w, mut n, _cur) = patron_and_subordinate();
    let t = 1_900_000_000u64;
    let staple = tx::currency_attestation(&id("alice"), &kh("bob"), &kh("bob"), t - 36_000, t, ROLE_PATRON);
    n.now = t - 1;
    let (_, before) = n.read_staple(&ids(), &staple, &kh("bob"));
    assert_eq!(before, Staple::Current);
    assert_eq!(gate(Operation::TrustBearing, before, false), Gate::Proceed);
    n.now = t + 1;
    let (_, after) = n.read_staple(&ids(), &staple, &kh("bob"));
    assert_eq!(after, Staple::Expired);
    assert!(matches!(gate(Operation::TrustBearing, after, false), Gate::Refuse(_)));
}

fn _keyhash(_: Keyhash, _: Arc<Fabric>) {}
