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

#![warn(missing_docs)]

/// Transport delegations (`wire-format.md` §8.2): a window-bounded
/// credential an identity signs for a key that is not its own.
pub mod delegation;

/// The proof an instance offers for the transport key it minted
/// (`infra-client-requirements.md` §8.2).
pub mod enrolment;
/// The hybrid identity, its keyhash, and the signing half.
pub mod identity;
/// PQXDH (design §5.3): the key agreement a payload session opens with.
pub mod pqxdh;
/// What a caller must provide to sign: the trait the profile's signing
/// contexts are written against.
pub mod signer;
/// Verification of the structures `rhtn-codec` parses, up to a complete
/// transaction envelope with its embedded evidence.
pub mod verify;

pub use identity::{Identity, SigningIdentity};
