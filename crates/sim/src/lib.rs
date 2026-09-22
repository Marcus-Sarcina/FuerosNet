//! The simulation harness (`Robot/implementation-plan.md`, crate
//! `rhtn-sim`).
//!
//! [`path`] is a datagram-level path between two endpoints that can drop,
//! delay, blackhole and replay.  It sits below QUIC, so what it perturbs is
//! what a real network perturbs: packets, not frames.  That is what lets a
//! test blackhole a heartbeat path without reaching into the session layer,
//! and what lets it replay a client's first flight as an attacker would.
//!
//! [`scenario`] builds several nodes over loopback and scripts them.

pub mod daemons;
pub mod mesh;
pub mod nat;
pub mod packages;
pub mod participants;
pub mod path;
pub mod scenario;
