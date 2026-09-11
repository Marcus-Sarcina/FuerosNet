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
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::sync::oneshot;

#[derive(Clone)]
struct Hosted {
    handle: Handle,
    courier: Arc<Courier>,
}

/// The verifiers this process hosts, by keyhash.
#[derive(Default)]
pub struct Verifiers {
    hosted: Mutex<HashMap<Keyhash, Hosted>>,
    /// Queries waiting for their grant, by id: the stream waits with them.
    waiting: Mutex<HashMap<[u8; 32], oneshot::Sender<Answer>>>,
}

impl Verifiers {
    pub fn new() -> Arc<Verifiers> {
        Arc::default()
    }

    /// Host a client as a verifier.  Its courier carries the subject's
    /// copy, and brings the grants that answer back here.
    pub fn host(self: &Arc<Self>, handle: Handle, courier: Arc<Courier>) {
        courier.report_grants_to(self.clone());
        self.hosted.lock().unwrap().insert(handle.me(), Hosted { handle, courier });
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
        let hosted = self.hosted.lock().unwrap().get(&req.query.verifier).cloned()?;
        let qid = req.query.query_id();
        // the stream waits here should the grant come later: registered
        // before the client sees the query, so no grant slips between
        let (tx, rx) = oneshot::channel();
        self.waiting.lock().unwrap().insert(qid, tx);
        let outcome = hosted.handle.with(move |c| c.take_query(peer, &body)).await;
        let answer = match outcome {
            QueryOutcome::Closed(_) => {
                self.waiting.lock().unwrap().remove(&qid);
                return None;
            }
            QueryOutcome::Answered(a) => {
                self.waiting.lock().unwrap().remove(&qid);
                a
            }
            QueryOutcome::AwaitingGrant => {
                let bound = hosted.handle.with(|c| c.cfg.verifier.grant_buffer_ms).await;
                match tokio::time::timeout(Duration::from_millis(bound + 20), rx).await {
                    Ok(Ok(a)) => a,
                    _ => {
                        // the bound passed: the client's own expiry answers
                        // `unavailable`, the key never having come
                        self.waiting.lock().unwrap().remove(&qid);
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
        if let Some(tx) = self.waiting.lock().unwrap().remove(&answer.query_id) {
            let _ = tx.send(answer);
        }
    }
}
