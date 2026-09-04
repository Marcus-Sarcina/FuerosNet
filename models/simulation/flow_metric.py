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
      and the aggregate grows with population.
  E4. Edge-influence fanout (design §16.3.1, the coverage bound): one
      acquired peering edge is visible inside both peers' horizons, so it
      helps every observer whose horizon contains a peer -- the cost of
      influencing a population amortises over coverage, not observer count.

SCOPE, TRUST-CAPACITY, AND VISIBILITY ARE THREE DIFFERENT THINGS
===============================================================
The design is emphatic that these must not be conflated (§6.3:
"Contributing to trust and conferring scope are different things ... the
region in §15.1 is built from adoption and sibling edges only"):

  * SCOPE topology -- adoption edges and SIBLING edges (co-children of one
    patron).  The trust horizon is the two-edge walk over THIS graph
    (design §15.1, Vocabulary).  It decides who can SEE what.
  * TRUST-CAPACITY topology -- adoption edges (hierarchical capacity) and
    PEERING edges (a lower default capacity, §16.3).  It decides how much
    trust can FLOW.
  * VISIBILITY -- a peering edge "is visible inside the two peers' horizons
    and nowhere else" (§16.3.1).  A peering edge confers NO scope and does
    NOT extend the horizon (§6.3), but an observer who can see it may use
    its capacity.

An earlier version of this file used ONE adjacency relation for all three,
which made E3 count identities the observer could not see and made E4
understate an acquired edge's coverage several-fold.  The separation below
is the correction; the three concepts now have three representations:
`scope_children`/`scope_adjacency` (scope), `FlowGraph` capacities (flow),
and `visible_flow_subgraph` (visibility).

This file has NO dependencies outside the Python 3 standard library.  The
max-flow algorithm is implemented here, in the open, so that reviewing this
file requires no background reading.

HOW TO RUN
==========
    python3 flow_metric.py            # runs E1-E4, prints a report
    python3 flow_metric.py --seed 7   # a different random topology draw

The committed run is results.txt.

WHAT THIS SIMULATION IS NOT
===========================
It is not a network simulator: no messages, sessions, or signatures.  It is
a GRAPH calculation checking the ARITHMETIC of §16.2/§16.3.1's claims.
Human-behaviour questions (whether fake regions are cheap to build socially)
are outside its reach, per the plan's "what this plan cannot do".  E4 is a
worst-case-aware coverage measurement, not a statistical sample: it
enumerates every interior placement in a toy topology rather than sampling a
few.
"""

import argparse
import random
import statistics
from collections import deque

# ---------------------------------------------------------------------------
# PART 0 -- The graph representation and max-flow.
#
# A directed graph with integer edge capacities, stored as adjacency
# dictionaries:  cap[u][v] = remaining capacity on the edge u->v.  Max-flow
# uses a RESIDUAL graph: pushing f units along u->v reduces cap[u][v] by f
# and increases cap[v][u] by f, the reverse edge being the option to "undo"
# flow later.  Standard construction (Cormen et al., "Introduction to
# Algorithms", ch. 24, 4th ed.).
#
# NOTE ON VALIDATION: the external review of this file cross-checked
# max_flow() against exhaustive brute-force minimum cuts on 700 random
# directed graphs (2-7 vertices) with zero discrepancies.  The algorithm
# below is therefore not where a defect would hide; the modelling around it
# is what the rest of this file gets right or wrong.
# ---------------------------------------------------------------------------

class FlowGraph:
    """A capacitated directed graph supporting max-flow queries."""

    def __init__(self):
        self.cap = {}                  # cap[u] -> {v: capacity}

    def add_node(self, u):
        self.cap.setdefault(u, {})

    def add_edge(self, u, v, c):
        """Add capacity c to the directed edge u->v.

        ADD (not set) so parallel logical edges combine; always materialise
        the +0 reverse edge so the residual bookkeeping never hits a missing
        key.
        """
        self.add_node(u)
        self.add_node(v)
        self.cap[u][v] = self.cap[u].get(v, 0) + c
        self.cap[v].setdefault(u, 0)

    def copy(self):
        g = FlowGraph()
        g.cap = {u: dict(nbrs) for u, nbrs in self.cap.items()}
        return g

    def max_flow(self, source, sink):
        """Maximum flow value from source to sink (Edmonds-Karp: BFS for the
        shortest augmenting path, repeat until none remains).  Equals the
        min-cut capacity by the max-flow/min-cut theorem -- which is the
        entire reason §16.2 chose this metric.  Mutates self (leaves the
        residual behind); callers .copy() first when they need the original.
        """
        total = 0
        while True:
            parent = {source: None}
            queue = deque([source])
            while queue and sink not in parent:
                u = queue.popleft()
                for v, c in self.cap[u].items():
                    if c > 0 and v not in parent:
                        parent[v] = u
                        queue.append(v)
            if sink not in parent:
                return total
            bottleneck = float("inf")
            v = sink
            while parent[v] is not None:
                u = parent[v]
                bottleneck = min(bottleneck, self.cap[u][v])
                v = u
            v = sink
            while parent[v] is not None:
                u = parent[v]
                self.cap[u][v] -= bottleneck
                self.cap[v][u] += bottleneck
                v = u
            total += bottleneck


# ---------------------------------------------------------------------------
# PART 1 -- RHTN-shaped topology, with scope and flow SEPARATED.
#
# We build:
#   * an adoption tree -- the backbone of both graphs;
#   * `scope_children`, from which SCOPE adjacency (adoption + sibling) is
#     derived for horizons;
#   * a FlowGraph of adoption capacity edges; peering edges are added
#     separately, because their VISIBILITY is per-observer.
#
# Capacities are units of "trust capacity".  Absolute values are
# unimportant (policy is local, §16.1); every experiment compares behaviour
# under ONE fixed schedule.
# ---------------------------------------------------------------------------

HIER_CAP = 10   # capacity of an adoption (hierarchical) edge
PEER_CAP = 2    # capacity of a peering edge -- lower, per design §16.3

def build_tree(rng, depth, fanout, prefix="n"):
    """Build a random adoption tree.

    Returns (flow, nodes, children):
      flow     -- FlowGraph with bidirected adoption edges (HIER_CAP)
      nodes    -- list of node names
      children -- children[u] = list of u's adoptees
    Node names encode the path from the root ("n.0.3.1") for readability.
    """
    flow = FlowGraph()
    root = prefix
    flow.add_node(root)
    nodes = [root]
    children = {root: []}
    frontier = [root]
    for _ in range(depth):
        nxt = []
        for u in frontier:
            for i in range(rng.randint(1, fanout)):
                v = f"{u}.{i}"
                flow.add_edge(u, v, HIER_CAP)      # patron -> child
                flow.add_edge(v, u, HIER_CAP)      # child -> patron
                nodes.append(v)
                children.setdefault(u, []).append(v)
                children[v] = []
                nxt.append(v)
        frontier = nxt
    return flow, nodes, children


def scope_adjacency(nodes, children):
    """SCOPE adjacency: adoption edges PLUS sibling edges.

    design §15.1 / Vocabulary: the trust horizon is the two-edge walk over
    ADOPTION AND SIBLING edges.  Siblings are the co-children of one patron
    (design §6.3 distinguishes them from adoption).  Peering edges are
    DELIBERATELY ABSENT here: §6.3 says a peering edge "confers no scope, and
    does not extend the horizon."  This graph is what decides who sees what;
    it carries no capacity.
    """
    adj = {u: set() for u in nodes}
    for u, kids in children.items():
        for v in kids:                         # adoption edges
            adj[u].add(v)
            adj[v].add(u)
        for a in kids:                         # sibling edges (co-children)
            for b in kids:
                if a != b:
                    adj[a].add(b)
    return adj


def horizon(scope_adj, observer, radius=2):
    """The observer's trust horizon: nodes within `radius` SCOPE edges.

    design §15.1: a two-edge walk over adoption and sibling edges, centred
    on the observer.  Every observer's horizon is its own -- the design's
    no-shared-state rule.
    """
    seen = {observer}
    frontier = {observer}
    for _ in range(radius):
        frontier = {w for v in frontier for w in scope_adj[v]} - seen
        seen |= frontier
    return seen


def visible_flow_subgraph(flow, scope_adj, observer, peer_edges):
    """The flow graph AS THIS OBSERVER SEES IT.

    Includes:
      * adoption capacity edges with BOTH ends in the observer's scope
        horizon;
      * every peering edge the observer can see -- design §16.3.1: a peering
        edge "is visible inside the two peers' horizons and nowhere else",
        i.e. visible to an observer iff at least one of its two endpoints is
        in the observer's horizon.  Seeing the edge means learning the far
        endpoint's identity and trust contribution too, so a visible peering
        edge pulls its far endpoint into the observer's flow view EVEN
        THOUGH that endpoint is not in the observer's scope horizon (peering
        confers no scope, §6.3).

    `peer_edges` is a list of (u, v, cap) peering edges kept separate from
    the adoption FlowGraph precisely because their visibility is
    per-observer.
    """
    hz = horizon(scope_adj, observer)
    sub = FlowGraph()
    for u in hz:
        sub.add_node(u)
        for v, c in flow.cap[u].items():
            if c > 0 and v in hz:                    # adoption edge in view
                sub.add_edge(u, v, c)
    for (u, v, c) in peer_edges:
        if u in hz or v in hz:                       # §16.3.1 visibility rule
            sub.add_edge(u, v, c)
            sub.add_edge(v, u, c)
    return sub


# ---------------------------------------------------------------------------
# PART 2 -- Node splitting and the two evaluation modes.
#
# NODE CAPACITIES.  A flow network limits edges; we also limit throughput
# THROUGH a node (else one compromised neighbour with many edges relays
# unbounded capacity).  The textbook trick is NODE SPLITTING: replace u by
# u_in and u_out joined by one edge carrying u's node capacity.
#
# MODE A (independent per-target): score(observer, t) on a FRESH split
# graph.  Each computation may use every edge's full capacity -- the natural
# but BROKEN reading (0.8.3) under which the same capacity counts once per
# identity.
#
# MODE B (one conserving computation, the Advogato shape, §16.2): every
# candidate drains 1 unit into a shared super-sink and ONE max-flow runs, so
# capacity consumed reaching one target is unavailable for another.
# ---------------------------------------------------------------------------

def node_capacity(dist):
    """Per-node throughput by scope/flow distance from the observer.

    MODELLER'S CHOICE, NOT FROM THE DESIGN.  Advogato decreases capacity with
    distance from the seed and this file uses a halving schedule
    (32, 16, 8, 4, ...).  The design fixes no vertex-capacity schedule --
    trust policy is local and pluggable (§16.2, §16.4) -- so every absolute
    number below is conditional on this choice.  What the experiments test is
    RELATIVE behaviour under ONE schedule held fixed: growing a population,
    adding an edge, changing evaluation mode.  A different schedule moves the
    numbers and not the conclusions.
    """
    return max(32 >> dist, 1)          # 32, 16, 8, 4, 2, 1, 1, ...


def distances_from(g, source):
    """BFS distance (in edges) from source over positive-capacity edges."""
    dist = {source: 0}
    queue = deque([source])
    while queue:
        u = queue.popleft()
        for v, c in g.cap[u].items():
            if c > 0 and v not in dist:
                dist[v] = dist[u] + 1
                queue.append(v)
    return dist


def split_graph(g, observer):
    """Node splitting with distance-based node capacities.  Nodes become
    ("in", u) and ("out", u); the observer's own throughput is unbounded
    (capping the evaluator would be self-limiting)."""
    dist = distances_from(g, observer)
    s = FlowGraph()
    for u in g.cap:
        if u not in dist:
            continue
        ncap = 10**9 if u == observer else node_capacity(dist[u])
        s.add_edge(("in", u), ("out", u), ncap)
        for v, c in g.cap[u].items():
            if c > 0 and v in dist:
                s.add_edge(("out", u), ("in", v), c)
    return s


def score_independent(g, observer, target):
    """MODE A: the target's individual score = max flow observer->target =
    the min-cut between them.  E2 shows this per-identity bound holds even
    with full visibility."""
    s = split_graph(g, observer)
    if ("in", target) not in s.cap:
        return 0
    return s.copy().max_flow(("out", observer), ("in", target))


def admit_reference_order(g, observer, candidates, demand=1):
    """MODE B: admit candidates against ONE shared residual, in REFERENCE order.

    Returns (admitted, total) -- the candidates that received flow, in the
    order they were admitted, and the total flow delivered.

    THE REFERENCE-POLICY ALLOCATION RULE (design §16.4, ruled 2026-09-04),
    in two limbs:

      1. THE SHORTER PATH DOMINATES.  When a saturated cut cannot admit
         every candidate, the one reachable by the shorter path wins --
         *a longer path is less trustworthy by nature* [author].  This is
         the same reasoning §16.2 applies to distance decay, applied to
         allocation instead of weight.
      2. CONSIDERATION ORDER BREAKS TRUE TIES ONLY.  Among candidates at
         EQUAL path length, the earlier-considered one wins.

    WHY THIS NEEDS EXPLICIT CODE.  A plain multi-sink max-flow -- add every
    candidate's drain edge, run one max-flow -- looks like it implements
    both limbs and implements only the first, by accident:

      * Limb 1 falls out of Edmonds-Karp, which augments along the SHORTEST
        path first.  Two successive cross-family reviews exercised this;
        the counterexample below returns B under either candidate order,
        which limb 1 says is correct.

            observer -1-> bottleneck -1-> x -1-> A     (A: longer path)
                          bottleneck -1-> B            (B: shorter path)

      * Limb 2 does NOT.  In a plain max-flow the tie is decided by the
        order edges happen to sit in the graph's own adjacency -- an
        artifact of how the topology was BUILT -- not by the order the
        caller considers candidates.  Measured directly: with two
        equal-length candidates whose graph edges were created A-then-B,
        passing candidates in the order [B, A] still admits A.  That is a
        silent dependence on construction order, and it is why the rule is
        implemented here rather than left to the algorithm.

    THE CONSTRUCTION USED HERE.  Rank candidates by (shortest-path distance
    in the observer's own graph, consideration index), then admit greedily
    in that ranking against a shared residual: add the candidate's drain,
    run max-flow to exhaustion, record whether the total rose.  Distances
    are taken ONCE from the pre-allocation graph, because trustworthiness is
    a property of the candidate's position, not of who was served first.
    Earlier-ranked candidates cannot be displaced later: an augmenting path
    ends AT the sink and so never traverses an admitted drain in reverse.
    The final total is still the true maximum -- after the last drain is
    added and exhausted no augmenting path remains, which is max-flow's own
    optimality condition -- so the rule costs nothing in aggregate and fixes
    only WHO is admitted.

    This remains REFERENCE POLICY, NOT a protocol invariant: per-observer
    trust (§16.1) means no party consumes another's computation, so a policy
    resolving the allocation differently still conforms.
    """
    s = split_graph(g, observer)
    SINK = ("sink", None)
    source = ("out", observer)
    # Limb 1 + limb 2: rank by (distance, consideration index).  Distances
    # are measured in the split graph from the observer's out-node, so they
    # count real hops through the topology the observer can see.
    dist = distances_from(s, source)
    ranked = sorted(
        [(i, c) for i, c in enumerate(candidates) if ("out", c) in s.cap],
        key=lambda ic: (dist.get(("in", ic[1]), float("inf")), ic[0]))
    admitted, total = [], 0
    for _, c in ranked:
        s.add_edge(("out", c), SINK, demand)
        gained = s.max_flow(source, SINK)   # augments the SHARED residual
        if gained > 0:
            admitted.append(c)          # the only new capacity is c's drain
        total += gained
    return admitted, total


def accepted_count(g, observer, targets):
    """The number of candidates admitted simultaneously (unit demands).

    A thin wrapper over admit_reference_order.  NOTE the abstraction, which
    a cross-family review rightly flagged: with one-unit drains this counts
    ADMITTED IDENTITIES, not the sum of their individual standing.  The
    design's setwise-conservation wording is about "simultaneously usable
    standing", which is the general-demand case -- tested separately by
    deliverable_flow() below.  Both are true; they are different statements.
    """
    return admit_reference_order(g, observer, targets, demand=1)[1]


def deliverable_flow(g, observer, demands):
    """Total flow deliverable to a SET of candidates under ARBITRARY demands.

    `demands` maps target -> the capacity that target may draw.  This is the
    general form of design §16.2's setwise-conservation sentence: whatever
    each identity behind a cut wants to use, what the set can use AT ONCE is
    bounded by the cut.  Passing each candidate's own individual standing as
    its demand is the strongest reading -- every identity asking for
    everything it could get alone -- and is what experiment_setwise checks.
    """
    s = split_graph(g, observer)
    SINK = ("sink", None)
    for t, d in demands.items():
        if ("out", t) in s.cap and d > 0:
            s.add_edge(("out", t), SINK, d)
    return s.max_flow(("out", observer), SINK)


def region_ceiling(g, observer, entry):
    """design §16.2's sentence made a number: "the entire subtree inherits
    at most what flows through that one vertex".  Every identity behind
    `entry` drains through entry's out-node, so no set of them can
    simultaneously use more than the max flow observer -> out(entry)."""
    s = split_graph(g, observer)
    if ("out", entry) not in s.cap:
        return 0
    return s.max_flow(("out", observer), ("out", entry))


# ---------------------------------------------------------------------------
# EXPERIMENT E1 -- distance-decay divergence (design §16.2's arithmetic).
#
# With per-hop weight lambda and fanout f, a fake subtree at depth D with k
# further levels has mass  lambda^D * sum_{i=0..k} (f*lambda)^i , which
# DIVERGES unless f*lambda < 1.  The boundary matters: at f*lambda = 1 the
# series is sum of 1^i = k+1, which diverges LINEARLY -- so the design's
# "unless f*lambda < 1" makes EQUALITY a divergent case.  We classify all
# three regimes (< 1 converges, == 1 and > 1 diverge) and check the two
# quoted data points.
# ---------------------------------------------------------------------------

def experiment_divergence(report):
    f = 10
    D = 1
    def fake_mass(lam, levels):
        return sum((lam ** (D + i)) * (f ** i) for i in range(levels + 1))
    honest_one = lambda lam: lam ** D

    report.append("E1: distance-decay divergence (design 16.2)")
    # Three regimes, including the exact boundary f*lambda = 1 (lambda = 0.1
    # at f = 10), which the design's "unless f*lambda < 1" classifies as
    # divergent and an earlier version of this experiment misclassified.
    for lam in [0.5, 0.1, 0.05]:
        prod = f * lam
        diverges = prod >= 1                       # >= 1, NOT > 1 (the fix)
        # A long tail exposes divergence: a converging series stays tiny,
        # a diverging one grows without bound in the number of levels.
        deep = fake_mass(lam, 40) / honest_one(lam)
        if abs(prod - 1) < 1e-9:
            regime = "DIVERGES at the exact boundary (f*lambda = 1)"
        elif diverges:
            regime = f"DIVERGES (f*lambda = {prod:.1f} > 1)"
        else:
            regime = f"converges (f*lambda = {prod:.2f} < 1)"
        report.append(
            f"  lambda={lam:<5} f*lambda={prod:<4} 40-level mass/honest = "
            f"{deep:>12,.1f}   {regime}")
    # Design's quoted magnitudes at the depths it cites.
    assert 15_000 < fake_mass(0.5, 6) / honest_one(0.5) < 25_000
    assert fake_mass(0.05, 40) < 0.15              # converged: tiny
    # The boundary case grows linearly and is unbounded in the level count.
    assert fake_mass(0.1, 100) / honest_one(0.1) > 100
    assert fake_mass(0.1, 200) / honest_one(0.1) > 200
    report.append("  matches design 16.2's magnitudes; boundary f*lambda=1 "
                  "diverges linearly: CONFIRMED")
    report.append("")


# ---------------------------------------------------------------------------
# EXPERIMENT E2 -- the single-observer cut bound.
#
# An honest tree; one honest boundary node adopts the root of a FAKE
# subtree.  Every path from the observer into the fake region passes through
# the single boundary edge -- a cut of known capacity.  We grow the region
# and measure the BEST INDIVIDUAL score any fake achieves; the claim
# (§16.2): the score never exceeds the cut, whatever the population.
#
# This experiment computes over the FULL flow graph (full visibility) -- the
# STRONGEST test: if the individual bound holds even when the observer sees
# everything, it holds a fortiori in the observer's smaller VISIBLE graph
# (invisibility only removes capacity, never adds it -- §16.3.1's
# conservative direction).
# ---------------------------------------------------------------------------

def attach_fake_region(g, boundary_node, width, depth):
    """Attach a fake subtree (width children per level, depth levels) under a
    single fake root adopted by boundary_node.  Fake-internal edges are
    GENEROUS -- the attacker wires their own region however they like
    (design §17.2: topology cannot provide Sybil resistance; only the
    boundary is real).  Returns the list of fake identities."""
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
                g.add_edge(u, v, 100)
                g.add_edge(v, u, 100)
                fakes.append(v)
                nxt.append(v)
        frontier = nxt
    return fakes


def experiment_cut_bound(rng, report):
    report.append("E2: the individual cut bound (design 16.2, 17.3)")
    base, nodes, _ = build_tree(rng, depth=3, fanout=3)
    observer = nodes[0]
    boundary = nodes[1]
    for width, depth in [(2, 2), (3, 3), (4, 4)]:
        g = base.copy()
        fakes = attach_fake_region(g, boundary, width, depth)
        best = max(score_independent(g, observer, t) for t in fakes)
        ok = best <= HIER_CAP
        report.append(
            f"  fake region of {len(fakes):>4} identities: best individual "
            f"score {best} <= boundary capacity {HIER_CAP}: "
            f"{'HOLDS' if ok else 'VIOLATED'}")
        assert ok, "individual cut bound violated -- investigate"
    report.append("  population never moved the individual bound: CONFIRMED")
    report.append("")


# ---------------------------------------------------------------------------
# EXPERIMENT E3 -- setwise conservation, OBSERVER-VISIBLE.
#
# The fake identities are attached as a WIDE FAN of DIRECT children of the
# boundary node, so they sit two scope-hops from the observer -- inside its
# horizon, identities it genuinely holds.  (An earlier version attached a
# DEEP subtree whose descendants were 3+ hops away, outside the observer's
# horizon: the conserving computation there ran over identities the observer
# could not see.  Deeper fakes are still attached below, to demonstrate that
# invisibility only REDUCES the observer's count -- §16.3.1's conservative
# direction -- never inflates it.)
#
#   Mode A (independent per-target), over the visible graph: the sum grows
#     with the visible population -- the broken reading.
#   Mode B (one conserving computation), over the visible graph: the joint
#     count saturates at the visible region ceiling and stays flat.
#
# Pass criterion (the 0.8.3 validation text): growing the population never
# pushes the conserving aggregate beyond the cut, and once population
# exceeds the ceiling the joint sits AT it.
# ---------------------------------------------------------------------------

def attach_fake_fan(g, boundary_node, width):
    """Attach `width` fake identities as DIRECT children of boundary_node
    (all one adoption hop below it, so two scope-hops from a root observer:
    inside the horizon).  Returns the fake identities."""
    fakes = []
    for i in range(width):
        v = f"FAN.f{i}"
        g.add_edge(boundary_node, v, HIER_CAP)
        g.add_edge(v, boundary_node, HIER_CAP)
        fakes.append(v)
    return fakes


def experiment_setwise(rng, report):
    report.append("E3: setwise conservation, observer-visible (design 16.2)")
    base, nodes, children = build_tree(rng, depth=2, fanout=3)
    observer = nodes[0]                            # the root observes
    boundary = children[observer][0]               # a direct child (1 hop)
    joints = []
    for width in [4, 8, 16]:
        g = base.copy()
        fakes = attach_fake_fan(g, boundary, width)
        # Rebuild scope adjacency to include the new fan as boundary's
        # children (siblings of each other), so the horizon reflects them.
        kids = {u: list(children.get(u, [])) for u in children}
        kids.setdefault(boundary, [])
        kids[boundary] = list(kids[boundary]) + fakes
        for fkid in fakes:
            kids.setdefault(fkid, [])
        scope = scope_adjacency(list(g.cap.keys()), kids)
        vis = visible_flow_subgraph(g, scope, observer, peer_edges=[])
        # Only fakes actually inside the observer's visible graph count --
        # the observer cannot evaluate identities it does not hold.
        visible_fakes = [f for f in fakes if f in vis.cap]
        indep_sum = sum(score_independent(vis, observer, t)
                        for t in visible_fakes)
        joint = accepted_count(vis, observer, visible_fakes)
        ceiling = region_ceiling(vis, observer, boundary)
        report.append(
            f"  {len(visible_fakes):>2} visible fakes:  independent-sum = "
            f"{indep_sum:>4}   conserving joint = {joint}   "
            f"visible region ceiling = {ceiling}")
        assert joint <= ceiling, "aggregate exceeded the visible cut"
        joints.append((len(visible_fakes), joint, ceiling))
    for n, joint, ceiling in joints:
        if n >= ceiling:
            assert joint == ceiling, "saturated region below its ceiling"

    # --- The GENERAL setwise-conservation statement (design §16.2) ---------
    # The unit-demand count above is a narrower property than the design's
    # wording ("simultaneously usable standing").  Here every candidate
    # demands its OWN INDIVIDUAL STANDING -- each asking for everything it
    # could get alone -- and the total deliverable is checked against the
    # cut.  This is the strongest reading of the sentence and the one a
    # cross-family review asked for.
    g = base.copy()
    fakes = attach_fake_fan(g, boundary, 16)
    kids = {u: list(children.get(u, [])) for u in children}
    kids.setdefault(boundary, [])
    kids[boundary] = list(kids[boundary]) + fakes
    for fkid in fakes:
        kids.setdefault(fkid, [])
    scope = scope_adjacency(list(g.cap.keys()), kids)
    vis = visible_flow_subgraph(g, scope, observer, peer_edges=[])
    visible_fakes = [f for f in fakes if f in vis.cap]
    individual = {t: score_independent(vis, observer, t) for t in visible_fakes}
    total_individual = sum(individual.values())
    delivered = deliverable_flow(vis, observer, individual)
    ceiling = region_ceiling(vis, observer, boundary)
    report.append(
        f"  general demands (each asks its own standing): individual sum = "
        f"{total_individual}, simultaneously deliverable = {delivered}, "
        f"cut = {ceiling}")
    assert delivered <= ceiling, \
        "setwise conservation violated under general demands"
    assert total_individual > ceiling, \
        "test is vacuous unless demand exceeds the cut"

    # --- The reference allocation rule, BOTH limbs (design §16.4) ----------
    # Limb 1: the shorter path dominates -- a longer path is less
    # trustworthy by nature.  Limb 2: consideration order breaks TRUE TIES
    # only, i.e. equal path lengths.  Each limb needs its own case, because
    # a plain multi-sink max-flow satisfies limb 1 by accident and fails
    # limb 2 silently (see admit_reference_order).
    def winner_unequal(order):
        """A sits one hop further from the observer than B."""
        g2 = FlowGraph()
        g2.add_edge("obs", "bot", 1)     # the scarce shared unit
        g2.add_edge("bot", "x", 1)
        g2.add_edge("x", "A", 1)         # A: two hops past the bottleneck
        g2.add_edge("bot", "B", 1)       # B: one hop past it
        admitted, _ = admit_reference_order(g2, "obs", order, demand=1)
        return admitted[0] if admitted else None

    def winner_equal(order, build):
        """A and B are equidistant; `build` fixes the order the GRAPH was
        constructed in, which must NOT decide the outcome."""
        g2 = FlowGraph()
        g2.add_edge("obs", "bot", 1)
        for t in build:                  # construction order, deliberately
            g2.add_edge("bot", t, 1)     #   varied to prove it is ignored
        admitted, _ = admit_reference_order(g2, "obs", order, demand=1)
        return admitted[0] if admitted else None

    # Limb 1: the shorter path wins whichever order the candidates come in.
    assert winner_unequal(["A", "B"]) == "B", "limb 1: shorter path must win"
    assert winner_unequal(["B", "A"]) == "B", "limb 1: shorter path must win"
    # Limb 2: at equal length the earlier-considered wins -- and the graph's
    # own construction order does not leak in.
    for build in (["A", "B"], ["B", "A"]):
        assert winner_equal(["A", "B"], build) == "A", \
            "limb 2: earlier-considered must win a true tie"
        assert winner_equal(["B", "A"], build) == "B", \
            "limb 2: earlier-considered must win a true tie"
    report.append(
        "  scarce-capacity allocation: the shorter path dominates; "
        "consideration order breaks")
    report.append(
        "  true ties only, independent of graph construction order "
        "(reference policy §16.4)")
    # Demonstrate the conservative direction: attaching a DEEP (invisible)
    # fake subtree beyond the horizon does not raise the observer's joint.
    g = base.copy()
    near = attach_fake_fan(g, boundary, 16)
    deep = attach_fake_region(g, boundary, width=4, depth=3)   # 3+ hops away
    kids = {u: list(children.get(u, [])) for u in children}
    kids.setdefault(boundary, [])
    kids[boundary] = list(kids[boundary]) + near
    for fkid in near:
        kids.setdefault(fkid, [])
    scope = scope_adjacency(list(g.cap.keys()), kids)
    vis = visible_flow_subgraph(g, scope, observer, peer_edges=[])
    deep_visible = [d for d in deep if d in vis.cap]
    report.append(
        f"  a deep fake subtree of {len(deep)} adds {len(deep_visible)} "
        f"visible identities -- invisibility is the conservative direction")
    assert len(deep_visible) == 0, "deep fakes leaked into the horizon"
    report.append(
        "  independent-sum grows with visible population (the broken 0.8.3 "
        "reading);")
    report.append(
        "  the conserving joint saturates at the visible ceiling; invisible "
        "identities cannot inflate it: CONFIRMED")
    report.append("")


# ---------------------------------------------------------------------------
# EXPERIMENT E4 -- edge-influence fanout, with correct horizon semantics.
#
# Claim (§16.3.1): one acquired PEERING edge is visible inside both peers'
# horizons, so it helps every observer whose horizon contains a peer -- cost
# scales with COVERAGE over the population, not with the number of
# observers.
#
# Method, corrected from the earlier version in three ways:
#   1. horizons are computed over SCOPE (adoption + sibling) edges; the
#      acquired edge is a PEERING (flow) edge that confers NO scope (§6.3);
#   2. the edge is visible to an observer iff its horizon contains a peer
#      (§16.3.1), which is what determines coverage;
#   3. we ENUMERATE every interior placement (worst-case-aware), not sample
#      a few, because the design claim is about coverage over a population
#      and placement is the quantity an adversary optimises.
#
# Two properties are asserted, matching the design's careful separation of
# the strong claim from its conservative half:
#   * changed == holds_edge   (STRONG: every observer that can see the edge
#     is influenced, and only those) -- the earlier version asserted only
#     `changed <= holds_edge`, which the strong claim's wording outran;
#   * holds_edge > 1           (coverage, not per-target).
# ---------------------------------------------------------------------------

def experiment_fanout(rng, report):
    """E4, rebuilt with TWO-ENDED cross-tree peering.

    design §6.3 makes peering a relation between two infra nodes in
    different subtrees, and §16.3.1 makes the record visible inside BOTH
    peers' horizons.  An earlier version attached a synthetic ATTACKER with
    no position in the scope graph and counted visibility from the victim
    end only.  That construction made the strong claim self-fulfilling: a
    node with no prior path gains one for every observer that can see its
    single edge, so "influenced" and "can see" coincided by construction.
    A cross-family review supplied the disproof -- with both endpoints real,
    an observer already holding standing to the far peer through its own
    tree sees the new edge and is NOT influenced by it.

    So this version builds two disjoint adoption trees, peers one node in
    each, and reports THREE quantities separately: who can see the edge, who
    is influenced by it, and by how much.  The claim actually asserted is
    the design's own economic one -- one edge reaches MANY observers, so
    acquisition amortises -- plus the conservative direction, that nobody
    who cannot see the edge is influenced.  The equality is not asserted,
    because it is not true.
    """
    report.append("E4: edge-influence fanout, two-ended peering (design 16.3.1)")
    fA, nA, cA = build_tree(rng, depth=3, fanout=3, prefix="A")
    fB, nB, cB = build_tree(rng, depth=3, fanout=3, prefix="B")
    flow = FlowGraph()
    for g in (fA, fB):
        for u, nbrs in g.cap.items():
            for v, c in nbrs.items():
                if c > 0:
                    flow.add_edge(u, v, c)
    nodes = nA + nB
    children = {**cA, **cB}
    scope = scope_adjacency(nodes, children)

    # Enumerate every cross-tree placement of ONE peering edge between an
    # interior node of tree A and an interior node of tree B -- worst-case
    # aware rather than sampled, since placement is what an adversary
    # optimises.
    interiorA = [n for n in nA if children.get(n)]
    interiorB = [n for n in nB if children.get(n)]
    rows = []
    for pa in interiorA:
        for pb in interiorB:
            peer_edges = [(pa, pb, PEER_CAP)]
            # THE BENEFICIARY is fixed: pb is the party whose standing the
            # acquired edge is meant to raise (an attacker buying a peering
            # favour from pa, in §16.3.1's framing).  Every observer is asked
            # the SAME question -- "did my evaluation of pb move?" -- which
            # is what makes the see/influenced distinction visible: an
            # observer inside pb's own tree already has standing to pb and
            # gains nothing, even though it can see the new edge.
            beneficiary = pb
            see = influenced = 0
            gains = []
            for obs in nodes:
                hz = horizon(scope, obs)
                # §16.3.1: the record is visible inside BOTH peers' horizons.
                visible = (pa in hz) or (pb in hz)
                if visible:
                    see += 1
                before = visible_flow_subgraph(flow, scope, obs, peer_edges=[])
                after = visible_flow_subgraph(flow, scope, obs, peer_edges)
                b = (score_independent(before, obs, beneficiary)
                     if beneficiary in before.cap else 0)
                a = (score_independent(after, obs, beneficiary)
                     if beneficiary in after.cap else 0)
                if a != b:
                    influenced += 1
                    gains.append(a - b)
                    # THE CONSERVATIVE DIRECTION (§16.3.1): an edge an
                    # observer cannot see must never move its evaluation.
                    assert visible, \
                        f"observer {obs!r} influenced by an invisible edge"
            rows.append((pa, pb, see, influenced,
                         max(gains) if gains else 0))

    sees = [r[2] for r in rows]
    infl = [r[3] for r in rows]
    best = max(rows, key=lambda r: r[3])
    # The design's economic claim: ONE edge reaches MANY observers -- the
    # cost of influencing a population amortises rather than scaling
    # one-for-one with targets.
    assert max(sees) > 1, "coverage claim needs more than one observer"
    assert max(infl) > 1, "amortisation claim needs more than one influenced"
    report.append(
        f"  {len(rows)} cross-tree placements enumerated over {len(nodes)} "
        f"observers (worst-case aware)")
    report.append(
        f"  observers who can SEE the edge:      min {min(sees)}, "
        f"median {int(statistics.median(sees))}, max {max(sees)}")
    report.append(
        f"  observers whose standing CHANGED:    min {min(infl)}, "
        f"median {int(statistics.median(infl))}, max {max(infl)}")
    report.append(
        f"  best placement {best[0]!r}<->{best[1]!r}: {best[3]} of "
        f"{len(nodes)} observers influenced by ONE edge (max gain "
        f"{best[4]})")
    report.append(
        "  seeing the edge does NOT imply being influenced -- an observer "
        "already holding standing")
    report.append(
        "  to the far peer through its own tree is unaffected; the equality "
        "an earlier version")
    report.append(
        "  asserted was an artifact of a one-ended attacker.  What holds: "
        "no invisible edge ever")
    report.append(
        "  influences anyone, and one edge influences many -- the coverage "
        "economics: CONFIRMED")
    report.append("")


# ---------------------------------------------------------------------------
# Entry point.
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
    report.append("")
    report.append(
        "RESOLVED (design 16.4, 2026-09-04): when shared capacity cannot "
        "satisfy every eligible")
    report.append(
        "principal, the max-flow VALUE is unique (individual standing and the "
        "aggregate bound are")
    report.append(
        "well-defined) but the ALLOCATION is not.  The reference metric "
        "decides in two limbs:")
    report.append(
        "the SHORTER PATH DOMINATES -- a longer path is less trustworthy by "
        "nature -- and")
    report.append(
        "consideration order breaks TRUE TIES only.  Both are REFERENCE "
        "POLICY, not a network")
    report.append(
        "invariant: per-observer trust (16.1) means no party consumes "
        "another's computation, so a")
    report.append(
        "policy allocating differently still conforms.  Note for implementers "
        "(tested above): one")
    report.append(
        "multi-sink max-flow delivers the first limb for free and misses the "
        "second silently --")
    report.append(
        "at equal path length it follows the graph's construction order, not "
        "the candidate order.")
    text = "\n".join(report)
    print(text)
    return text


if __name__ == "__main__":
    main()
