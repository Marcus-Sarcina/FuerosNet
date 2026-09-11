//! The catalog a hosting node answers from (`wire-format.md` §6; design
//! §11.5; `infra-client-requirements.md` §11): entries registered over
//! their owners' own sessions and held one per resource keyhash, served
//! byte-for-byte to askers inside the horizon whom the entry's discover
//! scope admits, in a total order with a continuation; and abuse reports,
//! stored for the owner and carried nowhere.

use crate::Keyhash;
use rhtn_archive::catalog::*;
use rhtn_archive::topology::Table;
use rhtn_codec::bounds::CATALOG_REPLY_ENTRIES;
use rhtn_crypto::verify::Lookup;
use std::collections::BTreeMap;

/// Whether a scope relative to `owner` admits `asker`, computed over the
/// topology the evaluator holds (design §11.4): no scope reaches outside
/// the owner's Dunbar Org, and one the evaluator cannot compute admits
/// nobody.
pub trait ScopeEval {
    fn admits(&self, scope: &Scope, owner: &Keyhash, asker: &Keyhash) -> bool;
    /// Whether `asker` is inside this node's own horizon.
    fn in_horizon(&self, asker: &Keyhash) -> bool;
}

/// The table's view: positions relative to the owner as the table holds
/// them.
pub struct TableScopes<'a> {
    pub table: &'a Table,
    pub me: Keyhash,
}

impl ScopeEval for TableScopes<'_> {
    fn admits(&self, scope: &Scope, owner: &Keyhash, asker: &Keyhash) -> bool {
        let t = self.table;
        let dunbar = t.horizon(owner, 2);
        match scope {
            Scope::Own => asker == owner,
            Scope::Down(n) => {
                let mut layer = vec![*owner];
                for _ in 0..*n {
                    let next: Vec<Keyhash> = layer.iter().flat_map(|p| t.subordinates(p)).collect();
                    if next.contains(asker) {
                        return true;
                    }
                    layer = next;
                }
                false
            }
            Scope::Up(n) => {
                let mut layer = vec![*owner];
                for _ in 0..*n {
                    let next: Vec<Keyhash> = layer.iter().flat_map(|p| t.patrons(p)).collect();
                    if next.contains(asker) {
                        return true;
                    }
                    layer = next;
                }
                false
            }
            Scope::Siblings => t.siblings(owner).contains(asker),
            Scope::Dunbar => dunbar.contains(asker),
            Scope::List(ks) => ks.contains(asker) && dunbar.contains(asker),
        }
    }

    fn in_horizon(&self, asker: &Keyhash) -> bool {
        self.table.horizon(&self.me, 2).contains(asker)
    }
}

/// One entry as held: the owner, the bytes as signed, the type for
/// filtering, and the host's discover rule for it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Held {
    pub owner: Keyhash,
    pub bytes: Vec<u8>,
    pub service_type: String,
    pub discover: Scope,
}

/// The entries this node serves and the reports it holds.
#[derive(Debug, Default)]
pub struct CatalogService {
    entries: BTreeMap<Keyhash, Held>,
    reports: BTreeMap<Keyhash, Vec<Vec<u8>>>,
}

impl CatalogService {
    /// Take a registration from the authenticated `peer`: the entry must
    /// name the peer as owner and verify under it, and the keyhash must not
    /// be served under another owner.  The reply bytes; `None` where the
    /// body is not a registration, which resets the stream.
    pub fn register<L: Lookup + ?Sized>(&mut self, ids: &L, peer: &Keyhash, body: &[u8]) -> Option<Vec<u8>> {
        let reg = ResourceRegistration::decode(body).ok()?;
        let refused = RegistrationReply { nonce: reg.nonce, code: REGISTRATION_REFUSED }.encode();
        let Ok(entry) = CatalogEntry::parse(&reg.entry) else { return Some(refused) };
        if entry.owner != *peer || entry.verify(ids) != Ok(true) {
            return Some(refused);
        }
        if let Some(held) = self.entries.get(&entry.resource)
            && held.owner != entry.owner {
                return Some(refused);
            }
        // the requested scope is honoured as asked; absent, the existing
        // rule stands, and on a first registration that is `self`
        let discover = match reg.scope {
            Some(s) => s,
            None => self.entries.get(&entry.resource).map(|h| h.discover.clone()).unwrap_or(Scope::Own),
        };
        self.entries.insert(entry.resource, Held { owner: entry.owner, bytes: reg.entry.clone(), service_type: entry.service_type, discover });
        Some(RegistrationReply { nonce: reg.nonce, code: REGISTRATION_RECORDED }.encode())
    }

    pub fn held(&self, resource: &Keyhash) -> Option<&Held> {
        self.entries.get(resource)
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// The entries `asker` may see, in the total order, whole.
    fn visible(&self, asker: &Keyhash, filter: Option<&str>, eval: &dyn ScopeEval) -> Vec<(Keyhash, &Held)> {
        let mut v: Vec<(Keyhash, &Held)> = self.entries.iter().filter(|(_, h)| filter.is_none_or(|t| h.service_type == t) && eval.admits(&h.discover, &h.owner, asker)).map(|(r, h)| (*r, h)).collect();
        v.sort_by_key(|(r, h)| (*r, h.owner));
        v
    }

    /// Answer a query from `asker`: nothing for an asker outside the
    /// horizon, which closes the stream; otherwise the first 111 visible
    /// entries by resource then owner keyhash, and the type of the first
    /// withheld as the continuation.
    pub fn answer(&self, asker: &Keyhash, body: &[u8], eval: &dyn ScopeEval) -> Option<Vec<u8>> {
        let q = CatalogQuery::decode(body).ok()?;
        if !eval.in_horizon(asker) {
            return None;
        }
        let visible = self.visible(asker, q.service_type.as_deref(), eval);
        let entries: Vec<Vec<u8>> = visible.iter().take(CATALOG_REPLY_ENTRIES).map(|(_, h)| h.bytes.clone()).collect();
        let continuation = visible.get(CATALOG_REPLY_ENTRIES).map(|(_, h)| h.service_type.clone());
        Some(CatalogReply { nonce: q.nonce, entries, continuation }.encode())
    }

    /// Take a report from a resource this node hosts for an owner: it must
    /// be signed by the resource it names.  Stored for that resource's
    /// owner; nothing carries it further.
    pub fn take_report<L: Lookup + ?Sized>(&mut self, ids: &L, bytes: &[u8]) -> Result<Keyhash, String> {
        let r = AbuseReport::parse(bytes)?;
        if !r.verify(ids)? {
            return Err("not signed by the resource it names".into());
        }
        let owner = self.entries.get(&r.resource).map(|h| h.owner).ok_or("no owner known for the resource")?;
        self.reports.entry(owner).or_default().push(bytes.to_vec());
        Ok(owner)
    }

    pub fn reports_for(&self, owner: &Keyhash) -> Vec<Vec<u8>> {
        self.reports.get(owner).cloned().unwrap_or_default()
    }
}
