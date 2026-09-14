//! The gateway (`wire-format.md` §11; `resource-requirements.md` §1 to
//! §3; `infra-client-requirements.md` §9, §10): a resource behind the
//! node, reached by a request the node evaluates in the normative order
//! from one snapshot, answered with the one code that step yields, and
//! handed on once as a credential the resource reads.  The role table is
//! materialised per resource and consulted by lookup; a row change ends
//! the hosted session and nothing else.

use crate::Keyhash;
use crate::catalog::CatalogService;
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
    fn handle(&self, message: &[u8]) -> Result<Vec<u8>, String>;
    fn running(&self) -> bool;
}

/// A resource bound at this node: its owner, the authority the backend is
/// addressed by, the backend where the node carries the traffic and none
/// where it brokers, and the roles the package declared.
pub struct Binding {
    pub owner: Keyhash,
    pub authority: String,
    pub backend: Option<Arc<dyn Backend>>,
    pub declared_roles: BTreeSet<String>,
}

/// One member's row for one resource: application roles, and whether
/// `connect` is granted.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Row {
    pub roles: BTreeSet<String>,
    pub connect: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RowError {
    /// Wider than a header could carry: refused at configuration.
    TooWide(usize),
    /// A role the package did not declare.
    Undeclared(String),
    /// A name reserved for the node's own evaluation
    /// (`resource-requirements.md` §3).
    Reserved(String),
    NoSuchResource,
}

/// The gateway's state.
#[derive(Default)]
pub struct Gateway {
    bindings: BTreeMap<Keyhash, Binding>,
    /// Grants standing over the owner's horizon, by resource.
    standing: BTreeMap<Keyhash, Row>,
    rows: BTreeMap<(Keyhash, Keyhash), Row>,
    /// Hosted sessions: the identifier under which a resource sees a
    /// principal, minted per (member, resource).
    sessions: BTreeMap<(Keyhash, Keyhash), [u8; 16]>,
    minted: u64,
    /// Handoffs attempted per resource, for a test to count.
    pub attempts: BTreeMap<Keyhash, u64>,
}

impl Gateway {
    pub fn bind(&mut self, resource: Keyhash, binding: Binding) {
        self.bindings.insert(resource, binding);
    }

    pub fn binding(&self, resource: &Keyhash) -> Option<&Binding> {
        self.bindings.get(resource)
    }

    /// Set a member's row for a resource, refusing one wider than 64 roles,
    /// naming a reserved name, or naming a role the package did not
    /// declare.  A changed row ends the member's hosted session with that
    /// resource.
    pub fn set_row(&mut self, resource: Keyhash, member: Keyhash, row: Row) -> Result<(), RowError> {
        let b = self.bindings.get(&resource).ok_or(RowError::NoSuchResource)?;
        if row.roles.len() > MAX_ROLES {
            return Err(RowError::TooWide(row.roles.len()));
        }
        if let Some(r) = row.roles.iter().find(|r| RESERVED_ROLES.contains(&r.as_str())) {
            return Err(RowError::Reserved(r.clone()));
        }
        if let Some(r) = row.roles.iter().find(|r| !b.declared_roles.contains(*r)) {
            return Err(RowError::Undeclared(r.clone()));
        }
        let changed = self.rows.get(&(resource, member)) != Some(&row);
        self.rows.insert((resource, member), row);
        if changed {
            self.sessions.remove(&(member, resource));
        }
        Ok(())
    }

    /// A predicate expanded into rows (design §11.4): `members` is what
    /// the predicate matched among the owner's Dunbar Org, each given
    /// `row`.  The predicate is the operator's tool; what authorises a
    /// request afterwards is the table alone.
    pub fn materialise(&mut self, resource: Keyhash, members: &[Keyhash], row: Row) -> Result<(), RowError> {
        for m in members {
            self.set_row(resource, *m, row.clone())?;
        }
        Ok(())
    }

    pub fn row(&self, resource: &Keyhash, member: &Keyhash) -> Option<&Row> {
        self.rows.get(&(*resource, *member))
    }

    /// A grant standing over every member of the owner's trust horizon.
    ///
    /// **This is the simplest predicate there is** — the one
    /// `infra-client-requirements.md` §10.1 calls membership being the
    /// outer gate — and it is held rather than evaluated because §10.1
    /// answers *from the row, not by evaluating a predicate*: what a
    /// request reads is a row that was already there.
    pub fn stand(&mut self, resource: Keyhash, row: Row) -> Result<(), RowError> {
        if !self.bindings.contains_key(&resource) {
            return Err(RowError::NoSuchResource);
        }
        if row.roles.len() > MAX_ROLES {
            return Err(RowError::TooWide(row.roles.len()));
        }
        self.standing.insert(resource, row);
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
    pub fn refresh(&mut self, table: &Table) -> (usize, usize) {
        let (mut granted, mut dropped) = (0, 0);
        let standing: Vec<(Keyhash, Row)> = self.standing.iter().map(|(k, r)| (*k, r.clone())).collect();
        for (resource, row) in standing {
            let Some(owner) = self.bindings.get(&resource).map(|b| b.owner) else { continue };
            let members = table.horizon(&owner, 2);
            for m in &members {
                if self.rows.contains_key(&(resource, *m)) {
                    continue;
                }
                if self.set_row(resource, *m, row.clone()).is_ok() {
                    granted += 1;
                }
            }
            // a party gone from the org: §10.2 has a departing one's rows
            // removed, and §10.5 has the row change end the session
            let gone: Vec<Keyhash> = self.rows.keys().filter(|(r, m)| *r == resource && !members.contains(m)).map(|(_, m)| *m).collect();
            for m in gone {
                self.rows.remove(&(resource, m));
                self.sessions.remove(&(m, resource));
                dropped += 1;
            }
        }
        (granted, dropped)
    }

    /// A member gone from the org: every row and session of theirs goes.
    pub fn remove_member(&mut self, member: &Keyhash) {
        self.rows.retain(|(_, m), _| m != member);
        self.sessions.retain(|(m, _), _| m != member);
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
    pub fn serve(&mut self, me: &Keyhash, table: &Table, requester: &Keyhash, body: &[u8]) -> ResourceResponse {
        let Ok(req) = ResourceRequest::decode(body) else { return ResourceResponse::code(STATUS_MALFORMED) };
        let Some(binding) = self.bindings.get(&req.resource) else { return ResourceResponse::code(STATUS_REFUSED) };
        if !table.horizon(&binding.owner, 2).contains(requester) {
            return ResourceResponse::code(STATUS_REFUSED);
        }
        // above the patron level: this host's own acknowledgement
        let below_me = table.downline_contains(me, requester) && !table.subordinates(me).contains(requester) && requester != me;
        if below_me && !table.acks().iter().any(|a| a.node == *requester && a.grandpatron == *me) {
            return ResourceResponse::code(STATUS_NO_ACK);
        }
        let Some(row) = self.rows.get(&(req.resource, *requester)).filter(|r| r.connect).cloned() else { return ResourceResponse::code(STATUS_NO_ROLE) };
        let parsed = match http::parse(&req.message) {
            Ok(p) => p,
            Err(_) => return ResourceResponse::code(STATUS_MALFORMED),
        };
        let Some(backend) = binding.backend.clone() else { return ResourceResponse::code(STATUS_UNAVAILABLE) };
        if !backend.running() {
            return ResourceResponse::code(STATUS_UNAVAILABLE);
        }
        let authority = binding.authority.clone();
        let session = self.session_for(*requester, req.resource);
        let cred = Credential { principal: pairwise_principal(&req.resource, requester), roles: row.roles.iter().cloned().collect(), audience: req.resource, session };
        let message = http::serialise(&parsed, &authority, &cred);
        *self.attempts.entry(req.resource).or_insert(0) += 1;
        match backend.handle(&message) {
            Ok(resp) => ResourceResponse { status: STATUS_DELIVERED, body: Some(resp) },
            // one attempt, never a retry: the requester decides
            Err(_) => ResourceResponse::code(STATUS_UNAVAILABLE),
        }
    }

    /// The catalog page for `viewer` (design §11.5): the entries this node
    /// holds whose resource the viewer holds connect on, each with the
    /// roles the viewer holds, and nothing the viewer cannot use.
    pub fn page(&self, viewer: &Keyhash, catalog: &CatalogService) -> Vec<(Vec<u8>, Vec<String>)> {
        self.rows.iter().filter(|((_, m), r)| m == viewer && r.connect).filter_map(|((res, _), r)| catalog.held(res).map(|h| (h.bytes.clone(), r.roles.iter().cloned().collect()))).collect()
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

/// What a package declares (`resource-requirements.md` §7, §8).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Manifest {
    pub roles: BTreeSet<String>,
    pub imports: Vec<String>,
}

/// A package instantiated against the host's exports.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Package {
    pub roles: BTreeSet<String>,
}

pub fn exports() -> &'static [&'static str] {
    &HOST_EXPORTS
}

/// Instantiate a package: every import must be an export the host has.
pub fn instantiate(m: &Manifest) -> Result<Package, String> {
    if let Some(i) = m.imports.iter().find(|i| !HOST_EXPORTS.contains(&i.as_str())) {
        return Err(format!("no such binding: {i}"));
    }
    if m.roles.iter().any(|r| r.is_empty() || r.len() > 32 || !r.bytes().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == b'_' || c == b'-')) {
        return Err("a role name is [a-z0-9_-], 1 to 32 bytes".into());
    }
    if let Some(r) = m.roles.iter().find(|r| RESERVED_ROLES.contains(&r.as_str())) {
        return Err(format!("{r} is reserved for the node's own evaluation"));
    }
    Ok(Package { roles: m.roles.clone() })
}
