//! A serving node's key for one recipient (`wire-format.md` §7.8): an
//! X-Wing key the node generates per identity it serves, signs classically
//! and publishes, and whose private half it holds while any message sealed
//! to it is queued (design §14.2.4.3).  How a receiver checks the record is
//! `verify::record` under the `NodeKey` kind; this is the node's side.

use crate::signer::Sign1;
use crate::xwing;
use rhtn_codec::cose::aad;
use rhtn_codec::encode::*;

/// The fields of a `NodeKey` as a node states them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NodeKeyFields {
    /// The recipient it is for, field 1.
    pub recipient: [u8; 32],
    /// The node holding its private half, field 2.
    pub node: [u8; 32],
    /// Its number, field 3: the node's counter for this recipient.
    pub n: u64,
    /// The X-Wing encapsulation key, field 4.
    pub public: [u8; xwing::PUBLIC_BYTES],
    /// When it was published, field 5, in seconds.
    pub published_at: u64,
}

impl NodeKeyFields {
    /// Fields 1 to 5 as the map the signature covers.
    fn payload(&self) -> Vec<u8> {
        let mut out = Vec::new();
        emit_map_head(&mut out, 5);
        emit_uint(&mut out, 1);
        emit_bstr(&mut out, &self.recipient);
        emit_uint(&mut out, 2);
        emit_bstr(&mut out, &self.node);
        emit_uint(&mut out, 3);
        emit_uint(&mut out, self.n);
        emit_uint(&mut out, 4);
        emit_bstr(&mut out, &self.public);
        emit_uint(&mut out, 5);
        emit_uint(&mut out, self.published_at);
        out
    }

    /// Read the fields of a `NodeKey` whose shape the codec has checked;
    /// none where it does not read. The signature is not verified here.
    pub fn read(raw: &[u8]) -> Option<NodeKeyFields> {
        use rhtn_codec::cbor::*;
        let item = parse_all(raw).ok()?;
        rhtn_codec::schema::check_kind(raw, "NodeKey", &item).ok()?;
        let Item::Map(m) = &item else { return None };
        let field = |k: u64| match map_get(m, k) {
            Some(Item::Bytes(r)) => Some(&raw[r.clone()]),
            _ => None,
        };
        Some(NodeKeyFields {
            recipient: field(1)?.try_into().ok()?,
            node: field(2)?.try_into().ok()?,
            n: map_get(m, 3).and_then(as_uint)?,
            public: field(4)?.try_into().ok()?,
            published_at: map_get(m, 5).and_then(as_uint)?,
        })
    }
}

/// The `NodeKey` record for `fields`, signed by `node` (its field 2 is
/// the signer's keyhash).
pub fn issue<S: Sign1 + ?Sized>(node: &S, fields: &NodeKeyFields) -> Vec<u8> {
    debug_assert_eq!(node.keyhash(), fields.node, "signed by the node it names");
    let payload = fields.payload();
    let sig = node.sign1_ed_unnamed(aad::NODE_KEY, &payload);
    let mut out = Vec::new();
    emit_map_head(&mut out, 6);
    // the payload is a five-entry map; its head is one byte (0xa5)
    out.extend_from_slice(&payload[1..]);
    emit_uint(&mut out, 6);
    out.extend_from_slice(&sig);
    out
}
