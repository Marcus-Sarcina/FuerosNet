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
        // **a patron's determination, taken at face value** (design
        // §18.5): a relationship ended with prejudice is the patron's
        // reading of the party it ended, and a member of the
        // neighbourhood the pair reaches defaults to it rather than
        // ordering it against a departure it may also hold. Read from the
        // store for that reason — whichever object reached the fold first
        // is the one the binding records, and the determination that lost
        // the race is still one the patron made. §4.3 puts the reason in
        // field 4; absent, nothing is alleged.
        for rec in self
            .store
            .transactions()
            .filter(|r| r.tx_type == rhtn_archive::tx::TYPE_DISAVOWAL)
        {
            if !rec
                .field_uint(4)
                .is_some_and(rhtn_archive::topology::End::band)
            {
                continue;
            }
            if let (Some(patron), Some(node)) = (rec.field_hash(1), rec.field_hash(2)) {
                ev.disavow(patron, node);
            }
        }
        ev
    }

    /// This node's standing for `subject` under its own policy, now.
    pub fn standing(&self, subject: &Keyhash) -> f64 {
        self.policy.score(&self.evidence(), subject)
    }

    /// **Expand every grant held here over the horizon as it is now**,
    /// and say how many rows were written and dropped.
    ///
    /// This is `infra-client-requirements.md` §10.2's first moment — "when
    /// an operator is configuring roles" — where the other three are a
    /// membership change, evidence arriving, and the periodic pass for a
    /// time-dependent predicate. It lives here because it is the one
    /// place the table, the gateway and the policy are all in hand.
    pub fn expand_grants(&mut self) -> (usize, usize) {
        let table = self.table.clone_for(self.me());
        let standing = if self.resources.wants_standing() {
            let who = self.resources.candidates(&table);
            crate::grant::Standing::of(&self.evaluate(&who).individual)
        } else {
            crate::grant::Standing::unknown()
        };
        self.resources.refresh(&table, &standing)
    }

    /// One computation over a set of candidates (design §16.2).
    pub fn evaluate(&self, candidates: &[Keyhash]) -> Evaluation<Keyhash> {
        self.policy.evaluate(&self.evidence(), candidates)
    }
}
