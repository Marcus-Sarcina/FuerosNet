//! The subnet's board over live sessions (`wire-format.md` §10.3;
//! functional_tests.md BRD-001, BRD-007, BRD-010, BRD-011): a member's
//! post rides control frame 10 to its node, the node delivers it numbered
//! to every member in its band, and request type 17 hands a member what
//! it missed.

mod common;

use common::*;
use rhtn_archive::board::{BoardPost, BoardReply, BoardRequest, FRAME_BOARD_POST, REQUEST_BOARD};
use rhtn_node::resolution::{AnchorTable, Ingestion};
use rhtn_node::runtime::LiveNode;
use rhtn_transport::session::*;

const I: u64 = 30;

/// The next `BoardPost` frame a session is sent, past whatever topology
/// its node replays at attach.
async fn next_post(s: &mut Session) -> Option<BoardPost> {
    let wait = std::time::Duration::from_secs(5);
    loop {
        match tokio::time::timeout(wait, s.frames.recv()).await {
            Ok(Some((FRAME_BOARD_POST, body))) => return BoardPost::decode(&body).ok(),
            Ok(Some(_)) => {}
            _ => return None,
        }
    }
}

async fn ask(s: &Session, after: Option<u64>) -> Result<BoardReply, String> {
    let req = BoardRequest {
        nonce: [4; 16],
        after,
    };
    let reply = s.request(REQUEST_BOARD, &req.encode()).await?;
    BoardReply::decode(&reply)
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_post_crosses_live_sessions_and_a_member_that_was_away_is_handed_it() {
    let _one = serial().await;
    // bob's node under alice, with two light members beneath it
    let mut s = Signers::new();
    let a_n = s.adopt("bob", "alice", "alice", &[0], 1);
    let a_c = s.adopt("carol", "bob", "alice", &[0, 0], 2);
    let a_w = s.adopt("w1", "bob", "alice", &[0, 1], 3);
    let records = [&a_n, &a_c, &a_w];
    let mut view = view_of(
        "bob",
        table_of("bob", &s, &records, &["alice", "bob"]),
        "alice",
        &[0],
        s.clock,
    );
    view.set_slot(0, Some(kh("carol")), s.clock);
    view.set_slot(1, Some(kh("w1")), s.clock);
    view.sync_downline();
    let n = LiveNode::start(
        node_cfg("bob", I),
        view,
        ids(),
        AnchorTable::new(0, Ingestion::UnverifiedGossip),
    );
    let attached = |name: &'static str| {
        let addr = n.addr;
        async move {
            let cfg = client_cfg(name);
            know(&cfg, "bob", addr);
            match attach(&cfg, &client_ep(), kh("bob"), addr, false).await {
                AttachOutcome::Attached(s) => s,
                _ => panic!("{name} attaches to the node"),
            }
        }
    };
    let mut carol = attached("carol").await;
    let mut w1 = attached("w1").await;

    // carol posts `+1/-0`: her patron and her patron's subordinates
    let frame = BoardPost {
        up: 1,
        down: 0,
        post: b"bytes the node does not read".to_vec(),
        number: None,
    };
    assert!(carol.send_control(FRAME_BOARD_POST, &frame.encode()));
    let delivered = next_post(&mut w1).await.expect("w1 is in the band");
    assert_eq!(delivered.post, frame.post);
    assert_eq!(
        (delivered.up, delivered.down, delivered.number),
        (0, 0, Some(1))
    );
    // and it comes back to its writer numbered, which is how her device
    // learns the node took it
    assert_eq!(next_post(&mut carol).await, Some(delivered.clone()));

    // a note to self reaches its writer alone
    let note = BoardPost {
        up: 0,
        down: 0,
        post: b"a note to self".to_vec(),
        number: None,
    };
    assert!(w1.send_control(FRAME_BOARD_POST, &note.encode()));
    assert_eq!(next_post(&mut w1).await.map(|p| p.number), Some(Some(2)));

    // what each missed, by request: its own band after the number it holds
    let all = ask(&w1, None).await.expect("answered");
    assert_eq!(all.nonce, [4; 16]);
    assert_eq!(
        all.posts,
        vec![(1, frame.post.clone()), (2, note.post.clone())]
    );
    assert_eq!((all.through, all.more), (2, false));
    let hers = ask(&carol, None).await.expect("answered");
    assert_eq!(hers.posts, vec![(1, frame.post.clone())]);
    assert_eq!(ask(&w1, Some(1)).await.unwrap().posts.len(), 1);
    assert!(ask(&w1, Some(2)).await.unwrap().posts.is_empty());

    // a malformed frame is discarded and the session survives it
    assert!(w1.send_control(FRAME_BOARD_POST, b"\xa0"));
    assert!(ask(&w1, Some(2)).await.is_ok());

    // a party the node cannot place is carried nothing and answered nothing
    let w2 = attached("w2").await;
    assert!(w2.send_control(FRAME_BOARD_POST, &frame.encode()));
    assert!(
        ask(&w2, None).await.is_err(),
        "the stream closes with no reply"
    );
    assert_eq!(n.view.lock().unwrap().board.len(), 2);
}
