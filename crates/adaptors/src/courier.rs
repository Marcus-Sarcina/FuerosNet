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
        Arc::new(move |from, bytes, binding| {
            let f = slot.lock().unwrap().clone();
            if let Some(f) = f {
                f(from, bytes, binding);
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
    /// Whether the relay carries what the direct path does not.  Off when
    /// the person chose the direct path alone (`light-client-requirements.md`
    /// §5): a message with no direct path is then unsent, and said so.
    relay_allowed: std::sync::atomic::AtomicBool,
    /// Whether a held direct path carries payload.  Off when the person
    /// chose the relay alone: what is held is not used, and nothing new is
    /// gathered, which the gate decides.
    direct_allowed: std::sync::atomic::AtomicBool,
}

impl Courier {
    /// A courier for `handle`, and where its application payload and the
    /// subject's copies arrive.
    pub fn new(
        handle: Handle,
        serving: Arc<dyn Serving>,
        direct: Arc<dyn Direct>,
    ) -> (Arc<Courier>, mpsc::UnboundedReceiver<(Keyhash, Dispatched)>) {
        let (app, rx) = mpsc::unbounded_channel();
        (
            Arc::new(Courier {
                handle,
                serving,
                direct,
                app,
                verifiers: Mutex::new(None),
                relay_allowed: std::sync::atomic::AtomicBool::new(true),
                direct_allowed: std::sync::atomic::AtomicBool::new(true),
                offered: Mutex::new(HashSet::new()),
            }),
            rx,
        )
    }

    pub(crate) fn report_grants_to(&self, v: Arc<Verifiers>) {
        *self.verifiers.lock().unwrap() = Some(v);
    }

    pub fn me(&self) -> Keyhash {
        self.handle.me()
    }

    /// Whether the relay may carry payload: the person's override, in the
    /// direction that forbids the serving node the communication graph.
    pub fn set_relay_allowed(&self, allowed: bool) {
        self.relay_allowed
            .store(allowed, std::sync::atomic::Ordering::Relaxed);
    }

    fn relay_allowed(&self) -> bool {
        self.relay_allowed
            .load(std::sync::atomic::Ordering::Relaxed)
    }

    /// Whether a held direct path may carry payload: the person's override
    /// in the direction that keeps this device's address from a peer.
    pub fn set_direct_allowed(&self, allowed: bool) {
        self.direct_allowed
            .store(allowed, std::sync::atomic::Ordering::Relaxed);
    }

    fn direct_allowed(&self) -> bool {
        self.direct_allowed
            .load(std::sync::atomic::Ordering::Relaxed)
    }

    /// Whether `peer` has been offered the direct path from here.
    pub fn offered(&self, peer: &Keyhash) -> bool {
        self.offered.lock().unwrap().contains(peer)
    }

    /// This client's inbound side, for a socket or a node to call with
    /// what arrived from a peer.
    pub fn inbound(self: &Arc<Self>) -> Inbound {
        let me = self.clone();
        Arc::new(move |from, bytes, binding| {
            let me = me.clone();
            tokio::spawn(async move { me.receive(from, bytes, binding).await });
        })
    }

    /// What arrived from `from` on the end-to-end channel: decrypted at the
    /// client and delivered by kind.  Candidates open the direct path, and
    /// this side offers its own if it has not; a grant that answered a
    /// waiting query completes its stream; the rest is the application's.
    pub async fn receive(self: Arc<Self>, from: Keyhash, bytes: Vec<u8>, binding: Option<Vec<u8>>) {
        // The binding the node carried is taken first, so an initial
        // message from a peer this client has never held one for is
        // attributable on arrival rather than a message too late
        // (`wire-format.md` §7.10).
        let d = self
            .handle
            .with(move |c| {
                if let Some(b) = binding {
                    c.take_binding(&b);
                }
                c.receive_payload(from, &bytes)
            })
            .await;
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
        let msgs = self
            .handle
            .with(move |c| c.attach(serving, &population))
            .await;
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
        let Some(cands) = self.direct.gather(peer).await else {
            return false;
        };
        self.offered.lock().unwrap().insert(peer);
        self.send_now(peer, KIND_CANDIDATES, encode_candidates(&cands))
            .await
            .is_ok()
    }

    /// Send `bytes` of `kind` to `to` from the client, and carry what the
    /// client says out.  What the adaptors do not carry comes back.
    pub async fn send(
        self: &Arc<Self>,
        to: Keyhash,
        kind: u64,
        bytes: Vec<u8>,
    ) -> Result<Carried, String> {
        // the direct path is attempted first and the relay on failure
        // (design §14.1.1): a peer not yet offered candidates is offered
        // them on the first send, where the path may be direct at all
        if !self.offered(&to) {
            self.offer(to).await;
        }
        self.send_now(to, kind, bytes).await
    }

    /// Send with no offer first: what an offer itself rides on.
    async fn send_now(
        self: &Arc<Self>,
        to: Keyhash,
        kind: u64,
        bytes: Vec<u8>,
    ) -> Result<Carried, String> {
        let msgs = self
            .handle
            .with(move |c| c.send_payload(to, kind, &bytes))
            .await
            .map_err(|e| e.to_string())?;
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
    pub fn carry(
        self: &Arc<Self>,
        msgs: Vec<Msg>,
    ) -> Pin<Box<dyn Future<Output = Carried> + Send>> {
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
                    Msg::Payload { to, bytes, device } => {
                        // a delivery short of complete leaves the message
                        // with the sender, and the relay carries it (design
                        // §14.1.1), unless the person forbade the relay
                        if !(me.direct_allowed() && me.direct.deliver(to, bytes.clone()).await)
                            && !(me.relay_allowed()
                                && me.serving.relay(me.me(), to, bytes.clone(), device).await)
                        {
                            out.refused.push(Msg::Payload { to, bytes, device });
                        }
                    }
                    Msg::Relay { to, bytes, device } => {
                        if !(me.relay_allowed()
                            && me.serving.relay(me.me(), to, bytes.clone(), device).await)
                        {
                            out.refused.push(Msg::Relay { to, bytes, device });
                        }
                    }
                    // a sweep of the serving node's catalog: each reply
                    // taken by the client, which says whether to ask again
                    // (`light-client-requirements.md` §8)
                    Msg::CatalogQuery(b) => match me.serving.catalog(me.me(), &b).await {
                        Some(reply) => {
                            let more = me.handle.with(move |c| c.take_catalog_reply(&reply)).await;
                            if let Ok((_, more)) = more {
                                out.absorb(me.carry(more).await);
                            }
                        }
                        None => out.refused.push(Msg::CatalogQuery(b)),
                    },
                    // **a client originates and does not forward.** The
                    // records it makes are its own, and the one party that
                    // can put them into the flood is the node serving it
                    // (`wire-format.md` §10.1.1 counts an attached client
                    // an adjacency, which is the same edge read the other
                    // way).
                    Msg::Record(b) => {
                        if !me.serving.propagate(b.clone()).await {
                            out.refused.push(Msg::Record(b));
                        }
                    }
                    Msg::Transport(b) => {
                        // material for the serving node itself is addressed
                        // to the device it presented (`wire-format.md` §7.10)
                        let node = me.serving.me();
                        let device = me.serving.node_device();
                        if !me.serving.relay(me.me(), node, b.clone(), device).await {
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
