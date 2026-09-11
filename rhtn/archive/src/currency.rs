//! Currency attestations as an inquirer reads them (`wire-format.md` §7.1,
//! design §9.0.2): parse and verify one, assess a set for a fork, and
//! notify both patrons when assertions diverge.

use crate::Keyhash;
use rhtn_codec::cbor::*;
use rhtn_crypto::verify::{self, Lookup};
use std::collections::BTreeSet;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Attestation {
    pub subject: Keyhash,
    pub current: Keyhash,
    pub issued_at: u64,
    pub expires_at: u64,
    pub role: u64,
    pub issuer: Keyhash,
    pub bytes: Vec<u8>,
}

fn kh(b: &[u8], m: &[(Item, Item)], k: u64) -> Option<Keyhash> {
    match map_get(m, k) {
        Some(Item::Bytes(r)) if r.len() == 32 => b[r.clone()].try_into().ok(),
        _ => None,
    }
}

/// Parse and verify an attestation under its named issuer.
pub fn parse_attestation<L: Lookup + ?Sized>(ids: &L, bytes: &[u8]) -> Result<Attestation, String> {
    let item = parse_all(bytes).map_err(|e| e.0)?;
    rhtn_codec::schema::check_kind(bytes, "CurrencyAttestation", &item).map_err(|e| e.0)?;
    if !verify::record(ids, "CurrencyAttestation", bytes).map_err(|e| e.to_string())? {
        return Err("issuer signature fails".into());
    }
    let Item::Map(m) = &item else { return Err("map".into()) };
    Ok(Attestation {
        subject: kh(bytes, m, 1).ok_or("subject")?,
        current: kh(bytes, m, 2).ok_or("current")?,
        issued_at: map_get(m, 3).and_then(as_uint).ok_or("issued_at")?,
        expires_at: map_get(m, 4).and_then(as_uint).ok_or("expires_at")?,
        role: map_get(m, 5).and_then(as_uint).ok_or("role")?,
        issuer: kh(bytes, m, 6).ok_or("issuer")?,
        bytes: bytes.to_vec(),
    })
}

/// The attestation a `CurrencyReply` carries, if its code says one follows.
pub fn reply_attestation(reply: &[u8]) -> Option<Vec<u8>> {
    let Ok(item) = parse_all(reply) else { return None };
    let Item::Map(m) = &item else { return None };
    if map_get(m, 2).and_then(as_uint) != Some(0) {
        return None;
    }
    value_slice(reply, 3).map(|r| reply[r].to_vec())
}

/// What an inquirer holds about one identity after asking.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CurrencyView {
    NoAnswer,
    Attested(Attestation),
    /// Divergent assertions, all retained; none chosen (design §9.0.2).
    Fork(Vec<Attestation>),
}

/// Assess the attestations received for one subject.
pub fn assess(atts: Vec<Attestation>) -> CurrencyView {
    let keys: BTreeSet<Keyhash> = atts.iter().map(|a| a.current).collect();
    match keys.len() {
        0 => CurrencyView::NoAnswer,
        1 => CurrencyView::Attested(atts.into_iter().next().unwrap()),
        _ => CurrencyView::Fork(atts),
    }
}

/// A conforming inquirer who receives divergent assertions notifies both
/// patrons (design §9.0.2).  The notice's content is unspecified (design
/// §22.3); `notify` is handed each issuer once.  Returns how many were told.
pub fn notify_fork(view: &CurrencyView, mut notify: impl FnMut(&Keyhash)) -> usize {
    let CurrencyView::Fork(atts) = view else { return 0 };
    let issuers: BTreeSet<Keyhash> = atts.iter().map(|a| a.issuer).collect();
    for i in &issuers {
        notify(i);
    }
    issuers.len()
}
