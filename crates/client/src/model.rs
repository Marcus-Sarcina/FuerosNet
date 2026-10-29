//! **The nouns as this client holds them** (`rhtn_model`;
//! `Robot/implementation-plan.md` section 8): each composed here, once,
//! from the archive, the horizon, the store and the catalog, so the FFI,
//! the desktop and anything else above the kernel read the same view.
//!
//! Nothing here decides what a noun is called on a screen; everything
//! here decides what it *is*. **An instance is composed above**: its
//! addresses are transport points, and the kernel holds no transport.

use crate::ceremony::Client;
use rhtn_model::{Contact, How, Line, Profile, Resource, Subnet};

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
        let standing: std::collections::BTreeMap<_, _> = self
            .subnet_standing()
            .into_iter()
            .map(|s| {
                (
                    s.subnet,
                    rhtn_model::Standing {
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
                standing: standing.get(&id).copied().unwrap_or_default(),
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
