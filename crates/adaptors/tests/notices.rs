//! **What a resource counts as waiting reaches the person's launcher, and
//! moves when it moves** (`wire-format.md` §6.9; `resource-requirements.md`
//! §3.3) [author, 2026-11-23]: the node carries a resource's count and its
//! line on the member's page, and frame 9 — carrying nothing — is what
//! has the client ask for that page again.

mod common;

use common::*;
use rhtn_adaptors::courier::Courier;
use rhtn_adaptors::direct::{NoDirect, Reachable};
use rhtn_adaptors::serving::{Answer, Serving};
use rhtn_archive::Keyhash;
use rhtn_archive::catalog::{CatalogPage, CatalogPageRequest, FRAME_PAGE_CHANGED, PageItem};
use rhtn_archive::submission::WakeEndpoint;
use rhtn_client::ceremony::Config;
use std::sync::{Arc, Mutex};

/// A node whose only answer is the asker's page, holding one resource and
/// whatever that resource last counted.
struct Pages {
    me: Keyhash,
    counted: Mutex<(u32, Option<String>)>,
}

impl Serving for Pages {
    fn me(&self) -> Keyhash {
        self.me
    }
    fn serves(&self, _: &Keyhash) -> bool {
        true
    }
    fn teach<'a>(&'a self, _: Vec<Vec<u8>>) -> Answer<'a, usize> {
        Box::pin(async { 0 })
    }
    fn publish_profile<'a>(&'a self, _: &'a [u8]) -> Answer<'a, bool> {
        Box::pin(async { false })
    }
    fn assert_horizon<'a>(&'a self, _: &'a [u8], _: [u8; 16]) -> Answer<'a, bool> {
        Box::pin(async { false })
    }
    fn profile<'a>(&'a self, _: &'a [u8]) -> Answer<'a, Option<Vec<u8>>> {
        Box::pin(async { None })
    }
    fn catalog<'a>(&'a self, _: Keyhash, _: &'a [u8]) -> Answer<'a, Option<Vec<u8>>> {
        Box::pin(async { None })
    }
    fn page<'a>(&'a self, _: Keyhash, body: &'a [u8]) -> Answer<'a, Option<Vec<u8>>> {
        Box::pin(async move {
            let nonce = CatalogPageRequest::decode(body).ok()?.nonce;
            let (count, line) = self.counted.lock().unwrap().clone();
            Some(
                CatalogPage {
                    nonce,
                    items: vec![PageItem {
                        resource: [0x5a; 32],
                        owner: kh("bob"),
                        name: "testapp.internal".into(),
                        roles: vec![],
                        count,
                        line,
                    }],
                }
                .encode(),
            )
        })
    }
    fn resource<'a>(&'a self, _: Keyhash, _: &'a [u8]) -> Answer<'a, Option<Vec<u8>>> {
        Box::pin(async { None })
    }
    fn wake<'a>(&'a self, _: Keyhash, _: Option<WakeEndpoint>) -> Answer<'a, bool> {
        Box::pin(async { false })
    }
    fn propagate<'a>(&'a self, _: Vec<u8>) -> Answer<'a, bool> {
        Box::pin(async { false })
    }
}

// functional: CAT-030
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn frame_nine_has_the_client_ask_its_page_again_and_the_count_and_line_arrive() {
    let handle = spawn_client("alice", Config::default(), Reachable::default());
    let (courier, _app) = Courier::new(handle.clone(), Arc::new(NoDirect(Reachable::default())));
    let pages = Arc::new(Pages {
        me: kh("w1"),
        counted: Mutex::new((1, Some("Your order is ready at the counter".into()))),
    });
    courier.serve_also(pages.clone());
    handle.with(|c| c.attach(kh("w1"), &[])).await;
    let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
    let task = rhtn_adaptors::attached::follow(handle.clone(), kh("w1"), courier, rx);
    let held = |h: &rhtn_adaptors::actor::Handle| {
        h.with_blocking(|c| c.resources_held())
            .into_iter()
            .map(|r| (r.waiting, r.notice))
            .collect::<Vec<_>>()
    };
    assert!(held(&handle).is_empty(), "nothing before the node says so");

    tx.send((FRAME_PAGE_CHANGED, rhtn_archive::catalog::page_changed()))
        .unwrap();
    assert!(
        until(3000, || held(&handle)
            == vec![(
                1,
                Some("Your order is ready at the counter".into())
            )])
        .await,
        "the page asked again carries the count and its line"
    );

    // **the latest replaces what it said**, and none withdraws it
    *pages.counted.lock().unwrap() = (0, None);
    tx.send((FRAME_PAGE_CHANGED, rhtn_archive::catalog::page_changed()))
        .unwrap();
    assert!(
        until(3000, || held(&handle) == vec![(0, None)]).await,
        "a count of none is nothing waiting"
    );
    drop(tx);
    let _ = task.await;
}
