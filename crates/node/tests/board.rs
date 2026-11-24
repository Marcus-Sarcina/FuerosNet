//! The subnet's board at a node (BRD): who a post reaches, how it is
//! carried on, what the node keeps, and what a member that was away is
//! handed (`wire-format.md` §10.3; design §14.4).

mod common;

use common::*;
use rhtn_archive::board::{BoardPost, BoardReply, BoardRequest, FRAME_BOARD_POST};
use rhtn_archive::topology::Table;
use rhtn_node::Keyhash;
use rhtn_node::board::{Discard, Taken};
use rhtn_node::view::NodeView;
use std::sync::Arc;

/// alice, the root, over bob (N) and w4, both running nodes. Under N:
/// carol, who runs a node, with the light member w3; and the light member
/// w1 with the light member w2 beneath them. Under w4, the light member
/// w5, who attaches to N in failover.
struct Scene {
    w: World,
    table: Table,
    n: NodeView,
    fab: Arc<Fabric>,
}

/// The slot `node` holds under its patron, as the table settled it.
fn slot(t: &Table, node: &str) -> u8 {
    t.bindings()
        .iter()
        .find(|b| b.node == kh(node))
        .and_then(|b| b.slot)
        .expect("bound")
}

/// A view for `name` at `path` under alice, its own slots filled from the
/// table and its downline settled.
fn node(name: &str, table: &Table, path: &[u8], t: u64) -> NodeView {
    let mut v = view(name, table.clone_for(kh(name)), "alice", path);
    let subs: Vec<(u8, Keyhash)> = table
        .bindings()
        .iter()
        .filter(|b| b.patron == kh(name))
        .filter_map(|b| b.slot.map(|s| (s, b.node)))
        .collect();
    for (s, k) in subs {
        v.set_slot(u64::from(s), Some(k), t);
    }
    v.sync_downline();
    v
}

fn scene() -> Scene {
    let mut w = World::new();
    let (a1, _) = w.adopt("bob", "alice", 1);
    let (a2, _) = w.adopt("w4", "alice", 2);
    let (a3, _) = w.adopt("carol", "bob", 3);
    let (a4, _) = w.adopt("w1", "bob", 4);
    let (a5, _) = w.adopt("w2", "w1", 5);
    let (a6, _) = w.adopt("w3", "carol", 6);
    let (a7, _) = w.adopt("w5", "w4", 7);
    let table = table_with(
        kh("bob"),
        &w,
        &[&a1, &a2, &a3, &a4, &a5, &a6, &a7],
        &["alice", "bob", "carol", "w4"],
    );
    let mut n = node("bob", &table, &[slot(&table, "bob")], w.clock);
    for member in ["bob", "w1", "w2", "carol"] {
        n.attached.insert(kh(member));
    }
    let fab = Fabric::with(&[kh("alice"), kh("bob"), kh("carol"), kh("w1"), kh("w2")]);
    Scene { w, table, n, fab }
}

/// A member's frame: the counts as written, and nothing of what the post
/// is carried with, which is the first node's to say.
fn frame(up: u64, down: u64, post: &[u8]) -> Vec<u8> {
    BoardPost {
        up,
        down,
        post: post.to_vec(),
        number: None,
        carried: None,
    }
    .encode()
}

/// A node's copy arriving, carrying the upward count the first node
/// carried the post with.
fn relay(up: u64, down: u64, carried: u64, post: &[u8]) -> Vec<u8> {
    copy(up, down, carried, post).encode()
}

/// The `BoardPost` frames sent to `to`, decoded.
fn got(fab: &Fabric, to: &str) -> Vec<BoardPost> {
    fab.to(&kh(to), FRAME_BOARD_POST)
        .iter()
        .map(|b| BoardPost::decode(b).expect("a board post"))
        .collect()
}

fn delivery(post: &[u8], number: u64, carried: u64) -> BoardPost {
    BoardPost {
        up: 0,
        down: 0,
        post: post.to_vec(),
        number: Some(number),
        carried: Some(carried),
    }
}

fn copy(up: u64, down: u64, carried: u64, post: &[u8]) -> BoardPost {
    BoardPost {
        up,
        down,
        post: post.to_vec(),
        number: None,
        carried: Some(carried),
    }
}

// functional_tests.md: BRD-003
#[test]
fn a_light_members_post_reaches_its_ancestors_and_their_direct_subordinates() {
    let mut s = scene();
    // w2 posts `+2/-0`: w2, the patron w1 with w1's subordinates, and the
    // operator with the operator's: w1 again, and carol, who runs a node
    let taken =
        s.n.receive_board(&*s.fab, &kh("w2"), &frame(2, 0, b"hello"));
    assert_eq!(
        taken,
        Taken::Carried {
            number: 1,
            delivered: 3,
            up: false,
            down: 1
        }
    );
    for member in ["w1", "w2", "bob"] {
        assert_eq!(
            got(&s.fab, member),
            vec![delivery(b"hello", 1, 2)],
            "{member}"
        );
    }
    // carol's node is sent a copy for its operator alone, and the root
    // nothing: two levels up from w2 is N's own operator
    assert_eq!(got(&s.fab, "carol"), vec![copy(0, 0, 2, b"hello")]);
    assert!(got(&s.fab, "alice").is_empty());
    // carol's node takes it from above: its operator, and not w3 beneath
    let mut c = node(
        "carol",
        &s.table,
        &[slot(&s.table, "bob"), slot(&s.table, "carol")],
        s.w.clock,
    );
    c.attached.insert(kh("w3"));
    let cfab = Fabric::with(&[kh("bob"), kh("carol"), kh("w3")]);
    let taken = c.receive_board(&*cfab, &kh("bob"), &relay(0, 0, 2, b"hello"));
    assert_eq!(
        taken,
        Taken::Carried {
            number: 1,
            delivered: 1,
            up: false,
            down: 0
        }
    );
    assert_eq!(got(&cfab, "carol"), vec![delivery(b"hello", 1, 2)]);
    assert!(got(&cfab, "w3").is_empty());
}

// functional_tests.md: BRD-005
#[test]
fn a_post_goes_up_as_far_as_its_writer_has_earned_and_no_further() {
    let mut s = scene();
    // w1 has nobody running a node beneath them: `+9` is carried as `+2`,
    // one level of it past this node, counted from this node
    let taken = s.n.receive_board(&*s.fab, &kh("w1"), &frame(9, 0, b"up"));
    assert!(matches!(taken, Taken::Carried { up: true, .. }));
    assert_eq!(got(&s.fab, "alice"), vec![copy(1, 1, 2, b"up")]);
    // and everyone it is delivered to, the writer among them, is told it
    // was carried as `+2` from its writer and not the `+9` asked
    for member in ["w1", "bob"] {
        assert_eq!(got(&s.fab, member), vec![delivery(b"up", 1, 2)], "{member}");
    }
    // the root takes it from the node beneath: itself and its direct
    // subordinates, of whom w4 runs a node and bob sent it
    let mut root = node("alice", &s.table, &[], s.w.clock);
    root.attached.insert(kh("alice"));
    let rfab = Fabric::with(&[kh("alice"), kh("bob"), kh("w4")]);
    let taken = root.receive_board(&*rfab, &kh("bob"), &relay(1, 1, 2, b"up"));
    assert_eq!(
        taken,
        Taken::Carried {
            number: 1,
            delivered: 1,
            up: false,
            down: 1
        }
    );
    assert_eq!(got(&rfab, "w4"), vec![copy(0, 0, 2, b"up")]);
    assert_eq!(got(&rfab, "alice"), vec![delivery(b"up", 1, 2)]);
    assert!(got(&rfab, "bob").is_empty(), "not back to its sender");
    // the operator's own downline runs a node one level down: `+9` is `+3`
    let taken =
        s.n.receive_board(&*s.fab, &kh("bob"), &frame(9, 0, b"mine"));
    assert!(matches!(taken, Taken::Carried { up: true, .. }));
    assert_eq!(got(&s.fab, "alice")[1], copy(3, 1, 3, b"mine"));
}

// functional_tests.md: BRD-006
#[test]
fn a_post_goes_down_through_the_nearest_nodes_with_what_is_left_of_its_count() {
    let mut s = scene();
    // the operator posts `-2`: carol and w1 one level down, w2 two; carol
    // runs a node and is sent one level more to carry
    let taken =
        s.n.receive_board(&*s.fab, &kh("bob"), &frame(0, 2, b"down"));
    assert_eq!(
        taken,
        Taken::Carried {
            number: 1,
            delivered: 3,
            up: false,
            down: 1
        }
    );
    assert_eq!(got(&s.fab, "carol"), vec![copy(0, 1, 0, b"down")]);
    for member in ["bob", "w1", "w2"] {
        assert_eq!(
            got(&s.fab, member),
            vec![delivery(b"down", 1, 0)],
            "{member}"
        );
    }
    // from above with nothing left, it is the operator's alone
    let taken =
        s.n.receive_board(&*s.fab, &kh("alice"), &relay(0, 0, 4, b"top"));
    assert_eq!(
        taken,
        Taken::Carried {
            number: 2,
            delivered: 1,
            up: false,
            down: 0
        }
    );
    assert_eq!(got(&s.fab, "bob")[1], delivery(b"top", 2, 4));
}

// functional_tests.md: BRD-002, BRD-007
#[test]
fn a_copy_that_is_a_repeat_or_fits_no_session_is_discarded_whole() {
    let mut s = scene();
    let first = frame(1, 1, b"once");
    assert!(matches!(
        s.n.receive_board(&*s.fab, &kh("w1"), &first),
        Taken::Carried { .. }
    ));
    let sent = s.fab.count(FRAME_BOARD_POST);
    // a second copy of the same post, by any path and with any counts
    assert_eq!(
        s.n.receive_board(&*s.fab, &kh("w1"), &first),
        Taken::Duplicate
    );
    assert_eq!(
        s.n.receive_board(&*s.fab, &kh("alice"), &relay(0, 5, 1, b"once")),
        Taken::Duplicate
    );
    // from a node beneath, a copy that is not on the upward leg
    assert_eq!(
        s.n.receive_board(&*s.fab, &kh("carol"), &relay(0, 3, 2, b"a")),
        Taken::Discarded(Discard::Leg)
    );
    assert_eq!(
        s.n.receive_board(&*s.fab, &kh("carol"), &relay(2, 2, 2, b"b")),
        Taken::Discarded(Discard::Leg)
    );
    // from above, one with anything left to go up
    assert_eq!(
        s.n.receive_board(&*s.fab, &kh("alice"), &relay(1, 0, 2, b"c")),
        Taken::Discarded(Discard::Leg)
    );
    // a number is a node's to give a member, and nobody's to give a node
    let numbered = delivery(b"d", 7, 2).encode();
    assert_eq!(
        s.n.receive_board(&*s.fab, &kh("w1"), &numbered),
        Taken::Malformed
    );
    // what a post was carried with is the first node's to say: a member
    // does not say it, and a node always does
    assert_eq!(
        s.n.receive_board(&*s.fab, &kh("w1"), &relay(2, 2, 2, b"g")),
        Taken::Malformed
    );
    assert_eq!(
        s.n.receive_board(&*s.fab, &kh("alice"), &frame(0, 1, b"h")),
        Taken::Malformed
    );
    assert_eq!(
        s.n.receive_board(&*s.fab, &kh("carol"), &frame(1, 1, b"i")),
        Taken::Malformed
    );
    assert_eq!(
        s.n.receive_board(&*s.fab, &kh("w1"), b"\xa0"),
        Taken::Malformed
    );
    // a sibling node, and a party this node has never placed
    assert_eq!(
        s.n.receive_board(&*s.fab, &kh("w4"), &frame(0, 0, b"e")),
        Taken::Stranger
    );
    assert_eq!(
        s.n.receive_board(&*s.fab, &kh("w6"), &frame(2, 2, b"f")),
        Taken::Stranger
    );
    assert_eq!(s.fab.count(FRAME_BOARD_POST), sent, "nothing more was sent");
    assert_eq!(s.n.board.len(), 1, "and nothing more kept");
}

// functional_tests.md: BRD-008
#[test]
fn a_node_carries_bytes_it_does_not_read() {
    let mut s = scene();
    // not CBOR, not signed, and naming nobody: carried as any post is
    for (n, post) in [&[0xff, 0x00, 0x13][..], b"{not json", &[0u8; 16_384]]
        .iter()
        .enumerate()
    {
        let taken = s.n.receive_board(&*s.fab, &kh("w1"), &frame(0, 0, post));
        assert!(
            matches!(taken, Taken::Carried { number, .. } if number == n as u64 + 1),
            "{taken:?}"
        );
    }
    // what is held is the bytes, a number and who they were for here
    for h in s.n.board.held() {
        assert!(h.band.holds(&[slot(&s.table, "w1")]));
    }
}

fn ask(n: &NodeView, who: &str, after: Option<u64>) -> Option<BoardReply> {
    let req = BoardRequest {
        nonce: [9; 16],
        after,
    };
    n.answer_board(&kh(who), &req.encode())
        .map(|b| BoardReply::decode(&b).expect("a reply"))
}

// functional_tests.md: BRD-011
#[test]
fn a_member_that_was_away_is_handed_its_own_band_after_the_number_it_holds() {
    let mut s = scene();
    s.n.receive_board(&*s.fab, &kh("bob"), &frame(0, 1, b"to the team"));
    s.n.receive_board(&*s.fab, &kh("w2"), &frame(0, 0, b"a note to self"));
    s.n.receive_board(&*s.fab, &kh("w1"), &frame(1, 1, b"from w1"));
    // w1 reads the operator's post and their own; w2's note is w2's alone
    let r = ask(&s.n, "w1", None).unwrap();
    assert_eq!(r.nonce, [9; 16]);
    assert_eq!(
        r.posts,
        vec![(1, b"to the team".to_vec(), 0), (3, b"from w1".to_vec(), 1)]
    );
    assert_eq!((r.through, r.more), (3, false));
    // w2 stands two levels down: not the operator's `-1`, but its own note
    // and its patron's `-1`
    let r = ask(&s.n, "w2", None).unwrap();
    assert_eq!(
        r.posts,
        vec![
            (2, b"a note to self".to_vec(), 0),
            (3, b"from w1".to_vec(), 1)
        ]
    );
    // after the number it holds, only what is newer
    assert_eq!(ask(&s.n, "w2", Some(2)).unwrap().posts.len(), 1);
    assert_eq!(ask(&s.n, "w2", Some(3)).unwrap().posts.len(), 0);
    // an asker holding no session attached here is refused, as is a
    // request that does not read
    assert!(ask(&s.n, "w3", None).is_none());
    assert!(s.n.answer_board(&kh("w1"), b"\xa0").is_none());
}

// functional_tests.md: BRD-014
#[test]
fn a_siblings_member_in_failover_is_carried_as_far_as_this_node_can() {
    let mut s = scene();
    // w5, one level below the sibling node w4, attaches here
    s.n.attached.insert(kh("w5"));
    s.fab.add_session(&kh("w5"));
    // what comes from above with one level left is what w4 would have
    // been sent: w5's, where w5 stands
    s.n.receive_board(&*s.fab, &kh("alice"), &relay(0, 1, 2, b"one down"));
    assert_eq!(got(&s.fab, "w5"), vec![delivery(b"one down", 1, 2)]);
    // and with none left it is the two operators' alone
    s.n.receive_board(&*s.fab, &kh("alice"), &relay(0, 0, 2, b"operators"));
    assert_eq!(got(&s.fab, "w5").len(), 1);
    // nothing that started beneath this node is theirs
    s.n.receive_board(&*s.fab, &kh("w1"), &frame(2, 2, b"local"));
    assert_eq!(got(&s.fab, "w5").len(), 1);
    // their own post goes upstream at what every member may post, counted
    // from this node, and reaches this node's operator
    s.fab.clear();
    let taken =
        s.n.receive_board(&*s.fab, &kh("w5"), &frame(9, 4, b"from w5"));
    assert_eq!(
        taken,
        Taken::Carried {
            number: 4,
            delivered: 1,
            up: true,
            down: 0
        }
    );
    assert_eq!(got(&s.fab, "alice"), vec![copy(1, 1, 2, b"from w5")]);
    assert_eq!(got(&s.fab, "bob"), vec![delivery(b"from w5", 4, 2)]);
    assert!(got(&s.fab, "w1").is_empty() && got(&s.fab, "carol").is_empty());
    // and catch-up is answered by the same test
    let r = ask(&s.n, "w5", None).unwrap();
    assert_eq!(r.posts, vec![(1, b"one down".to_vec(), 2)]);
}

// functional_tests.md: BRD-009
#[test]
fn the_queue_is_kept_a_file_a_post_and_taken_up_where_it_was_left() {
    let dir = std::env::temp_dir().join(format!("rhtn-board-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let mut s = scene();
    s.n.open_board(&dir).expect("opens");
    s.n.receive_board(&*s.fab, &kh("bob"), &frame(0, 1, b"first"));
    s.n.receive_board(&*s.fab, &kh("w1"), &frame(1, 0, b"second"));
    assert_eq!(std::fs::read_dir(&dir).unwrap().count(), 2);
    // the file is the count it was carried with and who it was for
    // here, then the post: no writer, no time
    let kept = std::fs::read(dir.join(format!("{:020}", 1))).unwrap();
    assert_eq!(kept, b"0 b=./1\nfirst");
    // started again from those files alone
    let mut again = scene();
    again.n.open_board(&dir).expect("opens again");
    assert_eq!(again.n.board, s.n.board);
    assert_eq!(
        ask(&again.n, "w1", None).unwrap().posts,
        vec![(1, b"first".to_vec(), 0), (2, b"second".to_vec(), 1)]
    );
    // and no number is given twice
    let taken = again
        .n
        .receive_board(&*again.fab, &kh("w1"), &frame(0, 0, b"third"));
    assert!(matches!(taken, Taken::Carried { number: 3, .. }));
    // a file that does not read is not held, and the rest are
    std::fs::write(dir.join(format!("{:020}", 9)), b"no band line").unwrap();
    std::fs::write(dir.join("stray"), b"0 b=./1\nnot a number").unwrap();
    std::fs::write(dir.join(format!("{:020}", 8)), b"b=./1\nno count").unwrap();
    let mut third = scene();
    third.n.open_board(&dir).expect("opens on a bad file");
    assert_eq!(third.n.board.len(), 3);
    let _ = std::fs::remove_dir_all(&dir);
}
