//! The boundary the mobile shells bind to.
//!
//! The ceremony needs a camera, NFC and UWB
//! (`light-client-requirements.md` §1.3), which exist on a phone and not in
//! a Rust process, so the light client application is Kotlin and Swift over
//! this crate. Everything above the boundary is a platform shell;
//! everything below it is the reference library.
//!
//! **One crate, deliberately.** The application tier depends on this and
//! nothing else in the workspace, so the shells have one surface to track
//! and a later move of the applications to their own repository has one
//! seam to cut.
//!
//! **The traffic is one-way in each direction.** [`client`] carries what a
//! shell asks of the client; [`device`] carries the platform's hardware
//! inward, implementing the interfaces `rhtn-client` already declares;
//! [`types`] holds what crosses in either direction.
//!
//! **No decision is taken here.** A rule enforced at this boundary and not
//! below it would be absent from every other client, so the facade
//! translates and never adjudicates.
//!
//! The binding generator is not adopted: `uniffi` is the candidate and the
//! choice is the author's. Until it is made the facade is plain Rust, which
//! is what a generator would read anyway.

pub mod client;
pub mod device;
pub mod types;
