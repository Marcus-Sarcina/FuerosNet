//! A verifier's conduct as one observer tallies it (design §7.4.3): what
//! it answered against what it left `unavailable`, over the records the
//! observer holds, with an instance's silence kept apart from a light
//! client's and the holding window doing the whole of the decay.

use rhtn_policy::{Conduct, Evidence};

fn s(x: &str) -> String {
    x.to_string()
}

#[test]
fn the_tally_keeps_answered_apart_from_unavailable_and_infra_from_light() {
    let mut ev = Evidence::new(s("obs"));
    // v answered twice and was unavailable once, running no instance
    ev.observed(s("v"), 100, false, false);
    ev.observed(s("v"), 200, false, false);
    ev.observed(s("v"), 300, true, false);
    // w, an instance, was unavailable twice
    ev.observed(s("w"), 150, true, true);
    ev.observed(s("w"), 250, true, true);
    assert_eq!(
        ev.conduct(&s("v"), 0),
        Conduct {
            answered: 2,
            unavailable_infra: 0,
            unavailable_light: 1,
        }
    );
    let w = ev.conduct(&s("w"), 0);
    assert_eq!(
        (w.answered, w.unavailable_infra, w.unavailable_light),
        (0, 2, 0)
    );
    assert_eq!(w.unavailable(), 2);
    assert_eq!(w.total(), 2);
}

#[test]
fn the_holding_window_is_the_decay_and_nothing_else_is() {
    let mut ev = Evidence::new(s("obs"));
    ev.observed(s("v"), 100, true, false);
    ev.observed(s("v"), 200, false, false);
    ev.observed(s("v"), 300, true, false);
    // everything held: three observations
    assert_eq!(ev.conduct(&s("v"), 0).total(), 3);
    // the record at 100 has fallen out of what the observer holds, and its
    // observation falls with it — no formula reweighted the rest
    let c = ev.conduct(&s("v"), 150);
    assert_eq!((c.answered, c.unavailable()), (1, 1));
    // an identity never observed has no conduct, which is not a bad one
    assert_eq!(ev.conduct(&s("nobody"), 0), Conduct::default());
}
