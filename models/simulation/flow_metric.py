#!/usr/bin/env python3
"""Trust-metric simulation for the RHTN reference flow metric.

WHAT THIS FILE IS
=================
`network-design.md` §16.2 chooses max-flow / min-cut as the reference trust
metric and makes four analytical claims that, per §20 of the design and the
review plan's Stage 1, have been argued but never run:

  E1. A distance-decay metric with per-hop factor lambda diverges unless
      lambda < 1/f, and a deep fake subtree exploits the divergent case
      (design §16.2's opening arithmetic).
  E2. Under a flow metric, a region behind a fixed boundary is bounded by
      that boundary's capacity REGARDLESS of the region's population
      (the single-observer cut bound; design §16.2, §17.3).
  E3. Setwise conservation (design §16.2, normative since 2026-09-03):
      for any SET of identities behind a cut, their SIMULTANEOUSLY USABLE
      standing totals at most the cut's capacity -- one computation, shared
      capacity.  Evaluating each identity with its own independent max-flow
      computation instead lets the same capacity count once per identity,
      and the aggregate grows with population.  This experiment demonstrates
      both halves: the conserving computation obeys the bound, the
      independent-per-target computation violates it.
  E4. Edge-influence fanout (design §16.3.1, the coverage bound): one
      acquired edge sits inside every observer's horizon that contains it,
      so the cost of influencing a population amortises.  We insert a single
      edge and count how many observers' accepted-sets change.

This file has NO dependencies outside the Python 3 standard library.  The
max-flow algorithm is implemented here, in the open, so that reviewing this
file requires no background reading: every algorithm used is explained where
it appears.

HOW TO RUN
==========
    python3 flow_metric.py            # runs E1-E4, prints a report
    python3 flow_metric.py --seed 7   # a different random topology draw

The output of the run committed alongside this file is in results.txt.

WHAT THIS SIMULATION IS NOT
===========================
It is not a network simulator: there are no messages, sessions, or
signatures here.  It is a GRAPH calculation.  Its job is to check the
ARITHMETIC of §16.2's claims, exactly as the review plan's Stage 1.3 asks
("the flow-metric claims ... are currently argued analytically and none has
been run").  Anything about human behaviour -- who adopts whom, whether
fake regions are cheap to build socially -- is outside its reach, per the
plan's "what this plan cannot do".
"""

import argparse
import random
from collections import deque

# ---------------------------------------------------------------------------
# PART 0 -- The graph representation.
#
# A directed graph with integer edge capacities, stored as adjacency
# dictionaries:  cap[u][v] = remaining capacity on the edge u->v.
#
# Max-flow algorithms want a RESIDUAL graph: when we push f units of flow
# along u->v we reduce cap[u][v] by f and INCREASE cap[v][u] by f.  The
# reverse edge represents the option to "undo" flow later, which is what
# lets the algorithm escape locally-good but globally-bad early choices.
# This is the standard construction from any algorithms textbook (see
# Cormen et al., "Introduction to Algorithms", ch. 24 in the 4th ed.).
# ---------------------------------------------------------------------------

class FlowGraph:
    """A capacitated directed graph supporting max-flow queries."""

    def __init__(self):
        # cap maps u -> {v: capacity}.  Missing entries mean capacity 0.
        self.cap = {}

    def add_node(self, u):
        self.cap.setdefault(u, {})

    def add_edge(self, u, v, c):
        """Add capacity c to the directed edge u->v.

        We ADD rather than SET so that parallel logical edges combine, and
        we always materialise the reverse edge with +0 capacity so the
        residual bookkeeping in max_flow never needs a missing-key case.
        """
        self.add_node(u)
        self.add_node(v)
        self.cap[u][v] = self.cap[u].get(v, 0) + c
        self.cap[v].setdefault(u, 0)

    def copy(self):
        g = FlowGraph()
        g.cap = {u: dict(nbrs) for u, nbrs in self.cap.items()}
        return g

    # -----------------------------------------------------------------------
    # Max-flow by the Edmonds-Karp algorithm.
    #
    # The idea: repeatedly find ANY path from source to sink along edges
    # with remaining capacity (an "augmenting path"), push as much flow as
    # the path's tightest edge allows, update the residual graph, and stop
    # when no augmenting path exists.  Ford-Fulkerson is this idea with any
    # path search; Edmonds-Karp is the refinement that uses breadth-first
    # search (BFS), which guarantees termination in O(V * E^2) regardless
    # of capacity values.  For our graph sizes (hundreds of nodes) this is
    # instantaneous.
    #
    # The max-flow/min-cut theorem -- the entire reason §16.2 chose this
    # metric -- says the value this returns EQUALS the total capacity of
    # the smallest edge cut separating source from sink.  So computing
    # max-flow IS computing the bound "what crosses the boundary".
    # -----------------------------------------------------------------------

    def max_flow(self, source, sink):
        """Return the maximum flow value from source to sink.

        Mutates self (leaves the residual graph behind).  Callers that need
        the original intact use .copy() first -- the experiments below
        always do, so each measurement starts from the same graph.
        """
        total = 0
        while True:
            # --- BFS for the shortest augmenting path ---------------------
            # parent[v] remembers how we reached v, so we can walk the path
            # backwards once we touch the sink.
            parent = {source: None}
            queue = deque([source])
            while queue and sink not in parent:
                u = queue.popleft()
                for v, c in self.cap[u].items():
                    if c > 0 and v not in parent:
                        parent[v] = u
                        queue.append(v)
            if sink not in parent:
                return total          # no augmenting path left: flow is max
            # --- find the bottleneck capacity along the path --------------
            bottleneck = float("inf")
            v = sink
            while parent[v] is not None:
                u = parent[v]
                bottleneck = min(bottleneck, self.cap[u][v])
                v = u
            # --- push the bottleneck along the path, update residuals -----
            v = sink
            while parent[v] is not None:
                u = parent[v]
                self.cap[u][v] -= bottleneck   # forward edge loses capacity
                self.cap[v][u] += bottleneck   # reverse edge gains "undo"
                v = u
            total += bottleneck


# ---------------------------------------------------------------------------
# PART 1 -- Building RHTN-shaped topologies.
#
# The design's structures, reduced to what the metric can see:
#
#   * Nodes form a tree by ADOPTION (design §6): each node has one patron
#     within a subnet; fanout is bounded by f = 10 (design §3.2).
#   * PEERING edges (design §6.3) cross the tree laterally and carry a
#     deliberately LOWER default flow capacity than hierarchical edges
#     (design §16.3's mitigation).
#   * An observer's TRUST HORIZON is the two-edge ball around it
#     (design §15.1, Vocabulary): every node within a two-edge walk over
#     adoption and sibling edges.
#
# Trust edges are treated as BIDIRECTED for flow purposes: an adoption is a
# mutual relationship, so capacity is added in both directions.  The
# capacities are units of "trust capacity" -- the design deliberately does
# not fix absolute values (policy is local, §16.1), so what matters in every
# experiment below is RELATIVE behaviour, never the absolute numbers.
# ---------------------------------------------------------------------------

HIER_CAP = 10   # capacity of an adoption (hierarchical) edge
PEER_CAP = 2    # capacity of a peering edge -- lower, per design §16.3

def build_tree(rng, depth, fanout, prefix="n"):
    """Build a random adoption tree.

    Returns (graph, nodes, children) where children[u] lists u's adoptees.
    Node names encode their path from the root, e.g. "n.0.3.1", purely for
    human readability of the output.
    """
    g = FlowGraph()
    root = prefix
    g.add_node(root)
    nodes = [root]
    children = {root: []}
    frontier = [root]
    for _ in range(depth):
        next_frontier = []
        for u in frontier:
            # Each node adopts between 1 and `fanout` children -- random,
            # because real subnets are ragged, and nothing below depends on
            # the tree being full.
            for i in range(rng.randint(1, fanout)):
                v = f"{u}.{i}"
                g.add_edge(u, v, HIER_CAP)   # patron -> child
                g.add_edge(v, u, HIER_CAP)   # child -> patron (bidirected)
                nodes.append(v)
                children.setdefault(u, []).append(v)
                children[v] = []
                next_frontier.append(v)
        frontier = next_frontier
    return g, nodes, children


def adjacency(g):
    """The undirected adjacency relation implied by positive capacities."""
    adj = {u: set() for u in g.cap}
    for u, nbrs in g.cap.items():
        for v, c in nbrs.items():
            if c > 0:
                adj[u].add(v)
                adj[v].add(u)
    return adj


def horizon(g, observer, radius=2):
    """The observer's trust horizon: nodes within `radius` edges.

    design §15.1 / Vocabulary: a two-edge walk centred on the observer.
    Every observer's horizon is different and centred on themselves -- the
    design's no-shared-state rule, which E4 depends on getting right.
    """
    adj = adjacency(g)
    seen = {observer}
    frontier = {observer}
    for _ in range(radius):
        frontier = {w for v in frontier for w in adj[v]} - seen
        seen |= frontier
    return seen


def visible_subgraph(g, observer):
    """The graph AS THIS OBSERVER SEES IT: edges with both ends in its
    horizon.  design §16.3.1: "It is a property of the graph the observer
    can see" -- an edge outside the horizon cannot raise any cut in this
    observer's view, and this function is where that rule is enforced.
    """
    hz = horizon(g, observer)
    sub = FlowGraph()
    for u in hz:
        sub.add_node(u)
        for v, c in g.cap[u].items():
            if c > 0 and v in hz:
                # Each DIRECTED edge is visited exactly once (u's own
                # adjacency lists it), so add_edge -- which also creates the
                # zero-capacity reverse entry the residual bookkeeping
                # needs -- is the right tool here, as everywhere.
                sub.add_edge(u, v, c)
    return sub


# ---------------------------------------------------------------------------
# PART 2 -- The two evaluation modes E3 compares.
#
# MODE A: independent per-target max-flow.  score(observer, t) is computed
# with a FRESH graph for every target t.  Each computation may use the full
# capacity of every edge.  This is the natural reading of "trust for every
# known node" that an implementer might adopt -- and the reading the 0.8.3
# adversarial review showed breaks the aggregate bound.
#
# MODE B: one conserving computation (the Advogato shape, design §16.2).
# We build ONE flow network in which every candidate target drains through
# a personal 1-capacity edge into a shared super-sink, and run ONE max-flow
# from the observer.  Because all targets share the same residual graph,
# capacity consumed reaching one target is unavailable for reaching
# another: conservation is intrinsic, not policed.
#
# The 1-capacity drain per target makes the result a count of ACCEPTED
# identities (Advogato's accept/reject semantics).  "Simultaneously usable
# standing" in §16.2's setwise-conservation statement is then: how many of
# the set can be accepted AT ONCE.
#
# One technical construction both modes share: NODE capacities.  A flow
# network limits EDGES, but we also need to limit how much trust can pass
# THROUGH an intermediate node (otherwise a single compromised neighbour
# with many edges relays unbounded capacity).  The standard trick -- again
# textbook material -- is NODE SPLITTING: replace node u by u_in and u_out
# joined by one edge carrying u's node capacity; every edge into u enters
# u_in, every edge out of u leaves u_out.
# ---------------------------------------------------------------------------

def node_capacity(dist):
    """Per-node throughput by distance from the observer.

    Advogato decreases capacity with distance from the seed; we use a
    simple halving schedule.  The absolute numbers are unimportant (policy
    is local, design §16.1); the experiments only compare behaviours under
    ONE schedule held fixed.
    """
    return max(32 >> dist, 1)      # 32, 16, 8, 4, 2, 1, 1, ...


def distances_from(g, source):
    """BFS distance (in edges) from source to every reachable node."""
    adj = adjacency(g)
    dist = {source: 0}
    queue = deque([source])
    while queue:
        u = queue.popleft()
        for v in adj[u]:
            if v not in dist:
                dist[v] = dist[u] + 1
                queue.append(v)
    return dist


def split_graph(g, observer):
    """Apply node splitting with distance-based node capacities.

    Returns a new FlowGraph over nodes ("in", u) and ("out", u).  The
    observer's own throughput is unbounded (they are the source; capping
    the party doing the evaluating would be self-limiting to no purpose).
    """
    dist = distances_from(g, observer)
    s = FlowGraph()
    for u in g.cap:
        if u not in dist:
            continue                       # unreachable: contributes nothing
        ncap = 10**9 if u == observer else node_capacity(dist[u])
        s.add_edge(("in", u), ("out", u), ncap)
        for v, c in g.cap[u].items():
            if c > 0 and v in dist:
                # each original edge u->v becomes out(u) -> in(v)
                s.add_edge(("out", u), ("in", v), c)
    return s


def score_independent(g, observer, target):
    """MODE A: the target's individual score, on a fresh copy of the
    split graph.  This is the max flow observer -> target, i.e. the
    min-cut between them -- the per-identity bound, which E2 shows is
    honoured even in this mode."""
    s = split_graph(g, observer)
    if ("in", target) not in s.cap:
        return 0
    return s.copy().max_flow(("out", observer), ("in", target))


def accepted_count(g, observer, targets):
    """MODE B: how many of `targets` are accepted SIMULTANEOUSLY.

    One split graph, one super-sink, one max-flow.  Each target drains at
    most 1 unit into the super-sink, so the returned flow value counts
    accepted identities, and every unit consumed real shared capacity on
    the way there.
    """
    s = split_graph(g, observer)
    SINK = ("sink", None)
    reachable = 0
    for t in targets:
        if ("out", t) in s.cap:
            s.add_edge(("out", t), SINK, 1)
            reachable += 1
    if reachable == 0:
        return 0
    return s.max_flow(("out", observer), SINK)


# ---------------------------------------------------------------------------
# EXPERIMENT E1 -- distance-decay divergence (design §16.2's arithmetic).
#
# The design's claim, verbatim in structure: with per-hop weight lambda and
# fanout f, the total weight of a fake subtree at depth D with k further
# levels is  lambda^D * sum_{i=0..k} (f*lambda)^i , which diverges as k
# grows unless f*lambda < 1.  We verify the two quoted data points:
#   lambda = 0.5,  f = 10, depth-6 fake subtree  -> mass ~19,500x one
#     honest node at the same distance,
#   lambda = 0.05, f = 10 -> the series converges to ~2x.
# No flow computation is involved; this is the arithmetic the flow metric
# exists to escape, run rather than asserted.
# ---------------------------------------------------------------------------

def experiment_divergence(report):
    f = 10
    D = 1                       # the fake subtree hangs one hop away
    def fake_mass(lam, levels):
        """Sum of lambda^(D+i) * f^i for i = 0..levels: every fake node's
        weight, where level i holds f^i nodes at distance D+i."""
        return sum((lam ** (D + i)) * (f ** i) for i in range(levels + 1))
    honest_one = lambda lam: lam ** D       # one honest node at distance D

    report.append("E1: distance-decay divergence (design 16.2)")
    for lam, levels in [(0.5, 6), (0.05, 6)]:
        ratio = fake_mass(lam, levels) / honest_one(lam)
        regime = "DIVERGES (f*lambda = %.1f > 1)" % (f * lam) if f * lam > 1 \
            else "converges (f*lambda = %.1f < 1)" % (f * lam)
        report.append(
            f"  lambda={lam:<5} depth-{levels} fake subtree carries "
            f"{ratio:,.0f}x the weight of one honest node -- {regime}")
    # The design's quoted figures: ~19,500x at 0.5 and total mass ~2 at 0.05.
    assert 15_000 < fake_mass(0.5, 6) / honest_one(0.5) < 25_000
    assert fake_mass(0.05, 20) < 0.15   # converged tail: tiny absolute mass
    report.append("  matches design 16.2's quoted magnitudes: CONFIRMED")
    report.append("")


# ---------------------------------------------------------------------------
# EXPERIMENT E2 -- the single-observer cut bound.
#
# Construction: an honest tree; one honest node B ("the boundary vertex")
# adopts the root of a FAKE subtree.  Every path from the observer into the
# fake region passes through the single edge B -> fake-root -- a cut of
# known capacity.  We then grow the fake region and measure the BEST
# INDIVIDUAL score any fake identity achieves.  The claim (design §16.2:
# "the entire subtree inherits at most what flows through that one
# vertex"): the score must never exceed the cut, no matter the population.
# ---------------------------------------------------------------------------

def attach_fake_region(g, boundary_node, width, depth):
    """Attach a fake subtree of `width` children per level, `depth` levels,
    under a single fake root adopted by boundary_node.  Returns the list of
    fake identities.  Fake-internal edges get GENEROUS capacity -- the
    attacker wires their own region however they like (design §17.2:
    topology cannot provide Sybil resistance; only the boundary is real).
    """
    fake_root = "FAKE"
    g.add_edge(boundary_node, fake_root, HIER_CAP)
    g.add_edge(fake_root, boundary_node, HIER_CAP)
    fakes = [fake_root]
    frontier = [fake_root]
    for _ in range(depth):
        nxt = []
        for u in frontier:
            for i in range(width):
                v = f"{u}.f{i}"
                g.add_edge(u, v, 100)          # attacker-internal: generous
                g.add_edge(v, u, 100)
                fakes.append(v)
                nxt.append(v)
        frontier = nxt
    return fakes


def experiment_cut_bound(rng, report):
    report.append("E2: the individual cut bound (design 16.2, 17.3)")
    base, nodes, _ = build_tree(rng, depth=3, fanout=3)
    observer = nodes[0]                        # the root observes
    boundary = nodes[1]                        # its first child adopts FAKE
    for width, depth in [(2, 2), (3, 3), (4, 4)]:
        g = base.copy()
        fakes = attach_fake_region(g, boundary, width, depth)
        best = max(score_independent(g, observer, t) for t in fakes)
        # The relevant cut is what the OBSERVER's own edges to the boundary
        # region admit; with one B->FAKE edge of HIER_CAP, no fake identity
        # can score above HIER_CAP however the interior is wired.
        ok = best <= HIER_CAP
        report.append(
            f"  fake region of {len(fakes):>4} identities: best individual "
            f"score {best} <= boundary capacity {HIER_CAP}: "
            f"{'HOLDS' if ok else 'VIOLATED'}")
        assert ok, "individual cut bound violated -- investigate"
    report.append("  population never moved the individual bound: CONFIRMED")
    report.append("")


# ---------------------------------------------------------------------------
# EXPERIMENT E3 -- setwise conservation, and the failure mode it forbids.
#
# Same construction as E2, but now we ask about the AGGREGATE.
#
#   Mode A (independent per-target): sum over fake identities of each one's
#     individual score.  Every computation reuses the boundary capacity, so
#     the sum grows linearly with population -- each identity is "bounded"
#     while the aggregate is not.  This is the broken reading.
#   Mode B (one conserving computation): the number of fake identities
#     accepted simultaneously.  Design §16.2 (normative, 2026-09-03): this
#     must be bounded by the cut and must NOT grow once the boundary
#     saturates.
#
# The pass criterion mirrors the 0.8.3 validation text: "increasing only
# the number of identities must never increase the aggregate ... beyond the
# fixed cut".
# ---------------------------------------------------------------------------

def region_ceiling(g, observer, entry):
    """The capacity of the cut between the observer and the region behind
    `entry` -- computed as the max flow from the observer to entry's OUT
    node in the split graph.  This is, literally, design §16.2's sentence
    "the entire subtree inherits at most what flows through that one
    vertex": every identity behind `entry` drains through entry's out-node,
    so no set of them can simultaneously use more than reaches it.
    """
    s = split_graph(g, observer)
    return s.max_flow(("out", observer), ("out", entry))


def experiment_setwise(rng, report):
    report.append("E3: setwise conservation (design 16.2, normative)")
    base, nodes, _ = build_tree(rng, depth=3, fanout=3)
    observer = nodes[0]
    boundary = nodes[1]
    joints = []
    for width, depth in [(2, 2), (3, 3), (4, 4)]:
        g = base.copy()
        fakes = attach_fake_region(g, boundary, width, depth)
        indep_sum = sum(score_independent(g, observer, t) for t in fakes)
        joint = accepted_count(g, observer, fakes)
        ceiling = region_ceiling(g, observer, "FAKE")
        # The claim, exactly as the 0.8.3 validation text states it:
        # growing the population must never push the AGGREGATE beyond the
        # fixed cut.  Growth UP TO the ceiling is legitimate (7 identities
        # cannot fill an 8-unit cut); growth BEYOND it never happens.
        report.append(
            f"  {len(fakes):>4} fake identities:  independent-sum = "
            f"{indep_sum:>5}   conserving joint = {joint}   "
            f"region ceiling = {ceiling}")
        assert joint <= ceiling, \
            "aggregate exceeded the cut under conservation -- investigate"
        joints.append((len(fakes), joint, ceiling))
    # Saturation: once the population exceeds the ceiling, the joint count
    # sits AT the ceiling and further population buys nothing.
    for n, joint, ceiling in joints:
        if n >= ceiling:
            assert joint == ceiling, "saturated region below its ceiling"
    report.append(
        "  the independent sum grows with population (the reading 0.8.3 "
        "showed is broken);")
    report.append(
        "  the conserving joint saturates at the region ceiling and "
        "population buys nothing more: CONFIRMED")
    report.append("")


# ---------------------------------------------------------------------------
# EXPERIMENT E4 -- edge-influence fanout (design §16.3.1's coverage bound).
#
# Claim under test: "one acquired edge sits inside every horizon that
# contains it and helps each of those observers at once -- the cost of
# influencing a population scales with the coverage of acquired edges over
# that population, not with the number of observers."
#
# Method: build a tree; attach one attacker identity by ONE new edge to a
# randomly chosen victim node; for EVERY observer in the graph, compute the
# attacker's score in that observer's VISIBLE subgraph (their horizon)
# before and after.  Count observers whose score changed.  If the old
# "per-target" reading were right, that count would be ~1; the coverage
# reading predicts it equals the number of observers whose horizon contains
# the new edge.
# ---------------------------------------------------------------------------

def experiment_fanout(rng, report):
    report.append("E4: edge-influence fanout (design 16.3.1)")
    g, nodes, children = build_tree(rng, depth=3, fanout=4)
    attacker = "ATTACKER"
    # Try the acquired edge at several victims -- interior nodes, because a
    # leaf's horizon overlaps few others and understates the effect the
    # design's own text predicts for a well-placed edge.
    interior = [n for n in nodes if children.get(n)]
    results = []
    for trial in range(5):
        victim = rng.choice(interior)
        g_after = g.copy()
        g_after.add_edge(victim, attacker, PEER_CAP)
        g_after.add_edge(attacker, victim, PEER_CAP)
        changed = 0        # observers whose evaluation of the attacker moved
        holds_edge = 0     # observers whose horizon contains the new edge
        for obs in nodes:
            # Score the attacker inside each observer's OWN visible
            # subgraph, before and after -- E4 is about visibility, and
            # design §16.3.1's rule is that only an edge an observer can
            # see can change that observer's cut.
            before_g = visible_subgraph(g, obs)
            after_g = visible_subgraph(g_after, obs)
            b = (score_independent(before_g, obs, attacker)
                 if attacker in before_g.cap else 0)
            a = (score_independent(after_g, obs, attacker)
                 if attacker in after_g.cap else 0)
            if a != b:
                changed += 1
            if attacker in horizon(g_after, obs):
                holds_edge += 1
        results.append((victim, changed, holds_edge))
        # The two claims under test, per placement:
        #  - coverage, not per-target: MORE THAN ONE observer moved;
        #  - the conservative direction: nobody OUTSIDE the edge's
        #    horizons moved (invisible edges cannot help).
        assert changed > 1, \
            "per-target reading would predict 1 changed observer"
        assert changed <= holds_edge, \
            "an observer moved without the edge in its horizon"
    for victim, changed, holds_edge in results:
        report.append(
            f"  edge at {victim!r}: {changed} of {len(nodes)} observers' "
            f"evaluations changed ({holds_edge} hold the edge in horizon)")
    report.append(
        "  one edge influences every observer whose horizon contains it and "
        "no observer whose")
    report.append(
        "  horizon does not -- the coverage bound, quantitatively, per "
        "placement: CONFIRMED")
    report.append("")


# ---------------------------------------------------------------------------
# Entry point: run all experiments, print and save the report.
# ---------------------------------------------------------------------------

def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--seed", type=int, default=1,
                    help="RNG seed; results are reproducible per seed")
    args = ap.parse_args()
    rng = random.Random(args.seed)
    report = [f"flow_metric.py report (seed {args.seed})", ""]
    experiment_divergence(report)
    experiment_cut_bound(rng, report)
    experiment_setwise(rng, report)
    experiment_fanout(rng, report)
    report.append("all assertions passed")
    text = "\n".join(report)
    print(text)
    return text


if __name__ == "__main__":
    main()
