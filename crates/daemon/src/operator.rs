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
/// on: which resource, whose it is, and where it runs.  The binding's
/// authority and its declared roles are omitted deliberately; they say what
/// the gateway addresses and what a row may name, neither of which is a
/// question about exposure.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BindingView {
    /// Which resource.
    pub resource: Keyhash,
    /// Whose it is.
    pub owner: Keyhash,
    /// Where it runs.
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
        let hosting = if binding.backend.is_some() {
            Hosting::Hosted
        } else {
            Hosting::Brokered
        };
        BindingView {
            resource,
            owner: binding.owner,
            hosting,
        }
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
        gateway
            .binding(&resource)
            .map(|b| BindingView::of_binding(resource, b))
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
                let _ = writeln!(
                    s,
                    "  a session there continues on the terms of the service the operator chose"
                );
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
    pub fn new(
        subordinates: usize,
        relays_payload: bool,
        hosts_resources: bool,
        has_upstream: bool,
    ) -> ExposureView {
        ExposureView {
            subordinates,
            relays_payload,
            hosts_resources,
            has_upstream,
        }
    }

    /// The exposure as a person reads it.
    ///
    /// Each line answers with a yes or a no first, so the shape of the
    /// screen does not change with the answer and a reader can scan the
    /// column.  A no carries no further clause: there is nothing to warn
    /// about in a thing this node does not do.
    pub fn render(&self) -> String {
        let mut s = String::new();
        let _ = writeln!(
            s,
            "what this node's configuration exposes the identities below it to"
        );
        let _ = writeln!(s, "  identities directly below  {}", self.subordinates);
        let _ = writeln!(
            s,
            "  payload relayed here       {}",
            match self.relays_payload {
                true => "yes: their payload passes through this node",
                false => "no",
            }
        );
        let _ = writeln!(
            s,
            "  resources hosted here      {}",
            match self.hosts_resources {
                true => "yes: their requests reach a backend this operator runs",
                false => "no",
            }
        );
        let _ = writeln!(
            s,
            "  attaches to a patron       {}",
            match self.has_upstream {
                true => "yes",
                false => "no: this node attaches to nobody",
            }
        );
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
        RoleView {
            principal,
            resource,
            roles: row.roles.iter().cloned().collect(),
            connect: row.connect,
        }
    }

    /// The view of one principal's row for one resource at `gateway`, or
    /// `None` where the gateway holds no row for the pair.  A missing row
    /// is not an empty one: it is the case where nothing was ever assigned.
    pub fn of(gateway: &Gateway, resource: Keyhash, principal: Keyhash) -> Option<RoleView> {
        gateway
            .row(&resource, &principal)
            .map(|r| RoleView::of_row(principal, resource, r))
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
        let _ = writeln!(
            s,
            "  roles        {}",
            match self.roles.is_empty() {
                true => "none".to_string(),
                false => self.roles.join(", "),
            }
        );
        let _ = writeln!(
            s,
            "  may reach the resource  {}",
            match self.connect {
                true => "yes",
                false => "no",
            }
        );
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
        Binding {
            owner: [2u8; 32],
            authority: "records.example".to_string(),
            backend,
            declared_roles: roles(&["reader", "writer"]),
            admin: Vec::new(),
        }
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
        assert!(
            hosted.render().contains("hosted at this node"),
            "{}",
            hosted.render()
        );
        assert!(
            brokered
                .render()
                .contains("brokered to an external service"),
            "{}",
            brokered.render()
        );

        // and the consequence that `infra-client-requirements.md` §10.6
        // says makes it matter: where the session ends
        assert!(hosted.render().contains("ends when this node ends it"));
        assert!(
            brokered
                .render()
                .contains("continues on the terms of the service")
        );

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

        assert_eq!(
            BindingView::of(&gateway, resource).map(|v| v.hosting),
            Some(Hosting::Hosted)
        );
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
        gateway
            .materialise(
                resource,
                &[principal],
                Row {
                    roles: roles(&["reader"]),
                    connect: true,
                },
            )
            .unwrap();

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
        for leak in [
            "predicate",
            "rank",
            "percentile",
            "tier",
            "joined",
            "template",
        ] {
            assert!(!out.to_lowercase().contains(leak), "{leak} in: {out}");
        }
    }

    #[test]
    fn an_empty_role_set_is_distinguishable_from_a_closed_door() {
        let (resource, principal) = ([6u8; 32], [7u8; 32]);
        let none = RoleView::of_row(principal, resource, &Row::default());
        let connect_only = RoleView::of_row(
            principal,
            resource,
            &Row {
                roles: BTreeSet::new(),
                connect: true,
            },
        );

        assert!(
            none.render().contains("roles        none"),
            "{}",
            none.render()
        );
        assert!(
            none.render().contains("may reach the resource  no"),
            "{}",
            none.render()
        );
        assert!(
            connect_only
                .render()
                .contains("may reach the resource  yes"),
            "{}",
            connect_only.render()
        );
    }

    #[test]
    fn the_exposure_view_states_every_answer_either_way() {
        let exposed = ExposureView::new(4, true, true, true);
        let bare = ExposureView::new(0, false, false, false);

        assert!(
            exposed.render().contains("identities directly below  4"),
            "{}",
            exposed.render()
        );
        assert!(
            exposed.render().contains("payload relayed here       yes"),
            "{}",
            exposed.render()
        );
        assert!(
            exposed.render().contains("resources hosted here      yes"),
            "{}",
            exposed.render()
        );
        assert!(
            exposed.render().contains("attaches to a patron       yes"),
            "{}",
            exposed.render()
        );

        // a root that carries nothing still answers all four questions
        assert_eq!(
            bare.render().lines().count(),
            exposed.render().lines().count()
        );
        assert!(
            bare.render().contains("attaches to nobody"),
            "{}",
            bare.render()
        );
    }
}

// ---------------------------------------------------------------- status

/// **What this node is, holding and attached to**, as a page or a line at a
/// time (`infra-client-requirements.md` §8.1: "what it is shown is state,
/// who is attached, what is queued, what is held").
///
/// **Every field is a count, a keyhash or a time**, which is §8.1's other
/// half: an interface handed frames would be a second parser where the
/// first one already works. Nothing here is a control, and the page says so
/// — an operator's interface "reads and never speaks for the node".
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StatusView {
    /// The identity it serves under.
    pub keyhash: Keyhash,
    /// The subnet it answers in.
    pub anchor: Keyhash,
    /// Its patron, or none at a root.
    pub patron: Option<Keyhash>,
    /// Whether it runs on a delegated credential and holds no seed
    /// (design §23.3).
    pub delegated: bool,
    /// How many credentials of its run are still ahead of now, and when
    /// the run ends: an instance's, and none for a node holding its seed.
    pub credentials: Option<usize>,
    /// When the run's last credential expires.
    pub run_end: Option<u64>,
    /// Who holds a session with it, the patron included: §8.1's "who is
    /// attached".
    pub attached: Vec<Keyhash>,
    /// How many identities sit directly beneath it.
    pub subordinates: usize,
    /// Topology objects held.
    pub topology_objects: usize,
    /// Subtree acknowledgements among them.
    pub acks_held: usize,
    /// Records in its own archive.
    pub archive_records: usize,
    /// The sequence number of the endpoint record it is serving, where its
    /// operator has signed one (`infra-client-requirements.md` §4.4).
    pub endpoint_seqno: Option<u32>,
    /// The same for its anchor entry.
    pub anchor_seqno: Option<u32>,
    /// Where it serves.
    pub listen: String,
    /// Its own clock, so an operator can see a skew for themselves.
    pub now: u64,
    /// The standing acknowledgement policy (design §11.2.1), which is an
    /// act an operator takes and so is shown beside its control.
    pub acknowledging: bool,
}

impl StatusView {
    /// **Read it off a running node**, under one lock each and nowhere
    /// twice: a page showing a count from one instant beside a count from
    /// another describes a state that never existed, which is the mistake
    /// `infra-client-requirements.md` §10.1 names for authorisation and is
    /// no better here.
    pub fn of(
        node: &rhtn_node::runtime::LiveNode,
        credential: Option<&std::sync::Arc<rhtn_transport::tls::Credential>>,
        endpoint_record: Option<&std::path::Path>,
        anchor_entry: Option<&std::path::Path>,
    ) -> StatusView {
        let attached = {
            use rhtn_node::Adjacency;
            node.adjacency.peers()
        };
        let view = node.view.lock().unwrap();
        let me = view.me();
        let (credentials, run_end) = match credential {
            None => (None, None),
            Some(cred) => {
                let now = view.now();
                (
                    Some(cred.issued().iter().filter(|i| i.not_after > now).count()),
                    cred.run_end(),
                )
            }
        };
        StatusView {
            keyhash: me,
            anchor: view.anchor(),
            patron: view.patron(),
            delegated: credential.is_some(),
            credentials,
            run_end,
            attached,
            subordinates: view.table.subordinates(&me).len(),
            topology_objects: view.store.len(),
            acks_held: view.store.acks_held(),
            archive_records: view.archive.len(),
            endpoint_seqno: endpoint_record.and_then(|p| {
                let b = std::fs::read(p).ok()?;
                rhtn_node::store::EndpointRecord::parse(&b)
                    .ok()
                    .map(|r| r.seqno.counter)
            }),
            anchor_seqno: anchor_entry.and_then(|p| {
                let b = std::fs::read(p).ok()?;
                rhtn_node::resolution::AnchorEntry::parse(&b)
                    .ok()
                    .map(|e| e.seqno.counter)
            }),
            listen: node.addr.to_string(),
            now: view.now(),
            acknowledging: view.ack_policy.is_some(),
        }
    }

    /// The status as lines, which is what survives a terminal and a log
    /// alike — the same form the other views take.
    pub fn render(&self) -> String {
        let mut out = String::new();
        let _ = writeln!(out, "serving as   {}", hex(&self.keyhash));
        let _ = writeln!(out, "in subnet    {}", hex(&self.anchor));
        let _ = writeln!(
            out,
            "patron       {}",
            match &self.patron {
                Some(p) => hex(p),
                None => "none: this node is a root".into(),
            }
        );
        let _ = writeln!(
            out,
            "runs on      {}",
            if self.delegated {
                "a credential its operator's client signed; no seed here (design §23.3)"
            } else {
                "its own seed"
            }
        );
        if let Some(n) = self.credentials {
            let _ = writeln!(
                out,
                "run          {n} credential(s) ahead of now, ending {}",
                self.run_end.unwrap_or(0)
            );
        }
        let _ = writeln!(out, "listening on {}", self.listen);
        let _ = writeln!(out, "clock        {}", self.now);
        let _ = writeln!(out, "attached     {}", self.attached.len());
        for a in &self.attached {
            let _ = writeln!(out, "             {}", hex(a));
        }
        let _ = writeln!(out, "subordinates {}", self.subordinates);
        let _ = writeln!(
            out,
            "acknowledges {}",
            if self.acknowledging {
                "yes: every adoption beneath it, as it is stored"
            } else {
                "no"
            }
        );
        let _ = writeln!(
            out,
            "holding      {} topology object(s), {} acknowledgement(s), {} record(s)",
            self.topology_objects, self.acks_held, self.archive_records
        );
        let _ = writeln!(
            out,
            "endpoints    {}",
            match self.endpoint_seqno {
                Some(s) => format!("an operator-signed record at seqno {s}"),
                None => "none signed: nothing has told your horizon where to reach you".into(),
            }
        );
        let _ = writeln!(
            out,
            "anchor entry {}",
            match self.anchor_seqno {
                Some(s) => format!("operator-signed at seqno {s}"),
                None => "none signed".into(),
            }
        );
        out
    }
}

/// **The administration page a node serves for its own operator**
/// (`infra-client-requirements.md` §8.3): "a node develops and serves its
/// own administration pages; a client provides the frame they are presented
/// in".
///
/// **Why the node serves it rather than the client drawing it.** §8.3:
/// third-party implementations of both roles are expected, and a surface
/// agreed between them would have to be defined universally — constraining
/// what either may build and fixing behaviour with no reason to be common.
/// Serving its own, each implementation administers itself with a surface
/// that matches its software.
///
/// **What this page may and may not carry** [corrected, 2026-10-08]. An
/// earlier note here read §8.1's "reads and never speaks for the node" as
/// forbidding any control at all. It does not: its own gloss is that the
/// interface "does not compose, sign or send anything **on the wire**",
/// OPS-011 names the prohibition exactly — "arbitrary frame composition,
/// traffic replay, signature creation or manual packet approval controls" —
/// and OPS-012 has an operator's hosting, predicates and standing policies
/// kept as **explicit management acts**, which presupposes acts. A control
/// within the node's own authority is one of those; a packet workbench is
/// what §8.1 and OPS-011 forbid.
///
/// **This page carries no control yet** because none is built, not because
/// none may be. What can never be one is an act needing the operator's
/// *seed*: the endpoint record and the anchor entry carry their signature
/// and not the delegation's (§4.4), and the instance does not hold that key
/// (design §23.3) — those the page can only ask the client for.
///
/// The client that presents this does so in a frame isolated from its keys,
/// archive and sealed captures (§8.3), because a seized node serving a
/// hostile page must reach nothing on the device that still holds the seed
/// (design §18.1, §23.3) — **that isolation is the client's to provide and
/// nothing here can assert it**.
pub fn page(
    status: &StatusView,
    exposure: &ExposureView,
    resources: &[ResourceView],
    showing: Option<&Keyhash>,
) -> String {
    let mut out = String::new();
    let _ = write!(
        out,
        "<!DOCTYPE html>\n<html lang=\"en\"><head><meta charset=\"utf-8\">\n\
         <meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\n\
         <title>{} · rhtnd</title>\n<style>\n\
         :root {{ color-scheme: light dark }}\n\
         body {{ font: 15px/1.5 system-ui, sans-serif; margin: 0; padding: 1.5rem;\n\
                max-width: 48rem }}\n\
         h1 {{ font-size: 1.1rem; margin: 0 0 .25rem }}\n\
         h2 {{ font-size: .95rem; margin: 1.5rem 0 .4rem }}\n\
         p.sub {{ margin: 0 0 1rem; opacity: .7 }}\n\
         pre {{ margin: 0; padding: .6rem .8rem; overflow-x: auto;\n\
               background: rgba(127,127,127,.12); border-radius: 4px;\n\
               font: 13px/1.5 ui-monospace, monospace; white-space: pre-wrap;\n\
               word-break: break-all }}\n\
         footer {{ margin-top: 2rem; opacity: .7; font-size: .85rem }}\n\
         </style></head><body>\n",
        &hex(&status.keyhash)[..16]
    );
    let _ = write!(
        out,
        "<h1>{}</h1>\n<p class=\"sub\">{}</p>\n",
        &hex(&status.keyhash)[..16],
        if status.delegated {
            "an instance: it runs on a credential its operator's client signed, and holds no seed"
        } else {
            "a node holding its own seed"
        }
    );
    let _ = write!(out, "{}", tabs(resources, showing));
    match showing.and_then(|k| resources.iter().find(|r| r.resource == *k)) {
        Some(r) => {
            let _ = write!(out, "{}", r.render_html());
        }
        None => {
            let _ = write!(
                out,
                "<h2>State</h2>\n<pre>{}</pre>\n",
                escape(&status.render())
            );
            let _ = write!(
                out,
                "<h2>What this exposes the identities below it to</h2>\n<pre>{}</pre>\n",
                escape(&exposure.render())
            );
            let _ = write!(out, "{}", owed(status));
            let _ = write!(out, "{}", node_controls(status));
        }
    }
    let _ = write!(
        out,
        "<footer>This page shows state and carries no control yet. What it will never carry is \
         an act needing the operator&#39;s key — the endpoint record and the anchor entry are \
         signed by that key and this instance does not hold it \
         (<code>infra-client-requirements.md</code> §4.4, design §23.3) — nor a way to compose \
         or replay traffic (OPS-011).</footer>\n</body></html>\n"
    );
    out
}

/// **One resource bound here**, as its tab shows it
/// (`infra-client-requirements.md` §10.6, §10.7).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResourceView {
    /// The resource's identity.
    pub resource: Keyhash,
    /// Who owns it.
    pub owner: Keyhash,
    /// The authority its backend is addressed by.
    pub authority: String,
    /// Whether this node runs the backend or brokers to one elsewhere:
    /// §10.6 has the two shown apart, since they expose a subordinate to
    /// different parties.
    pub hosted: bool,
    /// The roles the package declared.
    pub declared_roles: Vec<String>,
    /// The standing grant over the owner's horizon, where one is set.
    pub standing: Option<Vec<String>>,
    /// Each member with a row here, the roles it holds, and whether the
    /// row came from the standing grant rather than from the operator.
    pub rows: Vec<(Keyhash, Vec<String>, bool)>,
    /// Hosted sessions open against it.
    pub sessions: usize,
    /// What its package declared it can be asked to do
    /// (`rhtn_node::resources::Operation`).
    pub admin: Vec<rhtn_node::resources::Operation>,
}

impl ResourceView {
    /// The tab's body.
    fn render_html(&self) -> String {
        let mut out = String::new();
        let _ = write!(
            out,
            "<h2>{}</h2>\n<pre>resource  {}\nowner     {}\nhosting   {}\nsessions  {}\nroles     {}</pre>\n",
            escape(&self.authority),
            hex(&self.resource),
            hex(&self.owner),
            if self.hosted {
                "this node runs the backend"
            } else {
                "brokered: the traffic goes to a backend elsewhere"
            },
            self.sessions,
            escape(&self.declared_roles.join(", "))
        );
        let _ = write!(
            out,
            "<h2>Who may reach it</h2>\n<pre>standing  {}</pre>\n",
            match &self.standing {
                Some(r) if r.is_empty() => "a grant with no roles".to_string(),
                Some(r) => escape(&r.join(", ")),
                None => "no standing grant over the owner&#39;s horizon".to_string(),
            }
        );
        if self.rows.is_empty() {
            out.push_str("<pre>no member holds a row here</pre>\n");
        } else {
            let mut body = String::new();
            for (member, roles, derived) in &self.rows {
                let _ = writeln!(
                    body,
                    "{}  {}{}",
                    hex(member),
                    roles.join(", "),
                    if *derived {
                        "   (from the standing grant)"
                    } else {
                        ""
                    }
                );
            }
            let _ = writeln!(out, "<pre>{}</pre>", escape(&body));
        }
        out.push_str(&self.render_admin());
        out.push_str(
            "<p class=\"sub\">Editing a grant is not here yet. \
             <code>resource-requirements.md</code> §7.3 has an access template expressed in the \
             predicate language rather than as opaque configuration, so that what a one-click \
             grant means is legible in the same click; the vocabulary that section fixes is what \
             this section waits on.</p>\n",
        );
        out
    }
}

impl ResourceView {
    /// **What the package declared it can be asked to do**, drawn in the
    /// node's own vocabulary and not the package's
    /// (`infra-client-requirements.md` §8.3, `resource-requirements.md`
    /// §7.3): the manifest names operations and typed parameters, and
    /// every one of them is rendered the same way here, so an operator
    /// learns one idiom rather than one per package.
    fn render_admin(&self) -> String {
        use rhtn_node::resources::Kind;
        if self.admin.is_empty() {
            return "<h2>Acts</h2>\n<p class=\"sub\">This package declares none.</p>\n".into();
        }
        let mut out = String::from("<h2>Acts</h2>\n");
        for op in &self.admin {
            let _ = writeln!(out, "<h3>{}</h3>", escape(&op.label));
            if !op.help.is_empty() {
                let _ = writeln!(out, "<p class=\"sub\">{}</p>", escape(&op.help));
            }
            let mut says = String::new();
            for p in &op.parameters {
                let what = match &p.kind {
                    Kind::Flag => "on or off".to_string(),
                    Kind::Number { low, high } => format!("a number, {low} to {high}"),
                    Kind::Text { max } => format!("text, at most {max} characters"),
                    Kind::Choice { of } => format!("one of: {}", of.join(", ")),
                    Kind::Keyhash => "a keyhash".to_string(),
                };
                let _ = writeln!(says, "{}  {}  ({what})", p.name, p.label);
            }
            if says.is_empty() {
                says.push_str("takes nothing\n");
            }
            let _ = writeln!(out, "<pre>{}</pre>", escape(&says));
        }
        out.push_str(
            "<p class=\"sub\">Invoking one is not here yet: a reserved role has to be added \
             first, since a package could otherwise declare the same name as an application \
             role and a member granted it would be indistinguishable from the operator \
             (<code>rhtn_node::resources::RESERVED_ROLES</code>).</p>\n",
        );
        out
    }
}

/// The tab strip: the node itself, then one per resource bound here.
fn tabs(resources: &[ResourceView], showing: Option<&Keyhash>) -> String {
    let mut out = String::from("<nav>");
    let on = |yes: bool| if yes { " class=\"on\"" } else { "" };
    let _ = write!(out, "<a href=\"/\"{}>The node</a>", on(showing.is_none()));
    for r in resources {
        let _ = write!(
            out,
            "<a href=\"/?resource={}\"{}>{}</a>",
            hex(&r.resource),
            on(showing == Some(&r.resource)),
            escape(&r.authority)
        );
    }
    out.push_str("</nav>\n");
    out
}

/// **What this node cannot do for itself** (`infra-client-requirements.md`
/// §4.4): the two records carry its operator's signature and not its
/// delegation's, so where one is missing the page says so rather than
/// offering a control that could not work.
fn owed(status: &StatusView) -> String {
    let mut want = Vec::new();
    if status.endpoint_seqno.is_none() {
        want.push(
            "an <strong>endpoint record</strong>, without which nothing has told your horizon \
             where to reach this node",
        );
    }
    if status.anchor_seqno.is_none() {
        want.push("an <strong>anchor entry</strong>");
    }
    if want.is_empty() {
        return String::new();
    }
    format!(
        "<h2>Owed by your client</h2>\n<p class=\"sub\">This node is waiting for {}. The \
         signature on each is yours and not this instance&#39;s \
         (<code>infra-client-requirements.md</code> §4.4), so your client signs them and PUTs \
         them to <code>/node/endpoint-record</code> and <code>/node/anchor-entry</code>. The key \
         never comes here.</p>\n",
        want.join(" and ")
    )
}

/// **The acts this node can take on its own authority.**
///
/// §8.1 forbids the interface composing, signing or sending on the wire,
/// and OPS-011 forbids frame composition, traffic replay, signature
/// creation and manual packet approval. A management act is none of those,
/// and OPS-012 has an operator's hosting and standing policies kept as
/// exactly such acts.
///
/// **What authorises one is reaching this port** [author, 2026-10-08].
/// §8.2 puts administration "out of band, with everything else about the
/// host", so where the surface binds is the access decision — and these
/// carry no key, which is what lets an operator administer from a desktop
/// holding no seed (design §23.3).
fn node_controls(status: &StatusView) -> String {
    format!(
        "<h2>Acts</h2>\n\
         <form method=\"post\" action=\"/node/acknowledge\">\n\
         <p>Standing acknowledgement is <strong>{}</strong>: whether this node countersigns \
         every adoption beneath it as it is stored (design §11.2.1).<br>\
         <button name=\"acknowledge\" value=\"{}\">{}</button></p>\n</form>\n\
         <form method=\"post\" action=\"/node/reload\">\n\
         <p>Re-read the hosting file, without a restart.<br><button>Reload</button></p>\n\
         </form>\n\
         <form method=\"post\" action=\"/node/stop\">\n\
         <p>Stop, writing back what is held and losing nothing in flight: the same path a \
         SIGTERM takes.<br><button>Stop this node</button></p>\n</form>\n",
        if status.acknowledging { "on" } else { "off" },
        if status.acknowledging { "off" } else { "on" },
        if status.acknowledging {
            "Turn it off"
        } else {
            "Turn it on"
        },
    )
}

/// The five characters that would otherwise close a tag or an attribute.
/// **Everything on the page goes through this**, counts included: a keyhash
/// cannot carry one of them today, and a renderer that relies on what its
/// inputs happen to contain is one new field away from being wrong.
fn escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            _ => out.push(c),
        }
    }
    out
}

/// **The page an instance serves before it is enrolled**: what it is
/// waiting for, and the public half an operator's client signs over.
///
/// An operator who opens the surface on a fresh instance should read why it
/// is not serving rather than an empty table, and the one thing they need
/// from it is the key — so the key is the page.
pub fn waiting_page(
    transport_public: &str,
    credentials: usize,
    endpoint_record: &str,
    anchor_entry: &str,
) -> String {
    format!(
        "<!DOCTYPE html>\n<html lang=\"en\"><head><meta charset=\"utf-8\">\n\
         <meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\n\
         <title>waiting for a run · rhtnd</title>\n<style>\n\
         :root {{ color-scheme: light dark }}\n\
         body {{ font: 15px/1.5 system-ui, sans-serif; margin: 0; padding: 1.5rem;\n\
                max-width: 48rem }}\n\
         h1 {{ font-size: 1.1rem; margin: 0 0 .25rem }}\n\
         p.sub {{ margin: 0 0 1rem; opacity: .7 }}\n\
         pre {{ margin: 0; padding: .6rem .8rem; overflow-x: auto;\n\
               background: rgba(127,127,127,.12); border-radius: 4px;\n\
               font: 13px/1.5 ui-monospace, monospace; white-space: pre-wrap;\n\
               word-break: break-all }}\n\
         footer {{ margin-top: 2rem; opacity: .7; font-size: .85rem }}\n\
         </style></head><body>\n\
         <h1>Not serving yet</h1>\n\
         <p class=\"sub\">This instance has minted a transport key and is waiting for the run \
         its operator&#39;s client signs over it.</p>\n\
         <pre>transport key   {}\n\
         credentials     {credentials}\n\
         endpoint record {}\n\
         anchor entry    {}</pre>\n\
         <footer>Sign a run over that key and PUT it to <code>/node/run</code>; the two records \
         go to <code>/node/endpoint-record</code> and <code>/node/anchor-entry</code> first, \
         because taking the run is what ends this wait \
         (<code>infra-client-requirements.md</code> §4.4, §7).</footer>\n\
         </body></html>\n",
        escape(transport_public),
        escape(endpoint_record),
        escape(anchor_entry),
    )
}
