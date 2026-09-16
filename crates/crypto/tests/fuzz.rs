//! A long-running seeded mutation run over the corpus, for the hours-long
//! robustness exercise milestone 1's exit criterion asks for.  Skipped unless
//! `RHTN_FUZZ_SECONDS` is set; the short form of the same exercise is
//! DEC-02 in `decoder.rs`, which the gate always runs.
//!
//!     RHTN_FUZZ_SECONDS=3600 cargo test -p rhtn-crypto --test fuzz -- --nocapture
//!
//! Every input must get a verdict without a panic, within one second, and
//! every input reported valid must round-trip through the re-encoder.  The
//! seed is printed so a failing run can be reproduced.

mod common;
use common::*;
use rhtn_codec::cbor::*;
use std::time::{Duration, Instant};

struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        x
    }
    fn below(&mut self, n: usize) -> usize {
        (self.next() % (n as u64).max(1)) as usize
    }
}

#[test]
fn mutate_until_the_budget_runs_out() {
    let Some(secs) = std::env::var("RHTN_FUZZ_SECONDS").ok().and_then(|s| s.parse::<u64>().ok()) else {
        eprintln!("fuzz: RHTN_FUZZ_SECONDS unset, skipping");
        return;
    };
    let seed = std::env::var("RHTN_FUZZ_SEED").ok().and_then(|s| s.parse::<u64>().ok()).unwrap_or_else(|| {
        std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos() as u64 | 1
    });
    eprintln!("fuzz: seed {seed:#x}, budget {secs}s");
    let ids = identities();
    let set: Vec<Fixture> = fixtures().into_iter().filter(|f| f.outcome == "accept").collect();
    let mut rng = Rng(seed);
    let deadline = Instant::now() + Duration::from_secs(secs);
    let (mut n, mut valid) = (0u64, 0u64);
    while Instant::now() < deadline {
        let f = &set[rng.below(set.len())];
        let mut v = f.bytes.clone();
        // several stacked edits per input, so shapes drift further than DEC-02's single edit
        for _ in 0..1 + rng.below(4) {
            match rng.below(5) {
                0 => { let p = rng.below(v.len()); v[p] = rng.next() as u8; }
                1 => { let p = rng.below(v.len() + 1); v.insert(p, rng.next() as u8); }
                2 => { if v.len() > 1 { let p = rng.below(v.len()); v.remove(p); } }
                3 => { let p = rng.below(v.len()); v[p] ^= 1 << rng.below(8); }
                _ => {
                    let other = &set[rng.below(set.len())].bytes;
                    let len = 1 + rng.below(64.min(other.len()));
                    let src = rng.below(other.len() - len + 1);
                    let dst = rng.below(v.len() + 1);
                    let run = other[src..src + len].to_vec();
                    v.splice(dst..dst, run);
                }
            }
        }
        let t0 = Instant::now();
        let verdict = decode(&ids, &f.id, &f.kind, &v);
        assert!(t0.elapsed() < Duration::from_secs(1), "seed {seed:#x}: input {n} exceeded one second");
        if verdict.is_ok() {
            let span = canonical_span(&f.kind, &v);
            let item = parse_all(span).expect("valid means parsed");
            assert_eq!(reencode(&item, span), span, "seed {seed:#x}: input {n} valid but not round-tripping");
            valid += 1;
        }
        n += 1;
    }
    eprintln!("fuzz: {n} inputs, {valid} valid, no panic, none over budget");
}
