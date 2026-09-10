//! The forwarding rule and the rootward memo (`wire-format.md` §10.1, §10.2;
//! design §15).
//!
//! Forward if and only if you stored it, to every adjacency except the one
//! it arrived from; store when the subject falls in your own `h_store`.
//! Nothing in a frame tells a node how far to forward, and nothing here
//! reads one.

use crate::resolution::Path;
use crate::store::{Decision, Horizon, KIND_TRANSACTION};
use crate::view::{NodeView, Slot};
use crate::{Adjacency, Keyhash};
use rhtn_archive::record::Record;
use rhtn_archive::topology::Evaluation;
use rhtn_archive::tx::{self, Locator, TYPE_ADOPTION, TYPE_DEPARTURE, TYPE_DISAVOWAL};
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
    /// Forwarded rootward, and sent down the branch holding the other slot
    /// this node's table shows the occupant in (§10.2.4).
    ForwardedAndDescended { to: Keyhash, down: Keyhash },
    /// Applied where a table is kept; this node is the root.
    StoppedAtRoot,
    /// This node is the root, and the memo went down the other branch.
    StoppedAtRootAndDescended { down: Keyhash },
    /// This node holds that slot at or after the memo's timestamp.
    AlreadyPassed,
    /// Field 1 is this node and its own row confirms the memo: a cycle.
    CycleConfirmed { disavowed: Keyhash },
    /// Field 1 is this node and its own row does not confirm the memo.
    Unconfirmed,
    /// Field 1 is an attached client and the memo came from below that
    /// client: the hit is handed to the client at contact, and the records
    /// that answer it are theirs (§10.2).
    ForAttachedClient { client: Keyhash },
    /// The occupant sits in one of this node's own slots and the memo
    /// places it under another patron: this node is the patron who has not
    /// just spoken, its own records decide, and nothing compels it to act
    /// (§10.2.4).  `forwarded` says where the memo went on rootward.
    HeldElsewhere { patron: Keyhash, slot: u64, timestamp: u64, mine: u64, forwarded: Option<Keyhash> },
    /// A downward memo, passed on toward the patron whose slot it did not
    /// name; this node's table was updated on the way past.
    Descended { to: Keyhash },
    /// A downward memo this node can take no further: no table, no other
    /// slot, or no session down that branch.
    DescentEnded,
    /// This node has a patron but no session toward it and no serving node
    /// to stand in: nowhere to send.
    Unroutable,
    Malformed(String),
}

impl NodeView {
    /// Originate a push for an object this node is a party to: it enters
    /// this node's own store first, so an echo of it is a duplicate and dies,
    /// and goes to every adjacency (`wire-format.md` §10.1).
    pub fn originate_push<L: Lookup + ?Sized>(&mut self, adj: &dyn Adjacency, kind: u64, object: &[u8], ids: &L) -> Decision {
        let me = self.me();
        self.take_object(adj, &me, kind, object, ids)
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
                    // a change to one of this node's own slots travels rootward
                    // as a memo (§10.2)
                    if let Some(slot) = self.apply_stored(object, ids) {
                        self.originate_memo(adj, slot);
                    }
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

    /// Fold a stored transaction into this node's own table.  A relay's
    /// table binds on structural verification and records the evidence as
    /// unevaluated where it cannot dereference it, so its horizon follows
    /// the flood (`wire-format.md` §3.4's *valid versus effective*).  Where
    /// the transaction changes one of this node's own subordinate slots, the
    /// slot is written and returned.
    fn apply_stored<L: Lookup + ?Sized>(&mut self, object: &[u8], ids: &L) -> Option<u64> {
        let rec = Record::parse(object).ok()?;
        let me = self.me();
        let applied = self.table.apply_with(&rec, ids, &self.store, None, Evaluation::Deferred).is_ok();
        if !applied {
            return None;
        }
        match rec.tx_type {
            TYPE_ADOPTION if rec.field_hash(2) == Some(me) => {
                let node = rec.field_hash(1)?;
                let slot = rec.locator().and_then(|l| NodeView::slot_from(&l))?;
                self.set_slot(slot, Some(node), rec.time);
                Some(slot)
            }
            TYPE_DEPARTURE if rec.field_hash(2) == Some(me) => {
                let node = rec.field_hash(1)?;
                let slot = self.slot_of(&node)?;
                self.set_slot(slot, None, rec.time);
                Some(slot)
            }
            TYPE_DISAVOWAL if rec.field_hash(1) == Some(me) => {
                let node = rec.field_hash(2)?;
                let slot = self.slot_of(&node)?;
                self.set_slot(slot, None, rec.time);
                Some(slot)
            }
            _ => None,
        }
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

    /// Where a resolution goes when a conflict retires a record: the node
    /// re-resolves the subject from the locator it holds for that subject,
    /// with a fresh random nonce (`wire-format.md` §7.7.3).  Holding no
    /// locator, it has nothing to resolve from and sends nothing.
    fn re_resolve(&mut self, adj: &dyn Adjacency, subject: &Keyhash) -> Option<crate::resolution::ResolveRequest> {
        let held = self.locators.all(subject);
        let loc = held.first()?.locator.clone();
        let req = crate::resolution::ResolveRequest {
            subject: *subject,
            anchor: loc.anchor,
            path: loc.path.clone(),
            nibbles: loc.nibbles,
            nonce: rhtn_transport::tls::random_bytes(),
        };
        // a light client asks its serving node; an infra node asks the
        // anchor, where a session with it exists
        let target = self.serving_node.or(Some(loc.anchor)).filter(|t| adj.has_session(t))?;
        adj.send(&target, crate::resolution::REQUEST_RESOLVE, &req.encode());
        Some(req)
    }

    // ------------------------------------------------------------ the memo

    /// The memo this node originates for a change to one of its own slots.
    pub fn memo_for_slot(&self, slot: u64) -> Option<Memo> {
        let s = self.slots.get(&slot)?;
        Some(Memo { patron: self.me(), position: self.position.clone(), slot, timestamp: s.timestamp, occupant: s.occupant })
    }

    /// Send a memo rootward in its own subnet: to this node's patron there
    /// where a session exists, and otherwise to the nearest infrastructure
    /// node on the patron chain (`wire-format.md` §10.2).  `None` means this
    /// node is the root of that subnet, or has nowhere to send.
    pub fn send_memo(&self, adj: &dyn Adjacency, memo: &Memo) -> Option<Keyhash> {
        let patron = self.patron_in(&memo.position.anchor)?;
        let to = if adj.has_session(&patron) { patron } else { self.serving_node? };
        adj.send(&to, FRAME_TOPOLOGY_MEMO, &memo.encode());
        Some(to)
    }

    /// Originate the memo for a slot and send it rootward.
    pub fn originate_memo(&self, adj: &dyn Adjacency, slot: u64) -> Option<Keyhash> {
        let memo = self.memo_for_slot(slot)?;
        self.send_memo(adj, &memo)
    }

    /// Receive a memo.  From below it travels rootward, checked for a
    /// cycle and, where a table is kept, for re-parenting; from above it is
    /// a downward memo on its way to the patron who has not just spoken
    /// (`wire-format.md` §10.2, §10.2.1, §10.2.4).  Direction is implied by
    /// where it came from, never by a field.
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
        // a serving node runs the check for its attached clients as well
        // (§10.2): a memo naming an attached client that arrives from below
        // that client is the client's own memo come back, and the records
        // that answer it are the client's, not this node's.  The client's
        // own memo on its way up arrives from the client itself, or from a
        // party outside its subtree, and travels on.
        if self.attached.contains(&memo.patron) && *from != memo.patron && self.table.downline_contains(&memo.patron, from) {
            return MemoOutcome::ForAttachedClient { client: memo.patron };
        }
        // from the patron, or the serving node standing in for it, the memo
        // is descending
        let from_above = self.patron_in(&memo.position.anchor) == Some(*from) || (self.serving_node == Some(*from) && !self.table.downline_contains(&me, from));
        if from_above {
            return self.receive_downward(adj, &memo, body);
        }
        // a node holding that slot at or after the memo's timestamp does not
        // forward it (§10.2)
        let key = (memo.patron, memo.slot);
        if let Some(held) = self.memo_table.get(&key)
            && held.timestamp >= memo.timestamp {
                return MemoOutcome::AlreadyPassed;
            }
        // the re-parenting check needs a table (§10.2.1): the same occupant
        // already held in another slot of this subtree
        let other = if self.keeps_memo_table { self.other_slot_of(&memo) } else { None };
        self.note_memo(&memo);
        let down = other.and_then(|(p, _)| self.hop_toward(&p)).filter(|c| adj.has_session(c));
        if let Some(c) = &down {
            adj.send(c, FRAME_TOPOLOGY_MEMO, body);
        }
        let up = self.send_memo(adj, &memo);
        // the occupant in one of this node's own slots, placed under another
        // patron: this node's own records decide, and the memo travels on
        if let Some(mine) = memo.occupant.and_then(|o| self.slot_of(&o)) {
            return MemoOutcome::HeldElsewhere { patron: memo.patron, slot: memo.slot, timestamp: memo.timestamp, mine, forwarded: up };
        }
        match (up, down) {
            (Some(to), Some(down)) => MemoOutcome::ForwardedAndDescended { to, down },
            (Some(to), None) => MemoOutcome::Forwarded { to },
            (None, Some(down)) if self.patron_in(&memo.position.anchor).is_none() => MemoOutcome::StoppedAtRootAndDescended { down },
            (None, None) if self.patron_in(&memo.position.anchor).is_none() => MemoOutcome::StoppedAtRoot,
            (None, _) => MemoOutcome::Unroutable,
        }
    }

    /// Write the memo's row and the patron's position, where a table is
    /// kept: the table, not the update history (§10.2.2).
    fn note_memo(&mut self, memo: &Memo) {
        if self.keeps_memo_table {
            self.memo_table.insert((memo.patron, memo.slot), Slot { occupant: memo.occupant, timestamp: memo.timestamp });
            self.memo_positions.insert(memo.patron, memo.position.clone());
        }
    }

    /// The other slot this node's table holds the memo's occupant in, if
    /// any: read the other way, by occupant, the table answers the
    /// re-parenting question (§10.2.2).
    fn other_slot_of(&self, memo: &Memo) -> Option<(Keyhash, u64)> {
        let o = memo.occupant?;
        self.memo_table.iter().find(|((p, s), row)| row.occupant == Some(o) && (*p, *s) != (memo.patron, memo.slot)).map(|((p, s), _)| (*p, *s))
    }

    /// This node's subordinate on the way down to `patron`, by anchor and
    /// path as resolution descends (§7.7.2): the child at the next index of
    /// that patron's position after this node's own.
    fn hop_toward(&self, patron: &Keyhash) -> Option<Keyhash> {
        let pos = self.memo_positions.get(patron)?;
        let mine = Path { bytes: self.position.path.clone(), nibbles: self.position.nibbles }.indices();
        let theirs = Path { bytes: pos.path.clone(), nibbles: pos.nibbles }.indices();
        if theirs.len() <= mine.len() || theirs[..mine.len()] != mine[..] {
            return None;
        }
        self.child_at(theirs[mine.len()])
    }

    /// A downward memo (§10.2.4): the intervening nodes are updated on the
    /// way past, and it walks the tree toward the patron whose slot the
    /// memo did not name.  At that patron it stops: its own records say
    /// whether it still holds the subordinate, and nothing compels it.
    fn receive_downward(&mut self, adj: &dyn Adjacency, memo: &Memo, body: &[u8]) -> MemoOutcome {
        let other = if self.keeps_memo_table { self.other_slot_of(memo) } else { None };
        self.note_memo(memo);
        if let Some(mine) = memo.occupant.and_then(|o| self.slot_of(&o)) {
            return MemoOutcome::HeldElsewhere { patron: memo.patron, slot: memo.slot, timestamp: memo.timestamp, mine, forwarded: None };
        }
        match other.and_then(|(p, _)| self.hop_toward(&p)) {
            Some(c) if adj.has_session(&c) => {
                adj.send(&c, FRAME_TOPOLOGY_MEMO, body);
                MemoOutcome::Descended { to: c }
            }
            _ => MemoOutcome::DescentEnded,
        }
    }

    /// A memo naming this node has come back from below.  Confirm it against
    /// this node's own row before acting; a fabricated memo fails here at no
    /// traffic cost (`wire-format.md` §10.2.3).
    fn cycle_check(&mut self, adj: &dyn Adjacency, from: &Keyhash, memo: &Memo) -> MemoOutcome {
        // an empty slot is a row, not a deletion (§10.2.2): a memo about a
        // slot this node emptied is confirmed by the empty row it kept
        let row = self.slots.get(&memo.slot).copied();
        let confirmed = row.is_some_and(|r| r.occupant == memo.occupant);
        if !confirmed {
            // no disavowal, no fetch, no forward, rows unchanged
            return MemoOutcome::Unconfirmed;
        }
        // disavow the direct subordinate that forwarded the memo, reason
        // code 5, without prejudice (§10.2.4); the disavowal enters this
        // node's own store and table, empties the slot, and floods
        let Some(dis) = self.disavow(from, Some(5)) else { return MemoOutcome::Unconfirmed };
        let ids: Vec<rhtn_crypto::Identity> = vec![self.identity.public.clone()];
        self.originate_push(adj, KIND_TRANSACTION, &dis.bytes, &ids);
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
