//! What one observer holds, and where the conformance test gets it from.
//!
//! An evaluator's graph is over pairs (design §16.2.1): three sources of
//! standing — adoption, proof of presence, peering — enter as one edge per
//! pair, never one per relationship, and archive transactions are not
//! edges at all.  Every observer's evidence is its own (design §16.1), and
//! how far outward it reaches is available evidence rather than a protocol
//! quantity (design §16.2.1).

use crate::landscape::{HORIZON, Scope};
use std::collections::BTreeSet;
use std::fmt::Debug;

/// The evidence one observer evaluates from.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Evidence<N: Ord + Clone> {
    pub observer: N,
    /// `(patron, node)`: the routing and authority hierarchy.  Scope is
    /// built from these and from the sibling edges they imply.
    pub adoptions: BTreeSet<(N, N)>,
    /// Unordered pairs joined by a presence or peering record: the
    /// acquaintance graph, orthogonal to the hierarchy, conferring no scope
    /// (design §6.3, §16.2.1).
    pub acquaintances: BTreeSet<(N, N)>,
}

fn unordered<N: Ord + Clone>(a: N, b: N) -> (N, N) {
    if a <= b { (a, b) } else { (b, a) }
}

impl<N: Ord + Clone + Debug> Evidence<N> {
    pub fn new(observer: N) -> Self {
        Evidence { observer, adoptions: BTreeSet::new(), acquaintances: BTreeSet::new() }
    }

    /// An adoption of `node` under `patron` this observer holds.
    pub fn adopt(&mut self, patron: N, node: N) {
        self.adoptions.insert((patron, node));
    }

    /// A presence or peering record joining `a` and `b` this observer holds.
    pub fn meet(&mut self, a: N, b: N) {
        self.acquaintances.insert(unordered(a, b));
    }

    /// Scope adjacency from the adoptions held (design §15.1).
    pub fn scope(&self) -> Scope<N> {
        let mut s = Scope::from_adoptions(&self.adoptions);
        s.add_node(self.observer.clone());
        s
    }

    /// This observer's horizon: the two-edge patron/sibling ball.
    pub fn horizon(&self) -> BTreeSet<N> {
        self.scope().horizon(&self.observer, HORIZON)
    }

    /// Every identity the evidence names, the observer included.  What the
    /// observer knows is what it can weigh (design §16.1).
    pub fn known(&self) -> BTreeSet<N> {
        let mut out = BTreeSet::from([self.observer.clone()]);
        for (a, b) in self.adoptions.iter().chain(&self.acquaintances) {
            out.insert(a.clone());
            out.insert(b.clone());
        }
        out
    }

    pub fn knows(&self, n: &N) -> bool {
        self.known().contains(n)
    }

    /// The pairs of the evaluator's graph: adoptions and acquaintances
    /// collapsed to one unordered pair each (design §16.2.1).
    pub fn pairs(&self) -> BTreeSet<(N, N)> {
        self.adoptions.iter().map(|(p, c)| unordered(p.clone(), c.clone())).chain(self.acquaintances.iter().cloned()).collect()
    }
}

/// An omniscient topology, for the conformance test: every adoption and
/// every peering that exists.  No node holds this; each observer's evidence
/// is derived from it by the visibility rules, which is what makes the
/// bound observer-relative (design §16.3.1).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct World<N: Ord + Clone> {
    pub adoptions: Vec<(N, N)>,
    pub peerings: Vec<(N, N)>,
}

impl<N: Ord + Clone + Debug> World<N> {
    pub fn new() -> Self {
        World { adoptions: Vec::new(), peerings: Vec::new() }
    }

    pub fn adopt(&mut self, patron: N, node: N) {
        self.adoptions.push((patron, node));
    }

    pub fn peer(&mut self, a: N, b: N) {
        self.peerings.push((a, b));
    }

    pub fn scope(&self) -> Scope<N> {
        Scope::from_adoptions(&self.adoptions)
    }

    pub fn horizon(&self, observer: &N) -> BTreeSet<N> {
        self.scope().horizon(observer, HORIZON)
    }

    pub fn children(&self, patron: &N) -> Vec<N> {
        self.adoptions.iter().filter(|(p, _)| p == patron).map(|(_, c)| c.clone()).collect()
    }

    pub fn nodes(&self) -> BTreeSet<N> {
        self.adoptions.iter().chain(&self.peerings).flat_map(|(a, b)| [a.clone(), b.clone()]).collect()
    }

    /// The evidence `observer` holds: the floor plus `reach` shells.
    ///
    /// The floor is what design §15.1 stores and §16.3 makes visible —
    /// adoption edges with both ends in the observer's horizon, and every
    /// peering record with an endpoint in it, whose far endpoint enters the
    /// graph with it.  Each further shell adds the patronage structure
    /// around the nodes the previous shell brought in, and nothing else:
    /// what locator data exposes is patronage, and a peering record one
    /// hop past a visible one stays invisible at every reach.  Learning of
    /// a node confers no sight of the records around it (design §16.2.1,
    /// §16.3.1).
    pub fn view(&self, observer: &N, reach: usize) -> Evidence<N> {
        let hz = self.horizon(observer);
        let mut ev = Evidence::new(observer.clone());
        for (p, c) in &self.adoptions {
            if hz.contains(p) && hz.contains(c) {
                ev.adopt(p.clone(), c.clone());
            }
        }
        for (a, b) in &self.peerings {
            if hz.contains(a) || hz.contains(b) {
                ev.meet(a.clone(), b.clone());
            }
        }
        let mut expanded = hz;
        for _ in 0..reach {
            let frontier: Vec<N> = ev.known().into_iter().filter(|n| !expanded.contains(n)).collect();
            for u in frontier {
                expanded.insert(u.clone());
                for (p, c) in &self.adoptions {
                    if *p == u || *c == u {
                        ev.adopt(p.clone(), c.clone());
                    }
                }
            }
        }
        ev
    }

    /// An evaluator that has populated everything: every adoption and every
    /// peering, as evidence.  A richer graph is a better-populated input,
    /// never a different rule (design §16.2.1); this is the strongest test
    /// of a bound, since invisibility only removes capacity.
    pub fn full_view(&self, observer: &N) -> Evidence<N> {
        let mut ev = Evidence::new(observer.clone());
        for (p, c) in &self.adoptions {
            ev.adopt(p.clone(), c.clone());
        }
        for (a, b) in &self.peerings {
            ev.meet(a.clone(), b.clone());
        }
        ev
    }
}
