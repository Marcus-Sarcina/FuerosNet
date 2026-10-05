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

#![warn(missing_docs)]

/// The catalog a node answers from, and the abuse reports it holds for
/// an owner (`wire-format.md` §6; design §11.5, §11.6).
pub mod catalog;
/// Attestation issuance, its escalation ladder and the staple checks
/// (design §12.6.5, §12.6.5.1; `wire-format.md` §7.1).
pub mod currency;
/// The node's diagnostic events, compiled out of a releasable build.
pub mod diag;
/// The hosted-session surface a resource is reached through
/// (`resource-requirements.md` §2).
pub mod http;
/// Peering records and the direct payload path (design §6.3, §12.6.3,
/// §12.7.5; `wire-format.md` §4.4).
pub mod peering;
/// The prekey pool a node serves on a subject's behalf
/// (`wire-format.md` §7.8).
pub mod prekeys;
/// The forwarding rule and the rootward memo (`wire-format.md` §10.1,
/// §10.2; design §15).
pub mod propagation;
/// A directory-backed mailbox, for a node that keeps what it holds
/// across a restart.
pub mod queue;
/// The anchor table and the resolution exchange (`wire-format.md` §7.2,
/// §7.6, §7.7; design §12).
pub mod resolution;
/// Resources bound at this node: their rows, their packages and the
/// gateway in front of them (`resource-requirements.md`).
pub mod resources;
/// A node on a real endpoint: the sockets, the tasks and the limits.
pub mod runtime;
/// The topology store: what a node holds, by the rule
/// `wire-format.md` §10.1.1 states.
pub mod store;
/// What a client hands this node and what it answers
/// (`wire-format.md` §7.10).
pub mod submissions;
/// What a node computes about standing from what it holds (design §16).
pub mod trust;
/// A node's own view of where it sits and what it holds.
pub mod view;
/// Where to ring a client that is not attached (design §14.1.5).
pub mod wake;

pub use rhtn_archive::{Keyhash, Txid};

/// What a node sends, and to whom: the sessions it holds by virtue of a
/// topology relationship (`wire-format.md` §10.1.1).
pub trait Adjacency {
    /// Every peer this node holds a session with.
    fn peers(&self) -> Vec<Keyhash>;
    /// Send one control frame, already framed, to `peer` on stream 0.
    fn send(&self, peer: &Keyhash, frame_type: u64, body: &[u8]);
    /// Open a bidirectional request stream to `peer` (`wire-format.md`
    /// §9.2) and send one request on it; the reply reaches the node through
    /// its reply path, not here.  Whether it was sent: a session this node
    /// serves carries none, since a light client answers no request stream,
    /// and a peer without a session carries nothing.
    fn request(&self, peer: &Keyhash, request_type: u64, body: &[u8]) -> bool;
    /// Whether a session with `peer` exists right now.
    fn has_session(&self, peer: &Keyhash) -> bool {
        self.peers().contains(peer)
    }
}
