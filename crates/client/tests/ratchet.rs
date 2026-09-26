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
    assert!(
        a.can_send() && !b.can_send(),
        "the responder waits for the first message"
    );
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
    let mut c: Ratchet =
        Ratchet::responder([0u8; 32], DhSecret::from_seed([9; 32]), b"ad".to_vec());
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

/// A stand-in for the post-quantum half: it contributes a fixed secret at
/// every root advance and carries one chunk of its own.
///
/// **Not a Sparse Post-Quantum Ratchet** and not pretending to be. Its
/// only job is to prove the seam is load-bearing, and it contributes
/// unconditionally because the two ends reach a given root advance at
/// different moments — the initiator mixes when it is built, the responder
/// when it first steps — so a stub whose contribution depended on what it
/// had heard would derive two different roots and prove only that the
/// stub was wrong. Agreeing on what to mix at each advance is the real
/// ratchet's problem and is why it needs the carriage.
#[derive(Clone, Default, PartialEq, Eq, Debug)]
struct Stub {
    sent: bool,
    heard: bool,
}

impl PostQuantumRatchet for Stub {
    fn contribution(&mut self) -> Option<[u8; 32]> {
        Some([0xABu8; 32])
    }
    fn outgoing(&mut self) -> Vec<u8> {
        if self.sent {
            return Vec::new();
        }
        self.sent = true;
        b"chunk".to_vec()
    }
    fn incoming(&mut self, chunk: &[u8]) -> Result<(), String> {
        if chunk != b"chunk" {
            return Err("not this ratchet's chunk".into());
        }
        self.heard = true;
        Ok(())
    }
    fn encode(&self) -> Vec<u8> {
        vec![self.sent as u8, self.heard as u8]
    }
    fn restore(&mut self, b: &[u8]) -> Result<(), String> {
        let [s, h] = b else {
            return Err("two flags".into());
        };
        (self.sent, self.heard) = (*s == 1, *h == 1);
        Ok(())
    }
}

/// The seam carries and mixes: a chunk crosses in the header, and the root
/// advance takes the post-quantum contribution with the Diffie-Hellman
/// output (design §14.2.4.3).  The floor is the same ratchet with nothing
/// on the other side of the seam.
// acceptance: PAY-22
#[test]
fn the_post_quantum_seam_carries_its_chunk_and_changes_what_the_root_derives() {
    let sk = [42u8; 32];
    let spk = DhSecret::from_seed([9; 32]);
    let ad = b"ad".to_vec();

    // with the stub on both sides
    let mut a: Ratchet<Stub> =
        Ratchet::initiator(sk, spk.public(), DhSecret::from_seed([1; 32]), ad.clone());
    let mut b: Ratchet<Stub> = Ratchet::responder(sk, spk.clone(), ad.clone());
    let m1 = a.encrypt(b"one").expect("sends");
    assert!(
        m1.windows(5).any(|w| w == b"chunk"),
        "the chunk rides the header in the clear, as carriage and not payload"
    );
    assert_eq!(b.decrypt(&m1, &mut seeds(70)).expect("opens"), b"one");

    // the reply steps the ratchet on both sides, each mixing the
    // contribution into its root advance
    let m2 = b.encrypt(b"two").expect("sends");
    assert_eq!(a.decrypt(&m2, &mut seeds(80)).expect("opens"), b"two");

    // the same exchange at the floor: same inputs, no seam
    let mut c: Ratchet =
        Ratchet::initiator(sk, spk.public(), DhSecret::from_seed([1; 32]), ad.clone());
    let mut d: Ratchet = Ratchet::responder(sk, spk, ad);
    let f1 = c.encrypt(b"one").expect("sends");
    assert!(
        !f1.windows(5).any(|w| w == b"chunk"),
        "the floor carries nothing and its header is the header it always was"
    );
    assert_eq!(d.decrypt(&f1, &mut seeds(70)).expect("opens"), b"one");
    let f2 = d.encrypt(b"two").expect("sends");

    assert_ne!(
        m2, f2,
        "the contribution reached the root chain: the same messages under \
         the same keys encrypt differently once the seam has something in it"
    );
}

/// The post-quantum half is persisted with the rest, and a state written
/// before the seam existed still opens.
#[test]
fn the_seam_persists_and_reads_the_state_written_before_it() {
    let sk = [42u8; 32];
    let spk = DhSecret::from_seed([9; 32]);
    let mut a: Ratchet<Stub> = Ratchet::initiator(
        sk,
        spk.public(),
        DhSecret::from_seed([1; 32]),
        b"ad".to_vec(),
    );
    let _ = a.encrypt(b"one").expect("sends");
    let back: Ratchet<Stub> = <Ratchet<Stub>>::decode(&a.encode()).expect("round trips");
    assert_eq!(back.encode(), a.encode(), "including the post-quantum half");

    // ten elements is what a state written before the seam carries
    let floor: Ratchet = Ratchet::initiator(
        sk,
        spk.public(),
        DhSecret::from_seed([1; 32]),
        b"ad".to_vec(),
    );
    let whole = floor.encode();
    let ten = {
        let mut v = whole.clone();
        v[0] = 0x8a; // array(11) -> array(10)
        v.truncate(v.len() - 1); // drop the trailing empty bstr
        v
    };
    assert!(
        <Ratchet>::decode(&ten).is_some(),
        "the preceding shape is read, not refused"
    );
}
