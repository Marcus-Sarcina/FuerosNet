//! Ask a running node the questions that change nothing.
//!
//! Attach, then the read-only request types: resolve a locator
//! (`wire-format.md` §7.7), fetch an archive (§7.9), query a catalog
//! (§6.4). These are the three the wire calls read-only, answering the same
//! way however often they are replayed (`wire-format.md` §9.2).
//!
//! What it owes:
//!
//! - **Send nothing that changes state or spends a budget.** A prekey fetch
//!   consumes a one-time key and a resource request has an application
//!   effect; neither belongs behind a command whose purpose is to look.
//! - **Report the answer as it arrived**, including a refusal and its code.
//! - **Authenticate as a real identity and pin the node it dials.** There
//!   is no unauthenticated mode to add: the transport has none
//!   (`wire-format.md` §9.1).
