//! What a node holds and decides from: its own identity and position, the
//! topology table, the topology store, the memo table, and the sessions it
//! has.  Every field is this node's own view; nothing here is shared
//! (design §15.1).

use crate::Keyhash;
use rhtn_archive::chain::Archive;
use rhtn_archive::topology::Table;
use rhtn_archive::tx::Locator;
use rhtn_crypto::SigningIdentity;
use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use crate::store::{Horizon, TopologyStore};

/// One subordinate slot under this node's own position: the child index a
/// memo names, with the occupant and the timestamp that put them there
/// (`wire-format.md` §10.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Slot {
    pub occupant: Option<Keyhash>,
    pub timestamp: u64,
}

/// A node's own state above the session.
pub struct NodeView {
    pub identity: Arc<SigningIdentity>,
    /// This node's own position; its anchor names the subnet a memo may
    /// travel in (`wire-format.md` §10.2).
    pub position: Locator,
    pub table: Table,
    pub store: TopologyStore,
    /// This node's own archive, which its disavowals advance.
    pub archive: Archive,
    /// `slot -> occupant`, this node's own subordinate slots.
    pub slots: BTreeMap<u64, Slot>,
    /// `(patron, slot) -> occupant`: the memo table, optional by
    /// `wire-format.md` §10.2.2 and kept here.
    pub memo_table: BTreeMap<(Keyhash, u64), Slot>,
    /// Whether this node keeps a memo table at all.
    pub keeps_memo_table: bool,
    /// Light clients attached to this node (design §14.1.2).
    pub attached: BTreeSet<Keyhash>,
    /// Peering edges (design §6.3): adjacency for the flood, and no part of
    /// the horizon (design §15.1).
    pub peers: BTreeSet<Keyhash>,
    /// The nearest infrastructure node on this node's patron chain, where
    /// this node is not itself infrastructure.
    pub serving_node: Option<Keyhash>,
    /// The node's own clock.
    pub now: u64,
}

impl NodeView {
    pub fn new(identity: Arc<SigningIdentity>, position: Locator) -> Self {
        let me = identity.public.keyhash;
        let mut table = Table::with_me(me);
        table.mark_infra(me);
        NodeView {
            identity,
            position,
            table,
            store: TopologyStore::new(),
            archive: Archive::new(me),
            slots: BTreeMap::new(),
            memo_table: BTreeMap::new(),
            keeps_memo_table: true,
            attached: BTreeSet::new(),
            peers: BTreeSet::new(),
            serving_node: None,
            now: 1_800_000_000,
        }
    }

    pub fn me(&self) -> Keyhash {
        self.identity.public.keyhash
    }

    /// The subnet this node sits in: its own locator's anchor.
    pub fn anchor(&self) -> Keyhash {
        self.position.anchor
    }

    /// This node's patron, where it has one.
    pub fn patron(&self) -> Option<Keyhash> {
        self.table.patrons(&self.me()).into_iter().next()
    }

    /// A root has no patron, and rootward forwarding stops there
    /// (`wire-format.md` §10.2).
    pub fn is_root(&self) -> bool {
        self.patron().is_none()
    }

    /// design §15.1's `h_store`: the storage rule's question, answered from
    /// this node's own table (`wire-format.md` §10.1.1).
    pub fn in_h_store(&self, subject: &Keyhash) -> bool {
        self.within(subject, 2)
    }

    /// Record a subordinate in a slot, as this node's own row.
    pub fn set_slot(&mut self, slot: u64, occupant: Option<Keyhash>, timestamp: u64) {
        self.slots.insert(slot, Slot { occupant, timestamp });
    }

    pub fn slot_of(&self, occupant: &Keyhash) -> Option<u64> {
        self.slots.iter().find(|(_, s)| s.occupant.as_ref() == Some(occupant)).map(|(k, _)| *k)
    }
}

/// A node's own distance, over adoption and sibling edges (design §15.1).
/// A serving node also holds its whole light-client subtree
/// (design §12.6.1), so an attached client is inside it whatever the walk
/// says.
impl Horizon for NodeView {
    fn within(&self, x: &Keyhash, h: usize) -> bool {
        let me = self.me();
        x == &me || self.attached.contains(x) || self.table.horizon(&me, h).contains(x)
    }
}
