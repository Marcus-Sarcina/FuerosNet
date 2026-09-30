//! A query rides the end-to-end path to its verifier and the answer rides
//! it back (`wire-format.md` §5.6): the querier's client hands the request
//! to the courier, which routes it as it routes payload — direct where the
//! path is held, through the serving node otherwise (design §12.6.3) — and
//! the verifier's answer returns the way the query came, its copy to the
//! subject as always.

mod common;

use common::*;
use rhtn_adaptors::actor::Handle;
use rhtn_adaptors::courier::Courier;
use rhtn_adaptors::direct::{NoDirect, Reachable};
use rhtn_adaptors::serving::{Inboxes, LocalNode, Serving};
use rhtn_archive::Keyhash;
use rhtn_client::ceremony::{Config, Dispatched, Msg};
use rhtn_client::query::*;
use rhtn_client::selection::SelectionBasis;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::mpsc::UnboundedReceiver;

struct Party {
    handle: Handle,
    courier: Arc<Courier>,
    app: UnboundedReceiver<(Keyhash, Dispatched)>,
}

/// A light client beside the node, with no direct path: everything it
/// sends to a peer goes through the node as relay.
fn host(name: &'static str, cfg: Config, serving: &Arc<dyn Serving>, inboxes: &Inboxes) -> Party {
    let reachable = Reachable::default();
    let handle = spawn_client(name, cfg, reachable.clone());
    let (courier, app) = Courier::new(
        handle.clone(),
        serving.clone(),
        Arc::new(NoDirect(reachable)),
    );
    inboxes.host(kh(name), courier.inbound());
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

// acceptance: CER-43
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_query_reaches_its_verifier_on_the_end_to_end_path_and_the_answer_comes_back_on_it() {
    // bob the node; alice and w2 in a ceremony beside it; w1, alice's
    // chosen verifier for w2, a light client beside it too — reachable
    // only through bob, since nobody here holds a direct path
    let scene = Scene::new();
    let node = live_node("bob", scene.table("bob", &["bob"]), "bob", &[]);
    let inboxes = Inboxes::default();
    let serving: Arc<dyn Serving> = LocalNode::new(node.clone(), inboxes.clone());
    let mut alice = host("alice", buffered(400), &serving, &inboxes);
    let w1 = host("w1", buffered(400), &serving, &inboxes);
    let mut w2 = host("w2", buffered(400), &serving, &inboxes);
    for p in [&alice, &w1, &w2] {
        p.courier
            .attach(vec![kh("alice"), kh("w1"), kh("w2")])
            .await;
    }
    for p in [&alice, &w1, &w2] {
        p.courier.sweep(vec![kh("alice"), kh("w1"), kh("w2")]).await;
    }
    // the ceremony, to the point where alice holds a template of w2 and a
    // query about w2 for w1
    let ia = alice
        .handle
        .with(|c| c.begin(kh("w2"), vec![], true))
        .await
        .expect("alice begins");
    let iw = w2
        .handle
        .with(|c| c.begin(kh("alice"), vec![], false))
        .await
        .expect("w2 begins");
    let iw2 = iw.clone();
    let cid = alice
        .handle
        .with(move |c| c.take_intent(kh("w2"), &iw2))
        .await
        .expect("alice takes w2's intent");
    let ia2 = ia.clone();
    let cid_w = w2
        .handle
        .with(move |c| c.take_intent(kh("alice"), &ia2))
        .await
        .expect("w2 takes alice's intent");
    assert_eq!(cid, cid_w, "one ceremony");
    let key_w = w2.handle.with(|c| c.capture_key()).await.expect("w2's key");
    alice
        .handle
        .with(move |c| c.capture(key_w))
        .await
        .expect("alice captures w2");
    let q = alice
        .handle
        .with(|c| c.query_for(kh("w1")))
        .await
        .expect("a query about w2, for w1");
    let qid = q.query_id();
    assert_eq!(
        (q.subject, q.querier, q.verifier),
        (kh("w2"), kh("alice"), kh("w1"))
    );
    // w2 consents, within the window its own ceremony opened; it holds no
    // capture w1 sealed, so no grant travels
    let q1 = q.clone();
    let (signed, grant) = w2
        .handle
        .with(move |c| c.consent(&q1))
        .await
        .expect("w2 consents");
    assert!(grant.is_none());
    // the request goes out as the client's own message, and the courier
    // carries it to w1 — through bob, the one path there is
    let q2 = q.clone();
    let req = alice
        .handle
        .with(move |c| c.request(&q2, signed, SelectionBasis::Discretionary))
        .await;
    let t0 = Instant::now();
    let carried = alice.courier.carry(vec![Msg::Query(req)]).await;
    assert!(carried.complete(), "{carried:?}");
    // w1 holds no capture of w2: the query waits for a grant that never
    // comes, and past the bound is answered unavailable — back to alice on
    // the path it came, verified under w1, against the query she issued
    let (from, d) = next(&mut alice.app, 5000)
        .await
        .expect("the answer arrives");
    assert_eq!(from, kh("w1"));
    assert!(
        matches!(d, Dispatched::Response(Ok((id, Verdict::Unavailable))) if id == qid),
        "{d:?}"
    );
    assert!(
        t0.elapsed() >= Duration::from_millis(400),
        "the buffer bound passed first"
    );
    let gathered = alice.handle.with(|c| c.responses()).await;
    assert_eq!(gathered.len(), 1, "gathered for the record");
    let r = Response::read(&gathered[0]).unwrap();
    assert_eq!(
        (r.verifier, r.subject, r.query_id),
        (kh("w1"), kh("w2"), qid)
    );
    // and w2, the subject, gets its copy from w1
    let (from, d) = next(&mut w2.app, 3000).await.expect("the copy arrives");
    assert_eq!(from, kh("w1"));
    assert!(
        matches!(d, Dispatched::ResponseCopy(Ok(id)) if id == qid),
        "{d:?}"
    );
    // a response to a query alice never issued is refused on arrival, and
    // enters nothing
    let stray = w1
        .handle
        .with(move |c| {
            let q = VerificationQuery {
                subject: kh("w2"),
                querier: kh("alice"),
                ceremony_id: [9; 32],
                profile: vec![1],
                template_version: 1,
                verifier: kh("w1"),
            };
            let bytes = Response {
                verifier: kh("w1"),
                subject: kh("w2"),
                query_id: q.query_id(),
                verdict: Verdict::Match,
                basis: Some(Basis::PersonalKnowledge),
                template_version: None,
                consent: consent(&id("w2"), &q.query_id()),
                selection_basis: 3,
            }
            .sign(&id("w1"));
            c.send_payload(kh("alice"), rhtn_client::payload::KIND_RESPONSE, &bytes)
        })
        .await
        .expect("w1 sends");
    w1.courier.carry(stray).await;
    let (_, d) = next(&mut alice.app, 3000).await.expect("it arrives");
    assert!(
        matches!(d, Dispatched::Response(Err(ref why)) if why.contains("not a query I issued")),
        "{d:?}"
    );
    assert_eq!(alice.handle.with(|c| c.responses()).await.len(), 1);
}
