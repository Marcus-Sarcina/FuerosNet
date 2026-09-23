//! What a device writes to its own storage and reads back at the next
//! start: the CBOR shapes the client's live state takes on the way to disk,
//! and the readers that take nothing partial.
//!
//! **This is the device's own storage, not the backup** (design §13.7.1,
//! §23.3).  The payload material and the sessions are a device's: a
//! session is with a device, and material moved to another device would
//! part the ratchet at the first message.  So they survive a restart here
//! and never travel in the portable envelope, which carries the identity,
//! the records and the capture store.
//!
//! A blob that does not decode whole is refused whole: a client that
//! restored five of six parts would hold sessions it cannot advance or an
//! archive with no store, and would be quietly worse than one starting
//! from nothing that says so.

use rhtn_codec::cbor::{Item, parse_all};
use rhtn_codec::encode::*;

/// The raw bytes an item refers to, where it is a byte string.
pub(crate) fn bytes(b: &[u8], it: &Item) -> Option<Vec<u8>> {
    match it {
        Item::Bytes(r) => Some(b[r.clone()].to_vec()),
        _ => None,
    }
}

pub(crate) fn fixed<const N: usize>(b: &[u8], it: &Item) -> Option<[u8; N]> {
    bytes(b, it)?.try_into().ok()
}

pub(crate) fn uint(it: &Item) -> Option<u64> {
    match it {
        Item::Uint(u) => Some(*u),
        _ => None,
    }
}

pub(crate) fn array(it: &Item) -> Option<&Vec<Item>> {
    match it {
        Item::Array(a) => Some(a),
        _ => None,
    }
}

/// `null` or a value.
pub(crate) fn optional<T>(it: &Item, f: impl FnOnce(&Item) -> Option<T>) -> Option<Option<T>> {
    match it {
        Item::Null => Some(None),
        other => f(other).map(Some),
    }
}

pub(crate) fn emit_opt_bstr(out: &mut Vec<u8>, b: Option<&[u8]>) {
    match b {
        Some(b) => emit_bstr(out, b),
        None => emit_null(out),
    }
}

pub(crate) fn emit_opt_uint(out: &mut Vec<u8>, u: Option<u64>) {
    match u {
        Some(u) => emit_uint(out, u),
        None => emit_null(out),
    }
}

/// Parse a blob as an array of exactly `n` items.
pub(crate) fn parse_array(b: &[u8], n: usize) -> Option<(Item, Vec<Item>)> {
    let item = parse_all(b).ok()?;
    let arr = array(&item)?.clone();
    if arr.len() != n {
        return None;
    }
    Some((item, arr))
}
