//! What a client's own copy of its horizon gives its policy.
//!
//! **A participant calculates trust distance without asking anyone**, which
//! is one of the two jobs `light-client-requirements.md` §4.2 keeps the
//! copy for — the other being routing around a patron that is not
//! answering. Both have the same reason behind them: the party you would
//! ask is the party that is down.
//!
//! The fold is the node's (`rhtn-node`'s `trust`), over what a client
//! holds instead of what a node does: adoption pairs from the propagated
//! table, acquaintance pairs from the presence records this client is a
//! party to. Nothing here is retained — design §16.2 has standing computed
//! on demand and kept nowhere — and nothing the client sends depends on
//! it (design §16.4).

use crate::ceremony::Client;
use rhtn_archive::Keyhash;
use rhtn_archive::record::Record;
use rhtn_archive::tx::TYPE_PRESENCE;
use rhtn_policy::{Evaluation, Evidence};

impl Client {
    /// The evidence this client holds, as its policy sees it.
    ///
    /// **Two sources where a node has three.** A node adds the peerings it
    /// stores; a client is not a party to one and holds none, so the graph
    /// it builds is its horizon's adoptions plus the meetings it was
    /// present at.
    pub fn evidence(&self) -> Evidence<Keyhash> {
        let me = self.keyhash();
        let mut ev = Evidence::new(me);
        for b in self.horizon.table.bindings().iter().filter(|b| b.open()) {
            ev.adopt(b.patron, b.node);
        }
        for rec in self
            .archive
            .records()
            .filter(|r| r.tx_type == TYPE_PRESENCE)
        {
            let p = rec.participants();
            if p.len() == 2 {
                ev.meet(p[0], p[1]);
            }
        }
        for bytes in self.store.records.values() {
            if let Ok(rec) = Record::parse(bytes)
                && rec.tx_type == TYPE_PRESENCE
            {
                let p = rec.participants();
                if p.len() == 2 {
                    ev.meet(p[0], p[1]);
                }
            }
        }
        // **a patron's determination, taken at face value** (design
        // §18.5): a relationship ended with prejudice is the patron's
        // reading of the party it ended, and a member of the
        // neighbourhood the pair reaches defaults to it rather than
        // ordering it against a departure it may also hold. Read from the
        // records for that reason — whichever object reached the fold
        // first is the one the binding records, and the determination
        // that lost the race is still one the patron made.
        for (patron, node) in self.horizon.determinations() {
            ev.disavow(patron, node);
        }
        ev
    }

    /// This client's standing for `subject` under its own policy, now.
    pub fn standing(&self, subject: &Keyhash) -> f64 {
        self.cfg.policy.score(&self.evidence(), subject)
    }

    /// One computation over a set of candidates (design §16.2).
    pub fn evaluate(&self, candidates: &[Keyhash]) -> Evaluation<Keyhash> {
        self.cfg.policy.evaluate(&self.evidence(), candidates)
    }
}
