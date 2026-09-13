//! The four things a client hands the node serving it (`wire-format.md`
//! §7.10): its prekey bundle, one-time keys for its pool, payload to carry,
//! and where to ring it.  Each is its own request type, each is answered
//! with the nonce it carried and a code, and the shape of what the node
//! then delivers for a relayed message is here too, since the node
//! composes it.

use crate::Keyhash;
use rhtn_codec::cbor::*;
use rhtn_codec::encode::*;
use rhtn_codec::schema::{self, Family};


/// Request types 9 to 12 (`wire-format.md` §7.10, §9.2's table).
pub const REQUEST_PREKEY_PUBLICATION: u64 = 9;
pub const REQUEST_ONE_TIME_DEPOSIT: u64 = 10;
pub const REQUEST_RELAY: u64 = 11;
pub const REQUEST_WAKE: u64 = 12;

/// A client publishing its own bundle to the node that serves it
/// (`wire-format.md` §7.10).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PrekeyPublication {
    pub bundle: Vec<u8>,
    pub nonce: [u8; 16],
}

/// A client stocking its node's pool.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OneTimeDeposit {
    pub keys: Vec<Vec<u8>>,
    pub nonce: [u8; 16],
}

/// A client handing its node payload to carry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RelaySubmission {
    pub recipient: Keyhash,
    pub ciphertext: Vec<u8>,
    pub nonce: [u8; 16],
}

/// A client registering, refreshing or withdrawing a wake endpoint
/// (design §14.1.5).  **No endpoint withdraws**: opting out is as sayable
/// as opting in, and the key and the lapse describe an endpoint, so
/// neither may be carried without one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WakeRegistration {
    pub nonce: [u8; 16],
    pub endpoint: Option<String>,
    pub key: Option<Vec<u8>>,
    pub lapses_at: Option<u64>,
}

/// The answer to any of the four: the nonce and a code, and nothing
/// further.  Every reason a refusal could give is about capacity or about
/// somebody else.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubmissionReply {
    pub nonce: [u8; 16],
    pub code: u64,
}

pub const SUBMISSION_ACCEPTED: u64 = 0;
pub const SUBMISSION_REFUSED: u64 = 1;
pub const SUBMISSION_OVER_BOUND: u64 = 2;

impl PrekeyPublication {
    pub fn encode(&self) -> Vec<u8> {
        let mut out = Vec::new();
        emit_map_head(&mut out, 2);
        emit_uint(&mut out, 1);
        // the bundle is the object `wire-format.md` §7.10 names, spliced in as
        // §7.8 encoded it — not a byte string wrapping it, which is what a
        // conformant peer would refuse
        out.extend_from_slice(&self.bundle);
        emit_uint(&mut out, 2);
        emit_bstr(&mut out, &self.nonce);
        out
    }

    pub fn decode(b: &[u8]) -> Result<Self, String> {
        let item = parse_all(b).map_err(|e| e.0)?;
        schema::check_unsigned(Family::PrekeyPublication, b, 0).map_err(|e| e.0)?;
        let Item::Map(m) = &item else { return Err("not a map".into()) };
        let bundle = value_slice(b, 1).ok_or("field 1")?;
        Ok(PrekeyPublication { bundle: b[bundle].to_vec(), nonce: nonce_at(b, m, 2)? })
    }
}

impl OneTimeDeposit {
    pub fn encode(&self) -> Vec<u8> {
        let mut out = Vec::new();
        emit_map_head(&mut out, 2);
        emit_uint(&mut out, 1);
        emit_array_head(&mut out, self.keys.len());
        for k in &self.keys {
            emit_bstr(&mut out, k);
        }
        emit_uint(&mut out, 2);
        emit_bstr(&mut out, &self.nonce);
        out
    }

    pub fn decode(b: &[u8]) -> Result<Self, String> {
        let item = parse_all(b).map_err(|e| e.0)?;
        schema::check_unsigned(Family::OneTimeDeposit, b, 0).map_err(|e| e.0)?;
        let Item::Map(m) = &item else { return Err("not a map".into()) };
        let Some(Item::Array(a)) = map_get(m, 1) else { return Err("field 1".into()) };
        // each key's content, not the item that carries it
        let mut keys = Vec::with_capacity(a.len());
        for k in a {
            let Item::Bytes(r) = k else { return Err("a one-time key is a byte string".into()) };
            keys.push(b[r.clone()].to_vec());
        }
        Ok(OneTimeDeposit { keys, nonce: nonce_at(b, m, 2)? })
    }
}

impl RelaySubmission {
    pub fn encode(&self) -> Vec<u8> {
        let mut out = Vec::new();
        emit_map_head(&mut out, 3);
        emit_uint(&mut out, 1);
        emit_bstr(&mut out, &self.recipient);
        emit_uint(&mut out, 2);
        emit_bstr(&mut out, &self.ciphertext);
        emit_uint(&mut out, 3);
        emit_bstr(&mut out, &self.nonce);
        out
    }

    pub fn decode(b: &[u8]) -> Result<Self, String> {
        let item = parse_all(b).map_err(|e| e.0)?;
        schema::check_unsigned(Family::RelaySubmission, b, 0).map_err(|e| e.0)?;
        let Item::Map(m) = &item else { return Err("not a map".into()) };
        let recipient: Keyhash = bytes_at(b, m, 1)?.try_into().map_err(|_| "recipient")?;
        Ok(RelaySubmission { recipient, ciphertext: bytes_at(b, m, 2)?, nonce: nonce_at(b, m, 3)? })
    }
}

/// Where a client asks to be rung, as it hands it over (design §14.1.5):
/// a URL the client obtained from a service its user picked, and the key
/// the posted body is encrypted to.  The node holds both and reads
/// neither beyond posting to the one and encrypting to the other.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WakeEndpoint {
    pub url: String,
    pub key: Vec<u8>,
    /// When the client expects the endpoint to stop working, if it knows.
    pub lapses_at: Option<u64>,
}

impl WakeRegistration {
    /// Register or refresh `endpoint`, or withdraw where there is none.
    pub fn of(nonce: [u8; 16], endpoint: Option<WakeEndpoint>) -> WakeRegistration {
        match endpoint {
            Some(e) => WakeRegistration { nonce, endpoint: Some(e.url), key: Some(e.key), lapses_at: e.lapses_at },
            None => WakeRegistration::withdrawal(nonce),
        }
    }

    /// Withdraw whatever endpoint the node holds.
    pub fn withdrawal(nonce: [u8; 16]) -> WakeRegistration {
        WakeRegistration { nonce, endpoint: None, key: None, lapses_at: None }
    }

    pub fn withdraws(&self) -> bool {
        self.endpoint.is_none()
    }

    pub fn encode(&self) -> Vec<u8> {
        let n = 1 + self.endpoint.is_some() as usize + self.key.is_some() as usize + self.lapses_at.is_some() as usize;
        let mut out = Vec::new();
        emit_map_head(&mut out, n);
        emit_uint(&mut out, 1);
        emit_bstr(&mut out, &self.nonce);
        if let Some(e) = &self.endpoint {
            emit_uint(&mut out, 2);
            emit_tstr(&mut out, e);
        }
        if let Some(k) = &self.key {
            emit_uint(&mut out, 3);
            emit_bstr(&mut out, k);
        }
        if let Some(t) = self.lapses_at {
            emit_uint(&mut out, 4);
            emit_uint(&mut out, t);
        }
        out
    }

    pub fn decode(b: &[u8]) -> Result<Self, String> {
        let item = parse_all(b).map_err(|e| e.0)?;
        schema::check_unsigned(Family::WakeRegistration, b, 0).map_err(|e| e.0)?;
        let Item::Map(m) = &item else { return Err("not a map".into()) };
        let endpoint = match map_get(m, 2) {
            Some(Item::Text(r)) => Some(String::from_utf8(b[r.clone()].to_vec()).map_err(|_| "endpoint not utf-8")?),
            _ => None,
        };
        let key = map_get(m, 3).and_then(|_| bytes_at(b, m, 3).ok());
        // the endpoint, its key and its lapse stand or fall together, and
        // the decoder above has already refused the shapes where they do
        // not (`wire-format.md` §7.10)
        let lapses_at = map_get(m, 4).and_then(as_uint);
        Ok(WakeRegistration { nonce: nonce_at(b, m, 1)?, endpoint, key, lapses_at })
    }
}

impl SubmissionReply {
    pub fn accepted(nonce: [u8; 16]) -> SubmissionReply {
        SubmissionReply { nonce, code: SUBMISSION_ACCEPTED }
    }

    pub fn code(nonce: [u8; 16], code: u64) -> SubmissionReply {
        SubmissionReply { nonce, code }
    }

    pub fn encode(&self) -> Vec<u8> {
        let mut out = Vec::new();
        emit_map_head(&mut out, 2);
        emit_uint(&mut out, 1);
        emit_bstr(&mut out, &self.nonce);
        emit_uint(&mut out, 2);
        emit_uint(&mut out, self.code);
        out
    }

    pub fn decode(b: &[u8]) -> Result<Self, String> {
        let item = parse_all(b).map_err(|e| e.0)?;
        schema::check_unsigned(Family::SubmissionReply, b, 0).map_err(|e| e.0)?;
        let Item::Map(m) = &item else { return Err("not a map".into()) };
        Ok(SubmissionReply { nonce: nonce_at(b, m, 1)?, code: map_get(m, 2).and_then(as_uint).ok_or("code")? })
    }
}

fn bytes_at(b: &[u8], m: &[(Item, Item)], key: u64) -> Result<Vec<u8>, String> {
    match map_get(m, key) {
        Some(Item::Bytes(r)) => Ok(b[r.clone()].to_vec()),
        _ => Err(format!("field {key}")),
    }
}

fn nonce_at(b: &[u8], m: &[(Item, Item)], key: u64) -> Result<[u8; 16], String> {
    bytes_at(b, m, key)?.try_into().map_err(|_| format!("field {key} is not a 16-byte nonce"))
}

/// What a node delivers for a submission (`wire-format.md` §7.10): the
/// submitter in front of the ciphertext, so a recipient holding material
/// for many peers can choose which to try.  **A routing hint, not an
/// attribution**: the name is the node's assertion, and what a message is
/// attributed to is decided by the material it opens under.
pub fn relayed(from: Keyhash, ciphertext: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    emit_array_head(&mut out, 2);
    emit_bstr(&mut out, &from);
    emit_bstr(&mut out, ciphertext);
    out
}

/// Split what `relayed` composed; nothing where the bytes are not that.
pub fn unrelayed(b: &[u8]) -> Option<(Keyhash, Vec<u8>)> {
    let item = parse_all(b).ok()?;
    let Item::Array(parts) = &item else { return None };
    let [Item::Bytes(f), Item::Bytes(p)] = parts.as_slice() else { return None };
    Some((b[f.clone()].try_into().ok()?, b[p.clone()].to_vec()))
}
