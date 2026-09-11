//! One key's archive (design §10): the records it holds, its branch heads,
//! the back-pointers its next transaction carries, serving it in batches
//! (`wire-format.md` §7.9), and pruning at a checkpoint (design §10.1) —
//! which releases the chain and never the evidence (design §10.0).

use crate::record::Record;
use crate::tx::{Seqno, TYPE_ADOPTION, TYPE_REISSUE};
use crate::walk::Fetch;
use crate::{Keyhash, Txid, WINDOW_SECONDS, genesis};
use rhtn_codec::cbor::*;
use rhtn_codec::encode::*;
use rhtn_codec::schema::{self, Family};
use std::collections::{BTreeMap, BTreeSet, VecDeque};

/// An `ArchiveRequest` (`wire-format.md` §7.9).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArchiveRequest {
    pub subject: Keyhash,
    /// Absent: the holder's newest record — the recovery case.
    pub head: Option<Txid>,
    pub max_records: u64,
    pub stop_before: Option<u64>,
    pub nonce: [u8; 16],
}

impl ArchiveRequest {
    pub fn encode(&self) -> Vec<u8> {
        let mut out = Vec::new();
        emit_map_head(&mut out, 3 + self.head.is_some() as usize + self.stop_before.is_some() as usize);
        emit_uint(&mut out, 1);
        emit_bstr(&mut out, &self.subject);
        if let Some(h) = &self.head {
            emit_uint(&mut out, 2);
            emit_bstr(&mut out, h);
        }
        emit_uint(&mut out, 3);
        emit_uint(&mut out, self.max_records);
        if let Some(t) = self.stop_before {
            emit_uint(&mut out, 4);
            emit_uint(&mut out, t);
        }
        emit_uint(&mut out, 5);
        emit_bstr(&mut out, &self.nonce);
        out
    }

    pub fn decode(b: &[u8]) -> Result<Self, String> {
        parse_all(b).map_err(|e| e.0)?;
        schema::check_unsigned(Family::ArchiveRequest, b, 0).map_err(|e| e.0)?;
        let item = parse_all(b).map_err(|e| e.0)?;
        let Item::Map(m) = &item else { return Err("map".into()) };
        let bytes = |k: u64| match map_get(m, k) { Some(Item::Bytes(r)) => Some(b[r.clone()].to_vec()), _ => None };
        Ok(ArchiveRequest {
            subject: bytes(1).and_then(|v| v.try_into().ok()).ok_or("subject")?,
            head: bytes(2).and_then(|v| v.try_into().ok()),
            max_records: map_get(m, 3).and_then(as_uint).ok_or("max")?,
            stop_before: map_get(m, 4).and_then(as_uint),
            nonce: bytes(5).and_then(|v| v.try_into().ok()).ok_or("nonce")?,
        })
    }
}

/// An `ArchiveReply` (`wire-format.md` §7.9): records head first.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArchiveReply {
    pub nonce: [u8; 16],
    pub records: Vec<Vec<u8>>,
    pub more: bool,
    /// The oldest record returned, to continue from.
    pub continue_from: Option<Txid>,
}

impl ArchiveReply {
    pub fn encode(&self) -> Vec<u8> {
        let mut out = Vec::new();
        emit_map_head(&mut out, 3 + self.continue_from.is_some() as usize);
        emit_uint(&mut out, 1);
        emit_bstr(&mut out, &self.nonce);
        emit_uint(&mut out, 2);
        emit_array_head(&mut out, self.records.len());
        for r in &self.records {
            out.extend_from_slice(r);
        }
        emit_uint(&mut out, 3);
        emit_bool(&mut out, self.more);
        if let Some(c) = &self.continue_from {
            emit_uint(&mut out, 4);
            emit_bstr(&mut out, c);
        }
        out
    }

    pub fn decode(b: &[u8]) -> Result<Self, String> {
        parse_all(b).map_err(|e| e.0)?;
        schema::check_unsigned(Family::ArchiveReply, b, 0).map_err(|e| e.0)?;
        let item = parse_all(b).map_err(|e| e.0)?;
        let Item::Map(m) = &item else { return Err("map".into()) };
        let nonce = match map_get(m, 1) { Some(Item::Bytes(r)) => b[r.clone()].to_vec().try_into().map_err(|_| "nonce")?, _ => return Err("nonce".into()) };
        let r2 = value_slice(b, 2).ok_or("records")?;
        let records = array_item_ranges(b, r2.start).ok_or("records walk")?.into_iter().map(|r| b[r].to_vec()).collect();
        let more = matches!(map_get(m, 3), Some(Item::Bool(true)));
        let continue_from = match map_get(m, 4) { Some(Item::Bytes(r)) => b[r.clone()].to_vec().try_into().ok(), _ => None };
        Ok(ArchiveReply { nonce, records, more, continue_from })
    }
}

/// Presence evidence the chain does not govern (design §10.0): the record,
/// its sealed capture and the capture seed, kept by txid across pruning.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Evidence {
    pub record: Vec<u8>,
    pub sealed_capture: Vec<u8>,
    pub seed: Vec<u8>,
}

/// One key's archive: a Merkle DAG of the records it signed.
#[derive(Debug, Clone)]
pub struct Archive {
    pub key: Keyhash,
    records: BTreeMap<Txid, Record>,
    heads: BTreeSet<Txid>,
    /// The checkpoint the chain was pruned at, if any: a series reissue
    /// whose predecessors are no longer held (design §10.1).
    checkpoint: Option<Txid>,
    evidence: BTreeMap<Txid, Evidence>,
}

impl Archive {
    pub fn new(key: Keyhash) -> Self {
        Archive { key, records: BTreeMap::new(), heads: BTreeSet::new(), checkpoint: None, evidence: BTreeMap::new() }
    }

    /// The back-pointers this key's next transaction carries: every branch
    /// head, or the genesis value for a key with no transaction.  Longer
    /// than one means the next transaction is a merge (design §10.3).
    pub fn next_back_pointers(&self) -> Vec<Txid> {
        if self.heads.is_empty() { vec![genesis(&self.key)] } else { self.heads.iter().copied().collect() }
    }

    pub fn heads(&self) -> Vec<Txid> {
        self.heads.iter().copied().collect()
    }

    pub fn len(&self) -> usize {
        self.records.len()
    }

    pub fn is_empty(&self) -> bool {
        self.records.is_empty()
    }

    pub fn get(&self, txid: &Txid) -> Option<&Record> {
        self.records.get(txid)
    }

    pub fn checkpoint(&self) -> Option<Txid> {
        self.checkpoint
    }

    /// Take a record this key signed into the archive.  Its back-pointers for
    /// this key must each be held, be the genesis value, or be the
    /// checkpoint's own predecessors; the heads they name stop being heads.
    pub fn append(&mut self, rec: Record) -> Result<Txid, String> {
        // a record already held is held once: appending it again changes
        // nothing, and never makes a predecessor a head beside its
        // successor (design §10.3: one DAG, not a fork)
        if self.records.contains_key(&rec.txid) {
            return Ok(rec.txid);
        }
        let ptrs = rec.back_pointers_of(&self.key).ok_or("not signed by this key")?.to_vec();
        let g = genesis(&self.key);
        for p in &ptrs {
            if *p != g && !self.records.contains_key(p) {
                return Err("back-pointer names a record not held".into());
            }
            if *p == g && !self.records.is_empty() && ptrs.len() == 1 {
                // a second genesis-rooted record is a fork at genesis; held, as any branch
            }
        }
        // §3.3: the record's own time is at or after every predecessor's effective time
        for p in &ptrs {
            if let Some(prev) = self.records.get(p)
                && rec.time < prev.effective {
                    return Err("timestamp before predecessor's effective time".into());
                }
        }
        let txid = rec.txid;
        for p in &ptrs {
            self.heads.remove(p);
        }
        self.heads.insert(txid);
        self.records.insert(txid, rec);
        Ok(txid)
    }

    /// Whether the archive has forked: more than one head.
    pub fn is_forked(&self) -> bool {
        self.heads.len() > 1
    }

    /// The seqno this key last used in `series`, from its adoptions,
    /// departures and reissues.
    pub fn last_seqno(&self, series: u32) -> Option<Seqno> {
        self.records.values().filter_map(|r| r.seqno()).filter(|s| s.series == series).max_by_key(|s| s.counter)
    }

    /// Every record held, in no particular order.
    pub fn records(&self) -> impl Iterator<Item = &Record> {
        self.records.values()
    }

    /// Every series this key has occupied (`light-client-requirements.md`
    /// §2: never reissue into one of them).
    pub fn series_occupied(&self) -> BTreeSet<u32> {
        self.records.values().filter_map(|r| r.seqno()).map(|s| s.series).collect()
    }

    /// This key's chain under `patron` (`wire-format.md` §4.6.1): its
    /// latest adoption there and every reissue since, which is what it
    /// presents when asked which series it is in.
    pub fn chain_for(&self, patron: &Keyhash) -> Option<crate::series::SeriesChain> {
        let adoption = self.records.values().filter(|r| r.tx_type == TYPE_ADOPTION && r.field_hash(1) == Some(self.key) && r.field_hash(2) == Some(*patron)).max_by_key(|r| (r.time, r.txid))?;
        let mut chain = crate::series::SeriesChain::open(adoption).ok()?;
        let mut reissues: Vec<&Record> = self.records.values().filter(|r| r.tx_type == TYPE_REISSUE && r.field_hash(1) == Some(self.key) && r.field_hash(2) == Some(*patron) && r.time >= adoption.time).collect();
        reissues.sort_by_key(|r| (r.time, r.txid));
        for r in reissues {
            // a reissue that does not extend the chain is not a link of it,
            // whatever else this key signed
            let _ = chain.extend(r);
        }
        Some(chain)
    }

    /// Records reachable backward from `head`, head first, every record
    /// before any of its predecessors (a reverse topological order).
    fn walk_from(&self, head: &Txid) -> Vec<Txid> {
        let mut reach = BTreeSet::new();
        let mut stack = vec![*head];
        while let Some(t) = stack.pop() {
            if !reach.insert(t) {
                continue;
            }
            if let Some(r) = self.records.get(&t) {
                for p in r.back_pointers_of(&self.key).unwrap_or(&[]) {
                    if self.records.contains_key(p) {
                        stack.push(*p);
                    }
                }
            }
        }
        let mut indeg: BTreeMap<Txid, usize> = reach.iter().map(|t| (*t, 0)).collect();
        for t in &reach {
            for p in self.records[t].back_pointers_of(&self.key).unwrap_or(&[]) {
                if let Some(d) = indeg.get_mut(p) {
                    *d += 1;
                }
            }
        }
        let mut ready: VecDeque<Txid> = VecDeque::new();
        ready.push_back(*head);
        let mut out = Vec::new();
        while let Some(t) = ready.pop_front() {
            out.push(t);
            let mut preds: Vec<Txid> = self.records[&t].back_pointers_of(&self.key).unwrap_or(&[]).iter().filter(|p| reach.contains(*p)).copied().collect();
            preds.sort_by_key(|p| std::cmp::Reverse(self.records[p].effective));
            for p in preds {
                let d = indeg.get_mut(&p).unwrap();
                *d -= 1;
                if *d == 0 {
                    ready.push_back(p);
                }
            }
        }
        out
    }

    /// The record this archive serves as newest when a request names no
    /// head: the head with the latest effective time.
    pub fn newest(&self) -> Option<Txid> {
        self.heads.iter().max_by_key(|h| self.records[*h].effective).copied()
    }

    /// Answer an archive request (`wire-format.md` §7.9): head first, at
    /// most `max_records`, `continue_from` the oldest returned when more
    /// remain.  A head this archive does not hold gets an empty batch.
    pub fn serve(&self, req: &ArchiveRequest) -> ArchiveReply {
        let empty = ArchiveReply { nonce: req.nonce, records: Vec::new(), more: false, continue_from: None };
        if req.subject != self.key || !(1..=256).contains(&req.max_records) {
            return empty;
        }
        let Some(head) = req.head.or_else(|| self.newest()) else { return empty };
        if !self.records.contains_key(&head) {
            return empty;
        }
        let order = self.walk_from(&head);
        let mut records = Vec::new();
        let mut last = None;
        let mut stopped = false;
        for t in &order {
            let r = &self.records[t];
            if req.stop_before.is_some_and(|s| r.effective < s) {
                stopped = true;
                break;
            }
            if records.len() as u64 == req.max_records {
                break;
            }
            records.push(r.bytes.clone());
            last = Some(*t);
        }
        let more = !stopped && records.len() < order.len();
        ArchiveReply { nonce: req.nonce, records, more, continue_from: if more { last } else { None } }
    }

    // ---------------------------------------------------------------- evidence

    pub fn keep_evidence(&mut self, txid: Txid, ev: Evidence) {
        self.evidence.insert(txid, ev);
    }

    pub fn evidence(&self, txid: &Txid) -> Option<&Evidence> {
        self.evidence.get(txid)
    }

    /// Prune the chain at a checkpoint (design §10.1): `at` must be a
    /// series reissue, dated beyond the 730-day window as of `now`; every
    /// record before it is released.  The evidence store is untouched.
    pub fn prune(&mut self, at: &Txid, now: u64) -> Result<usize, String> {
        let cp = self.records.get(at).ok_or("checkpoint not held")?;
        if !cp.is_checkpoint() {
            return Err("not a series reissue".into());
        }
        if cp.effective.saturating_add(WINDOW_SECONDS) > now {
            return Err("inside the 730-day window".into());
        }
        let before: Vec<Txid> = self.walk_from(at).into_iter().skip(1).collect();
        let n = before.len();
        for t in &before {
            self.records.remove(t);
            self.heads.remove(t);
        }
        self.checkpoint = Some(*at);
        Ok(n)
    }
}

impl Fetch for Archive {
    fn fetch(&self, txid: &Txid) -> Option<Vec<u8>> {
        self.records.get(txid).map(|r| r.bytes.clone())
    }
}
