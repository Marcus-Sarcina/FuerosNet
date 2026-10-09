//! The gateway (`wire-format.md` §11; `resource-requirements.md` §1 to
//! §3; `infra-client-requirements.md` §9, §10): a resource behind the
//! node, reached by a request the node evaluates in the normative order
//! from one snapshot, answered with the one code that step yields, and
//! handed on once as a credential the resource reads.  The role table is
//! materialised per resource and consulted by lookup; a row change ends
//! the hosted session and nothing else.

use crate::Keyhash;
use crate::catalog::CatalogService;
use crate::grant::{Grant, GrantError, Standing, Template};
use crate::http::{self, Credential, pairwise_principal};
use rhtn_archive::catalog::*;
use rhtn_archive::topology::Table;
use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

/// A row carries at most this many roles (`resource-requirements.md` §3).
pub const MAX_ROLES: usize = 64;

/// What a hosted package is to the gateway: a place to hand one message
/// and read one back, and whether it is running.
pub trait Backend: Send + Sync {
    /// Answer one request to this resource.
    fn handle(&self, message: &[u8]) -> Result<Vec<u8>, String>;
    /// Whether the backend is up.
    fn running(&self) -> bool;
}

/// A resource bound at this node: its owner, the authority the backend is
/// addressed by, the backend where the node carries the traffic and none
/// where it brokers, and what the package declared.
///
/// **The declarations are the manifest itself** rather than fields copied
/// out of it [2026-10-08]: roles, administrative operations and templates
/// are one object with one set of bounds, checked once at admission, and a
/// binding that held its own copies would be a second place for them to
/// drift. A brokered resource (§10.6, no backend) carries what its
/// operator configured for it in the same shape, since nothing downstream
/// cares which of the two wrote it.
pub struct Binding {
    /// Who owns the resource.
    pub owner: Keyhash,
    /// The authority its backend is addressed by.
    pub authority: String,
    /// The backend where this node carries the traffic; `None` where it
    /// brokers and the traffic goes elsewhere.
    pub backend: Option<Arc<dyn Backend>>,
    /// What the package declared ([`Manifest`]).
    pub declared: Manifest,
}

/// One member's row for one resource: application roles, and whether
/// `connect` is granted.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Row {
    /// The application roles the member holds.
    pub roles: BTreeSet<String>,
    /// Whether `connect` is granted.
    pub connect: bool,
}

/// Why a row could not be made.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RowError {
    /// Wider than a header could carry: refused at configuration.
    TooWide(usize),
    /// A role the package did not declare.
    Undeclared(String),
    /// A name reserved for the node's own evaluation
    /// (`resource-requirements.md` §3).
    Reserved(String),
    /// No resource is bound here under that identity.
    NoSuchResource,
    /// More grants on one resource than the node will hold.
    TooManyGrants(usize),
    /// A grant the node will not hold ([`crate::grant::GrantError`]).
    BadGrant(GrantError),
}

impl std::fmt::Display for RowError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RowError::TooWide(n) => {
                write!(f, "a row carries at most {MAX_ROLES} roles, not {n}")
            }
            RowError::Undeclared(r) => write!(f, "`{r}` is not a role this package declared"),
            RowError::Reserved(r) => {
                write!(f, "`{r}` is reserved for the node's own evaluation")
            }
            RowError::NoSuchResource => write!(f, "no resource is bound under that identity"),
            RowError::TooManyGrants(n) => write!(
                f,
                "at most {} grants on one resource, not {n}",
                crate::grant::MAX_GRANTS
            ),
            RowError::BadGrant(e) => write!(f, "{e}"),
        }
    }
}

/// The gateway's state.
#[derive(Default)]
pub struct Gateway {
    bindings: BTreeMap<Keyhash, Binding>,
    /// The grants an operator wrote, by resource: the predicates whose
    /// expansion writes rows (`infra-client-requirements.md` §10.2).
    grants: BTreeMap<Keyhash, Vec<Grant>>,
    rows: BTreeMap<(Keyhash, Keyhash), Row>,
    /// Rows this gateway wrote by expanding a standing grant, as against
    /// ones the operator set for a named member.
    ///
    /// **A standing grant is a floor under the table, not an overwrite**,
    /// so the two must be told apart: a change to the grant rewrites what
    /// the grant produced and leaves the operator's own entries where they
    /// are.  Without the distinction a grant narrowed to nothing left
    /// every row it had written standing as if each were an individual
    /// decision (`infra-client-requirements.md` §10.2).
    derived: BTreeSet<(Keyhash, Keyhash)>,
    /// Hosted sessions: the identifier under which a resource sees a
    /// principal, minted per (member, resource).
    sessions: BTreeMap<(Keyhash, Keyhash), [u8; 16]>,
    minted: u64,
    /// Handoffs attempted per resource, for a test to count.
    pub attempts: BTreeMap<Keyhash, u64>,
}

impl Gateway {
    /// Bind `resource` at this node.
    pub fn bind(&mut self, resource: Keyhash, binding: Binding) {
        self.bindings.insert(resource, binding);
    }

    /// Every resource bound here, in keyhash order: what an operator's
    /// page lists (`infra-client-requirements.md` §10.6).
    pub fn bound(&self) -> Vec<Keyhash> {
        self.bindings.keys().copied().collect()
    }

    /// The grants written for `resource` (§10.2): the floor under the
    /// table, as against the rows an operator set for a named member.
    pub fn grants_for(&self, resource: &Keyhash) -> &[Grant] {
        self.grants.get(resource).map_or(&[], |g| g.as_slice())
    }

    /// Which of a resource's declared roles some grant reaches, and which
    /// none does: `resource-requirements.md` §7 wants a role list with
    /// "bound/unbound state, so *\"you installed this and have not decided
    /// who may use it\"* is visible rather than a silent default".
    pub fn role_bindings(&self, resource: &Keyhash) -> Vec<(String, bool)> {
        let Some(b) = self.bindings.get(resource) else {
            return Vec::new();
        };
        let granted: BTreeSet<&String> = self
            .grants_for(resource)
            .iter()
            .flat_map(|g| g.roles.iter())
            .collect();
        b.declared
            .roles
            .iter()
            .map(|r| (r.clone(), granted.contains(r)))
            .collect()
    }

    /// **Whom a re-score would consider**: the union of the horizons of
    /// every resource owner bound here, which is the set a rank clause's
    /// line is drawn across.
    pub fn candidates(&self, table: &Table) -> Vec<Keyhash> {
        let mut all = BTreeSet::new();
        for b in self.bindings.values() {
            all.extend(table.horizon(&b.owner, 2));
        }
        all.into_iter().collect()
    }

    /// Whether any grant held here reads a rank, so a caller can skip the
    /// policy where none does. design §16.2 has standing computed on
    /// demand; this says when the demand exists.
    pub fn wants_standing(&self) -> bool {
        self.grants.values().flatten().any(|g| g.reads_rank())
            || self
                .bindings
                .values()
                .flat_map(|b| b.declared.templates.iter())
                .any(|t| t.grant.reads_rank())
    }

    /// **How many members a grant would reach as the horizon is now.**
    ///
    /// `infra-client-requirements.md` §10.4 has a template's population
    /// made explicit, and the operator's page shows this count beside the
    /// template's name so that a one-click grant's reach is legible before
    /// the click rather than after it.
    pub fn population(
        &self,
        resource: &Keyhash,
        grant: &Grant,
        table: &Table,
        standing: &Standing,
    ) -> usize {
        let Some(b) = self.bindings.get(resource) else {
            return 0;
        };
        table
            .horizon(&b.owner, 2)
            .iter()
            .filter(|m| grant.admits(&b.owner, m, table, standing))
            .count()
    }

    /// The binding for `resource`, where it is bound here.
    pub fn binding(&self, resource: &Keyhash) -> Option<&Binding> {
        self.bindings.get(resource)
    }

    /// Set a member's row for a resource, refusing one wider than 64 roles,
    /// naming a reserved name, or naming a role the package did not
    /// declare.  A changed row ends the member's hosted session with that
    /// resource.
    pub fn set_row(
        &mut self,
        resource: Keyhash,
        member: Keyhash,
        row: Row,
    ) -> Result<(), RowError> {
        let b = self
            .bindings
            .get(&resource)
            .ok_or(RowError::NoSuchResource)?;
        if row.roles.len() > MAX_ROLES {
            return Err(RowError::TooWide(row.roles.len()));
        }
        if let Some(r) = row
            .roles
            .iter()
            .find(|r| RESERVED_ROLES.contains(&r.as_str()))
        {
            return Err(RowError::Reserved(r.clone()));
        }
        if let Some(r) = row.roles.iter().find(|r| !b.declared.roles.contains(*r)) {
            return Err(RowError::Undeclared(r.clone()));
        }
        self.derived.remove(&(resource, member));
        self.write_row(resource, member, row);
        Ok(())
    }

    /// The same write, without the checks the operator's path runs and
    /// marked as the grant's: §10.5 has a changed row end the member's
    /// hosted session either way.
    fn write_row(&mut self, resource: Keyhash, member: Keyhash, row: Row) {
        let changed = self.rows.get(&(resource, member)) != Some(&row);
        self.rows.insert((resource, member), row);
        if changed {
            self.sessions.remove(&(member, resource));
        }
    }

    /// A predicate expanded into rows (design §11.4): `members` is what
    /// the predicate matched among the owner's Dunbar Org, each given
    /// `row`.  The predicate is the operator's tool; what authorises a
    /// request afterwards is the table alone.
    pub fn materialise(
        &mut self,
        resource: Keyhash,
        members: &[Keyhash],
        row: Row,
    ) -> Result<(), RowError> {
        for m in members {
            self.set_row(resource, *m, row.clone())?;
        }
        Ok(())
    }

    /// The row `member` holds for `resource`.
    pub fn row(&self, resource: &Keyhash, member: &Keyhash) -> Option<&Row> {
        self.rows.get(&(*resource, *member))
    }

    /// Remove a member's row for a resource: the row, its provenance mark
    /// and the member's hosted session go together, and nothing says what
    /// the row used to be (`infra-client-requirements.md` §10.2: the table
    /// keeps no history of replaced rows).
    pub fn clear_row(&mut self, resource: Keyhash, member: Keyhash) {
        self.rows.remove(&(resource, member));
        self.derived.remove(&(resource, member));
        self.sessions.remove(&(member, resource));
    }

    /// Whether the row for this pair was written by a standing grant
    /// rather than set for the member by name: the one provenance mark the
    /// table keeps, and it exists only while the row does.
    pub fn is_derived(&self, resource: &Keyhash, member: &Keyhash) -> bool {
        self.derived.contains(&(*resource, *member))
    }

    /// Every row held, for an inspection that the table holds the current
    /// rows and nothing else.
    pub fn rows(&self) -> Vec<((Keyhash, Keyhash), Row)> {
        self.rows.iter().map(|(k, r)| (*k, r.clone())).collect()
    }

    /// How many sessions this node hosts.
    pub fn hosted_sessions(&self) -> usize {
        self.sessions.len()
    }

    /// **Write the grants for a resource**, replacing whatever was there.
    ///
    /// A grant is a predicate and its roles ([`crate::grant`]), and it is
    /// *expanded* rather than evaluated: `infra-client-requirements.md`
    /// §10.1 answers a request *from the row, not by evaluating a
    /// predicate*, so what a request reads is a row that was already
    /// there. The empty predicate is the simplest grant there is — §10.1's
    /// outer gate and nothing further.
    ///
    /// **Every grant is checked here**, once, because §10.2 has the
    /// operator hear about a bad one "at configuration time, never a
    /// requester at request time".
    pub fn set_grants(&mut self, resource: Keyhash, grants: Vec<Grant>) -> Result<(), RowError> {
        let Some(b) = self.bindings.get(&resource) else {
            return Err(RowError::NoSuchResource);
        };
        if grants.len() > crate::grant::MAX_GRANTS {
            return Err(RowError::TooManyGrants(grants.len()));
        }
        for g in &grants {
            if let Some(why) = g.refused(&b.declared.roles) {
                return Err(RowError::BadGrant(why));
            }
        }
        // **changing the policy changes the rows it wrote**
        // (`infra-client-requirements.md` §10.2: re-evaluate when an
        // operator is configuring roles).  The rows this grant produced
        // are dropped so the next expansion writes the new grant into
        // them; ending their sessions is §10.5's, and it happens because
        // the row changes rather than in spite of it.  An operator's own
        // row is not the grant's to rewrite.
        if self.grants.get(&resource) != Some(&grants) {
            let stale: Vec<Keyhash> = self
                .derived
                .iter()
                .filter(|(r, _)| *r == resource)
                .map(|(_, m)| *m)
                .collect();
            for m in stale {
                self.derived.remove(&(resource, m));
                self.rows.remove(&(resource, m));
                self.sessions.remove(&(m, resource));
            }
        }
        self.grants.insert(resource, grants);
        Ok(())
    }

    /// Expand every standing grant over the owner's horizon as it is now,
    /// and say how many rows were written and dropped.
    ///
    /// **Called when membership moves, not when a request arrives**
    /// (§10.2's four moments, none of them a request): a new member is
    /// scored against the standing grants and given rows, and a departing
    /// one has theirs removed. A member with a row of their own that the
    /// operator set is left alone — the standing grant is a floor under
    /// the table, not a thing that overwrites it.
    pub fn refresh(&mut self, table: &Table, standing: &Standing) -> (usize, usize) {
        let (granted, dropped) = self.rescore(table, standing);
        tracing::debug!(target: "node", granted, dropped, "node.resource.refreshed");
        (granted, dropped)
    }

    fn rescore(&mut self, table: &Table, standing: &Standing) -> (usize, usize) {
        let (mut granted, mut dropped) = (0, 0);
        let written: Vec<(Keyhash, Vec<Grant>)> =
            self.grants.iter().map(|(k, g)| (*k, g.clone())).collect();
        for (resource, grants) in written {
            let Some(owner) = self.bindings.get(&resource).map(|b| b.owner) else {
                continue;
            };
            for m in table.horizon(&owner, 2) {
                // an operator's own row is the floor's exception and is
                // left alone; a row a grant wrote is the grants' to
                // rewrite, and one already correct costs nothing
                if self.rows.contains_key(&(resource, m)) && !self.derived.contains(&(resource, m))
                {
                    continue;
                }
                // **a member matched by two grants holds both their
                // roles**, which is what makes a second grant the
                // disjunction the language has no operator for
                // ([`crate::grant::Grant`])
                let matched: Vec<&Grant> = grants
                    .iter()
                    .filter(|g| g.admits(&owner, &m, table, standing))
                    .collect();
                if matched.is_empty() {
                    // **a predicate that stopped matching is §10.5's own
                    // example** of an authorisation change, so the row it
                    // wrote goes and the session goes with it
                    if self.derived.remove(&(resource, m)) {
                        self.rows.remove(&(resource, m));
                        self.sessions.remove(&(m, resource));
                        dropped += 1;
                    }
                    continue;
                }
                let row = Row {
                    connect: true,
                    roles: matched
                        .iter()
                        .flat_map(|g| g.roles.iter().cloned())
                        .collect(),
                };
                if self.rows.get(&(resource, m)) != Some(&row) {
                    self.write_row(resource, m, row);
                    self.derived.insert((resource, m));
                    granted += 1;
                }
            }
        }
        // **the purge is over every resource, not only the granted ones**
        // [author, 2026-09-14].  A row an operator set by hand on a
        // resource with no standing grant outlived the membership it was
        // written for, so a departed or disavowed party kept a permission
        // after the tree had let it go.  Membership is the outer gate
        // (`infra-client-requirements.md` §10.1) and the gate is the same
        // whoever wrote the row.
        //
        // **Recursive by construction.** The horizon is a walk over open
        // bindings, so a departure takes the departing party's whole
        // down-line out of it in one step: nothing here enumerates a
        // subtree, and nothing can miss a generation of one.
        let bound: Vec<(Keyhash, Keyhash)> =
            self.bindings.iter().map(|(r, b)| (*r, b.owner)).collect();
        for (resource, owner) in bound {
            let members = table.horizon(&owner, 2);
            // §10.2 has a departing party's rows removed, and §10.5 has
            // the row change end the session
            let gone: Vec<Keyhash> = self
                .rows
                .keys()
                .filter(|(r, m)| *r == resource && !members.contains(m))
                .map(|(_, m)| *m)
                .collect();
            for m in gone {
                self.rows.remove(&(resource, m));
                self.derived.remove(&(resource, m));
                self.sessions.remove(&(m, resource));
                dropped += 1;
            }
        }
        (granted, dropped)
    }

    /// The hosted session identifier for (member, resource), if one is
    /// open.
    pub fn session(&self, member: &Keyhash, resource: &Keyhash) -> Option<[u8; 16]> {
        self.sessions.get(&(*member, *resource)).copied()
    }

    fn session_for(&mut self, member: Keyhash, resource: Keyhash) -> [u8; 16] {
        if let Some(s) = self.sessions.get(&(member, resource)) {
            return *s;
        }
        self.minted += 1;
        let mut pre = b"rhtn/1:hosted-session".to_vec();
        pre.extend_from_slice(&member);
        pre.extend_from_slice(&resource);
        pre.extend_from_slice(&self.minted.to_be_bytes());
        let id: [u8; 16] = rhtn_codec::cose::sha256(&pre)[..16].try_into().unwrap();
        self.sessions.insert((member, resource), id);
        id
    }

    /// Serve one request from `requester`, evaluated in the normative
    /// order from the state read now: decodes, exists, member of the
    /// owner's Dunbar Org, acknowledged where above the patron level, a
    /// row granting connect, a well-formed message, a running backend;
    /// then one handoff.
    pub fn serve(
        &mut self,
        me: &Keyhash,
        table: &Table,
        requester: &Keyhash,
        body: &[u8],
    ) -> ResourceResponse {
        let response = self.evaluate(me, table, requester, body);
        tracing::debug!(
            target: "node",
            requester8 = %crate::diag::id8(requester),
            status = response.status,
            "node.resource"
        );
        response
    }

    fn evaluate(
        &mut self,
        me: &Keyhash,
        table: &Table,
        requester: &Keyhash,
        body: &[u8],
    ) -> ResourceResponse {
        let Ok(req) = ResourceRequest::decode(body) else {
            return ResourceResponse::code(STATUS_MALFORMED);
        };
        let Some(binding) = self.bindings.get(&req.resource) else {
            return ResourceResponse::code(STATUS_REFUSED);
        };
        if !table.horizon(&binding.owner, 2).contains(requester) {
            return ResourceResponse::code(STATUS_REFUSED);
        }
        // above the patron level: this host's own acknowledgement
        let below_me = table.downline_contains(me, requester)
            && !table.subordinates(me).contains(requester)
            && requester != me;
        if below_me
            && !table
                .acks()
                .iter()
                .any(|a| a.node == *requester && a.grandpatron == *me)
        {
            return ResourceResponse::code(STATUS_NO_ACK);
        }
        let Some(row) = self
            .rows
            .get(&(req.resource, *requester))
            .filter(|r| r.connect)
            .cloned()
        else {
            return ResourceResponse::code(STATUS_NO_ROLE);
        };
        let parsed = match http::parse(&req.message) {
            Ok(p) => p,
            Err(_) => return ResourceResponse::code(STATUS_MALFORMED),
        };
        let Some(backend) = binding.backend.clone() else {
            return ResourceResponse::code(STATUS_UNAVAILABLE);
        };
        if !backend.running() {
            return ResourceResponse::code(STATUS_UNAVAILABLE);
        }
        let authority = binding.authority.clone();
        let session = self.session_for(*requester, req.resource);
        let cred = Credential {
            principal: pairwise_principal(&req.resource, requester),
            roles: row.roles.iter().cloned().collect(),
            audience: req.resource,
            session,
        };
        let message = http::serialise(&parsed, &authority, &cred);
        *self.attempts.entry(req.resource).or_insert(0) += 1;
        match backend.handle(&message) {
            Ok(resp) => ResourceResponse {
                status: STATUS_DELIVERED,
                body: Some(resp),
            },
            // one attempt, never a retry: the requester decides.  The
            // sandbox's refusal (exhausted, trapped, oversized) is known
            // here and nowhere later, since the wire carries one code
            Err(refusal) => {
                tracing::debug!(
                    target: "node",
                    requester8 = %crate::diag::id8(requester),
                    resource8 = %crate::diag::id8(&req.resource),
                    refusal = %refusal,
                    "node.resource.refused"
                );
                ResourceResponse::code(STATUS_UNAVAILABLE)
            }
        }
    }

    /// The catalog page for `viewer` (design §11.5): the entries this node
    /// holds whose resource the viewer holds connect on, each with the
    /// roles the viewer holds, and nothing the viewer cannot use.
    pub fn page(&self, viewer: &Keyhash, catalog: &CatalogService) -> Vec<(Vec<u8>, Vec<String>)> {
        self.rows
            .iter()
            .filter(|((_, m), r)| m == viewer && r.connect)
            .filter_map(|((res, _), r)| {
                catalog
                    .held(res)
                    .map(|h| (h.bytes.clone(), r.roles.iter().cloned().collect()))
            })
            .collect()
    }
}

// ------------------------------------------------------------ packages

/// The bindings a host offers a package (`infra-client-requirements.md`
/// §9.2): its credentialled request traffic and nothing else.  There is no
/// binding for topology, liveness, the queue, prekeys or role inputs, so a
/// manifest importing one cannot be instantiated.
pub const HOST_EXPORTS: [&str; 2] = ["rhtn/1:request", "rhtn/1:response"];

/// Reserved for the node's own evaluation and never an application role
/// (`resource-requirements.md` §3, design §11.4).  `connect` is the gate
/// spent getting a request to a backend and `discover` decides a catalog
/// answer; a backend is present for neither decision, so a row carrying
/// either name would put the node's own vocabulary in the credential and
/// a package could claim a grant nobody made.
pub const RESERVED_ROLES: [&str; 2] = ["connect", "discover"];

/// **An administrative operation a package declares**
/// (`resource-requirements.md` §8: "declare what you need in the
/// manifest"), for the node to render on its operator's page.
///
/// **This is the surface an application presents to its host, not the
/// application itself.** A hosted resource is an independent program — a
/// web interface, a business application, a game — and it serves its own
/// users directly, on ports it claims, with the node there to authenticate
/// them and gate their reach. None of that passes through here. What an
/// operation covers is only what the *operator* may ask of the instance
/// they are hosting, which is why the set is structured options, with free
/// text where only free text will do: a name for this instance.
///
/// **So the package describes an operation and the node draws it**, which
/// is the line §8.3 draws — a node "develops and serves its own
/// administration pages". Nothing here is markup, a layout or a script: it
/// is a name, a label, and parameters from a closed set, drawn the same way
/// for every package, so an operator learns one idiom rather than one per
/// package.
///
/// **Access is not declared here.** §7.3 requires an access template to be
/// "expressed in the predicate language, not as opaque configuration", so
/// who may reach a resource stays in the grant section the node renders
/// from predicates. What this covers is a resource's own configuration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Operation {
    /// Its name, which is what an invocation will carry: `[a-z0-9_-]`, as
    /// a role name is.
    pub name: String,
    /// What the operator reads.
    pub label: String,
    /// A sentence under it, or empty.
    pub help: String,
    /// What it takes.
    pub parameters: Vec<Parameter>,
}

/// One of an [`Operation`]'s parameters.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Parameter {
    /// Its name.
    pub name: String,
    /// What the operator reads beside the field.
    pub label: String,
    /// What it holds.
    pub kind: Kind,
}

/// **The closed set of things a parameter may be.**
///
/// Closed because the node renders every one of them: a type it cannot
/// draw is a type a package could declare and an operator could not
/// answer. A bound on each is part of the type rather than a convention,
/// so a manifest that would render an unusable field is refused at
/// admission instead of discovered at the screen.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Kind {
    /// A checkbox.
    Flag,
    /// A whole number between two bounds, inclusive.
    Number {
        /// The least it may be.
        low: i64,
        /// The most.
        high: i64,
    },
    /// Free text, **length-limited** [author, 2026-10-08]: a field a
    /// package can make arbitrarily long is a field that can fill a
    /// request, so the limit is declared and bounded by [`MAX_TEXT`].
    Text {
        /// The most characters it takes.
        max: usize,
    },
    /// One of a fixed list.
    Choice {
        /// What it may be.
        of: Vec<String>,
    },
    /// A keyhash, which the node checks for shape before it is sent.
    Keyhash,
}

/// At most this many operations in one manifest: a tab a person reads
/// rather than a surface they search.
pub const MAX_OPERATIONS: usize = 32;
/// At most this many parameters on one operation.
pub const MAX_PARAMETERS: usize = 16;
/// The longest a declared text field may be, whatever it asks for.
pub const MAX_TEXT: usize = 4096;
/// At most this many literals in a choice.
pub const MAX_CHOICES: usize = 32;
/// The longest a label may be, and a help line.
pub const MAX_LABEL: usize = 80;
/// The longest help line.
pub const MAX_HELP: usize = 240;

/// Whether `name` is the shape a role, an operation or a parameter takes.
pub(crate) fn namelike(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 32
        && name
            .bytes()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == b'_' || c == b'-')
}

impl Operation {
    /// Why this operation cannot be admitted, where it cannot.
    ///
    /// **Checked at admission and not at the screen**: a manifest naming a
    /// field nobody could answer is a manifest a node declines to host,
    /// which `infra-client-requirements.md` §9 makes ordinary capacity
    /// rather than a fault on either side.
    pub fn refused(&self) -> Option<String> {
        if !namelike(&self.name) {
            return Some(format!("`{}` is not [a-z0-9_-], 1 to 32 bytes", self.name));
        }
        if self.label.is_empty() || self.label.len() > MAX_LABEL {
            return Some(format!(
                "`{}`: a label is 1 to {MAX_LABEL} bytes",
                self.name
            ));
        }
        if self.help.len() > MAX_HELP {
            return Some(format!("`{}`: help is at most {MAX_HELP} bytes", self.name));
        }
        if self.parameters.len() > MAX_PARAMETERS {
            return Some(format!(
                "`{}` takes {} parameters, more than {MAX_PARAMETERS}",
                self.name,
                self.parameters.len()
            ));
        }
        let mut seen = BTreeSet::new();
        for p in &self.parameters {
            if !namelike(&p.name) {
                return Some(format!(
                    "`{}`: `{}` is not [a-z0-9_-], 1 to 32 bytes",
                    self.name, p.name
                ));
            }
            if !seen.insert(&p.name) {
                return Some(format!("`{}`: `{}` is named twice", self.name, p.name));
            }
            if p.label.is_empty() || p.label.len() > MAX_LABEL {
                return Some(format!(
                    "`{}`.`{}`: a label is 1 to {MAX_LABEL} bytes",
                    self.name, p.name
                ));
            }
            if let Some(why) = p.kind.refused() {
                return Some(format!("`{}`.`{}`: {why}", self.name, p.name));
            }
        }
        None
    }
}

impl Kind {
    /// Why this parameter cannot be drawn, where it cannot.
    pub fn refused(&self) -> Option<String> {
        match self {
            Kind::Flag | Kind::Keyhash => None,
            Kind::Number { low, high } if low >= high => Some(format!("{low} is not below {high}")),
            Kind::Number { .. } => None,
            Kind::Text { max } if *max == 0 || *max > MAX_TEXT => Some(format!(
                "a text field takes 1 to {MAX_TEXT} characters, not {max}"
            )),
            Kind::Text { .. } => None,
            Kind::Choice { of } if of.is_empty() || of.len() > MAX_CHOICES => Some(format!(
                "a choice is 1 to {MAX_CHOICES} literals, not {}",
                of.len()
            )),
            Kind::Choice { of } => of
                .iter()
                .find(|c| !namelike(c))
                .map(|c| format!("`{c}` is not [a-z0-9_-], 1 to 32 bytes")),
        }
    }
}

/// What a package declares (`resource-requirements.md` §7, §8).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Manifest {
    /// The application roles it declares. **These are what an operator
    /// has to allocate** — §7: "the package says what roles exist; the
    /// operator binds predicates to them", so a package cannot invent one
    /// after installation and an operator cannot grant one the package
    /// does not understand.
    pub roles: BTreeSet<String>,
    /// The host exports it imports.
    pub imports: Vec<String>,
    /// The administrative operations it offers its host's operator
    /// ([`Operation`]).
    pub admin: Vec<Operation>,
    /// The grants it ships ready-made
    /// ([`crate::grant::Template`]), which an operator may take or ignore.
    pub templates: Vec<Template>,
}

/// A package instantiated against the host's exports.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Package {
    /// The roles the instantiated package holds.
    pub roles: BTreeSet<String>,
}

/// What the host exports to a package.
pub fn exports() -> &'static [&'static str] {
    &HOST_EXPORTS
}

/// Instantiate a package: every import must be an export the host has.
pub fn instantiate(m: &Manifest) -> Result<Package, String> {
    if let Some(i) = m
        .imports
        .iter()
        .find(|i| !HOST_EXPORTS.contains(&i.as_str()))
    {
        return Err(format!("no such binding: {i}"));
    }
    if m.admin.len() > MAX_OPERATIONS {
        return Err(format!(
            "a manifest declares at most {MAX_OPERATIONS} administrative operations, not {}",
            m.admin.len()
        ));
    }
    if m.templates.len() > crate::grant::MAX_TEMPLATES {
        return Err(format!(
            "a manifest declares at most {} templates, not {}",
            crate::grant::MAX_TEMPLATES,
            m.templates.len()
        ));
    }
    let mut named = BTreeSet::new();
    for op in &m.admin {
        if let Some(why) = op.refused() {
            return Err(why);
        }
        if !named.insert(&op.name) {
            return Err(format!("`{}` is declared twice", op.name));
        }
    }
    // **a row could not encode them all** (`resource-requirements.md` §3:
    // at most 64 application roles per resource, and a node "refuses to
    // materialise a row it could not encode").  Asked here, the operator
    // hears about it at installation rather than at configuration, which
    // is earlier still
    if m.roles.len() > MAX_ROLES {
        return Err(format!(
            "a package declares at most {MAX_ROLES} roles, not {}",
            m.roles.len()
        ));
    }
    if m.roles.iter().any(|r| {
        r.is_empty()
            || r.len() > 32
            || !r
                .bytes()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == b'_' || c == b'-')
    }) {
        return Err("a role name is [a-z0-9_-], 1 to 32 bytes".into());
    }
    if let Some(r) = m
        .roles
        .iter()
        .find(|r| RESERVED_ROLES.contains(&r.as_str()))
    {
        return Err(format!("{r} is reserved for the node's own evaluation"));
    }
    let mut named = BTreeSet::new();
    for t in &m.templates {
        if let Some(why) = t.refused(&m.roles) {
            return Err(why);
        }
        if !named.insert(&t.name) {
            return Err(format!("`{}` is declared twice", t.name));
        }
    }
    Ok(Package {
        roles: m.roles.clone(),
    })
}
