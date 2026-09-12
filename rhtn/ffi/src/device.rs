//! The platform's hardware, passed inward.
//!
//! `rhtn-client` reaches the world through interfaces it declares: the
//! proximity channels, the camera, the clock, randomness, the person, the
//! notifier and the biometric engine. On a phone each is the platform's,
//! so each arrives across the boundary as a callback the shell supplies.
//!
//! What it owes:
//!
//! - **Run the channels the hardware has, strongest first, and promote
//!   nothing** (`light-client-requirements.md` §1.3). A channel the device
//!   lacks is not listed; a channel that failed is reported as failed.
//! - **Strip the camera's metadata at the boundary** (§1.3). Whatever the
//!   platform's pipeline attaches, what crosses inward is pixels.
//! - **Keep the person's answer the person's.** The operator interface is
//!   the one place a client may ask a question, and only where the
//!   documents say to ask rather than tell.
//! - **Let the platform's clock be the clock.** A skew the shell corrects
//!   silently would move a witness's tolerance check without saying so
//!   (§1.2).
//!
//! The biometric engine is design §22.2's open item. The reference
//! compares hashes and recognises nobody, which is enough to run the
//! ceremony end to end and is not a face matcher.
