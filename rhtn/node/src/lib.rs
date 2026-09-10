//! Node behaviour above the session (`Robot/implementation-plan.md`, crate
//! `rhtn-node`).
//!
//! - [`store`] is the topology store: what a node holds, by the storage rule
//!   `wire-format.md` §10.1.1 states, with §10.1.2's duplicate and conflict
//!   identities.
//! - [`propagation`] is the forwarding rule and the rootward memo
//!   (`wire-format.md` §10.1, §10.2; design §15).
//! - [`resolution`] is the anchor table and the resolution exchange
//!   (`wire-format.md` §7.2, §7.6, §7.7; design §12).
//! - [`currency`] is attestation issuance, its escalation ladder and the
//!   staple checks (design §12.6.5, §12.6.5.1; `wire-format.md` §7.1).
//! - [`peering`] is peering records and the direct payload path
//!   (design §6.3, §12.6.3, §12.7.5; `wire-format.md` §4.4).
//!
//! Every decision here is the node's own, taken against the node's own view.
//! Nothing in this crate consults a party it shares no state with.

pub mod currency;
pub mod peering;
pub mod propagation;
pub mod resolution;
pub mod runtime;
pub mod store;
pub mod trust;
pub mod view;

pub use rhtn_archive::{Keyhash, Txid};

/// What a node sends, and to whom: the sessions it holds by virtue of a
/// topology relationship (`wire-format.md` §10.1.1).
pub trait Adjacency {
    /// Every peer this node holds a session with.
    fn peers(&self) -> Vec<Keyhash>;
    /// Send one control frame, already framed, to `peer`.
    fn send(&self, peer: &Keyhash, frame_type: u64, body: &[u8]);
    /// Whether a session with `peer` exists right now.
    fn has_session(&self, peer: &Keyhash) -> bool {
        self.peers().contains(peer)
    }
}
