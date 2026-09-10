//! Backward verification of a presented or fetched history (design §10.1,
//! `wire-format.md` §3.1, §3.4, §7.9).  A walk follows the subject's
//! back-pointers from a head, fetching as it goes, and reports where the
//! range roots; a batch check does the same over one `ArchiveReply`.

use crate::chain::ArchiveReply;
use crate::record::{Record, SigStatus};
use crate::{Keyhash, Txid, genesis};
use rhtn_crypto::verify::Lookup;
use std::collections::{BTreeMap, BTreeSet, VecDeque};

/// Whoever serves records by txid: the subject, a holder, or a test.
pub trait Fetch {
    fn fetch(&self, txid: &Txid) -> Option<Vec<u8>>;
}

impl Fetch for BTreeMap<Txid, Vec<u8>> {
    fn fetch(&self, txid: &Txid) -> Option<Vec<u8>> {
        self.get(txid).cloned()
    }
}

/// Where a walked range ends (design §10.1: rooted at genesis or at a
/// checkpoint; `wire-format.md` §3.4: an unfetchable predecessor is
/// incomplete, not malformed; §3.1: a chain that does not reach back).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Root {
    Genesis,
    /// A series reissue whose predecessors were not served: the range is
    /// unbroken from the head to it, and `beyond` lies past the checkpoint.
    Checkpoint { at: Txid, beyond: Vec<Txid> },
    /// `at` names `missing`, which could not be fetched.
    Unfetched { at: Txid, missing: Txid },
    /// `at` names `named` as the subject's predecessor, and the subject did
    /// not sign it.
    DoesNotReachBack { at: Txid, named: Txid },
    Malformed { at: Txid, why: String },
}

impl Root {
    fn rank(&self) -> u8 {
        match self {
            Root::Malformed { .. } => 4,
            Root::DoesNotReachBack { .. } => 3,
            Root::Unfetched { .. } => 2,
            Root::Checkpoint { .. } => 1,
            Root::Genesis => 0,
        }
    }
}

#[derive(Debug, Clone)]
pub struct Walk {
    pub subject: Keyhash,
    pub head: Txid,
    /// Records whose structure verified, head first in visit order.
    pub records: Vec<Record>,
    pub signatures: Vec<(Txid, SigStatus)>,
    pub root: Root,
}

impl Walk {
    pub fn txids(&self) -> Vec<Txid> {
        self.records.iter().map(|r| r.txid).collect()
    }
    /// Unbroken from the head to genesis or to a checkpoint.
    pub fn is_unbroken(&self) -> bool {
        matches!(self.root, Root::Genesis | Root::Checkpoint { .. })
    }
}

/// Walk `subject`'s chain backward from `head`, fetching from `holder` and
/// checking signatures against `ids`, visiting at most `max_records`.  The
/// patron chooses its own depth (design §16.7).
pub fn walk<F: Fetch + ?Sized, L: Lookup + ?Sized>(subject: &Keyhash, head: &Txid, holder: &F, ids: &L, max_records: usize) -> Walk {
    let g = genesis(subject);
    let mut pending: VecDeque<(Txid, Option<Txid>)> = VecDeque::from([(*head, None)]);
    let mut seen: BTreeSet<Txid> = BTreeSet::new();
    let mut times: BTreeMap<Txid, (u64, bool)> = BTreeMap::new(); // txid -> (own time, is checkpoint)
    let mut records = Vec::new();
    let mut signatures = Vec::new();
    let mut ends: Vec<Root> = Vec::new();
    while let Some((t, from)) = pending.pop_front() {
        if !seen.insert(t) {
            continue;
        }
        if records.len() >= max_records {
            ends.push(Root::Unfetched { at: from.unwrap_or(t), missing: t });
            continue;
        }
        let Some(bytes) = holder.fetch(&t) else {
            match from.and_then(|f| times.get(&f).map(|x| (f, x.1))) {
                Some((f, true)) => ends.push(Root::Checkpoint { at: f, beyond: vec![t] }),
                _ => ends.push(Root::Unfetched { at: from.unwrap_or(t), missing: t }),
            }
            continue;
        };
        let rec = match Record::parse(&bytes) {
            Ok(r) => r,
            Err(why) => {
                ends.push(Root::Malformed { at: t, why });
                continue;
            }
        };
        if rec.txid != t {
            ends.push(Root::Malformed { at: t, why: "served record's txid differs from the one named".into() });
            continue;
        }
        let Some(ptrs) = rec.back_pointers_of(subject).map(|p| p.to_vec()) else {
            ends.push(Root::DoesNotReachBack { at: from.unwrap_or(t), named: t });
            continue;
        };
        if let Some(f) = from
            && let Some((ft, _)) = times.get(&f)
                && rec.effective > *ft {
                    ends.push(Root::Malformed { at: f, why: "time before predecessor's effective time".into() });
                    continue;
                }
        times.insert(t, (rec.time, rec.is_checkpoint()));
        signatures.push((t, rec.check_signatures(ids)));
        for p in ptrs {
            if p == g {
                ends.push(Root::Genesis);
            } else {
                pending.push_back((p, Some(t)));
            }
        }
        records.push(rec);
    }
    // the worst end decides; checkpoints at one reissue merge their beyonds
    let mut root = Root::Genesis;
    let mut beyond_by_cp: BTreeMap<Txid, Vec<Txid>> = BTreeMap::new();
    for e in &ends {
        if let Root::Checkpoint { at, beyond } = e {
            beyond_by_cp.entry(*at).or_default().extend(beyond.iter().copied());
        }
        if e.rank() > root.rank() {
            root = e.clone();
        }
    }
    if let Root::Checkpoint { at, .. } = &root {
        let mut b = beyond_by_cp.remove(at).unwrap_or_default();
        b.sort();
        b.dedup();
        root = Root::Checkpoint { at: *at, beyond: b };
    }
    Walk { subject: *subject, head: *head, records, signatures, root }
}

/// How one fetched batch ends.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BatchEnd {
    Genesis,
    Checkpoint(Txid),
    /// Predecessors named but not returned; the next request names
    /// `continue_from`, the oldest record returned.
    Unfetched { continue_from: Txid, missing: Vec<Txid> },
    /// Record `index` is not what the chain says follows.
    Mismatch { index: usize, why: String },
}

#[derive(Debug, Clone)]
pub struct BatchVerdict {
    /// Records verified in order, up to the first mismatch.
    pub verified: Vec<Record>,
    pub signatures: Vec<(Txid, SigStatus)>,
    pub end: BatchEnd,
    /// False when no head was requested: the first record's newestness is
    /// the holder's claim (`wire-format.md` §7.9).
    pub head_verified: bool,
}

impl BatchVerdict {
    pub fn verified_txids(&self) -> Vec<Txid> {
        self.verified.iter().map(|r| r.txid).collect()
    }
}

/// Verify one reply (`wire-format.md` §7.9): the first record matches the
/// requested head, and every later record is one an earlier record's
/// back-pointers name.  Reachability, not sequence, is what a merge needs
/// (`wire-format.md` §3.1).
pub fn verify_batch<L: Lookup + ?Sized>(subject: &Keyhash, requested_head: Option<&Txid>, reply: &ArchiveReply, ids: &L) -> BatchVerdict {
    let g = genesis(subject);
    let mut verified: Vec<Record> = Vec::new();
    let mut signatures = Vec::new();
    let mut expected: BTreeSet<Txid> = BTreeSet::new();
    let mut named_by: BTreeMap<Txid, Vec<usize>> = BTreeMap::new();
    let mut checkpoint_names: BTreeMap<Txid, Txid> = BTreeMap::new();
    let mut end = None;
    for (i, bytes) in reply.records.iter().enumerate() {
        let rec = match Record::parse(bytes) {
            Ok(r) => r,
            Err(why) => {
                end = Some(BatchEnd::Mismatch { index: i, why: format!("malformed: {why}") });
                break;
            }
        };
        let Some(ptrs) = rec.back_pointers_of(subject).map(|p| p.to_vec()) else {
            end = Some(BatchEnd::Mismatch { index: i, why: "not signed by the subject".into() });
            break;
        };
        if i == 0 {
            if let Some(h) = requested_head
                && rec.txid != *h {
                    end = Some(BatchEnd::Mismatch { index: 0, why: "first record is not the requested head".into() });
                    break;
                }
        } else if !expected.contains(&rec.txid) {
            end = Some(BatchEnd::Mismatch { index: i, why: "not named by any preceding record's back-pointers".into() });
            break;
        }
        if let Some(js) = named_by.get(&rec.txid)
            && js.iter().any(|j| rec.effective > verified[*j].time) {
                end = Some(BatchEnd::Mismatch { index: i, why: "effective time after the record that names it".into() });
                break;
            }
        expected.remove(&rec.txid);
        for p in &ptrs {
            if *p != g {
                expected.insert(*p);
                named_by.entry(*p).or_default().push(i);
                if rec.is_checkpoint() {
                    checkpoint_names.insert(*p, rec.txid);
                }
            }
        }
        signatures.push((rec.txid, rec.check_signatures(ids)));
        verified.push(rec);
    }
    let end = end.unwrap_or_else(|| {
        if expected.is_empty() {
            BatchEnd::Genesis
        } else if let Some(cp) = expected.iter().filter_map(|m| checkpoint_names.get(m)).next().filter(|_| expected.iter().all(|m| checkpoint_names.contains_key(m))) {
            BatchEnd::Checkpoint(*cp)
        } else {
            BatchEnd::Unfetched { continue_from: verified.last().map(|r| r.txid).unwrap_or(g), missing: expected.iter().copied().collect() }
        }
    });
    BatchVerdict { verified, signatures, end, head_verified: requested_head.is_some() }
}

/// The result of fetching a whole chain batch by batch: what a requester
/// records about the restored or presented history.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FetchOutcome {
    pub records: Vec<Txid>,
    pub end: BatchEnd,
    /// `true` only when a requested head anchored the first batch: a chain
    /// restored from the holder's claimed newest record is internally
    /// verified and never verified-complete (`light-client-requirements.md` §2).
    pub verified_complete: bool,
    /// The newest record, and whether it is the holder's claim.
    pub newest: Option<Txid>,
    pub newest_is_holders_claim: bool,
}

/// Fetch a chain through `serve` from `head` (or the holder's newest), one
/// batch of `max_records` at a time, verifying each and continuing from
/// the oldest record returned until the holder reports no more.
pub fn fetch_chain<L: Lookup + ?Sized>(subject: &Keyhash, head: Option<Txid>, max_records: u64, ids: &L, mut serve: impl FnMut(&crate::chain::ArchiveRequest) -> ArchiveReply) -> FetchOutcome {
    let mut records: Vec<Txid> = Vec::new();
    let mut next_head = head;
    let mut newest = None;
    let mut end;
    let mut first = true;
    loop {
        let nonce = rhtn_codec::cose::sha256(&records.len().to_be_bytes())[..16].try_into().unwrap();
        let req = crate::chain::ArchiveRequest { subject: *subject, head: next_head, max_records, stop_before: None, nonce };
        let reply = serve(&req);
        if reply.nonce != nonce {
            end = BatchEnd::Mismatch { index: 0, why: "nonce not echoed".into() };
            break;
        }
        let v = verify_batch(subject, next_head.as_ref(), &reply, ids);
        if first {
            newest = v.verified.first().map(|r| r.txid);
            first = false;
        }
        for r in &v.verified {
            if !records.contains(&r.txid) {
                records.push(r.txid);
            }
        }
        end = v.end.clone();
        match &v.end {
            BatchEnd::Unfetched { continue_from, .. } if reply.more || reply.continue_from.is_some() => {
                let cont = reply.continue_from.unwrap_or(*continue_from);
                if Some(cont) == next_head {
                    break; // no progress: the holder repeats itself
                }
                next_head = Some(cont);
            }
            _ => break,
        }
    }
    let unbroken = matches!(end, BatchEnd::Genesis | BatchEnd::Checkpoint(_));
    FetchOutcome { records, end, verified_complete: unbroken && head.is_some(), newest, newest_is_holders_claim: head.is_none() }
}
