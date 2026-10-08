//! The courier: what the client says goes out over the seams, and what
//! arrives on them goes in.

use crate::actor::Handle;
use crate::beyond::Beyond;
use crate::direct::Direct;
use crate::serving::{Inbound, Serving};
use crate::verifier::Verifiers;
use rhtn_archive::Keyhash;
use rhtn_archive::prekey::{PrekeyRequest, REQUEST_PREKEY};
use rhtn_client::ceremony::{Dispatched, Msg};
use rhtn_client::payload::{KIND_CANDIDATES, KIND_QUERY, KIND_RESPONSE, KIND_RESPONSE_COPY};
use rhtn_client::query::QueryRequest;
use rhtn_client::verifier::{Answer, GrantOutcome, QueryOutcome};
use rhtn_transport::traversal::{decode_candidates, encode_candidates};
use std::collections::{HashMap, HashSet};
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
        Arc::new(move |from, bytes, binding| {
            let f = slot.lock().unwrap().clone();
            if let Some(f) = f {
                f(from, bytes, binding);
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

/// One delivery as the node or the direct path hands it over: who from,
/// the bytes, and the binding the node carried where it carried one.
type Delivery = (Keyhash, Vec<u8>, Option<Vec<u8>>);

/// What carries the client's messages: the serving node for what it
/// relays, the direct path where one is held, and the client's own thread
/// behind `handle`.
pub struct Courier {
    /// The client, on its own thread.
    pub handle: Handle,
    /// The serving node, for what it publishes, stocks and relays.
    pub serving: Arc<dyn Serving>,
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
        serving: Arc<dyn Serving>,
        direct: Arc<dyn Direct>,
    ) -> (Arc<Courier>, mpsc::UnboundedReceiver<(Keyhash, Dispatched)>) {
        let (app, rx) = mpsc::unbounded_channel();
        (
            Arc::new(Courier {
                handle,
                serving,
                direct,
                beyond: Mutex::new(None),
                inbox: Mutex::new(None),
                app,
                verifiers: Mutex::new(None),
                offered: Mutex::new(HashSet::new()),
                awaiting: Mutex::new(Awaiting::default()),
            }),
            rx,
        )
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

    /// Hand payload to the node that serves `to`, where this client can
    /// say who that is and reach them.
    ///
    /// **The recipient's node, not this client's** [author, 2026-10-05]:
    /// it is the party that queues for its own client while the client is
    /// away (design §14.1.4), and reaching it needs no cooperation from
    /// the sender's patron, which is what makes a route obtained in person
    /// survive an uncooperative one (design §12.6.3, §6.4).
    async fn to_their_node(&self, to: Keyhash, bytes: &[u8], device: [u8; 32]) -> bool {
        let Some(beyond) = self.beyond.lock().unwrap().clone() else {
            return false;
        };
        let reach = self.handle.with(move |c| c.reach_for(&to)).await;
        beyond
            .carry(to, device, bytes.to_vec(), reach, Some(self.serving.me()))
            .await
    }

    /// A targeted prekey fetch, put to the node that serves the subject.
    ///
    /// **A node answers prekey requests from what was published to it**
    /// (`wire-format.md` §7.8), so the node that holds a subject's bundle
    /// is the node that subject attached to.  A client asking its own
    /// patron for a peer served elsewhere is asking a party that cannot
    /// know, and under the ruling of 2026-10-05 it is also asking the one
    /// party the ceremony must not depend on.
    ///
    /// **It discloses less, not more.** §7.8 states that requesting a
    /// one-time key discloses the intent to message that subject; put
    /// here, that intent reaches the subject's own node, which is about to
    /// be handed the message anyway, instead of this client's patron
    /// (design §19.4, P26).
    ///
    /// The sweep is not routed this way: a `PrekeyBatchRequest` names a
    /// whole population and is answered from what one node holds.
    async fn their_prekeys(&self, body: &[u8]) -> Option<Vec<u8>> {
        let beyond = self.beyond.lock().unwrap().clone()?;
        let subject = match PrekeyRequest::decode(body).ok()? {
            PrekeyRequest::One { subject, .. } => subject,
            PrekeyRequest::Batch { .. } => return None,
        };
        let reach = self.handle.with(move |c| c.reach_for(&subject)).await;
        beyond
            .ask_their_node(
                subject,
                reach,
                Some(self.serving.me()),
                REQUEST_PREKEY,
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
                        while let Some((from, bytes, binding)) = rx.recv().await {
                            let Some(me) = weak.upgrade() else { break };
                            me.receive(from, bytes, binding).await;
                        }
                    });
                    *slot = Some(tx.clone());
                    tx
                }
            }
        };
        Arc::new(move |from, bytes, binding| {
            // a courier whose task has gone is one nobody holds any more
            let _ = tx.send((from, bytes, binding));
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
        let (d, owed) = self
            .handle
            .with(move |c| {
                if let Some(b) = binding {
                    c.take_binding(&b);
                }
                let d = c.receive_payload(from, &bytes);
                // what the client owes in answer goes now, on this courier:
                // the conversation's replies (`wire-format.md` §7.10.1), an
                // archive reply, a record to propagate
                (d, c.outbox())
            })
            .await;
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
            // undecryptable: dropped, and nothing said to anyone
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

    /// The relayed path, in the order design §12.6.3 takes it: **the
    /// recipient's own node first**, and the node serving this client as
    /// the fallback.
    ///
    /// **Why the sender's node is still a term.** The ruling that put the
    /// recipient's node first also kept the sender's nearest infra node as
    /// a relay for a client that cannot open an outbound connection at all
    /// — one behind a proxy it must traverse [author, 2026-10-05]. That is
    /// a traversal relay rather than a carriage, and this workspace has no
    /// separate traversal path to put it on, so it stands where the
    /// store-and-forward fallback already stood. What changed is that it is
    /// no longer the *first* answer, and so no longer a party whose
    /// cooperation a sender needs.
    async fn relayed(&self, to: Keyhash, bytes: &[u8], device: [u8; 32]) -> bool {
        self.to_their_node(to, bytes, device).await
            || self
                .serving
                .relay(self.me(), to, bytes.to_vec(), device)
                .await
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
                        // the subject's own node first, and this client's
                        // where that one could not be reached
                        let answered = match me.their_prekeys(&b).await {
                            Some(reply) => Some(reply),
                            None => me.serving.prekey(me.me(), &b).await,
                        };
                        if let Some(reply) = answered {
                            let more = me.handle.with(move |c| c.take_prekey_reply(&reply)).await;
                            if let Ok(more) = more {
                                out.absorb(me.carry(more).await);
                            }
                        }
                    }
                    Msg::Payload { to, bytes, device } => {
                        // a delivery short of complete leaves the message
                        // with the sender, and a relayed path carries it
                        // (design §14.1.1): direct where a path is held,
                        // then the three terms below, nobody's choice
                        if !me.direct.deliver(to, bytes.clone()).await
                            && !me.relayed(to, &bytes, device).await
                        {
                            out.refused.push(Msg::Payload { to, bytes, device });
                        }
                    }
                    // the client already said no direct path is held, so
                    // the relayed path is the whole of what is left
                    Msg::Relay { to, bytes, device } => {
                        if !me.relayed(to, &bytes, device).await {
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

/// Eight hex characters of an identifier, for a diagnostic line.
fn hex8(b: &[u8]) -> String {
    b.iter().take(4).map(|x| format!("{x:02x}")).collect()
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
