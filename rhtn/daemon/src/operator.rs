//! The operator's view: what this node's configuration exposes, and to
//! whom.
//!
//! `infra-client-requirements.md` §8 obliges the daemon to tell an operator
//! plainly what their subordinates are exposed to by their configuration
//! and conduct.  §10.6 shows the hosting model when a resource is bound,
//! hosted against brokered, and §10.7 fixes what an accessing user sees:
//! the roles they hold, not the predicates that granted them.
//!
//! The catalogue carries all three as PRD-06, a manual entry, because the
//! obligation is discharged by a person reading a screen and not by a
//! value on the wire.
//!
//! What it owes:
//!
//! - **The binding view**: for each resource bound here, whether this node
//!   hosts it or brokers it, since the two expose the subordinate to
//!   different parties (`infra-client-requirements.md` §10.6).
//! - **The exposure view**: what this configuration creates for the
//!   identities below it — who can reach them, what is logged, and what
//!   leaves this node (`infra-client-requirements.md` §8, design §13).
//! - **The accessing user's view**: the roles a principal holds here,
//!   without the predicates behind them
//!   (`infra-client-requirements.md` §10.7).
//!
//! Where the view is rendered is not settled: a terminal on the host and a
//! page served to the operator alone are both open, and the choice belongs
//! to whoever runs one.  If it grows a frontend it leaves this crate, so
//! its dependencies stay out of the reference library's lockfile.
