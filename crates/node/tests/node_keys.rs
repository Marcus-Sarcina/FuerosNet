//! The node-key service at a running node (`wire-format.md` §7.8, §7.10;
//! `infra-client-requirements.md` §6; design §14.2.4.3): a key per
//! recipient, served to whoever asks and consumed by nobody, rotated, a
//! submission taken under the newest or the one before within its grace,
//! each delivery carrying the node's half, and a private half deleted with
//! the last message sealed to it.
//!
//! Every request here arrives on a connection under a key minted for it,
//! as a sender's does; the recipient collects on a session of its own.

mod common;

use common::*;
use rhtn_archive::node_key::{FAIL_NOT_SERVED, NodeKey};
use rhtn_archive::submission::{SUBMISSION_ACCEPTED, SUBMISSION_REFUSED, unrelayed};
use rhtn_node::node_keys::NodeKeyConfig;
use rhtn_transport::session::RETENTION_SECS;

const T0: u64 = 1_900_000_000;
const DAY: u64 = 86_400;

fn temp(tag: &str) -> std::path::PathBuf {
    let d = std::env::temp_dir().join(format!("rhtn-node-keys-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    d
}

fn node_key(record: &[u8]) -> NodeKey {
    NodeKey::parse(record).expect("a node key")
}

// acceptance: PAY-04
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_node_key_is_served_to_whoever_asks_and_nothing_is_consumed() {
    let b = Bench::start(T0, None, NodeKeyConfig::default(), |cfg| {
        let stranger = kh("w9");
        cfg.serves = std::sync::Arc::new(move |k| *k != stranger);
    });
    // N serves S and holds a node key for it
    let held = {
        let mut v = b.live.view.lock().unwrap();
        let signer = v.ack_signer().unwrap();
        let now = v.now();
        v.node_keys.newest(&*signer, &kh("carol"), now)
    };
    let before = b.live.view.lock().unwrap().node_keys.held(&kh("carol"));
    // two requests for S from different peers
    let (c1, _) = b.minted(1).await;
    let (c2, _) = b.minted(2).await;
    let r1 = fetch(&c1, &[kh("carol")], [1; 16]).await;
    let r2 = fetch(&c2, &[kh("carol")], [2; 16]).await;
    assert_eq!(r1[0].nonce, [1; 16]);
    assert_eq!(r2[0].nonce, [2; 16]);
    assert_eq!(r1[0].outcome, Ok(held.clone()), "the newest N holds");
    assert_eq!(r2[0].outcome, Ok(held.clone()), "the same key to both");
    let k = node_key(&held);
    assert_eq!(k.fields.recipient, kh("carol"), "field 1 names S");
    assert_eq!(k.fields.node, kh("bob"), "field 2 names N");
    k.verify(&ids()).expect("signed by N");
    // what N holds for S is unchanged by the serving
    assert_eq!(
        b.live.view.lock().unwrap().node_keys.held(&kh("carol")),
        before
    );
    assert_eq!(
        fetch(&c1, &[kh("carol")], [4; 16]).await[0].outcome,
        Ok(held)
    );
    // and a keyhash N does not serve gets no key and failure code 0
    let r3 = fetch(&c2, &[kh("w9")], [3; 16]).await;
    assert_eq!(r3[0].nonce, [3; 16]);
    assert_eq!(r3[0].outcome, Err(FAIL_NOT_SERVED));
    assert!(
        b.live
            .view
            .lock()
            .unwrap()
            .node_keys
            .held(&kh("w9"))
            .is_empty(),
        "nothing made for it"
    );
}

// acceptance: PAY-28
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_key_per_recipient_rotates_and_its_private_half_goes_with_its_last_message() {
    // the rotation interval as shipped, one day, against the test's clock
    let keys = NodeKeyConfig::default();
    assert_eq!(keys.rotate_secs, DAY);
    let b = Bench::start(T0, None, keys, |_| {});
    let (r1, r2) = (kh("carol"), kh("alice"));
    let carol = Recipient::of("carol");
    let first1 = b.node_key(&r1).await;
    let first2 = b.node_key(&r2).await;
    // one key per recipient, never one shared
    let (k1, k2) = (node_key(&first1), node_key(&first2));
    assert_ne!(k1.fields.public, k2.fields.public, "distinct keys");
    for (k, who) in [(&k1, r1), (&k2, r2)] {
        k.verify(&ids()).expect("verifies under N");
        assert_eq!(k.fields.recipient, who);
        assert_eq!(k.fields.node, kh("bob"));
    }
    assert_eq!(rhtn_codec::cose::aad::NODE_KEY, b"rhtn/1:node-key");
    // a message for R1 sealed to its key n
    let n = k1.fields.n;
    let sealed = carol.seal(&first1, b"for R1 under key n", 0);
    assert_eq!(b.submit(&r1, &sealed, 1).await, SUBMISSION_ACCEPTED);
    // the interval elapses twice, both keys fetched after each
    let mut last = (k1.fields.n, k2.fields.n);
    for i in 1..=2 {
        b.set(T0 + i * DAY);
        let (a, c) = (
            node_key(&b.node_key(&r1).await),
            node_key(&b.node_key(&r2).await),
        );
        assert!(
            a.fields.n > last.0 && c.fields.n > last.1,
            "a greater field 3"
        );
        assert_ne!(a.fields.public, c.fields.public);
        a.verify(&ids()).unwrap();
        c.verify(&ids()).unwrap();
        last = (a.fields.n, c.fields.n);
    }
    assert!(
        b.live.view.lock().unwrap().node_keys.held(&r1).contains(&n),
        "key n is held while its message is"
    );
    // while the message is held its delivery carries the half that opens it
    let (_, got) = b.collect("carol", None, 500).await;
    assert_eq!(got.len(), 1);
    assert_eq!(
        carol.open(&kh("bob"), &got[0]).as_deref(),
        Some(&b"for R1 under key n"[..])
    );
    // then its retention runs out, and the private half goes with it
    b.set(T0 + 2 * DAY + RETENTION_SECS);
    assert_eq!(b.live.node.expire(), 1);
    assert!(
        !b.live.view.lock().unwrap().node_keys.held(&r1).contains(&n),
        "deleted with the last message sealed to it"
    );
    let again = carol.seal(&first1, b"late, under key n", 1);
    assert_eq!(b.submit(&r1, &again, 2).await, SUBMISSION_REFUSED);
    assert_eq!(b.live.node.queued(&r1), 0, "nothing queued by the refusal");
}

// acceptance: SUB-16
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn the_newest_key_or_the_one_before_within_its_grace_and_no_other() {
    let keys = NodeKeyConfig::default();
    let g = keys.grace_secs;
    assert_eq!(g, 30, "thirty seconds as shipped");
    let b = Bench::start(T0, None, keys, |_| {});
    let r = kh("carol");
    let carol = Recipient::of("carol");
    // key n - 1, with a message sealed to it still held
    let older = b.node_key(&r).await;
    let held_msg = carol.seal(&older, b"sealed to n-1", 0);
    assert_eq!(b.submit(&r, &held_msg, 1).await, SUBMISSION_ACCEPTED);
    // key n, then the rotation to n + 1 at t
    b.set(T0 + DAY);
    let n = b.node_key(&r).await;
    let t = T0 + 2 * DAY;
    b.set(t);
    let next = b.node_key(&r).await;
    assert_eq!(
        (node_key(&older).fields.n + 1, node_key(&n).fields.n + 1),
        (node_key(&n).fields.n, node_key(&next).fields.n)
    );
    // before t + g: n accepted, n - 1 refused
    b.set(t + g - 1);
    let to_older = carol.seal(&older, b"n-1 within the grace", 2);
    let to_n = carol.seal(&n, b"n within the grace", 3);
    assert_eq!(b.submit(&r, &to_older, 2).await, SUBMISSION_REFUSED);
    assert_eq!(b.submit(&r, &to_n, 3).await, SUBMISSION_ACCEPTED);
    // after t + g: n refused, n + 1 accepted
    b.set(t + g);
    let late_n = carol.seal(&n, b"n after the grace", 4);
    let to_next = carol.seal(&next, b"n+1 after the grace", 5);
    assert_eq!(b.submit(&r, &late_n, 4).await, SUBMISSION_REFUSED);
    assert_eq!(b.submit(&r, &to_next, 5).await, SUBMISSION_ACCEPTED);
    // a refused submission queues nothing
    let held: Vec<Vec<u8>> = b
        .live
        .node
        .queue_records(&r)
        .into_iter()
        .map(|q| q.sealed)
        .collect();
    assert_eq!(held, vec![held_msg, to_n, to_next]);
    // and each accepted one is handed to R's device with the half that
    // opens it
    let (queued, got) = b.collect("carol", None, 500).await;
    assert_eq!(queued, 3);
    let mut opened: Vec<Vec<u8>> = got
        .iter()
        .map(|d| carol.open(&kh("bob"), d).expect("opens"))
        .collect();
    opened.sort();
    let mut expected = vec![
        b"sealed to n-1".to_vec(),
        b"n within the grace".to_vec(),
        b"n+1 after the grace".to_vec(),
    ];
    expected.sort();
    assert_eq!(opened, expected);
}

// acceptance: SUB-17
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_delivery_is_a_position_the_seal_unchanged_and_the_nodes_half_naming_no_sender() {
    use rhtn_codec::cbor::{Item, as_uint, parse_all};
    let b = Bench::start(T0, None, NodeKeyConfig::default(), |_| {});
    let r = kh("carol");
    let carol = Recipient::of("carol");
    let key = b.node_key(&r).await;
    // S submits two sealed messages for R, each under a key minted for it;
    // the sender's keyhash is inside the seal, as a SealedBody carries it
    let body = |i: u8| [&kh("alice")[..], &[i; 8]].concat();
    let seals = [carol.seal(&key, &body(1), 1), carol.seal(&key, &body(2), 2)];
    let mut presented = Vec::new();
    for (i, s) in seals.iter().enumerate() {
        let (c, k) = b.minted(0x50 + i as u8).await;
        assert_eq!(submit(&c, &r, s, [i as u8; 16]).await, SUBMISSION_ACCEPTED);
        presented.push(k);
    }
    // a device of R attaches with no field 5
    let (_, got) = b.collect("carol", None, 500).await;
    assert_eq!(got.len(), 2);
    let mut parts = Vec::new();
    for d in &got {
        // three elements: a position, the seal, 32 bytes
        let item = parse_all(d).expect("CBOR");
        let Item::Array(a) = &item else {
            panic!("an array")
        };
        assert_eq!(a.len(), 3, "three elements");
        let position = as_uint(&a[0]).expect("a position");
        let Item::Bytes(half) = &a[2] else {
            panic!("the half is a byte string")
        };
        assert_eq!(half.len(), 32);
        let rel = unrelayed(d).unwrap();
        assert_eq!(rel.position, position);
        parts.push((position, rel.sealed.clone(), *rel.ss_n));
    }
    parts.sort_by_key(|p| p.0);
    assert!(parts[0].0 < parts[1].0, "greater for the later arrival");
    assert_eq!(parts[0].1, seals[0], "byte for byte as submitted");
    assert_eq!(parts[1].1, seals[1]);
    // each half opens its own message and not the other
    assert_eq!(
        carol.open_with(&kh("bob"), &parts[0].1, &parts[0].2),
        Some(body(1))
    );
    assert_eq!(
        carol.open_with(&kh("bob"), &parts[1].1, &parts[1].2),
        Some(body(2))
    );
    assert_eq!(carol.open_with(&kh("bob"), &parts[0].1, &parts[1].2), None);
    assert_eq!(carol.open_with(&kh("bob"), &parts[1].1, &parts[0].2), None);
    // nothing in either delivery names S or the key S submitted under
    let alice = id("alice");
    for d in &got {
        assert!(!mentions(d, &kh("alice")), "S's keyhash");
        assert!(!mentions(d, &alice.public.ed.to_bytes()), "S's own key");
        for k in &presented {
            assert!(!mentions(d, k), "the key S submitted under");
        }
    }
}

// acceptance: DMN-04
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_node_keys_private_half_leaves_the_disk_with_its_last_message() {
    let dir = temp("dmn04");
    // the rotation a test parameter, long enough that the retention runs
    // out before the next one
    let keys = || NodeKeyConfig {
        rotate_secs: 40 * DAY,
        ..Default::default()
    };
    let b = Bench::start(T0, Some(&dir), keys(), |_| {});
    let r = kh("carol");
    let carol = Recipient::of("carol");
    let first = b.node_key(&r).await;
    let n = node_key(&first).fields.n;
    let sealed = carol.seal(&first, b"the one message under key n", 0);
    assert_eq!(b.submit(&r, &sealed, 1).await, SUBMISSION_ACCEPTED);
    // the private half as the node keeps it, to look for afterwards
    let key_file = dir
        .join("node-keys")
        .join(r.iter().map(|x| format!("{x:02x}")).collect::<String>())
        .join(n.to_string());
    let seed = std::fs::read(&key_file).expect("kept on disk")[..32].to_vec();
    // rotated past
    b.set(T0 + 40 * DAY);
    let newer = b.node_key(&r).await;
    assert_ne!(node_key(&newer).fields.n, n);
    // the message is delivered and its retention runs out: removed, and
    // nothing written back after
    let (_, got) = b.collect("carol", None, 500).await;
    assert_eq!(got.len(), 1);
    b.set(T0 + 40 * DAY + RETENTION_SECS);
    assert_eq!(b.live.node.expire(), 1);
    // the node's memory is lost, and a node starts from the bytes on disk
    let clock = b.kill();
    let again = Bench::start_on(clock, Some(&dir), keys(), |_| {});
    let disk = bytes_under(&dir);
    assert!(!key_file.exists(), "the key's file is gone");
    assert!(
        !mentions(&disk, &seed),
        "its private half is nowhere on disk"
    );
    assert!(
        !disk.windows(sealed.len()).any(|w| w == sealed.as_slice()),
        "nor the message"
    );
    let late = carol.seal(&first, b"sealed to the deleted key", 2);
    assert_eq!(again.submit(&r, &late, 2).await, SUBMISSION_REFUSED);
    // the newer key is still held and served
    assert_eq!(
        again.live.view.lock().unwrap().node_keys.held(&r),
        vec![node_key(&newer).fields.n]
    );
    assert_eq!(again.node_key(&r).await, newer);
    let _ = std::fs::remove_dir_all(&dir);
}

// functional: MAIL-003
/// **A key nobody rotated is not accepted past its day** [review,
/// 2026-11-25]: a rotation happens when somebody asks after the interval,
/// so a recipient nobody writes to keeps one key, and a message sealed to
/// it must not be put into the queue months later (design §14.2.4.3).
#[test]
fn the_newest_key_is_accepted_for_its_day_and_grace_and_no_longer() {
    use rhtn_node::node_keys::NodeKeyService;
    let cfg = NodeKeyConfig::default();
    let grace = cfg.grace_secs;
    let mut service = NodeKeyService::new(cfg);
    let record = service.newest(&id("bob"), &kh("carol"), T0);
    let n = node_key(&record).fields.n;
    assert!(service.accepts(&kh("carol"), n, T0));
    assert!(service.accepts(&kh("carol"), n, T0 + DAY + grace - 1));
    assert!(!service.accepts(&kh("carol"), n, T0 + DAY + grace));
    service.prune(&kh("carol"), &|_| false, T0 + 100 * DAY);
    assert!(
        !service.accepts(&kh("carol"), n, T0 + 100 * DAY),
        "an idle key is not accepted 100 days later"
    );
    // and a sender refused asks, which makes the next
    let next = node_key(&service.newest(&id("bob"), &kh("carol"), T0 + 100 * DAY));
    assert_eq!(next.fields.n, n + 1);
    assert!(service.accepts(&kh("carol"), n + 1, T0 + 100 * DAY));
}

// functional: MAIL-003
/// **A key that could not be written is not published** [review,
/// 2026-11-25]: a service that keeps its keys on disk and cannot make
/// its directory does not start, and one whose write fails later refuses
/// the asker and holds nothing, so no message is ever queued under a key
/// a restart would not have.
#[test]
fn a_key_that_cannot_be_kept_is_neither_published_nor_accepted() {
    use rhtn_archive::node_key::{FAIL_REFUSED, NodeKeyReply, NodeKeyRequest};
    use rhtn_node::node_keys::NodeKeyService;
    // where the keys would be kept is a file: nothing can be kept there
    let root = temp("unwritable-root");
    std::fs::create_dir_all(&root).unwrap();
    std::fs::write(root.join("node-keys"), b"not a directory").unwrap();
    assert!(
        NodeKeyService::at(&root, NodeKeyConfig::default()).is_err(),
        "a service that cannot keep its keys does not start"
    );
    std::fs::remove_dir_all(&root).unwrap();

    // the directory is there, and one recipient's place in it is a file
    let root = temp("unwritable-recipient");
    std::fs::create_dir_all(root.join("node-keys")).unwrap();
    let hex: String = kh("carol").iter().map(|b| format!("{b:02x}")).collect();
    std::fs::write(root.join("node-keys").join(&hex), b"in the way").unwrap();
    let mut service = NodeKeyService::at(&root, NodeKeyConfig::default()).unwrap();
    assert!(service.try_newest(&id("bob"), &kh("carol"), T0).is_err());
    assert!(service.held(&kh("carol")).is_empty(), "nothing is held");
    assert!(!service.accepts(&kh("carol"), 1, T0));
    // an asker is refused, and another recipient is served as ever
    let ask = NodeKeyRequest::One {
        recipient: kh("carol"),
        nonce: [7; 16],
    };
    let reply = service
        .answer(&id("bob"), &|_| true, &ask.encode(), T0)
        .unwrap();
    assert_eq!(
        NodeKeyReply::decode(&reply).unwrap().outcome,
        Err(FAIL_REFUSED)
    );
    let kept = node_key(&service.try_newest(&id("bob"), &kh("w1"), T0).unwrap());
    drop(service);
    let reopened = NodeKeyService::at(&root, NodeKeyConfig::default()).unwrap();
    assert_eq!(reopened.held(&kh("w1")), vec![kept.fields.n]);
    std::fs::remove_dir_all(&root).unwrap();
}

// functional: SES-019
/// **A device's earlier session ending does not detach the one it holds
/// now** [review, 2026-11-25]: a client that attaches again before its
/// earlier session is seen to end is still attached when that one closes.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn an_earlier_session_ending_does_not_detach_the_one_that_replaced_it() {
    use rhtn_transport::session::*;
    let b = Bench::start(T0, None, NodeKeyConfig::default(), |_| {});
    let cfg = ClientConfig {
        me: rhtn_transport::tls::Party::of(std::sync::Arc::new(id("carol"))),
        pins: b.pins.clone(),
        bind: Default::default(),
        capabilities: Default::default(),
        attestation: None,
        filter: None,
        sibling_cache: Default::default(),
        addresses: Default::default(),
        cursors: Default::default(),
        tls: Default::default(),
        connect_timeout: std::time::Duration::from_secs(3),
        on_reachability: None,
        log: Log::default(),
    };
    let ep = rhtn_transport::tls::client_endpoint("127.0.0.1:0".parse().unwrap()).unwrap();
    let AttachOutcome::Attached(first) = attach(&cfg, &ep, kh("bob"), b.live.addr, false).await
    else {
        panic!("first attach");
    };
    let AttachOutcome::Attached(second) = attach(&cfg, &ep, kh("bob"), b.live.addr, false).await
    else {
        panic!("second attach");
    };
    first.conn.close(0u32.into(), b"old connection closed");
    // the node reads the old stream's end; this is no heartbeat timeout
    tokio::time::sleep(std::time::Duration::from_millis(500)).await;
    assert!(
        second.conn.close_reason().is_none(),
        "the later one is live"
    );
    assert!(
        b.live.view.lock().unwrap().attached.contains(&kh("carol")),
        "and its client is still attached"
    );
    // and when the last goes, so does the client
    second.conn.close(0u32.into(), b"gone");
    tokio::time::sleep(std::time::Duration::from_millis(500)).await;
    assert!(!b.live.view.lock().unwrap().attached.contains(&kh("carol")));
}
