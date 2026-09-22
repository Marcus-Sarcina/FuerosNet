//! `rhtnd`: the infrastructure daemon.
//!
//! A node is `rhtn-node`'s `LiveNode` and the crates below it; the daemon
//! is what an operator runs.  It owns the three things a library cannot:
//! the configuration a node cannot derive ([`config`]), the lifecycle from
//! start to signal to stop ([`service`]), the packages it was told to host
//! ([`hosting`]), and the disclosures an operator is owed about what their
//! configuration exposes ([`operator`]).
//!
//! It answers to `infra-client-requirements.md`, which states the operator
//! obligations, and adds no protocol of its own: everything on the wire is
//! decided in the crates beneath.

pub mod config;
pub mod hosting;
pub mod operator;
pub mod service;

/// A line to the operator on stderr.  **A closed pipe is not the daemon's
/// end**: the operator's terminal going away, or a harness that stopped
/// reading, is no reason for a serving node to stop, so a write that fails
/// is dropped, where `eprintln!` would panic the process.
#[macro_export]
macro_rules! say {
    ($($arg:tt)*) => {{
        use std::io::Write as _;
        let _ = writeln!(std::io::stderr(), $($arg)*);
    }};
}

/// A line to stdout, the same way.
#[macro_export]
macro_rules! tell {
    ($($arg:tt)*) => {{
        use std::io::Write as _;
        let _ = writeln!(std::io::stdout(), $($arg)*);
    }};
}
