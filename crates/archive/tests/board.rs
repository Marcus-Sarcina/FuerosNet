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
        carried: None,
    }
}

/// A copy as a node sends it, with the upward count it was carried with.
fn carried(up: u64, down: u64, bytes: &[u8], number: Option<u64>, carried: u64) -> BoardPost {
    BoardPost {
        carried: Some(carried),
        ..post(up, down, bytes, number)
    }
}

// functional: BRD-001
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
        carried(0, 0, b"for a member", Some(7), 24),
        carried(3, 1, b"between nodes", None, 0),
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
    assert!(BoardPost::decode(&carried(0, 0, b"x", None, 25).encode()).is_err());
    // an unknown field on an unsigned message
    let mut b = post(1, 1, b"x", Some(3)).encode();
    b[0] += 1;
    b.extend([0x06, 0x00]);
    assert!(BoardPost::decode(&b).is_err());
    // and a missing one: the map head says three and holds two
    let whole = post(1, 1, b"x", None).encode();
    let mut short = vec![0xa2];
    short.extend(&whole[1..5]);
    assert!(BoardPost::decode(&short).is_err());
}

// functional: BRD-010
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
        posts: vec![(42, b"a".to_vec(), 2), (45, vec![9; 16_384], 24)],
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
    let reply = |posts: Vec<(u64, Vec<u8>, u64)>| BoardReply {
        nonce: [3; 16],
        posts,
        through: 900,
        more: false,
    };
    let x = || b"x".to_vec();
    assert!(BoardReply::decode(&reply(vec![(2, x(), 2), (1, x(), 2)]).encode()).is_err());
    assert!(BoardReply::decode(&reply(vec![(2, x(), 2), (2, x(), 2)]).encode()).is_err());
    assert!(BoardReply::decode(&reply(vec![(1, Vec::new(), 2)]).encode()).is_err());
    assert!(BoardReply::decode(&reply(vec![(1, vec![0; 16_385], 2)]).encode()).is_err());
    let many = |n: u64| (1..=n).map(|i| (i, x(), 2)).collect::<Vec<_>>();
    assert!(BoardReply::decode(&reply(many(256)).encode()).is_ok());
    assert!(BoardReply::decode(&reply(many(257)).encode()).is_err());
    // fifteen posts of the largest size fit a reply's frame, sixteen do not
    let big = |n: u64| reply((1..=n).map(|i| (i, vec![0; 16_384], 2)).collect());
    assert!(big(15).encode().len() <= 262_144);
    assert!(big(16).encode().len() > 262_144);
}

// ------------------------------------------------- the post as it is signed

use rhtn_archive::tx::{Locator, Seqno};
use rhtn_crypto::identity::testkit::test_identity;

fn at(path: &[u8]) -> Locator {
    Locator {
        anchor: test_identity("alice").public.keyhash,
        path: path.to_vec(),
        nibbles: 2 * path.len() as u64,
        seqno: Seqno {
            series: 1,
            counter: 0,
        },
        subnet: Some([0xdb; 32]),
    }
}

fn draft<'a>(position: &'a Locator, text: &'a str) -> Draft<'a> {
    Draft {
        position,
        up: 2,
        down: 2,
        written_at: 1_800_000_000,
        thread: None,
        title: Some("A title"),
        text,
        alias: Some("Bob"),
    }
}

#[test]
fn a_post_carries_everything_a_reader_needs_and_verifies_under_the_key_it_carries() {
    let bob = test_identity("bob");
    let km = bob.public.key_material();
    let here = at(&[0x10]);
    let bytes = Post::build(&Writer::own(&bob, &km), &draft(&here, "the words")).expect("builds");
    let p = Post::parse(&bytes).expect("parses");
    assert_eq!(p.writer, bob.public.keyhash);
    assert_eq!(p.material, bob.public.key_material());
    assert_eq!(p.position, here);
    assert_eq!((p.up, p.down, p.written_at), (2, 2, 1_800_000_000));
    assert_eq!((p.thread, p.title.as_deref()), (None, Some("A title")));
    assert_eq!(
        (p.text.as_str(), p.alias.as_deref()),
        ("the words", Some("Bob"))
    );
    // a reader that has never held the writer checks it all the same
    assert_eq!(p.verify(), Ok(()));
    // a thread is named by the hash of the post that began it
    assert_eq!(p.thread_id(), Post::hash(&bytes));
    let reply = Post::build(
        &Writer::own(&bob, &km),
        &Draft {
            thread: Some(p.thread_id()),
            title: None,
            alias: None,
            ..draft(&here, "a reply")
        },
    )
    .expect("builds");
    let r = Post::parse(&reply).unwrap();
    assert_eq!(r.thread_id(), p.thread_id());
    assert_eq!(r.verify(), Ok(()));
    // and it fits a frame's post
    assert!(BoardPost::decode(&post(2, 2, &bytes, None).encode()).is_ok());
}

#[test]
fn a_post_changed_or_carrying_another_key_is_not_its_writers() {
    let bob = test_identity("bob");
    let km = bob.public.key_material();
    let here = at(&[0x10]);
    let bytes = Post::build(&Writer::own(&bob, &km), &draft(&here, "the words")).unwrap();
    // one byte of the words
    let mut changed = bytes.clone();
    let i = changed.windows(9).position(|w| w == b"the words").unwrap();
    changed[i] ^= 0x01;
    assert!(Post::parse(&changed).unwrap().verify().is_err());
    // carol's keys under bob's name: the material does not hash to field 1
    let mut forged = Post::parse(&bytes).unwrap();
    forged.material = test_identity("carol").public.key_material();
    assert!(forged.verify().is_err());
    // carol signing as herself over bob's keyhash does not parse back as
    // bob's either: build names the signer
    let carol = test_identity("carol");
    let hers_km = carol.public.key_material();
    let hers = Post::build(&Writer::own(&carol, &hers_km), &draft(&here, "the words")).unwrap();
    assert_ne!(Post::parse(&hers).unwrap().writer, bob.public.keyhash);
}

// functional: BRD-015
#[test]
fn a_post_begins_a_thread_or_continues_one_and_stays_within_its_bounds() {
    let bob = test_identity("bob");
    let km = bob.public.key_material();
    let here = at(&[0x10]);
    let d = draft(&here, "x");
    // both a thread and a title, and neither
    assert!(
        Post::build(
            &Writer::own(&bob, &km),
            &Draft {
                thread: Some([1; 32]),
                ..d.clone()
            }
        )
        .is_none()
    );
    assert!(
        Post::build(
            &Writer::own(&bob, &km),
            &Draft {
                title: None,
                ..d.clone()
            }
        )
        .is_none()
    );
    // no words, a title over its bound, a reach over the path bound
    assert!(
        Post::build(
            &Writer::own(&bob, &km),
            &Draft {
                text: "",
                ..d.clone()
            }
        )
        .is_none()
    );
    let long = "t".repeat(129);
    assert!(
        Post::build(
            &Writer::own(&bob, &km),
            &Draft {
                title: Some(&long),
                ..d.clone()
            }
        )
        .is_none()
    );
    assert!(
        Post::build(
            &Writer::own(&bob, &km),
            &Draft {
                up: 25,
                ..d.clone()
            }
        )
        .is_none()
    );
    // the words at their bound fit a post, and one byte more does not
    let most = "w".repeat(8192);
    let bytes = Post::build(
        &Writer::own(&bob, &km),
        &Draft {
            text: &most,
            ..d.clone()
        },
    )
    .expect("fits");
    assert!(bytes.len() <= 16_384, "{}", bytes.len());
    let over = "w".repeat(8193);
    assert!(
        Post::build(
            &Writer::own(&bob, &km),
            &Draft {
                text: &over,
                ..d.clone()
            }
        )
        .is_none()
    );
    // bytes that are no post do not parse
    assert!(Post::parse(b"\xa0").is_err());
    assert!(Post::parse(&vec![0; 16_385]).is_err());
}

#[test]
fn a_device_holding_no_seed_signs_under_the_key_it_was_delegated() {
    use rhtn_crypto::signer::DelegatedSigner;
    let bob = test_identity("bob");
    let km = bob.public.key_material();
    let here = at(&[0x10]);
    // the desktop's own transport key, and bob's delegation of it for a
    // window the post is written in
    let key = ed25519_dalek::SigningKey::from_bytes(&[7; 32]);
    let public = key.verifying_key().to_bytes();
    let delegation = rhtn_crypto::delegation::issue(&bob, &public, 1_800_000_000 - 60);
    let device = DelegatedSigner::new(bob.public.keyhash, key);
    let writer = Writer {
        signer: &device,
        material: &km,
        delegation: Some(&delegation),
    };
    let bytes = Post::build(&writer, &draft(&here, "from the desktop")).expect("builds");
    let p = Post::parse(&bytes).unwrap();
    assert!(p.delegated);
    assert_eq!(
        p.writer, bob.public.keyhash,
        "it is bob's post all the same"
    );
    assert_eq!(p.verify(), Ok(()));
    // with the words at their bound it still fits a post
    let most = "w".repeat(8192);
    let full = Post::build(
        &writer,
        &Draft {
            text: &most,
            ..draft(&here, "")
        },
    )
    .expect("fits");
    assert!(full.len() <= 16_384, "{}", full.len());
    // written outside the window the key was delegated for
    let late = Post::build(
        &writer,
        &Draft {
            written_at: 1_800_000_000 + 172_800,
            ..draft(&here, "late")
        },
    )
    .unwrap();
    assert!(Post::parse(&late).unwrap().verify().is_err());
    // the delegated key without its delegation is nobody's signature
    let bare = Writer {
        delegation: None,
        ..writer
    };
    let unstapled = Post::build(&bare, &draft(&here, "bare")).unwrap();
    assert!(Post::parse(&unstapled).unwrap().verify().is_err());
    // and another identity's delegation does not make it bob's
    let carol = test_identity("carol");
    let hers = rhtn_crypto::delegation::issue(&carol, &public, 1_800_000_000 - 60);
    let wrong = Writer {
        delegation: Some(&hers),
        ..Writer {
            signer: &device,
            material: &km,
            delegation: None,
        }
    };
    let crossed = Post::build(&wrong, &draft(&here, "crossed")).unwrap();
    assert!(Post::parse(&crossed).unwrap().verify().is_err());
}
