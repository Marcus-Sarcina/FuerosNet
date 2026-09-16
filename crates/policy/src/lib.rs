//! Trust policy (`Robot/implementation-plan.md`, crate `rhtn-policy`; design
//! §16, §17).
//!
//! Every node computes its own trust from the transactions it observes,
//! using the reference algorithm or a variant of its own (design §16.1).
//! Nothing here is shared state and nothing here is retained: a standing is
//! computed on demand from the evaluator's own graph and kept nowhere
//! (design §16.2).
//!
//! - [`flow`] is a capacitated graph and its maximum flow: the metric is
//!   max-flow / min-cut (design §16.2).
//! - [`landscape`] builds the graph the metric runs on (design §16.2.1):
//!   scope and horizon, the distance function, node throughput by distance,
//!   and the three-pass allocation rule (design §16.4).
//! - [`evidence`] is what one observer holds — adoption pairs and
//!   acquaintance pairs — and, for the conformance test, an omniscient
//!   topology from which each observer's evidence follows by the visibility
//!   rules (design §16.3, §16.3.1).
//! - [`policy`] is the interface a node consults, the reference metric
//!   behind it, and two substitutes: distance decay, and a policy that
//!   scores every known node alike.
//! - [`archive`] is the reference policy's reading of a presented archive
//!   (design §16.1, §16.2.1).
//! - [`series`] is the arithmetic of design §16.2: what a decay policy sums
//!   over a fake region, and whether that sum converges.
//! - [`conformance`] reports a policy's resistance bound over the
//!   simulation's topologies (design §16.4).
//!
//! The reference calculation is `models/simulation/flow_metric.py`; the
//! functions here carry its constructions across, and the tests carry its
//! regression cases and committed figures.

pub mod archive;
pub mod conformance;
pub mod evidence;
pub mod flow;
pub mod landscape;
pub mod policy;
pub mod series;

pub use evidence::{Evidence, World};
pub use flow::{FlowGraph, UNTHROTTLED};
pub use landscape::{Allocation, Scope};
pub use policy::{DistanceDecay, Evaluation, Policy, ReferenceMetric, Uniform};
pub use rhtn_archive::{Keyhash, Txid};
