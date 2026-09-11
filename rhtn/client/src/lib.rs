//! The participant client above the session (`Robot/implementation-plan.md`,
//! crate `rhtn-client`; `light-client-requirements.md` §1; design §7, §8).
//!
//! - [`keys`] is the two constructions a ceremony fixes to the byte: the
//!   contributory pre-commitment and the capture key (design §7.5.2.6).
//! - [`store`] is the sealed capture store (design §7.5.2) and the client's
//!   persistent state, kept so a test can look at what a compliant client
//!   holds and does not hold.
//!
//! Every decision here is the client's own, taken against what the client
//! holds.  The device — camera, proximity channels, clock, the person — is
//! behind an interface, so the same client runs on a harness.

pub mod keys;
pub mod store;

pub use rhtn_archive::{Keyhash, Txid};
