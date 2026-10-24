//! **A network for the client's tests**: clients attached to serving
//! nodes that hold their prekeys and their mail, every message carried
//! where the client addressed it and whatever that causes carried in
//! turn.
//!
//! **It carries; it does not decide** [author, 2026-10-14]: "the harness
//! should only drive actions as user or network inputs might and gather
//! results". Nothing here knows the order of a ceremony's steps — the
//! node's part is the real `PrekeyService` and the mail queue, the
//! sessions are the clients' own, and [`Net::attend`] calls only what the
//! shell calls, when the kernel's own `progress()` says it is due.

use super::harness::*;
use super::*;
use rhtn_archive::prekey::*;
use rhtn_client::ceremony::*;
use rhtn_client::payload::KIND_APPLICATION;
use rhtn_node::prekeys::{PrekeyConfig, PrekeyService};
use std::collections::{BTreeMap, VecDeque};

/// Mail waiting for one client: who sent each item and its bytes.
pub type Mail = VecDeque<([u8; 32], Vec<u8>)>;

/// A serving node as the harness stands it in: the prekeys it holds and
/// the mail it queues for the clients it serves.
pub struct ServingNode {
    pub prekeys: PrekeyService,
    pub queues: BTreeMap<[u8; 32], Mail>,
}

/// Clients on a harness, each served by a node, every message moved by
/// hand and logged.
pub struct Net {
    pub s: Setup,
    pub nodes: BTreeMap<&'static str, ServingNode>,
    pub served_by: BTreeMap<[u8; 32], &'static str>,
    pub log: Vec<(String, String, Msg)>,
    pub delivered: Vec<([u8; 32], [u8; 32], String)>,
}

pub const CLIENTS: [&str; 5] = ["alice", "bob", "carol", "w1", "w2"];

pub fn net(nodes: &[&'static str], serving: &[(&'static str, &'static str)]) -> Net {
    let s = setup(&CLIENTS);
    net_of(s, nodes, serving)
}

/// The same, over a harness the caller built: its parties, its channels
/// and its clock skews.
pub fn net_of(s: Setup, nodes: &[&'static str], serving: &[(&'static str, &'static str)]) -> Net {
    s.clock.set(1_790_000_000_000);
    let nodes: BTreeMap<&'static str, ServingNode> = nodes
        .iter()
        .map(|n| {
            (
                *n,
                ServingNode {
                    prekeys: PrekeyService::new(PrekeyConfig {
                        one_time_per_requester_per_subject: 3,
                        window_s: 3600,
                        ..Default::default()
                    }),
                    queues: BTreeMap::new(),
                },
            )
        })
        .collect();
    let served_by = serving.iter().map(|(c, n)| (kh(c), *n)).collect();
    Net {
        s,
        nodes,
        served_by,
        log: Vec::new(),
        delivered: Vec::new(),
    }
}

/// What arrived, legibly: application text as text.
pub fn describe(d: &Dispatched) -> String {
    match d {
        Dispatched::Application { bytes, .. } => {
            format!("Application({})", String::from_utf8_lossy(bytes))
        }
        Dispatched::Grant(o) => format!("Grant({o:?})"),
        Dispatched::Late(r) => format!("Late({r:?})"),
        Dispatched::Candidates(b) => format!("Candidates({} bytes)", b.len()),
        Dispatched::ResponseCopy(r) => format!("ResponseCopy({r:?})"),
        Dispatched::Served { records, more } => format!("Served({records}, more {more})"),
        Dispatched::Fetched(r) => format!("Fetched({r:?})"),
        Dispatched::Query { query, outcome } => format!("Query({query:?}, {outcome:?})"),
        Dispatched::Response(r) => format!("Response({r:?})"),
        Dispatched::Conversation(c) => format!("Conversation({c})"),
    }
}

/// The device a seed-holding client presents: its classical member.
pub fn phone_key(n: &str) -> [u8; 32] {
    *rhtn_crypto::identity::testkit::test_identity(n)
        .public
        .ed
        .as_bytes()
}

pub fn name(k: &[u8; 32]) -> String {
    NAMES
        .iter()
        .find(|n| kh(n) == *k)
        .map(|s| s.to_string())
        .unwrap_or_else(|| "?".into())
}

impl Net {
    pub fn now(&self) -> u64 {
        self.s.clock.get() / 1000
    }

    pub fn node_of(&self, client: &[u8; 32]) -> &'static str {
        self.served_by[client]
    }

    /// The population a client at `node` prefetches: everyone served there and elsewhere.
    pub fn population(&self) -> Vec<[u8; 32]> {
        self.served_by.keys().copied().collect()
    }

    pub fn attach(&mut self, client: &str) {
        let node = self.node_of(&kh(client));
        let pop = self.population();
        let msgs = self.s.client(client).attach(kh(node), &pop);
        self.carry(kh(client), msgs);
    }

    /// Sweep again: the first client to attach swept before the second had
    /// published, and a recipient can only attribute an initial message
    /// from a peer whose binding it holds.
    pub fn sweep(&mut self, client: &str) {
        let pop = self.population();
        let msgs = self.s.client(client).sweep(&pop);
        self.carry(kh(client), msgs);
    }

    /// Move a client's outgoing messages where they go, and whatever they
    /// cause in turn.
    pub fn carry(&mut self, from: [u8; 32], msgs: Vec<Msg>) {
        for m in msgs {
            let node = self.node_of(&from);
            self.log.push((name(&from), node.to_string(), m.clone()));
            match m {
                Msg::PublishBundle(b) => {
                    self.nodes
                        .get_mut(node)
                        .unwrap()
                        .prekeys
                        .publish(&ids(), &b)
                        .expect("a bundle by the client");
                }
                Msg::StockOneTime(keys) => {
                    assert!(
                        self.nodes.get_mut(node).unwrap().prekeys.stock(from, keys),
                        "the pool takes the deposit"
                    );
                }
                Msg::PrekeyRequest(req) => {
                    // a single-subject request goes to that subject's serving
                    // node; a sweep to the requester's own
                    let node = match PrekeyRequest::decode(&req).unwrap() {
                        PrekeyRequest::One { subject, .. } => self.node_of(&subject),
                        PrekeyRequest::Batch { .. } => node,
                    };
                    let now = self.now();
                    let n = self.nodes.get_mut(node).unwrap();
                    let reply = n.prekeys.answer(&from, &req, now).expect("a reply");
                    let exhausted = n.prekeys.take_exhausted();
                    self.log.push((
                        node.to_string(),
                        name(&from),
                        Msg::PrekeyReply(reply.clone()),
                    ));
                    let more = self
                        .s
                        .h
                        .client(&from)
                        .take_prekey_reply(&reply)
                        .expect("a reply the client can use");
                    self.carry(from, more);
                    for subject in exhausted {
                        self.log
                            .push((node.to_string(), name(&subject), Msg::PoolExhausted));
                        let restock = self.s.h.client(&subject).on_pool_report(0);
                        self.carry(subject, restock);
                    }
                }
                Msg::Payload { to, bytes, .. } => {
                    self.log.push((
                        name(&from),
                        name(&to),
                        Msg::Payload {
                            to,
                            bytes: bytes.clone(),
                            device: [0; 32],
                        },
                    ));
                    let d = self
                        .s
                        .h
                        .client(&to)
                        .receive_payload(from, &bytes)
                        .expect("decrypts at the recipient");
                    self.delivered.push((from, to, describe(&d)));
                    // **what arriving caused goes out too**: a client
                    // answering an archive fetch replies on the same
                    // channel, and the reply is in its outbox
                    let back = self.s.h.client(&to).outbox();
                    self.carry(to, back);
                }
                Msg::Relay { to, bytes, .. } => {
                    // my serving node hands it to the recipient's, which queues it
                    let dest = self.node_of(&to);
                    self.log.push((
                        node.to_string(),
                        dest.to_string(),
                        Msg::Relay {
                            to,
                            bytes: bytes.clone(),
                            device: [0; 32],
                        },
                    ));
                    self.nodes
                        .get_mut(dest)
                        .unwrap()
                        .queues
                        .entry(to)
                        .or_default()
                        .push_back((from, bytes));
                }
                Msg::Transport(_) => {}
                // a client's own records go up to its serving node, which
                // is PRT-06's leg and not this harness's: here they are
                // whatever the ceremony left in the outbox
                Msg::Record(_) => {}
                other => panic!("not a payload message: {other:?}"),
            }
        }
    }

    /// The recipient collects its mail.
    pub fn collect(&mut self, client: &str) -> Vec<String> {
        let node = self.node_of(&kh(client));
        let mut out = Vec::new();
        while let Some((from, bytes)) = self
            .nodes
            .get_mut(node)
            .unwrap()
            .queues
            .entry(kh(client))
            .or_default()
            .pop_front()
        {
            let d = self
                .s
                .client(client)
                .receive_payload(from, &bytes)
                .expect("decrypts at the recipient");
            out.push(describe(&d));
        }
        out
    }

    /// The next item waiting for `client`, raw: what its serving node
    /// would hand over, before the client reads it.
    pub fn queued(&mut self, client: &str) -> ([u8; 32], Vec<u8>) {
        let node = self.node_of(&kh(client));
        self.nodes
            .get_mut(node)
            .unwrap()
            .queues
            .entry(kh(client))
            .or_default()
            .pop_front()
            .expect("something waiting")
    }

    pub fn send(&mut self, from: &str, to: &str, text: &str) {
        let msgs = self
            .s
            .client(from)
            .send_payload(kh(to), KIND_APPLICATION, text.as_bytes())
            .unwrap();
        self.carry(kh(from), msgs);
    }

    pub fn requests(&self, from: &str) -> Vec<PrekeyRequest> {
        self.log
            .iter()
            .filter(|(f, _, m)| f == from && matches!(m, Msg::PrekeyRequest(_)))
            .map(|(_, _, m)| {
                if let Msg::PrekeyRequest(b) = m {
                    PrekeyRequest::decode(b).unwrap()
                } else {
                    unreachable!()
                }
            })
            .collect()
    }
}
