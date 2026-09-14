//! A world for the node entries: named identities, real signed
//! transactions, and an in-process fabric that records the frames a node
//! sends on each of its sessions.
//!
//! The fabric carries the wire bytes the transport would carry; what it
//! stands in for is the QUIC session, not the encoding.

#![allow(dead_code)]

use rhtn_archive::chain::Archive;
use rhtn_archive::record::Record;
use rhtn_archive::topology::Table;
use rhtn_archive::tx::*;
use rhtn_archive::{Keyhash, Txid};
use rhtn_crypto::identity::testkit::test_identity;
use rhtn_crypto::{Identity, SigningIdentity};
use rhtn_node::view::NodeView;
use rhtn_node::{Adjacency, resolution::NetworkPoint};
use std::collections::{BTreeMap, BTreeSet};
use std::sync::{Arc, Mutex};

pub const NAMES: [&str; 26] = [
    "alice", "bob", "carol", "alice2", "w1", "w2", "w3", "w4", "w5", "w6", "w7", "w8", "w9", "w10", "w11",
    "w12", "w13", "w14", "w15", "w16", "c1", "c2", "c3", "c4", "c5", "witness",
];

pub fn ids() -> Vec<Identity> {
    NAMES.iter().map(|n| test_identity(n).public).collect()
}

pub fn id(n: &str) -> SigningIdentity {
    test_identity(n)
}

pub fn kh(n: &str) -> Keyhash {
    test_identity(n).public.keyhash
}

/// One recorded frame: who it went to, its type, and its body.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SentFrame {
    pub to: Keyhash,
    pub frame_type: u64,
    pub body: Vec<u8>,
}

/// An in-process stand-in for the sessions a node holds: control frames
/// and requests are recorded apart, as they travel apart, and a peer this
/// node serves carries no request.
#[derive(Default)]
pub struct Fabric {
    peers: Mutex<BTreeSet<Keyhash>>,
    served: Mutex<BTreeSet<Keyhash>>,
    sent: Mutex<Vec<SentFrame>>,
    requests: Mutex<Vec<SentFrame>>,
}

impl Fabric {
    pub fn with(peers: &[Keyhash]) -> Arc<Self> {
        let f = Arc::new(Fabric::default());
        for p in peers {
            f.peers.lock().unwrap().insert(*p);
        }
        f
    }

    pub fn drop_session(&self, peer: &Keyhash) {
        self.peers.lock().unwrap().remove(peer);
    }

    pub fn add_session(&self, peer: &Keyhash) {
        self.peers.lock().unwrap().insert(*peer);
    }

    pub fn frames(&self) -> Vec<SentFrame> {
        self.sent.lock().unwrap().clone()
    }

    pub fn clear(&self) {
        self.sent.lock().unwrap().clear();
        self.requests.lock().unwrap().clear();
    }

    /// Mark `peer` as a session this node serves: it carries no request.
    pub fn serve(&self, peer: &Keyhash) {
        self.served.lock().unwrap().insert(*peer);
    }

    /// Every request sent: to whom, of what type, with what body.
    pub fn requests(&self) -> Vec<SentFrame> {
        self.requests.lock().unwrap().clone()
    }

    /// Requests of one type sent to one peer.
    pub fn requests_to(&self, peer: &Keyhash, request_type: u64) -> Vec<Vec<u8>> {
        self.requests().into_iter().filter(|f| f.to == *peer && f.frame_type == request_type).map(|f| f.body).collect()
    }

    pub fn request_count(&self, request_type: u64) -> usize {
        self.requests().iter().filter(|f| f.frame_type == request_type).count()
    }

    /// Frames of one type sent to one peer.
    pub fn to(&self, peer: &Keyhash, frame_type: u64) -> Vec<Vec<u8>> {
        self.frames().into_iter().filter(|f| f.to == *peer && f.frame_type == frame_type).map(|f| f.body).collect()
    }

    /// Every peer that got a frame of this type.
    pub fn recipients(&self, frame_type: u64) -> BTreeSet<Keyhash> {
        self.frames().into_iter().filter(|f| f.frame_type == frame_type).map(|f| f.to).collect()
    }

    pub fn count(&self, frame_type: u64) -> usize {
        self.frames().iter().filter(|f| f.frame_type == frame_type).count()
    }
}

impl Adjacency for Fabric {
    fn peers(&self) -> Vec<Keyhash> {
        self.peers.lock().unwrap().iter().copied().collect()
    }
    fn send(&self, peer: &Keyhash, frame_type: u64, body: &[u8]) {
        self.sent.lock().unwrap().push(SentFrame { to: *peer, frame_type, body: body.to_vec() });
    }
    fn request(&self, peer: &Keyhash, request_type: u64, body: &[u8]) -> bool {
        if !self.peers.lock().unwrap().contains(peer) || self.served.lock().unwrap().contains(peer) {
            return false;
        }
        self.requests.lock().unwrap().push(SentFrame { to: *peer, frame_type: request_type, body: body.to_vec() });
        true
    }
}

/// Signed transactions and the archives they advance, shared by every node
/// in a scenario: the same objects reach every party, as a flood would.
pub struct World {
    pub archives: BTreeMap<Keyhash, Archive>,
    pub store: BTreeMap<Txid, Vec<u8>>,
    pub clock: u64,
}

impl Default for World {
    fn default() -> Self {
        Self::new()
    }
}

impl World {
    pub fn new() -> Self {
        let mut w = World { archives: BTreeMap::new(), store: BTreeMap::new(), clock: 1_800_000_000 };
        for n in NAMES {
            w.archives.insert(kh(n), Archive::new(kh(n)));
        }
        w
    }

    pub fn tick(&mut self) -> u64 {
        self.clock += 3600;
        self.clock
    }

    pub fn back(&self, n: &str) -> Vec<Txid> {
        self.archives[&kh(n)].next_back_pointers()
    }

    fn commit(&mut self, tx_type: u64, body: &[u8], signers: &[&str]) -> Record {
        let sids: Vec<SigningIdentity> = signers.iter().map(|s| id(s)).collect();
        let refs: Vec<&SigningIdentity> = sids.iter().collect();
        let env = envelope(tx_type, body, &refs);
        let rec = Record::parse(&env).expect("well-formed");
        for s in signers {
            self.archives.get_mut(&kh(s)).unwrap().append(rec.clone()).expect("appends");
        }
        self.store.insert(rec.txid, env);
        rec
    }

    pub fn formation(&mut self, a: &str, b: &str) -> Record {
        let t = self.tick();
        let (ba, bb) = (self.back(a), self.back(b));
        let root = rhtn_codec::cose::sha256(format!("formation:{a}:{b}:{t}").as_bytes());
        let body = formation_body([&ba, &bb], [&kh(a), &kh(b)], t, t + 600, &root);
        self.commit(TYPE_PRESENCE, &body, &[a, b])
    }

    /// Presence evidence between `a` and `b`: a formation record where both
    /// keys are fresh, and otherwise a normal record witnessed by the
    /// world's witness, since a key appears in at most one formation
    /// record, its first (`wire-format.md` §3.2).
    pub fn meet(&mut self, a: &str, b: &str) -> Record {
        if self.archives[&kh(a)].is_empty() && self.archives[&kh(b)].is_empty() {
            return self.formation(a, b);
        }
        let t = self.tick();
        let back = vec![self.back(a), self.back(b), self.back("witness")];
        let root = rhtn_codec::cose::sha256(format!("meeting:{a}:{b}:{t}").as_bytes());
        let w = Witness { keyhash: kh("witness"), nominated_by: kh(a), flags: 3 };
        let body = presence_record_body(&back, [&kh(a), &kh(b)], &[w], t, t + 600, &root);
        self.commit(TYPE_PRESENCE, &body, &[a, b, "witness"])
    }

    /// An adoption of `node` under `patron` on a fresh presence record.
    pub fn adopt(&mut self, node: &str, patron: &str, series: u32) -> (Record, Record) {
        let pop = self.meet(patron, node);
        let t = self.tick();
        let (bn, bp) = (self.back(node), self.back(patron));
        let a = Adoption {
            node: kh(node),
            patron: kh(patron),
            locator: Locator { anchor: kh(patron), path: vec![0x10], nibbles: 2, seqno: Seqno { series, counter: 0 } },
            timestamp: t,
            key_material: None,
            evidence: Evidence::Presence(pop.txid),
            presented_head: None,
            back: [&bn, &bp],
        };
        let body = adoption_body(&a);
        let rec = self.commit(TYPE_ADOPTION, &body, &[node, patron]);
        (rec, pop)
    }

    pub fn depart(&mut self, node: &str, patron: &str, seqno: Seqno) -> Record {
        let t = self.tick();
        let b = self.back(node);
        let body = departure_body(&b, &kh(node), &kh(patron), seqno, t, None);
        self.commit(TYPE_DEPARTURE, &body, &[node])
    }

    /// A series reissue for `node` under `patron`, countersigned by the
    /// patron: leaving `leaving`, entering `new_series` at counter 0.
    pub fn reissue(&mut self, node: &str, patron: &str, leaving: Seqno, new_series: u32) -> Record {
        let t = self.tick();
        let (bn, bp) = (self.back(node), self.back(patron));
        let body = reissue_body([&bn, &bp], &kh(node), &kh(patron), leaving, new_series, t);
        self.commit(TYPE_REISSUE, &body, &[node, patron])
    }

    pub fn disavow(&mut self, patron: &str, node: &str, code: Option<u64>) -> Record {
        let t = self.tick();
        let b = self.back(patron);
        let body = disavowal_body(&b, &kh(patron), &kh(node), t, code);
        self.commit(TYPE_DISAVOWAL, &body, &[patron])
    }

    pub fn bytes(&self, t: &Txid) -> Vec<u8> {
        self.store[t].clone()
    }
}

/// Apply a chain of adoptions to a fresh table, evidence and all.
pub fn table_with(me: Keyhash, w: &World, records: &[&Record], infra: &[&str]) -> Table {
    let mut t = Table::with_me(me);
    for n in infra {
        t.mark_infra(kh(n));
    }
    let lookup = ids();
    for r in records {
        t.apply(r, &lookup, &w.store, None).unwrap_or_else(|e| panic!("apply: {e:?}"));
    }
    t
}

/// A node view for `name` with the given position and table.
pub fn view(name: &str, table: Table, anchor: &str, path: &[u8]) -> NodeView {
    let mut v = NodeView::new(Arc::new(id(name)), Locator { anchor: kh(anchor), path: pack_path(path), nibbles: path.len() as u64, seqno: Seqno { series: 1, counter: 0 } });
    v.table = table;
    v
}

/// Nibble packing, as `wire-format.md` §2.1 has it.
pub fn pack_path(ix: &[u8]) -> Vec<u8> {
    rhtn_node::resolution::Path::from_indices(ix).bytes
}

pub fn point(last: u8, port: u16) -> NetworkPoint {
    NetworkPoint::new([127, 0, 0, last], Some(port as u64))
}

/// Poll `done` until it holds or `ms` elapse.
pub async fn until(ms: u64, mut done: impl FnMut() -> bool) -> bool {
    let end = tokio::time::Instant::now() + std::time::Duration::from_millis(ms);
    while tokio::time::Instant::now() < end {
        if done() {
            return true;
        }
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
    }
    done()
}
