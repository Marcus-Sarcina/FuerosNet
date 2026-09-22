//! `rhtn-transport` — the transport binding of `wire-format.md` §9 and the
//! stream-0 session of §8.
//!
//! QUIC with TLS 1.3 through quinn and rustls on the aws-lc-rs provider.
//! The key-exchange group is `X25519MLKEM768` and nothing else is offered
//! or accepted (§9.1); peers authenticate with RFC 7250 raw public keys
//! carrying the classical component of their identity, mutually; the ALPN
//! is `rhtn/1` (§9.2).  The dialling party pins the `KeyMaterial` of the
//! keyhash it intends to reach and abandons the handshake when the presented
//! key is not that material's classical member.
//!
//! Section references are to `wire-format.md` unless prefixed `design`.

pub mod bind;
pub mod queue;
pub mod session;
pub mod stun;
pub mod tls;
pub mod traversal;

pub use tls::{Pins, client_config, client_endpoint, server_config, server_endpoint};
