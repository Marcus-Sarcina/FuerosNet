//! **Key material is wiped, not left in freed memory** (APP-012;
//! `light-client-requirements.md` §3; design §14.2.4.3): the walk over
//! every holder, each asserting the marker, and the hand-written drops
//! checked by dropping a value in place and reading what is left.  Not
//! observable to a peer and not enforceable against one; held here
//! because no expert review of this implementation is coming.

use rhtn_client::payload::{OneTimePair, PayloadKeys};
use rhtn_client::ratchet::{NoPostQuantum, Ratchet};
use rhtn_crypto::SigningIdentity;
use rhtn_crypto::pqxdh::{DhSecret, KemSecret};
use rhtn_crypto::{seal::EncryptionSecret, xwing};
use std::mem::ManuallyDrop;
use zeroize::ZeroizeOnDrop;

/// The claim, at compile time: `T` wipes what it holds when dropped.
fn wipes_on_drop<T: ZeroizeOnDrop>() {}

/// **The walk**: every type that holds key material, root keys, chain
/// keys, message keys and prekey seeds among them.  A holder added without
/// its marker fails here, which is the point.
// functional: APP-012
#[test]
fn every_holder_of_key_material_claims_to_wipe_on_drop() {
    wipes_on_drop::<DhSecret>();
    wipes_on_drop::<KemSecret>();
    wipes_on_drop::<xwing::Secret>();
    wipes_on_drop::<EncryptionSecret>();
    wipes_on_drop::<SigningIdentity>();
    wipes_on_drop::<Ratchet<NoPostQuantum>>();
    wipes_on_drop::<PayloadKeys>();
    wipes_on_drop::<OneTimePair>();
    wipes_on_drop::<zeroize::Zeroizing<[u8; 32]>>();
}

/// Drop `v` where it stands and hand back the shell for reading: the
/// audit's one unsafe step, bounded to plain arrays the drop was to wipe.
/// The memory stays owned by the `ManuallyDrop`, so nothing reuses it
/// before the read.
fn dropped_in_place<T>(v: T) -> ManuallyDrop<T> {
    let mut m = ManuallyDrop::new(v);
    // SAFETY: `m` owns the memory for the rest of this function and the
    // caller's read; the value is dropped exactly once, here, and never
    // again, since `ManuallyDrop` does not drop.
    unsafe { std::ptr::drop_in_place(&mut *m) };
    m
}

/// **The hand-written drops do what they say**: a ratchet mid-conversation
/// with a skipped message key, a KEM secret, and the scalar the dependency
/// wipes for us, each read back as zeros after its drop ran.
// functional: APP-012
#[test]
fn the_drops_leave_zeros_behind() {
    let sk = [42u8; 32];
    let spk_b = DhSecret::from_seed([9; 32]);
    let ad = b"ad".to_vec();
    let mut a: Ratchet =
        Ratchet::initiator(sk, spk_b.public(), DhSecret::from_seed([1; 32]), ad.clone());
    let mut b: Ratchet = Ratchet::responder(sk, spk_b, ad);
    let mut fb = {
        let mut i = 200u8;
        move || {
            i = i.wrapping_add(1);
            [i; 32]
        }
    };
    let m1 = a.encrypt(b"one").unwrap();
    let _m2 = a.encrypt(b"two").unwrap();
    let m3 = a.encrypt(b"three").unwrap();
    assert_eq!(b.decrypt(&m1, &mut fb).unwrap(), b"one");
    // three arrives before two: a skipped message key is held for two
    assert_eq!(b.decrypt(&m3, &mut fb).unwrap(), b"three");
    assert!(!a.wiped() && !b.wiped(), "live ratchets hold keys");
    let a = dropped_in_place(a);
    let b = dropped_in_place(b);
    assert!(a.wiped(), "the initiator's root and chain keys are zero");
    assert!(
        b.wiped(),
        "the responder's root and chain keys are zero; its skipped key for two was wiped in the same drop, before its map freed"
    );

    let k = KemSecret::from_seed([7; 64]);
    assert!(!k.seed_wiped());
    let k = dropped_in_place(k);
    assert!(k.seed_wiped(), "the KEM seed is zero");

    let x = xwing::Secret::from_seed([3; 32]);
    assert!(!x.seed_wiped());
    let x = dropped_in_place(x);
    assert!(x.seed_wiped(), "the X-Wing seed is zero");

    // the dependency's own wipe, which the marker on `DhSecret` rests on
    let d = DhSecret::from_seed([5; 32]);
    assert_ne!(d.to_bytes(), [0; 32]);
    let d = dropped_in_place(d);
    assert_eq!(d.to_bytes(), [0; 32], "x25519-dalek wipes the scalar");
}
