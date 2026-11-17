//! **A node this client serves from, said to have moved**
//! (`light-client-requirements.md` §4) [author, 2026-11-16].
//!
//! An invitation ([`crate::invitation`]) naming a node this identity
//! already agreed to, at an address the agreement does not hold, is not a
//! first invitation: it says the patron's server moved. *"Someone might
//! hijack a device or identity to launch an alternate node and so the user
//! should have the option to not trust a new node, but they can not keep
//! interacting with the old node as if it were active."*
//!
//! So it is held here, one per node, the newest replacing whatever came
//! before, and it goes through four standings:
//!
//! - **Unproved** while nothing at the new address has answered as the
//!   node. The person is not told and the agreement is used as it was: a
//!   link that points at nothing changes nothing.
//! - **Proved** once the node answered there as itself — the key it
//!   presented binding to the patron. Now the person is asked, and the
//!   node is not used at either address until they answer.
//! - **Declined** where they would not connect there. The agreement keeps
//!   the old address and the node stays unused: the person waits for an
//!   invitation they trust. The question stays put to them, and they may
//!   still accept it.
//! - **Failed** where something answered there and did not prove itself
//!   the node. **The person is not told and nothing changes for them**
//!   [author, 2026-11-16]: *"No notification on a failed check to the
//!   user. User sends notification of the failed invite to the
//!   operator."* So a link can shut nothing: *"the attacker would have to
//!   impersonate the node operator AND have a node that verifies as the
//!   operator"*, and a node that verifies as the operator is a proved move
//!   the person answers. The record stands until the operator has been
//!   told ([`Client::failure_reported`]).
//!
//! **Proving is not this module's**: it holds no transport, and the
//! caller dials and says what it found ([`Client::relocation_proved`]).

use crate::ceremony::Client;
use rhtn_archive::Keyhash;
use rhtn_model::Standing;

/// One relocation, as held: where the node is said to answer now, how far
/// that has been settled, and what the inviter called themselves.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Held {
    /// Where the invitation says the node answers.
    pub to: String,
    /// How far it has been settled.
    pub standing: Standing,
    /// What answered instead of the node, where something did.
    pub why: Option<String>,
    /// The inviter's own label, for the screen alone.
    pub label: Option<String>,
    /// **The subnet the invitation named**, where this client holds it:
    /// the one whose orb carries the question [author, 2026-11-16: "The
    /// notifications should be keyed to the subnet"].
    pub subnet: Option<rhtn_archive::Txid>,
}

/// What a dial of the new address found, as the caller reports it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Found {
    /// The node answered there as itself.
    Proved,
    /// Nothing answered as anybody; asked again later.
    Unreached,
    /// Something answered and did not prove itself the node.
    Failed(String),
}

impl Client {
    /// **An invitation for a node already agreed to**: held as a
    /// relocation where it names an address the agreement does not, or
    /// where one is already held — the newest invitation replacing it,
    /// which is how a patron resolves a declined one.
    /// Whether it was held; false for a node never agreed to, whose
    /// invitation is a first one, and for one naming the address already
    /// agreed with nothing held.
    pub fn offer_relocation(
        &mut self,
        node: Keyhash,
        to: String,
        subnet: Option<rhtn_archive::Txid>,
        label: Option<String>,
    ) -> bool {
        let Some(agreed) = self.payload.agreed.get(&node) else {
            return false;
        };
        if agreed.contains(&to) && !self.payload.relocations.contains_key(&node) {
            return false;
        }
        tracing::info!(
            target: "pay",
            node = %crate::diag::id8(&node),
            "relocation.offered"
        );
        // a subnet this client does not hold names nothing here
        let subnet = subnet.filter(|s| self.subnets().contains(s));
        self.payload.relocations.insert(
            node,
            Held {
                to,
                standing: Standing::Unproved,
                why: None,
                label,
                subnet,
            },
        );
        self.changed.insert(rhtn_model::Noun::Subnets);
        true
    }

    /// **What a dial of the new address found**, for the relocation held
    /// for `node` at `to`. Only an unproved one moves: a proof arriving
    /// for an address since replaced is about nothing held.
    pub fn relocation_proved(&mut self, node: &Keyhash, to: &str, found: Found) {
        let Some(h) = self.payload.relocations.get_mut(node) else {
            return;
        };
        if h.to != to || h.standing != Standing::Unproved {
            return;
        }
        match found {
            Found::Proved => h.standing = Standing::Proved,
            Found::Unreached => return,
            Found::Failed(why) => {
                h.standing = Standing::Failed;
                h.why = Some(why);
            }
        }
        tracing::info!(
            target: "pay",
            node = %crate::diag::id8(node),
            standing = ?h.standing,
            "relocation.proved"
        );
        self.changed.insert(rhtn_model::Noun::Subnets);
    }

    /// **The person accepted the new address**: the agreement is that
    /// address from now on and the relocation is gone. The address, where
    /// a proved relocation was held, or one they had declined and come
    /// back to; nothing is changed otherwise.
    pub fn accept_relocation(&mut self, node: &Keyhash) -> Option<String> {
        if !matches!(
            self.payload.relocations.get(node)?.standing,
            Standing::Proved | Standing::Declined
        ) {
            return None;
        }
        let h = self.payload.relocations.remove(node)?;
        self.payload.agreed.insert(*node, vec![h.to.clone()]);
        self.changed.insert(rhtn_model::Noun::Subnets);
        Some(h.to)
    }

    /// **The person would not connect there.** The agreement stands and
    /// the node stays unused; whether a proved relocation was declined.
    pub fn decline_relocation(&mut self, node: &Keyhash) -> bool {
        match self.payload.relocations.get_mut(node) {
            Some(h) if h.standing == Standing::Proved => {
                h.standing = Standing::Declined;
                self.changed.insert(rhtn_model::Noun::Subnets);
                true
            }
            _ => false,
        }
    }

    /// **The operator has been told of a failed invitation**: its record
    /// goes. Whether a failed one was held.
    pub fn failure_reported(&mut self, node: &Keyhash) -> bool {
        if self
            .payload
            .relocations
            .get(node)
            .is_some_and(|h| h.standing == Standing::Failed)
        {
            self.payload.relocations.remove(node);
            return true;
        }
        false
    }

    /// **Whether this client may use `node`**: not while a move of it is
    /// put to the person, proved or declined. An unproved or failed one
    /// changes nothing.
    #[must_use]
    pub fn node_usable(&self, node: &Keyhash) -> bool {
        self.payload
            .relocations
            .get(node)
            .is_none_or(|h| matches!(h.standing, Standing::Unproved | Standing::Failed))
    }

    /// The relocation held for `node`, as the model reads it.
    #[must_use]
    pub fn relocation(&self, node: &Keyhash) -> Option<rhtn_model::Relocation> {
        let h = self.payload.relocations.get(node)?;
        Some(rhtn_model::Relocation {
            node: *node,
            from: self.payload.agreed.get(node).cloned().unwrap_or_default(),
            to: h.to.clone(),
            standing: h.standing,
            why: h.why.clone(),
            label: h.label.clone(),
        })
    }

    /// Every relocation held, in node order.
    #[must_use]
    pub fn relocations(&self) -> Vec<rhtn_model::Relocation> {
        self.payload
            .relocations
            .keys()
            .filter_map(|n| self.relocation(n))
            .collect()
    }

    /// **The move a subnet's orb carries**: the one whose invitation named
    /// this subnet, else — for an invitation that named none — the one
    /// held for the node serving it here.
    #[must_use]
    pub fn relocation_in(&self, subnet: &rhtn_archive::Txid) -> Option<rhtn_model::Relocation> {
        if let Some((n, _)) = self
            .payload
            .relocations
            .iter()
            .find(|(_, h)| h.subnet == Some(*subnet))
        {
            return self.relocation(n);
        }
        let n = self.serving_in(subnet)?;
        self.payload
            .relocations
            .get(&n)
            .filter(|h| h.subnet.is_none())?;
        self.relocation(&n)
    }

    /// **The node serving this client in `subnet`**: the one its attach
    /// there recorded, else its patron's — a member's node is its
    /// patron's, and a light client two levels down is served by its
    /// patron's patron (SES-002), which only the attach can say. None for
    /// a root with nothing recorded.
    #[must_use]
    pub fn serving_in(&self, subnet: &rhtn_archive::Txid) -> Option<Keyhash> {
        let at = self.position_in(subnet)?;
        self.payload
            .serving_at
            .get(&at.anchor)
            .copied()
            .or_else(|| self.patron_in(subnet))
    }

    /// **Whose line this client holds in `subnet`**: the patron of the
    /// line whose position is the one held there. None for a root, whose
    /// position no adoption gave.
    #[must_use]
    pub fn patron_in(&self, subnet: &rhtn_archive::Txid) -> Option<Keyhash> {
        let at = self.position_in(subnet)?;
        crate::rotation::relationships(&self.archive)
            .into_iter()
            .find(|r| {
                r.position.anchor == at.anchor
                    && r.position.path == at.path
                    && r.position.nibbles == at.nibbles
            })
            .map(|r| r.patron)
    }
}
