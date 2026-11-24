//! The kernel's own boards (`light-client-requirements.md` §4.5): a post
//! written and handed to the node, a delivery taken, a thread's reach
//! carried, what was missed asked for, and the pools kept.

mod common;

use common::harness::setup;
use common::kh;
use rhtn_archive::board::{BoardPost, BoardReply, BoardRequest, Post};
use rhtn_client::board::Took;
use rhtn_client::ceremony::Msg;

const PARTIES: [&str; 6] = ["alice", "bob", "carol", "w1", "w2", "w3"];

/// The one message a kernel call made, as a `BoardPost` frame to `node`.
fn posted(msgs: &[Msg], node: &str) -> BoardPost {
    let [Msg::BoardPost { node: to, bytes }] = msgs else {
        panic!("one post, handed to the node: {msgs:?}");
    };
    assert_eq!(*to, kh(node));
    BoardPost::decode(bytes).expect("a frame")
}

/// The frame a node hands a member: the post, the node's number, and the
/// upward count the post was carried with from its writer.
fn delivery(post: &[u8], number: u64, carried: u64) -> Vec<u8> {
    BoardPost {
        up: 0,
        down: 0,
        post: post.to_vec(),
        number: Some(number),
        carried: Some(carried),
    }
    .encode()
}

#[test]
fn a_post_is_written_signed_and_handed_to_the_node_serving_the_subnet() {
    let mut s = setup(&PARTIES);
    s.adopted("bob", "alice", "w1", 1);
    let subnet = s.client("alice").subnets()[0];
    // not attached, there is no node to hand it to
    let unattached =
        s.client("bob")
            .board_post(None, &subnet, None, None, Some("Hello"), "the words");
    assert!(unattached.is_err());
    // attached to the node serving it there, which is its patron's
    s.client("bob").attach(kh("alice"), &[]);
    let msgs = s
        .client("bob")
        .board_post(None, &subnet, None, None, Some("Hello"), "the words")
        .expect("posts");
    let frame = posted(&msgs, "alice");
    assert_eq!(
        (frame.up, frame.down, frame.number),
        (2, 2, None),
        "the default reach"
    );
    let post = Post::parse(&frame.post).unwrap();
    assert_eq!(post.verify(), Ok(()));
    assert_eq!(post.writer, kh("bob"));
    assert_eq!(
        Some(post.position.clone()),
        s.client("bob").position_in(&subnet)
    );
    assert_eq!(
        (post.title.as_deref(), post.text.as_str()),
        (Some("Hello"), "the words")
    );
    // the writer holds it at once, read, and the kernel says the board moved
    let bob = s.client("bob");
    assert_eq!(bob.board_threads(&subnet).len(), 1);
    assert_eq!(bob.board_unread(&subnet), 0);
    // a thread begins with a title, and a post has words
    assert!(
        bob.board_post(None, &subnet, None, None, None, "no title")
            .is_err()
    );
    assert!(
        bob.board_post(None, &subnet, None, None, Some("t"), "")
            .is_err()
    );
    assert!(
        bob.board_post(None, &[9; 32], None, None, Some("t"), "x")
            .is_err()
    );
}

#[test]
fn a_delivery_is_taken_and_a_reply_carries_its_threads_reach_from_where_it_is_written() {
    let mut s = setup(&PARTIES);
    s.adopted("bob", "alice", "w1", 1);
    let subnet = s.client("alice").subnets()[0];
    s.client("bob").attach(kh("alice"), &[]);
    // the root's own node carries its board, under its own key
    s.client("alice").attach(kh("alice"), &[]);
    let msgs = s
        .client("bob")
        .board_post(
            None,
            &subnet,
            Some((3, 1)),
            None,
            Some("Hello"),
            "the words",
        )
        .unwrap();
    let frame = posted(&msgs, "alice");
    let thread = Post::hash(&frame.post);
    // the node delivers it to alice, numbered
    let alice = s.client("alice");
    assert_eq!(
        alice.board_arrived(kh("alice"), &delivery(&frame.post, 7, 3)),
        Some(Took::New)
    );
    assert_eq!(alice.board_unread(&subnet), 1);
    let threads = alice.board_threads(&subnet);
    assert_eq!((threads.len(), threads[0].id), (1, thread));
    assert_eq!(threads[0].title.as_deref(), Some("Hello"));
    // bob's `+3/-1` read one level above him
    let read = alice.board_thread(&subnet, &thread);
    assert_eq!(read[0].1, (2, 2));
    // a second copy is the same post, and a copy between nodes, which
    // carries no number, is not a client's
    assert_eq!(
        alice.board_arrived(kh("alice"), &delivery(&frame.post, 7, 3)),
        Some(Took::Held)
    );
    let between = BoardPost {
        number: None,
        ..BoardPost::decode(&delivery(b"x", 1, 0)).unwrap()
    };
    assert_eq!(alice.board_arrived(kh("alice"), &between.encode()), None);
    // nor is anything from a node that carries no board of hers
    assert_eq!(
        alice.board_arrived(kh("w3"), &delivery(&frame.post, 1, 3)),
        None
    );
    // shown, it is read
    alice.board_read(&subnet, &thread);
    assert_eq!(alice.board_unread(&subnet), 0);
    // her reply is offered no reach: it carries the thread's, from where
    // she stands
    let msgs = alice
        .board_post(None, &subnet, Some((9, 9)), Some(thread), None, "a reply")
        .unwrap();
    let reply = posted(&msgs, "alice");
    assert_eq!((reply.up, reply.down), (2, 2));
    assert_eq!(Post::parse(&reply.post).unwrap().thread, Some(thread));
    // and a reply to a thread nothing is held of has no reach to carry
    assert!(
        alice
            .board_post(None, &subnet, None, Some([5; 32]), None, "to nothing")
            .is_err()
    );
}

// functional_tests.md: BRD-016
#[test]
fn a_post_asked_further_up_than_its_writer_earned_reads_as_it_was_carried() {
    let mut s = setup(&PARTIES);
    s.adopted("bob", "alice", "w1", 1);
    let subnet = s.client("alice").subnets()[0];
    s.client("bob").attach(kh("alice"), &[]);
    s.client("alice").attach(kh("alice"), &[]);
    // bob, with nobody beneath him, asks `+4/-2`
    let msgs = s
        .client("bob")
        .board_post(None, &subnet, Some((4, 2)), None, Some("Far"), "the words")
        .unwrap();
    let frame = posted(&msgs, "alice");
    assert_eq!((frame.up, frame.down, frame.carried), (4, 2, None));
    let thread = Post::hash(&frame.post);
    // until his node hands it back, his own copy reads as he wrote it
    let bob = s.client("bob");
    assert_eq!(bob.board_thread(&subnet, &thread)[0].1, (4, 2));
    // the node carried it as `+2`, and says so to everyone it delivers to:
    // his own copy now reads `+2/-2`, and the board is said to have moved
    let _ = bob.take_changed();
    assert_eq!(
        bob.board_arrived(kh("alice"), &delivery(&frame.post, 1, 2)),
        Some(Took::Held)
    );
    assert_eq!(bob.board_thread(&subnet, &thread)[0].1, (2, 2));
    assert!(bob.take_changed().contains(&rhtn_model::Noun::Board));
    // one level above him it reads `+1/-3`, not the `+3/-3` he asked for
    let alice = s.client("alice");
    alice.board_arrived(kh("alice"), &delivery(&frame.post, 1, 2));
    assert_eq!(alice.board_thread(&subnet, &thread)[0].1, (1, 3));
    // and a reply takes the band the thread was carried in
    let msgs = alice
        .board_post(None, &subnet, None, Some(thread), None, "a reply")
        .unwrap();
    let reply = posted(&msgs, "alice");
    assert_eq!((reply.up, reply.down), (1, 3));
    // a node that says more than was written is not believed past it
    let msgs = s
        .client("bob")
        .board_post(None, &subnet, Some((1, 0)), None, Some("Near"), "x")
        .unwrap();
    let near = posted(&msgs, "alice");
    let alice = s.client("alice");
    alice.board_arrived(kh("alice"), &delivery(&near.post, 2, 9));
    assert_eq!(
        alice.board_thread(&subnet, &Post::hash(&near.post))[0].1,
        (0, 1)
    );
    // what the node said is kept between runs
    let kept = alice.board_part();
    alice.restore_board(&kept).expect("restores");
    assert_eq!(alice.board_thread(&subnet, &thread)[0].1, (1, 3));
}

#[test]
fn what_was_missed_is_asked_for_from_the_last_number_and_again_while_more_is_held() {
    let mut s = setup(&PARTIES);
    s.adopted("bob", "alice", "w1", 1);
    let subnet = s.client("alice").subnets()[0];
    s.client("alice").attach(kh("alice"), &[]);
    let written: Vec<Vec<u8>> = ["one", "two", "three"]
        .iter()
        .map(|t| {
            let msgs = s
                .client("alice")
                .board_post(None, &subnet, None, None, Some("A thread"), t)
                .unwrap();
            posted(&msgs, "alice").post
        })
        .collect();
    s.client("bob").attach(kh("alice"), &[]);
    let bob = s.client("bob");
    let asked = |msgs: &[Msg]| -> BoardRequest {
        let [Msg::BoardAsk { node, bytes }] = msgs else {
            panic!("one asking: {msgs:?}");
        };
        assert_eq!(*node, kh("alice"));
        BoardRequest::decode(bytes).expect("a request")
    };
    // a first asking holds no number
    let first = asked(&bob.board_ask(kh("alice")));
    assert_eq!(first.after, None);
    // a reply to something else is not taken
    let forged = BoardReply {
        nonce: [0; 16],
        posts: vec![(1, written[0].clone(), 2)],
        through: 1,
        more: false,
    };
    assert!(bob.take_board_reply(kh("alice"), &forged.encode()).is_err());
    assert!(bob.board_threads(&subnet).is_empty());
    // the node says more is held: the next asking goes from where it read
    let reply = BoardReply {
        nonce: first.nonce,
        posts: vec![(4, written[0].clone(), 2), (6, written[1].clone(), 2)],
        through: 6,
        more: true,
    };
    let next = bob.take_board_reply(kh("alice"), &reply.encode()).unwrap();
    let second = asked(&next);
    assert_eq!(second.after, Some(6));
    assert_eq!(bob.board_unread(&subnet), 2);
    // a delivery meanwhile is taken and does not move the number
    bob.board_arrived(kh("alice"), &delivery(&written[2], 40, 2));
    let reply = BoardReply {
        nonce: second.nonce,
        posts: vec![],
        through: 9,
        more: false,
    };
    assert!(
        bob.take_board_reply(kh("alice"), &reply.encode())
            .unwrap()
            .is_empty()
    );
    assert_eq!(asked(&bob.board_ask(kh("alice"))).after, Some(9));
    assert_eq!(bob.board_unread(&subnet), 3);
    // a node that carries no board of his is asked nothing
    assert!(bob.board_ask(kh("w3")).is_empty());
}

#[test]
fn the_boards_are_kept_between_runs_apart_from_what_a_start_needs() {
    let mut s = setup(&PARTIES);
    s.adopted("bob", "alice", "w1", 1);
    let subnet = s.client("alice").subnets()[0];
    s.client("alice").attach(kh("alice"), &[]);
    s.client("alice")
        .board_post(None, &subnet, None, None, Some("Kept"), "words")
        .unwrap();
    let kept = s.client("alice").board_part();
    let before = s.client("alice").board_threads(&subnet);
    // another client's kernel takes the relation as its own boards
    let carol = s.client("carol");
    assert_eq!(carol.restore_board(&kept), Some(1));
    assert_eq!(carol.board_threads(&subnet), before);
    // and bytes that do not read leave them as they were
    assert_eq!(carol.restore_board(b"\x00"), None);
    assert_eq!(carol.board_threads(&subnet), before);
    // the relations a start needs do not name it
    assert!(!rhtn_client::ceremony::RELATIONS.contains(&rhtn_client::board::RELATION_BOARD));
}
