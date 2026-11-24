//! The split seal's two objects (`wire-format.md` §7.8.1): `Sealed`, what
//! travels to the recipient's node and back out, and `SealedBody`, what it
//! opens to.  The arithmetic is `rhtn-crypto`'s seal module; this is the
//! encoding either side of it.

use crate::Keyhash;
use rhtn_codec::cbor::*;
use rhtn_codec::encode::*;
use rhtn_codec::schema;
use rhtn_crypto::xwing::CIPHERTEXT_BYTES;

/// A sealed message: the node key it is sealed to, the two X-Wing
/// ciphertexts, and the nonce-prefixed AES-GCM ciphertext of the body.
#[derive(Clone, PartialEq, Eq)]
pub struct Sealed {
    /// The node key's number.
    pub n: u64,
    /// The encapsulation to the recipient's encryption key.
    pub ct_r: Box<[u8; CIPHERTEXT_BYTES]>,
    /// The encapsulation to the node's key.
    pub ct_n: Box<[u8; CIPHERTEXT_BYTES]>,
    /// Nonce ‖ ciphertext of the `SealedBody`.
    pub body: Vec<u8>,
}

impl std::fmt::Debug for Sealed {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Sealed(n {}, {} bytes)", self.n, self.body.len())
    }
}

impl Sealed {
    /// Its bytes.
    pub fn encode(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(2 * CIPHERTEXT_BYTES + self.body.len() + 16);
        emit_array_head(&mut out, 4);
        emit_uint(&mut out, self.n);
        emit_bstr(&mut out, &self.ct_r[..]);
        emit_bstr(&mut out, &self.ct_n[..]);
        emit_bstr(&mut out, &self.body);
        out
    }

    /// One from its bytes, the shape checked.
    pub fn decode(b: &[u8]) -> Result<Sealed, String> {
        let item = parse_all(b).map_err(|e| e.0)?;
        schema::check_kind(b, "Sealed", &item).map_err(|e| e.0)?;
        let Item::Array(a) = &item else {
            return Err("not an array".into());
        };
        let bytes = |i: usize| match &a[i] {
            Item::Bytes(r) => Ok(&b[r.clone()]),
            _ => Err(format!("element {i}")),
        };
        Ok(Sealed {
            n: as_uint(&a[0]).ok_or("n")?,
            ct_r: Box::new(bytes(1)?.try_into().map_err(|_| "ctR")?),
            ct_n: Box::new(bytes(2)?.try_into().map_err(|_| "ctN")?),
            body: bytes(3)?.to_vec(),
        })
    }

    /// The hash a recipient remembers an opened message by
    /// (`wire-format.md` §7.8.1): SHA-256 of the encoded seal.
    pub fn hash(&self) -> [u8; 32] {
        rhtn_codec::cose::sha256(&self.encode())
    }
}

/// What a seal opens to.
#[derive(Clone, PartialEq, Eq)]
pub struct SealedBody {
    /// The sender, as it names itself.
    pub sender: Keyhash,
    /// Its alias: display only, unverified until the tag is checked.
    pub alias: String,
    /// The sender's own `Introduction`, on its messages to this recipient
    /// until it has had a reply.
    pub introduction: Option<Vec<u8>>,
    /// The kind (§7.10.1).
    pub kind: u64,
    /// The bytes.
    pub bytes: Vec<u8>,
    /// The tag over the ciphertexts and fields 1 to 5.
    pub tag: [u8; 32],
}

impl std::fmt::Debug for SealedBody {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "SealedBody(kind {}, {} bytes, introduction {})",
            self.kind,
            self.bytes.len(),
            self.introduction.is_some()
        )
    }
}

impl SealedBody {
    /// **Fields 1 to 5 as encoded** (`wire-format.md` §7.8.1): each
    /// element's canonical encoding in order, as they stand in the body
    /// between its array head and the tag. What the tag is taken over,
    /// after the two ciphertexts.
    pub fn tagged(
        sender: &Keyhash,
        alias: &str,
        introduction: Option<&[u8]>,
        kind: u64,
        bytes: &[u8],
    ) -> Vec<u8> {
        let mut out = Vec::new();
        emit_bstr(&mut out, sender);
        emit_tstr(&mut out, alias);
        match introduction {
            Some(i) => out.extend_from_slice(i),
            None => emit_null(&mut out),
        }
        emit_uint(&mut out, kind);
        emit_bstr(&mut out, bytes);
        out
    }

    /// This body's fields 1 to 5, as [`SealedBody::tagged`] gives them.
    pub fn fields(&self) -> Vec<u8> {
        Self::tagged(
            &self.sender,
            &self.alias,
            self.introduction.as_deref(),
            self.kind,
            &self.bytes,
        )
    }

    /// Its bytes.
    pub fn encode(&self) -> Vec<u8> {
        let mut out = Vec::new();
        emit_array_head(&mut out, 6);
        out.extend_from_slice(&self.fields());
        emit_bstr(&mut out, &self.tag);
        out
    }

    /// One from its bytes, the shape checked; the tag is not verified.
    pub fn decode(b: &[u8]) -> Result<SealedBody, String> {
        let item = parse_all(b).map_err(|e| e.0)?;
        schema::check_kind(b, "SealedBody", &item).map_err(|e| e.0)?;
        let Item::Array(a) = &item else {
            return Err("not an array".into());
        };
        let parts = array_item_ranges(b, 0).ok_or("array walk")?;
        let bytes = |i: usize| match &a[i] {
            Item::Bytes(r) => Ok(&b[r.clone()]),
            _ => Err(format!("element {i}")),
        };
        let alias = match &a[1] {
            Item::Text(r) => std::str::from_utf8(&b[r.clone()])
                .map_err(|_| "alias")?
                .to_string(),
            _ => return Err("alias".into()),
        };
        Ok(SealedBody {
            sender: bytes(0)?.try_into().map_err(|_| "sender")?,
            alias,
            introduction: match &a[2] {
                Item::Null => None,
                _ => Some(b[parts[2].clone()].to_vec()),
            },
            kind: as_uint(&a[3]).ok_or("kind")?,
            bytes: bytes(4)?.to_vec(),
            tag: bytes(5)?.try_into().map_err(|_| "tag")?,
        })
    }
}
