//! The resource objects the network carries (`wire-format.md` §6, §11;
//! design §11.4, §11.5, §11.6): the scope vocabulary, the owner-signed
//! catalog entry, the resource-signed abuse report, the query and its
//! answer, the registration and its reply, and the request a hosting node
//! evaluates with its response.  Encodings only; what a node does with
//! them is `rhtn-node`'s, what a client does `rhtn-client`'s.

use crate::Keyhash;
use rhtn_codec::cbor::*;
use rhtn_codec::encode::*;
use rhtn_codec::schema::{self, Family};
use rhtn_crypto::SigningIdentity;
use rhtn_crypto::verify::{self, Lookup};

/// Stream-1 request types (`wire-format.md` §9.2).
pub const REQUEST_CATALOG_QUERY: u64 = 5;
/// Request frame 6: a request to a resource.
pub const REQUEST_RESOURCE: u64 = 6;
/// Request frame 7: registering a resource.
pub const REQUEST_REGISTRATION: u64 = 7;

/// `ResourceRegistrationReply` field 2.
pub const REGISTRATION_RECORDED: u64 = 0;
/// The registration was refused.
pub const REGISTRATION_REFUSED: u64 = 1;

/// `ResourceResponse` field 1, in the normative evaluation order's terms.
pub const STATUS_DELIVERED: u64 = 0;
/// Refused: the owner's rule did not admit the requester.
pub const STATUS_REFUSED: u64 = 1;
/// Unavailable: the resource exists but did not answer.
pub const STATUS_UNAVAILABLE: u64 = 2;
/// Malformed: the request did not read.
pub const STATUS_MALFORMED: u64 = 3;
/// No acknowledgement from the resource.
pub const STATUS_NO_ACK: u64 = 4;
/// The requester holds no role the entry's rule names.
pub const STATUS_NO_ROLE: u64 = 5;

/// `AbuseReport` field 3.
pub const ABUSE_UNAVAILABLE: u64 = 0;
/// Malfunction.
pub const ABUSE_MALFUNCTION: u64 = 1;
/// Excessive load.
pub const ABUSE_EXCESSIVE_LOAD: u64 = 2;
/// An unauthorised access attempt.
pub const ABUSE_UNAUTHORISED_ACCESS: u64 = 3;
/// Content.
pub const ABUSE_CONTENT: u64 = 4;
/// Anything else.
pub const ABUSE_OTHER: u64 = 5;

/// A scope (design §11.4; `wire-format.md` §6.6): a region relative to the
/// owner.  Tag 3 is retired and never decoded.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Scope {
    /// The owner alone: `self`.
    Own,
    /// Tag 1: the owner and everything `n` levels below it.
    Down(u64),
    /// Tag 2: the owner and everything `n` levels above it.
    Up(u64),
    /// Tag 4: the owner's siblings under one patron.
    Siblings,
    /// Tag 5: the owner's Dunbar neighbourhood.
    Dunbar,
    /// Tag 6: exactly these keyhashes. **Ascending with no duplicates**
    /// (`wire-format.md` §4): the list is signed, so a decoder that took an
    /// unordered or repeating one would accept bytes another decoder rejects.
    List(Vec<Keyhash>),
}

impl Scope {
    /// Emit this scope into `out` (`wire-format.md` §4): a bare `uint` for the
    /// forms that take no argument, `[tag, n]` for those that take a depth, and
    /// `[6, [...]]` for a list.
    pub fn emit(&self, out: &mut Vec<u8>) {
        match self {
            Scope::Own => emit_uint(out, 0),
            Scope::Down(n) => {
                emit_array_head(out, 2);
                emit_uint(out, 1);
                emit_uint(out, *n);
            }
            Scope::Up(n) => {
                emit_array_head(out, 2);
                emit_uint(out, 2);
                emit_uint(out, *n);
            }
            Scope::Siblings => emit_uint(out, 4),
            Scope::Dunbar => emit_uint(out, 5),
            Scope::List(ks) => {
                emit_array_head(out, 2);
                emit_uint(out, 6);
                emit_array_head(out, ks.len());
                for k in ks {
                    emit_bstr(out, k);
                }
            }
        }
    }

    /// This scope's own bytes.
    pub fn encode(&self) -> Vec<u8> {
        let mut out = Vec::new();
        self.emit(&mut out);
        out
    }

    /// Read a scope from its item, with the bytes it indexes.
    pub fn read(b: &[u8], item: &Item) -> Result<Scope, String> {
        schema::check_kind(b, "Scope", item).map_err(|e| e.0.to_string())?;
        match item {
            Item::Uint(0) => Ok(Scope::Own),
            Item::Uint(4) => Ok(Scope::Siblings),
            Item::Uint(5) => Ok(Scope::Dunbar),
            Item::Uint(_) => Err("scope tag".into()),
            Item::Array(a) if a.len() == 2 => match (as_uint(&a[0]), &a[1]) {
                (Some(1), Item::Uint(n)) => Ok(Scope::Down(*n)),
                (Some(2), Item::Uint(n)) => Ok(Scope::Up(*n)),
                (Some(6), Item::Array(list)) => {
                    let mut ks = Vec::new();
                    for k in list {
                        match k {
                            Item::Bytes(r) if r.len() == 32 => {
                                ks.push(<[u8; 32]>::try_from(&b[r.clone()]).unwrap())
                            }
                            _ => return Err("scope list entry".into()),
                        }
                    }
                    Ok(Scope::List(ks))
                }
                _ => Err("scope form".into()),
            },
            _ => Err("scope shape".into()),
        }
    }

    /// A scope from its own encoded bytes.
    pub fn decode(b: &[u8]) -> Result<Scope, String> {
        let item = parse_all(b).map_err(|e| e.0)?;
        Scope::read(b, &item)
    }
}

/// The fields an owner signs (`wire-format.md` §6.1).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EntryFields {
    /// Field 1: the resource's identity, which is also the entry's signer.
    pub resource: Keyhash,
    /// Field 3: the service type, DNS-SD style and **matched byte for byte**
    /// (`wire-format.md` §6.1): no case folding, no normalisation, no subtype
    /// grammar. A registry of conventional names may grow socially; the protocol
    /// compares bytes.
    pub service_type: String,
    /// Field 4: the instance name, **for display only**. Never matched against
    /// and never unique.
    pub instance: String,
    /// Field 5: the connection endpoint, the SRV analogue.
    pub endpoint: Vec<u8>,
    /// Field 6: `connect_scope`, **advisory**. Absent offers no prediction,
    /// neither allow-all nor deny-all; the owner decides at request time
    /// regardless (`wire-format.md` §11).
    pub connect_scope: Option<Scope>,
    /// Field 7: additional service metadata, the TXT analogue.
    pub metadata: Option<Vec<u8>>,
    /// Field 9: the owner's declared logging and retention practice.
    pub data_practice: Option<u64>,
}

/// A `CatalogEntry` as read: its fields, and its bytes, which are what a
/// host stores and serves unchanged.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CatalogEntry {
    /// Field 1: the resource's identity, which is also the signer.
    pub resource: Keyhash,
    /// Field 2: the owner.
    pub owner: Keyhash,
    /// Field 3: the service type, matched byte for byte.
    pub service_type: String,
    /// Field 4: the instance name, for display only.
    pub instance: String,
    /// Field 5: the connection endpoint.
    pub endpoint: Vec<u8>,
    /// Field 6: `connect_scope`, advisory.
    pub connect_scope: Option<Scope>,
    /// Field 7: additional service metadata.
    pub metadata: Option<Vec<u8>>,
    /// Field 9: the declared data practice.
    pub data_practice: Option<u64>,
    /// The entry's own bytes, kept because the signature covers them and a
    /// re-encoding is not what was signed.
    pub bytes: Vec<u8>,
}

fn emit_entry_fields(out: &mut Vec<u8>, owner: &Keyhash, f: &EntryFields, sig: Option<&[u8]>) {
    let n = 5
        + f.connect_scope.is_some() as usize
        + f.metadata.is_some() as usize
        + f.data_practice.is_some() as usize
        + sig.is_some() as usize;
    emit_map_head(out, n);
    emit_uint(out, 1);
    emit_bstr(out, &f.resource);
    emit_uint(out, 2);
    emit_bstr(out, owner);
    emit_uint(out, 3);
    emit_tstr(out, &f.service_type);
    emit_uint(out, 4);
    emit_tstr(out, &f.instance);
    emit_uint(out, 5);
    emit_bstr(out, &f.endpoint);
    if let Some(s) = &f.connect_scope {
        emit_uint(out, 6);
        s.emit(out);
    }
    if let Some(m) = &f.metadata {
        emit_uint(out, 7);
        emit_bstr(out, m);
    }
    if let Some(s) = sig {
        emit_uint(out, 8);
        out.extend_from_slice(s);
    }
    if let Some(d) = f.data_practice {
        emit_uint(out, 9);
        emit_uint(out, d);
    }
}

impl CatalogEntry {
    /// Build and sign an entry: the owner's classical signature over fields
    /// 1 to 7 and 9 under the catalog tag, in slot 8.
    pub fn build(owner: &SigningIdentity, f: &EntryFields) -> Vec<u8> {
        let mut payload = Vec::new();
        emit_entry_fields(&mut payload, &owner.public.keyhash, f, None);
        let sig = owner.sign1_ed_unnamed(rhtn_codec::cose::aad::CATALOG, &payload);
        let mut out = Vec::new();
        emit_entry_fields(&mut out, &owner.public.keyhash, f, Some(&sig));
        out
    }

    /// Parse an entry and verify the resource's signature over it.
    pub fn parse(b: &[u8]) -> Result<Self, String> {
        let item = parse_all(b).map_err(|e| e.0)?;
        schema::check_kind(b, "CatalogEntry", &item).map_err(|e| e.0.to_string())?;
        let Item::Map(m) = &item else {
            return Err("not a map".into());
        };
        let kh = |k: u64| -> Result<Keyhash, String> {
            match map_get(m, k) {
                Some(Item::Bytes(r)) if r.len() == 32 => {
                    Ok(<[u8; 32]>::try_from(&b[r.clone()]).unwrap())
                }
                _ => Err(format!("field {k}")),
            }
        };
        let text = |k: u64| -> Result<String, String> {
            match map_get(m, k) {
                Some(Item::Text(r)) => {
                    String::from_utf8(b[r.clone()].to_vec()).map_err(|_| format!("field {k} utf-8"))
                }
                _ => Err(format!("field {k}")),
            }
        };
        let bytes = |k: u64| -> Option<Vec<u8>> {
            match map_get(m, k) {
                Some(Item::Bytes(r)) => Some(b[r.clone()].to_vec()),
                _ => None,
            }
        };
        let connect_scope = match map_get(m, 6) {
            Some(it) => Some(Scope::read(b, it)?),
            None => None,
        };
        Ok(CatalogEntry {
            resource: kh(1)?,
            owner: kh(2)?,
            service_type: text(3)?,
            instance: text(4)?,
            endpoint: bytes(5).ok_or("field 5")?,
            connect_scope,
            metadata: bytes(7),
            data_practice: map_get(m, 9).and_then(as_uint),
            bytes: b.to_vec(),
        })
    }

    /// Signed by the owner it names.
    pub fn verify<L: Lookup + ?Sized>(&self, ids: &L) -> Result<(), String> {
        verify::record(ids, "CatalogEntry", &self.bytes).map_err(|e| e.to_string())
    }
}

/// An `AbuseReport` (`wire-format.md` §6.3): by the resource it names.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AbuseReport {
    /// Field 1: the resource, **and the signer**. A resource reports; its owner
    /// receives (design §11.6).
    pub resource: Keyhash,
    /// Field 2: when it occurred.
    pub occurred_at: u64,
    /// Field 3: the category, one of the `ABUSE_*` constants above.
    pub category: u64,
    /// Field 4: detail, resource-defined and uninterpreted by the network.
    /// **Bounded and deliberately small**: a signed object is portable, so what
    /// it carries travels further than its recipient.
    pub detail: Option<Vec<u8>>,
    /// The report's own bytes, which the signature covers.
    pub bytes: Vec<u8>,
}

impl AbuseReport {
    /// Build and sign a report as `resource`.
    pub fn build(
        resource: &SigningIdentity,
        occurred_at: u64,
        category: u64,
        detail: Option<&[u8]>,
    ) -> Vec<u8> {
        let emit = |out: &mut Vec<u8>, sig: Option<&[u8]>| {
            emit_map_head(out, 3 + detail.is_some() as usize + sig.is_some() as usize);
            emit_uint(out, 1);
            emit_bstr(out, &resource.public.keyhash);
            emit_uint(out, 2);
            emit_uint(out, occurred_at);
            emit_uint(out, 3);
            emit_uint(out, category);
            if let Some(d) = detail {
                emit_uint(out, 4);
                emit_bstr(out, d);
            }
            if let Some(s) = sig {
                emit_uint(out, 5);
                out.extend_from_slice(s);
            }
        };
        let mut payload = Vec::new();
        emit(&mut payload, None);
        let sig = resource.sign1_ed_unnamed(rhtn_codec::cose::aad::ABUSE, &payload);
        let mut out = Vec::new();
        emit(&mut out, Some(&sig));
        out
    }

    /// Parse a report and verify the resource's signature over it.
    pub fn parse(b: &[u8]) -> Result<Self, String> {
        let item = parse_all(b).map_err(|e| e.0)?;
        schema::check_kind(b, "AbuseReport", &item).map_err(|e| e.0.to_string())?;
        let Item::Map(m) = &item else {
            return Err("not a map".into());
        };
        let resource = match map_get(m, 1) {
            Some(Item::Bytes(r)) if r.len() == 32 => <[u8; 32]>::try_from(&b[r.clone()]).unwrap(),
            _ => return Err("field 1".into()),
        };
        let detail = match map_get(m, 4) {
            Some(Item::Bytes(r)) => Some(b[r.clone()].to_vec()),
            _ => None,
        };
        Ok(AbuseReport {
            resource,
            occurred_at: map_get(m, 2).and_then(as_uint).ok_or("field 2")?,
            category: map_get(m, 3).and_then(as_uint).ok_or("field 3")?,
            detail,
            bytes: b.to_vec(),
        })
    }

    /// Signed by the key whose keyhash is field 1, and no other.
    pub fn verify<L: Lookup + ?Sized>(&self, ids: &L) -> Result<(), String> {
        verify::record(ids, "AbuseReport", &self.bytes).map_err(|e| e.to_string())
    }
}

fn nonce_at(b: &[u8], m: &[(Item, Item)], k: u64) -> Result<[u8; 16], String> {
    match map_get(m, k) {
        Some(Item::Bytes(r)) if r.len() == 16 => Ok(<[u8; 16]>::try_from(&b[r.clone()]).unwrap()),
        _ => Err(format!("field {k} nonce")),
    }
}

/// A `CatalogQuery` (`wire-format.md` §6.4).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CatalogQuery {
    /// The service type asked for, or every type where absent.
    pub service_type: Option<String>,
    /// The nonce the reply must echo.
    pub nonce: [u8; 16],
}

impl CatalogQuery {
    /// The query's bytes.
    pub fn encode(&self) -> Vec<u8> {
        let mut out = Vec::new();
        emit_map_head(&mut out, 1 + self.service_type.is_some() as usize);
        if let Some(t) = &self.service_type {
            emit_uint(&mut out, 1);
            emit_tstr(&mut out, t);
        }
        emit_uint(&mut out, 2);
        emit_bstr(&mut out, &self.nonce);
        out
    }

    /// A query from its bytes.
    pub fn decode(b: &[u8]) -> Result<Self, String> {
        parse_all(b).map_err(|e| e.0)?;
        schema::check_unsigned(Family::CatalogQuery, b, 0).map_err(|e| e.0.to_string())?;
        let item = parse_all(b).map_err(|e| e.0)?;
        let Item::Map(m) = &item else {
            return Err("not a map".into());
        };
        let service_type = match map_get(m, 1) {
            Some(Item::Text(r)) => {
                Some(String::from_utf8(b[r.clone()].to_vec()).map_err(|_| "field 1 utf-8")?)
            }
            _ => None,
        };
        Ok(CatalogQuery {
            service_type,
            nonce: nonce_at(b, m, 2)?,
        })
    }
}

/// A `CatalogReply`: the entries byte-for-byte, and the continuation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CatalogReply {
    /// The nonce, echoed from the query.
    pub nonce: [u8; 16],
    /// The entries, each as its own signed bytes.
    pub entries: Vec<Vec<u8>>,
    /// Where to resume, where the answer did not fit.
    pub continuation: Option<String>,
}

impl CatalogReply {
    /// The reply's bytes.
    pub fn encode(&self) -> Vec<u8> {
        let mut out = Vec::new();
        emit_map_head(&mut out, 2 + self.continuation.is_some() as usize);
        emit_uint(&mut out, 1);
        emit_bstr(&mut out, &self.nonce);
        emit_uint(&mut out, 2);
        emit_array_head(&mut out, self.entries.len());
        for e in &self.entries {
            out.extend_from_slice(e);
        }
        if let Some(c) = &self.continuation {
            emit_uint(&mut out, 3);
            emit_tstr(&mut out, c);
        }
        out
    }

    /// A reply from its bytes.
    pub fn decode(b: &[u8]) -> Result<Self, String> {
        parse_all(b).map_err(|e| e.0)?;
        schema::check_unsigned(Family::CatalogReply, b, 0).map_err(|e| e.0.to_string())?;
        let item = parse_all(b).map_err(|e| e.0)?;
        let Item::Map(m) = &item else {
            return Err("not a map".into());
        };
        let r2 = value_slice(b, 2).ok_or("field 2")?;
        let entries = array_item_ranges(b, r2.start)
            .ok_or("entries")?
            .into_iter()
            .map(|r| b[r].to_vec())
            .collect();
        let continuation = match map_get(m, 3) {
            Some(Item::Text(r)) => {
                Some(String::from_utf8(b[r.clone()].to_vec()).map_err(|_| "field 3 utf-8")?)
            }
            _ => None,
        };
        Ok(CatalogReply {
            nonce: nonce_at(b, m, 1)?,
            entries,
            continuation,
        })
    }
}

/// A `ResourceRegistration` (`wire-format.md` §6.2).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResourceRegistration {
    /// The signed entry being registered.
    pub entry: Vec<u8>,
    /// The `discover_scope` the owner asks for. **It travels in this direction
    /// only** (`wire-format.md` §6.6): it decides which askers an answering node
    /// returns the entry to, and is never carried in the entry itself, since
    /// telling an asker how it was selected is what that would do.
    pub scope: Option<Scope>,
    /// The nonce the reply must echo.
    pub nonce: [u8; 16],
}

impl ResourceRegistration {
    /// The registration's bytes.
    pub fn encode(&self) -> Vec<u8> {
        let mut out = Vec::new();
        emit_map_head(&mut out, 2 + self.scope.is_some() as usize);
        emit_uint(&mut out, 1);
        out.extend_from_slice(&self.entry);
        if let Some(s) = &self.scope {
            emit_uint(&mut out, 2);
            s.emit(&mut out);
        }
        emit_uint(&mut out, 3);
        emit_bstr(&mut out, &self.nonce);
        out
    }

    /// A registration from its bytes.
    pub fn decode(b: &[u8]) -> Result<Self, String> {
        parse_all(b).map_err(|e| e.0)?;
        schema::check_unsigned(Family::ResourceRegistration, b, 0).map_err(|e| e.0.to_string())?;
        let item = parse_all(b).map_err(|e| e.0)?;
        let Item::Map(m) = &item else {
            return Err("not a map".into());
        };
        let entry = value_slice(b, 1).map(|r| b[r].to_vec()).ok_or("field 1")?;
        let scope = match map_get(m, 2) {
            Some(it) => Some(Scope::read(b, it)?),
            None => None,
        };
        Ok(ResourceRegistration {
            entry,
            scope,
            nonce: nonce_at(b, m, 3)?,
        })
    }
}

/// A `ResourceRegistrationReply`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegistrationReply {
    /// The nonce, echoed from the registration.
    pub nonce: [u8; 16],
    /// Recorded or refused: one of the `REGISTRATION_*` constants above.
    pub code: u64,
}

impl RegistrationReply {
    /// The reply's bytes.
    pub fn encode(&self) -> Vec<u8> {
        let mut out = Vec::new();
        emit_map_head(&mut out, 2);
        emit_uint(&mut out, 1);
        emit_bstr(&mut out, &self.nonce);
        emit_uint(&mut out, 2);
        emit_uint(&mut out, self.code);
        out
    }

    /// A reply from its bytes.
    pub fn decode(b: &[u8]) -> Result<Self, String> {
        parse_all(b).map_err(|e| e.0)?;
        schema::check_unsigned(Family::ResourceRegistrationReply, b, 0)
            .map_err(|e| e.0.to_string())?;
        let item = parse_all(b).map_err(|e| e.0)?;
        let Item::Map(m) = &item else {
            return Err("not a map".into());
        };
        Ok(RegistrationReply {
            nonce: nonce_at(b, m, 1)?,
            code: map_get(m, 2).and_then(as_uint).ok_or("field 2")?,
        })
    }
}

/// A `ResourceRequest` (`wire-format.md` §11): the resource, and the
/// application request as an HTTP/1.1 message.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResourceRequest {
    /// The resource addressed.
    pub resource: Keyhash,
    /// The request's body, uninterpreted by the network.
    pub message: Vec<u8>,
}

impl ResourceRequest {
    /// The request's bytes.
    pub fn encode(&self) -> Vec<u8> {
        let mut out = Vec::new();
        emit_map_head(&mut out, 2);
        emit_uint(&mut out, 1);
        emit_bstr(&mut out, &self.resource);
        emit_uint(&mut out, 2);
        emit_bstr(&mut out, &self.message);
        out
    }

    /// A request from its bytes.
    pub fn decode(b: &[u8]) -> Result<Self, String> {
        parse_all(b).map_err(|e| e.0)?;
        schema::check_unsigned(Family::ResourceRequest, b, 0).map_err(|e| e.0.to_string())?;
        let item = parse_all(b).map_err(|e| e.0)?;
        let Item::Map(m) = &item else {
            return Err("not a map".into());
        };
        let resource = match map_get(m, 1) {
            Some(Item::Bytes(r)) if r.len() == 32 => <[u8; 32]>::try_from(&b[r.clone()]).unwrap(),
            _ => return Err("field 1".into()),
        };
        let message = match map_get(m, 2) {
            Some(Item::Bytes(r)) => b[r.clone()].to_vec(),
            _ => return Err("field 2".into()),
        };
        Ok(ResourceRequest { resource, message })
    }
}

/// A `ResourceResponse`: the code, and the application response iff
/// delivered.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResourceResponse {
    /// The outcome, one of the `STATUS_*` constants above.
    pub status: u64,
    /// The resource's answer, where it delivered one.
    pub body: Option<Vec<u8>>,
}

impl ResourceResponse {
    /// A response carrying `status` and no body.
    pub fn code(status: u64) -> Self {
        ResourceResponse { status, body: None }
    }

    /// The response's bytes.
    pub fn encode(&self) -> Vec<u8> {
        let mut out = Vec::new();
        emit_map_head(&mut out, 1 + self.body.is_some() as usize);
        emit_uint(&mut out, 1);
        emit_uint(&mut out, self.status);
        if let Some(b) = &self.body {
            emit_uint(&mut out, 2);
            emit_bstr(&mut out, b);
        }
        out
    }

    /// A response from its bytes.
    pub fn decode(b: &[u8]) -> Result<Self, String> {
        parse_all(b).map_err(|e| e.0)?;
        schema::check_unsigned(Family::ResourceResponse, b, 0).map_err(|e| e.0.to_string())?;
        let item = parse_all(b).map_err(|e| e.0)?;
        let Item::Map(m) = &item else {
            return Err("not a map".into());
        };
        let body = match map_get(m, 2) {
            Some(Item::Bytes(r)) => Some(b[r.clone()].to_vec()),
            _ => None,
        };
        Ok(ResourceResponse {
            status: map_get(m, 1).and_then(as_uint).ok_or("field 1")?,
            body,
        })
    }
}
