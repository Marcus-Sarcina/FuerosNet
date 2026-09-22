//! The socket the direct payload path runs on (design §14.1.1, §12.6.3;
//! `wire-format.md` §9.2): one UDP socket carrying QUIC, with STUN told
//! apart on the way in.  As a serving node's socket it answers Binding
//! Requests, so the node is a STUN server at the address it already
//! serves QUIC on; as a client's socket it asks, so the reflexive address
//! it learns is the one the connection it then opens will be seen at.
//! Nothing above the socket knows either happened.
//!
//! For a harness, the socket can also sit behind an emulated NAT
//! (`rhtn-sim`'s): every datagram out is wrapped with its real destination
//! and sent to the NAT, every datagram in arrives wrapped with its real
//! source.  What the client and the NAT agree on is that framing and
//! nothing else, so the socket's users see the addresses they would see
//! on a real network.

use crate::stun;
use quinn::udp::{RecvMeta, Transmit};
use quinn::{AsyncUdpSocket, Runtime, TokioRuntime, UdpPoller};
use std::collections::HashMap;
use std::io::{self, IoSliceMut};
use std::net::{IpAddr, SocketAddr};
use std::pin::Pin;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll};
use tokio::sync::oneshot;

/// The socket, wrapping the runtime's own.
pub struct TraversalSocket {
    inner: Arc<dyn AsyncUdpSocket>,
    /// Binding Requests answered here: the STUN server role.
    pub answered: AtomicU64,
    /// Binding Requests sent from here: the client role.
    pub asked: AtomicU64,
    pending: Mutex<HashMap<[u8; 12], oneshot::Sender<SocketAddr>>>,
    /// The emulated NAT this socket sits behind, if any.
    nat: Option<SocketAddr>,
}

impl std::fmt::Debug for TraversalSocket {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "TraversalSocket({:?})", self.inner.local_addr())
    }
}

/// The harness framing between a socket and its NAT: the address first,
/// then the datagram.
pub fn wrap(addr: SocketAddr, payload: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(19 + payload.len());
    match addr.ip() {
        IpAddr::V4(ip) => {
            out.push(4);
            out.extend_from_slice(&ip.octets());
        }
        IpAddr::V6(ip) => {
            out.push(6);
            out.extend_from_slice(&ip.octets());
        }
    }
    out.extend_from_slice(&addr.port().to_be_bytes());
    out.extend_from_slice(payload);
    out
}

pub fn unwrap(b: &[u8]) -> Option<(SocketAddr, &[u8])> {
    match b.first()? {
        4 if b.len() >= 7 => {
            let ip: [u8; 4] = b[1..5].try_into().ok()?;
            let port = u16::from_be_bytes([b[5], b[6]]);
            Some((SocketAddr::new(IpAddr::V4(ip.into()), port), &b[7..]))
        }
        6 if b.len() >= 19 => {
            let ip: [u8; 16] = b[1..17].try_into().ok()?;
            let port = u16::from_be_bytes([b[17], b[18]]);
            Some((SocketAddr::new(IpAddr::V6(ip.into()), port), &b[19..]))
        }
        _ => None,
    }
}

impl TraversalSocket {
    /// Bind at `addr`, behind `nat` where the harness says so.
    pub fn bind(addr: SocketAddr, nat: Option<SocketAddr>) -> io::Result<Arc<Self>> {
        let sock = std::net::UdpSocket::bind(addr)?;
        sock.set_nonblocking(true)?;
        let inner = TokioRuntime.wrap_udp_socket(sock)?;
        Ok(Arc::new(TraversalSocket {
            inner,
            answered: AtomicU64::new(0),
            asked: AtomicU64::new(0),
            pending: Mutex::new(HashMap::new()),
            nat,
        }))
    }

    /// The socket's own address.
    pub fn addr(&self) -> io::Result<SocketAddr> {
        self.inner.local_addr()
    }

    fn send_raw(&self, to: SocketAddr, bytes: &[u8]) -> io::Result<()> {
        match self.nat {
            Some(nat) => {
                let wrapped = wrap(to, bytes);
                self.inner.try_send(&Transmit {
                    destination: nat,
                    ecn: None,
                    contents: &wrapped,
                    segment_size: None,
                    src_ip: None,
                })
            }
            None => self.inner.try_send(&Transmit {
                destination: to,
                ecn: None,
                contents: bytes,
                segment_size: None,
                src_ip: None,
            }),
        }
    }

    /// Ask `server` what address it sees this socket at (RFC 8489 §5): the
    /// server-reflexive candidate.  Times out where nothing answers.
    pub async fn reflexive(
        self: &Arc<Self>,
        server: SocketAddr,
        timeout: std::time::Duration,
    ) -> io::Result<SocketAddr> {
        let txid: [u8; 12] = crate::tls::random_bytes();
        let (tx, rx) = oneshot::channel();
        self.pending.lock().unwrap().insert(txid, tx);
        self.asked.fetch_add(1, Ordering::SeqCst);
        // the request may need the socket writable; retry briefly
        let req = stun::binding_request(&txid);
        let deadline = tokio::time::Instant::now() + timeout;
        loop {
            match self.send_raw(server, &req) {
                Ok(()) => break,
                Err(e) if e.kind() == io::ErrorKind::WouldBlock => {
                    tokio::time::sleep(std::time::Duration::from_millis(5)).await
                }
                Err(e) => {
                    self.pending.lock().unwrap().remove(&txid);
                    return Err(e);
                }
            }
            if tokio::time::Instant::now() > deadline {
                self.pending.lock().unwrap().remove(&txid);
                return Err(io::Error::new(
                    io::ErrorKind::TimedOut,
                    "socket not writable",
                ));
            }
        }
        match tokio::time::timeout_at(deadline, rx).await {
            Ok(Ok(addr)) => Ok(addr),
            _ => {
                self.pending.lock().unwrap().remove(&txid);
                Err(io::Error::new(
                    io::ErrorKind::TimedOut,
                    "no binding response",
                ))
            }
        }
    }

    /// One datagram in: a STUN request is answered and consumed, a STUN
    /// response settles the ask it answers and is consumed, anything else
    /// is QUIC and passes.  Returns whether the datagram passes.
    fn take(&self, from: SocketAddr, datagram: &[u8]) -> bool {
        if !stun::is_stun(datagram) {
            return true;
        }
        match stun::parse(datagram) {
            Some(stun::Parsed {
                kind: stun::Binding::Request,
                txid,
                ..
            }) => {
                self.answered.fetch_add(1, Ordering::SeqCst);
                let _ = self.send_raw(from, &stun::binding_response(&txid, from));
            }
            Some(stun::Parsed {
                kind: stun::Binding::Response,
                txid,
                mapped: Some(addr),
            }) => {
                if let Some(tx) = self.pending.lock().unwrap().remove(&txid) {
                    let _ = tx.send(addr);
                }
            }
            _ => {}
        }
        false
    }
}

impl AsyncUdpSocket for TraversalSocket {
    fn create_io_poller(self: Arc<Self>) -> Pin<Box<dyn UdpPoller>> {
        self.inner.clone().create_io_poller()
    }

    fn try_send(&self, transmit: &Transmit) -> io::Result<()> {
        match self.nat {
            Some(nat) => {
                let wrapped = wrap(transmit.destination, transmit.contents);
                self.inner.try_send(&Transmit {
                    destination: nat,
                    ecn: transmit.ecn,
                    contents: &wrapped,
                    segment_size: None,
                    src_ip: None,
                })
            }
            None => self.inner.try_send(transmit),
        }
    }

    fn poll_recv(
        &self,
        cx: &mut Context,
        bufs: &mut [IoSliceMut<'_>],
        meta: &mut [RecvMeta],
    ) -> Poll<io::Result<usize>> {
        loop {
            let n = match self.inner.poll_recv(cx, bufs, meta) {
                Poll::Ready(Ok(n)) => n,
                other => return other,
            };
            let mut kept = 0;
            for i in 0..n {
                let m = meta[i];
                // **One buffer may carry several datagrams**, `stride`
                // bytes each with the last possibly short: that is what
                // the kernel's receive offload does, and what `stride`
                // says.  Flattening them into one loses every packet after
                // the first, and the sender recovers only by retransmitting
                // on a backoff — which is minutes, not milliseconds, once
                // a message runs to a few thousand bytes.
                let stride = if m.stride == 0 {
                    m.len.max(1)
                } else {
                    m.stride
                };
                let coalesced = m.len > stride;
                let plain = self.nat.is_none();
                // the fast path, and the only one a node on the real
                // network takes: nothing to unwrap, and no STUN in the
                // batch, so it passes exactly as it arrived and the
                // receiver splits it by the stride it came with
                if plain
                    && !(0..m.len)
                        .step_by(stride)
                        .any(|o| stun::is_stun(&bufs[i][o..(o + stride).min(m.len)]))
                {
                    if kept != i {
                        let body = bufs[i][..m.len].to_vec();
                        bufs[kept][..body.len()].copy_from_slice(&body);
                    }
                    meta[kept] = m;
                    kept += 1;
                    continue;
                }
                // something in this batch is STUN, or the harness wrapped
                // it: take it apart datagram by datagram and put back what
                // passes, contiguously and at one stride
                let mut out: Vec<u8> = Vec::with_capacity(m.len);
                let mut from: Option<SocketAddr> = None;
                let mut width = 0usize;
                for off in (0..m.len).step_by(stride) {
                    let raw = bufs[i][off..(off + stride).min(m.len)].to_vec();
                    let (src, body) = match self.nat {
                        Some(_) => match unwrap(&raw) {
                            Some((s, p)) => (s, p.to_vec()),
                            None => continue,
                        },
                        None => (m.addr, raw),
                    };
                    if !self.take(src, &body) {
                        continue;
                    }
                    // one `RecvMeta` names one source, so a batch that
                    // unwrapped to two of them yields the first source's
                    // datagrams and drops the rest, which is a loss the
                    // sender repairs and cannot be a wrong delivery
                    match from {
                        None => from = Some(src),
                        Some(f) if f == src => {}
                        Some(_) => continue,
                    }
                    width = width.max(body.len());
                    out.extend_from_slice(&body);
                }
                let Some(from) = from.filter(|_| !out.is_empty()) else {
                    continue;
                };
                bufs[kept][..out.len()].copy_from_slice(&out);
                meta[kept] = RecvMeta {
                    addr: from,
                    len: out.len(),
                    stride: width,
                    ecn: m.ecn,
                    dst_ip: m.dst_ip,
                };
                kept += 1;
                let _ = coalesced;
            }
            if kept > 0 {
                return Poll::Ready(Ok(kept));
            }
            // everything was STUN or unwrappable: poll again, without
            // pretending a datagram arrived
        }
    }

    fn local_addr(&self) -> io::Result<SocketAddr> {
        self.inner.local_addr()
    }

    fn max_transmit_segments(&self) -> usize {
        1
    }

    /// What the inner socket's receive offload may coalesce into one
    /// buffer, which this socket passes through rather than flattening.
    fn max_receive_segments(&self) -> usize {
        self.inner.max_receive_segments()
    }

    fn may_fragment(&self) -> bool {
        self.inner.may_fragment()
    }
}

/// A quinn endpoint on a traversal socket: a server's, answering STUN at
/// the address it serves QUIC on, or a client's, able to ask.
pub fn endpoint(
    socket: Arc<TraversalSocket>,
    server: Option<quinn::ServerConfig>,
) -> io::Result<quinn::Endpoint> {
    quinn::Endpoint::new_with_abstract_socket(
        quinn::EndpointConfig::default(),
        server,
        socket,
        Arc::new(TokioRuntime),
    )
}

// ------------------------------------------------------------ candidates

/// A candidate address for the direct path (RFC 8445 §5.1.1): the socket's
/// own address, or the address the serving node saw it at.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CandidateKind {
    Host = 0,
    ServerReflexive = 1,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Candidate {
    pub kind: CandidateKind,
    pub addr: SocketAddr,
}

/// Candidates as they travel to the peer: an array of `[kind, ip, port]`.
pub fn encode_candidates(cs: &[Candidate]) -> Vec<u8> {
    use rhtn_codec::encode::*;
    let mut out = Vec::new();
    emit_array_head(&mut out, cs.len());
    for c in cs {
        emit_array_head(&mut out, 3);
        emit_uint(&mut out, c.kind as u64);
        match c.addr.ip() {
            IpAddr::V4(ip) => emit_bstr(&mut out, &ip.octets()),
            IpAddr::V6(ip) => emit_bstr(&mut out, &ip.octets()),
        }
        emit_uint(&mut out, c.addr.port() as u64);
    }
    out
}

pub fn decode_candidates(b: &[u8]) -> Result<Vec<Candidate>, String> {
    use rhtn_codec::cbor::*;
    let item = parse_all(b).map_err(|e| e.0)?;
    let Item::Array(list) = &item else {
        return Err("candidates not an array".into());
    };
    let mut out = Vec::new();
    for c in list {
        let Item::Array(f) = c else {
            return Err("candidate not an array".into());
        };
        if f.len() != 3 {
            return Err("candidate has three fields".into());
        }
        let kind = match as_uint(&f[0]) {
            Some(0) => CandidateKind::Host,
            Some(1) => CandidateKind::ServerReflexive,
            _ => return Err("candidate kind".into()),
        };
        let ip = match &f[1] {
            Item::Bytes(r) if r.len() == 4 => {
                IpAddr::V4(<[u8; 4]>::try_from(&b[r.clone()]).unwrap().into())
            }
            Item::Bytes(r) if r.len() == 16 => {
                IpAddr::V6(<[u8; 16]>::try_from(&b[r.clone()]).unwrap().into())
            }
            _ => return Err("candidate address".into()),
        };
        let port = as_uint(&f[2])
            .filter(|p| *p <= u16::MAX as u64)
            .ok_or("candidate port")? as u16;
        out.push(Candidate {
            kind,
            addr: SocketAddr::new(ip, port),
        });
    }
    Ok(out)
}

impl TraversalSocket {
    /// Gather this socket's candidates: its host address, and the
    /// server-reflexive address `stun` reports where one is given and it
    /// differs.
    pub async fn gather(
        self: &Arc<Self>,
        stun: Option<SocketAddr>,
        timeout: std::time::Duration,
    ) -> Vec<Candidate> {
        let mut out = Vec::new();
        if let Ok(host) = self.addr() {
            out.push(Candidate {
                kind: CandidateKind::Host,
                addr: host,
            });
        }
        if let Some(server) = stun
            && let Ok(seen) = self.reflexive(server, timeout).await
            && !out.iter().any(|c| c.addr == seen)
        {
            out.push(Candidate {
                kind: CandidateKind::ServerReflexive,
                addr: seen,
            });
        }
        out
    }
}

/// The connectivity check and the connection in one (RFC 8445 §7, with
/// QUIC's handshake as the check): dial every candidate of the peer at
/// once, authenticating as `me`, and keep the first handshake that
/// completes *and binds* to `peer`: by the pin, or by a delegation held
/// from the topology class, the direct path being inside the horizon
/// where that is held (design §12.6.3, `wire-format.md` §9.1).  A
/// completed handshake nothing binds is closed and counts as no path.
/// `None` within `timeout` is the direct path failing, and the relay is
/// next.
pub async fn connect_direct(
    endpoint: &quinn::Endpoint,
    me: impl Into<crate::tls::Presenter>,
    pins: &crate::tls::Pins,
    bind: &crate::bind::Binding,
    peer: &[u8; 32],
    candidates: &[Candidate],
    timeout: std::time::Duration,
) -> Option<quinn::Connection> {
    let me = me.into();
    let mut attempts = Vec::new();
    for c in candidates {
        if let Ok(connecting) = crate::tls::dial(endpoint, &me, pins, peer, c.addr) {
            let (pins, bind, peer) = (pins.clone(), bind.clone(), *peer);
            attempts.push(Box::pin(async move {
                let conn = connecting.await.ok()?;
                let presented = crate::tls::peer_key(&conn)?;
                match bind.without_frame(&pins, &peer, &presented) {
                    Ok(Some(_)) => Some(conn),
                    _ => {
                        conn.close(quinn::VarInt::from_u32(1), b"unbound");
                        None
                    }
                }
            }));
        }
    }
    if attempts.is_empty() {
        return None;
    }
    let race = async move {
        let mut pending = attempts;
        while !pending.is_empty() {
            let (result, _, rest) = futures_select(pending).await;
            if let Some(conn) = result {
                return Some(conn);
            }
            pending = rest;
        }
        None
    };
    tokio::time::timeout(timeout, race).await.ok().flatten()
}

/// `select_all` without the crate: poll every future, return the first
/// ready one with the rest.
async fn futures_select<F, T>(futs: Vec<Pin<Box<F>>>) -> (T, usize, Vec<Pin<Box<F>>>)
where
    F: std::future::Future<Output = T> + ?Sized,
{
    struct Select<F: ?Sized, T> {
        futs: Vec<Pin<Box<F>>>,
        _t: std::marker::PhantomData<T>,
    }
    impl<F: ?Sized, T> Unpin for Select<F, T> {}
    impl<F: std::future::Future<Output = T> + ?Sized, T> std::future::Future for Select<F, T> {
        type Output = (T, usize, Vec<Pin<Box<F>>>);
        fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
            let this = self.get_mut();
            for i in 0..this.futs.len() {
                if let Poll::Ready(v) = this.futs[i].as_mut().poll(cx) {
                    let mut rest = std::mem::take(&mut this.futs);
                    rest.remove(i);
                    return Poll::Ready((v, i, rest));
                }
            }
            Poll::Pending
        }
    }
    Select {
        futs,
        _t: std::marker::PhantomData,
    }
    .await
}
