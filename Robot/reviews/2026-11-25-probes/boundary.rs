
#[test]
fn review_20261125_missing_relation_must_refuse_start() {
    let shell = Arc::new(Shell::default());
    let p = Participant::start(seeds("alice"), vec![], platform_of(shell.clone())).unwrap();
    p.hold_contact(material("bob")).unwrap();
    p.save().unwrap();
    assert_eq!(p.contacts().len(), 1);
    drop(p);
    assert!(shell.store.lock().unwrap().remove("payload").is_some());
    let restarted = Participant::start(seeds("alice"), vec![], platform_of(shell));
    if let Ok(p) = &restarted {
        eprintln!("partial store accepted; contacts after restart = {}", p.contacts().len());
    }
    assert!(restarted.is_err(), "an incomplete previously saved store must not become a fresh client");
}
