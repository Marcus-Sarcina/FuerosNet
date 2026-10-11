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
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum How {
    /// Met: pinned from their screen at a ceremony.
    Counterparty = 0,
    /// Introduced in a counterparty's bundle, selected or not.
    Introduced = 1,
    /// A nominee that answered, attesting or declining.
    Nominee = 2,
    /// A verifier that answered.
    Verifier = 3,
    /// Infrastructure this client went through.
    Infra = 4,
    /// A candidate proposed in fishing and declined.
    Declined = 5,
    /// Named at start by whoever started this client.
    Provisioned = 6,
}

impl How {
    fn of(n: u64) -> Option<How> {
        Some(match n {
            0 => How::Counterparty,
            1 => How::Introduced,
            2 => How::Nominee,
            3 => How::Verifier,
            4 => How::Infra,
            5 => How::Declined,
            6 => How::Provisioned,
            _ => return None,
        })
    }
}

/// One contact, as the store keeps it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Contact {
    /// The party's `KeyMaterial`, whose hash is the key this is filed under.
    pub material: Vec<u8>,
    /// The most recent `SignedLocator` held for them, where one has been.
    pub locator: Option<Vec<u8>>,
    /// When this client last dealt with them, in seconds.
    pub last_at: u64,
    /// How many times it has.
    pub count: u64,
    /// How the contact came to be held.
    pub how: How,
}

impl Contact {
    /// A contact just learned, dealt with once, now.
    pub fn new(material: Vec<u8>, how: How, now: u64) -> Contact {
        Contact {
            material,
            locator: None,
            last_at: now,
            count: 1,
            how,
        }
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

    /// The bytes the store writes: `[1, material, locator or null, last_at, count, how]`.
    pub fn encode(&self) -> Vec<u8> {
        let mut out = Vec::new();
        emit_array_head(&mut out, 6);
        emit_uint(&mut out, 1);
        emit_bstr(&mut out, &self.material);
        match &self.locator {
            Some(l) => emit_bstr(&mut out, l),
            None => out.push(0xf6),
        }
        emit_uint(&mut out, self.last_at);
        emit_uint(&mut out, self.count);
        emit_uint(&mut out, self.how as u64);
        out
    }

    /// Read a contact back; none for bytes that are not one.
    pub fn decode(b: &[u8]) -> Option<Contact> {
        let item = parse_all(b).ok()?;
        let Item::Array(a) = &item else { return None };
        if a.len() != 6 || as_uint(&a[0]) != Some(1) {
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
        Some(Contact {
            material: bytes(&a[1])?,
            locator,
            last_at: as_uint(&a[3])?,
            count: as_uint(&a[4])?,
            how: How::of(as_uint(&a[5])?)?,
        })
    }
}

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
