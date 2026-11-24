//! The courier: what the client says goes out over the seams, and what
//! arrives on them goes in.

use crate::actor::Handle;
use crate::beyond::Beyond;
use crate::direct::Direct;
use crate::serving::{Inbound, Serving};
use crate::verifier::Verifiers;
use rhtn_archive::Keyhash;
use rhtn_archive::node_key::{NodeKeyRequest, REQUEST_NODE_KEY};
use rhtn_archive::submission::{
    REQUEST_RELAY, RelaySubmission, SUBMISSION_ACCEPTED, SUBMISSION_REFUSED, SubmissionReply,
};
use rhtn_client::ceremony::{Dispatched, Msg};
use rhtn_client::horizon::{Askable, Upstream};
use rhtn_client::payload::{KIND_CANDIDATES, KIND_QUERY, KIND_RESPONSE, KIND_RESPONSE_COPY};
use rhtn_client::query::QueryRequest;
use rhtn_client::verifier::{Answer, GrantOutcome, QueryOutcome};
use rhtn_transport::traversal::{decode_candidates, encode_candidates};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::sync::mpsc;

/// Where inbound payload goes once a courier exists: bound late, since the
/// socket that receives is built before the courier that reads.
#[derive(Clone, Default)]
pub struct Inlet(Arc<Mutex<Option<Inbound>>>);

impl Inlet {
    /// Where payload for this party is delivered.
    pub fn inbound(&self) -> Inbound {
        let slot = self.0.clone();
        Arc::new(move |from, bytes| {
            let f = slot.lock().unwrap().clone();
            if let Some(f) = f {
                f(from, bytes);
            }
        })
    }

    /// Deliver this party's payload to `to` from now on.
    pub fn bind(&self, to: Inbound) {
        *self.0.lock().unwrap() = Some(to);
    }
}

/// What a [`Courier::carry`] could not complete, with the two reasons kept
/// apart.  **Both are unsent work**, and a caller that ignores either
/// reports a message delivered that is not.
#[derive(Debug, Default)]
pub struct Carried {
    /// Not carried, and nobody refused it: the ceremony's own conversation
    /// between two present devices, which no document gives an encoding;
    /// and a message waiting on a node key no node answered for, named by
    /// the fetch that went unanswered, which stays with the client.
    pub left: Vec<Msg>,
    /// Handed to the serving node and refused by it.
    pub refused: Vec<Msg>,
    /// **Answers to requests that answer to their caller** rather than to
    /// this client's state: a `ResourceResponse` is the application's
    /// (`wire-format.md` §11.1), held nowhere and handed back as it came.
    pub answers: Vec<Vec<u8>>,
}

impl Carried {
    fn absorb(&mut self, other: Carried) {
        self.left.extend(other.left);
        self.refused.extend(other.refused);
        self.answers.extend(other.answers);
    }

    /// Whether everything travelled.
    pub fn complete(&self) -> bool {
        self.left.is_empty() && self.refused.is_empty()
    }
}

/// One delivery as the node or the direct path hands it over: who from,
/// the bytes, and the binding the node carried where it carried one.
/// What arrives: from whom, the bytes, a binding sent along, and **the
/// node it came by** where a tab's collector delivered it.
/// Who it came from, the bytes, and the node that delivered it where a
/// node did: then the bytes are a `RelayedPayload` and the sender is inside
/// the seal.
type Delivery = (Keyhash, Vec<u8>, Option<Keyhash>);

/// What carries the client's messages: the serving node for what it
/// relays, the direct path where one is held, and the client's own thread
/// behind `handle`.
pub struct Courier {
    /// The client, on its own thread.
    pub handle: Handle,
    /// **Every node this client holds a session with, and no first among
    /// them** [restructure step 3, 2026-11-14]: a message names the node
    /// it goes to, or goes to every one; where a message names none and
    /// one must be chosen, [`Courier::first`] is the lowest keyhash held,
    /// and that choice is the only "the serving node" left.
    /// **The further nodes this client holds a session with** — the tabs
    /// of the browser model [author, 2026-11-11], one per address past the
    /// first; a payload for a peer whose address is served by one of them
    /// goes over that session rather than being dialled.
    servings: Mutex<BTreeMap<Keyhash, Arc<dyn Serving>>>,
    /// The direct path, tried before the relay (design §12.6.3).
    pub direct: Arc<dyn Direct>,
    /// The network past this client's serving node, where a dialler was
    /// wired: how payload reaches the recipient's own node.
    beyond: Mutex<Option<Arc<Beyond>>>,
    /// What every [`Courier::inbound`] closure feeds: one queue, drained
    /// by one task, so deliveries are taken in the order they arrived.
    inbox: Mutex<Option<mpsc::UnboundedSender<Delivery>>>,
    app: mpsc::UnboundedSender<(Keyhash, Dispatched)>,
    verifiers: Mutex<Option<Arc<Verifiers>>>,
    offered: Mutex<HashSet<Keyhash>>,
    /// Queries that arrived on the end-to-end path and wait for their
    /// grant, by id: who asked, so the answer goes back the way the query
    /// came.
    awaiting: Mutex<Awaiting>,
    /// Run after every arrival this courier put through the client, to
    /// write what it changed.
    ///
    /// **It cannot be left to whatever the arrival becomes.** A delivery
    /// moves the client's state — the queue position collected, the seal
    /// remembered as opened — whether or not anything is handed up: a query
    /// and a grant are answered here and go to no application channel at
    /// all, so a verifier that served a whole ceremony produced nothing a
    /// caller could save on. On the bench a verifier opened sessions on
    /// two re-sent messages, answered a query on one of them, and was
    /// restarted by its platform still holding neither [2026-10-12].
    ///
    /// Blocking: whoever wires it decides what a write costs.
    wrote: Mutex<Option<Arc<dyn Fn() + Send + Sync>>>,
}

/// How many queries may wait for their grant at once.  A session peer can
/// put queries faster than grants arrive, and each waiting one holds a map
/// entry and a timer; past the bound the oldest is forgotten, and its
/// querier hears nothing, as it would have from a verifier that was never
/// reached.  Generous beside the client's own grant buffer, which bounds
/// the wait in time.
pub const AWAITING_QUERIES: usize = 256;

/// The waiting queries, oldest evicted at the bound.
#[derive(Default)]
struct Awaiting {
    by_id: HashMap<[u8; 32], (Keyhash, u64)>,
    next: u64,
}

impl Awaiting {
    fn insert(&mut self, qid: [u8; 32], querier: Keyhash) {
        if self.by_id.len() >= AWAITING_QUERIES && !self.by_id.contains_key(&qid) {
            let oldest = self
                .by_id
                .iter()
                .min_by_key(|(_, (_, seq))| *seq)
                .map(|(k, _)| *k);
            if let Some(oldest) = oldest {
                self.by_id.remove(&oldest);
            }
        }
        self.next += 1;
        self.by_id.insert(qid, (querier, self.next));
    }

    fn remove(&mut self, qid: &[u8; 32]) -> Option<Keyhash> {
        self.by_id.remove(qid).map(|(q, _)| q)
    }

    #[cfg(test)]
    fn len(&self) -> usize {
        self.by_id.len()
    }
}

impl Courier {
    /// A courier for `handle`, and where its application payload and the
    /// subject's copies arrive.
    pub fn new(
        handle: Handle,
        direct: Arc<dyn Direct>,
    ) -> (Arc<Courier>, mpsc::UnboundedReceiver<(Keyhash, Dispatched)>) {
        let (app, rx) = mpsc::unbounded_channel();
        (
            Arc::new(Courier {
                handle,
                servings: Mutex::new(BTreeMap::new()),
                direct,
                beyond: Mutex::new(None),
                inbox: Mutex::new(None),
                app,
                verifiers: Mutex::new(None),
                offered: Mutex::new(HashSet::new()),
                awaiting: Mutex::new(Awaiting::default()),
                wrote: Mutex::new(None),
            }),
            rx,
        )
    }

    /// A tab opened: a further node this courier carries over.
    pub fn serve_also(&self, s: Arc<dyn Serving>) {
        self.servings.lock().unwrap().insert(s.me(), s);
    }

    /// A tab closed.
    pub fn drop_serving(&self, node: &Keyhash) {
        self.servings.lock().unwrap().remove(node);
    }

    /// Every node this courier carries over: the serving node first, then
    /// the tabs.
    pub fn servings(&self) -> Vec<Arc<dyn Serving>> {
        self.servings.lock().unwrap().values().cloned().collect()
    }

    /// The node chosen where a message names none: the lowest keyhash
    /// held, or nothing where no session is held.
    pub fn first(&self) -> Option<Arc<dyn Serving>> {
        self.servings.lock().unwrap().values().next().cloned()
    }

    /// The node a request beyond this client's own nodes says it came
    /// by, where one is held.
    fn via(&self) -> Option<Keyhash> {
        self.first().map(|s| s.me())
    }

    async fn publish_profile_all(&self, b: &[u8]) -> bool {
        let mut took = false;
        for s in self.servings() {
            took |= s.publish_profile(b).await;
        }
        took
    }

    /// Each node held is told the horizon of the address it serves.
    async fn assert_horizon_all(&self, bytes: &[u8], nonce: [u8; 16]) -> bool {
        let mut took = false;
        for s in self.servings() {
            took |= s.assert_horizon(bytes, nonce).await;
        }
        took
    }

    /// A record to every node held: which of them keeps it is the node's
    /// own decision (`infra-client-requirements.md` §1).
    async fn propagate_all(&self, b: &[u8]) -> bool {
        let mut took = false;
        for s in self.servings() {
            took |= s.propagate(b.to_vec()).await;
        }
        took
    }

    async fn profile_first(&self, b: &[u8]) -> Option<Vec<u8>> {
        match self.first() {
            Some(s) => s.profile(b).await,
            None => None,
        }
    }

    /// A resource is asked of the node that served its entry, else the first.
    async fn resource_at(&self, node: &Keyhash, bytes: &[u8]) -> Option<Vec<u8>> {
        match self.serving_for(node).or_else(|| self.first()) {
            Some(s) => s.resource(self.me(), bytes).await,
            None => None,
        }
    }

    /// The session held with `node`, where one is: by what each answers
    /// to now, so a tab that failed over to a sibling is found by the
    /// sibling.
    fn serving_for(&self, node: &Keyhash) -> Option<Arc<dyn Serving>> {
        self.servings().into_iter().find(|s| s.me() == *node)
    }

    /// What to run after every arrival, once the client has taken it: the
    /// state written. Nothing runs where nobody wires one.
    pub fn write_after_arrivals(&self, wrote: Arc<dyn Fn() + Send + Sync>) {
        *self.wrote.lock().unwrap() = Some(wrote);
    }

    /// Write what an arrival changed, where a writer is wired.
    pub(crate) fn wrote(&self) {
        let w = self.wrote.lock().unwrap().clone();
        if let Some(w) = w {
            w();
        }
    }

    pub(crate) fn report_grants_to(&self, v: Arc<Verifiers>) {
        *self.verifiers.lock().unwrap() = Some(v);
    }

    /// Reach nodes this client is not attached to through `beyond`.
    ///
    /// **Wired late, like the inbound side**, because the dialler is built
    /// from the endpoint an attach opens and the courier exists before it.
    /// A courier nobody wires one into carries payload the way it always
    /// did: the direct path, then the node serving this client.
    pub fn reach_beyond(&self, beyond: Arc<Beyond>) {
        *self.beyond.lock().unwrap() = Some(beyond);
    }

    async fn their_profile(&self, body: &[u8]) -> Option<Vec<u8>> {
        let beyond = self.beyond.lock().unwrap().clone()?;
        let subject = rhtn_archive::profile::ProfileRequest::decode(body)
            .ok()?
            .subject;
        let reach = self.handle.with(move |c| c.reach_for(&subject)).await;
        if let Upstream::Known { node, .. } = &reach.upstream
            && let Some(held) = self.serving_for(node)
        {
            return held.profile(body).await;
        }
        beyond
            .ask_their_node(
                subject,
                reach,
                self.via(),
                rhtn_archive::profile::REQUEST_PROFILE,
                body.to_vec(),
            )
            .await
    }

    /// The identity this courier carries for.
    pub fn me(&self) -> Keyhash {
        self.handle.me()
    }

    /// Whether `peer` has been offered the direct path from here.
    pub fn offered(&self, peer: &Keyhash) -> bool {
        self.offered.lock().unwrap().contains(peer)
    }

    /// This client's inbound side, for a socket or a node to call with
    /// what arrived from a peer.
    pub fn inbound(self: &Arc<Self>) -> Inbound {
        // **Taken in the order they arrived, one at a time** [reviewer,
        // 2026-10-08]. Each delivery used to be spawned as a task of its
        // own, and two tasks on a multi-threaded runtime run in either
        // order — so the second of two messages a peer sent back to back
        // could reach the client before the first. Before a session
        // exists only the first message can open one, and a second that
        // arrives ahead of it is undecryptable and dropped: on the bench
        // a witness lost a participant's back-pointers that way in three
        // runs of twelve, the kind-12 request that would have opened the
        // session landing a moment after the kind-15 that needed it.
        //
        // One queue per courier, whatever closure fed it, so the direct
        // path and the relay cannot reorder one peer's messages between
        // them either. The task holds the courier weakly and ends with the
        // last closure, so a courier replaced by a re-attach is not kept
        // alive by its own queue.
        let tx = {
            let mut slot = self.inbox.lock().unwrap();
            match slot.as_ref() {
                Some(tx) => tx.clone(),
                None => {
                    let (tx, mut rx) = mpsc::unbounded_channel::<Delivery>();
                    let weak = Arc::downgrade(self);
                    tokio::spawn(async move {
                        while let Some((from, bytes, via)) = rx.recv().await {
                            let Some(me) = weak.upgrade() else { break };
                            me.receive_via(from, bytes, via).await;
                        }
                    });
                    *slot = Some(tx.clone());
                    tx
                }
            }
        };
        Arc::new(move |from, bytes| {
            // a courier whose task has gone is one nobody holds any more
            let _ = tx.send((from, bytes, None));
        })
    }

    /// The inbound a node's deliveries go to: each a `RelayedPayload` from
    /// `via` (`wire-format.md` §7.10), opened and attributed by the client,
    /// and the node recorded as where the next message to its sender goes.
    pub fn inbound_via(self: &Arc<Self>, via: Keyhash) -> Inbound {
        let _ = self.inbound();
        let tx = self
            .inbox
            .lock()
            .unwrap()
            .clone()
            .expect("the inbox is made by inbound()");
        Arc::new(move |_, bytes| {
            let _ = tx.send((via, bytes, Some(via)));
        })
    }

    /// What arrived from `from` on the direct path: `[kind, bytes]` from
    /// the peer the transport authenticated, delivered by kind.
    pub async fn receive(self: Arc<Self>, from: Keyhash, bytes: Vec<u8>) {
        self.receive_via(from, bytes, None).await
    }

    /// An arrival: on the direct path from `from`, or delivered by `via`,
    /// a node, whose seal the client opens and attributes. A payload taken
    /// from a node records that node as where the next message to its
    /// sender goes [author, 2026-11-11].
    pub async fn receive_via(self: Arc<Self>, from: Keyhash, bytes: Vec<u8>, via: Option<Keyhash>) {
        let (d, owed) = self
            .handle
            .with(move |c| {
                let d = match via {
                    Some(node) => c.receive_relayed(node, &bytes).map(|(sender, d)| {
                        if !matches!(d, Dispatched::Unverified { .. }) {
                            c.record_arrival(sender, &node);
                        }
                        (sender, d)
                    }),
                    None => c.receive_payload(from, &bytes).map(|d| (from, d)),
                };
                // what the client owes in answer goes now, on this courier:
                // the conversation's replies (`wire-format.md` §7.10.1), an
                // archive reply, a record to propagate
                (d, c.outbox())
            })
            .await;
        let (from, d) = match d {
            Ok((sender, d)) => (sender, Ok(d)),
            Err(e) => (from, Err(e)),
        };
        // **the state goes down before anything else happens**: what this
        // arrival collected and opened, and the query it answered
        self.wrote();
        if !owed.is_empty() {
            self.carry(owed).await;
        }
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
                // a query that came on the end-to-end path is answered on
                // it; one that came on a request stream completes the stream
                let querier = self.awaiting.lock().unwrap().remove(&a.query_id);
                match querier {
                    Some(querier) => self.answer_back(querier, a).await,
                    None => {
                        let v = self.verifiers.lock().unwrap().clone();
                        if let Some(v) = v {
                            v.granted(a);
                        }
                    }
                }
            }
            Ok(Dispatched::Grant(_)) => {}
            // a query on the end-to-end path (`wire-format.md` §5.6): the
            // querier is the peer the session attributed it to, and the
            // answer goes back to it the same way
            Ok(Dispatched::Query { query, outcome }) => match outcome {
                QueryOutcome::Answered(a) => self.answer_back(from, a).await,
                QueryOutcome::AwaitingGrant => {
                    if let Some(qid) = query {
                        self.awaiting.lock().unwrap().insert(qid, from);
                        self.expire_after(qid).await;
                    }
                }
                QueryOutcome::Closed(_) => {}
            },
            Ok(other) => {
                let _ = self.app.send((from, other));
            }
            // unopened or unattributed: dropped, and nothing said to anyone
            Err(_) => {}
        }
    }

    /// Take a conversation step off the **local bearer** and put what it
    /// came to where a network-borne step's outcome goes.
    ///
    /// **The two arrivals must look the same to an application.** Since
    /// 2026-10-05 the counterparty's leg of kinds 9 to 18 crosses the
    /// local interface and a witness's goes over the end-to-end path
    /// (design §7.1), and a screen showing one and not the other would be
    /// showing half a ceremony. So this ends where `deliver` ends: one
    /// `Dispatched::Conversation` on the application's channel, from the
    /// counterparty.
    ///
    /// A carriage that will not open is dropped and nothing is said to
    /// anyone, as an undecryptable payload is: there is no party to tell,
    /// since whoever sent it held no key.
    pub async fn take_carriage(&self, bytes: Vec<u8>) {
        let from = self.handle.with(|c| c.counterparty()).await;
        let Some(from) = from else { return };
        // a refusal is dropped, as the comment above says: the error is
        // not reported anywhere, because there is nobody it could be
        // reported to
        if let Ok(conversed) = self
            .handle
            .with(move |c| c.take_conversation_carriage(&bytes))
            .await
        {
            let _ = self.app.send((from, Dispatched::Conversation(conversed)));
        }
    }

    /// A verifier's answer, back to the querier on the end-to-end path and
    /// its copy to the subject over the association the grant established
    /// (`wire-format.md` §5.6, design §7.4.2).
    ///
    /// A send that fails is said, since the querier is left waiting for an
    /// answer this side gave: nothing more can be done for it from here,
    /// the path being the only one there is, but a run's record should
    /// show the answer went nowhere rather than that none was given.
    async fn answer_back(self: &Arc<Self>, querier: Keyhash, a: Answer) {
        let (subject, copy) = a.to_subject.clone();
        let qid = a.query_id;
        for (to, kind, bytes, leg) in [
            (querier, KIND_RESPONSE, a.to_querier, "querier"),
            (subject, KIND_RESPONSE_COPY, copy, "subject"),
        ] {
            match self.send(to, kind, bytes).await {
                Ok(c) if c.complete() => {}
                Ok(c) => tracing::warn!(
                    target: "pay",
                    qid = %hex8(&qid),
                    leg,
                    left = c.left.len(),
                    refused = c.refused.len(),
                    "pay.answer.unsent"
                ),
                Err(why) => tracing::warn!(
                    target: "pay",
                    qid = %hex8(&qid),
                    leg,
                    why = %why,
                    "pay.answer.unsent"
                ),
            }
        }
    }

    /// Let the grant buffer's bound pass for a waiting query: past it the
    /// client answers `unavailable`, the key never having come, and that
    /// answer goes back like any other.
    async fn expire_after(self: &Arc<Self>, qid: [u8; 32]) {
        let bound = self.handle.with(|c| c.cfg.verifier.grant_buffer_ms).await;
        let me = self.clone();
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(bound + 20)).await;
            let querier = me.awaiting.lock().unwrap().remove(&qid);
            let Some(querier) = querier else {
                return;
            };
            let due = me.handle.with(|c| c.expire()).await;
            if let Some(a) = due.into_iter().find(|a| a.query_id == qid) {
                me.answer_back(querier, a).await;
            }
        });
    }

    /// Attach to the serving node beside this client: what the client
    /// publishes, stocks and sweeps at attach, carried there.
    pub async fn attach_at(self: &Arc<Self>, node: Keyhash, population: Vec<Keyhash>) -> Carried {
        let msgs = self.handle.with(move |c| c.attach(node, &population)).await;
        // what this client taught its last node, taught to this one
        let frames = self.handle.with(|c| c.replay_frames()).await;
        let offered = frames.len();
        if offered > 0
            && let Some(s) = self.serving_for(&node)
        {
            let took = s.teach(frames).await;
            tracing::info!(target: "pay", offered, took, "pay.taught");
        }
        self.carry(msgs).await
    }

    /// [`Courier::attach_at`] the first node held, for a client that holds
    /// one; nothing where none is held.
    pub async fn attach(self: &Arc<Self>, population: Vec<Keyhash>) -> Carried {
        match self.first() {
            Some(s) => self.attach_at(s.me(), population).await,
            None => Carried::default(),
        }
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

    /// **This side's candidates for `peer`, as bytes for a carriage the
    /// caller owns** (design §12.6.3, "candidates may travel the ceremony's
    /// local interfaces"): gathered only where the path may be direct, and
    /// marked offered, so no second offer rides the relay behind them.
    /// None where nothing may be gathered.
    pub async fn offer_bytes(self: &Arc<Self>, peer: Keyhash) -> Option<Vec<u8>> {
        let cands = self.direct.gather(peer).await?;
        self.offered.lock().unwrap().insert(peer);
        Some(encode_candidates(&cands))
    }

    /// **Open the path a peer's candidates name**, the candidates having
    /// come by a carriage of the caller's rather than as a payload kind.
    /// Whether a path is now held.
    pub async fn open_offered(self: &Arc<Self>, peer: Keyhash, bytes: &[u8]) -> bool {
        let Ok(cands) = decode_candidates(bytes) else {
            return false;
        };
        self.direct.open(peer, cands).await
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

    /// **One request of `to`'s node, as nobody** (`wire-format.md` §9.1):
    /// in process where that node is beside this client, else on a
    /// connection under a key minted for it. The node asked and its reply.
    async fn minted_to(
        &self,
        to: Keyhash,
        request: u64,
        body: Vec<u8>,
    ) -> Option<(Keyhash, Vec<u8>)> {
        let reach = self.handle.with(move |c| c.reach_for(&to)).await;
        if let Upstream::Known { node, .. } = &reach.upstream
            && let Some(held) = self.serving_for(node)
            && let Some(reply) = held.minted(request, body.clone()).await
        {
            return Some((*node, reply));
        }
        // a node beside this client that does not itself serve `to` asks
        // the node beyond it that does
        for s in self.servings() {
            if s.serves(&to)
                && let Some(reply) = s.minted(request, body.clone()).await
            {
                return Some((s.me(), reply));
            }
        }
        let beyond = self.beyond.lock().unwrap().clone()?;
        let mut reach = reach;
        // **the serving node resolves where this client holds no node to
        // ask itself** (`wire-format.md` §7.7.1's default) [2026-11-15]
        if reach.ask.is_empty()
            && let Some(via) = self.via()
        {
            reach.ask.push(Askable {
                node: via,
                endpoints: Vec::new(),
                key_material: None,
            });
        }
        if let Some((up, reply)) = beyond.minted(to, reach, request, body.clone()).await {
            return Some((up.node, reply));
        }
        // **this client's own nodes are the fallback**
        // (`light-client-requirements.md` §5): where no node could be
        // resolved for `to`, each node this client holds a session with is
        // asked — as nobody still, on a connection of its own — whether it
        // serves `to`. A fetch takes the first that holds a key for them;
        // a sealed message goes only to the node whose key sealed it
        let sealed_at = self
            .handle
            .with(move |c| c.payload.node_keys.get(&to).map(|k| k.node))
            .await;
        for s in self.servings() {
            let node = s.me();
            if request == REQUEST_RELAY && sealed_at.is_some_and(|k| k != node) {
                continue;
            }
            let Some(reply) = beyond.minted_at(node, request, body.clone()).await else {
                continue;
            };
            if request == REQUEST_NODE_KEY
                && !rhtn_archive::node_key::NodeKeyReply::decode(&reply)
                    .is_ok_and(|r| r.outcome.is_ok())
            {
                continue;
            }
            return Some((node, reply));
        }
        None
    }

    /// **Fetch the node keys of `recipients`** (`wire-format.md` §7.8): each
    /// from the node that serves it, as nobody, and what was waiting for
    /// them sealed. A batch request where one node is asked about several
    /// is the sweep's business; a fetch at send time names one.
    ///
    /// **A fetch that left a message waiting is unsent work**
    /// (`light-client-requirements.md` §9): nobody answered, or the answer
    /// carried no key. Its recipient comes back beside what was sealed, so
    /// the caller reports the message unsent rather than taken. A sweep's
    /// fetch with nothing waiting says nothing.
    async fn fetch_node_keys(&self, recipients: Vec<Keyhash>) -> (Vec<Msg>, Vec<Keyhash>) {
        let mut out = Vec::new();
        let mut unsent = Vec::new();
        for to in recipients {
            let Some(req) =
                NodeKeyRequest::for_recipients(vec![to], rhtn_transport::tls::random_bytes())
            else {
                continue;
            };
            let Some((node, reply)) = self.minted_to(to, REQUEST_NODE_KEY, req.encode()).await
            else {
                tracing::info!(target: "pay", to = %hex4(&to), "pay.node_key.unanswered");
                let waiting = self
                    .handle
                    .with(move |c| c.payload.pending.get(&to).is_some_and(|w| !w.is_empty()))
                    .await;
                if waiting {
                    unsent.push(to);
                }
                continue;
            };
            let asked = req.recipients();
            let (sealed, waiting) = self
                .handle
                .with(move |c| {
                    let sealed = c.take_node_key_reply(&node, &asked, &reply);
                    let waiting = c.payload.pending.get(&to).is_some_and(|w| !w.is_empty());
                    (sealed, waiting)
                })
                .await;
            out.extend(sealed);
            // answered, and with no key for them: as unsent as no answer
            if waiting {
                unsent.push(to);
            }
        }
        (out, unsent)
    }

    /// **Hand a sealed message to `to`'s node** (`wire-format.md` §7.10), as
    /// nobody. The code it answered with, or none where nobody answered.
    async fn submit_sealed(&self, to: Keyhash, sealed: Vec<u8>) -> Option<u64> {
        // **one request frame**, bounded at 256 KiB (`wire-format.md` §9.2):
        // a body past it would be dropped unread with no reply, so it is
        // said here and not attempted
        if sealed.len() + 256 > rhtn_codec::bounds::REQUEST_FRAME_BYTES {
            tracing::info!(
                target: "pay",
                to = %hex4(&to),
                bytes = sealed.len(),
                reason = "over the request frame bound",
                "pay.sealed.unsent"
            );
            return None;
        }
        let nonce = rhtn_transport::tls::random_bytes();
        let body = RelaySubmission {
            recipient: to,
            sealed,
            nonce,
        }
        .encode();
        let (_, reply) = self.minted_to(to, REQUEST_RELAY, body).await?;
        let r = SubmissionReply::decode(&reply).ok()?;
        (r.nonce == nonce).then_some(r.code)
    }

    /// Carry the client's messages where the seams go: payload on the
    /// direct path where it is held, sealed messages and node-key fetches
    /// to the recipient's node as nobody.
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
                    // **the profiles go to every node held**: a peer in any
                    // of this client's subnets fetches from the node
                    // serving it there [author, 2026-11-11]
                    Msg::PublishProfile(b) => {
                        if !me.publish_profile_all(&b).await {
                            out.refused.push(Msg::PublishProfile(b));
                        }
                    }
                    // **a row refused is not retried** (`wire-format.md`
                    // §7.12): the node refuses one it already holds at
                    // this revision, one from a sender speaking past its
                    // downline, and one it cannot pin — none of which a
                    // second attempt changes. The next attach sends it
                    // again at a later revision, which is the repair.
                    Msg::AssertHorizon { bytes, nonce } => {
                        let took = me.assert_horizon_all(&bytes, nonce).await;
                        tracing::debug!(target: "pay", took, "pay.horizon");
                    }
                    // a profile is fetched from where the subject's locator
                    // leads (`wire-format.md` §7.11): the subject's own node
                    // first, and this client's where that one could not be
                    // reached
                    Msg::ProfileRequest(b) => {
                        let answered = match me.their_profile(&b).await {
                            Some(reply) => Some(reply),
                            None => me.profile_first(&b).await,
                        };
                        match answered {
                            Some(reply) => {
                                let _ = me.handle.with(move |c| c.take_profile_reply(&reply)).await;
                            }
                            None => out.refused.push(Msg::ProfileRequest(b)),
                        }
                    }
                    Msg::NodeKeyRequest(recipients) => {
                        let (sealed, unsent) = me.fetch_node_keys(recipients).await;
                        out.absorb(me.carry(sealed).await);
                        if !unsent.is_empty() {
                            out.left.push(Msg::NodeKeyRequest(unsent));
                        }
                    }
                    Msg::Payload { to, bytes } => {
                        // a delivery short of complete leaves the message
                        // with the sender, and the seal for the recipient's
                        // node carries it (design §12.6.3)
                        if !me.direct.deliver(to, bytes.clone()).await {
                            match me.handle.with(move |c| c.reseal(to, bytes)).await {
                                Ok(more) => out.absorb(me.carry(more).await),
                                Err(why) => {
                                    tracing::info!(target: "pay", to = %hex4(&to), %why, "pay.direct.unsealed");
                                }
                            }
                        }
                        // **the ceremony's own legs offer the direct path
                        // too** [author, 2026-10-24]: a witness or a verifier
                        // is a contact, and the first message to one not yet
                        // reached directly is followed by this side's
                        // candidates. **After the message, never ahead of it**
                        if !me.direct.reachable().holds(&to) && !me.offered(&to) {
                            me.offer(to).await;
                        }
                    }
                    // **a sealed message to the recipient's node**
                    // (`wire-format.md` §7.10). Refused for a stale node key,
                    // the newest is fetched and it is sealed again, once
                    // (§7.8): a second refusal is the node's answer
                    Msg::Sealed { to, sealed, plain } => {
                        let code = me.submit_sealed(to, sealed.clone()).await;
                        let took = match code {
                            Some(SUBMISSION_ACCEPTED) => true,
                            Some(SUBMISSION_REFUSED) => {
                                let plain = plain.clone();
                                let again = me
                                    .handle
                                    .with(move |c| {
                                        c.node_key_refused(&to);
                                        c.reseal(to, plain)
                                    })
                                    .await
                                    .unwrap_or_default();
                                let mut resealed = Vec::new();
                                for m in again {
                                    match m {
                                        Msg::NodeKeyRequest(r) => {
                                            resealed.extend(me.fetch_node_keys(r).await.0)
                                        }
                                        other => resealed.push(other),
                                    }
                                }
                                let mut took = !resealed.is_empty();
                                for m in resealed {
                                    if let Msg::Sealed { to, sealed, .. } = m {
                                        took &= me.submit_sealed(to, sealed).await
                                            == Some(SUBMISSION_ACCEPTED);
                                    }
                                }
                                took
                            }
                            _ => false,
                        };
                        tracing::info!(
                            target: "pay",
                            to = %hex4(&to),
                            bytes = sealed.len(),
                            took,
                            code = ?code,
                            "pay.sealed"
                        );
                        if !took {
                            out.refused.push(Msg::Sealed { to, sealed, plain });
                        }
                        // the offer follows the message, as above
                        if !me.offered(&to) {
                            me.offer(to).await;
                        }
                    }
                    // **a resource is reached through its host** (§11.1):
                    // the node that served the entry is the node asked,
                    // and the answer goes to the caller rather than into
                    // any state of this client's
                    Msg::ResourceRequest { node, bytes } => {
                        let answered = me.resource_at(&node, &bytes).await;
                        match answered {
                            Some(reply) => out.answers.push(reply),
                            None => out.refused.push(Msg::ResourceRequest { node, bytes }),
                        }
                    }
                    // a sweep of the serving node's catalog: each reply
                    // taken by the client, which says whether to ask again
                    // (`light-client-requirements.md` §8)
                    Msg::CatalogQuery(b) => {
                        // the sweep names the node it reads, and the query
                        // goes over the session held with that node
                        let at = me.handle.with(|c| c.browsing_at()).await;
                        let answered = match at.and_then(|n| me.serving_for(&n)) {
                            Some(s) => s.catalog(me.me(), &b).await,
                            None => None,
                        };
                        match answered {
                            Some(reply) => {
                                let more =
                                    me.handle.with(move |c| c.take_catalog_reply(&reply)).await;
                                if let Ok((_, more)) = more {
                                    out.absorb(me.carry(more).await);
                                }
                            }
                            None => out.refused.push(Msg::CatalogQuery(b)),
                        }
                    }
                    // **a post this client wrote**, on the session held
                    // with the node serving it there (`wire-format.md`
                    // §10.3): a frame with no reply
                    Msg::BoardPost { node, bytes } => {
                        let sent = match me.serving_for(&node) {
                            Some(s) => s.board_post(me.me(), &bytes).await,
                            None => false,
                        };
                        tracing::debug!(target: "pay", node = %hex4(&node), sent, "pay.board.post");
                        if !sent {
                            out.refused.push(Msg::BoardPost { node, bytes });
                        }
                    }
                    // **what this client missed there**: the reply taken,
                    // and the next asking carried where the node holds more
                    Msg::BoardAsk { node, bytes } => {
                        let answered = match me.serving_for(&node) {
                            Some(s) => s.board(me.me(), &bytes).await,
                            None => None,
                        };
                        match answered {
                            Some(reply) => {
                                let more = me
                                    .handle
                                    .with(move |c| c.take_board_reply(node, &reply))
                                    .await;
                                tracing::debug!(target: "pay", node = %hex4(&node), more = ?more.as_ref().map(Vec::len), "pay.board.caught_up");
                                if let Ok(more) = more
                                    && !more.is_empty()
                                {
                                    out.absorb(me.carry(more).await);
                                }
                            }
                            None => out.refused.push(Msg::BoardAsk { node, bytes }),
                        }
                    }
                    // the page beside it, over the session held with the
                    // node it names (`wire-format.md` §6.9)
                    Msg::CatalogPage { node, bytes } => {
                        let answered = match me.serving_for(&node) {
                            Some(s) => s.page(me.me(), &bytes).await,
                            None => None,
                        };
                        match answered {
                            Some(reply) => {
                                let took = me
                                    .handle
                                    .with(move |c| c.take_catalog_page(node, &reply))
                                    .await;
                                tracing::debug!(target: "pay", node = %hex4(&node), took = ?took, "pay.catalog.page");
                            }
                            None => out.refused.push(Msg::CatalogPage { node, bytes }),
                        }
                    }
                    // **a client originates and does not forward.** The
                    // records it makes are its own, and the one party that
                    // can put them into the flood is the node serving it
                    // (`wire-format.md` §10.1.1 counts an attached client
                    // an adjacency, which is the same edge read the other
                    // way).
                    Msg::Record(b) => {
                        if !me.propagate_all(&b).await {
                            out.refused.push(Msg::Record(b));
                        }
                    }
                    // a query to its verifier on the end-to-end path
                    // (`wire-format.md` §5.6): the body names the verifier
                    // it may reach, and the route is payload's (design
                    // §12.6.3) — direct where held, the relay otherwise
                    Msg::Query(b) => match QueryRequest::decode(&b) {
                        Ok(req) => match me.send(req.query.verifier, KIND_QUERY, b.clone()).await {
                            Ok(c) => out.absorb(c),
                            Err(_) => out.refused.push(Msg::Query(b)),
                        },
                        Err(_) => out.refused.push(Msg::Query(b)),
                    },
                    other => out.left.push(other),
                }
            }
            // **what the node would not take, by name** [2026-11-04]: until
            // here a refusal said only how many — so which message it was had to be
            // guessed at from the outside, which is what it cost an
            // evening to do. The kind, never the bytes.
            if !out.refused.is_empty() {
                let which: Vec<&str> = out.refused.iter().map(rhtn_client::diag::msg).collect();
                tracing::info!(target: "pay", refused = ?which, "pay.refused");
            }
            out
        })
    }
}

/// Eight hex characters of an identifier, for a diagnostic line.
fn hex8(b: &[u8]) -> String {
    b.iter().take(4).map(|x| format!("{x:02x}")).collect()
}

/// Four bytes of a keyhash, for an event.
fn hex4(k: &Keyhash) -> String {
    k[..4].iter().map(|b| format!("{b:02x}")).collect()
}

#[cfg(test)]
mod awaiting_tests {
    use super::*;

    /// The waiting queries are bounded, the oldest forgotten first, and a
    /// query asked again keeps its place rather than evicting another.
    #[test]
    fn the_waiting_queries_are_bounded_and_the_oldest_goes_first() {
        let mut w = Awaiting::default();
        let qid = |i: u32| {
            let mut q = [0u8; 32];
            q[..4].copy_from_slice(&i.to_be_bytes());
            q
        };
        let who = [7u8; 32];
        for i in 0..AWAITING_QUERIES as u32 {
            w.insert(qid(i), who);
        }
        assert_eq!(w.len(), AWAITING_QUERIES);
        w.insert(qid(0), who);
        assert_eq!(w.len(), AWAITING_QUERIES, "a repeat keeps its slot");
        w.insert(qid(AWAITING_QUERIES as u32), who);
        assert_eq!(w.len(), AWAITING_QUERIES);
        assert!(w.remove(&qid(1)).is_none(), "the oldest is gone");
        assert!(
            w.remove(&qid(0)).is_some(),
            "the repeat was refreshed and stays"
        );
        assert!(w.remove(&qid(AWAITING_QUERIES as u32)).is_some());
    }
}
