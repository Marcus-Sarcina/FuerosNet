
#[test]
fn review_20261125_idle_key_must_expire() {
    use rhtn_node::node_keys::NodeKeyService;
    let mut service = NodeKeyService::new(NodeKeyConfig::default());
    let record = service.newest(&id("bob"), &kh("carol"), T0);
    let n = node_key(&record).fields.n;
    service.prune(&kh("carol"), &|_| false, T0 + 100 * DAY);
    eprintln!("100 days later, no queued messages: held={:?}", service.held(&kh("carol")));
    assert!(!service.accepts(&kh("carol"), n, T0 + 100 * DAY), "an idle key must not be accepted 100 days later");
}

#[test]
fn review_20261125_published_key_must_survive_restart() {
    use rhtn_node::node_keys::NodeKeyService;
    let root = temp("review-write-failure");
    std::fs::create_dir_all(&root).unwrap();
    std::fs::write(root.join("node-keys"), b"not a directory").unwrap();
    let mut service = NodeKeyService::at(&root, NodeKeyConfig::default()).unwrap();
    let record = service.newest(&id("bob"), &kh("carol"), T0);
    let n = node_key(&record).fields.n;
    assert!(service.accepts(&kh("carol"), n, T0));
    drop(service);
    let reopened = NodeKeyService::at(&root, NodeKeyConfig::default()).unwrap();
    let held = reopened.held(&kh("carol"));
    std::fs::remove_dir_all(&root).unwrap();
    assert!(held.contains(&n), "the service published and accepted a key it could not persist: {held:?}");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn review_20261125_old_session_must_not_detach_replacement() {
    use rhtn_transport::session::*;
    let b = Bench::start(T0, None, NodeKeyConfig::default(), |_| {});
    let cfg = ClientConfig {
        me: rhtn_transport::tls::Party::of(std::sync::Arc::new(id("carol"))),
        pins: b.pins.clone(), bind: Default::default(), capabilities: Default::default(),
        attestation: None, filter: None, sibling_cache: Default::default(),
        addresses: Default::default(), cursors: Default::default(), tls: Default::default(),
        connect_timeout: std::time::Duration::from_secs(3), on_reachability: None,
        log: Log::default(),
    };
    let ep = rhtn_transport::tls::client_endpoint("127.0.0.1:0".parse().unwrap()).unwrap();
    let AttachOutcome::Attached(first) = attach(&cfg, &ep, kh("bob"), b.live.addr, false).await else { panic!("first attach"); };
    let AttachOutcome::Attached(second) = attach(&cfg, &ep, kh("bob"), b.live.addr, false).await else { panic!("second attach"); };
    first.conn.close(0u32.into(), b"old connection closed");
    // Allow the node to process the old stream's EOF. This is not a heartbeat timeout.
    tokio::time::sleep(std::time::Duration::from_millis(500)).await;
    assert!(second.conn.close_reason().is_none(), "replacement transport remains live");
    assert!(b.live.view.lock().unwrap().attached.contains(&kh("carol")), "old teardown removed the still-connected client from the node view");
}
