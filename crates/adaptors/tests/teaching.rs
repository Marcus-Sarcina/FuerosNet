//! **A node learns the subnet from the clients that attach to it**
//! (`wire-format.md` §10.1.1: a client's floods enter and leave through
//! the node that serves it).
//!
//! The defect this is written against [author, 2026-11-01]: a node
//! replayed its whole store to a client that attached, and to the patron
//! it attached to, and **nothing replayed the other way**. So a node
//! enrolled into a subnet that already existed held none of it: it could
//! materialise no role row for a member it had never heard of, resolve no
//! position, and the topology did not survive a change of hosting — which
//! it must, since an operator may move their instance and a subnet may
//! outlive any one of its nodes.
//!
//! What the fix is: an attach teaches. Each party hands the node the acts
//! it was party to, which is all it keeps of them
//! (`infra-client-requirements.md` §4.3), and the union of the members'
//! archives is the subnet.

mod common;

use common::*;
use rhtn_adaptors::actor::Handle;
use rhtn_adaptors::courier::Courier;
use rhtn_adaptors::direct::{NoDirect, Reachable};
use rhtn_adaptors::serving::{Inboxes, LocalNode, Serving};
use rhtn_archive::topology::Table;
use rhtn_client::ceremony::Config;
use std::sync::Arc;

/// **What a party to a record holds**: the records it signed, in order,
/// in its own archive — a chain, since an archive takes a record whose
/// predecessors it holds and no other.
fn mine(records: &[rhtn_archive::record::Record], name: &str) -> Vec<Vec<u8>> {
    records
        .iter()
        .filter(|r| r.back_pointers_of(&kh(name)).is_some())
        .map(|r| r.bytes.clone())
        .collect()
}

/// A client beside the node with no direct path, holding its own acts out
/// of `records` in its archive.
fn party(
    name: &'static str,
    records: &[rhtn_archive::record::Record],
    serving: &Arc<dyn Serving>,
    inboxes: &Inboxes,
) -> (Handle, Arc<Courier>) {
    let reachable = Reachable::default();
    let handle = spawn_client(name, Config::default(), reachable.clone());
    let held = mine(records, name);
    let who = name.to_string();
    handle.with_blocking(move |c| {
        for bytes in &held {
            let rec = rhtn_archive::record::Record::parse(bytes).expect("a record");
            c.archive
                .append(rec)
                .unwrap_or_else(|e| panic!("{who}'s own archive takes its own record: {e}"));
        }
    });
    let (courier, _app) = Courier::new(
        handle.clone(),
        serving.clone(),
        Arc::new(NoDirect(reachable)),
    );
    inboxes.host(kh(name), courier.inbound());
    (handle, courier)
}

// acceptance: PRP-29
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_node_that_holds_nothing_is_taught_the_subnet_by_the_members_that_attach() {
    // **the node speaks as its operator and holds an empty table**, which
    // is a node enrolled into a subnet that already exists: alice's own
    // client made these records before this node existed
    let node = live_node("alice", Table::with_me(kh("alice")), "alice", &[]);
    let inboxes = Inboxes::default();
    let serving: Arc<dyn Serving> = LocalNode::new(node.clone(), inboxes.clone());

    // the subnet as the phones made it: alice adopted bob, then bob
    // adopted carol under itself
    let mut scene = Scene::new();
    scene.adopt("bob", "alice", "alice", &[]);
    scene.adopt("carol", "bob", "alice", &[1]);

    // nothing yet: the node has never heard of either of them
    assert!(
        !node
            .view
            .lock()
            .unwrap()
            .table
            .horizon(&kh("alice"), 2)
            .contains(&kh("bob")),
        "the node starts knowing nothing of the subnet"
    );

    // **the operator's own client attaches**, and teaches what it was
    // party to: its adoption of bob
    let (_h1, alice) = party("alice", &scene.records, &serving, &inboxes);
    alice.attach(vec![kh("alice")]).await;
    assert!(
        until(2_000, || node
            .view
            .lock()
            .unwrap()
            .table
            .horizon(&kh("alice"), 2)
            .contains(&kh("bob")))
        .await,
        "after the operator attaches, the node holds its adoption of bob"
    );

    // **and the subordinate teaches the rest**: bob was party to its own
    // adoption of carol, which alice holds only as a fact and could not
    // have replayed (`infra-client-requirements.md` §4.3)
    let (_h2, bob) = party("bob", &scene.records, &serving, &inboxes);
    bob.attach(vec![kh("bob")]).await;
    assert!(
        until(2_000, || node
            .view
            .lock()
            .unwrap()
            .table
            .horizon(&kh("alice"), 2)
            .contains(&kh("carol")))
        .await,
        "after the subordinate attaches, the node holds the whole subnet"
    );

    // which is what a role row needs: the member is inside the owner's
    // horizon as the node itself computes it
    let horizon = node.view.lock().unwrap().table.horizon(&kh("alice"), 2);
    assert!(horizon.contains(&kh("alice")));
    assert!(horizon.contains(&kh("bob")));
    assert!(horizon.contains(&kh("carol")));
}

// acceptance: PRP-29
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_client_with_nothing_to_teach_teaches_nothing_and_attaches_as_before() {
    let node = live_node("alice", Table::with_me(kh("alice")), "alice", &[]);
    let inboxes = Inboxes::default();
    let serving: Arc<dyn Serving> = LocalNode::new(node.clone(), inboxes.clone());
    let (handle, courier) = party("alice", &[], &serving, &inboxes);
    let carried = courier.attach(vec![kh("alice")]).await;
    assert!(carried.refused.is_empty(), "{:?}", carried.refused.len());
    assert!(carried.left.is_empty());
    // and it says so: nothing held, nothing offered
    let frames: Vec<Vec<u8>> = handle.with_blocking(|c| c.replay_frames());
    assert!(frames.is_empty());
}

// acceptance: PRP-29
#[test]
fn what_a_client_replays_is_its_own_archive_and_the_push_frame_the_wire_names() {
    let mut scene = Scene::new();
    scene.adopt("bob", "alice", "alice", &[]);
    let held = mine(&scene.records, "alice");
    let reachable = Reachable::default();
    let handle = spawn_client("alice", Config::default(), reachable);
    let records = held.clone();
    handle.with_blocking(move |c| {
        for bytes in &records {
            let rec = rhtn_archive::record::Record::parse(bytes).expect("a record");
            c.archive.append(rec).expect("its own record");
        }
    });
    let frames: Vec<Vec<u8>> = handle.with_blocking(|c| c.replay_frames());
    // one frame per record it signed: the presence record the meeting
    // made and the adoption that rests on it
    assert_eq!(frames.len(), held.len());
    assert!(held.len() >= 2, "a meeting and an adoption");
    // the frame is §10.1's push: the body-kind tag and the object, and
    // kind 0 is a transaction
    for (frame, bytes) in frames.iter().zip(&held) {
        let (kind, object) = rhtn_archive::topology::decode_push(frame).expect("a push frame");
        assert_eq!(kind, 0);
        assert_eq!(&object, bytes);
    }
}
