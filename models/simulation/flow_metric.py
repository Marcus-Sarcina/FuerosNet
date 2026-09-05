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
  E3. Setwise conservation (design §16.2, normative since 2026-09-03),
      tested BEYOND THE HORIZON where the metric actually rations
      (§16.2.1): for any SET of identities behind a cut, their
      SIMULTANEOUSLY USABLE standing totals at most the cut's capacity --
      one computation, shared capacity.  Evaluating each identity with its
      own independent max-flow instead lets the same capacity count once
      per identity, and the aggregate grows with population.
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
  * TRUST-CAPACITY topology -- adoption edges AND peering edges, carrying
    the same capacity: design §16.2.1 [author, 2026-09-04] says the metric
    distinguishes INSIDE the horizon from BEYOND it, never edge kind from
    edge kind, and beyond the horizon trust flows equally over the
    hierarchical and the PoP/peering graphs.  It decides how much trust can
    FLOW.
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

WHAT THIS FILE DOES NOT MODEL, stated so the omissions are visible:
  * PROOF-OF-PRESENCE EDGES have no separate representation.  §16.2.1 puts
    them in the same class as peering edges for DISTANCE and CAPACITY, so
    peering edges stand in for both wherever those two are what matter.
    **They are not alike in VISIBILITY**, and that is a real limit of this
    file: a peering record is "visible inside the two peers' horizons and
    nowhere else" (§16.3), while presence records are pull, not push --
    "fetched on demand by evaluators" (§15's propagation classes), with no
    horizon bound.  Modelling PoP as peering therefore imports the narrower
    rule, which is why how far an observer's graph reaches is a PARAMETER
    here (`reach`) rather than a constant: §16.2.1 makes that reach
    available evidence, not a protocol quantity.  A model that priced
    ATTESTATION (met vs merely in-horizon, §16.2.1) would need them
    distinct on a third axis again.
  * ARCHIVE EVIDENCE is absent, faithfully: §16.2.1 makes an archive
    adoption-time REVIEW rather than a standing edge, so it creates no
    capacity for a flow calculation to find.

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
are outside its reach, per the plan's "what this plan cannot do".  E4
enumerates EVERY cross-tree peering placement in one toy topology rather
than sampling -- but one toy topology is not a population model, so E4
demonstrates that amortisation exists and does not measure the coverage
economics 16.3.1 argues.  Its own report says so.
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
# NOTE ON VALIDATION: three independent cross-family reviews have checked
# max_flow() against exhaustive brute-force minimum cuts on freshly generated
# random directed graphs (700, 600 and 600 graphs of 2-7 vertices, the last
# including antiparallel capacities) -- zero discrepancies in all three.  The
# algorithm below is therefore not where a defect would hide; every defect
# those reviews DID find was in the modelling around it.
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

# Edge capacity is NOT differentiated by edge kind (design §16.2.1): beyond
# the horizon trust flows equally over hierarchical and PoP/peering edges,
# and inside the horizon hierarchical edges are unthrottled (which the node
# capacities express, not these).  An earlier version gave peering a lower
# capacity; §16.3 now states there is no ratio between the two kinds, set or
# unset, a peering edge and a PoP edge being the same kind of edge.  What is
# peculiar to peering is off-protocol and stays between the two peers.
HIER_CAP = 10   # capacity of an adoption (hierarchical) edge
PEER_CAP = 10   # a peering edge is worth what any edge at its distance is

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


def visible_flow_subgraph(flow, scope_adj, observer, peer_edges, reach=0):
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

    `reach` -- HOW MANY FURTHER SHELLS THE OBSERVER'S EVIDENCE COVERS.
    reach=0 is the floor above: the stored horizon plus the far endpoints of
    peering edges it can see, and nothing more.  Each additional shell adds
    the edges around those endpoints.

    THE FLOOR IS NOT THE RULE [author, 2026-09-04]: *"It depends what
    relationships you are aware of.  You can discern some of a foreign
    subtree's structure from locator data, so it is entirely plausible that
    you could know that some newly encountered node is two or three edges
    from a PoPmate in that foreign patronage graph ... calculating this is
    not a requirement, but it seems better to make a generalizable rule and
    let later developers use the data available to them if they care to
    while keeping the same proven flow metric."*

    So `reach` is a MODELLING PARAMETER STANDING FOR AVAILABLE EVIDENCE, not
    a protocol quantity: the metric is identical at every value, and what
    changes is how much graph an implementation managed to populate.  A
    cross-family review found the earlier fixed one-shell view could not
    express the case setwise conservation is actually about -- a whole
    region behind ONE acquired edge -- because the region's interior was
    always invisible.  E3 needs reach>=1 for exactly that reason.
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
    # Outward shells: HIERARCHICAL EDGES ONLY.  The author's ruling names the
    # mechanism precisely -- *"you can discern some of a foreign subtree's
    # structure from LOCATOR DATA, so it is entirely plausible that you could
    # know that some newly encountered node is two or three edges from a
    # PoPmate in that foreign PATRONAGE graph"* -- and locators expose
    # patronage structure, not peering records.
    #
    # PEERING VISIBILITY DOES NOT COMPOSE, and this is the security property,
    # not a modelling nicety.  §16.3: a peering record is visible "inside the
    # two peers' horizons and NOWHERE ELSE"; §16.3.1: an edge an observer
    # cannot see cannot raise that observer's cut.  So a peering edge enters
    # this graph by ONE route only -- the endpoint-in-horizon rule applied
    # above -- and learning a node through some other relationship never
    # confers sight of the peering records around it.
    #
    # A cross-family review found the earlier version expanding over peering
    # too, and gave the minimal counterexample: with O--H adoption, peering
    # H--G visible and peering G--X invisible, reach=1 pulled G--X in and
    # moved X's standing from 0 to 8 -- an edge the design says O never
    # learns about, changing O's own computation.
    #
    # `expanded` stops a node's edges being added twice: add_edge ACCUMULATES
    # capacity, so re-expanding a node would silently double its edges.
    expanded = set(hz)
    for _ in range(reach):
        frontier = [u for u in list(sub.cap) if u not in expanded]
        for u in frontier:
            expanded.add(u)
            for v, c in flow.cap.get(u, {}).items():
                if c > 0 and sub.cap.get(u, {}).get(v, 0) == 0:
                    sub.add_edge(u, v, c)
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

def node_capacity(dist, in_horizon=False):
    """Per-node throughput by TRUST-LANDSCAPE distance (see landscape_distance).

    THROTTLING AND DISTANCE ARE TWO DIFFERENT FUNCTIONS, and this file keeps
    them apart because the design does.  design §16.2.1: hierarchical edges
    are "unthrottled out to the two-edge horizon ... beyond that horizon is
    where trust becomes throttled" -- so what decides whether a node is
    rationed is INSIDE-OR-BEYOND, while its DISTANCE decides how much.
    Distance 1 contains both kinds: every horizon member (unthrottled, being
    inside) and any counterparty the observer met itself who is NOT in the
    horizon (throttled, being outside).  See the note in the report.

    MODELLER'S CHOICE OF SCHEDULE, NOT FROM THE DESIGN.  Advogato decreases
    capacity with distance from the seed and this file uses a halving
    schedule.  The design fixes no vertex-capacity schedule -- trust policy
    is local and pluggable (§16.2, §16.4) -- so every absolute number is
    conditional on this choice.  What the experiments test is RELATIVE
    behaviour under ONE schedule held fixed.

    WHAT IS *NOT* A MODELLER'S CHOICE is the horizon being unthrottled.
    design §16.2.1: hierarchical edges are unthrottled out to the two-edge
    horizon and the metric throttles only beyond it, so nothing inside the
    horizon is rationed by flow.  NOTE this is not the same as the horizon
    sitting at the ORIGIN, which an earlier version of this file assumed:
    the observer alone is distance 0 and the horizon is distance 1, one
    collapsed step out (see landscape_distance).  Unthrottled and
    equidistant-from-the-origin are two different statements, and only the
    first is true of the horizon.
    """
    if dist == 0 or in_horizon:
        return UNTHROTTLED             # self, or inside the horizon: the
                                       # metric is not the binding constraint
    return max(32 >> dist, 1)          # beyond it: 16, 8, 4, 2, 1, 1, ...


UNTHROTTLED = 10 ** 9    # stands in for "the flow metric does not ration here"


def landscape_distance(scope_adj, flow, observer, peer_edges):
    """Distance in the TRUST LANDSCAPE, per design §16.2.1 [author].

    NOT hops in the graph.  The rule:

      * the OBSERVER ALONE is the origin, distance 0;
      * DISTANCE 1 is the observer's whole two-edge patron/sibling HORIZON,
        collapsed into one step -- every horizon member is equidistant --
        TOGETHER WITH anyone the observer has met itself (a PoP
        counterparty or a peer), in or out of the horizon;
      * DISTANCE 2 is one step further: a node outside the horizon that is
        one edge from a horizon member or from one of the observer's own
        counterparties.  And so on outward.  Beyond the horizon,
        **trust flows equally over the hierarchical and the
        proof-of-presence/peering graphs** [author, 2026-09-04] -- the
        metric distinguishes inside from beyond, never edge kind from edge
        kind.  So the outward walk below deliberately treats adoption and
        peering edges alike.

    So the observer's patron's sibling is at distance 1 (a horizon member)
    while THEIR PoP counterparty is at distance 2 -- outside the horizon,
    one edge from someone at 1.  The horizon flattens; the world past it
    does not.  (An earlier version of this file put the whole horizon at
    distance 0; the author corrected it -- only the observer is the origin.)

    An earlier version of this file measured plain BFS hops in the flow
    graph, which graded the INSIDE of the horizon -- a different landscape
    from the one the design describes, where the horizon is a single point.

    `peer_edges` is the (u, v, cap) list; `flow` supplies PoP/adoption
    adjacency for the outward walk.
    """
    dist = {n: 1 for n in horizon(scope_adj, observer)}   # the horizon shell
    # The observer's OWN counterparties are at distance 1 whether or not they
    # are in the horizon.  Reading them off `peer_edges` is sound where the
    # outward walk below is not: an edge incident to the observer is one the
    # observer is a party to, so it cannot be invisible to them.
    for (u, v, _c) in peer_edges:                          # own counterparties
        if u == observer:
            dist.setdefault(v, 1)
        if v == observer:
            dist.setdefault(u, 1)
    dist[observer] = 0                                     # the origin, alone
    # Beyond the horizon EVERY edge counts the same -- adoption, sibling,
    # peering, and (in a richer model) explicit PoP edges alike (§16.2.1).
    # But counting the same is not being VISIBLE the same:
    # THE ADJACENCY IS THE OBSERVER'S OWN GRAPH, NOTHING WIDER.  `flow` is
    # what this observer can see (visible_flow_subgraph in the RHTN-shaped
    # experiments), and the outward walk uses only that.  An earlier version
    # also folded in the raw `peer_edges` list here -- the caller's omniscient
    # view -- which let an edge the observer cannot see SHORTEN a landscape
    # distance and so raise a relay's node capacity, without that edge ever
    # entering the capacity graph.  Measured by a cross-family review: a chain
    # whose deepest relay sat at distance 4 (capacity 2) had it moved to
    # distance 2 (capacity 8) by one invisible edge, lifting the target's
    # standing from 2 to 4.  Distance is computed from the same graph the flow
    # is, or §16.3.1's observer-relative bound is not observer-relative.
    acq = {u: set() for u in flow.cap}
    for u, nbrs in flow.cap.items():
        for v, c in nbrs.items():
            if c > 0:
                acq[u].add(v)
                acq.setdefault(v, set()).add(u)
    frontier = {n for n, d0 in dist.items() if d0 == 1}
    d = 1
    while frontier:
        d += 1
        nxt = set()
        for u in frontier:
            for v in acq.get(u, ()):
                if v not in dist:
                    dist[v] = d
                    nxt.add(v)
        frontier = nxt
    return dist


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


def split_graph(g, observer, scope_adj=None, peer_edges=()):
    """Node splitting with distance-based node capacities.  Nodes become
    ("in", u) and ("out", u); the observer's own throughput is unbounded
    (capping the evaluator would be self-limiting).

    DISTANCE IS THE TRUST-LANDSCAPE DISTANCE when `scope_adj` is supplied
    (design §16.2.1): the observer alone is the origin, its whole two-edge
    horizon is distance 1, and only beyond the horizon does the metric
    ration.  Without `scope_adj` -- the small synthetic graphs in the
    allocation tests, which have no subnet structure -- it falls back to
    plain graph hops, where the observer alone is the origin.  Passing scope
    is what the RHTN-shaped experiments do; the fallback exists so the
    allocation unit tests can use bare graphs.

    THE HORIZON IS COLLAPSED TO A SINGLE EDGE [author, 2026-09-04]:
    *"In-horizon edges are collapsed to a single edge (distance 0 -> distance
    1) ... nodes within each other's horizons are always aware of each other,
    capable of point-to-point communications.  Even for a secondary patronage
    relation (two edges on the hierarchy graph), there is a direct
    relationship that justifies collapsing this distance.  In short, the
    distance from a node to the edge of its trust horizon is 1."*

    So the observer holds ONE edge to each horizon member and the horizon's
    internal topology is NOT a chain of throttling edges -- it is not
    represented in the flow graph at all.  Edges LEAVING the horizon are
    kept at their own capacity, which is where the metric starts working.

    WHY THIS MATTERS BEYOND TIDINESS.  An earlier version built the
    uncollapsed graph -- observer -> child -> grandchild as two capacity
    edges -- and a cross-family review found two consequences.  (1) The
    in-horizon hierarchical edge became the binding chokepoint in E3, so the
    headline conservation number was produced by throttling a relation the
    design says is not throttled.  (2) Ranking candidates by hops in that
    graph re-graded the inside of the horizon, which §16.2.1 flattens: two
    candidates at landscape distance 2 measured 3 and 5 and the deeper one
    lost every time, whatever order the caller passed.  Collapsing fixes
    both at the source, because hops in the COLLAPSED graph ARE the
    landscape distance -- §16.4's ranking key needs no special case.
    """
    if scope_adj is not None:
        dist = landscape_distance(scope_adj, g, observer, peer_edges)
        hz = horizon(scope_adj, observer)
    else:
        dist = distances_from(g, observer)
        hz = {observer}                # bare graphs: only self is "inside"
    s = FlowGraph()
    for u in g.cap:
        if u not in dist:
            continue
        ncap = node_capacity(dist[u], u in hz)
        s.add_edge(("in", u), ("out", u), ncap)
        for v, c in g.cap[u].items():
            if c > 0 and v in dist:
                if u in hz and v in hz:
                    continue          # collapsed away: the horizon is 1 step
                s.add_edge(("out", u), ("in", v), c)
    for m in hz:                      # ... and this is that one step.  The
        if m != observer:             # observer's own edges INTO the horizon
            s.add_edge(("out", observer), ("in", m), UNTHROTTLED)
    return s                          # are replaced by it, not added to it.


def score_independent(g, observer, target, scope_adj=None, peer_edges=()):
    """MODE A: the target's individual score = max flow observer->target =
    the min-cut between them.  E2 shows this per-identity bound holds even
    with full visibility."""
    s = split_graph(g, observer, scope_adj, peer_edges)
    if ("in", target) not in s.cap:
        return 0
    return s.copy().max_flow(("out", observer), ("in", target))


def admit_reference_order(g, observer, candidates, demand=1,
                          scope_adj=None, peer_edges=()):
    """MODE B: admit candidates against ONE shared residual, in REFERENCE order.

    Returns (admitted, total) -- the candidates that received flow, in the
    order they were admitted, and the total flow delivered.

    THE REFERENCE-POLICY ALLOCATION RULE (design §16.4, ruled 2026-09-04),
    in three passes -- and the FIRST is the metric itself:

      0. AVAILABLE FLOW RANKS FIRST.  The total capacity reaching a
         candidate is what the metric measures (§16.2), so a candidate
         carrying more flow outranks one carrying less whatever their
         distances.  Preferring a nearer candidate over a better-supported
         one would substitute the tie-break for the measurement.  This pass
         is invisible in the single-chokepoint examples below -- every
         candidate behind one saturated cut carries the same flow, so it
         decides nothing there -- which is exactly why it is easy to omit,
         and why an earlier version of this file did.
      1. THE SHORTER PATH DOMINATES among candidates the flow ranks
         equally -- *a longer path is less trustworthy by nature*
         [author], the reasoning §16.2 applies to distance decay, applied
         to allocation instead of weight.
      2. CONSIDERATION ORDER BREAKS TRUE TIES ONLY: candidates equal on
         both flow and path length.

    WHY THIS NEEDS EXPLICIT CODE.  A plain multi-sink max-flow -- add every
    candidate's drain edge, run one max-flow -- looks like it implements
    all three passes and implements only one, by accident:

      * PASS 1 falls out of Edmonds-Karp, which augments along the SHORTEST
        path first.  Two successive cross-family reviews exercised this;
        the counterexample below returns B under either candidate order,
        which the path-length pass says is correct.

            observer -1-> bottleneck -1-> x -1-> A     (A: longer path)
                          bottleneck -1-> B            (B: shorter path)

      * PASS 2 does NOT.  In a plain max-flow the tie is decided by the
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
    s = split_graph(g, observer, scope_adj, peer_edges)
    SINK = ("sink", None)
    source = ("out", observer)
    # The three ranking keys, measured on the PRE-ALLOCATION graph, because
    # a candidate's standing and position are properties of the topology,
    # not of who happened to be served first:
    #   pass 0  available flow  -- DESCENDING (more flow ranks earlier)
    #   pass 1  path length     -- ascending (shorter is more trustworthy)
    #   pass 2  consideration index -- ascending (true ties only)
    dist = distances_from(s, source)
    ranked = sorted(
        [(i, c) for i, c in enumerate(candidates) if ("in", c) in s.cap],
        key=lambda ic: (-score_independent(g, observer, ic[1],
                                          scope_adj, peer_edges),
                        dist.get(("in", ic[1]), float("inf")),
                        ic[0]))
    admitted, total = [], 0
    for _, c in ranked:
        # DRAIN FROM ("in", c), NOT ("out", c).  Node capacity models what a
        # node may RELAY (§ the node-splitting note above); a candidate is
        # the DESTINATION here, not a relay, so its own throughput limit must
        # not gate what it receives.  score_independent likewise terminates
        # at ("in", target), and an earlier version of this file drained
        # from ("out", c) -- which silently charged every candidate its own
        # relay capacity and made the joint computation disagree with the
        # individual one even with a single candidate and no contention
        # (measured: individual 8, joint 4).  A cross-family review found it.
        s.add_edge(("in", c), SINK, demand)
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


def deliverable_flow(g, observer, demands, scope_adj=None, peer_edges=()):
    """Total flow deliverable to a SET of candidates under ARBITRARY demands.

    `demands` maps target -> the capacity that target may draw.  This is the
    general form of design §16.2's setwise-conservation sentence: whatever
    each identity behind a cut wants to use, what the set can use AT ONCE is
    bounded by the cut.  Passing each candidate's own individual standing as
    its demand is the strongest reading -- every identity asking for
    everything it could get alone -- and is what experiment_setwise checks.
    """
    s = split_graph(g, observer, scope_adj, peer_edges)
    SINK = ("sink", None)
    for t, d in demands.items():
        # ("in", t), not ("out", t) -- see admit_reference_order: a candidate
        # receives as a destination and must not be charged its own relay
        # capacity, or this disagrees with score_independent.
        if ("in", t) in s.cap and d > 0:
            s.add_edge(("in", t), SINK, d)
    return s.max_flow(("out", observer), SINK)


def region_ceiling(g, observer, entry, scope_adj=None, peer_edges=()):
    """design §16.2's sentence made a number: "the entire subtree inherits
    at most what flows through that one vertex".  Every identity behind
    `entry` drains through entry's out-node, so no set of them can
    simultaneously use more than the max flow observer -> out(entry)."""
    s = split_graph(g, observer, scope_adj, peer_edges)
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
    """E2, with the entry adoption in SCOPE as well as in the flow graph.

    THE BOUND IS A BEYOND-THE-HORIZON CLAIM, and the experiment has to be
    built where the claim applies.  §16.2.1: the metric does not ration
    inside the horizon, so a region whose entry point the observer can reach
    in two scope edges is unthrottled and no cut bounds it -- correctly, and
    by design: inside your own subnet you are relying on participation
    (§16.2.1) and on hierarchy-specific remedies like disavowal (§12.7),
    not on the flow metric.

    An earlier version placed the boundary on a DIRECT CHILD of the observer
    and then built the scope graph from the honest `children` table only,
    which silently dropped the very adoption `attach_fake_region` says
    occurred.  A cross-family review caught it: restore that one edge and
    the fake root sits two scope edges from the observer -- inside the
    horizon -- so its score is UNTHROTTLED and E2's "CONFIRMED" evaporates.
    The bound was being produced by the omission, not by the metric.

    So the boundary here sits at scope distance 2 (the horizon's edge) and
    its adoption of the fake root IS in scope, putting the fake root at
    distance 3: outside, where the cut is what answers.  The fake region's
    OWN internal adoptions stay out of scope, which is faithful -- the
    observer holds topology for its horizon (§15.1), not for a stranger's
    subtree.
    """
    report.append("E2: the individual cut bound (design 16.2, 17.3)")
    base, nodes, children = build_tree(rng, depth=3, fanout=3)
    observer = nodes[0]
    honest_scope = scope_adjacency(nodes, children)
    # The boundary sits at scope distance exactly 2 -- the horizon's edge --
    # so the node it adopts lands at 3, just outside.
    ring = {observer}
    shell = {observer}
    for _ in range(2):
        shell = {w for v in shell for w in honest_scope[v]} - ring
        ring |= shell
    boundary = min(shell)                 # deterministic pick from the ring
    for width, depth in [(2, 2), (3, 3), (4, 4)]:
        g = base.copy()
        fakes = attach_fake_region(g, boundary, width, depth)
        # Scope INCLUDES the entry adoption -- flow and scope must agree about
        # an edge the experiment says exists -- and excludes the fake region's
        # internal adoptions, which this observer holds no topology for.
        kids = {k: list(v) for k, v in children.items()}
        kids.setdefault(boundary, []).append("FAKE")
        kids["FAKE"] = []
        scope = scope_adjacency(list(g.cap.keys()), kids)
        assert "FAKE" not in horizon(scope, observer), \
            "the fake root must sit OUTSIDE the horizon or the metric does " \
            "not ration it and there is no bound to test"
        best = max(score_independent(g, observer, t, scope) for t in fakes)
        ok = best <= HIER_CAP
        report.append(
            f"  fake region of {len(fakes):>4} identities: best individual "
            f"score {best} <= boundary capacity {HIER_CAP}: "
            f"{'HOLDS' if ok else 'VIOLATED'}")
        assert ok, "individual cut bound violated -- investigate"
    report.append("  population never moved the individual bound: CONFIRMED")

    # --- WHERE THE BOUND STOPS APPLYING, stated as a measurement -----------
    # The same construction with the boundary on a DIRECT CHILD of the
    # observer.  With the entry adoption faithfully in scope, the fake root
    # is then two scope edges away -- INSIDE the horizon -- and is
    # unthrottled.  That is not a violation of §16.2's bound; it is the
    # bound's edge: §16.2.1 says the metric does not ration inside the
    # horizon, so there is nothing there for a cut to bound.  What protects
    # an observer there is participation in its own subnet (§16.2.1) and
    # hierarchy-specific remedy (§12.7), not flow.
    #
    # This case is what makes the placement above load-bearing rather than
    # incidental: run E2 at a boundary inside the horizon and it "fails",
    # and the failure is the design speaking correctly.
    inside_boundary = children[observer][0]
    g = base.copy()
    fakes = attach_fake_region(g, inside_boundary, 2, 2)
    kids = {k: list(v) for k, v in children.items()}
    kids.setdefault(inside_boundary, []).append("FAKE")
    kids["FAKE"] = []
    scope_inside = scope_adjacency(list(g.cap.keys()), kids)
    assert "FAKE" in horizon(scope_inside, observer), \
        "a direct child's adoptee must be inside the observer's horizon"
    inside_best = max(score_independent(g, observer, t, scope_inside)
                      for t in fakes)
    assert inside_best == UNTHROTTLED, \
        "inside the horizon the metric must not ration -- if this bounds, " \
        "the horizon is being throttled somewhere it should not be"
    report.append(
        "  the same region entered from INSIDE the horizon is unthrottled, "
        "not bounded --")
    report.append(
        "  the bound is a BEYOND-the-horizon claim, and this is its edge "
        "(design 16.2.1)")
    report.append("")


# ---------------------------------------------------------------------------
# EXPERIMENT E3 -- setwise conservation, OBSERVER-VISIBLE.
#
# THE CONSTRUCTION IS A REGION BEHIND ONE ACQUIRED EDGE.  That is the only
# shape in which conservation says anything: a set of identities sharing a
# chokepoint the attacker had to buy once.
#
#   Mode A (independent per-target), over the visible graph: the sum grows
#     with the visible population -- the broken reading.
#   Mode B (one conserving computation), over the visible graph: the joint
#     saturates at the region's cut and stays flat.
#
# TWO EARLIER CONSTRUCTIONS FAILED, AND BOTH FAILURES ARE INSTRUCTIVE.
#
#   (a) A fan INSIDE the observer's horizon.  The metric does not ration
#       inside the horizon at all (§16.2.1), so this tested conservation in
#       the one region where conservation is not the operative constraint.
#
#   (b) A fan beyond the horizon, but reached by ONE PEERING EDGE EACH.
#       This looked right and passed, and a cross-family review showed the
#       pass was an artifact: the binding chokepoint was not the acquired
#       edges but the observer's own in-horizon hierarchical edge, throttled
#       at HIER_CAP by a graph that had not yet collapsed the horizon.
#       Collapse it -- as the design says to -- and the cut goes unbounded,
#       the joint tracks the population exactly, and the experiment trips
#       its own "test is vacuous unless demand exceeds the cut" guard.
#       An attacker who buys N edges SHOULD get N edges' worth; there was
#       never a conservation claim there to demonstrate.
#
# What (b) got right is that the region must be BEYOND the horizon and
# VISIBLE.  What it got wrong is that a shared cut needs identities that
# SHARE something -- hence one gate, bought once, with the region behind it.
# ---------------------------------------------------------------------------

def attach_region_behind(g, gate, width):
    """A fake region of `width` identities behind a single `gate` node.

    Fake-internal edges are GENEROUS (a clique at capacity 100): the attacker
    wires their own region however they like, and design §17.2 is explicit
    that topology cannot provide Sybil resistance -- only the boundary is
    real.  Returns the fake identities.
    """
    fakes = []
    for i in range(width):
        v = f"REGION.f{i}"
        g.add_edge(gate, v, 100)
        g.add_edge(v, gate, 100)
        fakes.append(v)
    for a in fakes:                    # ... and richly interconnected
        for b in fakes:
            if a != b:
                g.add_edge(a, b, 100)
    return fakes


def experiment_setwise(rng, report):
    """E3, rebuilt for the collapsed horizon (design §16.2.1) [author].

    THE FAITHFUL CONSTRUCTION.  The attacker obtains ONE peering edge, from
    a horizon member H to a gate node G that holds no position in the
    observer's subnet, and hangs an arbitrarily large region behind G.  The
    region is then:

      * VISIBLE -- H is in the observer's horizon, so the observer sees the
        peering record and G with it, and (given evidence reaching one shell
        further, `reach`) the region behind G;
      * BEYOND the horizon -- peering confers no scope (§6.3), so G is at
        landscape distance 2 and the metric DOES ration it;
      * behind ONE CUT the attacker bought ONCE.

    The cut here is G's relay capacity at its distance, not the peering
    edge's nominal capacity -- node_capacity(2) = 8 binds below PEER_CAP =
    10 -- which is the node-splitting note doing its job: what a region
    inherits is what its gate can pass, and the gate is throttled by how far
    away it is.
    """
    report.append("E3: setwise conservation beyond the horizon (design 16.2, 16.2.1)")
    # depth 3: the root's two-edge horizon reaches its grandchildren, so
    # great-grandchildren exist OUTSIDE it -- needed for the invisibility
    # case below, which peers to a node the observer cannot see.
    base, nodes, children = build_tree(rng, depth=3, fanout=3)
    observer = nodes[0]                       # the root observes
    scope = scope_adjacency(nodes, children)
    hz = horizon(scope, observer)
    inside = children[observer][0]            # a horizon member: the peer
    outside = next(n for n in nodes if n not in hz)   # NOT in the horizon
    GATE = "GATE"

    joints = []
    for width in [4, 8, 16, 32]:
        g = base.copy()
        fakes = attach_region_behind(g, GATE, width)
        peer_edges = [(inside, GATE, PEER_CAP)]      # ONE acquired edge
        # reach=1: the observer's evidence covers the shell past the gate.
        # See visible_flow_subgraph -- how much graph an evaluator can
        # populate is available evidence, not a protocol quantity [author].
        vis = visible_flow_subgraph(g, scope, observer, peer_edges, reach=1)
        visible_fakes = [f for f in fakes if f in vis.cap]
        indep_sum = sum(score_independent(vis, observer, t, scope, peer_edges)
                        for t in visible_fakes)
        joint = admit_reference_order(vis, observer, visible_fakes, demand=1,
                                      scope_adj=scope, peer_edges=peer_edges)[1]
        ceiling = region_ceiling(vis, observer, GATE, scope, peer_edges)
        report.append(
            f"  {len(visible_fakes):>2} identities behind ONE acquired edge:  "
            f"independent-sum = {indep_sum:>4}   conserving joint = {joint}"
            f"   cut at the gate = {ceiling}")
        assert len(visible_fakes) == width, "the region must all be visible"
        assert joint <= ceiling, "aggregate exceeded the cut"
        joints.append((width, joint, ceiling))
    assert joints[-1][1] == joints[-2][1], \
        "the joint must stop growing once the region exceeds its cut"

    # --- The GENERAL setwise-conservation statement (design §16.2) ---------
    # Every candidate demands its OWN INDIVIDUAL STANDING -- each asking for
    # everything it could get alone -- which is the strongest reading of
    # "simultaneously usable standing".
    g = base.copy()
    fakes = attach_region_behind(g, GATE, 32)
    peer_edges = [(inside, GATE, PEER_CAP)]
    vis = visible_flow_subgraph(g, scope, observer, peer_edges, reach=1)
    individual = {t: score_independent(vis, observer, t, scope, peer_edges)
                  for t in fakes}
    total_individual = sum(individual.values())
    delivered = deliverable_flow(vis, observer, individual, scope, peer_edges)
    ceiling = region_ceiling(vis, observer, GATE, scope, peer_edges)
    report.append(
        f"  general demands (each asks its own standing): individual sum = "
        f"{total_individual}, simultaneously deliverable = {delivered}, "
        f"cut = {ceiling}")
    assert delivered <= ceiling, \
        "setwise conservation violated under general demands"
    assert total_individual > ceiling, \
        "test is vacuous unless demand exceeds the cut"

    # --- INVISIBILITY is the conservative direction (design §16.3.1) -------
    # The same region, gated instead behind a node OUTSIDE the observer's
    # horizon: the observer cannot see that peering record at all, so
    # nothing behind it ever enters its graph.
    g = base.copy()
    fakes = attach_region_behind(g, GATE, 32)
    far_edges = [(outside, GATE, PEER_CAP)]
    vis_far = visible_flow_subgraph(g, scope, observer, far_edges, reach=1)
    leaked = [f for f in fakes if f in vis_far.cap]
    report.append(
        f"  the same {len(fakes)} gated behind a node OUTSIDE the horizon: "
        f"{len(leaked)} visible -- invisibility is the conservative direction")
    assert leaked == [], "identities beyond an invisible edge leaked in"

    # --- INVISIBILITY DOES NOT COMPOSE (design §16.3, §16.3.1) -------------
    # The case above uses ONE isolated invisible edge, and cannot catch a
    # leak that needs a VISIBLE edge to trigger it: no legitimate edge pulls
    # an endpoint into the frontier, so a faulty outward expansion never
    # fires.  A cross-family review supplied the case that does, and it is
    # kept here because it is the security property, not a nicety --
    # §16.3.1: an edge an observer cannot see cannot raise that observer's
    # cut, at any reach.
    #
    #     O --adoption-- H          H is in O's horizon
    #     H --peering--  G          VISIBLE to O (an endpoint is in-horizon)
    #     G --peering--  X          INVISIBLE (neither endpoint is)
    #
    # Learning G through the first edge must not confer sight of the second.
    n2, c2 = ["O", "H"], {"O": ["H"], "H": []}
    g2 = FlowGraph()
    g2.add_edge("O", "H", HIER_CAP); g2.add_edge("H", "O", HIER_CAP)
    s2 = scope_adjacency(n2, c2)
    chain = [("H", "G", PEER_CAP), ("G", "X", PEER_CAP)]
    for r in (0, 1, 2, 3):
        v2 = visible_flow_subgraph(g2, s2, "O", chain, reach=r)
        assert "X" not in v2.cap, \
            f"a peering edge with no in-horizon endpoint became visible at reach={r}"
        assert score_independent(v2, "O", "X", s2, chain) == 0, \
            f"an invisible peering edge carried standing at reach={r}"
    report.append(
        "  a peering edge one hop past a VISIBLE one stays invisible at every "
        "reach tested (0-3):")
    report.append(
        "  visibility does not compose -- learning a node confers no sight of "
        "the records around it")

    # --- DISTANCE IS COMPUTED FROM THE OBSERVER'S OWN GRAPH ----------------
    # The same property one layer down.  An edge absent from the observer's
    # capacity graph must not shorten a landscape distance either: node
    # capacity falls with distance, so a shortened distance raises a relay's
    # throughput and lifts standing without any capacity edge being added.
    g3 = FlowGraph()
    g3.add_edge("O", "H", HIER_CAP); g3.add_edge("H", "O", HIER_CAP)
    s3 = scope_adjacency(["O", "H"], {"O": ["H"], "H": []})
    seen = [("H", "A", PEER_CAP)]
    v3 = visible_flow_subgraph(g3, s3, "O", seen, reach=0)
    for u, v in (("A", "B"), ("B", "C"), ("C", "T")):
        v3.add_edge(u, v, PEER_CAP); v3.add_edge(v, u, PEER_CAP)
    ghost = seen + [("H", "C", PEER_CAP)]      # never enters the capacity graph
    honest = score_independent(v3, "O", "T", s3, seen)
    haunted = score_independent(v3, "O", "T", s3, ghost)
    assert honest == haunted, \
        "an edge absent from the observer's graph changed its computed standing"
    report.append(
        f"  an edge absent from the observer's graph moves nothing: standing "
        f"{honest} either way")

    # --- The reference allocation rule, ALL THREE PASSES (design §16.4) ----
    # Pass 0: available flow ranks first -- it is the metric itself.
    # Pass 1: shorter path dominates among equal flow.
    # Pass 2: consideration order breaks true ties only.
    # Each pass needs its own case: a plain multi-sink max-flow satisfies
    # pass 1 by accident and fails pass 2 silently, and NEITHER of those
    # cases can detect a missing pass 0, because candidates behind one
    # saturated chokepoint all carry the same flow (see admit_reference_order).
    def winner_flow_vs_distance(order):
        """B is NEARER but thinner; A is FURTHER but better supported.
        Available flow must decide, so A is admitted and B is not -- a
        distance-first ranking would serve B its unit first instead."""
        g2 = FlowGraph()
        g2.add_edge("obs", "hub", 2); g2.add_edge("hub", "obs", 2)  # scarce
        g2.add_edge("hub", "B", 1);   g2.add_edge("B", "hub", 1)    # near/thin
        g2.add_edge("hub", "m", 2);   g2.add_edge("m", "hub", 2)
        g2.add_edge("m", "A", 2);     g2.add_edge("A", "m", 2)      # far/wide
        admitted, _ = admit_reference_order(g2, "obs", order, demand=2)
        return admitted

    assert winner_flow_vs_distance(["A", "B"]) == ["A"], \
        "pass 0: available flow must outrank a shorter path"
    assert winner_flow_vs_distance(["B", "A"]) == ["A"], \
        "pass 0: available flow must outrank a shorter path"

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

    # Pass 1: the shorter path wins whichever order the candidates come in.
    assert winner_unequal(["A", "B"]) == "B", "pass 1: shorter path must win"
    assert winner_unequal(["B", "A"]) == "B", "pass 1: shorter path must win"
    # Pass 2: at equal length the earlier-considered wins -- and the graph's
    # own construction order does not leak in.
    for build in (["A", "B"], ["B", "A"]):
        assert winner_equal(["A", "B"], build) == "A", \
            "pass 2: earlier-considered must win a true tie"
        assert winner_equal(["B", "A"], build) == "B", \
            "pass 2: earlier-considered must win a true tie"
    report.append(
        "  scarce-capacity allocation: available flow ranks first (the "
        "metric itself), then the")
    report.append(
        "  shorter path, then consideration order for true ties -- "
        "independent of graph")
    report.append("  construction order (reference policy §16.4)")
    report.append(
        "  independent-sum grows with the population beyond the cut (the "
        "broken 0.8.3 reading);")
    report.append(
        "  the conserving joint saturates at the cut, and identities behind "
        "an invisible edge")
    report.append("  cannot inflate it: CONFIRMED")
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
    fA, nA, cA = build_tree(rng, depth=2, fanout=3, prefix="A")
    fB, nB, cB = build_tree(rng, depth=2, fanout=3, prefix="B")
    flow = FlowGraph()
    for g in (fA, fB):
        for u, nbrs in g.cap.items():
            for v, c in nbrs.items():
                if c > 0:
                    flow.add_edge(u, v, c)
    nodes = nA + nB
    children = {**cA, **cB}
    scope = scope_adjacency(nodes, children)

    # Enumerate EVERY cross-tree placement of one peering edge.  Endpoints
    # are not restricted to nodes with subordinates: infra status is not a
    # function of position or downline -- "a node becomes infra by launching
    # and signing an infra instance, without moving" (wire §10.1) -- so a
    # leaf may peer exactly as an interior node may.  An earlier version
    # restricted endpoints to nodes with children, which silently excluded
    # valid low-degree placements; a cross-family review caught it.
    endpointsA, endpointsB = nA, nB
    rows = []
    for pa in endpointsA:
        for pb in endpointsB:
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
                b = (score_independent(before, obs, beneficiary, scope)
                     if beneficiary in before.cap else 0)
                a = (score_independent(after, obs, beneficiary, scope,
                                       peer_edges)
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
        "  asserted was an artifact of a one-ended attacker.")
    report.append(
        "  DEMONSTRATED IN THIS TOPOLOGY (not confirmed as economics): one "
        "visible peering edge")
    report.append(
        "  influences several observers, and no observer that cannot see an "
        "edge is influenced by")
    report.append(
        "  it.  That establishes amortisation EXISTS and the conservative "
        "direction HOLDS.  It does")
    report.append(
        "  NOT measure the population-coverage economics 16.3.1 argues: how "
        "many edges are needed")
    report.append(
        "  to reach a target fraction, how successive cover sets overlap, or "
        "how any of it varies")
    report.append(
        "  across topology families.  Those need a topology model this file "
        "does not have.")
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
        "ranks in three passes:")
    report.append(
        "AVAILABLE FLOW FIRST -- it is what the metric measures -- then the "
        "SHORTER PATH among")
    report.append(
        "candidates the flow ranks equally, because a longer path is less "
        "trustworthy by nature,")
    report.append(
        "then consideration order for TRUE TIES only.  All three are "
        "REFERENCE POLICY, not a network")
    report.append(
        "invariant: per-observer trust (16.1) means no party consumes "
        "another's computation, so a")
    report.append(
        "policy allocating differently still conforms.  Note for implementers "
        "(tested above): one")
    report.append(
        "multi-sink max-flow delivers the PATH-LENGTH pass for free, misses "
        "the TIE pass silently")
    report.append(
        "(at equal path length it follows the graph's construction order, not "
        "the candidate order),")
    report.append(
        "and cannot deliver the FLOW pass at all.  Rank explicitly.")
    text = "\n".join(report)
    print(text)
    return text


if __name__ == "__main__":
    main()
