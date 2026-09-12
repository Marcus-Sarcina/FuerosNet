//! The chain that proves which series a node is in (`wire-format.md`
//! §4.6, §4.6.1; `light-client-requirements.md` §2): its adoption under a
//! patron and every reissue since, each countersigned by that patron.  Held
//! by the subject and presented on request; a holder who takes one knows
//! the current series and the abandoned ones, and rejects a reissue that
//! repeats a series in it.

use crate::Keyhash;
use crate::record::{Record, SigStatus};
use crate::tx::{TYPE_ADOPTION, TYPE_REISSUE};
use rhtn_crypto::verify::Lookup;
use std::collections::BTreeSet;

/// One relationship's chain: the adoption and the reissues since, in
/// order.
#[derive(Debug, Clone)]
pub struct SeriesChain {
    pub node: Keyhash,
    pub patron: Keyhash,
    pub adoption: Record,
    pub reissues: Vec<Record>,
}

/// How two presented chains stand to each other (§4.6.1: chain length is
/// the order; divergence is patron equivocation).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Ranking {
    /// The same chain.
    Same,
    /// This chain extends the other: this one is current.
    Extends,
    /// The other extends this one: the other is current.
    Extended,
    /// Two successors countersigned by the same patron.
    Diverge,
    /// Different adoptions: not one chain.
    Unrelated,
}

impl SeriesChain {
    /// Open a chain at its adoption, which must verify.
    pub fn from_adoption<L: Lookup + ?Sized>(rec: &Record, ids: &L) -> Result<Self, String> {
        if rec.check_signatures(ids) != SigStatus::Verified {
            return Err("adoption does not verify".into());
        }
        Self::open(rec)
    }

    /// Open a chain at an adoption already held as verified: a subject's
    /// own.
    pub fn open(rec: &Record) -> Result<Self, String> {
        if rec.tx_type != TYPE_ADOPTION {
            return Err("not an adoption".into());
        }
        let node = rec.field_hash(1).ok_or("node")?;
        let patron = rec.field_hash(2).ok_or("patron")?;
        rec.seqno().ok_or("series")?;
        Ok(SeriesChain { node, patron, adoption: rec.clone(), reissues: Vec::new() })
    }

    /// Take a reissue that must verify.
    pub fn take_reissue<L: Lookup + ?Sized>(&mut self, rec: &Record, ids: &L) -> Result<(), String> {
        if rec.check_signatures(ids) != SigStatus::Verified {
            return Err("reissue does not verify".into());
        }
        self.extend(rec)
    }

    /// Extend the chain by a reissue already held as verified: it must be
    /// this relationship's, leave the current series, and enter one the
    /// chain has never held (§4.6).
    pub fn extend(&mut self, rec: &Record) -> Result<(), String> {
        if rec.tx_type != TYPE_REISSUE {
            return Err("not a series reissue".into());
        }
        if rec.field_hash(1) != Some(self.node) || rec.field_hash(2) != Some(self.patron) {
            return Err("another relationship's reissue".into());
        }
        let left = rec.seqno_left().ok_or("field 3")?;
        if left.series != self.current() {
            return Err("leaves a series other than the current one".into());
        }
        let entered = rec.seqno().ok_or("field 4")?;
        if self.series().contains(&entered.series) {
            return Err("names a series already in the chain".into());
        }
        // §3.3's monotonicity, over the predecessor the chain carries: a
        // reissue's back-pointers name the record it follows, so the same
        // rule the archive applies on append applies to a presentation.
        // Without it a chain the archive would refuse advances a locator,
        // since `take_chain` proves the new series and abandons the old.
        let prev = self.reissues.last().unwrap_or(&self.adoption);
        if rec.time < prev.effective {
            return Err("a reissue timed before its predecessor's effective time".into());
        }
        self.reissues.push(rec.clone());
        Ok(())
    }

    /// The series the chain is in now.
    pub fn current(&self) -> u32 {
        self.reissues.last().and_then(|r| r.seqno()).or_else(|| self.adoption.seqno()).map(|s| s.series).expect("opened with a series")
    }

    /// Every series the chain has been in, first to current.
    pub fn series(&self) -> Vec<u32> {
        let mut v: Vec<u32> = self.adoption.seqno().map(|s| s.series).into_iter().collect();
        v.extend(self.reissues.iter().filter_map(|r| r.seqno()).map(|s| s.series));
        v
    }

    /// The series left behind: a holder rejects records in any of them
    /// whatever their counter (§4.6).
    pub fn abandoned(&self) -> BTreeSet<u32> {
        let cur = self.current();
        self.series().into_iter().filter(|s| *s != cur).collect()
    }

    /// The adoption and the reissues: what the chain discloses.
    pub fn depth(&self) -> usize {
        1 + self.reissues.len()
    }

    pub fn records(&self) -> Vec<&Record> {
        std::iter::once(&self.adoption).chain(self.reissues.iter()).collect()
    }

    /// The chain as it is presented: the adoption's envelope, then each
    /// reissue's.
    pub fn bytes(&self) -> Vec<Vec<u8>> {
        self.records().into_iter().map(|r| r.bytes.clone()).collect()
    }

    /// Read a presented chain: the adoption first, then each reissue in
    /// order, every one verified under the keys held.
    pub fn from_records<L: Lookup + ?Sized>(records: &[Vec<u8>], ids: &L) -> Result<Self, String> {
        let mut it = records.iter();
        let first = Record::parse(it.next().ok_or("an empty presentation")?)?;
        let mut chain = Self::from_adoption(&first, ids)?;
        for b in it {
            chain.take_reissue(&Record::parse(b)?, ids)?;
        }
        Ok(chain)
    }

    /// Rank this chain against another presented for the same node.
    pub fn rank(&self, other: &SeriesChain) -> Ranking {
        if self.adoption.txid != other.adoption.txid {
            return Ranking::Unrelated;
        }
        let (a, b): (Vec<_>, Vec<_>) = (self.reissues.iter().map(|r| r.txid).collect(), other.reissues.iter().map(|r| r.txid).collect());
        let common = a.iter().zip(&b).take_while(|(x, y)| x == y).count();
        match (common == a.len(), common == b.len()) {
            (true, true) => Ranking::Same,
            (false, true) => Ranking::Extends,
            (true, false) => Ranking::Extended,
            (false, false) => Ranking::Diverge,
        }
    }
}
