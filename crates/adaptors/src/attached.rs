//! The serving node a light client reaches over a session
//! (`wire-format.md` §7.10): the same five questions [`Serving`] asks of
//! the node beside it, put on the wire as the request types of §7.10 and
//! §7.12. A node-key fetch and a sealed message go as nobody, on a
//! connection minted for them, and never on this session (§9.1).
//!
//! This is the implementation a light client on its own host runs.  The
//! one beside a node in the same process answers from memory; this one
//! waits for the node to say.

use crate::actor::Handle;
use crate::serving::{Answer, Inbound, Serving};
use rhtn_archive::Keyhash;
use rhtn_archive::catalog::REQUEST_RESOURCE;
use rhtn_archive::profile::REQUEST_PROFILE;
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

/// A serving node beside this client in one process: what an attach
/// reaches when the node is not across the wire.
pub struct AttachedNode {
    /// The node's own keyhash, which a client knows before it attaches;
    /// replaced by a failover, since the session it holds is then with a
    /// sibling and what it asks is answered by that sibling.
    node: std::sync::Mutex<Keyhash>,
    session: Current,
    nonce: Nonces,
}

impl AttachedNode {
    /// An attached node for `node`, answering on `session`.
    pub fn new(node: Keyhash, session: Current, nonce: Nonces) -> Arc<AttachedNode> {
        Arc::new(AttachedNode {
            node: std::sync::Mutex::new(node),
            session,
            nonce,
        })
    }

    /// The session moved to `node`: everything asked from here on is asked
    /// of it and attributed to it.
    pub fn moved_to(&self, node: Keyhash) {
        *self.node.lock().unwrap() = node;
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
        matches!(
            self.reply(request_type, nonce, body).await,
            Some(SUBMISSION_ACCEPTED)
        )
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
        *self.node.lock().unwrap()
    }

    /// Nor whom it serves: a submission for a keyhash it holds no record
    /// of comes back refused, which is where a client learns it
    /// (`wire-format.md` §7.10).
    fn serves(&self, _client: &Keyhash) -> bool {
        false
    }

    fn publish_profile<'a>(&'a self, bytes: &'a [u8]) -> Answer<'a, bool> {
        Box::pin(async move {
            let nonce = (self.nonce)();
            let body = ProfilePublication {
                profile: bytes.to_vec(),
                nonce,
            }
            .encode();
            self.submit(REQUEST_PROFILE_PUBLICATION, nonce, body).await
        })
    }

    fn assert_horizon<'a>(&'a self, bytes: &'a [u8], nonce: [u8; 16]) -> Answer<'a, bool> {
        // the body is the client's: it holds the archive the row came from
        // and the material the node needs, so nothing is composed here
        Box::pin(async move {
            self.submit(REQUEST_HORIZON_ASSERTION, nonce, bytes.to_vec())
                .await
        })
    }

    fn profile<'a>(&'a self, body: &'a [u8]) -> Answer<'a, Option<Vec<u8>>> {
        Box::pin(async move { self.ask(REQUEST_PROFILE, body.to_vec()).await })
    }

    /// **A resource exchange travels as sequences where a frame is not
    /// enough** (`wire-format.md` §11): the request split into portions
    /// under the frame bound, each a `ResourceRequest` naming the same
    /// resource; the reply's portions joined into one `ResourceResponse`
    /// — the first's status, and the bodies in order where it delivered.
    fn teach<'a>(&'a self, frames: Vec<Vec<u8>>) -> Answer<'a, usize> {
        Box::pin(async move {
            let Some(s) = (self.session)() else {
                return 0;
            };
            let mut sent = 0usize;
            for frame in frames {
                if !s.send_control(rhtn_archive::topology::FRAME_TOPOLOGY_PUSH, &frame) {
                    break;
                }
                sent += 1;
            }
            sent
        })
    }

    fn resource<'a>(&'a self, _from: Keyhash, body: &'a [u8]) -> Answer<'a, Option<Vec<u8>>> {
        use rhtn_archive::catalog::{ResourceRequest, ResourceResponse, STATUS_DELIVERED};
        use rhtn_codec::bounds::REQUEST_FRAME_BYTES;
        Box::pin(async move {
            let s = (self.session)()?;
            let req = ResourceRequest::decode(body).ok()?;
            let portion = REQUEST_FRAME_BYTES - 128;
            let portions: Vec<Vec<u8>> = if req.message.len() <= portion {
                vec![body.to_vec()]
            } else {
                req.message
                    .chunks(portion)
                    .map(|c| {
                        ResourceRequest {
                            resource: req.resource,
                            message: c.to_vec(),
                        }
                        .encode()
                    })
                    .collect()
            };
            let replies = s.request_sequence(REQUEST_RESOURCE, &portions).await.ok()?;
            if replies.len() == 1 {
                return replies.into_iter().next();
            }
            let mut status = None;
            let mut joined = Vec::new();
            for r in &replies {
                let one = ResourceResponse::decode(r).ok()?;
                match status {
                    None => status = Some(one.status),
                    // a non-zero status ends the exchange; a frame after
                    // one is not the protocol
                    Some(st) if st != STATUS_DELIVERED => return None,
                    Some(_) if one.status != STATUS_DELIVERED => return None,
                    Some(_) => {}
                }
                if let Some(b) = one.body {
                    joined.extend_from_slice(&b);
                }
            }
            let status = status?;
            Some(
                ResourceResponse {
                    status,
                    body: (status == STATUS_DELIVERED).then_some(joined),
                }
                .encode(),
            )
        })
    }

    fn catalog<'a>(&'a self, _from: Keyhash, body: &'a [u8]) -> Answer<'a, Option<Vec<u8>>> {
        Box::pin(async move {
            self.ask(rhtn_archive::catalog::REQUEST_CATALOG_QUERY, body.to_vec())
                .await
        })
    }

    fn page<'a>(&'a self, _from: Keyhash, body: &'a [u8]) -> Answer<'a, Option<Vec<u8>>> {
        Box::pin(async move {
            self.ask(rhtn_archive::catalog::REQUEST_CATALOG_PAGE, body.to_vec())
                .await
        })
    }

    /// Frame 10 on stream 0 (`wire-format.md` §10.3): a post this client
    /// wrote. Not a request: the answer is only whether the frame left,
    /// and the post coming back numbered is what says the node took it.
    fn board_post<'a>(&'a self, _from: Keyhash, body: &'a [u8]) -> Answer<'a, bool> {
        Box::pin(async move {
            let Some(s) = (self.session)() else {
                return false;
            };
            s.send_control(rhtn_archive::board::FRAME_BOARD_POST, body)
        })
    }

    fn board<'a>(&'a self, _from: Keyhash, body: &'a [u8]) -> Answer<'a, Option<Vec<u8>>> {
        Box::pin(async move {
            self.ask(rhtn_archive::board::REQUEST_BOARD, body.to_vec())
                .await
        })
    }

    /// Frame 5 on stream 0, which is the one channel a transaction
    /// travels on (`wire-format.md` §10.1).  Not a request: §10.1.3
    /// defines no acknowledgement and the answer here is only whether the
    /// frame left.
    fn propagate<'a>(&'a self, bytes: Vec<u8>) -> Answer<'a, bool> {
        Box::pin(async move {
            let Some(s) = (self.session)() else {
                return false;
            };
            s.send_control(
                FRAME_TOPOLOGY_PUSH,
                &rhtn_node::propagation::encode_push(rhtn_node::store::KIND_TRANSACTION, &bytes),
            )
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
///
/// **And `node` saying the client's page there changed is asked again**
/// (`wire-format.md` §6.9) [author, 2026-11-23]: frame 9 carries nothing,
/// and the page it points at is the client's own to fetch, through
/// `courier`, as a sweep fetches it.
pub fn follow(
    handle: Handle,
    node: Keyhash,
    courier: Arc<crate::courier::Courier>,
    mut frames: tokio::sync::mpsc::UnboundedReceiver<(u64, Vec<u8>)>,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        // **a session just up is one to catch the board up on**
        // (`light-client-requirements.md` §4.5): from the last number this
        // node gave, and again while it says more is held
        let asking = handle.with(move |c| c.board_ask(node)).await;
        if !asking.is_empty() {
            let courier = courier.clone();
            tokio::spawn(async move {
                courier.carry(asking).await;
                courier.wrote();
            });
        }
        while let Some((frame_type, body)) = frames.recv().await {
            // **a post on the subnet's board** (`wire-format.md` §10.3):
            // into the pool, and the state written as after any arrival
            if frame_type == rhtn_archive::board::FRAME_BOARD_POST {
                let took = handle.with(move |c| c.board_arrived(node, &body)).await;
                tracing::info!(
                    target: "pay",
                    node = %rhtn_client::diag::id8(&node),
                    took = ?took,
                    "pay.board.arrived"
                );
                courier.wrote();
                continue;
            }
            if frame_type == rhtn_archive::catalog::FRAME_PAGE_CHANGED {
                let msgs = handle.with(move |c| c.browse_at(node)).await;
                let carried = courier.carry(msgs).await;
                tracing::info!(
                    target: "pay",
                    node = %rhtn_client::diag::id8(&node),
                    refused = carried.refused.len(),
                    "pay.page_changed"
                );
                continue;
            }
            // stream 0 carries memos too, and a memo is a node's routing
            // aid rather than a client's (`wire-format.md` §10.2)
            if frame_type != FRAME_TOPOLOGY_PUSH {
                continue;
            }
            let Ok((kind, object)) = rhtn_node::propagation::decode_push(&body) else {
                continue;
            };
            // **both kinds matter to a client.** The transactions are the
            // shape of its neighbourhood; the endpoint records are the
            // addresses in it, and §7.6 has only infra nodes publish one,
            // so holding one is also what says which parties are
            // infrastructure. A client that took the first and dropped the
            // second would hold a map with nowhere on it.
            handle
                .with(move |c| {
                    let known = c.known.clone();
                    match kind {
                        rhtn_node::store::KIND_TRANSACTION => c.take_propagated(&object),
                        // **and one for the node serving this client, at an
                        // address it has not agreed, is an invitation**
                        // [author, 2026-11-17]: proved and asked as a link's
                        rhtn_node::store::KIND_ENDPOINT_RECORD => {
                            let took = c.horizon.ingest_endpoint(&object, &known);
                            if took == rhtn_client::horizon::Took::Applied
                                && let Ok(er) =
                                    rhtn_archive::endpoint::EndpointRecord::parse(&object)
                            {
                                let at = er
                                    .endpoints
                                    .iter()
                                    .filter_map(|p| {
                                        rhtn_transport::session::NetworkPoint::decode_bytes(p).ok()
                                    })
                                    .map(|p| p.socket().to_string())
                                    .collect();
                                c.record_moved(er.node, at);
                            }
                            took
                        }
                        // and the delegations of the parties in it, which
                        // bind a delegated peer on a direct path with no
                        // frame (`light-client-requirements.md` §4.2)
                        rhtn_node::store::KIND_DELEGATION => {
                            c.horizon.ingest_delegation(&object, &known)
                        }
                        _ => rhtn_client::horizon::Took::Refused,
                    };
                })
                .await;
        }
        // the session ended: the next one catches the board up again
        handle.with(move |c| c.board_detached(&node)).await;
    })
}

/// `TopologyPush` (`wire-format.md` §8.2).
pub const FRAME_TOPOLOGY_PUSH: u64 = 5;

/// Carry what `node` delivers into the client.
///
/// Each message is a `RelayedPayload` (`wire-format.md` §7.10): its
/// position, the seal, and the node's half. **It names no sender**, which
/// is inside the seal and the client's to attribute; what arrives is said
/// to come from the node that delivered it.
pub fn collect(
    node: Keyhash,
    inbound: Inbound,
    mut deliveries: tokio::sync::mpsc::UnboundedReceiver<Vec<u8>>,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        while let Some(bytes) = deliveries.recv().await {
            inbound(node, bytes);
        }
    })
}
