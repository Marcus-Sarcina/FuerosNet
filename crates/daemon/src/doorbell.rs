//! **The doorbell** (design §14.1.5; `infra-client-requirements.md` §6.1):
//! a content-free post to the endpoint a client registered, rung when a
//! relay is taken for that client while it holds no session here, and
//! nothing else.  The shape is the one the design adopted rather than a
//! vendor's: RFC 8030's push message, its body encrypted to the client's
//! key as RFC 8291 has it (one record, an empty plaintext and the padding
//! delimiter, AES-128-GCM under keys HKDF derives from an ECDH agreement
//! and the client's auth secret), and the request signed with the key the
//! client lent for it as RFC 8292's VAPID token (ES256 over the endpoint's
//! origin and an expiry).
//!
//! **What travels says nothing**: no payload, no sender, no count, and the
//! service carrying it cannot read even the nothing that is in it.  **What
//! this node keeps says nothing either**: a ring is not logged, counted or
//! stored (`infra-client-requirements.md` §6.1, "do not log doorbells"), so
//! a failure here is a failure and not a line.
//!
//! Two choices the documents leave to an implementation are taken here
//! and are this implementation's, not the design's [author, 2026-11-15:
//! accepted as such, and deliberately not made a requirement]: the token
//! carries no `sub` contact claim and nothing configures one, nothing
//! being registered to this project; and a post's `TTL` is a day, or the
//! time to the endpoint's lapse where that is sooner.  Either may change
//! without a document changing.

use aws_lc_rs::{aead, agreement, hkdf, rand, signature};
use rhtn_node::wake::Endpoint;
use std::io::{Read, Write};
use std::net::{TcpStream, ToSocketAddrs};
use std::sync::Arc;
use std::time::Duration;

/// How long a push service may hold the doorbell before dropping it,
/// where the endpoint names no sooner lapse.
const TTL_DEFAULT: u64 = 86_400;
/// How long a VAPID token is good for: RFC 8292 allows a day, and a token
/// minted per post needs far less.
const TOKEN_LIFE: u64 = 12 * 3600;
/// How long one post may take to connect, send and be answered.
const PATIENCE: Duration = Duration::from_secs(15);
/// The record size the body's header states (RFC 8188): one record.
const RECORD_SIZE: u32 = 4096;

/// The length a derivation produces.
struct Len(usize);
impl hkdf::KeyType for Len {
    fn len(&self) -> usize {
        self.0
    }
}

/// **RFC 8291's encryption, step by step**, with the server's ephemeral
/// key and the salt given so the published vector can hold it: the
/// aes128gcm body the push service forwards, header included.  `ua_public`
/// is the client's 65-byte uncompressed P-256 point and `auth` its 16-byte
/// auth secret, which is what the registration's key carries.
pub fn rfc8291_encrypt(
    as_private: &agreement::PrivateKey,
    salt: &[u8; 16],
    ua_public: &[u8],
    auth: &[u8],
    plaintext: &[u8],
) -> Result<Vec<u8>, String> {
    if ua_public.len() != 65 || ua_public[0] != 4 {
        return Err("the client's key is not an uncompressed P-256 point".into());
    }
    if auth.len() != 16 {
        return Err("the client's auth secret is 16 bytes".into());
    }
    let as_public = as_private
        .compute_public_key()
        .map_err(|_| "the server key's public point")?;
    let as_public = as_public.as_ref().to_vec();
    let peer = agreement::UnparsedPublicKey::new(&agreement::ECDH_P256, ua_public);
    let ecdh = agreement::agree(as_private, peer, (), |s| Ok(s.to_vec()))
        .map_err(|()| "the agreement with the client's key".to_string())?;
    // IKM = HKDF-Expand(HKDF-Extract(auth, ecdh), "WebPush: info" || 0 || ua || as, 32)
    let mut ikm = [0u8; 32];
    hkdf::Salt::new(hkdf::HKDF_SHA256, auth)
        .extract(&ecdh)
        .expand(&[b"WebPush: info\0", ua_public, &as_public], Len(32))
        .and_then(|okm| okm.fill(&mut ikm))
        .map_err(|_| "the key derivation")?;
    let prk = hkdf::Salt::new(hkdf::HKDF_SHA256, salt).extract(&ikm);
    let mut cek = [0u8; 16];
    prk.expand(&[b"Content-Encoding: aes128gcm\0"], Len(16))
        .and_then(|okm| okm.fill(&mut cek))
        .map_err(|_| "the content key")?;
    let mut nonce = [0u8; 12];
    prk.expand(&[b"Content-Encoding: nonce\0"], Len(12))
        .and_then(|okm| okm.fill(&mut nonce))
        .map_err(|_| "the nonce")?;
    // one record: the plaintext, the padding delimiter, the tag
    let mut record = plaintext.to_vec();
    record.push(2);
    let key = aead::LessSafeKey::new(
        aead::UnboundKey::new(&aead::AES_128_GCM, &cek).map_err(|_| "the content key")?,
    );
    key.seal_in_place_append_tag(
        aead::Nonce::assume_unique_for_key(nonce),
        aead::Aad::empty(),
        &mut record,
    )
    .map_err(|_| "sealing the record")?;
    let mut body = Vec::with_capacity(86 + record.len());
    body.extend_from_slice(salt);
    body.extend_from_slice(&RECORD_SIZE.to_be_bytes());
    body.push(65);
    body.extend_from_slice(&as_public);
    body.extend_from_slice(&record);
    Ok(body)
}

/// The inverse, for a test that holds the client's private key: the
/// plaintext a body carried, the delimiter stripped.
pub fn rfc8291_decrypt(
    ua_private: &agreement::PrivateKey,
    auth: &[u8],
    body: &[u8],
) -> Result<Vec<u8>, String> {
    if body.len() < 86 + 17 {
        return Err("too short to be a record".into());
    }
    let salt = &body[..16];
    let as_public = &body[21..86];
    let ua_public = ua_private
        .compute_public_key()
        .map_err(|_| "the client key's public point")?;
    let peer = agreement::UnparsedPublicKey::new(&agreement::ECDH_P256, as_public);
    let ecdh = agreement::agree(ua_private, peer, (), |s| Ok(s.to_vec()))
        .map_err(|()| "the agreement with the server's key".to_string())?;
    let mut ikm = [0u8; 32];
    hkdf::Salt::new(hkdf::HKDF_SHA256, auth)
        .extract(&ecdh)
        .expand(
            &[b"WebPush: info\0", ua_public.as_ref(), as_public],
            Len(32),
        )
        .and_then(|okm| okm.fill(&mut ikm))
        .map_err(|_| "the key derivation")?;
    let prk = hkdf::Salt::new(hkdf::HKDF_SHA256, salt).extract(&ikm);
    let mut cek = [0u8; 16];
    prk.expand(&[b"Content-Encoding: aes128gcm\0"], Len(16))
        .and_then(|okm| okm.fill(&mut cek))
        .map_err(|_| "the content key")?;
    let mut nonce = [0u8; 12];
    prk.expand(&[b"Content-Encoding: nonce\0"], Len(12))
        .and_then(|okm| okm.fill(&mut nonce))
        .map_err(|_| "the nonce")?;
    let key = aead::LessSafeKey::new(
        aead::UnboundKey::new(&aead::AES_128_GCM, &cek).map_err(|_| "the content key")?,
    );
    let mut record = body[86..].to_vec();
    let opened = key
        .open_in_place(
            aead::Nonce::assume_unique_for_key(nonce),
            aead::Aad::empty(),
            &mut record,
        )
        .map_err(|_| "the record does not open")?;
    match opened.last() {
        Some(2) => Ok(opened[..opened.len() - 1].to_vec()),
        _ => Err("no padding delimiter".into()),
    }
}

/// **The VAPID token** (RFC 8292): the JWT, ES256-signed with the lent
/// key, and the key's public point for the `k` parameter, both base64url.
/// `aud` is the endpoint's origin.
pub fn vapid_token(
    signing_key: &[u8; 32],
    aud: &str,
    exp: u64,
    sub: Option<&str>,
) -> Result<(String, String), String> {
    let scalar = agreement::PrivateKey::from_private_key(&agreement::ECDH_P256, signing_key)
        .map_err(|_| "the lent key is not a P-256 scalar")?;
    let public = scalar
        .compute_public_key()
        .map_err(|_| "the lent key's public point")?;
    let public = public.as_ref().to_vec();
    let pair = signature::EcdsaKeyPair::from_private_key_and_public_key(
        &signature::ECDSA_P256_SHA256_FIXED_SIGNING,
        signing_key,
        &public,
    )
    .map_err(|_| "the lent key does not sign")?;
    let header = base64url(br#"{"typ":"JWT","alg":"ES256"}"#);
    let claims = match sub {
        Some(s) => format!(
            r#"{{"aud":"{}","exp":{exp},"sub":"{}"}}"#,
            escape(aud),
            escape(s)
        ),
        None => format!(r#"{{"aud":"{}","exp":{exp}}}"#, escape(aud)),
    };
    let signing_input = format!("{header}.{}", base64url(claims.as_bytes()));
    let sig = pair
        .sign(&rand::SystemRandom::new(), signing_input.as_bytes())
        .map_err(|_| "signing the token")?;
    Ok((
        format!("{signing_input}.{}", base64url(sig.as_ref())),
        base64url(&public),
    ))
}

fn escape(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
}

pub use rhtn_node::http::base64url;

/// base64url without padding, back to bytes; nothing for a string that is
/// not that.
pub fn unbase64url(s: &str) -> Option<Vec<u8>> {
    const A: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";
    let s = s.trim_end_matches('=');
    let mut out = Vec::with_capacity(s.len() * 3 / 4);
    let mut acc: u32 = 0;
    let mut bits = 0;
    for c in s.bytes() {
        let v = A.iter().position(|a| *a == c)? as u32;
        acc = (acc << 6) | v;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((acc >> bits) as u8);
            acc &= (1 << bits) - 1;
        }
    }
    Some(out)
}

/// Where a post goes: the endpoint's URL, read.
struct Target {
    host: String,
    port: u16,
    path: String,
}

impl Target {
    fn parse(url: &str) -> Result<Target, String> {
        let rest = url
            .strip_prefix("https://")
            .ok_or("a doorbell endpoint is an https URL")?;
        let (authority, path) = match rest.find('/') {
            Some(i) => (&rest[..i], &rest[i..]),
            None => (rest, "/"),
        };
        let (host, port) = match authority.rsplit_once(':') {
            Some((h, p)) if !h.contains(':') => (h, p.parse::<u16>().map_err(|_| "the port")?),
            _ => (authority, 443),
        };
        if host.is_empty() {
            return Err("no host".into());
        }
        Ok(Target {
            host: host.trim_matches(['[', ']']).to_ascii_lowercase(),
            port,
            path: path.to_string(),
        })
    }
    /// The origin the token is for.
    fn origin(&self) -> String {
        if self.port == 443 {
            format!("https://{}", self.host)
        } else {
            format!("https://{}:{}", self.host, self.port)
        }
    }
}

/// **What rings**: the TLS this node posts with and the contact the token
/// names, if any.
#[derive(Clone)]
pub struct Bell {
    tls: Arc<rustls::ClientConfig>,
    contact: Option<String>,
}

impl Bell {
    /// With the operating system's roots, as a push service is reached.
    pub fn native(contact: Option<String>) -> Bell {
        let mut roots = rustls::RootCertStore::empty();
        for cert in rustls_native_certs::load_native_certs().certs {
            let _ = roots.add(cert);
        }
        let provider = Arc::new(rustls::crypto::aws_lc_rs::default_provider());
        let tls = rustls::ClientConfig::builder_with_provider(provider)
            .with_safe_default_protocol_versions()
            .expect("the provider's default versions")
            .with_root_certificates(roots)
            .with_no_client_auth();
        Bell {
            tls: Arc::new(tls),
            contact,
        }
    }

    /// With the TLS given: a test's endpoint under a key of its own.
    pub fn with_tls(tls: Arc<rustls::ClientConfig>, contact: Option<String>) -> Bell {
        Bell { tls, contact }
    }

    /// **Ring `e` once**: the status the service answered, or why not.
    /// Nothing here is logged; the caller holds the answer and should say
    /// nothing of it either beyond a count.
    pub fn ring(&self, e: &Endpoint, now: u64) -> Result<u16, String> {
        let target = Target::parse(&e.url)?;
        if e.key.len() != 81 {
            return Err(
                "the endpoint's key is the client's 65-byte point and 16-byte auth secret".into(),
            );
        }
        let (ua_public, auth) = e.key.split_at(65);
        let rng = rand::SystemRandom::new();
        let mut salt = [0u8; 16];
        rand::SecureRandom::fill(&rng, &mut salt).map_err(|_| "no randomness")?;
        let as_private = agreement::PrivateKey::generate(&agreement::ECDH_P256)
            .map_err(|_| "no ephemeral key")?;
        let body = rfc8291_encrypt(&as_private, &salt, ua_public, auth, &[])?;
        let (token, k) = vapid_token(
            &e.signing_key,
            &target.origin(),
            now + TOKEN_LIFE,
            self.contact.as_deref(),
        )?;
        let ttl = e
            .lapses_at
            .map(|t| t.saturating_sub(now).min(TTL_DEFAULT))
            .unwrap_or(TTL_DEFAULT);
        let headers = [
            ("authorization", format!("vapid t={token}, k={k}")),
            ("content-encoding", "aes128gcm".to_string()),
            ("content-type", "application/octet-stream".to_string()),
            ("ttl", ttl.to_string()),
            ("urgency", "normal".to_string()),
        ];
        self.post(&target, &headers, &body)
    }

    fn post(&self, to: &Target, headers: &[(&str, String)], body: &[u8]) -> Result<u16, String> {
        let addrs: Vec<_> = (to.host.as_str(), to.port)
            .to_socket_addrs()
            .map_err(|e| format!("`{}` does not resolve: {e}", to.host))?
            .collect();
        let tcp = addrs
            .iter()
            .find_map(|a| TcpStream::connect_timeout(a, PATIENCE).ok())
            .ok_or_else(|| format!("`{}:{}` could not be reached", to.host, to.port))?;
        tcp.set_read_timeout(Some(PATIENCE))
            .map_err(|e| e.to_string())?;
        tcp.set_write_timeout(Some(PATIENCE))
            .map_err(|e| e.to_string())?;
        let name = rustls::pki_types::ServerName::try_from(to.host.clone())
            .map_err(|_| format!("`{}` is not a name TLS can check", to.host))?;
        let conn = rustls::ClientConnection::new(self.tls.clone(), name)
            .map_err(|e| format!("TLS to `{}`: {e}", to.host))?;
        let mut tls = rustls::StreamOwned::new(conn, tcp);
        let host = if to.port == 443 {
            to.host.clone()
        } else {
            format!("{}:{}", to.host, to.port)
        };
        let mut out = format!(
            "POST {} HTTP/1.1\r\nhost: {host}\r\nconnection: close\r\n",
            to.path
        );
        for (n, v) in headers {
            out.push_str(&format!("{n}: {v}\r\n"));
        }
        out.push_str(&format!("content-length: {}\r\n\r\n", body.len()));
        let mut bytes = out.into_bytes();
        bytes.extend_from_slice(body);
        tls.write_all(&bytes)
            .map_err(|e| format!("sending to `{}`: {e}", to.host))?;
        let mut reply = Vec::new();
        let mut buf = [0u8; 4096];
        loop {
            match tls.read(&mut buf) {
                Ok(0) => break,
                Ok(n) => {
                    reply.extend_from_slice(&buf[..n]);
                    if reply.len() > 64 << 10 {
                        break;
                    }
                }
                Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => break,
                Err(e) => return Err(format!("reading from `{}`: {e}", to.host)),
            }
        }
        let line = reply
            .split(|b| *b == b'\n')
            .next()
            .map(|l| String::from_utf8_lossy(l).to_string())
            .unwrap_or_default();
        line.split_whitespace()
            .nth(1)
            .and_then(|s| s.parse::<u16>().ok())
            .ok_or_else(|| format!("no status from `{}`: {line:?}", to.host))
    }
}
