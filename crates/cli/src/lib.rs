//! `rhtn`: the developer command line.
//!
//! Three things, none of them a protocol participant's job: read what the
//! wire carries ([`inspect`]), make and examine identities ([`keys`]), and
//! ask a running node the questions that change nothing ([`probe`]).
//!
//! **It answers to no obligation document.** No section requires a command
//! line, and nothing here may become the only way to do something a
//! participant must do. Where a subcommand and a library disagree, the
//! library is right and the subcommand is the defect.
//!
//! **It reads and asks; it does not sign for anyone.** Minting an identity
//! is the exception, and what it mints is a key, not a position: a key
//! becomes a participant by being adopted, which takes two parties present
//! to each other and cannot be done from a terminal.

pub mod inspect;
pub mod keys;
pub mod probe;
