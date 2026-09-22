//! Payload on the harness (design §14.2; `wire-format.md` §7.8;
//! `light-client-requirements.md` §3): clients attached to serving nodes
//! that hold their prekeys and their mail, sessions opened on PQXDH with
//! and without a one-time key, the direct path and the relay, and the
//! channel's dispatch.

mod common;

use common::harness::*;
use common::*;
use rhtn_archive::prekey::*;
use rhtn_client::ceremony::*;
use rhtn_client::device::ChannelKind;
use rhtn_client::notice::Notice;
use rhtn_client::payload::*;
use rhtn_client::query::KeyGrant;
use rhtn_client::store::{Capture, Frame, SealParams, seal};
use rhtn_node::prekeys::{PrekeyConfig, PrekeyService};
use std::collections::{BTreeMap, VecDeque};

/// Mail waiting for one client: who sent each item and its bytes.
type Mail = VecDeque<([u8; 32], Vec<u8>)>;

/// A serving node as the harness stands it in: the prekeys it holds and
/// the mail it queues for the clients it serves.
struct ServingNode {
    prekeys: PrekeyService,
    queues: BTreeMap<[u8; 32], Mail>,
}

/// Clients on a harness, each served by a node, every message moved by
/// hand and logged.
struct Net {
    s: Setup,
    nodes: BTreeMap<&'static str, ServingNode>,
    served_by: BTreeMap<[u8; 32], &'static str>,
    log: Vec<(String, String, Msg)>,
    delivered: Vec<([u8; 32], [u8; 32], String)>,
}

const CLIENTS: [&str; 5] = ["alice", "bob", "carol", "w1", "w2"];

fn net(nodes: &[&'static str], serving: &[(&'static str, &'static str)]) -> Net {
    let s = setup(&CLIENTS, &[ChannelKind::Nfc]);
    s.clock.set(1_790_000_000_000);
    let nodes: BTreeMap<&'static str, ServingNode> = nodes.iter().map(|n| (*n, ServingNode { prekeys: PrekeyService::new(PrekeyConfig { one_time_per_requester_per_subject: 3, window_s: 3600, ..Default::default() }), queues: BTreeMap::new() })).collect();
    let served_by = serving.iter().map(|(c, n)| (kh(c), *n)).collect();
    Net { s, nodes, served_by, log: Vec::new(), delivered: Vec::new() }
}

/// What arrived, legibly: application text as text.
fn describe(d: &Dispatched) -> String {
    match d {
        Dispatched::Application(b) => format!("Application({})", String::from_utf8_lossy(b)),
        Dispatched::Grant(o) => format!("Grant({o:?})"),
        Dispatched::Late(r) => format!("Late({r:?})"),
        Dispatched::Candidates(b) => format!("Candidates({} bytes)", b.len()),
        Dispatched::ResponseCopy(r) => format!("ResponseCopy({r:?})"),
        Dispatched::Served { records, more } => format!("Served({records}, more {more})"),
        Dispatched::Fetched(r) => format!("Fetched({r:?})"),
    }
}

fn name(k: &[u8; 32]) -> String {
    NAMES.iter().find(|n| kh(n) == *k).map(|s| s.to_string()).unwrap_or_else(|| "?".into())
}

impl Net {
    fn now(&self) -> u64 {
        self.s.clock.get() / 1000
    }

    fn node_of(&self, client: &[u8; 32]) -> &'static str {
        self.served_by[client]
    }

    /// The population a client at `node` prefetches: everyone served there and elsewhere.
    fn population(&self) -> Vec<[u8; 32]> {
        self.served_by.keys().copied().collect()
    }

    fn attach(&mut self, client: &str) {
        let node = self.node_of(&kh(client));
        let pop = self.population();
        let msgs = self.s.client(client).attach(kh(node), &pop);
        self.carry(kh(client), msgs);
    }

    /// Sweep again: the first client to attach swept before the second had
    /// published, and a recipient can only attribute an initial message
    /// from a peer whose binding it holds.
    fn sweep(&mut self, client: &str) {
        let pop = self.population();
        let msgs = self.s.client(client).sweep(&pop);
        self.carry(kh(client), msgs);
    }

    /// Move a client's outgoing messages where they go, and whatever they
    /// cause in turn.
    fn carry(&mut self, from: [u8; 32], msgs: Vec<Msg>) {
        for m in msgs {
            let node = self.node_of(&from);
            self.log.push((name(&from), node.to_string(), m.clone()));
            match m {
                Msg::PublishBundle(b) => {
                    self.nodes.get_mut(node).unwrap().prekeys.publish(&ids(), &b).expect("a bundle by the client");
                }
                Msg::StockOneTime(keys) => {
                    assert!(self.nodes.get_mut(node).unwrap().prekeys.stock(from, keys), "the pool takes the deposit");
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
                    self.log.push((node.to_string(), name(&from), Msg::PrekeyReply(reply.clone())));
                    let more = self.s.h.client(&from).take_prekey_reply(&reply).expect("a reply the client can use");
                    self.carry(from, more);
                    for subject in exhausted {
                        self.log.push((node.to_string(), name(&subject), Msg::PoolExhausted));
                        let restock = self.s.h.client(&subject).on_pool_report(0);
                        self.carry(subject, restock);
                    }
                }
                Msg::Payload { to, bytes } => {
                    self.log.push((name(&from), name(&to), Msg::Payload { to, bytes: bytes.clone() }));
                    let d = self.s.h.client(&to).receive_payload(from, &bytes).expect("decrypts at the recipient");
                    self.delivered.push((from, to, describe(&d)));
                    // **what arriving caused goes out too**: a client
                    // answering an archive fetch replies on the same
                    // channel, and the reply is in its outbox
                    let back = self.s.h.client(&to).outbox();
                    self.carry(to, back);
                }
                Msg::Relay { to, bytes } => {
                    // my serving node hands it to the recipient's, which queues it
                    let dest = self.node_of(&to);
                    self.log.push((node.to_string(), dest.to_string(), Msg::Relay { to, bytes: bytes.clone() }));
                    self.nodes.get_mut(dest).unwrap().queues.entry(to).or_default().push_back((from, bytes));
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
    fn collect(&mut self, client: &str) -> Vec<String> {
        let node = self.node_of(&kh(client));
        let mut out = Vec::new();
        while let Some((from, bytes)) = self.nodes.get_mut(node).unwrap().queues.entry(kh(client)).or_default().pop_front() {
            let d = self.s.client(client).receive_payload(from, &bytes).expect("decrypts at the recipient");
            out.push(describe(&d));
        }
        out
    }

    /// The next item waiting for `client`, raw: what its serving node
    /// would hand over, before the client reads it.
    fn queued(&mut self, client: &str) -> ([u8; 32], Vec<u8>) {
        let node = self.node_of(&kh(client));
        self.nodes.get_mut(node).unwrap().queues.entry(kh(client)).or_default().pop_front().expect("something waiting")
    }

    fn send(&mut self, from: &str, to: &str, text: &str) {
        let msgs = self.s.client(from).send_payload(kh(to), KIND_APPLICATION, text.as_bytes()).unwrap();
        self.carry(kh(from), msgs);
    }

    fn requests(&self, from: &str) -> Vec<PrekeyRequest> {
        self.log.iter().filter(|(f, _, m)| f == from && matches!(m, Msg::PrekeyRequest(_))).map(|(_, _, m)| if let Msg::PrekeyRequest(b) = m { PrekeyRequest::decode(b).unwrap() } else { unreachable!() }).collect()
    }
}

// owed: PAY-01 was re-derived on 2026-09-22 to the bundle-per-device shape and this test holds the rule it superseded until the code lands; it is not marked
#[test]
fn attaching_publishes_a_signed_bundle_and_stocks_the_pool() {
    let mut n = net(&["w1"], &[("alice", "w1"), ("bob", "w1")]);
    n.attach("alice");
    let node = &n.nodes["w1"];
    let b = PrekeyBundle::parse(node.prekeys.bundle(&kh("alice")).expect("N holds a bundle for S")).unwrap();
    assert_eq!((b.subject, b.construction), (kh("alice"), 1), "field 2 equal to 1");
    assert_eq!(b.verify(&ids()), Ok(()), "a signature by S under the prekey tag over fields 1 to 4");
    assert!(node.prekeys.pool_size(&kh("alice")) > 0, "a non-empty one-time pool");
    assert_eq!(node.prekeys.pool_size(&kh("alice")), n.s.client("alice").payload.cfg.pool_target);
    // the blob reads as this construction's material
    let blob = Blob::decode(&b.blob).unwrap();
    assert_eq!(blob.pqspk.0.len(), 1184);
}

// acceptance: PAY-03
#[test]
fn the_pool_is_replenished_before_exhaustion_and_the_signed_prekey_rotated_on_its_interval() {
    let mut n = net(&["w1"], &[("alice", "w1"), ("bob", "w1"), ("carol", "w1")]);
    n.s.client("alice").payload.cfg = PayloadConfig { pool_target: 4, replenish_below: 2, signed_prekey_interval_s: 3600 };
    n.attach("alice");
    assert_eq!(n.nodes["w1"].prekeys.pool_size(&kh("alice")), 4);
    let first_bundle = n.nodes["w1"].prekeys.bundle(&kh("alice")).unwrap().clone();
    // requesters draw: the node reports the pool as it shrinks
    for (who, nonce) in [("bob", 1u8), ("carol", 2), ("w2", 3)] {
        let req = PrekeyRequest::One { subject: kh("alice"), one_time: true, nonce: [nonce; 16] }.encode();
        let r = PrekeyReply::decode(&n.nodes.get_mut("w1").unwrap().prekeys.answer(&kh(who), &req, 0).unwrap()).unwrap();
        assert!(r.one_time.is_some());
        let left = n.nodes["w1"].prekeys.pool_size(&kh("alice"));
        let msgs = n.s.client("alice").on_pool_report(left);
        if left < 2 {
            assert!(matches!(msgs.as_slice(), [Msg::StockOneTime(_)]), "below the threshold S uploads more, before zero");
        } else {
            assert!(msgs.is_empty());
        }
        n.carry(kh("alice"), msgs);
        assert!(n.nodes["w1"].prekeys.pool_size(&kh("alice")) > 0, "never reaches zero");
    }
    // the interval elapses: a new bundle with a later field 4
    n.s.clock.set(n.s.clock.get() + 3_600_000 + 1000);
    let msgs = n.s.client("alice").maintain();
    assert!(msgs.iter().any(|m| matches!(m, Msg::PublishBundle(_))));
    n.carry(kh("alice"), msgs);
    let second = PrekeyBundle::parse(n.nodes["w1"].prekeys.bundle(&kh("alice")).unwrap()).unwrap();
    assert!(second.published_at > PrekeyBundle::parse(&first_bundle).unwrap().published_at);
    assert_ne!(Blob::decode(&second.blob).unwrap().spk, Blob::decode(&PrekeyBundle::parse(&first_bundle).unwrap().blob).unwrap().spk, "a new signed prekey");
}

// acceptance: PAY-07
#[test]
fn reusable_material_is_prefetched_for_the_whole_org_as_one_sweep() {
    let mut n = net(&["w1"], &[("alice", "w1"), ("bob", "w1"), ("carol", "w1")]);
    n.attach("bob");
    n.attach("carol");
    n.attach("alice");
    n.sweep("bob");
    n.sweep("carol");
    let reqs = n.requests("alice");
    let batches: Vec<&PrekeyRequest> = reqs.iter().filter(|r| matches!(r, PrekeyRequest::Batch { .. })).collect();
    assert_eq!(batches.len(), 1, "one sweep");
    let PrekeyRequest::Batch { subjects, .. } = batches[0] else { unreachable!() };
    let mut want = vec![kh("bob"), kh("carol")];
    want.sort();
    assert_eq!(*subjects, want, "the population in ascending keyhash order, without S itself");
    assert!(reqs.iter().all(|r| !matches!(r, PrekeyRequest::One { one_time: false, .. })), "no single-subject reusable request for any of them");
    assert!(n.s.client("alice").payload.sessions.prefetched.contains_key(&kh("bob")));
    assert!(n.s.client("alice").payload.sessions.prefetched.contains_key(&kh("carol")));
}

// acceptance: PAY-08
#[test]
fn a_one_time_key_is_requested_only_when_opening_a_session() {
    let mut n = net(&["w1"], &[("alice", "w1"), ("bob", "w1")]);
    n.attach("bob");
    n.attach("alice");
    n.sweep("bob");
    // idle
    n.s.clock.set(n.s.clock.get() + 600_000);
    let msgs = n.s.client("alice").maintain();
    n.carry(kh("alice"), msgs);
    assert!(n.requests("alice").iter().all(|r| !matches!(r, PrekeyRequest::One { one_time: true, .. })), "none while idle");
    // the first message to T
    n.send("alice", "bob", "hello");
    let reqs = n.requests("alice");
    let ones: Vec<&PrekeyRequest> = reqs.iter().filter(|r| matches!(r, PrekeyRequest::One { one_time: true, .. })).collect();
    assert_eq!(ones.len(), 1, "exactly one");
    assert!(matches!(ones[0], PrekeyRequest::One { subject, .. } if *subject == kh("bob")), "naming T");
    let pos_req = n.log.iter().position(|(f, _, m)| f == "alice" && matches!(m, Msg::PrekeyRequest(_)) && matches!(PrekeyRequest::decode(if let Msg::PrekeyRequest(b) = m { b } else { unreachable!() }).unwrap(), PrekeyRequest::One { one_time: true, .. })).unwrap();
    let pos_msg = n.log.iter().position(|(f, _, m)| f == "alice" && matches!(m, Msg::Payload { .. })).unwrap();
    assert!(pos_req < pos_msg, "immediately before the first message");
    assert_eq!(n.delivered.len(), 1);
    assert!(n.delivered[0].2.contains("hello"));
    // a second message: no further request
    n.send("alice", "bob", "again");
    assert_eq!(n.requests("alice").iter().filter(|r| matches!(r, PrekeyRequest::One { one_time: true, .. })).count(), 1);
    assert_eq!(n.delivered.len(), 2);
}

// acceptance: PAY-11
#[test]
fn the_asynchronous_construction_is_used_leaf_to_leaf_only() {
    let mut n = net(&["w1"], &[("alice", "w1"), ("bob", "w1")]);
    n.attach("bob");
    n.attach("alice");
    n.sweep("bob");
    // to the patron: on the transport session, no prekey request
    let msgs = n.s.client("alice").send_payload(kh("w1"), KIND_APPLICATION, b"to my patron").unwrap();
    assert!(matches!(msgs.as_slice(), [Msg::Transport(b)] if b == b"to my patron"));
    n.carry(kh("alice"), msgs);
    assert!(n.requests("alice").iter().all(|r| !matches!(r, PrekeyRequest::One { subject, .. } if *subject == kh("w1"))), "no prekey request names P");
    // to a leaf: a one-time request and the construction
    n.send("alice", "bob", "to a leaf");
    assert!(n.requests("alice").iter().any(|r| matches!(r, PrekeyRequest::One { subject, one_time: true, .. } if *subject == kh("bob"))));
    let (_, _, m) = n.log.iter().find(|(f, _, m)| f == "alice" && matches!(m, Msg::Payload { .. })).unwrap();
    let Msg::Payload { bytes, .. } = m else { unreachable!() };
    assert!(!bytes.windows(9).any(|w| w == b"to a leaf"), "encrypted under the payload construction");
    assert!(n.delivered[0].2.contains("to a leaf"));
}

// owed: PAY-12 was re-derived on 2026-09-22 to the bundle-per-device shape and this test holds the rule it superseded until the code lands; it is not marked
#[test]
fn a_session_opens_on_reusable_material_alone_when_no_one_time_key_remains() {
    let mut n = net(&["w1"], &[("alice", "w1"), ("bob", "w1")]);
    n.attach("bob");
    n.attach("alice");
    n.sweep("bob");
    // T's pool at N is drained
    let mut drained = 0;
    loop {
        let left = n.nodes["w1"].prekeys.pool_size(&kh("bob"));
        if left == 0 {
            break;
        }
        let req = PrekeyRequest::One { subject: kh("bob"), one_time: true, nonce: [drained; 16] }.encode();
        let now = n.now();
        let requester = kh(&format!("w{}", 2 + drained % 2));
        let reply = n.nodes.get_mut("w1").unwrap().prekeys.answer(&requester, &req, now + drained as u64 * 4000).unwrap();
        let r = PrekeyReply::decode(&reply).unwrap();
        assert!(r.one_time.is_some());
        drained += 1;
    }
    n.nodes.get_mut("w1").unwrap().prekeys.take_exhausted();
    assert_eq!(n.nodes["w1"].prekeys.pool_size(&kh("bob")), 0);
    n.send("alice", "bob", "without a one-time key");
    let (_, _, reply) = n.log.iter().find(|(f, t, m)| f == "w1" && t == "alice" && matches!(m, Msg::PrekeyReply(_))).unwrap();
    let Msg::PrekeyReply(rb) = reply else { unreachable!() };
    let r = PrekeyReply::decode(rb).unwrap();
    assert!(r.bundle.is_some() && r.one_time.is_none(), "field 2 and no field 3");
    assert_eq!(n.delivered.len(), 1, "S still opens the session and delivers");
    assert!(n.delivered[0].2.contains("without a one-time key"));
    assert!(n.s.client("bob").payload.sessions.has_session(&kh("alice")));
    // and it runs on
    n.send("bob", "alice", "reply");
    assert!(n.delivered[1].2.contains("reply"));
}

// acceptance: PAY-14
#[test]
fn protocol_objects_and_application_payload_reach_their_handlers() {
    let mut n = net(&["w1"], &[("alice", "w1"), ("bob", "w1")]);
    n.attach("bob");
    n.attach("alice");
    n.sweep("bob");
    // bob, a verifier, holds a capture of alice under a record both hold;
    // alice grants against it over the same session as an application message
    let mut w = World::new();
    let rec = w.meet("alice", "bob");
    let c = [11u8; 32];
    let seed = [31u8; 32];
    let key = rhtn_client::keys::capture_key(&seed, &kh("alice"), &kh("bob"), &c);
    let cap = Capture { modality: 0, template_version: 1, template: vec![1u8; 32], frames: vec![Frame { at_ms: 0, bytes: b"f".to_vec() }] };
    n.s.client("bob").store.sealed.insert(rec.txid, seal(&SealParams::default(), &key, kh("alice"), kh("bob"), c, &cap));
    n.s.client("bob").store.records.insert(rec.txid, rec.bytes.clone());
    n.s.client("alice").store.records.insert(rec.txid, rec.bytes.clone());
    let qid = [5u8; 32];
    let grant = KeyGrant { record: rec.txid, query_id: qid, key }.encode();
    let msgs = n.s.client("alice").send_payload(kh("bob"), KIND_KEY_GRANT, &grant).unwrap();
    n.carry(kh("alice"), msgs);
    n.send("alice", "bob", "an application message");
    assert_eq!(n.delivered.len(), 2);
    assert!(n.delivered[0].2.starts_with("Grant("), "the grant reached the grant handler: {}", n.delivered[0].2);
    assert!(n.delivered[0].2.contains("Buffered"), "buffered awaiting its query, the named capture in reach: {}", n.delivered[0].2);
    assert!(n.delivered[1].2.starts_with("Application("), "{}", n.delivered[1].2);
    assert!(!n.delivered[1].2.starts_with("Grant("));
    assert_eq!(n.s.client("bob").verifier.buffered(), vec![qid], "the grant is held by the verifier, not the application");
}

// acceptance: PAY-15
#[test]
fn payload_is_relayed_through_the_serving_node_when_no_direct_path_exists() {
    let mut n = net(&["w1", "w2"], &[("alice", "w1"), ("bob", "w2")]);
    n.attach("bob");
    n.attach("alice");
    n.sweep("bob");
    // the NATs defeat hole punching
    n.s.handles["alice"].reach.0.set(false);
    n.s.handles["bob"].reach.0.set(false);
    n.send("alice", "bob", "over the relay");
    assert!(n.log.iter().all(|(_, _, m)| !matches!(m, Msg::Payload { .. })), "the direct candidates failed: nothing on the direct path");
    assert!(n.log.iter().any(|(f, t, m)| f == "w1" && t == "w2" && matches!(m, Msg::Relay { .. })), "relayed serving node to serving node");
    assert!(n.delivered.is_empty(), "nothing until the recipient collects");
    let got = n.collect("bob");
    assert_eq!(got.len(), 1);
    assert!(got[0].contains("over the relay"), "the recipient decrypts it: {}", got[0]);
    // and the reply comes back the same way
    n.send("bob", "alice", "back over the relay");
    assert!(n.collect("alice")[0].contains("back over the relay"));
}

// acceptance: PAY-16
#[test]
fn an_initial_session_opens_only_under_the_identity_key_bound_to_its_named_sender() {
    // one node, so a sweep reaches every bundle: bob holds alice's
    // verified bundle and carol's
    let mut n = net(&["w1"], &[("alice", "w1"), ("carol", "w1"), ("bob", "w1")]);
    n.attach("bob");
    n.attach("carol");
    n.attach("alice");
    n.sweep("bob");
    n.sweep("carol");
    // carol composes an initial message to bob under carol's own keys, and
    // it waits at bob's serving node
    n.s.handles["carol"].reach.0.set(false);
    n.send("carol", "bob", "not from alice");
    let (sender, carols) = n.queued("bob");
    assert_eq!(sender, kh("carol"));
    // delivered under alice's name: refused, nothing opened, nothing said
    let refused = n.s.client("bob").receive_payload(kh("alice"), &carols).unwrap_err();
    assert!(refused.contains("not the named sender's"), "{refused}");
    assert!(!n.s.client("bob").payload.sessions.has_session(&kh("alice")), "no session for the name it claimed");
    assert!(n.s.notices("bob").iter().any(|(_, x)| matches!(x, Notice::PayloadUnattributable { from } if *from == kh("alice"))));
    // the same bytes under carol's name decrypt: the message is well formed
    // and it is carol's
    let d = n.s.client("bob").receive_payload(kh("carol"), &carols).expect("carol's own message");
    assert!(matches!(&d, Dispatched::Application(b) if b == b"not from alice"), "{d:?}");
    // a sender whose binding this client does not hold: refused, and the
    // binding asked for, so the peer's next attempt is attributable
    let mut m = net(&["w1", "w2"], &[("alice", "w1"), ("bob", "w2")]);
    m.attach("alice");
    m.attach("bob");
    m.s.handles["bob"].reach.0.set(false);
    m.send("bob", "alice", "first contact");
    let (_, first) = m.queued("alice");
    assert!(m.s.client("alice").receive_payload(kh("bob"), &first).unwrap_err().contains("no bundle"));
    assert!(m.s.client("alice").payload.wanted.contains(&kh("bob")), "the binding is wanted");
    let asks = m.s.client("alice").maintain();
    assert!(asks.iter().any(|x| matches!(x, Msg::PrekeyRequest(_))), "maintenance asks for it");
}

// acceptance: PAY-17
#[test]
fn a_one_time_prekey_is_spent_only_when_the_message_it_opened_authenticates() {
    let mut n = net(&["w1", "w2"], &[("alice", "w1"), ("bob", "w2")]);
    n.attach("bob");
    n.attach("alice");
    n.sweep("bob");
    n.s.handles["alice"].reach.0.set(false);
    n.send("alice", "bob", "the real one");
    let (_, good) = n.queued("bob");
    // one ciphertext byte flipped: the copy fails
    let mut corrupt = good.clone();
    let last = corrupt.len() - 1;
    corrupt[last] ^= 1;
    assert!(n.s.client("bob").receive_payload(kh("alice"), &corrupt).is_err(), "the corrupted copy fails");
    // and the pair it named is still there, so the original still opens
    let d = n.s.client("bob").receive_payload(kh("alice"), &good).expect("the original still opens");
    assert!(matches!(&d, Dispatched::Application(b) if b == b"the real one"), "{d:?}");
    // spent now: the same bytes a second time find no pair and no session
    // is reopened
    assert!(n.s.client("bob").receive_payload(kh("alice"), &good).is_err(), "the pair is gone once its message authenticated");
}

/// **The subject holds their archive, so a patron evaluating you fetches
/// from you** (`light-client-requirements.md` §2) — peer-to-peer payload,
/// not something an infra node serves on your behalf.  This is the half of
/// design §15's pull that nobody answered: attestation is fetched on
/// demand, and nothing could be fetched because no client served.
// acceptance: ARC-20
#[test]
fn a_client_serves_its_own_archive_and_an_evaluator_verifies_the_walk_it_gets() {
    let mut n = net(&["w1"], &[("alice", "w1"), ("bob", "w1"), ("carol", "w1")]);
    for c in ["alice", "bob", "carol"] {
        n.attach(c);
    }
    // alice and bob meet: the record is in both their archives and in
    // nobody else's, because attestation is never flooded
    for c in ["alice", "bob", "carol"] {
        n.sweep(c);
    }
    n.s.face_off("alice", "bob");
    let pop = n.s.h.run(kh("alice"), kh("bob"), vec![kh("w2")], vec![kh("w1")]).expect("a ceremony");

    // carol holds no acquaintance edge for the pair it was not party to
    let before = n.s.client("carol").evidence();
    assert!(!before.acquaintances.contains(&participants(kh("alice"), kh("bob")).into()), "not carol's to hold yet");

    // carol asks alice for alice's own archive from that record
    let msgs = n.s.client("carol").fetch_archive(kh("alice"), Some(pop), 8).expect("asks");
    n.carry(kh("carol"), msgs);

    let served = n.delivered.iter().filter(|(_, _, d)| d.starts_with("Served")).count();
    assert_eq!(served, 1, "alice answered from her own archive: {:?}", n.delivered);
    let fetched = n.delivered.iter().find(|(_, _, d)| d.starts_with("Fetched")).expect("and carol took the reply");
    assert!(fetched.2.contains("Ok("), "the walk verified: {}", fetched.2);

    // and the record is carol's evidence now, which is what the pull is for
    let after = n.s.client("carol").evidence();
    assert!(after.acquaintances.contains(&participants(kh("alice"), kh("bob")).into()), "the acquaintance edge the fetch bought");
    assert!(after.acquaintances.len() > before.acquaintances.len());
}

/// The negatives: a fetch nobody asked for is not taken, and a subject
/// answers for its own archive and nobody else's.
// acceptance: ARC-21
#[test]
fn an_unsolicited_reply_is_not_taken_and_a_subject_answers_only_for_itself() {
    let mut n = net(&["w1"], &[("alice", "w1"), ("bob", "w1"), ("carol", "w1")]);
    for c in ["alice", "bob", "carol"] {
        n.attach(c);
    }
    for c in ["alice", "bob", "carol"] {
        n.sweep(c);
    }
    n.s.face_off("alice", "bob");
    let pop = n.s.h.run(kh("alice"), kh("bob"), vec![kh("w2")], vec![kh("w1")]).expect("a ceremony");

    // **an attestation delivery carries the nonce the evaluator generated**
    // (design §15): one that does not is visibly unsolicited
    let forged = rhtn_archive::chain::ArchiveReply { nonce: [0xcd; 16], records: vec![], more: false, continue_from: None };
    let msgs = n.s.client("alice").send_payload(kh("carol"), KIND_ARCHIVE_REPLY, &forged.encode()).expect("sent");
    n.carry(kh("alice"), msgs);
    let took = n.delivered.iter().find(|(_, _, d)| d.starts_with("Fetched")).expect("carol saw it");
    assert!(took.2.contains("Err("), "nothing outstanding with this party: {}", took.2);

    // carol asks alice for *bob's* archive: alice answers empty, which
    // says nothing about bob's archive
    let msgs = n.s.client("carol").fetch_archive(kh("alice"), Some(pop), 8).expect("asks");
    n.carry(kh("carol"), msgs);
    n.delivered.clear();
    let mut req = rhtn_archive::chain::ArchiveRequest { subject: kh("bob"), head: Some(pop), max_records: 8, stop_before: None, nonce: [3; 16] };
    req.subject = kh("bob");
    let msgs = n.s.client("carol").send_payload(kh("alice"), KIND_ARCHIVE_REQUEST, &req.encode()).expect("asks alice about bob");
    n.carry(kh("carol"), msgs);
    let served = n.delivered.iter().find(|(_, _, d)| d.starts_with("Served")).expect("alice answered");
    assert_eq!(served.2, "Served(0, more false)", "a subject it is not gets an empty batch");
}
