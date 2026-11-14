//! **The client's state as relations** (restructure step 5): six blobs,
//! each versioned, read whole or refused whole, equal in what they hold
//! to the one blob every client wrote before them.

mod common;

use common::net::*;
use common::*;
use rhtn_client::ceremony::{RELATION_VERSION, RELATIONS};
use rhtn_codec::cbor::{Item, parse_all};

// functional: APP-016
#[test]
fn the_relations_hold_what_the_one_blob_held_and_a_missing_or_newer_one_refuses_whole() {
    let mut n = net(&["w1"], &[("alice", "w1"), ("bob", "w1")]);
    n.attach("bob");
    n.attach("alice");
    n.sweep("alice");
    n.send("alice", "bob", "kept");
    let parts: Vec<(String, Vec<u8>)> =
        n.s.client("alice")
            .durable_parts()
            .into_iter()
            .map(|(k, v)| (k.to_string(), v))
            .collect();
    assert_eq!(
        parts.iter().map(|(k, _)| k.as_str()).collect::<Vec<_>>(),
        RELATIONS.to_vec(),
        "every relation, in the order written"
    );
    for (_, b) in &parts {
        let item = parse_all(b).expect("a relation parses");
        let Item::Array(a) = &item else {
            panic!("not a relation: {item:?}")
        };
        assert_eq!(a.len(), 2, "[version, body]");
        assert!(matches!(a[0], Item::Uint(v) if v == RELATION_VERSION));
    }
    let blob = n.s.client("alice").durable();

    let mut again = fresh("alice");
    again.restore_parts(&parts).expect("the relations open");
    assert_eq!(
        again.durable(),
        blob,
        "what the relations hold is what the blob held"
    );

    // one relation missing: nothing changes
    let mut third = fresh("alice");
    let before = third.durable();
    let short: Vec<(String, Vec<u8>)> = parts
        .iter()
        .filter(|(k, _)| k != "payload")
        .cloned()
        .collect();
    assert!(third.restore_parts(&short).is_err());
    assert_eq!(third.durable(), before);
    // a version this reader does not know: refused by name
    let mut newer = parts.clone();
    let i = newer.iter().position(|(k, _)| k == "store").unwrap();
    let mut b = Vec::new();
    rhtn_codec::encode::emit_array_head(&mut b, 2);
    rhtn_codec::encode::emit_uint(&mut b, RELATION_VERSION + 1);
    rhtn_codec::encode::emit_bstr(&mut b, b"");
    newer[i].1 = b;
    let why = third.restore_parts(&newer).unwrap_err();
    assert!(why.contains("store") && why.contains("version"), "{why}");
    assert_eq!(third.durable(), before);
}

fn fresh(name: &str) -> rhtn_client::ceremony::Client {
    let clock = std::rc::Rc::new(std::cell::Cell::new(1_790_000_000_000u64));
    let (device, _) = common::harness::device(clock, 7, 0);
    rhtn_client::ceremony::Client::new(id(name), ids(), Default::default(), device)
}
