//! PQXDH (design §14.2.4.2), the adopted asynchronous key agreement,
//! instantiated with X25519, SHA-256 and ML-KEM-768: the party offline has
//! published a signed prekey, a post-quantum signed prekey and, while they
//! last, one-time keys of each kind; the party sending combines its
//! identity and an ephemeral key with them and a KEM encapsulation into
//! one shared secret.  What is here is the construction to the byte;
//! which keys are which, and how the bundle travels, is the client's
//! (`rhtn-client`) and the network's (`wire-format.md` §7.8).
//!
//! Identity keys sign; the key agreement uses X25519 keys of its own,
//! bound to the identity by the bundle's signature (design §14.2.3).

use hkdf::Hkdf;
use ml_kem::{B32, Decapsulate, DecapsulationKey, EncapsulationKey, KeyExport, MlKem768, Seed};
use sha2::Sha256;
use x25519_dalek::{PublicKey, StaticSecret};

/// The KDF's info string, in the specification's shape:
/// application, curve, hash, KEM.
pub const INFO: &[u8] = b"rhtn/1_X25519_SHA-256_ML-KEM-768";
pub const KEM_PUBLIC_BYTES: usize = 1184;
pub const KEM_CIPHERTEXT_BYTES: usize = 1088;

/// An X25519 private key.
#[derive(Clone)]
pub struct DhSecret(StaticSecret);

/// An X25519 public key.
#[derive(Clone, Copy, PartialEq, Eq, Debug, PartialOrd, Ord)]
pub struct DhPublic(pub [u8; 32]);

impl DhSecret {
    pub fn from_seed(seed: [u8; 32]) -> Self {
        DhSecret(StaticSecret::from(seed))
    }

    pub fn public(&self) -> DhPublic {
        DhPublic(*PublicKey::from(&self.0).as_bytes())
    }

    pub fn agree(&self, their: &DhPublic) -> [u8; 32] {
        *self.0.diffie_hellman(&PublicKey::from(their.0)).as_bytes()
    }
}

impl std::fmt::Debug for DhSecret {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "DhSecret({:?})", self.public())
    }
}

/// An ML-KEM-768 decapsulation key.
#[derive(Clone)]
pub struct KemSecret(DecapsulationKey<MlKem768>);

/// An ML-KEM-768 encapsulation key, encoded.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct KemPublic(pub Vec<u8>);

impl KemSecret {
    pub fn from_seed(seed: [u8; 64]) -> Self {
        KemSecret(DecapsulationKey::<MlKem768>::from_seed(Seed::from(seed)))
    }

    pub fn public(&self) -> KemPublic {
        KemPublic(self.0.encapsulation_key().to_bytes().to_vec())
    }

    pub fn decapsulate(&self, ciphertext: &[u8]) -> Result<[u8; 32], String> {
        let ss = self.0.decapsulate_slice(ciphertext).map_err(|_| "ciphertext length")?;
        Ok(ss.into())
    }
}

impl std::fmt::Debug for KemSecret {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "KemSecret(..)")
    }
}

impl KemPublic {
    /// Encapsulate to this key with the given randomness: the ciphertext to
    /// send and the shared secret.
    pub fn encapsulate(&self, m: [u8; 32]) -> Result<(Vec<u8>, [u8; 32]), String> {
        let key = ml_kem::Key::<EncapsulationKey<MlKem768>>::try_from(self.0.as_slice()).map_err(|_| "encapsulation key length")?;
        let ek = EncapsulationKey::<MlKem768>::new(&key).map_err(|_| "encapsulation key invalid")?;
        let (ct, ss) = ek.encapsulate_deterministic(&B32::from(m));
        Ok((ct.to_vec(), ss.into()))
    }
}

/// What the initiator holds of the responder's published material: the
/// identity key, the signed prekey, the post-quantum signed prekey, and
/// the one-time keys where served.
pub struct TheirBundle<'a> {
    pub ik: &'a DhPublic,
    pub spk: &'a DhPublic,
    pub pqspk: &'a KemPublic,
    pub opk: Option<&'a DhPublic>,
    pub pqopk: Option<&'a KemPublic>,
}

/// What initiating yields: the shared secret, the ephemeral public key and
/// the KEM ciphertext the initial message carries.
pub struct Initiated {
    pub sk: [u8; 32],
    pub ek: DhPublic,
    pub kem_ciphertext: Vec<u8>,
}

/// The initiator's side: DH1 = DH(IK_A, SPK_B), DH2 = DH(EK_A, IK_B),
/// DH3 = DH(EK_A, SPK_B), DH4 = DH(EK_A, OPK_B) where served, SS from
/// PQOPK_B where served and PQSPK_B otherwise, and SK = KDF(DH1 ‖ DH2 ‖
/// DH3 ‖ [DH4] ‖ SS).  `kem_m` is the encapsulation's randomness.
pub fn initiate(ik_a: &DhSecret, ek_a: &DhSecret, their: &TheirBundle, kem_m: [u8; 32]) -> Result<Initiated, String> {
    let mut km = Vec::with_capacity(5 * 32);
    km.extend_from_slice(&ik_a.agree(their.spk));
    km.extend_from_slice(&ek_a.agree(their.ik));
    km.extend_from_slice(&ek_a.agree(their.spk));
    if let Some(opk) = their.opk {
        km.extend_from_slice(&ek_a.agree(opk));
    }
    let (ct, ss) = their.pqopk.unwrap_or(their.pqspk).encapsulate(kem_m)?;
    km.extend_from_slice(&ss);
    Ok(Initiated { sk: kdf(&km), ek: ek_a.public(), kem_ciphertext: ct })
}

/// The responder's keys the initial message names.
pub struct Responder<'a> {
    pub ik: &'a DhSecret,
    pub spk: &'a DhSecret,
    pub pqspk: &'a KemSecret,
    pub opk: Option<&'a DhSecret>,
    pub pqopk: Option<&'a KemSecret>,
}

/// The responder's side of the same computation.
pub fn respond(me: &Responder, ik_a: &DhPublic, ek_a: &DhPublic, kem_ciphertext: &[u8]) -> Result<[u8; 32], String> {
    let mut km = Vec::with_capacity(5 * 32);
    km.extend_from_slice(&me.spk.agree(ik_a));
    km.extend_from_slice(&me.ik.agree(ek_a));
    km.extend_from_slice(&me.spk.agree(ek_a));
    if let Some(opk) = me.opk {
        km.extend_from_slice(&opk.agree(ek_a));
    }
    let ss = me.pqopk.unwrap_or(me.pqspk).decapsulate(kem_ciphertext)?;
    km.extend_from_slice(&ss);
    Ok(kdf(&km))
}

/// The associated data both parties bind their first messages to: the
/// initiator's identity key then the responder's.
pub fn associated_data(ik_a: &DhPublic, ik_b: &DhPublic) -> Vec<u8> {
    let mut ad = Vec::with_capacity(64);
    ad.extend_from_slice(&ik_a.0);
    ad.extend_from_slice(&ik_b.0);
    ad
}

/// KDF(KM) = HKDF-SHA-256 with a zero salt of the hash's length, input
/// F ‖ KM where F is 32 bytes of 0xFF for X25519, and the info string.
pub fn kdf(km: &[u8]) -> [u8; 32] {
    let mut ikm = vec![0xFFu8; 32];
    ikm.extend_from_slice(km);
    let hk = Hkdf::<Sha256>::new(Some(&[0u8; 32]), &ikm);
    let mut out = [0u8; 32];
    hk.expand(INFO, &mut out).expect("32 bytes is within HKDF's bound");
    out
}

/// HKDF-SHA-256 for the ratchet's chains: `salt`, `ikm`, `info`, `out`.
pub fn hkdf_sha256(salt: &[u8], ikm: &[u8], info: &[u8], out: &mut [u8]) {
    Hkdf::<Sha256>::new(Some(salt), ikm).expand(info, out).expect("within HKDF's bound");
}
