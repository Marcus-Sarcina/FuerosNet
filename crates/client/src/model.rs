//! **The nouns as this client holds them** (`rhtn_model`;
//! `Robot/implementation-plan.md` section 8): each composed here, once,
//! from the archive, the horizon, the store and the catalog, so the FFI,
//! the desktop and anything else above the kernel read the same view.
//!
//! Nothing here decides what a noun is called on a screen; everything
//! here decides what it *is*. **An instance is composed above**: its
//! addresses are transport points, and the kernel holds no transport.

use crate::ceremony::Client;
use rhtn_archive::Txid;
use rhtn_model::{Contact, How, Line, Meeting, Profile, Resource, Subnet};

impl Client {
    /// Every contact held, most recently met first
    /// (`light-client-requirements.md` §4.3).
    #[must_use]
    pub fn contacts_held(&self) -> Vec<Contact> {
        let mut rows: Vec<Contact> = self
            .store
            .contacts
            .iter()
            .map(|(k, v)| Contact {
                keyhash: *k,
                alias: v.alias.clone(),
                how: v.how,
                located: v.locator.is_some(),
                addressed: self.store.direct_hints.contains_key(k),
                count: v.count,
                last_at: v.last_at,
            })
            .collect();
        rows.sort_by_key(|r| std::cmp::Reverse(r.last_at));
        rows
    }

    /// Every subnet this client holds a position in, with what it is
    /// called, what this client is called there, what this client is in
    /// it, and where it sits.
    #[must_use]
    pub fn subnets_held(&self) -> Vec<Subnet> {
        let rank: std::collections::BTreeMap<_, _> = self
            .subnet_rank()
            .into_iter()
            .map(|s| {
                (
                    s.subnet,
                    rhtn_model::Rank {
                        root: s.root,
                        operator: s.operator,
                        subordinates: s.subordinates as u32,
                    },
                )
            })
            .collect();
        self.subnets()
            .into_iter()
            .map(|id| Subnet {
                id,
                alias: self.subnet_alias(&id),
                own_alias: self.own_alias(&id),
                rank: rank.get(&id).copied().unwrap_or_default(),
                position: self.position_in(&id).map(|l| rhtn_model::Position {
                    anchor: l.anchor,
                    path: l.path.clone(),
                    nibbles: l.nibbles,
                }),
            })
            .collect()
    }

    /// The subnets this client roots and could bring under a patron, in
    /// the same shape.
    #[must_use]
    pub fn subnets_rooted_held(&self) -> Vec<Subnet> {
        let rooted = self.scopes_rooted();
        self.subnets_held()
            .into_iter()
            .filter(|s| rooted.contains(&s.id))
            .collect()
    }

    /// The lines of authority this key holds (`light-client-requirements.md`
    /// §6), newest series and all.
    #[must_use]
    pub fn lines_held(&self) -> Vec<Line> {
        crate::rotation::relationships(&self.archive)
            .iter()
            .map(|r| Line {
                patron: r.patron,
                series: r.series(),
                anchor: r.position.anchor,
                nibbles: r.position.nibbles,
            })
            .collect()
    }

    /// Every resource the catalog holds, attributed to the node that
    /// served its entry and marked where that node's portion was not whole
    /// (`wire-format.md` §6.4).
    #[must_use]
    pub fn resources_held(&self) -> Vec<Resource> {
        self.catalog
            .portions
            .iter()
            .flat_map(|(node, p)| {
                p.entries.values().filter_map(move |b| {
                    let e = rhtn_archive::catalog::CatalogEntry::parse(b).ok()?;
                    Some(Resource {
                        node: *node,
                        resource: e.resource,
                        owner: e.owner,
                        service_type: e.service_type,
                        instance: e.instance,
                        endpoint: e.endpoint,
                        data_practice: e.data_practice,
                        truncated: p.truncated,
                        stale: p.stale,
                    })
                })
            })
            .collect()
    }

    /// **Every meeting this client took part in**, newest first: each
    /// presence record it holds naming it, and the adoption or peering
    /// that rested on that record (field 8 of either), where this client
    /// holds one.
    #[must_use]
    pub fn meetings_held(&self) -> Vec<Meeting> {
        use rhtn_archive::tx::{TYPE_ADOPTION, TYPE_PEERING, TYPE_PRESENCE};
        let me = self.keyhash();
        let rested: Vec<(Txid, u64, Txid)> = self
            .archive
            .records()
            .filter(|r| r.tx_type == TYPE_ADOPTION || r.tx_type == TYPE_PEERING)
            .filter_map(|r| r.field_hash(8).map(|pop| (pop, r.tx_type, r.txid)))
            .collect();
        let mut out: Vec<Meeting> = self
            .archive
            .records()
            .filter(|r| r.tx_type == TYPE_PRESENCE)
            .filter_map(|r| {
                let parts = r.participants();
                if !parts.contains(&me) {
                    return None;
                }
                let counterparty = parts.into_iter().find(|p| *p != me)?;
                let on = |t: u64| {
                    rested
                        .iter()
                        .find(|(pop, ty, _)| *pop == r.txid && *ty == t)
                        .map(|(_, _, x)| *x)
                };
                Some(Meeting {
                    record: r.txid,
                    counterparty,
                    finalized_at: r.time,
                    witnesses: r.witnesses().into_iter().map(|(w, _)| w).collect(),
                    adoption: on(TYPE_ADOPTION),
                    peering: on(TYPE_PEERING),
                })
            })
            .collect();
        out.sort_by_key(|m| std::cmp::Reverse(m.finalized_at));
        out
    }

    /// Every profile held: this client's own, by subnet, and the subnets'.
    #[must_use]
    pub fn profiles_held(&self) -> Vec<Profile> {
        self.store
            .profiles
            .values()
            .chain(self.store.subnet_profiles.values())
            .filter_map(|b| rhtn_archive::profile::Profile::parse(b).ok())
            .map(|p| Profile {
                subject: p.subject,
                subnet: p.subnet,
                kind: p.kind,
                alias: p.alias,
                image_format: p.image.as_ref().map(|(f, _)| *f),
                image: p.image.map(|(_, b)| b),
                published_at: p.published_at,
            })
            .collect()
    }

    /// The wire's kind for a contact, where a caller holds the number.
    #[must_use]
    pub fn how_of(n: u64) -> Option<How> {
        How::of(n)
    }
}

/// **A row this client would assert to a node it operates**
/// (`Robot/deferred-decisions.md`, the horizon as a table its operator
/// maintains): one binding of this client's own horizon, with the key
/// material that lets the node authenticate the member when it connects.
///
/// **Not a transaction and not evidence.** It carries the binding's
/// identity so it merges with one the node later learns from the record
/// itself, and no reason at all: a node runs under its operator's
/// authority and is not being persuaded of anything.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HorizonRow {
    /// The subordinate.
    pub node: rhtn_archive::Keyhash,
    /// The patron.
    pub patron: rhtn_archive::Keyhash,
    /// The series the adoption opened.
    pub series: u32,
    /// The adoption that opened it.
    pub adoption: Txid,
    /// The patron's clock on that adoption.
    pub from: u64,
    /// The subnet the adoption's locator named.
    pub anchor: Option<rhtn_archive::Keyhash>,
    /// The patron's slot the locator claimed.
    pub slot: Option<u8>,
    /// **The member's key material**, which is the part a keyhash cannot
    /// replace: a keyhash is taken over both keys and the transport
    /// presents only the classical one, so a node resolves a connection by
    /// comparing the presented key against material it holds.
    pub material: Vec<u8>,
    /// This client's own counter for the row.
    pub revision: u64,
}

impl Client {
    /// **Every row this client would assert about the horizon it serves.**
    ///
    /// **Read from this client's own archive, which is the only complete
    /// record of what it has issued** — the same answer
    /// [`Client::free_index`] gives, and for the same reason. The
    /// propagated copy is fed by a serving node, so a client with none
    /// holds no edge of its own in it: a subnet formed with no node at all
    /// — which `Robot/field-test-sequence.md` §G is entirely made of —
    /// would otherwise leave every patron unable to name a single member.
    ///
    /// **An ending is taken only where this client can see one.** A
    /// departure or disavowal in the propagated copy closes a row; where
    /// nothing propagated, every adoption this client issued stands, which
    /// is the conservative half of the same asymmetry: a patron with no
    /// news has no grounds to drop a member it adopted. Dropping one on
    /// silence would unbind a party that is still there.
    ///
    /// A member whose key material this client does not hold is **left out
    /// rather than sent incomplete**: a row without material names a
    /// member the node could never recognise, so sending it would bind a
    /// position that can never attach and report success.
    ///
    /// **The revision is this client's clock** [2026-11-04, and the
    /// weakest part of this]: an assertion carries no transaction and so
    /// no order of its own, and two of an operator's devices must still
    /// converge. Wall-clock seconds make the later post win, which is
    /// right when two devices agree about the time and wrong by exactly
    /// their skew when they do not. The node answers a losing post with
    /// the revision it holds, so a caller that is refused can come back
    /// above it — the self-healing counter this wants next.
    #[must_use]
    pub fn horizon_rows(&self) -> Vec<HorizonRow> {
        let now = self.now_s();
        let me = self.public.keyhash;
        // endings this client has actually been told about
        let ended: std::collections::BTreeSet<rhtn_archive::Keyhash> = self
            .horizon
            .table
            .bindings()
            .iter()
            .filter(|b| b.patron == me && b.end.is_some())
            .map(|b| b.node)
            .collect();
        let mut rows = Vec::new();
        for rec in self
            .archive
            .records()
            .filter(|r| r.tx_type == rhtn_archive::tx::TYPE_ADOPTION)
        {
            let (Some(node), Some(patron)) = (rec.field_hash(1), rec.field_hash(2)) else {
                continue;
            };
            if patron != me || ended.contains(&node) {
                continue;
            }
            let Some(series) = rec.seqno().map(|s| s.series) else {
                continue;
            };
            let Some(material) = self
                .known
                .iter()
                .find(|i| i.keyhash == node)
                .map(rhtn_crypto::Identity::key_material)
            else {
                continue;
            };
            rows.push(HorizonRow {
                node,
                patron,
                series,
                adoption: rec.txid,
                from: rec.time,
                anchor: rec.locator().map(|l| l.anchor),
                slot: rec.locator().and_then(|l| l.slot()),
                material,
                revision: now,
            });
        }
        rows
    }
}
