//! Contacts (`light-client-requirements.md` §4.3): for every party whose
//! locator has become visible, the most recent `SignedLocator` held and
//! the key material that verifies it — the two halves of an introduction
//! (`wire-format.md` §2.3) — with when this client last dealt with them
//! and how often. The store is bounded, and pruned toward what still
//! resolves [author, 2026-10-11].

use crate::Keyhash;
use rhtn_archive::locator::SignedLocator;
use rhtn_codec::cbor::*;
use rhtn_codec::encode::*;
use std::collections::BTreeMap;

/// How a contact came to be held: the first way, kept for what it says
/// about the party, never a rank.
pub use rhtn_model::How;

/// One contact, as the store keeps it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Contact {
    /// The party's `KeyMaterial`, whose hash is the key this is filed
    /// under — **empty where only the keyhash is known**.
    ///
    /// **A hop of a resolution walk can be that** [author, 2026-10-14].
    /// §7.7.3 leaves a referral's key material optional, and the session
    /// that reaches the hop does not supply it: a dial presents the
    /// classical Ed25519 key alone, while `KeyMaterial` is the pair of
    /// COSE keys, classical and post-quantum, whose whole encoding the
    /// keyhash is taken over (`wire-format.md` §2.2). One member cannot
    /// be reconstructed from the other, and there is no fetch path. Such
    /// a contact is a name and a place to try, nothing more: it cannot
    /// verify a signature, cannot bind a dial (§9.1), and is never
    /// introduced or handed on.
    pub material: Vec<u8>,
    /// The most recent `SignedLocator` held for them, where one has been.
    pub locator: Option<Vec<u8>>,
    /// When this client last dealt with them, in seconds.
    pub last_at: u64,
    /// How many times it has.
    pub count: u64,
    /// How the contact came to be held.
    pub how: How,
    /// This party's `EndpointRecord` (`wire-format.md` §7.6), signed by
    /// it, where one is held: **where it answers**.
    ///
    /// **This is the local table design §12.3 Case 1 looks an anchor up
    /// in**: "Bob looks up `anchor` in his local table for an address to
    /// start from — his contacts". A locator names an anchor and a path
    /// and never an address, so a party introduced out of band cannot be
    /// reached until some record of where its anchor answers is held, and
    /// the horizon is not that place: it keeps endpoints for its own
    /// neighbourhood alone (distance 2) and refuses a record for anything
    /// further off [2026-10-13]. None for the ordinary contact, which is a
    /// person and answers nowhere.
    ///
    /// **The record and not a copy of its contents**: the points, the
    /// series and the counter are all in it, and it is what this client
    /// hands on in a card of its own (§2.4).
    pub endpoint_record: Option<Vec<u8>>,
    /// Its `AnchorEntry`, where one came with that record: what subtree
    /// it claims, which a caching policy reads (design §12.5).
    pub anchor_entry: Option<Vec<u8>>,
    /// Addresses **heard in a referral** and signed by nobody, as the
    /// `NetworkPoint`s were encoded (`wire-format.md` §7.7.3).
    ///
    /// **Kept apart from the record for what they are.** A referral's
    /// endpoints are the referrer's assertion, and §7.7.3 says so
    /// deliberately: the key material of a hop is optional there because
    /// "a referrer's identity is not what protects the requester". They
    /// are worth keeping — a node this client reached once is a place a
    /// later walk can start — and they are not a publication, so they are
    /// never handed on in a card (§2.4) and are offered after anything
    /// signed.
    pub heard: Vec<Vec<u8>>,
    /// The alias this party published for the subnet its locator is in
    /// (`wire-format.md` §7.11), where one has reached this client.
    /// **Display only**: nothing here is keyed by it.
    pub alias: Option<String>,
    /// Its published image, a format tag and the bytes, where one has.
    pub image: Option<(u64, Vec<u8>)>,
    /// When the profile held was published, so a later one replaces it.
    pub profile_at: u64,
}

impl Contact {
    /// A contact that is a keyhash and no more: a hop whose referral
    /// carried no key material, kept for the walk it was on and for what
    /// it says about where another contact sits.
    pub fn named_only(how: How, now: u64) -> Contact {
        Contact {
            material: Vec::new(),
            ..Contact::new(Vec::new(), how, now)
        }
    }

    /// Whether the key material is held, and so whether this contact can
    /// be verified by, dialled bound, or introduced.
    pub fn named(&self) -> bool {
        !self.material.is_empty()
    }

    /// A contact just learned, dealt with once, now.
    pub fn new(material: Vec<u8>, how: How, now: u64) -> Contact {
        Contact {
            material,
            locator: None,
            last_at: now,
            count: 1,
            how,
            endpoint_record: None,
            anchor_entry: None,
            heard: Vec::new(),
            alias: None,
            image: None,
            profile_at: 0,
        }
    }

    /// Take a profile this party published, where it is later than the
    /// one held (`wire-format.md` §7.11).
    pub fn take_profile(&mut self, p: &rhtn_archive::profile::Profile) -> bool {
        if p.published_at <= self.profile_at && self.alias.is_some() {
            return false;
        }
        self.alias = Some(p.alias.clone());
        self.image = p.image.clone();
        self.profile_at = p.published_at;
        true
    }

    /// One more dealing, now.
    pub fn touch(&mut self, now: u64) {
        self.last_at = self.last_at.max(now);
        self.count = self.count.saturating_add(1);
    }

    /// The contact's position as its locator states it: the anchor and
    /// the significant nibbles of the path; none where no locator is held
    /// or it does not read.
    pub fn position(&self) -> Option<(Keyhash, Vec<u8>)> {
        let sl = SignedLocator::parse(self.locator.as_deref()?).ok()?;
        let l = sl.locator;
        let n = usize::try_from(l.nibbles).ok()?;
        let mut out = Vec::with_capacity(n);
        for i in 0..n {
            let b = *l.path.get(i / 2)?;
            out.push(if i % 2 == 0 { b >> 4 } else { b & 0x0f });
        }
        Some((l.anchor, out))
    }

    /// The bytes the store writes: `[1, material, locator or null, last_at,
    /// count, how, endpoint record, anchor entry, heard, alias or null,
    /// image or null, profile_at]`.
    pub fn encode(&self) -> Vec<u8> {
        let mut out = Vec::new();
        emit_array_head(&mut out, 12);
        emit_uint(&mut out, 1);
        emit_bstr(&mut out, &self.material);
        match &self.locator {
            Some(l) => emit_bstr(&mut out, l),
            None => out.push(0xf6),
        }
        emit_uint(&mut out, self.last_at);
        emit_uint(&mut out, self.count);
        emit_uint(&mut out, self.how as u64);
        for held in [&self.endpoint_record, &self.anchor_entry] {
            match held {
                Some(b) => emit_bstr(&mut out, b),
                None => out.push(0xf6),
            }
        }
        emit_array_head(&mut out, self.heard.len());
        for h in &self.heard {
            emit_bstr(&mut out, h);
        }
        match &self.alias {
            Some(a) => emit_tstr(&mut out, a),
            None => out.push(0xf6),
        }
        match &self.image {
            Some((f, b)) => {
                emit_array_head(&mut out, 2);
                emit_uint(&mut out, *f);
                emit_bstr(&mut out, b);
            }
            None => out.push(0xf6),
        }
        emit_uint(&mut out, self.profile_at);
        out
    }

    /// Read a contact back; none for bytes that are not one.
    pub fn decode(b: &[u8]) -> Option<Contact> {
        let item = parse_all(b).ok()?;
        let Item::Array(a) = &item else { return None };
        // six before a contact could hold an address, eight with the
        // records it publishes, nine with the addresses a referral
        // asserted, twelve with the profile it published
        if !matches!(a.len(), 6 | 8 | 9 | 12) || as_uint(&a[0]) != Some(1) {
            return None;
        }
        let bytes = |it: &Item| match it {
            Item::Bytes(r) => Some(b[r.clone()].to_vec()),
            _ => None,
        };
        let locator = match &a[2] {
            Item::Null => None,
            it => Some(bytes(it)?),
        };
        let held = |it: Option<&Item>| match it {
            None | Some(Item::Null) => Ok(None),
            Some(it) => bytes(it).map(Some).ok_or(()),
        };
        let endpoint_record = held(a.get(6)).ok()?;
        let anchor_entry = held(a.get(7)).ok()?;
        let mut heard = Vec::new();
        if let Some(Item::Array(hs)) = a.get(8) {
            for h in hs {
                heard.push(bytes(h)?);
            }
        }
        let alias = match a.get(9) {
            Some(Item::Text(r)) => Some(std::str::from_utf8(&b[r.clone()]).ok()?.to_string()),
            _ => None,
        };
        let image = match a.get(10) {
            Some(Item::Array(i)) if i.len() == 2 => Some((as_uint(&i[0])?, bytes(&i[1])?)),
            _ => None,
        };
        let profile_at = a.get(11).and_then(as_uint).unwrap_or(0);
        Some(Contact {
            material: bytes(&a[1])?,
            locator,
            last_at: as_uint(&a[3])?,
            count: as_uint(&a[4])?,
            how: How::of(as_uint(&a[5])?)?,
            endpoint_record,
            anchor_entry,
            heard,
            alias,
            image,
            profile_at,
        })
    }
}

/// How many heard addresses one contact keeps: a referral names at most
/// eight endpoints (`wire-format.md` §7.7.3) and a node this client keeps
/// reaching by other paths should not grow a list without end.
pub const HEARD_PER_CONTACT: usize = 8;

/// Whether `who` would still be discoverable from a contact kept here: its
/// anchor is held, or another held contact under the same anchor sits on
/// a proper prefix of its path (the nearest infra above it, design §12.3
/// Case 1).
pub fn upstream_held(contacts: &BTreeMap<Keyhash, Contact>, who: &Keyhash) -> bool {
    let Some((anchor, path)) = contacts.get(who).and_then(Contact::position) else {
        return false;
    };
    if anchor == *who {
        return false;
    }
    if contacts.contains_key(&anchor) {
        return true;
    }
    contacts.iter().any(|(k, c)| {
        k != who
            && c.position()
                .is_some_and(|(a, p)| a == anchor && p.len() < path.len() && path.starts_with(&p))
    })
}

/// **Prune toward what still resolves** (`light-client-requirements.md`
/// §4.3): rank the contacts least dealt with — longest ago first, fewest
/// times next — and remove, in that order, those an upstream still held
/// discovers, until `keep` remain; where that is not enough, the rest go
/// in the same order, a contact with no upstream held going last because
/// pruning it is losing it. Returns how many went.
pub fn prune(contacts: &mut BTreeMap<Keyhash, Contact>, keep: usize) -> usize {
    if contacts.len() <= keep {
        return 0;
    }
    let mut ranked: Vec<Keyhash> = contacts.keys().copied().collect();
    ranked.sort_by_key(|k| (contacts[k].last_at, contacts[k].count, *k));
    let mut gone = 0;
    for k in &ranked {
        if contacts.len() <= keep {
            break;
        }
        if upstream_held(contacts, k) {
            contacts.remove(k);
            gone += 1;
        }
    }
    for k in &ranked {
        if contacts.len() <= keep {
            break;
        }
        if contacts.remove(k).is_some() {
            gone += 1;
        }
    }
    gone
}
