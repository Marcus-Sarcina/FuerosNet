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
/// Issuing an identity's encryption key (`wire-format.md` §2.5).
pub mod encryption_key;

/// The proof an instance offers for the transport key it minted
/// (`infra-client-requirements.md` §8.2).
pub mod enrolment;
/// The hybrid identity, its keyhash, and the signing half.
pub mod identity;
/// HKDF-SHA-256, the derivation every keyed label here expands through.
pub mod kdf;
/// A serving node's key for one recipient (`wire-format.md` §7.8): what
/// the node signs and publishes, and the private half it opens with.
pub mod node_key;
/// **The key a package's storage is sealed under** on a node this identity
/// operates (`infra-client-requirements.md` §9.4): derived from the signing
/// seed and the resource, so the phrase that restores the identity reopens
/// what it sealed.
pub mod package_seal;
/// The split seal (design §14.2.4): the identity's encryption key, the
/// message key, the AEAD and the sender's tag.
pub mod seal;
pub mod signer;
/// What a caller must provide to sign: the trait the profile's signing
/// contexts are written against.
/// **The ssh key that controls a node this identity launched**: derived
/// from the signing seed, so the phrase that restores the identity
/// restores the shell on its machines (`light-client-requirements.md`
/// §10).
pub mod ssh;
/// Verification of the structures `rhtn-codec` parses, up to a complete
/// transaction envelope with its embedded evidence.
pub mod verify;
/// X-Wing (`draft-connolly-cfrg-xwing-kem`): ML-KEM-768 and X25519 as one
/// KEM, each half of the split seal.
pub mod xwing;

pub use identity::{Identity, SigningIdentity};
