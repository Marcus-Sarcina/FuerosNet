//! The policy interface and three policies behind it.
//!
//! Each node computes its own trust from the transactions it observes,
//! using the reference algorithm or a variant tuned to its application or
//! group norms (design §16.1).  What a policy computes is its own: nothing
//! on the wire consumes another party's computation, no pass of the
//! reference metric is a protocol rule (design §16.4), and a node running a
//! substitute stores and forwards exactly what a node running the
//! reference does.  Standing is computed on demand and retained nowhere
//! (design §16.2).

use crate::evidence::Evidence;
use crate::flow::FlowGraph;
use crate::landscape::{self, Allocation, Scope, landscape_distance};
use std::collections::BTreeMap;
use std::fmt::Debug;

/// One computation over a set of candidates (design §16.2: the bound holds
/// for the set an evaluation scores, which is the only set whose standing
/// is simultaneously usable).
#[derive(Clone, Debug, PartialEq)]
pub struct Evaluation<N> {
    /// Each candidate's individual standing, in the order considered.
    pub individual: Vec<(N, f64)>,
    /// The candidates the policy admits together, in its own order.
    pub admitted: Vec<N>,
    /// What the set can use at once.  For a conserving policy this is
    /// bounded by the cut; for one that is not, it is whatever it sums to.
    pub joint: f64,
}

/// What a node consults.  A policy is given the evidence the node holds
/// and answers for a set in one computation; it sees nothing else and
/// changes nothing else.
pub trait Policy<N: Ord + Clone + Debug>: Send + Sync {
    fn name(&self) -> &'static str;

    /// Score `candidates` from the observer's evidence, in one computation.
    fn evaluate(&self, ev: &Evidence<N>, candidates: &[N]) -> Evaluation<N>;

    /// One target's individual standing.
    fn score(&self, ev: &Evidence<N>, target: &N) -> f64 {
        self.evaluate(ev, std::slice::from_ref(target))
            .individual
            .first()
            .map(|(_, s)| *s)
            .unwrap_or(0.0)
    }

    /// The per-hop decay a distance-decay policy runs with, where it is one;
    /// the conformance test reports the λ < 1/f criterion against it.
    fn decay(&self) -> Option<f64> {
        None
    }
}

/// The reference metric: max-flow / min-cut over the evaluator's own graph
/// (design §16.2), built as design §16.2.1 says — one edge per pair, the
/// horizon collapsed to one unthrottled step, throughput falling with
/// landscape distance beyond it — and allocating in three passes (design
/// §16.4).  Every pair's edge carries `edge_capacity`: beyond the horizon
/// trust flows equally over hierarchical and acquaintance edges, and what
/// an edge is worth at its distance is policy.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReferenceMetric {
    pub edge_capacity: u64,
}

impl Default for ReferenceMetric {
    fn default() -> Self {
        ReferenceMetric { edge_capacity: 10 }
    }
}

impl ReferenceMetric {
    /// The capacity graph over the evidence's pairs.
    pub fn graph<N: Ord + Clone + Debug>(&self, ev: &Evidence<N>) -> FlowGraph<N> {
        let mut g = FlowGraph::new();
        g.add_node(ev.observer.clone());
        for (a, b) in ev.pairs() {
            g.join(a, b, self.edge_capacity);
        }
        g
    }

    pub fn scope<N: Ord + Clone + Debug>(&self, ev: &Evidence<N>) -> Scope<N> {
        ev.scope()
    }

    /// Landscape distance of every node the evidence reaches.
    pub fn distance<N: Ord + Clone + Debug>(&self, ev: &Evidence<N>) -> BTreeMap<N, usize> {
        landscape_distance(&ev.scope(), &self.graph(ev), &ev.observer)
    }

    /// A target's individual standing.
    pub fn flow<N: Ord + Clone + Debug>(&self, ev: &Evidence<N>, target: &N) -> u64 {
        landscape::score(&self.graph(ev), &ev.observer, target, Some(&ev.scope()))
    }

    /// The reference allocation over a set, `demand` units each.
    pub fn admit<N: Ord + Clone + Debug>(
        &self,
        ev: &Evidence<N>,
        candidates: &[N],
        demand: u64,
    ) -> Allocation<N> {
        landscape::admit(
            &self.graph(ev),
            &ev.observer,
            candidates,
            demand,
            Some(&ev.scope()),
        )
    }

    /// The flow deliverable to a set at once under the demands given.
    pub fn deliverable<N: Ord + Clone + Debug>(
        &self,
        ev: &Evidence<N>,
        demands: &[(N, u64)],
    ) -> u64 {
        landscape::deliverable(&self.graph(ev), &ev.observer, demands, Some(&ev.scope()))
    }

    /// The cut at `entry`: what everything behind it can use at once.
    pub fn ceiling<N: Ord + Clone + Debug>(&self, ev: &Evidence<N>, entry: &N) -> u64 {
        landscape::ceiling(&self.graph(ev), &ev.observer, entry, Some(&ev.scope()))
    }
}

impl<N: Ord + Clone + Debug> Policy<N> for ReferenceMetric {
    fn name(&self) -> &'static str {
        "reference flow metric"
    }

    fn evaluate(&self, ev: &Evidence<N>, candidates: &[N]) -> Evaluation<N> {
        // **a patron's determination is what the neighbourhood defaults
        // to** (design §18.5): a party some patron this observer holds has
        // disavowed with prejudice is denied, without the observer
        // weighing that against a departure it may also hold.  This is the
        // reference reading; §16.4 leaves a tree free to run a variant
        // that orders the pair instead, and nothing on the wire can tell
        // which is running.
        //
        // **Denied before the allocation, not after it.** The three passes
        // (design §16.4) divide a cut among the candidates they are given,
        // so a denied party left in would take capacity an eligible one
        // could have used and `joint` would report a set that includes it
        // — an allocation, an admission and a usable total disagreeing
        // about the same set.
        let eligible: Vec<N> = candidates
            .iter()
            .filter(|c| !ev.blacklisted(c))
            .cloned()
            .collect();
        let alloc = self.admit(ev, &eligible, 1);
        let flows: BTreeMap<&N, u64> = alloc
            .ranked
            .iter()
            .map(|r| (&r.candidate, r.flow))
            .collect();
        Evaluation {
            individual: candidates
                .iter()
                .map(|c| (c.clone(), flows.get(c).copied().unwrap_or(0) as f64))
                .collect(),
            admitted: alloc.admitted,
            joint: alloc.total as f64,
        }
    }
}

/// A distance-decay policy: weight λ^distance, summed over a set.  The
/// policy design §16.2 warns against, kept so the conformance test can
/// show what it gives up: nothing conserves, so a region's standing grows
/// with its population.
#[derive(Clone, Debug, PartialEq)]
pub struct DistanceDecay {
    pub lambda: f64,
}

impl DistanceDecay {
    pub fn new(lambda: f64) -> Self {
        DistanceDecay { lambda }
    }
}

impl<N: Ord + Clone + Debug> Policy<N> for DistanceDecay {
    fn name(&self) -> &'static str {
        "distance decay"
    }

    fn evaluate(&self, ev: &Evidence<N>, candidates: &[N]) -> Evaluation<N> {
        let dist = ReferenceMetric::default().distance(ev);
        let individual: Vec<(N, f64)> = candidates
            .iter()
            .map(|c| {
                (
                    c.clone(),
                    dist.get(c)
                        .map(|d| self.lambda.powi(*d as i32))
                        .unwrap_or(0.0),
                )
            })
            .collect();
        let admitted = individual
            .iter()
            .filter(|(_, s)| *s > 0.0)
            .map(|(c, _)| c.clone())
            .collect();
        let joint = individual.iter().map(|(_, s)| s).sum();
        Evaluation {
            individual,
            admitted,
            joint,
        }
    }

    fn decay(&self) -> Option<f64> {
        Some(self.lambda)
    }
}

/// A policy that scores every known node alike: the trivial substitute,
/// which conforms because conformance carries no policy (design §16.4).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Uniform;

impl<N: Ord + Clone + Debug> Policy<N> for Uniform {
    fn name(&self) -> &'static str {
        "uniform"
    }

    fn evaluate(&self, ev: &Evidence<N>, candidates: &[N]) -> Evaluation<N> {
        let known = ev.known();
        let individual: Vec<(N, f64)> = candidates
            .iter()
            .map(|c| {
                (
                    c.clone(),
                    if known.contains(c) && *c != ev.observer {
                        1.0
                    } else {
                        0.0
                    },
                )
            })
            .collect();
        let admitted = individual
            .iter()
            .filter(|(_, s)| *s > 0.0)
            .map(|(c, _)| c.clone())
            .collect();
        let joint = individual.iter().map(|(_, s)| s).sum();
        Evaluation {
            individual,
            admitted,
            joint,
        }
    }
}
