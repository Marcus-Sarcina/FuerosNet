//! The policy interface at the node (MET-04): a substitute policy changes
//! computed standing and nothing on the wire.

mod common;

use common::*;
use rhtn_archive::record::Record;
use rhtn_archive::tx;
use rhtn_node::Keyhash;
use rhtn_node::peering::peering_body;
use rhtn_node::resolution::NetworkPoint;
use rhtn_node::store::{Decision, KIND_TRANSACTION};
use rhtn_node::view::NodeView;
use rhtn_policy::{Evaluation, Policy, ReferenceMetric, Uniform};
use std::sync::Arc;

/// The observed transactions: bob's and carol's adoptions under alice and
/// w2's under carol (carol's own table), then w1 adopted by bob, a peering
/// bob-w5, and w6 adopted by w1, which lies beyond carol's horizon.
fn observed() -> (World, Vec<Record>, Vec<Record>) {
    let mut w = World::new();
    let (a_bob, _) = w.adopt("bob", "alice", 1);
    let (a_carol, _) = w.adopt("carol", "alice", 2);
    let (a_w2, _) = w.adopt("w2", "carol", 3);
    let (a_w1, _) = w.adopt("w1", "bob", 4);
    let (a_w6, _) = w.adopt("w6", "w1", 5);
    let (ib, i5) = (id("bob"), id("w5"));
    let pop = rhtn_codec::cose::sha256(b"pop:bob:w5");
    let body = peering_body(
        [
            &w.archives[&kh("bob")].next_back_pointers(),
            &[rhtn_archive::genesis(&kh("w5"))],
        ],
        &kh("bob"),
        &kh("w5"),
        &NetworkPoint::new([203, 0, 113, 7], None).with_asn(64_496),
        &NetworkPoint::new([198, 51, 100, 9], None).with_asn(64_497),
        w.clock,
        Some(1 << 20),
        &pop,
    );
    let peering =
        Record::parse(&tx::envelope(tx::TYPE_PEERING, &body, &[&ib, &i5])).expect("well-formed");
    (w, vec![a_bob, a_carol, a_w2], vec![a_w1, peering, a_w6])
}

struct Run {
    decisions: Vec<Decision>,
    frames: Vec<SentFrame>,
    objects: Vec<(u64, Vec<u8>)>,
    standing: Vec<f64>,
    joint: Evaluation<Keyhash>,
}

fn run(policy: Arc<dyn Policy<Keyhash>>) -> Run {
    let (w, own, pushed) = observed();
    let table = table_with(
        kh("carol"),
        &w,
        &own.iter().collect::<Vec<_>>(),
        &["alice", "bob", "carol", "w1", "w5"],
    );
    let mut v: NodeView = view("carol", table, "alice", &[1]);
    v.set_now(w.clock);
    v.policy = policy;
    // sessions to the patron and to a subordinate: what arrives from one is
    // forwarded to the other
    let fab = Fabric::with(&[kh("alice"), kh("w2")]);
    let lookup = ids();
    let decisions = pushed
        .iter()
        .map(|r| v.take_object(&*fab, &kh("alice"), KIND_TRANSACTION, &r.bytes, &lookup))
        .collect();
    let targets = [kh("bob"), kh("w1"), kh("w5"), kh("w6")];
    Run {
        decisions,
        frames: fab.frames(),
        objects: v.store.objects(),
        standing: targets.iter().map(|t| v.standing(t)).collect(),
        joint: v.evaluate(&targets),
    }
}

// acceptance: MET-04
#[test]
fn a_substitute_policy_changes_standing_and_nothing_on_the_wire() {
    let reference = run(Arc::new(ReferenceMetric::default()));
    let uniform = run(Arc::new(Uniform));
    assert_eq!(
        reference.decisions, uniform.decisions,
        "the same objects are stored"
    );
    assert_eq!(reference.decisions[0], Decision::Stored);
    assert_eq!(reference.decisions[1], Decision::Stored);
    assert_eq!(reference.objects, uniform.objects);
    assert_eq!(reference.frames, uniform.frames, "the same wire traffic");
    assert!(!reference.frames.is_empty(), "the pushes were forwarded");
    // only the computed standings differ: a sibling, unthrottled inside the
    // horizon; a sibling's subordinate, likewise; a peer of a sibling, one
    // edge beyond the horizon at the edge's capacity; a node beyond the
    // horizon carol holds nothing for
    assert_eq!(reference.standing, [1e9, 1e9, 10.0, 0.0]);
    assert_eq!(uniform.standing, [1.0, 1.0, 1.0, 0.0]);
    assert_ne!(reference.joint.individual, uniform.joint.individual);
    // one computation admits the three carol can place, under either
    assert_eq!(reference.joint.admitted, [kh("bob"), kh("w1"), kh("w5")]);
    assert_eq!(uniform.joint.admitted, [kh("bob"), kh("w1"), kh("w5")]);
    assert_eq!((reference.joint.joint, uniform.joint.joint), (3.0, 3.0));
}

/// **A peering joins two parties in the acquaintance graph and confers no
/// scope** (design §§6.3, 16.2.1).  Three sources of standing enter as one
/// edge per pair; only adoption carries authority, and the scope walk is
/// built from adoptions alone.
// acceptance: MET-11
#[test]
fn a_peering_is_an_acquaintance_edge_and_gives_its_parties_no_scope_over_each_other() {
    let (w, own, pushed) = observed();
    let table = table_with(
        kh("carol"),
        &w,
        &own.iter().collect::<Vec<_>>(),
        &["alice", "bob", "carol", "w1", "w5"],
    );
    let mut v: NodeView = view("carol", table, "alice", &[1]);
    v.set_now(w.clock);
    let fab = Fabric::with(&[kh("alice"), kh("w2")]);
    let lookup = ids();

    let before = v.evidence();
    assert!(
        !before.acquaintances.contains(&(kh("bob"), kh("w5"))),
        "no edge before the peering arrives"
    );

    for r in &pushed {
        v.take_object(&*fab, &kh("alice"), KIND_TRANSACTION, &r.bytes, &lookup);
    }
    let ev = v.evidence();

    // the positive: the pair is joined in the acquaintance graph
    assert!(
        ev.acquaintances.contains(&(kh("bob"), kh("w5"))),
        "the peering is an acquaintance edge: {:?}",
        ev.acquaintances
    );

    // **and it is not an adoption edge**: nothing about a peering says one
    // party holds authority over the other, which is the whole reason
    // peering can create the cycles the authority relation forbids
    assert!(
        !ev.adoptions.contains(&(kh("bob"), kh("w5"))),
        "and not an authority edge"
    );
    assert!(
        !ev.adoptions.contains(&(kh("w5"), kh("bob"))),
        "in either direction"
    );

    // the negative: it confers no scope.  w5 is reachable to the metric
    // over the acquaintance edge and is in nobody's subtree by it
    let scope = ev.scope();
    assert!(
        !scope.horizon(&kh("bob"), 1).contains(&kh("w5")),
        "a peering puts neither party in the other's scope"
    );
    assert!(!scope.horizon(&kh("w5"), 1).contains(&kh("bob")));
    // while an adoption in the same evidence does exactly that
    assert!(
        scope.horizon(&kh("bob"), 1).contains(&kh("w1")),
        "control: an adoption does confer scope"
    );
}
