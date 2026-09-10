//! What this node's own tables give its policy, and what the policy gives
//! back (design §16.1, §16.2.1).
//!
//! Standing is computed on demand from the evidence the node holds and is
//! retained nowhere (design §16.2: "nothing here is a retained
//! entitlement").  Nothing the node stores or forwards depends on it, so a
//! node running a substitute policy produces the same wire traffic as one
//! running the reference (design §16.4).

use crate::Keyhash;
use crate::view::NodeView;
use rhtn_archive::record::Record;
use rhtn_policy::{Evaluation, Evidence};

impl NodeView {
    /// The evidence this node holds, as its policy sees it: adoption pairs
    /// from the topology table's open bindings, acquaintance pairs from the
    /// presence records kept as evidence and the peering records stored.
    /// Three sources, one edge per pair (design §16.2.1).
    pub fn evidence(&self) -> Evidence<Keyhash> {
        let mut ev = Evidence::new(self.me());
        for b in self.table.bindings().iter().filter(|b| b.open()) {
            ev.adopt(b.patron, b.node);
        }
        for (_, bytes) in self.store.presence_records() {
            if let Ok(rec) = Record::parse(&bytes) {
                let p = rec.participants();
                if p.len() == 2 {
                    ev.meet(p[0], p[1]);
                }
            }
        }
        for p in self.peerings() {
            ev.meet(p.a, p.b);
        }
        ev
    }

    /// This node's standing for `subject` under its own policy, now.
    pub fn standing(&self, subject: &Keyhash) -> f64 {
        self.policy.score(&self.evidence(), subject)
    }

    /// One computation over a set of candidates (design §16.2).
    pub fn evaluate(&self, candidates: &[Keyhash]) -> Evaluation<Keyhash> {
        self.policy.evaluate(&self.evidence(), candidates)
    }
}
