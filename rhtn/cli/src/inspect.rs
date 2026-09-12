//! Decode what the wire carries and print it.
//!
//! `rhtn-codec` already parses every family strictly and names its reason
//! for refusing. This turns that into something a person reads: bytes in,
//! the object's family, its fields, and the txid where the object has one.
//!
//! What it owes:
//!
//! - **Take the bytes as they arrived**, from a file or standard input, and
//!   decode them with the same parser the node uses. A second, laxer
//!   decoder written for convenience would disagree with the first, and the
//!   disagreement would be invisible.
//! - **Print the refusal, not a stack trace**, when the bytes are
//!   malformed. The reason the strict decoder gives is the useful output.
//! - **Recompute what is derivable** and say whether it matches: the txid
//!   over the body map, the genesis value for a first transaction, the
//!   query id over its fields.
//! - **Verify signatures where the keys are to hand**, and say plainly when
//!   they are not. An unverifiable envelope is not an invalid one
//!   (`wire-format.md` §3).
