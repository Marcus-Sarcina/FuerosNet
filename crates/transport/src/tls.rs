//! The TLS profile (§9.1) as rustls configuration, and quinn endpoints over it.

use ed25519_dalek::pkcs8::{DecodePublicKey, EncodePrivateKey, EncodePublicKey};
use rhtn_crypto::verify::{self, Delegation, Lookup};
use rhtn_crypto::{Identity, SigningIdentity};
use rustls::client::danger::{HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier};
use rustls::crypto::{CryptoProvider, aws_lc_rs};
use rustls::pki_types::{
    CertificateDer, PrivateKeyDer, ServerName, SubjectPublicKeyInfoDer, UnixTime,
};
use rustls::server::ProducesTickets;
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

    /// Pin `key_material`; refused unless it has the shape §2.2 gives an
    /// identity, both components in order and nothing else, and hashes to
    /// `keyhash`.
    pub fn pin(&self, keyhash: [u8; 32], key_material: &[u8]) -> Result<(), &'static str> {
        rhtn_codec::cose::check_key_material(key_material).map_err(|e| e.0)?;
        if rhtn_codec::cose::sha256(key_material) != keyhash {
            return Err("key material does not hash to the keyhash");
        }
        self.inner
            .lock()
            .unwrap()
            .insert(keyhash, key_material.to_vec());
        Ok(())
    }

    pub fn pin_identity(&self, id: &rhtn_crypto::Identity) {
        self.pin(id.keyhash, &id.key_material())
            .expect("an identity hashes to its own keyhash");
    }

    /// The classical member's raw Ed25519 public key for a pinned keyhash.
    pub fn classical_key(&self, keyhash: &[u8; 32]) -> Option<[u8; 32]> {
        let km = self.inner.lock().unwrap().get(keyhash)?.clone();
        classical_member(&km)
    }

    /// The keyhash whose pinned classical member is `spki` (the presented raw key).
    pub fn keyhash_for_spki(&self, spki: &[u8]) -> Option<[u8; 32]> {
        let want = ed25519_dalek::VerifyingKey::from_public_key_der(spki)
            .ok()?
            .to_bytes();
        self.keyhash_for_key(&want)
    }

    /// The keyhash whose pinned classical member is the raw key `key`.
    pub fn keyhash_for_key(&self, key: &[u8; 32]) -> Option<[u8; 32]> {
        self.inner
            .lock()
            .unwrap()
            .iter()
            .find(|(_, km)| classical_member(km) == Some(*key))
            .map(|(kh, _)| *kh)
    }

    /// The identity a pinned keyhash names, for verifying what it signed:
    /// a delegation over a transport key (§8.2) is verified under the
    /// material pinned for its field 2.
    pub fn identity(&self, keyhash: &[u8; 32]) -> Option<Identity> {
        let km = self.inner.lock().unwrap().get(keyhash)?.clone();
        Identity::from_key_material(&km)
    }
}

/// The raw Ed25519 key inside a SubjectPublicKeyInfo, which is what the
/// handshake presented and what a delegation's field 1 is compared with.
pub fn key_of_spki(spki: &[u8]) -> Option<[u8; 32]> {
    Some(
        ed25519_dalek::VerifyingKey::from_public_key_der(spki)
            .ok()?
            .to_bytes(),
    )
}

/// The clock a credential reads: seconds since the Unix epoch.
pub type Clock = Arc<dyn Fn() -> u64 + Send + Sync>;

pub fn system_clock() -> Clock {
    Arc::new(|| {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0)
    })
}

/// One credential of a run: a delegation as issued, with the window it
/// states (§8.2).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Issued {
    pub raw: Vec<u8>,
    pub not_before: u64,
    pub not_after: u64,
}

/// A delegated transport credential (§8.2, design §23.3): the transport
/// keypair an instance minted for itself, the identity it speaks as, and
/// the run of delegations that identity signed over the public half.
///
/// **The private half never leaves the holder** (`infra-client-requirements.md`
/// §7): only [`Credential::public`] crosses the provisioning channel, and
/// the operator's client signs each credential of the run over it.  The
/// credentials rotate; the key does not until the holder is re-provisioned.
pub struct Credential {
    key: ed25519_dalek::SigningKey,
    /// The delegating identity.
    pub keyhash: [u8; 32],
    run: Mutex<Vec<Issued>>,
    clock: Clock,
}

impl std::fmt::Debug for Credential {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Credential(for {}, {} issued)",
            hex_prefix(&self.keyhash),
            self.run.lock().unwrap().len()
        )
    }
}

fn hex_prefix(b: &[u8]) -> String {
    b.iter().take(4).map(|x| format!("{x:02x}")).collect()
}

impl Credential {
    /// Mint a fresh transport keypair for `keyhash` from the provider's
    /// random source: what an instance does once at provisioning
    /// (`infra-client-requirements.md` §7).
    pub fn mint(keyhash: [u8; 32]) -> Self {
        Self::from_seed(&random_bytes::<32>(), keyhash)
    }

    /// A keypair from a stored seed: how a holder keeps the key across
    /// runs of the process, and how a test names one.
    pub fn from_seed(seed: &[u8; 32], keyhash: [u8; 32]) -> Self {
        Credential {
            key: ed25519_dalek::SigningKey::from_bytes(seed),
            keyhash,
            run: Mutex::new(Vec::new()),
            clock: system_clock(),
        }
    }

    pub fn with_clock(mut self, clock: Clock) -> Self {
        self.clock = clock;
        self
    }

    /// The public half, which the delegating identity signs over.
    pub fn public(&self) -> [u8; 32] {
        self.key.verifying_key().to_bytes()
    }

    pub(crate) fn signing_key(&self) -> &ed25519_dalek::SigningKey {
        &self.key
    }

    /// The signer of what a delegated key signs: a subtree acknowledgement
    /// and a currency attestation (`wire-format.md` §7.5, §7.1), never an
    /// envelope.
    pub fn signer(&self) -> rhtn_crypto::signer::DelegatedSigner {
        rhtn_crypto::signer::DelegatedSigner::new(self.keyhash, self.key.clone())
    }

    /// Take a delegation into the run: verified hybrid under the identity
    /// `ids` resolves for its field 2, which must be this credential's
    /// keyhash, and naming this credential's public key.  Kept in window
    /// order; one already held is not held twice.
    pub fn add<L: Lookup + ?Sized>(&self, ids: &L, raw: &[u8]) -> Result<Delegation, String> {
        let d = verify::delegation(ids, raw).map_err(|e| format!("{e:?}"))?;
        if d.keyhash != self.keyhash {
            return Err("delegation is by another identity".into());
        }
        if d.key != self.public() {
            return Err("delegation names another transport key".into());
        }
        let mut run = self.run.lock().unwrap();
        if !run.iter().any(|i| i.raw == raw) {
            run.push(Issued {
                raw: raw.to_vec(),
                not_before: d.not_before,
                not_after: d.not_after,
            });
            run.sort_by_key(|i| i.not_before);
        }
        Ok(d)
    }

    /// The credential in force at `now`: the newest whose window has
    /// opened.  Before the run opens, its first; after the run ends, its
    /// last, which a receiver refuses on the window and the holder is
    /// told about before it happens (`infra-client-requirements.md` §7).
    pub fn current_at(&self, now: u64) -> Option<Issued> {
        let run = self.run.lock().unwrap();
        run.iter()
            .rev()
            .find(|i| i.not_before <= now)
            .or_else(|| run.first())
            .cloned()
    }

    pub fn current(&self) -> Option<Issued> {
        self.current_at((self.clock)())
    }

    /// Seconds of validity the current credential has left, none when
    /// nothing is in force.
    pub fn remaining(&self) -> Option<u64> {
        let now = (self.clock)();
        let c = self.current_at(now)?;
        (c.not_before <= now).then(|| c.not_after.saturating_sub(now))
    }

    /// When the run ends: the last credential's `not_after`.
    pub fn run_end(&self) -> Option<u64> {
        self.run.lock().unwrap().last().map(|i| i.not_after)
    }

    pub fn issued(&self) -> Vec<Issued> {
        self.run.lock().unwrap().clone()
    }
}

/// What a peer presents in the handshake (§9.1): the classical member of
/// its own identity, or a transport key that identity delegated.
#[derive(Clone)]
pub enum Presenter {
    Own {
        keyhash: [u8; 32],
        ed: Box<ed25519_dalek::SigningKey>,
    },
    Delegated(Arc<Credential>),
}

impl Presenter {
    pub fn own(id: &SigningIdentity) -> Self {
        Presenter::Own {
            keyhash: id.public.keyhash,
            ed: Box::new(id.ed_signing_key().clone()),
        }
    }

    /// The identity this presenter speaks as.
    pub fn keyhash(&self) -> [u8; 32] {
        match self {
            Presenter::Own { keyhash, .. } => *keyhash,
            Presenter::Delegated(c) => c.keyhash,
        }
    }

    /// The raw key the handshake presents.
    pub fn presented_key(&self) -> [u8; 32] {
        match self {
            Presenter::Own { ed, .. } => ed.verifying_key().to_bytes(),
            Presenter::Delegated(c) => c.public(),
        }
    }

    fn signing_key(&self) -> &ed25519_dalek::SigningKey {
        match self {
            Presenter::Own { ed, .. } => ed,
            Presenter::Delegated(c) => c.signing_key(),
        }
    }

    pub fn credential(&self) -> Option<&Arc<Credential>> {
        match self {
            Presenter::Delegated(c) => Some(c),
            Presenter::Own { .. } => None,
        }
    }

    /// The delegation to present now (§8.2): the credential in force, or
    /// nothing for a peer presenting its own classical member, which owes
    /// no frame.
    pub fn delegation(&self) -> Option<Vec<u8>> {
        self.credential().and_then(|c| c.current()).map(|i| i.raw)
    }
}

/// Who a configuration speaks as: the keyhash, and what it presents in
/// the handshake for that keyhash.  A device holding a delegation and no
/// seed is a party as much as the one holding the seed (design §23.3).
#[derive(Clone)]
pub struct Party {
    pub keyhash: [u8; 32],
    pub presenter: Presenter,
}

impl Party {
    pub fn of(p: impl Into<Presenter>) -> Self {
        let presenter = p.into();
        Party {
            keyhash: presenter.keyhash(),
            presenter,
        }
    }
    pub fn credential(&self) -> Option<&Arc<Credential>> {
        self.presenter.credential()
    }
}

impl From<Arc<SigningIdentity>> for Presenter {
    fn from(id: Arc<SigningIdentity>) -> Self {
        Presenter::own(&id)
    }
}

impl From<SigningIdentity> for Presenter {
    fn from(id: SigningIdentity) -> Self {
        Presenter::own(&id)
    }
}

impl From<&SigningIdentity> for Presenter {
    fn from(id: &SigningIdentity) -> Self {
        Presenter::own(id)
    }
}

impl From<&Arc<SigningIdentity>> for Presenter {
    fn from(id: &Arc<SigningIdentity>) -> Self {
        Presenter::own(id)
    }
}

impl From<Arc<Credential>> for Presenter {
    fn from(c: Arc<Credential>) -> Self {
        Presenter::Delegated(c)
    }
}

impl From<&Arc<Credential>> for Presenter {
    fn from(c: &Arc<Credential>) -> Self {
        Presenter::Delegated(c.clone())
    }
}

impl From<&Presenter> for Presenter {
    fn from(p: &Presenter) -> Self {
        p.clone()
    }
}

/// The 32-byte Ed25519 key in a `KeyMaterial` array (§2.2): first COSE_Key,
/// label -2.
pub fn classical_member(key_material: &[u8]) -> Option<[u8; 32]> {
    use rhtn_codec::cbor::*;
    let __km = parse_all(key_material).ok()?;
    let Item::Array(a) = &__km else { return None };
    let Item::Map(m) = a.first()? else {
        return None;
    };
    m.iter().find_map(|(k, v)| match (k, v) {
        (Item::Neg(-2), Item::Bytes(r)) if r.len() == 32 => key_material[r.clone()].try_into().ok(),
        _ => None,
    })
}

pub fn spki_der(ed: &ed25519_dalek::VerifyingKey) -> Vec<u8> {
    ed.to_public_key_der()
        .expect("Ed25519 SPKI")
        .as_bytes()
        .to_vec()
}

/// The provider: aws-lc-rs with the one key-exchange group the profile names.
pub fn provider() -> Arc<CryptoProvider> {
    let mut p = aws_lc_rs::default_provider();
    p.kx_groups = vec![aws_lc_rs::kx_group::X25519MLKEM768];
    Arc::new(p)
}

/// The key a presenter offers, as the raw public key TLS presents.
fn certified_key(provider: &CryptoProvider, me: &Presenter) -> Arc<CertifiedKey> {
    let sk = me.signing_key();
    let pkcs8 = sk.to_pkcs8_der().expect("PKCS#8");
    let key = provider
        .key_provider
        .load_private_key(PrivateKeyDer::Pkcs8(pkcs8.as_bytes().to_vec().into()))
        .expect("Ed25519 is supported");
    Arc::new(CertifiedKey::new(
        vec![CertificateDer::from(spki_der(&sk.verifying_key()))],
        key,
    ))
}

/// Dialling side: any well-formed Ed25519 raw public key completes the
/// handshake.  **Which identity it speaks as is decided after it**, by
/// §9.1's three-way bind (`bind`): the pinned classical member, a
/// delegation held from the topology class, or the delegation the peer
/// presents first on the connection.  A handshake that refused every key
/// but the pinned one would leave no way to read the delegation, so the
/// verifier admits the key and the bind refuses the connection.
#[derive(Debug)]
struct AnyRawKeyServer {
    provider: Arc<CryptoProvider>,
}

impl ServerCertVerifier for AnyRawKeyServer {
    fn verify_server_cert(
        &self,
        end_entity: &CertificateDer<'_>,
        _intermediates: &[CertificateDer<'_>],
        _server_name: &ServerName<'_>,
        _ocsp: &[u8],
        _now: UnixTime,
    ) -> Result<ServerCertVerified, TlsError> {
        ed25519_dalek::VerifyingKey::from_public_key_der(end_entity.as_ref())
            .map(|_| ServerCertVerified::assertion())
            .map_err(|_| TlsError::General("server raw public key is not an Ed25519 SPKI".into()))
    }
    fn verify_tls12_signature(
        &self,
        _m: &[u8],
        _c: &CertificateDer<'_>,
        _d: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, TlsError> {
        Err(TlsError::PeerIncompatible(
            rustls::PeerIncompatible::Tls12NotOffered,
        ))
    }
    fn verify_tls13_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, TlsError> {
        rustls::crypto::verify_tls13_signature_with_raw_key(
            message,
            &SubjectPublicKeyInfoDer::from(cert.as_ref()),
            dss,
            &self.provider.signature_verification_algorithms,
        )
    }
    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
        self.provider
            .signature_verification_algorithms
            .supported_schemes()
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
    fn verify_client_cert(
        &self,
        end_entity: &CertificateDer<'_>,
        _intermediates: &[CertificateDer<'_>],
        _now: UnixTime,
    ) -> Result<ClientCertVerified, TlsError> {
        ed25519_dalek::VerifyingKey::from_public_key_der(end_entity.as_ref())
            .map(|_| ClientCertVerified::assertion())
            .map_err(|_| TlsError::General("client raw public key is not an Ed25519 SPKI".into()))
    }
    fn verify_tls12_signature(
        &self,
        _m: &[u8],
        _c: &CertificateDer<'_>,
        _d: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, TlsError> {
        Err(TlsError::PeerIncompatible(
            rustls::PeerIncompatible::Tls12NotOffered,
        ))
    }
    fn verify_tls13_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, TlsError> {
        rustls::crypto::verify_tls13_signature_with_raw_key(
            message,
            &SubjectPublicKeyInfoDer::from(cert.as_ref()),
            dss,
            &self.provider.signature_verification_algorithms,
        )
    }
    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
        self.provider
            .signature_verification_algorithms
            .supported_schemes()
    }
    fn requires_raw_public_keys(&self) -> bool {
        true
    }
}

/// A client configuration authenticating as `me`.  `groups` defaults to
/// the profile's one group; a test peer may pass others.  The server's key
/// is admitted here and bound after the handshake (§9.1).
pub fn client_config_with(
    me: impl Into<Presenter>,
    groups: Vec<&'static dyn rustls::crypto::SupportedKxGroup>,
) -> rustls::ClientConfig {
    let me = me.into();
    let mut p = aws_lc_rs::default_provider();
    p.kx_groups = groups;
    let provider = Arc::new(p);
    let mut cfg = rustls::ClientConfig::builder_with_provider(provider.clone())
        .with_protocol_versions(&[&rustls::version::TLS13])
        .expect("TLS 1.3")
        .dangerous()
        .with_custom_certificate_verifier(Arc::new(AnyRawKeyServer {
            provider: provider.clone(),
        }))
        .with_client_cert_resolver(Arc::new(
            rustls::client::AlwaysResolvesClientRawPublicKeys::new(certified_key(&provider, &me)),
        ));
    cfg.alpn_protocols = vec![ALPN.to_vec()];
    cfg.enable_early_data = true;
    cfg
}

/// A client configuration for reaching `target`: none where nothing is
/// pinned for it, since a peer whose `KeyMaterial` the dialler does not
/// hold can be bound by nothing (§9.1), a delegation included.
pub fn client_config(
    me: impl Into<Presenter>,
    pins: &Pins,
    target: &[u8; 32],
) -> Option<rustls::ClientConfig> {
    pins.classical_key(target)?;
    Some(client_config_with(
        me,
        vec![aws_lc_rs::kx_group::X25519MLKEM768],
    ))
}

/// A server configuration presenting `me`'s key and requiring a raw public
/// key from every client.  A delegated presenter's resumption tickets are
/// clamped to its credential's remaining validity (design §14.1.3).
pub fn server_config_with(
    me: impl Into<Presenter>,
    groups: Vec<&'static dyn rustls::crypto::SupportedKxGroup>,
) -> rustls::ServerConfig {
    let me = me.into();
    let mut p = aws_lc_rs::default_provider();
    p.kx_groups = groups;
    let provider = Arc::new(p);
    let mut cfg = rustls::ServerConfig::builder_with_provider(provider.clone())
        .with_protocol_versions(&[&rustls::version::TLS13])
        .expect("TLS 1.3")
        .with_client_cert_verifier(Arc::new(AnyRawKeyClient {
            provider: provider.clone(),
        }))
        .with_cert_resolver(Arc::new(
            rustls::server::AlwaysResolvesServerRawPublicKeys::new(certified_key(&provider, &me)),
        ));
    cfg.alpn_protocols = vec![ALPN.to_vec()];
    cfg.max_early_data_size = u32::MAX;
    cfg.send_half_rtt_data = false;
    if let Some(c) = me.credential() {
        cfg.ticketer = Arc::new(ClampedTicketer {
            inner: aws_lc_rs::Ticketer::new().expect("the provider's ticketer"),
            credential: c.clone(),
        });
    }
    cfg
}

/// A ticketer whose lifetime never exceeds the credential's remaining
/// validity: a resumption ticket that outlived the delegation would let a
/// 0-RTT attach ride a key nothing binds any more (design §14.1.3).  The
/// ticket keys themselves are the provider's, rolled as it rolls them.
#[derive(Debug)]
struct ClampedTicketer {
    inner: Arc<dyn ProducesTickets>,
    credential: Arc<Credential>,
}

impl ProducesTickets for ClampedTicketer {
    fn enabled(&self) -> bool {
        self.inner.enabled()
    }
    fn lifetime(&self) -> u32 {
        let left = self.credential.remaining().unwrap_or(0);
        self.inner.lifetime().min(left.min(u32::MAX as u64) as u32)
    }
    fn encrypt(&self, plain: &[u8]) -> Option<Vec<u8>> {
        // nothing left to live: no ticket, and rustls sends none
        (self.lifetime() > 0)
            .then(|| self.inner.encrypt(plain))
            .flatten()
    }
    fn decrypt(&self, cipher: &[u8]) -> Option<Vec<u8>> {
        self.inner.decrypt(cipher)
    }
}

pub fn server_config(me: impl Into<Presenter>) -> rustls::ServerConfig {
    server_config_with(me, vec![aws_lc_rs::kx_group::X25519MLKEM768])
}

/// A quinn server endpoint on `addr` for `me`.
pub fn server_endpoint(
    me: impl Into<Presenter>,
    addr: std::net::SocketAddr,
) -> std::io::Result<quinn::Endpoint> {
    server_endpoint_with(server_config(me), addr)
}

pub fn server_endpoint_with(
    cfg: rustls::ServerConfig,
    addr: std::net::SocketAddr,
) -> std::io::Result<quinn::Endpoint> {
    let crypto =
        quinn::crypto::rustls::QuicServerConfig::try_from(cfg).expect("quinn accepts the profile");
    let mut qcfg = quinn::ServerConfig::with_crypto(Arc::new(crypto));
    qcfg.transport_config(Arc::new(transport_config()));
    quinn::Endpoint::server(qcfg, addr)
}

/// A quinn client endpoint bound to `addr` with no default configuration;
/// `dial` supplies one per target.
pub fn client_endpoint(addr: std::net::SocketAddr) -> std::io::Result<quinn::Endpoint> {
    quinn::Endpoint::client(addr)
}

pub fn transport_config() -> quinn::TransportConfig {
    let mut t = quinn::TransportConfig::default();
    // Liveness is the session's heartbeat, not QUIC's idle timer; keep the
    // idle timeout above any advertised interval so it never pre-empts §8.2.
    t.max_idle_timeout(Some(
        quinn::IdleTimeout::try_from(std::time::Duration::from_secs(4000)).unwrap(),
    ));
    t
}

/// Dial `target` at `addr` from `endpoint`, authenticating as `me`.  Fails
/// before any stream is opened when nothing is pinned for `target`.  The
/// handshake admits whatever Ed25519 key the peer presents; what it is
/// bound to is `bind`'s answer afterwards, and a caller that does not
/// bind has reached nobody in particular.
pub fn dial(
    endpoint: &quinn::Endpoint,
    me: impl Into<Presenter>,
    pins: &Pins,
    target: &[u8; 32],
    addr: std::net::SocketAddr,
) -> Result<quinn::Connecting, DialError> {
    let cfg = client_config(me, pins, target).ok_or(DialError::NotPinned)?;
    dial_with(endpoint, cfg, addr)
}

pub fn dial_with(
    endpoint: &quinn::Endpoint,
    cfg: rustls::ClientConfig,
    addr: std::net::SocketAddr,
) -> Result<quinn::Connecting, DialError> {
    let crypto =
        quinn::crypto::rustls::QuicClientConfig::try_from(cfg).map_err(|_| DialError::Config)?;
    let mut qcfg = quinn::ClientConfig::new(Arc::new(crypto));
    qcfg.transport_config(Arc::new(transport_config()));
    endpoint
        .connect_with(qcfg, addr, SERVER_NAME)
        .map_err(|_| DialError::Config)
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

/// The raw Ed25519 key the peer presented on a connection.
pub fn peer_key(conn: &quinn::Connection) -> Option<[u8; 32]> {
    key_of_spki(&peer_spki(conn)?)
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
    aws_lc_rs::default_provider()
        .secure_random
        .fill(&mut out)
        .expect("the provider's random source");
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use rhtn_crypto::identity::testkit::test_identity;
    use std::sync::atomic::{AtomicU64, Ordering};

    /// The ticket lifetime at issue is at most the delegation's remaining
    /// validity, and no ticket is issued once nothing is left (design
    /// §14.1.3; TRN-27's first clause).
    #[test]
    fn tickets_never_outlive_the_credential() {
        let t = 1_800_000_000u64;
        let cell = Arc::new(AtomicU64::new(t + 172_800 - 3600));
        let c = cell.clone();
        let bob = test_identity("bob");
        let cred = Credential::from_seed(&[1; 32], bob.public.keyhash)
            .with_clock(Arc::new(move || c.load(Ordering::SeqCst)));
        cred.add(
            std::slice::from_ref(&bob.public),
            &rhtn_crypto::delegation::issue(&bob, &cred.public(), t),
        )
        .unwrap();
        let ticketer = ClampedTicketer {
            inner: aws_lc_rs::Ticketer::new().unwrap(),
            credential: Arc::new(cred),
        };
        assert!(ticketer.enabled());
        assert!(ticketer.lifetime() <= 3600, "{}", ticketer.lifetime());
        assert!(ticketer.encrypt(b"state").is_some());
        cell.store(t + 172_800 + 1, Ordering::SeqCst);
        assert_eq!(ticketer.lifetime(), 0);
        assert!(
            ticketer.encrypt(b"state").is_none(),
            "no ticket past the window"
        );
    }
}
