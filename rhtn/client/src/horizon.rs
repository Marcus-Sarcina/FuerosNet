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
use rhtn_archive::record::Record;
use rhtn_archive::topology::{Evaluation, Snapshot, Table, unfolded};
use rhtn_archive::tx::Locator;
use rhtn_crypto::verify::Lookup;
use std::collections::BTreeMap;

/// What a wake did with the snapshot it found.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Woke {
    /// The snapshot was the record set's value: nothing was folded.
    Current,
    Extended { folded: usize },
    Replayed { replayed: usize },
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
    places: BTreeMap<Keyhash, Place>,
    /// The locator as propagated, for the nodes a record placed.  A party
    /// this client only knows the position of, because a subordinate's
    /// record named it as an anchor, is in `places` and not here.
    locators: BTreeMap<Keyhash, Locator>,
}

impl Horizon {
    pub fn new(me: Keyhash) -> Horizon {
        Horizon { me, records: BTreeMap::new(), table: Table::with_me(me), places: BTreeMap::new(), locators: BTreeMap::new() }
    }

    pub fn me(&self) -> Keyhash {
        self.me
    }

    pub fn holds(&self, txid: &Txid) -> bool {
        self.records.contains_key(txid)
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
        let Ok(rec) = Record::parse(bytes) else { return Took::Refused };
        if self.records.contains_key(&rec.txid) {
            return Took::Duplicate;
        }
        if self.table.apply_with(&rec, ids, &self.records, None, Evaluation::Deferred).is_err() {
            return Took::Refused;
        }
        self.note_locator(&rec);
        self.records.insert(rec.txid, bytes.to_vec());
        Took::Applied
    }

    /// An adoption names where its subject sits, and that is the whole of
    /// what a client needs to reach it (`wire-format.md` §2.3).  A later
    /// one for the same party replaces the earlier, which is what makes
    /// the propagated version the system of record here.
    fn note_locator(&mut self, rec: &Record) {
        let (Some(node), Some(loc)) = (rec.field_hash(1), rec.locator()) else { return };
        self.places.insert(node, Place { anchor: loc.anchor, path: loc.path.clone(), nibbles: loc.nibbles });
        // the anchor a path is relative to sits at the empty path under
        // itself: a fact the record states rather than one derived from it
        self.places.entry(loc.anchor).or_insert(Place { anchor: loc.anchor, path: Vec::new(), nibbles: 0 });
        self.locators.insert(node, loc);
    }

    /// Where `node` sits, for a resolution this client runs itself.
    pub fn place(&self, node: &Keyhash) -> Option<&Place> {
        self.places.get(node)
    }

    /// The locator a record carried for `node`, series and counter
    /// included.  A node this client places only as an anchor has none.
    pub fn locator(&self, node: &Keyhash) -> Option<&Locator> {
        self.locators.get(node)
    }

    /// Every node this client can place without asking anyone.
    pub fn resolvable(&self) -> Vec<Keyhash> {
        self.places.keys().copied().collect()
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
        let before = self.places.len();
        self.places.retain(|k, _| inside.contains(k));
        self.locators.retain(|k, _| inside.contains(k));
        self.records.retain(|_, b| {
            Record::parse(b).is_ok_and(|r| r.participants().iter().any(|p| inside.contains(p)) || r.field_hash(1).is_some_and(|k| inside.contains(&k)) || r.field_hash(2).is_some_and(|k| inside.contains(&k)))
        });
        before - self.places.len()
    }

    /// The derived shape as it goes to local storage, with the watermark
    /// that says which records produced it.
    pub fn materialise(&self) -> Snapshot {
        let mut at: Vec<(u64, Txid)> = self.records.values().filter_map(|b| Record::parse(b).ok().map(|r| (r.effective, r.txid))).collect();
        at.sort();
        let mut out = Vec::new();
        rhtn_codec::encode::emit_array_head(&mut out, 2);
        rhtn_codec::encode::emit_bstr(&mut out, &self.table.materialise());
        rhtn_codec::encode::emit_bstr(&mut out, &encode_places(&self.places, &self.locators));
        Snapshot { records: at.len() as u64, high: at.last().copied(), table: out }
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
        all.sort_by_key(|b| Record::parse(b).map(|r| (r.effective, r.txid)).unwrap_or_default());
        let taken = snap.and_then(|s| {
            let later = unfolded(s, &all, |b| Record::parse(b).map(|r| (r.effective, r.txid)).unwrap_or_default())?;
            self.take(s)?;
            Some(later.into_iter().map(|i| all[i].clone()).collect::<Vec<_>>())
        });
        match taken {
            Some(later) => {
                for bytes in &later {
                    if let Ok(rec) = Record::parse(bytes)
                        && self.table.apply_with(&rec, ids, &self.records, None, Evaluation::Deferred).is_ok()
                    {
                        self.note_locator(&rec);
                    }
                }
                if later.is_empty() { Woke::Current } else { Woke::Extended { folded: later.len() } }
            }
            None => {
                self.table = Table::with_me(self.me);
                self.places.clear();
                self.locators.clear();
                for bytes in &all {
                    if let Ok(rec) = Record::parse(bytes)
                        && self.table.apply_with(&rec, ids, &self.records, None, Evaluation::Deferred).is_ok()
                    {
                        self.note_locator(&rec);
                    }
                }
                Woke::Replayed { replayed: all.len() }
            }
        }
    }

    /// Install a snapshot's table and locators.
    fn take(&mut self, snap: &Snapshot) -> Option<()> {
        let item = rhtn_codec::cbor::parse_all(&snap.table).ok()?;
        let rhtn_codec::cbor::Item::Array(parts) = &item else { return None };
        let [rhtn_codec::cbor::Item::Bytes(t), rhtn_codec::cbor::Item::Bytes(l)] = parts.as_slice() else { return None };
        let mut table = Table::from_materialised(&snap.table[t.clone()])?;
        let (places, locators) = decode_places(&snap.table[l.clone()])?;
        // whose horizon this is, is this client's own answer and never a
        // snapshot's: a copy naming somebody else is one to discard
        if table.me != Some(self.me) {
            return None;
        }
        table.prefer = self.table.prefer.take();
        self.table = table;
        self.places = places;
        self.locators = locators;
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
fn encode_places(places: &BTreeMap<Keyhash, Place>, locators: &BTreeMap<Keyhash, Locator>) -> Vec<u8> {
    use rhtn_codec::encode::*;
    let mut out = Vec::new();
    emit_array_head(&mut out, places.len());
    for (k, p) in places {
        emit_array_head(&mut out, 5);
        emit_bstr(&mut out, k);
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

type Placed = (BTreeMap<Keyhash, Place>, BTreeMap<Keyhash, Locator>);

fn decode_places(b: &[u8]) -> Option<Placed> {
    use rhtn_codec::cbor::{Item, parse_all};
    let item = parse_all(b).ok()?;
    let Item::Array(rows) = &item else { return None };
    let (mut places, mut locators) = (BTreeMap::new(), BTreeMap::new());
    for row in rows {
        let Item::Array(f) = row else { return None };
        let [Item::Bytes(k), Item::Bytes(a), Item::Bytes(p), Item::Uint(n), Item::Bytes(l)] = f.as_slice() else { return None };
        let key: Keyhash = b[k.clone()].try_into().ok()?;
        places.insert(key, Place { anchor: b[a.clone()].try_into().ok()?, path: b[p.clone()].to_vec(), nibbles: *n });
        if !l.is_empty() {
            locators.insert(key, Locator::decode(&b[l.clone()]).ok()?);
        }
    }
    Some((places, locators))
}

