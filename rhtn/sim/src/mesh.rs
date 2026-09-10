//! An in-process mesh of node views with severable links: what
//! `tla/PartitionMerge` is restated over.
//!
//! The model's questions are asked of the running code here.  A link is
//! severed rather than lossy, because that is what the model's `severed`
//! set is; healing is reconciliation, which the wire says is a replay of
//! the same frames (`wire-format.md` §10.1.3).

use rhtn_archive::record::Record;
use rhtn_archive::Keyhash;
use rhtn_crypto::Identity;
use rhtn_node::propagation::{FRAME_TOPOLOGY_PUSH, decode_push};
use rhtn_node::store::KIND_TRANSACTION;
use rhtn_node::view::NodeView;
use rhtn_node::Adjacency;
use std::collections::{BTreeMap, BTreeSet};
use std::sync::Mutex;

/// One node's outbound queue in the mesh.
#[derive(Default)]
struct Outbox(Mutex<Vec<(Keyhash, u64, Vec<u8>)>>);

/// A peer list that answers for one node, so `adjacent` can filter, and
/// the outbox its frames land in.
struct Links {
    peers: Vec<Keyhash>,
    out: Outbox,
}

impl Adjacency for Links {
    fn peers(&self) -> Vec<Keyhash> {
        self.peers.clone()
    }
    fn send(&self, peer: &Keyhash, frame_type: u64, body: &[u8]) {
        self.out.0.lock().unwrap().push((*peer, frame_type, body.to_vec()));
    }
}

/// Several node views with links between them, some of which may be cut.
pub struct Mesh {
    pub views: BTreeMap<Keyhash, NodeView>,
    pub identities: Vec<Identity>,
    /// Unordered pairs that carry nothing.
    severed: BTreeSet<(Keyhash, Keyhash)>,
    /// The records each node actually signed: the model's `store[t.subj]`,
    /// and the ground truth `SelfTruth` compares a node's own view against.
    origin: BTreeMap<Keyhash, Vec<Record>>,
}

fn pair(a: &Keyhash, b: &Keyhash) -> (Keyhash, Keyhash) {
    if a <= b { (*a, *b) } else { (*b, *a) }
}

impl Mesh {
    pub fn new(views: Vec<NodeView>, identities: Vec<Identity>) -> Mesh {
        let views = views.into_iter().map(|v| (v.me(), v)).collect();
        Mesh { views, identities, severed: BTreeSet::new(), origin: BTreeMap::new() }
    }

    pub fn nodes(&self) -> Vec<Keyhash> {
        self.views.keys().copied().collect()
    }

    pub fn sever(&mut self, a: &Keyhash, b: &Keyhash) {
        self.severed.insert(pair(a, b));
    }

    pub fn heal_all(&mut self) {
        self.severed.clear();
    }

    pub fn is_severed(&self, a: &Keyhash, b: &Keyhash) -> bool {
        self.severed.contains(&pair(a, b))
    }

    pub fn severed_count(&self) -> usize {
        self.severed.len()
    }

    /// Note that this record was signed: it enters each signer's own set,
    /// which is what gossip may copy and may never create.
    pub fn originate(&mut self, rec: &Record) {
        for s in &rec.signers {
            let set = self.origin.entry(*s).or_default();
            if !set.iter().any(|r| r.txid == rec.txid) {
                set.push(rec.clone());
            }
        }
    }

    /// Every transaction a node holds is in its subject's own store: the
    /// model's `t ∈ store[t.subj]`, asked of the running views.  Gossip
    /// copies, never creates.
    pub fn no_invention(&self) -> Result<(), String> {
        for (holder, v) in &self.views {
            for rec in v.store.transactions() {
                for subject in rhtn_node::store::subjects(rec) {
                    let Some(sv) = self.views.get(&subject) else { continue };
                    if !sv.store.holds_txid(&rec.txid) {
                        return Err(format!("{} holds {} which its subject {} does not", hex4(holder), hex4(&rec.txid), hex4(&subject)));
                    }
                }
            }
        }
        Ok(())
    }

    /// Deliver one object to one node and nowhere else, as an injection
    /// would; what that node forwards is dropped.
    pub fn inject(&mut self, to: Keyhash, from: Keyhash, object: &[u8]) {
        let links = Links { peers: Vec::new(), out: Outbox::default() };
        let ids = self.identities.clone();
        if let Some(v) = self.views.get_mut(&to) {
            v.take_object(&links, &from, KIND_TRANSACTION, object, &ids);
        }
    }

    /// A subject is never behind its own store: what a node's own store
    /// says about itself is what it actually signed.
    pub fn self_truth(&self) -> Result<(), String> {
        for (me, v) in &self.views {
            let signed = self.origin.get(me).cloned().unwrap_or_default();
            let truth = patrons_of(signed.iter(), me);
            let held = self.patrons_from_store(v, me);
            if truth != held {
                return Err(format!("{}: signed {:?} but its own store implies {:?}", hex4(me), truth.iter().map(hex4).collect::<Vec<_>>(), held.iter().map(hex4).collect::<Vec<_>>()));
            }
        }
        Ok(())
    }

    /// What one node's store says another node's patrons are.
    pub fn view_patrons(&self, viewer: &Keyhash, subject: &Keyhash) -> BTreeSet<Keyhash> {
        self.patrons_from_store(&self.views[viewer], subject)
    }

    fn patrons_from_store(&self, v: &NodeView, subject: &Keyhash) -> BTreeSet<Keyhash> {
        patrons_of(v.store.transactions(), subject)
    }

    /// Every pair of nodes agrees about every subject.
    pub fn agreed(&self) -> Result<(), String> {
        let ns = self.nodes();
        for c in &ns {
            let first = self.view_patrons(&ns[0], c);
            for u in &ns[1..] {
                let theirs = self.view_patrons(u, c);
                if theirs != first {
                    return Err(format!("about {}: {} says {:?}, {} says {:?}", hex4(c), hex4(&ns[0]), first.iter().map(hex4).collect::<Vec<_>>(), hex4(u), theirs.iter().map(hex4).collect::<Vec<_>>()));
                }
            }
        }
        Ok(())
    }

    /// Deliver one object from `from` to every unsevered link, and let each
    /// receiver's own forwarding rule carry it on, to a fixed point.
    pub fn flood(&mut self, from: Keyhash, object: &[u8]) {
        // the originator holds what it signed
        self.flood_one(from, from, object);
        let mut queue: Vec<(Keyhash, Keyhash, Vec<u8>)> = self
            .nodes()
            .into_iter()
            .filter(|n| *n != from && !self.is_severed(&from, n))
            .map(|n| (from, n, object.to_vec()))
            .collect();
        let mut guard = 0;
        while let Some((sender, to, bytes)) = queue.pop() {
            guard += 1;
            if guard > 10_000 {
                break;
            }
            let peers: Vec<Keyhash> = self.nodes().into_iter().filter(|n| *n != to && !self.is_severed(&to, n)).collect();
            let links = Links { peers, out: Outbox::default() };
            let ids = self.identities.clone();
            let Some(v) = self.views.get_mut(&to) else { continue };
            v.take_object(&links, &sender, KIND_TRANSACTION, &bytes, &ids);
            for (peer, ft, body) in links.out.0.into_inner().unwrap() {
                if ft != FRAME_TOPOLOGY_PUSH {
                    continue;
                }
                if let Ok((_, obj)) = decode_push(&body) {
                    queue.push((to, peer, obj));
                }
            }
        }
    }

    /// Reconciliation: `holder` replays its whole store to `to`, and what
    /// `to` stores floods on from there (`wire-format.md` §10.1.3).
    pub fn reconcile(&mut self, holder: Keyhash, to: Keyhash) {
        let objects: Vec<Vec<u8>> = self.views[&holder].store.objects().into_iter().filter(|(k, _)| *k == KIND_TRANSACTION).map(|(_, b)| b).collect();
        for object in objects {
            let peers: Vec<Keyhash> = self.nodes().into_iter().filter(|n| *n != to && !self.is_severed(&to, n)).collect();
            let links = Links { peers, out: Outbox::default() };
            let ids = self.identities.clone();
            let Some(v) = self.views.get_mut(&to) else { continue };
            v.take_object(&links, &holder, KIND_TRANSACTION, &object, &ids);
            for (peer, ft, body) in links.out.0.into_inner().unwrap() {
                if ft == FRAME_TOPOLOGY_PUSH
                    && let Ok((_, obj)) = decode_push(&body) {
                        self.flood_one(to, peer, &obj);
                    }
            }
        }
    }

    fn flood_one(&mut self, from: Keyhash, to: Keyhash, object: &[u8]) {
        let peers: Vec<Keyhash> = self.nodes().into_iter().filter(|n| *n != to && !self.is_severed(&to, n)).collect();
        let links = Links { peers, out: Outbox::default() };
        let ids = self.identities.clone();
        let Some(v) = self.views.get_mut(&to) else { return };
        v.take_object(&links, &from, KIND_TRANSACTION, object, &ids);
        for (peer, ft, body) in links.out.0.into_inner().unwrap() {
            if ft == FRAME_TOPOLOGY_PUSH
                && let Ok((_, obj)) = decode_push(&body) {
                    self.flood_one(to, peer, &obj);
                }
        }
    }

    /// Reconcile every unsevered pair in both directions until nothing
    /// changes.
    pub fn reconcile_all(&mut self) {
        for _ in 0..self.views.len() + 2 {
            let ns = self.nodes();
            for a in &ns {
                for b in &ns {
                    if a != b && !self.is_severed(a, b) {
                        self.reconcile(*a, *b);
                    }
                }
            }
        }
    }

    /// Put a presence record in every node's evidence store, so an
    /// adoption's evaluation can dereference what it names.
    pub fn share_presence(&mut self, txid: [u8; 32], bytes: Vec<u8>) {
        for v in self.views.values_mut() {
            v.store.keep_presence(txid, bytes.clone());
        }
    }
}

fn hex4(k: &Keyhash) -> String {
    k[..4].iter().map(|b| format!("{b:02x}")).collect()
}

/// The patrons a set of records leaves open for `subject`, in effective-time
/// order: adoptions open a binding, departures and disavowals close one.
fn patrons_of<'a>(records: impl Iterator<Item = &'a Record>, subject: &Keyhash) -> BTreeSet<Keyhash> {
    let mut rs: Vec<&Record> = records.collect();
    rs.sort_by_key(|r| (r.effective, r.txid));
    let mut open: BTreeSet<Keyhash> = BTreeSet::new();
    for rec in rs {
        match rec.tx_type {
            rhtn_archive::tx::TYPE_ADOPTION if rec.field_hash(1).as_ref() == Some(subject) => {
                if let Some(p) = rec.field_hash(2) {
                    open.insert(p);
                }
            }
            rhtn_archive::tx::TYPE_DEPARTURE if rec.field_hash(1).as_ref() == Some(subject) => {
                if let Some(p) = rec.field_hash(2) {
                    open.remove(&p);
                }
            }
            rhtn_archive::tx::TYPE_DISAVOWAL if rec.field_hash(2).as_ref() == Some(subject) => {
                if let Some(p) = rec.field_hash(1) {
                    open.remove(&p);
                }
            }
            _ => {}
        }
    }
    open
}
