//! The participant client above the session (`Robot/implementation-plan.md`,
//! crate `rhtn-client`; `light-client-requirements.md` §1; design §7, §8).
//!
//! - [`keys`] is the two constructions a ceremony fixes to the byte: the
//!   contributory pre-commitment and the capture key (design §7.5.2.6).
//! - [`store`] is the sealed capture store (design §7.5.2) and the client's
//!   persistent state, kept so a test can look at what a compliant client
//!   holds and does not hold.
//! - [`query`] is the verification query, the consent, the response and
//!   the key grant as bytes (`wire-format.md` §5.6, §7.4, §8.4).
//! - [`selection`] is how a participant picks whom to ask (design §7.4).
//! - [`verifier`] is what a client does when asked (design §7.4.1,
//!   §7.5.2.4): the comparison in memory and nothing kept of it.
//! - [`subject`] is the other side: consent, the per-ceremony counters,
//!   the key grant against the most recent eligible capture, and the
//!   responses held for the finalization veto.
//! - [`record`] is record assembly: the body every signer sees, the
//!   disclosure set and its root, a signer's refusals, the presentation
//!   that withholds by default, and late responses kept beside a record.
//! - [`rotation`] is what a subject does with its own lines: seal on
//!   rotation, seal then reissue on suspicion, never into a series it has
//!   occupied, and keep the chain that proves its series.
//! - [`payload`] is payload confidentiality: the prekey material published
//!   and stocked, the prefetch and the one-time request, the session opened
//!   on PQXDH, and the channel's framing.
//! - [`ratchet`] is the Double Ratchet the session runs.
//! - [`catalog`] is the client's catalog view, swept and cached, and the
//!   brokered service matched against its signed entry.
//! - [`notice`] is what the person is told, raised through one hook.
//! - [`device`] is the hardware and the person behind traits: proximity
//!   channels, the camera, the clock, randomness, the operator, the engine.
//! - [`ceremony`] is the ceremony itself (design §7.1): each party's steps
//!   and refusals, and an in-process harness that carries the direct
//!   channel and logs every path.
//! - [`conversation`] is that conversation as bytes (`wire-format.md`
//!   §7.10.2): the structures the participants and their witnesses send
//!   one another on the end-to-end channel after the local exchanges.
//! - [`sequence`] is that conversation driven over the end-to-end path
//!   (`wire-format.md` §7.10.1): the steps a participant takes, what each
//!   arriving kind is answered with, and the witness's side of it.
//!
//! - [`diag`] is the field-test diagnostics' helpers: the redacting
//!   constructors every `tracing` event's fields are built from, and the
//!   JSON renderer of the field-test flavour. The hooks are compiled out of
//!   the releasable flavour, which is the default.
//!
//! Every decision here is the client's own, taken against what the client
//! holds.  The device — camera, proximity channels, clock, the person — is
//! behind an interface, so the same client runs on a harness.

// Every public item carries its own documentation: the source is the
// fourth reading of the design, and a bare name is not one.
#![warn(missing_docs)]
// Two flavours of one build, never both: `releasable` compiles every hook
// out, `fieldtest` keeps them and renders them.  Built together they would
// render nothing and claim to be a field build.
#[cfg(all(feature = "releasable", feature = "fieldtest"))]
compile_error!(
    "rhtn-client: `releasable` and `fieldtest` are two flavours of one build; \
     build the field-test flavour with `--no-default-features --features fieldtest`"
);

pub mod backup;
pub mod catalog;
pub mod ceremony;
pub mod conversation;
pub mod device;
pub mod diag;
pub mod durable;
pub mod horizon;
pub mod keys;
pub mod local;
pub mod notice;
pub mod payload;
pub mod query;
pub mod ratchet;
pub mod record;
pub mod rotation;
pub mod sealed;
pub mod selection;
pub mod sequence;
pub mod store;
pub mod subject;
pub mod trust;
pub mod verifier;

pub use rhtn_archive::{Keyhash, Txid};
