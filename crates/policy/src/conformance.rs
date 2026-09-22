//! The conformance test (design §16.4): run a policy over the simulation's
//! topologies and report its resistance bound, so anyone tuning their own
//! can see what they have given up.  Nothing here is a protocol rule;
//! conformance carries no policy.
//!
//! The constructions are `models/simulation/flow_metric.py`'s: an honest
//! adoption tree; a fake region entered by one adoption at the horizon's
//! edge (E2, the individual cut bound); a region behind one acquired
//! peering edge beyond the horizon (E3, setwise conservation); and two
//! regions at the same fanout and decay, one with the acquaintance degree
//! bounded and one with it growing with depth (design §16.2's criterion,
//! necessary and not sufficient).

use crate::evidence::World;
use crate::policy::{Policy, ReferenceMetric};
use crate::series::{self, Convergence};
use std::fmt::Write;

/// design §3.1: every node has at most f = 10 subordinates.
pub const FANOUT: u64 = 10;

/// A small deterministic generator, so a topology is reproducible from its
/// seed without a seed file.
pub struct Rng(u64);

impl Rng {
    pub fn new(seed: u64) -> Self {
        Rng(seed.max(1))
    }

    fn next(&mut self) -> u64 {
        // xorshift64*
        let mut x = self.0;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.0 = x;
        x.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }

    /// Uniform in `lo..=hi`.
    pub fn between(&mut self, lo: u64, hi: u64) -> u64 {
        lo + self.next() % (hi - lo + 1)
    }
}

/// A random adoption tree: `depth` generations, one to `fanout` subordinates
/// per node.  Names encode the path from the root.
pub fn build_tree(
    rng: &mut Rng,
    depth: usize,
    fanout: u64,
    prefix: &str,
) -> (World<String>, Vec<String>) {
    let mut w = World::new();
    let mut nodes = vec![prefix.to_string()];
    let mut frontier = vec![prefix.to_string()];
    for _ in 0..depth {
        let mut next = Vec::new();
        for u in &frontier {
            for i in 0..rng.between(1, fanout) {
                let v = format!("{u}.{i}");
                w.adopt(u.clone(), v.clone());
                nodes.push(v.clone());
                next.push(v);
            }
        }
        frontier = next;
    }
    (w, nodes)
}

/// A fake subtree — `width` subordinates per node, `depth` generations —
/// under one fake root adopted by `boundary`.  Only the boundary is real
/// (design §17.2: topology cannot provide Sybil resistance).
pub fn attach_fake_region(
    w: &mut World<String>,
    boundary: &str,
    width: usize,
    depth: usize,
) -> Vec<String> {
    let root = "FAKE".to_string();
    w.adopt(boundary.to_string(), root.clone());
    let mut fakes = vec![root.clone()];
    let mut frontier = vec![root];
    for _ in 0..depth {
        let mut next = Vec::new();
        for u in &frontier {
            for i in 0..width {
                let v = format!("{u}.f{i}");
                w.adopt(u.clone(), v.clone());
                fakes.push(v.clone());
                next.push(v);
            }
        }
        frontier = next;
    }
    fakes
}

/// A region of `width` identities behind one `gate`, as a patronage tree at
/// design §3.1's fanout: filled breadth-first and round-robin, at most
/// `FANOUT` subordinates per node.  Returns the identities and how many
/// generations lie behind the gate, which is the reach an observer's
/// evidence must cover to see them all.
pub fn attach_region_behind(
    w: &mut World<String>,
    gate: &str,
    width: usize,
) -> (Vec<String>, usize) {
    let mut fakes: Vec<String> = Vec::new();
    let mut depth = 0;
    let mut frontier = vec![gate.to_string()];
    let mut count = std::collections::BTreeMap::<String, u64>::new();
    while fakes.len() < width {
        depth += 1;
        let mut shell = Vec::new();
        while fakes.len() < width
            && frontier
                .iter()
                .any(|p| count.get(p).copied().unwrap_or(0) < FANOUT)
        {
            for parent in &frontier {
                if fakes.len() == width {
                    break;
                }
                if count.get(parent).copied().unwrap_or(0) < FANOUT {
                    let v = format!("REGION.f{}", fakes.len());
                    w.adopt(parent.clone(), v.clone());
                    *count.entry(parent.clone()).or_default() += 1;
                    fakes.push(v.clone());
                    shell.push(v);
                }
            }
        }
        frontier = shell;
    }
    assert!(
        count.values().all(|c| *c <= FANOUT),
        "a patronage node exceeded design §3.1's f subordinates"
    );
    (fakes, depth)
}

/// One population size of a region, under the policy.
#[derive(Clone, Debug, PartialEq)]
pub struct Row {
    pub identities: usize,
    /// The best individual standing any identity in the region achieved.
    pub best: f64,
    /// The sum of the individual standings: the reading under which the
    /// same capacity counts once per identity.
    pub independent_sum: f64,
    /// What the region can use at once, in one computation.
    pub joint: f64,
    /// The region's cut, as the reference metric computes it on the same
    /// evidence.
    pub cut: u64,
}

/// What the rows say about the policy.
#[derive(Clone, Debug, PartialEq)]
pub enum Verdict {
    /// The joint standing stopped growing once the region exceeded its cut,
    /// and neither it nor any individual standing exceeded the cut at any
    /// size.
    Bounded { cut: u64 },
    /// The joint standing grew at every increase in population.
    GrowsWithSize,
    /// Neither saturated below the cut nor grew monotonically.
    Unbounded,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Region {
    pub title: &'static str,
    pub rows: Vec<Row>,
    pub verdict: Verdict,
}

impl Region {
    fn judge(title: &'static str, rows: Vec<Row>) -> Self {
        let n = rows.len();
        let cut = rows.last().map(|r| r.cut).unwrap_or(0);
        let within = rows
            .iter()
            .all(|r| r.best <= r.cut as f64 && r.joint <= r.cut as f64 && r.cut == cut);
        let saturated = n >= 2 && rows[n - 1].joint == rows[n - 2].joint;
        let grows = rows.windows(2).all(|w| w[1].joint > w[0].joint);
        let verdict = if within && saturated {
            Verdict::Bounded { cut }
        } else if grows {
            Verdict::GrowsWithSize
        } else {
            Verdict::Unbounded
        };
        Region {
            title,
            rows,
            verdict,
        }
    }
}

/// The branching criterion, measured on two regions (design §16.2, §16.4).
#[derive(Clone, Debug, PartialEq)]
pub struct Branching {
    pub fanout: u64,
    pub lambda: Option<f64>,
    /// Whether λ < 1/f, where the policy has a λ.
    pub criterion_satisfied: Option<bool>,
    /// The region's joint standing at each depth, hierarchy edges only.
    pub hierarchy_only: Vec<f64>,
    /// The same with an acquaintance degree that grows with depth.
    pub with_acquaintance: Vec<f64>,
    pub hierarchy_verdict: Convergence,
    pub acquaintance_verdict: Convergence,
}

/// Classify a sequence of cumulative standings by depth: the ratio of one
/// level's increment to the last, as `series::classify` does for the
/// arithmetic.
pub fn classify_levels(cumulative: &[f64]) -> Convergence {
    let increments: Vec<f64> = std::iter::once(cumulative.first().copied().unwrap_or(0.0))
        .chain(cumulative.windows(2).map(|w| w[1] - w[0]))
        .collect();
    let ratios: Vec<f64> = increments
        .windows(2)
        .map(|w| if w[0] > 0.0 { w[1] / w[0] } else { 0.0 })
        .collect();
    match (ratios.first(), ratios.last()) {
        (Some(first), Some(last)) if *last >= 1.0 || *last > *first => Convergence::Diverges,
        _ => Convergence::Converges,
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Report {
    pub policy: &'static str,
    /// E2: a fake region entered by one adoption at the horizon's edge.
    pub cut_bound: Region,
    /// E3: a region behind one acquired peering edge beyond the horizon.
    pub conservation: Region,
    pub branching: Branching,
}

impl Report {
    /// The headline: what the policy bounds a single-entry region to.
    pub fn bound(&self) -> &Verdict {
        &self.conservation.verdict
    }

    pub fn render(&self) -> String {
        let mut s = String::new();
        let _ = writeln!(s, "conformance report: {}", self.policy);
        for region in [&self.cut_bound, &self.conservation] {
            let _ = writeln!(s, "{}", region.title);
            for r in &region.rows {
                let _ = writeln!(
                    s,
                    "  {:>4} identities: best individual {:.3}  independent sum {:.3}  joint {:.3}  cut {}",
                    r.identities, r.best, r.independent_sum, r.joint, r.cut
                );
            }
            let _ = writeln!(
                s,
                "  {}",
                match &region.verdict {
                    Verdict::Bounded { cut } => format!("bounded by the cut ({cut}) at every size"),
                    Verdict::GrowsWithSize =>
                        "the region's standing grows with its population".to_string(),
                    Verdict::Unbounded => "not bounded by the cut".to_string(),
                }
            );
        }
        let b = &self.branching;
        let _ = writeln!(s, "the branching criterion (f = {})", b.fanout);
        match (b.lambda, b.criterion_satisfied) {
            (Some(l), Some(ok)) => {
                let _ = writeln!(
                    s,
                    "  lambda = {l}: lambda < 1/f is {}",
                    if ok { "SATISFIED" } else { "NOT satisfied" }
                );
            }
            _ => {
                let _ = writeln!(s, "  no decay parameter: the policy is independent of f");
            }
        }
        let show = |v: &[f64]| {
            v.iter()
                .map(|x| format!("{x:.3}"))
                .collect::<Vec<_>>()
                .join(", ")
        };
        let _ = writeln!(
            s,
            "  hierarchy only, by depth:            {}  {:?}",
            show(&b.hierarchy_only),
            b.hierarchy_verdict
        );
        let _ = writeln!(
            s,
            "  acquaintance degree growing, by depth: {}  {:?}",
            show(&b.with_acquaintance),
            b.acquaintance_verdict
        );
        let _ = writeln!(
            s,
            "  the criterion bounds the hierarchy's fanout and not the acquaintance degree: necessary, not sufficient"
        );
        s
    }
}

/// The population sizes the simulation uses.
pub const CUT_BOUND_SHAPES: [(usize, usize); 3] = [(2, 2), (3, 3), (4, 4)];
pub const CONSERVATION_SIZES: [usize; 4] = [4, 8, 16, 32];
/// The depths the branching regions are grown to.
pub const BRANCHING_DEPTH: usize = 3;
/// The decay the branching section runs where the policy has none of its
/// own: design §16.2's example, satisfying λ < 1/f at f = 10.
pub const BRANCHING_LAMBDA: f64 = 0.095;

/// Grow a region behind `parent` to `levels` generations: `fanout`
/// subordinates per node and, when `acquaintance` is set, as many fresh
/// acquaintances at depth `d` as `d`, every one of whom continues.
fn grow(
    w: &mut World<String>,
    parent: &str,
    depth: usize,
    levels: usize,
    fanout: u64,
    acquaintance: bool,
    members: &mut Vec<String>,
) {
    if depth > levels {
        return;
    }
    for k in 0..fanout {
        let child = format!("{parent}.c{k}");
        w.adopt(parent.to_string(), child.clone());
        members.push(child.clone());
        grow(w, &child, depth + 1, levels, fanout, acquaintance, members);
    }
    if acquaintance {
        for k in 0..depth.saturating_sub(1) {
            let met = format!("{parent}.m{k}");
            w.peer(parent.to_string(), met.clone());
            members.push(met.clone());
            grow(w, &met, depth + 1, levels, fanout, acquaintance, members);
        }
    }
}

/// Run the conformance test for `policy`.
pub fn run(policy: &dyn Policy<String>) -> Report {
    let reference = ReferenceMetric::default();
    let mut rng = Rng::new(1);
    let (base, nodes) = build_tree(&mut rng, 3, 3, "n");
    let observer = nodes[0].clone();
    let honest_scope = base.scope();

    // E2: the boundary sits at scope distance exactly 2, the horizon's
    // edge, so the fake root it adopts lands at 3: outside, where the cut is
    // what answers (design §16.2.1, §17.3).
    let mut ring = std::collections::BTreeSet::from([observer.clone()]);
    let mut shell = ring.clone();
    for _ in 0..2 {
        let mut next = std::collections::BTreeSet::new();
        for v in &shell {
            next.extend(honest_scope.neighbours(v).cloned());
        }
        shell = next.difference(&ring).cloned().collect();
        ring.extend(shell.iter().cloned());
    }
    let boundary = shell
        .iter()
        .min()
        .cloned()
        .expect("a node at the horizon's edge");
    let mut rows = Vec::new();
    for (width, depth) in CUT_BOUND_SHAPES {
        let mut w = base.clone();
        let fakes = attach_fake_region(&mut w, &boundary, width, depth);
        let ev = w.full_view(&observer);
        assert!(
            !ev.horizon().contains("FAKE"),
            "the fake root must sit outside the horizon or there is no bound to test"
        );
        let e = policy.evaluate(&ev, &fakes);
        rows.push(Row {
            identities: fakes.len(),
            best: e.individual.iter().map(|(_, s)| *s).fold(0.0, f64::max),
            independent_sum: e.individual.iter().map(|(_, s)| s).sum(),
            joint: e.joint,
            cut: reference.edge_capacity,
        });
    }
    let cut_bound = Region::judge(
        "a fake region entered by one adoption at the horizon's edge (E2)",
        rows,
    );

    // E3: one peering edge from a horizon member to a gate with no position
    // in the observer's subnet, and a region behind the gate.
    let inside = base.children(&observer)[0].clone();
    let gate = "GATE".to_string();
    let mut rows = Vec::new();
    for width in CONSERVATION_SIZES {
        let mut w = base.clone();
        let (fakes, depth) = attach_region_behind(&mut w, &gate, width);
        w.peer(inside.clone(), gate.clone());
        let ev = w.view(&observer, depth);
        assert!(
            fakes.iter().all(|f| ev.knows(f)),
            "the region must all be visible"
        );
        let e = policy.evaluate(&ev, &fakes);
        rows.push(Row {
            identities: fakes.len(),
            best: e.individual.iter().map(|(_, s)| *s).fold(0.0, f64::max),
            independent_sum: e.individual.iter().map(|(_, s)| s).sum(),
            joint: e.joint,
            cut: reference.ceiling(&ev, &gate),
        });
    }
    let conservation = Region::judge(
        "a region behind one acquired peering edge beyond the horizon (E3)",
        rows,
    );

    // The branching criterion: the same gate, and behind it two regions at
    // the design's fanout.
    let lambda = policy.decay();
    let mut by_depth = [Vec::new(), Vec::new()];
    for (i, acquaintance) in [false, true].into_iter().enumerate() {
        for levels in 1..=BRANCHING_DEPTH {
            let mut w = base.clone();
            w.peer(inside.clone(), gate.clone());
            let mut members = Vec::new();
            grow(&mut w, &gate, 1, levels, FANOUT, acquaintance, &mut members);
            let ev = w.full_view(&observer);
            by_depth[i].push(policy.evaluate(&ev, &members).joint);
        }
    }
    let [hierarchy_only, with_acquaintance] = by_depth;
    let branching = Branching {
        fanout: FANOUT,
        lambda,
        criterion_satisfied: lambda.map(|l| series::criterion_satisfied(l, FANOUT)),
        hierarchy_verdict: classify_levels(&hierarchy_only),
        acquaintance_verdict: classify_levels(&with_acquaintance),
        hierarchy_only,
        with_acquaintance,
    };

    Report {
        policy: policy.name(),
        cut_bound,
        conservation,
        branching,
    }
}
