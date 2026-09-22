//! A capacitated graph and its maximum flow.  The reference metric is
//! max-flow / min-cut (design §16.2): every standing it reports is a flow
//! value on a graph `landscape` builds, and the min-cut is what bounds a
//! region however many identities it holds.
//!
//! Two representations: [`FlowGraph`] is the graph over named pairs an
//! evaluator builds, collapsing a repeated pair; [`Network`] is the dense
//! residual network the flow itself runs on.

use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::fmt::Debug;

/// Stands in for "the flow metric does not ration here" (design §16.2.1):
/// the observer's own throughput, and every node inside its horizon.
pub const UNTHROTTLED: u64 = 1_000_000_000;

/// A directed graph with integer capacities over named nodes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FlowGraph<N: Ord + Clone> {
    cap: BTreeMap<N, BTreeMap<N, u64>>,
}

impl<N: Ord + Clone> Default for FlowGraph<N> {
    fn default() -> Self {
        FlowGraph {
            cap: BTreeMap::new(),
        }
    }
}

impl<N: Ord + Clone + Debug> FlowGraph<N> {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add_node(&mut self, u: N) {
        self.cap.entry(u).or_default();
    }

    /// Set the capacity of `u -> v`, collapsing a repeated pair rather than
    /// summing it.  design §16.2.1: "capacity belongs to the pair, not to
    /// the count of relationships between them" — a second meeting with the
    /// same party tells an evaluator what the first one did, and a metric
    /// that paid for it would pay for repetition rather than reach.  A pair
    /// offered two different capacities is a construction error, and is
    /// asserted as one: an edge is worth what an edge at its distance is
    /// worth, whatever relationship produced it.
    pub fn add_edge(&mut self, u: N, v: N, c: u64) {
        self.add_node(u.clone());
        self.add_node(v.clone());
        let existing = self.cap[&u].get(&v).copied().unwrap_or(0);
        assert!(
            existing == 0 || existing == c,
            "{u:?} -> {v:?} offered two capacities, {existing} and {c}: one pair carries one edge (design §16.2.1)"
        );
        self.cap.get_mut(&u).unwrap().insert(v.clone(), c);
    }

    /// One undirected pair at capacity `c`: an edge each way.
    pub fn join(&mut self, u: N, v: N, c: u64) {
        self.add_edge(u.clone(), v.clone(), c);
        self.add_edge(v, u, c);
    }

    pub fn has(&self, u: &N) -> bool {
        self.cap.contains_key(u)
    }

    pub fn capacity(&self, u: &N, v: &N) -> u64 {
        self.cap.get(u).and_then(|m| m.get(v)).copied().unwrap_or(0)
    }

    pub fn nodes(&self) -> impl Iterator<Item = &N> {
        self.cap.keys()
    }

    pub fn len(&self) -> usize {
        self.cap.len()
    }

    pub fn is_empty(&self) -> bool {
        self.cap.is_empty()
    }

    /// The positive-capacity edges out of `u`.
    pub fn edges_from(&self, u: &N) -> impl Iterator<Item = (&N, u64)> {
        self.cap
            .get(u)
            .into_iter()
            .flat_map(|m| m.iter().filter(|(_, c)| **c > 0).map(|(v, c)| (v, *c)))
    }

    /// The undirected adjacency over positive-capacity edges.
    pub fn adjacency(&self) -> BTreeMap<N, BTreeSet<N>> {
        let mut acq: BTreeMap<N, BTreeSet<N>> = self
            .cap
            .keys()
            .map(|u| (u.clone(), BTreeSet::new()))
            .collect();
        for (u, nbrs) in &self.cap {
            for (v, c) in nbrs {
                if *c > 0 {
                    acq.get_mut(u).unwrap().insert(v.clone());
                    acq.entry(v.clone()).or_default().insert(u.clone());
                }
            }
        }
        acq
    }

    /// The dense network of this graph, and each node's index in it.
    pub fn network(&self) -> (Network, BTreeMap<N, usize>) {
        let index: BTreeMap<N, usize> = self
            .cap
            .keys()
            .enumerate()
            .map(|(i, n)| (n.clone(), i))
            .collect();
        let mut net = Network::new(index.len());
        for (u, nbrs) in &self.cap {
            for (v, c) in nbrs {
                if *c > 0 {
                    net.add_edge(index[u], index[v], *c);
                }
            }
        }
        (net, index)
    }

    /// The maximum flow from `source` to `sink`, which is the minimum cut
    /// between them — the whole reason design §16.2 chose the metric.
    pub fn max_flow(&self, source: &N, sink: &N) -> u64 {
        let (mut net, index) = self.network();
        match (index.get(source), index.get(sink)) {
            (Some(s), Some(t)) => net.max_flow(*s, *t),
            _ => 0,
        }
    }
}

/// A residual network over dense vertex indices.  Edges come in pairs:
/// edge `2k` is the edge as added and `2k + 1` its reverse, which starts
/// empty and carries the option to undo flow later.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Network {
    to: Vec<usize>,
    cap: Vec<u64>,
    adj: Vec<Vec<usize>>,
}

impl Network {
    pub fn new(vertices: usize) -> Self {
        Network {
            to: Vec::new(),
            cap: Vec::new(),
            adj: vec![Vec::new(); vertices],
        }
    }

    pub fn add_vertex(&mut self) -> usize {
        self.adj.push(Vec::new());
        self.adj.len() - 1
    }

    pub fn vertices(&self) -> usize {
        self.adj.len()
    }

    /// A directed edge `u -> v` of capacity `c`; returns its index.
    pub fn add_edge(&mut self, u: usize, v: usize, c: u64) -> usize {
        let e = self.to.len();
        self.to.push(v);
        self.cap.push(c);
        self.adj[u].push(e);
        self.to.push(u);
        self.cap.push(0);
        self.adj[v].push(e + 1);
        e
    }

    /// The remaining capacity of edge `e`.
    pub fn residual(&self, e: usize) -> u64 {
        self.cap[e]
    }

    /// Hops from `source` over positive-residual edges.
    pub fn hops_from(&self, source: usize) -> Vec<Option<usize>> {
        let mut dist = vec![None; self.adj.len()];
        dist[source] = Some(0);
        let mut queue = VecDeque::from([source]);
        while let Some(u) = queue.pop_front() {
            let du = dist[u].unwrap();
            for &e in &self.adj[u] {
                let v = self.to[e];
                if self.cap[e] > 0 && dist[v].is_none() {
                    dist[v] = Some(du + 1);
                    queue.push_back(v);
                }
            }
        }
        dist
    }

    /// Edmonds-Karp: augment along the shortest path until none remains.
    /// Leaves the residual behind, which is what lets one computation share
    /// capacity across candidates (design §16.2).
    pub fn max_flow(&mut self, source: usize, sink: usize) -> u64 {
        let mut total = 0;
        let mut parent: Vec<Option<usize>> = vec![None; self.adj.len()];
        loop {
            parent.iter_mut().for_each(|p| *p = None);
            let mut seen = vec![false; self.adj.len()];
            seen[source] = true;
            let mut queue = VecDeque::from([source]);
            'bfs: while let Some(u) = queue.pop_front() {
                for &e in &self.adj[u] {
                    let v = self.to[e];
                    if self.cap[e] > 0 && !seen[v] {
                        seen[v] = true;
                        parent[v] = Some(e);
                        if v == sink {
                            break 'bfs;
                        }
                        queue.push_back(v);
                    }
                }
            }
            if !seen[sink] {
                return total;
            }
            let mut bottleneck = u64::MAX;
            let mut v = sink;
            while let Some(e) = parent[v] {
                bottleneck = bottleneck.min(self.cap[e]);
                v = self.to[e ^ 1];
            }
            let mut v = sink;
            while let Some(e) = parent[v] {
                self.cap[e] -= bottleneck;
                self.cap[e ^ 1] += bottleneck;
                v = self.to[e ^ 1];
            }
            total += bottleneck;
        }
    }
}
