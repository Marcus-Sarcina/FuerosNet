//! A subnet's board as a client keeps it (`light-client-requirements.md`
//! §4.5; functional_tests.md BRD-012, BRD-013).

use rhtn_archive::board::{Draft, Post, Writer};
use rhtn_archive::tx::{Locator, Seqno};
use rhtn_client::board::{Pool, Took, reach_from};
use rhtn_crypto::identity::testkit::test_identity;

const SUBNET: [u8; 32] = [0xdb; 32];

fn at(nibbles: u64) -> Locator {
    Locator {
        anchor: test_identity("alice").public.keyhash,
        path: vec![0x11; nibbles.div_ceil(2) as usize],
        nibbles,
        seqno: Seqno {
            series: 1,
            counter: 0,
        },
        subnet: Some(SUBNET),
    }
}

/// A post by `who` at `when`: beginning a thread titled `title`, or
/// continuing `thread`.
fn post(who: &str, when: u64, thread: Option<[u8; 32]>, text: &str) -> Vec<u8> {
    let id = test_identity(who);
    let km = id.public.key_material();
    let here = at(2);
    Post::build(
        &Writer::own(&id, &km),
        &Draft {
            position: &here,
            up: 2,
            down: 2,
            written_at: when,
            thread,
            title: thread.is_none().then_some("A title"),
            text,
            alias: None,
        },
    )
    .expect("builds")
}

fn me() -> [u8; 32] {
    test_identity("bob").public.keyhash
}

// functional: BRD-012
#[test]
fn a_post_is_kept_once_and_only_where_its_signature_makes_it_its_writers() {
    let mut pool = Pool::new(1 << 20);
    let a = post("carol", 100, None, "hello");
    assert_eq!(pool.take(&SUBNET, &me(), &a, 200, None), Took::New);
    // one hash is one post, however many times it arrives
    assert_eq!(pool.take(&SUBNET, &me(), &a, 300, None), Took::Held);
    assert_eq!(pool.len(), 1);
    // bytes that are no post, and a post changed after it was signed
    assert_eq!(
        pool.take(&SUBNET, &me(), b"not a post", 200, None),
        Took::Unread
    );
    let mut changed = a.clone();
    let i = changed.windows(5).position(|w| w == b"hello").unwrap();
    changed[i] ^= 1;
    assert_eq!(
        pool.take(&SUBNET, &me(), &changed, 200, None),
        Took::Unsigned
    );
    // and nothing on a board comes from another subnet
    assert_eq!(
        pool.take(
            &[0xee; 32],
            &me(),
            &post("carol", 101, None, "x"),
            200,
            None
        ),
        Took::Foreign
    );
    assert_eq!(pool.len(), 1);
}

#[test]
fn threads_are_listed_newest_first_and_a_reply_sits_in_its_thread() {
    let mut pool = Pool::new(1 << 20);
    let first = post("carol", 100, None, "the first thread");
    let second = post("w1", 200, None, "the second thread");
    let id = Post::hash(&first);
    pool.take(&SUBNET, &me(), &first, 1_000, None);
    pool.take(&SUBNET, &me(), &second, 1_000, None);
    assert_eq!(pool.threads()[0].id, Post::hash(&second), "newest first");
    // a reply moves its thread to the top and is listed under it
    pool.take(
        &SUBNET,
        &me(),
        &post("w1", 300, Some(id), "a reply"),
        1_000,
        None,
    );
    let threads = pool.threads();
    assert_eq!(threads[0].id, id);
    assert_eq!((threads[0].posts, threads[0].latest), (2, 300));
    assert_eq!(threads[0].title.as_deref(), Some("A title"));
    assert_eq!(
        threads[0].began_by,
        Some(test_identity("carol").public.keyhash)
    );
    let texts: Vec<&str> = pool
        .thread(&id)
        .iter()
        .map(|k| k.post.text.as_str())
        .collect();
    assert_eq!(texts, ["the first thread", "a reply"]);
    // a reply whose thread's first post is not held is still a thread,
    // with nothing to title it by
    pool.take(
        &SUBNET,
        &me(),
        &post("w1", 400, Some([7; 32]), "orphan"),
        1_000,
        None,
    );
    let t = &pool.threads()[0];
    assert_eq!((t.id, t.title.clone(), t.began_by), ([7; 32], None, None));
}

#[test]
fn what_is_unread_is_the_clients_own_count() {
    let mut pool = Pool::new(1 << 20);
    let first = post("carol", 100, None, "one");
    let id = Post::hash(&first);
    pool.take(&SUBNET, &me(), &first, 1_000, None);
    pool.take(
        &SUBNET,
        &me(),
        &post("w1", 200, Some(id), "two"),
        1_000,
        None,
    );
    // what this client wrote is read already
    pool.take(
        &SUBNET,
        &me(),
        &post("bob", 300, Some(id), "mine"),
        1_000,
        None,
    );
    pool.take(
        &SUBNET,
        &me(),
        &post("w1", 400, None, "elsewhere"),
        1_000,
        None,
    );
    assert_eq!(pool.unread(), 3);
    assert_eq!(pool.threads().iter().map(|t| t.unread).sum::<usize>(), 3);
    assert_eq!(pool.mark_read(&id), 2);
    assert_eq!(pool.mark_read(&id), 0);
    assert_eq!(pool.unread(), 1);
}

// functional: BRD-013
#[test]
fn the_oldest_go_first_by_the_posts_own_date_read_no_later_than_its_arrival() {
    let a = post("carol", 100, None, "oldest");
    let b = post("carol", 200, None, "middle");
    let c = post("carol", 300, None, "newest");
    // dated a century ahead: read as arriving when it did, at 150
    let ahead = post("w1", 5_000_000_000, None, "dated ahead");
    let room = a.len() + b.len() + c.len() + 64;
    let mut pool = Pool::new(room);
    // arrival order is not the order they go in
    for p in [&c, &a, &b] {
        assert_eq!(pool.take(&SUBNET, &me(), p, 1_000, None), Took::New);
    }
    assert_eq!(pool.take(&SUBNET, &me(), &ahead, 150, None), Took::New);
    assert_eq!(pool.get(&Post::hash(&ahead)).unwrap().dated, 150);
    // the oldest by date went, and the one dated ahead is next to go
    assert!(pool.get(&Post::hash(&a)).is_none());
    assert_eq!(pool.len(), 3);
    let d = post("carol", 400, None, "newer still");
    pool.take(&SUBNET, &me(), &d, 1_000, None);
    assert!(
        pool.get(&Post::hash(&ahead)).is_none(),
        "it outlived nothing"
    );
    assert!(pool.get(&Post::hash(&b)).is_some() && pool.get(&Post::hash(&d)).is_some());
}

#[test]
fn the_cursor_is_per_node_and_moves_by_the_asking_until_caught_up() {
    let (n1, n2) = ([1; 32], [2; 32]);
    let mut pool = Pool::new(1 << 20);
    assert_eq!(pool.cursor(&n1), None);
    // a delivery before this client has caught up does not move it: the
    // asking in progress would skip what lies between
    pool.delivered(&n1, 900);
    assert_eq!(pool.cursor(&n1), None);
    pool.caught_up(&n1, 500, true);
    pool.delivered(&n1, 901);
    assert_eq!(pool.cursor(&n1), Some(500));
    // caught up, a delivery's number is the cursor
    pool.caught_up(&n1, 902, false);
    pool.delivered(&n1, 905);
    assert_eq!(pool.cursor(&n1), Some(905));
    pool.delivered(&n1, 903);
    assert_eq!(pool.cursor(&n1), Some(905), "and it does not go back");
    // a number is a node's own, and means nothing at another
    assert_eq!(pool.cursor(&n2), None);
    // a session ended is one to catch up on again
    pool.detached(&n1);
    pool.delivered(&n1, 999);
    assert_eq!(pool.cursor(&n1), Some(905));
}

#[test]
fn the_pool_comes_back_as_it_was_kept() {
    let mut pool = Pool::new(1 << 20);
    let first = post("carol", 100, None, "one");
    pool.take(&SUBNET, &me(), &first, 1_000, None);
    pool.take(&SUBNET, &me(), &post("w1", 200, None, "two"), 150, None);
    pool.mark_read(&Post::hash(&first));
    pool.caught_up(&[1; 32], 77, true);
    let back = Pool::decode(1 << 20, &pool.encode()).expect("reads");
    assert_eq!(back, pool);
    assert_eq!((back.unread(), back.cursor(&[1; 32])), (1, Some(77)));
    // the session it was current on is not kept: the next run asks again
    pool.caught_up(&[1; 32], 77, false);
    let mut back = Pool::decode(1 << 20, &pool.encode()).unwrap();
    back.delivered(&[1; 32], 80);
    assert_eq!(back.cursor(&[1; 32]), Some(77));
    assert!(Pool::decode(1 << 20, b"\x00").is_none());
    assert!(Pool::decode(1 << 20, &[]).is_none());
}

#[test]
fn a_reach_is_read_from_where_the_reader_stands() {
    let writer = at(4);
    // the writer's own reading, and one level above and below
    assert_eq!(reach_from(3, 3, &writer, &at(4)), (3, 3));
    assert_eq!(reach_from(3, 3, &writer, &at(3)), (2, 4));
    assert_eq!(reach_from(3, 3, &writer, &at(5)), (4, 2));
    // the top of the band reads nothing above it, and the bottom nothing below
    assert_eq!(reach_from(3, 3, &writer, &at(1)), (0, 6));
    assert_eq!(reach_from(3, 3, &writer, &at(7)), (6, 0));
    // under another anchor there is nothing to measure by
    let mut elsewhere = at(3);
    elsewhere.anchor = [9; 32];
    assert_eq!(reach_from(3, 3, &writer, &elsewhere), (3, 3));
}
