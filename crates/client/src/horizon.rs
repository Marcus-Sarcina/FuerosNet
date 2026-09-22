//! What a client keeps of its own horizon (design §15.1.1;
//! `light-client-requirements.md` §4.2).
//!
//! Enough to resolve every node in the horizon directly, so a patron that
//! is not answering is something to route around rather than something to
//! wait for, and enough to say how far away a party is.
//!
//! **The patron's propagation is the system of record.**  Everything here
//! is a copy of what a serving node pushed; where the two differ, what was
//! pushed wins and this is rewritten.  A client never presents it as
//! authority, to a user or to a counterparty.
//!
//! **The derived shape is stored as derived.**  A wake folds in what
//! arrived since the snapshot was written, and replays the records held
//! only when the snapshot cannot account for them.

use crate::{Keyhash, Txid};
use rhtn_archive::endpoint::{self, EndpointRecord};
use rhtn_archive::record::Record;
use rhtn_archive::topology::{End, Evaluation, Snapshot, Table, unfolded};
use rhtn_archive::tx::Locator;
use rhtn_crypto::verify::Lookup;
use rhtn_crypto::verify::{self, Delegation};
use std::collections::{BTreeMap, BTreeSet};

/// What a wake did with the snapshot it found.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Woke {
    /// The snapshot was the record set's value: nothing was folded.
    Current,
    Extended {
        folded: usize,
    },
    Replayed {
        replayed: usize,
    },
}

/// Where a node sits, which is what a resolution needs to reach it: the
/// anchor the path is relative to, and the path (`wire-format.md` §7.7.3).
///
/// **Not a locator.** A locator also carries the series and counter of the
/// binding that made it, and those belong to the record that carried them.
/// A place derived from a subordinate's record — the anchor's own, at the
/// empty path — has no series to claim, and claiming one would be
/// inventing what nobody propagated.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Place {
    pub anchor: Keyhash,
    pub path: Vec<u8>,
    pub nibbles: u64,
}

/// A party, and the subnet a place of theirs is in.
pub type Placement = (Keyhash, Keyhash);

/// A packed path's nibbles, one per hop (`wire-format.md` §2.1).
fn nibbles_of(path: &[u8], n: u64) -> Vec<u8> {
    (0..usize::try_from(n).unwrap_or(usize::MAX))
        .map_while(|i| {
            path.get(i / 2).map(|b| {
                if i.is_multiple_of(2) {
                    b >> 4
                } else {
                    b & 0x0f
                }
            })
        })
        .collect()
}

/// The packing §2.1 defines: high nibble first, and the unused low nibble
/// of a final odd byte zero.
fn pack(nibbles: &[u8]) -> (Vec<u8>, u64) {
    let mut out = vec![0u8; nibbles.len().div_ceil(2)];
    for (i, v) in nibbles.iter().enumerate() {
        if i.is_multiple_of(2) {
            out[i / 2] |= v << 4;
        } else {
            out[i / 2] |= v & 0x0f;
        }
    }
    (out, nibbles.len() as u64)
}

/// What ingesting a propagated record did here.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Took {
    /// Held, and the table moved.
    Applied,
    /// Already held: propagation repeats, and a repeat is not news.
    Duplicate,
    /// Not well-formed, not signed by whom it names, or refused by the
    /// table.  Nothing here changed.
    Refused,
    /// Equal `seqno`, different signed contents (`wire-format.md`
    /// §10.1.2): the pair is malformed, **neither** is current now, and
    /// what was held is dropped.  Repaired by re-resolving (§7.7).
    Conflict,
    /// A second endpoint line for a subject already placed, in a series
    /// nothing this client holds proves current.  Not taken; a client
    /// floods nothing, so §10.1.2 leaves dropping it and letting the
    /// patron re-propagate a local choice.
    Unproved,
}

/// The topology a client holds, and the fold over it.
///
/// The record set is bounded by what a patron floods, which is bounded by
/// the horizon (design §15.1); [`Horizon::prune`] drops what has left it.
#[derive(Default)]
pub struct Horizon {
    me: Keyhash,
    /// What the serving node propagated, by txid: this client's own
    /// seen-set.  Kept because the table is a fold over it and a fold
    /// needs its input to be re-runnable.
    records: BTreeMap<Txid, Vec<u8>>,
    /// The fold's result.
    pub table: Table,
    /// Where each node was last said to be, so it can be reached without
    /// asking anyone (design §15.1.1).  A node's own adoption puts it
    /// here; the anchor its path is relative to is here too, at the empty
    /// path, which is what that anchor being the anchor means.
    ///
    /// **Keyed by party *and* subnet.**  A party adopted under two patrons
    /// is in two of them (`wire-format.md` §2.3: a node bound under two
    /// patrons keeps two series), and its path in one says nothing about
    /// its path in the other.  Keyed by party alone the second record
    /// would replace the first, and this client would hold one of the two
    /// places a party actually has with no way to tell which.
    places: BTreeMap<Placement, Place>,
    /// The locator as propagated, for the nodes a record placed.  A party
    /// this client only knows the position of, because a subordinate's
    /// record named it as an anchor, is in `places` and not here.
    locators: BTreeMap<Placement, Locator>,
    /// Where the infrastructure in this horizon answers, from the endpoint
    /// records the patron flooded (`wire-format.md` §7.6).
    ///
    /// **Only infra nodes publish one**, so holding a record is also what
    /// says its publisher is infrastructure — the same evidence a node
    /// reads it as [author, 2026-09-14]. Without these a client holds a
    /// shape with no addresses in it: it can say who its patron's siblings
    /// are and not reach any of them.
    endpoints: BTreeMap<(Keyhash, u32), EndpointRecord>,
    /// The delegations of the parties in the horizon, one per delegating
    /// keyhash, the newest by `not_before` (`light-client-requirements.md`
    /// §4.2) [author, 2026-09-21]: what binds a delegated peer on a direct
    /// path without waiting for its frame.  Current state, never a history
    /// of keys.
    delegations: BTreeMap<Keyhash, Delegation>,
    /// `(subject, series, counter)` retired by an equivocation: neither
    /// content is current and no later one for that pair is taken
    /// (`wire-format.md` §10.1.2).
    conflicts: BTreeSet<(Keyhash, u32, u32)>,
}

impl Horizon {
    pub fn new(me: Keyhash) -> Horizon {
        Horizon {
            me,
            records: BTreeMap::new(),
            table: Table::with_me(me),
            places: BTreeMap::new(),
            locators: BTreeMap::new(),
            endpoints: BTreeMap::new(),
            delegations: BTreeMap::new(),
            conflicts: BTreeSet::new(),
        }
    }

    pub fn me(&self) -> Keyhash {
        self.me
    }

    /// Take a delegation the serving node propagated (`wire-format.md`
    /// §10.1): stored when it verifies under the delegating keyhash's
    /// material and that keyhash is a party this client can place, the
    /// newest per keyhash kept and the rest dropped.
    pub fn ingest_delegation<L: Lookup + ?Sized>(&mut self, bytes: &[u8], ids: &L) -> Took {
        let Ok(d) = verify::delegation(ids, bytes) else {
            return Took::Refused;
        };
        // bounded by the horizon, as the records are: a delegation of a
        // party this client cannot place is not one it will dial
        if d.keyhash != self.me && self.distance(&d.keyhash).is_none() {
            return Took::Refused;
        }
        if self
            .delegations
            .get(&d.keyhash)
            .is_some_and(|held| held.not_before >= d.not_before)
        {
            return Took::Duplicate;
        }
        self.delegations.insert(d.keyhash, d);
        Took::Applied
    }

    /// The delegation held for a party in the horizon.
    pub fn delegation(&self, keyhash: &Keyhash) -> Option<&Delegation> {
        self.delegations.get(keyhash)
    }

    /// The held delegation naming transport key `key`.
    pub fn delegation_by_key(&self, key: &[u8; 32]) -> Option<&Delegation> {
        self.delegations.values().find(|d| d.key == *key)
    }

    pub fn delegations(&self) -> usize {
        self.delegations.len()
    }

    /// Take an endpoint record the serving node propagated.
    ///
    /// Bounded by the same horizon the records are: one for a party this
    /// client cannot place is not a party it has any use for an address
    /// of.
    pub fn ingest_endpoint<L: Lookup + ?Sized>(&mut self, bytes: &[u8], ids: &L) -> Took {
        let Ok(er) = EndpointRecord::parse(bytes) else {
            return Took::Refused;
        };
        if er.signature_checks(ids) == Some(false) {
            return Took::Refused;
        }
        if self.table.distance(&self.me, &er.node, 2).is_none() {
            return Took::Refused;
        }
        let key = (er.node, er.seqno.series);
        // **§10.1.2's rule, the same one a node applies** — decided in
        // `rhtn-archive` and stored here.  This client's series evidence
        // is its own table: an open binding's series is the one the
        // relationship is in, which a reissue moves and an adoption opens.
        let proved = self
            .table
            .bindings()
            .iter()
            .any(|b| b.node == er.node && b.open() && b.series == er.seqno.series);
        let line = endpoint::decide(
            self.endpoints.get(&key),
            &er,
            self.conflicts
                .contains(&(er.node, er.seqno.series, er.seqno.counter)),
            self.endpoints
                .keys()
                .any(|(n, ser)| *n == er.node && *ser != er.seqno.series),
            proved,
        );
        match line {
            endpoint::Line::Duplicate => return Took::Duplicate,
            endpoint::Line::Conflict => {
                // neither content is current, and the pair is retired: a
                // holder that kept the earlier one would let arrival order
                // split the view
                self.endpoints.remove(&key);
                self.conflicts
                    .insert((er.node, er.seqno.series, er.seqno.counter));
                return Took::Conflict;
            }
            endpoint::Line::Unproved => return Took::Unproved,
            endpoint::Line::Current => {}
        }
        // **holding it is what marks the publisher infrastructure**: §7.6
        // has only infra nodes publish, and nothing else distinguishes the
        // two from outside.
        self.table.mark_infra(er.node);
        self.endpoints.insert(key, er);
        Took::Applied
    }

    /// Where `node` says it answers, across every line it published on,
    /// as the `NetworkPoint`s were encoded.
    ///
    /// **Left encoded on the way out** because the type that reads one
    /// belongs to the transport, and a client's copy of its own
    /// neighbourhood has no business depending on a socket library.
    pub fn endpoints_of(&self, node: &Keyhash) -> Vec<Vec<u8>> {
        if self.table.distance(&self.me, node, 2).is_none() {
            return Vec::new();
        }
        let mut out: Vec<Vec<u8>> = Vec::new();
        for (_, e) in self.endpoints.range((*node, 0)..=(*node, u32::MAX)) {
            for p in &e.endpoints {
                if !out.contains(p) {
                    out.push(p.clone());
                }
            }
        }
        out
    }

    /// The infrastructure this client can reach without asking anyone: the
    /// parties in its horizon that have published an endpoint record, with
    /// where each answers.
    ///
    /// **This is what routing around an unanswering node is made of**
    /// (`light-client-requirements.md` §4.2): a client that had to ask its
    /// patron where the alternatives are cannot use them when the patron
    /// is the thing that is down.
    pub fn reachable_infra(&self) -> Vec<(Keyhash, Vec<Vec<u8>>)> {
        let inside = self.table.horizon(&self.me, 2);
        let mut seen: Vec<Keyhash> = self
            .endpoints
            .keys()
            .map(|(n, _)| *n)
            .filter(|n| inside.contains(n))
            .collect();
        seen.dedup();
        seen.into_iter()
            .map(|n| (n, self.endpoints_of(&n)))
            .collect()
    }

    pub fn holds(&self, txid: &Txid) -> bool {
        self.records.contains_key(txid)
    }

    /// `(patron, node)` for every relationship a patron ended **with
    /// prejudice** in what this client holds (`wire-format.md` §4.3).
    ///
    /// **Read from the records, not from the binding's end.** Whichever of
    /// a departure and a disavowal reached the fold first is the one the
    /// binding records, and design §18.5 is explicit that a member does
    /// not order the two — so a determination that lost the race is still
    /// a determination the patron made and this client holds.
    pub fn determinations(&self) -> Vec<(Keyhash, Keyhash)> {
        let mut out = Vec::new();
        for bytes in self.records.values() {
            let Ok(rec) = Record::parse(bytes) else {
                continue;
            };
            if rec.tx_type != rhtn_archive::tx::TYPE_DISAVOWAL {
                continue;
            }
            // §4.3 puts the reason in field 4; absent, nothing is alleged
            if !rec.field_uint(4).is_some_and(End::band) {
                continue;
            }
            if let (Some(patron), Some(node)) = (rec.field_hash(1), rec.field_hash(2)) {
                out.push((patron, node));
            }
        }
        out
    }

    pub fn records(&self) -> usize {
        self.records.len()
    }

    /// Take a record the serving node propagated.
    ///
    /// **Evidence is not dereferenced.**  What a client holds about two
    /// other parties is a position and not a weighed edge, which is what
    /// `Evaluation::Deferred` records (`wire-format.md` §3.4); a client
    /// that refused every adoption whose presence record it does not hold
    /// would hold almost no horizon at all.
    pub fn ingest<L: Lookup + ?Sized>(&mut self, bytes: &[u8], ids: &L) -> Took {
        let Ok(rec) = Record::parse(bytes) else {
            return Took::Refused;
        };
        if self.records.contains_key(&rec.txid) {
            return Took::Duplicate;
        }
        if self
            .table
            .apply_with(&rec, ids, &self.records, None, Evaluation::Deferred)
            .is_err()
        {
            return Took::Refused;
        }
        self.note_locator(&rec);
        self.note_end(&rec);
        self.records.insert(rec.txid, bytes.to_vec());
        Took::Applied
    }

    /// A record that ended a binding moves an address, not just an edge.
    ///
    /// **The table and the address book answer different questions**
    /// [author, 2026-09-14].  The fold closes the binding, and that is the
    /// whole of what the topology says.  What a holder still needs is a way
    /// to reach the party: it has left the place its old path named, and
    /// every path under it named through that patron is now wrong by one
    /// ancestor.  So an observer that saw the ending **re-anchors the
    /// departed party on itself and rewrites the subtree beneath it**,
    /// which is what a root is — `Locator::root`'s empty path, arrived at
    /// by subtraction rather than by minting.
    ///
    /// **Only where the party has no patron left in that subnet.** A node
    /// bound twice in one subnet is not a state the tree admits, but a
    /// disavowal racing a re-adoption can leave one binding open, and a
    /// party still placed under a patron is not a root of anything.
    fn note_end(&mut self, rec: &Record) {
        let Some((node, anchor)) = self
            .table
            .bindings()
            .iter()
            .find(|b| matches!(&b.end, Some((t, _, _)) if *t == rec.txid))
            .and_then(|b| b.anchor.map(|a| (b.node, a)))
        else {
            return;
        };
        if anchor == node
            || self
                .table
                .bindings()
                .iter()
                .any(|b| b.node == node && b.anchor == Some(anchor) && b.open())
        {
            return;
        }
        self.reanchor(node, anchor);
    }

    /// Move `node` and everything placed beneath it out of `anchor`'s
    /// subnet and under `node` itself, each path shortened by the prefix
    /// that reached `node`.
    fn reanchor(&mut self, node: Keyhash, anchor: Keyhash) {
        let Some(base) = self
            .places
            .get(&(node, anchor))
            .map(|p| nibbles_of(&p.path, p.nibbles))
        else {
            return;
        };
        // **an empty prefix is every path in the subnet**, and a party at
        // the empty path under an anchor that is not itself is malformed
        // (`wire-format.md` §2.1's self-anchor is the only empty one).
        // Moving the whole subnet on one is not a repair.
        if base.is_empty() {
            return;
        }
        let moved: Vec<(Keyhash, Vec<u8>)> = self
            .places
            .iter()
            .filter(|((_, a), _)| *a == anchor)
            .filter_map(|((x, _), p)| {
                nibbles_of(&p.path, p.nibbles)
                    .strip_prefix(&base[..])
                    .map(|rest| (*x, rest.to_vec()))
            })
            .collect();
        for (x, rest) in moved {
            self.places.remove(&(x, anchor));
            // **the propagated locator goes with it**: it named a position
            // in a subnet this party has left, and nobody has propagated a
            // replacement.  A place with no locator is the ordinary state
            // of a party known only by where it sits.
            self.locators.remove(&(x, anchor));
            let (path, nibbles) = pack(&rest);
            self.places.insert(
                (x, node),
                Place {
                    anchor: node,
                    path,
                    nibbles,
                },
            );
        }
    }

    /// An adoption names where its subject sits, and that is the whole of
    /// what a client needs to reach it (`wire-format.md` §2.3).  A later
    /// one for the same party replaces the earlier, which is what makes
    /// the propagated version the system of record here.
    fn note_locator(&mut self, rec: &Record) {
        let (Some(node), Some(loc)) = (rec.field_hash(1), rec.locator()) else {
            return;
        };
        // **a party placed under someone is not its own anchor.** An
        // ending may have re-anchored it on itself ([`Horizon::reanchor`]);
        // an adoption propagated afterwards says that is over, and leaving
        // the self-anchor would have this client holding a party as a root
        // and as a subordinate at once.  What was placed *beneath* it stays
        // where it was put: nothing translates a path when an ancestor
        // moves (design §12.6.2), and those go stale as any other does.
        if node != loc.anchor {
            self.places.remove(&(node, node));
        }
        self.places.insert(
            (node, loc.anchor),
            Place {
                anchor: loc.anchor,
                path: loc.path.clone(),
                nibbles: loc.nibbles,
            },
        );
        // the anchor a path is relative to sits at the empty path under
        // itself: a fact the record states rather than one derived from it
        self.places
            .entry((loc.anchor, loc.anchor))
            .or_insert(Place {
                anchor: loc.anchor,
                path: Vec::new(),
                nibbles: 0,
            });
        self.locators.insert((node, loc.anchor), loc);
    }

    /// Where `node` sits, for a resolution this client runs itself.
    ///
    /// **Bounded by the horizon on the way out**
    /// (`light-client-requirements.md` §4.2), not by remembering to prune.
    /// An ending moves a place rather than deleting one ([`Horizon::note_end`]),
    /// so a party that left one subnet is still in the map — under itself —
    /// until [`Horizon::prune`] drops what the horizon no longer covers, and
    /// a replay reaches the same map by the same route.  Reading through the
    /// current bound makes the replayed view and the incremental one agree by
    /// construction.
    /// Every subnet this client can place `node` in, ordered by anchor.
    ///
    /// **Plural because a party may be in more than one**, and a caller
    /// that took one of several without saying which would be asserting
    /// something the records do not.
    pub fn places_of(&self, node: &Keyhash) -> Vec<&Place> {
        if self.table.distance(&self.me, node, 2).is_none() {
            return Vec::new();
        }
        self.places
            .range((*node, [0; 32])..=(*node, [0xff; 32]))
            .map(|(_, p)| p)
            .collect()
    }

    /// Where `node` sits in the subnet `anchor` names, if this client can
    /// place it there.
    pub fn place_in(&self, node: &Keyhash, anchor: &Keyhash) -> Option<&Place> {
        self.table.distance(&self.me, node, 2)?;
        self.places.get(&(*node, *anchor))
    }

    /// The locator a record carried for `node`, series and counter
    /// included.  A node this client places only as an anchor has none.
    ///
    /// Bounded by the horizon, as [`Horizon::place`] is and for the same
    /// reason.
    pub fn locators_of(&self, node: &Keyhash) -> Vec<&Locator> {
        if self.table.distance(&self.me, node, 2).is_none() {
            return Vec::new();
        }
        self.locators
            .range((*node, [0; 32])..=(*node, [0xff; 32]))
            .map(|(_, l)| l)
            .collect()
    }

    /// The locator a record carried for `node` in the subnet `anchor`
    /// names.
    pub fn locator_in(&self, node: &Keyhash, anchor: &Keyhash) -> Option<&Locator> {
        self.table.distance(&self.me, node, 2)?;
        self.locators.get(&(*node, *anchor))
    }

    /// Every node this client can place without asking anyone, inside the
    /// horizon and no wider.
    pub fn resolvable(&self) -> Vec<Keyhash> {
        let inside = self.table.horizon(&self.me, 2);
        let mut out: Vec<Keyhash> = self
            .places
            .keys()
            .map(|(n, _)| *n)
            .filter(|n| inside.contains(n))
            .collect();
        out.dedup();
        out
    }

    /// The parties one adoption or sibling edge away.
    ///
    /// **Named because the flow metric asks for it**: an evaluator builds
    /// its graph from the edges it holds, and the first ring is where a
    /// client's own evidence is densest.
    pub fn adjacent(&self) -> Vec<Keyhash> {
        self.table
            .horizon(&self.me, 1)
            .into_iter()
            .filter(|k| *k != self.me)
            .collect()
    }

    /// How many adoption or sibling edges away `other` is; nothing beyond
    /// the horizon.
    pub fn distance(&self, other: &Keyhash) -> Option<usize> {
        self.table.distance(&self.me, other, 2)
    }

    /// Drop what has left the horizon (`light-client-requirements.md`
    /// §4.2), and say how many parties were forgotten.  A copy that only
    /// ever grew would become a record of people this client has no reason
    /// to know anything about.
    ///
    /// The records go with the locators: keeping the transactions of a
    /// party dropped from the table would put them back on the next
    /// rebuild.
    pub fn prune(&mut self) -> usize {
        let inside = self.table.horizon(&self.me, 2);
        let parties = |m: &BTreeMap<Placement, Place>| {
            m.keys()
                .map(|(n, _)| *n)
                .collect::<std::collections::BTreeSet<_>>()
                .len()
        };
        let before = parties(&self.places);
        self.places.retain(|(n, _), _| inside.contains(n));
        self.locators.retain(|(n, _), _| inside.contains(n));
        self.endpoints.retain(|(n, _), _| inside.contains(n));
        self.delegations
            .retain(|n, _| inside.contains(n) || *n == self.me);
        self.records.retain(|_, b| {
            Record::parse(b).is_ok_and(|r| {
                r.participants().iter().any(|p| inside.contains(p))
                    || r.field_hash(1).is_some_and(|k| inside.contains(&k))
                    || r.field_hash(2).is_some_and(|k| inside.contains(&k))
            })
        });
        before - parties(&self.places)
    }

    /// The derived shape as it goes to local storage, with the watermark
    /// that says which records produced it.
    pub fn materialise(&self) -> Snapshot {
        let mut at: Vec<(u64, Txid)> = self
            .records
            .values()
            .filter_map(|b| Record::parse(b).ok().map(|r| (r.effective, r.txid)))
            .collect();
        at.sort();
        let folded = rhtn_archive::topology::fold_digest(at.iter().map(|(_, t)| t));
        let mut out = Vec::new();
        rhtn_codec::encode::emit_array_head(&mut out, 3);
        rhtn_codec::encode::emit_bstr(&mut out, &self.table.materialise());
        rhtn_codec::encode::emit_bstr(&mut out, &encode_places(&self.places, &self.locators));
        // **the addresses go with the shape** (`light-client-requirements.md`
        // §4.2): the copy exists to route around a patron that is not
        // answering, and a restored horizon holding positions and no
        // endpoints cannot.  Endpoint records are not transactions, so a
        // replay cannot rebuild them — if they are not written here they
        // are gone until the patron floods them again.
        rhtn_codec::encode::emit_bstr(&mut out, &encode_lines(&self.endpoints, &self.conflicts));
        Snapshot {
            folded,
            high: at.last().copied(),
            table: out,
        }
    }

    /// Wake: take the snapshot where it can account for the records held,
    /// and fold in only what arrived since.  Anything else replays.
    ///
    /// **The failure is slow, never wrong.**  A snapshot from another
    /// client, one whose bytes are damaged, or one whose watermark cannot
    /// be squared with what is held, all land on the same answer a fold
    /// from nothing gives.
    pub fn wake<L: Lookup + ?Sized>(&mut self, snap: Option<&Snapshot>, ids: &L) -> Woke {
        let mut all: Vec<Vec<u8>> = self.records.values().cloned().collect();
        all.sort_by_key(|b| {
            Record::parse(b)
                .map(|r| (r.effective, r.txid))
                .unwrap_or_default()
        });
        let taken = snap.and_then(|s| {
            let later = unfolded(s, &all, |b| {
                Record::parse(b)
                    .map(|r| (r.effective, r.txid))
                    .unwrap_or_default()
            })?;
            self.take(s)?;
            Some(
                later
                    .into_iter()
                    .map(|i| all[i].clone())
                    .collect::<Vec<_>>(),
            )
        });
        match taken {
            Some(later) => {
                for bytes in &later {
                    if let Ok(rec) = Record::parse(bytes)
                        && self
                            .table
                            .apply_with(&rec, ids, &self.records, None, Evaluation::Deferred)
                            .is_ok()
                    {
                        self.note_locator(&rec);
                        self.note_end(&rec);
                    }
                }
                if later.is_empty() {
                    Woke::Current
                } else {
                    Woke::Extended {
                        folded: later.len(),
                    }
                }
            }
            None => {
                self.table = Table::with_me(self.me);
                self.places.clear();
                self.locators.clear();
                for bytes in &all {
                    if let Ok(rec) = Record::parse(bytes)
                        && self
                            .table
                            .apply_with(&rec, ids, &self.records, None, Evaluation::Deferred)
                            .is_ok()
                    {
                        self.note_locator(&rec);
                        self.note_end(&rec);
                    }
                }
                Woke::Replayed {
                    replayed: all.len(),
                }
            }
        }
    }

    /// Install a snapshot's table and locators.
    fn take(&mut self, snap: &Snapshot) -> Option<()> {
        let item = rhtn_codec::cbor::parse_all(&snap.table).ok()?;
        let rhtn_codec::cbor::Item::Array(parts) = &item else {
            return None;
        };
        let [
            rhtn_codec::cbor::Item::Bytes(t),
            rhtn_codec::cbor::Item::Bytes(l),
            rhtn_codec::cbor::Item::Bytes(e),
        ] = parts.as_slice()
        else {
            return None;
        };
        let mut table = Table::from_materialised(&snap.table[t.clone()])?;
        let (places, locators) = decode_places(&snap.table[l.clone()])?;
        let (endpoints, conflicts) = decode_lines(&snap.table[e.clone()])?;
        // whose horizon this is, is this client's own answer and never a
        // snapshot's: a copy naming somebody else is one to discard
        if table.me != Some(self.me) {
            return None;
        }
        table.prefer = self.table.prefer.take();
        self.table = table;
        self.places = places;
        self.locators = locators;
        self.endpoints = endpoints;
        self.conflicts = conflicts;
        Some(())
    }

    /// The records held, for a caller that persists them beside the
    /// snapshot.
    pub fn stored(&self) -> impl Iterator<Item = (&Txid, &Vec<u8>)> {
        self.records.iter()
    }

    /// Put a record back without folding it: what a load does before it
    /// wakes.
    pub fn restore_record(&mut self, bytes: Vec<u8>) -> bool {
        match Record::parse(&bytes) {
            Ok(r) => self.records.insert(r.txid, bytes).is_none(),
            Err(_) => false,
        }
    }
}

/// One row per node placed: the place, and the locator where a record
/// carried one.  An empty locator field is a node placed as an anchor and
/// nothing more.
/// **The row shape is unchanged by the key.**  It already carried the
/// party and the anchor as separate fields, because a place is a path
/// relative to one; keying by both puts two rows where there was one and
/// leaves what a row says alone.
/// The endpoint lines and the pairs an equivocation retired, as they go to
/// local storage (`wire-format.md` §10.1.2).
///
/// **The retirements travel with the lines.** A restored client that took
/// the addresses and forgot which pairs were retired would accept a
/// conflicting record it had already ruled out.
fn encode_lines(
    endpoints: &BTreeMap<(Keyhash, u32), EndpointRecord>,
    conflicts: &BTreeSet<(Keyhash, u32, u32)>,
) -> Vec<u8> {
    use rhtn_codec::encode::*;
    let mut out = Vec::new();
    emit_array_head(&mut out, 2);
    emit_array_head(&mut out, endpoints.len());
    for e in endpoints.values() {
        emit_bstr(&mut out, &e.bytes);
    }
    emit_array_head(&mut out, conflicts.len());
    for (n, series, counter) in conflicts {
        emit_array_head(&mut out, 3);
        emit_bstr(&mut out, n);
        emit_uint(&mut out, *series as u64);
        emit_uint(&mut out, *counter as u64);
    }
    out
}

type Lines = (
    BTreeMap<(Keyhash, u32), EndpointRecord>,
    BTreeSet<(Keyhash, u32, u32)>,
);

fn decode_lines(b: &[u8]) -> Option<Lines> {
    use rhtn_codec::cbor::{Item, parse_all};
    let item = parse_all(b).ok()?;
    let Item::Array(parts) = &item else {
        return None;
    };
    let [Item::Array(recs), Item::Array(cs)] = parts.as_slice() else {
        return None;
    };
    let mut endpoints = BTreeMap::new();
    for r in recs {
        let Item::Bytes(range) = r else { return None };
        let er = EndpointRecord::parse(&b[range.clone()]).ok()?;
        endpoints.insert((er.node, er.seqno.series), er);
    }
    let mut conflicts = BTreeSet::new();
    for c in cs {
        let Item::Array(f) = c else { return None };
        let [Item::Bytes(n), Item::Uint(series), Item::Uint(counter)] = f.as_slice() else {
            return None;
        };
        let node: Keyhash = b[n.clone()].try_into().ok()?;
        conflicts.insert((
            node,
            u32::try_from(*series).ok()?,
            u32::try_from(*counter).ok()?,
        ));
    }
    Some((endpoints, conflicts))
}

fn encode_places(
    places: &BTreeMap<Placement, Place>,
    locators: &BTreeMap<Placement, Locator>,
) -> Vec<u8> {
    use rhtn_codec::encode::*;
    let mut out = Vec::new();
    emit_array_head(&mut out, places.len());
    for (k, p) in places {
        let (node, _) = k;
        emit_array_head(&mut out, 5);
        emit_bstr(&mut out, node);
        emit_bstr(&mut out, &p.anchor);
        emit_bstr(&mut out, &p.path);
        emit_uint(&mut out, p.nibbles);
        match locators.get(k) {
            Some(loc) => {
                let mut one = Vec::new();
                loc.emit(&mut one);
                emit_bstr(&mut out, &one);
            }
            None => emit_bstr(&mut out, &[]),
        }
    }
    out
}

type Placed = (BTreeMap<Placement, Place>, BTreeMap<Placement, Locator>);

fn decode_places(b: &[u8]) -> Option<Placed> {
    use rhtn_codec::cbor::{Item, parse_all};
    let item = parse_all(b).ok()?;
    let Item::Array(rows) = &item else {
        return None;
    };
    let (mut places, mut locators) = (BTreeMap::new(), BTreeMap::new());
    for row in rows {
        let Item::Array(f) = row else { return None };
        let [
            Item::Bytes(k),
            Item::Bytes(a),
            Item::Bytes(p),
            Item::Uint(n),
            Item::Bytes(l),
        ] = f.as_slice()
        else {
            return None;
        };
        let node: Keyhash = b[k.clone()].try_into().ok()?;
        let anchor: Keyhash = b[a.clone()].try_into().ok()?;
        places.insert(
            (node, anchor),
            Place {
                anchor,
                path: b[p.clone()].to_vec(),
                nibbles: *n,
            },
        );
        if !l.is_empty() {
            locators.insert((node, anchor), Locator::decode(&b[l.clone()]).ok()?);
        }
    }
    Some((places, locators))
}
