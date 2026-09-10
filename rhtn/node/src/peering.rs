//! Peering, the direct payload path, and sibling replication (design §3.4,
//! §6.3, §12.6.3, §12.7.5; `wire-format.md` §4.4).

use crate::currency::{CurrencyState, Gate, Operation, Staple, gate};
use crate::resolution::NetworkPoint;
use crate::store::subjects;
use crate::view::NodeView;
use crate::Keyhash;
use rhtn_archive::record::Record;
use rhtn_archive::tx::{self, TYPE_PEERING};
use rhtn_codec::cbor::*;
use rhtn_codec::encode::*;
use std::collections::BTreeSet;

/// A peering record (`wire-format.md` §4.4) as a holder reads it.  Both
/// endpoints' addresses and ASNs are in it, which is what makes
/// concentration observable (design §3.4).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Peering {
    pub a: Keyhash,
    pub b: Keyhash,
    pub a_point: NetworkPoint,
    pub b_point: NetworkPoint,
    pub timestamp: u64,
    pub replication_commitment: Option<u64>,
    pub proof_of_presence: Option<[u8; 32]>,
}

impl Peering {
    pub fn from_record(rec: &Record) -> Result<Self, String> {
        if rec.tx_type != TYPE_PEERING {
            return Err("not a peering".into());
        }
        let body = &rec.bytes[rec.body.clone()];
        let point = |k: u64| -> Result<NetworkPoint, String> {
            let r = value_slice(body, k).ok_or("network point")?;
            NetworkPoint::decode(&body[r])
        };
        Ok(Peering {
            a: rec.field_hash(1).ok_or("field 1")?,
            b: rec.field_hash(2).ok_or("field 2")?,
            a_point: point(3)?,
            b_point: point(4)?,
            timestamp: rec.field_uint(5).ok_or("field 5")?,
            replication_commitment: rec.field_uint(6),
            proof_of_presence: rec.field_hash(8),
        })
    }

    /// Whether the two endpoints sit in one autonomous system.  This is
    /// concentration, which ASN can show; it is not independence, which ASN
    /// cannot (design §3.4, §17.3).
    pub fn concentrated(&self) -> Option<bool> {
        Some(self.a_point.asn? == self.b_point.asn?)
    }
}

/// A peering body (`wire-format.md` §4.4).  The ASN each endpoint carries is
/// its own claim; nothing validates it against the address, and design
/// §17.3 says nothing can.
#[allow(clippy::too_many_arguments)]
pub fn peering_body(back: [&[[u8; 32]]; 2], a: &Keyhash, b: &Keyhash, a_point: &NetworkPoint, b_point: &NetworkPoint, timestamp: u64, commitment: Option<u64>, pop: &[u8; 32]) -> Vec<u8> {
    let mut out = Vec::new();
    emit_map_head(&mut out, 7 + commitment.is_some() as usize);
    tx::emit_back_pointers(&mut out, &[back[0].to_vec(), back[1].to_vec()]);
    emit_uint(&mut out, 1);
    emit_bstr(&mut out, a);
    emit_uint(&mut out, 2);
    emit_bstr(&mut out, b);
    emit_uint(&mut out, 3);
    a_point.emit(&mut out);
    emit_uint(&mut out, 4);
    b_point.emit(&mut out);
    emit_uint(&mut out, 5);
    emit_uint(&mut out, timestamp);
    if let Some(c) = commitment {
        emit_uint(&mut out, 6);
        emit_uint(&mut out, c);
    }
    emit_uint(&mut out, 8);
    emit_bstr(&mut out, pop);
    out
}

/// Which path payload takes to a peer (design §12.6.3).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PayloadPath {
    /// Inside the horizon: the peers exchange addresses and connect, and
    /// nobody sees the flow.
    Direct,
    /// Outside the horizon, or where the user chose it: both serving infra
    /// nodes carry ciphertext.
    Relayed,
}

/// A user's choice, which overrides either default (design §12.6.3: both
/// defaults must be overridable).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PathOverride {
    None,
    ForceDirect,
    ForceRelayed,
}

/// What a node replicates to its siblings (design §3.4).  Payload queues are
/// not on this list, and the type says so: there is no variant for one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Replicated {
    /// A topology object, byte-for-byte with its body-kind tag.
    Topology { kind: u64, object: Vec<u8> },
    /// A trust-bearing transaction.
    TrustBearing { object: Vec<u8> },
    /// A client's reachability, so a sibling answering in failover knows the
    /// client's status (design §14.1.2).
    Reachability { client: Keyhash, reachable: bool },
}

/// A sibling that takes replicated state.
pub trait Replica {
    fn take(&self, item: &Replicated);
    fn who(&self) -> Keyhash;
}

impl NodeView {
    /// The peering edges this node holds for `node`, from stored peering
    /// records.  Peering transactions are public, so this is observable
    /// from held topology (design §12.7.5).
    pub fn peers_of(&self, node: &Keyhash) -> BTreeSet<Keyhash> {
        let mut out = BTreeSet::new();
        for rec in self.store.transactions().filter(|r| r.tx_type == TYPE_PEERING) {
            let ends = subjects(rec);
            if ends.contains(node) {
                out.extend(ends.into_iter().filter(|e| e != node));
            }
        }
        out
    }

    /// An infra node with no cross-tree peers exposes its subordinates to a
    /// cascade with no independent replication path (design §12.7.5).  Not
    /// an error, and not the operator's private risk either.
    pub fn peerless_infra(&self, node: &Keyhash) -> bool {
        self.table.is_infra(node) && self.peers_of(node).is_empty()
    }

    /// The peering records this node holds, parsed.
    pub fn peerings(&self) -> Vec<Peering> {
        self.store.transactions().filter(|r| r.tx_type == TYPE_PEERING).filter_map(|r| Peering::from_record(r).ok()).collect()
    }

    /// Peering is trust-bearing, so it is gated on the acting credential's
    /// currency (design §12.6.5).  Returns the body this node is willing to
    /// sign; nothing is produced while the gate refuses.
    pub fn peer_gated(&self, cur: &CurrencyState, other: &Keyhash, staple: Staple) -> Result<Vec<u8>, Gate> {
        match gate(Operation::TrustBearing, staple, cur.is_superseded(&self.me())) {
            Gate::Proceed => {}
            refusal => return Err(refusal),
        }
        let me = self.me();
        let back_me = self.archive.next_back_pointers();
        let back_other = vec![rhtn_archive::genesis(other)];
        let mine = self.own_endpoints().first().cloned().unwrap_or(NetworkPoint::new([127, 0, 0, 1], None));
        let theirs = NetworkPoint::new([127, 0, 0, 2], None);
        let pop = rhtn_codec::cose::sha256(&[&me[..], &other[..]].concat());
        Ok(peering_body([&back_me, &back_other], &me, other, &mine, &theirs, self.now, Some(1 << 20), &pop))
    }

    /// Which path payload takes to `peer`.  The check is the whole of it:
    /// is this peer in my h = 2 topology store?  A peering edge carries none
    /// of the subnet's authority and does not count (design §15.1, §6.3).
    pub fn payload_path(&self, peer: &Keyhash, over: PathOverride) -> PayloadPath {
        match over {
            PathOverride::ForceDirect => return PayloadPath::Direct,
            PathOverride::ForceRelayed => return PayloadPath::Relayed,
            PathOverride::None => {}
        }
        let me = self.me();
        if self.table.horizon(&me, 2).contains(peer) { PayloadPath::Direct } else { PayloadPath::Relayed }
    }

    /// The replication set a serving node pushes to its clients: its own
    /// siblings, not its patron's, where those differ (design §14.1.2).
    pub fn replication_set(&self) -> BTreeSet<Keyhash> {
        self.table.siblings(&self.me())
    }

    /// Replicate to siblings.  Topology and trust-bearing transaction
    /// history go; payload queues do not, because the mailbox is one node
    /// (design §3.4, §14.1.6).
    pub fn replicate(&self, siblings: &[&dyn Replica], item: &Replicated) {
        for s in siblings {
            s.take(item);
        }
    }

    /// Everything this node replicates to a sibling that joins it: its
    /// topology store and the trust-bearing history in it.  Nothing here
    /// reads the queue.
    pub fn replication_payload(&self) -> Vec<Replicated> {
        self.store.objects().into_iter().map(|(kind, object)| Replicated::Topology { kind, object }).collect()
    }

    /// Whether a move to `new_patron` lies inside the old patron's
    /// replication horizon, in which case the new patron already holds the
    /// node's history and no archive presentation is needed
    /// (`wire-format.md` §4.2.1, design §6.2.3).  Derivable, not declared.
    pub fn inside_replication_horizon(&self, old_patron: &Keyhash, new_patron: &Keyhash) -> bool {
        new_patron == old_patron || self.table.siblings(old_patron).contains(new_patron) || self.table.patrons(old_patron).contains(new_patron)
    }
}

/// Where payload went, and who carried it (design §12.6.3).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Delivery {
    /// The peers exchanged addresses during setup and connected; nobody
    /// else sees the flow.
    Direct { to: Keyhash },
    /// Both serving infra nodes relay ciphertext.
    Relayed { via: Keyhash, to: Keyhash },
    /// The recipient is offline: the recipient's serving node holds
    /// ciphertext until reconnect (design §14.1.4).
    Queued { at: Keyhash, to: Keyhash },
}

/// Whoever carries the bytes, so a test can see who saw them.
pub trait PayloadSink {
    fn carry(&self, carrier: Option<Keyhash>, to: &Keyhash, bytes: &[u8]);
}

impl NodeView {
    /// Send payload to `peer`.  The path decision is the whole of design
    /// §12.6.3's check, taken against this node's own store; where it is
    /// relayed, this node's serving node is the first carrier.
    pub fn send_payload(&self, peer: &Keyhash, over: PathOverride, online: bool, sink: &dyn PayloadSink, bytes: &[u8]) -> Delivery {
        if !online {
            let at = self.serving_node.unwrap_or(self.me());
            sink.carry(Some(at), peer, bytes);
            return Delivery::Queued { at, to: *peer };
        }
        match self.payload_path(peer, over) {
            PayloadPath::Direct => {
                sink.carry(None, peer, bytes);
                Delivery::Direct { to: *peer }
            }
            PayloadPath::Relayed => {
                let via = self.serving_node.unwrap_or(self.me());
                sink.carry(Some(via), peer, bytes);
                Delivery::Relayed { via, to: *peer }
            }
        }
    }
}
