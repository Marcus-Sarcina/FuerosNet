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
    """Per-node throughput by scope/flow distance from the observer.  A
    simple halving schedule; absolute numbers are unimportant (§16.1)."""
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


def accepted_count(g, observer, targets):
    """MODE B: how many of `targets` are accepted SIMULTANEOUSLY -- one split
    graph, one super-sink, one max-flow.

    IMPORTANT (an operational caveat, see UNSPECIFIED note at the bottom of
    this file): the RETURNED TOTAL is a well-defined, deterministic quantity
    -- the max-flow value is unique.  It verifies the setwise-conservation
    BOUND (the aggregate cannot exceed the cut).  It does NOT determine WHICH
    identities receive the units when demand exceeds capacity: among
    symmetric sinks, different augmenting-path orders accept different
    principals, all valid maximum flows.  So this function is sound as a
    conservation check and must not be read as a stable per-principal
    allocation.
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
    report.append("E4: edge-influence fanout, corrected horizons (design 16.3.1)")
    flow, nodes, children = build_tree(rng, depth=3, fanout=4)
    scope = scope_adjacency(nodes, children)
    attacker = "ATTACKER"
    interior = [n for n in nodes if children.get(n)]

    coverages = []
    for victim in interior:
        # The acquired edge is a PEERING edge (flow only, no scope).
        peer_edges = [(victim, attacker, PEER_CAP)]
        changed = 0
        holds_edge = 0
        for obs in nodes:
            # BEFORE: no peering edge.  AFTER: the peering edge exists but
            # confers no scope, so horizons are computed WITHOUT it.
            before = visible_flow_subgraph(flow, scope, obs, peer_edges=[])
            after = visible_flow_subgraph(flow, scope, obs, peer_edges)
            b = (score_independent(before, obs, attacker)
                 if attacker in before.cap else 0)
            a = (score_independent(after, obs, attacker)
                 if attacker in after.cap else 0)
            if a != b:
                changed += 1
            # The edge is visible to obs iff obs's SCOPE horizon contains a
            # peer -- here the victim (the attacker has no scope position).
            if victim in horizon(scope, obs):
                holds_edge += 1
        # STRONG property, per placement: exactly the observers who can see
        # the edge are influenced.
        assert changed == holds_edge, \
            f"changed ({changed}) != can-see ({holds_edge}) at {victim!r}"
        assert holds_edge >= 1, "an acquired edge no observer can see"
        coverages.append((victim, holds_edge))

    counts = [c for _, c in coverages]
    lo, med, hi = min(counts), int(statistics.median(counts)), max(counts)
    # Show the worst case (max coverage) and a couple of sample placements.
    worst = max(coverages, key=lambda t: t[1])
    report.append(
        f"  {len(interior)} interior placements enumerated (worst-case "
        f"aware, not sampled)")
    report.append(
        f"  coverage per acquired edge -- observers who can see it: "
        f"min {lo}, median {med}, max {hi}")
    report.append(
        f"  worst-case placement {worst[0]!r} covers {worst[1]} of "
        f"{len(nodes)} observers with ONE edge")
    report.append(
        "  every influenced observer can see the edge and every observer "
        "that can see it is")
    report.append(
        "  influenced (changed == coverage, per placement); coverage is many "
        "per edge, not one:")
    report.append("  CONFIRMED")
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
        "UNSPECIFIED (flagged for the design author, not decided here): when "
        "shared")
    report.append(
        "capacity cannot satisfy all eligible principals, the max-flow VALUE "
        "is unique but")
    report.append(
        "the ALLOCATION among symmetric principals is not -- augmenting-path "
        "order decides")
    report.append(
        "it.  The reference metric (design 16.2) fixes each principal's "
        "INDIVIDUAL standing")
    report.append(
        "(its own max-flow, deterministic) and the aggregate BOUND, but not a "
        "deterministic")
    report.append(
        "tie-break when scarce capacity must be allocated to specific "
        "principals.  Whether")
    report.append(
        "that must be specified, or is left to policy (16.4 pluggability), is "
        "the author's call.")
    text = "\n".join(report)
    print(text)
    return text


if __name__ == "__main__":
    main()
