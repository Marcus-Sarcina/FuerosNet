//! The node's mailbox on disk (QUE-04, QUE-17): what a restart leaves
//! behind, and what it finds.

mod common;

use common::*;
use rhtn_archive::submission::SUBMISSION_ACCEPTED;
use rhtn_node::node_keys::NodeKeyConfig;
use rhtn_node::queue::DirStore;
use rhtn_transport::queue::{QueueStore, Queued};

// acceptance: QUE-04
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn no_crash_recovery_copy_outlives_a_removal() {
    const T0: u64 = 1_900_000_000;
    const DAY: u64 = 86_400;
    // the retention a test parameter (thirty days as shipped)
    const RETENTION: u64 = 3 * DAY;
    let dir = std::env::temp_dir().join(format!("rhtn-queue-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let b = Bench::start(T0, Some(&dir), NodeKeyConfig::default(), |cfg| {
        cfg.retention_secs = RETENTION;
    });
    let carol = Recipient::of("carol");
    let key = b.node_key(&kh("carol")).await;
    let sent: Vec<Vec<u8>> = (0..3u8)
        .map(|i| carol.seal(&key, format!("q4:{i}").as_bytes(), i))
        .collect();
    for (i, m) in sent.iter().enumerate() {
        assert_eq!(
            b.submit(&kh("carol"), m, i as u8).await,
            SUBMISSION_ACCEPTED
        );
    }
    let store = DirStore::new(dir.join("queue"));
    assert_eq!(store.all_files().len(), 3, "persisted while waiting");
    // the key they were sealed to is rotated past, so it is held for them
    // alone, and its private half is read off the disk to look for later
    b.set(T0 + DAY);
    b.node_key(&kh("carol")).await;
    let key_dir = dir.join("node-keys").join(
        kh("carol")
            .iter()
            .map(|x| format!("{x:02x}"))
            .collect::<String>(),
    );
    let seed = std::fs::read(key_dir.join("1")).expect("key 1 on disk")[..32].to_vec();
    // delivered, then their retention runs out and the node removes them
    let (_, got) = b.collect("carol", None, 500).await;
    assert_eq!(got.len(), 3);
    b.set(T0 + DAY + RETENTION);
    assert_eq!(b.live.node.expire(), 3);
    assert!(store.all_files().is_empty());
    // the process dies without warning, and a node starts from whatever
    // the directory holds
    let clock = b.kill();
    let again = Bench::start_on(clock, Some(&dir), NodeKeyConfig::default(), |cfg| {
        cfg.retention_secs = RETENTION;
    });
    let (queued, got) = again.collect("carol", None, 500).await;
    assert_eq!(queued, 0);
    assert!(got.is_empty(), "no delivery follows");
    assert!(again.live.node.queue_records(&kh("carol")).is_empty());
    let disk = bytes_under(&dir);
    for m in &sent {
        assert!(
            !disk.windows(m.len()).any(|w| w == m.as_slice()),
            "none of the ciphertexts is present after the restart"
        );
    }
    assert!(!key_dir.join("1").exists());
    assert!(
        !mentions(&disk, &seed),
        "nor the private half of the key held only for them"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

fn item(store: &DirStore, recipient: [u8; 32], arrival: u64, sealed: &[u8]) -> Queued {
    Queued {
        sealed: sealed.to_vec(),
        recipient,
        n: 1,
        position: store.next_position(&recipient),
        arrival,
        delivered_at: None,
    }
}

// acceptance: QUE-17
#[test]
fn a_restart_in_the_arrival_second_overwrites_nothing_accepted() {
    let dir = std::env::temp_dir().join(format!("rhtn-queue-restart-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let first = DirStore::new(&dir);
    let one = item(&first, kh("carol"), 100, b"one");
    assert!(first.push(one));
    let two = item(&first, kh("carol"), 100, b"two");
    assert!(first.push(two));
    drop(first);
    // the node restarts within the same second and accepts a third
    let second = DirStore::new(&dir);
    let three = item(&second, kh("carol"), 100, b"three");
    assert_eq!(three.position, 3, "the sequence resumes past what is held");
    assert!(second.push(three));
    let held: Vec<Vec<u8>> = second
        .list(&kh("carol"))
        .into_iter()
        .map(|q| q.sealed)
        .collect();
    assert_eq!(
        held,
        vec![b"one".to_vec(), b"two".to_vec(), b"three".to_vec()],
        "all three, oldest first"
    );
    assert_eq!(second.all_files().len(), 3);
    let _ = std::fs::remove_dir_all(&dir);
}

// functional: MAIL-014
/// The recipient's directory outlives a removal: a push for the same
/// recipient may be between its file write and its directory flush when
/// what it just wrote is removed, and the flush of a directory that has
/// gone was reported as a refusal for a message the other side already
/// held [2026-10-04, the first field run].
#[test]
fn a_removal_leaves_the_recipients_directory_and_a_later_push_is_not_a_refusal() {
    let dir = std::env::temp_dir().join(format!("rhtn-queue-dir-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let store = DirStore::new(&dir);
    let first = item(&store, kh("alice"), 1_001, &[1; 40]);
    assert!(store.push(first.clone()));
    assert!(store.mark_delivered(&kh("alice"), first.position, 1_002));
    assert!(store.remove(&kh("alice"), first.position));
    let recipient_dir = std::fs::read_dir(&dir)
        .unwrap()
        .flatten()
        .find(|e| e.path().is_dir())
        .map(|e| e.path());
    assert!(recipient_dir.is_some(), "the directory stays once emptied");
    let second = item(&store, kh("alice"), 1_003, &[2; 40]);
    assert!(second.position > first.position, "never reused");
    assert!(store.push(second), "a push after a removal is accepted");
    assert_eq!(store.list(&kh("alice")).len(), 1);
    let _ = std::fs::remove_dir_all(&dir);
}
