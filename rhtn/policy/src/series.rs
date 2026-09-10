//! The arithmetic of design §16.2: under weight λ^distance the mass of an
//! attacker's fake region at depth D with k further levels is
//! λ^D · Σ (fλ)^i, which diverges unless fλ < 1 — and f bounds the
//! hierarchy's fanout, not the acquaintance degree of the graph a decay
//! policy actually evaluates, so the criterion is necessary and not
//! sufficient (design §16.4).

/// The mass of a region under a decay policy: level `i` of `0..=levels`
/// holds `branching(i)` times the identities of level `i-1`, every one of
/// them at distance `depth + i`.
pub fn fake_mass(lambda: f64, depth: usize, branching: impl Fn(usize) -> f64, levels: usize) -> f64 {
    let mut count = 1.0;
    let mut mass = 0.0;
    for i in 0..=levels {
        if i > 0 {
            count *= branching(i);
        }
        mass += lambda.powi((depth + i) as i32) * count;
    }
    mass
}

/// One honest node at the same depth.
pub fn honest_one(lambda: f64, depth: usize) -> f64 {
    lambda.powi(depth as i32)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Convergence {
    Converges,
    Diverges,
}

/// Whether a region's mass is bounded in its depth, from the ratio of one
/// level's mass to the last: `branching(i) · λ`.  The series converges
/// only if that ratio settles below one.  Equality diverges — linearly,
/// but without bound — so design §16.2's "unless fλ < 1" is strict.  A
/// ratio that grows with depth, which is what an acquaintance degree that
/// grows with depth produces, diverges whatever f is.
pub fn classify(lambda: f64, branching: impl Fn(usize) -> f64, levels: usize) -> Convergence {
    let ratio = |i: usize| branching(i) * lambda;
    let (first, last) = (ratio(1), ratio(levels.max(1)));
    if last >= 1.0 || last > first { Convergence::Diverges } else { Convergence::Converges }
}

/// Whether a per-hop decay satisfies the published criterion λ < 1/f.
/// Necessary and not sufficient: it says nothing about the acquaintance
/// edges design §16.2.1 puts in the same graph.
pub fn criterion_satisfied(lambda: f64, fanout: u64) -> bool {
    lambda < 1.0 / fanout as f64
}
