//! The client's side of the catalog (`light-client-requirements.md` §8;
//! design §11.5): a view swept from the horizon's nodes and cached, one
//! node's portion at a time, a repeated continuation read as truncation
//! and never followed twice; a brokered service routed to only where it
//! matches the signed entry; an unrecognised declaration surfaced; and a
//! page shown as the node filtered it, never re-expanded.

use crate::Keyhash;
use crate::notice::{Notice, Notifier};
use rhtn_archive::catalog::{CatalogEntry, CatalogQuery, CatalogReply};
use rhtn_crypto::verify::Lookup;
use std::collections::{BTreeMap, BTreeSet};

/// One node's portion of the view.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Portion {
    /// Entries by resource keyhash, as the node served them.
    pub entries: BTreeMap<Keyhash, Vec<u8>>,
    /// The node held more than the bound for some type: this portion is
    /// known incomplete.
    pub truncated: bool,
    /// The node could not be reached on the last sweep.
    pub stale: bool,
}

/// What a sweep of one node does next.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Step {
    /// Ask again, filtered to this type.
    Again(String),
    /// The reply does not echo the nonce of the query outstanding on this
    /// sweep (`wire-format.md` §6.4), so it is not this query's answer and
    /// nothing in it is taken.  The same answer `resolution` and `currency`
    /// give: a reply that cannot be tied to a request is one an asker has
    /// no way to attribute.
    WrongNonce,
    /// The node's portion is complete.
    Done,
    /// A full page and a continuation naming a type already asked for:
    /// the node holds more than the bound, the portion is truncated, and
    /// the hint is not followed.
    Truncated,
}

/// The sweep of one node: the types asked for so far.
#[derive(Debug, Clone, Default)]
pub struct Sweep {
    asked: BTreeSet<Option<String>>,
    /// What this sweep has been served so far, by resource: the portion it
    /// replaces on completion, so an entry the host no longer returns is
    /// gone from the view rather than kept from the last sweep.
    gathered: BTreeMap<Keyhash, Vec<u8>>,
    /// The nonce of the query this sweep is waiting on.  One at a time:
    /// a sweep asks, takes, and asks again.
    outstanding: Option<[u8; 16]>,
    pub queries: u32,
}

impl Sweep {
    /// The query to send next: unfiltered first, then each continuation.
    pub fn query(&mut self, filter: Option<String>, nonce: [u8; 16]) -> CatalogQuery {
        self.asked.insert(filter.clone());
        self.queries += 1;
        self.outstanding = Some(nonce);
        CatalogQuery {
            service_type: filter,
            nonce,
        }
    }

    /// Take a reply into `portion`: entries verified under their owners,
    /// and the continuation followed at most once per type.
    pub fn take<L: Lookup + ?Sized>(
        &mut self,
        ids: &L,
        portion: &mut Portion,
        reply: &CatalogReply,
        full_page: usize,
    ) -> Step {
        // checked before anything is read out of it: an entry from an
        // unattributable reply is an entry from nowhere
        if self.outstanding.is_some_and(|n| n != reply.nonce) {
            return Step::WrongNonce;
        }
        self.outstanding = None;
        for bytes in &reply.entries {
            if let Ok(e) = CatalogEntry::parse(bytes)
                && e.verify(ids).is_ok()
            {
                self.gathered.insert(e.resource, bytes.clone());
            }
        }
        // a completed sweep is the host's portion now, whole: what it served
        // and nothing it stopped serving (`light-client-requirements.md`
        // §8); a truncated one is the portion as far as it goes, marked
        let step = match &reply.continuation {
            None => Step::Done,
            Some(t)
                if self.asked.contains(&Some(t.clone())) && reply.entries.len() >= full_page =>
            {
                Step::Truncated
            }
            Some(t) => Step::Again(t.clone()),
        };
        if matches!(step, Step::Done | Step::Truncated) {
            portion.entries = std::mem::take(&mut self.gathered);
            portion.stale = false;
            portion.truncated = matches!(step, Step::Truncated);
        }
        step
    }
}

/// The catalog view: one portion per node asked.
#[derive(Debug, Clone, Default)]
pub struct View {
    pub portions: BTreeMap<Keyhash, Portion>,
}

impl View {
    pub fn portion(&mut self, node: Keyhash) -> &mut Portion {
        self.portions.entry(node).or_default()
    }

    /// Every entry in the view, with the node that served it.
    pub fn entries(&self) -> Vec<(Keyhash, CatalogEntry)> {
        self.portions
            .iter()
            .flat_map(|(n, p)| {
                p.entries
                    .values()
                    .filter_map(|b| CatalogEntry::parse(b).ok())
                    .map(|e| (*n, e))
            })
            .collect()
    }
}

/// The declared data practices a client recognises (`wire-format.md`
/// §6.1); anything else is surfaced as a declaration it does not.
pub const KNOWN_DATA_PRACTICES: [u64; 4] = [0, 1, 2, 3];

/// Tell the person about an entry's declaration where it is one the
/// client does not recognise.
pub fn surface_declaration(entry: &CatalogEntry, notifier: &dyn Notifier) {
    if let Some(d) = entry.data_practice
        && !KNOWN_DATA_PRACTICES.contains(&d)
    {
        notifier.notify(Notice::UnrecognisedDeclaration {
            resource: entry.resource,
            value: d,
        });
    }
}

/// Why a brokered service is not routed to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Refusal {
    /// The service presents an endpoint other than the signed entry's.
    EndpointDiffers { signed: Vec<u8>, presented: Vec<u8> },
}

/// Where traffic to a brokered resource goes: the entry's endpoint, and
/// the owner the person is shown.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Route {
    pub endpoint: Vec<u8>,
    pub owner: Keyhash,
}

/// Route to a brokered service only where what it presents matches the
/// signed entry's endpoint.
pub fn route_brokered(entry: &CatalogEntry, presented: &[u8]) -> Result<Route, Refusal> {
    if entry.endpoint != presented {
        return Err(Refusal::EndpointDiffers {
            signed: entry.endpoint.clone(),
            presented: presented.to_vec(),
        });
    }
    Ok(Route {
        endpoint: entry.endpoint.clone(),
        owner: entry.owner,
    })
}

/// The page as the node served it: what the person is shown, entry by
/// entry with the roles held, and nothing the node filtered out.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Shown {
    pub entry: CatalogEntry,
    pub roles: Vec<String>,
}

pub fn page<L: Lookup + ?Sized>(ids: &L, served: &[(Vec<u8>, Vec<String>)]) -> Vec<Shown> {
    served
        .iter()
        .filter_map(|(b, roles)| {
            CatalogEntry::parse(b)
                .ok()
                .filter(|e| e.verify(ids).is_ok())
                .map(|entry| Shown {
                    entry,
                    roles: roles.clone(),
                })
        })
        .collect()
}
