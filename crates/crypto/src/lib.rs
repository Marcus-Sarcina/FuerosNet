//! `rhtn-crypto` — identities and signatures.
//!
//! An identity is a hybrid pair (design §5.1): an Ed25519 key and an
//! ML-DSA-65 key, hashed together into the keyhash.  Every signing context
//! carries its role tag as COSE `external_aad` (`wire-format.md` §1.1).  The
//! primitives come from `ed25519-dalek` and RustCrypto's `ml-dsa`; both the
//! post-quantum crates carry an unaudited notice (design §5.2), which is why
//! they are reached only through this crate.
//!
//! Verification functions take the structures `rhtn-codec` parses and check
//! the signatures over them; `verify::envelope` is the complete check of a
//! transaction envelope including embedded evidence.

pub mod identity;
pub mod pqxdh;
pub mod verify;

pub use identity::{Identity, SigningIdentity};
