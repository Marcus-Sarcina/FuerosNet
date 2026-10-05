//! The adaptors: `rhtn-client` bound to what is local to its process.
//!
//! A client reaches its device through an interface and says what it
//! wants carried as messages; the crates below it carry nothing.  Here
//! those messages meet a node and the transport.  The client runs on a
//! thread of its own ([`actor`]); what it asks of a serving node is
//! answered by the node beside it ([`serving`]); the direct payload path
//! is the transport's socket, the client's own or the node's
//! ([`direct`]); a query for a verifier hosted here is answered on the
//! request stream of the node hosting it ([`verifier`]); and the courier
//! carries what the client says out and what arrives in ([`courier`]).
//!
//! Two kinds of client are served, and the same adaptors serve both: a
//! light client beside its serving node, and a node that is a
//! participant, which is its own serving node.  What the documents leave
//! unwritten stays unwritten here: how a serving node carries a query to
//! a client attached over the wire (`wire-format.md` §7.7.2 stops at the
//! serving node), and how a client hands its serving node payload to
//! relay.  Those are seams, traits with the in-process implementation
//! behind them, so that what exists is exactly what the documents say.

#![warn(missing_docs)]

/// The client on a thread of its own, and the handle everything else
/// reaches it through.
pub mod actor;
/// A serving node beside this client in one process, for a node that is
/// its own serving node.
pub mod attached;
/// What carries the client's messages out and what arrives in.
pub mod courier;
/// The direct payload path over the transport's socket (design §12.6.3).
pub mod direct;
/// What the client asks of a serving node, answered by the node beside
/// it.
pub mod serving;
/// A query for a verifier hosted in this process, answered on the
/// request stream of the node hosting it.
pub mod verifier;
