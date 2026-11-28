//! **A network for the client's tests**: clients served by nodes that hold
//! a node key for each recipient they serve and a queue of what was sealed
//! to it, every message carried where the client addressed it and
//! whatever that causes carried in turn.
//!
//! **It carries; it does not decide** [author, 2026-10-14]: "the harness
//! should only drive actions as user or network inputs might and gather
//! results". Nothing here knows the order of a ceremony's steps — the
//! node's part is the real `NodeKeyService` and a queue that delivers by
//! position, the seals are the clients' own, and a node delivers on
//! arrival to a recipient with a live session as the transport does
//! (`wire-format.md` §7.10), or holds it until the recipient collects.

use super::harness::*;
use super::*;
use rhtn_archive::node_key::NodeKeyRequest;
use rhtn_archive::sealed::Sealed;
use rhtn_archive::submission::relayed;
use rhtn_client::ceremony::*;
use rhtn_client::payload::KIND_APPLICATION;
use rhtn_node::node_keys::{NodeKeyConfig, NodeKeyService};
use std::collections::{BTreeMap, BTreeSet};

/// One message a node holds for a recipient: its queue position, the
/// node key it was sealed to, and the `Sealed` as submitted.
#[derive(Debug, Clone)]
pub struct Queued {
    pub position: u64,
    pub n: u64,
    pub sealed: Vec<u8>,
}

/// A serving node as the harness stands it in: the node keys it holds for
/// the recipients it serves and the queue of what was sealed to them.
/// It speaks as the identity of its own name, which signs its node keys.
pub struct ServingNode {
    pub name: &'static str,
    pub keys: NodeKeyService,
    pub queues: BTreeMap<[u8; 32], Vec<Queued>>,
    pub next: u64,
}

impl ServingNode {
    /// The `RelayedPayload` this node hands `recipient` for `q`: the
    /// position, the sealed bytes and the node's half beside them.
    pub fn relayed(&self, recipient: &[u8; 32], q: &Queued) -> Vec<u8> {
        let sealed = Sealed::decode(&q.sealed).expect("a seal the node took");
        let ss_n = self
            .keys
            .open_half(recipient, q.n, &sealed.ct_n)
            .map(|z| *z)
            .expect("the node holds the key a queued message is sealed to");
        relayed(q.position, &q.sealed, &ss_n)
    }
}

/// Clients on a harness, each served by a node, every message moved by
/// hand and logged.
pub struct Net {
    pub s: Setup,
    pub nodes: BTreeMap<&'static str, ServingNode>,
    pub served_by: BTreeMap<[u8; 32], &'static str>,
    /// Every message a client handed the courier: `(from, where, msg)`.
    pub log: Vec<(String, String, Msg)>,
    /// What arrived and was dispatched: `(sender, recipient, what)`.
    pub delivered: Vec<([u8; 32], [u8; 32], String)>,
    /// What a recipient's node delivered and the recipient discarded:
    /// `(recipient, why)`.
    pub discarded: Vec<([u8; 32], String)>,
    /// Every node-key fetch: the asker, the node asked and the request.
    pub fetches: Vec<(String, &'static str, NodeKeyRequest)>,
    /// Submissions a node refused for a node key it no longer accepts.
    pub refused: Vec<([u8; 32], u64)>,
    /// Clients holding no live session: what arrives for them waits at
    /// their node until they collect.
    pub away: BTreeSet<[u8; 32]>,
}

pub const CLIENTS: [&str; 5] = ["alice", "bob", "carol", "w1", "w2"];

/// A network whose served clients each hold the others' introductions,
/// as clients that exchanged cards do: every one of them can be written to.
pub fn net(nodes: &[&'static str], serving: &[(&'static str, &'static str)]) -> Net {
    let mut n = bare_net(nodes, serving);
    let names: Vec<&'static str> = serving.iter().map(|(c, _)| *c).collect();
    for a in &names {
        for b in &names {
            if a != b {
                n.introduce(a, b);
            }
        }
    }
    n
}

/// The same with nobody introduced to anybody: each client holds the
/// others' material and no encryption key of theirs.
pub fn bare_net(nodes: &[&'static str], serving: &[(&'static str, &'static str)]) -> Net {
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
                    name: n,
                    keys: NodeKeyService::new(NodeKeyConfig::default()),
                    queues: BTreeMap::new(),
                    next: 1,
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
        discarded: Vec::new(),
        fetches: Vec::new(),
        refused: Vec::new(),
        away: BTreeSet::new(),
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
        Dispatched::Peer(what) => format!("Peer({what})"),
        Dispatched::Transfer(what) => format!("Transfer({what})"),
        Dispatched::Unverified { alias, bytes, .. } => {
            format!("Unverified({alias}: {})", String::from_utf8_lossy(bytes))
        }
    }
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

    /// The population a client prefetches: everyone served anywhere.
    pub fn population(&self) -> Vec<[u8; 32]> {
        self.served_by.keys().copied().collect()
    }

    /// `to` takes `from`'s introduction, as handed over out of band: its
    /// material, its encryption key and where it sits.
    pub fn introduce(&mut self, from: &str, to: &str) {
        let card = self
            .s
            .client(from)
            .own_introduction()
            .expect("a client with a position introduces itself");
        self.s
            .client(to)
            .take_introduction(&card)
            .expect("an introduction verifies");
    }

    pub fn attach(&mut self, client: &str) {
        let node = self.node_of(&kh(client));
        let pop = self.population();
        let msgs = self.s.client(client).attach(kh(node), &pop);
        self.carry(kh(client), msgs);
    }

    /// Prefetch again: a sweep fetches the population's node keys.
    pub fn sweep(&mut self, client: &str) {
        let pop = self.population();
        let msgs = self.s.client(client).sweep(&pop);
        self.carry(kh(client), msgs);
    }

    /// Move a client's outgoing messages where they go, and whatever they
    /// cause in turn.
    pub fn carry(&mut self, from: [u8; 32], msgs: Vec<Msg>) {
        for m in msgs {
            let at = self
                .served_by
                .get(&from)
                .map(|n| n.to_string())
                .unwrap_or_else(|| "-".into());
            self.log.push((name(&from), at, m.clone()));
            match m {
                Msg::NodeKeyRequest(recipients) => self.fetch(from, recipients),
                Msg::Sealed { to, sealed, plain } => self.submit(from, to, sealed, plain),
                Msg::Payload { to, bytes } => {
                    // **the direct path**: the transport is end to end
                    // between the two, so the peer is the sender
                    let d = self
                        .s
                        .h
                        .client(&to)
                        .receive_payload(from, &bytes)
                        .expect("a direct message reads");
                    self.delivered.push((from, to, describe(&d)));
                    let back = self.s.h.client(&to).outbox();
                    self.carry(to, back);
                }
                // a client's own records go up to its serving node, which
                // is PRT-06's leg and not this harness's, and so do the
                // profiles and horizon an attach publishes
                Msg::Record(_)
                | Msg::PublishProfile(_)
                | Msg::AssertHorizon { .. }
                | Msg::ProfileRequest(_) => {}
                other => panic!("not a payload message: {other:?}"),
            }
        }
    }

    /// **The courier's fetch** (`wire-format.md` §7.8): each recipient's
    /// node asked about the recipients it serves, one request per node —
    /// a batch where it serves several — and the reply handed back.
    pub fn fetch(&mut self, from: [u8; 32], recipients: Vec<[u8; 32]>) {
        let mut by_node: BTreeMap<&'static str, Vec<[u8; 32]>> = BTreeMap::new();
        for r in recipients {
            if let Some(node) = self.served_by.get(&r) {
                by_node.entry(node).or_default().push(r);
            }
        }
        for (node, rs) in by_node {
            let mut nonce = [0u8; 16];
            nonce[..8].copy_from_slice(&(self.fetches.len() as u64).to_be_bytes());
            let req = NodeKeyRequest::for_recipients(rs, nonce).expect("a request");
            self.fetches.push((name(&from), node, req.clone()));
            let now = self.now();
            let served = self.served_by.clone();
            let reply = self
                .nodes
                .get_mut(node)
                .unwrap()
                .keys
                .answer(
                    &id(node),
                    &|r| served.get(r) == Some(&node),
                    &req.encode(),
                    now,
                )
                .expect("a request the node reads");
            let more =
                self.s
                    .h
                    .client(&from)
                    .take_node_key_reply(&kh(node), &req.recipients(), &reply);
            self.carry(from, more);
        }
    }

    /// **A relay submission** at the recipient's node: refused where it is
    /// sealed to a node key the node no longer accepts, which the courier
    /// meets by dropping the key, fetching the newest and sealing again;
    /// otherwise queued, and delivered at once to a live session.
    pub fn submit(&mut self, from: [u8; 32], to: [u8; 32], sealed: Vec<u8>, plain: Vec<u8>) {
        let node = self.node_of(&to);
        let n = Sealed::decode(&sealed).expect("a seal").n;
        let now = self.now();
        if !self.nodes[node].keys.accepts(&to, n, now) {
            self.refused.push((to, n));
            let c = self.s.h.client(&from);
            c.node_key_refused(&to);
            let again = c.reseal(to, plain).expect("sealed again");
            self.carry(from, again);
            return;
        }
        self.enqueue(to, n, sealed);
    }

    /// **A submission nobody will seal again**: `sealed` put to `to`'s
    /// node as a captured copy would be. Whether the node took it.
    pub fn inject(&mut self, to: [u8; 32], sealed: Vec<u8>) -> bool {
        let node = self.node_of(&to);
        let n = Sealed::decode(&sealed).expect("a seal").n;
        let now = self.now();
        if !self.nodes[node].keys.accepts(&to, n, now) {
            self.refused.push((to, n));
            return false;
        }
        self.enqueue(to, n, sealed);
        true
    }

    /// Queued at `to`'s node, at the next position, and delivered at once
    /// where `to` holds a live session.
    fn enqueue(&mut self, to: [u8; 32], n: u64, sealed: Vec<u8>) {
        let sn = self.nodes.get_mut(self.served_by[&to]).unwrap();
        let position = sn.next;
        sn.next += 1;
        sn.queues.entry(to).or_default().push(Queued {
            position,
            n,
            sealed,
        });
        if !self.away.contains(&to) {
            self.deliver(&to);
        }
    }

    /// What `client`'s node holds for it past its cursor, delivered in
    /// order: each `RelayedPayload` handed to `receive_relayed`, and what
    /// that causes carried. What each came to, or why it was discarded.
    pub fn deliver(&mut self, client: &[u8; 32]) -> Vec<Result<String, String>> {
        let node = self.node_of(client);
        let mut out = Vec::new();
        loop {
            let cursor = self
                .s
                .h
                .client(client)
                .payload
                .cursors
                .get(&kh(node))
                .copied()
                .unwrap_or(0);
            let sn = &self.nodes[node];
            let Some(q) = sn
                .queues
                .get(client)
                .and_then(|qs| qs.iter().find(|q| q.position > cursor))
                .cloned()
            else {
                break;
            };
            let bytes = sn.relayed(client, &q);
            match self.s.h.client(client).receive_relayed(kh(node), &bytes) {
                Ok((sender, d)) => {
                    let what = describe(&d);
                    self.delivered.push((sender, *client, what.clone()));
                    out.push(Ok(what));
                }
                Err(why) => {
                    self.discarded.push((*client, why.clone()));
                    out.push(Err(why));
                }
            }
            let back = self.s.h.client(client).outbox();
            self.carry(*client, back);
        }
        out
    }

    /// The recipient collects what waits at its node: what each came to,
    /// panicking on a discard.
    pub fn collect(&mut self, client: &str) -> Vec<String> {
        self.deliver(&kh(client))
            .into_iter()
            .map(|r| r.expect("opens at the recipient"))
            .collect()
    }

    /// What `client`'s node would hand it now, raw, before the client
    /// reads any of it: every `RelayedPayload` past its cursor.
    pub fn queued(&mut self, client: &str) -> Vec<Vec<u8>> {
        let node = self.node_of(&kh(client));
        let cursor = self
            .s
            .client(client)
            .payload
            .cursors
            .get(&kh(node))
            .copied()
            .unwrap_or(0);
        let sn = &self.nodes[node];
        sn.queues
            .get(&kh(client))
            .map(|qs| {
                qs.iter()
                    .filter(|q| q.position > cursor)
                    .map(|q| sn.relayed(&kh(client), q))
                    .collect()
            })
            .unwrap_or_default()
    }

    pub fn send(&mut self, from: &str, to: &str, text: &str) {
        let msgs = self
            .s
            .client(from)
            .send_payload(kh(to), KIND_APPLICATION, text.as_bytes())
            .unwrap();
        self.carry(kh(from), msgs);
    }

    /// The node-key requests `from` made, in order.
    pub fn requests(&self, from: &str) -> Vec<NodeKeyRequest> {
        self.fetches
            .iter()
            .filter(|(f, _, _)| f == from)
            .map(|(_, _, r)| r.clone())
            .collect()
    }

    /// The sealed submissions `from` made to `to`, in order.
    pub fn sealed_to(&self, from: &str, to: &str) -> Vec<Vec<u8>> {
        self.log
            .iter()
            .filter_map(|(f, _, m)| match m {
                Msg::Sealed { to: t, sealed, .. } if f == from && *t == kh(to) => {
                    Some(sealed.clone())
                }
                _ => None,
            })
            .collect()
    }
}
