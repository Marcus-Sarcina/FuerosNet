//! The serving node a light client reaches over a session
//! (`wire-format.md` §7.10): the same five questions [`Serving`] asks of
//! the node beside it, put on the wire as request types 9 to 12 and the
//! prekey fetch of §7.8.
//!
//! This is the implementation a light client on its own host runs.  The
//! one beside a node in the same process answers from memory; this one
//! waits for the node to say.

use crate::actor::Handle;
use crate::serving::{Answer, Inbound, Serving};
use rhtn_archive::Keyhash;
use rhtn_archive::prekey::REQUEST_PREKEY;
use rhtn_archive::submission::*;
use std::sync::Arc;

/// Where an attached node's nonces come from.  Nothing in this workspace
/// reads the operating system's randomness: every source is the host's to
/// supply, and a client already holds one.
pub type Nonces = Arc<dyn Fn() -> [u8; 16] + Send + Sync>;

/// A session with the node that serves this client.  Held behind a
/// closure rather than owned, so a client that reattaches after a failure
/// keeps talking to whatever session is current
/// (`light-client-requirements.md` §2).
pub type Current = Arc<dyn Fn() -> Option<Arc<rhtn_transport::session::Session>> + Send + Sync>;

pub struct AttachedNode {
    /// The node's own keyhash, which a client knows before it attaches.
    node: Keyhash,
    session: Current,
    nonce: Nonces,
}

impl AttachedNode {
    pub fn new(node: Keyhash, session: Current, nonce: Nonces) -> Arc<AttachedNode> {
        Arc::new(AttachedNode { node, session, nonce })
    }

    /// One request on the current session, or nothing where there is none.
    async fn ask(&self, request_type: u64, body: Vec<u8>) -> Option<Vec<u8>> {
        let s = (self.session)()?;
        s.request(request_type, &body).await.ok()
    }

    /// One submission: accepted, or not.  A refusal and a bound are both
    /// "the node did not take it"; which one it was is the reply's code,
    /// and a caller that needs to tell them apart asks for the reply.
    async fn submit(&self, request_type: u64, nonce: [u8; 16], body: Vec<u8>) -> bool {
        matches!(self.reply(request_type, nonce, body).await, Some(SUBMISSION_ACCEPTED))
    }

    /// The code the node answered with, where it answered at all and the
    /// answer was to this request.  **An answer carrying another
    /// request's nonce is discarded**: the nonce is what ties a reply to
    /// what was sent, and a reply that does not echo it says nothing
    /// about this submission.
    pub async fn reply(&self, request_type: u64, nonce: [u8; 16], body: Vec<u8>) -> Option<u64> {
        let bytes = self.ask(request_type, body).await?;
        let r = SubmissionReply::decode(&bytes).ok()?;
        (r.nonce == nonce).then_some(r.code)
    }
}

impl Serving for AttachedNode {
    fn me(&self) -> Keyhash {
        self.node
    }

    /// A node across a session tells nobody what it holds; a client finds
    /// out by fetching (`wire-format.md` §7.8).
    fn holds(&self, _subject: &Keyhash) -> bool {
        false
    }

    /// Nor whom it serves: a submission for a keyhash it holds no record
    /// of comes back refused, which is where a client learns it
    /// (`wire-format.md` §7.10).
    fn serves(&self, _client: &Keyhash) -> bool {
        false
    }

    fn publish<'a>(&'a self, bytes: &'a [u8]) -> Answer<'a, bool> {
        Box::pin(async move {
            let nonce = (self.nonce)();
            let body = PrekeyPublication { bundle: bytes.to_vec(), nonce }.encode();
            self.submit(REQUEST_PREKEY_PUBLICATION, nonce, body).await
        })
    }

    fn stock<'a>(&'a self, _subject: Keyhash, keys: Vec<Vec<u8>>) -> Answer<'a, bool> {
        Box::pin(async move {
            // the subject of a deposit is the client that sends it, which
            // the node takes from the session and not from the body.  A
            // deposit of nothing is not a message: the wire's smallest is
            // one key (`wire-format.md` §7.10), and there is nothing here
            // for the node to refuse
            if keys.is_empty() {
                return true;
            }
            let nonce = (self.nonce)();
            let body = OneTimeDeposit { keys, nonce }.encode();
            self.submit(REQUEST_ONE_TIME_DEPOSIT, nonce, body).await
        })
    }

    fn prekey<'a>(&'a self, _from: Keyhash, body: &'a [u8]) -> Answer<'a, Option<Vec<u8>>> {
        Box::pin(async move { self.ask(REQUEST_PREKEY, body.to_vec()).await })
    }

    fn relay<'a>(&'a self, _from: Keyhash, to: Keyhash, bytes: Vec<u8>) -> Answer<'a, bool> {
        Box::pin(async move {
            let nonce = (self.nonce)();
            let body = RelaySubmission { recipient: to, ciphertext: bytes, nonce }.encode();
            self.submit(REQUEST_RELAY, nonce, body).await
        })
    }

    fn wake<'a>(&'a self, _client: Keyhash, endpoint: Option<WakeEndpoint>) -> Answer<'a, bool> {
        Box::pin(async move {
            let nonce = (self.nonce)();
            let body = WakeRegistration::of(nonce, endpoint).encode();
            self.submit(REQUEST_WAKE, nonce, body).await
        })
    }
}

/// Carry what the serving node propagates into the client's horizon
/// (design §15.1.1).
///
/// A light client learns its neighbourhood from stream 0, the same flood a
/// node forwards (`wire-format.md` §10.1), and nothing else tells it.  The
/// task ends when the session does, which is the same lifetime the copy's
/// currency has: the next attach brings whatever arrived meanwhile.
///
/// **What is propagated wins.** Everything the client holds here is a copy
/// of what came down this channel, so an ingest never asks whether the
/// local copy disagreed.
pub fn follow(handle: Handle, mut frames: tokio::sync::mpsc::UnboundedReceiver<(u64, Vec<u8>)>) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        while let Some((frame_type, body)) = frames.recv().await {
            // stream 0 carries memos too, and a memo is a node's routing
            // aid rather than a client's (`wire-format.md` §10.2)
            if frame_type != FRAME_TOPOLOGY_PUSH {
                continue;
            }
            let Ok((kind, object)) = rhtn_node::propagation::decode_push(&body) else { continue };
            if kind != rhtn_node::store::KIND_TRANSACTION {
                continue;
            }
            handle
                .with(move |c| {
                    let known = c.known.clone();
                    c.horizon.ingest(&object, &known);
                })
                .await;
        }
    })
}

/// `TopologyPush` (`wire-format.md` §8.2).
pub const FRAME_TOPOLOGY_PUSH: u64 = 5;

/// Carry what the serving node delivers into the client.
///
/// Each message is a `RelayedPayload` (`wire-format.md` §7.10): the
/// submitter in front of the ciphertext.  **The name is a routing hint**,
/// which is all the client uses it for — whose material to try — and never
/// an attribution, which the material the message opens under decides.
/// Bytes that are not that shape are dropped, since nothing can be done
/// with a message whose sender is unknown.
pub fn collect(inbound: Inbound, mut deliveries: tokio::sync::mpsc::UnboundedReceiver<Vec<u8>>) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        while let Some(bytes) = deliveries.recv().await {
            if let Some((from, payload)) = rhtn_archive::submission::unrelayed(&bytes) {
                inbound(from, payload);
            }
        }
    })
}
