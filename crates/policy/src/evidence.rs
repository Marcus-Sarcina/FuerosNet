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
    /// Whose evidence this is. **Every standing computed from it is relative to
    /// this node** (design §16.2): there is no view from nowhere, and two
    /// observers of one network hold different sets.
    pub observer: N,
    /// `(patron, node)`: the routing and authority hierarchy.  Scope is
    /// built from these and from the sibling edges they imply.
    pub adoptions: BTreeSet<(N, N)>,
    /// Unordered pairs joined by a presence or peering record: the
    /// acquaintance graph, orthogonal to the hierarchy, conferring no scope
    /// (design §6.3, §16.2.1).
    pub acquaintances: BTreeSet<(N, N)>,
    /// `(patron, node)`: relationships a patron ended **with prejudice**
    /// (`wire-format.md` §4.3's banded reason codes), as this observer
    /// holds them.
    ///
    /// **The patron's determination is what the neighbourhood defaults
    /// to** (design §18.5): an observer holding one does not weigh it
    /// against a departure it may also hold, because ordering the two
    /// would have it adjudicate a question §1.1 gives it no standing to
    /// settle, on timestamps §2 says are signer-controlled. A tree wanting
    /// a timing rule runs a variant policy; nothing on the wire consumes
    /// either reading (design §16.4).
    pub disavowed: BTreeSet<(N, N)>,
    /// What this observer has seen each verifier *do*, one observation per
    /// response in a record it holds (design §7.4.3): whether the verifier
    /// answered or was `unavailable`, kept with the record's `finalized_at`
    /// so that the observer's own holding window is the decay — the tally
    /// is derived standing and falls away with the records that carry it,
    /// which are facts and do not (design §16.5).  `infra` is the
    /// observer's own knowledge of whether the verifier ran an instance,
    /// because silence from one weighs fully and from a light client
    /// lightly; the tally keeps them apart and weighs nothing itself.
    pub conduct: Vec<Observation<N>>,
}

/// One response a verifier gave, as this observer holds it.  A late reply
/// reaches the querier privately and is in no record, so it is never one
/// of these (design §7.4.3).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Observation<N> {
    /// The verifier that answered, or did not.
    pub verifier: N,
    /// When the record carrying it finalized, which is what puts the observation
    /// inside an observer's window or outside it.
    pub finalized_at: u64,
    /// Whether the response was `unavailable` (design §7.4.3).
    pub unavailable: bool,
    /// Whether the verifier runs infrastructure, which is what separates the two
    /// counts below: an infra node that will not answer is a different matter
    /// from a phone that was off.
    pub infra: bool,
}

/// A verifier's conduct over the records in an observer's window: what
/// enters the reliability channel and never the social one (design
/// §16.6).  Counts only — the magnitude any of it moves a score is the
/// observer's own policy, published nowhere (design §16.4).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Conduct {
    /// Responses that carried a verdict.
    pub answered: u32,
    /// `unavailable` from a verifier running infrastructure.
    pub unavailable_infra: u32,
    /// `unavailable` from a verifier that is not.
    pub unavailable_light: u32,
}

impl Conduct {
    /// Both unavailable counts together.
    pub fn unavailable(&self) -> u32 {
        self.unavailable_infra + self.unavailable_light
    }
    /// Every observation of this verifier in the window.
    pub fn total(&self) -> u32 {
        self.answered + self.unavailable()
    }
}

fn unordered<N: Ord + Clone>(a: N, b: N) -> (N, N) {
    if a <= b { (a, b) } else { (b, a) }
}

impl<N: Ord + Clone + Debug> Evidence<N> {
    /// Empty evidence for `observer`, which knows only itself.
    pub fn new(observer: N) -> Self {
        Evidence {
            observer,
            adoptions: BTreeSet::new(),
            acquaintances: BTreeSet::new(),
            disavowed: BTreeSet::new(),
            conduct: Vec::new(),
        }
    }

    /// A response this observer holds in a record finalized at `at`:
    /// `verifier` answered, or was unavailable, and ran an instance or did
    /// not as this observer knows it.
    pub fn observed(&mut self, verifier: N, at: u64, unavailable: bool, infra: bool) {
        self.conduct.push(Observation {
            verifier,
            finalized_at: at,
            unavailable,
            infra,
        });
    }

    /// `verifier`'s conduct over the records finalized at or after
    /// `since`: the observer's holding window, which is the whole of the
    /// decay (design §16.5).  An identity never observed has no conduct,
    /// which is a different thing from a bad one.
    pub fn conduct(&self, verifier: &N, since: u64) -> Conduct {
        let mut c = Conduct::default();
        for o in &self.conduct {
            if o.verifier != *verifier || o.finalized_at < since {
                continue;
            }
            match (o.unavailable, o.infra) {
                (false, _) => c.answered += 1,
                (true, true) => c.unavailable_infra += 1,
                (true, false) => c.unavailable_light += 1,
            }
        }
        c
    }

    /// An adoption of `node` under `patron` this observer holds.
    pub fn adopt(&mut self, patron: N, node: N) {
        self.adoptions.insert((patron, node));
    }

    /// A presence or peering record joining `a` and `b` this observer holds.
    pub fn meet(&mut self, a: N, b: N) {
        self.acquaintances.insert(unordered(a, b));
    }

    /// A relationship `patron` ended with prejudice, as this observer
    /// holds it.  Only the banded codes: an unbanded or unstated reason
    /// alleges nothing (`wire-format.md` §4.3), and ending a relationship
    /// is not itself an accusation.
    pub fn disavow(&mut self, patron: N, node: N) {
        self.disavowed.insert((patron, node));
    }

    /// Whether some patron this observer holds has ended a relationship
    /// with `n` with prejudice.
    ///
    /// **Held is the whole of the test.** The pair reaches the ball the
    /// disavowing patron floods to and no further, so an observer holding
    /// one is by construction a member of the neighbourhood whose default
    /// §18.5 is describing.
    pub fn blacklisted(&self, n: &N) -> bool {
        self.disavowed.iter().any(|(_, node)| node == n)
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

    /// Whether `n` appears anywhere in this observer's evidence.
    pub fn knows(&self, n: &N) -> bool {
        self.known().contains(n)
    }

    /// The pairs of the evaluator's graph: adoptions and acquaintances
    /// collapsed to one unordered pair each (design §16.2.1).
    pub fn pairs(&self) -> BTreeSet<(N, N)> {
        self.adoptions
            .iter()
            .map(|(p, c)| unordered(p.clone(), c.clone()))
            .chain(self.acquaintances.iter().cloned())
            .collect()
    }
}

/// An omniscient topology, for the conformance test: every adoption and
/// every peering that exists.  No node holds this; each observer's evidence
/// is derived from it by the visibility rules, which is what makes the
/// bound observer-relative (design §16.3.1).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct World<N: Ord + Clone> {
    /// Every `(patron, node)` adoption in the topology.
    pub adoptions: Vec<(N, N)>,
    /// Every peering.
    pub peerings: Vec<(N, N)>,
}

impl<N: Ord + Clone + Debug> World<N> {
    /// An empty topology.
    pub fn new() -> Self {
        World {
            adoptions: Vec::new(),
            peerings: Vec::new(),
        }
    }

    /// Record that `patron` adopted `node`.
    pub fn adopt(&mut self, patron: N, node: N) {
        self.adoptions.push((patron, node));
    }

    /// Record a peering between `a` and `b`.
    pub fn peer(&mut self, a: N, b: N) {
        self.peerings.push((a, b));
    }

    /// The scope the adoptions imply (design §16.2.1).
    pub fn scope(&self) -> Scope<N> {
        Scope::from_adoptions(&self.adoptions)
    }

    /// The nodes within `observer`'s horizon (design §15.1).
    pub fn horizon(&self, observer: &N) -> BTreeSet<N> {
        self.scope().horizon(observer, HORIZON)
    }

    /// The nodes `patron` adopted.
    pub fn children(&self, patron: &N) -> Vec<N> {
        self.adoptions
            .iter()
            .filter(|(p, _)| p == patron)
            .map(|(_, c)| c.clone())
            .collect()
    }

    /// Every node named by any adoption or peering.
    pub fn nodes(&self) -> BTreeSet<N> {
        self.adoptions
            .iter()
            .chain(&self.peerings)
            .flat_map(|(a, b)| [a.clone(), b.clone()])
            .collect()
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
            let frontier: Vec<N> = ev
                .known()
                .into_iter()
                .filter(|n| !expanded.contains(n))
                .collect();
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
