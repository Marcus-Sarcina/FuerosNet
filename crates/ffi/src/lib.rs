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
//! **`uniffi` is the binding generator.** The facade stays plain Rust — the
//! scaffolding reads it rather than shaping it — and the Kotlin the Android
//! shell speaks is generated from the built library by
//! `mobile/android/tools/build-native.sh`.
#![warn(missing_docs)]

uniffi::setup_scaffolding!();

// Two flavours of one build, never both (see `rhtn-client`'s manifest).
#[cfg(all(feature = "releasable", feature = "fieldtest"))]
compile_error!(
    "rhtn-ffi: `releasable` and `fieldtest` are two flavours of one build; \
     build the field-test flavour with `--no-default-features --features fieldtest`"
);

/// What a shell asks of the kernel: every call the ceremony, the
/// conversation and the session need.
pub mod client;
/// What the kernel asks of the platform: the camera, the radios, the
/// clock, randomness, storage, custody and diagnostics.
pub mod device;
pub(crate) mod diag;
/// A node and identities in one process, for a shell's own tests.
#[cfg(feature = "harness")]
pub mod harness;
/// The session and the payload path behind the boundary, and the events
/// a shell reads from them.
pub mod net;
/// The types that cross the boundary, in the shapes UniFFI can carry.
pub mod types;
