//! The stream-0 session (§8): attach and acknowledgement, heartbeats and the
//! three-missed-intervals liveness rule, the sibling cache, refusal as a
//! close code, and what a receiver does with unknown, malformed and
//! over-bound control frames.  Delivery of queued material rides
//! unidirectional streams (§9.2).
//!
//! Both roles share one control loop.  Every control frame a role sends
//! passes through an optional outbound filter, which is how a test stands
//! on the path: dropping a beat, replacing one with a malformed frame, or
//! blackholing a direction.

use crate::bind::{Binding, Bound, Refusal};
use crate::queue::{self, QueueStore, Queued};
use crate::tls::{self, Party, Pins, Presenter};
use quinn::{Connection, RecvStream, SendStream, VarInt};
use rhtn_archive::topology::Supersession;
use rhtn_codec::bounds;
use rhtn_codec::cbor::*;
use rhtn_codec::encode::*;
use rhtn_codec::frame::{self, Stream};
use rhtn_codec::schema::Family;
use rhtn_crypto::SigningIdentity;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::sync::{Arc, Mutex};
use tokio::sync::mpsc;
use tokio::time::{Duration, Instant, sleep_until};

/// Application close code for a refused attach (§8.2).
pub const CLOSE_REFUSED: u32 = 1;
pub const FRAME_ATTACH: u64 = 1;
pub const FRAME_ATTACH_ACK: u64 = 2;
/// A delegated peer's first frame on a connection that opens no session
/// (`wire-format.md` §8.0, §8.2).
pub const FRAME_DELEGATION: u64 = 7;
pub const FRAME_HEARTBEAT: u64 = 3;
pub const FRAME_SIBLING_UPDATE: u64 = 4;

// ------------------------------------------------------------ capabilities

/// `capability_id = first 8 bytes of SHA-256("rhtn/cap:" || name)`, big-endian (§8.1).
pub fn capability_id(name: &str) -> u64 {
    let h = rhtn_codec::cose::sha256(&[b"rhtn/cap:", name.as_bytes()].concat());
    u64::from_be_bytes(h[..8].try_into().unwrap())
}

fn secure_random(buf: &mut [u8]) {
    rustls::crypto::aws_lc_rs::default_provider()
        .secure_random
        .fill(buf)
        .expect("os randomness");
}

/// One greased parameter: a random id avoiding `known`, and 8 random bytes (§8.1.1).
pub fn grease(known: &[u64]) -> (u64, Vec<u8>) {
    loop {
        let mut id = [0u8; 8];
        secure_random(&mut id);
        let id = u64::from_be_bytes(id);
        if !known.contains(&id) {
            let mut v = vec![0u8; 8];
            secure_random(&mut v);
            return (id, v);
        }
    }
}

/// Encode `{ * uint => bstr }`, keys ascending; uint keys sort the same
/// numerically and bytewise.
pub fn encode_capabilities(caps: &BTreeMap<u64, Vec<u8>>) -> Vec<u8> {
    let mut out = Vec::new();
    emit_map_head(&mut out, caps.len());
    for (k, v) in caps {
        emit_uint(&mut out, *k);
        emit_bstr(&mut out, v);
    }
    out
}

pub fn decode_capabilities(b: &[u8], item: &Item) -> BTreeMap<u64, Vec<u8>> {
    let mut out = BTreeMap::new();
    if let Item::Map(m) = item {
        for (k, v) in m {
            if let (Item::Uint(id), Item::Bytes(r)) = (k, v) {
                out.insert(*id, b[r.clone()].to_vec());
            }
        }
    }
    out
}

// ------------------------------------------------------------ frames

/// `u32-be length || CBOR [type, body]` (§8.0).
pub fn control_frame(frame_type: u64, body: &[u8]) -> Vec<u8> {
    let mut payload = Vec::new();
    emit_array_head(&mut payload, 2);
    emit_uint(&mut payload, frame_type);
    payload.extend_from_slice(body);
    let mut out = (payload.len() as u32).to_be_bytes().to_vec();
    out.extend_from_slice(&payload);
    out
}

#[derive(Debug)]
pub enum FrameRead {
    /// The CBOR payload behind an in-bound length prefix.
    Payload(Vec<u8>),
    /// A declared length above the stream's bound: the session ends (§8.0).
    OverBound(u32),
    /// The stream or connection ended.
    Closed(Option<quinn::ConnectionError>),
}

/// Read one framed payload from a stream, honouring the bound before reading
/// the bytes it declares.
pub async fn read_frame(recv: &mut RecvStream, bound: usize) -> FrameRead {
    let mut head = [0u8; 4];
    if let Err(e) = recv.read_exact(&mut head).await {
        return FrameRead::Closed(read_error_connection(e));
    }
    let n = u32::from_be_bytes(head);
    if n as usize > bound {
        return FrameRead::OverBound(n);
    }
    let mut buf = vec![0u8; n as usize];
    if let Err(e) = recv.read_exact(&mut buf).await {
        return FrameRead::Closed(read_error_connection(e));
    }
    FrameRead::Payload(buf)
}

fn read_error_connection(e: quinn::ReadExactError) -> Option<quinn::ConnectionError> {
    match e {
        quinn::ReadExactError::ReadError(quinn::ReadError::ConnectionLost(c)) => Some(c),
        _ => None,
    }
}

/// A reader of length-prefixed frames that survives cancellation: bytes
/// arrive through the cancellation-safe chunk read into a buffer the
/// reader keeps, and a frame is returned only once whole, so a timer or an
/// outbound frame winning a select between a length prefix and its payload
/// loses nothing (§8.0, §9.2).  The one-shot [`read_frame`] serves where
/// nothing races the read.
pub struct FrameReader {
    recv: RecvStream,
    buf: Vec<u8>,
}

impl FrameReader {
    pub fn new(recv: RecvStream) -> Self {
        FrameReader {
            recv,
            buf: Vec::new(),
        }
    }

    /// The next whole frame's payload, or how the stream ended.  Dropping
    /// the future between polls loses nothing the reader has taken.
    pub async fn next(&mut self, bound: usize) -> FrameRead {
        loop {
            if self.buf.len() >= 4 {
                let n = u32::from_be_bytes([self.buf[0], self.buf[1], self.buf[2], self.buf[3]]);
                if n as usize > bound {
                    return FrameRead::OverBound(n);
                }
                if self.buf.len() >= 4 + n as usize {
                    let payload = self.buf[4..4 + n as usize].to_vec();
                    self.buf.drain(..4 + n as usize);
                    return FrameRead::Payload(payload);
                }
            }
            let mut chunk = [0u8; 4096];
            match self.recv.read(&mut chunk).await {
                Ok(Some(k)) => self.buf.extend_from_slice(&chunk[..k]),
                Ok(None) => return FrameRead::Closed(None),
                Err(quinn::ReadError::ConnectionLost(c)) => return FrameRead::Closed(Some(c)),
                Err(_) => return FrameRead::Closed(None),
            }
        }
    }
}

/// What a received control payload turned out to be.
pub enum Control {
    /// A known frame: its family, the whole payload the item's ranges
    /// index, the body's range within it, and the body item.
    Known(Family, Vec<u8>, std::ops::Range<usize>, Item),
    /// Unknown type: skipped, session survives (§8.0).
    Unknown(u64),
    /// Malformed body of a known type: discarded whole, session survives (§8.0).
    Malformed,
}

pub fn classify(payload: &[u8]) -> Control {
    match frame::parse_payload(Stream::Control, payload) {
        Ok(f) => match f.family {
            Some(fam) => Control::Known(fam, payload.to_vec(), f.body, f.body_item),
            None => Control::Unknown(f.frame_type),
        },
        Err(_) => Control::Malformed,
    }
}

// ------------------------------------------------------------ messages

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NetworkPoint {
    pub ip: [u8; 4],
    pub asn: Option<u64>,
    pub port: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SiblingRef {
    pub keyhash: [u8; 32],
    pub endpoints: Vec<NetworkPoint>,
    pub key_material: Option<Vec<u8>>,
}

/// The default UDP port (§4.4, §9.2): absent on the wire, since writing it
/// out is malformed, so a point is normalised to carry no port for it.
pub const DEFAULT_PORT: u64 = 7431;

impl NetworkPoint {
    pub fn new(ip: [u8; 4], port: Option<u64>) -> Self {
        NetworkPoint {
            ip,
            asn: None,
            port: port.filter(|p| *p != DEFAULT_PORT),
        }
    }
    pub fn with_asn(mut self, asn: u64) -> Self {
        self.asn = Some(asn);
        self
    }
    /// The socket address a dialler uses; the default port is 7431 (§4.4).
    pub fn socket(&self) -> std::net::SocketAddr {
        std::net::SocketAddr::from((self.ip, self.port.unwrap_or(DEFAULT_PORT) as u16))
    }
    /// A point for a loopback socket address, as a test on one host makes.
    pub fn from_socket(addr: std::net::SocketAddr) -> Option<Self> {
        match addr.ip() {
            std::net::IpAddr::V4(v4) => {
                Some(NetworkPoint::new(v4.octets(), Some(addr.port() as u64)))
            }
            _ => None,
        }
    }
    pub fn encode_bytes(&self) -> Vec<u8> {
        let mut out = Vec::new();
        self.encode(&mut out);
        out
    }
    pub fn decode_bytes(b: &[u8]) -> Result<Self, String> {
        let item = parse_all(b).map_err(|e| e.0)?;
        Self::decode(b, &item).ok_or_else(|| "network point".to_string())
    }
    pub fn encode(&self, out: &mut Vec<u8>) {
        emit_map_head(
            out,
            1 + self.asn.is_some() as usize + self.port.is_some_and(|p| p != DEFAULT_PORT) as usize,
        );
        emit_uint(out, 1);
        emit_bstr(out, &self.ip);
        if let Some(a) = self.asn {
            emit_uint(out, 2);
            emit_uint(out, a);
        }
        // the default port is never written out (§4.4)
        if let Some(p) = self.port.filter(|p| *p != DEFAULT_PORT) {
            emit_uint(out, 3);
            emit_uint(out, p);
        }
    }
    pub fn decode(b: &[u8], it: &Item) -> Option<Self> {
        let Item::Map(m) = it else { return None };
        let ip = match map_get(m, 1) {
            Some(Item::Bytes(r)) => b[r.clone()].try_into().ok()?,
            _ => return None,
        };
        Some(NetworkPoint {
            ip,
            asn: map_get(m, 2).and_then(as_uint),
            port: map_get(m, 3).and_then(as_uint),
        })
    }
}

impl SiblingRef {
    pub fn encode(&self, out: &mut Vec<u8>) {
        emit_map_head(out, 2 + self.key_material.is_some() as usize);
        emit_uint(out, 1);
        emit_bstr(out, &self.keyhash);
        emit_uint(out, 2);
        emit_array_head(out, self.endpoints.len());
        for e in &self.endpoints {
            e.encode(out);
        }
        if let Some(km) = &self.key_material {
            emit_uint(out, 3);
            out.extend_from_slice(km);
        }
    }
    pub fn decode(b: &[u8], it: &Item) -> Option<Self> {
        let Item::Map(m) = it else { return None };
        let keyhash = match map_get(m, 1) {
            Some(Item::Bytes(r)) => b[r.clone()].try_into().ok()?,
            _ => return None,
        };
        let Some(Item::Array(eps)) = map_get(m, 2) else {
            return None;
        };
        let endpoints = eps
            .iter()
            .map(|e| NetworkPoint::decode(b, e))
            .collect::<Option<Vec<_>>>()?;
        let key_material = map_get(m, 3).map(|km| reencode(km, b));
        Some(SiblingRef {
            keyhash,
            endpoints,
            key_material,
        })
    }
}

fn encode_sibling_list(out: &mut Vec<u8>, key: u64, refs: &[SiblingRef]) {
    if refs.is_empty() {
        return;
    }
    emit_uint(out, key);
    emit_array_head(out, refs.len());
    for r in refs {
        r.encode(out);
    }
}

fn decode_sibling_list(b: &[u8], it: Option<&Item>) -> Option<Vec<SiblingRef>> {
    match it {
        None => Some(Vec::new()),
        Some(Item::Array(a)) => a.iter().map(|s| SiblingRef::decode(b, s)).collect(),
        _ => None,
    }
}

#[derive(Debug, Clone)]
pub struct AttachAck {
    pub mode: u64,
    pub siblings: Vec<SiblingRef>,
    pub interval: u64,
    pub queued: u64,
    pub capabilities: BTreeMap<u64, Vec<u8>>,
    /// Field 6: the serving node's own delegation, present whenever the
    /// key it presented is a delegated one (`wire-format.md` §8.2).
    pub delegation: Option<Vec<u8>>,
}

impl AttachAck {
    pub fn encode(&self) -> Vec<u8> {
        let mut out = Vec::new();
        emit_map_head(
            &mut out,
            4 + !self.siblings.is_empty() as usize + self.delegation.is_some() as usize,
        );
        emit_uint(&mut out, 1);
        emit_uint(&mut out, self.mode);
        encode_sibling_list(&mut out, 2, &self.siblings);
        emit_uint(&mut out, 3);
        emit_uint(&mut out, self.interval);
        emit_uint(&mut out, 4);
        emit_uint(&mut out, self.queued);
        emit_uint(&mut out, 5);
        out.extend_from_slice(&encode_capabilities(&self.capabilities));
        if let Some(d) = &self.delegation {
            emit_uint(&mut out, 6);
            out.extend_from_slice(d);
        }
        out
    }
    /// Decode the map at `at` in `b`, whose parsed item is `it`: the
    /// body's offset within a frame payload, or 0 for the body alone.
    pub fn decode(b: &[u8], at: usize, it: &Item) -> Option<Self> {
        let Item::Map(m) = it else { return None };
        Some(AttachAck {
            mode: map_get(m, 1).and_then(as_uint)?,
            siblings: decode_sibling_list(b, map_get(m, 2))?,
            interval: map_get(m, 3).and_then(as_uint)?,
            queued: map_get(m, 4).and_then(as_uint)?,
            capabilities: decode_capabilities(b, map_get(m, 5)?),
            delegation: field_bytes(b, at, 6),
        })
    }
}

/// The encoded value of integer `key` in the map at `at` in `b`, whole:
/// how a nested signed object such as a delegation is carried through
/// untouched.
fn field_bytes(b: &[u8], at: usize, key: u64) -> Option<Vec<u8>> {
    value_slice_at(b, at, key).map(|r| b[r].to_vec())
}

/// An `Attach` (`wire-format.md` §8.2): field 4 carries the client's own
/// delegation where the key it presented is a delegated one.
pub fn encode_attach(
    keyhash: &[u8; 32],
    attestation: Option<&[u8]>,
    caps: &BTreeMap<u64, Vec<u8>>,
    delegation: Option<&[u8]>,
) -> Vec<u8> {
    let mut out = Vec::new();
    emit_map_head(
        &mut out,
        2 + attestation.is_some() as usize + delegation.is_some() as usize,
    );
    emit_uint(&mut out, 1);
    emit_bstr(&mut out, keyhash);
    if let Some(a) = attestation {
        emit_uint(&mut out, 2);
        out.extend_from_slice(a);
    }
    emit_uint(&mut out, 3);
    out.extend_from_slice(&encode_capabilities(caps));
    if let Some(d) = delegation {
        emit_uint(&mut out, 4);
        out.extend_from_slice(d);
    }
    out
}

pub fn encode_heartbeat(counter: u64, timestamp: u64) -> Vec<u8> {
    let mut out = Vec::new();
    emit_map_head(&mut out, 2);
    emit_uint(&mut out, 1);
    emit_uint(&mut out, counter);
    emit_uint(&mut out, 2);
    emit_uint(&mut out, timestamp);
    out
}

pub fn encode_sibling_update(refs: &[SiblingRef]) -> Vec<u8> {
    let mut out = Vec::new();
    emit_map_head(&mut out, !refs.is_empty() as usize);
    encode_sibling_list(&mut out, 1, refs);
    out
}

fn unix_now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

// ------------------------------------------------------------ shared machinery

/// A test's hand on the path: given a control frame about to be sent (its
/// type and complete bytes), return the bytes to send instead, or `None` to
/// drop it.
pub type OutboundFilter = Arc<dyn Fn(u64, &[u8]) -> Option<Vec<u8>> + Send + Sync>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    Sent {
        frame_type: u64,
        bytes: Vec<u8>,
    },
    Received {
        frame_type: u64,
    },
    Skipped {
        frame_type: u64,
    },
    Discarded,
    OverBound,
    Attached {
        mode: u64,
    },
    Refused,
    PeerUnreachable,
    Failover {
        to: [u8; 32],
    },
    Delivered {
        bytes: Vec<u8>,
    },
    /// The Attach went as 0-RTT early data on this connection.
    EarlyDataSent,
    /// A peer opened the direct payload path on this connection.
    DirectOpened,
    /// This connection's own handshake completed; `early_accepted` says
    /// whether the server took the early data.
    HandshakeDone {
        early_accepted: bool,
    },
    Closed,
    /// An attach under a credential this node has verified superseded: no
    /// AttachAck, nothing delivered (design §12.6.5).
    Superseded,
    /// The peer's presented key was bound to the keyhash meant, this way
    /// (`wire-format.md` §9.1).
    Bound {
        how: Bound,
    },
    /// Nothing bound the presented key; the connection was refused.
    Unbound {
        why: Refusal,
    },
    /// This side presented its own delegation first on the connection
    /// (`wire-format.md` §8.2).
    DelegationPresented,
    /// A request-only connection, served with no session (§8.2).
    RequestOnly,
}

/// Session events, for a test that reads what a session did.  Off by
/// default: a running node keeps the reachability state its sessions
/// settle and nothing of what passed on them (`infra-client-requirements.md`
/// §1, §2: process liveness updates and discard them; do not log queue
/// events), so a default log retains nothing and grows by nothing.  A test
/// switches recording on through its configuration.
#[derive(Default, Clone)]
pub struct Log {
    events: Arc<Mutex<Vec<(Instant, Event)>>>,
    recording: bool,
}

impl Log {
    /// A log that keeps every event: the test facility.
    pub fn recording() -> Self {
        Log {
            events: Arc::default(),
            recording: true,
        }
    }
    /// A fresh log with this one's setting: each session records into its
    /// own, or into nothing.
    pub fn fresh(&self) -> Self {
        if self.recording {
            Log::recording()
        } else {
            Log::default()
        }
    }
    pub fn is_recording(&self) -> bool {
        self.recording
    }
    pub fn push(&self, e: Event) {
        if self.recording {
            self.events.lock().unwrap().push((Instant::now(), e));
        }
    }
    pub fn events(&self) -> Vec<(Instant, Event)> {
        self.events.lock().unwrap().clone()
    }
    pub fn count(&self, f: impl Fn(&Event) -> bool) -> usize {
        self.events
            .lock()
            .unwrap()
            .iter()
            .filter(|(_, e)| f(e))
            .count()
    }
}

struct Sender {
    send: SendStream,
    filter: Option<OutboundFilter>,
    log: Log,
}

impl Sender {
    fn detached(send: SendStream) -> Self {
        Sender {
            send,
            filter: None,
            log: Log::default(),
        }
    }

    async fn frame(&mut self, frame_type: u64, body: &[u8]) -> Result<(), quinn::WriteError> {
        let bytes = control_frame(frame_type, body);
        let bytes = match &self.filter {
            Some(f) => match f(frame_type, &bytes) {
                Some(b) => b,
                None => return Ok(()),
            },
            None => bytes,
        };
        self.log.push(Event::Sent {
            frame_type,
            bytes: bytes.clone(),
        });
        self.send.write_all(&bytes).await
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Reachability {
    Reachable,
    Unreachable,
}

/// What a control loop runs with: the interval, where it logs, the
/// reachability it settles, whom it tells, and the frames it is handed to
/// send.
struct LoopIo {
    interval: Duration,
    log: Log,
    reach: Arc<Mutex<Reachability>>,
    on_change: Option<Arc<dyn Fn(Reachability) + Send + Sync>>,
    outbound: mpsc::UnboundedReceiver<(u64, Vec<u8>)>,
}

/// The heartbeat and liveness loop both roles run after the ack (§8.2).
/// Returns when the stream or connection ends.  `on_frame` sees every
/// known non-heartbeat frame.
async fn control_loop(
    mut sender: Sender,
    recv: RecvStream,
    io: LoopIo,
    mut on_frame: impl FnMut(Family, &[u8], std::ops::Range<usize>, &Item) -> bool,
) -> Option<quinn::ConnectionError> {
    let LoopIo {
        interval,
        log,
        reach,
        on_change,
        mut outbound,
    } = io;
    // the reader outlives every select below, so a frame half-read when
    // another branch wins is finished on the next turn
    let mut reader = FrameReader::new(recv);
    let start = Instant::now();
    let mut next_send = start + interval;
    let mut counter: u64 = 0;
    let mut last_valid = start;
    let mut seen: HashSet<u64> = HashSet::new();
    loop {
        // misses are elapsed full intervals on the local monotonic clock
        let deadline = last_valid + interval * 3;
        if Instant::now() >= deadline {
            let mut r = reach.lock().unwrap();
            if *r != Reachability::Unreachable {
                *r = Reachability::Unreachable;
                log.push(Event::PeerUnreachable);
                drop(r);
                if let Some(f) = &on_change {
                    f(Reachability::Unreachable);
                }
            }
        }
        tokio::select! {
            _ = sleep_until(next_send) => {
                if counter == u64::MAX { return None; }
                if sender.frame(FRAME_HEARTBEAT, &encode_heartbeat(counter, unix_now())).await.is_err() { return None; }
                counter += 1;
                next_send += interval;
            }
            _ = sleep_until(deadline), if deadline > Instant::now() => {}
            out = outbound.recv() => {
                if let Some((ft, body)) = out
                    && sender.frame(ft, &body).await.is_err() { return None; }
            }
            r = reader.next(bounds::CONTROL_FRAME_BYTES) => match r {
                FrameRead::Closed(e) => return e,
                FrameRead::OverBound(_) => { log.push(Event::OverBound); return None; }
                FrameRead::Payload(p) => match classify(&p) {
                    Control::Unknown(t) => log.push(Event::Skipped { frame_type: t }),
                    Control::Malformed => log.push(Event::Discarded),
                    Control::Known(Family::Heartbeat, _, _, item) => {
                        log.push(Event::Received { frame_type: FRAME_HEARTBEAT });
                        if let Item::Map(m) = &item
                            && let Some(c) = map_get(m, 1).and_then(as_uint)
                                && seen.insert(c) {
                                    last_valid = Instant::now();
                                    let was = std::mem::replace(&mut *reach.lock().unwrap(), Reachability::Reachable);
                                    if was == Reachability::Unreachable
                                        && let Some(f) = &on_change {
                                            f(Reachability::Reachable);
                                        }
                                }
                    }
                    Control::Known(fam, bytes, body, item) => {
                        let t = match fam { Family::Attach => 1, Family::AttachAck => 2, Family::SiblingUpdate => 4, Family::TopologyPush => 5, Family::TopologyMemo => 6, _ => 0 };
                        log.push(Event::Received { frame_type: t });
                        if !on_frame(fam, &bytes, body, &item) { return None; }
                    }
                }
            }
        }
    }
}

// ------------------------------------------------------------ the serving node

pub type Predicate = Arc<dyn Fn(&[u8; 32]) -> bool + Send + Sync>;

/// Where a client's reachability goes when the detector settles it.
pub type ReplicateHook = Arc<dyn Fn([u8; 32], Reachability) + Send + Sync>;

/// What a node does with a known control frame a session delivers beyond
/// the session's own (`wire-format.md` §8.0): the peer it came from, the
/// frame type, and the body bytes.
pub type ControlHandler = Arc<dyn Fn([u8; 32], u64, Vec<u8>) + Send + Sync>;

/// A session with a client opened (`true`) or ended (`false`).
///
/// **A node's adjacency is the sessions it holds by virtue of a topology
/// relationship** (`wire-format.md` §10.1.1), and the clients attached to
/// it are one of the four groups that rule names.  Nothing above the
/// transport can see an attach otherwise, so a node that is not told keeps
/// an empty set and forwards the flood to nobody it serves.
pub type AttachHook = Arc<dyn Fn([u8; 32], bool) + Send + Sync>;

/// What a node says its siblings are, asked at each attach.
pub type SiblingList = Arc<dyn Fn() -> Vec<SiblingRef> + Send + Sync>;
/// Payload delivered on the direct path (design §14.1.1): the
/// authenticated peer and one message.
pub type DirectHandler = Arc<dyn Fn([u8; 32], Vec<u8>) + Send + Sync>;

/// What a node answers on a request stream (`wire-format.md` §9.2): the
/// authenticated peer, the family, and the body bytes, to a reply body or
/// `None` to fail the stream.
pub type RequestHandler = Arc<
    dyn Fn(
            [u8; 32],
            Family,
            Vec<u8>,
        ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Option<Vec<u8>>> + Send>>
        + Send
        + Sync,
>;

pub struct NodeConfig {
    /// Who this node speaks as and what it presents: its identity's
    /// classical member, or the delegated credential an instance holds in
    /// place of the seed (design §23.3).
    pub me: Party,
    pub pins: Pins,
    /// §9.1's bind: what this node holds from the topology class, its
    /// leeway and clock, and its own delegated credential where it is an
    /// instance (design §23.3).
    pub bind: Binding,
    /// Seconds, 1..=3600; unset by design §21.1, chosen by the operator.
    pub interval_secs: u64,
    /// The siblings this node names to a client attaching to it
    /// (`wire-format.md` §8.2).
    ///
    /// **Asked for at each attach rather than fixed at start**, because it
    /// is a fold over a table that moves: a sibling adopted this morning
    /// is one this client can fail over to this afternoon, and a list
    /// frozen at start would be the set the node had before it had any.
    pub siblings: SiblingList,
    /// Published capabilities by id.  A greased entry is added per session.
    pub capabilities: BTreeMap<u64, Vec<u8>>,
    /// Local attach policy: false refuses with close code 1.
    pub policy: Predicate,
    /// The node's own topology: true means the client is in this node's subtree (mode 0).
    pub in_subtree: Predicate,
    pub filter: Option<OutboundFilter>,
    /// The mailbox (design §14.1.6).
    pub queue: Arc<dyn QueueStore>,
    /// Per-subordinate storage cap in bytes; unset by design §21.1, the
    /// operator's.  None is unbounded.
    pub queue_cap: Option<usize>,
    /// The node's clock, for arrival times.
    pub clock: Arc<dyn Fn() -> u64 + Send + Sync>,
    /// Whether this node holds a record of a keyhash at all.  A message for
    /// a subordinate it serves queues; one for a keyhash it has no record of
    /// is a different answer (design §14.1.2, §7.4.3).
    pub serves: Predicate,
    /// Whether a direct payload path with `keyhash` is one this node
    /// permits (design §12.6.3).  The decision is local and the same in
    /// both directions: a connection this node would not have dialled is
    /// not one it accepts, or a peer opens the path by dialling first.
    /// Everything is permitted by default, which is what a harness wants.
    pub accepts_direct: Predicate,
    /// Replicate a client's reachability to this node's siblings, so a
    /// sibling answering in failover knows the client's status
    /// (design §14.1.2).  The wire assigns no frame for this.
    pub replicate: Option<ReplicateHook>,
    /// The node's own handling of topology frames on stream 0.  Absent, a
    /// known frame beyond the session's own is logged and dropped.
    /// Told when a client attaches and when its session ends, so a node
    /// above can hold the set `wire-format.md` §10.1.1's adjacency names.
    pub on_attach: Option<AttachHook>,
    pub on_control: Option<ControlHandler>,
    /// Payload arriving on a direct connection from an authenticated peer
    /// that opens no session (design §14.1.1).  Absent, such a connection
    /// is closed.
    pub on_direct: Option<DirectHandler>,
    /// The emulated NAT this node's socket sits behind, for a harness
    /// (`rhtn-sim`); none on a real network.
    pub nat: Option<std::net::SocketAddr>,
    /// The address to serve on, which an operator chooses
    /// (`infra-client-requirements.md` §7).  Absent binds an ephemeral
    /// loopback port, which is what a harness wants and what every test
    /// here relies on.
    pub listen: Option<std::net::SocketAddr>,
    /// The node's own answers on request streams.  Absent, a currency
    /// request is answered "cannot issue" and anything else fails the stream.
    pub on_request: Option<RequestHandler>,
    /// Where session events go: nowhere by default, since a running node
    /// retains no session history; a test installs a recording log.
    pub log: Log,
}

impl NodeConfig {
    /// A configuration with an in-memory queue, no cap and the system clock.
    pub fn defaults(identity: Arc<SigningIdentity>, pins: Pins, interval_secs: u64) -> Self {
        Self::defaults_for(Party::of(identity), pins, interval_secs)
    }

    /// `defaults` for any party, a delegated one included.
    pub fn defaults_for(me: Party, pins: Pins, interval_secs: u64) -> Self {
        let clock: tls::Clock = tls::system_clock();
        let mut bind = Binding::default().with_clock(clock.clone());
        if let Some(c) = me.credential() {
            bind = bind.with_credential(c.clone());
        }
        NodeConfig {
            me,
            pins,
            bind,
            interval_secs,
            siblings: Arc::new(Vec::new),
            capabilities: BTreeMap::new(),
            policy: Arc::new(|_| true),
            in_subtree: Arc::new(|_| true),
            filter: None,
            serves: Arc::new(|_| true),
            accepts_direct: Arc::new(|_| true),
            replicate: None,
            on_attach: None,
            on_control: None,
            on_direct: None,
            nat: None,
            listen: None,
            on_request: None,
            queue: Arc::new(queue::MemoryStore::default()),
            queue_cap: None,
            clock,
            log: Log::default(),
        }
    }

    /// What this node presents in the handshake.
    pub fn presenter(&self) -> Presenter {
        self.me.presenter.clone()
    }

    /// Run under a delegated credential for the same keyhash: the key
    /// presented becomes the credential's, every AttachAck carries its
    /// delegation, and the identity's seed is needed for nothing the
    /// transport does.
    pub fn with_credential(mut self, c: Arc<tls::Credential>) -> Self {
        self.me = Party::of(c.clone());
        self.bind = self.bind.with_credential(c);
        self
    }
}

#[derive(Default)]
struct NodeState {
    /// One drain at a time per recipient.
    drains: HashMap<[u8; 32], Arc<tokio::sync::Mutex<()>>>,
    reach: HashMap<[u8; 32], Arc<Mutex<Reachability>>>,
    sessions: HashMap<[u8; 32], Connection>,
    /// Credentials this node holds supersession evidence for, and their
    /// successors (a reissue names the same key).
    superseded: HashMap<[u8; 32], [u8; 32]>,
    /// The reachability this node has settled on per client, which outlives
    /// the session it was learned in and is what replicates (design §14.1.2).
    marked: HashMap<[u8; 32], Reachability>,
    /// A handle into each live session's control stream, for frames this
    /// node originates or forwards.
    outbound: HashMap<[u8; 32], mpsc::UnboundedSender<(u64, Vec<u8>)>>,
}

pub struct Node {
    pub cfg: NodeConfig,
    state: Mutex<NodeState>,
    /// Held across a submission's cap check and its push, so two
    /// submissions cannot both see the room for one (design §14.1.6).
    queue_gate: Mutex<()>,
    pub log: Log,
}

impl Node {
    pub fn new(cfg: NodeConfig) -> Arc<Self> {
        let log = cfg.log.clone();
        Arc::new(Node {
            cfg,
            state: Mutex::new(NodeState::default()),
            queue_gate: Mutex::new(()),
            log,
        })
    }

    /// Accept material for a client: delivered now on a unidirectional
    /// stream if a session is up, otherwise queued — unless the recipient's
    /// queue is at its cap, when the newest is refused and the sender told
    /// (design §14.1.6), or the credential is one this node has verified
    /// superseded (design §12.6.5).
    pub fn enqueue(
        self: &Arc<Self>,
        keyhash: [u8; 32],
        bytes: Vec<u8>,
    ) -> Result<(), queue::Refusal> {
        if !(self.cfg.serves)(&keyhash) {
            return Err(queue::Refusal::NoRecord);
        }
        let conn = {
            let st = self.state.lock().unwrap();
            if st.superseded.get(&keyhash).is_some_and(|s| *s != keyhash) {
                return Err(queue::Refusal::Superseded);
            }
            // a client this node has marked unreachable is queued for, not
            // delivered to: the material waits for its return (design
            // §14.1.2).  A session object outlives the reachability the
            // detector settled on, and the detector is what decides.
            let live = st
                .reach
                .get(&keyhash)
                .map(|r| *r.lock().unwrap())
                .or_else(|| st.marked.get(&keyhash).copied());
            if live == Some(Reachability::Unreachable) {
                None
            } else {
                st.sessions.get(&keyhash).cloned()
            }
        };
        // the cap is read and the message stored under one gate: capacity is
        // reserved and taken as one step, so a concurrent submission sees
        // the store this one leaves
        let _gate = self.queue_gate.lock().unwrap();
        if let Some(cap) = self.cfg.queue_cap
            && self.cfg.queue.bytes(&keyhash) + bytes.len() > cap
        {
            return Err(queue::Refusal::AtCap);
        }
        // accepted means stored: the message enters the mailbox, and a live
        // session drains it from there, so nothing is reported delivered
        // before the peer has taken it
        self.cfg.queue.push(Queued {
            ciphertext: bytes,
            recipient: keyhash,
            arrival: (self.cfg.clock)(),
        });
        if let Some(conn) = conn {
            tokio::spawn(drain(self.clone(), keyhash, conn));
        }
        Ok(())
    }

    fn drain_lock(&self, recipient: &[u8; 32]) -> Arc<tokio::sync::Mutex<()>> {
        self.state
            .lock()
            .unwrap()
            .drains
            .entry(*recipient)
            .or_default()
            .clone()
    }

    pub fn queued(&self, keyhash: &[u8; 32]) -> usize {
        self.cfg.queue.count(keyhash)
    }

    /// The node's record of what waits for `keyhash`, as it holds it.
    pub fn queue_records(&self, keyhash: &[u8; 32]) -> Vec<Queued> {
        self.cfg.queue.list(keyhash)
    }

    /// Take authenticated supersession evidence for a binding
    /// (`infra-client-requirements.md` §2): its sessions end, an attach
    /// under it gets no AttachAck, and nothing queued for it is delivered
    /// to it or handed to its successor.
    pub fn supersede(&self, s: Supersession) {
        let conn = {
            let mut st = self.state.lock().unwrap();
            st.superseded.insert(s.superseded, s.successor);
            st.sessions.remove(&s.superseded)
        };
        if s.successor != s.superseded {
            self.cfg.queue.drop_all(&s.superseded);
        }
        if let Some(c) = conn {
            c.close(VarInt::from_u32(CLOSE_REFUSED), b"superseded");
        }
    }

    pub fn is_superseded(&self, keyhash: &[u8; 32]) -> bool {
        self.state
            .lock()
            .unwrap()
            .superseded
            .get(keyhash)
            .is_some_and(|s| s != keyhash)
    }

    pub fn reachability(&self, keyhash: &[u8; 32]) -> Option<Reachability> {
        let st = self.state.lock().unwrap();
        st.reach
            .get(keyhash)
            .map(|r| *r.lock().unwrap())
            .or_else(|| st.marked.get(keyhash).copied())
    }

    /// Take a sibling's replicated reachability for a client this node does
    /// not serve.  A sibling answering in failover needs the client's
    /// status and has no session of its own to learn it from.
    pub fn note_reachability(&self, keyhash: [u8; 32], r: Reachability) {
        self.state.lock().unwrap().marked.insert(keyhash, r);
    }

    /// Whether this node holds any record of `keyhash`.
    pub fn serves(&self, keyhash: &[u8; 32]) -> bool {
        (self.cfg.serves)(keyhash)
    }

    pub fn has_session(&self, keyhash: &[u8; 32]) -> bool {
        self.state.lock().unwrap().sessions.contains_key(keyhash)
    }

    /// Every peer with a live session on this node.
    pub fn sessions(&self) -> Vec<[u8; 32]> {
        self.state
            .lock()
            .unwrap()
            .sessions
            .keys()
            .copied()
            .collect()
    }

    /// Send a control frame on the session with `peer`, where one exists.
    pub fn send_control(&self, peer: &[u8; 32], frame_type: u64, body: &[u8]) -> bool {
        match self.state.lock().unwrap().outbound.get(peer) {
            Some(tx) => tx.send((frame_type, body.to_vec())).is_ok(),
            None => false,
        }
    }

    /// The address the client's session currently comes from.
    pub fn remote_address(&self, keyhash: &[u8; 32]) -> Option<std::net::SocketAddr> {
        self.state
            .lock()
            .unwrap()
            .sessions
            .get(keyhash)
            .map(|c| c.remote_address())
    }

    /// Accept connections forever.
    pub async fn serve(self: Arc<Self>, endpoint: quinn::Endpoint) {
        while let Some(incoming) = endpoint.accept().await {
            let node = self.clone();
            tokio::spawn(async move {
                let _ = node.handle(incoming).await;
            });
        }
    }

    async fn handle(self: Arc<Self>, incoming: quinn::Incoming) -> Result<(), String> {
        // The handshake completes before anything is read: an Attach in
        // 0-RTT early data is deferred to this point and never processed
        // early (§8.2, §9.2).
        let conn = incoming.await.map_err(|e| e.to_string())?;
        let presented = tls::peer_key(&conn).ok_or("no peer key")?;
        // the pinned classical member binds at once; a delegated key binds
        // at the attach, by its frame, or by a delegation already held
        let authenticated = self.cfg.pins.keyhash_for_key(&presented);
        // a delegated node presents its delegation before anything else on
        // every connection (§8.2): on a stream of its own, since a dialler
        // that must read it before sending anything has opened none
        if let Some(raw) = self.cfg.bind.own_delegation() {
            let c = conn.clone();
            let log = self.log.clone();
            tokio::spawn(async move {
                if let Ok((mut s, _)) = c.open_bi().await {
                    let _ = s.write_all(&control_frame(FRAME_DELEGATION, &raw)).await;
                    let _ = s.finish();
                    log.push(Event::DelegationPresented);
                }
            });
        }
        // a session opens stream 0 with an Attach; a direct payload path
        // (design §14.1.1) opens no session and delivers on unidirectional
        // streams alone, so whichever arrives first says which this is
        let (send, mut recv) = tokio::select! {
            bi = conn.accept_bi() => bi.map_err(|e| e.to_string())?,
            uni = conn.accept_uni() => {
                let first = uni.map_err(|e| e.to_string())?;
                // the direct path is inside the horizon, where the peer's
                // delegation is held from the topology class: the pin or a
                // held delegation binds it, with no frame (design §12.6.3)
                let peer = authenticated.or_else(|| {
                    self.cfg.bind.held.by_key(&presented)
                        .filter(|d| self.cfg.bind.in_window(d))
                        .map(|d| d.keyhash)
                });
                let (Some(peer), Some(handler)) = (peer, self.cfg.on_direct.clone()) else {
                    self.log.push(Event::Unbound { why: Refusal::Unbound });
                    conn.close(VarInt::from_u32(CLOSE_REFUSED), b"no direct path");
                    return Err("direct path from an unbound peer or with no handler".into());
                };
                if !(self.cfg.accepts_direct)(&peer) {
                    conn.close(VarInt::from_u32(CLOSE_REFUSED), b"no direct path");
                    return Err("direct path from a peer this node does not permit one with".into());
                }
                self.log.push(Event::DirectOpened);
                let mut streams = vec![first];
                loop {
                    for mut s in streams.drain(..) {
                        if let Ok(bytes) = s.read_to_end(1 << 20).await {
                            handler(peer, bytes);
                        }
                    }
                    match conn.accept_uni().await {
                        Ok(s) => streams.push(s),
                        Err(_) => return Ok(()),
                    }
                }
            }
        };
        let mut sender = Sender {
            send,
            filter: self.cfg.filter.clone(),
            log: self.log.clone(),
        };
        // the first known frame is an Attach, or a delegated client's
        // delegation on a connection that opens no session (§8.2); unknown
        // ones are skipped, and a request frame from a peer already bound
        // makes this a request-only connection from a pinned peer, whose
        // first stream is a request stream and never stream 0
        let (attach_bytes, attach_body, attach_item) = loop {
            match read_frame(&mut recv, bounds::CONTROL_FRAME_BYTES).await {
                FrameRead::Closed(_) => return Err("closed before attach".into()),
                FrameRead::OverBound(_) => {
                    self.log.push(Event::OverBound);
                    conn.close(VarInt::from_u32(CLOSE_REFUSED), b"frame over bound");
                    return Err("over bound".into());
                }
                FrameRead::Payload(p) => match classify(&p) {
                    Control::Known(Family::Attach, b, body, it) => break (b, body, it),
                    Control::Known(Family::Delegation, b, body, _) => {
                        if authenticated.is_some() {
                            // a pinned peer owes no frame; one it sends is skipped
                            self.log.push(Event::Skipped {
                                frame_type: FRAME_DELEGATION,
                            });
                            continue;
                        }
                        let raw = &b[body];
                        let (peer, how) =
                            match self
                                .cfg
                                .bind
                                .server(&self.cfg.pins, None, &presented, Some(raw))
                            {
                                Ok(x) => x,
                                Err(why) => return self.refuse_unbound(&conn, why),
                            };
                        self.log.push(Event::Bound { how });
                        return self.serve_requests(conn, peer, sender, recv, None).await;
                    }
                    Control::Known(..) => {
                        conn.close(
                            VarInt::from_u32(CLOSE_REFUSED),
                            b"known frame before attach",
                        );
                        return Err("known frame before attach".into());
                    }
                    other => {
                        if let Some(peer) = self.bound_without_frame(&presented)
                            && frame::parse_payload(Stream::Request, &p).is_ok()
                        {
                            self.log.push(Event::Bound {
                                how: if authenticated.is_some() {
                                    Bound::Pinned
                                } else {
                                    Bound::Held
                                },
                            });
                            let Sender { send, .. } = sender;
                            return self
                                .serve_requests(conn, peer, Sender::detached(send), recv, Some(p))
                                .await;
                        }
                        match other {
                            Control::Unknown(t) => self.log.push(Event::Skipped { frame_type: t }),
                            _ => self.log.push(Event::Discarded),
                        }
                    }
                },
            }
        };
        self.log.push(Event::Received {
            frame_type: FRAME_ATTACH,
        });
        let Item::Map(m) = &attach_item else {
            unreachable!()
        };
        let claimed: [u8; 32] = match map_get(m, 1) {
            Some(Item::Bytes(r)) => attach_bytes[r.clone()].try_into().unwrap(),
            _ => unreachable!("schema checked"),
        };
        // §9.1: field 1 names an identity whose classical member is the
        // key presented, or field 4 carries that identity's delegation
        // naming it, or one is held from the topology class.  A mismatch
        // is answered with no frame; the close code the wire assigns to
        // refusal is used, since no other is defined.
        let field4 = field_bytes(&attach_bytes, attach_body.start, 4);
        let how = match self.cfg.bind.server(
            &self.cfg.pins,
            Some(&claimed),
            &presented,
            field4.as_deref(),
        ) {
            Ok((_, how)) => how,
            Err(why) => return self.refuse_unbound(&conn, why),
        };
        self.log.push(Event::Bound { how });
        if self.is_superseded(&claimed) {
            self.log.push(Event::Superseded);
            conn.close(VarInt::from_u32(CLOSE_REFUSED), b"superseded");
            return Err("superseded".into());
        }
        if !(self.cfg.policy)(&claimed) {
            self.log.push(Event::Refused);
            conn.close(VarInt::from_u32(CLOSE_REFUSED), b"");
            return Err("refused".into());
        }
        let mode = if (self.cfg.in_subtree)(&claimed) {
            0
        } else {
            1
        };
        let queued = self.queued(&claimed) as u64;
        let mut caps = self.cfg.capabilities.clone();
        let known: Vec<u64> = caps.keys().copied().collect();
        let (gid, gval) = grease(&known);
        caps.insert(gid, gval);
        let ack = AttachAck {
            mode,
            siblings: (self.cfg.siblings)(),
            interval: self.cfg.interval_secs,
            queued,
            capabilities: caps,
            delegation: self.cfg.bind.own_delegation(),
        };
        sender
            .frame(FRAME_ATTACH_ACK, &ack.encode())
            .await
            .map_err(|e| e.to_string())?;
        self.log.push(Event::Attached { mode });
        let reach = Arc::new(Mutex::new(Reachability::Reachable));
        let (otx, orx) = mpsc::unbounded_channel();
        {
            let mut st = self.state.lock().unwrap();
            st.sessions.insert(claimed, conn.clone());
            st.reach.insert(claimed, reach.clone());
            st.outbound.insert(claimed, otx);
        }
        if let Some(f) = &self.cfg.on_attach {
            f(claimed, true);
        }
        // drain: each waiting message goes out oldest first and leaves the
        // store only once the peer has taken it, so a session that fails
        // mid-drain leaves the rest where it was (design §14.1.6)
        tokio::spawn(drain(self.clone(), claimed, conn.clone()));
        // requests on bidirectional streams, answered for as long as the session lives
        let req_conn = conn.clone();
        let handler = self.cfg.on_request.clone();
        let requests = tokio::spawn(async move {
            while let Ok((send, recv)) = req_conn.accept_bi().await {
                tokio::spawn(answer_request(send, recv, claimed, handler.clone()));
            }
        });
        let interval = Duration::from_secs(self.cfg.interval_secs);
        let log = self.log.clone();
        let node = self.clone();
        let on_change: Arc<dyn Fn(Reachability) + Send + Sync> = Arc::new(move |r| {
            node.state.lock().unwrap().marked.insert(claimed, r);
            if let Some(f) = &node.cfg.replicate {
                f(claimed, r);
            }
        });
        let on_control = self.cfg.on_control.clone();
        let _ = control_loop(
            sender,
            recv,
            LoopIo {
                interval,
                log,
                reach,
                on_change: Some(on_change),
                outbound: orx,
            },
            move |fam, b, body, _| match fam {
                Family::Attach | Family::AttachAck => false,
                Family::TopologyPush | Family::TopologyMemo => {
                    if let Some(h) = &on_control {
                        h(
                            claimed,
                            if fam == Family::TopologyPush { 5 } else { 6 },
                            b[body].to_vec(),
                        );
                    }
                    true
                }
                _ => true,
            },
        )
        .await;
        requests.abort();
        {
            let mut st = self.state.lock().unwrap();
            st.sessions.remove(&claimed);
            st.reach.remove(&claimed);
            st.outbound.remove(&claimed);
        }
        if let Some(f) = &self.cfg.on_attach {
            f(claimed, false);
        }
        self.log.push(Event::Closed);
        Ok(())
    }
}

impl Node {
    /// Refuse a connection nothing binds (§9.1): no frame in reply, the
    /// refusal close code, and the reason where the dialler acts on it,
    /// a window refusal being the one it retries (§8.2).
    fn refuse_unbound(&self, conn: &Connection, why: Refusal) -> Result<(), String> {
        let reason: &[u8] = match why {
            Refusal::Window => b"window",
            Refusal::WrongKeyhash => b"identity mismatch",
            _ => b"unbound",
        };
        self.log.push(Event::Unbound { why: why.clone() });
        conn.close(VarInt::from_u32(CLOSE_REFUSED), reason);
        Err(format!("refused: {why}"))
    }

    /// The keyhash `presented` speaks as with no frame read: the pin, or a
    /// delegation held from the topology class naming it, in window.
    fn bound_without_frame(&self, presented: &[u8; 32]) -> Option<[u8; 32]> {
        self.cfg.pins.keyhash_for_key(presented).or_else(|| {
            self.cfg
                .bind
                .held
                .by_key(presented)
                .filter(|d| self.cfg.bind.in_window(d))
                .map(|d| d.keyhash)
        })
    }

    /// A connection that opens no session (§8.2): every request stream is
    /// answered as `peer`'s until the connection closes.  Nothing is
    /// queued, drained or replicated for it, and the control stream, if
    /// the peer opened one, is held and read for nothing further.  `first`
    /// is a request already read from what turned out to be a request
    /// stream, answered on it.
    async fn serve_requests(
        self: Arc<Self>,
        conn: Connection,
        peer: [u8; 32],
        control: Sender,
        control_recv: RecvStream,
        first: Option<Vec<u8>>,
    ) -> Result<(), String> {
        self.log.push(Event::RequestOnly);
        let handler = self.cfg.on_request.clone();
        let mut control_recv = Some(control_recv);
        if let Some(p) = first {
            let Sender { send, .. } = control;
            let _ = control_recv.take();
            tokio::spawn(answer_payload(send, p, peer, handler.clone()));
        } else {
            // held open until the connection ends: closing it would tell
            // the peer to stop sending on a stream the wire leaves to it
            let c = conn.clone();
            tokio::spawn(async move {
                let _control = control;
                let _recv = control_recv;
                c.closed().await;
            });
        }
        while let Ok((send, recv)) = conn.accept_bi().await {
            tokio::spawn(answer_request(send, recv, peer, handler.clone()));
        }
        self.log.push(Event::Closed);
        Ok(())
    }
}

/// Deliver one message on a fresh unidirectional stream and wait for the
/// peer to take it: the stream written, finished, and every byte
/// acknowledged received.  Anything short of that is no delivery, and the
/// caller leaves the message where it was.  The direct path delivers the
/// same way, peer to peer.
pub async fn deliver(conn: &Connection, bytes: Vec<u8>) -> bool {
    let Ok(mut s) = conn.open_uni().await else {
        return false;
    };
    if s.write_all(&bytes).await.is_err() || s.finish().is_err() {
        return false;
    }
    matches!(s.stopped().await, Ok(None))
}

/// Deliver what waits for `recipient` on `conn`, oldest first, each removed
/// from the store once taken and not before (design §14.1.6,
/// `infra-client-requirements.md` §2: delete on delivery).  One drain runs
/// per recipient at a time; a failed delivery ends the drain, and the rest
/// waits for the next session.
async fn drain(node: Arc<Node>, recipient: [u8; 32], conn: Connection) {
    let lock = node.drain_lock(&recipient);
    let _running = lock.lock().await;
    while let Some(item) = node.cfg.queue.peek_oldest(&recipient) {
        if !deliver(&conn, item.ciphertext.clone()).await {
            return;
        }
        node.cfg.queue.remove(&recipient, &item);
    }
}

/// A request stream: the node's own handler answers where one is
/// installed; otherwise a currency request gets `cannot issue` (§7.1) and
/// anything else fails the stream (§9.2).
async fn answer_request(
    send: SendStream,
    mut recv: RecvStream,
    peer: [u8; 32],
    handler: Option<RequestHandler>,
) {
    let FrameRead::Payload(p) = read_frame(&mut recv, bounds::REQUEST_FRAME_BYTES).await else {
        return;
    };
    answer_payload(send, p, peer, handler).await
}

/// Answer one request whose payload is already read, on `send`.
async fn answer_payload(
    mut send: SendStream,
    p: Vec<u8>,
    peer: [u8; 32],
    handler: Option<RequestHandler>,
) {
    let f = match frame::parse_payload(Stream::Request, &p) {
        Ok(f) => f,
        // A resource request whose body does not decode is answered code 3
        // rather than failing the stream (`wire-format.md` §11's evaluation
        // order, step 0), so its handler is given the body to refuse.  Its
        // outer frame must still parse: a malformed frame is the stream's
        // failure, and every other request type's malformed body has no
        // answer defined (§9.2).
        Err(_) => match frame::parse_outer(Stream::Request, &p) {
            Ok(f) if f.family == Some(Family::ResourceRequest) => f,
            _ => {
                let _ = send.reset(VarInt::from_u32(0));
                return;
            }
        },
    };
    if let (Some(h), Some(fam)) = (handler, f.family) {
        let body = p[f.body.clone()].to_vec();
        match h(peer, fam, body).await {
            Some(reply) => {
                let mut out = (reply.len() as u32).to_be_bytes().to_vec();
                out.extend_from_slice(&reply);
                let _ = send.write_all(&out).await;
                let _ = send.finish();
            }
            None => {
                let _ = send.reset(VarInt::from_u32(0));
            }
        }
        return;
    }
    if f.family == Some(Family::CurrencyRequest) {
        let Item::Map(m) = &f.body_item else { return };
        let nonce = match map_get(m, 2) {
            Some(Item::Bytes(r)) => p[r.clone()].to_vec(),
            _ => return,
        };
        let mut body = Vec::new();
        emit_map_head(&mut body, 2);
        emit_uint(&mut body, 1);
        emit_bstr(&mut body, &nonce);
        emit_uint(&mut body, 2);
        emit_uint(&mut body, 1);
        let mut out = (body.len() as u32).to_be_bytes().to_vec();
        out.extend_from_slice(&body);
        let _ = send.write_all(&out).await;
        let _ = send.finish();
    } else {
        let _ = send.reset(VarInt::from_u32(0));
    }
}

// ------------------------------------------------------------ the client

#[derive(Clone)]
pub struct ClientConfig {
    /// Who this client speaks as and what it presents: the identity's own
    /// classical member on the ceremony device, or the delegated key of a
    /// desktop that holds no seed (design §23.3).
    pub me: Party,
    pub pins: Pins,
    /// §9.1's bind on the dialling side, and this client's own delegated
    /// credential where it is a delegated device (design §23.3).
    pub bind: Binding,
    pub capabilities: BTreeMap<u64, Vec<u8>>,
    pub attestation: Option<Vec<u8>>,
    pub filter: Option<OutboundFilter>,
    /// The cached sibling list, persisted by the client (§8.2).
    pub sibling_cache: Arc<Mutex<Vec<SiblingRef>>>,
    /// Endpoints to dial for a keyhash during failover; the test's address book.
    pub addresses: Arc<Mutex<HashMap<[u8; 32], Vec<std::net::SocketAddr>>>>,
    /// One TLS configuration per target, kept so its resumption store is
    /// shared across dials and 0-RTT reattachment is possible (§9.2).
    pub tls: Arc<Mutex<HashMap<[u8; 32], rustls::ClientConfig>>>,
    /// How long one dial may take before the next endpoint is tried.
    /// Selection and retry are local policy (`wire-format.md` §7.7.3), so
    /// this is the client's own number and no part of the wire.
    pub connect_timeout: Duration,
    /// Where this session's reachability verdicts go when the detector
    /// settles them: a node feeding its currency ladder from what its own
    /// sessions tell it (design §12.6.5.1), or nobody.
    pub on_reachability: Option<Arc<dyn Fn(Reachability) + Send + Sync>>,
    /// Where the session's events go: nowhere by default; a test installs
    /// a recording log.
    pub log: Log,
}

impl ClientConfig {
    pub fn tls_for(&self, target: &[u8; 32]) -> Option<rustls::ClientConfig> {
        let mut map = self.tls.lock().unwrap();
        if let Some(c) = map.get(target) {
            return Some(c.clone());
        }
        let c = tls::client_config(self.presenter(), &self.pins, target)?;
        map.insert(*target, c.clone());
        Some(c)
    }

    /// What this client presents in the handshake.
    pub fn presenter(&self) -> Presenter {
        self.me.presenter.clone()
    }

    /// Run as a delegated device: what is presented becomes the
    /// credential's key, and the Attach carries its delegation.
    pub fn with_credential(mut self, c: Arc<tls::Credential>) -> Self {
        self.me = Party::of(c.clone());
        self.bind = self.bind.with_credential(c);
        self
    }
}

pub struct Session {
    pub conn: Connection,
    pub ack: AttachAck,
    pub deliveries: mpsc::UnboundedReceiver<Vec<u8>>,
    /// Known control frames beyond the session's own, as `(type, body)`:
    /// topology pushes and memos the serving node delivers.
    pub frames: mpsc::UnboundedReceiver<(u64, Vec<u8>)>,
    /// Control frames this client sends on stream 0.
    pub outbound: mpsc::UnboundedSender<(u64, Vec<u8>)>,
    pub log: Log,
    pub reach: Arc<Mutex<Reachability>>,
    pub task: tokio::task::JoinHandle<()>,
}

#[derive(Debug)]
pub enum AttachOutcome {
    Attached(Session),
    /// Close code 1: the node's answer, not the endpoint's (§8.2).
    Refused,
    /// The handshake completed but nothing bound the key it presented to
    /// the keyhash meant (§9.1): a refused session, never a session with
    /// that keyhash.  `Window` is retried once by `attach_any`.
    Unbound(Refusal),
    EndpointFailure(String),
}

impl std::fmt::Debug for Session {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Session(mode {}, interval {})",
            self.ack.mode, self.ack.interval
        )
    }
}

fn close_code(e: &quinn::ConnectionError) -> Option<u64> {
    match e {
        quinn::ConnectionError::ApplicationClosed(ac) => Some(ac.error_code.into_inner()),
        _ => None,
    }
}

fn close_reason(e: &quinn::ConnectionError) -> Option<&[u8]> {
    match e {
        quinn::ConnectionError::ApplicationClosed(ac) => Some(ac.reason.as_ref()),
        _ => None,
    }
}

/// Dial `target` at `addr` and attach on stream 0.  With `early`, the Attach
/// rides 0-RTT early data when a resumption ticket is held; the node acts on
/// it only after the handshake (§8.2).
pub async fn attach(
    cfg: &ClientConfig,
    endpoint: &quinn::Endpoint,
    target: [u8; 32],
    addr: std::net::SocketAddr,
    early: bool,
) -> AttachOutcome {
    let Some(tls_cfg) = cfg.tls_for(&target) else {
        return AttachOutcome::EndpointFailure("NotPinned".into());
    };
    let connecting = match tls::dial_with(endpoint, tls_cfg, addr) {
        Ok(c) => c,
        Err(e) => return AttachOutcome::EndpointFailure(format!("{e:?}")),
    };
    let log = cfg.log.fresh();
    let conn = if early {
        match connecting.into_0rtt() {
            Ok((conn, accepted)) => {
                log.push(Event::EarlyDataSent);
                let hlog = log.clone();
                tokio::spawn(async move {
                    let ok = accepted.await;
                    hlog.push(Event::HandshakeDone { early_accepted: ok });
                });
                conn
            }
            Err(connecting) => match tokio::time::timeout(cfg.connect_timeout, connecting).await {
                Ok(Ok(c)) => {
                    log.push(Event::HandshakeDone {
                        early_accepted: false,
                    });
                    c
                }
                Ok(Err(e)) => return AttachOutcome::EndpointFailure(e.to_string()),
                Err(_) => return AttachOutcome::EndpointFailure("dial timed out".into()),
            },
        }
    } else {
        match tokio::time::timeout(cfg.connect_timeout, connecting).await {
            Ok(Ok(c)) => {
                log.push(Event::HandshakeDone {
                    early_accepted: false,
                });
                c
            }
            Ok(Err(e)) => return AttachOutcome::EndpointFailure(e.to_string()),
            Err(_) => return AttachOutcome::EndpointFailure("dial timed out".into()),
        }
    };
    attach_on(cfg, conn, log, target).await
}

async fn attach_on(
    cfg: &ClientConfig,
    conn: Connection,
    log: Log,
    target: [u8; 32],
) -> AttachOutcome {
    let (send, mut recv) = match conn.open_bi().await {
        Ok(s) => s,
        Err(e) => return AttachOutcome::EndpointFailure(e.to_string()),
    };
    let mut sender = Sender {
        send,
        filter: cfg.filter.clone(),
        log: log.clone(),
    };
    let mut caps = cfg.capabilities.clone();
    let known: Vec<u64> = caps.keys().copied().collect();
    let (gid, gval) = grease(&known);
    caps.insert(gid, gval);
    let own = cfg.bind.own_delegation();
    let body = encode_attach(
        &cfg.me.keyhash,
        cfg.attestation.as_deref(),
        &caps,
        own.as_deref(),
    );
    if let Err(e) = sender.frame(FRAME_ATTACH, &body).await {
        return AttachOutcome::EndpointFailure(e.to_string());
    }
    // wait for the ack: unknown frames skipped, any other known frame fails the attempt
    let ack = loop {
        match read_frame(&mut recv, bounds::CONTROL_FRAME_BYTES).await {
            FrameRead::Closed(Some(e)) if close_code(&e) == Some(CLOSE_REFUSED as u64) => {
                // the node's refusal of the bind on its side, named so the
                // dialler retries a window refusal (§8.2)
                if close_reason(&e) == Some(b"window") {
                    log.push(Event::Unbound {
                        why: Refusal::Window,
                    });
                    return AttachOutcome::Unbound(Refusal::Window);
                }
                log.push(Event::Refused);
                return AttachOutcome::Refused;
            }
            FrameRead::Closed(e) => {
                return AttachOutcome::EndpointFailure(format!("closed before ack: {e:?}"));
            }
            FrameRead::OverBound(_) => {
                conn.close(VarInt::from_u32(0), b"frame over bound");
                return AttachOutcome::EndpointFailure("over-bound frame".into());
            }
            FrameRead::Payload(p) => match classify(&p) {
                Control::Known(Family::AttachAck, b, body, it) => {
                    match AttachAck::decode(&b, body.start, &it) {
                        Some(a) if (1..=3600).contains(&a.interval) => break a,
                        _ => log.push(Event::Discarded),
                    }
                }
                Control::Unknown(t) => log.push(Event::Skipped { frame_type: t }),
                // an acknowledgement that does not decode binds nothing,
                // and a delegation the codec refuses (classical-only, or a
                // window not exactly 48 hours) is what makes one not
                // decode: the session is refused, not waited on (§8.2)
                Control::Malformed
                    if frame::parse_outer(Stream::Control, &p)
                        .is_ok_and(|f| f.frame_type == FRAME_ATTACH_ACK) =>
                {
                    let why = Refusal::Malformed("AttachAck does not decode".into());
                    log.push(Event::Unbound { why: why.clone() });
                    conn.close(VarInt::from_u32(CLOSE_REFUSED), b"unbound");
                    return AttachOutcome::Unbound(why);
                }
                Control::Malformed => log.push(Event::Discarded),
                Control::Known(..) => {
                    return AttachOutcome::EndpointFailure("known frame before ack".into());
                }
            },
        }
    };
    log.push(Event::Received {
        frame_type: FRAME_ATTACH_ACK,
    });
    // §9.1: the key the handshake presented is bound to the keyhash meant
    // by the pin, by a held delegation, or by AttachAck field 6; nothing
    // beyond the Attach was sent, and a session nothing binds is refused.
    // The key is read here and not before: an Attach in 0-RTT early data
    // goes out before the handshake completes, and the acknowledgement
    // arrives only after it has (§8.2)
    let Some(presented) = tls::peer_key(&conn) else {
        return AttachOutcome::EndpointFailure("no peer key".into());
    };
    match cfg
        .bind
        .dialler(&cfg.pins, &target, &presented, ack.delegation.as_deref())
    {
        Ok(how) => log.push(Event::Bound { how }),
        Err(why) => {
            log.push(Event::Unbound { why: why.clone() });
            conn.close(VarInt::from_u32(CLOSE_REFUSED), b"unbound");
            return AttachOutcome::Unbound(why);
        }
    }
    log.push(Event::Attached { mode: ack.mode });
    *cfg.sibling_cache.lock().unwrap() = ack.siblings.clone();
    let (dtx, drx) = mpsc::unbounded_channel();
    let dconn = conn.clone();
    let dlog = log.clone();
    tokio::spawn(async move {
        while let Ok(mut s) = dconn.accept_uni().await {
            if let Ok(bytes) = s.read_to_end(1 << 20).await {
                dlog.push(Event::Delivered {
                    bytes: bytes.clone(),
                });
                let _ = dtx.send(bytes);
            }
        }
    });
    let reach = Arc::new(Mutex::new(Reachability::Reachable));
    let interval = Duration::from_secs(ack.interval);
    let cache = cfg.sibling_cache.clone();
    let me = cfg.me.keyhash;
    let loop_log = log.clone();
    let loop_reach = reach.clone();
    let on_reachability = cfg.on_reachability.clone();
    let task_log = log.clone();
    let task_conn = conn.clone();
    let (ftx, frx) = mpsc::unbounded_channel();
    let (otx, orx) = mpsc::unbounded_channel();
    let task = tokio::spawn(async move {
        let _ = control_loop(
            sender,
            recv,
            LoopIo {
                interval,
                log: loop_log.clone(),
                reach: loop_reach.clone(),
                on_change: on_reachability,
                outbound: orx,
            },
            move |fam, b, body, it| match fam {
                Family::SiblingUpdate => {
                    let Item::Map(m) = it else { return true };
                    match decode_sibling_list(b, map_get(m, 1)) {
                        Some(list) if valid_sibling_list(&list, &me) => {
                            *cache.lock().unwrap() = list
                        }
                        _ => loop_log.push(Event::Discarded),
                    }
                    true
                }
                Family::Attach | Family::AttachAck => false,
                Family::TopologyPush => {
                    let _ = ftx.send((5, b[body].to_vec()));
                    true
                }
                Family::TopologyMemo => {
                    let _ = ftx.send((6, b[body].to_vec()));
                    true
                }
                _ => true,
            },
        )
        .await;
        task_conn.close(VarInt::from_u32(0), b"");
        task_log.push(Event::Closed);
    });
    AttachOutcome::Attached(Session {
        conn,
        ack,
        deliveries: drx,
        frames: frx,
        outbound: otx,
        log,
        reach,
        task,
    })
}

impl Session {
    /// Send a control frame on this session's stream 0.
    pub fn send_control(&self, frame_type: u64, body: &[u8]) -> bool {
        self.outbound.send((frame_type, body.to_vec())).is_ok()
    }

    /// One request on a fresh bidirectional stream (`wire-format.md` §9.2):
    /// the framed request out, the length-prefixed reply body back.
    pub async fn request(&self, frame_type: u64, body: &[u8]) -> Result<Vec<u8>, String> {
        request_on(&self.conn, frame_type, body).await
    }
}

/// One request on a fresh bidirectional stream of `conn` (`wire-format.md`
/// §9.2): the framed request out, the length-prefixed reply body back.
pub async fn request_on(
    conn: &Connection,
    frame_type: u64,
    body: &[u8],
) -> Result<Vec<u8>, String> {
    let (mut send, mut recv) = conn.open_bi().await.map_err(|e| e.to_string())?;
    send.write_all(&control_frame(frame_type, body))
        .await
        .map_err(|e| e.to_string())?;
    send.finish().map_err(|e| e.to_string())?;
    match read_frame(&mut recv, bounds::REQUEST_FRAME_BYTES).await {
        FrameRead::Payload(p) => Ok(p),
        FrameRead::OverBound(n) => Err(format!("reply over bound: {n}")),
        FrameRead::Closed(e) => Err(format!("stream ended: {e:?}")),
    }
}

/// Try a node's endpoints as alternatives (`light-client-requirements.md`
/// §4): an endpoint failure moves to the next address; a refusal is the
/// node's answer and ends the attempt (§8.2).
pub async fn attach_any(
    cfg: &ClientConfig,
    endpoint: &quinn::Endpoint,
    target: [u8; 32],
    addrs: &[std::net::SocketAddr],
    early: bool,
) -> AttachOutcome {
    let mut last = AttachOutcome::EndpointFailure("no endpoints".into());
    for addr in addrs {
        let mut outcome = attach(cfg, endpoint, target, *addr, early).await;
        // a window refusal is retried once at the same endpoint: the peer
        // may have rolled to its next credential between the two checks
        // (`wire-format.md` §8.2)
        if matches!(outcome, AttachOutcome::Unbound(Refusal::Window)) {
            outcome = attach(cfg, endpoint, target, *addr, early).await;
        }
        match outcome {
            AttachOutcome::Attached(s) => return AttachOutcome::Attached(s),
            AttachOutcome::Refused => return AttachOutcome::Refused,
            other => last = other,
        }
    }
    last
}

/// Dial `target` for requests alone, with no session (`wire-format.md`
/// §8.2, §9.1): the pin or a held delegation binds the presented key with
/// no frame; otherwise nothing is sent until the peer's delegation frame,
/// the first on a stream it opens, verifies and names that key, and the
/// connection is refused if the first frame is anything else.  A
/// delegated dialler presents its own delegation first, on stream 0.
pub async fn connect_request_only(
    cfg: &ClientConfig,
    endpoint: &quinn::Endpoint,
    target: [u8; 32],
    addr: std::net::SocketAddr,
) -> Result<(Connection, Bound), String> {
    let tls_cfg = cfg.tls_for(&target).ok_or("NotPinned")?;
    let connecting = tls::dial_with(endpoint, tls_cfg, addr).map_err(|e| format!("{e:?}"))?;
    let conn = match tokio::time::timeout(cfg.connect_timeout, connecting).await {
        Ok(Ok(c)) => c,
        Ok(Err(e)) => return Err(e.to_string()),
        Err(_) => return Err("dial timed out".into()),
    };
    // recorded into the caller's log: there is no session to carry one
    let log = cfg.log.clone();
    let presented = tls::peer_key(&conn).ok_or("no peer key")?;
    let how = match cfg.bind.without_frame(&cfg.pins, &target, &presented) {
        Ok(Some(how)) => how,
        Ok(None) | Err(Refusal::Window) => {
            // the peer's frame, read before anything is sent
            let frame = async {
                let (_, mut recv) = conn.accept_bi().await.map_err(|e| e.to_string())?;
                match read_frame(&mut recv, bounds::CONTROL_FRAME_BYTES).await {
                    FrameRead::Payload(p) => Ok(p),
                    other => Err(format!("no frame: {other:?}")),
                }
            };
            let p = match tokio::time::timeout(cfg.connect_timeout, frame).await {
                Ok(Ok(p)) => p,
                Ok(Err(e)) => return refuse(&conn, &log, Refusal::Unbound, e),
                Err(_) => return refuse(&conn, &log, Refusal::Unbound, "no first frame".into()),
            };
            let raw = match classify(&p) {
                Control::Known(Family::Delegation, b, body, _) => b[body].to_vec(),
                _ => {
                    return refuse(
                        &conn,
                        &log,
                        Refusal::Unbound,
                        "first frame is not a delegation".into(),
                    );
                }
            };
            log.push(Event::Received {
                frame_type: FRAME_DELEGATION,
            });
            match cfg
                .bind
                .verify_presented(&cfg.pins, Some(&target), &presented, &raw)
            {
                Ok(_) => Bound::Presented,
                Err(why) => return refuse(&conn, &log, why.clone(), why.to_string()),
            }
        }
        Err(why) => return refuse(&conn, &log, why.clone(), why.to_string()),
    };
    log.push(Event::Bound { how });
    // a delegated dialler's own delegation, first on stream 0, before any
    // request stream opens
    if let Some(raw) = cfg.bind.own_delegation() {
        let (mut s, _r) = conn.open_bi().await.map_err(|e| e.to_string())?;
        s.write_all(&control_frame(FRAME_DELEGATION, &raw))
            .await
            .map_err(|e| e.to_string())?;
        log.push(Event::DelegationPresented);
        // held open for the connection's life: a finished stream 0 is not
        // what the peer expects to read from
        let c = conn.clone();
        tokio::spawn(async move {
            let _s = s;
            let _r = _r;
            c.closed().await;
        });
    }
    Ok((conn, how))
}

fn refuse<T>(conn: &Connection, log: &Log, why: Refusal, detail: String) -> Result<T, String> {
    log.push(Event::Unbound { why });
    conn.close(VarInt::from_u32(CLOSE_REFUSED), b"unbound");
    Err(format!("unbound: {detail}"))
}

/// A fresh attach: the client's actual serving node first, and its cached
/// sibling list immediately where that node is unreachable at attach time
/// (`wire-format.md` §8.2, design §14.1.2).  No wait of three intervals
/// applies here — the three-interval rule governs a *running* session.
pub async fn fresh_attach(
    cfg: &ClientConfig,
    endpoint: &quinn::Endpoint,
    serving: [u8; 32],
    early: bool,
) -> AttachOutcome {
    let addrs = cfg
        .addresses
        .lock()
        .unwrap()
        .get(&serving)
        .cloned()
        .unwrap_or_default();
    match attach_any(cfg, endpoint, serving, &addrs, early).await {
        AttachOutcome::Attached(s) => return AttachOutcome::Attached(s),
        AttachOutcome::Refused => return AttachOutcome::Refused,
        _ => {}
    }
    let siblings = cfg.sibling_cache.lock().unwrap().clone();
    let mut last = AttachOutcome::EndpointFailure(
        "serving node unreachable and no cached sibling answered".into(),
    );
    for sib in siblings {
        if let Some(km) = &sib.key_material {
            let _ = cfg.pins.pin(sib.keyhash, km);
        }
        // **the ack carried the sibling's endpoints, and they are the
        // point of carrying them**: §4's rule is that the list is pushed
        // at attach because it cannot be discovered once the serving node
        // is dark, and an address book this client filled in beforehand is
        // exactly what it would not have.
        let mut addrs = cfg
            .addresses
            .lock()
            .unwrap()
            .get(&sib.keyhash)
            .cloned()
            .unwrap_or_default();
        for p in &sib.endpoints {
            let a = p.socket();
            if !addrs.contains(&a) {
                addrs.push(a);
            }
        }
        match attach_any(cfg, endpoint, sib.keyhash, &addrs, early).await {
            AttachOutcome::Attached(s) => return AttachOutcome::Attached(s),
            other => last = other,
        }
    }
    last
}

impl Session {
    /// A degraded session holds the replicated state but not the authority
    /// to countersign (design §14.1.2).  Payload flows; the subnet-scoped
    /// transactions the patron countersigns wait for the patron.
    pub fn may_countersign(&self) -> bool {
        self.ack.mode == 0
    }

    /// Whether this session is the degraded kind.
    pub fn degraded(&self) -> bool {
        self.ack.mode == 1
    }
}

/// §8.2: a list naming the receiver or holding duplicates is malformed.
fn valid_sibling_list(list: &[SiblingRef], me: &[u8; 32]) -> bool {
    let mut seen = HashSet::new();
    list.iter()
        .all(|s| s.keyhash != *me && seen.insert(s.keyhash))
}

impl Session {
    /// Wait until the peer is judged unreachable (three missed intervals),
    /// then try the cached siblings in order (§8.2).  Returns the sibling
    /// session or the outcome of the last attempt.
    pub async fn failover(
        &mut self,
        cfg: &ClientConfig,
        endpoint: &quinn::Endpoint,
    ) -> AttachOutcome {
        loop {
            if *self.reach.lock().unwrap() == Reachability::Unreachable {
                break;
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
        self.conn.close(VarInt::from_u32(0), b"failover");
        let siblings = cfg.sibling_cache.lock().unwrap().clone();
        let mut last = AttachOutcome::EndpointFailure("no cached siblings".into());
        for s in siblings {
            if let Some(km) = &s.key_material {
                let _ = cfg.pins.pin(s.keyhash, km);
            }
            let addrs = cfg
                .addresses
                .lock()
                .unwrap()
                .get(&s.keyhash)
                .cloned()
                .unwrap_or_default();
            for addr in addrs {
                self.log.push(Event::Failover { to: s.keyhash });
                match attach(cfg, endpoint, s.keyhash, addr, false).await {
                    AttachOutcome::Attached(sess) => return AttachOutcome::Attached(sess),
                    AttachOutcome::Refused => {
                        last = AttachOutcome::Refused;
                        break; // one sibling's refusal forecloses that sibling alone
                    }
                    other => last = other,
                }
            }
        }
        last
    }
}
