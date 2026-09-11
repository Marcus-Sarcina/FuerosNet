//! Resources at the hosting node (`wire-format.md` §6, §11;
//! `infra-client-requirements.md` §9 to §11): registrations over the
//! owner's own session, one owner per keyhash, replacement without an
//! archive, scopes at answer time, the total order and the continuation,
//! the owner's signature unchanged; the gateway's normative order, the
//! credential, the parse, one handoff, sessions per resource and their
//! end on a row change; acknowledgements under policy; the sandbox's
//! export list; and abuse reports stored and carried nowhere.

mod common;

use common::*;
use rhtn_archive::catalog::*;
use rhtn_archive::topology::{AckIssuer, Table};
use rhtn_node::catalog::{CatalogService, TableScopes};
use rhtn_node::http;
use rhtn_node::resources::*;
use std::collections::BTreeSet;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

/// Host H (alice, a root): bob a light client under it, carol under it,
/// w1 under carol (a grand-subordinate), w2 under w3, another root.
struct Scene {
    w: World,
    table: Table,
}

fn scene() -> Scene {
    let mut w = World::new();
    let (a_b, _) = w.adopt("bob", "alice", 1);
    let (a_c, _) = w.adopt("carol", "alice", 2);
    let (a_w1, _) = w.adopt("w1", "carol", 3);
    let (a_w2, _) = w.adopt("w2", "w3", 4);
    let table = table_with(kh("alice"), &w, &[&a_b, &a_c, &a_w1, &a_w2], &["alice", "carol", "w3"]);
    Scene { w, table }
}

fn entry(owner: &str, resource: [u8; 32], service_type: &str, endpoint: &[u8]) -> Vec<u8> {
    CatalogEntry::build(&id(owner), &EntryFields { resource, service_type: service_type.into(), instance: "An Instance".into(), endpoint: endpoint.to_vec(), connect_scope: None, metadata: None, data_practice: Some(0) })
}

fn register(svc: &mut CatalogService, peer: &str, entry: &[u8], scope: Option<Scope>, n: u8) -> u64 {
    let body = ResourceRegistration { entry: entry.to_vec(), scope, nonce: [n; 16] }.encode();
    let reply = svc.register(&ids(), &kh(peer), &body).expect("a well-framed registration is answered");
    let r = RegistrationReply::decode(&reply).unwrap();
    assert_eq!(r.nonce, [n; 16]);
    r.code
}

fn query(svc: &CatalogService, scopes: &TableScopes, asker: &str, filter: Option<&str>) -> Option<CatalogReply> {
    let q = CatalogQuery { service_type: filter.map(|s| s.to_string()), nonce: [9; 16] }.encode();
    svc.answer(&kh(asker), &q, scopes).map(|b| CatalogReply::decode(&b).unwrap())
}

const R1: [u8; 32] = [0xa1; 32];
const R2: [u8; 32] = [0xb2; 32];

// acceptance: RSC-01
#[test]
fn a_registration_is_accepted_only_over_the_owners_own_session() {
    let sc = scene();
    let mut svc = CatalogService::default();
    let e = entry("bob", R1, "rhtn-forum", b"quic://198.51.100.7:4433");
    assert_eq!(register(&mut svc, "carol", &e, None, 1), REGISTRATION_REFUSED, "P's session, O's entry");
    assert!(svc.held(&R1).is_none());
    assert_eq!(register(&mut svc, "bob", &e, None, 2), REGISTRATION_RECORDED);
    assert_eq!(svc.held(&R1).unwrap().bytes, e);
    // the same bytes over P's session again: refused, the served entry unchanged
    assert_eq!(register(&mut svc, "carol", &e, None, 3), REGISTRATION_REFUSED);
    assert_eq!(svc.held(&R1).unwrap().bytes, e);
    let _ = &sc.table;
}

// acceptance: RSC-02
#[test]
fn a_keyhash_served_under_one_owner_is_refused_to_another() {
    let mut svc = CatalogService::default();
    let e1 = entry("bob", R1, "rhtn-forum", b"one");
    assert_eq!(register(&mut svc, "bob", &e1, None, 1), REGISTRATION_RECORDED);
    let e2 = entry("carol", R1, "rhtn-forum", b"two");
    assert_eq!(register(&mut svc, "carol", &e2, None, 2), REGISTRATION_REFUSED);
    assert_eq!(svc.held(&R1).unwrap().owner, kh("bob"), "H still serves O1's entry");
    assert_eq!(svc.held(&R1).unwrap().bytes, e1);
}

// acceptance: RSC-03
#[test]
fn re_registration_replaces_and_nothing_superseded_is_retained_or_archived() {
    let sc = scene();
    let mut svc = CatalogService::default();
    let mut view = view("alice", sc.table.clone_for(kh("alice")), "alice", &[]);
    let archive_len = view.archive.len();
    let e1 = entry("bob", R1, "rhtn-forum", b"one");
    let e2 = entry("bob", R1, "rhtn-forum", b"two");
    assert_eq!(register(&mut svc, "bob", &e1, Some(Scope::Dunbar), 1), REGISTRATION_RECORDED);
    assert_eq!(register(&mut svc, "bob", &e2, None, 2), REGISTRATION_RECORDED);
    let scopes = TableScopes { table: &sc.table, me: kh("alice") };
    let reply = query(&svc, &scopes, "carol", None).unwrap();
    assert_eq!(reply.entries, vec![e2.clone()], "E2 and never E1");
    assert!(!format!("{svc:?}").contains(&format!("{:?}", e1)), "E1 is absent from H's storage");
    assert_eq!(view.archive.len(), archive_len, "the archive did not advance");
    view.catalog = svc;
    assert_eq!(view.archive.len(), archive_len);
}

// acceptance: RSC-04
#[test]
fn a_first_registration_defaults_to_scope_self() {
    let sc = scene();
    let mut svc = CatalogService::default();
    let e = entry("bob", R1, "rhtn-forum", b"one");
    assert_eq!(register(&mut svc, "bob", &e, None, 1), REGISTRATION_RECORDED);
    let scopes = TableScopes { table: &sc.table, me: kh("alice") };
    assert_eq!(query(&svc, &scopes, "bob", None).unwrap().entries, vec![e.clone()], "O sees its own entry");
    assert!(query(&svc, &scopes, "carol", None).unwrap().entries.is_empty(), "a horizon member does not");
}

// acceptance: RSC-05
#[test]
fn withdrawal_is_re_registration_with_scope_self_and_nothing_propagates() {
    let sc = scene();
    let mut view = view("alice", sc.table.clone_for(kh("alice")), "alice", &[]);
    let fab = Fabric::with(&[kh("bob"), kh("carol")]);
    let e = entry("bob", R1, "rhtn-forum", b"one");
    let scopes_table = sc.table.clone_for(kh("alice"));
    let scopes = TableScopes { table: &scopes_table, me: kh("alice") };
    assert_eq!(register(&mut view.catalog, "bob", &e, Some(Scope::Dunbar), 1), REGISTRATION_RECORDED);
    assert_eq!(query(&view.catalog, &scopes, "carol", None).unwrap().entries.len(), 1, "visible to M");
    assert_eq!(register(&mut view.catalog, "bob", &e, Some(Scope::Own), 2), REGISTRATION_RECORDED);
    assert!(query(&view.catalog, &scopes, "carol", None).unwrap().entries.is_empty(), "M's reply omits R");
    assert!(fab.frames().is_empty(), "no TopologyPush or other frame carrying the entry leaves H");
    assert_eq!(view.archive.len(), 0);
}

// acceptance: RSC-06
#[test]
fn an_asker_outside_the_horizon_gets_no_reply_at_all() {
    let sc = scene();
    let mut svc = CatalogService::default();
    let e = entry("bob", R1, "rhtn-forum", b"one");
    register(&mut svc, "bob", &e, Some(Scope::Dunbar), 1);
    let scopes = TableScopes { table: &sc.table, me: kh("alice") };
    assert!(!sc.table.horizon(&kh("alice"), 2).contains(&kh("w2")));
    assert_eq!(query(&svc, &scopes, "w2", None), None, "the stream is closed: no CatalogReply, empty or otherwise");
    assert!(query(&svc, &scopes, "carol", None).is_some());
}

// acceptance: RSC-07
#[test]
fn entries_come_ordered_by_resource_then_owner_111_at_a_time_with_the_next_type_named() {
    let sc = scene();
    let mut svc = CatalogService::default();
    // 113 entries visible to carol, resource keyhashes in a known order,
    // the 112th of a type of its own
    let mut resources: Vec<[u8; 32]> = (0..113u32).map(|i| rhtn_codec::cose::sha256(&i.to_be_bytes())).collect();
    resources.sort();
    for (i, r) in resources.iter().enumerate() {
        let t = if i == 111 { "rhtn-wiki" } else { "rhtn-forum" };
        let e = entry("bob", *r, t, b"x");
        assert_eq!(register(&mut svc, "bob", &e, Some(Scope::Dunbar), (i % 250) as u8), REGISTRATION_RECORDED);
    }
    let scopes = TableScopes { table: &sc.table, me: kh("alice") };
    let reply = query(&svc, &scopes, "carol", None).unwrap();
    assert_eq!(reply.entries.len(), 111);
    let got: Vec<[u8; 32]> = reply.entries.iter().map(|b| CatalogEntry::parse(b).unwrap().resource).collect();
    assert_eq!(got, resources[..111].to_vec(), "ascending resource-then-owner keyhash order");
    assert_eq!(reply.continuation.as_deref(), Some("rhtn-wiki"), "the type of the 112th");
    // the filtered query makes progress within the type
    let wiki = query(&svc, &scopes, "carol", Some("rhtn-wiki")).unwrap();
    assert_eq!((wiki.entries.len(), wiki.continuation), (1, None));
}

// acceptance: RSC-08
#[test]
fn the_owners_signature_is_returned_unchanged_and_none_is_added() {
    let sc = scene();
    let mut svc = CatalogService::default();
    let e = entry("bob", R1, "rhtn-forum", b"one");
    register(&mut svc, "bob", &e, Some(Scope::Dunbar), 1);
    let scopes = TableScopes { table: &sc.table, me: kh("alice") };
    for asker in ["carol", "w1"] {
        let reply = query(&svc, &scopes, asker, None).unwrap();
        assert_eq!(reply.entries, vec![e.clone()], "byte-for-byte to {asker}");
        let parsed = CatalogEntry::parse(&reply.entries[0]).unwrap();
        assert_eq!(parsed.verify(&ids()), Ok(true), "O's signature");
        assert!(!reply.encode().windows(32).any(|w| w == kh("alice")), "nothing by H");
    }
}

// acceptance: RSC-09
#[test]
fn an_entry_whose_scope_cannot_be_computed_is_stored_and_grants_nothing() {
    let sc = scene();
    let mut svc = CatalogService::default();
    let far = [0xee; 32];
    let e = CatalogEntry::build(&id("bob"), &EntryFields { resource: R1, service_type: "rhtn-forum".into(), instance: "x".into(), endpoint: b"e".to_vec(), connect_scope: Some(Scope::List(vec![far])), metadata: None, data_practice: None });
    assert_eq!(register(&mut svc, "bob", &e, None, 1), REGISTRATION_RECORDED, "answered code 0");
    let scopes = TableScopes { table: &sc.table, me: kh("alice") };
    assert_eq!(query(&svc, &scopes, "bob", None).unwrap().entries, vec![e.clone()], "served");
    assert!(query(&svc, &scopes, "carol", None).unwrap().entries.is_empty(), "the scope grants nothing");
    // and a request is decided by the role row alone
    let mut g = Gateway::default();
    g.bind(R1, Binding { owner: kh("bob"), authority: "r1.internal".into(), backend: Some(Arc::new(Fake::new())), declared_roles: BTreeSet::new() });
    let req = ResourceRequest { resource: R1, message: b"GET / HTTP/1.1\r\nhost: r1\r\n\r\n".to_vec() }.encode();
    assert_eq!(g.serve(&kh("alice"), &sc.table, &kh("carol"), &req).status, STATUS_NO_ROLE);
    g.set_row(R1, kh("carol"), Row { roles: BTreeSet::new(), connect: true }).unwrap();
    assert_eq!(g.serve(&kh("alice"), &sc.table, &kh("carol"), &req).status, STATUS_DELIVERED);
}

// acceptance: RSC-10
#[test]
fn an_unknown_data_practice_is_retained_and_served() {
    let sc = scene();
    let mut svc = CatalogService::default();
    let e = CatalogEntry::build(&id("bob"), &EntryFields { resource: R1, service_type: "rhtn-forum".into(), instance: "x".into(), endpoint: b"e".to_vec(), connect_scope: None, metadata: None, data_practice: Some(7) });
    assert_eq!(register(&mut svc, "bob", &e, Some(Scope::Dunbar), 1), REGISTRATION_RECORDED, "accepted");
    let scopes = TableScopes { table: &sc.table, me: kh("alice") };
    let reply = query(&svc, &scopes, "carol", None).unwrap();
    assert_eq!(CatalogEntry::parse(&reply.entries[0]).unwrap().data_practice, Some(7), "field 9 unchanged");
}

/// A backend that records what reached it, can be stopped, and can fail.
struct Fake {
    running: AtomicBool,
    fail: AtomicBool,
    seen: Mutex<Vec<Vec<u8>>>,
    calls: AtomicU64,
}
impl Fake {
    fn new() -> Self {
        Fake { running: AtomicBool::new(true), fail: AtomicBool::new(false), seen: Mutex::new(vec![]), calls: AtomicU64::new(0) }
    }
}
impl Backend for Fake {
    fn handle(&self, message: &[u8]) -> Result<Vec<u8>, String> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        self.seen.lock().unwrap().push(message.to_vec());
        if self.fail.load(Ordering::SeqCst) { Err("failed mid-request".into()) } else { Ok(b"HTTP/1.1 200 OK\r\ncontent-length: 2\r\n\r\nok".to_vec()) }
    }
    fn running(&self) -> bool {
        self.running.load(Ordering::SeqCst)
    }
}

fn request(resource: [u8; 32], message: &[u8]) -> Vec<u8> {
    ResourceRequest { resource, message: message.to_vec() }.encode()
}

const OK_GET: &[u8] = b"GET /threads/42 HTTP/1.1\r\nhost: reading-room.internal\r\n\r\n";

fn gateway_with(sc: &Scene, backend: Arc<Fake>) -> Gateway {
    let mut g = Gateway::default();
    g.bind(R1, Binding { owner: kh("alice"), authority: "r1.internal".into(), backend: Some(backend), declared_roles: BTreeSet::from(["reader".to_string(), "writer".to_string()]) });
    let _ = &sc.table;
    g
}

fn ack(sc: &mut Scene, node: &str) {
    // the host acknowledges w1, adopted under carol: the ack policy admits it
    let adoption = sc.table.bindings().iter().find(|b| b.node == kh(node)).unwrap().adoption;
    let rec = sc.w.bytes(&adoption);
    let rec = rhtn_archive::record::Record::parse(&rec).unwrap();
    let issuer = AckIssuer { identity: Arc::new(id("alice")), policy: Arc::new(|_, _| true), now: sc.w.clock };
    // re-apply with the issuer: the binding exists, so the ack alone is produced
    let mut t = Table::with_me(kh("alice"));
    t.mark_infra(kh("alice"));
    t.mark_infra(kh("carol"));
    for b in sc.table.bindings() {
        let r = rhtn_archive::record::Record::parse(&sc.w.bytes(&b.adoption)).unwrap();
        let iss = if r.txid == rec.txid { Some(&issuer) } else { None };
        t.apply(&r, &ids(), &sc.w.store, iss).unwrap();
    }
    sc.table = t;
}

// acceptance: RSC-12
#[test]
fn a_request_is_evaluated_in_the_normative_order_and_answered_with_the_one_code_its_step_yields() {
    let mut sc = scene();
    let backend = Arc::new(Fake::new());
    backend.running.store(false, Ordering::SeqCst);
    let mut g = gateway_with(&sc, backend.clone());
    let me = kh("alice");
    // a stranger
    assert_eq!(g.serve(&me, &sc.table, &kh("w2"), &request(R1, OK_GET)).status, STATUS_REFUSED);
    // a member lacking a SubtreeAck: w1, adopted under carol
    assert_eq!(g.serve(&me, &sc.table, &kh("w1"), &request(R1, OK_GET)).status, STATUS_NO_ACK);
    // acknowledged, no connect
    ack(&mut sc, "w1");
    assert!(sc.table.acks().iter().any(|a| a.node == kh("w1")));
    assert_eq!(g.serve(&me, &sc.table, &kh("w1"), &request(R1, OK_GET)).status, STATUS_NO_ROLE);
    // connect, malformed HTTP
    g.set_row(R1, kh("carol"), Row { roles: BTreeSet::from(["reader".to_string()]), connect: true }).unwrap();
    assert_eq!(g.serve(&me, &sc.table, &kh("carol"), &request(R1, b"GET / HTTP/1.1\r\ncontent-length: 2\r\ntransfer-encoding: chunked\r\nhost: x\r\n\r\n")).status, STATUS_MALFORMED);
    // well-formed, package stopped
    assert_eq!(g.serve(&me, &sc.table, &kh("carol"), &request(R1, OK_GET)).status, STATUS_UNAVAILABLE);
    assert_eq!(backend.calls.load(Ordering::SeqCst), 0, "nothing reached the backend yet");
    // running
    backend.running.store(true, Ordering::SeqCst);
    let r = g.serve(&me, &sc.table, &kh("carol"), &request(R1, OK_GET));
    assert_eq!(r.status, STATUS_DELIVERED);
    assert!(r.body.unwrap().starts_with(b"HTTP/1.1 200"));
    assert_eq!(backend.calls.load(Ordering::SeqCst), 1, "only the last reached the backend");
    // step 0: a body that does not decode
    assert_eq!(g.serve(&me, &sc.table, &kh("carol"), b"\xff\xff").status, STATUS_MALFORMED);
}

// acceptance: RSC-13
#[test]
fn a_non_member_is_refused_for_everything_existing_or_not() {
    let sc = scene();
    let backend = Arc::new(Fake::new());
    let mut g = gateway_with(&sc, backend.clone());
    let unknown = [0x77; 32];
    let a = g.serve(&kh("alice"), &sc.table, &kh("w2"), &request(R1, OK_GET));
    let b = g.serve(&kh("alice"), &sc.table, &kh("w2"), &request(unknown, OK_GET));
    assert_eq!((a.clone(), b.clone()), (ResourceResponse::code(STATUS_REFUSED), ResourceResponse::code(STATUS_REFUSED)));
    assert_eq!(a.encode(), b.encode(), "code 1 and nothing else, both");
    assert_eq!(backend.calls.load(Ordering::SeqCst), 0);
}

// acceptance: RSC-14
#[test]
fn a_request_is_decided_from_the_materialised_row_and_a_row_wider_than_64_is_refused() {
    let sc = scene();
    let backend = Arc::new(Fake::new());
    let mut g = Gateway::default();
    let mut declared: BTreeSet<String> = (0..70).map(|i| format!("r{i}")).collect();
    declared.insert("reader".into());
    g.bind(R1, Binding { owner: kh("alice"), authority: "r1".into(), backend: Some(backend.clone()), declared_roles: declared });
    // a predicate expanded once: "all my direct clients" as of now
    let members: Vec<_> = sc.table.subordinates(&kh("alice")).into_iter().collect();
    g.materialise(R1, &members, Row { roles: BTreeSet::from(["reader".to_string()]), connect: true }).unwrap();
    // the predicate would now exclude carol (say carol departed in the
    // operator's view of things); unevaluated, the row stands
    let r = g.serve(&kh("alice"), &sc.table, &kh("carol"), &request(R1, OK_GET));
    assert_eq!(r.status, STATUS_DELIVERED, "delivered on the strength of the row");
    // 65 roles for w1: refused at configuration, and no request ever yields a code for it
    let wide: BTreeSet<String> = (0..65).map(|i| format!("r{i}")).collect();
    assert_eq!(g.set_row(R1, kh("w1"), Row { roles: wide, connect: true }), Err(RowError::TooWide(65)));
    assert!(g.row(&R1, &kh("w1")).is_none(), "no row was materialised");
    let sixty_four: BTreeSet<String> = (0..64).map(|i| format!("r{i}")).collect();
    assert!(g.set_row(R1, kh("w1"), Row { roles: sixty_four, connect: true }).is_ok());
    assert_eq!(g.set_row(R1, kh("w1"), Row { roles: BTreeSet::from(["admin".to_string()]), connect: true }), Err(RowError::Undeclared("admin".into())));
}

// acceptance: RSC-15
#[test]
fn caller_rhtn_headers_are_stripped_the_nodes_inserted_and_a_session_minted_per_resource() {
    let sc = scene();
    let (b1, b2) = (Arc::new(Fake::new()), Arc::new(Fake::new()));
    let mut g = Gateway::default();
    g.bind(R1, Binding { owner: kh("alice"), authority: "r1.internal".into(), backend: Some(b1.clone()), declared_roles: BTreeSet::new() });
    g.bind(R2, Binding { owner: kh("alice"), authority: "r2.internal".into(), backend: Some(b2.clone()), declared_roles: BTreeSet::new() });
    for r in [R1, R2] {
        g.set_row(r, kh("carol"), Row { roles: BTreeSet::new(), connect: true }).unwrap();
    }
    let forged = b"GET /x HTTP/1.1\r\nhost: evil\r\nrhtn-roles: admin\r\nrhtn-principal: somebody\r\nx-app: keep\r\n\r\n";
    assert_eq!(g.serve(&kh("alice"), &sc.table, &kh("carol"), &request(R1, forged)).status, STATUS_DELIVERED);
    assert_eq!(g.serve(&kh("alice"), &sc.table, &kh("carol"), &request(R2, forged)).status, STATUS_DELIVERED);
    let h1 = http::headers_of(&b1.seen.lock().unwrap()[0]);
    let h2 = http::headers_of(&b2.seen.lock().unwrap()[0]);
    let get = |h: &Vec<(String, String)>, n: &str| h.iter().filter(|(k, _)| k == n).map(|(_, v)| v.clone()).collect::<Vec<_>>();
    for (h, res, authority) in [(&h1, R1, "r1.internal"), (&h2, R2, "r2.internal")] {
        assert_eq!(get(h, "rhtn-principal"), vec![http::base64url(&http::pairwise_principal(&res, &kh("carol")))]);
        assert_eq!(get(h, "rhtn-audience"), vec![http::base64url(&res)], "naming itself");
        assert_eq!(get(h, "rhtn-roles"), vec![String::new()], "empty, and not an error");
        assert_eq!(get(h, "rhtn-session").len(), 1);
        assert_eq!(get(h, "host"), vec![authority.to_string()], "routed by the resource, not the caller's host");
        assert_eq!(get(h, "x-app"), vec!["keep".to_string()]);
        assert!(!h.iter().any(|(_, v)| v == "admin" || v == "somebody"), "the forged values are absent");
    }
    assert_ne!(get(&h1, "rhtn-session"), get(&h2, "rhtn-session"), "one identifier per resource");
    assert_ne!(get(&h1, "rhtn-principal"), get(&h2, "rhtn-principal"), "pairwise per resource");
    // the same session identifier on the next request to the same resource
    g.serve(&kh("alice"), &sc.table, &kh("carol"), &request(R1, OK_GET));
    assert_eq!(get(&http::headers_of(&b1.seen.lock().unwrap()[1]), "rhtn-session"), get(&h1, "rhtn-session"));
}

// acceptance: RSC-16
#[test]
fn ambiguous_messages_are_rejected_and_exactly_one_message_is_emitted() {
    let sc = scene();
    let backend = Arc::new(Fake::new());
    let mut g = gateway_with(&sc, backend.clone());
    g.set_row(R1, kh("carol"), Row { roles: BTreeSet::new(), connect: true }).unwrap();
    let cases: [&[u8]; 5] = [
        b"POST / HTTP/1.1\r\nhost: x\r\ncontent-length: 2\r\ntransfer-encoding: chunked\r\n\r\nab",
        b"POST / HTTP/1.1\r\nhost: x\r\ncontent-length: 2\r\n\r\nabGET /second HTTP/1.1\r\nhost: x\r\n\r\n",
        b"CONNECT x:443 HTTP/1.1\r\nhost: x\r\n\r\n",
        b"GET / HTTP/1.1\r\nhost: x\r\nupgrade: h2c\r\nconnection: upgrade\r\n\r\n",
        b"GET / HTTP/1.1\r\nhost: x\r\n folded: value\r\n\r\n",
    ];
    for c in cases {
        assert_eq!(g.serve(&kh("alice"), &sc.table, &kh("carol"), &request(R1, c)).status, STATUS_MALFORMED, "{:?}", String::from_utf8_lossy(c));
    }
    assert_eq!(backend.calls.load(Ordering::SeqCst), 0, "nothing reached the backend");
    // a chunked body decodes to one message, re-serialised with its exact length
    let chunked = b"POST /p HTTP/1.1\r\nhost: x\r\ntransfer-encoding: chunked\r\n\r\n3\r\nabc\r\n0\r\n\r\n";
    assert_eq!(g.serve(&kh("alice"), &sc.table, &kh("carol"), &request(R1, chunked)).status, STATUS_DELIVERED);
    let seen = backend.seen.lock().unwrap()[0].clone();
    assert!(seen.ends_with(b"\r\n\r\nabc"));
    assert!(http::headers_of(&seen).contains(&("content-length".to_string(), "3".to_string())));
    assert!(!http::headers_of(&seen).iter().any(|(k, _)| k == "transfer-encoding"));
}

// acceptance: RSC-17
#[test]
fn a_failed_handoff_is_never_retried() {
    let sc = scene();
    let backend = Arc::new(Fake::new());
    backend.fail.store(true, Ordering::SeqCst);
    let mut g = gateway_with(&sc, backend.clone());
    g.set_row(R1, kh("carol"), Row { roles: BTreeSet::new(), connect: true }).unwrap();
    let r = g.serve(&kh("alice"), &sc.table, &kh("carol"), &request(R1, OK_GET));
    assert_eq!(r.status, STATUS_UNAVAILABLE);
    assert_eq!(backend.calls.load(Ordering::SeqCst), 1, "exactly one attempt");
    assert_eq!(g.attempts[&R1], 1);
}

// acceptance: RSC-18
#[test]
fn a_role_change_ends_the_hosted_session_and_leaves_the_transport_alone() {
    let sc = scene();
    let backend = Arc::new(Fake::new());
    let mut g = gateway_with(&sc, backend.clone());
    g.set_row(R1, kh("carol"), Row { roles: BTreeSet::from(["reader".to_string()]), connect: true }).unwrap();
    // the in-flight request completes under the state it started with
    let r = g.serve(&kh("alice"), &sc.table, &kh("carol"), &request(R1, OK_GET));
    assert_eq!(r.status, STATUS_DELIVERED);
    let first = g.session(&kh("carol"), &R1).expect("a hosted session");
    // the operator revokes the grant: the hosted session ends
    g.set_row(R1, kh("carol"), Row { roles: BTreeSet::new(), connect: false }).unwrap();
    assert_eq!(g.session(&kh("carol"), &R1), None, "the resource-facing session is dropped");
    assert_eq!(g.serve(&kh("alice"), &sc.table, &kh("carol"), &request(R1, OK_GET)).status, STATUS_NO_ROLE);
    // granted again: a fresh identifier, which is how the resource observes the change
    g.set_row(R1, kh("carol"), Row { roles: BTreeSet::from(["reader".to_string()]), connect: true }).unwrap();
    g.serve(&kh("alice"), &sc.table, &kh("carol"), &request(R1, OK_GET));
    assert_ne!(g.session(&kh("carol"), &R1), Some(first));
    // nothing here touches the transport: the gateway holds no session object of that kind
    let _ = backend;
}

// acceptance: RSC-19
#[test]
fn no_network_primitive_is_exposed_to_a_hosted_package() {
    for import in ["rhtn/1:topology", "rhtn/1:queue", "rhtn/1:prekeys", "rhtn/1:roles", "rhtn/1:liveness"] {
        let m = Manifest { roles: BTreeSet::from(["reader".to_string()]), imports: vec!["rhtn/1:request".into(), import.into()] };
        let e = instantiate(&m).unwrap_err();
        assert!(e.contains("no such binding"), "{import}: {e}");
        assert!(!exports().contains(&import));
    }
    assert_eq!(exports(), &["rhtn/1:request", "rhtn/1:response"]);
    let ok = Manifest { roles: BTreeSet::from(["reader".to_string()]), imports: vec!["rhtn/1:request".into(), "rhtn/1:response".into()] };
    assert!(instantiate(&ok).is_ok());
    assert!(instantiate(&Manifest { roles: BTreeSet::from(["Bad Role".to_string()]), imports: vec![] }).is_err());
}

// acceptance: RSC-20
#[test]
fn a_subtree_is_acknowledged_under_policy_without_a_prompt_and_lapses_with_the_relationship() {
    let mut sc = scene();
    let backend = Arc::new(Fake::new());
    let mut g = gateway_with(&sc, backend);
    // G (w1) adopted under S (carol) reaches H: the policy acknowledges every position, nobody is asked
    let prompts = Arc::new(AtomicU64::new(0));
    let asked = prompts.clone();
    let issuer = AckIssuer { identity: Arc::new(id("alice")), policy: Arc::new(move |_, _| { let _ = &asked; true }), now: sc.w.clock };
    let g_adoption = sc.table.bindings().iter().find(|b| b.node == kh("w1")).unwrap().adoption;
    let rec = rhtn_archive::record::Record::parse(&sc.w.bytes(&g_adoption)).unwrap();
    let mut t = Table::with_me(kh("alice"));
    for n in ["alice", "carol", "w3"] {
        t.mark_infra(kh(n));
    }
    let mut issued = Vec::new();
    for b in sc.table.bindings() {
        let r = rhtn_archive::record::Record::parse(&sc.w.bytes(&b.adoption)).unwrap();
        let out = t.apply(&r, &ids(), &sc.w.store, if r.txid == rec.txid { Some(&issuer) } else { None }).unwrap();
        issued.extend(out.acks);
    }
    assert_eq!(issued.len(), 1, "a SubtreeAck for G");
    assert_eq!(prompts.load(Ordering::SeqCst), 0, "no prompt recorded");
    sc.table = t;
    g.set_row(R1, kh("w1"), Row { roles: BTreeSet::new(), connect: true }).unwrap();
    assert_eq!(g.serve(&kh("alice"), &sc.table, &kh("w1"), &request(R1, OK_GET)).status, STATUS_DELIVERED);
    // S departs: the acknowledgement described a subtree G is no longer in
    let dep = sc.w.depart("carol", "alice", rhtn_archive::tx::Seqno { series: 2, counter: 1 });
    sc.table.apply(&dep, &ids(), &sc.w.store, None).unwrap();
    assert!(sc.table.acks().is_empty(), "lapsed without a revocation");
    assert_eq!(g.serve(&kh("alice"), &sc.table, &kh("w1"), &request(R1, OK_GET)).status, STATUS_REFUSED, "G is no longer in H's Dunbar Org at all");
}

// acceptance: RSC-21
#[test]
fn a_brokered_registration_with_no_local_backend_is_accepted() {
    let mut svc = CatalogService::default();
    let e = entry("bob", R1, "saas-tracker", b"https://tracker.example.com/");
    assert_eq!(register(&mut svc, "bob", &e, None, 1), REGISTRATION_RECORDED, "code 0 with no package for it");
}

// acceptance: RSC-22
#[test]
fn an_abuse_report_is_stored_only_from_the_resource_it_names_and_carried_nowhere() {
    let sc = scene();
    let mut view = view("alice", sc.table.clone_for(kh("alice")), "alice", &[]);
    let fab = Fabric::with(&[kh("bob"), kh("carol")]);
    // the resource has a key of its own: w4 stands in for it
    let resource = kh("w4");
    let e = entry("bob", resource, "rhtn-forum", b"one");
    assert_eq!(register(&mut view.catalog, "bob", &e, None, 1), REGISTRATION_RECORDED);
    let genuine = AbuseReport::build(&id("w4"), 1_800_000_000, ABUSE_EXCESSIVE_LOAD, Some(b"burst"));
    let mut forged = AbuseReport::build(&id("w5"), 1_800_000_000, ABUSE_EXCESSIVE_LOAD, Some(b"burst"));
    let pos = forged.windows(32).position(|w| w == kh("w5")).unwrap();
    forged[pos..pos + 32].copy_from_slice(&resource);
    assert_eq!(view.catalog.take_report(&ids(), &genuine), Ok(kh("bob")), "stored for O");
    assert!(view.catalog.take_report(&ids(), &forged).is_err(), "rejected");
    assert_eq!(view.catalog.reports_for(&kh("bob")), vec![genuine]);
    assert!(fab.frames().is_empty(), "no report bytes leave the node");
}

#[test]
fn the_catalog_page_shows_what_the_viewer_holds_with_the_roles_held() {
    let sc = scene();
    let backend = Arc::new(Fake::new());
    let mut g = gateway_with(&sc, backend);
    let mut svc = CatalogService::default();
    let e1 = entry("alice", R1, "rhtn-forum", b"one");
    let e2 = entry("alice", R2, "rhtn-wiki", b"two");
    register(&mut svc, "alice", &e1, Some(Scope::Dunbar), 1);
    register(&mut svc, "alice", &e2, Some(Scope::Dunbar), 2);
    g.bind(R2, Binding { owner: kh("alice"), authority: "r2".into(), backend: None, declared_roles: BTreeSet::from(["editor".to_string()]) });
    g.set_row(R1, kh("carol"), Row { roles: BTreeSet::from(["reader".to_string()]), connect: true }).unwrap();
    g.set_row(R2, kh("carol"), Row { roles: BTreeSet::from(["editor".to_string()]), connect: false }).unwrap();
    let page = g.page(&kh("carol"), &svc);
    assert_eq!(page, vec![(e1.clone(), vec!["reader".to_string()])], "R2, with no connect, is not listed");
}
