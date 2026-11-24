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

/// **A post as its writer signs it** (`wire-format.md` §10.3.1): the
/// reference client's, which is what a `BoardPost` frame's field 3 holds
/// and what no node reads. Everything a reader needs is inside it, the
/// writer's keys among them, so a reader that has never held the writer
/// can still check that the post is the key's it names.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Post {
    /// Field 1: the writer, and the signer.
    pub writer: crate::Keyhash,
    /// Field 2: the writer's key material, which hashes to field 1.
    pub material: Vec<u8>,
    /// Field 3: where the writer stood when writing, which is the
    /// writer's own statement.
    pub position: crate::tx::Locator,
    /// Field 4: the reach as written, upward.
    pub up: u64,
    /// Field 5: the reach as written, downward.
    pub down: u64,
    /// Field 6: when it was written, by the writer's clock.
    pub written_at: u64,
    /// Field 7: the thread it continues, the hash of the post that began
    /// it; `None` on a post that begins one.
    pub thread: Option<[u8; 32]>,
    /// Field 8: the title, on a post that begins a thread and no other.
    pub title: Option<String>,
    /// Field 9: the words.
    pub text: String,
    /// Field 10: the writer's alias in the subnet, where they gave one.
    /// **Display only, never matched.**
    pub alias: Option<String>,
    /// Field 12: whether the post was signed under a key the writer
    /// delegated, the delegation stapled beside it: what a device holding
    /// no seed writes (design §23.3).
    pub delegated: bool,
    /// The post's own bytes, which the signature covers and the hash names.
    pub bytes: Vec<u8>,
}

/// Who signs a post: the identity itself, or a device it delegated to.
pub struct Writer<'a> {
    /// What signs: the identity's own classical key, or the delegated one.
    pub signer: &'a dyn rhtn_crypto::signer::Sign1,
    /// The identity's key material, which the post carries.
    pub material: &'a [u8],
    /// The delegation of the signing key, where it is a delegated one.
    pub delegation: Option<&'a [u8]>,
}

impl<'a> Writer<'a> {
    /// The identity writing for itself, on a device that holds its seed.
    #[must_use]
    pub fn own(identity: &'a rhtn_crypto::SigningIdentity, material: &'a [u8]) -> Self {
        Writer {
            signer: identity,
            material,
            delegation: None,
        }
    }
}

/// What a writer puts in a post: everything but who they are, which the
/// signing identity says.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Draft<'a> {
    /// Where the writer stands in the subnet.
    pub position: &'a crate::tx::Locator,
    /// The reach, upward.
    pub up: u64,
    /// The reach, downward.
    pub down: u64,
    /// When it is written.
    pub written_at: u64,
    /// The thread it continues, or `None` to begin one.
    pub thread: Option<[u8; 32]>,
    /// The title of the thread it begins.
    pub title: Option<&'a str>,
    /// The words.
    pub text: &'a str,
    /// The writer's alias in the subnet.
    pub alias: Option<&'a str>,
}

impl Post {
    /// Build one, signed classically under the board-post tag over
    /// fields 1 to 10 (§10.3.1), the delegation of the signing key stapled
    /// after the signature where a delegated device writes. Nothing where
    /// it would not parse back: a thread and a title together or neither,
    /// words or a title out of bounds, or the whole over a post's bound.
    #[must_use]
    pub fn build(writer: &Writer<'_>, d: &Draft<'_>) -> Option<Vec<u8>> {
        let mut payload = Vec::new();
        let fields = 8 + usize::from(d.alias.is_some());
        emit_map_head(&mut payload, fields);
        emit_uint(&mut payload, 1);
        emit_bstr(&mut payload, &writer.signer.keyhash());
        emit_uint(&mut payload, 2);
        payload.extend_from_slice(writer.material);
        emit_uint(&mut payload, 3);
        d.position.emit(&mut payload);
        emit_uint(&mut payload, 4);
        emit_uint(&mut payload, d.up);
        emit_uint(&mut payload, 5);
        emit_uint(&mut payload, d.down);
        emit_uint(&mut payload, 6);
        emit_uint(&mut payload, d.written_at);
        match (d.thread, d.title) {
            (Some(thread), None) => {
                emit_uint(&mut payload, 7);
                emit_bstr(&mut payload, &thread);
            }
            (None, Some(title)) => {
                emit_uint(&mut payload, 8);
                emit_tstr(&mut payload, title);
            }
            _ => return None,
        }
        emit_uint(&mut payload, 9);
        emit_tstr(&mut payload, d.text);
        if let Some(alias) = d.alias {
            emit_uint(&mut payload, 10);
            emit_tstr(&mut payload, alias);
        }
        let sig = writer
            .signer
            .sign1_ed_unnamed(rhtn_codec::cose::aad::BOARD_POST, &payload);
        let mut out = Vec::new();
        emit_map_head(
            &mut out,
            fields + 1 + usize::from(writer.delegation.is_some()),
        );
        out.extend_from_slice(&payload[1..]);
        emit_uint(&mut out, 11);
        out.extend_from_slice(&sig);
        if let Some(delegation) = writer.delegation {
            emit_uint(&mut out, 12);
            out.extend_from_slice(delegation);
        }
        (out.len() <= rhtn_codec::bounds::BOARD_POST_BYTES && Post::parse(&out).is_ok())
            .then_some(out)
    }

    /// Parse one, held to the `Post` kind. **Parsing is not checking**: a
    /// post is its writer's on [`Post::verify`]'s word.
    pub fn parse(b: &[u8]) -> Result<Self, String> {
        if b.len() > rhtn_codec::bounds::BOARD_POST_BYTES {
            return Err("over a post's bound".into());
        }
        let item = parse_all(b).map_err(|e| e.0)?;
        schema::check_kind(b, "Post", &item).map_err(|e| e.0)?;
        let Item::Map(m) = &item else {
            return Err("not a map".into());
        };
        let bytes32 = |k: u64| -> Option<[u8; 32]> {
            match map_get(m, k) {
                Some(Item::Bytes(r)) if r.len() == 32 => <[u8; 32]>::try_from(&b[r.clone()]).ok(),
                _ => None,
            }
        };
        let text = |k: u64| -> Result<Option<String>, String> {
            match map_get(m, k) {
                None => Ok(None),
                Some(Item::Text(r)) => std::str::from_utf8(&b[r.clone()])
                    .map(|s| Some(s.to_string()))
                    .map_err(|_| format!("field {k} not utf-8")),
                Some(_) => Err(format!("field {k}")),
            }
        };
        let material = value_slice(b, 2).ok_or("field 2")?;
        let position = value_slice(b, 3).ok_or("field 3")?;
        Ok(Post {
            writer: bytes32(1).ok_or("field 1")?,
            material: b[material].to_vec(),
            position: crate::tx::Locator::decode(&b[position])
                .map_err(|e| format!("locator: {e}"))?,
            up: map_get(m, 4).and_then(as_uint).ok_or("field 4")?,
            down: map_get(m, 5).and_then(as_uint).ok_or("field 5")?,
            written_at: map_get(m, 6).and_then(as_uint).ok_or("field 6")?,
            thread: bytes32(7),
            title: text(8)?,
            text: text(9)?.ok_or("field 9")?,
            alias: text(10)?,
            delegated: map_get(m, 12).is_some(),
            bytes: b.to_vec(),
        })
    }

    /// **Signed by the key it names, and carrying that key.** The material
    /// in field 2 must hash to field 1, and the signature must verify
    /// under it, or under a key that identity delegated for a window the
    /// post's own date falls in, the delegation stapled as field 12. What
    /// this does not say is that the writer stands where field 3 says,
    /// which is the writer's own statement.
    pub fn verify(&self) -> Result<(), String> {
        let id = rhtn_crypto::Identity::from_key_material(&self.material)
            .ok_or("the key material does not read")?;
        if id.keyhash != self.writer {
            return Err("the key material is not the writer's".into());
        }
        let ids = [id];
        if let Some(r) = value_slice(&self.bytes, 12) {
            let d = rhtn_crypto::verify::delegation(&ids[..], &self.bytes[r])
                .map_err(|e| e.to_string())?;
            if !(d.not_before..=d.not_after).contains(&self.written_at) {
                return Err("written outside the window its signing key was delegated for".into());
            }
        }
        rhtn_crypto::verify::record(&ids[..], "Post", &self.bytes).map_err(|e| e.to_string())
    }

    /// The hash a post is known by (`wire-format.md` §10.3): what a thread
    /// names its first post by, and what makes one post one however many
    /// times it arrives.
    #[must_use]
    pub fn hash(bytes: &[u8]) -> [u8; 32] {
        use sha2::{Digest, Sha256};
        Sha256::digest(bytes).into()
    }

    /// The thread this post is in: the one it continues, or itself where
    /// it begins one.
    #[must_use]
    pub fn thread_id(&self) -> [u8; 32] {
        self.thread.unwrap_or_else(|| Post::hash(&self.bytes))
    }
}
