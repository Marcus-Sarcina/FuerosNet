//! What a subject does with its own lines (design §9.0; `wire-format.md`
//! §4.6; `light-client-requirements.md` §2): seal them as the old key's
//! last act when rotating, seal and then reissue on suspicion of
//! compromise, never into a series it has occupied, and keep the chain
//! that proves the series it is in.

use crate::Keyhash;
use rhtn_archive::Txid;
use rhtn_archive::chain::Archive;
use rhtn_archive::locator::{COUNTER_MAX, seal};
use rhtn_archive::record::Record;
use rhtn_archive::series::SeriesChain;
use rhtn_archive::tx::{Locator, Seqno, TYPE_REISSUE, reissue_body};
use rhtn_crypto::SigningIdentity;

/// One patron relationship as the subject's archive shows it: the patron,
/// the position the adoption gave, and the chain proving the series.
#[derive(Debug, Clone)]
pub struct Relationship {
    pub patron: Keyhash,
    pub position: Locator,
    pub chain: SeriesChain,
}

impl Relationship {
    pub fn series(&self) -> u32 {
        self.chain.current()
    }
}

/// Every relationship `archive`'s key holds: one per patron it was
/// adopted under.
pub fn relationships(archive: &Archive) -> Vec<Relationship> {
    let me = archive.key;
    let mut patrons: Vec<Keyhash> = archive
        .records()
        .filter(|r| r.tx_type == rhtn_archive::tx::TYPE_ADOPTION && r.field_hash(1) == Some(me))
        .filter_map(|r| r.field_hash(2))
        .collect();
    patrons.sort();
    patrons.dedup();
    patrons
        .into_iter()
        .filter_map(|p| {
            let chain = archive.chain_for(&p)?;
            let position = chain.adoption.locator()?;
            Some(Relationship {
                patron: p,
                position,
                chain,
            })
        })
        .collect()
}

/// A seal for one relationship: the self-signed locator at the maximum
/// counter of the series the subject is in there.
pub fn seal_line(identity: &SigningIdentity, rel: &Relationship) -> Vec<u8> {
    seal(identity, &rel.position, rel.series())
}

/// Seal every line at once (`light-client-requirements.md` §2): one
/// self-signed locator per relationship, no patron needed.
pub fn seal_all(identity: &SigningIdentity, archive: &Archive) -> Vec<(Keyhash, Vec<u8>)> {
    relationships(archive)
        .iter()
        .map(|r| (r.patron, seal_line(identity, r)))
        .collect()
}

/// Why a reissue is not taken.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Refusal {
    /// A series this key has occupied before (`wire-format.md` §4.6).
    SeriesOccupied(u32),
    /// No relationship with that patron.
    NoRelationship,
    /// The countersigned record is not the reissue proposed.
    NotTheProposal,
}

/// A series this key has never occupied, from `candidates` in order.
pub fn fresh_series(archive: &Archive, candidates: impl IntoIterator<Item = u32>) -> Option<u32> {
    let used = archive.series_occupied();
    candidates.into_iter().find(|s| !used.contains(s))
}

/// The reissue body a subject proposes to `patron` (`wire-format.md`
/// §4.6): leaving the current series at the counter it reached, entering
/// `new_series` at 0.  The patron's back-pointers are the patron's to
/// supply; the countersignature is theirs too.  Refused into any series
/// this key has occupied.
pub fn propose_reissue(
    archive: &Archive,
    patron: &Keyhash,
    patron_back: &[Txid],
    leaving_counter: u32,
    new_series: u32,
    timestamp: u64,
) -> Result<Vec<u8>, Refusal> {
    if archive.series_occupied().contains(&new_series) {
        return Err(Refusal::SeriesOccupied(new_series));
    }
    let rel = relationships(archive)
        .into_iter()
        .find(|r| r.patron == *patron)
        .ok_or(Refusal::NoRelationship)?;
    let back = archive.next_back_pointers();
    Ok(reissue_body(
        [&back, patron_back],
        &archive.key,
        patron,
        Seqno {
            series: rel.series(),
            counter: leaving_counter,
        },
        new_series,
        timestamp,
    ))
}

/// One relationship's repair on suspected compromise (`light-client-requirements.md`
/// §2): the seal, produced first, and the reissue proposed after it naming
/// the sealed counter.
#[derive(Debug, Clone)]
pub struct Repair {
    pub patron: Keyhash,
    pub seal: Vec<u8>,
    pub reissue: Vec<u8>,
    pub new_series: u32,
}

/// Act on suspicion: for every relationship, in order, seal the line at the
/// maximum counter, then propose the reissue naming that maximum into a
/// fresh series.  `patron_back` supplies each patron's back-pointers and
/// `fresh` the series to try, first unused taken.
pub fn suspect_compromise(
    identity: &SigningIdentity,
    archive: &Archive,
    timestamp: u64,
    patron_back: &dyn Fn(&Keyhash) -> Vec<Txid>,
    fresh: &[u32],
) -> Result<Vec<Repair>, Refusal> {
    let mut out = Vec::new();
    let mut taken: Vec<u32> = Vec::new();
    for rel in relationships(archive) {
        let seal = seal_line(identity, &rel);
        let new_series = fresh_series(
            archive,
            fresh.iter().copied().filter(|s| !taken.contains(s)),
        )
        .ok_or(Refusal::SeriesOccupied(0))?;
        taken.push(new_series);
        let reissue = propose_reissue(
            archive,
            &rel.patron,
            &patron_back(&rel.patron),
            COUNTER_MAX,
            new_series,
            timestamp,
        )?;
        out.push(Repair {
            patron: rel.patron,
            seal,
            reissue,
            new_series,
        });
    }
    Ok(out)
}

/// Take the patron-countersigned reissue back: it must be the body
/// proposed, verified; then it is this key's own transaction and extends
/// the chain.
pub fn take_reissue<L: rhtn_crypto::verify::Lookup + ?Sized>(
    archive: &mut Archive,
    proposed_body: &[u8],
    envelope: &[u8],
    ids: &L,
) -> Result<Txid, Refusal> {
    let rec = Record::parse(envelope).map_err(|_| Refusal::NotTheProposal)?;
    if rec.tx_type != TYPE_REISSUE || &rec.bytes[rec.body.clone()] != proposed_body {
        return Err(Refusal::NotTheProposal);
    }
    rhtn_crypto::verify::envelope(ids, envelope).map_err(|_| Refusal::NotTheProposal)?;
    let t = rec.txid;
    archive.append(rec).map_err(|_| Refusal::NotTheProposal)?;
    Ok(t)
}

/// A rotation from `old` to a new key (design §9.0, §9.0.1): the old key
/// signs what it must sign, seals every line as its last act, and is then
/// gone from this client.  Whether the inheritance is carried is the
/// subject's choice made elsewhere; here is only the order.
pub struct Rotation {
    /// Boxed: a signing identity carries its expanded post-quantum key
    /// inline, and a value this size does not belong on a stack frame.
    old: Option<Box<SigningIdentity>>,
    archive: Archive,
    pub seals: Vec<(Keyhash, Vec<u8>)>,
}

impl Rotation {
    pub fn begin(old: Box<SigningIdentity>, archive: Archive) -> Self {
        Rotation {
            old: Some(old),
            archive,
            seals: Vec::new(),
        }
    }

    /// The old key while it is still in hand: for the successor statement
    /// of a recovery, and for nothing after the seals.
    pub fn old_key(&self) -> Option<&SigningIdentity> {
        self.old.as_deref()
    }

    /// The head of the old archive, for field 7 of the adoption.
    pub fn old_head(&self) -> Option<Txid> {
        self.archive.newest()
    }

    /// Seal every old line as the old key's last act.  After this the old
    /// key signs nothing: it is dropped here.
    pub fn seal(&mut self) -> &[(Keyhash, Vec<u8>)] {
        if let Some(old) = self.old.take() {
            self.seals = seal_all(&old, &self.archive);
        }
        &self.seals
    }

    pub fn sealed(&self) -> bool {
        self.old.is_none()
    }
}
