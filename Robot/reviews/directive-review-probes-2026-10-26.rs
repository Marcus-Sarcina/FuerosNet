//! Review counterexamples, not acceptance tests: a passing assertion confirms
//! the observed defect in the implementation reviewed on 2026-10-26.
//! Compile against rhtn-node after building it; see the accompanying review.
use rhtn_node::federation::*;
use std::cell::{Cell, RefCell};
use std::collections::BTreeSet;

fn k(n: u8) -> [u8; 32] {
    [n; 32]
}
fn setup() -> Federation {
    let mut f = Federation::default();
    f.configure(
        k(2),
        Relationships {
            patron: Some(k(1)),
            ..Default::default()
        },
    )
    .unwrap();
    f
}
fn e(revision: u64) -> Envelope {
    Envelope::live(k(1), "policy", "test@1", revision, b"x")
}

#[derive(Default)]
struct Script {
    messages: RefCell<Vec<String>>,
    fail_first: Cell<bool>,
    fail_all: bool,
    fail_relationships: bool,
    only_source: bool,
    emitted: Cell<bool>,
}
impl Carrier for Script {
    fn authority(&self, r: &[u8; 32]) -> Option<String> {
        (!self.only_source || *r == k(1)).then(|| "test.internal".into())
    }
    fn hand(&self, _: &[u8; 32], message: &[u8]) -> Option<Result<Vec<u8>, String>> {
        let text = String::from_utf8(message.to_vec()).unwrap();
        self.messages.borrow_mut().push(text.clone());
        let delivery = text.starts_with("PUT /.rhtn/envelope");
        let reject = (delivery && (self.fail_all || self.fail_first.replace(false)))
            || (self.fail_relationships && text.starts_with("PUT /.rhtn/relationships"));
        let body = if self.only_source
            && text.starts_with("GET /.rhtn/outbox")
            && !self.emitted.replace(true)
        {
            format!(
                "{{\"emit\":[{{\"to\":\"subordinates\",\"envelope\":{}}}]}}",
                e(1).to_json()
            )
        } else {
            String::new()
        };
        Some(Ok(format!(
            "HTTP/1.1 {} X\r\ncontent-length: {}\r\n\r\n{}",
            if reject { 503 } else { 200 },
            body.len(),
            body
        )
        .into_bytes()))
    }
}

#[test]
fn unsigned_future_revision_poisoning_precedes_application_verification() {
    let mut f = setup();
    assert_eq!(f.deliver(&k(2), e(u64::MAX)).unwrap(), Outcome::New);
    assert_eq!(f.deliver(&k(2), e(1)).unwrap(), Outcome::Older);
    assert_eq!(f.plane(&k(2)).unwrap().waiting(), 1);
}

#[test]
fn retry_delivers_old_revision_after_a_newer_one() {
    let mut f = setup();
    f.deliver(&k(2), e(1)).unwrap();
    f.deliver(&k(2), e(2)).unwrap();
    let s = Script {
        fail_first: Cell::new(true),
        ..Default::default()
    };
    f.pass(&s);
    f.pass(&s);
    let messages = s.messages.borrow();
    let delivered: Vec<_> = messages
        .iter()
        .filter(|m| m.starts_with("PUT /.rhtn/envelope"))
        .collect();
    assert_eq!(delivered.len(), 3);
    assert!(delivered[0].contains("\"revision\":1"));
    assert!(delivered[1].contains("\"revision\":2"));
    assert!(delivered[2].contains("\"revision\":1"));
}

#[test]
fn rejected_relationship_snapshot_does_not_gate_delivery() {
    let mut f = setup();
    f.deliver(&k(2), e(1)).unwrap();
    let s = Script {
        fail_relationships: true,
        ..Default::default()
    };
    assert_eq!(f.pass(&s).delivered, 1);
    assert!(!f.plane(&k(2)).unwrap().relationships_told);
}

#[test]
fn removing_patron_does_not_revoke_queued_patron_delivery() {
    let mut f = setup();
    f.deliver(&k(2), e(1)).unwrap();
    f.configure(k(2), Relationships::default()).unwrap();
    let s = Script::default();
    assert_eq!(f.pass(&s).delivered, 1);
    assert!(
        s.messages
            .borrow()
            .iter()
            .any(|m| m.starts_with("PUT /.rhtn/envelope") && m.contains("rhtn-relation: patron"))
    );
}

#[test]
fn full_local_inbox_discards_sender_pending_item() {
    let mut f = setup();
    f.configure(
        k(1),
        Relationships {
            subordinates: BTreeSet::from([k(2)]),
            ..Default::default()
        },
    )
    .unwrap();
    for n in 0..MAX_INBOX {
        let mut item = e(1);
        item.record = format!("fill-{n}");
        assert_eq!(f.deliver(&k(2), item).unwrap(), Outcome::New);
    }
    let s = Script {
        only_source: true,
        ..Default::default()
    };
    let result = f.pass(&s);
    assert_eq!(result.refused.len(), 1);
    assert_eq!(f.plane(&k(2)).unwrap().waiting(), MAX_INBOX);
    assert!(f.plane(&k(1)).unwrap().pending.is_empty());
    assert!(f.plane(&k(1)).unwrap().dropped.is_empty());
}

#[test]
fn dropped_delivery_cannot_be_reoffered_at_the_same_revision() {
    let mut f = setup();
    f.deliver(&k(2), e(1)).unwrap();
    let s = Script {
        fail_all: true,
        ..Default::default()
    };
    for _ in 0..MAX_ATTEMPTS {
        f.pass(&s);
    }
    assert_eq!(f.plane(&k(2)).unwrap().waiting(), 0);
    assert_eq!(f.plane(&k(2)).unwrap().dropped.len(), 1);
    assert_eq!(f.deliver(&k(2), e(1)).unwrap(), Outcome::Duplicate);
    assert_eq!(f.plane(&k(2)).unwrap().waiting(), 0);
}

#[test]
fn bodyless_live_offer_suppresses_later_complete_copy() {
    let mut f = setup();
    let mut missing = e(1);
    missing.body = None;
    assert_eq!(f.deliver(&k(2), missing).unwrap(), Outcome::New);
    assert_eq!(f.deliver(&k(2), e(1)).unwrap(), Outcome::Duplicate);
}

#[test]
fn encoded_reserved_path_passes_the_current_guard() {
    let r = rhtn_node::http::parse(
        b"PUT /%2erhtn/envelope HTTP/1.1\r\nhost: x\r\ncontent-length: 0\r\n\r\n",
    )
    .unwrap();
    assert!(!r.target.starts_with(PATH_PREFIX));
    assert_eq!(r.target.replace("%2e", "."), PATH_ENVELOPE);
}

#[test]
fn removing_destination_does_not_revoke_pending_remote_delivery() {
    let mut f = Federation::default();
    f.configure(
        k(1),
        Relationships {
            subordinates: BTreeSet::from([k(2)]),
            ..Default::default()
        },
    )
    .unwrap();
    f.locate(
        k(2),
        Location {
            address: "127.0.0.1:7452".parse().unwrap(),
            key: k(3),
        },
    )
    .unwrap();
    f.pass(&Script {
        only_source: true,
        ..Default::default()
    });
    f.configure(k(1), Relationships::default()).unwrap();
    assert_eq!(f.abroad().len(), 1);
}

#[test]
fn gateway_delivers_encoded_host_path_to_a_decoding_backend() {
    use rhtn_archive::{catalog::*, topology::Table};
    use rhtn_node::resources::*;
    use std::sync::{Arc, Mutex};
    #[derive(Default)]
    struct DecodingBackend(Mutex<bool>);
    impl Backend for DecodingBackend {
        fn running(&self) -> bool {
            true
        }
        fn handle(&self, message: &[u8]) -> Result<Vec<u8>, String> {
            let request = rhtn_node::http::parse(message).unwrap();
            *self.0.lock().unwrap() = request.target.replace("%2e", ".") == PATH_ENVELOPE;
            Ok(b"HTTP/1.1 200 OK\r\ncontent-length: 0\r\n\r\n".to_vec())
        }
    }
    let backend = Arc::new(DecodingBackend::default());
    let mut gateway = Gateway::default();
    gateway.bind(
        k(2),
        Binding {
            owner: k(1),
            authority: "test.internal".into(),
            backend: Some(backend.clone()),
            declared: Manifest::default(),
        },
    );
    gateway
        .set_row(
            k(2),
            k(1),
            Row {
                connect: true,
                ..Default::default()
            },
        )
        .unwrap();
    let request = ResourceRequest {
        resource: k(2),
        message: b"PUT /%2erhtn/envelope HTTP/1.1\r\nhost: x\r\ncontent-length: 0\r\n\r\n".to_vec(),
    }
    .encode();
    assert_eq!(
        gateway
            .serve(&k(1), &Table::with_me(k(1)), &k(1), &request)
            .status,
        STATUS_DELIVERED
    );
    assert!(*backend.0.lock().unwrap());
}
