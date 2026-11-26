
#[test]
fn review_20261125_hosted_session_must_be_fresh_after_restart() {
    let sc = scene();
    let backend = Arc::new(Fake::new());
    let serve_as = |role: &str| {
        let mut g = gateway_with(&sc, backend.clone());
        g.set_row(R1, kh("carol"), Row {
            roles: BTreeSet::from([role.to_string()]), connect: true,
        }).unwrap();
        assert_eq!(g.serve(&kh("alice"), &sc.table, &kh("carol"), &request(R1, OK_GET), 0).status, STATUS_DELIVERED);
        g.session(&kh("carol"), &R1).unwrap()
    };
    let first = serve_as("writer");
    let restarted = serve_as("reader");
    assert_ne!(first, restarted, "a fresh gateway reused the previous role context's session identifier");
}
