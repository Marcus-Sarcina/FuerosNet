//! The daemon's lifecycle: start a node from a configuration, serve until
//! signalled, and stop without losing what was accepted.
//!
//! What it owes:
//!
//! - **Read the identity, and refuse to start without one.** A missing key
//!   file is a failure to report, never a key to mint.
//! - **Load the queue's directory store before serving**, so a restart
//!   redelivers what was accepted and nothing else
//!   (`infra-client-requirements.md` §2).
//! - **Load the prekey service's saved pools**, which are consumable state
//!   and must not be reissued after a restart (`wire-format.md` §7.8).
//! - **Start the node on the configured address** and attach upstream where
//!   the configuration names a patron (design §14.1.2).
//! - **Stop on SIGINT and SIGTERM**: refuse new sessions, let deliveries in
//!   flight finish, and persist the queue and the pools before exit.  A
//!   delivery half-made leaves its message where it was, which the store
//!   contract already guarantees; the shutdown must not defeat it.
//!
//! It owes no wire behaviour. Everything a peer observes is decided in
//! `rhtn-node`, `rhtn-transport` and the crates beneath them.
