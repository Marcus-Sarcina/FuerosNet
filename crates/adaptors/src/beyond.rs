//! Reaching a node that does not serve this client.
//!
//! **A light client does not need its patron or its serving node to
//! address a distant node for which it holds a locator** [author,
//! 2026-10-05].  Design §12.6.3 has payload take the direct path where it
//! can and a relayed path where it cannot; the relayed path's first term
//! is the **recipient's** own node, which queues for it while it is away
//! (§14.1.4) and hands it over when it returns, exactly as it does for a
//! message that arrived from anywhere else.  The sender's node is not
//! astride that route.
//!
//! What this module holds is the client's half of it: the §7.7 descent
//! driven from the client's own state, the cache of what it came to, and
//! the submission that hands the payload over.  Opening the connection is
//! somebody else's job — [`Distant`] is the seam, and the only
//! implementation that dials lives in the FFI crate beside the endpoint
//! it dials from.

use crate::attached::Nonces;
use crate::serving::Answer;
use rhtn_archive::Keyhash;
use rhtn_archive::submission::*;
use rhtn_client::horizon::{Askable, Reach, Upstream};
use rhtn_node::resolution::{
    Disposition, REQUEST_RESOLVE, ResolveReply, ResolveRequest, disposition,
};
use rhtn_transport::session::{ClientConfig, NetworkPoint, connect_request_only, request_on};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// How long a resolved position is believed: **fifteen minutes**, the
/// client's own number and no part of the wire.
///
/// `light-client-requirements.md` §4.2 requires every cache to have a
/// lifetime and the lifetime to be stated, because a cache with no expiry
/// is a retention decision made by omission.  Fifteen minutes is long
/// enough that the several steps of one ceremony share a resolution and
/// short enough that a subject who moved is re-resolved inside the
/// sitting [author, 2026-10-06: set here, to be adjusted if it proves
/// wrong in practice].
pub const RESOLVED_FOR: Duration = Duration::from_secs(900);

/// How many referrals one resolution may follow.  Each referral advances
/// at least one path index (`wire-format.md` §7.7.3), so a descent that
/// has taken more than a path can hold is not making progress.
pub const MAX_REFERRALS: usize = rhtn_codec::bounds::PATH_NIBBLES as usize;

/// One request of a node this client is not attached to.
///
/// **The seam, not the dial.**  An implementation holds the endpoint, the
/// pins and whatever connections it keeps; this says only what is asked
/// of whom.  `at` is the node's endpoints as the `NetworkPoint`s were
/// encoded, in the publisher's preference order, and an implementation
/// MUST treat them as alternatives (`wire-format.md` §7.7.3).
/// `key_material` is what binds the node where the caller holds it (§9.1)
/// and `None` where it does not, in which case the implementation is left
/// with whatever it has pinned.
pub trait Distant: Send + Sync {
    /// The reply bytes, or nothing where the node could not be reached,
    /// could not be bound, or did not answer.
    fn ask<'a>(
        &'a self,
        node: Keyhash,
        key_material: Option<Vec<u8>>,
        at: Vec<Vec<u8>>,
        request: u64,
        body: Vec<u8>,
    ) -> Answer<'a, Option<Vec<u8>>>;
}

/// Nothing beyond the serving node is reachable: what a client beside its
/// node in one process has, and the default where no dialler was wired.
pub struct NoDistant;

impl Distant for NoDistant {
    fn ask<'a>(
        &'a self,
        _node: Keyhash,
        _key_material: Option<Vec<u8>>,
        _at: Vec<Vec<u8>>,
        _request: u64,
        _body: Vec<u8>,
    ) -> Answer<'a, Option<Vec<u8>>> {
        Box::pin(async { None })
    }
}

/// A node payload can be handed to for somebody it serves.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Upstreamed {
    /// The node.
    pub node: Keyhash,
    /// Where it answers, encoded.
    pub endpoints: Vec<Vec<u8>>,
    /// What binds it, where it is held.
    pub key_material: Option<Vec<u8>>,
}

/// The network past this client's own serving node: who serves whom, as
/// far as resolution has said, and the submission that hands payload
/// over.
pub struct Beyond {
    distant: Arc<dyn Distant>,
    nonce: Nonces,
    /// What a resolution came to, by subject, with when it was taken.
    /// **Bounded in time rather than in size**: an entry is a record of
    /// who this client looked for and when (design §19.4).
    resolved: Mutex<HashMap<Keyhash, (Upstreamed, Instant)>>,
}

impl Beyond {
    /// A view of the network beyond, dialled through `distant`.
    pub fn new(distant: Arc<dyn Distant>, nonce: Nonces) -> Arc<Beyond> {
        Arc::new(Beyond {
            distant,
            nonce,
            resolved: Mutex::new(HashMap::new()),
        })
    }

    /// Hand `bytes` to the node that serves `to`, for `to`'s `device`.
    /// Whether that node took it.
    ///
    /// **Taking it is the whole of the answer** (design §14.1.6): the node
    /// answers on taking, not on delivering, because a sender made to wait
    /// for delivery would be waiting on somebody who may be away for days.
    pub async fn carry(
        &self,
        to: Keyhash,
        device: [u8; 32],
        bytes: Vec<u8>,
        reach: Reach,
        served_by: Option<Keyhash>,
    ) -> bool {
        let nonce = (self.nonce)();
        let body = RelaySubmission {
            recipient: to,
            ciphertext: bytes,
            nonce,
            device,
        }
        .encode();
        let Some(reply) = self
            .ask_their_node(to, reach, served_by, REQUEST_RELAY, body)
            .await
        else {
            return false;
        };
        // the reply must echo this submission's nonce, as every submission
        // reply must (`wire-format.md` §7.10): an answer carrying another
        // one's nonce says nothing about this one
        match SubmissionReply::decode(&reply) {
            Ok(r) => r.nonce == nonce && r.code == SUBMISSION_ACCEPTED,
            Err(_) => false,
        }
    }

    /// One request of the node that serves `subject`, where this client
    /// can say who that is and reach them.
    ///
    /// **Nothing is asked of the node already serving this client.**
    /// `served_by` names it, and a resolution that lands there answers
    /// nothing: that node is reachable on the session already up, which
    /// is cheaper and is where the caller's next term goes anyway.
    pub async fn ask_their_node(
        &self,
        subject: Keyhash,
        reach: Reach,
        served_by: Option<Keyhash>,
        request: u64,
        body: Vec<u8>,
    ) -> Option<Vec<u8>> {
        let up = self.upstream(subject, reach).await?;
        if served_by == Some(up.node) {
            return None;
        }
        self.distant
            .ask(up.node, up.key_material, up.endpoints, request, body)
            .await
    }

    /// Who serves `to`: what the client already held, what a cached
    /// resolution came to, or a resolution run now.
    async fn upstream(&self, to: Keyhash, reach: Reach) -> Option<Upstreamed> {
        match reach.upstream {
            Upstream::Known {
                node,
                endpoints,
                key_material,
            } => Some(Upstreamed {
                node,
                endpoints,
                key_material,
            }),
            Upstream::Position {
                anchor,
                path,
                nibbles,
            } => {
                if let Some(up) = self.cached(&to) {
                    return Some(up);
                }
                let up = self.descend(to, anchor, path, nibbles, &reach.ask).await?;
                self.resolved
                    .lock()
                    .unwrap()
                    .insert(to, (up.clone(), Instant::now()));
                Some(up)
            }
            Upstream::Unknown => None,
        }
    }

    /// A resolution taken inside its lifetime; an older one is dropped on
    /// the way past, so a stale entry is not kept against the day it
    /// might be wanted.
    fn cached(&self, subject: &Keyhash) -> Option<Upstreamed> {
        let mut held = self.resolved.lock().unwrap();
        match held.get(subject) {
            Some((up, at)) if at.elapsed() < RESOLVED_FOR => Some(up.clone()),
            Some(_) => {
                held.remove(subject);
                None
            }
            None => None,
        }
    }

    /// §7.7's descent, run by the client itself: the request put to each
    /// node it can reach in turn, referrals followed, and the terminal
    /// `ServingInfra` answer taken.
    ///
    /// **One nonce for the logical resolution**, reused across endpoints
    /// and referrals (`wire-format.md` §7.7.3): a fresh one per attempt
    /// would let a reply for one attempt be accepted as the answer to
    /// another.
    ///
    /// **The anchor and the full path go every time.** No consumed-prefix
    /// state exists, because each node reads the path from its own
    /// position; and no arrival-consistency equation is checked, because
    /// arrival is announced by the reply.
    async fn descend(
        &self,
        subject: Keyhash,
        anchor: Keyhash,
        path: Vec<u8>,
        nibbles: u64,
        ask: &[Askable],
    ) -> Option<Upstreamed> {
        let nonce = (self.nonce)();
        let req = ResolveRequest {
            subject,
            anchor,
            path,
            nibbles,
            nonce,
        };
        for first in ask {
            let mut at = Upstreamed {
                node: first.node,
                endpoints: first.endpoints.clone(),
                key_material: first.key_material.clone(),
            };
            let mut retried = false;
            for _ in 0..MAX_REFERRALS {
                let bytes = self
                    .distant
                    .ask(
                        at.node,
                        at.key_material.clone(),
                        at.endpoints.clone(),
                        REQUEST_RESOLVE,
                        req.encode(),
                    )
                    .await;
                // nothing answered: this node is no use for this
                // resolution, and the next one this client holds is
                let Some(reply) = bytes.and_then(|b| ResolveReply::decode(&b).ok()) else {
                    break;
                };
                if reply.nonce() != nonce {
                    break;
                }
                match reply {
                    ResolveReply::Serving { serving, .. } => {
                        return Some(Upstreamed {
                            node: serving.node,
                            endpoints: encoded(&serving.endpoints),
                            key_material: serving.key_material,
                        });
                    }
                    // **a referral that advances nothing is a loop** and a
                    // node with nothing to add reports failure instead, so
                    // one claiming no progress is malformed (§7.7.3)
                    ResolveReply::Referral { referral, .. } => {
                        if referral.advances == 0 || referral.endpoints.is_empty() {
                            break;
                        }
                        at = Upstreamed {
                            node: referral.next,
                            endpoints: encoded(&referral.endpoints),
                            key_material: referral.key_material,
                        };
                    }
                    // **the mapping `wire-format.md` §7.7.3 states, used
                    // as it states it.**  Unavailable is this node's
                    // invitation to ask again; refused is its answer and
                    // not its endpoint's, so repetition will not change
                    // it; and re-resolve means starting over somewhere
                    // else, which is the next node this client holds
                    ResolveReply::Failure { code, .. } => match disposition(code) {
                        Disposition::Retry if !retried => {
                            retried = true;
                            continue;
                        }
                        _ => break,
                    },
                }
            }
        }
        None
    }
}

/// `NetworkPoint`s as the client holds them: encoded, since that is the
/// form a position arrives in and the form the dialler takes.
fn encoded(points: &[NetworkPoint]) -> Vec<Vec<u8>> {
    points.iter().map(|p| p.encode_bytes()).collect()
}

/// How many nodes this client is not attached to are kept dialled at
/// once, and how long an unused one is kept.
///
/// **Both are this client's own numbers**, no part of the wire.  A
/// resolution's descent touches a handful of nodes and a conversation's
/// several steps go to the same few, so a small pool carries a sitting;
/// the idle bound is what keeps a connection from outliving the reason it
/// was opened, since an open connection to a stranger's node is a
/// standing statement about who this client is talking to (design §19.4).
const FAR_NODES: usize = 4;
/// How long an unused far connection is kept.
const FAR_IDLE: Duration = Duration::from_secs(120);
/// How long one dial of a far node may take.
///
/// **Shorter than the attach's, because this dial has a fallback and that
/// one does not.**  A node's endpoint list may hold eight addresses and
/// every one is tried (`wire-format.md` §7.7.3), so a client that gave
/// each the attach's patience would sit for over a minute before reaching
/// the term that was going to carry the message anyway.  The serving
/// node's own dial keeps the longer timeout: there is nothing after it.
const FAR_DIAL: Duration = Duration::from_secs(3);

/// A connection to a node that does not serve this client: **requests
/// only** (`wire-format.md` §8.2, §9.1).  No attach submission, no
/// heartbeat, no delivery readers, and nothing is ever delivered on it —
/// what this client receives arrives through the node that serves it.
struct Far {
    conn: quinn::Connection,
    last: Instant,
}

/// The dialler behind [`Distant`]: this client's own endpoint, the pins
/// and TLS store every dial shares with the serving session, and the few
/// request-only connections it keeps.
pub struct Beyonder {
    endpoint: quinn::Endpoint,
    cfg: ClientConfig,
    far: Mutex<HashMap<Keyhash, Far>>,
}

impl Beyonder {
    /// A dialler on `endpoint`, binding and pinning as `cfg` does.
    ///
    /// **The same configuration the serving session dials with**, so a
    /// node pinned here is pinned there and a resumption ticket taken on
    /// one dial is available to the next — with one field of its own, the
    /// dial timeout, for [`FAR_DIAL`]'s reason.
    pub fn new(endpoint: quinn::Endpoint, cfg: ClientConfig) -> Arc<Beyonder> {
        let mut cfg = cfg;
        cfg.connect_timeout = FAR_DIAL;
        Arc::new(Beyonder {
            endpoint,
            cfg,
            far: Mutex::new(HashMap::new()),
        })
    }

    /// The connection held for `node`, where one is and it has not been
    /// idle past the bound.  **Every entry is swept on the way past**, so
    /// nothing is kept because nobody looked.
    fn held(&self, node: &Keyhash) -> Option<quinn::Connection> {
        let mut far = self.far.lock().unwrap();
        far.retain(|_, f| {
            let keep = f.last.elapsed() < FAR_IDLE;
            if !keep {
                f.conn.close(quinn::VarInt::from_u32(0), b"idle");
            }
            keep
        });
        let f = far.get_mut(node)?;
        f.last = Instant::now();
        Some(f.conn.clone())
    }

    /// Keep this connection for the next request, evicting the least
    /// recently used where the pool is full.
    fn keep(&self, node: Keyhash, conn: quinn::Connection) {
        let mut far = self.far.lock().unwrap();
        while far.len() >= FAR_NODES {
            let Some(oldest) = far
                .iter()
                .min_by_key(|(_, f)| f.last)
                .map(|(k, _)| *k)
                .filter(|k| *k != node)
            else {
                break;
            };
            if let Some(f) = far.remove(&oldest) {
                f.conn.close(quinn::VarInt::from_u32(0), b"evicted");
            }
        }
        far.insert(
            node,
            Far {
                conn,
                last: Instant::now(),
            },
        );
    }

    fn forget(&self, node: &Keyhash) {
        if let Some(f) = self.far.lock().unwrap().remove(node) {
            f.conn.close(quinn::VarInt::from_u32(0), b"failed");
        }
    }

    /// Close everything held.  Called when the client is released.
    pub fn close(&self) {
        for (_, f) in self.far.lock().unwrap().drain() {
            f.conn.close(quinn::VarInt::from_u32(0), b"released");
        }
    }
}

impl Distant for Beyonder {
    /// One request of a node this client is not attached to.
    ///
    /// **The endpoints are alternatives** (`wire-format.md` §7.7.3): a
    /// single unreachable first entry would otherwise be a permanent
    /// outage for that node.  A held connection is tried before any dial,
    /// and a held connection that fails is dropped and the dials run —
    /// the peer may have restarted since it was opened.
    fn ask<'a>(
        &'a self,
        node: Keyhash,
        key_material: Option<Vec<u8>>,
        at: Vec<Vec<u8>>,
        request: u64,
        body: Vec<u8>,
    ) -> Answer<'a, Option<Vec<u8>>> {
        Box::pin(async move {
            // what a reply carried, so a node this client has never
            // contacted can be bound at all (`wire-format.md` §9.1): the
            // pin fails where the material does not hash to the keyhash,
            // and a dial with nothing pinned fails in its turn
            if let Some(km) = key_material {
                let _ = self.cfg.pins.pin(node, &km);
            }
            if let Some(conn) = self.held(&node) {
                match request_on(&conn, request, &body).await {
                    Ok(reply) => return Some(reply),
                    Err(_) => self.forget(&node),
                }
            }
            for addr in at
                .iter()
                .filter_map(|p| NetworkPoint::decode_bytes(p).ok())
                .map(|p| p.socket())
            {
                let Ok((conn, _)) =
                    connect_request_only(&self.cfg, &self.endpoint, node, addr).await
                else {
                    continue;
                };
                match request_on(&conn, request, &body).await {
                    Ok(reply) => {
                        self.keep(node, conn);
                        return Some(reply);
                    }
                    Err(_) => conn.close(quinn::VarInt::from_u32(0), b"no answer"),
                }
            }
            None
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rhtn_node::resolution::{Path, Referral, ServingInfra};

    fn kh(n: u8) -> Keyhash {
        [n; 32]
    }

    fn point(port: u16) -> NetworkPoint {
        NetworkPoint::new([127, 0, 0, 1], Some(port as u64))
    }

    fn nonces() -> Nonces {
        Arc::new(|| [7; 16])
    }

    /// Every ask recorded, each answered from a script keyed by the node
    /// asked.  A node with no entry answers nothing, as an unreachable
    /// one does.
    #[derive(Default)]
    struct Script {
        answers: HashMap<Keyhash, Vec<u8>>,
        asked: Mutex<Vec<(Keyhash, u64)>>,
    }

    impl Distant for Script {
        fn ask<'a>(
            &'a self,
            node: Keyhash,
            _key_material: Option<Vec<u8>>,
            _at: Vec<Vec<u8>>,
            request: u64,
            _body: Vec<u8>,
        ) -> Answer<'a, Option<Vec<u8>>> {
            self.asked.lock().unwrap().push((node, request));
            let answer = self.answers.get(&node).cloned();
            Box::pin(async move { answer })
        }
    }

    fn accepted() -> Vec<u8> {
        SubmissionReply::code([7; 16], SUBMISSION_ACCEPTED).encode()
    }

    fn known(node: Keyhash) -> Reach {
        Reach {
            upstream: Upstream::Known {
                node,
                endpoints: vec![point(9001).encode_bytes()],
                key_material: None,
            },
            ask: Vec::new(),
        }
    }

    fn position(ask: Vec<Askable>) -> Reach {
        Reach {
            upstream: Upstream::Position {
                anchor: kh(1),
                path: vec![0x21],
                nibbles: 2,
            },
            ask,
        }
    }

    fn askable(node: Keyhash) -> Askable {
        Askable {
            node,
            endpoints: vec![point(9002).encode_bytes()],
            key_material: None,
        }
    }

    /// A position the client already holds is sent to with nobody asked:
    /// no resolution request exists for it at all.
    #[tokio::test]
    async fn a_held_position_is_carried_to_with_no_resolution_asked() {
        let script = Arc::new(Script {
            answers: HashMap::from([(kh(2), accepted())]),
            asked: Mutex::new(Vec::new()),
        });
        let b = Beyond::new(script.clone(), nonces());
        assert!(
            b.carry(kh(3), [0; 32], vec![1, 2, 3], known(kh(2)), None)
                .await
        );
        assert_eq!(
            *script.asked.lock().unwrap(),
            vec![(kh(2), REQUEST_RELAY)],
            "one submission to the node that serves the recipient, and nothing else"
        );
    }

    /// A node that refuses the submission is a node that did not take it,
    /// and the caller is told so rather than told the message travelled.
    #[tokio::test]
    async fn a_refused_submission_is_not_a_delivery() {
        let script = Arc::new(Script {
            answers: HashMap::from([(
                kh(2),
                SubmissionReply::code([7; 16], SUBMISSION_REFUSED).encode(),
            )]),
            asked: Mutex::new(Vec::new()),
        });
        let b = Beyond::new(script, nonces());
        assert!(!b.carry(kh(3), [0; 32], vec![1], known(kh(2)), None).await);
    }

    /// A reply carrying another submission's nonce says nothing about
    /// this one.
    #[tokio::test]
    async fn a_reply_that_echoes_another_nonce_is_not_an_answer() {
        let script = Arc::new(Script {
            answers: HashMap::from([(
                kh(2),
                SubmissionReply::code([8; 16], SUBMISSION_ACCEPTED).encode(),
            )]),
            asked: Mutex::new(Vec::new()),
        });
        let b = Beyond::new(script, nonces());
        assert!(!b.carry(kh(3), [0; 32], vec![1], known(kh(2)), None).await);
    }

    /// A referral is followed, the terminal answer is taken, and the
    /// payload goes to the node that answer named.
    #[tokio::test]
    async fn a_referral_is_followed_and_the_serving_node_is_the_one_sent_to() {
        let referral = ResolveReply::Referral {
            nonce: [7; 16],
            referral: Referral {
                next: kh(5),
                endpoints: vec![point(9003)],
                advances: 1,
                key_material: None,
            },
        }
        .encode();
        let serving = ResolveReply::Serving {
            nonce: [7; 16],
            serving: ServingInfra {
                node: kh(6),
                endpoints: vec![point(9004)],
                residual: Path::empty(),
                key_material: None,
            },
        }
        .encode();
        let script = Arc::new(Script {
            answers: HashMap::from([(kh(4), referral), (kh(5), serving), (kh(6), accepted())]),
            asked: Mutex::new(Vec::new()),
        });
        let b = Beyond::new(script.clone(), nonces());
        assert!(
            b.carry(
                kh(3),
                [0; 32],
                vec![1],
                position(vec![askable(kh(4))]),
                None
            )
            .await
        );
        assert_eq!(
            *script.asked.lock().unwrap(),
            vec![
                (kh(4), REQUEST_RESOLVE),
                (kh(5), REQUEST_RESOLVE),
                (kh(6), REQUEST_RELAY)
            ],
            "asked, referred, then handed over to the node that serves the recipient"
        );
    }

    /// **The resolution is cached for its lifetime**: a second message to
    /// the same recipient asks nobody where to find them.
    #[tokio::test]
    async fn a_second_message_reuses_the_resolution() {
        let serving = ResolveReply::Serving {
            nonce: [7; 16],
            serving: ServingInfra {
                node: kh(6),
                endpoints: vec![point(9004)],
                residual: Path::empty(),
                key_material: None,
            },
        }
        .encode();
        let script = Arc::new(Script {
            answers: HashMap::from([(kh(4), serving), (kh(6), accepted())]),
            asked: Mutex::new(Vec::new()),
        });
        let b = Beyond::new(script.clone(), nonces());
        for _ in 0..2 {
            assert!(
                b.carry(
                    kh(3),
                    [0; 32],
                    vec![1],
                    position(vec![askable(kh(4))]),
                    None
                )
                .await
            );
        }
        assert_eq!(
            script
                .asked
                .lock()
                .unwrap()
                .iter()
                .filter(|(_, r)| *r == REQUEST_RESOLVE)
                .count(),
            1,
            "one resolution for two messages"
        );
    }

    /// A node that says nothing is not the end of the resolution: the
    /// next node this client holds is asked, which is what routing around
    /// an unanswering node means.
    #[tokio::test]
    async fn an_unanswering_node_moves_the_resolution_to_the_next_one() {
        let serving = ResolveReply::Serving {
            nonce: [7; 16],
            serving: ServingInfra {
                node: kh(6),
                endpoints: vec![point(9004)],
                residual: Path::empty(),
                key_material: None,
            },
        }
        .encode();
        let script = Arc::new(Script {
            answers: HashMap::from([(kh(5), serving), (kh(6), accepted())]),
            asked: Mutex::new(Vec::new()),
        });
        let b = Beyond::new(script.clone(), nonces());
        assert!(
            b.carry(
                kh(3),
                [0; 32],
                vec![1],
                position(vec![askable(kh(4)), askable(kh(5))]),
                None
            )
            .await
        );
        assert_eq!(
            script.asked.lock().unwrap()[0],
            (kh(4), REQUEST_RESOLVE),
            "the first was asked"
        );
        assert_eq!(
            script.asked.lock().unwrap()[1],
            (kh(5), REQUEST_RESOLVE),
            "and the second answered"
        );
    }

    /// A referral that advances nothing is a loop, and is not followed.
    #[tokio::test]
    async fn a_referral_advancing_nothing_is_refused() {
        let loopback = ResolveReply::Referral {
            nonce: [7; 16],
            referral: Referral {
                next: kh(4),
                endpoints: vec![point(9003)],
                advances: 0,
                key_material: None,
            },
        }
        .encode();
        let script = Arc::new(Script {
            answers: HashMap::from([(kh(4), loopback)]),
            asked: Mutex::new(Vec::new()),
        });
        let b = Beyond::new(script.clone(), nonces());
        assert!(
            !b.carry(
                kh(3),
                [0; 32],
                vec![1],
                position(vec![askable(kh(4))]),
                None
            )
            .await
        );
        assert_eq!(
            script.asked.lock().unwrap().len(),
            1,
            "asked once and not followed"
        );
    }

    /// A party the client cannot place has no position to resolve from
    /// and nobody to dial, and nothing is asked of anyone.
    #[tokio::test]
    async fn an_unplaceable_party_is_not_resolved_for() {
        let script = Arc::new(Script::default());
        let b = Beyond::new(script.clone(), nonces());
        let reach = Reach {
            upstream: Upstream::Unknown,
            ask: Vec::new(),
        };
        assert!(!b.carry(kh(3), [0; 32], vec![1], reach, None).await);
        assert!(script.asked.lock().unwrap().is_empty());
    }
}
