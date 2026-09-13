//! The courier: what the client says goes out over the seams, and what
//! arrives on them goes in.

use crate::actor::Handle;
use crate::direct::Direct;
use crate::serving::{Inbound, Serving};
use crate::verifier::Verifiers;
use rhtn_archive::Keyhash;
use rhtn_client::ceremony::{Dispatched, Msg};
use rhtn_client::payload::KIND_CANDIDATES;
use rhtn_client::verifier::GrantOutcome;
use rhtn_transport::traversal::{decode_candidates, encode_candidates};
use std::collections::HashSet;
use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use tokio::sync::mpsc;

/// Where inbound payload goes once a courier exists: bound late, since the
/// socket that receives is built before the courier that reads.
#[derive(Clone, Default)]
pub struct Inlet(Arc<Mutex<Option<Inbound>>>);

impl Inlet {
    pub fn inbound(&self) -> Inbound {
        let slot = self.0.clone();
        Arc::new(move |from, bytes| {
            let f = slot.lock().unwrap().clone();
            if let Some(f) = f {
                f(from, bytes);
            }
        })
    }

    pub fn bind(&self, to: Inbound) {
        *self.0.lock().unwrap() = Some(to);
    }
}

/// What a [`Courier::carry`] could not complete, with the two reasons kept
/// apart.  **Both are unsent work**, and a caller that ignores either
/// reports a message delivered that is not.
#[derive(Debug, Default)]
pub struct Carried {
    /// Not the adaptors' to carry: the ceremony's own conversation between
    /// two present devices, which no document gives an encoding.
    pub left: Vec<Msg>,
    /// Handed to the serving node and refused by it.
    pub refused: Vec<Msg>,
}

impl Carried {
    fn absorb(&mut self, other: Carried) {
        self.left.extend(other.left);
        self.refused.extend(other.refused);
    }

    /// Whether everything travelled.
    pub fn complete(&self) -> bool {
        self.left.is_empty() && self.refused.is_empty()
    }
}

pub struct Courier {
    pub handle: Handle,
    pub serving: Arc<dyn Serving>,
    pub direct: Arc<dyn Direct>,
    app: mpsc::UnboundedSender<(Keyhash, Dispatched)>,
    verifiers: Mutex<Option<Arc<Verifiers>>>,
    offered: Mutex<HashSet<Keyhash>>,
}

impl Courier {
    /// A courier for `handle`, and where its application payload and the
    /// subject's copies arrive.
    pub fn new(handle: Handle, serving: Arc<dyn Serving>, direct: Arc<dyn Direct>) -> (Arc<Courier>, mpsc::UnboundedReceiver<(Keyhash, Dispatched)>) {
        let (app, rx) = mpsc::unbounded_channel();
        (Arc::new(Courier { handle, serving, direct, app, verifiers: Mutex::new(None), offered: Mutex::new(HashSet::new()) }), rx)
    }

    pub(crate) fn report_grants_to(&self, v: Arc<Verifiers>) {
        *self.verifiers.lock().unwrap() = Some(v);
    }

    pub fn me(&self) -> Keyhash {
        self.handle.me()
    }

    /// This client's inbound side, for a socket or a node to call with
    /// what arrived from a peer.
    pub fn inbound(self: &Arc<Self>) -> Inbound {
        let me = self.clone();
        Arc::new(move |from, bytes| {
            let me = me.clone();
            tokio::spawn(async move { me.receive(from, bytes).await });
        })
    }

    /// What arrived from `from` on the end-to-end channel: decrypted at the
    /// client and delivered by kind.  Candidates open the direct path, and
    /// this side offers its own if it has not; a grant that answered a
    /// waiting query completes its stream; the rest is the application's.
    pub async fn receive(self: Arc<Self>, from: Keyhash, bytes: Vec<u8>) {
        let d = self.handle.with(move |c| c.receive_payload(from, &bytes)).await;
        match d {
            Ok(Dispatched::Candidates(b)) => {
                if let Ok(cands) = decode_candidates(&b) {
                    self.direct.open(from, cands).await;
                }
                let offered = self.offered.lock().unwrap().contains(&from);
                if !offered {
                    self.offer(from).await;
                }
            }
            Ok(Dispatched::Grant(GrantOutcome::Answered(a))) => {
                let v = self.verifiers.lock().unwrap().clone();
                if let Some(v) = v {
                    v.granted(a);
                }
            }
            Ok(Dispatched::Grant(_)) => {}
            Ok(other) => {
                let _ = self.app.send((from, other));
            }
            // undecryptable: dropped, and nothing said to anyone
            Err(_) => {}
        }
    }

    /// Attach to the serving node beside this client: what the client
    /// publishes, stocks and sweeps at attach, carried there.
    pub async fn attach(self: &Arc<Self>, population: Vec<Keyhash>) -> Carried {
        let serving = self.serving.me();
        let msgs = self.handle.with(move |c| c.attach(serving, &population)).await;
        self.carry(msgs).await
    }

    /// Sweep the population's reusable material again
    /// (`light-client-requirements.md` §3).  A sweep is a snapshot of what
    /// was published when it ran, so a client that must attribute an
    /// initial message from a peer who attached later sweeps again.
    pub async fn sweep(self: &Arc<Self>, population: Vec<Keyhash>) -> Carried {
        let msgs = self.handle.with(move |c| c.sweep(&population)).await;
        self.carry(msgs).await
    }

    /// Offer `peer` the direct path: this side's candidates, gathered only
    /// where the path may be direct, sent as their own kind on whatever
    /// path exists now.  Whether an offer went.
    pub async fn offer(self: &Arc<Self>, peer: Keyhash) -> bool {
        let Some(cands) = self.direct.gather(peer).await else { return false };
        self.offered.lock().unwrap().insert(peer);
        self.send(peer, KIND_CANDIDATES, encode_candidates(&cands)).await.is_ok()
    }

    /// Send `bytes` of `kind` to `to` from the client, and carry what the
    /// client says out.  What the adaptors do not carry comes back.
    pub async fn send(self: &Arc<Self>, to: Keyhash, kind: u64, bytes: Vec<u8>) -> Result<Carried, String> {
        let msgs = self.handle.with(move |c| c.send_payload(to, kind, &bytes)).await.map_err(|e| e.to_string())?;
        Ok(self.carry(msgs).await)
    }

    /// Carry the client's messages where the seams go: prekey traffic to
    /// the serving node, payload on the direct path where it is held and
    /// to the relay otherwise.
    ///
    /// **A refusal comes back.**  A serving node that will not take a
    /// submission answers with a code (`wire-format.md` §7.10), and a
    /// caller told nothing would report work done that was not
    /// (`light-client-requirements.md` §9).  The two ways a message can
    /// fail to travel are kept apart: `left` is what the adaptors do not
    /// carry at all, the ceremony's own conversation between two present
    /// devices, and `refused` is what was handed over and turned down.
    pub fn carry(self: &Arc<Self>, msgs: Vec<Msg>) -> Pin<Box<dyn Future<Output = Carried> + Send>> {
        let me = self.clone();
        Box::pin(async move {
            let mut out = Carried::default();
            for m in msgs {
                match m {
                    Msg::PublishBundle(b) => {
                        if !me.serving.publish(&b).await {
                            out.refused.push(Msg::PublishBundle(b));
                        }
                    }
                    Msg::StockOneTime(keys) => {
                        if !me.serving.stock(me.me(), keys.clone()).await {
                            out.refused.push(Msg::StockOneTime(keys));
                        }
                    }
                    Msg::PrekeyRequest(b) => {
                        if let Some(reply) = me.serving.prekey(me.me(), &b).await {
                            let more = me.handle.with(move |c| c.take_prekey_reply(&reply)).await;
                            if let Ok(more) = more {
                                out.absorb(me.carry(more).await);
                            }
                        }
                    }
                    Msg::Payload { to, bytes } => {
                        // a delivery short of complete leaves the message
                        // with the sender, and the relay carries it (design
                        // §14.1.1)
                        if !me.direct.deliver(to, bytes.clone()).await && !me.serving.relay(me.me(), to, bytes.clone()).await {
                            out.refused.push(Msg::Payload { to, bytes });
                        }
                    }
                    Msg::Relay { to, bytes } => {
                        if !me.serving.relay(me.me(), to, bytes.clone()).await {
                            out.refused.push(Msg::Relay { to, bytes });
                        }
                    }
                    Msg::Transport(b) => {
                        let node = me.serving.me();
                        if !me.serving.relay(me.me(), node, b.clone()).await {
                            out.refused.push(Msg::Transport(b));
                        }
                    }
                    other => out.left.push(other),
                }
            }
            out
        })
    }
}
