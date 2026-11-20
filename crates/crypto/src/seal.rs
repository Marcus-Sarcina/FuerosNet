//! The split seal (design §14.2.4, `wire-format.md` §7.8.1): a message is
//! sealed to two X-Wing keys at once, the recipient identity's and the one
//! its serving node holds for that recipient, so that it opens only where
//! both secrets meet.  What is here is the construction to the byte —
//! the encryption key's derivation, the message key, the AEAD and the
//! sender's tag.  Which bytes the tag covers, and how a `SealedBody` is
//! encoded, are the client's (`rhtn-client`); this crate takes them as
//! bytes, as [`crate::pqxdh`] takes a bundle.
//!
//! Randomness is the caller's: an encapsulation takes its 64 bytes
//! ([`crate::xwing::encapsulate`]) and a seal takes its nonce, so every
//! function here is a function and every vector reproduces.

use aws_lc_rs::aead::{AES_256_GCM, Aad, LessSafeKey, Nonce, UnboundKey};
use aws_lc_rs::constant_time::verify_slices_are_equal;
use aws_lc_rs::hmac;
use hkdf::Hkdf;
use sha2::Sha256;
use x25519_dalek::{PublicKey, StaticSecret};
use zeroize::Zeroizing;

use crate::xwing;

/// HKDF info for the identity's X-Wing seed (`wire-format.md` §2.5).
pub const ENCRYPTION_KEY_INFO: &[u8] = b"rhtn/1:encryption-key";
/// HKDF info for the identity's sealing-authentication key, and the
/// prefix of the tag key's info (`wire-format.md` §2.5, §7.8.1).
pub const SEAL_AUTH_INFO: &[u8] = b"rhtn/1:seal-auth";
/// The prefix of the message key's HKDF info (`wire-format.md` §7.8.1).
pub const SEAL_INFO: &[u8] = b"rhtn/1:seal";
/// The AES-256-GCM nonce, in bytes, carried in front of the ciphertext.
pub const NONCE_BYTES: usize = 12;
/// The tag, in bytes.
pub const TAG_BYTES: usize = 32;

/// **The identity's encryption secrets**: the X-Wing key a message is
/// sealed to and the X25519 key that authenticates what it seals.  Both
/// derived from the identity root (design §7.5.2.6), so every device the
/// phrase restores holds them and nothing carries them between devices
/// except a pairing (`wire-format.md` §14.3.3).
pub struct EncryptionSecret {
    kem: xwing::Secret,
    auth: StaticSecret,
}

/// The X-Wing secret wipes itself ([`xwing::Secret`]) and the X25519
/// scalar is wiped by `x25519-dalek`; stated for the audit (APP-012).
impl zeroize::ZeroizeOnDrop for EncryptionSecret {}

impl std::fmt::Debug for EncryptionSecret {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "EncryptionSecret({:?})", self.kem)
    }
}

fn expand(root: &[u8; 32], info: &[u8]) -> Zeroizing<[u8; 32]> {
    let mut out = Zeroizing::new([0u8; 32]);
    Hkdf::<Sha256>::new(None, root)
        .expand(info, &mut out[..])
        .expect("32 bytes is within HKDF's bound");
    out
}

impl EncryptionSecret {
    /// The identity's own, from its root: HKDF-SHA-256 with an empty salt,
    /// expanded under [`ENCRYPTION_KEY_INFO`] to the X-Wing seed and under
    /// [`SEAL_AUTH_INFO`] to the authentication key.
    pub fn from_identity_root(root: &[u8; 32]) -> Self {
        let kem_seed = expand(root, ENCRYPTION_KEY_INFO);
        let auth = expand(root, SEAL_AUTH_INFO);
        EncryptionSecret::from_secrets(&kem_seed, &auth)
    }

    /// From the two 32-byte secrets as a paired device receives them
    /// (`wire-format.md` §14.3.3, `DeviceCredential`).
    pub fn from_secrets(kem_seed: &[u8; 32], auth: &[u8; 32]) -> Self {
        EncryptionSecret {
            kem: xwing::Secret::from_seed(*kem_seed),
            auth: StaticSecret::from(*auth),
        }
    }

    /// The X-Wing seed, as it crosses to a paired device.
    pub fn kem_seed(&self) -> &[u8; 32] {
        self.kem.seed()
    }

    /// The authentication key's scalar, as it crosses to a paired device.
    pub fn auth_secret(&self) -> Zeroizing<[u8; 32]> {
        Zeroizing::new(self.auth.to_bytes())
    }

    /// The X-Wing encapsulation key: `EncryptionKey` field 2.
    pub fn kem_public(&self) -> &[u8; xwing::PUBLIC_BYTES] {
        self.kem.public()
    }

    /// The authentication key's public half: `EncryptionKey` field 3.
    pub fn auth_public(&self) -> [u8; 32] {
        *PublicKey::from(&self.auth).as_bytes()
    }

    /// The recipient's half: the secret `ctR` carries to this identity.
    pub fn decapsulate(&self, ct_r: &[u8; xwing::CIPHERTEXT_BYTES]) -> Zeroizing<[u8; 32]> {
        self.kem.decapsulate(ct_r)
    }

    /// **The tag key** between this identity and another, for a message
    /// from `sender` to `recipient`: HKDF-SHA-256 of the X25519 exchange
    /// between the two authentication keys, info [`SEAL_AUTH_INFO`] ‖
    /// sender ‖ recipient.  Either party computes the same key, which is
    /// why the tag proves nothing to a third (design §14.2.4.2).
    pub fn tag_key(
        &self,
        their_auth: &[u8; 32],
        sender: &[u8; 32],
        recipient: &[u8; 32],
    ) -> Zeroizing<[u8; 32]> {
        let dh = Zeroizing::new(
            *self
                .auth
                .diffie_hellman(&PublicKey::from(*their_auth))
                .as_bytes(),
        );
        let mut info = Vec::with_capacity(SEAL_AUTH_INFO.len() + 64);
        info.extend_from_slice(SEAL_AUTH_INFO);
        info.extend_from_slice(sender);
        info.extend_from_slice(recipient);
        let mut out = Zeroizing::new([0u8; 32]);
        Hkdf::<Sha256>::new(None, &dh[..])
            .expand(&info, &mut out[..])
            .expect("32 bytes is within HKDF's bound");
        out
    }
}

/// **The message key**: HKDF-SHA-256, empty salt, input `ssR ‖ ssN`, info
/// [`SEAL_INFO`] ‖ recipient keyhash ‖ node keyhash ‖ the node key's
/// number as a deterministic CBOR uint ‖ `ctR` ‖ `ctN`.  Both secrets
/// enter it, so breaking either half leaves nothing.
pub fn message_key(
    ss_r: &[u8; 32],
    ss_n: &[u8; 32],
    recipient: &[u8; 32],
    node: &[u8; 32],
    n: u64,
    ct_r: &[u8; xwing::CIPHERTEXT_BYTES],
    ct_n: &[u8; xwing::CIPHERTEXT_BYTES],
) -> Zeroizing<[u8; 32]> {
    let mut ikm = Zeroizing::new([0u8; 64]);
    ikm[..32].copy_from_slice(ss_r);
    ikm[32..].copy_from_slice(ss_n);
    let mut info = Vec::with_capacity(SEAL_INFO.len() + 64 + 9 + 2 * xwing::CIPHERTEXT_BYTES);
    info.extend_from_slice(SEAL_INFO);
    info.extend_from_slice(recipient);
    info.extend_from_slice(node);
    rhtn_codec::encode::emit_uint(&mut info, n);
    info.extend_from_slice(ct_r);
    info.extend_from_slice(ct_n);
    let mut out = Zeroizing::new([0u8; 32]);
    Hkdf::<Sha256>::new(None, &ikm[..])
        .expand(&info, &mut out[..])
        .expect("32 bytes is within HKDF's bound");
    out
}

/// **The tag**: HMAC-SHA-256 under the tag key over `ctR ‖ ctN ‖ fields`,
/// where `fields` is the canonical encoding of the body's fields before
/// the tag (`wire-format.md` §7.8.1).
pub fn tag(
    tag_key: &[u8; 32],
    ct_r: &[u8; xwing::CIPHERTEXT_BYTES],
    ct_n: &[u8; xwing::CIPHERTEXT_BYTES],
    fields: &[u8],
) -> [u8; TAG_BYTES] {
    let key = hmac::Key::new(hmac::HMAC_SHA256, &tag_key[..]);
    let mut ctx = hmac::Context::with_key(&key);
    ctx.update(ct_r);
    ctx.update(ct_n);
    ctx.update(fields);
    let mut out = [0u8; TAG_BYTES];
    out.copy_from_slice(ctx.sign().as_ref());
    out
}

/// Whether `claimed` is the tag over these inputs, compared in constant
/// time.
pub fn tag_verifies(
    tag_key: &[u8; 32],
    ct_r: &[u8; xwing::CIPHERTEXT_BYTES],
    ct_n: &[u8; xwing::CIPHERTEXT_BYTES],
    fields: &[u8],
    claimed: &[u8],
) -> bool {
    verify_slices_are_equal(&tag(tag_key, ct_r, ct_n, fields), claimed).is_ok()
}

/// **Seal a body**: AES-256-GCM under the message key, `nonce` in front
/// of the ciphertext, no associated data — the key already binds both
/// encapsulations, both keys and the recipient (`wire-format.md` §7.8.1).
pub fn seal(key: &[u8; 32], nonce: [u8; NONCE_BYTES], body: &[u8]) -> Vec<u8> {
    let k = LessSafeKey::new(UnboundKey::new(&AES_256_GCM, &key[..]).expect("a 32-byte key"));
    let mut out = Vec::with_capacity(NONCE_BYTES + body.len() + 16);
    out.extend_from_slice(&nonce);
    let mut ct = body.to_vec();
    k.seal_in_place_append_tag(Nonce::assume_unique_for_key(nonce), Aad::empty(), &mut ct)
        .expect("a body within AES-GCM's bound");
    out.extend_from_slice(&ct);
    out
}

/// **Open a sealed body**, or `None` where it does not open: a wrong key,
/// a changed byte, or too short to carry a nonce and a GCM tag.
pub fn open(key: &[u8; 32], sealed: &[u8]) -> Option<Zeroizing<Vec<u8>>> {
    if sealed.len() < NONCE_BYTES + 16 {
        return None;
    }
    let k = LessSafeKey::new(UnboundKey::new(&AES_256_GCM, &key[..]).ok()?);
    let mut nonce = [0u8; NONCE_BYTES];
    nonce.copy_from_slice(&sealed[..NONCE_BYTES]);
    let mut buf = Zeroizing::new(sealed[NONCE_BYTES..].to_vec());
    let len = k
        .open_in_place(
            Nonce::assume_unique_for_key(nonce),
            Aad::empty(),
            &mut buf[..],
        )
        .ok()?
        .len();
    buf.truncate(len);
    Some(buf)
}
