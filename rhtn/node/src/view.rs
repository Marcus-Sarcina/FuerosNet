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

use crate::resolution::LocatorStore;
use crate::store::{Horizon, TopologyStore};
use rhtn_policy::{Policy, ReferenceMetric};

/// One subordinate slot under this node's own position: the child index a
/// memo names, with the occupant and the timestamp that put them there
/// (`wire-format.md` §10.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Slot {
    pub occupant: Option<Keyhash>,
    pub timestamp: u64,
}

/// A node's own state above the session.
/// A source of the time, in seconds since the epoch.
pub type Clock = Arc<dyn Fn() -> u64 + Send + Sync>;

pub struct NodeView {
    pub identity: Arc<SigningIdentity>,
    /// This node's own position in its primary subnet; its anchor names
    /// the subnet a memo may travel in (`wire-format.md` §10.2).
    pub position: Locator,
    /// This node's position in every other subnet it is bound in, by
    /// anchor (design §3.1.1: one node, several bindings, each its own).
    pub positions: BTreeMap<Keyhash, Locator>,
    pub table: Table,
    pub store: TopologyStore,
    /// Locators this node holds for other parties (`wire-format.md` §2.3).
    pub locators: LocatorStore,
    /// Staples handed over with introductions, by subject (design §12.6.5).
    pub staples: BTreeMap<Keyhash, Vec<u8>>,
    /// This node's own archive, which its disavowals advance.
    pub archive: Archive,
    /// `slot -> occupant`, this node's own subordinate slots.
    pub slots: BTreeMap<u64, Slot>,
    /// `(patron, slot) -> occupant`: the memo table, optional by
    /// `wire-format.md` §10.2.2 and kept here.
    pub memo_table: BTreeMap<(Keyhash, u64), Slot>,
    /// Whether this node keeps a memo table at all (`wire-format.md`
    /// §10.2.2: optional, and detection degrades gracefully without one).
    pub keeps_memo_table: bool,
    /// The position each patron in the memo table last gave for itself:
    /// what a downward memo descends by (`wire-format.md` §10.2.4).
    pub memo_positions: BTreeMap<Keyhash, Locator>,
    /// Light clients attached to this node (design §14.1.2).
    pub attached: BTreeSet<Keyhash>,
    /// Peering edges (design §6.3): adjacency for the flood, and no part of
    /// the horizon (design §15.1).
    pub peers: BTreeSet<Keyhash>,
    /// The nearest infrastructure node on this node's patron chain, where
    /// this node is not itself infrastructure.
    pub serving_node: Option<Keyhash>,
    /// The node's own clock, read at every decision that needs the time.
    /// A view built for a test holds a fixed clock; a running node installs
    /// its configuration's clock at start, so issuance, outage stamps and
    /// the ladder's intervals all read time that moves.
    pub clock: Clock,
    /// The trust policy this node computes standing with: the reference
    /// metric unless its operator substitutes one (design §16.1).  Nothing
    /// the node stores or forwards consults it (design §16.4).
    pub policy: Arc<dyn Policy<Keyhash>>,
}

impl NodeView {
    pub fn new(identity: Arc<SigningIdentity>, position: Locator) -> Self {
        let me = identity.public.keyhash;
        let mut table = Table::with_me(me);
        table.mark_infra(me);
        NodeView {
            identity,
            position,
            positions: BTreeMap::new(),
            table,
            store: TopologyStore::new(),
            locators: LocatorStore::new(),
            staples: BTreeMap::new(),
            archive: Archive::new(me),
            slots: BTreeMap::new(),
            memo_table: BTreeMap::new(),
            keeps_memo_table: true,
            memo_positions: BTreeMap::new(),
            attached: BTreeSet::new(),
            peers: BTreeSet::new(),
            serving_node: None,
            clock: Arc::new(|| 1_800_000_000),
            policy: Arc::new(ReferenceMetric::default()),
        }
    }

    /// The time now, by this node's clock.
    pub fn now(&self) -> u64 {
        (self.clock)()
    }

    /// Fix the clock at `t`: what a test or a simulation does to move time.
    /// A running node's clock is its configuration's, installed at start.
    pub fn set_now(&mut self, t: u64) {
        self.clock = Arc::new(move || t);
    }

    pub fn me(&self) -> Keyhash {
        self.identity.public.keyhash
    }

    /// The subnet this node sits in: its own locator's anchor.
    pub fn anchor(&self) -> Keyhash {
        self.position.anchor
    }

    /// This node's patron, where it has one.  A node bound under several
    /// patrons has one per subnet; this is the primary subnet's, and
    /// `patron_in` picks by anchor.
    pub fn patron(&self) -> Option<Keyhash> {
        self.patron_in(&self.anchor())
    }

    /// The patron of this node's binding in the subnet `anchor` names,
    /// falling back to any patron where no binding records its anchor.
    pub fn patron_in(&self, anchor: &Keyhash) -> Option<Keyhash> {
        let me = self.me();
        let open: Vec<_> = self.table.bindings().iter().filter(|b| b.node == me && b.open()).collect();
        open.iter().find(|b| b.anchor.as_ref() == Some(anchor)).or_else(|| open.first()).map(|b| b.patron)
    }

    /// This node's own position in the subnet `anchor` names.
    pub fn position_in(&self, anchor: &Keyhash) -> Option<&Locator> {
        if self.position.anchor == *anchor { Some(&self.position) } else { self.positions.get(anchor) }
    }

    /// Every anchor this node has a position under.
    pub fn anchors(&self) -> Vec<Keyhash> {
        let mut out = vec![self.position.anchor];
        out.extend(self.positions.keys().copied());
        out
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

    /// The child index an adoption under this node assigns: the last nibble
    /// of the subordinate's locator path, under this node's own path
    /// (`wire-format.md` §2.1, §10.2).
    pub fn slot_from(loc: &Locator) -> Option<u64> {
        if loc.nibbles == 0 {
            return None;
        }
        let i = (loc.nibbles - 1) as usize;
        let byte = *loc.path.get(i / 2)?;
        Some(if i.is_multiple_of(2) { (byte >> 4) as u64 } else { (byte & 0x0f) as u64 })
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
