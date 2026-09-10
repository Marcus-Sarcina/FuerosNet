//! The forwarding rule and the rootward memo (`wire-format.md` §10.1, §10.2;
//! design §15).
//!
//! Forward if and only if you stored it, to every adjacency except the one
//! it arrived from; store when the subject falls in your own `h_store`.
//! Nothing in a frame tells a node how far to forward, and nothing here
//! reads one.

use crate::store::{Decision, Horizon, KIND_ENDPOINT_RECORD, KIND_TRANSACTION};
use crate::view::{NodeView, Slot};
use crate::{Adjacency, Keyhash};
use rhtn_archive::record::Record;
use rhtn_archive::tx::{self, Locator};
use rhtn_codec::cbor::*;
use rhtn_codec::encode::*;
use rhtn_crypto::verify::Lookup;
use std::collections::BTreeSet;

/// The horizon as it stood when an object arrived, so the borrow of the
/// table ends before the store is touched.  The storage rule asks at h = 2
/// and at h - 1, so both balls are taken.
struct HorizonSnapshot {
    me: Keyhash,
    balls: Vec<BTreeSet<Keyhash>>,
    attached: BTreeSet<Keyhash>,
}

impl Horizon for HorizonSnapshot {
    fn within(&self, x: &Keyhash, h: usize) -> bool {
        if x == &self.me || self.attached.contains(x) {
            return true;
        }
        self.balls.get(h).is_some_and(|b| b.contains(x))
    }
}

pub const FRAME_TOPOLOGY_PUSH: u64 = 5;
pub const FRAME_TOPOLOGY_MEMO: u64 = 6;

/// `TopologyPush` (`wire-format.md` §10.1): the body-kind tag and the object
/// byte-for-byte.  That is the only wrapper field permitted.
pub fn encode_push(kind: u64, object: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    emit_map_head(&mut out, 2);
    emit_uint(&mut out, 1);
    emit_uint(&mut out, kind);
    emit_uint(&mut out, 2);
    emit_bstr(&mut out, object);
    out
}

pub fn decode_push(body: &[u8]) -> Result<(u64, Vec<u8>), String> {
    let item = parse_all(body).map_err(|e| e.0)?;
    let Item::Map(m) = &item else { return Err("push not a map".into()) };
    if m.len() != 2 {
        return Err("push carries a field beyond the body-kind tag".into());
    }
    let kind = map_get(m, 1).and_then(as_uint).ok_or("field 1")?;
    let object = match map_get(m, 2) {
        Some(Item::Bytes(r)) => body[r.clone()].to_vec(),
        _ => return Err("field 2".into()),
    };
    Ok((kind, object))
}

/// `TopologyMemo` (`wire-format.md` §10.2).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Memo {
    /// The patron: the only party this object speaks for.
    pub patron: Keyhash,
    /// The patron's own position.
    pub position: Locator,
    pub slot: u64,
    /// The underlying transaction's own timestamp, copied.
    pub timestamp: u64,
    /// Absent means the slot is empty: a departure or a disavowal.
    pub occupant: Option<Keyhash>,
}

impl Memo {
    pub fn encode(&self) -> Vec<u8> {
        let mut out = Vec::new();
        emit_map_head(&mut out, 4 + self.occupant.is_some() as usize);
        emit_uint(&mut out, 1);
        emit_bstr(&mut out, &self.patron);
        emit_uint(&mut out, 2);
        self.position.emit(&mut out);
        emit_uint(&mut out, 3);
        emit_uint(&mut out, self.slot);
        emit_uint(&mut out, 4);
        emit_uint(&mut out, self.timestamp);
        if let Some(o) = &self.occupant {
            emit_uint(&mut out, 5);
            emit_bstr(&mut out, o);
        }
        out
    }

    pub fn decode(b: &[u8]) -> Result<Self, String> {
        parse_all(b).map_err(|e| e.0)?;
        rhtn_codec::schema::check_unsigned(rhtn_codec::schema::Family::TopologyMemo, b, 0).map_err(|e| e.0)?;
        let item = parse_all(b).map_err(|e| e.0)?;
        let Item::Map(m) = &item else { return Err("memo not a map".into()) };
        let kh = |k: u64| match map_get(m, k) {
            Some(Item::Bytes(r)) if r.len() == 32 => <[u8; 32]>::try_from(&b[r.clone()]).ok(),
            _ => None,
        };
        let r2 = value_slice(b, 2).ok_or("field 2")?;
        let position = Locator::decode(&b[r2]).map_err(|e| format!("locator: {e}"))?;
        Ok(Memo {
            patron: kh(1).ok_or("field 1")?,
            position,
            slot: map_get(m, 3).and_then(as_uint).ok_or("field 3")?,
            timestamp: map_get(m, 4).and_then(as_uint).ok_or("field 4")?,
            occupant: kh(5),
        })
    }
}

/// What a node did with an arriving memo.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MemoOutcome {
    /// Not this subnet's: neither applied to a table nor passed on (§10.2).
    ForeignSubnet,
    /// Applied where a table is kept, and forwarded rootward.
    Forwarded { to: Keyhash },
    /// Applied where a table is kept; this node is the root.
    StoppedAtRoot,
    /// This node holds that slot at or after the memo's timestamp.
    AlreadyPassed,
    /// Field 1 is this node and its own row confirms the memo: a cycle.
    CycleConfirmed { disavowed: Keyhash },
    /// Field 1 is this node and its own row does not confirm the memo.
    Unconfirmed,
    /// Field 1 is an attached client; the hit is handed to that client at
    /// contact, and the records that answer it are theirs (§10.2).
    ForAttachedClient { client: Keyhash },
    Malformed(String),
}

impl NodeView {
    /// Originate a push for an object this node is a party to, and forward
    /// it to every adjacency (`wire-format.md` §10.1).
    pub fn originate_push(&self, adj: &dyn Adjacency, kind: u64, object: &[u8]) {
        let body = encode_push(kind, object);
        for p in self.adjacent(adj, None) {
            adj.send(&p, FRAME_TOPOLOGY_PUSH, &body);
        }
    }

    /// The sessions this node holds by virtue of a topology relationship,
    /// minus the one an object arrived from (`wire-format.md` §10.1.1).
    pub fn adjacent(&self, adj: &dyn Adjacency, except: Option<&Keyhash>) -> Vec<Keyhash> {
        let me = self.me();
        let mut want: BTreeSet<Keyhash> = BTreeSet::new();
        want.extend(self.table.patrons(&me));
        want.extend(self.table.subordinates(&me));
        want.extend(self.peers.iter().copied());
        want.extend(self.attached.iter().copied());
        want.extend(self.serving_node);
        if let Some(e) = except {
            want.remove(e);
        }
        want.remove(&me);
        // adjacency is the sessions the node has anyway
        adj.peers().into_iter().filter(|p| want.contains(p)).collect()
    }

    /// Receive a `TopologyPush` on the session from `from`, decide against
    /// this node's own view, and forward what it stored.
    pub fn receive_push<L: Lookup + ?Sized>(&mut self, adj: &dyn Adjacency, from: &Keyhash, body: &[u8], ids: &L) -> Decision {
        let (kind, object) = match decode_push(body) {
            Ok(v) => v,
            Err(e) => return Decision::Malformed(e),
        };
        self.take_object(adj, from, kind, &object, ids)
    }

    /// The same decision for an object already unwrapped.
    pub fn take_object<L: Lookup + ?Sized>(&mut self, adj: &dyn Adjacency, from: &Keyhash, kind: u64, object: &[u8], ids: &L) -> Decision {
        // the storage question is answered against this node's own view,
        // taken now rather than when the object was sent (§10.1.1)
        let me = self.me();
        let hz = HorizonSnapshot {
            me,
            balls: (0..=2).map(|h| self.table.horizon(&me, h)).collect(),
            attached: self.attached.clone(),
        };
        let decision = self.store.accept(kind, object, from, ids, &hz);
        match &decision {
            Decision::Stored => {
                // forward byte-for-byte, on every adjacency but the arrival one
                let push = encode_push(kind, object);
                for p in self.adjacent(adj, Some(from)) {
                    adj.send(&p, FRAME_TOPOLOGY_PUSH, &push);
                }
                if kind == KIND_TRANSACTION {
                    self.apply_stored(object, ids);
                }
            }
            Decision::Conflict { subject, .. } => {
                // repair by re-resolving (§10.1.2, §7.7)
                self.re_resolve(adj, subject);
            }
            _ => {}
        }
        decision
    }

    /// Fold a stored transaction into this node's own table, where it can.
    fn apply_stored<L: Lookup + ?Sized>(&mut self, object: &[u8], ids: &L) {
        let Ok(rec) = Record::parse(object) else { return };
        let store_objects: Vec<Vec<u8>> = self.store.objects().into_iter().filter(|(k, _)| *k == KIND_TRANSACTION).map(|(_, b)| b).collect();
        let mut presence = std::collections::BTreeMap::new();
        for b in store_objects {
            if let Ok(r) = Record::parse(&b) {
                presence.insert(r.txid, b);
            }
        }
        let _ = self.table.apply(&rec, ids, &presence, None);
    }

    /// Re-offer everything whose prerequisite has since arrived
    /// (`wire-format.md` §10.1.1's *hold it, fetch the key*).
    pub fn release_pending<L: Lookup + ?Sized>(&mut self, adj: &dyn Adjacency, ids: &L) -> Vec<Decision> {
        let ready = self.store.release_pending(ids);
        ready.iter().map(|p| self.take_object(adj, &p.from.clone(), p.kind, &p.bytes.clone(), ids)).collect()
    }

    /// Replay this node's store to `to` as `TopologyPush` frames: that is
    /// the whole of reconciliation (`wire-format.md` §10.1.3).
    pub fn replay_to(&self, adj: &dyn Adjacency, to: &Keyhash) {
        for (kind, object) in self.store.objects() {
            adj.send(to, FRAME_TOPOLOGY_PUSH, &encode_push(kind, &object));
        }
    }

    /// Where a resolution goes when a conflict retires a locator.  The node
    /// re-resolves the subject from its own position.
    fn re_resolve(&mut self, adj: &dyn Adjacency, subject: &Keyhash) {
        let anchor = self.anchor();
        let req = crate::resolution::ResolveRequest {
            subject: *subject,
            anchor,
            path: self.position.path.clone(),
            nibbles: self.position.nibbles,
            nonce: rhtn_codec::cose::sha256(&[&subject[..], &self.now.to_be_bytes()[..]].concat())[..16].try_into().unwrap(),
        };
        let target = self.patron().or(self.serving_node).or_else(|| self.peers.iter().next().copied());
        if let Some(t) = target {
            adj.send(&t, crate::resolution::REQUEST_RESOLVE, &req.encode());
        }
    }

    // ------------------------------------------------------------ the memo

    /// The memo this node originates for a change to one of its own slots.
    pub fn memo_for_slot(&self, slot: u64) -> Option<Memo> {
        let s = self.slots.get(&slot)?;
        Some(Memo { patron: self.me(), position: self.position.clone(), slot, timestamp: s.timestamp, occupant: s.occupant })
    }

    /// Send a memo rootward: to the patron where a session exists, and
    /// otherwise to the nearest infrastructure node on the patron chain
    /// (`wire-format.md` §10.2).
    pub fn send_memo(&self, adj: &dyn Adjacency, memo: &Memo) -> Option<Keyhash> {
        let to = match self.patron() {
            Some(p) if adj.has_session(&p) => p,
            Some(_) | None => self.serving_node.filter(|_| self.patron().is_some())?,
        };
        adj.send(&to, FRAME_TOPOLOGY_MEMO, &memo.encode());
        Some(to)
    }

    /// Originate the memo for a slot and send it rootward.
    pub fn originate_memo(&self, adj: &dyn Adjacency, slot: u64) -> Option<Keyhash> {
        let memo = self.memo_for_slot(slot)?;
        self.send_memo(adj, &memo)
    }

    /// Receive a memo from below.
    pub fn receive_memo(&mut self, adj: &dyn Adjacency, from: &Keyhash, body: &[u8]) -> MemoOutcome {
        let memo = match Memo::decode(body) {
            Ok(m) => m,
            Err(e) => return MemoOutcome::Malformed(e),
        };
        // a memo whose anchor is not your subnet's is dropped, not forwarded:
        // neither applied to a table nor passed on (§10.2)
        if memo.position.anchor != self.anchor() {
            return MemoOutcome::ForeignSubnet;
        }
        let me = self.me();
        // "is this me?" — the cycle check, on the memo alone, at any depth
        if memo.patron == me {
            return self.cycle_check(adj, from, &memo);
        }
        // a serving node runs the check for its attached clients as well;
        // the records that answer it are the client's, not this node's
        if self.attached.contains(&memo.patron) {
            return MemoOutcome::ForAttachedClient { client: memo.patron };
        }
        // a node holding that slot at or after the memo's timestamp does not
        // forward it (§10.2)
        let key = (memo.patron, memo.slot);
        if let Some(held) = self.memo_table.get(&key) {
            if held.timestamp >= memo.timestamp {
                return MemoOutcome::AlreadyPassed;
            }
        }
        if self.keeps_memo_table {
            self.memo_table.insert(key, Slot { occupant: memo.occupant, timestamp: memo.timestamp });
        }
        match self.send_memo(adj, &memo) {
            Some(to) => MemoOutcome::Forwarded { to },
            None => MemoOutcome::StoppedAtRoot,
        }
    }

    /// A memo naming this node has come back from below.  Confirm it against
    /// this node's own row before acting; a fabricated memo fails here at no
    /// traffic cost (`wire-format.md` §10.2.3).
    fn cycle_check(&mut self, adj: &dyn Adjacency, from: &Keyhash, memo: &Memo) -> MemoOutcome {
        let row = self.slots.get(&memo.slot).copied();
        let confirmed = row.is_some_and(|r| r.occupant == memo.occupant && memo.occupant.is_some());
        if !confirmed {
            // no disavowal, no fetch, no forward, rows unchanged
            return MemoOutcome::Unconfirmed;
        }
        // disavow the direct subordinate that forwarded the memo, reason
        // code 5, without prejudice (§10.2.4)
        let Some(dis) = self.disavow(from, Some(5)) else { return MemoOutcome::Unconfirmed };
        self.originate_push(adj, KIND_TRANSACTION, &dis.bytes);
        if let Some(slot) = self.slot_of(from) {
            self.set_slot(slot, None, self.now);
        }
        MemoOutcome::CycleConfirmed { disavowed: *from }
    }

    /// Sign a disavowal of `node` by this node, advancing its own chain.
    pub fn disavow(&mut self, node: &Keyhash, code: Option<u64>) -> Option<Record> {
        let back = self.archive.next_back_pointers();
        let body = tx::disavowal_body(&back, &self.me(), node, self.now, code);
        let env = tx::envelope(tx::TYPE_DISAVOWAL, &body, &[&self.identity]);
        let rec = Record::parse(&env).ok()?;
        self.archive.append(rec.clone()).ok()?;
        Some(rec)
    }
}

/// The push a node sends for an endpoint record.
pub fn endpoint_push(object: &[u8]) -> Vec<u8> {
    encode_push(KIND_ENDPOINT_RECORD, object)
}
