//! **When a doorbell is owed** (design §14.1.5; `infra-client-requirements.md`
//! §6.1; SUB-14): one per relay taken for a client away from this node that
//! holds a live endpoint, the device's own first; none for a client with a
//! session here, none after a withdrawal, none for a lapsed endpoint, and
//! nothing kept of a ring once it is drained.

mod common;

use common::*;
use rhtn_archive::tx::{Locator, Seqno};
use rhtn_node::view::NodeView;
use std::sync::Arc;

fn node() -> NodeView {
    let mut v = NodeView::new(
        Arc::new(id("bob")),
        Locator::root(
            kh("bob"),
            Seqno {
                series: 1,
                counter: 0,
            },
        ),
    );
    v.set_now(1_800_000_000);
    v
}

// functional: MAIL-020
#[test]
fn a_ring_is_owed_per_arrival_for_an_away_client_with_a_live_endpoint_and_never_otherwise() {
    let mut v = node();
    let dev = [1u8; 32];
    v.wake.register(
        kh("alice"),
        dev,
        Some("https://push.example/alice".into()),
        Some(vec![7; 81]),
        Some(1_800_000_000 + 3600),
        Some([3; 32]),
    );
    assert!(
        !v.ring_for(&kh("alice"), &dev, false),
        "a client with a session here is not rung"
    );
    assert!(v.ring_for(&kh("alice"), &dev, true));
    assert!(v.ring_for(&kh("alice"), &dev, true), "one per arrival");
    let rings = v.take_rings();
    assert_eq!(rings.len(), 2);
    assert!(
        rings
            .iter()
            .all(|e| e.url == "https://push.example/alice" && e.signing_key == [3; 32])
    );
    assert!(v.take_rings().is_empty(), "drained, nothing kept");
    // another device of alice's that registered nothing rings what alice has
    assert!(v.ring_for(&kh("alice"), &[2u8; 32], true));
    assert_eq!(v.take_rings().len(), 1);
    // nobody registered: nothing
    assert!(!v.ring_for(&kh("carol"), &dev, true));
    // lapsed: nothing
    let mut late = node();
    late.wake.register(
        kh("alice"),
        dev,
        Some("https://push.example/alice".into()),
        Some(vec![7; 81]),
        Some(1_800_000_000 - 1),
        Some([3; 32]),
    );
    assert!(!late.ring_for(&kh("alice"), &dev, true));
    // withdrawn: nothing
    v.wake.register(kh("alice"), dev, None, None, None, None);
    assert!(!v.ring_for(&kh("alice"), &dev, true));
    assert!(v.take_rings().is_empty());
}
