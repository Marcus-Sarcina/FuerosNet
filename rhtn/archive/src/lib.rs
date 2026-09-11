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

pub mod chain;
pub mod currency;
pub mod locator;
pub mod prekey;
pub mod record;
pub mod series;
pub mod topology;
pub mod tx;
pub mod walk;

pub type Keyhash = [u8; 32];
pub type Txid = [u8; 32];

/// The genesis back-pointer of a key: SHA-256 over the 32 keyhash bytes
/// themselves (`wire-format.md` §3.1).
pub fn genesis(key: &Keyhash) -> Txid {
    rhtn_codec::cose::sha256(key)
}

/// design §10.1's window: pruning is permitted only beyond it.
pub const WINDOW_SECONDS: u64 = 730 * 86_400;
