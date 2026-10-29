//! What leaving costs (`light-client-requirements.md` §6).
//!
//! **The warning that has to come before the act**: §6 wants the person
//! told, before any change that ends or alters an authority relationship,
//! which of the resources they use that change takes away. The answer is
//! a difference between two horizons — the one this client has and the
//! one it would have — and the resources it can name are the ones its
//! catalog view holds.
//!
//! **This is a floor and not a ceiling, and it says so.** Access to a
//! resource is the resource's own predicate, evaluated by its host
//! against the asker's reach (design §11.5): a host that stays in view
//! may still refuse once a line is gone, and this client holds no
//! predicate to try. So [`Cost`] carries its own limits in
//! [`Cost::limits`] rather than presenting a list that reads as
//! complete — a stale or partial answer is worth having, and worth having
//! marked.

use crate::Keyhash;
use crate::ceremony::{Abort, Client};
use rhtn_archive::Txid;
use rhtn_archive::catalog::CatalogEntry;
use std::collections::BTreeMap;

/// The cost's own types are the model's (`rhtn_model`): a loss, why it
/// goes, every reason the list is a floor, and the whole.
pub use rhtn_model::{Cost, Limit, Loss, LossReason};

/// The horizon radius every reading in this client uses (design §15.1).
const H: usize = 2;

impl Client {
    /// **What leaving `patron` costs**, computed before
    /// [`Client::depart`] is called and never retained.
    ///
    /// Refused where that key holds no relationship with `patron`, which
    /// is the same refusal `depart` gives: a cost shown for a departure
    /// that would not happen is worse than no cost at all.
    pub fn cost_of_leaving(&self, patron: &Keyhash) -> Result<Cost, Abort> {
        // the archive decides whether there is a relationship, because it
        // is what `depart` asks; the table decides what ending it does to
        // the horizon, because it is what the horizon is folded from
        crate::rotation::relationships(&self.archive)
            .into_iter()
            .find(|r| r.patron == *patron)
            .ok_or(Abort::NoRelationship(*patron))?;
        let me = self.keyhash();
        let lines: Vec<Txid> = self
            .horizon
            .table
            .bindings()
            .iter()
            .filter(|b| b.open() && b.node == me && b.patron == *patron)
            .map(|b| b.adoption)
            .collect();
        Ok(self.cost_of_ending(&lines))
    }

    /// The same for an arbitrary set of this client's open lines.
    ///
    /// **A transfer ends a line too** — the adoption that carries it
    /// supersedes the binding to the former patron — so the cost of
    /// accepting one is this question asked about that patron. A
    /// disavowal is minted only by a node holding its own seed
    /// (`rhtn-node`'s `disavow`), so it is not an act this surface has.
    pub fn cost_of_ending(&self, lines: &[Txid]) -> Cost {
        let me = self.keyhash();
        let inside = self.horizon.table.horizon(&me, H);
        let after = self.horizon.table.horizon_without(&me, H, lines);
        let leaving: Vec<Keyhash> = inside.difference(&after).copied().collect();

        // **counted by resource and not by entry.** The same resource can
        // be served by two nodes, and one of them leaving is the loss of
        // one way to it rather than of the resource: a list counting
        // entries would tell somebody they were about to lose something
        // they would still have, which is the one error a warning cannot
        // afford — it is read as a reason not to act.
        let mut ways: BTreeMap<Keyhash, Vec<(Keyhash, CatalogEntry)>> = BTreeMap::new();
        for (node, e) in self.catalog.entries() {
            ways.entry(e.resource).or_default().push((node, e));
        }
        let mut losing = Vec::new();
        let mut keeping = 0u32;
        for (resource, served) in ways {
            // a resource already out of reach is not a cost of leaving
            let open_now: Vec<&(Keyhash, CatalogEntry)> = served
                .iter()
                .filter(|(n, e)| inside.contains(n) && inside.contains(&e.owner))
                .collect();
            if open_now.is_empty() {
                continue;
            }
            if open_now
                .iter()
                .any(|(n, e)| after.contains(n) && after.contains(&e.owner))
            {
                keeping += 1;
                continue;
            }
            // every way to it closes, and the reason is the one the last
            // way standing gives: a host still in view whose owner is
            // gone is a different thing to be told than a host gone
            let (node, e) = open_now
                .iter()
                .copied()
                .find(|(n, _)| after.contains(n))
                .unwrap_or(open_now[0]);
            losing.push(Loss {
                resource,
                node: *node,
                owner: e.owner,
                service_type: e.service_type.clone(),
                instance: e.instance.clone(),
                reason: match after.contains(node) {
                    true => LossReason::OwnerLeaves,
                    false => LossReason::HostLeaves,
                },
            });
        }

        let mut limits = vec![Limit::PredicateNotHeld];
        for (n, p) in &self.catalog.portions {
            if !inside.contains(n) {
                continue;
            }
            if p.stale {
                limits.push(Limit::Stale { node: *n });
            }
            if p.truncated {
                limits.push(Limit::Truncated { node: *n });
            }
        }
        for (n, _) in self.horizon.reachable_infra() {
            if !self.catalog.portions.contains_key(&n) {
                limits.push(Limit::Unswept { node: n });
            }
        }
        if lines.len() > 1 {
            limits.push(Limit::SeveralLines {
                lines: lines.len() as u32,
            });
        }
        if lines.is_empty() {
            limits.push(Limit::NotInTable);
        }
        Cost {
            losing,
            keeping,
            leaving,
            limits,
        }
    }
}
