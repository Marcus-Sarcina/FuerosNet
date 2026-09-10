//! The TLS profile (§9.1) as rustls configuration, and quinn endpoints over it.

use ed25519_dalek::pkcs8::{DecodePublicKey, EncodePrivateKey, EncodePublicKey};
use rhtn_crypto::SigningIdentity;
use rustls::client::danger::{HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier};
use rustls::crypto::{CryptoProvider, aws_lc_rs};
use rustls::pki_types::{CertificateDer, PrivateKeyDer, ServerName, SubjectPublicKeyInfoDer, UnixTime};
use rustls::server::danger::{ClientCertVerified, ClientCertVerifier};
use rustls::sign::CertifiedKey;
use rustls::{DigitallySignedStruct, DistinguishedName, Error as TlsError, SignatureScheme};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

pub const ALPN: &[u8] = b"rhtn/1";
/// The name the dialling party hands TLS; identities are keyhashes and the
/// verifier ignores it (§9.1).
pub const SERVER_NAME: &str = "rhtn";

/// Pinned key material: keyhash to the `KeyMaterial` bytes that hash to it.
/// The classical member's SubjectPublicKeyInfo is what a raw-public-key
/// handshake presents, so a pin is also what authenticates a peer.
#[derive(Default, Clone)]
pub struct Pins {
    inner: Arc<Mutex<HashMap<[u8; 32], Vec<u8>>>>,
}

impl Pins {
    pub fn new() -> Self {
        Self::default()
    }

    /// Pin `key_material`; refused if it does not hash to `keyhash`.
    pub fn pin(&self, keyhash: [u8; 32], key_material: &[u8]) -> Result<(), &'static str> {
        if rhtn_codec::cose::sha256(key_material) != keyhash {
            return Err("key material does not hash to the keyhash");
        }
        self.inner.lock().unwrap().insert(keyhash, key_material.to_vec());
        Ok(())
    }

    pub fn pin_identity(&self, id: &rhtn_crypto::Identity) {
        self.pin(id.keyhash, &id.key_material()).expect("an identity hashes to its own keyhash");
    }

    /// The classical member's raw Ed25519 public key for a pinned keyhash.
    pub fn classical_key(&self, keyhash: &[u8; 32]) -> Option<[u8; 32]> {
        let km = self.inner.lock().unwrap().get(keyhash)?.clone();
        classical_member(&km)
    }

    /// The keyhash whose pinned classical member is `spki` (the presented raw key).
    pub fn keyhash_for_spki(&self, spki: &[u8]) -> Option<[u8; 32]> {
        let want = ed25519_dalek::VerifyingKey::from_public_key_der(spki).ok()?.to_bytes();
        self.inner
            .lock()
            .unwrap()
            .iter()
            .find(|(_, km)| classical_member(km) == Some(want))
            .map(|(kh, _)| *kh)
    }
}

/// The 32-byte Ed25519 key in a `KeyMaterial` array (§2.2): first COSE_Key,
/// label -2.
pub fn classical_member(key_material: &[u8]) -> Option<[u8; 32]> {
    use rhtn_codec::cbor::*;
    let __km = parse_all(key_material).ok()?;
    let Item::Array(a) = &__km else { return None };
    let Item::Map(m) = a.first()? else { return None };
    m.iter().find_map(|(k, v)| match (k, v) {
        (Item::Neg(-2), Item::Bytes(r)) if r.len() == 32 => key_material[r.clone()].try_into().ok(),
        _ => None,
    })
}

pub fn spki_der(ed: &ed25519_dalek::VerifyingKey) -> Vec<u8> {
    ed.to_public_key_der().expect("Ed25519 SPKI").as_bytes().to_vec()
}

/// The provider: aws-lc-rs with the one key-exchange group the profile names.
pub fn provider() -> Arc<CryptoProvider> {
    let mut p = aws_lc_rs::default_provider();
    p.kx_groups = vec![aws_lc_rs::kx_group::X25519MLKEM768];
    Arc::new(p)
}

/// The identity's classical component as the raw public key TLS presents.
fn certified_key(provider: &CryptoProvider, id: &SigningIdentity) -> Arc<CertifiedKey> {
    let pkcs8 = id.ed_signing_key().to_pkcs8_der().expect("PKCS#8");
    let key = provider
        .key_provider
        .load_private_key(PrivateKeyDer::Pkcs8(pkcs8.as_bytes().to_vec().into()))
        .expect("Ed25519 is supported");
    Arc::new(CertifiedKey::new(vec![CertificateDer::from(spki_der(&id.public.ed))], key))
}

/// Dialling side: the presented raw key must be the pinned classical member
/// of the keyhash being reached (§9.1).
#[derive(Debug)]
struct PinnedServer {
    expected_spki: Vec<u8>,
    provider: Arc<CryptoProvider>,
}

impl ServerCertVerifier for PinnedServer {
    fn verify_server_cert(
        &self,
        end_entity: &CertificateDer<'_>,
        _intermediates: &[CertificateDer<'_>],
        _server_name: &ServerName<'_>,
        _ocsp: &[u8],
        _now: UnixTime,
    ) -> Result<ServerCertVerified, TlsError> {
        if end_entity.as_ref() == self.expected_spki.as_slice() {
            Ok(ServerCertVerified::assertion())
        } else {
            Err(TlsError::General("presented raw public key is not the pinned classical member".into()))
        }
    }
    fn verify_tls12_signature(&self, _m: &[u8], _c: &CertificateDer<'_>, _d: &DigitallySignedStruct) -> Result<HandshakeSignatureValid, TlsError> {
        Err(TlsError::PeerIncompatible(rustls::PeerIncompatible::Tls12NotOffered))
    }
    fn verify_tls13_signature(&self, message: &[u8], cert: &CertificateDer<'_>, dss: &DigitallySignedStruct) -> Result<HandshakeSignatureValid, TlsError> {
        rustls::crypto::verify_tls13_signature_with_raw_key(message, &SubjectPublicKeyInfoDer::from(cert.as_ref()), dss, &self.provider.signature_verification_algorithms)
    }
    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
        self.provider.signature_verification_algorithms.supported_schemes()
    }
    fn requires_raw_public_keys(&self) -> bool {
        true
    }
}

/// Serving side: any well-formed Ed25519 raw public key authenticates the
/// connection; which identity it is, if any, is the pin table's answer when
/// `Attach` names one (§9.1's binding).
#[derive(Debug)]
struct AnyRawKeyClient {
    provider: Arc<CryptoProvider>,
}

impl ClientCertVerifier for AnyRawKeyClient {
    fn root_hint_subjects(&self) -> &[DistinguishedName] {
        &[]
    }
    fn verify_client_cert(&self, end_entity: &CertificateDer<'_>, _intermediates: &[CertificateDer<'_>], _now: UnixTime) -> Result<ClientCertVerified, TlsError> {
        ed25519_dalek::VerifyingKey::from_public_key_der(end_entity.as_ref())
            .map(|_| ClientCertVerified::assertion())
            .map_err(|_| TlsError::General("client raw public key is not an Ed25519 SPKI".into()))
    }
    fn verify_tls12_signature(&self, _m: &[u8], _c: &CertificateDer<'_>, _d: &DigitallySignedStruct) -> Result<HandshakeSignatureValid, TlsError> {
        Err(TlsError::PeerIncompatible(rustls::PeerIncompatible::Tls12NotOffered))
    }
    fn verify_tls13_signature(&self, message: &[u8], cert: &CertificateDer<'_>, dss: &DigitallySignedStruct) -> Result<HandshakeSignatureValid, TlsError> {
        rustls::crypto::verify_tls13_signature_with_raw_key(message, &SubjectPublicKeyInfoDer::from(cert.as_ref()), dss, &self.provider.signature_verification_algorithms)
    }
    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
        self.provider.signature_verification_algorithms.supported_schemes()
    }
    fn requires_raw_public_keys(&self) -> bool {
        true
    }
}

/// A client configuration that reaches `target` and authenticates as `me`.
/// `groups` defaults to the profile's one group; a test peer may pass others.
pub fn client_config_with(me: &SigningIdentity, target_classical: &[u8; 32], groups: Vec<&'static dyn rustls::crypto::SupportedKxGroup>) -> rustls::ClientConfig {
    let mut p = aws_lc_rs::default_provider();
    p.kx_groups = groups;
    let provider = Arc::new(p);
    let expected_spki = spki_der(&ed25519_dalek::VerifyingKey::from_bytes(target_classical).expect("pinned key"));
    let mut cfg = rustls::ClientConfig::builder_with_provider(provider.clone())
        .with_protocol_versions(&[&rustls::version::TLS13])
        .expect("TLS 1.3")
        .dangerous()
        .with_custom_certificate_verifier(Arc::new(PinnedServer { expected_spki, provider: provider.clone() }))
        .with_client_cert_resolver(Arc::new(rustls::client::AlwaysResolvesClientRawPublicKeys::new(certified_key(&provider, me))));
    cfg.alpn_protocols = vec![ALPN.to_vec()];
    cfg.enable_early_data = true;
    cfg
}

pub fn client_config(me: &SigningIdentity, pins: &Pins, target: &[u8; 32]) -> Option<rustls::ClientConfig> {
    let classical = pins.classical_key(target)?;
    Some(client_config_with(me, &classical, vec![aws_lc_rs::kx_group::X25519MLKEM768]))
}

/// A server configuration presenting `me`'s classical component and
/// requiring a raw public key from every client.
pub fn server_config_with(me: &SigningIdentity, groups: Vec<&'static dyn rustls::crypto::SupportedKxGroup>) -> rustls::ServerConfig {
    let mut p = aws_lc_rs::default_provider();
    p.kx_groups = groups;
    let provider = Arc::new(p);
    let mut cfg = rustls::ServerConfig::builder_with_provider(provider.clone())
        .with_protocol_versions(&[&rustls::version::TLS13])
        .expect("TLS 1.3")
        .with_client_cert_verifier(Arc::new(AnyRawKeyClient { provider: provider.clone() }))
        .with_cert_resolver(Arc::new(rustls::server::AlwaysResolvesServerRawPublicKeys::new(certified_key(&provider, me))));
    cfg.alpn_protocols = vec![ALPN.to_vec()];
    cfg.max_early_data_size = u32::MAX;
    cfg.send_half_rtt_data = false;
    cfg
}

pub fn server_config(me: &SigningIdentity) -> rustls::ServerConfig {
    server_config_with(me, vec![aws_lc_rs::kx_group::X25519MLKEM768])
}

/// A quinn server endpoint on `addr` for `me`.
pub fn server_endpoint(me: &SigningIdentity, addr: std::net::SocketAddr) -> std::io::Result<quinn::Endpoint> {
    server_endpoint_with(server_config(me), addr)
}

pub fn server_endpoint_with(cfg: rustls::ServerConfig, addr: std::net::SocketAddr) -> std::io::Result<quinn::Endpoint> {
    let crypto = quinn::crypto::rustls::QuicServerConfig::try_from(cfg).expect("quinn accepts the profile");
    let mut qcfg = quinn::ServerConfig::with_crypto(Arc::new(crypto));
    qcfg.transport_config(Arc::new(transport_config()));
    quinn::Endpoint::server(qcfg, addr)
}

/// A quinn client endpoint bound to `addr` with no default configuration;
/// `dial` supplies one per target.
pub fn client_endpoint(addr: std::net::SocketAddr) -> std::io::Result<quinn::Endpoint> {
    quinn::Endpoint::client(addr)
}

fn transport_config() -> quinn::TransportConfig {
    let mut t = quinn::TransportConfig::default();
    // Liveness is the session's heartbeat, not QUIC's idle timer; keep the
    // idle timeout above any advertised interval so it never pre-empts §8.2.
    t.max_idle_timeout(Some(quinn::IdleTimeout::try_from(std::time::Duration::from_secs(4000)).unwrap()));
    t
}

/// Dial `target` at `addr` from `endpoint`, authenticating as `me` with the
/// pinned material for `target`.  Fails before any stream is opened when the
/// presented key is not the pinned classical member.
pub fn dial(endpoint: &quinn::Endpoint, me: &SigningIdentity, pins: &Pins, target: &[u8; 32], addr: std::net::SocketAddr) -> Result<quinn::Connecting, DialError> {
    let cfg = client_config(me, pins, target).ok_or(DialError::NotPinned)?;
    dial_with(endpoint, cfg, addr)
}

pub fn dial_with(endpoint: &quinn::Endpoint, cfg: rustls::ClientConfig, addr: std::net::SocketAddr) -> Result<quinn::Connecting, DialError> {
    let crypto = quinn::crypto::rustls::QuicClientConfig::try_from(cfg).map_err(|_| DialError::Config)?;
    let mut qcfg = quinn::ClientConfig::new(Arc::new(crypto));
    qcfg.transport_config(Arc::new(transport_config()));
    endpoint.connect_with(qcfg, addr, SERVER_NAME).map_err(|_| DialError::Config)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DialError {
    /// No pinned key material for the target keyhash: nothing to authenticate against.
    NotPinned,
    Config,
}

/// The raw public key the peer presented on a connection, as SPKI DER.
pub fn peer_spki(conn: &quinn::Connection) -> Option<Vec<u8>> {
    let id = conn.peer_identity()?;
    let certs = id.downcast::<Vec<CertificateDer<'static>>>().ok()?;
    certs.first().map(|c| c.as_ref().to_vec())
}

/// The negotiated application protocol on a connection.
pub fn negotiated_alpn(conn: &quinn::Connection) -> Option<Vec<u8>> {
    let hd = conn.handshake_data()?;
    let hd = hd.downcast::<quinn::crypto::rustls::HandshakeData>().ok()?;
    hd.protocol
}

/// `N` random bytes from the crypto provider the transport already runs on.
/// One source of randomness for every nonce the node makes.
pub fn random_bytes<const N: usize>() -> [u8; N] {
    let mut out = [0u8; N];
    aws_lc_rs::default_provider().secure_random.fill(&mut out).expect("the provider's random source");
    out
}
