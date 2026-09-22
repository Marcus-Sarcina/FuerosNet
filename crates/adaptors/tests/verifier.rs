//! A verifier hosted in this process answers on the request stream of the
//! node hosting it (`wire-format.md` §9.2, §5.6): a node that is a
//! participant for its own key, a light client beside its serving node
//! for its own, and the subject's copy on the payload channel.

mod common;

use common::*;
use rhtn_adaptors::actor::Handle;
use rhtn_adaptors::courier::Courier;
use rhtn_adaptors::direct::{NoDirect, Reachable};
use rhtn_adaptors::serving::{Inboxes, LocalNode, Serving};
use rhtn_adaptors::verifier::Verifiers;
use rhtn_archive::Keyhash;
use rhtn_client::ceremony::{Config, Dispatched};
use rhtn_client::payload::KIND_KEY_GRANT;
use rhtn_client::query::*;
use rhtn_crypto::verify;
use rhtn_transport::session::*;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::mpsc::UnboundedReceiver;

struct Party {
    handle: Handle,
    courier: Arc<Courier>,
    app: UnboundedReceiver<(Keyhash, Dispatched)>,
}

/// A client hosted beside the node, with no direct path, as a verifier.
fn host(
    name: &'static str,
    cfg: Config,
    serving: &Arc<dyn Serving>,
    inboxes: &Inboxes,
    verifiers: &Arc<Verifiers>,
) -> Party {
    let reachable = Reachable::default();
    let handle = spawn_client(name, cfg, reachable.clone());
    let (courier, app) = Courier::new(
        handle.clone(),
        serving.clone(),
        Arc::new(NoDirect(reachable)),
    );
    inboxes.host(kh(name), courier.inbound());
    verifiers.host(handle.clone(), courier.clone());
    Party {
        handle,
        courier,
        app,
    }
}

fn buffered(ms: u64) -> Config {
    let mut c = Config::default();
    c.verifier.grant_buffer_ms = ms;
    c
}

async fn next(
    app: &mut UnboundedReceiver<(Keyhash, Dispatched)>,
    ms: u64,
) -> Option<(Keyhash, Dispatched)> {
    tokio::time::timeout(Duration::from_millis(ms), app.recv())
        .await
        .ok()
        .flatten()
}

fn query(subject: &str, querier: &str, verifier: &str, ceremony: u8) -> VerificationQuery {
    VerificationQuery {
        subject: kh(subject),
        querier: kh(querier),
        ceremony_id: [ceremony; 32],
        profile: vec![1, 2, 3],
        template_version: 1,
        verifier: kh(verifier),
    }
}

// acceptance: CER-30
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_query_is_answered_at_the_node_hosting_the_verifier_and_the_subject_gets_its_copy() {
    // bob is a node and a participant; alice a light client beside it;
    // carol asks over a session
    let scene = Scene::new();
    let node = live_node("bob", scene.table("bob", &["bob"]), "bob", &[]);
    let inboxes = Inboxes::default();
    let serving: Arc<dyn Serving> = LocalNode::new(node.clone(), inboxes.clone());
    let verifiers = Verifiers::new();
    let bob = host("bob", buffered(400), &serving, &inboxes, &verifiers);
    let mut alice = host("alice", buffered(400), &serving, &inboxes, &verifiers);
    verifiers.install(&node);
    // attached: material published where the other can fetch it
    bob.courier.attach(vec![kh("alice")]).await;
    alice.courier.attach(vec![kh("bob")]).await;
    // the first to attach swept before the second had published, and a
    // recipient attributes an initial message only under the binding it holds
    bob.courier.sweep(vec![kh("alice")]).await;
    // carol's query about alice, addressed to bob, consented by alice
    // within her open window
    let q = query("alice", "carol", "bob", 7);
    let q1 = q.clone();
    let (signed, _) = alice
        .handle
        .with(move |c| {
            c.subject.open_window([7; 32]);
            c.consent(&q1)
        })
        .await
        .expect("alice consents");
    let body = QueryRequest {
        query: q.clone(),
        consent: signed,
        selection_basis: 1,
    }
    .encode();
    let ccfg = client_cfg("carol");
    know(&ccfg, "bob", node.addr);
    let AttachOutcome::Attached(c) = attach(&ccfg, &client_ep(), kh("bob"), node.addr, false).await
    else {
        panic!("carol attaches")
    };
    let t0 = Instant::now();
    let reply = c
        .request(REQUEST_VERIFIER_QUERY, &body)
        .await
        .expect("answered on the stream");
    // no grant within the bound: unavailable, signed by bob, on carol's stream
    let r = Response::read(&reply).unwrap();
    assert_eq!(
        (r.verifier, r.subject, r.query_id, r.verdict),
        (kh("bob"), kh("alice"), q.query_id(), Verdict::Unavailable)
    );
    verify::response(&ids(), &reply, false).expect("bob's signature");
    assert!(
        t0.elapsed() >= Duration::from_millis(400),
        "the buffer bound passed first"
    );
    // the copy reached alice on the payload channel, from bob, about a
    // query she consented to
    let (from, d) = next(&mut alice.app, 3000).await.expect("the copy arrives");
    assert_eq!(from, kh("bob"));
    assert!(
        matches!(d, Dispatched::ResponseCopy(Ok(qid)) if qid == q.query_id()),
        "{d:?}"
    );
    // a verifier nobody here hosts: the stream fails, nothing signed
    let q2 = VerificationQuery {
        verifier: kh("w1"),
        ..q.clone()
    };
    let body2 = QueryRequest {
        query: q2.clone(),
        consent: consent(&id("alice"), &q2.query_id()),
        selection_basis: 1,
    }
    .encode();
    assert!(c.request(REQUEST_VERIFIER_QUERY, &body2).await.is_err());
    // a query naming a querier the stream did not authenticate as: closed
    let q3 = VerificationQuery {
        querier: kh("w2"),
        ceremony_id: [8; 32],
        ..q.clone()
    };
    let body3 = QueryRequest {
        query: q3.clone(),
        consent: consent(&id("alice"), &q3.query_id()),
        selection_basis: 1,
    }
    .encode();
    assert!(c.request(REQUEST_VERIFIER_QUERY, &body3).await.is_err());
    assert!(
        next(&mut alice.app, 300).await.is_none(),
        "no copy for a query never answered"
    );
}

// acceptance: CER-30
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_grant_on_the_payload_channel_answers_the_waiting_stream_of_a_hosted_light_client() {
    // bob the node; alice the verifier, a light client beside it; w1 the
    // subject, beside it too; carol asks over a session
    let scene = Scene::new();
    let node = live_node("bob", scene.table("bob", &["bob"]), "bob", &[]);
    let inboxes = Inboxes::default();
    let serving: Arc<dyn Serving> = LocalNode::new(node.clone(), inboxes.clone());
    let verifiers = Verifiers::new();
    let alice = host("alice", buffered(5000), &serving, &inboxes, &verifiers);
    let mut w1 = host("w1", buffered(5000), &serving, &inboxes, &verifiers);
    verifiers.install(&node);
    alice.courier.attach(vec![kh("w1")]).await;
    w1.courier.attach(vec![kh("alice")]).await;
    alice.courier.sweep(vec![kh("w1")]).await;
    let q = query("w1", "carol", "alice", 9);
    let qid = q.query_id();
    let q1 = q.clone();
    let (consent, grant) = w1
        .handle
        .with(move |c| {
            c.subject.open_window([9; 32]);
            c.consent(&q1)
        })
        .await
        .expect("w1 consents");
    assert!(
        grant.is_none(),
        "w1 holds no capture alice sealed, so no grant travels with the consent"
    );
    let body = QueryRequest {
        query: q,
        consent,
        selection_basis: 1,
    }
    .encode();
    let ccfg = client_cfg("carol");
    know(&ccfg, "bob", node.addr);
    let AttachOutcome::Attached(c) = attach(&ccfg, &client_ep(), kh("bob"), node.addr, false).await
    else {
        panic!("carol attaches")
    };
    let t0 = Instant::now();
    let stream = tokio::spawn(async move { c.request(REQUEST_VERIFIER_QUERY, &body).await });
    // the query waits for its grant, the stream with it
    assert!(
        until(2000, || alice
            .handle
            .with_blocking(|c| c.verifier.awaiting())
            == vec![qid])
        .await
    );
    // w1's grant, for a record alice does not hold, over the payload channel
    let grant = KeyGrant {
        record: [5; 32],
        query_id: qid,
        key: [1; 32],
    }
    .encode();
    w1.courier
        .send(kh("alice"), KIND_KEY_GRANT, grant)
        .await
        .expect("sent");
    let reply = tokio::time::timeout(Duration::from_secs(3), stream)
        .await
        .expect("answered within the bound, not at it")
        .unwrap()
        .expect("answered on the stream");
    assert!(t0.elapsed() < Duration::from_secs(5));
    let r = Response::read(&reply).unwrap();
    assert_eq!(
        (r.verifier, r.subject, r.verdict, r.basis),
        (kh("alice"), kh("w1"), Verdict::Unavailable, None),
        "the record named is not held: unavailable, nothing compared"
    );
    verify::response(&ids(), &reply, false).expect("alice's signature");
    // and w1 gets its copy
    let (from, d) = next(&mut w1.app, 3000).await.expect("the copy arrives");
    assert_eq!(from, kh("alice"));
    assert!(
        matches!(d, Dispatched::ResponseCopy(Ok(id)) if id == qid),
        "{d:?}"
    );
}

// acceptance: CER-31
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_query_from_the_wrong_requester_leaves_a_waiting_request_where_it_was() {
    // alice the verifier beside node bob, w1 the subject; carol asks
    // legitimately and w2 replays her signed body under its own identity
    let scene = Scene::new();
    let node = live_node("bob", scene.table("bob", &["bob"]), "bob", &[]);
    let inboxes = Inboxes::default();
    let serving: Arc<dyn Serving> = LocalNode::new(node.clone(), inboxes.clone());
    let verifiers = Verifiers::new();
    let alice = host("alice", buffered(5000), &serving, &inboxes, &verifiers);
    let w1 = host("w1", buffered(5000), &serving, &inboxes, &verifiers);
    verifiers.install(&node);
    alice.courier.attach(vec![kh("w1")]).await;
    w1.courier.attach(vec![kh("alice")]).await;
    alice.courier.sweep(vec![kh("w1")]).await;
    let q = query("w1", "carol", "alice", 11);
    let qid = q.query_id();
    let q1 = q.clone();
    let (signed, _) = w1
        .handle
        .with(move |c| {
            c.subject.open_window([11; 32]);
            c.consent(&q1)
        })
        .await
        .expect("w1 consents");
    let body = QueryRequest {
        query: q,
        consent: signed,
        selection_basis: 1,
    }
    .encode();
    let ccfg = client_cfg("carol");
    know(&ccfg, "bob", node.addr);
    let AttachOutcome::Attached(c) = attach(&ccfg, &client_ep(), kh("bob"), node.addr, false).await
    else {
        panic!("carol attaches")
    };
    let carols = body.clone();
    let stream = tokio::spawn(async move { c.request(REQUEST_VERIFIER_QUERY, &carols).await });
    assert!(
        until(2000, || alice
            .handle
            .with_blocking(|c| c.verifier.awaiting())
            == vec![qid])
        .await,
        "carol's query waits for its grant"
    );
    // w2 presents the same signed body: field 2 names carol, not w2
    let wcfg = client_cfg("w2");
    know(&wcfg, "bob", node.addr);
    let AttachOutcome::Attached(w) = attach(&wcfg, &client_ep(), kh("bob"), node.addr, false).await
    else {
        panic!("w2 attaches")
    };
    assert!(
        w.request(REQUEST_VERIFIER_QUERY, &body).await.is_err(),
        "the wrong requester gets nothing"
    );
    // carol's request is still the one waiting, and its grant still answers it
    assert_eq!(
        alice.handle.with_blocking(|c| c.verifier.awaiting()),
        vec![qid],
        "still pending, not cancelled"
    );
    let grant = KeyGrant {
        record: [5; 32],
        query_id: qid,
        key: [1; 32],
    }
    .encode();
    w1.courier
        .send(kh("alice"), KIND_KEY_GRANT, grant)
        .await
        .expect("sent");
    let reply = tokio::time::timeout(Duration::from_secs(3), stream)
        .await
        .expect("answered within the bound")
        .unwrap()
        .expect("answered on the stream");
    let r = Response::read(&reply).unwrap();
    assert_eq!(
        (r.verifier, r.subject, r.query_id),
        (kh("alice"), kh("w1"), qid)
    );
    verify::response(&ids(), &reply, false).expect("alice's signature");
}
