//! The node-key objects the network carries (`wire-format.md` §7.8): the
//! key a serving node holds for one recipient, the two request forms,
//! structurally distinct, and the replies.  The service that holds and
//! serves them is `rhtn-node`'s; the key itself is `rhtn-crypto`'s.

use crate::Keyhash;
use rhtn_codec::cbor::*;
use rhtn_codec::encode::*;
use rhtn_codec::schema::{self, Family};
use rhtn_crypto::node_key::NodeKeyFields;
use rhtn_crypto::verify::{self, Lookup};

/// Stream-1 request type for both request forms (`wire-format.md` §9.2).
pub const REQUEST_NODE_KEY: u64 = 3;
/// `NodeKeyReply` field 3: not a recipient this node serves.
pub const FAIL_NOT_SERVED: u64 = 0;
/// `NodeKeyReply` field 3: refused.
pub const FAIL_REFUSED: u64 = 1;

/// A `NodeKey` as the network reads it: its fields and its own bytes,
/// which the signature covers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NodeKey {
    /// The fields, as the node stated them.
    pub fields: NodeKeyFields,
    /// The record's bytes.
    pub bytes: Vec<u8>,
}

impl NodeKey {
    /// Read one whose shape the codec accepts; the signature is
    /// [`NodeKey::verify`]'s.
    pub fn parse(b: &[u8]) -> Result<NodeKey, String> {
        let fields = NodeKeyFields::read(b).ok_or("not a node key")?;
        Ok(NodeKey {
            fields,
            bytes: b.to_vec(),
        })
    }

    /// Signed by the node field 2 names.
    pub fn verify<L: Lookup + ?Sized>(&self, ids: &L) -> Result<(), String> {
        verify::record(ids, "NodeKey", &self.bytes).map_err(|e| e.to_string())
    }
}

/// The two request forms, structurally distinct (`wire-format.md` §7.8):
/// one recipient, visibly targeted, or a population, visibly a sweep.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NodeKeyRequest {
    /// One recipient's newest key.
    One {
        /// Whose node key is wanted.
        recipient: Keyhash,
        /// The nonce the reply must echo.
        nonce: [u8; 16],
    },
    /// A population swept, two to 256, in ascending keyhash order.
    Batch {
        /// The recipients, ascending.
        recipients: Vec<Keyhash>,
        /// The nonce every reply echoes.
        nonce: [u8; 16],
    },
}

impl NodeKeyRequest {
    /// The request for `recipients`: targeted where there is one, a sweep
    /// where there are more, sorted and deduplicated either way. None for
    /// an empty or oversized population.
    pub fn for_recipients(mut recipients: Vec<Keyhash>, nonce: [u8; 16]) -> Option<Self> {
        recipients.sort();
        recipients.dedup();
        match recipients.len() {
            0 => None,
            1 => Some(NodeKeyRequest::One {
                recipient: recipients[0],
                nonce,
            }),
            2..=256 => Some(NodeKeyRequest::Batch { recipients, nonce }),
            _ => None,
        }
    }

    /// The nonce the reply echoes.
    pub fn nonce(&self) -> [u8; 16] {
        match self {
            NodeKeyRequest::One { nonce, .. } | NodeKeyRequest::Batch { nonce, .. } => *nonce,
        }
    }

    /// The recipients asked about, in the order replies come back.
    pub fn recipients(&self) -> Vec<Keyhash> {
        match self {
            NodeKeyRequest::One { recipient, .. } => vec![*recipient],
            NodeKeyRequest::Batch { recipients, .. } => recipients.clone(),
        }
    }

    /// The request's bytes.
    pub fn encode(&self) -> Vec<u8> {
        let mut out = Vec::new();
        emit_map_head(&mut out, 2);
        emit_uint(&mut out, 1);
        match self {
            NodeKeyRequest::One { recipient, .. } => emit_bstr(&mut out, recipient),
            NodeKeyRequest::Batch { recipients, .. } => {
                emit_array_head(&mut out, recipients.len());
                for r in recipients {
                    emit_bstr(&mut out, r);
                }
            }
        }
        emit_uint(&mut out, 2);
        emit_bstr(&mut out, &self.nonce());
        out
    }

    /// A request from its bytes.
    pub fn decode(b: &[u8]) -> Result<Self, String> {
        let item = parse_all(b).map_err(|e| e.0)?;
        schema::check_unsigned(Family::NodeKeyRequestOrBatch, b, 0).map_err(|e| e.0)?;
        let Item::Map(m) = &item else {
            return Err("not a map".into());
        };
        let nonce = match map_get(m, 2) {
            Some(Item::Bytes(r)) => b[r.clone()].try_into().map_err(|_| "nonce")?,
            _ => return Err("nonce".into()),
        };
        Ok(match map_get(m, 1) {
            Some(Item::Array(a)) => NodeKeyRequest::Batch {
                recipients: a
                    .iter()
                    .map(|k| match k {
                        Item::Bytes(r) => b[r.clone()].try_into().map_err(|_| "keyhash"),
                        _ => Err("keyhash"),
                    })
                    .collect::<Result<_, _>>()?,
                nonce,
            },
            Some(Item::Bytes(r)) => NodeKeyRequest::One {
                recipient: b[r.clone()].try_into().map_err(|_| "keyhash")?,
                nonce,
            },
            _ => return Err("field 1".into()),
        })
    }
}

/// One answer: the newest node key for the recipient, or why not.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NodeKeyReply {
    /// The request's nonce, echoed.
    pub nonce: [u8; 16],
    /// The key's bytes, or the failure code.
    pub outcome: Result<Vec<u8>, u64>,
}

impl NodeKeyReply {
    /// The reply's bytes.
    pub fn encode(&self) -> Vec<u8> {
        let mut out = Vec::new();
        emit_map_head(&mut out, 2);
        emit_uint(&mut out, 1);
        emit_bstr(&mut out, &self.nonce);
        match &self.outcome {
            Ok(key) => {
                emit_uint(&mut out, 2);
                out.extend_from_slice(key);
            }
            Err(code) => {
                emit_uint(&mut out, 3);
                emit_uint(&mut out, *code);
            }
        }
        out
    }

    /// A reply from its bytes.
    pub fn decode(b: &[u8]) -> Result<Self, String> {
        let item = parse_all(b).map_err(|e| e.0)?;
        schema::check_unsigned(Family::NodeKeyReply, b, 0).map_err(|e| e.0)?;
        let Item::Map(m) = &item else {
            return Err("not a map".into());
        };
        let nonce = match map_get(m, 1) {
            Some(Item::Bytes(r)) => b[r.clone()].try_into().map_err(|_| "nonce")?,
            _ => return Err("nonce".into()),
        };
        let outcome = match value_slice(b, 2) {
            Some(r) => Ok(b[r].to_vec()),
            None => Err(map_get(m, 3).and_then(as_uint).ok_or("field 3")?),
        };
        Ok(NodeKeyReply { nonce, outcome })
    }
}

/// The replies to a batch, one per recipient asked, in the request's own
/// order (`wire-format.md` §7.8).
pub fn encode_batch_reply(replies: &[NodeKeyReply]) -> Vec<u8> {
    let mut out = Vec::new();
    emit_array_head(&mut out, replies.len());
    for r in replies {
        out.extend_from_slice(&r.encode());
    }
    out
}

/// A batch reply from its bytes.
pub fn decode_batch_reply(b: &[u8]) -> Result<Vec<NodeKeyReply>, String> {
    let item = parse_all(b).map_err(|e| e.0)?;
    schema::check_kind(b, "NodeKeyBatchReply", &item).map_err(|e| e.0)?;
    array_item_ranges(b, 0)
        .ok_or("not an array")?
        .into_iter()
        .map(|r| NodeKeyReply::decode(&b[r]))
        .collect()
}
