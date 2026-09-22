//! §9.1's three-way bind: which identity the raw key a handshake presented
//! speaks as.  The handshake admits any well-formed Ed25519 key
//! (`tls::AnyRawKeyServer`, `tls::AnyRawKeyClient`); this decides, once the
//! key is known, whether it is the classical member pinned for the keyhash
//! the caller meant, a transport key that keyhash delegated and this holder
//! already keeps from the topology class (`wire-format.md` §10.1), or one
//! the peer's own delegation, presented first on the connection, names
//! (`wire-format.md` §8.2).  A connection on which none holds is refused,
//! session or not.
//!
//! What a delegation is checked for, in the order the wire states it: it
//! decodes (the codec refuses a window that is not exactly 172,800 seconds
//! and a classical-only signature as malformed), it verifies hybrid under
//! the identity pinned for its field 2, its field 2 is the keyhash sought,
//! its field 1 is the key the handshake presented, and its window contains
//! this receiver's clock within its leeway.  A verified delegation is
//! cached against its transport key, since it cannot change for the run.

use crate::tls::{Clock, Credential, Pins, system_clock};
use rhtn_crypto::verify::{self, Delegation, Failure};
use std::collections::HashMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

/// The receiver's default leeway on a delegation's window, in seconds
/// [author, 2026-09-21].  Configurable per receiver; a link measured in
/// seconds of latency, HF radio being the example, relaxes it.
pub const DEFAULT_LEEWAY_SECONDS: u64 = 10;

/// Delegations this holder keeps from the topology class: one per
/// delegating keyhash, the newest by `not_before` (`wire-format.md` §8.2).
/// A node's topology store answers this; a light client's horizon view
/// does; a harness answers from a map.
pub trait Held: Send + Sync {
    fn delegation(&self, keyhash: &[u8; 32]) -> Option<Delegation>;
    /// The held delegation naming transport key `key`, for a peer known
    /// by the key it presented and nothing else: the direct path.  None
    /// where the holder cannot answer by key.
    fn by_key(&self, _key: &[u8; 32]) -> Option<Delegation> {
        None
    }
}

/// Nothing held: every bind is by the pin or by the peer's own frame.
#[derive(Default)]
pub struct NoneHeld;

impl Held for NoneHeld {
    fn delegation(&self, _: &[u8; 32]) -> Option<Delegation> {
        None
    }
}

/// A holder over a map, keeping the newest per keyhash: what a harness
/// or a test stands up, and the shape a store implements.
#[derive(Default)]
pub struct HeldMap(Mutex<HashMap<[u8; 32], Delegation>>);

impl HeldMap {
    /// Keep `d` unless one at least as new is held for its keyhash.
    /// Returns whether it was kept.
    pub fn keep(&self, d: Delegation) -> bool {
        let mut m = self.0.lock().unwrap();
        match m.get(&d.keyhash) {
            Some(have) if have.not_before >= d.not_before => false,
            _ => {
                m.insert(d.keyhash, d);
                true
            }
        }
    }
    pub fn len(&self) -> usize {
        self.0.lock().unwrap().len()
    }
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

impl Held for HeldMap {
    fn delegation(&self, keyhash: &[u8; 32]) -> Option<Delegation> {
        self.0.lock().unwrap().get(keyhash).cloned()
    }
    fn by_key(&self, key: &[u8; 32]) -> Option<Delegation> {
        self.0
            .lock()
            .unwrap()
            .values()
            .find(|d| d.key == *key)
            .cloned()
    }
}

/// How a presented key was bound to the keyhash meant.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Bound {
    /// The pinned classical member itself.
    Pinned,
    /// A delegation already held from the topology class.
    Held,
    /// The delegation the peer presented on this connection.
    Presented,
}

/// Why a presented key bound to nothing.  Each is a refusal of the
/// connection; `Window` is the one the dialler retries, since the peer may
/// have rolled to its next credential between the two checks (§8.2).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Refusal {
    /// The delegation does not decode: the codec's shape, its exact
    /// window, or a classical-only signature.
    Malformed(String),
    /// No identity is pinned for the delegation's field 2, so nothing can
    /// verify it.
    Unverifiable,
    /// The hybrid signature does not verify under the identity field 2 names.
    BadSignature,
    /// Field 1 is not the key the handshake presented: a delegation
    /// captured from another connection.
    WrongKey,
    /// Field 2 is not the keyhash this side meant to reach.
    WrongKeyhash,
    /// The window does not contain this receiver's clock within its leeway.
    Window,
    /// Nothing binds the key: not pinned, nothing held, nothing presented.
    Unbound,
}

impl std::fmt::Display for Refusal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Refusal::Malformed(s) => write!(f, "malformed delegation: {s}"),
            Refusal::Unverifiable => write!(f, "delegation by an identity not pinned"),
            Refusal::BadSignature => write!(f, "delegation does not verify"),
            Refusal::WrongKey => write!(f, "delegation names a key the handshake did not present"),
            Refusal::WrongKeyhash => {
                write!(f, "delegation by another identity than the one sought")
            }
            Refusal::Window => write!(f, "delegation outside its window"),
            Refusal::Unbound => write!(f, "presented key bound by nothing"),
        }
    }
}

/// Verified delegations by transport key: the raw bytes' digest, and
/// what they said.
type Cache = HashMap<[u8; 32], ([u8; 32], Delegation)>;

/// What a receiver holds for binding: its pins, what it keeps from the
/// topology class, its leeway and clock, the cache of what it has
/// verified, and its own credential where it is a delegated peer.  One per
/// node or client; cloned into every connection.
#[derive(Clone)]
pub struct Binding {
    pub held: Arc<dyn Held>,
    pub leeway_secs: u64,
    pub clock: Clock,
    /// This side's own delegated credential, presented first on every
    /// connection; none for a peer presenting its own classical member.
    pub credential: Option<Arc<Credential>>,
    /// Verified delegations by transport key: the raw bytes' digest and
    /// what they said.  Bounded by the keys this side has met in a run.
    cache: Arc<Mutex<Cache>>,
    /// How many hybrid verifications this binding has done, for a test
    /// that counts them (TRN-26).
    pub verified: Arc<AtomicUsize>,
}

impl Default for Binding {
    fn default() -> Self {
        Binding {
            held: Arc::new(NoneHeld),
            leeway_secs: DEFAULT_LEEWAY_SECONDS,
            clock: system_clock(),
            credential: None,
            cache: Arc::default(),
            verified: Arc::default(),
        }
    }
}

impl Binding {
    pub fn with_held(mut self, held: Arc<dyn Held>) -> Self {
        self.held = held;
        self
    }
    pub fn with_leeway(mut self, secs: u64) -> Self {
        self.leeway_secs = secs;
        self
    }
    pub fn with_clock(mut self, clock: Clock) -> Self {
        self.clock = clock;
        self
    }
    pub fn with_credential(mut self, c: Arc<Credential>) -> Self {
        self.credential = Some(c);
        self
    }

    /// The delegation this side presents now, where it is delegated.
    pub fn own_delegation(&self) -> Option<Vec<u8>> {
        self.credential
            .as_ref()
            .and_then(|c| c.current())
            .map(|i| i.raw)
    }

    pub fn in_window(&self, d: &Delegation) -> bool {
        let now = (self.clock)();
        let l = self.leeway_secs;
        d.not_before.saturating_sub(l) <= now && now <= d.not_after.saturating_add(l)
    }

    /// Verify a presented delegation, from the cache where the same bytes
    /// under the same key were verified before.  Field 2 is checked
    /// against `target` where the caller sought one; on the serving side
    /// the claimed keyhash is the target.
    pub fn verify_presented(
        &self,
        pins: &Pins,
        target: Option<&[u8; 32]>,
        presented: &[u8; 32],
        raw: &[u8],
    ) -> Result<Delegation, Refusal> {
        let digest = rhtn_codec::cose::sha256(raw);
        let cached = self
            .cache
            .lock()
            .unwrap()
            .get(presented)
            .filter(|(h, _)| *h == digest)
            .map(|(_, d)| d.clone());
        let d = match cached {
            Some(d) => d,
            None => {
                let item = rhtn_codec::cbor::parse_all(raw)
                    .map_err(|_| Refusal::Malformed("not CBOR".into()))?;
                rhtn_codec::schema::check_kind(raw, "Delegation", &item)
                    .map_err(|e| Refusal::Malformed(e.0.into()))?;
                let keyhash: [u8; 32] = match &item {
                    rhtn_codec::cbor::Item::Map(m) => match rhtn_codec::cbor::map_get(m, 2) {
                        Some(rhtn_codec::cbor::Item::Bytes(r)) => raw[r.clone()]
                            .try_into()
                            .map_err(|_| Refusal::Malformed("field 2".into()))?,
                        _ => return Err(Refusal::Malformed("field 2".into())),
                    },
                    _ => return Err(Refusal::Malformed("not a map".into())),
                };
                let id = pins.identity(&keyhash).ok_or(Refusal::Unverifiable)?;
                self.verified.fetch_add(1, Ordering::Relaxed);
                let d =
                    verify::delegation(std::slice::from_ref(&id), raw).map_err(|e| match e {
                        Failure::MissingKey(_) | Failure::MissingDelegation(_) => {
                            Refusal::Unverifiable
                        }
                        Failure::Invalid(s) if s.starts_with("delegation:") => {
                            Refusal::BadSignature
                        }
                        Failure::Invalid(s) => Refusal::Malformed(s),
                    })?;
                // cached only once it is known to name the presented key:
                // a captured delegation under another key is not worth
                // remembering
                if d.key == *presented {
                    self.cache
                        .lock()
                        .unwrap()
                        .insert(*presented, (digest, d.clone()));
                }
                d
            }
        };
        if d.key != *presented {
            return Err(Refusal::WrongKey);
        }
        if let Some(t) = target
            && d.keyhash != *t
        {
            return Err(Refusal::WrongKeyhash);
        }
        if !self.in_window(&d) {
            return Err(Refusal::Window);
        }
        Ok(d)
    }

    /// Bind `presented` to `target` by what this side holds, with no frame:
    /// the pin, or a held delegation in window.  `Err(Window)` is a held
    /// delegation out of window, which the peer's own frame may still cure.
    pub fn without_frame(
        &self,
        pins: &Pins,
        target: &[u8; 32],
        presented: &[u8; 32],
    ) -> Result<Option<Bound>, Refusal> {
        if pins.keyhash_for_key(presented) == Some(*target) {
            return Ok(Some(Bound::Pinned));
        }
        match self.held.delegation(target) {
            Some(d) if d.key == *presented => {
                return if self.in_window(&d) {
                    Ok(Some(Bound::Held))
                } else {
                    Err(Refusal::Window)
                };
            }
            _ => {}
        }
        // a delegation this side verified on an earlier connection under
        // the same key: the cache is what makes the second connection
        // wait for no frame (§8.2)
        let cached = self
            .cache
            .lock()
            .unwrap()
            .get(presented)
            .map(|(_, d)| d.clone());
        match cached {
            Some(d) if d.keyhash == *target && self.in_window(&d) => Ok(Some(Bound::Presented)),
            _ => Ok(None),
        }
    }

    /// The dialling side's whole bind: what it holds, then the delegation
    /// the peer presented (`AttachAck` field 6, or control frame 7), or a
    /// refusal.
    pub fn dialler(
        &self,
        pins: &Pins,
        target: &[u8; 32],
        presented: &[u8; 32],
        presented_delegation: Option<&[u8]>,
    ) -> Result<Bound, Refusal> {
        match self.without_frame(pins, target, presented) {
            Ok(Some(b)) => return Ok(b),
            Ok(None) => {}
            Err(r) if presented_delegation.is_none() => return Err(r),
            Err(_) => {}
        }
        match presented_delegation {
            Some(raw) => self
                .verify_presented(pins, Some(target), presented, raw)
                .map(|_| Bound::Presented),
            None => Err(Refusal::Unbound),
        }
    }

    /// The serving side's bind of a client: the keyhash the presented key
    /// speaks as.  With `claimed` (Attach field 1) the pin must name it or
    /// the delegation must be by it; without (a delegation frame on a
    /// connection that opens no session) the delegation's own field 2 is
    /// the answer.
    pub fn server(
        &self,
        pins: &Pins,
        claimed: Option<&[u8; 32]>,
        presented: &[u8; 32],
        presented_delegation: Option<&[u8]>,
    ) -> Result<([u8; 32], Bound), Refusal> {
        if let Some(kh) = pins.keyhash_for_key(presented) {
            return match claimed {
                Some(c) if *c != kh => Err(Refusal::WrongKeyhash),
                _ => Ok((kh, Bound::Pinned)),
            };
        }
        if let Some(raw) = presented_delegation {
            let d = self.verify_presented(pins, claimed, presented, raw)?;
            return Ok((d.keyhash, Bound::Presented));
        }
        if let Some(c) = claimed {
            match self.held.delegation(c) {
                Some(d) if d.key == *presented => {
                    return if self.in_window(&d) {
                        Ok((*c, Bound::Held))
                    } else {
                        Err(Refusal::Window)
                    };
                }
                _ => {}
            }
        }
        Err(Refusal::Unbound)
    }

    /// Whether this side has a verified delegation cached under `key`.
    pub fn cached(&self, key: &[u8; 32]) -> bool {
        self.cache.lock().unwrap().contains_key(key)
    }
}
