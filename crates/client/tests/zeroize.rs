//! **Key material is wiped, not left in freed memory** (APP-012;
//! `light-client-requirements.md` §3; design §14.2.4.3): the walk over
//! every holder, each asserting the marker, and the hand-written drops
//! checked by dropping a value in place and reading what is left.  Not
//! observable to a peer and not enforceable against one; held here
//! because no expert review of this implementation is coming.

use rhtn_client::local::DeviceCredential;
use rhtn_client::payload::PayloadState;
use rhtn_crypto::SigningIdentity;
use rhtn_crypto::{seal::EncryptionSecret, xwing};
use std::mem::ManuallyDrop;
use zeroize::ZeroizeOnDrop;

/// The claim, at compile time: `T` wipes what it holds when dropped.
fn wipes_on_drop<T: ZeroizeOnDrop>() {}

/// **The walk**: every type that holds key material — the identity, its
/// encryption secrets, an X-Wing secret, and the 32-byte secrets a pairing
/// carries.  A holder added without its marker fails here, which is the
/// point.
// functional: APP-012
#[test]
fn every_holder_of_key_material_claims_to_wipe_on_drop() {
    wipes_on_drop::<xwing::Secret>();
    wipes_on_drop::<EncryptionSecret>();
    wipes_on_drop::<SigningIdentity>();
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

/// **The drops leave zeros behind**: an X-Wing secret, the identity's
/// encryption secrets on their own and as the payload state holds them,
/// and the two secrets a pairing's credential carries, each read back as
/// zeros after its drop ran.
// functional: APP-012
#[test]
fn the_drops_leave_zeros_behind() {
    let x = xwing::Secret::from_seed([3; 32]);
    assert!(!x.seed_wiped());
    let x = dropped_in_place(x);
    assert!(x.seed_wiped(), "the X-Wing seed is zero");

    let e = EncryptionSecret::from_identity_root(&[5; 32]);
    assert_ne!(*e.kem_seed(), [0; 32]);
    assert_ne!(*e.auth_secret(), [0; 32]);
    let e = dropped_in_place(e);
    assert_eq!(*e.kem_seed(), [0; 32], "the X-Wing seed is zero");
    assert_eq!(
        *e.auth_secret(),
        [0; 32],
        "the authentication scalar is zero: x25519-dalek wipes it"
    );

    // the payload state holds the identity's secrets on every device
    let p = PayloadState {
        secret: Some(EncryptionSecret::from_identity_root(&[6; 32])),
        ..PayloadState::default()
    };
    let p = dropped_in_place(p);
    let held = p.secret.as_ref().expect("the shell keeps its discriminant");
    assert_eq!(*held.kem_seed(), [0; 32], "the state's X-Wing seed is zero");
    assert_eq!(*held.auth_secret(), [0; 32], "and its authentication key");

    // what a pairing hands a desktop
    let c = DeviceCredential {
        run: vec![b"a delegation".to_vec()],
        kem_seed: zeroize::Zeroizing::new([7; 32]),
        auth: zeroize::Zeroizing::new([8; 32]),
    };
    let c = dropped_in_place(c);
    assert_eq!(*c.kem_seed, [0; 32], "the credential's X-Wing seed is zero");
    assert_eq!(
        *c.auth, [0; 32],
        "the credential's authentication key is zero"
    );
}
