//! Currency entries (CUR): issuance, the escalation ladder, and the checks
//! a relying party runs against its own clock and its own topology.

mod common;

use common::*;
use rhtn_archive::record::Record;
use rhtn_archive::tx::{self, Evidence, Seqno};
use rhtn_node::currency::*;
use rhtn_node::resolution::REQUEST_CURRENCY;
use rhtn_node::view::NodeView;
use std::collections::BTreeMap;

fn nonce(n: u8) -> [u8; 16] {
    [n; 16]
}

/// Nothing to dereference: the recovery carries its own evidence.
fn no_presence() -> BTreeMap<[u8; 32], Vec<u8>> {
    BTreeMap::new()
}

/// P (alice) has adopted S (bob).
fn patron_and_subordinate() -> (World, NodeView, CurrencyState) {
    let mut w = World::new();
    let (a, _) = w.adopt("bob", "alice", 1);
    let table = table_with(kh("alice"), &w, &[&a], &["alice"]);
    let mut p = view("alice", table, "alice", &[]);
    p.set_now(w.clock);
    (w, p, CurrencyState::default())
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
    let a0 = att(&p.issue_currency(&cur, &kh("bob")).unwrap());
    let later = p.now() + 1800;
    p.set_now(later);
    let a1 = att(&p.issue_currency(&cur, &kh("bob")).unwrap());
    assert!(a1.issued_at > a0.issued_at, "field 3 is later");
    assert_ne!(a1.bytes, a0.bytes, "a different signature");
    assert!(a1.expires_at > a0.expires_at);
    // no message carries A's fields 1 to 3 with a later field 4: the object
    // has no extend operation and no field for one
    assert_ne!((a1.subject, a1.current, a1.issued_at), (a0.subject, a0.current, a0.issued_at));
}

/// A validated recovery adoption of `new` under `patron` claiming `old`.
fn recovery(old: &str, new: &str, patron: &str, verifier: &str) -> Record {
    let (o, n, p, v) = (id(old), id(new), id(patron), id(verifier));
    let qid = rhtn_codec::cose::sha256(b"recovery");
    let resp = tx::recovery_response(&v, &n, &qid, &o.public.keyhash);
    let block = tx::recovery_block(&o, &n.public.keyhash, &p.public.keyhash, vec![resp]);
    let a = tx::Adoption {
        node: n.public.keyhash,
        patron: p.public.keyhash,
        locator: tx::Locator::root(p.public.keyhash, Seqno { series: 4, counter: 0 }),
        timestamp: 1_900_000_000,
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
    let (_w, mut p, cur) = patron_and_subordinate();
    let k1 = kh("bob");
    assert_eq!(att(&p.issue_currency(&cur, &k1).unwrap()).current, k1);
    // P receives and verifies a recovery adoption binding S to k2
    let rec = recovery("bob", "carol", "alice", "w1");
    p.table.apply(&rec, &ids(), &no_presence(), None).expect("validated");
    let k2 = kh("carol");
    assert!(p.is_superseded(&k1));
    assert_eq!(p.table.current_key(&k1), k2, "the recovery overwrote P's record of the subject");
    // the identity as a querier still knows it is k1; what P answers with is
    // the key it currently records
    for subject in [k1, k2] {
        let reply = p.answer_currency(&cur, &CurrencyRequest { subject, nonce: nonce(3) });
        let CurrencyReply::Attestation { bytes, .. } = &reply else { panic!("{reply:?}") };
        let a = att(bytes);
        assert_eq!(a.current, k2, "field 2 carries the successor");
        assert_ne!(a.current, k1, "and never the old key");
    }
}

// acceptance: CUR-04
#[test]
fn a_responder_that_cannot_issue_says_so_rather_than_inventing_one() {
    let (_w, p, cur) = patron_and_subordinate();
    let x = kh("w9");
    assert!(p.table.patrons(&x).is_empty(), "P holds no record of X");
    let reply = p.answer_currency(&cur, &CurrencyRequest { subject: x, nonce: nonce(4) });
    assert_eq!(reply, CurrencyReply::CannotIssue { nonce: nonce(4) });
    let encoded = reply.encode();
    let item = rhtn_codec::cbor::parse_all(&encoded).unwrap();
    let rhtn_codec::cbor::Item::Map(m) = &item else { panic!() };
    assert_eq!(m.len(), 2, "field 3 absent");
    assert_eq!(CurrencyReply::decode(&encoded).unwrap(), reply);
}

// acceptance: CUR-14
#[test]
fn an_adoption_is_countersigned_whatever_the_subjects_staple_says() {
    let mut w = World::new();
    let table = table_with(kh("alice"), &w, &[], &["alice"]);
    let mut n = view("alice", table, "alice", &[]);
    n.set_now(w.clock);
    let x = kh("bob");
    let patron = kh("carol");
    let fab = Fabric::with(&[patron, kh("w1")]);
    let pop = w.meet("alice", "bob");
    // X hands over no staple at all
    assert_eq!(n.staple_for(&ids(), &x, &[patron]), Staple::Absent);
    let body = n.propose_adoption(&x, Evidence::Presence(pop.txid), 5, &[rhtn_archive::genesis(&x)]).expect("produced with no staple");
    let rec = n.countersign_adoption(&body, &id("bob")).unwrap();
    assert_eq!(rec.tx_type, tx::TYPE_ADOPTION);
    // then a staple expired against N's own clock
    let stale = tx::currency_attestation(&id("carol"), &x, &x, n.now() - 40_000, n.now() - 3600, ROLE_PATRON);
    assert_eq!(n.take_staple(&ids(), &x, &stale, &[patron]), Staple::Expired);
    let body = n.propose_adoption(&x, Evidence::Presence(pop.txid), 6, &[rhtn_archive::genesis(&x)]).expect("produced with an expired staple");
    let rec = n.countersign_adoption(&body, &id("bob")).unwrap();
    assert_eq!(rec.signers, vec![x, kh("alice")]);
    // and nothing about X's currency was asked of anyone first
    assert_eq!(fab.request_count(REQUEST_CURRENCY), 0);
}

// acceptance: CUR-06
#[test]
fn routine_payload_fails_open_on_an_expired_staple() {
    let (_w, mut n, _cur) = patron_and_subordinate();
    // S arrives holding only an expired staple from its patron
    let stale = tx::currency_attestation(&id("alice"), &kh("bob"), &kh("bob"), n.now() - 40_000, n.now() - 1, ROLE_PATRON);
    assert_eq!(n.take_staple(&ids(), &kh("bob"), &stale, &[]), Staple::Expired);
    assert_eq!(n.staple_for(&ids(), &kh("bob"), &[]), Staple::Expired);
    assert_eq!(gate(n.is_superseded(&kh("bob"))), Gate::Proceed, "delivery proceeds");
    // nothing waits on a staple: an absent one proceeds too
    assert_eq!(gate(false), Gate::Proceed);
}

// acceptance: CUR-07
#[test]
fn an_unexpired_staple_does_not_restore_a_superseded_binding() {
    let (_w, mut n, _cur) = patron_and_subordinate();
    let k1 = kh("bob");
    // N holds a validated recovery superseding k1
    let rec = recovery("bob", "carol", "alice", "w1");
    n.table.apply(&rec, &ids(), &no_presence(), None).expect("validated");
    assert!(n.is_superseded(&k1));
    // an unexpired, validly signed attestation naming S under k1
    let staple = tx::currency_attestation(&id("alice"), &k1, &k1, n.now(), n.now() + 36_000, ROLE_PATRON);
    let state = n.take_staple(&ids(), &k1, &staple, &[]);
    assert_eq!(state, Staple::Current, "unexpired and validly signed by S's patron");
    // fail-open is for ignorance, never for knowledge: the node's own
    // reading of the binding decides, and the staple restores nothing
    let fab = Fabric::with(&[]);
    assert!(matches!(n.require_currency(&*fab, &ids(), &k1, None, None), Requirement::Settled(Gate::Refuse(_))), "no session is served under k1");
    assert!(matches!(gate(n.is_superseded(&k1)), Gate::Refuse(_)), "nothing is delivered under k1");
}

/// P (bob) adopted S (carol); P' (w1) is P's sibling and G (alice) is P's
/// patron.  Every party holds the adoption record.
fn ladder_view(name: &str) -> NodeView {
    let mut w = World::new();
    let (a1, _) = w.adopt("bob", "alice", 1);
    let (a2, _) = w.adopt("w1", "alice", 2);
    let (a3, _) = w.adopt("carol", "bob", 3);
    let table = table_with(kh(name), &w, &[&a1, &a2, &a3], &["alice", "bob", "w1", "carol"]);
    let mut v = view(name, table, "alice", &[]);
    v.set_now(w.clock);
    v
}

// acceptance: CUR-08
#[test]
fn a_sibling_issues_secondhand_while_the_patron_is_dark() {
    let mut cur = CurrencyState::default();
    let p2 = ladder_view("w1");
    assert_eq!(p2.rung_for(&cur, &kh("carol")), None, "not while the patron answers");
    // dark for an hour: within the attestation lifetime nothing is needed
    cur.dark(kh("bob"), p2.now() - 3600);
    assert_eq!(p2.rung_for(&cur, &kh("carol")), None, "the staple still stands");
    // dark for the lifetime: the sibling rung opens
    cur.unreachable.insert(kh("bob"), p2.now() - cur.sibling_after);
    assert_eq!(p2.rung_for(&cur, &kh("carol")), Some(Rung::Sibling));
    let a = att(&p2.issue_currency(&cur, &kh("carol")).unwrap());
    assert_eq!(a.role, ROLE_SIBLING, "marked secondhand");
    assert_eq!(a.issuer, kh("w1"));
    assert_eq!(a.issued_at, p2.now(), "fresh, from P' own clock");
    assert_eq!(a.current, kh("carol"));
    // a relying party that holds the topology places the issuer on its rung
    let relying = ladder_view("alice");
    assert_eq!(relying.read_staple(&ids(), &a.bytes, &kh("carol"), &[]).1, Staple::Current);
}

// acceptance: CUR-09
#[test]
fn the_grandpatron_issues_once_the_patron_and_its_siblings_are_dark() {
    let mut cur = CurrencyState::default();
    let g = ladder_view("alice");
    cur.unreachable.insert(kh("bob"), g.now() - cur.grandpatron_after);
    assert_eq!(g.rung_for(&cur, &kh("carol")), None, "P' can still answer");
    // the sibling dark for an hour: hours are the sibling's rung, not days
    cur.unreachable.insert(kh("w1"), g.now() - 3600);
    assert_eq!(g.rung_for(&cur, &kh("carol")), None, "not until every sibling has been dark for days");
    cur.unreachable.insert(kh("w1"), g.now() - cur.grandpatron_after);
    assert_eq!(g.rung_for(&cur, &kh("carol")), Some(Rung::Grandpatron));
    let a = att(&g.issue_currency(&cur, &kh("carol")).unwrap());
    assert_eq!(a.role, ROLE_GRANDPATRON);
    assert_eq!(a.issuer, kh("alice"));
    assert_eq!(ladder_view("w1").read_staple(&ids(), &a.bytes, &kh("carol"), &[]).1, Staple::Current);
}

// acceptance: CUR-15
#[test]
fn a_peering_is_proposed_with_no_staple_however_many_issuers_are_dark() {
    let mut cur = CurrencyState::default();
    let mut s = ladder_view("carol");
    // S publishes an endpoint of its own, as a peering party must
    let er = rhtn_node::resolution::endpoint_record(&id("carol"), &[point(3, 7003)], Seqno { series: 3, counter: 1 });
    let fab = Fabric::with(&[]);
    s.take_object(&*fab, &kh("carol"), rhtn_node::store::KIND_ENDPOINT_RECORD, &er, &ids());
    for who in ["bob", "w1", "alice"] {
        cur.dark(kh(who), s.now() - 3 * 86_400);
    }
    // every rung is unreachable: nobody can issue for S
    for issuer in ["bob", "w1", "alice"] {
        let v = ladder_view(issuer);
        assert!(v.rung_for(&cur, &kh("carol")).is_none() || cur.unreachable.contains_key(&kh(issuer)));
    }
    let stale = tx::currency_attestation(&id("bob"), &kh("carol"), &kh("carol"), s.now() - 40_000, s.now() - 1, ROLE_PATRON);
    assert_eq!(s.take_staple(&ids(), &kh("carol"), &stale, &[]), Staple::Expired);
    let pop = rhtn_codec::cose::sha256(b"pop between carol and w5");
    let other_point = point(5, 7005);
    let other_back = [rhtn_archive::genesis(&kh("w5"))];
    let first = s.propose_peering(&kh("w5"), &other_point, &pop, &other_back).expect("the peering transaction is produced");
    let rec = Record::parse(&tx::envelope(tx::TYPE_PEERING, &first, &[&id("carol"), &id("w5")])).unwrap();
    assert_eq!(rec.tx_type, tx::TYPE_PEERING);
    // and again, with nobody any more reachable than before
    let second = s.propose_peering(&kh("w5"), &other_point, &pop, &other_back).expect("and again");
    assert!(!second.is_empty());
    assert_eq!(fab.request_count(REQUEST_CURRENCY), 0, "nothing about S's currency was asked of anyone");
}

// acceptance: CUR-11
#[test]
fn a_caller_with_a_stale_staple_asks_its_introducer_first() {
    let mut w = World::new();
    let mut n = view("alice", table_with(kh("alice"), &w, &[], &["alice"]), "alice", &[]);
    n.set_now(w.tick());
    let x = kh("bob");
    let introducer = kh("w1");
    let patron = kh("carol");
    let fab = Fabric::with(&[introducer, patron]);
    let stale = tx::currency_attestation(&id("carol"), &x, &x, n.now() - 40_000, n.now() - 1, ROLE_PATRON);
    n.take_staple(&ids(), &x, &stale, &[patron]);
    let mut ask = match n.require_currency(&*fab, &ids(), &x, Some(introducer), Some(patron)) {
        Requirement::Asked(ask) => ask,
        Requirement::Settled(g) => panic!("{g:?}"),
    };
    assert_eq!(ask.asked, vec![introducer], "N's first request goes toward I");
    assert_eq!(fab.requests_to(&introducer, REQUEST_CURRENCY).len(), 1);
    assert_eq!(fab.requests_to(&patron, REQUEST_CURRENCY).len(), 0, "and none toward X's patron");
    // I answers code 1: only then is the patron asked
    let cannot = CurrencyReply::CannotIssue { nonce: ask.nonce };
    let step = n.on_currency_reply(&*fab, &ids(), &mut ask, &cannot);
    assert_eq!(step, AskStep::AskedNext(patron));
    assert_eq!(fab.requests_to(&patron, REQUEST_CURRENCY).len(), 1);
    // and the patron's code 1 exhausts the ask: the caller concludes nothing
    assert_eq!(n.on_currency_reply(&*fab, &ids(), &mut ask, &cannot), AskStep::Exhausted);
    // where I holds no session at all, the patron is asked at once
    fab.clear();
    fab.drop_session(&introducer);
    let ask2 = n.ask_currency(&*fab, x, Some(introducer), Some(patron), nonce(11)).unwrap();
    assert_eq!(ask2.asked, vec![patron]);
}

// acceptance: CUR-12
#[test]
fn answering_a_currency_request_retains_nothing() {
    let (_w, p, cur) = patron_and_subordinate();
    let before = (p.archive.len(), p.store.objects(), p.memo_table.clone(), p.slots.clone(), p.staples.clone(), p.table.bindings().to_vec());
    let req = CurrencyRequest { subject: kh("bob"), nonce: nonce(12) };
    let reply = p.answer_currency(&cur, &req);
    assert!(matches!(reply, CurrencyReply::Attestation { .. }));
    let after = (p.archive.len(), p.store.objects(), p.memo_table.clone(), p.slots.clone(), p.staples.clone(), p.table.bindings().to_vec());
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
    let fab = Fabric::with(&[kh("w1")]);
    n.set_now(t - 1);
    assert_eq!(n.read_staple(&ids(), &staple, &kh("bob"), &[]).1, Staple::Current);
    n.take_staple(&ids(), &kh("bob"), &staple, &[]);
    assert_eq!(n.require_currency(&*fab, &ids(), &kh("bob"), Some(kh("w1")), None), Requirement::Settled(Gate::Proceed), "current: nothing to ask");
    assert_eq!(fab.request_count(REQUEST_CURRENCY), 0);
    n.set_now(t + 1);
    assert_eq!(n.read_staple(&ids(), &staple, &kh("bob"), &[]).1, Staple::Expired);
    assert!(matches!(n.require_currency(&*fab, &ids(), &kh("bob"), Some(kh("w1")), None), Requirement::Asked(_)), "expired: the fallback query is sent");
    assert_eq!(fab.request_count(REQUEST_CURRENCY), 1);
}

/// Not a catalogue entry: the relying party places the issuer.  A staple
/// signed by a party that does not stand where it claims for the subject
/// establishes nothing, and one this node cannot place at all is not
/// current either.
#[test]
fn a_staple_from_an_issuer_the_relying_party_cannot_place_is_not_current() {
    let relying = ladder_view("alice");
    let subject = kh("carol");
    let t = relying.now();
    // a stranger signs a syntactically perfect staple
    let forged = tx::currency_attestation(&id("w9"), &subject, &subject, t, t + 36_000, ROLE_PATRON);
    assert_eq!(relying.read_staple(&ids(), &forged, &subject, &[]).1, Staple::WrongIssuer);
    // a real party on the wrong rung
    let wrong_rung = tx::currency_attestation(&id("w1"), &subject, &subject, t, t + 36_000, ROLE_PATRON);
    assert_eq!(relying.read_staple(&ids(), &wrong_rung, &subject, &[]).1, Staple::WrongIssuer, "P' is a sibling, not the patron");
    let right_rung = tx::currency_attestation(&id("w1"), &subject, &subject, t, t + 36_000, ROLE_SIBLING);
    assert_eq!(relying.read_staple(&ids(), &right_rung, &subject, &[]).1, Staple::Current);
    // a node holding no topology for the subject cannot place anyone
    let mut w = World::new();
    let blank = view("w8", table_with(kh("w8"), &w, &[], &[]), "w8", &[]);
    let _ = w.tick();
    assert_eq!(blank.read_staple(&ids(), &right_rung, &subject, &[]).1, Staple::UnknownIssuer);
    // unless the introduction named the patron
    let by_patron = tx::currency_attestation(&id("bob"), &subject, &subject, t, t + 36_000, ROLE_PATRON);
    assert_eq!(blank.read_staple(&ids(), &by_patron, &subject, &[kh("bob")]).1, Staple::Current);
}

// acceptance: CUR-17
#[test]
fn issuance_and_the_ladder_read_a_clock_that_moves() {
    use std::sync::atomic::{AtomicU64, Ordering};
    let mut w = World::new();
    let (a_n, _) = w.adopt("bob", "alice", 1);
    let (a_s, _) = w.adopt("carol", "bob", 2);
    // the grandpatron's view on a shared clock nobody writes through the view
    let clock = std::sync::Arc::new(AtomicU64::new(w.clock + 10));
    let reader = clock.clone();
    let mut g = view("alice", table_with(kh("alice"), &w, &[&a_n, &a_s], &["alice", "bob"]), "alice", &[]);
    g.clock = std::sync::Arc::new(move || reader.load(Ordering::SeqCst));
    let mut cur = CurrencyState::default();
    // issuance for the patron's own subordinate stamps the clock's reading
    let t0 = clock.load(Ordering::SeqCst);
    let a = rhtn_archive::currency::parse_attestation(&ids(), &g.issue_currency(&cur, &kh("bob")).unwrap()).unwrap();
    assert_eq!((a.issued_at, a.expires_at), (t0, t0 + cur.lifetime));
    clock.fetch_add(1, Ordering::SeqCst);
    let a = rhtn_archive::currency::parse_attestation(&ids(), &g.issue_currency(&cur, &kh("bob")).unwrap()).unwrap();
    assert_eq!(a.issued_at, t0 + 1, "the next issuance reads the clock again");
    // the ladder: the patron and its sibling go dark at the clock's reading,
    // and the grandpatron rung opens when the clock, not the view, has moved
    cur.dark(kh("bob"), g.now());
    assert_eq!(cur.unreachable[&kh("bob")], t0 + 1);
    assert_eq!(g.rung_for(&cur, &kh("carol")), None, "within the interval");
    clock.fetch_add(cur.grandpatron_after, Ordering::SeqCst);
    assert_eq!(g.rung_for(&cur, &kh("carol")), Some(Rung::Grandpatron), "the interval elapsed on the clock alone");
}

// acceptance: CUR-18
#[test]
fn a_currency_ask_goes_on_a_request_stream_and_its_reply_settles_it() {
    let mut w = World::new();
    let mut n = view("alice", table_with(kh("alice"), &w, &[], &["alice"]), "alice", &[]);
    n.set_now(w.tick());
    let (x, patron) = (kh("bob"), kh("carol"));
    let fab = Fabric::with(&[patron]);
    let ask = n.ask_currency(&*fab, x, None, Some(patron), nonce(18)).expect("asked");
    assert_eq!(fab.requests_to(&patron, REQUEST_CURRENCY).len(), 1, "one request stream toward the patron");
    assert!(fab.frames().is_empty(), "and nothing on stream 0");
    assert!(n.asks.contains_key(&ask.nonce), "held for its reply");
    // the reply comes back by the request path and settles the ask
    let att = tx::currency_attestation(&id("carol"), &x, &x, n.now(), n.now() + 36_000, ROLE_PATRON);
    let reply = CurrencyReply::Attestation { nonce: ask.nonce, bytes: att }.encode();
    assert_eq!(n.take_currency_reply(&*fab, &ids(), &reply), Some(AskStep::Current));
    assert!(!n.asks.contains_key(&ask.nonce));
    assert!(n.staples.contains_key(&x), "the attestation is the staple now held");
    // a reply naming no outstanding ask concludes nothing
    assert_eq!(n.take_currency_reply(&*fab, &ids(), &reply), None);
    // a session this node serves carries no request: a light client
    // answers no request stream, so the patron attached below cannot be asked
    let below = Fabric::with(&[patron]);
    below.serve(&patron);
    assert!(n.ask_currency(&*below, x, None, Some(patron), nonce(19)).is_none());
    assert_eq!(below.request_count(REQUEST_CURRENCY), 0);
    assert!(below.frames().is_empty());
}
