//! The operator's view: what this node's configuration exposes, and to
//! whom.
//!
//! `infra-client-requirements.md` §8 obliges the daemon to tell an operator
//! plainly what their subordinates are exposed to by their configuration
//! and conduct.  §10.6 shows the hosting model when a resource is bound,
//! hosted against brokered, and §10.7 fixes what an accessing user sees:
//! the roles they hold, not the predicates that granted them.
//!
//! The catalogue carries all three as PRD-06, a manual entry, because the
//! obligation is discharged by a person reading a screen and not by a
//! value on the wire.
//!
//! What it owes:
//!
//! - **The binding view**: for each resource bound here, whether this node
//!   hosts it or brokers it, since the two expose the subordinate to
//!   different parties (`infra-client-requirements.md` §10.6).
//! - **The exposure view**: what this configuration creates for the
//!   identities below it — who can reach them, what is logged, and what
//!   leaves this node (`infra-client-requirements.md` §8, design §13).
//! - **The accessing user's view**: the roles a principal holds here,
//!   without the predicates behind them
//!   (`infra-client-requirements.md` §10.7).
//!
//! Where the view is rendered is not settled: a terminal on the host and a
//! page served to the operator alone are both open, and the choice belongs
//! to whoever runs one.  If it grows a frontend it leaves this crate, so
//! its dependencies stay out of the reference library's lockfile.
//!
//! **Everything here is data plus a rendering.** The module decides
//! nothing and consults nothing: it is handed what a node already holds and
//! turns it into lines a person reads, so a mistake here is a misleading
//! screen and never a wrong answer on the wire.  The renderings are plain
//! text, which is the one form that survives a terminal, a log and a page
//! alike.

use rhtn_archive::Keyhash;
use rhtn_node::resources::{Binding, Gateway, Row};
use std::fmt::Write;

/// A keyhash in full, lower-case hex.  It is not abbreviated: the operator
/// reading this is the one who writes keyhashes into a configuration, and a
/// short form is not the value they would paste back.
fn hex(k: &Keyhash) -> String {
    k.iter().map(|b| format!("{b:02x}")).collect()
}

// ------------------------------------------------------------- bindings

/// Where a bound resource runs.
///
/// `infra-client-requirements.md` §10.6 requires this to be shown when a
/// resource is bound, and states that it follows from where the resource
/// runs rather than from anything declared.  It matters because revocation
/// differs: a hosted package's session ends at this node, while an external
/// service continues on its own terms (design §11.2).  Neither is the
/// better answer, and the operator is owed only the knowledge of which one
/// they have.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Hosting {
    /// The node carries the traffic itself, to a backend behind it.
    Hosted,
    /// The node brokers to an external service and holds no backend of its
    /// own for the resource.
    Brokered,
}

/// One resource bound at this node, as the operator is shown it.
///
/// The three facts are the ones `infra-client-requirements.md` §10.6 turns
/// on: which resource, whose it is, and where it runs.  The binding's authority and its declared roles
/// are omitted deliberately; they say what the gateway addresses and what a
/// row may name, neither of which is a question about exposure.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BindingView {
    pub resource: Keyhash,
    pub owner: Keyhash,
    pub hosting: Hosting,
}

impl BindingView {
    /// The view of one binding.
    ///
    /// The hosting model is derived and not declared: a binding with a
    /// local backend is a resource this node runs, and one without is a
    /// resource it brokers.  That is what `infra-client-requirements.md`
    /// §10.6 means by following from where the resource runs, and it is why
    /// there is no field on [`Binding`] to disagree with.
    pub fn of_binding(resource: Keyhash, binding: &Binding) -> BindingView {
        let hosting = if binding.backend.is_some() { Hosting::Hosted } else { Hosting::Brokered };
        BindingView { resource, owner: binding.owner, hosting }
    }

    /// The view of one resource bound at `gateway`, or `None` where the
    /// gateway holds no binding for it.
    ///
    /// **This looks one resource up rather than listing what is bound.**
    /// `rhtn_node::resources::Gateway` publishes a lookup by keyhash and no
    /// iterator over its bindings, so a caller wanting the whole set must
    /// bring the keyhashes from wherever it bound them.  Widening the
    /// gateway's surface for the sake of a display would change the type
    /// that decides requests, and a display is not worth that.
    pub fn of(gateway: &Gateway, resource: Keyhash) -> Option<BindingView> {
        gateway.binding(&resource).map(|b| BindingView::of_binding(resource, b))
    }

    /// The binding as a person reads it.
    ///
    /// The consequence is spelled out beside the model because the model
    /// alone means nothing to a reader who has not read
    /// `infra-client-requirements.md` §10.6.
    pub fn render(&self) -> String {
        let mut s = String::new();
        let _ = writeln!(s, "resource {}", hex(&self.resource));
        let _ = writeln!(s, "  owner  {}", hex(&self.owner));
        match self.hosting {
            Hosting::Hosted => {
                let _ = writeln!(s, "  hosted at this node");
                let _ = writeln!(s, "  a session here ends when this node ends it");
            }
            Hosting::Brokered => {
                let _ = writeln!(s, "  brokered to an external service");
                let _ = writeln!(s, "  a session there continues on the terms of the service the operator chose");
            }
        }
        s
    }
}

// -------------------------------------------------------------- exposure

/// What this node's configuration creates for the identities below it.
///
/// `infra-client-requirements.md` §8 owes an operator a plain statement of
/// what their subordinates are exposed to by their configuration and
/// conduct, and design §13 is where a subnet acquires the shape that makes
/// the question answerable at all.
///
/// **Every field is supplied by the caller.** The view does not read a node
/// to work any of it out, because a second reading of the node's state
/// taken at display time would be a second source of truth for it, and the
/// one on screen is the one nobody checks.  Whoever holds the node counts
/// the subordinates and knows what it is configured to carry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExposureView {
    /// How many identities sit directly below this node.
    pub subordinates: usize,
    /// Whether payload for those identities passes through this node.  It
    /// does wherever the relayed path is the one taken, which for a
    /// substantial minority of connections is the only path there is
    /// (`infra-client-requirements.md` §7).
    pub relays_payload: bool,
    /// Whether this node hosts resources those identities can reach, so
    /// that their requests arrive at a backend this operator runs
    /// (`infra-client-requirements.md` §9, §10.6).
    pub hosts_resources: bool,
    /// Whether this node attaches to a patron.  A node with none is a root
    /// and attaches to nobody.
    pub has_upstream: bool,
}

impl ExposureView {
    /// State the exposure.  The arguments are explicit for the reason given
    /// on the type: this module is told, and does not go looking.
    pub fn new(subordinates: usize, relays_payload: bool, hosts_resources: bool, has_upstream: bool) -> ExposureView {
        ExposureView { subordinates, relays_payload, hosts_resources, has_upstream }
    }

    /// The exposure as a person reads it.
    ///
    /// Each line answers with a yes or a no first, so the shape of the
    /// screen does not change with the answer and a reader can scan the
    /// column.  A no carries no further clause: there is nothing to warn
    /// about in a thing this node does not do.
    pub fn render(&self) -> String {
        let mut s = String::new();
        let _ = writeln!(s, "what this node's configuration exposes the identities below it to");
        let _ = writeln!(s, "  identities directly below  {}", self.subordinates);
        let _ = writeln!(s, "  payload relayed here       {}", match self.relays_payload {
            true => "yes: their payload passes through this node",
            false => "no",
        });
        let _ = writeln!(s, "  resources hosted here      {}", match self.hosts_resources {
            true => "yes: their requests reach a backend this operator runs",
            false => "no",
        });
        let _ = writeln!(s, "  attaches to a patron       {}", match self.has_upstream {
            true => "yes",
            false => "no: this node attaches to nobody",
        });
        s
    }
}

// ----------------------------------------------------------------- roles

/// The roles one principal holds on one resource, as that principal is
/// shown them.
///
/// `infra-client-requirements.md` §10.7 fixes the content: the roles held,
/// and not the predicates that granted them, which are the operator's
/// business and may encode judgments they would rather not publish
/// (`resource-requirements.md` §7.4).  Withholding the roles as well would
/// leave a user unable to tell refused by policy from broken, and unaware
/// that departing a patron cost them access.
///
/// **The predicate is not in reach of this type, which is why it cannot be
/// rendered by accident.** Roles are read from the materialised table, and
/// that table is what authorises a request in any case: authorisation at
/// request time is a lookup and never a predicate evaluation
/// (`infra-client-requirements.md` §10.2, design §11.4).  A row does not
/// record what produced it, so there is nothing here to leak.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RoleView {
    /// The principal the roles are held by.
    pub principal: Keyhash,
    /// The resource they are held on.
    pub resource: Keyhash,
    /// The application roles held, in the order the table holds them.
    pub roles: Vec<String>,
    /// Whether the row grants `connect`.  This is the node's own gate on
    /// reaching a backend rather than an application role, and a user who
    /// is not shown it cannot tell an empty role set from a closed door.
    pub connect: bool,
}

impl RoleView {
    /// The view of one row.
    pub fn of_row(principal: Keyhash, resource: Keyhash, row: &Row) -> RoleView {
        RoleView { principal, resource, roles: row.roles.iter().cloned().collect(), connect: row.connect }
    }

    /// The view of one principal's row for one resource at `gateway`, or
    /// `None` where the gateway holds no row for the pair.  A missing row
    /// is not an empty one: it is the case where nothing was ever assigned.
    pub fn of(gateway: &Gateway, resource: Keyhash, principal: Keyhash) -> Option<RoleView> {
        gateway.row(&resource, &principal).map(|r| RoleView::of_row(principal, resource, r))
    }

    /// The roles as the accessing user reads them.
    ///
    /// An empty role set renders as `none` rather than as a blank, because
    /// the user is owed the difference between holding no roles and being
    /// shown a screen that failed to load.
    pub fn render(&self) -> String {
        let mut s = String::new();
        let _ = writeln!(s, "principal {}", hex(&self.principal));
        let _ = writeln!(s, "  on resource  {}", hex(&self.resource));
        let _ = writeln!(s, "  roles        {}", match self.roles.is_empty() {
            true => "none".to_string(),
            false => self.roles.join(", "),
        });
        let _ = writeln!(s, "  may reach the resource  {}", match self.connect {
            true => "yes",
            false => "no",
        });
        s
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rhtn_node::resources::Backend;
    use std::collections::BTreeSet;
    use std::sync::Arc;

    /// A backend that exists.  Nothing here calls it: what the views read
    /// is whether one is there at all.
    struct Stub;

    impl Backend for Stub {
        fn handle(&self, _message: &[u8]) -> Result<Vec<u8>, String> {
            Ok(Vec::new())
        }
        fn running(&self) -> bool {
            true
        }
    }

    fn roles(names: &[&str]) -> BTreeSet<String> {
        names.iter().map(|r| r.to_string()).collect()
    }

    fn binding(backend: Option<Arc<dyn Backend>>) -> Binding {
        Binding { owner: [2u8; 32], authority: "records.example".to_string(), backend, declared_roles: roles(&["reader", "writer"]) }
    }

    #[test]
    fn hosted_and_brokered_render_differently() {
        let resource = [1u8; 32];
        let hosted = BindingView::of_binding(resource, &binding(Some(Arc::new(Stub))));
        let brokered = BindingView::of_binding(resource, &binding(None));

        assert_eq!(hosted.hosting, Hosting::Hosted);
        assert_eq!(brokered.hosting, Hosting::Brokered);
        assert_ne!(hosted.render(), brokered.render());

        // the model is named in words, not left to be inferred from a
        // field the reader cannot see
        assert!(hosted.render().contains("hosted at this node"), "{}", hosted.render());
        assert!(brokered.render().contains("brokered to an external service"), "{}", brokered.render());

        // and the consequence that `infra-client-requirements.md` §10.6
        // says makes it matter: where the session ends
        assert!(hosted.render().contains("ends when this node ends it"));
        assert!(brokered.render().contains("continues on the terms of the service"));

        // both name the same resource and owner, so the model is the only
        // difference between the two screens
        for view in [&hosted, &brokered] {
            assert!(view.render().contains(&hex(&resource)));
            assert!(view.render().contains(&hex(&[2u8; 32])));
        }
    }

    #[test]
    fn a_binding_view_reads_through_the_gateway() {
        let resource = [3u8; 32];
        let mut gateway = Gateway::default();
        gateway.bind(resource, binding(Some(Arc::new(Stub))));

        assert_eq!(BindingView::of(&gateway, resource).map(|v| v.hosting), Some(Hosting::Hosted));
        assert_eq!(BindingView::of(&gateway, [9u8; 32]), None);
    }

    #[test]
    fn a_role_view_never_renders_a_predicate() {
        // the predicate is the operator's, and lives entirely outside the
        // gateway: it selects members and is spent doing so
        let predicate = "clients above trust rank 10, joined before 2026-01-01";
        let resource = [4u8; 32];
        let principal = [5u8; 32];
        let mut gateway = Gateway::default();
        gateway.bind(resource, binding(None));
        gateway.materialise(resource, &[principal], Row { roles: roles(&["reader"]), connect: true }).unwrap();

        let view = RoleView::of(&gateway, resource, principal).unwrap();
        let out = view.render();

        // what `infra-client-requirements.md` §10.7 says the user is shown
        assert!(out.contains("reader"), "{out}");
        assert!(out.contains(&hex(&resource)), "{out}");

        // and what it says they are not.  The predicate's own text cannot
        // appear because the view never held it, and neither the word nor
        // any of the vocabulary `infra-client-requirements.md` §10.3 gives
        // a predicate is in the rendering.
        assert!(!out.contains(predicate), "{out}");
        for leak in ["predicate", "rank", "percentile", "tier", "joined", "template"] {
            assert!(!out.to_lowercase().contains(leak), "{leak} in: {out}");
        }
    }

    #[test]
    fn an_empty_role_set_is_distinguishable_from_a_closed_door() {
        let (resource, principal) = ([6u8; 32], [7u8; 32]);
        let none = RoleView::of_row(principal, resource, &Row::default());
        let connect_only = RoleView::of_row(principal, resource, &Row { roles: BTreeSet::new(), connect: true });

        assert!(none.render().contains("roles        none"), "{}", none.render());
        assert!(none.render().contains("may reach the resource  no"), "{}", none.render());
        assert!(connect_only.render().contains("may reach the resource  yes"), "{}", connect_only.render());
    }

    #[test]
    fn the_exposure_view_states_every_answer_either_way() {
        let exposed = ExposureView::new(4, true, true, true);
        let bare = ExposureView::new(0, false, false, false);

        assert!(exposed.render().contains("identities directly below  4"), "{}", exposed.render());
        assert!(exposed.render().contains("payload relayed here       yes"), "{}", exposed.render());
        assert!(exposed.render().contains("resources hosted here      yes"), "{}", exposed.render());
        assert!(exposed.render().contains("attaches to a patron       yes"), "{}", exposed.render());

        // a root that carries nothing still answers all four questions
        assert_eq!(bare.render().lines().count(), exposed.render().lines().count());
        assert!(bare.render().contains("attaches to nobody"), "{}", bare.render());
    }
}
