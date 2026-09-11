//! The Double Ratchet: messages in order, out of order, across ratchet
//! steps, and never twice.

use rhtn_client::ratchet::*;
use rhtn_crypto::pqxdh::DhSecret;

fn pair() -> (Ratchet, Ratchet) {
    let sk = [42u8; 32];
    let spk_b = DhSecret::from_seed([9; 32]);
    let ad = b"ad".to_vec();
    let a = Ratchet::initiator(sk, spk_b.public(), DhSecret::from_seed([1; 32]), ad.clone());
    let b = Ratchet::responder(sk, spk_b, ad);
    (a, b)
}

fn seeds(start: u8) -> impl FnMut() -> [u8; 32] {
    let mut i = start;
    move || {
        i = i.wrapping_add(1);
        [i; 32]
    }
}

#[test]
fn messages_cross_in_both_directions_and_out_of_order_and_decrypt_once() {
    let (mut a, mut b) = pair();
    let (mut fa, mut fb) = (seeds(100), seeds(200));
    assert!(a.can_send() && !b.can_send(), "the responder waits for the first message");
    let m1 = a.encrypt(b"one").unwrap();
    let m2 = a.encrypt(b"two").unwrap();
    let m3 = a.encrypt(b"three").unwrap();
    // out of order: three before two
    assert_eq!(b.decrypt(&m1, &mut fb).unwrap(), b"one");
    assert_eq!(b.decrypt(&m3, &mut fb).unwrap(), b"three");
    assert_eq!(b.decrypt(&m2, &mut fb).unwrap(), b"two");
    // never twice
    assert!(b.decrypt(&m2, &mut fb).is_err());
    assert!(b.decrypt(&m1, &mut fb).is_err());
    // the responder answers: a ratchet step on both sides
    assert!(b.can_send());
    let r1 = b.encrypt(b"reply").unwrap();
    assert_eq!(a.decrypt(&r1, &mut fa).unwrap(), b"reply");
    let m4 = a.encrypt(b"four").unwrap();
    assert_eq!(b.decrypt(&m4, &mut fb).unwrap(), b"four");
    // a tampered message does not open, and the state survives the attempt
    let mut bad = a.encrypt(b"five").unwrap();
    let n = bad.len();
    bad[n - 1] ^= 1;
    assert!(b.decrypt(&bad, &mut fb).is_err());
    let m6 = a.encrypt(b"six").unwrap();
    assert_eq!(b.decrypt(&m6, &mut fb).unwrap(), b"six");
    // a third party with the same public inputs and no secret opens nothing
    let mut c = Ratchet::responder([0u8; 32], DhSecret::from_seed([9; 32]), b"ad".to_vec());
    assert!(c.decrypt(&m1, &mut seeds(0)).is_err());
}

#[test]
fn too_many_skipped_messages_are_refused() {
    let (mut a, mut b) = pair();
    let mut fb = seeds(0);
    let first = a.encrypt(b"first").unwrap();
    assert_eq!(b.decrypt(&first, &mut fb).unwrap(), b"first");
    let mut last = Vec::new();
    for _ in 0..(MAX_SKIP + 2) {
        last = a.encrypt(b"x").unwrap();
    }
    assert!(b.decrypt(&last, &mut fb).is_err(), "beyond MAX_SKIP");
}
