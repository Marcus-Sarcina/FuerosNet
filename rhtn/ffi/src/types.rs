//! What crosses the boundary.
//!
//! A generated binding carries records, enums and byte strings; it does not
//! carry Rust's lifetimes, traits or errors. The types here are the
//! translation, and the rule for each is that it loses nothing a shell
//! needs to render and adds nothing the library did not say.
//!
//! What it owes:
//!
//! - **Keyhashes and txids as their 32 bytes.** No document fixes a display
//!   form for either, so how one is shown to a person is the shell's, and
//!   the two shells must agree: the same identity rendered two ways reads
//!   as two identities to the person comparing them aloud.
//! - **Verdicts, bases and channel outcomes as their own enums**, so a
//!   shell cannot render a value the protocol has no name for.
//! - **Aborts and refusals as values with their reason**, since every one
//!   of them is something the person is owed an explanation for.
//! - **Notices as a closed set** (design §19.6): a shell that meets a
//!   notice it does not know must still show something, so the set is
//!   versioned rather than open.
