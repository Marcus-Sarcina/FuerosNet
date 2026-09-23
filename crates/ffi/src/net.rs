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
use rhtn_adaptors::courier::{Courier, Inlet};
use rhtn_adaptors::direct::{Gate, LightDirect, Reachable};
use rhtn_archive::Keyhash;
use rhtn_archive::submission::WakeEndpoint;
use rhtn_client::ceremony::Dispatched;
use rhtn_transport::session::{
    AttachOutcome, ClientConfig, Log, Reachability, Session, attach_any, decode_sibling_update,
    encode_sibling_update,
};
use rhtn_transport::tls::{self, Party, Pins};
use std::collections::{BTreeMap, HashMap};
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};

/// What an attach achieved, as a screen shows it.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
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
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct Wake {
    pub url: String,
    /// What the posted body is encrypted to.
    pub key: Vec<u8>,
    /// When the service says the endpoint stops working, if it said.
    pub lapses_at: Option<u64>,
}

/// Direct versus relayed payload, overridable in both directions
/// (`light-client-requirements.md` §5; PRD-01).  **The two disclosures are
/// the shell's to state**: direct reveals this device's address to a peer
/// inside the horizon; relayed reveals the communication graph to the
/// serving node.
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum PathPolicy {
    /// The direct path where a peer inside the horizon answers, the relay
    /// otherwise (design §14.1.1).
    Auto,
    /// Never direct: nothing gathered, nothing offered, nothing dialled.
    RelayOnly,
    /// Never relayed: a message with no direct path is unsent, and said so.
    DirectOnly,
}

/// The connection as a screen shows it (`light-client-requirements.md`
/// §4: the user is told when attachment is degraded, and when it is gone).
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Enum)]
pub enum Status {
    /// No session: never attached, or detached on purpose.
    Detached,
    /// A session with `serving`.  `primary` false is the degraded kind: a
    /// sibling holds the replicated state and not the authority to
    /// countersign, so the subnet-scoped transactions the patron
    /// countersigns wait for the patron (design §14.1.2).
    Attached { serving: Id, primary: bool },
    /// The serving node was judged unreachable and the cached siblings are
    /// being tried, in order.
    Reconnecting { from: Id },
    /// Judged unreachable and no cached sibling answered: the kernel has
    /// no session and will not get one without an attach.
    Lost { from: Id, reason: String },
}

/// Something that arrived, already decrypted; or the connection changing.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Enum)]
pub enum Event {
    /// The connection changed: a failover began, landed on a sibling, or
    /// found none.
    Connection(Status),
    /// Application payload from `from`.
    Payload { from: Id, bytes: Vec<u8> },
    /// A verifier's copy of the response it gave about this subject: the
    /// query it answered, or why the copy was refused.
    ResponseCopy {
        from: Id,
        query: Option<Id>,
        refused: Option<String>,
    },
    /// A late response, attached to the record it supplements or not.
    Late {
        from: Id,
        record: Option<Id>,
        refused: Option<String>,
    },
}

/// The session and everything hung off it, replaced whole on each attach;
/// the session alone is replaced by a failover.
struct Live {
    courier: Arc<Courier>,
    _serving: Arc<AttachedNode>,
    tasks: Vec<tokio::task::JoinHandle<()>>,
}

/// The session in force, read by the attached node's requests and swapped
/// by a failover without rebuilding what hangs off it.
type Current = Arc<Mutex<Option<Arc<Session>>>>;

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
    /// What this client presents in a handshake: its own classical member,
    /// held here as well as on the client's thread from the same seeds; or
    /// the delegated transport credential of a device that holds no seed
    /// (design §23.3), which the transport alone needs.
    me: Party,
    /// The one configuration every attach and failover dials with: its
    /// sibling cache, address book and TLS resumption store survive the
    /// session they were filled on, which is what makes a cold-start
    /// fallback and 0-RTT reattachment possible (§8.2, §9.2).
    cfg: ClientConfig,
    live: Arc<Mutex<Option<Live>>>,
    current: Current,
    status: Arc<Mutex<Status>>,
    events_tx: UnboundedSender<Event>,
    events: Mutex<UnboundedReceiver<Event>>,
    /// The peers a direct path is held to: shared with the client, whose
    /// route reads it, and with the socket that fills it.
    reachable: Reachable,
    policy: Arc<Mutex<PathPolicy>>,
    /// Nonces for what this client hands its node: the platform's random
    /// source, which is the only one this workspace has.
    nonce: Arc<dyn Fn() -> [u8; 16] + Send + Sync>,
}

/// How long one dial may take before the next endpoint is tried: the
/// client's own number (`light-client-requirements.md` §4), no part of the
/// wire.
const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);

impl Net {
    /// A network for a client that has not attached to anything yet.
    ///
    /// The endpoint takes an ephemeral port on the loopback interface; a
    /// light client dials and is never dialled, so nothing depends on which
    /// port it got.
    pub(crate) fn new(
        me: Party,
        pins: Pins,
        nonce: Arc<dyn Fn() -> [u8; 16] + Send + Sync>,
    ) -> Result<Net, Refused> {
        let rt = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .map_err(|e| Refused::new(format!("no runtime: {e}")))?;
        // quinn binds inside a runtime, so the endpoint is built in this
        // one and not in whatever the caller happens to be on
        let endpoint = rt
            .block_on(async { tls::client_endpoint("0.0.0.0:0".parse().unwrap()) })
            .map_err(|e| Refused::new(format!("no endpoint: {e:?}")))?;
        let bind = match me.presenter.credential() {
            Some(c) => rhtn_transport::bind::Binding::default().with_credential(c.clone()),
            None => Default::default(),
        };
        let cfg = ClientConfig {
            me: me.clone(),
            pins: pins.clone(),
            bind,
            capabilities: BTreeMap::new(),
            attestation: None,
            filter: None,
            sibling_cache: Arc::new(Mutex::new(Vec::new())),
            addresses: Arc::new(Mutex::new(HashMap::new())),
            tls: Arc::new(Mutex::new(Default::default())),
            connect_timeout: CONNECT_TIMEOUT,
            on_reachability: None,
            log: Log::default(),
        };
        let (events_tx, events) = tokio::sync::mpsc::unbounded_channel();
        Ok(Net {
            reachable: Reachable::default(),
            policy: Arc::new(Mutex::new(PathPolicy::Auto)),
            rt: Some(rt),
            endpoint,
            me,
            cfg,
            live: Arc::new(Mutex::new(None)),
            current: Arc::new(Mutex::new(None)),
            status: Arc::new(Mutex::new(Status::Detached)),
            events_tx,
            events: Mutex::new(events),
            nonce,
        })
    }

    pub(crate) fn status(&self) -> Status {
        self.status.lock().unwrap().clone()
    }

    /// The set the client's own route reads, for the client to be built on.
    pub(crate) fn reachable(&self) -> Reachable {
        self.reachable.clone()
    }

    pub(crate) fn path(&self) -> PathPolicy {
        *self.policy.lock().unwrap()
    }

    /// The person's override, applied to the session in force at once:
    /// the gate reads it at each gather, and the courier's relay switch
    /// follows it.
    pub(crate) fn set_path(&self, p: PathPolicy) {
        *self.policy.lock().unwrap() = p;
        if let Some(l) = self.live.lock().unwrap().as_ref() {
            l.courier.set_relay_allowed(p != PathPolicy::DirectOnly);
            l.courier.set_direct_allowed(p != PathPolicy::RelayOnly);
        }
    }

    pub(crate) fn direct_to(&self, peer: &Keyhash) -> bool {
        self.reachable.holds(peer)
    }

    /// Sweep the serving node's catalog (`light-client-requirements.md`
    /// §8): the client's queries carried and each reply taken until the
    /// node's portion is complete or truncated.
    pub(crate) fn browse(&self, handle: &Handle) -> Result<(), Refused> {
        let courier = self.courier()?;
        let carried = self.rt().block_on(async move {
            let msgs = handle.with(|c| c.browse()).await;
            courier.carry(msgs).await
        });
        if !carried.refused.is_empty() {
            return Err(Refused::new("the serving node answered no catalog query"));
        }
        Ok(())
    }

    /// The cached sibling list as it goes to storage
    /// (`light-client-requirements.md` §4: persisted across restarts,
    /// because it cannot be discovered once the serving node is dark).
    pub(crate) fn siblings(&self) -> Vec<u8> {
        encode_sibling_update(&self.cfg.sibling_cache.lock().unwrap())
    }

    /// The list back, whole or not at all: a failed or partial load means
    /// no cached siblings, never a partial list.
    pub(crate) fn restore_siblings(&self, b: &[u8]) -> usize {
        let list = decode_sibling_update(b).unwrap_or_default();
        for s in &list {
            if let Some(km) = &s.key_material {
                let _ = self.cfg.pins.pin(s.keyhash, km);
            }
        }
        let n = list.len();
        *self.cfg.sibling_cache.lock().unwrap() = list;
        n
    }

    pub(crate) fn presented_key(&self) -> [u8; 32] {
        self.me.presenter.presented_key()
    }

    fn rt(&self) -> &tokio::runtime::Runtime {
        self.rt
            .as_ref()
            .expect("the runtime outlives every call on this side")
    }

    /// Close the session and drop everything hung off it.
    pub(crate) fn detach(&self) {
        if let Some(s) = self.current.lock().unwrap().take() {
            s.conn.close(quinn::VarInt::from_u32(0), b"detached");
            // the close frame leaves before anything else happens: a
            // process that ended its runtime with the frame still queued
            // would leave the node a session it believes live, and what
            // is queued for this device behind a delivery that never ends
            let conn = s.conn.clone();
            self.rt().block_on(async move {
                let _ = tokio::time::timeout(Duration::from_secs(1), conn.closed()).await;
            });
        }
        *self.live.lock().unwrap() = None;
        *self.status.lock().unwrap() = Status::Detached;
    }

    pub(crate) fn session(&self) -> Option<Arc<Session>> {
        self.current.lock().unwrap().clone()
    }

    fn courier(&self) -> Result<Arc<Courier>, Refused> {
        self.live
            .lock()
            .unwrap()
            .as_ref()
            .map(|l| l.courier.clone())
            .ok_or_else(|| Refused::new("no serving node is attached"))
    }

    pub(crate) fn attach(
        &self,
        handle: &Handle,
        node: Keyhash,
        addrs: &[SocketAddr],
        population: Vec<Keyhash>,
    ) -> Result<Attached, Refused> {
        self.detach();
        self.cfg
            .addresses
            .lock()
            .unwrap()
            .insert(node, addrs.to_vec());
        let cfg = self.cfg.clone();
        let (handle_for_task, ep, nonce) =
            (handle.clone(), self.endpoint.clone(), self.nonce.clone());
        let current = self.current.clone();
        let addrs = addrs.to_vec();
        let (reachable, policy) = (self.reachable.clone(), self.policy.clone());
        let built = self.rt().block_on(async move {
            // the serving node first, and the cached siblings where it is
            // unreachable at attach time: the cold-start fallback
            // (`light-client-requirements.md` §4), which a client that
            // waited for an established session to die could never make
            let (serving_node, mut session) = match attach_any(&cfg, &ep, node, &addrs, false)
                .await
            {
                AttachOutcome::Attached(s) => (node, s),
                AttachOutcome::Refused => {
                    return Err(Refused::new("the serving node refused this client"));
                }
                AttachOutcome::Unbound(why) => {
                    return Err(Refused::new(format!("no session: {why}")));
                }
                AttachOutcome::EndpointFailure(e) => match try_siblings(&cfg, &ep).await {
                    Ok(landed) => landed,
                    Err(last) => {
                        return Err(Refused::new(format!(
                            "no session: the serving node is unreachable ({e}) and no cached sibling answered ({last})"
                        )));
                    }
                },
            };
            let (deliveries, frames) = take_readers(&mut session);
            // the serving node answers STUN on the socket it serves on
            // (design §14.1.1): where this side's reflexive candidate comes
            // from
            let stun = session.conn.remote_address();
            // the direct socket sits on the interface the session uses, so
            // the host candidate it gathers is an address a peer can dial;
            // the unspecified address would name nothing
            let local = interface_toward(stun);
            let (mode, queued) = (session.ack.mode, session.ack.queued);
            let session = Arc::new(session);
            *current.lock().unwrap() = Some(session.clone());
            let reads = current.clone();
            let serving = AttachedNode::new(
                serving_node,
                Arc::new(move || reads.lock().unwrap().clone()),
                nonce,
            );
            // **The direct path, on a socket of this client's own**
            // (design §14.1.1): candidates gathered there, the peer's
            // dialled from it, and what a peer opens toward it taken under
            // the key it authenticates.  Gated to the horizon and to the
            // person's override; the relay is the answer where there is no
            // path.
            let inlet = Inlet::default();
            let gate: Gate = {
                let (h, p) = (handle_for_task.clone(), policy.clone());
                Arc::new(move |peer| {
                    if *p.lock().unwrap() == PathPolicy::RelayOnly {
                        return false;
                    }
                    let peer = *peer;
                    h.with_blocking(move |c| c.horizon.distance(&peer).is_some())
                })
            };
            let direct = LightDirect::bind(
                cfg.me.clone(),
                cfg.pins.clone(),
                cfg.bind.clone().with_held(Arc::new(handle_for_task.clone())),
                reachable.clone(),
                SocketAddr::new(local, 0),
                None,
                Some(stun),
                gate,
                inlet.inbound(),
            )
            .map_err(|e| Refused::new(format!("no direct socket: {e}")))?;
            let (courier, events) = Courier::new(
                handle_for_task.clone(),
                serving.clone(),
                Arc::new(direct),
            );
            inlet.bind(courier.inbound());
            let p = *policy.lock().unwrap();
            courier.set_relay_allowed(p != PathPolicy::DirectOnly);
            courier.set_direct_allowed(p != PathPolicy::RelayOnly);
            let tasks = vec![
                attached::follow(handle_for_task.clone(), frames),
                attached::collect(courier.inbound(), deliveries),
            ];
            // the bundle published, the pool stocked and the population
            // swept, all over this session
            let carried = courier.attach(population).await;
            Ok((
                Live {
                    courier,
                    _serving: serving,
                    tasks,
                },
                events,
                session,
                serving_node,
                mode,
                queued,
                (carried.left.len(), carried.refused.len()),
            ))
        })?;
        let (live, events, session, serving_node, mode, queued, (left, refused)) = built;
        // an attach produces nothing the adaptors cannot carry: what they
        // hand back is the ceremony's own device-to-device conversation,
        // and no ceremony is open here.  What the node *refused* is a
        // different answer and is reported as one
        if refused > 0 {
            self.detach();
            return Err(Refused::new(format!(
                "the serving node refused {refused} of what attaching published, stocked or swept"
            )));
        }
        if left > 0 {
            self.detach();
            return Err(Refused::new(format!(
                "{left} message(s) an attach produced had no path"
            )));
        }
        // what the courier hands up joins the one event stream, and the
        // session is watched for the failover the kernel drives itself
        let mut live = live;
        live.tasks
            .push(self.rt().spawn(forward(events, self.events_tx.clone())));
        live.tasks.push(self.rt().spawn(watch(
            session,
            serving_node,
            self.cfg.clone(),
            self.endpoint.clone(),
            handle.clone(),
            self.current.clone(),
            self.status.clone(),
            self.events_tx.clone(),
            live.courier.clone(),
        )));
        // the previous session's readers stop when this replaces it
        *self.live.lock().unwrap() = Some(live);
        *self.status.lock().unwrap() = Status::Attached {
            serving: id_of(&serving_node),
            primary: mode == 0,
        };
        Ok(Attached {
            serving: id_of(&serving_node),
            primary: mode == 0,
            queued,
        })
    }

    /// Carry whatever the client has made and not yet handed up.
    ///
    /// **Called after every step that finishes a record**, rather than
    /// left to the next maintenance: a transaction nobody has seen is one
    /// the network cannot act on, and the party that made it is the only
    /// one that can offer it.
    pub(crate) fn carry_outbox(&self, handle: &Handle) -> Result<(), Refused> {
        let Ok(courier) = self.courier() else {
            return Ok(());
        };
        let carried = self.rt().block_on(async move {
            let msgs = handle.with(|c| c.outbox()).await;
            courier.carry(msgs).await
        });
        if !carried.refused.is_empty() {
            return Err(Refused::new(
                "the serving node would not take the record: it is unpropagated",
            ));
        }
        Ok(())
    }

    /// Carry messages the client made outside its outbox: a bundle the
    /// ceremony device signed for this one, once it is attached.
    pub(crate) fn carry(&self, msgs: Vec<rhtn_client::ceremony::Msg>) -> Result<(), Refused> {
        let courier = self.courier()?;
        let carried = self.rt().block_on(async move { courier.carry(msgs).await });
        if !carried.refused.is_empty() {
            return Err(Refused::new("the serving node refused the publication"));
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
        let carried = self
            .rt()
            .block_on(async move { courier.send(to, kind, bytes).await })
            .map_err(Refused::new)?;
        if !carried.refused.is_empty() {
            return Err(Refused::new(
                "the serving node would not take the message: it is unsent",
            ));
        }
        if !carried.left.is_empty() {
            return Err(Refused::new("no path carried the message: it is unsent"));
        }
        Ok(())
    }

    pub(crate) fn wake(&self, endpoint: Option<Wake>) -> Result<(), Refused> {
        let courier = self.courier()?;
        let me = courier.me();
        let ep = endpoint.map(|w| WakeEndpoint {
            url: w.url,
            key: w.key,
            lapses_at: w.lapses_at,
        });
        let took = self
            .rt()
            .block_on(async move { courier.serving.wake(me, ep).await });
        if took {
            Ok(())
        } else {
            Err(Refused::new("the serving node did not take the endpoint"))
        }
    }

    pub(crate) fn next_event(&self, timeout_ms: u64) -> Option<Event> {
        let mut rx = self.events.lock().unwrap();
        self.rt().block_on(async {
            tokio::time::timeout(Duration::from_millis(timeout_ms), rx.recv())
                .await
                .ok()
                .flatten()
        })
    }

    /// Routine maintenance on the kernel's own clock, while the process
    /// runs and a session is held: what [`crate::client::Participant::maintain`]
    /// does when a shell calls it, done every `every` without one.  `after`
    /// runs when a round carried anything, so the state that changed is
    /// written.
    pub(crate) fn schedule_maintenance(
        &self,
        handle: Handle,
        every: Duration,
        after: Arc<dyn Fn() + Send + Sync>,
    ) {
        let live = self.live.clone();
        self.rt().spawn(async move {
            let mut tick = tokio::time::interval(every);
            tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
            tick.tick().await;
            loop {
                tick.tick().await;
                let courier = live.lock().unwrap().as_ref().map(|l| l.courier.clone());
                let Some(courier) = courier else {
                    continue;
                };
                let msgs = handle.with(|c| c.maintain()).await;
                if msgs.is_empty() {
                    continue;
                }
                courier.carry(msgs).await;
                after();
            }
        });
    }
}

/// The cached siblings in order (`wire-format.md` §8.2), each at the
/// endpoints its reference carried and any the address book holds: the
/// first that attaches, or why the last did not.
async fn try_siblings(
    cfg: &ClientConfig,
    ep: &quinn::Endpoint,
) -> Result<(Keyhash, Session), String> {
    let siblings = cfg.sibling_cache.lock().unwrap().clone();
    let mut last = "no cached siblings".to_string();
    for s in siblings {
        if let Some(km) = &s.key_material {
            let _ = cfg.pins.pin(s.keyhash, km);
        }
        let mut addrs = cfg
            .addresses
            .lock()
            .unwrap()
            .get(&s.keyhash)
            .cloned()
            .unwrap_or_default();
        for p in &s.endpoints {
            let a = p.socket();
            if !addrs.contains(&a) {
                addrs.push(a);
            }
        }
        match attach_any(cfg, ep, s.keyhash, &addrs, false).await {
            AttachOutcome::Attached(sess) => return Ok((s.keyhash, sess)),
            AttachOutcome::Refused => last = format!("{} refused", hex4(&s.keyhash)),
            AttachOutcome::Unbound(why) => last = format!("{}: {why}", hex4(&s.keyhash)),
            AttachOutcome::EndpointFailure(e) => last = format!("{}: {e}", hex4(&s.keyhash)),
        }
    }
    Err(last)
}

/// A session's two readers: deliveries, and the control frames beyond the
/// session's own.
type Readers = (
    UnboundedReceiver<Vec<u8>>,
    UnboundedReceiver<(u64, Vec<u8>)>,
);

/// The address of the interface the host routes to `remote` by: a socket
/// connected there and asked what it was bound on, which is how a client
/// learns its own address without enumerating interfaces.  The
/// unspecified address where the probe fails.
fn interface_toward(remote: SocketAddr) -> std::net::IpAddr {
    let any: SocketAddr = if remote.is_ipv4() {
        "0.0.0.0:0".parse().unwrap()
    } else {
        "[::]:0".parse().unwrap()
    };
    std::net::UdpSocket::bind(any)
        .and_then(|s| s.connect(remote).and_then(|_| s.local_addr()))
        .map(|a| a.ip())
        .unwrap_or(any.ip())
}

/// The two readers, taken before the session is shared.
fn take_readers(session: &mut Session) -> Readers {
    let (_, spare) = tokio::sync::mpsc::unbounded_channel();
    let deliveries = std::mem::replace(&mut session.deliveries, spare);
    let (_, spare) = tokio::sync::mpsc::unbounded_channel();
    let frames = std::mem::replace(&mut session.frames, spare);
    (deliveries, frames)
}

/// What the courier hands up, on the one event stream.
async fn forward(mut from: UnboundedReceiver<(Keyhash, Dispatched)>, to: UnboundedSender<Event>) {
    while let Some((k, d)) = from.recv().await {
        if let Some(e) = event_of(k, d) {
            let _ = to.send(e);
        }
    }
}

/// Watch a session for the three missed intervals that judge its peer
/// unreachable (`wire-format.md` §8.2), then try the cached siblings in
/// order, inside the kernel and with nothing asked of the shell but to
/// read the status.  A sibling that answers becomes the session; none
/// answering leaves the kernel with no session and says so.
#[allow(clippy::too_many_arguments)]
async fn watch(
    session: Arc<Session>,
    peer: Keyhash,
    cfg: ClientConfig,
    ep: quinn::Endpoint,
    handle: Handle,
    current: Current,
    status: Arc<Mutex<Status>>,
    events: UnboundedSender<Event>,
    courier: Arc<Courier>,
) {
    let mut session = session;
    let mut peer = peer;
    loop {
        loop {
            if *session.reach.lock().unwrap() == Reachability::Unreachable {
                break;
            }
            // this session was replaced or dropped on purpose: nothing to
            // watch
            let same = current
                .lock()
                .unwrap()
                .as_ref()
                .is_some_and(|c| Arc::ptr_eq(c, &session));
            if !same {
                return;
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
        let tell = |s: Status| {
            *status.lock().unwrap() = s.clone();
            let _ = events.send(Event::Connection(s));
        };
        tell(Status::Reconnecting { from: id_of(&peer) });
        session.conn.close(quinn::VarInt::from_u32(0), b"failover");
        *current.lock().unwrap() = None;
        let landed = try_siblings(&cfg, &ep).await;
        let (to, mut sess) = match landed {
            Ok(l) => l,
            Err(last) => {
                tell(Status::Lost {
                    from: id_of(&peer),
                    reason: last,
                });
                return;
            }
        };
        let (deliveries, frames) = take_readers(&mut sess);
        let mode = sess.ack.mode;
        let sess = Arc::new(sess);
        *current.lock().unwrap() = Some(sess.clone());
        // the new session's readers feed the same courier and client
        tokio::spawn(attached::follow(handle.clone(), frames));
        tokio::spawn(attached::collect(courier.inbound(), deliveries));
        tell(Status::Attached {
            serving: id_of(&to),
            primary: mode == 0,
        });
        session = sess;
        peer = to;
    }
}

fn hex4(k: &Keyhash) -> String {
    k[..4].iter().map(|b| format!("{b:02x}")).collect()
}

fn event_of(from: Keyhash, d: Dispatched) -> Option<Event> {
    let from = id_of(&from);
    match d {
        Dispatched::Application(bytes) => Some(Event::Payload { from, bytes }),
        Dispatched::ResponseCopy(Ok(q)) => Some(Event::ResponseCopy {
            from,
            query: Some(q.to_vec()),
            refused: None,
        }),
        Dispatched::ResponseCopy(Err(why)) => Some(Event::ResponseCopy {
            from,
            query: None,
            refused: Some(why),
        }),
        Dispatched::Late(Ok(txid)) => Some(Event::Late {
            from,
            record: Some(txid.to_vec()),
            refused: None,
        }),
        Dispatched::Late(Err(why)) => Some(Event::Late {
            from,
            record: None,
            refused: Some(why),
        }),
        // handled inside the adaptors and never the shell's; an archive
        // fetch is the kernel answering and evaluating for itself
        // (`light-client-requirements.md` §2), and its result reaches the
        // shell as the standing it changes rather than as an event
        Dispatched::Grant(_)
        | Dispatched::Candidates(_)
        | Dispatched::Served { .. }
        | Dispatched::Fetched(_) => None,
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
