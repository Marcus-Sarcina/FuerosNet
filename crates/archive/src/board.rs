//! The board's wire objects (`wire-format.md` §10.3; design §14.4): the
//! post as it is carried, a member's asking for what it missed, and the
//! answer. Encodings only; who a copy is for is `rhtn-node`'s and what a
//! post says is a client's.

use rhtn_codec::cbor::*;
use rhtn_codec::encode::*;
use rhtn_codec::schema::{self, Family};

/// **Control frame 10: a post on the subnet's board** [author,
/// 2026-11-23], from a member to the node serving it, from a node to the
/// next node up or down, and from a node to a member. It has no reply.
pub const FRAME_BOARD_POST: u64 = 10;

/// Request type 17: a member asking its node for the posts it missed.
pub const REQUEST_BOARD: u64 = 17;

/// A `BoardPost`. The two counts are measured from a different place in
/// each direction, which the session a frame arrives on says.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BoardPost {
    /// Levels above still to reach, 0 to 24.
    pub up: u64,
    /// Levels below still to reach, 0 to 24.
    pub down: u64,
    /// The post, which no node reads: 1 to 16,384 bytes.
    pub post: Vec<u8>,
    /// The number the sending node stored it under: from a node to a
    /// member, and nowhere else.
    pub number: Option<u64>,
}

impl BoardPost {
    /// The frame's body.
    #[must_use]
    pub fn encode(&self) -> Vec<u8> {
        let mut out = Vec::new();
        emit_map_head(&mut out, 3 + usize::from(self.number.is_some()));
        emit_uint(&mut out, 1);
        emit_uint(&mut out, self.up);
        emit_uint(&mut out, 2);
        emit_uint(&mut out, self.down);
        emit_uint(&mut out, 3);
        emit_bstr(&mut out, &self.post);
        if let Some(n) = self.number {
            emit_uint(&mut out, 4);
            emit_uint(&mut out, n);
        }
        out
    }

    /// A post from a frame's body.
    pub fn decode(b: &[u8]) -> Result<Self, String> {
        schema::check_unsigned(Family::BoardPost, b, 0).map_err(|e| e.0.to_string())?;
        let item = parse_all(b).map_err(|e| e.0)?;
        let Item::Map(m) = &item else {
            return Err("not a map".into());
        };
        let Some(Item::Bytes(post)) = map_get(m, 3) else {
            return Err("field 3".into());
        };
        Ok(BoardPost {
            up: map_get(m, 1).and_then(as_uint).ok_or("field 1")?,
            down: map_get(m, 2).and_then(as_uint).ok_or("field 2")?,
            post: b[post.clone()].to_vec(),
            number: map_get(m, 4).and_then(as_uint),
        })
    }
}

/// A `BoardRequest`: everything after the number the asker holds.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BoardRequest {
    /// The nonce the reply must echo.
    pub nonce: [u8; 16],
    /// The last number the asker holds from this node; `None` for
    /// everything held.
    pub after: Option<u64>,
}

impl BoardRequest {
    /// The request's bytes.
    #[must_use]
    pub fn encode(&self) -> Vec<u8> {
        let mut out = Vec::new();
        emit_map_head(&mut out, 1 + usize::from(self.after.is_some()));
        emit_uint(&mut out, 1);
        emit_bstr(&mut out, &self.nonce);
        if let Some(n) = self.after {
            emit_uint(&mut out, 2);
            emit_uint(&mut out, n);
        }
        out
    }

    /// A request from its bytes.
    pub fn decode(b: &[u8]) -> Result<Self, String> {
        schema::check_unsigned(Family::BoardRequest, b, 0).map_err(|e| e.0.to_string())?;
        let item = parse_all(b).map_err(|e| e.0)?;
        let Item::Map(m) = &item else {
            return Err("not a map".into());
        };
        Ok(BoardRequest {
            nonce: nonce_at(b, m)?,
            after: map_get(m, 2).and_then(as_uint),
        })
    }
}

/// A `BoardReply`: the posts in the asker's band, oldest first.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BoardReply {
    /// The request's nonce, echoed.
    pub nonce: [u8; 16],
    /// Number and post, ascending by number.
    pub posts: Vec<(u64, Vec<u8>)>,
    /// The number this reply read through: what to ask from next.
    pub through: u64,
    /// Whether posts numbered above that are held.
    pub more: bool,
}

impl BoardReply {
    /// The reply's bytes.
    #[must_use]
    pub fn encode(&self) -> Vec<u8> {
        let mut out = Vec::new();
        emit_map_head(&mut out, 4);
        emit_uint(&mut out, 1);
        emit_bstr(&mut out, &self.nonce);
        emit_uint(&mut out, 2);
        emit_array_head(&mut out, self.posts.len());
        for (number, post) in &self.posts {
            emit_array_head(&mut out, 2);
            emit_uint(&mut out, *number);
            emit_bstr(&mut out, post);
        }
        emit_uint(&mut out, 3);
        emit_uint(&mut out, self.through);
        emit_uint(&mut out, 4);
        emit_bool(&mut out, self.more);
        out
    }

    /// A reply from its bytes.
    pub fn decode(b: &[u8]) -> Result<Self, String> {
        schema::check_unsigned(Family::BoardReply, b, 0).map_err(|e| e.0.to_string())?;
        let item = parse_all(b).map_err(|e| e.0)?;
        let Item::Map(m) = &item else {
            return Err("not a map".into());
        };
        let Some(Item::Array(pairs)) = map_get(m, 2) else {
            return Err("field 2".into());
        };
        let mut posts = Vec::with_capacity(pairs.len());
        for pair in pairs {
            let Item::Array(p) = pair else {
                return Err("a post is a pair".into());
            };
            let [Item::Uint(number), Item::Bytes(post)] = p.as_slice() else {
                return Err("a post is a number and a post".into());
            };
            posts.push((*number, b[post.clone()].to_vec()));
        }
        let Some(Item::Bool(more)) = map_get(m, 4) else {
            return Err("field 4".into());
        };
        Ok(BoardReply {
            nonce: nonce_at(b, m)?,
            posts,
            through: map_get(m, 3).and_then(as_uint).ok_or("field 3")?,
            more: *more,
        })
    }
}

fn nonce_at(b: &[u8], m: &[(Item, Item)]) -> Result<[u8; 16], String> {
    match map_get(m, 1) {
        Some(Item::Bytes(r)) if r.len() == 16 => Ok(<[u8; 16]>::try_from(&b[r.clone()]).unwrap()),
        _ => Err("field 1 nonce".into()),
    }
}
