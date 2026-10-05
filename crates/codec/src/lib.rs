//! `rhtn-codec` — the wire format as bytes.
//!
//! Answers to `wire-format.md` §1 to §4 and §7 to §11 (the encoding layer):
//! deterministic CBOR validated on the received bytes, the structural bounds
//! of §1.3, the transaction envelope and its signer set, control and request
//! frames, and the unsigned message families.  Nothing here verifies a
//! signature; `rhtn-crypto` does that over the structures this crate parses.
//!
//! Every decoder here returns a verdict for every input and never panics:
//! the parser is iterative, every length is checked before use, and no
//! declared size is trusted before the bytes behind it are present.
//!
//! Section references in this crate are to `wire-format.md` unless prefixed
//! `design`.
//!
//! **Every public item here carries a comment and the lint below keeps it
//! so** [author, 2026-10-04]: a public item added without one fails the
//! gate, which runs `clippy -D warnings`.
#![warn(missing_docs)]

/// The structural bounds of §1.3, and the envelope ceiling derived from
/// them.
pub mod bounds;
/// Deterministic CBOR (§1.2): the parser, its item tree of ranges into the
/// received bytes, and the accessors over a parsed map.
pub mod cbor;
/// The COSE profile's pure-encoding parts (§1, §1.1, §1.4, §2.2): the
/// `Sig_structure` a signature covers, the `KeyMaterial` an identity is
/// hashed from, the role tags, and content addressing.
pub mod cose;
/// Deterministic emission (§1.2), the counterpart to [`cbor`]'s parser.
pub mod encode;
/// The transaction envelope (§3) and the signer set its type derives.
pub mod envelope;
/// Control and request frames (§8.0, §9.2) and the families they carry.
pub mod frame;
/// The per-family field schemas and the checks that hold a map to one.
pub mod schema;

pub use cbor::{Error, Item, Parser, parse_all};
