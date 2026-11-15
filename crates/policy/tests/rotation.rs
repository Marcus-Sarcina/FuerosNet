//! What a successor key is credited with, and what a shared prefix does
//! not carry (design §9.0.2, §16.7).  A rotation reaches an observer as a
//! disavowal and an adoption; the successor's standing is the vouching
//! paths the observer's own graph holds to it and nothing for the history
//! it claims, and a patron's later disavowal of one heir with prejudice
//! denies that heir alone, not a sibling adopted under the same patron.

use rhtn_policy::{Evidence, Policy, ReferenceMetric};

fn s(x: &str) -> String {
    x.to_string()
}

// functional: POL-014
#[test]
fn a_successors_credit_is_its_paths_and_a_shared_prefix_condemns_nobody() {
    let m = ReferenceMetric::default();
    let cap = m.edge_capacity;
    let mut ev = Evidence::new(s("o"));
    // o's horizon is two edges; p and everything under it lie beyond it,
    // where an edge is worth `edge_capacity` and a path is counted
    ev.adopt(s("o"), s("h1"));
    ev.adopt(s("h1"), s("h2"));
    ev.adopt(s("h2"), s("p"));
    ev.adopt(s("p"), s("a"));
    ev.adopt(s("p"), s("b"));
    assert!(!ev.horizon().contains("p"), "p is past the horizon");
    let one_path = m.flow(&ev, &s("a"));
    assert!(
        one_path > 0 && one_path <= cap,
        "worth at most one edge: {one_path}"
    );
    assert_eq!(
        m.flow(&ev, &s("b")),
        one_path,
        "the same position, the same credit"
    );

    // a rotates: what reaches o is a disavowal without prejudice, which
    // alleges nothing and so is not held as one, and an adoption of the
    // successor under the same patron (design §9.0.2)
    ev.adopt(s("p"), s("a2"));
    ev.adopt(s("p"), s("fresh"));
    assert_eq!(
        m.flow(&ev, &s("a2")),
        m.flow(&ev, &s("fresh")),
        "a successor is credited exactly as a stranger adopted beside it: its path, not its history"
    );
    assert_eq!(m.flow(&ev, &s("a2")), one_path);

    // a second vouching path is worth that path and no more: strictly
    // more than one, at most the two edges' worth
    ev.adopt(s("h2"), s("x"));
    ev.meet(s("x"), s("a2"));
    let two_paths = m.flow(&ev, &s("a2"));
    assert!(
        two_paths > one_path && two_paths <= 2 * one_path,
        "two independent paths: {two_paths} against {one_path}"
    );

    // a later offence: p ends a2 with prejudice.  a2 is denied; b, an
    // independent heir sharing the prefix o/h1/h2/p, is untouched, and so
    // is a, the key a2 succeeded
    let b_before = m.score(&ev, &s("b"));
    ev.disavow(s("p"), s("a2"));
    assert!(ev.blacklisted(&s("a2")));
    assert_eq!(m.score(&ev, &s("a2")), 0.0, "denied before allocation");
    assert!(!ev.blacklisted(&s("b")));
    assert_eq!(
        m.score(&ev, &s("b")),
        b_before,
        "a sibling under the same prefix"
    );
    assert!(!ev.blacklisted(&s("a")));
    assert_eq!(
        m.flow(&ev, &s("a")),
        one_path,
        "the prior key is a separate node"
    );
}
