//! A verifier hosted in this process, answering on the request stream of
//! the node hosting it (`wire-format.md` §9.2's request type 4, §5.6).
//!
//! The documents route a query to its verifier's serving node
//! (`wire-format.md` §7.7.2) and stop there: how that node carries it to a
//! client attached over the wire is unwritten.  What can be answered is
//! what the process holds: a node that is a participant answers for its
//! own key, and a light client beside its serving node answers for its.
//! Each is a client behind a handle, registered under its keyhash.

use crate::actor::Handle;
use crate::courier::Courier;
use rhtn_archive::Keyhash;
use rhtn_client::payload::KIND_RESPONSE_COPY;
use rhtn_client::query::QueryRequest;
use rhtn_client::verifier::{Answer, QueryOutcome};
use rhtn_node::runtime::{LiveNode, LocalVerifier};
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::sync::oneshot;

/// A request waiting for its grant: the channel that carries the answer
/// back to its stream, under the ticket of the request that registered it.
type Waiting = (u64, oneshot::Sender<Answer>);

#[derive(Clone)]
struct Hosted {
    handle: Handle,
    courier: Arc<Courier>,
}

/// The verifiers this process hosts, by keyhash.
#[derive(Default)]
pub struct Verifiers {
    hosted: Mutex<HashMap<Keyhash, Hosted>>,
    /// Queries waiting for their grant, by id, each under the ticket of
    /// the request that registered it: one request's cleanup must not
    /// remove another's channel, and a second request must not displace
    /// the first.
    waiting: Mutex<HashMap<[u8; 32], Waiting>>,
    tickets: AtomicU64,
}

impl Verifiers {
    pub fn new() -> Arc<Verifiers> {
        Arc::default()
    }

    /// Host a client as a verifier.  Its courier carries the subject's
    /// copy, and brings the grants that answer back here.
    pub fn host(self: &Arc<Self>, handle: Handle, courier: Arc<Courier>) {
        courier.report_grants_to(self.clone());
        self.hosted
            .lock()
            .unwrap()
            .insert(handle.me(), Hosted { handle, courier });
    }

    /// Answer `node`'s request type 4 from here.
    pub fn install(self: &Arc<Self>, node: &LiveNode) {
        let v = self.clone();
        let answer: LocalVerifier = Arc::new(move |peer, body| {
            let v = v.clone();
            Box::pin(async move { v.answer(peer, body).await })
        });
        *node.verifier.lock().unwrap() = Some(answer);
    }

    /// A type-4 body from the authenticated `peer`: the verifier field 7
    /// names answers, if hosted here.  The response for the stream, or
    /// nothing, which fails it.
    pub async fn answer(self: Arc<Self>, peer: Keyhash, body: Vec<u8>) -> Option<Vec<u8>> {
        let req = QueryRequest::decode(&body).ok()?;
        let hosted = self
            .hosted
            .lock()
            .unwrap()
            .get(&req.query.verifier)
            .cloned()?;
        let qid = req.query.query_id();
        // Field 2 names the authenticated requester (`wire-format.md`
        // §5.6), and the check happens before this request touches state
        // another request owns.  The client checks it again and is the
        // authority; what it cannot do is undo a registration made in its
        // name before it was consulted.
        if req.query.querier != peer {
            return None;
        }
        // The stream waits here should the grant come later, under a
        // ticket of its own: a second request for the same query finds one
        // waiting and displaces nothing.
        let ticket = self.tickets.fetch_add(1, Ordering::Relaxed);
        let (tx, rx) = oneshot::channel();
        let registered = {
            let mut w = self.waiting.lock().unwrap();
            if let std::collections::hash_map::Entry::Vacant(e) = w.entry(qid) {
                e.insert((ticket, tx));
                true
            } else {
                false
            }
        };
        let release = |me: &Self| {
            if !registered {
                return;
            }
            let mut w = me.waiting.lock().unwrap();
            if w.get(&qid).is_some_and(|(t, _)| *t == ticket) {
                w.remove(&qid);
            }
        };
        let outcome = hosted.handle.with(move |c| c.take_query(peer, &body)).await;
        let answer = match outcome {
            QueryOutcome::Closed(_) => {
                release(&self);
                return None;
            }
            QueryOutcome::Answered(a) => {
                release(&self);
                a
            }
            QueryOutcome::AwaitingGrant => {
                let bound = hosted.handle.with(|c| c.cfg.verifier.grant_buffer_ms).await;
                match tokio::time::timeout(Duration::from_millis(bound + 20), rx).await {
                    Ok(Ok(a)) => a,
                    _ => {
                        // the bound passed: the client's own expiry answers
                        // `unavailable`, the key never having come
                        release(&self);
                        let due = hosted.handle.with(|c| c.expire()).await;
                        due.into_iter().find(|a| a.query_id == qid)?
                    }
                }
            }
        };
        // the copy to the subject, over the association the grant
        // established (design §7.4.2)
        let (subject, copy) = answer.to_subject.clone();
        let _ = hosted.courier.send(subject, KIND_RESPONSE_COPY, copy).await;
        Some(answer.to_querier)
    }

    /// A grant answered a query whose stream was waiting.
    pub fn granted(&self, answer: Answer) {
        let waiting = self.waiting.lock().unwrap().remove(&answer.query_id);
        if let Some((_, tx)) = waiting {
            let _ = tx.send(answer);
        }
    }
}
