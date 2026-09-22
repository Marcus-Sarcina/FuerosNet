//! The client's catalog: a sweep that reads a repeated continuation as
//! truncation, a brokered service routed to only where it matches its
//! signed entry, a page shown as served, and an unrecognised declaration
//! surfaced.

mod common;

use common::*;
use rhtn_archive::catalog::*;
use rhtn_client::catalog::*;
use rhtn_client::notice::{Notice, Notifier};
use std::cell::RefCell;

struct Hook(RefCell<Vec<Notice>>);
impl Notifier for Hook {
    fn notify(&self, n: Notice) {
        self.0.borrow_mut().push(n);
    }
}

fn entry(
    owner: &str,
    resource: [u8; 32],
    service_type: &str,
    endpoint: &[u8],
    practice: Option<u64>,
) -> Vec<u8> {
    CatalogEntry::build(
        &id(owner),
        &EntryFields {
            resource,
            service_type: service_type.into(),
            instance: "x".into(),
            endpoint: endpoint.to_vec(),
            connect_scope: None,
            metadata: None,
            data_practice: practice,
        },
    )
}

// acceptance: RSC-23
#[test]
fn a_repeated_continuation_is_truncation_not_another_page() {
    let node = kh("w1");
    let mut view = View::default();
    let mut sweep = Sweep::default();
    // the host answers every forum query with the same full page and the same hint
    let page: Vec<Vec<u8>> = (0..111u32)
        .map(|i| {
            entry(
                "bob",
                rhtn_codec::cose::sha256(&i.to_be_bytes()),
                "rhtn-forum",
                b"e",
                None,
            )
        })
        .collect();
    let answer = |q: &CatalogQuery| CatalogReply {
        nonce: q.nonce,
        entries: page.clone(),
        continuation: Some("rhtn-forum".into()),
    };
    let q1 = sweep.query(None, [1; 16]);
    let step = sweep.take(&ids(), view.portion(node), &answer(&q1), 111);
    assert_eq!(
        step,
        Step::Again("rhtn-forum".into()),
        "the first hint is followed"
    );
    let q2 = sweep.query(Some("rhtn-forum".into()), [2; 16]);
    assert_eq!(q2.service_type.as_deref(), Some("rhtn-forum"));
    let step = sweep.take(&ids(), view.portion(node), &answer(&q2), 111);
    assert_eq!(
        step,
        Step::Truncated,
        "a full page and a hint already asked for"
    );
    assert!(view.portion(node).truncated, "H's portion marked truncated");
    assert_eq!(
        sweep.queries, 2,
        "asked for that type at most once more, then stopped"
    );
    assert_eq!(view.portion(node).entries.len(), 111);
    // a portion that completes: no continuation
    let mut s2 = Sweep::default();
    let q = s2.query(None, [3; 16]);
    assert_eq!(
        s2.take(
            &ids(),
            view.portion(kh("w2")),
            &CatalogReply {
                nonce: q.nonce,
                entries: page[..3].to_vec(),
                continuation: None
            },
            111
        ),
        Step::Done
    );
    assert!(!view.portion(kh("w2")).truncated);
    assert_eq!(view.entries().len(), 111 + 3);
}

/// **A reply that does not echo the query's nonce is not that query's
/// answer** (`wire-format.md` §6.4), and nothing is read out of it.  The
/// same rule `resolution` and `currency` already apply to their own
/// replies; the catalog sweep is where it was missing.
// acceptance: RSC-37
#[test]
fn a_reply_under_a_nonce_the_sweep_did_not_send_is_not_taken() {
    let node = kh("w3");
    let mut view = View::default();
    let mut sweep = Sweep::default();
    let page: Vec<Vec<u8>> = (0..3u32)
        .map(|i| {
            entry(
                "bob",
                rhtn_codec::cose::sha256(&i.to_be_bytes()),
                "rhtn-forum",
                b"e",
                None,
            )
        })
        .collect();
    let q = sweep.query(None, [4; 16]);
    assert_eq!(q.nonce, [4; 16]);

    let forged = CatalogReply {
        nonce: [5; 16],
        entries: page.clone(),
        continuation: None,
    };
    assert_eq!(
        sweep.take(&ids(), view.portion(node), &forged, 111),
        Step::WrongNonce,
        "not this query's answer"
    );
    assert!(
        view.portion(node).entries.is_empty(),
        "and no entry out of it reaches the view"
    );

    // the honest reply to the still-outstanding query is taken
    let answer = CatalogReply {
        nonce: [4; 16],
        entries: page.clone(),
        continuation: None,
    };
    assert_eq!(
        sweep.take(&ids(), view.portion(node), &answer, 111),
        Step::Done
    );
    assert_eq!(view.portion(node).entries.len(), 3);
}

// acceptance: RSC-24
#[test]
fn a_brokered_service_is_routed_to_only_where_it_matches_the_signed_entry() {
    let e = CatalogEntry::parse(&entry(
        "bob",
        [7; 32],
        "saas-tracker",
        b"https://x.example/",
        None,
    ))
    .unwrap();
    assert_eq!(
        route_brokered(&e, b"https://y.example/"),
        Err(Refusal::EndpointDiffers {
            signed: b"https://x.example/".to_vec(),
            presented: b"https://y.example/".to_vec()
        }),
        "no traffic to Y"
    );
    let r = route_brokered(&e, b"https://x.example/").unwrap();
    assert_eq!(
        (r.endpoint.as_slice(), r.owner),
        (&b"https://x.example/"[..], kh("bob")),
        "the owner named in the entry is shown"
    );
    // the page: what the node served, with the roles held; nothing re-expanded
    let served = vec![(
        entry(
            "bob",
            [7; 32],
            "saas-tracker",
            b"https://x.example/",
            Some(0),
        ),
        vec!["member".to_string()],
    )];
    let shown = page(&ids(), &served);
    assert_eq!(shown.len(), 1);
    assert_eq!(
        (shown[0].entry.resource, shown[0].roles.clone()),
        ([7; 32], vec!["member".to_string()])
    );
    // a forged entry the node would not have served is not shown either
    let mut forged = entry("bob", [8; 32], "saas-tracker", b"https://z.example/", None);
    let n = forged.len();
    forged[n - 1] ^= 1;
    assert!(page(&ids(), &[(forged, vec![])]).is_empty());
}

#[test]
fn an_unrecognised_declaration_is_surfaced_and_a_known_one_is_not() {
    let hook = Hook(RefCell::new(vec![]));
    let odd = CatalogEntry::parse(&entry("bob", [1; 32], "t", b"e", Some(9))).unwrap();
    surface_declaration(&odd, &hook);
    assert_eq!(
        hook.0.borrow().as_slice(),
        &[Notice::UnrecognisedDeclaration {
            resource: [1; 32],
            value: 9
        }]
    );
    let known = CatalogEntry::parse(&entry("bob", [2; 32], "t", b"e", Some(2))).unwrap();
    surface_declaration(&known, &hook);
    let none = CatalogEntry::parse(&entry("bob", [3; 32], "t", b"e", None)).unwrap();
    surface_declaration(&none, &hook);
    assert_eq!(hook.0.borrow().len(), 1);
}

/// The sweep, driven from the client the way production does it: a browse
/// posts the query, a reply is taken, and the continuation goes back out
/// until the node's portion is complete.  Answered by the real
/// `CatalogService`, so the two halves meet over the bytes rather than
/// over a fixture.
// acceptance: RSC-39
#[test]
fn a_client_browses_its_serving_nodes_catalog_and_follows_the_continuation() {
    use rhtn_client::ceremony::Msg;
    use rhtn_node::catalog::{CatalogService, ScopeEval};

    /// Every asker is in the horizon, and every scope admits: this test is
    /// about the sweep, and `node/tests/resources.rs` is about the gates.
    struct Open;
    impl ScopeEval for Open {
        fn in_horizon(&self, _: &rhtn_client::Keyhash) -> bool {
            true
        }
        fn admits(&self, _: &Scope, _: &rhtn_client::Keyhash, _: &rhtn_client::Keyhash) -> bool {
            true
        }
    }

    let mut svc = CatalogService::default();
    for (i, t) in ["rhtn-forum", "rhtn-wiki"].iter().enumerate() {
        let e = entry(
            "bob",
            rhtn_codec::cose::sha256(&(i as u32).to_be_bytes()),
            t,
            b"https://x.example/",
            Some(0),
        );
        let body = ResourceRegistration {
            entry: e,
            scope: None,
            nonce: [i as u8; 16],
        }
        .encode();
        let reply = svc.register(&ids(), &kh("bob"), &body).expect("answered");
        assert_eq!(
            RegistrationReply::decode(&reply).unwrap().code,
            0,
            "recorded"
        );
    }

    let mut c = fresh("alice");
    assert!(
        c.browse().is_empty(),
        "a client attached to nobody has nobody to ask"
    );
    c.attach(kh("w1"), &[]);

    let posted = c.browse();
    let [Msg::CatalogQuery(q)] = posted.as_slice() else {
        panic!("one query posted: {posted:?}")
    };
    let reply = svc
        .answer(&kh("alice"), q, &Open)
        .expect("the node answers a member");
    let (step, next) = c.take_catalog_reply(&reply).expect("taken");
    assert_eq!(
        step,
        Step::Done,
        "both entries fit one page, so there is no continuation"
    );
    assert!(next.is_empty(), "and nothing more to ask");
    assert_eq!(
        c.catalog.entries().len(),
        2,
        "the portion holds what the node served"
    );
    assert_eq!(
        c.catalog.entries()[0].0,
        kh("w1"),
        "attributed to the node that served it"
    );

    // a reply under a nonce no sweep sent is not taken, and the sweep
    // stays outstanding for the honest one
    let mut c2 = fresh("alice");
    c2.attach(kh("w1"), &[]);
    let posted = c2.browse();
    let [Msg::CatalogQuery(q2)] = posted.as_slice() else {
        panic!("one query")
    };
    let forged = CatalogReply {
        nonce: [0xee; 16],
        entries: vec![],
        continuation: None,
    }
    .encode();
    let (step, next) = c2.take_catalog_reply(&forged).expect("decodes");
    assert_eq!(step, Step::WrongNonce, "not this sweep's answer");
    assert!(next.is_empty());
    let honest = svc.answer(&kh("alice"), q2, &Open).expect("answered");
    assert_eq!(
        c2.take_catalog_reply(&honest).expect("taken").0,
        Step::Done,
        "the query it did send is still outstanding"
    );
    assert_eq!(c2.catalog.entries().len(), 2);
}

/// A client with nothing configured, for the paths that need one.
fn fresh(name: &str) -> rhtn_client::ceremony::Client {
    let clock = std::rc::Rc::new(std::cell::Cell::new(1_790_000_000_000u64));
    let (device, _) = common::harness::device(vec![], clock, 7, 0);
    rhtn_client::ceremony::Client::new(id(name), ids(), Default::default(), device)
}
