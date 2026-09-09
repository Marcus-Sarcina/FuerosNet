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

pub mod bounds;
pub mod cbor;
pub mod cose;
pub mod encode;
pub mod envelope;
pub mod frame;
pub mod schema;

pub use cbor::{Error, Item, Parser, parse_all};
