//! Metric entries (MET): the reference flow metric over fixed graphs and
//! the simulation's topologies, the conformance report, and the
//! regression cases carried across from `models/simulation/flow_metric.py`.

use rhtn_policy::conformance::{
    self, Rng, Row, Verdict, attach_fake_region, attach_region_behind, build_tree, run,
};
use rhtn_policy::landscape::{self, Allocation, Scope, landscape_distance, split};
use rhtn_policy::series::{self, Convergence};
use rhtn_policy::{DistanceDecay, Evidence, FlowGraph, ReferenceMetric, UNTHROTTLED, World};

fn s(x: &str) -> String {
    x.to_string()
}

/// MET-01's fixture: one scarce edge out of the observer, and five
/// candidates behind it that differ on flow, on hops, and on nothing.
///
/// ```text
/// obs -4-> hub -2-> m -2-> A          A: flow 2, hops 3
///          hub -1-> B                 B: flow 1, hops 2
///          hub -1-> x -1-> C          C: flow 1, hops 3
///          hub -1-> y -1-> z -1-> D   D: flow 1, hops 4
///                          z -1-> E   E: flow 1, hops 4
/// ```
const FIXTURE: [(&str, &str, u64); 9] = [
    ("obs", "hub", 4),
    ("hub", "m", 2),
    ("m", "A", 2),
    ("hub", "B", 1),
    ("hub", "x", 1),
    ("x", "C", 1),
    ("hub", "y", 1),
    ("y", "z", 1),
    ("z", "D", 1),
];

fn fixture(order: &[usize]) -> FlowGraph<String> {
    let mut g = FlowGraph::new();
    let mut edges: Vec<(&str, &str, u64)> = FIXTURE.to_vec();
    edges.push(("z", "E", 1));
    for &i in order {
        let (u, v, c) = edges[i];
        g.add_edge(s(u), s(v), c);
    }
    g
}

fn allocate(g: &FlowGraph<String>) -> Allocation<String> {
    let order = ["C", "B", "A", "D", "E"].map(s);
    landscape::admit(g, &s("obs"), &order, 1, None)
}

// acceptance: MET-01
#[test]
fn candidates_rank_by_flow_then_hops_then_consideration_order() {
    let g = fixture(&(0..10).collect::<Vec<_>>());
    let a = allocate(&g);
    let ranked: Vec<(&str, u64, Option<usize>)> = a
        .ranked
        .iter()
        .map(|r| (r.candidate.as_str(), r.flow, r.hops))
        .collect();
    // the fixture's expected per-candidate flows and hops
    assert_eq!(
        ranked,
        [
            ("A", 2, Some(3)),
            ("B", 1, Some(2)),
            ("C", 1, Some(3)),
            ("D", 1, Some(4)),
            ("E", 1, Some(4))
        ]
    );
    // A before B (flow beats a shorter path and an earlier consideration);
    // B before C (hops, against consideration order); D before E (true tie,
    // consideration order); the saturated cut of 4 leaves E out
    assert_eq!(a.admitted, ["A", "B", "C", "D"].map(s));
    assert_eq!(a.total, 4, "the true maximum, whatever the order");
}

// acceptance: MET-02
#[test]
fn no_adjacency_order_decides_an_equal_path_tie() {
    let forward = allocate(&fixture(&(0..10).collect::<Vec<_>>()));
    let reversed = allocate(&fixture(&(0..10).rev().collect::<Vec<_>>()));
    let shuffled = allocate(&fixture(&[9, 3, 0, 7, 1, 8, 5, 2, 6, 4]));
    assert_eq!(forward, reversed);
    assert_eq!(forward, shuffled);
    assert_eq!(forward.admitted, ["A", "B", "C", "D"].map(s));
}

/// Carried from `flow_metric.py`: each of the three passes needs its own
/// case, because a plain multi-sink max-flow satisfies pass 1 by accident,
/// fails pass 2 silently, and cannot detect a missing pass 0.
#[test]
fn the_three_passes_each_decide_their_own_case() {
    // pass 0: B is nearer but thinner, A further but better supported
    let flow_vs_distance = |order: [&str; 2]| {
        let mut g = FlowGraph::new();
        g.join(s("obs"), s("hub"), 2);
        g.join(s("hub"), s("B"), 1);
        g.join(s("hub"), s("m"), 2);
        g.join(s("m"), s("A"), 2);
        landscape::admit(&g, &s("obs"), &order.map(s), 2, None).admitted
    };
    assert_eq!(
        flow_vs_distance(["A", "B"]),
        [s("A")],
        "pass 0: available flow outranks a shorter path"
    );
    assert_eq!(flow_vs_distance(["B", "A"]), [s("A")]);

    // pass 1: A sits one hop further than B behind one scarce unit
    let unequal = |order: [&str; 2]| {
        let mut g = FlowGraph::new();
        g.add_edge(s("obs"), s("bot"), 1);
        g.add_edge(s("bot"), s("x"), 1);
        g.add_edge(s("x"), s("A"), 1);
        g.add_edge(s("bot"), s("B"), 1);
        landscape::admit(&g, &s("obs"), &order.map(s), 1, None).admitted
    };
    assert_eq!(
        unequal(["A", "B"]),
        [s("B")],
        "pass 1: the shorter path wins"
    );
    assert_eq!(unequal(["B", "A"]), [s("B")]);

    // pass 2: equidistant, and the graph's construction order does not leak
    let equal = |order: [&str; 2], build: [&str; 2]| {
        let mut g = FlowGraph::new();
        g.add_edge(s("obs"), s("bot"), 1);
        for t in build {
            g.add_edge(s("bot"), s(t), 1);
        }
        landscape::admit(&g, &s("obs"), &order.map(s), 1, None).admitted
    };
    for build in [["A", "B"], ["B", "A"]] {
        assert_eq!(
            equal(["A", "B"], build),
            [s("A")],
            "pass 2: the earlier-considered wins a true tie"
        );
        assert_eq!(equal(["B", "A"], build), [s("B")]);
    }
}

#[test]
fn max_flow_is_the_min_cut() {
    let mut g = FlowGraph::new();
    g.add_edge(1, 2, 3);
    g.add_edge(1, 3, 2);
    g.add_edge(2, 4, 2);
    g.add_edge(3, 4, 3);
    g.add_edge(2, 3, 1);
    assert_eq!(g.max_flow(&1, &4), 5);
    assert_eq!(g.max_flow(&4, &1), 0);
    assert_eq!(g.max_flow(&1, &9), 0, "an absent sink carries nothing");
}

/// The E3 construction: the honest tree, one peering from a horizon member
/// to a gate, and a region of `width` behind the gate, seen by an observer
/// whose evidence reaches the region's depth.
fn conserving(width: usize) -> (Evidence<String>, Vec<String>, String) {
    let mut rng = Rng::new(1);
    let (mut w, nodes) = build_tree(&mut rng, 3, 3, "n");
    let observer = nodes[0].clone();
    let inside = w.children(&observer)[0].clone();
    let (fakes, depth) = attach_region_behind(&mut w, "GATE", width);
    w.peer(inside, s("GATE"));
    (w.view(&observer, depth), fakes, s("GATE"))
}

// acceptance: MET-06
#[test]
fn parallel_edges_between_one_pair_collapse_to_one_capacity() {
    // h2 sits at the horizon's edge; p, its subordinate, lies just outside
    let m = ReferenceMetric::default();
    let mut ev = Evidence::new(s("o"));
    ev.adopt(s("o"), s("h1"));
    ev.adopt(s("h1"), s("h2"));
    ev.adopt(s("h2"), s("p")); // the adoption
    ev.meet(s("h2"), s("p")); // the meeting §6.1.1 requires with it
    ev.meet(s("h2"), s("p")); // and a second meeting, later
    assert!(!ev.horizon().contains("p"));
    assert_eq!(
        ev.pairs()
            .iter()
            .filter(|(a, b)| (a, b) == (&s("h2"), &s("p")))
            .count(),
        1,
        "one pair, one edge"
    );
    assert_eq!(m.graph(&ev).capacity(&s("h2"), &s("p")), m.edge_capacity);
    assert_eq!(
        m.flow(&ev, &s("p")),
        m.edge_capacity,
        "the computed flow is the single-edge value"
    );
    // the same graph with the three relationships summed carries more
    let mut summed = m.graph(&ev);
    let mut g = FlowGraph::new();
    for u in summed.nodes() {
        for (v, c) in summed.edges_from(u) {
            let c = if (u.as_str(), v.as_str()) == ("h2", "p")
                || (u.as_str(), v.as_str()) == ("p", "h2")
            {
                3 * c
            } else {
                c
            };
            g.add_edge(u.clone(), v.clone(), c);
        }
    }
    summed = g;
    assert_eq!(
        landscape::score(&summed, &s("o"), &s("p"), Some(&ev.scope())),
        3 * m.edge_capacity,
        "summing would give a larger flow"
    );
}

// acceptance: MET-07
#[test]
fn the_aggregate_a_set_can_draw_is_bounded_by_the_cut_per_computation() {
    let m = ReferenceMetric::default();
    let mut rows = Vec::new();
    for width in conformance::CONSERVATION_SIZES {
        let (ev, fakes, gate) = conserving(width);
        assert!(
            fakes.iter().all(|f| ev.knows(f)),
            "the region must all be visible"
        );
        let independent: u64 = fakes.iter().map(|f| m.flow(&ev, f)).sum();
        let joint = m.admit(&ev, &fakes, 1).total;
        let cut = m.ceiling(&ev, &gate);
        assert!(joint <= cut, "aggregate exceeded the cut");
        rows.push((width, independent, joint, cut));
    }
    assert_eq!(
        rows,
        [
            (4, 32, 4, 8),
            (8, 64, 8, 8),
            (16, 104, 8, 8),
            (32, 168, 8, 8)
        ]
    );
    assert_eq!(
        rows[2].2, rows[3].2,
        "the joint stops growing once the region exceeds its cut"
    );
    // the general statement: every candidate demands its own standing
    let (ev, fakes, gate) = conserving(32);
    let demands: Vec<(String, u64)> = fakes.iter().map(|f| (f.clone(), m.flow(&ev, f))).collect();
    let asked: u64 = demands.iter().map(|(_, d)| d).sum();
    let delivered = m.deliverable(&ev, &demands);
    assert!(
        asked > m.ceiling(&ev, &gate),
        "vacuous unless demand exceeds the cut"
    );
    assert_eq!((asked, delivered), (168, 8));
    // and the figures are the simulation's committed ones
    let committed = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../models/results/flow_metric.txt"
    ))
    .expect("the committed simulation results");
    for (width, independent, joint, cut) in rows {
        let line = committed
            .lines()
            .find(|l| l.contains(&format!("{width:>2} identities behind ONE acquired edge")))
            .unwrap_or_else(|| panic!("no committed row for {width}"));
        assert!(
            line.contains(&format!("independent-sum = {independent:>4}")),
            "{line}"
        );
        assert!(
            line.contains(&format!("conserving joint = {joint}")),
            "{line}"
        );
        assert!(line.contains(&format!("cut at the gate = {cut}")), "{line}");
    }
}

/// Carried from `flow_metric.py`: two disjoint halves of one region admitted
/// separately draw more than the cut; admitted together they draw the cut.
/// The bound rests on the whole set entering one computation, and nothing
/// here is a retained entitlement (design §16.2).
#[test]
fn setwise_conservation_holds_per_computation_not_per_lifetime() {
    let m = ReferenceMetric::default();
    let (ev, fakes, gate) = conserving(16);
    let (a, b) = fakes.split_at(fakes.len() / 2);
    let solo_a = m.admit(&ev, a, 1).total;
    let solo_b = m.admit(&ev, b, 1).total;
    let joint = m.admit(&ev, &fakes, 1).total;
    let cut = m.ceiling(&ev, &gate);
    assert_eq!((solo_a, solo_b, joint, cut), (8, 8, 8, 8));
    assert!(joint <= cut);
    assert!(
        solo_a + solo_b > joint,
        "separate admission draws more than the cut allows"
    );
}

/// Carried from `flow_metric.py`: a peering edge one hop past a visible one
/// stays invisible at every reach.  Learning of a node confers no sight of
/// the records around it (design §16.2.1, §16.3.1).
#[test]
fn visibility_does_not_compose() {
    let m = ReferenceMetric::default();
    let mut w = World::new();
    w.adopt(s("O"), s("H"));
    w.peer(s("H"), s("G")); // visible: an endpoint is in O's horizon
    w.peer(s("G"), s("X")); // invisible: neither is
    for reach in 0..=3 {
        let ev = w.view(&s("O"), reach);
        assert!(
            ev.knows(&s("G")),
            "the visible edge brings its far endpoint in"
        );
        assert!(
            !ev.knows(&s("X")),
            "a peering edge with no in-horizon endpoint became visible at reach {reach}"
        );
        assert_eq!(
            m.flow(&ev, &s("X")),
            0,
            "an invisible edge carried standing at reach {reach}"
        );
    }
}

/// Carried from `flow_metric.py`: distance is computed from the same graph
/// the flow is.  Here that is structural — an evaluator's distance and its
/// capacity graph come from one evidence — and what is checked is design
/// §16.2.1's consequence: hops in the collapsed graph are the landscape
/// distance, so ranking by hops ranks by distance and never re-grades the
/// inside of the horizon.
#[test]
fn hops_in_the_collapsed_graph_are_the_landscape_distance() {
    let m = ReferenceMetric::default();
    let (ev, fakes, _) = conserving(16);
    let g = m.graph(&ev);
    let scope = ev.scope();
    let dist = landscape_distance(&scope, &g, &ev.observer);
    let sp = split(&g, &ev.observer, Some(&scope));
    let hops = sp.network.hops_from(sp.source);
    let mut checked = 0;
    for (n, d) in &dist {
        if *d == 0 {
            continue;
        }
        let h = hops[sp.inn(n).unwrap()].unwrap_or_else(|| panic!("{n} placed but unreachable"));
        assert_eq!(h, 2 * d - 1, "{n}: landscape distance {d}, split hops {h}");
        checked += 1;
    }
    assert!(
        checked > fakes.len(),
        "the whole graph was checked, region included"
    );
    // every horizon member is one step out, near or far within it
    for h in ev.horizon() {
        if h != ev.observer {
            assert_eq!(dist[&h], 1);
        }
    }
    // an edge the evidence lacks shortens nothing: the region's identities
    // sit behind the gate at 2, its subordinates at 3, theirs at 4
    assert_eq!(dist["GATE"], 2);
    assert_eq!(dist["REGION.f0"], 3);
    assert_eq!(dist["REGION.f10"], 4);
}

/// The E2 construction's edge: entered from inside the horizon the same
/// region is unthrottled — the bound is a beyond-the-horizon claim (design
/// §16.2.1, §17.3).  And a region gated behind a node outside the horizon is
/// not seen at all: invisibility is the conservative direction (design
/// §16.3.1).
#[test]
fn the_bound_has_the_horizon_boundary_as_its_shape() {
    let m = ReferenceMetric::default();
    let mut rng = Rng::new(1);
    let (base, nodes) = build_tree(&mut rng, 3, 3, "n");
    let observer = nodes[0].clone();
    let inside = base.children(&observer)[0].clone();
    let mut w = base.clone();
    let fakes = attach_fake_region(&mut w, &inside, 2, 2);
    let ev = w.full_view(&observer);
    assert!(
        ev.horizon().contains("FAKE"),
        "a direct child's adoptee is inside the horizon"
    );
    let best = fakes.iter().map(|f| m.flow(&ev, f)).max().unwrap();
    assert_eq!(
        best, UNTHROTTLED,
        "inside the horizon the metric does not ration; if this bounds, the horizon is throttled somewhere it should not be"
    );

    let hz = base.horizon(&observer);
    let outside = nodes.iter().find(|n| !hz.contains(*n)).unwrap().clone();
    let mut w = base.clone();
    let (fakes, depth) = attach_region_behind(&mut w, "GATE", 32);
    w.peer(outside, s("GATE"));
    let ev = w.view(&observer, depth);
    assert!(
        fakes.iter().all(|f| !ev.knows(f)),
        "identities beyond an invisible edge leaked in"
    );
    assert!(!ev.knows(&s("GATE")));
}

// acceptance: MET-05
#[test]
fn the_conformance_test_reports_a_policy_s_resistance_bound() {
    let reference = run(&ReferenceMetric::default());
    for region in [&reference.cut_bound, &reference.conservation] {
        let cut = region.rows[0].cut;
        assert_eq!(region.verdict, Verdict::Bounded { cut }, "{}", region.title);
        for Row {
            identities,
            best,
            joint,
            ..
        } in &region.rows
        {
            assert!(
                *best <= cut as f64 && *joint <= cut as f64,
                "{identities} identities: best {best}, joint {joint}, cut {cut}"
            );
        }
    }
    let sizes: Vec<usize> = reference
        .cut_bound
        .rows
        .iter()
        .map(|r| r.identities)
        .collect();
    assert_eq!(sizes, [7, 40, 341], "the simulation's population sizes");
    assert_eq!(
        reference
            .conservation
            .rows
            .iter()
            .map(|r| r.identities)
            .collect::<Vec<_>>(),
        [4, 8, 16, 32]
    );
    assert_eq!(reference.conservation.verdict, Verdict::Bounded { cut: 8 });
    assert_eq!(reference.cut_bound.verdict, Verdict::Bounded { cut: 10 });

    let decay = run(&DistanceDecay::new(0.5));
    for region in [&decay.cut_bound, &decay.conservation] {
        assert_eq!(region.verdict, Verdict::GrowsWithSize, "{}", region.title);
        assert!(region.rows.windows(2).all(|w| w[1].joint > w[0].joint));
    }
    let text = reference.render() + &decay.render();
    assert!(text.contains("bounded by the cut (8) at every size"));
    assert!(text.contains("the region's standing grows with its population"));
}

// acceptance: MET-08
#[test]
fn the_branching_condition_is_necessary_and_not_sufficient() {
    // design §16.2's arithmetic, carried from the simulation: f = 10 and
    // λ = 0.095 satisfy λ < 1/f; one acquaintance edge per node beyond the
    // fanout puts the same λ in the divergent regime
    let (f, lam) = (10u64, 0.095);
    assert!(series::criterion_satisfied(lam, f));
    let honest = series::honest_one(lam, 1);
    assert!(
        series::fake_mass(lam, 1, |_| f as f64, 200) / honest < 21.0,
        "at branching f the series converges to 1/(1 - fλ) = 20"
    );
    assert!(
        series::fake_mass(lam, 1, |_| (f + 1) as f64, 200) / honest > 1000.0,
        "at branching f + 1 the same λ diverges"
    );
    assert_eq!(
        series::classify(lam, |_| f as f64, 200),
        Convergence::Converges
    );
    assert_eq!(
        series::classify(lam, |_| (f + 1) as f64, 200),
        Convergence::Diverges
    );
    assert_eq!(
        series::classify(lam, |i| (f + i as u64) as f64, 200),
        Convergence::Diverges,
        "an acquaintance degree growing with depth"
    );
    // the boundary fλ = 1 diverges, linearly
    assert!(series::fake_mass(0.1, 1, |_| 10.0, 100) / series::honest_one(0.1, 1) > 100.0);
    assert!(series::fake_mass(0.1, 1, |_| 10.0, 200) / series::honest_one(0.1, 1) > 200.0);
    assert_eq!(series::classify(0.1, |_| 10.0, 40), Convergence::Diverges);
    // and design §16.2's quoted magnitudes
    let ratio = series::fake_mass(0.5, 1, |_| 10.0, 6) / series::honest_one(0.5, 1);
    assert!((15_000.0..25_000.0).contains(&ratio), "{ratio}");
    assert!(series::fake_mass(0.05, 1, |_| 10.0, 40) < 0.15);

    // the same on two graphs the decay policy is scored over
    let report = run(&DistanceDecay::new(lam));
    let b = &report.branching;
    assert_eq!(
        (b.fanout, b.lambda, b.criterion_satisfied),
        (f, Some(lam), Some(true))
    );
    assert_eq!(
        b.hierarchy_verdict,
        Convergence::Converges,
        "bounded acquaintance degree: {:?}",
        b.hierarchy_only
    );
    assert_eq!(
        b.acquaintance_verdict,
        Convergence::Diverges,
        "acquaintance degree growing with depth: {:?}",
        b.with_acquaintance
    );
    // an implementation reporting the satisfied criterion as sufficient
    // would have said the second converges
    assert!(
        !(b.criterion_satisfied == Some(true) && b.acquaintance_verdict == Convergence::Converges)
    );
    let text = report.render();
    assert!(text.contains("lambda < 1/f is SATISFIED"));
    assert!(text.contains("Diverges"));
    assert!(text.contains("necessary, not sufficient"));
    // the flow metric is independent of f: bounded on both graphs
    let reference = run(&ReferenceMetric::default()).branching;
    assert_eq!(reference.lambda, None);
    assert_eq!(
        (reference.hierarchy_verdict, reference.acquaintance_verdict),
        (Convergence::Converges, Convergence::Converges)
    );
    assert!(
        reference.with_acquaintance.iter().all(|j| *j == 8.0),
        "{:?}",
        reference.with_acquaintance
    );
}

#[test]
fn a_scope_is_adoption_and_sibling_edges_only() {
    let scope = Scope::from_adoptions(&[(s("p"), s("a")), (s("p"), s("b")), (s("a"), s("c"))]);
    assert_eq!(
        scope.horizon(&s("c"), 2),
        ["c", "a", "p", "b"].map(s).into_iter().collect()
    );
    assert_eq!(
        scope.horizon(&s("b"), 1),
        ["b", "p", "a"].map(s).into_iter().collect(),
        "siblings are one scope edge apart"
    );
    let mut ev = Evidence::new(s("b"));
    ev.adopt(s("p"), s("a"));
    ev.adopt(s("p"), s("b"));
    ev.meet(s("b"), s("q"));
    assert!(!ev.horizon().contains("q"), "a meeting confers no scope");
    let d = ReferenceMetric::default().distance(&ev);
    assert_eq!(
        (d["b"], d["p"], d["a"], d["q"]),
        (0, 1, 1, 1),
        "a counterparty met is at 1, in the horizon or not"
    );
}
