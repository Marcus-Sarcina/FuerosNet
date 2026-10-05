//! The archive and topology layer of the reference implementation
//! (`Robot/implementation-plan.md`, crate `rhtn-archive`).
//!
//! - [`tx`] builds transaction bodies and envelopes with their chain
//!   back-pointers (`wire-format.md` §3.1, §4).
//! - [`record`] parses one envelope into the facts a chain walker needs.
//! - [`chain`] is one key's archive: heads, merges, serving (`wire-format.md`
//!   §7.9), pruning at a checkpoint (design §10.1) and the evidence store the
//!   chain does not govern (design §10.0).
//! - [`walk`] verifies a presented or fetched history backward from a head
//!   and says where it roots (design §10.1, `wire-format.md` §3.4).
//! - [`topology`] is the local table of verified bindings and what a node
//!   derives from it (design §6, §11.2.1, §6.2.5, §14.1.2).
//! - [`currency`] reads currency attestations and reports a fork (design §9.0.2).

#![warn(missing_docs)]

/// The resource objects the network carries (`wire-format.md` §6, §11):
/// the scope vocabulary, the catalog entry, the abuse report, and the
/// query, registration and request with their answers.
pub mod catalog;
/// One key's archive: heads, merges, serving (`wire-format.md` §7.9),
/// pruning at a checkpoint (design §10.1) and the evidence store the
/// chain does not govern (design §10.0).
pub mod chain;
/// Currency attestations and the fork a conflicting pair reports (design
/// §9.0.2).
pub mod currency;
/// A node's signed endpoint record (`wire-format.md` §7.6).
pub mod endpoint;
/// A standalone signed locator (`wire-format.md` §2.3).
pub mod locator;
/// Prekey bundles, their request and its reply (`wire-format.md` §7.8).
pub mod prekey;
/// One envelope parsed into the facts a chain walker needs.
pub mod record;
/// A relationship's series: the adoption that opened it and the reissues
/// that moved it.
pub mod series;
/// What a client hands its serving node and what comes back
/// (`wire-format.md` §7.10): publications, deposits, relays and wakes.
pub mod submission;
/// The local table of verified bindings and what a node derives from it
/// (design §6, §11.2.1, §6.2.5, §14.1.2).
pub mod topology;
/// Transaction bodies and envelopes with their chain back-pointers
/// (`wire-format.md` §3.1, §4).
pub mod tx;
/// Verifying a presented or fetched history backward from a head, and
/// where it roots (design §10.1, `wire-format.md` §3.4).
pub mod walk;

/// An identity, as every structure names one: `SHA-256` of its
/// `KeyMaterial` (`wire-format.md` §2.2).
pub type Keyhash = [u8; 32];
/// A transaction, as every reference names one: `SHA-256` of its body
/// map (`wire-format.md` §1.4).
pub type Txid = [u8; 32];

/// The genesis back-pointer of a key: SHA-256 over the 32 keyhash bytes
/// themselves (`wire-format.md` §3.1).
pub fn genesis(key: &Keyhash) -> Txid {
    rhtn_codec::cose::sha256(key)
}

/// design §10.1's window: pruning is permitted only beyond it.
pub const WINDOW_SECONDS: u64 = 730 * 86_400;
