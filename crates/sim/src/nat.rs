//! An emulated NAT on loopback (design §14.1.1: address- and port-dependent
//! mapping defeats hole punching; carrier-grade NAT raises the odds of
//! meeting it).  A client's traversal socket sends every datagram to the
//! NAT wrapped with its real destination; the NAT sends it on from an
//! external socket chosen by its mapping behaviour, and hands back what
//! arrives there, wrapped with its real source, where its filtering
//! behaviour admits it (RFC 4787's two axes).  Two clients behind two of
//! these, with a STUN server and each other on the far side, see what two
//! phones see.

use rhtn_transport::traversal::{unwrap, wrap};
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use tokio::net::UdpSocket;

/// How an external mapping is chosen (RFC 4787 §4.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mapping {
    /// One external port per inside socket, whatever the destination: the
    /// mapping a STUN server reports is the one a peer can reach.
    EndpointIndependent,
    /// A fresh external port per destination address and port: what the
    /// STUN server reports is not what a peer would reach.
    AddressAndPortDependent,
}

/// What inbound datagrams an external mapping admits (RFC 4787 §5).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Filtering {
    /// Anything, once the mapping exists.
    EndpointIndependent,
    /// Only from an address the inside socket has sent to.
    AddressDependent,
    /// Only from an address and port the inside socket has sent to.
    AddressAndPortDependent,
}

#[derive(Default)]
struct State {
    /// External sockets by mapping key: (inside, destination or none).
    external: HashMap<(SocketAddr, Option<SocketAddr>), Arc<UdpSocket>>,
    /// Destinations each inside socket has sent to.
    sent_to: HashMap<SocketAddr, Vec<SocketAddr>>,
}

pub struct Nat {
    pub mapping: Mapping,
    pub filtering: Filtering,
    /// Where inside sockets send their wrapped datagrams.
    pub inside: SocketAddr,
    inside_sock: Arc<UdpSocket>,
    state: Arc<Mutex<State>>,
    pub filtered: AtomicU64,
    pub forwarded: AtomicU64,
    task: tokio::task::JoinHandle<()>,
}

impl Drop for Nat {
    fn drop(&mut self) {
        self.task.abort();
    }
}

impl Nat {
    pub async fn start(mapping: Mapping, filtering: Filtering) -> std::io::Result<Arc<Nat>> {
        let inside_sock = Arc::new(UdpSocket::bind("127.0.0.1:0").await?);
        let inside = inside_sock.local_addr()?;
        let state: Arc<Mutex<State>> = Arc::default();
        let nat = Arc::new_cyclic(|weak: &std::sync::Weak<Nat>| {
            let (sock, st, w) = (inside_sock.clone(), state.clone(), weak.clone());
            let task = tokio::spawn(async move {
                let mut buf = [0u8; 4096];
                loop {
                    let Ok((n, from)) = sock.recv_from(&mut buf).await else {
                        continue;
                    };
                    let Some(nat) = w.upgrade() else { return };
                    let Some((dest, payload)) = unwrap(&buf[..n]) else {
                        continue;
                    };
                    let payload = payload.to_vec();
                    let ext = nat.external_for(from, dest).await;
                    st.lock()
                        .unwrap()
                        .sent_to
                        .entry(from)
                        .or_default()
                        .push(dest);
                    nat.forwarded.fetch_add(1, Ordering::SeqCst);
                    let _ = ext.send_to(&payload, dest).await;
                }
            });
            Nat {
                mapping,
                filtering,
                inside,
                inside_sock: inside_sock.clone(),
                state,
                filtered: AtomicU64::new(0),
                forwarded: AtomicU64::new(0),
                task,
            }
        });
        Ok(nat)
    }

    /// The external socket for `inside` sending to `dest`, opened on first
    /// use with a reader that hands inbound datagrams back inside.
    async fn external_for(
        self: &Arc<Self>,
        inside: SocketAddr,
        dest: SocketAddr,
    ) -> Arc<UdpSocket> {
        let key = match self.mapping {
            Mapping::EndpointIndependent => (inside, None),
            Mapping::AddressAndPortDependent => (inside, Some(dest)),
        };
        if let Some(s) = self.state.lock().unwrap().external.get(&key) {
            return s.clone();
        }
        let ext = Arc::new(
            UdpSocket::bind("127.0.0.1:0")
                .await
                .expect("an external port"),
        );
        self.state.lock().unwrap().external.insert(key, ext.clone());
        let (nat, sock) = (self.clone(), ext.clone());
        tokio::spawn(async move {
            let mut buf = [0u8; 4096];
            loop {
                let Ok((n, src)) = sock.recv_from(&mut buf).await else {
                    return;
                };
                let admitted = {
                    let st = nat.state.lock().unwrap();
                    let sent = st.sent_to.get(&inside).cloned().unwrap_or_default();
                    match nat.filtering {
                        Filtering::EndpointIndependent => true,
                        Filtering::AddressDependent => sent.iter().any(|d| d.ip() == src.ip()),
                        Filtering::AddressAndPortDependent => sent.contains(&src),
                    }
                };
                if !admitted {
                    nat.filtered.fetch_add(1, Ordering::SeqCst);
                    continue;
                }
                let _ = nat.inside_sock.send_to(&wrap(src, &buf[..n]), inside).await;
            }
        });
        ext
    }

    /// The external addresses in use, for a test to count mappings.  The
    /// order is the map's and means nothing: a caller that wants one
    /// socket's mapping asks for it by inside address.
    pub fn mappings(&self) -> Vec<SocketAddr> {
        self.state
            .lock()
            .unwrap()
            .external
            .values()
            .filter_map(|s| s.local_addr().ok())
            .collect()
    }

    /// The external addresses this NAT holds for one inside socket: one
    /// under endpoint-independent mapping, and one per destination under
    /// address-and-port-dependent mapping (RFC 4787 §4.1).
    pub fn mappings_for(&self, inside: SocketAddr) -> Vec<SocketAddr> {
        let st = self.state.lock().unwrap();
        st.external
            .iter()
            .filter(|((i, _), _)| *i == inside)
            .filter_map(|(_, s)| s.local_addr().ok())
            .collect()
    }
}
