//! The resource objects against the corpus: the catalog entry and the
//! abuse report with their wrong-signer analogues, the scope forms, the
//! query, registration and request frames, and every reply.

mod common;

use common::World;
use rhtn_archive::catalog::*;
use rhtn_codec::cbor::array_item_ranges;

fn fixture(id: &str) -> Vec<u8> {
    let c: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/../../test-vectors/corpus.json")).unwrap()).unwrap();
    let e = c["entries"].as_array().unwrap().iter().find(|e| e["id"] == id).unwrap_or_else(|| panic!("no fixture {id}"));
    hex::decode(e["hex"].as_str().unwrap()).unwrap()
}

fn body_of_frame(f: &[u8]) -> Vec<u8> {
    let p = &f[4..];
    let outer = array_item_ranges(p, 0).unwrap();
    p[outer[1].clone()].to_vec()
}

#[test]
fn the_corpus_catalog_entry_and_abuse_report_read_and_verify_and_their_forgeries_do_not() {
    let w = World::new(&["alice", "bob", "carol"]);
    let ids = w.lookup();
    let e = CatalogEntry::parse(&fixture("P-catalog")).unwrap();
    assert_eq!((e.owner, e.service_type.as_str(), e.instance.as_str()), (w.kh("bob"), "rhtn-forum", "The Reading Room"));
    assert_eq!(e.endpoint, b"quic://198.51.100.7:4433");
    assert_eq!((e.metadata.as_deref(), e.data_practice, e.connect_scope.clone()), (Some(&b"v=1"[..]), Some(1), None));
    assert_eq!(e.verify(&ids), Ok(()));
    let wrong = CatalogEntry::parse(&fixture("N-wrong-signer-catalog")).unwrap();
    assert!(wrong.verify(&ids).is_err(), "signed by a key the entry does not name as owner");
    let scoped = CatalogEntry::parse(&fixture("P-catalog-scoped")).unwrap();
    assert_eq!(scoped.connect_scope, Some(Scope::Dunbar));
    // built here, an entry with the same fields is the same bytes
    let f = EntryFields { resource: e.resource, service_type: e.service_type.clone(), instance: e.instance.clone(), endpoint: e.endpoint.clone(), connect_scope: None, metadata: e.metadata.clone(), data_practice: e.data_practice };
    let mine = CatalogEntry::build(w.id("bob"), &f);
    assert_eq!(CatalogEntry::parse(&mine).unwrap().verify(&ids), Ok(()));
    let (a, b) = (CatalogEntry::parse(&mine).unwrap(), e.clone());
    assert_eq!((a.resource, a.owner, a.endpoint, a.data_practice), (b.resource, b.owner, b.endpoint, b.data_practice));
    // the abuse report: by the resource it names, and not by another key
    let r = AbuseReport::parse(&fixture("P-abuse")).unwrap();
    assert_eq!((r.category, r.detail.as_deref()), (ABUSE_EXCESSIVE_LOAD, Some(&b"burst of 9k requests/min"[..])));
    assert_eq!(r.resource, e.resource, "the corpus's resource");
    // the resource's own key is not in the roster: unverifiable here, and the forgery under carol fails
    assert!(r.verify(&ids).is_err());
    let forged = AbuseReport::parse(&fixture("N-wrong-signer-abuse")).unwrap();
    assert!(forged.verify(&ids).is_err());
}

#[test]
fn the_scope_forms_and_the_frames_and_replies_round_trip() {
    assert_eq!(Scope::decode(&fixture("P-scope-self")).unwrap(), Scope::Own);
    assert_eq!(Scope::decode(&fixture("P-scope-down2")).unwrap(), Scope::Down(2));
    assert_eq!(Scope::Down(2).encode(), fixture("P-scope-down2"));
    assert!(matches!(Scope::decode(&fixture("B-scope-256")).unwrap(), Scope::List(l) if l.len() == 256));
    assert!(Scope::decode(&fixture("B-scope-257")).is_err());
    assert!(Scope::decode(&fixture("N-scope-unordered")).is_err());
    assert!(Scope::decode(&fixture("N-scope-duplicate")).is_err());
    assert!(Scope::decode(&[3]).is_err(), "the retired tag");
    for s in [Scope::Own, Scope::Up(3), Scope::Siblings, Scope::Dunbar, Scope::List(vec![[1; 32], [2; 32]])] {
        assert_eq!(Scope::decode(&s.encode()).unwrap(), s);
    }
    let q = CatalogQuery::decode(&body_of_frame(&fixture("P-frame-14"))).unwrap();
    assert_eq!(q.service_type.as_deref(), Some("rhtn-forum"));
    assert_eq!(q.encode(), body_of_frame(&fixture("P-frame-14")));
    let rr = ResourceRequest::decode(&body_of_frame(&fixture("P-frame-15"))).unwrap();
    assert!(rr.message.starts_with(b"GET /threads/42 HTTP/1.1\r\n"));
    assert_eq!(rr.encode(), body_of_frame(&fixture("P-frame-15")));
    let reg = ResourceRegistration::decode(&body_of_frame(&fixture("P-frame-16"))).unwrap();
    assert_eq!((reg.scope.clone(), reg.entry.clone()), (None, fixture("P-catalog")));
    assert_eq!(reg.encode(), body_of_frame(&fixture("P-frame-16")));
    let cr = CatalogReply::decode(&fixture("P-reply-07")).unwrap();
    assert_eq!((cr.entries.len(), cr.continuation.clone()), (1, None));
    assert_eq!(cr.encode(), fixture("P-reply-07"));
    let tr = CatalogReply::decode(&fixture("P-catalog-reply-truncated")).unwrap();
    assert_eq!((tr.entries.len(), tr.continuation.as_deref()), (111, Some("rhtn-wiki")));
    let r8 = ResourceResponse::decode(&fixture("P-reply-08")).unwrap();
    assert_eq!((r8.status, r8.body.as_deref()), (STATUS_DELIVERED, Some(&b"HTTP/1.1 200 OK\r\ncontent-length: 2\r\n\r\nok"[..])));
    assert_eq!(r8.encode(), fixture("P-reply-08"));
    let r9 = ResourceResponse::decode(&fixture("P-reply-09")).unwrap();
    assert_eq!((r9.status, r9.body), (STATUS_REFUSED, None));
    let r12 = RegistrationReply::decode(&fixture("P-reply-12")).unwrap();
    assert_eq!(r12.code, REGISTRATION_RECORDED);
    assert_eq!(r12.encode(), fixture("P-reply-12"));
}
