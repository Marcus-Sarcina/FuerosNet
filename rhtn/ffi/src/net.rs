//! The kernel's own side of the wire (design §14.1.0).
//!
//! **One participant, one kernel.** Everything that touches the network is
//! here: the endpoint, the session with the serving node, the courier that
//! carries what the client says, and the two readers that carry what
//! arrives back in.  A shell above this boundary holds no protocol state
//! and parses no wire bytes, which is the second parser design §11.2 names
//! as the hazard.
//!
//! **What crosses outward is what to draw** — decrypted payload, a verdict,
//! a session's mode — and never an encoding.

use crate::types::{Id, Refused, id_of};
use rhtn_adaptors::actor::Handle;
use rhtn_adaptors::attached::{self, AttachedNode};
use rhtn_adaptors::courier::Courier;
use rhtn_adaptors::direct::{NoDirect, Reachable};
use rhtn_archive::Keyhash;
use rhtn_archive::submission::WakeEndpoint;
use rhtn_client::ceremony::Dispatched;
use rhtn_crypto::SigningIdentity;
use rhtn_transport::session::{AttachOutcome, ClientConfig, Log, Session, attach_any};
use rhtn_transport::tls::{self, Pins};
use std::collections::{BTreeMap, HashMap};
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::sync::mpsc::UnboundedReceiver;

/// What an attach achieved, as a screen shows it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Attached {
    pub serving: Id,
    /// False on a sibling: the session holds the replicated state but not
    /// the authority to countersign, and the user is owed the difference
    /// (design §14.1.2, `light-client-requirements.md` §4).
    pub primary: bool,
    /// What the responding node says is waiting for this client.  Advisory,
    /// and in a degraded session normally zero, since siblings hold no
    /// queue state.
    pub queued: u64,
}

/// Where a client asks to be rung (design §14.1.5), as the shell hands it
/// over.  **The shell obtains it; this side never does.**
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Wake {
    pub url: String,
    /// What the posted body is encrypted to.
    pub key: Vec<u8>,
    /// When the service says the endpoint stops working, if it said.
    pub lapses_at: Option<u64>,
}

/// Something that arrived, already decrypted.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    /// Application payload from `from`.
    Payload { from: Id, bytes: Vec<u8> },
    /// A verifier's copy of the response it gave about this subject: the
    /// query it answered, or why the copy was refused.
    ResponseCopy { from: Id, query: Option<Id>, refused: Option<String> },
    /// A late response, attached to the record it supplements or not.
    Late { from: Id, record: Option<Id>, refused: Option<String> },
}

/// The session and everything hung off it, replaced whole on each attach.
struct Live {
    session: Arc<Session>,
    courier: Arc<Courier>,
    _serving: Arc<AttachedNode>,
    tasks: Vec<tokio::task::JoinHandle<()>>,
}

impl Drop for Live {
    fn drop(&mut self) {
        for t in &self.tasks {
            t.abort();
        }
    }
}

pub(crate) struct Net {
    /// Taken on the way out.  **A runtime dropped inside an asynchronous
    /// context panics**, and a shell has no way to know whether the thread
    /// it releases a participant on is inside one; shutting the runtime
    /// down in the background never blocks and so never can.
    rt: Option<tokio::runtime::Runtime>,
    endpoint: quinn::Endpoint,
    pins: Pins,
    /// This client's own key, held here as well as on the client's thread.
    /// Both are built from the same seeds, so they are the same key; the
    /// transport needs one and the client never leaves its thread.
    me: Arc<SigningIdentity>,
    live: Mutex<Option<Live>>,
    events: Mutex<Option<UnboundedReceiver<(Keyhash, Dispatched)>>>,
    /// Nonces for what this client hands its node: the platform's random
    /// source, which is the only one this workspace has.
    nonce: Arc<dyn Fn() -> [u8; 16] + Send + Sync>,
}

impl Net {
    /// A network for a client that has not attached to anything yet.
    ///
    /// The endpoint takes an ephemeral port on the loopback interface; a
    /// light client dials and is never dialled, so nothing depends on which
    /// port it got.
    pub(crate) fn new(me: Arc<SigningIdentity>, pins: Pins, nonce: Arc<dyn Fn() -> [u8; 16] + Send + Sync>) -> Result<Net, Refused> {
        let rt = tokio::runtime::Builder::new_multi_thread().enable_all().build().map_err(|e| Refused::new(format!("no runtime: {e}")))?;
        // quinn binds inside a runtime, so the endpoint is built in this
        // one and not in whatever the caller happens to be on
        let endpoint = rt.block_on(async { tls::client_endpoint("0.0.0.0:0".parse().unwrap()) }).map_err(|e| Refused::new(format!("no endpoint: {e:?}")))?;

        Ok(Net { rt: Some(rt), endpoint, pins, me, live: Mutex::new(None), events: Mutex::new(None), nonce })
    }

    fn rt(&self) -> &tokio::runtime::Runtime {
        self.rt.as_ref().expect("the runtime outlives every call on this side")
    }

    pub(crate) fn session(&self) -> Option<Arc<Session>> {
        self.live.lock().unwrap().as_ref().map(|l| l.session.clone())
    }

    fn courier(&self) -> Result<Arc<Courier>, Refused> {
        self.live.lock().unwrap().as_ref().map(|l| l.courier.clone()).ok_or_else(|| Refused::new("no serving node is attached"))
    }

    pub(crate) fn attach(&self, handle: &Handle, node: Keyhash, addrs: &[SocketAddr], population: Vec<Keyhash>) -> Result<Attached, Refused> {
        let cfg = ClientConfig {
            identity: self.me.clone(),
            pins: self.pins.clone(),
            capabilities: BTreeMap::new(),
            attestation: None,
            filter: None,
            sibling_cache: Arc::new(Mutex::new(Vec::new())),
            addresses: Arc::new(Mutex::new(HashMap::from([(node, addrs.to_vec())]))),
            tls: Arc::new(Mutex::new(Default::default())),
            connect_timeout: Duration::from_secs(10),
            on_reachability: None,
            log: Log::default(),
        };
        let (handle_for_task, ep, nonce) = (handle.clone(), self.endpoint.clone(), self.nonce.clone());
        let addrs = addrs.to_vec();
        let built = self.rt().block_on(async move {
            let mut session = match attach_any(&cfg, &ep, node, &addrs, false).await {
                AttachOutcome::Attached(s) => s,
                AttachOutcome::Refused => return Err(Refused::new("the serving node refused this client")),
                AttachOutcome::EndpointFailure(e) => return Err(Refused::new(format!("no session: {e}"))),
            };
            // the two readers take their ends before the session is shared
            let (_, spare) = tokio::sync::mpsc::unbounded_channel();
            let deliveries = std::mem::replace(&mut session.deliveries, spare);
            let (_, spare) = tokio::sync::mpsc::unbounded_channel();
            let frames = std::mem::replace(&mut session.frames, spare);
            let (mode, queued) = (session.ack.mode, session.ack.queued);
            let session = Arc::new(session);
            let held = session.clone();
            let serving = AttachedNode::new(node, Arc::new(move || Some(held.clone())), nonce);
            // **No direct path from here.**  design §14.1.1 gathers
            // candidates on a socket, and this endpoint dials and is never
            // dialled; the relay is the answer where there is no path, which
            // is what the light client's own document already says.
            let (courier, events) = Courier::new(handle_for_task.clone(), serving.clone(), Arc::new(NoDirect(Reachable::default())));
            let tasks = vec![attached::follow(handle_for_task, frames), attached::collect(courier.inbound(), deliveries)];
            // the bundle published, the pool stocked and the population
            // swept, all over this session
            let carried = courier.attach(population).await;
            Ok((Live { session, courier, _serving: serving, tasks }, events, mode, queued, (carried.left.len(), carried.refused.len())))
        })?;
        let (live, events, mode, queued, (left, refused)) = built;
        // an attach produces nothing the adaptors cannot carry: what they
        // hand back is the ceremony's own device-to-device conversation,
        // and no ceremony is open here.  What the node *refused* is a
        // different answer and is reported as one
        if refused > 0 {
            return Err(Refused::new(format!("the serving node refused {refused} of what attaching published, stocked or swept")));
        }
        if left > 0 {
            return Err(Refused::new(format!("{left} message(s) an attach produced had no path")));
        }
        *self.events.lock().unwrap() = Some(events);
        // the previous session's readers stop when this replaces it
        *self.live.lock().unwrap() = Some(live);
        Ok(Attached { serving: id_of(&node), primary: mode == 0, queued })
    }

    /// Carry whatever the client has made and not yet handed up.
    ///
    /// **Called after every step that finishes a record**, rather than
    /// left to the next maintenance: a transaction nobody has seen is one
    /// the network cannot act on, and the party that made it is the only
    /// one that can offer it.
    pub(crate) fn carry_outbox(&self, handle: &Handle) -> Result<(), Refused> {
        let Ok(courier) = self.courier() else { return Ok(()) };
        let carried = self.rt().block_on(async move {
            let msgs = handle.with(|c| c.outbox()).await;
            courier.carry(msgs).await
        });
        if !carried.refused.is_empty() {
            return Err(Refused::new("the serving node would not take the record: it is unpropagated"));
        }
        Ok(())
    }

    pub(crate) fn maintain(&self, handle: &Handle) -> Result<(), Refused> {
        let courier = self.courier()?;
        self.rt().block_on(async move {
            let msgs = handle.with(|c| c.maintain()).await;
            courier.carry(msgs).await;
        });
        Ok(())
    }

    /// **A refusal is an answer, and reaches the caller as one.**  The
    /// serving node says whether it took a submission (`wire-format.md`
    /// §7.10), and reporting success for a message it declined would have
    /// an application believe work was done that was not
    /// (`light-client-requirements.md` §9).
    pub(crate) fn send(&self, to: Keyhash, kind: u64, bytes: Vec<u8>) -> Result<(), Refused> {
        let courier = self.courier()?;
        let carried = self.rt().block_on(async move { courier.send(to, kind, bytes).await }).map_err(Refused::new)?;
        if !carried.refused.is_empty() {
            return Err(Refused::new("the serving node would not take the message: it is unsent"));
        }
        if !carried.left.is_empty() {
            return Err(Refused::new("no path carried the message: it is unsent"));
        }
        Ok(())
    }

    pub(crate) fn wake(&self, endpoint: Option<Wake>) -> Result<(), Refused> {
        let courier = self.courier()?;
        let me = courier.me();
        let ep = endpoint.map(|w| WakeEndpoint { url: w.url, key: w.key, lapses_at: w.lapses_at });
        let took = self.rt().block_on(async move { courier.serving.wake(me, ep).await });
        if took { Ok(()) } else { Err(Refused::new("the serving node did not take the endpoint")) }
    }

    pub(crate) fn next_event(&self, timeout_ms: u64) -> Option<Event> {
        let mut slot = self.events.lock().unwrap();
        let rx = slot.as_mut()?;
        let got = self.rt().block_on(async { tokio::time::timeout(Duration::from_millis(timeout_ms), rx.recv()).await.ok().flatten() });
        got.and_then(|(from, d)| event_of(from, d))
    }
}

fn event_of(from: Keyhash, d: Dispatched) -> Option<Event> {
    let from = id_of(&from);
    match d {
        Dispatched::Application(bytes) => Some(Event::Payload { from, bytes }),
        Dispatched::ResponseCopy(Ok(q)) => Some(Event::ResponseCopy { from, query: Some(q.to_vec()), refused: None }),
        Dispatched::ResponseCopy(Err(why)) => Some(Event::ResponseCopy { from, query: None, refused: Some(why) }),
        Dispatched::Late(Ok(txid)) => Some(Event::Late { from, record: Some(txid.to_vec()), refused: None }),
        Dispatched::Late(Err(why)) => Some(Event::Late { from, record: None, refused: Some(why) }),
        // handled inside the adaptors and never the shell's
        Dispatched::Grant(_) | Dispatched::Candidates(_) => None,
    }
}

impl Drop for Net {
    fn drop(&mut self) {
        if let Some(rt) = self.rt.take() {
            // what is in flight is abandoned, which is what releasing a
            // client means; nothing here waits
            rt.shutdown_background();
        }
    }
}

/// So a caller need not name `Keyhash` to build a pin set.
pub(crate) fn pins_for(known: &[rhtn_crypto::Identity]) -> Pins {
    let p = Pins::new();
    for k in known {
        p.pin_identity(k);
    }
    p
}
