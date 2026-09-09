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

use crate::tls::{self, Pins};
use quinn::{Connection, RecvStream, SendStream, VarInt};
use rhtn_codec::bounds;
use rhtn_codec::cbor::*;
use rhtn_codec::encode::*;
use rhtn_codec::frame::{self, Stream};
use rhtn_codec::schema::Family;
use rhtn_crypto::SigningIdentity;
use std::collections::{BTreeMap, HashMap, HashSet, VecDeque};
use std::sync::{Arc, Mutex};
use tokio::sync::mpsc;
use tokio::time::{Duration, Instant, sleep_until};

/// Application close code for a refused attach (§8.2).
pub const CLOSE_REFUSED: u32 = 1;
pub const FRAME_ATTACH: u64 = 1;
pub const FRAME_ATTACH_ACK: u64 = 2;
pub const FRAME_HEARTBEAT: u64 = 3;
pub const FRAME_SIBLING_UPDATE: u64 = 4;

// ------------------------------------------------------------ capabilities

/// `capability_id = first 8 bytes of SHA-256("rhtn/cap:" || name)`, big-endian (§8.1).
pub fn capability_id(name: &str) -> u64 {
    let h = rhtn_codec::cose::sha256(&[b"rhtn/cap:", name.as_bytes()].concat());
    u64::from_be_bytes(h[..8].try_into().unwrap())
}

fn secure_random(buf: &mut [u8]) {
    rustls::crypto::aws_lc_rs::default_provider().secure_random.fill(buf).expect("os randomness");
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

/// What a received control payload turned out to be.
pub enum Control {
    Known(Family, Vec<u8>, Item),
    /// Unknown type: skipped, session survives (§8.0).
    Unknown(u64),
    /// Malformed body of a known type: discarded whole, session survives (§8.0).
    Malformed,
}

pub fn classify(payload: &[u8]) -> Control {
    match frame::parse_payload(Stream::Control, payload) {
        Ok(f) => match f.family {
            Some(fam) => Control::Known(fam, payload.to_vec(), f.body_item),
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

impl NetworkPoint {
    fn encode(&self, out: &mut Vec<u8>) {
        emit_map_head(out, 1 + self.asn.is_some() as usize + self.port.is_some() as usize);
        emit_uint(out, 1);
        emit_bstr(out, &self.ip);
        if let Some(a) = self.asn {
            emit_uint(out, 2);
            emit_uint(out, a);
        }
        if let Some(p) = self.port {
            emit_uint(out, 3);
            emit_uint(out, p);
        }
    }
    fn decode(b: &[u8], it: &Item) -> Option<Self> {
        let Item::Map(m) = it else { return None };
        let ip = match map_get(m, 1) { Some(Item::Bytes(r)) => b[r.clone()].try_into().ok()?, _ => return None };
        Some(NetworkPoint { ip, asn: map_get(m, 2).and_then(as_uint), port: map_get(m, 3).and_then(as_uint) })
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
        let keyhash = match map_get(m, 1) { Some(Item::Bytes(r)) => b[r.clone()].try_into().ok()?, _ => return None };
        let Some(Item::Array(eps)) = map_get(m, 2) else { return None };
        let endpoints = eps.iter().map(|e| NetworkPoint::decode(b, e)).collect::<Option<Vec<_>>>()?;
        let key_material = map_get(m, 3).map(|km| reencode(km, b));
        Some(SiblingRef { keyhash, endpoints, key_material })
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
}

impl AttachAck {
    pub fn encode(&self) -> Vec<u8> {
        let mut out = Vec::new();
        emit_map_head(&mut out, 4 + !self.siblings.is_empty() as usize);
        emit_uint(&mut out, 1);
        emit_uint(&mut out, self.mode);
        encode_sibling_list(&mut out, 2, &self.siblings);
        emit_uint(&mut out, 3);
        emit_uint(&mut out, self.interval);
        emit_uint(&mut out, 4);
        emit_uint(&mut out, self.queued);
        emit_uint(&mut out, 5);
        out.extend_from_slice(&encode_capabilities(&self.capabilities));
        out
    }
    pub fn decode(b: &[u8], it: &Item) -> Option<Self> {
        let Item::Map(m) = it else { return None };
        Some(AttachAck {
            mode: map_get(m, 1).and_then(as_uint)?,
            siblings: decode_sibling_list(b, map_get(m, 2))?,
            interval: map_get(m, 3).and_then(as_uint)?,
            queued: map_get(m, 4).and_then(as_uint)?,
            capabilities: decode_capabilities(b, map_get(m, 5)?),
        })
    }
}

pub fn encode_attach(keyhash: &[u8; 32], attestation: Option<&[u8]>, caps: &BTreeMap<u64, Vec<u8>>) -> Vec<u8> {
    let mut out = Vec::new();
    emit_map_head(&mut out, 2 + attestation.is_some() as usize);
    emit_uint(&mut out, 1);
    emit_bstr(&mut out, keyhash);
    if let Some(a) = attestation {
        emit_uint(&mut out, 2);
        out.extend_from_slice(a);
    }
    emit_uint(&mut out, 3);
    out.extend_from_slice(&encode_capabilities(caps));
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
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

// ------------------------------------------------------------ shared machinery

/// A test's hand on the path: given a control frame about to be sent (its
/// type and complete bytes), return the bytes to send instead, or `None` to
/// drop it.
pub type OutboundFilter = Arc<dyn Fn(u64, &[u8]) -> Option<Vec<u8>> + Send + Sync>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    Sent { frame_type: u64, bytes: Vec<u8> },
    Received { frame_type: u64 },
    Skipped { frame_type: u64 },
    Discarded,
    OverBound,
    Attached { mode: u64 },
    Refused,
    PeerUnreachable,
    Failover { to: [u8; 32] },
    Delivered { bytes: Vec<u8> },
    Closed,
}

#[derive(Default, Clone)]
pub struct Log(Arc<Mutex<Vec<(Instant, Event)>>>);

impl Log {
    pub fn push(&self, e: Event) {
        self.0.lock().unwrap().push((Instant::now(), e));
    }
    pub fn events(&self) -> Vec<(Instant, Event)> {
        self.0.lock().unwrap().clone()
    }
    pub fn count(&self, f: impl Fn(&Event) -> bool) -> usize {
        self.0.lock().unwrap().iter().filter(|(_, e)| f(e)).count()
    }
}

struct Sender {
    send: SendStream,
    filter: Option<OutboundFilter>,
    log: Log,
}

impl Sender {
    async fn frame(&mut self, frame_type: u64, body: &[u8]) -> Result<(), quinn::WriteError> {
        let bytes = control_frame(frame_type, body);
        let bytes = match &self.filter {
            Some(f) => match f(frame_type, &bytes) {
                Some(b) => b,
                None => return Ok(()),
            },
            None => bytes,
        };
        self.log.push(Event::Sent { frame_type, bytes: bytes.clone() });
        self.send.write_all(&bytes).await
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Reachability {
    Reachable,
    Unreachable,
}

/// The heartbeat and liveness loop both roles run after the ack (§8.2).
/// Returns when the stream or connection ends.  `on_frame` sees every
/// known non-heartbeat frame.
async fn control_loop(
    mut sender: Sender,
    mut recv: RecvStream,
    interval: Duration,
    log: Log,
    reach: Arc<Mutex<Reachability>>,
    mut on_frame: impl FnMut(Family, &[u8], &Item) -> bool,
) -> Option<quinn::ConnectionError> {
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
            r = read_frame(&mut recv, bounds::CONTROL_FRAME_BYTES) => match r {
                FrameRead::Closed(e) => return e,
                FrameRead::OverBound(_) => { log.push(Event::OverBound); return None; }
                FrameRead::Payload(p) => match classify(&p) {
                    Control::Unknown(t) => log.push(Event::Skipped { frame_type: t }),
                    Control::Malformed => log.push(Event::Discarded),
                    Control::Known(Family::Heartbeat, _, item) => {
                        log.push(Event::Received { frame_type: FRAME_HEARTBEAT });
                        if let Item::Map(m) = &item {
                            if let Some(c) = map_get(m, 1).and_then(as_uint) {
                                if seen.insert(c) {
                                    last_valid = Instant::now();
                                    *reach.lock().unwrap() = Reachability::Reachable;
                                }
                            }
                        }
                    }
                    Control::Known(fam, bytes, item) => {
                        let t = match fam { Family::Attach => 1, Family::AttachAck => 2, Family::SiblingUpdate => 4, Family::TopologyPush => 5, Family::TopologyMemo => 6, _ => 0 };
                        log.push(Event::Received { frame_type: t });
                        if !on_frame(fam, &bytes, &item) { return None; }
                    }
                }
            }
        }
    }
}

// ------------------------------------------------------------ the serving node

pub type Predicate = Arc<dyn Fn(&[u8; 32]) -> bool + Send + Sync>;

pub struct NodeConfig {
    pub identity: Arc<SigningIdentity>,
    pub pins: Pins,
    /// Seconds, 1..=3600; unset by design §21.1, chosen by the operator.
    pub interval_secs: u64,
    pub siblings: Vec<SiblingRef>,
    /// Published capabilities by id.  A greased entry is added per session.
    pub capabilities: BTreeMap<u64, Vec<u8>>,
    /// Local attach policy: false refuses with close code 1.
    pub policy: Predicate,
    /// The node's own topology: true means the client is in this node's subtree (mode 0).
    pub in_subtree: Predicate,
    pub filter: Option<OutboundFilter>,
}

#[derive(Default)]
struct NodeState {
    queues: HashMap<[u8; 32], VecDeque<Vec<u8>>>,
    reach: HashMap<[u8; 32], Arc<Mutex<Reachability>>>,
    sessions: HashMap<[u8; 32], Connection>,
}

pub struct Node {
    pub cfg: NodeConfig,
    state: Mutex<NodeState>,
    pub log: Log,
}

impl Node {
    pub fn new(cfg: NodeConfig) -> Arc<Self> {
        Arc::new(Node { cfg, state: Mutex::new(NodeState::default()), log: Log::default() })
    }

    /// Queue material for a client; delivered now on a unidirectional stream
    /// if a session is up, otherwise held.
    pub fn enqueue(&self, keyhash: [u8; 32], bytes: Vec<u8>) {
        let conn = {
            let mut st = self.state.lock().unwrap();
            match st.sessions.get(&keyhash).cloned() {
                Some(c) => Some(c),
                None => {
                    st.queues.entry(keyhash).or_default().push_back(bytes.clone());
                    None
                }
            }
        };
        if let Some(conn) = conn {
            tokio::spawn(async move { deliver(&conn, bytes).await });
        }
    }

    pub fn queued(&self, keyhash: &[u8; 32]) -> usize {
        self.state.lock().unwrap().queues.get(keyhash).map_or(0, |q| q.len())
    }

    pub fn reachability(&self, keyhash: &[u8; 32]) -> Option<Reachability> {
        self.state.lock().unwrap().reach.get(keyhash).map(|r| *r.lock().unwrap())
    }

    pub fn has_session(&self, keyhash: &[u8; 32]) -> bool {
        self.state.lock().unwrap().sessions.contains_key(keyhash)
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
        let spki = tls::peer_spki(&conn).ok_or("no peer key")?;
        let authenticated = self.cfg.pins.keyhash_for_spki(&spki);
        let (send, mut recv) = conn.accept_bi().await.map_err(|e| e.to_string())?;
        let mut sender = Sender { send, filter: self.cfg.filter.clone(), log: self.log.clone() };
        // the first known frame must be Attach; unknown ones are skipped
        let (attach_bytes, attach_item) = loop {
            match read_frame(&mut recv, bounds::CONTROL_FRAME_BYTES).await {
                FrameRead::Closed(_) => return Err("closed before attach".into()),
                FrameRead::OverBound(_) => {
                    self.log.push(Event::OverBound);
                    conn.close(VarInt::from_u32(CLOSE_REFUSED), b"frame over bound");
                    return Err("over bound".into());
                }
                FrameRead::Payload(p) => match classify(&p) {
                    Control::Known(Family::Attach, b, it) => break (b, it),
                    Control::Unknown(t) => self.log.push(Event::Skipped { frame_type: t }),
                    Control::Malformed => self.log.push(Event::Discarded),
                    Control::Known(..) => {
                        conn.close(VarInt::from_u32(CLOSE_REFUSED), b"known frame before attach");
                        return Err("known frame before attach".into());
                    }
                },
            }
        };
        self.log.push(Event::Received { frame_type: FRAME_ATTACH });
        let Item::Map(m) = &attach_item else { unreachable!() };
        let claimed: [u8; 32] = match map_get(m, 1) {
            Some(Item::Bytes(r)) => attach_bytes[r.clone()].try_into().unwrap(),
            _ => unreachable!("schema checked"),
        };
        // §9.1: field 1 MUST equal the connection-authenticated identity.  A
        // mismatch is answered with no frame; the close code the wire assigns
        // to refusal is used, since no other is defined.
        if authenticated != Some(claimed) {
            conn.close(VarInt::from_u32(CLOSE_REFUSED), b"identity mismatch");
            return Err("attach identity mismatch".into());
        }
        if !(self.cfg.policy)(&claimed) {
            self.log.push(Event::Refused);
            conn.close(VarInt::from_u32(CLOSE_REFUSED), b"");
            return Err("refused".into());
        }
        let mode = if (self.cfg.in_subtree)(&claimed) { 0 } else { 1 };
        let queued = self.queued(&claimed) as u64;
        let mut caps = self.cfg.capabilities.clone();
        let known: Vec<u64> = caps.keys().copied().collect();
        let (gid, gval) = grease(&known);
        caps.insert(gid, gval);
        let ack = AttachAck { mode, siblings: self.cfg.siblings.clone(), interval: self.cfg.interval_secs, queued, capabilities: caps };
        sender.frame(FRAME_ATTACH_ACK, &ack.encode()).await.map_err(|e| e.to_string())?;
        self.log.push(Event::Attached { mode });
        let reach = Arc::new(Mutex::new(Reachability::Reachable));
        let pending: Vec<Vec<u8>> = {
            let mut st = self.state.lock().unwrap();
            st.sessions.insert(claimed, conn.clone());
            st.reach.insert(claimed, reach.clone());
            st.queues.remove(&claimed).map(|q| q.into_iter().collect()).unwrap_or_default()
        };
        for item in pending {
            deliver(&conn, item).await;
        }
        // requests on bidirectional streams, answered for as long as the session lives
        let req_conn = conn.clone();
        let requests = tokio::spawn(async move {
            while let Ok((send, recv)) = req_conn.accept_bi().await {
                tokio::spawn(answer_request(send, recv));
            }
        });
        let interval = Duration::from_secs(self.cfg.interval_secs);
        let log = self.log.clone();
        let _ = control_loop(sender, recv, interval, log, reach, |fam, _, _| !matches!(fam, Family::Attach | Family::AttachAck)).await;
        requests.abort();
        self.state.lock().unwrap().sessions.remove(&claimed);
        self.log.push(Event::Closed);
        Ok(())
    }
}

async fn deliver(conn: &Connection, bytes: Vec<u8>) {
    if let Ok(mut s) = conn.open_uni().await {
        let _ = s.write_all(&bytes).await;
        let _ = s.finish();
    }
}

/// The one request this node answers so far: a currency request gets
/// `cannot issue` (§7.1); anything else fails the stream (§9.2).
async fn answer_request(mut send: SendStream, mut recv: RecvStream) {
    let FrameRead::Payload(p) = read_frame(&mut recv, bounds::REQUEST_FRAME_BYTES).await else { return };
    let Ok(f) = frame::parse_payload(Stream::Request, &p) else {
        let _ = send.reset(VarInt::from_u32(0));
        return;
    };
    if f.family == Some(Family::CurrencyRequest) {
        let Item::Map(m) = &f.body_item else { return };
        let nonce = match map_get(m, 2) { Some(Item::Bytes(r)) => p[r.clone()].to_vec(), _ => return };
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

pub struct ClientConfig {
    pub identity: Arc<SigningIdentity>,
    pub pins: Pins,
    pub capabilities: BTreeMap<u64, Vec<u8>>,
    pub attestation: Option<Vec<u8>>,
    pub filter: Option<OutboundFilter>,
    /// The cached sibling list, persisted by the client (§8.2).
    pub sibling_cache: Arc<Mutex<Vec<SiblingRef>>>,
    /// Endpoints to dial for a keyhash during failover; the test's address book.
    pub addresses: Arc<Mutex<HashMap<[u8; 32], Vec<std::net::SocketAddr>>>>,
}

pub struct Session {
    pub conn: Connection,
    pub ack: AttachAck,
    pub deliveries: mpsc::UnboundedReceiver<Vec<u8>>,
    pub log: Log,
    pub reach: Arc<Mutex<Reachability>>,
    pub task: tokio::task::JoinHandle<()>,
}

#[derive(Debug)]
pub enum AttachOutcome {
    Attached(Session),
    /// Close code 1: the node's answer, not the endpoint's (§8.2).
    Refused,
    EndpointFailure(String),
}

impl std::fmt::Debug for Session {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Session(mode {}, interval {})", self.ack.mode, self.ack.interval)
    }
}

fn close_code(e: &quinn::ConnectionError) -> Option<u64> {
    match e {
        quinn::ConnectionError::ApplicationClosed(ac) => Some(ac.error_code.into_inner()),
        _ => None,
    }
}

/// Dial `target` at `addr` and attach on stream 0.  With `early`, the Attach
/// rides 0-RTT early data when a resumption ticket is held; the node acts on
/// it only after the handshake (§8.2).
pub async fn attach(cfg: &ClientConfig, endpoint: &quinn::Endpoint, target: [u8; 32], addr: std::net::SocketAddr, early: bool) -> AttachOutcome {
    let connecting = match tls::dial(endpoint, &cfg.identity, &cfg.pins, &target, addr) {
        Ok(c) => c,
        Err(e) => return AttachOutcome::EndpointFailure(format!("{e:?}")),
    };
    let conn = if early {
        match connecting.into_0rtt() {
            Ok((conn, _accepted)) => conn,
            Err(connecting) => match connecting.await {
                Ok(c) => c,
                Err(e) => return AttachOutcome::EndpointFailure(e.to_string()),
            },
        }
    } else {
        match connecting.await {
            Ok(c) => c,
            Err(e) => return AttachOutcome::EndpointFailure(e.to_string()),
        }
    };
    attach_on(cfg, conn).await
}

async fn attach_on(cfg: &ClientConfig, conn: Connection) -> AttachOutcome {
    let log = Log::default();
    let (send, mut recv) = match conn.open_bi().await {
        Ok(s) => s,
        Err(e) => return AttachOutcome::EndpointFailure(e.to_string()),
    };
    let mut sender = Sender { send, filter: cfg.filter.clone(), log: log.clone() };
    let mut caps = cfg.capabilities.clone();
    let known: Vec<u64> = caps.keys().copied().collect();
    let (gid, gval) = grease(&known);
    caps.insert(gid, gval);
    let body = encode_attach(&cfg.identity.public.keyhash, cfg.attestation.as_deref(), &caps);
    if let Err(e) = sender.frame(FRAME_ATTACH, &body).await {
        return AttachOutcome::EndpointFailure(e.to_string());
    }
    // wait for the ack: unknown frames skipped, any other known frame fails the attempt
    let ack = loop {
        match read_frame(&mut recv, bounds::CONTROL_FRAME_BYTES).await {
            FrameRead::Closed(Some(e)) if close_code(&e) == Some(CLOSE_REFUSED as u64) => {
                log.push(Event::Refused);
                return AttachOutcome::Refused;
            }
            FrameRead::Closed(e) => return AttachOutcome::EndpointFailure(format!("closed before ack: {e:?}")),
            FrameRead::OverBound(_) => {
                conn.close(VarInt::from_u32(0), b"frame over bound");
                return AttachOutcome::EndpointFailure("over-bound frame".into());
            }
            FrameRead::Payload(p) => match classify(&p) {
                Control::Known(Family::AttachAck, b, it) => match AttachAck::decode(&b, &it) {
                    Some(a) if (1..=3600).contains(&a.interval) => break a,
                    _ => log.push(Event::Discarded),
                },
                Control::Unknown(t) => log.push(Event::Skipped { frame_type: t }),
                Control::Malformed => log.push(Event::Discarded),
                Control::Known(..) => return AttachOutcome::EndpointFailure("known frame before ack".into()),
            },
        }
    };
    log.push(Event::Received { frame_type: FRAME_ATTACH_ACK });
    log.push(Event::Attached { mode: ack.mode });
    *cfg.sibling_cache.lock().unwrap() = ack.siblings.clone();
    let (dtx, drx) = mpsc::unbounded_channel();
    let dconn = conn.clone();
    let dlog = log.clone();
    tokio::spawn(async move {
        while let Ok(mut s) = dconn.accept_uni().await {
            if let Ok(bytes) = s.read_to_end(1 << 20).await {
                dlog.push(Event::Delivered { bytes: bytes.clone() });
                let _ = dtx.send(bytes);
            }
        }
    });
    let reach = Arc::new(Mutex::new(Reachability::Reachable));
    let interval = Duration::from_secs(ack.interval);
    let cache = cfg.sibling_cache.clone();
    let me = cfg.identity.public.keyhash;
    let loop_log = log.clone();
    let loop_reach = reach.clone();
    let task_log = log.clone();
    let task_conn = conn.clone();
    let task = tokio::spawn(async move {
        let _ = control_loop(sender, recv, interval, loop_log.clone(), loop_reach.clone(), move |fam, b, it| match fam {
            Family::SiblingUpdate => {
                let Item::Map(m) = it else { return true };
                match decode_sibling_list(b, map_get(m, 1)) {
                    Some(list) if valid_sibling_list(&list, &me) => *cache.lock().unwrap() = list,
                    _ => loop_log.push(Event::Discarded),
                }
                true
            }
            Family::Attach | Family::AttachAck => false,
            _ => true,
        })
        .await;
        task_conn.close(VarInt::from_u32(0), b"");
        task_log.push(Event::Closed);
    });
    AttachOutcome::Attached(Session { conn, ack, deliveries: drx, log, reach, task })
}

/// Try a node's endpoints as alternatives (`light-client-requirements.md`
/// §4): an endpoint failure moves to the next address; a refusal is the
/// node's answer and ends the attempt (§8.2).
pub async fn attach_any(cfg: &ClientConfig, endpoint: &quinn::Endpoint, target: [u8; 32], addrs: &[std::net::SocketAddr], early: bool) -> AttachOutcome {
    let mut last = AttachOutcome::EndpointFailure("no endpoints".into());
    for addr in addrs {
        match attach(cfg, endpoint, target, *addr, early).await {
            AttachOutcome::Attached(s) => return AttachOutcome::Attached(s),
            AttachOutcome::Refused => return AttachOutcome::Refused,
            other => last = other,
        }
    }
    last
}

/// §8.2: a list naming the receiver or holding duplicates is malformed.
fn valid_sibling_list(list: &[SiblingRef], me: &[u8; 32]) -> bool {
    let mut seen = HashSet::new();
    list.iter().all(|s| s.keyhash != *me && seen.insert(s.keyhash))
}

impl Session {
    /// Wait until the peer is judged unreachable (three missed intervals),
    /// then try the cached siblings in order (§8.2).  Returns the sibling
    /// session or the outcome of the last attempt.
    pub async fn failover(&mut self, cfg: &ClientConfig, endpoint: &quinn::Endpoint) -> AttachOutcome {
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
            let addrs = cfg.addresses.lock().unwrap().get(&s.keyhash).cloned().unwrap_or_default();
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
