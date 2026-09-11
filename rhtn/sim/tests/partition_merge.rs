//! `models/tla/PartitionMerge` restated over the running code: the two
//! safety invariants at every step, and convergence once the network is
//! healed and the topology quiesces.
//!
//! The model checks these over a full state graph of three nodes and three
//! events.  What runs here is the implementation's own storage and
//! forwarding, over the same shape.

use rhtn_archive::chain::Archive;
use rhtn_archive::record::Record;
use rhtn_archive::topology::Table;
use rhtn_archive::tx::*;
use rhtn_archive::{Keyhash, Txid};
use rhtn_crypto::identity::testkit::test_identity;
use rhtn_crypto::{Identity, SigningIdentity};
use rhtn_node::view::NodeView;
use rhtn_sim::mesh::Mesh;
use std::collections::BTreeMap;
use std::sync::Arc;

const NODES: [&str; 3] = ["alice", "bob", "carol"];
/// The witness on every meeting after a key's first (`wire-format.md`
/// §3.2): it signs presence records and runs no node.
const WITNESS: &str = "witness";

fn kh(n: &str) -> Keyhash {
    test_identity(n).public.keyhash
}

fn ids() -> Vec<Identity> {
    NODES.iter().chain([WITNESS].iter()).map(|n| test_identity(n).public).collect()
}

/// The signing side: archives advance as transactions are made, exactly as
/// they do in the archive crate's own tests.
struct Signers {
    archives: BTreeMap<Keyhash, Archive>,
    clock: u64,
}

impl Signers {
    fn new() -> Signers {
        Signers { archives: NODES.iter().chain([WITNESS].iter()).map(|n| (kh(n), Archive::new(kh(n)))).collect(), clock: 1_800_000_000 }
    }
    fn tick(&mut self) -> u64 {
        self.clock += 3600;
        self.clock
    }
    fn back(&self, n: &str) -> Vec<Txid> {
        self.archives[&kh(n)].next_back_pointers()
    }
    fn commit(&mut self, tx_type: u64, body: &[u8], signers: &[&str]) -> Record {
        let sids: Vec<SigningIdentity> = signers.iter().map(|s| test_identity(s)).collect();
        let refs: Vec<&SigningIdentity> = sids.iter().collect();
        let rec = Record::parse(&envelope(tx_type, body, &refs)).expect("well-formed");
        for s in signers {
            self.archives.get_mut(&kh(s)).unwrap().append(rec.clone()).expect("appends");
        }
        rec
    }
    fn formation(&mut self, a: &str, b: &str) -> Record {
        let t = self.tick();
        let (ba, bb) = (self.back(a), self.back(b));
        let root = rhtn_codec::cose::sha256(format!("f:{a}:{b}:{t}").as_bytes());
        let body = formation_body([&ba, &bb], [&kh(a), &kh(b)], t, t + 600, &root);
        self.commit(TYPE_PRESENCE, &body, &[a, b])
    }
    /// Presence evidence: a formation for two fresh keys, otherwise a
    /// witnessed normal record, a key forming only once.
    fn meet(&mut self, a: &str, b: &str) -> Record {
        if self.archives[&kh(a)].is_empty() && self.archives[&kh(b)].is_empty() {
            return self.formation(a, b);
        }
        let t = self.tick();
        let back = vec![self.back(a), self.back(b), self.back(WITNESS)];
        let root = rhtn_codec::cose::sha256(format!("m:{a}:{b}:{t}").as_bytes());
        let w = Witness { keyhash: kh(WITNESS), nominated_by: kh(a), flags: 3 };
        let body = presence_record_body(&back, [&kh(a), &kh(b)], &[w], t, t + 600, &root);
        self.commit(TYPE_PRESENCE, &body, &[a, b, WITNESS])
    }
    fn adoption(&mut self, node: &str, patron: &str, pop: Txid, series: u32) -> Record {
        let t = self.tick();
        let (bn, bp) = (self.back(node), self.back(patron));
        let a = Adoption {
            node: kh(node),
            patron: kh(patron),
            locator: Locator::root(kh(patron), Seqno { series, counter: 0 }),
            timestamp: t,
            key_material: None,
            evidence: Evidence::Presence(pop),
            presented_head: None,
            back: [&bn, &bp],
        };
        self.commit(TYPE_ADOPTION, &adoption_body(&a), &[node, patron])
    }
    fn departure(&mut self, node: &str, patron: &str, seqno: Seqno) -> Record {
        let t = self.tick();
        let b = self.back(node);
        let body = departure_body(&b, &kh(node), &kh(patron), seqno, t, None);
        self.commit(TYPE_DEPARTURE, &body, &[node])
    }
}

/// Three views, each its own, all mutually linked.
fn mesh() -> Mesh {
    let views: Vec<NodeView> = NODES
        .iter()
        .map(|n| {
            let mut t = Table::with_me(kh(n));
            for m in NODES {
                t.mark_infra(kh(m));
            }
            let mut v = NodeView::new(Arc::new(test_identity(n)), Locator::root(kh("alice"), Seqno { series: 1, counter: 0 }));
            v.table = t;
            // every node is adjacent to the other two, so h_store covers them
            for m in NODES {
                if kh(m) != kh(n) {
                    v.peers.insert(kh(m));
                }
            }
            v
        })
        .collect();
    Mesh::new(views, ids())
}

/// Both safety invariants, checked wherever the model checks them.
fn safety(m: &Mesh) {
    m.no_invention().expect("NoInvention");
    m.self_truth().expect("SelfTruth");
}

/// Put every node inside every other's h_store by seeding one adoption
/// everywhere, and share the presence records evaluation needs.
fn seed(m: &mut Mesh, s: &mut Signers) -> Vec<Record> {
    let mut made = Vec::new();
    let pop_ab = s.meet("alice", "bob");
    let a_b = s.adoption("bob", "alice", pop_ab.txid, 1);
    let pop_bc = s.meet("bob", "carol");
    let a_c = s.adoption("carol", "bob", pop_bc.txid, 2);
    for r in [&pop_ab, &pop_bc] {
        m.share_presence(r.txid, r.bytes.clone());
    }
    for r in [&pop_ab, &a_b, &pop_bc, &a_c] {
        m.originate(r);
        made.push(r.clone());
    }
    for r in [&a_b, &a_c] {
        m.flood(r.signers[0], &r.bytes);
    }
    // a node joining catches up on what it was not yet in range for when it
    // passed: reconciliation is a replay of the same frames
    m.reconcile_all();
    made
}

// The milestone exit criterion rather than a catalogue entry: the two
// safety invariants and the convergence property, over the running code.
#[test]
fn views_converge_after_a_partition_heals() {
    let mut s = Signers::new();
    let mut m = mesh();
    seed(&mut m, &mut s);
    safety(&m);
    m.agreed().expect("agreed before the partition");
    assert_eq!(m.view_patrons(&kh("alice"), &kh("carol")), [kh("bob")].into());

    // the network partitions: carol is cut off from both others
    m.sever(&kh("alice"), &kh("carol"));
    m.sever(&kh("bob"), &kh("carol"));
    assert_eq!(m.severed_count(), 2);

    // carol departs from bob while cut off, so the two sides diverge
    let dep = s.departure("carol", "bob", Seqno { series: 2, counter: 1 });
    m.originate(&dep);
    m.flood(kh("carol"), &dep.bytes);
    safety(&m);
    assert_eq!(m.view_patrons(&kh("carol"), &kh("carol")), [].into(), "carol knows she left");
    assert_eq!(m.view_patrons(&kh("alice"), &kh("carol")), [kh("bob")].into(), "alice has not heard");
    assert!(m.agreed().is_err(), "the views have diverged, which is the point of the partition");

    // the network heals and stays healed; the topology quiesces
    m.heal_all();
    assert_eq!(m.severed_count(), 0);
    m.reconcile_all();
    safety(&m);
    m.agreed().expect("every pair agrees about every subject once healed and quiesced");
    for viewer in NODES {
        assert_eq!(m.view_patrons(&kh(viewer), &kh("carol")), [].into(), "{viewer} sees the departure");
        assert_eq!(m.view_patrons(&kh(viewer), &kh("bob")), [kh("alice")].into());
    }
}

#[test]
fn a_partition_that_never_heals_does_not_claim_convergence() {
    let mut s = Signers::new();
    let mut m = mesh();
    seed(&mut m, &mut s);
    m.sever(&kh("alice"), &kh("carol"));
    m.sever(&kh("bob"), &kh("carol"));
    let dep = s.departure("carol", "bob", Seqno { series: 2, counter: 1 });
    m.originate(&dep);
    m.flood(kh("carol"), &dep.bytes);
    m.reconcile_all();
    // reconciliation runs, and the cut links still carry nothing
    safety(&m);
    assert!(m.agreed().is_err(), "convergence is conditional on the network healing");
}

#[test]
fn gossip_copies_and_never_creates() {
    let mut s = Signers::new();
    let mut m = mesh();
    seed(&mut m, &mut s);
    // an adoption of carol handed to alice alone, as an injection would:
    // alice holds a transaction its subject does not, and the check says so
    let pop = s.meet("alice", "carol");
    m.share_presence(pop.txid, pop.bytes.clone());
    m.originate(&pop);
    let later = s.adoption("carol", "alice", pop.txid, 9);
    m.originate(&later);
    m.inject(kh("alice"), kh("bob"), &later.bytes);
    assert!(m.no_invention().is_err(), "the check is live: a store holding what its subject does not is caught");
    // once the subject holds what it signed, as origination makes it, it passes
    m.flood(kh("carol"), &later.bytes);
    m.no_invention().expect("a genuinely originated object passes everywhere");
    safety(&m);
    assert_eq!(m.view_patrons(&kh("alice"), &kh("carol")), [kh("alice"), kh("bob")].into(), "adopting elsewhere leaves the old binding in view");
}
