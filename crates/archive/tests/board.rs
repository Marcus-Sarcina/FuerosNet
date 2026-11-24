//! The board's wire objects (`wire-format.md` §10.3; BRD-001, BRD-010).

use rhtn_archive::board::*;
use rhtn_codec::frame::{Stream, family_of};
use rhtn_codec::schema::Family;

fn post(up: u64, down: u64, bytes: &[u8], number: Option<u64>) -> BoardPost {
    BoardPost {
        up,
        down,
        post: bytes.to_vec(),
        number,
    }
}

#[test]
fn a_post_is_frame_ten_and_the_asking_request_seventeen() {
    assert_eq!((FRAME_BOARD_POST, REQUEST_BOARD), (10, 17));
    assert_eq!(family_of(Stream::Control, 10), Some(Family::BoardPost));
    assert_eq!(family_of(Stream::Request, 17), Some(Family::BoardRequest));
}

#[test]
fn a_post_round_trips_with_and_without_the_nodes_number() {
    for p in [
        post(0, 0, b"x", None),
        post(24, 24, &[7; 16_384], None),
        post(0, 0, b"for a member", Some(u64::MAX)),
    ] {
        assert_eq!(BoardPost::decode(&p.encode()), Ok(p));
    }
}

#[test]
fn a_post_out_of_bounds_does_not_decode() {
    // an empty post, one a byte over, and a count past the path bound
    assert!(BoardPost::decode(&post(0, 0, b"", None).encode()).is_err());
    assert!(BoardPost::decode(&post(0, 0, &[7; 16_385], None).encode()).is_err());
    assert!(BoardPost::decode(&post(25, 0, b"x", None).encode()).is_err());
    assert!(BoardPost::decode(&post(0, 25, b"x", None).encode()).is_err());
    // an unknown field on an unsigned message
    let mut b = post(1, 1, b"x", Some(3)).encode();
    b[0] += 1;
    b.extend([0x05, 0x00]);
    assert!(BoardPost::decode(&b).is_err());
    // and a missing one: the map head says three and holds two
    let whole = post(1, 1, b"x", None).encode();
    let mut short = vec![0xa2];
    short.extend(&whole[1..5]);
    assert!(BoardPost::decode(&short).is_err());
}

#[test]
fn the_asking_and_its_answer_round_trip() {
    for r in [
        BoardRequest {
            nonce: [1; 16],
            after: None,
        },
        BoardRequest {
            nonce: [2; 16],
            after: Some(41),
        },
    ] {
        assert_eq!(BoardRequest::decode(&r.encode()), Ok(r));
    }
    let reply = BoardReply {
        nonce: [3; 16],
        posts: vec![(42, b"a".to_vec()), (45, vec![9; 16_384])],
        through: 47,
        more: true,
    };
    assert_eq!(BoardReply::decode(&reply.encode()), Ok(reply));
    let empty = BoardReply {
        nonce: [3; 16],
        posts: Vec::new(),
        through: 0,
        more: false,
    };
    assert_eq!(BoardReply::decode(&empty.encode()), Ok(empty));
}

#[test]
fn an_answer_out_of_order_or_over_its_bounds_does_not_decode() {
    let reply = |posts: Vec<(u64, Vec<u8>)>| BoardReply {
        nonce: [3; 16],
        posts,
        through: 900,
        more: false,
    };
    let x = || b"x".to_vec();
    assert!(BoardReply::decode(&reply(vec![(2, x()), (1, x())]).encode()).is_err());
    assert!(BoardReply::decode(&reply(vec![(2, x()), (2, x())]).encode()).is_err());
    assert!(BoardReply::decode(&reply(vec![(1, Vec::new())]).encode()).is_err());
    assert!(BoardReply::decode(&reply(vec![(1, vec![0; 16_385])]).encode()).is_err());
    let many = |n: u64| (1..=n).map(|i| (i, x())).collect::<Vec<_>>();
    assert!(BoardReply::decode(&reply(many(256)).encode()).is_ok());
    assert!(BoardReply::decode(&reply(many(257)).encode()).is_err());
    // fifteen posts of the largest size fit a reply's frame, sixteen do not
    let big = |n: u64| reply((1..=n).map(|i| (i, vec![0; 16_384])).collect());
    assert!(big(15).encode().len() <= 262_144);
    assert!(big(16).encode().len() > 262_144);
}
