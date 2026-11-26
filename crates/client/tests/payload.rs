//! Payload on the harness (design §14.2, §14.2.4; `wire-format.md` §7.8,
//! §7.8.1, §7.10; `light-client-requirements.md` §3): clients served by
//! nodes that hold a key for each recipient they serve, messages sealed to
//! the person and to that node's key, the direct path for what is not a
//! direct message, and the channel's dispatch once a seal is opened.

mod common;

use common::harness::device;
use common::net::*;
use common::*;
use rhtn_archive::node_key::{NodeKey, NodeKeyReply, NodeKeyRequest, decode_batch_reply};
use rhtn_archive::sealed::{Sealed, SealedBody};
use rhtn_archive::submission::{relayed, unrelayed};
use rhtn_client::ceremony::*;
use rhtn_client::local::DeviceCredential;
use rhtn_client::payload::*;
use rhtn_client::query::KeyGrant;
use rhtn_client::store::{Capture, Frame, SealParams, seal};
use rhtn_codec::cose::sha256;
use rhtn_crypto::seal::EncryptionSecret;
use rhtn_crypto::verify::EncryptionKey;
use rhtn_crypto::xwing;

/// `who`'s encryption secrets, as every device its phrase restores derives
/// them.
fn secret_of(who: &str) -> EncryptionSecret {
    EncryptionSecret::from_identity_root(id(who).identity_root())
}

/// `who`'s encryption key, as its card carries it, verified.
fn key_of(who: &str) -> EncryptionKey {
    rhtn_crypto::verify::encryption_key(&ids()[..], &rhtn_crypto::encryption_key::own(&id(who)))
        .expect("verifies under its own material")
}

/// What a `RelayedPayload` for `to` from `node` opens to, under `to`'s own
/// secrets: the seal and its body, before any attribution.
fn opened(to: &str, node: &str, bytes: &[u8]) -> (Sealed, SealedBody) {
    let r = unrelayed(bytes).expect("a relayed payload");
    open_relayed(&secret_of(to), &kh(to), &kh(node), &r).expect("opens")
}

/// The newest key `node` holds for `to`, as a sender holds it.
fn node_key_for(n: &mut Net, to: &str) -> NodeKeyHeld {
    let node = n.node_of(&kh(to));
    let now = n.now();
    let record = n
        .nodes
        .get_mut(node)
        .unwrap()
        .keys
        .newest(&id(node), &kh(to), now);
    let k = NodeKey::parse(&record).expect("a node key");
    NodeKeyHeld {
        node: kh(node),
        n: k.fields.n,
        public: Box::new(k.fields.public),
    }
}

/// Randomness for a hand-made seal: the bytes of a counter, never twice
/// the same.
fn counter(start: u8) -> impl FnMut(&mut [u8]) {
    let mut i = start;
    move |out: &mut [u8]| {
        for b in out {
            i = i.wrapping_add(1);
            *b = i;
        }
    }
}

/// `who` restarted from what it wrote, on a device of its own: the client
/// the harness holds for it is replaced.
fn restart(n: &mut Net, who: &'static str, seed: u64) {
    let blob = n.s.client(who).durable();
    let (dev, handles) = device(n.s.clock.clone(), seed, 0);
    handles.reach.0.set(n.s.handles[who].reach.0.get());
    let mut c = Client::new(id(who), ids(), Config::default(), dev);
    c.restore_durable(&blob).expect("opens whole");
    n.s.h.clients.insert(kh(who), c);
    n.s.handles.insert(who, handles);
}

/// What was delivered to `to`, as it came to.
fn said_to(n: &Net, from: &str, to: &str) -> Vec<String> {
    n.delivered
        .iter()
        .filter(|(f, t, _)| *f == kh(from) && *t == kh(to))
        .map(|(_, _, d)| d.clone())
        .collect()
}

// acceptance: PAY-07
// functional: MAIL-023
// functional: PAY-013
/// **The population's node keys, prefetched as one sweep** at attach:
/// one batch to the node that serves several of them, in ascending
/// keyhash order, and no single-recipient request for any; only those
/// this client holds an encryption key for are asked about, since nobody
/// else can be written to.
#[test]
fn node_keys_are_prefetched_for_the_whole_horizon_as_one_sweep() {
    let mut n = net(
        &["w1", "w2"],
        &[("alice", "w2"), ("bob", "w1"), ("carol", "w1")],
    );
    // somebody served at the same node whose card alice does not hold
    n.served_by.insert(kh("w2"), "w1");
    n.attach("alice");
    let reqs = n.requests("alice");
    assert_eq!(reqs.len(), 1, "one sweep, to the one node serving them");
    assert_eq!(n.fetches[0].1, "w1", "asked of the node that serves them");
    let NodeKeyRequest::Batch { recipients, .. } = &reqs[0] else {
        panic!("a batch: {:?}", reqs[0])
    };
    let mut want = vec![kh("bob"), kh("carol")];
    want.sort();
    assert_eq!(
        *recipients, want,
        "the population in ascending keyhash order, without S itself or a party it cannot write to"
    );
    let held = &n.s.client("alice").payload.node_keys;
    for who in ["bob", "carol"] {
        let k = held.get(&kh(who)).expect("a key held for each");
        assert_eq!(k.node, kh("w1"), "the node serving {who}");
    }
    assert!(!held.contains_key(&kh("w2")));
    // and a send to either seals at once: nothing more is asked
    n.send("alice", "bob", "no fetch needed");
    assert_eq!(n.requests("alice").len(), 1, "no further request");
    assert_eq!(
        said_to(&n, "alice", "bob"),
        vec!["Application(no fetch needed)"]
    );
}

// acceptance: PAY-32
/// **A batch is answered one reply per recipient in the request's own
/// order**, each echoing the one nonce, a failure in place of a key the
/// node cannot give; and the client holds a key for each recipient answered
/// and none for the one refused.
#[test]
fn a_batch_is_answered_in_order_with_a_failure_in_place_and_the_client_takes_what_came() {
    let mut n = net(&["w1"], &[("alice", "w1"), ("bob", "w1"), ("carol", "w1")]);
    // R1 < R2 < R3 by keyhash, the node serving R1 and R3 and not R2
    let mut rs = [kh("bob"), kh("carol"), kh("w2")];
    rs.sort();
    let (r1, r2, r3) = (rs[0], rs[1], rs[2]);
    let nonce = [0x5a; 16];
    let req = NodeKeyRequest::for_recipients(vec![r3, r1, r2], nonce).unwrap();
    assert_eq!(
        req.recipients(),
        vec![r1, r2, r3],
        "ascending keyhash order"
    );
    let now = n.now();
    let keys = &mut n.nodes.get_mut("w1").unwrap().keys;
    let reply = keys
        .answer(&id("w1"), &|r| *r != r2, &req.encode(), now)
        .expect("answered");
    let replies = decode_batch_reply(&reply).expect("a batch reply");
    assert_eq!(replies.len(), 3);
    assert!(replies.iter().all(|r| r.nonce == nonce), "the one nonce");
    let newest =
        |keys: &mut rhtn_node::node_keys::NodeKeyService, r| keys.newest(&id("w1"), &r, now);
    assert_eq!(replies[0].outcome, Ok(newest(keys, r1)), "R1's newest key");
    assert_eq!(
        replies[1],
        NodeKeyReply {
            nonce,
            outcome: Err(rhtn_archive::node_key::FAIL_NOT_SERVED)
        },
        "no key and failure code 0"
    );
    assert_eq!(replies[2].outcome, Ok(newest(keys, r3)), "R3's newest key");
    // the client takes the two keys, each naming its recipient and the node
    let out =
        n.s.client("alice")
            .take_node_key_reply(&kh("w1"), &req.recipients(), &reply);
    assert!(out.is_empty(), "nothing was waiting");
    let held = &n.s.client("alice").payload.node_keys;
    assert!(held.contains_key(&r1) && held.contains_key(&r3));
    assert!(!held.contains_key(&r2), "nothing for the one refused");
}

/// **A node key that does not verify under the node it came from is not
/// held**: one signed by another, or naming another recipient, is refused,
/// and what waits for it still waits.
#[test]
fn a_node_key_that_is_not_the_nodes_or_not_the_recipients_is_not_held() {
    let mut n = net(&["w1"], &[("alice", "w1"), ("bob", "w1")]);
    let msgs =
        n.s.client("alice")
            .send_payload(kh("bob"), KIND_APPLICATION, b"waits")
            .unwrap();
    assert_eq!(msgs, vec![Msg::NodeKeyRequest(vec![kh("bob")])]);
    let now = n.now();
    let req = NodeKeyRequest::for_recipients(vec![kh("bob")], [1; 16]).unwrap();
    // carol's node key for bob, answered as though w1 had sent it
    let mut other = rhtn_node::node_keys::NodeKeyService::new(Default::default());
    let forged = other
        .answer(&id("carol"), &|_| true, &req.encode(), now)
        .unwrap();
    assert!(
        n.s.client("alice")
            .take_node_key_reply(&kh("w1"), &[kh("bob")], &forged)
            .is_empty(),
        "nothing sealed under a key w1 did not sign"
    );
    // w1's key for carol, offered as bob's
    let carols = n
        .nodes
        .get_mut("w1")
        .unwrap()
        .keys
        .answer(
            &id("w1"),
            &|_| true,
            &NodeKeyRequest::for_recipients(vec![kh("carol")], [1; 16])
                .unwrap()
                .encode(),
            now,
        )
        .unwrap();
    assert!(
        n.s.client("alice")
            .take_node_key_reply(&kh("w1"), &[kh("bob")], &carols)
            .is_empty(),
        "nothing sealed under a key for somebody else"
    );
    let a = n.s.client("alice");
    assert!(!a.payload.node_keys.contains_key(&kh("bob")));
    assert_eq!(a.payload.pending[&kh("bob")].len(), 1, "it still waits");
}

// acceptance: PAY-11
// functional: PAY-020
/// **Every identity is a leaf, the patron included**: a client attached
/// at the node its patron operates — which speaks as the patron — seals
/// to the patron as to any leaf, through the node that serves the patron;
/// nothing goes to the node as material of its own, and each recipient
/// opens its own and nobody else's.
#[test]
fn the_patron_is_sealed_to_like_any_leaf() {
    let mut n = net(&["w1"], &[("alice", "w1"), ("bob", "w1"), ("w1", "w1")]);
    n.attach("w1");
    n.attach("bob");
    n.attach("alice");
    assert!(
        n.requests("alice")
            .iter()
            .any(|r| r.recipients().contains(&kh("w1"))),
        "a node-key request names P"
    );
    n.away.insert(kh("w1"));
    n.away.insert(kh("bob"));
    let to_patron =
        n.s.client("alice")
            .send_payload(kh("w1"), KIND_APPLICATION, b"to my patron")
            .unwrap();
    let to_leaf =
        n.s.client("alice")
            .send_payload(kh("bob"), KIND_APPLICATION, b"to a leaf")
            .unwrap();
    for (msgs, to) in [(&to_patron, "w1"), (&to_leaf, "bob")] {
        assert!(
            matches!(msgs.as_slice(), [Msg::Sealed { to: t, .. }] if *t == kh(to)),
            "one relay submission for {to}, sealed: {msgs:?}"
        );
    }
    n.carry(kh("alice"), to_patron);
    n.carry(kh("alice"), to_leaf);
    // each sealed to its recipient's encryption key and w1's key for it
    let for_patron = n.queued("w1");
    let for_leaf = n.queued("bob");
    let (sealed, body) = opened("w1", "w1", &for_patron[0]);
    assert_eq!(sealed.n, n.s.client("alice").payload.node_keys[&kh("w1")].n);
    assert_eq!(body.bytes, b"to my patron");
    // the leaf's seal does not open as the patron's, nor the other way
    for (bytes, wrong) in [(&for_leaf[0], "w1"), (&for_patron[0], "bob")] {
        let r = unrelayed(bytes).unwrap();
        assert_eq!(
            open_relayed(&secret_of(wrong), &kh(wrong), &kh("w1"), &r).err(),
            Some(PayloadError::Unopened),
            "{wrong} cannot open another's"
        );
    }
    assert_eq!(n.collect("w1"), vec!["Application(to my patron)"]);
    assert_eq!(n.collect("bob"), vec!["Application(to a leaf)"]);
}

// functional: PAY-001
/// **The seal through the client reproduces the canonical vector, and
/// opens only with both halves** (`wire-format.md` §7.8.1;
/// `test-vectors/records.md`, "The seal"). The primitives are held to
/// their own specifications' vectors in `rhtn-crypto`; this is the
/// composition as the client makes it — the body's fields, the tag over
/// them, the message key over both halves — fed the vector's randomness.
#[test]
fn the_seal_reproduces_the_canonical_vector_and_opens_only_with_both_halves() {
    let canonical = fixture("P-sealed");
    let canonical_body = fixture("P-sealed-body");
    let body = SealedBody::decode(&canonical_body).expect("the body decodes");
    let node_key = NodeKey::parse(&fixture("P-node-key")).expect("the node key");
    node_key.verify(&ids()[..]).expect("signed by bob's node");
    assert_eq!(
        (
            node_key.fields.recipient,
            node_key.fields.node,
            node_key.fields.n
        ),
        (kh("carol"), kh("bob"), 3)
    );
    let held = NodeKeyHeld {
        node: kh("bob"),
        n: 3,
        public: Box::new(node_key.fields.public),
    };
    // the vector's own randomness, in the order a seal draws it
    let mut stream: Vec<u8> = Vec::new();
    for label in ["r-1", "r-2", "n-1", "n-2"] {
        stream.extend_from_slice(&sha256(
            format!("rhtn-test-vectors:seal-eseed-{label}").as_bytes(),
        ));
    }
    stream.extend_from_slice(&sha256(b"rhtn-test-vectors:seal-nonce")[..12]);
    let mut at = 0;
    let mut fresh = |out: &mut [u8]| {
        out.copy_from_slice(&stream[at..at + out.len()]);
        at += out.len();
    };
    let alice = secret_of("alice");
    let from = Sender {
        keyhash: &kh("alice"),
        secret: &alice,
        alias: "alice",
        card: body.card.as_deref(),
    };
    let sealed = seal_to(
        &from,
        &kh("carol"),
        &key_of("carol"),
        &held,
        KIND_APPLICATION,
        b"hello carol",
        &mut fresh,
    )
    .expect("sealed");
    assert_eq!(sealed, canonical, "byte equality with P-sealed");

    // the node's half: what bob's node decapsulates under key 3
    let read = Sealed::decode(&sealed).unwrap();
    let node_secret =
        xwing::Secret::from_seed(sha256(b"rhtn-test-vectors:node-key-seed:bob:carol:3"));
    let ss_n = node_secret.decapsulate(&read.ct_n);
    let delivery = relayed(7, &sealed, &ss_n);
    assert_eq!(
        delivery,
        fixture("P-relayed"),
        "byte equality with P-relayed"
    );

    // carol opens it with both halves, to the canonical body, and its tag
    // verifies for alice
    let carol = secret_of("carol");
    let r = unrelayed(&delivery).unwrap();
    let (s, b) = open_relayed(&carol, &kh("carol"), &kh("bob"), &r).expect("opens");
    assert_eq!(b.encode(), canonical_body, "to the P-sealed-body fixture");
    assert!(tag_verifies(&carol, &kh("carol"), &key_of("alice"), &s, &b));
    // ... and not with either half replaced
    let mut wrong_half = r.clone();
    wrong_half.ss_n = zeroize::Zeroizing::new([0; 32]);
    assert_eq!(
        open_relayed(&carol, &kh("carol"), &kh("bob"), &wrong_half).err(),
        Some(PayloadError::Unopened),
        "the node's half replaced"
    );
    assert_eq!(
        open_relayed(&secret_of("bob"), &kh("carol"), &kh("bob"), &r).err(),
        Some(PayloadError::Unopened),
        "the recipient's half replaced: another identity's secrets"
    );
    assert_eq!(
        open_relayed(&carol, &kh("carol"), &kh("w1"), &r).err(),
        Some(PayloadError::Unopened),
        "the node named otherwise: the key binds it"
    );
    // and its tag does not verify for anybody but its sender
    assert!(!tag_verifies(&carol, &kh("carol"), &key_of("bob"), &s, &b));
    // **and a recipient holding nothing of alice attributes it by what it
    // carries**: the introduction in the body is alice's, its locator reads,
    // and its encryption key is the one the tag verified under
    let intro = rhtn_archive::locator::carried_introduction(
        b.card.as_deref().expect("a first message carries it"),
    )
    .expect("what it carries of its sender reads");
    assert_eq!(intro.identity.keyhash, kh("alice"));
    assert!(tag_verifies(
        &carol,
        &kh("carol"),
        &intro.encryption,
        &s,
        &b
    ));
}

// acceptance: PAY-14
/// **Protocol objects and application payload reach their handlers once
/// opened**: a `KeyGrant` and an application message sealed to the same
/// verifier, the one to the grant handler and the other to the
/// application.
#[test]
fn protocol_objects_and_application_payload_reach_their_handlers() {
    let mut n = net(&["w1"], &[("alice", "w1"), ("bob", "w1")]);
    n.attach("bob");
    n.attach("alice");
    // no direct path, so both are sealed and both pass bob's node
    n.s.handles["alice"].reach.0.set(false);
    // bob, a verifier, holds a capture of alice under a record both hold
    let mut w = World::new();
    let rec = w.meet("alice", "bob");
    let c = [11u8; 32];
    let seed = [31u8; 32];
    let key = rhtn_client::keys::capture_key(&seed, &kh("alice"), &kh("bob"), &c);
    let cap = Capture {
        modality: 0,
        template_version: 1,
        template: vec![1u8; 32],
        frames: vec![Frame {
            at_ms: 0,
            bytes: b"f".to_vec(),
        }],
    };
    n.s.client("bob").store.sealed.insert(
        rec.txid,
        seal(
            &SealParams::default(),
            &key,
            kh("alice"),
            kh("bob"),
            c,
            &cap,
        ),
    );
    n.s.client("bob")
        .store
        .records
        .insert(rec.txid, rec.bytes.clone());
    n.s.client("alice")
        .store
        .records
        .insert(rec.txid, rec.bytes.clone());
    let qid = [5u8; 32];
    let grant = KeyGrant {
        record: rec.txid,
        query_id: qid,
        key,
    }
    .encode();
    let msgs =
        n.s.client("alice")
            .send_payload(kh("bob"), KIND_KEY_GRANT, &grant)
            .unwrap();
    assert!(matches!(msgs.as_slice(), [Msg::Sealed { .. }]), "{msgs:?}");
    n.carry(kh("alice"), msgs);
    n.send("alice", "bob", "an application message");
    let got = said_to(&n, "alice", "bob");
    assert_eq!(got.len(), 2);
    assert!(
        got[0].starts_with("Grant(") && got[0].contains("Buffered"),
        "the grant reached the grant handler, buffered awaiting its query: {}",
        got[0]
    );
    assert_eq!(
        got[1], "Application(an application message)",
        "the application message reached the application"
    );
    assert_eq!(
        n.s.client("bob").verifier.buffered(),
        vec![qid],
        "the grant is held by the verifier, not the application"
    );
}

// acceptance: PAY-15
// functional: PAY-005
/// **A direct message always passes the recipient's node; other payload
/// takes the direct path where one is held, and the node where it is
/// not.** Application payload goes sealed through the recipient's node
/// though the two can reach each other; a protocol object goes direct;
/// and once the NATs defeat hole punching the same object goes through
/// the node serving the recipient — not the sender's — and opens there.
#[test]
fn a_direct_message_always_passes_the_node_and_other_payload_falls_back_to_it() {
    let mut n = net(&["w1", "w2"], &[("alice", "w1"), ("bob", "w2")]);
    n.attach("bob");
    n.attach("alice");
    const KIND_OTHER: u64 = 99;
    // the path is held: the direct message is sealed to bob's node anyway
    let msgs =
        n.s.client("alice")
            .send_payload(kh("bob"), KIND_APPLICATION, b"a direct message")
            .unwrap();
    assert!(
        matches!(msgs.as_slice(), [Msg::Sealed { to, .. }] if *to == kh("bob")),
        "sealed for bob's node, not the path: {msgs:?}"
    );
    n.carry(kh("alice"), msgs);
    // other payload takes the path, and nothing is sealed on it
    let msgs =
        n.s.client("alice")
            .send_payload(kh("bob"), KIND_OTHER, b"on the path")
            .unwrap();
    assert_eq!(
        msgs,
        vec![Msg::Payload {
            to: kh("bob"),
            bytes: wrap(KIND_OTHER, b"on the path")
        }]
    );
    n.carry(kh("alice"), msgs);
    // the NATs defeat hole punching: the same goes through bob's node
    n.s.handles["alice"].reach.0.set(false);
    n.s.handles["bob"].reach.0.set(false);
    n.away.insert(kh("bob"));
    let msgs =
        n.s.client("alice")
            .send_payload(kh("bob"), KIND_OTHER, b"over the relay")
            .unwrap();
    assert!(
        matches!(msgs.as_slice(), [Msg::Sealed { .. }]),
        "the direct candidates failed: {msgs:?}"
    );
    n.carry(kh("alice"), msgs);
    assert!(
        n.nodes["w1"].queues.is_empty(),
        "the sender's own node queues nothing"
    );
    assert_eq!(n.queued("bob").len(), 1, "it waits at the node serving bob");
    assert_eq!(n.collect("bob").len(), 1, "and the recipient opens it");
    assert_eq!(
        said_to(&n, "alice", "bob"),
        vec![
            "Application(a direct message)",
            "Application(on the path)",
            "Application(over the relay)"
        ]
    );
    // and the reply comes back the same way
    n.away.remove(&kh("bob"));
    n.s.client("bob");
    let back =
        n.s.client("bob")
            .send_payload(kh("alice"), KIND_OTHER, b"back over the relay")
            .unwrap();
    n.carry(kh("bob"), back);
    assert_eq!(
        said_to(&n, "bob", "alice"),
        vec!["Application(back over the relay)"]
    );
}

// acceptance: PAY-16
// functional: PAY-004
/// **A sealed message is attributed only to the sender whose tag
/// verifies; one that does not open, or whose tag fails, is discarded.**
/// Bob holds alice's introduction. Carol seals to bob naming alice, under
/// her own key: discarded, nothing shown. Carol's own, with her
/// introduction: attributed to her. A copy of that with a byte of its body
/// flipped: does not open, discarded.
#[test]
fn a_seal_is_attributed_only_by_a_verifying_tag_and_what_fails_is_discarded() {
    let mut n = bare_net(&["w1"], &[("alice", "w1"), ("bob", "w1"), ("carol", "w1")]);
    n.introduce("alice", "bob");
    n.introduce("bob", "carol");
    n.s.client("bob").known.retain(|i| i.keyhash != kh("carol"));
    n.attach("bob");
    n.attach("carol");
    // carol's seal naming alice, the tag under carol's own secret
    let node_key = node_key_for(&mut n, "bob");
    let carol = secret_of("carol");
    let forged = seal_to(
        &Sender {
            keyhash: &kh("alice"),
            secret: &carol,
            alias: "alice",
            card: None,
        },
        &kh("bob"),
        &key_of("bob"),
        &node_key,
        KIND_APPLICATION,
        b"not from alice",
        &mut counter(1),
    )
    .unwrap();
    assert!(
        n.inject(kh("bob"), forged),
        "the node takes any seal to its key"
    );
    assert_eq!(
        n.discarded,
        vec![(kh("bob"), PayloadError::TagFailed.to_string())],
        "the tag fails for the sender it names"
    );
    assert!(
        n.delivered.is_empty(),
        "nothing dispatched and nothing shown"
    );
    assert!(
        n.s.notices("bob").is_empty(),
        "and nothing said of it: {:?}",
        n.s.notices("bob")
    );

    // carol's own, naming herself, her introduction carried
    n.send("carol", "bob", "from carol");
    assert_eq!(said_to(&n, "carol", "bob"), vec!["Application(from carol)"]);
    let (sealed_bytes, plain) = match n.log.iter().rev().find_map(|(_, _, m)| match m {
        Msg::Sealed { sealed, plain, .. } => Some((sealed.clone(), plain.clone())),
        _ => None,
    }) {
        Some(x) => x,
        None => panic!("carol's submission"),
    };
    assert_eq!(plain, wrap(KIND_APPLICATION, b"from carol"));
    // a copy with one byte of the body's ciphertext flipped
    let mut copy = Sealed::decode(&sealed_bytes).unwrap();
    copy.body[20] ^= 1;
    assert!(n.inject(kh("bob"), copy.encode()));
    assert_eq!(
        n.discarded.last(),
        Some(&(kh("bob"), PayloadError::Unopened.to_string())),
        "it does not open"
    );
    assert_eq!(
        said_to(&n, "carol", "bob").len(),
        1,
        "and nothing more is shown"
    );

    // a body whose carried introduction is somebody else's is not carol's
    let alices = n.s.client("alice").own_introduction().unwrap();
    let wrong_intro = seal_to(
        &Sender {
            keyhash: &kh("carol"),
            secret: &carol,
            alias: "carol",
            card: Some(&alices),
        },
        &kh("bob"),
        &key_of("bob"),
        &node_key,
        KIND_APPLICATION,
        b"under alice's card",
        &mut counter(2),
    )
    .unwrap();
    assert!(n.inject(kh("bob"), wrong_intro));
    assert!(
        n.discarded
            .last()
            .is_some_and(|(_, why)| why.contains("not the sender's")),
        "{:?}",
        n.discarded
    );
    assert_eq!(said_to(&n, "carol", "bob").len(), 1);
}

// acceptance: PAY-23
// functional: PAY-023
/// **A replay opens once, across a restart too**: a captured submission
/// put to the node again within the grace after it rotated is delivered
/// and discarded, and so it is once the recipient has restarted from what
/// it wrote; past the grace the node refuses it outright. The message
/// opened first stands.
#[test]
fn a_replayed_seal_is_discarded_within_the_run_and_after_a_restart() {
    let mut n = net(&["w1"], &[("alice", "w1"), ("bob", "w1"), ("carol", "w1")]);
    n.attach("bob");
    n.attach("alice");
    n.send("alice", "bob", "first");
    assert_eq!(said_to(&n, "alice", "bob"), vec!["Application(first)"]);
    let captured = n.sealed_to("alice", "bob")[0].clone();
    let n1 = Sealed::decode(&captured).unwrap().n;
    // the rotation interval passes and somebody fetches bob's key: the
    // node rotates to n + 1 and accepts n for its grace
    n.s.clock.set(n.s.clock.get() + 86_400_000 + 1000);
    n.sweep("carol");
    assert_eq!(
        n.nodes["w1"].keys.held(&kh("bob")),
        vec![n1, n1 + 1],
        "rotated, the previous kept for its grace"
    );
    assert!(n.inject(kh("bob"), captured.clone()), "within the grace");
    assert_eq!(
        n.discarded,
        vec![(kh("bob"), PayloadError::Replayed.to_string())],
        "the copy is discarded"
    );
    // bob restarts from what it wrote, and the copy comes again
    restart(&mut n, "bob", 93);
    assert!(n.inject(kh("bob"), captured.clone()));
    assert_eq!(
        n.discarded.last(),
        Some(&(kh("bob"), PayloadError::Replayed.to_string())),
        "a replay after a restart is discarded too"
    );
    assert_eq!(
        said_to(&n, "alice", "bob"),
        vec!["Application(first)"],
        "neither copy dispatched; the one opened stands"
    );
    // past the grace the node takes nothing sealed to n
    n.s.clock.set(n.s.clock.get() + 31_000);
    assert!(!n.inject(kh("bob"), captured), "refused at the node");
    // and the restarted client goes on: a fresh message opens
    n.send("alice", "bob", "second");
    assert_eq!(
        said_to(&n, "alice", "bob"),
        vec!["Application(first)", "Application(second)"]
    );
}

/// **A sender holding a stale node key is refused and seals again**: the
/// courier drops the key, fetches the newest and the same plaintext goes
/// under it (`wire-format.md` §7.8).
#[test]
fn a_seal_to_a_key_the_node_no_longer_accepts_is_refused_and_sealed_again() {
    let mut n = net(&["w1"], &[("alice", "w1"), ("bob", "w1"), ("carol", "w1")]);
    n.attach("bob");
    n.attach("alice");
    let old = n.s.client("alice").payload.node_keys[&kh("bob")].n;
    // a day on, carol's sweep rotates bob's key; past the grace alice
    // still holds the old one
    n.s.clock.set(n.s.clock.get() + 86_400_000 + 1000);
    n.sweep("carol");
    n.s.clock.set(n.s.clock.get() + 31_000);
    n.send("alice", "bob", "after the rotation");
    assert_eq!(n.refused, vec![(kh("bob"), old)], "the stale seal refused");
    assert_eq!(
        n.s.client("alice").payload.node_keys[&kh("bob")].n,
        old + 1,
        "the newest fetched"
    );
    assert_eq!(
        said_to(&n, "alice", "bob"),
        vec!["Application(after the rotation)"],
        "and the same plaintext delivered under it"
    );
}

// functional: PAY-015
/// **The state a seal needs survives a restart**: the queue position
/// collected from each node and the hashes opened, so a message handed
/// again is discarded; what waits for a node key, and the parties heard
/// from. Node keys and the device's own card are never written, and the
/// identity's secrets only where a pairing handed them over.
#[test]
fn the_payload_state_writes_what_a_restart_needs_and_nothing_else() {
    let mut n = net(&["w1"], &[("alice", "w1"), ("bob", "w1")]);
    n.attach("bob");
    n.attach("alice");
    n.send("bob", "alice", "before the restart");
    let delivered = n.queued("alice");
    assert!(delivered.is_empty(), "collected already");
    let handed = {
        let q = n.nodes["w1"].queues[&kh("alice")][0].clone();
        n.nodes["w1"].relayed(&kh("alice"), &q)
    };
    let position = unrelayed(&handed).unwrap().position;
    restart(&mut n, "alice", 91);
    let a = n.s.client("alice");
    assert_eq!(
        a.payload.cursors.get(&kh("w1")),
        Some(&position),
        "the newest position it held"
    );
    assert!(a.payload.heard_from.contains(&kh("bob")));
    assert!(a.payload.node_keys.is_empty(), "node keys are not written");
    assert_eq!(
        a.encryption_secrets().map(|(k, _)| k),
        Some(*secret_of("alice").kem_seed()),
        "derived again from the seed"
    );
    assert_eq!(
        a.receive_relayed(kh("w1"), &handed).unwrap_err(),
        PayloadError::Replayed.to_string(),
        "handed again, it is discarded"
    );

    // the state's own round trip
    let mut p = PayloadState {
        secret: Some(secret_of("carol")),
        card: Some(b"a card".to_vec()),
        ..PayloadState::default()
    };
    p.node_keys.insert(
        kh("bob"),
        NodeKeyHeld {
            node: kh("w1"),
            n: 3,
            public: Box::new([7; xwing::PUBLIC_BYTES]),
        },
    );
    p.pending
        .insert(kh("bob"), vec![wrap(KIND_APPLICATION, b"waits")]);
    p.heard_from.insert(kh("carol"));
    assert!(p.first_opening(&kh("w1"), 1, [1; 32]));
    p.cursors.insert(kh("w1"), 9);
    let back = PayloadState::decode(&p.encode()).expect("reads");
    assert!(back.secret.is_none(), "a derived secret is not written");
    assert!(back.card.is_none() && back.node_keys.is_empty());
    assert_eq!(back.pending, p.pending, "what waits for a key");
    assert_eq!(back.heard_from, p.heard_from);
    assert_eq!(back.opened, p.opened);
    assert_eq!(back.cursors, p.cursors);
    p.secret_handed = true;
    let back = PayloadState::decode(&p.encode()).expect("reads");
    let s = back.secret.as_ref().expect("a handed secret is written");
    assert!(back.secret_handed);
    assert_eq!(s.kem_seed(), secret_of("carol").kem_seed());
    assert_eq!(*s.auth_secret(), *secret_of("carol").auth_secret());
    // a blob cut short, or one of another shape, is refused whole
    let bytes = p.encode();
    assert!(PayloadState::decode(&bytes[..bytes.len() / 2]).is_none());
    assert!(PayloadState::decode(b"\x80").is_none());
}

// functional: PAY-023
/// **What was opened is remembered under the node's two newest keys**: a
/// message under an older key than both cannot be submitted again, so it
/// is taken as a replay without a record of it; the same hash under a key
/// held opens once.
#[test]
fn what_was_opened_is_remembered_under_the_two_newest_keys() {
    let mut p = PayloadState::default();
    let node = kh("w1");
    assert!(p.first_opening(&node, 1, [1; 32]));
    assert!(!p.first_opening(&node, 1, [1; 32]), "the same, again");
    assert!(p.first_opening(&node, 2, [2; 32]));
    assert!(p.first_opening(&node, 3, [3; 32]));
    assert_eq!(
        p.opened[&node].keys().copied().collect::<Vec<_>>(),
        vec![2, 3],
        "the {OPENED_KEYS} newest"
    );
    assert!(
        !p.first_opening(&node, 1, [9; 32]),
        "older than every key remembered: a replay"
    );
    assert!(!p.first_opening(&node, 3, [3; 32]));
    assert!(
        p.first_opening(&kh("w2"), 1, [1; 32]),
        "another node's numbers are its own"
    );
}

// acceptance: PAY-29
/// **The encryption key derives from the seed**, so a device restored from
/// the phrase holds the same key to the byte and opens what was sealed
/// to the person before it existed; and an introduction whose encryption
/// key is not the person's is not taken, so nothing is sealed to it.
#[test]
fn a_device_restored_from_the_phrase_opens_what_was_sealed_before_it() {
    let mut n = net(&["w1", "w2"], &[("alice", "w1"), ("bob", "w2")]);
    n.attach("bob");
    n.attach("alice");
    let first_key = n.s.client("bob").own_encryption_key().unwrap();
    n.away.insert(kh("bob"));
    n.send("alice", "bob", "sealed before the restore");
    // a second device from bob's phrase: nothing written, no cursor
    let (dev, handles) = device(n.s.clock.clone(), 94, 0);
    let restored = Client::new(id("bob"), ids(), Config::default(), dev);
    assert_eq!(
        restored.own_encryption_key().unwrap(),
        first_key,
        "byte for byte the first device's"
    );
    assert_eq!(
        restored.encryption_secrets().map(|(k, a)| (k, *a)),
        n.s.client("bob").encryption_secrets().map(|(k, a)| (k, *a))
    );
    assert!(
        restored.payload.cursors.is_empty(),
        "attaches with no field 5"
    );
    n.s.h.clients.insert(kh("bob"), restored);
    n.s.handles.insert("bob", handles);
    assert_eq!(
        n.collect("bob"),
        vec!["Application(sealed before the restore)"],
        "handed everything still held, and it opens"
    );

    // the malformed introductions of bob, handed to carol, who holds nothing
    // of him
    let mut m = bare_net(&["w1"], &[("bob", "w1"), ("carol", "w1")]);
    let good = m.s.client("bob").own_introduction().unwrap();
    let read = rhtn_archive::locator::Introduction::parse(&good).unwrap();
    let another_keyhash = rhtn_archive::locator::introduction(
        &read.material,
        &rhtn_crypto::encryption_key::own(&id("carol")),
        &read.locator.bytes,
    );
    let mut bad_signature = rhtn_crypto::encryption_key::own(&id("bob"));
    let last = bad_signature.len() - 1;
    bad_signature[last] ^= 1;
    let unsigned =
        rhtn_archive::locator::introduction(&read.material, &bad_signature, &read.locator.bytes);
    for bad in [another_keyhash, unsigned] {
        assert!(
            m.s.client("carol").take_introduction(&bad).is_err(),
            "not taken"
        );
        assert_eq!(
            m.s.client("carol")
                .send_payload(kh("bob"), KIND_APPLICATION, b"to bob")
                .unwrap_err(),
            PayloadError::NoEncryptionKey,
            "nothing sealed to the key it carried"
        );
    }
    assert!(m.log.is_empty());
}

// acceptance: PAY-30
/// **A paired device holds the person's two encryption secrets**, handed
/// in its credential, and with them opens what is sealed to the person and
/// sends as the person; the card others hold is unchanged by the pairing.
#[test]
fn a_paired_device_opens_and_sends_as_the_person() {
    let mut n = net(&["w1", "w2"], &[("alice", "w1"), ("bob", "w2")]);
    n.attach("bob");
    n.attach("alice");
    let card_before = n.s.client("bob").store.contacts[&kh("alice")]
        .encryption_key
        .clone()
        .expect("bob holds alice's card");
    // the desktop presents a key of its own and holds no seed
    let d2 = [0x42u8; 32];
    let (dev, _h) = device(n.s.clock.clone(), 77, 0);
    let mut desktop = Client::delegated(id("alice").public, d2, ids(), Config::default(), dev);
    assert!(!desktop.holds_seed());
    assert!(
        desktop.encryption_secrets().is_none(),
        "nothing to open with yet"
    );
    // the phone answers its introduction with the credential
    let now = n.now();
    let phone = n.s.client("alice");
    let (kem_seed, auth) = phone.encryption_secrets().expect("the phone holds them");
    let credential = DeviceCredential {
        run: phone.delegate_run(&d2, now, 3).unwrap(),
        kem_seed: zeroize::Zeroizing::new(kem_seed),
        auth,
    };
    let carried = DeviceCredential::decode(&credential.encode()).expect("reads");
    assert_eq!(carried.run.len(), 3, "the run of delegations");
    assert_eq!(*carried.kem_seed, *secret_of("alice").kem_seed());
    assert_eq!(*carried.auth, *secret_of("alice").auth_secret());
    desktop.take_encryption_secrets(&carried.kem_seed, &carried.auth);
    let bobs = n.s.client("bob").own_introduction().unwrap();
    desktop.take_introduction(&bobs).unwrap();

    // bob seals to alice: the desktop opens it from the node, as the phone does
    n.away.insert(kh("alice"));
    n.send("bob", "alice", "to every device");
    let waiting = n.queued("alice");
    let (from, got) = desktop
        .receive_relayed(kh("w1"), &waiting[0])
        .expect("the desktop opens it");
    assert_eq!(
        (from, describe(&got)),
        (kh("bob"), "Application(to every device)".into())
    );
    assert_eq!(n.collect("alice"), vec!["Application(to every device)"]);

    // the desktop writes to bob, as alice
    let asked = desktop
        .send_payload(kh("bob"), KIND_APPLICATION, b"from the desktop")
        .unwrap();
    assert_eq!(asked, vec![Msg::NodeKeyRequest(vec![kh("bob")])]);
    let req = NodeKeyRequest::for_recipients(vec![kh("bob")], [9; 16]).unwrap();
    let now = n.now();
    let reply = n
        .nodes
        .get_mut("w2")
        .unwrap()
        .keys
        .answer(&id("w2"), &|_| true, &req.encode(), now)
        .unwrap();
    let out = desktop.take_node_key_reply(&kh("w2"), &req.recipients(), &reply);
    let [Msg::Sealed { sealed, .. }] = out.as_slice() else {
        panic!("sealed: {out:?}")
    };
    assert!(n.inject(kh("bob"), sealed.clone()));
    assert_eq!(
        said_to(&n, "alice", "bob"),
        vec!["Application(from the desktop)"],
        "bob attributes it to alice"
    );
    assert_eq!(
        n.s.client("bob").store.contacts[&kh("alice")].encryption_key,
        Some(card_before),
        "alice's card as bob holds it is the same"
    );
}

// acceptance: PAY-33
// functional: PAY-023
/// **A sender not held as a contact is shown unverified, under its alias,
/// and cannot be answered**: alice, restored from her phrase holding
/// nothing of bob, collects a later message of his — no introduction in
/// it — and one from carol under bob's alias; each is unverified, kept
/// apart by the keyhash it names, dispatched as nothing of anybody's, and
/// a reply to bob is refused before anything is sealed.
#[test]
fn an_unattributed_sender_is_shown_unverified_and_cannot_be_answered() {
    let mut n = net(&["w1"], &[("alice", "w1"), ("bob", "w1"), ("carol", "w1")]);
    n.attach("bob");
    n.attach("alice");
    // alice wrote to bob first, so his next carries no introduction
    n.send("alice", "bob", "hello bob");
    assert!(n.s.client("bob").payload.heard_from.contains(&kh("alice")));
    n.away.insert(kh("alice"));
    n.send("bob", "alice", "a later message");
    let waiting = n.queued("alice");
    let (_, body) = opened("alice", "w1", &waiting[0]);
    assert_eq!(body.card, None, "field 3 is nil");
    let alias = body.alias.clone();

    // alice restored from her phrase: her own material and nothing else
    let (dev, handles) = device(n.s.clock.clone(), 95, 0);
    let mut restored = Client::new(id("alice"), ids(), Config::default(), dev);
    restored.known.retain(|k| k.keyhash == kh("alice"));
    n.s.h.clients.insert(kh("alice"), restored);
    n.s.handles.insert("alice", handles);

    // carol, holding alice's card, chooses bob's alias for a message of hers
    let node_key = node_key_for(&mut n, "alice");
    let carol = secret_of("carol");
    let same_alias = seal_to(
        &Sender {
            keyhash: &kh("carol"),
            secret: &carol,
            alias: &alias,
            card: None,
        },
        &kh("alice"),
        &key_of("alice"),
        &node_key,
        KIND_APPLICATION,
        b"under bob's alias",
        &mut counter(3),
    )
    .unwrap();
    assert!(n.inject(kh("alice"), same_alias));
    let mut shown = Vec::new();
    for bytes in n.queued("alice") {
        shown.push(
            n.s.client("alice")
                .receive_relayed(kh("w1"), &bytes)
                .expect("opened and shown"),
        );
    }
    let threads: Vec<([u8; 32], String, Vec<u8>)> = shown
        .iter()
        .map(|(from, d)| match d {
            Dispatched::Unverified {
                sender,
                alias,
                kind,
                bytes,
            } => {
                assert_eq!(sender, from);
                assert_eq!(*kind, KIND_APPLICATION);
                (*sender, alias.clone(), bytes.clone())
            }
            other => panic!("shown as somebody's: {}", describe(other)),
        })
        .collect();
    assert_eq!(
        threads,
        vec![
            (kh("bob"), alias.clone(), b"a later message".to_vec()),
            (kh("carol"), alias.clone(), b"under bob's alias".to_vec()),
        ],
        "one alias, two threads, apart by the keyhash"
    );
    // no reply: alice holds no contact for bob
    let a = n.s.client("alice");
    assert!(!a.store.contacts.contains_key(&kh("bob")));
    assert_eq!(
        a.send_payload(kh("bob"), KIND_APPLICATION, b"who are you?")
            .unwrap_err(),
        PayloadError::NoEncryptionKey
    );
    assert!(a.payload.pending.is_empty(), "nothing waits to be sealed");
    assert!(a.outbox().is_empty(), "nothing submitted");
    // and a protocol object from somebody unattributed is nothing at all
    let grant = seal_to(
        &Sender {
            keyhash: &kh("carol"),
            secret: &carol,
            alias: &alias,
            card: None,
        },
        &kh("alice"),
        &key_of("alice"),
        &node_key,
        KIND_KEY_GRANT,
        b"a grant",
        &mut counter(4),
    )
    .unwrap();
    assert!(n.inject(kh("alice"), grant));
    let bytes = n.queued("alice").pop().unwrap();
    assert!(
        n.s.client("alice")
            .receive_relayed(kh("w1"), &bytes)
            .is_err(),
        "discarded"
    );
}

// functional: PAY-021
/// **A first message from a stranger is attributed on arrival** by the
/// introduction inside its seal, which rides every message to the
/// recipient until the recipient has written back; the recipient takes
/// the sender as a contact from it, and can answer.
#[test]
fn a_stranger_is_attributed_by_the_introduction_in_the_seal_until_it_hears_back() {
    use rhtn_client::contacts::How;
    let mut n = bare_net(&["w1", "w2"], &[("alice", "w1"), ("bob", "w2")]);
    n.introduce("bob", "alice");
    n.s.client("bob").known.retain(|i| i.keyhash != kh("alice"));
    n.attach("bob");
    n.attach("alice");
    assert!(
        n.s.client("alice").payload.card.is_some(),
        "an attached client with a position carries its introduction"
    );
    n.away.insert(kh("bob"));
    n.send("alice", "bob", "hello, stranger");
    n.send("alice", "bob", "and again");
    for bytes in n.queued("bob") {
        let (_, body) = opened("bob", "w2", &bytes);
        let intro = body.card.expect("her introduction rides");
        let read = rhtn_archive::locator::Introduction::parse(&intro).unwrap();
        assert_eq!(read.identity.keyhash, kh("alice"));
    }
    assert_eq!(
        n.collect("bob"),
        vec!["Application(hello, stranger)", "Application(and again)"],
        "each attributed on arrival"
    );
    let bob = n.s.client("bob");
    assert!(
        bob.known.iter().any(|i| i.keyhash == kh("alice")),
        "bob holds alice now"
    );
    let c = bob.store.contacts.get(&kh("alice")).expect("as a contact");
    assert_eq!(c.how, How::Introduced);
    assert!(c.locator.is_some(), "with a locator to answer at");
    assert!(c.encryption_key.is_some(), "and her encryption key");
    // bob answers, which he can only because he holds her now
    n.away.remove(&kh("bob"));
    n.send("bob", "alice", "hello, alice");
    assert_eq!(
        said_to(&n, "bob", "alice"),
        vec!["Application(hello, alice)"]
    );
    // and alice's introduction stops riding
    n.away.insert(kh("bob"));
    n.send("alice", "bob", "no card now");
    let later = n.queued("bob");
    let (_, body) = opened("bob", "w2", &later[0]);
    assert_eq!(body.card, None, "nil once she has heard back");
    assert_eq!(n.collect("bob"), vec!["Application(no card now)"]);
}

// functional: PAY-021
/// **A stranger is answered from the message alone** [author, 2026-11-25:
/// *"cold calls are expected behavior from any user holding your card"*]:
/// a sender that holds where its anchor answers carries its whole card
/// inside the seal, so a recipient that held nothing of the sender or of
/// the anchor it sits under holds both on arrival, with the record of
/// where that anchor answers, and starts the walk back there.
#[test]
fn a_stranger_is_answered_from_the_card_its_first_message_carries() {
    let mut n = bare_net(&["w1", "w2"], &[("alice", "w1"), ("bob", "w2")]);
    // alice sits under w1 and holds where w1 answers, as a member does
    // once its node has said so
    n.s.adopted("alice", "w1", "w2", 1);
    let mut point = Vec::new();
    rhtn_transport::session::NetworkPoint::new([127, 0, 0, 1], Some(7447)).encode(&mut point);
    let record = rhtn_archive::endpoint::endpoint_record(
        &id("w1"),
        &[point],
        rhtn_archive::tx::Seqno {
            series: 1,
            counter: 1,
        },
    );
    n.s.client("alice")
        .take_endpoint(&record)
        .expect("w1's own");
    n.introduce("bob", "alice");
    // bob holds nothing of alice, and nothing of the anchor she sits under
    n.s.client("bob")
        .known
        .retain(|i| i.keyhash != kh("alice") && i.keyhash != kh("w1"));
    n.s.client("bob").store.contacts.remove(&kh("w1"));
    n.attach("bob");
    n.attach("alice");
    n.away.insert(kh("bob"));
    n.send("alice", "bob", "hello, stranger");
    let (_, body) = opened("bob", "w2", &n.queued("bob")[0]);
    let card = rhtn_archive::locator::Card::parse(&body.card.expect("her card rides"))
        .expect("a card, since she holds where her anchor answers");
    assert_eq!(card.introduction.identity.keyhash, kh("alice"));
    assert_eq!(card.anchor.keyhash, kh("w1"));
    assert_eq!(n.collect("bob"), vec!["Application(hello, stranger)"]);

    let bob = n.s.client("bob");
    assert!(bob.store.contacts.contains_key(&kh("alice")), "a contact");
    assert!(
        bob.known.iter().any(|i| i.keyhash == kh("w1")),
        "and her anchor's material, which bob had never held"
    );
    assert!(
        bob.store
            .contacts
            .get(&kh("w1"))
            .is_some_and(|c| c.endpoint_record.is_some()),
        "with the record of where it answers"
    );
    assert_eq!(
        bob.reach_for(&kh("alice")).ask.first().map(|a| a.node),
        Some(kh("w1")),
        "so the walk to her starts at the anchor her card named"
    );
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
    let pop = n.s.met("alice", "bob", "w1");

    // carol holds no acquaintance edge for the pair it was not party to
    let before = n.s.client("carol").evidence();
    assert!(
        !before
            .acquaintances
            .contains(&participants(kh("alice"), kh("bob")).into()),
        "not carol's to hold yet"
    );

    // carol asks alice for alice's own archive from that record
    let msgs =
        n.s.client("carol")
            .fetch_archive(kh("alice"), Some(pop), 8)
            .expect("asks");
    n.carry(kh("carol"), msgs);

    let served = n
        .delivered
        .iter()
        .filter(|(_, _, d)| d.starts_with("Served"))
        .count();
    assert_eq!(
        served, 1,
        "alice answered from her own archive: {:?}",
        n.delivered
    );
    let fetched = n
        .delivered
        .iter()
        .find(|(_, _, d)| d.starts_with("Fetched"))
        .expect("and carol took the reply");
    assert!(
        fetched.2.contains("Ok("),
        "the walk verified: {}",
        fetched.2
    );

    // and the record is carol's evidence now, which is what the pull is for
    let after = n.s.client("carol").evidence();
    assert!(
        after
            .acquaintances
            .contains(&participants(kh("alice"), kh("bob")).into()),
        "the acquaintance edge the fetch bought"
    );
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
    let pop = n.s.met("alice", "bob", "w1");

    // **an attestation delivery carries the nonce the evaluator generated**
    // (design §15): one that does not is visibly unsolicited
    let forged = rhtn_archive::chain::ArchiveReply {
        nonce: [0xcd; 16],
        records: vec![],
        more: false,
        frontier: Vec::new(),
    };
    let msgs =
        n.s.client("alice")
            .send_payload(kh("carol"), KIND_ARCHIVE_REPLY, &forged.encode())
            .expect("sent");
    n.carry(kh("alice"), msgs);
    let took = n
        .delivered
        .iter()
        .find(|(_, _, d)| d.starts_with("Fetched"))
        .expect("carol saw it");
    assert!(
        took.2.contains("Err("),
        "nothing outstanding with this party: {}",
        took.2
    );

    // carol asks alice for *bob's* archive: alice answers empty, which
    // says nothing about bob's archive
    let msgs =
        n.s.client("carol")
            .fetch_archive(kh("alice"), Some(pop), 8)
            .expect("asks");
    n.carry(kh("carol"), msgs);
    n.delivered.clear();
    let req = rhtn_archive::chain::ArchiveRequest {
        subject: kh("bob"),
        frontier: vec![pop],
        max_records: 8,
        stop_before: None,
        nonce: [3; 16],
    };
    let msgs =
        n.s.client("carol")
            .send_payload(kh("alice"), KIND_ARCHIVE_REQUEST, &req.encode())
            .expect("asks alice about bob");
    n.carry(kh("carol"), msgs);
    let served = n
        .delivered
        .iter()
        .find(|(_, _, d)| d.starts_with("Served"))
        .expect("alice answered");
    assert_eq!(
        served.2, "Served(0, more false)",
        "a subject it is not gets an empty batch"
    );
}

/// **A party this client knows and cannot place is placed by the
/// introduction its first message carries** [2026-11-15]: the material
/// stays as pinned, and the locator and the encryption key are taken.
// functional: PAY-021
#[test]
fn a_cold_introduction_from_a_known_party_places_it() {
    let mut n = net(&["bob"], &[("alice", "bob"), ("w1", "bob")]);
    n.attach("alice");
    let card =
        n.s.client("alice")
            .payload
            .card
            .clone()
            .expect("alice carries her introduction once attached");
    // w1 knows alice's material from the start and holds nothing of where she sits
    n.s.client("w1").store.contacts.remove(&kh("alice"));
    assert!(
        matches!(
            n.s.client("w1").reach_for(&kh("alice")).upstream,
            rhtn_client::horizon::Upstream::Unknown
        ),
        "nothing placed yet"
    );
    assert!(
        n.s.client("w1").take_cold_introduction(&kh("alice"), &card),
        "taken, though alice is known"
    );
    assert!(
        matches!(
            n.s.client("w1").reach_for(&kh("alice")).upstream,
            rhtn_client::horizon::Upstream::Position { .. }
        ),
        "placed by the introduction"
    );
    // a second cold call from a party already placed does nothing, and a
    // card that names somebody else is not taken for alice
    assert!(!n.s.client("w1").take_cold_introduction(&kh("alice"), &card));
    assert!(
        !n.s.client("w1").take_cold_introduction(&kh("carol"), &card),
        "the material must hash to the keyhash the channel names"
    );
}

/// **A ceremony's legs do not outlive the ceremony** [bench, 2026-10-24]:
/// what was queued for a peer with no node key yet — a trace, a request —
/// goes when the ceremony ends, and what the application queued stays.
/// Two phones that met again a day later each flushed 207 trace events of
/// an earlier exchange at the other on the first direct send after the
/// record (run a5a2f39-a2-nodeless-2).
#[test]
fn legs_queued_for_an_unreached_peer_go_with_the_ceremony() {
    let mut s = common::harness::setup(&["alice", "bob", "w1"]);
    s.client("alice")
        .begin(Some(kh("bob")), vec![kh("w1")], true)
        .expect("alice begins");
    s.client("alice").payload.pending.insert(
        kh("w1"),
        vec![
            wrap(KIND_EXCHANGE_TRACE, b"shape"),
            wrap(KIND_WITNESS_REQUEST, b"ask"),
            wrap(KIND_APPLICATION, b"a word for later"),
        ],
    );
    s.client("alice").abandon();
    let left = s
        .client("alice")
        .payload
        .pending
        .get(&kh("w1"))
        .cloned()
        .unwrap_or_default();
    assert_eq!(
        left,
        vec![wrap(KIND_APPLICATION, b"a word for later")],
        "the legs went, the application's word stays"
    );
}
