//! The topology store: what a node holds, and the identities by which it
//! decides an object is a duplicate (`wire-format.md` §10.1.1, §10.1.2).
//!
//! The store is the seen-set.  There is no suppression cache beside it, and
//! §10.1.2 says there should not be.

use rhtn_archive::record::{Record, SigStatus};
use rhtn_archive::tx::*;
use rhtn_archive::{Keyhash, Txid};
use rhtn_codec::cbor::*;
use rhtn_codec::schema;
use rhtn_crypto::verify::{self, Lookup};
use std::collections::{BTreeMap, BTreeSet};

/// `TopologyPush` field 1 (`wire-format.md` §10.1): the one wrapper field.
pub const KIND_TRANSACTION: u64 = 0;
pub const KIND_ENDPOINT_RECORD: u64 = 1;

/// An `EndpointRecord` (`wire-format.md` §7.6) as a holder reads it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EndpointRecord {
    pub node: Keyhash,
    /// The publisher's preference order; entries are distinct.
    pub endpoints: Vec<Vec<u8>>,
    pub seqno: Seqno,
    pub bytes: Vec<u8>,
}

impl EndpointRecord {
    pub fn parse(b: &[u8]) -> Result<Self, String> {
        let item = parse_all(b).map_err(|e| e.0)?;
        schema::check_kind(b, "EndpointRecord", &item).map_err(|e| e.0)?;
        let Item::Map(m) = &item else { return Err("not a map".into()) };
        let node: Keyhash = match map_get(m, 1) {
            Some(Item::Bytes(r)) if r.len() == 32 => b[r.clone()].try_into().map_err(|_| "node")?,
            _ => return Err("field 1".into()),
        };
        let r2 = value_slice(b, 2).ok_or("field 2")?;
        let endpoints: Vec<Vec<u8>> = array_item_ranges(b, r2.start).ok_or("endpoints walk")?.into_iter().map(|r| b[r].to_vec()).collect();
        if endpoints.is_empty() || endpoints.len() > 8 {
            return Err("endpoint count".into());
        }
        let mut sorted = endpoints.clone();
        sorted.sort();
        sorted.dedup();
        if sorted.len() != endpoints.len() {
            return Err("a repeated NetworkPoint is malformed".into());
        }
        let Some(Item::Array(sq)) = map_get(m, 3) else { return Err("field 3".into()) };
        let seqno = Seqno { series: as_uint(sq.first().ok_or("series")?).ok_or("series")? as u32, counter: as_uint(sq.get(1).ok_or("counter")?).ok_or("counter")? as u32 };
        Ok(EndpointRecord { node, endpoints, seqno, bytes: b.to_vec() })
    }

    /// Self-signed by the node it names; checkable only by a holder that
    /// already has the key (`wire-format.md` §7.6).  Unknown only for want
    /// of the key: a signature that fails or is malformed under a key this
    /// holder has is a failure, never gossip.
    pub fn signature_checks<L: Lookup + ?Sized>(&self, ids: &L) -> Option<bool> {
        match verify::record(ids, "EndpointRecord", &self.bytes) {
            Ok(()) => Some(true),
            Err(verify::Failure::MissingKey(_)) => None,
            Err(verify::Failure::Invalid(_)) => Some(false),
        }
    }
}

/// The receiver's own view of distance, which is the only thing the
/// storage rule consults (`wire-format.md` §10.1.1, design §15.1).
pub trait Horizon {
    /// Whether `x` lies within `h` adoption-or-sibling edges of this node.
    fn within(&self, x: &Keyhash, h: usize) -> bool;
}

/// Whether the storage rule takes this transaction: its subject's position,
/// as the transaction establishes it, falls within the receiver's `h_store`
/// (`wire-format.md` §10.1.1).
///
/// An adoption's subject sits one edge below the patron it names, so a
/// patron within h-1 puts the subject within h — which is what makes a new
/// member's arrival flood at all.  Reading the *issuer* as the subject would
/// put a patron's adoption of a distant node in range of everyone near the
/// patron, which is not whose neighbourhood changed.
pub fn stores(rec: &Record, hz: &dyn Horizon) -> bool {
    const H: usize = 2;
    match rec.tx_type {
        TYPE_PEERING => subjects(rec).iter().any(|s| hz.within(s, H)),
        TYPE_ADOPTION | TYPE_DEPARTURE | TYPE_DISAVOWAL | TYPE_REISSUE => {
            let subject_here = subjects(rec).iter().any(|s| hz.within(s, H));
            let counterparty = match rec.tx_type {
                TYPE_DISAVOWAL => rec.field_hash(1),
                _ => rec.field_hash(2),
            };
            subject_here || counterparty.is_some_and(|p| hz.within(&p, H - 1))
        }
        _ => false,
    }
}

/// The node whose position a topology transaction changes (`wire-format.md`
/// §10.1.1).  A peering has two, so the object is in range if either is.
pub fn subjects(rec: &Record) -> Vec<Keyhash> {
    match rec.tx_type {
        TYPE_ADOPTION | TYPE_DEPARTURE | TYPE_REISSUE => rec.field_hash(1).into_iter().collect(),
        TYPE_DISAVOWAL => rec.field_hash(2).into_iter().collect(),
        TYPE_PEERING => [rec.field_hash(1), rec.field_hash(2)].into_iter().flatten().collect(),
        _ => Vec::new(),
    }
}

/// Whether a transaction type travels in the topology class at all.
pub fn is_topology_class(tx_type: u64) -> bool {
    matches!(tx_type, TYPE_ADOPTION | TYPE_DEPARTURE | TYPE_DISAVOWAL | TYPE_PEERING | TYPE_REISSUE)
}

/// An object held for a prerequisite: a signer's key material, or a chain
/// proving the series current (`wire-format.md` §10.1.1, §10.1.2).  It
/// enters storage and propagation when the prerequisite arrives.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pending {
    pub kind: u64,
    pub bytes: Vec<u8>,
    pub from: Keyhash,
    /// The identity whose key material is missing, where that is the want.
    pub missing_key: Option<Keyhash>,
    /// The series the receiver cannot prove current, where that is the want.
    pub unproved_series: Option<(Keyhash, u32)>,
}

/// What the store decided about an arriving object.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Decision {
    /// Stored, and to be forwarded on every adjacency but the arrival one.
    Stored,
    /// Already held, or superseded by what is held: neither stored nor
    /// forwarded (§10.1.2).
    Duplicate,
    /// The subject lies outside this node's own `h_store` (§10.1.1).
    OutOfStore,
    /// Held for a prerequisite; nothing is forwarded meanwhile (§10.1.1).
    Held(Pending),
    /// Equal `seqno`, different signed contents: the pair is malformed,
    /// neither is current, and the holder re-resolves (§10.1.2).
    Conflict { subject: Keyhash, seqno: Seqno },
    /// The bytes do not parse, or the signatures fail.
    Malformed(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HeldEndpoint {
    pub record: EndpointRecord,
}

#[derive(Default)]
pub struct TopologyStore {
    transactions: BTreeMap<Txid, Record>,
    /// One endpoint record per subject per series (`wire-format.md` §7.6:
    /// one record per patron relationship).
    endpoints: BTreeMap<(Keyhash, u32), HeldEndpoint>,
    /// `(subject, series, counter)` pairs retired by a conflict: neither
    /// content is current and nothing further is forwarded for the pair.
    conflicts: BTreeSet<(Keyhash, u32, u32)>,
    pending: Vec<Pending>,
    /// Series this node has been shown a chain for, by subject.
    proved_series: BTreeSet<(Keyhash, u32)>,
    /// Presence records, which the topology class does not carry.
    presence: BTreeMap<Txid, Vec<u8>>,
}

impl TopologyStore {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn holds_txid(&self, t: &Txid) -> bool {
        self.transactions.contains_key(t)
    }

    pub fn transaction(&self, t: &Txid) -> Option<&Record> {
        self.transactions.get(t)
    }

    pub fn transactions(&self) -> impl Iterator<Item = &Record> {
        self.transactions.values()
    }

    pub fn len(&self) -> usize {
        self.transactions.len() + self.endpoints.len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// The current endpoint record this node holds for `subject`.  Holding
    /// one line, that line; holding several, the one whose series this
    /// node has been shown a chain for — and none where it has been shown
    /// none, since records in different series do not rank and a reader
    /// MUST NOT invent an order (`wire-format.md` §2.3).
    pub fn endpoint(&self, subject: &Keyhash) -> Option<&EndpointRecord> {
        let mine: Vec<_> = self.endpoints.iter().filter(|((s, _), _)| s == subject).collect();
        match mine.len() {
            0 => None,
            1 => Some(&mine[0].1.record),
            _ => mine.iter().find(|((s, ser), _)| self.proved_series.contains(&(*s, *ser))).map(|(_, h)| &h.record),
        }
    }

    /// Every endpoint record held for `subject`, one per series.
    pub fn endpoints_of(&self, subject: &Keyhash) -> Vec<&EndpointRecord> {
        self.endpoints.iter().filter(|((s, _), _)| s == subject).map(|(_, h)| &h.record).collect()
    }

    /// The nodes this store holds an endpoint record for.
    ///
    /// **`wire-format.md` §7.6 has only infra nodes publish**, so holding
    /// a record is what tells a node that another node is infrastructure
    /// [author, 2026-09-14].  The record is the only carrier of that fact
    /// there is: a light client's endpoints arrive when it attaches and it
    /// holds no static address, so nothing else distinguishes the two from
    /// outside.  Storage already bounds this to the horizon, since
    /// `accept_endpoint` refuses a record for a node further than two
    /// edges away.
    pub fn publishers(&self) -> BTreeSet<Keyhash> {
        self.endpoints.keys().map(|(n, _)| *n).collect()
    }

    pub fn endpoint_in(&self, subject: &Keyhash, series: u32) -> Option<&EndpointRecord> {
        self.endpoints.get(&(*subject, series)).map(|h| &h.record)
    }

    pub fn prove_series(&mut self, subject: Keyhash, series: u32) {
        self.proved_series.insert((subject, series));
    }

    pub fn series_proved(&self, subject: &Keyhash, series: u32) -> bool {
        self.proved_series.contains(&(*subject, series))
    }

    pub fn conflicted(&self, subject: &Keyhash, seqno: Seqno) -> bool {
        self.conflicts.contains(&(*subject, seqno.series, seqno.counter))
    }

    pub fn pending(&self) -> &[Pending] {
        &self.pending
    }

    /// Presence records the node holds beside the topology class: evidence,
    /// not topology, and kept because an adoption's evaluation needs it
    /// (`wire-format.md` §3.4, design §10.0).
    pub fn keep_presence(&mut self, txid: Txid, bytes: Vec<u8>) {
        self.presence.insert(txid, bytes);
    }

    pub fn presence(&self, txid: &Txid) -> Option<&Vec<u8>> {
        self.presence.get(txid)
    }

    pub fn presence_records(&self) -> Vec<(Txid, Vec<u8>)> {
        self.presence.iter().map(|(t, b)| (*t, b.clone())).collect()
    }

    /// Every object the store holds, as `(kind, bytes)`: what a
    /// reconciliation replays (`wire-format.md` §10.1.3).
    pub fn objects(&self) -> Vec<(u64, Vec<u8>)> {
        let mut out: Vec<(u64, Vec<u8>)> = self.transactions.values().map(|r| (KIND_TRANSACTION, r.bytes.clone())).collect();
        out.extend(self.endpoints.values().map(|h| (KIND_ENDPOINT_RECORD, h.record.bytes.clone())));
        out
    }

    /// Decide an arriving object against this node's own view.  `in_store`
    /// answers §10.1.1's storage question for a subject; the caller owns the
    /// topology the answer comes from.
    pub fn accept<L: Lookup + ?Sized>(&mut self, kind: u64, bytes: &[u8], from: &Keyhash, ids: &L, hz: &dyn Horizon) -> Decision {
        match kind {
            KIND_TRANSACTION => self.accept_transaction(bytes, from, ids, hz),
            KIND_ENDPOINT_RECORD => self.accept_endpoint(bytes, from, ids, hz),
            _ => Decision::Malformed("unknown body kind".into()),
        }
    }

    fn accept_transaction<L: Lookup + ?Sized>(&mut self, bytes: &[u8], from: &Keyhash, ids: &L, hz: &dyn Horizon) -> Decision {
        let rec = match Record::parse(bytes) {
            Ok(r) => r,
            Err(e) => return Decision::Malformed(e),
        };
        if !is_topology_class(rec.tx_type) {
            return Decision::OutOfStore;
        }
        // the store is the seen-set, and it is consulted before anything else
        // that costs work (§10.1.2)
        if self.transactions.contains_key(&rec.txid) {
            return Decision::Duplicate;
        }
        if !stores(&rec, hz) {
            return Decision::OutOfStore;
        }
        match rec.check_signatures(ids) {
            SigStatus::Verified => {}
            SigStatus::Unverifiable { missing } => {
                let p = Pending { kind: KIND_TRANSACTION, bytes: bytes.to_vec(), from: *from, missing_key: Some(missing), unproved_series: None };
                if !self.pending.contains(&p) {
                    self.pending.push(p.clone());
                }
                return Decision::Held(p);
            }
            SigStatus::Invalid(e) => return Decision::Malformed(e),
        }
        self.transactions.insert(rec.txid, rec);
        Decision::Stored
    }

    fn accept_endpoint<L: Lookup + ?Sized>(&mut self, bytes: &[u8], from: &Keyhash, ids: &L, hz: &dyn Horizon) -> Decision {
        let er = match EndpointRecord::parse(bytes) {
            Ok(r) => r,
            Err(e) => return Decision::Malformed(e),
        };
        if !hz.within(&er.node, 2) {
            return Decision::OutOfStore;
        }
        // accepted as gossip: a receiver holding the key checks it, and one
        // that does not learns on contact (§7.6)
        if er.signature_checks(ids) == Some(false) {
            return Decision::Malformed("endpoint record signature fails".into());
        }
        if self.conflicted(&er.node, er.seqno) {
            return Decision::Duplicate;
        }
        let key = (er.node, er.seqno.series);
        if let Some(held) = self.endpoints.get(&key) {
            match compare(held.record.seqno, er.seqno) {
                Order::Older | Order::Incomparable => return Decision::Duplicate,
                Order::Same => {
                    if held.record.bytes == er.bytes {
                        return Decision::Duplicate;
                    }
                    // two signed contents at one number: the pair is
                    // malformed and neither is current (§10.1.2)
                    self.endpoints.remove(&key);
                    self.conflicts.insert((er.node, er.seqno.series, er.seqno.counter));
                    return Decision::Conflict { subject: er.node, seqno: er.seqno };
                }
                Order::Newer => {}
            }
        } else if self.endpoints.keys().any(|(s, _)| *s == er.node) && !self.series_proved(&er.node, er.seqno.series) {
            // a second line for a subject this node already holds a line
            // for: a record in a series the receiver cannot prove current
            // is neither stored nor forwarded (§10.1.2).  It is held, and
            // enters when a §4.6 chain proves the series.  A subject's
            // first line is taken as gossip, there being nothing to rank it
            // against and nothing a chain could say yet.
            let p = Pending { kind: KIND_ENDPOINT_RECORD, bytes: bytes.to_vec(), from: *from, missing_key: None, unproved_series: Some((er.node, er.seqno.series)) };
            if !self.pending.contains(&p) {
                self.pending.push(p.clone());
            }
            return Decision::Held(p);
        }
        self.endpoints.insert(key, HeldEndpoint { record: er });
        Decision::Stored
    }

    /// Objects whose prerequisite is now satisfied, removed from the pending
    /// list for the caller to re-offer.
    pub fn release_pending<L: Lookup + ?Sized>(&mut self, ids: &L) -> Vec<Pending> {
        let (ready, still): (Vec<Pending>, Vec<Pending>) = std::mem::take(&mut self.pending).into_iter().partition(|p| match (&p.missing_key, &p.unproved_series) {
            (Some(k), _) => ids.identity(k).is_some(),
            (_, Some((s, ser))) => self.proved_series.contains(&(*s, *ser)),
            _ => true,
        });
        self.pending = still;
        ready
    }

}

fn hex(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect()
}

fn unhex(s: &str) -> Option<Vec<u8>> {
    if !s.len().is_multiple_of(2) {
        return None;
    }
    (0..s.len()).step_by(2).map(|i| u8::from_str_radix(&s[i..i + 2], 16).ok()).collect()
}

impl TopologyStore {
    /// Write the store to a directory: one file per object, and the series
    /// proved and pairs retired beside them.  A node keeps its topology
    /// store across a restart because it is the seen-set: forgetting it
    /// replays a forwarding wave into every cycle in the horizon
    /// (`infra-client-requirements.md` §4.3).
    pub fn save(&self, dir: &std::path::Path) -> std::io::Result<()> {
        for sub in ["tx", "ep", "presence"] {
            std::fs::create_dir_all(dir.join(sub))?;
        }
        for (t, r) in &self.transactions {
            std::fs::write(dir.join("tx").join(hex(t)), &r.bytes)?;
        }
        // the endpoint records are rewritten whole: a record retired since
        // the last save, by a newer counter or a conflict, must not outlive
        // it on disk
        if let Ok(rd) = std::fs::read_dir(dir.join("ep")) {
            for e in rd.flatten() {
                std::fs::remove_file(e.path())?;
            }
        }
        for ((subject, series), h) in &self.endpoints {
            std::fs::write(dir.join("ep").join(format!("{}-{series}", hex(subject))), &h.record.bytes)?;
        }
        for (t, b) in &self.presence {
            std::fs::write(dir.join("presence").join(hex(t)), b)?;
        }
        let proved: Vec<String> = self.proved_series.iter().map(|(s, ser)| format!("{} {ser}", hex(s))).collect();
        std::fs::write(dir.join("proved"), proved.join("\n"))?;
        let conflicts: Vec<String> = self.conflicts.iter().map(|(s, ser, c)| format!("{} {ser} {c}", hex(s))).collect();
        std::fs::write(dir.join("conflicts"), conflicts.join("\n"))?;
        Ok(())
    }

    /// Read a store back.  Objects that no longer parse are skipped rather
    /// than failing the load; the pending list is not kept, since what it
    /// held was waiting on a prerequisite the restart may have lost too.
    pub fn load(dir: &std::path::Path) -> std::io::Result<TopologyStore> {
        let mut st = TopologyStore::new();
        if let Ok(rd) = std::fs::read_dir(dir.join("tx")) {
            for e in rd.flatten() {
                if let Ok(rec) = Record::parse(&std::fs::read(e.path())?) {
                    st.transactions.insert(rec.txid, rec);
                }
            }
        }
        if let Ok(rd) = std::fs::read_dir(dir.join("ep")) {
            for e in rd.flatten() {
                if let Ok(er) = EndpointRecord::parse(&std::fs::read(e.path())?) {
                    st.endpoints.insert((er.node, er.seqno.series), HeldEndpoint { record: er });
                }
            }
        }
        if let Ok(rd) = std::fs::read_dir(dir.join("presence")) {
            for e in rd.flatten() {
                let name = e.file_name().to_string_lossy().to_string();
                if let Some(t) = unhex(&name).and_then(|v| <[u8; 32]>::try_from(v).ok()) {
                    st.presence.insert(t, std::fs::read(e.path())?);
                }
            }
        }
        if let Ok(text) = std::fs::read_to_string(dir.join("proved")) {
            for line in text.lines() {
                let mut it = line.split(' ');
                if let (Some(s), Some(ser)) = (it.next().and_then(unhex).and_then(|v| <[u8; 32]>::try_from(v).ok()), it.next().and_then(|x| x.parse().ok())) {
                    st.proved_series.insert((s, ser));
                }
            }
        }
        if let Ok(text) = std::fs::read_to_string(dir.join("conflicts")) {
            for line in text.lines() {
                let mut it = line.split(' ');
                let s = it.next().and_then(unhex).and_then(|v| <[u8; 32]>::try_from(v).ok());
                let ser = it.next().and_then(|x| x.parse().ok());
                let c = it.next().and_then(|x| x.parse().ok());
                if let (Some(s), Some(ser), Some(c)) = (s, ser, c) {
                    st.conflicts.insert((s, ser, c));
                }
            }
        }
        // a record whose number the conflict markers retire is not current,
        // whatever the directory held (`wire-format.md` §10.1.2)
        let conflicts = st.conflicts.clone();
        st.endpoints.retain(|_, h| !conflicts.contains(&(h.record.node, h.record.seqno.series, h.record.seqno.counter)));
        Ok(st)
    }
}

/// The store answers a txid with the transaction or presence record it
/// holds, which is what an adoption's evaluation dereferences.
impl rhtn_archive::walk::Fetch for TopologyStore {
    fn fetch(&self, txid: &Txid) -> Option<Vec<u8>> {
        self.transactions.get(txid).map(|r| r.bytes.clone()).or_else(|| self.presence.get(txid).cloned())
    }
}
