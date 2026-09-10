//! A datagram-level path: a UDP relay between a client and a server, with
//! per-direction controls.
//!
//! The client dials [`Path::addr`] instead of the server's own address; the
//! server sees the path's outbound socket as the client's address.  Nothing
//! above the socket knows it is there.

use std::net::SocketAddr;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Mutex;
use tokio::net::UdpSocket;

/// What one direction does to the datagrams crossing it.
#[derive(Default)]
pub struct Direction {
    blackhole: AtomicBool,
    /// Drop one datagram in every `n`; 0 means none.
    drop_one_in: AtomicU64,
    /// Delay each datagram by this many milliseconds.
    delay_ms: AtomicU64,
    /// Keep a copy of each datagram while capturing.
    capturing: AtomicBool,
    captured: Mutex<Vec<Vec<u8>>>,
    passed: AtomicU64,
    dropped: AtomicU64,
}

impl Direction {
    pub fn blackhole(&self, on: bool) {
        self.blackhole.store(on, Ordering::SeqCst);
    }
    pub fn is_blackholed(&self) -> bool {
        self.blackhole.load(Ordering::SeqCst)
    }
    pub fn drop_one_in(&self, n: u64) {
        self.drop_one_in.store(n, Ordering::SeqCst);
    }
    pub fn delay(&self, ms: u64) {
        self.delay_ms.store(ms, Ordering::SeqCst);
    }
    pub fn capture(&self, on: bool) {
        self.capturing.store(on, Ordering::SeqCst);
    }
    pub fn captured(&self) -> Vec<Vec<u8>> {
        self.captured.lock().unwrap().clone()
    }
    pub fn clear_captured(&self) {
        self.captured.lock().unwrap().clear();
    }
    pub fn passed(&self) -> u64 {
        self.passed.load(Ordering::SeqCst)
    }
    pub fn dropped(&self) -> u64 {
        self.dropped.load(Ordering::SeqCst)
    }

    /// Decide one datagram: `None` drops it, `Some(delay)` passes it.
    fn admit(&self, datagram: &[u8]) -> Option<u64> {
        if self.capturing.load(Ordering::SeqCst) {
            self.captured.lock().unwrap().push(datagram.to_vec());
        }
        if self.blackhole.load(Ordering::SeqCst) {
            self.dropped.fetch_add(1, Ordering::SeqCst);
            return None;
        }
        let n = self.drop_one_in.load(Ordering::SeqCst);
        let seen = self.passed.load(Ordering::SeqCst) + self.dropped.load(Ordering::SeqCst);
        if n > 0 && seen % n == 0 {
            self.dropped.fetch_add(1, Ordering::SeqCst);
            return None;
        }
        self.passed.fetch_add(1, Ordering::SeqCst);
        Some(self.delay_ms.load(Ordering::SeqCst))
    }
}

/// A path between a dialling party and a server.
pub struct Path {
    /// The address the client dials.
    pub addr: SocketAddr,
    /// Client to server.
    pub to_server: Arc<Direction>,
    /// Server to client.
    pub to_client: Arc<Direction>,
    target: SocketAddr,
    task: tokio::task::JoinHandle<()>,
}

impl Drop for Path {
    fn drop(&mut self) {
        self.task.abort();
    }
}

impl Path {
    /// Open a path to `target`.  Both sockets are on loopback.
    pub async fn open(target: SocketAddr) -> std::io::Result<Path> {
        let front = UdpSocket::bind("127.0.0.1:0").await?;
        let addr = front.local_addr()?;
        let back = UdpSocket::bind("127.0.0.1:0").await?;
        let front = Arc::new(front);
        let back = Arc::new(back);
        let to_server: Arc<Direction> = Arc::default();
        let to_client: Arc<Direction> = Arc::default();
        let (f, b) = (front.clone(), back.clone());
        let (ts, tc) = (to_server.clone(), to_client.clone());
        let task = tokio::spawn(async move {
            let peer: Arc<Mutex<Option<SocketAddr>>> = Arc::default();
            let mut from_client = [0u8; 2048];
            let mut from_server = [0u8; 2048];
            loop {
                tokio::select! {
                    r = f.recv_from(&mut from_client) => {
                        let Ok((n, src)) = r else { continue };
                        *peer.lock().unwrap() = Some(src);
                        if let Some(delay) = ts.admit(&from_client[..n]) {
                            let datagram = from_client[..n].to_vec();
                            let b = b.clone();
                            tokio::spawn(async move {
                                if delay > 0 {
                                    tokio::time::sleep(std::time::Duration::from_millis(delay)).await;
                                }
                                let _ = b.send_to(&datagram, target).await;
                            });
                        }
                    }
                    r = b.recv_from(&mut from_server) => {
                        let Ok((n, _)) = r else { continue };
                        let Some(dst) = *peer.lock().unwrap() else { continue };
                        if let Some(delay) = tc.admit(&from_server[..n]) {
                            let datagram = from_server[..n].to_vec();
                            let f = f.clone();
                            tokio::spawn(async move {
                                if delay > 0 {
                                    tokio::time::sleep(std::time::Duration::from_millis(delay)).await;
                                }
                                let _ = f.send_to(&datagram, dst).await;
                            });
                        }
                    }
                }
            }
        });
        Ok(Path { addr, to_server, to_client, target, task })
    }

    /// Blackhole both directions: the two parties can send and neither
    /// arrives, which is what a partition looks like from inside.
    pub fn partition(&self, on: bool) {
        self.to_server.blackhole(on);
        self.to_client.blackhole(on);
    }

    /// Replay captured client datagrams to the server from a fresh source
    /// address, as an attacker holding a recording would.  Returns how many
    /// were sent.
    pub async fn replay_to_server(&self, datagrams: &[Vec<u8>]) -> std::io::Result<usize> {
        let sock = UdpSocket::bind("127.0.0.1:0").await?;
        let mut sent = 0;
        for d in datagrams {
            sock.send_to(d, self.target).await?;
            sent += 1;
        }
        // hold the socket open long enough for any answer to arrive
        tokio::time::sleep(std::time::Duration::from_millis(200)).await;
        Ok(sent)
    }
}
