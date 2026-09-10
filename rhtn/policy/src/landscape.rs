//! The trust landscape (design §16.2.1): scope and the horizon, the
//! distance function with the horizon collapsed to one step, node
//! throughput by distance, and the reference allocation rule (design
//! §16.4).
//!
//! Two functions are kept apart because the design keeps them apart:
//! whether a node is rationed at all is inside-or-beyond the horizon, and
//! how much is its distance.  Inside the horizon the metric is not what is
//! being asked (design §16.2.1); beyond it, the metric is all that is.

use crate::flow::{FlowGraph, Network, UNTHROTTLED};
use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::fmt::Debug;

/// Scope adjacency: adoption edges plus sibling edges, the graph the
/// horizon is walked over (design §15.1).  A sibling edge is an abstraction
/// of graph distance in the patronage hierarchy and carries no capacity
/// (design §16.2.1); peering edges are absent because peering confers no
/// scope (design §6.3).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Scope<N: Ord + Clone> {
    adj: BTreeMap<N, BTreeSet<N>>,
}

impl<N: Ord + Clone> Scope<N> {
    pub fn new() -> Self {
        Scope { adj: BTreeMap::new() }
    }

    /// From `(patron, node)` pairs: an adoption edge for each, and a sibling
    /// edge between every two subordinates of one patron.
    pub fn from_adoptions<'a>(pairs: impl IntoIterator<Item = &'a (N, N)>) -> Self
    where
        N: 'a,
    {
        let mut s = Scope::new();
        let mut children: BTreeMap<&N, Vec<&N>> = BTreeMap::new();
        for (p, c) in pairs {
            s.link(p, c);
            children.entry(p).or_default().push(c);
        }
        for kids in children.values() {
            for a in kids {
                for b in kids {
                    if a != b {
                        s.link(a, b);
                    }
                }
            }
        }
        s
    }

    pub fn add_node(&mut self, n: N) {
        self.adj.entry(n).or_default();
    }

    pub fn link(&mut self, a: &N, b: &N) {
        self.adj.entry(a.clone()).or_default().insert(b.clone());
        self.adj.entry(b.clone()).or_default().insert(a.clone());
    }

    pub fn contains(&self, n: &N) -> bool {
        self.adj.contains_key(n)
    }

    pub fn nodes(&self) -> impl Iterator<Item = &N> {
        self.adj.keys()
    }

    pub fn neighbours(&self, n: &N) -> impl Iterator<Item = &N> {
        self.adj.get(n).into_iter().flatten()
    }

    /// Every node within `radius` scope edges of `observer`, the observer
    /// included (design §15.1).  Every observer's horizon is its own.
    pub fn horizon(&self, observer: &N, radius: usize) -> BTreeSet<N> {
        let mut seen = BTreeSet::from([observer.clone()]);
        let mut frontier = seen.clone();
        for _ in 0..radius {
            let mut next = BTreeSet::new();
            for v in &frontier {
                next.extend(self.neighbours(v).cloned());
            }
            frontier = next.difference(&seen).cloned().collect();
            seen.extend(frontier.iter().cloned());
        }
        seen
    }
}

/// design §15.1's horizon: the two-edge walk.
pub const HORIZON: usize = 2;

/// Per-node throughput by landscape distance.  The observer and everything
/// inside its horizon are unthrottled — the metric does not ration there
/// (design §16.2.1) — and beyond it a halving schedule: 16, 8, 4, 2, 1, 1…
/// The schedule is the reference policy's choice; the design fixes no
/// vertex-capacity schedule, since policy is local (design §16.2, §16.4).
/// What is not a choice is the horizon being unthrottled.
pub fn node_capacity(dist: usize, in_horizon: bool) -> u64 {
    if dist == 0 || in_horizon { UNTHROTTLED } else { (32u64 >> dist.min(63)).max(1) }
}

/// Distance in the trust landscape (design §16.2.1), not hops in the graph:
/// the observer alone is 0; its whole horizon is 1, collapsed into one
/// step, together with anyone the observer has met itself; a node outside
/// the horizon one edge from someone at 1 is at 2, and so on outward,
/// every edge alike.  The walk uses the observer's own graph and nothing
/// wider: an edge absent from the capacity graph shortens no distance,
/// or design §16.3.1's bound would not be observer-relative.
pub fn landscape_distance<N: Ord + Clone + Debug>(scope: &Scope<N>, flow: &FlowGraph<N>, observer: &N) -> BTreeMap<N, usize> {
    let mut dist: BTreeMap<N, usize> = scope.horizon(observer, HORIZON).into_iter().map(|n| (n, 1)).collect();
    // The observer's own counterparties are at 1 whether or not they are in
    // the horizon: an edge incident to the observer is one it is party to.
    for (v, _) in flow.edges_from(observer) {
        dist.entry(v.clone()).or_insert(1);
    }
    dist.insert(observer.clone(), 0);
    let acq = flow.adjacency();
    let mut frontier: BTreeSet<N> = dist.iter().filter(|(_, d)| **d == 1).map(|(n, _)| n.clone()).collect();
    let mut d = 1;
    while !frontier.is_empty() {
        d += 1;
        let mut next = BTreeSet::new();
        for u in &frontier {
            for v in acq.get(u).into_iter().flatten() {
                if !dist.contains_key(v) {
                    dist.insert(v.clone(), d);
                    next.insert(v.clone());
                }
            }
        }
        frontier = next;
    }
    dist
}

/// Hops from `source` over positive-capacity edges, directed.
pub fn hops_from<N: Ord + Clone + Debug>(g: &FlowGraph<N>, source: &N) -> BTreeMap<N, usize> {
    let mut dist = BTreeMap::from([(source.clone(), 0)]);
    let mut queue = VecDeque::from([source.clone()]);
    while let Some(u) = queue.pop_front() {
        let du = dist[&u];
        for (v, _) in g.edges_from(&u) {
            if !dist.contains_key(v) {
                dist.insert(v.clone(), du + 1);
                queue.push_back(v.clone());
            }
        }
    }
    dist
}

/// The split graph: each node an in-vertex and an out-vertex joined by one
/// edge carrying its throughput, the observer joined to every horizon
/// member by one unthrottled edge, and one shared sink for an allocation's
/// drains.  Built once per computation; `network` is cloned for each
/// individual flow.
#[derive(Clone, Debug)]
pub struct Split<N: Ord + Clone> {
    pub network: Network,
    index: BTreeMap<N, usize>,
    pub sink: usize,
    pub source: usize,
}

impl<N: Ord + Clone + Debug> Split<N> {
    /// The in-vertex of `n`: where flow to `n` as a destination ends.
    pub fn inn(&self, n: &N) -> Option<usize> {
        self.index.get(n).map(|i| 2 * i)
    }

    /// The out-vertex of `n`: what `n` may relay is what passes it.
    pub fn out(&self, n: &N) -> Option<usize> {
        self.index.get(n).map(|i| 2 * i + 1)
    }

    /// The maximum flow from the observer to `target`, on a fresh copy.
    pub fn flow_to(&self, target: &N) -> u64 {
        match self.inn(target) {
            Some(t) => self.network.clone().max_flow(self.source, t),
            None => 0,
        }
    }
}

/// Node splitting with distance-based throughput, and the horizon collapsed
/// to a single edge (design §16.2.1).  With a scope the distances are
/// landscape distances and the whole horizon is unthrottled; without one —
/// a bare fixture graph with no subnet structure — distances are plain hops
/// and only the observer is inside.
///
/// The observer holds one unthrottled edge to each horizon member and the
/// horizon's internal topology is not in the flow graph at all; edges
/// leaving the horizon keep their own capacity, which is where the metric
/// starts working.  Built any other way, an in-horizon edge decides the
/// bound for a region beyond the horizon, and hops re-grade the inside of
/// the horizon the design flattens.
pub fn split<N: Ord + Clone + Debug>(g: &FlowGraph<N>, observer: &N, scope: Option<&Scope<N>>) -> Split<N> {
    let (dist, hz) = match scope {
        Some(s) => (landscape_distance(s, g, observer), s.horizon(observer, HORIZON)),
        None => (hops_from(g, observer), BTreeSet::from([observer.clone()])),
    };
    // every node the observer can place, the observer and its horizon
    // included even where the capacity graph lacks them
    let mut names: BTreeSet<N> = g.nodes().filter(|u| dist.contains_key(u)).cloned().collect();
    names.extend(hz.iter().cloned());
    names.insert(observer.clone());
    let index: BTreeMap<N, usize> = names.iter().enumerate().map(|(i, n)| (n.clone(), i)).collect();
    let mut network = Network::new(2 * index.len() + 1);
    let sink = 2 * index.len();
    let (inn, out) = (|i: usize| 2 * i, |i: usize| 2 * i + 1);
    for (u, &i) in &index {
        let du = dist.get(u).copied().unwrap_or(1);
        network.add_edge(inn(i), out(i), node_capacity(du, hz.contains(u)));
        for (v, c) in g.edges_from(u) {
            let Some(&j) = index.get(v) else { continue };
            if hz.contains(u) && hz.contains(v) {
                continue; // collapsed away: the horizon is one step
            }
            network.add_edge(out(i), inn(j), c);
        }
    }
    let source = out(index[observer]);
    for m in &hz {
        if m != observer {
            network.add_edge(source, inn(index[m]), UNTHROTTLED);
        }
    }
    Split { network, index, sink, source }
}

/// A target's individual standing: the maximum flow from the observer to
/// it, which is the minimum cut between them.  The drain ends at the
/// target's in-vertex: a candidate is a destination, not a relay, and its
/// own throughput does not gate what it receives.
pub fn score<N: Ord + Clone + Debug>(g: &FlowGraph<N>, observer: &N, target: &N, scope: Option<&Scope<N>>) -> u64 {
    split(g, observer, scope).flow_to(target)
}

/// One candidate as the allocation ranked it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Ranked<N> {
    pub candidate: N,
    /// Available flow: the candidate's individual standing on the graph
    /// before any allocation.
    pub flow: u64,
    /// Hops in the collapsed graph, counted in nodes: the landscape
    /// distance (design §16.2.1).
    pub hops: Option<usize>,
    /// The index at which the caller considered the candidate.
    pub considered: usize,
}

/// What one conserving computation delivered to a set of candidates.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Allocation<N> {
    /// Every candidate present in the graph, in the order it was ranked.
    pub ranked: Vec<Ranked<N>>,
    /// The candidates that received flow, in admission order.
    pub admitted: Vec<N>,
    /// The total delivered: the true maximum, whatever the order.
    pub total: u64,
}

/// Admit candidates against one shared residual, in reference order
/// (design §16.4): available flow first, since it is what the metric
/// measures; the shorter path among candidates the flow ranks equally,
/// because a longer path is less trustworthy by nature; consideration
/// order for true ties only.  The keys are measured on the pre-allocation
/// graph, because a candidate's standing and position are properties of
/// the topology and not of who was served first.
///
/// Ranked explicitly rather than left to one multi-sink max-flow, which
/// delivers the path-length pass by accident and decides equal-length ties
/// by the order edges occupy in the adjacency structure (design §16.4's
/// implementation note).  Admitting in rank against a shared residual
/// cannot displace an earlier candidate, and the final total is still the
/// maximum, so the rule fixes only who is admitted.
pub fn admit<N: Ord + Clone + Debug>(g: &FlowGraph<N>, observer: &N, candidates: &[N], demand: u64, scope: Option<&Scope<N>>) -> Allocation<N> {
    let mut s = split(g, observer, scope);
    let hops = s.network.hops_from(s.source);
    let mut seen = BTreeSet::new();
    let mut ranked: Vec<Ranked<N>> = candidates
        .iter()
        .enumerate()
        .filter(|(_, c)| s.inn(c).is_some() && seen.insert((*c).clone()))
        .map(|(i, c)| Ranked { candidate: c.clone(), flow: s.flow_to(c), hops: hops[s.inn(c).unwrap()].map(|h| h.div_ceil(2)), considered: i })
        .collect();
    ranked.sort_by(|a, b| {
        b.flow
            .cmp(&a.flow)
            .then_with(|| a.hops.unwrap_or(usize::MAX).cmp(&b.hops.unwrap_or(usize::MAX)))
            .then_with(|| a.considered.cmp(&b.considered))
    });
    let mut admitted = Vec::new();
    let mut total = 0;
    for r in &ranked {
        let t = s.inn(&r.candidate).unwrap();
        s.network.add_edge(t, s.sink, demand);
        let gained = s.network.max_flow(s.source, s.sink);
        if gained > 0 {
            admitted.push(r.candidate.clone());
        }
        total += gained;
    }
    Allocation { ranked, admitted, total }
}

/// The flow deliverable to a set at once under arbitrary demands: design
/// §16.2's setwise conservation in its general form.  Passing each
/// candidate's own individual standing is the strongest reading, every
/// identity asking for everything it could get alone.
pub fn deliverable<N: Ord + Clone + Debug>(g: &FlowGraph<N>, observer: &N, demands: &[(N, u64)], scope: Option<&Scope<N>>) -> u64 {
    let mut s = split(g, observer, scope);
    let mut seen = BTreeSet::new();
    for (t, d) in demands {
        if let Some(i) = s.inn(t)
            && *d > 0
            && seen.insert(t.clone())
        {
            s.network.add_edge(i, s.sink, *d);
        }
    }
    s.network.max_flow(s.source, s.sink)
}

/// design §16.2's sentence made a number: everything behind `entry` drains
/// through its out-vertex, so no set of identities there can use more at
/// once than flows from the observer to that vertex.
pub fn ceiling<N: Ord + Clone + Debug>(g: &FlowGraph<N>, observer: &N, entry: &N, scope: Option<&Scope<N>>) -> u64 {
    let mut s = split(g, observer, scope);
    match s.out(entry) {
        Some(t) => s.network.max_flow(s.source, t),
        None => 0,
    }
}
