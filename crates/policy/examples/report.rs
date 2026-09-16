//! Print the conformance report for the reference metric and a decay policy.
use rhtn_policy::conformance::run;
use rhtn_policy::{DistanceDecay, ReferenceMetric};

fn main() {
    let t = std::time::Instant::now();
    print!("{}", run(&ReferenceMetric::default()).render());
    eprintln!("reference: {:?}", t.elapsed());
    let t = std::time::Instant::now();
    print!("{}", run(&DistanceDecay::new(0.5)).render());
    print!("{}", run(&DistanceDecay::new(0.095)).render());
    eprintln!("decay: {:?}", t.elapsed());
}
