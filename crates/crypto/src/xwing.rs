//! X-Wing (`draft-connolly-cfrg-xwing-kem`, revision -11): ML-KEM-768 and
//! X25519 as one KEM, their two shared secrets combined by SHA3-256 with
//! the X25519 ciphertext, its public key and a label.  Each half of the
//! split seal is one of these (design §14.2.4.2, `wire-format.md`
//! §7.8.1) [author, 2026-11-20].
//!
//! What is here is the draft to the byte, checked against its Appendix C
//! (`tests/xwing.rs`).  Randomness is the caller's, as it is in
//! [`crate::pqxdh`]: [`encapsulate`] takes the 64 bytes the draft's
//! derandomised form takes, so the same function is what the vectors
//! check and what a sender runs.

use ml_kem::{B32, Decapsulate, DecapsulationKey, EncapsulationKey, KeyExport, MlKem768, Seed};
use sha3::digest::{ExtendableOutput, Update, XofReader};
use sha3::{Digest, Sha3_256, Shake256};
use x25519_dalek::{PublicKey, StaticSecret};
use zeroize::{Zeroize, Zeroizing};

/// A decapsulation key, in bytes: the seed everything else expands from.
pub const SECRET_BYTES: usize = 32;
/// An encapsulation key, in bytes: ML-KEM-768's then X25519's.
pub const PUBLIC_BYTES: usize = 1216;
/// A ciphertext, in bytes: ML-KEM-768's then X25519's.
pub const CIPHERTEXT_BYTES: usize = 1120;
/// The randomness one encapsulation takes: ML-KEM's 32 bytes, then the
/// X25519 ephemeral's.
pub const ESEED_BYTES: usize = 64;

/// The draft's `XWingLabel`, `\./` then `/^\`.
pub const LABEL: [u8; 6] = [0x5c, 0x2e, 0x2f, 0x2f, 0x5e, 0x5c];

const ML_KEM_PUBLIC: usize = 1184;
const ML_KEM_CIPHERTEXT: usize = 1088;

/// An X-Wing decapsulation key, expanded once from its seed and kept
/// expanded (the draft's §5.5.1 permits it), with its encapsulation key.
pub struct Secret {
    seed: Zeroizing<[u8; SECRET_BYTES]>,
    m: DecapsulationKey<MlKem768>,
    x: StaticSecret,
    public: [u8; PUBLIC_BYTES],
}

/// The seed is wiped by its `Zeroizing`, the ML-KEM key by `ml-kem`'s
/// `zeroize` feature and the X25519 scalar by `x25519-dalek`'s; stated
/// for the audit (`rhtn-client`'s `tests/zeroize.rs`, APP-012).
impl zeroize::ZeroizeOnDrop for Secret {}

impl std::fmt::Debug for Secret {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "xwing::Secret({:02x?}…)", &self.public[ML_KEM_PUBLIC..])
    }
}

impl Secret {
    /// The draft's `expandDecapsulationKey`: SHAKE256 of the seed to 96
    /// bytes, the first 64 ML-KEM-768's seed form (FIPS 203's `d ‖ z`)
    /// and the last 32 the X25519 private key.
    pub fn from_seed(seed: [u8; SECRET_BYTES]) -> Self {
        let mut expanded = Zeroizing::new([0u8; 96]);
        let mut xof = Shake256::default();
        xof.update(&seed);
        xof.finalize_xof().read(&mut expanded[..]);
        let mut m_seed = [0u8; 64];
        m_seed.copy_from_slice(&expanded[..64]);
        let m = DecapsulationKey::<MlKem768>::from_seed(Seed::from(m_seed));
        m_seed.zeroize();
        let mut x_bytes = [0u8; 32];
        x_bytes.copy_from_slice(&expanded[64..]);
        let x = StaticSecret::from(x_bytes);
        x_bytes.zeroize();
        let mut public = [0u8; PUBLIC_BYTES];
        public[..ML_KEM_PUBLIC].copy_from_slice(&m.encapsulation_key().to_bytes());
        public[ML_KEM_PUBLIC..].copy_from_slice(PublicKey::from(&x).as_bytes());
        Secret {
            seed: Zeroizing::new(seed),
            m,
            x,
            public,
        }
    }

    /// The seed, as the key goes to a device's own storage or across a
    /// pairing (`wire-format.md` §14.3.3).
    pub fn seed(&self) -> &[u8; SECRET_BYTES] {
        &self.seed
    }

    /// The encapsulation key, as a card carries it (`wire-format.md` §2.5).
    pub fn public(&self) -> &[u8; PUBLIC_BYTES] {
        &self.public
    }

    /// The draft's `Decapsulate`: the 32-byte shared secret a ciphertext
    /// to this key carries.  ML-KEM rejects implicitly, so a ciphertext
    /// that is the right length always yields a secret, and a wrong one
    /// shows only as a message that does not open.
    pub fn decapsulate(&self, ct: &[u8; CIPHERTEXT_BYTES]) -> Zeroizing<[u8; 32]> {
        let ss_m = self
            .m
            .decapsulate_slice(&ct[..ML_KEM_CIPHERTEXT])
            .expect("the ML-KEM ciphertext is its fixed length");
        let mut ct_x = [0u8; 32];
        ct_x.copy_from_slice(&ct[ML_KEM_CIPHERTEXT..]);
        let ss_x = Zeroizing::new(*self.x.diffie_hellman(&PublicKey::from(ct_x)).as_bytes());
        let mut pk_x = [0u8; 32];
        pk_x.copy_from_slice(&self.public[ML_KEM_PUBLIC..]);
        let mut ss_m = ss_m;
        let ss = combine(&ss_m[..], &ss_x[..], &ct_x, &pk_x);
        ss_m.zeroize();
        ss
    }

    /// Whether the seed this holds is all zero: what the audit reads after
    /// dropping one in place.  Not a key accessor.
    #[doc(hidden)]
    pub fn seed_wiped(&self) -> bool {
        self.seed.iter().all(|b| *b == 0)
    }
}

/// The draft's `Combiner`: SHA3-256 over the ML-KEM secret, the X25519
/// secret, the X25519 ciphertext, the X25519 public key and the label.
fn combine(ss_m: &[u8], ss_x: &[u8], ct_x: &[u8; 32], pk_x: &[u8; 32]) -> Zeroizing<[u8; 32]> {
    let mut h = Sha3_256::new();
    Digest::update(&mut h, ss_m);
    Digest::update(&mut h, ss_x);
    Digest::update(&mut h, ct_x);
    Digest::update(&mut h, pk_x);
    Digest::update(&mut h, LABEL);
    Zeroizing::new(h.finalize().into())
}

/// The draft's `EncapsulateDerand`: the shared secret and the ciphertext
/// that carries it to `public`, from 64 bytes of the caller's randomness
/// — ML-KEM's message first, the X25519 ephemeral second.  A public key
/// whose ML-KEM half fails FIPS 203's input check is refused, as the
/// draft's §5.4 allows.
pub fn encapsulate(
    public: &[u8; PUBLIC_BYTES],
    eseed: &[u8; ESEED_BYTES],
) -> Result<(Zeroizing<[u8; 32]>, [u8; CIPHERTEXT_BYTES]), &'static str> {
    let key = public[..ML_KEM_PUBLIC]
        .try_into()
        .map_err(|_| "encapsulation key length")?;
    let m = EncapsulationKey::<MlKem768>::new(&key).map_err(|_| "encapsulation key invalid")?;
    let mut msg = [0u8; 32];
    msg.copy_from_slice(&eseed[..32]);
    let (ct_m, mut ss_m) = m.encapsulate_deterministic(&B32::from(msg));
    msg.zeroize();
    let mut ek = [0u8; 32];
    ek.copy_from_slice(&eseed[32..]);
    let ek_x = StaticSecret::from(ek);
    ek.zeroize();
    let ct_x = *PublicKey::from(&ek_x).as_bytes();
    let mut pk_x = [0u8; 32];
    pk_x.copy_from_slice(&public[ML_KEM_PUBLIC..]);
    let ss_x = Zeroizing::new(*ek_x.diffie_hellman(&PublicKey::from(pk_x)).as_bytes());
    let mut ct = [0u8; CIPHERTEXT_BYTES];
    ct[..ML_KEM_CIPHERTEXT].copy_from_slice(&ct_m);
    ct[ML_KEM_CIPHERTEXT..].copy_from_slice(&ct_x);
    let ss = combine(&ss_m[..], &ss_x[..], &ct_x, &pk_x);
    ss_m.zeroize();
    Ok((ss, ct))
}
